//! Mac CODE 14 powered transit-route model and search worker.
//!
//! Seed priority, transition gates, the bounded weighted search, and
//! successful route effects are modeled here; route backtracking follows
//! the native direction-mask evidence captured in the disassembly ledger.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum MacTransitRouteTransition {
    Reject,
    Accept { next_state: u8, cost: u16 },
    Success,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::app) struct MacTransitRouteCell {
    pub building: u8,
    pub zone_low: u8,
    pub tunnel_levels: u8,
    pub underground: u8,
    pub text_overlay: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct MacTransitRouteResult {
    /// Accepted route cells, from the first seed through the successful end.
    /// The terminal zone/edge cell itself is not part of the native deque.
    pub route: Vec<(usize, usize, u8)>,
    pub cost: u16,
    pub random_draws: usize,
}

#[derive(Clone, Copy, Debug)]
struct MacTransitRouteNode {
    x: usize,
    y: usize,
    state: u8,
    directions_left: u8,
    cost: u16,
}

const MAC_TRANSIT_ROUTE_SEED_OFFSETS: [(isize, isize); 24] = [
    (0, 1),
    (1, 0),
    (0, -1),
    (-1, 0),
    (0, 2),
    (2, 0),
    (0, -2),
    (-2, 0),
    (0, 3),
    (3, 0),
    (0, -3),
    (-3, 0),
    (1, 1),
    (-1, 1),
    (1, -1),
    (-1, -1),
    (2, 1),
    (-2, 1),
    (2, -1),
    (-2, -1),
    (1, 2),
    (-1, 2),
    (1, -2),
    (-1, -2),
];

/// Mac CODE 14 `export_217`'s ordered 24-cell transit-family seed scan.
/// Returns the first seed coordinate and route state, without consuming RNG.
pub(in crate::app) fn mac_transit_route_seed<F>(
    x: usize,
    y: usize,
    mut building_at: F,
) -> Option<(usize, usize, u8)>
where
    F: FnMut(usize, usize) -> u8,
{
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return None;
    }
    for (dx, dy) in MAC_TRANSIT_ROUTE_SEED_OFFSETS {
        let sx = x as isize + dx;
        let sy = y as isize + dy;
        if !(0..CITY_WIDTH as isize).contains(&sx) || !(0..CITY_HEIGHT as isize).contains(&sy) {
            continue;
        }
        let sx = sx as usize;
        let sy = sy as usize;
        let tile = building_at(sx, sy);
        let state = if matches!(tile, 0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c | 0x5d..=0x60) {
            Some(0)
        } else {
            match tile {
                0xec => Some(8),
                0xed => Some(10),
                0xe9 => Some(11),
                _ => None,
            }
        };
        if let Some(state) = state {
            return Some((sx, sy, state));
        }
    }
    None
}

/// CODE 8 `FUN_00036c8c`: current cell or an accepted cardinal neighbor has
/// XBIT powered (`0x40`). The native helper skips the negative-axis neighbor
/// when its coordinate is 0 or 1, but accepts the corresponding positive-axis
/// neighbor from coordinate 0. The Mac power pass rebuilds this transient plane.
pub(in crate::app) fn mac_transit_has_powered_adjacency(city: &City, x: usize, y: usize) -> bool {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return false;
    }
    let powered = |nx, ny| city.tile_bits_at(nx, ny).is_some_and(TileBits::powered);
    powered(x, y)
        || (x > 1 && powered(x - 1, y))
        || (y > 1 && powered(x, y - 1))
        || (x < 0x7f && powered(x + 1, y))
        || (y < 0x7f && powered(x, y + 1))
}

