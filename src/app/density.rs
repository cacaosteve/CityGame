//! Live pollution / land-value / crime density maps (Win95 and Mac scan slices).

use super::*;

pub(super) fn live_density_maps_for_city(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
) -> LiveDensityMaps {
    live_density_maps_for_city_with_power_seed(
        city,
        population,
        outcomes,
        pe_power_recalc_seed(city),
    )
}

/// Same as [`live_density_maps_for_city`], but plant-capacity CRT uses `power_seed`
/// (`FUN_004679B0` / process CRT on the live day-2 path).
pub(super) fn live_density_maps_for_city_with_power_seed(
    city: &City,
    _population: PopulationStats,
    _outcomes: CityServiceOutcomes,
    power_seed: u32,
) -> LiveDensityMaps {
    live_density_maps_for_city_with_power_seed_and_pe_xbit(
        city,
        _population,
        _outcomes,
        power_seed,
        false,
    )
}

pub(super) fn live_density_maps_for_city_with_power_seed_and_pe_xbit(
    city: &City,
    _population: PopulationStats,
    _outcomes: CityServiceOutcomes,
    power_seed: u32,
    use_pe_xbit_power: bool,
) -> LiveDensityMaps {
    live_density_maps_for_city_with_fidelity(
        city,
        _population,
        _outcomes,
        power_seed,
        use_pe_xbit_power,
        EngineFidelity::Win95,
    )
}

pub(super) fn live_density_maps_for_city_with_fidelity(
    city: &City,
    _population: PopulationStats,
    _outcomes: CityServiceOutcomes,
    power_seed: u32,
    use_pe_xbit_power: bool,
    fidelity: EngineFidelity,
) -> LiveDensityMaps {
    // Win95 `SimulationPollutionTerrainAndLandValueScan` order:
    // pollution → land value (saved XPOP/XCRM) → police/fire maps → XPOP rebuild → crime.
    let power_maps = power_service_maps_with_entry_seed(city, power_seed);
    let water_maps = water_service_maps_with_entry_seed(city, power_seed);
    let services = rci_development_services_for_city_with_power(city, &power_maps);
    let pollution = if fidelity.uses_win95() {
        live_pollution_density_map_for_city(city)
    } else {
        live_mac_pollution_density_map_for_city(city)
    };
    // Both executables read the persisted XPOP/XCRM planes during the XVAL
    // stage, before rebuilding either map later in this worker. Keep these
    // inputs separate from the newly computed maps used by the final crime
    // pass below.
    let xval_population = city
        .population_density_slice()
        .map(|values| values.to_vec())
        .unwrap_or_else(|| vec![0; CITY_TILES]);
    let xval_crime = city
        .crime_density_slice()
        .map(|values| values.to_vec())
        .unwrap_or_else(|| vec![0; CITY_TILES]);
    let land_value_center = if fidelity.uses_win95() {
        land_value_city_center(city)
    } else {
        let (mac_x, mac_y) = mac_developed_tile_anchor(city);
        (mac_x / 2, mac_y / 2)
    };
    let land_value = live_land_value_density_map_for_city_at_center(
        city,
        &water_maps,
        &pollution,
        &xval_crime,
        &xval_population,
        land_value_center,
        fidelity.uses_win95(),
    );
    let (police, fire) = if fidelity.uses_win95() {
        (
            live_police_coverage_map_for_city_with_pe_xbit(
                city,
                &power_maps,
                &services,
                use_pe_xbit_power,
            ),
            live_fire_coverage_map_for_city_with_pe_xbit(
                city,
                &power_maps,
                &services,
                use_pe_xbit_power,
            ),
        )
    } else {
        (
            live_mac_service_coverage_map(city, 210, fidelity),
            live_mac_service_coverage_map(city, 211, fidelity),
        )
    };
    let population_density = live_population_density_map_for_city(city);
    let crime = live_crime_density_map_for_city_with_fidelity(
        city,
        &population_density,
        &police,
        &land_value,
        fidelity,
    );
    // XROG updates from the XPOP delta against the prior stored map (PE @ `0x46b437`).
    let rate_of_growth = live_rate_of_growth_map_for_city(city, &population_density);
    LiveDensityMaps {
        land_value,
        crime,
        pollution,
        police,
        fire,
        population_density,
        rate_of_growth,
    }
}

/// Pre-blur Win95 pollution words; the XVAL pass reuses this scratch plane.
fn live_pe_pollution_source_words(city: &City) -> Vec<u32> {
    const MICRO: usize = 64;
    let prior = city.pollution_density_slice();
    let mut words = vec![0_u32; MICRO * MICRO];

    for my in 0..MICRO {
        for mx in 0..MICRO {
            let tx = mx * 2;
            let ty = my * 2;
            let mut xtrf = 0_u8;
            let mut prior_cell = 0_u8;
            let mut building_sum = 0_u32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let x = tx + dx;
                    let y = ty + dy;
                    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
                        continue;
                    }
                    xtrf = xtrf.max(city.traffic_density_at(x, y).unwrap_or(0));
                    if let Some(values) = prior {
                        prior_cell = prior_cell.max(values[density_map_index(x, y)]);
                    }
                    building_sum = building_sum
                        .saturating_add(pollution_weight_for_building(city.building_at(x, y)));
                }
            }
            words[my * MICRO + mx] = u32::from(xtrf) / 5 + u32::from(prior_cell) + building_sum;
        }
    }

    words
}

pub(super) fn live_pollution_density_map_for_city(city: &City) -> Vec<u8> {
    // Win95 `SimulationPollutionTerrainAndLandValueScan` pollution pass:
    // 64×64 cells = XTRF/5 + prior XPLT + building table / tile-5, then neighbor blur.
    const MICRO: usize = 64;
    let words = live_pe_pollution_source_words(city);

    // Win95 blur base: `wSewerBonus - wIndustrialMixPollutionBonus + 4`
    // (+1 when pollution-controls ordinance bit `0x80000` is set), floored at 1.
    let blur_base = pollution_blur_divisor_base_for_city(city);
    let mut micro = vec![0_u8; MICRO * MICRO];
    for my in 0..MICRO {
        for mx in 0..MICRO {
            let index = my * MICRO + mx;
            let mut total = i32::try_from(words[index].saturating_mul(2)).unwrap_or(i32::MAX);
            let mut divisor = blur_base;
            if my > 0 {
                divisor += 1;
                total =
                    total.saturating_add(i32::try_from(words[index - MICRO]).unwrap_or(i32::MAX));
            }
            if my + 1 < MICRO {
                divisor += 1;
                total =
                    total.saturating_add(i32::try_from(words[index + MICRO]).unwrap_or(i32::MAX));
            }
            if mx > 0 {
                divisor += 1;
                total = total.saturating_add(i32::try_from(words[index - 1]).unwrap_or(i32::MAX));
            }
            if mx + 1 < MICRO {
                divisor += 1;
                total = total.saturating_add(i32::try_from(words[index + 1]).unwrap_or(i32::MAX));
            }
            // PE applies pollution controls to the divisor base above; it does not
            // apply an additional percentage scale after the neighborhood average.
            micro[index] = (total / divisor).clamp(0, 255) as u8;
        }
    }

    let mut values = vec![0_u8; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            values[density_map_index(x, y)] = micro[(y / 2) * MICRO + (x / 2)];
        }
    }
    values
}

/// Mac `FUN_00027AB4` pollution producer (`0x27BB6..0x27CC6`). The source
/// field is native 64×64 XTRF/ XPLT; the four XBLD reads cover the matching
/// 2×2 full-map footprint. Its edge divisor comes from the two persisted WORD
/// globals at MISC dwords 89 and 95, rather than the PE sewer/ordinance terms.
pub(super) fn live_mac_pollution_density_map_for_city(city: &City) -> Vec<u8> {
    let words = live_mac_pollution_source_words(city);
    mac_pollution_blur_from_source_words(city, &words)
}

