//! Military-base site selection, coastal/silo models and placement stamping.
//!
//! Attempt/scan order, process RNG use, compatibility fallbacks, map/census writes
//! and auto-goto centers retain their existing behavior. Offer/progression UI and
//! native RCI growth retain their owners and shared stamping primitives.

use super::{
    AIRPORT_RUNWAY_CROSS_BUILDING_ID, EDIT_MAP_COAST_MISC_OFFSET, MILITARY_BASE_AIR_FORCE,
    MILITARY_BASE_ARMY, MILITARY_BASE_MISSILE_SILOS, MILITARY_BASE_NAVY, NETWORK_BOTTOM,
    NETWORK_LEFT, NETWORK_RIGHT, NETWORK_TOP, NetworkKind, RUNWAY_BUILDING_ID,
    SMALL_PARK_BUILDING_ID, ViewerState, ZONE_MILITARY, is_saltwater_tile, is_water_terrain,
    neighbor_tiles, network_building_for_mask, pe_army_after_runway_dd_zone_4384e6,
    pe_army_form_strip_de_flag, pe_army_missile_silo_place_plot_438def,
    pe_army_runway_dd_to_de_43857b, pe_fun_004392a0_clear_before_runway, pe_place_tile_441f00,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City, ZONE_CORNER_ALL};
use crate::win95_random::Win95GameRand;
use std::collections::HashSet;

mod mac68k;
pub(super) use mac68k::place_mac_military_base;

pub(super) const MILITARY_BASE_PLOT_SIZE: usize = 8;

pub(super) const MILITARY_BASE_MIN_VALID_TILES: usize = 40;

pub(super) const MILITARY_SILO_PLOT_SIZE: usize = 3;

pub(super) const MILITARY_MAX_SILOS: usize = 6;

pub(super) const MILITARY_BASE_FALLBACK_PATCH_COUNT: usize = 6;

const MILITARY_SILO_MIN_ALTITUDE: u8 = 6;

const MILITARY_NAVAL_MIN_COAST_RUN: usize = 12;

const MILITARY_NAVAL_ZONE_DEPTH: usize = 4;

pub(super) const MILITARY_NAVAL_ZONE_LENGTH: usize = 10;

/// PE military zone stamp: clear small XBLD (`!= 0 && < SMALL_PARK`) without census;
/// leave XTXT/XTRF/watered. Used after `pe_stamp_military_zone_tile_4144d4` so the
/// pre-clear XBLD is what left `dwTileCount`. Silo plots still clear via PlaceTile.
pub(super) fn pe_clear_small_xbld_for_military_zone(city: &mut City, x: usize, y: usize) {
    let building = city.building_at(x, y);
    if building != 0 && building < SMALL_PARK_BUILDING_ID {
        city.set_building(x, y, 0);
    }
}

/// PE military zone stamp census @ `0x4144d4`–`0x414512` (8×8) / twin `0x414714`:
/// `dec dwTileCount[old XBLD]`, XZON write when in bounds, `inc [0x4ca934] word 0`.
pub(super) fn pe_stamp_military_zone_tile_4144d4(city: &mut City, x: usize, y: usize, zone: u8) {
    let old = city.building_at(x, y) as usize;
    city.pe_dw_tile_count[old] = city.pe_dw_tile_count[old].wrapping_sub(1);
    if x < 0x80 && y < 0x80 {
        city.set_zone(x, y, zone);
    }
    city.pe_increment_military_zone_census(old as u8);
}

/// sc2kfix `MILITARY_RETRY_ATTEMPT_MAX` / Hook_SimulationProposeMilitaryBase outer loop.
const MILITARY_RETRY_ATTEMPT_MAX: usize = 10;

/// PE / OpenSC2K / sc2kfix MilitaryBasePlotCheck inner samples (`LCGMod(map_edge-9)`).
const MILITARY_PLOT_SAMPLE_ATTEMPTS: usize = 24;

const MILITARY_BASE_FALLBACK_SAMPLE_ATTEMPTS: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NavalCoastStrip {
    pub(super) start_x: usize,
    pub(super) start_y: usize,
    pub(super) along_dx: isize,
    pub(super) along_dy: isize,
    pub(super) inland_dx: isize,
    pub(super) inland_dy: isize,
    pub(super) run_len: usize,
}

