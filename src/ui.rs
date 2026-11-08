//! CityGame's project-authored high-density tool palettes and original action mapping.
use crate::city::AppMode;
use crate::renderer::{Renderer, UiImage, UiSolid};
use glam::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTool {
    Query,
    Bulldoze,
    RaiseTerrain,
    LowerTerrain,
    StretchTerrain,
    LevelTerrain,
    /// PE terrain tool-4 (`0x442a30`): copy center ALTM land onto NESW neighbors.
    SpreadTerrain,
    Water,
    Stream,
    Trees,
    Forest,
    Sign,
    Road,
    Highway,
    Tunnel,
    Onramp,
    BusDepot,
    Rail,
    Subway,
    RailDepot,
    SubStation,
    SubToRail,
    Power,
    ZoneResidential,
    ZoneResidentialDense,
    ZoneCommercial,
    ZoneCommercialDense,
    ZoneIndustrial,
    ZoneIndustrialDense,
    SmallPark,
    BigPark,
    Zoo,
    Stadium,
    Marina,
    PowerPlant,
    PowerPlantCoal,
    PowerPlantHydro,
    PowerPlantOil,
    PowerPlantGas,
    PowerPlantNuclear,
    PowerPlantWind,
    PowerPlantSolar,
    PowerPlantMicrowave,
    PowerPlantFusion,
    Police,
    FireStation,
    Hospital,
    Prison,
    School,
    College,
    Library,
    Museum,
    Pipes,
    WaterPump,
    WaterTower,
    WaterTreatment,
    Desalinization,
    Seaport,
    Airport,
    DispatchPolice,
    DispatchFirefighters,
    DispatchMilitary,
    MayorHouse,
    CityHall,
    Statue,
    BraunLlamaDome,
    Arcologies,
    ArcologyPlymouth,
    ArcologyForest,
    ArcologyDarco,
    ArcologyLaunch,
    DeZone,
}

