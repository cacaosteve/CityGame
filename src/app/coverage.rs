//! Population census, facility counts, and service coverage maps.

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RciCensus {
    pub(super) residential_tiles: u32,
    pub(super) commercial_tiles: u32,
    pub(super) industrial_tiles: u32,
    pub(super) population: u32,
    pub(super) commercial_population: u32,
    pub(super) industrial_population: u32,
    pub(super) jobs: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PopulationStats {
    pub(super) normal_population: u32,
    pub(super) arcology_population: u32,
    pub(super) total_population: u32,
    pub(super) residential_population: u32,
    pub(super) commercial_population: u32,
    pub(super) industrial_population: u32,
    pub(super) populated_tiles: u32,
    pub(super) residential_tiles: u32,
    pub(super) commercial_tiles: u32,
    pub(super) industrial_tiles: u32,
    pub(super) work_force: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PopulationStatsMode {
    SavedFloor,
    LiveCensus,
}

/// Mac CODE 3 `export_162` publishes the summed arcology XMIC residents here.
const MAC_ARCOLOGY_RESIDENT_TOTAL_MISC_OFFSET: usize = 0x1020;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FacilityCensus {
    pub(super) power_plants: u32,
    pub(super) water_utilities: u32,
    pub(super) civic_services: u32,
    pub(super) education: u32,
    pub(super) parks: u32,
    pub(super) transit: u32,
    pub(super) rewards: u32,
    pub(super) arcologies: u32,
    pub(super) airport_tiles: u32,
    pub(super) seaport_tiles: u32,
    pub(super) developed_airport_tiles: u32,
    pub(super) developed_seaport_tiles: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ServiceCoverage {
    pub(super) served: u32,
    pub(super) demand: u32,
}

impl ServiceCoverage {
    pub(super) fn label(self) -> String {
        format!(
            "{}/{}",
            format_number(self.served),
            format_number(self.demand)
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CityServiceCoverage {
    pub(super) police: ServiceCoverage,
    pub(super) fire: ServiceCoverage,
    pub(super) health: ServiceCoverage,
    pub(super) education: ServiceCoverage,
    pub(super) recreation: ServiceCoverage,
    pub(super) transit: ServiceCoverage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CityServiceOutcomes {
    pub(super) life_expectancy: u32,
    pub(super) education_quotient: u32,
    pub(super) service_score: u32,
    pub(super) city_value: u32,
    pub(super) land_value: u32,
    pub(super) land_value_quality: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CoverageSource {
    pub(super) origin_x: usize,
    pub(super) origin_y: usize,
    pub(super) size: usize,
    pub(super) radius: u32,
}

impl CoverageSource {
    pub(super) fn covers_tile(self, x: usize, y: usize) -> bool {
        let max_x = self.origin_x + self.size.saturating_sub(1);
        let max_y = self.origin_y + self.size.saturating_sub(1);
        axis_distance_to_span(x, self.origin_x, max_x).max(axis_distance_to_span(
            y,
            self.origin_y,
            max_y,
        )) <= self.radius
    }

    pub(super) fn coverage_percent(self, x: usize, y: usize) -> u32 {
        let max_x = self.origin_x + self.size.saturating_sub(1);
        let max_y = self.origin_y + self.size.saturating_sub(1);
        let distance = axis_distance_to_span(x, self.origin_x, max_x).max(axis_distance_to_span(
            y,
            self.origin_y,
            max_y,
        ));
        if distance > self.radius {
            return 0;
        }
        if self.radius == 0 {
            return 100;
        }
        100_u32.saturating_sub(distance.saturating_mul(100) / self.radius.saturating_add(1))
    }
}
pub(super) fn population_stats_for_city(city: &City) -> PopulationStats {
    population_stats_for_city_with_mode(city, PopulationStatsMode::SavedFloor)
}

/// Apply the selected engine's saved population scalar without interpreting
/// overlapping tile-count fields through the other container's MISC layout.
pub(super) fn population_stats_for_city_with_fidelity(
    city: &City,
    fidelity: EngineFidelity,
) -> PopulationStats {
    if !fidelity.uses_win95() {
        // Mac's live RCI census is engine state on either container; PE's
        // saved tile-count floors must not leak into Mac-fidelity consumers.
        return live_population_stats_for_city_with_fidelity(city, fidelity);
    }

    let mut population = population_stats_for_city(city);
    if city.kind == CityFileKind::MacCity {
        // PE dwCityPopulation and Mac fffff01a share MISC +0x102c. The
        // selected PE engine reads this saved DWORD even in a Mac .city file;
        // the adjacent PE tile-count offsets are not shared and stay census
        // derived above.
        population.normal_population = city
            .misc_u32_at(MISC_NORMAL_POPULATION_OFFSET)
            .unwrap_or(0)
            .max(population.normal_population);
        population.arcology_population = city
            .misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET)
            .unwrap_or(0)
            .max(population.arcology_population);
        population.total_population = population
            .normal_population
            .saturating_add(population.arcology_population);
    }
    population
}

pub(super) fn live_population_stats_for_city(city: &City) -> PopulationStats {
    population_stats_for_city_with_mode(city, PopulationStatsMode::LiveCensus)
}

/// Use the selected simulation's arcology state without changing the
/// format-only census used by other callers and the Win95 traffic path.
pub(super) fn live_population_stats_for_city_with_fidelity(
    city: &City,
    fidelity: EngineFidelity,
) -> PopulationStats {
    let mut population = live_population_stats_for_city(city);
    if !fidelity.uses_win95() {
        population.arcology_population = city.mac_arcology_resident_total();
        population.total_population = population
            .normal_population
            .saturating_add(population.arcology_population);
    }
    population
}

impl ViewerState {
    pub(super) fn live_population_stats_for_fidelity(&self) -> PopulationStats {
        live_population_stats_for_city_with_fidelity(&self.city, self.engine_fidelity)
    }
}

pub(super) fn work_force_percent_for_population(population: PopulationStats) -> u32 {
    if population.total_population == 0 {
        0
    } else {
        population.work_force.saturating_mul(100) / population.total_population
    }
    .min(100)
}

/// After EQ_LE averages exist, MISC `0x44` is PE `0x4cb460` (fertile pop × 100 /
/// (table pop + 1)); otherwise keep the job-census stand-in.
pub(super) fn work_force_percent_for_city(city: &City, population: PopulationStats) -> u32 {
    work_force_percent_for_city_with_fidelity(city, population, EngineFidelity::Win95)
}

pub(super) fn work_force_percent_for_city_with_fidelity(
    city: &City,
    population: PopulationStats,
    fidelity: EngineFidelity,
) -> u32 {
    if fidelity.uses_win95() && city.pe_eq_averages_ready() {
        city.pe_eq_workforce_percent().min(100)
    } else {
        work_force_percent_for_population(population)
    }
}

/// Win95 `dwCityUnemployment` @ `0x4CA5F8`.
///
/// PE `FUN_00413E50` publishes `100 * slot7 / (slot0 + slot7 + 1)` into this
/// global on day 21; live consumers read the stored percent rather than
/// recomputing from an abandoned-census stand-in.
pub(super) fn unemployment_percent_for_city(city: &City, population: PopulationStats) -> u32 {
    unemployment_percent_for_city_with_fidelity(city, population, EngineFidelity::Win95)
}

pub(super) fn unemployment_percent_for_city_with_fidelity(
    city: &City,
    population: PopulationStats,
    fidelity: EngineFidelity,
) -> u32 {
    if fidelity.uses_win95() {
        return city.pe_unemployment_percent();
    }
    city.mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET)
        .unwrap_or_else(|| abandoned_census_unemployment_percent(city, population))
}

/// Abandoned-tile census ratio used when the selected engine and file layout
/// do not share either edition's saved unemployment field.
pub(super) fn abandoned_census_unemployment_percent(
    city: &City,
    population: PopulationStats,
) -> u32 {
    let abandoned = abandoned_population_for_city(city);
    abandoned.saturating_mul(100)
        / abandoned
            .saturating_add(population.normal_population)
            .saturating_add(1)
}

/// Occupied-capacity proxy for abandoned RCI tiles (same per-tile weights as `rci_census`).
pub(super) fn abandoned_population_for_city(city: &City) -> u32 {
    let mut total = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = city.building_at(x, y);
            if !is_abandoned_building(building) {
                continue;
            }
            let kind = city
                .zone_at(x, y)
                .and_then(|zone| rci_kind_for_zone(zone.zone_id()))
                .unwrap_or(RciKind::Residential);
            total = total.saturating_add(rci_tile_capacity(building, kind));
        }
    }
    total
}

pub(super) fn population_stats_for_city_with_mode(
    city: &City,
    mode: PopulationStatsMode,
) -> PopulationStats {
    let census = rci_census(city);
    let live_populated_tiles = census
        .residential_tiles
        .saturating_add(census.commercial_tiles)
        .saturating_add(census.industrial_tiles);
    // Mac CODE 3 and PE both use MISC dword 1032 (+0x1020) for the arcology
    // resident total. The Mac-fidelity wrapper below reads that shared saved
    // role on either layout; this legacy format-only census keeps its existing
    // derived fallback for Win-layout callers.
    let live_arcology_population = if city.kind == CityFileKind::MacCity {
        city.mac_state_misc_u32_at(MAC_ARCOLOGY_RESIDENT_TOTAL_MISC_OFFSET)
            .unwrap_or_else(|| arcology_population_for_city(city))
    } else {
        arcology_population_for_city(city)
    };

    let (
        normal_population,
        arcology_population,
        populated_tiles,
        residential_tiles,
        commercial_tiles,
        industrial_tiles,
    ) = match mode {
        PopulationStatsMode::SavedFloor if city.kind == CityFileKind::MacCity => (
            census.population,
            live_arcology_population,
            live_populated_tiles,
            census.residential_tiles,
            census.commercial_tiles,
            census.industrial_tiles,
        ),
        PopulationStatsMode::SavedFloor => (
            city.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET)
                .unwrap_or(0)
                .max(census.population),
            city.pe_dw_arcology_population()
                .max(live_arcology_population),
            city.misc_u32_at(MISC_POPULATED_TILE_COUNT_OFFSET)
                .unwrap_or(0)
                .max(live_populated_tiles),
            city.misc_u32_at(MISC_RESIDENTIAL_TILE_COUNT_OFFSET)
                .unwrap_or(0)
                .max(census.residential_tiles),
            city.misc_u32_at(MISC_COMMERCIAL_TILE_COUNT_OFFSET)
                .unwrap_or(0)
                .max(census.commercial_tiles),
            city.misc_u32_at(MISC_INDUSTRIAL_TILE_COUNT_OFFSET)
                .unwrap_or(0)
                .max(census.industrial_tiles),
        ),
        PopulationStatsMode::LiveCensus => (
            census.population,
            live_arcology_population,
            live_populated_tiles,
            census.residential_tiles,
            census.commercial_tiles,
            census.industrial_tiles,
        ),
    };
    let work_force = match mode {
        PopulationStatsMode::SavedFloor => {
            // PE workforce% BSS `0x4cb460` (MISC `+0x44`).
            let percent = city.pe_eq_workforce_percent().min(100);
            (normal_population
                .saturating_add(arcology_population)
                .saturating_mul(percent)
                / 100)
                .max(census.jobs)
        }
        PopulationStatsMode::LiveCensus => census.jobs,
    };

    PopulationStats {
        normal_population,
        arcology_population,
        total_population: normal_population.saturating_add(arcology_population),
        residential_population: census.population,
        commercial_population: census.commercial_population,
        industrial_population: census.industrial_population,
        populated_tiles,
        residential_tiles,
        commercial_tiles,
        industrial_tiles,
        work_force,
    }
}

pub(super) fn arcology_population_for_city(city: &City) -> u32 {
    ARCOLOGY_CHOICES
        .iter()
        .map(|choice| {
            complete_building_count_for_city(
                city,
                choice.building,
                usize::from(xbld_tile_size(choice.building).max(1)),
            )
            .saturating_mul(choice.capacity)
        })
        .fold(0_u32, u32::saturating_add)
}

pub(super) fn complete_building_count_for_city(city: &City, building: u8, size: usize) -> u32 {
    if size == 0 || size > CITY_WIDTH || size > CITY_HEIGHT {
        return 0;
    }

    let mut claimed = vec![false; CITY_WIDTH * CITY_HEIGHT];
    let mut count = 0_u32;
    for origin_y in 0..=CITY_HEIGHT - size {
        for origin_x in 0..=CITY_WIDTH - size {
            let complete = (origin_y..origin_y + size).all(|y| {
                (origin_x..origin_x + size).all(|x| {
                    let index = y * CITY_WIDTH + x;
                    !claimed[index] && city.building_at(x, y) == building
                })
            });
            if !complete {
                continue;
            }
            count = count.saturating_add(1);
            for y in origin_y..origin_y + size {
                for x in origin_x..origin_x + size {
                    claimed[y * CITY_WIDTH + x] = true;
                }
            }
        }
    }
    count
}

pub(super) fn complete_building_total_for_city(city: &City, buildings: &[u8]) -> u32 {
    buildings.iter().fold(0_u32, |total, &building| {
        total.saturating_add(complete_building_count_for_city(
            city,
            building,
            usize::from(xbld_tile_size(building).max(1)),
        ))
    })
}

pub(super) fn zone_tile_count_for_city(city: &City, zone_id: u8) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == zone_id)
            {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

pub(super) fn developed_zone_tile_count_for_city(city: &City, zone_id: u8) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == zone_id)
                && special_zone_developed_building(zone_id, city.building_at(x, y))
            {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

pub(super) fn special_zone_building_tile_count_for_city(
    city: &City,
    zone_id: u8,
    building_id: u8,
) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == zone_id)
                && city.building_at(x, y) == building_id
            {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

pub(super) fn facility_census_for_city(city: &City) -> FacilityCensus {
    FacilityCensus {
        power_plants: complete_building_total_for_city(
            city,
            &[198, 199, 200, 201, 202, 203, 204, 205, 206, 207],
        ),
        water_utilities: complete_building_total_for_city(city, &[220, 235, 244, 250]),
        civic_services: complete_building_total_for_city(city, &[209, 210, 211, 216]),
        education: complete_building_total_for_city(city, &[212, 214, 217, 245]),
        parks: complete_building_total_for_city(city, &[13, 213, 215, 218, 248]),
        transit: complete_building_total_for_city(city, &[233, 236, 237]),
        rewards: complete_building_total_for_city(city, &[208, 219, 243, 255]),
        arcologies: complete_building_total_for_city(city, &[251, 252, 253, 254]),
        airport_tiles: zone_tile_count_for_city(city, ZONE_AIRPORT),
        seaport_tiles: zone_tile_count_for_city(city, ZONE_SEAPORT),
        developed_airport_tiles: developed_zone_tile_count_for_city(city, ZONE_AIRPORT),
        developed_seaport_tiles: developed_zone_tile_count_for_city(city, ZONE_SEAPORT),
    }
}

pub(super) fn city_service_coverage_for_city(city: &City) -> CityServiceCoverage {
    city_service_coverage_for_city_with_power(city, &power_service_maps(city))
}

pub(super) fn city_service_coverage_for_city_with_power(
    city: &City,
    power_maps: &PowerServiceMaps,
) -> CityServiceCoverage {
    let sources = rci_development_services_for_city_with_power(city, power_maps);
    CityServiceCoverage {
        police: service_coverage_for_city(city, &sources.police, |_| true),
        fire: service_coverage_for_city(city, &sources.fire, |_| true),
        health: service_coverage_for_city(city, &sources.health, |kind| {
            kind == RciKind::Residential
        }),
        education: service_coverage_for_city(city, &sources.education, |kind| {
            kind == RciKind::Residential
        }),
        recreation: service_coverage_for_city(city, &sources.recreation, |kind| {
            kind == RciKind::Residential
        }),
        transit: service_coverage_for_city(city, &sources.transit, |_| true),
    }
}

pub(super) fn city_service_outcomes_for_city(city: &City) -> CityServiceOutcomes {
    city_service_outcomes_for_city_with_fidelity(city, EngineFidelity::Win95)
}

pub(super) fn city_service_outcomes_for_city_with_fidelity(
    city: &City,
    fidelity: EngineFidelity,
) -> CityServiceOutcomes {
    let population = population_stats_for_city_with_fidelity(city, fidelity);
    city_service_outcomes_for_population_with_power_and_fidelity(
        city,
        population,
        true,
        &power_service_maps(city),
        fidelity,
    )
}

pub(super) fn live_city_service_outcomes_for_city(city: &City) -> CityServiceOutcomes {
    let population = live_population_stats_for_city(city);
    city_service_outcomes_for_population(city, population, false)
}

pub(super) fn live_city_service_outcomes_for_city_with_power(
    city: &City,
    power_maps: &PowerServiceMaps,
) -> CityServiceOutcomes {
    live_city_service_outcomes_for_city_with_power_and_fidelity(
        city,
        power_maps,
        EngineFidelity::Win95,
    )
}

pub(super) fn live_city_service_outcomes_for_city_with_power_and_fidelity(
    city: &City,
    power_maps: &PowerServiceMaps,
    fidelity: EngineFidelity,
) -> CityServiceOutcomes {
    let population = live_population_stats_for_city_with_fidelity(city, fidelity);
    city_service_outcomes_for_population_with_power_and_fidelity(
        city, population, false, power_maps, fidelity,
    )
}

pub(super) fn saved_city_service_outcomes_for_city(city: &City) -> CityServiceOutcomes {
    saved_city_service_outcomes_for_city_with_fidelity(city, EngineFidelity::Win95)
}

pub(super) fn saved_city_service_outcomes_for_city_with_fidelity(
    city: &City,
    fidelity: EngineFidelity,
) -> CityServiceOutcomes {
    let land_value = city
        .land_value_density_sum()
        .filter(|value| *value > 0)
        .unwrap_or(city.pe_dw_city_land_value);
    let land_value_quality = city
        .land_value_slice()
        .and_then(land_value_quality_from_density)
        .unwrap_or_else(|| city.pe_dw_city_land_value.min(100));
    // The saved LE/EQ positions are shared; the chosen engine owns their use.
    let (life_expectancy, education_quotient) = if fidelity.uses_win95() {
        (city.pe_eq_life_average(), city.pe_eq_education_average())
    } else {
        (0, 0)
    };
    CityServiceOutcomes {
        life_expectancy,
        education_quotient,
        service_score: 0,
        city_value: {
            let saved = city.pe_city_value.max(0) as u32;
            if saved > 0 {
                saved
            } else {
                city_value_for_land_value(land_value)
            }
        },
        land_value,
        land_value_quality,
    }
}

pub(super) fn city_service_outcomes_for_population(
    city: &City,
    population: PopulationStats,
    use_saved_fallback: bool,
) -> CityServiceOutcomes {
    city_service_outcomes_for_population_with_power(
        city,
        population,
        use_saved_fallback,
        &power_service_maps(city),
    )
}

pub(super) fn city_service_outcomes_for_population_with_power(
    city: &City,
    population: PopulationStats,
    use_saved_fallback: bool,
    power_maps: &PowerServiceMaps,
) -> CityServiceOutcomes {
    city_service_outcomes_for_population_with_power_and_fidelity(
        city,
        population,
        use_saved_fallback,
        power_maps,
        EngineFidelity::Win95,
    )
}

pub(super) fn city_service_outcomes_for_population_with_power_and_fidelity(
    city: &City,
    population: PopulationStats,
    use_saved_fallback: bool,
    power_maps: &PowerServiceMaps,
    fidelity: EngineFidelity,
) -> CityServiceOutcomes {
    if population.populated_tiles == 0 {
        return if use_saved_fallback {
            saved_city_service_outcomes_for_city_with_fidelity(city, fidelity)
        } else {
            CityServiceOutcomes::default()
        };
    }
    if use_saved_fallback && live_population_stats_for_city(city).populated_tiles == 0 {
        return saved_city_service_outcomes_for_city_with_fidelity(city, fidelity);
    }

    let coverage = city_service_coverage_for_city_with_power(city, power_maps);
    let built_counts = budget_service_built_counts_for_city(city);
    let ordinance_effects = ordinance_effects_for_city_with_fidelity(city, population, fidelity);
    let police = clamp_percent_i32(
        funded_coverage_percent(city, 0, coverage.police),
        ordinance_effects.police_bonus,
    );
    let fire = clamp_percent_i32(
        thresholded_funded_service_coverage_percent(
            city,
            1,
            coverage.fire,
            population,
            FIRE_STATION_DEMAND_POPULATION_STEP,
            built_counts.fire,
        ),
        ordinance_effects.fire_bonus,
    );
    let health = clamp_percent_i32(
        thresholded_funded_service_coverage_percent(
            city,
            2,
            coverage.health,
            population,
            HOSPITAL_DEMAND_POPULATION_STEP,
            built_counts.health,
        ),
        ordinance_effects.health_bonus,
    );
    let education = thresholded_service_coverage_percent(
        coverage.education,
        population,
        SCHOOL_DEMAND_POPULATION_STEP,
        built_counts.schools,
    )
    .saturating_mul(education_budget_funding_percent_for_city(city))
        / 100;
    let education = clamp_percent_i32(education, ordinance_effects.education_bonus);
    let recreation = clamp_percent_i32(
        coverage_percent(coverage.recreation),
        ordinance_effects.recreation_bonus,
    );
    let transit = coverage_percent(coverage.transit)
        .saturating_mul(transport_budget_funding_percent_for_city(city))
        / 100;
    let safety = average_percent(&[police, fire]);
    let service_score = average_percent(&[safety, health, education, recreation, transit]);
    let facilities = facility_census_for_city(city);

    let life_expectancy =
        (35 + health * 35 / 100 + safety * 10 / 100 + recreation * 10 / 100).clamp(20, 95);
    let education_quotient =
        (20 + education * 85 / 100 + facilities.education.min(10) * 3).clamp(0, 150);
    let (life_expectancy, education_quotient) =
        if fidelity.uses_win95() && city.pe_eq_averages_ready() {
            (city.pe_eq_life_average(), city.pe_eq_education_average())
        } else {
            (life_expectancy, education_quotient)
        };
    let computed_land_value_quality = clamp_percent_i32(
        20 + safety * 15 / 100 + health * 15 / 100 + recreation * 35 / 100 + transit * 15 / 100,
        ordinance_effects.land_value_bonus,
    );
    let land_value_quality =
        land_value_quality_for_city(city, computed_land_value_quality, use_saved_fallback);
    let land_value = land_value_total_for_city(city, land_value_quality, use_saved_fallback);
    let city_value = if use_saved_fallback {
        let saved = city.pe_city_value.max(0) as u32;
        if saved > 0 {
            saved
        } else {
            city_value_for_land_value(land_value)
        }
    } else {
        city_value_for_land_value(land_value)
    };

    CityServiceOutcomes {
        life_expectancy,
        education_quotient,
        service_score,
        city_value,
        land_value,
        land_value_quality,
    }
}
pub(super) fn scale_u32_percent(value: u32, percent: u32) -> u32 {
    saturating_u32_from_u64(u64::from(value).saturating_mul(u64::from(percent)) / 100)
}

pub(super) fn coverage_percent(coverage: ServiceCoverage) -> u32 {
    coverage
        .served
        .saturating_mul(100)
        .checked_div(coverage.demand)
        .unwrap_or(100)
}

pub(super) fn citizen_service_required_count(
    population: PopulationStats,
    population_step: u32,
) -> u32 {
    population
        .normal_population
        .checked_div(population_step)
        .unwrap_or(0)
}

pub(super) fn thresholded_service_coverage_percent(
    coverage: ServiceCoverage,
    population: PopulationStats,
    population_step: u32,
    built_count: u32,
) -> u32 {
    let required_count = citizen_service_required_count(population, population_step);
    if required_count == 0 {
        return 100;
    }

    let coverage = coverage_percent(coverage).min(100);
    let facility_capacity = built_count.min(required_count).saturating_mul(100) / required_count;
    coverage.saturating_mul(facility_capacity) / 100
}

pub(super) fn clamp_percent_i32(base: u32, delta: i32) -> u32 {
    (base.min(100) as i32).saturating_add(delta).clamp(0, 100) as u32
}
pub(super) fn average_percent(values: &[u32]) -> u32 {
    if values.is_empty() {
        0
    } else {
        values.iter().copied().sum::<u32>() / values.len() as u32
    }
}

pub(super) fn saturating_u32_from_u64(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub(super) fn service_coverage_for_city<F>(
    city: &City,
    sources: &[CoverageSource],
    demand_kind: F,
) -> ServiceCoverage
where
    F: Fn(RciKind) -> bool,
{
    let mut coverage = ServiceCoverage::default();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some(kind) = rci_building_kind(city.building_at(x, y)) else {
                continue;
            };
            if !demand_kind(kind) {
                continue;
            }
            coverage.demand = coverage.demand.saturating_add(1);
            if sources.iter().any(|source| source.covers_tile(x, y)) {
                coverage.served = coverage.served.saturating_add(1);
            }
        }
    }
    coverage
}

pub(super) fn coverage_sources_for_buildings(
    city: &City,
    specs: &[(u8, u32)],
) -> Vec<CoverageSource> {
    coverage_sources_for_buildings_with_radius_scale(city, specs, |_ox, _oy, _size| 1.0)
}

pub(super) fn coverage_sources_for_buildings_powered(
    city: &City,
    power_maps: &PowerServiceMaps,
    specs: &[(u8, u32)],
) -> Vec<CoverageSource> {
    coverage_sources_for_buildings_with_radius_scale(city, specs, |origin_x, origin_y, size| {
        let powered = (origin_y..origin_y + size).any(|y| {
            (origin_x..origin_x + size).any(|x| power_maps.powered_tiles[y * CITY_WIDTH + x])
        });
        if powered { 1.0 } else { 0.5 }
    })
}

pub(super) fn coverage_sources_for_buildings_with_radius_scale(
    city: &City,
    specs: &[(u8, u32)],
    radius_scale: impl Fn(usize, usize, usize) -> f32,
) -> Vec<CoverageSource> {
    let mut claimed = vec![false; CITY_WIDTH * CITY_HEIGHT];
    let mut sources = Vec::new();
    for &(building, radius) in specs {
        let size = usize::from(xbld_tile_size(building).max(1));
        if size > CITY_WIDTH || size > CITY_HEIGHT {
            continue;
        }
        for origin_y in 0..=CITY_HEIGHT - size {
            for origin_x in 0..=CITY_WIDTH - size {
                let complete = (origin_y..origin_y + size).all(|y| {
                    (origin_x..origin_x + size).all(|x| {
                        let index = y * CITY_WIDTH + x;
                        !claimed[index] && city.building_at(x, y) == building
                    })
                });
                if !complete {
                    continue;
                }
                let scale = radius_scale(origin_x, origin_y, size);
                let scaled_radius = ((radius as f32) * scale).round().max(1.0) as u32;
                sources.push(CoverageSource {
                    origin_x,
                    origin_y,
                    size,
                    radius: scaled_radius,
                });
                for y in origin_y..origin_y + size {
                    for x in origin_x..origin_x + size {
                        claimed[y * CITY_WIDTH + x] = true;
                    }
                }
            }
        }
    }
    sources
}

pub(super) fn coverage_sources_for_developed_zone(
    city: &City,
    zone_id: u8,
    radius: u32,
) -> Vec<CoverageSource> {
    let mut claimed = vec![false; CITY_WIDTH * CITY_HEIGHT];
    let mut sources = Vec::new();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = y * CITY_WIDTH + x;
            if claimed[index] {
                continue;
            }
            let building = city.building_at(x, y);
            if !special_zone_developed_building(zone_id, building) {
                continue;
            }
            let size = usize::from(xbld_tile_size(building).max(1));
            let Some(footprint) = building_footprint_from_origin(x, y, size) else {
                continue;
            };
            if !footprint.iter().all(|&(tx, ty)| {
                !claimed[ty * CITY_WIDTH + tx]
                    && city.building_at(tx, ty) == building
                    && city
                        .zone_at(tx, ty)
                        .is_some_and(|zone| zone.zone_id() == zone_id)
            }) {
                continue;
            }

            sources.push(CoverageSource {
                origin_x: x,
                origin_y: y,
                size,
                radius,
            });
            for &(tx, ty) in &footprint {
                claimed[ty * CITY_WIDTH + tx] = true;
            }
        }
    }
    sources
}

pub(super) fn axis_distance_to_span(value: usize, min: usize, max: usize) -> u32 {
    if value < min {
        (min - value) as u32
    } else if value > max {
        (value - max) as u32
    } else {
        0
    }
}
