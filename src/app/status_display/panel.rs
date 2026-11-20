//! Compact, non-modal tool/notice/weather display shared by desktop and touch layouts.
use super::super::dialog_content::wrap_dialog_text;
use super::super::{AppMode, FloatRect, MenubarUi, ViewerState, point_in_float_rect, push_ui_rect};
use crate::renderer::{Renderer, UiImage, UiSolid};
use crate::ui;
use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacStatusOverride {
    Paused,
    Action(u16),
}

fn mac_status_override(speed: crate::traffic::GameSpeed, city: &crate::city::City) -> Option<MacStatusOverride> {
    if speed == crate::traffic::GameSpeed::Paused {
        Some(MacStatusOverride::Paused)
    } else if city.mac_state_misc_u32_at(0x1048).unwrap_or(0) as u8 != 0 {
        Some(MacStatusOverride::Action(city.mac_state_misc_u32_at(0x1044).unwrap_or(0) as u16))
    } else {
        None
    }
}

fn mac_status_picture(city: &crate::city::City) -> u16 {
    if city.mac_state_misc_u32_at(0x1048).unwrap_or(0) as u8 != 0 {
        712
    } else {
        700 + (city.mac_state_misc_u32_at(super::super::MISC_WEATHER_TREND_OFFSET).unwrap_or(0) as u8) as u16
    }
}

fn mac_action_status_label(selector: u16) -> &'static str {
    use super::super::DisasterKind as D;
    match selector {
        0 => "None",
        1 => D::Fire.label(), 2 => D::Flood.label(), 3 => D::Rioters.label(),
        4 => D::PollutionAccident.label(), 5 => D::AirCrash.label(),
        6 => D::Earthquake.label(), 7 => D::Tornado.label(), 8 => D::Monster.label(),
        9 => D::Meltdown.label(), 10 => D::Microwave.label(), 11 => D::Volcano.label(),
        12 => D::Firestorm.label(), 13 => D::MassRiot.label(), 14 => D::BlizzardFlood.label(),
        15 => D::ToxicSpill.label(), 16 => D::Hurricane.label(),
        _ => "",
    }
}

#[cfg(test)]
mod mac_status_tests {
    use super::*;
    use crate::app::tests::{blank_city_layout_fixture, test_state};
    use crate::engine_fidelity::EngineFidelity;
    use crate::traffic::GameSpeed;

    #[test]
    fn pause_and_active_action_override_the_mac_city_notice() {
        for path in ["status-override.city", "status-override.sc2"] {
            let mut state = test_state(blank_city_layout_fixture(path, "Status"));
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.mac_loop_scheduler.status_message = Some(super::super::super::mac_loop_scheduler::MacStatusMessage::Story(54));
            state.set_sim_status_announcement("Older notice");
            state.city.set_mac_state_misc_u32_at(0x1044, 1);
            state.city.set_mac_state_misc_u32_at(0x1048, 1);
            state.traffic.set_speed(GameSpeed::Paused);
            assert_eq!(state.status_panel_notice(), "*PAUSED*");
            assert_eq!(mac_status_picture(&state.city), 712);
            state.traffic.set_speed(GameSpeed::Turtle);
            assert_eq!(state.status_panel_notice(), super::super::super::DisasterKind::Fire.label());
            state.city.set_mac_state_misc_u32_at(0x1044, 17);
            assert_eq!(state.status_panel_notice(), "");
            state.city.set_mac_state_misc_u32_at(0x1048, 0);
            state.city.set_mac_state_misc_u32_at(super::super::super::MISC_WEATHER_TREND_OFFSET, 7);
            assert_eq!(mac_status_picture(&state.city), 707);
            assert_eq!(state.status_panel_notice(), super::super::super::newspaper_stories::pe_health_amenities_newspaper_label(54).unwrap());
        }
    }

    #[test]
    #[ignore = "requires private original-Mac status override fixture"]
    fn status_override_matches_native_pointer_choice() {
        let path = std::env::var_os("CITYGAME_MAC_STATUS_OVERRIDE").unwrap();
        let cases: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for path in ["status-override-native.city", "status-override-native.sc2"] {
            let mut city = blank_city_layout_fixture(path, "Status");
            for row in cases.as_array().unwrap() {
                let selector = row["selector"].as_u64().unwrap() as u16;
                let speed = match row["speed"].as_u64().unwrap() {
                    1 => GameSpeed::Paused, 2 => GameSpeed::Turtle,
                    3 => GameSpeed::Llama, 4 => GameSpeed::Cheetah, _ => unreachable!(),
                };
                city.set_mac_state_misc_u32_at(0x1044, u32::from(selector));
                city.set_mac_state_misc_u32_at(0x1048, row["active"].as_u64().unwrap() as u32);
                city.set_mac_state_misc_u32_at(super::super::super::MISC_WEATHER_TREND_OFFSET, 7);
                let selected = match mac_status_override(speed, &city) {
                    Some(MacStatusOverride::Paused) => "pause",
                    Some(MacStatusOverride::Action(index)) if mac_action_status_label(index).is_empty() => "empty",
                    Some(MacStatusOverride::Action(_)) => "action",
                    None if row["status"].as_bool().unwrap() => "status",
                    None => "empty",
                };
                assert_eq!(selected, row["selected"].as_str().unwrap(), "{path} {row}");
                assert_eq!(u64::from(mac_status_picture(&city)), row["picture"].as_u64().unwrap(), "{path} {row}");
            }
        }
    }

