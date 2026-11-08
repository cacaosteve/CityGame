//! Transport budget deterioration workers and their existing test adapters.
//!
//! Raw funding gates, tile-family dispatch, process RNG handoff, map writes and
//! dirty/redraw boundaries retain their existing behavior. Growth scheduling,
//! shared destruction primitives and monthly XTRF simulation stay with callers.

#[cfg(test)]
use super::sim_calendar_growth_phases;
use super::{
    PE_NEWSPAPER_STORY_BRIDGE_COLLAPSE, TILE_ROAD_LR, UNDER_TILE_SUBWAY_ENTRANCE, ViewerState,
    ZONE_AIRPORT, ZONE_SEAPORT, budget_service_funding_raw_for_city, clear_xbit_powerable,
    is_bridge_tile, is_highway_tile, is_rail_tile, is_road_tile,
    pe_destroy_building_mode0_optional_crt, pe_place_tile_441f00, pe_place_xund_442370,
    sim_calendar_growth_phase_tiles, underground_tile_counts_as_subway,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::win95_random::{Win95CrtRand, Win95Lfsr};

impl ViewerState {
    fn apply_transport_budget_decay_for_phase(
        &mut self,
        step: u16,
        substep: u16,
        force_probe: bool,
        lfsr: &mut Win95Lfsr,
    ) -> usize {
        let mut rng = self.pe_process_crt;
        let mut decayed = 0usize;
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            decayed = decayed.saturating_add(self.maybe_decay_surface_transport_at(
                x,
                y,
                &mut rng,
                lfsr,
                force_probe,
            ));
        }
        self.pe_process_crt = rng;
        if decayed > 0 {
            self.pending_traffic_redraw = true;
            self.city.mark_all_dirty();
        }
        decayed
    }

    fn apply_subway_budget_decay_for_phase(
        &mut self,
        step: u16,
        substep: u16,
        force_probe: bool,
        lfsr: &mut Win95Lfsr,
    ) -> usize {
        let mut rng = self.pe_process_crt;
        let mut decayed = 0usize;
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            decayed = decayed.saturating_add(self.maybe_decay_subway_at(
                x,
                y,
                &mut rng,
                lfsr,
                force_probe,
            ));
        }
        self.pe_process_crt = rng;
        if decayed > 0 {
            self.city.mark_all_dirty();
        }
        decayed
    }

    pub(super) fn maybe_decay_surface_transport_at(
        &mut self,
        x: usize,
        y: usize,
        rng: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        force_probe: bool,
    ) -> usize {
        if self
            .city
            .zone_at(x, y)
            .is_some_and(|zone| zone.zone_id() != 0)
        {
            return 0;
        }
        let building = self.city.building_at(x, y);
        if building < TILE_ROAD_LR {
            return 0;
        }
        // PE DoBudgetOvergroundTransportCheck @ `0x43599e`: `0x402b3f` RandomAnd7F
        // (`state & 0x7f`); nonzero → `0x435dda` (skip transport decay).
        if !force_probe && lfsr.random_mask_7f() != 0 {
            self.update_microsim_growth_tick_at(x, y, lfsr, false);
            return 0;
        }
        let Some(budget_index) = transport_surface_budget_index(building) else {
            self.update_microsim_growth_tick_at(x, y, lfsr, force_probe);
            return 0;
        };
        // PE DoBudgetHighwayCheck @ `0x435add`–`0x435aed`: `test SI/DI, 1` →
        // `jnz 0x43689b` before funding CRT @ `0x435b0e`. Odd anchors must not
        // consume the underfunding draw.
        if budget_index == 6 && (x % 2 != 0 || y % 2 != 0) {
            return 0;
        }
        // PE reads a WORD from the active budget row (`+0x4c4`, `+0x534`,
        // `+0x5a4`, `+0x614`, or `+0x6f4`) and sign-extends it for `JL`.
        // Keep this separate from the 0..=100 UI getter: saved raw values also
        // control whether the shared CRT stream advances.
        let funding_word = budget_service_funding_raw_for_city(&self.city, budget_index) as u16;
        let collapses = if budget_index == 7 {
            rng.roll_pe_bridge_underfunding_decay(funding_word, self.city.pe_b_weather_wind)
        } else {
            rng.roll_pe_underfunding_decay(funding_word)
        };
        if !collapses {
            return 0;
        }
        if budget_index == 6 {
            let decayed = self.rubble_highway_block(x, y, rng);
            self.update_microsim_growth_tick_at(x, y, lfsr, force_probe);
            return decayed;
        }
        if budget_index == 7 {
            // PE DoBudgetBridgeCheck @ `0x435cbe` AutoGoto `0x4019ec`, DestroyBuilding
            // `0x402211`, newspaper story 39 — no invent status-line string.
            self.queue_budget_collapse_auto_goto(x, y);
            self.destroy_bridge_tile(x, y, rng);
            self.city
                .queue_pe_newspaper_story(PE_NEWSPAPER_STORY_BRIDGE_COLLAPSE, 0);
            self.update_microsim_growth_tick_at(x, y, lfsr, force_probe);
            return 1;
        }
        if budget_index == 10 {
            // PE inline tunnel arm @ `0x435a58` (DestroyBuilding mode 1) is unreachable
            // for XBLD `0x3f..=0x42` — road band @ `0x4359bc` catches them first. Kept
            // for the PE body if a future dispatcher maps a tile here.
            // PE @ `0x435aa1` AutoGoto `0x4019ec` then DestroyBuilding — no status string.
            self.queue_budget_collapse_auto_goto(x, y);
            self.destroy_tunnel_structure_at(x, y, rng);
            self.update_microsim_growth_tick_at(x, y, lfsr, force_probe);
            return 1;
        }
        self.rubble_transport_tile(x, y, rng);
        self.update_microsim_growth_tick_at(x, y, lfsr, force_probe);
        1
    }

    pub(super) fn maybe_decay_subway_at(
        &mut self,
        x: usize,
        y: usize,
        rng: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        force_probe: bool,
    ) -> usize {
        // PE `SimulationGrowthTick` handles seaport zone 9 at `0x43651a..0x4365f0`
        // and airport zone 8 at `0x4365f0..0x43680b`; both paths jump to the lattice
        // epilogue without entering DoBudgetSubwayCheck. Remaining zones reach `0x43680b`,
        // which calls `0x402b3f` RandomAnd7F before the XUND check; nonzero → `0x43689b`.
        if self
            .city
            .zone_at(x, y)
            .is_some_and(|zone| matches!(zone.zone_id(), ZONE_AIRPORT | ZONE_SEAPORT))
        {
            return 0;
        }
        if !force_probe && lfsr.random_mask_7f() != 0 {
            return 0;
        }
        let tile = self.city.underground_at(x, y);
        if !underground_tile_counts_as_subway(tile) {
            return 0;
        }
        // PE DoBudgetSubwayCheck @ `0x436855..0x43687c` reads the raw low WORD
        // from the active budget row. Only literal 100 skips `rand()`; all other
        // words consume one CRT draw and compare against the sign-extended WORD.
        // Keep the player-facing 0..=100 getter out of this simulation path.
        let funding_word = budget_service_funding_raw_for_city(&self.city, 9) as u16;
        if !rng.roll_pe_underfunding_decay(funding_word) {
            return 0;
        }
        // PE DoBudgetSubwayCheck @ `0x43687e`: `cmp bx, 0x23` → push 0 / DestroyBuilding
        // (`0x402211`) then `jmp 0x43689b` past PlaceXUND. Non-entrance @ `0x436893`
        // is PlaceXUND(0) only (crossovers `0x1f`/`0x20` included — no pipe-stub invent).
        // Station XBLD `0xe9` is `>= 0x70`, so DestroyBuilding's FUN_00447020 XUND clear
        // @ `0x45a1ee` does not run — leave XUND `0x23`.
        if tile == UNDER_TILE_SUBWAY_ENTRANCE {
            pe_destroy_building_mode0_optional_crt(&mut self.city, Some(rng), x, y);
        } else {
            pe_place_xund_442370(&mut self.city, x, y, 0);
        }
        // PE DoBudgetSubwayCheck → DestroyBuilding / PlaceXUND then lattice step
        // @ `0x43689b` — no invent MISC/budget/reward refresh or neighbor reshape.
        1
    }

    pub(super) fn rubble_transport_tile(&mut self, x: usize, y: usize, rng: &mut Win95CrtRand) {
        // PE DoBudgetRoadCheck @ `0x435d9c`/`0x435daa` / RailCheck @ `0x435d2f`: CRT
        // `&3+1` PlaceTile then XBIT `&= 0x7f` @ `0x435dcc` → `0x435dda`. Clears
        // powerable only (keeps powered `0x40`); no invent PlaceRoads/PlaceRails
        // neighbor reshape or MISC/budget/reward refresh. Highway E/S/SE PlaceTiles
        // are a separate arm (`0x435add`), not road/rail.
        let tile = rng.rubble_tile_id();
        pe_place_tile_441f00(&mut self.city, x, y, tile);
        // PE @ `0x435dcc`/`0x435dd6`: XBIT `&= 0x7f` (clear powerable).
        clear_xbit_powerable(&mut self.city, x, y);
    }

    /// PE bridge/tunnel DoBudget AutoGoto @ `0x4019ec` when Options Auto-Goto is on.
    fn queue_budget_collapse_auto_goto(&mut self, x: usize, y: usize) {
        if self.auto_goto_enabled {
            self.pending_disaster_auto_goto = Some((x, y));
        }
    }

    pub(super) fn rubble_highway_block(
        &mut self,
        origin_x: usize,
        origin_y: usize,
        rng: &mut Win95CrtRand,
    ) -> usize {
        let mut decayed = 0usize;
        // PE DoBudgetHighwayCheck @ `0x435b30`–`0x435c71`: after the even/even highway
        // arm fires, PlaceTile the full 2×2 footprint with no per-cell XBLD filter.
        // Water cells (`test XBIT, 0x04` @ `0x435b52` / E/S/SE twins) get PlaceTile(0);
        // else CRT `&3+1`. Pure bridge XBLD inside the footprint is PlaceTile-rubbled
        // here; DoBudgetBridgeCheck DestroyBuilding is only when the walker lands on
        // a bridge tile as the primary budget tile.
        // PE then `jmp 0x435dda` — it does **not** fall through road/rail
        // `XBIT &= 0x7f` @ `0x435dcc`. No invent clear_xbit_powerable here.
        for y in origin_y..origin_y.saturating_add(2).min(CITY_HEIGHT) {
            for x in origin_x..origin_x.saturating_add(2).min(CITY_WIDTH) {
                if self.highway_budget_xbit_water_at(x, y) {
                    // PE @ `0x435b52`/`0x435b58`: XBIT `0x04` (water_tile) → PlaceTile `0`.
                    pe_place_tile_441f00(&mut self.city, x, y, 0);
                } else {
                    // PE @ `0x435b5c`/`0x435b6a`: CRT `&3+1` then PlaceTile `0x40178f`.
                    pe_place_tile_441f00(&mut self.city, x, y, rng.rubble_tile_id());
                }
                // PlaceTile only — XTXT survives (no invent clear).
                decayed += 1;
            }
        }
        decayed
    }

    /// PE DoBudgetHighwayCheck @ `0x435b52`: `test XBIT, 0x04` (water_tile) only —
    /// not XTER water.
    fn highway_budget_xbit_water_at(&self, x: usize, y: usize) -> bool {
        self.city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.water_tile())
    }

    #[cfg(test)]
    pub(super) fn decay_transport_budget_for_test(&mut self) -> usize {
        let mut growth_lfsr = self.pe_process_lfsr;
        let mut total = 0usize;
        for (step, substep) in sim_calendar_growth_phases() {
            total = total.saturating_add(self.apply_transport_budget_decay_for_phase(
                step,
                substep,
                true,
                &mut growth_lfsr,
            ));
            total = total.saturating_add(self.apply_subway_budget_decay_for_phase(
                step,
                substep,
                true,
                &mut growth_lfsr,
            ));
        }
        self.pe_process_lfsr = growth_lfsr;
        total
    }

    #[cfg(test)]
    pub(super) fn decay_subway_at_for_test(&mut self, x: usize, y: usize) -> usize {
        let mut rng = self.pe_process_crt;
        let mut lfsr = self.pe_process_lfsr;
        let decayed = self.maybe_decay_subway_at(x, y, &mut rng, &mut lfsr, true);
        self.pe_process_crt = rng;
        self.pe_process_lfsr = lfsr;
        decayed
    }

    #[cfg(test)]
    pub(super) fn decay_subway_at_for_test_noforce(&mut self, x: usize, y: usize) -> usize {
        let mut rng = self.pe_process_crt;
        let mut lfsr = self.pe_process_lfsr;
        let decayed = self.maybe_decay_subway_at(x, y, &mut rng, &mut lfsr, false);
        self.pe_process_crt = rng;
        self.pe_process_lfsr = lfsr;
        decayed
    }

    #[cfg(test)]
    pub(super) fn decay_surface_transport_at_for_test(&mut self, x: usize, y: usize) -> usize {
        let mut rng = self.pe_process_crt;
        let mut lfsr = self.pe_process_lfsr;
        let decayed = self.maybe_decay_surface_transport_at(x, y, &mut rng, &mut lfsr, true);
        self.pe_process_crt = rng;
        self.pe_process_lfsr = lfsr;
        decayed
    }

    /// PE growth lattice gate @ `0x435988`: only XZON nibble 0 reaches surface DoBudget.
    #[cfg(test)]
    pub(super) fn decay_surface_via_growth_zone_gate_for_test(
        &mut self,
        x: usize,
        y: usize,
    ) -> usize {
        let zone_id = self.city.zone_at(x, y).map(|zone| zone.zone_id());
        if !zone_id.is_none_or(|id| id == 0) {
            return 0;
        }
        self.decay_surface_transport_at_for_test(x, y)
    }

    #[cfg(test)]
    pub(super) fn decay_surface_transport_at_for_test_noforce(
        &mut self,
        x: usize,
        y: usize,
    ) -> usize {
        let mut rng = self.pe_process_crt;
        let mut lfsr = self.pe_process_lfsr;
        let decayed = self.maybe_decay_surface_transport_at(x, y, &mut rng, &mut lfsr, false);
        self.pe_process_crt = rng;
        self.pe_process_lfsr = lfsr;
        decayed
    }
}

