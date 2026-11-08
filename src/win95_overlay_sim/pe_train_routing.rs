//! Win95 PE train cardinal routing, rail occupancy, and drawing-code helpers.
//!
//! The parent overlay adapter retains actor scheduling and movement progression.

use super::{City, Win95GameRand, Win95OverlayRecord, Win95OverlayRng};
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::win95_overlay::{
    XTHG_EXPLOSION_XTXT_GATE, XTHG_SUBWAY_TRAIN_CAR, XTHG_SUBWAY_TRAIN_ENGINE, XTHG_TRAIN_CAR,
    XTHG_TRAIN_ENGINE, xthg_direction_delta,
};

// PE byte[0x004E8300 + next_cardinal + 4 * previous_cardinal]. Both indices
// are the train's native N/E/S/W direction byte (0..=3), not XTHG's 8-way lane.
pub(super) const PE_TRAIN_DRAW_CODES: [[u8; 4]; 4] =
    [[0, 1, 2, 7], [1, 2, 3, 4], [2, 3, 4, 5], [7, 4, 5, 6]];

pub(super) fn pe_train_draw_code(previous_lane: u8, next_lane: u8) -> u8 {
    PE_TRAIN_DRAW_CODES[(previous_lane & 3) as usize][(next_lane & 3) as usize]
}

/// PE train XTHG stores cardinal indices 0=N, 1=E, 2=S, 3=W. Convert only
/// when a helper expects the general eight-way XTHG compass encoding.
pub(super) fn pe_train_compass_direction(cardinal: u8) -> u8 {
    (cardinal & 3) * 2
}

pub(super) fn train_can_occupy(city: &City, x: usize, y: usize) -> bool {
    pe_train_destination_xtxt_clear(city, x, y) && train_track_at(city, x, y)
}

pub(super) fn train_track_at(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    // `FUN_00455C60` train arm: surface rail, tunnel mouths, and crossings.
    // The rail depot is a spawn source, not a valid train-route destination.
    matches!(building, 44..=63 | 69..=73 | 77 | 78 | 90 | 91 | 108..=112)
}

pub(super) fn subway_can_occupy(city: &City, x: usize, y: usize) -> bool {
    pe_train_destination_xtxt_clear(city, x, y) && subway_track_at(city, x, y)
}

pub(super) fn subway_track_at(city: &City, x: usize, y: usize) -> bool {
    // `FUN_00455C60` subway arm: XBIT subway values, plus either portal face.
    if matches!(city.building_at(x, y), 108..=112) {
        return true;
    }
    let underground = city.underground_at(x, y);
    (underground > 0 && underground < 16) || matches!(underground, 31 | 32 | 34 | 35)
}

/// `FUN_00455C60` rejects both train arms when destination XTXT (`0x4CA600`) is `>= 0xC9`.
pub(super) fn pe_train_destination_xtxt_clear(city: &City, x: usize, y: usize) -> bool {
    city.text_overlay_at(x, y).unwrap_or(0) < XTHG_EXPLOSION_XTXT_GATE
}

/// Spawn heading can seek either rail system before the engine reaches a portal.
pub(super) fn train_family_can_occupy(city: &City, x: usize, y: usize) -> bool {
    train_can_occupy(city, x, y) || subway_can_occupy(city, x, y)
}

pub(super) fn tile_is_rail_network(city: &City, x: usize, y: usize) -> bool {
    train_can_occupy(city, x, y)
}

pub(super) fn sync_train_kind_to_tile(record: &mut Win95OverlayRecord, city: &City) {
    if !record.is_ambient_train_engine() {
        return;
    }
    if matches!(
        city.building_at(record.tile_x as usize, record.tile_y as usize),
        108..=112
    ) {
        // `FUN_00455500`: flip 0x0A <-> 0x0C before checking the next route.
        record.kind = match record.kind {
            XTHG_TRAIN_ENGINE => XTHG_SUBWAY_TRAIN_ENGINE,
            XTHG_SUBWAY_TRAIN_ENGINE => XTHG_TRAIN_ENGINE,
            _ => unreachable!("train-engine guard accepted an unrelated XTHG kind"),
        };
    }
}
fn pe_train_kind_can_occupy(kind: u8, city: &City, x: usize, y: usize) -> bool {
    match kind {
        XTHG_TRAIN_ENGINE | XTHG_TRAIN_CAR => train_can_occupy(city, x, y),
        XTHG_SUBWAY_TRAIN_ENGINE | XTHG_SUBWAY_TRAIN_CAR => subway_can_occupy(city, x, y),
        _ => false,
    }
}

