//! Calendar budget entry and annual/monthly publication adapters.
//!
//! Preserve edition gates, settlement/publication order and native random-stream
//! chronology. The live PE modal/idle continuation, accounting models and native
//! microsim workers retain their existing owners; the synchronous fixture stays
//! test-only.

use super::{
    BUDGET_ORDINANCE_ROW, BUDGET_SERVICE_CURRENT_FIELD, CityInfoDialog,
    MISC_ARCOLOGY_POPULATION_OFFSET, PlacementSound, ViewerState, apply_mac_budget_ordinance_tail,
    auto_budget_enabled_for_engine, budget_row_misc_offset_for_city,
    budget_service_funding_raw_for_city, budget_tax_bases_for_city_with_fidelity,
    live_traffic_count_for_city_with_power, mac_auto_budget_enabled_for_city,
    mac_budget_clear_monthly_rci_scan_counters, mac_budget_month_snapshot_and_accrue,
    mac_budget_refresh_derived_currents, mac_ordinance_raw_cash_total, mac_raw_tax_rate_for_city,
    pe_attitude_demand_inputs_for_city_with_power, pe_budget_month_index,
    pe_fiscal_crisis_after_settlement, pe_misc_mirror_storage_active,
    pe_water_worker_utilization_percent, persist_pe_year_end_misc,
    persist_speed_misc_value_to_city, population_stats_for_city_with_fidelity,
    power_service_maps_with_entry_seed, publish_pe_sewer_bonus, service_percent,
    set_auto_budget_enabled_for_engine, set_mac_auto_budget_enabled, speed_misc_value_for_city,
    sync_bond_budget_row_for_city, water_service_maps_with_entry_seed_and_fidelity,
    water_service_summary_with_entry_seed,
};
use crate::city::{CityDate, CityFileKind};
use crate::engine_fidelity::EngineFidelity;
use crate::mac_menu;
use crate::traffic::GameSpeed;

impl ViewerState {
    /// Interleave arcology water passes with the QuickDraw-consuming rows.
    pub(super) fn refresh_mac_xmic_with_arcologies(
        &mut self,
        funding: [u32; 5],
        tax_rates: [u32; 3],
    ) {
        let water_seed = self.pe_process_crt.seed();
        let arcology_count = self.city.mac_arcology_count_snapshot();
        let mut arcology_total = 0_u32;
        let game_rng = &mut self.mac_game_rng;
        self.city.refresh_mac_monthly_random_xmic_rows_with_external_slots(
            funding[0], funding[1], funding[2], funding[3], funding[4],
            self.no_disasters_enabled, &mut self.mac_process_rng,
            |city, slot| {
                if city.microsim_at(slot).is_some_and(|row| row.tile_id == 0xda) {
                    city.refresh_mac_monthly_zoo_microsim_at(slot, game_rng);
                    return;
                }
                let maps = water_service_maps_with_entry_seed_and_fidelity(
                    city, water_seed, EngineFidelity::Mac68k,
                );
                if let Some(water_percent) = maps.mac_worker_utilization_percent() {
                    super::power_water_refresh::publish_mac_water_result_for_city(city, &maps);
                    city.refresh_mac_monthly_arcology_microsim_at(slot, tax_rates, water_percent, arcology_count);
                } else {
                    log::debug!("Mac arcology slot {slot} skipped: water-worker inputs outside the recovered boundary");
                }
                if let Some(row) = city.microsim_at(slot) {
                    arcology_total = arcology_total.wrapping_add(u32::from(row.stat2));
                }
            },
        );
        // CODE 3 adds the value immediately after each arcology row, even if
        // a later slot removes that row before the sweep finishes.
        self.city.set_mac_arcology_resident_total(arcology_total);
    }

    pub(super) fn open_pe_negative_auto_budget_notice(&mut self) {
        if !self.engine_fidelity.uses_win95()
            || !pe_fiscal_crisis_after_settlement(
                self.city.city_funds(),
                auto_budget_enabled_for_engine(&self.city, self.engine_fidelity),
            )
        {
            return;
        }
        // PE 0x47352f clears Auto-Budget after settlement/yearly XMIC, then
        // waits for MessageBox resource 0x124 before snapshotting monthly rows.
        set_auto_budget_enabled_for_engine(&mut self.city, self.engine_fidelity, false);
        self.sync_edit_map_menu();
        self.city.mark_all_dirty();
        self.open_city_info_dialog(CityInfoDialog::FiscalCrisis);
        self.set_sim_status_announcement("Fiscal crisis: Auto-Budget turned off");
    }

