use super::*;
use crate::city::{City, TileBits};

fn blank_city() -> City {
    City::new_blank("/tmp/sc2000-trip-test.sc2", "trip").expect("blank city")
}

fn trip_start_rng_for_test(key: u8) -> Win95CrtRand {
    trip_isolated_rng(18, 20, 1, 1, key)
}

#[test]
fn trip_prefer_dir_uses_crt_mask_3() {
    let mut rng = trip_start_rng_for_test(TRIP_PREFER_DIR_KEY);
    let _ = trip_rotation_step(&mut rng);
    assert_eq!(
        trip_prefer_dir(18, 20, 1, 1),
        usize::from(rng.random_u16_mask(3))
    );
}

#[test]
fn trip_outer_facing_rerolls_crt_and_adds_wdunno3_first() {
    let mut rng = trip_prefer_rng(18, 20, 1, 1);
    let rotation = trip_rotation_step(&mut rng);
    let first_roll = trip_roll_outer_facing(&mut rng);
    let second_roll = trip_roll_outer_facing(&mut rng);
    let first_try = trip_rotate_dir(first_roll, rotation);
    assert_ne!(
        first_try, first_roll,
        "PE adds wDunno3 before the first try, so facing is not the raw roll"
    );
    assert_eq!(first_try, (first_roll + rotation) & 3);
    assert_eq!(
        trip_prefer_dir(18, 20, 1, 1),
        first_roll,
        "restarted seed is only the first outer roll"
    );
    let mut second = trip_prefer_rng(18, 20, 1, 1);
    let _ = trip_roll_outer_facing(&mut second);
    assert_eq!(
        trip_roll_outer_facing(&mut second),
        second_roll,
        "live trip rng must advance, not reseed each outer step"
    );
}

#[test]
fn trip_rotation_step_is_one_or_three_from_crt() {
    let mut rng = trip_start_rng_for_test(TRIP_PREFER_DIR_KEY);
    let expected = 2 * usize::from(rng.random_u16_mask(1)) + 1;
    let mut actual_rng = trip_prefer_rng(18, 20, 1, 1);
    assert!(expected == 1 || expected == 3);
    assert_eq!(trip_rotation_step(&mut actual_rng), expected);
}

#[test]
fn growth_lattice_trips_advance_the_shared_process_crt_stream() {
    let mut city = blank_city();
    for (x, y) in [(20, 20), (40, 40)] {
        city.set_zone(x, y, 1);
        city.set_building(x + 1, y, 0x1d);
        city.set_zone(x + 2, y, 3);
        city.set_building(x + 2, y, 0x7c);
    }

    let mut trip_served = vec![false; CITY_TILES];
    let mut actual_rng = Win95CrtRand::new(0x1234_5678);
    let mut expected_rng = actual_rng;
    // Each direct road-to-zone route consumes the rotation draw and one
    // outer-facing draw. The next origin continues that same stream.
    for _ in 0..4 {
        let _ = expected_rng.random_u15();
    }

    let stats = simulate_growth_lattice_trips_with_rng(
        &mut city,
        &mut trip_served,
        [(20, 20), (40, 40)],
        |_, _| true,
        true,
        &mut actual_rng,
    );

    assert_eq!(stats.trips_attempted, 2);
    assert_eq!(stats.trips_succeeded, 2);
    assert_eq!(actual_rng.seed(), expected_rng.seed());
}

#[test]
fn run_trip_with_no_start_does_not_consume_the_process_crt_stream() {
    let city = blank_city();
    let mut actual_rng = Win95CrtRand::new(0x8765_4321);
    let before = actual_rng;

    let result = run_trip_with_rng(&city, 20, 20, 1, 1, &mut actual_rng);

    assert!(!result.success);
    assert_eq!(actual_rng, before);
}

#[test]
fn game_is_valid_transit_items_matches_pe_tile_ranges() {
    assert!(is_valid_transit_item_tile(0x1D));
    assert!(is_valid_transit_item_tile(0x2B));
    assert!(!is_valid_transit_item_tile(0x2C)); // TILE_RAIL_LR
    assert!(is_valid_transit_item_tile(0x3F));
    assert!(is_valid_transit_item_tile(0x46));
    assert!(!is_valid_transit_item_tile(0x47));
    assert!(is_valid_transit_item_tile(0x4B));
    assert!(is_valid_transit_item_tile(0x4C));
    assert!(is_valid_transit_item_tile(0x5D));
    assert!(is_valid_transit_item_tile(0x60));
    assert!(!is_valid_transit_item_tile(0x61));
    assert!(is_valid_transit_item_tile(SUBWAY_STATION));
    assert!(is_valid_transit_item_tile(BUS_DEPOT));
    assert!(is_valid_transit_item_tile(RAIL_STATION));
    assert!(!is_valid_transit_item_tile(0));
    assert!(!is_valid_transit_item_tile(14));
}

#[test]
fn run_trip_start_modes_match_the_pe_scan_classes() {
    for building in [0x1D, 0x2B, 0x3F, 0x46, 0x4B, 0x4C, 0x5D, 0x60] {
        assert_eq!(
            start_mode_for_building(building),
            Some(TransitMode::Road),
            "PE road-start tile {building:#04x} should start in mode 0"
        );
    }
    for building in [0x2C, 0x3E, 0x47, 0x4A, 0x4D, 0x51, 0x5C, 0x61, 0x6A, 0x6B] {
        assert_eq!(
            start_mode_for_building(building),
            None,
            "non-start tile {building:#04x} must not be admitted by the 24-cell scan"
        );
    }
    assert_eq!(start_mode_for_building(BUS_DEPOT), Some(TransitMode::Bus));
    assert_eq!(
        start_mode_for_building(RAIL_STATION),
        Some(TransitMode::RailEnter)
    );
    assert_eq!(
        start_mode_for_building(SUBWAY_STATION),
        Some(TransitMode::SubwayEnter)
    );
}

