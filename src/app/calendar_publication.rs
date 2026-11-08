//! Calendar density, demand/history, weather, statistics and UI publication.
//!
//! Preserve edition-specific snapshot/weather timing, native random-stream order
//! and saved-field ownership. Native workers, shared models, calendar scheduling,
//! XTRF simulation and road rendering retain their existing owners.

use super::{
    CityServiceOutcomes, MISC_CITY_CRIME_OFFSET, MISC_CITY_POLLUTION_OFFSET,
    MISC_LAND_VALUE_OFFSET, MISC_NORMAL_POPULATION_OFFSET, MISC_POPULATED_TILE_COUNT_OFFSET,
    MISC_RESIDENTIAL_POPULATION_OFFSET, NEIGHBOR_CONNECTION_MISC_OFFSET, PopulationStats,
    ViewerState, advance_weather_state, advance_weather_state_win95,
    budget_service_funding_raw_for_city, density_map_microsim64_sum,
    mac_budget_refresh_rci_budget_currents, mac_consume_phase21_pre_weather_event_draws,
    mac_publish_monthly_growth_scalar, mac_publish_monthly_riot_ratio, mac_rci_nature_sample,
    mac_weather_state_for_city, neighbor_connection_tiles_for_city, pe_misc_mirror_storage_active,
    pe_rci_demand_updates_attitude_newspaper_46881f, pe_rci_demand_updates_eq_newspaper_468905,
    pe_rci_demand_updates_le_newspaper_468960, pe_rci_demand_updates_nature_sample,
    pe_rci_demand_updates_neighbor_fame_newspaper_4689c8,
    pe_rci_demand_updates_sports_newspaper_4687f3,
    pe_rci_demand_updates_unemployment_newspaper_4688a8, pe_rci_zone_pops_for_demand,
    pe_rebuild_w_city_developed_tiles_46ab56, persist_mac_weather_state_to_city,
    persist_pe_neighbor_connection_overlays, persist_weather_state_to_city,
    refresh_crime_eligibility_xbit_for_density_phase, weather_state_for_city,
};

impl ViewerState {
    pub(super) fn sim_calendar_day2_density(
        &mut self,
        population: PopulationStats,
        outcomes: CityServiceOutcomes,
    ) {
        let density_maps = self.refresh_live_density_maps(population, outcomes);
        // The native worker publishes this transient marker during its crime
        // stage. Rust derives the same mask directly while computing crime,
        // so mirror the XBIT side effect after the derived maps are complete.
        refresh_crime_eligibility_xbit_for_density_phase(&mut self.city, self.engine_fidelity);
        if self.engine_fidelity.uses_win95() {
            // The same day-2 worker clears and rebuilds the developed-tile WORD
            // at 0x46ab56 / 0x46ac82 before publishing the XVAL sum.
            pe_rebuild_w_city_developed_tiles_46ab56(&mut self.city);
            // FUN_0046A840 publishes these BSS sums during day 2. Keep the
            // runtime totals separate from the saved MISC mirror until save.
            self.city.pe_dw_city_land_value = density_map_microsim64_sum(&density_maps.land_value);
            self.city.pe_dw_city_pollution = density_map_microsim64_sum(&density_maps.pollution);
            self.city.pe_dw_city_crime = density_map_microsim64_sum(&density_maps.crime);
        } else {
            // CODE 14 phase 2 rebuilds `ffffbb50` from full-map XBLD/XZON.
            // Retain the snapshot independently when using a Win-layout city.
            let developed = super::graph_history::mac_xgrp_count_developed_cells(&self.city);
            self.city.set_mac_developed_cells(developed);
            // CODE 14 phase 2 also republishes the shared kind-2 actor
            // hotspot. Keep the native MISC words for Mac files and the
            // Mac-only runtime adapter for Win-layout files.
            let (hotspot_x, hotspot_y) = super::density::mac_developed_tile_anchor(&self.city);
            self.city
                .set_mac_kind2_hotspot(hotspot_x as u16, hotspot_y as u16);
            // Mac CODE 14 phase 2 accumulates native 64×64 XVAL, XPLT and
            // XCRM outputs into `ffffeffa`, `fffff006` and `ffffeffe`. The
            // in-memory density planes are expanded to 128×128, so collapse
            // each 2×2 footprint once when publishing those native totals.
            self.city.set_mac_state_misc_u32_at(
                MISC_LAND_VALUE_OFFSET,
                density_map_microsim64_sum(&density_maps.land_value),
            );
            self.city.set_mac_state_misc_u32_at(
                MISC_CITY_POLLUTION_OFFSET,
                density_map_microsim64_sum(&density_maps.pollution),
            );
            self.city.set_mac_state_misc_u32_at(
                MISC_CITY_CRIME_OFFSET,
                density_map_microsim64_sum(&density_maps.crime),
            );
        }
    }

