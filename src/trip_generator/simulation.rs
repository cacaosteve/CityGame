use super::*;

/// Soft monthly decay matching Win95 `FUN_004586F1` @ `0x4586f1`
/// (`SimulationUpdateMonthlyTrafficData`): walk native 64×64 XTRF (`0x4ca940`),
/// write `value - (value >> 2)`, accumulate into `dwCityTrafficCount` (`0x4cc6f4`).
pub fn decay_traffic_density(city: &mut City) {
    if !city.has_traffic_density_map() {
        return;
    }
    // XTRF is a 64x64 microsim grid expanded across each 2x2 block of the
    // 128x128 city map. `set_traffic_density` writes that whole block, so
    // visit one representative tile per cell or the same cell would decay
    // four times in one monthly pass.
    for y in (0..CITY_HEIGHT).step_by(2) {
        for x in (0..CITY_WIDTH).step_by(2) {
            let value = city.traffic_density_at(x, y).unwrap_or(0);
            city.set_traffic_density(x, y, (u16::from(value) - (u16::from(value) >> 2)) as u8);
        }
    }
}

/// Sum of Win95 64×64 XTRF microsim cells after the PE monthly decay pass.
/// Each expanded 2×2 uses the max byte so the expanded city view still represents one native cell.
pub fn traffic_density_microsim_sum(city: &City) -> u32 {
    if !city.has_traffic_density_map() {
        return 0;
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
                        cell = cell.max(city.traffic_density_at(tx, ty).unwrap_or(0));
                    }
                }
            }
            sum = sum.saturating_add(u32::from(cell));
        }
    }
    sum
}

/// Add `delta` into the microsim cell covering `(x, y)` (all four 128-map tiles).
pub fn bump_xtrf_microsim_cell(city: &mut City, x: usize, y: usize, delta: u8) {
    if delta == 0 {
        return;
    }
    city.add_traffic_density_saturating(x, y, delta);
}

pub fn bump_xtrf_along_path(city: &mut City, path: &[(usize, usize)], pop_level: u8) -> usize {
    if pop_level == 0 || path.is_empty() {
        return 0;
    }
    // Win95 `L_RunTripGenerator` adds `nBuildingPopLevel` for every road/highway/
    // bridge stack point, including multiple tiles that share one XTRF microsim cell.
    let mut bumped = 0usize;
    for &(x, y) in path {
        bump_xtrf_microsim_cell(city, x, y, pop_level);
        bumped += 1;
    }
    bumped
}

fn rci_pop_level_for_building(building: u8) -> Option<u8> {
    Some(match building {
        0x70..=0x8B => 1,
        0x8C..=0x8F | 0x94..=0x98 | 0x9E..=0xA1 | 0xA6..=0xA7 | 0xAA..=0xAB => 2,
        0x90..=0x93 | 0x99..=0x9D | 0xA2..=0xA5 | 0xA8..=0xA9 | 0xAC..=0xAD => 3,
        0xAE..=0xC5 => 4,
        _ => return None,
    })
}

fn is_rci_zone(zone_id: u8) -> bool {
    matches!(zone_id, 1..=6)
}

fn xzon_corner_check(city: &City, x: usize, y: usize) -> bool {
    let angle = PE_POSITION_ANGLE[usize::from(city.rotation() & 3)];
    city.zone_at(x, y)
        .is_some_and(|zone| zone.raw_bits() & ZONE_CORNER_ALL & angle != 0)
}

