//! Input dispatch for the main window and floating/docked Tools palette.

use super::super::CityInfoDialog;
use super::super::dialog_content::CityInfoDialogAction;
use super::super::map_interaction::{
    handle_map_left_pointer_down, release_map_tools, update_map_paint_pointer_motion,
};
use super::*;
use crate::app::dialogs::{
    apply_city_info_dialog_action, city_info_dialog_rect_for_engine,
    mac_budget_review_action_for_release,
};

pub(super) struct InputState {
    pub(super) running: bool,
    pub(super) return_to_title: bool,
    pub(super) close_save_pending: Option<Win95CloseAction>,
    pub(super) win95_file_dialog_pending: Option<Win95FileDialog>,
    pub(super) dragging: bool,
    pub(super) painting: bool,
    pub(super) paint_tile: Option<(usize, usize)>,
    pub(super) shift_down: bool,
    pub(super) mouse_pos: Vec2,
    mac_budget_review_pointer_down: Option<Vec2>,
    pub(super) touch: TouchTracker,
    pub(super) fiscal_notice_mouse_down: bool,
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
            close_save_pending: None,
            win95_file_dialog_pending: None,
            dragging: false,
            painting: false,
            paint_tile: None,
            shift_down: false,
            mouse_pos: Vec2::ZERO,
            mac_budget_review_pointer_down: None,
            touch: TouchTracker::new(),
            fiscal_notice_mouse_down: false,
            map_touch: MapTouchArbiter::new(),
            docked_palette: DockedPalette::new(),
            info_preview: None,
            lifecycle: AppLifecycle::new(),
        }
    }

    pub(super) fn blocking_file_dialog(&self) -> bool {
        self.close_save_pending.is_some() || self.win95_file_dialog_pending.is_some()
    }

    pub(super) fn holds_pe_idle_for_common_dialog(&self) -> bool {
        self.close_save_pending.is_some()
            || matches!(
                self.win95_file_dialog_pending,
                Some(Win95FileDialog::City | Win95FileDialog::SaveAs)
            )
    }

    pub(super) fn mac_budget_review_pointer_is_down(&self) -> bool {
        self.mac_budget_review_pointer_down.is_some()
    }

    /// Capture the initial press for the Mac annual budget modal. The native
    /// CODE 4 handler stays in `_StillDown` until the matching release.
    pub(super) fn mac_budget_review_pointer_down(
        &mut self,
        state: &mut ViewerState,
        screen: Vec2,
        window_size: (u32, u32),
    ) -> bool {
        self.mac_budget_review_pointer_down = None;
        if state.engine_fidelity == EngineFidelity::Mac68k
            && state.mac_loop_scheduler.january_budget_review_pending
            && state.city_info_dialog == Some(CityInfoDialog::Budget)
            && point_in_float_rect(
                screen,
                city_info_dialog_rect_for_engine(
                    CityInfoDialog::Budget,
                    window_size,
                    state.engine_fidelity,
                ),
            )
        {
            state.note_mac_january_budget_review_window_click();
            self.mac_budget_review_pointer_down = Some(screen);
            true
        } else {
            false
        }
    }

    /// Return the control activated by a release over the same control that
    /// was pressed, while the January review is still the active modal.
    pub(super) fn mac_budget_review_pointer_up(
        &mut self,
        state: &ViewerState,
        screen: Vec2,
        window_size: (u32, u32),
    ) -> Option<CityInfoDialogAction> {
        let down = self.mac_budget_review_pointer_down.take()?;
        if state.engine_fidelity != EngineFidelity::Mac68k
            || !state.mac_loop_scheduler.january_budget_review_pending
            || state.city_info_dialog != Some(CityInfoDialog::Budget)
        {
            return None;
        }
        mac_budget_review_action_for_release(down, screen, window_size)
            .filter(|action| state.city_info_dialog_action_enabled(*action))
    }
}

impl InputState {
    fn begin_info_preview(
        &mut self,
        state: &mut ViewerState,
        action: UiAction,
        screen: Vec2,
        tools_origin: Vec2,
        size: Vec2,
    ) {
        if matches!(
            action,
            UiAction::OpenCityMap
                | UiAction::OpenCityGraphs
                | UiAction::OpenCityPopulation
                | UiAction::OpenCityIndustry
                | UiAction::OpenCityNeighbors
        ) && state.city_info_dialog.is_some()
        {
            let origin = Vec2::new(tools_origin.x + size.x + 8.0, screen.y - 12.0);
            state.info_panel_origin = Some(origin);
            state.info_panel_scale = 0.5;
            self.info_preview = Some((screen, origin, tools_origin, size, false));
        }
    }

    fn move_info_preview(&mut self, state: &mut ViewerState, screen: Vec2) -> bool {
        if let Some((start, origin, toolbar, size, ref mut detached)) = self.info_preview {
            let delta = screen - start;
            let outside = screen.x < toolbar.x
                || screen.y < toolbar.y
                || screen.x >= toolbar.x + size.x
                || screen.y >= toolbar.y + size.y;
            if outside && delta.length() > 8.0 {
                *detached = true;
            }
            if *detached {
                state.info_panel_origin = Some(origin + delta);
            }
            true
        } else {
            false
        }
    }

