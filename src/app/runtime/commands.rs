//! Queued application commands and their platform/asset effects.

use super::*;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use sdl3::messagebox::{ButtonData, ClickedButton, MessageBoxButtonFlag, MessageBoxFlag};

pub(super) fn drain_commands(
    mut windows: RuntimeWindows<'_, '_, '_>,
    assets: RuntimeAssets<'_>,
    state: &mut ViewerState,
    input: &mut InputState,
) -> Result<()> {
    let video = windows.video;
    let window = windows.window;
    let renderer = &mut *windows.renderer;
    let palette_window = windows.palette_window;
    let mut palette_renderer = windows.palette_renderer.as_deref_mut();
    let palette_id = windows.palette_id;
    let palette_logical_size = &mut *windows.palette_logical_size;
    let asset_profile = assets.profile;
    let asset_root = assets.root;
    let cursors = assets.cursors;
    let sound = assets.sound;
    let city_music = assets.city_music;
    let audio = assets.audio;
    let mut deferred = std::collections::VecDeque::new();
    while let Some(command) = pop_command() {
        if input.blocking_file_dialog()
            && !matches!(
                command,
                AppCommand::Quit
                    | AppCommand::CloseSaveDialogResult(_)
                    | AppCommand::Win95FileDialogResult(_, _)
            )
        {
            deferred.push_back(command);
            continue;
        }
        if input.blocking_file_dialog() && matches!(command, AppCommand::Quit) {
            continue;
        }
        if state.mac_loop_scheduler.fiscal_notice.is_some()
            && input::mac_fiscal_notice_defers_command(&command)
        {
            deferred.push_back(command);
            continue;
        }
        if input::handle_mac_fiscal_notice_command(state, &command) {
            continue;
        }
        let (command, close_approved) = match command {
            AppCommand::ContinueAfterClose(action) => (action.command(), true),
            other => (other, false),
        };
        if !close_approved
            && cfg!(not(any(target_os = "android", target_os = "ios")))
            && state.engine_fidelity.uses_win95()
            && let Some(action) = Win95CloseAction::from_command(&command)
            && win95_close_action_needs_prompt(&action, state.mode)
        {
            if action == Win95CloseAction::Quit {
                // SDL can report one close through both app and window events.
                super::super::command_bus::command_queue()
                    .lock()
                    .expect("command queue")
                    .retain(|queued| !matches!(queued, AppCommand::Quit));
            }
            match confirm_win95_close(window, &state.city.name) {
                Win95CloseChoice::Cancel => continue,
                Win95CloseChoice::Discard => {}
                Win95CloseChoice::Save => {
                    if state.city.has_backing_file() {
                        let size = renderer.size();
                        if let Err(error) = state.save_current_city(size, pixel_scale(window, size))
                        {
                            show_win95_save_failure(window, asset_profile, &error);
                        } else {
                            show_win95_save_success(window, asset_profile, &state.city.path);
                        }
                    } else {
                        if save_city_dialog_before_close(window, state.city.kind, &state.city.name)
                        {
                            input.close_save_pending = Some(action);
                            continue;
                        }
                    }
                }
            }
        }
        if matches!(&command, AppCommand::ReturnToTitle) {
            if confirm_return_to_title(window) {
                input.return_to_title = true;
                input.running = false;
                break;
            }
            continue;
        }
        if let Some(effect) = state.apply_state_app_command(&command) {
            if let Some(effect) = effect.placement_sound {
                let _ = state.take_pending_disaster_start_sound();
                play_placement_sound(state.sound_effects_player(sound.as_ref()), effect);
                if state.take_pending_disaster_start_siren() {
                    play_placement_sound(
                        state.sound_effects_player(sound.as_ref()),
                        PlacementSound::Disaster,
                    );
                }
            }
            if let Some((x, y)) = effect.auto_goto_tile {
                let _ = state.take_pending_disaster_auto_goto();
                state.center_tile_at_viewport(
                    x,
                    y,
                    renderer.size(),
                    pixel_scale(window, renderer.size()),
                );
            }
            if effect.sync_renderer_to_window {
                sync_renderer_to_window(renderer, window);
            }
            if effect.sync_palette_stacking {
                if let Some(palette_window) = palette_window {
                    palette_window::sync_palette_stacking(window, palette_window);
                }
            }
            if effect.log_traffic_stats {
                state.log_traffic_stats(renderer);
            }
            if effect.update_title {
                state.update_window_title(video, window.id());
            }
            match &command {
                AppCommand::ToggleAutoBudget => {
                    log::info!(
                        "auto-budget: {}",
                        if auto_budget_enabled_for_engine(&state.city, state.engine_fidelity) {
                            "on"
                        } else {
                            "off"
                        }
                    );
                }
                AppCommand::ToggleAutoGoto => {
                    log::info!(
                        "auto-goto: {}",
                        if state.auto_goto_enabled { "on" } else { "off" }
                    );
                }
                AppCommand::ToggleSoundEffects => {
                    log::info!(
                        "sound effects: {}",
                        if state.sound_effects_enabled {
                            "on"
                        } else {
                            "off"
                        }
                    );
                }
                AppCommand::ToggleMusic => {
                    log::info!("music: {}", if state.music_enabled { "on" } else { "off" });
                }
                AppCommand::ToggleNoDisasters => {
                    log::info!(
                        "no disasters: {}",
                        if state.no_disasters_enabled {
                            "on"
                        } else {
                            "off"
                        }
                    );
                }
                AppCommand::ToggleNewspaperSubscription => {
                    log::info!(
                        "newspaper subscription: {}",
                        if state.menu_checks.newspaper_subscription {
                            "on"
                        } else {
                            "off"
                        }
                    );
                }
                AppCommand::ToggleNewspaperExtra => {
                    log::info!(
                        "newspaper extra: {}",
                        if state.menu_checks.newspaper_extra {
                            "on"
                        } else {
                            "off"
                        }
                    );
                }
                AppCommand::ToggleBalloonHelp => {
                    log::info!(
                        "balloon help: {}",
                        if state.balloon_help_enabled {
                            "on"
                        } else {
                            "off"
                        }
                    );
                }
                _ => {}
            }
            if let AppCommand::SetSpeed(speed) = &command {
                log::info!(
                    "speed: {} (calendar + congestion; ambient anim pauses with game)",
                    speed.label()
                );
            }
            if let Some((kind, started)) = effect.disaster_log {
                if started {
                    log::info!("disaster: {}", kind.label());
                } else {
                    log::info!("disaster blocked by No Disasters: {}", kind.label());
                }
            }
            continue;
        }
        let title_dialog_open = matches!(
            &command,
            AppCommand::OpenTitleDialogPath(_) | AppCommand::OpenTitleScenarioPath(_)
        ) || matches!(
            &command,
            AppCommand::OpenConfirmedClassicCityPath {
                title_tail: true,
                ..
            }
        );
        let scenario_open = matches!(
            &command,
            AppCommand::OpenScenarioPath(_) | AppCommand::OpenTitleScenarioPath(_)
        );
        let confirmed_city_open = matches!(
            &command,
            AppCommand::OpenConfirmedCityPath(_) | AppCommand::OpenConfirmedClassicCityPath { .. }
        );
        let mut classic_cty_conversion_approved =
            matches!(&command, AppCommand::OpenConfirmedClassicCityPath { .. });
        if !classic_cty_conversion_approved
            && classic_cty_confirmation_required(&command, state.engine_fidelity, scenario_open)
        {
            if super::super::title_flow::confirm_classic_cty_conversion(window) {
                classic_cty_conversion_approved = true;
            } else {
                if title_dialog_open {
                    return_to_title_after_failed_replacement(input);
                }
                continue;
            }
        }
        match command {
            AppCommand::OpenCityDialog => {
                let track = cfg!(not(any(target_os = "android", target_os = "ios")))
                    && state.engine_fidelity.uses_win95();
                if open_city_dialog(window, track) && track {
                    input.win95_file_dialog_pending = Some(Win95FileDialog::City);
                }
            }
            AppCommand::OpenScenarioDialog => {
                let track = cfg!(not(any(target_os = "android", target_os = "ios")))
                    && state.engine_fidelity.uses_win95();
                if open_scenario_dialog(window, asset_profile, track) && track {
                    input.win95_file_dialog_pending = Some(Win95FileDialog::Scenario);
                    state.set_pe_external_modal(true);
                }
            }
            AppCommand::Win95FileDialogResult(kind, path) => {
                let admitted_scenario = kind == Win95FileDialog::Scenario
                    && input.win95_file_dialog_pending == Some(kind);
                if let Some(command) = finish_win95_file_dialog(input, kind, path) {
                    super::super::command_bus::command_queue()
                        .lock()
                        .expect("command queue")
                        .push_front(command);
                }
                if admitted_scenario {
                    state.set_pe_external_modal(false);
                }
            }
            AppCommand::OpenPath(path)
            | AppCommand::OpenConfirmedCityPath(path)
            | AppCommand::OpenTitlePath(path)
            | AppCommand::OpenTitleDialogPath(path)
            | AppCommand::OpenScenarioPath(path)
            | AppCommand::OpenTitleScenarioPath(path) => {
                if let Err(error) = open_city_document_with_title_tail(
                    state,
                    &path,
                    renderer.size(),
                    pixel_scale(window, renderer.size()),
                    title_dialog_open,
                    scenario_open,
                    classic_cty_conversion_approved,
                    |state| update_mac_city_music(state, city_music, asset_profile),
                    |state| state.update_window_title(video, window.id()),
                ) {
                    if state.engine_fidelity.uses_win95()
                        && !scenario_open
                        && cfg!(not(any(target_os = "android", target_os = "ios")))
                    {
                        crate::win95_file_error::record_city_io_error(&path, &error);
                        show_win95_city_open_errors(
                            window,
                            asset_profile,
                            &error,
                            confirmed_city_open,
                        );
                        if confirmed_city_open {
                            return_to_title_after_failed_replacement(input);
                            break;
                        }
                    } else {
                        show_asset_error(window, "Could Not Open City", &error);
                    }
                    continue;
                }
                if let Err(error) = asset_profile.remember_city(&path) {
                    log::warn!("could not remember city {}: {error:#}", path.display());
                }
            }
            AppCommand::StartMacScenario(path) => {
                if let Err(error) = state.load_mac_scenario(
                    &path,
                    renderer.size(),
                    pixel_scale(window, renderer.size()),
                ) {
                    show_asset_error(window, "Could Not Start Scenario", &error);
                }
                state.update_window_title(video, window.id());
            }
            AppCommand::ReloadAssets => {
                *asset_root = reload_asset_profile(
                    asset_profile,
                    renderer,
                    palette_renderer.as_deref_mut(),
                    cursors,
                )?;
                state.invalidate_sprite_cache();
            }
            AppCommand::OpenTileSetDialog => {
                open_tile_set_dialog(window, asset_root);
            }
            AppCommand::LoadTileSet(path) => {
                match asset_profile.set_original_pack(&path).and_then(|_| {
                    reload_asset_profile(
                        asset_profile,
                        renderer,
                        palette_renderer.as_deref_mut(),
                        cursors,
                    )
                }) {
                    Ok(path) => {
                        log::info!(
                            "loaded and remembered original assets from {}",
                            path.display()
                        );
                        *asset_root = path;
                        state.invalidate_sprite_cache();
                    }
                    Err(error) => show_asset_error(window, "Could Not Load Assets", &error),
                }
            }
            AppCommand::OpenImportOriginalAssetsDialog => {
                open_asset_folder_dialog(window, AssetFolderPurpose::ImportOriginal);
            }
            AppCommand::OpenOriginalAssetsDialog => {
                open_asset_folder_dialog(window, AssetFolderPurpose::ChooseOriginal);
            }
            AppCommand::OpenReplacementAssetsDialog => {
                open_asset_folder_dialog(window, AssetFolderPurpose::ChooseReplacement);
            }
            AppCommand::ImportOriginalAssets(path) => {
                match asset_profile
                    .import_original(&path)
                    .and_then(|(format, imported)| {
                        log::info!(
                            "imported {} assets to {}",
                            format.label(),
                            imported.display()
                        );
                        reload_asset_profile(
                            asset_profile,
                            renderer,
                            palette_renderer.as_deref_mut(),
                            cursors,
                        )
                    }) {
                    Ok(path) => {
                        *asset_root = path;
                        *sound = load_profile_sound(audio, asset_profile);
                        state.invalidate_sprite_cache();
                    }
                    Err(error) => show_asset_error(window, "Asset Import Failed", &error),
                }
            }
            AppCommand::SetOriginalAssets(path) => {
                match asset_profile.set_original_pack(path).and_then(|format| {
                    log::info!("selected {} original graphics", format.label());
                    reload_asset_profile(
                        asset_profile,
                        renderer,
                        palette_renderer.as_deref_mut(),
                        cursors,
                    )
                }) {
                    Ok(path) => {
                        *asset_root = path;
                        *sound = load_profile_sound(audio, asset_profile);
                        state.invalidate_sprite_cache();
                    }
                    Err(error) => show_asset_error(window, "Could Not Use Original Pack", &error),
                }
            }
            AppCommand::SetReplacementAssets(path) => {
                match asset_profile.set_replacement_pack(path).and_then(|()| {
                    reload_asset_profile(
                        asset_profile,
                        renderer,
                        palette_renderer.as_deref_mut(),
                        cursors,
                    )
                }) {
                    Ok(path) => {
                        *asset_root = path;
                        *sound = load_profile_sound(audio, asset_profile);
                        state.invalidate_sprite_cache();
                    }
                    Err(error) => {
                        show_asset_error(window, "Could Not Use Our Graphics Pack", &error)
                    }
                }
            }
            AppCommand::OpenConfirmedClassicCityPath { path, title_tail } => {
                if let Err(error) = open_city_document_with_title_tail(
                    state,
                    &path,
                    renderer.size(),
                    pixel_scale(window, renderer.size()),
                    title_tail,
                    false,
                    true,
                    |state| update_mac_city_music(state, city_music, asset_profile),
                    |state| state.update_window_title(video, window.id()),
                ) {
                    if state.engine_fidelity.uses_win95()
                        && cfg!(not(any(target_os = "android", target_os = "ios")))
                    {
                        crate::win95_file_error::record_city_io_error(&path, &error);
                        show_win95_city_open_errors(window, asset_profile, &error, true);
                        return_to_title_after_failed_replacement(input);
                    } else {
                        log::error!("could not open city {}: {error:#}", path.display());
                    }
                }
            }
            AppCommand::SetGraphicsPreference(preference) => {
                match asset_profile.set_preference(preference).and_then(|()| {
                    reload_asset_profile(
                        asset_profile,
                        renderer,
                        palette_renderer.as_deref_mut(),
                        cursors,
                    )
                }) {
                    Ok(path) => {
                        *asset_root = path;
                        state.sync_menu_graphics_preference(preference.prefers_replacement());
                        state.invalidate_sprite_cache();
                        log::info!(
                            "graphics preference: {}",
                            if preference.prefers_replacement() {
                                "ours first"
                            } else {
                                "original first"
                            }
                        );
                    }
                    Err(error) => show_asset_error(window, "Could Not Switch Graphics", &error),
                }
            }
            AppCommand::SetAppearance(preference) => {
                match asset_profile.set_appearance_preference(preference) {
                    Ok(()) => {
                        let dark = preference.is_dark(matches!(
                            sdl3::VideoSubsystem::get_system_theme(),
                            sdl3::video::SystemTheme::Dark
                        ));
                        state.sync_menu_appearance(preference);
                        state.dark_appearance = dark;
                        renderer.set_dark_appearance(dark);
                        if let Some(palette_renderer) = palette_renderer.as_deref_mut() {
                            palette_renderer.set_dark_appearance(dark);
                        }
                        mac_menu::set_appearance_preference(preference);
                        log::info!("appearance preference: {preference:?}");
                    }
                    Err(error) => show_asset_error(window, "Could Not Change Appearance", &error),
                }
            }
            AppCommand::CycleAssetSource(media) => match asset_profile.cycle_source(media) {
                Ok(Some(path)) => {
                    log::info!("selected {media:?} source {}", path.display());
                    if media == OriginalMedia::Graphics {
                        match reload_asset_profile(
                            asset_profile,
                            renderer,
                            palette_renderer.as_deref_mut(),
                            cursors,
                        ) {
                            Ok(path) => {
                                *asset_root = path;
                                state.invalidate_sprite_cache();
                            }
                            Err(error) => {
                                show_asset_error(window, "Could Not Switch Graphics", &error)
                            }
                        }
                    } else if media == OriginalMedia::Effects {
                        *sound = load_profile_sound(audio, asset_profile);
                    } else if media == OriginalMedia::Music {
                        *city_music = crate::title_music::CityMusic::new(
                            audio,
                            asset_profile,
                            state.music_enabled,
                            asset_profile.citygame_city_music_enabled(),
                            !state.engine_fidelity.uses_win95(),
                            None,
                        );
                    }
                }
                Ok(None) => log::warn!("no imported {media:?} source is available"),
                Err(error) => show_asset_error(window, "Could Not Switch Asset Source", &error),
            },
            AppCommand::ToggleCityGameEffects => {
                match asset_profile
                    .set_citygame_effects_enabled(!asset_profile.citygame_effects_enabled())
                {
                    Ok(()) => {
                        *sound = load_profile_sound(audio, asset_profile);
                        state.sync_menu_citygame_overrides(
                            asset_profile.citygame_effects_enabled(),
                            asset_profile.title_music_preference()
                                == TitleMusicPreference::Replacement,
                            asset_profile.citygame_city_music_enabled(),
                            asset_profile
                                .toolbar_palette_preference()
                                .uses_citygame_palette(),
                        );
                    }
                    Err(error) => show_asset_error(window, "Could Not Switch Sounds", &error),
                }
            }
            AppCommand::ToggleCityGameMusic => {
                let next = if asset_profile.title_music_preference()
                    == TitleMusicPreference::Replacement
                {
                    TitleMusicPreference::Original
                } else {
                    TitleMusicPreference::Replacement
                };
                match asset_profile.set_title_music_preference(next) {
                    Ok(()) => state.sync_menu_citygame_overrides(
                        asset_profile.citygame_effects_enabled(),
                        asset_profile.title_music_preference() == TitleMusicPreference::Replacement,
                        asset_profile.citygame_city_music_enabled(),
                        asset_profile
                            .toolbar_palette_preference()
                            .uses_citygame_palette(),
                    ),
                    Err(error) => show_asset_error(window, "Could Not Switch Music", &error),
                }
            }
            AppCommand::ToggleCityGameCityMusic => {
                let enabled = !asset_profile.citygame_city_music_enabled();
                match asset_profile.set_citygame_city_music_enabled(enabled) {
                    Ok(()) => {
                        *city_music = crate::title_music::CityMusic::new(
                            audio,
                            asset_profile,
                            state.music_enabled,
                            enabled,
                            !state.engine_fidelity.uses_win95(),
                            None,
                        );
                        state.sync_menu_citygame_overrides(
                            asset_profile.citygame_effects_enabled(),
                            asset_profile.title_music_preference()
                                == TitleMusicPreference::Replacement,
                            enabled,
                            asset_profile
                                .toolbar_palette_preference()
                                .uses_citygame_palette(),
                        );
                    }
                    Err(error) => show_asset_error(window, "Could Not Switch City Music", &error),
                }
            }
            AppCommand::ToggleToolbarPalette => {
                let preference = asset_profile.toolbar_palette_preference().next();
                match asset_profile.set_toolbar_palette_preference(preference) {
                    Ok(()) => {
                        let citygame_palette = preference.uses_citygame_palette();
                        renderer.set_citygame_toolbar_palette(citygame_palette);
                        if let Some(palette_renderer) = palette_renderer.as_deref_mut() {
                            palette_renderer.set_citygame_toolbar_palette(citygame_palette);
                        }
                        state.sync_menu_citygame_overrides(
                            asset_profile.citygame_effects_enabled(),
                            asset_profile.title_music_preference()
                                == TitleMusicPreference::Replacement,
                            asset_profile.citygame_city_music_enabled(),
                            citygame_palette,
                        );
                    }
                    Err(error) => {
                        show_asset_error(window, "Could Not Switch Toolbar Palette", &error)
                    }
                }
            }
            AppCommand::ToggleCityGameNewspaper => {
                let enabled = !asset_profile.citygame_newspaper_enabled();
                match asset_profile.set_citygame_newspaper_enabled(enabled) {
                    Ok(()) => {
                        state.sync_menu_citygame_newspaper(enabled);
                        log::info!("CityGame newspaper: {}", if enabled { "on" } else { "off" });
                    }
                    Err(error) => show_asset_error(window, "Could Not Switch Newspaper", &error),
                }
            }
            AppCommand::Quit => {
                input.running = false;
            }
            AppCommand::CloseSaveDialogResult(path) => {
                let size = renderer.size();
                let path_was_selected = path.is_some();
                if let Some((action, result)) = finish_close_save_dialog(input, path, |path| {
                    state.save_city_as(path, size, pixel_scale(window, size))
                }) {
                    if let Err(error) = result {
                        show_win95_save_failure(window, asset_profile, &error);
                    } else if state.city.has_backing_file() {
                        // A canceled Save As has no path and must not show a success alert.
                        if path_was_selected {
                            show_win95_save_success(window, asset_profile, &state.city.path);
                        }
                    }
                    super::super::command_bus::command_queue()
                        .lock()
                        .expect("command queue")
                        .push_front(AppCommand::ContinueAfterClose(action));
                }
            }
            AppCommand::ZoomIn => state.zoom_in_at(
                viewport_center(renderer.size()),
                renderer.size(),
                pixel_scale(window, renderer.size()),
            ),
            AppCommand::ZoomOut => state.zoom_out_at(
                viewport_center(renderer.size()),
                renderer.size(),
                pixel_scale(window, renderer.size()),
            ),
            AppCommand::ActualSize => state.actual_size_at(
                viewport_center(renderer.size()),
                renderer.size(),
                pixel_scale(window, renderer.size()),
            ),
            AppCommand::CenterMap => {
                state.center_map(renderer.size(), pixel_scale(window, renderer.size()))
            }
            AppCommand::FitMap => {
                state.fit_map(renderer.size(), pixel_scale(window, renderer.size()))
            }
            AppCommand::InspectTile => {
                log::info!(
                    "city={} chunks={} compass={} camera_rotation={} zoom={:.2} camera={:?} zone_overlay={} traffic={}",
                    state.city.name,
                    state.city.chunks.len(),
                    state.city.rotation(),
                    state.camera_rotation,
                    state.zoom,
                    state.camera,
                    state.show_zone_overlay,
                    state.show_traffic
                );
            }
            AppCommand::InspectHoverTile => {
                state.inspect_tile_at(
                    input.mouse_pos,
                    renderer.size(),
                    pixel_scale(window, renderer.size()),
                );
                state.update_window_title(video, window.id());
            }
            #[cfg(target_os = "ios")]
            AppCommand::SaveCity | AppCommand::SaveCityAs => {
                let snapshot =
                    state.city_save_snapshot(renderer.size(), pixel_scale(window, renderer.size()));
                if let Err(error) = crate::ios_documents::save_city(
                    window,
                    snapshot,
                    state.city.path.clone(),
                    state.engine_fidelity.uses_win95(),
                ) {
                    crate::ios_documents::show_error(window, &error);
                }
            }
            #[cfg(target_os = "ios")]
            AppCommand::DocumentSaved {
                path,
                previous_path,
            } => {
                if state.city.path == previous_path {
                    state.city.adopt_saved_document(path.clone());
                    state.update_mac_name_after_save();
                    if let Err(error) = asset_profile.remember_city(&path) {
                        log::warn!("could not remember saved city: {error:#}");
                    }
                    state.update_window_title(video, window.id());
                }
            }
            #[cfg(not(target_os = "ios"))]
            AppCommand::SaveCity => {
                if state.engine_fidelity.uses_win95() && !state.city.has_backing_file() {
                    // The PE's current-path string is empty for a new city;
                    // its ordinary Save command enters the Save As chooser.
                    let track = cfg!(not(any(target_os = "android", target_os = "ios")));
                    if save_city_dialog(window, state.city.kind, &state.city.name, true, track)
                        && track
                    {
                        input.win95_file_dialog_pending = Some(Win95FileDialog::SaveAs);
                    }
                } else {
                    let size = renderer.size();
                    if let Err(error) = state.save_current_city(size, pixel_scale(window, size)) {
                        if state.engine_fidelity.uses_win95() {
                            show_win95_save_failure(window, asset_profile, &error);
                        } else {
                            log::error!("save failed: {error:#}");
                        }
                    } else {
                        log::info!("saved city to {}", state.city.path.display());
                        if state.engine_fidelity.uses_win95() {
                            show_win95_save_success(window, asset_profile, &state.city.path);
                        }
                    }
                }
            }
            #[cfg(not(target_os = "ios"))]
            AppCommand::SaveCityAs => {
                let track = cfg!(not(any(target_os = "android", target_os = "ios")))
                    && state.engine_fidelity.uses_win95();
                if save_city_dialog(
                    window,
                    state.city.kind,
                    &state.city.name,
                    state.engine_fidelity.uses_win95(),
                    track,
                ) && track
                {
                    input.win95_file_dialog_pending = Some(Win95FileDialog::SaveAs);
                }
            }
            AppCommand::SaveToPath(path) => {
                let size = renderer.size();
                if let Err(error) = state.save_city_as(&path, size, pixel_scale(window, size)) {
                    if state.engine_fidelity.uses_win95() {
                        show_win95_save_failure(window, asset_profile, &error);
                    } else {
                        log::error!("save failed: {error:#}");
                    }
                } else {
                    log::info!("saved city to {}", path.display());
                    if state.engine_fidelity.uses_win95() {
                        show_win95_save_success(window, asset_profile, &state.city.path);
                    }
                    state.update_window_title(video, window.id());
                }
            }
            AppCommand::NewCity => {
                if let Err(error) = state.new_city() {
                    log::error!("new city failed: {error:#}");
                } else {
                    state.update_window_title(video, window.id());
                    *palette_logical_size = match palette_renderer.as_deref_mut() {
                        Some(palette_renderer) => palette_window::sync_for_mode_ex(
                            video,
                            palette_id,
                            window,
                            palette_renderer,
                            state.mode,
                            true,
                        )?,
                        None => ui::palette_logical_size(state.mode),
                    };
                }
            }
            AppCommand::NewCityFromTitle {
                city_name,
                starting_funds,
                starting_year,
                difficulty,
                generation_seed,
                start_flow,
            } => {
                if let Err(error) = state.new_city_from_title(
                    city_name,
                    starting_funds,
                    starting_year,
                    difficulty,
                    generation_seed,
                    start_flow,
                ) {
                    log::error!("new city failed: {error:#}");
                } else {
                    state.update_window_title(video, window.id());
                    *palette_logical_size = match palette_renderer.as_deref_mut() {
                        Some(palette_renderer) => palette_window::sync_for_mode_ex(
                            video,
                            palette_id,
                            window,
                            palette_renderer,
                            state.mode,
                            true,
                        )?,
                        None => ui::palette_logical_size(state.mode),
                    };
                }
            }
            AppCommand::FinishWin95Charter | AppCommand::ToggleEditMap => {
                if matches!(command, AppCommand::FinishWin95Charter) {
                    state.finish_edit_map_after_charter();
                    state.update_window_title(video, window.id());
                } else {
                    state.toggle_edit_mode(video, window.id());
                }
                *palette_logical_size = match palette_renderer.as_deref_mut() {
                    Some(palette_renderer) => palette_window::sync_for_mode_ex(
                        video,
                        palette_id,
                        window,
                        palette_renderer,
                        state.mode,
                        true,
                    )?,
                    None => ui::palette_logical_size(state.mode),
                };
            }
            AppCommand::ToggleGrid
            | AppCommand::ToggleZoneOverlay
            | AppCommand::ToggleTraffic
            | AppCommand::ToggleUnderground
            | AppCommand::SetDataMapMode(_)
            | AppCommand::ToggleAutoBudget
            | AppCommand::ToggleAutoGoto
            | AppCommand::ToggleSoundEffects
            | AppCommand::ToggleMusic
            | AppCommand::ToggleNoDisasters
            | AppCommand::ToggleNewspaperSubscription
            | AppCommand::ToggleNewspaperExtra
            | AppCommand::ToggleBalloonHelp
            | AppCommand::StartDisaster(_)
            | AppCommand::OpenCityInfo(_)
            | AppCommand::OpenMacPaper(_)
            | AppCommand::SetSpeed(_)
            | AppCommand::SetTool(_)
            | AppCommand::ReturnToTitle
            | AppCommand::ContinueAfterClose(_)
            | AppCommand::RotateView(_) => {
                unreachable!("state command should have been handled before runtime match")
            }
        }
        if !input.running {
            break;
        }
    }
    if !deferred.is_empty() {
        let mut queue = super::super::command_bus::command_queue()
            .lock()
            .expect("command queue");
        restore_deferred_commands(&mut queue, deferred);
    }
    Ok(())
}

