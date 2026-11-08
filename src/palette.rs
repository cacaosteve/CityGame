use anyhow::{Context, Result};
use image::RgbaImage;
use std::fs;
use std::path::Path;

pub const MAX_ANIM_FRAMES: usize = 24;
/// Existing Win95/default palette animation rate.
pub const ANIMATION_FPS: f32 = 4.0;
/// Mac CODE 6 `export_130` advances the 49-entry water/smoke palette track
/// every 12 classic TickCount ticks (60 ticks per second).
pub const MAC_PALETTE_ANIMATION_FPS: f32 = 5.0;
pub const ANIM_SPRITE_STRIDE: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub fn to_rgba(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    fn distance_sq(self, other: Self) -> u32 {
        let dr = (self.r as i32 - other.r as i32).unsigned_abs();
        let dg = (self.g as i32 - other.g as i32).unsigned_abs();
        let db = (self.b as i32 - other.b as i32).unsigned_abs();
        dr * dr + dg * dg + db * db
    }
}

#[derive(Debug, Clone)]
pub struct AnimatedPalette {
    primary: [Color; 256],
    frames: [[Color; MAX_ANIM_FRAMES]; 256],
    frame_count: [u8; 256],
    global_cycle: u8,
}

impl AnimatedPalette {
    pub fn load(asset_root: impl AsRef<Path>) -> Result<Self> {
        let palette_root = asset_root.as_ref().join("palettes");
        let csv_path = palette_root.join("pltt_00000_Default_Palette.csv");
        let primary = if csv_path.is_file() {
            parse_default_palette(&csv_path)?
        } else {
            [
                "win95_pal_mstr.png",
                "mine_palette.png",
                "pltt_00000_Default_Palette.png",
            ]
            .iter()
            .map(|name| palette_root.join(name))
            .find(|path| path.is_file())
            .map(|path| parse_palette_image(&path))
            .transpose()?
            .unwrap_or_else(|| {
                log::warn!(
                    "no exported palette found under {}; using a neutral runtime palette",
                    palette_root.display()
                );
                std::array::from_fn(|index| Color {
                    r: index as u8,
                    g: index as u8,
                    b: index as u8,
                    a: 255,
                })
            })
        };
        let mut palette = Self {
            primary,
            frames: [[Color::TRANSPARENT; MAX_ANIM_FRAMES]; 256],
            frame_count: [1; 256],
            global_cycle: 24,
        };
        palette.build_animation_tables();
        Ok(palette)
    }

    pub fn frame_count_for_index(&self, index: u8) -> u8 {
        self.frame_count[index as usize]
    }

    pub fn color_for_index(&self, index: u8, frame: u8) -> Color {
        let count = self.frame_count[index as usize].max(1);
        let frame = (frame as usize) % count as usize;
        self.frames[index as usize][frame]
    }

    pub fn nearest_index(&self, pixel: [u8; 4]) -> Option<u8> {
        if pixel[3] == 0 {
            return None;
        }
        let sample = Color {
            r: pixel[0],
            g: pixel[1],
            b: pixel[2],
            a: pixel[3],
        };
        let mut best_index = 0u8;
        let mut best_distance = u32::MAX;
        for (index, color) in self.primary.iter().enumerate() {
            let distance = sample.distance_sq(*color);
            if distance < best_distance {
                best_distance = distance;
                best_index = index as u8;
            }
        }
        if best_distance > 60 * 60 * 3 {
            None
        } else {
            Some(best_index)
        }
    }

    pub fn sprite_animation_frame_count(&self, pixels: &[(u8, u8, u8)]) -> u8 {
        let mut counts = Vec::new();
        for &(r, g, b) in pixels {
            if let Some(index) = self.nearest_index([r, g, b, 255]) {
                let count = self.frame_count_for_index(index);
                if count > 1 {
                    counts.push(count);
                }
            }
        }
        lcm_counts(&counts).max(1)
    }

    pub fn bake_sprite_frame(&self, pixels: &mut RgbaImage, anim_frame: u8) {
        let width = pixels.width();
        let height = pixels.height();
        for y in 0..height {
            for x in 0..width {
                let pixel = pixels.get_pixel(x, y).0;
                if pixel[3] == 0 {
                    continue;
                }
                let dither_even = (x + y) % 2 == 0;
                if let Some(color) = self.remap_pixel(pixel, anim_frame, dither_even) {
                    pixels.put_pixel(x, y, image::Rgba(color.to_rgba()));
                }
            }
        }
    }

    pub fn bake_traffic_sprite_frame(
        &self,
        pixels: &mut RgbaImage,
        car_color_index: u8,
        anim_frame: u8,
    ) {
        let body = self.traffic_car_color(car_color_index, anim_frame);
        let width = pixels.width();
        let height = pixels.height();
        let source = pixels.clone();

        for y in 0..height {
            for x in 0..width {
                let pixel = source.get_pixel(x, y).0;
                let color = self
                    .traffic_mask_pixel(pixel, anim_frame, body)
                    .map(|color| color.to_rgba())
                    .unwrap_or([0, 0, 0, 0]);
                pixels.put_pixel(x, y, image::Rgba(color));
            }
        }
    }

    pub fn bake_traffic_overlay_sprite_frame(
        &self,
        pixels: &mut RgbaImage,
        car_color_index: u8,
        anim_frame: u8,
    ) {
        let body = self.traffic_car_color(car_color_index, anim_frame);
        let source = pixels.clone();

        for y in 0..source.height() {
            for x in 0..source.width() {
                let pixel = source.get_pixel(x, y).0;
                let color = self
                    .traffic_overlay_mask_pixel(pixel, anim_frame, body)
                    .map(|color| color.to_rgba())
                    .unwrap_or([0, 0, 0, 0]);
                pixels.put_pixel(x, y, image::Rgba(color));
            }
        }
    }

    /// 16×11+ car shapes (418–426): white highlight + tinted body, like lane dashes.
    pub fn bake_traffic_car_sprite(
        &self,
        pixels: &mut RgbaImage,
        car_color_index: u8,
        anim_frame: u8,
    ) {
        let body = self.traffic_car_color(car_color_index, anim_frame);
        let source = pixels.clone();
        for y in 0..source.height() {
            for x in 0..source.width() {
                let pixel = source.get_pixel(x, y).0;
                let color = self
                    .traffic_car_mask_pixel(pixel, body)
                    .map(|color| color.to_rgba())
                    .unwrap_or([0, 0, 0, 0]);
                pixels.put_pixel(x, y, image::Rgba(color));
            }
        }
    }

    /// Mac traffic reads as a saturated lane color on the default palette.
    fn traffic_car_color(&self, car_color_index: u8, anim_frame: u8) -> Color {
        let mut color = self.color_for_index(car_color_index, anim_frame);
        if car_color_index == 81 {
            color = Color {
                r: 0,
                g: 0,
                b: 168,
                a: 255,
            };
        }
        color
    }

    pub fn traffic_tint_rgba(&self, color_slot: u8, anim_frame: u8) -> [f32; 4] {
        let index = traffic_color_index_for_slot(color_slot);
        let color = self.traffic_car_color(index, anim_frame);
        [
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
            1.0,
        ]
    }