    #[cfg(test)]
    pub(super) fn sim_calendar_day0_budget_and_date(&mut self) {
        self.sim_calendar_day0_budget_and_date_at(
            self.game_clock.year(),
            self.game_clock.month(),
            self.game_clock.day(),
        );
    }

    pub(super) fn sim_calendar_day0_budget_and_date_at(&mut self, year: u16, month: u8, day: u8) {
        let previous_date = self.city.saved_date();
        let year_rolled = previous_date.is_some_and(|date| {
            date.month == 12 && month == 1 && year == date.year.saturating_add(1)
        });
        if year_rolled {
            self.apply_budget_year_end_settlement();
            self.clear_budget_year_history();
            // Mac CODE 4 export 227 turns Auto-Budget off after an annual
            // settlement that leaves city funds negative. The January review
            // gate below then opens the budget window, matching the native
            // option's negative-funds fallback without applying PE funding.
            if self.engine_fidelity == EngineFidelity::Mac68k
                && mac_auto_budget_enabled_for_city(&self.city)
                && self.city.city_funds() < 0
            {
                if set_mac_auto_budget_enabled(&mut self.city, false) {
                    self.sync_edit_map_menu();
                }
            }
            // Both native Mac and Win95 paths gate the January budget window
            // on their own edition-specific Auto-Budget setting.
            if !auto_budget_enabled_for_engine(&self.city, self.engine_fidelity) {
                self.open_january_budget_review();
            }
            self.sim_calendar_yearly_updates();
        }
        if self.engine_fidelity.uses_win95() {
            self.refresh_monthly_budget_rows(population_stats_for_city_with_fidelity(
                &self.city,
                self.engine_fidelity,
            ));
        }
        self.finish_sim_calendar_day0_publication(year, month, day);
    }

