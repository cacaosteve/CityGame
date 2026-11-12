//! Mac CODE 10 disaster actions and their shared marker lifecycle.

use super::*;
use crate::win95_overlay::Win95OverlayRecord;

pub(super) const FIRE_ACTION: u16 = 1;
pub(super) const FLOOD_ACTION: u16 = 2;
pub(super) const TOXIC_SPILL_ACTION: u16 = 4;
pub(super) const ACTOR_IGNITION_ACTION: u16 = 5;
pub(super) const EARTHQUAKE_ACTION: u16 = 6;
pub(super) const TORNADO_ACTION: u16 = 7;
pub(in crate::app) const OAKLAND_ACTION: u16 = 12;
pub(in crate::app) const HOLLYWOOD_ACTION: u16 = 8;
pub(super) const REPEATED_FLOOD_ACTION: u16 = 14;
pub(super) const CLOUD_SCATTER_ACTION: u16 = 15;
pub(in crate::app) const HURRICANE_ACTION: u16 = 16;
pub(super) const MONSTER_IGNITION_ACTION: u16 = 17;
pub(super) const AIR_CRASH_ACTION: u16 = 18;
const ACTIVE_SELECTOR: usize = 0x1044;
const ACTIVE_FLAG: usize = 0x1048;
const ACTION_ACTOR: usize = 0x1040;
const CARDINAL: [(isize, isize); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];

/// Selectors whose Mac scenario-opening behavior is implemented in CityGame.
/// Native CODE 4 copies any `SCEN` resource selector into the action latch;
/// this narrower predicate is an implementation gate, not the game's format
/// or selector validity rule. Selector 11 remains unsupported until its
/// terrain-height worker is modeled.
pub(in crate::app) fn mac_scenario_action_supported(selector: u16) -> bool {
    matches!(
        selector,
        0 | HOLLYWOOD_ACTION | OAKLAND_ACTION | HURRICANE_ACTION
    )
}

pub(in crate::app) fn mac_scenario_action_active(city: &City) -> bool {
    matches!(
        city.mac_state_misc_u32_at(ACTIVE_SELECTOR).unwrap_or(0) as u16,
        FIRE_ACTION
            | FLOOD_ACTION
            | super::riot::RIOTERS_ACTION
            | super::riot::REPEATED_RIOTERS_ACTION
            | TOXIC_SPILL_ACTION
            | ACTOR_IGNITION_ACTION
            | MONSTER_IGNITION_ACTION
            | EARTHQUAKE_ACTION
            | TORNADO_ACTION
            | HOLLYWOOD_ACTION
            | OAKLAND_ACTION
            | REPEATED_FLOOD_ACTION
            | CLOUD_SCATTER_ACTION
            | HURRICANE_ACTION
            | AIR_CRASH_ACTION
            | super::facility_disaster::SCATTER_ACTION
            | super::facility_disaster::WALK_ACTION
    ) && city.mac_state_misc_u32_at(ACTIVE_FLAG).unwrap_or(0) as u8 != 0
}

/// The menu arm and CODE 10 FUN_3c0bc independently choose this origin.
pub(super) fn fire_origin(city: &City, rng: &mut MacToolboxRng) -> (i16, i16) {
    let (x, y) = city.mac_kind2_hotspot().unwrap_or((0, 0));
    let mut jitter =
        |anchor: u16| (anchor as i16).wrapping_add((rng.random_u16() % 40) as i16 - 20);
    (jitter(x), jitter(y))
}

