//! Experimental, test-only model of the Mac 68k 40-slot actor table (`fffffd4c`).
//!
//! The recovered record layout, `0xC9 + slot` markers, and some kind-1 update
//! gates are real. The road-following movement, XTRF-based admission, car art,
//! bootstrap behavior, and fixed-rate scheduler below are hypotheses from an
//! earlier traffic experiment; the disassembly does not establish kind 1 as a
//! road vehicle or connect it to XTRF road traffic. The rooted kind-1 creator
//! is now narrowed to XBLD `0xdd` runway cells, which suggests an aircraft-like
//! actor but still does not validate this road-vehicle experiment. Do not treat
//! these tests as validation of native Mac road traffic or use this model in
//! the app.
//!
//! A relocation-aware CODE/CREL census roots export 109 in CODE 2's repeating
//! speed/TickCount-gated loop, export 110 in CODE 12 draw-helper families, and
//! export 114 in CODE 8's monthly map worker. These roots establish lifecycle
//! entry points, not this harness's vehicle identity or synthetic behavior.
//! The updater's external launch and exact frequency remain unresolved. This
//! module is compiled only for tests, not runtime actor simulation. Ordinary
//! road traffic uses the supported Win95/XTRF blue palette-mask overlay for
//! every city format and graphics mode.

use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};
use crate::mac_random::MacToolboxRng;
use crate::mac_traffic_geometry::mac_direction_to_tile;
use crate::traffic::{GameSpeed, MAC_TRAFFIC_VISIBLE_BYTE_MIN, has_decorative_traffic};
use crate::win95_traffic_placement::is_east_west_road;

const SLOT_COUNT: usize = 40;
const FIRST_ACTIVE_SLOT: usize = 1;
const SLOT_KIND_TRAFFIC: u8 = 1;
const MAX_KIND1_LIVE_SLOTS: usize = SLOT_COUNT - FIRST_ACTIVE_SLOT;
const MAX_KIND1_SPAWNS_PER_TICK: usize = 2;
const OCCUPANCY_MARKER_BASE: u8 = 0xC9;
const SUBTILE_GRID: i16 = 16;
const SUBTILE_CENTER: i16 = 8;
/// Hypothetical lane inset used only by this unverified test experiment.
const RIGHT_LANE_INSET: i16 = 4;
/// Synthetic test cadence; the original Mac CODE 2/TickCount path does not
/// establish a fixed ticks-per-second value.
const MAC_LIVE_TRAFFIC_TICKS_PER_SECOND: f32 = 4.0;

const DIRECTION_DELTA: [(i16, i16); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// The 12-byte record shape recovered at `fffffd4c`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MacTrafficSlot {
    kind: u8,
    direction: u8,
    phase: u8,
    tile_x: u8,
    tile_y: u8,
    height: u8,
    sub_x: i16,
    sub_y: i16,
    target_x: u8,
    target_y: u8,
    saved_byte: u8,
    saved_b: u8,
}

/// Hypothetical vehicle-like draw record emitted by the test experiment.
///
/// Neither its road-tile sprite mapping nor car identity is established by the
/// Mac binary; it is not consumed by CityGame's renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveDraw {
    pub tile_x: usize,
    pub tile_y: usize,
    pub sub_x: u8,
    pub sub_y: u8,
    pub direction: u8,
    pub blit_flip: bool,
}

pub struct MacLiveTraffic {
    slots: [MacTrafficSlot; SLOT_COUNT],
    /// Runtime-only `0xC9 + slot` occupancy markers; saved XTRF bytes are never mutated.
    occupancy: Vec<u8>,
    seeded_key: Option<String>,
    tick_accumulator: f32,
    scan_cursor: usize,
    global_counter: u16,
    /// Mirrors the spawn throttle observed around `ffffa092`: cleared by `export_109`.
    kind1_spawns_this_tick: usize,
    rng: MacToolboxRng,
}

impl Default for MacLiveTraffic {
    fn default() -> Self {
        Self::new()
    }
}