fn restore_deferred_commands(
    queue: &mut std::collections::VecDeque<AppCommand>,
    mut deferred: std::collections::VecDeque<AppCommand>,
) {
    while let Some(command) = deferred.pop_back() {
        queue.push_front(command);
    }
}

fn win95_save_feedback(profile: &AssetProfile, index: usize, fallback: &str) -> String {
    profile
        .win95_save_feedback_path()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|data| serde_json::from_slice::<[String; 2]>(&data).ok())
        .and_then(|strings| strings.get(index).cloned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn show_win95_save_success(
    window: &sdl3::video::Window,
    profile: &AssetProfile,
    path: &std::path::Path,
) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let prefix = win95_save_feedback(profile, 0, "City saved: ");
        let _ = sdl3::messagebox::show_simple_message_box(
            MessageBoxFlag::INFORMATION,
            "CityGame",
            &format!("{prefix}{}", path.display()),
            window,
        );
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = (window, profile, path);
}

fn show_win95_save_failure(
    window: &sdl3::video::Window,
    profile: &AssetProfile,
    error: &anyhow::Error,
) {
    log::error!("save failed: {error:#}");
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let message = win95_save_feedback(profile, 1, "City could not be saved.");
        let _ = sdl3::messagebox::show_simple_message_box(
            MessageBoxFlag::ERROR,
            "CityGame",
            &message,
            window,
        );
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = (window, profile);
}

