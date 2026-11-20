//! Title-screen event handling and transition into a running city session.
//!
//! The existing event/command order and local title-music lifetime are preserved.
//! SDL windows, the GPU surface, and the audio subsystem remain runtime-owned.

use super::startup_assets::{choose_folder_blocking, reload_title_assets};
use super::{
    AppCommand, ViewerState, command_queue, input_pixel_scale, open_title_city_dialog,
    open_title_scenario_dialog, push_command, raise_window, set_window_title, show_window,
    sync_renderer_to_window, title_screen, wait_for_window_pixels,
};
use crate::asset_profile::{
    AssetProfile, GraphicsPreference, OriginalMedia, TitleMusicPreference, ToolbarPalettePreference,
};
use crate::city::City;
use crate::engine_fidelity::EngineFidelity;
use crate::mac_menu;
use crate::mac_random::{MacAppLfsrRng, MacGameLcgRng, MacToolboxRng};
use crate::renderer::Renderer;
use anyhow::Result;
use glam::Vec2;
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::Keycode;
use sdl3::mouse::{MouseButton, MouseWheelDirection};
use std::time::{Duration, Instant};

const MAC_STARTUP_SONGS: [u16; 5] = [10001, 10004, 10008, 10012, 10018];

pub(super) fn mac_tick_count() -> u32 {
    (u128::from(sdl3::timer::ticks()) * 60 / 1000) as u32
}

pub(super) struct MacTitleStartup {
    app_lfsr: MacAppLfsrRng,
    game: MacGameLcgRng,
    quickdraw: MacToolboxRng,
    pub(super) song: u16,
    paper_tick: Option<i16>,
    palette: Option<super::mac_loop_scheduler::palette::MacPaletteSchedule>,
}

impl MacTitleStartup {
    pub(super) fn new(mut tick: impl FnMut() -> u32) -> Self {
        let app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::startup_seed(tick()));
        let mut game = MacGameLcgRng::new(tick() | 1);
        let quickdraw = MacToolboxRng::new(tick());
        let song = MAC_STARTUP_SONGS[usize::from(game.bounded(5))];
        Self {
            app_lfsr,
            game,
            quickdraw,
            song,
            paper_tick: None,
            palette: None,
        }
    }

    pub(super) fn initialize_palette(&mut self, tick: impl FnMut() -> u32) {
        self.palette =
            Some(super::mac_loop_scheduler::palette::MacPaletteSchedule::initialize(tick));
    }

    fn advance_palette(&mut self, tick: impl FnMut() -> u32) {
        if let Some(palette) = &mut self.palette {
            // The native title loop uses speed 3 and has a front window.
            palette.update(3, tick, || true);
        }
    }

    pub(super) fn initialize_newspaper(&mut self, tick: u32) {
        super::session::initialize_mac_title_newspaper(&mut self.quickdraw, tick);
        self.paper_tick = Some(tick as i16);
    }

    pub(super) fn install(self, state: &mut ViewerState) {
        state.mac_app_lfsr = self.app_lfsr;
        state.mac_game_rng = self.game;
        state.mac_process_rng = self.quickdraw;
        state.mac_newspaper.tick_word = self.paper_tick;
        if let Some(palette) = self.palette {
            state.mac_loop_scheduler.install_live_palette(palette);
        }
    }
}

pub(super) struct MacMusicSelector {
    deadline: u32,
    recent: [u16; 4],
    // The pinned Quadra's caller frames live in the 0x0076xxxx region.
    // Other Mac memory layouts can yield a different signed high word.
    caller_frame_high: i16,
}

impl Default for MacMusicSelector {
    fn default() -> Self {
        Self {
            deadline: 0,
            recent: [0; 4],
            caller_frame_high: 0x76,
        }
    }
}