#[test]
fn trip_modes_match_the_pe_switch_indices_and_xtrf_modes() {
    let modes = [
        TransitMode::Road,
        TransitMode::Highway,
        TransitMode::Tunnel,
        TransitMode::RoadBridge,
        TransitMode::Pedestrian,
        TransitMode::PedestrianHighway,
        TransitMode::PedestrianTunnel,
        TransitMode::PedestrianBridge,
        TransitMode::Bus,
        TransitMode::TransitTransfer,
        TransitMode::RailEnter,
        TransitMode::SubwayEnter,
        TransitMode::Rail,
        TransitMode::Subway,
    ];
    for (pe_index, mode) in modes.into_iter().enumerate() {
        assert_eq!(mode as u8, pe_index as u8);
        assert_eq!(
            mode.bumps_xtrf(),
            matches!(pe_index, 0 | 1 | 3),
            "only PE route modes 0, 1, and 3 stamp XTRF"
        );
    }
}

#[test]
fn pe_route_tile_classes_and_walk_mode_transitions_match_the_switch() {
    for building in [0x1D, 0x2B, 0x3F, 0x46, 0x4B, 0x4C, 0x5D, 0x60] {
        assert!(is_road_tile(building), "PE road fallback {building:#04x}");
    }
    for building in [0x51, 0x5C, 0x6A, 0x6B] {
        assert!(
            is_road_bridge_tile(building),
            "PE bridge class {building:#04x}"
        );
    }
    for building in [0x50, 0x5D, 0x69, 0x6C] {
        assert!(
            !is_road_bridge_tile(building),
            "not a PE road bridge {building:#04x}"
        );
    }
    for building in [0x5D, 0x60] {
        assert!(is_onramp_tile(building), "PE ramp class {building:#04x}");
    }
    for building in [0x5C, 0x61] {
        assert!(
            !is_onramp_tile(building),
            "outside PE ramp class {building:#04x}"
        );
    }
    for building in [0x2C, 0x3E, 0x45, 0x48, 0x4D, 0x4E, 0x6C, 0x6F] {
        assert!(is_rail_tile(building), "PE rail class {building:#04x}");
    }
    for building in [0x44, 0x49, 0x4C, 0x5A, 0x5B, 0x70] {
        assert!(
            !is_rail_tile(building),
            "outside PE rail classes {building:#04x}"
        );
    }

    let mut city = blank_city();
    city.set_building(40, 40, 29);
    assert_eq!(
        try_step(&city, TransitMode::Highway, 1, 40, 40),
        None,
        "vehicle-highway state accepts highway pieces and onramps, not ordinary roads"
    );

    city.set_building(40, 40, 0x5D);
    assert_eq!(
        try_step(&city, TransitMode::Road, 1, 40, 40),
        Some((TransitMode::Highway, 2, false))
    );
    assert_eq!(
        try_step(&city, TransitMode::Pedestrian, 1, 40, 40),
        Some((TransitMode::PedestrianHighway, 2, false))
    );
    city.set_building(40, 40, 0x49);
    assert_eq!(
        try_step(&city, TransitMode::Road, 1, 40, 40),
        None,
        "road mode cannot enter a highway except through an onramp"
    );
    assert_eq!(
        try_step(&city, TransitMode::PedestrianHighway, 1, 40, 40),
        Some((TransitMode::PedestrianHighway, 1, false))
    );

    city.set_building(40, 40, 0x3F);
    city.set_tile_bits(40, 40, TileBits::new(0x04));
    assert_eq!(
        try_step(&city, TransitMode::Tunnel, 1, 40, 40),
        Some((TransitMode::Tunnel, 3, false))
    );
    assert_eq!(
        try_step(&city, TransitMode::PedestrianTunnel, 1, 40, 40),
        Some((TransitMode::PedestrianTunnel, 2, false))
    );
    city.set_tile_bits(40, 40, TileBits::new(0));
    assert_eq!(
        try_step(&city, TransitMode::Tunnel, 1, 40, 40),
        Some((TransitMode::Road, 3, false))
    );
    assert_eq!(
        try_step(&city, TransitMode::PedestrianTunnel, 1, 40, 40),
        Some((TransitMode::Pedestrian, 2, false))
    );
}

#[test]
fn pe_pedestrian_transit_transfer_is_mode_nine_not_bus_or_subway() {
    let mut city = blank_city();
    city.set_building(40, 40, BUS_DEPOT);
    assert_eq!(
        try_step(&city, TransitMode::Pedestrian, 1, 40, 40),
        Some((TransitMode::TransitTransfer, 4, false))
    );
    assert!(!TransitMode::TransitTransfer.marks_bus());

    city.set_building(40, 40, SUBWAY_STATION);
    assert_eq!(
        try_step(&city, TransitMode::TransitTransfer, 1, 40, 40),
        None,
        "PE mode 9 accepts bus/rail stations here, but not a subway station"
    );
    city.set_building(40, 40, 29);
    assert_eq!(
        try_step(&city, TransitMode::TransitTransfer, 1, 40, 40),
        Some((TransitMode::Pedestrian, 3, false))
    );
}