/// CODE 14 `export_217` neighbor-switch transition, transcribed from the Mac
/// 1.2 worker. The route worker below owns direction order and backtracking;
/// this function models the switch's local gates.
pub(in crate::app) fn mac_transit_route_transition(
    state: u8,
    zone_category: usize,
    neighbor_zone_low: u8,
    building: u8,
    tunnel_levels: u8,
    underground: u8,
) -> MacTransitRouteTransition {
    use MacTransitRouteTransition::{Accept, Reject, Success};

    const ZONE_MASKS: [u16; 7] = [0xffff, 0xfff8, 0xfff8, 0xffe6, 0xffe6, 0xff9e, 0xff9e];
    let zone_mask_matches = ZONE_MASKS
        .get(zone_category)
        .is_some_and(|mask| mask & (1_u16 << (neighbor_zone_low & 0x0f)) != 0);
    let road = matches!(building, 0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c | 0x5d..=0x60);
    let ordinary_surface = matches!(building, 0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c);
    let bridge = matches!(building, 0x51..=0x5c | 0x6a..=0x6b);
    let highway = matches!(building, 0x49..=0x50 | 0x61..=0x6b);
    let rail = matches!(building, 0x2c..=0x3e | 0x45..=0x48 | 0x6c..=0x6f | 0x4d..=0x4e);
    let xund_connected = matches!(underground, 0x01..=0x0f | 0x1f | 0x20 | 0x22 | 0x23);

    match state {
        0 | 4 | 8 | 9 if zone_mask_matches => Success,
        0 if matches!(building, 0x3f..=0x42) => Accept {
            next_state: 2,
            cost: 3,
        },
        0 if bridge => Accept {
            next_state: 3,
            cost: 3,
        },
        0 if matches!(building, 0x5d..=0x60) => Accept {
            next_state: 1,
            cost: 2,
        },
        0 if building == 0xec => Accept {
            next_state: 8,
            cost: 4,
        },
        0 if building == 0xed => Accept {
            next_state: 10,
            cost: 4,
        },
        0 if building == 0xe9 => Accept {
            next_state: 11,
            cost: 4,
        },
        0 if ordinary_surface => Accept {
            next_state: 0,
            cost: 3,
        },
        0 if building > 0xfa => Success,

        1 if highway => Accept {
            next_state: 1,
            cost: 1,
        },
        1 if matches!(building, 0x5d..=0x60) => Accept {
            next_state: 0,
            cost: 1,
        },
        2 if tunnel_levels != 0 => Accept {
            next_state: 2,
            cost: 3,
        },
        2 if road => Accept {
            next_state: 0,
            cost: 3,
        },
        3 if bridge => Accept {
            next_state: 3,
            cost: 3,
        },
        3 if road => Accept {
            next_state: 0,
            cost: 3,
        },

        4 if matches!(building, 0x3f..=0x42) => Accept {
            next_state: 6,
            cost: 2,
        },
        4 if bridge => Accept {
            next_state: 7,
            cost: 2,
        },
        4 if matches!(building, 0x5d..=0x60) => Accept {
            next_state: 5,
            cost: 2,
        },
        4 if building == 0xec => Accept {
            next_state: 9,
            cost: 4,
        },
        4 if building == 0xed => Accept {
            next_state: 10,
            cost: 4,
        },
        4 if building == 0xe9 => Accept {
            next_state: 11,
            cost: 4,
        },
        4 if ordinary_surface => Accept {
            next_state: 4,
            cost: 2,
        },
        4 if building > 0xfa => Success,
        5 if highway => Accept {
            next_state: 5,
            cost: 1,
        },
        5 if matches!(building, 0x5d..=0x60) => Accept {
            next_state: 4,
            cost: 1,
        },
        6 if tunnel_levels != 0 => Accept {
            next_state: 6,
            cost: 2,
        },
        6 if road => Accept {
            next_state: 4,
            cost: 2,
        },
        7 if bridge => Accept {
            next_state: 7,
            cost: 2,
        },
        7 if road => Accept {
            next_state: 4,
            cost: 2,
        },

        8 if building == 0xec => Accept {
            next_state: 8,
            cost: 4,
        },
        8 if road => Accept {
            next_state: 4,
            cost: 2,
        },
        9 if matches!(building, 0xec | 0xed) => Accept {
            next_state: 9,
            cost: 4,
        },
        9 if road => Accept {
            next_state: 0,
            cost: 3,
        },
        10 if building == 0xed => Accept {
            next_state: 10,
            cost: 4,
        },
        10 if rail => Accept {
            next_state: 12,
            cost: 1,
        },
        11 if xund_connected => Accept {
            next_state: 13,
            cost: 1,
        },
        12 if building == 0xed => Accept {
            next_state: 9,
            cost: 4,
        },
        12 if rail => Accept {
            next_state: 12,
            cost: 1,
        },
        12 if building > 0xfa => Success,
        13 if building == 0xe9 => Accept {
            next_state: 9,
            cost: 4,
        },
        13 if xund_connected => Accept {
            next_state: 13,
            cost: 1,
        },
        _ => Reject,
    }
}

