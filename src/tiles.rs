use std::collections::HashMap;
use std::sync::LazyLock;

use crate::city::{City, TileBits};

pub fn rotated_building_id(building: u8, rotation: u8) -> u8 {
    INFRA_ROTATE
        .get(&building)
        .map(|rotate| rotate[(rotation & 0x03) as usize])
        .unwrap_or(building)
}

/// Rotate terrain artwork for the view without changing the stored XTER map.
/// The three four-facing slope families are shared by dry land, underwater,
/// shorelines, and surface water. Flat tiles and waterfalls are symmetric.
pub fn rotated_terrain_id(terrain: u8, rotation: u8) -> u8 {
    let turn = rotation & 3;
    match terrain {
        0x00..=0x0d | 0x10..=0x1d | 0x20..=0x2d | 0x30..=0x3d => {
            let shape = terrain & 0x0f;
            if (1..=12).contains(&shape) {
                let family = ((shape - 1) / 4) * 4 + 1;
                (terrain & 0xf0) | (family + (shape - family + turn) % 4)
            } else {
                terrain
            }
        }
        0x40..=0x41 => 0x40 + ((terrain - 0x40 + turn) & 1),
        0x42..=0x45 => 0x42 + ((terrain - 0x42 + turn) & 3),
        _ => terrain,
    }
}

static INFRA_ROTATE: LazyLock<HashMap<u8, [u8; 4]>> = LazyLock::new(|| {
    HashMap::from([
        (14, [14, 15, 14, 15]),
        (15, [15, 14, 15, 14]),
        (16, [16, 17, 18, 19]),
        (17, [17, 18, 19, 16]),
        (18, [18, 19, 16, 17]),
        (19, [19, 16, 17, 18]),
        (20, [20, 21, 22, 23]),
        (21, [21, 22, 23, 20]),
        (22, [22, 23, 20, 21]),
        (23, [23, 20, 21, 22]),
        (24, [24, 25, 26, 27]),
        (25, [25, 26, 27, 24]),
        (26, [26, 27, 24, 25]),
        (27, [27, 24, 25, 26]),
        (29, [29, 30, 29, 30]),
        (30, [30, 29, 30, 29]),
        (31, [31, 32, 33, 34]),
        (32, [32, 33, 34, 31]),
        (33, [33, 34, 31, 32]),
        (34, [34, 31, 32, 33]),
        (35, [35, 36, 37, 38]),
        (36, [36, 37, 38, 35]),
        (37, [37, 38, 35, 36]),
        (38, [38, 35, 36, 37]),
        (39, [39, 40, 41, 42]),
        (40, [40, 41, 42, 39]),
        (41, [41, 42, 39, 40]),
        (42, [42, 39, 40, 41]),
        (44, [44, 45, 44, 45]),
        (45, [45, 44, 45, 44]),
        (46, [46, 47, 48, 49]),
        (47, [47, 48, 49, 46]),
        (48, [48, 49, 46, 47]),
        (49, [49, 46, 47, 48]),
        (50, [50, 51, 52, 53]),
        (51, [51, 52, 53, 50]),
        (52, [52, 53, 50, 51]),
        (53, [53, 50, 51, 52]),
        (54, [54, 55, 56, 57]),
        (55, [55, 56, 57, 54]),
        (56, [56, 57, 54, 55]),
        (57, [57, 54, 55, 56]),
        (59, [59, 60, 61, 62]),
        (60, [60, 61, 62, 59]),
        (61, [61, 62, 59, 60]),
        (62, [62, 59, 60, 61]),
        (63, [63, 64, 65, 66]),
        (64, [64, 65, 66, 63]),
        (65, [65, 66, 63, 64]),
        (66, [66, 63, 64, 65]),
        (67, [67, 68, 67, 68]),
        (68, [68, 67, 68, 67]),
        (69, [69, 70, 69, 70]),
        (70, [70, 69, 70, 69]),
        (71, [71, 72, 71, 72]),
        (72, [72, 71, 72, 71]),
        (73, [73, 74, 73, 74]),
        (74, [74, 73, 74, 73]),
        (75, [75, 76, 75, 76]),
        (76, [76, 75, 76, 75]),
        (77, [77, 78, 77, 78]),
        (78, [78, 77, 78, 77]),
        (79, [79, 80, 79, 80]),
        (80, [80, 79, 80, 79]),
        (81, [81, 85, 81, 85]),
        (82, [82, 84, 82, 84]),
        (83, [83, 83, 83, 83]),
        (84, [84, 82, 84, 82]),
        (85, [85, 81, 85, 81]),
        (93, [93, 96, 93, 96]),
        (94, [94, 95, 94, 95]),
        (95, [95, 94, 95, 94]),
        (96, [96, 93, 96, 93]),
        (97, [97, 98, 99, 100]),
        (98, [98, 99, 100, 97]),
        (99, [99, 100, 97, 98]),
        (100, [100, 97, 98, 99]),
        (101, [101, 102, 103, 104]),
        (102, [102, 103, 104, 101]),
        (103, [103, 104, 101, 102]),
        (104, [104, 101, 102, 103]),
        (108, [108, 109, 110, 111]),
        (109, [109, 110, 111, 108]),
        (110, [110, 111, 108, 109]),
        (111, [111, 108, 109, 110]),
    ])
});

