//! Calendar entry and edition-specific phase dispatch.
//!
//! Preserve chronological day replay, native phase order and status-suppression
//! gates. The synchronous month fixture remains test-only; idle/modal scheduling,
//! phase workers, XTRF simulation and road rendering retain their existing owners.

use super::ViewerState;
use super::calendar::{
    SIM_DAY_BUDGET, SIM_DAY_DEMAND, SIM_DAY_DENSITY, SIM_DAY_GROWTH_FIRST, SIM_DAY_GROWTH_LAST,
    SIM_DAY_POWER, SIM_DAY_SCENARIO, SIM_DAY_STATS_DIALOGS, SIM_DAY_TRAFFIC, SIM_DAY_UI_CHROME,
    SIM_DAY_WATER, sim_calendar_growth_phase,
};
use super::city_valuation::sc2kfix_pe_recalculate_city_value;
use super::scenario_lifecycle::reward_available_status_line;
use crate::city::CityDate;
use crate::game_clock::{GameClock, GameClockAdvance};

impl ViewerState {
    #[cfg(test)]
    pub(super) fn handle_simulation_month_advanced(&mut self) {
        // Synchronous phase-math fixture, not the live modal/idle dispatcher.
        // Win95 `dwCityDays % 25` SimCalendar, batched into one month tick.
        self.query_status_pinned = false;
        self.sim_calendar_day0_budget_and_date();
        let had_active_disaster = self.city.has_active_disaster();
        // Calendar phases do not own the separate pending disaster dispatcher.
        if self.engine_fidelity.uses_win95() {
            self.sim_calendar_day1_power();
            // Snapshot outcomes so density and later day-21 demand use the same pre-growth maps.
            let population = self.live_population_stats_for_fidelity();
            let outcomes = self.live_city_service_outcomes_with_process_crt();
            self.sim_calendar_day2_density(population, outcomes);
            // PE growth uses last month's `wCityDemand`; day 21 writes the next step.
            self.sim_calendar_days_3_18_growth();
            self.sim_calendar_day19_traffic();
        } else {
            // These Mac phases are independent of the synthetic zone-
            // construction toggle. Phase 2 rebuilds derived maps; phases
            // 3–18 run the recovered partitioned scan, including weighted
            // counters, transport upkeep/routes, and bounded growth/special
            // placement paths; phase 19 decays/sums XTRF. Phase 21 currently
            // publishes from those six counters, but unmodeled scan mutations
            // and whole-month QuickDraw chronology mean those are provisional
            // inputs, not a claim of native demand parity. CODE 14 phase 1
            // follows phase 0 in the same monthly dispatch.
            self.sim_calendar_day1_power();
            let population = self.live_population_stats_for_fidelity();
            let outcomes = self.live_city_service_outcomes_with_process_crt();
            self.sim_calendar_day2_density(population, outcomes);
            self.sim_calendar_mac_days_3_18_rci_scan();
            self.sim_calendar_day19_traffic();
        }
        self.sim_calendar_day20_water();
        self.publish_pe_growth_aggregate();
        let (population, outcomes) = self.sim_calendar_day21_demand_and_graphs();
        let day22 = self.sim_calendar_day22_scenario(had_active_disaster, population, outcomes);
        let scenario_outcome_announced = day22.scenario_outcome_announced;
        self.sim_calendar_day23_stats_dialogs();
        let newly_available_rewards = self.sim_calendar_day24_ui_chrome();
        if !self.query_is_inspect && !scenario_outcome_announced && !day22.ambient_disaster_started
        {
            if newly_available_rewards != 0 {
                self.set_sim_status_announcement(reward_available_status_line(
                    newly_available_rewards,
                ));
            } else if !day22.status_announced && !self.query_status_pinned {
                self.refresh_idle_status_line();
            }
        }
    }

    pub(super) fn apply_game_clock_advance(&mut self, start: GameClock, advance: GameClockAdvance) {
        for step in 1..=advance.days {
            let at = start.wrapping_add_days(step);
            self.handle_simulation_day_advanced(at.year(), at.month(), at.day());
        }
    }

