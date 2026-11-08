//! In-game Classic Mac–style menubar for non-macOS (and future tablet) builds.
use crate::menu_model::{self, MenuCheckState, MenuEntry};
use crate::renderer::UiSolid;
use crate::ui::{pixel_text_width, push_pixel_text};
use glam::Vec2;

/// Logical menubar height before DPI scale (matches palette offset tuning).
pub const BAR_HEIGHT_LOGICAL: f32 = 22.0;

#[derive(Debug, Clone, Default)]
pub struct MenubarUi {
    open: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenubarClick {
    Consumed,
    Command(i32),
    Ignored,
}

impl MenubarUi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_enabled() -> bool {
        crate::platform_ui::uses_ingame_menubar()
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn close(&mut self) {
        self.open = None;
    }

    pub fn bar_height(pixel_scale: f32) -> f32 {
        (BAR_HEIGHT_LOGICAL * pixel_scale.max(1.0)).round()
    }

    /// Safe-area top + menubar height (backing pixels).
    pub fn occupied_top(pixel_scale: f32) -> f32 {
        let (safe_top, _, _, _) = crate::platform_ui::safe_area_insets(pixel_scale);
        safe_top + Self::bar_height(pixel_scale)
    }

    pub fn push_draw(
        &self,
        window_size: (u32, u32),
        pixel_scale: f32,
        checks: MenuCheckState,
        dark: bool,
        solids: &mut Vec<UiSolid>,
    ) {
        if !Self::is_enabled() {
            return;
        }
        let scale = pixel_scale.max(1.0);
        let text_scale = (2.0 * scale).round().max(2.0);
        let bar_h = Self::bar_height(pixel_scale);
        let (safe_top, _, _, _) = crate::platform_ui::safe_area_insets(pixel_scale);
        let bar_y = safe_top;
        let width = window_size.0 as f32;
        if safe_top > 0.0 {
            solids.push(UiSolid {
                x: 0.0,
                y: 0.0,
                w: width,
                h: safe_top,
                color: if dark {
                    [0.10, 0.10, 0.12, 1.0]
                } else {
                    [0.86, 0.86, 0.90, 1.0]
                },
                depth: 899.0,
            });
        }
        // Platinum-ish bar.
        solids.push(UiSolid {
            x: 0.0,
            y: bar_y,
            w: width,
            h: bar_h,
            color: if dark {
                [0.10, 0.15, 0.18, 1.0]
            } else {
                [0.86, 0.86, 0.90, 1.0]
            },
            depth: 900.0,
        });
        solids.push(UiSolid {
            x: 0.0,
            y: bar_y + bar_h - scale,
            w: width,
            h: scale,
            color: if dark {
                [0.25, 0.34, 0.38, 1.0]
            } else {
                [0.35, 0.35, 0.40, 1.0]
            },
            depth: 901.0,
        });

        let mut x = 8.0 * scale;
        for (index, menu) in menu_model::menus().iter().enumerate() {
            let label_w = pixel_text_width(menu.title, text_scale);
            let pad = 6.0 * scale;
            let tab_w = label_w + pad * 2.0;
            let selected = self.open == Some(index);
            if selected {
                solids.push(UiSolid {
                    x,
                    y: bar_y,
                    w: tab_w,
                    h: bar_h,
                    color: [0.20, 0.20, 0.55, 1.0],
                    depth: 902.0,
                });
            }
            let color = if selected {
                [1.0, 1.0, 1.0, 1.0]
            } else if checks.menu_enabled(menu.title) {
                if dark {
                    [0.86, 0.91, 0.92, 1.0]
                } else {
                    [0.05, 0.05, 0.08, 1.0]
                }
            } else {
                if dark {
                    [0.48, 0.56, 0.58, 1.0]
                } else {
                    [0.55, 0.55, 0.60, 1.0]
                }
            };
            let text_y = bar_y + ((bar_h - 5.0 * text_scale) * 0.5).max(scale);
            push_pixel_text(
                solids,
                menu.title,
                x + pad,
                text_y,
                text_scale,
                color,
                903.0,
            );
            if selected {
                self.push_dropdown(
                    x,
                    bar_y + bar_h,
                    menu,
                    checks,
                    text_scale,
                    scale,
                    dark,
                    solids,
                );
            }
            x += tab_w + 2.0 * scale;
        }
    }