    fn end_info_preview(&mut self, state: &mut ViewerState) -> bool {
        if let Some((_, _, _, _, detached)) = self.info_preview.take() {
            if !detached {
                state.close_city_info_dialog();
            }
            true
        } else {
            false
        }
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
        input.begin_info_preview(
            &mut state,
            UiAction::OpenCityGraphs,
            start,
            Vec2::ZERO,
            Vec2::new(140.0, 624.0),
        );
        assert_eq!(state.info_panel_origin, Some(Vec2::new(148.0, 188.0)));
        assert!(input.end_info_preview(&mut state));
        assert_eq!(state.city_info_dialog, None);
        state.city_info_dialog = Some(CityInfoDialog::Graphs);
        input.begin_info_preview(
            &mut state,
            UiAction::OpenCityGraphs,
            start,
            Vec2::ZERO,
            Vec2::new(140.0, 624.0),
        );
        assert!(input.move_info_preview(&mut state, start + Vec2::new(70.0, 20.0)));
        assert!(input.end_info_preview(&mut state));
        assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Graphs));
        state.city_info_dialog = Some(CityInfoDialog::Budget);
        input.begin_info_preview(
            &mut state,
            UiAction::OpenBudget,
            start,
            Vec2::ZERO,
            Vec2::new(140.0, 624.0),
        );
        assert!(!input.end_info_preview(&mut state));
        assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
        state.city_info_dialog = None;
        state.info_panel_scale = 0.5;
        state.info_panel_origin = Some(Vec2::new(30.0, 40.0));
        state.open_city_info_dialog(CityInfoDialog::Budget);
        assert_eq!(state.info_panel_scale, 1.0);
        assert_eq!(state.info_panel_origin, None);
    }

    #[test]
    fn mac_budget_review_pointer_dispatch_activates_on_matching_release() {
        let city = City::new_blank("/tmp/mac-budget-pointer-dispatch.sc2", "Budget").unwrap();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.city_info_dialog = Some(CityInfoDialog::Budget);
        state.mac_loop_scheduler.january_budget_review_pending = true;
        let mut input = InputState::new();
        let done = Vec2::new(464.0, 126.0);

        assert!(input.mac_budget_review_pointer_down(&mut state, done, (640, 480)));
        assert!(input.mac_budget_review_pointer_is_down());
        assert_eq!(
            input.mac_budget_review_pointer_up(&state, done, (640, 480)),
            Some(CityInfoDialogAction::Close)
        );
        assert!(!input.mac_budget_review_pointer_is_down());

        assert!(input.mac_budget_review_pointer_down(&mut state, done, (640, 480)));
        assert_eq!(
            input.mac_budget_review_pointer_up(&state, Vec2::new(329.0, 228.0), (640, 480)),
            None
        );
        assert!(!input.mac_budget_review_pointer_is_down());

        // The captured R10 Police edit used this Mac review control and
        // activated only when MouseUp returned over the same control.
        let police_decrease = Vec2::new(329.0, 228.0);
        assert!(input.mac_budget_review_pointer_down(&mut state, police_decrease, (640, 480)));
        assert_eq!(
            input.mac_budget_review_pointer_up(&state, police_decrease, (640, 480)),
            Some(CityInfoDialogAction::AdjustBudgetFunding {
                service_index: 0,
                delta: -5,
            })
        );
        assert!(!input.mac_budget_review_pointer_is_down());
    }

    #[test]
    fn mac_budget_review_click_inside_resets_hourglass_deadline() {
        let city = City::new_blank("/tmp/mac-budget-pointer-timeout.sc2", "Budget").unwrap();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        let start = 10_000;
        state.mac_loop_scheduler.start_captured_clock(start);
        state.mac_loop_scheduler.january_budget_review_pending = true;
        state.mac_loop_scheduler.january_budget_review_started_at = Some(start);
        state.open_city_info_dialog(CityInfoDialog::Budget);
        let mut input = InputState::new();
        let window_size = (640, 480);
        let page = city_info_dialog_rect_for_engine(
            CityInfoDialog::Budget,
            window_size,
            EngineFidelity::Mac68k,
        );
        let inside = Vec2::new(page.x + page.w * 0.9, page.y + page.h * 0.45);

        assert_eq!(
            state.advance_frame_simulation(Duration::from_secs(79)),
            GameClockAdvance::default()
        );
        assert_eq!(
            state.mac_loop_scheduler.january_budget_review_started_at,
            Some(start)
        );
        assert!(state.mac_loop_scheduler.january_budget_review_pending);

        assert!(!input.mac_budget_review_pointer_down(
            &mut state,
            Vec2::new(10.0, 10.0),
            window_size,
        ));
        assert_eq!(
            state.mac_loop_scheduler.january_budget_review_started_at,
            Some(start)
        );

        assert!(input.mac_budget_review_pointer_down(&mut state, inside, window_size));
        let restarted_at = start + 79 * 60;
        assert_eq!(
            state.mac_loop_scheduler.january_budget_review_started_at,
            Some(restarted_at)
        );
        assert_eq!(
            input.mac_budget_review_pointer_up(&state, inside, window_size),
            None
        );
        assert!(!input.mac_budget_review_pointer_is_down());

        state.advance_frame_simulation(Duration::from_secs(79));
        assert!(state.mac_loop_scheduler.january_budget_review_pending);
        assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
        state.advance_frame_simulation(Duration::from_secs(1));
        assert!(!state.mac_loop_scheduler.january_budget_review_pending);
        assert_ne!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    }

    #[test]
    fn mac_palette_rotation_music_matches_two_native_lcg_boundaries() {
        let city = City::new_blank("/tmp/palette-music.sc2", "Palette music").unwrap();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.music_enabled = true;
        state.mac_game_rng = crate::mac_random::MacGameLcgRng::new(0xc40e_094b);
        assert!(mac_palette_has_post_action_music(UiAction::RotateViewCcw));
        assert!(!mac_palette_has_post_action_music(UiAction::CenterMap));

        // The Quadra's two selector entries around this rotation had ticks
        // 6249/6251 and game-LCG states c40e094b/fbd0ff28/66ab0441.
        assert_eq!(state.mac_music_callback(6249, true), None);
        assert_eq!(state.mac_game_rng.state(), 0xfbd0_ff28);
        state.handle_ui_action_state(UiAction::RotateViewCcw);
        assert_eq!(state.mac_music_callback(6251, true), None);
        assert_eq!(state.mac_game_rng.state(), 0x66ab_0441);

        state.music_enabled = false;
        state.mac_music_callback(6252, true);
        assert_eq!(state.mac_game_rng.state(), 0x66ab_0441);
        state.engine_fidelity = EngineFidelity::Win95;
        state.music_enabled = true;
        state.mac_music_callback(6253, true);
        assert_eq!(state.mac_game_rng.state(), 0x66ab_0441);
    }

    #[test]
    fn palette_music_service_uses_the_mac_game_stream_in_city_and_editor() {
        let city = City::new_blank("/tmp/palette-music-service.sc2", "Palette music").unwrap();
        let mut state = test_state(city);
        let profile = crate::asset_profile::AssetProfile::load_from(std::env::temp_dir().join(
            format!("citygame-palette-music-{}.json", std::process::id()),
        ))
        .unwrap();
        let mut music = None;
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.music_enabled = true;
        state.mac_game_rng = crate::mac_random::MacGameLcgRng::new(0x4724_20c5);
        service_mac_palette_music(&mut state, &mut music, &profile);
        assert_eq!(state.mac_game_rng.state(), 0xa9bc_2a1a);

        state.engine_fidelity = EngineFidelity::Win95;
        service_mac_palette_music(&mut state, &mut music, &profile);
        assert_eq!(state.mac_game_rng.state(), 0xa9bc_2a1a);
        let city = City::new_blank("/tmp/palette-editor-music.sc2", "Music").expect("blank");
        let mut editor = test_state(city);
        editor.engine_fidelity = EngineFidelity::Mac68k;
        editor.mode = AppMode::EditNewMap;
        editor.music_enabled = true;
        editor.mac_game_rng = crate::mac_random::MacGameLcgRng::new(0x4724_20c5);
        service_mac_palette_music(&mut editor, &mut music, &profile);
        assert_eq!(editor.mac_game_rng.state(), 0xa9bc_2a1a);
    }
}

