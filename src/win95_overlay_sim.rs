//! CityGame's Win95-style overlay actor adapters for native `XTHG` records and
//! port-only road actors. The native PE dispatcher is `FUN_00450890`; CityGame
//! currently joins selected disaster, train, and Maxis Man handlers into a
//! slot-ordered pass; other ambient actor families still use separate adapters.
//!
//! Synthetic road actors use runtime kind `0xD0` and the port's XTRF threshold
//! heuristic; the PE `0xAA` comparison belongs to its kind-2 helicopter path.
//! Ambient port/transit things include planes, ships, trains, and sailboats.
//! Native PE kind 6 is an explosion handled by `FUN_004546F0`; the `0x4554F0`
//! return stub is kind 7, disabled in the native dispatch mask. Kind 16 is
//! Maxis Man (`FUN_00455020`). Engine selection controls this adapter; the
//! saved city container only controls record decoding.

#[cfg(test)]
use crate::city::CityFileKind;
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};
use crate::mac_random::MacToolboxRng;
use crate::traffic::GameSpeed;
#[cfg(test)]
use crate::traffic::MAC_TRAFFIC_HEAVY_BYTE_MIN;
#[cfg(test)]
use crate::win95_overlay::{
    OVERLAY_KIND_TRAFFIC, OverlayState, XTHG_SUBWAY_TRAIN_CAR, XTHG_TRAIN_CAR,
};
use crate::win95_overlay::{
    PeMaxisManState, Win95OverlayRecord, XTHG_AIRPLANE, XTHG_CARGO_SHIP, XTHG_EXPLOSION,
    XTHG_EXPLOSION_XTXT_GATE, XTHG_FIRST_USABLE_SLOT, XTHG_HELICOPTER, XTHG_RECORD_COUNT,
    XTHG_SAILBOAT, XTHG_SUBWAY_TRAIN_ENGINE, XTHG_TRAIN_ENGINE, xthg_proxy_for_slot,
};
use crate::win95_random::{Win95CrtRand, Win95GameRand, Win95Lfsr};

mod maxis_man;
#[cfg(test)]
use crate::win95_overlay::XTHG_MAXIS_MAN;
#[cfg(test)]
use maxis_man::{move_maxis_man, update_maxis_man_light, update_maxis_man_search};
mod ambient_tick;

mod ambient_actor;
#[cfg(test)]
use ambient_actor::ambient_subtile_step;
use ambient_actor::step_ambient_actor;

mod road_traffic;
#[cfg(test)]
use road_traffic::{
    BUILDING_OVERLAY_BLOCK, altm_packed_word, clear_traffic_actor, cross_tile_byte_handoff,
    port_traffic_actor_spawn_gate, reset_traffic_actor_pe, resolve_scan_origin,
    scan_inactive_window, spawn_traffic_actor, step_actor, traffic_spawn_free_slot,
    update_inactive_slot,
};
use road_traffic::{
    SAVED_B_SLOT_LINK, SCAN_RADIUS, refresh_overlay_height, try_spawn_overlays, update_slot,
};

mod ambient_routing;
pub use ambient_routing::ambient_spawn_direction;
use ambient_routing::{
    ambient_actor_can_occupy, ambient_legal_exits, pick_ambient_turn, tile_is_water_surface,
    watercraft_can_occupy,
};

mod pe_train_routing;
pub use pe_train_routing::ambient_train_spawn_direction;
#[cfg(test)]
use pe_train_routing::{PE_TRAIN_DRAW_CODES, pick_pe_train_fallback};
use pe_train_routing::{
    pe_train_compass_direction, pe_train_draw_code, pick_pe_train_fallback_legacy,
    pick_pe_train_route, subway_can_occupy, subway_track_at, sync_train_kind_to_tile,
    tile_is_rail_network, train_can_occupy, train_family_can_occupy, train_track_at,
};

mod pe_train_consist;
#[cfg(test)]
use pe_train_consist::pe_train_target_xtext_is_open;
#[cfg(test)]
use pe_train_consist::pe_xthg_copy_train_follower;
#[cfg(test)]
use pe_train_consist::step_pe_train_engine_slot;
use pe_train_consist::{
    TrainEngineStep, pe_train_consist_slots, pe_train_ed_neighbor_defers_tick,
    pe_train_merge_finish_explosion, pe_train_merge_tile_eligible, reverse_pe_train_consist,
    step_ambient_train_car, step_ambient_train_engine,
};