impl EditTool {
    pub fn label(self) -> &'static str {
        match self {
            EditTool::Query => "Query",
            EditTool::Bulldoze => "Bulldoze",
            EditTool::RaiseTerrain => "Raise Terrain",
            EditTool::LowerTerrain => "Lower Terrain",
            EditTool::StretchTerrain => "Stretch Terrain",
            EditTool::LevelTerrain => "Level Terrain",
            EditTool::SpreadTerrain => "Spread Terrain",
            EditTool::Water => "Water",
            EditTool::Stream => "Stream",
            EditTool::Trees => "Trees",
            EditTool::Forest => "Forest",
            EditTool::Sign => "Signs",
            EditTool::Road => "Road",
            EditTool::Highway => "Highway",
            EditTool::Tunnel => "Tunnel",
            EditTool::Onramp => "Onramp",
            EditTool::BusDepot => "Bus Depot",
            EditTool::Rail => "Rail",
            EditTool::Subway => "Subway",
            EditTool::RailDepot => "Rail Depot",
            EditTool::SubStation => "Sub Station",
            EditTool::SubToRail => "Sub<-->Rail",
            EditTool::Power => "Power Lines",
            EditTool::ZoneResidential => "Light Residential",
            EditTool::ZoneResidentialDense => "Dense Residential",
            EditTool::ZoneCommercial => "Light Commercial",
            EditTool::ZoneCommercialDense => "Dense Commercial",
            EditTool::ZoneIndustrial => "Light Industrial",
            EditTool::ZoneIndustrialDense => "Dense Industrial",
            EditTool::SmallPark => "Small Park",
            EditTool::BigPark => "Big Park",
            EditTool::Zoo => "Zoo",
            EditTool::Stadium => "Stadium",
            EditTool::Marina => "Marina",
            EditTool::PowerPlant => "Power Plant",
            EditTool::PowerPlantCoal => "Coal Power",
            EditTool::PowerPlantHydro => "Hydro Electric",
            EditTool::PowerPlantOil => "Oil Power",
            EditTool::PowerPlantGas => "Gas Power",
            EditTool::PowerPlantNuclear => "Nuclear Power",
            EditTool::PowerPlantWind => "Wind Power",
            EditTool::PowerPlantSolar => "Solar Power",
            EditTool::PowerPlantMicrowave => "Microwave Power",
            EditTool::PowerPlantFusion => "Fusion Power",
            EditTool::Police => "Police",
            EditTool::FireStation => "Fire Station",
            EditTool::Hospital => "Hospital",
            EditTool::Prison => "Prison",
            EditTool::School => "School",
            EditTool::College => "College",
            EditTool::Library => "Library",
            EditTool::Museum => "Museum",
            EditTool::Pipes => "Pipes",
            EditTool::WaterPump => "Water Pump",
            EditTool::WaterTower => "Water Tower",
            EditTool::WaterTreatment => "Treatment",
            EditTool::Desalinization => "Desalinization",
            EditTool::Seaport => "Seaport",
            EditTool::Airport => "Airport",
            EditTool::DispatchPolice => "Dispatch Police",
            EditTool::DispatchFirefighters => "Dispatch Firefighters",
            EditTool::DispatchMilitary => "Dispatch Military",
            EditTool::MayorHouse => "Mayor's House",
            EditTool::CityHall => "City Hall",
            EditTool::Statue => "Statue",
            EditTool::BraunLlamaDome => "Braun Llama Dome",
            EditTool::Arcologies => "Arcologies",
            EditTool::ArcologyPlymouth => "Plymouth Arco",
            EditTool::ArcologyForest => "Forest Arco",
            EditTool::ArcologyDarco => "Darco",
            EditTool::ArcologyLaunch => "Launch Arco",
            EditTool::DeZone => "De-zone",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMenu {
    Residential,
    Commercial,
    Industrial,
    Parks,
    Power,
    Civic,
    Education,
    Water,
    Terrain,
    TerrainEdit,
    Road,
    Rail,
    Transport,
    Emergency,
    Rewards,
}

impl ToolMenu {
    pub fn label(self) -> &'static str {
        match self {
            ToolMenu::Residential => "Residential",
            ToolMenu::Commercial => "Commercial",
            ToolMenu::Industrial => "Industrial",
            ToolMenu::Parks => "Parks",
            ToolMenu::Power => "Power",
            ToolMenu::Civic => "Civic",
            ToolMenu::Education => "Education",
            ToolMenu::Water => "Water",
            ToolMenu::Terrain => "Terrain",
            ToolMenu::TerrainEdit => "Terrain Tools",
            ToolMenu::Road => "Roads",
            ToolMenu::Rail => "Rails",
            ToolMenu::Transport => "Transportation",
            ToolMenu::Emergency => "Emergency",
            ToolMenu::Rewards => "Rewards",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolMenuItem {
    pub label: &'static str,
    pub tool: EditTool,
}

const RESIDENTIAL_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Light Residential",
        tool: EditTool::ZoneResidential,
    },
    ToolMenuItem {
        label: "Dense Residential",
        tool: EditTool::ZoneResidentialDense,
    },
];

const COMMERCIAL_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Light Commercial",
        tool: EditTool::ZoneCommercial,
    },
    ToolMenuItem {
        label: "Dense Commercial",
        tool: EditTool::ZoneCommercialDense,
    },
];

const INDUSTRIAL_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Light Industrial",
        tool: EditTool::ZoneIndustrial,
    },
    ToolMenuItem {
        label: "Dense Industrial",
        tool: EditTool::ZoneIndustrialDense,
    },
];

const PARKS_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Small Park",
        tool: EditTool::SmallPark,
    },
    ToolMenuItem {
        label: "Big Park",
        tool: EditTool::BigPark,
    },
    ToolMenuItem {
        label: "Zoo",
        tool: EditTool::Zoo,
    },
    ToolMenuItem {
        label: "Stadium",
        tool: EditTool::Stadium,
    },
    ToolMenuItem {
        label: "Marina",
        tool: EditTool::Marina,
    },
];

const POWER_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Power Lines",
        tool: EditTool::Power,
    },
    ToolMenuItem {
        label: "Power Plant...",
        tool: EditTool::PowerPlant,
    },
];

const CIVIC_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Police",
        tool: EditTool::Police,
    },
    ToolMenuItem {
        label: "Fire Station",
        tool: EditTool::FireStation,
    },
    ToolMenuItem {
        label: "Hospital",
        tool: EditTool::Hospital,
    },
    ToolMenuItem {
        label: "Prison",
        tool: EditTool::Prison,
    },
];