impl MacLiveTraffic {
    pub fn new() -> Self {
        Self {
            slots: [MacTrafficSlot::default(); SLOT_COUNT],
            occupancy: vec![0; CITY_WIDTH * CITY_HEIGHT],
            seeded_key: None,
            tick_accumulator: 0.0,
            scan_cursor: 0,
            global_counter: 0,
            kind1_spawns_this_tick: 0,
            rng: MacToolboxRng::new(0x0684_B109),
        }
    }

    pub fn clear(&mut self) {
        self.slots = [MacTrafficSlot::default(); SLOT_COUNT];
        self.occupancy.fill(0);
        self.seeded_key = None;
        self.tick_accumulator = 0.0;
        self.scan_cursor = 0;
        self.global_counter = 0;
        self.kind1_spawns_this_tick = 0;
    }

    /// (Re)initialize the runtime slot table for a city if needed.
    pub fn ensure_seeded(&mut self, city: &City, congestion: impl Fn(usize, usize) -> u8) {
        let key = format!("{}:{}", city.name, city.rotation());
        if self.seeded_key.as_deref() != Some(key.as_str()) {
            self.reset_for_city(city, key);
            self.bootstrap_spawns(city, &congestion);
        }
    }

    pub fn seed(&mut self, city: &City, congestion: impl Fn(usize, usize) -> u8) {
        let key = format!("{}:{}", city.name, city.rotation());
        self.reset_for_city(city, key);
        self.bootstrap_spawns(city, congestion);
    }

    /// Advance the recovered kind-1 traffic slots.
    pub fn tick(
        &mut self,
        city: &City,
        congestion: impl Fn(usize, usize) -> u8,
        speed: GameSpeed,
        delta_seconds: f32,
    ) {
        let multiplier = speed.tick_multiplier();
        if multiplier <= 0.0 {
            return;
        }

        self.tick_accumulator += delta_seconds * multiplier * MAC_LIVE_TRAFFIC_TICKS_PER_SECOND;
        while self.tick_accumulator >= 1.0 {
            self.tick_accumulator -= 1.0;
            self.tick_once(city, &congestion);
        }
    }

