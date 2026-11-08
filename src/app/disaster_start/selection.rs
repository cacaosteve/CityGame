use super::*;

mod targets;
pub(in crate::app) use targets::*;

pub(in crate::app) fn ambient_disaster_for_month<R: AmbientDisasterRandom>(
    city: &City,
    clock: GameClock,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
    fidelity: EngineFidelity,
) -> Option<(DisasterKind, (usize, usize))> {
    if ambient_disaster_month_gates_fail(city, clock) {
        return None;
    }

    let difficulty = city_difficulty_for_city(city);
    let mut chance_rng = R::from_seed(ambient_disaster_chance_seed(city, clock));
    let Some(remainder) = pe_ambient_disaster_chance_remainder(&mut chance_rng, difficulty) else {
        return None;
    };
    let mut rng = R::from_seed(ambient_disaster_seed(city, clock, population, outcomes));
    ambient_disaster_select_with_remainder(
        city, population, outcomes, fidelity, remainder, &mut rng, None,
    )
}

/// Live Win95 day-22 path: chance + kind/tile selection advance one process CRT stream.
/// Optional LFSR peeks Win95 air-crash tile placement (`Game_Random` @ `0x45e7d0`).
pub(in crate::app) fn ambient_disaster_for_month_with_rng<R: AmbientDisasterRandom>(
    city: &City,
    clock: GameClock,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
    fidelity: EngineFidelity,
    rng: &mut R,
    lfsr: Option<&mut Win95Lfsr>,
) -> Option<(DisasterKind, (usize, usize))> {
    if ambient_disaster_month_gates_fail(city, clock) {
        return None;
    }

    let difficulty = city_difficulty_for_city(city);
    let Some(remainder) = pe_ambient_disaster_chance_remainder(rng, difficulty) else {
        return None;
    };
    ambient_disaster_select_with_remainder(
        city, population, outcomes, fidelity, remainder, rng, lfsr,
    )
}

pub(in crate::app) fn ambient_disaster_month_gates_fail(city: &City, clock: GameClock) -> bool {
    city.has_active_disaster()
        || city.pe_dw_city_population() < AMBIENT_DISASTER_MIN_POPULATION
        || !pe_ambient_disaster_age_ready(city, clock)
}

