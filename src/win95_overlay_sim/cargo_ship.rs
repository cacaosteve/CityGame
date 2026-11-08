//! Win95 PE cargo-ship XTHG state, route, and movement workers.
//!
//! Shared watercraft admission and ambient actor dispatch remain in the parent adapter.

use super::{
    City, Win95CrtRand, Win95Lfsr, Win95OverlayRng, Win95OverlaySim, pe_clear_xthg_kind,
    pe_step_direction_toward, pe_zero_tile_xtxt, watercraft_can_occupy,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::win95_overlay::{
    Win95OverlayRecord, XTHG_CARGO_SHIP, XTHG_EXPLOSION, XTHG_EXPLOSION_XTXT_GATE,
    XTHG_FIRST_USABLE_SLOT,
};

pub(super) fn first_cargo_ship_position(table: &[Win95OverlayRecord]) -> Option<(usize, usize)> {
    table
        .iter()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find(|record| record.kind == XTHG_CARGO_SHIP)
        .map(|record| (record.tile_x as usize, record.tile_y as usize))
}

impl Win95OverlaySim {
    /// Step a kind-3 cargo ship at its position in the app's selected XTHG walk.
    /// The native handler's CRT sound gate precedes its LFSR state/movement work.
    pub fn tick_pe_cargo_ship_slot(
        &mut self,
        city: &mut City,
        slot: usize,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) -> bool {
        let table_snapshot = city
            .traffic_overlays()
            .map(|table| table.to_vec())
            .unwrap_or_default();
        let Some(mut record) = table_snapshot.get(slot).copied() else {
            return false;
        };
        if record.kind != XTHG_CARGO_SHIP {
            return false;
        }
        let old_first_cargo_ship = first_cargo_ship_position(&table_snapshot);
        let old_x = record.tile_x as usize;
        let old_y = record.tile_y as usize;
        let (ok, play_sfx) = step_pe_cargo_actor_with_streams(&mut record, city, crt, lfsr, slot);
        if play_sfx {
            self.cargo_sfx_plays = self.cargo_sfx_plays.saturating_add(1);
        }
        if !ok {
            return false;
        }
        if let Some(overlays) = city.traffic_overlays_mut() {
            overlays[slot] = record;
        }
        city.mark_render_dirty(old_x, old_y);
        if record.kind != 0 {
            city.mark_render_dirty(record.tile_x as usize, record.tile_y as usize);
        }
        let new_first_cargo_ship = city
            .traffic_overlays()
            .map(first_cargo_ship_position)
            .flatten();
        if old_first_cargo_ship != new_first_cargo_ship {
            for position in [old_first_cargo_ship, new_first_cargo_ship]
                .into_iter()
                .flatten()
            {
                city.mark_win95_pe_raising_bridge_render_dirty_near(position.0, position.1);
            }
        }
        city.mark_traffic_layers_dirty();
        true
    }
}

fn update_pe_cargo_state<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &City,
    rng: &mut R,
) -> bool {
    match record.state {
        0 => update_pe_cargo_state_zero(record, city, rng),
        1 => {
            update_pe_cargo_state_one(record, city);
            false
        }
        2 => {
            update_pe_cargo_state_two(record, city, rng);
            false
        }
        3 => {
            update_pe_cargo_state_three(record, city, rng);
            false
        }
        4 => update_pe_cargo_state_four(record, city, rng),
        _ => false,
    }
}

/// PE `FUN_00456C70` -> `FUN_00456D40` cargo-ship route predicate.
///
/// `City::new_blank` is a synthetic fixture with no decoded chunks; retain its
/// port/terrain behavior while loaded Win95 cities use the disassembled XBIT,
/// XTER, and XBLD gates below. Occupancy XTXT `>= 0xC9` is enforced by move
/// `FUN_00456E10` / spawn, not this route predicate — and PE does not consult XTRF.
pub(super) fn cargo_ship_can_occupy(city: &City, x: usize, y: usize) -> bool {
    if city.chunks.is_empty() {
        return watercraft_can_occupy(city, x, y);
    }
    pe_cargo_ship_can_occupy(city, x, y)
}