/// Rebuilds the Mac worker's native pollution WORD plane. The plane is shared
/// with the later XVAL builder, so keeping these pre-blur words is necessary to
/// reproduce that pass's order-dependent terrain baseline.
fn live_mac_pollution_source_words(city: &City) -> Vec<u16> {
    const MICRO: usize = 64;
    let prior = city.pollution_density_slice();
    let mut words = vec![0_u16; MICRO * MICRO];

    for my in 0..MICRO {
        for mx in 0..MICRO {
            let tx = mx * 2;
            let ty = my * 2;
            let traffic = u32::from(city.traffic_density_at(tx, ty).unwrap_or(0));
            let prior_cell = prior
                .and_then(|values| values.get(density_map_index(tx, ty)))
                .copied()
                .unwrap_or(0);
            let mut building_sum = 0_u32;
            for dy in 0..2 {
                for dx in 0..2 {
                    building_sum = building_sum.saturating_add(pollution_weight_for_building(
                        city.building_at(tx + dx, ty + dy),
                    ));
                }
            }
            // Mac performs the traffic division before adding the prior XPLT
            // byte and each of the four tile contributions.
            words[my * MICRO + mx] = (traffic / 5 + u32::from(prior_cell) + building_sum) as u16;
        }
    }

    words
}

#[cfg(test)]
pub(super) fn mac_pollution_source_words_for_test(city: &City) -> Vec<u16> {
    live_mac_pollution_source_words(city)
}

#[cfg(test)]
pub(super) fn mac_xval_terrain_grids_for_test(city: &City) -> (Vec<i16>, Vec<i16>) {
    mac_xval_terrain_grids(city)
}

fn mac_pollution_blur_from_source_words(city: &City, words: &[u16]) -> Vec<u8> {
    const MICRO: usize = 64;
    let divisor_adjustment_a = i32::from(city.mac_pollution_adjustment());
    let divisor_adjustment_b = i32::from(city.mac_treatment_gate());
    // 68k @ 0x27bfa tests bit 19 of the ordinance mask and adds one.
    let divisor_base = 4 - divisor_adjustment_a
        + divisor_adjustment_b
        + i32::from(ordinance_enabled_for_city(
            city,
            ORDINANCE_POLLUTION_CONTROLS,
        ));

    let mut micro = vec![0_u8; MICRO * MICRO];
    for my in 0..MICRO {
        for mx in 0..MICRO {
            let index = my * MICRO + mx;
            let mut total = i32::from(words[index]) * 2;
            let mut divisor = divisor_base;
            if my > 0 {
                divisor += 1;
                total += i32::from(words[index - MICRO]);
            }
            if my + 1 < MICRO {
                divisor += 1;
                total += i32::from(words[index + MICRO]);
            }
            if mx > 0 {
                divisor += 1;
                total += i32::from(words[index - 1]);
            }
            if mx + 1 < MICRO {
                divisor += 1;
                total += i32::from(words[index + 1]);
            }
            // The original city state keeps this divisor positive. Avoid a
            // Rust panic if a malformed save supplies an invalid combination.
            micro[index] = if divisor > 0 {
                (total / divisor).clamp(0, 255) as u8
            } else {
                0
            };
        }
    }

    let mut values = vec![0_u8; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            values[density_map_index(x, y)] = micro[(y / 2) * MICRO + (x / 2)];
        }
    }
    values
}

/// Win95 pollution blur divisor base @ `0x46a9a3`:
/// `max(1, sewer_bonus - industrial_mix_pollution_bonus + 4 [+1 if pollution controls])`.
pub(super) fn pollution_blur_divisor_base_for_city(city: &City) -> i32 {
    let mut base = i32::from(sewer_bonus_for_pollution_blur(city))
        - i32::from(industrial_mix_pollution_bonus_for_blur(city))
        + 4;
    if ordinance_enabled_for_city(city, ORDINANCE_POLLUTION_CONTROLS) {
        base += 1;
    }
    base.max(1)
}

/// Win95 `wSewerBonus` (`0x4cb3dc`): PE pollution blur @ `0x46a9a5` reads BSS.
pub(super) fn sewer_bonus_for_pollution_blur(city: &City) -> i16 {
    city.pe_w_sewer_bonus() as i16
}

/// PE water-scan formula (`0x46488c`/`0x4648a2`): 1 when the served-water
/// numerator meets 2,000 per Water Treatment tile-footprint.
pub(super) fn sewer_bonus_from_water_scan(city: &City) -> i16 {
    let entry_seed = pe_power_recalc_seed(city);
    let water_maps =
        water_service_maps_with_entry_seed_and_fidelity(city, entry_seed, EngineFidelity::Win95);
    let served = water_maps.pe_served_worker_counter(city);
    let treatment_tiles = u32::from(city.pe_dw_tile_count[244]);
    let threshold = treatment_tiles.div_ceil(4).saturating_mul(2_000);
    if served >= threshold { 1 } else { 0 }
}

/// PE `FUN_00464550` finalizes `0x4CC4B8` as the worker's served-cell count
/// divided by raw source capacity. Return `None` when a component cannot be
/// reconciled with the saved-XBIT worker flood so callers can retain their
/// established estimate without presenting it as recovered PE behavior.
pub(super) fn pe_water_worker_utilization_percent(city: &City, entry_seed: u32) -> Option<u32> {
    let maps =
        water_service_maps_with_entry_seed_and_fidelity(city, entry_seed, EngineFidelity::Win95);
    maps.pe_worker_utilization_percent(city)
}

/// Publish PE `wSewerBonus` BSS + MISC from served water and raw plant tiles.
pub(super) fn publish_pe_sewer_bonus(city: &mut City) {
    let entry_seed = pe_power_recalc_seed(city);
    let water_maps =
        water_service_maps_with_entry_seed_and_fidelity(city, entry_seed, EngineFidelity::Win95);
    city.publish_pe_sewer_bonus(
        water_maps.pe_served_worker_counter(city),
        u32::from(city.pe_dw_tile_count[244]),
    );
}

/// Win95 `wIndustrialMixBonus` (`0x4ca1e8`): PE `FUN_00468150` @ `0x4682be` reads BSS.
pub(super) fn industrial_mix_bonus_for_city(city: &City) -> i16 {
    city.pe_w_industrial_mix_bonus() as i16
}

/// Tile-scan formula matching PE max-share `(max-20)/5` (tests / archaeology).
pub(super) fn industrial_mix_bonus_from_tiles(city: &City) -> i16 {
    let counts = industrial_tile_counts_for_mix_bonus(city);
    let total = counts.iter().copied().sum::<u32>();
    let max_share = counts
        .iter()
        .map(|count| count.saturating_mul(100) / total.saturating_add(1))
        .max()
        .unwrap_or(0);
    if max_share < 20 {
        0
    } else {
        ((max_share - 20) / 5) as i16
    }
}

/// Win95 `wIndustrialMixPollutionBonus` (`0x4c9428`): PE pollution blur @ `0x46a9b2` reads BSS.
pub(super) fn industrial_mix_pollution_bonus_for_blur(city: &City) -> i16 {
    city.pe_w_industrial_mix_pollution_bonus()
}

/// Tile-scan formula matching PE dirty-share score (tests / archaeology).
pub(super) fn industrial_mix_pollution_bonus_from_tiles(city: &City) -> i16 {
    let counts = industrial_tile_counts_for_mix_bonus(city);
    let total = counts.iter().copied().sum::<u32>();
    let dirty = counts[0]
        .saturating_add(counts[1])
        .saturating_add(counts[2])
        .saturating_add(counts[5]);
    let score = dirty.saturating_mul(100) / total.saturating_add(1);
    if score < 20 {
        -1
    } else {
        ((score - 20) / 30) as i16
    }
}

fn industrial_tile_counts_for_mix_bonus(city: &City) -> [u32; INDUSTRY_COUNT] {
    let mut counts = [0_u32; INDUSTRY_COUNT];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some(index) = industrial_building_industry_index(city.building_at(x, y)) else {
                continue;
            };
            counts[index] = counts[index].saturating_add(1);
        }
    }
    counts
}

/// Win95 pollution building weight (`0x4e95b8`) plus the hard-coded tile-5 term (`+0xC8`).
pub(super) fn pollution_weight_for_building(building: u8) -> u32 {
    if building == 5 {
        return 0xc8;
    }
    if building < 0x70 {
        return 0;
    }
    u32::from(POLLUTION_BUILDING_WEIGHT_TABLE[building as usize])
}

