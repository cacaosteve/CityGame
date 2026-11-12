//! Highway drag geometry and tile/orientation mappings.
//!
//! These helpers describe candidate paths and translate their axes to native
//! highway building classes; city-aware routing and placement remain separate.

use super::super::{
    HIGHWAY_TILES_PER_SEGMENT, HighwayDragPlan, NETWORK_BOTTOM, NETWORK_LEFT, NETWORK_RIGHT,
    NETWORK_TOP, NetworkAxis, NetworkKind, highway_axis_for_pe_direction, network_path_axis_mask,
    network_single_axis, network_step_axis, straight_network_axis,
};
use super::is_highway_tile;
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::engine_fidelity::EngineFidelity;

pub(in crate::app) fn highway_drag_path(
    start: (usize, usize),
    end: (usize, usize),
) -> Vec<(usize, usize)> {
    let mut current = highway_canonical_origin(start);
    let end = highway_canonical_origin(end);
    let mut path =
        Vec::with_capacity(current.0.abs_diff(end.0) / 2 + current.1.abs_diff(end.1) / 2 + 1);
    path.push(current);

    // PE FUN_0044C680 chooses the larger remaining map-coordinate delta and
    // chooses y when the deltas tie. FUN_00461AE0 repeats that choice after
    // each accepted segment, which produces a stair-step route on a clear map.
    // City-aware refusal/alternate-step handling remains in the placement
    // pass; this helper supplies the unobstructed geometric baseline.
    while current != end {
        let dx = current.0.abs_diff(end.0);
        let dy = current.1.abs_diff(end.1);
        if dy >= dx && dy != 0 {
            current.1 = if current.1 < end.1 {
                current.1.saturating_add(2)
            } else {
                current.1.saturating_sub(2)
            };
        } else if dx != 0 {
            current.0 = if current.0 < end.0 {
                current.0.saturating_add(2)
            } else {
                current.0.saturating_sub(2)
            };
        } else {
            break;
        }
        path.push(current);
    }
    path
}

pub(in crate::app) fn highway_canonical_origin(tile: (usize, usize)) -> (usize, usize) {
    (tile.0 & !1, tile.1 & !1)
}

pub(in crate::app) fn highway_axis_for_path_tile(
    fidelity: EngineFidelity,
    path: &[(usize, usize)],
    index: usize,
) -> NetworkAxis {
    if fidelity == EngineFidelity::Mac68k {
        return network_single_axis(network_path_axis_mask(path, index))
            .unwrap_or(NetworkAxis::GridX);
    }
    let Some(&origin) = path.get(index) else {
        return NetworkAxis::GridX;
    };
    // PE FUN_004621F0 calls FUN_00461AE0 first, then stamps the old origin
    // with FUN_004626D0 using the direction FUN_00461AE0 just saved. At a
    // stair-step turn, that is the outgoing leg, not the union of the
    // incoming and outgoing path axes.
    if let Some(&next) = path.get(index + 1)
        && let Some(axis) = network_step_axis(origin, next)
    {
        return axis;
    }
    if index > 0
        && let Some(axis) = network_step_axis(path[index - 1], origin)
    {
        return axis;
    }
    // A one-origin drag stamps before any successful step. FUN_00461A00
    // initializes the saved direction with FUN_0044C680; its zero-delta tie
    // falls through to the y axis (direction 2), so this singleton is GridY.
    if path.len() == 1 && index == 0 {
        return NetworkAxis::GridY;
    }
    NetworkAxis::GridX
}

pub(in crate::app) fn highway_axis_for_drag_plan_tile(
    fidelity: EngineFidelity,
    plan: &HighwayDragPlan,
    index: usize,
) -> NetworkAxis {
    if let Some(direction) = plan.terminal_direction
        && index + 1 == plan.path.len()
    {
        return highway_axis_for_pe_direction(direction);
    }
    highway_axis_for_path_tile(fidelity, &plan.path, index)
}

pub(in crate::app) fn highway_building_for_axis(axis: NetworkAxis) -> u8 {
    match axis {
        NetworkAxis::GridY => 73,
        NetworkAxis::GridX => 74,
    }
}

