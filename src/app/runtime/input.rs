//! Input dispatch for the main window and floating/docked Tools palette.

use super::super::CityInfoDialog;
use super::super::map_interaction::{
    handle_map_left_pointer_down, release_map_tools, update_map_paint_pointer_motion,
};
use super::*;

pub(super) struct InputState {
    pub(super) running: bool,
    pub(super) return_to_title: bool,
    pub(super) dragging: bool,
    pub(super) painting: bool,
    pub(super) paint_tile: Option<(usize, usize)>,
    pub(super) shift_down: bool,
    pub(super) mouse_pos: Vec2,
    pub(super) touch: TouchTracker,
    pub(super) map_touch: MapTouchArbiter,
    pub(super) docked_palette: DockedPalette,
    info_preview: Option<(Vec2, Vec2, Vec2, Vec2, bool)>,
    pub(super) lifecycle: AppLifecycle,
}

impl InputState {
    pub(super) fn new() -> Self {
        Self {
            running: true,
            return_to_title: false,
            dragging: false,
            painting: false,
            paint_tile: None,
            shift_down: false,
            mouse_pos: Vec2::ZERO,
            touch: TouchTracker::new(),
            map_touch: MapTouchArbiter::new(),
            docked_palette: DockedPalette::new(),
            info_preview: None,
            lifecycle: AppLifecycle::new(),
        }
    }
}

impl InputState {
    fn begin_info_preview(&mut self, state: &mut ViewerState, action: UiAction, screen: Vec2, tools_origin: Vec2, size: Vec2) {
        if matches!(action, UiAction::OpenCityMap | UiAction::OpenCityGraphs | UiAction::OpenCityPopulation | UiAction::OpenCityIndustry | UiAction::OpenCityNeighbors) && state.city_info_dialog.is_some() {
            let origin = Vec2::new(tools_origin.x + size.x + 8.0, screen.y - 12.0);
            state.info_panel_origin = Some(origin);
            state.info_panel_scale = 0.5;
            self.info_preview = Some((screen, origin, tools_origin, size, false));
        }
    }

    fn move_info_preview(&mut self, state: &mut ViewerState, screen: Vec2) -> bool {
        if let Some((start, origin, toolbar, size, ref mut detached)) = self.info_preview {
            let delta = screen - start;
            let outside = screen.x < toolbar.x || screen.y < toolbar.y || screen.x >= toolbar.x + size.x || screen.y >= toolbar.y + size.y;
            if outside && delta.length() > 8.0 { *detached = true; }
            if *detached { state.info_panel_origin = Some(origin + delta); }
            true
        } else { false }
    }

    fn end_info_preview(&mut self, state: &mut ViewerState) -> bool {
        if let Some((_, _, _, _, detached)) = self.info_preview.take() {
            if !detached { state.close_city_info_dialog(); }
            true
        } else { false }
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    use crate::app::tests::test_state;
    use crate::city::City;

    #[test]
    fn info_preview_closes_on_release_and_stays_when_dragged() {
        let city = City::new_blank("/tmp/preview.sc2", "Preview").unwrap();
        let mut state = test_state(city);
        let mut input = InputState::new();
        let start = Vec2::new(100.0, 200.0);
        state.city_info_dialog = Some(CityInfoDialog::Graphs);
        input.begin_info_preview(&mut state, UiAction::OpenCityGraphs, start, Vec2::ZERO, Vec2::new(140.0, 624.0));
        assert_eq!(state.info_panel_origin, Some(Vec2::new(148.0, 188.0)));
        assert!(input.end_info_preview(&mut state));
        assert_eq!(state.city_info_dialog, None);
        state.city_info_dialog = Some(CityInfoDialog::Graphs);
        input.begin_info_preview(&mut state, UiAction::OpenCityGraphs, start, Vec2::ZERO, Vec2::new(140.0, 624.0));
        assert!(input.move_info_preview(&mut state, start + Vec2::new(70.0, 20.0)));
        assert!(input.end_info_preview(&mut state));
        assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Graphs));
        state.city_info_dialog = Some(CityInfoDialog::Budget);
        input.begin_info_preview(&mut state, UiAction::OpenBudget, start, Vec2::ZERO, Vec2::new(140.0, 624.0));
        assert!(!input.end_info_preview(&mut state));
        assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
        state.city_info_dialog = None;
        state.info_panel_scale = 0.5;
        state.info_panel_origin = Some(Vec2::new(30.0, 40.0));
        state.open_city_info_dialog(CityInfoDialog::Budget);
        assert_eq!(state.info_panel_scale, 1.0);
        assert_eq!(state.info_panel_origin, None);
    }
}

