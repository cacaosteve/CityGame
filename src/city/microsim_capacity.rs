// PE population load, footprint census, and capacity-width helpers.

use super::*;

/// Win95 `FUN_0044f7d0` (thunk `0x401b45`):
/// `min((city_pop + arco_residents + arco_tile_bonus) / divisor, funding_cap)`.
///
/// `arco_tile_bonus` is the PE term from footprint counts at tiles 251..=254
/// (`sum/16`); when that count is `> 0x8C`, bonus = `count*20000 - 0x2AB980`.
pub(crate) fn pe_arco_tile_pop_bonus(arco_count: u32) -> u32 {
    if arco_count <= 0x8c {
        0
    } else {
        arco_count
            .saturating_mul(20_000)
            .saturating_sub(0x002A_B980)
    }
}

/// Occupancy prologue `dwTileCount` cell census (`WORD[256]` at `0x4CA4C8`).
pub(super) fn pe_occupancy_tile_words(city: &City, buildings: &[u8]) -> i32 {
    count_building_tiles(city, buildings).min(i32::MAX as u32) as i32
}

/// Win95 `cwd; idiv 9` on a tile WORD (3×3 school/hospital footprints).
pub(super) fn pe_occupancy_div9_count(city: &City, building: u8) -> i32 {
    pe_occupancy_tile_words(city, &[building]).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) / 9
}

/// Win95 `cdq; and 0xf; add; sar 4` on a tile WORD (4×4 college/prison/arco).
pub(super) fn pe_occupancy_div16_count(city: &City, building: u8) -> i32 {
    pe_sar_div(pe_occupancy_tile_words(city, &[building]), 4)
}

/// Arco cells 251..=254 `/16`, matching `FUN_0044f7d0` and the occupancy prologue.
pub(crate) fn pe_occupancy_arco_footprints(city: &City) -> u32 {
    pe_sar_div(pe_occupancy_tile_words(city, &[251, 252, 253, 254]), 4).max(0) as u32
}

/// PE `cmp count, 1; jge` then `mov 1` when the census word is empty/negative.
pub(super) fn pe_occupancy_divisor(count: i32) -> u32 {
    if count >= 1 { count as u32 } else { 1 }
}

/// Win95 `FUN_0044f7d0` (thunk `0x401b45`): divide the population load by the
/// signed-extended divisor word (zero becomes 100), mask the quotient to 16
/// bits, then compare against the signed-extended cap word. If the cap is not
/// greater, return its original low 16 bits. Callers pass the already-summed
/// population load and their PE-width cap value.
pub(super) fn pe_microsim_population_load(
    city_population: u32,
    arcology_population: u32,
    arcology_bonus: u32,
) -> u32 {
    city_population
        .wrapping_add(arcology_population)
        .wrapping_add(arcology_bonus)
}

pub(super) fn pe_microsim_scaled_capacity(pop_load: u32, divisor: u32, funding_cap: u32) -> u32 {
    let divisor_word = divisor as u16;
    let divisor_signed = i32::from(divisor_word as i16);
    // PE sign-extends CX into ECX, then uses unsigned DIV. A negative CX
    // therefore becomes a large unsigned divisor (normally quotient 0 at
    // city-scale population loads).
    let divisor = if divisor_word == 0 {
        100
    } else {
        divisor_signed as u32
    };
    let quot16 = (pop_load / divisor) & 0xffff;
    let cap_word = funding_cap as u16;
    let cap_signed = i32::from(cap_word as i16);
    if cap_signed > quot16 as i32 {
        quot16
    } else {
        u32::from(cap_word)
    }
}

pub(super) fn count_building_tiles(city: &City, buildings: &[u8]) -> u32 {
    // PE occupancy / system microsim counts read BSS `dwTileCount` (`0x4CA4C8`),
    // not a live XBLD walk. PlaceTile non-military updates keep the table coherent.
    let mut total = 0_u32;
    for &building in buildings {
        total = total.saturating_add(u32::from(city.pe_dw_tile_count[building as usize]));
    }
    total
}