/// Bytes at `SIMCITY.EXE!0x4e95b8` (indexed by XBLD id).
const POLLUTION_BUILDING_WEIGHT_TABLE: [u8; 256] = {
    let mut table = [0_u8; 256];
    table[0x84] = 6;
    table[0x85] = 6;
    table[0x86] = 6;
    table[0x87] = 6;
    table[0x9e] = 12;
    table[0x9f] = 12;
    table[0xa0] = 12;
    table[0xa1] = 12;
    table[0xa2] = 18;
    table[0xa3] = 18;
    table[0xa4] = 18;
    table[0xa5] = 18;
    table[0xbc] = 24;
    table[0xbd] = 24;
    table[0xbe] = 24;
    table[0xbf] = 24;
    table[0xc0] = 24;
    table[0xc1] = 24;
    table[0xc9] = 10;
    table[0xca] = 25;
    table[0xcb] = 2;
    table[0xce] = 2;
    table[0xcf] = 50;
    table[0xd7] = 4;
    table[0xd8] = 10;
    table[0xdc] = 2;
    table[0xdd] = 10;
    table[0xde] = 10;
    table[0xdf] = 10;
    table[0xe0] = 5;
    table[0xe3] = 5;
    table[0xe4] = 5;
    table[0xe5] = 5;
    table[0xe6] = 10;
    table[0xe7] = 10;
    table[0xe9] = 5;
    table[0xec] = 3;
    table[0xed] = 4;
    table[0xee] = 2;
    table[0xef] = 2;
    table[0xf0] = 2;
    table[0xf1] = 2;
    table[0xf2] = 10;
    table[0xf4] = 10;
    table[0xf6] = 5;
    table[0xfb] = 25;
    table[0xfc] = 10;
    table[0xfd] = 12;
    table[0xfe] = 15;
    table
};

/// Win95 `XPLC` paint: `(wPrisonBonus+5)×funding/2`, halved when unpowered, ring falloff on 32×32.
/// Mac `FUN_00027AB4` paints via `0x28884` into the `ffffeb40` row-pointer plane (XPLC).
pub(super) fn live_police_coverage_map_for_city(
    city: &City,
    power_maps: &PowerServiceMaps,
    services: &RciDevelopmentServices,
) -> Vec<u8> {
    live_police_coverage_map_for_city_with_pe_xbit(city, power_maps, services, false)
}

pub(super) fn live_police_coverage_map_for_city_with_pe_xbit(
    city: &City,
    power_maps: &PowerServiceMaps,
    _services: &RciDevelopmentServices,
    use_pe_xbit_power: bool,
) -> Vec<u8> {
    live_pe_station_coverage_map(
        city,
        power_maps,
        210,
        use_pe_xbit_power,
        pe_police_coverage_strength(
            budget_service_funding_raw_for_city(city, 0),
            prison_bonus_for_city(city),
        ),
    )
}

/// Win95 `XFIR` paint: budget×5/2 strength, halved when unpowered, ring falloff on 32×32.
/// Mac `FUN_00027AB4` paints via `0x28884` into the `ffffebc0` row-pointer plane (XFIR).
pub(super) fn live_fire_coverage_map_for_city(
    city: &City,
    power_maps: &PowerServiceMaps,
    services: &RciDevelopmentServices,
) -> Vec<u8> {
    live_fire_coverage_map_for_city_with_pe_xbit(city, power_maps, services, false)
}

pub(super) fn live_fire_coverage_map_for_city_with_pe_xbit(
    city: &City,
    power_maps: &PowerServiceMaps,
    _services: &RciDevelopmentServices,
    use_pe_xbit_power: bool,
) -> Vec<u8> {
    live_pe_station_coverage_map(
        city,
        power_maps,
        211,
        use_pe_xbit_power,
        pe_fire_coverage_strength(budget_service_funding_raw_for_city(city, 1)),
    )
}

/// Police: signed `wPrisonBonus + 5`, wrapping DWORD multiply by raw saved
/// funding, then signed divide-by-two (PE @ `0x46b331`).
pub(super) fn pe_police_coverage_strength(funding: u32, prison_bonus: u32) -> i32 {
    i32::from(prison_bonus as u16 as i16)
        .wrapping_add(5)
        .wrapping_mul(funding as i32)
        / 2
}

/// Win95 `wOldArrests` (`0x4CA430` / MISC `0x1038`): PE reads BSS; year-end police writes it.
pub(super) fn police_old_arrests_for_city(city: &City) -> u32 {
    u32::from(city.pe_w_old_arrests())
}

/// Sum of police XMIC `stat3` (arrests) — archaeology / formula check.
pub(super) fn police_arrests_sum_from_microsims(city: &City) -> u32 {
    let mut total = 0_u32;
    for index in MICROSIM_STRUCTURE_FIRST..MICROSIM_COUNT {
        let Some(record) = city.microsim_at(index) else {
            continue;
        };
        if record.tile_id != 210 {
            continue;
        }
        total = total.saturating_add(u32::from(record.stat3));
        if total >= 0xffff {
            return 0xffff;
        }
    }
    total
}

/// Win95 `wPrisonBonus` (`0x4CA1DC` / MISC `0x103C`): PE coverage @ `0x46b334` uses MOVSX.
pub(super) fn prison_bonus_for_city(city: &City) -> u32 {
    u32::from(city.pe_w_prison_bonus())
}

/// Fire: wrapping DWORD `funding * 5`, then signed divide-by-two.
pub(super) fn pe_fire_coverage_strength(funding: u32) -> i32 {
    (funding as i32).wrapping_mul(5) / 2
}

fn live_pe_station_coverage_map(
    city: &City,
    power_maps: &PowerServiceMaps,
    building: u8,
    use_pe_xbit_power: bool,
    base_strength: i32,
) -> Vec<u8> {
    const MICRO: usize = 32;
    let mut micro = vec![0_u8; MICRO * MICRO];
    let ordinance = match building {
        210 => ORDINANCE_NEIGHBORHOOD_WATCH,
        211 => ORDINANCE_VOLUNTEER_FIRE_DEPT,
        _ => return upscale_coverage_32_to_density_map(&micro),
    };
    let ordinance_enabled = ordinance_enabled_for_city(city, ordinance);

    // PE `FUN_0046A840` scans individual interior map cells and calls
    // `FUN_0046BA90` for every eligible XBLD station tile. A 3×3 station is
    // therefore nine ring sources, not one source at the footprint center.
    // In the same x-major pass, each developed tile (`0x70..=0xc5`) adds two
    // points to the ordinance-selected plane, even when station funding is 0.
    for x in 1..CITY_WIDTH - 1 {
        for y in 1..CITY_HEIGHT - 1 {
            let tile = city.building_at(x, y);
            if ordinance_enabled && (0x70..0xc6).contains(&tile) {
                let mx = x >> 2;
                let my = y >> 2;
                let index = mx * MICRO + my;
                if micro[index] < 0xfe {
                    micro[index] += 2;
                }
            }

            if base_strength == 0 || tile != building {
                continue;
            }
            let Some(tile_bits) = city.tile_bits_at(x, y) else {
                continue;
            };
            // PE gates every source tile on XBIT 0x80, then halves its
            // contribution when that same tile lacks connected XBIT 0x40.
            if !tile_bits.powerable() {
                continue;
            }
            let connected = if use_pe_xbit_power {
                tile_bits.powered()
            } else {
                power_maps.energized_tiles[y * CITY_WIDTH + x]
            };
            // `FUN_0046BA90` consumes a signed WORD. For the unpowered arm,
            // PE first MOVSXes the low WORD and then halves it.
            let mut strength = base_strength as i16;
            if !connected {
                strength /= 2;
            }
            if strength == 0 {
                continue;
            }

            // PE's signed divide-by-four has no correction for positive tiles.
            let mx = (x >> 2) as i32;
            let my = (y >> 2) as i32;
            paint_pe_coverage_rings(&mut micro, mx, my, strength);
        }
    }

    upscale_coverage_32_to_density_map(&micro)
}