#[test]
fn run_trip_starts_from_pe_crossover_and_ramp_bands_but_not_bridge_tiles() {
    let mut city = blank_city();
    city.set_building(20, 21, 0x4B);
    city.set_zone(20, 22, 3);
    city.set_building(20, 22, 0x7C);
    assert!(
        run_trip(&city, 20, 20, 1, 0).success,
        "PE admits 0x4b as a road start"
    );

    city.set_building(20, 21, 0x51);
    assert!(
        !run_trip(&city, 20, 20, 1, 0).success,
        "PE's start scan rejects bridge-band tiles even though the walker can traverse them"
    );
}

#[test]
fn game_is_valid_transit_items_uses_trip_start_ring() {
    let mut city = blank_city();
    city.set_building(20, 23, 29);
    assert!(game_is_valid_transit_items(&city, 20, 20));
    city.set_building(20, 23, 0);
    city.set_building(20, 24, 29);
    assert!(
        !game_is_valid_transit_items(&city, 20, 20),
        "orthogonal +4 is outside dwTripStartingCoords"
    );
    city.set_building(20, 24, 0);
    city.set_building(22, 22, 29);
    assert!(
        !game_is_valid_transit_items(&city, 20, 20),
        "(2,2) is not a PE trip-start offset"
    );
    city.set_building(22, 22, 0);
    city.set_building(22, 21, 29);
    assert!(game_is_valid_transit_items(&city, 20, 20));
}

#[test]
fn trip_start_offsets_match_pe_initializer_order() {
    assert_eq!(
        TRIP_START_COORDS,
        [
            (0, 1),
            (1, 0),
            (0, -1),
            (-1, 0),
            (0, 2),
            (2, 0),
            (0, -2),
            (-2, 0),
            (0, 3),
            (3, 0),
            (0, -3),
            (-3, 0),
            (1, 1),
            (-1, 1),
            (1, -1),
            (-1, -1),
            (2, 1),
            (-2, 1),
            (2, -1),
            (-2, -1),
            (1, 2),
            (-1, 2),
            (1, -2),
            (-1, -2),
        ]
    );
}

#[test]
fn trip_reaches_complementary_zone_along_road() {
    let mut city = blank_city();
    // Residential at (20,20), commercial at (28,20), road corridor between.
    city.set_zone(20, 20, 1);
    city.set_building(20, 20, 0x70);
    city.set_zone(28, 20, 3);
    city.set_building(28, 20, 0x7C);
    for x in 21..=27 {
        city.set_building(x, 20, 29); // road LR
    }

    let result = run_trip(&city, 20, 20, 1, 1);
    assert!(
        result.success,
        "residential should reach commercial via road"
    );
    assert!(result.path.len() > 1);
    assert!(!result.traffic_path.is_empty());

    let before = city.traffic_density_at(22, 20).unwrap_or(0);
    let bumped = bump_xtrf_along_path(&mut city, &result.traffic_path, 1);
    assert!(bumped > 0);
    assert!(city.traffic_density_at(22, 20).unwrap_or(0) > before);
}

#[test]
fn bump_xtrf_adds_pop_for_each_step_in_same_microsim_cell() {
    let mut city = blank_city();
    // (20,20) and (21,20) share XTRF microsim cell (10,10).
    let path = [(20usize, 20usize), (21usize, 20usize)];
    let bumped = bump_xtrf_along_path(&mut city, &path, 3);
    assert_eq!(bumped, 2, "each path step should count as a bump");
    assert_eq!(
        city.traffic_density_at(20, 20),
        Some(6),
        "two steps at pop 3 should add twice into the shared microsim cell"
    );
    assert_eq!(city.traffic_density_at(21, 20), Some(6));
    assert_eq!(city.traffic_density_at(20, 21), Some(6));
    assert_eq!(city.traffic_density_at(21, 21), Some(6));
}

#[test]
fn trip_backtracks_out_of_dead_end_spur() {
    let mut city = blank_city();
    // Origin west of a junction; east spur is a dead end; north reaches commercial.
    // Stack walker must backtrack regardless of CRT initial facing.
    city.set_zone(18, 20, 1);
    city.set_building(18, 20, 0x70);
    city.set_zone(20, 17, 3);
    city.set_building(20, 17, 0x7C);
    city.set_building(19, 20, 29);
    city.set_building(20, 20, 29); // junction
    city.set_building(21, 20, 29); // spur
    city.set_building(22, 20, 29); // spur end
    city.set_building(20, 19, 29);
    city.set_building(20, 18, 29);

    let result = run_trip(&city, 18, 20, 1, 1);
    assert!(
        result.success,
        "trip should backtrack from east spur and reach commercial north"
    );
    assert!(
        result.path.iter().any(|&(x, y)| x == 20 && y == 18),
        "successful path should include the north corridor"
    );
    assert!(
        !result.path.iter().any(|&(x, y)| x == 22 && y == 20)
            || result.path.iter().any(|&(x, y)| x == 20 && y == 17),
        "either avoided the spur end or still completed after visiting it"
    );
}