impl MacMusicSelector {
    fn select_with_playback<C>(
        &mut self,
        game: &mut MacGameLcgRng,
        music_enabled: bool,
        now: u32,
        playback: &mut C,
        song_active: impl FnOnce(&mut C) -> bool,
        request_and_tick: impl FnOnce(&mut C, u16) -> u32,
    ) -> Option<u16> {
        const SONG_OFFSETS: [u16; 9] = [0, 3, 4, 7, 8, 11, 12, 13, 18];
        if !music_enabled || now < self.deadline {
            return None;
        }
        // CODE 5 draws before querying playback and recent-song history.
        let index = usize::from(game.bounded(9));
        let offset = SONG_OFFSETS[index];
        if song_active(playback) || self.recent.contains(&offset) {
            return None;
        }
        self.recent.rotate_left(1);
        self.recent[3] = offset;
        // Sparse offsets 11..13 read the adjacent song-choice table; offset
        // 18 reads the signed high word of the caller's saved frame pointer.
        const SONG_DELAYS: [i16; 8] = [9_000, 11_000, 13_000, 9_000, 9_000, 4, 7, 8];
        let delay = SONG_DELAYS
            .get(index)
            .copied()
            .unwrap_or(self.caller_frame_high);
        let song = 10_000 + offset;
        self.deadline = request_and_tick(playback, song).wrapping_add_signed(i32::from(delay));
        Some(song)
    }

    #[cfg(test)]
    fn select(
        &mut self,
        game: &mut MacGameLcgRng,
        music_enabled: bool,
        now: u32,
        mut active: bool,
        request_and_tick: impl FnOnce(u16) -> u32,
    ) -> Option<u16> {
        self.select_with_playback(
            game,
            music_enabled,
            now,
            &mut active,
            |active| *active,
            |_, song| request_and_tick(song),
        )
    }
}

impl ViewerState {
    pub(super) fn mac_music_callback_with_playback<C>(
        &mut self,
        now: u32,
        playback: &mut C,
        song_active: impl FnOnce(&mut C) -> bool,
        request_and_tick: impl FnOnce(&mut C, u16) -> u32,
    ) -> Option<u16> {
        if self.engine_fidelity.uses_win95() {
            return None;
        }
        self.mac_music.select_with_playback(
            &mut self.mac_game_rng,
            self.music_enabled,
            now,
            playback,
            song_active,
            request_and_tick,
        )
    }

    #[cfg(test)]
    pub(super) fn mac_music_callback_with_request(
        &mut self,
        now: u32,
        mut active: bool,
        request_and_tick: impl FnOnce(u16) -> u32,
    ) -> Option<u16> {
        self.mac_music_callback_with_playback(
            now,
            &mut active,
            |active| *active,
            |_, song| request_and_tick(song),
        )
    }

    #[cfg(test)]
    pub(super) fn mac_music_callback(&mut self, now: u32, song_active: bool) -> Option<u16> {
        self.mac_music_callback_with_request(now, song_active, |_| now)
    }
}

