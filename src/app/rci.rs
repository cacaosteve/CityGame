//! RCI demand, growth selection, decline, and local quality scoring.

use super::*;

mod demand;
mod growth;
mod pipeline;
mod placement;
pub(super) use demand::*;
pub(super) use growth::*;
pub(super) use placement::*;

pub(super) const ZONE_RESIDENTIAL_LIGHT: u8 = 1;
pub(super) const ZONE_RESIDENTIAL_DENSE: u8 = 2;
pub(super) const ZONE_COMMERCIAL_LIGHT: u8 = 3;
pub(super) const ZONE_COMMERCIAL_DENSE: u8 = 4;
pub(super) const ZONE_INDUSTRIAL_LIGHT: u8 = 5;
pub(super) const ZONE_INDUSTRIAL_DENSE: u8 = 6;

#[cfg(test)]
pub(super) const RESIDENTIAL_RECREATION_BASE_POPULATION_CAP: u32 = 10_000;
#[cfg(test)]
const RESIDENTIAL_RECREATION_CAP_PER_TILE: u32 = 1_000;

#[cfg(test)]
pub(super) fn sports_teams_for_city(city: &City) -> u32 {
    // PE `wSportsTeams` @ `0x4cc4e0` is a milestone bitmask (`1/3/7/0xf/0x1f`), not stadium count.
    u32::from(city.pe_w_sports_teams())
}

#[cfg(test)]
pub(super) fn residential_recreation_population_cap_for_city(city: &City) -> u32 {
    RESIDENTIAL_RECREATION_BASE_POPULATION_CAP
        .saturating_add(
            complete_building_count_for_city(city, SMALL_PARK_BUILDING_ID, 1)
                .saturating_mul(RESIDENTIAL_RECREATION_CAP_PER_TILE),
        )
        .saturating_add(
            complete_building_count_for_city(city, BIG_PARK_BUILDING_ID, 3)
                .saturating_mul(3 * RESIDENTIAL_RECREATION_CAP_PER_TILE),
        )
        .saturating_add(
            complete_building_count_for_city(city, MARINA_BUILDING_ID, 3)
                .saturating_mul(9 * RESIDENTIAL_RECREATION_CAP_PER_TILE),
        )
        .saturating_add(
            complete_building_count_for_city(city, STADIUM_BUILDING_ID, STADIUM_BUILDING_SIZE)
                .saturating_mul(16 * RESIDENTIAL_RECREATION_CAP_PER_TILE),
        )
        .saturating_add(
            complete_building_count_for_city(city, ZOO_BUILDING_ID, 4)
                .saturating_mul(16 * RESIDENTIAL_RECREATION_CAP_PER_TILE),
        )
}

/// Win95 `ChooseAndPlaceBuilding` R/C/I entries, grouped by zone density.
pub(super) const RESIDENTIAL_1X1_BUILDINGS: [u8; 12] = [
    0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B,
];
pub(super) const RESIDENTIAL_2X2_BUILDINGS: [u8; 8] =
    [0x8C, 0x8D, 0x8E, 0x8F, 0x90, 0x91, 0x92, 0x93];
pub(super) const RESIDENTIAL_3X3_BUILDINGS: [u8; 4] = [0xAE, 0xAF, 0xB0, 0xB1];
pub(super) const COMMERCIAL_1X1_BUILDINGS: [u8; 8] =
    [0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81, 0x82, 0x83];
pub(super) const COMMERCIAL_2X2_BUILDINGS: [u8; 10] =
    [0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D];
pub(super) const COMMERCIAL_3X3_BUILDINGS: [u8; 10] =
    [0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB];
pub(super) const INDUSTRIAL_1X1_BUILDINGS: [u8; 4] = [0x84, 0x85, 0x86, 0x87];
pub(super) const INDUSTRIAL_2X2_BUILDINGS: [u8; 8] =
    [0x9E, 0x9F, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5];
pub(super) const INDUSTRIAL_3X3_BUILDINGS: [u8; 6] = [0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xC1];