fn pe_cargo_ship_can_occupy(city: &City, x: usize, y: usize) -> bool {
    // PE `FUN_00456D40`: XBIT water, then XTER `0x10..=0x1f`, then XBLD.
    if !city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
    {
        return false;
    }
    if !matches!(city.terrain_at(x, y), 0x10..=0x1f) {
        return false;
    }
    let building = city.building_at(x, y);
    if building == 0xf8 {
        return false;
    }
    if building == 0 {
        return true;
    }
    // Nine PE route tiles at `0x004E83D0`.
    matches!(
        building,
        0x51 | 0x52 | 0x54 | 0x55 | 0x58 | 0x59 | 0x5b | 0x5c | 0x6b
    )
}

/// PE `FUN_00456C70` state 1: turn one compass step toward XTHG `+8/+9`,
/// then enter state 0 only after the aligned next tile passes `FUN_0040123A`.
pub(super) fn update_pe_cargo_state_one(record: &mut Win95OverlayRecord, city: &City) {
    if city.chunks.is_empty() || record.state != 1 {
        return;
    }

    let target_direction = pe_cargo_direction_to_target(
        record.tile_x,
        record.tile_y,
        record.target_x,
        record.target_y,
    );
    record.lane = pe_step_direction_toward(record.lane, target_direction);
    if record.lane != target_direction {
        return;
    }

    record.state = if pe_cargo_candidate_in_direction(city, record, record.lane) {
        0
    } else {
        2
    };
}

/// PE `FUN_00456C70` state 0 at `0x45682e`: `0x401cad(0xa)` occasionally adjust toward the
/// target, fall back to state 1 when the current candidate is blocked, then
/// detect a two-tile cardinal XBLD `0xdf` port for state 3.
pub(super) fn update_pe_cargo_state_zero<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &City,
    rng: &mut R,
) -> bool {
    if city.chunks.is_empty() || record.state != 0 {
        return false;
    }

    if rng.roll_mod_is_zero(10) {
        let target_direction = pe_cargo_direction_to_target(
            record.tile_x,
            record.tile_y,
            record.target_x,
            record.target_y,
        );
        let adjusted = pe_step_direction_toward(record.lane, target_direction);
        if pe_cargo_candidate_in_direction(city, record, adjusted) {
            record.lane = adjusted;
        }
    }

    // PE @ `0x45689a`: only the success arm calls FUN_00456E10.
    let may_move = pe_cargo_candidate_in_direction(city, record, record.lane);
    if !may_move {
        record.state = 1;
    }

    for (dx, dy) in [(2_i16, 0_i16), (0, 2), (-2, 0), (0, -2)] {
        let x = i16::from(record.tile_x) + dx;
        let y = i16::from(record.tile_y) + dy;
        if x >= 0
            && y >= 0
            && x < CITY_WIDTH as i16
            && y < CITY_HEIGHT as i16
            && city.building_at(x as usize, y as usize) == 0xdf
        {
            record.state = 3;
            break;
        }
    }
    may_move
}

/// PE `FUN_00456C70` -> `FUN_00456C70` candidate offsets at `0x4e83b0/0x4e83c0`.
pub(super) fn pe_cargo_candidate_in_direction(
    city: &City,
    record: &Win95OverlayRecord,
    direction: u8,
) -> bool {
    const DX: [i16; 8] = [0, 3, 4, 3, 0, -3, -4, -3];
    const DY: [i16; 8] = [-4, -3, 0, 3, 4, 3, 0, -3];
    let index = usize::from(direction & 0x07);
    let x = i16::from(record.tile_x) + DX[index];
    let y = i16::from(record.tile_y) + DY[index];
    // PE @ `0x456d04`: out-of-bounds dest returns success (`ax=1`), same as sailboat.
    if x < 0 || y < 0 || x >= 0x80 || y >= 0x80 {
        return true;
    }
    pe_cargo_ship_can_occupy(city, x as usize, y as usize)
        // PE `FUN_00456C70` @ `0x456cf4`: refuse dest XTXT `>= 0xc9`.
        && city
            .text_overlay_at(x as usize, y as usize)
            .unwrap_or(0)
            < XTHG_EXPLOSION_XTXT_GATE
}

