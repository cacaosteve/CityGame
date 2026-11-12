//! Bulldozer transactions, shared destruction entry points, and footprint cleanup.
//!
//! These workers preserve the existing billing, RNG, and city-plane write order.
//! Disaster-specific scheduling and terrain/PlaceTile primitives stay in their
//! existing modules; the Mac destruction worker remains in `budget`.

use super::{
    ToolPlacement, ToolPreview, UNDER_TILE_BUILDING_PIPE_TEE, UNDER_TILE_GREY_BOX_1,
    UNDER_TILE_GREY_BOX_2, UndergroundKind, ViewerState, ZONE_MILITARY, budget,
    building_footprint_from_origin, is_bridge_tile, is_decorative_water_terrain, is_water_terrain,
    neighbor_tiles, network_kinds_for_building, offset_city_tile,
    pe_fun_00447020_mode0_xund_slope_sea, pe_fun_00447020_mode1_destroy_and_city_clear_447085,
    pe_highway_classify_463c40, pe_place_tile_441f00, pe_place_water_helper_446650,
    pe_place_water_reslope_neighbors_446a66, pe_place_xund_442370,
    pe_selected_water_height_for_fun_00447020, terrain_generation, tunnel_portal_direction,
    underground_user_pipe_tile, xbld_tile_size,
};
use crate::city::{CITY_HEIGHT, CITY_TILES, CITY_WIDTH, City, TileBits};
use crate::engine_fidelity::EngineFidelity;
use crate::ui::EditTool;
use crate::win95_random::Win95CrtRand;
use sdl3::keyboard::Mod;
use std::collections::HashSet;

mod bridge;
mod destroy_building;
mod map_tool;

pub(super) use bridge::*;
pub(super) use destroy_building::*;
pub(super) use map_tool::*;