/// Win95 construction stages by population level (`ChooseAndPlace` mode 3).
pub(super) const CONSTRUCTION_POP1_BUILDINGS: [u8; 2] = [0x88, 0x89];
pub(super) const CONSTRUCTION_POP2_BUILDINGS: [u8; 2] = [0xA6, 0xA7];
pub(super) const CONSTRUCTION_POP3_BUILDINGS: [u8; 2] = [0xA8, 0xA9];
pub(super) const CONSTRUCTION_POP4_BUILDINGS: [u8; 2] = [0xC2, 0xC3];
pub(super) const CONSTRUCTION_1X1_BUILDINGS: [u8; 2] = CONSTRUCTION_POP1_BUILDINGS;
pub(super) const CONSTRUCTION_2X2_BUILDINGS: [u8; 4] = [0xA6, 0xA7, 0xA8, 0xA9];
pub(super) const CONSTRUCTION_3X3_BUILDINGS: [u8; 2] = CONSTRUCTION_POP4_BUILDINGS;

/// Win95 abandoned stages by population level (`ChooseAndPlace` mode 4).
pub(super) const ABANDONED_POP1_BUILDINGS: [u8; 2] = [0x8A, 0x8B];
pub(super) const ABANDONED_POP2_BUILDINGS: [u8; 2] = [0xAA, 0xAB];
pub(super) const ABANDONED_POP3_BUILDINGS: [u8; 2] = [0xAC, 0xAD];
pub(super) const ABANDONED_POP4_BUILDINGS: [u8; 2] = [0xC4, 0xC5];
pub(super) const ABANDONED_1X1_BUILDINGS: [u8; 2] = ABANDONED_POP1_BUILDINGS;
pub(super) const ABANDONED_2X2_BUILDINGS: [u8; 4] = [0xAA, 0xAB, 0xAC, 0xAD];
pub(super) const ABANDONED_3X3_BUILDINGS: [u8; 2] = ABANDONED_POP4_BUILDINGS;

/// Win95 `wPositionAngle` @ `0x4DC4C8`, indexed by `wViewRotation`.
pub(super) const PE_POSITION_ANGLE: [u8; 4] = [0x80, 0x10, 0x20, 0x40];

/// Win95 `XZONCornerCheck` (`GetPopulatedTileAndLevel`): XZON `0xF0` bits vs view angle.
pub(super) fn xzon_corner_check(city: &City, x: usize, y: usize) -> bool {
    let angle = PE_POSITION_ANGLE[usize::from(city.rotation() & 3)];
    city.zone_at(x, y)
        .is_some_and(|zone| zone.raw_bits() & ZONE_CORNER_ALL & angle != 0)
}

/// GetPopulatedTileAndLevel origin: 1×1 is the tile; multi-tile only on the view-angle corner.
pub(super) fn rci_view_corner_footprint_origin(
    city: &City,
    x: usize,
    y: usize,
    building: u8,
    size: usize,
) -> Option<(usize, usize)> {
    if size <= 1 {
        return Some((x, y));
    }
    if !xzon_corner_check(city, x, y) {
        return None;
    }
    building_footprint_origin_containing(city, x, y, building, size)
}

pub(super) fn rci_zone_id(zone_id: u8) -> bool {
    matches!(
        zone_id,
        ZONE_RESIDENTIAL_LIGHT
            | ZONE_RESIDENTIAL_DENSE
            | ZONE_COMMERCIAL_LIGHT
            | ZONE_COMMERCIAL_DENSE
            | ZONE_INDUSTRIAL_LIGHT
            | ZONE_INDUSTRIAL_DENSE
    )
}

/// Win95 `PerhapsGeneralZoneChooseAndPlaceBuilding` @ `0x436E80`.
///
/// Mode is implied by zone kind. Residential pop1 uses `XVAL >> 6` (clamped 0..2) to pick a
/// 4-wide wealth band, then `rand&3` within the band. All other mode/pop pairs pick uniformly
/// from the ChooseAndPlace base/count tables.
/// Win95 growth-tick demand mapping: `iCurrentDemand = wCityDemand + 2000` (clamped).
/// `3 * iCurrentDemand / (popLevel + 1)` compared against `rand()` as u16.
/// `16384 / popLevel` construction-completion threshold.
/// `15 * iCurrentDemand / popLevel` abandoned-redevelop threshold.
/// Win95 `ChangeBuilding` body @ `0x437D40`.
///
/// Returns origin tiles to stamp (multi-tile buildings expand via `xbld_tile_size` at stamp time).
fn zone_is_dense_rci(zone_id: u8) -> bool {
    matches!(
        zone_id,
        ZONE_RESIDENTIAL_DENSE | ZONE_COMMERCIAL_DENSE | ZONE_INDUSTRIAL_DENSE
    )
}

