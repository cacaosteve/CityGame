//! Disaster entry, initial map/actor seeding and pending start effects.
//!
//! Eligibility gates, process random-stream order and saved-state writes retain
//! their existing behavior. Schedulers, ongoing damage/actor ticking, UI routing
//! and sound playback remain with their existing owners.

// Win95 start entry points covered here:
// type 1 @ `0x45e920`: city-center jitter, then one fire ignite.
// type 2 @ `0x45db00`: XTER `0x20..0x30` Chebyshev search, then flood marker `0xfc`.
// type 0x12 @ `0x45e7d0`: LFSR-selected empty XTXT, XTHG + proxy.
// type 6 @ `0x45eb20`: ±32 1-in-64 earthquake scatter.
// types 7/8: XTHG + XTXT `slot-0x37`; type 0x10: coast-edge hurricane marker.
// type 9 @ `0x45f2b0`: `si`/`di` `-0x20..=0x20` scatter from the plant.

use super::*;

mod pending;
mod seed;
mod selection;
pub(super) use selection::*;

/// Win95 `0x4c93a8`: flood/hurricane duration. Start thunks write `0x3c`; `0x45f760` decrements.
pub(super) const PE_FLOOD_HURRICANE_DURATION: u16 = 0x3c;
/// Win95 `0x4e86b0`: hurricane start writes `0x32`; `0x45f760` decrements after the map scan.
pub(super) const PE_HURRICANE_WIND_REMAINING: u16 = 0x32;

/// Win95 `0x4E9908` months-before-ambient-disaster, indexed by `wCityDifficulty`.
const PE_AMBIENT_DISASTER_MIN_MONTHS: [u32; 4] = [0, 100, 60, 30];