    pub(super) fn sim_calendar_yearly_updates(&mut self) {
        // This is the Win95 January/yearly publisher. Mac CODE 14 phase 0 runs
        // its own monthly budget work and annual January XMIC sweep; it has no
        // rooted PE annual structure, attitude-cache, or population-microsim
        // pass. The shared
        // batched calendar can call this method for either fidelity, so keep
        // those PE-only cache writes out of Mac January.
        if !self.engine_fidelity.uses_win95() {
            return;
        }
        // City-wide system microsims refresh aggregate counts yearly;
        // structure microsims get occupancy / aging from InitializeMicrosim-style rules.
        self.city.refresh_system_microsim_counts();
        let population = population_stats_for_city_with_fidelity(&self.city, self.engine_fidelity);
        // Refresh PE `0x4CC4B8` coverage before year-end structure microsims.
        // Same FUN_004679B0 process CRT as day-20 water used% / summary.
        let water_entry_seed = self.pe_process_crt.seed();
        let water = water_service_summary_with_entry_seed(&self.city, water_entry_seed);
        // Mac 68k has no recovered `0x4CC4B8` water-coverage January writer.
        if self.engine_fidelity.uses_win95() {
            let water_percent = pe_water_worker_utilization_percent(&self.city, water_entry_seed)
                .unwrap_or_else(|| service_percent(water.served, water.demand));
            self.city.set_pe_water_coverage_percent(water_percent);
            publish_pe_sewer_bonus(&mut self.city);
        }
        // PE `0x4CA5F8` unemployment feeds the January opinion poll weight vector.
        // Day-21 `FUN_00413E50` already published the growth-aggregate ratio into
        // `pe_unemployment_percent`; leave that PE global alone here.
        // PE attitude score 0 (`0x413f89`): traffic / (R+C+I tax bases + 1).
        // Map-less traffic heuristic shares process-CRT plant maps with attitude inputs.
        // Mac 68k has no recovered `0x4CC6F4` / dwCityTrafficCount January writer.
        let power_maps = power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        if self.engine_fidelity.uses_win95() {
            self.city
                .set_pe_traffic_count(live_traffic_count_for_city_with_power(
                    &self.city,
                    population,
                    &power_maps,
                ));
        }
        self.city
            .set_pe_attitude_tax_bases(budget_tax_bases_for_city_with_fidelity(
                &self.city,
                population,
                self.engine_fidelity,
            ));
        // Scores 1..3: XPLT / XVAL / XCRM totals ÷ developed-tile divisor (`0x4c93b4`).
        let police_funding_raw = budget_service_funding_raw_for_city(&self.city, 0);
        let fire_funding_raw = budget_service_funding_raw_for_city(&self.city, 1);
        let health_funding_raw = budget_service_funding_raw_for_city(&self.city, 2);
        let school_funding_raw = budget_service_funding_raw_for_city(&self.city, 3);
        let college_funding_raw = budget_service_funding_raw_for_city(&self.city, 4);
        let (_counted, pollution, land_value, crime) =
            pe_attitude_demand_inputs_for_city_with_power(&self.city, population, &power_maps);
        // Preserve the count last published by the day-2 density worker.
        let developed = self.city.pe_w_city_developed_tiles;
        self.city
            .set_pe_attitude_demand_inputs(developed, pollution, land_value, crime);
        // Coverage LE for overlays. Fertile EQ_LE averages overwrite MISC `0x44`/`0x48`/`0x4c`.
        let outcomes = self.live_city_service_outcomes_with_process_crt();
        self.city.set_pe_life_expectancy(outcomes.life_expectancy);
        // PE `FUN_0044e390` reads `0x4caa74`, the day-22 zone snapshot, not
        // the live census. Schools/colleges then divide leftover-filled EQ_LE
        // buckets `[1]+[2]` / `[3]`.
        if self.engine_fidelity.uses_win95() {
            let mut microsim_crt = self.pe_process_crt;
            let mut microsim_game_rand = self.pe_process_game_rand;
            let mut microsim_lfsr = self.pe_process_lfsr;
            let arco_residents = self.city.refresh_structure_microsim_yearly_with_rngs(
                self.city.pe_dw_city_population(),
                police_funding_raw,
                fire_funding_raw,
                health_funding_raw,
                school_funding_raw,
                college_funding_raw,
                &mut microsim_crt,
                &mut microsim_game_rand,
                &mut microsim_lfsr,
            );
            // PE clears these dwords at `0x44f10e..0x44f118` after the annual
            // transit rows have consumed their year-to-date passenger totals.
            self.trip_bus_passengers = 0;
            self.trip_rail_passengers = 0;
            self.trip_subway_passengers = 0;
            self.city.reset_pe_transit_passenger_totals();
            self.persist_transit_passengers_to_city();
            self.pe_process_crt = microsim_crt;
            self.pe_process_game_rand = microsim_game_rand;
            self.pe_process_lfsr = microsim_lfsr;
            self.city.set_pe_dw_arcology_population(arco_residents);
            if pe_misc_mirror_storage_active(&self.city, self.engine_fidelity) {
                self.city
                    .set_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, arco_residents);
            } else if self.city.kind == CityFileKind::MacCity {
                // Mac CODE 4 stores its arcology resident total at the same
                // +0x1020 slot consumed by the PE loader and saver. Persist
                // this shared role without enabling unrelated PE MISC mirrors.
                self.city
                    .set_mac_state_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, arco_residents);
            }
            // PE `0x4b174f` effect ids queued by yearly structure (Launch wipe `0x211`/`0x212`,
            // plant-expiry explode `504`).
            for id in self.city.take_pe_pending_effect_sound_ids() {
                if let Some(sound) = PlacementSound::from_pe_effect_id(id) {
                    self.queue_disaster_extra_sounds(sound, 1);
                }
            }
            // OpenSC2K annual utilities: plant-expiry AutoGoto when Options Auto-Goto is on.
            let plant_gotos = self.city.take_pe_pending_auto_goto();
            if self.auto_goto_enabled {
                if let Some(&(x, y)) = plant_gotos.last() {
                    self.pending_disaster_auto_goto = Some((x, y));
                }
            }
            // PE `0x47b681`: the live phase-0 continuation has already returned
            // from Budget. Extra can suspend the remaining phase publication.
            // Per-record popup/RNG interleaving inside the yearly worker is
            // still a separate boundary; this producer currently batches it.
            self.maybe_open_pe_newspaper_extra_popup();
        }
    }

    pub(super) fn finish_sim_calendar_day0_publication(&mut self, year: u16, month: u8, day: u8) {
        // Native New City reset clears `ffffe4ae`; December's budget pass
        // sets it and January's pass consumes it. Use the saved-date rollover
        // as the selected-engine equivalent while preserving the overlapping
        // native MISC byte on Win-layout cities.
        let year_rolled = self.city.saved_date().is_some_and(|date| {
            date.month == 12 && month == 1 && year == date.year.saturating_add(1)
        });
        self.city
            .set_saved_date_and_day(CityDate { year, month }, day);
        if self.engine_fidelity.uses_win95() {
            persist_pe_year_end_misc(&mut self.city, month);
        }
        if self.engine_fidelity.uses_win95() {
            if month == 1 {
                sync_bond_budget_row_for_city(&mut self.city, 0);
            }
        } else {
            // CODE 4 export 227 settles annual cash at the January boundary,
            // then calls CODE 3 export 162 only inside that annual branch.
            // Its XMIC sweep therefore runs once per year, before this
            // month's budget snapshot. Monthly budget snapshots, current-row
            // refresh, and the automatic-ordinance tail remain outside it.
            if year_rolled {
                let police_funding = budget_service_funding_raw_for_city(&self.city, 0);
                let fire_funding = budget_service_funding_raw_for_city(&self.city, 1);
                let health_funding = budget_service_funding_raw_for_city(&self.city, 2);
                let school_funding = budget_service_funding_raw_for_city(&self.city, 3);
                let college_funding = budget_service_funding_raw_for_city(&self.city, 4);
                let tax_rates = [0, 1, 2].map(|row| mac_raw_tax_rate_for_city(&self.city, row));
                self.refresh_mac_xmic_with_arcologies(
                    [
                        police_funding,
                        fire_funding,
                        health_funding,
                        school_funding,
                        college_funding,
                    ],
                    tax_rates,
                );
            }

            // The monthly snapshot precedes the derived-current rebuild, so
            // those refreshed values apply to the following month's history.
            mac_budget_month_snapshot_and_accrue(&mut self.city, pe_budget_month_index(month));
            mac_budget_refresh_derived_currents(&mut self.city);
            let row_bases = [0, 1, 2].map(|row_index| {
                budget_row_misc_offset_for_city(&self.city, row_index, BUDGET_SERVICE_CURRENT_FIELD)
                    .and_then(|offset| self.city.budget_misc_u32_at(offset))
                    .unwrap_or(0) as i32
            });
            let ordinance_current =
                mac_ordinance_raw_cash_total(&self.city, row_bases[0], row_bases[1], row_bases[2]);
            if let Some(offset) = budget_row_misc_offset_for_city(
                &self.city,
                BUDGET_ORDINANCE_ROW,
                BUDGET_SERVICE_CURRENT_FIELD,
            ) {
                self.city
                    .set_budget_misc_u32_values(offset, &[ordinance_current as u32]);
            }

            // CODE 4 export 227's automatic ordinance branch runs monthly,
            // after the XMIC call when January's annual branch ran, and after
            // monthly budget/current-row work on other months. It consumes
            // the same process QuickDraw stream and mutates the saved Mac
            // ordinance mask. Its story callback is presentation work and is
            // intentionally not sent through the PE newspaper queue.
            let _ = apply_mac_budget_ordinance_tail(
                &mut self.city,
                self.no_disasters_enabled,
                &mut self.mac_process_rng,
            );
            // These deterministic workers consume no QuickDraw draws, so they
            // do not guess the shared Mac RNG chronology of other XMIC cases.
            // CODE 14 phase 0 clears these scan accumulators after export 227
            // and export 162 have consumed them.
            mac_budget_clear_monthly_rci_scan_counters(&mut self.city);
        }
    }

    pub(super) fn open_january_budget_review(&mut self) {
        self.open_city_info_dialog(CityInfoDialog::Budget);
        // PE is a transient DialogBox pause; Mac retains its independently
        // modeled budget behavior until its callback contract is recovered.
        if !self.engine_fidelity.uses_win95() && self.traffic.speed() != GameSpeed::Paused {
            self.traffic.set_speed(GameSpeed::Paused);
            if let Some(value) =
                speed_misc_value_for_city(&self.city, self.engine_fidelity, GameSpeed::Paused)
            {
                persist_speed_misc_value_to_city(&mut self.city, value);
            }
            self.menu_checks.speed = GameSpeed::Paused;
            mac_menu::set_game_speed(GameSpeed::Paused);
        }
        self.set_sim_status_announcement("Budget: review January funding");
    }
}