pub(super) fn rci_kind_for_zone(zone_id: u8) -> Option<RciKind> {
    match zone_id {
        ZONE_RESIDENTIAL_LIGHT | ZONE_RESIDENTIAL_DENSE => Some(RciKind::Residential),
        ZONE_COMMERCIAL_LIGHT | ZONE_COMMERCIAL_DENSE => Some(RciKind::Commercial),
        ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE => Some(RciKind::Industrial),
        _ => None,
    }
}

/// Win95 `TILE_ROAD_LR`. `GetPopulatedTileAndLevel` feeds XBLD below this into
/// growth at `TILEPOPLEVEL_NONE`, so StartBuilding may overwrite rubble, trees,
/// small parks, and power lines. Roads and above stay out of vacant growth.
pub(super) fn rci_development_building_allowed(zone_id: u8, size: usize, building: u8) -> bool {
    let Some(kind) = rci_kind_for_zone(zone_id) else {
        return false;
    };
    rci_building_kind(building) == Some(kind)
        && usize::from(xbld_tile_size(building).max(1)) == size
        && rci_development_buildings_for_zone_and_size(zone_id, size)
            .is_some_and(|buildings| buildings.contains(&building))
}

pub(super) fn size_to_default_pop_level(size: usize) -> Option<u8> {
    match size {
        1 => Some(1),
        2 => Some(2),
        3 => Some(4),
        _ => None,
    }
}

pub(super) fn rci_building_pop_level(building: u8) -> Option<u8> {
    // `wBuildingPopLevel[building - 0x70]` from SIMCITY.EXE @ `0x4E7458`.
    Some(match building {
        0x70..=0x8B => 1,
        0x8C..=0x8F | 0x94..=0x98 | 0x9E..=0xA1 | 0xA6..=0xA7 | 0xAA..=0xAB => 2,
        0x90..=0x93 | 0x99..=0x9D | 0xA2..=0xA5 | 0xA8..=0xA9 | 0xAC..=0xAD => 3,
        0xAE..=0xC5 => 4,
        _ => return None,
    })
}

pub(super) fn pop_level_footprint_size(pop: u8) -> usize {
    match pop {
        1 => 1,
        2 | 3 => 2,
        4 => 3,
        _ => 1,
    }
}

pub(super) fn next_growth_pop_level(current_pop: u8) -> Option<u8> {
    match current_pop {
        0 => Some(1),
        1 => Some(2),
        2 => Some(3),
        3 => Some(4),
        _ => None,
    }
}

/// PE `FUN_004358B0` pre-roll StartBuilding gates: reject terminal pop 4 and
/// enforce the R/C XVAL minimum before consuming the growth-roll CRT value.
pub(super) fn pe_startbuilding_pre_roll_target_pop(
    city: &City,
    x: usize,
    y: usize,
    zone_id: u8,
    current_pop: u8,
) -> Option<u8> {
    let next_pop = next_growth_pop_level(current_pop)?;
    land_value_allows_start_from_pop(city, x, y, zone_id, current_pop).then_some(next_pop)
}

pub(super) fn land_value_allows_start_from_pop(
    city: &City,
    x: usize,
    y: usize,
    zone_id: u8,
    current_pop: u8,
) -> bool {
    // Win95: XVAL pop gates apply only when `iCurrZoneType < ZONE_LIGHT_INDUSTRIAL`.
    if zone_id >= ZONE_INDUSTRIAL_LIGHT {
        return true;
    }
    // CityGame treats an absent XVAL plane as zero here. PE New City explicitly
    // clears XVAL; an absent chunk in a malformed save has no recovered default.
    // When present, the native read uses microsim coordinates.
    let value = pe_xval_byte_at_city_coords(city, x, y).unwrap_or(0);
    match current_pop {
        1 => value >= XVAL_POPLEVEL_LOW,
        2 => value >= XVAL_POPLEVEL_MEDIUM,
        3 => value >= XVAL_POPLEVEL_HIGH,
        _ => true,
    }
}