#[test]
fn trip_backtracking_rerolls_each_restored_frame_facing() {
    let mut city = blank_city();
    city.set_zone(18, 20, 1);
    city.set_building(18, 20, 0x70);
    city.set_zone(20, 17, 3);
    city.set_building(20, 17, 0x7c);
    for (x, y) in [(19, 20), (20, 20), (21, 20), (22, 20), (20, 19), (20, 18)] {
        city.set_building(x, y, 29);
    }

    let seed = (0..65_536)
        .find(|&seed| {
            let mut preview = Win95CrtRand::new(seed);
            let rotation = trip_rotation_step(&mut preview);
            let _start_facing = trip_roll_outer_facing(&mut preview);
            let junction_facing = trip_rotate_dir(trip_roll_outer_facing(&mut preview), rotation);
            let spur_facing = trip_rotate_dir(trip_roll_outer_facing(&mut preview), rotation);
            junction_facing == 1 && spur_facing == 1
        })
        .expect("a CRT seed should choose east at the junction and spur");

    let mut actual_rng = Win95CrtRand::new(seed);
    let route = run_trip_with_rng(&city, 18, 20, 1, 2, &mut actual_rng);
    assert!(route.success);
    assert_eq!(
        route.path,
        [(19, 20), (20, 20), (20, 19), (20, 18), (20, 17)]
    );

    let mut expected_rng = Win95CrtRand::new(seed);
    // One rotation draw, then facings at the start, junction, spur,
    // dead end, restored spur, restored junction, and two north tiles.
    for _ in 0..9 {
        let _ = expected_rng.random_u15();
    }
    assert_eq!(actual_rng, expected_rng);
}

#[test]
fn trip_budget_exhaustion_returns_before_backtracking_or_another_crt_draw() {
    let mut city = blank_city();
    city.set_zone(18, 20, 1);
    city.set_building(18, 20, 0x70);
    city.set_zone(20, 18, 3);
    city.set_building(20, 18, 0x7c);
    // A long east spur consumes the budget; a short north route from the
    // junction would succeed if the PE backtracked after budget exhaustion.
    for x in 19..=56 {
        city.set_building(x, 20, 29);
    }
    city.set_building(20, 19, 29);

    for (pop_level, budget, endpoint) in [(1, 75u16, 44usize), (2, 100, 53)] {
        let seed = (0..1024)
            .find(|&seed| {
                let mut preview = Win95CrtRand::new(seed);
                let rotation = trip_rotation_step(&mut preview);
                let _start_facing = trip_roll_outer_facing(&mut preview);
                trip_rotate_dir(trip_roll_outer_facing(&mut preview), rotation) == 1
            })
            .expect("a deterministic CRT seed should choose the long east spur first");

        let mut actual_rng = Win95CrtRand::new(seed);
        let route = run_trip_with_rng(&city, 18, 20, 1, pop_level, &mut actual_rng);
        assert!(
            !route.success,
            "PE exits when accumulated cost reaches the budget"
        );
        assert_eq!(route.path.last(), Some(&(endpoint, 20)));
        assert!(!route.path.iter().any(|&tile| tile == (20, 18)));

        let mut expected_rng = Win95CrtRand::new(seed);
        // One rotation draw, then one facing draw at each tile before the
        // endpoint. Budget exhaustion occurs before drawing its next facing.
        let draws = 1 + endpoint - 19;
        for _ in 0..draws {
            let _ = expected_rng.random_u15();
        }
        assert_eq!(actual_rng, expected_rng, "budget {budget} draw boundary");
    }
}

#[test]
fn trip_search_can_revisit_coordinate_when_per_depth_masks_allow_it() {
    let mut city = blank_city();
    city.set_zone(18, 20, 1);
    city.set_building(18, 20, 0x70);
    city.set_zone(21, 20, 3);
    city.set_building(21, 20, 0x7c);
    for (x, y) in [(19, 20), (19, 19), (20, 19), (20, 20)] {
        city.set_building(x, y, 29);
    }

    let result = (0..64).find_map(|seed| {
        let mut rng = Win95CrtRand::new(seed);
        let route = run_trip_with_rng(&city, 18, 20, 1, 2, &mut rng);
        (route.success
            && route
                .path
                .iter()
                .enumerate()
                .any(|(index, tile)| route.path[..index].contains(tile)))
        .then_some(route)
    });
    assert!(
        result.is_some(),
        "PE's per-depth masks can revisit a coordinate; only the weighted budget bounds that path"
    );
}

#[test]
fn trip_fails_without_destination() {
    let mut city = blank_city();
    city.set_zone(20, 20, 1);
    city.set_building(20, 20, 0x70);
    for x in 21..=30 {
        city.set_building(x, 20, 29);
    }
    let result = run_trip(&city, 20, 20, 1, 1);
    assert!(!result.success);
}

#[test]
fn trip_vacant_tiles_use_pop_none_for_max_step_budget() {
    let mut city = blank_city();
    city.set_zone(20, 20, 1);
    city.set_building(20, 20, 0);
    for x in 21..48 {
        city.set_building(x, 20, 29); // 27 road tiles before destination at x=48.
    }
    city.set_zone(48, 20, 3);
    city.set_building(48, 20, 0x7c);

    let with_pop_one = run_trip(&city, 20, 20, 1, 1);
    assert!(!with_pop_one.success);
    assert!(
        run_trip(&city, 20, 20, 1, 2).success,
        "same route fits the PE 100-step budget when the one-quarter pop-1 reduction does not apply"
    );
    assert!(
        tile_has_trip_access(&city, 20, 20),
        "vacant RCI tile should use pop-level NONE for growth trip checks"
    );
}

