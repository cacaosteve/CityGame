//! Win95 port-side road-decoration traffic actors over the shared XTRF map.
//!
//! This module owns road-actor state updates and spawn probes; PE XTHG actor
//! dispatch remains in the parent and in the actor-family modules.

use super::{
    City, SUBTILE_CENTER, Win95OverlayRecord, Win95OverlayRng, advance_subtile, pe_clear_xthg_kind,
    tile_in_bounds,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::tiles::rotated_building_id;
use crate::traffic::{
    MAC_TRAFFIC_HEAVY_BYTE_MIN, MAC_TRAFFIC_VISIBLE_BYTE_MIN, PORT_TRAFFIC_ACTOR_SPAWN_THRESHOLD,
    has_decorative_traffic, is_spawnable_traffic_road,
};
use crate::win95_overlay::{
    OVERLAY_KIND_TRAFFIC, OverlayState, RUST_TRAFFIC_ACTOR_KIND, pe_overlay_spawn_slot,
};
use crate::win95_traffic_placement::{
    lane_index_for_tile, lane_spread_vector, road_placement_scale,
};

/// `saved_b` values below this index another overlay slot (`FUN_00455020` @ `0x455064`).
pub(super) const SAVED_B_SLOT_LINK: u8 = 0xf1;
/// Building ids at or above this are not overlay-road tiles (`cmp 0xf1` in overlay update).
pub(super) const BUILDING_OVERLAY_BLOCK: u8 = 0xf1;
/// Inactive scan radius around `target_x`/`target_y` (`add/sub 0x20` loops @ `0x4550c5`).
pub(super) const SCAN_RADIUS: i16 = 0x20;

const SPAWN_TILES_PER_TICK: usize = 6;
pub(super) fn update_slot<R: Win95OverlayRng>(
    city: &mut City,
    slot: usize,
    table: &[Win95OverlayRecord],
    rotation: u8,
    rng: &mut R,
) -> bool {
    let Some(mut record) = table.get(slot).copied() else {
        return false;
    };
    // Emergency dispatch XTHG records deliberately keep state zero and are updated by their
    // own game logic. The optional traffic simulation must leave them and their height intact.
    if !city.is_port_traffic_record(record) {
        return false;
    }

    let old_x = record.tile_x as usize;
    let old_y = record.tile_y as usize;

    let mut changed = match record.overlay_state() {
        OverlayState::Inactive => update_inactive_slot(&mut record, city, table, rotation),
        OverlayState::SpawnPending => promote_spawn_pending(&mut record, city, rotation),
        OverlayState::LightActive => update_light_actor(&mut record, city, rotation, rng),
        OverlayState::HeavyActive => update_heavy_actor(&mut record, city, rotation, rng),
    };

    let height_before = record.height;
    refresh_overlay_height(&mut record, city, table);
    if record.height != height_before {
        changed = true;
    }

    if changed {
        if let Some(overlays) = city.traffic_overlays_mut() {
            overlays[slot] = record;
        }
        city.mark_render_dirty(old_x, old_y);
        city.mark_render_dirty(record.tile_x as usize, record.tile_y as usize);
    }
    changed
}

/// Inactive overlay scan via `saved_b` slot link or `target_x`/`target_y` (`FUN_00455020` state 0).
pub(super) fn update_inactive_slot(
    record: &mut Win95OverlayRecord,
    city: &City,
    table: &[Win95OverlayRecord],
    rotation: u8,
) -> bool {
    if !matches!(record.kind, OVERLAY_KIND_TRAFFIC | RUST_TRAFFIC_ACTOR_KIND) {
        return false;
    }
    if record.saved_b >= SAVED_B_SLOT_LINK
        && record.target_x == 0
        && record.target_y == 0
        && record.tile_x == 0
        && record.tile_y == 0
    {
        return false;
    }

    let (origin_x, origin_y) = resolve_scan_origin(record, table);
    if tile_in_bounds(origin_x, origin_y) {
        let building = rotated_building_id(city.building_at(origin_x, origin_y), rotation);
        if is_overlay_road_building(building) {
            let byte = city.traffic_density_at(origin_x, origin_y).unwrap_or(0);
            if port_traffic_actor_spawn_gate(byte) {
                record.tile_x = origin_x as u8;
                record.tile_y = origin_y as u8;
                record.target_x = origin_x as u8;
                record.target_y = origin_y as u8;
                record.saved_a = byte;
                record.state = OverlayState::SpawnPending as u8;
                return true;
            }
        }
    }

    scan_inactive_window(record, city, rotation)
}

/// Port-only road-actor admission heuristic; `0xaa` is not a recovered PE car-spawn gate.
pub(super) fn port_traffic_actor_spawn_gate(byte: u8) -> bool {
    byte >= PORT_TRAFFIC_ACTOR_SPAWN_THRESHOLD
}

pub(super) fn resolve_scan_origin(
    record: &Win95OverlayRecord,
    table: &[Win95OverlayRecord],
) -> (usize, usize) {
    if record.saved_b < SAVED_B_SLOT_LINK {
        let linked = record.saved_b as usize;
        if let Some(parent) = table.get(linked) {
            return (parent.tile_x as usize, parent.tile_y as usize);
        }
    }
    if record.target_x != 0 || record.target_y != 0 {
        return (record.target_x as usize, record.target_y as usize);
    }
    (record.tile_x as usize, record.tile_y as usize)
}

/// Walk the `±0x20` window around the scan origin (`FUN_00455020` @ `0x455104..0x455160`).
pub(super) fn scan_inactive_window(
    record: &mut Win95OverlayRecord,
    city: &City,
    rotation: u8,
) -> bool {
    let origin_x = record.tile_x as i16;
    let origin_y = record.tile_y as i16;
    let min_x = origin_x - SCAN_RADIUS;
    let max_x = origin_x + SCAN_RADIUS;
    let min_y = origin_y - SCAN_RADIUS;
    let max_y = origin_y + SCAN_RADIUS;

    // PE FUN_00455020 scans x outer / y inner, testing bounds before reading the map.
    let mut x = min_x;
    while x <= max_x {
        let mut y = min_y;
        while y <= max_y {
            if x >= 0 && y >= 0 && x < CITY_WIDTH as i16 && y < CITY_HEIGHT as i16 {
                let ux = x as usize;
                let uy = y as usize;
                let building = rotated_building_id(city.building_at(ux, uy), rotation);
                if building >= BUILDING_OVERLAY_BLOCK {
                    record.target_x = ux as u8;
                    record.target_y = uy as u8;
                    return true;
                }
            }
            y += 1;
        }
        x += 1;
    }
    false
}

/// Height byte `+5` from linked slot or ALTM word (`FUN_00455020` tail @ `0x455359`).
pub(super) fn refresh_overlay_height(
    record: &mut Win95OverlayRecord,
    city: &City,
    table: &[Win95OverlayRecord],
) {
    if record.saved_b < SAVED_B_SLOT_LINK {
        let linked = record.saved_b as usize;
        if let Some(parent) = table.get(linked) {
            record.height = parent.height;
            return;
        }
    }
    let x = record.tile_x as usize;
    let y = record.tile_y as usize;
    if !tile_in_bounds(x, y) {
        return;
    }
    let water_height = city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.raw_bits() & 0x04 != 0);
    let altm_word = altm_packed_word(city, x, y);
    let base = if water_height {
        (altm_word >> 5) & 0x1f
    } else {
        altm_word & 0x1f
    } as u8;
    record.height = base.wrapping_add(record.state);
}

