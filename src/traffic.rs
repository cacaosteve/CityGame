//! Traffic art selection, saved-XTRF interpretation, and overlay planning.
//! Mac `export_80` stamping and fallback congestion state are kept in child modules.

use crate::city::{CITY_HEIGHT, CITY_TILES, CITY_WIDTH, City};
use crate::engine_fidelity::EngineFidelity;
use crate::mac_render;
use crate::tiles::rotated_building_id;
use glam::Vec2;

mod mac_export80;
mod simulation;
mod win95;

#[allow(unused_imports)]
pub use mac_export80::*;
pub use simulation::TrafficSystem;
#[allow(unused_imports)]
pub use win95::*;

pub const TRAFFIC_ANIMATION_FPS: f32 = 6.0;
pub const TRAFFIC_SIM_TICKS_PER_SECOND: f32 = 4.0;
pub const ROAD_SPRITE_W: f32 = 32.0;
pub const ROAD_SPRITE_H: f32 = 17.0;
pub const MAC_EXPORT80_DIAMOND_BASE_STEP_X: f32 = 4.0;
pub const MAC_EXPORT80_DIAMOND_BASE_STEP_Y: f32 = 2.0;
/// This viewer's road grid is the 32×17 projection, equivalent to Mac `ffffe0c9 == 2`.
pub const MAC_EXPORT80_VIEWER_PROJECTION_SHIFT: u8 = 2;

/// PE-visible congestion thresholds for road traffic overlays (`getTrafficDensity()`).
pub const TRAFFIC_LIGHT_THRESHOLD: u8 = 0x33;
pub const TRAFFIC_HEAVY_THRESHOLD: u8 = 0xC9;

/// Win95 `FUN_0043C900` draw-path bands, which are lower than the simulation/query bands.
pub const WIN95_TRAFFIC_DRAW_LIGHT_THRESHOLD: u8 = 0x1C;
pub const WIN95_TRAFFIC_DRAW_HEAVY_THRESHOLD: u8 = 0x38;

/// Mac `export_156` LARGE tile set offset (`addi.w #0x3e8`).
pub const TRAFFIC_LARGE_SPRITE_OFFSET: u32 = 1000;

/// Mac medium zoom band (`ffffe0c8 == 1`): 16px traffic art at sprite 900+.
pub const TRAFFIC_MEDIUM_SPRITE_OFFSET: u32 = 500;

/// Viewer zoom at/above which Mac `ffffe0c8 == 1` selects the 16px band (sprite 900+).
///
/// Default city view is `zoom == 2.0`, which should still use the normal Mac bank
/// (`400..403`). Promoting default view to the 16px bank creates the long double-line marks
/// visible in screenshots.
pub const TRAFFIC_MEDIUM_BAND_ZOOM: f32 = 3.0;

/// Viewer zoom at/above which Mac `ffffe0c8 == 2` selects SPRT_00129 @ 1400+ overlays.
///
/// The large traffic bank is a full-tile 32px canvas. Selecting it at normal viewer zoom makes
/// the Mac dash mask read as a road-length bar, so reserve it for deliberate close inspection.
pub const TRAFFIC_LARGE_BAND_ZOOM: f32 = 4.5;

/// Bump to the next Mac shape bank when the dash would be narrower than this on screen.
pub const TRAFFIC_MIN_SCREEN_PX: f32 = 3.0;

/// Max bank for the static Mac dash overlay.
///
/// The Mac binary's `1400..1403` rows are full-tile canvases. `mac_render` detects that shape
/// family and keeps its offset at the road origin, so the bank can be used at deliberate close
/// zoom without affecting the small/medium lane-dash path.
pub const TRAFFIC_MAX_OVERLAY_SPRITE_OFFSET: u32 = TRAFFIC_LARGE_SPRITE_OFFSET;

/// Player-visible SC2K traffic overlays are authored as full road-tile canvases in the
/// 1400+ bank. Use that bank for the shared visual path so traffic aligns with road art instead
/// of rendering the tiny 400-series mask marks as standalone stamps.
pub const TRAFFIC_ROAD_OVERLAY_SPRITE_OFFSET: u32 = TRAFFIC_LARGE_SPRITE_OFFSET;

/// Congestion cutoff used by the port-only animated road-actor experiment.
/// The matching `0xAA` comparison in PE routine `0x454152` is part of the
/// kind-2 helicopter handler, not evidence for a Win95 road-car spawn gate.
pub const PORT_TRAFFIC_ACTOR_SPAWN_THRESHOLD: u8 = 0xaa;

/// PE monthly `FUN_004358B0` visits the simulation lattice in four-tile steps.
/// This cadence is distinct from the renderer's per-tile traffic overlay pass.
pub const WIN95_TRAFFIC_MONTHLY_TILE_STRIDE: usize = 4;
pub const WIN95_TRAFFIC_MONTHLY_TILE_LIMIT: usize = 0x80;

/// Mac decorative traffic visibility (`fffffb44` / query path); below this, no dash.
pub const MAC_TRAFFIC_VISIBLE_BYTE_MIN: u8 = 0x33;