impl ViewerState {
    /// sc2kfix Hook_SimulationProposeMilitaryBase: up to
    /// [`MILITARY_RETRY_ATTEMPT_MAX`] attempts, then force-silo fallback.
    /// Returns `(base_type, AutoGoto center)`.
    pub(super) fn try_place_military_base(&mut self) -> Option<(u32, (usize, usize))> {
        for _ in 0..MILITARY_RETRY_ATTEMPT_MAX {
            if let Some(placed) = self.try_place_military_base_once() {
                return Some(placed);
            }
        }
        // OpenSC2K / our prior fallback: force silos after land siting exhausted.
        if self.try_place_missile_silos(true) {
            return Some((
                MILITARY_BASE_MISSILE_SILOS,
                military_silo_view_center(&self.city),
            ));
        }
        None
    }

    fn try_place_military_base_once(&mut self) -> Option<(u32, (usize, usize))> {
        // Win95 / sc2kfix / OpenSC2K order:
        // naval (50% coin) → plot sample → AF/army when at least 40 tiles match altitude.
        if self.city_has_ocean_for_navy() && self.pe_process_crt.random_mod(2) == 1 {
            if let Some(center) = self.try_place_naval_yard() {
                return Some((MILITARY_BASE_NAVY, center));
            }
        }

        if let Some((origin_x, origin_y, valid, same_altitude)) =
            sample_military_base_plot(&self.city, &mut self.pe_process_game_rand)
        {
            if valid < MILITARY_BASE_MIN_VALID_TILES {
                let altitude = self.city.altitude_at(origin_x, origin_y) & 0x1f;
                if let Some(plots) = sample_pe_military_base_fallback_plots(
                    &self.city,
                    altitude,
                    &mut self.pe_process_game_rand,
                ) {
                    let center = plots.last().copied().unwrap_or((origin_x, origin_y));
                    for &(plot_x, plot_y) in &plots {
                        stamp_pe_military_base_fallback_plot(&mut self.city, plot_x, plot_y);
                    }
                    return Some((MILITARY_BASE_MISSILE_SILOS, center));
                }
                return None;
            }

            let base_type = pe_military_base_plot_type_from_counts(valid, same_altitude)?;
            let center = (
                origin_x + MILITARY_BASE_PLOT_SIZE / 2,
                origin_y + MILITARY_BASE_PLOT_SIZE / 2,
            );
            if base_type == MILITARY_BASE_AIR_FORCE {
                self.stamp_military_base_plot(origin_x, origin_y);
                return Some((MILITARY_BASE_AIR_FORCE, center));
            }
            if base_type == MILITARY_BASE_ARMY {
                self.stamp_military_base_plot(origin_x, origin_y);
                self.stamp_army_base_road_strips(origin_x, origin_y);
                return Some((MILITARY_BASE_ARMY, center));
            }
        }
        None
    }

    fn city_has_ocean_for_navy(&self) -> bool {
        self.edit_map_option_enabled(EDIT_MAP_COAST_MISC_OFFSET)
            || (0..CITY_HEIGHT)
                .any(|y| (0..CITY_WIDTH).any(|x| is_saltwater_tile(&self.city, x, y)))
    }

    fn try_place_naval_yard(&mut self) -> Option<(usize, usize)> {
        if !self.city_has_ocean_for_navy() {
            return None;
        }
        let strip = find_naval_coast_strip(&self.city)?;
        let center = naval_strip_view_center(&strip);
        self.stamp_naval_yard_strip(strip);
        Some(center)
    }

