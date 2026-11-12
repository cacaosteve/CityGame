//! Player zone painting, drag pricing, and user-sign allocation and placement.
//!
//! This groups tool input state with its preview and commit workers. Growth gates,
//! native PlaceTile, and the sign text dialog stay with their existing owners.

use super::tool_controller::{ToolPlacement, ToolPreview, tool_build_cost};
use super::{
    PlacementSound, SMALL_PARK_BUILDING_ID, ViewerState, ZONE_AIRPORT, ZONE_COMMERCIAL_DENSE,
    ZONE_COMMERCIAL_LIGHT, ZONE_INDUSTRIAL_DENSE, ZONE_INDUSTRIAL_LIGHT, ZONE_MILITARY,
    ZONE_RESIDENTIAL_DENSE, ZONE_RESIDENTIAL_LIGHT, ZONE_SEAPORT, pe_place_tile_441f00,
};
use crate::city::{
    CITY_HEIGHT, CITY_WIDTH, MAX_USER_TEXT_ENTRIES, MIN_USER_TEXT_ENTRIES, TileBits,
    ZONE_CORNER_ALL,
};
use crate::engine_fidelity::EngineFidelity;
use crate::ui::EditTool;
use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZoneDrag {
    pub(super) start: (usize, usize),
    pub(super) current: (usize, usize),
    pub(super) zone_id: u8,
}

impl ViewerState {
    pub(super) fn begin_zone_drag(&mut self, x: usize, y: usize, zone_id: u8) {
        self.zone_drag = Some(ZoneDrag {
            start: (x, y),
            current: (x, y),
            zone_id,
        });
    }

    pub(super) fn update_zone_drag_at(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        if let Some(tile) = self.tile_at_screen(screen, window_size, pixel_scale) {
            self.update_zone_drag_to(tile);
        }
    }

    pub(super) fn update_zone_drag_to(&mut self, tile: (usize, usize)) {
        if let Some(drag) = self.zone_drag.as_mut() {
            drag.current = tile;
        }
    }

    pub(super) fn finish_zone_drag(&mut self) -> usize {
        if let Some(drag) = self.zone_drag.take() {
            self.place_zone_rect(drag.start, drag.current, drag.zone_id)
        } else {
            0
        }
    }

    pub(super) fn cancel_zone_drag(&mut self) {
        self.zone_drag = None;
    }

    pub(super) fn zone_drag_active(&self) -> bool {
        self.zone_drag.is_some()
    }

    pub(super) fn zone_drag_cost(&self) -> Option<u32> {
        let drag = self.zone_drag?;
        if self.mac_zone_start_rejected(drag.start) {
            return None;
        }
        let billable_tiles =
            self.zone_drag_billable_tile_count(drag.start, drag.current, drag.zone_id);
        Some(
            self.zone_unit_cost(drag.zone_id)
                .saturating_mul(billable_tiles as u32),
        )
    }

    pub(super) fn zone_unit_cost(&self, zone_id: u8) -> u32 {
        if self.engine_fidelity == EngineFidelity::Mac68k && zone_id == 0 {
            // CODE 13 selects the original tool-0/state-4 price WORD.
            1
        } else {
            zone_build_cost(zone_id)
        }
    }