/// Query/status text band for heavy traffic (`export_156` reads `fffffb44`).
pub const MAC_TRAFFIC_HEAVY_BYTE_MIN: u8 = 0xC9;
pub const MAC_TRAFFIC_HEAVY_BYTE_MAX: u8 = 0xF0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficDensity {
    Light,
    Heavy,
}

/// Per-tile visual traffic overlay.
///
/// The active viewer path draws road-specific overlay art from saved XTRF congestion. The
/// recovered Mac `export_80` static dash path and Win95 XTHG actor path remain separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrafficOverlay {
    pub sprite_id: u32,
    pub density: TrafficDensity,
    pub lane: usize,
    /// Mac `jsr 0x55a` arg 4: `Random & 1` flip / heavy animation phase.
    pub blit_flip: bool,
}

/// Mac decorative dashes are raw shape blits — no OpenSC2K palette tint at draw time.
pub const TRAFFIC_DECORATIVE_TINT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
/// Per-frame draw state reserved for branch-specific Mac renderer claims.
///
/// Do not use this as a generic 2x2 XTRF dedup gate. `export_80` stamps traffic from
/// renderer branches; ordinary eligible road tiles are allowed to stamp independently.
pub struct TrafficDrawState {
    /// One bit per 2x2 XTRF block, kept only for experiments that prove such a claim locally.
    claimed: [u64; 64],
}

impl Default for TrafficDrawState {
    fn default() -> Self {
        Self { claimed: [0; 64] }
    }
}

impl TrafficDrawState {
    fn xtrf_block_index(x: usize, y: usize) -> usize {
        (y / 2) * (CITY_WIDTH / 2) + (x / 2)
    }

    /// Returns true the first time any tile in this 2x2 XTRF block requests a dash.
    ///
    /// This is not part of the ordinary Mac `export_80` traffic path.
    pub fn claim_block(&mut self, x: usize, y: usize) -> bool {
        let idx = Self::xtrf_block_index(x, y);
        let word = idx / 64;
        let bit = idx % 64;
        let mask = 1u64 << bit;
        if self.claimed[word] & mask != 0 {
            return false;
        }
        self.claimed[word] |= mask;
        true
    }

    pub fn reset(&mut self) {
        self.claimed = [0; 64];
    }
}

/// Road sprite bottom-left in screen space (matches tile-anchored roads).
pub fn road_sprite_bottom_left(tile_anchor: Vec2, road_w: f32, scale: f32) -> Vec2 {
    Vec2::new(tile_anchor.x - road_w * 0.5 * scale, tile_anchor.y)
}

