//! Scenario lifecycle, bankruptcy and city progression adapters.
//!
//! Calendar scheduling and goal/counter workers retain their existing owners.
//! These adapters preserve edition gates, deadline/milestone writes, day-22
//! ordering and announcement suppression around existing UI/disaster calls.

use super::{
    CITY_PROGRESSION_REQUIREMENTS, MILITARY_BASE_NONE, MISC_CITY_PROGRESSION_OFFSET,
    PlacementSound, PopulationStats, ScenarioOutcome, ViewerState, ordinance_flags_for_city,
    pe_misc_mirror_storage_active, persist_pe_garbage_day22, scenario_goal_counts,
    scenario_goals_for_city, scenario_outcome_for_progress, scenario_outcome_message,
    scenario_progress_for_city_with_entry_seed,
};
use crate::city::{CityFileKind, MacScenarioResult};
use crate::game_clock::GameClock;

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Day22Outcome {
    pub(super) status_announced: bool,
    pub(super) scenario_outcome_announced: bool,
}

#[derive(Debug, Default, Clone, Copy)]
struct CityProgressionAdvance {
    advanced: bool,
    newly_granted_rewards: u32,
    military_offered: bool,
}

impl ViewerState {
    pub(super) fn scenario_deadline_base_clock(&self) -> GameClock {
        self.scenario_start_clock.unwrap_or(self.game_clock)
    }

    pub(super) fn sim_calendar_day22_scenario(
        &mut self,
        population: PopulationStats,
    ) -> Day22Outcome {
        if !self.engine_fidelity.uses_win95() {
            let progression = self.advance_mac_city_progression();
            let status_announced = self.announce_city_progression(progression);
            // A Mac military response resumes the same phase-22 tail.
            let scenario_outcome_announced =
                !progression.military_offered && self.tick_mac_scenario_tail();
            return Day22Outcome {
                status_announced,
                scenario_outcome_announced,
            };
        }
        // Win95 `0x4681ea`: `wCityGarbage += slot0*10` lives in day-21 aggregate
        // publish when growth ran; census stand-in remains for growth-disabled paths.
        persist_pe_garbage_day22(&mut self.city, population);
        let progression = self.advance_city_progression_milestones();
        let status_announced = self.announce_city_progression(progression);
        self.tick_pe_scenario_time_limit();
        // PE @ 0x413a4a handles a scenario result before the independent
        // bankruptcy check at 0x413a52. Bankruptcy can then replace the
        // status, but must not erase the already-processed scenario outcome.
        let scenario_outcome_announced = self.refresh_scenario_outcome_message();
        let bankruptcy_announced = self.tick_pe_bankruptcy_impeachment();
        Day22Outcome {
            status_announced: bankruptcy_announced || status_announced,
            scenario_outcome_announced,
        }
    }

    fn announce_city_progression(&mut self, progression: CityProgressionAdvance) -> bool {
        let message = if progression.military_offered {
            self.open_military_base_dialog();
            return true;
        } else if self.query_is_inspect {
            return false;
        } else if progression.newly_granted_rewards != 0 {
            reward_available_status_line(progression.newly_granted_rewards)
        } else if progression.advanced {
            "City Milestone"
        } else {
            return false;
        };
        self.set_sim_status_announcement(message);
        true
    }

    fn advance_mac_city_progression(&mut self) -> CityProgressionAdvance {
        let level = self.city.mac_city_progression();
        let Some(&requirement) = CITY_PROGRESSION_REQUIREMENTS.get(level as usize) else {
            return CityProgressionAdvance::default();
        };
        // CODE 14 compares the threshold as a DWORD against export 237's
        // published six-counter growth scalar.
        if self.city.mac_state_misc_u32_at(0x102c).unwrap_or(0) <= requirement {
            return CityProgressionAdvance::default();
        }

        let next_level = level + 1;
        // The milestone's selector-3 Extra bulletin precedes reward/offer work.
        self.city
            .mac_poll_for_extra(|| self.mac_process_rng.random_u16());
        if next_level == 4 {
            // Commit level 4 with the response so a save taken while the
            // modal is open cannot skip the still-unanswered offer.
            return CityProgressionAdvance {
                military_offered: true,
                ..CityProgressionAdvance::default()
            };
        }
        let before = self.city.mac_civic_latch();
        self.city.set_mac_city_progression(next_level);
        match next_level {
            1..=3 => self
                .city
                .set_mac_civic_latch(before | (1 << (next_level - 1))),
            5 => self.city.set_mac_civic_latch(before | (1 << 3)),
            6 => self.city.refresh_mac_arcology_latch(),
            _ => {}
        }
        CityProgressionAdvance {
            advanced: true,
            newly_granted_rewards: self.city.mac_civic_latch() & !before,
            military_offered: false,
        }
    }

