//! Road, rail, and power-line strokes, previews, reshaping, and neighbor links.

use super::super::{
    BuildablePrefixPath, PE_PIPE_XTER_SLOPE_ADJUST_4E7BA8, PE_PLACE_PIPES_MASK_TILE,
    PE_PLACE_PIPES_XTXT_GATE, PE_PLACE_PIPES_XTXT_TILE, PE_XUND_EW_GATE_4E7C70,
    PE_XUND_NS_GATE_4E7CB8, ToolPlacement, ToolPreview, ViewerState, ZONE_MILITARY,
    is_tree_building, is_water_terrain, neighbor_tiles, pe_pipe_alternate_direction_44c6f0,
    pe_pipe_direction_for_path_tile, pe_pipe_direction_to_target_44c680,
    pe_pipe_inside_stroke_bounds_44f50, pe_pipe_step_coordinate_44f50,
    pe_pipe_tile_slope_blocks_44f50, pe_pipe_xter_step_blocked_44f50, pe_place_tile_441f00,
    pe_powerline_post_place_power_recalc_43f396, persist_pe_neighbor_connection_overlay,
    push_sprite, sprite_depth, stamp_xbit_powerable, xbld_sprite_info,
};
use super::bridges::{
    can_place_network_bridge_on_building, network_bridge_building_for_tile,
    opposite_rail_bridge_building, pe_road_bridge_mode1_direction,
    rail_bridge_building_for_run_index,
};
use super::highways::onramp_sides;
use super::{
    NETWORK_BOTTOM, NETWORK_LEFT, NETWORK_RIGHT, NETWORK_TOP, NetworkDrag, NetworkKind,
    PE_NETWORK_SURFACE_CLEAR_COST, PE_NETWORK_SURFACE_COST_GATE_4E7B28,
    PE_NETWORK_SURFACE_REMAP_4E7BD8, PE_NETWORK_SURFACE_REMAP_GATE_4E7B68, PeRoadStrokePlan,
    PeRoadStrokeStep, PeRoadStrokeWalk, RailNeighborLinkCandidate, RoadNeighborLinkCandidate,
    SubToRailPlan, alternating_grid_path, axis_aligned_path, crossover_network_axis,
    directional_neighbor_tiles, mac_network_slope_building_for_terrain, network_axis_mask,
    network_axis_mask_for_side_mask, network_building_connects_to, network_building_for_tile,
    network_crossing_building_for_path_tile, network_kind_for_tool, network_path_side_mask,
    network_slope_building_for_terrain, opposite_network_side, proportional_grid_path,
    terrain_allows_network_axis,
};
use crate::city::{CITY_HEIGHT, CITY_TILES, CITY_WIDTH, City, TileBits, ZONE_CORNER_ALL};
use crate::engine_fidelity::EngineFidelity;
use crate::renderer::{RenderSprite, Renderer};
use crate::ui::EditTool;
use glam::Vec2;
use sdl3::messagebox::{
    ButtonData, ClickedButton, MessageBoxButtonFlag, MessageBoxFlag, show_message_box,
};
use std::collections::HashSet;

pub(in crate::app) fn pe_network_surface_terrain_result_448e20(
    xter: u8,
    direction: u8,
) -> Option<(u8, bool)> {
    if xter >= 0x30 {
        return None;
    }
    let class = usize::from(xter & 0x0f);
    if PE_NETWORK_SURFACE_COST_GATE_4E7B28[class] == 0 {
        return None;
    }

    if PE_NETWORK_SURFACE_REMAP_GATE_4E7B68[class] == 0 {
        let mapped = if (0x10..=0x1f).contains(&xter) {
            0x1d
        } else {
            0x0d
        };
        return Some((mapped, true));
    }

    let mapped = PE_NETWORK_SURFACE_REMAP_4E7BD8[class - 9][usize::from(direction & 3)];
    Some((mapped + if xter > 0x0f { 0x10 } else { 0 }, false))
}

pub(in crate::app) fn pe_network_surface_edit_448e20(
    city: &mut City,
    x: usize,
    y: usize,
    direction: u8,
) -> bool {
    let xter = city.terrain_at(x, y);
    let Some((mapped, clears_surface_water)) =
        pe_network_surface_terrain_result_448e20(xter, direction)
    else {
        return false;
    };

    let mut changed = false;
    if mapped != xter {
        city.set_terrain(x, y, mapped);
        changed = true;
    }
    if clears_surface_water
        && let Some(bits) = city.tile_bits_at(x, y)
        && bits.water_tile()
    {
        city.set_tile_bits(x, y, bits.with_water_tile(false));
        changed = true;
    }
    changed
}