const EDUCATION_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "School",
        tool: EditTool::School,
    },
    ToolMenuItem {
        label: "College",
        tool: EditTool::College,
    },
    ToolMenuItem {
        label: "Library",
        tool: EditTool::Library,
    },
    ToolMenuItem {
        label: "Museum",
        tool: EditTool::Museum,
    },
];

const WATER_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Pipes",
        tool: EditTool::Pipes,
    },
    ToolMenuItem {
        label: "Water Pump",
        tool: EditTool::WaterPump,
    },
    ToolMenuItem {
        label: "Water Tower",
        tool: EditTool::WaterTower,
    },
    ToolMenuItem {
        label: "Treatment",
        tool: EditTool::WaterTreatment,
    },
    ToolMenuItem {
        label: "Desalinization",
        tool: EditTool::Desalinization,
    },
];

const ROAD_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Road",
        tool: EditTool::Road,
    },
    ToolMenuItem {
        label: "Highway",
        tool: EditTool::Highway,
    },
    ToolMenuItem {
        label: "Tunnel",
        tool: EditTool::Tunnel,
    },
    ToolMenuItem {
        label: "Onramp",
        tool: EditTool::Onramp,
    },
    ToolMenuItem {
        label: "Bus Depot",
        tool: EditTool::BusDepot,
    },
];

const RAIL_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Rail",
        tool: EditTool::Rail,
    },
    ToolMenuItem {
        label: "Subway",
        tool: EditTool::Subway,
    },
    ToolMenuItem {
        label: "Rail Depot",
        tool: EditTool::RailDepot,
    },
    ToolMenuItem {
        label: "Sub Station",
        tool: EditTool::SubStation,
    },
    ToolMenuItem {
        label: "Sub<-->Rail",
        tool: EditTool::SubToRail,
    },
];

const TRANSPORT_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Seaport",
        tool: EditTool::Seaport,
    },
    ToolMenuItem {
        label: "Airport",
        tool: EditTool::Airport,
    },
];

const EMERGENCY_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Dispatch Police",
        tool: EditTool::DispatchPolice,
    },
    ToolMenuItem {
        label: "Dispatch Firefighters",
        tool: EditTool::DispatchFirefighters,
    },
    ToolMenuItem {
        label: "Dispatch Military",
        tool: EditTool::DispatchMilitary,
    },
];

const REWARDS_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Mayor's House",
        tool: EditTool::MayorHouse,
    },
    ToolMenuItem {
        label: "City Hall",
        tool: EditTool::CityHall,
    },
    ToolMenuItem {
        label: "Statue",
        tool: EditTool::Statue,
    },
    ToolMenuItem {
        label: "Braun Llama Dome",
        tool: EditTool::BraunLlamaDome,
    },
    ToolMenuItem {
        label: "Arcologies",
        tool: EditTool::Arcologies,
    },
];

const TERRAIN_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Trees",
        tool: EditTool::Trees,
    },
    ToolMenuItem {
        label: "Water",
        tool: EditTool::Water,
    },
];

const TERRAIN_EDIT_MENU_ITEMS: &[ToolMenuItem] = &[
    ToolMenuItem {
        label: "Demolish/Clear",
        tool: EditTool::Bulldoze,
    },
    ToolMenuItem {
        label: "Level Terrain",
        tool: EditTool::LevelTerrain,
    },
    ToolMenuItem {
        label: "Raise Terrain",
        tool: EditTool::RaiseTerrain,
    },
    ToolMenuItem {
        label: "Lower Terrain",
        tool: EditTool::LowerTerrain,
    },
    ToolMenuItem {
        label: "Spread Terrain",
        tool: EditTool::SpreadTerrain,
    },
    ToolMenuItem {
        label: "De-zone",
        tool: EditTool::DeZone,
    },
];