    pub fn bake_water_sprite_frame(&self, pixels: &mut RgbaImage, anim_frame: u8) {
        let width = pixels.width();
        let height = pixels.height();
        for y in 0..height {
            for x in 0..width {
                let pixel = pixels.get_pixel(x, y).0;
                let dither_even = (x + y) % 2 == 0;
                if let Some(color) = self.normalize_water_pixel(pixel, anim_frame, dither_even) {
                    pixels.put_pixel(x, y, image::Rgba(color.to_rgba()));
                }
            }
        }
    }

    /// Smoke stacks only: Mac/Win palette animation changes color entries in place. The extracted
    /// PNGs expose those entries as green marker pixels, so only marker pixels inside the known
    /// plume band should be remapped.
    pub fn bake_smoke_sprite_frame(&self, pixels: &mut RgbaImage, sprite_id: u32, anim_frame: u8) {
        let width = pixels.width();
        let height = pixels.height();
        for y in 0..height {
            for x in 0..width {
                let pixel = pixels.get_pixel(x, y).0;
                if !is_smoke_plume_pixel(sprite_id, width, height, x, y, pixel) {
                    continue;
                }
                if let Some(color) = self.remap_smoke_pixel(pixel, anim_frame) {
                    pixels.put_pixel(x, y, image::Rgba(color.to_rgba()));
                }
            }
        }
    }

    fn remap_smoke_pixel(&self, pixel: [u8; 4], anim_frame: u8) -> Option<Color> {
        let phase = smoke_marker_phase(pixel)?;
        Some(smoke_cycle_color(phase, anim_frame, pixel[3]))
    }

    fn remap_pixel(&self, pixel: [u8; 4], anim_frame: u8, dither_even: bool) -> Option<Color> {
        if pixel[3] == 0 {
            return None;
        }

        if is_water_animation_pixel(pixel) {
            return self.normalize_water_pixel(pixel, anim_frame, dither_even);
        }

        if is_smoke_animation_pixel(pixel) {
            if let Some(index) = self.nearest_index(pixel)
                && self.frame_count_for_index(index) > 1
            {
                return Some(self.color_for_index(index, anim_frame));
            }
            return self.normalize_smoke_pixel(pixel);
        }

        None
    }

    pub fn normalize_water_pixel(
        &self,
        pixel: [u8; 4],
        anim_frame: u8,
        _dither_even: bool,
    ) -> Option<Color> {
        if pixel[3] == 0 {
            return None;
        }
        // Grass/shore colors in the water shape export are not part of the overlay.
        if is_shore_terrain_color(pixel) {
            return Some(Color::TRANSPARENT);
        }

        // Default-palette marker pixels (greens 171–178 / 187–194, browns 195–198)
        // are cycling slots. Win95 `AnimatePalette(171, 49)` and Mac CLUT 500 turn
        // them into light water blues; do not snap them to navy 127–129.
        if let Some(phase) = self.water_marker_phase(pixel) {
            return Some(water_cycle_color(phase, anim_frame, pixel[3]));
        }

        // Stable body (palette 79 = rgb 35,39,255) and decoded foam/highlight
        // blues already match original SC2K. Leave them alone.
        None
    }

    fn water_marker_phase(&self, pixel: [u8; 4]) -> Option<usize> {
        if let Some(phase) = water_marker_phase_exact(pixel) {
            return Some(phase);
        }
        if !(is_water_animation_green(pixel) || is_water_animation_brown(pixel)) {
            return None;
        }
        match self.nearest_index(pixel)? {
            index @ 171..=178 => Some((index - 171) as usize),
            index @ 187..=194 => Some((index - 187) as usize),
            index @ 195..=198 => Some((index - 195) as usize),
            _ => Some(0),
        }
    }

    pub fn normalize_smoke_pixel(&self, pixel: [u8; 4]) -> Option<Color> {
        let phase = smoke_marker_phase(pixel)?;
        Some(smoke_cycle_color(phase, 0, pixel[3]))
    }

    fn traffic_mask_pixel(&self, pixel: [u8; 4], anim_frame: u8, body: Color) -> Option<Color> {
        if pixel[3] == 0 {
            return None;
        }
        let index = self.nearest_index(pixel)?;
        if !is_traffic_palette_index(index) {
            return None;
        }
        let animated = self.color_for_index(index, anim_frame);
        if is_traffic_drop_color(animated) {
            return None;
        }
        Some(body)
    }

    fn traffic_overlay_mask_pixel(
        &self,
        pixel: [u8; 4],
        anim_frame: u8,
        body: Color,
    ) -> Option<Color> {
        if pixel[3] == 0 || is_lane_dash_anchor_pixel(pixel) {
            return None;
        }
        let index = nearest_traffic_palette_index(pixel)?;
        if !traffic_overlay_phase_visible(index, anim_frame) {
            return None;
        }
        if (163..=170).contains(&index) {
            Some(Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            })
        } else {
            Some(body)
        }
    }

    fn traffic_car_mask_pixel(&self, pixel: [u8; 4], body: Color) -> Option<Color> {
        if pixel[3] == 0 || is_lane_dash_anchor_pixel(pixel) {
            return None;
        }
        if let Some(index) = nearest_traffic_palette_index(pixel) {
            return traffic_car_color_for_mask_index(index, body);
        }
        if matches!(pixel, [115, 115, 115, _]) {
            return Some(Color {
                r: 128,
                g: 128,
                b: 128,
                a: pixel[3],
            });
        }
        None
    }

    fn build_animation_tables(&mut self) {
        for index in 0..256u16 {
            let count = frame_count_from_index(index as u8);
            self.frame_count[index as usize] = count;
            for frame in 0..count as usize {
                self.frames[index as usize][frame] = self.primary[index as usize];
            }
        }

        self.apply_cycle(171, 178, &[171, 178, 177, 176, 175, 174, 173, 172]);
        self.apply_cycle(179, 186, &[179, 186, 185, 184, 183, 182, 181, 180]);
        self.apply_cycle(187, 194, &[187, 194, 193, 192, 191, 190, 189, 188]);
        self.apply_cycle(195, 198, &[195, 198, 197, 196]);
        self.apply_cycle(
            200,
            211,
            &[200, 211, 210, 209, 208, 207, 206, 205, 204, 203, 202, 201],
        );
        self.apply_cycle(212, 219, &[212, 219, 218, 217, 216, 215, 214, 213]);
        self.apply_cycle(220, 223, &[220, 220, 220, 220]);
        self.apply_cycle(224, 225, &[224, 224, 225, 225]);
        self.apply_cycle(226, 227, &[226, 226, 227, 227]);
        self.apply_cycle(228, 229, &[228, 228, 229, 229]);
        self.apply_cycle(230, 231, &[230, 230, 231, 231]);

        self.global_cycle = 24;
    }

    fn apply_cycle(&mut self, start: u8, end: u8, order: &[u8]) {
        let order_len = order.len().max(1);
        for index in start..=end {
            let count = self.frame_count[index as usize];
            let base_offset = (order_len + start as usize - index as usize) % order_len;
            for frame in 0..count as usize {
                let cycle_index = (frame + base_offset) % order_len;
                let mapped = order[cycle_index];
                self.frames[index as usize][frame] = self.primary[mapped as usize];
            }
        }
    }
}

