//! Mac map redraw calls that share QuickDraw's process random stream.
//!
//! CODE 2's map redraw scans projected map cells by diagonals. CODE 3 then
//! dispatches special XTXT markers to CODE 10 export 248, whose sprite choice
//! and flip consume QuickDraw random values. These calls matter even when a
//! resulting sprite is clipped by QuickDraw.

use super::*;

/// CODE 12's underground map walk services the palette at the end of each
/// diagonal as well. Underground tile drawing is still handled elsewhere.
pub(super) fn redraw_underground_palette(palette: &mut impl FnMut(usize)) {
    for diagonal in 0..(CITY_WIDTH + CITY_HEIGHT - 1) {
        palette(diagonal);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MacMapRedrawView {
    width: i32,
    height: i32,
    origin_x: i32,
    origin_y: i32,
    zoom: u32,
    rotation: u8,
}

/// Window-local QuickDraw rectangle supplied to the surface map walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MacMapClip {
    top: i32,
    left: i32,
    bottom: i32,
    right: i32,
}

#[derive(Clone, Copy)]
struct MacActorTileBounds {
    min_diff: i32,
    max_diff: i32,
    min_sum: i32,
    max_sum: i32,
}

// CODE 6 export 110 and export 111 use the same kind-indexed zoom gate.
const MAC_ACTOR_MIN_ZOOM: [u32; 18] = [0, 0, 2, 0, 0, 0, 1, 0, 0, 2, 2, 2, 2, 3, 0, 0, 2, 0];

/// Simulation effects produced while walking a surface redraw. Applied after
/// the walk because each map coordinate is visited once and palette callbacks
/// do not read these fields.
#[derive(Default)]
pub(super) struct MacMapRedrawEffects {
    kind6_draws: usize,
    stale_proxies: Vec<(usize, usize)>,
}

impl MacMapRedrawEffects {
    pub(super) fn apply(self, city: &mut City, lfsr: &mut MacAppLfsrRng) {
        for _ in 0..self.kind6_draws { lfsr.bounded(2); }
        for (x, y) in self.stale_proxies { city.set_text_overlay(x, y, 0); }
    }
}

// The small-zoom shape table stores these building widths in four-pixel
// units. CODE 12 uses them for the left overlap of export 175's tile walk.
fn mac_small_building_width_units(building: u8) -> i32 {
    match building {
        0 => 0,
        0x01..=0x60 | 0x6c..=0x8b | 0xc6..=0xc8 | 0xdb..=0xea => 2,
        0x61..=0x6b | 0x8c..=0xad | 0xeb..=0xf7 => 4,
        0xae..=0xc5 | 0xd0..=0xd6 | 0xf8..=0xfa => 6,
        _ => 8,
    }
}

impl MacMapClip {
    #[cfg(test)]
    pub(super) fn from_native_snapshot(top: i32, left: i32, bottom: i32, right: i32) -> Self {
        Self { top, left, bottom, right }
    }

    pub(super) fn union(self, other: Self) -> Self {
        // QuickDraw UnionRect ignores an empty input rectangle.
        if self.top >= self.bottom || self.left >= self.right { return other; }
        if other.top >= other.bottom || other.left >= other.right { return self; }
        Self {
            top: self.top.min(other.top),
            left: self.left.min(other.left),
            bottom: self.bottom.max(other.bottom),
            right: self.right.max(other.right),
        }
    }
}

impl MacMapRedrawView {
    #[cfg(test)]
    pub(super) fn from_native_snapshot(width: i32, height: i32, origin_x: i32, origin_y: i32, zoom: u32, rotation: u8) -> Self {
        Self { width, height, origin_x, origin_y, zoom, rotation }
    }

    /// Surface part of CODE 12 export 178. The caller resolves shape
    /// dimensions from the selected original sprite table and separately
    /// handles any actor bounds associated with an XTXT marker.
    pub(super) fn surface_tile_dirty_bounds(
        self, city: &City, x: usize, y: usize, terrain_height: i32,
        building_shape: Option<(i32, i32)>,
    ) -> Option<MacMapClip> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT || self.zoom > 2 { return None; }
        let (xv, yv) = super::super::view_navigation::rotate_tile_coords(x, y, self.rotation);
        let unit = 4 << self.zoom;
        let left = self.origin_x + unit * (xv as i32 - yv as i32);
        let base = self.origin_y + (unit / 2) * (xv + yv) as i32;
        let altitude = city.altm_at(x, y);
        let height = if city.terrain_at(x, y) < 16 { altitude.land } else { altitude.water };
        let mut bottom = base - 3 * (1 << self.zoom) * i32::from(height);
        let mut top = bottom - terrain_height;
        let mut right = left + unit * 2;
        let building = city.building_at(x, y);
        if building != 0 {
            let (building_height, building_width) = building_shape?;
            let mut sprite_y = bottom;
            if (0x51..0x5d).contains(&building) || matches!(building, 0x6a | 0x6b) {
                sprite_y += (i32::from(altitude.land) - i32::from(altitude.water)) * 3 * (1 << self.zoom);
            }
            if building >= 0x61 {
                let mask = [0x80, 0x10, 0x20, 0x40][usize::from(self.rotation & 3)];
                if city.zone_at(x, y).is_some_and(|zone| zone.raw_bits() & mask != 0) {
                    let sprite_bottom = sprite_y - unit / 2 + building_width / 4;
                    top = top.min(sprite_bottom - building_height);
                    bottom = bottom.max(sprite_bottom);
                    right = right.max(left + building_width);
                }
            } else {
                top = top.min(sprite_y - building_height);
            }
        }
        let margin = 8 << self.zoom;
        if bottom < 0 || top > self.height || right < 0 || left > self.width
            || self.width - left < margin || right < margin
        { return None; }
        Some(MacMapClip { top, left, bottom, right })
    }

    /// Resolve shape dimensions from the same imported Mac pack as the map.
    /// A missing mapping or sprite leaves the bounds unknown to the caller.
    pub(super) fn surface_tile_dirty_bounds_from_assets(
        self, city: &City, x: usize, y: usize,
        shapes: &crate::mac_shape_table::MacShapeDimensions,
    ) -> Option<MacMapClip> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT { return None; }
        let (_, terrain_height) = shapes.terrain_dimensions(city.terrain_at(x, y), self.zoom)?;
        let building = city.building_at(x, y);
        let building_shape = if building == 0 { None } else {
            let (width, height) = shapes.building_dimensions(building, self.zoom)?;
            Some((height as i32, width as i32))
        };
        self.surface_tile_dirty_bounds(city, x, y, terrain_height as i32, building_shape)
    }

    /// Include a known actor proxy after the surface tile passes clipping,
    /// as CODE 12 export 178 does before it returns to the terrain walker.
    pub(super) fn tile_dirty_bounds_from_assets(
        self, city: &City, x: usize, y: usize,
        shapes: &crate::mac_shape_table::MacShapeDimensions,
    ) -> Option<MacMapClip> {
        let surface = self.surface_tile_dirty_bounds_from_assets(city, x, y, shapes)?;
        let actor = city.text_overlay_at(x, y).and_then(|marker| {
            marker.checked_sub(0xc9).filter(|&slot| slot < 40).map(usize::from)
        }).and_then(|slot| city.traffic_overlays()?.get(slot))
            .and_then(|actor| self.actor_dirty_bounds(city, actor));
        Some(actor.map_or(surface, |bounds| surface.union(bounds)))
    }

    /// CODE 5 export 80's generic explosion frame, as accumulated by its
    /// local `0x7e34` helper before export 174 flushes the rectangle.
    pub(super) fn destruction_frame_bounds(
        self, x: usize, y: usize, height: u8, frame: usize,
        column: usize, row: usize, variant: u8,
    ) -> MacMapClip {
        // Shape-table heights at indices 0x188 + zoom*500 + variant.
        const HEIGHTS: [[i32; 4]; 3] = [
            [17, 13, 11, 8], [31, 27, 21, 17], [64, 49, 39, 32],
        ];
        let (x, y) = super::super::view_navigation::rotate_tile_coords(x, y, self.rotation);
        let unit = 4 << self.zoom;
        let left = self.origin_x + unit * (x as i32 - y as i32)
            + unit * (column + row) as i32;
        let bottom = self.origin_y + unit / 2 * (x + y) as i32
            - 3 * (1 << self.zoom) * i32::from(height)
            + unit / 2 * (column as i32 - row as i32 - frame as i32);
        MacMapClip {
            top: bottom - HEIGHTS[self.zoom as usize][usize::from(variant & 3)],
            left,
            bottom,
            right: left + (8 << self.zoom),
        }
    }

    /// CODE 6 export 111's screen rectangle for a live actor. Its kind tables
    /// define the minimum zoom and horizontal, upper and lower extents.
    pub(super) fn actor_dirty_bounds(
        self, city: &City, actor: &crate::win95_overlay::Win95OverlayRecord,
    ) -> Option<MacMapClip> {
        const ABOVE: [i32; 18] = [0, 10, 6, 6, 4, 15, 2, 6, 6, 2, 4, 4, 0, 0, 6, 9, 12, 0];
        const BELOW: [i32; 18] = [0, 0, 0, 1, 4, 0, 0, 2, 2, 2, 4, 4, 0, 0, 2, 0, 2, 0];
        const HALF_WIDTH: [i32; 18] = [0, 2, 2, 5, 4, 8, 2, 2, 2, 2, 6, 6, 0, 0, 2, 5, 2, 0];
        let kind = usize::from(actor.kind);
        if kind == 0 || self.zoom > 2 || self.zoom < *MAC_ACTOR_MIN_ZOOM.get(kind)? { return None; }
        let (x, y) = (usize::from(actor.tile_x), usize::from(actor.tile_y));
        if x >= CITY_WIDTH || y >= CITY_HEIGHT { return None; }
        let (xv, yv) = super::super::view_navigation::rotate_tile_coords(x, y, self.rotation);
        let unit = 4 << self.zoom;
        let px = self.origin_x + unit * (xv as i32 - yv as i32);
        let water = city.tile_bits_at(x, y).is_some_and(|bits| bits.raw_bits() & 4 != 0);
        let height = if water { city.water_height_at(x, y) } else { city.altitude_at(x, y) };
        let py = self.origin_y + unit / 2 * (xv + yv) as i32
            - 3 * (1 << self.zoom) * i32::from(height);
        let above = match kind {
            5 => (i32::from(actor.height) + 6) * unit,
            6 => i32::from(actor.height) * unit,
            _ => ABOVE[kind] * unit,
        };
        let half_width = HALF_WIDTH[kind] * unit;
        let bounds = MacMapClip {
            top: py - above,
            left: px + unit - half_width,
            bottom: py + BELOW[kind] * unit,
            right: px + unit + half_width,
        };
        let margin = 8 << self.zoom;
        if bounds.bottom < 0 || bounds.top > self.height
            || bounds.right < 0 || bounds.left > self.width
            || self.width - bounds.left < margin || bounds.right < margin
        {
            None
        } else {
            Some(bounds)
        }
    }

    /// Kind-15 bounds before an Auto-Goto changes the camera.
    pub(super) fn tornado_dirty_bounds(self, city: &City, x: usize, y: usize) -> Option<MacMapClip> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT { return None; }
        self.actor_dirty_bounds(city, &crate::win95_overlay::Win95OverlayRecord {
            kind: 15, tile_x: x as u8, tile_y: y as u8, ..Default::default()
        })
    }

    /// CODE 12 export 174 expands the dirty rectangle, intersects it with
    /// the map window, and then walks the clipped surface scene.
    pub(super) fn redraw_dirty_with_palette(
        self,
        city: &City,
        rng: &mut MacToolboxRng,
        dirty: MacMapClip,
        palette: &mut impl FnMut(usize),
    ) -> MacMapRedrawEffects {
        let expansion = 20 << self.zoom;
        let clip = MacMapClip {
            top: dirty.top.max(0),
            left: (dirty.left - expansion).max(0),
            bottom: (dirty.bottom + expansion).min(self.height),
            right: dirty.right.min(self.width),
        };
        let mut effects = MacMapRedrawEffects::default();
        self.redraw_markers_in_clip_with_palette(city, rng, clip, palette, &mut None, None, &mut effects, &mut |_, _, _| {});
        effects
    }

    /// CODE 12 export 175 intersects the actor bounds without export 174's
    /// extra left and bottom expansion.
    pub(super) fn redraw_actor_with_palette(
        self,
        city: &City,
        rng: &mut MacToolboxRng,
        dirty: MacMapClip,
        old_tile: Option<(usize, usize)>,
        new_tile: Option<(usize, usize)>,
        palette: &mut impl FnMut(usize),
    ) -> MacMapRedrawEffects {
        let clip = MacMapClip {
            top: dirty.top.max(0),
            left: dirty.left.max(0),
            bottom: dirty.bottom.min(self.height),
            right: dirty.right.min(self.width),
        };
        let mut tile_bounds: Option<MacActorTileBounds> = None;
        if self.zoom == 0 {
            for (x, y) in old_tile.into_iter().chain(new_tile) {
                let (x, y) = super::super::view_navigation::rotate_tile_coords(x, y, self.rotation);
                let diff = x as i32 - y as i32 + 127;
                let sum = x as i32 + y as i32;
                let next = MacActorTileBounds {
                    min_diff: diff - 5, max_diff: diff + 5,
                    min_sum: sum - 9, max_sum: sum,
                };
                tile_bounds = Some(match tile_bounds {
                    Some(old) => MacActorTileBounds {
                        min_diff: old.min_diff.min(next.min_diff),
                        max_diff: old.max_diff.max(next.max_diff),
                        min_sum: old.min_sum.min(next.min_sum),
                        max_sum: old.max_sum.max(next.max_sum),
                    },
                    None => next,
                });
            }
        }
        let mut effects = MacMapRedrawEffects::default();
        self.redraw_markers_in_clip_with_palette(city, rng, clip, palette, &mut None, tile_bounds, &mut effects, &mut |_, _, _| {});
        effects
    }

    /// Construct a view from the application's window and current camera.
    /// Classic Mac map windows reserve a scroll bar and menu/title area.
    pub(super) fn from_host(
        size: (u32, u32),
        pixel_scale: f32,
        zoom: f32,
        camera: glam::Vec2,
        rotation: u8,
    ) -> Self {
        let scale = pixel_scale.max(1.0);
        let width = ((size.0 as f32 / scale) as i32 - 16).max(1);
        let height = ((size.1 as f32 / scale) as i32 - 55).max(1);
        let host_zoom = super::super::view_navigation::render_zoom(zoom, scale);
        let zoom = ((zoom.max(0.25).log2() + 2.0).round() as i32).clamp(0, 2) as u32;
        let unit = 4 << zoom;
        let ratio = unit as f32 / (16.0 * host_zoom);
        Self {
            width,
            height,
            origin_x: width / 2 + (camera.x * ratio).round() as i32 + unit,
            origin_y: height / 2 + (camera.y * ratio).round() as i32 - 7 * (1 << zoom),
            zoom,
            rotation,
        }
    }

    pub(super) fn center_on(&mut self, city: &City, x: usize, y: usize) {
        let (xv, yv) = super::super::view_navigation::rotate_tile_coords(x, y, self.rotation);
        let unit = 4 << self.zoom;
        self.origin_x = self.width / 2 - unit * (xv as i32 - yv as i32);
        self.origin_y = self.height / 2 - unit / 2 * (xv + yv) as i32
            + 3 * (1 << self.zoom) * i32::from(city.altitude_at(x, y));
    }

    #[cfg(test)]
    pub(super) fn redraw_markers(self, city: &City, rng: &mut MacToolboxRng) {
        self.redraw_markers_with_palette(city, rng, &mut |_| {});
    }

    pub(super) fn redraw_markers_with_palette(
        self,
        city: &City,
        rng: &mut MacToolboxRng,
        palette: &mut impl FnMut(usize),
    ) -> MacMapRedrawEffects {
        let mut effects = MacMapRedrawEffects::default();
        self.redraw_markers_in_clip_with_palette(
            city,
            rng,
            MacMapClip { top: 0, left: 0, bottom: self.height, right: self.width },
            palette,
            &mut None,
            None,
            &mut effects,
            &mut |_, _, _| {},
        );
        effects
    }

    pub(super) fn redraw_markers_recording_dirty(
        self,
        city: &City,
        rng: &mut MacToolboxRng,
        palette: &mut impl FnMut(usize),
        dirty: &mut Option<MacMapClip>,
    ) -> MacMapRedrawEffects {
        let mut effects = MacMapRedrawEffects::default();
        self.redraw_markers_in_clip_with_palette(
            city,
            rng,
            MacMapClip { top: 0, left: 0, bottom: self.height, right: self.width },
            palette,
            dirty,
            None,
            &mut effects,
            &mut |_, _, _| {},
        );
        effects
    }

    fn redraw_markers_in_clip_with_palette(
        self,
        city: &City,
        rng: &mut MacToolboxRng,
        clip: MacMapClip,
        palette: &mut impl FnMut(usize),
        dirty: &mut Option<MacMapClip>,
        actor_tiles: Option<MacActorTileBounds>,
        effects: &mut MacMapRedrawEffects,
        observed: &mut impl FnMut(usize, usize, u32),
    ) {
        if clip.top >= clip.bottom || clip.left >= clip.right {
            return;
        }
        let unit = 4 << self.zoom;
        let left = clip.left - [24, 48, 128][self.zoom as usize];
        let top = clip.top - [8, 16, 32][self.zoom as usize];
        for diagonal in 0..(CITY_WIDTH + CITY_HEIGHT - 1) {
            for xv in diagonal.saturating_sub(CITY_HEIGHT - 1)..=diagonal.min(CITY_WIDTH - 1)
            {
                let yv = diagonal - xv;
                // The Mac rotates its map planes; CityGame retains canonical
                // coordinates, so walk the native rotated planes in reverse.
                let (x, y) = super::super::view_navigation::rotate_tile_coords(
                    xv, yv, (4 - (self.rotation & 3)) & 3,
                );
                let Some(marker) = city.text_overlay_at(x, y) else {
                    continue;
                };
                let actor = marker.checked_sub(0xc9).filter(|&slot| slot < 40)
                    .and_then(|slot| city.traffic_overlays()?.get(usize::from(slot)))
                    .filter(|actor| actor.kind <= 16);
                if actor.is_none() && !(0xfb..=0xff).contains(&marker) {
                    continue;
                }
                if marker >= 0xfd
                    && city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.raw_bits() & 4 != 0)
                {
                    continue;
                }
                let px = self.origin_x + unit * (xv as i32 - yv as i32);
                if let Some(bounds) = actor_tiles {
                    let sum = xv as i32 + yv as i32;
                    let diff = xv as i32 - yv as i32 + 127;
                    // The original walks diff in steps of two. Its signed
                    // division maps alternating diagonals to the next value.
                    let step_diff = diff - (diff - bounds.min_diff).rem_euclid(2);
                    if sum < bounds.min_sum - 16 || sum > bounds.max_sum + 22
                        || step_diff < bounds.min_diff - 8 || step_diff > bounds.max_diff
                        || (step_diff < bounds.min_diff
                            && step_diff + mac_small_building_width_units(city.building_at(x, y))
                                <= bounds.min_diff)
                    {
                        continue;
                    }
                } else if px <= left || px >= clip.right {
                    continue;
                }
                let base_y = self.origin_y + unit / 2 * (xv + yv) as i32;
                if xv == CITY_WIDTH - 1 || yv == CITY_HEIGHT - 1 {
                    // The edge wall loops return from the tile renderer if a
                    // wall segment rises above its expanded top clip. These
                    // are the pinned Mac terrain-wall sprite heights.
                    let land = city.altitude_at(x, y);
                    let water = city.water_height_at(x, y);
                    let wet = city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.raw_bits() & 4 != 0);
                    let steps = land + if wet { water.saturating_sub(land) } else { 0 };
                    let wall_height = [8, 15, 29][self.zoom as usize];
                    if steps > 0
                        && base_y - wall_height - 3 * (1 << self.zoom) * i32::from(steps - 1) < top
                    {
                        continue;
                    }
                }
                let height = if city.terrain_at(x, y) < 16 {
                    city.altitude_at(x, y)
                } else {
                    city.water_height_at(x, y)
                };
                let py = base_y - 3 * (1 << self.zoom) * i32::from(height);
                if city.building_at(x, y) < 0x70 && py < top {
                    continue;
                }
                if let Some(actor) = actor {
                    if self.zoom < MAC_ACTOR_MIN_ZOOM[usize::from(actor.kind)] { continue; }
                    if !matches!(actor.kind, 10 | 11)
                        && (usize::from(actor.tile_x), usize::from(actor.tile_y)) != (x, y) {
                        effects.stale_proxies.push((x, y));
                        continue;
                    }
                    if actor.kind == 6 {
                        effects.kind6_draws += 1;
                        continue;
                    }
                    if actor.kind != XTHG_TORNADO { continue; }
                }
                observed(x, y, rng.seed());
                let tornado = actor.is_some();
                let selection = if tornado || marker >= 0xfd { rng.random_u16() } else { 0 };
                rng.random_u16();
                if !tornado {
                    // CODE 10 export 248 unions the drawn marker sprite into
                    // the pending rectangle, including during Auto-Goto.
                    let zoom = self.zoom as usize;
                    let (height, width) = match marker {
                        0xfb => ([13, 26, 51][zoom], [16, 32, 64][zoom]),
                        0xfc => ([6, 9, 19][zoom], [8, 16, 32][zoom]),
                        0xfd | 0xfe => (
                            [[10, 10], [13, 14], [24, 25]][zoom][usize::from(selection & 1)],
                            [8, 16, 32][zoom],
                        ),
                        0xff => (
                            [[8, 9, 8, 9], [14, 15, 15, 15], [24, 20, 23, 22]][zoom]
                                [usize::from(selection & 3)],
                            [8, 16, 32][zoom],
                        ),
                        _ => unreachable!(),
                    };
                    let left = px + unit - width / 2;
                    let rect = MacMapClip { top: py - height, left, bottom: py, right: left + width };
                    *dirty = Some(dirty.map_or(rect, |old| old.union(rect)));
                }
            }
            // CODE 2 calls export 130 after reaching the end of each
            // diagonal, including diagonals entirely outside the view.
            if actor_tiles.is_none() {
                palette(diagonal);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_rotated_marker_order(rotation: u8, points: &[(usize, usize)]) -> Vec<(usize, usize)> {
        let mut city = City::new_blank("/tmp/mac-rotation-order.city", "Order").unwrap();
        for &(x, y) in points {
            city.set_altm_word(x, y, 4);
            city.set_terrain(x, y, 0);
            city.set_building(x, y, 0);
            city.set_text_overlay(x, y, 0xfb);
            city.set_tile_bits(x, y, TileBits::new(0));
        }
        let view = MacMapRedrawView::from_native_snapshot(624, 425, 312, -42, 0, rotation);
        let mut rng = MacToolboxRng::new(1);
        let mut order = Vec::new();
        view.redraw_markers_in_clip_with_palette(&city, &mut rng,
            MacMapClip::from_native_snapshot(0, 0, 425, 624), &mut |_| {}, &mut None, None,
            &mut MacMapRedrawEffects::default(),
            &mut |x, y, _| order.push((x, y)));
        order
    }

    #[test]
    fn rotated_map_redraw_walks_screen_diagonals() {
        let points = [(62, 62), (63, 61), (64, 62), (65, 60), (66, 61)];
        assert_eq!(record_rotated_marker_order(1, &points),
            [(62, 62), (64, 62), (63, 61), (66, 61), (65, 60)]);
    }

    #[test]
    fn marker_walk_below_window_still_consumes_original_random_draws() {
        let mut city = City::new_blank("/tmp/mac-below-window.city", "Order").unwrap();
        for (x, y, marker) in [(62, 62, 0xfb), (63, 61, 0xfc), (64, 62, 0xfd), (65, 60, 0xfe), (66, 61, 0xff)] {
            city.set_altm_word(x, y, 4);
            city.set_terrain(x, y, 0);
            city.set_building(x, y, 0);
            city.set_tile_bits(x, y, TileBits::new(0));
            city.set_text_overlay(x, y, marker);
        }
        let view = MacMapRedrawView::from_native_snapshot(624, 425, 312, 358, 0, 0);
        let mut rng = MacToolboxRng::new(0x48538fb1);
        view.redraw_markers(&city, &mut rng);
        assert_eq!(rng.seed(), 0x63645a4d);
    }

    fn compare_actor_redraw_fixture(filename: &str, kind: u8, expected_cases: usize) {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(root.join(filename)).unwrap()).unwrap();
        assert_eq!(rows.len(), expected_cases);
        for row in rows {
            let zoom = row["zoom"].as_u64().unwrap() as u32;
            let stale = row["stale"].as_bool().unwrap();
            let mut city = City::new_blank("/tmp/mac-actor-redraw-lfsr.city", "Actor").unwrap();
            for (x, y, marker) in [(62, 62, 0xfb), (64, 62, 0xcb)] {
                city.set_altm_word(x, y, 4);
                city.set_terrain(x, y, 0);
                city.set_building(x, y, 0);
                city.set_tile_bits(x, y, TileBits::new(0));
                city.set_text_overlay(x, y, marker);
            }
            city.set_mac_object_slot_record(2, crate::win95_overlay::Win95OverlayRecord {
                kind, lane: 1, tile_x: 64 + u8::from(stale), tile_y: 62,
                height: 4, sub_x: 8, sub_y: 8, ..Default::default()
            });
            let view = MacMapRedrawView::from_native_snapshot(624, 425, 312,
                row["origin_y"].as_i64().unwrap() as i32, zoom, 0);
            let mut qd = MacToolboxRng::new(0x48538fb1);
            let mut lfsr = MacAppLfsrRng::new(0xace1);
            view.redraw_markers_with_palette(&city, &mut qd, &mut |_| {}).apply(&mut city, &mut lfsr);
            assert_eq!(qd.seed(), row["qd_seed"].as_u64().unwrap() as u32, "{row}");
            assert_eq!(lfsr.state(), row["lfsr_after"].as_u64().unwrap() as u16, "{row}");
            assert_eq!(city.text_overlay_at(64, 62), Some(row["marker_after"].as_u64().unwrap() as u8), "{row}");
        }
    }

    #[test]
    #[ignore = "requires original Mac actor redraw replay"]
    fn kind6_redraw_stream_and_stale_proxy_match_original() {
        compare_actor_redraw_fixture("redraw-actor-lfsr.json", 6, 6);
    }

    #[test]
    #[ignore = "requires original Mac actor stale-proxy matrix"]
    fn actor_stale_proxy_zoom_gates_match_original_worker() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(root.join("actor-stale-matrix.json")).unwrap()).unwrap();
        assert_eq!(rows.len(), 51);
        for row in rows {
            let kind = row["kind"].as_u64().unwrap() as u8;
            let zoom = row["zoom"].as_u64().unwrap() as u32;
            let mut city = City::new_blank("/tmp/mac-stale-actor-matrix.city", "Actor").unwrap();
            city.set_altm_word(64, 62, 4);
            city.set_terrain(64, 62, 0);
            city.set_building(64, 62, 0);
            city.set_tile_bits(64, 62, TileBits::new(0));
            city.set_text_overlay(64, 62, 0xcb);
            city.set_mac_object_slot_record(2, crate::win95_overlay::Win95OverlayRecord {
                kind, lane: 1, tile_x: 65, tile_y: 62, height: 4, sub_x: 8, sub_y: 8,
                ..Default::default()
            });
            let view = MacMapRedrawView::from_native_snapshot(624, 425, 312, 210 - (2 << zoom) * 126, zoom, 0);
            let mut qd = MacToolboxRng::new(1);
            let mut lfsr = MacAppLfsrRng::new(0xace1);
            view.redraw_markers_with_palette(&city, &mut qd, &mut |_| {}).apply(&mut city, &mut lfsr);
            assert_eq!(city.text_overlay_at(64, 62), Some(row["marker_after"].as_u64().unwrap() as u8), "{row}");
            if !row["rail_stubbed"].as_bool().unwrap() {
                assert_eq!(lfsr.state(), row["lfsr_after"].as_u64().unwrap() as u16, "{row}");
                assert_eq!(qd.seed(), 1, "{row}");
            }
        }
    }

    #[test]
    #[ignore = "requires original Mac actor draw matrix"]
    fn matching_actor_redraw_random_streams_match_original_worker() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(root.join("actor-matching-draw-matrix.json")).unwrap()).unwrap();
        assert_eq!(rows.len(), 192);
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap();
            let kind = n("kind") as u8;
            let zoom = n("zoom") as u32;
            let mut city = City::new_blank("/tmp/mac-actor-matching.city", "Actor").unwrap();
            city.set_altm_word(64, 62, 4);
            city.set_terrain(64, 62, 0);
            city.set_building(64, 62, 0);
            city.set_tile_bits(64, 62, TileBits::new(0));
            city.set_text_overlay(64, 62, 0xcb);
            city.set_mac_object_slot_record(2, crate::win95_overlay::Win95OverlayRecord {
                kind, lane: n("lane") as u8, state: n("state") as u8,
                tile_x: 64, tile_y: 62, height: 4, sub_x: 8, sub_y: 8,
                ..Default::default()
            });
            let view = MacMapRedrawView::from_native_snapshot(624, 425, 312, 210 - (2 << zoom) * 126, zoom, 0);
            let mut qd = MacToolboxRng::new(0x48538fb1);
            let mut lfsr = MacAppLfsrRng::new(0xace1);
            view.redraw_markers_with_palette(&city, &mut qd, &mut |_| {}).apply(&mut city, &mut lfsr);
            assert_eq!(city.text_overlay_at(64, 62), Some(n("marker_after") as u8), "{row}");
            assert_eq!(lfsr.state(), n("lfsr_after") as u16, "{row}");
            assert_eq!(qd.seed(), n("qd_after") as u32, "{row}");
        }
    }

    #[test]
    #[ignore = "requires original Mac tornado redraw replay"]
    fn tornado_redraw_and_stale_proxy_match_original() {
        compare_actor_redraw_fixture("redraw-tornado-proxy.json", XTHG_TORNADO, 2);
    }

    #[test]
    #[ignore = "requires original Mac renderer rotation replay"]
    fn rotated_map_redraw_order_matches_original_renderer() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(root.join("redraw-rotation-order.json")).unwrap()).unwrap();
        assert_eq!(rows.len(), 4);
        for row in rows {
            let rotation = row["rotation"].as_u64().unwrap() as u8;
            let points: Vec<_> = row["points"].as_array().unwrap().iter().map(|p|
                (p[0].as_u64().unwrap() as usize, p[1].as_u64().unwrap() as usize)).collect();
            let actual: Vec<_> = record_rotated_marker_order(rotation, &points).into_iter().map(|(x, y)|
                super::super::super::view_navigation::rotate_tile_coords(x, y, rotation)).collect();
            let expected: Vec<_> = row["calls"].as_array().unwrap().iter().map(|p|
                (p[0].as_u64().unwrap() as usize, p[1].as_u64().unwrap() as usize)).collect();
            assert_eq!(actual, expected, "rotation {rotation}");
        }
    }

    fn compare_redraw_marker_rng_fixture(filename: &str, expected_cases: usize) {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(root.join(filename)).unwrap()).unwrap();
        assert_eq!(rows.len(), expected_cases);
        for row in rows {
            let zoom = row["zoom"].as_u64().unwrap() as u32;
            let rotation = row["rotation"].as_u64().unwrap() as u8;
            let mut city = City::new_blank("/tmp/mac-zoom-rotation.city", "Order").unwrap();
            for point in row["points"].as_array().unwrap() {
                let x = point[0].as_u64().unwrap() as usize;
                let y = point[1].as_u64().unwrap() as usize;
                city.set_altm_word(x, y, 4);
                city.set_terrain(x, y, 0);
                city.set_building(x, y, 0);
                city.set_tile_bits(x, y, TileBits::new(0));
                city.set_text_overlay(x, y, point[2].as_u64().unwrap() as u8);
            }
            let view = MacMapRedrawView::from_native_snapshot(624, 425,
                row["origin_x"].as_i64().unwrap_or(312) as i32,
                row["origin_y"].as_i64().unwrap() as i32, zoom, rotation);
            let mut rng = MacToolboxRng::new(0x48538fb1);
            let mut calls = Vec::new();
            view.redraw_markers_in_clip_with_palette(&city, &mut rng,
                MacMapClip::from_native_snapshot(0, 0, 425, 624), &mut |_| {}, &mut None, None,
                &mut MacMapRedrawEffects::default(),
                &mut |x, y, seed| {
                    let (xv, yv) = super::super::super::view_navigation::rotate_tile_coords(x, y, rotation);
                    calls.push((xv, yv, city.text_overlay_at(x, y).unwrap(), seed));
                });
            let expected: Vec<_> = row["calls"].as_array().unwrap().iter().map(|p|
                (p[0].as_u64().unwrap() as usize, p[1].as_u64().unwrap() as usize,
                 p[2].as_u64().unwrap() as u8, p[3].as_u64().unwrap() as u32)).collect();
            assert_eq!(calls, expected, "zoom {zoom}, rotation {rotation}");
            assert_eq!(rng.seed(), row["seed"].as_u64().unwrap() as u32, "zoom {zoom}, rotation {rotation}");
        }
    }

    #[test]
    #[ignore = "requires original Mac redraw zoom and rotation replay"]
    fn rotated_redraw_marker_rng_matches_original_all_zooms() {
        compare_redraw_marker_rng_fixture("redraw-zoom-rotation.json", 12);
    }

    #[test]
    #[ignore = "requires original Mac clipped redraw replay"]
    fn clipped_rotated_redraw_marker_rng_matches_original() {
        compare_redraw_marker_rng_fixture("redraw-clipped-rotation.json", 16);
    }

    #[test]
    fn dirty_union_ignores_empty_rectangles() {
        let tile = MacMapClip::from_native_snapshot(10, 20, 30, 40);
        let zero_height = MacMapClip::from_native_snapshot(15, 0, 15, 60);
        assert_eq!(tile.union(zero_height), tile);
        assert_eq!(zero_height.union(tile), tile);
    }

    #[test]
    fn water_building_offset_does_not_extend_terrain_bottom() {
        let mut city = City::new_blank("/tmp/mac-water-building-bounds.city", "Bounds").unwrap();
        city.set_altm_word(65, 61, 6 | (3 << 5));
        city.set_terrain(65, 61, 0);
        city.set_building(65, 61, 0x51);
        let view = MacMapRedrawView::from_native_snapshot(624, 425, 312, -42, 0, 0);
        assert_eq!(view.surface_tile_dirty_bounds(&city, 65, 61, 5, Some((5, 8))),
            Some(MacMapClip::from_native_snapshot(187, 328, 192, 336)));
    }

    #[test]
    #[ignore = "requires synthetic original Mac actor-bounds matrix"]
    fn actor_bounds_match_original_kind_zoom_rotation_matrix() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("actor-bounds-matrix.json")).unwrap()).unwrap();
        let rows = fixture["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 552);
        let mut city = City::new_blank("/tmp/mac-actor-bounds-matrix.city", "Bounds").unwrap();
        let mut mismatches = Vec::new();
        for row in rows {
            let n = |key: &str| row[key].as_i64().unwrap();
            let (x, y) = (n("x") as usize, n("y") as usize);
            city.set_altm_word(x, y, 6 | (3 << 5));
            city.set_tile_bits(x, y, TileBits::new(if row["wet"].as_bool().unwrap() { 4 } else { 0 }));
            let actor = crate::win95_overlay::Win95OverlayRecord {
                kind: n("kind") as u8, height: n("height") as u8,
                tile_x: x as u8, tile_y: y as u8, ..Default::default()
            };
            let view = MacMapRedrawView::from_native_snapshot(624, 425, n("origin_x") as i32,
                n("origin_y") as i32, n("zoom") as u32, n("rotation") as u8);
            let bounds = view.actor_dirty_bounds(&city, &actor);
            let a = &row["after"];
            let expected = MacMapClip::from_native_snapshot(a[0].as_i64().unwrap() as i32, a[1].as_i64().unwrap() as i32,
                a[2].as_i64().unwrap() as i32, a[3].as_i64().unwrap() as i32);
            let actual = bounds.unwrap_or(MacMapClip::from_native_snapshot(-1000, -1000, -1, -1));
            if actual != expected { mismatches.push(format!("{row} actual {actual:?}")); }
        }
        assert!(mismatches.is_empty(), "{} mismatches:\n{}", mismatches.len(), mismatches.join("\n"));
    }

    #[test]
    #[ignore = "requires original Mac assets and synthetic tile/actor matrix"]
    fn tile_and_actor_bounds_match_original_composition() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let pack = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SHAPE_PACK").unwrap());
        let resources = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCES").unwrap());
        let mut shapes = crate::mac_shape_table::MacShapeDimensions::from_pack(&pack).unwrap();
        shapes.set_terrain_shapes(crate::mac_import::terrain_shape_map_from_resources(&resources).unwrap().unwrap());
        let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("tile-actor-bounds-matrix.json")).unwrap()).unwrap();
        let rows = fixture["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 96);
        let mut city = City::new_blank("/tmp/mac-tile-actor-bounds.city", "Bounds").unwrap();
        let mut mismatches = Vec::new();
        for row in rows {
            let n = |key: &str| row[key].as_i64().unwrap();
            let (x, y) = (n("x") as usize, n("y") as usize);
            city.set_altm_word(x, y, 6 | (3 << 5));
            city.set_terrain(x, y, 0); city.set_building(x, y, 0);
            city.set_text_overlay(x, y, 0xcb);
            city.set_tile_bits(x, y, TileBits::new(if row["wet"].as_bool().unwrap() { 4 } else { 0 }));
            city.set_mac_object_slot_record(2, crate::win95_overlay::Win95OverlayRecord {
                kind: n("kind") as u8, height: n("height") as u8,
                tile_x: x as u8, tile_y: y as u8, ..Default::default()
            });
            let view = MacMapRedrawView::from_native_snapshot(624, 425, n("origin_x") as i32,
                n("origin_y") as i32, n("zoom") as u32, n("rotation") as u8);
            let actual = view.tile_dirty_bounds_from_assets(&city, x, y, &shapes);
            let a = &row["after"];
            let expected = MacMapClip::from_native_snapshot(a[0].as_i64().unwrap() as i32, a[1].as_i64().unwrap() as i32,
                a[2].as_i64().unwrap() as i32, a[3].as_i64().unwrap() as i32);
            if actual != Some(expected) { mismatches.push(format!("{row} actual {actual:?}")); }
        }
        assert!(mismatches.is_empty(), "{} mismatches:\n{}", mismatches.len(), mismatches.join("\n"));
    }

    #[test]
    #[ignore = "requires private original-Mac selector 11 bounds fixture"]
    fn surface_tile_bounds_match_original_selector11_classifier() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("tile-bounds-worker.json")).unwrap()).unwrap();
        let view = MacMapRedrawView::from_native_snapshot(624, 425, 312, -32, 0, 0);
        let mut city = City::new_blank("/tmp/mac-bounds-fixture.city", "Bounds").unwrap();
        let rows = fixture["tile_bounds"].as_array().unwrap();
        assert_eq!(rows.len(), 18);
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let x = n("x"); let y = n("y");
            city.set_altm_word(x, y, n("altm") as u16);
            city.set_terrain(x, y, n("terrain") as u8);
            city.set_building(x, y, n("building") as u8);
            city.set_zone(x, y, n("zone") as u8);
            let bounds = view.surface_tile_dirty_bounds(&city, x, y, n("terrain_height") as i32,
                Some((n("building_height") as i32, n("building_width") as i32)));
            let clip = |name: &str| {
                let a = &row[name];
                MacMapClip::from_native_snapshot(a[0].as_i64().unwrap() as i32, a[1].as_i64().unwrap() as i32,
                    a[2].as_i64().unwrap() as i32, a[3].as_i64().unwrap() as i32)
            };
            let previous = clip("before");
            let actual = if previous.top == -1000 { bounds } else { bounds.map_or(Some(previous), |b| Some(previous.union(b))) };
            assert_eq!(actual, Some(clip("after")), "tile {x},{y}");
        }
    }

    #[test]
    #[ignore = "requires original Mac assets and selector 11 capture"]
    fn asset_backed_tile_bounds_match_original_selector11_classifier() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let pack = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SHAPE_PACK").unwrap());
        let resources = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCES").unwrap());
        let ram = std::fs::read(root.join("entry.ram")).unwrap();
        let a5 = u32::from_be_bytes(ram[0x904..0x908].try_into().unwrap()) as usize;
        let map = crate::mac_import::terrain_shape_map_from_resources(&resources).unwrap().unwrap();
        for (index, &id) in map.iter().enumerate() {
            let address = a5 - 0x5082 + index * 2;
            assert_eq!(id, u16::from_be_bytes(ram[address..address + 2].try_into().unwrap()), "terrain {index}");
        }
        let mut shapes = crate::mac_shape_table::MacShapeDimensions::from_pack(&pack).unwrap();
        shapes.set_terrain_shapes(map);
        let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("tile-bounds-worker.json")).unwrap()).unwrap();
        let rows = fixture["tile_bounds"].as_array().unwrap();
        assert_eq!(rows.len(), 18);
        let view = MacMapRedrawView::from_native_snapshot(624, 425, 312, -32, 0, 0);
        let mut city = City::new_blank("/tmp/mac-asset-bounds.city", "Bounds").unwrap();
        for row in rows {
            let n = |key: &str| row[key].as_u64().unwrap() as usize;
            let (x, y) = (n("x"), n("y"));
            city.set_altm_word(x, y, n("altm") as u16);
            city.set_terrain(x, y, n("terrain") as u8);
            city.set_building(x, y, n("building") as u8);
            city.set_zone(x, y, n("zone") as u8);
            let bounds = view.surface_tile_dirty_bounds_from_assets(&city, x, y, &shapes);
            let clip = |name: &str| {
                let a = &row[name];
                MacMapClip::from_native_snapshot(a[0].as_i64().unwrap() as i32, a[1].as_i64().unwrap() as i32,
                    a[2].as_i64().unwrap() as i32, a[3].as_i64().unwrap() as i32)
            };
            let previous = clip("before");
            let actual = if previous.top == -1000 { bounds } else { bounds.map_or(Some(previous), |b| Some(previous.union(b))) };
            assert_eq!(actual, Some(clip("after")), "tile {x},{y}");
        }
    }

    #[test]
    #[ignore = "requires original Mac assets and synthetic original-code matrix"]
    fn asset_backed_bounds_match_original_zoom_rotation_matrix() {
        let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SELECTOR11_ACCEPTED").unwrap());
        let pack = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SHAPE_PACK").unwrap());
        let resources = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_RESOURCES").unwrap());
        let mut shapes = crate::mac_shape_table::MacShapeDimensions::from_pack(&pack).unwrap();
        shapes.set_terrain_shapes(crate::mac_import::terrain_shape_map_from_resources(&resources).unwrap().unwrap());
        let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("tile-bounds-matrix.json")).unwrap()).unwrap();
        let rows = fixture["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 192);
        let mut city = City::new_blank("/tmp/mac-asset-bounds-matrix.city", "Bounds").unwrap();
        let mut mismatches = Vec::new();
        for row in rows {
            let n = |key: &str| row[key].as_i64().unwrap();
            let (x, y) = (n("x") as usize, n("y") as usize);
            city.set_altm_word(x, y, n("altm") as u16);
            city.set_terrain(x, y, n("terrain") as u8);
            city.set_building(x, y, n("building") as u8);
            city.set_zone(x, y, n("zone") as u8);
            let view = MacMapRedrawView::from_native_snapshot(624, 425, n("origin_x") as i32, n("origin_y") as i32,
                n("zoom") as u32, n("rotation") as u8);
            let bounds = view.surface_tile_dirty_bounds_from_assets(&city, x, y, &shapes);
            let a = &row["after"];
            let expected = MacMapClip::from_native_snapshot(a[0].as_i64().unwrap() as i32, a[1].as_i64().unwrap() as i32,
                a[2].as_i64().unwrap() as i32, a[3].as_i64().unwrap() as i32);
            let actual = bounds.unwrap_or(MacMapClip::from_native_snapshot(-1000, -1000, -1, -1));
            if actual != expected { mismatches.push(format!("{row} actual {actual:?}")); }
        }
        assert!(mismatches.is_empty(), "{} mismatches:\n{}", mismatches.len(), mismatches.join("\n"));
    }

    #[test]
    fn redraw_calls_palette_after_each_map_diagonal() {
        let city = City::new_blank("/tmp/mac-palette-redraw.sc2", "Redraw").unwrap();
        let view = MacMapRedrawView {
            width: 80,
            height: 80,
            origin_x: 40,
            origin_y: 0,
            zoom: 0,
            rotation: 0,
        };
        let mut rng = MacToolboxRng::new(1);
        let mut diagonals = Vec::new();
        view.redraw_markers_with_palette(&city, &mut rng, &mut |diagonal| diagonals.push(diagonal));
        assert_eq!(diagonals, (0..255).collect::<Vec<_>>());
        assert_eq!(rng.seed(), 1);
    }

    #[test]
    fn underground_redraw_calls_palette_after_each_diagonal() {
        let mut diagonals = Vec::new();
        redraw_underground_palette(&mut |diagonal| diagonals.push(diagonal));
        assert_eq!(diagonals, (0..255).collect::<Vec<_>>());
    }

    #[test]
    fn marker_redraw_clips_and_respects_water_marker_types() {
        let mut city = City::new_blank("/tmp/mac-map-redraw.sc2", "Redraw").unwrap();
        let view = MacMapRedrawView {
            width: 80,
            height: 80,
            origin_x: 40,
            origin_y: 0,
            zoom: 0,
            rotation: 0,
        };
        city.set_text_overlay(5, 5, 0xff); // two draws
        city.set_text_overlay(6, 5, 0xfb); // one draw, allowed on water
        city.set_tile_bits(6, 5, TileBits::new(4));
        city.set_text_overlay(7, 5, 0xfd); // rejected on water
        city.set_tile_bits(7, 5, TileBits::new(4));
        city.set_text_overlay(100, 0, 0xff); // outside horizontal clip
        let mut actual = MacToolboxRng::new(1);
        view.redraw_markers(&city, &mut actual);
        let mut expected = MacToolboxRng::new(1);
        for _ in 0..3 {
            expected.random_u16();
        }
        assert_eq!(actual.seed(), expected.seed());
    }

    #[test]
    fn destruction_frame_uses_selected_shape_and_clips_marker_redraw() {
        let mut city = City::new_blank("/tmp/mac-destruction-redraw.sc2", "Redraw").unwrap();
        let view = MacMapRedrawView {
            width: 80, height: 80, origin_x: 40, origin_y: 0, zoom: 0, rotation: 0,
        };
        let bounds = view.destruction_frame_bounds(5, 5, 0, 0, 0, 0, 1);
        assert_eq!(bounds, MacMapClip { top: 7, left: 40, bottom: 20, right: 48 });
        assert_eq!(view.destruction_frame_bounds(5, 5, 0, 1, 1, 0, 0),
            MacMapClip { top: 3, left: 44, bottom: 20, right: 52 });
        city.set_mac_object_slot_record(1, crate::win95_overlay::Win95OverlayRecord {
            kind: XTHG_TORNADO, tile_x: 5, tile_y: 5, ..Default::default()
        });
        city.set_altitude(5, 5, 0, 0);
        city.set_text_overlay(5, 5, 0xca);
        city.set_text_overlay(20, 5, 0xff); // outside the explosion clip
        let mut actual = MacToolboxRng::new(1);
        view.redraw_dirty_with_palette(&city, &mut actual, bounds, &mut |_| {});
        let mut expected = MacToolboxRng::new(1);
        expected.random_u16();
        expected.random_u16();
        assert_eq!(actual.seed(), expected.seed());
    }

    #[test]
    fn visible_tornado_proxy_draws_from_shared_quickdraw_stream() {
        let mut city = City::new_blank("/tmp/mac-tornado-redraw.sc2", "Redraw").unwrap();
        let view = MacMapRedrawView {
            width: 80,
            height: 80,
            origin_x: 40,
            origin_y: 0,
            zoom: 0,
            rotation: 0,
        };
        city.set_mac_object_slot_record(
            1,
            crate::win95_overlay::Win95OverlayRecord {
                kind: XTHG_TORNADO,
                tile_x: 5,
                tile_y: 5,
                ..Default::default()
            },
        );
        city.set_text_overlay(5, 5, 0xca);
        let mut actual = MacToolboxRng::new(1);
        view.redraw_markers(&city, &mut actual);
        let mut expected = MacToolboxRng::new(1);
        expected.random_u16();
        expected.random_u16();
        assert_eq!(actual.seed(), expected.seed());

        city.set_text_overlay(5, 5, 0);
        city.set_text_overlay(100, 0, 0xca);
        let mut clipped = MacToolboxRng::new(1);
        view.redraw_markers(&city, &mut clipped);
        assert_eq!(clipped.seed(), 1);
    }

    #[test]
    fn stale_tornado_proxy_clears_without_drawing() {
        let mut city = City::new_blank("/tmp/mac-stale-tornado.sc2", "Redraw").unwrap();
        city.set_mac_object_slot_record(1, crate::win95_overlay::Win95OverlayRecord {
            kind: XTHG_TORNADO, tile_x: 6, tile_y: 5, ..Default::default()
        });
        city.set_text_overlay(5, 5, 0xca);
        let view = MacMapRedrawView::from_native_snapshot(80, 80, 40, 0, 0, 0);
        let mut qd = MacToolboxRng::new(1);
        let mut lfsr = MacAppLfsrRng::new(1);
        view.redraw_markers_with_palette(&city, &mut qd, &mut |_| {}).apply(&mut city, &mut lfsr);
        assert_eq!(city.text_overlay_at(5, 5), Some(0));
        assert_eq!(qd.seed(), 1);
        assert_eq!(lfsr.state(), 1);
    }

    #[test]
    fn surface_walk_uses_supplied_clip_without_moving_the_camera() {
        let mut city = City::new_blank("/tmp/mac-clipped-redraw.sc2", "Redraw").unwrap();
        let view = MacMapRedrawView {
            width: 80,
            height: 80,
            origin_x: 40,
            origin_y: 0,
            zoom: 0,
            rotation: 0,
        };
        city.set_text_overlay(5, 5, 0xff); // x=40
        city.set_text_overlay(7, 5, 0xff); // x=48, beyond the clip's right edge
        let mut actual = MacToolboxRng::new(1);
        view.redraw_markers_in_clip_with_palette(
            &city,
            &mut actual,
            MacMapClip { top: 0, left: 0, bottom: 80, right: 44 },
            &mut |_| {},
            &mut None,
            None,
            &mut MacMapRedrawEffects::default(),
            &mut |_, _, _| {},
        );
        let mut expected = MacToolboxRng::new(1);
        expected.random_u16();
        expected.random_u16();
        assert_eq!(actual.seed(), expected.seed());

        let mut empty = MacToolboxRng::new(1);
        view.redraw_markers_in_clip_with_palette(
            &city,
            &mut empty,
            MacMapClip { top: 2, left: 0, bottom: 2, right: 44 },
            &mut |_| panic!("empty rectangles do not enter the walk"),
            &mut None,
            None,
            &mut MacMapRedrawEffects::default(),
            &mut |_, _, _| {},
        );
        assert_eq!(empty.seed(), 1);
    }

    #[test]
    fn tornado_invalidation_uses_the_view_before_recentering() {
        let mut city = City::new_blank("/tmp/mac-tornado-bounds.sc2", "Bounds").unwrap();
        city.set_altitude(77, 114, 4, 0);
        let view = MacMapRedrawView {
            width: 624,
            height: 425,
            origin_x: 260,
            origin_y: -70,
            zoom: 0,
            rotation: 0,
        };
        assert_eq!(view.tornado_dirty_bounds(&city, 77, 114), Some(MacMapClip {
            top: 264, left: 96, bottom: 300, right: 136,
        }));
        assert_eq!(view.tornado_dirty_bounds(&city, 0, 127), None);
    }

    #[test]
    fn fire_sprite_unions_its_selected_shape_with_existing_actor_bounds() {
        let mut city = City::new_blank("/tmp/mac-fire-dirty.sc2", "Bounds").unwrap();
        city.set_altitude(5, 5, 0, 0);
        city.set_text_overlay(5, 5, 0xff);
        let view = MacMapRedrawView {
            width: 80, height: 80, origin_x: 40, origin_y: 0, zoom: 0, rotation: 0,
        };
        let mut dirty = Some(MacMapClip { top: 30, left: 1, bottom: 40, right: 10 });
        let mut rng = MacToolboxRng::new(1);
        view.redraw_markers_recording_dirty(&city, &mut rng, &mut |_| {}, &mut dirty);
        // Seed 1 selects shape 399: 8 pixels wide and 9 high at zoom 0.
        assert_eq!(dirty, Some(MacMapClip { top: 11, left: 1, bottom: 40, right: 48 }));
    }

    #[test]
    fn centering_projects_native_ignition_tile_to_window_anchor() {
        let mut city = City::new_blank("/tmp/mac-map-center.sc2", "Center").unwrap();
        city.set_altitude(72, 48, 4, 0);
        let mut view = MacMapRedrawView::from_host((640, 480), 1.0, 0.35, glam::Vec2::ZERO, 0);
        view.center_on(&city, 72, 48);
        assert_eq!((view.width, view.height), (624, 425));
        assert_eq!((view.origin_x, view.origin_y), (216, -16));
    }
}