pub(in crate::app) fn ambient_disaster_select_with_remainder<R: AmbientDisasterRandom>(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
    fidelity: EngineFidelity,
    remainder: u16,
    rng: &mut R,
    lfsr: Option<&mut Win95Lfsr>,
) -> Option<(DisasterKind, (usize, usize))> {
    // Mac export_231 @ 0x35932 and PE 0x471ef6 share the weather-first branch:
    // remainder < 0x0F, weather 0x0A+ocean → hurricane type 0x10, weather 0x0B → tornado 0x07.
    // Mac hurricane writes the type and exits with no tile; Mac tornado places `%0x7E+1`.
    if let Some(kind) = pe_ambient_weather_disaster_kind(city, remainder) {
        if fidelity.uses_win95()
            && kind == DisasterKind::Tornado
            && !pe_xthg_disaster_slot_available(city)
        {
            return None;
        }
        let tile = weather_disaster_ambient_target_tile(city, rng, fidelity, kind);
        return Some((kind, tile));
    }
    if remainder != 0 {
        return None;
    }

    let Some(kind) = ambient_disaster_kind_for_city(city, population, outcomes, rng, fidelity)
    else {
        return None;
    };
    if kind == DisasterKind::ToxicSpill {
        let tile = toxic_spill_target_tile_for_engine(city, rng, fidelity)?;
        return Some((kind, tile));
    }
    if kind == DisasterKind::Meltdown {
        let tile = meltdown_ambient_target_tile(city, rng, fidelity)?;
        return Some((kind, tile));
    }
    if kind == DisasterKind::Microwave {
        let tile = microwave_ambient_target_tile(city, rng, fidelity)?;
        return Some((kind, tile));
    }
    if kind == DisasterKind::Rioters {
        let hint = rioters_ambient_target_tile(city, rng, fidelity);
        pe_rioters_target_tile(city, hint.0, hint.1)?;
        return Some((kind, hint));
    }
    if kind == DisasterKind::MassRiot {
        let hint = mass_riot_ambient_target_tile(city, rng, fidelity);
        let ok = if let Some(mut peek) = rng.as_win95_crt_peek() {
            pe_mass_riot_attempt_ok_with_rng(city, hint.0, hint.1, &mut peek)
        } else {
            pe_mass_riot_attempt_ok(city, hint.0, hint.1)
        };
        if !ok {
            return None;
        }
        return Some((kind, hint));
    }
    if kind == DisasterKind::AirCrash {
        if fidelity.uses_win95() && !pe_xthg_disaster_slot_available(city) {
            return None;
        }
        let tile = aircrash_ambient_target_tile(city, rng, fidelity, lfsr)?;
        return Some((kind, tile));
    }
    if kind == DisasterKind::PollutionAccident {
        let tile = pollution_accident_ambient_target_tile(city, rng, fidelity);
        return Some((kind, tile));
    }
    if kind == DisasterKind::Monster {
        // PE type-8 / tornado / air-crash share the usable XTHG free-slot gate.
        if fidelity.uses_win95() && !pe_xthg_disaster_slot_available(city) {
            return None;
        }
        let tile = monster_ambient_target_tile(city, rng, fidelity);
        return Some((kind, tile));
    }
    if kind == DisasterKind::Hurricane {
        let tile = hurricane_ambient_target_tile(city, rng, fidelity);
        return Some((kind, tile));
    }
    if kind == DisasterKind::BlizzardFlood {
        let tile = mac_mod7e_plus_one_ambient_target_tile(city, rng, fidelity);
        // PE type-0xE: CRT jitter + shared process LFSR for nested type-2 fallbacks.
        let ok = if let (Some(mut peek_crt), Some(lfsr_ref)) =
            (rng.as_win95_crt_peek(), lfsr.as_deref())
        {
            let mut peek_lfsr = *lfsr_ref;
            pe_blizzard_flood_attempt_ok_with_rng(
                city,
                tile.0,
                tile.1,
                &mut peek_crt,
                &mut peek_lfsr,
            )
        } else {
            pe_blizzard_flood_attempt_ok(city, tile.0, tile.1)
        };
        if !ok {
            return None;
        }
        return Some((kind, tile));
    }
    if kind == DisasterKind::Fire {
        // PE type-1 @ `0x45e920`: CRT center jitter + LFSR fallback. Live day-22
        // peeks process streams; offline/Mac keeps the city-name helper.
        if let (Some(mut peek_crt), Some(lfsr_ref)) = (rng.as_win95_crt_peek(), lfsr.as_deref()) {
            let mut peek_lfsr = *lfsr_ref;
            let tile = pe_fire_target_tile_with_rng(city, &mut peek_crt, &mut peek_lfsr)?;
            return Some((kind, tile));
        }
        let tile = pe_fire_target_tile(city)?;
        return Some((kind, tile));
    }
    if kind == DisasterKind::Flood {
        // PE type-2 @ `0x45db00`: source spiral or LFSR empty-XTER fallback.
        let tile = mac_mod7e_plus_one_ambient_target_tile(city, rng, fidelity);
        let ok = if let Some(lfsr_ref) = lfsr.as_deref() {
            let mut peek = *lfsr_ref;
            pe_flood_source_tile(city, tile.0, tile.1).is_some()
                || !pe_flood_overlays_with_lfsr(city, tile.0, tile.1, &mut peek).is_empty()
        } else {
            pe_flood_source_tile(city, tile.0, tile.1).is_some()
                || !pe_flood_overlays(city, tile.0, tile.1).is_empty()
        };
        if !ok {
            return None;
        }
        return Some((kind, tile));
    }
    if kind == DisasterKind::Tornado {
        if fidelity.uses_win95() && !pe_xthg_disaster_slot_available(city) {
            return None;
        }
        let tile = mac_mod7e_plus_one_ambient_target_tile(city, rng, fidelity);
        return Some((kind, tile));
    }
    if kind == DisasterKind::Earthquake {
        let tile = mac_mod7e_plus_one_ambient_target_tile(city, rng, fidelity);
        return Some((kind, tile));
    }
    let tile = ambient_disaster_target_tile(city, rng);
    Some((kind, tile))
}

pub(in crate::app) fn city_month_index(clock: GameClock) -> u32 {
    u32::from(clock.year()).saturating_mul(12) + u32::from(clock.month().saturating_sub(1))
}

/// Win95 `0x471ee9`: remainder of `rand16 % wDisasterChanceTable[difficulty]`.
pub(in crate::app) fn pe_ambient_disaster_chance_remainder<R: AmbientDisasterRandom>(
    rng: &mut R,
    difficulty: u32,
) -> Option<u16> {
    let min_months = pe_ambient_disaster_min_months(difficulty);
    (min_months > 0).then(|| rng.random_u16() % min_months as u16)
}

/// Win95 `0x471ee9`: `rand16 % wDisasterChanceTable[difficulty] == 0`.
pub(in crate::app) fn pe_ambient_disaster_chance_ready<R: AmbientDisasterRandom>(
    rng: &mut R,
    difficulty: u32,
) -> bool {
    pe_ambient_disaster_chance_remainder(rng, difficulty) == Some(0)
}

/// Win95 `0x471ef6`: remainder `< 0x0F` plus hurricane/tornado weather starts that disaster.
pub(in crate::app) fn pe_ambient_weather_disaster_kind(
    city: &City,
    remainder: u16,
) -> Option<DisasterKind> {
    if remainder >= 0x0f {
        return None;
    }
    match pe_weather_trend(city) {
        WEATHER_TREND_HURRICANE if pe_city_has_ocean(city) => Some(DisasterKind::Hurricane),
        WEATHER_TREND_TORNADO => Some(DisasterKind::Tornado),
        _ => None,
    }
}

pub(in crate::app) fn ambient_disaster_chance_seed(city: &City, clock: GameClock) -> u32 {
    let mut seed = 0xC4A4_CE00_u32 ^ city_month_index(clock).rotate_left(7);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed ^= city_difficulty_for_city(city).rotate_left(13);
    seed
}

pub(in crate::app) fn ambient_disaster_seed(
    city: &City,
    clock: GameClock,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
) -> u32 {
    let mut seed = 0xA11E_D15A_u32 ^ city_month_index(clock).rotate_left(7);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed ^= population.total_population.rotate_left(3);
    seed ^= population.work_force.rotate_left(11);
    seed ^= outcomes.city_value.rotate_left(17);
    seed
}