fn run_origin_trip(
    city: &mut City,
    trip_served: &mut [bool],
    stats: &mut MonthlyTripStats,
    x: usize,
    y: usize,
    tile_powered: &impl Fn(usize, usize) -> bool,
    write_xtrf: bool,
    pe_low_surface_vacant_origin: bool,
    crt_rng: Option<&mut Win95CrtRand>,
) {
    let served_index = y * CITY_WIDTH + x;
    if trip_served[served_index] {
        return;
    }
    let Some(zone) = city.zone_at(x, y) else {
        return;
    };
    let zone_id = zone.zone_id();
    if !is_rci_zone(zone_id) {
        return;
    }
    let building = city.building_at(x, y);
    // PE `FUN_004358B0 @ 0x436032..0x436054` accepts every low surface
    // XBLD below 0x1d as a pop-0 vacancy after `Game_IsValidTransitItems`;
    // unlike developed RCI footprints, that branch does not require a corner.
    let low_surface_vacancy = pe_low_surface_vacant_origin && building < 0x1d;
    if building != 0 && !low_surface_vacancy && !xzon_corner_check(city, x, y) {
        return;
    }
    let pop = match rci_pop_level_for_building(building) {
        Some(pop) => pop,
        None if building == 0 || low_surface_vacancy => 0, // vacant growth-gate origin
        _ => return,
    };
    // PE `FUN_00436D00` @ `0x436d00` gates RunTripGenerator for vacant and populated alike.
    if !tile_powered(x, y) {
        return;
    }
    stats.trips_attempted += 1;
    let result = if let Some(crt_rng) = crt_rng {
        run_trip_with_rng(city, x, y, zone_id, pop, crt_rng)
    } else {
        run_trip(city, x, y, zone_id, pop)
    };
    if !result.success {
        return;
    }
    stats.trips_succeeded += 1;
    trip_served[served_index] = true;
    if pop == 0 {
        return;
    }
    // Win95 only congests road / highway / road-bridge steps (`traffic_path`).
    // Never fall back to the full walk — rail/subway/bus/pedestrian must not stamp XTRF.
    // This shared PE-style trip helper does not implement the separate native
    // Mac CODE 8/14 route-success XTRF writer in app/budget.rs.
    if write_xtrf {
        stats.tiles_bumped += bump_xtrf_along_path(city, &result.traffic_path, pop);
    }
    // Win95 adds `nBuildingPopLevel` once per used transit mode.
    let passengers = u32::from(pop);
    if result.used_bus {
        stats.bus_passengers = stats.bus_passengers.saturating_add(passengers);
    }
    if result.used_rail {
        stats.rail_passengers = stats.rail_passengers.saturating_add(passengers);
    }
    if result.used_subway {
        stats.subway_passengers = stats.subway_passengers.saturating_add(passengers);
    }
}

/// Win95 `SimulationGrowthTick` trip pass for one iStep/iSubStep lattice.
/// Does not decay XTRF or clear `trip_served`; callers accumulate across days 3–18.
/// `write_xtrf` selects this helper's PE-style XTRF writes. The native Mac
/// monthly route scan has a separate XTRF writer in `app/budget.rs`.
pub fn simulate_growth_lattice_trips(
    city: &mut City,
    trip_served: &mut [bool],
    tiles: impl IntoIterator<Item = (usize, usize)>,
    tile_powered: impl Fn(usize, usize) -> bool,
    write_xtrf: bool,
) -> MonthlyTripStats {
    simulate_growth_lattice_trips_inner(
        city,
        trip_served,
        tiles,
        tile_powered,
        write_xtrf,
        false,
        None,
    )
}

/// Win95 growth-lattice trip pass using the process-wide CRT stream.
pub fn simulate_growth_lattice_trips_with_rng(
    city: &mut City,
    trip_served: &mut [bool],
    tiles: impl IntoIterator<Item = (usize, usize)>,
    tile_powered: impl Fn(usize, usize) -> bool,
    write_xtrf: bool,
    crt_rng: &mut Win95CrtRand,
) -> MonthlyTripStats {
    simulate_growth_lattice_trips_inner(
        city,
        trip_served,
        tiles,
        tile_powered,
        write_xtrf,
        true,
        Some(crt_rng),
    )
}

