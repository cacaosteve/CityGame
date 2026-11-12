use super::*;

pub(in crate::app) fn pe_fb_tick_active(city: &City) -> bool {
    matches!(city.pe_w_current_disaster_type(), 0x04 | 0x09 | 0x0b | 0x0f)
}

/// PE overlay scan `0x45ff9a` matches XTXT `0xfb` with no disaster-type gate.
pub(in crate::app) fn pe_fb_overlay_scan_due(city: &City) -> bool {
    pe_fb_tick_active(city) || pe_overlay_marker_present(city, &[0xfb])
}

pub(in crate::app) fn pe_fb_tick_seed(city: &City) -> u32 {
    // Offline LFSR helper seed; CRT rolls use process stream on the live tick path.
    let mut seed = 0x45FF_9A00u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(in crate::app) fn pe_fb_zone_degradable(building: u8) -> bool {
    (0x70..0xc6).contains(&building) && PE_FB_ZONE_PROTECT[usize::from(building - 0x70)] == 0
}

/// Win95 `0x460061`: ChangeBuilding with replacement disabled.
pub(in crate::app) fn pe_fb_degrade_under_cloud(
    city: &mut City,
    x: usize,
    y: usize,
    crt: &mut Win95CrtRand,
) -> bool {
    let building = city.building_at(x, y);
    if !pe_fb_zone_degradable(building) {
        return false;
    }
    let pop = match pe_destroy_building_size(building) {
        3 => 4,
        size => size as u8,
    };
    let Some(abandoned) = abandoned_building_for_pop_with_rng(pop, crt) else {
        return false;
    };
    let footprint = pe_destroy_building_footprint(city, x, y);
    // The footprint is in row order. Native 0x402496 returns its low-X/high-Y corner.
    let call_x = footprint[0].0;
    let call_y = footprint[footprint.len() - 1].1;
    // ChooseAndPlace draws before the raw call-coordinate bounds check.
    if !choose_and_place_multitile_call_in_bounds(call_x, call_y, pop) {
        return false;
    }
    let (ox, oy) = (call_x, call_y - choose_and_place_half_pop(pop));
    let abandon_size = usize::from(xbld_tile_size(abandoned).max(1));
    let Some(tiles) = building_footprint_from_origin(ox, oy, abandon_size) else {
        return false;
    };
    let corner_bits = pe_choose_and_place_corner_bits(city.rotation());
    for (tx, ty) in tiles {
        // Mode 4 writes in place through PlaceTile. It preserves XTXT and the
        // low XZON nibble and ORs XBIT with 0xe0.
        pe_place_tile_441f00(city, tx, ty, abandoned);
        let zone_id = city.zone_at(tx, ty).map_or(0, |zone| zone.zone_id());
        city.set_zone(
            tx,
            ty,
            zone_id | building_zone_bits_for_tile(ox, oy, tx, ty, abandon_size, corner_bits),
        );
        let current = city.tile_bits_at(tx, ty).unwrap_or_default();
        city.set_tile_bits(
            tx,
            ty,
            TileBits::new(current.raw_bits() | XBIT_CHOOSE_AND_PLACE),
        );
    }
    true
}

/// Win95 `0x4e8828` / `0x4e8838` NESW: strictly lower ALTM `&0x1f`.
pub(in crate::app) fn pe_fb_downhill_dir(city: &City, x: usize, y: usize) -> Option<usize> {
    let src_alt = city.altitude_at(x, y) & 0x1f;
    let mut best_dir = None;
    let mut best_alt = src_alt;
    for dir in 0..4 {
        let Some((nx, ny)) = offset_city_tile(x, y, PE_FIRESTORM_DX[dir], PE_FIRESTORM_DY[dir])
        else {
            continue;
        };
        let dest_alt = city.altitude_at(nx, ny) & 0x1f;
        if dest_alt < best_alt {
            best_dir = Some(dir);
            best_alt = dest_alt;
        }
    }
    best_dir
}

/// Win95 `0x45ff9a` XTXT `0xfb`: 1-in-2 act, 1-in-64 recede (`0x401415`), water 1-in-16,
/// zone degrade via `0x4e7508`/`0x4024fa`, then clear source and paint downhill
/// (else CRT `rand&3`) neighbor if XTXT `< 0x33`.
pub(in crate::app) fn pe_fb_tick_city(city: &mut City) -> PeFbTickStats {
    let seed = pe_fb_tick_seed(city);
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_fb_tick_city_with_rng(city, &mut crt, &mut lfsr)
}

pub(in crate::app) fn pe_fb_tick_city_with_rng(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeFbTickStats {
    let mut stats = PeFbTickStats::default();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            pe_fb_tick_cell_with_rng(city, crt, lfsr, x, y, &mut stats);
        }
    }
    stats
}

