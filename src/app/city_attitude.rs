//! Developed-tile census and the city inputs used by PE attitude scoring.

use super::*;

/// Win95 land-value / developed-tile scan clears `wCityDevelopedTiles` @ `0x46ab58`
/// then increments qualifying tiles @ `0x46ac82` (XBLD `>= 0x1D` or XZON nibble).
const PE_ATTITUDE_DEVELOPED_BUILDING_MIN: u8 = 0x1d;

/// Recount developed tiles into BSS `0x4c93b4` (`pe_w_city_developed_tiles`).
/// The city-open scan `0x434ce4..0x434d34` uses the same predicate.
pub(super) fn pe_rebuild_w_city_developed_tiles_46ab56(city: &mut City) -> u32 {
    let count = pe_attitude_developed_tile_count(city);
    city.pe_w_city_developed_tiles = count;
    count
}

/// Developed-tile / pollution / land-value / crime totals for attitude scores 1..3
/// (`0x4c93b4` / `0x4cc910` / `0x4ca440` / `0x4cb454`).
pub(super) fn pe_attitude_demand_inputs_for_city(
    city: &City,
    population: PopulationStats,
) -> (u32, u32, u32, u32) {
    pe_attitude_demand_inputs_for_city_with_power(city, population, &power_service_maps(city))
}

pub(super) fn pe_attitude_demand_inputs_for_city_with_power(
    city: &City,
    _population: PopulationStats,
    _power_maps: &PowerServiceMaps,
) -> (u32, u32, u32, u32) {
    // PE `0x413fb6..0x414001` consumes the published BSS totals, including
    // zero, rather than rescanning expanded display maps at the poll.
    (
        pe_attitude_developed_tile_count(city),
        city.pe_dw_city_pollution,
        city.pe_dw_city_land_value,
        city.pe_dw_city_crime,
    )
}

pub(super) fn pe_attitude_tile_is_developed(city: &City, x: usize, y: usize) -> bool {
    city.building_at(x, y) >= PE_ATTITUDE_DEVELOPED_BUILDING_MIN
        || city.zone_at(x, y).is_some_and(|zone| zone.zone_id() != 0)
}

pub(super) fn pe_attitude_developed_tile_count(city: &City) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if pe_attitude_tile_is_developed(city, x, y) {
                count = count.saturating_add(1);
            }
        }
    }
    count.min(u32::from(u16::MAX))
}
