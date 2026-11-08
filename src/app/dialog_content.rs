//! Dialog models and the data/text builders used by their views.

use super::*;
use crate::city::BLANK_MAP_BASE_HEIGHT;
use crate::ui::UiAction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SignDialogState {
    pub(super) sign_id: u8,
    pub(super) text: String,
    pub(super) original_text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SignDialogAction {
    Commit,
    Cancel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct MilitaryBaseDialogState {
    pub(super) pressed: Option<MilitaryBaseDialogAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MilitaryBaseDialogAction {
    Accept,
    Decline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NewspaperEdition {
    Founding,
    Subscription,
    Extra,
    Courier,
}

impl NewspaperEdition {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Founding => "Founding Edition",
            Self::Subscription => "Subscription",
            Self::Extra => "Extra!!!",
            Self::Courier => "Courier",
        }
    }

    pub(super) fn masthead(self, city_name: &str, newspaper_choice: u32) -> String {
        match self {
            Self::Founding => format!("{city_name} Herald"),
            Self::Subscription => format!("{city_name} Newspaper Subscription"),
            Self::Extra => format!("{city_name} Extra!!!"),
            Self::Courier => format!("The {city_name} {}", newspaper_paper_name(newspaper_choice)),
        }
    }

    pub(super) fn headline(
        self,
        services: &ServiceStatus,
        active_disaster_tiles: usize,
        available_rewards: u32,
    ) -> String {
        match self {
            Self::Founding => "Groundwork Laid for a New City".to_owned(),
            Self::Subscription => {
                "Subscription active. City hall news delivery is available.".to_owned()
            }
            Self::Extra if active_disaster_tiles > 0 => "Extra: Disaster Reported".to_owned(),
            Self::Extra if available_rewards > 0 => {
                format!("Extra: {}", reward_available_status_line(available_rewards))
            }
            Self::Extra if services.has_warnings() => {
                format!("Extra: {}", services.warning_parts().join(", "))
            }
            Self::Extra => "Extra: no urgent city service warnings".to_owned(),
            Self::Courier if active_disaster_tiles > 0 => "Disaster Reported".to_owned(),
            Self::Courier if available_rewards > 0 => {
                reward_available_status_line(available_rewards).to_owned()
            }
            Self::Courier if services.has_warnings() => services.warning_parts().join(", "),
            Self::Courier => "City services report no active warnings".to_owned(),
        }
    }

    pub(super) fn story(
        self,
        city_name: &str,
        population: &PopulationStats,
        active_disaster_tiles: usize,
        available_rewards: u32,
    ) -> String {
        match self {
            Self::Founding => format!(
                "A new chapter begins for {city_name}. City hall has opened its doors, and the first neighborhoods are ready to take shape. The mayor's first priorities are power, road access, and places for residents to live and work. Local planners look forward to watching this landscape grow into a thriving city."
            ),
            Self::Subscription => format!(
                "{} residents receive regular reports from {} on city services, budgets, and local growth.",
                format_number(population.total_population),
                city_name
            ),
            Self::Extra if active_disaster_tiles > 0 => format!(
                "Emergency crews are tracking {} active disaster {} in {} while city hall coordinates response.",
                format_number(active_disaster_tiles as u32),
                if active_disaster_tiles == 1 {
                    "tile"
                } else {
                    "tiles"
                },
                city_name
            ),
            Self::Extra if available_rewards > 0 => format!(
                "{} has {} available mayoral {} waiting in the rewards menu.",
                city_name,
                format_number(available_rewards.count_ones()),
                if available_rewards.count_ones() == 1 {
                    "reward"
                } else {
                    "rewards"
                },
            ),
            Self::Extra => format!(
                "{} residents watch the latest bulletin as {} responds to current conditions.",
                format_number(population.total_population),
                city_name
            ),
            Self::Courier if active_disaster_tiles > 0 => format!(
                "{} residents monitor {} as emergency crews track {} active disaster {}.",
                format_number(population.total_population),
                city_name,
                format_number(active_disaster_tiles as u32),
                if active_disaster_tiles == 1 {
                    "tile"
                } else {
                    "tiles"
                },
            ),
            Self::Courier if available_rewards > 0 => format!(
                "{} residents hear that city hall has {} available {} for the mayor to place.",
                format_number(population.total_population),
                format_number(available_rewards.count_ones()),
                if available_rewards.count_ones() == 1 {
                    "reward"
                } else {
                    "rewards"
                },
            ),
            Self::Courier => format!(
                "{} residents continue to shape {} as city hall balances growth, services, and the monthly budget.",
                format_number(population.total_population),
                city_name
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CityInfoDialog {
    Map,
    Population,
    Neighbors,
    Graphs,
    Industry,
    Budget,
    /// PE January MessageBox: holds its caller without a temporary speed pause.
    FiscalCrisis,
    Ordinances,
    Newspaper(NewspaperEdition),
    Scenario,
    Help,
    /// Query-tool tile inspect (ClubOpolis QueryGeneral + QuerySpecific fields).
    Query {
        x: usize,
        y: usize,
    },
    /// Win95 bridge chooser after a valid road-water span has been preflighted.
    RoadBridge {
        seed_x: usize,
        seed_y: usize,
        direction: u8,
        span_count: usize,
        selector_mask: u8,
    },
    /// Win95 Rail `FUN_0045B930` selector-1 water span.
    RailBridge {
        seed_x: usize,
        seed_y: usize,
        direction: u8,
        span_count: usize,
    },
    /// Win95 Power `FUN_0045B930` selector-0 water span.
    PowerBridge {
        seed_x: usize,
        seed_y: usize,
        direction: u8,
        span_count: usize,
    },
    /// Win95 Highway `FUN_0045C1E0` water-worker selector dialog.
    HighwayWaterBridge {
        candidate: PeHighwayWaterBridgeCandidate,
    },
}

impl CityInfoDialog {
    pub(super) fn for_action(action: UiAction) -> Option<Self> {
        match action {
            UiAction::OpenCityMap => Some(Self::Map),
            UiAction::OpenCityPopulation => Some(Self::Population),
            UiAction::OpenCityNeighbors => Some(Self::Neighbors),
            UiAction::OpenCityGraphs => Some(Self::Graphs),
            UiAction::OpenCityIndustry => Some(Self::Industry),
            UiAction::OpenBudget => Some(Self::Budget),
            UiAction::OpenHelp => Some(Self::Help),
            _ => None,
        }
    }

    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Map => "City Map",
            Self::Population => "Population",
            Self::Neighbors => "Neighbors",
            Self::Graphs => "City Graphs",
            Self::Industry => "Industry",
            Self::Budget => "Budget",
            Self::FiscalCrisis => "Fiscal Crisis",
            Self::Ordinances => "Ordinances",
            Self::Newspaper(edition) => edition.label(),
            Self::Scenario => "Scenario Briefing",
            Self::Help => "CityGame Help",
            Self::Query { .. } => "Query",
            Self::RoadBridge { .. } => "Select Road Bridge",
            Self::RailBridge { .. } => "Select Rail Bridge",
            Self::PowerBridge { .. } => "Select Power Bridge",
            Self::HighwayWaterBridge { .. } => "Select Highway Bridge",
        }
    }

    pub(super) fn toolbar_action(self) -> Option<UiAction> {
        match self {
            Self::Map => Some(UiAction::OpenCityMap),
            Self::Population => Some(UiAction::OpenCityPopulation),
            Self::Neighbors => Some(UiAction::OpenCityNeighbors),
            Self::Graphs => Some(UiAction::OpenCityGraphs),
            Self::Industry => Some(UiAction::OpenCityIndustry),
            Self::Budget => Some(UiAction::OpenBudget),
            Self::FiscalCrisis => None,
            Self::Ordinances => None,
            Self::Newspaper(_) => None,
            Self::Scenario => None,
            Self::Help => Some(UiAction::OpenHelp),
            Self::Query { .. } => None,
            Self::RoadBridge { .. } => None,
            Self::RailBridge { .. } => None,
            Self::PowerBridge { .. } => None,
            Self::HighwayWaterBridge { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CityInfoDialogAction {
    Close,
    CenterMapOn { x: usize, y: usize },
    ToggleAutoBudget,
    ToggleGraphSeries(usize),
    SetGraphScale(GraphScale),
    ToggleOrdinance(usize),
    AdjustIndustryTax { industry_index: usize, delta: i32 },
    AdjustBudgetTax { tax_index: usize, delta: i32 },
    AdjustBudgetFunding { service_index: usize, delta: i32 },
    IssueBond,
    RepayBond,
    SelectRoadBridge(u8),
    SelectRailBridge,
    SelectPowerBridge,
    SelectHighwayWaterBridge(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PowerPlantDialogState {
    pub(super) info_tool: Option<EditTool>,
    pub(super) pressed_tool: Option<EditTool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PowerPlantDialogAction {
    Cancel,
    Select(EditTool),
    Info(EditTool),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PowerPlantChoice {
    pub(super) tool: EditTool,
    pub(super) building: u8,
    pub(super) megawatts: u16,
    pub(super) cost: u32,
    pub(super) label: &'static str,
    pub(super) info: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ArcologyDialogState {
    pub(super) info_tool: Option<EditTool>,
    pub(super) pressed_tool: Option<EditTool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArcologyDialogAction {
    Cancel,
    Select(EditTool),
    Info(EditTool),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ArcologyChoice {
    pub(super) tool: EditTool,
    pub(super) building: u8,
    pub(super) capacity: u32,
    pub(super) cost: u32,
    pub(super) label: &'static str,
    pub(super) info: &'static str,
}

pub(super) fn graph_panel_xgrp_series(city: &City, scale: GraphScale) -> Vec<Vec<u32>> {
    let values = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    XGRP_GRAPH_LEGEND_SERIES
        .into_iter()
        .map(|series_index| xgrp_samples_oldest_first(&values, series_index, scale))
        .collect()
}

pub(super) fn query_dialog_field_pair(part: &str) -> (String, String) {
    if let Some((label, value)) = part.split_once(": ") {
        return (label.trim().to_string(), value.trim().to_string());
    }
    let mut pieces = part.splitn(2, ' ');
    let label = pieces.next().unwrap_or("").to_string();
    let value = pieces.next().unwrap_or("").to_string();
    (label, value)
}

/// Left = QueryGeneral fields; right = transit / microsim QuerySpecific fields.
pub(super) fn query_dialog_row_groups(
    city: &City,
    x: usize,
    y: usize,
    prefer_underground: bool,
    passengers: TransitPassengerStats,
    entry_seed: u32,
) -> (Vec<(String, String)>, Vec<(String, String)>) {
    let _ = prefer_underground;
    let land = city.land_value_at(x, y).unwrap_or(0);
    let crime = city.crime_density_at(x, y).unwrap_or(0);
    let pollution = city.pollution_density_at(x, y).unwrap_or(0);
    let traffic = city.traffic_density_at(x, y).unwrap_or(0);
    let feet = query_altitude_feet(city, x, y);
    let water_level = i32::from(
        city.misc_water_table_level()
            .unwrap_or(u32::from(BLANK_MAP_BASE_HEIGHT)) as u8,
    );
    let under_water = city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
        && i32::from(city.altitude_at(x, y)) < water_level;
    let altitude_value = if under_water {
        format!("{} feet deep", feet.max(0) as u32)
    } else {
        format!("{feet} feet")
    };

    let general = vec![
        ("Location".to_string(), format!("({x},{y})")),
        ("Altitude".to_string(), altitude_value),
        (
            "Land Value".to_string(),
            format!("${},000/acre", u32::from(land).saturating_add(1)),
        ),
        ("Crime".to_string(), query_metric_band(crime).to_string()),
        (
            "Pollution".to_string(),
            query_metric_band(pollution).to_string(),
        ),
        (
            "Traffic".to_string(),
            query_metric_band(traffic).to_string(),
        ),
        (
            "Power".to_string(),
            query_service_yes_no(tile_power_status_text_with_entry_seed(
                city, x, y, entry_seed,
            ))
            .to_string(),
        ),
        (
            "Water".to_string(),
            query_service_yes_no(tile_water_status_text_with_entry_seed(
                city, x, y, entry_seed,
            ))
            .to_string(),
        ),
    ];

    let mut specific = Vec::new();
    if let Some(extra) = microsim_query_extra(city, x, y) {
        for part in extra.split("  ").filter(|part| !part.is_empty()) {
            specific.push(query_dialog_field_pair(part));
        }
    } else if let Some(extra) = transit_query_extra(city, city.building_at(x, y), passengers) {
        for part in extra.split("  ").filter(|part| !part.is_empty()) {
            specific.push(query_dialog_field_pair(part));
        }
    }
    (general, specific)
}

pub(super) fn population_panel_transit_rows(
    passengers: TransitPassengerStats,
) -> [(&'static str, u32); 3] {
    [
        ("Bus Passengers", passengers.bus),
        ("Rail Passengers", passengers.rail),
        ("Subway Passengers", passengers.subway),
    ]
}

pub(super) const POWER_PLANT_DIALOG_W: f32 = 682.0;

pub(super) const POWER_PLANT_DIALOG_H: f32 = 602.0;

pub(super) const POWER_PLANT_CARD_W: f32 = 208.0;

pub(super) const POWER_PLANT_CARD_H: f32 = 142.0;

pub(super) const POWER_PLANT_INFO_W: f32 = 72.0;

pub(super) const POWER_PLANT_INFO_H: f32 = 26.0;

pub(super) const POWER_PLANT_UI_SCALE: f32 = 3.0;

pub(super) const ARCOLOGY_DIALOG_W: f32 = 620.0;

pub(super) const ARCOLOGY_DIALOG_H: f32 = 472.0;

pub(super) const ARCOLOGY_CARD_W: f32 = 268.0;

pub(super) const ARCOLOGY_CARD_H: f32 = 148.0;

pub(super) const ARCOLOGY_INFO_W: f32 = 72.0;

pub(super) const ARCOLOGY_INFO_H: f32 = 26.0;

pub(super) const ARCOLOGY_UI_SCALE: f32 = 3.0;

pub(super) const CITY_INFO_DIALOG_W: f32 = 720.0;

pub(super) const CITY_INFO_DIALOG_H: f32 = 500.0;

pub(super) const CITY_INFO_UI_SCALE: f32 = 3.0;

pub(super) const CITY_MAP_GRID: usize = 32;

pub(super) const POWER_PLANT_CHOICES: &[PowerPlantChoice] = &[
    PowerPlantChoice {
        tool: EditTool::PowerPlantCoal,
        building: 207,
        megawatts: 200,
        cost: 4_000,
        label: "Coal Power",
        info: "COAL POWER - coal plants generate large amounts of vile black smoke that will cover your buildings in soot. Your citizens may start complaining when they have to repaint their buildings every two years.",
    },
    PowerPlantChoice {
        tool: EditTool::PowerPlantHydro,
        building: 198,
        megawatts: 20,
        cost: 400,
        label: "Hydro Electric",
        info: "HYDRO ELECTRIC - hydro plants are clean and cheap, but they must be built where falling water can drive the generators.",
    },
    PowerPlantChoice {
        tool: EditTool::PowerPlantOil,
        building: 202,
        megawatts: 220,
        cost: 6_600,
        label: "Oil Power",
        info: "OIL POWER - oil plants produce strong output but create heavy pollution and become expensive to keep running.",
    },
    PowerPlantChoice {
        tool: EditTool::PowerPlantGas,
        building: 201,
        megawatts: 50,
        cost: 2_000,
        label: "Gas Power",
        info: "GAS POWER - gas plants are smaller and cleaner than coal or oil, with modest output for a growing city.",
    },
    PowerPlantChoice {
        tool: EditTool::PowerPlantNuclear,
        building: 203,
        megawatts: 500,
        cost: 15_000,
        label: "Nuclear",
        info: "NUCLEAR POWER - nuclear plants provide a lot of power without smoke, but accidents can be catastrophic.",
    },
    PowerPlantChoice {
        tool: EditTool::PowerPlantWind,
        building: 200,
        megawatts: 4,
        cost: 100,
        label: "Wind Power",
        info: "WIND POWER - wind plants are cheap and clean, but each generator produces only a small amount of power.",
    },
    PowerPlantChoice {
        tool: EditTool::PowerPlantSolar,
        building: 204,
        megawatts: 50,
        cost: 1_300,
        label: "Solar Power",
        info: "SOLAR POWER - solar plants are clean and inexpensive, but their output is limited by available sunlight.",
    },
    PowerPlantChoice {
        tool: EditTool::PowerPlantMicrowave,
        building: 205,
        megawatts: 1600,
        cost: 28_000,
        label: "Microwave",
        info: "MICROWAVE POWER - microwave receivers collect power beamed down from orbit. Output is excellent, but failures can be dramatic.",
    },
    PowerPlantChoice {
        tool: EditTool::PowerPlantFusion,
        building: 206,
        megawatts: 2500,
        cost: 40_000,
        label: "Fusion",
        info: "FUSION POWER - fusion plants are expensive but produce enormous clean power for advanced cities.",
    },
];

pub(super) const ARCOLOGY_CHOICES: &[ArcologyChoice] = &[
    ArcologyChoice {
        tool: EditTool::ArcologyPlymouth,
        building: 251,
        capacity: 55_000,
        cost: 100_000,
        label: "Plymouth Arco",
        info: "PLYMOUTH ARCOLOGY - a rugged, industry-oriented arcology designed to withstand major earthquakes. It can attract up to 55,000 residents.",
    },
    ArcologyChoice {
        tool: EditTool::ArcologyForest,
        building: 252,
        capacity: 30_000,
        cost: 120_000,
        label: "Forest Arco",
        info: "FOREST ARCOLOGY - an ecologically focused community built around recycling, clean industry, and a forested upper level. It can attract up to 30,000 residents.",
    },
    ArcologyChoice {
        tool: EditTool::ArcologyDarco,
        building: 253,
        capacity: 45_000,
        cost: 150_000,
        label: "Darco",
        info: "DARCO - a de-urbanized arcological construct with an unconventional internal design. It can attract up to 45,000 residents.",
    },
    ArcologyChoice {
        tool: EditTool::ArcologyLaunch,
        building: 254,
        capacity: 65_000,
        cost: 200_000,
        label: "Launch Arco",
        info: "LAUNCH ARCOLOGY - the largest arcology, built to become a self-contained space colony. It can attract up to 65,000 residents.",
    },
];

pub(super) fn power_plant_choice_for_tool(tool: EditTool) -> Option<&'static PowerPlantChoice> {
    POWER_PLANT_CHOICES
        .iter()
        .find(|choice| choice.tool == tool)
}

pub(super) fn arcology_choice_for_tool(tool: EditTool) -> Option<&'static ArcologyChoice> {
    ARCOLOGY_CHOICES.iter().find(|choice| choice.tool == tool)
}

/// Modal overlays (budget / graphs / sign / plant pickers). Shared by mouse and finger.
/// Returns true when the pointer event should not fall through to map/tools.
pub(super) fn city_map_cell_color(city: &City, map_x: usize, map_y: usize) -> [f32; 4] {
    const TILE_BLOCK: usize = CITY_WIDTH / CITY_MAP_GRID;
    let mut counts = [0u16; 7];
    for x in map_x * TILE_BLOCK..(map_x + 1) * TILE_BLOCK {
        for y in map_y * TILE_BLOCK..(map_y + 1) * TILE_BLOCK {
            let building = city.building_at(x, y);
            if is_surface_infrastructure_tile(building) {
                counts[5] += 1;
                continue;
            }
            match city.zone_at(x, y).map(|zone| zone.zone_id()).unwrap_or(0) {
                1..=3 => counts[2] += 1,
                4..=6 => counts[3] += 1,
                7..=9 => counts[4] += 1,
                _ if building != 0 => counts[6] += 1,
                _ if city
                    .tile_bits_at(x, y)
                    .is_some_and(|bits| bits.water_tile())
                    || city.terrain_at(x, y) >= 0x10 =>
                {
                    counts[1] += 1
                }
                _ => counts[0] += 1,
            }
        }
    }
    let category = counts
        .iter()
        .enumerate()
        .max_by_key(|(index, count)| (**count, *index))
        .map(|(index, _)| index)
        .unwrap_or(0);
    match category {
        1 => [0.12, 0.28, 0.82, 1.0],
        2 => [0.15, 0.68, 0.18, 1.0],
        3 => [0.16, 0.32, 0.86, 1.0],
        4 => [0.88, 0.72, 0.08, 1.0],
        5 => [0.22, 0.22, 0.22, 1.0],
        6 => [0.48, 0.48, 0.48, 1.0],
        _ => [0.60, 0.50, 0.28, 1.0],
    }
}

pub(super) fn population_panel_demographic_rows(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
) -> Vec<(&'static str, u32)> {
    population_panel_demographic_rows_with_power(
        city,
        population,
        outcomes,
        &power_service_maps(city),
    )
}

pub(super) fn population_panel_demographic_rows_with_power(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
    power_maps: &PowerServiceMaps,
) -> Vec<(&'static str, u32)> {
    population_panel_demographic_rows_with_power_and_fidelity(
        city,
        population,
        outcomes,
        power_maps,
        EngineFidelity::Win95,
    )
}

pub(super) fn population_panel_demographic_rows_with_power_and_fidelity(
    city: &City,
    population: PopulationStats,
    outcomes: CityServiceOutcomes,
    power_maps: &PowerServiceMaps,
    fidelity: EngineFidelity,
) -> Vec<(&'static str, u32)> {
    let counters =
        live_city_scenario_counters_for_city_with_power(city, population, outcomes, power_maps);
    // PE stores fame at MISC +0x38, which Mac CODE 4 leaves as opaque state.
    let city_fame =
        if fidelity.uses_win95() && city.kind != CityFileKind::MacCity && city.pe_dw_city_fame != 0
        {
            city.pe_dw_city_fame
        } else {
            live_city_fame_value_with_fidelity(city, population, outcomes, counters, fidelity)
        };
    vec![
        (
            "Work Force %",
            work_force_percent_for_city_with_fidelity(city, population, fidelity),
        ),
        (
            "Unemployment %",
            unemployment_percent_for_city_with_fidelity(city, population, fidelity),
        ),
        ("Life Expectancy", outcomes.life_expectancy),
        ("Education Quotient", outcomes.education_quotient),
        ("Service Score", outcomes.service_score),
        ("City Fame", city_fame),
        ("National Population", city.pe_dw_national_population),
        ("National Value", city.pe_dw_national_value),
        ("National Tax", u32::from(city.pe_w_national_tax)),
        ("National Trend", u32::from(city.pe_w_national_trend)),
        ("Heat", u32::from(city.pe_b_weather_heat)),
        ("Wind", u32::from(city.pe_b_weather_wind)),
        ("Humid", u32::from(city.pe_b_weather_humid)),
        ("Weather Trend", u32::from(city.pe_b_weather_trend)),
        ("City Value", outcomes.city_value),
        ("Land Value", outcomes.land_value),
    ]
}

pub(super) fn push_info_text(
    solids: &mut Vec<UiSolid>,
    label: &str,
    value: &str,
    x: f32,
    y: f32,
    depth: f32,
) {
    ui::push_pixel_text(solids, label, x, y, 2.5, [0.0, 0.0, 0.0, 1.0], depth);
    ui::push_pixel_text(
        solids,
        value,
        x,
        y + 18.0,
        2.5,
        [0.12, 0.12, 0.12, 1.0],
        depth,
    );
}

pub(super) fn push_compact_info_text(
    solids: &mut Vec<UiSolid>,
    label: &str,
    value: &str,
    x: f32,
    y: f32,
    depth: f32,
) {
    ui::push_pixel_text(solids, label, x, y, 2.0, [0.0, 0.0, 0.0, 1.0], depth);
    ui::push_pixel_text(
        solids,
        value,
        x,
        y + 14.0,
        2.0,
        [0.12, 0.12, 0.12, 1.0],
        depth,
    );
}

pub(super) fn push_demand_row(
    solids: &mut Vec<UiSolid>,
    label: &str,
    value: i32,
    bar: FloatRect,
    color: [f32; 4],
    depth: f32,
) {
    ui::push_pixel_text(
        solids,
        label,
        bar.x,
        bar.y - 18.0,
        2.0,
        [0.0, 0.0, 0.0, 1.0],
        depth,
    );
    push_ui_rect(solids, bar, [0.44, 0.44, 0.44, 1.0], depth);
    let center = bar.x + bar.w * 0.5;
    push_ui_rect(
        solids,
        FloatRect {
            x: center,
            y: bar.y,
            w: 1.0,
            h: bar.h,
        },
        [0.0, 0.0, 0.0, 1.0],
        depth + 0.1,
    );
    let normalized = (value.clamp(-2000, 2000) as f32 / 2000.0) * bar.w * 0.5;
    push_ui_rect(
        solids,
        FloatRect {
            x: if normalized < 0.0 {
                center + normalized
            } else {
                center
            },
            y: bar.y + 2.0,
            w: normalized.abs(),
            h: bar.h - 4.0,
        },
        color,
        depth + 0.2,
    );
}

pub(super) fn push_graph_series(
    solids: &mut Vec<UiSolid>,
    values: &[u32],
    chart: FloatRect,
    max_value: u32,
    color: [f32; 4],
    depth: f32,
) {
    if values.len() < 2 {
        return;
    }
    let step = (chart.w - 8.0) / (values.len() - 1) as f32;
    let point_y =
        |value: u32| chart.y + chart.h - 4.0 - (value as f32 / max_value as f32) * (chart.h - 8.0);
    for index in 0..values.len() - 1 {
        let x0 = chart.x + 4.0 + index as f32 * step;
        let x1 = chart.x + 4.0 + (index + 1) as f32 * step;
        let y0 = point_y(values[index]);
        let y1 = point_y(values[index + 1]);
        push_ui_rect(
            solids,
            FloatRect {
                x: x0,
                y: y0.min(y1),
                w: 2.0,
                h: (y1 - y0).abs().max(2.0),
            },
            color,
            depth,
        );
        push_ui_rect(
            solids,
            FloatRect {
                x: x0,
                y: y1,
                w: (x1 - x0 + 2.0).max(2.0),
                h: 2.0,
            },
            color,
            depth,
        );
    }
}

pub(super) fn push_dialog_button(
    solids: &mut Vec<UiSolid>,
    rect: FloatRect,
    label: &str,
    pressed: bool,
    depth: f32,
) {
    push_dialog_button_disabled(solids, rect, label, pressed, false, depth);
}

pub(super) fn push_dialog_button_disabled(
    solids: &mut Vec<UiSolid>,
    rect: FloatRect,
    label: &str,
    pressed: bool,
    disabled: bool,
    depth: f32,
) {
    push_ui_rect(
        solids,
        rect,
        if disabled {
            [0.58, 0.58, 0.58, 1.0]
        } else if pressed {
            [0.52, 0.52, 0.52, 1.0]
        } else {
            [0.66, 0.66, 0.66, 1.0]
        },
        depth,
    );
    push_ui_bevel(solids, rect, 3.0, pressed, depth + 0.1);
    ui::push_pixel_text(
        solids,
        label,
        rect.x + (rect.w - ui::pixel_text_width(label, POWER_PLANT_UI_SCALE)) * 0.5,
        rect.y + (rect.h - 5.0 * POWER_PLANT_UI_SCALE) * 0.5,
        POWER_PLANT_UI_SCALE,
        if disabled {
            [0.24, 0.24, 0.24, 1.0]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        },
        depth + 0.2,
    );
}

pub(super) fn wrap_dialog_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len = if current.is_empty() {
            word.len()
        } else {
            current.len() + 1 + word.len()
        };
        if next_len > max_chars && !current.is_empty() {
            lines.push(current);
            current = word.to_owned();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}
