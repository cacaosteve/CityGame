use super::*;

#[test]
fn mac_rci_candidate_caps_zero_extend_raw_unsigned_words() {
    let xbld_counts = [0; 256];
    let commercial_cap = mac_rci_demand_candidates(MacRciCandidateInputs {
        pair_totals: [0, 10, 10],
        previous_residential_total: 1_000,
        current_scalar: 200,
        prior_industry_modifier: -1_000,
        difficulty: 1,
        xbld_counts: &xbld_counts,
        cap_modifier_1: 65_534,
        cap_modifier_2: 0,
    });
    assert_eq!(
        commercial_cap.map(f32::to_bits),
        [0x41a0_0000, 0x431f_5d76, 0x4170_0000]
    );

    // CODE 8 applies the 15-point floor before the cap. A raw 0xfffe cap
    // modifier is zero-extended, so it leaves that floor intact; sign
    // extension would incorrectly make the result negative.
    let industrial_cap = mac_rci_demand_candidates(MacRciCandidateInputs {
        pair_totals: [0; 3],
        previous_residential_total: 0,
        current_scalar: 0,
        prior_industry_modifier: -1_000,
        difficulty: 1,
        xbld_counts: &xbld_counts,
        cap_modifier_1: 0,
        cap_modifier_2: 65_534,
    });
    assert_eq!(industrial_cap[2].to_bits(), 0x4170_0000);
}

#[test]
fn mac_rci_candidate_cap_count_sums_wrap_as_native_words() {
    let mut xbld_counts = [0; 256];
    for tile in [0xd5, 0xd7, 0xda, 0xf8, 0xdd, 0xde, 0xe0] {
        xbld_counts[tile] = u16::MAX;
    }
    let candidates = mac_rci_demand_candidates(MacRciCandidateInputs {
        pair_totals: [2_000_000; 3],
        previous_residential_total: i32::MAX,
        current_scalar: 60_000_000,
        prior_industry_modifier: i16::MAX,
        difficulty: 2,
        xbld_counts: &xbld_counts,
        cap_modifier_1: u16::MAX,
        cap_modifier_2: u16::MAX,
    });

    // Original CODE 8 wraps the WORD cap sums before multiplying. These
    // singles match the native SANE trace for the all-0xffff fixture.
    assert_eq!(
        candidates.map(f32::to_bits),
        [0x4a48_0fa0, 0x4b95_ff6a, 0x4cbb_7f44]
    );
}

#[test]
fn mac_saved_flint_phase21_candidate_singles_match_68020() {
    let mut xbld_counts = [0; 256];
    xbld_counts[0xd5] = 36;
    xbld_counts[0xd7] = 16;

    let candidates = mac_rci_demand_candidates(MacRciCandidateInputs {
        pair_totals: [302, 172, 175],
        previous_residential_total: 1_508,
        current_scalar: 6_490,
        prior_industry_modifier: 5,
        difficulty: 2,
        xbld_counts: &xbld_counts,
        cap_modifier_1: 1,
        cap_modifier_2: 13,
    });

    // Saved Flint CODE 8 export_237 candidate singles, before normalization
    // and tax/previous-demand finalization.
    assert_eq!(
        candidates.map(f32::to_bits),
        [0x43b0_8000, 0x438e_cb4f, 0x445a_0556]
    );
}

#[test]
fn mac_rci_finalizer_truncates_signed_zero_crossings_like_native_68020() {
    let xbld_counts = [0; 256];
    let candidates = mac_rci_demand_candidates(MacRciCandidateInputs {
        pair_totals: [9, 48, 20],
        previous_residential_total: 1_508,
        current_scalar: 770,
        prior_industry_modifier: 5,
        difficulty: 2,
        xbld_counts: &xbld_counts,
        cap_modifier_1: 0,
        cap_modifier_2: 0,
    });
    assert_eq!(
        candidates.map(f32::to_bits),
        [0x4288_0000, 0x4313_f1bc, 0x43fb_5555]
    );

    // Native CODE 8's pre-FINTRZ values are respectively small positive,
    // positive, and small negative fractions. FINTRZ truncates all three
    // toward zero before the signed-WORD clamp.
    let finalized = std::array::from_fn(|index| {
        mac_rci_demand_from_candidate(
            candidates[index],
            [9, 48, 20][index],
            200,
            [-3_680, -1_411, -13_962][index],
        )
    });
    assert_eq!(finalized, [0, 0, 0]);
}

#[test]
fn mac_rci_finalizer_accepts_signed_word_endpoints_before_demand_clamp() {
    let xbld_counts = [0; 256];
    let pair_totals = [0, 0, 0];
    let candidates = mac_rci_demand_candidates(MacRciCandidateInputs {
        pair_totals,
        previous_residential_total: 0,
        current_scalar: 0,
        prior_industry_modifier: -1_000,
        difficulty: 1,
        xbld_counts: &xbld_counts,
        cap_modifier_1: 0,
        cap_modifier_2: 0,
    });
    assert_eq!(candidates.map(f32::to_bits), [0, 0, 0x4170_0000]);

    let previous = [-32_368, -32_368, 24_167];
    let pre_clamp_words = std::array::from_fn(|index| {
        mac_rci_demand_pre_clamp_word(
            mac_rci_normalize_demand_candidate(candidates[index], pair_totals[index] as u32),
            200,
            previous[index],
        )
    });
    assert_eq!(
        pre_clamp_words,
        [i16::MIN as i32, i16::MIN as i32, i16::MAX as i32]
    );

    let finalized = std::array::from_fn(|index| {
        mac_rci_demand_from_candidate(
            candidates[index],
            pair_totals[index] as u32,
            200,
            previous[index],
        )
    });
    assert_eq!(finalized, [-2_000, -2_000, 2_000]);
}