pub(super) fn transport_surface_budget_index(building: u8) -> Option<usize> {
    // Service budget indices: roads=5, highways=6, bridges=7, rail=8, tunnel=10.
    // PE DoBudgetBridgeCheck @ `0x435a34`–`0x435a3e`: XBLD `0x51..=0x5c` → `0x435c76`
    // before highway; overlaps `0x6a`/`0x6b` @ `0x435a44`/`0x435a4e` likewise.
    // Bridge must win the highway overlap (and power-line label tile `0x5c`).
    // PE DoBudgetRoadCheck @ `0x4359bc`–`0x4359c6` / `0x4359cc`/`0x4359d6` /
    // `0x4359e0`–`0x4359ea`: XBLD `0x3f..=0x46`, `0x4b`/`0x4c`, and on-ramps
    // `0x5d..=0x60` → road PlaceTile arm (`0x435d5f`). The `0x3f..=0x46` band runs
    // before rail and before the tunnel arm @ `0x435a58`, so portals `0x3f..=0x42`,
    // power×road `0x43`/`0x44`, and road×rail `0x45`/`0x46` all take road funding —
    // the inline tunnel DestroyBuilding body is unreachable for those XBLDs.
    // PE DoBudgetRailCheck @ `0x435a06` / `0x435a20`/`0x435a2a`: XBLD `0x45..=0x48`
    // and `0x4d`/`0x4e` → rail PlaceTile arm (`0x435ce4`). After the earlier road
    // band, only power×rail `0x47`/`0x48` still reach `0x435a06`; `0x4d`/`0x4e` must
    // not take highway 2×2 (`0x435add`).
    if is_bridge_tile(building) {
        Some(7)
    } else if matches!(building, 0x3f..=0x46 | 0x4b | 0x4c | 0x5d..=0x60) {
        Some(5)
    } else if matches!(building, 0x47 | 0x48 | 0x4d | 0x4e) {
        Some(8)
    } else if is_highway_tile(building) {
        Some(6)
    } else if is_rail_tile(building) {
        Some(8)
    } else if is_road_tile(building) {
        Some(5)
    } else {
        None
    }
}
