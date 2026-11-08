//! Monthly growth bookkeeping and transit passenger counters.
//!
//! Preserve aggregate publication and saved-field guards. The monthly trip-origin
//! gate and yearly passenger totals retain their distinct lifecycles. Shared
//! models, trip/growth workers, XTRF simulation and road rendering keep their
//! existing owners.

use super::{
    MISC_BUS_PASSENGERS_OFFSET, MISC_GARBAGE_OFFSET, MISC_RAIL_PASSENGERS_OFFSET,
    MISC_SUBWAY_PASSENGERS_OFFSET, ViewerState, pe_misc_mirror_storage_active,
};
use crate::city::CITY_TILES;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TransitPassengerStats {
    pub(super) bus: u32,
    pub(super) rail: u32,
    pub(super) subway: u32,
}

impl ViewerState {
    pub(super) fn transit_passenger_stats(&self) -> TransitPassengerStats {
        TransitPassengerStats {
            bus: self.trip_bus_passengers,
            rail: self.trip_rail_passengers,
            subway: self.trip_subway_passengers,
        }
    }

    pub(super) fn reset_monthly_growth_budget(&mut self) {
        self.pe_growth_aggregate.reset();
        self.reset_monthly_trip_gate();
    }

    pub(super) fn publish_pe_growth_aggregate(&mut self) {
        if !self.engine_fidelity.uses_win95() {
            // Mac phases 3–18 accumulate different scan counters for its native
            // phase-21 RCI publisher. Never publish the PE transient lattice as
            // Mac population, garbage, unemployment, or EQ growth state.
            return;
        }
        if !self.zone_growth_enabled {
            // Keep last published pZonePops when synthetic growth is off so
            // loaded cities are not wiped by an empty transient aggregate.
            return;
        }
        self.pe_growth_aggregate.fold_normal_slots();
        // PE stores the low DWORD after wrapping multiplication, including a
        // high-bit result; it does not clamp the signed interpretation.
        let scaled = self.pe_growth_aggregate.scaled_city_population() as u32;
        // PE `FUN_00468150` @ `0x46818a`/`0x4681e5`/`0x4681ea`: leftover, then
        // `dwCityPopulation = slot0*10`, then `wCityGarbage += slot0*10`.
        self.city.persist_pe_day22_population_leftover(scaled);
        // PE `0x4681c9`/`0x4681ea`: `wCityGarbage` BSS `0x4ca5f0` += `slot0*10`.
        self.city.pe_w_city_garbage = self.city.pe_w_city_garbage.wrapping_add(scaled);
        let pe_mirrors_active = pe_misc_mirror_storage_active(&self.city, self.engine_fidelity);
        if pe_mirrors_active {
            self.city
                .set_misc_u32_at(MISC_GARBAGE_OFFSET, self.city.pe_w_city_garbage);
        }
        // PE `FUN_00468150`: write paired slots into `pZonePops[0..2]`.
        self.city
            .set_pe_zone_pops(self.pe_growth_aggregate.zone_population_rows());
        // PE `FUN_00468D30` @ `0x469761` reads slot 7 with `0x4caa74` for EQ_LE
        // age-forward life penalty after this fold/publish.
        self.city
            .set_pe_eq_growth_slot7(self.pe_growth_aggregate.slots()[7]);
        // The unemployment ratio is not published until UpdateGraphData,
        // after day-21 demand and newspaper processing have consumed the old value.
    }

    fn reset_monthly_trip_gate(&mut self) {
        if self.trip_served_tiles.len() != CITY_TILES {
            self.trip_served_tiles = vec![false; CITY_TILES];
        } else {
            self.trip_served_tiles.fill(false);
        }
        // PE passenger dwords are yearly accumulators: growth trips add to
        // them across months, and FUN_0044E390 clears them after January reads.
        // Only the per-month successful-origin gate resets here.
    }

    pub(super) fn load_transit_passengers_from_city(&mut self) {
        self.trip_bus_passengers = self.city.pe_dw_bus_passengers();
        self.trip_rail_passengers = self.city.pe_dw_rail_passengers();
        self.trip_subway_passengers = self.city.pe_dw_subway_passengers();
    }

    pub(super) fn persist_transit_passengers_to_city(&mut self) {
        if !self.engine_fidelity.uses_win95() {
            // These BSS totals and MISC `+0x1120..+0x1128` snapshots belong to
            // the PE annual XMIC path. Mac CODE 4 zero-fills this tail; its
            // transit accumulators use separate native globals and MISC rows.
            return;
        }
        // PE trip epilogue writes BSS `0x4c85a0`/`0x4c85a4`/`0x4c8600` live.
        self.city.set_pe_dw_bus_passengers(self.trip_bus_passengers);
        self.city
            .set_pe_dw_rail_passengers(self.trip_rail_passengers);
        self.city
            .set_pe_dw_subway_passengers(self.trip_subway_passengers);
        if !self.can_persist_viewer_service_misc() {
            return;
        }
        self.city
            .set_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, self.trip_bus_passengers);
        self.city
            .set_misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET, self.trip_rail_passengers);
        self.city
            .set_misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET, self.trip_subway_passengers);
    }
}