/// `FUN_00402671`: preserve the PE's train fallback scan rather than applying
/// the generic actor left/right/reverse heuristic.
pub(super) fn pick_pe_train_fallback(
    record: &Win95OverlayRecord,
    city: &City,
    game_rand: &mut Win95GameRand,
) -> Option<u8> {
    // `FUN_00456100` uses Game_Random(2) to choose one of its two stored
    // candidate orders before probing four cardinal neighbors.
    let mode = game_rand.random_mod(2);
    let candidates = pe_train_fallback_candidates(record.lane, mode);
    for cardinal in candidates {
        let (dx, dy) = xthg_direction_delta(pe_train_compass_direction(cardinal));
        let x = record.tile_x as i16 + dx;
        let y = record.tile_y as i16 + dy;
        if x >= 0
            && y >= 0
            && x < CITY_WIDTH as i16
            && y < CITY_HEIGHT as i16
            && pe_train_kind_can_occupy(record.kind, city, x as usize, y as usize)
        {
            return Some(cardinal);
        }
    }
    None
}

/// Compatibility path for the generic, non-dispatch helper. The live PE XTHG
/// walker uses `pick_pe_train_fallback` with its shared process game-LCG.
pub(super) fn pick_pe_train_fallback_legacy<R: Win95OverlayRng>(
    record: &Win95OverlayRecord,
    city: &City,
    rng: &mut R,
) -> Option<u8> {
    let random_zero = rng.roll_mod_is_zero(2);
    let mode = u16::from(!random_zero);
    for cardinal in pe_train_fallback_candidates(record.lane, mode) {
        let (dx, dy) = xthg_direction_delta(pe_train_compass_direction(cardinal));
        let x = record.tile_x as i16 + dx;
        let y = record.tile_y as i16 + dy;
        if x >= 0
            && y >= 0
            && x < CITY_WIDTH as i16
            && y < CITY_HEIGHT as i16
            && pe_train_kind_can_occupy(record.kind, city, x as usize, y as usize)
        {
            return Some(cardinal);
        }
    }
    None
}

pub(super) fn pe_train_fallback_candidates(current: u8, mode: u16) -> [u8; 4] {
    const ZERO_ORDER: [u8; 4] = [0, 3, 1, 2];
    const NONZERO_ORDER: [u8; 4] = [0, 1, 3, 2];
    let offsets = if mode == 0 { ZERO_ORDER } else { NONZERO_ORDER };
    offsets.map(|offset| (current.wrapping_add(offset)) & 3)
}

/// `SpawnTrain` seeds `FUN_00402671` with `rand() % 4`; the helper then scans
/// its PE cardinal fallback order from the rail tile adjacent to the station.
pub fn ambient_train_spawn_direction(
    city: &City,
    x: usize,
    y: usize,
    initial_cardinal: u8,
    game_rand: &mut Win95GameRand,
) -> Option<u8> {
    let mode = game_rand.random_mod(2);
    for cardinal in pe_train_fallback_candidates(initial_cardinal, mode) {
        let (dx, dy) = xthg_direction_delta(pe_train_compass_direction(cardinal));
        let x = x as i16 + dx;
        let y = y as i16 + dy;
        if x >= 0
            && y >= 0
            && x < CITY_WIDTH as i16
            && y < CITY_HEIGHT as i16
            && train_can_occupy(city, x as usize, y as usize)
        {
            // Keep the public helper's compass-heading result. SpawnTrain stores
            // its direction byte as the corresponding native cardinal index.
            return Some(pe_train_compass_direction(cardinal));
        }
    }
    None
}

/// `FUN_00455500`: occasionally attempt a perpendicular route, then continue
/// straight or invoke the PE fallback scan.
pub(super) fn pick_pe_train_route<R: Win95OverlayRng>(
    record: &Win95OverlayRecord,
    city: &City,
    rng: &mut R,
    game_rand: &mut Win95GameRand,
) -> Option<u8> {
    let mut direction = record.lane & 3;
    if rng.roll_mod_is_zero(4) {
        // PE uses `rand() % 2 != 0` for +1 cardinal (clockwise) and zero for -1.
        let turn = if rng.roll_mod_is_zero(2) {
            direction.wrapping_add(3) & 3
        } else {
            direction.wrapping_add(1) & 3
        };
        let (dx, dy) = xthg_direction_delta(pe_train_compass_direction(turn));
        let x = record.tile_x as i16 + dx;
        let y = record.tile_y as i16 + dy;
        if x >= 0
            && y >= 0
            && x < CITY_WIDTH as i16
            && y < CITY_HEIGHT as i16
            && pe_train_kind_can_occupy(record.kind, city, x as usize, y as usize)
        {
            direction = turn;
        }
    }
    let (dx, dy) = xthg_direction_delta(pe_train_compass_direction(direction));
    let x = record.tile_x as i16 + dx;
    let y = record.tile_y as i16 + dy;
    if x >= 0
        && y >= 0
        && x < CITY_WIDTH as i16
        && y < CITY_HEIGHT as i16
        && pe_train_kind_can_occupy(record.kind, city, x as usize, y as usize)
    {
        return Some(direction);
    }
    let mut fallback = *record;
    fallback.lane = direction;
    pick_pe_train_fallback(&fallback, city, game_rand)
}
