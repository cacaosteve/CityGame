//! Mac CODE 10 Hurricane and Flood starters and their shared marker operations.

use super::scenario_action::{
    FLOOD_ACTION, HURRICANE_ACTION, REPEATED_FLOOD_ACTION, begin_action, checked_tile, neighbor,
    scatter_attempts,
};
use super::*;

pub(super) const WIND_DURATION: usize = 0x10b0;
pub(super) const FLOOD_DURATION: usize = 0x10b4;

pub(super) fn start(city: &mut City, rng: &mut MacToolboxRng, now: &mut u32) -> u16 {
    // The reconstructed zero seed traps the original opener on its first
    // empty lane. Use the smallest nonzero seed for a fresh hurricane.
    let mut lfsr = MacAppLfsrRng::new(1);
    city.set_mac_pending_action(0);
    city.set_mac_state_misc_low_word_at(WIND_DURATION, 50);
    city.set_mac_state_misc_low_word_at(FLOOD_DURATION, 60);

    let rotation = city.saved_rotation();
    let descending = matches!(rotation, 0 | 3);
    let scan_x = matches!(rotation, 0 | 2);
    let (start, end, direction) = if descending {
        (127, 0, -1)
    } else {
        (0, 127, 1)
    };
    let tile = |lane: usize, position: i16| {
        let position = position as usize;
        if scan_x {
            (position, lane)
        } else {
            (lane, position)
        }
    };

    let mut spent = 0;
    while spent < 20 {
        spent += 1;
        let lane = usize::from(lfsr.low_seven_bits());
        let mut position = start;
        while (0..=127).contains(&position) {
            let (x, y) = tile(lane, position);
            if position != end && city.building_at(x, y) >= 0x0d {
                spent += u8::from(rotation == 1);
                damage(city, x, y, rng, rotation >= 2, now);
                break;
            }
            position += direction * lfsr.bounded(20) as i16;
        }
    }

    for _ in 0..if rotation >= 2 { 50 } else { 100 } {
        let lane = usize::from(lfsr.low_seven_bits());
        let mut position = start;
        while position != end {
            let (x, y) = tile(lane, position);
            if city.building_at(x, y) >= 6 {
                city.set_text_overlay(x, y, 0xfc);
                break;
            }
            position += direction;
        }
    }

    // The original branches on an incidental sound/FreeMem return byte.
    // Admit the simulation once the opener finishes, independent of audio.
    begin_action(city, HURRICANE_ACTION);
    lfsr.state()
}

pub(super) fn start_flood(city: &mut City, point: &mut (i16, i16)) -> bool {
    if !seed_flood(city, point) {
        return false;
    }
    begin_action(city, FLOOD_ACTION);
    true
}

/// CODE 10 FUN_3b800 captures the proposal point before any nested Flood
/// can move it. The loop compares its attempt limit as a signed WORD.
pub(super) fn start_repeated_flood(
    city: &mut City,
    point: &mut (i16, i16),
    rng: &mut MacToolboxRng,
) -> usize {
    let (origin_x, origin_y) = *point;
    let mut successes = 0;
    for _ in 0..scatter_attempts(city) {
        let x = origin_x.wrapping_add((rng.random_u16() % 32) as i16 - 16);
        let y = origin_y.wrapping_add((rng.random_u16() % 32) as i16 - 16);
        if checked_tile(x, y).is_some() {
            *point = (x, y);
            successes += usize::from(seed_flood(city, point));
        }
    }
    if successes != 0 {
        begin_action(city, REPEATED_FLOOD_ACTION);
    }
    successes
}