    pub(super) fn stamp_naval_yard_strip(&mut self, strip: NavalCoastStrip) {
        let skip = strip.run_len.saturating_sub(MILITARY_NAVAL_ZONE_LENGTH) / 2;
        for along in skip..skip
            .saturating_add(MILITARY_NAVAL_ZONE_LENGTH)
            .min(strip.run_len)
        {
            let mut x = strip.start_x as isize + strip.along_dx * along as isize;
            let mut y = strip.start_y as isize + strip.along_dy * along as isize;
            if x < 0 || y < 0 || x as usize >= CITY_WIDTH || y as usize >= CITY_HEIGHT {
                continue;
            }
            let mut ux = x as usize;
            let mut uy = y as usize;
            let base_altitude = self.city.altitude_at(ux, uy);
            for _ in 0..MILITARY_NAVAL_ZONE_DEPTH {
                if ux >= CITY_WIDTH || uy >= CITY_HEIGHT {
                    break;
                }
                if military_base_tile_is_candidate(&self.city, ux, uy)
                    && self.city.altitude_at(ux, uy) == base_altitude
                {
                    // PE @ `0x4144d4`: census + XZON first; Rust then clears small XBLD.
                    pe_stamp_military_zone_tile_4144d4(
                        &mut self.city,
                        ux,
                        uy,
                        ZONE_MILITARY | ZONE_CORNER_ALL,
                    );
                    pe_clear_small_xbld_for_military_zone(&mut self.city, ux, uy);
                }
                x = ux as isize + strip.inland_dx;
                y = uy as isize + strip.inland_dy;
                if x < 0 || y < 0 {
                    break;
                }
                ux = x as usize;
                uy = y as usize;
            }
        }
    }

    pub(super) fn try_place_missile_silos(&mut self, force: bool) -> bool {
        let Some(positions) = find_missile_silo_plots(&self.city, force) else {
            return false;
        };
        for &(origin_x, origin_y) in &positions {
            pe_army_missile_silo_place_plot_438def(&mut self.city, origin_x, origin_y);
        }
        true
    }

    pub(super) fn stamp_military_base_plot(&mut self, origin_x: usize, origin_y: usize) {
        for y in origin_y
            ..origin_y
                .saturating_add(MILITARY_BASE_PLOT_SIZE)
                .min(CITY_HEIGHT)
        {
            for x in origin_x
                ..origin_x
                    .saturating_add(MILITARY_BASE_PLOT_SIZE)
                    .min(CITY_WIDTH)
            {
                if !military_base_tile_is_candidate(&self.city, x, y) {
                    continue;
                }
                // PE @ `0x4144d4`–`0x414512`: dwTileCount dec + military word0 inc + XZON.
                pe_stamp_military_zone_tile_4144d4(
                    &mut self.city,
                    x,
                    y,
                    ZONE_MILITARY | ZONE_CORNER_ALL,
                );
                pe_clear_small_xbld_for_military_zone(&mut self.city, x, y);
            }
        }
    }

    fn stamp_army_base_road_strips(&mut self, origin_x: usize, origin_y: usize) {
        let end = MILITARY_BASE_PLOT_SIZE.saturating_sub(1);
        self.form_army_base_strip(origin_x + 2, origin_y, origin_x + 2, origin_y + end);
        self.form_army_base_strip(origin_x + 5, origin_y, origin_x + 5, origin_y + end);
        self.form_army_base_strip(origin_x, origin_y + 2, origin_x + end, origin_y + 2);
        self.form_army_base_strip(origin_x, origin_y + 5, origin_x + end, origin_y + 5);
    }

    pub(super) fn form_army_base_strip(&mut self, x1: usize, y1: usize, x2: usize, y2: usize) {
        let tiles = axis_aligned_tile_line(x1, y1, x2, y2);
        if tiles.is_empty() {
            return;
        }
        let axis_mask = if x1 == x2 {
            NETWORK_TOP | NETWORK_BOTTOM
        } else {
            NETWORK_LEFT | NETWORK_RIGHT
        };
        let mut placed = Vec::new();
        for &(x, y) in &tiles {
            let building = self.city.building_at(x, y);
            if building >= SMALL_PARK_BUILDING_ID {
                continue;
            }
            // PE PlaceTile (`0x441f00`): army road stamp leaves watered/piped/XTRF.
            // Do not invent-clear XBLD first — PlaceTile overwrites the cell.
            pe_place_tile_441f00(
                &mut self.city,
                x,
                y,
                network_building_for_mask(NetworkKind::Road, axis_mask),
            );
            self.city.set_zone(x, y, ZONE_MILITARY | ZONE_CORNER_ALL);
            placed.push((x, y));
        }
        let mut refresh = HashSet::new();
        for &(x, y) in &placed {
            refresh.insert((x, y));
            for neighbor in neighbor_tiles(x, y) {
                refresh.insert(neighbor);
            }
        }
        for (x, y) in refresh {
            self.refresh_network_tile(x, y, NetworkKind::Road);
        }
        if placed.is_empty() {
            return;
        }
        self.maybe_stamp_army_runway_cross(placed[0].0, placed[0].1, x1 == x2);
        if placed.len() > 1 {
            let (ex, ey) = *placed.last().expect("len > 1");
            self.maybe_stamp_army_runway_cross(ex, ey, x1 == x2);
        }
    }