pub(super) fn altm_packed_word(city: &City, x: usize, y: usize) -> u16 {
    let sample = city.altm_at(x, y);
    (sample.land as u16) | ((sample.water as u16) << 5)
}

fn promote_spawn_pending(record: &mut Win95OverlayRecord, city: &mut City, rotation: u8) -> bool {
    let tile_x = record.tile_x as usize;
    let tile_y = record.tile_y as usize;
    if !tile_in_bounds(tile_x, tile_y) {
        clear_traffic_actor(record, city);
        return true;
    }
    let building = rotated_building_id(city.building_at(tile_x, tile_y), rotation);
    if !has_decorative_traffic(building) {
        clear_traffic_actor(record, city);
        return true;
    }
    record.kind = RUST_TRAFFIC_ACTOR_KIND;
    record.lane = lane_index_for_tile(building, tile_x, tile_y) as u8;
    // PE's class-2 spawn path reaches state 3 with the selected subtile
    // already in the record. Keep it through the pending phase instead of
    // snapping the car back to the tile center before activation.
    record.target_x = record.tile_x;
    record.target_y = record.tile_y;
    if record.saved_a == 0 {
        record.saved_a = city.traffic_density_at(tile_x, tile_y).unwrap_or(0);
    }
    record.state = OverlayState::LightActive as u8;
    true
}