    pub(super) fn sim_calendar_day21_demand_and_graphs(
        &mut self,
    ) -> (PopulationStats, CityServiceOutcomes) {
        // Win95 day 21: `SimulationRCIDemandUpdates`, then EQ_LE (`0x401262`)
        // which starts with national pop/value/tax/trend, then neighbor
        // pop/value, then industry mix, then age tables.
        // `UpdateGraphData` (`0x413e50`) appends the month's XGRP samples last.
        // `0x4caa74` is last month's day-22 zone snapshot: 0 on a new city so
        // the first day 21 wipes/skips instead of averaging empty EQ_LE to 0.
        // Keep Win95's existing phase-entry weather roll before its snapshot.
        // Mac CODE 8 export 237 updates weather near its tail, after publishing
        // demand and processing earlier work, so defer only the Mac transition
        // until after this month's pre-weather viewer/service snapshot.
        if !self.engine_fidelity.uses_win95() {
            // Export 237 first publishes the weighted scalar, then the budget
            // rows. The controlled Flint scan and phase-21 demand worker now
            // match native, but live demand remains gated until the natural
            // CODE 2/6/8/14 actor/event chronology and cohort transition join.
            mac_publish_monthly_growth_scalar(&mut self.city);
            mac_budget_refresh_rci_budget_currents(&mut self.city);
            // CODE 8 export 237's first two QuickDraw draws select a map tile;
            // its rooted radioactivity/tree gates and PlaceTile effects are
            // replayed. Run this proven map slice on either city container.
            mac_rci_nature_sample(&mut self.city, || self.mac_process_rng.random_i16() as u16);
            // The intervening story-queue writes are presentation state, but
            // their QuickDraw gates precede the native weather choice.
            // Preserve that stream order on either city container. For date
            // slots 0..=11, export 240 adds matching native command options
            // and CityGame's date gate exposes the individual tools. Slots
            // 12..=15 only increment an event/status count in that routine;
            // their gameplay effects and arcology relation remain unresolved.
            // The separate command-strip latch and event/status actions also
            // remain unmodeled.
            mac_consume_phase21_pre_weather_event_draws(
                &mut self.city,
                self.game_clock.year(),
                || self.mac_process_rng.random_i16() as u16,
            );
        }
        if self.engine_fidelity.uses_win95() {
            self.advance_simulation_weather();
        }
        let (population, outcomes) = self.refresh_live_misc_current_values();
        if !self.engine_fidelity.uses_win95() {
            self.advance_simulation_weather();
            // Mac CODE 14 runs export 237's late weather transition before
            // export 238's deterministic national-population/value drift.
            // Export 238's first national-event draw is later still (0x392fe),
            // so omitting that event does not affect this month's weather draw.
            // The local native prefix (map sample, alert/history gates, and
            // technology timer) has now been replayed in order. The weather
            // result is still not session-aligned because the process stream
            // entering phase 21 and earlier actor/event/render interleave are
            // unresolved; do not replace that chronology gap with a skip.
            self.city.advance_mac_national_metric_drift_month();
            // The later industry-demand WORD update uses the separate Mac
            // app-LFSR, whose reconstructed zero state remains a fixed point.
            // Its subsequent industry-population balancing is still gated.
            self.city.advance_mac_industry_demand_words_month();
            // The zero-scalar arm skips the later cohort transition and does
            // not publish new education/life/workforce summaries.
            self.city.mac_clear_cohorts_if_zero_growth();
            // The last phase-21 helper publishes the saved riot-gate ratio
            // after export 237's counter total and export 238's work.
            let ratio = mac_publish_monthly_riot_ratio(&mut self.city);
            // CODE 14's later XGRP helper publishes each verified row group
            // from its engine-owned phase-21 values; the shared roller keeps
            // each group's monthly, half-year, and five-year samples aligned.
            super::graph_history::mac_publish_xgrp_population(&mut self.city);
            super::graph_history::mac_publish_xgrp_environment(&mut self.city);
            super::graph_history::mac_publish_xgrp_services(&mut self.city);
            super::graph_history::mac_publish_xgrp_cohort_metrics(&mut self.city);
            if let Some(ratio) = ratio {
                super::graph_history::mac_publish_xgrp_riot_ratio(&mut self.city, ratio);
            }
            super::graph_history::mac_publish_xgrp_national(&mut self.city);
        }
        if self.engine_fidelity.uses_win95() {
            self.refresh_live_rci_demand_values(population, outcomes);
        }
        // PE SimulationRCIDemandUpdates @ 0x468626: one-tile nature sample (radioactivity
        // clear / tree age-up). The Mac CODE 8 counterpart already runs in the
        // Mac branch above; its worker is replayed, while full phase-21
        // QuickDraw chronology remains unaligned.
        if self.engine_fidelity.uses_win95() {
            pe_rci_demand_updates_nature_sample(&mut self.city, &mut self.pe_process_crt);
            // PE `FUN_00479a70` ages/reorders existing stories, then
            // `FUN_0047b5c0(1, 0)` inserts the monthly baseline story before
            // the conditional RCI/newspaper stories below.
            self.city.age_pe_newspaper_story_queue();
            self.city.queue_pe_newspaper_story(1, 0);
            // PE FUN_00468150 @ `0x4687f3`: stadium sports-team newspaper story.
            pe_rci_demand_updates_sports_newspaper_4687f3(&mut self.city, &mut self.pe_process_crt);
            // PE FUN_00468150 @ `0x46881f`: attitude traffic/pollution/crime stories.
            pe_rci_demand_updates_attitude_newspaper_46881f(
                &mut self.city,
                &mut self.pe_process_crt,
            );
            // PE FUN_00468150 @ `0x4688a8`: unemployment newspaper stories.
            pe_rci_demand_updates_unemployment_newspaper_4688a8(
                &mut self.city,
                &mut self.pe_process_crt,
            );
            // PE FUN_00468150 @ `0x468905`: EQ education newspaper stories.
            pe_rci_demand_updates_eq_newspaper_468905(&mut self.city, &mut self.pe_process_crt);
            // PE FUN_00468150 @ `0x468960`: LE life-expectancy newspaper stories.
            pe_rci_demand_updates_le_newspaper_468960(&mut self.city, &mut self.pe_process_crt);
            // PE FUN_00468150 @ `0x4689c8`: neighbor-fame newspaper stories.
            pe_rci_demand_updates_neighbor_fame_newspaper_4689c8(
                &mut self.city,
                &mut self.pe_process_crt,
            );
            // PE Extra auto-open after FUN_00468150 story inserts (`0x47b681`).
            self.maybe_open_pe_newspaper_extra_popup();
        }
        let outcomes = if self.engine_fidelity.uses_win95() {
            self.city.set_pe_life_expectancy(outcomes.life_expectancy);
            // PE EQ_LE continues the shared process CRT stream after the weather roll;
            // industry-mix Game_Random shares pe_process_lfsr. Mac phase 21 has its own
            // CODE 8 RCI publisher and history helper; its SANE/RNG path is not this one.
            let mut rng = self.pe_process_crt;
            let mut lfsr = self.pe_process_lfsr;
            let (zone_r, zone_c, zone_i) = pe_rci_zone_pops_for_demand(&self.city, population);
            let industrial_zone_pop = u32::try_from(zone_i.max(0)).unwrap_or(0);
            let rci_zone_people = u32::try_from(
                zone_r
                    .saturating_add(zone_c)
                    .saturating_add(zone_i)
                    .saturating_mul(10)
                    .max(0),
            )
            .unwrap_or(0);
            // PE industry-mix construction overflow compares against `dwCityPopulation`
            // (`0x4CAA74`); EQ_LE itself already reads pe_dw inside the calendar step.
            let city_population = if self.city.pe_zone_pops_ready() {
                self.city.pe_dw_city_population()
            } else {
                population.normal_population
            };
            // PE `FUN_00468d30` reads these three active-document funding dwords
            // directly when it builds the EQ/LE age and hospital birth caps.
            self.city.advance_pe_eq_le_calendar_month(
                city_population,
                budget_service_funding_raw_for_city(&self.city, 3),
                budget_service_funding_raw_for_city(&self.city, 2),
                budget_service_funding_raw_for_city(&self.city, 4),
                industrial_zone_pop,
                rci_zone_people,
                &mut rng,
                &mut lfsr,
            );
            self.pe_process_crt = rng;
            self.pe_process_lfsr = lfsr;
            if self.city.pe_eq_averages_ready() {
                self.live_city_service_outcomes_with_process_crt()
            } else {
                outcomes
            }
        } else {
            outcomes
        };
        if self.engine_fidelity.uses_win95() {
            // Mac CODE 14 phase 0 owns the 16-row current/funding snapshot and
            // signed raw accumulator. The PE day-21 refresh would reinterpret
            // Mac's overlapping MISC rows and overwrite those accumulators.
            // Mac rows are rebuilt by their own CODE 4 producer after its
            // phase-0 snapshot/accrual in the month-boundary path.
            self.refresh_monthly_budget_inputs(population);
        }
        if self.engine_fidelity.uses_win95() {
            self.refresh_xgrp_history(population, outcomes);
        }
        (population, outcomes)
    }

