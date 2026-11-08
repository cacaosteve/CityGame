//! Mac 68k SimCity 2000 rendering helpers derived from Ghidra disassembly
//! (`outputs/disasm_probe/mac_traffic_trace_notes.md`).

use crate::mac_shape_table;
use crate::traffic::is_full_tile_traffic_overlay;
use crate::win95_traffic_placement::lane_spread_vector;
use glam::Vec2;

pub const TILE_WIDTH_HALF: f32 = 16.0;

/// Mac shape-table blit cell (`FUN_000403ba`): `moveq 0x4d` / `moveq 0x32`.
pub const MAC_SHAPE_TILE_H: f32 = 50.0;
const STANDARD_ROAD_BAND_H: f32 = 17.0;
const ROAD_ASPHALT_INSET_X: f32 = 4.0;
const MAC_TRAFFIC_BASE_DASH_W: f32 = 8.0;
const MAC_TRAFFIC_BASE_DASH_H: f32 = 4.0;
const MAC_TRAFFIC_LANE_NUDGE_X: f32 = 2.0;
const MAC_TRAFFIC_LANE_NUDGE_Y: f32 = 1.0;

pub mod runtime_tile_flags {
    pub const DRAW_TRAFFIC: u8 = 2;
}

pub fn mac_shape_blit_offset(shape_w: f32, shape_h: f32) -> Vec2 {
    Vec2::new(
        0x4d as f32 - shape_w * 0.5 - 0x0f as f32,
        MAC_SHAPE_TILE_H - shape_h * 0.5,
    )
}

pub fn tile_top_left(map_anchor: Vec2, zoom: f32) -> Vec2 {
    Vec2::new(map_anchor.x - TILE_WIDTH_HALF * zoom, map_anchor.y)
}

/// Mac static `XTRF` dash offset from **road bottom-left** (+Y screen = down).
///
/// Separate from Win95 `XTHG` sub-tile placement in `win95_tile_draw.rs`. The Mac 68k
/// city-view path draws tiny static dashes from a fixed four-variant shape family. It does
/// not use the Win95 lane tables for this pass. `export_80` computes X directly from the
/// tile projection (`fdb4 + (row-col)*half_width`) and subtracts only the shape-table
/// `y_sub` from Y before calling the blitter.
///
/// The Mac global origin and this viewer's `projected_tile_units` origin do not use the same
/// bottom-left convention. Mapping the literal X to this renderer's road bbox left edge lands
/// the dash in the empty isometric corner/dirt. The equivalent visible blit is the road-asphalt
/// centre baseline: same Mac draw row, same random flip, only translated into this renderer's
/// bottom-left sprite anchor.
#[allow(clippy::too_many_arguments)]
pub fn traffic_overlay_offset_for_sprite(
    building: u8,
    lane: usize,
    sprite_id: u32,
    road_w: f32,
    road_h: f32,
    car_w: f32,
    car_h: f32,
    _altm_land: u8,
) -> Vec2 {
    if is_full_tile_traffic_overlay(car_w, car_h, road_w, road_h) {
        return Vec2::ZERO;
    }
    if is_mac_static_dash_sprite(sprite_id) && lane != 0 {
        return lane_positioned_dash_offset(building, lane, road_w, road_h, car_w, car_h);
    }
    if is_mac_static_dash_sprite(sprite_id) {
        return mac_export80_asphalt_offset(road_w, road_h, car_w, car_h);
    }
    mac_export80_asphalt_offset(road_w, road_h, car_w, car_h)
}

fn is_mac_static_dash_sprite(sprite_id: u32) -> bool {
    matches!(sprite_id, 400..=403 | 900..=903 | 1400..=1403)
}

fn lane_positioned_dash_offset(
    _building: u8,
    lane: usize,
    road_w: f32,
    road_h: f32,
    car_w: f32,
    car_h: f32,
) -> Vec2 {
    let base = mac_export80_asphalt_offset(road_w, road_h, car_w, car_h);
    let (lx, ly) = lane_spread_vector(lane);
    Vec2::new(
        base.x + lx * MAC_TRAFFIC_LANE_NUDGE_X,
        base.y - ly * MAC_TRAFFIC_LANE_NUDGE_Y,
    )
}

