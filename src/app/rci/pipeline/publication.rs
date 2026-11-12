use super::*;

impl ViewerState {
    pub(in crate::app) fn refresh_live_rci_demand_values(
        &mut self,
        population: PopulationStats,
        outcomes: CityServiceOutcomes,
    ) {
        // The first three saved words at MISC +0x718 are prior R/C/I demand
        // in both layouts (Mac CODE 4 slots 454–456 and Win95 demand dwords).
        // Mac also saves five more words after the triple; leave those bytes
        // untouched because their native roles are not all identified.
        for (index, value) in rci_demand_values_for_city(&self.city, population, outcomes)
            .into_iter()
            .enumerate()
        {
            self.city
                .set_misc_u32_at(RCI_DEMAND_MISC_OFFSET + index * 4, value as u32);
        }
        commit_pe_rci_previous_residential_zone_pop(&mut self.city, population);
    }

    pub(in crate::app) fn refresh_xgrp_history(
        &mut self,
        population: PopulationStats,
        outcomes: CityServiceOutcomes,
    ) {
        // PE UpdateGraphData @ `0x41354e`: publish month-mod/season/fame-base before XGRP.
        self.city
            .pe_publish_graph_calendar_words_from_city_age_41354e();
        // PE UpdateGraphData @ `0x4138ee` → tech-unlock rebuild. Mac MISC
        // +0xfa0 begins Mac's saved zone-7 selector census, not PE ordinance state.
        if self.engine_fidelity.uses_win95() {
            let _ = self
                .city
                .pe_rebuild_tech_unlock_mask_46a0be(ordinance_flags_for_city(&self.city));
        }
        if self.engine_fidelity.uses_win95() && self.zone_growth_enabled {
            // PE 0x414090..0x4140bf publishes after demand/news consumes
            // the previous value, then stores the same DWORD in XGRP row 12.
            let unemployment = self.pe_growth_aggregate.unemployment_ratio_percent();
            self.city.set_pe_unemployment_percent(unemployment);
            // This DWORD is shared by PE and Mac at MISC +0xfa4; publish it
            // for PE fidelity on either city container.
            self.city
                .set_misc_u32_at(MISC_UNEMPLOYMENT_OFFSET, unemployment);
        }
        if self.engine_fidelity.uses_win95() {
            // PE `UpdateGraphData` @ 0x413f89..0x414001 publishes these
            // scores every month, after the demand/newspaper worker has read
            // the previous scores. Read the budget object's raw current rows.
            let tax_bases = std::array::from_fn(|row| {
                budget_row_misc_offset_for_city(&self.city, row, BUDGET_SERVICE_CURRENT_FIELD)
                    .and_then(|offset| self.city.budget_misc_u32_at(offset))
                    .unwrap_or(0)
            });
            // +0x4c0/+0x530/+0x5a0 are document rows 10..12 (roads,
            // highways, bridges). The R/C/I rows above feed the EQ/LE cache.
            let transport_currents = [10, 11, 12].map(|row| {
                budget_row_misc_offset_for_city(&self.city, row, BUDGET_SERVICE_CURRENT_FIELD)
                    .and_then(|offset| self.city.budget_misc_u32_at(offset))
                    .unwrap_or(0)
            });
            let developed = self.city.pe_w_city_developed_tiles;
            let pollution = self.city.pe_dw_city_pollution;
            let land = self.city.pe_dw_city_land_value;
            let crime = self.city.pe_dw_city_crime;
            self.city.set_pe_attitude_tax_bases(tax_bases);
            self.city
                .set_pe_attitude_demand_inputs(developed, pollution, land, crime);
            self.city.publish_pe_attitude_scores(transport_currents);
        }
        let mut latest_values = xgrp_history_latest_values(&self.city, population, outcomes);
        if self.engine_fidelity.uses_win95() {
            if self.zone_growth_enabled && self.city.pe_zone_pops_ready() {
                let exact = crate::app::graph_history::pe_xgrp_population_values(
                    &self.city,
                    self.pe_growth_aggregate.slots()[0],
                );
                latest_values[..4].copy_from_slice(&exact);
            }
            // The four score pointers are the newest XGRP entries themselves.
            for index in 0..4 {
                latest_values[index + 4] = self.city.pe_attitude_score_at(index);
            }
        }
        let mut values = self.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
        values.resize(XGRP_VALUES, 0);
        roll_xgrp_history_section(
            &mut values,
            XGRP_MONTH_OFFSET,
            XGRP_MONTH_SAMPLES,
            &latest_values,
        );
        if pe_xgrp_rolls_half_year(&self.city) {
            roll_xgrp_history_section(
                &mut values,
                XGRP_HALF_YEAR_OFFSET,
                XGRP_HALF_YEAR_SAMPLES,
                &latest_values,
            );
        }
        if pe_xgrp_rolls_five_year(&self.city) {
            roll_xgrp_history_section(
                &mut values,
                XGRP_FIVE_YEAR_OFFSET,
                XGRP_FIVE_YEAR_SAMPLES,
                &latest_values,
            );
        }
        self.city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &values);
    }

    pub(in crate::app) fn refresh_reward_availability(&mut self) -> u32 {
        // Reward bits are mutated by the rooted engine transitions: progression
        // milestones, successful civic placement, demolition restoration, and
        // the PE technology gate. Generic edits and periodic service refreshes
        // do not grant them. Keep this adapter until its callers are removed or
        // gain a native-proven refresh effect.
        0
    }

    pub(in crate::app) fn refresh_live_trips(&mut self) {
        // Test/helper path: production Win95 uses growth-lattice trips (days 3–18)
        // and day-19 decay. Mac's production calendar uses its separate CODE 8/14
        // route scan, whose successful routes can write XTRF.
        if self.trip_served_tiles.len() != CITY_TILES {
            self.trip_served_tiles = vec![false; CITY_TILES];
        }
        // FUN_004679B0 plant capacity shares process CRT with growth-tick power maps.
        let power_maps = power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        // Both editions gate trips on the saved-XBIT powered cross.
        let stats = trip_generator::simulate_monthly_trips_with_xtrf(
            &mut self.city,
            &mut self.trip_served_tiles,
            |x, y| power_water::growth_powered_cross_for_engine(&power_maps, x, y),
            self.engine_fidelity.uses_win95(),
        );
        self.trip_bus_passengers = self.trip_bus_passengers.wrapping_add(stats.bus_passengers);
        self.trip_rail_passengers = self
            .trip_rail_passengers
            .wrapping_add(stats.rail_passengers);
        self.trip_subway_passengers = self
            .trip_subway_passengers
            .wrapping_add(stats.subway_passengers);
        self.pending_traffic_redraw = true;
        self.city.mark_all_dirty();
    }

    pub(in crate::app) fn tile_has_growth_trip_access(&self, x: usize, y: usize) -> bool {
        if self.bypass_growth_trip_gate {
            return true;
        }
        // PE FUN_004358B0 calls RunTripGenerator once, then skips growth on a
        // false result; do not retry here with the standalone local-RNG helper.
        let index = y * CITY_WIDTH + x;
        self.trip_served_tiles.get(index).copied().unwrap_or(false)
    }
}