/// Mac traffic overlay sprites: keep only vehicle marks for the separate overlay pass.
pub fn bake_lane_dash_sprite(pixels: &mut RgbaImage) {
    let source = pixels.clone();
    for y in 0..source.height() {
        for x in 0..source.width() {
            let pixel = source.get_pixel(x, y).0;
            if pixel[3] == 0 || is_lane_dash_anchor_pixel(pixel) {
                pixels.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                continue;
            }
            let color = nearest_traffic_palette_index(pixel)
                .and_then(traffic_overlay_color_for_mask_index)
                .unwrap_or([0, 0, 0, 0]);
            pixels.put_pixel(x, y, image::Rgba(color));
        }
    }
}

pub fn animated_sprite_id(base_id: u32, frame: u8) -> u32 {
    if frame == 0 {
        base_id
    } else {
        base_id * ANIM_SPRITE_STRIDE + frame as u32
    }
}

pub fn is_water_sprite(sprite_id: u32) -> bool {
    (1270..=1289).contains(&sprite_id)
}

pub fn is_smoke_sprite(sprite_id: u32) -> bool {
    matches!(sprite_id, 1201 | 1202 | 1207 | 1250)
}

pub fn is_smoke_building(building: u8) -> bool {
    matches!(building, 201 | 202 | 207 | 250)
}

pub fn is_traffic_sprite(sprite_id: u32) -> bool {
    (400..=447).contains(&sprite_id)
        || (900..=947).contains(&sprite_id)
        || (1400..=1449).contains(&sprite_id)
}

pub fn traffic_animation_frame_count() -> u8 {
    8
}

pub const TRAFFIC_ANIMATION_FPS: f32 = 6.0;

pub fn should_animate_sprite(sprite_id: u32) -> bool {
    sprite_id == 1005 || is_water_sprite(sprite_id) || is_smoke_sprite(sprite_id)
}

pub fn smoke_plume_rows(sprite_id: u32, sprite_height: u32) -> u32 {
    if !is_smoke_sprite(sprite_id) {
        return 0;
    }
    match sprite_id {
        1201 => 20,
        1202 => 48,
        1207 => 32,
        1250 => 13,
        _ => sprite_height / 3,
    }
    .min(sprite_height)
}

pub fn is_smoke_plume_pixel(
    sprite_id: u32,
    _sprite_width: u32,
    sprite_height: u32,
    _x: u32,
    y: u32,
    pixel: [u8; 4],
) -> bool {
    is_smoke_sprite(sprite_id)
        && y < smoke_plume_rows(sprite_id, sprite_height)
        && is_smoke_animation_pixel(pixel)
}

/// Mac animated CLUT 500 rows 33..40. Water sprites carry marker pixels from
/// default palette rows 171–178 / 187–198; the game turns those slots into this
/// light-blue cycle via AnimatePalette. Smoke uses rows 41..48 of the same CLUT.
const WATER_CLUT_CYCLE: [[u8; 4]; 8] = [
    [99, 155, 255, 255],
    [92, 119, 255, 255],
    [73, 69, 255, 255],
    [39, 32, 255, 255],
    [49, 62, 255, 255],
    [35, 39, 255, 255],
    [35, 39, 255, 255],
    [35, 39, 255, 255],
];

fn is_water_animation_pixel(pixel: [u8; 4]) -> bool {
    if is_shore_terrain_color(pixel) {
        return false;
    }
    is_water_overlay_pixel(pixel)
}

fn is_water_overlay_pixel(pixel: [u8; 4]) -> bool {
    is_water_animation_green(pixel)
        || is_water_animation_brown(pixel)
        || is_bright_water_blue(pixel)
        || is_dark_water_blue(pixel)
}

fn is_shore_terrain_color(pixel: [u8; 4]) -> bool {
    pixel[0] >= 110 && pixel[1] >= 80 && pixel[1] <= 160 && pixel[2] <= 95
}

fn is_bright_water_blue(pixel: [u8; 4]) -> bool {
    pixel[2] > 180 && pixel[0] < 80 && (60..=140).contains(&pixel[1])
}

fn is_dark_water_blue(pixel: [u8; 4]) -> bool {
    pixel[2] > 80 && pixel[0] < 30 && pixel[1] < 55
}

fn is_water_animation_green(pixel: [u8; 4]) -> bool {
    pixel[1] > 120 && pixel[0] < 90 && pixel[2] < 90
}

fn is_water_animation_brown(pixel: [u8; 4]) -> bool {
    pixel[0] <= 90 && pixel[1] <= 70 && pixel[2] <= 50
}

/// Mac traffic body colors (blue, red, white, gray).
pub const TRAFFIC_CAR_COLOR_INDICES: [u8; 4] = [81, 12, 138, 139];

pub const TRAFFIC_VARIANT_STRIDE: u32 = 50_000;

pub fn traffic_color_slot(seed: usize) -> u8 {
    match seed % 5 {
        0..=3 => 0,
        _ => (1 + seed % (TRAFFIC_CAR_COLOR_INDICES.len() - 1)) as u8,
    }
}

pub fn traffic_animation_band_count(pixels: &RgbaImage) -> u8 {
    let counts: Vec<u8> = pixels
        .pixels()
        .filter_map(|pixel| {
            let pixel = pixel.0;
            if pixel[3] == 0 {
                return None;
            }
            let index = nearest_traffic_palette_index(pixel)?;
            Some(frame_count_from_index(index))
        })
        .collect();
    lcm_counts(&counts).max(1)
}

pub fn traffic_color_index_for_slot(slot: u8) -> u8 {
    TRAFFIC_CAR_COLOR_INDICES[slot as usize % TRAFFIC_CAR_COLOR_INDICES.len()]
}

pub fn traffic_colored_sprite_id(base_id: u32, color_slot: u8) -> u32 {
    if color_slot == 0 {
        base_id
    } else {
        base_id + TRAFFIC_VARIANT_STRIDE + color_slot as u32
    }
}

fn is_traffic_palette_index(index: u8) -> bool {
    (155..=178).contains(&index)
}

fn nearest_traffic_palette_index(pixel: [u8; 4]) -> Option<u8> {
    if pixel[3] == 0 {
        return None;
    }
    TRAFFIC_MASK_PALETTE
        .iter()
        .copied()
        .min_by_key(|&(_, color)| {
            let dr = pixel[0] as i32 - color.r as i32;
            let dg = pixel[1] as i32 - color.g as i32;
            let db = pixel[2] as i32 - color.b as i32;
            dr * dr + dg * dg + db * db
        })
        .and_then(|(index, color)| {
            let dr = pixel[0] as i32 - color.r as i32;
            let dg = pixel[1] as i32 - color.g as i32;
            let db = pixel[2] as i32 - color.b as i32;
            if dr * dr + dg * dg + db * db <= 60 * 60 * 3 {
                Some(index)
            } else {
                None
            }
        })
}

fn is_traffic_drop_color(color: Color) -> bool {
    matches!((color.r, color.g, color.b), (255, 181, 255) | (0, 227, 48))
}

fn is_lane_dash_anchor_pixel(pixel: [u8; 4]) -> bool {
    matches!(pixel, [115, 83, 35, _])
}