pub fn tool_menu_items(menu: ToolMenu) -> &'static [ToolMenuItem] {
    match menu {
        ToolMenu::Residential => RESIDENTIAL_MENU_ITEMS,
        ToolMenu::Commercial => COMMERCIAL_MENU_ITEMS,
        ToolMenu::Industrial => INDUSTRIAL_MENU_ITEMS,
        ToolMenu::Parks => PARKS_MENU_ITEMS,
        ToolMenu::Power => POWER_MENU_ITEMS,
        ToolMenu::Civic => CIVIC_MENU_ITEMS,
        ToolMenu::Education => EDUCATION_MENU_ITEMS,
        ToolMenu::Water => WATER_MENU_ITEMS,
        ToolMenu::Terrain => TERRAIN_MENU_ITEMS,
        ToolMenu::TerrainEdit => TERRAIN_EDIT_MENU_ITEMS,
        ToolMenu::Road => ROAD_MENU_ITEMS,
        ToolMenu::Rail => RAIL_MENU_ITEMS,
        ToolMenu::Transport => TRANSPORT_MENU_ITEMS,
        ToolMenu::Emergency => EMERGENCY_MENU_ITEMS,
        ToolMenu::Rewards => REWARDS_MENU_ITEMS,
    }
}

pub const UI_DEPTH: f32 = 2_000_000.0;

const REFERENCE_HEIGHT: f32 = 768.0;
const MAC_PALETTE_SCALE: f32 = 2.0;
const PALETTE_ART_SCALE: u32 = 8;

const CITY_PALETTE_W: u32 = 71;
const CITY_PALETTE_H: u32 = 312;
const EDIT_PALETTE_W: u32 = 50;
const EDIT_PALETTE_H: u32 = 306;

#[derive(Debug, Clone, Copy)]
struct UiMetrics {
    palette_scale: f32,
}

impl UiMetrics {
    fn for_logical_height(logical_height: u32) -> Self {
        let window = (logical_height.max(480) as f32 / REFERENCE_HEIGHT).clamp(1.0, 2.75);
        Self {
            palette_scale: MAC_PALETTE_SCALE * window,
        }
    }

    fn palette_width(&self, mode: AppMode) -> f32 {
        let src_w = match mode {
            AppMode::CityView => CITY_PALETTE_W,
            AppMode::EditNewMap => EDIT_PALETTE_W,
        };
        src_w as f32 * self.palette_scale
    }

    fn palette_height(&self, mode: AppMode) -> f32 {
        let src_h = match mode {
            AppMode::CityView => CITY_PALETTE_H,
            AppMode::EditNewMap => EDIT_PALETTE_H,
        };
        src_h as f32 * self.palette_scale
    }
}

