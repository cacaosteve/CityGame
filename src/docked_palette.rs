//! Movable docked Tools palette for single-window (tablet) UI.
use crate::platform_ui;
use crate::renderer::UiSolid;
use glam::Vec2;

/// Title-strip height in logical units (before pixel_scale). Slim drag handle.
const TITLE_H_LOGICAL: f32 = 12.0;

#[derive(Debug, Clone)]
pub struct DockedPalette {
    /// User-chosen top-left in backing pixels; `None` → default under menubar.
    origin: Option<Vec2>,
    dragging: bool,
    drag_grab: Vec2,
}

impl Default for DockedPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl DockedPalette {
    pub fn new() -> Self {
        Self {
            origin: None,
            dragging: false,
            drag_grab: Vec2::ZERO,
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub fn origin(&self, menubar_occupied_top: f32, pixel_scale: f32) -> Vec2 {
        self.origin.unwrap_or_else(|| {
            platform_ui::docked_palette_origin(menubar_occupied_top, pixel_scale)
        })
    }

    pub fn title_height(pixel_scale: f32) -> f32 {
        (TITLE_H_LOGICAL * pixel_scale.max(1.0)).round().max(10.0)
    }

    /// Size of the chrome including the drag title strip (backing pixels).
    pub fn chrome_size(logical_size: (u32, u32), pixel_scale: f32) -> Vec2 {
        let ps = pixel_scale.max(1.0);
        Vec2::new(
            logical_size.0 as f32 * ps,
            logical_size.1 as f32 * ps + Self::title_height(pixel_scale),
        )
    }

    /// Origin of the tool art (below the title strip).
    pub fn tools_origin(&self, menubar_occupied_top: f32, pixel_scale: f32) -> Vec2 {
        let o = self.origin(menubar_occupied_top, pixel_scale);
        Vec2::new(o.x, o.y + Self::title_height(pixel_scale))
    }

    pub fn push_title_bar(
        &self,
        menubar_occupied_top: f32,
        pixel_scale: f32,
        logical_size: (u32, u32),
        dark: bool,
        solids: &mut Vec<UiSolid>,
    ) {
        let origin = self.origin(menubar_occupied_top, pixel_scale);
        let title_h = Self::title_height(pixel_scale);
        let w = logical_size.0 as f32 * pixel_scale.max(1.0);
        solids.push(UiSolid {
            x: origin.x,
            y: origin.y,
            w,
            h: title_h,
            color: if dark {
                [0.10, 0.15, 0.18, 1.0]
            } else {
                [0.70, 0.77, 0.79, 1.0]
            },
            depth: 905.0,
        });
        // Grip lines
        let line = pixel_scale.max(1.0);
        for i in 0..3 {
            let y = origin.y + title_h * 0.35 + i as f32 * line * 2.5;
            solids.push(UiSolid {
                x: origin.x + 8.0 * line,
                y,
                w: w - 16.0 * line,
                h: line,
                color: if dark {
                    [0.42, 0.54, 0.58, 1.0]
                } else {
                    [0.36, 0.50, 0.55, 1.0]
                },
                depth: 906.0,
            });
        }
    }

    pub fn point_in_title(
        &self,
        screen: Vec2,
        menubar_occupied_top: f32,
        pixel_scale: f32,
        logical_size: (u32, u32),
    ) -> bool {
        let origin = self.origin(menubar_occupied_top, pixel_scale);
        let title_h = Self::title_height(pixel_scale);
        let w = logical_size.0 as f32 * pixel_scale.max(1.0);
        screen.x >= origin.x
            && screen.x < origin.x + w
            && screen.y >= origin.y
            && screen.y < origin.y + title_h
    }

    pub fn point_in_chrome(
        &self,
        screen: Vec2,
        menubar_occupied_top: f32,
        pixel_scale: f32,
        logical_size: (u32, u32),
    ) -> bool {
        let origin = self.origin(menubar_occupied_top, pixel_scale);
        let size = Self::chrome_size(logical_size, pixel_scale);
        screen.x >= origin.x
            && screen.x < origin.x + size.x
            && screen.y >= origin.y
            && screen.y < origin.y + size.y
    }

    pub fn begin_drag(
        &mut self,
        screen: Vec2,
        menubar_occupied_top: f32,
        pixel_scale: f32,
        logical_size: (u32, u32),
    ) -> bool {
        if !self.point_in_title(screen, menubar_occupied_top, pixel_scale, logical_size) {
            return false;
        }
        let origin = self.origin(menubar_occupied_top, pixel_scale);
        self.dragging = true;
        self.drag_grab = screen - origin;
        self.origin = Some(origin);
        true
    }

    pub fn drag_to(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        menubar_occupied_top: f32,
        pixel_scale: f32,
        logical_size: (u32, u32),
    ) {
        if !self.dragging {
            return;
        }
        let size = Self::chrome_size(logical_size, pixel_scale);
        let (safe_top, safe_right, safe_bottom, safe_left) =
            platform_ui::safe_area_insets(pixel_scale);
        let min_x = safe_left;
        let min_y = menubar_occupied_top.max(safe_top);
        let max_x = (window_size.0 as f32 - safe_right - size.x).max(min_x);
        let max_y = (window_size.1 as f32 - safe_bottom - size.y).max(min_y);
        let mut next = screen - self.drag_grab;
        next.x = next.x.clamp(min_x, max_x);
        next.y = next.y.clamp(min_y, max_y);
        self.origin = Some(next);
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_drag_moves_palette() {
        let mut dock = DockedPalette::new();
        let menubar = 22.0;
        let ps = 1.0;
        let logical = (100u32, 200u32);
        let start = dock.origin(menubar, ps) + Vec2::new(10.0, 4.0);
        assert!(dock.begin_drag(start, menubar, ps, logical));
        dock.drag_to(
            start + Vec2::new(40.0, 60.0),
            (800, 600),
            menubar,
            ps,
            logical,
        );
        let origin = dock.origin(menubar, ps);
        assert!(origin.x > 0.0);
        assert!(origin.y >= menubar);
        dock.end_drag();
        assert!(!dock.is_dragging());
    }
}
