//! Compact, non-modal tool/notice/weather display shared by desktop and touch layouts.
use super::super::dialog_content::wrap_dialog_text;
use super::super::{AppMode, FloatRect, MenubarUi, ViewerState, point_in_float_rect, push_ui_rect};
use crate::renderer::{Renderer, UiImage, UiSolid};
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
        h: 44.0 * s,
    }
}

impl ViewerState {
    fn placed_status_panel_rect(&self, size: (u32, u32), ps: f32) -> FloatRect {
        let mut rect = status_panel_rect(size, ps);
        if let Some(origin) = self.status_panel_origin {
            rect.x = origin.x.clamp(0.0, (size.0 as f32 - rect.w).max(0.0));
            rect.y = origin.y.clamp(
                MenubarUi::occupied_top(ps).min((size.1 as f32 - rect.h).max(0.0)),
                (size.1 as f32 - rect.h).max(0.0),
            );
        }
        rect
    }

    pub(in crate::app) fn begin_status_panel_drag(
        &mut self,
        screen: Vec2,
        size: (u32, u32),
        ps: f32,
    ) -> bool {
        let rect = self.placed_status_panel_rect(size, ps);
        if !point_in_float_rect(screen, rect) {
            return false;
        }
        self.status_panel_grab = Some(screen - Vec2::new(rect.x, rect.y));
        true
    }

    pub(in crate::app) fn drag_status_panel(&mut self, screen: Vec2) -> bool {
        if let Some(grab) = self.status_panel_grab {
            self.status_panel_origin = Some(screen - grab);
            true
        } else {
            false
        }
    }

    pub(in crate::app) fn end_status_panel_drag(&mut self) {
        self.status_panel_grab = None;
    }

    pub(in crate::app) fn point_in_status_panel(
        &self,
        screen: Vec2,
        size: (u32, u32),
        ps: f32,
    ) -> bool {
        point_in_float_rect(screen, self.placed_status_panel_rect(size, ps))
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
        renderer: Option<&Renderer<'_>>,
        images: &mut Vec<UiImage>,
    ) {
        let rect = self.placed_status_panel_rect(size, ps);
        let s = rect.h / 44.0;
        let depth = ui::UI_DEPTH + 12.0;
        let (background, ink, muted, edge) = if self.dark_appearance {
            (
                [0.08, 0.15, 0.19, 0.97],
                [0.88, 0.96, 0.94, 1.0],
                [0.65, 0.80, 0.82, 1.0],
                [0.39, 0.76, 0.74, 1.0],
            )
        } else if !self.menu_checks.citygame_toolbar {
            (
                [0.76, 0.76, 0.76, 1.0],
                [0.0, 0.0, 0.0, 1.0],
                [0.15, 0.15, 0.15, 1.0],
                [0.15, 0.15, 0.15, 1.0],
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
        if !self.dark_appearance && !self.menu_checks.citygame_toolbar {
            for (x, y, w, h, color) in [
                (
                    rect.x + s,
                    rect.y + s,
                    rect.w - 2.0 * s,
                    s,
                    [1.0, 1.0, 1.0, 1.0],
                ),
                (
                    rect.x + s,
                    rect.y + s,
                    s,
                    rect.h - 2.0 * s,
                    [1.0, 1.0, 1.0, 1.0],
                ),
                (
                    rect.x + s,
                    rect.y + rect.h - 2.0 * s,
                    rect.w - 2.0 * s,
                    s,
                    [0.28, 0.28, 0.28, 1.0],
                ),
                (
                    rect.x + rect.w - 2.0 * s,
                    rect.y + s,
                    s,
                    rect.h - 2.0 * s,
                    [0.28, 0.28, 0.28, 1.0],
                ),
            ] {
                push_ui_rect(solids, FloatRect { x, y, w, h }, color, depth + 0.75);
            }
        }
        let max_chars = ((rect.w - 60.0 * s) / (4.0 * 1.8 * s)).floor().max(1.0) as usize;
        let tool = self
            .selected_tool_status_line()
            .unwrap_or_else(|| self.tool.label().into());
        let title = tool;
        for (row, line) in std::iter::once(title)
            .chain(
                wrap_dialog_text(&self.status_panel_notice(), max_chars)
                    .into_iter()
                    .take(1),
            )
            .enumerate()
        {
            let line: String = line.chars().take(max_chars).collect();
            ui::push_pixel_text_preserving_case(
                solids,
                &line,
                rect.x + 12.0 * s,
                rect.y + (5.0 + 18.0 * row as f32) * s,
                1.8 * s,
                ink,
                depth + 1.0,
            );
        }
        push_ui_rect(
            solids,
            FloatRect {
                x: rect.x + rect.w - 43.0 * s,
                y: rect.y + 2.0 * s,
                w: 2.0 * s,
                h: rect.h - 4.0 * s,
            },
            muted,
            depth + 1.0,
        );
        // Mac PICT 700..711 follow the twelve weather states in order.
        let picture = crate::assets::NEWSPAPER_PICTURE_BASE
            + 700
            + u32::from(super::super::weather::pe_weather_trend(&self.city));
        if let Some((atlas, [src_x, src_y, src_w, src_h])) = renderer.and_then(|r| {
            let atlas = r.atlas_for_sprite(picture)?;
            Some((atlas, r.sprite_frame_rect(atlas, picture)?))
        }) {
            images.push(UiImage {
                atlas,
                src_x,
                src_y,
                src_w,
                src_h,
                x: rect.x + rect.w - 39.0 * s,
                y: rect.y + 4.0 * s,
                w: 36.0 * s,
                h: 36.0 * s,
                depth: depth + 1.0,
                tint: [1.0; 4],
            });
            return;
        }
        // CityGame indicator used when the original Mac pictures are unavailable.
        let cloudy = self.city.pe_b_weather_trend >= 3 && self.city.pe_b_weather_trend != 4;
        let icon = if cloudy {
            [0.43, 0.65, 0.75, 1.0]
        } else {
            [0.96, 0.72, 0.23, 1.0]
        };
        let x = rect.x + rect.w - 31.0 * s;
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
                    y: rect.y + (15.0 + dy) * s,
                    w: w * s,
                    h: h * s,
                },
                icon,
                depth + 1.0,
            );
        }
    }
}