pub(super) fn seed_fire(
    city: &mut City,
    point: &mut (i16, i16),
    rng: &mut MacToolboxRng,
    now: &mut u32,
) -> bool {
    let (x, y) = fire_origin(city, rng);
    let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    let target = disaster_spiral_candidates((isize::from(x), isize::from(y)), 0x3f)
        .find(|&(x, y)| {
            city.building_at(x, y) >= 0x70
                && mac_export247_candidate(city, x, y, true, &mut lfsr, rng, now)
        })
        .or_else(|| {
            (0..200).find_map(|_| {
                let x = usize::from(lfsr.low_seven_bits());
                let y = usize::from(lfsr.low_seven_bits());
                mac_export247_candidate(city, x, y, true, &mut lfsr, rng, now).then_some((x, y))
            })
        });
    let Some((x, y)) = target else {
        return false;
    };
    *point = (x as i16, y as i16);
    begin_action(city, FIRE_ACTION);
    true
}

/// CODE 10 FUN_3bdf2 stamps one cloud directly at the proposal point.
pub(super) fn seed_toxic_spill(city: &mut City, point: (i16, i16)) -> bool {
    let Some((x, y)) = checked_tile(point.0, point.1) else {
        return false;
    };
    city.set_text_overlay(x, y, 0xfb);
    begin_action(city, TOXIC_SPILL_ACTION);
    true
}

/// CODE 10 FUN_3bfb2 uses two CODE 13 export-207 results for the spawn.
pub(super) fn seed_air_crash(city: &mut City) -> Option<(i16, i16)> {
    // The reconstructed A5-2 state is a zero fixed point. Both coordinates
    // resolve to 32 on every iteration, so reject an occupied cell instead
    // of reproducing the original's unbounded retry.
    let (x, y) = (32, 32);
    if city.text_overlay_at(x, y).unwrap_or(0) != 0 {
        return None;
    }
    let (slot, free) = city
        .mac_object_slot_records_mut()
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, record)| record.kind == 0)?;
    // The constructor preserves +1, +8, +9 and +0xb in the recycled slot.
    let record = Win95OverlayRecord {
        kind: 1,
        state: 7,
        tile_x: x as u8,
        tile_y: y as u8,
        height: 16,
        sub_x: 8,
        sub_y: 8,
        saved_a: 0,
        ..*free
    };
    city.set_mac_object_slot_record(slot, record);
    city.set_text_overlay(x, y, 0xc9 + slot as u8);
    begin_action(city, AIR_CRASH_ACTION);
    Some((x as i16, y as i16))
}

/// CODE 10 compares the low WORD of the computed count as signed.
pub(super) fn scatter_attempts(city: &City) -> i16 {
    let growth = city
        .mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET)
        .unwrap_or(0);
    (growth / 10_000 + 5) as i16
}

/// CODE 10 FUN_3b74c scatters clouds without testing existing tile contents.
pub(super) fn seed_cloud_scatter(
    city: &mut City,
    (origin_x, origin_y): (i16, i16),
    rng: &mut MacToolboxRng,
) -> Option<(i16, i16)> {
    let mut last_stamp = None;
    for _ in 0..scatter_attempts(city) {
        let x = origin_x.wrapping_add((rng.random_u16() & 7) as i16 - 4);
        let y = origin_y.wrapping_add((rng.random_u16() & 7) as i16 - 4);
        if let Some((x, y)) = checked_tile(x, y) {
            city.set_text_overlay(x, y, 0xfb);
            last_stamp = Some((x as i16, y as i16));
        }
    }
    if last_stamp.is_some() {
        begin_action(city, CLOUD_SCATTER_ACTION);
    }
    last_stamp
}