/// PE ChooseAndPlace / GetXVAL / arco microsim density fold (`0x436ea0` / `0x435f35`):
/// `x &= ~1`, `y >>= 1` into the native 64×64 map. On the expanded 128×128 chunk,
/// that is the NW tile of the covering 2×2.
pub(super) fn pe_density_microsim_origin(x: usize, y: usize) -> (usize, usize) {
    ((x / 2) * 2, (y / 2) * 2)
}

/// PE ChooseAndPlace / GetXVAL city→microsim XVAL read (`0x436ea0` / `0x43bca6`).
pub(super) fn pe_xval_byte_at_city_coords(city: &City, x: usize, y: usize) -> Option<u8> {
    let (mx, my) = pe_density_microsim_origin(x, y);
    city.land_value_at(mx, my)
}

pub(super) fn abandoned_building_for_size(size: usize, x: usize, y: usize) -> Option<u8> {
    abandoned_building_for_pop(size_to_default_pop_level(size)?, x, y)
}

pub(super) fn rci_development_buildings_for_zone_and_pop(
    zone_id: u8,
    pop: u8,
) -> Option<&'static [u8]> {
    // PE ChooseAndPlace indexes by RCI kind and population level. The light
    // zone size limit belongs to StartBuilding, so existing sites can finish
    // or redevelop at their existing size after a density change.
    match (rci_kind_for_zone(zone_id)?, pop) {
        (RciKind::Residential, 1) => Some(RESIDENTIAL_1X1_BUILDINGS.as_slice()),
        (RciKind::Residential, 2) => Some(&RESIDENTIAL_2X2_BUILDINGS[0..4]),
        (RciKind::Residential, 3) => Some(&RESIDENTIAL_2X2_BUILDINGS[4..8]),
        (RciKind::Residential, 4) => Some(RESIDENTIAL_3X3_BUILDINGS.as_slice()),
        (RciKind::Commercial, 1) => Some(COMMERCIAL_1X1_BUILDINGS.as_slice()),
        (RciKind::Commercial, 2) => Some(&COMMERCIAL_2X2_BUILDINGS[0..5]),
        (RciKind::Commercial, 3) => Some(&COMMERCIAL_2X2_BUILDINGS[5..10]),
        (RciKind::Commercial, 4) => Some(COMMERCIAL_3X3_BUILDINGS.as_slice()),
        (RciKind::Industrial, 1) => Some(INDUSTRIAL_1X1_BUILDINGS.as_slice()),
        (RciKind::Industrial, 2) => Some(&INDUSTRIAL_2X2_BUILDINGS[0..4]),
        (RciKind::Industrial, 3) => Some(&INDUSTRIAL_2X2_BUILDINGS[4..8]),
        (RciKind::Industrial, 4) => Some(INDUSTRIAL_3X3_BUILDINGS.as_slice()),
        _ => None,
    }
}

pub(super) fn rci_development_buildings_for_zone_and_size(
    zone_id: u8,
    size: usize,
) -> Option<&'static [u8]> {
    match (zone_id, size) {
        (ZONE_RESIDENTIAL_LIGHT | ZONE_RESIDENTIAL_DENSE, 1) => {
            Some(RESIDENTIAL_1X1_BUILDINGS.as_slice())
        }
        (ZONE_RESIDENTIAL_DENSE, 2) => Some(RESIDENTIAL_2X2_BUILDINGS.as_slice()),
        (ZONE_RESIDENTIAL_DENSE, 3) => Some(RESIDENTIAL_3X3_BUILDINGS.as_slice()),
        (ZONE_COMMERCIAL_LIGHT | ZONE_COMMERCIAL_DENSE, 1) => {
            Some(COMMERCIAL_1X1_BUILDINGS.as_slice())
        }
        (ZONE_COMMERCIAL_DENSE, 2) => Some(COMMERCIAL_2X2_BUILDINGS.as_slice()),
        (ZONE_COMMERCIAL_DENSE, 3) => Some(COMMERCIAL_3X3_BUILDINGS.as_slice()),
        (ZONE_INDUSTRIAL_LIGHT | ZONE_INDUSTRIAL_DENSE, 1) => {
            Some(INDUSTRIAL_1X1_BUILDINGS.as_slice())
        }
        (ZONE_INDUSTRIAL_DENSE, 2) => Some(INDUSTRIAL_2X2_BUILDINGS.as_slice()),
        (ZONE_INDUSTRIAL_DENSE, 3) => Some(INDUSTRIAL_3X3_BUILDINGS.as_slice()),
        _ => None,
    }
}