fn traffic_overlay_color_for_mask_index(index: u8) -> Option<[u8; 4]> {
    match index {
        // The pale half of the traffic mask ramp is coverage/dropout data. Rendering it as
        // vehicle body creates the long solid blue bars seen in the viewer at medium/large banks.
        155..=162 => None,
        163..=165 => Some([232, 238, 255, 255]),
        166..=170 => Some([0, 0, 168, 255]),
        _ => None,
    }
}

fn traffic_overlay_phase_visible(index: u8, anim_frame: u8) -> bool {
    // These canonical mask colors retain Win95 palette indices 171..194. Its first
    // two eight-slot cycles have two vehicle/shadow pixels followed by six road pixels;
    // the heavy cycle has four vehicle pixels followed by four road pixels. Road pixels
    // become transparent here so the same traffic works over either graphics preference.
    let (slot, length) = match index {
        155..=162 => (index - 155, 2),
        163..=170 => (index - 163, 2),
        171..=178 => (index - 171, 4),
        _ => return false,
    };
    let phase = anim_frame & 7;
    slot.wrapping_sub(phase) & 7 < length
}

/// Lossless interchange colors for the original Win95 traffic palette cycles.
/// They encode the original index rather than a displayed color, which is shared by
/// several indices in PAL_MSTR and cannot be used to reconstruct animation later.
pub(crate) fn traffic_mask_color(win95_index: u8) -> Option<[u8; 3]> {
    let slot = win95_index.checked_sub(171)? as usize;
    TRAFFIC_MASK_PALETTE
        .get(slot)
        .map(|(_, color)| [color.r, color.g, color.b])
}

pub(crate) fn has_traffic_mask_pixels(image: &RgbaImage) -> bool {
    image.pixels().any(|pixel| {
        pixel[3] != 0
            && TRAFFIC_MASK_PALETTE
                .iter()
                .any(|(_, color)| pixel.0[..3] == [color.r, color.g, color.b])
    })
}
fn traffic_car_color_for_mask_index(index: u8, body: Color) -> Option<Color> {
    if !is_traffic_palette_index(index) {
        return None;
    }
    let primary = TRAFFIC_MASK_PALETTE
        .iter()
        .find(|(mask_index, _)| *mask_index == index)
        .map(|(_, color)| *color)?;
    if is_traffic_drop_color(primary) || index >= 171 {
        return None;
    }
    if (163..=165).contains(&index) {
        Some(Color {
            r: 232,
            g: 238,
            b: 255,
            a: 255,
        })
    } else {
        Some(body)
    }
}

const TRAFFIC_MASK_PALETTE: [(u8, Color); 24] = [
    (
        155,
        Color {
            r: 255,
            g: 0,
            b: 179,
            a: 255,
        },
    ),
    (
        156,
        Color {
            r: 255,
            g: 25,
            b: 255,
            a: 255,
        },
    ),
    (
        157,
        Color {
            r: 255,
            g: 51,
            b: 255,
            a: 255,
        },
    ),
    (
        158,
        Color {
            r: 255,
            g: 103,
            b: 255,
            a: 255,
        },
    ),
    (
        159,
        Color {
            r: 255,
            g: 127,
            b: 255,
            a: 255,
        },
    ),
    (
        160,
        Color {
            r: 255,
            g: 156,
            b: 255,
            a: 255,
        },
    ),
    (
        161,
        Color {
            r: 255,
            g: 181,
            b: 255,
            a: 255,
        },
    ),
    (
        162,
        Color {
            r: 255,
            g: 207,
            b: 255,
            a: 255,
        },
    ),
    (
        163,
        Color {
            r: 232,
            g: 0,
            b: 255,
            a: 255,
        },
    ),
    (
        164,
        Color {
            r: 207,
            g: 0,
            b: 255,
            a: 255,
        },
    ),
    (
        165,
        Color {
            r: 181,
            g: 0,
            b: 255,
            a: 255,
        },
    ),
    (
        166,
        Color {
            r: 156,
            g: 0,
            b: 255,
            a: 255,
        },
    ),
    (
        167,
        Color {
            r: 127,
            g: 0,
            b: 255,
            a: 255,
        },
    ),
    (
        168,
        Color {
            r: 103,
            g: 0,
            b: 255,
            a: 255,
        },
    ),
    (
        169,
        Color {
            r: 76,
            g: 0,
            b: 255,
            a: 255,
        },
    ),
    (
        170,
        Color {
            r: 53,
            g: 0,
            b: 255,
            a: 255,
        },
    ),
    (
        171,
        Color {
            r: 0,
            g: 76,
            b: 25,
            a: 255,
        },
    ),
    (
        172,
        Color {
            r: 0,
            g: 99,
            b: 25,
            a: 255,
        },
    ),
    (
        173,
        Color {
            r: 0,
            g: 127,
            b: 25,
            a: 255,
        },
    ),
    (
        174,
        Color {
            r: 0,
            g: 151,
            b: 23,
            a: 255,
        },
    ),
    (
        175,
        Color {
            r: 0,
            g: 179,
            b: 25,
            a: 255,
        },
    ),
    (
        176,
        Color {
            r: 0,
            g: 207,
            b: 23,
            a: 255,
        },
    ),
    (
        177,
        Color {
            r: 0,
            g: 227,
            b: 48,
            a: 255,
        },
    ),
    (
        178,
        Color {
            r: 0,
            g: 255,
            b: 25,
            a: 255,
        },
    ),
];

fn is_smoke_animation_pixel(pixel: [u8; 4]) -> bool {
    smoke_marker_phase(pixel).is_some()
}

fn water_marker_phase_exact(pixel: [u8; 4]) -> Option<usize> {
    if pixel[3] == 0 {
        return None;
    }
    match (pixel[0], pixel[1], pixel[2]) {
        (28, 28, 0) | (0, 76, 25) | (53, 28, 0) => Some(0),
        (28, 53, 0) | (0, 99, 25) | (53, 53, 0) => Some(1),
        (28, 79, 0) | (0, 127, 25) | (53, 79, 0) => Some(2),
        (28, 104, 0) | (0, 151, 23) | (53, 104, 0) => Some(3),
        (28, 130, 0) | (0, 179, 25) => Some(4),
        (28, 156, 0) | (0, 207, 23) => Some(5),
        (28, 181, 0) | (0, 227, 48) => Some(6),
        (28, 207, 0) | (0, 255, 25) => Some(7),
        _ => None,
    }
}

fn water_cycle_color(phase: usize, anim_frame: u8, alpha: u8) -> Color {
    let color = WATER_CLUT_CYCLE[(anim_frame as usize + phase) % WATER_CLUT_CYCLE.len()];
    Color {
        r: color[0],
        g: color[1],
        b: color[2],
        a: ((alpha as u16 * color[3] as u16) / 255) as u8,
    }
}

/// Mac animated CLUT 500 rows 41..48. Smoke sprites carry marker pixels
/// from default palette rows 195..203 in the extracted PNGs; the game turns
/// those palette slots into brown/gray smoke colors at runtime via AnimatePalette.
const SMOKE_CLUT_CYCLE: [[u8; 4]; 8] = [
    [155, 135, 71, 255],
    [128, 69, 19, 255],
    [112, 60, 17, 255],
    [80, 43, 12, 255],
    [155, 135, 71, 255],
    [80, 74, 67, 255],
    [64, 62, 62, 255],
    [155, 135, 71, 255],
];