pub(in crate::app) fn pe_fb_tick_cell_with_rng(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    x: usize,
    y: usize,
    stats: &mut PeFbTickStats,
) {
    if city.text_overlay_at(x, y) != Some(0xfb) {
        return;
    }
    if crt.random_u16() & 1 != 0 {
        return;
    }
    if lfsr.random_mod64() == 0 {
        city.set_text_overlay(x, y, 0);
        stats.receded += 1;
        return;
    }
    if city.tile_bits_at(x, y).is_some_and(TileBits::water_tile) && crt.random_u16() & 0xf == 0 {
        city.set_text_overlay(x, y, 0);
        stats.receded += 1;
        return;
    }
    if pe_fb_degrade_under_cloud(city, x, y, crt) {
        stats.degraded += 1;
    }
    let dir = pe_fb_downhill_dir(city, x, y).unwrap_or_else(|| usize::from(crt.random_u16() & 3));
    city.set_text_overlay(x, y, 0);
    let Some((nx, ny)) = offset_city_tile(x, y, PE_FIRESTORM_DX[dir], PE_FIRESTORM_DY[dir]) else {
        stats.receded += 1;
        return;
    };
    if city.text_overlay_at(nx, ny).unwrap_or(0) < 0x33 {
        city.set_text_overlay(nx, ny, 0xfb);
        stats.moved += 1;
    } else {
        stats.receded += 1;
    }
}

