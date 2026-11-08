//! Shared Classic Mac–style menu definitions and check state.
//!
//! Command IDs match [`native/mac_menu.m`] / `app_command_for_menu_command`.
//! macOS keeps AppKit; Linux/Windows (and later tablets) use the in-game menubar.

use crate::data_map::CityDataMapMode;
use crate::traffic::GameSpeed;

pub const CMD_OPEN_CITY: i32 = 1;
pub const CMD_RELOAD_ASSETS: i32 = 2;
pub const CMD_ZOOM_IN: i32 = 3;
pub const CMD_ZOOM_OUT: i32 = 4;
pub const CMD_ACTUAL_SIZE: i32 = 5;
pub const CMD_CENTER_MAP: i32 = 6;
pub const CMD_TOGGLE_GRID: i32 = 7;
pub const CMD_INSPECT_TILE: i32 = 8;
pub const CMD_TOGGLE_ZONE_OVERLAY: i32 = 9;
pub const CMD_FIT_MAP: i32 = 10;
pub const CMD_TOGGLE_TRAFFIC: i32 = 11;
pub const CMD_INSPECT_HOVER_TILE: i32 = 12;
pub const CMD_SAVE_CITY: i32 = 13;
pub const CMD_SAVE_CITY_AS: i32 = 14;
pub const CMD_RETURN_TO_TITLE: i32 = 19;
pub const CMD_NEW_CITY: i32 = 15;
pub const CMD_TOGGLE_EDIT_MAP: i32 = 16;
pub const CMD_LOAD_TILE_SET: i32 = 17;
pub const CMD_LOAD_SCENARIO: i32 = 18;
pub const CMD_SPEED_PAUSE: i32 = 20;
pub const CMD_SPEED_TURTLE: i32 = 21;
pub const CMD_SPEED_LLAMA: i32 = 22;
pub const CMD_SPEED_CHEETAH: i32 = 23;
pub const CMD_SPEED_SWALLOW: i32 = 24;
pub const CMD_ROTATE_CCW: i32 = 30;
pub const CMD_ROTATE_CW: i32 = 31;
pub const CMD_OPEN_MAP: i32 = 40;
pub const CMD_OPEN_BUDGET: i32 = 41;
pub const CMD_OPEN_POPULATION: i32 = 42;
pub const CMD_OPEN_INDUSTRY: i32 = 43;
pub const CMD_OPEN_GRAPHS: i32 = 44;
pub const CMD_OPEN_NEIGHBORS: i32 = 45;
pub const CMD_OPEN_ORDINANCES: i32 = 46;
pub const CMD_OPEN_SCENARIO_PROGRESS: i32 = 47;
/// Main-view data map overlays (View menu).
pub const CMD_DATA_MAP_CITY: i32 = 110;
pub const CMD_DATA_MAP_POWER: i32 = 111;
pub const CMD_DATA_MAP_WATER: i32 = 112;
pub const CMD_DATA_MAP_TRAFFIC: i32 = 113;
pub const CMD_DATA_MAP_LAND_VALUE: i32 = 114;
pub const CMD_TOGGLE_UNDERGROUND: i32 = 115;
pub const CMD_DATA_MAP_POLLUTION: i32 = 116;
pub const CMD_DATA_MAP_CRIME: i32 = 117;
pub const CMD_DATA_MAP_POLICE: i32 = 118;
pub const CMD_DATA_MAP_FIRE: i32 = 119;
pub const CMD_DATA_MAP_DENSITY: i32 = 120;
pub const CMD_DATA_MAP_GROWTH: i32 = 121;
pub const CMD_DATA_MAP_HEIGHT: i32 = 122;
pub const CMD_TOGGLE_AUTO_BUDGET: i32 = 50;
pub const CMD_TOGGLE_AUTO_GOTO: i32 = 51;
pub const CMD_TOGGLE_SOUND_EFFECTS: i32 = 52;
pub const CMD_TOGGLE_MUSIC: i32 = 53;
pub const CMD_TOGGLE_NO_DISASTERS: i32 = 60;
pub const CMD_START_FIRE: i32 = 61;
pub const CMD_START_FLOOD: i32 = 62;
pub const CMD_START_AIR_CRASH: i32 = 63;
pub const CMD_START_TORNADO: i32 = 64;
pub const CMD_START_EARTHQUAKE: i32 = 65;
pub const CMD_START_MONSTER: i32 = 66;
pub const CMD_START_HURRICANE: i32 = 67;
pub const CMD_START_RIOTERS: i32 = 68;
pub const CMD_TOGGLE_NEWSPAPER_SUBSCRIPTION: i32 = 70;
pub const CMD_OPEN_COURIER: i32 = 71;
pub const CMD_TOGGLE_NEWSPAPER_EXTRA: i32 = 72;
pub const CMD_OPEN_HELP: i32 = 80;
pub const CMD_TOGGLE_BALLOON_HELP: i32 = 81;
pub const CMD_IMPORT_ORIGINAL_ASSETS: i32 = 90;
pub const CMD_CHOOSE_ORIGINAL_ASSETS: i32 = 91;
pub const CMD_CHOOSE_REPLACEMENT_ASSETS: i32 = 92;
pub const CMD_PREFER_ORIGINAL_GRAPHICS: i32 = 93;
pub const CMD_PREFER_REPLACEMENT_GRAPHICS: i32 = 94;
pub const CMD_NEXT_GRAPHICS_SOURCE: i32 = 95;
pub const CMD_NEXT_EFFECTS_SOURCE: i32 = 96;
pub const CMD_NEXT_MUSIC_SOURCE: i32 = 97;
pub const CMD_TOGGLE_CITYGAME_EFFECTS: i32 = 98;
pub const CMD_TOGGLE_CITYGAME_MUSIC: i32 = 99;
pub const CMD_TOGGLE_CITYGAME_TOOLBAR: i32 = 101;
pub const CMD_TOGGLE_CITYGAME_NEWSPAPER: i32 = 102;
/// In-game Quit (AppKit uses NSApp terminate instead).
pub const CMD_QUIT: i32 = 100;
pub const CMD_TOGGLE_CITYGAME_CITY_MUSIC: i32 = 103;
pub const CMD_APPEARANCE_SYSTEM: i32 = 130;
pub const CMD_APPEARANCE_LIGHT: i32 = 131;
pub const CMD_APPEARANCE_DARK: i32 = 132;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuEntry {
    Command {
        id: i32,
        label: &'static str,
        /// Shown hint only (e.g. `Ctrl+L`); input may already use bare keys.
        accel: Option<&'static str>,
        checkable: bool,
    },
    Separator,
}