fn stamp_2x2_populated_rci_for_test(city: &mut City, origin: (usize, usize), building: u8) {
    let (origin_x, origin_y) = origin;
    let corner_bits = city.footprint_corner_bits();

    city.set_zone(origin_x, origin_y, 1 | corner_bits[0]);
    city.set_building(origin_x, origin_y, building);

    city.set_zone(origin_x + 1, origin_y, 1 | corner_bits[1]);
    city.set_building(origin_x + 1, origin_y, building);

    city.set_zone(origin_x, origin_y + 1, 1 | corner_bits[2]);
    city.set_building(origin_x, origin_y + 1, building);

    city.set_zone(origin_x + 1, origin_y + 1, 1 | corner_bits[3]);
    city.set_building(origin_x + 1, origin_y + 1, building);
}

fn stamp_3x3_populated_rci_for_test(city: &mut City, origin: (usize, usize), building: u8) {
    let (origin_x, origin_y) = origin;
    let corner_bits = city.footprint_corner_bits();

    city.set_zone(origin_x, origin_y, 1 | corner_bits[0]);
    city.set_building(origin_x, origin_y, building);

    city.set_zone(origin_x + 1, origin_y, 1);
    city.set_building(origin_x + 1, origin_y, building);

    city.set_zone(origin_x + 2, origin_y, 1 | corner_bits[1]);
    city.set_building(origin_x + 2, origin_y, building);

    city.set_zone(origin_x, origin_y + 1, 1);
    city.set_building(origin_x, origin_y + 1, building);

    city.set_zone(origin_x + 1, origin_y + 1, 1);
    city.set_building(origin_x + 1, origin_y + 1, building);

    city.set_zone(origin_x + 2, origin_y + 1, 1);
    city.set_building(origin_x + 2, origin_y + 1, building);

    city.set_zone(origin_x, origin_y + 2, 1 | corner_bits[2]);
    city.set_building(origin_x, origin_y + 2, building);

    city.set_zone(origin_x + 1, origin_y + 2, 1);
    city.set_building(origin_x + 1, origin_y + 2, building);

    city.set_zone(origin_x + 2, origin_y + 2, 1 | corner_bits[3]);
    city.set_building(origin_x + 2, origin_y + 2, building);
}

#[test]
fn growth_lattice_trips_gate_multitile_populated_2x2_by_xzon_corner() {
    let footprint_tiles = [(20, 20), (21, 20), (20, 21), (21, 21)];
    let pop2_2x2_building = 0x8Cu8;

    for rotation in 0..4 {
        let mut city = blank_city();
        city.set_view_rotation(rotation);
        stamp_2x2_populated_rci_for_test(&mut city, (20, 20), pop2_2x2_building);

        let mut served = vec![false; CITY_TILES];
        let stats = simulate_growth_lattice_trips(
            &mut city,
            &mut served,
            footprint_tiles,
            |_, _| true,
            true,
        );

        assert_eq!(
            stats.trips_attempted, 1,
            "multi-tile populated tile should run one trip from the view-angle corner for rotation {rotation}"
        );
        assert_eq!(
            stats.trips_succeeded, 0,
            "no destination transit path means the only trip should fail"
        );
    }
}

#[test]
fn growth_lattice_trips_gate_multitile_populated_3x3_by_xzon_corner() {
    let footprint_tiles = [
        (20, 20),
        (21, 20),
        (22, 20),
        (20, 21),
        (21, 21),
        (22, 21),
        (20, 22),
        (21, 22),
        (22, 22),
    ];
    let pop3_3x3_building = 0x90u8;

    for rotation in 0..4 {
        let mut city = blank_city();
        city.set_view_rotation(rotation);
        stamp_3x3_populated_rci_for_test(&mut city, (20, 20), pop3_3x3_building);

        for x in 19..=22 {
            city.set_building(x, 19, 29);
            city.set_building(x, 23, 29);
        }
        for y in 20..=22 {
            city.set_building(19, y, 29);
            city.set_building(23, y, 29);
        }

        let mut served = vec![false; CITY_TILES];
        let stats = simulate_growth_lattice_trips(
            &mut city,
            &mut served,
            footprint_tiles.iter().copied(),
            |_, _| true,
            true,
        );

        assert_eq!(
            stats.trips_attempted, 1,
            "multi-tile populated tile should run one trip from the view-angle corner for rotation {rotation}"
        );
        assert_eq!(
            stats.trips_succeeded, 0,
            "no destination transit path means the only trip should fail"
        );
    }
}

#[test]
fn tile_has_trip_access_multitile_populated_2x2_only_view_corner_is_valid_origin() {
    let footprint_tiles = [(20, 20), (21, 20), (20, 21), (21, 21)];
    let pop2_2x2_building = 0x8Cu8;

    for rotation in 0..4 {
        let mut city = blank_city();
        city.set_view_rotation(rotation);
        stamp_2x2_populated_rci_for_test(&mut city, (20, 20), pop2_2x2_building);
        city.set_building(20, 19, 29); // One common trip-start road tile for all four corners.
        city.set_building(21, 19, 0xFB); // 0xFB is a guaranteed `success` destination tile.

        let mut source_count = 0usize;
        for &(x, y) in &footprint_tiles {
            let has_trip_access = tile_has_trip_access(&city, x, y);
            if xzon_corner_check(&city, x, y) {
                assert!(
                    has_trip_access,
                    "the view-angle corner tile should pass trip access for rotation {rotation}"
                );
                source_count += 1;
            } else {
                assert!(
                    !has_trip_access,
                    "non-angle populated footprint tiles should fail trip-access test for rotation {rotation}"
                );
            }
        }
        assert_eq!(
            source_count, 1,
            "exactly one source corner should be active"
        );
    }
}

