//! Pipe and subway strokes, reshaping and previews, plus tunnel construction.
//!
//! Existing pricing, commit order, RNG use, and city-plane writes are preserved.
//! Water-service simulation, demolition, and general map rendering remain in
//! their existing modules.

use super::network::pe_reshape::pe_network_terrain_neighbor_mask;
use super::network::{mac_connector_slope_offset, mac_connector_terrain_open};
use super::{
    BuildablePrefixPath, CityInfoDialog, DEPTH_ZONE, NETWORK_BOTTOM, NETWORK_LEFT, NETWORK_RIGHT,
    NETWORK_TOP, NetworkAxis, PE_NETWORK_SURFACE_REMAP_GATE_4E7B68, PlacementSound, ToolPlacement,
    ToolPreview, UndergroundRenderTile, UndergroundTileSource, ViewerState, ZONE_MILITARY,
    directional_neighbor_tiles, neighbor_tiles, network_axis_mask_for_side_mask, network_drag_path,
    network_path_axis_mask, network_side_axis, network_single_axis, pe_network_gate_connects_side,
    pe_network_surface_edit_448e20, pe_network_surface_stroke_cost_448a90,
    pe_pipetool_post_place_water_recalc_43f419, pe_place_tile_441f00, power_line_drag_path,
    push_underground_tile_sprite, rotated_underground_tile, sprite_depth, stamp_xbit_piped,
    underground_building_pipe_tile, water_source_connects_as_building_pipe,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City, TileBits};
use crate::renderer::{RenderSprite, Renderer};
use crate::ui::EditTool;
use glam::Vec2;
use std::collections::HashSet;

/// PE PlacePipes reshape `0x44b8d0` XTXT early arm @ `0x44b945`–`0x44b961`:
/// when XUND in `0x10..0x1f` and XTXT low-nibble gate `0x4e7b68` is set, PlaceXUND
/// (`0x4e7c60[nibble] + 0x10`) and return (skip neighbor-mask reshape).
pub(super) const PE_PLACE_PIPES_XTXT_GATE: [u32; 16] =
    [0, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0];

pub(super) const PE_PLACE_PIPES_XTXT_TILE: [u16; 16] = [
    0, 2, 3, 4, 5, 0, 0, 0, 1, 1, 0x101, 0x101, 0x101, 0x101, 0x101, 1,
];

pub(super) fn pe_place_pipes_xtxt_reshape_44b961(city: &mut City, x: usize, y: usize) -> bool {
    let und = city.underground_at(x, y);
    if !(0x10..0x1f).contains(&und) {
        return false;
    }
    let xtxt = city.text_overlay_at(x, y).unwrap_or(0);
    let idx = if xtxt > 0x30 {
        0
    } else {
        usize::from(xtxt & 0x0f)
    };
    if PE_PLACE_PIPES_XTXT_GATE[idx] == 0 {
        return false;
    }
    let tile = (PE_PLACE_PIPES_XTXT_TILE[idx].wrapping_add(0x10) & 0xff) as u8;
    pe_place_xund_442370(city, x, y, tile);
    true
}

/// PE PlacePipes reshape mask→offset table `0x4e7c40` (@ `0x44ba8b`); PlaceXUND adds `0x10`
/// (`0x44ba9a`). Mask 0 forces index `0xf`. Bit layout matches Rust NETWORK_* (N=1,E=2,S=4,W=8).
pub(super) const PE_PLACE_PIPES_MASK_TILE: [u16; 16] =
    [0, 0, 1, 6, 0, 0, 7, 0xb, 1, 9, 1, 0xa, 8, 0xd, 0xc, 0xe];

pub(super) fn pe_place_pipes_mask_tile_44ba9a(mask: u8) -> u8 {
    let idx = if mask == 0 {
        0x0f
    } else {
        usize::from(mask & 0x0f)
    };
    (PE_PLACE_PIPES_MASK_TILE[idx].wrapping_add(0x10) & 0xff) as u8
}

/// PE subway reshape `0x44bc40` XTXT early arm @ `0x44bcb8`–`0x44be07`:
/// XUND in `0x01..0x10` + XTXT gate → PlaceXUND(`0x4e7c60[nibble] + 1`).
pub(super) fn pe_subway_xtxt_reshape_44bcc5(city: &mut City, x: usize, y: usize) -> bool {
    let und = city.underground_at(x, y);
    if !(0x01..0x10).contains(&und) {
        return false;
    }
    let xtxt = city.text_overlay_at(x, y).unwrap_or(0);
    let idx = if xtxt > 0x30 {
        0
    } else {
        usize::from(xtxt & 0x0f)
    };
    if PE_PLACE_PIPES_XTXT_GATE[idx] == 0 {
        return false;
    }
    let tile = (PE_PLACE_PIPES_XTXT_TILE[idx].wrapping_add(1) & 0xff) as u8;
    pe_place_xund_442370(city, x, y, tile);
    true
}

/// PE subway reshape mask→tile @ `0x44bdff`–`0x44be07`: `0x4e7c40[mask] + 1`
/// (no pipes-style mask0→0xf force).
pub(super) fn pe_subway_mask_tile_44be07(mask: u8) -> u8 {
    let idx = usize::from(mask & 0x0f);
    (PE_PLACE_PIPES_MASK_TILE[idx].wrapping_add(1) & 0xff) as u8
}

