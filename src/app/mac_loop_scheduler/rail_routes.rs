//! Mac CODE 6 actor-route helpers. Rail-head movement is scheduled by the Mac
//! actor sweep; station-based creation is called from the monthly map scan.

use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

/// CODE 6 `FUN_0000ef44`: candidate gate shared by kind 10 and other actors.
#[allow(dead_code)]
pub(super) fn mac_actor_route_candidate(city: &City, row: i16, col: i16, kind: u16) -> bool {
    if row < 0 || col < 0 || row as usize >= CITY_HEIGHT || col as usize >= CITY_WIDTH {
        return false;
    }
    // CODE 6 computes the map pointer as tile_x * 128 + tile_y. City planes
    // are x-major too, so these actor bytes map directly to City (x, y).
    let (x, y) = (row as usize, col as usize);
    if city.text_overlay_at(x, y).unwrap_or(0) >= 0xc9 {
        return false;
    }
    let building = city.building_at(x, y);
    if kind == 10 {
        mac_rail_surface_candidate(building)
    } else {
        mac_actor_underground_candidate(city.underground_at(x, y))
            || (0x6c..=0x70).contains(&building)
    }
}

fn mac_rail_surface_candidate(building: u8) -> bool {
    matches!(building, 0x2c..=0x3e | 0x45..=0x48 | 0x4d | 0x4e | 0x5a | 0x5b | 0x6c..=0x6f)
}

fn mac_actor_underground_candidate(underground: u8) -> bool {
    matches!(underground, 1..=0x0f | 0x1f | 0x20 | 0x22 | 0x23)
}

/// CODE 6 `FUN_0000f052`: nonzero means the train's current cell triggers
/// its teardown branch. The original reads XBLD and XUND, not XZON or XTXT.
#[allow(dead_code)]
pub(super) fn mac_rail_current_cell_requires_teardown(city: &City, row: i16, col: i16) -> bool {
    if row < 0 || col < 0 || row as usize >= CITY_HEIGHT || col as usize >= CITY_WIDTH {
        return false;
    }
    let (x, y) = (row as usize, col as usize);
    let building = city.building_at(x, y);
    !mac_rail_surface_candidate(building)
        && building != 0x70
        && !mac_actor_underground_candidate(city.underground_at(x, y))
}

/// CODE 6 `FUN_0000f3ee`: one app-game-LCG bound-2 draw, then up to four
/// adjacent checks. The callback represents that draw, not QuickDraw.
#[allow(dead_code)]
pub(super) fn mac_actor_choose_route(
    city: &City,
    row: i16,
    col: i16,
    offset: i16,
    kind: u16,
    draw_bound_two: impl FnOnce() -> u16,
) -> Option<u8> {
    const DROW: [i16; 4] = [0, 1, 0, -1];
    const DCOL: [i16; 4] = [-1, 0, 1, 0];
    let order = if draw_bound_two() == 0 {
        [0_i16, 3, 1, 2]
    } else {
        [0_i16, 1, 3, 2]
    };
    for candidate in order {
        let direction = (candidate.wrapping_add(offset) & 3) as usize;
        let next_row = row.wrapping_add(DROW[direction]);
        let next_col = col.wrapping_add(DCOL[direction]);
        if mac_actor_route_candidate(city, next_row, next_col, kind) {
            return Some(direction as u8);
        }
    }
    None
}

#[cfg(test)]
mod tests;