mod cargo_ship;
use cargo_ship::{
    cargo_ship_can_occupy, first_cargo_ship_position, pe_cargo_direction_to_target,
    step_pe_cargo_actor_mut, step_pe_cargo_actor_with_streams,
};
#[cfg(test)]
use cargo_ship::{
    pe_cargo_candidate_in_direction, step_pe_cargo_movement, step_pe_cargo_movement_mut,
    update_pe_cargo_state_four, update_pe_cargo_state_one, update_pe_cargo_state_three,
    update_pe_cargo_state_two, update_pe_cargo_state_zero,
};

mod pe_aircraft_sailboat;
#[cfg(test)]
use pe_aircraft_sailboat::pe_sailboat_subtile_step;
use pe_aircraft_sailboat::{
    pe_heli_avoid_large_building, pe_heli_state2_after_move, pe_plane_absolute_direction_to_target,
    pe_plane_maybe_jitter_dir, pe_plane_state0_after_move, pe_plane_state1_after_move,
    pe_plane_state3_after_move, pe_plane_state4_after_move, step_pe_sailboat,
};

pub const SUBTILE_GRID: i16 = 16;
pub const SUBTILE_CENTER: u8 = 8;

/// PE `FUN_00403071` type `0x10` uses `0x004E81F8[0x10] == 0x10` subtiles.
const MAXIS_MAN_SUBTILE_STEP: i16 = 16;
/// Port-side baseline for overlay ticks at Llama speed; Mac `TrafficSystem` uses its own rate (`×4.0`).
/// PE `FUN_00407490` instead gates `SimulationDoThingTick` on a GetTickCount deadline, choosing
/// 30/45 ms from adaptive state and adding 30 ms in controller state 2. This fixed-rate accumulator
/// does not reproduce that controller-dependent PE cadence. `FUN_00479b90` is a separate 60s timer.
pub const WIN95_OVERLAY_SIM_TICKS_PER_SECOND: f32 = 30.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PeAircraftSlotTickStats {
    pub changed: bool,
    /// PE `FUN_00401096(0x203)` from helicopter state 5 when `+5 == 4`.
    pub mayday_sounds: usize,
}

trait Win95OverlayRng {
    fn random_i16(&mut self) -> i16;
    fn random_u16(&mut self) -> u16;
    fn random_bit(&mut self) -> bool;
    fn roll_mod_is_zero(&mut self, modulus: u16) -> bool;
}

impl Win95OverlayRng for MacToolboxRng {
    fn random_i16(&mut self) -> i16 {
        self.random_i16()
    }

    fn random_u16(&mut self) -> u16 {
        self.random_u16()
    }

    fn random_bit(&mut self) -> bool {
        self.random_bit()
    }

    fn roll_mod_is_zero(&mut self, modulus: u16) -> bool {
        self.roll_mod_is_zero(modulus)
    }
}

impl Win95OverlayRng for Win95CrtRand {
    fn random_i16(&mut self) -> i16 {
        self.random_u16() as i16
    }

    fn random_u16(&mut self) -> u16 {
        self.random_u16()
    }

    fn random_bit(&mut self) -> bool {
        self.random_u16() & 1 != 0
    }

    fn roll_mod_is_zero(&mut self, modulus: u16) -> bool {
        self.random_mod(modulus) == 0
    }
}

impl Win95OverlayRng for Win95Lfsr {
    fn random_i16(&mut self) -> i16 {
        self.random_mod(0x8000) as i16
    }

    fn random_u16(&mut self) -> u16 {
        // Full-width word after one LFSR advance (Game_Random prelude).
        let _ = self.random_mod(1);
        self.state()
    }

    fn random_bit(&mut self) -> bool {
        self.random_mod(2) != 0
    }

    fn roll_mod_is_zero(&mut self, modulus: u16) -> bool {
        self.random_mod(modulus) == 0
    }
}

#[derive(Debug, Clone)]
pub struct Win95OverlaySim {
    tick_accumulator: f32,
    ambient_tick_accumulator: f32,
    sim_rng: Win95CrtRand,
    /// PE cargo `0x401096(0x205)` plays queued since the last [`Self::take_cargo_sfx_plays`].
    cargo_sfx_plays: usize,
}

impl Default for Win95OverlaySim {
    fn default() -> Self {
        Self::new()
    }
}

impl Win95OverlaySim {
    pub fn new() -> Self {
        Self {
            tick_accumulator: 0.0,
            ambient_tick_accumulator: 0.0,
            sim_rng: Win95CrtRand::new(0x9500_0EAF),
            cargo_sfx_plays: 0,
        }
    }