fn smoke_marker_phase(pixel: [u8; 4]) -> Option<usize> {
    if pixel[3] == 0 || pixel[2] != 0 {
        return None;
    }

    let phase = match (pixel[0], pixel[1]) {
        (53, 28) | (79, 28) => 0,
        (53, 53) | (7, 67) => 1,
        (53, 79) | (7, 91) => 2,
        (53, 104) | (7, 115) => 3,
        (53, 130) | (7, 139) => 4,
        (53, 156) | (7, 167) => 5,
        (53, 181) | (7, 191) => 6,
        (53, 207) => 7,
        _ => return None,
    };
    Some(phase)
}

fn smoke_cycle_color(phase: usize, anim_frame: u8, alpha: u8) -> Color {
    let color = SMOKE_CLUT_CYCLE[(anim_frame as usize + phase) % SMOKE_CLUT_CYCLE.len()];
    Color {
        r: color[0],
        g: color[1],
        b: color[2],
        a: ((alpha as u16 * color[3] as u16) / 255) as u8,
    }
}

fn parse_default_palette(path: &Path) -> Result<[Color; 256]> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut colors = [Color::TRANSPARENT; 256];
    for line in text.lines().skip(1) {
        let mut parts = line.split(',');
        let index = parts
            .next()
            .and_then(|value| value.parse::<usize>().ok())
            .context("palette index")?;
        let r = parts
            .next()
            .and_then(|v| v.parse().ok())
            .context("palette r")?;
        let g = parts
            .next()
            .and_then(|v| v.parse().ok())
            .context("palette g")?;
        let b = parts
            .next()
            .and_then(|v| v.parse().ok())
            .context("palette b")?;
        if index < 256 {
            colors[index] = Color { r, g, b, a: 255 };
        }
    }
    Ok(colors)
}

fn parse_palette_image(path: &Path) -> Result<[Color; 256]> {
    let image = image::open(path)
        .with_context(|| format!("reading {}", path.display()))?
        .to_rgba8();
    ensure_palette_grid(&image, path)?;
    Ok(std::array::from_fn(|index| {
        let column = (index % 16) as u32;
        let row = (index / 16) as u32;
        let pixel = image.get_pixel(column * 18 + 9, row * 18 + 9).0;
        Color {
            r: pixel[0],
            g: pixel[1],
            b: pixel[2],
            a: 255,
        }
    }))
}

fn ensure_palette_grid(image: &RgbaImage, path: &Path) -> Result<()> {
    anyhow::ensure!(
        image.width() >= 288 && image.height() >= 288,
        "palette image {} must contain a 16 by 16 grid of 18-pixel cells",
        path.display()
    );
    Ok(())
}

pub fn frame_count_from_index(color: u8) -> u8 {
    if (200..=211).contains(&color) {
        return 12;
    }
    if matches!(
        color,
        171 | 172
            | 173
            | 174
            | 175
            | 176
            | 177
            | 178
            | 179
            | 180
            | 181
            | 182
            | 183
            | 184
            | 185
            | 186
            | 187
            | 188
            | 189
            | 190
            | 191
            | 192
            | 193
            | 194
            | 212
            | 213
            | 214
            | 215
            | 216
            | 217
            | 218
            | 219
    ) {
        return 8;
    }
    if matches!(
        color,
        195 | 196
            | 197
            | 198
            | 220
            | 221
            | 222
            | 223
            | 224
            | 225
            | 226
            | 227
            | 228
            | 229
            | 230
            | 231
    ) {
        return 4;
    }
    1
}

fn lcm_counts(counts: &[u8]) -> u8 {
    counts.iter().copied().fold(1u8, lcm)
}

fn lcm(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 1;
    }
    (a as u16 * b as u16 / gcd(a, b) as u16) as u8
}