#[derive(Debug, Clone, Copy)]
pub struct MenuDef {
    pub title: &'static str,
    pub items: &'static [MenuEntry],
}

pub fn menus() -> &'static [MenuDef] {
    &[
        MenuDef {
            title: "File",
            items: &[
                MenuEntry::Command {
                    id: CMD_LOAD_TILE_SET,
                    label: "Load Tile Set",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_OPEN_CITY,
                    label: "Load City",
                    accel: Some("Ctrl+L"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_NEW_CITY,
                    label: "New City",
                    accel: Some("Ctrl+N"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_EDIT_MAP,
                    label: "Edit New Map",
                    accel: Some("Ctrl+E"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_LOAD_SCENARIO,
                    label: "Load Scenario",
                    accel: Some("Ctrl+Z"),
                    checkable: false,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_SAVE_CITY,
                    label: "Save City",
                    accel: Some("Ctrl+S"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_SAVE_CITY_AS,
                    label: "Save City As...",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_RETURN_TO_TITLE,
                    label: "Return to Title Screen...",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_QUIT,
                    label: "Quit",
                    accel: Some("Ctrl+Q"),
                    checkable: false,
                },
            ],
        },
        MenuDef {
            title: "Speed",
            items: &[
                MenuEntry::Command {
                    id: CMD_SPEED_PAUSE,
                    label: "Pause",
                    accel: Some("Ctrl+P"),
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_SPEED_TURTLE,
                    label: "Turtle",
                    accel: Some("Ctrl+1"),
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_SPEED_LLAMA,
                    label: "Llama",
                    accel: Some("Ctrl+2"),
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_SPEED_CHEETAH,
                    label: "Cheetah",
                    accel: Some("Ctrl+3"),
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_SPEED_SWALLOW,
                    label: "African Swallow",
                    accel: Some("Ctrl+4"),
                    checkable: true,
                },
            ],
        },
        MenuDef {
            title: "Options",
            items: &[
                MenuEntry::Command {
                    id: CMD_TOGGLE_AUTO_BUDGET,
                    label: "Auto-Budget",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_AUTO_GOTO,
                    label: "Auto-Goto",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_SOUND_EFFECTS,
                    label: "Sound Effects",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_MUSIC,
                    label: "Music",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_APPEARANCE_SYSTEM,
                    label: "Appearance: Follow System",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_APPEARANCE_LIGHT,
                    label: "Appearance: Light",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_APPEARANCE_DARK,
                    label: "Appearance: Dark",
                    accel: None,
                    checkable: true,
                },
            ],
        },
        MenuDef {
            title: "Assets",
            items: &[
                MenuEntry::Command {
                    id: CMD_IMPORT_ORIGINAL_ASSETS,
                    label: "Import from Game Files...",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_CHOOSE_ORIGINAL_ASSETS,
                    label: "Choose Extracted Original Pack...",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_CHOOSE_REPLACEMENT_ASSETS,
                    label: "Choose Our Graphics Pack...",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_NEXT_GRAPHICS_SOURCE,
                    label: "Next Graphics Edition",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_NEXT_EFFECTS_SOURCE,
                    label: "Next Sound Edition",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_NEXT_MUSIC_SOURCE,
                    label: "Next Music Edition",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_PREFER_ORIGINAL_GRAPHICS,
                    label: "Prefer Original Graphics",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_PREFER_REPLACEMENT_GRAPHICS,
                    label: "Prefer High-Res CityGame Graphics",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_CITYGAME_EFFECTS,
                    label: "Use CityGame Sounds",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_CITYGAME_MUSIC,
                    label: "Use CityGame Title Music",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_CITYGAME_CITY_MUSIC,
                    label: "Use CityGame City Music",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_CITYGAME_TOOLBAR,
                    label: "Use CityGame Toolbar Palette",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_CITYGAME_NEWSPAPER,
                    label: "Show CityGame Newspaper",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_RELOAD_ASSETS,
                    label: "Reload Assets",
                    accel: Some("Ctrl+Shift+R"),
                    checkable: false,
                },
            ],
        },
        MenuDef {
            title: "Disasters",
            items: &[
                MenuEntry::Command {
                    id: CMD_START_FIRE,
                    label: "Fire",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_START_FLOOD,
                    label: "Flood",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_START_AIR_CRASH,
                    label: "Air Crash",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_START_TORNADO,
                    label: "Tornado",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_START_EARTHQUAKE,
                    label: "Earthquake",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_START_MONSTER,
                    label: "Monster",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_START_HURRICANE,
                    label: "Hurricane",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_START_RIOTERS,
                    label: "Rioters",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_TOGGLE_NO_DISASTERS,
                    label: "No Disasters",
                    accel: None,
                    checkable: true,
                },
            ],
        },
        MenuDef {
            title: "View",
            items: &[
                MenuEntry::Command {
                    id: CMD_DATA_MAP_CITY,
                    label: "City",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_POWER,
                    label: "Power Supply",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_WATER,
                    label: "Water Supply",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_TRAFFIC,
                    label: "Traffic",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_LAND_VALUE,
                    label: "Land Value",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_POLLUTION,
                    label: "Pollution",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_CRIME,
                    label: "Crime",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_POLICE,
                    label: "Police Power",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_FIRE,
                    label: "Fire Power",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_DENSITY,
                    label: "Density",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_GROWTH,
                    label: "Rate of Growth",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_DATA_MAP_HEIGHT,
                    label: "Heightmap",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_UNDERGROUND,
                    label: "Underground",
                    accel: Some("U"),
                    checkable: true,
                },
            ],
        },
        MenuDef {
            title: "Windows",
            items: &[
                MenuEntry::Command {
                    id: CMD_OPEN_MAP,
                    label: "Map",
                    accel: Some("Ctrl+M"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_OPEN_BUDGET,
                    label: "Budget",
                    accel: Some("Ctrl+B"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_OPEN_ORDINANCES,
                    label: "Ordinances",
                    accel: Some("Ctrl+O"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_OPEN_POPULATION,
                    label: "Population",
                    accel: Some("Ctrl+C"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_OPEN_INDUSTRY,
                    label: "Industry",
                    accel: Some("Ctrl+I"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_OPEN_GRAPHS,
                    label: "Graphs",
                    accel: Some("Ctrl+G"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_OPEN_NEIGHBORS,
                    label: "Neighbors",
                    accel: Some("Ctrl+H"),
                    checkable: false,
                },
            ],
        },
        MenuDef {
            title: "Newspaper",
            items: &[
                MenuEntry::Command {
                    id: CMD_TOGGLE_NEWSPAPER_SUBSCRIPTION,
                    label: "Subscription",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_NEWSPAPER_EXTRA,
                    label: "Extra!!!",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_OPEN_COURIER,
                    label: "Courier",
                    accel: None,
                    checkable: true,
                },
            ],
        },
        MenuDef {
            title: "Help",
            items: &[
                MenuEntry::Command {
                    id: CMD_OPEN_HELP,
                    label: "About Balloon Help...",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_BALLOON_HELP,
                    label: "Show Balloons",
                    accel: None,
                    checkable: true,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_OPEN_SCENARIO_PROGRESS,
                    label: "Scenario Progress",
                    accel: None,
                    checkable: false,
                },
            ],
        },
        MenuDef {
            title: "Debug",
            items: &[
                MenuEntry::Command {
                    id: CMD_RELOAD_ASSETS,
                    label: "Reload Assets",
                    accel: Some("Ctrl+R"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_ZOOM_IN,
                    label: "Zoom In",
                    accel: Some("+"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_ZOOM_OUT,
                    label: "Zoom Out",
                    accel: Some("-"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_ACTUAL_SIZE,
                    label: "Actual Size",
                    accel: Some("Ctrl+0"),
                    checkable: false,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_CENTER_MAP,
                    label: "Center Map",
                    accel: Some("Ctrl+Shift+C"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_FIT_MAP,
                    label: "Fit Whole Map",
                    accel: Some("Ctrl+Shift+F"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_GRID,
                    label: "Toggle Grid",
                    accel: Some("Ctrl+Shift+G"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_ZONE_OVERLAY,
                    label: "Toggle Zone Overlay",
                    accel: Some("Ctrl+Shift+Z"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_TOGGLE_TRAFFIC,
                    label: "Toggle Traffic",
                    accel: Some("Ctrl+Shift+T"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_INSPECT_TILE,
                    label: "Inspect Tile",
                    accel: Some("Ctrl+Shift+I"),
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_INSPECT_HOVER_TILE,
                    label: "Inspect Hover Tile",
                    accel: Some("Ctrl+Shift+H"),
                    checkable: false,
                },
                MenuEntry::Separator,
                MenuEntry::Command {
                    id: CMD_ROTATE_CCW,
                    label: "Rotate Counter-Clockwise",
                    accel: None,
                    checkable: false,
                },
                MenuEntry::Command {
                    id: CMD_ROTATE_CW,
                    label: "Rotate Clockwise",
                    accel: Some("Ctrl+]"),
                    checkable: false,
                },
            ],
        },
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuCheckState {
    pub appearance: crate::asset_profile::AppearancePreference,
    pub speed: GameSpeed,
    pub edit_map: bool,
    pub data_map_mode: CityDataMapMode,
    pub show_underground: bool,
    pub auto_budget: bool,
    pub auto_goto: bool,
    pub sound_effects: bool,
    pub music: bool,
    pub no_disasters: bool,
    pub newspaper_subscription: bool,
    pub newspaper_extra: bool,
    pub balloon_help: bool,
    pub prefer_replacement_graphics: bool,
    pub citygame_effects: bool,
    pub citygame_music: bool,
    pub citygame_city_music: bool,
    pub citygame_toolbar: bool,
    pub citygame_newspaper: bool,
}

impl Default for MenuCheckState {
    fn default() -> Self {
        Self {
            appearance: crate::asset_profile::AppearancePreference::System,
            speed: GameSpeed::Llama,
            edit_map: false,
            data_map_mode: CityDataMapMode::City,
            show_underground: false,
            auto_budget: false,
            auto_goto: false,
            sound_effects: true,
            music: true,
            no_disasters: false,
            newspaper_subscription: false,
            newspaper_extra: false,
            balloon_help: false,
            prefer_replacement_graphics: false,
            citygame_effects: false,
            citygame_music: false,
            citygame_city_music: false,
            citygame_toolbar: true,
            citygame_newspaper: true,
        }
    }
}

impl MenuCheckState {
    pub fn item_checked(self, id: i32) -> bool {
        match id {
            CMD_APPEARANCE_SYSTEM => {
                self.appearance == crate::asset_profile::AppearancePreference::System
            }
            CMD_APPEARANCE_LIGHT => {
                self.appearance == crate::asset_profile::AppearancePreference::Light
            }
            CMD_APPEARANCE_DARK => {
                self.appearance == crate::asset_profile::AppearancePreference::Dark
            }
            CMD_SPEED_PAUSE => self.speed == GameSpeed::Paused,
            CMD_SPEED_TURTLE => self.speed == GameSpeed::Turtle,
            CMD_SPEED_LLAMA => self.speed == GameSpeed::Llama,
            CMD_SPEED_CHEETAH => self.speed == GameSpeed::Cheetah,
            CMD_SPEED_SWALLOW => self.speed == GameSpeed::AfricanSwallow,
            CMD_DATA_MAP_CITY => self.data_map_mode == CityDataMapMode::City,
            CMD_DATA_MAP_POWER => self.data_map_mode == CityDataMapMode::Power,
            CMD_DATA_MAP_WATER => self.data_map_mode == CityDataMapMode::Water,
            CMD_DATA_MAP_TRAFFIC => self.data_map_mode == CityDataMapMode::Traffic,
            CMD_DATA_MAP_LAND_VALUE => self.data_map_mode == CityDataMapMode::LandValue,
            CMD_DATA_MAP_POLLUTION => self.data_map_mode == CityDataMapMode::Pollution,
            CMD_DATA_MAP_CRIME => self.data_map_mode == CityDataMapMode::Crime,
            CMD_DATA_MAP_POLICE => self.data_map_mode == CityDataMapMode::Police,
            CMD_DATA_MAP_FIRE => self.data_map_mode == CityDataMapMode::Fire,
            CMD_DATA_MAP_DENSITY => self.data_map_mode == CityDataMapMode::Density,
            CMD_DATA_MAP_GROWTH => self.data_map_mode == CityDataMapMode::Growth,
            CMD_DATA_MAP_HEIGHT => self.data_map_mode == CityDataMapMode::Height,
            CMD_TOGGLE_UNDERGROUND => self.show_underground,
            CMD_TOGGLE_AUTO_BUDGET => self.auto_budget,
            CMD_TOGGLE_AUTO_GOTO => self.auto_goto,
            CMD_TOGGLE_SOUND_EFFECTS => self.sound_effects,
            CMD_TOGGLE_MUSIC => self.music,
            CMD_TOGGLE_NO_DISASTERS => self.no_disasters,
            CMD_TOGGLE_NEWSPAPER_SUBSCRIPTION => self.newspaper_subscription,
            CMD_TOGGLE_NEWSPAPER_EXTRA => self.newspaper_extra,
            CMD_TOGGLE_BALLOON_HELP => self.balloon_help,
            CMD_PREFER_ORIGINAL_GRAPHICS => !self.prefer_replacement_graphics,
            CMD_PREFER_REPLACEMENT_GRAPHICS => self.prefer_replacement_graphics,
            CMD_TOGGLE_CITYGAME_EFFECTS => self.citygame_effects,
            CMD_TOGGLE_CITYGAME_MUSIC => self.citygame_music,
            CMD_TOGGLE_CITYGAME_CITY_MUSIC => self.citygame_city_music,
            CMD_TOGGLE_CITYGAME_TOOLBAR => self.citygame_toolbar,
            CMD_TOGGLE_CITYGAME_NEWSPAPER => self.citygame_newspaper,
            // AppKit keeps Courier permanently checked as the active edition.
            CMD_OPEN_COURIER => true,
            _ => false,
        }
    }

    pub fn item_enabled(self, id: i32) -> bool {
        if self.edit_map
            && matches!(
                id,
                CMD_SPEED_PAUSE
                    | CMD_SPEED_TURTLE
                    | CMD_SPEED_LLAMA
                    | CMD_SPEED_CHEETAH
                    | CMD_SPEED_SWALLOW
            )
        {
            return false;
        }
        if self.no_disasters
            && matches!(
                id,
                CMD_START_FIRE
                    | CMD_START_FLOOD
                    | CMD_START_AIR_CRASH
                    | CMD_START_TORNADO
                    | CMD_START_EARTHQUAKE
                    | CMD_START_MONSTER
                    | CMD_START_HURRICANE
                    | CMD_START_RIOTERS
            )
        {
            return false;
        }
        true
    }

    pub fn menu_enabled(self, title: &str) -> bool {
        !(self.edit_map && title == "Speed")
    }
}