    pub(super) fn maybe_stamp_army_runway_cross(
        &mut self,
        x: usize,
        y: usize,
        strip_dx_is_zero: bool,
    ) {
        let building = self.city.building_at(x, y);
        // PE @ 0x438466 / 0x438470: already cross `0xDE` → skip.
        if building == AIRPORT_RUNWAY_CROSS_BUILDING_ID {
            return;
        }
        // PE @ `0x438530`–`0x43857b`: already runway `0xDD` may upgrade to `0xDE`.
        if building == RUNWAY_BUILDING_ID {
            let flag = pe_army_form_strip_de_flag(strip_dx_is_zero, self.city.rotation());
            pe_army_runway_dd_to_de_43857b(&mut self.city, x, y, ZONE_MILITARY, flag);
            return;
        }
        if self.city.terrain_at(x, y) != 0 {
            return;
        }
        if !self
            .city
            .zone_at(x, y)
            .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
        {
            return;
        }
        // PE @ 0x43847a: XBLD >= 0x0D → FUN_004392A0 clear, then PlaceTile(0xDD).
        if building >= SMALL_PARK_BUILDING_ID {
            pe_fun_004392a0_clear_before_runway(&mut self.city, x, y);
        }
        // PE PlaceTile (`0x441f00`) @ `0x43848a`: runway `0xDD` (not cross `0xDE`).
        pe_place_tile_441f00(&mut self.city, x, y, RUNWAY_BUILDING_ID);
        let flag = pe_army_form_strip_de_flag(strip_dx_is_zero, self.city.rotation());
        pe_army_after_runway_dd_zone_4384e6(&mut self.city, x, y, ZONE_MILITARY, flag);
        // Fresh DD may qualify for DE upgrade (`0x43857b`).
        pe_army_runway_dd_to_de_43857b(&mut self.city, x, y, ZONE_MILITARY, flag);
    }
}

fn military_base_tile_is_candidate(city: &City, x: usize, y: usize) -> bool {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return false;
    }
    let building = city.building_at(x, y);
    if building >= SMALL_PARK_BUILDING_ID {
        return false;
    }
    let terrain = city.terrain_at(x, y);
    if terrain != 0 || is_water_terrain(terrain) {
        return false;
    }
    if city.zone_at(x, y).is_some_and(|zone| zone.zone_id() != 0) {
        return false;
    }
    city.underground_at(x, y) == 0
}