/// Lane offset from road bottom-left in source pixels (+Y = down on screen).
#[allow(clippy::too_many_arguments)]
pub fn traffic_lane_offset_for_road(
    building: u8,
    lane: usize,
    road_w: f32,
    road_h: f32,
    car_w: f32,
    car_h: f32,
    sprite_id: u32,
    altm_land: u8,
) -> Vec2 {
    mac_render::traffic_overlay_offset_for_sprite(
        building, lane, sprite_id, road_w, road_h, car_w, car_h, altm_land,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn traffic_screen_bottom_left(
    city: &City,
    x: usize,
    y: usize,
    tile_anchor: Vec2,
    zoom: f32,
    building: u8,
    road_w: f32,
    road_h: f32,
    car_w: f32,
    car_h: f32,
    sprite_id: u32,
    lane: usize,
) -> Vec2 {
    mac_render::traffic_bottom_left_for_sprite(
        tile_anchor,
        zoom,
        building,
        lane,
        road_w,
        road_h,
        car_w,
        car_h,
        sprite_id,
        city.altitude_at(x, y),
    )
}

/// Mac static traffic dash family resolved from the 68k shape-table draw rows
/// `392..395` at the normal shape bank.
pub const MAC_TRAFFIC_DASH_BASE_SPRITE: u32 = 400;
pub const MAC_TRAFFIC_DASH_VARIANTS: u8 = 4;

/// Per-road traffic art from the Win95 PE traffic tables.
#[derive(Clone, Copy, Debug)]
struct TrafficArt {
    light: u32,
    heavy: u32,
}

const TRAFFIC_SPRITE_ID_BASE: u32 = 399;

/// Extracted from Win95 `SIMCITY.EXE` `trafficSpriteOffsets`.
///
/// The PE table has additional nonzero low-index entries that are not raw
/// SC2K building IDs, so CityGame intentionally exposes only the road
/// and highway tile IDs cross-checked against the Mac/OpenSC2K tile ranges.
const WIN95_TRAFFIC_SPRITE_OFFSETS: &[(u8, u8)] = &[
    (29, 1),
    (30, 2),
    (31, 3),
    (32, 4),
    (33, 5),
    (34, 6),
    (35, 7),
    (36, 8),
    (37, 9),
    (38, 10),
    (39, 2),
    (40, 1),
    (41, 2),
    (42, 1),
    (43, 2),
    (63, 2),
    (64, 1),
    (67, 1),
    (68, 2),
    (69, 1),
    (70, 2),
    (73, 11),
    (74, 12),
    (75, 11),
    (76, 12),
    (77, 11),
    (78, 12),
    (79, 11),
    (80, 12),
    (81, 13),
    (82, 13),
    (83, 13),
    (84, 13),
    (85, 13),
    (86, 13),
    (87, 13),
    (88, 13),
    (93, 15),
    (94, 16),
    (95, 17),
    (96, 18),
    (97, 19),
    (98, 20),
    (99, 21),
    (100, 22),
    (101, 23),
    (102, 24),
    (103, 25),
    (104, 26),
    (105, 27),
];

/// Extracted from Win95 `SIMCITY.EXE` `trafficSpriteOverlayLevels`.
const WIN95_TRAFFIC_HEAVY_OFFSETS: [u8; 28] = [
    0, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 15, 16, 17, 18, 42, 43, 44, 45, 46,
    47, 48, 49, 50,
];

fn traffic_art(building: u8) -> Option<TrafficArt> {
    let light_offset = traffic_light_offset_for_tile(building)?;
    let heavy_offset = traffic_heavy_offset_for_light_offset(light_offset);
    Some(TrafficArt {
        light: traffic_sprite_id_for_offset(light_offset),
        heavy: traffic_sprite_id_for_offset(heavy_offset),
    })
}

fn traffic_light_offset_for_tile(building: u8) -> Option<u8> {
    WIN95_TRAFFIC_SPRITE_OFFSETS
        .iter()
        .find_map(|&(tile, offset)| (tile == building).then_some(offset))
}

/// Win95 PE normalized road index used by the visible traffic sprite tables.
///
/// The draw routine indexes `0x004E7628`/`0x004E7798` with the road variant's
/// light-table offset, not necessarily the XTHG record's saved lane byte.
pub fn win95_traffic_lane_index_for_tile(building: u8) -> Option<usize> {
    traffic_light_offset_for_tile(building).map(usize::from)
}

/// Map Win95 PE light offset → Mac static dash variant 0–3 (sprites 400–403).
///
/// Offsets 1–4 match the Mac four-variant family exactly. Curves (5–10) and highways
/// map to diagonal variants 402/403 only — never collapse to horizontal 400/401.
fn mac_static_dash_variant_for_light_offset(offset: u8) -> u8 {
    match offset {
        1 => 0, // R-L
        2 => 1, // T-B
        3 => 2, // HT-B
        4 => 3, // HR-L
        5 | 7 | 9 => 2,
        6 | 8 | 10 => 3,
        11 | 15 | 17 | 19 | 21 | 23 | 25 | 27 => 2,
        12 | 14 | 16 | 18 | 20 | 22 | 24 | 26 => 3,
        13 => 0, // causeway / bridge (Win95 sprite 412)
        _ => 0,
    }
}

/// Map road tile → Mac static dash variant 0–3 (sprites 400–403).
pub fn mac_static_dash_variant_for_road(building: u8) -> u8 {
    let offset = traffic_light_offset_for_tile(building).unwrap_or(1);
    mac_static_dash_variant_for_light_offset(offset)
}

fn traffic_heavy_offset_for_light_offset(offset: u8) -> u8 {
    WIN95_TRAFFIC_HEAVY_OFFSETS
        .get(offset as usize)
        .copied()
        .filter(|heavy| *heavy != 0)
        .unwrap_or(offset)
}

fn traffic_sprite_id_for_offset(offset: u8) -> u32 {
    TRAFFIC_SPRITE_ID_BASE + offset as u32
}

/// Roads / highways / bridges that carry visual traffic overlays. Tile-ID ranges match
/// the viewer's road/highway taxonomy and the `export_80` `fffff344` road branch ranges
/// (29-43, 63-66, 69-70, 81-89 roads/bridges; 73-80, 93-107 highways/ramps).
pub fn mac_road_carries_traffic(building: u8) -> bool {
    matches!(
        building,
        29..=43 | 63..=66 | 69 | 70 | 73..=80 | 81..=89 | 93..=107
    )
}

/// Mac `export_80` has separate traffic-shape branches for runway and seaport tiles.
pub fn mac_export80_special_stamp_building(building: u8) -> bool {
    matches!(building, 221..=224)
}

/// Mac `export_80` ordinary single-road branch: `0x51..0x5c`, `0x6a`, and `0x6b`.
///
/// Keep this separate from the broader road-family predicate because the PE and Mac
/// renderer branches do not use identical tile taxonomies.
pub fn mac_export80_ordinary_road_building(building: u8) -> bool {
    matches!(building, 81..=92 | 106..=107)
}

pub fn mac_export80_stamp_building(building: u8) -> bool {
    mac_road_carries_traffic(building)
        || mac_export80_ordinary_road_building(building)
        || mac_export80_special_stamp_building(building)
}

/// `Some(true)` means the 68k branch samples ALTM water only when XBIT bit 2 is set.
/// `Some(false)` always samples the land elevation. `None` is not a special branch.
pub fn mac_export80_special_stamp_water_from_xbit(building: u8) -> Option<bool> {
    match building {
        221 | 222 => Some(false),
        223 | 224 => Some(true),
        _ => None,
    }
}

pub fn has_decorative_traffic(building: u8) -> bool {
    mac_road_carries_traffic(building)
}

pub fn road_tile_overlay_flip_x(city: &City, x: usize, y: usize) -> bool {
    let building = rotated_building_id(city.building_at(x, y), city.rotation());
    match building {
        // Both 1410/1411 (and 1437/1438) are authored on the SW-NE diagonal.
        // Their palette phases differ, but selecting the second mask does not rotate its
        // footprint. The NW-SE deck is a separate already-oriented surface sprite, so its
        // traffic needs this extra mirror even though the surface's XBIT flip is false.
        74 | 76 | 78 | 80 => true,
        73 | 75 | 77 | 79 => false,
        _ => crate::tiles::xbit_sprite_flip_x(city, x, y),
    }
}

pub fn traffic_sprite_for_road(building: u8, density: TrafficDensity) -> Option<u32> {
    let art = traffic_art(building)?;
    Some(road_traffic_sprite(density, art))
}

/// Road-oriented traffic tile helper.
///
/// The historical name says "dash" because this started as an `export_80` static-dash
/// experiment. The default renderer now uses the returned road-specific overlay art.
pub fn decorative_dash_base_sprite(building: u8) -> Option<u32> {
    // Shared road-overlay helper used by the default/Win95 decorative path and Mac live-debug
    // draws. Mac export_80 special-tile stamping goes through `traffic_overlay_at_for_engine`.
    if !has_decorative_traffic(building) {
        return None;
    }
    traffic_sprite_for_road(building, TrafficDensity::Light)
}

/// True for the Mac 68k static traffic dash sprites (four variants per zoom bank).
pub fn is_mac_static_dash_family(sprite_id: u32) -> bool {
    mac_static_dash_band(sprite_id).is_some()
}

/// Road-specific visual traffic uses the SC2K traffic art table.
pub fn win95_decorative_dash_base_sprite(building: u8, density: TrafficDensity) -> Option<u32> {
    let raw = traffic_sprite_for_road(building, density)?;
    Some(resolve_mac_traffic_sprite(raw, density))
}

pub fn is_traffic_lane_dash_sprite(sprite_id: u32) -> bool {
    (400..=417).contains(&sprite_id)
}

/// 16×11+ oriented heavy-traffic car shapes in SPRT_00128 (418–426).
pub fn is_traffic_car_shape(sprite_id: u32) -> bool {
    (418..=426).contains(&sprite_id)
}

/// Mac lane dashes are shape blits from the active zoom bank. Keep their renderer scale tied
/// to the road sprites; bank size already reflects the Mac shape-table zoom family.
pub const TRAFFIC_DASH_VISUAL_SCALE: f32 = 1.0;

/// Mac `export_80` shape bank from the viewer zoom: `draw_index = 392 + 500*bank + (Random&3)`.
///
/// Maps the viewer's continuous zoom to the Mac's three discrete zoom levels (`ffffe0c8` 0/1/2):
/// - bank 0 (+0)    -> sprites 400-403, 8px dashes (zoomed out / normal)
/// - bank 1 (+500)  -> sprites 900-903, 16px dashes (mid zoom)
/// - bank 2 (+1000) -> sprites 1400-1403, 32px dashes (zoomed in)
pub fn traffic_shape_scale_band(viewer_zoom: f32) -> u32 {
    traffic_shape_scale_band_discrete(viewer_zoom)
}

fn traffic_shape_scale_band_discrete(viewer_zoom: f32) -> u32 {
    if viewer_zoom >= TRAFFIC_LARGE_BAND_ZOOM {
        TRAFFIC_LARGE_SPRITE_OFFSET
    } else if viewer_zoom >= TRAFFIC_MEDIUM_BAND_ZOOM {
        TRAFFIC_MEDIUM_SPRITE_OFFSET
    } else {
        0
    }
}

/// Pick the static Mac dash bank for a draw pass, bumping when dashes would be sub-pixel.
///
/// Mirrors `export_80`'s discrete zoom banks (`ffffe0c8` 0/1/2 -> +0/+500/+1000). The large
/// bank is deliberately capped out for now because this renderer does not yet have the exact
/// Mac bank-2 anchor semantics, and using it creates the long double-line artifacts.
pub fn traffic_shape_scale_band_for_draw(
    renderer: &crate::renderer::Renderer<'_>,
    viewer_zoom: f32,
) -> u32 {
    let base_w = mac_static_dash_probe_width(renderer, 0).unwrap_or(8.0);
    traffic_shape_scale_band_for_dash_width(viewer_zoom, base_w)
}

fn static_traffic_overlay_shape_scale_band(preferred: u32) -> u32 {
    preferred.min(TRAFFIC_MAX_OVERLAY_SPRITE_OFFSET)
}

/// Same band logic as [`traffic_shape_scale_band_for_draw`] without atlas lookup (unit tests).
pub fn traffic_shape_scale_band_for_dash_width(viewer_zoom: f32, base_dash_w: f32) -> u32 {
    let preferred =
        static_traffic_overlay_shape_scale_band(traffic_shape_scale_band_discrete(viewer_zoom));
    let bands = [
        0u32,
        TRAFFIC_MEDIUM_SPRITE_OFFSET,
        TRAFFIC_LARGE_SPRITE_OFFSET,
    ];
    let start = bands
        .iter()
        .position(|&band| band >= preferred)
        .unwrap_or(0);
    for &band in &bands[start..] {
        let scale = mac_static_dash_band_screen_scale(band);
        if base_dash_w * scale * viewer_zoom >= TRAFFIC_MIN_SCREEN_PX {
            return band;
        }
    }
    TRAFFIC_MAX_OVERLAY_SPRITE_OFFSET
}

fn mac_static_dash_probe_width(renderer: &crate::renderer::Renderer<'_>, band: u32) -> Option<f32> {
    let sprite_id = MAC_TRAFFIC_DASH_BASE_SPRITE + band;
    let atlas = renderer.traffic_atlas_for_sprite(sprite_id)?;
    let (w, _) = renderer.sprite_size(atlas, sprite_id)?;
    Some(w as f32)
}

fn mac_static_dash_band_screen_scale(band: u32) -> f32 {
    match band {
        TRAFFIC_MEDIUM_SPRITE_OFFSET => 2.0,
        TRAFFIC_LARGE_SPRITE_OFFSET => 4.0,
        _ => 1.0,
    }
}

fn mac_static_dash_band(sprite_id: u32) -> Option<(u32, u8)> {
    if (400..=403).contains(&sprite_id) {
        Some((0, (sprite_id - 400) as u8))
    } else if (900..=903).contains(&sprite_id) {
        Some((TRAFFIC_MEDIUM_SPRITE_OFFSET, (sprite_id - 900) as u8))
    } else if (1400..=1403).contains(&sprite_id) {
        Some((TRAFFIC_LARGE_SPRITE_OFFSET, (sprite_id - 1400) as u8))
    } else {
        None
    }
}

fn normalize_mac_static_dash_sprite(sprite_id: u32) -> u32 {
    if let Some((band, variant)) = mac_static_dash_band(sprite_id) {
        MAC_TRAFFIC_DASH_BASE_SPRITE + band + (variant as u32 % MAC_TRAFFIC_DASH_VARIANTS as u32)
    } else {
        sprite_id
    }
}

/// True when the baked traffic sprite covers the road tile (SPRT_00129 @ 1400+).
pub fn is_full_tile_traffic_overlay(car_w: f32, car_h: f32, road_w: f32, road_h: f32) -> bool {
    car_w >= road_w * 0.9 && car_h >= road_h * 0.9
}

/// Screen scale for a traffic sprite.
///
/// The Mac lane dash sprites are already in the same pixel coordinate space as
/// the extracted road sprites. Scaling them separately makes the bottom-left
/// lane anchor drift away from the road.
pub fn traffic_dash_screen_scale(
    sprite_id: u32,
    zoom: f32,
    car_w: f32,
    car_h: f32,
    road_w: f32,
    road_h: f32,
) -> f32 {
    let _ = (car_h, road_h);
    if is_mac_static_dash_family(sprite_id) {
        return zoom * TRAFFIC_DASH_VISUAL_SCALE;
    }
    if car_w <= road_w * 0.5 {
        zoom * TRAFFIC_DASH_VISUAL_SCALE
    } else {
        zoom
    }
}

/// Resolve the logical traffic sprite for the active Mac shape-table band.
pub fn traffic_render_sprite_id(
    renderer: &crate::renderer::Renderer<'_>,
    logical_id: u32,
    scale_band: u32,
) -> u32 {
    traffic_render_sprite_id_with_lookup(logical_id, scale_band, |sprite_id| {
        renderer.traffic_atlas_for_sprite(sprite_id).is_some()
    })
}

fn traffic_render_sprite_id_with_lookup(
    logical_id: u32,
    scale_band: u32,
    has_traffic_sprite: impl Fn(u32) -> bool,
) -> u32 {
    if scale_band == 0 {
        return normalize_traffic_dash_sprite(logical_id);
    }
    let large_id = logical_id.saturating_add(scale_band);
    if has_traffic_sprite(large_id) {
        large_id
    } else {
        normalize_traffic_dash_sprite(logical_id)
    }
}

/// Map missing heavy ids to light lane dashes (8px band only).
pub fn normalize_traffic_dash_sprite(sprite_id: u32) -> u32 {
    match sprite_id {
        427..=449 => heavy_to_light_traffic_sprite(sprite_id).unwrap_or(sprite_id),
        927..=949 => heavy_to_light_traffic_sprite(sprite_id - TRAFFIC_MEDIUM_SPRITE_OFFSET)
            .map(|id| id + TRAFFIC_MEDIUM_SPRITE_OFFSET)
            .unwrap_or(sprite_id),
        1427..=1449 => heavy_to_light_traffic_sprite(sprite_id - TRAFFIC_LARGE_SPRITE_OFFSET)
            .map(|id| id + TRAFFIC_LARGE_SPRITE_OFFSET)
            .unwrap_or(sprite_id),
        _ => sprite_id,
    }
}

/// Heavy traffic maps PE table offsets to oriented car shapes 418–426 (427+ absent).
fn resolve_mac_traffic_sprite(sprite_id: u32, density: TrafficDensity) -> u32 {
    if density != TrafficDensity::Heavy {
        return normalize_traffic_dash_sprite(sprite_id);
    }
    let offset = sprite_id.saturating_sub(TRAFFIC_SPRITE_ID_BASE);
    if (20..=26).contains(&offset) {
        return sprite_id;
    }
    if (28..=50).contains(&offset) {
        return 418 + ((offset - 28) % 9);
    }
    if (15..=19).contains(&offset) {
        return 418 + (offset - 15);
    }
    if is_traffic_car_shape(sprite_id) {
        return sprite_id;
    }
    normalize_traffic_dash_sprite(sprite_id)
}

fn heavy_to_light_traffic_sprite(sprite_id: u32) -> Option<u32> {
    let heavy_offset = sprite_id.checked_sub(TRAFFIC_SPRITE_ID_BASE)? as u8;
    WIN95_TRAFFIC_HEAVY_OFFSETS
        .iter()
        .position(|&offset| offset == heavy_offset)
        .map(|light_offset| traffic_sprite_id_for_offset(light_offset as u8))
}

/// OpenSC2K: `flip` on bridge/highway tiles is gated by per-tile `XBIT.rotate`.
pub fn traffic_flip_for_tile(city: &City, x: usize, y: usize, building: u8) -> bool {
    let xbit_rotate = city.tile_bits_at(x, y).is_some_and(|bits| bits.rotate());
    if crate::tiles::building_uses_xbit_flip(building) {
        return xbit_rotate;
    }
    traffic_sprite_flip_x(building)
}

/// Mac `export_80` passes `Random & 1` as the shape-blitt flip arg (`jsr 0x55a`).
#[deprecated(note = "use TrafficOverlay::blit_flip from MacToolboxRng draw pass")]
pub fn traffic_lane_flip_x(x: usize, y: usize) -> bool {
    traffic_pseudo_random_bit(x, y, 0)
}

/// Deterministic stand-in retained for tests only — real draws use [`MacToolboxRng`].
#[deprecated(note = "use MacToolboxRng::random_u3 during traffic_overlay_at")]
pub fn traffic_dash_variant(x: usize, y: usize) -> u8 {
    ((x.wrapping_mul(17) + y.wrapping_mul(31) + 7) as u8) & 3
}

/// Mac `ffffed40` bit 1 alternate draw variant (`btst #1` @ `0x6994`).
pub fn traffic_alternate_draw_variant(x: usize, y: usize) -> bool {
    traffic_pseudo_random_bit(x, y, 1)
}

/// Mac `export_80`: `(Random & 3) + 0x188 + shape_bank`.
pub fn traffic_sprite_with_dash_variant(sprite_id: u32, variant: u8) -> u32 {
    if !is_traffic_lane_dash_sprite(sprite_id) {
        return sprite_id;
    }
    mac_static_traffic_sprite_with_variant(variant)
}

pub fn mac_static_traffic_sprite_with_variant(variant: u8) -> u32 {
    MAC_TRAFFIC_DASH_BASE_SPRITE + (variant as u32 % MAC_TRAFFIC_DASH_VARIANTS as u32)
}

fn traffic_sprite_for_building_density(building: u8, density: TrafficDensity) -> Option<u32> {
    traffic_sprite_for_road(building, density)
}

fn mac_static_traffic_sprite_for_sprite_family(sprite_id: u32, random_u3: u8) -> u32 {
    if is_mac_static_dash_family(sprite_id) {
        let band = mac_static_dash_band(sprite_id)
            .map(|(band, _)| band)
            .unwrap_or_default();
        return MAC_TRAFFIC_DASH_BASE_SPRITE
            + band
            + (random_u3 as u32 % MAC_TRAFFIC_DASH_VARIANTS as u32);
    }
    sprite_id
}

pub fn win95_traffic_sprite_with_dash_variant(sprite_id: u32, variant: u8) -> u32 {
    if !is_traffic_lane_dash_sprite(sprite_id) {
        return sprite_id;
    }
    let Some(road_offset) = sprite_id.checked_sub(TRAFFIC_SPRITE_ID_BASE) else {
        return sprite_id;
    };
    if road_offset == 0 || road_offset > 50 {
        return sprite_id;
    }
    TRAFFIC_SPRITE_ID_BASE + road_offset + (variant as u32 & 3)
}

/// Traffic tile art is palette-cycled. SC2KFix documents the road traffic tile family
/// as 8-frame animated traffic overlays.
pub fn traffic_anim_frame_for_tile(
    _density: TrafficDensity,
    global_frame: u8,
    _blit_flip: bool,
) -> u8 {
    global_frame % 8
}

pub fn traffic_decorative_flip_x(
    city: &City,
    x: usize,
    y: usize,
    building: u8,
    sprite_id: u32,
    blit_flip: bool,
) -> bool {
    if crate::tiles::building_uses_xbit_flip(building) {
        traffic_flip_for_tile(city, x, y, building)
    } else if is_mac_static_dash_family(sprite_id) {
        // `Random&1` is an argument to the Mac 0x055a shape blitter, but the low helper
        // has not been proven to equal this renderer's GPU-side horizontal flip. The dash
        // row (`400..403` / bank variants) already contains paired mirror/diagonal shapes;
        // applying another random x-flip creates orientations not justified by export_80.
        false
    } else {
        blit_flip
    }
}

fn traffic_pseudo_random_bit(x: usize, y: usize, salt: u8) -> bool {
    let mixed = x
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(y.wrapping_mul(0x85eb_ca6b))
        .wrapping_add(salt as usize);
    mixed & 1 == 1
}

#[allow(deprecated)]
pub fn traffic_decorative_flip_x_legacy(city: &City, x: usize, y: usize, building: u8) -> bool {
    traffic_decorative_flip_x(city, x, y, building, 400, traffic_lane_flip_x(x, y))
}

/// Decide whether a decorative dash belongs on this road tile.
pub fn traffic_overlay_at(
    city: &City,
    x: usize,
    y: usize,
    _draw_state: &mut TrafficDrawState,
    congestion: impl Fn(usize, usize) -> u8,
    rng: &mut crate::mac_random::MacToolboxRng,
    _anim_frame: u8,
) -> Option<TrafficOverlay> {
    traffic_overlay_stamp_at(city, x, y, congestion, rng)
}

/// Select a traffic overlay using the explicit executable-era contract.
pub fn traffic_overlay_at_for_engine(
    city: &City,
    x: usize,
    y: usize,
    fidelity: EngineFidelity,
    congestion: impl Fn(usize, usize) -> u8,
    rng: &mut crate::mac_random::MacToolboxRng,
    _anim_frame: u8,
) -> Option<TrafficOverlay> {
    traffic_overlay_stamp_at_for_engine(city, x, y, fidelity, congestion, rng)
}

/// Emit one shared XTRF-driven traffic stamp without applying a 2x2 XTRF claim.
///
/// This compatibility helper has no engine selector, so it uses the shared road-art lookup and
/// never infers behavior from city-file provenance. The app renderer uses the explicit engine
/// helper below. Multi-cell callers can use this helper for each footprint tile.
pub fn traffic_overlay_stamp_at(
    city: &City,
    x: usize,
    y: usize,
    congestion: impl Fn(usize, usize) -> u8,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<TrafficOverlay> {
    win95::traffic_overlay_stamp_at(city, x, y, congestion, rng)
}

/// Emit one traffic stamp without inferring the engine from the save-file format.
pub fn traffic_overlay_stamp_at_for_engine(
    city: &City,
    x: usize,
    y: usize,
    fidelity: EngineFidelity,
    congestion: impl Fn(usize, usize) -> u8,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> Option<TrafficOverlay> {
    if fidelity.uses_win95() {
        win95::traffic_overlay_stamp_at(city, x, y, congestion, rng)
    } else {
        mac_export80::traffic_overlay_stamp_at(city, x, y, rng)
    }
}

/// Mac `D5==2` multi-cell road/highway footprints (`export_80` @ `0x6b0c`).
pub fn multi_cell_road_footprint(building: u8) -> &'static [(i32, i32)] {
    if is_multi_cell_road(building) {
        &[(0, 0), (1, 0), (0, 1), (1, 1)]
    } else {
        &[(0, 0)]
    }
}

pub fn is_multi_cell_road(building: u8) -> bool {
    // Mac export_81 returns size 2 for 0x49..0x50 and 0x61..0x6b. The
    // 0x5d..0x60 on-ramp family is traffic-capable but remains one tile.
    matches!(building, 73..=80 | 97..=107)
}

pub fn traffic_sprite_flip_for_road(building: u8) -> bool {
    traffic_sprite_flip_x(building)
}

pub fn is_spawnable_traffic_road(building: u8) -> bool {
    matches!(building, 29 | 30 | 39..=43)
}

pub fn traffic_sprite_flip_x(building: u8) -> bool {
    matches!(building, 63..=66 | 69 | 70 | 81..=89)
}

pub fn count_traffic_overlays(
    city: &City,
    congestion: impl Fn(usize, usize) -> u8,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> usize {
    let mut state = TrafficDrawState::default();
    let mut count = 0usize;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if traffic_overlay_at(city, x, y, &mut state, &congestion, rng, 0).is_some() {
                count += 1;
            }
        }
    }
    count
}

pub fn count_traffic_overlays_for_engine(
    city: &City,
    fidelity: EngineFidelity,
    congestion: impl Fn(usize, usize) -> u8,
    rng: &mut crate::mac_random::MacToolboxRng,
) -> usize {
    let mut count = 0usize;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if traffic_overlay_at_for_engine(city, x, y, fidelity, &congestion, rng, 0).is_some() {
                count += 1;
            }
        }
    }
    count
}

