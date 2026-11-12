//! Title-screen event handling and transition into a running city session.
//!
//! The existing event/command order and local title-music lifetime are preserved.
//! SDL windows, the GPU surface, and the audio subsystem remain runtime-owned.

use super::startup_assets::{choose_folder_blocking, reload_title_assets};
use super::{
    AppCommand, command_queue, input_pixel_scale, open_title_city_dialog,
    open_title_scenario_dialog, push_command, raise_window, set_window_title, show_window,
    sync_renderer_to_window, title_screen, wait_for_window_pixels,
};
use crate::asset_profile::{
    AssetProfile, GraphicsPreference, OriginalMedia, TitleMusicPreference, ToolbarPalettePreference,
};
use crate::city::City;
use crate::mac_menu;
use crate::renderer::Renderer;
use anyhow::Result;
use glam::Vec2;
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::Keycode;
use sdl3::mouse::{MouseButton, MouseWheelDirection};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub(super) fn run_title_screen(
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    event_pump: &mut sdl3::EventPump,
    renderer: &mut Renderer<'_>,
    profile: &mut AssetProfile,
    audio: Option<&sdl3::AudioSubsystem>,
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
    let mut music = crate::title_music::TitleMusic::new(audio, profile);
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
                                push_command(AppCommand::OpenPath(path));
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
                                    music = crate::title_music::TitleMusic::new(audio, profile);
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
                                        push_command(AppCommand::OpenPath(entry.path.clone()));
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
                                            music =
                                                crate::title_music::TitleMusic::new(audio, profile);
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
                                        music = crate::title_music::TitleMusic::new(audio, profile);
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
                                    music = crate::title_music::TitleMusic::new(audio, profile);
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

        if let Some(path) = take_title_open_path() {
            match City::load(&path) {
                Ok(_) => {
                    push_command(AppCommand::OpenPath(path));
                    enter_game = true;
                }
                Err(error) => {
                    screen.status = Some(format!("Could not open city: {error:#}"));
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
    drop(music);
    log::info!("CityGame title screen finished (enter_game={enter_game})");
    Ok(running && enter_game)
}

pub(super) fn take_title_open_path() -> Option<PathBuf> {
    let mut queue = command_queue().lock().expect("command queue");
    let index = queue
        .iter()
        .position(|command| matches!(command, AppCommand::OpenTitlePath(_)))?;
    match queue.remove(index)? {
        AppCommand::OpenTitlePath(path) => Some(path),
        _ => None,
    }
}