/// Mac `FUN_00027AB4`: ordinance-gated developed-tile baseline plus one
/// orientation-selected station anchor per XZON-marked footprint. The Mac
/// worker consumes saved XBIT `0x40`; it does not substitute the geometry graph.
pub(super) fn live_mac_service_coverage_map(
    city: &City,
    building: u8,
    fidelity: EngineFidelity,
) -> Vec<u8> {
    const MICRO: usize = 32;
    let mut micro = vec![0_u8; MICRO * MICRO];
    let ordinance = match building {
        210 => ORDINANCE_NEIGHBORHOOD_WATCH,
        211 => ORDINANCE_VOLUNTEER_FIRE_DEPT,
        _ => return upscale_coverage_32_to_density_map(&micro),
    };

    let ordinance_enabled = ordinance_enabled_for_city_with_fidelity(city, ordinance, fidelity);
    let funding = budget_service_funding_raw_for_city(city, if building == 210 { 0 } else { 1 });
    let police_adjustment = if building == 210 {
        // CODE 4 reads cursor 1039 at payload byte `1039*4 + 2` and
        // saveback writes the sign-extended WORD at payload dword 1039.
        i32::from(city.mac_police_coverage_gate())
    } else {
        0
    };
    // The Mac worker uses `MULU.L` on the saved funding dword, then signed
    // long division by two. Its station helper receives only the low WORD.
    let base = (5 + police_adjustment).wrapping_mul(funding as i32) / 2;
    // CODE 14 scans the first coordinate outside the second and applies each
    // developed-tile bonus or station stamp immediately. The bonus stops at
    // 254, so moving all bonuses ahead of the stamps changes saturation.
    for x in 1..CITY_WIDTH - 1 {
        for y in 1..CITY_HEIGHT - 1 {
            let tile = city.building_at(x, y);
            if ordinance_enabled && (0x70..0xc6).contains(&tile) {
                let index = (x / 4) * MICRO + (y / 4);
                if micro[index] < 0xfe {
                    micro[index] += 2;
                }
            }
            if tile != building || !city.zone_at(x, y).is_some_and(|zone| zone.left()) {
                continue;
            }
            let connected = city.tile_bits_at(x, y).is_some_and(TileBits::powered);
            let strength = if connected {
                base as i16
            } else {
                // The unpowered arm sign-extends D4's low WORD before
                // `DIVS.W #2`; it does not halve the full longword.
                (i32::from(base as i16) / 2) as i16
            };
            if strength != 0 {
                paint_pe_coverage_rings(&mut micro, (x / 4) as i32, (y / 4) as i32, strength);
            }
        }
    }
    upscale_coverage_32_to_density_map(&micro)
}

/// Win95 `FUN_0046ba90` / Mac `0x28884` coverage stamp: concentric rings with decaying strength.
///
/// Mac add helper `0x28814` (PE thunk `0x401bd6` → `0x46ba20`): signed-WORD add
/// clamped to `0..=255`, bounds `[0,32)`, index via row-pointer table `base[x] + y`
/// (flat equivalent `x * 32 + y`).
/// Decay chain: full → `×4/5` cardinals → `×3/4` diagonals → `×2/3` r=2 ring → `/2` outer ring.
pub(super) fn paint_pe_coverage_rings(micro: &mut [u8], x: i32, y: i32, strength: i16) {
    const MICRO: i32 = 32;
    let add = |micro: &mut [u8], px: i32, py: i32, amount: i16| {
        if !(0..MICRO).contains(&px) || !(0..MICRO).contains(&py) || amount == 0 {
            return;
        }
        let index = (px as usize) * (MICRO as usize) + (py as usize);
        // PE 0x46ba5d and Mac 0x28846 add in a signed WORD before
        // comparing against the output-byte limits.
        let summed = amount.wrapping_add(i16::from(micro[index]));
        micro[index] = summed.clamp(0, 255) as u8;
    };

    let mut s = strength;
    add(micro, x, y, s);

    s = (i32::from(s) * 4 / 5) as i16;
    add(micro, x - 1, y, s);
    add(micro, x + 1, y, s);
    add(micro, x, y - 1, s);
    add(micro, x, y + 1, s);

    s = (i32::from(s) * 3 / 4) as i16;
    add(micro, x - 1, y - 1, s);
    add(micro, x + 1, y - 1, s);
    add(micro, x - 1, y + 1, s);
    add(micro, x + 1, y + 1, s);

    s = (i32::from(s) * 2 / 3) as i16;
    for &(dx, dy) in &[
        (-2, -1),
        (-2, 0),
        (-2, 1),
        (2, -1),
        (2, 0),
        (2, 1),
        (-1, -2),
        (0, -2),
        (1, -2),
        (-1, 2),
        (0, 2),
        (1, 2),
    ] {
        add(micro, x + dx, y + dy, s);
    }

    s /= 2;
    for &(dx, dy) in &[
        (-3, -1),
        (-3, 0),
        (-3, 1),
        (3, -1),
        (3, 0),
        (3, 1),
        (-1, -3),
        (0, -3),
        (1, -3),
        (-1, 3),
        (0, 3),
        (1, 3),
        (-2, -2),
        (2, -2),
        (-2, 2),
        (2, 2),
    ] {
        add(micro, x + dx, y + dy, s);
    }
}

fn upscale_coverage_32_to_density_map(micro: &[u8]) -> Vec<u8> {
    const OLD: usize = 32;
    let mut out = vec![0_u8; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let sx = x * OLD / CITY_WIDTH;
            let sy = y * OLD / CITY_HEIGHT;
            out[density_map_index(x, y)] = micro[sx * OLD + sy];
        }
    }
    out
}

pub(super) fn live_land_value_density_map_for_city(
    city: &City,
    _water_maps: &WaterServiceMaps,
    pollution: &[u8],
    crime: &[u8],
    population_density: &[u8],
) -> Vec<u8> {
    live_land_value_density_map_for_city_at_center(
        city,
        _water_maps,
        pollution,
        crime,
        population_density,
        land_value_city_center(city),
        true,
    )
}

pub(super) fn live_land_value_density_map_for_city_at_center(
    city: &City,
    // Retained at this internal boundary for focused comparisons. The rooted
    // Win95 XVAL worker reads saved XBIT bit 0x10 directly instead of using a
    // freshly computed service-assignment map.
    _water_maps: &WaterServiceMaps,
    pollution: &[u8],
    crime: &[u8],
    population_density: &[u8],
    center: (usize, usize),
    pe_grid_scale: bool,
) -> Vec<u8> {
    const TERRAIN_GRID: usize = 32;
    let (terrain, industrial_terrain) = if pe_grid_scale {
        let words = live_pe_pollution_source_words(city);
        let words = words
            .into_iter()
            .map(|word| word as u16)
            .collect::<Vec<_>>();
        shared_xval_terrain_grids(city, &words)
    } else {
        mac_xval_terrain_grids(city)
    };

    let mut values = vec![0_u8; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = density_map_index(x, y);
            let mac = !pe_grid_scale;
            // Both native workers produce one XVAL cell for each 2x2 area.
            let (sample_x, sample_y) = (x & !1, y & !1);
            let zone = mac_xval_zone_at_output(city, sample_x, sample_y);
            let building = city.building_at(sample_x, sample_y);
            let developed = if mac {
                mac_xval_output_is_eligible(city, sample_x, sample_y)
            } else {
                pe_xval_output_is_eligible(city, sample_x, sample_y)
            };
            if !developed {
                values[index] = 0;
                continue;
            }

            let gx = x / 4;
            let gy = y / 4;
            let terrain_average = if mac {
                let grid = if matches!(zone, ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE) {
                    &industrial_terrain
                } else {
                    &terrain
                };
                mac_land_value_neighbor_average_terrain_grid(grid, gx, gy, TERRAIN_GRID)
            } else if matches!(zone, ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE) {
                pe_industrial_land_value_neighbor_average(
                    &terrain,
                    &industrial_terrain,
                    gx,
                    gy,
                    TERRAIN_GRID,
                )
            } else {
                land_value_neighbor_average_terrain_grid(&terrain, gx, gy, TERRAIN_GRID)
            };
            let mut value = terrain_average;
            value += if pe_grid_scale {
                land_value_center_bonus(zone, sample_x, sample_y, center)
            } else {
                mac_land_value_center_bonus(zone, x / 2, y / 2, center)
            };
            let sample_index = density_map_index(sample_x, sample_y);
            let pollution_byte = pollution.get(sample_index).copied().unwrap_or(0);
            let crime_byte = crime.get(sample_index).copied().unwrap_or(0);
            value -= if pe_grid_scale {
                land_value_pollution_penalty(zone, pollution_byte)
            } else {
                mac_land_value_pollution_penalty(zone, pollution_byte)
            };
            value -= if pe_grid_scale {
                land_value_crime_penalty(zone, crime_byte)
            } else {
                mac_land_value_crime_penalty(zone, crime_byte)
            };
            if zone == ZONE_INDUSTRIAL_DENSE {
                value += 0x15;
            }
            let pop = population_density.get(sample_index).copied().unwrap_or(0);
            value += if pe_grid_scale {
                land_value_population_adjustment(zone, pop)
            } else {
                mac_land_value_population_adjustment(zone, pop)
            };
            if is_abandoned_building(building) {
                // PE `0x46b0fc`: `ebx -= toward_zero(ebx/2)`, not floor-halve.
                value = land_value_apply_abandoned_halve(value);
            }
            values[index] = value.clamp(0, 255) as u8;
        }
    }
    values
}

