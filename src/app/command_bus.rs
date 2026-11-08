//! Shared application commands, the process-wide FIFO, and native menu mapping.
//!
//! Queue locking/order, callback ABI, shortcut modifier gates, and native command
//! IDs retain their existing behavior. State effects and runtime execution remain
//! in their respective session and runtime modules.

use super::dialogs::{CityInfoDialog, NewspaperEdition};
use super::{DisasterKind, title_screen};
use crate::asset_profile::{AppearancePreference, GraphicsPreference, OriginalMedia};
use crate::data_map::CityDataMapMode;
use crate::menu_model;
use crate::traffic::GameSpeed;
use crate::ui::EditTool;
use sdl3::keyboard::Keycode;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AppCommand {
    OpenCityDialog,
    OpenScenarioDialog,
    OpenPath(PathBuf),
    OpenTitlePath(PathBuf),
    SaveCity,
    SaveCityAs,
    SaveToPath(PathBuf),
    #[cfg(target_os = "ios")]
    DocumentSaved {
        path: PathBuf,
        previous_path: PathBuf,
    },
    NewCity,
    NewCityFromTitle {
        city_name: String,
        starting_funds: i32,
        starting_year: u16,
        difficulty: u16,
        generation_seed: u32,
        start_flow: title_screen::NewCityStartFlow,
    },
    ToggleEditMap,
    ReloadAssets,
    OpenTileSetDialog,
    LoadTileSet(PathBuf),
    OpenImportOriginalAssetsDialog,
    OpenOriginalAssetsDialog,
    OpenReplacementAssetsDialog,
    ImportOriginalAssets(PathBuf),
    SetOriginalAssets(PathBuf),
    SetReplacementAssets(PathBuf),
    SetGraphicsPreference(GraphicsPreference),
    SetAppearance(AppearancePreference),
    CycleAssetSource(OriginalMedia),
    ToggleCityGameEffects,
    ToggleCityGameMusic,
    ToggleCityGameCityMusic,
    ToggleToolbarPalette,
    ToggleCityGameNewspaper,
    ZoomIn,
    ZoomOut,
    ActualSize,
    CenterMap,
    ToggleGrid,
    ToggleZoneOverlay,
    ToggleTraffic,
    ToggleUnderground,
    SetDataMapMode(CityDataMapMode),
    ToggleAutoBudget,
    ToggleAutoGoto,
    ToggleSoundEffects,
    ToggleMusic,
    ToggleNoDisasters,
    ToggleNewspaperSubscription,
    ToggleNewspaperExtra,
    ToggleBalloonHelp,
    StartDisaster(DisasterKind),
    FitMap,
    InspectTile,
    InspectHoverTile,
    ReturnToTitle,
    OpenCityInfo(CityInfoDialog),
    SetSpeed(GameSpeed),
    SetTool(EditTool),
    RotateView(i32),
    Quit,
}

static COMMAND_QUEUE: OnceLock<Mutex<VecDeque<AppCommand>>> = OnceLock::new();

pub(super) fn command_queue() -> &'static Mutex<VecDeque<AppCommand>> {
    COMMAND_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(super) fn push_command(command: AppCommand) {
    command_queue()
        .lock()
        .expect("command queue")
        .push_back(command);
}

pub(super) fn pop_command() -> Option<AppCommand> {
    command_queue().lock().expect("command queue").pop_front()
}

pub(super) fn windows_menu_shortcut(
    key: Keycode,
    modifiers: sdl3::keyboard::Mod,
) -> Option<AppCommand> {
    use sdl3::keyboard::Mod;
    if !modifiers.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD | Mod::LGUIMOD | Mod::RGUIMOD)
        || modifiers.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD | Mod::LALTMOD | Mod::RALTMOD)
    {
        return None;
    }
    let dialog = match key {
        Keycode::M => CityInfoDialog::Map,
        Keycode::B => CityInfoDialog::Budget,
        Keycode::O => CityInfoDialog::Ordinances,
        Keycode::C => CityInfoDialog::Population,
        Keycode::I => CityInfoDialog::Industry,
        Keycode::G => CityInfoDialog::Graphs,
        Keycode::H => CityInfoDialog::Neighbors,
        _ => return None,
    };
    Some(AppCommand::OpenCityInfo(dialog))
}

