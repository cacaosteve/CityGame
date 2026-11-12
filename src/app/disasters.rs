//! Disaster overlay scans, hazard-cell updates, and eligibility checks.
//!
//! These workers retain their existing RNG calls and city-plane writes. Shared
//! destruction workers live in `demolition`; app state and ignition primitives
//! remain in the parent module.

use super::*;

mod fire;
mod flood;
mod riot;
mod weather;
pub(super) use fire::*;
pub(super) use flood::*;
pub(super) use riot::*;
pub(super) use weather::*;

/// Win95 `0x4ca940` XTRF 64×64 cell at `(x>>1, y>>1)` when a disaster effect clears it.
pub(super) fn pe_clear_xtrf_microsim64(city: &mut City, x: usize, y: usize) {
    // One microsim cell — `set_traffic_density` already expands across the 2×2.
    city.set_traffic_density((x / 2) * 2, (y / 2) * 2, 0);
}

/// Win95 `FUN_00460e10` ignite (flag 0/1): high-XTXT rubble, mid-XTXT DestroyStructure(1,1),
/// signs clear XLAB, then paint hit `0xff` and zero XTRF microsim cell.
pub(super) fn pe_fun_00460e10_ignite(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    lfsr: &mut Win95Lfsr,
    x: usize,
    y: usize,
    flag: u8,
) -> bool {
    // PE @ `0x460e6b`: XBIT water-tile bit `0x04` → return 0 before XTXT arms.
    if city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
    {
        return false;
    }
    // PE @ `0x460e8c`: XBLD `< 6` and flag 0 → return 0; flag 1 skips the gate.
    if city.building_at(x, y) < 6 && flag == 0 {
        return false;
    }
    let xtxt = city.text_overlay_at(x, y).unwrap_or(0);
    if matches!(xtxt, 0xf1..=0xf9) {
        // PE @ `0x460f1f`: `0x401cad(4)+1` then `0x40178f` PlaceTile; leave XTXT; no XTRF clear.
        pe_place_tile_441f00(city, x, y, pe_fire_rubble_building(lfsr));
        return false;
    }
    // PE @ `0x460f12`: XTXT `0xc9..=0xf0` (dispatch proxies) → return 0, no paint.
    if matches!(xtxt, 0xc9..=0xf0) {
        return false;
    }
    // PE @ `0x460f43`: XTXT `> 0xfa` (`0xfb..=0xff`) → return 0, no repaint.
    if xtxt > 0xfa {
        return false;
    }
    if xtxt == 0xfa {
        // PE @ `0x460f45`–`0x460f84` decrements the non-network WORD directly;
        // the separate network bucket remains a live map census in CityGame.
        if !city.is_synthetic_trip_marker(x, y)
            && !pe_xbld_counts_fa_network_overlay_4cc4d8(city.building_at(x, y))
        {
            city.pe_nonnetwork_neighbor_links = city.pe_nonnetwork_neighbor_links.wrapping_sub(1);
        }
        city.set_text_overlay(x, y, 0);
    } else if matches!(xtxt, 0x33..=0xc8) {
        // PE @ `0x460efc`: `push 1; push 1; call 0x401604` then fall through to paint.
        pe_destroy_structure_fire_paint(city, crt, x, y);
    } else if matches!(xtxt, 0x01..=0x32) {
        pe_clear_xtxt_sign_label(city, xtxt);
    }
    // PE @ `0x460ecc` / `0x460ef6`: paint `0xff`, clear XTRF microsim, return 1.
    city.set_text_overlay(x, y, 0xff);
    pe_clear_xtrf_microsim64(city, x, y);
    true
}

/// Win95 type-1 start @ `0x45e920`: CRT `rand%0x28-0x14` from `wCityCenter`.
const PE_FIRE_CENTER_JITTER_MOD: u16 = 0x28;
const PE_FIRE_CENTER_JITTER_BIAS: isize = 0x14;
pub(super) const PE_FIRE_SPIRAL_ARM_LIMIT: isize = 0x40;
pub(super) const PE_FIRE_FALLBACK_ATTEMPTS: usize = 0xc8;
/// Win95 `0x45e030` and Mac selector 12 both count up to 65 successful ignitions.
pub(super) const FIRESTORM_IGNITION_LIMIT: usize = 0x41;
/// Win95 `0x4e87d8` / `0x4e87e0` NESW step tables.
pub(super) const PE_FIRESTORM_DX: [isize; 4] = [0, 1, 0, -1];
pub(super) const PE_FIRESTORM_DY: [isize; 4] = [-1, 0, 1, 0];