    #[test]
    #[ignore = "requires the user's original Mac STRS resource"]
    fn status_panel_uses_imported_mac_strings() {
        let bytes = std::fs::read(std::env::var_os("CITYGAME_MAC_STRS").unwrap()).unwrap();
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("citygame-status-strings-{}-{nonce}", std::process::id()));
        let pack = root.join("pack");
        std::fs::create_dir_all(pack.join("catalog/mac-newspaper")).unwrap();
        std::fs::write(pack.join("catalog/mac-newspaper/strs-0.bin"), bytes).unwrap();
        let settings = root.join("settings.json");
        std::fs::write(&settings, serde_json::json!({"schema_version": 1, "original_graphics": pack}).to_string()).unwrap();
        let profile = crate::asset_profile::AssetProfile::load_from(settings).unwrap();
        let mut state = test_state(blank_city_layout_fixture("mac-native-status.city", "Status"));
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_loop_scheduler.status_message = Some(super::super::super::mac_loop_scheduler::MacStatusMessage::Story(47));
        state.configure_original_newspaper_data(&profile);
        let status = state.mac_newspaper.strings.as_ref().unwrap().status.as_ref().unwrap();
        let (paused, action, story, weather) = (&status.paused, &status.actions[12], &status.stories[1], &status.weather[0]);
        let expected = [paused, action, story, weather].map(|value| crate::city::mac_roman::decode(value));
        state.traffic.set_speed(GameSpeed::Paused);
        assert_eq!(state.status_panel_notice(), expected[0]);
        state.traffic.set_speed(GameSpeed::Turtle);
        state.city.set_mac_state_misc_u32_at(0x1044, 12);
        state.city.set_mac_state_misc_u32_at(0x1048, 1);
        assert_eq!(state.status_panel_notice(), expected[1]);
        state.city.set_mac_state_misc_u32_at(0x1048, 0);
        assert_eq!(state.status_panel_notice(), expected[2]);
        state.mac_loop_scheduler.status_message = Some(super::super::super::mac_loop_scheduler::MacStatusMessage::Weather(9));
        assert_eq!(state.status_panel_notice(), expected[3]);
        std::fs::remove_dir_all(root).unwrap();
    }
}

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
        if self.query_is_inspect {
            return self.query_line.clone();
        }
        if self.mode == AppMode::EditNewMap {
            return "Shape the terrain, then choose Done to start your city.".into();
        }
        if self.engine_fidelity == crate::engine_fidelity::EngineFidelity::Mac68k {
            use super::super::mac_loop_scheduler::MacStatusMessage;
            let strings = self.mac_newspaper.strings.as_ref().and_then(|strings| strings.status.as_ref());
            if let Some(status) = mac_status_override(self.traffic.speed(), &self.city) {
                let native = strings.and_then(|strings| match status {
                    MacStatusOverride::Paused => Some(strings.paused.as_slice()),
                    MacStatusOverride::Action(selector) => strings.actions.get(usize::from(selector)).map(Vec::as_slice),
                });
                if let Some(native) = native {
                    return crate::city::mac_roman::decode(native);
                }
                return match status {
                    MacStatusOverride::Paused => "*PAUSED*",
                    MacStatusOverride::Action(selector) => mac_action_status_label(selector),
                }.to_owned();
            }
            let native = strings.and_then(|strings| match self.mac_loop_scheduler.status_message {
                Some(MacStatusMessage::Story(selector)) => selector.checked_sub(46)
                    .and_then(|index| strings.stories.get(usize::from(index))).map(Vec::as_slice),
                Some(MacStatusMessage::Weather(trend)) => trend.checked_sub(9)
                    .and_then(|index| strings.weather.get(usize::from(index))).map(Vec::as_slice),
                None => None,
            });
            if let Some(native) = native {
                return crate::city::mac_roman::decode(native);
            }
            return match self.mac_loop_scheduler.status_message {
                Some(MacStatusMessage::Story(selector)) => super::super::newspaper_stories::pe_health_amenities_newspaper_label(selector).unwrap_or(""),
                Some(MacStatusMessage::Weather(9)) => "Blizzard conditions",
                Some(MacStatusMessage::Weather(10)) => "Hurricane conditions",
                Some(MacStatusMessage::Weather(11)) => "Tornado conditions",
                _ => "",
            }.to_owned();
        }
        if self.query_status_pinned {
            return self.query_line.clone();
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
        // Mac uses PICT 712 during an action and 700 + weather otherwise.
        let picture_id = if self.engine_fidelity == crate::engine_fidelity::EngineFidelity::Mac68k {
            mac_status_picture(&self.city)
        } else {
            700 + u16::from(super::super::weather::pe_weather_trend(&self.city))
        };
        let picture = crate::assets::NEWSPAPER_PICTURE_BASE + u32::from(picture_id);
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