pub(super) fn handle_event(
    event: Event,
    mut windows: RuntimeWindows<'_, '_, '_>,
    state: &mut ViewerState,
    input: &mut InputState,
    sound: &mut Option<crate::audio::SoundPlayer>,
    city_music: &mut Option<crate::title_music::CityMusic>,
) -> Result<()> {
    #[cfg(target_os = "ios")]
    if crate::ios_documents::is_presenting()
        && matches!(
            &event,
            Event::MouseButtonDown { .. }
                | Event::MouseButtonUp { .. }
                | Event::MouseMotion { .. }
                | Event::MouseWheel { .. }
                | Event::FingerDown { .. }
                | Event::FingerMotion { .. }
                | Event::FingerUp { .. }
                | Event::KeyDown { .. }
                | Event::KeyUp { .. }
                | Event::TextInput { .. }
        )
    {
        return Ok(());
    }
    match event {
        // Finger* events already drive touch; ignore SDL's mirrored mouse events.
        Event::MouseButtonDown { which, .. }
        | Event::MouseButtonUp { which, .. }
        | Event::MouseMotion { which, .. }
        | Event::MouseWheel { which, .. }
            if touch_input::is_touch_synthesized_mouse(which) => {}
        Event::Quit { .. } | Event::AppTerminating { .. } => input.running = false,
        event @ (Event::AppDidEnterBackground { .. }
        | Event::AppWillEnterForeground { .. }
        | Event::Window { .. }) => {
            handle_window_event(event, &mut windows, state, input, sound, city_music);
        }
        event @ (Event::KeyDown { .. } | Event::KeyUp { .. } | Event::TextInput { .. }) => {
            handle_keyboard_event(event, &windows, state, input);
        }
        event @ (Event::MouseButtonDown { .. }
        | Event::MouseButtonUp { .. }
        | Event::MouseMotion { .. }
        | Event::MouseWheel { .. }) => {
            handle_mouse_event(event, &mut windows, state, input, sound)?;
        }
        event
        @ (Event::FingerDown { .. } | Event::FingerMotion { .. } | Event::FingerUp { .. }) => {
            handle_touch_event(event, &windows, state, input, sound);
        }
        _ => {}
    }
    Ok(())
}

fn handle_window_event(
    event: Event,
    windows: &mut RuntimeWindows<'_, '_, '_>,
    state: &mut ViewerState,
    input: &mut InputState,
    sound: &mut Option<crate::audio::SoundPlayer>,
    city_music: &mut Option<crate::title_music::CityMusic>,
) {
    let video = windows.video;
    let window = windows.window;
    let renderer = &mut *windows.renderer;
    let palette_window = windows.palette_window;
    let mut palette_renderer = windows.palette_renderer.as_deref_mut();
    let palette_id = windows.palette_id;
    let single_window = windows.single_window;
    match event {
        Event::AppDidEnterBackground { .. } => {
            state.city.set_mac_newspaper_suppressed(true);
            let paused = input.lifecycle.enter_background(state.traffic.speed());
            state.traffic.set_speed(paused);
            state.menu_checks.speed = paused;
            mac_menu::set_game_speed(paused);
            if let Some(player) = sound.as_mut() {
                player.set_suspended(true);
            }
            if let Some(player) = city_music.as_mut() {
                player.set_suspended(true);
            }
        }
        Event::AppWillEnterForeground { .. } => {
            state.city.set_mac_newspaper_suppressed(false);
            if let Some(speed) = input.lifecycle.enter_foreground() {
                state.traffic.set_speed(speed);
                state.menu_checks.speed = speed;
                mac_menu::set_game_speed(speed);
            }
            if let Some(player) = sound.as_mut() {
                player.set_suspended(false);
            }
            if let Some(player) = city_music.as_mut() {
                player.set_suspended(false);
            }
            sync_renderer_to_window(renderer, window);
            if let (Some(palette_renderer), Some(palette_window)) =
                (&mut palette_renderer, palette_window)
            {
                palette_window::sync_renderer_to_window(palette_renderer, palette_window);
            }
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::Minimized,
            ..
        } if window_id == window.id() => {
            let paused = input.lifecycle.enter_background(state.traffic.speed());
            state.traffic.set_speed(paused);
            state.menu_checks.speed = paused;
            mac_menu::set_game_speed(paused);
            if let Some(player) = sound.as_mut() {
                player.set_suspended(true);
            }
            if let Some(player) = city_music.as_mut() {
                player.set_suspended(true);
            }
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::Moved(_, _),
            ..
        } if window_id == window.id() && !single_window => {
            // Keep the floating Tools window under the in-game menubar on
            // X11/Windows; Wayland ignores programmatic placement.
            if video.current_video_driver() != "wayland" {
                if let Some(palette_window) = palette_window {
                    palette_window::anchor_floating_palette(video, window, palette_id);
                    palette_window::sync_palette_stacking(window, palette_window);
                }
            }
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::CloseRequested,
            ..
        } if window_id == palette_id => {
            // Tools palette is not closable (Classic Mac SC2K).
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::CloseRequested,
            ..
        } if window_id == window.id() => {
            input.running = false;
        }
        Event::Window {
            window_id,
            win_event:
                WindowEvent::Resized(width, height) | WindowEvent::PixelSizeChanged(width, height),
            ..
        } if window_id == palette_id && !single_window => {
            if let (Some(palette_renderer), Some(palette_window)) =
                (&mut palette_renderer, palette_window)
            {
                palette_window::sync_renderer_to_window(palette_renderer, palette_window);
                let (pixel_width, pixel_height) = palette_window.size_in_pixels();
                palette_renderer.resize(
                    pixel_width.max(width.max(1) as u32),
                    pixel_height.max(height.max(1) as u32),
                );
            }
        }
        Event::Window {
            window_id,
            win_event:
                WindowEvent::Resized(width, height) | WindowEvent::PixelSizeChanged(width, height),
            ..
        } if window_id == window.id() => {
            sync_renderer_to_window(renderer, window);
            let old_size = renderer.size();
            let old_scale = pixel_scale(window, old_size);
            let center_map_units =
                state.screen_to_map_units(viewport_center(old_size), old_size, old_scale);
            let (pixel_width, pixel_height) = window.size_in_pixels();
            renderer.resize(
                pixel_width.max(width.max(1) as u32),
                pixel_height.max(height.max(1) as u32),
            );
            let new_size = renderer.size();
            let new_scale = pixel_scale(window, new_size);
            state.place_map_units_at_screen(
                center_map_units,
                viewport_center(new_size),
                new_size,
                new_scale,
            );
            if let Some(palette_window) = palette_window {
                palette_window::sync_palette_stacking(window, palette_window);
            }
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::Maximized | WindowEvent::Restored | WindowEvent::Exposed,
            ..
        } if window_id == palette_id && !single_window => {
            if let (Some(palette_renderer), Some(palette_window)) =
                (&mut palette_renderer, palette_window)
            {
                palette_window::sync_renderer_to_window(palette_renderer, palette_window);
                let (pixel_width, pixel_height) = palette_window.size_in_pixels();
                palette_renderer.resize(pixel_width.max(1), pixel_height.max(1));
                palette_window::sync_palette_stacking(window, palette_window);
            }
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::FocusGained,
            ..
        } if window_id == palette_id && !single_window => {
            if let Some(palette_window) = palette_window {
                palette_window::set_app_active(video, palette_window, true);
                palette_window::sync_palette_stacking(window, palette_window);
            }
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::FocusLost,
            ..
        } if !single_window && (window_id == window.id() || window_id == palette_id) => {
            if let Some(palette_window) = palette_window {
                palette_window::set_app_active(
                    video,
                    palette_window,
                    window.has_input_focus() || palette_window.has_input_focus(),
                );
            }
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::FocusGained,
            ..
        } if window_id == window.id() => {
            if let Some(palette_window) = palette_window {
                palette_window::set_app_active(video, palette_window, true);
                palette_window::sync_palette_stacking(window, palette_window);
            }
            mac_menu::focus_main_window(window);
            sync_renderer_to_window(renderer, window);
            state.invalidate_sprite_cache();
        }
        Event::Window {
            window_id,
            win_event: WindowEvent::Maximized | WindowEvent::Restored | WindowEvent::Exposed,
            ..
        } if window_id == window.id() => {
            if let Some(speed) = input.lifecycle.enter_foreground() {
                state.traffic.set_speed(speed);
                state.menu_checks.speed = speed;
                mac_menu::set_game_speed(speed);
                if let Some(player) = sound.as_mut() {
                    player.set_suspended(false);
                }
                if let Some(player) = city_music.as_mut() {
                    player.set_suspended(false);
                }
            }
            sync_renderer_to_window(renderer, window);
            let old_size = renderer.size();
            let old_scale = pixel_scale(window, old_size);
            let center_map_units =
                state.screen_to_map_units(viewport_center(old_size), old_size, old_scale);
            let (pixel_width, pixel_height) = window.size_in_pixels();
            renderer.resize(pixel_width.max(1), pixel_height.max(1));
            let new_size = renderer.size();
            let new_scale = pixel_scale(window, new_size);
            state.place_map_units_at_screen(
                center_map_units,
                viewport_center(new_size),
                new_size,
                new_scale,
            );
            state.invalidate_sprite_cache();
            if let Some(palette_window) = palette_window {
                palette_window::sync_palette_stacking(window, palette_window);
            }
        }
        _ => {}
    }
}