pub(in crate::app) fn pe_ambient_disaster_min_months(difficulty: u32) -> u32 {
    PE_AMBIENT_DISASTER_MIN_MONTHS
        .get(difficulty as usize)
        .copied()
        .unwrap_or(PE_AMBIENT_DISASTER_MIN_MONTHS[CITY_DIFFICULTY_MEDIUM as usize])
}

pub(in crate::app) fn pe_city_age_months(city: &City, clock: GameClock) -> u32 {
    if let Some(days) = city.city_age_days().filter(|&days| days > 0) {
        return days / u32::from(DAYS_PER_MONTH);
    }
    let founded = pe_city_founded_year(city, clock);
    u32::from(clock.year().saturating_sub(founded))
        .saturating_mul(12)
        .saturating_add(u32::from(clock.month().saturating_sub(1)))
}

/// Win95 `0x471ec8`: skip ambient disasters until `dwCityDays/25 >= wDisasterChanceTable[difficulty]`.
pub(in crate::app) fn pe_ambient_disaster_age_ready(city: &City, clock: GameClock) -> bool {
    let min_months = pe_ambient_disaster_min_months(city_difficulty_for_city(city));
    min_months > 0 && pe_city_age_months(city, clock) >= min_months
}

pub(in crate::app) fn disaster_kind_misc_type(kind: DisasterKind) -> u32 {
    match kind {
        DisasterKind::Fire => 0x01,
        DisasterKind::Flood => 0x02,
        DisasterKind::BlizzardFlood => 0x0e,
        DisasterKind::Rioters => 0x03,
        DisasterKind::MassRiot => 0x0d,
        DisasterKind::AirCrash => 0x12,
        DisasterKind::NopAirCrash => 0x05,
        DisasterKind::HelicopterCrash => 0x11,
        DisasterKind::Earthquake => 0x06,
        DisasterKind::Tornado => 0x07,
        DisasterKind::Monster => 0x08,
        DisasterKind::Meltdown => 0x09,
        DisasterKind::Microwave => 0x0a,
        DisasterKind::ToxicSpill => 0x04,
        DisasterKind::Volcano => 0x0b,
        DisasterKind::Firestorm => 0x0c,
        DisasterKind::PollutionAccident => 0x0f,
        DisasterKind::Hurricane => 0x10,
    }
}

/// Mac `export_231_CODE_7`: common type-3/type-D gate from saved `fffffdf2`
/// and the low byte of Mac weather state `fffff084`. The riot input is a
/// shared full-DWORD MISC field at +0x0fa4 in both native save layouts; the
/// weather input remains the Mac low-byte interpretation.
pub(in crate::app) fn mac_riot_disaster_ready(city: &City) -> bool {
    city.mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET)
        // CMP.L followed by signed BGT @ 0x35a80..0x35a86.
        .is_some_and(|value| (value as i32) >= 10)
        && city
            .mac_state_misc_u32_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET)
            .is_some_and(|value| value & 0xff >= 0xaa)
}

/// Mac type D adds `fffff01a >= 0x7530` to the shared riot gate. This scalar
/// is the monthly weighted R/C/I light/dense growth aggregate, not population.
pub(in crate::app) fn mac_mass_riot_disaster_ready(city: &City) -> bool {
    city.mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET)
        .is_some_and(|value| value >= 30_000)
        && mac_riot_disaster_ready(city)
}

/// Mac CODE 7 `export_231` selector 8 @ `0x35c98` gates Monster on the
/// saved weighted R/C/I growth scalar, not PE's `dwCityPopulation` cache.
pub(in crate::app) fn mac_monster_disaster_ready(city: &City) -> bool {
    city.mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET)
        .is_some_and(|value| value >= 45_000)
}

/// Win95 type-0x12 air-crash arm @ `0x4722a4`: `dwTileCount[0xdd] != 0` (`0x4ca4c8+0x1ba`).
/// Intersection `0xde` is not consulted by this ready gate.
pub(in crate::app) fn pe_aircrash_disaster_ready(city: &City) -> bool {
    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] != 0
}

