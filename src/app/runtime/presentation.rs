//! Main-map and Tools-palette frame construction and presentation.

use super::*;
use crate::renderer::UiSolid;

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
        if state.mode == crate::city::AppMode::CityView {
            let unit = palette_logical_size.0 as f32 * main_ps / 71.0;
            let x = tools_origin.x + 48.0 * unit;
            let y = tools_origin.y + 202.0 * unit;
            ui_solids.push(UiSolid {
                x,
                y,
                w: 21.0 * unit,
                h: 59.0 * unit,
                color: if state.dark_appearance {
                    [0.08, 0.14, 0.17, 1.0]
                } else {
                    [0.70, 0.70, 0.70, 1.0]
                },
                depth: ui::UI_DEPTH + 10.0,
            });
            ui::push_pixel_text(
                &mut ui_solids,
                "RCI",
                x + 4.0 * unit,
                y + 25.0 * unit,
                unit,
                if state.dark_appearance {
                    [0.9, 0.95, 0.95, 1.0]
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                },
                ui::UI_DEPTH + 11.0,
            );
            for (i, (kind, color)) in [
                (super::super::RciKind::Residential, [0.1, 0.8, 0.2, 1.0]),
                (super::super::RciKind::Commercial, [0.1, 0.35, 0.95, 1.0]),
                (super::super::RciKind::Industrial, [0.95, 0.8, 0.1, 1.0]),
            ]
            .into_iter()
            .enumerate()
            {
                let value =
                    super::super::rci::rci_demand_for_kind(&state.city, kind).clamp(-2000, 2000);
                let height = value.unsigned_abs() as f32 / 2000.0 * 20.0 * unit;
                if height > 0.0 {
                    let top = if value > 0 {
                        y + 22.0 * unit - height
                    } else {
                        y + 36.0 * unit
                    };
                    ui_solids.push(UiSolid {
                        x: x + (3.0 + i as f32 * 6.0) * unit,
                        y: top,
                        w: 4.0 * unit,
                        h: height,
                        color,
                        depth: ui::UI_DEPTH + 11.0,
                    });
                }
            }
        }
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
    state.push_status_panel(
        renderer.size(),
        main_ps,
        &mut ui_solids,
        Some(renderer),
        &mut ui_images,
    );
    let dialog_solids_start = ui_solids.len();
    state.push_sign_dialog(renderer.size(), &mut ui_solids);
    state.push_military_base_dialog(renderer.size(), &mut ui_solids);
    state.push_power_plant_dialog(renderer, renderer.size(), &mut ui_images, &mut ui_solids);
    state.push_arcology_dialog(renderer, renderer.size(), &mut ui_images, &mut ui_solids);
    let info_solid_start = ui_solids.len();
    let info_image_start = ui_images.len();
    state.push_city_info_dialog(
        Some(renderer),
        renderer.size(),
        pixel_scale(window, renderer.size()),
        &mut ui_solids,
        &mut ui_images,
    );
    if let Some((base, origin, scale)) = state.info_panel_transform(renderer.size()) {
        for solid in &mut ui_solids[info_solid_start..] {
            solid.x = origin.x + (solid.x - base.x) * scale;
            solid.y = origin.y + (solid.y - base.y) * scale;
            solid.w *= scale;
            solid.h *= scale;
        }
        for image in &mut ui_images[info_image_start..] {
            image.x = origin.x + (image.x - base.x) * scale;
            image.y = origin.y + (image.y - base.y) * scale;
            image.w *= scale;
            image.h *= scale;
        }
        ui_solids.push(UiSolid {
            x: origin.x + base.w * scale - 14.0 * scale,
            y: origin.y + base.h * scale - 6.0 * scale,
            w: 9.0 * scale,
            h: 2.0 * scale,
            color: [0.2, 0.35, 0.38, 1.0],
            depth: ui::UI_DEPTH + 320.0,
        });
    }
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
