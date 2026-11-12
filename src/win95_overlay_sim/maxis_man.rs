//! Win95 Maxis Man XTHG state and movement workers.
//!
//! Slot clearing and shared terrain/XTXT helpers remain in the parent adapter.

use super::{
    City, MAXIS_MAN_SUBTILE_STEP, PeMaxisManState, SAVED_B_SLOT_LINK, SCAN_RADIUS, Win95CrtRand,
    Win95OverlayRng, Win95OverlaySim, advance_subtile, clear_xthg_slot_pe, pe_clear_xthg_kind,
    pe_zero_tile_xtxt, refresh_overlay_height, tile_in_bounds,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::win95_overlay::{
    Win95OverlayRecord, XTHG_EXPLOSION_XTXT_GATE, XTHG_FIRST_USABLE_SLOT, XTHG_MAXIS_MAN,
    XTHG_RECORD_COUNT, xthg_direction_delta, xthg_proxy_for_slot,
};

impl Win95OverlaySim {
    /// Step PE `XTHG_MAXIS_MAN` on the Things-walker cadence.
    ///
    /// Handler rolls (`FUN_00455020`) share the live process CRT stream, not the
    /// city-name overlay archaeology `sim_rng`.
    pub fn tick_pe_maxis_man(&mut self, city: &mut City, crt: &mut Win95CrtRand) -> bool {
        if !city.has_traffic_overlays() {
            return false;
        }
        let table_snapshot = city
            .traffic_overlays()
            .map(|table| table.to_vec())
            .unwrap_or_default();
        let mut changed = false;
        for slot in XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT {
            if update_maxis_man_slot(city, slot, &table_snapshot, crt) {
                changed = true;
            }
        }
        if changed {
            city.mark_traffic_layers_dirty();
        }
        changed
    }

    /// Step one kind-16 record at its position in the native XTHG slot walk.
    /// Take a fresh table view so earlier handlers' slot and proxy changes are visible.
    pub fn tick_pe_maxis_man_slot(
        &mut self,
        city: &mut City,
        slot: usize,
        crt: &mut Win95CrtRand,
    ) -> bool {
        let table_snapshot = city
            .traffic_overlays()
            .map(|table| table.to_vec())
            .unwrap_or_default();
        let changed = update_maxis_man_slot(city, slot, &table_snapshot, crt);
        if changed {
            city.mark_traffic_layers_dirty();
        }
        changed
    }
}

pub(super) fn update_maxis_man_slot<R: Win95OverlayRng>(
    city: &mut City,
    slot: usize,
    table: &[Win95OverlayRecord],
    rng: &mut R,
) -> bool {
    let Some(mut record) = table.get(slot).copied() else {
        return false;
    };
    if record.kind != XTHG_MAXIS_MAN {
        return false;
    }
    let old_x = record.tile_x as usize;
    let old_y = record.tile_y as usize;
    let mut changed = match record.state {
        state if state == PeMaxisManState::Inactive as u8 => {
            update_maxis_man_search(&mut record, city, table, rng, slot)
        }
        state if state == PeMaxisManState::LightActive as u8 => {
            update_maxis_man_light(&mut record, city, rng, slot)
        }
        state if state == PeMaxisManState::HeavyActive as u8 => {
            update_maxis_man_heavy(&mut record, city, slot)
        }
        _ => false,
    };
    if record.kind != 0 {
        let height_before = record.height;
        refresh_overlay_height(&mut record, city, table);
        if record.height != height_before {
            changed = true;
        }
    }
    if changed {
        if let Some(overlays) = city.traffic_overlays_mut() {
            overlays[slot] = record;
        }
        if old_x < CITY_WIDTH && old_y < CITY_HEIGHT {
            city.mark_render_dirty(old_x, old_y);
        }
        if (record.tile_x as usize) < CITY_WIDTH && (record.tile_y as usize) < CITY_HEIGHT {
            city.mark_render_dirty(record.tile_x as usize, record.tile_y as usize);
        }
    }
    changed
}

/// PE state 1: `rand() & 3` gates an occasional random-direction move.
pub(super) fn update_maxis_man_light<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    rng: &mut R,
    slot: usize,
) -> bool {
    if rng.roll_mod_is_zero(4) {
        record.state = PeMaxisManState::Inactive as u8;
        return true;
    }
    let direction = (rng.random_u16() & 7) as u8;
    let (dx_unit, dy_unit) = xthg_direction_delta(direction);
    let (_, tile_dx) = advance_subtile(record.sub_x as i16 + dx_unit * MAXIS_MAN_SUBTILE_STEP);
    let (_, tile_dy) = advance_subtile(record.sub_y as i16 + dy_unit * MAXIS_MAN_SUBTILE_STEP);
    let next_x = record.tile_x as i16 + tile_dx;
    let next_y = record.tile_y as i16 + tile_dy;
    if next_x < 0 || next_y < 0 || next_x >= CITY_WIDTH as i16 || next_y >= CITY_HEIGHT as i16 {
        return false;
    }
    // PE state 1 @ `0x455314`: candidate XTXT `>= 0xC9` → no-op (skip this direction).
    if city
        .text_overlay_at(next_x as usize, next_y as usize)
        .unwrap_or(0)
        >= XTHG_EXPLOSION_XTXT_GATE
    {
        return false;
    }
    move_maxis_man(record, city, direction, MAXIS_MAN_SUBTILE_STEP, slot)
}

