//! Stateful Win95 highway route selection and terminal planning.
//!
//! The planner builds a route against a simulated city copy and returns a plan;
//! the live placement and cost transactions remain in the highway workers.

use super::super::{HighwayDragPlan, HighwayNeighborLinkCandidate, PeHighwayRouteStep461ae0};
use super::water::{
    pe_highway_route_footprint_has_water_xbit_461ae0, pe_highway_water_bridge_candidate_45c1e0,
};
use super::{
    highway_axis_for_pe_direction, highway_canonical_origin,
    pe_city_tool_place_highway_stamp_body_4626d0, pe_highway_altitude_463f60,
    pe_highway_route_gate_low_word_464040, pe_highway_route_step_coordinate_461ae0,
    pe_pipe_alternate_direction_44c6f0, pe_pipe_direction_to_target_44c680,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

pub(in crate::app) fn pe_highway_route_candidate_passes_461ae0(
    city: &City,
    current_height: u16,
    candidate: (usize, usize),
) -> bool {
    if pe_highway_route_gate_low_word_464040(city, candidate.0, candidate.1) == 0x8000 {
        return false;
    }
    let candidate_height = pe_highway_altitude_463f60(city, candidate.0, candidate.1);
    (i32::from(candidate_height) - i32::from(current_height)).abs() <= 1
}

pub(in crate::app) fn pe_highway_origin_touches_outer_edge_461ae0(origin: (usize, usize)) -> bool {
    matches!(origin.0, 0 | 0x7e) || matches!(origin.1, 0 | 0x7e)
}

pub(in crate::app) fn pe_highway_terminal_plan_461ae0(
    simulated_city: &City,
    start: (usize, usize),
    path: Vec<(usize, usize)>,
    current: (usize, usize),
    direction: u8,
) -> HighwayDragPlan {
    let next = pe_highway_route_step_coordinate_461ae0(current, direction);
    // FUN_004621F0 performs this lookahead after the route loop even when the
    // stepper stopped without advancing. A water bit takes precedence over
    // the edge prompt; the water worker may itself refuse the candidate.
    let water_handoff =
        next.filter(|&next| pe_highway_route_footprint_has_water_xbit_461ae0(simulated_city, next));
    let water_bridge = water_handoff.and_then(|seed| {
        pe_highway_water_bridge_candidate_45c1e0(simulated_city, seed, 0, direction)
    });
    let neighbor_link = if water_handoff.is_none()
        && (next.is_none()
            || (path.len() == 1 && pe_highway_origin_touches_outer_edge_461ae0(start)))
    {
        Some(HighwayNeighborLinkCandidate {
            start,
            endpoint: current,
        })
    } else {
        None
    };
    HighwayDragPlan {
        mac_dry_endpoint: None,
        path,
        terminal_direction: None,
        neighbor_link,
        water_bridge,
    }
}

/// One `FUN_00461AE0` step, including the direction write that happens before
/// the candidate's XBIT and final XBLD filters. Route bounds use
/// `(min_x, min_y, max_x, max_y)`.
pub(in crate::app) fn pe_highway_route_step_461ae0(
    city: &City,
    current: (usize, usize),
    endpoint: (usize, usize),
    bounds: (usize, usize, usize, usize),
    direction: &mut u8,
) -> PeHighwayRouteStep461ae0 {
    if current == endpoint {
        return PeHighwayRouteStep461ae0::Stopped;
    }
    let current_gate = pe_highway_route_gate_low_word_464040(city, current.0, current.1);
    if current_gate == 0x8000 {
        return PeHighwayRouteStep461ae0::Stopped;
    }

    // PE bVar2 selects the saved-direction branch for every accepted gate
    // other than 0x0f. At 0x0f, inspect NW, NE, SE, SW XBLD in that order. Each
    // high XBLD must match the direction-adjusted allowlist; a mismatch returns
    // immediately, while any matching cell sets bVar2 for the saved direction.
    let mut follow_saved_direction = current_gate != 0x000f;
    if current_gate == 0x000f {
        for (x, y) in [
            current,
            (current.0 + 1, current.1),
            (current.0 + 1, current.1 + 1),
            (current.0, current.1 + 1),
        ] {
            let xbld = city.building_at(x, y);
            if xbld > 0x0e {
                let adjusted = u16::from(xbld) + u16::from(*direction & 1);
                if matches!(adjusted, 0x001e | 0x000f | 0x002d | 0x0040) {
                    follow_saved_direction = true;
                } else {
                    return PeHighwayRouteStep461ae0::Stopped;
                }
            }
        }
    }

    let current_height = pe_highway_altitude_463f60(city, current.0, current.1);
    let selected = if follow_saved_direction {
        let candidate = pe_highway_route_step_coordinate_461ae0(current, *direction);
        let Some(candidate) = candidate else {
            return PeHighwayRouteStep461ae0::Stopped;
        };
        if candidate.0 < bounds.0
            || candidate.1 < bounds.1
            || candidate.0 > bounds.2
            || candidate.1 > bounds.3
            || !pe_highway_route_candidate_passes_461ae0(city, current_height, candidate)
        {
            return PeHighwayRouteStep461ae0::Stopped;
        }
        (*direction, candidate)
    } else {
        let primary = pe_pipe_direction_to_target_44c680(current, endpoint);
        let Some(primary_candidate) = pe_highway_route_step_coordinate_461ae0(current, primary)
        else {
            return PeHighwayRouteStep461ae0::Stopped;
        };
        // The primary arm checks the drag rectangle before testing the map;
        // if it leaves that rectangle PE returns without trying the alternate.
        if primary_candidate.0 < bounds.0
            || primary_candidate.1 < bounds.1
            || primary_candidate.0 > bounds.2
            || primary_candidate.1 > bounds.3
        {
            return PeHighwayRouteStep461ae0::Stopped;
        }
        let selected =
            if pe_highway_route_candidate_passes_461ae0(city, current_height, primary_candidate) {
                (primary, primary_candidate)
            } else {
                let alternate = pe_pipe_alternate_direction_44c6f0(current, endpoint);
                let Some(alternate_candidate) =
                    pe_highway_route_step_coordinate_461ae0(current, alternate)
                else {
                    return PeHighwayRouteStep461ae0::Stopped;
                };
                if !pe_highway_route_candidate_passes_461ae0(
                    city,
                    current_height,
                    alternate_candidate,
                ) {
                    return PeHighwayRouteStep461ae0::Stopped;
                }
                (alternate, alternate_candidate)
            };
        selected
    };

    // PE stores DAT_004C7D60 at 0x461e81 before these last filters.
    *direction = selected.0;
    if pe_highway_route_footprint_has_water_xbit_461ae0(city, selected.1) {
        return PeHighwayRouteStep461ae0::WaterHandoff;
    }
    let (candidate_x, candidate_y) = selected.1;
    for (x, y) in [
        (candidate_x, candidate_y),
        (candidate_x + 1, candidate_y),
        (candidate_x + 1, candidate_y + 1),
        (candidate_x, candidate_y + 1),
    ] {
        let xbld = city.building_at(x, y);
        if xbld > 0x0e && u16::from(xbld) - u16::from(*direction & 1) == 0x002c {
            return PeHighwayRouteStep461ae0::LateXbldReject;
        }
    }
    PeHighwayRouteStep461ae0::Advanced(selected.1)
}

/// Stateful route planner for the ordinary Win95 highway worker. Each accepted
/// coordinate stamps the prior 2×2 origin before the next step; the caller's
/// final stamp, water lookahead, and edge candidate are kept as terminal state.
pub(in crate::app) fn pe_highway_route_path_461ae0(
    city: &City,
    start: (usize, usize),
    endpoint: (usize, usize),
) -> Option<HighwayDragPlan> {
    let start = highway_canonical_origin(start);
    let endpoint = highway_canonical_origin(endpoint);
    if start.0 > 0x7e || start.1 > 0x7e || endpoint.0 > 0x7e || endpoint.1 > 0x7e {
        return None;
    }
    if pe_highway_route_footprint_has_water_xbit_461ae0(city, start) {
        // The selected-command caller dispatches to the water worker before
        // opening the ordinary drag UI. A worker refusal must not fall through
        // to the geometric dry-route approximation.
        let water_bridge = pe_highway_water_bridge_candidate_45c1e0(city, start, 1, 0);
        return Some(HighwayDragPlan {
            mac_dry_endpoint: None,
            path: Vec::new(),
            terminal_direction: None,
            neighbor_link: None,
            water_bridge,
        });
    }
    if pe_highway_route_gate_low_word_464040(city, start.0, start.1) == 0x8000 {
        // FUN_004621F0 rejects the initial footprint before asking for a drag.
        // Keep that native refusal distinct from an unsupported planner case.
        return Some(HighwayDragPlan {
            mac_dry_endpoint: None,
            path: Vec::new(),
            terminal_direction: None,
            neighbor_link: None,
            water_bridge: None,
        });
    }

    let bounds = highway_drag_bounds(start, endpoint);
    let mut simulated_city = city.clone();
    let mut current = start;
    let mut direction = pe_pipe_direction_to_target_44c680(start, endpoint);
    let mut path = vec![start];
    for _ in 0..(CITY_WIDTH * CITY_HEIGHT) {
        match pe_highway_route_step_461ae0(
            &simulated_city,
            current,
            endpoint,
            bounds,
            &mut direction,
        ) {
            PeHighwayRouteStep461ae0::Advanced(next) => {
                pe_city_tool_place_highway_stamp_body_4626d0(
                    &mut simulated_city,
                    current.0,
                    current.1,
                    highway_axis_for_pe_direction(direction),
                );
                current = next;
                path.push(current);
            }
            PeHighwayRouteStep461ae0::Stopped => {
                pe_city_tool_place_highway_stamp_body_4626d0(
                    &mut simulated_city,
                    current.0,
                    current.1,
                    highway_axis_for_pe_direction(direction),
                );
                let mut plan = pe_highway_terminal_plan_461ae0(
                    &simulated_city,
                    start,
                    path,
                    current,
                    direction,
                );
                plan.terminal_direction = Some(direction);
                return Some(plan);
            }
            PeHighwayRouteStep461ae0::LateXbldReject => {
                // FUN_00461AE0 has already stored `direction` but has not
                // advanced its coordinate arguments. FUN_004621F0 exits the
                // loop and stamps this current origin with the stored axis.
                pe_city_tool_place_highway_stamp_body_4626d0(
                    &mut simulated_city,
                    current.0,
                    current.1,
                    highway_axis_for_pe_direction(direction),
                );
                let mut plan = pe_highway_terminal_plan_461ae0(
                    &simulated_city,
                    start,
                    path,
                    current,
                    direction,
                );
                plan.terminal_direction = Some(direction);
                return Some(plan);
            }
            PeHighwayRouteStep461ae0::WaterHandoff => {
                let seed = pe_highway_route_step_coordinate_461ae0(current, direction)?;
                pe_city_tool_place_highway_stamp_body_4626d0(
                    &mut simulated_city,
                    current.0,
                    current.1,
                    highway_axis_for_pe_direction(direction),
                );
                debug_assert_eq!(
                    pe_highway_route_step_coordinate_461ae0(current, direction),
                    Some(seed)
                );
                let mut plan = pe_highway_terminal_plan_461ae0(
                    &simulated_city,
                    start,
                    path,
                    current,
                    direction,
                );
                plan.terminal_direction = Some(direction);
                return Some(plan);
            }
        }
    }
    None
}

/// CODE 13 @ 0x204ca/0x20658 prices the route against the unchanged map.
/// The Mac worker shares the route and terrain gates with Win95. It prices
/// against the unchanged map and re-runs the route during the live stamp.
pub(super) fn mac_highway_dry_plan(
    city: &City,
    start: (usize, usize),
    raw_endpoint: (usize, usize),
) -> HighwayDragPlan {
    let clicked_without_drag = start == raw_endpoint;
    let start = highway_canonical_origin(start);
    let raw_endpoint = if clicked_without_drag {
        start
    } else {
        raw_endpoint
    };
    let endpoint = highway_canonical_origin(raw_endpoint);
    if start.0 >= CITY_WIDTH
        || start.1 >= CITY_HEIGHT
        || endpoint.0 >= CITY_WIDTH
        || endpoint.1 >= CITY_HEIGHT
    {
        return HighwayDragPlan::default();
    }
    if pe_highway_route_footprint_has_water_xbit_461ae0(city, start) {
        return HighwayDragPlan {
            water_bridge: super::water::mac_highway_water_bridge_candidate(city, start, 1, 0),
            ..HighwayDragPlan::default()
        };
    }
    let mut plan = HighwayDragPlan {
        mac_dry_endpoint: Some(raw_endpoint),
        ..HighwayDragPlan::default()
    };
    if pe_highway_route_gate_low_word_464040(city, start.0, start.1) == 0x8000 {
        return plan;
    }

    let bounds = highway_drag_bounds(start, endpoint);
    let mut current = start;
    let mut direction = pe_pipe_direction_to_target_44c680(start, raw_endpoint);
    plan.path.push(current);
    while current != endpoint {
        let step = pe_highway_route_step_461ae0(city, current, endpoint, bounds, &mut direction);
        match step {
            PeHighwayRouteStep461ae0::Advanced(next) => {
                plan.path.push(next);
                current = next;
            }
            PeHighwayRouteStep461ae0::WaterHandoff
            | PeHighwayRouteStep461ae0::Stopped
            | PeHighwayRouteStep461ae0::LateXbldReject => break,
        }
    }
    plan.terminal_direction = Some(direction);
    plan
}

fn highway_drag_bounds(
    start: (usize, usize),
    endpoint: (usize, usize),
) -> (usize, usize, usize, usize) {
    (
        start.0.min(endpoint.0),
        start.1.min(endpoint.1),
        start.0.max(endpoint.0),
        start.1.max(endpoint.1),
    )
}

pub(super) fn stamp_mac_highway_dry_route(
    city: &mut City,
    start: (usize, usize),
    raw_endpoint: (usize, usize),
) -> (usize, (usize, usize), u8) {
    let mut direction = pe_pipe_direction_to_target_44c680(start, raw_endpoint);
    let mut current = highway_canonical_origin(start);
    let endpoint = highway_canonical_origin(raw_endpoint);
    let bounds = highway_drag_bounds(current, endpoint);
    let mut placed = 0;

    // CODE 13 @ 0x20bf6 advances first, then stamps the old origin using the
    // newly saved direction. The next step reads the mutations from that stamp.
    loop {
        let step = pe_highway_route_step_461ae0(city, current, endpoint, bounds, &mut direction);
        pe_city_tool_place_highway_stamp_body_4626d0(
            city,
            current.0,
            current.1,
            highway_axis_for_pe_direction(direction),
        );
        placed += 1;
        let PeHighwayRouteStep461ae0::Advanced(next) = step else {
            return (placed, current, direction);
        };
        current = next;
    }
}
