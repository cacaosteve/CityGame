//! Win95 PE disaster actor construction, movement, and slot-ordered ticking.
//!
//! Keep actor traversal and random-state ownership intact; map overlay sweeps stay in
//! `disasters`.

use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeTornadoTickStats {
    pub(super) moved: usize,
    pub(super) destroyed: usize,
    pub(super) despawned: usize,
}

impl PeTornadoTickStats {
    pub(super) fn changed(self) -> usize {
        self.moved + self.destroyed + self.despawned
    }
}

/// Win95 `0x4e81f8[0x0f]` / `[5]` / `[1]` sub-tile steps via `0x403071`.
const PE_TORNADO_SUBTILE_STEP: i16 = 8;
const PE_MONSTER_SUBTILE_STEP: i16 = 8;
const PE_AIRCRASH_SUBTILE_STEP: i16 = 16;
const PE_XTHG_SUBTILE_GRID: i16 = 16;
/// Win95 airplane tick `0x453980` state 7 → kind-6 debris fields.
pub(super) const PE_AIRCRASH_IMPACT_STATE: u8 = 5;
const PE_AIRCRASH_MAYDAY_HEIGHT: u8 = 8;
/// Win95 monster AI `+2` states in `0x4548c0`.
pub(super) const PE_MONSTER_STATE_WALK: u8 = 0;
pub(super) const PE_MONSTER_STATE_ATTACK: u8 = 1;
const PE_MONSTER_STATE_CLIMB: u8 = 2;
pub(super) const PE_MONSTER_STATE_FLEE: u8 = 3;
const PE_MONSTER_ATTACK_SOUND_BIT: u8 = 0x80;

pub(super) fn pe_xthg_advance_subtile(sub: i16) -> (u8, i16) {
    let mut value = sub;
    let mut tile_delta = 0i16;
    // PE `FUN_004516F0` @ `0x451763`: one strict `> 0x10` carry and one `< 0` borrow
    // (exact `0x10` stays; no repeated normalize).
    if value > PE_XTHG_SUBTILE_GRID {
        value -= PE_XTHG_SUBTILE_GRID;
        tile_delta += 1;
    }
    if value < 0 {
        value += PE_XTHG_SUBTILE_GRID;
        tile_delta -= 1;
    }
    (value as u8, tile_delta)
}

/// Win95 `FUN_00452FA0` after `0x4CAA58/5C`: `rand()&3` nudges spawn ±0x10.
pub(super) fn pe_maxis_man_spawn_tile_jitter(
    event_x: usize,
    event_y: usize,
    crt: &mut Win95CrtRand,
) -> (usize, usize) {
    let mut x = event_x as i16;
    let mut y = event_y as i16;
    match crt.random_u16() & 3 {
        0 => {
            x = x.saturating_add(0x10).min(0x7f);
        }
        1 => {
            y = y.saturating_add(0x10).min(0x7f);
        }
        2 => {
            x = x.saturating_sub(0x10).max(0);
        }
        _ => {
            y = y.saturating_sub(0x10).max(0);
        }
    }
    (x as usize, y as usize)
}