pub(super) fn evaluate_military_base_plot(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> Option<(usize, usize)> {
    if origin_x + MILITARY_BASE_PLOT_SIZE > CITY_WIDTH
        || origin_y + MILITARY_BASE_PLOT_SIZE > CITY_HEIGHT
    {
        return None;
    }
    let base_altitude = city.altitude_at(origin_x, origin_y);
    let mut valid = 0usize;
    let mut same_altitude = 0usize;
    for y in origin_y..origin_y + MILITARY_BASE_PLOT_SIZE {
        for x in origin_x..origin_x + MILITARY_BASE_PLOT_SIZE {
            // PE's census pass counts XBLD/XTXT/XBIT only. Zone and underground
            // are checked later, and XUND is sampled once at the plot origin.
            if !pe_military_base_census_tile_is_eligible(city, x, y) {
                continue;
            }
            valid += 1;
            if (city.altitude_at(x, y) & 0x1f) == (base_altitude & 0x1f) {
                same_altitude += 1;
            }
        }
    }
    Some((valid, same_altitude))
}

fn pe_military_base_census_tile_is_eligible(city: &City, x: usize, y: usize) -> bool {
    x < CITY_WIDTH
        && y < CITY_HEIGHT
        && city.building_at(x, y) < SMALL_PARK_BUILDING_ID
        && city.text_overlay_at(x, y).unwrap_or(0) == 0
        && city.tile_bits_at(x, y).unwrap_or_default().raw_bits() & 0x04 == 0
}

/// Win95 `FUN_004142C0`: an accepted 8×8 sample only selects a base when at least
/// 40 eligible cells also share the origin's low-five-bit elevation. A mixed-altitude
/// sample with 40+ eligible cells but fewer than 40 matching elevations is declined;
/// it does not immediately enter the missile-silo path.
pub(super) fn pe_military_base_plot_type_from_counts(
    valid: usize,
    same_altitude: usize,
) -> Option<u32> {
    if valid < MILITARY_BASE_MIN_VALID_TILES
        || same_altitude < MILITARY_BASE_MIN_VALID_TILES
        || same_altitude > valid
    {
        return None;
    }
    Some(if valid == same_altitude {
        MILITARY_BASE_AIR_FORCE
    } else {
        MILITARY_BASE_ARMY
    })
}

/// Win95 `FUN_004142C0`: up to 24 game-LCG probes, stopping once the sampled 8×8
/// contains 40 eligible cells. If all probes fall short, return the final sample
/// so the caller can run the PE's six 3×3 fallback.
pub(super) fn sample_military_base_plot(
    city: &City,
    rng: &mut Win95GameRand,
) -> Option<(usize, usize, usize, usize)> {
    let origin_mod = (CITY_WIDTH.saturating_sub(MILITARY_BASE_PLOT_SIZE + 1)) as u16;
    let mut last = None;
    for _ in 0..MILITARY_PLOT_SAMPLE_ATTEMPTS {
        let origin_x = usize::from(rng.random_mod(origin_mod.max(1)));
        let origin_y = usize::from(rng.random_mod(origin_mod.max(1)));
        if let Some((valid, same_altitude)) = evaluate_military_base_plot(city, origin_x, origin_y)
        {
            last = Some((origin_x, origin_y, valid, same_altitude));
            if valid >= MILITARY_BASE_MIN_VALID_TILES {
                return last;
            }
        }
    }
    last
}

/// PE fallback candidate gate inside `FUN_004142C0` (`0x4145e1..0x414664`).
/// XUND is tested only at the sampled origin; each 3×3 cell gets the XBLD,
/// XTXT, XBIT, low-five-bit altitude, and non-military-zone checks.
pub(super) fn pe_military_base_fallback_plot_is_candidate(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    altitude: u8,
) -> bool {
    if origin_x + MILITARY_SILO_PLOT_SIZE > CITY_WIDTH
        || origin_y + MILITARY_SILO_PLOT_SIZE > CITY_HEIGHT
        || city.underground_at(origin_x, origin_y) != 0
    {
        return false;
    }
    for y in origin_y..origin_y + MILITARY_SILO_PLOT_SIZE {
        for x in origin_x..origin_x + MILITARY_SILO_PLOT_SIZE {
            if city.building_at(x, y) >= SMALL_PARK_BUILDING_ID
                || city.text_overlay_at(x, y).unwrap_or(0) != 0
                || city.tile_bits_at(x, y).unwrap_or_default().raw_bits() & 0x04 != 0
                || city.altitude_at(x, y) & 0x1f != altitude
                || city
                    .zone_at(x, y)
                    .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
            {
                return false;
            }
        }
    }
    true
}

/// Keep the LCG sampling separate from selection so tests can exercise repeated
/// or overlapping PE samples without relying on a particular process seed.
pub(super) fn select_pe_military_base_fallback_plots(
    city: &City,
    altitude: u8,
    samples: impl IntoIterator<Item = (usize, usize)>,
) -> Option<Vec<(usize, usize)>> {
    let mut plots = Vec::with_capacity(MILITARY_BASE_FALLBACK_PATCH_COUNT);
    for (x, y) in samples {
        if pe_military_base_fallback_plot_is_candidate(city, x, y, altitude) {
            plots.push((x, y));
            if plots.len() == MILITARY_BASE_FALLBACK_PATCH_COUNT {
                return Some(plots);
            }
        }
    }
    None
}

fn sample_pe_military_base_fallback_plots(
    city: &City,
    altitude: u8,
    rng: &mut Win95GameRand,
) -> Option<Vec<(usize, usize)>> {
    let samples = (0..MILITARY_BASE_FALLBACK_SAMPLE_ATTEMPTS).map(|_| {
        (
            usize::from(rng.random_mod(0x7c)),
            usize::from(rng.random_mod(0x7c)),
        )
    });
    select_pe_military_base_fallback_plots(city, altitude, samples)
}

/// PE @ `0x4146bb..0x414759`: after six plots are collected, stamp every tile
/// unconditionally. This intentionally permits overlapping plots and applies
/// each repeated census decrement/counter increment, while preserving XZON's
/// upper flag bits as `old & 0xf7 | 7` does.
pub(super) fn stamp_pe_military_base_fallback_plot(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
) {
    for y in origin_y..origin_y + MILITARY_SILO_PLOT_SIZE {
        for x in origin_x..origin_x + MILITARY_SILO_PLOT_SIZE {
            let building = city.building_at(x, y) as usize;
            city.pe_dw_tile_count[building] = city.pe_dw_tile_count[building].wrapping_sub(1);
            let old_zone = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
            city.set_zone(x, y, (old_zone & 0xf7) | ZONE_MILITARY);
            city.pe_increment_military_zone_census(building as u8);
        }
    }
}

fn naval_strip_view_center(strip: &NavalCoastStrip) -> (usize, usize) {
    let skip = strip.run_len.saturating_sub(MILITARY_NAVAL_ZONE_LENGTH) / 2;
    let mid = skip + MILITARY_NAVAL_ZONE_LENGTH / 2;
    let x = strip.start_x as isize
        + strip.along_dx * mid as isize
        + strip.inland_dx * (MILITARY_NAVAL_ZONE_DEPTH as isize / 2);
    let y = strip.start_y as isize
        + strip.along_dy * mid as isize
        + strip.inland_dy * (MILITARY_NAVAL_ZONE_DEPTH as isize / 2);
    (
        x.clamp(0, (CITY_WIDTH - 1) as isize) as usize,
        y.clamp(0, (CITY_HEIGHT - 1) as isize) as usize,
    )
}

fn military_silo_view_center(city: &City) -> (usize, usize) {
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
            {
                return (x, y);
            }
        }
    }
    (CITY_WIDTH / 2, CITY_HEIGHT / 2)
}