pub(in crate::app) fn building_tile_count_for_city(city: &City, building: u8) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city.building_at(x, y) == building {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

/// Mac `export_231` type-0x12 @ `0x35bf0`: `tst.w` tile-count word for building `221`
/// (`0x1ba` into the `fffff07a` table). Intersection `222` is not tested on Mac.
pub(in crate::app) fn mac_aircrash_disaster_ready(city: &City) -> bool {
    building_tile_count_for_city(city, RUNWAY_BUILDING_ID) > 0
}

pub(in crate::app) fn aircrash_disaster_ready(city: &City, fidelity: EngineFidelity) -> bool {
    if fidelity.uses_win95() {
        pe_aircrash_disaster_ready(city)
    } else {
        mac_aircrash_disaster_ready(city)
    }
}

/// Win95 type-9 meltdown arm @ `0x472160`: `dwTileCount[0xcb] != 0` (`0x4ca4c8+0x196`).
pub(in crate::app) fn pe_meltdown_disaster_ready(city: &City) -> bool {
    city.pe_dw_tile_count[NUCLEAR_POWER_BUILDING_ID as usize] != 0
}

pub(in crate::app) fn pe_disaster_auto_goto_tile(
    city: &City,
    kind: DisasterKind,
    origin: (usize, usize),
) -> Option<(usize, usize)> {
    match kind {
        // Win95 types 0x05 / 0x11: `mov esi,1` with no start site — no camera goto.
        DisasterKind::NopAirCrash | DisasterKind::HelicopterCrash => None,
        // Live flood goto is set in `seed_pe_flood` after source/fallback resolution.
        DisasterKind::Flood => None,
        DisasterKind::BlizzardFlood => pe_blizzard_flood_goto_tile(city, origin.0, origin.1),
        DisasterKind::Fire => None,
        // Live firestorm goto is set in `seed_pe_firestorm_spiral` after the last paint success.
        DisasterKind::Firestorm => None,
        DisasterKind::Meltdown => pe_meltdown_plant_tile(city, origin),
        // Live microwave goto is set in `seed_pe_microwave_walk` after the process-CRT walk.
        DisasterKind::Microwave => None,
        // Live air-crash goto is set in `seed_pe_aircrash` after the process-LFSR probe.
        DisasterKind::AirCrash => None,
        DisasterKind::Earthquake
        | DisasterKind::PollutionAccident
        | DisasterKind::MassRiot
        | DisasterKind::Rioters
        | DisasterKind::Volcano
        | DisasterKind::Tornado
        | DisasterKind::Monster => Some(origin),
        _ => Some(origin),
    }
}

/// Win95 type-9 start @ `0x45f2b0`: use hint if XBLD is `0xcb`, else scan x then y.
pub(in crate::app) fn pe_meltdown_plant_tile(
    city: &City,
    hint: (usize, usize),
) -> Option<(usize, usize)> {
    if hint.0 < CITY_WIDTH
        && hint.1 < CITY_HEIGHT
        && city.building_at(hint.0, hint.1) == NUCLEAR_POWER_BUILDING_ID
    {
        return Some(hint);
    }
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            if city.building_at(x, y) == NUCLEAR_POWER_BUILDING_ID {
                return Some((x, y));
            }
        }
    }
    None
}

fn pe_meltdown_scatter_seed(city: &City, x: usize, y: usize) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 type-9 @ `0x45f2b0`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream for the ±32 scatter.
    let mut seed = 0x45F2_B000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 type-9 start @ `0x45f2b0`: `0x401604(x,y,1,1)` clears the nuclear footprint and paints `0xff`.
pub(in crate::app) fn pe_meltdown_apply_plant_destroy(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    hit_x: usize,
    hit_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    // PE @ `0x45f39f`: `push 1; push 1; call 0x401604` after footprint origin walk `0x402496`.
    let footprint = pe_destroy_building_footprint(city, hit_x, hit_y);
    pe_destroy_structure_fire_paint(city, crt, hit_x, hit_y);
    let mut tiles = std::collections::HashMap::new();
    for (x, y) in footprint {
        if city.text_overlay_at(x, y) == Some(0xff) {
            tiles.insert((x, y), 0xff);
        }
    }
    tiles
}