/// Feed one SDL input to the native-style fiscal notice. Returns true for
/// input events consumed by the notice, including events it does not dismiss.
pub(super) fn handle_mac_fiscal_notice_input(state: &mut ViewerState, event: &Event) -> bool {
    let Some(notice) = state.mac_loop_scheduler.fiscal_notice.as_mut() else {
        return false;
    };
    match event {
        Event::KeyDown { repeat: false, .. } => notice.dismissed = true,
        Event::MouseButtonDown { .. } | Event::FingerDown { .. } => {
            notice.dismissed = true;
            notice.button_down = true;
        }
        Event::MouseButtonUp { .. } | Event::FingerUp { .. } => notice.button_down = false,
        Event::KeyDown { .. }
        | Event::MouseMotion { .. }
        | Event::MouseWheel { .. }
        | Event::FingerMotion { .. }
        | Event::KeyUp { .. }
        | Event::TextInput { .. } => {}
        _ => return false,
    }
    true
}

/// Keep physical touch bookkeeping current while the modal notice consumes
/// gestures, including fingers that were already down when it opened.
fn handle_mac_fiscal_notice_touch(state: &mut ViewerState, input: &mut InputState, event: &Event) {
    match event {
        Event::FingerDown {
            finger_id, x, y, ..
        } => {
            input.touch.on_down(*finger_id, Vec2::new(*x, *y));
        }
        Event::FingerUp { finger_id, .. } => {
            input.touch.on_up(*finger_id);
            if input.touch.finger_count() == 0 {
                if input.map_touch.is_active() || input.painting || input.dragging {
                    state.cancel_active_drag();
                }
                input.map_touch.clear();
                input.painting = false;
                input.dragging = false;
                input.paint_tile = None;
            }
        }
        _ => return,
    }
    if let Some(notice) = state.mac_loop_scheduler.fiscal_notice.as_mut() {
        notice.button_down = input.fiscal_notice_mouse_down || input.touch.finger_count() != 0;
    }
}

