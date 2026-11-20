//! SDL startup, event dispatch, and presentation for the running application.

#[cfg(any(target_os = "android", target_os = "ios"))]
use super::seed_bundled_assets_if_needed;
use super::title_flow::{MacTitleStartup, mac_tick_count};
use super::{
    AppCommand, Win95CloseAction, Win95FileDialog, AssetFolderPurpose, DisasterEffectPlayback, GameCursors, PlacementSound, ViewerState, WINDOW_HEIGHT,
    WINDOW_WIDTH, app_command_for_menu_command, apply_map_touch_action, arcology_dialog_hit_test,
    auto_budget_enabled_for_engine, bootstrap_window,
    configure_linux_video_driver, disaster_effect_playback, ensure_first_run_assets, fit_window_to_usable_display,
    handle_modal_pointer_down, handle_modal_pointer_up, hide_window, input_pixel_scale,
    load_profile_sound, menu_callback, military_base_dialog_hit_test, open_asset_folder_dialog,
    open_city_dialog, open_scenario_dialog, open_tile_set_dialog, pixel_scale,
    play_placement_sound, point_in_float_rect, pop_command, power_plant_dialog_hit_test,
    push_command, raise_window, reload_asset_profile, run_title_screen, save_city_dialog, save_city_dialog_before_close,
    show_asset_error, show_window, sync_renderer_to_window, sync_sign_text_input, sync_window,
    tool_allows_touch_paint_stroke, viewport_center, wait_for_window_pixels, wheel_zoom_factor,
    win95_file_menu_shortcut, windows_menu_shortcut,
};
use crate::asset_profile::{AssetProfile, OriginalMedia, TitleMusicPreference};
use crate::city::AppMode;
use crate::docked_palette::DockedPalette;
use crate::engine_fidelity::EngineFidelity;
use crate::game_clock::GameClockAdvance;
use crate::mac_menu;
use crate::menubar::{MenubarClick, MenubarUi};
use crate::mobile_lifecycle::AppLifecycle;
use crate::palette_window;
use crate::platform_ui;
use crate::renderer::Renderer;
use crate::touch_input::{self, MapTouchArbiter, TouchGesture, TouchTracker};
use crate::traffic::GameSpeed;
use crate::ui::{self, EditTool, UiAction};
use crate::win95_random::{Win95CrtRand, Win95GameRand, Win95Lfsr};
use anyhow::{Context, Result};
use glam::Vec2;
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::Keycode;
use sdl3::mouse::MouseButton;
use std::time::{Duration, Instant};

/// Seed the two Win95 view-constructor process streams from independent tick
/// reads, in PE order: Game_Random LFSR first, game LCG second.
pub(super) fn win95_view_randoms_from_tick_source(
    mut tick: impl FnMut() -> u64,
) -> (Win95Lfsr, Win95GameRand) {
    let lfsr = Win95Lfsr::from_u32_seed(tick() as u32 | 1);
    let game_rand = Win95GameRand::new(tick() as u32 | 1);
    (lfsr, game_rand)
}

/// The first city view initializes the CRT and newspaper after the title
/// choice. Later city opens continue the resulting process stream.
pub(super) fn win95_document_randoms_from_tick_source(
    tick: impl FnOnce() -> u64,
) -> (i16, Win95CrtRand, crate::city::PeNewspaperTables) {
    let tick_word = tick() as i16;
    let mut crt = Win95CrtRand::new(i32::from(tick_word) as u32);
    let tables = super::newspaper_state::initialize_pe_newspaper_tables(&mut crt);
    (tick_word, crt, tables)
}

mod commands;
pub(super) use commands::show_win95_city_open_errors;
mod input;
mod presentation;

use input::InputState;

/// Borrowed only for a dispatch/presentation call. SDL windows and GPU surfaces
/// stay in `run`, with their original creation and destruction order.
struct RuntimeWindows<'a, 'main, 'palette> {
    video: &'a sdl3::VideoSubsystem,
    window: &'a sdl3::video::Window,
    renderer: &'a mut Renderer<'main>,
    palette_window: Option<&'a sdl3::video::Window>,
    palette_renderer: Option<&'a mut Renderer<'palette>>,
    palette_id: u32,
    palette_logical_size: &'a mut (u32, u32),
    single_window: bool,
}