    pub fn reset_for_city(&mut self, city: &City) {
        let seed = city.name.bytes().fold(0x95_05_u32, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u32)
        });
        self.sim_rng = Win95CrtRand::new(seed);
        self.tick_accumulator = 0.0;
        self.ambient_tick_accumulator = 0.0;
        self.cargo_sfx_plays = 0;
    }

    /// Drain queued cargo Things-tick SFX plays (`0x205`).
    pub fn take_cargo_sfx_plays(&mut self) -> usize {
        let plays = self.cargo_sfx_plays;
        self.cargo_sfx_plays = 0;
        plays
    }

    /// Advance overlay actors when the city has an `XTHG` table.
    ///
    /// Ambient train ED-neighbor defer (`FUN_00455500`) draws process LFSR+game-LCG;
    /// road-decoration spawn/update draw process CRT (city-name `sim_rng` remains
    /// for reset archaeology only).
    pub fn tick(
        &mut self,
        city: &mut City,
        speed: GameSpeed,
        delta_seconds: f32,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) -> bool {
        let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
        self.tick_with_game_rand(city, speed, delta_seconds, crt, lfsr, &mut game_rand)
    }

    /// PE live path with the shared process game-LCG (`0x40116d`) stream.
    pub fn tick_with_game_rand(
        &mut self,
        city: &mut City,
        speed: GameSpeed,
        delta_seconds: f32,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
    ) -> bool {
        self.tick_with_game_rand_policy(city, speed, delta_seconds, crt, lfsr, game_rand, false)
    }

    /// Win95 live path when the app joins extracted PE actors to their native
    /// positions in the selected XTHG slot walk.
    pub fn tick_with_game_rand_defer_xthg_slot_actors(
        &mut self,
        city: &mut City,
        speed: GameSpeed,
        delta_seconds: f32,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
    ) -> bool {
        self.tick_with_game_rand_policy(city, speed, delta_seconds, crt, lfsr, game_rand, true)
    }

    fn tick_with_game_rand_policy(
        &mut self,
        city: &mut City,
        speed: GameSpeed,
        delta_seconds: f32,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
        defer_slot_actors: bool,
    ) -> bool {
        if !city.has_traffic_overlays() {
            return false;
        }
        let multiplier = speed.tick_multiplier();
        if multiplier <= 0.0 {
            return false;
        }
        self.tick_accumulator += delta_seconds * multiplier * WIN95_OVERLAY_SIM_TICKS_PER_SECOND;
        let mut changed = false;
        while self.tick_accumulator >= 1.0 {
            self.tick_accumulator -= 1.0;
            changed |= self.tick_once_with_game_rand(city, crt, lfsr, game_rand, defer_slot_actors);
        }
        changed
    }

    pub fn sim_rng_seed(&self) -> u32 {
        self.sim_rng.seed()
    }

    fn tick_once(&mut self, city: &mut City, crt: &mut Win95CrtRand, lfsr: &mut Win95Lfsr) -> bool {
        let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
        self.tick_once_with_game_rand(city, crt, lfsr, &mut game_rand, false)
    }

    fn tick_once_with_game_rand(
        &mut self,
        city: &mut City,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
        game_rand: &mut Win95GameRand,
        defer_slot_actors: bool,
    ) -> bool {
        let rotation = city.rotation();
        let table_snapshot = city
            .traffic_overlays()
            .map(|table| table.to_vec())
            .unwrap_or_default();
        let mut changed = false;
        // The native `FUN_00450890` pass starts its XTHG slot walk at 1; slot 0 is reserved.
        // Port-only road-actor updates share the process CRT stream with its spawn probes.
        for slot in XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT {
            if update_slot(city, slot, &table_snapshot, rotation, crt) {
                changed = true;
            }
        }
        // Keep ambient motion in sync when the full Win95 traffic sim is enabled.
        changed |= self.tick_ambient_once_with_game_rand_policy(
            city,
            crt,
            lfsr,
            game_rand,
            defer_slot_actors,
        );
        // Port-only road-actor probes draw the process CRT, not city-name archaeology.
        if try_spawn_overlays(city, rotation, crt) {
            changed = true;
        }
        if changed {
            city.mark_traffic_layers_dirty();
        }
        changed
    }
}

/// PE `FUN_004507D0` / thunk `0x401528`: zero XTHG kind and the tile's `0x4CA600` byte.
fn clear_xthg_slot_pe(city: &mut City, slot: usize) {
    if !(XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT).contains(&slot) {
        return;
    }
    let Some(record) = city
        .traffic_overlays()
        .and_then(|table| table.get(slot).copied())
    else {
        return;
    };
    let x = record.tile_x as usize;
    let y = record.tile_y as usize;
    if let Some(overlays) = city.traffic_overlays_mut() {
        overlays[slot].kind = 0;
    }
    if tile_in_bounds(x, y) {
        city.set_text_overlay(x, y, 0);
        city.mark_render_dirty(x, y);
    }
}