pub(super) fn run_title_screen(
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    event_pump: &mut sdl3::EventPump,
    renderer: &mut Renderer<'_>,
    profile: &mut AssetProfile,
    audio: Option<&sdl3::AudioSubsystem>,
    fidelity: EngineFidelity,
    mut startup: Option<&mut MacTitleStartup>,
    mut resumed_mac_scheduler: Option<&mut super::mac_loop_scheduler::MacLoopScheduler>,
    continued_music: &mut Option<crate::title_music::TitleMusic>,
) -> Result<bool> {
    use title_screen::{Choice, Page, TitleScreen};

    let window_id = window.id();
    set_window_title(video, window_id, "CityGame");
    show_window(video, window_id);
    raise_window(video, window_id);
    window.sync();
    wait_for_window_pixels(window);
    sync_renderer_to_window(renderer, window);

    let mut screen = TitleScreen::new(profile);
    mac_menu::set_citygame_overrides(
        profile.citygame_effects_enabled(),
        profile.title_music_preference() == TitleMusicPreference::Replacement,
        profile.citygame_city_music_enabled(),
        profile.toolbar_palette_preference().uses_citygame_palette(),
    );
    let startup_song = startup.as_ref().map(|startup| startup.song);
    let start_music = |profile: &AssetProfile| {
        let original_mac = !fidelity.uses_win95()
            && profile.title_music_preference() == TitleMusicPreference::Original;
        crate::title_music::TitleMusic::new(
            audio,
            profile,
            startup_song,
            !original_mac,
            original_mac,
        )
    };
    let mut music = start_music(profile);
    if let Some(startup) = startup.as_mut() {
        startup.initialize_newspaper(mac_tick_count());
        startup.initialize_palette(mac_tick_count);
    }
    log::info!(
        "CityGame title screen shown (music: {:?})",
        profile.title_music_preference()
    );
    let started = Instant::now();
    let mut running = true;
    let mut enter_game = false;
    let mut city_name_keyboard = false;

    while running && !enter_game {
        let frame_start = Instant::now();
        if let Some(startup) = startup.as_mut() {
            startup.advance_palette(mac_tick_count);
        }
        if let Some(scheduler) = resumed_mac_scheduler.as_mut() {
            scheduler.advance_palette(3, scheduler.current_tick());
        }
        mac_menu::pump_events();
        event_pump.pump_events();
        let appearance = {
            let mut queue = command_queue().lock().expect("command queue");
            let mut pending_appearance = None;
            let mut deferred = std::collections::VecDeque::new();
            for command in queue.drain(..) {
                match command {
                    AppCommand::SetAppearance(preference) => pending_appearance = Some(preference),
                    other => deferred.push_back(other),
                }
            }
            *queue = deferred;
            pending_appearance
        };
        if let Some(preference) = appearance {
            match profile.set_appearance_preference(preference) {
                Ok(()) => {
                    let dark = preference.is_dark(matches!(
                        sdl3::VideoSubsystem::get_system_theme(),
                        sdl3::video::SystemTheme::Dark
                    ));
                    renderer.set_dark_appearance(dark);
                    mac_menu::set_appearance_preference(preference);
                    screen.status = Some(format!("Appearance: {preference:?}"));
                }
                Err(error) => screen.status = Some(format!("Appearance setting failed: {error}")),
            }
        }
        let events: Vec<_> = event_pump.poll_iter().collect();
        for event in events {
            #[cfg(target_os = "ios")]
            if crate::ios_documents::is_presenting()
                && !matches!(&event, Event::Window { .. } | Event::Quit { .. })
            {
                continue;
            }
            match event {
                Event::Quit { .. } => running = false,
                Event::Window {
                    window_id: event_window,
                    win_event: WindowEvent::CloseRequested,
                    ..
                } if event_window == window_id => running = false,
                Event::Window {
                    window_id: event_window,
                    win_event:
                        WindowEvent::Resized(_, _)
                        | WindowEvent::PixelSizeChanged(_, _)
                        | WindowEvent::Maximized
                        | WindowEvent::Restored
                        | WindowEvent::Exposed,
                    ..
                } if event_window == window_id => sync_renderer_to_window(renderer, window),
                Event::KeyDown {
                    window_id: event_window,
                    keycode: Some(Keycode::Escape),
                    ..
                } if event_window == window_id => {
                    if screen.page == Page::Home {
                        running = false;
                    } else {
                        screen.back();
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Backspace),
                    ..
                } if screen.page == Page::NewCity => {
                    screen.backspace_city_name();
                }
                Event::TextInput { text, .. } if screen.page == Page::NewCity => {
                    screen.append_city_name(&text);
                }
                Event::MouseWheel {
                    window_id: event_window,
                    y,
                    direction,
                    ..
                } if event_window == window_id && screen.page == Page::Scenarios => {
                    let delta = match direction {
                        MouseWheelDirection::Flipped => -y.round() as i32,
                        MouseWheelDirection::Normal | MouseWheelDirection::Unknown(_) => {
                            y.round() as i32
                        }
                    };
                    screen.scroll_scenarios(delta);
                }
                Event::MouseButtonDown {
                    window_id: event_window,
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } if event_window == window_id => {
                    let point = Vec2::new(x, y) * input_pixel_scale(window, renderer.size());
                    if screen.page == Page::NewCity
                        && TitleScreen::point_in_new_city_name_field(
                            renderer.size(),
                            point.x,
                            point.y,
                        )
                        && !video.text_input().is_active(window)
                    {
                        city_name_keyboard = false;
                    }
                    let Some(choice) = screen.hit_test(point.x, point.y) else {
                        continue;
                    };
                    log::info!("CityGame title choice: {choice:?}");
                    match choice {
                        Choice::Continue => {
                            if let Some(path) = profile.recent_city() {
                                push_command(AppCommand::OpenTitleDialogPath(path));
                                enter_game = true;
                            }
                        }
                        Choice::NewCity => {
                            screen.refresh_generation_seed();
                            screen.set_page(Page::NewCity);
                        }
                        Choice::EditMap => {
                            screen.refresh_generation_seed();
                            let (_, difficulty, starting_funds) = title_screen::DIFFICULTIES[0];
                            let starting_year = title_screen::YEARS[0];
                            push_command(AppCommand::NewCityFromTitle {
                                city_name: screen.city_name.clone(),
                                starting_funds,
                                starting_year,
                                difficulty,
                                generation_seed: screen.generation_seed,
                                start_flow: title_screen::NewCityStartFlow::GenerateAndEdit,
                            });
                            enter_game = true;
                        }
                        Choice::LoadCity => {
                            open_title_city_dialog(window);
                        }
                        Choice::Scenarios => screen.set_page(Page::Scenarios),
                        Choice::Graphics => screen.set_page(Page::Graphics),
                        Choice::CycleMusic => {
                            let preference = profile.title_music_preference().next();
                            match profile.set_title_music_preference(preference) {
                                Ok(()) => {
                                    music = start_music(profile);
                                    mac_menu::set_citygame_overrides(
                                        profile.citygame_effects_enabled(),
                                        profile.title_music_preference()
                                            == TitleMusicPreference::Replacement,
                                        profile.citygame_city_music_enabled(),
                                        profile
                                            .toolbar_palette_preference()
                                            .uses_citygame_palette(),
                                    );
                                    screen.status = None;
                                }
                                Err(error) => {
                                    screen.status = Some(format!("Music setting failed: {error}"))
                                }
                            }
                        }
                        Choice::Quit => running = false,
                        Choice::StartGenerated | Choice::StartEditing => {
                            let (_, difficulty, starting_funds) = title_screen::DIFFICULTIES
                                .get(screen.difficulty_index)
                                .copied()
                                .unwrap_or(title_screen::DIFFICULTIES[0]);
                            let starting_year = title_screen::YEARS
                                .get(screen.year_index)
                                .copied()
                                .unwrap_or(title_screen::YEARS[0]);
                            push_command(AppCommand::NewCityFromTitle {
                                city_name: screen.city_name.clone(),
                                starting_funds,
                                starting_year,
                                difficulty,
                                generation_seed: screen.generation_seed,
                                start_flow: choice
                                    .new_city_start_flow()
                                    .expect("new-city start choice has a flow"),
                            });
                            enter_game = true;
                        }
                        Choice::CycleDifficulty => {
                            screen.cycle_difficulty();
                        }
                        Choice::CycleYear => {
                            screen.year_index = (screen.year_index + 1) % title_screen::YEARS.len();
                        }
                        Choice::SelectScenario(index) => screen.scenario_index = index,
                        Choice::PlayScenario => {
                            if let Some(entry) = screen.scenarios.get(screen.scenario_index) {
                                match entry.mac_action {
                                    Some(action) if super::mac_loop_scheduler::scenario_action::mac_scenario_action_supported(action) => {
                                        push_command(AppCommand::StartMacScenario(
                                            entry.path.clone(),
                                        ));
                                        enter_game = true;
                                    }
                                    Some(_) => {
                                        screen.status = Some("This Mac scenario's opening event is not implemented yet.".into());
                                    }
                                    None => {
                                        push_command(AppCommand::OpenTitleScenarioPath(
                                            entry.path.clone(),
                                        ));
                                        enter_game = true;
                                    }
                                }
                            }
                        }
                        Choice::BrowseScenario => {
                            open_title_scenario_dialog(window, profile);
                        }
                        Choice::ImportOriginal
                        | Choice::ChooseOriginal
                        | Choice::ChooseReplacement
                        | Choice::ChooseTileSet => {
                            let purpose = match choice {
                                Choice::ImportOriginal => "Import Original Game Files",
                                Choice::ChooseOriginal => "Choose Extracted Original Pack",
                                Choice::ChooseReplacement => "Choose Our Graphics Pack",
                                Choice::ChooseTileSet => "Choose Alternate Tile Set",
                                _ => unreachable!(),
                            };
                            match choose_folder_blocking(window, event_pump) {
                                Ok(Some(path)) => {
                                    let changed = match choice {
                                        Choice::ImportOriginal => {
                                            profile.import_original(&path).map(|_| ())
                                        }
                                        Choice::ChooseOriginal => {
                                            profile.set_original_pack(&path).map(|_| ())
                                        }
                                        Choice::ChooseReplacement => {
                                            profile.set_replacement_pack(&path)
                                        }
                                        Choice::ChooseTileSet => profile.set_tile_set_pack(&path),
                                        _ => unreachable!(),
                                    };
                                    match changed.and_then(|()| {
                                        reload_title_assets(profile, renderer).map(|_| ())
                                    }) {
                                        Ok(()) => {
                                            music = start_music(profile);
                                            if choice == Choice::ImportOriginal {
                                                screen.refresh_scenarios(profile);
                                            }
                                            screen.status = Some(format!("{purpose} updated"));
                                        }
                                        Err(error) => {
                                            screen.status =
                                                Some(format!("{purpose} failed: {error}"))
                                        }
                                    }
                                }
                                Ok(None) => {}
                                Err(error) => {
                                    #[cfg(any(target_os = "android", target_os = "ios"))]
                                    {
                                        let _ = error;
                                        screen.status = Some(
                                            "Folder picker unsupported on this device — reinstall from Mac with SeedAssets (scripts/build_ios_host.sh or build_android_host.sh)"
                                                .into(),
                                        );
                                    }
                                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                                    {
                                        screen.status =
                                            Some(format!("Folder picker failed: {error}"));
                                    }
                                }
                            }
                        }
                        Choice::CycleGraphicsSource
                        | Choice::CycleEffectsSource
                        | Choice::CycleMusicSource => {
                            let media = match choice {
                                Choice::CycleGraphicsSource => OriginalMedia::Graphics,
                                Choice::CycleEffectsSource => OriginalMedia::Effects,
                                _ => OriginalMedia::Music,
                            };
                            let result = profile.cycle_source(media).and_then(|selected| {
                                if media == OriginalMedia::Graphics {
                                    reload_title_assets(profile, renderer)?;
                                }
                                Ok(selected)
                            });
                            match result {
                                Ok(Some(path)) => {
                                    if media == OriginalMedia::Music {
                                        music = start_music(profile);
                                    }
                                    screen.status = Some(format!(
                                        "Selected {}",
                                        path.file_name().unwrap_or_default().to_string_lossy()
                                    ));
                                }
                                Ok(None) => {
                                    screen.status =
                                        Some("Import an edition with this media first".into())
                                }
                                Err(error) => {
                                    screen.status =
                                        Some(format!("Could not change source: {error}"))
                                }
                            }
                        }
                        Choice::ToggleCityGameEffects => {
                            match profile
                                .set_citygame_effects_enabled(!profile.citygame_effects_enabled())
                            {
                                Ok(()) => {
                                    mac_menu::set_citygame_overrides(
                                        profile.citygame_effects_enabled(),
                                        profile.title_music_preference()
                                            == TitleMusicPreference::Replacement,
                                        profile.citygame_city_music_enabled(),
                                        profile
                                            .toolbar_palette_preference()
                                            .uses_citygame_palette(),
                                    );
                                    screen.status = Some("Sound preference updated".into());
                                }
                                Err(error) => {
                                    screen.status =
                                        Some(format!("Could not change sounds: {error}"))
                                }
                            }
                        }
                        Choice::ToggleCityGameMusic => {
                            let next = if profile.title_music_preference()
                                == TitleMusicPreference::Replacement
                            {
                                TitleMusicPreference::Original
                            } else {
                                TitleMusicPreference::Replacement
                            };
                            match profile.set_title_music_preference(next) {
                                Ok(()) => {
                                    music = start_music(profile);
                                    mac_menu::set_citygame_overrides(
                                        profile.citygame_effects_enabled(),
                                        profile.title_music_preference()
                                            == TitleMusicPreference::Replacement,
                                        profile.citygame_city_music_enabled(),
                                        profile
                                            .toolbar_palette_preference()
                                            .uses_citygame_palette(),
                                    );
                                    screen.status = Some("Music preference updated".into());
                                }
                                Err(error) => {
                                    screen.status = Some(format!("Could not change music: {error}"))
                                }
                            }
                        }
                        Choice::ToggleCityGameCityMusic => {
                            let enabled = !profile.citygame_city_music_enabled();
                            match profile.set_citygame_city_music_enabled(enabled) {
                                Ok(()) => {
                                    mac_menu::set_citygame_overrides(
                                        profile.citygame_effects_enabled(),
                                        profile.title_music_preference()
                                            == TitleMusicPreference::Replacement,
                                        enabled,
                                        profile
                                            .toolbar_palette_preference()
                                            .uses_citygame_palette(),
                                    );
                                    screen.status = Some(format!(
                                        "CityGame city music: {}",
                                        if enabled { "ON" } else { "OFF" }
                                    ));
                                }
                                Err(error) => {
                                    screen.status =
                                        Some(format!("City music setting failed: {error}"))
                                }
                            }
                        }
                        Choice::ToggleToolbarPalette => {
                            let preference = profile.toolbar_palette_preference().next();
                            match profile.set_toolbar_palette_preference(preference) {
                                Ok(()) => {
                                    renderer.set_citygame_toolbar_palette(
                                        preference.uses_citygame_palette(),
                                    );
                                    mac_menu::set_citygame_overrides(
                                        profile.citygame_effects_enabled(),
                                        profile.title_music_preference()
                                            == TitleMusicPreference::Replacement,
                                        profile.citygame_city_music_enabled(),
                                        preference.uses_citygame_palette(),
                                    );
                                    screen.status = Some(match preference {
                                        ToolbarPalettePreference::CityGame => {
                                            "CityGame toolbar palette enabled".into()
                                        }
                                        ToolbarPalettePreference::Classic => {
                                            "Classic toolbar palette enabled".into()
                                        }
                                    });
                                }
                                Err(error) => {
                                    screen.status =
                                        Some(format!("Could not change toolbar: {error}"))
                                }
                            }
                        }
                        Choice::ClearTileSet => match profile.clear_tile_set_pack() {
                            Ok(()) => match reload_title_assets(profile, renderer) {
                                Ok(_) => {
                                    screen.status = Some("Default tile set restored".to_string())
                                }
                                Err(error) => {
                                    screen.status =
                                        Some(format!("Could not reload graphics: {error}"))
                                }
                            },
                            Err(error) => {
                                screen.status = Some(format!("Could not clear tile set: {error}"))
                            }
                        },
                        Choice::PreferOriginal | Choice::PreferOurs => {
                            let preference = if choice == Choice::PreferOurs {
                                GraphicsPreference::PreferReplacement
                            } else {
                                GraphicsPreference::PreferOriginal
                            };
                            match profile
                                .set_preference(preference)
                                .and_then(|()| reload_title_assets(profile, renderer).map(|_| ()))
                            {
                                Ok(()) => {
                                    mac_menu::set_graphics_preference(
                                        preference.prefers_replacement(),
                                    );
                                    screen.status = Some(if preference.prefers_replacement() {
                                        "Our replacement graphics are preferred".to_string()
                                    } else {
                                        "Original graphics are preferred".to_string()
                                    });
                                }
                                Err(error) => {
                                    screen.status =
                                        Some(format!("Could not switch graphics: {error}"))
                                }
                            }
                        }
                        Choice::Back => screen.back(),
                    }
                }
                _ => {}
            }
        }

        let text_input = video.text_input();
        if screen.page == Page::NewCity {
            if !city_name_keyboard {
                let field = TitleScreen::new_city_name_field_px(renderer.size());
                let (x, y, w, h) =
                    TitleScreen::text_input_area_points(field, renderer.size(), window.size());
                text_input.set_rect(window, sdl3::rect::Rect::new(x, y, w, h), 0);
                text_input.start(window);
                city_name_keyboard = true;
            }
        } else if city_name_keyboard {
            text_input.stop(window);
            city_name_keyboard = false;
        }

        if let Some(command) = take_title_open_path() {
            let path = match &command {
                AppCommand::OpenTitlePath(path)
                | AppCommand::OpenTitleDialogPath(path)
                | AppCommand::OpenTitleScenarioPath(path) => path,
                _ => unreachable!(),
            };
            match City::load(&path) {
                Ok(_) => {
                    // The title has no city to replace; the runtime must not
                    // ask to save the placeholder city for a Finder open.
                    push_command(approve_title_open(command));
                    enter_game = true;
                }
                Err(error) => {
                    if fidelity.uses_win95() {
                        crate::win95_file_error::record_city_io_error(path, &error);
                    }
                    if is_win95_title_format_error(fidelity, &command, &error) {
                        super::runtime::show_win95_city_open_errors(window, profile, &error, true);
                    } else {
                        screen.status = Some(format!("Could not open city: {error:#}"));
                    }
                }
            }
        }

        if !running || enter_game {
            break;
        }
        if let Some(music) = music.as_mut() {
            music.update();
        }
        let size = renderer.size();
        let solids = screen.render(size, started.elapsed().as_secs_f32(), profile);
        if let Err(error) =
            renderer.render_with_clear_color(&[], &solids, &[], [0.035, 0.075, 0.12, 1.0])
        {
            log::error!("title screen render failed: {error:#}");
        }
        window.sync();
        let frame_time = Duration::from_millis(16);
        if let Some(remaining) = frame_time.checked_sub(frame_start.elapsed()) {
            std::thread::sleep(remaining);
        }
    }

    let text_input = video.text_input();
    if text_input.is_active(window) {
        text_input.stop(window);
    }
    if running
        && enter_game
        && !fidelity.uses_win95()
        && profile.title_music_preference() == TitleMusicPreference::Original
        && !profile.citygame_city_music_enabled()
    {
        *continued_music = music.take();
    }
    drop(music);
    log::info!("CityGame title screen finished (enter_game={enter_game})");
    Ok(running && enter_game)
}