/// Mac `FUN_00027AB4 @ 0x27f88..0x27fc2` zone sample for one native XVAL
/// output coordinate. It checks the even/even corner first, then the
/// opposite odd/odd corner if the first low nibble is zero.
pub(super) fn mac_xval_zone_at_output(city: &City, x: usize, y: usize) -> u8 {
    let x0 = x & !1;
    let y0 = y & !1;
    let first = city.zone_at(x0, y0).map(|zone| zone.zone_id()).unwrap_or(0);
    if first != 0 {
        return first;
    }
    city.zone_at(x0 + 1, y0 + 1)
        .map(|zone| zone.zone_id())
        .unwrap_or(0)
}

/// Mac clears XBIT bit 3, then projects eligibility from each 128×128 tile
/// into its 64×64 output cell. Any tile ID >= 0x1d or a nonzero XZON nibble
/// sets the corresponding output mask bit (`0x27e68..0x27e92`).
pub(super) fn mac_xval_output_is_eligible(city: &City, x0: usize, y0: usize) -> bool {
    (0..2).any(|dx| {
        (0..2).any(|dy| {
            let x = x0 + dx;
            let y = y0 + dy;
            city.building_at(x, y) >= 0x1d
                || city.zone_at(x, y).is_some_and(|zone| zone.zone_id() != 0)
        })
    })
}

/// PE clears bit 3 only at XBLD >= 0x70 before projecting the source
/// predicate onto the 64x64 region; preexisting bits elsewhere survive.
pub(super) fn pe_xval_output_is_eligible(city: &City, x0: usize, y0: usize) -> bool {
    let (mask_x, mask_y) = (x0 / 2, y0 / 2);
    let survives_clear = city.building_at(mask_x, mask_y) < 0x70
        && city
            .tile_bits_at(mask_x, mask_y)
            .is_some_and(|bits| bits.raw_bits() & 0x08 != 0);
    survives_clear || mac_xval_output_is_eligible(city, x0, y0)
}

/// The Mac XVAL pass reuses the 128×128 WORD plane left by pollution, writes
/// `0x28` at every developed full-map coordinate, and then folds every tile
/// into two 32×32 WORD grids. This intentionally retains the original
/// order-dependent baseline instead of substituting the PE terrain table.
pub(super) fn mac_xval_terrain_grids(city: &City) -> (Vec<i16>, Vec<i16>) {
    shared_xval_terrain_grids(city, &live_mac_pollution_source_words(city))
}

/// PE and Mac reuse the pollution WORD plane for the subsequent XVAL pass.
fn shared_xval_terrain_grids(city: &City, source_words: &[u16]) -> (Vec<i16>, Vec<i16>) {
    const GRID: usize = 32;
    let mut scratch = vec![0_u16; CITY_TILES];

    // FUN_00027AB4 writes native cell (x,y) directly to row-pointer x and
    // WORD column y in the shared 128×128 plane. XVAL then reuses those words
    // from the plane's upper-left 64×64 region.
    for native_y in 0..64 {
        for native_x in 0..64 {
            scratch[density_map_index(native_x, native_y)] = source_words[native_y * 64 + native_x];
        }
    }

    // The center pass overwrites the same plane at full resolution before the
    // coarse terrain accumulation begins.
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            if city.building_at(x, y) >= 0x70 {
                scratch[density_map_index(x, y)] = 0x28;
            }
        }
    }

    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let building = city.building_at(x, y);
            let bits = city.tile_bits_at(x, y).map(TileBits::raw_bits).unwrap_or(0);
            let gx = x / 4;
            let gy = y / 4;
            let primary_index = density_map_index(gx, gy);
            let industrial_index = density_map_index(gx + GRID, gy);
            let mut primary_word = scratch[primary_index];
            let mut industrial_word = scratch[industrial_index];

            if building == 0 {
                if bits & 0x04 != 0 {
                    primary_word = primary_word.wrapping_add(0x0c);
                    industrial_word = industrial_word.wrapping_add(0x0c);
                } else {
                    primary_word = primary_word.wrapping_add(4);
                }
            } else if building == 0xd5 {
                primary_word = primary_word.wrapping_add(0x28);
            } else if (6..=13).contains(&building) {
                primary_word = primary_word.wrapping_add(0x14);
            } else if building <= 5 {
                primary_word = primary_word.wrapping_sub(0x14);
            }

            if bits & 0x10 != 0 {
                primary_word = primary_word.wrapping_add(4);
                industrial_word = industrial_word.wrapping_add(4);
            }
            // Mac FUN_00027AB4 reads A5-0x8bc here. FUN_000321f0 and the
            // CITY layer loader identify that row-pointer table as XTER;
            // XTXT is the separate A5-0x4bc table.
            if (1..=15).contains(&city.terrain_at(x, y)) {
                primary_word = primary_word.wrapping_add(0x0c);
            }

            scratch[primary_index] = primary_word;
            scratch[industrial_index] = industrial_word;
            debug_assert_eq!(primary_index, density_map_index(gx, gy));
            debug_assert_eq!(industrial_index, density_map_index(gx + GRID, gy));
        }
    }

    let primary = (0..GRID * GRID)
        .map(|index| scratch[density_map_index(index / GRID, index % GRID)] as i16)
        .collect();
    let industrial = (0..GRID * GRID)
        .map(|index| scratch[density_map_index(index / GRID + GRID, index % GRID)] as i16)
        .collect();
    (primary, industrial)
}

pub(super) fn mac_land_value_neighbor_average_terrain_grid(
    terrain: &[i16],
    gx: usize,
    gy: usize,
    grid: usize,
) -> i32 {
    let mut total = i32::from(terrain[gx * grid + gy]);
    let mut count = 1_i32;
    if gx > 0 {
        total += i32::from(terrain[(gx - 1) * grid + gy]);
        count += 1;
    }
    if gx + 1 < grid {
        total += i32::from(terrain[(gx + 1) * grid + gy]);
        count += 1;
    }
    if gy > 0 {
        total += i32::from(terrain[gx * grid + gy - 1]);
        count += 1;
    }
    if gy + 1 < grid {
        total += i32::from(terrain[gx * grid + gy + 1]);
        count += 1;
    }
    total / count
}

/// Win95 XPOP rebuild in `SimulationPollutionTerrainAndLandValueScan` @ `0x46b1e3`/`0x46b402`:
/// scan interior source tiles 1..=126 on each axis, sum building weights on a
/// 32×32 grid, then `min(255, sum << 2)`. Mac `FUN_00027AB4 @ 0x283e6..0x285c4`
/// uses the same interior bounds and 4×4 coarse-cell projection.
pub(super) fn live_population_density_map_for_city(city: &City) -> Vec<u8> {
    const GRID: usize = 32;
    let mut words = vec![0_i32; GRID * GRID];
    for y in 1..CITY_HEIGHT - 1 {
        for x in 1..CITY_WIDTH - 1 {
            let weight = population_density_weight_for_building(city.building_at(x, y));
            if weight != 0 {
                words[(x / 4) * GRID + (y / 4)] += i32::from(weight);
            }
        }
    }
    let mut micro = vec![0_u8; GRID * GRID];
    for (index, word) in words.iter().copied().enumerate() {
        micro[index] = (word.saturating_mul(4)).clamp(0, 255) as u8;
    }
    upscale_coverage_32_to_density_map(&micro)
}

fn population_density_weight_for_building(building: u8) -> u8 {
    if (0x70..0xc6).contains(&building) {
        LAND_VALUE_BUILDING_WEIGHT_TABLE[building as usize]
    } else if (0xfb..=0xfe).contains(&building) {
        0x0c
    } else if building >= 0xc6 {
        // Non-arcology specials in the same scan arm (`+2`).
        2
    } else {
        0
    }
}

/// Native Win95 and Mac new-city XROG fill (`0x7F` = no change).
pub(super) const RATE_OF_GROWTH_NEUTRAL: u8 = 0x7f;