/// PE state 2: the heavy branch calls the movement helper twice per dispatch.
fn update_maxis_man_heavy(record: &mut Win95OverlayRecord, city: &mut City, slot: usize) -> bool {
    let direction = record.lane;
    let first = move_maxis_man(record, city, direction, MAXIS_MAN_SUBTILE_STEP, slot);
    if record.kind == XTHG_MAXIS_MAN {
        return move_maxis_man(record, city, direction, MAXIS_MAN_SUBTILE_STEP, slot) || first;
    }
    first
}

/// PE `FUN_00403071` → `FUN_004516F0` for Maxis Man: XTXT occupancy plane (not XTRF).
/// Heavy dest walks the tile carry past XTXT `>= 0xC9`; clear neighbor paints `slot-0x37`.
pub(super) fn move_maxis_man(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    direction: u8,
    step: i16,
    slot: usize,
) -> bool {
    let old_x = record.tile_x as usize;
    let old_y = record.tile_y as usize;
    let (dx_unit, dy_unit) = xthg_direction_delta(direction);
    let (sub_x, tile_dx) = advance_subtile(record.sub_x as i16 + dx_unit * step);
    let (sub_y, tile_dy) = advance_subtile(record.sub_y as i16 + dy_unit * step);
    let new_x = record.tile_x as i16 + tile_dx;
    let new_y = record.tile_y as i16 + tile_dy;
    if new_x < 0 || new_y < 0 || new_x >= CITY_WIDTH as i16 || new_y >= CITY_HEIGHT as i16 {
        pe_zero_tile_xtxt(city, old_x, old_y);
        pe_clear_xthg_kind(record);
        return true;
    }
    let crossed = new_x as usize != old_x || new_y as usize != old_y;
    record.lane = direction;
    record.sub_x = sub_x as u8;
    record.sub_y = sub_y as u8;
    if !crossed {
        return true;
    }
    // PE `FUN_004516F0` @ `0x451821`: restore +A onto old XTXT before the dest loop.
    if tile_in_bounds(old_x, old_y) {
        city.set_text_overlay(old_x, old_y, record.saved_a);
    }
    let mut cx = new_x;
    let mut cy = new_y;
    loop {
        let nx = cx as usize;
        let ny = cy as usize;
        let dest_xtxt = city.text_overlay_at(nx, ny).unwrap_or(0);
        if dest_xtxt < XTHG_EXPLOSION_XTXT_GATE {
            record.saved_a = dest_xtxt;
            record.tile_x = nx as u8;
            record.tile_y = ny as u8;
            if let Some(proxy) = xthg_proxy_for_slot(slot) {
                city.set_text_overlay(nx, ny, proxy);
            }
            return true;
        }
        record.tile_x = nx as u8;
        record.tile_y = ny as u8;
        cx += tile_dx;
        cy += tile_dy;
        if cx < 0 || cy < 0 || cx >= CITY_WIDTH as i16 || cy >= CITY_HEIGHT as i16 {
            pe_zero_tile_xtxt(city, nx, ny);
            pe_clear_xthg_kind(record);
            return true;
        }
    }
}

/// PE `FUN_00455020` state 0: follow the linked/current target when XTXT is
/// `>= 0xf1`, otherwise scan the current tile's `±0x20` window in x-outer,
/// y-inner order and switch to heavy movement when no target is found.
///
/// Before moving toward a target, PE probes the ahead tile's `0x4CA600`
/// (XTXT) occupancy (`0x4551e4`). When that byte is the linked slot's proxy and
/// `rand() & 3 == 0`, it clears the linked slot (`0x401528`) and calls
/// `FUN_00453280` (`0x45526b`) to place a kind-6 explosion, then enters state 2.
pub(super) fn update_maxis_man_search<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    table: &[Win95OverlayRecord],
    rng: &mut R,
    slot: usize,
) -> bool {
    if record.saved_b < SAVED_B_SLOT_LINK {
        // PE `0x455061`: linked `saved_b` always feeds the direction / ahead-XTXT path.
        let (target_x, target_y) = table
            .get(record.saved_b as usize)
            .map(|parent| (parent.tile_x as usize, parent.tile_y as usize))
            .unwrap_or((record.target_x as usize, record.target_y as usize));
        if tile_in_bounds(target_x, target_y) {
            let direction = maxis_man_direction_toward(
                record.tile_x as i16,
                record.tile_y as i16,
                target_x as i16,
                target_y as i16,
            );
            record.lane = direction;
            return maxis_man_search_try_ahead_move(record, city, direction, rng, slot);
        }
    } else {
        let target_x = record.target_x as usize;
        let target_y = record.target_y as usize;
        // PE `0x4550ae`: unlinked target follow when XTXT `>= 0xf1` (not XBLD).
        if tile_in_bounds(target_x, target_y)
            && city.text_overlay_at(target_x, target_y).unwrap_or(0) >= 0xf1
        {
            let direction = maxis_man_direction_toward(
                record.tile_x as i16,
                record.tile_y as i16,
                target_x as i16,
                target_y as i16,
            );
            record.lane = direction;
            return maxis_man_search_try_ahead_move(record, city, direction, rng, slot);
        }
    }

    record.state = PeMaxisManState::HeavyActive as u8;
    let origin_x = record.tile_x as i16;
    let origin_y = record.tile_y as i16;
    for x in (origin_x - SCAN_RADIUS)..=(origin_x + SCAN_RADIUS) {
        for y in (origin_y - SCAN_RADIUS)..=(origin_y + SCAN_RADIUS) {
            if x >= 0
                && y >= 0
                && x < CITY_WIDTH as i16
                && y < CITY_HEIGHT as i16
                // PE `0x455130`: scan hit when XTXT `>= 0xf1` (not XBLD).
                && city
                    .text_overlay_at(x as usize, y as usize)
                    .unwrap_or(0)
                    >= 0xf1
            {
                record.target_x = x as u8;
                record.target_y = y as u8;
                record.state = PeMaxisManState::Inactive as u8;
                return true;
            }
        }
    }
    true
}