/// PE `FUN_00456C70` state 2 at `0x4569a4`: search nine directions around the
/// current lane, using either the forward or reverse table selected by
/// `0x401cad(2)`, then return to state 0.
pub(super) fn update_pe_cargo_state_two<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &City,
    rng: &mut R,
) {
    if city.chunks.is_empty() || record.state != 2 {
        return;
    }

    const FORWARD: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 0];
    const REVERSE: [u8; 9] = [0, 7, 6, 5, 4, 3, 2, 1, 2];
    let order = if rng.roll_mod_is_zero(2) {
        &REVERSE
    } else {
        &FORWARD
    };
    let mut selected = None;
    for (index, offset) in order.iter().copied().enumerate() {
        let direction = (record.lane + offset) & 0x07;
        if pe_cargo_candidate_in_direction(city, record, direction) {
            selected = Some((index, direction));
            break;
        }
    }

    match selected {
        Some((8, _)) => {
            // PE `FUN_00401528` clears the XTHG kind on this branch.
            record.kind = 0;
            record.state = 0;
        }
        Some((_, direction)) => {
            record.lane = direction;
            record.state = 0;
        }
        None => {
            // The PE loop still writes the ninth attempted direction when no
            // candidate succeeds; index 8 is the last table entry.
            record.lane = (record.lane + order[8]) & 0x07;
            record.state = 0;
        }
    }
}

/// PE state 3 at `0x456a48`: on `0x401cad(0x1e) == 0`, enter state 4 and copy
/// SpawnShip globals `0x4e80cc`/`0x4e80d0` into XTHG `+8/+9`.
pub(super) fn update_pe_cargo_state_three<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &City,
    rng: &mut R,
) {
    if city.chunks.is_empty() || record.state != 3 {
        return;
    }
    if rng.roll_mod_is_zero(30) {
        record.state = 4;
        let (tx, ty) = city.pe_cargo_spawn_tile;
        record.target_x = tx;
        record.target_y = ty;
    }
}

/// PE state 4 at `0x456a9b`: preserve the lane when its current candidate is
/// valid; otherwise search the forward table from a `rand()%2` start index.
pub(super) fn update_pe_cargo_state_four<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &City,
    rng: &mut R,
) -> bool {
    if city.chunks.is_empty() || record.state != 4 {
        return false;
    }
    if pe_cargo_candidate_in_direction(city, record, record.lane) {
        return true;
    }

    const FORWARD: [u8; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 0];
    let start = if rng.roll_mod_is_zero(2) { 0 } else { 1 };
    for offset in FORWARD.iter().copied().skip(start) {
        let direction = (record.lane + offset) & 0x07;
        if pe_cargo_candidate_in_direction(city, record, direction) {
            record.lane = direction;
            break;
        }
    }
    false
}

/// PE `FUN_00456E10`: advance cargo subtiles with the 12-unit movement ring
/// and the same 3/4-tile compass vectors used by `FUN_00456C70`.
pub(super) fn step_pe_cargo_movement(record: &mut Win95OverlayRecord, _city: &City) -> bool {
    let (sub_x, sub_y, next_x, next_y, _) = pe_cargo_movement_result(record);
    record.sub_x = sub_x;
    record.sub_y = sub_y;
    if next_x < 0 || next_y < 0 || next_x >= CITY_WIDTH as i16 || next_y >= CITY_HEIGHT as i16 {
        pe_clear_xthg_kind(record);
        return true;
    }
    record.tile_x = next_x as u8;
    record.tile_y = next_y as u8;
    true
}

pub(super) fn step_pe_cargo_actor_mut<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    lfsr: &mut R,
    slot: usize,
) -> bool {
    // PE FUN_00456760 @ `0x4567d5`/`0x45680e`: OOB or missing XBIT water `0x04`
    // → kind-6 / state 0 / lane 0 / +B 0 (no state JT, no FUN_00456E10).
    let tx = record.tile_x as usize;
    let ty = record.tile_y as usize;
    let water_ok = tx < CITY_WIDTH
        && ty < CITY_HEIGHT
        && city
            .tile_bits_at(tx, ty)
            .is_some_and(|bits| bits.water_tile());
    if !water_ok {
        record.kind = XTHG_EXPLOSION;
        record.state = 0;
        record.lane = 0;
        record.saved_b = 0;
        return true;
    }
    let may_move = update_pe_cargo_state(record, &*city, lfsr);
    if record.kind == 0 {
        // PE state-2 clear @ `0x456a25` calls `FUN_00401528` → `FUN_004507D0`,
        // which zeros XTXT at the actor tile (does not restore `+A`).
        pe_clear_cargo_xtxt_occupancy(city, record);
        return true;
    }
    // PE: only state 0/4 success arms call FUN_00456E10.
    if !may_move {
        return true;
    }
    step_pe_cargo_movement_mut(record, city, slot)
}