    fn push_dropdown(
        &self,
        origin_x: f32,
        bar_h: f32,
        menu: &menu_model::MenuDef,
        checks: MenuCheckState,
        text_scale: f32,
        scale: f32,
        dark: bool,
        solids: &mut Vec<UiSolid>,
    ) {
        let (panel_w, panel_h, _) = dropdown_metrics(menu, text_scale, scale);
        let y0 = bar_h;
        solids.push(UiSolid {
            x: origin_x,
            y: y0,
            w: panel_w,
            h: panel_h,
            color: if dark {
                [0.10, 0.15, 0.18, 1.0]
            } else {
                [0.92, 0.92, 0.94, 1.0]
            },
            depth: 910.0,
        });
        // Border
        let border = if dark {
            [0.45, 0.55, 0.59, 1.0]
        } else {
            [0.15, 0.15, 0.20, 1.0]
        };
        solids.push(UiSolid {
            x: origin_x,
            y: y0,
            w: panel_w,
            h: scale,
            color: border,
            depth: 911.0,
        });
        solids.push(UiSolid {
            x: origin_x,
            y: y0 + panel_h - scale,
            w: panel_w,
            h: scale,
            color: border,
            depth: 911.0,
        });
        solids.push(UiSolid {
            x: origin_x,
            y: y0,
            w: scale,
            h: panel_h,
            color: border,
            depth: 911.0,
        });
        solids.push(UiSolid {
            x: origin_x + panel_w - scale,
            y: y0,
            w: scale,
            h: panel_h,
            color: border,
            depth: 911.0,
        });

        let row_h = row_height(text_scale, scale);
        let mut y = y0 + 2.0 * scale;
        for item in menu.items {
            match item {
                MenuEntry::Separator => {
                    solids.push(UiSolid {
                        x: origin_x + 4.0 * scale,
                        y: y + row_h * 0.5,
                        w: panel_w - 8.0 * scale,
                        h: scale,
                        color: if dark {
                            [0.32, 0.40, 0.43, 1.0]
                        } else {
                            [0.55, 0.55, 0.60, 1.0]
                        },
                        depth: 912.0,
                    });
                    y += row_h * 0.55;
                }
                MenuEntry::Command {
                    id,
                    label,
                    accel,
                    checkable,
                } => {
                    let enabled = checks.item_enabled(*id);
                    let checked = *checkable && checks.item_checked(*id);
                    let color = if enabled {
                        if dark {
                            [0.86, 0.91, 0.92, 1.0]
                        } else {
                            [0.05, 0.05, 0.08, 1.0]
                        }
                    } else {
                        if dark {
                            [0.48, 0.56, 0.58, 1.0]
                        } else {
                            [0.55, 0.55, 0.60, 1.0]
                        }
                    };
                    let text_y = y + ((row_h - 5.0 * text_scale) * 0.5).max(0.0);
                    if checked {
                        let mark = (3.0 * text_scale).max(3.0);
                        solids.push(UiSolid {
                            x: origin_x + 5.0 * scale,
                            y: text_y + text_scale,
                            w: mark,
                            h: mark,
                            color,
                            depth: 913.0,
                        });
                    }
                    push_pixel_text(
                        solids,
                        label,
                        origin_x + 14.0 * scale,
                        text_y,
                        text_scale,
                        color,
                        913.0,
                    );
                    if let Some(accel) = accel {
                        let aw = pixel_text_width(accel, text_scale);
                        push_pixel_text(
                            solids,
                            accel,
                            origin_x + panel_w - aw - 8.0 * scale,
                            text_y,
                            text_scale,
                            color,
                            913.0,
                        );
                    }
                    y += row_h;
                }
            }
        }
    }