/// PE `0x455194`–`0x45528e`: XTXT ahead-tile gate before the state-0 move helper.
fn maxis_man_search_try_ahead_move<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    direction: u8,
    rng: &mut R,
    slot: usize,
) -> bool {
    let (dx, dy) = xthg_direction_delta(direction);
    let ahead_x = record.tile_x as i16 + dx;
    let ahead_y = record.tile_y as i16 + dy;
    if ahead_x < 0 || ahead_y < 0 || ahead_x >= CITY_WIDTH as i16 || ahead_y >= CITY_HEIGHT as i16 {
        return move_maxis_man(record, city, direction, MAXIS_MAN_SUBTILE_STEP, slot);
    }
    let ax = ahead_x as usize;
    let ay = ahead_y as usize;
    let ahead_xtxt = city.text_overlay_at(ax, ay).unwrap_or(0);

    // PE `cmp ax, 0xfa` / `jg`: clear occupancy then still attempt the move helper.
    if ahead_xtxt > 0xfa {
        if rng.random_u16() & 1 != 0 {
            city.set_text_overlay(ax, ay, 0);
            return move_maxis_man(record, city, direction, MAXIS_MAN_SUBTILE_STEP, slot);
        }
        return false;
    }

    // PE `cmp ax, 0xc9` / `jge`: proxy/occupancy band — may spawn kind 6.
    if ahead_xtxt >= XTHG_EXPLOSION_XTXT_GATE {
        let linked = record.saved_b;
        let is_linked_proxy = xthg_proxy_for_slot(linked as usize) == Some(ahead_xtxt);
        if is_linked_proxy && rng.random_u16() & 3 == 0 {
            clear_xthg_slot_pe(city, linked as usize);
            let _ = city.place_pe_explosion_actor(ax, ay, record.height, 0, 1);
            record.state = PeMaxisManState::HeavyActive as u8;
            return true;
        }
        record.state = PeMaxisManState::LightActive as u8;
        return true;
    }

    let moved = move_maxis_man(record, city, direction, MAXIS_MAN_SUBTILE_STEP, slot);
    // PE `0x45528e..0x4552c7`: look ahead again from the new tile.
    let next_x = i16::from(record.tile_x) + dx;
    let next_y = i16::from(record.tile_y) + dy;
    if record.kind == XTHG_MAXIS_MAN
        && (0..CITY_WIDTH as i16).contains(&next_x)
        && (0..CITY_HEIGHT as i16).contains(&next_y)
        && city
            .text_overlay_at(next_x as usize, next_y as usize)
            .unwrap_or(0)
            < XTHG_EXPLOSION_XTXT_GATE
    {
        return move_maxis_man(record, city, direction, MAXIS_MAN_SUBTILE_STEP, slot) || moved;
    }
    moved
}

/// PE `0x402dbf` direction table used by the Maxis Man target branch.
fn maxis_man_direction_toward(x: i16, y: i16, target_x: i16, target_y: i16) -> u8 {
    let dx = target_x - x;
    let dy = target_y - y;
    let abs_dx = dx.unsigned_abs();
    let abs_dy = dy.unsigned_abs();
    if (u32::from(abs_dy) + 1) / 2 > u32::from(abs_dx) {
        if dy < 0 { 0 } else { 4 }
    } else if dx < 0 {
        if (u32::from(abs_dx) + 1) / 2 > u32::from(abs_dy) {
            6
        } else if dy < 0 {
            7
        } else {
            5
        }
    } else if (u32::from(abs_dx) + 1) / 2 > u32::from(abs_dy) {
        2
    } else if dy < 0 {
        1
    } else {
        3
    }
}
