use super::*;
use crate::mac_random::MacToolboxRng;
use crate::win95_overlay::{
    Win95OverlayRecord, XTHG_EXPLOSION, XTHG_FIRST_USABLE_SLOT, XTHG_MAXIS_MAN, XTHG_PROXY_BASE,
    XTHG_RECORD_COUNT,
};

const KIND16_MOVE_STRIDE: i16 = 16;
const KIND16_SEARCH_RADIUS: i16 = 32;

pub(super) fn tick(
    city: &mut City,
    slot: usize,
    record: &mut Win95OverlayRecord,
    quickdraw: &mut MacToolboxRng,
    sounds: &mut Vec<PlacementSound>,
) {
    match record.state as i8 {
        0 => tick_state_zero(city, slot, record, quickdraw, sounds),
        1 => tick_state_one(city, slot, record, quickdraw),
        2 => {
            let heading = record.lane;
            super::mac_move_one_step_with_heading(city, slot, record, KIND16_MOVE_STRIDE, heading);
            super::mac_move_one_step_with_heading(city, slot, record, KIND16_MOVE_STRIDE, heading);
        }
        // The native signed-byte dispatch skips unknown states, but the
        // common height/record tail still executes below.
        _ => {}
    }
    update_height(city, record);
}

fn tick_state_zero(
    city: &mut City,
    slot: usize,
    record: &mut Win95OverlayRecord,
    quickdraw: &mut MacToolboxRng,
    sounds: &mut Vec<PlacementSound>,
) {
    let follows_actor_reference = usize::from(record.saved_b) < 0xf1;
    let (mut target_x, mut target_y) = if follows_actor_reference {
        city.traffic_overlays()
            .and_then(|records| records.get(usize::from(record.saved_b)))
            .map(|target| (usize::from(target.tile_x), usize::from(target.tile_y)))
            .unwrap_or((usize::from(record.target_x), usize::from(record.target_y)))
    } else {
        (usize::from(record.target_x), usize::from(record.target_y))
    };

    // Native CODE 6 branches referenced targets directly to the heading
    // helper. Only the saved-coordinate path checks XTXT and scans for a
    // replacement marker.
    if !follows_actor_reference && city.text_overlay_at(target_x, target_y).unwrap_or(0) < 0xf1 {
        // CODE 6 scans X from origin-32 through origin+32, then Y through the
        // same interval. It stops at the first in-bounds XTXT marker >= 0xf1.
        record.state = 2;
        let origin_x = i16::from(record.tile_x);
        let origin_y = i16::from(record.tile_y);
        'scan: for x_delta in -KIND16_SEARCH_RADIUS..=KIND16_SEARCH_RADIUS {
            for y_delta in -KIND16_SEARCH_RADIUS..=KIND16_SEARCH_RADIUS {
                let x = origin_x + x_delta;
                let y = origin_y + y_delta;
                if !(0..CITY_WIDTH as i16).contains(&x) || !(0..CITY_HEIGHT as i16).contains(&y) {
                    continue;
                }
                if city.text_overlay_at(x as usize, y as usize).unwrap_or(0) >= 0xf1 {
                    target_x = x as usize;
                    target_y = y as usize;
                    record.target_x = target_x as u8;
                    record.target_y = target_y as u8;
                    record.state = 0;
                    break 'scan;
                }
            }
        }
        if record.state == 2 {
            return;
        }
    }

    let heading = crate::mac_traffic_geometry::mac_direction_to_tile(
        usize::from(record.tile_x),
        usize::from(record.tile_y),
        target_x,
        target_y,
    );
    record.lane = heading;
    let Some((candidate_x, candidate_y)) = candidate(record, heading) else {
        // A map-edge candidate follows the same shared-mover cleanup path as
        // any later carry beyond the map.
        super::mac_move_one_step_with_heading(city, slot, record, KIND16_MOVE_STRIDE, heading);
        return;
    };
    let marker = city.text_overlay_at(candidate_x, candidate_y).unwrap_or(0);

    if marker > 0xfa {
        // Reserved high marker cells use a one-bit QuickDraw gate. The set
        // path removes only the candidate marker before attempting movement.
        if quickdraw.random_u16() & 1 != 0 {
            city.set_text_overlay(candidate_x, candidate_y, 0);
            city.mark_render_dirty(candidate_x, candidate_y);
            super::mac_move_one_step_with_heading(city, slot, record, KIND16_MOVE_STRIDE, heading);
        }
        return;
    }

    if marker >= XTHG_PROXY_BASE {
        if u16::from(marker) == u16::from(XTHG_PROXY_BASE) + u16::from(record.saved_b) {
            if quickdraw.random_u16() & 3 == 0 {
                sounds.push(PlacementSound::Explode);
                let referenced_slot = usize::from(record.saved_b);
                if let Some(mut target) = city
                    .traffic_overlays()
                    .and_then(|records| records.get(referenced_slot))
                    .copied()
                {
                    super::mac_export121_clear_actor(city, referenced_slot, &mut target);
                }
                if let Some(created_slot) = create_kind6(city, candidate_x, candidate_y, record) {
                    record.state = 2;
                    // export_121 may have freed the current record itself.
                    // If export_119 then reuses that slot, A4 still aliases
                    // the new kind-6 record and the native state write lands
                    // on that newly allocated record.
                    if created_slot == slot {
                        if let Some(created) = city
                            .traffic_overlays()
                            .and_then(|records| records.get(slot))
                            .copied()
                        {
                            *record = created;
                            record.state = 2;
                        }
                    }
                } else {
                    record.state = 2;
                }
            } else {
                record.state = 1;
            }
        } else {
            record.state = 1;
        }
        return;
    }

    // XTXT below 0xc9 admits a movement step. The handler checks the next
    // candidate again after that step and may move a second time in this visit.
    super::mac_move_one_step_with_heading(city, slot, record, KIND16_MOVE_STRIDE, heading);
    if let Some((next_x, next_y)) = candidate(record, heading)
        && city.text_overlay_at(next_x, next_y).unwrap_or(0) < XTHG_PROXY_BASE
    {
        super::mac_move_one_step_with_heading(city, slot, record, KIND16_MOVE_STRIDE, heading);
    }
}