fn is_win95_title_format_error(
    fidelity: EngineFidelity,
    command: &AppCommand,
    error: &anyhow::Error,
) -> bool {
    cfg!(not(any(target_os = "android", target_os = "ios")))
        && fidelity.uses_win95()
        && matches!(command, AppCommand::OpenTitleDialogPath(_))
        && (error
            .downcast_ref::<crate::city::NotIffCityFile>()
            .is_some()
            || error
                .downcast_ref::<crate::city::WrongIffCityType>()
                .is_some()
            || error.downcast_ref::<crate::city::CityReadEof>().is_some())
}

pub(super) fn take_title_open_path() -> Option<AppCommand> {
    let mut queue = command_queue().lock().expect("command queue");
    take_title_open_command(&mut queue)
}

fn approve_title_open(command: AppCommand) -> AppCommand {
    match command {
        AppCommand::OpenTitlePath(path) => AppCommand::OpenConfirmedCityPath(path),
        other => other,
    }
}

fn take_title_open_command(
    queue: &mut std::collections::VecDeque<AppCommand>,
) -> Option<AppCommand> {
    let index = queue.iter().position(|command| {
        matches!(
            command,
            AppCommand::OpenTitlePath(_)
                | AppCommand::OpenTitleDialogPath(_)
                | AppCommand::OpenTitleScenarioPath(_)
        )
    })?;
    queue.remove(index)
}