pub fn resolve_traffic_sprite(
    renderer: &crate::renderer::Renderer<'_>,
    base_sprite_id: u32,
    scale_band: u32,
    _blit_flip: bool,
    anim_frame: u8,
) -> Option<(usize, u32)> {
    let base = traffic_render_sprite_id(renderer, base_sprite_id, scale_band);
    let normalized = if is_mac_static_dash_family(base_sprite_id) || is_mac_static_dash_family(base)
    {
        normalize_mac_static_dash_sprite(base)
    } else {
        normalize_traffic_dash_sprite(base)
    };
    let candidates = [
        base,
        normalized,
        fallback_traffic_sprite_id(base, scale_band),
        fallback_traffic_sprite_id(normalized, scale_band),
    ];
    for candidate in candidates {
        if !is_lane_dash_baked_sprite(candidate) {
            continue;
        }
        let Some(atlas) = renderer.traffic_atlas_for_sprite(candidate) else {
            continue;
        };
        let resolved = renderer.resolve_sprite_id(atlas, candidate, anim_frame);
        if renderer.has_sprite(atlas, resolved) {
            return Some((atlas, resolved));
        }
    }
    None
}

/// Sprites that went through lane-dash or car-shape baking — never draw raw mask indices.
fn is_lane_dash_baked_sprite(sprite_id: u32) -> bool {
    is_mac_static_dash_family(sprite_id)
        || (400..=426).contains(&sprite_id)
        || (900..=947).contains(&sprite_id)
        || (1400..=1449).contains(&sprite_id)
}

