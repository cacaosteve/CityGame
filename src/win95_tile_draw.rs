//! Win95 main map overlay draw consumer (`FUN_0046f4a2` draw path).
//!
//! Actor simulation for port road cars is the optional runtime kind `0xd0` path in
//! `Win95OverlaySim` — not PE `FUN_00455020` (Maxis Man / kind `0x10`). Loaded PE kind
//! `0x06` is `XTHG_EXPLOSION` (`FUN_00453280` create; dispatcher thunk to `0x4554F0` is
//! `ret` only) and must not select the live-actor dash draw path.
//!
//! When a city has active eligible port traffic actors (`0xd0`, or legacy `0x06` only in
//! blank/test cities), dashes are drawn from the 40-slot overlay table with sub-tile
//! coords (`+6`/`+7` on a 16×16 grid) instead of the Mac per-tile `XTRF` decoration scan
//! in `traffic_overlay_at`.

use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};
use crate::mac_random::MacToolboxRng;
use crate::mac_shape_table;
use crate::tiles::rotated_building_id;
use crate::traffic::{
    TrafficDensity, TrafficOverlay, has_decorative_traffic, win95_decorative_dash_base_sprite,
    win95_traffic_lane_index_for_tile, win95_traffic_sprite_with_dash_variant,
};
use crate::win95_overlay::{OverlayState, Win95OverlayRecord, XTHG_FIRST_USABLE_SLOT};
use glam::Vec2;

/// Win95 overlay actors move on a 16×16 sub-tile grid (`FUN_004516f0` wraps at `0x10`).
pub const WIN95_SUBTILE_GRID: f32 = 16.0;

const STANDARD_ROAD_BAND_H: f32 = 17.0;
const ROAD_ASPHALT_INSET_X: f32 = 4.0;

// PE FUN_0043C900 uses lower XTRF bands for visible traffic art than the sim's
// 0x33/0xC9 state thresholds: <= 0x1C is hidden, <= 0x38 is light art, and
// values above 0x38 select the heavy art branch.
const PE_TRAFFIC_VISIBLE_MIN: u8 = 0x1c;
const PE_TRAFFIC_HEAVY_ART_MIN: u8 = 0x38;
const PE_LIGHT_SPRITE_BASE: u32 = 0x3e8;
const PE_HEAVY_SPRITE_BASE: u32 = 0x577;

// SIMCITY.EXE 0x004E7628, little-endian words. The PE draw path adds 0x3E8.
const PE_TRAFFIC_LANE_INDEX_WORDS: [u16; 64] = [
    0x100, 0x101, 0x102, 0x103, 0x104, 0x105, 0x106, 0x107, 0x108, 0x109, 0x10a, 0x10b, 0x10c,
    0x10d, 0x10d, 0x000, 0x10e, 0x10e, 0x10e, 0x10e, 0x10e, 0x10e, 0x10e, 0x10e, 0x10e, 0x10e,
    0x10e, 0x10e, 0x10e, 0x10e, 0x10e, 0x10e, 0x10e, 0x10f, 0x110, 0x111, 0x112, 0x113, 0x114,
    0x115, 0x116, 0x117, 0x118, 0x119, 0x11a, 0x11b, 0x11c, 0x000, 0x10e, 0x10f, 0x110, 0x111,
    0x112, 0x113, 0x114, 0x115, 0x116, 0x117, 0x118, 0x119, 0x11a, 0x11b, 0x11c, 0x000,
];

// SIMCITY.EXE 0x004E7798. Zero entries fall back to the light lane sprite.
const PE_TRAFFIC_HEAVY_OFFSETS: [u8; 32] = [
    0x00, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x0f,
    0x10, 0x11, 0x12, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x31, 0x32, 0x00, 0x00, 0x00, 0x00,
];

/// One traffic blit from the Win95 overlay actor table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Win95TrafficDraw {
    pub tile_x: usize,
    pub tile_y: usize,
    pub overlay: TrafficOverlay,
    pub sub_x: u8,
    pub sub_y: u8,
}