pub(super) fn consume_mac_fiscal_notice_event(
    state: &mut ViewerState,
    input: &mut InputState,
    event: &Event,
) -> bool {
    if matches!(event,
        Event::MouseButtonDown { which, .. } | Event::MouseButtonUp { which, .. }
        | Event::MouseMotion { which, .. } | Event::MouseWheel { which, .. }
        if touch_input::is_touch_synthesized_mouse(*which))
    {
        return true;
    }
    if !handle_mac_fiscal_notice_input(state, event) {
        return false;
    }
    match event {
        Event::MouseButtonDown { .. } => input.fiscal_notice_mouse_down = true,
        Event::MouseButtonUp { .. } => input.fiscal_notice_mouse_down = false,
        _ => {}
    }
    handle_mac_fiscal_notice_touch(state, input, event);
    if matches!(
        event,
        Event::MouseButtonDown { .. } | Event::MouseButtonUp { .. }
    ) && let Some(notice) = state.mac_loop_scheduler.fiscal_notice.as_mut()
    {
        notice.button_down = input.fiscal_notice_mouse_down || input.touch.finger_count() != 0;
    }
    true
}

/// Native menu callbacks bypass SDL input, but the Mac notice consumes the
/// click or key before the main event dispatcher can execute the command.
pub(super) fn handle_mac_fiscal_notice_command(
    state: &mut ViewerState,
    command: &AppCommand,
) -> bool {
    let Some(notice) = state.mac_loop_scheduler.fiscal_notice.as_mut() else {
        return false;
    };
    let _ = command;
    notice.dismissed = true;
    true
}

/// Dialog/provider completions can arrive asynchronously, so retain them for
/// after the notice rather than interpreting them as a new click or key.
pub(super) fn mac_fiscal_notice_defers_command(command: &AppCommand) -> bool {
    #[cfg(target_os = "ios")]
    if matches!(command, AppCommand::DocumentSaved { .. }) {
        return true;
    }
    matches!(
        command,
        AppCommand::OpenPath(_)
            | AppCommand::OpenConfirmedCityPath(_)
            | AppCommand::OpenTitlePath(_)
            | AppCommand::OpenTitleDialogPath(_)
            | AppCommand::OpenScenarioPath(_)
            | AppCommand::OpenTitleScenarioPath(_)
            | AppCommand::SaveToPath(_)
            | AppCommand::LoadTileSet(_)
            | AppCommand::ImportOriginalAssets(_)
            | AppCommand::SetOriginalAssets(_)
            | AppCommand::SetReplacementAssets(_)
    )
}