pub(in crate::app) fn pe_hurricane_wind_seed(city: &City, wind_remaining: u16) -> u32 {
    let mut seed = 0x45D4_E000u32 ^ u32::from(wind_remaining);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 `0x460602`: while duration and `0x4e86b0` are nonzero, CRT 1-in-8 wind, then
/// `RandomWordLFSR&1==0` → LFSR `Random(0x80)` tile, destroy XBLD `> 0x70` with arg2=1.
pub(in crate::app) fn pe_hurricane_wind_tick_with_rng(
    city: &mut City,
    duration: u16,
    wind_remaining: u16,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeHurricaneWindTick {
    if duration == 0 || wind_remaining == 0 {
        return PeHurricaneWindTick {
            play_wind: false,
            destroyed: None,
            wind_remaining,
        };
    }
    let play_wind = crt.random_u16() & 7 == 0;
    let destroyed = if lfsr.random_word_lsb() == 0 {
        let x = usize::from(lfsr.random_mod(0x80));
        let y = usize::from(lfsr.random_mod(0x80));
        if x < CITY_WIDTH && y < CITY_HEIGHT && city.building_at(x, y) > 0x70 {
            // PE @ `0x4606a4`: DestroyStructure → CRT PlaceTile rubble.
            pe_flood_destroy_tile_with_crt(city, crt, x, y);
            Some((x, y))
        } else {
            None
        }
    } else {
        None
    };
    PeHurricaneWindTick {
        play_wind,
        destroyed,
        wind_remaining: wind_remaining - 1,
    }
}

pub(in crate::app) fn pe_hurricane_wind_tick(
    city: &mut City,
    duration: u16,
    wind_remaining: u16,
) -> PeHurricaneWindTick {
    let seed = pe_hurricane_wind_seed(city, wind_remaining);
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_hurricane_wind_tick_with_rng(city, duration, wind_remaining, &mut crt, &mut lfsr)
}

/// Win95 type-0xE `0x4019ec(origin)` after any successful type-2 attempt.
pub(in crate::app) fn pe_blizzard_flood_goto_tile(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> Option<(usize, usize)> {
    pe_blizzard_flood_attempt_ok(city, origin_x, origin_y).then_some((origin_x, origin_y))
}

pub(in crate::app) fn pe_blizzard_flood_seed(city: &City, x: usize, y: usize) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 type-0xE @ `0x45d9d0`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream for each jitter attempt.
    let mut seed = 0x45D9_D000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(in crate::app) fn pe_blizzard_flood_jitter(rng: &mut Win95CrtRand) -> (isize, isize) {
    let dx = (rng.random_u16() & 0x1f) as isize - 0x10;
    let dy = (rng.random_u16() & 0x1f) as isize - 0x10;
    (dx, dy)
}

/// Win95 type-0xE start @ `0x45d9d0`: `dwCityPopulation/0x2710+5` type-2 calls, jitter `rand&0x1f-0x10`.
pub(in crate::app) fn pe_blizzard_flood_start(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> (std::collections::HashMap<(usize, usize), u8>, bool) {
    let mut rng = Win95CrtRand::new(pe_blizzard_flood_seed(city, origin_x, origin_y));
    let mut tiles = std::collections::HashMap::new();
    let mut any = false;
    for _ in 0..pe_population_jitter_attempt_count(city) {
        let (dx, dy) = pe_blizzard_flood_jitter(&mut rng);
        let Some((x, y)) = offset_city_tile(origin_x, origin_y, dx, dy) else {
            continue;
        };
        let source = pe_flood_source_tile(city, x as isize, y as isize);
        // Offline: each nested type-2 keeps its city-name LFSR seed.
        let part = pe_flood_overlays(city, x, y);
        if source.is_some() || !part.is_empty() {
            any = true;
            tiles.extend(part);
        }
    }
    (tiles, any)
}

pub(in crate::app) fn pe_blizzard_flood_start_with_rng(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> (std::collections::HashMap<(usize, usize), u8>, bool) {
    let mut tiles = std::collections::HashMap::new();
    let mut any = false;
    for _ in 0..pe_population_jitter_attempt_count(city) {
        let (dx, dy) = pe_blizzard_flood_jitter(rng);
        let Some((x, y)) = offset_city_tile(origin_x, origin_y, dx, dy) else {
            continue;
        };
        // PE @ `0x45da5e`: `call 0x401b59` type-2 paints `0xfc` on the map live.
        // Nested type-2 fallbacks share process Game_Random LFSR.
        let (source, part) = pe_flood_paint_with_lfsr(city, x as isize, y as isize, lfsr);
        if source.is_some() {
            any = true;
            tiles.extend(part);
        }
    }
    (tiles, any)
}

pub(in crate::app) fn pe_blizzard_flood_overlays(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    pe_blizzard_flood_start(city, origin_x, origin_y).0
}

pub(in crate::app) fn pe_blizzard_flood_overlays_with_rng(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> std::collections::HashMap<(usize, usize), u8> {
    pe_blizzard_flood_start_with_rng(city, origin_x, origin_y, rng, lfsr).0
}

pub(in crate::app) fn pe_blizzard_flood_attempt_ok(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> bool {
    pe_blizzard_flood_start(city, origin_x, origin_y).1
}

pub(in crate::app) fn pe_blizzard_flood_attempt_ok_with_rng(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> bool {
    // Peek helper: clone so readiness probes do not paint the live city.
    pe_blizzard_flood_start_with_rng(&mut city.clone(), origin_x, origin_y, rng, lfsr).1
}

pub(in crate::app) fn pe_hurricane_seed(city: &City) -> u32 {
    // Offline seed for the shared LFSR lane walk and CRT damage draws.
    // The live worker receives both process streams.
    let mut seed = 0x45D4_E000u32 ^ u32::from(city.rotation());
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 `0x45d4e0`: `jmp [(wViewRotation+1)&3]` — south/east/north/west coast arms.
pub(in crate::app) fn pe_hurricane_arm(rotation: u8) -> u8 {
    rotation.wrapping_add(1) & 3
}

pub(in crate::app) fn pe_hurricane_paint_count(arm: u8) -> usize {
    match arm {
        0 | 3 => 0x32,
        _ => 0x64,
    }
}

/// Win95 `0x401604(x,y,1,0)` on arms 0|3: `0x458d40` plays explode `0x1f8` per hit.
pub(in crate::app) fn pe_hurricane_destroy_extra_explode(arm: u8) -> bool {
    matches!(arm, 0 | 3)
}

/// Win95 XBLD `>= 6` shoreline scan, excluding the far edge of each lane.
pub(in crate::app) fn pe_hurricane_edge_tile(
    city: &City,
    arm: u8,
    lane: u8,
) -> Option<(usize, usize)> {
    let lane = usize::from(lane);
    match arm {
        0 => {
            let x = lane;
            if x >= CITY_WIDTH {
                return None;
            }
            let mut y = 0x7f_isize;
            while y >= 0 {
                if city.building_at(x, y as usize) >= 6 {
                    return (y > 0).then_some((x, y as usize));
                }
                y -= 1;
            }
            None
        }
        1 => {
            let y = lane;
            if y >= CITY_HEIGHT {
                return None;
            }
            let mut x = 0x7f_isize;
            while x >= 0 {
                if city.building_at(x as usize, y) >= 6 {
                    return (x > 0).then_some((x as usize, y));
                }
                x -= 1;
            }
            None
        }
        2 => {
            let x = lane;
            if x >= CITY_WIDTH {
                return None;
            }
            let mut y = 0_isize;
            while y < 0x7f {
                if city.building_at(x, y as usize) >= 6 {
                    return (y < 0x7f).then_some((x, y as usize));
                }
                y += 1;
            }
            None
        }
        3 => {
            let y = lane;
            if y >= CITY_HEIGHT {
                return None;
            }
            let mut x = 0_isize;
            while x <= 0x7f {
                if city.building_at(x as usize, y) >= 6 {
                    return (x < 0x7f).then_some((x as usize, y));
                }
                x += 1;
            }
            None
        }
        _ => None,
    }
}

/// Win95 type-0x10 destroy scan: LFSR `Random(0x14)` steps from the same coast as flood.
pub(in crate::app) fn pe_hurricane_destroy_tile(
    city: &City,
    arm: u8,
    lane: u8,
    lfsr: &mut Win95Lfsr,
) -> Option<(usize, usize)> {
    let (scan_x, descending) = match arm {
        0 => (false, true),
        1 => (true, true),
        2 => (false, false),
        3 => (true, false),
        _ => return None,
    };
    let lane = usize::from(lane);
    let (lane_count, extent) = if scan_x {
        (CITY_HEIGHT, CITY_WIDTH)
    } else {
        (CITY_WIDTH, CITY_HEIGHT)
    };
    if lane >= lane_count {
        return None;
    }

    let last = extent as isize - 1;
    let (mut position, far_edge, direction) = if descending {
        (last, 0, -1)
    } else {
        (0, last, 1)
    };
    while (0..=last).contains(&position) {
        let step_axis = position as usize;
        let (x, y) = if scan_x {
            (step_axis, lane)
        } else {
            (lane, step_axis)
        };
        if position != far_edge && city.building_at(x, y) >= PE_HURRICANE_DESTROY_MIN_BUILDING {
            return Some((x, y));
        }
        // The far edge still consumes a draw. A zero step revisits the
        // coordinate, and Win95Lfsr guarantees a nonzero internal state.
        position += direction * lfsr.random_mod(0x14) as isize;
    }
    None
}

pub(in crate::app) fn pe_hurricane_apply_destroy(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    arm: u8,
) -> usize {
    // The same argument controls the destruction animation and its CRT draws.
    let mode1 = pe_hurricane_destroy_extra_explode(arm);
    let mut hits = 0usize;
    let mut spent = 0;
    while spent < PE_HURRICANE_DESTROY_ATTEMPTS {
        spent += 1;
        let lane = lfsr.random_mask_7f() as u8;
        let Some((x, y)) = pe_hurricane_destroy_tile(city, arm, lane, lfsr) else {
            continue;
        };
        // The ascending-Y arm charges a second count at 0x45d6a6,
        // before DestroyStructure can reject the selected target.
        if arm == 2 {
            spent += 1;
        }
        // PE @ `0x45d56e`/`0x45d60c`: DestroyStructure; elevated early-out skips
        // DestroyBuilding (and flag_a=1 explode on arms 0|3).
        if pe_flood_destroy_tile_mode_maybe_crt(city, Some(crt), x, y, mode1) {
            hits += 1;
        }
    }
    hits
}

pub(in crate::app) fn pe_hurricane_extra_explode_count(city: &City) -> usize {
    let arm = pe_hurricane_arm(city.rotation());
    if !pe_hurricane_destroy_extra_explode(arm) {
        return 0;
    }
    let mut clone = city.clone();
    let mut crt = Win95CrtRand::new(pe_hurricane_seed(&clone));
    let mut lfsr = Win95Lfsr::from_u32_seed(pe_hurricane_seed(&clone));
    pe_hurricane_apply_destroy(&mut clone, &mut crt, &mut lfsr, arm)
}

/// Win95 type-0x10 start @ `0x45d4e0`: destroy XBLD `>= 0x0D` with fire flag 0, then coast `0xfc`.
pub(in crate::app) fn pe_hurricane_overlays_with_lfsr(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> (std::collections::HashMap<(usize, usize), u8>, usize) {
    let arm = pe_hurricane_arm(city.rotation());
    let hits = pe_hurricane_apply_destroy(city, crt, lfsr, arm);
    let mut tiles = std::collections::HashMap::new();
    for _ in 0..pe_hurricane_paint_count(arm) {
        let lane = lfsr.random_mask_7f() as u8;
        if let Some(tile) = pe_hurricane_edge_tile(city, arm, lane) {
            tiles.insert(tile, 0xfc);
        }
    }
    (tiles, hits)
}

pub(in crate::app) fn pe_hurricane_overlays(
    city: &mut City,
) -> std::collections::HashMap<(usize, usize), u8> {
    let seed = pe_hurricane_seed(city);
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_hurricane_overlays_with_lfsr(city, &mut crt, &mut lfsr).0
}

pub(in crate::app) fn pe_hurricane_flood_overlays(
    city: &City,
) -> std::collections::HashMap<(usize, usize), u8> {
    pe_hurricane_overlays(&mut city.clone())
}