    pub(super) fn advance_simulation_weather(&mut self) {
        let next = if self.engine_fidelity.uses_win95() {
            let current = weather_state_for_city(&self.city, self.game_clock.month());
            advance_weather_state_win95(current, &mut self.pe_process_crt)
        } else {
            let current = mac_weather_state_for_city(&self.city, self.game_clock.month());
            advance_weather_state(current, &mut self.mac_process_rng)
        };
        if self.engine_fidelity.uses_win95() {
            persist_weather_state_to_city(&mut self.city, next);
        } else {
            persist_mac_weather_state_to_city(&mut self.city, next);
        }
    }

    pub(super) fn sim_calendar_day23_stats_dialogs(&mut self) {
        // Win95 UpdatePopulationDialog / UpdateIndustryDialog / UpdateGraphDialog
        // (`0x401087` view invalidates 0x80 / 0xa0 / 0x98). UI-only; XGRP samples
        // were appended on day 21 by `UpdateGraphData`.
        // Re-snapshot census after day 22. Industry demand/pop were written on day 21.
        // When day-21 publish owns `dwCityPopulation` / MISC `0x102c`, do not clobber
        // with census `normal_population` (same ownership rule as day-22 leftover).
        let population = self.live_population_stats_for_fidelity();
        let pe_mirrors_active = pe_misc_mirror_storage_active(&self.city, self.engine_fidelity);
        if pe_mirrors_active {
            self.city.set_misc_u32_at(
                MISC_RESIDENTIAL_POPULATION_OFFSET,
                population.residential_population,
            );
        }
        if pe_mirrors_active {
            self.city
                .set_misc_u32_at(MISC_POPULATED_TILE_COUNT_OFFSET, population.populated_tiles);
        }
        if pe_mirrors_active && !self.city.pe_zone_pops_ready() {
            self.city
                .set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, population.normal_population);
        }
        // PE `UpdatePopulationDialog` only invalidates the view here. The
        // `0x4cb460`/`0x4cb410`/`0x4cc4b4` fields remain the New City, loaded,
        // or EQ/LE-worker values; census and service estimates do not write them.
        self.city.mark_all_dirty();
    }

    pub(super) fn sim_calendar_day24_ui_chrome(&mut self) -> u32 {
        if !self.engine_fidelity.uses_win95() {
            // The native Mac phase-24 CODE 7 proposal worker is separate and
            // still unported. Do not publish PE reward gates or neighbor overlays.
            return 0;
        }
        // Win95 UpdateCityToolBar / UpdateCityMap / UpdateSimNationDialog /
        // UpdateWeatherOrDisasterState consumes the selected disaster state;
        // PE weather disasters are proposed by `0x471bc0` in simulation.
        let newly_available = self.refresh_reward_availability();
        if pe_misc_mirror_storage_active(&self.city, self.engine_fidelity) {
            self.city.set_misc_u32_at(
                NEIGHBOR_CONNECTION_MISC_OFFSET,
                neighbor_connection_tiles_for_city(&self.city),
            );
        }
        persist_pe_neighbor_connection_overlays(&mut self.city);
        // PE FUN_00471bc0 performs the weather proposal in simulation. Mac
        // export_231_CODE_7 at 0x3563a is its own phase-24 producer and is not
        // dispatched by this UI-chrome adapter. Do not perform another roll here.
        self.city.mark_all_dirty();
        newly_available
    }
}