/// Win95 type-9 start @ `0x45f2b0`: destroy plant, then 1-in-32 scatter.
pub(in crate::app) fn pe_meltdown_overlays_with_rng(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> std::collections::HashMap<(usize, usize), u8> {
    let mut tiles = pe_meltdown_apply_plant_destroy(city, Some(rng), origin_x, origin_y);
    for (tile, marker) in pe_meltdown_scatter_overlays_with_rng(city, origin_x, origin_y, rng, lfsr)
    {
        tiles.insert(tile, marker);
    }
    tiles
}

/// Win95 type-9 start @ `0x45f2b0`: 1-in-32 cells; `rand&3==0` ignite else
/// `0x401604(0,0)` then water `0xfb` / dry PlaceTile XBLD `5`.
pub(in crate::app) fn pe_meltdown_overlays(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    let seed = pe_meltdown_scatter_seed(city, origin_x, origin_y);
    let mut rng = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_meltdown_overlays_with_rng(city, origin_x, origin_y, &mut rng, &mut lfsr)
}

pub(in crate::app) fn pe_meltdown_scatter_overlays(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    let seed = pe_meltdown_scatter_seed(city, origin_x, origin_y);
    let mut rng = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_meltdown_scatter_overlays_with_rng(city, origin_x, origin_y, &mut rng, &mut lfsr)
}

pub(in crate::app) fn pe_meltdown_scatter_overlays_with_rng(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> std::collections::HashMap<(usize, usize), u8> {
    let mut tiles = std::collections::HashMap::new();
    let mut si = -PE_MELTDOWN_SCATTER_RADIUS;
    while si <= PE_MELTDOWN_SCATTER_RADIUS {
        let mut di = -PE_MELTDOWN_SCATTER_RADIUS;
        while di <= PE_MELTDOWN_SCATTER_RADIUS {
            if rng.random_u16() & 0x1f != 0 {
                di += 1;
                continue;
            }
            let Some((x, y)) = offset_city_tile(origin_x, origin_y, si, di) else {
                di += 1;
                continue;
            };
            if rng.random_u16() & 3 == 0 {
                // PE @ `0x45f484`: `push 1; call 0x401bf4` — no outer XTXT prefilter.
                if pe_fun_00460e10_ignite(city, Some(rng), lfsr, x, y, 1) {
                    tiles.insert((x, y), 0xff);
                }
            } else {
                // PE @ `0x45f411`: DestroyStructure(0,0), then `rand&1` water `0xfb` / dry PlaceTile `5`.
                pe_flood_destroy_tile_with_crt(city, rng, x, y);
                if rng.random_u16() & 1 != 0 {
                    if city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.water_tile())
                    {
                        // PE paints `0xfb` onto the map (then seed may re-stamp from the return map).
                        city.set_text_overlay(x, y, 0xfb);
                        tiles.insert((x, y), 0xfb);
                    } else {
                        // PE @ `0x45f471`: `push 5; call 0x40178f` → PlaceTile `0x441f00`.
                        pe_place_tile_441f00(city, x, y, 5);
                    }
                }
            }
            di += 1;
        }
        si += 1;
    }
    // PE @ `0x45f4b8`: after ±32 scatter, near-origin PlaceTile radioactivity.
    pe_meltdown_near_origin_place_tile_radioactivity(city, origin_x, origin_y, rng);
    tiles
}

/// Win95 type-9 @ `0x45f4b8`: `si=-1..2`, `di=-2..1`; CRT `rand&1` → PlaceTile XBLD `5`.
pub(in crate::app) fn pe_meltdown_near_origin_place_tile_radioactivity(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    rng: &mut Win95CrtRand,
) {
    let mut si: i32 = -1;
    while si <= 2 {
        let mut di: i32 = -2;
        while di <= 1 {
            if rng.random_u16() & 1 != 0 {
                if let Some((x, y)) = offset_city_tile(origin_x, origin_y, si as isize, di as isize)
                {
                    pe_place_tile_441f00(city, x, y, 5);
                }
            }
            di += 1;
        }
        si += 1;
    }
}

/// Mac `export_231` type-9 @ `0x35cd6`: nuclear tile-count gate only (no Random / no
/// `a880` tile push). Resolve the plant by scan; Win95 ambient still land-probes a hint.
pub(in crate::app) fn meltdown_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> Option<(usize, usize)> {
    if fidelity.uses_win95() {
        let hint = ambient_disaster_target_tile(city, rng);
        pe_meltdown_plant_tile(city, hint)
    } else {
        let _ = rng;
        pe_meltdown_plant_tile(city, (CITY_WIDTH, CITY_HEIGHT))
    }
}

/// Win95 type-0xA microwave arm @ `0x472177`: `dwTileCount[0xcd] != 0` (`0x4ca4c8+0x19a`).
pub(in crate::app) fn pe_microwave_disaster_ready(city: &City) -> bool {
    city.pe_dw_tile_count[MICROWAVE_POWER_BUILDING_ID as usize] != 0
}

/// Win95 type-0xA start @ `0x45f5c0`: scan x then y for XBLD `0xcd`.
pub(in crate::app) fn pe_microwave_plant_tile(city: &City) -> Option<(usize, usize)> {
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            if city.building_at(x, y) == MICROWAVE_POWER_BUILDING_ID {
                return Some((x, y));
            }
        }
    }
    None
}

pub(in crate::app) fn pe_microwave_walk_seed(city: &City, x: usize, y: usize) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 type-0xA @ `0x45f5c0`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream for each `rand&7` step.
    let mut seed = 0x45F5_C000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(in crate::app) fn pe_microwave_walk_dir(rng: &mut Win95CrtRand) -> usize {
    usize::from(rng.random_u16() & 7)
}

pub(in crate::app) fn pe_microwave_walk_step(dir: usize) -> (isize, isize) {
    (PE_MICROWAVE_WALK_DX[dir], PE_MICROWAVE_WALK_DY[dir])
}

#[derive(Debug, Clone)]
pub(in crate::app) struct PeMicrowaveWalkResult {
    pub(in crate::app) overlays: std::collections::HashMap<(usize, usize), u8>,
    pub(in crate::app) zap_count: usize,
    pub(in crate::app) periodic_goto: (usize, usize),
}

