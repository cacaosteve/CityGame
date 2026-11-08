//! Main-map and Tools-palette frame construction and presentation.

use super::*;

pub(super) fn present_frame(
    mut windows: RuntimeWindows<'_, '_, '_>,
    state: &mut ViewerState,
    docked_palette: &DockedPalette,
    occluded_frames: &mut u32,
) {
    let video = windows.video;
    let window = windows.window;
    let renderer = &mut *windows.renderer;
    let palette_window = windows.palette_window;
    let mut palette_renderer = windows.palette_renderer.as_deref_mut();
    let single_window = windows.single_window;
    let palette_logical_size = *windows.palette_logical_size;
    let dirty = state.city.take_dirty_tiles();
    let traffic_redraw =
        state.consume_traffic_redraw_request(&dirty) || state.consume_pending_traffic_redraw();
    let mut sprites = state.build_sprites(
        renderer,
        renderer.size(),
        pixel_scale(window, renderer.size()),
        dirty,
        traffic_redraw,
    );
    state.push_tool_preview(
        renderer,
        renderer.size(),
        pixel_scale(window, renderer.size()),
        &mut sprites,
    );
    let mut ui_solids = Vec::new();
    let mut ui_images = Vec::new();
    let main_ps = pixel_scale(window, renderer.size());
    state.menubar.push_draw(
        renderer.size(),
        main_ps,
        state.menu_checks,
        state.dark_appearance,
        &mut ui_solids,
    );
    if single_window {
        let menubar_top = MenubarUi::occupied_top(main_ps);
        docked_palette.push_title_bar(
            menubar_top,
            main_ps,
            palette_logical_size,
            state.dark_appearance,
            &mut ui_solids,
        );
        let tools_origin = docked_palette.tools_origin(menubar_top, main_ps);
        let palette_pressed = state.pressed_palette_action();
        let disabled_palette_actions = state.disabled_palette_actions();
        let checked_palette_actions = state.checked_palette_actions();
        let edit_map_sliders = state.edit_map_slider_values();
        let mut docked = ui::build_palette_ui(
            renderer,
            palette_logical_size,
            main_ps,
            state.mode,
            state.tool,
            palette_pressed,
            state.open_tool_menu,
            &disabled_palette_actions,
            &checked_palette_actions,
            edit_map_sliders,
        );
        docked.offset_by(tools_origin);
        ui_solids.extend(docked.solids);
        ui_images.extend(docked.images);
    }
    state.push_drag_cost_overlay(
        renderer.size(),
        pixel_scale(window, renderer.size()),
        &mut ui_solids,
    );
    state.push_sign_labels(
        renderer.size(),
        pixel_scale(window, renderer.size()),
        &mut ui_solids,
    );
    state.push_status_panel(renderer.size(), main_ps, &mut ui_solids);
    let dialog_solids_start = ui_solids.len();
    state.push_sign_dialog(renderer.size(), &mut ui_solids);
    state.push_military_base_dialog(renderer.size(), &mut ui_solids);
    state.push_power_plant_dialog(renderer, renderer.size(), &mut ui_images, &mut ui_solids);
    state.push_arcology_dialog(renderer, renderer.size(), &mut ui_images, &mut ui_solids);
    state.push_city_info_dialog(
        Some(renderer),
        renderer.size(),
        pixel_scale(window, renderer.size()),
        &mut ui_solids,
        &mut ui_images,
    );
    if state.dark_appearance {
        ui::apply_dark_window_colors(&mut ui_solids[dialog_solids_start..]);
    }
    sync_renderer_to_window(renderer, window);
    let presented = match renderer.render_with_clear_color(
        &sprites,
        &ui_solids,
        &ui_images,
        state.clear_color(),
    ) {
        Ok(true) => {
            *occluded_frames = 0;
            true
        }
        Ok(false) => {
            *occluded_frames = occluded_frames.saturating_add(1);
            if *occluded_frames == 1 || occluded_frames.is_multiple_of(60) {
                log::debug!(
                    "render skipped (surface occluded); anim frame {}",
                    state.animation_frame()
                );
                raise_window(video, window.id());
            }
            false
        }
        Err(error) => {
            log::error!("render error: {error:#}");
            false
        }
    };
    if let (Some(palette_window), Some(palette_renderer)) = (palette_window, &mut palette_renderer)
    {
        let palette_pressed = state.pressed_palette_action();
        let disabled_palette_actions = state.disabled_palette_actions();
        let checked_palette_actions = state.checked_palette_actions();
        let edit_map_sliders = state.edit_map_slider_values();
        if let Err(error) = palette_window::render_palette(
            palette_window,
            palette_renderer,
            palette_logical_size,
            state.mode,
            state.tool,
            palette_pressed,
            state.open_tool_menu,
            &disabled_palette_actions,
            &checked_palette_actions,
            edit_map_sliders,
        ) {
            log::error!("palette render failed: {error:#}");
        }
        palette_window.sync();
    }
    let _ = presented;
}