/// Logical client size for the dedicated SDL palette window at the reference scale.
pub fn palette_logical_size(mode: AppMode) -> (u32, u32) {
    let (_src_w, src_h) = match mode {
        AppMode::CityView => (CITY_PALETTE_W, CITY_PALETTE_H),
        AppMode::EditNewMap => (EDIT_PALETTE_W, EDIT_PALETTE_H),
    };
    let m = UiMetrics::for_logical_height(src_h);
    (
        m.palette_width(mode).round() as u32,
        m.palette_height(mode).round() as u32,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    SelectTool(EditTool),
    OpenMenu(ToolMenu),
    ToggleEditCoast,
    ToggleEditRiver,
    GenerateEditMap,
    RaiseEditSeaLevel,
    LowerEditSeaLevel,
    DoneEditMap,
    RotateViewCcw,
    RotateViewCw,
    ZoomOut,
    ZoomIn,
    CenterMap,
    OpenCityMap,
    OpenCityPopulation,
    OpenCityNeighbors,
    OpenCityGraphs,
    OpenCityIndustry,
    OpenBudget,
    OpenHelp,
    ToggleBuildings,
    ToggleSigns,
    ToggleInfrastructure,
    ToggleZones,
    ToggleUnderground,
}

impl UiAction {
    pub fn label(self) -> &'static str {
        match self {
            UiAction::SelectTool(tool) => tool.label(),
            UiAction::OpenMenu(menu) => menu.label(),
            UiAction::ToggleEditCoast => "Coast",
            UiAction::ToggleEditRiver => "River",
            UiAction::GenerateEditMap => "Make",
            UiAction::RaiseEditSeaLevel => "Raise Sea Level",
            UiAction::LowerEditSeaLevel => "Lower Sea Level",
            UiAction::DoneEditMap => "Done",
            UiAction::RotateViewCcw => "Rotate Counter-Clockwise",
            UiAction::RotateViewCw => "Rotate Clockwise",
            UiAction::ZoomOut => "Zoom Out",
            UiAction::ZoomIn => "Zoom In",
            UiAction::CenterMap => "Center Map",
            UiAction::OpenCityMap => "City Map",
            UiAction::OpenCityPopulation => "Population",
            UiAction::OpenCityNeighbors => "Neighbors",
            UiAction::OpenCityGraphs => "Graphs",
            UiAction::OpenCityIndustry => "Industry",
            UiAction::OpenBudget => "Budget",
            UiAction::OpenHelp => "Help",
            UiAction::ToggleBuildings => "Buildings",
            UiAction::ToggleSigns => "Signs",
            UiAction::ToggleInfrastructure => "Infrastructure",
            UiAction::ToggleZones => "Zones",
            UiAction::ToggleUnderground => "Underground",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PaletteButton {
    action: UiAction,
    src: [u32; 4],
}

pub const EDIT_MAP_SLIDER_MAX: u8 = 47;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMapSlider {
    Hills,
    Water,
    Trees,
}

impl EditMapSlider {
    pub fn label(self) -> &'static str {
        match self {
            EditMapSlider::Hills => "Hills",
            EditMapSlider::Water => "Water",
            EditMapSlider::Trees => "Trees",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditMapSliderValues {
    pub hills: u8,
    pub water: u8,
    pub trees: u8,
}

impl Default for EditMapSliderValues {
    fn default() -> Self {
        // SC2K-docs terrain generator defaults: Hills 12, Water 5, Trees 15.
        Self {
            hills: 12,
            water: 5,
            trees: 15,
        }
    }
}

impl EditMapSliderValues {
    pub fn value(self, slider: EditMapSlider) -> u8 {
        match slider {
            EditMapSlider::Hills => self.hills,
            EditMapSlider::Water => self.water,
            EditMapSlider::Trees => self.trees,
        }
    }

    pub fn set(&mut self, slider: EditMapSlider, value: u8) {
        let value = value.min(EDIT_MAP_SLIDER_MAX);
        match slider {
            EditMapSlider::Hills => self.hills = value,
            EditMapSlider::Water => self.water = value,
            EditMapSlider::Trees => self.trees = value,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EditMapSliderRect {
    slider: EditMapSlider,
    src: [u32; 4],
}

const EDIT_MAP_SLIDERS: &[EditMapSliderRect] = &[
    EditMapSliderRect {
        slider: EditMapSlider::Hills,
        src: [1, 30, 15, 48],
    },
    EditMapSliderRect {
        slider: EditMapSlider::Water,
        src: [17, 30, 15, 48],
    },
    EditMapSliderRect {
        slider: EditMapSlider::Trees,
        src: [33, 30, 15, 48],
    },
];

const CITY_PALETTE_BUTTONS: &[PaletteButton] = &[
    // Map-window rotate arrows (PICT 500): left = counter-clockwise, right = clockwise.
    PaletteButton {
        action: UiAction::RotateViewCcw,
        src: [9, 153, 26, 19],
    },
    PaletteButton {
        action: UiAction::RotateViewCw,
        src: [36, 153, 26, 19],
    },
    // View controls (PICT 500): magnifier -, magnifier +, center-map target.
    PaletteButton {
        action: UiAction::ZoomOut,
        src: [1, 177, 23, 24],
    },
    PaletteButton {
        action: UiAction::ZoomIn,
        src: [24, 177, 23, 24],
    },
    PaletteButton {
        action: UiAction::CenterMap,
        src: [47, 177, 23, 24],
    },
    // City information buttons (PICT 500), arranged in visual row order.
    PaletteButton {
        action: UiAction::OpenCityMap,
        src: [1, 202, 23, 19],
    },
    PaletteButton {
        action: UiAction::OpenCityGraphs,
        src: [24, 202, 23, 19],
    },
    PaletteButton {
        action: UiAction::OpenCityPopulation,
        src: [1, 222, 23, 19],
    },
    PaletteButton {
        action: UiAction::OpenCityIndustry,
        src: [24, 222, 23, 19],
    },
    PaletteButton {
        action: UiAction::OpenCityNeighbors,
        src: [1, 242, 23, 19],
    },
    PaletteButton {
        action: UiAction::OpenBudget,
        src: [24, 242, 23, 19],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::TerrainEdit),
        src: [1, 1, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Terrain),
        src: [24, 1, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Emergency),
        src: [47, 1, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Power),
        src: [1, 25, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Water),
        src: [24, 25, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Rewards),
        src: [47, 25, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Road),
        src: [1, 49, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Rail),
        src: [24, 49, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Transport),
        src: [47, 49, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Residential),
        src: [1, 73, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Commercial),
        src: [24, 73, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Industrial),
        src: [47, 73, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Education),
        src: [1, 97, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Civic),
        src: [24, 97, 23, 24],
    },
    PaletteButton {
        action: UiAction::OpenMenu(ToolMenu::Parks),
        src: [47, 97, 23, 24],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::Sign),
        src: [1, 121, 34, 26],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::Query),
        src: [36, 121, 33, 26],
    },
    // Original display-layer controls at the bottom of PICT 500.
    PaletteButton {
        action: UiAction::ToggleBuildings,
        src: [1, 266, 34, 14],
    },
    PaletteButton {
        action: UiAction::ToggleSigns,
        src: [36, 266, 33, 14],
    },
    PaletteButton {
        action: UiAction::ToggleInfrastructure,
        src: [1, 281, 34, 14],
    },
    PaletteButton {
        action: UiAction::ToggleZones,
        src: [36, 281, 33, 14],
    },
    PaletteButton {
        action: UiAction::ToggleUnderground,
        src: [1, 296, 44, 14],
    },
    PaletteButton {
        action: UiAction::OpenHelp,
        src: [46, 296, 23, 14],
    },
];

#[cfg(test)]
pub(crate) fn city_palette_actions_for_test() -> impl Iterator<Item = UiAction> {
    CITY_PALETTE_BUTTONS.iter().map(|button| button.action)
}

const EDIT_PALETTE_BUTTONS: &[PaletteButton] = &[
    PaletteButton {
        action: UiAction::ToggleEditCoast,
        src: [1, 1, 48, 16],
    },
    PaletteButton {
        action: UiAction::ToggleEditRiver,
        src: [1, 17, 48, 16],
    },
    PaletteButton {
        action: UiAction::GenerateEditMap,
        src: [1, 88, 48, 16],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::RaiseTerrain),
        src: [1, 107, 24, 21],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::LowerTerrain),
        src: [25, 107, 24, 21],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::LevelTerrain),
        src: [25, 128, 24, 21],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::StretchTerrain),
        src: [1, 128, 24, 21],
    },
    PaletteButton {
        action: UiAction::RaiseEditSeaLevel,
        src: [1, 149, 24, 21],
    },
    PaletteButton {
        action: UiAction::LowerEditSeaLevel,
        src: [25, 149, 24, 21],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::Water),
        src: [1, 170, 24, 21],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::Stream),
        src: [25, 170, 24, 21],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::Trees),
        src: [1, 191, 24, 21],
    },
    PaletteButton {
        action: UiAction::SelectTool(EditTool::Forest),
        src: [25, 191, 24, 21],
    },
    PaletteButton {
        action: UiAction::ZoomOut,
        src: [1, 218, 24, 21],
    },
    PaletteButton {
        action: UiAction::ZoomIn,
        src: [25, 218, 24, 21],
    },
    PaletteButton {
        action: UiAction::RotateViewCcw,
        src: [1, 239, 24, 21],
    },
    PaletteButton {
        action: UiAction::RotateViewCw,
        src: [25, 239, 24, 21],
    },
    PaletteButton {
        action: UiAction::CenterMap,
        src: [1, 260, 24, 21],
    },
    PaletteButton {
        action: UiAction::OpenHelp,
        src: [25, 260, 24, 21],
    },
];

const EDIT_DONE_BUTTON: [u32; 4] = [1, 287, 48, 16];

mod palette;
pub use palette::*;
mod theme;
pub use theme::*;