pub(super) fn rci_development_variant_band_range(
    zone_id: u8,
    size: usize,
    buildings: &[u8],
    band: usize,
) -> (usize, usize) {
    let band = band.min(2);
    // Residential XBLD wealth classes from SC2k-docs / djm.cc. Commercial and industrial
    // lists are building-type catalogs, so they keep equal thirds.
    if rci_kind_for_zone(zone_id) == Some(RciKind::Residential) {
        match size {
            1 if buildings.len() == RESIDENTIAL_1X1_BUILDINGS.len() => {
                // 0x70-0x73 lower / 0x74-0x77 middle / 0x78-0x7B upper.
                return match band {
                    0 => (0, 4),
                    1 => (4, 8),
                    _ => (8, buildings.len()),
                };
            }
            2 if buildings.len() == RESIDENTIAL_2X2_BUILDINGS.len() => {
                // 0x8C-0x8E cheap+apts / 0x8F-0x90 nice apts / 0x91-0x93 condos.
                return match band {
                    0 => (0, 3),
                    1 => (3, 5),
                    _ => (5, buildings.len()),
                };
            }
            3 if buildings.len() == RESIDENTIAL_3X3_BUILDINGS.len() => {
                // Only two SC2K classes: 0xAE-0xAF apartments, 0xB0-0xB1 condos.
                // Low and mid score bands both select apartments.
                return match band {
                    0 | 1 => (0, 2),
                    _ => (2, buildings.len()),
                };
            }
            _ => {}
        }
    }

    if buildings.len() <= 2 {
        return (0, buildings.len());
    }
    let start = buildings.len() * band / 3;
    let end = if band == 2 {
        buildings.len()
    } else {
        buildings.len() * (band + 1) / 3
    }
    .max(start + 1);
    (start, end.min(buildings.len()))
}

pub(super) fn rci_development_quality_band(outcomes: CityServiceOutcomes) -> usize {
    rci_development_quality_band_for_score(rci_development_quality_score(outcomes))
}

pub(super) fn rci_development_quality_score(outcomes: CityServiceOutcomes) -> u32 {
    let quality = if outcomes.service_score == 0 {
        outcomes.land_value_quality
    } else {
        average_percent(&[
            outcomes.land_value_quality.min(100),
            outcomes.service_score.min(100),
        ])
    };
    quality.min(100)
}

pub(super) fn rci_development_quality_band_for_score(quality: u32) -> usize {
    match quality {
        0..=39 => 0,
        40..=69 => 1,
        _ => 2,
    }
}

