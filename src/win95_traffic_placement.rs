//! Win95 PE traffic overlay placement (`FUN_004516f0` @ `0x4516f0`).
//!
//! Tables at `SIMCITY.EXE` `0x004e80e0`, `0x004e80f0`, `0x004e81f8` — see
//! `outputs/disasm_win95_pe/sc2000_win95_traffic_trace_notes.md`.

/// Eight-way lane direction vectors indexed by `FUN_004515e0` orientation.
const LANE_OFFSET_X: [i16; 8] = [0, 1, 1, 1, 0, -1, -1, -1];
const LANE_OFFSET_Y: [i16; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

/// Per road-type scale byte (`mov dl, [road_type + 0x4e81f8]`).
const ROAD_PLACEMENT_SCALE: [u8; 256] = {
    let mut t = [0u8; 256];
    let pairs: &[(u8, u8)] = &[
        (29, 15),
        (30, 2),
        (31, 6),
        (32, 6),
        (33, 2),
        (34, 4),
        (35, 4),
        (36, 0),
        (37, 0),
        (38, 6),
        (39, 9),
        (40, 12),
        (41, 0),
        (42, 0),
        (43, 0),
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
    let mut i = 0;
    while i < pairs.len() {
        t[pairs[i].0 as usize] = pairs[i].1;
        i += 1;
    }
    t
};

/// Mac `export_80` @ `0x06cbe`: `mulu` tile factor × ALTM height before dash Y subtract.
/// At city rotation 0 the factor is 3 (`0x06906..0x06910`); viewer is half native scale.
const MAC_ALTM_Y_FACTOR: f32 = 1.5;

/// Win95 scales lane vectors on a 16×16 sub-tile grid; viewer roads are half native size.
const SUBTILE_TO_ROAD: f32 = 1.0 / 40.0;

pub fn road_placement_scale(building: u8) -> u8 {
    ROAD_PLACEMENT_SCALE[building as usize]
}

pub fn lane_index_for_tile(building: u8, x: usize, y: usize) -> usize {
    let parity = (x + y).is_multiple_of(2);
    match building {
        29 | 31 | 33 | 35 | 37 | 39 | 41 | 63..=66 | 69 | 70 | 73 | 75 | 77 | 79 => {
            if parity {
                1
            } else {
                5
            }
        }
        30 | 32 | 34 | 36 | 38 | 40 | 42 | 43 | 74 | 76 | 78 | 80 => {
            if parity {
                3
            } else {
                7
            }
        }
        _ => {
            if parity {
                1
            } else {
                5
            }
        }
    }
}

fn lane_vector(lane: usize) -> (f32, f32) {
    let lane = lane.min(7);
    (LANE_OFFSET_X[lane] as f32, LANE_OFFSET_Y[lane] as f32)
}

/// Lane direction for center-based dash spread (exported for Mac placement).
pub fn lane_spread_vector(lane: usize) -> (f32, f32) {
    lane_vector(lane)
}

pub fn is_east_west_road(building: u8) -> bool {
    matches!(
        building,
        29 | 31 | 33 | 35 | 37 | 39 | 41 | 63..=66 | 69 | 70 | 73 | 75 | 77 | 79
    )
}

/// Pixel offset from road top-left (+Y down) using Win95 scaled lane tables.
pub fn win95_lane_pixel_offset(
    building: u8,
    lane: usize,
    road_w: f32,
    road_h: f32,
    car_w: f32,
    car_h: f32,
) -> (f32, f32) {
    let scale = road_placement_scale(building) as f32;
    let (lx, ly) = lane_vector(lane);
    let nudge_x = lx * scale * road_w * SUBTILE_TO_ROAD;
    let nudge_y = ly * scale * road_h * SUBTILE_TO_ROAD;

    if is_east_west_road(building) {
        let x = road_w * 0.5 + nudge_x - car_w * 0.5;
        let y = road_h * 0.5 + nudge_y - car_h * 0.5;
        (x.clamp(0.0, road_w - car_w), y.clamp(0.0, road_h - car_h))
    } else {
        let x = road_w * 0.5 - car_w * 0.5;
        let y = road_h * 0.5 + nudge_y - car_h * 0.5;
        (x.clamp(0.0, road_w - car_w), y.clamp(0.0, road_h - car_h))
    }
}

/// Mac subtracts land ALTM (low 5 bits) from projected dash Y (`export_80` @ `0x072d4`).
pub fn mac_altm_y_adjustment(altm_land: u8) -> f32 {
    (altm_land & 0x1f) as f32 * MAC_ALTM_Y_FACTOR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn east_west_straight_lane_matches_legacy_right_inset() {
        let lane = lane_index_for_tile(29, 64, 56);
        assert_eq!(lane, 1);
        let (x, _) = win95_lane_pixel_offset(29, lane, 32.0, 17.0, 8.0, 4.0);
        assert!((x - 24.0).abs() < 1.0, "x={x}");
    }

    #[test]
    fn north_south_road_centers_horizontally() {
        let (x, _) = win95_lane_pixel_offset(30, 3, 32.0, 17.0, 8.0, 4.0);
        assert!((x - 12.0).abs() < 0.01);
    }
}
