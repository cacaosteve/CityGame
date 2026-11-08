//! Live city-stat publication, density sync and service snapshot adapters.
//!
//! Preserve saved-field ownership/readiness gates, entry-seed reads and write
//! order. Native density/service/counter workers, the shared density-map model
//! and calendar/history scheduling retain their existing owners.

use super::{
    BUDGET_SERVICE_COUNT, BUDGET_SERVICE_CURRENT_FIELD, BUDGET_SERVICE_FUNDING_FIELD,
    BUDGET_SERVICE_MISC_STRIDE, BUDGET_SERVICE_ROW_START, BUDGET_SERVICE_YEAR_TO_DATE_FIELD,
    BUILDING_TILE_COUNT_VALUES, CityServiceCoverage, CityServiceOutcomes, DEFAULT_TAX_RATE,
    LiveDensityMaps, MILITARY_TILE_COUNT_VALUES, MISC_ARCOLOGY_POPULATION_OFFSET,
    MISC_BUILDING_TILE_COUNTS_OFFSET, MISC_CITY_CRIME_OFFSET, MISC_CITY_FAME_OFFSET,
    MISC_CITY_POLLUTION_OFFSET, MISC_CITY_TRAFFIC_OFFSET, MISC_CITY_VALUE_OFFSET,
    MISC_COMMERCIAL_TILE_COUNT_OFFSET, MISC_DIFFICULTY_OFFSET, MISC_GARBAGE_OFFSET,
    MISC_INDUSTRIAL_MIX_BONUS_OFFSET, MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET,
    MISC_INDUSTRIAL_TILE_COUNT_OFFSET, MISC_LAND_VALUE_OFFSET, MISC_MILITARY_TILE_COUNTS_OFFSET,
    MISC_NORMAL_POPULATION_OFFSET, MISC_OLD_ARRESTS_OFFSET, MISC_POPULATED_TILE_COUNT_OFFSET,
    MISC_PRISON_BONUS_OFFSET, MISC_RESIDENTIAL_POPULATION_OFFSET,
    MISC_RESIDENTIAL_TILE_COUNT_OFFSET, MISC_SEWER_BONUS_OFFSET, MISC_SPEED_OFFSET,
    MISC_SPORTS_TEAMS_OFFSET, MISC_SUBWAY_COUNT_OFFSET, MISC_UNEMPLOYMENT_OFFSET,
    MISC_WORK_FORCE_PERCENT_OFFSET, MISC_YEAR_END_OFFSET, NEIGHBOR_CONNECTION_MISC_OFFSET,
    PopulationStats, ViewerState, budget_row_is_unset, budget_service_funding_misc_offset_for_city,
    budget_service_misc_offset_for_city, budget_service_row_needs_live_defaults,
    budget_service_rows_for_city, city_difficulty_for_city,
    city_service_coverage_for_city_with_power, ensure_budget_misc_defaults_before_extension,
    land_value_total_from_density, live_city_fame_value_with_fidelity,
    live_city_scenario_counters_for_city_with_power,
    live_city_service_outcomes_for_city_with_power_and_fidelity,
    live_density_maps_for_city_with_fidelity, neighbor_connection_tiles_for_city,
    pe_misc_mirror_storage_active, persist_pe_disaster_misc, power_service_maps_with_entry_seed,
    sc2k_speed_misc_value, tax_rate_misc_offset_for_city,
};
use crate::city::{City, CityFileKind};

fn misc_offset_is_mac_owned(offset: usize) -> bool {
    super::MAC_NATIVE_MISC_STATE_COLLISIONS.contains(&offset)
        // CODE 4 serializes every state slot from 380 through 1067, covering
        // offsets +0x5f0..=+0x10ac. PE mirrors in that range are not safe.
        || (0x05f0..=0x10ac).contains(&offset)
}

/// Combined R/C/I census estimate, excluding arcologies.
pub(super) fn pe_zone_pops_scaled_population(population: PopulationStats) -> u32 {
    population
        .residential_population
        .saturating_add(population.commercial_population)
        .saturating_add(population.industrial_population)
}