/// CODE 10 FUN_3b8a0 scans increasing square radii, then X and Y offsets.
/// Rank matching cells in that order to avoid revisiting the inner squares.
fn seed_flood(city: &mut City, point: &mut (i16, i16)) -> bool {
    let (origin_x, origin_y) = *point;
    let shore = (0..CITY_WIDTH)
        .flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)))
        .filter(|&(x, y)| matches!(city.terrain_at(x, y), 0x20..=0x2f))
        .map(|(x, y)| {
            (
                x,
                y,
                (x as i16).wrapping_sub(origin_x),
                (y as i16).wrapping_sub(origin_y),
            )
        })
        .filter(|&(_, _, dx, dy)| dx.unsigned_abs().max(dy.unsigned_abs()) <= 127)
        .min_by_key(|&(x, y, dx, dy)| (dx.unsigned_abs().max(dy.unsigned_abs()), x, y));

    let (x, y) = if let Some((x, y, dx, dy)) = shore {
        // The positive-side stamps read XBIT from the negative-side
        // neighbor. Keep those tests but bound both coordinates.
        for (gate, read_offset, write_offset) in [
            (dx > 0, (-1, 0), (-1, 0)),
            (dy > 0, (0, -1), (0, -1)),
            (dx < 127, (-1, 0), (1, 0)),
            (dy < 127, (0, -1), (0, 1)),
        ] {
            if gate
                && let Some((sx, sy)) = offset_city_tile(x, y, read_offset.0, read_offset.1)
                && city
                    .tile_bits_at(sx, sy)
                    .is_some_and(|bits| bits.raw_bits() & 4 == 0)
                && let Some((tx, ty)) = offset_city_tile(x, y, write_offset.0, write_offset.1)
            {
                city.set_text_overlay(tx, ty, 0xfc);
            }
        }
        (x, y)
    } else {
        let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
        let target = (0..200)
            .map(|_| {
                (
                    usize::from(lfsr.low_seven_bits()),
                    usize::from(lfsr.low_seven_bits()),
                )
            })
            .find(|&(x, y)| city.terrain_at(x, y) == 0);
        let Some((x, y)) = target else {
            return false;
        };
        city.set_text_overlay(x, y, 0xfc);
        (x, y)
    };
    *point = (x as i16, y as i16);
    city.set_mac_state_misc_low_word_at(FLOOD_DURATION, 60);
    true
}

pub(super) fn begin_pass(city: &mut City) -> i16 {
    let duration = city.mac_state_misc_u32_at(FLOOD_DURATION).unwrap_or(0) as i16;
    if duration <= 0 {
        return duration;
    }
    city.set_mac_state_misc_low_word_at(FLOOD_DURATION, (duration - 1) as u16);
    duration - 1
}

pub(super) fn visit(
    city: &mut City,
    x: usize,
    y: usize,
    duration: i16,
    rng: &mut MacToolboxRng,
    lfsr: &mut MacAppLfsrRng,
    now: &mut u32,
) {
    if duration == 0 && lfsr.bounded(2) != 0 {
        city.set_text_overlay(x, y, 0);
        return;
    }
    if duration < 52 && rng.random_u16() & 3 != 0 {
        return;
    }
    if duration < 30 && rng.random_u16() & 3 == 0 {
        if city.building_at(x, y) >= 0x70 {
            damage(city, x, y, rng, false, now);
        }
        city.set_text_overlay(x, y, 0);
    }
    if duration <= 0 {
        return;
    }

    let Some((nx, ny)) = neighbor(x, y, rng.random_u16() & 3) else {
        return;
    };
    if city.altitude_at(nx, ny) > city.altitude_at(x, y)
        || matches!(city.terrain_at(nx, ny), 0x10..=0x1f)
    {
        return;
    }
    match city.text_overlay_at(nx, ny).unwrap_or(0) {
        0 => {}
        label @ 1..=0x32 => city.mac_export_161_clear_xtxt_resources(label),
        0x33..=0xc8 => damage(city, nx, ny, rng, true, now),
        0xf1..=0xf9 => {
            city.set_building(nx, ny, lfsr.bounded(4) as u8 + 1);
            return;
        }
        _ => return,
    }
    city.set_text_overlay(nx, ny, 0xfc);
    city.set_traffic_density(nx, ny, 0);
}

pub(super) fn finish_pass(
    city: &mut City,
    duration: i16,
    rng: &mut MacToolboxRng,
    lfsr: &mut MacAppLfsrRng,
    now: &mut u32,
) -> (bool, bool) {
    if duration == 0 {
        return (false, false);
    }
    let flood_sound = rng.random_u16() & 7 == 0;
    let wind = city.mac_state_misc_u32_at(WIND_DURATION).unwrap_or(0) as u16;
    if wind == 0 {
        return (flood_sound, false);
    }
    let wind_sound = rng.random_u16() & 7 == 0;
    if lfsr.bounded(2) == 0 {
        let x = usize::from(lfsr.bounded(128));
        let y = usize::from(lfsr.bounded(128));
        if city.building_at(x, y) > 0x70 {
            damage(city, x, y, rng, true, now);
        }
    }
    city.set_mac_state_misc_low_word_at(WIND_DURATION, wind - 1);
    (flood_sound, wind_sound)
}

fn damage(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    render: bool,
    now: &mut u32,
) {
    // FUN_3dc90's elevated bypass precedes export 80. Hurricane passes a
    // separate zero fire-fill flag even when enabling render RNG.
    if !matches!(city.building_at(x, y), 0x3f..=0x42) {
        let waits = super::super::budget::mac_destroy_building_export80(city, x, y, rng, render);
        *now = now.wrapping_add(waits.wrapping_mul(MAC_EXPORT80_RENDER_WAIT_TICKS_PER_LOOP));
    }
}