pub(super) fn app_command_for_menu_command(command: i32) -> Option<AppCommand> {
    Some(match command {
        1 => AppCommand::OpenCityDialog,
        2 => AppCommand::ReloadAssets,
        3 => AppCommand::ZoomIn,
        4 => AppCommand::ZoomOut,
        5 => AppCommand::ActualSize,
        6 => AppCommand::CenterMap,
        7 => AppCommand::ToggleGrid,
        8 => AppCommand::InspectTile,
        9 => AppCommand::ToggleZoneOverlay,
        10 => AppCommand::FitMap,
        11 => AppCommand::ToggleTraffic,
        12 => AppCommand::InspectHoverTile,
        13 => AppCommand::SaveCity,
        14 => AppCommand::SaveCityAs,
        15 => AppCommand::NewCity,
        16 => AppCommand::ToggleEditMap,
        17 => AppCommand::OpenTileSetDialog,
        18 => AppCommand::OpenScenarioDialog,
        20 => AppCommand::SetSpeed(GameSpeed::Paused),
        21 => AppCommand::SetSpeed(GameSpeed::Turtle),
        22 => AppCommand::SetSpeed(GameSpeed::Llama),
        23 => AppCommand::SetSpeed(GameSpeed::Cheetah),
        24 => AppCommand::SetSpeed(GameSpeed::AfricanSwallow),
        30 => AppCommand::RotateView(-1),
        31 => AppCommand::RotateView(1),
        40 => AppCommand::OpenCityInfo(CityInfoDialog::Map),
        41 => AppCommand::OpenCityInfo(CityInfoDialog::Budget),
        42 => AppCommand::OpenCityInfo(CityInfoDialog::Population),
        43 => AppCommand::OpenCityInfo(CityInfoDialog::Industry),
        44 => AppCommand::OpenCityInfo(CityInfoDialog::Graphs),
        45 => AppCommand::OpenCityInfo(CityInfoDialog::Neighbors),
        46 => AppCommand::OpenCityInfo(CityInfoDialog::Ordinances),
        47 => AppCommand::OpenCityInfo(CityInfoDialog::Scenario),
        110 => AppCommand::SetDataMapMode(CityDataMapMode::City),
        111 => AppCommand::SetDataMapMode(CityDataMapMode::Power),
        112 => AppCommand::SetDataMapMode(CityDataMapMode::Water),
        113 => AppCommand::SetDataMapMode(CityDataMapMode::Traffic),
        114 => AppCommand::SetDataMapMode(CityDataMapMode::LandValue),
        116 => AppCommand::SetDataMapMode(CityDataMapMode::Pollution),
        117 => AppCommand::SetDataMapMode(CityDataMapMode::Crime),
        118 => AppCommand::SetDataMapMode(CityDataMapMode::Police),
        119 => AppCommand::SetDataMapMode(CityDataMapMode::Fire),
        120 => AppCommand::SetDataMapMode(CityDataMapMode::Density),
        121 => AppCommand::SetDataMapMode(CityDataMapMode::Growth),
        122 => AppCommand::SetDataMapMode(CityDataMapMode::Height),
        115 => AppCommand::ToggleUnderground,
        50 => AppCommand::ToggleAutoBudget,
        51 => AppCommand::ToggleAutoGoto,
        52 => AppCommand::ToggleSoundEffects,
        53 => AppCommand::ToggleMusic,
        60 => AppCommand::ToggleNoDisasters,
        61 => AppCommand::StartDisaster(DisasterKind::Fire),
        62 => AppCommand::StartDisaster(DisasterKind::Flood),
        63 => AppCommand::StartDisaster(DisasterKind::AirCrash),
        64 => AppCommand::StartDisaster(DisasterKind::Tornado),
        65 => AppCommand::StartDisaster(DisasterKind::Earthquake),
        66 => AppCommand::StartDisaster(DisasterKind::Monster),
        67 => AppCommand::StartDisaster(DisasterKind::Hurricane),
        68 => AppCommand::StartDisaster(DisasterKind::Rioters),
        70 => AppCommand::ToggleNewspaperSubscription,
        71 => AppCommand::OpenCityInfo(CityInfoDialog::Newspaper(NewspaperEdition::Courier)),
        72 => AppCommand::ToggleNewspaperExtra,
        80 => AppCommand::OpenCityInfo(CityInfoDialog::Help),
        81 => AppCommand::ToggleBalloonHelp,
        90 => AppCommand::OpenImportOriginalAssetsDialog,
        91 => AppCommand::OpenOriginalAssetsDialog,
        92 => AppCommand::OpenReplacementAssetsDialog,
        93 => AppCommand::SetGraphicsPreference(GraphicsPreference::PreferOriginal),
        94 => AppCommand::SetGraphicsPreference(GraphicsPreference::PreferReplacement),
        95 => AppCommand::CycleAssetSource(OriginalMedia::Graphics),
        96 => AppCommand::CycleAssetSource(OriginalMedia::Effects),
        97 => AppCommand::CycleAssetSource(OriginalMedia::Music),
        98 => AppCommand::ToggleCityGameEffects,
        99 => AppCommand::ToggleCityGameMusic,
        menu_model::CMD_TOGGLE_CITYGAME_TOOLBAR => AppCommand::ToggleToolbarPalette,
        menu_model::CMD_TOGGLE_CITYGAME_NEWSPAPER => AppCommand::ToggleCityGameNewspaper,
        menu_model::CMD_TOGGLE_CITYGAME_CITY_MUSIC => AppCommand::ToggleCityGameCityMusic,
        menu_model::CMD_APPEARANCE_SYSTEM => {
            AppCommand::SetAppearance(AppearancePreference::System)
        }
        menu_model::CMD_APPEARANCE_LIGHT => AppCommand::SetAppearance(AppearancePreference::Light),
        menu_model::CMD_APPEARANCE_DARK => AppCommand::SetAppearance(AppearancePreference::Dark),
        menu_model::CMD_QUIT => AppCommand::Quit,
        menu_model::CMD_RETURN_TO_TITLE => AppCommand::ReturnToTitle,
        _ => return None,
    })
}

pub(super) extern "C" fn menu_callback(command: i32) {
    if let Some(command) = app_command_for_menu_command(command) {
        push_command(command);
    }
}