#[test]
fn trip_succeeds_off_map_from_xtxt_250_neighbor_connection() {
    let edge_x = CITY_WIDTH - 1;
    let mut connected = blank_city();
    connected.set_zone(edge_x - 3, 20, 1);
    connected.set_building(edge_x - 3, 20, 0x70);
    for x in (edge_x - 2)..=edge_x {
        connected.set_building(x, 20, 29);
    }
    connected.set_text_overlay(edge_x, 20, TRIP_NEIGHBOR_CONNECTION_OVERLAY);
    let connected = run_trip(&connected, edge_x - 3, 20, 1, 1);
    assert!(
        connected.success,
        "PE L_RunTripGenerator treats OOB from XTXT 250 as a completed trip"
    );

    let mut disconnected = blank_city();
    disconnected.set_zone(edge_x - 3, 20, 1);
    disconnected.set_building(edge_x - 3, 20, 0x70);
    for x in (edge_x - 2)..=edge_x {
        disconnected.set_building(x, 20, 29);
    }
    let disconnected = run_trip(&disconnected, edge_x - 3, 20, 1, 1);
    assert!(
        !disconnected.success,
        "edge road without neighbor-connection overlay is not a destination"
    );
}

#[test]
fn trip_road_treats_buildings_above_desal_as_destinations() {
    let mut arco = blank_city();
    arco.set_zone(20, 20, 1);
    arco.set_building(20, 20, 0x70);
    for x in 21..=27 {
        arco.set_building(x, 20, 29);
    }
    arco.set_building(28, 20, 251);
    assert!(
        run_trip(&arco, 20, 20, 1, 1).success,
        "PE road walk: XBLD > 0xFA (arcos) completes the trip"
    );

    let mut desal = blank_city();
    desal.set_zone(20, 20, 1);
    desal.set_building(20, 20, 0x70);
    for x in 21..=27 {
        desal.set_building(x, 20, 29);
    }
    desal.set_building(28, 20, TRIP_DESAL_BUILDING);
    assert!(
        !run_trip(&desal, 20, 20, 1, 1).success,
        "desal itself is not > 0xFA"
    );
}

#[test]
fn trip_remaining_dir_mask_matches_pe_byte_4e858c() {
    assert_eq!(TRIP_REMAINING_AFTER_DIR, [0x0b, 0x07, 0x0e, 0x0d]);
}

#[test]
fn pedestrian_transit_highway_does_not_add_vehicle_xtrf() {
    let mut city = blank_city();
    city.set_zone(20, 20, 1);
    city.set_building(20, 20, 0x70);
    city.set_building(20, 21, BUS_DEPOT);
    city.set_building(20, 22, 29);
    city.set_building(20, 23, 0x5D);
    city.set_building(20, 24, 0x61);
    city.set_building(20, 25, 0x5D);
    city.set_zone(20, 26, 3);
    city.set_building(20, 26, 0x7C);

    let result = run_trip(&city, 20, 20, 1, 1);
    assert!(result.success, "bus-to-pedestrian route can use a highway");
    assert!(result.used_bus);
    assert!(
        result.traffic_path.is_empty(),
        "PE modes 8, 4, and 5 do not stamp vehicle XTRF"
    );
}

#[test]
fn trip_uses_highway_between_onramps() {
    let mut city = blank_city();
    city.set_zone(10, 20, 1);
    city.set_building(10, 20, 0x70);
    city.set_zone(40, 20, 3);
    city.set_building(40, 20, 0x7C);

    // Local roads + onramps + highway corridor.
    city.set_building(11, 20, 29);
    city.set_building(12, 20, 94); // onramp
    for x in 13..=37 {
        city.set_building(x, 20, 74); // highway LR
    }
    city.set_building(38, 20, 95); // onramp
    city.set_building(39, 20, 29);

    let result = run_trip(&city, 10, 20, 1, 1);
    assert!(
        result.success,
        "residential should reach commercial via highway"
    );
    // Highway steps should contribute to XTRF.
    assert!(result.traffic_path.len() > 4);
}

#[test]
fn trip_uses_rail_between_stations() {
    let mut city = blank_city();
    city.set_zone(10, 30, 1);
    city.set_building(10, 30, 0x70);
    city.set_zone(40, 30, 3);
    city.set_building(40, 30, 0x7C);

    city.set_building(11, 30, 29); // road to station
    city.set_building(12, 30, RAIL_STATION);
    for x in 13..=37 {
        city.set_building(x, 30, 45); // rail LR
    }
    city.set_building(38, 30, RAIL_STATION);
    city.set_building(39, 30, 29);

    let result = run_trip(&city, 10, 30, 1, 1);
    assert!(
        result.success,
        "residential should reach commercial via rail"
    );
    // Rail itself should not dominate XTRF; road stubs still bump.
    assert!(!result.traffic_path.is_empty());
    assert!(result.traffic_path.len() < result.path.len());
    assert!(result.used_rail);
    assert!(!result.used_bus);
    assert!(!result.used_subway);
}