/// Win95 type-6 start @ `0x45eb20`: same ±32 window, `rand&0x3f==0`, XBLD `>= 0x0E`.
pub(super) const PE_EARTHQUAKE_SCATTER_RADIUS: isize = 0x20;
pub(super) const PE_EARTHQUAKE_MIN_BUILDING: u8 = 0x0e;
/// Win95 shake loop `ebx < 0x18`: dest blit `+(4 << zoom)` on even iters, explode `0x1f8`.
pub(super) const PE_EARTHQUAKE_SHAKE_ITERS: usize = 0x18;
pub(super) const PE_EARTHQUAKE_SHAKE_SHIFT: i32 = 4;

/// Win95 `0x45f760` flood tick: duration zero recedes by LFSR low-bit; thresholds gate CRT rolls.
pub(super) const PE_FLOOD_TICK_SLOW_AT: u16 = 0x34;
pub(super) const PE_FLOOD_TICK_DESTROY_AT: u16 = 0x1e;
/// Win95 jump table `0x46077c`: west, north, east, south.
pub(super) const PE_FLOOD_SPREAD_DX: [isize; 4] = [-1, 0, 1, 0];
pub(super) const PE_FLOOD_SPREAD_DY: [isize; 4] = [0, -1, 0, 1];

/// Win95 `0x4e7508` XBLD `0x70..0xc6`: non-zero entries skip `0x4024fa` degrade.
pub(super) const PE_FB_ZONE_PROTECT: [u8; 0x56] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2,
];

/// Win95 type-0x10 destroy arm: `0x14` RandomAnd7F scans for XBLD `>= 0x0D`.
pub(super) const PE_HURRICANE_DESTROY_ATTEMPTS: usize = 0x14;
pub(super) const PE_HURRICANE_DESTROY_MIN_BUILDING: u8 = 0x0d;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeFloodTickStats {
    pub(super) receded: usize,
    pub(super) destroyed: usize,
    pub(super) spread: usize,
    pub(super) explodes: usize,
    pub(super) rubble: usize,
}

