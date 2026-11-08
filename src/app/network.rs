//! Shared surface-network models, direction masks, and connection helpers.
//!
//! Strokes, highways, and bridge transactions retain their existing worker bodies.

use super::PlacementSound;
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::ui::EditTool;

mod bridges;
mod city_connections;
mod highways;
mod strokes;

pub(super) use city_connections::{
    highway_neighbor_connection_tiles_for_city, neighbor_connection_tiles_for_city,
    persist_pe_neighbor_connection_overlay, persist_pe_neighbor_connection_overlays,
    rail_neighbor_connection_tiles_for_city, road_neighbor_connection_tiles_for_city,
};

use bridges::network_bridge_building_for_tile;
#[cfg(test)]
pub(super) use bridges::{
    pe_power_bridge_selector_cost_45b930, pe_power_bridge_span_plan, pe_rail_bridge_interior_tile,
    pe_rail_bridge_selector_cost, pe_rail_bridge_span_plan, pe_road_bridge_direction_mask,
    pe_road_bridge_interior_tile, pe_road_bridge_selector_mask, pe_road_bridge_span_plan,
};
pub(super) use bridges::{
    pe_road_bridge_selector_cost, pe_road_bridge_selector_name, pe_road_bridge_selectors,
};
#[cfg(test)]
pub(super) use highways::{
    apply_highway_neighbor_link_confirmation, highway_axis_for_drag_plan_tile,
    highway_axis_for_path_tile, highway_building_for_axis, highway_building_for_mask,
    highway_drag_path, highway_drag_path_for_fidelity, highway_neighbor_link_is_available,
    mac_onramp_compatible_road_mask, onramp_building_for_sides,
    pe_city_tool_place_highway_stamp_body_4626d0, pe_get_highway_tile_462910,
    pe_get_highway_tile_classify_refuse_462956, pe_highway_gate_early_8000_4640a1,
    pe_highway_gate_returns_4000_4642e7, pe_highway_gate_si0_fallthrough_464310,
    pe_highway_neighbor_low_bit_462990, pe_highway_route_gate_low_word_464040,
    pe_highway_route_path_461ae0, pe_highway_route_step_461ae0,
    pe_highway_water_bridge_candidate_45c1e0, pe_highway_water_footprint_class_45c1e0,
    pe_onramp_compatible_road_mask, pe_place_highway_atcoordinates_funds_debit_44c267,
    pe_place_highway_tile_462ed0_class_4_to_7, pe_place_highway_tile_462ed0_class_8_to_c,
    pe_place_highway_tile_462ed0_class_ge_0d, pe_place_highway_tile_462ed0_class_lt4,
    pe_place_highway_tile_463b30,
};
pub(super) use highways::{
    highway_segment_tiles, pe_highway_classify_463c40, pe_highway_neighbor_link_origin_at,
    pe_highway_water_selector_cost_45c1e0, pe_highway_water_selector_name_45c1e0,
    pe_highway_water_selectors_45c1e0, show_highway_neighbor_link_confirmation,
};
use strokes::sub_to_rail_surface_side;
#[cfg(test)]
pub(super) use strokes::{
    apply_rail_neighbor_link_confirmation, apply_road_neighbor_link_confirmation,
    network_commit_path_for_kind, network_drag_path_for_kind, network_preview_path_for_kind,
    pe_network_surface_terrain_result_448e20, pe_network_xtxt_fa_edge_mask_bits_44b0ac,
    pe_place_network_crossover_from_existing, pe_place_power_crossover_from_existing_44a9e8,
    pe_place_power_mask_tile_44ac2c, pe_place_power_reshape_neighbor_connects_44ab30,
    pe_place_power_xtxt_reshape_44aafb, pe_place_rail_mask_tile_44b6a0,
    pe_place_rail_reshape_neighbor_connects_44b485,
    pe_place_rail_slope_from_cardinal_neighbor_44b467, pe_place_rail_xtxt_reshape_44b41d,
    pe_place_rails_crossover_from_existing_44b2f4, pe_place_road_mask_tile_44b0e5,
    pe_place_road_reshape_neighbor_connects_44af6e, pe_place_road_xtxt_reshape_44af50,
    pe_place_roads_crossover_from_existing_44ae24, pe_rail_next_stroke_tile_449f50,
    pe_road_candidate_xbld_allows_step_448f50, pe_road_next_stroke_tile_448f50,
    pe_road_stroke_plan_449bb0, pe_road_stroke_walk_init_448bc0, rail_neighbor_link_is_available,
    road_neighbor_link_is_available,
};
pub(super) use strokes::{
    network_drag_path, pe_network_gate_connects_side, pe_network_surface_edit_448e20,
    pe_network_surface_stroke_cost_448a90, power_line_drag_path,
    show_rail_neighbor_link_confirmation, show_road_neighbor_link_confirmation,
};