fn mac_export80_asphalt_offset(road_w: f32, road_h: f32, car_w: f32, car_h: f32) -> Vec2 {
    let placement_h = if road_h > 20.0 {
        STANDARD_ROAD_BAND_H
    } else {
        road_h
    };

    let x_min = ROAD_ASPHALT_INSET_X;
    let x_max = (road_w - car_w - ROAD_ASPHALT_INSET_X).max(x_min);
    let x = (road_w * 0.5 - car_w * 0.5).clamp(x_min, x_max);

    // `export_80` variants 400/401 have y_sub == height == 4. The taller variants share
    // the same projected bottom baseline and extend upward from it.
    let max_y_from_bottom = (placement_h - car_h).max(0.0);
    let y_from_bottom =
        ((placement_h - MAC_TRAFFIC_BASE_DASH_H) * 0.5).clamp(0.0, max_y_from_bottom);
    Vec2::new(x, -y_from_bottom)
}

#[allow(clippy::too_many_arguments)]
pub fn traffic_bottom_left_for_sprite(
    map_anchor: Vec2,
    zoom: f32,
    building: u8,
    lane: usize,
    road_w: f32,
    road_h: f32,
    car_w: f32,
    car_h: f32,
    sprite_id: u32,
    altm_land: u8,
) -> Vec2 {
    let road_bl = Vec2::new(map_anchor.x - road_w * 0.5 * zoom, map_anchor.y);
    let offset = traffic_overlay_offset_for_sprite(
        building, lane, sprite_id, road_w, road_h, car_w, car_h, altm_land,
    );
    road_bl + offset * zoom
}