pub fn building_uses_xbit_flip(building: u8) -> bool {
    // Straight/diagonal roads (29–62) encode orientation in XBLD; only bridge/on-ramp
    // pieces and elevated tiles use per-tile XBIT rotation (SC2k-docs bit 6).
    matches!(
        building,
        63..=66 | 69 | 70 | 81..=89 | 93..=96 | 106 | 107 | 221 | 223
    )
}

pub fn building_uses_xbit_pier_runway_rotate(building: u8) -> bool {
    matches!(building, 221 | 223)
}

/// Bridge and highway-bridge segments share one sprite set per piece. Their sprites are authored
/// for a "/" (grid-y, SW–NE) span, so a grid-x span must be horizontally flipped. On-ramps are
/// excluded because their perpendicular highway and road neighbors make the run ambiguous; they
/// use the saved XBIT rotate flag instead.
pub fn building_uses_bridge_axis_flip(building: u8) -> bool {
    matches!(building, 63..=66 | 69 | 70 | 81..=89 | 106 | 107)
}

pub fn xbit_sprite_flip_x_for(
    building: u8,
    city_rotation: u8,
    tile_bits: Option<TileBits>,
) -> bool {
    let toggled = tile_bits.is_some_and(TileBits::rotate);
    if building_uses_xbit_pier_runway_rotate(building) {
        return toggled == (city_rotation & 1 != 0);
    }
    building_uses_xbit_flip(building) && toggled
}

pub fn xbit_sprite_flip_x(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    if building_uses_bridge_axis_flip(building) {
        // The grid axis a bridge spans is intrinsic, but the diagonal it occupies on screen
        // swaps ("\" <-> "/") at the odd view rotations, so invert the flip there.
        let view_swaps_axis = city.rotation() & 1 == 1;
        return bridge_runs_along_grid_x(city, x, y) ^ view_swaps_axis;
    }
    xbit_sprite_flip_x_for(building, city.rotation(), city.tile_bits_at(x, y))
}

/// A bridge tile spans grid-x when its road/bridge continuation lies along the x-axis.
fn bridge_runs_along_grid_x(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    let along_x = bridge_continuation(city, x.wrapping_sub(1), y, building)
        || bridge_continuation(city, x + 1, y, building);
    let along_y = bridge_continuation(city, x, y.wrapping_sub(1), building)
        || bridge_continuation(city, x, y + 1, building);
    if matches!(building, 81..=89) && along_x == along_y {
        // Parallel roads and isolated surviving bridge tiles can still leave the
        // connectivity probe ambiguous. XBIT bit 1 stores the bridge's own axis.
        return city.tile_bits_at(x, y).is_some_and(TileBits::rotate);
    }
    along_x && !along_y
}