/// CODE 10 selector 6 scans x first after the display-only shake.
pub(super) fn seed_earthquake(
    city: &mut City,
    point: (i16, i16),
    rng: &mut MacToolboxRng,
    now: &mut u32,
) {
    let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    for dx in -32..=32 {
        for dy in -32..=32 {
            // The gate draws even when the coordinate is outside the map.
            if rng.random_u16() & 0x3f != 0 {
                continue;
            }
            let x = i32::from(point.0) + dx;
            let y = i32::from(point.1) + dy;
            if !(0..CITY_WIDTH as i32).contains(&x) || !(0..CITY_HEIGHT as i32).contains(&y) {
                continue;
            }
            let (x, y) = (x as usize, y as usize);
            let building = city.building_at(x, y);
            if building < 0x0e {
                continue;
            }
            if rng.random_u16() & 3 == 0 {
                mac_export247_candidate(city, x, y, true, &mut lfsr, rng, now);
            } else if !matches!(building, 0x3f..=0x42) {
                // FUN_3dc90 calls export 80 even when its footprint-fill flag is zero.
                super::super::budget::mac_destroy_building_export80(city, x, y, rng, false);
            }
        }
    }
    // Even an empty scan enters the ordinary action pass once.
    begin_action(city, EARTHQUAKE_ACTION);
}

pub(super) fn seed_oakland(
    city: &mut City,
    point: &mut (i16, i16),
    rng: &mut MacToolboxRng,
    now: &mut u32,
) -> usize {
    let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    city.set_mac_pending_action(0);
    let count = disaster_spiral_candidates((isize::from(point.0), isize::from(point.1)), 0x7f)
        .filter(|&(x, y)| mac_export247_candidate(city, x, y, false, &mut lfsr, rng, now))
        .take(FIRESTORM_IGNITION_LIMIT)
        .inspect(|&(x, y)| *point = (x as i16, y as i16))
        .count();
    if count != 0 {
        begin_action(city, OAKLAND_ACTION);
    }
    count
}

/// CODE 10 selector 8 consumes the point from MENU 1004 or SCEN.
pub(super) fn seed_monster(city: &mut City, point: (i16, i16), rng: &mut MacToolboxRng) -> bool {
    city.set_mac_pending_action(0);
    if city.mac_kind5_actor_counter as i16 >= 1 {
        return false;
    }
    let Some((slot, x, y)) = prepare_disaster_actor_slot(city, point) else {
        return false;
    };
    let (tx, ty) = (usize::from(x), usize::from(y));
    let actor = Win95OverlayRecord {
        kind: 5,
        lane: 2,
        tile_x: x,
        tile_y: y,
        height: 15,
        sub_x: 8,
        sub_y: 8,
        target_x: (rng.random_u16() & 0x7f) as u8,
        target_y: (rng.random_u16() & 0x7f) as u8,
        saved_a: city.text_overlay_at(tx, ty).unwrap_or(0),
        saved_b: if city.mac_scenario_active() || rng.random_u16() & 1 != 0 {
            0
        } else {
            (rng.random_u16() % 3 + 1) as u8
        },
        ..Default::default()
    };
    city.set_mac_object_slot_record(slot, actor);
    city.mac_kind5_actor_counter = city.mac_kind5_actor_counter.wrapping_add(1);
    city.set_text_overlay(tx, ty, 0xc9 + slot as u8);
    city.set_mac_state_misc_u32_at(ACTION_ACTOR, slot as u32);
    begin_action(city, HOLLYWOOD_ACTION);
    true
}

/// CODE 10 FUN_3c570 allocates and publishes the actor, then invalidates
/// its redraw bounds via CODE 6 export 111. Movement waits for export 109.
pub(super) fn seed_tornado(city: &mut City, point: (i16, i16), rng: &mut MacToolboxRng) -> bool {
    if city.mac_tornado_actor_counter as i16 >= 1 {
        return false;
    }
    let Some((slot, x, y)) = prepare_disaster_actor_slot(city, point) else {
        return false;
    };
    let (tx, ty) = (usize::from(x), usize::from(y));
    let actor = Win95OverlayRecord {
        kind: XTHG_TORNADO,
        lane: (rng.random_u16() & 7) as u8,
        tile_x: x,
        tile_y: y,
        height: city.altitude_at(tx, ty) & 0x1f,
        sub_x: 8,
        sub_y: 8,
        target_x: (rng.random_u16() & 0x7f) as u8,
        target_y: (rng.random_u16() & 0x7f) as u8,
        saved_a: city.text_overlay_at(tx, ty).unwrap_or(0),
        // Export 113 finds an empty slot without clearing its payload.
        saved_b: city.mac_object_slot_records_mut()[slot].saved_b,
        ..Default::default()
    };
    city.set_mac_object_slot_record(slot, actor);
    city.mac_tornado_actor_counter = city.mac_tornado_actor_counter.wrapping_add(1);
    city.set_text_overlay(tx, ty, 0xc9 + slot as u8);
    city.set_mac_state_misc_u32_at(ACTION_ACTOR, slot as u32);
    begin_action(city, TORNADO_ACTION);
    true
}

