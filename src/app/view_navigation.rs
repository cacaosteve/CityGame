//! Camera navigation, tile picking, and map-to-screen projection.
//!
//! Pointer picking retains its nearest land-tile scan; saved-view picking can
//! include the water surface. Rotation retains fractional anchoring and its
//! existing redraw/cache/status tail. Sprite construction and simulation stay
//! with their existing owners.

use super::ViewerState;
use crate::city::{AppMode, CITY_HEIGHT, CITY_WIDTH};
use glam::Vec2;
use sdl3::mouse::MouseWheelDirection;

pub(super) const MIN_ZOOM: f32 = 0.35;
pub(super) const MAX_ZOOM: f32 = 5.0;
pub(super) const TILE_WIDTH_HALF: f32 = 16.0;
pub(super) const TILE_HEIGHT_HALF: f32 = 16.0;
pub(super) const ALTITUDE_LAYER_STEP: f32 = 12.0;

impl ViewerState {
    /// Rotate the camera 90° (delta +1 clockwise / -1 counter-clockwise). Rotates both the drawn
    /// tile positions and the per-tile sprite ids together, so roads/bridges stay connected.
    pub(super) fn rotate_view(&mut self, delta: i32) {
        // The viewport origin is its center. Preserve that exact projected point,
        // including its fractional position within the tile, rather than rotating
        // around the city center or snapping to a nearby tile's anchor.
        let scale = render_zoom(self.zoom, self.viewport_pixel_scale);
        let center = -self.camera / scale;
        let elevation = self
            .tile_at_map_units(center, !self.show_underground)
            .map(|(x, y)| {
                let land = self.city.altitude_at(x, y);
                if self.show_underground {
                    land
                } else {
                    land.max(self.city.water_height_at(x, y))
                }
            })
            .unwrap_or(0);
        let sum =
            (center.y + (f32::from(elevation) + 1.0) * ALTITUDE_LAYER_STEP + TILE_HEIGHT_HALF)
                / (TILE_HEIGHT_HALF * 0.5);
        let difference = (center.x - TILE_WIDTH_HALF) / TILE_WIDTH_HALF;
        let x = (sum + difference) * 0.5;
        let y = (sum - difference) * 0.5;
        let max = (CITY_WIDTH - 1) as f32;
        let (x, y) = match delta.rem_euclid(4) {
            0 => (x, y),
            1 => (max - y, x),
            2 => (max - x, max - y),
            _ => (y, max - x),
        };
        self.camera = -projected_tile_units_f32(x, y, f32::from(elevation)) * scale;
        let next_compass = (self.city.rotation() as i32 + delta).rem_euclid(4) as u8;
        let next_camera = next_compass;
        self.city.set_view_rotation(next_compass);
        self.camera_rotation = next_camera;
        self.reset_traffic_redraw_phase();
        self.invalidate_sprite_cache();
        self.refresh_status_line();
        log::info!("view rotation compass={next_compass} camera={next_camera}");
    }

    pub(super) fn view_camera(&self) -> Vec2 {
        self.camera + self.earthquake_shake_offset()
    }

    pub(super) fn update_hover_tile(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        self.hover_tile = self.tile_at_screen(screen, window_size, pixel_scale);
    }

    pub(super) fn structure_anchor(
        &self,
        window_size: (u32, u32),
        pixel_scale: f32,
        x: usize,
        y: usize,
    ) -> (Vec2, f32, usize, usize) {
        let origin = viewport_origin(window_size) + self.view_camera();
        let zoom = render_zoom(self.zoom, pixel_scale);
        let (sx, sy) = rotate_tile_coords(x, y, self.camera_rotation);
        let elevation = self.city.structure_elevation_at(x, y) as f32;
        let anchor = origin + projected_tile_units(sx, sy, elevation) * zoom;
        (anchor, zoom, sx, sy)
    }

    pub(super) fn zoom_in_at(&mut self, anchor: Vec2, window_size: (u32, u32), pixel_scale: f32) {
        self.zoom_by_at(1.25, anchor, window_size, pixel_scale);
    }

    pub(super) fn zoom_out_at(&mut self, anchor: Vec2, window_size: (u32, u32), pixel_scale: f32) {
        self.zoom_by_at(1.0 / 1.25, anchor, window_size, pixel_scale);
    }

    pub(super) fn zoom_by_at(
        &mut self,
        factor: f32,
        anchor: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        self.set_zoom_at(self.zoom * factor, anchor, window_size, pixel_scale);
    }