pub(super) fn handle_event(
    event: Event,
    mut windows: RuntimeWindows<'_, '_, '_>,
    state: &mut ViewerState,
    input: &mut InputState,
    sound: &mut Option<crate::audio::SoundPlayer>,
    city_music: &mut Option<crate::title_music::CityMusic>,
    asset_profile: &crate::asset_profile::AssetProfile,
) -> Result<()> {
    if input.blocking_file_dialog()
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
    // CODE 2 export 66 consumes events while its temporary message is up.
    // Keep the notice until the frame dispatcher resumes the suspended worker,
    // so the rest of this event batch is discarded as the native flush does.
    if state.mac_loop_scheduler.fiscal_notice.is_some() {
        if consume_mac_fiscal_notice_event(state, input, &event) {
            return Ok(());
        }
        // Window and application lifecycle events must still be handled.
        match event {
            Event::Quit { .. } | Event::AppTerminating { .. } => input.running = false,
            event @ (Event::AppDidEnterBackground { .. }
            | Event::AppWillEnterForeground { .. }
            | Event::Window { .. }) => {
                handle_window_event(event, &mut windows, state, input, sound, city_music);
            }
            _ => {}
        }
        return Ok(());
    }
    match event {
        // Finger* events already drive touch; ignore SDL's mirrored mouse events.
        Event::MouseButtonDown { which, .. }
        | Event::MouseButtonUp { which, .. }
        | Event::MouseMotion { which, .. }
        | Event::MouseWheel { which, .. }
            if touch_input::is_touch_synthesized_mouse(which) => {}
        Event::Quit { .. }
            if cfg!(not(any(target_os = "android", target_os = "ios")))
                && state.engine_fidelity.uses_win95() =>
        {
            push_command(AppCommand::Quit)
        }
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
            handle_mouse_event(
                event,
                &mut windows,
                state,
                input,
                sound,
                city_music,
                asset_profile,
            )?;
        }
        event
        @ (Event::FingerDown { .. } | Event::FingerMotion { .. } | Event::FingerUp { .. }) => {
            handle_touch_event(
                event,
                &windows,
                state,
                input,
                sound,
                city_music,
                asset_profile,
            );
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
            if cfg!(not(any(target_os = "android", target_os = "ios")))
                && state.engine_fidelity.uses_win95()
            {
                push_command(AppCommand::Quit);
            } else {
                input.running = false;
            }
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
    if let Some(setup) = state.win95_new_city_setup.as_mut() {
        match event {
            Event::KeyDown {
                keycode: Some(Keycode::Return | Keycode::KpEnter),
                ..
            } => {
                state.commit_win95_new_city_setup();
                push_command(AppCommand::FinishWin95Charter);
                video.text_input().stop(window);
            }
            Event::KeyDown {
                keycode: Some(Keycode::Backspace),
                ..
            } => {
                if setup.selected {
                    setup.name.clear();
                    setup.selected = false;
                } else {
                    setup.name.pop();
                }
            }
            Event::TextInput { text, .. } => {
                if setup.selected {
                    setup.name.clear();
                    setup.selected = false;
                }
                for ch in text.chars() {
                    if setup.name.chars().count() >= 31 {
                        break;
                    }
                    if !ch.is_control() {
                        setup.name.push(ch);
                    }
                }
            }
            _ => {}
        }
        return;
    }
    if state.pe_newspaper_article.is_some() {
        match event {
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => state.pe_newspaper_article = None,
            Event::KeyDown {
                keycode: Some(Keycode::Up),
                ..
            } => {
                state.scroll_pe_newspaper_article(-1);
            }
            Event::KeyDown {
                keycode: Some(Keycode::Down),
                ..
            } => {
                state.scroll_pe_newspaper_article(1);
            }
            Event::KeyUp {
                keycode: Some(Keycode::LShift | Keycode::RShift),
                ..
            } => input.shift_down = false,
            _ => {}
        }
        return;
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
            } else if state.mac_newspaper.detail_open {
                state.close_mac_paper_detail();
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
        Event::KeyDown {
            keycode: Some(Keycode::Left),
            ..
        } if state.sign_dialog.is_some() && !state.engine_fidelity.uses_win95() => {
            state.move_sign_dialog_caret(false)
        }
        Event::KeyDown {
            keycode: Some(Keycode::Right),
            ..
        } if state.sign_dialog.is_some() && !state.engine_fidelity.uses_win95() => {
            state.move_sign_dialog_caret(true)
        }
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
        } if MenubarUi::is_enabled()
            && state.engine_fidelity.uses_win95()
            && win95_file_menu_shortcut(key, keymod).is_some() =>
        {
            push_command(
                win95_file_menu_shortcut(key, keymod).expect("matched Win95 File shortcut"),
            );
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
            keycode: Some(Keycode::Up),
            ..
        } if state.mac_newspaper.detail_open => {
            state.scroll_mac_paper_detail(-1);
        }
        Event::KeyDown {
            keycode: Some(Keycode::Down),
            ..
        } if state.mac_newspaper.detail_open => {
            state.scroll_mac_paper_detail(1);
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

fn service_mac_palette_music(
    state: &mut ViewerState,
    city_music: &mut Option<crate::title_music::CityMusic>,
    asset_profile: &crate::asset_profile::AssetProfile,
) {
    if matches!(state.mode, AppMode::CityView | AppMode::EditNewMap)
        && !state.engine_fidelity.uses_win95()
    {
        super::commands::update_mac_city_music(state, city_music, asset_profile);
    }
}

fn mac_palette_has_post_action_music(action: UiAction) -> bool {
    matches!(
        action,
        UiAction::RotateViewCcw | UiAction::RotateViewCw | UiAction::ZoomOut | UiAction::ZoomIn
    )
}

fn apply_palette_leaf_action(
    state: &mut ViewerState,
    action: UiAction,
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    size: (u32, u32),
) {
    if state.mode == AppMode::CityView && !state.engine_fidelity.uses_win95() {
        let scale = pixel_scale(window, size);
        match action {
            UiAction::ZoomOut => {
                state.zoom_out_at(viewport_center(size), size, scale);
                return;
            }
            UiAction::ZoomIn => {
                state.zoom_in_at(viewport_center(size), size, scale);
                return;
            }
            _ => {}
        }
    }
    state.handle_ui_action(action, video, window.id());
}

fn handle_mouse_event(
    event: Event,
    windows: &mut RuntimeWindows<'_, '_, '_>,
    state: &mut ViewerState,
    input: &mut InputState,
    sound: &Option<crate::audio::SoundPlayer>,
    city_music: &mut Option<crate::title_music::CityMusic>,
    asset_profile: &crate::asset_profile::AssetProfile,
) -> Result<()> {
    let video = windows.video;
    let window = windows.window;
    let renderer = &mut *windows.renderer;
    let palette_window = windows.palette_window;
    let mut palette_renderer = windows.palette_renderer.as_deref_mut();
    let palette_id = windows.palette_id;
    let palette_logical_size = &mut *windows.palette_logical_size;
    let single_window = windows.single_window;
    if state.win95_new_city_setup.is_some() {
        if let Event::MouseButtonDown {
            window_id,
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } = event
        {
            if window_id == window.id() {
                let screen = Vec2::new(x, y) * input_pixel_scale(window, renderer.size());
                if state.win95_setup_pointer(screen, renderer.size()) {
                    push_command(AppCommand::FinishWin95Charter);
                }
            }
        }
        return Ok(());
    }
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
                    service_mac_palette_music(state, city_music, asset_profile);
                    state.close_tool_menu();
                    state.edit_map_slider_drag = Some(slider);
                    state.set_edit_map_slider(slider, value);
                    if state.engine_fidelity.uses_win95() {
                        state.play_click_sound(sound.as_ref());
                    }
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
                        service_mac_palette_music(state, city_music, asset_profile);
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
                                apply_palette_leaf_action(
                                    state,
                                    action,
                                    video,
                                    window,
                                    renderer.size(),
                                );
                                // The floating Tools palette can hold key focus on
                                // macOS. Return focus to the map as soon as a leaf
                                // tool/action is chosen, so the next map press is a
                                // gameplay input instead of the window-activation click.
                                mac_menu::focus_main_window(window);
                                palette_window::sync_palette_stacking(window, palette_window);
                            }
                        }
                        if state.mode == AppMode::CityView
                            && mac_palette_has_post_action_music(action)
                        {
                            service_mac_palette_music(state, city_music, asset_profile);
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
            if input.mac_budget_review_pointer_down(state, screen, renderer.size()) {
                input.painting = false;
                input.dragging = false;
                return Ok(());
            }
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
                    service_mac_palette_music(state, city_music, asset_profile);
                    state.close_tool_menu();
                    state.edit_map_slider_drag = Some(slider);
                    state.set_edit_map_slider(slider, value);
                    if state.engine_fidelity.uses_win95() {
                        state.play_click_sound(sound.as_ref());
                    }
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
                        service_mac_palette_music(state, city_music, asset_profile);
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
                                apply_palette_leaf_action(
                                    state,
                                    action,
                                    video,
                                    window,
                                    renderer.size(),
                                );
                                input.begin_info_preview(
                                    state,
                                    action,
                                    screen,
                                    tools_origin,
                                    Vec2::new(
                                        palette_logical_size.0 as f32 * ps,
                                        palette_logical_size.1 as f32 * ps,
                                    ),
                                );
                            }
                        }
                        if state.mode == AppMode::CityView
                            && mac_palette_has_post_action_music(action)
                        {
                            service_mac_palette_music(state, city_music, asset_profile);
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
                point_in_float_rect(
                    state.info_panel_input_point(screen, renderer.size()),
                    city_info_dialog_rect_for_engine(
                        dialog,
                        renderer.size(),
                        state.engine_fidelity,
                    ),
                )
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
        } if window_id == window.id() && input.mac_budget_review_pointer_down.is_some() => {
            let screen = Vec2::new(x, y) * input_pixel_scale(window, renderer.size());
            input.mouse_pos = screen;
            if let Some(action) = input.mac_budget_review_pointer_up(state, screen, renderer.size())
            {
                apply_city_info_dialog_action(
                    state,
                    action,
                    renderer.size(),
                    pixel_scale(window, renderer.size()),
                    video,
                    window,
                    sound.as_ref(),
                );
            }
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
                if let Some(active_slider) = state.edit_map_slider_drag {
                    let value = palette_window::palette_mouse_slider_drag_value(
                        palette_window,
                        palette_renderer,
                        *palette_logical_size,
                        active_slider,
                        x,
                        y,
                    );
                    state.set_edit_map_slider(active_slider, value);
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
            if single_window && let Some(slider) = state.edit_map_slider_drag {
                let ps = pixel_scale(window, renderer.size());
                let origin = input
                    .docked_palette
                    .tools_origin(MenubarUi::occupied_top(ps), ps);
                let value = ui::edit_map_slider_drag_value(
                    input.mouse_pos - origin,
                    *palette_logical_size,
                    ps,
                    slider,
                );
                state.set_edit_map_slider(slider, value);
                state.update_window_title(video, window.id());
                return Ok(());
            }
            if input.move_info_preview(state, input.mouse_pos) {
                return Ok(());
            }
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
            let signed = match direction {
                sdl3::mouse::MouseWheelDirection::Flipped => y,
                sdl3::mouse::MouseWheelDirection::Normal
                | sdl3::mouse::MouseWheelDirection::Unknown(_) => -y,
            };
            if state.scroll_pe_newspaper_article((signed * 3.0).round() as i32) {
                return Ok(());
            }
            if state.scroll_mac_paper_detail((signed * 3.0).round() as i32) {
                return Ok(());
            }
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
    city_music: &mut Option<crate::title_music::CityMusic>,
    asset_profile: &crate::asset_profile::AssetProfile,
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
                state.edit_map_slider_drag = None;
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
                                    if let Some((slider, value)) = ui::edit_map_slider_hit_test(
                                        screen - tools_origin,
                                        palette_logical_size,
                                        ps,
                                        state.mode,
                                        state.open_tool_menu,
                                    ) {
                                        service_mac_palette_music(state, city_music, asset_profile);
                                        state.close_tool_menu();
                                        state.edit_map_slider_drag = Some(slider);
                                        state.set_edit_map_slider(slider, value);
                                        if state.engine_fidelity.uses_win95() {
                                            state.play_click_sound(sound.as_ref());
                                        }
                                        state.update_window_title(video, window.id());
                                        ui_consumed = true;
                                    } else if let Some(action) = ui::palette_hit_test_at(
                                        screen,
                                        tools_origin,
                                        palette_logical_size,
                                        ps,
                                        state.mode,
                                        state.open_tool_menu,
                                        platform_ui::touch_hit_pad(ps),
                                    ) {
                                        if state.ui_action_enabled(action) {
                                            service_mac_palette_music(
                                                state,
                                                city_music,
                                                asset_profile,
                                            );
                                            state.press_palette_button(action);
                                            state.play_click_sound(sound.as_ref());
                                            match action {
                                                UiAction::OpenMenu(menu) => {
                                                    state.open_tool_menu(menu);
                                                    state.update_window_title(video, window.id());
                                                }
                                                _ => {
                                                    state.close_tool_menu();
                                                    apply_palette_leaf_action(
                                                        state,
                                                        action,
                                                        video,
                                                        window,
                                                        renderer.size(),
                                                    );
                                                    input.begin_info_preview(
                                                        state,
                                                        action,
                                                        screen,
                                                        tools_origin,
                                                        Vec2::new(
                                                            palette_logical_size.0 as f32 * ps,
                                                            palette_logical_size.1 as f32 * ps,
                                                        ),
                                                    );
                                                }
                                            }
                                            if state.mode == AppMode::CityView
                                                && mac_palette_has_post_action_music(action)
                                            {
                                                service_mac_palette_music(
                                                    state,
                                                    city_music,
                                                    asset_profile,
                                                );
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
            if single_window
                && state.edit_map_slider_drag.is_some()
                && input.touch.finger_count() == 1
            {
                let slider = state.edit_map_slider_drag.expect("active slider");
                let origin = input
                    .docked_palette
                    .tools_origin(MenubarUi::occupied_top(ps), ps);
                let value = ui::edit_map_slider_drag_value(
                    screen - origin,
                    palette_logical_size,
                    ps,
                    slider,
                );
                state.set_edit_map_slider(slider, value);
                state.update_window_title(video, window.id());
                let _ = input.touch.on_motion(finger_id, screen);
            } else if input.move_info_preview(state, screen) {
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
                state.edit_map_slider_drag = None;
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
    if state.mac_loop_scheduler.fiscal_notice.is_some() {
        return;
    }
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

#[cfg(test)]
mod mac_fiscal_notice_input_tests {
    use super::*;
    use crate::app::tests::test_state;
    use crate::city::City;

    #[test]
    fn mac_fiscal_notice_tracks_preheld_touch_until_last_release() {
        let city = City::new_blank("/tmp/mac-fiscal-touch.sc2", "Fiscal").unwrap();
        let mut state = test_state(city);
        let mut input = InputState::new();
        input.touch.on_down(7, Vec2::new(10.0, 10.0));
        input.map_touch.begin_map_gesture(Vec2::ZERO, true, 1.0);
        input.painting = true;
        input.paint_tile = Some((2, 3));
        state.open_mac_fiscal_crisis_notice();
        state
            .mac_loop_scheduler
            .fiscal_notice
            .as_mut()
            .unwrap()
            .button_down = true;
        let down = Event::FingerDown {
            timestamp: 0,
            touch_id: 1,
            finger_id: 8,
            x: 0.2,
            y: 0.3,
            dx: 0.0,
            dy: 0.0,
            pressure: 1.0,
            window_id: 0,
        };
        let up = |finger_id| Event::FingerUp {
            timestamp: 0,
            touch_id: 1,
            finger_id,
            x: 0.2,
            y: 0.3,
            dx: 0.0,
            dy: 0.0,
            pressure: 0.0,
            window_id: 0,
        };
        assert!(consume_mac_fiscal_notice_event(
            &mut state, &mut input, &down
        ));
        assert!(consume_mac_fiscal_notice_event(
            &mut state,
            &mut input,
            &up(7)
        ));
        assert_eq!(input.touch.finger_count(), 1);
        assert!(state.mac_loop_scheduler.fiscal_notice.unwrap().button_down);
        let synthetic_up = Event::MouseButtonUp {
            timestamp: 0,
            window_id: 0,
            which: u32::MAX,
            mouse_btn: sdl3::mouse::MouseButton::Left,
            clicks: 1,
            x: 0.2,
            y: 0.3,
        };
        assert!(consume_mac_fiscal_notice_event(
            &mut state,
            &mut input,
            &synthetic_up
        ));
        assert!(state.mac_loop_scheduler.fiscal_notice.unwrap().button_down);
        let waits = state.begin_mac_frame_actor_phase(std::time::Duration::ZERO);
        assert_eq!(
            state
                .finish_mac_frame_calendar_phase(std::time::Duration::ZERO, waits)
                .days,
            0
        );
        let mouse_down = Event::MouseButtonDown {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn: sdl3::mouse::MouseButton::Left,
            clicks: 1,
            x: 0.0,
            y: 0.0,
        };
        assert!(consume_mac_fiscal_notice_event(
            &mut state,
            &mut input,
            &mouse_down
        ));
        assert!(consume_mac_fiscal_notice_event(
            &mut state,
            &mut input,
            &up(8)
        ));
        assert_eq!(input.touch.finger_count(), 0);
        assert!(!input.map_touch.is_active());
        assert!(!input.painting);
        assert_eq!(input.paint_tile, None);
        assert!(state.mac_loop_scheduler.fiscal_notice.unwrap().button_down);
        let mouse_up = Event::MouseButtonUp {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn: sdl3::mouse::MouseButton::Left,
            clicks: 1,
            x: 0.0,
            y: 0.0,
        };
        assert!(consume_mac_fiscal_notice_event(
            &mut state, &mut input, &mouse_up
        ));
        let waits = state.begin_mac_frame_actor_phase(std::time::Duration::ZERO);
        assert_eq!(
            state
                .finish_mac_frame_calendar_phase(std::time::Duration::ZERO, waits)
                .days,
            1
        );
    }

    #[test]
    fn mac_fiscal_notice_consumes_real_input_and_waits_for_mouse_release() {
        let city = City::new_blank("/tmp/mac-fiscal-input.sc2", "Fiscal").unwrap();
        let mut state = test_state(city);
        state.open_mac_fiscal_crisis_notice();
        let key = |repeat| Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(Keycode::Return),
            scancode: None,
            keymod: sdl3::keyboard::Mod::NOMOD,
            repeat,
            which: 0,
            raw: 0,
        };
        assert!(handle_mac_fiscal_notice_input(&mut state, &key(true)));
        assert!(!state.mac_loop_scheduler.fiscal_notice.unwrap().dismissed);
        assert!(handle_mac_fiscal_notice_input(&mut state, &key(false)));
        assert!(state.mac_loop_scheduler.fiscal_notice.unwrap().dismissed);
        state.mac_loop_scheduler.fiscal_notice = None;
        state.city_info_dialog = None;
        state.open_mac_fiscal_crisis_notice();
        let down = Event::MouseButtonDown {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn: sdl3::mouse::MouseButton::Left,
            clicks: 1,
            x: 0.0,
            y: 0.0,
        };
        let up = Event::MouseButtonUp {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn: sdl3::mouse::MouseButton::Left,
            clicks: 1,
            x: 0.0,
            y: 0.0,
        };
        assert!(handle_mac_fiscal_notice_input(&mut state, &down));
        assert!(state.mac_loop_scheduler.fiscal_notice.unwrap().button_down);
        assert!(handle_mac_fiscal_notice_input(&mut state, &up));
        let notice = state.mac_loop_scheduler.fiscal_notice.unwrap();
        assert!(notice.dismissed && !notice.button_down);

        // A key can dismiss the notice while a prior mouse press is still
        // held, but the native _Button wait does not return until release.
        state.mac_loop_scheduler.fiscal_notice = None;
        state.city_info_dialog = None;
        state.open_mac_fiscal_crisis_notice();
        state
            .mac_loop_scheduler
            .fiscal_notice
            .as_mut()
            .unwrap()
            .button_down = true;
        assert!(handle_mac_fiscal_notice_input(&mut state, &key(false)));
        let waits = state.begin_mac_frame_actor_phase(std::time::Duration::ZERO);
        assert_eq!(
            state
                .finish_mac_frame_calendar_phase(std::time::Duration::ZERO, waits)
                .days,
            0
        );
        assert!(handle_mac_fiscal_notice_input(&mut state, &up));
        let waits = state.begin_mac_frame_actor_phase(std::time::Duration::ZERO);
        assert_eq!(
            state
                .finish_mac_frame_calendar_phase(std::time::Duration::ZERO, waits)
                .days,
            1
        );
    }

    #[test]
    fn mac_fiscal_notice_consumes_native_menu_command_before_execution() {
        let city = City::new_blank("/tmp/mac-fiscal-menu.sc2", "Fiscal").unwrap();
        let mut state = test_state(city);
        let initial_speed = state.traffic.speed();
        assert_ne!(initial_speed, GameSpeed::Cheetah);
        assert!(mac_fiscal_notice_defers_command(&AppCommand::SaveToPath(
            "/tmp/deferred.sc2".into()
        )));
        assert!(!mac_fiscal_notice_defers_command(&AppCommand::SetSpeed(
            GameSpeed::Cheetah
        )));
        state.open_mac_fiscal_crisis_notice();
        state
            .mac_loop_scheduler
            .fiscal_notice
            .as_mut()
            .unwrap()
            .button_down = true;
        let command = AppCommand::SetSpeed(GameSpeed::Cheetah);
        if !handle_mac_fiscal_notice_command(&mut state, &command) {
            state.apply_state_app_command(&command);
        }
        let notice = state.mac_loop_scheduler.fiscal_notice.unwrap();
        assert!(notice.dismissed && notice.button_down);
        assert_eq!(state.traffic.speed(), initial_speed);
        let waits = state.begin_mac_frame_actor_phase(std::time::Duration::ZERO);
        assert_eq!(
            state
                .finish_mac_frame_calendar_phase(std::time::Duration::ZERO, waits)
                .days,
            0
        );
        state
            .mac_loop_scheduler
            .fiscal_notice
            .as_mut()
            .unwrap()
            .button_down = false;
        let waits = state.begin_mac_frame_actor_phase(std::time::Duration::ZERO);
        assert_eq!(
            state
                .finish_mac_frame_calendar_phase(std::time::Duration::ZERO, waits)
                .days,
            1
        );
        assert!(!handle_mac_fiscal_notice_command(&mut state, &command));
        state.apply_state_app_command(&command);
        assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
    }
}
