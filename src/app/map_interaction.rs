//! Map painting, drag application, and gesture transitions through existing tools.
//!
//! Mouse and touch share tool workers while their callers retain input flags,
//! modal/palette routing, and panning behavior. Tool/mode/shift gates, drag order,
//! placement sounds, neighbor confirmations, and held-tool cleanup are preserved.

use super::nature_tools::play_nature_placement_sound;
use super::network::{
    network_kind_for_tool, network_placement_sound, show_highway_neighbor_link_confirmation,
    show_rail_neighbor_link_confirmation, show_road_neighbor_link_confirmation,
};
use super::terrain_editing::pe_raise_tool0_clear_temp_xtxt_0xc9;
use super::tool_controller::one_click_placement_sound;
use super::underground::{underground_kind_for_tool, underground_placement_sound};
use super::{
    PE_RAISE_SUBMODE_ADJUST_TOWARD, PlacementSound, ViewerState, play_placement_sound,
    zone_id_for_tool, zone_placement_sound,
};
use crate::city::AppMode;
use crate::engine_fidelity::EngineFidelity;
use crate::touch_input::MapTouchAction;
use crate::ui::EditTool;
use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MapInteractionFeedback {
    UpdateTitle,
    StartSignInput,
    PlaySound(PlacementSound),
}

fn apply_map_feedback(
    state: &ViewerState,
    effect: MapInteractionFeedback,
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    sound: Option<&crate::audio::SoundPlayer>,
) {
    match effect {
        MapInteractionFeedback::UpdateTitle => state.update_window_title(video, window.id()),
        MapInteractionFeedback::StartSignInput => video.text_input().start(window),
        MapInteractionFeedback::PlaySound(effect) => {
            play_placement_sound(state.sound_effects_player(sound), effect);
        }
    }
}

pub(super) fn apply_map_touch_action(
    action: MapTouchAction,
    state: &mut ViewerState,
    painting: &mut bool,
    paint_tile: &mut Option<(usize, usize)>,
    dragging: &mut bool,
    shift_down: bool,
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    sound: Option<&crate::audio::SoundPlayer>,
    window_size: (u32, u32),
    pixel_scale: f32,
) {
    match action {
        MapTouchAction::ToolTap { screen } | MapTouchAction::BeginPaint { screen } => {
            let mut next_painting = false;
            let mut next_tile = None;
            handle_map_left_pointer_down(
                state,
                screen,
                window_size,
                pixel_scale,
                &mut next_painting,
                &mut next_tile,
                shift_down,
                video,
                window,
                sound,
            );
            *painting = next_painting;
            *paint_tile = next_tile;
            *dragging = false;
        }
        MapTouchAction::PaintMove { screen } => {
            update_map_paint_pointer_motion(
                state,
                screen,
                window_size,
                pixel_scale,
                *painting,
                paint_tile,
                shift_down,
                video,
                window,
            );
        }
        MapTouchAction::EndPaint => {
            if *painting {
                finish_map_left_pointer_up(state, paint_tile, shift_down, video, window, sound);
            }
            *painting = false;
            *dragging = false;
        }
        MapTouchAction::Pan { delta } => {
            if !*painting {
                state.camera += delta;
            }
        }
        MapTouchAction::Pinch { factor, anchor } => {
            if *painting {
                finish_map_left_pointer_up(state, paint_tile, shift_down, video, window, sound);
                *painting = false;
            }
            state.zoom_by_at(factor, anchor, window_size, pixel_scale);
            *dragging = false;
        }
    }
}

/// Shared left-pointer map interaction for mouse and touch (place / bulldoze / dispatch / …).
pub(super) fn handle_map_left_pointer_down(
    state: &mut ViewerState,
    screen: Vec2,
    window_size: (u32, u32),
    pixel_scale: f32,
    painting: &mut bool,
    paint_tile: &mut Option<(usize, usize)>,
    shift_down: bool,
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    sound: Option<&crate::audio::SoundPlayer>,
) {
    begin_map_pointer_interaction(
        state,
        screen,
        window_size,
        pixel_scale,
        painting,
        paint_tile,
        shift_down,
        |state, effect| apply_map_feedback(state, effect, video, window, sound),
    );
}