fn prepare_disaster_actor_slot(city: &mut City, (x, y): (i16, i16)) -> Option<(usize, u8, u8)> {
    let slot = city
        .mac_object_slot_records_mut()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, actor)| (actor.kind == 0).then_some(slot))?;
    let (x, y) = (x.clamp(0, 127) as u8, y.clamp(0, 127) as u8);
    if let Some(proxy @ 0xc9..=0xf0) = city.text_overlay_at(usize::from(x), usize::from(y)) {
        let mut actor = proxy_actor(city, proxy);
        mac_export121_clear_actor(city, usize::from(proxy - 0xc9), &mut actor);
    }
    Some((slot, x, y))
}

pub(super) fn begin_action(city: &mut City, selector: u16) {
    city.set_mac_state_misc_u32_at(ACTIVE_SELECTOR, u32::from(selector));
    city.set_mac_state_misc_u32_at(ACTIVE_FLAG, 0xff);
    city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_DISASTER);
}

#[derive(Default)]
pub(super) struct ActionPass {
    pub complete: bool,
    pub riot_sound: bool,
    pub flood_sound: bool,
    pub wind_sound: bool,
}

pub(super) fn advance_action(
    city: &mut City,
    rng: &mut MacToolboxRng,
    now: &mut u32,
    hurricane_lfsr: &mut u16,
) -> ActionPass {
    if !mac_scenario_action_active(city) {
        return ActionPass::default();
    }
    let hurricane =
        city.mac_state_misc_u32_at(ACTIVE_SELECTOR).unwrap_or(0) as u16 == HURRICANE_ACTION;
    let mut lfsr = MacAppLfsrRng::new(if hurricane { *hurricane_lfsr } else { 0 });
    let duration = super::hurricane::begin_pass(city);
    let mut marker_active = false;
    let mut riot_seen = false;
    // CODE 10 reads live XTXT in X-major order. A newly stamped cell can
    // participate in this same pass when it lies ahead of the cursor.
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            match city.text_overlay_at(x, y) {
                Some(0xff) => {
                    marker_active = true;
                    fire_cell(city, x, y, rng, &mut lfsr, now);
                }
                Some(0xfc) => {
                    marker_active = true;
                    super::hurricane::visit(city, x, y, duration, rng, &mut lfsr, now);
                }
                Some(0xfb) => {
                    marker_active = true;
                    cloud_cell(city, x, y, rng, &mut lfsr);
                }
                Some(marker @ (0xfd | 0xfe)) => {
                    marker_active = true;
                    riot_seen = true;
                    super::riot::visit(city, (x, y), marker, rng, &mut lfsr, now);
                }
                Some(proxy @ 0xc9..=0xf0) => {
                    let kind = proxy_actor(city, proxy).kind;
                    if matches!(kind, 8 | 14) || (kind == 7 && lfsr.bounded(16) == 0) {
                        if let Some((nx, ny)) = neighbor(x, y, rng.random_u16() & 3) {
                            extinguish(city, nx, ny, rng, &mut lfsr);
                        }
                    }
                    if matches!(kind, 7 | 14) {
                        if let Some((nx, ny)) = neighbor(x, y, rng.random_u16() & 3) {
                            if matches!(city.text_overlay_at(nx, ny), Some(0xfd | 0xfe)) {
                                city.set_text_overlay(nx, ny, 0);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let riot_sound = riot_seen && rng.random_u16() & 7 == 0;
    let (flood_sound, wind_sound) =
        super::hurricane::finish_pass(city, duration, rng, &mut lfsr, now);
    if hurricane {
        *hurricane_lfsr = lfsr.state();
    }
    let actor_active = city.traffic_overlays().is_some_and(|records| {
        records
            .iter()
            .skip(1)
            .any(|actor| matches!(actor.kind, 5 | 6 | 15) || (actor.kind == 1 && actor.state == 7))
    });
    city.set_mac_state_misc_u32_at(
        ACTIVE_FLAG,
        if actor_active {
            0xff
        } else {
            u32::from(marker_active)
        },
    );
    let complete = !marker_active && !actor_active;
    if complete {
        city.mac_clear_dispatch_proxies();
        city.set_mac_state_misc_u32_at(ACTION_ACTOR, u32::MAX);
        city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        // CODE 10's completion tail opens the paper directly after switching
        // back to run state 1, independently of Extra and Subscription.
        city.mac_poll_for_newspaper(|| rng.random_u16());
    }
    ActionPass {
        complete,
        riot_sound,
        flood_sound,
        wind_sound,
    }
}

fn fire_cell(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    lfsr: &mut MacAppLfsrRng,
    now: &mut u32,
) {
    if rng.random_u16() & 3 != 0 {
        return;
    }
    if water(city, x, y) {
        city.set_text_overlay(x, y, 0);
        return;
    }
    match rng.random_u16() & 7 {
        dir @ 0..=3 => {
            if let Some((nx, ny)) = neighbor(x, y, dir) {
                mac_export247_candidate(city, nx, ny, true, lfsr, rng, now);
            }
        }
        5 => {
            let building = city.building_at(x, y);
            if building < 0x70 {
                return;
            }
            burn_footprint(city, x, y, rng, false);
            extinguish(city, x, y, rng, lfsr);
            if lfsr.bounded(16) == 0 {
                spawn_fire_actor(city, x, y);
            }
            if matches!(building, 0x85 | 0x9f | 0xbc) {
                if let Some((ax, ay, side)) =
                    super::super::budget::mac_export81_footprint(city, x, y, building)
                {
                    for dx in 0..side as i16 {
                        for dy in 0..side as i16 {
                            if let Some((tx, ty)) = checked_tile(ax + dx, ay - dy) {
                                paint_cloud(city, tx, ty);
                            }
                        }
                    }
                }
            }
        }
        _ => {
            let threshold = u16::from(city.fire_coverage_at(x, y).unwrap_or(0)) + 8;
            if rng.random_u16() & 0xff < threshold {
                burn_footprint(city, x, y, rng, false);
                extinguish(city, x, y, rng, lfsr);
            }
        }
    }
}

/// CODE 10 FUN_3dc90 footprint writes for the two burning call forms.
pub(super) fn burn_footprint(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    render: bool,
) -> u32 {
    let building = city.building_at(x, y);
    if matches!(building, 0x3f..=0x42) {
        return 0;
    }
    let footprint = super::super::budget::mac_export81_footprint(city, x, y, building);
    let waits = super::super::budget::mac_destroy_building_export80(city, x, y, rng, render);
    if let Some((ax, ay, side)) = footprint {
        for dx in 0..side as i16 {
            for dy in 0..side as i16 {
                if let Some((tx, ty)) = checked_tile(ax + dx, ay - dy) {
                    if !water(city, tx, ty) {
                        city.set_text_overlay(tx, ty, 0xff);
                    }
                }
            }
        }
    }
    waits
}

fn extinguish(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    lfsr: &mut MacAppLfsrRng,
) {
    if city.text_overlay_at(x, y) != Some(0xff) {
        return;
    }
    city.set_text_overlay(x, y, 0);
    let building = city.building_at(x, y);
    if matches!(building, 0x3f..=0x42) {
        return;
    }
    if building < 0x61
        || city
            .zone_at(x, y)
            .is_some_and(|zone| zone.raw_bits() & 0xf0 == 0xf0)
    {
        super::super::budget::mac_destroy_building_export80(city, x, y, rng, false);
        if building < 0x61 {
            city.set_building(x, y, lfsr.bounded(4) as u8 + 1);
        }
    }
}

fn cloud_cell(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    lfsr: &mut MacAppLfsrRng,
) {
    if rng.random_u16() & 1 != 0 {
        return;
    }
    if lfsr.bounded(64) == 0 || (water(city, x, y) && rng.random_u16() & 0xf == 0) {
        city.set_text_overlay(x, y, 0);
        return;
    }
    let building = city.building_at(x, y);
    // Both executables have the same per-building protection table.
    if super::super::pe_fb_zone_degradable(building) {
        if let Some((ax, ay, extent)) =
            super::super::budget::mac_export81_footprint(city, x, y, building)
        {
            let coefficient = if extent == 3 { 4 } else { extent as u16 };
            super::super::budget::mac_budget_place_growth_pattern(
                city,
                ax as usize,
                ay as usize,
                coefficient,
                4,
                rng,
            );
        }
    }
    let mut target = None;
    let mut height = city.altitude_at(x, y);
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        if let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) {
            let neighbor_height = city.altitude_at(nx, ny);
            if neighbor_height < height {
                height = neighbor_height;
                target = Some((nx, ny));
            }
        }
    }
    let target = target.or_else(|| {
        let (dx, dy) = [(0, -1), (1, 0), (0, 1), (-1, 0)][usize::from(rng.random_u16() & 3)];
        offset_city_tile(x, y, dx, dy)
    });
    city.set_text_overlay(x, y, 0);
    if let Some((nx, ny)) = target {
        paint_cloud(city, nx, ny);
    }
}

fn spawn_fire_actor(city: &mut City, x: usize, y: usize) {
    let marker = city.text_overlay_at(x, y).unwrap_or(0);
    if marker >= 0xc9 {
        return;
    }
    let Some(slot) = city
        .mac_object_slot_records_mut()
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(slot, record)| (record.kind == 0).then_some(slot))
    else {
        return;
    };
    city.set_mac_object_slot_record(
        slot,
        Win95OverlayRecord {
            kind: 6,
            tile_x: x as u8,
            tile_y: y as u8,
            sub_x: 8,
            sub_y: 8,
            saved_a: marker,
            saved_b: 1,
            ..Win95OverlayRecord::default()
        },
    );
    city.set_text_overlay(x, y, 0xc9 + slot as u8);
}

fn proxy_actor(city: &City, proxy: u8) -> Win95OverlayRecord {
    city.traffic_overlays()
        .and_then(|records| records.get(usize::from(proxy - 0xc9)))
        .copied()
        .unwrap_or_default()
}

fn water(city: &City, x: usize, y: usize) -> bool {
    city.tile_bits_at(x, y).is_some_and(TileBits::water_tile)
}

fn paint_cloud(city: &mut City, x: usize, y: usize) {
    if city.text_overlay_at(x, y).unwrap_or(0) < 0x33 {
        city.set_text_overlay(x, y, 0xfb);
    }
}

pub(super) fn neighbor(x: usize, y: usize, dir: u16) -> Option<(usize, usize)> {
    let (dx, dy) = CARDINAL[usize::from(dir)];
    offset_city_tile(x, y, dx, dy)
}

pub(super) fn checked_tile(x: i16, y: i16) -> Option<(usize, usize)> {
    (0..CITY_WIDTH as i16).contains(&x).then_some(())?;
    (0..CITY_HEIGHT as i16)
        .contains(&y)
        .then_some((x as usize, y as usize))
}
