//! Camera navigation, tile picking, and map-to-screen projection.
//!
//! Mac pointer and saved-view picking use the native integer walk. Other
//! pointer picking retains its nearest land-tile scan. Rotation retains fractional anchoring and its
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

/// PE `FUN_00444bf0` and its three terrain-aware diagonal walks. Inputs are
/// the original signed-WORD screen and scroll coordinates, not host pixels.
/// The host-to-PE viewport mapping for saveback is still being established.
fn pe_inverse_view_tile(
    mode: u8, doubled: bool, screen: (i16, i16), scroll: (i16, i16),
    mut height: impl FnMut(usize, usize) -> u8,
) -> Option<(u8, u8)> {
    let (scale, bias_x, bias_y) = match mode {
        0 => (4_i16, 8_i16, 5_i16),
        1 => (8, 12, 11),
        2 => (16, 16, 23),
        _ => return None,
    };
    let (mut px, mut py) = screen;
    if doubled { px >>= 1; py >>= 1; }
    let i = (i32::from(px) - i32::from(scroll.0) - 6) >> 1;
    let p1 = py.wrapping_sub(scroll.1).wrapping_add(i as i16)
        .wrapping_sub(3).wrapping_add(bias_x);
    let p2 = py.wrapping_sub(i as i16).wrapping_sub(scroll.1).wrapping_add(bias_y);
    inverse_view_tile_walk(p1, p2, scale, &mut height)
}

/// Mac CODE 5 export 83. Inputs use the original map window's local pixels;
/// converting a CityGame viewport to that frame remains a separate step.
fn mac_inverse_view_tile(
    mode: u8, screen: (i16, i16), origin: (i16, i16),
    mut height: impl FnMut(usize, usize) -> u8,
) -> Option<(u8, u8)> {
    let (scale, bias_x, bias_y) = match mode {
        0 => (4_i16, 8_i16, 5_i16),
        1 => (8, 12, 11),
        2 => (16, 16, 23),
        _ => return None,
    };
    let horizontal = screen.0.wrapping_sub(origin.0).wrapping_sub(6) >> 1;
    let vertical = screen.1.wrapping_sub(origin.1);
    let p1 = vertical.wrapping_add(horizontal).wrapping_sub(3).wrapping_add(bias_x);
    let p2 = vertical.wrapping_sub(horizontal).wrapping_add(bias_y);
    inverse_view_tile_walk(p1, p2, scale, &mut height)
}

fn inverse_view_tile_walk(
    p1: i16, p2: i16, scale: i16, mut height: impl FnMut(usize, usize) -> u8,
) -> Option<(u8, u8)> {
    let mut x = p1 / scale;
    let mut y = p2 / scale;
    let (mut result_x, mut result_y) = (x, y);
    let correction = scale * 3 / 4;
    let sample = |x: i16, y: i16, height: &mut dyn FnMut(usize, usize) -> u8| -> i16 {
        if (0..128).contains(&x) && (0..128).contains(&y) {
            i16::from(height(x as usize, y as usize))
        } else { 0 }
    };
    for _ in 0..128 {
        if p2 < p1 {
            x = x.wrapping_add(1);
            if x >= 128 { break; }
            let value = x.wrapping_mul(scale).wrapping_sub(sample(x, y, &mut height).wrapping_mul(correction));
            if value < p1 { (result_x, result_y) = (x, y); }
            y = y.wrapping_add(1);
            if y >= 128 { break; }
            let value = y.wrapping_mul(scale).wrapping_sub(sample(x, y, &mut height).wrapping_mul(correction));
            if value < p2 { (result_x, result_y) = (x, y); }
        } else {
            y = y.wrapping_add(1);
            if y >= 128 { break; }
            let value = y.wrapping_mul(scale).wrapping_sub(sample(x, y, &mut height).wrapping_mul(correction));
            if value < p2 { (result_x, result_y) = (x, y); }
            x = x.wrapping_add(1);
            if x >= 128 { break; }
            let value = x.wrapping_mul(scale).wrapping_sub(sample(x, y, &mut height).wrapping_mul(correction));
            if value < p1 { (result_x, result_y) = (x, y); }
        }
    }
    let packed = result_y.wrapping_shl(8).wrapping_add(result_x) as u16;
    let (x, y) = ((packed & 0xff) as u8, (packed >> 8) as u8);
    (x < 128 && y < 128).then_some((x, y))
}