fn fallback_traffic_sprite_id(sprite_id: u32, _scale_band: u32) -> u32 {
    if let Some((band, variant)) = mac_static_dash_band(sprite_id) {
        return MAC_TRAFFIC_DASH_BASE_SPRITE
            + band
            + (variant as u32 % MAC_TRAFFIC_DASH_VARIANTS as u32);
    }
    if is_mac_static_dash_family(sprite_id) {
        return normalize_mac_static_dash_sprite(sprite_id);
    }
    match sprite_id {
        427..=449 => heavy_to_light_traffic_sprite(sprite_id).unwrap_or(sprite_id),
        1427..=1449 => heavy_to_light_traffic_sprite(sprite_id - TRAFFIC_LARGE_SPRITE_OFFSET)
            .map(|id| id + TRAFFIC_LARGE_SPRITE_OFFSET)
            .unwrap_or(sprite_id),
        1400..=1449 => sprite_id,
        _ => sprite_id.min(426),
    }
}

pub fn density_from_xtrf(value: u8) -> Option<TrafficDensity> {
    if value < TRAFFIC_LIGHT_THRESHOLD {
        None
    } else if value < TRAFFIC_HEAVY_THRESHOLD {
        Some(TrafficDensity::Light)
    } else {
        Some(TrafficDensity::Heavy)
    }
}