/// Win95 `0x402dbf` / `0x4515e0`: 8-way direction from `(x,y)` toward `(tx,ty)`.
pub(super) fn pe_xthg_direction_toward(x: i16, y: i16, tx: i16, ty: i16) -> u8 {
    let dx = tx - x;
    let dy = ty - y;
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

/// Win95 `0x401848`: keep `lane`, or with 1-in-`modulus` odds apply `(rand%3)-1` then `&7`.
fn pe_xthg_maybe_jitter_dir(lane: u8, modulus: u16, rng: &mut Win95CrtRand) -> u8 {
    if modulus == 0 || rng.random_u16() % modulus != 0 {
        return lane & 7;
    }
    let delta = (rng.random_u16() % 3) as i16 - 1;
    (i16::from(lane) + delta).rem_euclid(8) as u8
}

/// Win95 `0x401528` → `FUN_004507D0`: clear XTHG kind only and zero the tile's XTXT
/// (no `saved_a` restore). Height and `saved_b` remain until the slot is recycled.
fn pe_xthg_despawn(city: &mut City, slot: usize, x: usize, y: usize) {
    if let Some(overlays) = city.traffic_overlays_mut()
        && let Some(record) = overlays.get_mut(slot)
    {
        record.kind = 0;
    }
    if x < CITY_WIDTH && y < CITY_HEIGHT {
        city.set_text_overlay(x, y, 0);
    }
}

/// Win95 `0x403071` / `0x4516f0` one step for tornado (`0x0f`) or monster (`5`).
fn pe_xthg_move_step(
    city: &mut City,
    slot: usize,
    rng_lane: u8,
    step: i16,
    is_actor: impl Fn(Win95OverlayRecord) -> bool,
    update_height_from_altm: bool,
) -> bool {
    let Some(mut record) = city
        .traffic_overlays()
        .and_then(|table| table.get(slot).copied())
    else {
        return false;
    };
    if !is_actor(record) {
        return false;
    }
    let old_x = usize::from(record.tile_x);
    let old_y = usize::from(record.tile_y);
    record.lane = rng_lane;
    let (dx_unit, dy_unit) = xthg_direction_delta(record.lane);
    let (sub_x, tile_dx) = pe_xthg_advance_subtile(i16::from(record.sub_x) + dx_unit * step);
    let (sub_y, tile_dy) = pe_xthg_advance_subtile(i16::from(record.sub_y) + dy_unit * step);
    let new_x = i16::from(record.tile_x) + tile_dx;
    let new_y = i16::from(record.tile_y) + tile_dy;
    // FUN_004516F0 stores both subtile bytes before it checks the carried tile
    // against the map bounds. An edge clear keeps those new subtile values.
    record.sub_x = sub_x;
    record.sub_y = sub_y;
    if new_x < 0 || new_y < 0 || new_x >= CITY_WIDTH as i16 || new_y >= CITY_HEIGHT as i16 {
        if let Some(overlays) = city.traffic_overlays_mut()
            && let Some(slot_rec) = overlays.get_mut(slot)
        {
            slot_rec.sub_x = sub_x;
            slot_rec.sub_y = sub_y;
        }
        pe_xthg_despawn(city, slot, old_x, old_y);
        return true;
    }
    let crossed = new_x as usize != old_x || new_y as usize != old_y;
    if crossed {
        // PE `FUN_004516F0` @ `0x451821`: unconditional old-tile XTXT restore from +A
        // before the dest occupancy loop.
        if old_x < CITY_WIDTH && old_y < CITY_HEIGHT {
            city.set_text_overlay(old_x, old_y, record.saved_a);
        }
        // PE @ `0x451848`/`0x45185c`: while dest XTXT `>= 0xC9`, commit tile and
        // re-add the same tile carry; only `FUN_00401528` when the next candidate
        // is OOB. A later clear cell takes the success paint path (heavy XTXT left).
        let mut cx = new_x;
        let mut cy = new_y;
        loop {
            if cx < 0 || cy < 0 || cx >= CITY_WIDTH as i16 || cy >= CITY_HEIGHT as i16 {
                pe_xthg_despawn(
                    city,
                    slot,
                    usize::from(record.tile_x),
                    usize::from(record.tile_y),
                );
                return true;
            }
            let nx = cx as usize;
            let ny = cy as usize;
            let dest_xtxt = city.text_overlay_at(nx, ny).unwrap_or(0);
            if dest_xtxt < 0xc9 {
                record.saved_a = dest_xtxt;
                record.tile_x = nx as u8;
                record.tile_y = ny as u8;
                if let Some(proxy) = xthg_proxy_for_slot(slot) {
                    city.set_text_overlay(nx, ny, proxy);
                }
                break;
            }
            record.tile_x = nx as u8;
            record.tile_y = ny as u8;
            if let Some(overlays) = city.traffic_overlays_mut()
                && let Some(slot_rec) = overlays.get_mut(slot)
            {
                slot_rec.sub_x = sub_x;
                slot_rec.sub_y = sub_y;
                slot_rec.tile_x = nx as u8;
                slot_rec.tile_y = ny as u8;
            }
            cx += tile_dx;
            cy += tile_dy;
            if cx < 0 || cy < 0 || cx >= CITY_WIDTH as i16 || cy >= CITY_HEIGHT as i16 {
                pe_xthg_despawn(city, slot, nx, ny);
                return true;
            }
        }
    }
    let nx = usize::from(record.tile_x);
    let ny = usize::from(record.tile_y);
    if update_height_from_altm {
        record.height = city.altitude_at(nx, ny) & 0x1f;
    }
    if let Some(overlays) = city.traffic_overlays_mut()
        && let Some(slot_rec) = overlays.get_mut(slot)
    {
        *slot_rec = record;
    }
    true
}

/// Win95 `0x403071` one step for air-crash airplane kind `1` (keeps dive `height`).
pub(super) fn pe_aircrash_move_step(city: &mut City, slot: usize, move_dir: u8) -> bool {
    pe_xthg_move_step(
        city,
        slot,
        move_dir,
        PE_AIRCRASH_SUBTILE_STEP,
        Win95OverlayRecord::is_pe_aircrash_actor,
        false,
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeAircraftTickStats {
    pub(super) moved: usize,
    pub(super) impacted: usize,
    pub(super) morphed: usize,
    pub(super) despawned: usize,
    pub(super) mayday_sounds: usize,
}

impl PeAircraftTickStats {
    pub(super) fn changed(self) -> usize {
        self.moved + self.impacted + self.morphed + self.despawned
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeExplosionTickStats {
    pub(super) aged: usize,
    pub(super) cleared: usize,
    pub(super) ignited: usize,
    /// PE `0x401096(0x1f8)` when XTHG `+1` (lane) is 0 before aging.
    pub(super) explode_sounds: usize,
    /// PE `0x40174e(0x4CAA58, tile)` when scatter lands at least one ignite.
    pub(super) auto_goto_tile: Option<(usize, usize)>,
}

impl PeExplosionTickStats {
    pub(super) fn changed(self) -> usize {
        self.aged + self.cleared + self.ignited
    }
}

pub(super) fn pe_aircrash_tick_city_with_lfsr(
    city: &mut City,
    lfsr: &mut Win95Lfsr,
) -> PeAircraftTickStats {
    let mut stats = PeAircraftTickStats::default();
    let slots: Vec<usize> = city
        .traffic_overlays()
        .map(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .filter_map(|(slot, record)| record.is_pe_aircrash_actor().then_some(slot))
                .collect()
        })
        .unwrap_or_default();
    for slot in slots {
        let one = pe_aircrash_tick_slot_with_lfsr(city, slot, lfsr);
        stats.moved += one.moved;
        stats.impacted += one.impacted;
        stats.morphed += one.morphed;
        stats.despawned += one.despawned;
        stats.mayday_sounds += one.mayday_sounds;
    }
    stats
}

fn pe_aircrash_tick_slot_with_lfsr(
    city: &mut City,
    slot: usize,
    lfsr: &mut Win95Lfsr,
) -> PeAircraftTickStats {
    let mut stats = PeAircraftTickStats::default();
    let Some(mut record) = city
        .traffic_overlays()
        .and_then(|table| table.get(slot).copied())
    else {
        return stats;
    };
    if !record.is_pe_aircrash_actor() {
        return stats;
    }
    // PE FUN_00453980 runs its shared airplane/building gate before the
    // state-7 arm. The >=0xfb case advances the process Game_Random LFSR.
    if crate::win95_overlay_sim::pe_plane_morph_on_building_with_lfsr(&mut record, city, lfsr) {
        if let Some(overlays) = city.traffic_overlays_mut()
            && let Some(slot_rec) = overlays.get_mut(slot)
        {
            *slot_rec = record;
        }
        stats.morphed += 1;
        return stats;
    }
    if record.height == 0 {
        // PE state-7 arm `0x453ec3`: morph directly. The XTXT restore and
        // XBLD `0xdd` exception at `0x453baf..0x453bc0` belong to state 1.
        record.kind = OVERLAY_KIND_TRAFFIC;
        record.state = PE_AIRCRASH_IMPACT_STATE;
        record.saved_b = 1;
        record.lane = 0;
        if let Some(overlays) = city.traffic_overlays_mut()
            && let Some(slot_rec) = overlays.get_mut(slot)
        {
            *slot_rec = record;
        }
        stats.impacted += 1;
        return stats;
    }
    record.height -= 1;
    if record.height == PE_AIRCRASH_MAYDAY_HEIGHT {
        stats.mayday_sounds += 1;
    }
    let move_dir = record.lane;
    let spun = (move_dir.wrapping_add(1)) & 7;
    record.lane = spun;
    if let Some(overlays) = city.traffic_overlays_mut()
        && let Some(slot_rec) = overlays.get_mut(slot)
    {
        slot_rec.height = record.height;
        slot_rec.lane = spun;
    }
    let before = (record.tile_x, record.tile_y);
    if pe_aircrash_move_step(city, slot, move_dir) {
        let after = city
            .traffic_overlays()
            .and_then(|table| table.get(slot).copied());
        if !after.is_some_and(Win95OverlayRecord::is_pe_aircrash_actor) {
            // Out-of-bounds / blocked proxy: `0x403071` despawned this slot.
            stats.despawned += 1;
            return stats;
        }
        // PE post-spin lane is written by the kind-1 handler after movement.
        if let Some(overlays) = city.traffic_overlays_mut()
            && let Some(slot_rec) = overlays.get_mut(slot)
        {
            slot_rec.lane = spun;
            slot_rec.height = record.height;
        }
        if after.is_some_and(|r| (r.tile_x, r.tile_y) != before) {
            stats.moved += 1;
        }
    }
    stats
}

pub(super) fn pe_aircrash_tick_city(city: &mut City) -> PeAircraftTickStats {
    let mut lfsr = Win95Lfsr::from_u32_seed(0x4539_8000);
    pe_aircrash_tick_city_with_lfsr(city, &mut lfsr)
}

/// Interleave extracted Win95 Things handlers in ascending native XTHG slot
/// order, returning disaster and train change counts separately so train
/// redraws cannot trigger disaster-state persistence.
pub(super) fn pe_disaster_xthg_tick_slots_in_order(
    overlay_sim: &mut Win95OverlaySim,
    city: &mut City,
    view_x: u8,
    view_y: u8,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    game_rand: &mut Win95GameRand,
) -> (
    PeAircraftTickStats,
    PeMonsterTickStats,
    PeTornadoTickStats,
    PeExplosionTickStats,
    usize,
    usize,
) {
    let mut aircraft = PeAircraftTickStats::default();
    let mut monster = PeMonsterTickStats::default();
    let mut tornado = PeTornadoTickStats::default();
    let mut explosion = PeExplosionTickStats::default();
    let mut maxis_man_changed = 0;
    let mut train_changed = 0;
    let mut airplane_prefix = false;
    let mut helicopter_prefix = false;
    let mut monster_dispatched = false;
    // PE `FUN_00450890` resets `0x4E99D0` before the ascending slot sweep.
    city.pe_sailboat_budget = 0;
    for slot in XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT {
        let Some(record) = city
            .traffic_overlays()
            .and_then(|table| table.get(slot).copied())
        else {
            continue;
        };
        match record.kind {
            XTHG_AIRPLANE => {
                // `FUN_00453980` increments this dispatcher-prefix counter at
                // entry, even when its state arm later morphs the record.
                airplane_prefix = true;
                if record.is_pe_aircrash_actor() {
                    let one = pe_aircrash_tick_slot_with_lfsr(city, slot, lfsr);
                    aircraft.moved += one.moved;
                    aircraft.impacted += one.impacted;
                    aircraft.morphed += one.morphed;
                    aircraft.despawned += one.despawned;
                    aircraft.mayday_sounds += one.mayday_sounds;
                } else {
                    overlay_sim.tick_pe_aircraft_slot(city, slot, crt, lfsr);
                }
            }
            XTHG_HELICOPTER => {
                helicopter_prefix = true;
                let one = overlay_sim.tick_pe_aircraft_slot_with_stats(city, slot, crt, lfsr);
                aircraft.mayday_sounds += one.mayday_sounds;
            }
            XTHG_MONSTER if !monster_dispatched => {
                let one = pe_monster_tick_city_with_prefix(
                    city,
                    view_x,
                    view_y,
                    crt,
                    lfsr,
                    Some((airplane_prefix, helicopter_prefix)),
                );
                monster.moved += one.moved;
                monster.destroyed += one.destroyed;
                monster.despawned += one.despawned;
                monster.attack_sounds += one.attack_sounds;
                monster_dispatched = true;
            }
            XTHG_TORNADO => {
                let one = pe_tornado_tick_slot_with_rng(city, slot, crt);
                tornado.moved += one.moved;
                tornado.destroyed += one.destroyed;
                tornado.despawned += one.despawned;
            }
            XTHG_EXPLOSION => {
                let one = pe_explosion_tick_slot_with_lfsr(city, slot, crt, lfsr);
                explosion.aged += one.aged;
                explosion.cleared += one.cleared;
                explosion.ignited += one.ignited;
                explosion.explode_sounds += one.explode_sounds;
                if one.auto_goto_tile.is_some() {
                    explosion.auto_goto_tile = one.auto_goto_tile;
                }
            }
            XTHG_MAXIS_MAN => {
                if overlay_sim.tick_pe_maxis_man_slot(city, slot, crt) {
                    maxis_man_changed += 1;
                }
            }
            XTHG_TRAIN_ENGINE | XTHG_SUBWAY_TRAIN_ENGINE => {
                if overlay_sim.tick_pe_train_engine_slot(city, slot, crt, lfsr, game_rand) {
                    train_changed += 1;
                }
            }
            XTHG_CARGO_SHIP => {
                overlay_sim.tick_pe_cargo_ship_slot(city, slot, crt, lfsr);
            }
            XTHG_SAILBOAT => {
                overlay_sim.tick_pe_sailboat_slot(city, slot, crt, lfsr);
            }
            _ => {}
        }
    }
    (
        aircraft,
        monster,
        tornado,
        explosion,
        maxis_man_changed,
        train_changed,
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeMonsterTickStats {
    pub(super) moved: usize,
    pub(super) destroyed: usize,
    pub(super) despawned: usize,
    pub(super) attack_sounds: usize,
}

impl PeMonsterTickStats {
    pub(super) fn changed(self) -> usize {
        self.moved + self.destroyed + self.despawned
    }
}

/// Win95 monster arm @ `0x472125`: `dwCityPopulation >= 0xAFC8`.
///
/// This reads the PE runtime global directly. Census estimates and `pZonePops`
/// readiness are separate state and must not change the disaster gate.
pub(super) fn pe_monster_disaster_ready(city: &City) -> bool {
    city.pe_dw_city_population() >= 45_000
}

pub(super) fn pe_monster_actor_seed(city: &City, x: usize, y: usize) -> u32 {
    let mut seed = 0x45EE_1000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(super) fn pe_tornado_actor_seed(city: &City, x: usize, y: usize) -> u32 {
    let mut seed = 0x45F0_9000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 tornado start @ `0x45f090`: dir `rand&7`, height `ALTM&0x1f`, `target=rand&0x7f`.
pub(super) fn pe_tornado_xthg_fields_with_rng(
    city: &City,
    x: usize,
    y: usize,
    rng: &mut Win95CrtRand,
) -> (u8, u8, u8, u8, u8) {
    let lane = (rng.random_u15() & 7) as u8;
    let target_x = (rng.random_u15() & 0x7f) as u8;
    let target_y = (rng.random_u15() & 0x7f) as u8;
    let height = city.altitude_at(x, y) & 0x1f;
    let saved_a = city.text_overlay_at(x, y).unwrap_or(0);
    (lane, height, target_x, target_y, saved_a)
}

pub(super) fn pe_tornado_xthg_fields(city: &City, x: usize, y: usize) -> (u8, u8, u8, u8, u8) {
    let mut rng = Win95CrtRand::new(pe_tornado_actor_seed(city, x, y));
    pe_tornado_xthg_fields_with_rng(city, x, y, &mut rng)
}

pub(super) fn pe_tornado_tick_seed(city: &City) -> u32 {
    let mut seed = 0x454E_A000u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(super) fn pe_tornado_jitter_dir(lane: u8, rng: &mut Win95CrtRand) -> u8 {
    let a = rng.random_u16() % 3;
    let b = rng.random_u16() % 3;
    (i16::from(lane) + a as i16 - b as i16).rem_euclid(8) as u8
}

/// Win95 `0x454f84..0x454fa2`: second tornado move uses two low-bit CRT rolls,
/// relative to the original record lane rather than the first temporary heading.
pub(super) fn pe_tornado_second_jitter_dir(lane: u8, rng: &mut Win95CrtRand) -> u8 {
    let a = (rng.random_u16() & 1) as i16;
    let b = (rng.random_u16() & 1) as i16;
    (i16::from(lane) + a - b).rem_euclid(8) as u8
}

pub(super) fn pe_tornado_despawn(city: &mut City, slot: usize, x: usize, y: usize) {
    pe_xthg_despawn(city, slot, x, y);
}

pub(super) fn pe_monster_despawn(city: &mut City, slot: usize, x: usize, y: usize) {
    pe_xthg_despawn(city, slot, x, y);
}

/// Win95 `0x403071` one step for tornado kind `0x0f`.
pub(super) fn pe_tornado_move_step(city: &mut City, slot: usize, rng_lane: u8) -> bool {
    let saved_lane = city
        .traffic_overlays()
        .and_then(|table| table.get(slot))
        .map(|record| record.lane);
    let moved = pe_xthg_move_step(
        city,
        slot,
        rng_lane,
        PE_TORNADO_SUBTILE_STEP,
        // The PE thunk moves by slot without rechecking kind. FUN_00454EA0
        // can clear this slot and still call it again before the handler ends.
        |_| true,
        false,
    );
    // FUN_004516F0 uses the supplied heading for displacement, but does not
    // write it back to XTHG +1. Tornado headings are temporary RNG results.
    if let Some(saved_lane) = saved_lane
        && let Some(record) = city
            .traffic_overlays_mut()
            .and_then(|table| table.get_mut(slot))
    {
        record.lane = saved_lane;
    }
    moved
}

/// Win95 `0x403071` one step for monster kind `5` (keeps animated `height`).
pub(super) fn pe_monster_move_step(city: &mut City, slot: usize, rng_lane: u8) -> bool {
    pe_xthg_move_step(
        city,
        slot,
        rng_lane,
        PE_MONSTER_SUBTILE_STEP,
        Win95OverlayRecord::is_pe_monster_actor,
        false,
    )
}

/// Win95 kind-6 Things handler `0x4546f0`: play explode when lane/`+1` is 0,
/// then age XTHG `+1` while `<=1`; when `+1>1`, `FUN_00401528` clear kind and
/// zero tile XTXT; when `+B≠0`, four `0x401cad(5)` ±2 probes call ignite flag 1
/// (`FUN_00460e10` @ `0x4547da` → mid-XTXT DestroyStructure CRT PlaceTile rubble).
pub(super) fn pe_explosion_tick_city_with_lfsr(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeExplosionTickStats {
    let mut stats = PeExplosionTickStats::default();
    let slots: Vec<usize> = city
        .traffic_overlays()
        .map(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .filter_map(|(slot, record)| record.is_pe_explosion_actor().then_some(slot))
                .collect()
        })
        .unwrap_or_default();
    for slot in slots {
        let one = pe_explosion_tick_slot_with_lfsr(city, slot, crt, lfsr);
        stats.aged += one.aged;
        stats.cleared += one.cleared;
        stats.ignited += one.ignited;
        stats.explode_sounds += one.explode_sounds;
        if one.auto_goto_tile.is_some() {
            stats.auto_goto_tile = one.auto_goto_tile;
        }
    }
    stats
}

pub(super) fn pe_explosion_tick_slot_with_lfsr(
    city: &mut City,
    slot: usize,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeExplosionTickStats {
    let mut stats = PeExplosionTickStats::default();
    let Some(record) = city
        .traffic_overlays()
        .and_then(|table| table.get(slot).copied())
    else {
        return stats;
    };
    if !record.is_pe_explosion_actor() {
        return stats;
    }
    // PE @ `0x454719`: lane/`+1` == 0 → `0x401096(0x1f8)` before age.
    if record.lane == 0 {
        stats.explode_sounds += 1;
    }
    if record.lane <= 1 {
        if let Some(overlays) = city.traffic_overlays_mut()
            && let Some(slot_rec) = overlays.get_mut(slot)
        {
            slot_rec.lane = slot_rec.lane.saturating_add(1);
        }
        stats.aged += 1;
        return stats;
    }
    let x = usize::from(record.tile_x);
    let y = usize::from(record.tile_y);
    let saved_b = record.saved_b;
    if let Some(overlays) = city.traffic_overlays_mut()
        && let Some(slot_rec) = overlays.get_mut(slot)
    {
        // PE `FUN_00401528` → `FUN_004507D0`: clear kind only.
        slot_rec.kind = 0;
    }
    if x < CITY_WIDTH && y < CITY_HEIGHT {
        city.set_text_overlay(x, y, 0);
    }
    stats.cleared += 1;
    // PE `0x454782`–`0x4547f1`: `+B≠0` → four LFSR ±2 ignite probes (flag 1).
    // PE @ `0x45478a`: skip scatter only when No Disasters is set
    // (`0x4CC4D4 != 0`) and no scenario is active (`0x4CAD44 == 0`).
    // This gate is before all four calls to `0x401cad`, so a skipped path
    // consumes no Game_Random values.
    if saved_b == 0
        || (no_disasters_enabled_for_engine(city, EngineFidelity::Win95)
            && !city.pe_scenario_active())
    {
        return stats;
    }
    let mut scatter_hit = false;
    for _ in 0..4 {
        let nx = x as i16 + lfsr.random_mod(5) as i16 - 2;
        let ny = y as i16 + lfsr.random_mod(5) as i16 - 2;
        if !(0..CITY_WIDTH as i16).contains(&nx) || !(0..CITY_HEIGHT as i16).contains(&ny) {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        // PE @ `0x4547da`: `push 1; call 0x401bf4` — no outer XTXT/water prefilter.
        // PE @ `0x4547e2`: `test eax` — rubble-band / refuse / already-`0xff` ret 0.
        if pe_fun_00460e10_ignite(city, Some(crt), lfsr, nx, ny, 1) {
            stats.ignited += 1;
            scatter_hit = true;
        }
    }
    // PE `0x4547f3`–`0x454838`: any ignite → `0x40174e` on the explosion tile
    // when auto-goto mode `word[0x4CA42C] ≠ 2`.
    // Then `wSetTriggerDisasterType` (`0x4CA420`) = XTHG state, or 1 if state is 0.
    if scatter_hit {
        stats.auto_goto_tile = Some((x, y));
        let trigger = if record.state != 0 {
            u32::from(record.state)
        } else {
            1
        };
        city.pe_w_trigger_disaster = trigger as u16;
        if city.kind != CityFileKind::MacCity {
            city.set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, trigger);
        }
    }
    stats
}

pub(super) fn pe_explosion_tick_city(city: &mut City) -> PeExplosionTickStats {
    let mut crt = Win95CrtRand::new(0x4546_f000);
    let mut lfsr = Win95Lfsr::from_u32_seed(0x4546_f000);
    pe_explosion_tick_city_with_lfsr(city, &mut crt, &mut lfsr)
}

/// Win95 `0x454ea0` XTHG tornado: DestroyBuilding mode 1 when XBLD `>=6`, jitter dir,
/// move (`0x403071`), 1-in-256 despawn; second move unless pre-destroy XBLD `>=0x0d`.
pub(super) fn pe_tornado_tick_city_with_rng(
    city: &mut City,
    crt: &mut Win95CrtRand,
) -> PeTornadoTickStats {
    let mut stats = PeTornadoTickStats::default();
    let slots: Vec<usize> = city
        .traffic_overlays()
        .map(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .filter_map(|(slot, record)| record.is_pe_tornado_actor().then_some(slot))
                .collect()
        })
        .unwrap_or_default();
    for slot in slots {
        let one = pe_tornado_tick_slot_with_rng(city, slot, crt);
        stats.moved += one.moved;
        stats.destroyed += one.destroyed;
        stats.despawned += one.despawned;
    }
    stats
}

pub(super) fn pe_tornado_tick_slot_with_rng(
    city: &mut City,
    slot: usize,
    crt: &mut Win95CrtRand,
) -> PeTornadoTickStats {
    let mut stats = PeTornadoTickStats::default();
    let Some(record) = city
        .traffic_overlays()
        .and_then(|table| table.get(slot).copied())
    else {
        return stats;
    };
    if !record.is_pe_tornado_actor() {
        return stats;
    }
    let x = usize::from(record.tile_x);
    let y = usize::from(record.tile_y);
    let building = city.building_at(x, y);
    let mut heavy = false;
    if building >= 6 {
        // PE @ `0x454f08`: DestroyBuilding mode 1 (`0x402211` → `0x458d40`), not
        // DestroyStructure. Elevated `0x3f..=0x42` take the elevated arm then still
        // count as a destroy; `heavy` is pre-destroy XBLD `>= 0x0d` (`0x454f0f`).
        if matches!(building, 0x3f..=0x42) {
            pe_destroy_building_elevated_arm_45994e(city, x, y);
        } else {
            let _ = pe_flood_destroy_tile_with_crt(city, crt, x, y);
        }
        stats.destroyed += 1;
        heavy = building >= 0x0d;
    }
    let base_lane = record.lane;
    let dir = pe_tornado_jitter_dir(base_lane, crt);
    let before = city
        .traffic_overlays()
        .and_then(|table| table.get(slot).copied());
    if pe_tornado_move_step(city, slot, dir) {
        let after = city
            .traffic_overlays()
            .and_then(|table| table.get(slot).copied());
        if before.is_some_and(Win95OverlayRecord::is_pe_tornado_actor)
            && after.is_some_and(|r| !r.is_pe_tornado_actor())
        {
            stats.despawned += 1;
        }
        if let (Some(before), Some(after)) = (before, after)
            && after.is_pe_tornado_actor()
            && (before.tile_x, before.tile_y, before.sub_x, before.sub_y)
                != (after.tile_x, after.tile_y, after.sub_x, after.sub_y)
        {
            stats.moved += 1;
        }
    }
    if crt.random_u16() & 0xff == 0 {
        if let Some(live) = city
            .traffic_overlays()
            .and_then(|table| table.get(slot).copied())
        {
            let was_actor = live.is_pe_tornado_actor();
            pe_tornado_despawn(
                city,
                slot,
                usize::from(live.tile_x),
                usize::from(live.tile_y),
            );
            if was_actor {
                stats.despawned += 1;
            }
        }
    }
    if heavy {
        return stats;
    }
    let Some(live) = city
        .traffic_overlays()
        .and_then(|table| table.get(slot).copied())
    else {
        return stats;
    };
    let dir2 = pe_tornado_second_jitter_dir(base_lane, crt);
    let before2 = live;
    if pe_tornado_move_step(city, slot, dir2) {
        let after2 = city
            .traffic_overlays()
            .and_then(|table| table.get(slot).copied());
        if before2.is_pe_tornado_actor() && after2.is_some_and(|r| !r.is_pe_tornado_actor()) {
            stats.despawned += 1;
        }
        if let Some(after2) = after2
            && after2.is_pe_tornado_actor()
            && (before2.tile_x, before2.tile_y, before2.sub_x, before2.sub_y)
                != (after2.tile_x, after2.tile_y, after2.sub_x, after2.sub_y)
        {
            stats.moved += 1;
        }
    }
    if crt.random_u16() & 0xff == 0
        && let Some(live) = city
            .traffic_overlays()
            .and_then(|table| table.get(slot).copied())
    {
        let was_actor = live.is_pe_tornado_actor();
        pe_tornado_despawn(
            city,
            slot,
            usize::from(live.tile_x),
            usize::from(live.tile_y),
        );
        if was_actor {
            stats.despawned += 1;
        }
    }
    stats
}

pub(super) fn pe_tornado_tick_city(city: &mut City) -> PeTornadoTickStats {
    // PE tornado tick `0x454ea0` draws `0x48cdd8` (CRT rand), not the game LFSR.
    let mut rng = Win95CrtRand::new(pe_tornado_tick_seed(city));
    pe_tornado_tick_city_with_rng(city, &mut rng)
}

pub(super) fn pe_monster_tick_seed(city: &City) -> u32 {
    let mut seed = 0x4548_C000u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(super) fn pe_monster_neighbor_has_military(city: &City, x: i16, y: i16, dir: u8) -> bool {
    let (dx, dy) = xthg_direction_delta(dir);
    let nx = x + dx;
    let ny = y + dy;
    if nx < 0 || ny < 0 || nx >= CITY_WIDTH as i16 || ny >= CITY_HEIGHT as i16 {
        return false;
    }
    let xtxt = city.text_overlay_at(nx as usize, ny as usize).unwrap_or(0);
    if !(0xc9..0xf1).contains(&xtxt) {
        return false;
    }
    let Some(slot) = xthg_slot_for_proxy(xtxt) else {
        return false;
    };
    city.traffic_overlays()
        .and_then(|table| table.get(slot).copied())
        .is_some_and(|record| record.kind == OVERLAY_KIND_DISPATCH_MILITARY)
}

pub(super) fn pe_monster_attack_tile(
    city: &mut City,
    rng: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    ax: usize,
    ay: usize,
    saved_b: u8,
) -> bool {
    let building = city.building_at(ax, ay);
    if saved_b == 0 {
        // PE @ `0x454b45`: XBLD `>= 6` then `0x401bf4` flag 0; `test eax` for success.
        if city.building_at(ax, ay) < 6 {
            return false;
        }
        return pe_fun_00460e10_ignite(city, Some(rng), lfsr, ax, ay, 0);
    }
    if city
        .tile_bits_at(ax, ay)
        .is_some_and(crate::city::TileBits::water_tile)
    {
        return false;
    }
    if building < 0x0e || building == 0xc8 {
        return false;
    }
    // Win95 `0x454bc7`: DestroyBuilding mode 1 (`0x402211` → `0x458d40`), then
    // mode follow-up (`0x40178f` / `0x401884` / `0x4027f2`) and always OR `target_x|0x80`
    // @ `0x454c20`. Not DestroyStructure — elevated `0x3f..=0x42` take the
    // DestroyBuilding elevated arm (`0x45994e` PlaceTile 0 / XZON `&=0x0f` / ALTM
    // `&=0x3ff`), then still run follow-ups.
    if matches!(building, 0x3f..=0x42) {
        pe_destroy_building_elevated_arm_45994e(city, ax, ay);
    } else {
        let _ = pe_flood_destroy_tile_with_crt(city, rng, ax, ay);
    }
    match saved_b {
        1 => {
            // PE @ `0x454bb0`: CRT `&3+9` then `0x40178f` → PlaceTile `0x441f00`.
            let rubble = (rng.random_u16() & 3) as u8 + 9;
            pe_place_tile_441f00(city, ax, ay, rubble);
        }
        2 => {
            // PE @ `0x454c03`: `push y; push x; call 0x401884` → PlaceWater helper `0x446650`.
            pe_place_water_helper_446650(city, ax, ay);
        }
        3 => {
            // PE @ `0x454c0f`: `push 1; push 0xc8; call 0x4027f2` → PlaceTile-size `0x440c50`.
            let _ = pe_place_tile_size_440c50(city, ax, ay, 0xc8, 1);
        }
        _ => {}
    }
    true
}

/// Win95 `0x4548c0` XTHG monster: height/state AI, SE attack, steer/move, 1-in-100 flee despawn.
pub(super) fn pe_monster_tick_city_with_rng(
    city: &mut City,
    view_x: u8,
    view_y: u8,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeMonsterTickStats {
    pe_monster_tick_city_with_prefix(city, view_x, view_y, crt, lfsr, None)
}

pub(super) fn pe_monster_tick_city_with_prefix(
    city: &mut City,
    view_x: u8,
    view_y: u8,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    dispatch_prefix: Option<(bool, bool)>,
) -> PeMonsterTickStats {
    let mut stats = PeMonsterTickStats::default();
    let slots: Vec<usize> = city
        .traffic_overlays()
        .map(|table| {
            table
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .filter_map(|(slot, record)| record.is_pe_monster_actor().then_some(slot))
                .collect()
        })
        .unwrap_or_default();
    if slots.is_empty() {
        // `FUN_004548c0` is dispatched per kind-5 record. Its scare prologue
        // cannot affect ambient actors when the XTHG walk contains no monster.
        return stats;
    }

    // PE `FUN_004548c0` @ `0x454903` sees counters accumulated by earlier
    // handlers in the ascending `FUN_00450890` slot walk. A preceding airplane
    // (`0x4e99c0`) forces every plane to state 7; a preceding heli (`0x4e99c4`)
    // forces every heli to state 5. Later slots do not increment those counters
    // before this monster call, even though the prologue's write loop scans all slots.
    let (has_plane, has_heli) = dispatch_prefix.unwrap_or_else(|| {
        city.traffic_overlays()
            .map(|overlays| {
                let preceded_by = |kind| {
                    slots.iter().any(|&monster_slot| {
                        overlays
                            .iter()
                            .take(monster_slot)
                            .skip(XTHG_FIRST_USABLE_SLOT)
                            .any(|record| record.kind == kind)
                    })
                };
                (preceded_by(XTHG_AIRPLANE), preceded_by(XTHG_HELICOPTER))
            })
            .unwrap_or_default()
    });
    if let Some(overlays) = city.traffic_overlays_mut() {
        if has_plane || has_heli {
            for record in overlays.iter_mut().skip(XTHG_FIRST_USABLE_SLOT) {
                if has_plane && record.kind == XTHG_AIRPLANE {
                    record.state = 7;
                }
                if has_heli && record.kind == XTHG_HELICOPTER {
                    record.state = 5;
                }
            }
        }
    }
    for slot in slots {
        let Some(mut record) = city
            .traffic_overlays()
            .and_then(|table| table.get(slot).copied())
        else {
            continue;
        };
        if !record.is_pe_monster_actor() {
            continue;
        }
        let mut dir = record.lane;
        let mut skip_move = false;
        match record.state {
            PE_MONSTER_STATE_WALK => {
                if record.height > 8 {
                    record.height -= 1;
                } else {
                    record.state = PE_MONSTER_STATE_ATTACK;
                }
                record.target_x = 0;
                record.target_y = 0;
                dir = pe_xthg_direction_toward(
                    i16::from(record.tile_x),
                    i16::from(record.tile_y),
                    i16::from(view_x),
                    i16::from(view_y),
                );
                if let Some(overlays) = city.traffic_overlays_mut()
                    && let Some(slot_rec) = overlays.get_mut(slot)
                {
                    *slot_rec = record;
                }
            }
            PE_MONSTER_STATE_ATTACK => {
                if crt.random_u16() % 25 == 0 {
                    record.state = PE_MONSTER_STATE_CLIMB;
                }
                record.target_x = (crt.random_u16() & 0x7f) as u8;
                record.target_y = (crt.random_u16() & 0x7f) as u8;
                // PE `0x4549ea`→`0x454a03`: choose the direction toward the
                // current view, then check the next tile's XTXT actor proxy
                // before attack side effects (`0x454a4a`..`0x454a99`).
                dir = pe_xthg_direction_toward(
                    i16::from(record.tile_x),
                    i16::from(record.tile_y),
                    i16::from(view_x),
                    i16::from(view_y),
                );
                let (dx, dy) = xthg_direction_delta(dir);
                let candidate_x = i16::from(record.tile_x) + dx;
                let candidate_y = i16::from(record.tile_y) + dy;
                if pe_monster_neighbor_has_military(
                    city,
                    i16::from(record.tile_x),
                    i16::from(record.tile_y),
                    dir,
                ) {
                    record.state = PE_MONSTER_STATE_FLEE;
                    skip_move = true;
                } else if (0..0x7f).contains(&candidate_x)
                    && (0..0x7f).contains(&candidate_y)
                    && pe_monster_attack_tile(
                        city,
                        crt,
                        lfsr,
                        (candidate_x + 1) as usize,
                        (candidate_y + 1) as usize,
                        record.saved_b,
                    )
                {
                    stats.destroyed += 1;
                    record.target_x |= PE_MONSTER_ATTACK_SOUND_BIT;
                }
                if let Some(overlays) = city.traffic_overlays_mut()
                    && let Some(slot_rec) = overlays.get_mut(slot)
                {
                    slot_rec.state = record.state;
                    slot_rec.target_x = record.target_x;
                    slot_rec.target_y = record.target_y;
                }
            }
            PE_MONSTER_STATE_CLIMB => {
                if record.height < 0x0f {
                    record.height += 1;
                } else if crt.random_u16() % 3 == 0 {
                    record.state = PE_MONSTER_STATE_FLEE;
                } else {
                    record.state = PE_MONSTER_STATE_WALK;
                }
                let step = ((crt.random_u16() & 7) as u8).wrapping_mul(9);
                record.target_x = step;
                record.target_y = step;
                dir = pe_xthg_maybe_jitter_dir(record.lane, 5, crt);
                if let Some(overlays) = city.traffic_overlays_mut()
                    && let Some(slot_rec) = overlays.get_mut(slot)
                {
                    *slot_rec = record;
                }
            }
            PE_MONSTER_STATE_FLEE => {
                record.target_x = 0;
                record.target_y = 0;
                if crt.random_u16() & 1 != 0 {
                    record.target_x = 0x24;
                    record.target_y = 0x24;
                }
                if let Some(overlays) = city.traffic_overlays_mut()
                    && let Some(slot_rec) = overlays.get_mut(slot)
                {
                    slot_rec.target_x = record.target_x;
                    slot_rec.target_y = record.target_y;
                }
                // Win95 `0x401cad(0x64)`: despawn when remainder is 0.
                if lfsr.random_mod(100) == 0 {
                    pe_monster_despawn(
                        city,
                        slot,
                        usize::from(record.tile_x),
                        usize::from(record.tile_y),
                    );
                    stats.despawned += 1;
                    continue;
                }
            }
            _ => {}
        }

        let Some(live) = city
            .traffic_overlays()
            .and_then(|table| table.get(slot).copied())
        else {
            continue;
        };
        if !live.is_pe_monster_actor() {
            continue;
        }
        if live.state != PE_MONSTER_STATE_FLEE
            && pe_monster_neighbor_has_military(
                city,
                i16::from(live.tile_x),
                i16::from(live.tile_y),
                dir,
            )
        {
            if let Some(overlays) = city.traffic_overlays_mut()
                && let Some(slot_rec) = overlays.get_mut(slot)
            {
                slot_rec.state = PE_MONSTER_STATE_FLEE;
            }
            skip_move = true;
        }
        if skip_move {
            continue;
        }

        let before = (live.tile_x, live.tile_y, live.lane);
        if pe_monster_move_step(city, slot, dir) {
            if city.pe_monster_actor().is_none() {
                stats.despawned += 1;
                continue;
            }
            if city
                .pe_monster_actor()
                .map(|(_, r)| (r.tile_x, r.tile_y, r.lane))
                != Some(before)
            {
                stats.moved += 1;
            }
        }
        if let Some((_, after)) = city.pe_monster_actor()
            && after.target_x & PE_MONSTER_ATTACK_SOUND_BIT != 0
        {
            stats.attack_sounds += 1;
            if let Some(overlays) = city.traffic_overlays_mut()
                && let Some(slot_rec) = overlays.get_mut(slot)
            {
                slot_rec.target_x &= !PE_MONSTER_ATTACK_SOUND_BIT;
            }
        }
    }
    stats
}

pub(super) fn pe_monster_tick_city(city: &mut City, view_x: u8, view_y: u8) -> PeMonsterTickStats {
    // PE monster tick `0x4548c0`: CRT `0x48cdd8` for AI rolls; LFSR `0x401cad` for flee despawn.
    let mut rng = Win95CrtRand::new(pe_monster_tick_seed(city));
    let mut lfsr = Win95Lfsr::from_u32_seed(pe_monster_tick_seed(city));
    pe_monster_tick_city_with_rng(city, view_x, view_y, &mut rng, &mut lfsr)
}

/// Win95 monster start @ `0x45ee10`: `target=rand&0x7f`, `saved_b` 0 or `rand%3+1`.
pub(super) fn pe_monster_xthg_fields_with_rng(
    city: &City,
    x: usize,
    y: usize,
    rng: &mut Win95CrtRand,
) -> (u8, u8, u8, u8) {
    let target_x = (rng.random_u15() & 0x7f) as u8;
    let target_y = (rng.random_u15() & 0x7f) as u8;
    let saved_a = city.text_overlay_at(x, y).unwrap_or(0);
    let saved_b = if rng.random_u15() & 1 == 0 {
        0
    } else {
        (rng.random_u15() % 3) as u8 + 1
    };
    (target_x, target_y, saved_a, saved_b)
}

pub(super) fn pe_monster_xthg_fields(city: &City, x: usize, y: usize) -> (u8, u8, u8, u8) {
    let mut rng = Win95CrtRand::new(pe_monster_actor_seed(city, x, y));
    pe_monster_xthg_fields_with_rng(city, x, y, &mut rng)
}
