//! City-session creation, opening, saving, editor transitions, and menu state.
//!
//! Process RNG continuity, terrain setup, cache and traffic initialization, and
//! menu-command side-effect order are preserved. Runtime resource ownership,
//! native simulation workers, and traffic drawing remain in their existing modules.

use super::{
    AppCommand, CITY_MODE_DISASTER, CITY_MODE_EDIT_MAP, CITY_MODE_GAMEPLAY,
    CITY_OPTION_DISABLED_VALUE, CITY_OPTION_ENABLED_VALUE, CityInfoDialog, DEFAULT_WEATHER_HEAT,
    DEFAULT_WEATHER_HUMID, DEFAULT_WEATHER_TREND, DEFAULT_WEATHER_WIND, DisasterKind,
    EDIT_MAP_COAST_MISC_OFFSET, EDIT_MAP_RIVER_MISC_OFFSET, GRAPH_HISTORY_TOTAL_SERIES, GraphScale,
    MISC_CITY_MODE_OFFSET, MISC_DIFFICULTY_OFFSET, MISC_HEAT_OFFSET, MISC_HUMID_OFFSET,
    MISC_NATIONAL_POPULATION_OFFSET, MISC_NATIONAL_TAX_OFFSET, MISC_NATIONAL_TREND_OFFSET,
    MISC_NATIONAL_VALUE_OFFSET, MISC_SPEED_OFFSET, MISC_WEATHER_TREND_OFFSET, MISC_WIND_OFFSET,
    PE_RAISE_SUBMODE_DEFAULT, PeGrowthAggregate, PlacementSound, RATE_OF_GROWTH_NEUTRAL,
    SpriteCache, ViewerState, auto_budget_enabled_for_engine, auto_goto_enabled_for_engine,
    city_display_name, disaster_start_sound, ensure_budget_misc_defaults_before_extension,
    ensure_default_city_options_for_engine, ensure_industry_misc_defaults_before_extension,
    game_clock_from_city, init_pe_process_crt, init_pe_process_lfsr, initialize_hard_start_bond,
    initialize_new_city_budget_rows, loaded_city_info_dialog_for_city, mac_loop_scheduler,
    music_enabled_for_engine, newspaper_extra_enabled_for_city,
    newspaper_subscription_enabled_for_city, no_disasters_enabled_for_engine,
    pe_disaster_auto_goto_tile, pe_idle_scheduler, pe_misc_mirror_storage_active,
    pe_rebuild_w_city_developed_tiles_46ab56, push_command, scenario_start_clock_for_city,
    set_auto_budget_enabled_for_engine, set_auto_goto_enabled_for_engine,
    set_city_newspaper_extra_enabled_with_crt, set_city_newspaper_subscription_enabled_with_crt,
    set_music_enabled_for_engine, set_no_disasters_enabled_for_engine,
    set_sound_effects_enabled_for_engine, sound_effects_enabled_for_engine, terrain_generation,
    title_screen,
};
use crate::asset_profile::{AppearancePreference, AssetProfile};
use crate::city::{
    AppMode, BLANK_MAP_BASE_HEIGHT, CITY_HEIGHT, CITY_TILES, City, CityDate, CityFileKind, TileBits,
};
use crate::data_map::CityDataMapMode;
use crate::engine_fidelity::EngineFidelity;
use crate::mac_random::{MacGameLcgRng, MacToolboxRng};
use crate::menu_model::MenuCheckState;
use crate::menubar::MenubarUi;
use crate::traffic::{GameSpeed, TrafficSystem};
use crate::ui::EditTool;
use crate::win95_overlay_sim::Win95OverlaySim;
use crate::win95_random::{Win95CrtRand, Win95GameRand, Win95Lfsr};
use crate::{mac_menu, paths, ui};
use anyhow::{Context, Result};
use glam::Vec2;
use sdl3::dialog::{DialogFileFilter, show_open_file_dialog, show_save_file_dialog};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const EDIT_MAP_GENERATION_SEED_STEP: u32 = 0x9E37_79B9;

fn normalize_new_city_name(value: &str) -> String {
    let mut name = String::new();
    for ch in value.chars() {
        if name.chars().count() >= 31 {
            break;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '\'') {
            name.push(ch);
        }
    }
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        "New City".to_string()
    } else {
        name
    }
}

fn new_city_path_for_name(current: &Path, name: &str) -> PathBuf {
    let parent = current.parent().unwrap_or_else(|| Path::new("."));
    let stem = name
        .chars()
        .map(|ch| if ch == ' ' { '-' } else { ch })
        .collect::<String>();
    let mut path = parent.join(format!("{stem}.SC2"));
    let mut suffix = 2u32;
    while path.exists() && path != current {
        path = parent.join(format!("{stem}-{suffix}.SC2"));
        suffix += 1;
    }
    path
}

/// Publish an explicit editor/gameplay transition using the selected engine's
/// state owner. Mac CODE 2 export 75 (A5+0x27a) writes run-state 0/1 for
/// these actions; its separate state-2 arm is not inferred from PE disasters.
fn persist_app_mode_transition(city: &mut City, mode: AppMode, fidelity: EngineFidelity) {
    if fidelity == EngineFidelity::Mac68k {
        let run_state = match mode {
            AppMode::EditNewMap => CITY_MODE_EDIT_MAP,
            AppMode::CityView => CITY_MODE_GAMEPLAY,
        };
        city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, run_state);
    } else {
        persist_pe_city_mode_misc(city, mode, fidelity);
    }
}

pub(super) fn persist_pe_city_mode_misc(city: &mut City, mode: AppMode, fidelity: EngineFidelity) {
    if !fidelity.uses_win95() {
        return;
    }
    let city_mode = pe_city_mode_misc_value(mode, city.has_active_disaster()) as u16;
    city.pe_w_city_mode = city_mode;
    if !pe_misc_mirror_storage_active(city, fidelity) {
        // Mac CODE 4 owns MISC +0x04 as its `ffffe0c4` run-state gate.
        return;
    }
    city.set_misc_u32_at(MISC_CITY_MODE_OFFSET, u32::from(city_mode));
}

pub(super) fn pe_city_mode_misc_value(mode: AppMode, disaster_active: bool) -> u32 {
    match mode {
        AppMode::EditNewMap => CITY_MODE_EDIT_MAP,
        AppMode::CityView if disaster_active => CITY_MODE_DISASTER,
        AppMode::CityView => CITY_MODE_GAMEPLAY,
    }
}

pub(super) fn sc2k_speed_misc_value(speed: GameSpeed) -> u32 {
    match speed {
        GameSpeed::Paused => 1,
        GameSpeed::Turtle => 2,
        GameSpeed::Llama => 3,
        GameSpeed::Cheetah => 4,
        GameSpeed::AfricanSwallow => 5,
    }
}

pub(super) fn game_speed_from_sc2k_misc(value: u32) -> Option<GameSpeed> {
    match value {
        1 => Some(GameSpeed::Paused),
        2 => Some(GameSpeed::Turtle),
        3 => Some(GameSpeed::Llama),
        4 => Some(GameSpeed::Cheetah),
        5 => Some(GameSpeed::AfricanSwallow),
        _ => None,
    }
}

fn game_speed_for_city(city: &City, engine_fidelity: EngineFidelity) -> GameSpeed {
    city.misc_u32_at(MISC_SPEED_OFFSET)
        .map(|value| {
            if city.kind == CityFileKind::MacCity {
                u32::from(value as u16)
            } else {
                value
            }
        })
        .and_then(game_speed_from_sc2k_misc)
        .map(|speed| {
            if engine_fidelity == EngineFidelity::Mac68k && speed == GameSpeed::AfricanSwallow {
                GameSpeed::Cheetah
            } else {
                speed
            }
        })
        .unwrap_or(GameSpeed::Paused)
}