fn bridge_continuation(city: &City, x: usize, y: usize, building: u8) -> bool {
    if x >= crate::city::CITY_WIDTH || y >= crate::city::CITY_HEIGHT {
        return false;
    }
    let neighbor = city.building_at(x, y);
    if matches!(building, 81..=89) {
        // A road bridge beside a highway or railway continues along its own road,
        // not across to the neighboring transport network (Charleston/Drum Island).
        return matches!(neighbor, 29..=43 | 63..=66 | 69 | 70 | 81..=89);
    }
    matches!(neighbor, 29..=96 | 106..=111)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths;

    #[test]
    fn pier_runway_flip_xor_city_rotation_parity() {
        assert!(xbit_sprite_flip_x_for(223, 0, Some(TileBits::new(0))));
        assert!(!xbit_sprite_flip_x_for(
            223,
            0,
            Some(TileBits::new(0b0000_0010))
        ));
    }

    #[test]
    fn rotates_road_tiles_with_city_rotation() {
        assert_eq!(rotated_building_id(29, 0), 29);
        assert_eq!(rotated_building_id(29, 1), 30);
        assert_eq!(rotated_building_id(63, 2), 65);
    }

    #[test]
    fn straight_roads_do_not_use_xbit_flip() {
        assert!(!building_uses_xbit_flip(29));
        assert!(!building_uses_xbit_flip(31));
        assert!(building_uses_xbit_flip(63));
    }

    #[test]
    fn highway_bridge_tiles_use_bridge_axis_flip() {
        assert!(building_uses_bridge_axis_flip(106));
        assert!(building_uses_bridge_axis_flip(107));
    }

    #[test]
    fn parallel_road_bridge_ignores_highway_and_railway_neighbors() {
        for neighbor in [74, 90, 92, 106] {
            let mut city = City::new_blank("/tmp/parallel-bridges.sc2", "Bridges").unwrap();
            for x in 19..=21 {
                city.set_building(x, 30, 87);
                city.set_tile_bits(x, 30, TileBits::new(0x06));
                city.set_building(x, 31, neighbor);
            }
            for rotation in 0..4 {
                city.set_view_rotation(rotation);
                assert_eq!(
                    xbit_sprite_flip_x(&city, 20, 30),
                    rotation & 1 == 0,
                    "neighbor={neighbor} rotation={rotation}"
                );
            }
        }
    }

    #[test]
    fn ambiguous_or_isolated_road_bridge_retains_saved_axis() {
        for rotate in [false, true] {
            let mut city = City::new_blank("/tmp/bridge-axis.sc2", "BridgeAxis").unwrap();
            city.set_building(20, 30, 88);
            city.set_tile_bits(20, 30, TileBits::new(0x04).with_rotate(rotate));
            for neighbors in [false, true] {
                if neighbors {
                    for (x, y) in [(19, 30), (21, 30), (20, 29), (20, 31)] {
                        city.set_building(x, y, 43);
                    }
                }
                for rotation in 0..4 {
                    city.set_view_rotation(rotation);
                    assert_eq!(
                        xbit_sprite_flip_x(&city, 20, 30),
                        rotate ^ (rotation & 1 != 0)
                    );
                }
            }
        }
    }

    #[test]
    fn charleston_parallel_road_bridge_matches_saved_axis_in_both_formats() {
        let mut fixtures = Vec::new();
        if let Some(root) = paths::test_win95_root() {
            fixtures.push(root.join("SCENARIO/CHARLEST.SCN"));
        }
        if let Some(path) = paths::test_city_from_env("SC2000_TEST_MAC_SCENARIO") {
            fixtures.push(path);
        }
        for path in fixtures {
            let mut city = City::load(&path).unwrap();
            for rotation in 0..4 {
                city.set_view_rotation(rotation);
                for x in 78..=82 {
                    assert!(matches!(city.building_at(x, 57), 86..=88));
                    assert_eq!(
                        xbit_sprite_flip_x(&city, x, 57),
                        rotation & 1 == 0,
                        "{} tile=({x},57) rotation={rotation}",
                        path.display()
                    );
                }
            }
        }
    }

    #[test]
    fn leaves_non_infrastructure_untouched() {
        assert_eq!(rotated_building_id(112, 2), 112);
        assert_eq!(rotated_building_id(207, 3), 207);
        assert!(!building_uses_xbit_flip(207));
    }

    #[test]
    fn cape_wells_straight_road_orientation_probe() {
        use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

        let Some(path) = paths::test_win_cape_city() else {
            return;
        };
        let city = City::load(path).expect("cape");
        let rotation = city.rotation();

        let mut stored_lr = 0u32;
        let mut stored_tb = 0u32;
        let mut x_axis_lr = 0u32;
        let mut y_axis_tb = 0u32;
        let mut x_axis_stored_tb = 0u32;
        let mut y_axis_stored_lr = 0u32;
        let mut display_mismatch = 0u32;

        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let raw = city.building_at(x, y);
                if raw != 29 && raw != 30 {
                    continue;
                }
                if raw == 29 {
                    stored_lr += 1;
                } else {
                    stored_tb += 1;
                }

                let x_conn =
                    road_neighbor(&city, x.wrapping_sub(1), y) || road_neighbor(&city, x + 1, y);
                let y_conn =
                    road_neighbor(&city, x, y.wrapping_sub(1)) || road_neighbor(&city, x, y + 1);

                if x_conn && !y_conn {
                    x_axis_lr += 1;
                    if raw == 30 {
                        x_axis_stored_tb += 1;
                    }
                } else if y_conn && !x_conn {
                    y_axis_tb += 1;
                    if raw == 29 {
                        y_axis_stored_lr += 1;
                    }
                }

                // Ground truth from the known-good Maxis (rotation 0) render: a road
                // connecting along the grid x-axis displays sprite 30, grid y-axis displays 29.
                let displayed = rotated_building_id(raw, rotation);
                let expected_display = if x_conn && !y_conn {
                    30
                } else if y_conn && !x_conn {
                    29
                } else {
                    displayed
                };
                if displayed != expected_display && (x_conn ^ y_conn) {
                    display_mismatch += 1;
                }
            }
        }

        eprintln!(
            "CAPE rotation={rotation} stored LR={stored_lr} TB={stored_tb} \
             x-axis={x_axis_lr} y-axis={y_axis_tb} \
             x-axis-but-TB={x_axis_stored_tb} y-axis-but-LR={y_axis_stored_lr} \
             display_mismatch={display_mismatch}"
        );

        // After applying the city rotation, every straight road's displayed sprite must
        // match the axis it physically connects along in the saved grid. If this drifts,
        // roads render perpendicular to their run (the "opposite way" Cape Wells bug).
        assert_eq!(
            display_mismatch, 0,
            "rotated road sprite ids disagree with grid connectivity"
        );
    }

    fn road_neighbor(city: &crate::city::City, x: usize, y: usize) -> bool {
        use crate::city::{CITY_HEIGHT, CITY_WIDTH};
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return false;
        }
        matches!(city.building_at(x, y), 29..=43 | 63..=66 | 69 | 70 | 81..=89)
    }

    #[test]
    fn cape_bridge_tile_probe() {
        use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

        let Some(p) = paths::test_win_cape_city() else {
            return;
        };
        let city = City::load(&p).expect("cape");
        // id -> (count, sample xbit-rotate-true count, sample (x,y))
        let mut counts: std::collections::BTreeMap<u8, (u32, u32, (usize, usize))> =
            Default::default();
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let b = city.building_at(x, y);
                if matches!(b, 63..=96) {
                    let e = counts.entry(b).or_insert((0, 0, (x, y)));
                    e.0 += 1;
                    if city
                        .tile_bits_at(x, y)
                        .is_some_and(crate::city::TileBits::rotate)
                    {
                        e.1 += 1;
                    }
                }
            }
        }
        for (id, (n, rot, sample)) in &counts {
            eprintln!("bridge id={id} count={n} xbit_rotate_true={rot} sample={sample:?}");
        }

        // Raw XBIT bytes along the grid-x suspension bridge run (y=53) and a grid-y road column.
        for x in 70..=78 {
            let b = city.building_at(x, 53);
            let xbit = city.tile_bits_at(x, 53).map(|t| t.raw_bits()).unwrap_or(0);
            eprintln!("bridge-run ({x},53) id={b} xbit=0b{xbit:08b}");
        }
        for (label, sx, sy) in [("x-road", 60usize, 60usize), ("y-road", 60, 61)] {
            let b = city.building_at(sx, sy);
            let xbit = city.tile_bits_at(sx, sy).map(|t| t.raw_bits()).unwrap_or(0);
            eprintln!("{label} ({sx},{sy}) id={b} xbit=0b{xbit:08b}");
        }
    }

    #[test]
    fn axis_to_displayed_sprite_compare_cities() {
        use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

        let cities = [
            paths::test_mac_city(),
            paths::test_city_from_env("SC2000_TEST_DOWNLOADED_MAC_CITY"),
            paths::test_win_cape_city(),
            paths::test_win_populated_city(),
        ];

        for p in cities.into_iter().flatten() {
            let city = City::load(&p).expect("city");
            let rotation = city.rotation();
            // displayed_sprite -> (count on x-axis road, count on y-axis road)
            let mut x_axis: std::collections::BTreeMap<u8, u32> = Default::default();
            let mut y_axis: std::collections::BTreeMap<u8, u32> = Default::default();

            for y in 0..CITY_HEIGHT {
                for x in 0..CITY_WIDTH {
                    let raw = city.building_at(x, y);
                    if raw != 29 && raw != 30 {
                        continue;
                    }
                    let x_conn = road_neighbor(&city, x.wrapping_sub(1), y)
                        || road_neighbor(&city, x + 1, y);
                    let y_conn = road_neighbor(&city, x, y.wrapping_sub(1))
                        || road_neighbor(&city, x, y + 1);
                    let disp = rotated_building_id(raw, rotation);
                    if x_conn && !y_conn {
                        *x_axis.entry(disp).or_default() += 1;
                    } else if y_conn && !x_conn {
                        *y_axis.entry(disp).or_default() += 1;
                    }
                }
            }
            let label = p.file_name().unwrap().to_string_lossy().to_string();
            eprintln!(
                "{label} rotation={rotation} x-axis-sprites={x_axis:?} y-axis-sprites={y_axis:?}"
            );

            // Regardless of the file's saved rotation, the active view renders north-up, so
            // every straight road must display the same sprite for a given grid axis.
            let x_ids: Vec<u8> = x_axis.keys().copied().collect();
            let y_ids: Vec<u8> = y_axis.keys().copied().collect();
            if !x_ids.is_empty() {
                assert_eq!(
                    x_ids,
                    vec![30],
                    "{label}: grid x-axis roads must display sprite 30"
                );
            }
            if !y_ids.is_empty() {
                assert_eq!(
                    y_ids,
                    vec![29],
                    "{label}: grid y-axis roads must display sprite 29"
                );
            }
        }
    }
}