#[cfg(test)]
mod title_open_tests {
    use super::*;

    #[test]
    fn mac_music_queries_playback_only_after_deadline_and_rng() {
        #[derive(Default)]
        struct Playback {
            queries: usize,
            requests: usize,
        }
        let mut playback = Playback::default();
        let mut selector = MacMusicSelector::default();
        let mut rng = MacGameLcgRng::new(1);
        let before = rng.state();
        let query = |playback: &mut Playback| {
            playback.queries += 1;
            false
        };
        let request = |playback: &mut Playback, _: u16| {
            playback.requests += 1;
            1000
        };
        assert_eq!(
            selector.select_with_playback(&mut rng, false, 1000, &mut playback, query, request),
            None
        );
        selector.deadline = 1001;
        assert_eq!(
            selector.select_with_playback(&mut rng, true, 1000, &mut playback, query, request),
            None
        );
        assert_eq!(
            (playback.queries, playback.requests, rng.state()),
            (0, 0, before)
        );

        // Choice zero is already in the recent list; CODE 5 still queries
        // playback after drawing and before rejecting the recent song.
        let seed = (1..100_000)
            .find(|&seed| MacGameLcgRng::new(seed).bounded(9) == 0)
            .unwrap();
        rng = MacGameLcgRng::new(seed);
        let mut expected = MacGameLcgRng::new(seed);
        expected.bounded(9);
        assert_eq!(
            selector.select_with_playback(&mut rng, true, 1001, &mut playback, query, request),
            None
        );
        assert_eq!(
            (playback.queries, playback.requests, rng.state()),
            (1, 0, expected.state())
        );
    }