pub(super) fn traffic_system_for_city(
    city: &City,
    engine_fidelity: EngineFidelity,
) -> TrafficSystem {
    let mut traffic = TrafficSystem::new(city);
    traffic.set_speed(game_speed_for_city(city, engine_fidelity));
    traffic
}

pub(super) fn speed_misc_value_for_city(
    city: &City,
    engine_fidelity: EngineFidelity,
    speed: GameSpeed,
) -> Option<u32> {
    if pe_misc_mirror_storage_active(city, engine_fidelity) {
        return Some(sc2k_speed_misc_value(speed));
    }

    // Mac CODE 4 saves a WORD in the low half of this shared MISC slot. A
    // Win95 engine over a Mac file can persist the four native selectors too,
    // but its fifth selector has no Mac-file representation.
    let speed = if engine_fidelity == EngineFidelity::Mac68k && speed == GameSpeed::AfricanSwallow {
        GameSpeed::Cheetah
    } else {
        speed
    };
    if city.kind == CityFileKind::MacCity
        && city.misc_u32_at(MISC_SPEED_OFFSET).is_some()
        && speed != GameSpeed::AfricanSwallow
    {
        return Some(sc2k_speed_misc_value(speed));
    }

    // A Mac engine can also run over a Win95-layout save; that file stores the
    // selector as a full DWORD and follows the ordinary MISC writer.
    if engine_fidelity == EngineFidelity::Mac68k && city.misc_u32_at(MISC_SPEED_OFFSET).is_some() {
        return Some(sc2k_speed_misc_value(speed));
    }

    None
}

pub(super) fn persist_speed_misc_value_to_city(city: &mut City, value: u32) {
    if city.kind == CityFileKind::MacCity {
        city.set_mac_state_misc_low_word_at(MISC_SPEED_OFFSET, value as u16);
    } else {
        city.set_misc_u32_at(MISC_SPEED_OFFSET, value);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct AppCommandStateEffect {
    pub(super) update_title: bool,
    pub(super) sync_renderer_to_window: bool,
    pub(super) sync_palette_stacking: bool,
    pub(super) log_traffic_stats: bool,
    pub(super) placement_sound: Option<PlacementSound>,
    pub(super) disaster_log: Option<(DisasterKind, bool)>,
    pub(super) auto_goto_tile: Option<(usize, usize)>,
}

/// SDL holds the real window handle; wgpu keeps an immutable borrow for the surface lifetime.
pub(super) fn set_window_title(video: &sdl3::VideoSubsystem, window_id: u32, title: &str) {
    unsafe {
        if let Ok(mut window) = video.window_from_id(window_id) {
            window.set_title(title).ok();
        }
    }
}

pub(super) fn save_city_dialog(window: &sdl3::video::Window) {
    let filters = [DialogFileFilter {
        name: "CityGame City",
        pattern: "sc2",
    }];
    // Tablets: SDL routes this to the platform document picker; default into Cities/.
    let default_path = paths::default_open_dialog_dir().join("Untitled.SC2");
    if let Err(error) = show_save_file_dialog(
        &filters,
        Some(default_path),
        window,
        Box::new(|result, _filter| match result {
            Ok(paths) => {
                if let Some(path) = paths.into_iter().next() {
                    push_command(AppCommand::SaveToPath(path));
                }
            }
            Err(error) => log::error!("save dialog error: {error}"),
        }),
    ) {
        log::error!("failed to open save dialog: {error}");
    }
}

#[cfg(target_os = "ios")]
pub(super) fn open_city_dialog(window: &sdl3::video::Window) {
    crate::ios_documents::open_city(window, false);
}

#[cfg(not(target_os = "ios"))]
pub(super) fn open_city_dialog(window: &sdl3::video::Window) {
    let filters = [
        DialogFileFilter {
            name: "CityGame City (.sc2)",
            pattern: "sc2",
        },
        DialogFileFilter {
            name: "Scenario (.scn)",
            pattern: "scn",
        },
        DialogFileFilter {
            name: "All Files",
            pattern: "*",
        },
    ];

    // On Android/iOS SDL shows the system document picker; start dir is best-effort.
    let default_path = paths::default_open_dialog_dir();
    if let Err(error) = show_open_file_dialog(
        &filters,
        Some(default_path),
        false,
        window,
        Box::new(|result, _filter| match result {
            Ok(paths) => {
                if let Some(path) = paths.into_iter().next() {
                    push_command(AppCommand::OpenPath(path));
                }
            }
            Err(error) => log::error!("file dialog error: {error}"),
        }),
    ) {
        log::error!("failed to open file dialog: {error}");
    }
}

#[cfg(target_os = "ios")]
pub(super) fn open_title_city_dialog(window: &sdl3::video::Window) {
    crate::ios_documents::open_city(window, true);
}

#[cfg(not(target_os = "ios"))]
pub(super) fn open_title_city_dialog(window: &sdl3::video::Window) {
    let filters = [
        DialogFileFilter {
            name: "CityGame City (.sc2)",
            pattern: "sc2",
        },
        DialogFileFilter {
            name: "Scenario (.scn)",
            pattern: "scn",
        },
        DialogFileFilter {
            name: "All Files",
            pattern: "*",
        },
    ];
    if let Err(error) = show_open_file_dialog(
        &filters,
        Some(paths::default_open_dialog_dir()),
        false,
        window,
        Box::new(|result, _filter| match result {
            Ok(paths) => {
                if let Some(path) = paths.into_iter().next() {
                    push_command(AppCommand::OpenTitlePath(path));
                }
            }
            Err(error) => log::error!("file dialog error: {error}"),
        }),
    ) {
        log::error!("failed to open city dialog from title: {error}");
    }
}

#[cfg(target_os = "ios")]
pub(super) fn open_scenario_dialog(window: &sdl3::video::Window, _profile: &AssetProfile) {
    crate::ios_documents::open_city(window, false);
}

#[cfg(not(target_os = "ios"))]
pub(super) fn open_scenario_dialog(window: &sdl3::video::Window, profile: &AssetProfile) {
    let filters = [
        DialogFileFilter {
            name: "Scenario (.scn)",
            pattern: "scn",
        },
        DialogFileFilter {
            name: "All Files",
            pattern: "*",
        },
    ];

    let default_path = scenario_dialog_start_dir(profile);
    if let Err(error) = show_open_file_dialog(
        &filters,
        Some(default_path),
        false,
        window,
        Box::new(|result, _filter| match result {
            Ok(paths) => {
                if let Some(path) = paths.into_iter().next() {
                    push_command(AppCommand::OpenPath(path));
                }
            }
            Err(error) => log::error!("scenario dialog error: {error}"),
        }),
    ) {
        log::error!("failed to open scenario dialog: {error}");
    }
}

#[cfg(target_os = "ios")]
pub(super) fn open_title_scenario_dialog(window: &sdl3::video::Window, _profile: &AssetProfile) {
    crate::ios_documents::open_city(window, true);
}

#[cfg(not(target_os = "ios"))]
pub(super) fn open_title_scenario_dialog(window: &sdl3::video::Window, profile: &AssetProfile) {
    let filters = [
        DialogFileFilter {
            name: "Scenario (.scn)",
            pattern: "scn",
        },
        DialogFileFilter {
            name: "All Files",
            pattern: "*",
        },
    ];
    if let Err(error) = show_open_file_dialog(
        &filters,
        Some(scenario_dialog_start_dir(profile)),
        false,
        window,
        Box::new(|result, _filter| match result {
            Ok(paths) => {
                if let Some(path) = paths.into_iter().next() {
                    push_command(AppCommand::OpenTitlePath(path));
                }
            }
            Err(error) => log::error!("scenario dialog error: {error}"),
        }),
    ) {
        log::error!("failed to open scenario dialog from title: {error}");
    }
}

pub(super) fn scenario_dialog_start_dir(profile: &AssetProfile) -> PathBuf {
    let mut candidates = Vec::new();
    for pack in profile.imported_packs() {
        candidates.extend([
            pack.root.join("catalog/scenarios"),
            pack.root.join("SCENARIO"),
            pack.root.join("scenarios"),
        ]);
    }
    if let Some(root) = profile.original_root() {
        candidates.extend([
            root.join("catalog/scenarios"),
            root.join("SCENARIO"),
            root.join("scenarios"),
        ]);
    }

    let configured_root = paths::default_open_dialog_dir();
    candidates.extend([
        configured_root.join("SCENARIO"),
        configured_root.join("scenarios"),
        configured_root.clone(),
    ]);

    candidates
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or(configured_root)
}

pub(super) fn apply_new_city_title_selections(
    city: &mut City,
    starting_funds: Option<i32>,
    starting_year: Option<u16>,
    difficulty: Option<u16>,
) {
    if let Some(funds) = starting_funds {
        city.set_city_funds(funds);
    }
    if let Some(year) = starting_year {
        city.set_saved_date(CityDate { year, month: 1 });
    }
    if let Some(year) = starting_year.or_else(|| city.saved_date().map(|date| date.year)) {
        city.initialize_new_city_founding_year(year);
    }
    if let Some(difficulty) = difficulty {
        if city.kind == CityFileKind::MacCity {
            // Mac CODE 4 owns this same MISC slot as a signed WORD. Write it
            // through the Mac state path so title selections initialize the
            // native value without treating it as a PE-only cache field.
            city.set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, u32::from(difficulty));
        } else {
            city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, u32::from(difficulty));
        }
    }
}

