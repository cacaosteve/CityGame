//! Isolated Mac CODE 6 rail marker and linked-record movement.

use super::rail_routes::{
    mac_actor_choose_route, mac_actor_route_candidate, mac_rail_current_cell_requires_teardown,
};

/// CODE 6 export 120 as called by the monthly station scan. D6 initially
/// holds the scan column and D5 the zone nibble. The original checks those
/// registers before calculating each candidate, including the preceding
/// candidate on subsequent iterations. The return value here reports whether
/// a train was actually constructed, not the native helper's leftover D0.
/// The live CODE 8 monthly scan calls this with its shared CODE 13 game LCG.
pub(in crate::app) fn mac_spawn_rail_near_station(
    city: &mut City,
    row: i16,
    col: i16,
    mut draw_offset: impl FnMut() -> u16,
    mut draw_order: impl FnMut() -> u16,
) -> bool {
    if row < 0 || col < 0 || row as usize >= CITY_HEIGHT || col as usize >= CITY_WIDTH {
        return false;
    }
    let mut prior_d6 = col;
    let mut prior_d5 = i16::from(
        city.zone_at(row as usize, col as usize)
            .map_or(0, |z| z.zone_id()),
    );
    for (dr, dc) in [(0_i16, -1_i16), (2, 0), (0, 2), (-1, 0)] {
        if !(0..128).contains(&prior_d6) || !(0..128).contains(&prior_d5) {
            break;
        }
        prior_d6 = row.wrapping_add(dr);
        prior_d5 = col.wrapping_add(dc);
        if mac_spawn_rail_at(city, prior_d6, prior_d5, &mut draw_offset, &mut draw_order) {
            return true;
        }
    }
    false
}
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};
use crate::mac_random::MacAppLfsrRng;
use crate::win95_overlay::{Win95OverlayRecord, XTHG_PROXY_BASE, XTHG_RECORD_COUNT};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum MacRailAdmission {
    Station,
    Retire,
    Move,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum MacRailPrefixOutcome {
    Station,
    Retired,
    Blocked,
    Moved,
}

/// Execute one normal-state Mac rail handler with a shared app draw stream.
/// A sound request is returned only when the post-movement path runs. Actor
/// sweep scheduling and sound playback remain the caller's responsibility.
#[allow(dead_code)]
pub(super) fn mac_rail_handler(
    city: &mut City,
    head_slot: usize,
    mut app_draw_bound_two: impl FnMut() -> u16,
    quickdraw: impl FnOnce() -> u16,
) -> Option<(MacRailPrefixOutcome, Option<bool>)> {
    let outcome = mac_rail_handler_prefix(city, head_slot, &mut app_draw_bound_two)?;
    if outcome != MacRailPrefixOutcome::Moved {
        return Some((outcome, None));
    }
    let records = city.traffic_overlays()?;
    let middle = usize::from(records.get(head_slot)?.state);
    let tail = usize::from(records.get(middle)?.state);
    let sound = mac_rail_handler_suffix(city, head_slot, tail, quickdraw, app_draw_bound_two)?;
    Some((outcome, Some(sound)))
}

/// The normal-state `0xec04..0xef34` handler suffix. The CODE 13 export-203
/// calls are zero in the recovered initialized state; the two callbacks are
/// the distinct QuickDraw and conditional app-LCG draws. The return value
/// reports the native sound request, not whether sound playback succeeded.
#[allow(dead_code)]
pub(super) fn mac_rail_handler_suffix(
    city: &mut City,
    head_slot: usize,
    tail_slot: usize,
    quickdraw: impl FnOnce() -> u16,
    draw_bound_two: impl FnOnce() -> u16,
) -> Option<bool> {
    if head_slot >= XTHG_RECORD_COUNT || tail_slot >= XTHG_RECORD_COUNT || head_slot == tail_slot {
        return None;
    }
    let mut head = *city.traffic_overlays()?.get(head_slot)?;
    let row = i16::from(head.tile_x);
    let col = i16::from(head.tile_y);
    let initial = head.lane & 0x0f;
    if row as usize >= CITY_HEIGHT || col as usize >= CITY_WIDTH || initial >= 4 {
        return None;
    }
    if (0x6c..=0x70).contains(&city.building_at(row as usize, col as usize)) {
        head.kind = if head.kind == 10 { 12 } else { 10 };
    }
    let kind = u16::from(head.kind);
    const DROW: [i16; 4] = [0, 1, 0, -1];
    const DCOL: [i16; 4] = [-1, 0, 1, 0];
    const SHAPE: [[u8; 4]; 4] = [[0, 1, 2, 7], [1, 2, 3, 4], [2, 3, 4, 5], [7, 4, 5, 6]];
    // CODE 6 `0xec94..0xeccc` calls CODE 13 export 203 with bounds 4 and 2.
    // The reconstructed A5-2 app-LFSR begins at zero and has no loaded-code
    // writer, so the native zero/zero branch chooses heading+3.
    let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    let selected = if app_lfsr.bounded(4) != 0 {
        initial
    } else if app_lfsr.bounded(2) != 0 {
        (initial + 1) & 3
    } else {
        (initial + 3) & 3
    };
    let candidate = |direction: u8| {
        let i = usize::from(direction);
        (row + DROW[i], col + DCOL[i])
    };
    let (r, c) = candidate(selected);
    let candidate_is_open = mac_actor_route_candidate(city, r, c, kind);
    let sound_requested = quickdraw() & 0xff == 0;
    if candidate_is_open {
        head.lane = initial;
        head.target_x = selected * 2;
        head.sub_x = r as u8;
        head.sub_y = c as u8;
        city.set_mac_object_slot_record(head_slot, head);
    } else if selected != initial
        && mac_actor_route_candidate(city, candidate(initial).0, candidate(initial).1, kind)
    {
        let (r, c) = candidate(initial);
        head.lane = initial;
        head.target_x = initial * 2;
        head.sub_x = r as u8;
        head.sub_y = c as u8;
        city.set_mac_object_slot_record(head_slot, head);
    } else if let Some(next) =
        mac_actor_choose_route(city, row, col, i16::from(initial), kind, draw_bound_two)
    {
        let (r, c) = candidate(next);
        head.lane = next;
        head.target_x = SHAPE[usize::from(initial)][usize::from(next)];
        head.sub_x = r as u8;
        head.sub_y = c as u8;
        city.set_mac_object_slot_record(head_slot, head);
    } else {
        city.set_mac_object_slot_record(head_slot, head);
        if !mac_turn_rail_consist(city, head_slot, tail_slot) {
            return None;
        }
    }
    Some(sound_requested)
}

/// Execute the handler through the end of marker movement, or its earlier
/// station/retirement exits. Rerouting and actor scheduling are separate.
#[allow(dead_code)]
pub(super) fn mac_rail_handler_prefix(
    city: &mut City,
    head_slot: usize,
    draw_bound_two: impl FnOnce() -> u16,
) -> Option<MacRailPrefixOutcome> {
    let records = city.traffic_overlays()?;
    let head = *records.get(head_slot)?;
    if head_slot >= XTHG_RECORD_COUNT {
        return None;
    }
    let admission = mac_rail_admission(city, head, draw_bound_two)?;
    if admission == MacRailAdmission::Station {
        return Some(MacRailPrefixOutcome::Station);
    }
    let middle = usize::from(head.state);
    let tail = usize::from(records.get(middle)?.state);
    if middle >= XTHG_RECORD_COUNT
        || tail >= XTHG_RECORD_COUNT
        || head_slot == middle
        || head_slot == tail
        || middle == tail
    {
        return None;
    }
    if admission == MacRailAdmission::Retire {
        return mac_retire_rail_consist(city, head_slot, middle, tail)
            .then_some(MacRailPrefixOutcome::Retired);
    }
    // Distinguish the original occupancy refusal from the Rust worker's
    // invalid-coordinate guard rather than treating invalid input as blocked.
    if usize::from(head.sub_x) >= CITY_HEIGHT || usize::from(head.sub_y) >= CITY_WIDTH {
        return None;
    }
    let marker = city.text_overlay_at(usize::from(head.sub_x), usize::from(head.sub_y))?;
    if marker >= XTHG_PROXY_BASE && marker != XTHG_PROXY_BASE + tail as u8 {
        return Some(MacRailPrefixOutcome::Blocked);
    }
    mac_move_rail_consist(city, head_slot, middle, tail).then_some(MacRailPrefixOutcome::Moved)
}

/// The opening of CODE 6 `FUN_0000e848`, before occupancy and movement.
/// The callback returns the native bound-2 app RNG draw and is only called
/// for kind 10. Invalid head positions are rejected before any draw.
#[allow(dead_code)]
pub(super) fn mac_rail_admission(
    city: &City,
    head: Win95OverlayRecord,
    draw_bound_two: impl FnOnce() -> u16,
) -> Option<MacRailAdmission> {
    let (row, col) = (i16::from(head.tile_x), i16::from(head.tile_y));
    if row as usize >= CITY_HEIGHT || col as usize >= CITY_WIDTH {
        return None;
    }
    if head.kind == 10 {
        let selector = draw_bound_two().wrapping_add(if head.lane & 1 != 0 { 2 } else { 0 });
        let delta = match selector {
            0 => Some((-1, 0)),
            1 => Some((1, 0)),
            2 => Some((0, -1)),
            3 => Some((0, 1)),
            _ => None,
        };
        if let Some((dr, dc)) = delta {
            let (r, c) = (row + dr, col + dc);
            if r >= 0
                && c >= 0
                && (r as usize) < CITY_HEIGHT
                && (c as usize) < CITY_WIDTH
                && city.building_at(r as usize, c as usize) == 0xed
            {
                return Some(MacRailAdmission::Station);
            }
        }
    }
    Some(if mac_rail_current_cell_requires_teardown(city, row, col) {
        MacRailAdmission::Retire
    } else {
        MacRailAdmission::Move
    })
}

/// CODE 6 `FUN_0000cd2c`: one source-cell train constructor called by the
/// monthly station search. The caller supplies the two separate bounded draws
/// in native call order. It shares the City-owned rail-head count with the
/// actor sweep; whole-month RNG chronology remains unverified.
pub(super) fn mac_spawn_rail_at(
    city: &mut City,
    row: i16,
    col: i16,
    draw_offset: impl FnOnce() -> u16,
    draw_order: impl FnOnce() -> u16,
) -> bool {
    if (city.mac_rail_actor_counter as i16) >= 5
        || !(2..=124).contains(&row)
        || !(2..=124).contains(&col)
    {
        return false;
    }
    let (x, y) = (row as usize, col as usize);
    if !(0x2c..=0x35).contains(&city.building_at(x, y))
        || city.text_overlay_at(x, y).unwrap_or(0) != 0
    {
        return false;
    }
    let offset = draw_offset() as i16;
    let Some(direction) = mac_actor_choose_route(city, row, col, offset, 10, draw_order) else {
        return false;
    };
    const DROW: [i16; 4] = [0, 1, 0, -1];
    const DCOL: [i16; 4] = [-1, 0, 1, 0];
    let marker = city.text_overlay_at(x, y).unwrap_or(0);
    let records = city.mac_object_slot_records_mut();
    fn allocate(records: &mut [Win95OverlayRecord], kind: u8) -> usize {
        // Native export 113 returns slot zero when 1..=39 are all occupied.
        let slot = (1..XTHG_RECORD_COUNT)
            .find(|&i| records[i].kind == 0)
            .unwrap_or(0);
        records[slot].kind = kind;
        slot
    }
    let head = allocate(records, 10);
    let middle = allocate(records, 11);
    let tail = allocate(records, 11);
    // Keep the original write order: allocation can return slot zero more
    // than once, and previously unused record bytes are not cleared.
    for slot in [head, middle, tail] {
        records[slot].lane = direction;
    }
    for slot in [head, middle, tail] {
        records[slot].tile_x = row as u8;
    }
    for slot in [head, middle, tail] {
        records[slot].tile_y = col as u8;
    }
    records[head].sub_x = row.wrapping_add(DROW[direction as usize]) as u8;
    records[middle].sub_x = row as u8;
    records[tail].sub_x = row as u8;
    records[head].sub_y = col.wrapping_add(DCOL[direction as usize]) as u8;
    records[middle].sub_y = col as u8;
    records[tail].sub_y = col as u8;
    for slot in [head, middle, tail] {
        records[slot].height = 0;
    }
    records[head].saved_a = marker;
    records[middle].saved_a = 0;
    records[tail].saved_a = 0;
    records[head].state = middle as u8;
    records[middle].state = tail as u8;
    city.set_text_overlay(x, y, XTHG_PROXY_BASE.wrapping_add(head as u8));
    city.mac_rail_actor_counter = city.mac_rail_actor_counter.wrapping_add(1);
    true
}

/// CODE 6 `FUN_0000f142`: copy the moving fields while retaining the
/// destination's link and slot-local bytes. These records share the XTHG
/// storage shape with Win95 but use the Mac handler's meanings here.
fn mac_copy_rail_follower(
    source: Win95OverlayRecord,
    mut destination: Win95OverlayRecord,
) -> Win95OverlayRecord {
    destination.kind = match source.kind {
        10 => 11,
        12 => 13,
        kind => kind,
    };
    destination.lane = source.lane;
    destination.tile_x = source.tile_x;
    destination.tile_y = source.tile_y;
    destination.sub_x = source.sub_x;
    destination.sub_y = source.sub_y;
    destination.target_x = source.target_x;
    destination.saved_a = source.saved_a;
    destination
}

/// CODE 6 `FUN_0000f26a`: reverse a consist after route choices are
/// exhausted. No map markers or middle-record bytes are changed here.
#[allow(dead_code)]
pub(super) fn mac_turn_rail_consist(city: &mut City, head: usize, tail: usize) -> bool {
    if head >= XTHG_RECORD_COUNT || tail >= XTHG_RECORD_COUNT || head == tail {
        return false;
    }
    let Some(records) = city.traffic_overlays() else {
        return false;
    };
    let (Some(&old_head), Some(&old_tail)) = (records.get(head), records.get(tail)) else {
        return false;
    };
    let mut new_tail = old_tail;
    new_tail.tile_x = old_head.tile_x;
    new_tail.tile_y = old_head.tile_y;
    new_tail.sub_x = old_head.sub_x;
    new_tail.sub_y = old_head.sub_y;
    new_tail.saved_a = old_head.saved_a;
    let mut new_head = old_head;
    new_head.tile_x = old_tail.tile_x;
    new_head.tile_y = old_tail.tile_y;
    new_head.sub_x = old_tail.tile_x;
    new_head.sub_y = old_tail.tile_y;
    new_head.saved_a = old_tail.saved_a;
    new_head.lane = (old_tail.lane.wrapping_add(2)) & 3;
    new_head.target_x = 4 + new_head.lane;
    city.set_mac_object_slot_record(tail, new_tail);
    city.set_mac_object_slot_record(head, new_head);
    true
}

/// CODE 6 `0xe99e..0xeabd`: retire the three linked records and their
/// owned markers, then attempt the native kind-6 effect at the head cell.
/// This isolated worker does not schedule actors or alter the sweep counter.
#[allow(dead_code)]
pub(super) fn mac_retire_rail_consist(
    city: &mut City,
    head: usize,
    middle: usize,
    tail: usize,
) -> bool {
    if [head, middle, tail]
        .iter()
        .any(|&slot| slot >= XTHG_RECORD_COUNT)
        || head == middle
        || head == tail
        || middle == tail
    {
        return false;
    }
    let Some(records) = city.traffic_overlays() else {
        return false;
    };
    let (Some(&lead), Some(&mid), Some(&last)) =
        (records.get(head), records.get(middle), records.get(tail))
    else {
        return false;
    };
    let members = [(head, lead), (middle, mid), (tail, last)];
    if members
        .iter()
        .any(|(_, r)| usize::from(r.tile_x) >= CITY_HEIGHT || usize::from(r.tile_y) >= CITY_WIDTH)
    {
        return false;
    }
    for (slot, mut record) in members {
        record.kind = 0;
        city.set_mac_object_slot_record(slot, record);
    }
    for (slot, record) in members {
        let (x, y) = (usize::from(record.tile_x), usize::from(record.tile_y));
        if city.text_overlay_at(x, y) == Some(XTHG_PROXY_BASE + slot as u8) {
            city.set_text_overlay(x, y, 0);
        }
    }
    let (x, y) = (usize::from(lead.tile_x), usize::from(lead.tile_y));
    let marker = city.text_overlay_at(x, y).unwrap_or(0);
    let records = city.traffic_overlays().unwrap();
    let free = (1..XTHG_RECORD_COUNT).find(|&slot| records[slot].kind == 0);
    if let Some(slot) = free.filter(|_| marker < XTHG_PROXY_BASE) {
        let mut effect = records[slot];
        effect.kind = 6;
        effect.lane = 0;
        effect.state = 0;
        effect.tile_x = lead.tile_x;
        effect.tile_y = lead.tile_y;
        effect.height = 0;
        effect.sub_x = 8;
        effect.sub_y = 8;
        effect.saved_a = marker;
        effect.saved_b = 0;
        city.set_mac_object_slot_record(slot, effect);
        city.set_text_overlay(x, y, XTHG_PROXY_BASE + slot as u8);
    }
    true
}

/// CODE 6 `0xeada..0xec04`, after route selection: check destination
/// occupancy, rotate the tail and middle markers/records, and move the head.
/// Returns false for blocked or invalid inputs without changing the city.
/// The caller owns timing, RNG, route selection, and kind-specific teardown.
#[allow(dead_code)]
pub(super) fn mac_move_rail_consist(
    city: &mut City,
    head: usize,
    middle: usize,
    tail: usize,
) -> bool {
    if head >= XTHG_RECORD_COUNT
        || middle >= XTHG_RECORD_COUNT
        || tail >= XTHG_RECORD_COUNT
        || head == middle
        || head == tail
        || middle == tail
    {
        return false;
    }
    let Some(records) = city.traffic_overlays() else {
        return false;
    };
    let (Some(&mut_lead), Some(&mid), Some(&last)) =
        (records.get(head), records.get(middle), records.get(tail))
    else {
        return false;
    };
    let mut lead = mut_lead;
    // CODE 6 uses XTHG +3/+4 and +6/+7 as tile_x/tile_y. Its x-major map
    // pointer and City accessors share that same axis order.
    let in_bounds = |x: u8, y: u8| usize::from(x) < CITY_WIDTH && usize::from(y) < CITY_HEIGHT;
    if !in_bounds(lead.tile_x, lead.tile_y)
        || !in_bounds(mid.tile_x, mid.tile_y)
        || !in_bounds(last.tile_x, last.tile_y)
        || !in_bounds(lead.sub_x, lead.sub_y)
    {
        return false;
    }
    let destination = (usize::from(lead.sub_x), usize::from(lead.sub_y));
    let marker = city
        .text_overlay_at(destination.0, destination.1)
        .unwrap_or(0);
    if marker >= XTHG_PROXY_BASE && marker != XTHG_PROXY_BASE.wrapping_add(tail as u8) {
        return false;
    }
    if (lead.tile_x, lead.tile_y) != (lead.sub_x, lead.sub_y) {
        city.set_text_overlay(
            usize::from(lead.tile_x),
            usize::from(lead.tile_y),
            XTHG_PROXY_BASE.wrapping_add(middle as u8),
        );
        city.set_text_overlay(
            usize::from(mid.tile_x),
            usize::from(mid.tile_y),
            XTHG_PROXY_BASE.wrapping_add(tail as u8),
        );
        city.set_text_overlay(
            usize::from(last.tile_x),
            usize::from(last.tile_y),
            last.saved_a,
        );
        city.set_mac_object_slot_record(tail, mac_copy_rail_follower(mid, last));
        city.set_mac_object_slot_record(middle, mac_copy_rail_follower(lead, mid));
        lead.saved_a = city
            .text_overlay_at(destination.0, destination.1)
            .unwrap_or(0);
        city.set_text_overlay(
            destination.0,
            destination.1,
            XTHG_PROXY_BASE.wrapping_add(head as u8),
        );
    }
    lead.tile_x = lead.sub_x;
    lead.tile_y = lead.sub_y;
    city.set_mac_object_slot_record(head, lead);
    true
}

#[cfg(test)]
mod tests;