    pub(super) fn actual_size_at(
        &mut self,
        anchor: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        self.set_zoom_at(2.0, anchor, window_size, pixel_scale);
    }

    fn set_zoom_at(&mut self, zoom: f32, anchor: Vec2, window_size: (u32, u32), pixel_scale: f32) {
        let new_zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if (new_zoom - self.zoom).abs() < f32::EPSILON {
            return;
        }
        let map_units = self.screen_to_map_units(anchor, window_size, pixel_scale);
        self.zoom = new_zoom;
        self.place_map_units_at_screen(map_units, anchor, window_size, pixel_scale);
    }

    pub(super) fn center_map(&mut self, window_size: (u32, u32), pixel_scale: f32) {
        let map_center = self.projected_map_center();
        self.place_map_units_at_screen(
            map_center,
            viewport_center(window_size),
            window_size,
            pixel_scale,
        );
    }

    pub(super) fn fit_map(&mut self, window_size: (u32, u32), pixel_scale: f32) {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let (sx, sy) = rotate_tile_coords(x, y, self.camera_rotation);
                let point = projected_tile_units(sx, sy, self.city.altitude_at(x, y) as f32);
                min = min.min(point);
                max = max.max(point);
            }
        }

        let map_size = max - min;
        let (width, height) = window_size;
        let usable = Vec2::new(width as f32 * 0.9, height as f32 * 0.9);
        let zoom_x = usable.x / map_size.x.max(1.0);
        let zoom_y = usable.y / map_size.y.max(1.0);
        self.zoom = zoom_x.min(zoom_y).clamp(MIN_ZOOM, MAX_ZOOM);
        self.center_map(window_size, pixel_scale);
    }

    pub(super) fn tile_at_screen(
        &self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> Option<(usize, usize)> {
        self.tile_at_screen_with_surface_height(screen, window_size, pixel_scale, false)
    }

    fn tile_at_screen_with_surface_height(
        &self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
        include_water_surface: bool,
    ) -> Option<(usize, usize)> {
        let map_point = self.screen_to_map_units(screen, window_size, pixel_scale);
        self.tile_at_map_units(map_point, include_water_surface)
    }

    fn tile_at_map_units(
        &self,
        map_point: Vec2,
        include_water_surface: bool,
    ) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, f32)> = None;
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let (sx, sy) = rotate_tile_coords(x, y, self.camera_rotation);
                let land = self.city.altitude_at(x, y);
                let elevation = if include_water_surface {
                    land.max(self.city.water_height_at(x, y))
                } else {
                    land
                };
                let tile_point = projected_tile_units(sx, sy, elevation as f32);
                let distance = (tile_point - map_point).length_squared();
                if best.is_none_or(|(_, _, best_distance)| distance < best_distance) {
                    best = Some((x, y, distance));
                }
            }
        }
        best.map(|(x, y, _)| (x, y))
    }

    pub(super) fn screen_to_map_units(
        &self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> Vec2 {
        screen_to_map_units(
            screen,
            window_size,
            self.view_camera(),
            self.zoom,
            pixel_scale,
        )
    }

    pub(super) fn place_map_units_at_screen(
        &mut self,
        map_units: Vec2,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        self.viewport_pixel_scale = pixel_scale;
        self.camera =
            camera_for_map_units_at_screen(map_units, screen, window_size, self.zoom, pixel_scale);
    }

    pub(super) fn center_tile_at_viewport(
        &mut self,
        x: usize,
        y: usize,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        self.place_map_units_at_screen(
            self.projected_tile_units_for_view(x, y),
            viewport_center(window_size),
            window_size,
            pixel_scale,
        );
    }

    pub(super) fn restore_saved_pe_view(
        &mut self,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> bool {
        let Some(view) = self.city.saved_view() else {
            return false;
        };
        let Some(zoom) = pe_saved_view_zoom(view.mode) else {
            return false;
        };

        self.zoom = zoom;
        self.center_tile_at_viewport(
            usize::from(view.x),
            usize::from(view.y),
            window_size,
            pixel_scale,
        );
        true
    }

    /// Persist the tile nearest the viewport center, matching the PE save's center-target intent.
    pub(super) fn persist_saved_pe_view(
        &mut self,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> bool {
        self.saved_pe_view_for_window(window_size, pixel_scale)
            .is_some_and(|view| self.city.set_saved_view(view))
    }

    pub(super) fn saved_pe_view_for_window(
        &self,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> Option<crate::city::CityView> {
        if self.mode != AppMode::CityView {
            return None;
        }
        // PE's saved-camera inverse (`FUN_00445ad0`, reached from `FUN_00444bf0`) resolves
        // each tile to max(ALTM.land, ALTM.water) when the center-view path clears its
        // terrain flag. The ordinary pointer picker retains its existing land-only behavior.
        let (x, y) = self.tile_at_screen_with_surface_height(
            viewport_center(window_size),
            window_size,
            pixel_scale,
            true,
        )?;
        Some(crate::city::CityView {
            mode: pe_saved_view_mode(self.zoom),
            x: x as u16,
            y: y as u16,
        })
    }

    fn projected_tile_units_for_view(&self, x: usize, y: usize) -> Vec2 {
        let x = x.min(CITY_WIDTH - 1);
        let y = y.min(CITY_HEIGHT - 1);
        let (sx, sy) = rotate_tile_coords(x, y, self.camera_rotation);
        projected_tile_units(sx, sy, self.city.altitude_at(x, y) as f32)
    }

    fn projected_map_center(&self) -> Vec2 {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);

        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let (sx, sy) = rotate_tile_coords(x, y, self.camera_rotation);
                let point = projected_tile_units(sx, sy, self.city.altitude_at(x, y) as f32);
                min = min.min(point);
                max = max.max(point);
            }
        }

        (min + max) * 0.5
    }
}