    pub fn draws(&self) -> impl Iterator<Item = LiveDraw> + '_ {
        self.slots
            .iter()
            .skip(FIRST_ACTIVE_SLOT)
            .filter(|slot| slot.kind == SLOT_KIND_TRAFFIC)
            .map(|slot| {
                let direction = slot.direction & 7;
                let (rx, ry) = right_lane_offset(direction);
                LiveDraw {
                    tile_x: slot.tile_x as usize,
                    tile_y: slot.tile_y as usize,
                    sub_x: (slot.sub_x + rx).clamp(0, SUBTILE_GRID - 1) as u8,
                    sub_y: (slot.sub_y + ry).clamp(0, SUBTILE_GRID - 1) as u8,
                    direction,
                    blit_flip: car_facing_flip(direction),
                }
            })
    }

    pub fn active_count(&self) -> usize {
        self.active_kind1_count()
    }

    fn reset_for_city(&mut self, city: &City, key: String) {
        self.clear();
        let seed = city.name.bytes().fold(0x4D_41_43_u32, |acc, b| {
            acc.wrapping_mul(33).wrapping_add(u32::from(b))
        }) ^ (u32::from(city.rotation()) << 24);
        self.rng = MacToolboxRng::new(seed);
        self.seeded_key = Some(key);
    }

    fn bootstrap_spawns(&mut self, city: &City, congestion: impl Fn(usize, usize) -> u8) {
        while self.active_kind1_count() < MAX_KIND1_LIVE_SLOTS {
            if !self.spawn_kind1(city, &congestion) {
                break;
            }
        }
        self.kind1_spawns_this_tick = 0;
    }

    fn tick_once(&mut self, city: &City, congestion: &impl Fn(usize, usize) -> u8) {
        self.global_counter = (self.global_counter + 1) & 0x03ff;
        self.kind1_spawns_this_tick = 0;

        for slot_index in FIRST_ACTIVE_SLOT..SLOT_COUNT {
            if self.slots[slot_index].kind != SLOT_KIND_TRAFFIC {
                continue;
            }

            // Kind-1 traffic moves on roughly half of ticks (`Random` bit test at 0xd02a).
            if self.rng.random_bit() {
                self.move_slot(city, congestion, slot_index);
            }
        }

        while self.active_kind1_count() < MAX_KIND1_LIVE_SLOTS {
            if !self.try_spawn_kind1(city, congestion) {
                break;
            }
        }
    }

    fn try_spawn_kind1(&mut self, city: &City, congestion: &impl Fn(usize, usize) -> u8) -> bool {
        if self.kind1_spawns_this_tick >= MAX_KIND1_SPAWNS_PER_TICK {
            return false;
        }
        if !self.spawn_kind1(city, congestion) {
            return false;
        }
        self.kind1_spawns_this_tick += 1;
        true
    }

    fn spawn_kind1(&mut self, city: &City, congestion: &impl Fn(usize, usize) -> u8) -> bool {
        let Some(slot_index) = self.free_slot_index() else {
            return false;
        };

        for _ in 0..(CITY_WIDTH * CITY_HEIGHT) {
            let cursor = self.scan_cursor % (CITY_WIDTH * CITY_HEIGHT);
            self.scan_cursor = (self.scan_cursor + 1) % (CITY_WIDTH * CITY_HEIGHT);
            let x = cursor % CITY_WIDTH;
            let y = cursor / CITY_WIDTH;
            if !self.can_spawn_at(city, congestion, x, y) {
                continue;
            }
            let building = city.building_at(x, y);
            let (target_x, target_y) = choose_target_tile(x, y, building);
            let mut direction = mac_direction_to_tile(x, y, target_x, target_y);
            direction = steer_direction_one_step(direction, x, y, target_x, target_y);
            let saved_byte = congestion(x, y);
            let slot = MacTrafficSlot {
                kind: SLOT_KIND_TRAFFIC,
                direction,
                phase: 0,
                tile_x: x as u8,
                tile_y: y as u8,
                height: city.structure_elevation_at(x, y),
                sub_x: SUBTILE_CENTER,
                sub_y: SUBTILE_CENTER,
                target_x: target_x as u8,
                target_y: target_y as u8,
                saved_byte,
                saved_b: 0,
            };
            self.slots[slot_index] = slot;
            self.occupancy[city_index(x, y)] = occupancy_marker(slot_index);
            return true;
        }

        false
    }

    fn can_spawn_at(
        &self,
        city: &City,
        congestion: &impl Fn(usize, usize) -> u8,
        x: usize,
        y: usize,
    ) -> bool {
        driveable_traffic_tile(city, congestion, x, y)
            && self.occupancy[city_index(x, y)] < OCCUPANCY_MARKER_BASE
    }

    fn move_slot(
        &mut self,
        city: &City,
        congestion: &impl Fn(usize, usize) -> u8,
        slot_index: usize,
    ) {
        let old_slot = self.slots[slot_index];
        let (dx, dy) = DIRECTION_DELTA[usize::from(old_slot.direction & 7)];
        let (sub_x, tile_dx) = advance_subtile(old_slot.sub_x + dx);
        let (sub_y, tile_dy) = advance_subtile(old_slot.sub_y + dy);

        if tile_dx == 0 && tile_dy == 0 {
            self.slots[slot_index].sub_x = sub_x;
            self.slots[slot_index].sub_y = sub_y;
            return;
        }

        let old_x = old_slot.tile_x as usize;
        let old_y = old_slot.tile_y as usize;

        let Some((next_direction, probe_x, probe_y)) =
            self.choose_next_road_direction(city, congestion, old_x, old_y, old_slot.direction)
        else {
            self.reverse_slot(slot_index);
            return;
        };
        let (entry_sub_x, entry_sub_y) = entry_subtile_for_direction(next_direction);

        self.occupancy[city_index(old_x, old_y)] = 0;
        self.slots[slot_index].saved_byte = congestion(probe_x, probe_y);
        self.occupancy[city_index(probe_x, probe_y)] = occupancy_marker(slot_index);
        self.slots[slot_index].tile_x = probe_x as u8;
        self.slots[slot_index].tile_y = probe_y as u8;
        self.slots[slot_index].height = city.structure_elevation_at(probe_x, probe_y);
        self.slots[slot_index].sub_x = entry_sub_x;
        self.slots[slot_index].sub_y = entry_sub_y;
        self.slots[slot_index].direction = steer_direction_one_step(
            next_direction,
            probe_x,
            probe_y,
            old_slot.target_x as usize,
            old_slot.target_y as usize,
        );
    }

    fn choose_next_road_direction(
        &self,
        city: &City,
        congestion: &impl Fn(usize, usize) -> u8,
        x: usize,
        y: usize,
        current_direction: u8,
    ) -> Option<(u8, usize, usize)> {
        for direction in candidate_road_directions(current_direction) {
            let (dx, dy) = DIRECTION_DELTA[usize::from(direction)];
            let Some(nx) = add_signed(x, dx) else {
                continue;
            };
            let Some(ny) = add_signed(y, dy) else {
                continue;
            };
            if tile_in_bounds(nx, ny)
                && driveable_traffic_tile(city, congestion, nx, ny)
                && self.occupancy[city_index(nx, ny)] < OCCUPANCY_MARKER_BASE
            {
                return Some((direction, nx, ny));
            }
        }
        None
    }

    fn reverse_slot(&mut self, slot_index: usize) {
        let slot = &mut self.slots[slot_index];
        let direction = (slot.direction + 4) & 7;
        slot.direction = direction;
        slot.sub_x = SUBTILE_CENTER;
        slot.sub_y = SUBTILE_CENTER;
        let (target_x, target_y) =
            target_for_direction(slot.tile_x as usize, slot.tile_y as usize, direction);
        slot.target_x = target_x as u8;
        slot.target_y = target_y as u8;
    }

    fn kill_slot(&mut self, slot_index: usize) {
        let slot = self.slots[slot_index];
        if slot.kind != 0 {
            let x = slot.tile_x as usize;
            let y = slot.tile_y as usize;
            if tile_in_bounds(x, y)
                && self.occupancy[city_index(x, y)] == occupancy_marker(slot_index)
            {
                self.occupancy[city_index(x, y)] = 0;
            }
        }
        self.slots[slot_index] = MacTrafficSlot::default();
    }

    fn active_kind1_count(&self) -> usize {
        self.slots
            .iter()
            .skip(FIRST_ACTIVE_SLOT)
            .filter(|slot| slot.kind == SLOT_KIND_TRAFFIC)
            .count()
    }

    fn free_slot_index(&self) -> Option<usize> {
        (FIRST_ACTIVE_SLOT..SLOT_COUNT).find(|&slot_index| self.slots[slot_index].kind == 0)
    }
}