/// PE network-stroke cost and terrain gates from FUN_00448A90 / FUN_00448E20.
/// XTER low nibbles 5..=12 add $25; classes 5..=8 and 9..=12 use separate remaps.
pub(super) const PE_NETWORK_SURFACE_COST_GATE_4E7B28: [u8; 16] =
    [0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0];

pub(super) const PE_NETWORK_SURFACE_REMAP_GATE_4E7B68: [u8; 16] =
    [0, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0];

pub(super) const PE_NETWORK_SURFACE_REMAP_4E7BD8: [[u8; 4]; 4] =
    [[2, 1, 2, 1], [2, 3, 2, 3], [4, 3, 4, 3], [4, 1, 4, 1]];

pub(super) const PE_NETWORK_SURFACE_CLEAR_COST: u32 = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PeHighwayRouteStep461ae0 {
    Advanced((usize, usize)),
    Stopped,
    WaterHandoff,
    LateXbldReject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PeHighwayWaterBridgeCandidate {
    pub(super) seed: (usize, usize),
    pub(super) direction: u8,
    pub(super) span_count: usize,
    pub(super) selector_mask: u8,
    pub(super) mode: u8,
}

#[derive(Clone, Debug)]
pub(super) struct HighwayDragPlan {
    pub(super) path: Vec<(usize, usize)>,
    /// PE writes the saved direction before a late XBLD refusal; the caller
    /// then stamps the current origin using that newly selected direction.
    pub(super) terminal_direction: Option<u8>,
    /// Win95 FUN_004621F0 offers the paid neighbor link after ordinary route
    /// tiles have been committed. Keep this separate from trip-overlay XTXT.
    pub(super) neighbor_link: Option<HighwayNeighborLinkCandidate>,
    /// A dry route can hand its accepted direction and water seed to
    /// FUN_0045C1E0 mode 0; a water-start click enters mode 1 directly.
    pub(super) water_bridge: Option<PeHighwayWaterBridgeCandidate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HighwayNeighborLinkCandidate {
    pub(super) start: (usize, usize),
    pub(super) endpoint: (usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RoadNeighborLinkCandidate {
    pub(super) endpoint: (usize, usize),
    pub(super) initial_xtxt: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RailNeighborLinkCandidate {
    pub(super) endpoint: (usize, usize),
}

/// PE MapTool / PlaceHighway funds table @ `0x4dc260` (dword per tool-id).
/// Tool-id 1 (CityTool highway dispatch @ `0x43f4df`) costs `0x64` (100).
/// Tool-id 3 (Onramp) costs `0x19` (25).
pub(super) const PE_TOOL_COST_TABLE_4DC260: [i32; 16] = [
    0x0a, 0x64, 0x96, 0x19, 0xfa, -1, -1, -1, -1, -1, -1, -1, 0x19, 0x64, 0x1f4, 0xfa,
];

/// PE MapTool Onramp funds table alias (tool-id 3 → 25).
pub(super) const PE_MAPTOOL_ONRAMP_COST_TABLE_4DC260: [i32; 16] = PE_TOOL_COST_TABLE_4DC260;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NetworkKind {
    Power,
    Road,
    Rail,
}

impl NetworkKind {
    pub(super) fn base_tile(self) -> u8 {
        match self {
            Self::Power => 14,
            Self::Road => 29,
            Self::Rail => 44,
        }
    }

    pub(super) fn contains(self, building: u8) -> bool {
        match self {
            Self::Power => matches!(building, 14..=28 | 92),
            Self::Road => matches!(building, 29..=43 | 69..=70 | 73..=80 | 81..=89),
            // Mac movement/placement accepts the complete rail domain 0x2c..0x3e;
            // 0x3b..0x3e are the four rotated rail-slope variants.
            Self::Rail => matches!(building, 44..=62 | 71..=72 | 90 | 91),
        }
    }

    pub(super) fn cost_per_tile(self) -> Option<u32> {
        match self {
            Self::Power => Some(2),
            Self::Road => Some(10),
            Self::Rail => Some(25),
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Power => "Power Lines",
            Self::Road => "Road",
            Self::Rail => "Rail",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OnrampPlan {
    pub(super) building: u8,
    pub(super) rotate: bool,
    pub(super) road_tile: (usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SubToRailPlan {
    pub(super) building: u8,
    /// PE adjacency-hit neighbor `(edi,ebx)` for PlaceRails reshape @ `0x441da1`.
    pub(super) reshape_neighbor: (usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NetworkDrag {
    pub(super) start: (usize, usize),
    pub(super) current: (usize, usize),
    pub(super) kind: NetworkKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HighwayDrag {
    pub(super) start: (usize, usize),
    pub(super) current: (usize, usize),
}

pub(super) const NETWORK_TOP: u8 = 0b0001;

pub(super) const NETWORK_RIGHT: u8 = 0b0010;

pub(super) const NETWORK_BOTTOM: u8 = 0b0100;

pub(super) const NETWORK_LEFT: u8 = 0b1000;

pub(super) fn directional_neighbor_tiles(
    x: usize,
    y: usize,
) -> impl Iterator<Item = (u8, usize, usize)> {
    [
        y.checked_sub(1).map(|ny| (NETWORK_TOP, x, ny)),
        (x + 1 < CITY_WIDTH).then_some((NETWORK_RIGHT, x + 1, y)),
        (y + 1 < CITY_HEIGHT).then_some((NETWORK_BOTTOM, x, y + 1)),
        x.checked_sub(1).map(|nx| (NETWORK_LEFT, nx, y)),
    ]
    .into_iter()
    .flatten()
}

pub(super) fn opposite_network_side(side: u8) -> u8 {
    match side {
        NETWORK_TOP => NETWORK_BOTTOM,
        NETWORK_RIGHT => NETWORK_LEFT,
        NETWORK_BOTTOM => NETWORK_TOP,
        NETWORK_LEFT => NETWORK_RIGHT,
        _ => 0,
    }
}

// Mac 68k `0x42fc`: D5 is the adjacent `0x49..0x50` highway mask, and D4 is
// the adjacent `0x1d..0x2b` road mask. The helper accepts only a surviving
// road bit after `D4 &= table[D5]`.
pub(super) const MAC_HIGHWAY_ROAD_COMPATIBILITY: [u8; 16] = [
    0x00, 0x0a, 0x05, 0x0c, 0x0a, 0x00, 0x09, 0x00, 0x05, 0x06, 0x00, 0x00, 0x03, 0x00, 0x00, 0x01,
];

// PE MapTool Onramp `FUN_004417b0` prologue stack table (`0x4417b0`–`0x441830`):
// same as Mac except index `0x0f` is `0` (Mac keeps `0x01`).
pub(super) const PE_HIGHWAY_ROAD_COMPATIBILITY: [u8; 16] = [
    0x00, 0x0a, 0x05, 0x0c, 0x0a, 0x00, 0x09, 0x00, 0x05, 0x06, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NetworkAxis {
    GridX,
    GridY,
}

impl NetworkAxis {
    pub(super) fn perpendicular(self) -> Self {
        match self {
            Self::GridX => Self::GridY,
            Self::GridY => Self::GridX,
        }
    }
}

pub(super) fn proportional_grid_path(
    start: (usize, usize),
    end: (usize, usize),
) -> Vec<(usize, usize)> {
    let nx = start.0.abs_diff(end.0);
    let ny = start.1.abs_diff(end.1);
    let sx = if end.0 >= start.0 { 1 } else { -1 };
    let sy = if end.1 >= start.1 { 1 } else { -1 };
    let mut x = start.0 as isize;
    let mut y = start.1 as isize;
    let mut ix = 0;
    let mut iy = 0;
    let mut path = vec![start];

    while ix < nx || iy < ny {
        if iy >= ny || (ix < nx && (2 * ix + 1) * ny < (2 * iy + 1) * nx) {
            x += sx;
            ix += 1;
        } else {
            y += sy;
            iy += 1;
        }
        path.push((x as usize, y as usize));
    }

    path
}

pub(super) fn alternating_grid_path(
    start: (usize, usize),
    end: (usize, usize),
) -> Vec<(usize, usize)> {
    let nx = start.0.abs_diff(end.0);
    let ny = start.1.abs_diff(end.1);
    let sx = if end.0 >= start.0 { 1 } else { -1 };
    let sy = if end.1 >= start.1 { 1 } else { -1 };
    let mut x = start.0 as isize;
    let mut y = start.1 as isize;
    let mut ix = 0;
    let mut iy = 0;
    let mut take_x = nx >= ny;
    let mut path = vec![start];

    while ix < nx || iy < ny {
        if (take_x && ix < nx) || iy >= ny {
            x += sx;
            ix += 1;
        } else {
            y += sy;
            iy += 1;
        }
        take_x = !take_x;
        path.push((x as usize, y as usize));
    }

    path
}

pub(super) fn axis_aligned_path(start: (usize, usize), end: (usize, usize)) -> Vec<(usize, usize)> {
    axis_aligned_path_with_stride(start, end, 1)
}

pub(super) fn axis_aligned_path_with_stride(
    start: (usize, usize),
    end: (usize, usize),
    stride: usize,
) -> Vec<(usize, usize)> {
    let stride = stride.max(1);
    let mut path = Vec::new();
    if start.0 == end.0 {
        let step = if end.1 >= start.1 {
            stride as isize
        } else {
            -(stride as isize)
        };
        let mut y = start.1 as isize;
        loop {
            path.push((start.0, y as usize));
            if (step > 0 && y + step > end.1 as isize) || (step < 0 && y + step < end.1 as isize) {
                break;
            }
            y += step;
        }
    } else if start.1 == end.1 {
        let step = if end.0 >= start.0 {
            stride as isize
        } else {
            -(stride as isize)
        };
        let mut x = start.0 as isize;
        loop {
            path.push((x as usize, start.1));
            if (step > 0 && x + step > end.0 as isize) || (step < 0 && x + step < end.0 as isize) {
                break;
            }
            x += step;
        }
    } else {
        path.push(start);
        path.push(end);
    }
    path
}

pub(super) fn highway_axis_for_pe_direction(direction: u8) -> NetworkAxis {
    if direction & 1 == 0 {
        NetworkAxis::GridY
    } else {
        NetworkAxis::GridX
    }
}

pub(super) fn network_kind_for_tool(tool: EditTool) -> Option<NetworkKind> {
    match tool {
        EditTool::Power => Some(NetworkKind::Power),
        EditTool::Road => Some(NetworkKind::Road),
        EditTool::Rail => Some(NetworkKind::Rail),
        _ => None,
    }
}

pub(super) fn network_placement_sound(kind: NetworkKind) -> PlacementSound {
    match kind {
        NetworkKind::Power => PlacementSound::PowerLine,
        NetworkKind::Road | NetworkKind::Rail => PlacementSound::Build,
    }
}

pub(super) fn network_kinds_for_building(building: u8) -> impl Iterator<Item = NetworkKind> {
    [NetworkKind::Power, NetworkKind::Road, NetworkKind::Rail]
        .into_iter()
        .filter(move |kind| {
            kind.contains(building)
                || crossover_network_axis(building, *kind).is_some()
                || (*kind == NetworkKind::Road && matches!(building, 93..=96))
                || (*kind == NetworkKind::Rail && matches!(building, 108..=111))
                || building_network_connects_to_kind(building, *kind)
        })
}

pub(super) fn network_building_connects_to(building: u8, kind: NetworkKind, side: u8) -> bool {
    if kind == NetworkKind::Rail
        && let Some(connection) = sub_to_rail_surface_side(building)
    {
        return connection == side;
    }
    if building_network_connects_to_kind(building, kind) {
        return matches!(
            side,
            NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT
        );
    }
    // PE PlaceRoads reshape neighbor accept @ `0x44af6e` includes elevated/`0x43`
    // (`0x3f..0x47` band; gates both 1 through 0x43). Not in Road.contains — avoid
    // refresh reshaping elevated/crossover art. Before crossover_network_axis so
    // power×road 0x43 is all-sides for Road (not GridY-only).
    if kind == NetworkKind::Road && matches!(building, 0x3f..=0x43) {
        return matches!(
            side,
            NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT
        );
    }
    // Same PE band includes 0x44, but NS/EW gates are both 0 — omit before axis.
    if kind == NetworkKind::Road && building == 0x44 {
        return false;
    }
    // Tool crossovers are also in kind.contains (road×rail 69/70, highway×* 75..80);
    // connect only on the PE crossover axis, not every cardinal.
    if let Some(axis) = crossover_network_axis(building, kind) {
        return axis == network_side_axis(side);
    }
    // Road.contains includes plain/foreign highways 73..=80 for placement membership;
    // PE PlaceRoads reshape neighbor accept only 0x4b/0x4c among highway×* (via axis above).
    // Keep 81..=89 (incl. road water bridge 87) connecting for bridge mask runs.
    if kind == NetworkKind::Road && matches!(building, 73..=80) {
        return false;
    }
    kind.contains(building)
}

pub(super) fn building_network_access_kinds(building: u8) -> impl Iterator<Item = NetworkKind> {
    [NetworkKind::Road, NetworkKind::Rail]
        .into_iter()
        .filter(move |kind| building_network_connects_to_kind(building, *kind))
}

pub(super) fn building_network_connects_to_kind(building: u8, kind: NetworkKind) -> bool {
    matches!(
        (building, kind),
        (236, NetworkKind::Road) | (237, NetworkKind::Rail)
    )
}

pub(super) fn network_side_axis(side: u8) -> NetworkAxis {
    match side {
        NETWORK_TOP | NETWORK_BOTTOM => NetworkAxis::GridY,
        _ => NetworkAxis::GridX,
    }
}

pub(super) fn crossover_network_axis(building: u8, kind: NetworkKind) -> Option<NetworkAxis> {
    match (building, kind) {
        // SC2K crossover labels use the canonical tile-art axes. In this viewer's grid,
        // canonical L/R maps to +/-y and canonical T/B maps to +/-x.
        (67, NetworkKind::Power) => Some(NetworkAxis::GridX),
        (67, NetworkKind::Road) => Some(NetworkAxis::GridY),
        (68, NetworkKind::Power) => Some(NetworkAxis::GridY),
        (68, NetworkKind::Road) => Some(NetworkAxis::GridX),
        (69, NetworkKind::Road) => Some(NetworkAxis::GridY),
        (69, NetworkKind::Rail) => Some(NetworkAxis::GridX),
        (70, NetworkKind::Road) => Some(NetworkAxis::GridX),
        (70, NetworkKind::Rail) => Some(NetworkAxis::GridY),
        (71, NetworkKind::Power) => Some(NetworkAxis::GridX),
        (71, NetworkKind::Rail) => Some(NetworkAxis::GridY),
        (72, NetworkKind::Power) => Some(NetworkAxis::GridY),
        (72, NetworkKind::Rail) => Some(NetworkAxis::GridX),
        // Highway crossover labels use the same canonical-to-grid axis swap as the
        // road/rail/power crossover family above.
        (75, NetworkKind::Road) => Some(NetworkAxis::GridX),
        (76, NetworkKind::Road) => Some(NetworkAxis::GridY),
        (77, NetworkKind::Rail) => Some(NetworkAxis::GridX),
        (78, NetworkKind::Rail) => Some(NetworkAxis::GridY),
        (79, NetworkKind::Power) => Some(NetworkAxis::GridX),
        (80, NetworkKind::Power) => Some(NetworkAxis::GridY),
        _ => None,
    }
}

pub(super) fn network_crossing_building_for(
    kind: NetworkKind,
    axis: NetworkAxis,
    existing: u8,
) -> Option<u8> {
    for existing_kind in [NetworkKind::Power, NetworkKind::Road, NetworkKind::Rail] {
        let Some(existing_axis) = straight_network_axis(existing, existing_kind) else {
            continue;
        };
        if existing_kind == kind || existing_axis == axis {
            return None;
        }
        return match ((kind, axis), (existing_kind, existing_axis)) {
            ((NetworkKind::Power, NetworkAxis::GridX), (NetworkKind::Road, NetworkAxis::GridY))
            | ((NetworkKind::Road, NetworkAxis::GridY), (NetworkKind::Power, NetworkAxis::GridX)) => {
                Some(67)
            }
            ((NetworkKind::Power, NetworkAxis::GridY), (NetworkKind::Road, NetworkAxis::GridX))
            | ((NetworkKind::Road, NetworkAxis::GridX), (NetworkKind::Power, NetworkAxis::GridY)) => {
                Some(68)
            }
            ((NetworkKind::Road, NetworkAxis::GridY), (NetworkKind::Rail, NetworkAxis::GridX))
            | ((NetworkKind::Rail, NetworkAxis::GridX), (NetworkKind::Road, NetworkAxis::GridY)) => {
                Some(69)
            }
            ((NetworkKind::Road, NetworkAxis::GridX), (NetworkKind::Rail, NetworkAxis::GridY))
            | ((NetworkKind::Rail, NetworkAxis::GridY), (NetworkKind::Road, NetworkAxis::GridX)) => {
                Some(70)
            }
            ((NetworkKind::Power, NetworkAxis::GridX), (NetworkKind::Rail, NetworkAxis::GridY))
            | ((NetworkKind::Rail, NetworkAxis::GridY), (NetworkKind::Power, NetworkAxis::GridX)) => {
                Some(71)
            }
            ((NetworkKind::Power, NetworkAxis::GridY), (NetworkKind::Rail, NetworkAxis::GridX))
            | ((NetworkKind::Rail, NetworkAxis::GridX), (NetworkKind::Power, NetworkAxis::GridY)) => {
                Some(72)
            }
            _ => None,
        };
    }
    None
}

pub(super) fn network_crossing_building_for_path_tile(
    kind: NetworkKind,
    axis_mask: u8,
    existing: u8,
) -> Option<u8> {
    if let Some(axis) = network_single_axis(axis_mask) {
        return network_crossing_building_for(kind, axis, existing);
    }
    if axis_mask != 0 {
        return None;
    }
    let inferred_axis = inferred_single_tile_crossing_axis(kind, existing)?;
    network_crossing_building_for(kind, inferred_axis, existing)
}

pub(super) fn inferred_single_tile_crossing_axis(
    kind: NetworkKind,
    existing: u8,
) -> Option<NetworkAxis> {
    for existing_kind in [NetworkKind::Power, NetworkKind::Road, NetworkKind::Rail] {
        if existing_kind == kind {
            continue;
        }
        if let Some(existing_axis) = straight_network_axis(existing, existing_kind) {
            return Some(existing_axis.perpendicular());
        }
    }
    None
}

pub(super) fn straight_network_axis(building: u8, kind: NetworkKind) -> Option<NetworkAxis> {
    match (kind, building) {
        (NetworkKind::Power, 14) | (NetworkKind::Road, 29) | (NetworkKind::Rail, 44) => {
            Some(NetworkAxis::GridY)
        }
        (NetworkKind::Power, 15) | (NetworkKind::Road, 30) | (NetworkKind::Rail, 45) => {
            Some(NetworkAxis::GridX)
        }
        _ => None,
    }
}

pub(super) fn network_tile_connects_to(building: u8, kind: NetworkKind, side: u8) -> bool {
    if building_network_connects_to_kind(building, kind) {
        return true;
    }
    if let Some(axis) = crossover_network_axis(building, kind) {
        return axis == network_side_axis(side);
    }
    network_tile_side_mask(building, kind).is_some_and(|mask| mask & side != 0)
}

pub(super) fn network_tile_side_mask(building: u8, kind: NetworkKind) -> Option<u8> {
    if !kind.contains(building) {
        return None;
    }
    let offset = building - kind.base_tile();
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
        // Rail slope variants 59..62 (offsets 15..18) retain cardinal two-sided
        // connectivity while alternating axis with the displayed rotation.
        15 | 17 | 27 | 40 | 44 | 46 | 48 | 50 => NETWORK_TOP | NETWORK_BOTTOM,
        16 | 18 | 28 | 41 | 45 | 47 | 49 | 51 => NETWORK_LEFT | NETWORK_RIGHT,
        _ => return None,
    };
    Some(mask)
}

pub(super) fn network_building_for_mask(kind: NetworkKind, mask: u8) -> u8 {
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

pub(super) fn network_building_for_tile(kind: NetworkKind, mask: u8, terrain: u8) -> u8 {
    if let Some(bridge) = network_bridge_building_for_tile(kind, mask, terrain) {
        return bridge;
    }
    if let Some(slope) = network_slope_building_for_terrain(kind, mask, terrain) {
        return slope;
    }
    network_building_for_mask(kind, mask)
}

pub(super) fn network_slope_building_for_terrain(
    kind: NetworkKind,
    mask: u8,
    terrain: u8,
) -> Option<u8> {
    let base = kind.base_tile();
    let axis_mask = network_axis_mask_for_side_mask(mask);
    let axis = network_single_axis(axis_mask);
    let no_axis = axis_mask == 0;
    match terrain {
        0x01 if no_axis || axis == Some(NetworkAxis::GridY) => Some(base + 2),
        0x02 if no_axis || axis == Some(NetworkAxis::GridX) => Some(base + 3),
        0x03 if no_axis || axis == Some(NetworkAxis::GridY) => Some(base + 4),
        0x04 if no_axis || axis == Some(NetworkAxis::GridX) => Some(base + 5),
        _ => None,
    }
}

pub(super) fn mac_network_slope_building_for_terrain(kind: NetworkKind, terrain: u8) -> Option<u8> {
    let offset = match terrain {
        0x01 => 2,
        0x02 => 3,
        0x03 => 4,
        0x04 => 5,
        _ => return None,
    };
    Some(kind.base_tile() + offset)
}

pub(super) fn network_path_axis_mask(path: &[(usize, usize)], index: usize) -> u8 {
    let Some(&tile) = path.get(index) else {
        return 0;
    };
    let mut mask = 0;
    if index > 0
        && let Some(axis) = network_step_axis(path[index - 1], tile)
    {
        mask |= network_axis_mask(axis);
    }
    if let Some(&next) = path.get(index + 1)
        && let Some(axis) = network_step_axis(tile, next)
    {
        mask |= network_axis_mask(axis);
    }
    mask
}

pub(super) fn network_path_side_mask(path: &[(usize, usize)], index: usize) -> u8 {
    let Some(&tile) = path.get(index) else {
        return 0;
    };
    let mut mask = 0;
    if index > 0 {
        mask |= network_step_side(tile, path[index - 1]);
    }
    if let Some(&next) = path.get(index + 1) {
        mask |= network_step_side(tile, next);
    }
    mask
}

pub(super) fn network_step_side(from: (usize, usize), to: (usize, usize)) -> u8 {
    match (
        to.0 as isize - from.0 as isize,
        to.1 as isize - from.1 as isize,
    ) {
        (0, -1) => NETWORK_TOP,
        (1, 0) => NETWORK_RIGHT,
        (0, 1) => NETWORK_BOTTOM,
        (-1, 0) => NETWORK_LEFT,
        _ => 0,
    }
}

pub(super) fn network_step_axis(a: (usize, usize), b: (usize, usize)) -> Option<NetworkAxis> {
    if a.0 == b.0 && a.1 != b.1 {
        Some(NetworkAxis::GridY)
    } else if a.1 == b.1 && a.0 != b.0 {
        Some(NetworkAxis::GridX)
    } else {
        None
    }
}

pub(super) fn network_axis_mask(axis: NetworkAxis) -> u8 {
    match axis {
        NetworkAxis::GridX => 0b01,
        NetworkAxis::GridY => 0b10,
    }
}

pub(super) fn network_axis_mask_for_side_mask(side_mask: u8) -> u8 {
    let mut mask = 0;
    if side_mask & (NETWORK_LEFT | NETWORK_RIGHT) != 0 {
        mask |= network_axis_mask(NetworkAxis::GridX);
    }
    if side_mask & (NETWORK_TOP | NETWORK_BOTTOM) != 0 {
        mask |= network_axis_mask(NetworkAxis::GridY);
    }
    mask
}

pub(super) fn network_single_axis(mask: u8) -> Option<NetworkAxis> {
    match mask {
        0b01 => Some(NetworkAxis::GridX),
        0b10 => Some(NetworkAxis::GridY),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PeRoadStrokeWalk {
    pub(super) target: (usize, usize),
    pub(super) bounds: (usize, usize, usize, usize),
    pub(super) direction: u8,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PeRoadStrokeStep {
    pub(super) next: Option<(usize, usize)>,
    pub(super) direction: u8,
    pub(super) water_bridge_candidate: Option<(usize, usize)>,
}

#[derive(Debug)]
pub(super) struct PeRoadStrokePlan {
    pub(super) path: Vec<(usize, usize)>,
    pub(super) water_bridge_candidate: Option<(usize, usize)>,
}

#[derive(Debug)]
pub(super) struct PeRoadBridgeSpanPlan {
    pub(super) cells: Vec<(usize, usize)>,
    pub(super) terminal: (usize, usize),
    pub(super) span_count: usize,
    pub(super) selector_mask: u8,
}

pub(super) const PE_ROAD_BRIDGE_SELECTOR_MASK: u8 = 0x1c;

pub(super) const PE_POWER_BRIDGE_SELECTOR_MASK: u8 = 0x01;

pub(super) const HIGHWAY_TILES_PER_SEGMENT: usize = 4;

pub(super) const HIGHWAY_COST_PER_SEGMENT: u32 = 100;

/// PE PlaceHighway slope-walk step tables @ `0x4e7b18` / `0x4e7b20` (dir NESW).
pub(super) const PLACE_HIGHWAY_SLOPE_WALK_DX: [isize; 4] = [0, 1, 0, -1];

pub(super) const PLACE_HIGHWAY_SLOPE_WALK_DY: [isize; 4] = [-1, 0, 1, 0];

pub(super) fn terrain_allows_network_axis(terrain: u8, axis_mask: u8) -> bool {
    match terrain {
        0x01 | 0x03 => axis_mask == 0 || axis_mask == network_axis_mask(NetworkAxis::GridY),
        0x02 | 0x04 => axis_mask == 0 || axis_mask == network_axis_mask(NetworkAxis::GridX),
        0x05..=0x0c => false,
        _ => true,
    }
}