/// Convert the Win95 MISC view selector (`obj+0x140`) to CityGame's logical zoom.
/// The PE's 4/8/16-pixel tile projection corresponds to CityGame zoom 0.25/0.5/1.
fn pe_saved_view_zoom(mode: u32) -> Option<f32> {
    match mode {
        0..=2 => Some(2.0_f32.powi(mode as i32 - 2)),
        _ => None,
    }
}

/// Pick the nearest of the three Win95 saved-view zoom levels for CityGame's continuous zoom.
fn pe_saved_view_mode(zoom: f32) -> u32 {
    (zoom.max(MIN_ZOOM).log2() + 2.0).round().clamp(0.0, 2.0) as u32
}

pub(super) fn viewport_origin(window_size: (u32, u32)) -> Vec2 {
    viewport_center(window_size)
}

pub(super) fn viewport_center(window_size: (u32, u32)) -> Vec2 {
    let (width, height) = window_size;
    Vec2::new(width as f32 * 0.5, height as f32 * 0.5)
}

pub(super) fn projected_tile_units(x: usize, y: usize, altitude: f32) -> Vec2 {
    projected_tile_units_f32(x as f32, y as f32, altitude)
}

/// Map a data-grid tile to its on-screen grid position for the active view rotation.
/// Tile data is stored absolute (north-up); this rotates only where a tile is *drawn*.
pub(super) fn rotate_tile_coords(x: usize, y: usize, rotation: u8) -> (usize, usize) {
    const MAX: usize = CITY_WIDTH - 1;
    match rotation & 0x03 {
        0 => (x, y),
        1 => (MAX - y, x),
        2 => (MAX - x, MAX - y),
        _ => (y, MAX - x),
    }
}

pub(super) fn projected_tile_units_f32(x: f32, y: f32, altitude: f32) -> Vec2 {
    Vec2::new(
        (x - y) * TILE_WIDTH_HALF + TILE_WIDTH_HALF,
        (x + y) * (TILE_HEIGHT_HALF * 0.5)
            - (altitude + 1.0) * ALTITUDE_LAYER_STEP
            - TILE_HEIGHT_HALF,
    )
}

pub(super) fn screen_to_map_units(
    screen: Vec2,
    window_size: (u32, u32),
    camera: Vec2,
    zoom: f32,
    pixel_scale: f32,
) -> Vec2 {
    (screen - viewport_origin(window_size) - camera) / render_zoom(zoom, pixel_scale)
}

pub(super) fn camera_for_map_units_at_screen(
    map_units: Vec2,
    screen: Vec2,
    window_size: (u32, u32),
    zoom: f32,
    pixel_scale: f32,
) -> Vec2 {
    screen - viewport_origin(window_size) - map_units * render_zoom(zoom, pixel_scale)
}

pub(super) fn render_zoom(zoom: f32, pixel_scale: f32) -> f32 {
    zoom * pixel_scale.max(1.0)
}

pub(super) fn wheel_zoom_factor(y: f32, direction: MouseWheelDirection) -> f32 {
    let y = match direction {
        MouseWheelDirection::Flipped => -y,
        MouseWheelDirection::Normal | MouseWheelDirection::Unknown(_) => y,
    };
    1.25_f32.powf(y)
}

pub(super) fn layer_height(zoom: f32) -> f32 {
    ALTITUDE_LAYER_STEP * zoom
}
