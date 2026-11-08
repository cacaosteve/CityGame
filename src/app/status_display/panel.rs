//! Compact, non-modal tool/notice/weather display shared by desktop and touch layouts.
use super::super::dialog_content::wrap_dialog_text;
use super::super::{AppMode, FloatRect, MenubarUi, ViewerState, point_in_float_rect, push_ui_rect};
use crate::renderer::UiSolid;
use crate::ui;
use glam::Vec2;

pub(in crate::app) fn status_panel_rect(size: (u32, u32), pixel_scale: f32) -> FloatRect {
    let s = pixel_scale
        .max(1.0)
        .min(((size.1 as f32 - MenubarUi::occupied_top(pixel_scale)) / 110.0).max(0.25));
    let margin = 10.0 * s;
    let w = (size.0 as f32 * 0.34)
        .clamp(270.0 * s, 380.0 * s)
        .min((size.0 as f32 - 2.0 * margin).max(1.0));
    FloatRect {
        x: size.0 as f32 - margin - w,
        y: MenubarUi::occupied_top(pixel_scale) + margin,
        w,
        h: 88.0 * s,
    }
}

impl ViewerState {
    pub(in crate::app) fn point_in_status_panel(
        &self,
        screen: Vec2,
        size: (u32, u32),
        ps: f32,
    ) -> bool {
        point_in_float_rect(screen, status_panel_rect(size, ps))
    }

    pub(in crate::app) fn status_panel_notice(&self) -> String {
        if let Some(help) = self.balloon_help_line {
            return help.to_owned();
        }
        if self.query_is_inspect || self.query_status_pinned {
            return self.query_line.clone();
        }
        if self.mode == AppMode::EditNewMap {
            return "Shape the terrain, then choose Done to start your city.".into();
        }
        if self.city.pe_dw_tile_count[198..=207]
            .iter()
            .all(|&count| count == 0)
        {
            return "Power Plant Needed".into();
        }
        // The idle query retains diagnostics for development. Extract its cached
        // service notices here without recomputing power/water flood maps per frame.
        let date = self.game_clock.status_line();
        let line = self
            .query_line
            .split_once(&date)
            .map_or(self.query_line.as_str(), |(_, tail)| tail);
        let notice = line
            .split("  ")
            .map(str::trim)
            .filter(|s| {
                !s.is_empty()
                    && !s.starts_with("engine=")
                    && !s.starts_with("anim ")
                    && *s != "*PAUSED*"
            })
            .collect::<Vec<_>>()
            .join("; ");
        if notice.is_empty() {
            "City services ready".into()
        } else {
            notice
        }
    }

    pub(in crate::app) fn push_status_panel(
        &self,
        size: (u32, u32),
        ps: f32,
        solids: &mut Vec<UiSolid>,
    ) {
        let rect = status_panel_rect(size, ps);
        let s = rect.h / 88.0;
        let depth = ui::UI_DEPTH + 12.0;
        let (background, ink, muted, edge) = if self.dark_appearance {
            (
                [0.08, 0.15, 0.19, 0.97],
                [0.88, 0.96, 0.94, 1.0],
                [0.65, 0.80, 0.82, 1.0],
                [0.39, 0.76, 0.74, 1.0],
            )
        } else {
            (
                [0.88, 0.93, 0.93, 0.97],
                [0.06, 0.15, 0.19, 1.0],
                [0.22, 0.36, 0.40, 1.0],
                [0.28, 0.51, 0.54, 1.0],
            )
        };
        push_ui_rect(solids, rect, edge, depth);
        push_ui_rect(
            solids,
            FloatRect {
                x: rect.x + s,
                y: rect.y + s,
                w: rect.w - 2.0 * s,
                h: rect.h - 2.0 * s,
            },
            background,
            depth + 0.5,
        );
        let max_chars = ((rect.w - 24.0 * s) / (4.0 * 1.8 * s)).floor().max(1.0) as usize;
        let tool = self
            .selected_tool_status_line()
            .unwrap_or_else(|| self.tool.label().into());
        let title = format!("{tool} / {}", self.traffic.speed().label());
        for (row, line) in std::iter::once(title)
            .chain(
                wrap_dialog_text(&self.status_panel_notice(), max_chars)
                    .into_iter()
                    .take(2),
            )
            .enumerate()
        {
            let line: String = line.chars().take(max_chars).collect();
            ui::push_pixel_text_preserving_case(
                solids,
                &line,
                rect.x + 12.0 * s,
                rect.y + (11.0 + 18.0 * row as f32) * s,
                1.8 * s,
                ink,
                depth + 1.0,
            );
        }
        let weather = format!(
            "{} / Wind {}",
            super::super::weather::weather_status_label(&self.city),
            self.city.pe_b_weather_wind
        );
        ui::push_pixel_text_preserving_case(
            solids,
            &weather
                .chars()
                .take(max_chars.saturating_sub(5))
                .collect::<String>(),
            rect.x + 12.0 * s,
            rect.y + 68.0 * s,
            1.8 * s,
            muted,
            depth + 1.0,
        );
        // A small authored sun/cloud indicator; all values come from current weather state.
        let cloudy = self.city.pe_b_weather_trend >= 3 && self.city.pe_b_weather_trend != 4;
        let icon = if cloudy {
            [0.43, 0.65, 0.75, 1.0]
        } else {
            [0.96, 0.72, 0.23, 1.0]
        };
        let x = rect.x + rect.w - 29.0 * s;
        for (dx, dy, w, h) in if cloudy {
            vec![(0.0, 4.0, 18.0, 7.0), (4.0, 0.0, 8.0, 8.0)]
        } else {
            vec![
                (4.0, 3.0, 10.0, 10.0),
                (7.0, -2.0, 4.0, 3.0),
                (7.0, 15.0, 4.0, 3.0),
                (-1.0, 6.0, 3.0, 4.0),
                (16.0, 6.0, 3.0, 4.0),
            ]
        } {
            push_ui_rect(
                solids,
                FloatRect {
                    x: x + dx * s,
                    y: rect.y + (66.0 + dy) * s,
                    w: w * s,
                    h: h * s,
                },
                icon,
                depth + 1.0,
            );
        }
    }
}
