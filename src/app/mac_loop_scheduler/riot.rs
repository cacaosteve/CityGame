//! Mac CODE 10 Rioters starts and their shared XTXT marker continuation.

use super::scenario_action::{begin_action, neighbor, scatter_attempts};
use super::*;

pub(super) const RIOTERS_ACTION: u16 = 3;
pub(super) const REPEATED_RIOTERS_ACTION: u16 = 13;

pub(super) fn start(
    city: &mut City,
    selector: u16,
    point: &mut (i16, i16),
    rng: &mut MacToolboxRng,
) -> usize {
    let mut count = 0;
    if selector == RIOTERS_ACTION {
        for _ in 0..3 {
            if let Some(next) = stamp(city, *point, rng) {
                *point = next;
                count += 1;
            } else if count == 0 {
                break;
            }
        }
    } else {
        // A scatter point outside the map can still find a tile in its spiral.
        let (origin_x, origin_y) = *point;
        for _ in 0..scatter_attempts(city) {
            let x = origin_x.wrapping_add((rng.random_u16() & 31) as i16 - 16);
            let y = origin_y.wrapping_add((rng.random_u16() & 31) as i16 - 16);
            *point = (x, y);
            if let Some(next) = stamp(city, *point, rng) {
                *point = next;
                count += 1;
            }
        }
    }
    if count != 0 {
        begin_action(city, selector);
    }
    count
}

fn stamp(city: &mut City, (x, y): (i16, i16), rng: &mut MacToolboxRng) -> Option<(i16, i16)> {
    let (tx, ty) =
        disaster_spiral_candidates((isize::from(x), isize::from(y)), 127).find(|&(tx, ty)| {
            riot_route_tile(city.building_at(tx, ty))
                && !wet(city, tx, ty)
                && city.text_overlay_at(tx, ty).unwrap_or(0) == 0
        })?;
    city.set_text_overlay(tx, ty, 0xfd + (rng.random_u16() & 1) as u8);
    Some((tx as i16, ty as i16))
}

fn riot_route_tile(building: u8) -> bool {
    matches!(building, 0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c | 0x5d..=0x60)
}

fn wet(city: &City, x: usize, y: usize) -> bool {
    city.tile_bits_at(x, y).is_some_and(TileBits::water_tile)
}

pub(super) fn visit(
    city: &mut City,
    (x, y): (usize, usize),
    marker: u8,
    rng: &mut MacToolboxRng,
    lfsr: &mut MacAppLfsrRng,
    now: &mut u32,
) {
    if rng.random_u16() & 3 != 0 {
        return;
    }
    if rng.random_u16() as u8 == 0 || wet(city, x, y) {
        city.set_text_overlay(x, y, 0);
        return;
    }
    city.set_traffic_density(x, y, 0);
    let ignition = rng.random_u16() & 127;
    if ignition < 4
        && let Some((nx, ny)) = neighbor(x, y, ignition)
    {
        mac_export247_candidate(city, nx, ny, true, lfsr, rng, now);
    }

    let opposite = marker ^ 3;
    let [dx, dy] = if marker == 0xfe { [-1, -1] } else { [1, 1] };
    let x_route = offset_city_tile(x, y, dx, 0).filter(|&(nx, ny)| {
        let building = city.building_at(nx, ny);
        riot_route_tile(building) || matches!(building, 1..=4)
    });
    let y_route = offset_city_tile(x, y, 0, dy).filter(|&(nx, ny)| {
        let building = city.building_at(nx, ny);
        riot_route_tile(building) || matches!(building, 1..=4)
    });
    if x_route.is_none() && y_route.is_none() {
        city.set_text_overlay(x, y, opposite);
        return;
    }
    let remaining = if rng.random_u16() & 7 == 0 {
        opposite
    } else {
        0
    };
    city.set_text_overlay(x, y, remaining);
    let destination = match (x_route, y_route) {
        (Some(a), Some(b)) => Some(if rng.random_u16() & 1 == 0 { a } else { b }),
        (a, b) => a.or(b),
    };
    if let Some((nx, ny)) = destination
        && city.text_overlay_at(nx, ny).unwrap_or(0) < 0x33
    {
        city.set_text_overlay(nx, ny, marker);
    }
}