fn update_light_actor<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    rotation: u8,
    rng: &mut R,
) -> bool {
    let tile_x = record.tile_x as usize;
    let tile_y = record.tile_y as usize;
    if !tile_in_bounds(tile_x, tile_y) {
        clear_traffic_actor(record, city);
        return true;
    }
    let building = rotated_building_id(city.building_at(tile_x, tile_y), rotation);
    if !has_decorative_traffic(building) {
        clear_traffic_actor(record, city);
        return true;
    }

    let byte = city.traffic_density_at(tile_x, tile_y).unwrap_or(0);
    if byte < MAC_TRAFFIC_VISIBLE_BYTE_MIN {
        clear_traffic_actor(record, city);
        return true;
    }
    if byte >= MAC_TRAFFIC_HEAVY_BYTE_MIN {
        record.state = OverlayState::HeavyActive as u8;
        return true;
    }

    if rng.roll_mod_is_zero(4) {
        record.state = OverlayState::Inactive as u8;
        return true;
    }
    record.lane = (rng.random_u16() & 7) as u8;
    let (lane_dx, lane_dy) = lane_spread_vector(record.lane as usize);
    let target_x = tile_x as i16 + lane_dx as i16;
    let target_y = tile_y as i16 + lane_dy as i16;
    if target_x < 0
        || target_y < 0
        || target_x >= CITY_WIDTH as i16
        || target_y >= CITY_HEIGHT as i16
        || city
            .traffic_density_at(target_x as usize, target_y as usize)
            .unwrap_or(0)
            >= MAC_TRAFFIC_HEAVY_BYTE_MIN
    {
        return true;
    }
    step_actor(record, building, city, rotation, rng);
    true
}

fn update_heavy_actor<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    rotation: u8,
    rng: &mut R,
) -> bool {
    let tile_x = record.tile_x as usize;
    let tile_y = record.tile_y as usize;
    if !tile_in_bounds(tile_x, tile_y) {
        clear_traffic_actor(record, city);
        return true;
    }
    let building = rotated_building_id(city.building_at(tile_x, tile_y), rotation);
    if !has_decorative_traffic(building) {
        clear_traffic_actor(record, city);
        return true;
    }

    let byte = city.traffic_density_at(tile_x, tile_y).unwrap_or(0);
    if byte < MAC_TRAFFIC_VISIBLE_BYTE_MIN {
        clear_traffic_actor(record, city);
        return true;
    }
    if byte < MAC_TRAFFIC_HEAVY_BYTE_MIN {
        record.state = OverlayState::LightActive as u8;
        return true;
    }

    step_actor(record, building, city, rotation, rng);
    if record.state == OverlayState::HeavyActive as u8 {
        step_actor(record, building, city, rotation, rng);
    }
    true
}

pub(super) fn step_actor<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    building: u8,
    city: &mut City,
    rotation: u8,
    rng: &mut R,
) {
    let old_x = record.tile_x as usize;
    let old_y = record.tile_y as usize;
    let lane = record.lane as usize;
    let step = road_placement_scale(building).max(1) as i16;
    let (lx, ly) = lane_spread_vector(lane);
    let dx = if lx == 0.0 {
        0
    } else {
        lx.signum() as i16 * step
    };
    let dy = if ly == 0.0 {
        0
    } else {
        ly.signum() as i16 * step
    };

    let (sub_x, tile_dx) = advance_subtile(record.sub_x as i16 + dx);
    let (sub_y, tile_dy) = advance_subtile(record.sub_y as i16 + dy);
    let new_x = record.tile_x as i16 + tile_dx;
    let new_y = record.tile_y as i16 + tile_dy;

    if new_x < 0 || new_y < 0 || new_x >= CITY_WIDTH as i16 || new_y >= CITY_HEIGHT as i16 {
        record.lane = turn_lane(building, old_x, old_y, lane, rng);
        return;
    }

    let nx = new_x as usize;
    let ny = new_y as usize;
    if city.traffic_density_at(nx, ny).unwrap_or(0) >= MAC_TRAFFIC_HEAVY_BYTE_MIN {
        // Heavy dest: clear kind like `FUN_00401528` → `FUN_004507D0`.
        // PE restores old XTXT occupancy; road-decoration must not invent an XTRF restore.
        record.tile_x = new_x as u8;
        record.tile_y = new_y as u8;
        reset_traffic_actor_pe(record, city);
        return;
    }
    let next_building = rotated_building_id(city.building_at(nx, ny), rotation);
    if !has_decorative_traffic(next_building) {
        record.lane = turn_lane(building, old_x, old_y, lane, rng);
        return;
    }

    if nx != old_x || ny != old_y {
        cross_tile_byte_handoff(city, record, old_x, old_y, nx, ny);
    }

    record.sub_x = sub_x as u8;
    record.sub_y = sub_y as u8;
    record.tile_x = new_x as u8;
    record.tile_y = new_y as u8;
    record.lane = lane_index_for_tile(next_building, nx, ny) as u8;
    record.target_x = record.tile_x;
    record.target_y = record.tile_y;
}

