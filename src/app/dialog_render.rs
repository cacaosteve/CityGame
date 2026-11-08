//! Drawing routines for modal dialogs.

use super::dialog_content::*;
use super::dialog_layout::*;
use super::*;

impl ViewerState {
    pub(super) fn push_sign_labels(
        &self,
        window_size: (u32, u32),
        pixel_scale: f32,
        solids: &mut Vec<UiSolid>,
    ) {
        if !self.show_signs || self.show_underground || self.power_plant_dialog.is_some() {
            return;
        }
        let screen_w = window_size.0 as f32;
        let screen_h = window_size.1 as f32;
        let scale = (2.0 * pixel_scale).round().max(2.0);
        let padding_x = 3.0 * scale;
        let padding_y = 2.0 * scale;
        let text_h = 5.0 * scale;
        let depth = ui::UI_DEPTH + 20.0;

        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                let Some(sign_id) = self.city.text_overlay_at(x, y) else {
                    continue;
                };
                if !is_user_sign(sign_id) {
                    continue;
                }
                let Some(label) = self.city.label_at(sign_id) else {
                    continue;
                };
                if label.is_empty() {
                    continue;
                }

                let width = ui::pixel_text_width(label, scale) + padding_x * 2.0;
                let height = text_h + padding_y * 2.0;
                let (anchor, _, _, _) = self.structure_anchor(window_size, pixel_scale, x, y);
                let label_x = (anchor.x - width * 0.5).clamp(0.0, (screen_w - width).max(0.0));
                let label_y = (anchor.y - height - 10.0 * pixel_scale)
                    .clamp(0.0, (screen_h - height).max(0.0));
                let rect = FloatRect {
                    x: label_x,
                    y: label_y,
                    w: width,
                    h: height,
                };
                push_ui_rect(solids, rect, [0.68, 0.68, 0.68, 1.0], depth);
                push_ui_bevel(solids, rect, scale, false, depth + 0.1);
                ui::push_pixel_text(
                    solids,
                    label,
                    label_x + padding_x,
                    label_y + padding_y,
                    scale,
                    [0.0, 0.0, 0.0, 1.0],
                    depth + 0.2,
                );
            }
        }
    }

    pub(super) fn push_sign_dialog(&self, window_size: (u32, u32), solids: &mut Vec<UiSolid>) {
        let Some(dialog) = &self.sign_dialog else {
            return;
        };
        let rect = sign_dialog_rect(window_size);
        let field = sign_dialog_field_rect(rect);
        let ok = sign_dialog_ok_rect(rect);
        let cancel = sign_dialog_cancel_rect(rect);
        let depth = ui::UI_DEPTH + 250.0;
        let scale = 3.0;

        push_ui_rect(solids, rect, [0.66, 0.66, 0.66, 1.0], depth);
        push_ui_bevel(solids, rect, 3.0, false, depth + 0.1);
        ui::push_pixel_text(
            solids,
            "Sign",
            rect.x + (rect.w - ui::pixel_text_width("Sign", scale)) * 0.5,
            rect.y + 12.0,
            scale,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.2,
        );

        push_ui_rect(solids, field, [0.88, 0.88, 0.88, 1.0], depth + 0.2);
        push_ui_bevel(solids, field, 2.0, true, depth + 0.3);
        let text = format!("{}_", dialog.text);
        ui::push_pixel_text(
            solids,
            &text,
            field.x + 8.0,
            field.y + 8.0,
            scale,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );

        push_dialog_button(solids, ok, "OK", false, depth + 0.5);
        push_dialog_button(solids, cancel, "Cancel", false, depth + 0.5);
    }

    pub(super) fn push_military_base_dialog(
        &self,
        window_size: (u32, u32),
        solids: &mut Vec<UiSolid>,
    ) {
        let Some(dialog) = &self.military_base_dialog else {
            return;
        };
        let rect = military_base_dialog_rect(window_size);
        let accept = military_base_accept_rect(rect);
        let decline = military_base_decline_rect(rect);
        let depth = ui::UI_DEPTH + 260.0;
        let scale = 3.0;
        let title = "Military Base";
        let body = "The military wants a base in your city.";

        push_ui_rect(solids, rect, [0.66, 0.66, 0.66, 1.0], depth);
        push_ui_bevel(solids, rect, 3.0, false, depth + 0.1);
        ui::push_pixel_text(
            solids,
            title,
            rect.x + (rect.w - ui::pixel_text_width(title, scale)) * 0.5,
            rect.y + 16.0,
            scale,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.2,
        );
        ui::push_pixel_text(
            solids,
            body,
            rect.x + (rect.w - ui::pixel_text_width(body, 2.0)) * 0.5,
            rect.y + 56.0,
            2.0,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.2,
        );
        push_dialog_button(
            solids,
            accept,
            "Accept",
            dialog.pressed == Some(MilitaryBaseDialogAction::Accept),
            depth + 0.5,
        );
        push_dialog_button(
            solids,
            decline,
            "Decline",
            dialog.pressed == Some(MilitaryBaseDialogAction::Decline),
            depth + 0.5,
        );
    }

    pub(super) fn push_power_plant_dialog(
        &self,
        renderer: &Renderer<'_>,
        window_size: (u32, u32),
        images: &mut Vec<UiImage>,
        solids: &mut Vec<UiSolid>,
    ) {
        let Some(dialog) = self.power_plant_dialog else {
            return;
        };
        let dialog_rect = power_plant_dialog_rect(window_size);
        let layout_scale = power_plant_dialog_scale(dialog_rect);
        let text_scale = POWER_PLANT_UI_SCALE * layout_scale;
        let depth = ui::UI_DEPTH + 300.0;
        push_ui_rect(solids, dialog_rect, [0.62, 0.62, 0.62, 1.0], depth);
        push_ui_bevel(solids, dialog_rect, 3.0, false, depth + 0.1);

        let title = "Select Power Plant";
        ui::push_pixel_text(
            solids,
            title,
            dialog_rect.x + (dialog_rect.w - ui::pixel_text_width(title, text_scale)) * 0.5,
            dialog_rect.y + 16.0 * layout_scale,
            text_scale,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );
        let cancel_rect = power_plant_cancel_rect(dialog_rect);
        push_dialog_button(solids, cancel_rect, "Cancel", false, depth + 0.2);

        for (index, choice) in POWER_PLANT_CHOICES.iter().enumerate() {
            let card = power_plant_card_rect(dialog_rect, index);
            let info = power_plant_info_rect(dialog_rect, index);
            push_dialog_button(solids, info, "INFO", false, depth + 0.2);
            let enabled = self.chooser_tool_enabled(choice.tool);
            let pressed = enabled && dialog.pressed_tool == Some(choice.tool);
            push_ui_rect(
                solids,
                card,
                if !enabled {
                    [0.56, 0.56, 0.56, 1.0]
                } else if pressed {
                    [0.44, 0.44, 0.44, 1.0]
                } else {
                    [0.64, 0.64, 0.64, 1.0]
                },
                depth + 0.1,
            );
            push_ui_bevel(solids, card, 3.0, pressed, depth + 0.2);

            if let Some(sprite_id) = building_sprite_id(choice.building)
                && let Some(atlas) = renderer.atlas_for_sprite(sprite_id)
                && let (resolved_atlas, resolved_id) =
                    renderer.resolve_sprite_frame(atlas, sprite_id, self.animation_frame())
                && let Some([src_x, src_y, src_w, src_h]) =
                    renderer.sprite_frame_rect(resolved_atlas, resolved_id)
            {
                let scale = ((card.w - 28.0 * layout_scale) / src_w as f32)
                    .min((card.h - 62.0 * layout_scale) / src_h as f32)
                    .min(1.25 * layout_scale);
                let w = src_w as f32 * scale;
                let h = src_h as f32 * scale;
                images.push(UiImage {
                    atlas: resolved_atlas,
                    src_x,
                    src_y,
                    src_w,
                    src_h,
                    x: card.x + (card.w - w) * 0.5,
                    y: card.y + 8.0 * layout_scale,
                    w,
                    h,
                    depth: depth + 1.0 + index as f32 * 0.01,
                    tint: if enabled {
                        [1.0, 1.0, 1.0, 1.0]
                    } else {
                        [0.55, 0.55, 0.55, 1.0]
                    },
                });
            }

            let mw = format!("{} Mw", choice.megawatts);
            let cost = format!("${}", format_number(choice.cost));
            let text_color = if enabled {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                [0.24, 0.24, 0.24, 1.0]
            };
            ui::push_pixel_text(
                solids,
                &mw,
                card.x + 22.0 * layout_scale,
                card.y + card.h - 40.0 * layout_scale,
                text_scale,
                text_color,
                depth + 0.4,
            );
            ui::push_pixel_text(
                solids,
                &cost,
                card.x + card.w - ui::pixel_text_width(&cost, text_scale) - 18.0 * layout_scale,
                card.y + card.h - 40.0 * layout_scale,
                text_scale,
                text_color,
                depth + 0.4,
            );
            ui::push_pixel_text(
                solids,
                choice.label,
                card.x + (card.w - ui::pixel_text_width(choice.label, text_scale)) * 0.5,
                card.y + card.h - 18.0 * layout_scale,
                text_scale,
                text_color,
                depth + 0.4,
            );
        }

        if let Some(tool) = dialog.info_tool
            && let Some(choice) = power_plant_choice_for_tool(tool)
        {
            let info_rect = power_plant_info_panel_rect(dialog_rect);
            push_ui_rect(solids, info_rect, [0.70, 0.70, 0.70, 1.0], depth + 30.0);
            push_ui_bevel(solids, info_rect, 4.0, false, depth + 30.1);
            let mut y = info_rect.y + 24.0 * layout_scale;
            for line in wrap_dialog_text(choice.info, 58) {
                ui::push_pixel_text(
                    solids,
                    &line,
                    info_rect.x + 16.0 * layout_scale,
                    y,
                    text_scale,
                    [0.0, 0.0, 0.0, 1.0],
                    depth + 30.3,
                );
                y += 18.0 * layout_scale;
            }
        }
    }

    pub(super) fn push_arcology_dialog(
        &self,
        renderer: &Renderer<'_>,
        window_size: (u32, u32),
        images: &mut Vec<UiImage>,
        solids: &mut Vec<UiSolid>,
    ) {
        let Some(dialog) = self.arcology_dialog else {
            return;
        };
        let dialog_rect = arcology_dialog_rect(window_size);
        let layout_scale = arcology_dialog_scale(dialog_rect);
        let text_scale = ARCOLOGY_UI_SCALE * layout_scale;
        let depth = ui::UI_DEPTH + 300.0;
        push_ui_rect(solids, dialog_rect, [0.62, 0.62, 0.62, 1.0], depth);
        push_ui_bevel(solids, dialog_rect, 3.0, false, depth + 0.1);

        let title = "Select Arcology";
        ui::push_pixel_text(
            solids,
            title,
            dialog_rect.x + (dialog_rect.w - ui::pixel_text_width(title, text_scale)) * 0.5,
            dialog_rect.y + 16.0 * layout_scale,
            text_scale,
            [0.0, 0.0, 0.0, 1.0],
            depth + 0.4,
        );
        push_dialog_button(
            solids,
            arcology_cancel_rect(dialog_rect),
            "Cancel",
            false,
            depth + 0.2,
        );

        for (index, choice) in ARCOLOGY_CHOICES.iter().enumerate() {
            let card = arcology_card_rect(dialog_rect, index);
            push_dialog_button(
                solids,
                arcology_info_rect(dialog_rect, index),
                "INFO",
                false,
                depth + 0.2,
            );
            let enabled = self.chooser_tool_enabled(choice.tool);
            let pressed = enabled && dialog.pressed_tool == Some(choice.tool);
            push_ui_rect(
                solids,
                card,
                if !enabled {
                    [0.56, 0.56, 0.56, 1.0]
                } else if pressed {
                    [0.44, 0.44, 0.44, 1.0]
                } else {
                    [0.64, 0.64, 0.64, 1.0]
                },
                depth + 0.1,
            );
            push_ui_bevel(solids, card, 3.0 * layout_scale, pressed, depth + 0.2);

            if let Some(sprite_id) = building_sprite_id(choice.building)
                && let Some(atlas) = renderer.atlas_for_sprite(sprite_id)
                && let (resolved_atlas, resolved_id) =
                    renderer.resolve_sprite_frame(atlas, sprite_id, self.animation_frame())
                && let Some([src_x, src_y, src_w, src_h]) =
                    renderer.sprite_frame_rect(resolved_atlas, resolved_id)
            {
                let scale = ((card.w - 24.0 * layout_scale) / src_w as f32)
                    .min((card.h - 58.0 * layout_scale) / src_h as f32)
                    .min(1.25 * layout_scale);
                let w = src_w as f32 * scale;
                let h = src_h as f32 * scale;
                images.push(UiImage {
                    atlas: resolved_atlas,
                    src_x,
                    src_y,
                    src_w,
                    src_h,
                    x: card.x + (card.w - w) * 0.5,
                    y: card.y + 8.0 * layout_scale,
                    w,
                    h,
                    depth: depth + 1.0 + index as f32 * 0.01,
                    tint: if enabled {
                        [1.0, 1.0, 1.0, 1.0]
                    } else {
                        [0.55, 0.55, 0.55, 1.0]
                    },
                });
            }

            let capacity = format!("{} people", format_number(choice.capacity));
            let cost = format!("${}", format_number(choice.cost));
            let text_color = if enabled {
                [0.0, 0.0, 0.0, 1.0]
            } else {
                [0.24, 0.24, 0.24, 1.0]
            };
            ui::push_pixel_text(
                solids,
                &capacity,
                card.x + 16.0 * layout_scale,
                card.y + card.h - 38.0 * layout_scale,
                text_scale,
                text_color,
                depth + 0.4,
            );
            ui::push_pixel_text(
                solids,
                &cost,
                card.x + card.w - ui::pixel_text_width(&cost, text_scale) - 16.0 * layout_scale,
                card.y + card.h - 38.0 * layout_scale,
                text_scale,
                text_color,
                depth + 0.4,
            );
            ui::push_pixel_text(
                solids,
                choice.label,
                card.x + (card.w - ui::pixel_text_width(choice.label, text_scale)) * 0.5,
                card.y + card.h - 17.0 * layout_scale,
                text_scale,
                text_color,
                depth + 0.4,
            );
        }

        if let Some(tool) = dialog.info_tool
            && let Some(choice) = arcology_choice_for_tool(tool)
        {
            let info_rect = arcology_info_panel_rect(dialog_rect);
            push_ui_rect(solids, info_rect, [0.70, 0.70, 0.70, 1.0], depth + 30.0);
            push_ui_bevel(solids, info_rect, 4.0, false, depth + 30.1);
            let mut y = info_rect.y + 22.0 * layout_scale;
            for line in wrap_dialog_text(choice.info, 52) {
                ui::push_pixel_text(
                    solids,
                    &line,
                    info_rect.x + 18.0 * layout_scale,
                    y,
                    text_scale,
                    [0.0, 0.0, 0.0, 1.0],
                    depth + 30.3,
                );
                y += 18.0 * layout_scale;
            }
        }
    }
}
