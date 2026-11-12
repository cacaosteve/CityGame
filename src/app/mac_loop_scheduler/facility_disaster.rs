//! Mac CODE 10 disaster starts keyed to the saved building census.

use super::scenario_action::{begin_action, burn_footprint, checked_tile};
use super::*;

pub(super) const SCATTER_ACTION: u16 = 9;
pub(super) const WALK_ACTION: u16 = 10;
const SCATTER_SOURCE: u8 = 0xcb;
const WALK_SOURCE: u8 = 0xcd;
const WALK_STEPS: [(i16, i16); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

fn first_source(city: &City, building: u8) -> Option<(usize, usize)> {
    (0..CITY_WIDTH)
        .flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)))
        .find(|&(x, y)| city.building_at(x, y) == building)
}

/// CODE 10 FUN_3c71c uses the retained Point before scanning the map.
pub(super) fn start_scatter(
    city: &mut City,
    point: (i16, i16),
    rng: &mut MacToolboxRng,
    now: &mut u32,
) -> bool {
    let target = checked_tile(point.0, point.1)
        .filter(|&(x, y)| city.building_at(x, y) == SCATTER_SOURCE)
        .or_else(|| first_source(city, SCATTER_SOURCE));
    let Some((x, y)) = target else {
        return false;
    };
    let (x, y) = super::super::budget::mac_export81_footprint(city, x, y, SCATTER_SOURCE)
        .map_or((x as i16, y as i16), |(x, y, _)| (x, y));
    let (x, y) = (x + 1, y - 1);
    if let Some((cx, cy)) = checked_tile(x, y) {
        let waits = burn_footprint(city, cx, cy, rng, true);
        *now = now.wrapping_add(waits.wrapping_mul(MAC_EXPORT80_RENDER_WAIT_TICKS_PER_LOOP));
    }

    let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    for dx in -32..=32 {
        for dy in -32..=32 {
            if rng.random_u16() & 31 != 0 {
                continue;
            }
            let Some((tx, ty)) = checked_tile(x + dx, y + dy) else {
                continue;
            };
            if rng.random_u16() & 3 == 0 {
                mac_export247_candidate(city, tx, ty, false, &mut lfsr, rng, now);
                continue;
            }
            // FUN_3dc90 with zero flags still calls export 80 unless the
            // target is in its protected portal band.
            if !matches!(city.building_at(tx, ty), 0x3f..=0x42) {
                super::super::budget::mac_destroy_building_export80(city, tx, ty, rng, false);
            }
            if rng.random_u16() & 1 != 0 {
                if city.tile_bits_at(tx, ty).is_some_and(TileBits::water_tile) {
                    city.set_text_overlay(tx, ty, 0xfb);
                } else {
                    pe_place_tile_441f00(city, tx, ty, 5);
                }
            }
        }
    }
    for dx in -1..=2 {
        for dy in -2..=1 {
            if rng.random_u16() & 1 != 0
                && let Some((tx, ty)) = checked_tile(x + dx, y + dy)
            {
                pe_place_tile_441f00(city, tx, ty, 5);
            }
        }
    }
    begin_action(city, SCATTER_ACTION);
    true
}

/// CODE 10 FUN_3c944 selects the first matching XBLD in X-major order.
pub(super) fn start_walk(city: &mut City, rng: &mut MacToolboxRng, now: &mut u32) -> Option<usize> {
    let (x, y) = first_source(city, WALK_SOURCE)?;
    // The native scan rechecks one byte beyond each row. Contiguous XBLD
    // rows make a source at (x, 0) appear as (x - 1, 128) first.
    let (mut x, mut y) = if x > 0 && y == 0 {
        (x as i16 - 1, CITY_HEIGHT as i16)
    } else {
        (x as i16, y as i16)
    };
    let mut direction = rng.random_u16() & 7;
    let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    let mut sound_requests = 0;
    for _ in 0..39 {
        let Some((tx, ty)) = checked_tile(x, y) else {
            break;
        };
        if city.building_at(tx, ty) != WALK_SOURCE {
            if city.tile_bits_at(tx, ty).is_some_and(TileBits::water_tile) {
                city.set_text_overlay(tx, ty, 0xfb);
            }
            mac_export247_candidate(city, tx, ty, false, &mut lfsr, rng, now);
            sound_requests += 1;
        }
        let (dx, dy) = WALK_STEPS[usize::from(direction)];
        x += dx;
        y += dy;
        direction = rng.random_u16() & 7;
    }
    begin_action(city, WALK_ACTION);
    Some(sound_requests)
}
