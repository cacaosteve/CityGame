//! Shared Win95 ambient actor tile admission and route selection.
//!
//! Actor state transitions and slot dispatch remain in the parent adapter.

use super::{
    City, Win95OverlayRecord, Win95OverlayRng, cargo_ship_can_occupy,
    pick_pe_train_fallback_legacy, subway_can_occupy, tile_in_bounds, tile_is_rail_network,
    train_can_occupy, train_family_can_occupy,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::win95_overlay::{
    XTHG_AIRPLANE, XTHG_CARGO_SHIP, XTHG_HELICOPTER, XTHG_SAILBOAT, XTHG_SUBWAY_TRAIN_CAR,
    XTHG_SUBWAY_TRAIN_ENGINE, XTHG_TRAIN_CAR, XTHG_TRAIN_ENGINE, xthg_direction_delta,
};

const AMBIENT_SPAWN_LOOKAHEAD: i16 = 8;

pub(super) fn pick_heading_toward(
    city: &City,
    x: usize,
    y: usize,
    fallback: u8,
    max_steps: i16,
    can_transit: fn(&City, usize, usize) -> bool,
    is_goal: fn(&City, usize, usize) -> bool,
) -> u8 {
    let mut best: Option<(u16, u8)> = None;
    for dir in 0..8u8 {
        let (dx, dy) = xthg_direction_delta(dir);
        let mut cx = x as i16;
        let mut cy = y as i16;
        for step in 1..=max_steps {
            cx += dx;
            cy += dy;
            if cx < 0 || cy < 0 || cx >= CITY_WIDTH as i16 || cy >= CITY_HEIGHT as i16 {
                break;
            }
            let ux = cx as usize;
            let uy = cy as usize;
            if is_goal(city, ux, uy) {
                let dist = step as u16;
                let take = match best {
                    None => true,
                    Some((best_dist, best_dir)) => {
                        dist < best_dist
                            || (dist == best_dist && prefer_spawn_dir(dir, best_dir, fallback))
                    }
                };
                if take {
                    best = Some((dist, dir));
                }
                break;
            }
            if !can_transit(city, ux, uy) {
                break;
            }
        }
    }
    best.map(|(_, dir)| dir).unwrap_or(fallback)
}

/// Win95 `SpawnShip` / `SpawnSailBoat` / `SpawnTrain` leave direction to the spawn helper.
/// Prefer the nearest goal tile (open water for boats, rail network for trains) along a
/// legal corridor; otherwise keep `fallback` (historical east / north defaults).
pub fn ambient_spawn_direction(kind: u8, city: &City, x: usize, y: usize, fallback: u8) -> u8 {
    let fallback = fallback % 8;
    match kind {
        XTHG_CARGO_SHIP | XTHG_SAILBOAT => pick_heading_toward(
            city,
            x,
            y,
            fallback,
            AMBIENT_SPAWN_LOOKAHEAD,
            watercraft_can_occupy,
            tile_is_water_surface,
        ),
        XTHG_TRAIN_ENGINE | XTHG_SUBWAY_TRAIN_ENGINE => pick_heading_toward(
            city,
            x,
            y,
            fallback,
            AMBIENT_SPAWN_LOOKAHEAD,
            train_family_can_occupy,
            tile_is_rail_network,
        ),
        _ => fallback,
    }
}

fn prefer_spawn_dir(candidate: u8, incumbent: u8, fallback: u8) -> bool {
    if candidate == fallback {
        return true;
    }
    if incumbent == fallback {
        return false;
    }
    let cand_cardinal = candidate % 2 == 0;
    let inc_cardinal = incumbent % 2 == 0;
    if cand_cardinal != inc_cardinal {
        return cand_cardinal;
    }
    candidate < incumbent
}
pub(super) fn ambient_actor_can_occupy(kind: u8, city: &City, x: usize, y: usize) -> bool {
    if !tile_in_bounds(x, y) {
        return false;
    }
    match kind {
        XTHG_AIRPLANE | XTHG_HELICOPTER => true,
        XTHG_CARGO_SHIP => cargo_ship_can_occupy(city, x, y),
        XTHG_SAILBOAT => watercraft_can_occupy(city, x, y),
        XTHG_TRAIN_ENGINE | XTHG_TRAIN_CAR => train_can_occupy(city, x, y),
        XTHG_SUBWAY_TRAIN_ENGINE | XTHG_SUBWAY_TRAIN_CAR => subway_can_occupy(city, x, y),
        _ => false,
    }
}

pub(super) fn tile_is_water_surface(city: &City, x: usize, y: usize) -> bool {
    city.tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
        || matches!(city.terrain_at(x, y), 0x10..=0x1d | 0x30..=0x3e | 0x40..=0x45)
}

pub(super) fn watercraft_can_occupy(city: &City, x: usize, y: usize) -> bool {
    if tile_is_water_surface(city, x, y) {
        return true;
    }
    // Shore infrastructure where ships/sailboats spawn and leave port.
    matches!(city.building_at(x, y), 223 | 224 | 248)
}
pub(super) fn ambient_legal_exits(
    kind: u8,
    city: &City,
    x: usize,
    y: usize,
    current: u8,
    exclude_reverse: bool,
    cardinal_only: bool,
) -> Vec<u8> {
    let reverse = current.wrapping_add(4) % 8;
    let mut options = Vec::with_capacity(8);
    for dir in 0..8u8 {
        if cardinal_only && dir % 2 != 0 {
            continue;
        }
        if exclude_reverse && dir == reverse {
            continue;
        }
        let (dx, dy) = xthg_direction_delta(dir);
        let nx = x as i16 + dx;
        let ny = y as i16 + dy;
        if nx < 0 || ny < 0 || nx >= CITY_WIDTH as i16 || ny >= CITY_HEIGHT as i16 {
            continue;
        }
        if ambient_actor_can_occupy(kind, city, nx as usize, ny as usize) {
            options.push(dir);
        }
    }
    options
}

/// Prefer a 90° / reverse turn onto a legal neighbor; `None` means despawn.
pub(super) fn pick_ambient_turn<R: Win95OverlayRng>(
    record: &Win95OverlayRecord,
    city: &City,
    rng: &mut R,
) -> Option<u8> {
    if record.is_ambient_train_family() {
        return pick_pe_train_fallback_legacy(record, city, rng);
    }
    let x = record.tile_x as usize;
    let y = record.tile_y as usize;
    let current = record.lane % 8;
    // Left, right, reverse, then remaining compass headings.
    let candidates = [
        current.wrapping_add(6) % 8,
        current.wrapping_add(2) % 8,
        current.wrapping_add(4) % 8,
        current.wrapping_add(7) % 8,
        current.wrapping_add(1) % 8,
        current.wrapping_add(5) % 8,
        current.wrapping_add(3) % 8,
    ];
    for &dir in &candidates {
        let (dx, dy) = xthg_direction_delta(dir);
        let nx = x as i16 + dx;
        let ny = y as i16 + dy;
        if nx < 0 || ny < 0 || nx >= CITY_WIDTH as i16 || ny >= CITY_HEIGHT as i16 {
            continue;
        }
        if ambient_actor_can_occupy(record.kind, city, nx as usize, ny as usize) {
            return Some(dir);
        }
    }
    None
}