impl ViewerState {
    fn bulldozer_cell_refused(&self, x: usize, y: usize) -> bool {
        // The first gate is CODE 2 isCmdDown on Mac and GetAsyncKeyState(VK_MENU)
        // on Win95. It applies to the clicked XTXT marker in either view mode.
        let override_key = if self.engine_fidelity.uses_win95() {
            Mod::LALTMOD | Mod::RALTMOD
        } else {
            Mod::LGUIMOD | Mod::RGUIMOD
        };
        if (self.city.text_overlay_at(x, y) == Some(0xff)
            && !self.modifiers.intersects(override_key))
            || self
                .city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == 7)
        {
            return true;
        }
        if self.show_underground {
            // Both originals admit XUND, ALTM tunnel level 1 or XBIT piped.
            return pe_maptool_underground_empty_refuse_443350(&self.city, x, y);
        }
        let building = self.city.building_at(x, y);
        building == 5 || (building == 0 && self.city.terrain_at(x, y) < 0x30)
    }

    pub(super) fn bulldoze_cost_units_for_click(
        &self,
        x: usize,
        y: usize,
        _legacy_tile_count: usize,
    ) -> u32 {
        // Both originals charge once per admitted cursor coordinate.
        u32::from(!self.bulldozer_cell_refused(x, y))
    }

    pub(super) fn bulldoze_placement_at(&self, x: usize, y: usize) -> ToolPlacement {
        let tiles = self.bulldoze_clearable_tiles(x, y);
        if tiles.is_empty() {
            ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview: ToolPreview::Bulldoze,
            }
        } else {
            ToolPlacement::valid(ToolPreview::Bulldoze, tiles)
        }
    }

    pub(super) fn destroy_bridge_tile(&mut self, x: usize, y: usize, rng: &mut Win95CrtRand) {
        // PE DoBudgetBridgeCheck AutoGoto `0x4019ec` @ `0x435cbe` then DestroyBuilding
        // mode 1 @ `0x435cce` (`0x402211`) + newspaper `0x402900` story 39 → `jmp 0x435dda`.
        // No invent PlaceRoads/PlaceRails neighbor reshape, MISC/budget/reward refresh,
        // or status-line string (same epilogue class as tunnel @ `0x435dda`).
        pe_destroy_building_mode1_optional_crt(&mut self.city, Some(rng), x, y);
    }

    pub(super) fn destroy_tunnel_structure_at(
        &mut self,
        origin_x: usize,
        origin_y: usize,
        rng: &mut Win95CrtRand,
    ) {
        // PE DoBudgetTunnelCheck AutoGoto `0x4019ec` @ `0x435aa1` then DestroyBuilding
        // mode 1 @ 0x435ab3, then jump to the budget loop tail. The shared
        // DestroyBuilding worker owns both portals and the intervening ALTM.
        pe_destroy_building_mode1_optional_crt(&mut self.city, Some(rng), origin_x, origin_y);
    }

    pub(super) fn clear_building_underground_side_effects(
        &mut self,
        x: usize,
        y: usize,
        building: u8,
    ) {
        match building {
            220 => {
                // PlacePipesAtCoordinates left user-pipe / crossover XUND — clear those, and
                // any legacy OpenCity tee still present on older saves.
                let tile = self.city.underground_at(x, y);
                if tile == UNDER_TILE_BUILDING_PIPE_TEE || underground_user_pipe_tile(tile) {
                    pe_place_xund_442370(&mut self.city, x, y, 0);
                }
            }
            250 => {
                self.clear_building_underground_footprint(x, y, UNDER_TILE_BUILDING_PIPE_TEE);
            }
            108..=111 | 233 => {
                // PE MapTool surface clear @ `0x44344b`: PlaceXUND(0) for these XBLDs
                // (no XUND==0x23 gate). Neighbor reshape stays local.
                if pe_maptool_surface_clear_subway_xund_44344b(&mut self.city, x, y, building) {
                    for (nx, ny) in neighbor_tiles(x, y) {
                        self.refresh_underground_tile(nx, ny, UndergroundKind::Subway);
                    }
                }
            }
            249 => {
                self.clear_building_underground_footprint(x, y, UNDER_TILE_GREY_BOX_1);
            }
            _ => {}
        }
    }

    pub(super) fn clear_building_underground_footprint(&mut self, x: usize, y: usize, tile: u8) {
        let Some(footprint) = self.building_footprint_at(x, y) else {
            return;
        };
        for (tx, ty) in footprint {
            if self.city.underground_at(tx, ty) == tile {
                // PE PlaceXUND(0): XUND only — leave watered/piped/XTRF.
                pe_place_xund_442370(&mut self.city, tx, ty, 0);
            }
        }
    }

    /// Underground-view bulldoze: PE MapTool DisplayLayer clear (`0x4433ce`–`0x44342f`).
    pub(super) fn bulldoze_underground_at(&mut self, x: usize, y: usize) {
        if self.bulldozer_cell_refused(x, y) {
            return;
        }
        let und = self.city.underground_at(x, y);
        let billable = 1u32;
        if !self.can_afford_tool_build_cost(EditTool::Bulldoze, billable) {
            return;
        }
        self.debit_tool_build_cost(EditTool::Bulldoze, billable);
        if self.engine_fidelity == EngineFidelity::Mac68k {
            let building = self.city.building_at(x, y);
            if building < 0x70 {
                pe_maptool_underground_clear_piped_bit_4433fa(&mut self.city, x, y);
            }
            if (self.city.tunnel_levels_at(x, y) & 0x1f) == 1 || und == UNDER_TILE_GREY_BOX_2 {
                // CODE 5 `FUN_0000884e` calls export 80 with render flag `-1`
                // for these two underground cases. Preserve its Mac Toolbox
                // draw order and map-plane cleanup instead of routing this
                // accepted Mac edit through the PE flood-destroy worker.
                budget::mac_destroy_building_export80(
                    &mut self.city,
                    x,
                    y,
                    &mut self.mac_process_rng,
                    true,
                );
            }
            // Native export 106 is called for every accepted underground
            // click, including the cases that skip export 80.
            self.city.set_underground(x, y, 0);
        } else {
            pe_maptool_underground_clear_subway_443427(
                &mut self.city,
                Some(&mut self.pe_process_crt),
                x,
                y,
                true,
            );
        }
        if self.engine_fidelity == EngineFidelity::Mac68k {
            // CODE 13 export 198 runs after export 106 and retile-checks the
            // four cardinal XBLD neighbors. Export 199's rooted helpers do
            // not directly write XUND; they invoke the Mac timing/event pump
            // and scene-redraw gateway. That is not evidence for PE-style
            // pipe/subway reshapes or the PE-only water recalculation.
            self.mac_export198_retile_surface_neighbors(x, y);
        } else {
            pe_maptool_epilogue_44378e(&mut self.city, x, y);
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
        self.invalidate_sprite_cache();
    }

    pub(super) fn bulldoze_at(&mut self, x: usize, y: usize) {
        if self.bulldozer_cell_refused(x, y) {
            return;
        }
        let billable_tiles = self.bulldoze_billable_tiles(x, y);
        // Capture the PE caller's one-unit transaction before any PlaceTile or
        // DestroyBuilding writes change the clicked cell's classification.
        let billable_units = self.bulldoze_cost_units_for_click(x, y, billable_tiles.len());
        let old_building = self.city.building_at(x, y);
        let mut mac_lowclass_cost_debited = false;
        if self.engine_fidelity.uses_win95() {
            // FUN_00443270 debits after its ordinary-city refusal gates but
            // before the random rubble refusal or any tile mutation.
            self.debit_tool_build_cost(EditTool::Bulldoze, billable_units);
            if matches!(old_building, 6..=12) && self.pe_process_crt.random_mod(20) == 0 {
                // PE's 1-in-20 refusal is after the $1 debit and before
                // PlaceTile(0); the caller still refreshes the view.
                self.query_is_inspect = false;
                self.refresh_status_line();
                return;
            }
            if old_building < 0x0d {
                self.win95_player_bulldoze_lowclass_at(x, y, old_building);
                pe_maptool_epilogue_44378e(&mut self.city, x, y);
                self.invalidate_sprite_cache();
                self.query_is_inspect = false;
                self.refresh_status_line();
                return;
            }
        } else if matches!(old_building, 6..=12) {
            // Mac CODE 5 FUN_0000884e charges selector 0/state 0 before its
            // QuickDraw `_Random % 20` low-class refusal. Preserve that draw
            // and the paid no-op branch before entering the shared map edit.
            self.debit_tool_build_cost(EditTool::Bulldoze, billable_units);
            mac_lowclass_cost_debited = true;
            if self.mac_process_rng.random_u16() % 20 == 0 {
                self.query_is_inspect = false;
                self.refresh_status_line();
                return;
            }
        }
        // Compute this before the support-tip and footprint writes: the PE
        // selector-2 low-class gate classifies the snapped NW highway window
        // while the original map is still live.
        let selector2_lowclass_cleanup =
            pe_destroy_building_selector2_low_class_cleanup_order(&self.city, old_building, x, y);
        let old_kinds: Vec<_> = network_kinds_for_building(old_building).collect();
        if self.engine_fidelity.uses_win95() && old_building >= 0x0d {
            // Ordinary Win95 city clicks reach FUN_00443270 → DestroyBuilding(1)
            // for every high-class hit. Delegate the complete mode-1 selector,
            // scatter, footprint, and bridge-walk behavior to that port instead
            // of duplicating only selected plane writes here.
            let before: Vec<_> = (0..CITY_TILES)
                .map(|index| {
                    let (tx, ty) = (index % CITY_WIDTH, index / CITY_WIDTH);
                    (
                        self.city.building_at(tx, ty),
                        self.city.terrain_at(tx, ty),
                        self.city.altitude_at(tx, ty),
                        self.city.tile_bits_at(tx, ty).map(TileBits::raw_bits),
                        self.city.zone_at(tx, ty).map(|zone| zone.raw_bits()),
                        self.city.underground_at(tx, ty),
                        self.city.text_overlay_at(tx, ty),
                    )
                })
                .collect();
            // FUN_00443270 calls FUN_00442370 before DestroyBuilding for the
            // subway transition/substation IDs; this is an XUND-only clear.
            pe_maptool_surface_clear_subway_xund_44344b(&mut self.city, x, y, old_building);
            pe_destroy_building_mode1_optional_crt(
                &mut self.city,
                Some(&mut self.pe_process_crt),
                x,
                y,
            );
            // FUN_00443270 resumes after DestroyBuilding with its ordinary-city
            // XTER/PlaceWater cleanup before the common neighbor sweeps.
            pe_maptool_surface_xter_gate_4434de(&mut self.city, x, y, true);
            pe_maptool_epilogue_44378e(&mut self.city, x, y);
            let changed_tiles: Vec<_> = before
                .iter()
                .enumerate()
                .filter_map(|(index, old)| {
                    let (tx, ty) = (index % CITY_WIDTH, index / CITY_WIDTH);
                    let current = (
                        self.city.building_at(tx, ty),
                        self.city.terrain_at(tx, ty),
                        self.city.altitude_at(tx, ty),
                        self.city.tile_bits_at(tx, ty).map(TileBits::raw_bits),
                        self.city.zone_at(tx, ty).map(|zone| zone.raw_bits()),
                        self.city.underground_at(tx, ty),
                        self.city.text_overlay_at(tx, ty),
                    );
                    (*old != current).then_some((tx, ty))
                })
                .collect();
            self.clear_zone_growth_for_player_tiles(&changed_tiles);
            self.refresh_live_misc_current_values();
            self.refresh_budget_service_rows_from_live_counts();
            self.refresh_reward_availability();
            self.pending_traffic_redraw = true;
            self.query_is_inspect = false;
            self.refresh_status_line();
            return;
        }
        let mac_export80_surface = old_building >= 0x0d
            && !(budget::is_mac_bridge_tile(old_building)
                && self.mac_surface_bridge_uses_legacy_axis(x, y));
        if self.engine_fidelity == EngineFidelity::Mac68k
            && !self.show_underground
            && (old_building < 0x0d || mac_export80_surface)
        {
            // CODE 5 0x884e debits before dispatching PlaceTile(0) for low
            // classes or export 80 with rendering enabled for buildings.
            if !mac_lowclass_cost_debited {
                self.debit_tool_build_cost(EditTool::Bulldoze, billable_units);
            }
            if mac_export80_surface {
                if self.zone_growth_uses_allowlist {
                    let tiles = self.bulldoze_clearable_tiles(x, y);
                    self.clear_zone_growth_for_player_tiles(&tiles);
                }
                // 0x89ca calls PlaceXUND(0) before export 80 for these IDs.
                if matches!(old_building, 0x6c..=0x70 | 0xe9) {
                    self.city.set_underground(x, y, 0);
                }
                budget::mac_destroy_building_export80(
                    &mut self.city,
                    x,
                    y,
                    &mut self.mac_process_rng,
                    true,
                );
            } else {
                pe_place_tile_441f00(&mut self.city, x, y, 0);
            }
            // Normalize water at the click after the removal worker, then
            // recheck its four surface neighbors through export 198.
            if self.city.terrain_at(x, y) >= 0x30 {
                if let Some(bits) = self.city.tile_bits_at(x, y) {
                    self.city.set_tile_bits(x, y, bits.with_water_tile(false));
                }
                if self.city.terrain_at(x, y) == 0x3e {
                    budget::mac_classify_terrain_tile(
                        &mut self.city,
                        x,
                        y,
                        &mut self.mac_process_rng,
                    );
                } else {
                    self.city.set_terrain(x, y, 0);
                }
                terrain_generation::mac68k_refresh_water_neighbors_after_surface_clear(
                    &mut self.city,
                    x,
                    y,
                );
            }
            self.mac_export198_retile_surface_neighbors(x, y);
            if !mac_export80_surface {
                self.refresh_live_misc_current_values();
                self.refresh_budget_service_rows_from_live_counts();
                self.refresh_reward_availability();
            }
            self.pending_traffic_redraw = true;
            self.query_is_inspect = false;
            self.refresh_status_line();
            return;
        }
        // OpenSC2K DemolishBridges: bulldozing any bridge tile clears the contiguous span.
        let tiles = if is_bridge_tile(old_building) {
            opensc2k_bridge_span_tiles(&self.city, x, y)
        } else if let Some(tiles) = selector2_lowclass_cleanup.clone() {
            tiles
        } else {
            // PE DestroyStructure footprint walk (`0x402496` / `0x45b1c0`): highways are 2×2.
            pe_destroy_building_footprint(&self.city, x, y)
        };
        let bridge_span_demolish = is_bridge_tile(old_building) && tiles.len() > 1;
        let origin_x = tiles.iter().map(|&(tx, _)| tx).min().unwrap_or(x);
        let origin_y = tiles.iter().map(|&(_, ty)| ty).min().unwrap_or(y);
        // PE support-walk + tip land-- runs while XBLD is still live (`0x458e0f`).
        if !bridge_span_demolish {
            pe_destroy_building_size1_support_land_dec_458f90(&mut self.city, x, y, old_building);
        }
        self.clear_building_underground_side_effects(x, y, old_building);

        for &(tx, ty) in &tiles {
            if is_decorative_water_terrain(self.city.terrain_at(tx, ty)) {
                let sample = self.city.altm_at(tx, ty);
                self.city.set_terrain(tx, ty, 0);
                self.city.set_altitude(tx, ty, sample.land, 0);
            }
        }
        // PE DestroyBuilding footprint: selective XTXT/XLAB, then clear XBLD via PlaceTile
        // so `dwTileCount` stays coherent (not raw `set_building(0)` then PlaceTile).
        // Elevated `0x3f..=0x42` arm never reaches XTXT clear @ `0x45a0c4`.
        // Do not use pe_flood_destroy's blanket XTXT wipe — PE skips `0xc9..=0xf9`.
        let mac_export80_fa_counter = !self.engine_fidelity.uses_win95()
            && budget::mac_export80_uses_generic_cell_cleanup(old_building);
        if mac_export80_fa_counter && !tiles.is_empty() {
            // The Mac surface bulldozer reaches export 80 for high-class
            // buildings. Reopen the civic tool when its building is removed.
            budget::mac_export80_update_special_civic_latch(&mut self.city, old_building);
        }
        for &(tx, ty) in &tiles {
            if !matches!(old_building, 0x3f..=0x42) {
                // The Mac generic export-80 XTXT `0xfa` arm decrements its
                // persisted network/non-network counter using the original
                // root XBLD. The shared PE clear helper does not own that
                // Mac state side effect.
                if mac_export80_fa_counter && self.city.text_overlay_at(tx, ty) == Some(0xfa) {
                    self.city
                        .mac_export80_decrement_xtxt_fa_counter(old_building);
                }
                if self.engine_fidelity.uses_win95()
                    && self.city.text_overlay_at(tx, ty) == Some(0xfa)
                    && !self.city.is_synthetic_trip_marker(tx, ty)
                    && !super::pe_xbld_counts_fa_network_overlay_4cc4d8(old_building)
                {
                    self.city.pe_nonnetwork_neighbor_links =
                        self.city.pe_nonnetwork_neighbor_links.wrapping_sub(1);
                }
                pe_destroy_building_clear_xtxt_45a0c4(&mut self.city, tx, ty);
            }
            // Elevated arm PlaceTile(0) @ `0x45994e`; general arm PlaceTile @ `0x45a01f`.
            // PE DestroyBuilding early-outs for XBLD < 6 (`0x458d8a`) — no CRT rubble restamp.
            if matches!(old_building, 0x3f..=0x42) {
                pe_place_tile_441f00(&mut self.city, tx, ty, 0);
            } else if bridge_span_demolish {
                // OpenSC2K DemolishBridges: NetworkState.replace_building → EMPTY (no CRT rubble).
                pe_place_tile_441f00(&mut self.city, tx, ty, 0);
            } else if old_building >= 6 {
                pe_destroy_building_placetile_45a01f(
                    &mut self.city,
                    &mut self.pe_process_crt,
                    tx,
                    ty,
                );
            } else {
                pe_place_tile_441f00(&mut self.city, tx, ty, 0);
            }
            // PE DestroyBuilding @ `0x45997e`: XZON &= 0x0f (keep type, clear corners).
            // Elevated arm uses XZON table `0x4ca1f0` here — not XBIT `0x4cab10`.
            let zone = self.city.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0);
            self.city.set_zone(tx, ty, zone & 0x0f);
            if matches!(old_building, 0x3f..=0x42) {
                // PE elevated arm has no XBIT &= 0x3d; only ALTM &= 0x3ff @ `0x4599aa`.
                pe_destroy_building_clear_altm_tunnel_4599aa(&mut self.city, tx, ty);
            } else {
                // PE DestroyBuilding @ `0x45a07f`: XBIT &= 0x3d.
                pe_destroy_building_clear_xbit_45a07f(&mut self.city, tx, ty);
            }
        }
        // PE @ `0x459a8c`/`0x459a94`: with XBLD still in `0x3f..=0x42`, the
        // neighbor ALTM walk at `0x459a96` is skipped (`jl` to `0x459b03`).
        // Elevated arm then jumps to DestroyBuilding epilogue — skips
        // `0x45a1ee` / `0x45a19a` FUN_00447020 arms.
        if !bridge_span_demolish && !matches!(old_building, 0x3f..=0x42) {
            // PE DestroyBuilding @ `0x45a1ee`: size-1, XBLD < 0x70, XTER ≠ 0 → FUN_00447020.
            // Sea level matches support-tip / flood DestroyBuilding (`0x4ca818` stand-in).
            let sea = pe_selected_water_height_for_fun_00447020(&self.city);
            pe_destroy_building_size1_nonflat_fun_00447020_45a1ee(
                &mut self.city,
                x,
                y,
                old_building,
                sea,
            );
            if selector2_lowclass_cleanup.is_some() {
                // The low-class selector-2 route reaches the four-cell
                // post-cleanup window at `0x45a19a`; it never enters either
                // high-class walker helper window.
                pe_destroy_building_selector2_lowclass_post_cleanup_fun_00447020(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    x,
                    y,
                    old_building,
                    sea,
                );
            } else {
                // Other non-Win95 highway/bridge routes still use the documented fallback approximation.
                pe_destroy_building_highway_bridge_neighbor_fun_00447020_45a19a(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    origin_x,
                    origin_y,
                    old_building,
                    sea,
                );
            }
        }
        // OpenSC2K DemolishBridges: after the span is EMPTY, retile both dry banks.
        let bank_tiles = if bridge_span_demolish {
            opensc2k_demolish_bridge_retile_banks(&mut self.city, &tiles)
        } else {
            Vec::new()
        };
        if matches!(old_building, 220 | 250) {
            // After the pump footprint is gone, reshape adjacent user pipes (PE 0x4016b8).
            self.refresh_pipe_neighbors_of_tiles(&tiles);
        }
        self.clear_zone_growth_for_player_tiles(&tiles);
        // PE DestroyStructure: no invent citywide water-network refresh.
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();

        for kind in old_kinds {
            for &(tx, ty) in tiles.iter().chain(bank_tiles.iter()) {
                for (nx, ny) in neighbor_tiles(tx, ty) {
                    self.refresh_network_tile(nx, ny, kind);
                }
            }
            self.pending_traffic_redraw = true;
        }
        if !self.engine_fidelity.uses_win95() && !mac_lowclass_cost_debited {
            self.debit_tool_build_cost(EditTool::Bulldoze, billable_units);
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
    }

    /// Ordinary Win95 `FUN_00443270` path for XBLD `< 0x0D`: the player worker
    /// uses PlaceTile(0), not DestroyBuilding. IDs 6–12 may have already been
    /// refused by the caller's post-debit 1-in-20 CRT roll.
    pub(super) fn win95_player_bulldoze_lowclass_at(
        &mut self,
        x: usize,
        y: usize,
        old_building: u8,
    ) {
        debug_assert!(self.engine_fidelity.uses_win95());
        debug_assert!(old_building < 0x0d);
        pe_place_tile_441f00(&mut self.city, x, y, 0);

        // FUN_00443270 skips water normalization when the saved XTER is below 0x30.
        // Higher water/edge classes take a small shared surface-normalization
        // path rather than DestroyBuilding's footprint/XBIT masks.
        let terrain = self.city.terrain_at(x, y);
        if terrain < 0x30 {
            return;
        }
        if terrain == 0x3e {
            let sea = pe_selected_water_height_for_fun_00447020(&self.city);
            pe_fun_00447020_mode0_xund_slope_sea(&mut self.city, x, y, sea);
        } else {
            self.city.set_terrain(x, y, 0);
        }
        if let Some(bits) = self.city.tile_bits_at(x, y) {
            self.city.set_tile_bits(x, y, bits.with_water_tile(false));
        }

        // Native low-class cleanup re-normalizes only neighboring cells still
        // marked as water, in this exact west/north/east/south then diagonal
        // order. The PlaceWater helper itself leaves XTRF untouched.
        const WATER_NEIGHBORS: [(isize, isize); 8] = [
            (-1, 0),
            (0, 1),
            (1, 0),
            (0, -1),
            (-1, -1),
            (1, -1),
            (1, 1),
            (-1, 1),
        ];
        for (dx, dy) in WATER_NEIGHBORS {
            let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
                continue;
            };
            if self
                .city
                .tile_bits_at(nx, ny)
                .is_some_and(TileBits::water_tile)
            {
                pe_place_water_helper_446650(&mut self.city, nx, ny);
            }
        }
    }

    pub(super) fn bulldoze_clearable_tiles(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        if self.bulldozer_cell_refused(x, y) {
            return Vec::new();
        }
        if self.show_underground {
            return vec![(x, y)];
        }
        let click_building = self.city.building_at(x, y);
        if !self.engine_fidelity.uses_win95()
            && click_building >= 0x0d
            && budget::mac_export80_uses_generic_cell_cleanup(click_building)
        {
            return self
                .mac_generic_bulldoze_footprint(x, y, click_building)
                .unwrap_or_default();
        }
        let selector2_lowclass_cleanup =
            pe_destroy_building_selector2_low_class_cleanup_order(&self.city, click_building, x, y);
        let tiles = if is_bridge_tile(click_building) {
            opensc2k_bridge_span_tiles(&self.city, x, y)
        } else if let Some(tiles) = selector2_lowclass_cleanup {
            tiles
        } else {
            pe_destroy_building_footprint(&self.city, x, y)
        };
        tiles
            .into_iter()
            .filter(|&(tx, ty)| {
                let building = self.city.building_at(tx, ty);
                let zone = self
                    .city
                    .zone_at(tx, ty)
                    .is_some_and(|zone| zone.raw_bits() != 0);
                let decorative_water = is_decorative_water_terrain(self.city.terrain_at(tx, ty));
                // The rooted Win95 worker also accepts XBLD 0 when XTER is
                // at least 0x30, including the special 0x3e classifier arm.
                let native_terrain_target = self.engine_fidelity.uses_win95()
                    && building == 0
                    && self.city.terrain_at(tx, ty) >= 0x30;
                building != 0 || zone || decorative_water || native_terrain_target
            })
            .collect()
    }

    fn mac_surface_bridge_uses_legacy_axis(&self, x: usize, y: usize) -> bool {
        let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
        if bits.water_tile() || !is_water_terrain(self.city.terrain_at(x, y)) {
            return false;
        }
        // Older CityGame bridge placement could omit both the water flag and
        // axis bit. Keep its perpendicular-span fallback for those saves.
        let (dx, dy) = if bits.rotate() { (1, 0) } else { (0, 1) };
        bridge_span_along_axis(&self.city, x, y, dx, dy).len() == 1
            && bridge_span_along_axis(&self.city, x, y, dy, dx).len() > 1
    }

    fn mac_generic_bulldoze_footprint(
        &self,
        x: usize,
        y: usize,
        building: u8,
    ) -> Option<Vec<(usize, usize)>> {
        let (anchor_x, anchor_y, size) =
            budget::mac_export81_footprint(&self.city, x, y, building)?;
        let origin_x = usize::try_from(anchor_x).ok()?;
        let origin_y = (usize::try_from(anchor_y).ok()? + 1).checked_sub(size)?;
        building_footprint_from_origin(origin_x, origin_y, size)
    }

    pub(super) fn bulldoze_billable_tiles(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        if self.show_underground {
            return if !self.bulldozer_cell_refused(x, y) {
                vec![(x, y)]
            } else {
                Vec::new()
            };
        }
        let click_building = self.city.building_at(x, y);
        let selector2_lowclass_cleanup =
            pe_destroy_building_selector2_low_class_cleanup_order(&self.city, click_building, x, y);
        let tiles = if is_bridge_tile(click_building) {
            opensc2k_bridge_span_tiles(&self.city, x, y)
        } else if let Some(tiles) = selector2_lowclass_cleanup {
            tiles
        } else {
            pe_destroy_building_footprint(&self.city, x, y)
        };
        tiles
            .into_iter()
            .filter(|&(tx, ty)| {
                let building = self.city.building_at(tx, ty);
                // PE DestroyBuilding no-ops for XBLD < 6; rubble clear is free.
                let clearable_building = building >= 6;
                let zone = self
                    .city
                    .zone_at(tx, ty)
                    .is_some_and(|zone| zone.raw_bits() != 0);
                let decorative_water = is_decorative_water_terrain(self.city.terrain_at(tx, ty));
                clearable_building || zone || decorative_water
            })
            .collect()
    }

    pub(super) fn building_footprint_at(&self, x: usize, y: usize) -> Option<Vec<(usize, usize)>> {
        let building = self.city.building_at(x, y);
        if building == 0 {
            return None;
        }
        let size = usize::from(xbld_tile_size(building));
        if size <= 1 {
            return Some(vec![(x, y)]);
        }
        let min_x = x.saturating_sub(size - 1);
        let min_y = y.saturating_sub(size - 1);
        for origin_y in min_y..=y {
            for origin_x in min_x..=x {
                if origin_x + size > CITY_WIDTH || origin_y + size > CITY_HEIGHT {
                    continue;
                }
                let mut tiles = Vec::with_capacity(size * size);
                let mut matches = true;
                for ty in origin_y..origin_y + size {
                    for tx in origin_x..origin_x + size {
                        if self.city.building_at(tx, ty) != building {
                            matches = false;
                            break;
                        }
                        tiles.push((tx, ty));
                    }
                    if !matches {
                        break;
                    }
                }
                if matches {
                    return Some(tiles);
                }
            }
        }

        let mut fragments = HashSet::new();
        let max_origin_x = x.min(CITY_WIDTH - size);
        let max_origin_y = y.min(CITY_HEIGHT - size);
        for origin_y in min_y..=max_origin_y {
            for origin_x in min_x..=max_origin_x {
                for ty in origin_y..origin_y + size {
                    for tx in origin_x..origin_x + size {
                        if self.city.building_at(tx, ty) == building {
                            fragments.insert((tx, ty));
                        }
                    }
                }
            }
        }

        let mut tiles = fragments.into_iter().collect::<Vec<_>>();
        tiles.sort_unstable_by_key(|&(tx, ty)| (ty, tx));
        Some(if tiles.is_empty() {
            vec![(x, y)]
        } else {
            tiles
        })
    }
}