fn show_win95_not_iff(window: &sdl3::video::Window, profile: &AssetProfile) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let message = profile
            .win95_not_iff_path()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|data| serde_json::from_slice::<String>(&data).ok())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "This file is not a supported IFF city.".to_owned());
        let _ = sdl3::messagebox::show_simple_message_box(
            MessageBoxFlag::ERROR,
            "CityGame",
            &message,
            window,
        );
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = (window, profile);
}

fn show_win95_wrong_iff_city_type(window: &sdl3::video::Window, profile: &AssetProfile) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let message = profile
            .win95_wrong_iff_city_type_path()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|data| serde_json::from_slice::<String>(&data).ok())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "This IFF file is not a SimCity 2000 city.".to_owned());
        let _ = sdl3::messagebox::show_simple_message_box(
            MessageBoxFlag::ERROR,
            "CityGame",
            &message,
            window,
        );
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = (window, profile);
}

fn show_win95_city_open_failure(
    window: &sdl3::video::Window,
    profile: &AssetProfile,
    error: &anyhow::Error,
) {
    log::error!("city open failed: {error:#}");
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let message = profile
            .win95_city_open_failure_path()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|data| serde_json::from_slice::<String>(&data).ok())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Could not open the city.".to_owned());
        let _ = sdl3::messagebox::show_simple_message_box(
            MessageBoxFlag::ERROR,
            "CityGame",
            &message,
            window,
        );
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = (window, profile);
}

