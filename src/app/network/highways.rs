//! Highway route selection, native stamps, onramps, previews, and neighbor links.

use super::super::{
    PE_PIPE_WALK_DIRECTIONS_4E7B18, ToolPlacement, ToolPreview, ViewerState,
    building_zone_bits_for_tile, footprint_origin, is_highway_tile, is_water_terrain,
    pe_choose_and_place_corner_bits, pe_pipe_alternate_direction_44c6f0,
    pe_pipe_direction_to_target_44c680, pe_place_tile_441f00, projected_tile_units,
};
use super::{
    HIGHWAY_COST_PER_SEGMENT, HIGHWAY_TILES_PER_SEGMENT, HighwayDrag, HighwayDragPlan,
    HighwayNeighborLinkCandidate, NETWORK_LEFT, NETWORK_RIGHT, NetworkAxis, NetworkKind,
    OnrampPlan, PLACE_HIGHWAY_SLOPE_WALK_DX, PLACE_HIGHWAY_SLOPE_WALK_DY,
    directional_neighbor_tiles, highway_axis_for_pe_direction, network_axis_mask,
    network_axis_mask_for_side_mask, network_side_axis,
};
use crate::engine_fidelity::EngineFidelity;
use crate::ui::EditTool;
use glam::Vec2;
use std::collections::HashSet;

mod classify;
mod geometry;
mod neighbor_links;
mod onramps;
mod route;
mod stamps;
mod state;
mod water;

pub(in crate::app) use self::classify::{
    pe_get_highway_tile_462910, pe_highway_altitude_463f60, pe_highway_classify_463c40,
    pe_highway_gate_early_8000_4640a1, pe_highway_route_gate_low_word_464040,
};
#[cfg(test)]
pub(in crate::app) use self::classify::{
    pe_get_highway_tile_classify_refuse_462956, pe_highway_gate_returns_4000_4642e7,
    pe_highway_gate_si0_fallthrough_464310, pe_highway_neighbor_low_bit_462990,
};
#[cfg(test)]
pub(in crate::app) use self::geometry::highway_drag_path;
pub(in crate::app) use self::geometry::{
    highway_axis_for_building, highway_axis_for_drag_plan_tile, highway_axis_for_path_tile,
    highway_bridge_building_for_run_index, highway_building_for_axis, highway_building_for_mask,
    highway_canonical_origin, highway_conflicts_with_axis, highway_crossable_building,
    highway_drag_path_for_fidelity, highway_neighbor_origins, highway_segment_tiles,
    highway_slope_building_for_terrain, is_flat_highway_building, opposite_highway_bridge_building,
    place_highway_slope_delta1_xbld_refused,
};
#[cfg(test)]
pub(in crate::app) use self::neighbor_links::{
    apply_highway_neighbor_link_confirmation, highway_neighbor_link_is_available,
};
pub(in crate::app) use self::neighbor_links::{
    pe_highway_neighbor_link_origin_at, show_highway_neighbor_link_confirmation,
};
pub(in crate::app) use self::onramps::{
    mac_onramp_compatible_road_mask, onramp_building_for_sides, onramp_sides,
    pe_maptool_onramp_funds_debit_44197b, pe_onramp_compatible_road_mask,
    pe_place_highway_atcoordinates_climb_and_terminal_44c347,
    pe_place_highway_atcoordinates_funds_debit_44c267,
    pe_place_highway_atcoordinates_origin_place_44c2a6,
};
pub(in crate::app) use self::route::pe_highway_route_path_461ae0;
#[cfg(test)]
pub(in crate::app) use self::route::pe_highway_route_step_461ae0;
pub(in crate::app) use self::stamps::pe_city_tool_place_highway_clear_xzon_4626d0;
pub(in crate::app) use self::stamps::{
    pe_city_tool_place_highway_stamp_body_4626d0, pe_place_highway_tile_for_stamp_462ed0,
};
#[cfg(test)]
pub(in crate::app) use self::stamps::{
    pe_place_highway_tile_462ed0_class_4_to_7, pe_place_highway_tile_462ed0_class_8_to_c,
    pe_place_highway_tile_462ed0_class_ge_0d, pe_place_highway_tile_462ed0_class_lt4,
    pe_place_highway_tile_463b30,
};
pub(in crate::app) use self::water::pe_highway_water_bridge_candidate_45c1e0;
#[cfg(test)]
pub(in crate::app) use self::water::pe_highway_water_footprint_class_45c1e0;

pub(in crate::app) fn pe_highway_route_step_coordinate_461ae0(
    current: (usize, usize),
    direction: u8,
) -> Option<(usize, usize)> {
    let (dx, dy) = PE_PIPE_WALK_DIRECTIONS_4E7B18[usize::from(direction & 3)];
    let x = current.0.checked_add_signed(dx * 2)?;
    let y = current.1.checked_add_signed(dy * 2)?;
    (x < 0x80 && y < 0x80 && x + 1 < 0x80 && y + 1 < 0x80).then_some((x, y))
}

pub(in crate::app) fn pe_highway_water_selectors_45c1e0(mask: u8) -> Vec<u8> {
    (5u8..=6)
        .filter(|&selector| mask & (1u8 << selector) != 0)
        .collect()
}

pub(in crate::app) fn pe_highway_water_selector_name_45c1e0(selector: u8) -> &'static str {
    match selector {
        5 => "Hiway",
        6 => "Reinforced",
        _ => "Unknown highway",
    }
}

pub(in crate::app) fn pe_highway_water_selector_cost_45c1e0(selector: u8) -> Option<u32> {
    match selector {
        5 => Some(200),
        6 => Some(300),
        _ => None,
    }
}