/// Win95 type-0xA @ `0x45f5c0`: one CRT stream for dir, paint, zap, and periodic goto.
pub(in crate::app) fn pe_microwave_walk_with_rng(
    city: &mut City,
    rng: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> Option<PeMicrowaveWalkResult> {
    let (mut x, mut y) = pe_microwave_plant_tile(city)?;
    let mut dir = pe_microwave_walk_dir(rng);
    let mut overlays = std::collections::HashMap::new();
    let mut zap_count = 0usize;
    let mut periodic_goto = (x, y);
    let mut remaining = PE_MICROWAVE_WALK_STEPS;
    while remaining > 0 {
        if city.building_at(x, y) != MICROWAVE_POWER_BUILDING_ID {
            if city
                .tile_bits_at(x, y)
                .is_some_and(|bits| bits.water_tile())
            {
                // PE @ `0x45f68a`: water-tile paints `0xfb` on the map, then still falls into ignite.
                city.set_text_overlay(x, y, 0xfb);
                overlays.insert((x, y), 0xfb);
            }
            // PE @ `0x45f6bc`: `push 1; call 0x401bf4` even after water `0xfb` (FUN refuses water).
            if pe_fun_00460e10_ignite(city, Some(rng), lfsr, x, y, 1) {
                overlays.insert((x, y), 0xff);
            }
            zap_count += 1;
            if remaining % PE_MICROWAVE_GOTO_PERIOD == 0 {
                periodic_goto = (x, y);
            }
        }
        let (dx, dy) = pe_microwave_walk_step(dir);
        let next = offset_city_tile(x, y, dx, dy);
        // PE draws the next direction after stepping, before the next loop's
        // bounds check. Preserve that CRT draw even when this step leaves the
        // map; the native walk returns only when it checks the new position.
        dir = pe_microwave_walk_dir(rng);
        let Some((nx, ny)) = next else {
            break;
        };
        x = nx;
        y = ny;
        remaining -= 1;
    }
    Some(PeMicrowaveWalkResult {
        overlays,
        zap_count,
        periodic_goto,
    })
}

pub(in crate::app) fn pe_microwave_walk_overlays(
    city: &City,
) -> std::collections::HashMap<(usize, usize), u8> {
    let Some((x, y)) = pe_microwave_plant_tile(city) else {
        return std::collections::HashMap::new();
    };
    let seed = pe_microwave_walk_seed(city, x, y);
    let mut clone = city.clone();
    let mut rng = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_microwave_walk_with_rng(&mut clone, &mut rng, &mut lfsr)
        .map(|walk| walk.overlays)
        .unwrap_or_default()
}

/// Win95 type-0xA: plant `0x4019ec`, then again when remaining `bx % 10 == 0` off the plant.
pub(in crate::app) fn pe_microwave_periodic_goto_tile(city: &City) -> Option<(usize, usize)> {
    let (x, y) = pe_microwave_plant_tile(city)?;
    let seed = pe_microwave_walk_seed(city, x, y);
    let mut clone = city.clone();
    let mut rng = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_microwave_walk_with_rng(&mut clone, &mut rng, &mut lfsr).map(|walk| walk.periodic_goto)
}

/// Win95 type-0xA: `0x401096(0x202)` on every walk step that is not XBLD `0xcd`.
pub(in crate::app) fn pe_microwave_zap_count(city: &City) -> usize {
    let Some((x, y)) = pe_microwave_plant_tile(city) else {
        return 0;
    };
    let seed = pe_microwave_walk_seed(city, x, y);
    let mut clone = city.clone();
    let mut rng = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_microwave_walk_with_rng(&mut clone, &mut rng, &mut lfsr)
        .map(|walk| walk.zap_count)
        .unwrap_or(0)
}

/// Mac `export_231` type-0xA @ `0x35ce4` matches PE: count gate only, plant scan at start.
pub(in crate::app) fn microwave_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> Option<(usize, usize)> {
    let _ = (rng, fidelity);
    pe_microwave_plant_tile(city)
}

pub(in crate::app) fn pe_aircrash_tile_seed(city: &City) -> u32 {
    let mut seed = 0x45E7_D000u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 type-0x12 start @ `0x45e7d0`: `Game_Random()&0x3f+0x20` until XTXT==0.
pub(in crate::app) fn pe_aircrash_tile_with_lfsr(
    city: &City,
    lfsr: &mut Win95Lfsr,
) -> Option<(usize, usize)> {
    for _ in 0..PE_AIRCRASH_SEARCH_LIMIT {
        let x =
            PE_AIRCRASH_INNER_ORIGIN + usize::from(lfsr.random_mask_7f() & PE_AIRCRASH_INNER_MASK);
        let y =
            PE_AIRCRASH_INNER_ORIGIN + usize::from(lfsr.random_mask_7f() & PE_AIRCRASH_INNER_MASK);
        if x < CITY_WIDTH && y < CITY_HEIGHT && city.text_overlay_at(x, y).unwrap_or(0) == 0 {
            return Some((x, y));
        }
    }
    None
}

pub(in crate::app) fn pe_aircrash_tile(city: &City) -> Option<(usize, usize)> {
    let mut rng = Win95Lfsr::from_u32_seed(pe_aircrash_tile_seed(city));
    pe_aircrash_tile_with_lfsr(city, &mut rng)
}

/// Mac type-0x12 ambient placement after the runway-count gate: shared `%0x7E+1` map pick.
/// Win95 uses `Game_Random` LFSR probes when `lfsr` is provided; otherwise city-name seed.
pub(in crate::app) fn aircrash_ambient_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
    lfsr: Option<&mut Win95Lfsr>,
) -> Option<(usize, usize)> {
    if fidelity.uses_win95() {
        if let Some(lfsr) = lfsr {
            return pe_aircrash_tile_with_lfsr(city, lfsr);
        }
        pe_aircrash_tile(city)
    } else {
        Some(mac_mod7e_plus_one_ambient_target_tile(city, rng, fidelity))
    }
}

/// Win95 type-4 toxic-spill arm @ `0x471fe5`: any 64×64 XPLT cell `>= 0x96`.
pub(in crate::app) fn pe_pollution_fire_ready(city: &City) -> bool {
    let Some(values) = city.pollution_density_slice() else {
        return false;
    };
    if values.len() != CITY_TILES {
        return values.iter().any(|&cell| cell >= 0x96);
    }
    for my in 0..CITY_HEIGHT / 2 {
        for mx in 0..CITY_WIDTH / 2 {
            if pe_xplt_microsim64_value(values, mx, my) >= 0x96 {
                return true;
            }
        }
    }
    false
}

pub(in crate::app) fn pe_xplt_microsim64_value(values: &[u8], mx: usize, my: usize) -> u8 {
    if values.len() != CITY_TILES {
        return 0;
    }
    let mut cell = 0_u8;
    for dy in 0..2 {
        for dx in 0..2 {
            let tx = mx * 2 + dx;
            let ty = my * 2 + dy;
            if tx < CITY_WIDTH && ty < CITY_HEIGHT {
                cell = cell.max(values[density_map_index(tx, ty)]);
            }
        }
    }
    cell
}

/// Win95 type-4 scan @ `0x471fe5`: hottest XPLT `>= 0x96` that passes `rand%10==0`,
/// jittered `rand%10 + cell*2 - 5`. Scan is mx-major (`0x4ca828` column, then my).
/// Last pick must land in `0..0x80`.
pub(in crate::app) fn pe_toxic_spill_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
) -> Option<(usize, usize)> {
    toxic_spill_target_tile_scan(city, rng, ToxicSpillScanOrder::ColumnMajor)
}

