//! Shared geometry and hit-testing for modal and city-information dialogs.

use super::dialogs::{
    ARCOLOGY_CARD_H, ARCOLOGY_CARD_W, ARCOLOGY_CHOICES, ARCOLOGY_DIALOG_H, ARCOLOGY_DIALOG_W,
    ARCOLOGY_INFO_H, ARCOLOGY_INFO_W, ArcologyDialogAction, CITY_INFO_DIALOG_H, CITY_INFO_DIALOG_W,
    CITY_MAP_GRID, CityInfoDialog, MilitaryBaseDialogAction, POWER_PLANT_CARD_H,
    POWER_PLANT_CARD_W, POWER_PLANT_CHOICES, POWER_PLANT_DIALOG_H, POWER_PLANT_DIALOG_W,
    POWER_PLANT_INFO_H, POWER_PLANT_INFO_W, PowerPlantDialogAction, SignDialogAction,
};
use super::*;
use sdl3::rect::Rect;

pub(super) fn city_info_dialog_rect(window_size: (u32, u32)) -> FloatRect {
    let screen_w = window_size.0 as f32;
    let screen_h = window_size.1 as f32;
    let w = CITY_INFO_DIALOG_W.min((screen_w - 24.0).max(320.0));
    let h = CITY_INFO_DIALOG_H.min((screen_h - 24.0).max(320.0));
    FloatRect {
        x: ((screen_w - w) * 0.5).max(0.0),
        y: ((screen_h - h) * 0.5).max(0.0),
        w,
        h,
    }
}

pub(super) fn city_info_dialog_rect_for(
    dialog: CityInfoDialog,
    window_size: (u32, u32),
) -> FloatRect {
    if matches!(dialog, CityInfoDialog::Newspaper(_)) {
        newspaper::page_rect(window_size)
    } else if dialog == CityInfoDialog::Budget {
        budget_window::page_rect(window_size)
    } else if matches!(
        dialog,
        CityInfoDialog::RoadBridge { .. }
            | CityInfoDialog::RailBridge { .. }
            | CityInfoDialog::PowerBridge { .. }
            | CityInfoDialog::HighwayWaterBridge { .. }
    ) {
        let w = 620.0_f32.min(window_size.0 as f32 - 24.0).max(320.0);
        let h = 330.0_f32.min(window_size.1 as f32 - 24.0).max(260.0);
        FloatRect {
            x: (window_size.0 as f32 - w) * 0.5,
            y: (window_size.1 as f32 - h) * 0.5,
            w,
            h,
        }
    } else if dialog == CityInfoDialog::FiscalCrisis {
        let w = 600.0_f32.min(window_size.0 as f32 - 24.0).max(200.0);
        let h = 260.0_f32.min(window_size.1 as f32 - 24.0).max(160.0);
        FloatRect {
            x: (window_size.0 as f32 - w) * 0.5,
            y: (window_size.1 as f32 - h) * 0.5,
            w,
            h,
        }
    } else {
        city_info_dialog_rect(window_size)
    }
}

pub(super) fn road_bridge_choice_rect(dialog: FloatRect, ordinal: usize) -> FloatRect {
    FloatRect {
        x: dialog.x + 34.0,
        y: dialog.y + 96.0 + ordinal as f32 * 58.0,
        w: dialog.w - 68.0,
        h: 46.0,
    }
}

pub(super) fn city_info_close_rect(dialog: FloatRect) -> FloatRect {
    FloatRect {
        x: dialog.x + dialog.w - 104.0,
        y: dialog.y + 10.0,
        w: 82.0,
        h: 30.0,
    }
}

pub(super) fn city_map_panel_rect_and_cell(dialog: FloatRect) -> (FloatRect, f32) {
    let cell = ((dialog.h - 100.0) / CITY_MAP_GRID as f32)
        .floor()
        .clamp(3.0, 11.0);
    let map_size = cell * CITY_MAP_GRID as f32;
    (
        FloatRect {
            x: dialog.x + 24.0,
            y: dialog.y + 64.0,
            w: map_size,
            h: map_size,
        },
        cell,
    )
}