pub(in crate::app) fn show_win95_city_open_errors(
    window: &sdl3::video::Window,
    profile: &AssetProfile,
    error: &anyhow::Error,
    include_format_error: bool,
) {
    if include_format_error && error.downcast_ref::<crate::city::CityReadEof>().is_some() {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let messages = profile
                .win95_read_error_path()
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|data| serde_json::from_slice::<Vec<String>>(&data).ok());
            let message = crate::win95_file_error::read_message(
                messages.as_deref(),
                crate::win95_file_error::current_cause(),
            );
            let _ = sdl3::messagebox::show_simple_message_box(
                MessageBoxFlag::ERROR,
                "CityGame",
                &message,
                window,
            );
        }
    } else if include_format_error
        && error
            .downcast_ref::<crate::city::NotIffCityFile>()
            .is_some()
    {
        show_win95_not_iff(window, profile);
    } else if include_format_error
        && error
            .downcast_ref::<crate::city::WrongIffCityType>()
            .is_some()
    {
        show_win95_wrong_iff_city_type(window, profile);
    }
    show_win95_city_open_failure(window, profile, error);
}

fn return_to_title_after_failed_replacement(input: &mut InputState) {
    input.return_to_title = true;
    input.running = false;
}

fn win95_close_action_needs_prompt(action: &Win95CloseAction, mode: AppMode) -> bool {
    // PE 0x42e250 bypasses its close helper when the edit-map latch is set.
    // The CityGame editor-exit toggle is not the PE's replace-city command.
    !matches!(
        action,
        Win95CloseAction::NewCity | Win95CloseAction::EditMap
    ) || mode != AppMode::EditNewMap
}