/// Win95 XROG update paired with XPOP write @ `0x46b437`:
/// `next = ((old * 7) + (delta * 8) + 0x80) / 8`, clamped to 0..255.
pub(super) fn live_rate_of_growth_map_for_city(city: &City, new_population: &[u8]) -> Vec<u8> {
    const GRID: usize = 32;
    let prior_population = city.population_density_slice();
    let prior_growth = city.rate_of_growth_slice();
    let mut micro = vec![RATE_OF_GROWTH_NEUTRAL; GRID * GRID];
    for gy in 0..GRID {
        for gx in 0..GRID {
            let tx = gx * 4;
            let ty = gy * 4;
            let index = density_map_index(tx, ty);
            let new_pop = new_population.get(index).copied().unwrap_or(0);
            // Missing prior XPOP ⇒ delta 0 so the first live pass keeps neutral growth.
            let old_pop = prior_population
                .and_then(|values| values.get(index).copied())
                .unwrap_or(new_pop);
            let old_growth = prior_growth
                .and_then(|values| values.get(index).copied())
                .unwrap_or(RATE_OF_GROWTH_NEUTRAL);
            micro[gx * GRID + gy] = pe_rate_of_growth_step(old_growth, old_pop, new_pop);
        }
    }
    upscale_coverage_32_to_density_map(&micro)
}

pub(super) fn pe_rate_of_growth_step(old_growth: u8, old_pop: u8, new_pop: u8) -> u8 {
    let delta = i32::from(new_pop) - i32::from(old_pop);
    let next = (i32::from(old_growth) * 7 + delta * 8 + 0x80) / 8;
    next.clamp(0, 255) as u8
}

pub(super) fn land_value_city_center(city: &City) -> (usize, usize) {
    let mut sum_x = 0_u32;
    let mut sum_y = 0_u32;
    // PE `FUN_0046A840` initializes the divisor to one before this scan, then
    // increments it once for each XBLD >= 0x70. The worker divides each sum
    // by `2*divisor`, then doubles the truncated quotient; preserve that
    // even-coordinate result (including (0, 0) on an empty developed map).
    let mut divisor = 1_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = city.building_at(x, y);
            if building < 0x70 {
                continue;
            }
            sum_x += x as u32;
            sum_y += y as u32;
            divisor += 1;
        }
    }
    (
        ((sum_x / (divisor * 2)) * 2) as usize,
        ((sum_y / (divisor * 2)) * 2) as usize,
    )
}

/// Mac `FUN_00027AB4 @ 0x27cca..0x27db2` rebuilds its active coordinate pair
/// from every XBLD tile `>= 0x70`. The 68k code initializes the divisor to one,
/// adds each qualifying tile coordinate, doubles the divisor, performs an
/// unsigned divide, then doubles the quotient word before storing the pair.
/// This preserves the original even-coordinate and small-city bias. Empty maps
/// therefore produce `(0, 0)`; this is deliberately separate from PE's
/// land-value centroid and its map-center empty fallback.
pub(super) fn mac_developed_tile_anchor(city: &City) -> (usize, usize) {
    let mut sum_x = 0_u32;
    let mut sum_y = 0_u32;
    let mut divisor = 1_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city.building_at(x, y) >= 0x70 {
                sum_x += x as u32;
                sum_y += y as u32;
                divisor += 1;
            }
        }
    }
    let doubled_divisor = divisor * 2;
    (
        ((sum_x / doubled_divisor) * 2) as usize,
        ((sum_y / doubled_divisor) * 2) as usize,
    )
}

/// Bytes at `SIMCITY.EXE!0x4e9648` (indexed by XBLD id).
const LAND_VALUE_BUILDING_WEIGHT_TABLE: [u8; 256] = {
    let mut table = [0_u8; 256];
    table[0x0e] = 12;
    table[0x0f] = 12;
    table[0x10] = 12;
    table[0x11] = 12;
    table[0x12] = 18;
    table[0x13] = 18;
    table[0x14] = 18;
    table[0x15] = 18;
    table[0x2c] = 24;
    table[0x2d] = 24;
    table[0x2e] = 24;
    table[0x2f] = 24;
    table[0x30] = 24;
    table[0x31] = 24;
    table[0x39] = 10;
    table[0x3a] = 25;
    table[0x3b] = 2;
    table[0x3e] = 2;
    table[0x3f] = 50;
    table[0x47] = 4;
    table[0x48] = 10;
    table[0x4c] = 2;
    table[0x4d] = 10;
    table[0x4e] = 10;
    table[0x4f] = 10;
    table[0x50] = 5;
    table[0x53] = 5;
    table[0x54] = 5;
    table[0x55] = 5;
    table[0x56] = 10;
    table[0x57] = 10;
    table[0x59] = 5;
    table[0x5c] = 3;
    table[0x5d] = 4;
    table[0x5e] = 2;
    table[0x5f] = 2;
    table[0x60] = 2;
    table[0x61] = 2;
    table[0x62] = 10;
    table[0x64] = 10;
    table[0x66] = 5;
    table[0x6b] = 25;
    table[0x6c] = 10;
    table[0x6d] = 12;
    table[0x6e] = 15;
    table[0x70] = 1;
    table[0x71] = 1;
    table[0x72] = 1;
    table[0x73] = 1;
    table[0x74] = 1;
    table[0x75] = 1;
    table[0x76] = 1;
    table[0x77] = 1;
    table[0x78] = 1;
    table[0x79] = 1;
    table[0x7a] = 1;
    table[0x7b] = 1;
    table[0x7c] = 1;
    table[0x7d] = 1;
    table[0x7e] = 1;
    table[0x7f] = 1;
    table[0x80] = 1;
    table[0x81] = 1;
    table[0x82] = 1;
    table[0x83] = 1;
    table[0x84] = 1;
    table[0x85] = 1;
    table[0x86] = 1;
    table[0x87] = 1;
    table[0x88] = 1;
    table[0x89] = 1;
    table[0x8c] = 2;
    table[0x8d] = 2;
    table[0x8e] = 2;
    table[0x8f] = 2;
    table[0x90] = 3;
    table[0x91] = 3;
    table[0x92] = 3;
    table[0x93] = 3;
    table[0x94] = 2;
    table[0x95] = 2;
    table[0x96] = 2;
    table[0x97] = 2;
    table[0x98] = 2;
    table[0x99] = 3;
    table[0x9a] = 3;
    table[0x9b] = 3;
    table[0x9c] = 3;
    table[0x9d] = 3;
    table[0x9e] = 2;
    table[0x9f] = 2;
    table[0xa0] = 2;
    table[0xa1] = 2;
    table[0xa2] = 3;
    table[0xa3] = 3;
    table[0xa4] = 3;
    table[0xa5] = 3;
    table[0xa6] = 2;
    table[0xa7] = 2;
    table[0xa8] = 3;
    table[0xa9] = 3;
    table[0xae] = 4;
    table[0xaf] = 4;
    table[0xb0] = 4;
    table[0xb1] = 4;
    table[0xb2] = 4;
    table[0xb3] = 4;
    table[0xb4] = 4;
    table[0xb5] = 4;
    table[0xb6] = 4;
    table[0xb7] = 4;
    table[0xb8] = 4;
    table[0xb9] = 4;
    table[0xba] = 4;
    table[0xbb] = 4;
    table[0xbc] = 4;
    table[0xbd] = 4;
    table[0xbe] = 4;
    table[0xbf] = 4;
    table[0xc0] = 4;
    table[0xc1] = 4;
    table[0xc2] = 4;
    table[0xc3] = 4;
    table
};

fn land_value_neighbor_average_terrain_grid(
    terrain: &[i16],
    gx: usize,
    gy: usize,
    grid: usize,
) -> i32 {
    let mut total = i32::from(terrain[gx * grid + gy]);
    let mut count = 1_i32;
    if gx > 0 {
        total += i32::from(terrain[(gx - 1) * grid + gy]);
        count += 1;
    }
    if gx + 1 < grid {
        total += i32::from(terrain[(gx + 1) * grid + gy]);
        count += 1;
    }
    if gy > 0 {
        total += i32::from(terrain[gx * grid + (gy - 1)]);
        count += 1;
    }
    if gy + 1 < grid {
        total += i32::from(terrain[gx * grid + (gy + 1)]);
        count += 1;
    }
    total / count
}