fn handle_keyboard_event(
    event: Event,
    windows: &RuntimeWindows<'_, '_, '_>,
    state: &mut ViewerState,
    input: &mut InputState,
) {
    let video = windows.video;
    let window = windows.window;
    let renderer = &*windows.renderer;
    if let Event::KeyDown { keymod, .. } | Event::KeyUp { keymod, .. } = event {
        state.modifiers = keymod;
    }
    match event {
        Event::KeyDown {
            keycode: Some(Keycode::LShift | Keycode::RShift),
            ..
        } => {
            input.shift_down = true;
            if state.cancel_active_drag() {
                input.painting = false;
                input.paint_tile = None;
            }
        }
        Event::KeyUp {
            keycode: Some(Keycode::LShift | Keycode::RShift),
            ..
        } => {
            input.shift_down = false;
        }
        Event::KeyDown {
            keycode: Some(Keycode::Escape),
            ..
        } => {
            if state.sign_dialog.is_some() {
                state.cancel_sign_dialog();
                video.text_input().stop(window);
            } else if state.military_base_dialog.is_some() {
                if state.engine_fidelity.uses_win95() {
                    state.decline_military_base_offer();
                } else {
                    state.accept_military_base_offer();
                }
            } else if state.power_plant_dialog.is_some() {
                state.close_power_plant_dialog();
            } else if state.arcology_dialog.is_some() {
                state.close_arcology_dialog();
            } else if state.city_info_dialog.is_some() {
                super::super::dialog_input::close_city_info_dialog_from_input(state, window);
            }
        }
        Event::KeyDown {
            keycode: Some(Keycode::Return | Keycode::KpEnter),
            ..
        } if state.mac_loop_scheduler.january_budget_review_pending
            && state.city_info_dialog == Some(CityInfoDialog::Budget) =>
        {
            state.close_city_info_dialog();
        }
        Event::KeyDown {
            keycode: Some(Keycode::Return),
            ..
        } if state.military_base_dialog.is_some() && !state.engine_fidelity.uses_win95() => {
            state.accept_military_base_offer();
        }
        Event::KeyDown {
            keycode: Some(Keycode::Return),
            ..
        } if state.sign_dialog.is_some() => {
            state.commit_sign_dialog();
            video.text_input().stop(window);
        }
        Event::KeyDown {
            keycode: Some(Keycode::Backspace),
            ..
        } if state.sign_dialog.is_some() => state.backspace_sign_dialog_text(),
        Event::KeyDown { .. } if state.sign_dialog.is_some() => {}
        Event::TextInput {
            window_id, text, ..
        } if window_id == window.id() && state.sign_dialog.is_some() => {
            state.append_sign_dialog_text(&text);
        }
        Event::KeyDown {
            keycode: Some(key),
            keymod,
            ..
        } if MenubarUi::is_enabled() && windows_menu_shortcut(key, keymod).is_some() => {
            push_command(windows_menu_shortcut(key, keymod).expect("matched window shortcut"));
        }
        // Mac 68k map keys: '=' rotates the view CCW; Shift+'=' ('+') and numpad '+' zoom
        // in; '-'/numpad '-' zoom out. This is the single source of truth for '=' (the
        // native menu intentionally has no '=' accelerator) so it never collides with zoom.
        Event::KeyDown {
            keycode: Some(Keycode::Equals),
            keymod: modstate,
            ..
        } => {
            if modstate.contains(sdl3::keyboard::Mod::LSHIFTMOD)
                || modstate.contains(sdl3::keyboard::Mod::RSHIFTMOD)
            {
                state.zoom_in_at(
                    viewport_center(renderer.size()),
                    renderer.size(),
                    pixel_scale(window, renderer.size()),
                );
            } else {
                push_command(AppCommand::RotateView(-1));
            }
        }
        Event::KeyDown {
            keycode: Some(Keycode::Plus | Keycode::KpPlus),
            ..
        } => state.zoom_in_at(
            viewport_center(renderer.size()),
            renderer.size(),
            pixel_scale(window, renderer.size()),
        ),
        Event::KeyDown {
            keycode: Some(Keycode::Minus | Keycode::KpMinus),
            ..
        } => state.zoom_out_at(
            viewport_center(renderer.size()),
            renderer.size(),
            pixel_scale(window, renderer.size()),
        ),
        Event::KeyDown {
            keycode: Some(Keycode::_0 | Keycode::Kp0),
            ..
        } => state.actual_size_at(
            viewport_center(renderer.size()),
            renderer.size(),
            pixel_scale(window, renderer.size()),
        ),
        Event::KeyDown {
            keycode: Some(Keycode::C),
            ..
        } => state.center_map(renderer.size(), pixel_scale(window, renderer.size())),
        Event::KeyDown {
            keycode: Some(Keycode::G),
            ..
        } => state.show_grid = !state.show_grid,
        Event::KeyDown {
            keycode: Some(Keycode::Z),
            ..
        } => state.show_zone_overlay = !state.show_zone_overlay,
        Event::KeyDown {
            keycode: Some(Keycode::T),
            ..
        } => {
            state.show_traffic = !state.show_traffic;
            state.log_traffic_stats(renderer);
        }
        Event::KeyDown {
            keycode: Some(Keycode::U),
            ..
        } if state.mode == AppMode::CityView => {
            state.toggle_underground_view();
        }
        Event::KeyDown {
            keycode: Some(Keycode::F),
            ..
        } => state.fit_map(renderer.size(), pixel_scale(window, renderer.size())),
        Event::KeyDown {
            keycode: Some(Keycode::I),
            ..
        } => push_command(AppCommand::InspectHoverTile),
        Event::KeyDown {
            keycode: Some(Keycode::O),
            ..
        } => push_command(AppCommand::OpenCityDialog),
        Event::KeyDown {
            keycode: Some(Keycode::B),
            ..
        } => state.set_tool(video, window.id(), EditTool::Bulldoze),
        Event::KeyDown {
            keycode: Some(Keycode::R),
            ..
        } if state.mode == AppMode::CityView => {
            state.set_tool(video, window.id(), EditTool::Road);
        }
        Event::KeyDown {
            keycode: Some(Keycode::P),
            ..
        } => push_command(AppCommand::SetSpeed(GameSpeed::Paused)),
        Event::KeyDown {
            keycode: Some(Keycode::_1),
            ..
        } => push_command(AppCommand::SetSpeed(GameSpeed::Turtle)),
        Event::KeyDown {
            keycode: Some(Keycode::_2),
            ..
        } => push_command(AppCommand::SetSpeed(GameSpeed::Llama)),
        Event::KeyDown {
            keycode: Some(Keycode::_3),
            ..
        } => push_command(AppCommand::SetSpeed(GameSpeed::Cheetah)),
        Event::KeyDown {
            keycode: Some(Keycode::_4),
            ..
        } => push_command(AppCommand::SetSpeed(GameSpeed::AfricanSwallow)),
        Event::KeyDown {
            keycode: Some(Keycode::N),
            keymod: modstate,
            ..
        } if modstate.contains(sdl3::keyboard::Mod::LCTRLMOD)
            || modstate.contains(sdl3::keyboard::Mod::RCTRLMOD)
            || modstate.contains(sdl3::keyboard::Mod::LGUIMOD)
            || modstate.contains(sdl3::keyboard::Mod::RGUIMOD) =>
        {
            push_command(AppCommand::NewCity);
        }
        Event::KeyDown {
            keycode: Some(Keycode::S),
            keymod: modstate,
            ..
        } if modstate.contains(sdl3::keyboard::Mod::LCTRLMOD)
            || modstate.contains(sdl3::keyboard::Mod::RCTRLMOD)
            || modstate.contains(sdl3::keyboard::Mod::LGUIMOD)
            || modstate.contains(sdl3::keyboard::Mod::RGUIMOD) =>
        {
            push_command(AppCommand::SaveCity);
        }
        Event::KeyDown {
            keycode: Some(Keycode::Left),
            ..
        } => state.camera.x += 48.0 * input_pixel_scale(window, renderer.size()).x,
        Event::KeyDown {
            keycode: Some(Keycode::Right),
            ..
        } => state.camera.x -= 48.0 * input_pixel_scale(window, renderer.size()).x,
        Event::KeyDown {
            keycode: Some(Keycode::Up),
            ..
        } => state.camera.y += 32.0 * input_pixel_scale(window, renderer.size()).y,
        Event::KeyDown {
            keycode: Some(Keycode::Down),
            ..
        } => state.camera.y -= 32.0 * input_pixel_scale(window, renderer.size()).y,
        _ => {}
    }
}

