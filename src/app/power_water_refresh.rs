//! Power/water refresh, saved service bits and summary-publication adapters.
//!
//! Preserve edition-specific XBIT/state ownership, random-stream use and short
//! MISC guards. Calendar scheduling, native allocation workers and shared service
//! models retain their existing owners; unused adapters stay unscheduled.

use super::{
    GRAPH_EXTRA_HISTORY_MISC_OFFSET, MISC_POWER_USED_PERCENT_OFFSET, MISC_SPEED_OFFSET,
    MISC_WATER_USED_PERCENT_OFFSET, NEIGHBOR_CONNECTION_MISC_OFFSET, ViewerState, WaterServiceMaps,
    pe_attitude_tile_is_developed, pe_fun_00464550, pe_fun_00467860,
    power_used_percent_for_city_with_process_crt, power_water, service_utilization_percent,
    water_pipe_tile_connects_to, water_service_maps_with_entry_seed_and_fidelity,
    water_supply_capacity_tiles_with_entry_seed,
};
#[cfg(test)]
use super::{saved_base_pipe_bit_can_persist, zone_pipe_service_mask};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City, CityFileKind, TileBits};
use crate::engine_fidelity::EngineFidelity;

impl ViewerState {
    /// Win95 post-load helper 0x434b70 rebuilds power before water, then
    /// marks the half-resolution XBIT cells used by the derived city scan.
    pub(super) fn rebuild_services_after_city_open(&mut self) {
        if self.engine_fidelity.uses_win95() {
            self.city.reset_pe_transit_passenger_totals();
            pe_fun_00467860(&mut self.city, &mut self.pe_process_crt);
            pe_fun_00464550(&mut self.city);
            for x in 0..CITY_WIDTH {
                for y in 0..CITY_HEIGHT {
                    if pe_attitude_tile_is_developed(&self.city, x, y)
                        && let Some(bits) = self.city.tile_bits_at(x / 2, y / 2)
                        && bits.raw_bits() & 0x08 == 0
                    {
                        self.city.set_tile_bits(
                            x / 2,
                            y / 2,
                            TileBits::new(bits.raw_bits() | 0x08),
                        );
                    }
                }
            }
        }
        // Mac loads its saved XBIT and summary longwords for first use.
        self.load_transit_passengers_from_city();
    }