/// Mac `export_80` traffic shape bottom-left from the recovered 0x055a contract.
///
/// The disassembly computes a projected `screen_x` and a `screen_y` that has already had
/// `shape_table[draw_row].y_sub` subtracted. The renderer wants bottom-left, so convert from
/// the shape blit point using the same shape-table record instead of re-centering in the road
/// bitmap. This keeps traffic tied to the Mac projection, not a viewer road-lane heuristic.
pub fn export80_traffic_bottom_left(map_anchor: Vec2, zoom: f32, sprite_id: u32) -> Vec2 {
    let record = mac_shape_table::lookup(sprite_id);
    let x_sub = record.map(|record| record.x_sub).unwrap_or(0.0);
    let height = record.map(|record| record.height as f32).unwrap_or(4.0);
    let y_sub = record.map(|record| record.y_sub).unwrap_or(height);
    Vec2::new(
        map_anchor.x - x_sub * zoom,
        map_anchor.y - (height + y_sub) * zoom,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_table_offset_matches_disassembly() {
        assert_eq!(mac_shape_blit_offset(8.0, 4.0), Vec2::new(58.0, 48.0));
    }

    #[test]
    fn top_left_from_map_anchor() {
        let map = Vec2::new(32.0, -56.0);
        assert_eq!(tile_top_left(map, 2.0), Vec2::new(0.0, -56.0));
    }

    #[test]
    fn full_tile_overlay_aligns_with_road_origin() {
        let offset = traffic_overlay_offset_for_sprite(29, 1, 1400, 32.0, 17.0, 32.0, 17.0, 0);
        assert_eq!(offset, Vec2::ZERO);
    }

    #[test]
    fn mac_static_dash_uses_export80_asphalt_anchor() {
        let offset = traffic_overlay_offset_for_sprite(29, 0, 400, 32.0, 17.0, 8.0, 4.0, 0);
        assert_eq!(offset, Vec2::new(12.0, -6.5));
    }

    #[test]
    fn mac_static_dash_uses_small_lane_nudges_not_win95_spread() {
        let one = traffic_overlay_offset_for_sprite(29, 1, 400, 32.0, 17.0, 8.0, 4.0, 0);
        let five = traffic_overlay_offset_for_sprite(29, 5, 400, 32.0, 17.0, 8.0, 4.0, 0);
        assert_eq!(one, Vec2::new(14.0, -5.5));
        assert_eq!(five, Vec2::new(10.0, -7.5));
    }

    #[test]
    fn tall_diagonal_road_uses_same_export80_origin() {
        let short = traffic_overlay_offset_for_sprite(31, 0, 402, 32.0, 29.0, 8.0, 7.0, 0);
        let tall = traffic_overlay_offset_for_sprite(31, 0, 402, 32.0, 29.0, 8.0, 7.0, 0);
        assert_eq!(short, tall);
        assert_eq!(tall, Vec2::new(12.0, -6.5));
    }

    #[test]
    fn north_south_road_uses_export80_asphalt_anchor() {
        let offset = traffic_overlay_offset_for_sprite(30, 0, 400, 32.0, 17.0, 8.0, 4.0, 0);
        assert_eq!(offset, Vec2::new(12.0, -6.5));
    }

    #[test]
    fn taller_traffic_sprite_keeps_same_bottom_anchor() {
        let straight = traffic_overlay_offset_for_sprite(31, 0, 400, 32.0, 17.0, 8.0, 4.0, 0);
        let curve = traffic_overlay_offset_for_sprite(31, 0, 402, 32.0, 17.0, 8.0, 7.0, 0);
        assert_eq!(curve.y, straight.y);
    }

    #[test]
    fn traffic_bottom_left_sits_on_asphalt_row() {
        let map = Vec2::new(16.0, -28.0);
        let bl = traffic_bottom_left_for_sprite(map, 1.0, 29, 0, 32.0, 17.0, 8.0, 4.0, 400, 0);
        assert_eq!(bl.x, 12.0);
        assert_eq!(bl.y, -34.5);
    }

    #[test]
    fn export80_static_dash_uses_shape_table_anchor() {
        let map = Vec2::new(16.0, -28.0);
        assert_eq!(
            export80_traffic_bottom_left(map, 1.0, 400),
            Vec2::new(12.0, -36.0)
        );
        assert_eq!(
            export80_traffic_bottom_left(map, 1.0, 402),
            Vec2::new(12.0, -42.0)
        );
    }

    #[test]
    fn dash_does_not_apply_viewer_lane_margin() {
        let offset = traffic_overlay_offset_for_sprite(31, 0, 402, 32.0, 29.0, 8.0, 7.0, 0);
        assert_eq!(offset, Vec2::new(12.0, -6.5));
    }

    #[test]
    fn road_families_share_same_centered_export80_anchor() {
        assert_eq!(
            traffic_overlay_offset_for_sprite(35, 0, 400, 32.0, 17.0, 8.0, 4.0, 0),
            Vec2::new(12.0, -6.5)
        );
        assert_eq!(
            traffic_overlay_offset_for_sprite(37, 0, 400, 32.0, 17.0, 8.0, 4.0, 0),
            Vec2::new(12.0, -6.5)
        );
        assert_eq!(
            traffic_overlay_offset_for_sprite(38, 0, 400, 32.0, 17.0, 8.0, 4.0, 0),
            Vec2::new(12.0, -6.5)
        );
    }

    #[test]
    fn bridge_and_highway_dash_anchors_use_same_centered_band() {
        assert_eq!(
            traffic_overlay_offset_for_sprite(63, 0, 400, 32.0, 29.0, 8.0, 4.0, 0),
            Vec2::new(12.0, -6.5)
        );
        assert_eq!(
            traffic_overlay_offset_for_sprite(73, 0, 400, 32.0, 31.0, 8.0, 4.0, 0),
            Vec2::new(12.0, -6.5)
        );
        assert_eq!(
            traffic_overlay_offset_for_sprite(97, 0, 400, 64.0, 59.0, 8.0, 4.0, 0),
            Vec2::new(28.0, -6.5)
        );
        assert_eq!(
            traffic_overlay_offset_for_sprite(107, 0, 400, 64.0, 63.0, 8.0, 4.0, 0),
            Vec2::new(28.0, -6.5)
        );
    }

    #[test]
    fn banked_dash_art_stays_centered_on_same_asphalt_anchor() {
        let small = traffic_overlay_offset_for_sprite(29, 0, 400, 32.0, 17.0, 8.0, 4.0, 0);
        let medium = traffic_overlay_offset_for_sprite(29, 0, 900, 32.0, 17.0, 16.0, 9.0, 0);

        assert_eq!(small + Vec2::new(4.0, -2.0), Vec2::new(16.0, -8.5));
        assert_eq!(medium + Vec2::new(8.0, -4.5), Vec2::new(16.0, -11.0));
    }
}