#[test]
fn check_tile_area_xbld_matches_pe_pop1_max_and_transit() {
    assert!(check_tile_area_xbld_allowed(
        0,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(check_tile_area_xbld_allowed(
        14,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(check_tile_area_xbld_allowed(
        0x8B,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x8C,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        221,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x1D,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x2B,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x2C,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x3E,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x3F,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x48,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(check_tile_area_xbld_allowed(
        0x49,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(check_tile_area_xbld_allowed(
        0x4A,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x4B,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x4E,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(check_tile_area_xbld_allowed(
        0x4F,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x5D,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x60,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(check_tile_area_xbld_allowed(
        0x61,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x6C,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0x6F,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
    assert!(check_tile_area_xbld_allowed(
        0x70,
        CHECK_TILE_AREA_POP1_MAX_XBLD
    ));
}

#[test]
fn check_tile_area_xbld_matches_pe_pop3_max() {
    assert!(check_tile_area_xbld_allowed(
        0x90,
        CHECK_TILE_AREA_POP3_MAX_XBLD
    ));
    assert!(check_tile_area_xbld_allowed(
        0xAD,
        CHECK_TILE_AREA_POP3_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        0xAE,
        CHECK_TILE_AREA_POP3_MAX_XBLD
    ));
    assert!(!check_tile_area_xbld_allowed(
        221,
        CHECK_TILE_AREA_POP3_MAX_XBLD
    ));
    assert_eq!(check_tile_area_max_xbld_for_start_pop(1), Some(0x8C));
    assert_eq!(check_tile_area_max_xbld_for_start_pop(2), None);
    assert_eq!(check_tile_area_max_xbld_for_start_pop(3), Some(0xAE));
}

#[test]
fn check_tile_area_matches_pe_altitude_zone_and_xbld_masks() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, ZONE_RESIDENTIAL_DENSE | 0xF0);
    city.set_altitude(10, 10, 5, 0);
    assert!(check_tile_area(
        &city,
        10,
        10,
        0x25,
        ZONE_RESIDENTIAL_DENSE,
        0xAE
    ));
    assert!(
        check_tile_area(&city, 10, 10, 0x05, ZONE_RESIDENTIAL_DENSE, 0xAE),
        "PE compares ALTM low five bits"
    );
    assert!(
        check_tile_area(&city, 10, 10, 5, ZONE_RESIDENTIAL_DENSE, 0xAE),
        "PE compares XZON low nibble"
    );

    assert!(!check_tile_area(
        &city,
        10,
        10,
        6,
        ZONE_RESIDENTIAL_DENSE,
        0xAE
    ));
    assert!(!check_tile_area(
        &city,
        10,
        10,
        5,
        ZONE_COMMERCIAL_DENSE,
        0xAE
    ));
    city.set_building(10, 10, 0xAD);
    assert!(check_tile_area(
        &city,
        10,
        10,
        5,
        ZONE_RESIDENTIAL_DENSE,
        0xAE
    ));
    city.set_building(10, 10, 0xAE);
    assert!(!check_tile_area(
        &city,
        10,
        10,
        5,
        ZONE_RESIDENTIAL_DENSE,
        0xAE
    ));
    assert!(!check_tile_area(
        &city,
        CITY_WIDTH,
        10,
        5,
        ZONE_RESIDENTIAL_DENSE,
        0xAE
    ));
}

#[test]
fn pop1_startbuilding_quadrant_origins_match_pe_order() {
    assert_eq!(
        pop1_startbuilding_quadrant_origins(21, 21).collect::<Vec<_>>(),
        vec![(21, 21), (21, 20), (20, 21), (20, 20)]
    );
    assert_eq!(
        pop1_startbuilding_quadrant_origins(0, 0).collect::<Vec<_>>(),
        vec![(0, 0)]
    );
    assert_eq!(
        pop1_startbuilding_quadrant_origins(0, 5).collect::<Vec<_>>(),
        vec![(0, 5), (0, 4)]
    );
    assert_eq!(
        pop1_startbuilding_quadrant_origins(5, 0).collect::<Vec<_>>(),
        vec![(5, 0), (4, 0)]
    );
}

#[test]
fn pop1_startbuilding_candidate_origins_invert_to_pe_pop2_calls() {
    let calls = pop1_startbuilding_quadrant_origins(21, 21)
        .map(|(x, y)| pop1_startbuilding_candidate_call_from_origin(x, y).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(calls, vec![(21, 22), (21, 21), (20, 22), (20, 21)]);

    assert_eq!(
        pop1_startbuilding_candidate_call_from_origin(20, usize::MAX),
        None,
        "the inverse coordinate transform must reject overflow"
    );
}

#[test]
fn pop2_choose_and_place_stamp_origin_matches_direct_pe_call() {
    assert_eq!(pop2_choose_and_place_stamp_origin(21, 21), Some((21, 20)));
    assert_eq!(pop2_choose_and_place_stamp_origin(21, 1), Some((21, 0)));
    assert_eq!(pop2_choose_and_place_stamp_origin(21, 0), None);
}

#[test]
fn pop3_startbuilding_quadrant_origins_match_pe_raw_call_order() {
    assert_eq!(
        pop3_startbuilding_quadrant_origins(21, 21).collect::<Vec<_>>(),
        vec![(21, 19), (20, 19), (21, 20), (20, 20)]
    );
    assert_eq!(
        pop3_startbuilding_quadrant_origins(0, 0).collect::<Vec<_>>(),
        Vec::<(usize, usize)>::new()
    );
    assert_eq!(
        pop3_startbuilding_quadrant_origins(0, 5).collect::<Vec<_>>(),
        vec![(0, 3), (0, 4)]
    );
    assert_eq!(
        pop3_startbuilding_quadrant_origins(5, 0).collect::<Vec<_>>(),
        Vec::<(usize, usize)>::new()
    );
    assert_eq!(
        pop3_startbuilding_quadrant_origins(5, 1).collect::<Vec<_>>(),
        vec![(5, 0), (4, 0)]
    );
}

#[test]
fn pop3_startbuilding_quadrants_expand_from_raw_call_tile() {
    assert_eq!(
        pop3_startbuilding_quadrant_origins(21, 19).collect::<Vec<_>>(),
        vec![(21, 17), (20, 17), (21, 18), (20, 18)]
    );
}

#[test]
fn pop3_startbuilding_candidate_origins_invert_to_pe_pop4_calls() {
    let calls = pop3_startbuilding_quadrant_origins(20, 20)
        .map(|(x, y)| pop3_startbuilding_candidate_call_from_origin(x, y).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(calls, vec![(20, 20), (19, 20), (20, 21), (19, 21)]);

    assert_eq!(
        pop3_startbuilding_candidate_call_from_origin(20, usize::MAX),
        None,
        "the inverse coordinate transform must reject overflow"
    );
}

#[test]
fn pe_startbuilding_pop4_picks_before_raw_call_bounds() {
    for (call_x, call_y, expected_origin) in [
        (2, 0, None),
        (1, 2, None),
        (2, 1, None),
        (125, 124, None),
        (124, 125, None),
        (2, 2, Some((2, 0))),
        (124, 124, Some((124, 122))),
        (2, 124, Some((2, 122))),
        (124, 2, Some((124, 0))),
    ] {
        let mut actual_crt = Win95CrtRand::new(0x0043_75C0);
        let mut expected_crt = actual_crt;
        let expected_building = expected_crt.random_mod(2) as u8;
        let actual = pe_startbuilding_pop4_direct_placement_with_picker(call_x, call_y, || {
            Some(actual_crt.random_mod(2) as u8)
        });

        assert_eq!(actual_crt.seed(), expected_crt.seed());
        match expected_origin {
            Some(origin) => assert_eq!(actual, Some((origin.0, origin.1, expected_building))),
            None => assert_eq!(actual, None, "invalid raw call ({call_x},{call_y})"),
        }
    }
}

#[test]
fn pe_startbuilding_pop3_picks_before_raw_call_bounds() {
    for (call_x, call_y, expected_origin) in [
        (0, 0, None),
        (1, 2, None),
        (2, 1, None),
        (126, 125, None),
        (125, 126, None),
        (2, 2, Some((2, 1))),
        (125, 125, Some((125, 124))),
        (2, 125, Some((2, 124))),
        (125, 2, Some((125, 1))),
    ] {
        let mut actual_crt = Win95CrtRand::new(0x0043_75C0);
        let mut expected_crt = actual_crt;
        let expected_building = expected_crt.random_mod(2) as u8;
        let actual = pe_startbuilding_pop3_direct_placement_with_picker(call_x, call_y, || {
            Some(actual_crt.random_mod(2) as u8)
        });

        assert_eq!(actual_crt.seed(), expected_crt.seed());
        match expected_origin {
            Some(origin) => assert_eq!(actual, Some((origin.0, origin.1, expected_building))),
            None => assert_eq!(actual, None, "invalid raw call ({call_x},{call_y})"),
        }
    }
}

#[test]
fn pe_startbuilding_pop2_quadrant_uses_mode3_picker_before_raw_call_bounds() {
    for (call_x, call_y, expected_origin) in [
        (0, 0, None),
        (1, 2, None),
        (2, 1, None),
        (126, 125, None),
        (125, 126, None),
        (2, 2, Some((2, 1))),
        (125, 125, Some((125, 124))),
        (2, 125, Some((2, 124))),
        (125, 2, Some((125, 1))),
    ] {
        let mut actual_crt = Win95CrtRand::new(0x0043_75C0);
        let mut expected_crt = actual_crt;
        let expected_building =
            construction_building_for_pop_with_rng(2, &mut expected_crt).unwrap();
        let actual = pe_startbuilding_pop2_quadrant_placement_with_picker(call_x, call_y, || {
            construction_building_for_pop_with_rng(2, &mut actual_crt)
        });

        assert!(CONSTRUCTION_POP2_BUILDINGS.contains(&expected_building));
        assert_eq!(actual_crt.seed(), expected_crt.seed());
        match expected_origin {
            Some(origin) => assert_eq!(actual, Some((origin.0, origin.1, expected_building))),
            None => assert_eq!(actual, None, "invalid raw call ({call_x},{call_y})"),
        }
    }
}

#[test]
fn choose_and_place_multitile_call_bounds_match_pe() {
    assert!(choose_and_place_multitile_call_in_bounds(0, 0, 1));
    assert!(choose_and_place_multitile_call_in_bounds(127, 127, 1));
    assert!(choose_and_place_multitile_call_in_bounds(2, 2, 2));
    assert!(!choose_and_place_multitile_call_in_bounds(1, 2, 2));
    assert!(!choose_and_place_multitile_call_in_bounds(2, 1, 2));
    assert!(choose_and_place_multitile_call_in_bounds(125, 125, 2));
    assert!(!choose_and_place_multitile_call_in_bounds(126, 125, 2));
    assert!(!choose_and_place_multitile_call_in_bounds(125, 126, 2));
    assert!(choose_and_place_multitile_call_in_bounds(2, 2, 3));
    assert!(choose_and_place_multitile_call_in_bounds(124, 124, 4));
    assert!(!choose_and_place_multitile_call_in_bounds(125, 124, 4));
    assert!(choose_and_place_nw_origin_in_bounds(2, 1, 2));
    assert!(!choose_and_place_nw_origin_in_bounds(2, 0, 2));
    assert!(!choose_and_place_nw_origin_in_bounds(1, 20, 2));
    assert!(choose_and_place_nw_origin_in_bounds(2, 0, 4));
}

#[test]
fn xzon_view_corner_matches_pe_position_angle_for_all_rotations() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE | 0x10); // NW
    city.set_zone(21, 20, ZONE_RESIDENTIAL_DENSE | 0x20); // NE
    city.set_zone(20, 21, ZONE_RESIDENTIAL_DENSE | 0x80); // SW
    city.set_zone(21, 21, ZONE_RESIDENTIAL_DENSE | 0x40); // SE

    for (rotation, expected_corner) in [
        (0, (20, 21)), // SW
        (1, (20, 20)), // NW
        (2, (21, 20)), // NE
        (3, (21, 21)), // SE
    ] {
        city.set_view_rotation(rotation);
        let actual_corner = [(20, 20), (21, 20), (20, 21), (21, 21)]
            .into_iter()
            .find(|&(x, y)| xzon_corner_check(&city, x, y));
        assert_eq!(actual_corner, Some(expected_corner), "rotation {rotation}");
    }
}

#[test]
fn choose_and_place_consumes_picker_draw_before_bounds_rejection() {
    let mut crt = Win95CrtRand::new(0x0043_6166);
    let seed_before = crt.seed();
    let result = pe_choose_and_place_then_origin_bounds(|| Some(crt.random_u16()), 0, 0, 2);
    assert!(
        result.is_none(),
        "the edge origin is outside ChooseAndPlace bounds"
    );
    assert_ne!(
        crt.seed(),
        seed_before,
        "PE consumes ChooseAndPlace's picker draw before it rejects the edge"
    );
}

#[test]
fn pe_startbuilding_pre_roll_gate_matches_pop_and_zone_thresholds() {
    let mut city = City::new_blank("/tmp/pe-startbuilding-gates.sc2", "test").expect("blank city");
    let xval_gated_zones = [
        ZONE_RESIDENTIAL_LIGHT,
        ZONE_RESIDENTIAL_DENSE,
        ZONE_COMMERCIAL_LIGHT,
        ZONE_COMMERCIAL_DENSE,
    ];
    let pop_thresholds = [
        (1, XVAL_POPLEVEL_LOW),
        (2, XVAL_POPLEVEL_MEDIUM),
        (3, XVAL_POPLEVEL_HIGH),
    ];
    for zone_id in xval_gated_zones {
        for (current_pop, threshold) in pop_thresholds {
            city.sync_land_value_from_slice(&vec![threshold - 1; CITY_TILES]);
            assert_eq!(
                pe_startbuilding_pre_roll_target_pop(&city, 20, 20, zone_id, current_pop),
                None,
                "zone {zone_id} pop {current_pop} rejects XVAL below {threshold:#04x}"
            );
            city.sync_land_value_from_slice(&vec![threshold; CITY_TILES]);
            assert_eq!(
                pe_startbuilding_pre_roll_target_pop(&city, 20, 20, zone_id, current_pop),
                Some(current_pop + 1),
                "zone {zone_id} pop {current_pop} admits XVAL at {threshold:#04x}"
            );
        }
    }

    city.sync_land_value_from_slice(&vec![0; CITY_TILES]);
    for zone_id in [ZONE_INDUSTRIAL_LIGHT, ZONE_INDUSTRIAL_DENSE] {
        assert_eq!(
            pe_startbuilding_pre_roll_target_pop(&city, 20, 20, zone_id, 1),
            Some(2),
            "industrial zone {zone_id} bypasses the XVAL pop gate"
        );
    }
    assert_eq!(
        pe_startbuilding_pre_roll_target_pop(&city, 20, 20, ZONE_RESIDENTIAL_DENSE, 4,),
        None,
        "pop 4 is terminal before the growth roll"
    );
}

#[test]
fn pe_startbuilding_xval_samples_the_rotation_corner_microsim_cell() {
    let mut city =
        City::new_blank("/tmp/pe-startbuilding-xval-corner.sc2", "test").expect("blank city");
    // The pop-2 direct-growth path receives one of these four raw scan tiles,
    // depending on the view-angle corner. All four belong to the same native cell.
    let scan_tiles = [(20, 21), (20, 20), (21, 20), (21, 21)];
    let mut xval = vec![u8::MAX; CITY_TILES];
    xval[20 * CITY_HEIGHT + 20] = XVAL_POPLEVEL_MEDIUM;
    city.sync_land_value_from_slice(&xval);

    for (x, y) in scan_tiles {
        assert_eq!(
            pe_density_microsim_origin(x, y),
            (20, 20),
            "scan tile ({x},{y}) must fold to its native XVAL cell"
        );
        assert_eq!(
            pe_xval_byte_at_city_coords(&city, x, y),
            Some(XVAL_POPLEVEL_MEDIUM)
        );
        assert!(land_value_allows_start_from_pop(
            &city,
            x,
            y,
            ZONE_RESIDENTIAL_DENSE,
            2,
        ));
    }

    // The three other bytes in the expanded 2×2 are deliberately high. PE
    // samples the NW representative, so lowering that byte rejects each corner.
    xval[20 * CITY_HEIGHT + 20] = XVAL_POPLEVEL_MEDIUM - 1;
    city.sync_land_value_from_slice(&xval);
    for (x, y) in scan_tiles {
        assert!(!land_value_allows_start_from_pop(
            &city,
            x,
            y,
            ZONE_RESIDENTIAL_DENSE,
            2,
        ));
    }
}

#[test]
fn pe_choose_and_place_can_complete_large_sites_in_light_zones() {
    let cases = [
        (ZONE_RESIDENTIAL_LIGHT, 0xae, 4),
        (ZONE_COMMERCIAL_LIGHT, 0xb2, 10),
        (ZONE_INDUSTRIAL_LIGHT, 0xbc, 6),
    ];
    for (zone, base, count) in cases {
        let mut actual_crt = Win95CrtRand::new(2_317_907_988);
        let mut expected_crt = actual_crt;
        let expected = base + expected_crt.random_mod(count) as u8;
        assert_eq!(
            rci_choose_and_place_building_with_rng(zone, 4, None, &mut actual_crt),
            Some(expected),
        );
        assert_eq!(actual_crt.seed(), expected_crt.seed());
        assert!(rci_development_buildings_for_zone_and_size(zone, 3).is_none());
    }
}

#[test]
fn pe_choose_and_place_pop3_variant_table_does_not_depend_on_xval() {
    let mut low_xval_crt = Win95CrtRand::new(0x0043_6EA0);
    let mut high_xval_crt = Win95CrtRand::new(0x0043_6EA0);
    let low_xval = rci_choose_and_place_building_with_rng(
        ZONE_RESIDENTIAL_DENSE,
        3,
        Some(0),
        &mut low_xval_crt,
    );
    let high_xval = rci_choose_and_place_building_with_rng(
        ZONE_RESIDENTIAL_DENSE,
        3,
        Some(u8::MAX),
        &mut high_xval_crt,
    );

    assert_eq!(low_xval, high_xval);
    assert_eq!(low_xval_crt.seed(), high_xval_crt.seed());
}

#[test]
fn pe_choose_and_place_pop1_clamps_xval_band_and_masks_one_rand() {
    let cases = [
        (None, 0),
        (Some(0), 0),
        (Some(63), 0),
        (Some(64), 1),
        (Some(127), 1),
        (Some(128), 2),
        (Some(191), 2),
        (Some(192), 2),
        (Some(u8::MAX), 2),
    ];

    for (xval, band) in cases {
        let mut actual_crt = Win95CrtRand::new(0x0043_6EA0);
        let mut expected_crt = actual_crt;
        let variant = usize::from(expected_crt.random_u15() & 3);
        let expected = RESIDENTIAL_1X1_BUILDINGS[band * 4 + variant];
        assert_eq!(
            rci_choose_and_place_building_with_rng(
                ZONE_RESIDENTIAL_DENSE,
                1,
                xval,
                &mut actual_crt,
            ),
            Some(expected),
            "XVAL {xval:?} should choose band {band} and CRT variant {variant}"
        );
        assert_eq!(
            actual_crt.seed(),
            expected_crt.seed(),
            "the PE pop-1 branch consumes exactly one rand()"
        );
    }
}

#[test]
fn pe_choose_and_place_mode3_pop1_uses_construction_base_and_count() {
    for (seed, expected_variant) in [(0x0043_6E80, 0), (0x0043_6E81, 1)] {
        let mut actual_crt = Win95CrtRand::new(seed);
        let mut expected_crt = actual_crt;
        assert_eq!(expected_crt.random_u15() % 2, expected_variant);

        assert_eq!(
            construction_building_for_pop_with_rng(1, &mut actual_crt),
            Some(0x88 + expected_variant as u8),
            "mode 3 pop 1 uses base 0x88 plus rand() % 2"
        );
        assert_eq!(actual_crt.seed(), expected_crt.seed());
    }
}

#[test]
fn pe_choose_and_place_mode4_pop1_uses_abandoned_base_and_count() {
    for (seed, expected_variant) in [(0x0043_6E80, 0), (0x0043_6E81, 1)] {
        let mut actual_crt = Win95CrtRand::new(seed);
        let mut expected_crt = actual_crt;
        assert_eq!(expected_crt.random_u15() % 2, expected_variant);

        assert_eq!(
            abandoned_building_for_pop_with_rng(1, &mut actual_crt),
            Some(0x8A + expected_variant as u8),
            "mode 4 pop 1 uses base 0x8A plus rand() % 2"
        );
        assert_eq!(actual_crt.seed(), expected_crt.seed());
    }
}

#[test]
fn pe_changebuilding_pop1_direct_accepts_city_edges_and_uses_raw_call() {
    for ((call_x, call_y), replace_tile) in [((0, 0), false), ((127, 127), true)] {
        let footprint = [(call_x, call_y)];
        let placements = rci_decline_change_building_at_call_optional(
            0x70,
            &footprint,
            call_x,
            call_y,
            40,
            41,
            replace_tile,
            None,
        )
        .expect("mode-4 pop-1 direct abandon");
        assert_eq!(placements.len(), 1);
        assert_eq!((placements[0].0, placements[0].1), (call_x, call_y));
        assert!((0x8A..=0x8B).contains(&placements[0].2));
        assert!(choose_and_place_nw_origin_in_bounds(call_x, call_y, 1));
    }
}

#[test]
fn pe_changebuilding_pop1_picker_precedes_city_edge_rejection() {
    for (call_x, call_y, in_city) in [(0, 0, true), (127, 127, true), (128, 0, false)] {
        let mut actual_crt = Win95CrtRand::new(0x0043_6166);
        let mut expected_crt = actual_crt;
        expected_crt.random_u15();

        let placements =
            pe_changebuilding_pop1_direct_placement_with_picker(call_x, call_y, || {
                abandoned_building_for_pop_with_rng(1, &mut actual_crt)
            })
            .expect("mode-4 pop-1 picker succeeds before ordinary tile bounds");
        assert_eq!(placements.len(), if in_city { 1 } else { 0 });
        if in_city {
            assert_eq!((placements[0].0, placements[0].1), (call_x, call_y));
            assert!((0x8A..=0x8B).contains(&placements[0].2));
        }
        assert_eq!(
            actual_crt.seed(),
            expected_crt.seed(),
            "one pop-1 picker draw occurs before tile ({call_x},{call_y}) is handled"
        );
    }
}

#[test]
fn pe_changebuilding_mode4_pop2_uses_abandoned_base_and_count() {
    for (seed, expected_variant) in [(0x0043_6E80, 0), (0x0043_6E81, 1)] {
        let mut actual_crt = Win95CrtRand::new(seed);
        let mut expected_crt = actual_crt;
        assert_eq!(expected_crt.random_u15() % 2, expected_variant);

        assert_eq!(
            abandoned_building_for_pop_with_rng(2, &mut actual_crt),
            Some(0xAA + expected_variant as u8),
            "mode 4 pop 2 uses base 0xAA plus rand() % 2"
        );
        assert_eq!(actual_crt.seed(), expected_crt.seed());
    }
}

#[test]
fn pe_changebuilding_mode4_pop3_uses_abandoned_base_and_count() {
    for (seed, expected_variant) in [(0x0043_6E80, 0), (0x0043_6E81, 1)] {
        let mut actual_crt = Win95CrtRand::new(seed);
        let mut expected_crt = actual_crt;
        assert_eq!(expected_crt.random_u15() % 2, expected_variant);

        assert_eq!(
            abandoned_building_for_pop_with_rng(3, &mut actual_crt),
            Some(0xAC + expected_variant as u8),
            "mode 4 pop 3 uses base 0xAC plus rand() % 2"
        );
        assert_eq!(actual_crt.seed(), expected_crt.seed());
    }
}

#[test]
fn pe_changebuilding_mode4_pop4_uses_abandoned_base_and_count() {
    for (seed, expected_variant) in [(0x0043_6E80, 0), (0x0043_6E81, 1)] {
        let mut actual_crt = Win95CrtRand::new(seed);
        let mut expected_crt = actual_crt;
        assert_eq!(expected_crt.random_u15() % 2, expected_variant);

        assert_eq!(
            abandoned_building_for_pop_with_rng(4, &mut actual_crt),
            Some(0xC4 + expected_variant as u8),
            "mode 4 pop 4 uses base 0xC4 plus rand() % 2"
        );
        assert_eq!(actual_crt.seed(), expected_crt.seed());
    }
}

#[test]
fn pe_changebuilding_pop2_uses_raw_call_tile_and_quadrant_order() {
    let footprint = [(20, 20), (21, 20), (20, 21), (21, 21)];
    let cases = [
        ((20, 21), vec![(20, 21), (21, 21), (21, 20), (20, 20)]), // rotation 0 / SW
        ((20, 20), vec![(20, 20), (21, 20), (21, 19), (20, 19)]), // rotation 1 / NW
        ((21, 20), vec![(21, 20), (22, 20), (22, 19), (21, 19)]), // rotation 2 / NE
        ((21, 21), vec![(21, 21), (22, 21), (22, 20), (21, 20)]), // rotation 3 / SE
    ];

    for ((call_x, call_y), expected_quadrants) in cases {
        let split = rci_decline_change_building_at_call_optional(
            0x8C, &footprint, call_x, call_y, 20, 20, true, None,
        )
        .expect("pop-2 abandoned split");
        assert_eq!(
            split.iter().map(|&(x, y, _)| (x, y)).collect::<Vec<_>>(),
            expected_quadrants,
            "replace=true preserves PE quadrant call order from raw tile ({call_x},{call_y})"
        );
        assert!(
            split
                .iter()
                .all(|&(_, _, building)| (0x8A..=0x8B).contains(&building))
        );

        let direct = rci_decline_change_building_at_call_optional(
            0x8C, &footprint, call_x, call_y, 20, 20, false, None,
        )
        .expect("direct pop-2 abandon");
        assert_eq!(
            direct.iter().map(|&(x, y, _)| (x, y)).collect::<Vec<_>>(),
            vec![(call_x, call_y - 1)],
            "replace=false stamps from raw ChooseAndPlace call (x,y-1)"
        );
        assert!(
            direct
                .iter()
                .all(|&(_, _, building)| (0xAA..=0xAB).contains(&building))
        );
    }
}

#[test]
fn pe_changebuilding_pop2_replace_picks_all_edge_calls_before_filtering() {
    let cases = [
        (
            (0, 0),
            [(0, 0), (1, 0), (1, -1), (0, -1)],
            vec![(0, 0), (1, 0)],
        ),
        (
            (0, 127),
            [(0, 127), (1, 127), (1, 126), (0, 126)],
            vec![(0, 127), (1, 127), (1, 126), (0, 126)],
        ),
        (
            (127, 0),
            [(127, 0), (128, 0), (128, -1), (127, -1)],
            vec![(127, 0)],
        ),
        (
            (127, 127),
            [(127, 127), (128, 127), (128, 126), (127, 126)],
            vec![(127, 127), (127, 126)],
        ),
    ];

    for ((call_x, call_y), expected_calls, expected_valid_calls) in cases {
        let mut actual_crt = Win95CrtRand::new(0x0043_6166);
        let mut expected_crt = actual_crt;
        let mut expected_placements = Vec::new();
        for (x, y) in expected_calls {
            let abandoned = abandoned_building_for_pop_with_rng(1, &mut expected_crt)
                .expect("mode-4 pop-1 picker has two variants");
            if (0..CITY_WIDTH as isize).contains(&x) && (0..CITY_HEIGHT as isize).contains(&y) {
                expected_placements.push((x as usize, y as usize, abandoned));
            }
        }
        assert_eq!(
            expected_placements
                .iter()
                .map(|&(x, y, _)| (x, y))
                .collect::<Vec<_>>(),
            expected_valid_calls
        );

        let mut picked_calls = Vec::new();
        let placements =
            pe_changebuilding_pop2_replace_placements_with_picker(call_x, call_y, |x, y| {
                picked_calls.push((x, y));
                abandoned_building_for_pop_with_rng(1, &mut actual_crt)
            })
            .expect("all four PE pop-1 picks succeed, including off-map calls");

        assert_eq!(picked_calls, expected_calls);
        assert_eq!(placements, expected_placements);
        assert!(
            placements
                .iter()
                .all(|&(_, _, building)| (0x8A..=0x8B).contains(&building))
        );
        assert_eq!(
            actual_crt.seed(),
            expected_crt.seed(),
            "all four mode-4/pop-1 calls consume one draw, including off-map calls"
        );
    }
}

#[test]
fn pe_changebuilding_pop2_picker_precedes_edge_bounds_rejection() {
    for (call_x, call_y) in [(1, 22), (126, 22), (20, 1), (20, 126), (20, 0)] {
        let mut actual_crt = Win95CrtRand::new(0x0043_6166);
        let mut expected_crt = actual_crt;
        expected_crt.random_u15();

        let placements =
            pe_changebuilding_pop2_direct_placement_with_picker(call_x, call_y, || {
                abandoned_building_for_pop_with_rng(2, &mut actual_crt)
            })
            .expect("mode-4 pop-2 picker succeeds before PE bounds rejection");
        assert!(
            placements.is_empty(),
            "raw pop-2 call ({call_x},{call_y}) is outside inclusive [2,125] bounds"
        );
        assert_eq!(
            actual_crt.seed(),
            expected_crt.seed(),
            "PE consumes one picker draw before rejecting raw call ({call_x},{call_y})"
        );
    }

    for (call_x, call_y) in [(2, 2), (125, 2), (2, 125), (125, 125)] {
        assert_eq!(
            pe_changebuilding_pop2_direct_placement_with_picker(call_x, call_y, || Some(0xAA)),
            Some(vec![(call_x, call_y - 1, 0xAA)]),
            "raw pop-2 call ({call_x},{call_y}) lies on an inclusive PE bound"
        );
    }
}

#[test]
fn pe_changebuilding_pop3_uses_raw_call_tile_for_direct_and_downgrade() {
    let footprint = [(20, 20), (21, 20), (20, 21), (21, 21)];
    let cases = [
        ((20, 21), (20, 20)), // rotation 0 / SW
        ((20, 20), (20, 19)), // rotation 1 / NW
        ((21, 20), (21, 19)), // rotation 2 / NE
        ((21, 21), (21, 20)), // rotation 3 / SE
    ];

    for ((call_x, call_y), expected_origin) in cases {
        for (replace_tile, expected_buildings) in [(false, 0xAC..=0xAD), (true, 0xAA..=0xAB)] {
            let placements = rci_decline_change_building_at_call_optional(
                0x90,
                &footprint,
                call_x,
                call_y,
                20,
                20,
                replace_tile,
                None,
            )
            .expect("pop-3 abandoned change");
            assert_eq!(
                placements
                    .iter()
                    .map(|&(x, y, _)| (x, y))
                    .collect::<Vec<_>>(),
                vec![expected_origin],
                "replace_tile={replace_tile} keeps PE raw call tile ({call_x},{call_y})"
            );
            assert!(
                placements
                    .iter()
                    .all(|&(_, _, building)| expected_buildings.contains(&building))
            );
        }
    }
}

#[test]
fn pe_changebuilding_pop3_picker_precedes_edge_bounds_for_both_outcomes() {
    let picker_cases = [(3, 0xAC), (2, 0xAA)];
    let rejected_calls = [
        (0, 22),
        (1, 22),
        (126, 22),
        (127, 22),
        (22, 0),
        (22, 1),
        (22, 126),
        (22, 127),
    ];

    for (abandoned_pop, base) in picker_cases {
        for (call_x, call_y) in rejected_calls {
            let mut actual_crt = Win95CrtRand::new(0x0043_6166);
            let mut expected_crt = actual_crt;
            expected_crt.random_u15();

            let placements =
                pe_changebuilding_pop3_direct_placement_with_picker(call_x, call_y, || {
                    abandoned_building_for_pop_with_rng(abandoned_pop, &mut actual_crt)
                })
                .expect("mode-4 pop-2/3 picker succeeds before PE bounds rejection");
            assert!(
                placements.is_empty(),
                "raw pop-{abandoned_pop} call ({call_x},{call_y}) is outside [2,125]"
            );
            assert_eq!(
                actual_crt.seed(),
                expected_crt.seed(),
                "PE consumes one pop-{abandoned_pop} picker draw before rejecting ({call_x},{call_y})"
            );
        }

        for (call_x, call_y) in [(2, 2), (125, 2), (2, 125), (125, 125)] {
            let mut actual_crt = Win95CrtRand::new(0x0043_6166);
            let mut expected_crt = actual_crt;
            expected_crt.random_u15();

            let placements =
                pe_changebuilding_pop3_direct_placement_with_picker(call_x, call_y, || {
                    abandoned_building_for_pop_with_rng(abandoned_pop, &mut actual_crt)
                })
                .expect("inclusive PE edge call accepts the picker");
            assert_eq!(
                placements.len(),
                1,
                "raw pop-{abandoned_pop} call ({call_x},{call_y}) lies on an inclusive bound"
            );
            assert_eq!(
                (placements[0].0, placements[0].1),
                (call_x, call_y - 1),
                "pop-3 call maps to the expected 2x2 NW stamp origin"
            );
            assert!((base..=base + 1).contains(&placements[0].2));
            assert_eq!(actual_crt.seed(), expected_crt.seed());
        }
    }
}

#[test]
fn pe_changebuilding_pop4_replace_uses_raw_ring_order_and_scatter_call() {
    let footprint = building_footprint_from_origin(20, 20, 3).unwrap();
    let ring_offsets: [(isize, isize); 8] = [
        (0, 0),
        (1, 0),
        (2, 0),
        (2, -1),
        (2, -2),
        (1, -2),
        (0, -2),
        (0, -1),
    ];
    let call_tiles = [(20, 22), (20, 20), (22, 20), (22, 22)];

    for (call_x, call_y) in call_tiles {
        let placements = rci_decline_change_building_at_call_optional(
            0xAE, &footprint, call_x, call_y, 20, 20, true, None,
        )
        .expect("pop-4 replacement scatter");
        let expected_ring: Vec<_> = ring_offsets
            .iter()
            .map(|&(dx, dy)| {
                (
                    (call_x as isize + dx) as usize,
                    (call_y as isize + dy) as usize,
                )
            })
            .collect();
        assert_eq!(
            placements[..8]
                .iter()
                .map(|&(x, y, _)| (x, y))
                .collect::<Vec<_>>(),
            expected_ring,
            "pop-1 calls retain PE ring order from raw tile ({call_x},{call_y})"
        );
        assert!(
            placements[..8]
                .iter()
                .all(|&(_, _, building)| (0x8A..=0x8B).contains(&building))
        );

        let mut scatter_rng =
            rci_choose_and_place_isolated_rng(call_x, call_y, CHANGEBUILDING_SCATTER_KEY, 4);
        let roll = scatter_rng.random_u16_mask(3);
        let (scatter_x, scatter_y) =
            pop4_change_building_scatter_call_coords(call_x as isize, call_y as isize, roll);
        let expected_origin = (scatter_x as usize, (scatter_y - 1) as usize);
        assert_eq!(placements.len(), 9);
        assert_eq!(
            (placements[8].0, placements[8].1),
            expected_origin,
            "pop-3 stamp converts its raw ChooseAndPlace call by y-1"
        );
        if roll == 3 {
            assert!(
                expected_ring.contains(&expected_origin),
                "PE's final pop-3 PlaceTile may overwrite an earlier ring stamp"
            );
        }
        assert!((0xAC..=0xAD).contains(&placements[8].2));
    }
}

#[test]
fn pe_changebuilding_pop4_replace_picks_ring_then_roll_then_bounded_scatter() {
    for (call_x, call_y) in [(0, 0), (0, 127), (127, 0), (127, 127), (20, 20)] {
        for (expected_roll, seed) in [(0, 4), (1, 2), (2, 1), (3, 7)] {
            let mut actual_crt = Win95CrtRand::new(seed);
            let mut actual_steps = Vec::new();
            let placements =
                pe_changebuilding_pop4_replace_placements_with_picker(call_x, call_y, |step| {
                    actual_steps.push(step);
                    match step {
                        PeChangeBuildingPop4ReplaceStep::ChooseBuilding { pop, .. } => {
                            abandoned_building_for_pop_with_rng(pop, &mut actual_crt)
                                .map(PeChangeBuildingPop4ReplaceResult::Building)
                        }
                        PeChangeBuildingPop4ReplaceStep::ScatterRoll { .. } => {
                            Some(PeChangeBuildingPop4ReplaceResult::ScatterRoll(
                                actual_crt.random_u16_mask(3),
                            ))
                        }
                    }
                })
                .expect("all ring and scatter picker requests are valid");

            let ring_calls = pop4_change_building_replace_call_coords(call_x, call_y);
            let mut expected_crt = Win95CrtRand::new(seed);
            let mut expected_steps = Vec::new();
            let mut expected_placements = Vec::new();
            for (x, y) in ring_calls {
                expected_steps.push(PeChangeBuildingPop4ReplaceStep::ChooseBuilding {
                    pop: 1,
                    x,
                    y,
                });
                let building = abandoned_building_for_pop_with_rng(1, &mut expected_crt)
                    .expect("mode-4 pop-1 picker has two variants");
                if (0..CITY_WIDTH as isize).contains(&x) && (0..CITY_HEIGHT as isize).contains(&y) {
                    expected_placements.push((x as usize, y as usize, building));
                }
            }

            expected_steps.push(PeChangeBuildingPop4ReplaceStep::ScatterRoll {
                x: call_x,
                y: call_y,
            });
            let roll = expected_crt.random_u16_mask(3);
            assert_eq!(roll, expected_roll, "seed selects each scatter quadrant");
            let (scatter_x, scatter_y) =
                pop4_change_building_scatter_call_coords(call_x as isize, call_y as isize, roll);
            expected_steps.push(PeChangeBuildingPop4ReplaceStep::ChooseBuilding {
                pop: 3,
                x: scatter_x,
                y: scatter_y,
            });
            let scatter_building = abandoned_building_for_pop_with_rng(3, &mut expected_crt)
                .expect("mode-4 pop-3 picker has two variants");
            if (2..=125).contains(&scatter_x) && (2..=125).contains(&scatter_y) {
                expected_placements.push((
                    scatter_x as usize,
                    (scatter_y - 1) as usize,
                    scatter_building,
                ));
            }

            assert_eq!(actual_steps, expected_steps);
            assert_eq!(placements, expected_placements);
            assert_eq!(
                actual_crt.seed(),
                expected_crt.seed(),
                "eight ring picks, the scatter roll, and the pop-3 picker consume ten draws"
            );
            let valid_ring_count = ring_calls
                .iter()
                .filter(|&&(x, y)| {
                    (0..CITY_WIDTH as isize).contains(&x) && (0..CITY_HEIGHT as isize).contains(&y)
                })
                .count();
            assert!(
                placements[..valid_ring_count]
                    .iter()
                    .all(|&(_, _, building)| (0x8A..=0x8B).contains(&building))
            );
            if (2..=125).contains(&(call_x as isize))
                && (2..=125).contains(&(call_y as isize))
                && (2..=125).contains(&scatter_x)
                && (2..=125).contains(&scatter_y)
            {
                assert!((0xAC..=0xAD).contains(&placements.last().unwrap().2));
            }
        }
    }
}

#[test]
fn pe_changebuilding_pop4_direct_uses_raw_call_tile_and_y_minus_two() {
    let footprint = building_footprint_from_origin(20, 20, 3).unwrap();
    let cases = [
        ((20, 22), (20, 20)), // rotation 0 / SW
        ((20, 20), (20, 18)), // rotation 1 / NW
        ((22, 20), (22, 18)), // rotation 2 / NE
        ((22, 22), (22, 20)), // rotation 3 / SE
    ];

    for ((call_x, call_y), expected_origin) in cases {
        let placements = rci_decline_change_building_at_call_optional(
            0xAE, &footprint, call_x, call_y, 20, 20, false, None,
        )
        .expect("direct pop-4 abandonment");
        assert_eq!(placements.len(), 1);
        assert_eq!(
            (placements[0].0, placements[0].1),
            expected_origin,
            "replace=false stamps from raw ChooseAndPlace call ({call_x},{call_y})"
        );
        assert!((0xC4..=0xC5).contains(&placements[0].2));
    }

    let invalid_stamp =
        rci_decline_change_building_at_call_optional(0xAE, &footprint, 20, 1, 20, 20, false, None)
            .expect("pop-4 pick still succeeds before Y-origin underflow");
    assert!(invalid_stamp.is_empty());
}

#[test]
fn pe_changebuilding_pop4_picker_precedes_all_edge_call_rejections() {
    for (call_x, call_y) in [(1, 22), (125, 22), (20, 1), (20, 125)] {
        let mut actual_crt = Win95CrtRand::new(0x0043_6166);
        let mut expected_crt = actual_crt;
        expected_crt.random_u15();

        let placements =
            pe_changebuilding_pop4_direct_placement_with_picker(call_x, call_y, || {
                abandoned_building_for_pop_with_rng(4, &mut actual_crt)
            })
            .expect("mode-4 pop-4 picker succeeds before PE bounds rejection");
        assert!(
            placements.is_empty(),
            "raw pop-4 call ({call_x},{call_y}) is outside PE bounds"
        );
        assert_eq!(
            actual_crt.seed(),
            expected_crt.seed(),
            "PE consumes one picker draw before rejecting raw call ({call_x},{call_y})"
        );
    }

    for (call_x, call_y) in [(2, 2), (124, 124)] {
        assert_eq!(
            pe_changebuilding_pop4_direct_placement_with_picker(call_x, call_y, || Some(0xC4)),
            Some(vec![(call_x, call_y - 2, 0xC4)]),
            "raw pop-4 call ({call_x},{call_y}) lies on an inclusive PE bound"
        );
    }
}

#[test]
fn pop3_startbuilding_destroy_coords_skip_center() {
    assert_eq!(
        pop3_startbuilding_destroy_coords(20, 20).collect::<Vec<_>>(),
        vec![
            (20, 22),
            (20, 21),
            (20, 20),
            (21, 20),
            (22, 20),
            (22, 21),
            (22, 22),
            (21, 22),
        ]
    );
    assert!(!pop3_startbuilding_destroy_coords(20, 20).any(|tile| tile == (21, 21)));
}

#[test]
fn pop3_startbuilding_road_probes_match_pe_corners_and_ids() {
    let probes: Vec<_> = pop3_startbuilding_road_probes(20, 22).collect();
    assert_eq!(
        probes.iter().map(|&(x, y, _)| (x, y)).collect::<Vec<_>>(),
        vec![(19, 23), (19, 19), (23, 19), (23, 23)]
    );
    assert_eq!(probes[0].2, [0x23, 0x27, 0x28, 0x2B]);
    assert_eq!(probes[1].2, [0x24, 0x28, 0x29, 0x2B]);
    assert_eq!(probes[2].2, [0x25, 0x29, 0x2A, 0x2B]);
    assert_eq!(probes[3].2, [0x26, 0x2A, 0x27, 0x2B]);
    assert_eq!(
        pop3_startbuilding_road_probes(0, 22)
            .map(|(x, y, _)| (x, y))
            .collect::<Vec<_>>(),
        vec![(3, 19), (3, 23)]
    );
}