/// PE cargo Things tick `FUN_00456760`: CRT SFX gate then LFSR state machine.
/// Returns `(moved_or_updated, play_sfx_0x205)`.
pub(super) fn step_pe_cargo_actor_with_streams(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    slot: usize,
) -> (bool, bool) {
    // PE @ `0x45676e`: always `rand()`; `test al,al` → optional `0x401096(0x205)`.
    let sfx = crt.random_u16();
    let play_sfx = sfx & 0xff == 0;
    (step_pe_cargo_actor_mut(record, city, lfsr, slot), play_sfx)
}

/// Mutable `FUN_00456E10` path: restore prior XTXT on the old tile, then paint
/// `slot-0x37` occupancy on the destination (`0x4CA600`, not XTRF).
/// Unlike `FUN_004516F0` (plane/heli), cargo move has no dest `>= 0xC9` refuse.
pub(super) fn step_pe_cargo_movement_mut(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    slot: usize,
) -> bool {
    let old_x = record.tile_x as usize;
    let old_y = record.tile_y as usize;
    let saved_a = record.saved_a;
    let (sub_x, sub_y, next_x, next_y, crossed_tile) = pe_cargo_movement_result(record);
    if next_x < 0 || next_y < 0 || next_x >= CITY_WIDTH as i16 || next_y >= CITY_HEIGHT as i16 {
        // PE `FUN_00456E10` OOB: restore `+A` at the actor tile, keep kind/tile,
        // commit subtules, and return 0 (callers ignore AX — no FUN_00401528).
        if old_x < CITY_WIDTH && old_y < CITY_HEIGHT {
            city.set_text_overlay(old_x, old_y, saved_a);
        }
        record.sub_x = sub_x;
        record.sub_y = sub_y;
        return true;
    }
    let nx = next_x as usize;
    let ny = next_y as usize;
    record.sub_x = sub_x;
    record.sub_y = sub_y;
    if crossed_tile {
        record.saved_a = city.move_ambient_cargo_xthg_proxy(slot, old_x, old_y, saved_a, nx, ny);
    }
    record.tile_x = next_x as u8;
    record.tile_y = next_y as u8;
    true
}

/// PE `FUN_00401528` / `FUN_004507D0`: zero `0x4CA600` at the cargo tile.
fn pe_clear_cargo_xtxt_occupancy(city: &mut City, record: &Win95OverlayRecord) {
    pe_zero_tile_xtxt(city, record.tile_x as usize, record.tile_y as usize);
}

fn pe_cargo_movement_result(record: &Win95OverlayRecord) -> (u8, u8, i16, i16, bool) {
    const DX: [i16; 8] = [0, 3, 4, 3, 0, -3, -4, -3];
    const DY: [i16; 8] = [-4, -3, 0, 3, 4, 3, 0, -3];
    let direction = usize::from(record.lane & 0x07);
    let (sub_x, tile_dx) = pe_cargo_advance_subtile(i16::from(record.sub_x) + DX[direction]);
    let (sub_y, tile_dy) = pe_cargo_advance_subtile(i16::from(record.sub_y) + DY[direction]);
    let next_x = i16::from(record.tile_x) + tile_dx;
    let next_y = i16::from(record.tile_y) + tile_dy;
    (sub_x, sub_y, next_x, next_y, tile_dx != 0 || tile_dy != 0)
}

fn pe_cargo_advance_subtile(value: i16) -> (u8, i16) {
    if value > 12 {
        ((value - 12) as u8, 1)
    } else if value < 0 {
        ((value + 12) as u8, -1)
    } else {
        (value as u8, 0)
    }
}

/// PE `FUN_00451?E0` direction-to-target helper used by cargo state 1.
pub(super) fn pe_cargo_direction_to_target(
    current_x: u8,
    current_y: u8,
    target_x: u8,
    target_y: u8,
) -> u8 {
    let dx = i16::from(target_x) - i16::from(current_x);
    let dy = i16::from(target_y) - i16::from(current_y);
    let abs_dx = dx.abs();
    let abs_dy = dy.abs();

    if (abs_dy + 1) / 2 > abs_dx {
        return if dy < 0 { 0 } else { 4 };
    }
    if dx < 0 {
        if (abs_dx + 1) / 2 <= abs_dy {
            return if dy < 0 { 7 } else { 5 };
        }
        return 6;
    }
    if (abs_dx + 1) / 2 <= abs_dy {
        return if dy < 0 { 1 } else { 3 };
    }
    2
}