    pub(super) fn handle_simulation_day_advanced(&mut self, year: u16, month: u8, mon_day: u8) {
        if self.engine_fidelity.uses_win95() {
            // SC2KFix `b10a5d9`: retail only recalculates city value in bond
            // checks, so the bug-fixed Win95 path refreshes it once per day
            // before the calendar day's simulation work.
            sc2kfix_pe_recalculate_city_value(&mut self.city);
        }
        let mon_day_u32 = u32::from(mon_day);
        if mon_day_u32 == SIM_DAY_BUDGET {
            self.query_status_pinned = false;
            self.sim_calendar_day0_budget_and_date_at(year, month, mon_day);
        } else {
            self.city
                .set_saved_date_and_day(CityDate { year, month }, mon_day);
        }

        if self.engine_fidelity.uses_win95() {
            match mon_day_u32 {
                SIM_DAY_POWER => self.sim_calendar_day1_power(),
                SIM_DAY_DENSITY => {
                    let population = self.live_population_stats_for_fidelity();
                    let outcomes = self.live_city_service_outcomes_with_process_crt();
                    self.sim_calendar_day2_density(population, outcomes);
                }
                d if (SIM_DAY_GROWTH_FIRST..=SIM_DAY_GROWTH_LAST).contains(&d) => {
                    if d == SIM_DAY_GROWTH_FIRST {
                        self.reset_monthly_growth_budget();
                    }
                    if let Some((step, substep)) = sim_calendar_growth_phase(d) {
                        self.sim_calendar_one_growth_tick(step, substep);
                    }
                }
                SIM_DAY_TRAFFIC => self.sim_calendar_day19_traffic(),
                _ => {}
            }
        } else {
            match mon_day_u32 {
                SIM_DAY_POWER => self.sim_calendar_day1_power(),
                SIM_DAY_DENSITY => {
                    let population = self.live_population_stats_for_fidelity();
                    let outcomes = self.live_city_service_outcomes_with_process_crt();
                    self.sim_calendar_day2_density(population, outcomes);
                }
                d if (SIM_DAY_GROWTH_FIRST..=SIM_DAY_GROWTH_LAST).contains(&d) => {
                    let phase = (d - SIM_DAY_GROWTH_FIRST) as usize;
                    self.sim_calendar_mac_rci_scan_phase(phase);
                }
                SIM_DAY_TRAFFIC => self.sim_calendar_day19_traffic(),
                _ => {}
            }
        }

        match mon_day_u32 {
            SIM_DAY_WATER => self.sim_calendar_day20_water(),
            SIM_DAY_DEMAND => {
                self.publish_pe_growth_aggregate();
                let (_population, _outcomes) = self.sim_calendar_day21_demand_and_graphs();
                if !self.query_is_inspect {
                    self.refresh_status_line();
                }
            }
            SIM_DAY_SCENARIO => {
                let had_active_disaster = self.city.has_active_disaster();
                let population = self.live_population_stats_for_fidelity();
                let outcomes = self.live_city_service_outcomes_with_process_crt();
                let day22 =
                    self.sim_calendar_day22_scenario(had_active_disaster, population, outcomes);
                let scenario_outcome_announced = day22.scenario_outcome_announced;
                if !self.query_is_inspect
                    && !scenario_outcome_announced
                    && !day22.ambient_disaster_started
                    && !day22.status_announced
                {
                    self.refresh_status_line();
                }
            }
            SIM_DAY_STATS_DIALOGS => self.sim_calendar_day23_stats_dialogs(),
            SIM_DAY_UI_CHROME => {
                let newly_available_rewards = self.sim_calendar_day24_ui_chrome();
                if newly_available_rewards != 0 {
                    self.set_sim_status_announcement(reward_available_status_line(
                        newly_available_rewards,
                    ));
                } else if !self.query_is_inspect {
                    self.refresh_status_line();
                }
            }
            _ => {}
        }
    }
}
