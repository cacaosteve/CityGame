use super::*;

#[test]
fn mac_actor_route_candidates_match_original_68020() {
    let mut city = City::new_blank("/tmp/mac-route-candidates.sc2", "Routes").unwrap();
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_candidates.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<i32> = line.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 7);
        if (0..128).contains(&v[0]) && (0..128).contains(&v[1]) {
            let (x, y) = (v[0] as usize, v[1] as usize);
            city.set_building(x, y, v[3] as u8);
            city.set_underground(x, y, v[4] as u8);
            city.set_text_overlay(x, y, v[5] as u8);
        }
        assert_eq!(
            mac_actor_route_candidate(&city, v[0] as i16, v[1] as i16, v[2] as u16),
            v[6] != 0,
            "{line}"
        );
        count += 1;
    }
    assert_eq!(count, 1292);
}

#[test]
fn mac_rail_teardown_predicate_matches_original_68020() {
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_teardown.csv");
    let mut city = City::new_blank("/tmp/mac-rail-teardown.sc2", "Rail").unwrap();
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<i32> = line.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 5);
        if (0..128).contains(&v[0]) && (0..128).contains(&v[1]) {
            let (x, y) = (v[0] as usize, v[1] as usize);
            city.set_building(x, y, v[2] as u8);
            city.set_underground(x, y, v[3] as u8);
        }
        assert_eq!(
            mac_rail_current_cell_requires_teardown(&city, v[0] as i16, v[1] as i16),
            v[4] != 0,
            "{line}"
        );
        count += 1;
    }
    assert_eq!(count, 516);
}

#[test]
fn mac_actor_alternate_route_reads_underground_not_zone() {
    let mut city = City::new_blank("/tmp/mac-route-layer.sc2", "Layers").unwrap();
    city.set_zone(30, 50, 1);
    city.set_underground(30, 50, 0);
    assert!(!mac_actor_route_candidate(&city, 30, 50, 12));
    city.set_zone(30, 50, 0);
    city.set_underground(30, 50, 1);
    assert!(mac_actor_route_candidate(&city, 30, 50, 12));
}

#[test]
fn mac_actor_route_chooser_matches_original_68020_and_draw_count() {
    let mut city = City::new_blank("/tmp/mac-route-chooser.sc2", "Routes").unwrap();
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_chooser.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<i64> = line.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 19);
        for (direction, (dx, dy)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
            let (row, col) = (v[0] + dx, v[1] + dy);
            if (0..128).contains(&row) && (0..128).contains(&col) {
                let (x, y) = (row as usize, col as usize);
                city.set_building(x, y, v[5 + 3 * direction] as u8);
                city.set_underground(x, y, v[6 + 3 * direction] as u8);
                city.set_text_overlay(x, y, v[7 + 3 * direction] as u8);
            }
        }
        let mut draws = 0;
        let result = mac_actor_choose_route(
            &city,
            v[0] as i16,
            v[1] as i16,
            v[2] as i16,
            v[3] as u16,
            || {
                draws += 1;
                v[4] as u16
            },
        );
        let expected = (v[17] != i64::from(u32::MAX)).then_some(v[17] as u8);
        assert_eq!(result, expected, "{line}");
        assert_eq!(draws, v[18], "{line}");
        count += 1;
    }
    assert_eq!(count, 160);
}