pub(super) fn city_map_tile_for_point(point: Vec2, dialog: FloatRect) -> Option<(usize, usize)> {
    const TILE_BLOCK: usize = CITY_WIDTH / CITY_MAP_GRID;
    let (map, cell) = city_map_panel_rect_and_cell(dialog);
    if !point_in_float_rect(point, map) {
        return None;
    }
    let map_x = ((point.x - map.x) / cell).floor() as usize;
    let map_y = ((point.y - map.y) / cell).floor() as usize;
    let tile_x = (map_x.min(CITY_MAP_GRID - 1) * TILE_BLOCK + TILE_BLOCK / 2).min(CITY_WIDTH - 1);
    let tile_y = (map_y.min(CITY_MAP_GRID - 1) * TILE_BLOCK + TILE_BLOCK / 2).min(CITY_HEIGHT - 1);
    Some((tile_x, tile_y))
}

pub(super) fn city_map_cell_rect_for_tile(
    tile_x: usize,
    tile_y: usize,
    map: FloatRect,
    cell: f32,
) -> FloatRect {
    let map_x = (tile_x.min(CITY_WIDTH - 1) * CITY_MAP_GRID / CITY_WIDTH).min(CITY_MAP_GRID - 1);
    let map_y = (tile_y.min(CITY_HEIGHT - 1) * CITY_MAP_GRID / CITY_HEIGHT).min(CITY_MAP_GRID - 1);
    FloatRect {
        x: map.x + map_x as f32 * cell,
        y: map.y + map_y as f32 * cell,
        w: cell.max(1.0),
        h: cell.max(1.0),
    }
}

pub(super) fn graph_legend_rect(dialog: FloatRect, series_index: usize) -> FloatRect {
    let col = series_index % 3;
    let row = series_index / 3;
    FloatRect {
        x: dialog.x + 46.0 + col as f32 * 186.0,
        y: dialog.y + 58.0 + row as f32 * 22.0,
        w: 168.0,
        h: 18.0,
    }
}

pub(super) fn graph_scale_rect(dialog: FloatRect, scale_index: usize) -> FloatRect {
    FloatRect {
        x: dialog.x + 54.0 + scale_index as f32 * 204.0,
        y: dialog.y + dialog.h - 48.0,
        w: 188.0,
        h: 28.0,
    }
}

pub(super) fn budget_tax_adjust_rect(
    dialog: FloatRect,
    tax_index: usize,
    increase: bool,
) -> FloatRect {
    budget_window::tax_adjust_rect(dialog, tax_index, increase)
}

pub(super) fn budget_service_funding_adjust_rect(
    dialog: FloatRect,
    service_index: usize,
    increase: bool,
) -> FloatRect {
    budget_window::service_adjust_rect(dialog, service_index, increase)
}

pub(super) fn budget_auto_budget_rect(dialog: FloatRect) -> FloatRect {
    budget_window::auto_rect(dialog)
}

pub(super) fn budget_bond_button_rect(dialog: FloatRect, issue: bool) -> FloatRect {
    budget_window::bond_rect(dialog, issue)
}

pub(super) fn industry_row_y(dialog: FloatRect, index: usize) -> f32 {
    dialog.y + 106.0 + index as f32 * 31.0
}

pub(super) fn industry_tax_adjust_rect(
    dialog: FloatRect,
    industry_index: usize,
    increase: bool,
) -> FloatRect {
    let y = industry_row_y(dialog, industry_index);
    FloatRect {
        x: dialog.x + if increase { 498.0 } else { 410.0 },
        y: y - 6.0,
        w: 24.0,
        h: 24.0,
    }
}