/// PE industrial XVAL arm uses the industrial pointer for the current row,
/// but the preceding/following pointers come from the primary row table
/// (`0x46aff1..0x46b04f`). Mac uses its industrial table for all neighbors.
pub(super) fn pe_industrial_land_value_neighbor_average(
    primary: &[i16],
    industrial: &[i16],
    gx: usize,
    gy: usize,
    grid: usize,
) -> i32 {
    let mut total = i32::from(industrial[gx * grid + gy]);
    let mut count = 1_i32;
    if gx > 0 {
        total += i32::from(primary[(gx - 1) * grid + gy]);
        count += 1;
    }
    if gx + 1 < grid {
        total += i32::from(primary[(gx + 1) * grid + gy]);
        count += 1;
    }
    if gy > 0 {
        total += i32::from(industrial[gx * grid + gy - 1]);
        count += 1;
    }
    if gy + 1 < grid {
        total += i32::from(industrial[gx * grid + gy + 1]);
        count += 1;
    }
    total / count
}

pub(super) fn land_value_center_bonus(zone: u8, x: usize, y: usize, center: (usize, usize)) -> i32 {
    // PE compares its 64x64 output coordinate with the half-scale centroid
    // retained in locals (`0x46af68..0x46af8d`). Inputs x/y are world coordinates.
    let dx = (x / 2).abs_diff(center.0 / 2) as i32;
    let dy = (y / 2).abs_diff(center.1 / 2) as i32;
    let raw = 0x40 - dx - dy;
    if raw <= 0 {
        return 0;
    }
    match zone {
        // PE residential `0x46aeb9`: toward-zero `/2` (`cdq; sub; sar`).
        ZONE_RESIDENTIAL_LIGHT | ZONE_RESIDENTIAL_DENSE => raw / 2,
        // PE commercial `0x46af90`: full strength.
        ZONE_COMMERCIAL_LIGHT | ZONE_COMMERCIAL_DENSE => raw,
        // PE industrial `0x46b093`: toward-zero `/4` (`cdq; and 3; add; sar 2`).
        ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE => raw / 4,
        _ => raw / 2,
    }
}

pub(super) fn mac_land_value_center_bonus(
    zone: u8,
    x: usize,
    y: usize,
    center: (usize, usize),
) -> i32 {
    // The Mac XVAL pass compares native 64×64 output coordinates with its
    // undoubled local center (`0x28070..0x280a6`). XZON 1–2 take the general
    // arm's /2 center term, 3–4 take it at full strength, and 5–6 divide by
    // four. Unrecognized/zero categories use the general /2 arm.
    let dx = x.abs_diff(center.0) as i32;
    let dy = y.abs_diff(center.1) as i32;
    let raw = 0x40 - dx - dy;
    if raw <= 0 {
        return 0;
    }
    match zone {
        ZONE_RESIDENTIAL_LIGHT | ZONE_RESIDENTIAL_DENSE => raw / 2,
        ZONE_COMMERCIAL_LIGHT | ZONE_COMMERCIAL_DENSE => raw,
        ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE => raw / 4,
        _ => raw / 2,
    }
}

/// PE abandoned class-2 tail @ `0x46b0fc`: `value -= toward_zero(value / 2)`.
///
/// For non-negative values this equals `ceil(value / 2)`, not Rust/`C` floor-halve.
pub(super) fn land_value_apply_abandoned_halve(value: i32) -> i32 {
    value - value / 2
}

/// Win95 `SimulationPollutionTerrainAndLandValueScan` pollution term (`0x46a840`).
/// Operates on the raw XPLT microsim byte, not a percent conversion.
pub(super) fn land_value_pollution_penalty(zone: u8, pollution_byte: u8) -> i32 {
    let p = i32::from(pollution_byte);
    match zone {
        ZONE_RESIDENTIAL_LIGHT | ZONE_RESIDENTIAL_DENSE => p / 5,
        ZONE_COMMERCIAL_LIGHT | ZONE_COMMERCIAL_DENSE => p >> 2,
        ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE => p >> 4,
        _ => p / 5,
    }
}

/// Win95 land-value crime term on the raw XCRM microsim byte.
pub(super) fn land_value_crime_penalty(zone: u8, crime_byte: u8) -> i32 {
    let c = i32::from(crime_byte);
    match zone {
        ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE => c >> 2,
        _ => c / 3,
    }
}

/// Win95 XPOP contribution to land value by zone class (`0x46ae7d` / `0x46afcb`):
/// default/residential sparse `XPOP < 0x40` → `+0x15`; commercial → `+XPOP/3`; industrial → none.
pub(super) fn land_value_population_adjustment(zone: u8, population_byte: u8) -> i32 {
    match zone {
        ZONE_COMMERCIAL_LIGHT | ZONE_COMMERCIAL_DENSE => i32::from(population_byte) / 3,
        ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE => 0,
        // PE falls through to the residential arm for all other zone nibbles.
        _ if population_byte < 0x40 => 0x15,
        _ => 0,
    }
}

/// Mac XVAL pollution divisor depends on its selected zone arm.
pub(super) fn mac_land_value_pollution_penalty(zone: u8, pollution_byte: u8) -> i32 {
    let p = i32::from(pollution_byte);
    match zone {
        ZONE_COMMERCIAL_LIGHT | ZONE_COMMERCIAL_DENSE => p / 4,
        ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE => p / 16,
        _ => p / 5,
    }
}

/// Mac XVAL uses XCRM/3 for non-industrial classes and XCRM/4 for industrial.
pub(super) fn mac_land_value_crime_penalty(zone: u8, crime_byte: u8) -> i32 {
    let c = i32::from(crime_byte);
    match zone {
        ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE => c / 4,
        _ => c / 3,
    }
}

/// Mac adds XPOP/3 only in the commercial arm. The general arm adds
/// 0x15 below 0x40; industrial omits population.
pub(super) fn mac_land_value_population_adjustment(zone: u8, population_byte: u8) -> i32 {
    match zone {
        ZONE_COMMERCIAL_LIGHT | ZONE_COMMERCIAL_DENSE => i32::from(population_byte) / 3,
        ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE => 0,
        _ if population_byte < 0x40 => 0x15,
        _ => 0,
    }
}

/// PE `FUN_0046A840` / Mac `FUN_00027AB4` @ `0x286bc` crime source cell before blur.
///
/// Mac reads XPOP (`ffffec40`), XPLC (`ffffeb40`), and XVAL (`ffffe940`); PE uses the
/// same shifts. Legalized gambling adds `0x10` (Mac `ffffeff5` bit 2 / PE ordinance bit 4).
pub(super) fn pe_crime_source_cell(
    population_byte: u8,
    land_value_byte: u8,
    police_byte: u8,
    legalized_gambling: bool,
) -> i32 {
    let mut crime =
        i32::from(population_byte) - i32::from(land_value_byte >> 2) - i32::from(police_byte >> 1);
    if legalized_gambling {
        crime += 0x10;
    }
    crime
}

/// Win95 crime refresh: `XPOP - (XVAL>>2) - (XPLC>>1)` (+ gambling), 64×64 blur, clamp.
///
/// Mac `FUN_00027AB4` forms the same source cell at `0x286bc` from `ffffec40` /
/// `ffffeb40` / `ffffe940` before the cardinal blur into XCRM (`ffffea40`).
pub(super) fn live_crime_density_map_for_city(
    city: &City,
    population_density: &[u8],
    police: &[u8],
    land_value: &[u8],
) -> Vec<u8> {
    live_crime_density_map_for_city_with_fidelity(
        city,
        population_density,
        police,
        land_value,
        EngineFidelity::Win95,
    )
}