/// Snapshot arriving congestion into `saved_a`.
/// PE `FUN_004516F0` restores departing XTXT occupancy; road-decoration never writes
/// that plane and must not invent an XTRF restore on departure.
pub(super) fn cross_tile_byte_handoff(
    city: &mut City,
    record: &mut Win95OverlayRecord,
    _old_x: usize,
    _old_y: usize,
    new_x: usize,
    new_y: usize,
) {
    let new_byte = city.traffic_density_at(new_x, new_y).unwrap_or(0);
    record.saved_a = new_byte;
}

fn turn_lane<R: Win95OverlayRng>(
    building: u8,
    x: usize,
    y: usize,
    current: usize,
    rng: &mut R,
) -> u8 {
    let base = lane_index_for_tile(building, x, y);
    if rng.random_bit() {
        ((base + 2) & 7) as u8
    } else {
        ((current + 2) & 7) as u8
    }
}

pub(super) fn traffic_spawn_free_slot(city: &City) -> Option<usize> {
    city.traffic_overlays().and_then(pe_overlay_spawn_slot)
}

pub(super) fn try_spawn_overlays<R: Win95OverlayRng>(
    city: &mut City,
    rotation: u8,
    rng: &mut R,
) -> bool {
    if traffic_spawn_free_slot(city).is_none() {
        return false;
    }
    if !city.has_traffic_density_map() {
        return false;
    }

    let mut spawned = false;
    for _ in 0..SPAWN_TILES_PER_TICK {
        let x = rng.random_i16().unsigned_abs() as usize % CITY_WIDTH;
        let y = rng.random_i16().unsigned_abs() as usize % CITY_HEIGHT;
        let building = rotated_building_id(city.building_at(x, y), rotation);
        if !is_spawnable_traffic_road(building) && !has_decorative_traffic(building) {
            continue;
        }
        let byte = city.traffic_density_at(x, y).unwrap_or(0);
        if byte < PORT_TRAFFIC_ACTOR_SPAWN_THRESHOLD {
            continue;
        }
        if !rng.random_bit() {
            continue;
        }
        if spawn_traffic_actor(city, x, y, building, byte) {
            spawned = true;
        }
    }
    spawned
}

pub(super) fn spawn_traffic_actor(
    city: &mut City,
    x: usize,
    y: usize,
    building: u8,
    saved_byte: u8,
) -> bool {
    let Some(slot) = traffic_spawn_free_slot(city) else {
        return false;
    };
    let Some(table) = city.traffic_overlays_mut() else {
        return false;
    };
    table[slot] = Win95OverlayRecord {
        kind: RUST_TRAFFIC_ACTOR_KIND,
        lane: lane_index_for_tile(building, x, y) as u8,
        state: OverlayState::SpawnPending as u8,
        tile_x: x as u8,
        tile_y: y as u8,
        sub_x: SUBTILE_CENTER,
        sub_y: SUBTILE_CENTER,
        target_x: x as u8,
        target_y: y as u8,
        saved_a: saved_byte,
        saved_b: 0,
        height: 0,
    };
    // Synthetic actor creation leaves its XTRF input unchanged.
    true
}

pub(super) fn clear_traffic_actor(record: &mut Win95OverlayRecord, city: &mut City) {
    let _ = city;
    // Soft cleanup: FUN_004507D0 clears kind (and XTXT for XTXT-occupancy actors).
    // Road-decoration slots never write XTXT and must not invent an XTRF restore.
    pe_clear_xthg_kind(record);
}

/// Road-decoration analog of Win95 `FUN_00401528` → `FUN_004507D0`: clear XTHG
/// kind only. PE also zeros XTXT (`0x4CA600`) for XTXT-occupancy actors; synthetic
/// road-decoration slots never write that plane, and must not invent an XTRF clear.
/// Distinct from ordinary movement cleanup, which restores `saved_a`.
pub(super) fn reset_traffic_actor_pe(record: &mut Win95OverlayRecord, _city: &mut City) {
    // PE clears only the record kind; height and saved_b remain for
    // the linked-slot tail and until the slot is explicitly recycled.
    record.kind = 0;
}
fn is_overlay_road_building(building: u8) -> bool {
    building < BUILDING_OVERLAY_BLOCK && has_decorative_traffic(building)
}
