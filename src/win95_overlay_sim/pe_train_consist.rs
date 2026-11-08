//! Win95 PE linked rail-consist slot update and merge workers.
//!
//! The parent adapter retains slot scheduling, ambient train routing, and common XTXT helpers.

use super::{
    City, Win95CrtRand, Win95GameRand, Win95Lfsr, Win95OverlayRecord, Win95OverlayRng,
    Win95OverlaySim, ambient_actor_can_occupy, clear_xthg_slot_pe, pe_clear_xthg_kind,
    pe_train_compass_direction, pe_train_draw_code, pe_zero_tile_xtxt, pick_pe_train_route,
    subway_track_at, sync_train_kind_to_tile, tile_in_bounds, train_track_at, xthg_direction_delta,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::win95_overlay::{
    XTHG_EXPLOSION_XTXT_GATE, XTHG_FIRST_USABLE_SLOT, XTHG_RECORD_COUNT, XTHG_SUBWAY_TRAIN_CAR,
    XTHG_SUBWAY_TRAIN_ENGINE, XTHG_TRAIN_CAR, XTHG_TRAIN_ENGINE,
};

impl Win95OverlaySim {
    /// Step a kind-10/12 train engine and its linked cars at the engine's
    /// native XTHG slot. The PE disables car-kind dispatch and copies both
    /// followers from inside this engine handler.
    pub fn tick_pe_train_engine_slot(
        &mut self,
        city: &mut City,
        slot: usize,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
    ) -> bool {
        if !(XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT).contains(&slot) {
            return false;
        }
        let table_snapshot = city
            .traffic_overlays()
            .map(|table| table.to_vec())
            .unwrap_or_default();
        let Some(record) = table_snapshot.get(slot).copied() else {
            return false;
        };
        if !record.is_ambient_train_engine() {
            return false;
        }
        let changed =
            step_pe_train_engine_slot(city, slot, record, &table_snapshot, crt, lfsr, game_rand);
        if changed {
            city.mark_traffic_layers_dirty();
        }
        changed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrainEngineStep {
    Advanced,
    NeedsTurnaround,
    Blocked,
    Cleared,
}

pub(super) fn step_ambient_train_engine<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &City,
    rng: &mut R,
    game_rand: &mut Win95GameRand,
    tail_slot: Option<usize>,
) -> TrainEngineStep {
    // PE train iPX/iPY are the next rail tile, not 0..15 subtile offsets.
    let next_x = record.sub_x as usize;
    let next_y = record.sub_y as usize;
    if !pe_train_target_xtext_is_open(city, tail_slot, next_x, next_y) {
        return TrainEngineStep::Blocked;
    }
    if !pe_train_target_tile_is_valid(record, city, next_x, next_y) {
        pe_clear_xthg_kind(record);
        return TrainEngineStep::Cleared;
    }

    step_ambient_train_engine_after_preflight(record, city, rng, game_rand, next_x, next_y)
}

pub(super) fn pe_train_target_xtext_is_open(
    city: &City,
    tail_slot: Option<usize>,
    next_x: usize,
    next_y: usize,
) -> bool {
    let target_xtext = city.text_overlay_at(next_x, next_y).unwrap_or(0);
    target_xtext < XTHG_EXPLOSION_XTXT_GATE
        || tail_slot.and_then(crate::win95_overlay::xthg_proxy_for_slot) == Some(target_xtext)
}

fn pe_train_target_tile_is_valid(
    record: &Win95OverlayRecord,
    city: &City,
    next_x: usize,
    next_y: usize,
) -> bool {
    tile_in_bounds(next_x, next_y)
        && match record.kind {
            // XTXT is checked separately, including the linked-tail proxy exception.
            XTHG_TRAIN_ENGINE | XTHG_TRAIN_CAR => train_track_at(city, next_x, next_y),
            XTHG_SUBWAY_TRAIN_ENGINE | XTHG_SUBWAY_TRAIN_CAR => {
                subway_track_at(city, next_x, next_y)
            }
            _ => ambient_actor_can_occupy(record.kind, city, next_x, next_y),
        }
}

pub(super) fn step_ambient_train_engine_after_preflight<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &City,
    rng: &mut R,
    game_rand: &mut Win95GameRand,
    next_x: usize,
    next_y: usize,
) -> TrainEngineStep {
    let previous_lane = record.lane & 3;
    record.tile_x = next_x as u8;
    record.tile_y = next_y as u8;
    sync_train_kind_to_tile(record, city);
    // The PE lane byte is a compact cardinal index, unlike other XTHG actors.
    record.lane = previous_lane;
    let Some(direction) = pick_pe_train_route(record, city, rng, game_rand) else {
        return TrainEngineStep::NeedsTurnaround;
    };
    record.lane = direction;

    let (dx, dy) = xthg_direction_delta(pe_train_compass_direction(record.lane));
    let target_x = next_x as i16 + dx;
    let target_y = next_y as i16 + dy;
    if target_x < 0
        || target_y < 0
        || target_x >= CITY_WIDTH as i16
        || target_y >= CITY_HEIGHT as i16
    {
        pe_clear_xthg_kind(record);
        return TrainEngineStep::Cleared;
    }
    record.sub_x = target_x as u8;
    record.sub_y = target_y as u8;
    record.target_x = pe_train_draw_code(previous_lane, record.lane);
    TrainEngineStep::Advanced
}

/// Cars trail one tick behind the linked leader (`saved_b` slot), matching a consist.
///
/// PE tile-cross path calls `FUN_004029FF` as first→second then engine→first before the
/// engine commits `+6/+7` into `+3/+4`; Rust applies the same field contract from the
/// pre-tick snapshot so cars lag one step.
pub(super) fn step_ambient_train_car(
    record: &mut Win95OverlayRecord,
    snapshot: &[Win95OverlayRecord],
) -> bool {
    if !record.is_ambient_train_car() {
        return false;
    }
    let leader_slot = record.saved_b as usize;
    let Some(leader) = snapshot.get(leader_slot).copied() else {
        pe_clear_xthg_kind(record);
        return true;
    };
    if !leader.is_ambient_train_family() {
        pe_clear_xthg_kind(record);
        return true;
    }
    pe_xthg_copy_train_follower(record, &leader);
    true
}

/// PE train merge success @ `0x4557bb` (`FUN_00455500` kinds `0x0A`/`0x0C`):
/// clear linked slots via `0x401528`, then `FUN_00453280(x, y, height=0, state=0, saved_b=0)`.
pub(super) fn pe_train_merge_finish_explosion(
    city: &mut City,
    clear_slots: &[usize],
    x: usize,
    y: usize,
) -> Option<usize> {
    for &slot in clear_slots {
        clear_xthg_slot_pe(city, slot);
    }
    city.place_pe_explosion_actor(x, y, 0, 0, 0)
}

/// PE `0x402E6E` / `FUN_00455DC0`: tile is eligible for the train-handler merge path.
///
/// Reads XBLD (`0x4CC4F0`) and XUND (`0x4CB1D0`). Returns true off rail/subway network
/// tiles so `FUN_00455500` can clear the consist and place kind-6.
pub(super) fn pe_train_merge_tile_eligible(city: &City, x: usize, y: usize) -> bool {
    if x >= 128 || y >= 128 {
        return false;
    }
    let xbld = u16::from(city.building_at(x, y));
    if (0x2c..0x3f).contains(&xbld) || (0x45..0x49).contains(&xbld) || (0x6c..0x70).contains(&xbld)
    {
        return false;
    }
    if matches!(xbld, 0x4d | 0x4e | 0x5a | 0x5b) {
        return false;
    }
    let xund = u16::from(city.underground_at(x, y));
    if (0x01..0x10).contains(&xund) || matches!(xund, 0x1f | 0x20 | 0x22 | 0x23) {
        return false;
    }
    // Second XBLD band also rejects `0x70` (first band was `0x6c..0x6f`).
    if (0x6c..=0x70).contains(&xbld) {
        return false;
    }
    true
}

/// PE `FUN_00455500` kind-`0x0A` only: RNG gate + one cardinal `XBLD == 0xED` probe.
/// Matching neighbor returns from the handler without merge or movement this tick.
///
/// PE uses `0x402946` (LFSR low bit) then `0x40116d` game-LCG `%2` (+2 when `lane&1`).
pub(super) fn pe_train_ed_neighbor_defers_tick(
    record: &Win95OverlayRecord,
    city: &City,
    lfsr: &mut Win95Lfsr,
    game_rand: &mut Win95GameRand,
) -> bool {
    if record.kind != XTHG_TRAIN_ENGINE {
        return false;
    }
    if lfsr.random_word_lsb() == 0 {
        return false;
    }
    let mut dir = game_rand.random_mod(2) as i16;
    if record.lane & 1 != 0 {
        dir += 2;
    }
    let x = i16::from(record.tile_x);
    let y = i16::from(record.tile_y);
    let (nx, ny) = match dir {
        0 => (x - 1, y),
        1 => (x + 1, y),
        2 => (x, y - 1),
        3 => (x, y + 1),
        _ => return false,
    };
    if !(0..128).contains(&nx) || !(0..128).contains(&ny) {
        return false;
    }
    city.building_at(nx as usize, ny as usize) == 0xed
}

pub(super) fn pe_train_consist_slots(
    engine_slot: usize,
    engine: &Win95OverlayRecord,
    table: &[Win95OverlayRecord],
) -> Option<(usize, Win95OverlayRecord, usize, Win95OverlayRecord)> {
    let first_slot = engine.state as usize;
    let first_car = *table.get(first_slot)?;
    let second_slot = first_car.state as usize;
    let second_car = *table.get(second_slot)?;
    (first_slot != engine_slot
        && second_slot != engine_slot
        && second_slot != first_slot
        && first_car.is_ambient_train_car()
        && second_car.is_ambient_train_car())
    .then_some((first_slot, first_car, second_slot, second_car))
}

/// Run the enabled kind-10/12 PE handler at its XTHG position. Unlike the
/// ambient compatibility pass, the native dispatcher does not visit kinds 11
/// or 13: the engine copies both follower records during its own tile crossing.
pub(super) fn step_pe_train_engine_slot<R: Win95OverlayRng>(
    city: &mut City,
    slot: usize,
    record: Win95OverlayRecord,
    table_snapshot: &[Win95OverlayRecord],
    crt: &mut R,
    lfsr: &mut Win95Lfsr,
    game_rand: &mut Win95GameRand,
) -> bool {
    let old_x = record.tile_x as usize;
    let old_y = record.tile_y as usize;
    let train_consist = pe_train_consist_slots(slot, &record, table_snapshot);
    if pe_train_ed_neighbor_defers_tick(&record, city, lfsr, game_rand) {
        return false;
    }

    if pe_train_merge_tile_eligible(city, old_x, old_y) {
        let mut clear_slots = vec![slot];
        if let Some((first_slot, _, second_slot, _)) = train_consist {
            clear_slots.extend([first_slot, second_slot]);
        }
        let _ = pe_train_merge_finish_explosion(city, &clear_slots, old_x, old_y);
        city.mark_render_dirty(old_x, old_y);
        return true;
    }

    let next_x = record.sub_x as usize;
    let next_y = record.sub_y as usize;
    // PE admits the previously selected `+6/+7` target by its XTXT proxy
    // check. Track routing happens after the engine commits that tile; do not
    // add the ambient adapter's terrain predicate to this native slot path.
    if !tile_in_bounds(next_x, next_y) {
        return false;
    }
    let tail_slot = train_consist.map(|(_, _, tail_slot, _)| tail_slot);
    if !pe_train_target_xtext_is_open(city, tail_slot, next_x, next_y) {
        return false;
    }

    let crossed_tile = (old_x, old_y) != (next_x, next_y);
    let mut next_saved_a = None;
    let mut first_car_after_copy = None;
    let mut second_car_after_copy = None;
    if crossed_tile {
        if let Some((first_slot, first_car, second_slot, second_car)) = train_consist {
            // PE `0x455822`: proxy writes, then first-car→second-car, then engine→first-car.
            next_saved_a = Some(city.advance_ambient_train_xthg_proxies(
                slot,
                record,
                first_slot,
                first_car,
                second_slot,
                second_car,
                next_x,
                next_y,
            ));
            let mut first_after = first_car;
            let mut second_after = second_car;
            pe_xthg_copy_train_follower(&mut second_after, &first_car);
            pe_xthg_copy_train_follower(&mut first_after, &record);
            if let Some(overlays) = city.traffic_overlays_mut() {
                overlays[first_slot] = first_after;
                overlays[second_slot] = second_after;
            }
            first_car_after_copy = Some((first_slot, first_after));
            second_car_after_copy = Some((second_slot, second_after));
        } else {
            next_saved_a = Some(city.move_ambient_train_xthg_proxy(
                slot,
                old_x,
                old_y,
                record.saved_a,
                next_x,
                next_y,
            ));
        }
    }

    let mut next = record;
    let step =
        step_ambient_train_engine_after_preflight(&mut next, city, crt, game_rand, next_x, next_y);
    if let Some(saved_a) = next_saved_a {
        next.saved_a = saved_a;
    }

    match step {
        TrainEngineStep::Blocked => return false,
        TrainEngineStep::NeedsTurnaround => {
            let turnaround_tail = second_car_after_copy
                .or_else(|| train_consist.map(|(_, _, tail_slot, tail_car)| (tail_slot, tail_car)));
            if let Some((tail_slot, tail_car)) = turnaround_tail {
                let mut tail_after = tail_car;
                reverse_pe_train_consist(&mut next, &mut tail_after);
                if let Some(overlays) = city.traffic_overlays_mut() {
                    overlays[slot] = next;
                    overlays[tail_slot] = tail_after;
                }
                city.mark_render_dirty(old_x, old_y);
                city.mark_render_dirty(next.tile_x as usize, next.tile_y as usize);
                if let Some((first_slot, first_car)) = first_car_after_copy {
                    city.mark_render_dirty(first_car.tile_x as usize, first_car.tile_y as usize);
                    if let Some(overlays) = city.traffic_overlays_mut() {
                        overlays[first_slot] = first_car;
                    }
                }
                city.mark_render_dirty(tail_car.tile_x as usize, tail_car.tile_y as usize);
                city.mark_render_dirty(tail_after.tile_x as usize, tail_after.tile_y as usize);
                return true;
            }
            pe_clear_xthg_kind(&mut next);
            pe_zero_tile_xtxt(city, old_x, old_y);
            pe_zero_tile_xtxt(city, next.tile_x as usize, next.tile_y as usize);
        }
        TrainEngineStep::Cleared => {
            if record.is_ambient_train_engine() && !next.is_ambient_train_engine() {
                pe_zero_tile_xtxt(city, old_x, old_y);
            }
        }
        TrainEngineStep::Advanced => {}
    }

    if let Some(overlays) = city.traffic_overlays_mut() {
        overlays[slot] = next;
    }
    city.mark_render_dirty(old_x, old_y);
    if next.kind != 0 {
        city.mark_render_dirty(next.tile_x as usize, next.tile_y as usize);
    }
    if let Some((_, first_car)) = first_car_after_copy {
        city.mark_render_dirty(first_car.tile_x as usize, first_car.tile_y as usize);
    }
    if let Some((_, second_car)) = second_car_after_copy {
        city.mark_render_dirty(second_car.tile_x as usize, second_car.tile_y as usize);
    }
    true
}

/// PE FUN_00401708 / FUN_00455FC0: swap the engine and tail spatial payloads,
/// then reverse the engine's cardinal lane from the old tail lane. The tail's
/// lane and draw code remain unchanged; links, kinds, heights, and +9/+B stay put.
pub(super) fn reverse_pe_train_consist(
    engine: &mut Win95OverlayRecord,
    tail_car: &mut Win95OverlayRecord,
) {
    let engine_tile_x = engine.tile_x;
    let engine_tile_y = engine.tile_y;
    let engine_sub_x = engine.sub_x;
    let engine_sub_y = engine.sub_y;
    let engine_saved_a = engine.saved_a;
    let tail_tile_x = tail_car.tile_x;
    let tail_tile_y = tail_car.tile_y;
    let tail_lane = tail_car.lane;
    let tail_saved_a = tail_car.saved_a;

    engine.tile_x = tail_tile_x;
    engine.tile_y = tail_tile_y;
    engine.sub_x = tail_tile_x;
    engine.sub_y = tail_tile_y;
    engine.saved_a = tail_saved_a;
    engine.lane = tail_lane.wrapping_add(2) & 3;
    engine.target_x = engine.lane.wrapping_add(4);

    tail_car.tile_x = engine_tile_x;
    tail_car.tile_y = engine_tile_y;
    tail_car.sub_x = engine_sub_x;
    tail_car.sub_y = engine_sub_y;
    tail_car.saved_a = engine_saved_a;
}

/// PE `FUN_004029FF` / `0x455EF0`: copy follower fields from `source` into `dest`.
///
/// Copies `+0,+1,+3,+4,+6,+7,+8,+A` (kind, lane, tile, subtile, target_x, saved_a), then
/// converts destination kind `0x0A -> 0x0B` or `0x0C -> 0x0D`. Leaves `+2/+5/+9/+B`
/// (state, height, target_y, saved_b) unchanged on `dest`.
pub(super) fn pe_xthg_copy_train_follower(
    dest: &mut Win95OverlayRecord,
    source: &Win95OverlayRecord,
) {
    dest.kind = source.kind;
    dest.lane = source.lane;
    dest.tile_x = source.tile_x;
    dest.tile_y = source.tile_y;
    dest.sub_x = source.sub_x;
    dest.sub_y = source.sub_y;
    dest.target_x = source.target_x;
    dest.saved_a = source.saved_a;
    match dest.kind {
        XTHG_TRAIN_ENGINE => dest.kind = XTHG_TRAIN_CAR,
        XTHG_SUBWAY_TRAIN_ENGINE => dest.kind = XTHG_SUBWAY_TRAIN_CAR,
        _ => {}
    }
}