pub(super) fn persist_pe_garbage_day22(city: &mut City, population: PopulationStats) {
    // PE `FUN_00468150` already wrote leftover / `dwCityPopulation` / garbage from
    // the folded growth aggregate on day 21. Do not re-derive those from census.
    if city.pe_zone_pops_ready() {
        return;
    }
    // Native slot 0 sums the six residential/commercial/industrial slots.
    // This live census remains an approximation of that weighted scan.
    let increment = pe_zone_pops_scaled_population(population);
    city.pe_w_city_garbage = city.pe_w_city_garbage.wrapping_add(increment);
    if city.kind != CityFileKind::MacCity {
        city.set_misc_u32_at(MISC_GARBAGE_OFFSET, city.pe_w_city_garbage);
    }
    // Offline / growth-disabled path: census stand-in for the same PE prologue.
    city.persist_pe_day22_population_leftover(increment);
}

/// Win95 `0x4735e6`: `bYearEndFlag = 1` when 0-indexed month is `0x0B` (December).
pub(super) fn pe_year_end_flag(month: u8) -> u32 {
    u32::from(month == 12)
}

pub(super) fn building_tile_counts_for_city(city: &City) -> [u32; BUILDING_TILE_COUNT_VALUES] {
    // MISC `0x01f0` is the saved `dwTileCount` WORD[256] (`0x4CA4C8`), not a live XBLD walk.
    let mut counts = [0_u32; BUILDING_TILE_COUNT_VALUES];
    for (index, &word) in city.pe_dw_tile_count.iter().enumerate() {
        if index >= BUILDING_TILE_COUNT_VALUES {
            break;
        }
        counts[index] = u32::from(word);
    }
    counts
}

pub(super) fn military_tile_counts_for_city(city: &City) -> [u32; MILITARY_TILE_COUNT_VALUES] {
    // MISC `0x0fa8` is the saved `0x4ca934` WORD[16] (PlaceTile military census),
    // not a live XBLD walk or sum of special runway ids.
    let mut counts = [0_u32; MILITARY_TILE_COUNT_VALUES];
    for (index, &word) in city.pe_military_tile_count.iter().enumerate() {
        counts[index] = u32::from(word);
    }
    counts
}

pub(super) fn persist_pe_year_end_misc(city: &mut City, month: u8) {
    if city.kind == CityFileKind::MacCity {
        return;
    }
    ensure_budget_misc_defaults_before_extension(city);
    city.set_misc_u32_at(MISC_YEAR_END_OFFSET, pe_year_end_flag(month));
}

impl ViewerState {
    /// Live LE/EQ/service coverage with FUN_004679B0 process-CRT plant maps.
    pub(super) fn live_city_service_outcomes_with_process_crt(&self) -> CityServiceOutcomes {
        let power_maps = power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        live_city_service_outcomes_for_city_with_power_and_fidelity(
            &self.city,
            &power_maps,
            self.engine_fidelity,
        )
    }

    /// City-info/budget police/fire coverage with the same process-CRT plant maps.
    pub(super) fn live_city_service_coverage_with_process_crt(&self) -> CityServiceCoverage {
        let power_maps = power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        city_service_coverage_for_city_with_power(&self.city, &power_maps)
    }

    pub(super) fn refresh_live_misc_stats(&mut self) {
        let (population, outcomes) = self.refresh_live_misc_current_values();
        self.refresh_xgrp_history(population, outcomes);
    }

    pub(super) fn refresh_live_density_maps(
        &mut self,
        population: PopulationStats,
        outcomes: CityServiceOutcomes,
    ) -> LiveDensityMaps {
        let density_maps = live_density_maps_for_city_with_fidelity(
            &self.city,
            population,
            outcomes,
            self.pe_process_crt.seed(),
            self.engine_fidelity.uses_win95(),
            self.engine_fidelity,
        );
        self.city
            .sync_land_value_from_slice(&density_maps.land_value);
        self.city.sync_crime_density_from_slice(&density_maps.crime);
        self.city
            .sync_pollution_density_from_slice(&density_maps.pollution);
        self.city
            .sync_police_coverage_from_slice(&density_maps.police);
        self.city.sync_fire_coverage_from_slice(&density_maps.fire);
        self.city
            .sync_population_density_from_slice(&density_maps.population_density);
        self.city
            .sync_rate_of_growth_from_slice(&density_maps.rate_of_growth);
        density_maps
    }