    #[test]
    fn mac_music_uses_adjacent_choice_words_and_post_request_tick() {
        for (index, song, delay) in [(5, 10011, 4), (6, 10012, 7), (7, 10013, 8)] {
            let seed = (1..100_000)
                .find(|&seed| usize::from(MacGameLcgRng::new(seed).bounded(9)) == index)
                .expect("a seed for the song choice");
            let mut rng = MacGameLcgRng::new(seed);
            let mut selector = MacMusicSelector::default();
            let mut requested = None;
            assert_eq!(
                selector.select(&mut rng, true, 1000, false, |selected| {
                    requested = Some(selected);
                    1004
                }),
                Some(song)
            );
            assert_eq!(requested, Some(song));
            assert_eq!(selector.deadline, 1004 + delay);
            let mut called = false;
            assert_eq!(
                selector.select(&mut rng, true, selector.deadline - 1, false, |_| {
                    called = true;
                    0
                }),
                None
            );
            assert!(!called);
        }
    }

    #[test]
    fn mac_music_last_delay_reads_signed_caller_frame_word() {
        let seed = (1..100_000)
            .find(|&seed| MacGameLcgRng::new(seed).bounded(9) == 8)
            .expect("a seed for song 10018");
        for high_word in [0x76, 0x56, 0x8123u16 as i16] {
            let mut selector = MacMusicSelector {
                caller_frame_high: high_word,
                ..Default::default()
            };
            let mut rng = MacGameLcgRng::new(seed);
            assert_eq!(
                selector.select(&mut rng, true, 1000, false, |_| 1004),
                Some(10018)
            );
            assert_eq!(
                selector.deadline,
                1004u32.wrapping_add_signed(i32::from(high_word))
            );
        }
    }