pub(super) fn rci_development_quality_score_for_footprint(
    city: &City,
    zone_id: u8,
    footprint: &[(usize, usize)],
    city_outcomes: CityServiceOutcomes,
    services: &RciDevelopmentServices,
) -> u32 {
    let Some(kind) = rci_kind_for_zone(zone_id) else {
        return rci_development_quality_score(city_outcomes);
    };

    let city_quality = rci_development_quality_score(city_outcomes);
    let mut local_scores = Vec::new();
    let mut map_scores = Vec::new();
    rci_development_push_local_funded_score(
        &mut local_scores,
        &services.police,
        footprint,
        budget_service_funding_percent_for_city(city, 0),
    );
    rci_development_push_local_funded_score(
        &mut local_scores,
        &services.fire,
        footprint,
        budget_service_funding_percent_for_city(city, 1),
    );
    rci_development_push_local_funded_score(
        &mut local_scores,
        &services.transit,
        footprint,
        transport_budget_funding_percent_for_city(city),
    );
    rci_development_push_local_land_value_score(&mut map_scores, city, footprint);
    rci_development_push_local_density_score(
        &mut map_scores,
        city.crime_density_slice(),
        footprint,
        |x, y| city.crime_density_at(x, y),
    );
    rci_development_push_local_density_score(
        &mut map_scores,
        city.pollution_density_slice(),
        footprint,
        |x, y| city.pollution_density_at(x, y),
    );
    rci_development_push_local_density_score(
        &mut map_scores,
        city.traffic_density_slice(),
        footprint,
        |x, y| city.traffic_density_at(x, y),
    );
    match kind {
        RciKind::Residential => {
            rci_development_push_local_funded_score(
                &mut local_scores,
                &services.health,
                footprint,
                budget_service_funding_percent_for_city(city, 2),
            );
            rci_development_push_local_funded_score(
                &mut local_scores,
                &services.education,
                footprint,
                education_budget_funding_percent_for_city(city),
            );
            rci_development_push_local_score(&mut local_scores, &services.recreation, footprint);
        }
        RciKind::Commercial => {
            rci_development_push_local_score(&mut local_scores, &services.recreation, footprint);
        }
        RciKind::Industrial => {}
    }

    if local_scores.is_empty() && map_scores.is_empty() {
        return city_quality;
    }

    let mut quality_inputs = vec![city_outcomes.land_value_quality.min(100), city_quality];
    if !local_scores.is_empty() {
        quality_inputs.push(average_percent(&local_scores));
    }
    quality_inputs.extend(map_scores);
    average_percent(&quality_inputs)
}

pub(super) fn rci_development_push_local_land_value_score(
    scores: &mut Vec<u32>,
    city: &City,
    footprint: &[(usize, usize)],
) {
    if !rci_development_density_slice_has_signal(city.land_value_slice()) {
        return;
    }
    if let Some(local_value) =
        rci_development_average_metric_percent(footprint, |x, y| city.land_value_at(x, y))
    {
        scores.push(local_value);
    }
}

pub(super) fn rci_development_push_local_density_score<F>(
    scores: &mut Vec<u32>,
    slice: Option<&[u8]>,
    footprint: &[(usize, usize)],
    value_at: F,
) where
    F: FnMut(usize, usize) -> Option<u8>,
{
    if !rci_development_density_slice_has_signal(slice) {
        return;
    }
    if let Some(local_pressure) = rci_development_average_metric_percent(footprint, value_at) {
        scores.push(100_u32.saturating_sub(local_pressure.min(100)));
    }
}

pub(super) fn rci_development_density_slice_has_signal(slice: Option<&[u8]>) -> bool {
    slice.is_some_and(|values| values.iter().any(|&value| value != 0))
}

pub(super) fn rci_development_average_metric_percent<F>(
    footprint: &[(usize, usize)],
    mut value_at: F,
) -> Option<u32>
where
    F: FnMut(usize, usize) -> Option<u8>,
{
    if footprint.is_empty() {
        return None;
    }

    let mut total = 0_u32;
    let mut count = 0_u32;
    for &(x, y) in footprint {
        if let Some(value) = value_at(x, y) {
            total = total.saturating_add(u32::from(value));
            count += 1;
        }
    }
    if count == 0 {
        None
    } else {
        Some((total * 100 / (count * 255)).min(100))
    }
}

pub(super) fn rci_development_push_local_score(
    scores: &mut Vec<u32>,
    sources: &[CoverageSource],
    footprint: &[(usize, usize)],
) {
    rci_development_push_local_funded_score(scores, sources, footprint, 100);
}

pub(super) fn rci_development_push_local_funded_score(
    scores: &mut Vec<u32>,
    sources: &[CoverageSource],
    footprint: &[(usize, usize)],
    funding_percent: u32,
) {
    if sources.is_empty() || footprint.is_empty() {
        return;
    }
    let covered = footprint
        .iter()
        .filter(|&&(x, y)| sources.iter().any(|source| source.covers_tile(x, y)))
        .count() as u32;
    let coverage = covered * 100 / footprint.len() as u32;
    scores.push(coverage.saturating_mul(funding_percent.min(100)) / 100);
}