pub fn mac_heavy_from_traffic_byte(value: u8) -> bool {
    (MAC_TRAFFIC_HEAVY_BYTE_MIN..=MAC_TRAFFIC_HEAVY_BYTE_MAX).contains(&value)
}

pub fn density_from_xtrf_mac(value: u8) -> Option<TrafficDensity> {
    if value < MAC_TRAFFIC_VISIBLE_BYTE_MIN {
        return None;
    }
    if mac_heavy_from_traffic_byte(value) || value >= TRAFFIC_HEAVY_THRESHOLD {
        Some(TrafficDensity::Heavy)
    } else {
        Some(TrafficDensity::Light)
    }
}

/// Win95 `FUN_0043C900` selects visible traffic art at XTRF 0x1c/0x38.
pub fn density_from_xtrf_win95_draw(value: u8) -> Option<TrafficDensity> {
    if value <= WIN95_TRAFFIC_DRAW_LIGHT_THRESHOLD {
        None
    } else if value <= WIN95_TRAFFIC_DRAW_HEAVY_THRESHOLD {
        Some(TrafficDensity::Light)
    } else {
        Some(TrafficDensity::Heavy)
    }
}

fn road_traffic_sprite(density: TrafficDensity, art: TrafficArt) -> u32 {
    match density {
        TrafficDensity::Heavy => art.heavy,
        TrafficDensity::Light => art.light,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameSpeed {
    Paused,
    Turtle,
    Llama,
    Cheetah,
    AfricanSwallow,
}

impl GameSpeed {
    pub fn tick_multiplier(self) -> f32 {
        match self {
            GameSpeed::Paused => 0.0,
            GameSpeed::Turtle => 0.25,
            GameSpeed::Llama => 1.0,
            GameSpeed::Cheetah => 2.5,
            GameSpeed::AfricanSwallow => 5.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GameSpeed::Paused => "Paused",
            GameSpeed::Turtle => "Turtle",
            GameSpeed::Llama => "Llama",
            GameSpeed::Cheetah => "Cheetah",
            GameSpeed::AfricanSwallow => "African Swallow",
        }
    }
}

#[cfg(test)]
#[path = "traffic/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "traffic/mac_export80_special_branch_tests.rs"]
mod mac_export80_special_branch_tests;