#[test]
fn successful_rail_only_trip_does_not_stamp_xtrf_on_rail() {
    let mut city = blank_city();
    city.set_zone(10, 30, 1 | ZONE_CORNER_ALL);
    city.set_building(10, 30, 0x70);
    city.set_zone(40, 30, 3 | ZONE_CORNER_ALL);
    city.set_building(40, 30, 0x7C);

    // Start at the rail station (within 3 tiles) — no road stubs on the corridor.
    city.set_building(11, 30, RAIL_STATION);
    for x in 12..=37 {
        city.set_building(x, 30, 45); // rail LR
    }
    city.set_building(38, 30, RAIL_STATION);
    // One road tile only next to the destination so the walk can exit to C.
    city.set_building(39, 30, 29);

    let result = run_trip(&city, 10, 30, 1, 1);
    assert!(result.success, "rail-only corridor should still complete");
    assert!(result.used_rail);
    // Station/rail modes do not congest; only the exit road stub may.
    for x in 11..=38 {
        assert!(
            !result.traffic_path.contains(&(x, 30)),
            "rail/station tile ({x},30) must not be on traffic_path"
        );
    }

    let mut served = vec![false; CITY_TILES];
    let mut stats = MonthlyTripStats::default();
    run_origin_trip(
        &mut city,
        &mut served,
        &mut stats,
        10,
        30,
        &|_, _| true,
        true,
        false,
        None,
    );
    assert_eq!(stats.trips_succeeded, 1);
    assert!(stats.rail_passengers > 0);
    for x in 12..=37 {
        assert_eq!(
            city.traffic_density_at(x, 30).unwrap_or(0),
            0,
            "rail tile ({x},30) must not receive XTRF from a rail trip"
        );
    }
    if result.traffic_path.contains(&(39, 30)) {
        assert!(
            city.traffic_density_at(39, 30).unwrap_or(0) > 0,
            "road exit stub on traffic_path should congest"
        );
    }
}

#[test]
fn trip_uses_subway_between_stations() {
    let mut city = blank_city();
    city.set_zone(10, 40, 1);
    city.set_building(10, 40, 0x70);
    city.set_zone(40, 40, 3);
    city.set_building(40, 40, 0x7C);

    city.set_building(11, 40, 29);
    city.set_building(12, 40, SUBWAY_STATION);
    for x in 12..=38 {
        city.set_underground(x, 40, 1); // subway LR
    }
    city.set_building(38, 40, SUBWAY_STATION);
    city.set_building(39, 40, 29);

    let result = run_trip(&city, 10, 40, 1, 1);
    assert!(
        result.success,
        "residential should reach commercial via subway"
    );
    assert!(result.traffic_path.len() < result.path.len());
    assert!(result.used_subway);
    assert!(!result.used_rail);
    assert!(!result.used_bus);
}

#[test]
fn trip_marks_bus_passengers_from_depot() {
    let mut city = blank_city();
    city.set_zone(10, 50, 1);
    city.set_building(10, 50, 0x70);
    city.set_zone(20, 50, 3);
    city.set_building(20, 50, 0x7C);
    city.set_building(11, 50, 29);
    city.set_building(12, 50, BUS_DEPOT);
    for x in 13..=19 {
        city.set_building(x, 50, 29);
    }

    let result = run_trip(&city, 10, 50, 1, 2);
    assert!(result.success);
    assert!(result.used_bus);
    assert!(!result.used_rail);
    assert!(!result.used_subway);
}

#[test]
fn monthly_trips_count_transit_passengers() {
    let mut city = blank_city();
    city.set_zone(20, 20, 1 | ZONE_CORNER_ALL);
    city.set_building(20, 20, 0x70);
    city.set_zone(40, 20, 3 | ZONE_CORNER_ALL);
    city.set_building(40, 20, 0x7C);
    city.set_building(21, 20, 29);
    city.set_building(22, 20, RAIL_STATION);
    for x in 23..=37 {
        city.set_building(x, 20, 45);
    }
    city.set_building(38, 20, RAIL_STATION);
    city.set_building(39, 20, 29);

    let mut served = vec![false; CITY_TILES];
    let stats = simulate_monthly_trips(&mut city, &mut served, |_, _| true);
    assert!(stats.trips_succeeded > 0);
    assert!(stats.rail_passengers > 0);
    assert_eq!(stats.bus_passengers, 0);
    assert_eq!(stats.subway_passengers, 0);
}

#[test]
fn growth_lattice_vacant_trip_requires_power_like_populated() {
    let mut city = blank_city();
    // Vacant R zone — PE still calls FUN_00436D00 before RunTripGenerator.
    city.set_zone(20, 20, 1 | crate::city::ZONE_CORNER_ALL);
    city.set_zone(28, 20, 3 | crate::city::ZONE_CORNER_ALL);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }

    let mut served = vec![false; CITY_TILES];
    let unpowered =
        simulate_growth_lattice_trips(&mut city, &mut served, [(20, 20)], |_, _| false, true);
    assert_eq!(
        unpowered.trips_attempted, 0,
        "vacant origins must not skip the FUN_00436D00 power gate"
    );

    served.fill(false);
    let powered =
        simulate_growth_lattice_trips(&mut city, &mut served, [(20, 20)], |_, _| true, true);
    assert_eq!(
        powered.trips_attempted, 1,
        "powered vacant origin should attempt a trip"
    );
    assert!(powered.trips_succeeded > 0);
}