pub(super) fn ordinance_row_rect(dialog: FloatRect, index: usize) -> FloatRect {
    let col = index / 10;
    let row = index % 10;
    FloatRect {
        x: dialog.x + 34.0 + col as f32 * 338.0,
        y: dialog.y + 104.0 + row as f32 * 32.0,
        w: 308.0,
        h: 28.0,
    }
}

pub(super) fn ordinance_checkbox_rect(row: FloatRect) -> FloatRect {
    FloatRect {
        x: row.x + 8.0,
        y: row.y + 5.0,
        w: 18.0,
        h: 18.0,
    }
}

pub(super) fn power_plant_dialog_rect(window_size: (u32, u32)) -> FloatRect {
    responsive_picker_rect(window_size, POWER_PLANT_DIALOG_W, POWER_PLANT_DIALOG_H)
}

fn responsive_picker_rect(window_size: (u32, u32), base_w: f32, base_h: f32) -> FloatRect {
    let screen_w = window_size.0 as f32;
    let screen_h = window_size.1 as f32;
    let available_scale =
        ((screen_w - 24.0).max(1.0) / base_w).min((screen_h - 24.0).max(1.0) / base_h);
    let scale = available_scale.clamp(0.58, 1.16).min(available_scale);
    let w = base_w * scale;
    let h = base_h * scale;
    FloatRect {
        x: (screen_w - w) * 0.5,
        y: (screen_h - h) * 0.5,
        w,
        h,
    }
}

pub(super) fn power_plant_dialog_scale(dialog: FloatRect) -> f32 {
    dialog.w / POWER_PLANT_DIALOG_W
}

pub(super) fn power_plant_cancel_rect(dialog: FloatRect) -> FloatRect {
    let s = power_plant_dialog_scale(dialog);
    FloatRect {
        x: dialog.x + dialog.w - 132.0 * s,
        y: dialog.y + 10.0 * s,
        w: 104.0 * s,
        h: 32.0 * s,
    }
}

pub(super) fn power_plant_card_rect(dialog: FloatRect, index: usize) -> FloatRect {
    let s = power_plant_dialog_scale(dialog);
    let col = index % 3;
    let row = index / 3;
    FloatRect {
        x: dialog.x + 24.0 * s + col as f32 * 216.0 * s,
        y: dialog.y + 76.0 * s + row as f32 * 166.0 * s,
        w: POWER_PLANT_CARD_W * s,
        h: POWER_PLANT_CARD_H * s,
    }
}

pub(super) fn power_plant_info_rect(dialog: FloatRect, index: usize) -> FloatRect {
    let card = power_plant_card_rect(dialog, index);
    let s = power_plant_dialog_scale(dialog);
    FloatRect {
        x: card.x + (card.w - POWER_PLANT_INFO_W * s) * 0.5,
        y: card.y - 32.0 * s,
        w: POWER_PLANT_INFO_W * s,
        h: POWER_PLANT_INFO_H * s,
    }
}

pub(super) fn power_plant_info_panel_rect(dialog: FloatRect) -> FloatRect {
    let s = power_plant_dialog_scale(dialog);
    FloatRect {
        x: dialog.x + 68.0 * s,
        y: dialog.y + 232.0 * s,
        w: dialog.w - 136.0 * s,
        h: 132.0 * s,
    }
}

pub(super) fn arcology_dialog_rect(window_size: (u32, u32)) -> FloatRect {
    responsive_picker_rect(window_size, ARCOLOGY_DIALOG_W, ARCOLOGY_DIALOG_H)
}

pub(super) fn arcology_dialog_scale(dialog: FloatRect) -> f32 {
    dialog.w / ARCOLOGY_DIALOG_W
}

pub(super) fn arcology_cancel_rect(dialog: FloatRect) -> FloatRect {
    let s = arcology_dialog_scale(dialog);
    FloatRect {
        x: dialog.x + dialog.w - 132.0 * s,
        y: dialog.y + 10.0 * s,
        w: 104.0 * s,
        h: 32.0 * s,
    }
}