struct RuntimeAssets<'a> {
    profile: &'a mut AssetProfile,
    root: &'a mut std::path::PathBuf,
    cursors: &'a mut GameCursors,
    sound: &'a mut Option<crate::audio::SoundPlayer>,
    city_music: &'a mut Option<crate::title_music::CityMusic>,
    audio: Option<&'a sdl3::AudioSubsystem>,
}

pub fn run() -> Result<()> {
    #[cfg(target_os = "ios")]
    crate::ios_documents::initialize();
    configure_linux_video_driver();
    // The floating Tools window can be key after a tool selection. Cocoa must
    // deliver the click that refocuses the map so its first build drag starts.
    #[cfg(target_os = "macos")]
    if !sdl3::hint::set("SDL_MOUSE_FOCUS_CLICKTHROUGH", "1") {
        log::warn!("could not enable mouse input on the first window-focus click");
    }
    let sdl = sdl3::init().map_err(|err| anyhow::anyhow!(err))?;
    let video = sdl.video().map_err(|err| anyhow::anyhow!(err))?;
    let audio = sdl.audio().map_err(|err| anyhow::anyhow!(err)).ok();

    mac_menu::install(
        menu_callback,
        super::document_open_callback,
        EngineFidelity::from_env().uses_win95(),
    );

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let mut window = video
        .window("CityGame", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .resizable()
        .high_pixel_density()
        .metal_view()
        .hidden()
        .build()
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    let mut window = video
        .window("CityGame", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .resizable()
        .high_pixel_density()
        .hidden()
        .build()
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;
    crate::app_icon::apply_window_icon(&mut window);
    fit_window_to_usable_display(&video, &mut window);
    // Present before any SDL message box — iOS rejects UIAlertController until the
    // UIKit view is in the window hierarchy (black screen + console warning otherwise).
    show_window(&video, window.id());
    raise_window(&video, window.id());
    window.sync();

    mac_menu::activate_app();

    let mut event_pump = sdl.event_pump().map_err(|err| anyhow::anyhow!(err))?;
    #[cfg(any(target_os = "android", target_os = "ios"))]
    seed_bundled_assets_if_needed();
    let mut asset_profile = AssetProfile::load()?;
    #[cfg(any(target_os = "android", target_os = "ios"))]
    crate::mobile_graphics::apply_mobile_graphics_tier(&mut asset_profile)?;
    ensure_first_run_assets(&window, &mut event_pump, &mut asset_profile)?;
    let mut asset_root = asset_profile
        .original_root()
        .context("no original graphics pack is configured")?;
    let replacement_root = asset_profile.replacement_root();
    let tile_set_root = asset_profile.tile_set_root();
    let prefer_replacement = asset_profile.preference().prefers_replacement();
    mac_menu::set_graphics_preference(prefer_replacement);
    mac_menu::set_appearance_preference(asset_profile.appearance_preference());
    mac_menu::set_citygame_overrides(
        asset_profile.citygame_effects_enabled(),
        asset_profile.title_music_preference() == TitleMusicPreference::Replacement,
        asset_profile.citygame_city_music_enabled(),
        asset_profile
            .toolbar_palette_preference()
            .uses_citygame_palette(),
    );
    log::info!("loading assets from {}", asset_root.display());
    let load_started = Instant::now();
    let mut renderer = Renderer::new_with_library(
        &window,
        &asset_profile.ordered_sources(OriginalMedia::Graphics),
        replacement_root.as_deref(),
        tile_set_root.as_deref(),
        prefer_replacement,
    )?;
    renderer.set_citygame_toolbar_palette(
        asset_profile
            .toolbar_palette_preference()
            .uses_citygame_palette(),
    );
    let initial_dark = asset_profile.appearance_preference().is_dark(matches!(
        sdl3::VideoSubsystem::get_system_theme(),
        sdl3::video::SystemTheme::Dark
    ));
    renderer.set_dark_appearance(initial_dark);
    log::info!(
        "assets ready in {:.1}s",
        load_started.elapsed().as_secs_f32()
    );
    // The PE view seeds these streams before the title menu appears.
    let (view_process_lfsr, view_process_game_rand) = if EngineFidelity::from_env().uses_win95() {
        let (lfsr, game_rand) = win95_view_randoms_from_tick_source(sdl3::timer::ticks);
        (Some(lfsr), Some(game_rand))
    } else {
        (None, None)
    };
    let mut mac_startup =
        (!EngineFidelity::from_env().uses_win95()).then(|| MacTitleStartup::new(mac_tick_count));
    let mut continued_title_music = None;
    if !run_title_screen(
        &video,
        &window,
        &mut event_pump,
        &mut renderer,
        &mut asset_profile,
        audio.as_ref(),
        EngineFidelity::from_env(),
        mac_startup.as_mut(),
        None,
        &mut continued_title_music,
    )? {
        return Ok(());
    }
    // The native view's initial update seeds the CRT only after a title
    // choice. SDL ticks are the portable input for the PE's GetTickCount.
    let (startup_tick_word, startup_process_crt, startup_newspaper) = if EngineFidelity::from_env()
        .uses_win95()
    {
        let (tick_word, crt, tables) = win95_document_randoms_from_tick_source(sdl3::timer::ticks);
        (Some(tick_word), Some(crt), Some(tables))
    } else {
        (None, None, None)
    };
    let mut sound = load_profile_sound(audio.as_ref(), &asset_profile);
    asset_root = asset_profile
        .original_root()
        .context("no original graphics pack is configured")?;
    let replacement_root = asset_profile.replacement_root();
    let tile_set_root = asset_profile.tile_set_root();
    let prefer_replacement = asset_profile.preference().prefers_replacement();
    let palette_logical = ui::palette_logical_size(AppMode::CityView);
    // Keep the tool palette in the game window on desktop as well as tablets.
    // The native macOS menu remains independently controlled by platform_ui.
    let single_window = true;
    let palette_window =
        palette_window::build_palette_window(&video, &window, palette_logical, single_window)?;
    if single_window {
        log::info!("single-window UI: movable in-game Tools palette");
    }
    let mut palette_renderer = if let Some(palette_window) = &palette_window {
        palette_window.sync();
        let mut palette_renderer = Renderer::new_with_library(
            palette_window,
            &asset_profile.ordered_sources(OriginalMedia::Graphics),
            replacement_root.as_deref(),
            tile_set_root.as_deref(),
            prefer_replacement,
        )?;
        palette_renderer.set_citygame_toolbar_palette(
            asset_profile
                .toolbar_palette_preference()
                .uses_citygame_palette(),
        );
        palette_renderer.set_dark_appearance(initial_dark);
        Some(palette_renderer)
    } else {
        None
    };
    let mut cursors = GameCursors::for_assets(&asset_root);
    let mut palette_logical_size = palette_logical;
    let palette_id = palette_window.as_ref().map(|w| w.id()).unwrap_or(u32::MAX);
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        asset_profile.default_city_path()?,
        startup_process_crt,
        view_process_game_rand,
        view_process_lfsr,
        startup_newspaper,
    )?;
    state.pe_document_tick_word = startup_tick_word;
    state.configure_original_newspaper_data(&asset_profile);
    if let Some(startup) = mac_startup {
        startup.install(&mut state);
    }
    let mut city_music = crate::title_music::CityMusic::new(
        audio.as_ref(),
        &asset_profile,
        state.music_enabled,
        asset_profile.citygame_city_music_enabled(),
        !state.engine_fidelity.uses_win95(),
        continued_title_music.take(),
    );
    let mut city_music_enabled = state.music_enabled;
    state.sync_menu_graphics_preference(prefer_replacement);
    state.sync_menu_appearance(asset_profile.appearance_preference());
    state.dark_appearance = initial_dark;
    state.sync_menu_citygame_overrides(
        asset_profile.citygame_effects_enabled(),
        asset_profile.title_music_preference() == TitleMusicPreference::Replacement,
        asset_profile.citygame_city_music_enabled(),
        asset_profile
            .toolbar_palette_preference()
            .uses_citygame_palette(),
    );
    state.sync_menu_citygame_newspaper(asset_profile.citygame_newspaper_enabled());
    let mut input = InputState::new();

    show_window(&video, window.id());
    raise_window(&video, window.id());
    window.sync();
    wait_for_window_pixels(&window);
    if let (Some(palette_window), Some(palette_renderer)) = (&palette_window, &mut palette_renderer)
    {
        show_window(&video, palette_id);
        raise_window(&video, palette_id);
        palette_window::set_app_active(&video, palette_window, true);
        palette_window.sync();
        palette_window::wait_for_pixels(palette_window);
        palette_window::sync_renderer_to_window(palette_renderer, palette_window);
        mac_menu::present_windows(&window, palette_window);
    }
    sync_renderer_to_window(&mut renderer, &window);
    bootstrap_window(&mut renderer, &mut event_pump, &window, &mut state);
    #[cfg(target_os = "macos")]
    state
        .city
        .set_mac_newspaper_suppressed(!mac_menu::app_is_active());
    state.sync_edit_map_menu();
    state.update_window_title(&video, window.id());
    state.log_traffic_stats(&renderer);
    if let Some(atlas) = renderer.atlas_for_sprite(1270) {
        let (resolved_atlas, resolved_id) =
            renderer.resolve_sprite_frame(atlas, 1270, state.animation_frame());
        log::info!(
            "water anim: base=1270 atlas={atlas} frames={} resolved_atlas={resolved_atlas} resolved_id={resolved_id}",
            renderer.animation_frame_count(atlas, 1270),
        );
    }
    // Apply the title choice before the first actor prefix can run on the
    // bootstrap city. The title hands its process RNGs to the chosen city.
    if !state.engine_fidelity.uses_win95() {
        commands::drain_commands(
            RuntimeWindows {
                video: &video,
                window: &window,
                renderer: &mut renderer,
                palette_window: palette_window.as_ref(),
                palette_renderer: palette_renderer.as_mut(),
                palette_id,
                palette_logical_size: &mut palette_logical_size,
                single_window,
            },
            RuntimeAssets {
                profile: &mut asset_profile,
                root: &mut asset_root,
                cursors: &mut cursors,
                sound: &mut sound,
                city_music: &mut city_music,
                audio: audio.as_ref(),
            },
            &mut state,
            &mut input,
        )?;
    }
    let mut last_frame = Instant::now();
    if !state.engine_fidelity.uses_win95() {
        state.mac_loop_scheduler.start_live_clock(mac_tick_count());
    }
    let mut occluded_frames = 0u32;

    loop {
        mac_menu::set_city_runtime_active(true);
        input.running = true;
        let keyboard = sdl.keyboard();
        state.modifiers = keyboard.mod_state();
        input.return_to_title = false;
        while input.running {
            if let Some(player) = &sound {
                player.set_mac_voice_mode(state.engine_fidelity == EngineFidelity::Mac68k);
            }
            state.mac_loop_scheduler.set_viewport_size(renderer.size());
            let run_mac_actor_prefix =
                !state.engine_fidelity.uses_win95() && !input.lifecycle.is_suspended();
            #[cfg(target_os = "ios")]
            let run_mac_actor_prefix =
                run_mac_actor_prefix && !crate::ios_documents::is_presenting();
            let mac_actor_prefix = run_mac_actor_prefix.then(|| {
                let sampled_at = Instant::now();
                let waits = state.begin_mac_frame_actor_phase_with_tick_refresh(
                    sampled_at.duration_since(last_frame),
                    || sampled_at.elapsed(),
                );
                (sampled_at, waits)
            });
            mac_menu::pump_events();
            event_pump.pump_events();
            #[cfg(target_os = "macos")]
            state
                .city
                .set_mac_newspaper_suppressed(!mac_menu::app_is_active());

            for event in event_pump.poll_iter() {
                input::handle_event(
                    event,
                    RuntimeWindows {
                        video: &video,
                        window: &window,
                        renderer: &mut renderer,
                        palette_window: palette_window.as_ref(),
                        palette_renderer: palette_renderer.as_mut(),
                        palette_id,
                        palette_logical_size: &mut palette_logical_size,
                        single_window,
                    },
                    &mut state,
                    &mut input,
                    &mut sound,
                    &mut city_music,
                    &asset_profile,
                )?;
            }

            state.modifiers = keyboard.mod_state();
            input::tick_held_touch(
                RuntimeWindows {
                    video: &video,
                    window: &window,
                    renderer: &mut renderer,
                    palette_window: palette_window.as_ref(),
                    palette_renderer: palette_renderer.as_mut(),
                    palette_id,
                    palette_logical_size: &mut palette_logical_size,
                    single_window,
                },
                &mut state,
                &mut input,
                &sound,
            );

            commands::drain_commands(
                RuntimeWindows {
                    video: &video,
                    window: &window,
                    renderer: &mut renderer,
                    palette_window: palette_window.as_ref(),
                    palette_renderer: palette_renderer.as_mut(),
                    palette_id,
                    palette_logical_size: &mut palette_logical_size,
                    single_window,
                },
                RuntimeAssets {
                    profile: &mut asset_profile,
                    root: &mut asset_root,
                    cursors: &mut cursors,
                    sound: &mut sound,
                    city_music: &mut city_music,
                    audio: audio.as_ref(),
                },
                &mut state,
                &mut input,
            )?;
            #[cfg(target_os = "ios")]
            if crate::ios_documents::is_presenting() {
                // The export snapshot stays current while Files owns the UI.
                last_frame = Instant::now();
                std::thread::sleep(Duration::from_millis(16));
                continue;
            }
            if city_music_enabled != state.music_enabled {
                city_music_enabled = state.music_enabled;
                city_music = crate::title_music::CityMusic::new(
                    audio.as_ref(),
                    &asset_profile,
                    city_music_enabled,
                    asset_profile.citygame_city_music_enabled(),
                    !state.engine_fidelity.uses_win95(),
                    None,
                );
            }
            if let Some(player) = city_music.as_mut() {
                if let Some(id) = state.pending_newspaper_music.take() {
                    player.request_original_song(&asset_profile, id);
                }
                player.update();
            } else {
                state.pending_newspaper_music = None;
            }
            // The sampled editor tools repeat while held, including on a
            // stationary pointer. A frame supplies one native worker sample.
            if !input.lifecycle.is_suspended()
                && window.has_input_focus()
                && state.tick_edit_map_height_hold(
                    input.painting
                        && (event_pump.mouse_state().left() || input.touch.finger_count() > 0),
                )
            {
                state.refresh_status_line();
                state.update_window_title(&video, window.id());
            }
            cursors.set(state.cursor_kind(input.dragging));
            sync_sign_text_input(
                &video,
                &window,
                state.sign_dialog.is_some() || state.win95_new_city_setup.is_some(),
                renderer.size(),
            );

            // The full frame delta drives animation and live traffic. Mac
            // calendar admission receives only the time since the actor phase
            // sampled the clock, including any blocking event work.
            let frame_start = Instant::now();
            let elapsed = if input.lifecycle.is_suspended() {
                // Keep the clock stable while backgrounded; still advance last_frame.
                last_frame = frame_start;
                Duration::ZERO
            } else {
                let e = frame_start.duration_since(last_frame);
                last_frame = frame_start;
                e
            };
            if !input.blocking_file_dialog() && state.traffic.speed() != GameSpeed::Paused {
                state.advance_ambient_animation(elapsed.as_secs_f32());
            }
            let mac_budget_review_was_pending =
                state.mac_loop_scheduler.january_budget_review_pending;
            let mac_fiscal_notice_was_pending = state.mac_loop_scheduler.fiscal_notice.is_some();
            let clock_advance = if input.holds_pe_idle_for_common_dialog() {
                state.advance_pe_common_dialog_timers(elapsed);
                GameClockAdvance::default()
            } else if let Some((sampled_at, waits)) = mac_actor_prefix {
                let event_elapsed = if input.lifecycle.is_suspended() {
                    Duration::ZERO
                } else {
                    frame_start.duration_since(sampled_at)
                };
                state.finish_mac_frame_calendar_phase_with_tick_refresh(
                    event_elapsed,
                    waits,
                    || frame_start.elapsed(),
                )
            } else {
                state.advance_frame_simulation(elapsed)
            };
            if !mac_fiscal_notice_was_pending
                && let Some(notice) = state.mac_loop_scheduler.fiscal_notice.as_mut()
            {
                // The native helper polls _Button even when the press happened
                // before the notice began; it must wait for that release too.
                input.fiscal_notice_mouse_down = event_pump.mouse_state().left();
                notice.button_down = input.fiscal_notice_mouse_down || input.touch.finger_count() > 0;
            }
            if clock_advance.days > 0
                || (!mac_budget_review_was_pending
                    && state.mac_loop_scheduler.january_budget_review_pending)
                || (!mac_fiscal_notice_was_pending
                    && state.mac_loop_scheduler.fiscal_notice.is_some())
            {
                state.update_window_title(&video, window.id());
            }
            // Disaster mode can request a camera move while the calendar is held.
            if let Some((x, y)) = state.take_pending_disaster_auto_goto() {
                state.center_tile_at_viewport(
                    x,
                    y,
                    renderer.size(),
                    pixel_scale(&window, renderer.size()),
                );
            }
            let mac_effect_reset = state.take_mac_disaster_effect_reset();
            if let Some(effect) = state.take_pending_disaster_start_sound() {
                play_placement_sound(state.sound_effects_player(sound.as_ref()), effect);
            }
            if state.take_pending_disaster_start_siren() {
                play_placement_sound(
                    state.sound_effects_player(sound.as_ref()),
                    PlacementSound::Disaster,
                );
            }
            for event in disaster_effect_playback(mac_effect_reset, state.take_pending_disaster_extra_sounds()) {
                match event {
                    DisasterEffectPlayback::StopEffects => {
                        if let Some(player) = &sound { player.stop_effects(); }
                    }
                    DisasterEffectPlayback::Play(effect) => {
                        play_placement_sound(state.sound_effects_player(sound.as_ref()), effect);
                    }
                }
            }
            if !state.query_is_inspect {
                state.refresh_status_line();
            }
            if state.mode == AppMode::CityView && !input.blocking_file_dialog() {
                state
                    .traffic
                    .tick_traffic(&mut state.city, elapsed.as_secs_f32());
                let sim_traffic_redraw =
                    state.advance_static_traffic_redraw_phase(elapsed.as_secs_f32());
                let actor_sim_changed = state.tick_win95_overlay_sim_frame(elapsed.as_secs_f32());
                let cargo_sfx = state.win95_overlay_sim.take_cargo_sfx_plays();
                if cargo_sfx > 0 {
                    state.queue_disaster_extra_sounds(PlacementSound::Cargo, cargo_sfx);
                }
                state.pending_traffic_redraw |= sim_traffic_redraw | actor_sim_changed;
            }

            let dark = asset_profile.appearance_preference().is_dark(matches!(
                sdl3::VideoSubsystem::get_system_theme(),
                sdl3::video::SystemTheme::Dark
            ));
            if state.dark_appearance != dark {
                state.dark_appearance = dark;
                renderer.set_dark_appearance(dark);
                if let Some(palette_renderer) = palette_renderer.as_mut() {
                    palette_renderer.set_dark_appearance(dark);
                }
            }
            presentation::present_frame(
                RuntimeWindows {
                    video: &video,
                    window: &window,
                    renderer: &mut renderer,
                    palette_window: palette_window.as_ref(),
                    palette_renderer: palette_renderer.as_mut(),
                    palette_id,
                    palette_logical_size: &mut palette_logical_size,
                    single_window,
                },
                &mut state,
                &input.docked_palette,
                &mut occluded_frames,
            );
            if state.take_pending_mac_earthquake_sound() && state.sound_effects_enabled {
                play_placement_sound(
                    state.sound_effects_player(sound.as_ref()),
                    PlacementSound::Explode,
                );
            }
            state.tick_earthquake_shake();
            sync_window(&video, window.id());

            // Cap to ~60 fps based on the actual work done this frame (not the sim delta).
            let frame_time = Duration::from_millis(16);
            let work = frame_start.elapsed();
            if work < frame_time {
                std::thread::sleep(frame_time - work);
            }
        }

        mac_menu::set_city_runtime_active(false);
        if !input.return_to_title {
            break;
        }

        // Stop city playback before the title screen starts its own music. Keep the
        // SDL windows and GPU surfaces alive so returning to a city is immediate.
        city_music = None;
        if let Some(palette_window) = &palette_window {
            palette_window::set_app_active(&video, palette_window, false);
            hide_window(&video, palette_id);
        }
        let title_fidelity = state.engine_fidelity;
        let resumed_mac_scheduler =
            (!state.engine_fidelity.uses_win95()).then_some(&mut state.mac_loop_scheduler);
        if !run_title_screen(
            &video,
            &window,
            &mut event_pump,
            &mut renderer,
            &mut asset_profile,
            audio.as_ref(),
            title_fidelity,
            None,
            resumed_mac_scheduler,
            &mut continued_title_music,
        )? {
            break;
        }

        // Title preferences may have changed while the city was suspended. Reload
        // both renderers and audio from that profile before applying its city choice.
        asset_root = reload_asset_profile(
            &asset_profile,
            &mut renderer,
            palette_renderer.as_mut(),
            &mut cursors,
        )?;
        state.configure_original_newspaper_data(&asset_profile);
        let citygame_palette = asset_profile
            .toolbar_palette_preference()
            .uses_citygame_palette();
        renderer.set_citygame_toolbar_palette(citygame_palette);
        let dark = asset_profile.appearance_preference().is_dark(matches!(
            sdl3::VideoSubsystem::get_system_theme(),
            sdl3::video::SystemTheme::Dark
        ));
        renderer.set_dark_appearance(dark);
        if let Some(palette_renderer) = palette_renderer.as_mut() {
            palette_renderer.set_citygame_toolbar_palette(citygame_palette);
            palette_renderer.set_dark_appearance(dark);
        }
        state.sync_menu_graphics_preference(asset_profile.preference().prefers_replacement());
        state.sync_menu_appearance(asset_profile.appearance_preference());
        state.dark_appearance = dark;
        state.sync_menu_citygame_overrides(
            asset_profile.citygame_effects_enabled(),
            asset_profile.title_music_preference() == TitleMusicPreference::Replacement,
            asset_profile.citygame_city_music_enabled(),
            citygame_palette,
        );
        state.sync_menu_citygame_newspaper(asset_profile.citygame_newspaper_enabled());
        sound = load_profile_sound(audio.as_ref(), &asset_profile);
        city_music_enabled = state.music_enabled;
        city_music = crate::title_music::CityMusic::new(
            audio.as_ref(),
            &asset_profile,
            city_music_enabled,
            asset_profile.citygame_city_music_enabled(),
            !state.engine_fidelity.uses_win95(),
            continued_title_music.take(),
        );

        input = InputState::new();
        sync_renderer_to_window(&mut renderer, &window);
        bootstrap_window(&mut renderer, &mut event_pump, &window, &mut state);
        #[cfg(target_os = "macos")]
        state
            .city
            .set_mac_newspaper_suppressed(!mac_menu::app_is_active());
        commands::drain_commands(
            RuntimeWindows {
                video: &video,
                window: &window,
                renderer: &mut renderer,
                palette_window: palette_window.as_ref(),
                palette_renderer: palette_renderer.as_mut(),
                palette_id,
                palette_logical_size: &mut palette_logical_size,
                single_window,
            },
            RuntimeAssets {
                profile: &mut asset_profile,
                root: &mut asset_root,
                cursors: &mut cursors,
                sound: &mut sound,
                city_music: &mut city_music,
                audio: audio.as_ref(),
            },
            &mut state,
            &mut input,
        )?;
        if let (Some(palette_window), Some(palette_renderer)) =
            (&palette_window, &mut palette_renderer)
        {
            show_window(&video, palette_id);
            raise_window(&video, palette_id);
            palette_window::set_app_active(&video, palette_window, true);
            palette_window.sync();
            palette_window::wait_for_pixels(palette_window);
            palette_window::sync_renderer_to_window(palette_renderer, palette_window);
            mac_menu::present_windows(&window, palette_window);
        }
        state.sync_edit_map_menu();
        state.update_window_title(&video, window.id());
        state.log_traffic_stats(&renderer);
        last_frame = Instant::now();
        occluded_frames = 0;
    }

    Ok(())
}

#[cfg(test)]
mod startup_random_tests {
    use super::{win95_document_randoms_from_tick_source, win95_view_randoms_from_tick_source};

    #[test]
    fn win95_view_and_first_document_use_separate_clock_reads() {
        let mut view_ticks = [0x10987_u64, 0x10988].into_iter();
        let (lfsr, game_rand) = win95_view_randoms_from_tick_source(|| {
            view_ticks.next().expect("unexpected view clock read")
        });
        assert_eq!(lfsr.state(), 0x0987);
        assert_eq!(game_rand.state(), 0x10989);
        assert_eq!(view_ticks.next(), None);

        // The low WORD is signed before srand; the initializer then consumes
        // the native newspaper shuffle's 122 CRT draws.
        let (tick_word, crt, _) = win95_document_randoms_from_tick_source(|| 0x1bb7d);
        assert_eq!(tick_word, -17539);
        assert_eq!(crt.seed(), 1_467_602_999);
        assert_eq!(lfsr.state(), 0x0987);
        assert_eq!(game_rand.state(), 0x10989);
    }
}