    /// PE day-22: `dwCityFunds < -100000` → `GAMEOVER_BANKRUPT` (mayor impeached).
    fn tick_pe_bankruptcy_impeachment(&mut self) -> bool {
        const PE_BANKRUPT_FUNDS: i32 = -100_000;
        if self.announced_bankruptcy || self.city.city_funds() >= PE_BANKRUPT_FUNDS {
            return false;
        }
        self.announced_bankruptcy = true;
        // PE GAMEOVER_BANKRUPT requests sound 0x200 before its result dialog.
        self.queue_disaster_extra_sounds(PlacementSound::Riot, 1);
        // PE outcome helper FUN_004149B0 clears `0x4CAD44` for terminal
        // result paths, including bankruptcy, before any result UI runs.
        self.city.set_pe_scenario_active(false);
        if self.query_is_inspect {
            return false;
        }
        self.set_sim_status_announcement("Mayor Impeached - City Bankrupt");
        true
    }

    /// The native scenario result runs before the independent bankruptcy check.
    pub(super) fn tick_mac_scenario_tail(&mut self) -> bool {
        let mut announced = false;
        if let Some(result) = self.city.tick_mac_saved_scenario() {
            let (outcome, sound) = match result {
                MacScenarioResult::Won => (ScenarioOutcome::Complete, PlacementSound::Cheers),
                MacScenarioResult::TimedOut => (ScenarioOutcome::Failed, PlacementSound::Riot),
            };
            self.queue_disaster_extra_sounds(sound, 1);
            self.announced_scenario_outcome = Some(outcome);
            self.set_sim_status_announcement(scenario_outcome_message(outcome));
            announced = true;
        }
        self.tick_mac_bankruptcy_outcome();
        announced
    }

    /// Mac CODE 14 phase 22: cash below -100,000 invokes `FUN_00027a06(2)`.
    /// The rooted outcome handler clears the persisted scenario-active flag;
    /// native modal/window behavior is outside this simulation handler.
    fn tick_mac_bankruptcy_outcome(&mut self) {
        const MAC_BANKRUPT_FUNDS: i32 = -100_000;
        if self.announced_bankruptcy || self.city.city_funds() >= MAC_BANKRUPT_FUNDS {
            return;
        }
        self.announced_bankruptcy = true;
        self.city.set_mac_scenario_active(false);
        self.queue_disaster_extra_sounds(PlacementSound::Riot, 1);
    }

    /// PE day-22: on unmet goals, `--wScenarioTimeLimitMonths` (`0x4CC91C` / SCEN `0x08`);
    /// hitting 0 fires scenario failure. Victory when all measurable goals are already met.
    fn tick_pe_scenario_time_limit(&mut self) {
        if self.city.kind != CityFileKind::WinScenario || !self.city.pe_scenario_active() {
            return;
        }
        let Some(goals) = scenario_goals_for_city(&self.city) else {
            return;
        };
        let progress =
            scenario_progress_for_city_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let counts = scenario_goal_counts(&self.city, goals, progress);
        if counts.pending == 0 && counts.complete == counts.known && !counts.implicit_minimum_failed
        {
            return;
        }
        let Some(remaining) = self.city.scenario_time_limit_months() else {
            return;
        };
        // PE uses `dec word ptr [wScenarioTimeLimitMonths]` without a zero
        // guard; preserve the 16-bit wrap when a custom SCEN starts at zero.
        let next = remaining.wrapping_sub(1);
        self.city.set_scenario_time_limit_months(next);
        if next == 0 {
            self.city.set_pe_scenario_deadline_expired(true);
        }
    }