pub(in crate::app) fn pe_network_surface_stroke_cost_448a90(
    city: &City,
    path: &[(usize, usize)],
    base_cost_per_tile: u32,
) -> u32 {
    let surcharge_tiles = path
        .iter()
        .filter(|&&(x, y)| {
            let xter = city.terrain_at(x, y);
            xter < 0x30 && PE_NETWORK_SURFACE_COST_GATE_4E7B28[usize::from(xter & 0x0f)] != 0
        })
        .count() as u32;
    (path.len() as u32)
        .saturating_mul(base_cost_per_tile)
        .saturating_add(surcharge_tiles.saturating_mul(PE_NETWORK_SURFACE_CLEAR_COST))
}

pub(in crate::app) fn pe_network_gate_connects_side(tile: u8, side: u8) -> bool {
    let idx = usize::from(tile);
    match side {
        NETWORK_TOP | NETWORK_BOTTOM => PE_XUND_NS_GATE_4E7CB8.get(idx).copied().unwrap_or(0) != 0,
        NETWORK_LEFT | NETWORK_RIGHT => PE_XUND_EW_GATE_4E7C70.get(idx).copied().unwrap_or(0) != 0,
        _ => false,
    }
}

/// PE PlacePowerLines reshape XTXT early arm @ `0x44aad0`–`0x44aafb`:
/// XBLD `0x0e..0x1d` + XTXT gate → PlaceTile(`0x4e7c60[nibble] + 0x0e`).
pub(in crate::app) fn pe_place_power_xtxt_reshape_44aafb(
    city: &mut City,
    x: usize,
    y: usize,
) -> bool {
    let building = city.building_at(x, y);
    if !(0x0e..0x1d).contains(&building) {
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
    let tile = (PE_PLACE_PIPES_XTXT_TILE[idx].wrapping_add(0x0e) & 0xff) as u8;
    pe_place_tile_441f00(city, x, y, tile);
    true
}

/// PE PlacePowerLines reshape mask→tile @ `0x44ac24`–`0x44ac30`:
/// `0x4e7c40[mask] + 0x0e` (no pipes-style mask0→0xf force).
pub(in crate::app) fn pe_place_power_mask_tile_44ac2c(mask: u8) -> u8 {
    let idx = usize::from(mask & 0x0f);
    (PE_PLACE_PIPES_MASK_TILE[idx].wrapping_add(0x0e) & 0xff) as u8
}

/// PE PlaceRoads reshape XTXT early arm @ `0x44af30`–`0x44af50`:
/// XBLD `0x1d..0x2c` + XTXT gate → PlaceTile(`0x4e7c60[nibble] + 0x1d`).
pub(in crate::app) fn pe_place_road_xtxt_reshape_44af50(
    city: &mut City,
    x: usize,
    y: usize,
) -> bool {
    let building = city.building_at(x, y);
    if !(0x1d..0x2c).contains(&building) {
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
    let tile = (PE_PLACE_PIPES_XTXT_TILE[idx].wrapping_add(0x1d) & 0xff) as u8;
    pe_place_tile_441f00(city, x, y, tile);
    true
}

/// PE PlaceRoads reshape mask→tile @ `0x44b0d9`–`0x44b0e5`:
/// `0x4e7c40[mask] + 0x1d` (no pipes-style mask0→0xf force).
pub(in crate::app) fn pe_place_road_mask_tile_44b0e5(mask: u8) -> u8 {
    let idx = usize::from(mask & 0x0f);
    (PE_PLACE_PIPES_MASK_TILE[idx].wrapping_add(0x1d) & 0xff) as u8
}

/// PE PlaceRoads reshape neighbor accept @ `0x44af6e`–`0x44b0a0`: XBLD in
/// `0x1d..0x2c` / `0x3f..0x47` / `0x4b`/`0x4c` / `0x5d..0x61`, then NS/EW gates.
pub(in crate::app) fn pe_place_road_reshape_neighbor_connects_44af6e(
    building: u8,
    side: u8,
) -> bool {
    let in_range = (0x1d..0x2c).contains(&building)
        || (0x3f..0x47).contains(&building)
        || matches!(building, 0x4b | 0x4c)
        || (0x5d..0x61).contains(&building);
    in_range && pe_network_gate_connects_side(building, side)
}

/// PE PlaceRoads coordinate writer admission @ `0x44acf4`–`0x44ad3d`.
/// Existing road/highway/crossing families return before PlaceTile; remaining
/// `XBLD >= 0x0e` values only write when the PE crossover table has a hit.
pub(in crate::app) fn pe_place_roads_coordinate_should_stamp_44acf4(existing: u8) -> bool {
    if (0x1d..0x2c).contains(&existing)
        || (0x3f..0x47).contains(&existing)
        || matches!(existing, 0x4b | 0x4c)
        || (0x5d..0x61).contains(&existing)
    {
        return false;
    }
    existing < 0x0e || pe_place_roads_crossover_from_existing_44ae24(existing).is_some()
}

/// PE PlaceRails coordinate writer admission @ `0x44b1ca`–`0x44b21d`.
/// Existing rail/elevated/tunnel/crossover bands return before PlaceTile;
/// other `XBLD >= 0x0e` values only write on a crossover-table hit.
pub(in crate::app) fn pe_place_rails_coordinate_should_stamp_44b1a0(existing: u8) -> bool {
    if (0x2c..0x3f).contains(&existing)
        || (0x45..0x49).contains(&existing)
        || (0x6c..0x70).contains(&existing)
        || matches!(existing, 0x4d | 0x4e)
    {
        return false;
    }
    existing < 0x0e || pe_place_rails_crossover_from_existing_44b2f4(existing).is_some()
}

/// PE PlaceRoads/PlaceRails reshape XTXT `0xfa` edge OR @ `0x44b0ac` / `0x44b665`:
/// `0x4ca600` is the XTXT row table; when the cell is `0xfa`, OR map-edge bits into the
/// neighbor mask before `0x4e7c40` lookup (`x==0`→L, `x==0x7f`→R, `y==0`→T, `y==0x7f`→B).
pub(in crate::app) fn pe_network_xtxt_fa_edge_mask_bits_44b0ac(x: usize, y: usize, xtxt: u8) -> u8 {
    if xtxt != 0xfa {
        return 0;
    }
    let mut bits = 0u8;
    if x == 0 {
        bits |= NETWORK_LEFT;
    }
    if x + 1 == CITY_WIDTH {
        bits |= NETWORK_RIGHT;
    }
    if y == 0 {
        bits |= NETWORK_TOP;
    }
    if y + 1 == CITY_HEIGHT {
        bits |= NETWORK_BOTTOM;
    }
    bits
}

/// PE PlaceRails reshape XTXT early arm @ `0x44b400`–`0x44b41d`:
/// XBLD `0x2c..0x3b` + XTXT gate → PlaceTile(`0x4e7c60[nibble] + 0x2c`).
pub(in crate::app) fn pe_place_rail_xtxt_reshape_44b41d(
    city: &mut City,
    x: usize,
    y: usize,
) -> bool {
    let building = city.building_at(x, y);
    if !(0x2c..0x3b).contains(&building) {
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
    let tile = (PE_PLACE_PIPES_XTXT_TILE[idx].wrapping_add(0x2c) & 0xff) as u8;
    pe_place_tile_441f00(city, x, y, tile);
    true
}

/// PE PlaceRails reshape mask→tile @ `0x44b692`–`0x44b6a0`:
/// `0x4e7c40[mask] + 0x2c` (no pipes-style mask0→0xf force).
pub(in crate::app) fn pe_place_rail_mask_tile_44b6a0(mask: u8) -> u8 {
    let idx = usize::from(mask & 0x0f);
    (PE_PLACE_PIPES_MASK_TILE[idx].wrapping_add(0x2c) & 0xff) as u8
}

/// PE PlaceRails reshape slope early-outs @ `0x44b467`/`0x44b4f5`/`0x44b57b`/`0x44b5fe`:
/// N=`0x2e`→`0x3b`, S=`0x30`→`0x3d`, W=`0x2f`→`0x3c`, E=`0x31`→`0x3e` (first match).
pub(in crate::app) fn pe_place_rail_slope_from_cardinal_neighbor_44b467(
    city: &City,
    x: usize,
    y: usize,
) -> Option<u8> {
    let probes = [
        (NETWORK_TOP, 0_isize, -1_isize, 0x2e_u8, 0x3b_u8),
        (NETWORK_BOTTOM, 0, 1, 0x30, 0x3d),
        (NETWORK_LEFT, -1, 0, 0x2f, 0x3c),
        (NETWORK_RIGHT, 1, 0, 0x31, 0x3e),
    ];
    for &(_side, dx, dy, need, place) in &probes {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        if !(0..CITY_WIDTH as isize).contains(&nx) || !(0..CITY_HEIGHT as isize).contains(&ny) {
            continue;
        }
        if city.building_at(nx as usize, ny as usize) == need {
            return Some(place);
        }
    }
    None
}

/// PE PlaceRails reshape neighbor accept @ `0x44b485`–`0x44b4c0`: XBLD in
/// `0x2c..0x3f` / `0x45..0x49` / `0x6c..0x70` / `0x4d`/`0x4e`, then NS/EW gates.
pub(in crate::app) fn pe_place_rail_reshape_neighbor_connects_44b485(
    building: u8,
    side: u8,
) -> bool {
    let in_range = (0x2c..0x3f).contains(&building)
        || (0x45..0x49).contains(&building)
        || (0x6c..0x70).contains(&building)
        || matches!(building, 0x4d | 0x4e);
    in_range && pe_network_gate_connects_side(building, side)
}

/// PE PlaceRoads tool existing→crossover @ `0x44ad40` / table `0x44ae24`:
/// `0x0e`→`0x44`, `0x0f`→`0x43`, `0x2c`→`0x46`, `0x2d`→`0x45`, `0x49`→`0x4b`, `0x4a`→`0x4c`.
pub(in crate::app) fn pe_place_roads_crossover_from_existing_44ae24(existing: u8) -> Option<u8> {
    match existing {
        0x0e => Some(0x44),
        0x0f => Some(0x43),
        0x2c => Some(0x46),
        0x2d => Some(0x45),
        0x49 => Some(0x4b),
        0x4a => Some(0x4c),
        _ => None,
    }
}

/// PE PlaceRails tool existing→crossover @ `0x44b220` / table `0x44b2f4`:
/// `0x0e`→`0x48`, `0x0f`→`0x47`, `0x1d`→`0x45`, `0x1e`→`0x46`, `0x49`→`0x4d`, `0x4a`→`0x4e`.
pub(in crate::app) fn pe_place_rails_crossover_from_existing_44b2f4(existing: u8) -> Option<u8> {
    match existing {
        0x0e => Some(0x48),
        0x0f => Some(0x47),
        0x1d => Some(0x45),
        0x1e => Some(0x46),
        0x49 => Some(0x4d),
        0x4a => Some(0x4e),
        _ => None,
    }
}

/// PE PlacePowerLines tool existing→crossover @ `0x449ce0` / table `0x44a9e8`:
/// `0x1d`→`0x43`, `0x1e`→`0x44`, `0x2c`→`0x47`, `0x2d`→`0x48`, `0x49`→`0x4f`, `0x4a`→`0x50`.
pub(in crate::app) fn pe_place_power_crossover_from_existing_44a9e8(existing: u8) -> Option<u8> {
    match existing {
        0x1d => Some(0x43),
        0x1e => Some(0x44),
        0x2c => Some(0x47),
        0x2d => Some(0x48),
        0x49 => Some(0x4f),
        0x4a => Some(0x50),
        _ => None,
    }
}

pub(in crate::app) fn pe_place_network_crossover_from_existing(
    kind: NetworkKind,
    existing: u8,
) -> Option<u8> {
    match kind {
        NetworkKind::Road => pe_place_roads_crossover_from_existing_44ae24(existing),
        NetworkKind::Rail => pe_place_rails_crossover_from_existing_44b2f4(existing),
        NetworkKind::Power => pe_place_power_crossover_from_existing_44a9e8(existing),
    }
}

/// PE PlaceRoads AtCoordinates XZON @ `0x44ada6`: if type nibble `!= 7`, `&= 0xf0`
/// (clear zone type, keep corners). PlaceRails @ `0x44b276` always `&= 0xf0`.
/// PlacePowerLines leaves XZON alone.
pub(in crate::app) fn pe_place_network_clear_zone_type_nibble(
    city: &mut City,
    x: usize,
    y: usize,
    kind: NetworkKind,
) {
    match kind {
        NetworkKind::Power => {}
        NetworkKind::Road => {
            let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
            if zone & 0x0f != ZONE_MILITARY {
                city.set_zone(x, y, zone & 0xf0);
            }
        }
        NetworkKind::Rail => {
            let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
            city.set_zone(x, y, zone & 0xf0);
        }
    }
}

pub(in crate::app) fn pe_network_route_origin_touches_outer_edge(origin: (usize, usize)) -> bool {
    origin.0 == 0 || origin.1 == 0 || origin.0 + 1 == CITY_WIDTH || origin.1 + 1 == CITY_HEIGHT
}

/// PE PlacePowerLines reshape neighbor @ `0x44ab2c`–`0x44ac14`: neighbor needs XBIT
/// `0x80` (powerable) then NS/EW gates on neighbor XBLD (`0x4e7cb8`/`0x4e7c70`).
pub(in crate::app) fn pe_place_power_reshape_neighbor_connects_44ab30(
    city: &City,
    x: usize,
    y: usize,
    side: u8,
) -> bool {
    if !city.tile_bits_at(x, y).is_some_and(|bits| bits.powerable()) {
        return false;
    }
    let building = city.building_at(x, y);
    pe_network_gate_connects_side(building, side)
}

/// PE MapTool SubToRail XTER gate @ `0x441bce`–`0x441bd2`:
/// `mov edx,[y*4+0x4c9f58]` / `cmp byte [edx+x],0` / `je` adjacency (`0x441bde`);
/// non-zero XTER falls through to `xor eax; ret` (refuse). Require flat dry land —
/// opposite polarity from MapTool surface vacant (`XTER < 0x30`).
pub(in crate::app) fn pe_sub_to_rail_xter_refuse_441bd2(city: &City, x: usize, y: usize) -> bool {
    city.terrain_at(x, y) != 0
}

pub(in crate::app) fn sub_to_rail_building_for_side(side: u8) -> Option<u8> {
    match side {
        // Verified against 77 transitions in the stock Win95 cities. The sprite labels use
        // canonical isometric directions, which map to these grid sides in saved XBLD data.
        NETWORK_RIGHT => Some(108),
        NETWORK_BOTTOM => Some(109),
        NETWORK_LEFT => Some(110),
        NETWORK_TOP => Some(111),
        _ => None,
    }
}

/// PE MapTool SubToRail adjacency @ `0x441be8`–`0x441cff`: scan sides E/S/W/N
/// (`0x4e7840`/`0x4e7848`). Rail XBLD in `[0x2c,0x3f)` / `[0x45,0x49)` / `[0x6c,0x70)`
/// keeps that side; else subway XUND `[0x01,0x10)` / `0x1f`/`0x20`/`0x22`/`0x23`
/// flips `si = (si+2)&3`. Neither → refuse.
/// Returns `(si side, neighbor x/edi, neighbor y/ebx)` for PlaceRails @ `0x441da1`.
pub(in crate::app) fn pe_sub_to_rail_adjacent_side_441be8(
    city: &City,
    x: usize,
    y: usize,
) -> Option<(u8, usize, usize)> {
    const DELTAS: [(isize, isize); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    const SIDES: [u8; 4] = [NETWORK_RIGHT, NETWORK_BOTTOM, NETWORK_LEFT, NETWORK_TOP];
    let in_bounds = |nx: isize, ny: isize| {
        (0..CITY_WIDTH as isize).contains(&nx) && (0..CITY_HEIGHT as isize).contains(&ny)
    };
    for (si, &(dx, dy)) in DELTAS.iter().enumerate() {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        if !in_bounds(nx, ny) {
            continue;
        }
        let building = city.building_at(nx as usize, ny as usize);
        if (0x2c..0x3f).contains(&building)
            || (0x45..0x49).contains(&building)
            || (0x6c..0x70).contains(&building)
        {
            return Some((SIDES[si], nx as usize, ny as usize));
        }
    }
    for (si, &(dx, dy)) in DELTAS.iter().enumerate() {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        if !in_bounds(nx, ny) {
            continue;
        }
        let und = city.underground_at(nx as usize, ny as usize);
        if (0x01..0x10).contains(&und) || matches!(und, 0x1f | 0x20 | 0x22 | 0x23) {
            return Some((SIDES[(si + 2) & 3], nx as usize, ny as usize));
        }
    }
    None
}

pub(in crate::app) fn sub_to_rail_surface_side(building: u8) -> Option<u8> {
    match building {
        108 => Some(NETWORK_RIGHT),
        109 => Some(NETWORK_BOTTOM),
        110 => Some(NETWORK_LEFT),
        111 => Some(NETWORK_TOP),
        _ => None,
    }
}

pub(in crate::app) fn network_drag_path(
    start: (usize, usize),
    end: (usize, usize),
) -> Vec<(usize, usize)> {
    if start.0 == end.0 || start.1 == end.1 {
        return axis_aligned_path(start, end);
    }

    proportional_grid_path(start, end)
}

pub(in crate::app) fn network_drag_path_for_kind(
    start: (usize, usize),
    end: (usize, usize),
    kind: NetworkKind,
) -> Vec<(usize, usize)> {
    network_commit_path_for_kind(start, end, kind)
}

pub(in crate::app) fn network_preview_path_for_kind(
    start: (usize, usize),
    end: (usize, usize),
    kind: NetworkKind,
) -> Vec<(usize, usize)> {
    match kind {
        NetworkKind::Power => straight_power_line_drag_path(start, end),
        NetworkKind::Rail => power_line_drag_path(start, end),
        NetworkKind::Road => network_drag_path(start, end),
    }
}

pub(in crate::app) fn network_commit_path_for_kind(
    start: (usize, usize),
    end: (usize, usize),
    kind: NetworkKind,
) -> Vec<(usize, usize)> {
    match kind {
        NetworkKind::Power => straight_power_line_drag_path(start, end),
        NetworkKind::Rail | NetworkKind::Road => network_drag_path(start, end),
    }
}

pub(in crate::app) fn pe_road_start_is_surface_water_449bb0(
    city: &City,
    start: (usize, usize),
) -> bool {
    start.0 < CITY_WIDTH
        && start.1 < CITY_HEIGHT
        && city.terrain_at(start.0, start.1) < 0x40
        && city
            .tile_bits_at(start.0, start.1)
            .is_some_and(TileBits::water_tile)
}

/// Win95 Road mode-0 route initialization in `FUN_00448BC0`.
pub(in crate::app) fn pe_road_stroke_walk_init_448bc0(
    city: &City,
    start: (usize, usize),
    end: (usize, usize),
) -> Option<PeRoadStrokeWalk> {
    if start.0 >= CITY_WIDTH
        || start.1 >= CITY_HEIGHT
        || end.0 >= CITY_WIDTH
        || end.1 >= CITY_HEIGHT
    {
        return None;
    }

    let start_building = city.building_at(start.0, start.1);
    let start_xter = city.terrain_at(start.0, start.1);
    if start_building >= 0x51
        || start_building == 0x05
        || (0x10..=0x1f).contains(&start_xter)
        || (start_building == 0x0d && start_xter < 0x10)
        || city
            .zone_at(start.0, start.1)
            .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
    {
        return None;
    }

    let mut target = end;
    let mut direction = pe_pipe_direction_to_target_44c680(start, end);
    if (1..=4).contains(&start_xter) && pe_pipe_xter_step_blocked_44f50(city, start, direction) {
        direction = pe_pipe_alternate_direction_44c6f0(start, end);
        if pe_pipe_xter_step_blocked_44f50(city, start, direction) {
            target = start;
        }
    }

    Some(PeRoadStrokeWalk {
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

pub(in crate::app) fn pe_road_xbld_keeps_saved_direction_448f50(
    building: u8,
    direction: u8,
) -> bool {
    building >= 0x0e
        && matches!(
            u16::from(building) + u16::from(direction & 1),
            0x0f | 0x1e | 0x2d | 0x4a
        )
}

pub(in crate::app) fn pe_road_candidate_xbld_allows_step_448f50(
    building: u8,
    direction: u8,
) -> bool {
    match building {
        0x05 | 0x0d => false,
        0x00..=0x0c => true,
        _ => pe_road_xbld_keeps_saved_direction_448f50(building, direction),
    }
}

/// Win95 Road mode-0 mutable-cursor step in `FUN_00448F50`. The direction,
/// bounds, XTER step gates, and slope table are shared with the adjacent pipe
/// walker, but surface routing uses XBLD and XBIT instead of XUND.
pub(in crate::app) fn pe_road_next_stroke_tile_448f50(
    city: &City,
    current: (usize, usize),
    target: (usize, usize),
    bounds: (usize, usize, usize, usize),
    direction: u8,
) -> PeRoadStrokeStep {
    let stop = |direction, water_bridge_candidate| PeRoadStrokeStep {
        next: None,
        direction,
        water_bridge_candidate,
    };
    if current == target {
        return stop(direction, None);
    }

    let xter = city.terrain_at(current.0, current.1);
    let current_building = city.building_at(current.0, current.1);
    let surface_class_keeps_direction =
        PE_NETWORK_SURFACE_REMAP_GATE_4E7B68[usize::from(xter & 0x0f)] != 0;
    let follow_saved_direction = surface_class_keeps_direction
        || pe_road_xbld_keeps_saved_direction_448f50(current_building, direction);

    let (next, next_direction) = if follow_saved_direction {
        let Some(next) = pe_pipe_step_coordinate_44f50(current, direction) else {
            return stop(direction, None);
        };
        if !pe_pipe_inside_stroke_bounds_44f50(next, bounds)
            || pe_pipe_xter_step_blocked_44f50(city, next, direction)
        {
            return stop(direction, None);
        }
        (next, direction)
    } else {
        let expected_land = i16::from(city.altitude_at(current.0, current.1) & 0x1f)
            + PE_PIPE_XTER_SLOPE_ADJUST_4E7BA8[usize::from(xter & 0x0f)];
        let primary = pe_pipe_direction_to_target_44c680(current, target);
        let Some(primary_tile) = pe_pipe_step_coordinate_44f50(current, primary) else {
            return stop(direction, None);
        };
        if !pe_pipe_inside_stroke_bounds_44f50(primary_tile, bounds) {
            return stop(direction, None);
        }

        let try_alternate = pe_pipe_xter_step_blocked_44f50(city, primary_tile, primary)
            || pe_pipe_tile_slope_blocks_44f50(city, primary_tile, expected_land);
        if !try_alternate {
            (primary_tile, primary)
        } else {
            let alternate = pe_pipe_alternate_direction_44c6f0(current, target);
            let Some(alternate_tile) = pe_pipe_step_coordinate_44f50(current, alternate) else {
                return stop(direction, None);
            };
            if !pe_pipe_inside_stroke_bounds_44f50(alternate_tile, bounds)
                || pe_pipe_xter_step_blocked_44f50(city, alternate_tile, alternate)
                || pe_pipe_tile_slope_blocks_44f50(city, alternate_tile, expected_land)
            {
                return stop(direction, None);
            }
            (alternate_tile, alternate)
        }
    };

    // The PE stores DAT_004C7D60 before these late refusals. A water candidate
    // is the wrapper's separate bridge-lookahead handoff, not an accepted step.
    let direction = next_direction;
    let next_xter = city.terrain_at(next.0, next.1);
    if next_xter < 0x40
        && city
            .tile_bits_at(next.0, next.1)
            .is_some_and(TileBits::water_tile)
    {
        return stop(direction, Some(next));
    }
    if !pe_road_candidate_xbld_allows_step_448f50(city.building_at(next.0, next.1), direction) {
        return stop(direction, None);
    }

    PeRoadStrokeStep {
        next: Some(next),
        direction,
        water_bridge_candidate: None,
    }
}

/// Read-only route prepass for the Road wrapper's price and bridge handoff.
pub(in crate::app) fn pe_road_stroke_plan_449bb0(
    city: &City,
    start: (usize, usize),
    end: (usize, usize),
) -> Option<PeRoadStrokePlan> {
    let walk = pe_road_stroke_walk_init_448bc0(city, start, end)?;
    let mut path = vec![start];
    let mut current = start;
    let mut direction = walk.direction;

    for _ in 0..(CITY_WIDTH + CITY_HEIGHT) {
        let step =
            pe_road_next_stroke_tile_448f50(city, current, walk.target, walk.bounds, direction);
        direction = step.direction;
        if let Some(next) = step.next {
            path.push(next);
            current = next;
        } else {
            return Some(PeRoadStrokePlan {
                path,
                water_bridge_candidate: step.water_bridge_candidate,
            });
        }
    }

    Some(PeRoadStrokePlan {
        path,
        water_bridge_candidate: None,
    })
}

/// Win95 Rail mode-1 mutable-cursor step in `FUN_00448F50`.
pub(in crate::app) fn pe_rail_next_stroke_tile_449f50(
    city: &City,
    current: (usize, usize),
    target: (usize, usize),
    bounds: (usize, usize, usize, usize),
    direction: u8,
) -> PeRoadStrokeStep {
    let stop = |direction, water_bridge_candidate| PeRoadStrokeStep {
        next: None,
        direction,
        water_bridge_candidate,
    };
    if current == target {
        return stop(direction, None);
    }

    let current_xter = city.terrain_at(current.0, current.1);
    let current_building = city.building_at(current.0, current.1);
    let follow_saved_direction =
        PE_NETWORK_SURFACE_REMAP_GATE_4E7B68[usize::from(current_xter & 0x0f)] != 0
            || pe_road_xbld_keeps_saved_direction_448f50(current_building, direction);

    let (next, next_direction) = if follow_saved_direction {
        let Some(next) = pe_pipe_step_coordinate_44f50(current, direction) else {
            return stop(direction, None);
        };
        if !pe_pipe_inside_stroke_bounds_44f50(next, bounds)
            || pe_pipe_xter_step_blocked_44f50(city, next, direction)
        {
            return stop(direction, None);
        }
        let next_xter = city.terrain_at(next.0, next.1);
        let current_land = city.altitude_at(current.0, current.1) & 0x1f;
        let next_land = city.altitude_at(next.0, next.1) & 0x1f;
        if next_xter != 0 && next_land != current_land {
            return stop(direction, None);
        }
        (next, direction)
    } else {
        let expected_land = i16::from(city.altitude_at(current.0, current.1) & 0x1f)
            + PE_PIPE_XTER_SLOPE_ADJUST_4E7BA8[usize::from(current_xter & 0x0f)];
        let primary = pe_pipe_direction_to_target_44c680(current, target);
        let Some(primary_tile) = pe_pipe_step_coordinate_44f50(current, primary) else {
            return stop(direction, None);
        };
        if !pe_pipe_inside_stroke_bounds_44f50(primary_tile, bounds) {
            return stop(direction, None);
        }

        let try_alternate = pe_pipe_xter_step_blocked_44f50(city, primary_tile, primary)
            || pe_pipe_tile_slope_blocks_44f50(city, primary_tile, expected_land);
        let (next, next_direction) = if !try_alternate {
            (primary_tile, primary)
        } else {
            let alternate = pe_pipe_alternate_direction_44c6f0(current, target);
            let Some(alternate_tile) = pe_pipe_step_coordinate_44f50(current, alternate) else {
                return stop(direction, None);
            };
            if !pe_pipe_inside_stroke_bounds_44f50(alternate_tile, bounds)
                || pe_pipe_xter_step_blocked_44f50(city, alternate_tile, alternate)
                || pe_pipe_tile_slope_blocks_44f50(city, alternate_tile, expected_land)
            {
                return stop(direction, None);
            }
            (alternate_tile, alternate)
        };

        // Selected Rail mode 1 refuses a direction change onto any nonzero
        // XTER candidate after the common axis/slope checks.
        if next_direction != direction && city.terrain_at(next.0, next.1) != 0 {
            return stop(direction, None);
        }
        (next, next_direction)
    };

    // The PE updates DAT_004C7D60 before the XZON, surface-water, and late
    // XBLD refusals; the unchanged cursor is then used for the final probe.
    if city
        .zone_at(next.0, next.1)
        .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
    {
        return stop(next_direction, None);
    }
    let next_xter = city.terrain_at(next.0, next.1);
    if next_xter < 0x40
        && city
            .tile_bits_at(next.0, next.1)
            .is_some_and(TileBits::water_tile)
    {
        return stop(next_direction, Some(next));
    }
    if !pe_road_candidate_xbld_allows_step_448f50(city.building_at(next.0, next.1), next_direction)
    {
        return stop(next_direction, None);
    }

    PeRoadStrokeStep {
        next: Some(next),
        direction: next_direction,
        water_bridge_candidate: None,
    }
}

pub(in crate::app) fn pe_rail_stroke_plan_449f50(
    city: &City,
    start: (usize, usize),
    end: (usize, usize),
) -> Option<PeRoadStrokePlan> {
    let walk = pe_road_stroke_walk_init_448bc0(city, start, end)?;
    let mut path = vec![start];
    let mut current = start;
    let mut direction = walk.direction;
    for _ in 0..(CITY_WIDTH + CITY_HEIGHT) {
        let step =
            pe_rail_next_stroke_tile_449f50(city, current, walk.target, walk.bounds, direction);
        direction = step.direction;
        if let Some(next) = step.next {
            path.push(next);
            current = next;
        } else {
            return Some(PeRoadStrokePlan {
                path,
                water_bridge_candidate: step.water_bridge_candidate,
            });
        }
    }
    Some(PeRoadStrokePlan {
        path,
        water_bridge_candidate: None,
    })
}

pub(in crate::app) fn power_line_drag_path(
    start: (usize, usize),
    end: (usize, usize),
) -> Vec<(usize, usize)> {
    if start.0 == end.0 || start.1 == end.1 {
        return axis_aligned_path(start, end);
    }

    alternating_grid_path(start, end)
}

/// Power-line strokes follow one continuous grid axis. A freehand endpoint may
/// land a tile off-axis; snap that minor component away so the wires do not
/// alternate between the two isometric directions on each step.
pub(in crate::app) fn straight_power_line_drag_path(
    start: (usize, usize),
    end: (usize, usize),
) -> Vec<(usize, usize)> {
    let dx = start.0.abs_diff(end.0);
    let dy = start.1.abs_diff(end.1);
    if dx >= dy {
        axis_aligned_path(start, (end.0, start.1))
    } else {
        axis_aligned_path(start, (start.0, end.1))
    }
}

mod actions;
mod confirmation;
pub(in crate::app) use confirmation::*;
mod topology;