/// Select the live-actor draw path only when an eligible port actor is active. An empty XTHG
/// table, or a loaded table containing only PE kinds such as explosions, must leave the normal
/// per-tile XTRF traffic-art pass enabled.
pub fn uses_win95_overlay_draw(city: &City) -> bool {
    city.traffic_overlays().is_some_and(|table| {
        table
            .iter()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .any(|record| city.is_port_traffic_record(*record) && record.is_drawable_traffic())
    })
}

/// Collect drawable traffic from all active `XTHG` slots (main tile overlay pass).
pub fn collect_win95_traffic_draws(
    city: &City,
    congestion: impl Fn(usize, usize) -> u8,
    rng: &mut MacToolboxRng,
) -> Vec<Win95TrafficDraw> {
    let Some(table) = city.traffic_overlays() else {
        return Vec::new();
    };
    let rotation = city.rotation();
    table
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .filter_map(|(slot, record)| {
            win95_traffic_draw_from_record(city, record, slot, rotation, &congestion, rng)
        })
        .collect()
}

fn win95_traffic_draw_from_record(
    city: &City,
    record: &Win95OverlayRecord,
    slot: usize,
    rotation: u8,
    congestion: &impl Fn(usize, usize) -> u8,
    rng: &mut MacToolboxRng,
) -> Option<Win95TrafficDraw> {
    if !city.is_port_traffic_record(*record) || !record.is_drawable_traffic() {
        return None;
    }
    let tile_x = record.tile_x as usize;
    let tile_y = record.tile_y as usize;
    if tile_x >= CITY_WIDTH || tile_y >= CITY_HEIGHT {
        return None;
    }
    let building = rotated_building_id(city.building_at(tile_x, tile_y), rotation);
    if !has_decorative_traffic(building) {
        return None;
    }
    let density = overlay_density(record, tile_x, tile_y, congestion)?;
    let lane_index = win95_traffic_lane_index_for_tile(building).unwrap_or(record.lane as usize);
    let base_sprite = pe_traffic_sprite_for_lane(lane_index, density)
        .or_else(|| win95_decorative_dash_base_sprite(building, density))?;
    let variant = pe_traffic_dash_variant(rng);
    let blit_flip = rng.random_bit() ^ (slot & 1 == 1);
    let sprite_id = win95_traffic_sprite_with_dash_variant(base_sprite, variant);

    Some(Win95TrafficDraw {
        tile_x,
        tile_y,
        overlay: TrafficOverlay {
            sprite_id,
            density,
            lane: record.lane as usize,
            blit_flip,
        },
        sub_x: record.sub_x,
        sub_y: record.sub_y,
    })
}

/// PE traffic draw sites select one of four dash variants with `Random & 3`.
fn pe_traffic_dash_variant(rng: &mut MacToolboxRng) -> u8 {
    (rng.random_u16() & 0x03) as u8
}

fn overlay_density(
    record: &Win95OverlayRecord,
    tile_x: usize,
    tile_y: usize,
    congestion: &impl Fn(usize, usize) -> u8,
) -> Option<TrafficDensity> {
    let byte = congestion(tile_x, tile_y);
    if byte <= PE_TRAFFIC_VISIBLE_MIN {
        return None;
    }
    let density = if byte > PE_TRAFFIC_HEAVY_ART_MIN {
        TrafficDensity::Heavy
    } else {
        TrafficDensity::Light
    };
    match record.overlay_state() {
        OverlayState::HeavyActive | OverlayState::LightActive => Some(density),
        OverlayState::SpawnPending | OverlayState::Inactive => None,
    }
}

fn pe_traffic_sprite_for_lane(lane: usize, density: TrafficDensity) -> Option<u32> {
    let light = PE_TRAFFIC_LANE_INDEX_WORDS
        .get(lane)
        .copied()
        .map(|offset| PE_LIGHT_SPRITE_BASE + u32::from(offset))?;
    match density {
        TrafficDensity::Light => Some(light),
        TrafficDensity::Heavy => PE_TRAFFIC_HEAVY_OFFSETS
            .get(lane)
            .copied()
            .filter(|offset| *offset != 0)
            .map(|offset| PE_HEAVY_SPRITE_BASE + u32::from(offset))
            .or(Some(light)),
    }
}

