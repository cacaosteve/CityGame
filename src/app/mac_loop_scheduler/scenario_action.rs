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

/// Incremental core of CODE 10 selector 11. The caller supplies the native
/// terrain validator/writer, inherited refresh counter and live sound query;
/// these are not yet available together in the normal application path.
#[allow(dead_code)]
struct Selector11Work {
    point: (i16, i16),
    temporary_funds: i32,
    refresh_counter: u32,
}

#[allow(dead_code)]
impl Selector11Work {
    fn new(point: (i16, i16), refresh_counter: u32) -> Self {
        Self {
            point,
            temporary_funds: 25_000,
            refresh_counter,
        }
    }

    /// Advance one sampled candidate, returning true when the native budget
    /// loop exits. Out-of-map candidates consume two draws and spend nothing.
    fn step(
        &mut self,
        city: &mut City,
        rng: &mut MacToolboxRng,
        mut validate_and_raise: impl FnMut(
            &mut City,
            usize,
            usize,
            &mut i32,
            &mut MacToolboxRng,
        ) -> bool,
        mut refresh: impl FnMut(&mut City, &mut MacToolboxRng),
        mut sound_query: impl FnMut(&mut MacToolboxRng) -> bool,
        mut sound: impl FnMut(u16, &mut MacToolboxRng),
    ) -> bool {
        if self.temporary_funds <= 0 {
            return true;
        }
        let x = self.point.0.wrapping_add((rng.random_u16() % 5) as i16 - 2);
        let y = self.point.1.wrapping_add((rng.random_u16() % 5) as i16 - 2);
        let Some((x, y)) = checked_tile(x, y) else {
            return false;
        };
        city.set_text_overlay(x, y, if rng.random_bit() { 0xff } else { 0xfb });
        if validate_and_raise(city, x, y, &mut self.temporary_funds, rng) {
            self.refresh_counter = self.refresh_counter.wrapping_add(1);
            if self.refresh_counter & 3 == 0 {
                refresh(city, rng);
            }
        } else {
            self.temporary_funds = self.temporary_funds.wrapping_sub(1_000);
        }
        let x = self
            .point
            .0
            .wrapping_add((rng.random_u16() % 32) as i16 - 16);
        let y = self
            .point
            .1
            .wrapping_add((rng.random_u16() % 32) as i16 - 16);
        if let Some((x, y)) = checked_tile(x, y) {
            let wet = city.tile_bits_at(x, y).is_some_and(TileBits::water_tile);
            city.set_text_overlay(x, y, if wet { 0xfb } else { 0xff });
        }
        if sound_query(rng) {
            let id = if rng.random_u16() & 3 == 0 { 504 } else { 507 };
            sound(id, rng);
        }
        self.temporary_funds <= 0
    }
}

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
    lfsr: &mut MacAppLfsrRng,
) -> bool {
    let (x, y) = fire_origin(city, rng);
    let target = disaster_spiral_candidates((isize::from(x), isize::from(y)), 0x3f)
        .find(|&(x, y)| {
            city.building_at(x, y) >= 0x70
                && mac_export247_candidate(city, x, y, true, lfsr, rng, now)
        })
        .or_else(|| {
            (0..200).find_map(|_| {
                let x = usize::from(lfsr.low_seven_bits());
                let y = usize::from(lfsr.low_seven_bits());
                mac_export247_candidate(city, x, y, true, lfsr, rng, now).then_some((x, y))
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
pub(super) fn seed_air_crash(city: &mut City, lfsr: &mut MacAppLfsrRng) -> Option<(i16, i16)> {
    let initial_state = lfsr.state();
    let (x, y) = loop {
        let x = usize::from(lfsr.low_six_bits()) + 32;
        let y = usize::from(lfsr.low_six_bits()) + 32;
        if city.text_overlay_at(x, y).unwrap_or(0) == 0 {
            break (x, y);
        }
        // The original retries indefinitely if this stream cycles over
        // occupied cells. Stop after one cycle to keep the app responsive.
        if lfsr.state() == initial_state {
            return None;
        }
    };
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

/// CODE 10 selector 6 scans x first after its 24-frame shake and Explode requests.
pub(super) fn seed_earthquake(
    city: &mut City,
    point: (i16, i16),
    rng: &mut MacToolboxRng,
    now: &mut u32,
    lfsr: &mut MacAppLfsrRng,
) {
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
                mac_export247_candidate(city, x, y, true, lfsr, rng, now);
            } else if !matches!(building, 0x3f..=0x42) {
                // FUN_3dc90 records the affected class even when footprint fill is off.
                city.record_mac_disaster_damage_class_at(x, y);
                super::super::budget::mac_destroy_building_export80_for_earthquake(city, x, y, rng);
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
    lfsr: &mut MacAppLfsrRng,
) -> usize {
    city.set_mac_pending_action(0);
    let count = disaster_spiral_candidates((isize::from(point.0), isize::from(point.1)), 0x7f)
        .filter(|&(x, y)| mac_export247_candidate(city, x, y, false, lfsr, rng, now))
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

#[cfg(test)]
pub(super) fn advance_action(
    city: &mut City,
    rng: &mut MacToolboxRng,
    now: &mut u32,
    lfsr: &mut MacAppLfsrRng,
) -> ActionPass {
    let pass = advance_action_with_redraw(city, rng, now, lfsr, None, None, &mut |_| {});
    if pass.complete {
        city.mac_poll_for_newspaper(|| rng.random_u16());
    }
    pass
}

pub(super) fn advance_action_with_redraw(
    city: &mut City,
    rng: &mut MacToolboxRng,
    now: &mut u32,
    lfsr: &mut MacAppLfsrRng,
    view: Option<super::map_redraw::MacMapRedrawView>,
    dirty: Option<super::map_redraw::MacMapClip>,
    palette: &mut impl FnMut(usize),
) -> ActionPass {
    if !mac_scenario_action_active(city) {
        return ActionPass::default();
    }
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
                    fire_cell(city, x, y, rng, lfsr, now);
                }
                Some(0xfc) => {
                    marker_active = true;
                    super::hurricane::visit(city, x, y, duration, rng, lfsr, now);
                }
                Some(0xfb) => {
                    marker_active = true;
                    cloud_cell(city, x, y, rng, lfsr);
                }
                Some(marker @ (0xfd | 0xfe)) => {
                    marker_active = true;
                    riot_seen = true;
                    super::riot::visit(city, (x, y), marker, rng, lfsr, now);
                }
                Some(proxy @ 0xc9..=0xf0) => {
                    let kind = proxy_actor(city, proxy).kind;
                    if matches!(kind, 8 | 14) || (kind == 7 && lfsr.bounded(16) == 0) {
                        if let Some((nx, ny)) = neighbor(x, y, rng.random_u16() & 3) {
                            extinguish(city, nx, ny, rng, lfsr);
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
    let (flood_sound, wind_sound) = super::hurricane::finish_pass(city, duration, rng, lfsr, now);
    if let (Some(view), Some(dirty)) = (view, dirty) {
        view.redraw_dirty_with_palette(city, rng, dirty, palette)
            .apply(city, lfsr);
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
    // CODE 10 export 246 redraws after the marker/actor scans and before
    // the completion tail can open the newspaper.
    if let Some(view) = view {
        view.redraw_markers_with_palette(city, rng, palette)
            .apply(city, lfsr);
    }
    let complete = !marker_active && !actor_active;
    if complete {
        city.mac_clear_dispatch_proxies();
        city.set_mac_state_misc_u32_at(ACTION_ACTOR, u32::MAX);
        city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
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
    city.record_mac_disaster_damage_class_at(x, y);
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
    // CODE 6 export 119 leaves bytes +8/+9 untouched when reusing a
    // free slot. They can retain data from an earlier actor.
    let mut record = city.mac_object_slot_records_mut()[slot];
    record.kind = 6;
    record.lane = 0;
    record.state = 0;
    record.tile_x = x as u8;
    record.tile_y = y as u8;
    record.height = 0;
    record.sub_x = 8;
    record.sub_y = 8;
    record.saved_a = marker;
    record.saved_b = 1;
    city.set_mac_object_slot_record(slot, record);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires private original-Mac selector 11 fixtures"]
    fn selector11_rejected_terrain_matches_native_marker_and_sound_paths() {
        let root =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_FIXTURE").unwrap());
        for (name, guard) in [("reject", false), ("reject_sound", true)] {
            let row: serde_json::Value = serde_json::from_slice(
                &std::fs::read(root.join(format!("selector11-{name}.json"))).unwrap(),
            )
            .unwrap();
            let expected: Vec<u8> = row["markers"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let mut city = City::new_blank("/tmp/mac-selector11.sc2", "Selector 11").unwrap();
            let mut rng = MacToolboxRng::new(1);
            let mut work = Selector11Work::new((64, 64), 0);
            let mut sounds = Vec::new();
            let mut count = 0;
            loop {
                count += 1;
                if work.step(
                    &mut city,
                    &mut rng,
                    |_, _, _, _, _| false,
                    |_, _| panic!("rejected validation cannot refresh"),
                    |_| guard,
                    |id, _| sounds.push(id),
                ) {
                    break;
                }
                assert!(count < 100);
            }
            assert_eq!(count, row["validations"].as_u64().unwrap() as usize);
            assert_eq!(rng.seed(), row["seed"].as_u64().unwrap() as u32);
            assert_eq!(
                sounds,
                row["sounds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u16)
                    .collect::<Vec<_>>()
            );
            for x in 0..128 {
                for y in 0..128 {
                    assert_eq!(
                        city.text_overlay_at(x, y).unwrap(),
                        expected[x * 128 + y],
                        "{name} {x},{y}"
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "requires private original-Mac selector 11 fixtures"]
    fn selector11_terrain_loop_matches_original_flat_and_mixed_workers() {
        fn bytes(s: &str) -> Vec<u8> {
            s.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        }
        let root =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_FIXTURE").unwrap());
        for name in ["flat", "mixed"] {
            let row: serde_json::Value = serde_json::from_slice(
                &std::fs::read(root.join(format!("selector11-{name}.json"))).unwrap(),
            )
            .unwrap();
            let mut city = City::new_blank("/tmp/mac-selector11-city", "Selector 11").unwrap();
            city.kind = crate::city::CityFileKind::MacCity;
            for x in 0..128 {
                for y in 0..128 {
                    city.set_altm_word(x, y, 0x0145);
                    city.set_zone(x, y, 0xa2);
                }
            }
            if name == "mixed" {
                city.set_zone(65, 67, 0xa7);
                city.set_underground(64, 66, 2);
            }
            let mut rng = MacToolboxRng::new(1);
            let mut work = Selector11Work::new((64, 64), 0);
            let mut count = 0;
            loop {
                count += 1;
                if work.step(
                    &mut city,
                    &mut rng,
                    |city, x, y, funds, rng| {
                        if !super::super::super::mac_terrain_editing::mac_mark_terrain_raise(
                            city, x, y,
                        ) {
                            return false;
                        }
                        super::super::super::mac_terrain_editing::mac_apply_marked_terrain_raise(
                            city, x, y, funds, rng,
                        );
                        true
                    },
                    |_, _| {},
                    |_| false,
                    |_, _| unreachable!(),
                ) {
                    break;
                }
                assert!(count < 500);
            }
            assert_eq!(
                count,
                row["validations"].as_u64().unwrap() as usize,
                "{name}"
            );
            assert_eq!(rng.seed(), row["seed"].as_u64().unwrap() as u32, "{name}");
            let altm = bytes(row["altm"].as_str().unwrap());
            let expected: Vec<_> = [
                "terrain",
                "bits",
                "buildings",
                "zones",
                "markers",
                "underground",
            ]
            .map(|key| bytes(row[key].as_str().unwrap()))
            .into_iter()
            .collect();
            for x in 0..128 {
                for y in 0..128 {
                    let i = x * 128 + y;
                    let sample = city.altm_at(x, y);
                    let actual = u16::from(sample.land)
                        | u16::from(sample.water) << 5
                        | u16::from(sample.tunnel_levels) << 10;
                    assert_eq!(
                        actual,
                        u16::from_be_bytes([altm[2 * i], altm[2 * i + 1]]),
                        "{name} ALTM {x},{y}"
                    );
                    let actual = [
                        city.terrain_at(x, y),
                        city.tile_bits_at(x, y).unwrap().raw_bits(),
                        city.building_at(x, y),
                        city.zone_at(x, y).unwrap().raw_bits(),
                        city.text_overlay_at(x, y).unwrap(),
                        city.underground_at(x, y),
                    ];
                    for (plane, value) in actual.into_iter().enumerate() {
                        assert_eq!(value, expected[plane][i], "{name} plane {plane} {x},{y}");
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "requires private original-Mac capture and shape data; runtime integration incomplete"]
    fn selector11_first_accepted_refresh_matches_native_boundary() {
        fn bytes(s: &str) -> Vec<u8> {
            s.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        }
        let root =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let row: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("selector11-first-refresh.json")).unwrap(),
        )
        .unwrap();
        let before = &row["before"];
        let names = [
            "altm",
            "terrain",
            "buildings",
            "zones",
            "underground",
            "markers",
            "bits",
        ];
        let planes: Vec<_> = names
            .map(|name| bytes(before[name].as_str().unwrap()))
            .into_iter()
            .collect();
        let mut city = City::load(root.join("Selector11.city")).unwrap();
        for x in 0..128 {
            for y in 0..128 {
                let i = x * 128 + y;
                let word = u16::from_be_bytes([planes[0][2 * i], planes[0][2 * i + 1]]);
                let altm = city.altm_at(x, y);
                let old = u16::from(altm.land)
                    | u16::from(altm.water) << 5
                    | u16::from(altm.tunnel_levels) << 10;
                if old != word {
                    city.set_altm_word(x, y, word);
                }
                if city.terrain_at(x, y) != planes[1][i] {
                    city.set_terrain(x, y, planes[1][i]);
                }
                if city.building_at(x, y) != planes[2][i] {
                    city.set_building(x, y, planes[2][i]);
                }
                if city.zone_at(x, y).unwrap().raw_bits() != planes[3][i] {
                    city.set_zone(x, y, planes[3][i]);
                }
                if city.underground_at(x, y) != planes[4][i] {
                    city.set_underground(x, y, planes[4][i]);
                }
                if city.text_overlay_at(x, y).unwrap() != planes[5][i] {
                    city.set_text_overlay(x, y, planes[5][i]);
                }
                if city.tile_bits_at(x, y).unwrap().raw_bits() != planes[6][i] {
                    city.set_tile_bits(x, y, TileBits::new(planes[6][i]));
                }
            }
        }
        let point = (
            before["point"][0].as_u64().unwrap() as i16,
            before["point"][1].as_u64().unwrap() as i16,
        );
        let mut work = Selector11Work::new(point, row["refresh_counter"].as_u64().unwrap() as u32);
        let mut rng = MacToolboxRng::new(row["seed_before"].as_u64().unwrap() as u32);
        let viewport = &row["view"];
        let mut view = super::super::map_redraw::MacMapRedrawView::from_native_snapshot(
            viewport["width"].as_i64().unwrap() as i32,
            viewport["height"].as_i64().unwrap() as i32,
            viewport["origin_x"].as_i64().unwrap() as i32,
            viewport["origin_y"].as_i64().unwrap() as i32,
            viewport["zoom"].as_u64().unwrap() as u32,
            viewport["rotation"].as_u64().unwrap() as u8,
        );
        if row["auto_goto"].as_bool().unwrap() {
            view.center_on(&city, point.0 as usize, point.1 as usize);
        }
        let expected_view = super::super::map_redraw::MacMapRedrawView::from_native_snapshot(
            viewport["width"].as_i64().unwrap() as i32,
            viewport["height"].as_i64().unwrap() as i32,
            row["mid_origin"][0].as_i64().unwrap() as i32,
            row["mid_origin"][1].as_i64().unwrap() as i32,
            viewport["zoom"].as_u64().unwrap() as u32,
            viewport["rotation"].as_u64().unwrap() as u8,
        );
        assert_eq!(view, expected_view);
        // Diagnostic isolation only: supply the original worker's preexisting
        // dirty rectangles. Normal gameplay must generate these itself.
        let native_dirty = std::env::var_os("CITYGAME_MAC_SELECTOR11_NATIVE_DIRTY").map(|_| {
            let native: serde_json::Value =
                serde_json::from_slice(&std::fs::read(root.join("redraw-worker.json")).unwrap())
                    .unwrap();
            native["destroy_entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| {
                    let rect = &entry[3];
                    super::super::map_redraw::MacMapClip::from_native_snapshot(
                        rect[0].as_i64().unwrap() as i32,
                        rect[1].as_i64().unwrap() as i32,
                        rect[2].as_i64().unwrap() as i32,
                        rect[3].as_i64().unwrap() as i32,
                    )
                })
                .collect::<Vec<_>>()
        });
        // The bounds fixture supplies sprite dimensions, not dirty rectangles.
        let shape_data = std::env::var_os("CITYGAME_MAC_SELECTOR11_SHAPE_DATA").map(|_| {
            let fixture: serde_json::Value = serde_json::from_slice(
                &std::fs::read(root.join("tile-bounds-worker.json")).unwrap(),
            )
            .unwrap();
            fixture["tile_bounds"].as_array().unwrap().clone()
        });
        let asset_shapes = std::env::var_os("CITYGAME_MAC_SHAPE_PACK").map(|pack| {
            let mut shapes =
                crate::mac_shape_table::MacShapeDimensions::from_pack(std::path::Path::new(&pack))
                    .unwrap();
            if let Some(resources) = std::env::var_os("CITYGAME_MAC_RESOURCES") {
                shapes.set_terrain_shapes(
                    crate::mac_import::terrain_shape_map_from_resources(std::path::Path::new(
                        &resources,
                    ))
                    .unwrap()
                    .unwrap(),
                );
            }
            shapes
        });
        let pending_dirty = std::cell::RefCell::new(None::<super::super::map_redraw::MacMapClip>);
        let mut destruction_index = 0;
        let mut refresh_seen = false;
        let finished = work.step(&mut city, &mut rng,
            |city, x, y, funds, rng| {
                if !super::super::super::mac_terrain_editing::mac_mark_terrain_raise(city, x, y) { return false; }
                super::super::super::mac_terrain_editing::mac_apply_marked_terrain_raise_with_callbacks(city, x, y, funds, rng,
                    &mut |city, x, y, rng| {
                        let origin = super::super::super::budget::mac_export81_footprint(city, x, y, city.building_at(x, y))
                            .and_then(|(x, y, _)| Some((usize::try_from(x).ok()?, usize::try_from(y).ok()?)))
                            .filter(|&(x, y)| x < CITY_WIDTH && y < CITY_HEIGHT)
                            .map(|(ax, ay)| (ax, ay, if city.tile_bits_at(x, y).is_some_and(TileBits::water_tile) { city.water_height_at(ax, ay) } else { city.altitude_at(ax, ay) }));
                        let mut dirty = native_dirty.as_ref().map(|rects| rects[destruction_index])
                            .or_else(|| pending_dirty.borrow_mut().take());
                        destruction_index += 1;
                        let waits = super::super::super::budget::mac_destroy_building_export80_with_animation(city, x, y, rng,
                            &mut |frame, column, row, variant| {
                                if let Some((ax, ay, height)) = origin {
                                    let bounds = view.destruction_frame_bounds(ax, ay, height, frame, column, row, variant);
                                    dirty = Some(dirty.map_or(bounds, |old| old.union(bounds)));
                                }
                            });
                        if let Some(dirty) = dirty { view.redraw_dirty_with_palette(city, rng, dirty, &mut |_| {}); }
                        waits
                    }, &mut |city, x, y| {
                        let terrain = city.terrain_at(x, y);
                        let building = city.building_at(x, y);
                        let bounds = if let Some(shapes) = &asset_shapes {
                            view.tile_dirty_bounds_from_assets(city, x, y, shapes)
                        } else if let Some(rows) = &shape_data {
                            let item = rows.iter().find(|item| item["terrain"].as_u64() == Some(u64::from(terrain))
                                && item["building"].as_u64() == Some(u64::from(building))).expect("fixture sprite dimensions");
                            view.surface_tile_dirty_bounds(city, x, y, item["terrain_height"].as_i64().unwrap() as i32,
                             Some((item["building_height"].as_i64().unwrap() as i32, item["building_width"].as_i64().unwrap() as i32)))
                        } else { return; };
                        if let Some(bounds) = bounds {
                            let mut pending = pending_dirty.borrow_mut();
                            *pending = Some(pending.map_or(bounds, |old| old.union(bounds)));
                        }
                    });
                true
            }, |city, rng| {
                refresh_seen = true;
                assert_eq!(rng.seed(), row["refresh_seed"].as_u64().unwrap() as u32);
                view.redraw_markers(city, rng);
                assert_eq!(rng.seed(), row["after_refresh_seed"].as_u64().unwrap() as u32);
            }, |_| false, |_, _| unreachable!());
        assert!(refresh_seen && !finished);
        assert_eq!(destruction_index, 4);
        assert_eq!(
            work.temporary_funds,
            row["refresh_funds"].as_i64().unwrap() as i32
        );
    }

    #[test]
    #[ignore = "requires private original-Mac live selector 11 capture"]
    fn selector11_matches_live_scenario_worker_with_captured_sound_queries() {
        fn bytes(s: &str) -> Vec<u8> {
            s.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        }
        let root =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_LIVE").unwrap());
        let row: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("selector11-live-reference.json")).unwrap(),
        )
        .unwrap();
        let before = &row["before"];
        let names = [
            "altm",
            "terrain",
            "buildings",
            "zones",
            "underground",
            "markers",
            "bits",
        ];
        let input: Vec<_> = names
            .map(|name| bytes(before[name].as_str().unwrap()))
            .into_iter()
            .collect();
        let expected: Vec<_> = names
            .map(|name| bytes(row["after"][name].as_str().unwrap()))
            .into_iter()
            .collect();
        let mut city = City::load(root.join("Selector11.city")).unwrap();
        for x in 0..128 {
            for y in 0..128 {
                let i = x * 128 + y;
                let word = u16::from_be_bytes([input[0][2 * i], input[0][2 * i + 1]]);
                let a = city.altm_at(x, y);
                let old =
                    u16::from(a.land) | u16::from(a.water) << 5 | u16::from(a.tunnel_levels) << 10;
                if old != word {
                    city.set_altm_word(x, y, word);
                }
                if city.terrain_at(x, y) != input[1][i] {
                    city.set_terrain(x, y, input[1][i]);
                }
                if city.building_at(x, y) != input[2][i] {
                    city.set_building(x, y, input[2][i]);
                }
                if city.zone_at(x, y).unwrap().raw_bits() != input[3][i] {
                    city.set_zone(x, y, input[3][i]);
                }
                if city.underground_at(x, y) != input[4][i] {
                    city.set_underground(x, y, input[4][i]);
                }
                if city.text_overlay_at(x, y).unwrap() != input[5][i] {
                    city.set_text_overlay(x, y, input[5][i]);
                }
                if city.tile_bits_at(x, y).unwrap().raw_bits() != input[6][i] {
                    city.set_tile_bits(x, y, TileBits::new(input[6][i]));
                }
            }
        }
        let point = (
            before["point"][0].as_u64().unwrap() as i16,
            before["point"][1].as_u64().unwrap() as i16,
        );
        let mut rng = MacToolboxRng::new(row["seed_before"].as_u64().unwrap() as u32);
        let mut work = Selector11Work::new(point, row["refresh_counter"].as_u64().unwrap() as u32);
        let mut queries = row["sound_queries"].as_array().unwrap().iter();
        let mut sounds = Vec::new();
        let mut steps = 0;
        loop {
            steps += 1;
            if work.step(
                &mut city,
                &mut rng,
                |city, x, y, funds, rng| {
                    if !super::super::super::mac_terrain_editing::mac_mark_terrain_raise(city, x, y)
                    {
                        return false;
                    }
                    super::super::super::mac_terrain_editing::mac_apply_marked_terrain_raise(
                        city, x, y, funds, rng,
                    );
                    true
                },
                |_, _| panic!("live rejected fixture does not refresh"),
                |_| {
                    queries
                        .next()
                        .expect("unexpected sound query")
                        .as_u64()
                        .unwrap()
                        != 0
                },
                |id, _| sounds.push(id),
            ) {
                break;
            }
            assert!(steps < 100);
        }
        assert_eq!(steps, 25);
        assert!(queries.next().is_none());
        assert_eq!(rng.seed(), row["seed_after"].as_u64().unwrap() as u32);
        assert_eq!(
            sounds,
            row["sound_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u16)
                .collect::<Vec<_>>()
        );
        for x in 0..128 {
            for y in 0..128 {
                let i = x * 128 + y;
                let a = city.altm_at(x, y);
                let word =
                    u16::from(a.land) | u16::from(a.water) << 5 | u16::from(a.tunnel_levels) << 10;
                assert_eq!(
                    word,
                    u16::from_be_bytes([expected[0][2 * i], expected[0][2 * i + 1]]),
                    "ALTM {x},{y}"
                );
                let values = [
                    city.terrain_at(x, y),
                    city.building_at(x, y),
                    city.zone_at(x, y).unwrap().raw_bits(),
                    city.underground_at(x, y),
                    city.text_overlay_at(x, y).unwrap(),
                    city.tile_bits_at(x, y).unwrap().raw_bits(),
                ];
                for (plane, value) in values.into_iter().enumerate() {
                    assert_eq!(
                        value,
                        expected[plane + 1][i],
                        "{} {x},{y}",
                        names[plane + 1]
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "requires private original-Mac live selector 11 capture"]
    fn selector11_live_worker_matches_with_modeled_sound_admission() {
        use crate::mac_sound_state::{
            MacManagedSoundRecord, MacSoundRecord, MacSoundVoice, mac_reserve_sound_record,
            mac_sound_records_are_idle, mac_start_sound_voice,
        };
        use std::cell::RefCell;
        fn bytes(s: &str) -> Vec<u8> {
            s.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        }
        let root =
            std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_LIVE").unwrap());
        let row: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("selector11-live-reference.json")).unwrap(),
        )
        .unwrap();
        let before = &row["before"];
        let zones = bytes(before["zones"].as_str().unwrap());
        let bits = bytes(before["bits"].as_str().unwrap());
        let markers = bytes(before["markers"].as_str().unwrap());
        let expected = bytes(row["after"]["markers"].as_str().unwrap());
        let mut city = City::load(root.join("Selector11.city")).unwrap();
        for x in 0..128 {
            for y in 0..128 {
                let i = x * 128 + y;
                if city.zone_at(x, y).unwrap().raw_bits() != zones[i] {
                    city.set_zone(x, y, zones[i]);
                }
                if city.tile_bits_at(x, y).unwrap().raw_bits() != bits[i] {
                    city.set_tile_bits(x, y, TileBits::new(bits[i]));
                }
                if city.text_overlay_at(x, y).unwrap() != markers[i] {
                    city.set_text_overlay(x, y, markers[i]);
                }
            }
        }
        // The live entry snapshot has two empty CODE 9 records and two
        // inactive MDRV slots. The original sound-507 request admits one
        // descriptor into slot 6; driver retirement is not exercised here.
        let audio = RefCell::new((
            vec![MacManagedSoundRecord::default(); 2],
            vec![
                MacSoundVoice {
                    descriptor: 505,
                    state: -1,
                },
                MacSoundVoice {
                    descriptor: 0,
                    state: -1,
                },
            ],
        ));
        let point = (
            before["point"][0].as_u64().unwrap() as i16,
            before["point"][1].as_u64().unwrap() as i16,
        );
        let mut work = Selector11Work::new(point, row["refresh_counter"].as_u64().unwrap() as u32);
        let mut rng = MacToolboxRng::new(row["seed_before"].as_u64().unwrap() as u32);
        let mut results = Vec::new();
        let mut sounds = Vec::new();
        for _ in 0..100 {
            let finished = work.step(
                &mut city,
                &mut rng,
                |city, x, y, funds, rng| {
                    if !super::super::super::mac_terrain_editing::mac_mark_terrain_raise(city, x, y)
                    {
                        return false;
                    }
                    super::super::super::mac_terrain_editing::mac_apply_marked_terrain_raise(
                        city, x, y, funds, rng,
                    );
                    true
                },
                |_, _| panic!("live rejected fixture does not refresh"),
                |_| {
                    let state = audio.borrow();
                    let records: Vec<_> = state
                        .0
                        .iter()
                        .map(|r| MacSoundRecord {
                            active: r.active,
                            descriptor: r.descriptor,
                        })
                        .collect();
                    let result =
                        mac_sound_records_are_idle(false, false, 1, &records, &state.1, |_| {});
                    results.push(result);
                    result
                },
                |id, _| {
                    sounds.push(id);
                    let mut state = audio.borrow_mut();
                    let (records, voices) = &mut *state;
                    let slot = mac_reserve_sound_record(records, voices, false, 1, 2002, |_, _| {})
                        .unwrap();
                    assert_eq!(slot, 0);
                    records[slot] = MacManagedSoundRecord {
                        active: true,
                        descriptor: id,
                        rank: 2002,
                    };
                    assert_eq!(mac_start_sound_voice(&mut state.1, id, true), Some(0));
                },
            );
            if finished {
                break;
            }
        }
        assert_eq!(
            results,
            row["sound_queries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() != 0)
                .collect::<Vec<_>>()
        );
        assert_eq!(sounds, [507]);
        assert_eq!(rng.seed(), row["seed_after"].as_u64().unwrap() as u32);
        for x in 0..128 {
            for y in 0..128 {
                assert_eq!(
                    city.text_overlay_at(x, y).unwrap(),
                    expected[x * 128 + y],
                    "XTXT {x},{y}"
                );
            }
        }
    }

    #[test]
    fn fire_actor_constructor_preserves_reused_slot_target_bytes() {
        let mut city = City::new_blank("/tmp/mac-fire-slot.sc2", "Fire slot").unwrap();
        city.set_mac_object_slot_record(
            1,
            Win95OverlayRecord {
                kind: 3,
                ..Default::default()
            },
        );
        city.set_mac_object_slot_record(
            2,
            Win95OverlayRecord {
                lane: 3,
                state: 64,
                tile_x: 127,
                tile_y: 255,
                height: 2,
                sub_x: 109,
                sub_y: 99,
                target_x: 0x5a,
                target_y: 1,
                saved_a: 2,
                saved_b: 3,
                ..Default::default()
            },
        );
        city.set_text_overlay(52, 62, 0);
        spawn_fire_actor(&mut city, 52, 62);
        let actor = city.traffic_overlays().unwrap()[2];
        assert_eq!(actor.to_bytes(), [6, 0, 0, 52, 62, 0, 8, 8, 0x5a, 1, 0, 1]);
        assert_eq!(city.text_overlay_at(52, 62), Some(0xcb));
    }
}