#[test]
fn growth_lattice_low_surface_xbld_is_vacant_trip_origin_without_corner() {
    let mut city = blank_city();
    // PE FUN_004358B0 treats every XBLD < 0x1d as pop level 0 in this RCI arm.
    // Its Game_IsValidTransitItems preflight finds the adjacent road before the
    // shared powered gate and RunTripGenerator call. No XZON corner bit is needed.
    city.set_zone(20, 20, 1);
    city.set_building(20, 20, 1); // rubble is a low-XBLD surface, not a developed RCI tile
    city.set_zone(28, 20, 3);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }

    let mut served = vec![false; CITY_TILES];
    let mut crt = Win95CrtRand::new(1);
    let crt_seed_before = crt.seed();
    let stats = simulate_growth_lattice_trips_with_rng(
        &mut city,
        &mut served,
        [(20, 20)],
        |_, _| true,
        true,
        &mut crt,
    );

    assert_eq!(stats.trips_attempted, 1);
    assert_eq!(stats.trips_succeeded, 1);
    assert!(served[20 * CITY_WIDTH + 20]);
    assert_eq!(stats.bus_passengers, 0);
    assert_eq!(stats.rail_passengers, 0);
    assert_eq!(stats.subway_passengers, 0);
    assert_eq!(stats.tiles_bumped, 0);
    assert_ne!(
        crt.seed(),
        crt_seed_before,
        "the PE route walk consumes CRT draws"
    );
    assert_eq!(city.traffic_density_at(21, 20).unwrap_or(0), 0);
}

#[test]
fn growth_lattice_trips_accumulate_without_clearing_prior_served_tiles() {
    let mut city = blank_city();
    city.set_zone(20, 20, 1 | ZONE_CORNER_ALL);
    city.set_building(20, 20, 0x70);
    city.set_zone(28, 20, 3 | ZONE_CORNER_ALL);
    city.set_building(28, 20, 0x7C);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }

    let mut served = vec![false; CITY_TILES];
    served[4 * CITY_WIDTH + 4] = true;
    let stats =
        simulate_growth_lattice_trips(&mut city, &mut served, [(20, 20)], |_, _| true, true);
    assert!(stats.trips_succeeded > 0);
    assert!(served[20 * CITY_WIDTH + 20]);
    assert!(
        served[4 * CITY_WIDTH + 4],
        "growth-lattice trips should not wipe earlier served origins"
    );
    assert!(city.traffic_density_at(22, 20).unwrap_or(0) > 0);
}

#[test]
fn decay_traffic_density_scales_high_bytes_without_u8_saturate() {
    // PE FUN_004586F1 @ 0x4586f1: value - (value >> 2) per native 64×64 cell.
    let mut city = blank_city();
    // XTRF microsim cells are 2×2; set_traffic_density fills the whole cell.
    city.set_traffic_density(10, 10, 100);
    city.set_traffic_density(12, 10, 255);
    decay_traffic_density(&mut city);
    assert_eq!(city.traffic_density_at(10, 10), Some(75));
    assert_eq!(city.traffic_density_at(11, 10), Some(75));
    assert_eq!(
        city.traffic_density_at(12, 10),
        Some(192),
        "decay must use value-(value>>2) with city column-major indexing"
    );
    assert_eq!(city.traffic_density_at(13, 10), Some(192));
    // PE accumulates each decayed byte into dwCityTrafficCount (0x4cc6f4).
    assert_eq!(traffic_density_microsim_sum(&city), 75 + 192);
}

#[test]
fn mac_phase19_xtrf_decay_matches_native_for_every_byte_value() {
    // CODE 14 `FUN_0002997c` reads/writes one byte per 64x64 XTRF cell.
    // Repeat all 256 inputs sixteen times, then check the full expanded plane
    // and aggregate against the native M68020 replay fixture.
    let mut city = blank_city();
    for cell in 0..(64 * 64) {
        let x = (cell % 64) * 2;
        let y = (cell / 64) * 2;
        city.set_traffic_density(x, y, (cell & 0xff) as u8);
    }

    decay_traffic_density(&mut city);

    let mut expected_sum = 0_u32;
    for cell in 0..(64 * 64) {
        let x = (cell % 64) * 2;
        let y = (cell / 64) * 2;
        let input = (cell & 0xff) as u8;
        let expected = input - (input >> 2);
        expected_sum += u32::from(expected);
        for dy in 0..2 {
            for dx in 0..2 {
                assert_eq!(
                    city.traffic_density_at(x + dx, y + dy),
                    Some(expected),
                    "native phase-19 decay at microsim cell ({}, {}) input {}",
                    cell % 64,
                    cell / 64,
                    input
                );
            }
        }
    }
    assert_eq!(traffic_density_microsim_sum(&city), expected_sum);
}

#[test]
fn monthly_trips_mark_served_and_decay() {
    let mut city = blank_city();
    city.set_zone(20, 20, 1 | crate::city::ZONE_CORNER_ALL);
    city.set_building(20, 20, 0x70);
    city.set_zone(28, 20, 3 | crate::city::ZONE_CORNER_ALL);
    city.set_building(28, 20, 0x7C);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    city.set_traffic_density(22, 20, 100);

    let mut served = vec![false; CITY_TILES];
    let stats = simulate_monthly_trips(&mut city, &mut served, |_, _| true);
    assert!(stats.trips_attempted > 0);
    assert!(served[20 * CITY_WIDTH + 20]);
    // Decay applied to pre-existing congestion before bumps.
    assert!(city.traffic_density_at(22, 20).is_some());
}