/// Win95 type-0x12 tile selection window and retry bound.
pub(super) const PE_AIRCRASH_INNER_ORIGIN: usize = 0x20;
pub(super) const PE_AIRCRASH_INNER_MASK: u16 = 0x3f;
pub(super) const PE_AIRCRASH_SEARCH_LIMIT: usize = 0x100;
/// Win95 type-0xA start @ `0x45f5c0`: `bx = 0x27` walk steps from XBLD `0xcd`.
pub(super) const PE_MICROWAVE_WALK_STEPS: usize = 0x27;
/// Win95 `0x4019ec` during the walk when remaining `bx % 10 == 0` (not on the plant).
pub(super) const PE_MICROWAVE_GOTO_PERIOD: usize = 10;
/// Win95 `0x4e8808` / `0x4e8818` 8-way step tables indexed by `rand&7`.
const PE_MICROWAVE_WALK_DX: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
const PE_MICROWAVE_WALK_DY: [isize; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

/// Win95 volcano start @ `0x45e2a0`: near-arm jitter and marker, then a far scatter.
/// Flatten failure subtracts `0x3e8`; success subtracts `0x19` and raises ALTM.
/// The far arm uses `rand&0x1f-0x10`, `0xfb` if XBIT bit 2 else `0xff`.
const PE_VOLCANO_SCATTER_FUNDS: u32 = 0x61a8;
const PE_VOLCANO_SCATTER_STEP: u32 = 0x3e8;
pub(super) const PE_VOLCANO_SCATTER_COUNT: usize =
    (PE_VOLCANO_SCATTER_FUNDS / PE_VOLCANO_SCATTER_STEP) as usize;
const PE_VOLCANO_LOOP_GUARD: usize = 0x800;
pub(super) const PE_MELTDOWN_SCATTER_RADIUS: isize = 0x20;

/// Win95 `0x45e2a0` dummy-funds seed.
fn pe_volcano_scatter_seed(city: &City, x: usize, y: usize) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 type-0xB @ `0x45e2a0`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream each dummy-funds step.
    let mut seed = 0x45E2_A000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(super) fn pe_volcano_near_scatter_offset(rng: &mut Win95CrtRand) -> (isize, isize) {
    let dx = (rng.random_u16() % 5) as isize - 2;
    let dy = (rng.random_u16() % 5) as isize - 2;
    (dx, dy)
}

pub(super) fn pe_volcano_near_marker(rng: &mut Win95CrtRand) -> u8 {
    if rng.random_u16() & 1 != 0 {
        0xff
    } else {
        0xfb
    }
}

pub(super) fn pe_volcano_far_scatter_offset(rng: &mut Win95CrtRand) -> (isize, isize) {
    let dx = (rng.random_u16() & 0x1f) as isize - 0x10;
    let dy = (rng.random_u16() & 0x1f) as isize - 0x10;
    (dx, dy)
}

/// Win95 type-0xB start @ `0x45e2a0`: dummy-funds near/far scatter plus flatten.
pub(super) fn pe_volcano_overlays(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    pe_volcano_scatter(city, origin_x, origin_y).0
}

pub(super) fn pe_volcano_overlays_with_rng(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
) -> std::collections::HashMap<(usize, usize), u8> {
    pe_volcano_scatter_with_rng(city, origin_x, origin_y, rng).0
}

pub(super) fn pe_volcano_explode_count(city: &City, origin_x: usize, origin_y: usize) -> usize {
    pe_volcano_scatter(&mut city.clone(), origin_x, origin_y).1
}

/// Win95 `0x45e2a0`: after each far scatter, `rand&7 != 0` plays explode `0x1f8`.
pub(super) fn pe_volcano_scatter_with_rng(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
) -> (std::collections::HashMap<(usize, usize), u8>, usize) {
    let mut tiles = std::collections::HashMap::new();
    let mut funds = PE_VOLCANO_SCATTER_FUNDS;
    let mut guard = 0;
    let mut explodes = 0usize;
    while funds > 0 && guard < PE_VOLCANO_LOOP_GUARD {
        guard += 1;
        let (dx, dy) = pe_volcano_near_scatter_offset(rng);
        let Some((near_x, near_y)) = offset_city_tile(origin_x, origin_y, dx, dy) else {
            continue;
        };
        let near_marker = pe_volcano_near_marker(rng);
        city.set_text_overlay(near_x, near_y, near_marker);
        tiles.insert((near_x, near_y), near_marker);
        if pe_volcano_flatten_check(city, near_x, near_y) {
            let _ = pe_volcano_apply_flatten_with_crt(city, Some(rng), near_x, near_y, &mut funds);
        } else {
            funds = funds.saturating_sub(PE_VOLCANO_SCATTER_STEP);
        }
        let (fdx, fdy) = pe_volcano_far_scatter_offset(rng);
        if let Some((far_x, far_y)) = offset_city_tile(origin_x, origin_y, fdx, fdy) {
            let far_marker = pe_volcano_far_marker(city, far_x, far_y);
            city.set_text_overlay(far_x, far_y, far_marker);
            tiles.insert((far_x, far_y), far_marker);
        }
        if rng.random_u16() & 7 != 0 {
            explodes += 1;
        }
    }
    (tiles, explodes)
}

fn pe_volcano_scatter(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
) -> (std::collections::HashMap<(usize, usize), u8>, usize) {
    let mut rng = Win95CrtRand::new(pe_volcano_scatter_seed(city, origin_x, origin_y));
    pe_volcano_scatter_with_rng(city, origin_x, origin_y, &mut rng)
}

fn pe_volcano_far_marker(city: &City, x: usize, y: usize) -> u8 {
    if city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
    {
        0xfb
    } else {
        0xff
    }
}

impl ViewerState {
    pub(super) fn start_disaster(&mut self, kind: DisasterKind) -> bool {
        self.start_disaster_at(kind, self.disaster_target_tile())
    }

    pub(super) fn start_ambient_disaster_if_due(
        &mut self,
        population: PopulationStats,
        outcomes: CityServiceOutcomes,
    ) -> bool {
        // This is the Win95 day-22 direct-start path. Mac CODE 14 phase 24
        // calls CODE 7's separate proposal worker, which queues ffffd604;
        // CODE 2's pending/timed calls to CODE 10 export 246 are statically
        // rooted, but native Mac action/actor integration and session RNG
        // chronology are not. Do not route Mac through this seeded PE fallback.
        if !self.engine_fidelity.uses_win95() {
            return false;
        }
        if self.no_disasters_enabled || self.city.has_active_disaster() {
            return false;
        }
        // Peek process LFSR for Win95 air-crash tile proposal; seed_pe_aircrash
        // consumes the live stream once after kind selection.
        let mut peek_lfsr = self.pe_process_lfsr;
        let candidate = ambient_disaster_for_month_with_rng(
            &self.city,
            self.game_clock,
            population,
            outcomes,
            EngineFidelity::Win95,
            &mut self.pe_process_crt,
            Some(&mut peek_lfsr),
        );
        let Some((kind, (x, y))) = candidate else {
            return false;
        };
        self.queue_pe_disaster_start_effects(kind, (x, y));
        self.seed_disaster_footprint(kind, x, y);
        self.city.mark_all_dirty();
        self.query_is_inspect = false;
        self.set_sim_status_announcement(format!("{} Reported", kind.label()));
        true
    }

    pub(super) fn disaster_target_tile(&self) -> (usize, usize) {
        self.hover_tile.unwrap_or((CITY_WIDTH / 2, CITY_HEIGHT / 2))
    }

    pub(super) fn start_disaster_at(&mut self, kind: DisasterKind, (x, y): (usize, usize)) -> bool {
        // Manual Mac actions belong to its native pending-selector/action-mode
        // worker. Until that lifecycle is integrated, do not write a PE trigger
        // or run PE start workers under Mac fidelity.
        if !self.engine_fidelity.uses_win95() {
            return false;
        }
        if self.no_disasters_enabled {
            return false;
        }
        self.close_tool_menu();
        self.close_modal_dialogs();
        self.cancel_active_drag();
        let trigger = disaster_kind_misc_type(kind);
        self.city.pe_w_trigger_disaster = trigger as u16;
        if pe_misc_mirror_storage_active(&self.city, self.engine_fidelity) {
            self.city
                .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, trigger);
        }
        self.consume_pe_trigger_disaster_at((x, y))
    }

    /// Win95 `0x45cf40`: consume `wSetTriggerDisasterType` and start that disaster.
    pub(super) fn consume_pe_trigger_disaster(&mut self) -> bool {
        if !self.engine_fidelity.uses_win95() {
            // Mac `ffffd604` is its native pending-action selector, whose
            // exported consumer/cadence is not the PE trigger-disaster poll.
            return false;
        }
        self.consume_pe_trigger_disaster_at(self.disaster_target_tile())
    }

    fn consume_pe_trigger_disaster_at(&mut self, (x, y): (usize, usize)) -> bool {
        // PE `0x45cf40` reads/clears BSS word `wSetTriggerDisasterType` @ `0x4ca420`.
        let trigger = u32::from(self.city.pe_w_trigger_disaster);
        self.city.pe_w_trigger_disaster = 0;
        if pe_misc_mirror_storage_active(&self.city, self.engine_fidelity) {
            self.city.set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0);
        }
        if trigger == 0 || self.no_disasters_enabled || self.city.has_active_disaster() {
            return false;
        }
        let Some(kind) = scenario_disaster_kind(u16::try_from(trigger).unwrap_or(0)) else {
            return false;
        };
        if kind == DisasterKind::Meltdown && pe_meltdown_plant_tile(&self.city, (x, y)).is_none() {
            return false;
        }
        if kind == DisasterKind::Microwave && pe_microwave_plant_tile(&self.city).is_none() {
            return false;
        }
        if kind == DisasterKind::Rioters && pe_rioters_target_tile(&self.city, x, y).is_none() {
            return false;
        }
        if kind == DisasterKind::MassRiot && {
            let mut peek = self.pe_process_crt;
            !pe_mass_riot_attempt_ok_with_rng(&self.city, x, y, &mut peek)
        } {
            return false;
        }
        if kind == DisasterKind::Flood && pe_flood_source_tile(&self.city, x, y).is_none() && {
            let mut peek = self.pe_process_lfsr;
            pe_flood_overlays_with_lfsr(&self.city, x, y, &mut peek).is_empty()
        } {
            return false;
        }
        if kind == DisasterKind::BlizzardFlood && {
            let mut peek_crt = self.pe_process_crt;
            let mut peek_lfsr = self.pe_process_lfsr;
            !pe_blizzard_flood_attempt_ok_with_rng(&self.city, x, y, &mut peek_crt, &mut peek_lfsr)
        } {
            return false;
        }
        if kind == DisasterKind::Fire && {
            let mut peek_crt = self.pe_process_crt;
            let mut peek_lfsr = self.pe_process_lfsr;
            pe_fire_target_tile_with_rng(&self.city, &mut peek_crt, &mut peek_lfsr).is_none()
        } {
            return false;
        }
        if matches!(
            kind,
            DisasterKind::AirCrash | DisasterKind::Tornado | DisasterKind::Monster
        ) && !pe_xthg_disaster_slot_available(&self.city)
        {
            return false;
        }
        if kind == DisasterKind::AirCrash && {
            let mut peek = self.pe_process_lfsr;
            pe_aircrash_tile_with_lfsr(&self.city, &mut peek).is_none()
        } {
            return false;
        }
        self.queue_pe_disaster_start_effects(kind, (x, y));
        self.seed_disaster_footprint(kind, x, y);
        self.city.mark_all_dirty();
        self.query_is_inspect = false;
        self.set_sim_status_announcement(format!("{} Reported", kind.label()));
        true
    }

    pub(super) fn seed_loaded_scenario_disaster(&mut self) -> Option<DisasterKind> {
        if self.city.kind != CityFileKind::WinScenario || self.city.has_active_disaster() {
            return None;
        }
        let goals = scenario_goals_for_city(&self.city)?;
        let kind = scenario_disaster_kind(goals.disaster_type)?;
        let x = usize::from(goals.disaster_x);
        let y = usize::from(goals.disaster_y);
        if kind == DisasterKind::Meltdown && pe_meltdown_plant_tile(&self.city, (x, y)).is_none() {
            return None;
        }
        if kind == DisasterKind::Microwave && pe_microwave_plant_tile(&self.city).is_none() {
            return None;
        }
        if kind == DisasterKind::Rioters && pe_rioters_target_tile(&self.city, x, y).is_none() {
            return None;
        }
        if kind == DisasterKind::MassRiot && {
            let mut peek = self.pe_process_crt;
            !pe_mass_riot_attempt_ok_with_rng(&self.city, x, y, &mut peek)
        } {
            return None;
        }
        if kind == DisasterKind::Flood && pe_flood_source_tile(&self.city, x, y).is_none() && {
            let mut peek = self.pe_process_lfsr;
            pe_flood_overlays_with_lfsr(&self.city, x, y, &mut peek).is_empty()
        } {
            return None;
        }
        if kind == DisasterKind::BlizzardFlood && {
            let mut peek_crt = self.pe_process_crt;
            let mut peek_lfsr = self.pe_process_lfsr;
            !pe_blizzard_flood_attempt_ok_with_rng(&self.city, x, y, &mut peek_crt, &mut peek_lfsr)
        } {
            return None;
        }
        if kind == DisasterKind::Fire && {
            let mut peek_crt = self.pe_process_crt;
            let mut peek_lfsr = self.pe_process_lfsr;
            pe_fire_target_tile_with_rng(&self.city, &mut peek_crt, &mut peek_lfsr).is_none()
        } {
            return None;
        }
        if matches!(
            kind,
            DisasterKind::AirCrash | DisasterKind::Tornado | DisasterKind::Monster
        ) && !pe_xthg_disaster_slot_available(&self.city)
        {
            return None;
        }
        if kind == DisasterKind::AirCrash && {
            let mut peek = self.pe_process_lfsr;
            pe_aircrash_tile_with_lfsr(&self.city, &mut peek).is_none()
        } {
            return None;
        }
        self.queue_pe_disaster_start_effects(kind, (x, y));
        self.seed_disaster_footprint(kind, x, y);
        self.city.mark_all_dirty();
        self.query_is_inspect = false;
        self.set_sim_status_announcement(format!("{} Reported", kind.label()));
        Some(kind)
    }

    pub(super) fn start_weather_disaster_if_due(&mut self) -> bool {
        if self.no_disasters_enabled || self.city.has_active_disaster() {
            return false;
        }
        let trend = pe_weather_trend(&self.city);
        let kind = match trend {
            WEATHER_TREND_HURRICANE if self.edit_map_option_enabled(EDIT_MAP_COAST_MISC_OFFSET) => {
                Some(DisasterKind::Hurricane)
            }
            WEATHER_TREND_TORNADO => Some(DisasterKind::Tornado),
            _ => None,
        };
        let Some(kind) = kind else {
            return false;
        };
        // PE FUN_00471bc0 / weather arms draw CRT `rand()` (`0x48cdd8`).
        let mut rng = self.pe_process_crt;
        if !weather_disaster_roll_succeeds(&mut rng) {
            self.pe_process_crt = rng;
            return false;
        }
        let started = self.begin_weather_disaster(kind, &mut rng);
        self.pe_process_crt = rng;
        started
    }

    fn begin_weather_disaster(&mut self, kind: DisasterKind, rng: &mut Win95CrtRand) -> bool {
        let (x, y) = ambient_disaster_target_tile(&self.city, rng);
        self.queue_pe_disaster_start_effects(kind, (x, y));
        self.seed_disaster_footprint(kind, x, y);
        self.city.mark_all_dirty();
        self.query_is_inspect = false;
        self.set_sim_status_announcement(format!("{} Reported", kind.label()));
        true
    }

    #[cfg(test)]
    pub(super) fn start_weather_disaster_for_test(&mut self) -> bool {
        let trend = pe_weather_trend(&self.city);
        let kind = match trend {
            WEATHER_TREND_HURRICANE => DisasterKind::Hurricane,
            WEATHER_TREND_TORNADO => DisasterKind::Tornado,
            _ => return false,
        };
        let mut rng = Win95CrtRand::new(1);
        self.begin_weather_disaster(kind, &mut rng)
    }
}