pub(in crate::app) fn highway_bridge_building_for_run_index(index: usize) -> u8 {
    if index.is_multiple_of(2) { 107 } else { 106 }
}

pub(in crate::app) fn opposite_highway_bridge_building(building: u8) -> u8 {
    match building {
        106 => 107,
        107 => 106,
        _ => 107,
    }
}

pub(in crate::app) fn highway_slope_building_for_terrain(
    axis: NetworkAxis,
    terrain: u8,
) -> Option<u8> {
    match (axis, terrain) {
        (NetworkAxis::GridY, 0x01) => Some(97),
        (NetworkAxis::GridX, 0x02) => Some(98),
        (NetworkAxis::GridY, 0x03) => Some(99),
        (NetworkAxis::GridX, 0x04) => Some(100),
        _ => None,
    }
}
/// PE PlaceHighway delta==1 XBLD refuse @ `0x44bfea`–`0x44c057` (dialog `0x73`).
pub(in crate::app) fn place_highway_slope_delta1_xbld_refused(xbld: u8) -> bool {
    matches!(xbld, 1..=0x20 | 0x22 | 0x23)
}

pub(in crate::app) fn highway_axis_for_building(building: u8) -> NetworkAxis {
    match building {
        73 | 75 | 77 | 79 | 97 | 99 => NetworkAxis::GridY,
        74 | 76 | 78 | 80 | 98 | 100 => NetworkAxis::GridX,
        _ => NetworkAxis::GridX,
    }
}

pub(in crate::app) fn highway_conflicts_with_axis(building: u8, axis: NetworkAxis) -> bool {
    is_highway_tile(building) && highway_axis_for_building(building) != axis
}

pub(in crate::app) fn is_flat_highway_building(building: u8) -> bool {
    matches!(building, 73 | 74)
}

pub(in crate::app) fn highway_crossable_building(building: u8) -> bool {
    straight_network_axis(building, NetworkKind::Road).is_some()
        || straight_network_axis(building, NetworkKind::Rail).is_some()
        || straight_network_axis(building, NetworkKind::Power).is_some()
}

pub(in crate::app) fn highway_building_for_mask(mask: u8) -> u8 {
    match mask {
        m if m == (NETWORK_TOP | NETWORK_BOTTOM) => 73,
        m if m == (NETWORK_LEFT | NETWORK_RIGHT) => 74,
        m if m == (NETWORK_TOP | NETWORK_RIGHT) => 101,
        m if m == (NETWORK_RIGHT | NETWORK_BOTTOM) => 102,
        m if m == (NETWORK_BOTTOM | NETWORK_LEFT) => 103,
        m if m == (NETWORK_LEFT | NETWORK_TOP) => 104,
        m if m.count_ones() >= 3 => 105,
        m if m == NETWORK_TOP || m == NETWORK_BOTTOM => 73,
        m if m == NETWORK_LEFT || m == NETWORK_RIGHT => 74,
        _ => 74,
    }
}

pub(in crate::app) fn highway_segment_tiles(
    origin: (usize, usize),
    axis: NetworkAxis,
) -> Vec<(usize, usize)> {
    let _axis = axis;
    (origin.1..=origin.1 + 1)
        .flat_map(|y| (origin.0..=origin.0 + 1).map(move |x| (x, y)))
        .filter(|&(x, y)| x < CITY_WIDTH && y < CITY_HEIGHT)
        .collect()
}

pub(in crate::app) fn highway_neighbor_origins(
    origin: (usize, usize),
) -> [Option<(u8, (usize, usize))>; HIGHWAY_TILES_PER_SEGMENT] {
    let (x, y) = origin;
    [
        y.checked_sub(2).map(|ny| (NETWORK_TOP, (x, ny))),
        (x + 2 < CITY_WIDTH).then_some((NETWORK_RIGHT, (x + 2, y))),
        (y + 2 < CITY_HEIGHT).then_some((NETWORK_BOTTOM, (x, y + 2))),
        x.checked_sub(2).map(|nx| (NETWORK_LEFT, (nx, y))),
    ]
}