pub(super) fn rci_development_services_for_city(city: &City) -> RciDevelopmentServices {
    rci_development_services_for_city_with_power(city, &power_service_maps(city))
}

pub(super) fn rci_development_services_for_city_with_power(
    city: &City,
    power_maps: &PowerServiceMaps,
) -> RciDevelopmentServices {
    let mut transit_sources =
        coverage_sources_for_buildings(city, &[(233, 18), (236, 14), (237, 18)]);
    transit_sources.extend(coverage_sources_for_developed_zone(city, ZONE_AIRPORT, 30));
    transit_sources.extend(coverage_sources_for_developed_zone(city, ZONE_SEAPORT, 30));

    RciDevelopmentServices {
        // Win95 halves unpowered police/fire station coverage strength.
        police: coverage_sources_for_buildings_powered(city, power_maps, &[(210, 20)]),
        fire: coverage_sources_for_buildings_powered(city, power_maps, &[(211, 20)]),
        health: coverage_sources_for_buildings(city, &[(209, 22)]),
        education: coverage_sources_for_buildings(
            city,
            &[(214, 16), (217, 26), (245, 18), (212, 18)],
        ),
        recreation: coverage_sources_for_buildings(
            city,
            &[(13, 6), (213, 14), (215, 20), (218, 20), (248, 18)],
        ),
        transit: transit_sources,
    }
}

pub(super) fn rci_census(city: &City) -> RciCensus {
    let mut census = RciCensus::default();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = city.building_at(x, y);
            let Some(kind) = rci_building_kind(building) else {
                continue;
            };
            let capacity = rci_tile_capacity(building, kind);
            match kind {
                RciKind::Residential => {
                    census.residential_tiles = census.residential_tiles.saturating_add(1);
                    census.population = census.population.saturating_add(capacity);
                }
                RciKind::Commercial => {
                    census.commercial_tiles = census.commercial_tiles.saturating_add(1);
                    census.commercial_population =
                        census.commercial_population.saturating_add(capacity);
                    census.jobs = census.jobs.saturating_add(capacity);
                }
                RciKind::Industrial => {
                    census.industrial_tiles = census.industrial_tiles.saturating_add(1);
                    census.industrial_population =
                        census.industrial_population.saturating_add(capacity);
                    census.jobs = census.jobs.saturating_add(capacity);
                }
            }
        }
    }
    census
}

pub(super) fn rci_building_kind(building: u8) -> Option<RciKind> {
    match building {
        0x70..=0x7B | 0x8C..=0x93 | 0xAE..=0xB1 => Some(RciKind::Residential),
        0x7C..=0x83 | 0x94..=0x9D | 0xB2..=0xBB => Some(RciKind::Commercial),
        0x84..=0x87 | 0x9E..=0xA5 | 0xBC..=0xC1 => Some(RciKind::Industrial),
        _ => None,
    }
}

pub(super) fn rci_tile_capacity(building: u8, kind: RciKind) -> u32 {
    let size = xbld_tile_size(building).max(1);
    match (kind, size) {
        (RciKind::Residential, 1) => 12,
        (RciKind::Residential, 2) => 32,
        (RciKind::Residential, _) => 72,
        (RciKind::Commercial, 1) => 8,
        (RciKind::Commercial, 2) => 28,
        (RciKind::Commercial, _) => 64,
        (RciKind::Industrial, 1) => 10,
        (RciKind::Industrial, 2) => 36,
        (RciKind::Industrial, _) => 80,
    }
}

pub(super) fn is_abandoned_building(building: u8) -> bool {
    ABANDONED_1X1_BUILDINGS.contains(&building)
        || ABANDONED_2X2_BUILDINGS.contains(&building)
        || ABANDONED_3X3_BUILDINGS.contains(&building)
}

#[cfg(test)]
#[path = "rci/tests.rs"]
mod tests;