    fn mac_zone_start_rejected(&self, (x, y): (usize, usize)) -> bool {
        // CODE 13 checks water before entering the drag and rejects
        // radioactivity or a military zone in its initial-cell helper.
        self.engine_fidelity == EngineFidelity::Mac68k
            && (self
                .city
                .tile_bits_at(x, y)
                .is_some_and(TileBits::water_tile)
                || self.city.building_at(x, y) == 5
                || self
                    .city
                    .zone_at(x, y)
                    .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY))
    }

    pub(super) fn sign_placement_at(&self, x: usize, y: usize) -> ToolPlacement {
        if self
            .user_sign_id_for_text_overlay(self.city.text_overlay_at(x, y).unwrap_or(0))
            .is_some()
        {
            ToolPlacement::valid(ToolPreview::Zone(0), vec![(x, y)])
        } else {
            ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview: ToolPreview::Zone(0),
            }
        }
    }

    pub(super) fn place_sign_at(&mut self, x: usize, y: usize) -> bool {
        let current = self.city.text_overlay_at(x, y).unwrap_or(0);
        let Some(sign_id) = self.user_sign_id_for_text_overlay(current) else {
            return false;
        };
        self.city.set_text_overlay(x, y, sign_id);
        if self.city.label_at(sign_id).is_none_or(str::is_empty) {
            self.city.set_label(sign_id, "Sign");
        }
        // PE sign tool: XTXT/XLAB only — leave watered/piped/XTRF alone.
        self.open_sign_dialog(sign_id);
        self.query_is_inspect = false;
        self.refresh_status_line();
        true
    }

    fn user_sign_id_for_text_overlay(&self, text_overlay: u8) -> Option<u8> {
        if is_user_sign(text_overlay) {
            Some(text_overlay)
        } else if text_overlay == 0 {
            self.first_free_user_sign_id()
        } else {
            None
        }
    }

    fn first_free_user_sign_id(&self) -> Option<u8> {
        let mut used = [false; 0x33];
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                let Some(sign_id) = self.city.text_overlay_at(x, y) else {
                    continue;
                };
                if is_user_sign(sign_id) {
                    used[usize::from(sign_id)] = true;
                }
            }
        }
        (MIN_USER_TEXT_ENTRIES..=MAX_USER_TEXT_ENTRIES).find(|&sign_id| !used[usize::from(sign_id)])
    }

    pub(super) fn zone_placement_for_rect(
        &self,
        start: (usize, usize),
        end: (usize, usize),
        zone_id: u8,
    ) -> ToolPlacement {
        let mut valid_tiles = Vec::new();
        let mut invalid_tiles = Vec::new();
        for (x, y) in tile_rectangle(start, end) {
            if self.can_zone_tile(x, y) {
                valid_tiles.push((x, y));
            } else {
                invalid_tiles.push((x, y));
            }
        }
        ToolPlacement {
            valid_tiles,
            invalid_tiles,
            preview: ToolPreview::Zone(zone_id),
        }
    }

    pub(super) fn place_zone_at(&mut self, x: usize, y: usize, zone_id: u8) -> usize {
        self.place_zone_rect((x, y), (x, y), zone_id)
    }

    pub(super) fn place_zone_rect(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
        zone_id: u8,
    ) -> usize {
        let mac = self.engine_fidelity == EngineFidelity::Mac68k;
        if self.mac_zone_start_rejected(start) {
            return 0;
        }

        let total_cost = self
            .zone_unit_cost(zone_id)
            .saturating_mul(self.zone_drag_billable_tile_count(start, end, zone_id) as u32);
        if total_cost == 0 || !self.can_afford_build_cost(total_cost) {
            // PE FUN_004495F0 and Mac CODE 13 export 181 both refuse a zero
            // price and reject the complete drag when its full price exceeds
            // available funds.
            return 0;
        }
        // Both executable paths debit the preview price before their commit
        // scan. Water cells can therefore be priced candidates that the later
        // XBIT-water refusal leaves unwritten.
        self.debit_build_cost(total_cost);

        let mut placed = 0;
        let mut placed_tiles = Vec::new();
        for (x, y) in tile_rectangle(start, end) {
            if self.can_zone_tile(x, y) {
                let building = self.city.building_at(x, y);
                if zone_id == 0 && (1..=4).contains(&building) {
                    if mac {
                        self.city.set_building(x, y, 0);
                    } else {
                        pe_place_tile_441f00(&mut self.city, x, y, 0);
                    }
                }
                // Mac replaces the low nibble for paint and De-Zone.
                // Win95's De-Zone skips the nibble write.
                if mac || zone_id != 0 {
                    let corners = self
                        .city
                        .zone_at(x, y)
                        .map(|zone| zone.raw_bits() & 0xf0)
                        .unwrap_or(0);
                    // Mac export 181 preserves the upper nibble verbatim.
                    let zone_bits = if !mac && corners == 0 {
                        zone_id | ZONE_CORNER_ALL
                    } else {
                        zone_id | corners
                    };
                    self.city.set_zone(x, y, zone_bits);
                }
                placed_tiles.push((x, y));
                placed += 1;
            }
        }
        if placed > 0 {
            if zone_id != 0 {
                self.enable_zone_growth_for_player_tiles(&placed_tiles);
            } else {
                self.clear_zone_growth_for_player_tiles(&placed_tiles);
            }
            if !mac {
                self.refresh_live_misc_current_values();
                self.refresh_budget_service_rows_from_live_counts();
                self.refresh_reward_availability();
            }
            self.query_is_inspect = false;
            self.refresh_status_line();
        }
        if placed == 0 {
            self.refresh_status_line();
        }
        placed
    }

    fn can_zone_tile(&self, x: usize, y: usize) -> bool {
        // PE zone paint @ `0x4499c6`: require XTER == 0.
        if self.city.terrain_at(x, y) != 0 {
            return false;
        }
        // PE zone paint @ `0x449997`: refuse XBIT water-tile `0x04`.
        if self
            .city
            .tile_bits_at(x, y)
            .is_some_and(TileBits::water_tile)
        {
            return false;
        }
        // PE @ `0x4499f3`: refuse existing military zone type nibble.
        if self
            .city
            .zone_at(x, y)
            .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
        {
            return false;
        }
        let building = self.city.building_at(x, y);
        // PE @ `0x4499df` / `0x4499e1`: refuse radioactivity `5` and small park `0x0D`.
        if building == 5 || building == SMALL_PARK_BUILDING_ID {
            return false;
        }
        // PE @ `0x4499d5`–`0x4499e7`: XBLD must be `< 0x1D` (roads start at `0x1D`).
        // Power lines `0x0e..=0x1c`, trees, rubble, and vacant all pass; paint is XZON-only.
        building < 0x1d
    }

    fn zone_tile_changes_type(&self, x: usize, y: usize, zone_id: u8) -> bool {
        self.city
            .zone_at(x, y)
            .map(|zone| zone.zone_id())
            .unwrap_or(0)
            != zone_id
    }

    fn zone_drag_billable_tile_count(
        &self,
        start: (usize, usize),
        end: (usize, usize),
        zone_id: u8,
    ) -> usize {
        // This is the shared price-preview predicate recovered from Win95
        // FUN_00448380 and Mac CODE 13 FUN_000225de. Neither preview excludes
        // XBIT water: the later commit scan does, after charging the drag.
        tile_rectangle(start, end)
            .into_iter()
            .filter(|&(x, y)| {
                let building = self.city.building_at(x, y);
                self.city.terrain_at(x, y) == 0
                    && building < 0x1d
                    && building != 5
                    && building != SMALL_PARK_BUILDING_ID
                    && self
                        .city
                        .zone_at(x, y)
                        .is_none_or(|zone| zone.zone_id() != ZONE_MILITARY)
                    && self.zone_tile_changes_type(x, y, zone_id)
            })
            .count()
    }
}