/// PE rotation's center-tile anchoring at ordinary (non-doubled) display scale.
/// The inverse uses the selected visible surface; forward projection uses land
/// height, which cancels because the same ALTM word moves with the tile.
fn pe_rotate_view_scroll(
    mode: u8, clockwise: bool, center: (i16, i16), scroll: (i16, i16),
    mut height: impl FnMut(usize, usize) -> u8,
) -> Option<(i16, i16)> {
    let scale = match mode { 0 => 4_i16, 1 => 8, 2 => 16, _ => return None };
    let mut query_y = center.1;
    let mut attempts = 0_u32;
    let (x, y) = loop {
        if let Some(tile) = pe_inverse_view_tile(mode, false, (center.0, query_y), scroll, &mut height) {
            break tile;
        }
        query_y = query_y.wrapping_add(if mode == 0 { 2 } else { 4 });
        attempts += 1;
        if attempts >= 32768 { return None; }
    };
    let (x, y) = (i16::from(x), i16::from(y));
    let (nx, ny) = if clockwise { (127 - y, x) } else { (y, 127 - x) };
    let dx = x.wrapping_sub(y).wrapping_sub(nx.wrapping_sub(ny)).wrapping_mul(scale);
    let dy = x.wrapping_add(y).wrapping_sub(nx.wrapping_add(ny)).wrapping_mul(scale / 2)
        .wrapping_add(center.1.wrapping_sub(query_y));
    Some((scroll.0.wrapping_add(dx), scroll.1.wrapping_add(dy)))
}

/// Convert the host camera to the PE's integer client center and scroll.
fn pe_view_scroll(camera: Vec2, zoom: f32, pixel_scale: f32, window_size: (u32, u32), mode: u32)
    -> Option<((i16, i16), (i16, i16))>
{
    let native_zoom = pe_saved_view_zoom(mode)?;
    let display_scale = pixel_scale.max(1.0);
    let center = viewport_center(window_size) / display_scale;
    let center = (center.x as i16, center.y as i16);
    let camera = camera / display_scale * (native_zoom / zoom);
    let scroll = (
        (f32::from(center.0) + camera.x + 16.0 * native_zoom).round() as i16,
        (f32::from(center.1) + camera.y - 28.0 * native_zoom).round() as i16,
    );
    Some((center, scroll))
}

impl ViewerState {
    /// Restore the Mac CODE 4 view target and discrete zoom from MISC.
    pub(super) fn restore_saved_city_view(&mut self, window_size: (u32, u32), pixel_scale: f32) -> bool {
        if self.engine_fidelity.uses_win95() {
            return self.restore_saved_pe_view(window_size, pixel_scale);
        }
        let Some(target) = self.city.mac_state_misc_u32_at(0x1010) else { return false };
        let Some(mode) = self.city.mac_state_misc_u32_at(0x1014) else { return false };
        let Some(zoom) = pe_saved_view_zoom(u32::from(mode as u16)) else { return false };
        let packed = target as u16;
        let (x, y) = if packed == u16::MAX { (64, 64) } else { (packed & 0x7f, ((packed as i16) >> 8) as u16) };
        if x >= 128 || y >= 128 { return false; }
        self.zoom = zoom;
        self.center_tile_at_viewport(usize::from(x), usize::from(y), window_size, pixel_scale);
        true
    }

    /// Mac CODE 4 saves the native point-to-tile result at the view anchor.
    pub(super) fn saved_mac_view_for_window(&self, window_size: (u32, u32), pixel_scale: f32) -> Option<(u32, u32)> {
        if self.mode != AppMode::CityView { return None; }
        let mode = pe_saved_view_mode(self.zoom);
        let (center, origin) = pe_view_scroll(self.camera, self.zoom, pixel_scale, window_size, mode)?;
        let target = mac_inverse_view_tile(mode as u8, center, origin, |x, y| {
            let (x, y) = rotate_tile_coords(x, y, (4 - self.camera_rotation) & 3);
            let land = self.city.altitude_at(x, y);
            if self.show_underground { land } else { land.max(self.city.water_height_at(x, y)) }
        });
        let packed = target.map_or(u16::MAX, |(x,y)| {
            let (x,y) = rotate_tile_coords(usize::from(x), usize::from(y), (4 - self.camera_rotation) & 3);
            (y as u16) << 8 | x as u16
        });
        Some(((packed as i16 as i32) as u32, mode))
    }