fn find_naval_coast_strip(city: &City) -> Option<NavalCoastStrip> {
    // Four shoreline orientations: walk along the coast, stamp inland away from water.
    const ORIENTATIONS: [(isize, isize, isize, isize); 4] = [
        (0, 1, 1, 0),  // along +Y, inland +X (water to the west)
        (0, 1, -1, 0), // along +Y, inland -X (water to the east)
        (1, 0, 0, 1),  // along +X, inland +Y (water to the north)
        (1, 0, 0, -1), // along +X, inland -Y (water to the south)
    ];

    let mut best: Option<NavalCoastStrip> = None;
    for &(along_dx, along_dy, inland_dx, inland_dy) in &ORIENTATIONS {
        let water_dx = -inland_dx;
        let water_dy = -inland_dy;
        for start_y in 0..CITY_HEIGHT {
            for start_x in 0..CITY_WIDTH {
                let water_x = start_x as isize + water_dx;
                let water_y = start_y as isize + water_dy;
                if water_x < 0
                    || water_y < 0
                    || water_x as usize >= CITY_WIDTH
                    || water_y as usize >= CITY_HEIGHT
                {
                    continue;
                }
                if !is_saltwater_tile(city, water_x as usize, water_y as usize) {
                    continue;
                }
                if !military_base_tile_is_candidate(city, start_x, start_y) {
                    continue;
                }
                // Only start a run at the beginning of a contiguous shore segment.
                let prev_x = start_x as isize - along_dx;
                let prev_y = start_y as isize - along_dy;
                if prev_x >= 0
                    && prev_y >= 0
                    && (prev_x as usize) < CITY_WIDTH
                    && (prev_y as usize) < CITY_HEIGHT
                {
                    let prev_wx = prev_x + water_dx;
                    let prev_wy = prev_y + water_dy;
                    if prev_wx >= 0
                        && prev_wy >= 0
                        && (prev_wx as usize) < CITY_WIDTH
                        && (prev_wy as usize) < CITY_HEIGHT
                        && military_base_tile_is_candidate(city, prev_x as usize, prev_y as usize)
                        && is_saltwater_tile(city, prev_wx as usize, prev_wy as usize)
                    {
                        continue;
                    }
                }

                let mut run_len = 0usize;
                let mut x = start_x as isize;
                let mut y = start_y as isize;
                while x >= 0 && y >= 0 && (x as usize) < CITY_WIDTH && (y as usize) < CITY_HEIGHT {
                    let wx = x + water_dx;
                    let wy = y + water_dy;
                    if wx < 0
                        || wy < 0
                        || (wx as usize) >= CITY_WIDTH
                        || (wy as usize) >= CITY_HEIGHT
                        || !military_base_tile_is_candidate(city, x as usize, y as usize)
                        || !is_saltwater_tile(city, wx as usize, wy as usize)
                    {
                        break;
                    }
                    run_len += 1;
                    x += along_dx;
                    y += along_dy;
                }
                if run_len < MILITARY_NAVAL_MIN_COAST_RUN {
                    continue;
                }
                let candidate = NavalCoastStrip {
                    start_x,
                    start_y,
                    along_dx,
                    along_dy,
                    inland_dx,
                    inland_dy,
                    run_len,
                };
                if best.as_ref().is_none_or(|b| candidate.run_len > b.run_len) {
                    best = Some(candidate);
                }
            }
        }
    }
    best
}