/// Right-hand lane bias perpendicular to travel. In screen/tile coords (x right, y down) the
/// right of heading `(dx, dy)` is `(-dy, dx)`, so cars hug the right lane of their direction.
fn right_lane_offset(direction: u8) -> (i16, i16) {
    let (dx, dy) = DIRECTION_DELTA[usize::from(direction & 7)];
    (-dy * RIGHT_LANE_INSET, dx * RIGHT_LANE_INSET)
}

/// Only a horizontal blit flip exists, so mirror the oriented car shape for westbound travel.
fn car_facing_flip(direction: u8) -> bool {
    matches!(direction & 7, 5..=7)
}

fn advance_subtile(mut sub: i16) -> (i16, i16) {
    let mut tile_delta = 0;
    while sub > SUBTILE_GRID {
        sub -= SUBTILE_GRID;
        tile_delta += 1;
    }
    while sub < 0 {
        sub += SUBTILE_GRID;
        tile_delta -= 1;
    }
    (sub, tile_delta)
}

fn choose_target_tile(x: usize, y: usize, building: u8) -> (usize, usize) {
    if is_east_west_road(building) {
        if x < CITY_WIDTH / 2 {
            (CITY_WIDTH - 1, y)
        } else {
            (0, y)
        }
    } else if y < CITY_HEIGHT / 2 {
        (x, CITY_HEIGHT - 1)
    } else {
        (x, 0)
    }
}