    fn advance_city_progression_milestones(&mut self) -> CityProgressionAdvance {
        // Win95 / sc2kfix day 22: one milestone per month when
        // `dwCityProgressionRequirements[wCityProgression] < dwCityPopulation`
        // (`0x4CAA74` / `pe_dw_city_population`), not live census totals.
        let level = u32::from(self.city.pe_w_city_progression);
        let Some(&requirement) = CITY_PROGRESSION_REQUIREMENTS.get(level as usize) else {
            return CityProgressionAdvance::default();
        };
        let city_population = self.city.pe_dw_city_population();
        if requirement == 0 || requirement >= city_population {
            return CityProgressionAdvance::default();
        }

        let new_level = level.saturating_add(1);
        self.city.pe_w_city_progression = new_level as u16;
        if pe_misc_mirror_storage_active(&self.city, self.engine_fidelity) {
            self.city
                .set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, new_level);
        }

        let mut newly_granted_rewards = 0_u32;
        let mut military_offered = false;
        if new_level >= 4 {
            if new_level == 4 {
                military_offered = self.city.pe_w_military_base_type == MILITARY_BASE_NONE;
            } else if new_level == 5 {
                newly_granted_rewards = 1 << 3;
            }
        } else {
            newly_granted_rewards = 1 << (new_level.saturating_sub(1));
        }

        if newly_granted_rewards != 0 {
            let available = self.city.rewards_available_mask();
            let updated = available | newly_granted_rewards;
            newly_granted_rewards &= !available;
            if updated != available {
                self.city.set_rewards_available_mask(updated);
            }
        }

        // PE progression worker @ 0x4138ee rebuilds tech unlocks immediately
        // after a successful milestone. This second call matters when the
        // transition itself reaches level 6 and opens the Arcologies gate.
        let _ = self
            .city
            .pe_rebuild_tech_unlock_mask_46a0be(ordinance_flags_for_city(&self.city));

        CityProgressionAdvance {
            advanced: true,
            newly_granted_rewards,
            military_offered,
        }
    }

    pub(super) fn refresh_scenario_outcome_message(&mut self) -> bool {
        if !self.engine_fidelity.uses_win95() || self.city.kind != CityFileKind::WinScenario {
            self.announced_scenario_outcome = None;
            return false;
        }
        // PE @ 0x41390b skips all SCEN checks once the active flag is clear.
        // Preserve the prior announcement when the scenario has already ended.
        if !self.city.pe_scenario_active() {
            return false;
        }
        let Some(goals) = scenario_goals_for_city(&self.city) else {
            self.announced_scenario_outcome = None;
            return false;
        };
        let progress =
            scenario_progress_for_city_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let outcome = scenario_outcome_for_progress(
            &self.city,
            goals,
            progress,
            self.scenario_deadline_base_clock(),
            self.game_clock,
        );
        let Some(outcome @ (ScenarioOutcome::Complete | ScenarioOutcome::Failed)) = outcome else {
            return false;
        };
        // PE FUN_004149B0 requests 0x201 on success and 0x200 on failure,
        // independent of CityGame's inspect-query status-line suppression.
        self.queue_disaster_extra_sounds(
            match outcome {
                ScenarioOutcome::Complete => PlacementSound::Cheers,
                ScenarioOutcome::Failed => PlacementSound::Riot,
                ScenarioOutcome::InProgress => unreachable!(),
            },
            1,
        );
        // PE FUN_004149B0 clears `0x4CAD44` on all terminal scenario outcomes,
        // even when inspect-query mode suppresses the result announcement.
        self.city.set_pe_scenario_active(false);
        if self.announced_scenario_outcome == Some(outcome) || self.query_is_inspect {
            return false;
        }

        self.announced_scenario_outcome = Some(outcome);
        self.set_sim_status_announcement(scenario_outcome_message(outcome));
        true
    }
}

pub(super) fn reward_available_status_line(mask: u32) -> &'static str {
    if mask.count_ones() == 1 {
        "Reward Available"
    } else {
        "Rewards Available"
    }
}