pub(super) fn zone_placement_sound(tool: EditTool) -> PlacementSound {
    match tool {
        EditTool::ZoneResidential
        | EditTool::ZoneResidentialDense
        | EditTool::ZoneCommercial
        | EditTool::ZoneCommercialDense
        | EditTool::ZoneIndustrial
        | EditTool::ZoneIndustrialDense => PlacementSound::Plop,
        _ => PlacementSound::Build,
    }
}

pub(super) fn zone_id_for_tool(tool: EditTool) -> Option<u8> {
    match tool {
        EditTool::ZoneResidential => Some(ZONE_RESIDENTIAL_LIGHT),
        EditTool::ZoneResidentialDense => Some(ZONE_RESIDENTIAL_DENSE),
        EditTool::ZoneCommercial => Some(ZONE_COMMERCIAL_LIGHT),
        EditTool::ZoneCommercialDense => Some(ZONE_COMMERCIAL_DENSE),
        EditTool::ZoneIndustrial => Some(ZONE_INDUSTRIAL_LIGHT),
        EditTool::ZoneIndustrialDense => Some(ZONE_INDUSTRIAL_DENSE),
        EditTool::Airport => Some(ZONE_AIRPORT),
        EditTool::Seaport => Some(ZONE_SEAPORT),
        EditTool::DeZone => Some(0),
        _ => None,
    }
}

pub(super) fn zone_build_cost(zone_id: u8) -> u32 {
    match zone_id {
        ZONE_AIRPORT => tool_build_cost(EditTool::Airport).unwrap_or(250),
        ZONE_SEAPORT => tool_build_cost(EditTool::Seaport).unwrap_or(150),
        ZONE_RESIDENTIAL_DENSE | ZONE_COMMERCIAL_DENSE | ZONE_INDUSTRIAL_DENSE => 10,
        _ => 5,
    }
}

pub(super) fn tile_rectangle(start: (usize, usize), end: (usize, usize)) -> Vec<(usize, usize)> {
    let min_x = start.0.min(end.0).min(CITY_WIDTH - 1);
    let min_y = start.1.min(end.1).min(CITY_HEIGHT - 1);
    let max_x = start.0.max(end.0).min(CITY_WIDTH - 1);
    let max_y = start.1.max(end.1).min(CITY_HEIGHT - 1);
    let mut tiles = Vec::with_capacity((max_x - min_x + 1) * (max_y - min_y + 1));
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            tiles.push((x, y));
        }
    }
    tiles
}

pub(super) fn is_user_sign(text_overlay: u8) -> bool {
    (MIN_USER_TEXT_ENTRIES..=MAX_USER_TEXT_ENTRIES).contains(&text_overlay)
}