    /// Handle a left-click in main-window pixel coordinates.
    pub fn handle_click(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
        checks: MenuCheckState,
    ) -> MenubarClick {
        if !Self::is_enabled() {
            return MenubarClick::Ignored;
        }
        let scale = pixel_scale.max(1.0);
        let text_scale = (2.0 * scale).round().max(2.0);
        let bar_h = Self::bar_height(pixel_scale);
        let (safe_top, _, _, _) = crate::platform_ui::safe_area_insets(pixel_scale);
        let bar_y = safe_top;
        let tabs = tab_layout(text_scale, scale);

        if screen.y < bar_y + bar_h {
            if screen.y < bar_y {
                self.open = None;
                return MenubarClick::Consumed;
            }
            if let Some((index, _)) = tabs
                .iter()
                .enumerate()
                .find(|(_, tab)| screen.x >= tab.0 && screen.x < tab.1)
            {
                let menu = &menu_model::menus()[index];
                if !checks.menu_enabled(menu.title) {
                    self.open = None;
                    return MenubarClick::Consumed;
                }
                self.open = if self.open == Some(index) {
                    None
                } else {
                    Some(index)
                };
                return MenubarClick::Consumed;
            }
            self.open = None;
            return MenubarClick::Consumed;
        }

        if let Some(index) = self.open {
            let menu = &menu_model::menus()[index];
            let (tab_x0, _) = tabs[index];
            let (panel_w, panel_h, rows) = dropdown_metrics(menu, text_scale, scale);
            let y0 = bar_y + bar_h;
            let in_panel = screen.x >= tab_x0
                && screen.x < tab_x0 + panel_w
                && screen.y >= y0
                && screen.y < y0 + panel_h;
            if !in_panel {
                self.open = None;
                return MenubarClick::Consumed;
            }
            let row_h = row_height(text_scale, scale);
            let mut y = y0 + 2.0 * scale;
            for (entry, _) in menu.items.iter().zip(rows.iter()) {
                match entry {
                    MenuEntry::Separator => {
                        y += row_h * 0.55;
                    }
                    MenuEntry::Command { id, .. } => {
                        if screen.y >= y && screen.y < y + row_h {
                            self.open = None;
                            if checks.item_enabled(*id) {
                                return MenubarClick::Command(*id);
                            }
                            return MenubarClick::Consumed;
                        }
                        y += row_h;
                    }
                }
            }
            return MenubarClick::Consumed;
        }

        let _ = window_size;
        MenubarClick::Ignored
    }
}

fn row_height(text_scale: f32, scale: f32) -> f32 {
    (5.0 * text_scale + 6.0 * scale).max(14.0)
}

fn tab_layout(text_scale: f32, scale: f32) -> Vec<(f32, f32)> {
    let mut x = 8.0 * scale;
    let mut tabs = Vec::new();
    for menu in menu_model::menus() {
        let label_w = pixel_text_width(menu.title, text_scale);
        let pad = 6.0 * scale;
        let tab_w = label_w + pad * 2.0;
        tabs.push((x, x + tab_w));
        x += tab_w + 2.0 * scale;
    }
    tabs
}

fn dropdown_metrics(
    menu: &menu_model::MenuDef,
    text_scale: f32,
    scale: f32,
) -> (f32, f32, Vec<()>) {
    let row_h = row_height(text_scale, scale);
    let mut max_label = 0.0f32;
    let mut max_accel = 0.0f32;
    let mut height = 4.0 * scale;
    let mut rows = Vec::new();
    for item in menu.items {
        match item {
            MenuEntry::Separator => {
                height += row_h * 0.55;
                rows.push(());
            }
            MenuEntry::Command { label, accel, .. } => {
                max_label = max_label.max(pixel_text_width(label, text_scale));
                if let Some(accel) = accel {
                    max_accel = max_accel.max(pixel_text_width(accel, text_scale));
                }
                height += row_h;
                rows.push(());
            }
        }
    }
    let width = (14.0 * scale + max_label + 16.0 * scale + max_accel + 8.0 * scale).max(120.0);
    (width, height, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_model::{CMD_LOAD_TILE_SET, CMD_SPEED_LLAMA, MenuCheckState};
    use crate::traffic::GameSpeed;

    #[test]
    fn menubar_click_opens_and_fires_command_when_enabled() {
        if !MenubarUi::is_enabled() {
            return;
        }
        let mut ui = MenubarUi::new();
        let checks = MenuCheckState::default();
        let scale = 1.0;
        assert_eq!(
            ui.handle_click(Vec2::new(20.0, 8.0), (800, 600), scale, checks),
            MenubarClick::Consumed
        );
        assert!(ui.is_open());
        let result = ui.handle_click(Vec2::new(30.0, 30.0), (800, 600), scale, checks);
        assert_eq!(result, MenubarClick::Command(CMD_LOAD_TILE_SET));
        let _ = CMD_SPEED_LLAMA;
    }

    #[test]
    fn menu_check_state_speed_and_disaster_gates() {
        let mut checks = MenuCheckState::default();
        checks.speed = GameSpeed::Cheetah;
        assert!(checks.item_checked(crate::menu_model::CMD_SPEED_CHEETAH));
        assert!(!checks.item_checked(crate::menu_model::CMD_SPEED_LLAMA));
        checks.no_disasters = true;
        assert!(!checks.item_enabled(crate::menu_model::CMD_START_FIRE));
        assert!(checks.item_enabled(crate::menu_model::CMD_TOGGLE_NO_DISASTERS));
        checks.edit_map = true;
        assert!(!checks.menu_enabled("Speed"));
    }
}