/// Mac `export_231` type-4 arm @ `0x35ad6`: the XPLT row-pointer table
/// (`A5-0x17c0` / `ffffe840`) is indexed by `my` in the outer loop (`D3`) with `mx` as
/// the inner offset (`D4`) — row-major / my-major. Its bound-10 draws use the signed
/// QuickDraw `_Random` result through `AmbientDisasterRandom::random_range`.
pub(in crate::app) fn mac_toxic_spill_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
) -> Option<(usize, usize)> {
    toxic_spill_target_tile_scan(city, rng, ToxicSpillScanOrder::RowMajor)
}

pub(in crate::app) fn toxic_spill_target_tile_for_engine<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> Option<(usize, usize)> {
    if fidelity.uses_win95() {
        pe_toxic_spill_target_tile(city, rng)
    } else {
        mac_toxic_spill_target_tile(city, rng)
    }
}

#[derive(Clone, Copy)]
enum ToxicSpillScanOrder {
    /// PE: outer `mx`, inner `my`.
    ColumnMajor,
    /// Mac: outer `my` (row pointer), inner `mx`.
    RowMajor,
}

fn toxic_spill_target_tile_scan<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
    order: ToxicSpillScanOrder,
) -> Option<(usize, usize)> {
    let values = city.pollution_density_slice()?;
    if values.len() != CITY_TILES {
        return None;
    }
    let mut best = 0_u8;
    let mut last = None;
    let visit =
        |mx: usize, my: usize, best: &mut u8, last: &mut Option<(isize, isize)>, rng: &mut R| {
            let cell = pe_xplt_microsim64_value(values, mx, my);
            if cell < 0x96 || cell <= *best {
                return;
            }
            // This adapter preserves signed QuickDraw modulo for Mac and CRT rand modulo for PE.
            if rng.random_range(10) != 0 {
                return;
            }
            *best = cell;
            let x = (mx as isize) * 2 + rng.random_range(10) as isize - 5;
            let y = (my as isize) * 2 + rng.random_range(10) as isize - 5;
            *last = Some((x, y));
        };
    match order {
        ToxicSpillScanOrder::ColumnMajor => {
            for mx in 0..CITY_WIDTH / 2 {
                for my in 0..CITY_HEIGHT / 2 {
                    visit(mx, my, &mut best, &mut last, rng);
                }
            }
        }
        ToxicSpillScanOrder::RowMajor => {
            for my in 0..CITY_HEIGHT / 2 {
                for mx in 0..CITY_WIDTH / 2 {
                    visit(mx, my, &mut best, &mut last, rng);
                }
            }
        }
    }
    if best == 0 {
        return None;
    }
    let (x, y) = last?;
    (x >= 0 && y >= 0 && x < CITY_WIDTH as isize && y < CITY_HEIGHT as isize)
        .then_some((x as usize, y as usize))
}

/// Win95 type-0xF pollution-accident arm @ `0x472241`: industrial tax current (`pBudgetArr+0x140`) >= `0x2710`.
pub(in crate::app) fn pe_pollution_accident_ready(city: &City) -> bool {
    budget_row_misc_offset_for_city(city, 2, BUDGET_SERVICE_CURRENT_FIELD)
        .and_then(|offset| city.budget_misc_u32_at(offset))
        .unwrap_or(0)
        >= 0x2710
}