fn finish_win95_file_dialog(
    input: &mut InputState,
    kind: Win95FileDialog,
    path: Option<std::path::PathBuf>,
) -> Option<AppCommand> {
    if input.win95_file_dialog_pending != Some(kind) {
        return None;
    }
    input.win95_file_dialog_pending = None;
    path.map(|path| match kind {
        Win95FileDialog::City => AppCommand::OpenConfirmedCityPath(path),
        Win95FileDialog::Scenario => AppCommand::OpenScenarioPath(path),
        Win95FileDialog::SaveAs => AppCommand::SaveToPath(path),
    })
}

fn finish_close_save_dialog(
    input: &mut InputState,
    path: Option<std::path::PathBuf>,
    save: impl FnOnce(&std::path::Path) -> Result<()>,
) -> Option<(Win95CloseAction, Result<()>)> {
    let action = input.close_save_pending.take()?;
    let result = path.map_or(Ok(()), |path| save(&path));
    Some((action, result))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Win95CloseChoice {
    Save,
    Discard,
    Cancel,
}

fn confirm_win95_close(window: &sdl3::video::Window, city_name: &str) -> Win95CloseChoice {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let buttons = [
            ButtonData {
                flags: MessageBoxButtonFlag::RETURNKEY_DEFAULT,
                button_id: 6,
                text: "Yes",
            },
            ButtonData {
                flags: MessageBoxButtonFlag::empty(),
                button_id: 7,
                text: "No",
            },
            ButtonData {
                flags: MessageBoxButtonFlag::ESCAPEKEY_DEFAULT,
                button_id: 2,
                text: "Cancel",
            },
        ];
        let message = format!("Do You Want To Save Your City: {city_name}");
        return match sdl3::messagebox::show_message_box(
            MessageBoxFlag::INFORMATION,
            &buttons,
            "CityGame",
            &message,
            window,
            None,
        ) {
            Ok(ClickedButton::CustomButton(button)) if button.button_id == 6 => {
                Win95CloseChoice::Save
            }
            Ok(ClickedButton::CustomButton(button)) if button.button_id == 7 => {
                Win95CloseChoice::Discard
            }
            Ok(_) => Win95CloseChoice::Cancel,
            Err(error) => {
                log::warn!("could not show save-before-exit prompt: {error}");
                Win95CloseChoice::Cancel
            }
        };
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (window, city_name);
        Win95CloseChoice::Cancel
    }
}

fn classic_cty_confirmation_required(
    command: &AppCommand,
    fidelity: crate::engine_fidelity::EngineFidelity,
    scenario_open: bool,
) -> bool {
    if !fidelity.uses_win95() || scenario_open {
        return false;
    }
    let path = match command {
        AppCommand::OpenPath(path)
        | AppCommand::OpenConfirmedCityPath(path)
        | AppCommand::OpenTitlePath(path)
        | AppCommand::OpenTitleDialogPath(path) => path,
        _ => return false,
    };
    crate::city::City::load(path).is_err()
        && crate::city::classic_cty::ClassicCtyFile::has_legacy_import_size(path)
        && crate::city::classic_cty::ClassicCtyFile::read(path).is_ok()
}

#[cfg(test)]
mod fiscal_command_tests {
    use super::*;