fn target_for_direction(x: usize, y: usize, direction: u8) -> (usize, usize) {
    let (dx, dy) = DIRECTION_DELTA[usize::from(direction & 7)];
    let target_x = if dx < 0 {
        0
    } else if dx > 0 {
        CITY_WIDTH - 1
    } else {
        x
    };
    let target_y = if dy < 0 {
        0
    } else if dy > 0 {
        CITY_HEIGHT - 1
    } else {
        y
    };
    (target_x, target_y)
}

fn candidate_road_directions(direction: u8) -> [u8; 4] {
    match direction & 7 {
        0 | 1 | 7 => [0, 2, 6, 0],
        2 | 3 => [2, 0, 4, 2],
        4 | 5 => [4, 2, 6, 4],
        6 => [6, 0, 4, 6],
        _ => [2, 0, 4, 2],
    }
}

fn entry_subtile_for_direction(direction: u8) -> (i16, i16) {
    match direction & 7 {
        0 => (SUBTILE_CENTER, SUBTILE_GRID - 1),
        2 => (1, SUBTILE_CENTER),
        4 => (SUBTILE_CENTER, 1),
        6 => (SUBTILE_GRID - 1, SUBTILE_CENTER),
        _ => (SUBTILE_CENTER, SUBTILE_CENTER),
    }
}

fn driveable_traffic_tile(
    city: &City,
    congestion: &impl Fn(usize, usize) -> u8,
    x: usize,
    y: usize,
) -> bool {
    let building = city.building_at(x, y);
    has_decorative_traffic(building) && congestion(x, y) >= MAC_TRAFFIC_VISIBLE_BYTE_MIN
}

fn steer_direction_one_step(
    current: u8,
    x: usize,
    y: usize,
    target_x: usize,
    target_y: usize,
) -> u8 {
    let desired = mac_direction_to_tile(x, y, target_x, target_y);
    if current == desired {
        return current & 7;
    }
    let current = current & 7;
    let desired = desired & 7;
    if current > desired {
        let diff = current - desired;
        if diff > 4 {
            (current + 1) & 7
        } else {
            current.wrapping_sub(1) & 7
        }
    } else {
        let diff = desired - current;
        if diff > 4 {
            current.wrapping_sub(1) & 7
        } else {
            (current + 1) & 7
        }
    }
}

fn manhattan_distance_to_tile(x: usize, y: usize, target_x: usize, target_y: usize) -> usize {
    x.abs_diff(target_x) + y.abs_diff(target_y)
}

fn add_signed(value: usize, delta: i16) -> Option<usize> {
    if delta < 0 {
        value.checked_sub(delta.unsigned_abs() as usize)
    } else {
        value.checked_add(delta as usize)
    }
}

fn tile_in_bounds(x: usize, y: usize) -> bool {
    x < CITY_WIDTH && y < CITY_HEIGHT
}

fn occupancy_marker(slot_index: usize) -> u8 {
    OCCUPANCY_MARKER_BASE.saturating_add(slot_index as u8)
}