fn simulate_growth_lattice_trips_inner(
    city: &mut City,
    trip_served: &mut [bool],
    tiles: impl IntoIterator<Item = (usize, usize)>,
    tile_powered: impl Fn(usize, usize) -> bool,
    write_xtrf: bool,
    pe_low_surface_vacant_origin: bool,
    mut crt_rng: Option<&mut Win95CrtRand>,
) -> MonthlyTripStats {
    assert_eq!(trip_served.len(), CITY_TILES);
    let mut stats = MonthlyTripStats::default();
    for (x, y) in tiles {
        run_origin_trip(
            city,
            trip_served,
            &mut stats,
            x,
            y,
            &tile_powered,
            write_xtrf,
            pe_low_surface_vacant_origin,
            crt_rng.as_deref_mut(),
        );
    }
    stats
}

/// Standalone monthly trip helper: sample RCI origins on a stride-4 grid, optionally
/// write congestion, then optionally decay XTRF. Win95 day 19 is
/// `SimulationUpdateMonthlyTrafficData` (decay + traffic sum) after growth-tick
/// trips on days 3–18 — not a re-trip pass. Production Mac CODE 8/14 uses its
/// separate route-scan worker for Mac XTRF writes; it does not use this PE helper.
pub fn simulate_monthly_trips(
    city: &mut City,
    trip_served: &mut [bool],
    tile_powered: impl Fn(usize, usize) -> bool,
) -> MonthlyTripStats {
    simulate_monthly_trips_with_xtrf(city, trip_served, tile_powered, true)
}

pub fn simulate_monthly_trips_with_xtrf(
    city: &mut City,
    trip_served: &mut [bool],
    tile_powered: impl Fn(usize, usize) -> bool,
    write_xtrf: bool,
) -> MonthlyTripStats {
    assert_eq!(trip_served.len(), CITY_TILES);
    trip_served.fill(false);

    let mut stats = MonthlyTripStats::default();
    // Stride-4 matches Win95 growth-tick sampling; cover all 16 phase pairs over months
    // by using a fixed phase here (0,0) for a stable monthly slice.
    for y in (0..CITY_HEIGHT).step_by(4) {
        for x in (0..CITY_WIDTH).step_by(4) {
            run_origin_trip(
                city,
                trip_served,
                &mut stats,
                x,
                y,
                &tile_powered,
                write_xtrf,
                false,
                None,
            );
        }
    }
    if write_xtrf {
        decay_traffic_density(city);
    }
    stats
}

/// Win95 `Game_IsValidTransitItems` (`0x457530`).
///
/// `GetPopulatedTileAndLevel` uses this 24-offset ring so vacant tiles can enter
/// growth when a matching transit tile is within the trip-start coordinates, not
/// only when a cardinal neighbor is a road. Rail tracks are not in the PE list.
pub fn game_is_valid_transit_items(city: &City, x: usize, y: usize) -> bool {
    let x = x as i32;
    let y = y as i32;
    for &(dx, dy) in &TRIP_START_COORDS {
        let nx = x + dx;
        let ny = y + dy;
        if !(0..CITY_WIDTH as i32).contains(&nx) || !(0..CITY_HEIGHT as i32).contains(&ny) {
            continue;
        }
        if is_valid_transit_item_tile(city.building_at(nx as usize, ny as usize)) {
            return true;
        }
    }
    false
}

/// Stock tile-id ranges from `0x457530`: roads, road/power/rail crossovers, a
/// highway-to-road band, and bus/rail/subway stations.
pub(super) fn is_valid_transit_item_tile(building: u8) -> bool {
    matches!(
        building,
        0x1D..=0x2B | 0x3F..=0x46 | 0x4B | 0x4C | 0x5D..=0x60 | 0xE9 | 0xEC | 0xED
    )
}

/// Probe whether a vacant/candidate tile can complete a trip (used by growth gate).
pub fn tile_has_trip_access(city: &City, x: usize, y: usize) -> bool {
    let Some(zone) = city.zone_at(x, y) else {
        return false;
    };
    let zone_id = zone.zone_id();
    if !is_rci_zone(zone_id) {
        return false;
    }
    let building = city.building_at(x, y);
    if building != 0 && !xzon_corner_check(city, x, y) {
        return false;
    }
    let pop = rci_pop_level_for_building(building).unwrap_or(0);
    run_trip(city, x, y, zone_id, pop).success
}

#[cfg(test)]
mod tests;