/// Compass delta for Win95 `XTHG` directions (`lane` byte on ambient actors).
pub use crate::win95_overlay::xthg_direction_delta;

pub(super) fn advance_subtile(mut sub: i16) -> (i16, i16) {
    let mut tile_delta = 0i16;
    // PE FUN_004516F0 uses one strict `> 0x10` carry and one `< 0` borrow;
    // it does not normalize repeatedly and leaves an exact 0x10 offset intact.
    if sub > SUBTILE_GRID {
        sub -= SUBTILE_GRID;
        tile_delta += 1;
    }
    if sub < 0 {
        sub += SUBTILE_GRID;
        tile_delta -= 1;
    }
    (sub, tile_delta)
}

/// PE `FUN_004507d0`: zero `0x4CA600` XTXT at the record tile when clearing an
/// occupancy-painting ambient (plane / heli / sailboat).
fn pe_clear_ambient_xtxt_occupancy(city: &mut City, record: &Win95OverlayRecord) {
    if !matches!(record.kind, XTHG_AIRPLANE | XTHG_HELICOPTER | XTHG_SAILBOAT) {
        return;
    }
    let x = record.tile_x as usize;
    let y = record.tile_y as usize;
    if x < CITY_WIDTH && y < CITY_HEIGHT {
        city.set_text_overlay(x, y, 0);
    }
}

/// PE `FUN_00401528` / `FUN_004507D0`: clear XTHG kind only (height/`saved_b` remain).
fn pe_clear_xthg_kind(record: &mut Win95OverlayRecord) {
    record.kind = 0;
}

fn pe_zero_tile_xtxt(city: &mut City, x: usize, y: usize) {
    if x < CITY_WIDTH && y < CITY_HEIGHT {
        city.set_text_overlay(x, y, 0);
    }
}

/// PE `FUN_00451490`: rotate one step along the shorter modulo-8 direction.
fn pe_step_direction_toward(current: u8, target: u8) -> u8 {
    let current = current & 0x07;
    let target = target & 0x07;
    if current == target {
        return current;
    }
    if target > current {
        if target - current > 4 {
            (current + 7) & 0x07
        } else {
            (current + 1) & 0x07
        }
    } else if current - target > 4 {
        (current + 1) & 0x07
    } else {
        (current + 7) & 0x07
    }
}

fn tile_in_bounds(x: usize, y: usize) -> bool {
    x < CITY_WIDTH && y < CITY_HEIGHT
}

/// Public bridge to the shared PE airplane obstruction morph used by flight and disaster paths.
pub(crate) fn pe_plane_morph_on_building_with_lfsr(
    record: &mut Win95OverlayRecord,
    city: &City,
    lfsr: &mut Win95Lfsr,
) -> bool {
    pe_aircraft_sailboat::pe_plane_morph_on_building_with_lfsr(record, city, lfsr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::win95_overlay::{DispatchKind, empty_overlay_table, xthg_spawn_height};

    #[path = "road_traffic.rs"]
    mod road_traffic;

    #[path = "pe_aircraft.rs"]
    mod pe_aircraft;

    #[path = "pe_sailboat.rs"]
    mod pe_sailboat;

    #[path = "pe_cargo_ship.rs"]
    mod pe_cargo_ship;

    #[path = "pe_train_consist.rs"]
    mod pe_train_consist;

    #[path = "train_rendering.rs"]
    mod train_rendering;

    #[path = "pe_train_routing.rs"]
    mod pe_train_routing;

    #[path = "pe_train_lifecycle.rs"]
    mod pe_train_lifecycle;

    #[path = "maxis_man.rs"]
    mod maxis_man;

    #[path = "overlay_core.rs"]
    mod overlay_core;

    #[path = "ambient_spawn.rs"]
    mod ambient_spawn;

    struct ZeroModRng;

    impl Win95OverlayRng for ZeroModRng {
        fn random_i16(&mut self) -> i16 {
            0
        }

        fn random_u16(&mut self) -> u16 {
            0
        }

        fn random_bit(&mut self) -> bool {
            false
        }

        fn roll_mod_is_zero(&mut self, _modulus: u16) -> bool {
            true
        }
    }

    struct NonZeroLowRng;

    impl Win95OverlayRng for NonZeroLowRng {
        fn random_i16(&mut self) -> i16 {
            1
        }

        fn random_u16(&mut self) -> u16 {
            1
        }

        fn random_bit(&mut self) -> bool {
            true
        }

        fn roll_mod_is_zero(&mut self, _modulus: u16) -> bool {
            false
        }
    }
}