/// Mac CODE 14 `export_217` weighted depth-first route search. The supplied
/// random callback is one QuickDraw `_Random` result per native draw site;
/// callers retain ownership of the process-wide stream and its chronology.
pub(in crate::app) fn mac_transit_route_search<F, R>(
    x: usize,
    y: usize,
    zone_category: usize,
    coefficient: u16,
    cost_limit: u16,
    mut cell_at: F,
    mut random_i16: R,
) -> Option<MacTransitRouteResult>
where
    F: FnMut(usize, usize) -> MacTransitRouteCell,
    R: FnMut() -> i16,
{
    let cost_limit = if coefficient == 1 {
        cost_limit - cost_limit / 4
    } else {
        cost_limit
    };
    let (seed_x, seed_y, seed_state) =
        mac_transit_route_seed(x, y, |sx, sy| cell_at(sx, sy).building)?;

    // The native table at A5-0x4426 is [0x0b, 0x07, 0x0e, 0x0d]. These are
    // the remaining direction masks after entering from W/S/E/N respectively.
    const ENTERED_DIRECTION_MASKS: [u8; 4] = [0x0b, 0x07, 0x0e, 0x0d];
    const DIRECTIONS: [(isize, isize); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];

    let mut draws = 0;
    let stride = ((random_i16() as u16 & 1) * 2 + 1) as u8;
    draws += 1;
    let mut route = vec![MacTransitRouteNode {
        x: seed_x,
        y: seed_y,
        state: seed_state,
        directions_left: 0x0f,
        cost: 0,
    }];
    let mut cost = 0_u16;
    let mut succeeded = false;

    while !succeeded && cost < cost_limit {
        // CODE 14 removes exhausted deque entries before choosing another
        // direction. In particular, the parent can already have a zero mask
        // when its final remaining direction accepted the child. Collapse
        // that whole exhausted suffix without consuming `_Random` words.
        while route.last().is_some_and(|node| node.directions_left == 0) {
            route.pop();
            if let Some(parent) = route.last() {
                cost = parent.cost;
            } else {
                break;
            }
        }
        if route.is_empty() {
            break;
        }

        let mut direction = (random_i16() as u16 & 3) as u8;
        draws += 1;
        let mut accepted = false;
        for _ in 0..4 {
            direction = direction.wrapping_add(stride) & 3;
            let bit = 1_u8 << direction;
            let Some((current_x, current_y, current_state, direction_was_left)) =
                route.last_mut().map(|node| {
                    let direction_was_left = node.directions_left & bit != 0;
                    if direction_was_left {
                        node.directions_left &= !bit;
                    }
                    (node.x, node.y, node.state, direction_was_left)
                })
            else {
                break;
            };
            if !direction_was_left {
                continue;
            }

            let (dx, dy) = DIRECTIONS[usize::from(direction)];
            let nx = current_x as isize + dx;
            let ny = current_y as isize + dy;
            if !(0..CITY_WIDTH as isize).contains(&nx) || !(0..CITY_HEIGHT as isize).contains(&ny) {
                succeeded = cell_at(current_x, current_y).text_overlay == 0xfa;
                accepted = succeeded;
                if succeeded {
                    break;
                }
                continue;
            }

            let next_x = nx as usize;
            let next_y = ny as usize;
            let next = cell_at(next_x, next_y);
            match mac_transit_route_transition(
                current_state,
                zone_category,
                next.zone_low,
                next.building,
                next.tunnel_levels,
                next.underground,
            ) {
                MacTransitRouteTransition::Reject => {}
                MacTransitRouteTransition::Success => {
                    succeeded = true;
                    accepted = true;
                    break;
                }
                MacTransitRouteTransition::Accept {
                    next_state,
                    cost: step_cost,
                } => {
                    cost = cost.wrapping_add(step_cost);
                    let directions_left = match next_state {
                        3 | 7 => bit,
                        11 => 0x0f,
                        _ => ENTERED_DIRECTION_MASKS[usize::from(direction)],
                    };
                    route.push(MacTransitRouteNode {
                        x: next_x,
                        y: next_y,
                        state: next_state,
                        directions_left,
                        cost,
                    });
                    accepted = true;
                    break;
                }
            }
        }

        if succeeded {
            break;
        }
        if !accepted {
            route.pop();
            if let Some(parent) = route.last() {
                cost = parent.cost;
            } else {
                break;
            }
        }
    }

    succeeded.then(|| MacTransitRouteResult {
        route: route
            .into_iter()
            .map(|node| (node.x, node.y, node.state))
            .collect(),
        cost,
        random_draws: draws,
    })
}

/// Apply the successful-path writes at CODE 14 export 217's epilogue.
pub(in crate::app) fn mac_transit_apply_route_success(
    city: &mut City,
    result: &MacTransitRouteResult,
    coefficient: u16,
) {
    let mut saw_bus = false;
    let mut saw_rail = false;
    let mut saw_subway = false;
    for &(x, y, state) in &result.route {
        match state {
            0 | 1 | 3 => {
                let current = u16::from(city.traffic_density_at(x, y).unwrap_or(0));
                let updated = current.saturating_add(coefficient).min(0xff) as u8;
                city.set_traffic_density(x, y, updated);
            }
            8 => saw_bus = true,
            10 => saw_rail = true,
            11 => saw_subway = true,
            _ => {}
        }
    }
    for (mode, encountered) in [saw_bus, saw_rail, saw_subway].into_iter().enumerate() {
        if encountered {
            let prior = city.mac_transit_accumulator(mode);
            city.set_mac_transit_accumulator(mode, prior.wrapping_add(u32::from(coefficient)));
        }
    }
}