impl PeFloodTickStats {
    pub(super) fn changed(self) -> usize {
        self.receded + self.destroyed + self.spread + self.rubble
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PeFloodNeighborAction {
    OverlayOnly,
    Destroy,
    Rubble,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeFireTickStats {
    pub(super) receded: usize,
    pub(super) destroyed: usize,
    pub(super) ignited: usize,
}

impl PeFireTickStats {
    pub(super) fn changed(self) -> usize {
        self.receded + self.destroyed + self.ignited
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeRiotTickStats {
    pub(super) receded: usize,
    pub(super) flipped: usize,
    pub(super) spread: usize,
    pub(super) ignited: usize,
}

impl PeRiotTickStats {
    pub(super) fn changed(self) -> usize {
        self.receded + self.flipped + self.spread + self.ignited
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeFbTickStats {
    pub(super) receded: usize,
    pub(super) moved: usize,
    pub(super) degraded: usize,
}

impl PeFbTickStats {
    pub(super) fn changed(self) -> usize {
        self.receded + self.moved + self.degraded
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeHurricaneWindTick {
    pub(super) play_wind: bool,
    pub(super) destroyed: Option<(usize, usize)>,
    pub(super) wind_remaining: u16,
}

/// PE `FUN_00461190` @ `0x461190`: paint `0xfb` only when current XTXT `< 0x33`.
pub(super) fn pe_fun_00461190_paint_fb_if_low(city: &mut City, x: usize, y: usize) {
    if city.text_overlay_at(x, y).unwrap_or(0) < 0x33 {
        city.set_text_overlay(x, y, 0xfb);
    }
}

/// PE `FUN_00461140` @ `0x461140`: paint marker only when current XTXT `< 0x33`.
pub(super) fn pe_fun_00461140_paint_xtxt_if_low(city: &mut City, x: usize, y: usize, marker: u8) {
    if city.text_overlay_at(x, y).unwrap_or(0) < 0x33 {
        city.set_text_overlay(x, y, marker);
    }
}

pub(super) fn pe_overlay_marker_present(city: &City, markers: &[u8]) -> bool {
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            if city
                .text_overlay_at(x, y)
                .is_some_and(|marker| markers.contains(&marker))
            {
                return true;
            }
        }
    }
    false
}

pub(super) fn pe_population_jitter_attempt_count(city: &City) -> usize {
    // PE type-0xD `0x45e180` and type-0xE `0x45d9d0` both divide
    // `dwCityPopulation` (`0x4caa74`) by 0x2710, add 5, then test the
    // low word as signed before entering a loop that decrements that word.
    let attempts = (city.pe_dw_city_population() / 0x2710 + 5) as u16 as i16;
    attempts.max(0) as usize
}

/// XTXT bands that take the FUN_00460e10 fire-paint / XTRF path (not high-XTXT rubble).
pub(super) fn pe_ignite_paints_fire_xtxt(xtxt: u8) -> bool {
    xtxt < 0xc9 || xtxt == 0xfa
}

/// Win95 `obj+0x140` zoom byte used as `shl` count. Viewer zoom is a scale factor.
pub(super) fn pe_earthquake_zoom_level(zoom: f32) -> u32 {
    zoom.max(1.0).log2().round().clamp(0.0, 7.0) as u32
}

/// Win95 `4 << [obj+0x140]`: even-iter dest-left shift in screen pixels.
pub(super) fn pe_earthquake_shake_shift(zoom: f32) -> f32 {
    (PE_EARTHQUAKE_SHAKE_SHIFT << pe_earthquake_zoom_level(zoom)) as f32
}

/// Remaining countdown: 24 → phase 0 (shifted), 23 → phase 1 (identity), …, 0 → none.
pub(super) fn pe_earthquake_shake_delta(remaining: u8, zoom: f32) -> Vec2 {
    if remaining == 0 {
        return Vec2::ZERO;
    }
    let phase = PE_EARTHQUAKE_SHAKE_ITERS.saturating_sub(usize::from(remaining));
    if phase & 1 == 0 {
        Vec2::new(pe_earthquake_shake_shift(zoom), 0.0)
    } else {
        Vec2::ZERO
    }
}

pub(super) fn pe_earthquake_scatter_seed(city: &City, x: usize, y: usize) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 type-6 @ `0x45eb20`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream for the ±32 pass.
    let mut seed = 0x45EB_2000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 type-6 start @ `0x45eb20`: 1-in-64 cells, XBLD `>= 0x0E`, `rand&3==0` ignite else
/// `0x401604(x,y,0,0)` multi-tile destroy (no fire paint).
pub(super) fn pe_earthquake_overlays(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    let seed = pe_earthquake_scatter_seed(city, origin_x, origin_y);
    let mut rng = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_earthquake_overlays_with_rng(city, origin_x, origin_y, &mut rng, &mut lfsr)
}

pub(super) fn pe_earthquake_overlays_with_rng(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> std::collections::HashMap<(usize, usize), u8> {
    let mut tiles = std::collections::HashMap::new();
    let mut si = -PE_EARTHQUAKE_SCATTER_RADIUS;
    while si <= PE_EARTHQUAKE_SCATTER_RADIUS {
        let mut di = -PE_EARTHQUAKE_SCATTER_RADIUS;
        while di <= PE_EARTHQUAKE_SCATTER_RADIUS {
            if rng.random_u16() & 0x3f != 0 {
                di += 1;
                continue;
            }
            let Some((x, y)) = offset_city_tile(origin_x, origin_y, si, di) else {
                di += 1;
                continue;
            };
            if city.building_at(x, y) < PE_EARTHQUAKE_MIN_BUILDING {
                di += 1;
                continue;
            }
            if rng.random_u16() & 3 == 0 {
                // PE @ `0x45ed53`: `push 0; call 0x401bf4` — no outer XTXT prefilter.
                if pe_fun_00460e10_ignite(city, Some(rng), lfsr, x, y, 0) {
                    tiles.insert((x, y), 0xff);
                }
            } else {
                // PE `0x45ed49` passes both DestroyStructure flags as zero.
                pe_flood_destroy_tile_mode_maybe_crt(city, Some(rng), x, y, false);
            }
            di += 1;
        }
        si += 1;
    }
    tiles
}

pub(super) fn pe_rioters_seed(city: &City, x: usize, y: usize) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 type-3 @ `0x45e5f0`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream for the marker.
    let mut seed = 0x45E5_F000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(super) fn pe_rioters_marker(rng: &mut Win95CrtRand) -> u8 {
    ((rng.random_u16() & 1) as u8).wrapping_sub(3)
}

/// Win95 type-3 start @ `0x45e5f0`: roads `0x1d..=0x2b`, rail-ish `0x3f..=0x46`, `0x4b`/`0x4c`, `0x5d..=0x60`.
pub(super) fn pe_rioters_building_eligible(building: u8) -> bool {
    matches!(
        building,
        0x1d..=0x2b | 0x3f..=0x46 | 0x4b | 0x4c | 0x5d..=0x60
    )
}

pub(super) fn pe_rioters_overlay_at(
    city: &City,
    x: usize,
    y: usize,
    painted: Option<&std::collections::HashMap<(usize, usize), u8>>,
) -> u8 {
    if let Some(painted) = painted {
        if let Some(&marker) = painted.get(&(x, y)) {
            return marker;
        }
    }
    city.text_overlay_at(x, y).unwrap_or(0)
}

pub(super) fn pe_rioters_tile_ready_excluding(
    city: &City,
    x: usize,
    y: usize,
    painted: Option<&std::collections::HashMap<(usize, usize), u8>>,
) -> bool {
    pe_rioters_building_eligible(city.building_at(x, y))
        && !city
            .tile_bits_at(x, y)
            .is_some_and(|bits| bits.water_tile())
        && pe_rioters_overlay_at(city, x, y, painted) == 0
}

/// Win95 type-3 start @ `0x45e5f0`: step first, first eligible tile, arm `< 0x80`.
pub(super) fn pe_rioters_target_tile_excluding(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    painted: Option<&std::collections::HashMap<(usize, usize), u8>>,
) -> Option<(usize, usize)> {
    let mut x = origin_x as isize;
    let mut y = origin_y as isize;
    let mut dir = 0_usize;
    let mut arm_len = 1_isize;
    let mut steps_on_arm = 0_isize;
    while arm_len < 0x80 {
        x += PE_FIRESTORM_DX[dir];
        y += PE_FIRESTORM_DY[dir];
        if (0..CITY_WIDTH as isize).contains(&x)
            && (0..CITY_HEIGHT as isize).contains(&y)
            && pe_rioters_tile_ready_excluding(city, x as usize, y as usize, painted)
        {
            return Some((x as usize, y as usize));
        }
        steps_on_arm += 1;
        if steps_on_arm >= arm_len {
            steps_on_arm = 0;
            if dir & 1 != 0 {
                arm_len += 1;
            }
            dir = (dir + 1) & 3;
        }
    }
    None
}

pub(super) fn pe_rioters_target_tile(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> Option<(usize, usize)> {
    pe_rioters_target_tile_excluding(city, origin_x, origin_y, None)
}

pub(super) fn pe_mass_riot_seed(city: &City, x: usize, y: usize) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 type-0xD @ `0x45e180`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream for each jitter
    // attempt and nested type-3 marker.
    let mut seed = 0x45E1_8000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 type-0xD start @ `0x45e180`: `dwCityPopulation/0x2710+5` type-3 calls, jitter `rand&0x1f-0x10`.
pub(super) fn pe_mass_riot_start(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
) -> (std::collections::HashMap<(usize, usize), u8>, usize) {
    let mut rng = Win95CrtRand::new(pe_mass_riot_seed(city, origin_x, origin_y));
    pe_mass_riot_start_with_rng(city, origin_x, origin_y, &mut rng)
}

pub(super) fn pe_mass_riot_start_with_rng(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
) -> (std::collections::HashMap<(usize, usize), u8>, usize) {
    let mut tiles = std::collections::HashMap::new();
    let mut hits = 0usize;
    for _ in 0..pe_population_jitter_attempt_count(city) {
        let (dx, dy) = pe_blizzard_flood_jitter(rng);
        let Some((x, y)) = offset_city_tile(origin_x, origin_y, dx, dy) else {
            continue;
        };
        let Some((tile_x, tile_y)) = pe_rioters_target_tile_excluding(city, x, y, Some(&tiles))
        else {
            continue;
        };
        hits += 1;
        // PE @ `0x45e20e`: `call 0x40266c` type-3 paints the marker on the map live.
        let marker = pe_rioters_marker(rng);
        city.set_text_overlay(tile_x, tile_y, marker);
        tiles.insert((tile_x, tile_y), marker);
    }
    (tiles, hits)
}

pub(super) fn pe_mass_riot_overlays(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    pe_mass_riot_start(&mut city.clone(), origin_x, origin_y).0
}

pub(super) fn pe_mass_riot_overlays_with_rng(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
) -> std::collections::HashMap<(usize, usize), u8> {
    pe_mass_riot_start_with_rng(city, origin_x, origin_y, rng).0
}

pub(super) fn pe_mass_riot_sound_count(city: &City, origin_x: usize, origin_y: usize) -> usize {
    pe_mass_riot_start(&mut city.clone(), origin_x, origin_y).1
}

pub(super) fn pe_mass_riot_sound_count_with_rng(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
) -> usize {
    // Peek helper: clone so readiness probes do not paint the live city.
    pe_mass_riot_start_with_rng(&mut city.clone(), origin_x, origin_y, rng).1
}

pub(super) fn pe_mass_riot_attempt_ok(city: &City, origin_x: usize, origin_y: usize) -> bool {
    pe_mass_riot_sound_count(city, origin_x, origin_y) > 0
}

pub(super) fn pe_mass_riot_attempt_ok_with_rng(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
) -> bool {
    pe_mass_riot_sound_count_with_rng(city, origin_x, origin_y, rng) > 0
}

pub(super) fn pe_fire_seed(city: &City, x: usize, y: usize) -> u32 {
    let mut seed = 0x45E9_2000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(super) fn pe_fire_center_jitter(rng: &mut Win95CrtRand) -> isize {
    (rng.random_u16() % PE_FIRE_CENTER_JITTER_MOD) as isize - PE_FIRE_CENTER_JITTER_BIAS
}

/// Win95 ignite @ `0x460e10` with flag 0: XBLD `>= 6`, no XBIT bit 2, igniteable XTXT.
pub(super) fn pe_fire_ignite_ok(city: &City, x: usize, y: usize) -> bool {
    if city.building_at(x, y) < 6 {
        return false;
    }
    pe_firestorm_can_ignite(city, x, y)
}

/// Win95 type-1 start @ `0x45e920`: NESW tables `0x4e87f8`/`0x4e8800`, arm `< 0x40`, XBLD `>= 0x70`.
/// Candidate tiles must be able to take the FUN_00460e10 fire-paint path (rubble band ret 0 → keep searching).
pub(super) fn pe_fire_start_candidate_ok(city: &City, x: usize, y: usize) -> bool {
    if city.building_at(x, y) < 0x70 {
        return false;
    }
    if city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
    {
        return false;
    }
    pe_ignite_paints_fire_xtxt(city.text_overlay_at(x, y).unwrap_or(0))
}

pub(super) fn pe_fire_spiral_tile(
    city: &City,
    origin_x: isize,
    origin_y: isize,
) -> Option<(usize, usize)> {
    let mut x = origin_x;
    let mut y = origin_y;
    let mut dir = 0_usize;
    let mut arm_len = 1_isize;
    let mut steps_on_arm = 0_isize;
    while arm_len < PE_FIRE_SPIRAL_ARM_LIMIT {
        x += PE_FIRESTORM_DX[dir];
        y += PE_FIRESTORM_DY[dir];
        if (0..CITY_WIDTH as isize).contains(&x)
            && (0..CITY_HEIGHT as isize).contains(&y)
            && pe_fire_start_candidate_ok(city, x as usize, y as usize)
        {
            return Some((x as usize, y as usize));
        }
        steps_on_arm += 1;
        if steps_on_arm >= arm_len {
            steps_on_arm = 0;
            if dir & 1 != 0 {
                arm_len += 1;
            }
            dir = (dir + 1) & 3;
        }
    }
    None
}

pub(super) fn pe_fire_fallback_tile(city: &City, lfsr: &mut Win95Lfsr) -> Option<(usize, usize)> {
    // PE @ `0x45e9e9`: RandomAnd7F then `0x401bf4` flag 0 — no spiral `XBLD >= 0x70`
    // gate. Trees (XBLD 6) succeed when FUN_00460e10 would paint fire.
    for _ in 0..PE_FIRE_FALLBACK_ATTEMPTS {
        let x = usize::from(lfsr.random_mask_7f());
        let y = usize::from(lfsr.random_mask_7f());
        if x < CITY_WIDTH
            && y < CITY_HEIGHT
            && city.building_at(x, y) >= 6
            && !city
                .tile_bits_at(x, y)
                .is_some_and(|bits| bits.water_tile())
            && pe_ignite_paints_fire_xtxt(city.text_overlay_at(x, y).unwrap_or(0))
        {
            return Some((x, y));
        }
    }
    None
}

/// Win95 type-1 start @ `0x45e920`: CRT center jitter, then LFSR RandomAnd7F fallback.
pub(super) fn pe_fire_target_tile_with_rng(
    city: &City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> Option<(usize, usize)> {
    let (cx, cy) = land_value_city_center(city);
    let origin_x = cx as isize + pe_fire_center_jitter(crt);
    let origin_y = cy as isize + pe_fire_center_jitter(crt);
    pe_fire_spiral_tile(city, origin_x, origin_y).or_else(|| pe_fire_fallback_tile(city, lfsr))
}

pub(super) fn pe_fire_target_tile(city: &City) -> Option<(usize, usize)> {
    let (cx, cy) = land_value_city_center(city);
    let seed = pe_fire_seed(city, cx, cy);
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_fire_target_tile_with_rng(city, &mut crt, &mut lfsr)
}

/// Win95 ignite @ `0x460e10` with flag 1 (firestorm): empty XBLD allowed, no XBIT bit 2;
/// paint-success XTXT only (`< 0xc9` or `0xfa` — rubble band returns 0 without paint).
pub(super) fn pe_firestorm_can_ignite(city: &City, x: usize, y: usize) -> bool {
    if city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
    {
        return false;
    }
    pe_ignite_paints_fire_xtxt(city.text_overlay_at(x, y).unwrap_or(0))
}

/// The two binaries walk the same NESW spiral, starting one cell north
/// of the origin. Ignition and successful-paint accounting are caller-owned.
pub(super) fn firestorm_spiral_candidates(
    origin_x: usize,
    origin_y: usize,
) -> impl Iterator<Item = (usize, usize)> {
    disaster_spiral_candidates((origin_x as isize, origin_y as isize), 0x7f)
}

pub(super) fn disaster_spiral_candidates(
    origin: (isize, isize),
    max_leg_length: usize,
) -> impl Iterator<Item = (usize, usize)> {
    (0..2 * max_leg_length)
        .flat_map(|leg| {
            let direction = leg & 3;
            std::iter::repeat_n(
                (PE_FIRESTORM_DX[direction], PE_FIRESTORM_DY[direction]),
                leg / 2 + 1,
            )
        })
        .scan(origin, |point, step| {
            point.0 += step.0;
            point.1 += step.1;
            Some(*point)
        })
        .filter_map(|(x, y)| {
            ((0..CITY_WIDTH as isize).contains(&x) && (0..CITY_HEIGHT as isize).contains(&y))
                .then_some((x as usize, y as usize))
        })
}

/// Win95 firestorm start @ `0x45e030`: predict tiles that would return paint success from
/// FUN_00460e10 flag 1 (water / non-paint XTXT skipped). Live seeding walks every in-bounds
/// step and calls the helper — including rubble-band side effects.
pub(super) fn pe_firestorm_spiral_tiles(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> Vec<(usize, usize)> {
    firestorm_spiral_candidates(origin_x, origin_y)
        .filter(|&(x, y)| pe_firestorm_can_ignite(city, x, y))
        .take(FIRESTORM_IGNITION_LIMIT)
        .collect()
}

/// Win95 type-3 riot arm @ `0x47219a`: `dwCityUnemployment >= 0x0A`, `bWeatherHeat >= 0xAA`.
pub(super) fn pe_riot_disaster_ready(city: &City) -> bool {
    city.pe_unemployment_percent() >= 10 && u32::from(city.pe_b_weather_heat) >= 0xAA
}

/// Win95 type-0xD mass-riot arm @ `0x47218e`: type 3 plus `dwCityPopulation >= 0x7530`.
pub(super) fn pe_mass_riot_disaster_ready(city: &City) -> bool {
    city.pe_dw_city_population() >= 30_000 && pe_riot_disaster_ready(city)
}

/// Win95 `0x45f090` checks the signed visit counter before allocating a slot.
pub(super) fn pe_tornado_constructor_ready(city: &City) -> bool {
    (city.pe_tornado_actor_counter as i16) < 1 && pe_xthg_disaster_slot_available(city)
}

pub(super) fn pe_xthg_disaster_slot_available(city: &City) -> bool {
    city.traffic_overlays().map_or(true, |table| {
        table
            .iter()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .any(|record| record.kind == 0)
    })
}

pub(super) fn pe_fire_heat(city: &City) -> u32 {
    // PE fire arm reads BSS `bWeatherHeat` @ `0x4cade0`.
    u32::from(city.pe_b_weather_heat)
}

/// Win95 fire arm @ `0x471fa1`: `(rand16 & 0x7f) + 0x7f <= bWeatherHeat`.
pub(super) fn pe_fire_disaster_ready<R: AmbientDisasterRandom>(city: &City, rng: &mut R) -> bool {
    let roll = u32::from(rng.random_u16()) & 0x7f;
    roll + 0x7f <= pe_fire_heat(city)
}

/// Win95 flood arm @ `0x472200` / Mac type-2 entry @ `0x35a22`:
/// `(bCityHasRiver || bCityHasOcean) && bWeatherTrend >= 3` (foggy).
/// Mac jump-table slot `0x02` lands after the blizzard compare at `0x35a16`; that
/// stricter weather gate belongs only to type `0x0E` (see blizzard flood below).
pub(super) fn pe_flood_disaster_ready(city: &City) -> bool {
    (pe_city_has_river(city) || pe_city_has_ocean(city))
        && pe_weather_trend(city) >= WEATHER_TREND_FOGGY
}

/// Win95 type-0xE flood arm @ `0x4721f7` / Mac slot `0x0E` @ `0x35a16`:
/// type-2 flood gates plus `bWeatherTrend >= 9` (blizzard).
pub(super) fn pe_blizzard_flood_disaster_ready(city: &City) -> bool {
    pe_flood_disaster_ready(city) && pe_weather_trend(city) >= WEATHER_TREND_BLIZZARD
}

/// Win95 tornado arm @ `0x4720ef`: `bWeatherTrend >= 8` (windy).
pub(super) fn pe_tornado_disaster_ready(city: &City) -> bool {
    pe_weather_trend(city) >= WEATHER_TREND_WINDY
}

/// Win95 hurricane arm @ `0x472282`: `bWeatherTrend >= 8` and `bCityHasOcean`.
pub(super) fn pe_hurricane_disaster_ready(city: &City) -> bool {
    pe_tornado_disaster_ready(city) && pe_city_has_ocean(city)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeDisasterOverlayTickStats {
    fire: PeFireTickStats,
    riot: PeRiotTickStats,
    firestorm: PeFbTickStats,
    flood: PeFloodTickStats,
    dispatch: PeDispatchTickStats,
}

impl PeDisasterOverlayTickStats {
    pub(super) fn changed(self) -> usize {
        self.fire.changed()
            + self.riot.changed()
            + self.firestorm.changed()
            + self.flood.changed()
            + self.dispatch.changed()
    }

    pub(super) fn flood_explodes(self) -> usize {
        self.flood.explodes
    }
}

/// One live PE `SimulationDoDisasterTick` map pass. The native worker dispatches each
/// XTXT marker at its coordinate inside a single x-major scan: fire (`0xff`), riot
/// (`0xfd/0xfe`), firestorm (`0xfb`), flood (`0xfc`), then dispatch proxies (`0xc9..=0xf0`).
/// Keeping these transitions interleaved preserves same-pass marker propagation and the
/// shared CRT/LFSR call chronology; the standalone family walkers remain for focused tests.
pub(super) fn pe_disaster_overlay_tick_city_with_rng(
    city: &mut City,
    duration: u16,
    dispatch_due: bool,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeDisasterOverlayTickStats {
    let flood_due = pe_flood_overlay_scan_due(city);
    let fire_due = pe_fire_overlay_scan_due(city);
    let riot_due = pe_riot_overlay_scan_due(city);
    let firestorm_due = pe_fb_overlay_scan_due(city);
    if !(flood_due || fire_due || riot_due || firestorm_due || dispatch_due) {
        return PeDisasterOverlayTickStats::default();
    }
    let dispatch_records = if dispatch_due {
        city.traffic_overlays().map(|records| records.to_vec())
    } else {
        None
    };
    let mut stats = PeDisasterOverlayTickStats::default();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            match city.text_overlay_at(x, y) {
                Some(0xff) if fire_due => {
                    pe_fire_tick_cell_with_rng(city, crt, lfsr, x, y, &mut stats.fire);
                }
                Some(0xfd | 0xfe) if riot_due => {
                    pe_riot_tick_cell_with_rng(city, crt, lfsr, x, y, &mut stats.riot);
                }
                Some(0xfb) if firestorm_due => {
                    pe_fb_tick_cell_with_rng(city, crt, lfsr, x, y, &mut stats.firestorm);
                }
                Some(0xfc) if flood_due => {
                    pe_flood_tick_cell_with_rng(city, duration, crt, lfsr, x, y, &mut stats.flood);
                }
                Some(0xc9..=0xf0) if dispatch_due => {
                    if let Some(records) = dispatch_records.as_deref() {
                        pe_dispatch_proxy_tick_cell_with_rng(
                            city,
                            records,
                            crt,
                            lfsr,
                            x,
                            y,
                            &mut stats.dispatch,
                        );
                    }
                }
                _ => {}
            }
        }
    }
    stats
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PeDispatchTickStats {
    pub(super) fires_cleared: usize,
    pub(super) riots_cleared: usize,
}

impl PeDispatchTickStats {
    fn changed(self) -> usize {
        self.fires_cleared + self.riots_cleared
    }
}

fn pe_dispatch_tick_seed(city: &City) -> u32 {
    let mut seed = 0x4603_C100u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 `0x4605e0` (thunk `0x401f85`) when XTXT is `0xff`: clear overlay, then:
/// - XBLD `0x3f..=0x42`: clear only
/// - else if XBLD `< 0x61` (including vacant 0): DestroyBuilding mode 0 + PlaceTile rubble
/// - else if XZON corners `0xf0`: DestroyBuilding mode 0 only (no follow-on PlaceTile)
pub(super) fn pe_dispatch_extinguish_fire(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    x: usize,
    y: usize,
) -> bool {
    if city.text_overlay_at(x, y) != Some(0xff) {
        return false;
    }
    city.set_text_overlay(x, y, 0);
    let building = city.building_at(x, y);
    if (0x3f..0x43).contains(&building) {
        return true;
    }
    if building < 0x61 {
        // PE @ `0x46065f`: DestroyBuilding mode 0 (`0x401611` push 0), not
        // DestroyStructure. XBLD `< 6` early-outs (`0x458d8a`) then still
        // PlaceTile `(0x47eef0%4)+1` rubble — zone corners stay.
        pe_destroy_building_mode0_optional_crt(city, Some(crt), x, y);
        pe_place_tile_441f00(city, x, y, pe_fire_rubble_building(lfsr));
        return true;
    }
    if building >= 0x61 {
        // PE @ `0x46067f`: XZON `(bits & 0xf0) == 0xf0` → DestroyBuilding mode 0 only.
        let corners = city
            .zone_at(x, y)
            .map(|zone| zone.raw_bits() & 0xf0)
            .unwrap_or(0);
        if corners == ZONE_CORNER_ALL {
            pe_destroy_building_mode0_optional_crt(city, Some(crt), x, y);
        }
    }
    true
}

fn pe_dispatch_clear_riot(city: &mut City, x: usize, y: usize) -> bool {
    match city.text_overlay_at(x, y) {
        Some(0xfd | 0xfe) => {
            city.set_text_overlay(x, y, 0);
            true
        }
        _ => false,
    }
}

/// Win95 `0x4603c1`: XTXT proxy `0xc9..0xf0` for dispatch police/fire/military.
/// Fire/military always NESW-extinguish `0xff`; police 1-in-16. Police/military NESW-clear riot.
/// Fire direction is CRT `0x48cdd8`. Police fight chance is LFSR `0x4025e5`
/// (`random_mask_0f() == 0`), not CRT. Extinguish rubble is the next LFSR.
pub(super) fn pe_dispatch_proxy_tick_city(city: &mut City) -> PeDispatchTickStats {
    let seed = pe_dispatch_tick_seed(city);
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_dispatch_proxy_tick_city_with_rng(city, &mut crt, &mut lfsr)
}

pub(super) fn pe_dispatch_proxy_tick_city_with_rng(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeDispatchTickStats {
    let Some(overlays) = city.traffic_overlays().map(|t| t.to_vec()) else {
        return PeDispatchTickStats::default();
    };
    let mut stats = PeDispatchTickStats::default();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            pe_dispatch_proxy_tick_cell_with_rng(city, &overlays, crt, lfsr, x, y, &mut stats);
        }
    }
    stats
}

fn pe_dispatch_proxy_tick_cell_with_rng(
    city: &mut City,
    overlays: &[Win95OverlayRecord],
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    x: usize,
    y: usize,
    stats: &mut PeDispatchTickStats,
) {
    let Some(proxy) = city.text_overlay_at(x, y) else {
        return;
    };
    if !(0xc9..0xf1).contains(&proxy) {
        return;
    }
    let Some(slot) = xthg_slot_for_proxy(proxy) else {
        return;
    };
    let Some(record) = overlays.get(slot) else {
        return;
    };
    let kind = record.kind;
    let fight_fire = match kind {
        OVERLAY_KIND_DISPATCH_FIRE | OVERLAY_KIND_DISPATCH_MILITARY => true,
        // PE `0x460408` → `0x4025e5` / `0x47fbf0`: fight when the LFSR nibble is 0.
        OVERLAY_KIND_DISPATCH_POLICE => lfsr.random_mask_0f() == 0,
        _ => false,
    };
    if fight_fire {
        let dir = usize::from(crt.random_u16() & 3);
        if let Some((nx, ny)) =
            offset_city_tile(x, y, PE_FLOOD_SPREAD_DX[dir], PE_FLOOD_SPREAD_DY[dir])
        {
            if pe_dispatch_extinguish_fire(city, crt, lfsr, nx, ny) {
                stats.fires_cleared += 1;
            }
        }
    }
    if matches!(
        kind,
        OVERLAY_KIND_DISPATCH_POLICE | OVERLAY_KIND_DISPATCH_MILITARY
    ) {
        let dir = usize::from(crt.random_u16() & 3);
        if let Some((nx, ny)) =
            offset_city_tile(x, y, PE_FLOOD_SPREAD_DX[dir], PE_FLOOD_SPREAD_DY[dir])
        {
            if pe_dispatch_clear_riot(city, nx, ny) {
                stats.riots_cleared += 1;
            }
        }
    }
}