    pub(super) fn refresh_live_misc_current_values(
        &mut self,
    ) -> (PopulationStats, CityServiceOutcomes) {
        ensure_budget_misc_defaults_before_extension(&mut self.city);
        let missing_tax_rates = (0..3)
            .filter(|index| budget_row_is_unset(&self.city, *index))
            .collect::<Vec<_>>();
        let missing_budget_funding = (0..BUDGET_SERVICE_COUNT)
            .filter(|index| {
                budget_row_is_unset(&self.city, BUDGET_SERVICE_ROW_START.saturating_add(*index))
            })
            .collect::<Vec<_>>();
        let population = self.live_population_stats_for_fidelity();
        let mut outcomes = self.live_city_service_outcomes_with_process_crt();
        // Generic UI/tool refreshes may calculate a preview, but must not
        // run either edition's full density worker between phase-2 scans.
        let density_maps = live_density_maps_for_city_with_fidelity(
            &self.city,
            population,
            outcomes,
            self.pe_process_crt.seed(),
            self.engine_fidelity.uses_win95(),
            self.engine_fidelity,
        );
        outcomes.land_value = land_value_total_from_density(&density_maps.land_value);
        // Both editions recalculate city value in their rooted finance paths.
        // The relocated-call censuses found no recurring publisher, so keep
        // the loaded/current BSS value instead of turning the density-derived
        // display fallback into a simulation-state update.
        outcomes.city_value = self.city.pe_city_value.max(0) as u32;
        let power_maps = power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let scenario_counters = live_city_scenario_counters_for_city_with_power(
            &self.city,
            population,
            outcomes,
            &power_maps,
        );
        let city_fame = live_city_fame_value_with_fidelity(
            &self.city,
            population,
            outcomes,
            scenario_counters,
            self.engine_fidelity,
        );

        for (offset, value) in [
            (MISC_CITY_VALUE_OFFSET, outcomes.city_value),
            (MISC_LAND_VALUE_OFFSET, outcomes.land_value),
            (MISC_CITY_CRIME_OFFSET, scenario_counters.crime),
            (MISC_CITY_TRAFFIC_OFFSET, scenario_counters.traffic),
            (MISC_CITY_POLLUTION_OFFSET, scenario_counters.pollution),
            (MISC_CITY_FAME_OFFSET, city_fame),
            (
                MISC_WORK_FORCE_PERCENT_OFFSET,
                self.city.pe_eq_workforce_percent(),
            ),
            (
                MISC_RESIDENTIAL_POPULATION_OFFSET,
                population.residential_population,
            ),
            (MISC_POPULATED_TILE_COUNT_OFFSET, population.populated_tiles),
            (
                MISC_RESIDENTIAL_TILE_COUNT_OFFSET,
                population.residential_tiles,
            ),
            (
                MISC_COMMERCIAL_TILE_COUNT_OFFSET,
                population.commercial_tiles,
            ),
            (
                MISC_INDUSTRIAL_TILE_COUNT_OFFSET,
                population.industrial_tiles,
            ),
            (
                MISC_ARCOLOGY_POPULATION_OFFSET,
                population.arcology_population,
            ),
            (
                NEIGHBOR_CONNECTION_MISC_OFFSET,
                neighbor_connection_tiles_for_city(&self.city),
            ),
            (
                MISC_SUBWAY_COUNT_OFFSET,
                // PE MISC `0xfe8` mirrors BSS `0x4ca41c` (PlaceXUND occupancy), not a
                // full underground subway rescan (`0x431a40` / `0x4340b8`).
                u32::from(self.city.pe_xund_occupancy),
            ),
            (
                MISC_SPORTS_TEAMS_OFFSET,
                u32::from(self.city.pe_w_sports_teams()),
            ),
            (
                MISC_INDUSTRIAL_MIX_BONUS_OFFSET,
                u32::from(self.city.pe_w_industrial_mix_bonus()),
            ),
            (
                MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET,
                self.city.pe_w_industrial_mix_pollution_bonus() as u32,
            ),
            (
                MISC_SEWER_BONUS_OFFSET,
                u32::from(self.city.pe_w_sewer_bonus()),
            ),
            (
                MISC_OLD_ARRESTS_OFFSET,
                u32::from(self.city.pe_w_old_arrests()),
            ),
            (
                MISC_PRISON_BONUS_OFFSET,
                u32::from(self.city.pe_w_prison_bonus()),
            ),
            (
                MISC_UNEMPLOYMENT_OFFSET,
                u32::from(self.city.pe_unemployment_percent()),
            ),
            (
                MISC_SPEED_OFFSET,
                sc2k_speed_misc_value(self.traffic.speed()),
            ),
            (
                MISC_YEAR_END_OFFSET,
                pe_year_end_flag(self.game_clock.month()),
            ),
            (MISC_DIFFICULTY_OFFSET, city_difficulty_for_city(&self.city)),
        ] {
            // The Win95 density and traffic workers own these BSS totals on
            // days 2 and 19. Generic presentation/tool refreshes must not
            // publish another scan or overwrite the saved MISC mirrors early.
            if offset == MISC_LAND_VALUE_OFFSET
                || (self.engine_fidelity.uses_win95()
                    && matches!(
                        offset,
                        MISC_CITY_CRIME_OFFSET
                            | MISC_CITY_TRAFFIC_OFFSET
                            | MISC_CITY_POLLUTION_OFFSET
                    ))
            {
                continue;
            }
            if misc_offset_is_mac_owned(offset)
                && !pe_misc_mirror_storage_active(&self.city, self.engine_fidelity)
            {
                continue;
            }
            if offset == MISC_WORK_FORCE_PERCENT_OFFSET && !self.city.pe_eq_averages_ready() {
                continue;
            }
            self.city.set_misc_u32_at(offset, value);
        }
        // Keep BSS `0x4ca4d0` aligned with the MISC city-value dword so save
        // persist @ `0x432ee0` does not wipe the published value.
        self.city.pe_city_value = outcomes.city_value as i32;
        // Fame BSS `0x4cad28` / MISC `+0x38` (`0x433b26`).
        if self.engine_fidelity.uses_win95() && self.city.kind != CityFileKind::MacCity {
            self.city.pe_dw_city_fame = city_fame;
        }
        if !self.engine_fidelity.uses_win95() {
            // Preserve existing traffic compatibility behavior. Crime and
            // pollution caches must remain latched until phase 2; saving a
            // Win95-format city writes these shared caches back to MISC.
            self.city.set_pe_traffic_count(scenario_counters.traffic);
        }
        // Until the PE EQ/LE writer runs, keep the local service estimates in
        // `outcomes`. It does not publish the native MISC/BSS summary fields.
        if pe_misc_mirror_storage_active(&self.city, self.engine_fidelity)
            && !self.city.pe_zone_pops_ready()
        {
            self.city
                .set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, population.normal_population);
        }
        if self.engine_fidelity.uses_win95() {
            persist_pe_disaster_misc(&mut self.city, None, self.mode, self.engine_fidelity);
            self.maybe_open_pe_disaster_end_newspaper();
        }
        // The Mac state vector owns these same MISC ranges: its XBLD census
        // begins at +0x1f0, and its selected-XUND count at +0xfe8 shares the
        // PE XUND occupancy slot. Publish these PE arrays only for PE fidelity.
        if pe_misc_mirror_storage_active(&self.city, self.engine_fidelity) {
            self.city.set_misc_u32_values(
                MISC_BUILDING_TILE_COUNTS_OFFSET,
                &building_tile_counts_for_city(&self.city),
            );
            self.city.set_misc_u32_values(
                MISC_MILITARY_TILE_COUNTS_OFFSET,
                &military_tile_counts_for_city(&self.city),
            );
        }
        for index in missing_tax_rates {
            if let Some(offset) = tax_rate_misc_offset_for_city(&self.city, index) {
                self.city
                    .set_budget_misc_u32_values(offset, &[DEFAULT_TAX_RATE]);
            }
        }
        for index in missing_budget_funding {
            if let Some(offset) = budget_service_funding_misc_offset_for_city(&self.city, index) {
                self.city.set_budget_misc_u32_values(offset, &[100]);
            }
        }
        for (index, row) in budget_service_rows_for_city(&self.city)
            .into_iter()
            .enumerate()
        {
            if budget_service_row_needs_live_defaults(&self.city, index, row.current) {
                let mut values = [0_u32; BUDGET_SERVICE_MISC_STRIDE];
                values[BUDGET_SERVICE_CURRENT_FIELD] = row.current;
                values[BUDGET_SERVICE_FUNDING_FIELD] = row.funding;
                values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] = 0;
                for month in 0..12 {
                    values[3 + month * 2] = 0;
                    values[4 + month * 2] = row.funding;
                }
                if let Some(offset) = budget_service_misc_offset_for_city(&self.city, index, 0) {
                    self.city.set_budget_misc_u32_values(offset, &values);
                }
            }
        }
        self.persist_transit_passengers_to_city();
        (population, outcomes)
    }
}
