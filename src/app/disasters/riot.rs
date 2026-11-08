use super::*;

pub(in crate::app) fn pe_riot_tick_active(city: &City) -> bool {
    matches!(city.pe_w_current_disaster_type(), 0x03 | 0x0d)
}

/// PE overlay scan `0x45fb80` / `0x45fd8c` matches XTXT `0xfe`/`0xfd` with no type gate.
pub(in crate::app) fn pe_riot_overlay_scan_due(city: &City) -> bool {
    pe_riot_tick_active(city) || pe_overlay_marker_present(city, &[0xfd, 0xfe])
}

pub(in crate::app) fn pe_riot_tick_seed(city: &City) -> u32 {
    // Offline/deterministic helper seed only. Live Win95 riot arm @ `0x45fb80`
    // draws CRT `rand()` (`0x48cdd8`) from the process stream each overlay tick.
    let mut seed = 0x45FB_8000u32;
    for byte in city.name.bytes() {
        seed = seed.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    seed
}

/// Win95 `0x45f760` neighbor windows plus roads `1..=4`.
pub(in crate::app) fn pe_riot_tick_neighbor_eligible(building: u8) -> bool {
    matches!(building, 1..=4) || pe_rioters_building_eligible(building)
}

pub(in crate::app) fn pe_riot_paint_neighbor(
    city: &mut City,
    x: usize,
    y: usize,
    marker: u8,
) -> bool {
    let xtxt = city.text_overlay_at(x, y).unwrap_or(0);
    if xtxt >= 0x33 {
        return false;
    }
    city.set_text_overlay(x, y, marker);
    true
}

/// Win95 `0x45fb80` / `0x45fd8c`: 1-in-4 act, water/CRT recede, NESW ignite,
/// flip `0xfe`↔`0xfd`, paint look-neighbor when XTXT `< 0x33`.
pub(in crate::app) fn pe_riot_tick_city(city: &mut City) -> PeRiotTickStats {
    let seed = pe_riot_tick_seed(city);
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    pe_riot_tick_city_with_rng(city, &mut crt, &mut lfsr)
}

pub(in crate::app) fn pe_riot_tick_city_with_rng(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> PeRiotTickStats {
    let mut stats = PeRiotTickStats::default();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            pe_riot_tick_cell_with_rng(city, crt, lfsr, x, y, &mut stats);
        }
    }
    stats
}

pub(in crate::app) fn pe_riot_tick_cell_with_rng(
    city: &mut City,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
    x: usize,
    y: usize,
    stats: &mut PeRiotTickStats,
) {
    let Some(marker) = city.text_overlay_at(x, y) else {
        return;
    };
    if marker != 0xfe && marker != 0xfd {
        return;
    }
    if crt.random_u16() & 3 != 0 {
        return;
    }
    if crt.random_u16() & 0xff == 0 {
        city.set_text_overlay(x, y, 0);
        stats.receded += 1;
        return;
    }
    if city.tile_bits_at(x, y).is_some_and(TileBits::water_tile) {
        city.set_text_overlay(x, y, 0);
        stats.receded += 1;
        return;
    }
    pe_clear_xtrf_microsim64(city, x, y);
    let ignite_roll = crt.random_u16() & 0x7f;
    if ignite_roll <= 3 {
        let dir = usize::from(ignite_roll);
        if let Some((nx, ny)) =
            offset_city_tile(x, y, PE_FLOOD_SPREAD_DX[dir], PE_FLOOD_SPREAD_DY[dir])
        {
            // PE @ `0x45fe3e`: `push 0; call 0x401bf4` — `test eax` counts paint success.
            if pe_fun_00460e10_ignite(city, Some(crt), lfsr, nx, ny, 0) {
                stats.ignited += 1;
            }
        }
    }
    let (look_a, look_b, flip_to, paint_marker) = if marker == 0xfe {
        ((-1isize, 0), (0, -1isize), 0xfdu8, 0xfeu8)
    } else {
        ((1isize, 0), (0, 1isize), 0xfeu8, 0xfdu8)
    };
    let mut mask = 0u16;
    if let Some((nx, ny)) = offset_city_tile(x, y, look_a.0, look_a.1) {
        if pe_riot_tick_neighbor_eligible(city.building_at(nx, ny)) {
            mask |= 1;
        }
    }
    if let Some((nx, ny)) = offset_city_tile(x, y, look_b.0, look_b.1) {
        if pe_riot_tick_neighbor_eligible(city.building_at(nx, ny)) {
            mask |= 2;
        }
    }
    if mask == 0 {
        city.set_text_overlay(x, y, flip_to);
        stats.flipped += 1;
        return;
    }
    if crt.random_u16() & 7 != 0 {
        city.set_text_overlay(x, y, 0);
        stats.receded += 1;
    } else {
        city.set_text_overlay(x, y, flip_to);
        stats.flipped += 1;
    }
    if mask == 3 {
        mask = (crt.random_u16() & 1) + 1;
    }
    let (dx, dy) = if mask == 1 { look_a } else { look_b };
    if let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) {
        if pe_riot_paint_neighbor(city, nx, ny, paint_marker) {
            stats.spread += 1;
        }
    }
}