pub(super) fn arcology_card_rect(dialog: FloatRect, index: usize) -> FloatRect {
    let s = arcology_dialog_scale(dialog);
    let col = index % 2;
    let row = index / 2;
    FloatRect {
        x: dialog.x + 28.0 * s + col as f32 * 296.0 * s,
        y: dialog.y + 86.0 * s + row as f32 * 184.0 * s,
        w: ARCOLOGY_CARD_W.min((ARCOLOGY_DIALOG_W - 84.0) * 0.5) * s,
        h: ARCOLOGY_CARD_H * s,
    }
}

pub(super) fn arcology_info_rect(dialog: FloatRect, index: usize) -> FloatRect {
    let card = arcology_card_rect(dialog, index);
    let s = arcology_dialog_scale(dialog);
    FloatRect {
        x: card.x + (card.w - ARCOLOGY_INFO_W * s) * 0.5,
        y: card.y - 34.0 * s,
        w: ARCOLOGY_INFO_W * s,
        h: ARCOLOGY_INFO_H * s,
    }
}

pub(super) fn arcology_info_panel_rect(dialog: FloatRect) -> FloatRect {
    let s = arcology_dialog_scale(dialog);
    FloatRect {
        x: dialog.x + 52.0 * s,
        y: dialog.y + 158.0 * s,
        w: dialog.w - 104.0 * s,
        h: 154.0 * s,
    }
}

pub(super) fn sign_dialog_rect(window_size: (u32, u32)) -> FloatRect {
    let w = 360.0;
    let h = 150.0;
    FloatRect {
        x: (window_size.0 as f32 - w) * 0.5,
        y: (window_size.1 as f32 - h) * 0.5,
        w,
        h,
    }
}

/// Keep the soft keyboard up while the sign dialog is open (tablets / touch devices).
pub(super) fn sync_sign_text_input(
    video: &sdl3::VideoSubsystem,
    window: &sdl3::video::Window,
    sign_open: bool,
    window_size: (u32, u32),
) {
    let text = video.text_input();
    if sign_open {
        let field = sign_dialog_field_rect(sign_dialog_rect(window_size));
        let rect = Rect::new(
            field.x.round() as i32,
            field.y.round() as i32,
            field.w.round().max(1.0) as u32,
            field.h.round().max(1.0) as u32,
        );
        text.set_rect(window, rect, 0);
        if !text.is_active(window) {
            text.start(window);
            if text.has_screen_keyboard_support() {
                log::debug!("screen keyboard support present; text input started for sign dialog");
            }
        }
    } else if text.is_active(window) {
        text.stop(window);
    }
}

pub(super) fn sign_dialog_field_rect(dialog: FloatRect) -> FloatRect {
    FloatRect {
        x: dialog.x + 28.0,
        y: dialog.y + 48.0,
        w: dialog.w - 56.0,
        h: 34.0,
    }
}

pub(super) fn sign_dialog_ok_rect(dialog: FloatRect) -> FloatRect {
    FloatRect {
        x: dialog.x + dialog.w - 176.0,
        y: dialog.y + dialog.h - 44.0,
        w: 68.0,
        h: 28.0,
    }
}

pub(super) fn sign_dialog_cancel_rect(dialog: FloatRect) -> FloatRect {
    FloatRect {
        x: dialog.x + dialog.w - 96.0,
        y: dialog.y + dialog.h - 44.0,
        w: 76.0,
        h: 28.0,
    }
}

pub(super) fn military_base_dialog_rect(window_size: (u32, u32)) -> FloatRect {
    let w = 420.0;
    let h = 150.0;
    FloatRect {
        x: (window_size.0 as f32 - w) * 0.5,
        y: (window_size.1 as f32 - h) * 0.5,
        w,
        h,
    }
}