fn handle_mouse_event(
    event: Event,
    windows: &mut RuntimeWindows<'_, '_, '_>,
    state: &mut ViewerState,
    input: &mut InputState,
    sound: &Option<crate::audio::SoundPlayer>,
) -> Result<()> {
    let video = windows.video;
    let window = windows.window;
    let renderer = &mut *windows.renderer;
    let palette_window = windows.palette_window;
    let mut palette_renderer = windows.palette_renderer.as_deref_mut();
    let palette_id = windows.palette_id;
    let palette_logical_size = &mut *windows.palette_logical_size;
    let single_window = windows.single_window;
    match event {
        Event::MouseButtonDown {
            window_id,
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } if window_id == palette_id && !single_window => {
            let mut need_resync = false;
            if let (Some(palette_window), Some(palette_renderer)) =
                (palette_window, &palette_renderer)
            {
                if let Some((slider, value)) = palette_window::palette_mouse_slider(
                    palette_window,
                    palette_renderer,
                    *palette_logical_size,
                    state.mode,
                    state.open_tool_menu,
                    x,
                    y,
                ) {
                    state.close_tool_menu();
                    state.edit_map_slider_drag = Some(slider);
                    state.set_edit_map_slider(slider, value);
                    state.play_click_sound(sound.as_ref());
                    state.update_window_title(video, window.id());
                } else if let Some(action) = palette_window::palette_mouse_action(
                    palette_window,
                    palette_renderer,
                    *palette_logical_size,
                    state.mode,
                    state.open_tool_menu,
                    x,
                    y,
                ) {
                    if state.ui_action_enabled(action) {
                        state.press_palette_button(action);
                        state.play_click_sound(sound.as_ref());
                        need_resync = matches!(action, UiAction::DoneEditMap);
                        match action {
                            UiAction::OpenMenu(menu) => {
                                state.open_tool_menu(menu);
                                state.update_window_title(video, window.id());
                            }
                            _ => {
                                state.close_tool_menu();
                                state.handle_ui_action(action, video, window.id());
                                // The floating Tools palette can hold key focus on
                                // macOS. Return focus to the map as soon as a leaf
                                // tool/action is chosen, so the next map press is a
                                // gameplay input instead of the window-activation click.
                                mac_menu::focus_main_window(window);
                                palette_window::sync_palette_stacking(window, palette_window);
                            }
                        }
                    }
                } else {
                    state.close_tool_menu();
                }
            }
            if need_resync {
                if let Some(palette_renderer) = &mut palette_renderer {
                    *palette_logical_size = palette_window::sync_for_mode_ex(
                        video,
                        palette_id,
                        window,
                        palette_renderer,
                        state.mode,
                        true,
                    )?;
                }
            }
        }
        Event::MouseButtonDown {
            window_id,
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } if window_id == window.id() => {
            mac_menu::focus_main_window(window);
            let screen = Vec2::new(x, y) * input_pixel_scale(window, renderer.size());
            input.mouse_pos = screen;
            match state.menubar.handle_click(
                screen,
                renderer.size(),
                pixel_scale(window, renderer.size()),
                state.menu_checks,
            ) {
                MenubarClick::Command(id) => {
                    if let Some(command) = app_command_for_menu_command(id) {
                        push_command(command);
                    }
                    input.painting = false;
                    input.dragging = false;
                    return Ok(());
                }
                MenubarClick::Consumed => {
                    input.painting = false;
                    input.dragging = false;
                    return Ok(());
                }
                MenubarClick::Ignored => {}
            }
            if state.begin_info_panel_drag(screen, renderer.size()) {
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            if handle_modal_pointer_down(
                state,
                screen,
                renderer.size(),
                pixel_scale(window, renderer.size()),
                video,
                window,
                sound.as_ref(),
            ) {
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            if state.begin_status_panel_drag(
                screen,
                renderer.size(),
                pixel_scale(window, renderer.size()),
            ) {
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            if single_window {
                let ps = pixel_scale(window, renderer.size());
                let menubar_top = MenubarUi::occupied_top(ps);
                if input
                    .docked_palette
                    .begin_drag(screen, menubar_top, ps, *palette_logical_size)
                {
                    input.painting = false;
                    input.dragging = false;
                    return Ok(());
                }
                let tools_origin = input.docked_palette.tools_origin(menubar_top, ps);
                let hit_pad = platform_ui::touch_hit_pad(ps);
                if let Some((slider, value)) = ui::edit_map_slider_hit_test(
                    screen - tools_origin,
                    *palette_logical_size,
                    ps,
                    state.mode,
                    state.open_tool_menu,
                ) {
                    state.close_tool_menu();
                    state.edit_map_slider_drag = Some(slider);
                    state.set_edit_map_slider(slider, value);
                    state.play_click_sound(sound.as_ref());
                    state.update_window_title(video, window.id());
                    input.painting = false;
                    input.dragging = false;
                    return Ok(());
                } else if let Some(action) = ui::palette_hit_test_at(
                    screen,
                    tools_origin,
                    *palette_logical_size,
                    ps,
                    state.mode,
                    state.open_tool_menu,
                    hit_pad,
                ) {
                    if state.ui_action_enabled(action) {
                        state.press_palette_button(action);
                        state.play_click_sound(sound.as_ref());
                        let resync = matches!(action, UiAction::DoneEditMap);
                        match action {
                            UiAction::OpenMenu(menu) => {
                                state.open_tool_menu(menu);
                                state.update_window_title(video, window.id());
                            }
                            _ => {
                                state.close_tool_menu();
                                state.handle_ui_action(action, video, window.id());
                                input.begin_info_preview(state, action, screen, tools_origin, Vec2::new(palette_logical_size.0 as f32 * ps, palette_logical_size.1 as f32 * ps));
                            }
                        }
                        if resync {
                            *palette_logical_size = ui::palette_logical_size(state.mode);
                        }
                    }
                    input.painting = false;
                    input.dragging = false;
                    return Ok(());
                } else if input.docked_palette.point_in_chrome(
                    screen,
                    menubar_top,
                    ps,
                    *palette_logical_size,
                ) {
                    state.close_tool_menu();
                    input.painting = false;
                    input.dragging = false;
                    return Ok(());
                }
            }
            handle_map_left_pointer_down(
                state,
                screen,
                renderer.size(),
                pixel_scale(window, renderer.size()),
                &mut input.painting,
                &mut input.paint_tile,
                input.shift_down,
                video,
                window,
                sound.as_ref(),
            );
        }
        Event::MouseButtonDown {
            window_id,
            mouse_btn: MouseButton::Right,
            x,
            y,
            ..
        } if window_id == window.id() => {
            let screen = Vec2::new(x, y) * input_pixel_scale(window, renderer.size());
            input.mouse_pos = screen;
            if state.sign_dialog.is_some() {
                state.cancel_sign_dialog();
                video.text_input().stop(window);
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            if state.military_base_dialog.is_some() {
                state.decline_military_base_offer();
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            if state.power_plant_dialog.is_some() {
                state.close_power_plant_dialog();
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            if state.arcology_dialog.is_some() {
                state.close_arcology_dialog();
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            if state.city_info_dialog.is_some_and(|dialog| {
                point_in_float_rect(state.info_panel_input_point(screen, renderer.size()), city_info_dialog_rect_for(dialog, renderer.size()))
            }) {
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            if state.point_in_status_panel(
                screen,
                renderer.size(),
                pixel_scale(window, renderer.size()),
            ) {
                return Ok(());
            }
            state.update_hover_tile(
                screen,
                renderer.size(),
                pixel_scale(window, renderer.size()),
            );
            state.cancel_zone_drag();
            state.cancel_network_drag();
            state.cancel_underground_drag();
            state.cancel_highway_drag();
            state.cancel_terrain_drag();
            state.cancel_edit_map_stretch_drag();
            state.cancel_edit_map_level_drag();
            state.cancel_edit_map_height_hold();
            state.cancel_edit_map_spread_hold();
            state.cancel_nature_drag();
            input.paint_tile = None;
            input.painting = false;
            input.dragging = true;
        }
        Event::MouseButtonUp {
            window_id,
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } if window_id == window.id() && state.military_base_dialog.is_some() => {
            let screen = Vec2::new(x, y) * input_pixel_scale(window, renderer.size());
            input.mouse_pos = screen;
            state.handle_military_base_dialog_mouse_up(military_base_dialog_hit_test(
                screen,
                renderer.size(),
            ));
            input.dragging = false;
            input.painting = false;
            state.release_palette_button();
            state.update_window_title(video, window.id());
        }
        Event::MouseButtonUp {
            window_id,
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } if window_id == window.id() && state.power_plant_dialog.is_some() => {
            let screen = Vec2::new(x, y) * input_pixel_scale(window, renderer.size());
            input.mouse_pos = screen;
            state.handle_power_plant_dialog_mouse_up(
                power_plant_dialog_hit_test(screen, renderer.size()),
                video,
                window.id(),
            );
            input.dragging = false;
            input.painting = false;
            state.release_palette_button();
            state.update_window_title(video, window.id());
        }
        Event::MouseButtonUp {
            window_id,
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } if window_id == window.id() && state.arcology_dialog.is_some() => {
            let screen = Vec2::new(x, y) * input_pixel_scale(window, renderer.size());
            input.mouse_pos = screen;
            state.handle_arcology_dialog_mouse_up(
                arcology_dialog_hit_test(screen, renderer.size()),
                video,
                window.id(),
            );
            input.dragging = false;
            input.painting = false;
            state.release_palette_button();
            state.update_window_title(video, window.id());
        }
        Event::MouseButtonUp {
            mouse_btn: MouseButton::Left,
            ..
        } => {
            if input.end_info_preview(state) {
                state.release_palette_button();
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            if state.info_panel_drag.take().is_some() {
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
            state.end_status_panel_drag();
            if input.docked_palette.is_dragging() {
                input.docked_palette.end_drag();
                input.dragging = false;
                input.painting = false;
                return Ok(());
            }
            input.dragging = false;
            release_map_tools(state, input.shift_down, window, sound.as_ref());
            input.painting = false;
            input.paint_tile = None;
            state.release_palette_button();
            state.update_window_title(video, window.id());
        }
        Event::MouseButtonUp {
            mouse_btn: MouseButton::Right,
            ..
        } => {
            input.dragging = false;
            input.painting = false;
        }
        Event::MouseMotion {
            window_id, x, y, ..
        } if window_id == palette_id && !single_window => {
            if let (Some(palette_window), Some(palette_renderer)) =
                (palette_window, &palette_renderer)
            {
                if let Some(active_slider) = state.edit_map_slider_drag
                    && let Some((slider, value)) = palette_window::palette_mouse_slider(
                        palette_window,
                        palette_renderer,
                        *palette_logical_size,
                        state.mode,
                        state.open_tool_menu,
                        x,
                        y,
                    )
                {
                    state.set_edit_map_slider(
                        if slider == active_slider {
                            slider
                        } else {
                            active_slider
                        },
                        value,
                    );
                    state.update_window_title(video, window.id());
                }
                let label = palette_window::palette_hover_label(
                    palette_window,
                    palette_renderer,
                    *palette_logical_size,
                    state.mode,
                    state.open_tool_menu,
                    x,
                    y,
                );
                if state.set_palette_balloon_help(label) {
                    state.update_window_title(video, window.id());
                }
            }
        }
        Event::MouseMotion {
            window_id,
            x,
            y,
            xrel,
            yrel,
            ..
        } if window_id == window.id() => {
            if state.set_palette_balloon_help(None) {
                state.update_window_title(video, window.id());
            }
            input.mouse_pos = Vec2::new(x, y) * input_pixel_scale(window, renderer.size());
            if input.move_info_preview(state, input.mouse_pos) { return Ok(()); }
            if state.drag_info_panel(input.mouse_pos, renderer.size()) {
                return Ok(());
            }
            if state.drag_status_panel(input.mouse_pos) {
                return Ok(());
            }
            if single_window && input.docked_palette.is_dragging() {
                let ps = pixel_scale(window, renderer.size());
                input.docked_palette.drag_to(
                    input.mouse_pos,
                    renderer.size(),
                    MenubarUi::occupied_top(ps),
                    ps,
                    *palette_logical_size,
                );
                return Ok(());
            }
            if state.point_in_status_panel(
                input.mouse_pos,
                renderer.size(),
                pixel_scale(window, renderer.size()),
            ) {
                return Ok(());
            }
            update_map_paint_pointer_motion(
                state,
                input.mouse_pos,
                renderer.size(),
                pixel_scale(window, renderer.size()),
                input.painting,
                &mut input.paint_tile,
                input.shift_down,
                video,
                window,
            );
            if !input.painting && input.dragging {
                state.camera += Vec2::new(xrel, yrel) * input_pixel_scale(window, renderer.size());
            }
        }
        Event::MouseWheel {
            y,
            direction,
            mouse_x,
            mouse_y,
            ..
        } if y != 0.0 => {
            let factor = wheel_zoom_factor(y, direction);
            let anchor = Vec2::new(mouse_x, mouse_y) * input_pixel_scale(window, renderer.size());
            state.zoom_by_at(
                factor,
                anchor,
                renderer.size(),
                pixel_scale(window, renderer.size()),
            );
        }
        _ => {}
    }
    Ok(())
}

fn handle_touch_event(
    event: Event,
    windows: &RuntimeWindows<'_, '_, '_>,
    state: &mut ViewerState,
    input: &mut InputState,
    sound: &Option<crate::audio::SoundPlayer>,
) {
    let video = windows.video;
    let window = windows.window;
    let renderer = &*windows.renderer;
    let palette_logical_size = *windows.palette_logical_size;
    let single_window = windows.single_window;
    match event {
        Event::FingerDown {
            window_id,
            finger_id,
            x,
            y,
            ..
        } if window_id == window.id() => {
            let screen = touch_input::finger_to_screen(x, y, renderer.size());
            input.mouse_pos = screen;
            input.touch.on_down(finger_id, screen);
            if input.touch.finger_count() >= 2 {
                for action in input.map_touch.on_second_finger() {
                    apply_map_touch_action(
                        action,
                        state,
                        &mut input.painting,
                        &mut input.paint_tile,
                        &mut input.dragging,
                        input.shift_down,
                        video,
                        window,
                        sound.as_ref(),
                        renderer.size(),
                        pixel_scale(window, renderer.size()),
                    );
                }
                input.painting = false;
                input.dragging = false;
            } else if input.touch.finger_count() == 1 {
                input.map_touch.clear();
                let ps = pixel_scale(window, renderer.size());
                if state.begin_info_panel_drag(screen, renderer.size()) {
                    input.painting = false;
                    input.dragging = false;
                } else if handle_modal_pointer_down(
                    state,
                    screen,
                    renderer.size(),
                    ps,
                    video,
                    window,
                    sound.as_ref(),
                ) {
                    input.painting = false;
                    input.dragging = false;
                } else {
                    match state
                        .menubar
                        .handle_click(screen, renderer.size(), ps, state.menu_checks)
                    {
                        MenubarClick::Command(id) => {
                            if let Some(command) = app_command_for_menu_command(id) {
                                push_command(command);
                            }
                            input.painting = false;
                            input.dragging = false;
                        }
                        MenubarClick::Consumed => {
                            input.painting = false;
                            input.dragging = false;
                        }
                        MenubarClick::Ignored => {
                            let mut ui_consumed =
                                state.begin_status_panel_drag(screen, renderer.size(), ps);
                            if single_window && !ui_consumed {
                                let menubar_top = MenubarUi::occupied_top(ps);
                                if input.docked_palette.begin_drag(
                                    screen,
                                    menubar_top,
                                    ps,
                                    palette_logical_size,
                                ) {
                                    ui_consumed = true;
                                } else {
                                    let tools_origin =
                                        input.docked_palette.tools_origin(menubar_top, ps);
                                    if let Some(action) = ui::palette_hit_test_at(
                                        screen,
                                        tools_origin,
                                        palette_logical_size,
                                        ps,
                                        state.mode,
                                        state.open_tool_menu,
                                        platform_ui::touch_hit_pad(ps),
                                    ) {
                                        if state.ui_action_enabled(action) {
                                            state.press_palette_button(action);
                                            state.play_click_sound(sound.as_ref());
                                            match action {
                                                UiAction::OpenMenu(menu) => {
                                                    state.open_tool_menu(menu);
                                                    state.update_window_title(video, window.id());
                                                }
                                                _ => {
                                                    state.close_tool_menu();
                                                    state.handle_ui_action(
                                                        action,
                                                        video,
                                                        window.id(),
                                                    );
                                                    input.begin_info_preview(state, action, screen, tools_origin, Vec2::new(palette_logical_size.0 as f32 * ps, palette_logical_size.1 as f32 * ps));
                                                }
                                            }
                                        }
                                        ui_consumed = true;
                                    } else if input.docked_palette.point_in_chrome(
                                        screen,
                                        menubar_top,
                                        ps,
                                        palette_logical_size,
                                    ) {
                                        state.close_tool_menu();
                                        ui_consumed = true;
                                    }
                                }
                            }
                            if ui_consumed {
                                input.painting = false;
                                input.dragging = false;
                            } else {
                                // Map gesture: decide tap / pan / paint later.
                                input.map_touch.begin_map_gesture(
                                    screen,
                                    tool_allows_touch_paint_stroke(state.mode, state.tool),
                                    ps,
                                );
                                input.painting = false;
                                input.dragging = false;
                            }
                        }
                    }
                }
            }
        }
        Event::FingerMotion {
            window_id,
            finger_id,
            x,
            y,
            ..
        } if window_id == window.id() => {
            let screen = touch_input::finger_to_screen(x, y, renderer.size());
            input.mouse_pos = screen;
            let ps = pixel_scale(window, renderer.size());
            if input.move_info_preview(state, screen) {
                let _ = input.touch.on_motion(finger_id, screen);
            } else if state.drag_info_panel(screen, renderer.size()) {
                let _ = input.touch.on_motion(finger_id, screen);
            } else if state.drag_status_panel(screen) {
                let _ = input.touch.on_motion(finger_id, screen);
            } else if input.docked_palette.is_dragging() {
                input.docked_palette.drag_to(
                    screen,
                    renderer.size(),
                    MenubarUi::occupied_top(ps),
                    ps,
                    palette_logical_size,
                );
                let _ = input.touch.on_motion(finger_id, screen);
            } else {
                let finger_count = input.touch.finger_count();
                for action in input.map_touch.on_motion(screen, finger_count) {
                    apply_map_touch_action(
                        action,
                        state,
                        &mut input.painting,
                        &mut input.paint_tile,
                        &mut input.dragging,
                        input.shift_down,
                        video,
                        window,
                        sound.as_ref(),
                        renderer.size(),
                        ps,
                    );
                }
                if let Some(gesture) = input.touch.on_motion(finger_id, screen) {
                    match gesture {
                        TouchGesture::Pan(delta) => {
                            // One-finger pan only after arbiter commits Panning;
                            // two-finger pan while pinching.
                            let allow = input.map_touch.phase()
                                == touch_input::MapTouchPhase::Panning
                                || input.map_touch.phase() == touch_input::MapTouchPhase::Pinching
                                || finger_count >= 2;
                            let modal_open = state.menubar.is_open()
                                || state.city_info_dialog.is_some()
                                || state.sign_dialog.is_some()
                                || state.power_plant_dialog.is_some()
                                || state.arcology_dialog.is_some()
                                || state.military_base_dialog.is_some();
                            if allow && !modal_open && !input.painting {
                                state.camera += delta;
                            }
                        }
                        TouchGesture::Pinch { factor, anchor } => {
                            for action in input.map_touch.on_second_finger() {
                                apply_map_touch_action(
                                    action,
                                    state,
                                    &mut input.painting,
                                    &mut input.paint_tile,
                                    &mut input.dragging,
                                    input.shift_down,
                                    video,
                                    window,
                                    sound.as_ref(),
                                    renderer.size(),
                                    ps,
                                );
                            }
                            state.zoom_by_at(factor, anchor, renderer.size(), ps);
                            input.painting = false;
                            input.dragging = false;
                        }
                    }
                }
            }
        }
        Event::FingerUp {
            window_id,
            finger_id,
            ..
        } if window_id == window.id() => {
            input.touch.on_up(finger_id);
            if input.touch.finger_count() == 0 {
                if input.end_info_preview(state) {
                    input.painting = false;
                    input.dragging = false;
                    state.release_palette_button();
                    return;
                }
                let was_info_drag = state.info_panel_drag.take().is_some();
                state.end_status_panel_drag();
                if was_info_drag {
                    input.docked_palette.end_drag();
                    input.painting = false;
                    input.dragging = false;
                    return;
                }
                let ps = pixel_scale(window, renderer.size());
                let _ =
                    handle_modal_pointer_up(state, input.mouse_pos, renderer.size(), video, window);
                for action in input.map_touch.on_up() {
                    apply_map_touch_action(
                        action,
                        state,
                        &mut input.painting,
                        &mut input.paint_tile,
                        &mut input.dragging,
                        input.shift_down,
                        video,
                        window,
                        sound.as_ref(),
                        renderer.size(),
                        ps,
                    );
                }
                input.docked_palette.end_drag();
                input.dragging = false;
                if !input.painting {
                    state.release_palette_button();
                }
            }
        }

        _ => {}
    }
}

pub(super) fn tick_held_touch(
    windows: RuntimeWindows<'_, '_, '_>,
    state: &mut ViewerState,
    input: &mut InputState,
    sound: &Option<crate::audio::SoundPlayer>,
) {
    let video = windows.video;
    let window = windows.window;
    let renderer = &*windows.renderer;
    // Long-press paint commit while the finger is held still.
    if input.map_touch.phase() == touch_input::MapTouchPhase::Pending {
        let ps = pixel_scale(window, renderer.size());
        for action in input.map_touch.on_tick() {
            apply_map_touch_action(
                action,
                state,
                &mut input.painting,
                &mut input.paint_tile,
                &mut input.dragging,
                input.shift_down,
                video,
                window,
                sound.as_ref(),
                renderer.size(),
                ps,
            );
        }
    }
}