    #[cfg(test)]
    pub(super) fn refresh_water_pipe_bits(&mut self) {
        let maps = water_service_maps_with_entry_seed_and_fidelity(
            &self.city,
            self.pe_process_crt.seed(),
            self.engine_fidelity,
        );
        if !self.engine_fidelity.uses_win95() {
            self.refresh_mac_watered_bits(&maps);
            return;
        }
        let zone_piped = zone_pipe_service_mask(&self.city);

        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let index = y * CITY_WIDTH + x;
                let current_bits = self.city.tile_bits_at(x, y);
                let keep_saved_building_pipe = current_bits.is_some_and(|bits| bits.piped())
                    && saved_base_pipe_bit_can_persist(&self.city, x, y);
                let piped = maps.pipe_tiles[index] || keep_saved_building_pipe || zone_piped[index];
                let bits = current_bits
                    .unwrap_or_default()
                    .with_piped(piped)
                    .with_watered(piped && maps.watered_tiles[index]);
                if current_bits != Some(bits) && (current_bits.is_some() || bits.raw_bits() != 0) {
                    self.city.set_tile_bits(x, y, bits);
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn refresh_mac_watered_bits(&mut self, maps: &WaterServiceMaps) {
        apply_mac_watered_bits(&mut self.city, maps);
    }

    /// Publish all state written by the original Mac water pass together.
    pub(super) fn publish_mac_water_result(&mut self, maps: &WaterServiceMaps) {
        publish_mac_water_result_for_city(&mut self.city, maps);
    }

    #[cfg(test)]
    pub(super) fn refresh_simulation_service_bits(&mut self) {
        // Water bits have a verified refresh path. PE power XBIT `0x40` is known to
        // be network membership, but the current Rust power graph still differs from
        // PE's XBIT-`0x80` flood, so do not write its result back to saved XBIT yet.
        self.refresh_water_pipe_bits();
        self.city.mark_all_dirty();
    }

    pub(super) fn sim_calendar_day1_power(&mut self) {
        // PE calendar day 1 calls `FUN_00467860` directly (`0x4136d2`): rebuild
        // the saved XBIT allocation and publish `dwPowerUsedPercentage`. The
        // Mac CODE 14 phase 1 is the monthly `export_218` pass. Keep its
        // source units, persisted summary, and QuickDraw stream separate from PE.
        if self.engine_fidelity.uses_win95() {
            pe_fun_00467860(&mut self.city, &mut self.pe_process_crt);
            if self.can_persist_viewer_service_misc() {
                self.city.set_misc_u32_at(
                    MISC_POWER_USED_PERCENT_OFFSET,
                    u32::from(self.city.pe_power_used_percent()),
                );
            }
        } else {
            // Mac CODE 14 phase 1 (`export_218`) rebuilds the same transient
            // XBIT bits with Mac source units and the shared QuickDraw stream.
            // Its city summary occupies MISC dword 1048 in Mac-format saves.
            if let Some(percent) = power_water::mac_export_218_power_with_rng(
                &mut self.city,
                &mut self.mac_process_rng,
            ) {
                self.city.set_mac_power_service_summary(percent);
            } else {
                log::warn!(
                    "Mac CODE 14 power worker hit its native divide-by-zero boundary; pass was not published"
                );
            }
        }
        if !self.query_is_inspect {
            self.refresh_status_line();
        }
        self.city.mark_all_dirty();
    }

    /// CODE 5 @ 0x2ef8 calls the same Mac worker after a successful
    /// wire tool return below the weighted growth threshold.
    pub(super) fn mac_power_tool_post_place_recalc(&mut self) {
        if self.city.mac_state_misc_u32_at(0x102c).unwrap_or(0) < 50_000 {
            self.sim_calendar_day1_power();
        }
    }

    pub(super) fn sim_calendar_day20_water(&mut self) {
        // Both native phase-20 callers publish map bits and counters from one pass.
        if self.engine_fidelity.uses_win95() {
            let maps = pe_fun_00464550(&mut self.city);
            if self.can_persist_viewer_service_misc() {
                let capacity = water_supply_capacity_tiles_with_entry_seed(
                    &self.city,
                    self.city.misc_water_table_level().unwrap_or(0),
                    self.city.misc_humidity().unwrap_or(0),
                    self.pe_process_crt.seed(),
                );
                let used =
                    service_utilization_percent(maps.watered_consumers(&self.city), capacity);
                self.city
                    .set_misc_u32_at(MISC_WATER_USED_PERCENT_OFFSET, used);
            }
        } else {
            // export_209 publishes the map and summary from one component
            // walk. A newly filled tower contributes carry only next month.
            let maps = water_service_maps_with_entry_seed_and_fidelity(
                &self.city,
                self.pe_process_crt.seed(),
                EngineFidelity::Mac68k,
            );
            self.publish_mac_water_result(&maps);
        }
        if !self.query_is_inspect {
            self.refresh_status_line();
        }
    }

    /// Mac CODE 5 selector-4 pipe success tail calls CODE 14 export 209 when
    /// the saved weighted R/C/I growth scalar at MISC +0x102c is below 0xc350.
    /// This must use the Mac worker and its map publication, not PE's
    /// `dwCityPopulation` gate or `FUN_00464550` allocator.
    pub(super) fn mac_pipe_tool_post_place_water_recalc(&mut self) {
        if self.city.mac_state_misc_u32_at(0x102c).unwrap_or(0) >= 0xc350 {
            return;
        }
        let maps = water_service_maps_with_entry_seed_and_fidelity(
            &self.city,
            self.pe_process_crt.seed(),
            EngineFidelity::Mac68k,
        );
        self.publish_mac_water_result(&maps);
    }

    pub(super) fn can_persist_viewer_service_misc(&self) -> bool {
        if self.city.kind == CityFileKind::MacCity {
            // These offsets are PE/viewer extensions. Mac CODE 4 zero-fills
            // through them, so MISC span alone does not grant PE ownership.
            return false;
        }
        // Only write once MISC already spans graph-extra history so we do not
        // zero-fill budget funding fields by extending a short blank MISC.
        self.city
            .misc_u32_at(GRAPH_EXTRA_HISTORY_MISC_OFFSET)
            .is_some()
            || self.city.misc_u32_at(MISC_SPEED_OFFSET).is_some()
            || self
                .city
                .misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET)
                .is_some()
    }

    fn persist_power_used_percent(&mut self) {
        if !self.engine_fidelity.uses_win95() || !self.can_persist_viewer_service_misc() {
            return;
        }
        let percent =
            power_used_percent_for_city_with_process_crt(&self.city, &mut self.pe_process_crt);
        self.city.set_pe_power_used_percent(percent);
        self.city
            .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, percent);
    }

    fn water_pipe_connects_to(&self, x: usize, y: usize, side: u8) -> bool {
        water_pipe_tile_connects_to(&self.city, x, y, side)
    }
}

/// Apply the water result to a City while the XMIC slot dispatcher owns it.
pub(super) fn publish_mac_water_result_for_city(city: &mut City, maps: &WaterServiceMaps) {
    apply_mac_watered_bits(city, maps);
    if maps.mac_worker_contract_complete {
        let treatment_tiles = city.mac_xbld_tile_count(0xf4);
        city.set_mac_treatment_gate(power_water::mac_water_treatment_gate(
            maps.mac_worker_served_cells,
            treatment_tiles,
        ));
    }
    if let Some(water_percent) = maps.mac_worker_utilization_percent() {
        city.set_mac_water_service_summary(water_percent);
    }
    city.mark_all_dirty();
}

fn apply_mac_watered_bits(city: &mut City, maps: &WaterServiceMaps) {
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = y * CITY_WIDTH + x;
            let Some(current_bits) = city.tile_bits_at(x, y) else {
                continue;
            };
            let updated = TileBits::new(current_bits.raw_bits() & !0x08)
                .with_watered(maps.watered_tiles[index]);
            if current_bits != updated {
                city.set_tile_bits(x, y, updated);
            }
        }
    }
}