fn city_index(x: usize, y: usize) -> usize {
    y * CITY_WIDTH + x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::City;

    fn straight_ew_city() -> City {
        let mut city = City::new_blank("/tmp/sc2000-live-traffic.sc2", "live").expect("blank city");
        for x in 4..80 {
            city.set_building(x, 20, 29);
            city.set_traffic_density(x, 20, 0xC0);
        }
        city
    }

    #[test]
    fn ports_forty_slot_table_with_slot_zero_unused_and_live_kind1_slots() {
        let city = straight_ew_city();
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));

        assert_eq!(live.slots[0].kind, 0);
        assert_eq!(live.active_count(), MAX_KIND1_LIVE_SLOTS);
        assert_eq!(live.draws().count(), MAX_KIND1_LIVE_SLOTS);
    }

    #[test]
    fn mac_visible_traffic_below_win95_spawn_threshold_still_seeds_live_slots() {
        let mut city =
            City::new_blank("/tmp/sc2000-live-visible-traffic.sc2", "live").expect("blank city");
        for x in 4..80 {
            city.set_building(x, 20, 29);
            city.set_traffic_density(x, 20, MAC_TRAFFIC_VISIBLE_BYTE_MIN);
        }
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));

        assert_eq!(live.active_count(), MAX_KIND1_LIVE_SLOTS);
        assert_eq!(live.draws().count(), MAX_KIND1_LIVE_SLOTS);
    }

    #[test]
    fn mac_heavy_traffic_bytes_are_not_confused_with_runtime_occupancy_markers() {
        let mut city =
            City::new_blank("/tmp/sc2000-live-heavy-traffic.sc2", "live").expect("blank city");
        for x in 4..80 {
            city.set_building(x, 20, 29);
            city.set_traffic_density(x, 20, 0xF0);
        }
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));

        assert_eq!(live.active_count(), MAX_KIND1_LIVE_SLOTS);
        assert_eq!(live.draws().count(), MAX_KIND1_LIVE_SLOTS);
    }

    #[test]
    fn spawned_slots_start_at_subtile_center_and_mark_occupancy() {
        let city = straight_ew_city();
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));
        for slot_index in FIRST_ACTIVE_SLOT..SLOT_COUNT {
            let slot = live.slots[slot_index];
            assert_eq!(slot.kind, SLOT_KIND_TRAFFIC);
            assert_eq!(slot.sub_x, SUBTILE_CENTER);
            assert_eq!(slot.sub_y, SUBTILE_CENTER);
            assert_eq!(
                live.occupancy[city_index(slot.tile_x as usize, slot.tile_y as usize)],
                occupancy_marker(slot_index)
            );
        }
    }

    #[test]
    fn kind1_spawn_gate_is_a_refill_throttle_not_an_active_slot_cap() {
        let city = straight_ew_city();
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));
        assert_eq!(live.active_count(), MAX_KIND1_LIVE_SLOTS);

        live.kill_slot(FIRST_ACTIVE_SLOT);
        live.kill_slot(FIRST_ACTIVE_SLOT + 1);
        live.kill_slot(FIRST_ACTIVE_SLOT + 2);
        assert_eq!(live.active_count(), MAX_KIND1_LIVE_SLOTS - 3);

        live.kind1_spawns_this_tick = MAX_KIND1_SPAWNS_PER_TICK;
        assert!(!live.try_spawn_kind1(&city, &|x, y| {
            city.traffic_density_at(x, y).unwrap_or(0)
        }));

        live.kind1_spawns_this_tick = 0;
        assert!(live.try_spawn_kind1(&city, &|x, y| {
            city.traffic_density_at(x, y).unwrap_or(0)
        }));
        assert!(live.try_spawn_kind1(&city, &|x, y| {
            city.traffic_density_at(x, y).unwrap_or(0)
        }));
        assert!(!live.try_spawn_kind1(&city, &|x, y| {
            city.traffic_density_at(x, y).unwrap_or(0)
        }));
        assert_eq!(live.kind1_spawns_this_tick, MAX_KIND1_SPAWNS_PER_TICK);
        assert_eq!(live.active_count(), MAX_KIND1_LIVE_SLOTS - 1);
    }

    #[test]
    fn actors_move_on_subtile_grid_over_time() {
        let city = straight_ew_city();
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));
        let before: Vec<_> = live
            .draws()
            .map(|d| (d.tile_x, d.tile_y, d.sub_x, d.sub_y))
            .collect();
        for _ in 0..120 {
            live.tick(
                &city,
                |x, y| city.traffic_density_at(x, y).unwrap_or(0),
                GameSpeed::Llama,
                1.0 / 60.0,
            );
        }
        let after: Vec<_> = live
            .draws()
            .map(|d| (d.tile_x, d.tile_y, d.sub_x, d.sub_y))
            .collect();
        assert_ne!(before, after);
    }

    #[test]
    fn live_draw_facing_is_stable_while_driving_straight() {
        let city = straight_ew_city();
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));
        let before: Vec<_> = live.draws().map(|d| (d.direction, d.blit_flip)).collect();
        for _ in 0..60 {
            live.tick(
                &city,
                |x, y| city.traffic_density_at(x, y).unwrap_or(0),
                GameSpeed::Llama,
                1.0 / 60.0,
            );
        }
        let after: Vec<_> = live.draws().map(|d| (d.direction, d.blit_flip)).collect();

        assert_eq!(before, after);
    }

    #[test]
    fn right_lane_offset_pushes_cars_to_the_right_of_travel() {
        // East-bound (dir 2) right side is +y (south); west-bound (dir 6) right side is -y (north).
        assert_eq!(right_lane_offset(2), (0, RIGHT_LANE_INSET));
        assert_eq!(right_lane_offset(6), (0, -RIGHT_LANE_INSET));
        // North-bound (dir 0) right side is +x (east); south-bound (dir 4) right side is -x (west).
        assert_eq!(right_lane_offset(0), (RIGHT_LANE_INSET, 0));
        assert_eq!(right_lane_offset(4), (-RIGHT_LANE_INSET, 0));
    }

    #[test]
    fn car_facing_flip_mirrors_westbound_travel() {
        assert!(!car_facing_flip(0));
        assert!(!car_facing_flip(2));
        assert!(car_facing_flip(5));
        assert!(car_facing_flip(6));
        assert!(car_facing_flip(7));
    }

    #[test]
    fn live_draws_sit_on_the_right_lane_not_the_tile_centre() {
        let city = straight_ew_city();
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));
        // Every car is biased off the sub-tile centre (8,8) onto its right lane.
        for draw in live.draws() {
            assert_ne!((draw.sub_x, draw.sub_y), (8, 8));
        }
    }

    #[test]
    fn direction_helper_matches_recovered_b41e_octants() {
        assert_eq!(mac_direction_to_tile(10, 10, 10, 0), 0);
        assert_eq!(mac_direction_to_tile(10, 10, 20, 0), 1);
        assert_eq!(mac_direction_to_tile(10, 10, 20, 10), 2);
        assert_eq!(mac_direction_to_tile(10, 10, 20, 20), 3);
        assert_eq!(mac_direction_to_tile(10, 10, 10, 20), 4);
        assert_eq!(mac_direction_to_tile(10, 10, 0, 20), 5);
        assert_eq!(mac_direction_to_tile(10, 10, 0, 10), 6);
        assert_eq!(mac_direction_to_tile(10, 10, 0, 0), 7);
    }

    #[test]
    fn steering_helper_turns_one_octant_toward_target() {
        assert_eq!(steer_direction_one_step(0, 10, 10, 20, 10), 1);
        assert_eq!(steer_direction_one_step(7, 10, 10, 20, 10), 0);
        assert_eq!(steer_direction_one_step(2, 10, 10, 20, 10), 2);
        assert_eq!(manhattan_distance_to_tile(10, 10, 13, 14), 7);
    }

    #[test]
    fn subtile_wrap_occurs_after_sixteen_not_at_sixteen() {
        assert_eq!(advance_subtile(16), (16, 0));
        assert_eq!(advance_subtile(17), (1, 1));
        assert_eq!(advance_subtile(-1), (15, -1));
    }

    #[test]
    fn slots_restore_runtime_occupancy_on_cross_tile_handoff() {
        let city = straight_ew_city();
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));
        let slot_index = FIRST_ACTIVE_SLOT;
        let old_x = live.slots[slot_index].tile_x as usize;
        let old_y = live.slots[slot_index].tile_y as usize;
        let target_x = old_x + 1;
        live.slots[2].kind = 0;
        live.occupancy[city_index(target_x, old_y)] = 0;
        live.slots[slot_index].direction = 2;
        live.slots[slot_index].sub_x = 16;
        live.slots[slot_index].sub_y = 8;

        live.move_slot(
            &city,
            &|x, y| city.traffic_density_at(x, y).unwrap_or(0),
            slot_index,
        );

        assert_eq!(live.occupancy[city_index(old_x, old_y)], 0);
        let new_x = live.slots[slot_index].tile_x as usize;
        let new_y = live.slots[slot_index].tile_y as usize;
        assert_eq!(
            live.occupancy[city_index(new_x, new_y)],
            occupancy_marker(slot_index)
        );
    }

    #[test]
    fn rotated_view_still_moves_along_unrotated_road_axis() {
        let mut city = straight_ew_city();
        city.set_rotation(1);
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));
        let slot_index = FIRST_ACTIVE_SLOT;
        live.occupancy.fill(0);
        live.slots[slot_index] = MacTrafficSlot {
            kind: SLOT_KIND_TRAFFIC,
            direction: 2,
            phase: 0,
            tile_x: 10,
            tile_y: 20,
            height: city.structure_elevation_at(10, 20),
            sub_x: 16,
            sub_y: 8,
            target_x: (CITY_WIDTH - 1) as u8,
            target_y: 20,
            saved_byte: city.traffic_density_at(10, 20).unwrap_or(0),
            saved_b: 0,
        };
        live.occupancy[city_index(10, 20)] = occupancy_marker(slot_index);

        live.move_slot(
            &city,
            &|x, y| city.traffic_density_at(x, y).unwrap_or(0),
            slot_index,
        );

        assert_eq!(live.slots[slot_index].tile_x, 11);
        assert_eq!(live.slots[slot_index].tile_y, 20);
    }

    #[test]
    fn road_end_reverses_instead_of_entering_dirt() {
        let city = straight_ew_city();
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));
        let slot_index = FIRST_ACTIVE_SLOT;
        let old_x = 79;
        let old_y = 20;
        live.occupancy.fill(0);
        live.slots[slot_index] = MacTrafficSlot {
            kind: SLOT_KIND_TRAFFIC,
            direction: 2,
            phase: 0,
            tile_x: old_x as u8,
            tile_y: old_y as u8,
            height: city.structure_elevation_at(old_x, old_y),
            sub_x: 16,
            sub_y: 8,
            target_x: (CITY_WIDTH - 1) as u8,
            target_y: old_y as u8,
            saved_byte: city.traffic_density_at(old_x, old_y).unwrap_or(0),
            saved_b: 0,
        };
        live.occupancy[city_index(old_x, old_y)] = occupancy_marker(slot_index);

        live.move_slot(
            &city,
            &|x, y| city.traffic_density_at(x, y).unwrap_or(0),
            slot_index,
        );

        assert_eq!(live.slots[slot_index].tile_x as usize, old_x);
        assert_eq!(live.slots[slot_index].tile_y as usize, old_y);
        assert_eq!(live.slots[slot_index].direction, 6);
        assert_eq!(live.slots[slot_index].sub_x, SUBTILE_CENTER);
        assert_eq!(
            live.occupancy[city_index(old_x, old_y)],
            occupancy_marker(slot_index)
        );
    }

    #[test]
    fn paused_speed_does_not_move_slots() {
        let city = straight_ew_city();
        let mut live = MacLiveTraffic::new();
        live.seed(&city, |x, y| city.traffic_density_at(x, y).unwrap_or(0));
        let before: Vec<_> = live
            .draws()
            .map(|d| (d.tile_x, d.tile_y, d.sub_x, d.sub_y))
            .collect();
        for _ in 0..60 {
            live.tick(
                &city,
                |x, y| city.traffic_density_at(x, y).unwrap_or(0),
                GameSpeed::Paused,
                1.0 / 60.0,
            );
        }
        let after: Vec<_> = live
            .draws()
            .map(|d| (d.tile_x, d.tile_y, d.sub_x, d.sub_y))
            .collect();
        assert_eq!(before, after);
    }
}