fn gcd(mut a: u8, mut b: u8) -> u8 {
    while b != 0 {
        let temp = b;
        b = a % b;
        a = temp;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_assets() -> Option<(std::path::PathBuf, AnimatedPalette)> {
        let root = crate::paths::test_asset_root()?;
        let palette = AnimatedPalette::load(&root).ok()?;
        Some((root, palette))
    }

    fn test_palette() -> Option<AnimatedPalette> {
        test_assets().map(|(_, palette)| palette)
    }

    #[test]
    fn frame_count_matches_opensc2k_groups() {
        assert_eq!(frame_count_from_index(171), 8);
        assert_eq!(frame_count_from_index(195), 4);
        assert_eq!(frame_count_from_index(200), 12);
        assert_eq!(frame_count_from_index(64), 1);
    }

    #[test]
    fn reads_pc_export_palette_grid() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("sc2k-palette-{nonce}"));
        fs::create_dir_all(root.join("palettes")).unwrap();
        let mut grid = RgbaImage::from_pixel(288, 288, image::Rgba([0, 0, 0, 255]));
        for y in 0..18 {
            for x in 18..36 {
                grid.put_pixel(x, y, image::Rgba([10, 20, 30, 255]));
            }
        }
        grid.save(root.join("palettes/win95_pal_mstr.png")).unwrap();
        let palette = AnimatedPalette::load(&root).unwrap();
        assert_eq!(
            palette.color_for_index(1, 0),
            Color {
                r: 10,
                g: 20,
                b: 30,
                a: 255
            }
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn water_palette_frames_differ() {
        let Some(palette) = test_palette() else {
            return;
        };
        let frame0 = palette.color_for_index(171, 0);
        let frame4 = palette.color_for_index(171, 4);
        assert_ne!(frame0, frame4);
    }

    #[test]
    fn bakes_water_pixels_to_different_frames() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut frame0 = RgbaImage::from_pixel(1, 1, image::Rgba([0, 180, 25, 255]));
        let mut frame4 = frame0.clone();
        palette.bake_water_sprite_frame(&mut frame0, 0);
        palette.bake_water_sprite_frame(&mut frame4, 1);
        let p0 = frame0.get_pixel(0, 0).0;
        let p1 = frame4.get_pixel(0, 0).0;
        assert_eq!(p0[2], 255, "marker pixels bake to water CLUT blues: {p0:?}");
        assert_ne!(p0, p1);
        assert_ne!(p0[0..3], [0, 180, 25]);
    }

    #[test]
    fn bakes_green_water_markers_to_animated_clut_not_navy() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(1, 1, image::Rgba([28, 207, 0, 255]));
        palette.bake_water_sprite_frame(&mut image, 0);
        let pixel = image.get_pixel(0, 0).0;
        let expected = WATER_CLUT_CYCLE[7];
        assert_eq!(pixel, expected);
        assert_ne!(pixel[0..3], [0, 0, 167], "must not snap to navy 128");
    }

    #[test]
    fn bakes_brown_water_marker_to_animated_clut() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(1, 1, image::Rgba([53, 28, 0, 255]));
        palette.bake_water_sprite_frame(&mut image, 0);
        let pixel = image.get_pixel(0, 0).0;
        assert_eq!(pixel, WATER_CLUT_CYCLE[0]);
    }

    #[test]
    fn open_water_body_keeps_original_sc2k_blue() {
        // Win95 LARGE.DAT water (and Mac overlay fill) is palette 79 = rgb(35,39,255).
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(2, 1, image::Rgba([35, 39, 255, 255]));
        palette.bake_water_sprite_frame(&mut image, 0);
        palette.bake_water_sprite_frame(&mut image, 3);
        for x in 0..2 {
            assert_eq!(image.get_pixel(x, 0).0, [35, 39, 255, 255]);
        }
    }

    #[test]
    fn water_clut_cycle_matches_win95_sparkle() {
        assert!(
            WATER_CLUT_CYCLE.contains(&[49, 62, 255, 255]),
            "Win95 water tile 1270 sparkle rgb(49,62,255) is CLUT 500 row 37"
        );
        assert!(WATER_CLUT_CYCLE.contains(&[35, 39, 255, 255]));
    }

    #[test]
    fn shore_pixels_in_water_shape_become_transparent() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(1, 1, image::Rgba([115, 83, 35, 255]));
        palette.bake_water_sprite_frame(&mut image, 0);
        assert_eq!(image.get_pixel(0, 0).0[3], 0);
    }

    #[test]
    fn animated_sprite_id_encoding() {
        assert_eq!(animated_sprite_id(1270, 0), 1270);
        assert_eq!(animated_sprite_id(1270, 3), 127003);
    }

    #[test]
    fn water_and_smoke_sprites_are_atlas_baked() {
        assert!(should_animate_sprite(1270));
        assert!(should_animate_sprite(1005));
        assert!(should_animate_sprite(1201));
        for sprite_id in [1201, 1202, 1207, 1250] {
            assert!(
                should_animate_sprite(sprite_id),
                "smoke sprite {sprite_id} should be baked into animated atlas frames"
            );
        }
        assert!(
            !should_animate_sprite(1220),
            "water pump sprite 1220 has no smokestack plume markers"
        );
        for sprite_id in [1160, 1162, 1163, 1164, 1165, 1189, 1191] {
            assert!(
                !should_animate_sprite(sprite_id),
                "RCI growth sprite {sprite_id} must not be frame-remapped as smoke"
            );
        }
        assert!(
            !should_animate_sprite(1135),
            "sprite 1135 has no extracted smoke marker pixels"
        );
        assert!(!should_animate_sprite(400));
    }

    #[test]
    fn smoke_building_metadata_matches_animated_building_sprites() {
        for building in 1..=u8::MAX {
            let sprite_id = 1000 + building as u32;
            assert_eq!(
                is_smoke_building(building),
                is_smoke_sprite(sprite_id),
                "building {building} sprite {sprite_id} smoke classification should match"
            );
        }
    }

    #[test]
    fn smoke_plume_row_limits_cover_known_marker_bands() {
        let expected = [(1201, 20), (1202, 48), (1207, 32), (1250, 13)];
        for (sprite_id, rows) in expected {
            assert_eq!(
                smoke_plume_rows(sprite_id, 200),
                rows,
                "smoke sprite {sprite_id} should only recolor plume rows"
            );
        }
        assert_eq!(smoke_plume_rows(1220, 200), 0);
    }

    #[test]
    fn smoke_marker_pixels_are_limited_to_plume_band() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(2, 40, image::Rgba([0, 0, 0, 0]));
        image.put_pixel(0, 0, image::Rgba([53, 156, 0, 255]));
        image.put_pixel(0, 39, image::Rgba([53, 156, 0, 255]));
        image.put_pixel(1, 0, image::Rgba([120, 118, 80, 255]));

        palette.bake_smoke_sprite_frame(&mut image, 1201, 0);

        assert_eq!(
            image.get_pixel(0, 0).0[0..3],
            [80, 74, 67],
            "smoke marker pixels inside the plume band should bake to Mac smoke CLUT colors"
        );
        assert_eq!(
            image.get_pixel(0, 39).0[0..3],
            [53, 156, 0],
            "marker-like pixels outside the plume band should stay stable"
        );
        assert_eq!(
            image.get_pixel(1, 0).0[0..3],
            [120, 118, 80],
            "ordinary gray/brown stack pixels should not use smoke fallback animation"
        );
    }

    #[test]
    fn smoke_animation_keeps_building_body_pixels_stable() {
        let Some(palette) = test_palette() else {
            return;
        };
        let Some(asset_root) = crate::paths::test_asset_root() else {
            return;
        };
        let sprite_path = asset_root
            .as_path()
            .join("sprites/SPRT_00129_ShapeData/individual/sprite_0035_id_01201_128x86.png");
        if !sprite_path.exists() {
            return;
        }
        let base = image::open(&sprite_path).expect("smoke sprite").to_rgba8();
        let plume_rows = smoke_plume_rows(1201, base.height());
        let mut frame0 = base.clone();
        let mut frame3 = base;
        palette.bake_smoke_sprite_frame(&mut frame0, 1201, 0);
        palette.bake_smoke_sprite_frame(&mut frame3, 1201, 3);

        for y in plume_rows..frame0.height() {
            for x in 0..frame0.width() {
                assert_eq!(
                    frame0.get_pixel(x, y),
                    frame3.get_pixel(x, y),
                    "non-plume pixel changed at ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn coal_power_footprint_marker_pixels_stay_stable() {
        let Some(palette) = test_palette() else {
            return;
        };
        let Some(asset_root) = crate::paths::test_asset_root() else {
            return;
        };
        let sprite_path = asset_root
            .as_path()
            .join("sprites/SPRT_00129_ShapeData/individual/sprite_0042_id_01207_128x106.png");
        if !sprite_path.exists() {
            return;
        }
        let base = image::open(&sprite_path)
            .expect("coal power sprite")
            .to_rgba8();
        let plume_rows = smoke_plume_rows(1207, base.height());
        let mut frame0 = base.clone();
        let mut frame2 = base.clone();
        palette.bake_smoke_sprite_frame(&mut frame0, 1207, 0);
        palette.bake_smoke_sprite_frame(&mut frame2, 1207, 2);

        let mut checked_marker_pixels = 0usize;
        for y in plume_rows..base.height() {
            for x in 0..base.width() {
                let pixel = base.get_pixel(x, y).0;
                if is_smoke_animation_pixel(pixel) {
                    checked_marker_pixels += 1;
                    assert_eq!(
                        frame0.get_pixel(x, y),
                        base.get_pixel(x, y),
                        "coal non-plume marker pixel changed in frame 0 at ({x},{y})"
                    );
                    assert_eq!(
                        frame2.get_pixel(x, y),
                        base.get_pixel(x, y),
                        "coal non-plume marker pixel changed in frame 2 at ({x},{y})"
                    );
                }
            }
        }
        assert!(
            checked_marker_pixels > 0,
            "coal sprite should contain marker-colored non-smoke footprint pixels"
        );
    }

    #[test]
    fn coal_power_plume_marker_pixels_animate() {
        let Some(palette) = test_palette() else {
            return;
        };
        let Some(asset_root) = crate::paths::test_asset_root() else {
            return;
        };
        let sprite_path = asset_root
            .as_path()
            .join("sprites/SPRT_00129_ShapeData/individual/sprite_0042_id_01207_128x106.png");
        if !sprite_path.exists() {
            return;
        }
        let base = image::open(&sprite_path)
            .expect("coal power sprite")
            .to_rgba8();
        let plume_rows = smoke_plume_rows(1207, base.height());
        let mut frame0 = base.clone();
        let mut frame2 = base;
        palette.bake_smoke_sprite_frame(&mut frame0, 1207, 0);
        palette.bake_smoke_sprite_frame(&mut frame2, 1207, 2);

        let mut animated_plume_pixels = 0usize;
        for y in 0..plume_rows {
            for x in 0..frame0.width() {
                let p0 = frame0.get_pixel(x, y).0;
                let p2 = frame2.get_pixel(x, y).0;
                if p0[3] > 0 && p0 != p2 {
                    animated_plume_pixels += 1;
                }
            }
        }
        assert!(
            animated_plume_pixels > 0,
            "coal smoke plume should animate without swapping the building sprite"
        );
    }

    #[test]
    fn bakes_traffic_sprite_to_mac_sized_dash() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(8, 4, image::Rgba([0, 0, 0, 0]));
        for x in 3..7 {
            image.put_pixel(x, 0, image::Rgba([207, 0, 255, 255]));
        }
        palette.bake_traffic_sprite_frame(&mut image, 81, 0);
        let baked_opaque = image.pixels().filter(|p| p.0[3] > 0).count();
        assert_eq!(baked_opaque, 4);
        assert_eq!(image.get_pixel(3, 0).0[0..3], [0, 0, 168]);
        assert_eq!(image.get_pixel(3, 1).0[3], 0);
    }

    #[test]
    fn traffic_animation_count_matches_palette_cycle() {
        let Some(asset_root) = crate::paths::test_asset_root() else {
            return;
        };
        let light = asset_root
            .join("sprites/SPRT_00129_ShapeData/individual/sprite_0288_id_01400_32x17.png");
        let heavy = asset_root
            .join("sprites/SPRT_00129_ShapeData/individual/sprite_0315_id_01427_32x17.png");
        if !light.as_path().exists() || !heavy.as_path().exists() {
            return;
        }
        let light = image::open(&light).expect("light traffic").to_rgba8();
        let heavy = image::open(&heavy).expect("heavy traffic").to_rgba8();
        assert_eq!(traffic_animation_band_count(&light), 1);
        assert_eq!(traffic_animation_band_count(&heavy), 8);
    }

    #[test]
    fn bakes_traffic_lane_dash_row() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(8, 4, image::Rgba([0, 0, 0, 0]));
        for x in 3..7 {
            image.put_pixel(x, 0, image::Rgba([207, 0, 255, 255]));
        }
        palette.bake_traffic_sprite_frame(&mut image, 81, 0);
        let baked_opaque = image.pixels().filter(|p| p.0[3] > 0).count();
        assert_eq!(baked_opaque, 4);
        assert_eq!(image.get_pixel(0, 0).0[3], 0);
    }

    #[test]
    fn traffic_overlay_moves_light_and_heavy_marks_through_all_eight_positions() {
        let palette = AnimatedPalette::load("/nonexistent/citygame-test-palette").unwrap();
        let mut mask = RgbaImage::new(24, 1);
        for (x, (_, color)) in TRAFFIC_MASK_PALETTE.iter().enumerate() {
            mask.put_pixel(x as u32, 0, image::Rgba(color.to_rgba()));
        }
        for phase in 0..8 {
            let mut image = mask.clone();
            palette.bake_traffic_overlay_sprite_frame(&mut image, 81, phase);
            for (first, length) in [(0, 2), (8, 2), (16, 4)] {
                let mut expected: Vec<u32> = (0..length)
                    .map(|offset| first + u32::from((phase + offset) % 8))
                    .collect();
                expected.sort_unstable();
                let visible: Vec<u32> = (first..first + 8)
                    .filter(|x| image.get_pixel(*x, 0).0[3] > 0)
                    .collect();
                assert_eq!(visible, expected, "ramp {first} at phase {phase}");
            }
        }
    }

    #[test]
    fn traffic_overlay_bake_separates_shadow_from_blue_vehicle_marks() {
        let palette = AnimatedPalette::load("/nonexistent/citygame-test-palette").unwrap();
        let mut image = RgbaImage::new(8, 1);
        for (x, (_, color)) in TRAFFIC_MASK_PALETTE.iter().skip(8).take(8).enumerate() {
            image.put_pixel(x as u32, 0, image::Rgba(color.to_rgba()));
        }

        palette.bake_traffic_overlay_sprite_frame(&mut image, 81, 0);

        let visible: Vec<usize> = (0..8)
            .filter(|x| image.get_pixel(*x as u32, 0).0[3] > 0)
            .collect();
        assert_eq!(visible, vec![0, 1]);
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [0, 0, 0, 255]);
        assert_eq!(image.get_pixel(7, 0).0, [0, 0, 0, 0]);
    }

    #[test]
    fn bakes_traffic_car_shape_with_highlight_and_body() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(3, 1, image::Rgba([0, 0, 0, 0]));
        image.put_pixel(0, 0, image::Rgba([207, 0, 255, 255])); // mask index 164 highlight
        image.put_pixel(1, 0, image::Rgba([53, 0, 255, 255])); // mask index 170 body
        image.put_pixel(2, 0, image::Rgba([115, 83, 35, 255])); // anchor → transparent
        palette.bake_traffic_car_sprite(&mut image, 81, 0);
        assert_eq!(image.get_pixel(0, 0).0[0..3], [232, 238, 255]);
        assert_eq!(image.get_pixel(1, 0).0[0..3], [0, 0, 168]);
        assert_eq!(image.get_pixel(2, 0).0[3], 0);
    }

    #[test]
    fn lane_dash_bake_drops_pale_mask_ramp_and_keeps_vehicle_pixels() {
        let mut image = RgbaImage::from_pixel(4, 1, image::Rgba([0, 0, 0, 0]));
        image.put_pixel(0, 0, image::Rgba([255, 25, 255, 255])); // mask index 156 coverage/drop
        image.put_pixel(1, 0, image::Rgba([255, 207, 255, 255])); // mask index 162 coverage/drop
        image.put_pixel(2, 0, image::Rgba([232, 0, 255, 255])); // mask index 163 highlight
        image.put_pixel(3, 0, image::Rgba([115, 83, 35, 255])); // anchor/dropout

        bake_lane_dash_sprite(&mut image);

        assert_eq!(image.get_pixel(0, 0).0[3], 0);
        assert_eq!(image.get_pixel(1, 0).0[3], 0);
        assert_eq!(image.get_pixel(2, 0).0[0..3], [232, 238, 255]);
        assert_eq!(image.get_pixel(3, 0).0[3], 0);
    }

    #[test]
    fn bakes_traffic_sprite_to_solid_blue() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(1, 1, image::Rgba([207, 0, 255, 255]));
        palette.bake_traffic_sprite_frame(&mut image, 81, 0);
        let pixel = image.get_pixel(0, 0).0;
        assert_eq!(pixel[0..3], [0, 0, 168]);
    }

    #[test]
    fn bakes_traffic_slots_to_distinct_car_colors() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut red = RgbaImage::from_pixel(1, 1, image::Rgba([207, 0, 255, 255]));
        let mut blue = RgbaImage::from_pixel(1, 1, image::Rgba([127, 0, 255, 255]));
        palette.bake_traffic_sprite_frame(&mut red, 12, 0);
        palette.bake_traffic_sprite_frame(&mut blue, 81, 0);
        assert_eq!(red.get_pixel(0, 0).0[0..3], [255, 0, 0]);
        assert_eq!(blue.get_pixel(0, 0).0[0..3], [0, 0, 168]);
    }

    #[test]
    fn traffic_colored_sprite_ids_are_unique() {
        assert_eq!(traffic_colored_sprite_id(400, 0), 400);
        assert_eq!(traffic_colored_sprite_id(400, 1), 50401);
    }

    #[test]
    fn traffic_sprite_bank_stops_before_watered_underground_tiles() {
        assert!(is_traffic_sprite(1449));
        assert!(!is_traffic_sprite(1450));
        assert!(!is_traffic_sprite(1467));
    }

    #[test]
    fn smoke_plume_bakes_to_mac_clut_not_water_blue() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(1, 1, image::Rgba([53, 156, 0, 255]));
        palette.bake_smoke_sprite_frame(&mut image, 1201, 0);
        let pixel = image.get_pixel(0, 0).0;
        assert_ne!(
            pixel[0..3],
            [35, 55, 255],
            "smoke must not become water blue"
        );
        assert_ne!(
            pixel[0..3],
            [53, 156, 0],
            "smoke marker green should not survive into the baked frame"
        );
        assert_eq!(
            pixel,
            [80, 74, 67, 255],
            "smoke should use the exact Mac animated CLUT row"
        );
    }

    #[test]
    fn smoke_marker_bands_map_to_mac_clut_phases() {
        let Some(palette) = test_palette() else {
            return;
        };
        let markers = [
            ([53, 28, 0, 255], [155, 135, 71, 255]),
            ([53, 53, 0, 255], [128, 69, 19, 255]),
            ([53, 79, 0, 255], [112, 60, 17, 255]),
            ([53, 104, 0, 255], [80, 43, 12, 255]),
            ([53, 130, 0, 255], [155, 135, 71, 255]),
            ([53, 156, 0, 255], [80, 74, 67, 255]),
            ([53, 181, 0, 255], [64, 62, 62, 255]),
            ([53, 207, 0, 255], [155, 135, 71, 255]),
        ];

        for (marker, expected) in markers {
            let mut image = RgbaImage::from_pixel(1, 1, image::Rgba(marker));
            palette.bake_smoke_sprite_frame(&mut image, 1201, 0);
            assert_eq!(
                image.get_pixel(0, 0).0,
                expected,
                "smoke marker {marker:?} should map to the Mac CLUT phase"
            );
        }
    }

    #[test]
    fn smoke_marker_phase_zero_accepts_brown_source_marker() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut image = RgbaImage::from_pixel(1, 1, image::Rgba([79, 28, 0, 255]));

        palette.bake_smoke_sprite_frame(&mut image, 1201, 0);

        assert_eq!(
            image.get_pixel(0, 0).0,
            [155, 135, 71, 255],
            "source smoke sprites use this brown palette marker for the first smoke CLUT phase"
        );
    }

    #[test]
    fn smoke_marker_plume_frames_differ() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut frame0 = RgbaImage::from_pixel(1, 1, image::Rgba([53, 104, 0, 255]));
        let mut frame1 = frame0.clone();
        palette.bake_smoke_sprite_frame(&mut frame0, 1201, 0);
        palette.bake_smoke_sprite_frame(&mut frame1, 1201, 1);
        assert_ne!(
            frame0.get_pixel(0, 0).0,
            frame1.get_pixel(0, 0).0,
            "smoke marker plume must cycle across anim frames"
        );
    }

    #[test]
    fn smoke_marker_plume_bands_cycle_out_of_phase() {
        let Some(palette) = test_palette() else {
            return;
        };
        let mut frame0 = RgbaImage::from_pixel(2, 1, image::Rgba([0, 0, 0, 0]));
        frame0.put_pixel(0, 0, image::Rgba([53, 53, 0, 255]));
        frame0.put_pixel(1, 0, image::Rgba([53, 156, 0, 255]));
        let mut frame1 = frame0.clone();

        palette.bake_smoke_sprite_frame(&mut frame0, 1201, 0);
        palette.bake_smoke_sprite_frame(&mut frame1, 1201, 1);

        assert_eq!(
            frame0.get_pixel(0, 0).0,
            [128, 69, 19, 255],
            "smoke should keep opaque indexed CLUT colors instead of alpha fading"
        );
        assert_eq!(
            frame1.get_pixel(0, 0).0,
            [112, 60, 17, 255],
            "the same source marker should advance to the next Mac CLUT row"
        );
        assert_eq!(frame0.get_pixel(1, 0).0, [80, 74, 67, 255]);
        assert_eq!(frame1.get_pixel(1, 0).0, [64, 62, 62, 255]);
        assert_ne!(
            frame0.get_pixel(0, 0).0,
            frame0.get_pixel(1, 0).0,
            "smoke marker bands should not collapse to one synchronized color"
        );
        assert_ne!(
            frame1.get_pixel(0, 0).0,
            frame1.get_pixel(1, 0).0,
            "smoke brightness bands should keep distinct palette phases"
        );
    }

    #[test]
    fn smoke_plume_animation_phases_cycle_opaque_clut_colors_to_move_plume() {
        let Some(palette) = test_palette() else {
            return;
        };
        let marker = image::Rgba([53, 79, 0, 255]);
        let mut frame0 = RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 0]));
        frame0.put_pixel(1, 4, marker);
        let mut frame6 = frame0.clone();

        palette.bake_smoke_sprite_frame(&mut frame0, 1201, 0);
        palette.bake_smoke_sprite_frame(&mut frame6, 1201, 6);

        assert_eq!(
            frame0.get_pixel(1, 4).0,
            [112, 60, 17, 255],
            "frame 0 should use the marker's Mac CLUT row"
        );
        assert_eq!(
            frame6.get_pixel(1, 4).0,
            [155, 135, 71, 255],
            "later frames should pass through the opaque Mac smoke fade row"
        );
        assert_eq!(
            frame6.get_pixel(3, 3).0[3],
            0,
            "palette animation should not draw outside the original plume marker positions"
        );
        assert_ne!(
            frame0.get_pixel(1, 4).0,
            frame6.get_pixel(1, 4).0,
            "fixed plume markers should still animate by palette visibility/color"
        );
    }

    #[test]
    fn brick_pixels_are_not_remapped() {
        let Some(palette) = test_palette() else {
            return;
        };
        let brick = [140, 72, 48, 255];
        assert!(palette.normalize_smoke_pixel(brick).is_none());
        let mut image = RgbaImage::from_pixel(1, 1, image::Rgba(brick));
        palette.bake_sprite_frame(&mut image, 0);
        assert_eq!(image.get_pixel(0, 0).0, brick);
    }
}