fn silo_plot_is_valid(city: &City, origin_x: usize, origin_y: usize, force: bool) -> bool {
    if origin_x + MILITARY_SILO_PLOT_SIZE > CITY_WIDTH
        || origin_y + MILITARY_SILO_PLOT_SIZE > CITY_HEIGHT
    {
        return false;
    }
    let base_altitude = city.altitude_at(origin_x, origin_y);
    if !force && base_altitude < MILITARY_SILO_MIN_ALTITUDE {
        return false;
    }
    for y in origin_y..origin_y + MILITARY_SILO_PLOT_SIZE {
        for x in origin_x..origin_x + MILITARY_SILO_PLOT_SIZE {
            if !military_base_tile_is_candidate(city, x, y) {
                return false;
            }
            if city.altitude_at(x, y) != base_altitude {
                return false;
            }
            if city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
            {
                return false;
            }
        }
    }
    true
}

fn silo_plots_overlap(a: (usize, usize), b: (usize, usize)) -> bool {
    let dx = a.0.abs_diff(b.0);
    let dy = a.1.abs_diff(b.1);
    dx < MILITARY_SILO_PLOT_SIZE + 2 && dy < MILITARY_SILO_PLOT_SIZE + 2
}

fn find_missile_silo_plots(city: &City, force: bool) -> Option<Vec<(usize, usize)>> {
    let max_origin = CITY_WIDTH.saturating_sub(MILITARY_SILO_PLOT_SIZE);
    let mut positions = Vec::with_capacity(MILITARY_MAX_SILOS);
    for origin_y in 0..=max_origin {
        for origin_x in 0..=max_origin {
            if positions.len() >= MILITARY_MAX_SILOS {
                break;
            }
            if !silo_plot_is_valid(city, origin_x, origin_y, force) {
                continue;
            }
            if positions
                .iter()
                .any(|&existing| silo_plots_overlap(existing, (origin_x, origin_y)))
            {
                continue;
            }
            positions.push((origin_x, origin_y));
        }
        if positions.len() >= MILITARY_MAX_SILOS {
            break;
        }
    }
    (positions.len() == MILITARY_MAX_SILOS).then_some(positions)
}

fn axis_aligned_tile_line(x1: usize, y1: usize, x2: usize, y2: usize) -> Vec<(usize, usize)> {
    let mut tiles = Vec::new();
    if x1 == x2 {
        let (lo, hi) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
        for y in lo..=hi {
            tiles.push((x1, y));
        }
    } else if y1 == y2 {
        let (lo, hi) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        for x in lo..=hi {
            tiles.push((x, y1));
        }
    }
    tiles
}