    #[test]
    fn win95_title_chooser_recognizes_both_format_errors() {
        let path = std::path::Path::new("fixture.sc2");
        let dialog = AppCommand::OpenTitleDialogPath(path.into());
        let external = AppCommand::OpenTitlePath(path.into());
        let scenario = AppCommand::OpenTitleScenarioPath(path.into());
        for bytes in [&b"not an IFF"[..], &b"FORM\0\0\0\x04ILBM"[..], &b""[..], &b"FORM"[..], &b"FORM\0\0\0\x04"[..]] {
            let error = crate::city::detect_city_kind(path, bytes).unwrap_err();
            assert_eq!(
                is_win95_title_format_error(EngineFidelity::Win95, &dialog, &error),
                cfg!(not(any(target_os = "android", target_os = "ios")))
            );
            assert!(!is_win95_title_format_error(
                EngineFidelity::Mac68k,
                &dialog,
                &error
            ));
            assert!(!is_win95_title_format_error(
                EngineFidelity::Win95,
                &external,
                &error
            ));
            assert!(!is_win95_title_format_error(
                EngineFidelity::Win95,
                &scenario,
                &error
            ));
        }
        let truncated = crate::city::detect_city_kind(path, b"FOR").unwrap_err();
        assert!(!is_win95_title_format_error(
            EngineFidelity::Win95,
            &dialog,
            &truncated
        ));
    }

    #[test]
    fn title_open_queue_preserves_dialog_and_external_origins() {
        let ordinary = AppCommand::OpenPath("ordinary.sc2".into());
        let external = AppCommand::OpenTitlePath("external.sc2".into());
        let external_next = AppCommand::OpenTitlePath("external-next.sc2".into());
        assert_eq!(
            approve_title_open(external.clone()),
            AppCommand::OpenConfirmedCityPath("external.sc2".into())
        );
        let dialog = AppCommand::OpenTitleDialogPath("dialog.sc2".into());
        let scenario = AppCommand::OpenTitleScenarioPath("flint.scn".into());
        let mut queue = std::collections::VecDeque::from([
            ordinary.clone(),
            external.clone(),
            external_next.clone(),
            dialog.clone(),
            scenario.clone(),
        ]);
        assert_eq!(take_title_open_command(&mut queue), Some(external));
        assert_eq!(take_title_open_command(&mut queue), Some(external_next));
        assert_eq!(take_title_open_command(&mut queue), Some(dialog));
        assert_eq!(take_title_open_command(&mut queue), Some(scenario));
        assert_eq!(queue.pop_front(), Some(ordinary));
        assert_eq!(take_title_open_command(&mut queue), None);
    }
}