    /// Rotate the camera 90° (delta +1 clockwise / -1 counter-clockwise). Rotates both the drawn
    /// tile positions and the per-tile sprite ids together, so roads/bridges stay connected.
    pub(super) fn rotate_view(&mut self, delta: i32) {
        let native_camera = if self.engine_fidelity.uses_win95() && self.city.kind.is_win95()
            && !self.show_underground && self.mode == AppMode::CityView
        {
            self.mac_loop_scheduler.viewport_size().and_then(|size| {
                let mode = pe_saved_view_mode(self.zoom);
                let native_zoom = pe_saved_view_zoom(mode)?;
                let display_scale = self.viewport_pixel_scale.max(1.0);
                let (center, mut scroll) = pe_view_scroll(self.camera, self.zoom, display_scale, size, mode)?;
                let mut rotation = self.camera_rotation;
                let clockwise = delta >= 0;
                for _ in 0..delta.unsigned_abs() % 4 {
                    scroll = pe_rotate_view_scroll(mode as u8, clockwise, center, scroll, |x, y| {
                        let (x, y) = rotate_tile_coords(x, y, (4 - rotation) & 3);
                        self.city.altitude_at(x, y).max(self.city.water_height_at(x, y))
                    })?;
                    rotation = (i32::from(rotation) + if clockwise { 1 } else { -1 }).rem_euclid(4) as u8;
                }
                Some(Vec2::new(
                    (f32::from(scroll.0.wrapping_sub(center.0)) - 16.0 * native_zoom) * self.zoom / native_zoom * display_scale,
                    (f32::from(scroll.1.wrapping_sub(center.1)) + 28.0 * native_zoom) * self.zoom / native_zoom * display_scale,
                ))
            })
        } else { None };
        // Other views preserve the exact projected viewport-center point,
        // including its fractional position within the tile.
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
        self.camera = native_camera.unwrap_or_else(|| -projected_tile_units_f32(x, y, f32::from(elevation)) * scale);
        let next_compass = (self.city.rotation() as i32 + delta).rem_euclid(4) as u8;
        let next_camera = next_compass;
        self.city.set_view_rotation(next_compass);
        if self.engine_fidelity.uses_win95() {
            self.city.pe_w_city_rotation =
                (i32::from(self.city.pe_w_city_rotation) - delta).rem_euclid(4) as u16;
        }
        self.camera_rotation = next_camera;
        self.reset_traffic_redraw_phase();
        self.invalidate_sprite_cache();
        self.refresh_status_line();
        log::info!("view rotation compass={next_compass} camera={next_camera}");
    }

    pub(super) fn view_camera(&self) -> Vec2 {
        let shake = if self.engine_fidelity.uses_win95() {
            self.earthquake_shake_offset()
        } else {
            self.mac_earthquake_shake_offset()
        };
        self.camera + shake
    }