pub(super) fn initialize_original_new_city_national_misc(city: &mut City) {
    if city.kind == CityFileKind::MacCity {
        return;
    }
    let start_year = city.saved_date().map(|date| date.year).unwrap_or(1900);
    let national_population = match start_year {
        1900 => 10_000,
        1950 => 25_000,
        2000 => 60_000,
        2050 => 150_000,
        _ => 10_000,
    };
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, national_population);
    city.set_misc_u32_at(
        MISC_NATIONAL_VALUE_OFFSET,
        national_population.saturating_mul(3) / 10,
    );
    // PE new-city @ `0x42e547`: `wNationalTax` = 3.
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 3);
    // PE @ `0x42e562`: `wNationalTrend = wCityDifficulty - 1`.
    city.set_misc_u32_at(
        MISC_NATIONAL_TREND_OFFSET,
        u32::from(city.pe_w_city_difficulty).saturating_sub(1),
    );
    // PE new-city @ `0x40e173`: ocean=0, river=1 (BSS `0x4c94c0`/`0x4cb3f8`).
    city.pe_init_ocean_river_new_city_40e173();
    city.set_misc_u32_at(MISC_HEAT_OFFSET, DEFAULT_WEATHER_HEAT);
    city.set_misc_u32_at(MISC_WIND_OFFSET, DEFAULT_WEATHER_WIND);
    city.set_misc_u32_at(MISC_HUMID_OFFSET, DEFAULT_WEATHER_HUMID);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, DEFAULT_WEATHER_TREND);
}

pub(super) fn initialize_mac_fidelity_new_city_industry_rows(
    city: &mut City,
    engine_fidelity: EngineFidelity,
) {
    if engine_fidelity.uses_win95() || city.kind == CityFileKind::MacCity {
        return;
    }
    // The current Mac-fidelity New City fallback is still a Win-format blank
    // city, so the Win95 extension seeder above is correctly disabled. Both
    // native reset paths nevertheless establish these shared rows as 0/7/0.
    // Seed that recovered input without treating a Mac-layout vector as PE
    // industry state.
    ensure_industry_misc_defaults_before_extension(city);
}

pub(super) fn initialize_new_city_microsim_planes(city: &mut City) {
    let empty_map = vec![0; CITY_TILES];
    city.sync_population_density_from_slice(&empty_map);
    city.sync_police_coverage_from_slice(&empty_map);
    city.sync_fire_coverage_from_slice(&empty_map);
    let neutral_growth = vec![RATE_OF_GROWTH_NEUTRAL; CITY_TILES];
    city.sync_rate_of_growth_from_slice(&neutral_growth);
}

pub(super) fn edit_map_slider_percent(value: u8) -> u8 {
    (((u16::from(value.min(ui::EDIT_MAP_SLIDER_MAX)) * 100) + 23)
        / u16::from(ui::EDIT_MAP_SLIDER_MAX)) as u8
}

impl ViewerState {
    pub(super) fn sync_menu_appearance(&mut self, preference: AppearancePreference) {
        self.menu_checks.appearance = preference;
    }

    pub(super) fn window_title_base(&self) -> String {
        let city_name = city_display_name(&self.city);
        format!(
            "CityGame — {city_name} — {} — ${}{}",
            self.game_clock.month_year_label(),
            super::format_signed_number(self.city.city_funds()),
            if self.mode == AppMode::EditNewMap {
                " — Edit Map"
            } else {
                ""
            }
        )
    }

    pub(super) fn load_default() -> Result<Self> {
        Self::load_default_from_path(paths::default_city_path())
    }

    pub(super) fn load_default_from_path(path: PathBuf) -> Result<Self> {
        Self::load_default_from_path_with_process_rng(path, None, None, None)
    }