pub(in crate::app) fn ambient_disaster_kind_for_city<R: AmbientDisasterRandom>(
    city: &City,
    _population: PopulationStats,
    _outcomes: CityServiceOutcomes,
    rng: &mut R,
    fidelity: EngineFidelity,
) -> Option<DisasterKind> {
    // Mac `export_231` @ `0x35d1c` and PE `rand() % 0x13` share a 19-slot table
    // indexed by disaster type id. Slots `0x00` / `0x05` / `0x0B` / `0x0C` / `0x11`
    // are immediate exits (`0x35d0c` on Mac). Slot `0x12` is Air Crash (Mac @
    // `0x35bf0`). An unready selected arm returns no disaster instead of compacting
    // to a fallback.
    let choices = [
        None, // 0x00 DISASTER_NONE
        Some(DisasterKind::Fire),
        Some(DisasterKind::Flood),
        Some(DisasterKind::Rioters),
        Some(DisasterKind::ToxicSpill),
        None, // 0x05 NopAirCrash / ambient exit
        Some(DisasterKind::Earthquake),
        Some(DisasterKind::Tornado),
        Some(DisasterKind::Monster),
        Some(DisasterKind::Meltdown),
        Some(DisasterKind::Microwave),
        None, // 0x0B Volcano — SCEN/consume only
        None, // 0x0C Firestorm — SCEN/consume only
        Some(DisasterKind::MassRiot),
        Some(DisasterKind::BlizzardFlood),
        Some(DisasterKind::PollutionAccident),
        Some(DisasterKind::Hurricane),
        None, // 0x11 HelicopterCrash — ambient exit
        Some(DisasterKind::AirCrash),
    ];
    debug_assert_eq!(choices.len(), 0x13);
    let kind = choices[toolbox_random_range(rng, choices.len())]?;
    let ready = match kind {
        DisasterKind::Fire => pe_fire_disaster_ready(city, rng),
        DisasterKind::Flood => pe_flood_disaster_ready(city),
        // Mac type 3 enters at 0x35a7e after the type-D-only comparison and
        // uses the saved unemployment/weather roles through Mac's loader
        // interpretation. The container codec does not select the engine.
        DisasterKind::Rioters => {
            if fidelity.uses_win95() {
                pe_riot_disaster_ready(city)
            } else {
                mac_riot_disaster_ready(city)
            }
        }
        DisasterKind::ToxicSpill => pe_pollution_fire_ready(city),
        DisasterKind::Earthquake => true,
        DisasterKind::Tornado => pe_tornado_disaster_ready(city),
        DisasterKind::Monster => {
            if fidelity.uses_win95() {
                pe_monster_disaster_ready(city)
            } else {
                mac_monster_disaster_ready(city)
            }
        }
        DisasterKind::Meltdown => pe_meltdown_disaster_ready(city),
        DisasterKind::Microwave => pe_microwave_disaster_ready(city),
        // Mac type D compares its phase-21 weighted R/C/I growth scalar before
        // entering the same saved unemployment/weather gates as type 3.
        DisasterKind::MassRiot => {
            if fidelity.uses_win95() {
                pe_mass_riot_disaster_ready(city)
            } else {
                mac_mass_riot_disaster_ready(city)
            }
        }
        DisasterKind::BlizzardFlood => pe_blizzard_flood_disaster_ready(city),
        DisasterKind::PollutionAccident => pe_pollution_accident_ready(city),
        DisasterKind::Hurricane => pe_hurricane_disaster_ready(city),
        DisasterKind::AirCrash => aircrash_disaster_ready(city, fidelity),
        DisasterKind::NopAirCrash | DisasterKind::HelicopterCrash => false,
        DisasterKind::Volcano | DisasterKind::Firestorm => false,
    };
    ready.then_some(kind)
}

pub(in crate::app) fn ambient_disaster_target_tile<R: AmbientDisasterRandom>(
    city: &City,
    rng: &mut R,
) -> (usize, usize) {
    for _ in 0..128 {
        let x = toolbox_random_range(rng, CITY_WIDTH);
        let y = toolbox_random_range(rng, CITY_HEIGHT);
        if is_ambient_disaster_target_tile(city, x, y) {
            return (x, y);
        }
    }
    (CITY_WIDTH / 2, CITY_HEIGHT / 2)
}

pub(in crate::app) fn is_ambient_disaster_target_tile(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    !(building == 0 || is_tree_building(building)) && !is_water_terrain(city.terrain_at(x, y))
}

pub(in crate::app) fn toolbox_random_range<R: AmbientDisasterRandom>(
    rng: &mut R,
    max: usize,
) -> usize {
    rng.random_range(max)
}

pub(in crate::app) fn pe_pollution_accident_scatter_seed(city: &City, x: usize, y: usize) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 type-0xF @ `0x45d8b0`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream for each cloud offset.
    let mut seed = 0x45D8_B000u32 ^ (x as u32).rotate_left(8) ^ (y as u32);
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

pub(in crate::app) fn pe_pollution_accident_cloud_offset(rng: &mut Win95CrtRand) -> (isize, isize) {
    let dx = (rng.random_u16() & 7) as isize - 4;
    let dy = (rng.random_u16() & 7) as isize - 4;
    (dx, dy)
}

/// Win95 type-0xF start @ `0x45d8b0`: population-scaled clouds jitter by `rand&7-4`.
pub(in crate::app) fn pe_pollution_accident_cloud_tiles_with_rng(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut Win95CrtRand,
) -> Vec<(usize, usize)> {
    let mut tiles = Vec::new();
    for _ in 0..pe_population_jitter_attempt_count(city) {
        let (dx, dy) = pe_pollution_accident_cloud_offset(rng);
        if let Some((tx, ty)) = offset_city_tile(x, y, dx, dy) {
            // PE @ `0x45d93f`: write XTXT `0xfb` on the map during each cloud attempt.
            city.set_text_overlay(tx, ty, 0xfb);
            tiles.push((tx, ty));
        }
    }
    tiles
}