/// The caller chooses whether painting starts fresh (touch) or is retained (mouse).
/// Feedback runs immediately at the original point in the tool operation.
pub(super) fn begin_map_pointer_interaction(
    state: &mut ViewerState,
    screen: Vec2,
    window_size: (u32, u32),
    pixel_scale: f32,
    painting: &mut bool,
    paint_tile: &mut Option<(usize, usize)>,
    shift_down: bool,
    mut feedback: impl FnMut(&ViewerState, MapInteractionFeedback),
) {
    state.close_tool_menu();
    state.update_hover_tile(screen, window_size, pixel_scale);
    *paint_tile = None;
    if state.mode == AppMode::EditNewMap {
        if state.tool == EditTool::StretchTerrain {
            *painting = state.begin_edit_map_stretch_drag(screen, window_size, pixel_scale);
        } else if state.tool == EditTool::LowerTerrain
            && state.engine_fidelity != EngineFidelity::Mac68k
        {
            *painting = state.begin_edit_map_lower_drag(screen, window_size, pixel_scale);
        } else if state.tool == EditTool::LevelTerrain {
            *painting = state.begin_edit_map_level_drag(screen, window_size, pixel_scale);
        } else {
            if state.tool == EditTool::RaiseTerrain
                && state.engine_fidelity != EngineFidelity::Mac68k
                && let Some((tile_x, tile_y)) = state.hover_tile
            {
                state.pe_raise_press_y = tile_y as u8;
                if state.pe_raise_submode == PE_RAISE_SUBMODE_ADJUST_TOWARD {
                    state.pe_raise_adjust_land = state.city.altitude_at(tile_x, tile_y);
                }
            }
            *painting = true;
            state.apply_tool_at_with_shift(screen, window_size, pixel_scale, shift_down);
            *paint_tile = state.hover_tile;
        }
    } else if state.mode == AppMode::CityView && state.tool == EditTool::Query {
        state.inspect_tile_at(screen, window_size, pixel_scale);
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.mode == AppMode::CityView && zone_id_for_tool(state.tool).is_some() {
        if let Some((tile_x, tile_y)) = state.hover_tile
            && let Some(zone_id) = zone_id_for_tool(state.tool)
        {
            *painting = true;
            state.begin_zone_drag(tile_x, tile_y, zone_id);
        }
    } else if state.mode == AppMode::CityView && network_kind_for_tool(state.tool).is_some() {
        if !shift_down
            && let Some((tile_x, tile_y)) = state.hover_tile
            && let Some(kind) = network_kind_for_tool(state.tool)
        {
            *painting = true;
            state.begin_network_drag(tile_x, tile_y, kind);
        }
    } else if state.mode == AppMode::CityView && underground_kind_for_tool(state.tool).is_some() {
        if !shift_down
            && let Some((tile_x, tile_y)) = state.hover_tile
            && let Some(kind) = underground_kind_for_tool(state.tool)
        {
            *painting = true;
            state.begin_underground_drag(tile_x, tile_y, kind);
        }
    } else if state.mode == AppMode::CityView && state.tool == EditTool::Highway {
        if !shift_down && let Some((tile_x, tile_y)) = state.hover_tile {
            *painting = true;
            state.begin_highway_drag(tile_x, tile_y);
        }
    } else if state.mode == AppMode::CityView
        && matches!(
            state.tool,
            EditTool::RaiseTerrain
                | EditTool::LowerTerrain
                | EditTool::LevelTerrain
                | EditTool::StretchTerrain
                | EditTool::SpreadTerrain
        )
    {
        if !shift_down && let Some((tile_x, tile_y)) = state.hover_tile {
            *painting = true;
            state.begin_terrain_drag(tile_x, tile_y);
        }
    } else if state.mode == AppMode::CityView
        && matches!(state.tool, EditTool::Trees | EditTool::Water)
    {
        if let Some((tile_x, tile_y)) = state.hover_tile {
            *painting = true;
            state.begin_nature_drag_with_shift(tile_x, tile_y, state.tool, shift_down);
        }
    } else if state.mode == AppMode::CityView && state.tool == EditTool::Forest {
        *painting = true;
        let placed = state.apply_tool_at_with_shift(screen, window_size, pixel_scale, shift_down);
        *paint_tile = state.hover_tile;
        if placed && let Some(effect) = one_click_placement_sound(state.tool) {
            feedback(state, MapInteractionFeedback::PlaySound(effect));
        }
    } else {
        *painting = true;
        let placed = state.apply_tool_at(screen, window_size, pixel_scale);
        *paint_tile = state.hover_tile;
        if state.sign_dialog.is_some() {
            feedback(state, MapInteractionFeedback::StartSignInput);
        }
        if placed && let Some(effect) = one_click_placement_sound(state.tool) {
            feedback(state, MapInteractionFeedback::PlaySound(effect));
        }
    }
}

pub(super) fn update_map_paint_pointer_motion(
    state: &mut ViewerState,
    screen: Vec2,
    window_size: (u32, u32),
    pixel_scale: f32,
    painting: bool,
    paint_tile: &mut Option<(usize, usize)>,
    shift_down: bool,
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
) {
    move_map_pointer_interaction(
        state,
        screen,
        window_size,
        pixel_scale,
        painting,
        paint_tile,
        shift_down,
        |state, effect| apply_map_feedback(state, effect, video, window, None),
    );
}

pub(super) fn move_map_pointer_interaction(
    state: &mut ViewerState,
    screen: Vec2,
    window_size: (u32, u32),
    pixel_scale: f32,
    painting: bool,
    paint_tile: &mut Option<(usize, usize)>,
    shift_down: bool,
    mut feedback: impl FnMut(&ViewerState, MapInteractionFeedback),
) {
    state.update_hover_tile(screen, window_size, pixel_scale);
    if !painting {
        return;
    }
    if state.zone_drag_active() {
        state.update_zone_drag_at(screen, window_size, pixel_scale);
        state.refresh_status_line();
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.network_drag_active() {
        state.update_network_drag_at(screen, window_size, pixel_scale);
        state.refresh_status_line();
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.underground_drag_active() {
        state.update_underground_drag_at(screen, window_size, pixel_scale);
        state.refresh_status_line();
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.highway_drag_active() {
        state.update_highway_drag_at(screen, window_size, pixel_scale);
        state.refresh_status_line();
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.terrain_drag_active() {
        state.update_terrain_drag_at(screen, window_size, pixel_scale);
        state.refresh_status_line();
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.edit_map_stretch_drag_active() {
        state.update_edit_map_stretch_drag_at(screen.y);
        state.refresh_status_line();
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.edit_map_lower_drag_active() {
        state.update_edit_map_lower_drag_at(screen.y);
        state.refresh_status_line();
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.edit_map_level_drag_active() {
        state.update_edit_map_level_drag_at(screen.y);
        state.refresh_status_line();
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.nature_drag_active() {
        state.update_nature_drag_at(screen, window_size, pixel_scale);
        state.refresh_status_line();
        feedback(state, MapInteractionFeedback::UpdateTitle);
    } else if state.mode == AppMode::EditNewMap {
        if let Some(tile) = state.hover_tile {
            let (_, next_tile) = state.paint_edit_map_tool_drag_to(*paint_tile, tile, shift_down);
            *paint_tile = next_tile;
        }
    } else if state.mode == AppMode::CityView {
        if let Some(tile) = state.hover_tile {
            let (_, next_tile) = state.paint_city_tool_drag_to(*paint_tile, tile);
            *paint_tile = next_tile;
        }
    }
}

fn finish_map_left_pointer_up(
    state: &mut ViewerState,
    paint_tile: &mut Option<(usize, usize)>,
    shift_down: bool,
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    sound: Option<&crate::audio::SoundPlayer>,
) {
    release_map_tools(state, *paint_tile, shift_down, window, sound);
    *paint_tile = None;
    state.release_palette_button();
    state.update_window_title(video, window.id());
}

/// Commit/cancel tools, confirmations and held-tool cleanup before caller flag resets.
pub(super) fn release_map_tools(
    state: &mut ViewerState,
    paint_tile: Option<(usize, usize)>,
    shift_down: bool,
    window: &sdl3::video::Window,
    sound: Option<&crate::audio::SoundPlayer>,
) {
    state.edit_map_slider_drag = None;
    let outcome = state.release_left_mouse_drag(shift_down);
    show_highway_neighbor_link_confirmation(state, window, outcome.highway_neighbor_link);
    show_rail_neighbor_link_confirmation(state, window, outcome.rail_neighbor_link);
    show_road_neighbor_link_confirmation(state, window, outcome.road_neighbor_link);
    let sound_effects = state.sound_effects_player(sound);
    if outcome.placed_zones > 0 {
        play_placement_sound(sound_effects, zone_placement_sound(state.tool));
    }
    if let Some((kind, placed_tiles)) = outcome.network
        && placed_tiles > 0
    {
        play_placement_sound(sound_effects, network_placement_sound(kind));
    }
    if let Some((kind, placed_tiles)) = outcome.underground
        && placed_tiles > 0
    {
        play_placement_sound(sound_effects, underground_placement_sound(kind));
    }
    if outcome.highway_segments > 0 || outcome.terrain_tiles > 0 {
        play_placement_sound(sound_effects, PlacementSound::Build);
    }
    if let Some((tool, placed_tiles)) = outcome.nature {
        play_nature_placement_sound(sound_effects, tool, placed_tiles);
    }
    if state.mode == AppMode::EditNewMap
        && state.tool == EditTool::RaiseTerrain
        && state.engine_fidelity != EngineFidelity::Mac68k
        && let Some((tx, ty)) = paint_tile
    {
        pe_raise_tool0_clear_temp_xtxt_0xc9(&mut state.city, tx, ty);
        state.finish_edit_map_raise_hold();
    }
    if state.mode == AppMode::EditNewMap && state.tool == EditTool::SpreadTerrain {
        state.finish_edit_map_spread_hold();
    }
}