    fn mac_earthquake_shake_offset(&self) -> Vec2 {
        // CODE 10 FUN_0003c1ea copies the same source Rect on odd frames and
        // shifts its even-frame destination right by (4 << MISC zoom byte).
        let Some(phase) = self.mac_loop_scheduler.mac_earthquake_shake_phase() else {
            return Vec2::ZERO;
        };
        let shift = (4_u32 << pe_saved_view_mode(self.zoom)) as f32
            * self.viewport_pixel_scale.max(1.0);
        Vec2::new(if phase & 1 == 0 { shift } else { 0.0 }, 0.0)
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
        if !self.engine_fidelity.uses_win95() {
            let mode = pe_saved_view_mode(self.zoom);
            let native_zoom = pe_saved_view_zoom(mode)?;
            let display_scale = pixel_scale.max(1.0);
            let (center, origin) = pe_view_scroll(self.view_camera(), self.zoom, pixel_scale, window_size, mode)?;
            let center_vec = Vec2::new(f32::from(center.0), f32::from(center.1));
            let point = center_vec + (screen / display_scale - center_vec) * (native_zoom / self.zoom);
            let point = (point.x.round() as i16, point.y.round() as i16);
            let (x, y) = mac_inverse_view_tile(mode as u8, point, origin, |x, y| {
                let (x, y) = rotate_tile_coords(x, y, (4 - self.camera_rotation) & 3);
                let land = self.city.altitude_at(x, y);
                if self.show_underground { land } else { land.max(self.city.water_height_at(x, y)) }
            })?;
            return Some(rotate_tile_coords(usize::from(x), usize::from(y), (4 - self.camera_rotation) & 3));
        }
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

    /// Persist the Win95 view target used by the save serializer.
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
        let mode = pe_saved_view_mode(self.zoom);
        if self.engine_fidelity.uses_win95() && self.city.kind.is_win95() {
            // The PE saves by passing the client X center for *both* axes.
            // Convert CityGame's projected-map origin to the PE's discrete
            // view scale, retaining its extra anchor offsets.
            let (center, scroll) = pe_view_scroll(self.camera, self.zoom, pixel_scale, window_size, mode)?;
            let center_x = center.0;
            let target = pe_inverse_view_tile(mode as u8, false, (center_x, center_x), scroll, |x, y| {
                let (x, y) = rotate_tile_coords(x, y, (4 - self.camera_rotation) & 3);
                self.city.altitude_at(x, y).max(self.city.water_height_at(x, y))
            });
            let (x, y) = target.map_or((64, 128), |(x, y)| (u16::from(x), u16::from(y)));
            return Some(crate::city::CityView { mode, x, y });
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
            mode,
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

#[cfg(test)]
mod pe_inverse_tests {
    use super::{mac_inverse_view_tile, pe_inverse_view_tile, pe_rotate_view_scroll, pe_view_scroll};

    #[test]
    fn mac_picker_matches_original_instructions_on_synthetic_surfaces() {
        let points = [(312,212),(310,210),(320,220),(150,70),(500,390),(0,0),
            (639,479),(220,34),(100,400),(600,30),(245,75),(300,300)];
        // Original Mac 1.2 CODE 5 with origin (220,34), synthetic ALTM and
        // both surface and underground height selection; 0xffff is no tile.
        let expected = [
            [0x394f,0x384d,0x3a52,0x2714,0x3b7f,0xffff,0xffff,0x1916,0x7f60,0xffff,0x2024,0x4c60],
            [0x384d,0x384d,0x374e,0x2714,0x3a7f,0xffff,0xffff,0x1916,0x7f60,0xffff,0x1e23,0x4c60],
            [0x2832,0x2832,0x2734,0x2016,0x3355,0x2004,0x3366,0x1413,0x4a39,0xffff,0x1b1d,0x333d],
            [0x2832,0x2832,0x2734,0x2015,0x3355,0x1e02,0x3064,0x1413,0x4a39,0xffff,0x1b1d,0x303a],
            [0x1e23,0x1e23,0x1e23,0x1b15,0x2636,0x190b,0x263f,0x1413,0x3026,0x091f,0x1718,0x2328],
            [0x1e23,0x1e23,0x1e23,0x1a14,0x2132,0x190b,0x223c,0x1413,0x2f26,0x071e,0x1718,0x2227],
        ];
        for mode in 0..3 {
            for underground in 0..2 {
                for (i, point) in points.into_iter().enumerate() {
                    let got = mac_inverse_view_tile(mode, point, (220,34), |x,y| {
                        let land = ((3*x+5*y)%32) as u8;
                        let water = ((7*x+11*y+3)%32) as u8;
                        if underground == 0 { land.max(water) } else { land }
                    }).map_or(0xffff, |(x,y)| u16::from(x) | (u16::from(y)<<8));
                    assert_eq!(got, expected[usize::from(mode)*2+underground][i], "mode {mode}, underground {underground}, point {point:?}");
                }
            }
        }
    }

    #[test]
    fn pe_view_scroll_uses_integer_client_center_for_odd_windows() {
        let camera = glam::Vec2::new(12.25, -17.25);
        let even = pe_view_scroll(camera, 0.5, 1.0, (800, 600), 1).unwrap();
        let odd = pe_view_scroll(camera, 0.5, 1.0, (801, 601), 1).unwrap();
        assert_eq!(even, odd);
        assert_eq!(even.0, (400, 300));
    }

    #[test]
    #[ignore = "requires private original-PE rotation captures"]
    fn pe_rotation_scroll_matches_original_maxis_turn_and_return() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_PE_CAMERA_LIVE").unwrap());
        for (before, after, clockwise) in [("mx0", "mx1", true), ("mx1", "mx2", false)] {
            let input: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(format!("{before}.json"))).unwrap()).unwrap();
            let expected: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(format!("{after}.json"))).unwrap()).unwrap();
            let altm = std::fs::read(root.join(format!("{before}.altm"))).unwrap();
            let value = |key: &str| input[key].as_i64().unwrap() as i16;
            let actual = pe_rotate_view_scroll(value("view_mode") as u8, clockwise,
                (value("center_x"), value("center_y")), (value("scroll_x"), value("scroll_y")),
                |x, y| {
                    let i = (x * 128 + y) * 2;
                    let word = u16::from_le_bytes([altm[i], altm[i + 1]]);
                    (word as u8 & 31).max(((word >> 5) & 31) as u8)
                });
            assert_eq!(actual, Some((expected["scroll_x"].as_i64().unwrap() as i16,
                expected["scroll_y"].as_i64().unwrap() as i16)), "{before} -> {after}");
        }
    }

    #[test]
    #[ignore = "requires private original-PE rotation instruction fixture"]
    fn pe_rotation_scroll_matches_original_instructions() {
        let path = std::env::var_os("CITYGAME_PE_ROTATION_SCROLL_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 90);
        for row in rows {
            let pair = |key: &str| (row[key][0].as_i64().unwrap() as i16, row[key][1].as_i64().unwrap() as i16);
            let actual = pe_rotate_view_scroll(row["mode"].as_u64().unwrap() as u8,
                row["clockwise"].as_bool().unwrap(), pair("center"), pair("scroll"),
                |x, y| (((x / 9 + y / 11) % 12) as u8).max(((x / 7 + y / 5) % 20) as u8));
            assert_eq!(actual, Some(pair("expected")), "{row}");
        }
    }

    #[test]
    #[ignore = "requires private original-PE off-map rotation fixture"]
    fn pe_rotation_scroll_matches_original_off_map_retries() {
        let path = std::env::var_os("CITYGAME_PE_ROTATION_OFFMAP_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(rows.len(), 18);
        for row in rows {
            let pair = |key: &str| (row[key][0].as_i64().unwrap() as i16, row[key][1].as_i64().unwrap() as i16);
            assert!(row["inverse_calls"].as_u64().unwrap() > 1);
            let actual = pe_rotate_view_scroll(row["mode"].as_u64().unwrap() as u8,
                row["clockwise"].as_bool().unwrap(), pair("center"), pair("scroll"),
                |x, y| (((x / 9 + y / 11) % 12) as u8).max(((x / 7 + y / 5) % 20) as u8));
            assert_eq!(actual, Some(pair("expected")), "{row}");
        }
    }

    #[test]
    fn pe_inverse_matches_sampled_original_view_cases() {
        // Values from the original PE wrapper, with a flat surface and a
        // 640x480 example view. The save path passes center X for both axes.
        for (mode, y, expected) in [
            (0, 240, 16960_u16), (0, 320, 22100),
            (1, 240, 8480), (1, 320, 11050),
            (2, 240, 4368), (2, 320, 5653),
        ] {
            let result = pe_inverse_view_tile(mode, false, (320, y), (320, -16), |_, _| 0);
            assert_eq!(result.map(|(x, y)| u16::from(x) | (u16::from(y) << 8)), Some(expected));
        }
        // Preserve the original signed-WORD packing behavior at extreme input.
        assert_eq!(pe_inverse_view_tile(0, false, (320, 240), (320, -784), |_, _| 0), Some((0, 3)));
    }

    #[test]
    #[ignore = "requires private original-PE camera fixture"]
    fn pe_inverse_matches_original_instruction_fixtures() {
        let path = std::env::var_os("CITYGAME_PE_CAMERA_FIXTURE").unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for row in &rows {
            let integer = |key: &str| row[key].as_i64().unwrap() as i16;
            let result = pe_inverse_view_tile(
                integer("mode") as u8, integer("doubled") != 0,
                (integer("px"), integer("py")), (integer("scroll_x"), integer("scroll_y")),
                |x, y| match row["terrain"].as_str().unwrap() {
                    "flat" => 0,
                    "land" => ((x / 9 + y / 11) % 12) as u8,
                    "water" => 3_u8.max(((x / 7 + y / 5) % 20) as u8),
                    _ => unreachable!(),
                },
            );
            let expected = row["result"].as_u64().unwrap() as u16;
            let packed = result.map_or(0xffff, |(x, y)| u16::from(x) | (u16::from(y) << 8));
            assert_eq!(packed, expected, "{row}");
        }
        assert_eq!(rows.len(), 798);
    }
}