    #[test]
    fn win95_classic_cty_prompt_uses_the_native_fixed_length_fallback_gate() {
        let path = std::env::temp_dir().join(format!(
            "citygame-classic-cty-prompt-{}.cty",
            std::process::id()
        ));
        std::fs::write(&path, vec![0; 0x6a70]).unwrap();
        let command = AppCommand::OpenConfirmedCityPath(path.clone());

        assert!(classic_cty_confirmation_required(
            &command,
            crate::engine_fidelity::EngineFidelity::Win95,
            false,
        ));
        assert!(!classic_cty_confirmation_required(
            &command,
            crate::engine_fidelity::EngineFidelity::Mac68k,
            false,
        ));
        assert!(!classic_cty_confirmation_required(
            &command,
            crate::engine_fidelity::EngineFidelity::Win95,
            true,
        ));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn win95_save_feedback_uses_local_resources_and_falls_back() {
        let root =
            std::env::temp_dir().join(format!("citygame-save-feedback-{}", std::process::id()));
        let pack = root.join("pack");
        let catalog = pack.join("catalog/data");
        std::fs::create_dir_all(&catalog).unwrap();
        let settings = root.join("settings.json");
        std::fs::write(
            &settings,
            serde_json::json!({"schema_version": 1, "original_graphics": pack}).to_string(),
        )
        .unwrap();
        let profile = AssetProfile::load_from(settings).unwrap();
        assert_eq!(win95_save_feedback(&profile, 1, "fallback"), "fallback");
        let path = catalog.join("win95-save-feedback.json");
        std::fs::write(&path, b"invalid json").unwrap();
        assert_eq!(win95_save_feedback(&profile, 1, "fallback"), "fallback");
        std::fs::write(&path, br#"["prefix ","failure"]"#).unwrap();
        assert_eq!(win95_save_feedback(&profile, 0, "fallback"), "prefix ");
        assert_eq!(win95_save_feedback(&profile, 1, "fallback"), "failure");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fiscal_notice_preserves_deferred_file_operations_in_order() {
        let save = AppCommand::SaveToPath("/tmp/fiscal-save.sc2".into());
        let open = AppCommand::OpenPath("/tmp/fiscal-open.sc2".into());
        let later = AppCommand::SetSpeed(GameSpeed::Llama);
        let mut queue = std::collections::VecDeque::from([later.clone()]);
        restore_deferred_commands(
            &mut queue,
            std::collections::VecDeque::from([save.clone(), open.clone()]),
        );
        assert_eq!(queue.into_iter().collect::<Vec<_>>(), [save, open, later]);
    }

    #[test]
    fn win95_open_dialog_holds_simulation_and_resumes_only_its_own_result() {
        let mut input = InputState::new();
        let path = std::path::PathBuf::from("/tmp/example.sc2");
        assert!(!input.blocking_file_dialog());
        input.win95_file_dialog_pending = Some(Win95FileDialog::City);
        assert!(input.blocking_file_dialog());
        assert!(input.holds_pe_idle_for_common_dialog());
        assert_eq!(
            finish_win95_file_dialog(&mut input, Win95FileDialog::Scenario, Some(path.clone())),
            None
        );
        assert!(input.blocking_file_dialog());
        let city = finish_win95_file_dialog(&mut input, Win95FileDialog::City, Some(path.clone()))
            .unwrap();
        assert_eq!(city, AppCommand::OpenConfirmedCityPath(path.clone()));
        assert_eq!(Win95CloseAction::from_command(&city), None);
        assert!(!input.blocking_file_dialog());
        assert_eq!(
            finish_win95_file_dialog(&mut input, Win95FileDialog::City, Some(path.clone())),
            None
        );
        input.win95_file_dialog_pending = Some(Win95FileDialog::Scenario);
        assert!(input.blocking_file_dialog());
        assert!(!input.holds_pe_idle_for_common_dialog());
        let scenario =
            finish_win95_file_dialog(&mut input, Win95FileDialog::Scenario, Some(path.clone()))
                .unwrap();
        assert_eq!(scenario, AppCommand::OpenScenarioPath(path));
        assert!(Win95CloseAction::from_command(&scenario).is_some());
        input.win95_file_dialog_pending = Some(Win95FileDialog::Scenario);
        assert_eq!(
            finish_win95_file_dialog(&mut input, Win95FileDialog::Scenario, None),
            None
        );
        assert!(!input.blocking_file_dialog());
        input.win95_file_dialog_pending = Some(Win95FileDialog::SaveAs);
        assert!(input.holds_pe_idle_for_common_dialog());
        let target = std::path::PathBuf::from("/tmp/saved.sc2");
        assert_eq!(
            finish_win95_file_dialog(&mut input, Win95FileDialog::SaveAs, Some(target.clone())),
            Some(AppCommand::SaveToPath(target))
        );
        assert!(!input.blocking_file_dialog());
    }

    #[test]
    fn close_save_dialog_resumes_the_requested_action_after_each_save_outcome() {
        use crate::city::City;
        let directory =
            std::env::temp_dir().join(format!("citygame-quit-save-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let target = directory.join("new.sc2");
        let mut city = City::new_blank(&target, "New City").unwrap();
        let mut input = InputState::new();

        input.close_save_pending = Some(Win95CloseAction::LoadCity);
        let (action, result) =
            finish_close_save_dialog(&mut input, None, |_| panic!("cancel must not save")).unwrap();
        assert_eq!(action, Win95CloseAction::LoadCity);
        assert!(result.is_ok());
        assert!(input.running && input.close_save_pending.is_none() && !city.has_backing_file());
        assert!(
            finish_close_save_dialog(&mut input, Some(target.clone()), |_| panic!(
                "stale result must not save"
            ))
            .is_none()
        );
        assert!(input.running && !city.has_backing_file());

        input.close_save_pending = Some(Win95CloseAction::NewCity);
        let (action, result) =
            finish_close_save_dialog(&mut input, Some(directory.clone()), |path| city.save(path))
                .unwrap();
        assert_eq!(action, Win95CloseAction::NewCity);
        assert!(result.is_err());
        assert!(input.running && input.close_save_pending.is_none() && !city.has_backing_file());

        input.close_save_pending = Some(Win95CloseAction::LoadCity);
        let (action, result) =
            finish_close_save_dialog(&mut input, Some(target.clone()), |path| city.save(path))
                .unwrap();
        result.unwrap();
        assert_eq!(action.command(), AppCommand::OpenCityDialog);
        assert!(input.running && input.close_save_pending.is_none() && city.has_backing_file());
        assert_eq!(City::load(&target).unwrap().name, "New City");

        input.close_save_pending = Some(Win95CloseAction::Quit);
        let (action, result) =
            finish_close_save_dialog(&mut input, Some(target.clone()), |path| city.save(path))
                .unwrap();
        result.unwrap();
        assert_eq!(action, Win95CloseAction::Quit);
        assert_eq!(Win95CloseAction::Quit.command(), AppCommand::Quit);
        input.close_save_pending = Some(Win95CloseAction::NewCity);
        let (action, result) =
            finish_close_save_dialog(&mut input, Some(target.clone()), |path| city.save(path))
                .unwrap();
        result.unwrap();
        assert_eq!(action, Win95CloseAction::NewCity);
        assert_eq!(Win95CloseAction::NewCity.command(), AppCommand::NewCity);
        assert_eq!(
            Win95CloseAction::from_command(&AppCommand::OpenTitleDialogPath(target.clone())),
            None
        );
        for command in [
            AppCommand::OpenPath(target.clone()),
            AppCommand::OpenScenarioPath(target.clone()),
            AppCommand::OpenTitlePath(target.clone()),
        ] {
            let action = Win95CloseAction::from_command(&command).unwrap();
            input.close_save_pending = Some(action);
            let (resumed, result) =
                finish_close_save_dialog(&mut input, Some(target.clone()), |path| city.save(path))
                    .unwrap();
            result.unwrap();
            assert_eq!(resumed.command(), command);
        }
        assert_eq!(
            Win95CloseAction::from_command(&AppCommand::OpenConfirmedCityPath(target.clone())),
            None
        );
        assert_eq!(
            Win95CloseAction::from_command(&AppCommand::OpenCityDialog),
            Some(Win95CloseAction::LoadCity)
        );
        assert_eq!(
            Win95CloseAction::from_command(&AppCommand::ToggleEditMap),
            Some(Win95CloseAction::EditMap)
        );
        input.close_save_pending = Some(Win95CloseAction::EditMap);
        let (action, result) =
            finish_close_save_dialog(&mut input, None, |_| panic!("cancel must not save")).unwrap();
        result.unwrap();
        assert_eq!(action.command(), AppCommand::ToggleEditMap);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn win95_new_city_and_edit_map_prompt_depend_on_editor_mode() {
        for action in [Win95CloseAction::NewCity, Win95CloseAction::EditMap] {
            assert!(win95_close_action_needs_prompt(&action, AppMode::CityView));
            assert!(!win95_close_action_needs_prompt(
                &action,
                AppMode::EditNewMap
            ));
        }
        assert!(win95_close_action_needs_prompt(
            &Win95CloseAction::Quit,
            AppMode::EditNewMap
        ));
    }
}

fn confirm_return_to_title(window: &sdl3::video::Window) -> bool {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let buttons = [
            ButtonData {
                flags: MessageBoxButtonFlag::RETURNKEY_DEFAULT,
                button_id: 1,
                text: "Return to Title Screen",
            },
            ButtonData {
                flags: MessageBoxButtonFlag::ESCAPEKEY_DEFAULT,
                button_id: 0,
                text: "Cancel",
            },
        ];
        return match sdl3::messagebox::show_message_box(
            MessageBoxFlag::INFORMATION,
            &buttons,
            "Return to Title Screen?",
            "Returning to the title screen does not save the city. Save it first if you want to keep your latest changes.",
            window,
            None,
        ) {
            Ok(ClickedButton::CustomButton(button)) => button.button_id == 1,
            Ok(ClickedButton::CloseButton) => false,
            Err(error) => {
                log::warn!("could not show return-to-title confirmation: {error}");
                false
            }
        };
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = window;
        true
    }
}

pub(super) fn update_mac_city_music(
    state: &mut ViewerState,
    player: &mut Option<crate::title_music::CityMusic>,
    profile: &AssetProfile,
) {
    state.mac_music_callback_with_playback(
        mac_tick_count(),
        player,
        |player| {
            player
                .as_ref()
                .is_some_and(crate::title_music::CityMusic::song_active)
        },
        |player, song| {
            if let Some(player) = player {
                player.request_original_song(profile, song);
            }
            mac_tick_count()
        },
    );
}

#[cfg(test)]
pub(super) fn open_city_document(
    state: &mut ViewerState,
    path: &std::path::Path,
    size: (u32, u32),
    scale: f32,
    music_callback: impl FnMut(&mut ViewerState),
    finish: impl FnOnce(&ViewerState),
) -> Result<()> {
    open_city_document_with_title_tail(
        state,
        path,
        size,
        scale,
        true,
        false,
        false,
        music_callback,
        finish,
    )
}

#[cfg(test)]
mod music_open_tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn invalid_city_open_skips_completion_and_returns_to_title_after_replacement() {
        let root =
            std::env::temp_dir().join(format!("citygame-invalid-open-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let bad = root.join("broken.sc2");
        std::fs::write(&bad, b"not a city").unwrap();
        let city = crate::city::City::new_blank(root.join("current.sc2"), "Current").unwrap();
        let mut state = crate::app::tests::test_state(city);
        let path = state.city.path.clone();
        let funds = state.city.city_funds();
        let callbacks = RefCell::new(Vec::new());
        let result = open_direct_city_document(
            &mut state,
            &bad,
            (640, 480),
            1.0,
            |_: &mut ViewerState| callbacks.borrow_mut().push("music"),
            |_: &ViewerState| callbacks.borrow_mut().push("finish"),
        );
        assert!(
            result
                .unwrap_err()
                .downcast_ref::<crate::city::NotIffCityFile>()
                .is_some()
        );
        assert_eq!(state.city.path, path);
        assert_eq!(state.city.name, "Current");
        assert_eq!(state.city.city_funds(), funds);
        assert!(callbacks.borrow().is_empty());
        std::fs::write(&bad, b"FORM\0\0\0\x04ILBM").unwrap();
        let result = open_direct_city_document(
            &mut state,
            &bad,
            (640, 480),
            1.0,
            |_: &mut ViewerState| callbacks.borrow_mut().push("music"),
            |_: &ViewerState| callbacks.borrow_mut().push("finish"),
        );
        assert!(
            result
                .unwrap_err()
                .downcast_ref::<crate::city::WrongIffCityType>()
                .is_some()
        );
        assert_eq!(state.city.path, path);
        assert!(callbacks.borrow().is_empty());
        let mut input = InputState::new();
        return_to_title_after_failed_replacement(&mut input);
        assert!(!input.running);
        assert!(input.return_to_title);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn not_iff_error_handles_partial_legacy_and_form_headers() {
        let path = std::path::Path::new("fixture.sc2");
        for bytes in [vec![], vec![0; 0x6a70], b"FORMxxxxxxxx".to_vec()] {
            let error = crate::city::detect_city_kind(path, &bytes).unwrap_err();
            assert!(
                error
                    .downcast_ref::<crate::city::NotIffCityFile>()
                    .is_none()
            );
        }
        let error = crate::city::detect_city_kind(path, b"invalid city fixture\n").unwrap_err();
        assert!(
            error
                .downcast_ref::<crate::city::NotIffCityFile>()
                .is_some()
        );
        for bytes in [&b"X"[..], &b"FX"[..], &b"FOX"[..]] {
            let error = crate::city::detect_city_kind(path, bytes).unwrap_err();
            assert!(
                error
                    .downcast_ref::<crate::city::NotIffCityFile>()
                    .is_some()
            );
        }
        // Matching partial FORM prefixes depend on the original stack residue.
        for bytes in [&b"F"[..], &b"FO"[..], &b"FOR"[..]] {
            let error = crate::city::detect_city_kind(path, bytes).unwrap_err();
            assert!(
                error
                    .downcast_ref::<crate::city::NotIffCityFile>()
                    .is_none()
            );
        }
    }

    #[test]
    fn wrong_iff_city_type_handles_partial_type_reads() {
        let path = std::path::Path::new("fixture.sc2");
        let wrong = crate::city::detect_city_kind(path, b"FORM\0\0\0\x04ILBM").unwrap_err();
        assert!(
            wrong
                .downcast_ref::<crate::city::WrongIffCityType>()
                .is_some()
        );
        for bytes in [
            &b"FORM\0\0\0\x04S"[..],
            &b"FORM\0\0\0\x04SC"[..],
            &b"FORM\0\0\0\x04SCD"[..],
            &b"FORM\0\0\0\x04ILB"[..],
        ] {
            let error = crate::city::detect_city_kind(path, bytes).unwrap_err();
            assert!(
                error
                    .downcast_ref::<crate::city::WrongIffCityType>()
                    .is_some()
            );
        }
        for bytes in [&b"FORM\0\0\0\x04"[..], &b"NOPE\0\0\0\x04ILBM"[..]] {
            let error = crate::city::detect_city_kind(path, bytes).unwrap_err();
            assert!(
                error
                    .downcast_ref::<crate::city::WrongIffCityType>()
                    .is_none()
            );
        }
        assert!(crate::city::detect_city_kind(path, b"FORM\0\0\0\x04SCDH").is_ok());
    }

    #[test]
    fn win95_header_eof_covers_short_length_words() {
        let path = std::path::Path::new("fixture.sc2");
        let header = b"FORM\x12\x34\x56\x78SCDH";
        for len in [0, 4, 5, 6, 7, 8] {
            let error = crate::city::detect_city_kind(path, &header[..len]).unwrap_err();
            assert!(
                error.downcast_ref::<crate::city::CityReadEof>().is_some(),
                "length {len}"
            );
        }
        for len in [1, 2, 3, 9, 10, 11] {
            let error = crate::city::detect_city_kind(path, &header[..len]).unwrap_err();
            assert!(
                error.downcast_ref::<crate::city::CityReadEof>().is_none(),
                "length {len}"
            );
        }
    }

    #[test]
    fn title_and_direct_open_service_music_at_their_native_boundaries() {
        let path =
            std::env::temp_dir().join(format!("citygame-music-open-{}.sc2", std::process::id()));
        let mut city = crate::city::City::new_blank(&path, "Music open").unwrap();
        city.save(&path).unwrap();
        for title in [false, true] {
            let mut state = crate::app::tests::test_state(city.clone());
            state.engine_fidelity = EngineFidelity::Mac68k;
            let calls = RefCell::new(Vec::new());
            let music = |_: &mut ViewerState| calls.borrow_mut().push("music");
            let finish = |_: &ViewerState| calls.borrow_mut().push("finish");
            if title {
                open_city_document(&mut state, &path, (640, 480), 1.0, music, finish).unwrap();
                assert_eq!(*calls.borrow(), ["music", "finish", "music"]);
            } else {
                open_direct_city_document(&mut state, &path, (640, 480), 1.0, music, finish)
                    .unwrap();
                assert_eq!(*calls.borrow(), ["music", "finish"]);
            }
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    #[ignore = "requires an authentic private Classic CTY and native Win95 reader plane capture"]
    fn win95_authentic_classic_cty_confirmation_open_and_save_reload_match_native_planes() {
        use std::collections::BTreeMap;

        let source = std::path::PathBuf::from(
            std::env::var_os("CITYGAME_CLASSIC_CTY_NATIVE_INPUT").unwrap(),
        );
        let city_name = source.file_stem().unwrap().to_string_lossy().to_string();
        let (native_year, native_month) = match city_name.as_str() {
            "BAZ" => (2000, 1),
            "BERN" => (1977, 10),
            other => panic!("unsupported native Classic CTY fixture {other}"),
        };
        let native_planes_path = std::path::PathBuf::from(
            std::env::var_os("CITYGAME_CLASSIC_CTY_NATIVE_PLANES").unwrap(),
        );
        let expected: BTreeMap<String, String> = serde_json::from_slice(
            &std::fs::read(native_planes_path).expect("read retail reader plane capture"),
        )
        .expect("decode retail plane capture");
        let expected_bytes = |name: &str| {
            expected
                .get(name)
                .unwrap_or_else(|| panic!("missing native {name}"))
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
        };
        let collect_planes = |city: &crate::city::City| {
            let (mut xbld, mut xzon, mut xbit, mut altm, mut xtxt) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for x in 0..128 {
                for y in 0..128 {
                    xbld.push(city.building_at(x, y));
                    xzon.push(city.zone_at(x, y).unwrap_or_default().raw_bits());
                    xbit.push(city.tile_bits_at(x, y).unwrap_or_default().raw_bits());
                    xtxt.push(city.text_overlay_at(x, y).unwrap_or_default());
                    let sample = city.altm_at(x, y);
                    let word = u16::from(sample.land)
                        | (u16::from(sample.water) << 5)
                        | (u16::from(sample.tunnel_levels) << 10);
                    altm.extend_from_slice(&word.to_le_bytes());
                }
            }
            [
                ("XBLD", xbld),
                ("XZON", xzon),
                ("XBIT", xbit),
                ("ALTM", altm),
                ("XTXT", xtxt),
            ]
            .into_iter()
            .collect::<BTreeMap<_, _>>()
        };

        let source_city = crate::city::City::new_blank("initial.sc2", "Initial").unwrap();
        let mut state = crate::app::tests::test_state(source_city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.pe_process_crt = crate::win95_random::Win95CrtRand::new(2_503_793_637);
        state.pe_process_game_rand = crate::win95_random::Win95GameRand::new(1);
        assert!(classic_cty_confirmation_required(
            &AppCommand::OpenConfirmedCityPath(source.clone()),
            state.engine_fidelity,
            false,
        ));

        let callbacks = RefCell::new(Vec::new());
        open_city_document_with_title_tail(
            &mut state,
            &source,
            (800, 600),
            1.0,
            false,
            false,
            true,
            |_: &mut ViewerState| callbacks.borrow_mut().push("music"),
            |state: &ViewerState| {
                assert_eq!(state.city.name, city_name);
                assert_eq!(
                    state.city.saved_date(),
                    Some(crate::city::CityDate {
                        year: native_year,
                        month: native_month,
                    })
                );
                callbacks.borrow_mut().push("finish");
            },
        )
        .expect("normal confirmed document-open path imports CTY");
        assert_eq!(*callbacks.borrow(), ["music", "finish"]);
        assert_eq!(
            state.pe_process_crt_seed(),
            0xb8bb_6730,
            "confirmed BAZ open preserves reader output plus native open-controller samples"
        );
        assert_eq!(
            state.pe_process_game_rand_state(),
            0x9302_aeea,
            "confirmed BAZ open preserves the native border LCG sequence"
        );
        let opened_planes = collect_planes(&state.city);
        for name in ["XBLD", "XZON", "XBIT", "ALTM", "XTXT"] {
            let native = expected_bytes(name);
            let differences = opened_planes[name]
                .iter()
                .zip(&native)
                .enumerate()
                .filter_map(|(index, (citygame, retail))| {
                    (citygame != retail).then_some((index, *citygame, *retail))
                })
                .collect::<Vec<_>>();
            assert!(
                differences.is_empty(),
                "opened {name} has {} differing cells; first differences: {:?}",
                differences.len(),
                &differences[..differences.len().min(12)]
            );
        }

        let root = std::env::temp_dir().join(format!(
            "citygame-authentic-cty-roundtrip-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let saved = root.join(format!("{city_name}-roundtrip.sc2"));
        state.city.save(&saved).expect("save imported city");
        let mut reopened = crate::city::City::load(&saved).expect("reload imported save");
        reopened.prepare_pe_fidelity_from_saved_city();
        assert_eq!(reopened.saved_date(), state.city.saved_date());
        assert_eq!(reopened.year_founded(), state.city.year_founded());
        assert_eq!(reopened.year_founded(), Some(1900));
        let reloaded_planes = collect_planes(&reopened);
        for name in ["XBLD", "XZON", "XBIT", "ALTM", "XTXT"] {
            assert_eq!(
                reloaded_planes[name],
                expected_bytes(name),
                "reloaded {name}"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
fn open_direct_city_document(
    state: &mut ViewerState,
    path: &std::path::Path,
    size: (u32, u32),
    scale: f32,
    music_callback: impl FnMut(&mut ViewerState),
    finish: impl FnOnce(&ViewerState),
) -> Result<()> {
    open_city_document_with_title_tail(
        state,
        path,
        size,
        scale,
        false,
        false,
        false,
        music_callback,
        finish,
    )
}

fn open_city_document_with_title_tail(
    state: &mut ViewerState,
    path: &std::path::Path,
    size: (u32, u32),
    scale: f32,
    title_tail: bool,
    scenario_open: bool,
    classic_cty_conversion_approved: bool,
    mut music_callback: impl FnMut(&mut ViewerState),
    finish: impl FnOnce(&ViewerState),
) -> Result<()> {
    if scenario_open {
        state.load_win_scenario(path, size, scale)?;
    } else if classic_cty_conversion_approved {
        state.load_confirmed_classic_cty(path, size, scale)?;
    } else {
        state.load_city(path, size, scale)?;
    }
    // CODE 4 enters CODE 2's city-window mode after restoring SCDH.
    music_callback(state);
    finish(state);
    // CODE 5's title-choice handler services music after its load branch.
    if title_tail {
        music_callback(state);
    }
    Ok(())
}