fn tick_state_one(
    city: &mut City,
    slot: usize,
    record: &mut Win95OverlayRecord,
    quickdraw: &mut MacToolboxRng,
) {
    if quickdraw.random_u16() & 3 == 0 {
        record.state = 0;
        return;
    }
    let heading = (quickdraw.random_u16() & 7) as u8;
    if candidate(record, heading)
        .is_some_and(|(x, y)| city.text_overlay_at(x, y).unwrap_or(0) < XTHG_PROXY_BASE)
    {
        super::mac_move_one_step_with_heading(city, slot, record, KIND16_MOVE_STRIDE, heading);
        record.lane = heading;
    }
}

fn candidate(record: &Win95OverlayRecord, heading: u8) -> Option<(usize, usize)> {
    let direction = usize::from(heading & 7);
    let x = i16::from(record.tile_x) + super::MAC_KIND1_MOVE_ROW_UNIT[direction];
    let y = i16::from(record.tile_y) + super::MAC_KIND1_MOVE_COL_UNIT[direction];
    ((0..CITY_WIDTH as i16).contains(&x) && (0..CITY_HEIGHT as i16).contains(&y))
        .then_some((x as usize, y as usize))
}

fn create_kind6(city: &mut City, x: usize, y: usize, source: &Win95OverlayRecord) -> Option<usize> {
    if city.text_overlay_at(x, y).unwrap_or(0) >= XTHG_PROXY_BASE {
        return None;
    }
    let slot = city.traffic_overlays().and_then(|records| {
        records
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .take(XTHG_RECORD_COUNT - XTHG_FIRST_USABLE_SLOT)
            .find_map(|(slot, record)| (record.kind == 0).then_some(slot))
    })?;
    let prior_marker = city.text_overlay_at(x, y).unwrap_or(0);
    let mut record = city.traffic_overlays()?.get(slot).copied()?;
    record.kind = XTHG_EXPLOSION;
    record.lane = 0;
    record.state = 0;
    record.tile_x = x as u8;
    record.tile_y = y as u8;
    record.height = source.height;
    record.sub_x = 8;
    record.sub_y = 8;
    // export_119 does not write +8/+9; preserve whatever remained in the
    // allocator's reused record, just as the native slot table does.
    record.saved_a = prior_marker;
    record.saved_b = 1;
    city.set_mac_object_slot_record(slot, record);
    city.set_text_overlay(x, y, XTHG_PROXY_BASE.wrapping_add(slot as u8));
    city.mark_render_dirty(x, y);
    Some(slot)
}

fn update_height(city: &City, record: &mut Win95OverlayRecord) {
    if usize::from(record.saved_b) < 0xf1 {
        if let Some(target) = city
            .traffic_overlays()
            .and_then(|records| records.get(usize::from(record.saved_b)))
        {
            record.height = target.height;
        }
        return;
    }
    let x = usize::from(record.tile_x);
    let y = usize::from(record.tile_y);
    let terrain = city.altm_at(x, y);
    let altitude = if city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.raw_bits() & 0x04 != 0)
    {
        terrain.water
    } else {
        terrain.land
    };
    record.height = altitude.wrapping_add(record.state);
}

#[cfg(test)]
mod tests;