pub(super) fn live_crime_density_map_for_city_with_fidelity(
    city: &City,
    population_density: &[u8],
    police: &[u8],
    land_value: &[u8],
    fidelity: EngineFidelity,
) -> Vec<u8> {
    const MICRO: usize = 64;
    let gambling =
        ordinance_enabled_for_city_with_fidelity(city, ORDINANCE_LEGALIZED_GAMBLING, fidelity);
    let mut words = vec![0_i32; MICRO * MICRO];
    for my in 0..MICRO {
        for mx in 0..MICRO {
            let tx = mx * 2;
            let ty = my * 2;
            let mut pop = 0_u8;
            let mut land_byte = 0_u8;
            let mut police_byte = 0_u8;
            for dy in 0..2 {
                for dx in 0..2 {
                    let x = tx + dx;
                    let y = ty + dy;
                    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
                        continue;
                    }
                    let index = density_map_index(x, y);
                    pop = pop.max(population_density.get(index).copied().unwrap_or(0));
                    land_byte = land_byte.max(land_value.get(index).copied().unwrap_or(0));
                    police_byte = police_byte.max(police.get(index).copied().unwrap_or(0));
                }
            }
            // The marker is consumed after the edition-specific clear and
            // projection stages. PE may retain a bit that Mac clears.
            let eligible = if fidelity.uses_win95() {
                pe_xval_output_is_eligible(city, tx, ty)
            } else {
                mac_xval_output_is_eligible(city, tx, ty)
            };
            if !eligible {
                continue;
            }
            words[my * MICRO + mx] = pe_crime_source_cell(pop, land_byte, police_byte, gambling);
        }
    }

    // Win95 crime blur: center weight 1, divisor starts at 1 (unlike pollution's *2 / base-4).
    let mut micro = vec![0_u8; MICRO * MICRO];
    for my in 0..MICRO {
        for mx in 0..MICRO {
            let index = my * MICRO + mx;
            let mut total = words[index];
            let mut divisor = 1_i32;
            if my > 0 {
                total += words[index - MICRO];
                divisor = 2;
            }
            if my + 1 < MICRO {
                total += words[index + MICRO];
                divisor += 1;
            }
            if mx > 0 {
                total += words[index - 1];
                divisor += 1;
            }
            if mx + 1 < MICRO {
                total += words[index + 1];
                divisor += 1;
            }
            // PE applies the legalized-gambling term while forming the source cell;
            // it does not apply anti-drug or neighborhood-watch scales after blur.
            micro[index] = (total / divisor).clamp(0, 255) as u8;
        }
    }

    let mut values = vec![0_u8; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            values[density_map_index(x, y)] = micro[(y / 2) * MICRO + (x / 2)];
        }
    }
    values
}

/// Rebuild the density phase's shared crime-eligibility marker in XBIT.
///
/// Win95 `FUN_0046A840` clears bit `0x08` on full-resolution cells with
/// XBLD `> 0x6f` (`0x46AB06`), then ORs it into the half-resolution cell for
/// each full tile with XBLD `> 0x1c` or a nonzero XZON low nibble
/// (`0x46AC7E`). Mac CODE 14 `FUN_00027AB4` instead clears bit 3 across the
/// full plane before making the same half-resolution projection
/// (`0x27D52..0x27D5A`, `0x27E68..0x27E92`). Crime and XVAL derive the
/// post-clear/projection marker before this side effect is mirrored to XBIT.
pub(super) fn refresh_crime_eligibility_xbit_for_density_phase(
    city: &mut City,
    fidelity: EngineFidelity,
) {
    if fidelity.uses_win95() {
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                if city.building_at(x, y) > 0x6f {
                    let bits = city.tile_bits_at(x, y).unwrap_or_default();
                    if bits.raw_bits() & 0x08 != 0 {
                        city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & !0x08));
                    }
                }
            }
        }
    } else {
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let bits = city.tile_bits_at(x, y).unwrap_or_default();
                if bits.raw_bits() & 0x08 != 0 {
                    city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & !0x08));
                }
            }
        }
    }

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if !pe_crime_source_is_eligible(city, x, y) {
                continue;
            }
            let (mask_x, mask_y) = (x / 2, y / 2);
            let bits = city.tile_bits_at(mask_x, mask_y).unwrap_or_default();
            if bits.raw_bits() & 0x08 == 0 {
                city.set_tile_bits(mask_x, mask_y, TileBits::new(bits.raw_bits() | 0x08));
            }
        }
    }
}

/// Predicate used by Win95 `FUN_00434B70` when setting XBIT bit 0x08.
fn pe_crime_source_is_eligible(city: &City, x: usize, y: usize) -> bool {
    city.building_at(x, y) >= 0x1d
        || city
            .zone_at(x, y)
            .is_some_and(|zone| zone.raw_bits() & 0x0f != 0)
}

pub(super) fn strongest_coverage_percent(sources: &[CoverageSource], x: usize, y: usize) -> u32 {
    sources
        .iter()
        .map(|source| source.coverage_percent(x, y))
        .max()
        .unwrap_or(0)
}

pub(super) fn strongest_funded_coverage_percent(
    sources: &[CoverageSource],
    x: usize,
    y: usize,
    funding_percent: u32,
) -> u32 {
    strongest_coverage_percent(sources, x, y).saturating_mul(funding_percent.min(100)) / 100
}

pub(super) fn add_density_pressure(
    pressure: &mut [u32],
    source_x: usize,
    source_y: usize,
    radius: u32,
    amount: u32,
) {
    let radius_usize = radius as usize;
    let min_x = source_x.saturating_sub(radius_usize);
    let max_x = source_x
        .saturating_add(radius_usize)
        .min(CITY_WIDTH.saturating_sub(1));
    let min_y = source_y.saturating_sub(radius_usize);
    let max_y = source_y
        .saturating_add(radius_usize)
        .min(CITY_HEIGHT.saturating_sub(1));
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let distance = source_x.abs_diff(x).max(source_y.abs_diff(y)) as u32;
            if distance > radius {
                continue;
            }
            let contribution = amount
                .saturating_mul(radius.saturating_add(1).saturating_sub(distance))
                / radius.saturating_add(1);
            let index = density_map_index(x, y);
            pressure[index] = pressure[index].saturating_add(contribution);
        }
    }
}

pub(super) fn density_map_index(x: usize, y: usize) -> usize {
    x * CITY_HEIGHT + y
}

/// Sum the native 64×64 microsim cells represented by an expanded 128×128
/// density map. Each 2×2 uses the max byte so sparse per-tile writes count once.
pub(super) fn density_map_microsim64_sum(values: &[u8]) -> u32 {
    if values.len() != CITY_TILES {
        return values.iter().fold(0_u32, |total, value| {
            total.saturating_add(u32::from(*value))
        });
    }
    let mut sum = 0_u32;
    for y in (0..CITY_HEIGHT).step_by(2) {
        for x in (0..CITY_WIDTH).step_by(2) {
            let mut cell = 0_u8;
            for dy in 0..2 {
                for dx in 0..2 {
                    let tx = x + dx;
                    let ty = y + dy;
                    if tx < CITY_WIDTH && ty < CITY_HEIGHT {
                        cell = cell.max(values[density_map_index(tx, ty)]);
                    }
                }
            }
            sum = sum.saturating_add(u32::from(cell));
        }
    }
    sum
}

pub(super) fn density_percent_to_byte(percent: u32) -> u8 {
    (percent.min(100).saturating_mul(255) / 100) as u8
}

pub(super) fn density_byte_to_percent(value: u8) -> u32 {
    u32::from(value).saturating_mul(100) / 255
}

pub(super) fn land_value_total_from_density(values: &[u8]) -> u32 {
    values.iter().fold(0_u32, |total, value| {
        total.saturating_add(u32::from(*value))
    })
}

pub(super) fn city_value_for_land_value(land_value: u32) -> u32 {
    land_value / 2
}

pub(super) fn land_value_quality_from_density(values: &[u8]) -> Option<u32> {
    if values.is_empty() || values.iter().all(|value| *value == 0) {
        return None;
    }
    let max_total = (values.len() as u32).saturating_mul(255);
    if max_total == 0 {
        return None;
    }
    Some(
        land_value_total_from_density(values)
            .saturating_mul(100)
            .checked_div(max_total)
            .unwrap_or(0)
            .min(100),
    )
}

pub(super) fn estimated_land_value_total_from_quality(quality: u32) -> u32 {
    u32::from(density_percent_to_byte(quality)).saturating_mul(CITY_TILES as u32)
}

pub(super) fn land_value_total_for_city(
    city: &City,
    quality: u32,
    use_saved_fallback: bool,
) -> u32 {
    city.land_value_density_sum()
        .filter(|sum| *sum > 0)
        .or_else(|| {
            let value = city.pe_dw_city_land_value;
            (value > 0 && (use_saved_fallback || value > 100)).then_some(value)
        })
        .unwrap_or_else(|| estimated_land_value_total_from_quality(quality))
}

pub(super) fn land_value_quality_for_city(
    city: &City,
    computed: u32,
    use_saved_fallback: bool,
) -> u32 {
    // XVAL is a sparse tile map (undeveloped = 0). Its citywide average is not the
    // 0–100 quality used by rewards / service outcomes. Prefer a legacy MISC percent
    // when present; otherwise keep the service-formula score.
    if use_saved_fallback {
        let saved = city.pe_dw_city_land_value;
        if (1..=100).contains(&saved) {
            return saved;
        }
    }
    computed.min(100)
}