pub(super) fn military_base_accept_rect(dialog: FloatRect) -> FloatRect {
    FloatRect {
        x: dialog.x + dialog.w - 196.0,
        y: dialog.y + dialog.h - 44.0,
        w: 84.0,
        h: 28.0,
    }
}

pub(super) fn military_base_decline_rect(dialog: FloatRect) -> FloatRect {
    FloatRect {
        x: dialog.x + dialog.w - 100.0,
        y: dialog.y + dialog.h - 44.0,
        w: 80.0,
        h: 28.0,
    }
}

pub(super) fn point_in_military_base_dialog(screen: Vec2, window_size: (u32, u32)) -> bool {
    point_in_float_rect(screen, military_base_dialog_rect(window_size))
}

pub(super) fn military_base_dialog_hit_test(
    screen: Vec2,
    window_size: (u32, u32),
) -> Option<MilitaryBaseDialogAction> {
    let dialog = military_base_dialog_rect(window_size);
    if !point_in_float_rect(screen, dialog) {
        return None;
    }
    if point_in_float_rect(screen, military_base_accept_rect(dialog)) {
        Some(MilitaryBaseDialogAction::Accept)
    } else if point_in_float_rect(screen, military_base_decline_rect(dialog)) {
        Some(MilitaryBaseDialogAction::Decline)
    } else {
        None
    }
}

pub(super) fn sign_dialog_hit_test(
    screen: Vec2,
    window_size: (u32, u32),
) -> Option<SignDialogAction> {
    let dialog = sign_dialog_rect(window_size);
    if !point_in_float_rect(screen, dialog) {
        return None;
    }
    if point_in_float_rect(screen, sign_dialog_ok_rect(dialog)) {
        Some(SignDialogAction::Commit)
    } else if point_in_float_rect(screen, sign_dialog_cancel_rect(dialog)) {
        Some(SignDialogAction::Cancel)
    } else {
        None
    }
}

pub(super) fn power_plant_dialog_hit_test(
    screen: Vec2,
    window_size: (u32, u32),
) -> Option<PowerPlantDialogAction> {
    let dialog = power_plant_dialog_rect(window_size);
    if !point_in_power_plant_dialog(screen, window_size) {
        return None;
    }
    if point_in_float_rect(screen, power_plant_cancel_rect(dialog)) {
        return Some(PowerPlantDialogAction::Cancel);
    }
    for (index, choice) in POWER_PLANT_CHOICES.iter().enumerate() {
        if point_in_float_rect(screen, power_plant_info_rect(dialog, index)) {
            return Some(PowerPlantDialogAction::Info(choice.tool));
        }
        if point_in_float_rect(screen, power_plant_card_rect(dialog, index)) {
            return Some(PowerPlantDialogAction::Select(choice.tool));
        }
    }
    None
}

pub(super) fn point_in_power_plant_dialog(screen: Vec2, window_size: (u32, u32)) -> bool {
    point_in_float_rect(screen, power_plant_dialog_rect(window_size))
}

pub(super) fn arcology_dialog_hit_test(
    screen: Vec2,
    window_size: (u32, u32),
) -> Option<ArcologyDialogAction> {
    let dialog = arcology_dialog_rect(window_size);
    if !point_in_arcology_dialog(screen, window_size) {
        return None;
    }
    if point_in_float_rect(screen, arcology_cancel_rect(dialog)) {
        return Some(ArcologyDialogAction::Cancel);
    }
    for (index, choice) in ARCOLOGY_CHOICES.iter().enumerate() {
        if point_in_float_rect(screen, arcology_info_rect(dialog, index)) {
            return Some(ArcologyDialogAction::Info(choice.tool));
        }
        if point_in_float_rect(screen, arcology_card_rect(dialog, index)) {
            return Some(ArcologyDialogAction::Select(choice.tool));
        }
    }
    None
}

pub(super) fn point_in_arcology_dialog(screen: Vec2, window_size: (u32, u32)) -> bool {
    point_in_float_rect(screen, arcology_dialog_rect(window_size))
}