    pub(super) fn load_default_from_path_with_process_rng(
        path: PathBuf,
        process_crt: Option<Win95CrtRand>,
        process_game_rand: Option<Win95GameRand>,
        process_lfsr: Option<Win95Lfsr>,
    ) -> Result<Self> {
        let mut city = City::load(&path)?;
        let engine_fidelity = EngineFidelity::from_env();
        if engine_fidelity.uses_win95() {
            city.prepare_pe_fidelity_from_saved_city();
            // City-open scan 0x434b70 initializes the day-2 developed-tile WORD.
            pe_rebuild_w_city_developed_tiles_46ab56(&mut city);
            city.hydrate_pe_attitude_scores_from_xgrp();
        } else {
            city.prepare_mac_fidelity_from_saved_city();
        }
        city.set_pe_scenario_active(
            engine_fidelity.uses_win95() && city.kind == CityFileKind::WinScenario,
        );
        let game_clock = game_clock_from_city(&city);
        let mut pe_process_crt =
            process_crt.unwrap_or_else(|| init_pe_process_crt(&city, game_clock));
        ensure_default_city_options_for_engine(
            &mut city,
            engine_fidelity,
            Some(&mut pe_process_crt),
        );
        let pe_process_game_rand =
            process_game_rand.unwrap_or_else(|| Win95GameRand::new(Win95GameRand::DEFAULT_STATE));
        let pe_process_lfsr =
            process_lfsr.unwrap_or_else(|| init_pe_process_lfsr(&city, game_clock));
        // Mac CODE 15 selects its startup SONG with one bound-5 draw from the
        // process-owned CODE 13 game LCG before city gameplay. Preserve that
        // stream advance even when CityGame supplies replacement music.
        let mac_game_rng = if engine_fidelity.uses_win95() {
            MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE)
        } else {
            MacGameLcgRng::after_startup_playlist_choice()
        };
        city.clear_legacy_dispatch_actor_proxies();
        log::info!(
            "loaded city {} from {} (traffic on by default, T toggle{}, compass={}, camera_rotation={}, saved_rotation={}, saved_view={:?})",
            city.name,
            path.display(),
            if city.has_traffic_density_map() {
                ", XTRF density"
            } else {
                ", no XTRF — heuristic placement"
            },
            city.rotation(),
            city.rotation(),
            city.saved_rotation(),
            city.saved_view(),
        );
        let mut win95_overlay_sim = Win95OverlaySim::new();
        win95_overlay_sim.reset_for_city(&city);
        let auto_goto_enabled = auto_goto_enabled_for_engine(&city, engine_fidelity);
        let sound_effects_enabled = sound_effects_enabled_for_engine(&city, engine_fidelity);
        let music_enabled = music_enabled_for_engine(&city, engine_fidelity);
        let no_disasters_enabled = no_disasters_enabled_for_engine(&city, engine_fidelity);
        let mut state = Self {
            city: city.clone(),
            camera: Vec2::ZERO,
            zoom: 2.0,
            viewport_pixel_scale: 1.0,
            camera_rotation: city.rotation(),
            show_grid: false,
            show_zone_overlay: false,
            show_traffic: true,
            data_map_mode: CityDataMapMode::City,
            show_buildings: true,
            show_signs: true,
            show_infrastructure: true,
            show_zones: true,
            show_underground: false,
            auto_goto_enabled,
            pending_disaster_auto_goto: None,
            pending_disaster_start_sound: None,
            pending_disaster_start_siren: false,
            pending_disaster_extra_sounds: Vec::new(),
            earthquake_shake_remaining: 0,
            pe_disaster_duration: 0,
            pe_hurricane_wind_remaining: 0,
            sound_effects_enabled,
            music_enabled,
            no_disasters_enabled,
            balloon_help_enabled: false,
            balloon_help_line: None,
            anim_time: 0.0,
            traffic_redraw_epoch: 0,
            traffic_redraw_accumulator: 0.0,
            pending_traffic_redraw: false,
            pe_idle_scheduler: pe_idle_scheduler::PeIdleScheduler::default(),
            mac_loop_scheduler: mac_loop_scheduler::MacLoopScheduler::default(),
            traffic: traffic_system_for_city(&city, engine_fidelity),
            win95_overlay_sim,
            pe_process_crt,
            mac_process_rng: MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED),
            mac_kind2_sound_deadline: 0,
            mac_game_rng,
            pe_process_game_rand,
            pe_raise_submode: PE_RAISE_SUBMODE_DEFAULT,
            pe_raise_adjust_land: 0,
            pe_raise_xter_ui_byte: 0,
            pe_raise_ui_tool_id: 0,
            pe_raise_ui_tile_x: 0,
            pe_raise_ui_tile_y: 0,
            pe_raise_press_y: 0,
            pe_raise_ui_press_y_byte: 0,
            pe_raise_ui_byte6: 0,
            pe_raise_ui_byte7: 0,
            pe_raise_ui_byte8: 0,
            pe_raise_ui_byte9: 0,
            pe_raise_drag_dir: 0xffff,
            pe_raise_this_f8: 0,
            pe_spread_tool4_ui_timer: 0,
            pe_spread_prev_tile_x: 0,
            pe_spread_prev_tile_y: 0,
            pe_stretch_hold_si: 0,
            pe_stretch_hold_ui_refresh_count: 0,
            pe_stretch_hold_cursor_id: 0,
            pe_stretch_hold_cursor_arg: 0,
            pe_stretch_hold_ui_wait_4012c1_count: 0,
            pe_lower_hold_ui_refresh_count: 0,
            pe_lower_hold_cursor_id: 0,
            pe_lower_hold_cursor_arg: 0,
            pe_level_hold_sound_after_altm: 0,
            pe_level_hold_sound_after_smooth: 0,
            pe_level_hold_ui_refresh_count: 0,
            pe_level_hold_cursor_id: 0,
            pe_level_hold_cursor_arg: 0,
            pe_level_hold_ui_wait_4012c1_count: 0,
            pe_spread_hold_ui_refresh_count: 0,
            pe_spread_hold_cursor_id: 0,
            pe_spread_hold_cursor_arg: 0,
            pe_spread_ui_cursor_4519e0_count: 0,
            pe_spread_ui_cursor_4519e0_last_arg: 0,
            pe_spread_ui_wait_4012c1_count: 0,
            pe_raise_hold_ui_refresh_count: 0,
            pe_raise_hold_cursor_id: 0,
            pe_raise_hold_cursor_arg: 0,
            pe_raise_ui_cursor_4519e0_count: 0,
            pe_raise_ui_cursor_4519e0_last_arg: 0,
            pe_raise_ui_wait_4012c1_count: 0,
            pe_raise_midband_refuse_sound_401096_count: 0,
            pe_raise_midband_refuse_dialog_402c4d_count: 0,
            pe_raise_midband_refuse_dialog_402c4d_a: 0,
            pe_raise_midband_refuse_dialog_402c4d_b: 0,
            pe_raise_midband_refuse_dialog_402c4d_c: 0,
            pe_raise_midband_refuse_ui_402900_count: 0,
            pe_raise_midband_refuse_ui_402900_a: 0,
            pe_raise_midband_refuse_ui_402900_b: 0,
            pe_raise_hold_blit_count: 0,
            pe_lower_hold_blit_count: 0,
            pe_process_lfsr,
            engine_fidelity,
            game_clock,
            zone_growth_enabled: false,
            zone_growth_uses_allowlist: true,
            zone_growth_allowed_tiles: vec![false; CITY_TILES],
            trip_served_tiles: vec![false; CITY_TILES],
            bypass_growth_trip_gate: false,
            pe_growth_aggregate: PeGrowthAggregate::default(),
            trip_bus_passengers: 0,
            trip_rail_passengers: 0,
            trip_subway_passengers: 0,
            scenario_start_clock: scenario_start_clock_for_city(&city, game_clock),
            announced_scenario_outcome: None,
            announced_bankruptcy: false,
            edit_map_generation_seed: MacToolboxRng::DEFAULT_SEED,
            edit_map_sliders: ui::EditMapSliderValues::default(),
            edit_map_slider_drag: None,
            mode: AppMode::CityView,
            tool: EditTool::Query,
            open_tool_menu: None,
            hover_tile: None,
            zone_drag: None,
            network_drag: None,
            underground_drag: None,
            highway_drag: None,
            terrain_drag: None,
            edit_map_stretch_drag: None,
            edit_map_lower_drag: None,
            edit_map_level_drag: None,
            nature_drag: None,
            power_plant_dialog: None,
            arcology_dialog: None,
            sign_dialog: None,
            military_base_dialog: None,
            city_info_dialog: None,
            status_panel_origin: None,
            status_panel_grab: None,
            info_panel_origin: None,
            info_panel_scale: 1.0,
            info_panel_drag: None,
            founding_newspaper_pending: false,
            graph_series_visible: [true; GRAPH_HISTORY_TOTAL_SERIES],
            graph_scale: GraphScale::Year1,
            query_line: String::new(),
            query_is_inspect: false,
            query_status_pinned: false,
            sprite_cache: SpriteCache::new(),
            palette_press: None,
            menu_checks: MenuCheckState::default(),
            dark_appearance: false,
            menubar: MenubarUi::new(),
        };
        if state.engine_fidelity.uses_win95() && state.city.kind == CityFileKind::MacCity {
            // CityView is the default app mode, but Mac MISC +0x04 stores the
            // Mac run-state rather than PE's transient editor/game mode.
            // Initialize the selected PE runtime without replacing Mac data.
            persist_pe_city_mode_misc(&mut state.city, state.mode, state.engine_fidelity);
        }
        mac_menu::set_game_speed(state.traffic.speed());
        state.menu_checks.speed = state.traffic.speed();
        state.load_transit_passengers_from_city();
        state.refresh_simulation_service_bits();
        state.city.mark_all_dirty();
        state.refresh_status_line();
        Ok(state)
    }

    pub(super) fn edit_map_option_enabled(&self, offset: usize) -> bool {
        if self.city.kind == CityFileKind::MacCity {
            if self.engine_fidelity.uses_win95() {
                return false;
            }
            return self
                .city
                .mac_state_misc_u32_at(offset)
                .is_some_and(|value| value as u8 != 0);
        }
        self.city
            .misc_u32_at(offset)
            .unwrap_or(CITY_OPTION_DISABLED_VALUE)
            != 0
    }

    pub(super) fn toggle_edit_map_option(&mut self, offset: usize) {
        if self.city.kind == CityFileKind::MacCity {
            if self.engine_fidelity.uses_win95() {
                return;
            }
            let next = if self.edit_map_option_enabled(offset) {
                0
            } else {
                CITY_OPTION_ENABLED_VALUE
            };
            self.city.set_mac_state_misc_u32_at(offset, next);
            return;
        }
        let next = if self.edit_map_option_enabled(offset) {
            CITY_OPTION_DISABLED_VALUE
        } else {
            CITY_OPTION_ENABLED_VALUE
        };
        ensure_budget_misc_defaults_before_extension(&mut self.city);
        self.city.set_misc_u32_at(offset, next);
    }

    pub(super) fn edit_map_slider_values(&self) -> Option<ui::EditMapSliderValues> {
        (self.mode == AppMode::EditNewMap).then_some(self.edit_map_sliders)
    }

    pub(super) fn set_edit_map_slider(&mut self, slider: ui::EditMapSlider, value: u8) {
        self.edit_map_sliders.set(slider, value);
        self.query_is_inspect = false;
        self.query_line = format!(
            "{:3}% {}",
            edit_map_slider_percent(self.edit_map_sliders.value(slider)),
            slider.label()
        );
    }

    pub(super) fn generate_edit_map(&mut self) {
        self.edit_map_generation_seed = self
            .edit_map_generation_seed
            .wrapping_add(EDIT_MAP_GENERATION_SEED_STEP);
        self.generate_edit_map_with_seed(self.edit_map_generation_seed);
    }

    pub(super) fn generate_edit_map_with_seed(&mut self, seed: u32) {
        self.cancel_edit_map_level_drag();
        let coast_enabled = self.edit_map_option_enabled(EDIT_MAP_COAST_MISC_OFFSET);
        let river_enabled = self.edit_map_option_enabled(EDIT_MAP_RIVER_MISC_OFFSET);
        let slider_values = self.edit_map_sliders;
        let hills_setting = usize::from(slider_values.hills.min(ui::EDIT_MAP_SLIDER_MAX));
        let water_setting = usize::from(slider_values.water.min(ui::EDIT_MAP_SLIDER_MAX));
        let trees_setting = usize::from(slider_values.trees.min(ui::EDIT_MAP_SLIDER_MAX));
        if self.engine_fidelity.uses_win95() {
            terrain_generation::generate_win95(
                &mut self.city,
                slider_values,
                coast_enabled,
                river_enabled,
                &mut self.pe_process_crt,
                &mut self.pe_process_game_rand,
            );
        } else {
            let (terrain, altm, tile_bits, buildings, selected_water_height) =
                terrain_generation::generate_with_rng(
                    &mut self.mac_process_rng,
                    coast_enabled,
                    river_enabled,
                    slider_values,
                    &mut self.mac_game_rng,
                );
            self.city
                .replace_edit_map_canvas_with_buildings(terrain, altm, buildings);
            self.city
                .set_mac_selected_water_height(selected_water_height);
            for (index, bits) in tile_bits.into_iter().enumerate() {
                let x = index / CITY_HEIGHT;
                let y = index % CITY_HEIGHT;
                self.city.set_tile_bits(x, y, TileBits::new(bits));
            }
        }
        self.traffic = traffic_system_for_city(&self.city, self.engine_fidelity);
        self.win95_overlay_sim.reset_for_city(&self.city);
        self.reset_traffic_redraw_phase();
        self.invalidate_sprite_cache();
        self.query_is_inspect = false;
        self.query_line = "Map generated".to_owned();
        log::info!(
            "Edit Map: generated terrain (coast={}, river={}, hills={}, water={}, trees={}, seed=0x{seed:08x})",
            coast_enabled,
            river_enabled,
            hills_setting,
            water_setting,
            trees_setting
        );
    }

    pub(super) fn apply_state_app_command(
        &mut self,
        command: &AppCommand,
    ) -> Option<AppCommandStateEffect> {
        let mut effect = AppCommandStateEffect::default();
        match command {
            AppCommand::ToggleGrid => self.show_grid = !self.show_grid,
            AppCommand::ToggleZoneOverlay => {
                self.show_zone_overlay = !self.show_zone_overlay;
            }
            AppCommand::ToggleTraffic => {
                self.show_traffic = !self.show_traffic;
                self.invalidate_sprite_cache();
                effect.log_traffic_stats = true;
            }
            AppCommand::SetDataMapMode(mode) => {
                self.set_data_map_mode(*mode);
                effect.update_title = true;
            }
            AppCommand::ToggleUnderground => self.toggle_underground_view(),
            AppCommand::OpenCityInfo(dialog) => {
                self.open_city_info_dialog(*dialog);
                effect.update_title = true;
            }
            AppCommand::SetSpeed(speed) => {
                // DialogBox disables the native parent speed commands. Do not
                // persist a transient modal speed or replace its entry snapshot.
                if self.pe_dialog_owns_input() {
                    return Some(effect);
                }
                let speed = if self.engine_fidelity == EngineFidelity::Mac68k
                    && *speed == GameSpeed::AfricanSwallow
                {
                    // Mac CODE 4 persists a four-choice selector (Pause through
                    // Cheetah). CityGame's fifth speed has no native Mac value.
                    GameSpeed::Cheetah
                } else {
                    *speed
                };
                self.traffic.set_speed(speed);
                if let Some(value) =
                    speed_misc_value_for_city(&self.city, self.engine_fidelity, speed)
                {
                    persist_speed_misc_value_to_city(&mut self.city, value);
                }
                self.reset_traffic_redraw_phase();
                self.menu_checks.speed = speed;
                mac_menu::set_game_speed(speed);
                self.invalidate_sprite_cache();
                self.query_is_inspect = false;
                self.refresh_status_line();
                effect.update_title = true;
                effect.sync_renderer_to_window = true;
                effect.sync_palette_stacking = true;
            }
            AppCommand::ToggleAutoBudget => {
                self.toggle_auto_budget();
                effect.update_title = true;
            }
            AppCommand::ToggleAutoGoto => {
                self.toggle_auto_goto();
                effect.update_title = true;
            }
            AppCommand::ToggleSoundEffects => {
                self.toggle_sound_effects();
                effect.update_title = true;
            }
            AppCommand::ToggleMusic => {
                self.toggle_music();
                effect.update_title = true;
            }
            AppCommand::ToggleNoDisasters => {
                self.toggle_no_disasters();
                effect.update_title = true;
            }
            AppCommand::ToggleNewspaperSubscription => {
                self.toggle_newspaper_subscription();
                effect.update_title = true;
            }
            AppCommand::ToggleNewspaperExtra => {
                self.toggle_newspaper_extra();
                effect.update_title = true;
            }
            AppCommand::ToggleBalloonHelp => {
                self.toggle_balloon_help();
                effect.update_title = true;
            }
            AppCommand::StartDisaster(kind) => {
                let target = self.disaster_target_tile();
                let started = self.start_disaster_at(*kind, target);
                effect.disaster_log = Some((*kind, started));
                if started {
                    effect.update_title = true;
                    effect.placement_sound = Some(disaster_start_sound(*kind));
                    if self.auto_goto_enabled {
                        // Prefer the post-seed pending tile (microwave walk uses process CRT).
                        effect.auto_goto_tile = self
                            .take_pending_disaster_auto_goto()
                            .or_else(|| pe_disaster_auto_goto_tile(&self.city, *kind, target))
                            .or(Some(target));
                    }
                    self.invalidate_sprite_cache();
                }
            }
            AppCommand::SetTool(tool) => {
                self.set_tool_and_log(*tool);
                effect.update_title = true;
            }
            AppCommand::RotateView(delta) => self.rotate_view(*delta),
            _ => return None,
        }
        Some(effect)
    }

    pub(super) fn sync_edit_map_menu(&mut self) {
        self.menu_checks.edit_map = self.mode == AppMode::EditNewMap;
        self.menu_checks.data_map_mode = self.data_map_mode;
        self.menu_checks.show_underground = self.show_underground;
        self.menu_checks.auto_budget =
            auto_budget_enabled_for_engine(&self.city, self.engine_fidelity);
        self.menu_checks.auto_goto = self.auto_goto_enabled;
        self.menu_checks.sound_effects = self.sound_effects_enabled;
        self.menu_checks.music = self.music_enabled;
        self.menu_checks.no_disasters = self.no_disasters_enabled;
        self.menu_checks.newspaper_subscription = self.engine_fidelity.uses_win95()
            && newspaper_subscription_enabled_for_city(&self.city);
        self.menu_checks.newspaper_extra =
            self.engine_fidelity.uses_win95() && newspaper_extra_enabled_for_city(&self.city);
        self.menu_checks.balloon_help = self.balloon_help_enabled;
        self.menu_checks.speed = self.traffic.speed();
        mac_menu::set_edit_map_mode(self.menu_checks.edit_map);
        mac_menu::set_data_map_mode(self.menu_checks.data_map_mode);
        mac_menu::set_underground_view(self.menu_checks.show_underground);
        mac_menu::set_auto_budget(self.menu_checks.auto_budget);
        mac_menu::set_auto_goto(self.menu_checks.auto_goto);
        mac_menu::set_sound_effects(self.menu_checks.sound_effects);
        mac_menu::set_music(self.menu_checks.music);
        mac_menu::set_no_disasters(self.menu_checks.no_disasters);
        mac_menu::set_newspaper_subscription(self.menu_checks.newspaper_subscription);
        mac_menu::set_newspaper_extra(self.menu_checks.newspaper_extra);
        mac_menu::set_balloon_help(self.menu_checks.balloon_help);
    }

    pub(super) fn sync_menu_graphics_preference(&mut self, prefer_replacement: bool) {
        self.menu_checks.prefer_replacement_graphics = prefer_replacement;
        mac_menu::set_graphics_preference(prefer_replacement);
    }

    pub(super) fn sync_menu_citygame_overrides(
        &mut self,
        effects: bool,
        music: bool,
        city_music: bool,
        toolbar: bool,
    ) {
        self.menu_checks.citygame_effects = effects;
        self.menu_checks.citygame_music = music;
        self.menu_checks.citygame_city_music = city_music;
        self.menu_checks.citygame_toolbar = toolbar;
        mac_menu::set_citygame_overrides(effects, music, city_music, toolbar);
    }

    pub(super) fn sync_menu_citygame_newspaper(&mut self, enabled: bool) {
        self.menu_checks.citygame_newspaper = enabled;
        mac_menu::set_citygame_newspaper(enabled);
        if !enabled && matches!(&self.city_info_dialog, Some(CityInfoDialog::Newspaper(_))) {
            self.close_city_info_dialog();
        }
    }

    pub(super) fn toggle_auto_budget(&mut self) {
        let next = !auto_budget_enabled_for_engine(&self.city, self.engine_fidelity);
        let changed =
            set_auto_budget_enabled_for_engine(&mut self.city, self.engine_fidelity, next);
        if changed {
            // PE 0x4101a0 toggles only the option/profile. It does not rescale
            // service funding, rebuild currents or accrue another month.
            self.sync_edit_map_menu();
            self.refresh_status_line();
            self.city.mark_all_dirty();
        }
    }

    pub(super) fn toggle_auto_goto(&mut self) {
        self.auto_goto_enabled = !self.auto_goto_enabled;
        set_auto_goto_enabled_for_engine(
            &mut self.city,
            self.engine_fidelity,
            self.auto_goto_enabled,
        );
        self.sync_edit_map_menu();
        self.refresh_status_line();
    }

    pub(super) fn toggle_sound_effects(&mut self) {
        self.sound_effects_enabled = !self.sound_effects_enabled;
        set_sound_effects_enabled_for_engine(
            &mut self.city,
            self.engine_fidelity,
            self.sound_effects_enabled,
        );
        self.sync_edit_map_menu();
        self.refresh_status_line();
    }

    pub(super) fn toggle_music(&mut self) {
        self.music_enabled = !self.music_enabled;
        set_music_enabled_for_engine(&mut self.city, self.engine_fidelity, self.music_enabled);
        self.sync_edit_map_menu();
        self.refresh_status_line();
    }

    pub(super) fn toggle_no_disasters(&mut self) {
        self.no_disasters_enabled = !self.no_disasters_enabled;
        // This is a shared saved option: Mac CODE 4 stores it at slot 1024,
        // MISC +0x1000, and the PE layout uses the same option word.
        set_no_disasters_enabled_for_engine(
            &mut self.city,
            self.engine_fidelity,
            self.no_disasters_enabled,
        );
        self.sync_edit_map_menu();
        self.refresh_status_line();
    }

    pub(super) fn toggle_newspaper_subscription(&mut self) {
        if !self.engine_fidelity.uses_win95() || self.city.kind == CityFileKind::MacCity {
            return;
        }
        let next = !newspaper_subscription_enabled_for_city(&self.city);
        if set_city_newspaper_subscription_enabled_with_crt(
            &mut self.city,
            next,
            Some(&mut self.pe_process_crt),
        ) {
            self.sync_edit_map_menu();
            self.refresh_status_line();
            self.city.mark_all_dirty();
        }
    }

    pub(super) fn toggle_newspaper_extra(&mut self) {
        if !self.engine_fidelity.uses_win95() {
            return;
        }
        let next = !newspaper_extra_enabled_for_city(&self.city);
        if set_city_newspaper_extra_enabled_with_crt(
            &mut self.city,
            next,
            Some(&mut self.pe_process_crt),
        ) {
            self.sync_edit_map_menu();
            self.refresh_status_line();
            self.city.mark_all_dirty();
        }
    }

    pub(super) fn toggle_balloon_help(&mut self) {
        self.balloon_help_enabled = !self.balloon_help_enabled;
        if !self.balloon_help_enabled {
            self.balloon_help_line = None;
        }
        self.sync_edit_map_menu();
        self.refresh_status_line();
    }

    pub(super) fn set_palette_balloon_help(&mut self, label: Option<&'static str>) -> bool {
        let next = if self.balloon_help_enabled {
            label
        } else {
            None
        };
        if self.balloon_help_line == next {
            return false;
        }
        self.balloon_help_line = next;
        true
    }

    pub(super) fn reset_zone_growth_for_city(&mut self) {
        // Win95's recovered monthly growth lattice scans every map tile; the
        // native executable has no player-session allowlist. Keep Mac's
        // synthetic/incomplete growth path disabled until its native caller
        // and whole-month behavior are implemented.
        self.zone_growth_enabled = self.engine_fidelity.uses_win95();
        self.zone_growth_uses_allowlist = !self.engine_fidelity.uses_win95();
        if self.zone_growth_allowed_tiles.len() == CITY_TILES {
            self.zone_growth_allowed_tiles.fill(false);
        } else {
            self.zone_growth_allowed_tiles = vec![false; CITY_TILES];
        }
    }

    pub(super) fn update_window_title(&self, video: &sdl3::VideoSubsystem, window_id: u32) {
        set_window_title(video, window_id, &self.window_title_base());
    }

    pub(super) fn new_city(&mut self) -> Result<()> {
        self.new_city_with_setup(None, None, None, true)?;
        self.reseed_new_city_technology_dates();
        Ok(())
    }

    pub(super) fn new_city_from_title(
        &mut self,
        city_name: String,
        starting_funds: i32,
        starting_year: u16,
        difficulty: u16,
        generation_seed: u32,
        start_flow: title_screen::NewCityStartFlow,
    ) -> Result<()> {
        self.new_city_with_setup(
            Some(starting_funds),
            Some(starting_year),
            Some(difficulty),
            start_flow == title_screen::NewCityStartFlow::GenerateAndPlay,
        )?;
        let city_name = normalize_new_city_name(&city_name);
        self.city.name.clone_from(&city_name);
        self.city.path = new_city_path_for_name(&self.city.path, &city_name);
        let pe_generated_city = self.engine_fidelity.uses_win95()
            && start_flow == title_screen::NewCityStartFlow::GenerateAndPlay;
        if pe_generated_city {
            // PE `FUN_0040E250` seeds the four neighboring economies during
            // map reset, before `FUN_00410950` consumes the terrain CRT stream.
            self.city
                .pe_initialize_new_city_neighbors_40e250(&mut self.pe_process_crt);
        } else if self.engine_fidelity.uses_win95() {
            self.city
                .pe_reseed_neighbor_fame_ranks_42e585(&mut self.pe_process_crt);
        } else {
            self.reseed_new_city_technology_dates();
        }
        self.apply_new_city_start_flow(start_flow, generation_seed);
        if pe_generated_city {
            // PE `FUN_0042E460` reseeds invention/fame ranks after terrain setup.
            self.city
                .pe_reseed_neighbor_fame_ranks_42e585(&mut self.pe_process_crt);
        }
        self.maybe_show_founding_newspaper();
        Ok(())
    }

    fn reseed_new_city_technology_dates(&mut self) {
        if self.engine_fidelity.uses_win95() {
            self.city
                .pe_reseed_neighbor_fame_ranks_42e585(&mut self.pe_process_crt);
        } else {
            let starting_year = self.city.saved_date().map(|date| date.year).unwrap_or(1900);
            self.city
                .mac_reseed_technology_dates_00041564(starting_year, &mut self.mac_process_rng);
        }
    }

    pub(super) fn apply_new_city_start_flow(
        &mut self,
        start_flow: title_screen::NewCityStartFlow,
        generation_seed: u32,
    ) {
        ensure_budget_misc_defaults_before_extension(&mut self.city);
        self.city
            .set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
        self.city
            .set_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
        self.edit_map_generation_seed = generation_seed;
        self.generate_edit_map_with_seed(generation_seed);
        // Both original engines reset XPOP/XPLC/XFIR to zero and XROG to 0x7f.
        // Replacing the terrain canvas clears optional microsim planes, so restore
        // those native reset values after generation.
        initialize_new_city_microsim_planes(&mut self.city);
        if start_flow == title_screen::NewCityStartFlow::GenerateAndPlay {
            self.mode = AppMode::CityView;
            persist_app_mode_transition(&mut self.city, self.mode, self.engine_fidelity);
            self.tool = EditTool::Query;
            self.show_grid = false;
            self.show_traffic = true;
            self.show_zone_overlay = false;
            self.show_underground = false;
            self.data_map_mode = CityDataMapMode::City;
            self.sync_edit_map_menu();
        }
    }

    pub(super) fn new_city_with_setup(
        &mut self,
        starting_funds: Option<i32>,
        starting_year: Option<u16>,
        difficulty: Option<u16>,
        seed_new_city_budget_rows: bool,
    ) -> Result<()> {
        let configured_path = paths::default_city_path();
        let path = configured_path
            .parent()
            .filter(|parent| parent.is_dir())
            .map(|parent| parent.join("Untitled.SC2"))
            .unwrap_or_else(|| crate::asset_profile::cities_directory().join("Untitled.SC2"));
        fs::create_dir_all(path.parent().context("new city path has no parent")?)
            .with_context(|| format!("creating city directory for {}", path.display()))?;
        self.cancel_edit_map_level_drag();
        self.close_other_modal_dialogs();
        self.close_city_info_dialog();
        self.query_is_inspect = false;
        self.query_status_pinned = false;
        self.query_line.clear();
        self.city = City::new_blank(&path, "Untitled")?;
        // A new city resets its compass. Reset the camera with it: inheriting the
        // previous city's camera would rotate positions but leave wire art and
        // picking/placement orientation on the new city's zero compass.
        self.camera_rotation = self.city.rotation();
        self.mac_kind2_sound_deadline = 0;
        if !self.engine_fidelity.uses_win95() {
            self.city.prepare_mac_fidelity_from_saved_city();
            self.city.initialize_mac_fidelity_blank_xbld_census();
        }
        self.founding_newspaper_pending = true;
        if seed_new_city_budget_rows {
            initialize_new_city_budget_rows(&mut self.city, self.engine_fidelity);
        }
        apply_new_city_title_selections(&mut self.city, starting_funds, starting_year, difficulty);
        if self.engine_fidelity.uses_win95() {
            self.city.apply_default_new_city_technology_schedule();
        }
        ensure_default_city_options_for_engine(
            &mut self.city,
            self.engine_fidelity,
            Some(&mut self.pe_process_crt),
        );
        initialize_mac_fidelity_new_city_industry_rows(&mut self.city, self.engine_fidelity);
        initialize_original_new_city_national_misc(&mut self.city);
        initialize_new_city_microsim_planes(&mut self.city);
        if self.engine_fidelity.uses_win95() {
            self.city.initialize_pe_new_city_microsims();
        } else {
            self.city.initialize_mac_new_city_microsims();
        }
        initialize_hard_start_bond(&mut self.city);
        self.auto_goto_enabled = auto_goto_enabled_for_engine(&self.city, self.engine_fidelity);
        self.pending_disaster_auto_goto = None;
        self.pending_disaster_start_sound = None;
        self.pending_disaster_start_siren = false;
        self.pending_disaster_extra_sounds.clear();
        self.earthquake_shake_remaining = 0;
        self.pe_disaster_duration = 0;
        self.pe_hurricane_wind_remaining = 0;
        self.sound_effects_enabled =
            sound_effects_enabled_for_engine(&self.city, self.engine_fidelity);
        self.music_enabled = music_enabled_for_engine(&self.city, self.engine_fidelity);
        self.no_disasters_enabled =
            no_disasters_enabled_for_engine(&self.city, self.engine_fidelity);
        self.reset_zone_growth_for_city();
        self.balloon_help_enabled = false;
        self.balloon_help_line = None;
        self.traffic = traffic_system_for_city(&self.city, self.engine_fidelity);
        self.reset_traffic_redraw_phase();
        self.win95_overlay_sim.reset_for_city(&self.city);
        self.edit_map_generation_seed = MacToolboxRng::DEFAULT_SEED;
        self.edit_map_sliders = ui::EditMapSliderValues::default();
        self.edit_map_slider_drag = None;
        self.mode = AppMode::EditNewMap;
        persist_app_mode_transition(&mut self.city, self.mode, self.engine_fidelity);
        self.tool = EditTool::RaiseTerrain;
        self.show_grid = true;
        self.show_traffic = false;
        self.show_zone_overlay = false;
        self.show_underground = false;
        self.data_map_mode = CityDataMapMode::City;
        self.sync_edit_map_menu();
        self.invalidate_sprite_cache();
        self.city.mark_all_dirty();
        log::info!(
            "Edit Map: generated terrain canvas (base height {BLANK_MAP_BASE_HEIGHT}). \
             Shift+= raise, Shift+- lower, B water, drag to sculpt. \
             Load an existing .sc2 via File → Load City for a built city."
        );
        Ok(())
    }

    pub(super) fn toggle_edit_mode(&mut self, video: &sdl3::VideoSubsystem, window_id: u32) {
        self.cancel_edit_map_level_drag();
        self.mode = match self.mode {
            AppMode::CityView => {
                self.tool = EditTool::RaiseTerrain;
                self.show_grid = true;
                self.show_traffic = false;
                self.show_zone_overlay = false;
                self.show_underground = false;
                self.data_map_mode = CityDataMapMode::City;
                self.reset_traffic_redraw_phase();
                log::info!(
                    "Edit Map: terrain only. Shift+= raise, Shift+- lower, B bulldoze. Drag to paint."
                );
                AppMode::EditNewMap
            }
            AppMode::EditNewMap => {
                self.tool = EditTool::Query;
                self.show_traffic = true;
                self.show_underground = false;
                self.reset_traffic_redraw_phase();
                self.refresh_status_line();
                log::info!(
                    "City View: buildings visible. R road, B bulldoze, right-drag to paint."
                );
                AppMode::CityView
            }
        };
        persist_app_mode_transition(&mut self.city, self.mode, self.engine_fidelity);
        self.update_window_title(video, window_id);
        self.sync_edit_map_menu();
        self.maybe_show_founding_newspaper();
        log::info!("mode: {:?}", self.mode);
    }

    pub(super) fn finish_edit_map_state(&mut self) {
        self.cancel_edit_map_level_drag();
        self.mode = AppMode::CityView;
        persist_app_mode_transition(&mut self.city, self.mode, self.engine_fidelity);
        self.tool = EditTool::Query;
        self.show_traffic = true;
        self.show_underground = false;
        self.reset_traffic_redraw_phase();
        self.refresh_status_line();
        self.sync_edit_map_menu();
        self.maybe_show_founding_newspaper();
        log::info!("City View: buildings visible.");
    }

    pub(super) fn finish_edit_map(&mut self, video: &sdl3::VideoSubsystem, window_id: u32) {
        self.finish_edit_map_state();
        self.update_window_title(video, window_id);
    }

    pub(super) fn save_city(
        &mut self,
        path: &Path,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> Result<()> {
        self.city
            .sync_traffic_from_slice(self.traffic.congestion_slice());
        self.persist_saved_pe_view(window_size, pixel_scale);
        self.city.save(path)
    }

    /// Serialize a snapshot for a provider without changing the active city's
    /// path, modified flag, or dirty render tiles if the user later cancels.
    #[cfg(any(target_os = "ios", test))]
    pub(super) fn city_save_snapshot(&self, window_size: (u32, u32), pixel_scale: f32) -> City {
        let mut snapshot = self.city.clone();
        snapshot.sync_traffic_from_slice(self.traffic.congestion_slice());
        if let Some(view) = self.saved_pe_view_for_window(window_size, pixel_scale) {
            snapshot.set_saved_view(view);
        }
        snapshot
    }

    pub(super) fn load_city(
        &mut self,
        path: &Path,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> Result<()> {
        let mut city =
            City::load(path).with_context(|| format!("loading city {}", path.display()))?;
        if self.engine_fidelity.uses_win95() {
            city.prepare_pe_fidelity_from_saved_city();
            // The Win95 engine owns this count regardless of the saved-city layout.
            pe_rebuild_w_city_developed_tiles_46ab56(&mut city);
            city.hydrate_pe_attitude_scores_from_xgrp();
        } else {
            city.prepare_mac_fidelity_from_saved_city();
        }
        city.set_pe_scenario_active(
            self.engine_fidelity.uses_win95() && city.kind == CityFileKind::WinScenario,
        );
        let game_clock = game_clock_from_city(&city);
        // The Win95 CRT `rand()` state is per-thread process state, seeded during
        // startup before title interaction. Opening a different city does not
        // restart that stream; city-derived initialization here would rewind it.
        let mut pe_process_crt = if self.engine_fidelity.uses_win95() {
            self.pe_process_crt
        } else {
            init_pe_process_crt(&city, game_clock)
        };
        ensure_default_city_options_for_engine(
            &mut city,
            self.engine_fidelity,
            Some(&mut pe_process_crt),
        );
        // PE Game_Random reads/writes process-global state at `0x4CDB7C`, which
        // is absent from city saves. No city-open reset has been established;
        // preserve the live stream instead of inventing one from the new city.
        let pe_process_lfsr = if self.engine_fidelity.uses_win95() {
            self.pe_process_lfsr
        } else {
            init_pe_process_lfsr(&city, game_clock)
        };
        // Mac export 201's app-owned A5-8 stream is not city state; leave
        // `mac_game_rng` untouched across an ordinary city open as well.
        // PE scenario open @ `0x482fba` → `FUN_00484cd0` national-from-fame join.
        if city.kind == CityFileKind::WinScenario && self.engine_fidelity.uses_win95() {
            city.pe_publish_graph_calendar_words_from_city_age_41354e();
            city.pe_fun_00484cd0_national_from_fame(&mut pe_process_crt);
        }
        city.clear_legacy_dispatch_actor_proxies();
        log::info!(
            "loaded city {} from {}{} (compass={}, camera_rotation={}, saved_rotation={}, saved_view={:?})",
            city.name,
            path.display(),
            if city.has_traffic_density_map() {
                " (XTRF traffic density)"
            } else {
                " (no XTRF — heuristic traffic)"
            },
            city.rotation(),
            city.rotation(),
            city.saved_rotation(),
            city.saved_view(),
        );
        self.close_tool_menu();
        self.close_modal_dialogs();
        self.cancel_edit_map_level_drag();
        self.cancel_pe_suspended_city_work();
        self.city = city.clone();
        self.auto_goto_enabled = auto_goto_enabled_for_engine(&city, self.engine_fidelity);
        self.pending_disaster_auto_goto = None;
        self.pending_disaster_start_sound = None;
        self.pending_disaster_start_siren = false;
        self.pending_disaster_extra_sounds.clear();
        self.earthquake_shake_remaining = 0;
        self.pe_disaster_duration = 0;
        self.pe_hurricane_wind_remaining = 0;
        self.sound_effects_enabled = sound_effects_enabled_for_engine(&city, self.engine_fidelity);
        self.music_enabled = music_enabled_for_engine(&city, self.engine_fidelity);
        self.no_disasters_enabled = no_disasters_enabled_for_engine(&city, self.engine_fidelity);
        self.reset_zone_growth_for_city();
        self.balloon_help_enabled = false;
        self.balloon_help_line = None;
        self.camera_rotation = city.rotation();
        self.traffic = traffic_system_for_city(&city, self.engine_fidelity);
        self.reset_traffic_redraw_phase();
        self.win95_overlay_sim.reset_for_city(&city);
        self.game_clock = game_clock;
        self.pe_process_crt = pe_process_crt;
        self.pe_process_lfsr = pe_process_lfsr;
        self.bypass_growth_trip_gate = false;
        self.load_transit_passengers_from_city();
        self.scenario_start_clock = scenario_start_clock_for_city(&city, self.game_clock);
        self.announced_scenario_outcome = None;
        self.announced_bankruptcy = false;
        self.query_is_inspect = false;
        self.mode = AppMode::CityView;
        persist_app_mode_transition(&mut self.city, self.mode, self.engine_fidelity);
        self.tool = EditTool::Query;
        self.show_traffic = true;
        self.show_underground = false;
        self.city_info_dialog = loaded_city_info_dialog_for_city(&city);
        self.refresh_simulation_service_bits();
        self.refresh_status_line();
        self.seed_loaded_scenario_disaster();
        self.consume_pe_trigger_disaster();
        self.sync_edit_map_menu();
        mac_menu::set_game_speed(self.traffic.speed());
        self.invalidate_sprite_cache();
        self.city.mark_all_dirty();
        self.fit_map(window_size, pixel_scale);
        self.restore_saved_pe_view(window_size, pixel_scale);
        if let Some((x, y)) = self.take_pending_disaster_auto_goto() {
            self.center_tile_at_viewport(x, y, window_size, pixel_scale);
        }
        Ok(())
    }
}
