//! Live scenario environment counters (crime, traffic, pollution) and city fame.

use super::*;

pub(super) fn graph_points_for_scale(value: u32, scale: u32) -> u32 {
    if value == 0 || scale == 0 {
        0
    } else {
        value.div_ceil(scale)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ScenarioProgress {
    pub(super) population: PopulationStats,
    pub(super) outcomes: CityServiceOutcomes,
    pub(super) counters: CityScenarioCounters,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ScenarioGoalCounts {
    pub(super) known: u32,
    pub(super) complete: u32,
    pub(super) pending: u32,
    /// The PE's signed R/C/I and cash comparisons still run for zero-valued
    /// SCEN fields, even though the UI does not present those as configured goals.
    pub(super) implicit_minimum_failed: bool,
}

impl ScenarioGoalCounts {
    fn add_known(&mut self, is_complete: bool) {
        self.known = self.known.saturating_add(1);
        if is_complete {
            self.complete = self.complete.saturating_add(1);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScenarioOutcome {
    InProgress,
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct ScenarioText {
    pub(super) selection: Option<String>,
    pub(super) extended: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct ScenarioGoals {
    pub(super) disaster_type: u16,
    pub(super) disaster_x: u8,
    pub(super) disaster_y: u8,
    pub(super) time_limit_months: u16,
    pub(super) city_population_goal: u32,
    pub(super) residential_population_goal: u32,
    pub(super) commercial_population_goal: u32,
    pub(super) industrial_population_goal: u32,
    pub(super) fund_goal: u32,
    pub(super) land_value_goal: u32,
    /// SCEN `wScenarioLEGoal` (BE u16 @ `0x22`).
    pub(super) life_expectancy_goal: u32,
    /// SCEN `wScenarioEQGoal` (BE u16 @ `0x24`).
    pub(super) education_goal: u32,
    pub(super) pollution_limit: u32,
    pub(super) crime_limit: u32,
    pub(super) traffic_limit: u32,
    pub(super) build_item_1: u8,
    pub(super) build_item_2: u8,
    pub(super) build_item_1_tiles: Option<u16>,
    pub(super) build_item_2_tiles: Option<u16>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CityScenarioCounters {
    pub(super) crime: u32,
    pub(super) traffic: u32,
    pub(super) pollution: u32,
}
pub(super) fn live_city_scenario_counters_for_city(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
) -> CityScenarioCounters {
    live_city_scenario_counters_for_city_with_power(
        city,
        population,
        outcomes,
        &power_service_maps(city),
    )
}

pub(super) fn live_city_scenario_counters_for_city_with_power(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
    power_maps: &PowerServiceMaps,
) -> CityScenarioCounters {
    let crime = live_crime_count_for_city_with_power(city, population, outcomes, power_maps);
    let traffic = live_traffic_count_for_city_with_power(city, population, power_maps);
    let pollution = live_pollution_count_for_city(city, population);

    CityScenarioCounters {
        // Crime/pollution prefer live XCRM/XPLT microsim sums when maps exist.
        crime,
        // A present XTRF map is authoritative even when its live sum is zero; do not
        // resurrect stale saved MISC traffic in that case.
        traffic: if city.has_traffic_density_map() {
            traffic
        } else {
            city.misc_traffic_count().unwrap_or(traffic)
        },
        pollution,
    }
}

pub(super) fn live_city_fame_value(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
    counters: CityScenarioCounters,
) -> u32 {
    live_city_fame_value_with_fidelity(city, population, outcomes, counters, EngineFidelity::Win95)
}

pub(super) fn live_city_fame_value_with_fidelity(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
    counters: CityScenarioCounters,
    fidelity: EngineFidelity,
) -> u32 {
    let people = if fidelity.uses_win95() && city.pe_zone_pops_ready() {
        city.pe_dw_city_population()
            .saturating_add(population.arcology_population)
    } else {
        population.total_population
    };
    if people == 0 {
        return 0;
    }

    let population_score = graph_points_for_scale(people, 2_500).min(30);
    let service_score = outcomes.service_score.min(100) * 30 / 100;
    let land_score = outcomes.land_value_quality.min(100) * 20 / 100;
    let health_score = outcomes.life_expectancy.min(100) * 10 / 100;
    let education_score = outcomes.education_quotient.min(150) * 10 / 150;
    let raw_score = population_score
        .saturating_add(service_score)
        .saturating_add(land_score)
        .saturating_add(health_score)
        .saturating_add(education_score)
        .min(100);
    let pressure = counters
        .crime
        .saturating_add(counters.traffic)
        .saturating_add(counters.pollution);
    let pressure_scale = people.saturating_mul(3).max(1);
    let penalty = pressure
        .saturating_mul(100)
        .checked_div(pressure_scale)
        .unwrap_or(0)
        .min(50);

    raw_score.saturating_sub(penalty).min(100)
}
pub(super) fn live_counter_or_saved_sum(live: u32, saved: Option<u32>) -> u32 {
    if live == 0 { saved.unwrap_or(0) } else { live }
}

/// City crime counter for MISC / scenarios.
/// Prefers the live `XCRM` microsim sum (day-2 density scan) over the pressure heuristic.
pub(super) fn live_crime_count_for_city(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
) -> u32 {
    live_crime_count_for_city_with_power(city, population, outcomes, &power_service_maps(city))
}

pub(super) fn live_crime_count_for_city_with_power(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
    power_maps: &PowerServiceMaps,
) -> u32 {
    if let Some(values) = city.crime_density_slice() {
        return density_map_microsim64_sum(values);
    }
    if population.populated_tiles == 0 {
        return 0;
    }

    let coverage = city_service_coverage_for_city_with_power(city, power_maps);
    let police = funded_coverage_percent(city, 0, coverage.police).min(100);
    let police_gap = 100_u32.saturating_sub(police);
    let value_gap = 100_u32.saturating_sub(outcomes.land_value_quality.min(100));
    let pressure = 45_u32
        .saturating_add(police_gap)
        .saturating_add(value_gap / 2);

    let mut total = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = city.building_at(x, y);
            let Some(kind) = rci_building_kind(building) else {
                continue;
            };
            let capacity = rci_tile_capacity(building, kind);
            let weight = match kind {
                RciKind::Residential => capacity,
                RciKind::Commercial => capacity.saturating_mul(2),
                RciKind::Industrial => capacity,
            };
            total = total.saturating_add(weight.saturating_mul(pressure) / 100);
        }
    }

    if ordinance_enabled_for_city(city, ORDINANCE_LEGALIZED_GAMBLING) {
        total = scale_u32_percent(total, 120);
    }
    if ordinance_enabled_for_city(city, ORDINANCE_ANTI_DRUG_CAMPAIGN) {
        total = scale_u32_percent(total, 90);
    }
    if ordinance_enabled_for_city(city, ORDINANCE_NEIGHBORHOOD_WATCH) {
        total = scale_u32_percent(total, 88);
    }
    total
}

pub(super) fn live_traffic_count_for_city(city: &City, population: PopulationStats) -> u32 {
    live_traffic_count_for_city_with_power(city, population, &power_service_maps(city))
}

/// Same as [`live_traffic_count_for_city`], but the map-less heuristic uses the given
/// FUN_004679B0 plant maps (live January / status paths pass process-CRT maps).
pub(super) fn live_traffic_count_for_city_with_power(
    city: &City,
    population: PopulationStats,
    power_maps: &PowerServiceMaps,
) -> u32 {
    // Win95 FUN_004586F1 stores the post-day-19-decay XTRF microsim aggregate in MISC.
    // Prefer that PE value whenever a traffic map exists so day-21 refresh does not
    // replace the executable's aggregate with a separate heuristic.
    if city.has_traffic_density_map() {
        return trip_generator::traffic_density_microsim_sum(city);
    }

    let coverage = city_service_coverage_for_city_with_power(city, power_maps);
    let transit = coverage_percent(coverage.transit)
        .saturating_mul(transport_budget_funding_percent_for_city(city))
        / 100;
    let transit_gap = 100_u32.saturating_sub(transit.min(100));
    let commuter_pressure = population.work_force.saturating_mul(60 + transit_gap) / 100;

    let mut network_pressure = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = city.building_at(x, y);
            if !is_road_tile(building) && !is_highway_tile(building) {
                continue;
            }
            let base = if is_highway_tile(building) { 8 } else { 4 };
            let density = city
                .traffic_density_at(x, y)
                .map(u32::from)
                .unwrap_or(base * 8);
            network_pressure = network_pressure.saturating_add(base.saturating_add(density / 16));
        }
    }

    let mut total = commuter_pressure.saturating_add(network_pressure);
    if ordinance_enabled_for_city(city, ORDINANCE_PARKING_FINES) {
        total = scale_u32_percent(total, 85);
    }
    total
}

/// City pollution counter for MISC / scenarios.
/// Prefers the live `XPLT` microsim sum (day-2 density scan) over the tile heuristic.
pub(super) fn live_pollution_count_for_city(city: &City, population: PopulationStats) -> u32 {
    if let Some(values) = city.pollution_density_slice() {
        return density_map_microsim64_sum(values);
    }

    let mut total = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = city.building_at(x, y);
            let mut tile_pollution = pollution_weight_for_building(building);

            if let Some(kind) = rci_building_kind(building) {
                let capacity = rci_tile_capacity(building, kind);
                tile_pollution = tile_pollution.saturating_add(match kind {
                    RciKind::Residential => capacity / 8,
                    RciKind::Commercial => capacity / 2,
                    RciKind::Industrial => capacity.saturating_mul(5),
                });
            }

            if is_road_tile(building) || is_highway_tile(building) {
                let base = if is_highway_tile(building) { 4 } else { 2 };
                let density = city
                    .traffic_density_at(x, y)
                    .map(u32::from)
                    .unwrap_or(base * 8);
                tile_pollution = tile_pollution.saturating_add(base + density / 32);
            }

            total = total.saturating_add(tile_pollution);
        }
    }

    total = total.saturating_add(population.work_force / 40);
    if ordinance_enabled_for_city(city, ORDINANCE_POLLUTION_CONTROLS) {
        total = scale_u32_percent(total, 75);
    }
    if ordinance_enabled_for_city(city, ORDINANCE_ENERGY_CONSERVATION) {
        total = scale_u32_percent(total, 90);
    }
    if ordinance_enabled_for_city(city, ORDINANCE_PUBLIC_SMOKING_BAN) {
        total = scale_u32_percent(total, 97);
    }
    total
}

// Scenario record parsing and goal evaluation are kept together below.
pub(super) fn loaded_city_info_dialog_for_city(city: &City) -> Option<CityInfoDialog> {
    (city.kind == CityFileKind::WinScenario).then_some(CityInfoDialog::Scenario)
}

pub(super) fn scenario_text_for_city(city: &City) -> ScenarioText {
    let mut text = ScenarioText::default();
    for chunk in &city.chunks {
        if chunk.name != "TEXT" || chunk.data.len() < 4 {
            continue;
        }
        let Some(decoded) = decode_scenario_text_payload(&chunk.data[4..]) else {
            continue;
        };
        match chunk.data[0] {
            0x80 => text.selection = Some(decoded),
            0x81 => text.extended = Some(decoded),
            _ => {}
        }
    }
    text
}

pub(super) fn decode_scenario_text_payload(data: &[u8]) -> Option<String> {
    let raw = String::from_utf8_lossy(data);
    let text = raw
        .split('\0')
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

pub(super) fn scenario_goals_for_city(city: &City) -> Option<ScenarioGoals> {
    let data = city.scen_bytes()?;
    if data.len() < 52 {
        return None;
    }
    Some(ScenarioGoals {
        disaster_type: read_be_u16(&data, 0x04)?,
        disaster_x: *data.get(0x06)?,
        disaster_y: *data.get(0x07)?,
        time_limit_months: read_be_u16(&data, 0x08)?,
        city_population_goal: read_be_u32(&data, 0x0a)?,
        residential_population_goal: read_be_u32(&data, 0x0e)?,
        commercial_population_goal: read_be_u32(&data, 0x12)?,
        industrial_population_goal: read_be_u32(&data, 0x16)?,
        fund_goal: read_be_u32(&data, 0x1a)?,
        land_value_goal: read_be_u32(&data, 0x1e)?,
        // PE RAM: `wScenarioLEGoal` @ `0x4CC936`, `wScenarioEQGoal` @ `0x4CC938`
        // (two WORDs after land-value, before pollution DWORD).
        life_expectancy_goal: u32::from(read_be_u16(&data, 0x22)?),
        education_goal: u32::from(read_be_u16(&data, 0x24)?),
        pollution_limit: read_be_u32(&data, 0x26)?,
        crime_limit: read_be_u32(&data, 0x2a)?,
        traffic_limit: read_be_u32(&data, 0x2e)?,
        build_item_1: *data.get(0x32)?,
        build_item_2: *data.get(0x33)?,
        build_item_1_tiles: (data.len() >= 54)
            .then(|| read_be_u16(&data, 0x34))
            .flatten(),
        build_item_2_tiles: (data.len() >= 56)
            .then(|| read_be_u16(&data, 0x36))
            .flatten(),
    })
}

pub(super) fn game_clock_from_city(city: &City) -> GameClock {
    match (city.saved_date(), city.city_age_days()) {
        (Some(date), Some(age_days)) => GameClock::new_with_day(
            date.year,
            date.month,
            (age_days % u32::from(DAYS_PER_MONTH)) as u8,
        ),
        (Some(date), None) => GameClock::new(date.year, date.month),
        (None, Some(age_days)) => {
            let founded = city
                .year_founded()
                .and_then(|year| u16::try_from(year).ok())
                .unwrap_or(1900);
            GameClock::from_city_age(founded, age_days)
        }
        (None, None) => GameClock::default_start(),
    }
}

pub(super) fn scenario_start_clock_for_city(city: &City, fallback: GameClock) -> Option<GameClock> {
    (city.kind == CityFileKind::WinScenario).then(|| {
        city.saved_date()
            .map(|_| game_clock_from_city(city))
            .unwrap_or(fallback)
    })
}

pub(super) fn read_be_u16(data: &[u8], offset: usize) -> Option<u16> {
    data.get(offset..offset + 2)
        .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
}

pub(super) fn read_be_u32(data: &[u8], offset: usize) -> Option<u32> {
    data.get(offset..offset + 4)
        .map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub(super) fn scenario_goal_lines(
    city: &City,
    goals: ScenarioGoals,
    start_clock: GameClock,
    current_clock: GameClock,
) -> Vec<String> {
    scenario_goal_lines_with_entry_seed(
        city,
        goals,
        start_clock,
        current_clock,
        pe_power_recalc_seed(city),
    )
}

pub(super) fn scenario_goal_lines_with_entry_seed(
    city: &City,
    goals: ScenarioGoals,
    start_clock: GameClock,
    current_clock: GameClock,
    entry_seed: u32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let progress = scenario_progress_for_city_with_entry_seed(city, entry_seed);
    let population = progress.population;
    let outcomes = progress.outcomes;
    let counters = progress.counters;
    if goals.time_limit_months > 0 {
        lines.push(scenario_deadline_line(start_clock, goals.time_limit_months));
    }
    if let Some(status) = scenario_status_line(city, goals, progress, start_clock, current_clock) {
        lines.push(status);
    }
    if let Some(summary) = scenario_known_goal_summary_line(city, goals, progress) {
        lines.push(summary);
    }
    if goals.disaster_type != 0 {
        lines.push(format!(
            "Disaster: {} at ({}, {})",
            scenario_disaster_label(goals.disaster_type),
            goals.disaster_x,
            goals.disaster_y
        ));
    }
    if goals.city_population_goal > 0 {
        // PE / sc2kfix: `dwScenarioCitySize > dwCityPopulation` fails the goal.
        // `dwCityPopulation` is `0x4CAA74` (`pe_dw_city_population`), not census.
        lines.push(format_goal_progress(
            "City population",
            city.pe_dw_city_population(),
            goals.city_population_goal,
        ));
    }
    let [res_costs, com_costs, ind_costs] =
        budget_tax_fund_current_costs_for_city(city, population);
    for (label, current, goal) in [
        (
            "Residential population",
            res_costs,
            goals.residential_population_goal,
        ),
        (
            "Commercial population",
            com_costs,
            goals.commercial_population_goal,
        ),
        (
            "Industrial population",
            ind_costs,
            goals.industrial_population_goal,
        ),
    ] {
        if goal > 0 {
            lines.push(format_goal_progress(label, current, goal));
        }
    }
    if goals.fund_goal > 0 {
        // PE / sc2kfix: fail while `dwCityFunds - dwCityBonds < dwScenarioCashGoal`.
        lines.push(format_money_goal_progress(
            "Funds",
            scenario_net_city_funds(city),
            goals.fund_goal,
        ));
    }
    if goals.land_value_goal > 0 {
        // PE / sc2kfix: fail while `dwCityLandValue < dwScenarioLandValueGoal`.
        lines.push(format_goal_progress(
            "Land value",
            scenario_city_land_value(city, outcomes),
            goals.land_value_goal,
        ));
    }
    let (workforce_le, workforce_eq) = scenario_workforce_le_eq(city, outcomes);
    if goals.life_expectancy_goal > 0 {
        // PE: fail while `wScenarioLEGoal > dwCityWorkforceLE` (`0x4CB410`).
        lines.push(format_goal_progress(
            "Life expectancy",
            workforce_le,
            goals.life_expectancy_goal,
        ));
    }
    if goals.education_goal > 0 {
        // PE: fail while `wScenarioEQGoal > dwCityWorkforceEQ` (`0x4CC4B4`).
        lines.push(format_goal_progress(
            "Education",
            workforce_eq,
            goals.education_goal,
        ));
    }
    if (goals.pollution_limit as i32) > 0 {
        lines.push(format_scaled_limit_progress(
            "Pollution below",
            Some(counters.pollution),
            goals.pollution_limit,
            scenario_environment_limit_divisor(population),
        ));
    }
    if (goals.crime_limit as i32) > 0 {
        lines.push(format_scaled_limit_progress(
            "Crime below",
            Some(counters.crime),
            goals.crime_limit,
            scenario_environment_limit_divisor(population),
        ));
    }
    if (goals.traffic_limit as i32) > 0 {
        lines.push(format_limit_progress(
            "Traffic below",
            Some(counters.traffic),
            goals.traffic_limit,
        ));
    }
    push_scenario_build_goal(
        city,
        &mut lines,
        goals.build_item_1,
        goals.build_item_1_tiles,
    );
    push_scenario_build_goal(
        city,
        &mut lines,
        goals.build_item_2,
        goals.build_item_2_tiles,
    );
    if lines.is_empty() {
        lines.push("No explicit win conditions found.".to_owned());
    }
    lines
}

pub(super) fn scenario_known_goal_summary_line(
    city: &City,
    goals: ScenarioGoals,
    progress: ScenarioProgress,
) -> Option<String> {
    let counts = scenario_goal_counts(city, goals, progress);

    if counts.known == 0 && counts.pending == 0 {
        return None;
    }
    let mut line = format!(
        "Known goals: {} / {} complete",
        counts.complete, counts.known
    );
    if counts.pending > 0 {
        line.push_str(&format!("; {} pending", counts.pending));
    }
    Some(line)
}

pub(super) fn scenario_status_line(
    city: &City,
    goals: ScenarioGoals,
    progress: ScenarioProgress,
    start_clock: GameClock,
    current_clock: GameClock,
) -> Option<String> {
    let counts = scenario_goal_counts(city, goals, progress);
    match scenario_outcome_for_counts_with_city(
        Some(city),
        goals,
        counts,
        start_clock,
        current_clock,
    )? {
        ScenarioOutcome::Complete => Some("Status: Complete".to_owned()),
        ScenarioOutcome::Failed => Some("Status: Failed - deadline passed".to_owned()),
        ScenarioOutcome::InProgress => {
            if goals.time_limit_months == 0 {
                return None;
            }
            // PE UI uses the live `wScenarioTimeLimitMonths` countdown word.
            let months = u32::from(goals.time_limit_months);
            let unit = if months == 1 { "month" } else { "months" };
            Some(format!(
                "Status: In progress - {} {unit} left",
                format_number(months)
            ))
        }
    }
}

pub(super) fn scenario_outcome_for_counts(
    goals: ScenarioGoals,
    counts: ScenarioGoalCounts,
    start_clock: GameClock,
    current_clock: GameClock,
) -> Option<ScenarioOutcome> {
    scenario_outcome_for_counts_with_city(None, goals, counts, start_clock, current_clock)
}

pub(super) fn scenario_outcome_for_progress(
    city: &City,
    goals: ScenarioGoals,
    progress: ScenarioProgress,
    start_clock: GameClock,
    current_clock: GameClock,
) -> Option<ScenarioOutcome> {
    let counts = scenario_goal_counts(city, goals, progress);
    scenario_outcome_for_counts_with_city(Some(city), goals, counts, start_clock, current_clock)
}

pub(super) fn scenario_outcome_for_counts_with_city(
    city: Option<&City>,
    goals: ScenarioGoals,
    counts: ScenarioGoalCounts,
    start_clock: GameClock,
    current_clock: GameClock,
) -> Option<ScenarioOutcome> {
    if counts.known == 0 && counts.pending == 0 && !counts.implicit_minimum_failed {
        // The PE day-22 evaluator starts with its shared success flag set.
        // Even with no displayed goals, its unconditional signed R/C/I and
        // cash comparisons must all pass before this no-goals win is valid.
        return city
            .filter(|city| city.kind == CityFileKind::WinScenario)
            .map(|_| ScenarioOutcome::Complete);
    }
    if counts.pending == 0
        && counts.known > 0
        && counts.complete == counts.known
        && !counts.implicit_minimum_failed
    {
        return Some(ScenarioOutcome::Complete);
    }
    if city.is_some_and(|city| city.pe_scenario_deadline_expired()) {
        return Some(ScenarioOutcome::Failed);
    }
    // An active PE scenario's SCEN word is already the remaining time. Its
    // day-22 worker decrements it and records expiry; subtracting elapsed
    // calendar months again would fail the scenario before the deadline.
    if city.is_some_and(|city| {
        city.kind == CityFileKind::WinScenario
            && city.pe_scenario_active()
            && city.scenario_time_limit_months().is_some()
    }) {
        return Some(ScenarioOutcome::InProgress);
    }
    // Legacy / constructed-goals path: clock budget when SCEN has not been ticked.
    if scenario_deadline_passed(start_clock, current_clock, goals.time_limit_months) {
        return Some(ScenarioOutcome::Failed);
    }
    Some(ScenarioOutcome::InProgress)
}

pub(super) fn scenario_outcome_message(outcome: ScenarioOutcome) -> &'static str {
    match outcome {
        ScenarioOutcome::InProgress => "Scenario In Progress",
        ScenarioOutcome::Complete => "Scenario Complete",
        ScenarioOutcome::Failed => "Scenario Failed - deadline passed",
    }
}

pub(super) fn scenario_workforce_le_eq(city: &City, _outcomes: CityServiceOutcomes) -> (u32, u32) {
    // PE `dwCityWorkforceLE` / `dwCityWorkforceEQ` BSS `0x4cb410` / `0x4cc4b4`.
    (city.pe_eq_life_average(), city.pe_eq_education_average())
}

/// PE `dwCityLandValue` (`0x4CA440`), mirrored at MISC `0x28`.
pub(super) fn scenario_city_land_value(city: &City, _outcomes: CityServiceOutcomes) -> u32 {
    city.pe_dw_city_land_value
}

pub(super) fn scenario_goal_counts(
    city: &City,
    goals: ScenarioGoals,
    progress: ScenarioProgress,
) -> ScenarioGoalCounts {
    let population = progress.population;
    let outcomes = progress.outcomes;
    let counters = progress.counters;
    let mut counts = ScenarioGoalCounts::default();

    if goals.city_population_goal > 0 {
        // PE / sc2kfix: fail while `dwScenarioCitySize > dwCityPopulation`
        // (`0x4CAA74`). Census totals are not this word.
        counts.add_known(city.pe_dw_city_population() >= goals.city_population_goal);
    }
    // PE / sc2kfix: fail while `pBudgetArr[BUDGET_*FUND].iCurrentCosts < goal`.
    let [res_costs, com_costs, ind_costs] =
        budget_tax_fund_current_costs_for_city(city, population);
    for (current, goal) in [
        (res_costs, goals.residential_population_goal),
        (com_costs, goals.commercial_population_goal),
        (ind_costs, goals.industrial_population_goal),
    ] {
        // PE uses signed JGE for each DWORD current/goal pair, including a
        // zero goal. Keep implicit zero-goal failures out of the UI's count of
        // explicitly configured objectives, but let them block completion.
        let met = (current as i32) >= (goal as i32);
        if goal > 0 {
            counts.add_known(met);
        } else if !met {
            counts.implicit_minimum_failed = true;
        }
    }
    // PE's signed `JGE` compares the wrapped net-funds DWORD to the raw goal
    // DWORD. It runs even when SCEN's cash goal is zero.
    let net_funds = scenario_net_city_funds(city);
    let cash_goal_met = net_funds >= goals.fund_goal as i32;
    if goals.fund_goal > 0 {
        counts.add_known(cash_goal_met);
    } else if !cash_goal_met {
        counts.implicit_minimum_failed = true;
    }
    if goals.land_value_goal > 0 {
        // PE / sc2kfix: fail while `dwCityLandValue < dwScenarioLandValueGoal`.
        counts.add_known(scenario_city_land_value(city, outcomes) >= goals.land_value_goal);
    }
    let (workforce_le, workforce_eq) = scenario_workforce_le_eq(city, outcomes);
    if goals.life_expectancy_goal > 0 {
        // PE / sc2kfix: fail while `wScenarioLEGoal > dwCityWorkforceLE`.
        counts.add_known(workforce_le >= (goals.life_expectancy_goal as u16 as i16 as i32 as u32));
    }
    if goals.education_goal > 0 {
        // PE / sc2kfix: fail while `wScenarioEQGoal > dwCityWorkforceEQ`.
        counts.add_known(workforce_eq >= (goals.education_goal as u16 as i16 as i32 as u32));
    }
    if (goals.pollution_limit as i32) > 0 {
        // PE: fail while `dwScenarioPollutionLimit > 0 && dwCityPollution > limit`.
        counts.add_known(counters.pollution <= goals.pollution_limit);
    }
    if (goals.crime_limit as i32) > 0 {
        // PE: fail while `dwScenarioCrimeLimit > 0 && dwCityCrime > limit`.
        counts.add_known(counters.crime <= goals.crime_limit);
    }
    if (goals.traffic_limit as i32) > 0 {
        // PE: fail while `dwScenarioTrafficLimit > 0 && dwCityTrafficUnknown > limit`.
        counts.add_known(counters.traffic <= goals.traffic_limit);
    }
    add_scenario_build_goal_summary(
        city,
        goals.build_item_1,
        goals.build_item_1_tiles,
        &mut counts,
    );
    add_scenario_build_goal_summary(
        city,
        goals.build_item_2,
        goals.build_item_2_tiles,
        &mut counts,
    );
    counts
}

pub(super) fn scenario_progress_for_city(city: &City) -> ScenarioProgress {
    scenario_progress_for_city_with_entry_seed(city, pe_power_recalc_seed(city))
}

pub(super) fn scenario_progress_for_city_with_entry_seed(
    city: &City,
    entry_seed: u32,
) -> ScenarioProgress {
    let population = population_stats_for_city(city);
    let power_maps = power_service_maps_with_entry_seed(city, entry_seed);
    let outcomes =
        city_service_outcomes_for_population_with_power(city, population, true, &power_maps);
    let counters = if city.kind == CityFileKind::WinScenario {
        // PE day 22 reads published BSS totals, not a fresh map scan. They can
        // legitimately be zero or lag a map changed after its publishing phase.
        CityScenarioCounters {
            crime: city.pe_dw_city_crime,
            traffic: city.pe_traffic_count(),
            pollution: city.pe_dw_city_pollution,
        }
    } else {
        let live_population = live_population_stats_for_city(city);
        let live_outcomes = live_city_service_outcomes_for_city_with_power(city, &power_maps);
        let live_counters = live_city_scenario_counters_for_city_with_power(
            city,
            live_population,
            live_outcomes,
            &power_maps,
        );
        CityScenarioCounters {
            // Retain the existing live-summary policy for non-scenario callers.
            crime: if city.crime_density_slice().is_some() {
                live_counters.crime
            } else {
                live_counter_or_saved_sum(live_counters.crime, city.misc_crime_count())
            },
            traffic: if city.has_traffic_density_map() {
                live_counters.traffic
            } else {
                live_counter_or_saved_sum(live_counters.traffic, city.misc_traffic_count())
            },
            pollution: if city.pollution_density_slice().is_some() {
                live_counters.pollution
            } else {
                live_counter_or_saved_sum(live_counters.pollution, city.misc_pollution_count())
            },
        }
    };

    ScenarioProgress {
        population,
        outcomes,
        counters,
    }
}

pub(super) fn add_scenario_build_goal_summary(
    city: &City,
    item: u8,
    tiles: Option<u16>,
    counts: &mut ScenarioGoalCounts,
) {
    if item == 0 {
        return;
    }
    let Some(tiles) = tiles else {
        counts.pending = counts.pending.saturating_add(1);
        return;
    };
    // PE / sc2kfix: fail while `dwTileCount[bScenarioBuildingGoal] < wScenarioBuildingGoalCount`
    // (`0x4CA4C8` WORD table). Counts every XBLD cell with that id (full multi-tile footprint).
    counts.add_known((city.pe_dw_tile_count[item as usize] as i16) >= (tiles as i16));
}

pub(super) fn scenario_deadline_line(clock: GameClock, time_limit_months: u16) -> String {
    const MONTH_NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let current_month_index = u32::from(clock.month().saturating_sub(1));
    let due_month_index = current_month_index.saturating_add(u32::from(time_limit_months));
    let due_year = u32::from(clock.year()).saturating_add(due_month_index / 12);
    let due_month = MONTH_NAMES[(due_month_index % 12) as usize];
    format!(
        "Deadline: {due_month} 23, {due_year} ({} months)",
        format_number(u32::from(time_limit_months))
    )
}

pub(super) fn scenario_deadline_passed(
    start_clock: GameClock,
    current_clock: GameClock,
    time_limit_months: u16,
) -> bool {
    if time_limit_months == 0 {
        return false;
    }
    scenario_clock_month_index(current_clock)
        > scenario_clock_month_index(start_clock).saturating_add(u32::from(time_limit_months))
}

pub(super) fn scenario_months_remaining(
    start_clock: GameClock,
    current_clock: GameClock,
    time_limit_months: u16,
) -> u32 {
    scenario_clock_month_index(start_clock)
        .saturating_add(u32::from(time_limit_months))
        .saturating_sub(scenario_clock_month_index(current_clock))
}

pub(super) fn scenario_clock_month_index(clock: GameClock) -> u32 {
    u32::from(clock.year()).saturating_mul(12) + u32::from(clock.month().saturating_sub(1))
}

pub(super) fn format_goal_progress(label: &str, current: u32, goal: u32) -> String {
    format!(
        "{label}: {} / {}",
        format_number(current),
        format_number(goal)
    )
}

pub(super) fn format_goal_target(label: &str, goal: u32) -> String {
    format!("{label}: {}", format_number(goal))
}

pub(super) fn format_money_goal_progress(label: &str, current: i32, goal: u32) -> String {
    let current = format!("${}", format_signed_number(current));
    format!("{label}: {current} / ${}", format_number(goal))
}

pub(super) fn format_limit_progress(label: &str, current: Option<u32>, limit: u32) -> String {
    if let Some(current) = current {
        format!(
            "{label}: {} / {}",
            format_number(current),
            format_number(limit)
        )
    } else {
        format!("{label}: {}", format_number(limit))
    }
}

pub(super) fn format_scaled_limit_progress(
    label: &str,
    current: Option<u32>,
    limit: u32,
    divisor: u32,
) -> String {
    let limit = scenario_scaled_limit_percent(limit, divisor);
    if let Some(current) = current {
        let current = scenario_scaled_limit_percent(current, divisor);
        format!("{label}: {current}% / {limit}%")
    } else {
        format!("{label}: {limit}%")
    }
}

pub(super) fn scenario_environment_limit_divisor(population: PopulationStats) -> u32 {
    population.populated_tiles / 4 + 1
}

pub(super) fn scenario_scaled_limit_percent(value: u32, divisor: u32) -> u32 {
    let divisor = u64::from(divisor.max(1));
    let value = u64::from(value);
    value
        .saturating_add(divisor / 2)
        .checked_div(divisor)
        .unwrap_or(0)
        .min(u64::from(u32::MAX)) as u32
}

pub(super) fn push_scenario_build_goal(
    city: &City,
    lines: &mut Vec<String>,
    item: u8,
    tiles: Option<u16>,
) {
    if item == 0 {
        return;
    }
    let label = surface_building_query_label(item).unwrap_or("Building");
    let current_tiles = u32::from(city.pe_dw_tile_count[item as usize]);
    if let Some(tiles) = tiles.filter(|tiles| *tiles > 0) {
        lines.push(format!(
            "Build {label}: {} / {tiles} tiles",
            format_number(current_tiles)
        ));
    } else {
        lines.push(format!(
            "Build {label}: {} tiles",
            format_number(current_tiles)
        ));
    }
}

pub(super) fn scenario_disaster_kind(disaster_type: u16) -> Option<DisasterKind> {
    match disaster_type {
        0x01 => Some(DisasterKind::Fire),
        0x0c => Some(DisasterKind::Firestorm),
        0x02 => Some(DisasterKind::Flood),
        0x0e => Some(DisasterKind::BlizzardFlood),
        0x03 => Some(DisasterKind::Rioters),
        0x0d => Some(DisasterKind::MassRiot),
        0x05 => Some(DisasterKind::NopAirCrash),
        0x11 => Some(DisasterKind::HelicopterCrash),
        0x12 => Some(DisasterKind::AirCrash),
        0x06 => Some(DisasterKind::Earthquake),
        0x07 => Some(DisasterKind::Tornado),
        0x08 => Some(DisasterKind::Monster),
        0x04 => Some(DisasterKind::ToxicSpill),
        0x0f => Some(DisasterKind::PollutionAccident),
        0x09 => Some(DisasterKind::Meltdown),
        0x0a => Some(DisasterKind::Microwave),
        0x0b => Some(DisasterKind::Volcano),
        0x10 => Some(DisasterKind::Hurricane),
        _ => None,
    }
}

pub(super) fn scenario_disaster_label(disaster_type: u16) -> String {
    match disaster_type {
        0x01 => "Fire".to_owned(),
        0x02 => "Flood".to_owned(),
        0x03 => "Riot".to_owned(),
        0x04 => "Toxic Spill".to_owned(),
        0x05 => "Air Crash".to_owned(),
        0x06 => "Earthquake".to_owned(),
        0x07 => "Tornado".to_owned(),
        0x08 => "Monster".to_owned(),
        0x09 => "Meltdown".to_owned(),
        0x0a => "Microwave".to_owned(),
        0x0b => "Volcano".to_owned(),
        0x0c => "Firestorm".to_owned(),
        0x0d => "Mass Riots".to_owned(),
        0x0e => "Mass Floods".to_owned(),
        0x0f => "Pollution Accident".to_owned(),
        0x10 => "Hurricane".to_owned(),
        0x11 => "Helicopter Crash".to_owned(),
        0x12 => "Plane Crash".to_owned(),
        other => format!("Disaster {other}"),
    }
}