/// Bottom-left anchor for a Win95 overlay actor using saved sub-tile coords.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn win95_traffic_screen_bottom_left(
    tile_anchor: Vec2,
    zoom: f32,
    road_w: f32,
    road_h: f32,
    car_w: f32,
    car_h: f32,
    sprite_id: u32,
    sub_x: u8,
    sub_y: u8,
) -> Vec2 {
    let road_bl = Vec2::new(tile_anchor.x - road_w * 0.5 * zoom, tile_anchor.y);
    let offset =
        win95_subtile_overlay_offset(sprite_id, road_w, road_h, car_w, car_h, sub_x, sub_y);
    road_bl + offset * zoom
}

fn win95_subtile_overlay_offset(
    sprite_id: u32,
    road_w: f32,
    road_h: f32,
    car_w: f32,
    car_h: f32,
    sub_x: u8,
    sub_y: u8,
) -> Vec2 {
    let placement_h = if road_h > 20.0 {
        STANDARD_ROAD_BAND_H
    } else {
        road_h
    };

    let mut x = (sub_x as f32 / WIN95_SUBTILE_GRID) * road_w - car_w * 0.5;
    let y_top = (sub_y as f32 / WIN95_SUBTILE_GRID) * placement_h - car_h * 0.5;

    let x_min = ROAD_ASPHALT_INSET_X;
    let x_max = (road_w - car_w - ROAD_ASPHALT_INSET_X).max(x_min);
    x = x.clamp(x_min, x_max);

    let y_sub = mac_shape_table::lookup(sprite_id)
        .map(|record| record.y_sub)
        .unwrap_or(4.0);
    let y_from_bottom =
        (placement_h - y_top - car_h + (y_sub - 4.0)).clamp(0.0, (placement_h - car_h).max(0.0));

    Vec2::new(x, -y_from_bottom)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::TileBits;
    use crate::win95_overlay::{
        OVERLAY_KIND_TRAFFIC, OverlayState, RUST_TRAFFIC_ACTOR_KIND, Win95OverlayRecord,
        XTHG_FIRST_USABLE_SLOT, empty_overlay_table,
    };

    #[test]
    fn pe_traffic_dash_variant_uses_random_low_two_bits() {
        let mut actual = MacToolboxRng::new(0x1234_5678);
        let mut expected = MacToolboxRng::new(0x1234_5678);
        assert_eq!(
            pe_traffic_dash_variant(&mut actual),
            (expected.random_u16() & 0x03) as u8
        );
        assert_eq!(actual.seed(), expected.seed());
    }

    #[test]
    fn drawable_traffic_requires_active_state() {
        let mut record = Win95OverlayRecord {
            kind: OVERLAY_KIND_TRAFFIC,
            ..Default::default()
        };
        assert!(!record.is_drawable_traffic());
        record.state = OverlayState::LightActive as u8;
        assert!(record.is_drawable_traffic());
        record.state = OverlayState::SpawnPending as u8;
        assert!(!record.is_drawable_traffic());
    }

    #[test]
    fn subtile_center_lands_on_road_middle() {
        let offset = win95_subtile_overlay_offset(400, 32.0, 17.0, 8.0, 4.0, 8, 8);
        assert!((offset.x - 12.0).abs() < 2.0, "x={}", offset.x);
        assert!(offset.y < -4.0 && offset.y > -12.0, "y={}", offset.y);
    }

    #[test]
    fn win95_subtile_center_sits_on_road_asphalt_not_dirt_corner() {
        // This guards only the Win95/XTHG sub-tile helper. Shipping Mac `export_80`
        // static traffic must not call this path.
        let zoom = 2.0;
        let road_w = 32.0;
        let road_h = 17.0;
        let (car_w, car_h) = (8.0, 4.0);
        // Road tile bottom-centre anchor (bottom_left_anchor=false convention).
        let map = Vec2::new(100.0, 50.0);
        let bl =
            win95_traffic_screen_bottom_left(map, zoom, road_w, road_h, car_w, car_h, 400, 8, 8);
        // Renderer (bottom_left_anchor=true): x0=bl.x, x1=bl.x+w; y1=bl.y, y0=bl.y-h.
        let (x0, x1) = (bl.x, bl.x + car_w * zoom);
        let (y0, y1) = (bl.y - car_h * zoom, bl.y);
        let dash_cx = 0.5 * (x0 + x1);
        // Horizontally centred on the road centre (map.x).
        assert!(
            (dash_cx - map.x).abs() <= zoom,
            "dash centre x={dash_cx} should sit on road centre x={}",
            map.x
        );
        // Vertically inside the diamond band: above the bottom vertex (map.y) and
        // below the top vertex (map.y - road_h*zoom), straddling the mid-line.
        let mid = map.y - 0.5 * road_h * zoom;
        assert!(
            y1 < map.y,
            "dash bottom {y1} must be above road bottom {}",
            map.y
        );
        assert!(
            y0 > map.y - road_h * zoom,
            "dash top {y0} must be below road top {}",
            map.y - road_h * zoom
        );
        assert!(
            y0 <= mid && y1 >= mid,
            "dash band [{y0},{y1}] should straddle road mid-line {mid}"
        );
    }

    #[test]
    fn tall_road_uses_bottom_asphalt_band() {
        let short = win95_subtile_overlay_offset(402, 32.0, 17.0, 8.0, 7.0, 8, 8);
        let tall = win95_subtile_overlay_offset(402, 32.0, 29.0, 8.0, 7.0, 8, 8);
        assert_eq!(short.y, tall.y);
    }

    #[test]
    fn collects_from_active_xthg_slot() {
        let mut table = empty_overlay_table();
        table[0] = Win95OverlayRecord {
            kind: OVERLAY_KIND_TRAFFIC,
            state: OverlayState::LightActive as u8,
            tile_x: 40,
            tile_y: 40,
            sub_x: 8,
            sub_y: 8,
            lane: 1,
            ..Default::default()
        };
        table[1] = table[0];
        let mut city = City::new_blank(std::path::Path::new("/tmp/xthg-test.sc2"), "XTHGTest")
            .expect("blank city");
        city.set_traffic_overlays_for_test(table);
        city.set_building(40, 40, 29);
        city.set_traffic_density(40, 40, 0xbb);

        let mut rng = MacToolboxRng::new(0xBEEF);
        let congestion = |x: usize, y: usize| city.traffic_density_at(x, y).unwrap_or(0);
        let draws = collect_win95_traffic_draws(&city, congestion, &mut rng);
        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0].tile_x, 40);
        assert_eq!(draws[0].sub_x, 8);
    }

    #[test]
    fn pe_visible_traffic_uses_draw_bands_not_sim_bands() {
        let density = |state, byte| {
            let record = Win95OverlayRecord {
                kind: OVERLAY_KIND_TRAFFIC,
                state: state as u8,
                ..Default::default()
            };
            overlay_density(&record, 0, 0, &|_, _| byte)
        };

        assert_eq!(density(OverlayState::HeavyActive, 0x1c), None);
        assert_eq!(
            density(OverlayState::HeavyActive, 0x1d),
            Some(TrafficDensity::Light)
        );
        assert_eq!(
            density(OverlayState::HeavyActive, 0x38),
            Some(TrafficDensity::Light)
        );
        assert_eq!(
            density(OverlayState::HeavyActive, 0x39),
            Some(TrafficDensity::Heavy)
        );

        assert_eq!(
            density(OverlayState::LightActive, 0x39),
            Some(TrafficDensity::Heavy)
        );
        assert_eq!(density(OverlayState::SpawnPending, 0x39), None);
    }

    #[test]
    fn pe_traffic_uses_lane_words_and_heavy_base() {
        assert_eq!(
            pe_traffic_sprite_for_lane(3, TrafficDensity::Light),
            Some(0x3e8 + 0x103)
        );
        assert_eq!(
            pe_traffic_sprite_for_lane(1, TrafficDensity::Heavy),
            Some(0x577 + 0x1c)
        );
        assert_eq!(
            pe_traffic_sprite_for_lane(0, TrafficDensity::Heavy),
            Some(0x3e8 + 0x100)
        );
    }

    #[test]
    fn pe_traffic_normalizes_road_variant_before_lane_lookup() {
        assert_eq!(win95_traffic_lane_index_for_tile(29), Some(1));
        assert_eq!(win95_traffic_lane_index_for_tile(73), Some(11));
        assert_eq!(win95_traffic_lane_index_for_tile(63), Some(2));
        assert_eq!(win95_traffic_lane_index_for_tile(65), None);
    }

    #[test]
    fn win95_actor_draws_on_unrotated_roads() {
        let mut city =
            City::new_blank(std::path::Path::new("/tmp/xbit-traffic.sc2"), "XBITTraffic")
                .expect("blank city");
        let mut table = empty_overlay_table();
        table[XTHG_FIRST_USABLE_SLOT] = Win95OverlayRecord {
            kind: RUST_TRAFFIC_ACTOR_KIND,
            state: OverlayState::LightActive as u8,
            tile_x: 40,
            tile_y: 40,
            sub_x: 8,
            sub_y: 8,
            lane: 1,
            ..Default::default()
        };
        city.set_traffic_overlays_for_test(table);
        city.set_building(40, 40, 29);
        city.set_traffic_density(40, 40, 0x39);
        city.set_tile_bits(40, 40, TileBits::new(0));
        let draws = collect_win95_traffic_draws(
            &city,
            |x, y| city.traffic_density_at(x, y).unwrap_or(0),
            &mut MacToolboxRng::new(1),
        );
        assert_eq!(draws.len(), 1);
        city.set_tile_bits(40, 40, TileBits::new(0x02));
        let rotated_draws = collect_win95_traffic_draws(
            &city,
            |x, y| city.traffic_density_at(x, y).unwrap_or(0),
            &mut MacToolboxRng::new(1),
        );
        assert_eq!(rotated_draws.len(), 1);
    }

    #[test]
    fn win_city_xthg_slots_decode_without_panic() {
        let Some(path) = crate::paths::test_win_city() else {
            return;
        };
        let city = City::load(path).expect("win city");
        let _ = uses_win95_overlay_draw(&city);
        let mut rng = MacToolboxRng::new(0x1234);
        let congestion = |x: usize, y: usize| city.traffic_density_at(x, y).unwrap_or(0);
        let _ = collect_win95_traffic_draws(&city, congestion, &mut rng);
    }

    #[test]
    fn loaded_pe_kind6_does_not_select_port_traffic_draw() {
        let mut city = City::new_blank(std::path::Path::new("/tmp/kind6-draw.sc2"), "Kind6Draw")
            .expect("blank city");
        let mut table = empty_overlay_table();
        let slot = XTHG_FIRST_USABLE_SLOT;
        table[slot] = Win95OverlayRecord {
            kind: OVERLAY_KIND_TRAFFIC, // PE XTHG_EXPLOSION byte
            state: OverlayState::LightActive as u8,
            lane: 0,
            tile_x: 40,
            tile_y: 40,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        city.set_traffic_overlays_loaded_for_test(table.clone());
        assert!(!city.is_port_traffic_record(table[slot]));
        assert!(!uses_win95_overlay_draw(&city));
        let mut rng = MacToolboxRng::new(1);
        assert!(collect_win95_traffic_draws(&city, |_, _| 0x40, &mut rng).is_empty());

        table[slot].kind = RUST_TRAFFIC_ACTOR_KIND;
        city.set_traffic_overlays_loaded_for_test(table.clone());
        assert!(city.is_port_traffic_record(table[slot]));
        assert!(uses_win95_overlay_draw(&city));

        // Blank/synthetic from_file=false still treats legacy 0x06 as port traffic.
        table[slot].kind = OVERLAY_KIND_TRAFFIC;
        city.set_traffic_overlays_for_test(table);
        assert!(uses_win95_overlay_draw(&city));
    }
}