/// PE PlacePipes/subway/power reshape NS connect gate `0x4e7cb8` (indexed by XUND/XBLD).
pub(super) const PE_XUND_NS_GATE_4E7CB8: [u8; 128] = [
    1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
    1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// PE PlacePipes/subway/power reshape EW connect gate `0x4e7c70` (indexed by XUND/XBLD).
pub(super) const PE_XUND_EW_GATE_4E7C70: [u8; 128] = [
    1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
    1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1,
];

pub(super) fn pe_xund_gate_connects_side(tile: u8, side: u8) -> bool {
    pe_network_gate_connects_side(tile, side)
}

/// PE subway reshape neighbor accept @ `0x44bce9`–`0x44bd25`: XUND `0x01..0x10` or
/// `0x1f`/`0x20`/`0x22`/`0x23`, then NS gate `0x4e7cb8` / EW gate `0x4e7c70`.
pub(super) fn pe_subway_reshape_neighbor_connects_44bcf6(tile: u8, side: u8) -> bool {
    let in_range = (0x01..0x10).contains(&tile) || matches!(tile, 0x1f | 0x20 | 0x22 | 0x23);
    in_range && pe_network_gate_connects_side(tile, side)
}

/// PE PlacePipes reshape neighbor accept @ `0x44b987`–`0x44ba73`: XUND `0x10..0x1f`
/// or `0x1f`/`0x20`, then same NS/EW gates as subway.
pub(super) fn pe_place_pipes_reshape_neighbor_connects_44b98a(tile: u8, side: u8) -> bool {
    let in_range = (0x10..0x1f).contains(&tile) || matches!(tile, 0x1f | 0x20);
    in_range && pe_network_gate_connects_side(tile, side)
}

/// PE PlacePipesAtCoordinates existing→crossover @ `0x44b7e3`–`0x44b7fd`:
/// subway `1`→`0x1f`, `2`→`0x20` (vacant `0`→`0x10` is base place, not crossover).
pub(super) fn pe_place_pipes_crossover_from_existing_44b7f5(existing: u8) -> Option<u8> {
    match existing {
        1 => Some(UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR),
        2 => Some(UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB),
        _ => None,
    }
}

/// PE PlaceSubway existing→crossover @ `0x44bb7e`–`0x44bb96`:
/// pipes `0x10`→`0x20`, `0x11`→`0x1f` (vacant `0`→`0x01` is base place, not crossover).
pub(super) fn pe_place_subway_crossover_from_existing_44bb8e(existing: u8) -> Option<u8> {
    match existing {
        0x10 => Some(UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB),
        0x11 => Some(UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR),
        _ => None,
    }
}

pub(super) fn pe_place_underground_crossover_from_existing(
    kind: UndergroundKind,
    existing: u8,
) -> Option<u8> {
    match kind {
        UndergroundKind::Pipes => pe_place_pipes_crossover_from_existing_44b7f5(existing),
        UndergroundKind::Subway => pe_place_subway_crossover_from_existing_44bb8e(existing),
    }
}

/// PE PlacePipesAtCoordinates early ret @ `0x44b7ba`–`0x44b7d8`: already pipes or crossover.
pub(super) fn pe_place_pipes_at_coordinates_noop_44b7b8(existing: u8) -> bool {
    (0x10..0x1f).contains(&existing) || matches!(existing, 0x1f | 0x20)
}

/// PE PlacePipesAtCoordinates refuse @ `0x44b7f0`: not vacant, not subway 1/2, not noop.
pub(super) fn pe_place_pipes_at_coordinates_refuse_44b7f0(existing: u8) -> bool {
    existing != 0
        && !matches!(existing, 1 | 2)
        && !pe_place_pipes_at_coordinates_noop_44b7b8(existing)
}

/// PE PlaceSubway early ret @ `0x44bb41`–`0x44bb73`: already subway / crossover / station box.
pub(super) fn pe_place_subway_at_coordinates_noop_44bb40(existing: u8) -> bool {
    (0x01..0x10).contains(&existing) || matches!(existing, 0x1f | 0x20 | 0x22 | 0x23)
}

/// Win95 PlaceXUND `0x442370` (thunk `0x401e38`): write XUND tile.
/// Non-military adjusts subway occupancy word `0x4ca41c` (@ `0x4423a0`–`0x4423fb`):
/// old/new in `[1,0x10) ∪ {0x1f,0x20,0x22,0x23}` → dec/inc. Military XZON type 7
/// still writes (@ `0x442407`) but skips the counter.
pub(super) fn pe_place_xund_442370(city: &mut City, x: usize, y: usize, tile: u8) {
    pe_place_xund_word_442370(city, x, y, u16::from(tile));
}

fn pe_place_xund_word_442370(city: &mut City, x: usize, y: usize, tile: u16) {
    let military = city.zone_at(x, y).is_some_and(|z| z.zone_id() == 7);
    if !military && !city.mac_cross_layout_active {
        let old = city.underground_at(x, y);
        if pe_place_subway_at_coordinates_noop_44bb40(old) {
            city.pe_xund_occupancy = city.pe_xund_occupancy.wrapping_sub(1);
        }
        // PlaceXUND tests the full input word for census membership before storing its low byte.
        if u8::try_from(tile).is_ok_and(pe_place_subway_at_coordinates_noop_44bb40) {
            city.pe_xund_occupancy = city.pe_xund_occupancy.wrapping_add(1);
        }
    }
    city.set_underground(x, y, tile as u8);
    if city.pe_cross_layout_active {
        // The Mac loader maps its selected-XUND counter to this same MISC
        // dword, but Mac counts military-zone cells while PE skips them.
        // Keep the selected PE engine's divergent count in CGEX so the Mac
        // engine's native counter written by set_underground remains intact.
        city.pe_cross_layout_saved_xund_occupancy = Some(city.pe_xund_occupancy);
    }
}

/// Win95 Pipe and Subway reshape workers at 0x44b8d0 and 0x44bc40.
pub(in crate::app) fn pe_reshape_underground_tile(
    city: &mut City,
    x: usize,
    y: usize,
    kind: UndergroundKind,
) {
    if !kind.contains(city.underground_at(x, y)) {
        return;
    }
    let xter = city.terrain_at(x, y);
    let grade = if xter <= 0x30 {
        usize::from(xter & 0x0f)
    } else {
        0
    };
    let tile = if PE_NETWORK_SURFACE_REMAP_GATE_4E7B68[grade] != 0 {
        PE_PLACE_PIPES_XTXT_TILE[grade] + u16::from(kind.base_tile())
    } else {
        let mask = pe_network_terrain_neighbor_mask(city, x, y, |nx, ny| {
            let neighbor = city.underground_at(nx, ny);
            match kind {
                UndergroundKind::Pipes => (0x10..=0x20).contains(&neighbor),
                UndergroundKind::Subway => pe_place_subway_at_coordinates_noop_44bb40(neighbor),
            }
        });
        u16::from(match kind {
            UndergroundKind::Pipes => pe_place_pipes_mask_tile_44ba9a(mask),
            UndergroundKind::Subway => pe_subway_mask_tile_44be07(mask),
        })
    };
    pe_place_xund_word_442370(city, x, y, tile);
}

/// PE PlaceSubway refuse @ `0x44bb8b`: not vacant, not pipes 0x10/0x11, not noop.
pub(super) fn pe_place_subway_at_coordinates_refuse_44bb8b(existing: u8) -> bool {
    existing != 0
        && !matches!(existing, 0x10 | 0x11)
        && !pe_place_subway_at_coordinates_noop_44bb40(existing)
}

pub(super) fn pe_place_underground_at_coordinates_noop(
    kind: UndergroundKind,
    existing: u8,
) -> bool {
    match kind {
        UndergroundKind::Pipes => pe_place_pipes_at_coordinates_noop_44b7b8(existing),
        UndergroundKind::Subway => pe_place_subway_at_coordinates_noop_44bb40(existing),
    }
}

pub(super) fn pe_place_underground_at_coordinates_refuse(
    kind: UndergroundKind,
    existing: u8,
) -> bool {
    match kind {
        UndergroundKind::Pipes => pe_place_pipes_at_coordinates_refuse_44b7f0(existing),
        UndergroundKind::Subway => pe_place_subway_at_coordinates_refuse_44bb8b(existing),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UndergroundKind {
    Subway,
    Pipes,
}

impl UndergroundKind {
    pub(super) fn base_tile(self) -> u8 {
        match self {
            Self::Subway => 1,
            Self::Pipes => 16,
        }
    }

    pub(super) fn contains(self, tile: u8) -> bool {
        match self {
            Self::Subway => matches!(tile, 1..=15),
            Self::Pipes => matches!(tile, 16..=30),
        }
    }

    pub(super) fn cost_per_tile(self) -> u32 {
        match self {
            Self::Subway => 100,
            Self::Pipes => 3,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Subway => "Subway",
            Self::Pipes => "Pipes",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TunnelPlan {
    pub(super) tiles: Vec<(usize, usize)>,
    pub(super) entrance_building: u8,
    pub(super) exit_building: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct UndergroundDrag {
    pub(super) start: (usize, usize),
    pub(super) current: (usize, usize),
    pub(super) kind: UndergroundKind,
}

pub(super) fn underground_kind_for_tool(tool: EditTool) -> Option<UndergroundKind> {
    match tool {
        EditTool::Pipes => Some(UndergroundKind::Pipes),
        EditTool::Subway => Some(UndergroundKind::Subway),
        _ => None,
    }
}

pub(super) fn underground_placement_sound(kind: UndergroundKind) -> PlacementSound {
    match kind {
        UndergroundKind::Pipes => PlacementSound::Pipe,
        UndergroundKind::Subway => PlacementSound::Build,
    }
}

pub(super) const UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR: u8 = 31;

pub(super) const UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB: u8 = 32;

pub(super) const UNDER_TILE_PIPES_LR: u8 = 16;

pub(super) const UNDER_TILE_PIPES_TB: u8 = 17;

pub(super) const UNDER_TILE_SUBWAY_ENTRANCE: u8 = 35;

pub(super) const UNDER_TILE_BUILDING_PIPE_TEE: u8 = 0x1c;

// Win95 FUN_00442370 counts XUND 31, 32, 34, and 35 with subway underground
// occupancy, while XUND 33 is the distinct building-pipe sprite family.
pub(super) const UNDER_TILE_BUILDING_PIPES: u8 = 33;

pub(super) const UNDER_TILE_GREY_BOX_1: u8 = 34;

pub(super) const UNDER_TILE_GREY_BOX_2: u8 = 35;

pub(super) fn underground_crossover_axis(tile: u8, kind: UndergroundKind) -> Option<NetworkAxis> {
    match (tile, kind) {
        (UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR, UndergroundKind::Pipes) => {
            Some(NetworkAxis::GridX)
        }
        (UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR, UndergroundKind::Subway) => {
            Some(NetworkAxis::GridY)
        }
        (UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB, UndergroundKind::Pipes) => {
            Some(NetworkAxis::GridY)
        }
        (UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB, UndergroundKind::Subway) => {
            Some(NetworkAxis::GridX)
        }
        _ => None,
    }
}

pub(super) fn underground_crossing_tile_for(
    kind: UndergroundKind,
    axis: NetworkAxis,
    existing: u8,
) -> Option<u8> {
    for existing_kind in [UndergroundKind::Subway, UndergroundKind::Pipes] {
        if existing_kind == kind {
            continue;
        }
        let Some(existing_axis) = straight_underground_axis(existing, existing_kind) else {
            continue;
        };
        if existing_axis == axis {
            return None;
        }
        return match ((kind, axis), (existing_kind, existing_axis)) {
            (
                (UndergroundKind::Pipes, NetworkAxis::GridX),
                (UndergroundKind::Subway, NetworkAxis::GridY),
            )
            | (
                (UndergroundKind::Subway, NetworkAxis::GridY),
                (UndergroundKind::Pipes, NetworkAxis::GridX),
            ) => Some(UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR),
            (
                (UndergroundKind::Pipes, NetworkAxis::GridY),
                (UndergroundKind::Subway, NetworkAxis::GridX),
            )
            | (
                (UndergroundKind::Subway, NetworkAxis::GridX),
                (UndergroundKind::Pipes, NetworkAxis::GridY),
            ) => Some(UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB),
            _ => None,
        };
    }
    None
}

pub(super) fn inferred_single_tile_underground_crossing_axis(
    kind: UndergroundKind,
    existing: u8,
) -> Option<NetworkAxis> {
    for existing_kind in [UndergroundKind::Subway, UndergroundKind::Pipes] {
        if existing_kind == kind {
            continue;
        }
        if let Some(existing_axis) = straight_underground_axis(existing, existing_kind) {
            return Some(existing_axis.perpendicular());
        }
    }
    None
}

pub(super) fn straight_underground_axis(tile: u8, kind: UndergroundKind) -> Option<NetworkAxis> {
    match (kind, tile) {
        (UndergroundKind::Subway, 1) | (UndergroundKind::Pipes, 16) => Some(NetworkAxis::GridY),
        (UndergroundKind::Subway, 2) | (UndergroundKind::Pipes, 17) => Some(NetworkAxis::GridX),
        _ => None,
    }
}

pub(super) fn underground_tile_connects_to(tile: u8, kind: UndergroundKind, side: u8) -> bool {
    // PE subway reshape neighbor @ `0x44bcf6`..`0x44bdf0`: range then NS/EW gates.
    if kind == UndergroundKind::Subway {
        return pe_subway_reshape_neighbor_connects_44bcf6(tile, side);
    }
    if let Some(axis) = underground_crossover_axis(tile, kind) {
        return axis == network_side_axis(side);
    }
    underground_tile_side_mask(tile, kind).is_some_and(|mask| mask & side != 0)
}

pub(super) fn underground_tile_side_mask(tile: u8, kind: UndergroundKind) -> Option<u8> {
    if !kind.contains(tile) {
        return None;
    }
    let offset = tile - kind.base_tile();
    let mask = match offset {
        0 => NETWORK_TOP | NETWORK_BOTTOM,
        1 => NETWORK_LEFT | NETWORK_RIGHT,
        2 | 4 => NETWORK_TOP | NETWORK_BOTTOM,
        3 | 5 => NETWORK_LEFT | NETWORK_RIGHT,
        6 => NETWORK_TOP | NETWORK_RIGHT,
        7 => NETWORK_RIGHT | NETWORK_BOTTOM,
        8 => NETWORK_BOTTOM | NETWORK_LEFT,
        9 => NETWORK_LEFT | NETWORK_TOP,
        10 => NETWORK_LEFT | NETWORK_TOP | NETWORK_RIGHT,
        11 => NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM,
        12 => NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT,
        13 => NETWORK_BOTTOM | NETWORK_LEFT | NETWORK_TOP,
        14 => NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT,
        _ => return None,
    };
    Some(mask)
}

pub(super) fn underground_tile_for_mask(kind: UndergroundKind, mask: u8) -> u8 {
    let offset = match mask {
        m if m == (NETWORK_TOP | NETWORK_BOTTOM) => 0,
        m if m == (NETWORK_LEFT | NETWORK_RIGHT) => 1,
        m if m == (NETWORK_TOP | NETWORK_RIGHT) => 6,
        m if m == (NETWORK_RIGHT | NETWORK_BOTTOM) => 7,
        m if m == (NETWORK_BOTTOM | NETWORK_LEFT) => 8,
        m if m == (NETWORK_LEFT | NETWORK_TOP) => 9,
        m if m == (NETWORK_LEFT | NETWORK_TOP | NETWORK_RIGHT) => 10,
        m if m == (NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM) => 11,
        m if m == (NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT) => 12,
        m if m == (NETWORK_BOTTOM | NETWORK_LEFT | NETWORK_TOP) => 13,
        m if m == (NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT) => 14,
        m if m == NETWORK_TOP || m == NETWORK_BOTTOM => 0,
        m if m == NETWORK_LEFT || m == NETWORK_RIGHT => 1,
        _ => 0,
    };
    kind.base_tile() + offset
}

pub(super) fn underground_preview_path_for_kind(
    start: (usize, usize),
    end: (usize, usize),
    kind: UndergroundKind,
) -> Vec<(usize, usize)> {
    match kind {
        UndergroundKind::Pipes => power_line_drag_path(start, end),
        UndergroundKind::Subway => power_line_drag_path(start, end),
    }
}

pub(super) fn underground_commit_path_for_kind(
    start: (usize, usize),
    end: (usize, usize),
    kind: UndergroundKind,
) -> Vec<(usize, usize)> {
    match kind {
        UndergroundKind::Pipes => power_line_drag_path(start, end),
        UndergroundKind::Subway => network_drag_path(start, end),
    }
}

pub(super) fn pe_pipe_direction_for_path_tile(path: &[(usize, usize)], index: usize) -> u8 {
    let Some(&current) = path.get(index) else {
        return 2;
    };
    let (from, to) = if let Some(&next) = path.get(index + 1) {
        (current, next)
    } else if let Some(&previous) = index.checked_sub(1).and_then(|i| path.get(i)) {
        (previous, current)
    } else {
        (current, current)
    };
    let dx = to.0 as isize - from.0 as isize;
    let dy = to.1 as isize - from.1 as isize;
    if dy.unsigned_abs() < dx.unsigned_abs() {
        if dx >= 0 { 1 } else { 3 }
    } else if dy < 0 {
        0
    } else {
        2
    }
}

/// Win95 pipe-stroke step vectors from `0x4E7B18`/`0x4E7B20` (N, E, S, W).
pub(super) const PE_PIPE_WALK_DIRECTIONS_4E7B18: [(isize, isize); 4] =
    [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// `FUN_00448F50`'s direction-dependent XTER step gates from `0x4E7BE8`.
pub(super) const PE_PIPE_XTER_STEP_GATES_4E7BE8: [[bool; 4]; 16] = [
    [false, false, false, false],
    [true, false, true, false],
    [false, true, false, true],
    [true, false, true, false],
    [false, true, false, true],
    [false, false, false, false],
    [false, false, false, false],
    [false, false, false, false],
    [false, false, false, false],
    [false, false, false, false],
    [false, false, false, false],
    [false, false, false, false],
    [false, false, false, false],
    [false, false, false, false],
    [false, false, false, false],
    [false, false, false, false],
];

/// Terrain-height adjustment table read by the PE pipe walk at `0x4E7BA8`.
pub(super) const PE_PIPE_XTER_SLOPE_ADJUST_4E7BA8: [i16; 16] =
    [0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];

#[derive(Clone, Copy, Debug)]
pub(super) struct PePipeStrokeWalk {
    pub(super) target: (usize, usize),
    pub(super) bounds: (usize, usize, usize, usize),
    pub(super) direction: u8,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PePipeStrokeStep {
    pub(super) next: Option<(usize, usize)>,
    pub(super) direction: u8,
}

pub(super) fn pe_pipe_direction_to_target_44c680(
    current: (usize, usize),
    target: (usize, usize),
) -> u8 {
    let dx = target.0 as isize - current.0 as isize;
    let dy = target.1 as isize - current.1 as isize;
    if dy.unsigned_abs() < dx.unsigned_abs() {
        if dx >= 0 { 1 } else { 3 }
    } else if dy < 0 {
        0
    } else {
        2
    }
}

/// PE `FUN_0044C6F0`: use the other axis when both target deltas are nonzero.
pub(super) fn pe_pipe_alternate_direction_44c6f0(
    current: (usize, usize),
    target: (usize, usize),
) -> u8 {
    let dx = target.0 as isize - current.0 as isize;
    let dy = target.1 as isize - current.1 as isize;
    let primary = pe_pipe_direction_to_target_44c680(current, target);
    if dx == 0 || dy == 0 {
        return primary;
    }
    if primary & 1 == 1 {
        if dy < 0 { 0 } else { 2 }
    } else if dx < 0 {
        3
    } else {
        1
    }
}

pub(super) fn pe_pipe_step_coordinate_44f50(
    current: (usize, usize),
    direction: u8,
) -> Option<(usize, usize)> {
    let (dx, dy) = PE_PIPE_WALK_DIRECTIONS_4E7B18[usize::from(direction & 3)];
    let x = current.0.checked_add_signed(dx)?;
    let y = current.1.checked_add_signed(dy)?;
    (x < CITY_WIDTH && y < CITY_HEIGHT).then_some((x, y))
}

pub(super) fn pe_pipe_xter_step_blocked_44f50(
    city: &City,
    tile: (usize, usize),
    direction: u8,
) -> bool {
    let xter = city.terrain_at(tile.0, tile.1);
    xter < 0x40
        && PE_PIPE_XTER_STEP_GATES_4E7BE8[usize::from(xter & 0x0f)][usize::from(direction & 3)]
}

pub(super) fn pe_pipe_tile_slope_blocks_44f50(
    city: &City,
    tile: (usize, usize),
    expected: i16,
) -> bool {
    let xter = city.terrain_at(tile.0, tile.1);
    let land = i16::from(city.altitude_at(tile.0, tile.1) & 0x1f);
    let delta = land - expected;
    PE_PIPE_XTER_SLOPE_ADJUST_4E7BA8[usize::from(xter & 0x0f)] != 0 && (delta == 0 || delta == -2)
        || (xter == 0 && delta == -1)
}

pub(super) fn pe_pipe_inside_stroke_bounds_44f50(
    tile: (usize, usize),
    bounds: (usize, usize, usize, usize),
) -> bool {
    let (min_x, min_y, max_x, max_y) = bounds;
    (min_x..=max_x).contains(&tile.0) && (min_y..=max_y).contains(&tile.1)
}

pub(super) fn pe_pipe_stroke_walk_init_448bc0(
    city: &City,
    start: (usize, usize),
    end: (usize, usize),
) -> Option<PePipeStrokeWalk> {
    if start.0 >= CITY_WIDTH
        || start.1 >= CITY_HEIGHT
        || end.0 >= CITY_WIDTH
        || end.1 >= CITY_HEIGHT
        || city
            .zone_at(start.0, start.1)
            .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
    {
        return None;
    }

    let start_xter = city.terrain_at(start.0, start.1);
    let mut target = end;
    let mut direction = pe_pipe_direction_to_target_44c680(start, end);
    if (1..5).contains(&start_xter) && pe_pipe_xter_step_blocked_44f50(city, start, direction) {
        direction = pe_pipe_alternate_direction_44c6f0(start, end);
        if pe_pipe_xter_step_blocked_44f50(city, start, direction) {
            target = start;
        }
    }

    Some(PePipeStrokeWalk {
        target,
        bounds: (
            start.0.min(target.0),
            start.1.min(target.1),
            start.0.max(target.0),
            start.1.max(target.1),
        ),
        direction,
    })
}

pub(super) fn pe_pipe_next_stroke_tile_448f50(
    city: &City,
    current: (usize, usize),
    target: (usize, usize),
    bounds: (usize, usize, usize, usize),
    direction: u8,
) -> PePipeStrokeStep {
    let stop = |direction| PePipeStrokeStep {
        next: None,
        direction,
    };
    if current == target {
        return stop(direction);
    }

    let xter = city.terrain_at(current.0, current.1);
    let underground = city.underground_at(current.0, current.1);
    let underground_direction_match = underground != 0
        && matches!(
            u16::from(underground) + u16::from(direction & 1),
            0x11 | 0x02
        );
    let follow_existing_direction = underground_direction_match
        || PE_NETWORK_SURFACE_REMAP_GATE_4E7B68[usize::from(xter & 0x0f)] != 0;

    let (next, next_direction) = if follow_existing_direction {
        let Some(next) = pe_pipe_step_coordinate_44f50(current, direction) else {
            return stop(direction);
        };
        if !pe_pipe_inside_stroke_bounds_44f50(next, bounds)
            || pe_pipe_xter_step_blocked_44f50(city, next, direction)
        {
            return stop(direction);
        }
        (next, direction)
    } else {
        let expected_land = i16::from(city.altitude_at(current.0, current.1) & 0x1f)
            + PE_PIPE_XTER_SLOPE_ADJUST_4E7BA8[usize::from(xter & 0x0f)];
        let primary = pe_pipe_direction_to_target_44c680(current, target);
        let Some(primary_tile) = pe_pipe_step_coordinate_44f50(current, primary) else {
            return stop(direction);
        };
        if !pe_pipe_inside_stroke_bounds_44f50(primary_tile, bounds) {
            return stop(direction);
        }

        let try_alternate = pe_pipe_xter_step_blocked_44f50(city, primary_tile, primary)
            || pe_pipe_tile_slope_blocks_44f50(city, primary_tile, expected_land);
        if !try_alternate {
            (primary_tile, primary)
        } else {
            let alternate = pe_pipe_alternate_direction_44c6f0(current, target);
            let Some(alternate_tile) = pe_pipe_step_coordinate_44f50(current, alternate) else {
                return stop(direction);
            };
            if !pe_pipe_inside_stroke_bounds_44f50(alternate_tile, bounds)
                || pe_pipe_xter_step_blocked_44f50(city, alternate_tile, alternate)
                || pe_pipe_tile_slope_blocks_44f50(city, alternate_tile, expected_land)
            {
                return stop(direction);
            }
            (alternate_tile, alternate)
        }
    };

    // PE stores DAT_004c7d60 at 0x449235 before its late tunnel and XUND
    // rejection branches. Mode 4 does not check candidate XZON here.
    let direction = next_direction;
    if matches!(city.tunnel_levels_at(next.0, next.1), 1 | 2) {
        return stop(direction);
    }
    let next_underground = city.underground_at(next.0, next.1);
    if next_underground != 0
        && !matches!(
            u16::from(next_underground) + u16::from(next_direction & 1),
            0x11 | 0x02
        )
    {
        return stop(direction);
    }
    PePipeStrokeStep {
        next: Some(next),
        direction,
    }
}

/// Port of the mode-4 start initialization and candidate walk in
/// `FUN_00448BC0` / `FUN_00448F50`; a refused step leaves the accepted prefix.
pub(super) fn pe_pipe_stroke_path_448bc0_448f50(
    city: &City,
    start: (usize, usize),
    end: (usize, usize),
) -> Vec<(usize, usize)> {
    let Some(walk) = pe_pipe_stroke_walk_init_448bc0(city, start, end) else {
        return Vec::new();
    };
    let mut path = vec![start];
    let mut current = start;
    let mut direction = walk.direction;
    for _ in 0..(CITY_WIDTH + CITY_HEIGHT) {
        let step =
            pe_pipe_next_stroke_tile_448f50(city, current, walk.target, walk.bounds, direction);
        direction = step.direction;
        let Some(next) = step.next else {
            break;
        };
        path.push(next);
        current = next;
    }
    path
}

/// Mac CODE 13 export 191/192 route state for pipes, subways and military connectors.
#[derive(Clone, Copy, Debug)]
pub(super) struct MacRouteWalk {
    military: bool,
    target: (usize, usize),
    bounds: (usize, usize, usize, usize),
    current: (usize, usize),
    direction: u8,
    done: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MacRouteCell {
    pub(super) tile: (usize, usize),
    /// export 192's route orientation after requesting this tile's successor;
    /// export 193 consumes this orientation before the XUND write.
    pub(super) direction: u8,
}

const MAC_PIPE_XTER_CLASS_DELTA: [i16; 16] = [0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 1, 0, 0];

const MAC_PIPE_XTER_PRICE_GATE: [u8; 16] = [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0];

const MAC_PIPE_XTER_CLEAR_COST: u32 = 25;

const MAC_PIPE_XTER_DIRECTIONAL_REMAP: [[u8; 4]; 4] =
    [[2, 1, 2, 1], [2, 3, 2, 3], [4, 3, 4, 3], [4, 1, 4, 1]];

fn mac_pipe_direction_to_target(current: (usize, usize), target: (usize, usize)) -> u8 {
    pe_pipe_direction_to_target_44c680(current, target)
}

fn mac_pipe_alternate_direction(current: (usize, usize), target: (usize, usize)) -> u8 {
    pe_pipe_alternate_direction_44c6f0(current, target)
}

fn mac_pipe_step_coordinate(current: (usize, usize), direction: u8) -> Option<(usize, usize)> {
    pe_pipe_step_coordinate_44f50(current, direction)
}

fn mac_pipe_direction_gate(xter: u8, direction: u8) -> bool {
    xter < 0x40
        && PE_PIPE_XTER_STEP_GATES_4E7BE8[usize::from(xter & 0x0f)][usize::from(direction & 3)]
}

fn mac_route_direction_gate(xter: u8, direction: u8) -> bool {
    // CODE 0's Mac table also enables the east and west entries for 14–15.
    mac_pipe_direction_gate(xter, direction)
        || (xter < 0x40 && xter & 0x0f >= 14 && direction & 1 != 0)
}

fn mac_pipe_route_candidate_matches(
    city: &City,
    candidate: (usize, usize),
    direction: u8,
    target_height: i16,
) -> bool {
    let xter = city.terrain_at(candidate.0, candidate.1);
    let land = i16::from(city.altitude_at(candidate.0, candidate.1) & 0x1f);
    mac_route_direction_gate(xter, direction)
        || (MAC_PIPE_XTER_CLASS_DELTA[usize::from(xter & 0x0f)] != 0
            && (land == target_height || land + 2 == target_height))
        || (xter == 0 && land + 1 == target_height)
}

fn mac_underground_walk(
    city: &City,
    start: (usize, usize),
    end: (usize, usize),
) -> Option<MacRouteWalk> {
    mac_route_walk_init(city, start, end, false)
}

pub(super) fn mac_military_route_walk(
    city: &City,
    start: (usize, usize),
    end: (usize, usize),
) -> Option<MacRouteWalk> {
    mac_route_walk_init(city, start, end, true)
}

fn mac_route_walk_init(
    city: &City,
    start: (usize, usize),
    end: (usize, usize),
    military: bool,
) -> Option<MacRouteWalk> {
    if start.0 >= CITY_WIDTH
        || start.1 >= CITY_HEIGHT
        || end.0 >= CITY_WIDTH
        || end.1 >= CITY_HEIGHT
        || if military {
            city.building_at(start.0, start.1) == 5
        } else {
            city.zone_at(start.0, start.1)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
        }
    {
        return None;
    }

    let mut target = end;
    let mut direction = mac_pipe_direction_to_target(start, end);
    let start_xter = city.terrain_at(start.0, start.1);
    if (1..=4).contains(&start_xter) && mac_route_direction_gate(start_xter, direction) {
        // CODE 13 export 191 changes the saved orientation to the minor-axis
        // selector once the dominant selector passes. It then truncates the
        // endpoint to the origin only when that alternate selector also
        // passes the start tile's XTER gate.
        direction = mac_pipe_alternate_direction(start, end);
        if mac_route_direction_gate(start_xter, direction) {
            target = start;
        }
    }

    Some(MacRouteWalk {
        military,
        target,
        bounds: (
            start.0.min(target.0),
            start.1.min(target.1),
            start.0.max(target.0),
            start.1.max(target.1),
        ),
        current: start,
        direction,
        done: false,
    })
}

impl MacRouteWalk {
    /// Return the next cell export 185 or 186 visits. The iterator attempts the
    /// successor first, so its resulting direction is the one export 193 uses
    /// for the current cell, including a final rejected candidate.
    pub(super) fn next_commit(&mut self, city: &City) -> Option<MacRouteCell> {
        if self.done {
            return None;
        }
        let current = self.current;
        if current == self.target {
            self.done = true;
            return Some(MacRouteCell {
                tile: current,
                direction: self.direction,
            });
        }

        let current_xter = city.terrain_at(current.0, current.1);
        let current_xund = city.underground_at(current.0, current.1);
        let existing_xund_continues = current_xund != 0
            && matches!(
                u16::from(current_xund) + u16::from(self.direction & 1),
                0x11 | 0x02
            );
        let follows_existing_route = existing_xund_continues
            || PE_NETWORK_SURFACE_REMAP_GATE_4E7B68[usize::from(current_xter & 0x0f)] != 0;

        let candidate = if follows_existing_route {
            let Some(next) = mac_pipe_step_coordinate(current, self.direction) else {
                self.done = true;
                return Some(MacRouteCell {
                    tile: current,
                    direction: self.direction,
                });
            };
            if !pe_pipe_inside_stroke_bounds_44f50(next, self.bounds)
                || mac_route_direction_gate(city.terrain_at(next.0, next.1), self.direction)
            {
                self.done = true;
                return Some(MacRouteCell {
                    tile: current,
                    direction: self.direction,
                });
            }
            next
        } else {
            let primary = mac_pipe_direction_to_target(current, self.target);
            let Some(primary_tile) = mac_pipe_step_coordinate(current, primary) else {
                self.done = true;
                return Some(MacRouteCell {
                    tile: current,
                    direction: self.direction,
                });
            };
            if !pe_pipe_inside_stroke_bounds_44f50(primary_tile, self.bounds) {
                self.done = true;
                return Some(MacRouteCell {
                    tile: current,
                    direction: self.direction,
                });
            }

            let current_height = i16::from(city.altitude_at(current.0, current.1) & 0x1f);
            let target_height =
                current_height + MAC_PIPE_XTER_CLASS_DELTA[usize::from(current_xter & 0x0f)];
            let (next, direction) =
                if mac_pipe_route_candidate_matches(city, primary_tile, primary, target_height) {
                    let minor = mac_pipe_alternate_direction(current, self.target);
                    let minor_tile = mac_pipe_step_coordinate(current, minor)
                        .filter(|&tile| pe_pipe_inside_stroke_bounds_44f50(tile, self.bounds));
                    let Some(tile) = minor_tile.filter(|&tile| {
                        !mac_pipe_route_candidate_matches(city, tile, minor, target_height)
                    }) else {
                        // CODE 13 export 192 stops when both candidates pass.
                        self.done = true;
                        return Some(MacRouteCell {
                            tile: current,
                            direction: self.direction,
                        });
                    };
                    (tile, minor)
                } else {
                    (primary_tile, primary)
                };
            self.direction = direction;
            next
        };

        if (!self.military
            && city
                .zone_at(candidate.0, candidate.1)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY))
            || matches!(city.tunnel_levels_at(candidate.0, candidate.1), 1 | 2)
        {
            self.done = true;
            return Some(MacRouteCell {
                tile: current,
                direction: self.direction,
            });
        }
        let next_xund = city.underground_at(candidate.0, candidate.1);
        if next_xund != 0
            && !matches!(
                u16::from(next_xund) + u16::from(self.direction & 1),
                0x11 | 0x02
            )
        {
            self.done = true;
            return Some(MacRouteCell {
                tile: current,
                direction: self.direction,
            });
        }

        self.current = candidate;
        Some(MacRouteCell {
            tile: current,
            direction: self.direction,
        })
    }
}

fn mac_underground_path_and_cost(
    city: &City,
    start: (usize, usize),
    end: (usize, usize),
    per_tile_cost: u32,
) -> (Vec<MacRouteCell>, u32) {
    let Some(mut walk) = mac_underground_walk(city, start, end) else {
        return (Vec::new(), 0);
    };
    let mut cells = Vec::new();
    let mut surcharge_cells = 0u32;
    while let Some(cell) = walk.next_commit(city) {
        let xter = city.terrain_at(cell.tile.0, cell.tile.1);
        if xter < 0x30 && MAC_PIPE_XTER_PRICE_GATE[usize::from(xter & 0x0f)] != 0 {
            surcharge_cells = surcharge_cells.saturating_add(1);
        }
        cells.push(cell);
    }
    let cost = surcharge_cells
        .saturating_mul(MAC_PIPE_XTER_CLEAR_COST)
        .saturating_add((cells.len() as u32).saturating_mul(per_tile_cost));
    (cells, cost)
}

pub(super) fn mac_route_commit_xter_and_water_bit(
    city: &mut City,
    tile: (usize, usize),
    direction: u8,
) {
    let old_xter = city.terrain_at(tile.0, tile.1);
    if old_xter >= 0x30 || MAC_PIPE_XTER_PRICE_GATE[usize::from(old_xter & 0x0f)] == 0 {
        return;
    }
    let class = usize::from(old_xter & 0x0f);
    if PE_NETWORK_SURFACE_REMAP_GATE_4E7B68[class] == 0 {
        city.set_terrain(
            tile.0,
            tile.1,
            if (0x10..0x20).contains(&old_xter) {
                0x1d
            } else {
                0x0d
            },
        );
        if let Some(bits) = city.tile_bits_at(tile.0, tile.1)
            && bits.raw_bits() & 0x04 != 0
        {
            city.set_tile_bits(tile.0, tile.1, TileBits::new(bits.raw_bits() & !0x04));
        }
    } else {
        let output = MAC_PIPE_XTER_DIRECTIONAL_REMAP[class - 9][usize::from(direction & 3)];
        city.set_terrain(
            tile.0,
            tile.1,
            output + if old_xter >= 0x10 { 0x10 } else { 0 },
        );
    }
}

pub(super) fn mac_place_underground_tile(
    city: &mut City,
    tile: (usize, usize),
    kind: UndergroundKind,
) -> bool {
    let (x, y) = tile;
    let replacement = match (kind, city.underground_at(x, y)) {
        (_, 0) => kind.base_tile(),
        (UndergroundKind::Pipes, 1) | (UndergroundKind::Subway, 0x11) => 0x1f,
        (UndergroundKind::Pipes, 2) | (UndergroundKind::Subway, 0x10) => 0x20,
        _ => return false,
    };
    city.set_underground(x, y, replacement);
    if kind == UndergroundKind::Pipes {
        stamp_xbit_piped(city, x, y);
    }
    for (nx, ny) in std::iter::once(tile).chain(neighbor_tiles(x, y)) {
        mac_reshape_underground_tile(city, nx, ny, kind);
    }
    true
}

fn mac_reshape_underground_tile(city: &mut City, x: usize, y: usize, kind: UndergroundKind) {
    if !kind.contains(city.underground_at(x, y)) {
        return;
    }
    // CODE 13 @ 0x2489c / 0x24bfa folds XTER to the low nibble before
    // the WORD slope lookup. PlaceXUND stores the low byte.
    let terrain = city.terrain_at(x, y);
    let replacement = if let Some(offset) =
        mac_connector_slope_offset(if terrain <= 0x30 { terrain & 0x0f } else { 0 })
    {
        u16::from(kind.base_tile()) + offset
    } else {
        let mut mask = 0;
        for (side, nx, ny) in directional_neighbor_tiles(x, y) {
            let tile = city.underground_at(nx, ny);
            let compatible = match kind {
                UndergroundKind::Pipes => (0x10..=0x20).contains(&tile),
                UndergroundKind::Subway => matches!(tile, 1..=0x0f | 0x1f | 0x20 | 0x22 | 0x23),
            };
            if compatible && mac_connector_terrain_open(city.terrain_at(nx, ny), nx == x) {
                mask |= side;
            }
        }
        u16::from(match kind {
            UndergroundKind::Pipes => pe_place_pipes_mask_tile_44ba9a(mask),
            UndergroundKind::Subway => pe_subway_mask_tile_44be07(mask),
        })
    };
    city.mac_place_xund_word(x, y, replacement);
}

impl ViewerState {
    pub(super) fn begin_underground_drag(&mut self, x: usize, y: usize, kind: UndergroundKind) {
        self.underground_drag = Some(UndergroundDrag {
            start: (x, y),
            current: (x, y),
            kind,
        });
    }

    pub(super) fn update_underground_drag_at(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        if let Some(tile) = self.tile_at_screen(screen, window_size, pixel_scale) {
            self.update_underground_drag_to(tile);
        }
    }

    pub(super) fn update_underground_drag_to(&mut self, tile: (usize, usize)) {
        if let Some(drag) = self.underground_drag.as_mut() {
            drag.current = tile;
        }
    }

    fn uses_mac_underground_tool(&self, kind: UndergroundKind) -> bool {
        !self.engine_fidelity.uses_win95() && underground_kind_for_tool(self.tool) == Some(kind)
    }

    fn uses_win95_underground_tool(&self, kind: UndergroundKind) -> bool {
        self.engine_fidelity.uses_win95() && underground_kind_for_tool(self.tool) == Some(kind)
    }

    pub(super) fn finish_underground_drag(&mut self) -> Option<(UndergroundKind, usize)> {
        if let Some(drag) = self.underground_drag.take() {
            let placed_tiles = if self.uses_mac_underground_tool(drag.kind) {
                self.place_mac_underground_stroke(drag.start, drag.current, drag.kind)
            } else if self.uses_win95_underground_tool(drag.kind) {
                self.place_win95_underground_stroke(drag.start, drag.current, drag.kind)
            } else {
                let path =
                    self.underground_drag_path_for_kind(drag.start, drag.current, drag.kind, false);
                self.place_underground_path(&path, drag.kind)
            };
            return Some((drag.kind, placed_tiles));
        }
        None
    }

    pub(super) fn cancel_underground_drag(&mut self) {
        self.underground_drag = None;
    }

    pub(super) fn underground_drag_active(&self) -> bool {
        self.underground_drag.is_some()
    }

    pub(super) fn underground_drag_path_for_kind(
        &self,
        start: (usize, usize),
        end: (usize, usize),
        kind: UndergroundKind,
        preview: bool,
    ) -> Vec<(usize, usize)> {
        if self.uses_win95_underground_tool(kind)
            || (kind == UndergroundKind::Pipes && self.engine_fidelity.uses_win95())
        {
            pe_pipe_stroke_path_448bc0_448f50(&self.city, start, end)
        } else if self.uses_mac_underground_tool(kind) {
            mac_underground_path_and_cost(&self.city, start, end, kind.cost_per_tile())
                .0
                .into_iter()
                .map(|cell| cell.tile)
                .collect()
        } else if preview {
            underground_preview_path_for_kind(start, end, kind)
        } else {
            underground_commit_path_for_kind(start, end, kind)
        }
    }

    pub(super) fn underground_drag_cost(&self) -> Option<(UndergroundKind, u32)> {
        let drag = self.underground_drag?;
        if self.uses_mac_underground_tool(drag.kind) {
            let (_, cost) = mac_underground_path_and_cost(
                &self.city,
                drag.start,
                drag.current,
                drag.kind.cost_per_tile(),
            );
            return Some((drag.kind, cost));
        }
        let path = self.underground_drag_path_for_kind(drag.start, drag.current, drag.kind, true);
        let cost = if self.uses_win95_underground_tool(drag.kind) {
            pe_network_surface_stroke_cost_448a90(&self.city, &path, drag.kind.cost_per_tile())
        } else {
            let placement = self.underground_drag_placement_for_path(&path, drag.kind);
            if drag.kind == UndergroundKind::Pipes
                || (drag.kind == UndergroundKind::Subway && self.engine_fidelity.uses_win95())
            {
                pe_network_surface_stroke_cost_448a90(
                    &self.city,
                    &placement.valid_tiles,
                    drag.kind.cost_per_tile(),
                )
            } else {
                placement.valid_tiles.len() as u32 * drag.kind.cost_per_tile()
            }
        };
        Some((drag.kind, cost))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn push_underground_preview(
        &self,
        renderer: &Renderer<'_>,
        window_size: (u32, u32),
        pixel_scale: f32,
        sprites: &mut Vec<RenderSprite>,
        x: usize,
        y: usize,
        kind: UndergroundKind,
        virtual_tiles: &HashSet<(usize, usize)>,
    ) {
        let tile = self.underground_preview_render_tile(x, y, kind, virtual_tiles);
        let (anchor, zoom, sx, sy) = self.structure_anchor(window_size, pixel_scale, x, y);
        push_underground_tile_sprite(
            renderer,
            sprites,
            tile,
            anchor,
            zoom,
            sprite_depth(sx, sy, DEPTH_ZONE, 24),
            self.animation_frame(),
        );
    }

    pub(super) fn set_tile_base_pipe_bit(&mut self, x: usize, y: usize) {
        let current_bits = self.city.tile_bits_at(x, y);
        let bits = current_bits.unwrap_or_default().with_piped(true);
        if current_bits != Some(bits) {
            self.city.set_tile_bits(x, y, bits);
        }
    }

    pub(super) fn underground_placement_at(
        &self,
        x: usize,
        y: usize,
        kind: UndergroundKind,
    ) -> ToolPlacement {
        if self.can_place_underground_at(x, y, kind) {
            ToolPlacement::valid(ToolPreview::Underground(kind), vec![(x, y)])
        } else {
            ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview: ToolPreview::Underground(kind),
            }
        }
    }

    pub(super) fn place_underground_at(
        &mut self,
        x: usize,
        y: usize,
        kind: UndergroundKind,
    ) -> usize {
        if self.uses_win95_underground_tool(kind) {
            return self.place_win95_underground_stroke((x, y), (x, y), kind);
        }
        if self.uses_mac_underground_tool(kind) {
            return self.place_mac_underground_stroke((x, y), (x, y), kind);
        }
        self.place_underground_path(&[(x, y)], kind)
    }

    fn place_mac_underground_stroke(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
        kind: UndergroundKind,
    ) -> usize {
        // CODE 13 exports 185 and 186 price the read-only accepted walk and debits it
        // in one transaction before re-running the iterator against mutations.
        let (_, cost) = mac_underground_path_and_cost(&self.city, start, end, kind.cost_per_tile());
        if cost == 0 || !self.can_afford_build_cost(cost) {
            return 0;
        }
        self.debit_build_cost(cost);

        let Some(mut walk) = mac_underground_walk(&self.city, start, end) else {
            return 0;
        };
        let mut placed = 0usize;
        for _ in 0..=(CITY_WIDTH * CITY_HEIGHT) {
            let Some(cell) = walk.next_commit(&self.city) else {
                break;
            };
            mac_route_commit_xter_and_water_bit(&mut self.city, cell.tile, cell.direction);
            if mac_place_underground_tile(&mut self.city, cell.tile, kind) {
                placed += 1;
            }
        }

        if kind == UndergroundKind::Pipes {
            self.mac_pipe_tool_post_place_water_recalc();
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
        placed
    }

    fn place_win95_underground_stroke(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
        kind: UndergroundKind,
    ) -> usize {
        // Pipe (0x44a5a0) and Subway (0x44a720) share the XUND walk.
        // FUN_00448610 / FUN_00448A90 price a read-only walk before the parent
        // debits funds. The commit then reinitializes the walk and reevaluates
        // each next tile after PlaceXUND and its neighbor reshapes have run.
        let priced_path = pe_pipe_stroke_path_448bc0_448f50(&self.city, start, end);
        if priced_path.is_empty() {
            return 0;
        }
        let cost =
            pe_network_surface_stroke_cost_448a90(&self.city, &priced_path, kind.cost_per_tile());
        if !self.can_afford_build_cost(cost) {
            return 0;
        }
        self.debit_build_cost(cost);

        let Some(walk) = pe_pipe_stroke_walk_init_448bc0(&self.city, start, end) else {
            return 0;
        };
        let mut current = start;
        let mut direction = walk.direction;
        let mut placed = 0usize;

        for _ in 0..(CITY_WIDTH + CITY_HEIGHT) {
            // The native caller asks for the next coordinate before it writes
            // the current one. This sets the XTER-remap direction; then the
            // current tile and its four neighbors are mutated before the next
            // call to FUN_00448F50.
            let step = pe_pipe_next_stroke_tile_448f50(
                &self.city,
                current,
                walk.target,
                walk.bounds,
                direction,
            );
            let edit_direction = step.direction;
            pe_network_surface_edit_448e20(&mut self.city, current.0, current.1, edit_direction);

            if self.pe_place_underground_coordinate(current.0, current.1, kind) {
                placed += 1;
            }

            let Some(next) = step.next else {
                break;
            };
            current = next;
            direction = step.direction;
        }

        // The pipe event at 0x43f419 can run water allocation even when the
        // accepted walk only visited existing pipes. Subway has no service callback.
        if kind == UndergroundKind::Pipes {
            pe_pipetool_post_place_water_recalc_43f419(&mut self.city);
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
        placed
    }

    pub(super) fn place_underground_path(
        &mut self,
        path: &[(usize, usize)],
        kind: UndergroundKind,
    ) -> usize {
        let selected_tool = underground_kind_for_tool(self.tool) == Some(kind);
        let pe_subway_contract =
            selected_tool && kind == UndergroundKind::Subway && self.engine_fidelity.uses_win95();
        let pe_stroke_pricing =
            selected_tool && (kind == UndergroundKind::Pipes || pe_subway_contract);
        let stroke_path: Vec<_> = if pe_subway_contract {
            self.underground_buildable_prefix_path(path, kind).0
        } else {
            Vec::new()
        };
        let mut stroke_cost = 0;
        if pe_subway_contract {
            stroke_cost = pe_network_surface_stroke_cost_448a90(
                &self.city,
                &stroke_path,
                kind.cost_per_tile(),
            );
            if !self.can_afford_build_cost(stroke_cost) {
                return 0;
            }
            self.debit_build_cost(stroke_cost);
            for (index, &(x, y)) in stroke_path.iter().enumerate() {
                pe_network_surface_edit_448e20(
                    &mut self.city,
                    x,
                    y,
                    pe_pipe_direction_for_path_tile(&stroke_path, index),
                );
            }
        }

        let mut commit_path = self.underground_buildable_prefix_path(path, kind).0;
        if kind == UndergroundKind::Pipes {
            stroke_cost = pe_network_surface_stroke_cost_448a90(
                &self.city,
                &commit_path,
                kind.cost_per_tile(),
            );
        } else if !pe_subway_contract {
            stroke_cost = (commit_path.len() as u32).saturating_mul(kind.cost_per_tile());
        }
        if pe_stroke_pricing && !pe_subway_contract {
            if !self.can_afford_build_cost(stroke_cost) {
                return 0;
            }
        } else if selected_tool && !pe_subway_contract {
            commit_path.truncate(self.affordable_build_units(kind.cost_per_tile()));
        }
        if commit_path.is_empty() && !pe_subway_contract {
            return 0;
        }

        let mut placed = 0usize;
        for (index, &(x, y)) in commit_path.iter().enumerate() {
            if kind == UndergroundKind::Pipes {
                let direction = pe_pipe_direction_for_path_tile(&commit_path, index);
                pe_network_surface_edit_448e20(&mut self.city, x, y, direction);
            }
            let existing = self.city.underground_at(x, y);
            // PE PlacePipes/Subway AtCoordinates early ret: already own-kind / exclusive.
            if pe_place_underground_at_coordinates_noop(kind, existing) {
                continue;
            }
            if let Some(crossover) = pe_place_underground_crossover_from_existing(kind, existing)
                .or_else(|| {
                    let axis = network_single_axis(network_path_axis_mask(&commit_path, index))
                        .or_else(|| {
                            inferred_single_tile_underground_crossing_axis(kind, existing)
                        })?;
                    underground_crossing_tile_for(kind, axis, existing)
                })
            {
                // PE PlacePipes/subway arms → PlaceXUND (`0x44b801` / `0x44bb9a`).
                pe_place_xund_442370(&mut self.city, x, y, crossover);
            } else {
                // PlacePipesAtCoordinates commits a plain pipe/subway tile first;
                // the caller then reshapes this tile and its cardinal neighbors.
                pe_place_xund_442370(&mut self.city, x, y, kind.base_tile());
            }
            if kind == UndergroundKind::Pipes {
                // PE PlacePipesAtCoordinates @ 0x44b829: OR XBIT piped (0x20).
                self.set_tile_base_pipe_bit(x, y);
            }
            placed += 1;
            // PE processes a drag one coordinate at a time.  Its 0x4016b8
            // reshape runs immediately after each commit, on the committed tile
            // and its north/east/south/west neighbors.  Keeping this order matters:
            // a newly reshaped neighbor can change the gates seen by the next tile.
            self.refresh_underground_tile(x, y, kind);
            for (_, nx, ny) in directional_neighbor_tiles(x, y) {
                self.refresh_underground_tile(nx, ny, kind);
            }
        }
        // PE PlacePipesAtCoordinates (`0x44b82d`): reshape via `0x4016b8` on the commit
        // tile and its orthogonal neighbors only. Citywide water clear is the parent
        // tool arm @ `0x43f419`–`0x43f42f` (below), not inside PlacePipes.
        // Subway place is likewise underground reshape + redraw; neither writes XTRF.
        if placed == 0 {
            let accepted_pipe_stroke =
                kind == UndergroundKind::Pipes && selected_tool && !commit_path.is_empty();
            let accepted_pe_subway_stroke = pe_subway_contract && !stroke_path.is_empty();
            if !accepted_pipe_stroke && !accepted_pe_subway_stroke {
                return 0;
            }
        }
        // The two native engines have separate post-route callbacks. PE calls
        // FUN_00464550 under its dwCityPopulation gate; Mac selector 4 calls
        // CODE 14 export 209 under its weighted-growth-scalar gate.
        if kind == UndergroundKind::Pipes {
            if self.engine_fidelity.uses_win95() && (placed > 0 || selected_tool) {
                pe_pipetool_post_place_water_recalc_43f419(&mut self.city);
            } else if !self.engine_fidelity.uses_win95() && selected_tool {
                self.mac_pipe_tool_post_place_water_recalc();
            }
        }
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        if !pe_subway_contract {
            let cost = if pe_stroke_pricing {
                stroke_cost
            } else {
                kind.cost_per_tile().saturating_mul(placed as u32)
            };
            self.debit_build_cost(cost);
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
        placed
    }

    pub(super) fn can_place_underground_at(
        &self,
        x: usize,
        y: usize,
        kind: UndergroundKind,
    ) -> bool {
        self.can_place_underground_path_tile(&[(x, y)], 0, kind)
    }

    pub(super) fn can_place_underground_path_tile(
        &self,
        path: &[(usize, usize)],
        index: usize,
        kind: UndergroundKind,
    ) -> bool {
        let Some(&(x, y)) = path.get(index) else {
            return false;
        };
        if path.len() == 1 {
            if self.uses_mac_underground_tool(kind) {
                return mac_underground_walk(&self.city, (x, y), (x, y)).is_some();
            }
            if self.uses_win95_underground_tool(kind) {
                return pe_pipe_stroke_walk_init_448bc0(&self.city, (x, y), (x, y)).is_some();
            }
        }
        // PE FUN_00448BC0 rejects a mode-4 stroke only when its starting tile
        // is military-zone XZON type 7; FUN_00448F50 does not reject later
        // candidates for that zone type.
        if underground_kind_for_tool(self.tool) == Some(kind)
            && kind == UndergroundKind::Pipes
            && index == 0
            && self
                .city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
        {
            return false;
        }
        let existing = self.city.underground_at(x, y);
        // PE AtCoordinates refuse (curves / foreign exclusive) — not a valid place tile.
        if pe_place_underground_at_coordinates_refuse(kind, existing) {
            return false;
        }
        // PE early-out noop (already own-kind / crossover): valid drag pass-through, no stamp.
        if pe_place_underground_at_coordinates_noop(kind, existing) {
            return true;
        }
        existing == 0
            || pe_place_underground_crossover_from_existing(kind, existing).is_some()
            || network_single_axis(network_path_axis_mask(path, index))
                .and_then(|axis| underground_crossing_tile_for(kind, axis, existing))
                .is_some()
            || (path.len() == 1
                && inferred_single_tile_underground_crossing_axis(kind, existing).is_some())
    }

    pub(super) fn refresh_underground_tile(&mut self, x: usize, y: usize, kind: UndergroundKind) {
        let existing = self.city.underground_at(x, y);
        if kind == UndergroundKind::Pipes && self.protected_building_pipe_at(x, y, existing) {
            return;
        }
        if !kind.contains(existing) {
            return;
        }
        // PE PlacePipes reshape `0x44b8d0`: XTXT gate PlaceXUND early-out before mask.
        if kind == UndergroundKind::Pipes
            && pe_place_pipes_xtxt_reshape_44b961(&mut self.city, x, y)
        {
            return;
        }
        // PE subway reshape `0x44bc40`: XTXT gate PlaceXUND early-out before mask.
        if kind == UndergroundKind::Subway && pe_subway_xtxt_reshape_44bcc5(&mut self.city, x, y) {
            return;
        }
        let mask = self.underground_neighbor_mask(x, y, kind);
        let tile = match kind {
            UndergroundKind::Pipes => pe_place_pipes_mask_tile_44ba9a(mask),
            UndergroundKind::Subway => pe_subway_mask_tile_44be07(mask),
        };
        if existing != tile {
            // PE PlaceXUND reshape: XUND only — leave watered/piped/XTRF.
            pe_place_xund_442370(&mut self.city, x, y, tile);
        }
    }

    pub(super) fn underground_neighbor_mask(
        &self,
        x: usize,
        y: usize,
        kind: UndergroundKind,
    ) -> u8 {
        let mut mask = 0;
        if y > 0 && self.underground_neighbor_matches(x, y - 1, kind, NETWORK_BOTTOM) {
            mask |= NETWORK_TOP;
        }
        if x + 1 < CITY_WIDTH && self.underground_neighbor_matches(x + 1, y, kind, NETWORK_LEFT) {
            mask |= NETWORK_RIGHT;
        }
        if y + 1 < CITY_HEIGHT && self.underground_neighbor_matches(x, y + 1, kind, NETWORK_TOP) {
            mask |= NETWORK_BOTTOM;
        }
        if x > 0 && self.underground_neighbor_matches(x - 1, y, kind, NETWORK_RIGHT) {
            mask |= NETWORK_LEFT;
        }
        mask
    }

    pub(super) fn underground_neighbor_mask_with_virtual(
        &self,
        x: usize,
        y: usize,
        kind: UndergroundKind,
        virtual_tiles: &HashSet<(usize, usize)>,
    ) -> u8 {
        let mut mask = 0;
        if y > 0
            && self.underground_neighbor_matches_with_virtual(
                x,
                y - 1,
                kind,
                virtual_tiles,
                NETWORK_BOTTOM,
            )
        {
            mask |= NETWORK_TOP;
        }
        if x + 1 < CITY_WIDTH
            && self.underground_neighbor_matches_with_virtual(
                x + 1,
                y,
                kind,
                virtual_tiles,
                NETWORK_LEFT,
            )
        {
            mask |= NETWORK_RIGHT;
        }
        if y + 1 < CITY_HEIGHT
            && self.underground_neighbor_matches_with_virtual(
                x,
                y + 1,
                kind,
                virtual_tiles,
                NETWORK_TOP,
            )
        {
            mask |= NETWORK_BOTTOM;
        }
        if x > 0
            && self.underground_neighbor_matches_with_virtual(
                x - 1,
                y,
                kind,
                virtual_tiles,
                NETWORK_RIGHT,
            )
        {
            mask |= NETWORK_LEFT;
        }
        mask
    }

    pub(super) fn underground_neighbor_matches(
        &self,
        x: usize,
        y: usize,
        kind: UndergroundKind,
        side: u8,
    ) -> bool {
        let tile = self.city.underground_at(x, y);
        match kind {
            // PE PlacePipes reshape neighbor @ `0x44b987` — XUND range + NS/EW gates
            // (not water-service `water_pipe_connects_to`).
            UndergroundKind::Pipes => pe_place_pipes_reshape_neighbor_connects_44b98a(tile, side),
            UndergroundKind::Subway => underground_tile_connects_to(tile, kind, side),
        }
    }

    pub(super) fn underground_neighbor_matches_with_virtual(
        &self,
        x: usize,
        y: usize,
        kind: UndergroundKind,
        virtual_tiles: &HashSet<(usize, usize)>,
        side: u8,
    ) -> bool {
        virtual_tiles.contains(&(x, y)) || self.underground_neighbor_matches(x, y, kind, side)
    }

    pub(super) fn underground_preview_render_tile(
        &self,
        x: usize,
        y: usize,
        kind: UndergroundKind,
        virtual_tiles: &HashSet<(usize, usize)>,
    ) -> UndergroundRenderTile {
        let existing = self.city.underground_at(x, y);
        let mask = self.underground_neighbor_mask_with_virtual(x, y, kind, virtual_tiles);
        let tile = network_single_axis(network_axis_mask_for_side_mask(mask))
            .or_else(|| inferred_single_tile_underground_crossing_axis(kind, existing))
            .and_then(|axis| underground_crossing_tile_for(kind, axis, existing))
            .unwrap_or_else(|| underground_tile_for_mask(kind, mask));

        UndergroundRenderTile {
            tile: rotated_underground_tile(tile, self.city.rotation()),
            source: UndergroundTileSource::UserLaid,
            watered: false,
        }
    }

    pub(super) fn protected_building_pipe_at(&self, x: usize, y: usize, existing: u8) -> bool {
        water_source_connects_as_building_pipe(&self.city, x, y)
            || underground_building_pipe_tile(existing)
    }

    pub(super) fn underground_placement_for_path(
        &self,
        path: &[(usize, usize)],
        kind: UndergroundKind,
    ) -> ToolPlacement {
        let mut valid_tiles = Vec::new();
        let mut invalid_tiles = Vec::new();
        for (index, &(x, y)) in path.iter().enumerate() {
            if self.can_place_underground_path_tile(path, index, kind) {
                valid_tiles.push((x, y));
            } else {
                invalid_tiles.push((x, y));
            }
        }
        ToolPlacement {
            valid_tiles,
            invalid_tiles,
            preview: ToolPreview::Underground(kind),
        }
    }

    pub(super) fn underground_drag_placement_for_path(
        &self,
        path: &[(usize, usize)],
        kind: UndergroundKind,
    ) -> ToolPlacement {
        if self.uses_mac_underground_tool(kind) || self.uses_win95_underground_tool(kind) {
            return ToolPlacement::valid(ToolPreview::Underground(kind), path.to_vec());
        }
        let (valid_tiles, invalid_tile) = self.underground_buildable_prefix_path(path, kind);
        ToolPlacement {
            valid_tiles,
            invalid_tiles: invalid_tile.into_iter().collect(),
            preview: ToolPreview::Underground(kind),
        }
    }

    pub(super) fn underground_buildable_prefix_path(
        &self,
        path: &[(usize, usize)],
        kind: UndergroundKind,
    ) -> BuildablePrefixPath {
        let mut prefix = Vec::new();
        for &tile in path {
            prefix.push(tile);
            if !self
                .underground_placement_for_path(&prefix, kind)
                .invalid_tiles
                .is_empty()
            {
                prefix.pop();
                return (prefix, Some(tile));
            }
        }
        (prefix, None)
    }

    pub(super) fn tunnel_placement_at(&self, x: usize, y: usize) -> ToolPlacement {
        if let Some(plan) = self.tunnel_plan_at(x, y) {
            ToolPlacement::valid(ToolPreview::Building(plan.entrance_building), plan.tiles)
        } else {
            let slope = self.city.terrain_at(x, y);
            ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview: ToolPreview::Building(if (1..=4).contains(&slope) {
                    62 + slope
                } else {
                    63
                }),
            }
        }
    }

    pub(super) fn tunnel_plan_at(&self, x: usize, y: usize) -> Option<TunnelPlan> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return None;
        }
        let mac = !self.engine_fidelity.uses_win95();
        let start_building = self.city.building_at(x, y);
        if start_building >= 14 || start_building == 5 || self.city.underground_at(x, y) != 0 {
            return None;
        }

        let start_slope = self.city.terrain_at(x, y);
        let (dx, dy) = match start_slope {
            1 => (-1isize, 0isize),
            2 => (0, -1),
            3 => (1, 0),
            4 => (0, 1),
            _ => return None,
        };
        let base_altitude = self.city.altitude_at(x, y);
        let mut current = (x, y);
        let mut tiles = vec![current];

        loop {
            // Win95 reserves the last map row and column for a possible tunnel exit.
            if !mac && (current.0 >= CITY_WIDTH - 1 || current.1 >= CITY_HEIGHT - 1) {
                return None;
            }
            if self.city.tunnel_levels_at(current.0, current.1) != 0 {
                return None;
            }

            let depth =
                i16::from(self.city.altitude_at(current.0, current.1)) - i16::from(base_altitude);
            if depth > 30 {
                return None;
            }
            if depth == 1 {
                let xund = self.city.underground_at(current.0, current.1);
                let conflict = if mac {
                    matches!(xund, 1..=0x20 | 0x22 | 0x23)
                } else {
                    !matches!(xund, 0 | UNDER_TILE_BUILDING_PIPES)
                };
                if conflict {
                    return None;
                }
            }

            let next_x = current.0 as isize + dx;
            let next_y = current.1 as isize + dy;
            if !(0..CITY_WIDTH as isize).contains(&next_x)
                || !(0..CITY_HEIGHT as isize).contains(&next_y)
            {
                return None;
            }
            current = (next_x as usize, next_y as usize);
            tiles.push(current);
            if self.city.altitude_at(current.0, current.1) <= base_altitude {
                break;
            }
        }

        let exit_slope = ((start_slope + 1) & 3) + 1;
        (self.city.terrain_at(current.0, current.1) == exit_slope).then_some(TunnelPlan {
            tiles,
            entrance_building: 62 + start_slope,
            exit_building: 62 + exit_slope,
        })
    }

    pub(super) fn place_tunnel_at(&mut self, x: usize, y: usize) -> usize {
        if matches!(
            self.city_info_dialog,
            Some(CityInfoDialog::TunnelQuote { .. })
        ) {
            return 0;
        }
        let Some(plan) = self.tunnel_plan_at(x, y) else {
            return 0;
        };
        if !self.engine_fidelity.uses_win95() {
            let exit = *plan.tiles.last().expect("tunnel has an exit");
            let cost = Self::tool_build_total_cost(EditTool::Tunnel, plan.tiles.len() as u32)
                .expect("tunnel unit price");
            self.open_city_info_dialog(CityInfoDialog::TunnelQuote {
                entrance: (x, y),
                exit,
                cost,
            });
            return 0;
        }
        if !self.can_afford_tool_build_cost(EditTool::Tunnel, plan.tiles.len() as u32) {
            return 0;
        }
        let base_altitude = self.city.altitude_at(x, y);
        let exit = *plan.tiles.last().expect("tunnel has an exit");

        // PE PlaceTile (`0x441f00` / `0x40178f`): portal stamps leave watered/piped/XTRF.
        // PE MapToolTunnel never clears XZON on portals (unlike PlaceRoads/Rails).
        pe_place_tile_441f00(&mut self.city, x, y, plan.entrance_building);
        pe_place_tile_441f00(&mut self.city, exit.0, exit.1, plan.exit_building);
        self.clear_zone_growth_for_player_tiles(&[(x, y), exit]);
        for &(tx, ty) in &plan.tiles {
            let levels =
                (i16::from(self.city.altitude_at(tx, ty)) - i16::from(base_altitude) + 1) as u8;
            self.city.set_tunnel_levels(tx, ty, levels);
        }
        // PE PlaceTile path: no invent citywide water-network refresh.
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.debit_tool_build_cost(EditTool::Tunnel, plan.tiles.len() as u32);
        self.query_is_inspect = false;
        self.refresh_status_line();
        plan.tiles.len()
    }

    pub(super) fn accept_mac_tunnel_quote(&mut self) -> usize {
        let Some(CityInfoDialog::TunnelQuote {
            entrance,
            exit,
            cost,
        }) = self.city_info_dialog
        else {
            return 0;
        };
        self.close_city_info_dialog();
        let (x, y) = entrance;
        let Some(plan) = self.tunnel_plan_at(x, y) else {
            return 0;
        };
        if self.engine_fidelity.uses_win95()
            || plan.tiles.last().copied() != Some(exit)
            || Self::tool_build_total_cost(EditTool::Tunnel, plan.tiles.len() as u32) != Some(cost)
            || !self.can_afford_build_cost(cost)
        {
            return 0;
        }

        // CODE 13 @ 0x25170 debits before stamping. The two portals
        // clear the entire grade field and use grade one.
        self.debit_build_cost(cost);
        let base = self.city.altitude_at(x, y);
        pe_place_tile_441f00(&mut self.city, x, y, plan.entrance_building);
        self.write_mac_tunnel_grade(x, y, 1);
        for (nx, ny) in neighbor_tiles(x, y) {
            self.refresh_mac_road_tile(nx, ny);
        }
        for &(tx, ty) in &plan.tiles[1..plan.tiles.len() - 1] {
            let grade = self.city.altitude_at(tx, ty) - base + 1;
            self.write_mac_tunnel_grade(tx, ty, grade);
        }
        pe_place_tile_441f00(&mut self.city, exit.0, exit.1, plan.exit_building);
        self.write_mac_tunnel_grade(exit.0, exit.1, 1);
        for (nx, ny) in neighbor_tiles(exit.0, exit.1) {
            self.refresh_mac_road_tile(nx, ny);
        }
        self.clear_zone_growth_for_player_tiles(&[entrance, exit]);
        self.pending_traffic_redraw = true;
        self.query_is_inspect = false;
        self.refresh_status_line();
        plan.tiles.len()
    }

    fn write_mac_tunnel_grade(&mut self, x: usize, y: usize, grade: u8) {
        let sample = self.city.altm_at(x, y);
        let word =
            u16::from(sample.land) | (u16::from(sample.water) << 5) | (u16::from(grade) << 10);
        self.city.set_altm_word(x, y, word);
    }

    pub(in crate::app) fn place_mac_subway_transition_underground(&mut self, x: usize, y: usize) {
        // CODE 5 0x3c6e (station) and 0x470e (SubToRail): PlaceSubway
        // first, then PlaceXUND(0x23).
        mac_place_underground_tile(&mut self.city, (x, y), UndergroundKind::Subway);
        self.city
            .mac_place_xund_word(x, y, u16::from(UNDER_TILE_GREY_BOX_2));
        if let Some(bits) = self.city.tile_bits_at(x, y) {
            self.city.set_tile_bits(x, y, bits.with_piped(false));
        }
    }

    pub(super) fn apply_building_underground_side_effects(
        &mut self,
        x: usize,
        y: usize,
        building: u8,
    ) {
        match building {
            // PE CityTool water pump @ `0x44087b` → PlacePipesAtCoordinates `0x44b790`.
            220 => {
                self.pe_place_pipes_at_coordinates_44b790(x, y);
            }
            // Desalination: keep OpenCity building-pipe tee until a PE CityTool/MapTool
            // PlacePipesAtCoordinates caller for XBLD `250` is mapped (jump table skips it).
            250 => {
                self.stamp_building_underground_footprint(x, y, UNDER_TILE_BUILDING_PIPE_TEE);
                // PE PlacePipesAtCoordinates @ 0x44b829: OR XBIT piped (0x20).
                if let Some(footprint) = self.building_footprint_at(x, y) {
                    for &(tx, ty) in &footprint {
                        stamp_xbit_piped(&mut self.city, tx, ty);
                    }
                } else {
                    stamp_xbit_piped(&mut self.city, x, y);
                }
                // PE PlacePipes then calls 0x4016b8 on the commit tile and its four
                // orthogonal neighbors so adjacent user pipes rejoin the tee (0x20).
                self.refresh_place_pipes_neighbors_like_pe_4016b8(x, y);
            }
            // PE CityTool SubStation @ `0x44088d` / MapTool SubToRail @ `0x441d06`:
            // PlaceSubwayAtCoordinates `0x44bb20`, then PlaceXUND(0x23), then XBIT &= 0xDF.
            108..=111 | 233 => {
                self.pe_place_subway_at_coordinates_44bb20(x, y);
                pe_place_xund_442370(&mut self.city, x, y, UNDER_TILE_GREY_BOX_2);
                for (nx, ny) in neighbor_tiles(x, y) {
                    self.refresh_underground_tile(nx, ny, UndergroundKind::Subway);
                }
                if let Some(bits) = self.city.tile_bits_at(x, y) {
                    self.city.set_tile_bits(x, y, bits.with_piped(false));
                }
            }
            // Recovered OpenCity2k metadata uses XUND 0x22 for missile silo underground pieces.
            249 => {
                self.stamp_building_underground_footprint(x, y, UNDER_TILE_GREY_BOX_1);
            }
            _ => {}
        }
    }

    pub(super) fn stamp_building_underground_footprint(&mut self, x: usize, y: usize, tile: u8) {
        let Some(footprint) = self.building_footprint_at(x, y) else {
            return;
        };
        for (tx, ty) in footprint {
            // PE PlaceXUND family: XUND only — leave watered/piped/XTRF.
            pe_place_xund_442370(&mut self.city, tx, ty, tile);
        }
    }

    /// PE PlacePipes post-commit refresh via `0x4016b8` (`0x44b8d0`): reshape user
    /// pipe XUND on each commit tile and its orthogonal neighbors.
    pub(super) fn refresh_place_pipes_neighbors_like_pe_4016b8(
        &mut self,
        origin_x: usize,
        origin_y: usize,
    ) {
        let mut refresh_tiles = HashSet::new();
        let tiles = self
            .building_footprint_at(origin_x, origin_y)
            .unwrap_or_else(|| vec![(origin_x, origin_y)]);
        for &(tx, ty) in &tiles {
            refresh_tiles.insert((tx, ty));
            refresh_tiles.extend(neighbor_tiles(tx, ty));
        }
        for (tx, ty) in refresh_tiles {
            self.refresh_underground_tile(tx, ty, UndergroundKind::Pipes);
        }
    }

    pub(super) fn refresh_pipe_neighbors_of_tiles(&mut self, tiles: &[(usize, usize)]) {
        let mut refresh_tiles = HashSet::new();
        for &(tx, ty) in tiles {
            refresh_tiles.insert((tx, ty));
            refresh_tiles.extend(neighbor_tiles(tx, ty));
        }
        for (tx, ty) in refresh_tiles {
            self.refresh_underground_tile(tx, ty, UndergroundKind::Pipes);
        }
    }

    /// PE PlacePipesAtCoordinates `0x44b790`: vacant→`0x10`, subway `1`→`0x1f` / `2`→`0x20`,
    /// noop on existing pipes/crossover, else refuse; then OR piped + reshape `0x4016b8`.
    pub(super) fn pe_place_pipes_at_coordinates_44b790(&mut self, x: usize, y: usize) {
        let existing = self.city.underground_at(x, y);
        if pe_place_pipes_at_coordinates_noop_44b7b8(existing) {
            return;
        }
        if pe_place_pipes_at_coordinates_refuse_44b7f0(existing) {
            return;
        }
        let tile = match existing {
            0 => UNDER_TILE_PIPES_LR,
            1 => UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR,
            2 => UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB,
            _ => return,
        };
        pe_place_xund_442370(&mut self.city, x, y, tile);
        self.set_tile_base_pipe_bit(x, y);
        self.refresh_place_pipes_neighbors_like_pe_4016b8(x, y);
    }

    /// PE PlaceSubwayAtCoordinates `0x44bb20`: vacant→`0x01`, pipes `0x10`→`0x20` /
    /// `0x11`→`0x1f`, noop on subway/crossover/station, else refuse; then reshape
    /// `0x401f64`→`0x44bc40` on commit + orthogonal neighbors.
    pub(super) fn pe_place_subway_at_coordinates_44bb20(&mut self, x: usize, y: usize) {
        let existing = self.city.underground_at(x, y);
        if pe_place_subway_at_coordinates_noop_44bb40(existing) {
            return;
        }
        if pe_place_subway_at_coordinates_refuse_44bb8b(existing) {
            return;
        }
        let tile = match existing {
            0 => 1,
            0x10 => UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB,
            0x11 => UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR,
            _ => return,
        };
        pe_place_xund_442370(&mut self.city, x, y, tile);
        self.refresh_place_subway_neighbors_like_pe_401f64(x, y);
    }

    /// PE PlaceSubway post-commit refresh via `0x401f64` (`0x44bc40`).
    pub(super) fn refresh_place_subway_neighbors_like_pe_401f64(
        &mut self,
        origin_x: usize,
        origin_y: usize,
    ) {
        let mut refresh_tiles = HashSet::new();
        refresh_tiles.insert((origin_x, origin_y));
        refresh_tiles.extend(neighbor_tiles(origin_x, origin_y));
        for (tx, ty) in refresh_tiles {
            self.refresh_underground_tile(tx, ty, UndergroundKind::Subway);
        }
    }
    /// PE PlacePipesAtCoordinates (0x44b790) and PlaceSubwayAtCoordinates (0x44bb20).
    fn pe_place_underground_coordinate(
        &mut self,
        x: usize,
        y: usize,
        kind: UndergroundKind,
    ) -> bool {
        let existing = self.city.underground_at(x, y);
        let tile = if existing == 0 {
            kind.base_tile()
        } else if let Some(crossover) = pe_place_underground_crossover_from_existing(kind, existing)
        {
            crossover
        } else {
            return false;
        };
        pe_place_xund_442370(&mut self.city, x, y, tile);
        if kind == UndergroundKind::Pipes {
            self.set_tile_base_pipe_bit(x, y);
        }
        pe_reshape_underground_tile(&mut self.city, x, y, kind);
        for (nx, ny) in neighbor_tiles(x, y) {
            pe_reshape_underground_tile(&mut self.city, nx, ny, kind);
        }
        true
    }
}
