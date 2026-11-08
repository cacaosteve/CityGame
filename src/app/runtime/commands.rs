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
    while let Some(command) = pop_command() {
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
                        if state.engine_fidelity.uses_win95()
                            && newspaper_subscription_enabled_for_city(&state.city)
                        {
                            "on"
                        } else {
                            "off"
                        }
                    );
                }
                AppCommand::ToggleNewspaperExtra => {
                    log::info!(
                        "newspaper extra: {}",
                        if state.engine_fidelity.uses_win95()
                            && newspaper_extra_enabled_for_city(&state.city)
                        {
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
        match command {
            AppCommand::OpenCityDialog => {
                open_city_dialog(window);
            }
            AppCommand::OpenScenarioDialog => {
                open_scenario_dialog(window, asset_profile);
            }
            AppCommand::OpenPath(path) | AppCommand::OpenTitlePath(path) => {
                state.load_city(&path, renderer.size(), pixel_scale(window, renderer.size()))?;
                if let Err(error) = asset_profile.remember_city(&path) {
                    log::warn!("could not remember city {}: {error:#}", path.display());
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
                if let Err(error) =
                    crate::ios_documents::save_city(window, snapshot, state.city.path.clone())
                {
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
                    if let Err(error) = asset_profile.remember_city(&path) {
                        log::warn!("could not remember saved city: {error:#}");
                    }
                    state.update_window_title(video, window.id());
                }
            }
            #[cfg(not(target_os = "ios"))]
            AppCommand::SaveCity => {
                let path = state.city.path.clone();
                let size = renderer.size();
                if let Err(error) = state.save_city(&path, size, pixel_scale(window, size)) {
                    log::error!("save failed: {error:#}");
                } else {
                    log::info!("saved city to {}", path.display());
                }
            }
            #[cfg(not(target_os = "ios"))]
            AppCommand::SaveCityAs => save_city_dialog(window),
            AppCommand::SaveToPath(path) => {
                let size = renderer.size();
                if let Err(error) = state.save_city(&path, size, pixel_scale(window, size)) {
                    log::error!("save failed: {error:#}");
                } else {
                    log::info!("saved city to {}", path.display());
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
            AppCommand::ToggleEditMap => {
                state.toggle_edit_mode(video, window.id());
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
            | AppCommand::SetSpeed(_)
            | AppCommand::SetTool(_)
            | AppCommand::ReturnToTitle
            | AppCommand::RotateView(_) => {
                unreachable!("state command should have been handled before runtime match")
            }
        }
    }
    Ok(())
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
