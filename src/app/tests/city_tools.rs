use super::*;

#[test]
fn mac_zone_seven_event_five_places_f9_after_northwest_origin_walk() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-zone-seven-f9.sc2", "ZoneSevenF9").expect("blank city");
    city.kind = CityFileKind::MacCity;
    for y in 40..43 {
        for x in 40..43 {
            city.set_zone(x, y, 7);
        }
    }
    city.set_mac_state_misc_u32_at(915 * 4, 5);
    let mut rng = MacToolboxRng::new(0x1234);
    let seed_before = rng.seed();
    // Native scan starts at the southeast member; two north then two west
    // same-zone matches recover the footprint origin at (40, 40).
    assert_eq!(
        mac_budget_process_zone_seven_special_growth(&mut city, 42, 42, &mut rng),
        MacSpecialTileAttempt::Placed
    );
    assert_eq!(rng.seed(), seed_before);
    for y in 40..43 {
        for x in 40..43 {
            assert_eq!(city.building_at(x, y), 0xf9);
        }
    }
    assert_eq!(city.mac_zone7_census_count(15), 9);
    assert_eq!(city.zone_at(40, 40).unwrap().raw_bits() & 0xf0, 0x10);
    assert_eq!(city.zone_at(42, 40).unwrap().raw_bits() & 0xf0, 0x20);
    assert_eq!(city.zone_at(42, 42).unwrap().raw_bits() & 0xf0, 0x40);
    assert_eq!(city.zone_at(40, 42).unwrap().raw_bits() & 0xf0, 0x80);
}

#[test]
fn mac_scan_zone_seven_f9_footprint_changes_later_residue_candidate() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-zone-seven-f9-interleave.sc2",
        "MacScanZoneSevenF9",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(42, 42, 7);
    city.set_zone(41, 42, 7);
    city.set_zone(42, 41, 7);
    city.set_mac_state_misc_u32_at(915 * 4, 5);

    let mut rng = MacToolboxRng::new(0x7f9);
    let seed_before = rng.seed();
    // Phase 10 visits (42,42). Native probes the preceding row first, moving
    // the origin to (41,42), then probes the preceding byte-offset at (41,41)
    // and stops. Reversing the axes would instead shift the footprint to
    // (42,41); the row-first footprint reaches (42,43) in later phase 11.
    for phase in [10, 11] {
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, phase / 4, phase % 4, &mut rng);
    }

    assert_eq!(city.building_at(42, 43), 0xf9);
    assert_eq!(city.building_at(44, 42), 0);
    assert_eq!(city.building_at(42, 41), 0);
    for x in 41..=43 {
        for y in 42..=44 {
            assert_eq!(city.building_at(x, y), 0xf9, "footprint cell ({x},{y})");
        }
    }
    assert_eq!(city.zone_at(41, 42).unwrap().raw_bits(), 0x17);
    assert_eq!(city.zone_at(43, 42).unwrap().raw_bits(), 0x20);
    assert_eq!(city.zone_at(43, 44).unwrap().raw_bits(), 0x40);
    assert_eq!(city.zone_at(41, 44).unwrap().raw_bits(), 0x80);
    assert_eq!(city.zone_at(42, 43).unwrap().raw_bits(), 0x00);
    assert_eq!(city.mac_xbld_tile_count(0xf9), 7);
    assert_eq!(city.mac_zone7_census_count(15), 2);
    let footprint_tiles = (0..CITY_WIDTH)
        .flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)))
        .filter(|&(x, y)| city.building_at(x, y) == 0xf9)
        .count();
    assert_eq!(footprint_tiles, 9);
    assert_eq!(
        rng.seed(),
        seed_before,
        "event-5 footprint and repeat gate draw no RNG"
    );
}

#[test]
fn mac_zone_seven_event_six_tails_without_selector_draw_or_placement() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-zone-seven-event-six.sc2",
        "ZoneSevenEventSix",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, 7);
    city.set_mac_state_misc_u32_at(915 * 4, 6);
    let mut rng = MacToolboxRng::new(0x9abc);
    let seed_before = rng.seed();

    assert_eq!(
        mac_budget_process_zone_seven_special_growth(&mut city, 40, 40, &mut rng),
        MacSpecialTileAttempt::Refused
    );
    assert_eq!(rng.seed(), seed_before);
    assert_eq!(city.building_at(40, 40), 0);
}

#[test]
fn mac_zone_eight_scan_branch_preserves_selector_placement_and_random_cadence() {
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-eight-scan.sc2", "ZoneEightScan")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for y in 40..=44 {
        city.set_zone(41, y, 8);
    }
    city.set_tile_bits(41, 40, TileBits::new(0x40));

    let accepted_seed = (1..100_000)
        .find(|&seed| MacToolboxRng::new(seed).random_u16() & 3 == 0)
        .expect("accepted zone-eight selector seed");
    let mut accepted_rng = MacToolboxRng::new(accepted_seed);
    let mut expected_accepted_rng = MacToolboxRng::new(accepted_seed);
    let mut expected_deadline = 0;
    let mut actual_deadline = 0;
    let mut expected_city = city.clone();
    mac_budget_process_zone_eight_special_growth(
        &mut expected_city,
        41,
        40,
        &mut expected_accepted_rng,
        &mut expected_deadline,
    );
    mac_budget_process_zone_eight_special_growth(
        &mut expected_city,
        41,
        44,
        &mut expected_accepted_rng,
        &mut expected_deadline,
    );
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 1, 0, &mut accepted_rng);
    assert_eq!(accepted_rng.seed(), expected_accepted_rng.seed());
    assert_eq!(
        city.mac_xbld_tile_count(0xdd),
        expected_city.mac_xbld_tile_count(0xdd)
    );
    assert_eq!(city.building_at(41, 40), 0xdd);
    assert_eq!(city.building_at(41, 44), 0xdd);

    let rejected_seed = (1..100_000)
        .find(|&seed| MacToolboxRng::new(seed).random_u16() & 3 != 0)
        .expect("rejected zone-eight selector seed");
    let mut rejected_city = City::new_blank(
        "/tmp/sc2000-mac-zone-eight-rejected.sc2",
        "ZoneEightRejected",
    )
    .expect("blank city");
    rejected_city.kind = CityFileKind::MacCity;
    rejected_city.set_zone(41, 40, 8);
    let mut rejected_rng = MacToolboxRng::new(rejected_seed);
    let mut expected_rejected_rng = MacToolboxRng::new(rejected_seed);
    expected_rejected_rng.random_i16();
    assert_eq!(
        mac_budget_process_zone_eight_special_growth(
            &mut rejected_city,
            41,
            40,
            &mut rejected_rng,
            &mut actual_deadline,
        ),
        MacSpecialTileAttempt::Refused
    );
    assert_eq!(rejected_rng.seed(), expected_rejected_rng.seed());

    let third_draw_seed = (1..1_000_000)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            let first = probe.random_u16();
            let second = probe.random_u16();
            let third = probe.random_u16();
            first & 3 != 0 && second % 30 == 0 && third % 10 >= 4
        })
        .expect("zone-eight kind-1 selection seed");
    rejected_city.set_building(41, 40, 0xdd);
    rejected_city.set_tile_bits(41, 40, TileBits::new(0x40));
    rejected_city.mac_kind1_actor_counter = 2; // constructor rejects before its first draw
    let mut third_draw_rng = MacToolboxRng::new(third_draw_seed);
    let mut expected_third_draw_rng = MacToolboxRng::new(third_draw_seed);
    for _ in 0..3 {
        expected_third_draw_rng.random_i16();
    }
    assert_eq!(
        mac_budget_process_zone_eight_special_growth(
            &mut rejected_city,
            41,
            40,
            &mut third_draw_rng,
            &mut actual_deadline,
        ),
        MacSpecialTileAttempt::Refused
    );
    assert_eq!(third_draw_rng.seed(), expected_third_draw_rng.seed());
}

#[test]
fn mac_zone_eight_runway_branch_creates_kind2_with_native_random_order() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-zone-eight-kind2-constructor.sc2",
        "ZoneEightKind2",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y) = (41, 40);
    city.set_building(x, y, 0xdd);
    city.set_zone(x, y, 8);
    city.set_tile_bits(x, y, TileBits::new(0x40));
    city.set_text_overlay(x, y, 0x35);
    city.mac_object_slot_records_mut()[1].saved_b = 0x77;
    let mut sound_deadline = 0x1234_5678;

    let seed = (1..1_000_000)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            let first = probe.random_u16();
            let second = probe.random_u16();
            let third = probe.random_u16();
            first & 3 != 0 && second % 30 == 0 && third % 10 < 4
        })
        .expect("kind-2 runway constructor seed");
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_u16(); // zone-8 branch selector
    expected_rng.random_u16(); // runway modulo-30 gate
    expected_rng.random_u16(); // kind-2 versus kind-1 choice
    let expected_target_x = (expected_rng.random_u16() % 128) as u8;
    let expected_target_y = (expected_rng.random_u16() % 128) as u8;
    let expected_next = expected_rng.random_u16();

    let mut actual_rng = MacToolboxRng::new(seed);
    let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
    mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
        &mut city,
        1,
        0,
        &mut actual_rng,
        &mut game_rng,
        &mut sound_deadline,
    );
    assert_eq!(actual_rng.random_u16(), expected_next);

    let actor = city.traffic_overlays().expect("Mac XTHG table")[1];
    assert_eq!(actor.kind, 2);
    assert_eq!(actor.lane, 2);
    assert_eq!(actor.state, 0);
    assert_eq!((actor.tile_x, actor.tile_y), (y as u8, x as u8));
    assert_eq!((actor.height, actor.sub_x, actor.sub_y), (0, 8, 8));
    assert_eq!(
        (actor.target_x, actor.target_y),
        (expected_target_x, expected_target_y)
    );
    assert_eq!(actor.saved_a, 0x35);
    assert_eq!(actor.saved_b, 0x77); // export 115 does not write +0x0b
    assert_eq!(city.text_overlay_at(x, y), Some(0xca));
    assert_eq!(city.mac_kind2_actor_counter, 1);
    assert_eq!(sound_deadline, 0);
}

#[test]
fn mac_zone_eight_runway_branch_creates_kind1_with_native_mode_and_edge_mapping() {
    let seed_for = |direct: bool| {
        (1..1_000_000)
            .find(|&seed| {
                let mut probe = MacToolboxRng::new(seed);
                let first = probe.random_u16();
                let second = probe.random_u16();
                let third = probe.random_u16();
                let constructor = probe.random_u16();
                first & 3 != 0
                    && second % 30 == 0
                    && third % 10 >= 4
                    && (constructor % 10 >= 5) == direct
            })
            .expect("kind-1 runway constructor seed")
    };

    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for orientation in [false, true] {
            for candidate_bit in [false, true] {
                for direct in [false, true] {
                    let mut city = City::new_blank(
                        "/tmp/sc2000-mac-zone-eight-kind1-constructor.sc2",
                        "ZoneEightKind1",
                    )
                    .expect("blank city");
                    city.kind = kind;
                    city.saved_rotation = u8::from(orientation);
                    let (x, y) = (41, 40);
                    city.set_building(x, y, 0xdd);
                    city.set_zone(x, y, 8);
                    city.set_tile_bits(
                        x,
                        y,
                        TileBits::new(0x40 | if candidate_bit { 0x02 } else { 0 }),
                    );
                    city.set_text_overlay(x, y, 0x35);
                    city.mac_object_slot_records_mut()[1].saved_b = 0x77;
                    let mode = if orientation == candidate_bit { 0 } else { 2 };

                    let seed = seed_for(direct);
                    let mut expected_rng = MacToolboxRng::new(seed);
                    expected_rng.random_u16(); // zone-eight branch selector
                    expected_rng.random_u16(); // runway modulo-30 gate
                    expected_rng.random_u16(); // kind-2 versus kind-1 choice
                    let constructor_roll = expected_rng.random_u16();
                    let expected: (u8, u8, u8, u8, u8, u8, u8) = if constructor_roll % 10 >= 5 {
                        (mode, 0, 0, y as u8, x as u8, 20, 20)
                    } else {
                        let edge = (expected_rng.random_u16() & 3) as u8;
                        let inset = (expected_rng.random_u16() % 100 + 10) as u8;
                        let (row, column, heading) = match edge {
                            0 => (0, inset, 3),
                            1 => (inset, 0, 5),
                            2 => (127, inset, 7),
                            _ => (inset, 127, 1),
                        };
                        let (target_row, target_column) = if mode == 0 {
                            (y as u8, (x as u8).wrapping_add(16))
                        } else {
                            ((y as u8).wrapping_sub(16), x as u8)
                        };
                        (
                            heading,
                            if mode == 0 { 0x03 } else { 0x23 },
                            16,
                            row,
                            column,
                            target_row,
                            target_column,
                        )
                    };
                    let expected_next = expected_rng.random_u16();

                    let mut actual_rng = MacToolboxRng::new(seed);
                    let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
                    let mut sound_deadline = 0x1234_5678;
                    mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
                        &mut city,
                        1,
                        0,
                        &mut actual_rng,
                        &mut game_rng,
                        &mut sound_deadline,
                    );
                    assert_eq!(actual_rng.random_u16(), expected_next);

                    let actor = city.traffic_overlays().expect("Mac XTHG table")[1];
                    assert_eq!(
                        actor.kind, 1,
                        "layout={kind:?} orientation={orientation} bit={candidate_bit} direct={direct}"
                    );
                    assert_eq!(
                        (
                            actor.lane,
                            actor.state,
                            actor.height,
                            actor.tile_x,
                            actor.tile_y,
                            actor.target_x,
                            actor.target_y,
                        ),
                        expected,
                        "layout={kind:?} orientation={orientation} bit={candidate_bit} direct={direct}"
                    );
                    assert_eq!((actor.sub_x, actor.sub_y), (8, 8));
                    assert_eq!(actor.saved_a, 0x35);
                    assert_eq!(actor.saved_b, 0x77); // export 114 leaves +0x0b untouched
                    assert_eq!(city.text_overlay_at(x, y), Some(0xca));
                    assert_eq!(city.mac_kind1_actor_counter, 1);
                    assert_eq!(sound_deadline, 0x1234_5678);
                }
            }
        }
    }
}

#[test]
fn mac_scan_zone_eight_runway_carries_into_later_residues() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-zone-eight-runway-interleave.sc2",
        "MacScanZoneEightRunway",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let x = 41; // odd X and even Y select the positive vertical runway ray
    for y in 40..=44 {
        city.set_zone(x, y, 8);
    }
    city.set_tile_bits(x, 40, TileBits::new(0x40));

    // Phase 4 places the first five-cell ray. Its y=44 continuation is scanned
    // later in phase 4; y=41, 42, and 43 are candidates in phases 5, 6, and 7.
    // Every continuation candidate must take the existing-runway alternative
    // and stop before its third, timing-only draw.
    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            let anchor_gate = probe.random_i16() as u16;
            if anchor_gate & 3 != 0 {
                return false;
            }
            (0..4).all(|_| {
                let existing_runway_gate = probe.random_i16() as u16;
                let timing_gate = probe.random_i16() as u16;
                existing_runway_gate & 3 != 0 && timing_gate % 30 != 0
            })
        })
        .expect("find accepted anchor and no-placement continuation draws");
    let mut expected_rng = MacToolboxRng::new(seed);
    for _ in 0..9 {
        expected_rng.random_i16();
    }

    let mut rng = MacToolboxRng::new(seed);
    for phase in 0..=7 {
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, phase / 4, phase % 4, &mut rng);
    }

    for y in 40..=44 {
        assert_eq!(city.building_at(x, y), 0xdd, "y={y}");
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0xf8);
    }
    assert_eq!(city.building_at(x, 45), 0);
    assert_eq!(city.mac_xbld_tile_count(0xdd), 5);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_zone_nine_special_selector_falls_back_to_e3_after_e0_refusal() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-zone-nine-fallback.sc2", "ZoneNine").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, 9);
    city.set_tile_bits(40, 40, TileBits::new(0x40));

    // With empty selector counts the first candidate is e0. No nearby water
    // means its ray helper refuses, so CODE 8 retries the one-cell e3 arm.
    let seed = (1..100_000)
        .find(|&seed| MacToolboxRng::new(seed).random_u16() & 3 == 0)
        .expect("find accepted zone-nine selector draw");
    let mut rng = MacToolboxRng::new(seed);
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_i16();
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);
    assert_eq!(city.building_at(40, 40), 0xe3);
    assert_eq!(city.zone_at(40, 40).unwrap().raw_bits(), 0xf9);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_zone_nine_type_three_creation_preserves_record_bytes_and_draw_order() {
    for requested_edge in 0..4 {
        let mut city = City::new_blank("/tmp/sc2000-mac-zone-nine-type-three.sc2", "ZoneNine")
            .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_zone(40, 40, 9);
        city.set_building(40, 40, 0xe0);
        city.set_tile_bits(40, 40, TileBits::new(0x40));

        let seed = (1..1_000_000)
            .find(|&seed| {
                let mut probe = MacToolboxRng::new(seed);
                let first = probe.random_i16() as u16;
                let second = probe.random_i16() as u16;
                let edge = (probe.random_i16() as u16 & 3) as usize;
                first & 3 != 0 && second & 3 == 0 && edge == requested_edge
            })
            .expect("find zone-nine alternate-arm seed for each native edge");
        let mut expected_rng = MacToolboxRng::new(seed);
        expected_rng.random_i16();
        expected_rng.random_i16();
        let edge = (expected_rng.random_i16() as u16 & 3) as usize;
        assert_eq!(edge, requested_edge);
        // CODE 6 export 116 selector order uses the raw map-plane axes:
        // selectors 0/1 fix the first coordinate, 2/3 fix the second.
        // The scan keeps the last matching cell on each selected edge.
        let selected_edge = match edge {
            0 => (2, CITY_HEIGHT - 1),
            1 => (CITY_WIDTH - 2, CITY_HEIGHT - 1),
            2 => (CITY_WIDTH - 1, 2),
            _ => (CITY_WIDTH - 1, CITY_HEIGHT - 2),
        };
        let first_edge = match edge {
            0 => (2, 0),
            1 => (CITY_WIDTH - 2, 0),
            2 => (0, 2),
            _ => (0, CITY_HEIGHT - 2),
        };
        city.set_terrain(first_edge.0, first_edge.1, 0x10);
        city.set_text_overlay(first_edge.0, first_edge.1, 0x52);
        city.set_terrain(selected_edge.0, selected_edge.1, 0x10);
        // The constructor tests XTXT's admission byte and saves that byte; a
        // high XBLD at this edge must not block creation or become the proxy.
        city.set_building(selected_edge.0, selected_edge.1, 0xd4);
        city.set_text_overlay(selected_edge.0, selected_edge.1, 0x25);
        let records = city.traffic_overlays_mut().expect("blank XTHG table");
        records[1] = Win95OverlayRecord {
            target_x: 0x81,
            target_y: 0x82,
            saved_b: 0x83,
            ..Win95OverlayRecord::default()
        };

        let mut rng = MacToolboxRng::new(seed);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);
        assert_eq!(rng.seed(), expected_rng.seed());
        assert_eq!(city.building_at(selected_edge.0, selected_edge.1), 0xd4);
        assert_eq!(
            city.text_overlay_at(selected_edge.0, selected_edge.1),
            Some(0xca)
        );
        assert_eq!(city.text_overlay_at(first_edge.0, first_edge.1), Some(0x52));
        let record = city.traffic_overlays().unwrap()[1];
        assert_eq!(record.kind, 3);
        assert_eq!(record.state, 0);
        assert_eq!(record.tile_x, selected_edge.0 as u8); // Native +3 is first map coordinate.
        assert_eq!(record.tile_y, selected_edge.1 as u8); // Native +4 is second map coordinate.
        assert_eq!(record.height, 1);
        assert_eq!((record.sub_x, record.sub_y), (8, 8));
        assert_eq!(
            (record.target_x, record.target_y, record.saved_b),
            (0x81, 0x82, 0x83)
        );
        assert_eq!(record.saved_a, 0x25); // Previous XTXT, not previous XBLD.
        assert_eq!(city.mac_xbld_tile_count(0xd4), 1);
        assert_eq!(city.mac_xbld_tile_count(0xca), 0);
    }
}

#[test]
fn mac_zone_nine_type_three_counter_blocks_before_edge_draw() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-zone-nine-type-three-counter-block.sc2",
        "ZoneNineCounterBlock",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, 9);
    city.set_building(40, 40, 0xe0);
    city.set_tile_bits(40, 40, TileBits::new(0x40));
    city.set_terrain(126, 127, 0x10);
    city.mac_kind3_actor_counter = 1;

    let seed = (1..1_000_000)
        .find(|&seed| {
            let mut rng = MacToolboxRng::new(seed);
            let first = rng.random_i16() as u16;
            let second = rng.random_i16() as u16;
            first & 3 != 0 && second & 3 == 0
        })
        .expect("find alternate-arm seed");
    let mut rng = MacToolboxRng::new(seed);
    let mut expected = MacToolboxRng::new(seed);
    expected.random_i16();
    expected.random_i16();
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(
        rng.seed(),
        expected.seed(),
        "a closed native gate skips edge RNG"
    );
    assert_eq!(city.building_at(40, 40), 0xe0);
    assert_eq!(city.mac_kind3_actor_counter, 1);
    assert_eq!(city.traffic_overlays().unwrap()[1].kind, 0);
}

#[test]
fn mac_engine_publishes_native_power_summary_for_win_layout_city() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-engine-win-layout-power-summary.sc2",
        "MacEngineWinLayoutPowerSummary",
    )
    .expect("blank city");
    city.kind = CityFileKind::WinSc2;
    city.set_misc_u32_at(100 * 4, 0x1234_5678);
    for (x, building) in [(10, 0xC6), (11, 0x70), (12, 0x71)] {
        city.set_building(x, 20, building);
        city.set_tile_bits(x, 20, TileBits::new(0).with_powerable(true));
    }

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day1_power();

    assert_eq!(state.city.misc_u32_at(100 * 4), Some(0x1234_5678));

    assert_eq!(
        state.city.mac_power_service_summary(),
        5,
        "Mac phase 1 publishes its runtime summary independent of saved layout"
    );
    state.city.set_microsim_tile_id(0, 0xc9);
    let mut expected_rng = MacToolboxRng::new(1);
    let expected = 5_u16.wrapping_add(expected_rng.random_u16() & 7);
    state
        .city
        .refresh_mac_monthly_power_plant_microsims(&mut MacToolboxRng::new(1));
    assert_eq!(state.city.microsim_at(0).unwrap().stat2, expected);
    assert_eq!(state.city.misc_u32_at(100 * 4), Some(0x1234_5678));
}

#[test]
fn power_adjacent_building_fixture_powers_neighbor_after_xval_stamp() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    power_adjacent_building_for_test(&mut city, 20, 19);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let power_maps = power_service_maps(&state.city);
    assert!(
        power_maps.powered_tiles[20 * CITY_WIDTH + 20],
        "coal adjacent-plant fixture must still power neighbors after full-map XVAL stamp"
    );
}

#[test]
fn loaded_developed_building_tiles_have_visible_plumbing() {
    let Some(path) = paths::test_win_populated_city().or_else(paths::test_mac_city) else {
        return;
    };
    let mut state = test_state(City::load(&path).expect("load populated city"));
    state.refresh_simulation_service_bits();
    let city = &state.city;
    let mut checked = 0usize;
    let mut missing = Vec::new();

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = city.building_at(x, y);
            if !building_has_internal_pipes(building) {
                continue;
            }
            checked += 1;
            let render_tile = underground_render_tile(city, x, y, city.rotation());
            let overlay_tile = underground_building_pipe_overlay_tile(city, x, y, city.rotation());
            if render_tile.is_none() && overlay_tile.is_none() {
                missing.push((x, y, building, city.underground_at(x, y)));
                if missing.len() >= 16 {
                    break;
                }
            }
        }
        if missing.len() >= 16 {
            break;
        }
    }

    assert!(checked > 0, "expected developed building tiles in {path:?}");
    assert!(
        missing.is_empty(),
        "developed building tiles without visible plumbing in {path:?}: {missing:?}"
    );
}

#[test]
fn default_city_pipe_eligible_tiles_have_visible_plumbing() {
    let path = paths::default_city_path();
    if !path.exists() {
        return;
    }
    let mut state = test_state(City::load(&path).expect("load default city"));
    state.refresh_simulation_service_bits();
    let city = &state.city;
    let mut checked = 0usize;
    let mut missing = Vec::new();

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if !structural_internal_pipe_eligible(city, x, y) {
                continue;
            }
            checked += 1;
            let render_tile = underground_render_tile(city, x, y, city.rotation());
            let overlay_tile = underground_building_pipe_overlay_tile(city, x, y, city.rotation());
            if render_tile.is_none() && overlay_tile.is_none() {
                missing.push((
                    x,
                    y,
                    city.building_at(x, y),
                    city.zone_at(x, y).map(|zone| zone.raw_bits()).unwrap_or(0),
                    city.underground_at(x, y),
                ));
                if missing.len() >= 16 {
                    break;
                }
            }
        }
        if missing.len() >= 16 {
            break;
        }
    }

    assert!(
        checked > 0,
        "expected pipe-eligible building or zone tiles in {}",
        path.display()
    );
    assert!(
        missing.is_empty(),
        "pipe-eligible default city tiles without visible plumbing: {missing:?}"
    );
}

#[test]
fn pe_selected_height_drives_neighbor_and_destroy_cleanup_in_both_layouts() {
    // PE 0x4471b3 reads MISC slot 912's WORD, never a map-wide water maximum.
    for extension in ["sc2", "city"] {
        for (level, expected_xter, expected_water, wet) in
            [(4, 0, 9, false), (5, 0x20, 5, true), (6, 0x10, 6, true)]
        {
            for destroy in [false, true] {
                let mut city = blank_city_layout_fixture(
                    format!("/tmp/pe-selected-cleanup.{extension}"),
                    "SelectedCleanup",
                );
                for x in 0..CITY_WIDTH {
                    for y in 0..CITY_HEIGHT {
                        city.set_altitude(x, y, 4, 0);
                    }
                }
                city.set_mac_selected_water_height(level);
                city.set_altitude(100, 100, 4, 20);
                city.set_altitude(20, 20, 4, 9);
                city.set_terrain(20, 20, 3);
                city.set_building(20, 20, if destroy { 0x0d } else { 0 });
                city.set_underground(20, 20, 0x11);
                city.set_tile_bits(20, 20, TileBits::new(0xa5));
                city.set_traffic_density(20, 20, 0x37);
                let mut crt = Win95CrtRand::new(1);
                if destroy {
                    pe_flood_destroy_tile_with_crt(&mut city, &mut crt, 20, 20);
                } else {
                    pe_terrain_neighbor_fun_00447020_447490(&mut city, Some(&mut crt), 20, 20);
                }
                assert_eq!(
                    city.terrain_at(20, 20),
                    expected_xter,
                    "layout={extension} level={level} destroy={destroy}"
                );
                assert_eq!(city.water_height_at(20, 20), expected_water);
                let retained_bits = if destroy { 0x21 } else { 0xa1 };
                assert_eq!(
                    city.tile_bits_at(20, 20).unwrap().raw_bits(),
                    retained_bits | if wet { 4 } else { 0 }
                );
                assert_eq!(city.underground_at(20, 20), 0);
                assert_eq!(city.building_at(20, 20), 0);
                assert_eq!(city.traffic_density_at(20, 20), Some(0x37));
                assert_eq!(city.mac_selected_water_height(), Some(u32::from(level)));
            }
        }
    }
}

#[test]
fn placing_road_refreshes_neighbor_shapes() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.place_network_at(20, 20, NetworkKind::Road);
    state.place_network_at(21, 20, NetworkKind::Road);
    assert_eq!(state.city.building_at(20, 20), 30);
    assert_eq!(state.city.building_at(21, 20), 30);

    state.place_network_at(21, 21, NetworkKind::Road);
    assert_eq!(state.city.building_at(21, 20), 37);
    assert_eq!(state.city.building_at(21, 21), 29);
}

#[test]
fn road_neighbors_connect_through_crossovers_only_on_road_axis() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 69);
    let mut state = test_state(city);

    assert_eq!(
        state.network_neighbor_mask(20, 19, NetworkKind::Road),
        NETWORK_BOTTOM
    );
    assert_eq!(state.network_neighbor_mask(19, 20, NetworkKind::Road), 0);
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Rail),
        NETWORK_RIGHT
    );
    assert_eq!(state.network_neighbor_mask(20, 19, NetworkKind::Rail), 0);
    assert_eq!(
        state.place_network_path(&[(20, 19), (20, 20), (20, 21)], NetworkKind::Road),
        2
    );
    // PE PlaceRoads admits neighbors but refuses to write existing XBLD 0x45.
    assert_eq!(state.city.building_at(20, 20), 69);
    assert_eq!(state.city.building_at(20, 19), 29);
    assert_eq!(state.city.building_at(20, 21), 29);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 70);
    let state = test_state(city);
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Road),
        NETWORK_RIGHT
    );
    assert_eq!(state.network_neighbor_mask(20, 19, NetworkKind::Road), 0);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 45);
    let mut state = test_state(city);
    assert_eq!(
        state.place_network_path(&[(20, 19), (20, 20), (20, 21)], NetworkKind::Road),
        3
    );
    assert_eq!(state.city.building_at(20, 20), 69);
    assert_eq!(state.city.building_at(20, 19), 29);
    assert_eq!(state.city.building_at(20, 21), 29);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 45);
    let mut state = test_state(city);
    // PE PlaceRoads table `0x44ae24`: existing rail 0x2d → crossover 0x45 regardless of path axis.
    let placement =
        state.network_placement_for_path(&[(19, 20), (20, 20), (21, 20)], NetworkKind::Road);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(placement.valid_tiles, vec![(19, 20), (20, 20), (21, 20)]);
    assert_eq!(
        state.place_network_path(&[(19, 20), (20, 20), (21, 20)], NetworkKind::Road),
        3
    );
    assert_eq!(state.city.building_at(20, 20), 0x45);
}

#[test]
fn mac68k_road_neighbors_connect_through_crossovers_only_on_road_axis() {
    // Shared crossover-axis neighbor connect (not kind.contains-all-sides).
    let mut city =
        City::new_blank("/tmp/sc2000-cross-axis-mac.sc2", "CrossAxisMac").expect("blank");
    city.set_building(20, 20, 69);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.network_neighbor_mask(20, 19, NetworkKind::Road),
        NETWORK_BOTTOM
    );
    assert_eq!(state.network_neighbor_mask(19, 20, NetworkKind::Road), 0);
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Rail),
        NETWORK_RIGHT
    );
    assert_eq!(state.network_neighbor_mask(20, 19, NetworkKind::Rail), 0);
}

#[test]
fn mac68k_road_neighbors_omit_power_road_0x44_like_pe_place_roads() {
    let mut city = City::new_blank("/tmp/sc2000-road-044-mac.sc2", "Road044Mac").expect("blank");
    city.set_building(20, 20, 0x44);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.network_neighbor_mask(19, 20, NetworkKind::Road), 0);
    assert_eq!(state.network_neighbor_mask(20, 19, NetworkKind::Road), 0);
}

#[test]
fn mac68k_place_road_clear_zone_type_keep_corners() {
    let mut city = City::new_blank("/tmp/sc2000-net-xzon-mac.sc2", "NetXzonMac").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 1);
    assert_eq!(state.city.zone_at(20, 20).map(|z| z.raw_bits()), Some(0x80));
}

#[test]
fn place_road_rail_clear_zone_type_keep_corners_power_leaves_xzon() {
    // PE PlaceRoads `0x44ada6` / PlaceRails `0x44b276`: XZON &= 0xf0; Power leaves XZON.
    let mut city = City::new_blank("/tmp/sc2000-net-xzon.sc2", "NetXzon").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut state = test_state(city);
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 1);
    assert_eq!(state.city.zone_at(20, 20).map(|z| z.raw_bits()), Some(0x80));

    let mut city = City::new_blank("/tmp/sc2000-net-xzon-r.sc2", "NetXzonR").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut state = test_state(city);
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Rail), 1);
    assert_eq!(state.city.zone_at(20, 20).map(|z| z.raw_bits()), Some(0x80));

    let mut city = City::new_blank("/tmp/sc2000-net-xzon-m.sc2", "NetXzonM").expect("blank");
    city.set_zone(20, 20, ZONE_MILITARY | 0x80);
    let mut state = test_state(city);
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 1);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_MILITARY | 0x80)
    );

    let mut city = City::new_blank("/tmp/sc2000-net-xzon-p.sc2", "NetXzonP").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut state = test_state(city);
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 1);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x80)
    );
}

#[test]
fn road_neighbors_omit_power_road_0x44_like_pe_place_roads() {
    // PE PlaceRoads band includes 0x44 but gates NS=0 EW=0 — no Road neighbor connect.
    let mut city = City::new_blank("/tmp/sc2000-road-044-n.sc2", "Road044N").expect("blank");
    city.set_building(20, 20, 0x44);
    let state = test_state(city);
    assert_eq!(state.network_neighbor_mask(20, 19, NetworkKind::Road), 0);
    assert_eq!(state.network_neighbor_mask(19, 20, NetworkKind::Road), 0);
    assert_eq!(state.network_neighbor_mask(21, 20, NetworkKind::Road), 0);
    assert_eq!(state.network_neighbor_mask(20, 21, NetworkKind::Road), 0);
    let mut city = City::new_blank("/tmp/sc2000-road-044-rf.sc2", "Road044Rf").expect("blank");
    city.set_building(20, 20, 0x44);
    let mut state = test_state(city);
    state.refresh_network_tile(20, 20, NetworkKind::Road);
    assert_eq!(state.city.building_at(20, 20), 0x44);
}

#[test]
fn road_neighbors_connect_power_road_0x43_all_sides_like_pe_place_roads() {
    // PE PlaceRoads `0x3f..0x47` + gates: 0x43 has NS=1 EW=1 (all sides), not axis-only.
    let mut city = City::new_blank("/tmp/sc2000-road-043-n.sc2", "Road043N").expect("blank");
    city.set_building(20, 20, 0x43);
    let state = test_state(city);
    assert_eq!(
        state.network_neighbor_mask(20, 19, NetworkKind::Road),
        NETWORK_BOTTOM
    );
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Road),
        NETWORK_RIGHT
    );
    assert_eq!(
        state.network_neighbor_mask(21, 20, NetworkKind::Road),
        NETWORK_LEFT
    );
    assert_eq!(
        state.network_neighbor_mask(20, 21, NetworkKind::Road),
        NETWORK_TOP
    );
    // Stay out of Road.contains: refresh must not smash crossover art.
    let mut city = City::new_blank("/tmp/sc2000-road-043-rf.sc2", "Road043Rf").expect("blank");
    city.set_building(20, 20, 0x43);
    let mut state = test_state(city);
    state.refresh_network_tile(20, 20, NetworkKind::Road);
    assert_eq!(state.city.building_at(20, 20), 0x43);
}

#[test]
fn mac68k_road_neighbors_connect_elevated_roads_like_pe_place_roads_reshape() {
    let mut city = City::new_blank("/tmp/sc2000-road-elev-mac.sc2", "RoadElevMac").expect("blank");
    city.set_building(20, 20, 0x3f);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.network_neighbor_mask(20, 19, NetworkKind::Road),
        NETWORK_BOTTOM
    );
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Road),
        NETWORK_RIGHT
    );
}

#[test]
fn road_neighbors_connect_elevated_roads_like_pe_place_roads_reshape() {
    // PE PlaceRoads reshape neighbor accept `0x3f..0x47` + gates: elevated 0x3f..=0x42
    // connect all sides. Stay out of Road.contains so refresh does not smash art.
    let mut city = City::new_blank("/tmp/sc2000-road-elev-n.sc2", "RoadElevN").expect("blank");
    city.set_building(20, 20, 0x3f);
    let state = test_state(city);
    assert_eq!(
        state.network_neighbor_mask(20, 19, NetworkKind::Road),
        NETWORK_BOTTOM
    );
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Road),
        NETWORK_RIGHT
    );
    assert_eq!(
        state.network_neighbor_mask(21, 20, NetworkKind::Road),
        NETWORK_LEFT
    );
    assert_eq!(
        state.network_neighbor_mask(20, 21, NetworkKind::Road),
        NETWORK_TOP
    );
}

#[test]
fn road_neighbors_do_not_treat_plain_highways_as_all_sides() {
    // PE PlaceRoads reshape neighbor accept omits plain highways 0x49/0x4a;
    // only highway×road crossovers 0x4b/0x4c connect (via crossover axis).
    let mut city = City::new_blank("/tmp/sc2000-road-hwy-neighbor.sc2", "RoadHwyN").expect("blank");
    city.set_building(20, 20, 0x49);
    let state = test_state(city);
    assert_eq!(state.network_neighbor_mask(20, 19, NetworkKind::Road), 0);
    assert_eq!(state.network_neighbor_mask(19, 20, NetworkKind::Road), 0);

    let mut city = City::new_blank("/tmp/sc2000-road-hwy-cross-n.sc2", "RoadHwyCN").expect("blank");
    city.set_building(20, 20, 0x4b);
    let state = test_state(city);
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Road),
        NETWORK_RIGHT
    );
    assert_eq!(state.network_neighbor_mask(20, 19, NetworkKind::Road), 0);
}

#[test]
fn mac68k_road_neighbors_do_not_treat_plain_highways_as_all_sides() {
    let mut city = City::new_blank("/tmp/sc2000-road-hwy-n-mac.sc2", "RoadHwyNMac").expect("blank");
    city.set_building(20, 20, 0x49);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.network_neighbor_mask(19, 20, NetworkKind::Road), 0);
    let mut city =
        City::new_blank("/tmp/sc2000-road-hwy-cn-mac.sc2", "RoadHwyCNMac").expect("blank");
    city.set_building(20, 20, 0x4b);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Road),
        NETWORK_RIGHT
    );
}

#[test]
fn power_lines_cross_single_straight_road_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 30);
    let mut state = test_state(city);

    let vertical_power = [(20, 19), (20, 20), (20, 21)];
    let placement = state.network_placement_for_path(&vertical_power, NetworkKind::Power);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(placement.valid_tiles, vertical_power);
    assert_eq!(
        state.place_network_path(&vertical_power, NetworkKind::Power),
        3
    );
    assert_eq!(state.city.building_at(20, 20), 68);
    // PE PlaceRoads has no road-connect gate through power/road XBLD 0x44.
    assert_eq!(state.network_neighbor_mask(19, 20, NetworkKind::Road), 0);
    // PE power reshape gates: crossover XBLD 68 has NS/EW gate 0 — not a neighbor.
    assert_eq!(state.network_neighbor_mask(20, 19, NetworkKind::Power), 0);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 29);
    let mut state = test_state(city);

    let horizontal_power = [(19, 20), (20, 20), (21, 20)];
    let placement = state.network_placement_for_path(&horizontal_power, NetworkKind::Power);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(placement.valid_tiles, horizontal_power);
    assert_eq!(
        state.place_network_path(&horizontal_power, NetworkKind::Power),
        3
    );
    assert_eq!(state.city.building_at(20, 20), 67);
    assert_eq!(
        state.network_neighbor_mask(20, 19, NetworkKind::Road),
        NETWORK_BOTTOM
    );
    // Crossover 67 is NS+EW gated; powerable stamp lets RIGHT connect.
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Power),
        NETWORK_RIGHT
    );

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 30);
    let mut state = test_state(city);

    let placement = state.network_placement_for_path(&[(20, 20)], NetworkKind::Power);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(placement.valid_tiles, vec![(20, 20)]);
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(20, 20), 68);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 29);
    let mut state = test_state(city);

    let placement = state.network_placement_for_path(&[(20, 20)], NetworkKind::Power);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(placement.valid_tiles, vec![(20, 20)]);
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(20, 20), 67);
}

#[test]
fn roads_cross_single_straight_power_line_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 14);
    let mut state = test_state(city);

    let placement = state.network_placement_for_path(&[(20, 20)], NetworkKind::Road);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(placement.valid_tiles, vec![(20, 20)]);
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(20, 20), 68);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 15);
    let mut state = test_state(city);

    let placement = state.network_placement_for_path(&[(20, 20)], NetworkKind::Road);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(placement.valid_tiles, vec![(20, 20)]);
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(20, 20), 67);
}

#[test]
fn roads_do_not_cross_curved_power_line_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 20);
    let mut state = test_state(city);

    let placement = state.network_placement_for_path(&[(20, 20)], NetworkKind::Road);
    assert_eq!(placement.invalid_tiles, vec![(20, 20)]);
    assert!(placement.valid_tiles.is_empty());
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Road), 0);
    assert_eq!(state.city.building_at(20, 20), 20);
}

#[test]
fn power_line_drag_snaps_to_one_grid_axis_instead_of_zigzagging() {
    assert_eq!(
        network_drag_path_for_kind((10, 10), (13, 12), NetworkKind::Power),
        vec![(10, 10), (11, 10), (12, 10), (13, 10)]
    );
    assert_eq!(
        network_drag_path_for_kind((10, 10), (13, 12), NetworkKind::Road),
        network_drag_path((10, 10), (13, 12))
    );
}

#[test]
fn rail_drag_preview_alternates_like_power_but_commit_uses_road_diagonal() {
    assert_eq!(
        network_preview_path_for_kind((10, 10), (13, 12), NetworkKind::Rail),
        power_line_drag_path((10, 10), (13, 12))
    );
    assert_eq!(
        network_commit_path_for_kind((10, 10), (13, 12), NetworkKind::Rail),
        network_drag_path((10, 10), (13, 12))
    );
    assert_eq!(
        network_preview_path_for_kind((10, 10), (13, 10), NetworkKind::Rail),
        vec![(10, 10), (11, 10), (12, 10), (13, 10)]
    );
    assert_eq!(
        network_commit_path_for_kind((10, 10), (13, 10), NetworkKind::Rail),
        vec![(10, 10), (11, 10), (12, 10), (13, 10)]
    );
}

#[test]
fn placing_diagonal_rail_drag_uses_road_style_diagonal_tracks() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    let path = network_commit_path_for_kind((20, 20), (23, 18), NetworkKind::Rail);

    assert_eq!(path, network_drag_path((20, 20), (23, 18)));
    assert_eq!(
        state.place_network_path(&path, NetworkKind::Rail),
        path.len()
    );
    for &(x, y) in &path {
        assert!(NetworkKind::Rail.contains(state.city.building_at(x, y)));
    }
    assert!(path.iter().any(|&tile| tile.0 != 20 && tile.1 != 20));
}

#[test]
fn sub_to_rail_tool_matches_stock_city_surface_orientations() {
    let cases = [
        (NETWORK_RIGHT, 108),
        (NETWORK_BOTTOM, 109),
        (NETWORK_LEFT, 110),
        (NETWORK_TOP, 111),
    ];

    for (rail_side, building) in cases {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(20_000);
        let transition = (64, 64);
        let rail_tile = tile_next_to_side(transition.0, transition.1, rail_side);
        let rail_mask = match network_side_axis(rail_side) {
            NetworkAxis::GridX => NETWORK_LEFT | NETWORK_RIGHT,
            NetworkAxis::GridY => NETWORK_TOP | NETWORK_BOTTOM,
        };
        city.set_building(
            rail_tile.0,
            rail_tile.1,
            network_building_for_mask(NetworkKind::Rail, rail_mask),
        );
        let mut state = test_state(city);
        state.tool = EditTool::SubToRail;

        let placement = state
            .tool_placement_at(transition.0, transition.1)
            .expect("sub-to-rail placement");
        assert_eq!(placement.preview, ToolPreview::Building(building));
        assert_eq!(placement.valid_tiles, vec![transition]);
        assert!(placement.invalid_tiles.is_empty());

        assert!(state.apply_tool_at_tile(transition.0, transition.1));
        assert_eq!(state.city.building_at(transition.0, transition.1), building);
        assert_eq!(
            state.city.underground_at(transition.0, transition.1),
            UNDER_TILE_GREY_BOX_2
        );
        for side in [NETWORK_TOP, NETWORK_RIGHT, NETWORK_BOTTOM, NETWORK_LEFT] {
            assert_eq!(
                network_building_connects_to(building, NetworkKind::Rail, side),
                side == rail_side,
                "transition {building} has the wrong surface side"
            );
        }
        assert_ne!(
            state.network_neighbor_mask(rail_tile.0, rail_tile.1, NetworkKind::Rail)
                & opposite_network_side(rail_side),
            0,
            "rail does not connect to transition {building}"
        );
    }
}

#[test]
fn selected_sub_station_refuses_military_zone_before_xund_write() {
    // PE CityTool validation at 0x440d64 rejects XZON type 7 before reaching
    // PlaceXUND(0x23); lower-level PlaceXUND military behavior is separate.
    let mut city = City::new_blank("/tmp/sc2000-sub-xund-mil.sc2", "SubXundMil").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(20, 20, ZONE_MILITARY | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::SubStation;
    assert!(!state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.underground_at(20, 20), 0);
    assert_eq!(state.city.city_funds(), 20_000);
}

#[test]
fn bulldoze_sub_station_clears_xund_via_place_xund() {
    // PE subway clear uses PlaceXUND(0) (e.g. `0x443427`); military still writes.
    let mut city = City::new_blank("/tmp/sc2000-bulldoze-sub-xund.sc2", "BulSubXu").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 233);
    city.set_underground(20, 20, UNDER_TILE_GREY_BOX_2);
    city.set_zone(20, 20, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_tile_bits(20, 20, TileBits::new(0).with_watered(true));
    city.set_traffic_density(20, 20, 0x21);
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;
    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert_eq!(state.city.underground_at(20, 20), 0);
    assert!(state.city.tile_bits_at(20, 20).unwrap().watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x21));
}

#[test]
fn sub_to_rail_preserves_xtxt_like_pe_place_tile() {
    // PE PlaceTile + 0x4408dd: stamp XBLD / clear piped; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-subrail-xtxt.sc2", "SubRailT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(20, 20, 0x39);
    city.set_building(21, 20, 0x2c);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    assert!(state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x39));
}

#[test]
fn mac68k_sub_to_rail_preserves_xtrf_like_pe_4408dd() {
    // Shared SubToRail clears piped and leaves XTRF; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-subrail-xtrf.sc2", "MacSubRail").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x44);
    city.set_building(21, 20, 0x2c);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::SubToRail;
    assert!(state.place_sub_to_rail_at(20, 20));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(!bits.piped(), "PE clears piped on sub-rail access");
    assert!(bits.watered(), "invent water refresh must not wipe watered");
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x44));
}

#[test]
fn mac68k_sub_to_rail_preserves_xtxt_like_pe_place_tile() {
    // Shared SubToRail PlaceTile path leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-subrail-xtxt.sc2", "MacSubRailT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(20, 20, 0x39);
    city.set_building(21, 20, 0x2c);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::SubToRail;
    assert!(state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x39));
}

#[test]
fn onramp_tool_encodes_all_saved_city_orientations() {
    let cases = [
        (NETWORK_TOP, NETWORK_RIGHT, 93, false),
        (NETWORK_TOP, NETWORK_LEFT, 94, false),
        (NETWORK_BOTTOM, NETWORK_LEFT, 95, false),
        (NETWORK_BOTTOM, NETWORK_RIGHT, 96, false),
        (NETWORK_LEFT, NETWORK_BOTTOM, 93, true),
        (NETWORK_LEFT, NETWORK_TOP, 94, true),
        (NETWORK_RIGHT, NETWORK_TOP, 95, true),
        (NETWORK_RIGHT, NETWORK_BOTTOM, 96, true),
    ];

    for (highway_side, road_side, building, rotate) in cases {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(20_000);
        let mut state = test_state(city);
        state.tool = EditTool::Onramp;
        let ramp = (64, 64);
        let road_tile = stamp_onramp_fixture(&mut state, ramp, highway_side, road_side);

        let placement = state
            .tool_placement_at(ramp.0, ramp.1)
            .expect("on-ramp placement");
        assert_eq!(placement.preview, ToolPreview::Building(building));
        assert_eq!(placement.valid_tiles, vec![ramp]);
        assert!(placement.invalid_tiles.is_empty());

        assert!(state.place_onramp_at(ramp.0, ramp.1));
        assert_eq!(state.city.building_at(ramp.0, ramp.1), building);
        assert_eq!(
            state.city.tile_bits_at(ramp.0, ramp.1).unwrap().rotate(),
            rotate
        );
        assert_ne!(
            state.network_neighbor_mask(road_tile.0, road_tile.1, NetworkKind::Road)
                & opposite_network_side(road_side),
            0,
            "road does not connect to ramp {building}"
        );
    }
}

#[test]
fn onramp_tool_rejects_missing_or_same_axis_connections() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let state = test_state(city);
    let placement = state.onramp_placement_at(64, 64);
    assert!(placement.valid_tiles.is_empty());
    assert_eq!(placement.invalid_tiles, vec![(64, 64)]);

    assert_eq!(onramp_building_for_sides(NETWORK_TOP, NETWORK_BOTTOM), None);
    assert_eq!(onramp_building_for_sides(0, NETWORK_RIGHT), None);
}

#[test]
fn bulldozing_onramp_disconnects_and_refreshes_adjacent_road() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    let ramp = (64, 64);
    let road_side = NETWORK_RIGHT;
    let road_tile = stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, road_side);

    assert!(state.place_onramp_at(ramp.0, ramp.1));
    let ramp_direction = opposite_network_side(road_side);
    assert_ne!(
        state.network_neighbor_mask(road_tile.0, road_tile.1, NetworkKind::Road) & ramp_direction,
        0
    );

    state.bulldoze_at(ramp.0, ramp.1);

    assert!(matches!(state.city.building_at(ramp.0, ramp.1), 1..=4));
    assert_eq!(
        state.network_neighbor_mask(road_tile.0, road_tile.1, NetworkKind::Road) & ramp_direction,
        0
    );
}

#[test]
fn highway_drag_uses_two_tile_stride_and_stamps_two_by_two_segments() {
    assert_eq!(
        highway_drag_path((10, 10), (16, 10)),
        vec![(10, 10), (12, 10), (14, 10), (16, 10)]
    );
    assert_eq!(
        highway_drag_path((16, 10), (10, 10)),
        vec![(16, 10), (14, 10), (12, 10), (10, 10)]
    );
    assert_eq!(
        highway_drag_path((11, 11), (17, 11)),
        vec![(10, 10), (12, 10), (14, 10), (16, 10)]
    );

    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    let path = highway_drag_path((10, 10), (14, 10));

    assert_eq!(state.place_highway_path(&path), 3);
    for origin_x in [10, 12, 14] {
        for y in 10..=11 {
            for x in origin_x..=origin_x + 1 {
                assert_eq!(state.city.building_at(x, y), 74, "highway at ({x},{y})");
                assert_eq!(
                    state.city.zone_at(x, y).unwrap().raw_bits(),
                    0xf0,
                    "PE PlaceHighwayTile class 0 underlay ORs XZON 0xf0 ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn highway_drag_uses_pe_larger_delta_step_and_y_tie_break() {
    assert_eq!(
        highway_drag_path((10, 10), (16, 16)),
        vec![
            (10, 10),
            (10, 12),
            (12, 12),
            (12, 14),
            (14, 14),
            (14, 16),
            (16, 16),
        ]
    );
    assert_eq!(
        highway_drag_path((10, 10), (16, 14)),
        vec![(10, 10), (12, 10), (12, 12), (14, 12), (14, 14), (16, 14)]
    );
    assert_eq!(
        highway_drag_path((16, 16), (10, 10)),
        vec![
            (16, 16),
            (16, 14),
            (14, 14),
            (14, 12),
            (12, 12),
            (12, 10),
            (10, 10),
        ]
    );
}

#[test]
fn highway_stamp_axis_at_turn_uses_pe_saved_outgoing_direction() {
    assert_eq!(
        highway_axis_for_path_tile(EngineFidelity::Win95, &[(20, 20)], 0),
        NetworkAxis::GridY,
        "zero-delta FUN_0044C680 initialization chooses the y axis"
    );
    let path = [(10, 10), (12, 10), (12, 12), (14, 12)];
    assert_eq!(
        highway_axis_for_path_tile(EngineFidelity::Win95, &path, 0),
        NetworkAxis::GridX
    );
    assert_eq!(
        highway_axis_for_path_tile(EngineFidelity::Win95, &path, 1),
        NetworkAxis::GridY
    );
    assert_eq!(
        highway_axis_for_path_tile(EngineFidelity::Win95, &path, 2),
        NetworkAxis::GridX
    );
    // The terminal stamp keeps the last accepted direction.
    assert_eq!(
        highway_axis_for_path_tile(EngineFidelity::Win95, &path, 3),
        NetworkAxis::GridX
    );
    // Keep the preexisting Mac-fidelity two-axis turn interpretation.
    assert_eq!(
        highway_axis_for_path_tile(EngineFidelity::Mac68k, &path, 1),
        NetworkAxis::GridX
    );
}

#[test]
fn highway_drag_continues_over_later_road_and_can_start_on_one() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(14, 10, 44);
    let mut state = test_state(city);
    let path = highway_drag_path((10, 10), (16, 10));

    let placement = state.highway_drag_placement_for_path(&path);
    assert!(placement.valid_tiles.contains(&(10, 10)));
    assert!(placement.valid_tiles.contains(&(13, 11)));
    assert!(
        placement.valid_tiles.contains(&(14, 10)),
        "PE CityTool gate allows later XBLD 0x2c"
    );
    assert_eq!(state.place_highway_path(&path), 4);
    assert_eq!(state.city.building_at(14, 10), 74);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 44);
    let mut state = test_state(city);
    let path = highway_drag_path((10, 10), (14, 10));

    assert_eq!(state.place_highway_path(&path), 3);
    assert_eq!(state.city.building_at(10, 10), 74);
    assert_eq!(state.city.building_at(14, 10), 74);
}

#[test]
fn mac68k_stamp_flat_highway_tiles_leaves_xzon_like_pe_place_tile() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-xzon-mac.sc2", "HwyXzonMac").expect("blank");
    let tiles = highway_segment_tiles((10, 10), NetworkAxis::GridX);
    for &(x, y) in &tiles {
        city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT | 0x80);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_flat_highway_tiles(106, &tiles);
    assert_eq!(
        state.city.zone_at(10, 10).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x80)
    );
}

#[test]
fn stamp_flat_highway_tiles_leaves_xzon_like_pe_place_tile() {
    // PE PlaceTile stamps XBLD only — invent set_zone(0) removed from stamp_flat_highway_tiles.
    let mut city = City::new_blank("/tmp/sc2000-hwy-xzon.sc2", "HwyXzon").expect("blank");
    let tiles = highway_segment_tiles((10, 10), NetworkAxis::GridX);
    for &(x, y) in &tiles {
        city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT | 0x80);
    }
    let mut state = test_state(city);
    state.stamp_flat_highway_tiles(106, &tiles);
    for &(x, y) in &tiles {
        assert_eq!(state.city.building_at(x, y), 106);
        assert_eq!(
            state.city.zone_at(x, y).map(|z| z.raw_bits()),
            Some(ZONE_RESIDENTIAL_LIGHT | 0x80)
        );
    }
}

#[test]
fn highway_flat_stamp_clears_xzon_type_nibble_like_citytool_4626d0() {
    // PE `FUN_004626d0` clear then PlaceHighwayTile `FUN_00463b30` ORs `0xf0`
    // corners (`0x463bba`). Type nibble stays cleared.
    let mut city = City::new_blank("/tmp/sc2000-hwy-xzon-clear.sc2", "HwyXzonClr").expect("blank");
    city.set_city_funds(50_000);
    let tiles = highway_segment_tiles((20, 20), NetworkAxis::GridX);
    for &(x, y) in &tiles {
        city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT | 0x80);
    }
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    for &(x, y) in &tiles {
        assert!(is_highway_tile(state.city.building_at(x, y)));
        assert_eq!(
            state.city.zone_at(x, y).map(|z| z.raw_bits()),
            Some(0xf0),
            "XZON type cleared then corners OR 0xf0 at ({x},{y})"
        );
    }
}

#[test]
fn highway_flat_stamp_origin_get_placetile_like_4627a5() {
    // PE `0x4627a5`–`0x4627bc`: GetHighwayTile → if ax>=0 PlaceHighwayTile.
    // A zero-delta one-origin drag initializes direction 2 (GridY), so its
    // vacant flat footprint is class 2 → XBLD `0x49`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-getplace.sc2", "HwyGetPlace").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    let tiles = highway_segment_tiles((20, 20), NetworkAxis::GridY);
    for &(x, y) in &tiles {
        assert_eq!(state.city.building_at(x, y), 0x49);
    }
    state.city.set_altitude(22, 22, 2, 0);
    pe_place_highway_tile_462ed0_class_lt4(&mut state.city, 22, 22, 2);
    assert_eq!(state.city.terrain_at(22, 22), 0x0d);
    assert_eq!(state.city.terrain_at(23, 22), 0);
    assert_eq!(state.city.building_at(22, 22), 0x49);
    // OOB GetHighwayTile refuse. Existing `0x4a` with an empty neighbor mask
    // follows `0x4c7d60` (`0x462c45`), so GridY is class 2, not the building id.
    assert_eq!(
        pe_get_highway_tile_462910(&state.city, 0x80, 20, NetworkAxis::GridX),
        -1
    );
    assert_eq!(
        pe_get_highway_tile_462910(&state.city, 20, 20, NetworkAxis::GridY),
        2
    );
    assert_eq!(
        pe_get_highway_tile_462910(&state.city, 40, 40, NetworkAxis::GridY),
        2
    );
    assert_eq!(
        pe_get_highway_tile_462910(&state.city, 40, 40, NetworkAxis::GridX),
        3
    );
}

#[test]
fn highway_flat_stamp_plus_minus_2_neighbor_arms_like_4626d0() {
    // PE `0x4627bf`–`0x462870`: ±2 classify `>=2` → Get→Place; vacant skips.
    let mut city = City::new_blank("/tmp/sc2000-hwy-pm2.sc2", "HwyPm2").expect("blank");
    city.set_city_funds(50_000);
    // Existing same-axis segment at (18,20) with full corner bits.
    let neighbor = highway_segment_tiles((18, 20), NetworkAxis::GridX);
    for &(x, y) in &neighbor {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }
    let mut state = test_state(city);
    // Classify gate: crossover `0x4b` → 0/1 (skip); OOB → 0.
    state.city.set_building(30, 30, 0x4b);
    state.city.set_zone(30, 30, 0xf0);
    assert_eq!(pe_highway_classify_463c40(&state.city, 30, 30), 1);
    assert_eq!(pe_highway_classify_463c40(&state.city, -2, 20), 0);
    assert_eq!(pe_highway_classify_463c40(&state.city, 18, 20), 2);

    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    for &(x, y) in &neighbor {
        assert_eq!(
            state.city.building_at(x, y),
            0x4a,
            "same-axis neighbor kept"
        );
    }
    // Distant crossover classify-only tile untouched by place.
    assert_eq!(state.city.building_at(30, 30), 0x4b);
    let placed = highway_segment_tiles((20, 20), NetworkAxis::GridX);
    for &(x, y) in &placed {
        assert_eq!(state.city.building_at(x, y), 0x4a);
    }
}

#[test]
fn highway_flat_stamp_underlay_crossover_like_463b30() {
    // PE PlaceHighwayTile class`<4` underlay rewrite + `FUN_00463b30` table.
    // Exercise helpers directly: post-place `refresh_highway_segments_around`
    // still restamps via mask and would smash crossovers (separate leftover).
    let mut city = City::new_blank("/tmp/sc2000-hwy-underlay.sc2", "HwyUnder").expect("blank");
    let tiles = highway_segment_tiles((20, 20), NetworkAxis::GridX);
    city.set_building(20, 20, 0x1d);
    // Incoming class 2 rewritten to 3 by underlay scan → companions `0x4a`.
    pe_place_highway_tile_462ed0_class_lt4(&mut city, 20, 20, 2);
    assert_eq!(city.building_at(20, 20), 0x4c);
    for &(x, y) in &tiles {
        if (x, y) == (20, 20) {
            continue;
        }
        assert_eq!(city.building_at(x, y), 0x4a, "companion at ({x},{y})");
        assert_eq!(city.zone_at(x, y).map(|z| z.raw_bits()), Some(0xf0));
    }
    let cases = [
        (0x1e, 0x4b),
        (0x0e, 0x50),
        (0x0f, 0x4f),
        (0x2c, 0x4e),
        (0x2d, 0x4d),
    ];
    for (underlay, crossover) in cases {
        let mut c = City::new_blank("/tmp/sc2000-hwy-ul.sc2", "HwyUl").expect("blank");
        c.set_building(10, 10, underlay);
        pe_place_highway_tile_463b30(&mut c, 10, 10, 0);
        assert_eq!(c.building_at(10, 10), crossover, "underlay {underlay:#x}");
        assert_eq!(c.zone_at(10, 10).map(|z| z.raw_bits()), Some(0xf0));
    }
    let mut c = City::new_blank("/tmp/sc2000-hwy-tree.sc2", "HwyTree").expect("blank");
    c.set_building(10, 10, 0x10);
    pe_place_highway_tile_463b30(&mut c, 10, 10, 1);
    assert_eq!(c.building_at(10, 10), 0x4a);
}

#[test]
fn highway_get_tile_refuses_classify_0_1_4_7_and_ge_0x0d_like_462956() {
    // PE `0x462956`/`0x462966`/`0x46297e`: classify 0/1, 4..=7, or >=0x0d → -1.
    for classified in [0, 1, 4, 5, 6, 7, 0x0d, 0x10] {
        assert!(
            pe_get_highway_tile_classify_refuse_462956(classified),
            "classified {classified}"
        );
    }
    for classified in [-1, 2, 3, 8, 0x0c] {
        assert!(
            !pe_get_highway_tile_classify_refuse_462956(classified),
            "classified {classified}"
        );
    }
    let mut city = City::new_blank("/tmp/sc2000-hwy-get-refuse.sc2", "HwyGetRef").expect("blank");
    city.set_building(20, 20, 0x4b);
    city.set_zone(20, 20, 0xf0);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 1);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        -1
    );
    city.set_building(20, 20, 0x4c);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 0);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        -1
    );
    // Flat `0x4a` without corner nibble classifies `0x4a-0x5d` (`-19`), not 0, so
    // the early Get gate does not refuse. Empty mask follows `0x4c7d60`, so GridX
    // is class 3 and GridY is class 2 (`0x462c45`).
    city.set_building(20, 20, 0x4a);
    city.set_zone(20, 20, 0);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 0x4a - 0x5d);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        3
    );
    city.set_zone(20, 20, 0xf0);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        2
    );
    // Vacant classify is -1, so the early gate does not steal the axis class.
    assert_eq!(
        pe_get_highway_tile_462910(&city, 40, 40, NetworkAxis::GridX),
        3
    );
}

#[test]
fn highway_get_tile_north_classify_1_or_3_forces_class_2_like_4629f1() {
    // PE `0x4629be`/`0x4629c2` then `0x462d4f` table index 1 → class 2.
    let mut city = City::new_blank("/tmp/sc2000-hwy-north.sc2", "HwyNorth").expect("blank");
    city.set_building(20, 18, 0x4b);
    city.set_zone(20, 18, 0xf0);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 18), 1);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2
    );
    city.set_building(20, 18, 0);
    for &(x, y) in &[(20, 18), (21, 18), (20, 19), (21, 19)] {
        city.set_building(x, y, 0x49);
        city.set_zone(x, y, 0xf0);
    }
    assert_eq!(pe_highway_classify_463c40(&city, 20, 18), 3);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2
    );
    // y<=1 skips the north arm; drag axis still wins.
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 1, NetworkAxis::GridX),
        3
    );
}

#[test]
fn highway_get_tile_cardinal_classify_bits_like_462a25() {
    // East class 0/2 → bit 2 → table index 2 → class 3. South 1/3 → bit 4 → 2.
    // West 0/2 → bit 8 → 3. North+east → index 3 → class 8, not class`<4` Place.
    let mut city = City::new_blank("/tmp/sc2000-hwy-card.sc2", "HwyCard").expect("blank");
    city.set_building(22, 20, 0x4c);
    city.set_zone(22, 20, 0xf0);
    assert_eq!(pe_highway_classify_463c40(&city, 22, 20), 0);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        3
    );
    city.set_building(22, 20, 0);
    city.set_zone(22, 20, 0);
    city.set_building(20, 22, 0x4b);
    city.set_zone(20, 22, 0xf0);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2
    );
    city.set_building(20, 22, 0);
    city.set_zone(20, 22, 0);
    city.set_building(18, 20, 0x4c);
    city.set_zone(18, 20, 0xf0);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        3
    );
    city.set_building(18, 20, 0);
    city.set_zone(18, 20, 0);
    city.set_building(22, 20, 0x4c);
    city.set_zone(22, 20, 0xf0);
    city.set_building(20, 18, 0x4b);
    city.set_zone(20, 18, 0xf0);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        8
    );
    pe_city_tool_place_highway_stamp_body_4626d0(&mut city, 20, 20, NetworkAxis::GridX);
    assert_eq!(
        city.building_at(20, 20),
        0x65,
        "class 8 Place is class+0x5d"
    );
    assert_eq!(city.building_at(21, 20), 0x65);
    assert_eq!(city.building_at(22, 20), 0x4c);
    assert_eq!(city.building_at(20, 18), 0x4b);
}

#[test]
fn pe_highway_route_gate_low_word_keeps_pe_return_classes() {
    let mut city =
        City::new_blank("/tmp/sc2000-hwy-route-gate.sc2", "HwyRouteGate").expect("blank");

    assert_eq!(pe_highway_route_gate_low_word_464040(&city, 20, 20), 0x000f);

    city.set_terrain(20, 20, 1);
    assert_eq!(
        pe_highway_route_gate_low_word_464040(&city, 20, 20),
        0x0003,
        "the XTER slope-category arm returns its directional mask"
    );

    city.set_terrain(20, 20, 0x0d);
    assert_eq!(
        pe_highway_route_gate_low_word_464040(&city, 20, 20),
        0x4000,
        "the minimum-altitude XTER 0x0d arm returns the special gate value"
    );

    city.set_terrain(20, 20, 0);
    city.set_building(21, 20, 5);
    assert_eq!(
        pe_highway_route_gate_low_word_464040(&city, 20, 20),
        0x8000,
        "the early XBLD refusal returns the blocked gate value"
    );

    city.set_building(21, 20, 0);
    city.set_terrain(20, 20, 9);
    city.set_terrain(21, 20, 1);
    city.set_terrain(21, 21, 9);
    city.set_terrain(20, 21, 0);
    city.set_altitude(20, 20, 0, 0);
    city.set_altitude(21, 20, 0, 0);
    city.set_altitude(21, 21, 0, 0);
    city.set_altitude(20, 21, 1, 0);
    assert_eq!(
        pe_highway_route_gate_low_word_464040(&city, 20, 20),
        0x000f,
        "an active slope mask can also equal the ordinary-branch value 0x000f"
    );
}

#[test]
fn pe_highway_route_ordinary_branch_retries_alternate_after_blocked_primary() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-route-alt.sc2", "HwyRouteAlt").expect("blank");
    city.set_building(10, 12, 5);
    let mut direction = pe_pipe_direction_to_target_44c680((10, 10), (14, 14));
    assert_eq!(direction, 2, "equal deltas choose south as primary");
    assert_eq!(
        pe_highway_route_step_461ae0(&city, (10, 10), (14, 14), (10, 10, 14, 14), &mut direction,),
        PeHighwayRouteStep461ae0::Advanced((12, 10)),
        "blocked south candidate falls back east"
    );
    assert_eq!(direction, 1);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    let path = state.highway_drag_path_for_city((10, 10), (14, 14));
    assert_eq!(path.first(), Some(&(10, 10)));
    assert_eq!(path.get(1), Some(&(12, 10)));
    assert!(
        !path.contains(&(10, 12)),
        "the PE stateful route must not stamp the blocked primary footprint"
    );
    state.city.set_city_funds(50_000);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&path), path.len());
    assert_eq!(
        state.city.building_at(10, 12),
        5,
        "the rejected primary footprint remains untouched after commit"
    );
}

#[test]
fn pe_highway_terminal_flat_crossing_stops_preview_cost_and_commit_together() {
    // FUN_00461AE0 stops on a perpendicular flat-highway footprint; the caller
    // still stamps that terminal cursor through FUN_004626D0. Keep that rooted
    // terminal case on the PE route and stop preview/cost at the same segment.
    let mut city = City::new_blank("/tmp/sc2000-hwy-terminal-crossing.sc2", "HwyCrossing")
        .expect("blank city");
    city.set_city_funds(50_000);
    for origin_y in [18, 20, 22] {
        for &(x, y) in &highway_segment_tiles((22, origin_y), NetworkAxis::GridY) {
            city.set_building(x, y, 0x49);
            city.set_zone(x, y, 0xf0);
            city.set_traffic_density(x, y, 0x40 + (y * 2 + x) as u8);
        }
    }

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.tool = EditTool::Highway;
    let traffic_before = state
        .city
        .traffic_density_slice()
        .map(|values| values.to_vec());
    let plan = state.highway_drag_plan_for_city((20, 20), (30, 20));
    assert_eq!(plan.path, vec![(20, 20), (22, 20)]);
    assert_eq!(plan.terminal_direction, Some(1));

    let preview = state.highway_drag_placement_for_plan(&plan);
    assert!(preview.invalid_tiles.is_empty());
    assert_eq!(preview.valid_tiles.len(), 2 * HIGHWAY_TILES_PER_SEGMENT);
    assert!(!preview.valid_tiles.contains(&(24, 20)));

    state.highway_drag = Some(HighwayDrag {
        start: (20, 20),
        current: (30, 20),
    });
    assert_eq!(
        state.highway_drag_cost(),
        Some(2 * HIGHWAY_COST_PER_SEGMENT)
    );
    assert_eq!(state.place_highway_plan(&plan), 2);
    assert_eq!(state.city.city_funds(), 49_800);
    assert_eq!(state.city.building_at(24, 20), 0);
    // West plus the two existing north/south arms give Get mask 0x0d, class
    // 0x0c, and Place XBLD 0x69; this is not a hardcoded generic 4-way tile.
    for &(x, y) in &highway_segment_tiles((22, 20), NetworkAxis::GridX) {
        assert_eq!(state.city.building_at(x, y), 0x69);
    }
    assert_eq!(
        state
            .city
            .traffic_density_slice()
            .map(|values| values.to_vec()),
        traffic_before,
        "the crossing stamp preserves the established XTRF plane"
    );
}

#[test]
fn pe_highway_terminal_flat_t_crossings_resolve_all_four_orientations() {
    // The flat-terrain `0x4e8950` lookup maps all four three-arm masks
    // (0x7, 0xb, 0xd, 0xe) to class 0xc, which PlaceHighwayTile stamps as
    // XBLD 0x69. The existing 0xd case has a separate end-to-end regression;
    // cover its rotations and the complementary T shapes here.
    let cases = [
        // N/S trunk with a new west arm: mask 0xd.
        ((20, 20), (30, 20), (22, 20), NetworkAxis::GridY, true, 0x0d),
        // N/S trunk with a new east arm: mask 0x7.
        ((30, 20), (20, 20), (22, 20), NetworkAxis::GridY, true, 0x07),
        // E/W trunk with a new north arm: mask 0xb.
        (
            (20, 18),
            (20, 30),
            (20, 20),
            NetworkAxis::GridX,
            false,
            0x0b,
        ),
        // E/W trunk with a new south arm: mask 0xe.
        (
            (20, 30),
            (20, 20),
            (20, 20),
            NetworkAxis::GridX,
            false,
            0x0e,
        ),
    ];

    for (case_index, (start, end, junction, trunk_axis, vertical_trunk, expected_mask)) in
        cases.into_iter().enumerate()
    {
        let mut city = City::new_blank(
            format!("/tmp/sc2000-hwy-t-crossing-{case_index}.sc2"),
            "HwyTCrossing",
        )
        .expect("blank city");
        city.set_city_funds(50_000);
        let trunk_origins = match trunk_axis {
            NetworkAxis::GridY => [(22, 18), (22, 20), (22, 22)],
            NetworkAxis::GridX => [(18, 20), (20, 20), (22, 20)],
        };
        for origin in trunk_origins {
            for &(x, y) in &highway_segment_tiles(origin, trunk_axis) {
                city.set_building(x, y, if vertical_trunk { 0x49 } else { 0x4a });
                city.set_zone(x, y, 0xf0);
            }
        }

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.tool = EditTool::Highway;
        let plan = state.highway_drag_plan_for_city(start, end);
        assert_eq!(plan.path.last(), Some(&junction), "case {case_index}");
        assert!(state.highway_pe_route_path_has_simple_stamps(&plan));
        assert_eq!(
            state.place_highway_plan(&plan),
            plan.path.len(),
            "case {case_index}"
        );

        let neighbor_mask = [
            (0x01, junction.0 as isize, junction.1 as isize - 2, true),
            (0x02, junction.0 as isize + 2, junction.1 as isize, false),
            (0x04, junction.0 as isize, junction.1 as isize + 2, true),
            (0x08, junction.0 as isize - 2, junction.1 as isize, false),
        ]
        .into_iter()
        .fold(0, |mask, (bit, x, y, ns)| {
            if pe_highway_neighbor_low_bit_462990(&state.city, x, y, ns) {
                mask | bit
            } else {
                mask
            }
        });
        assert_eq!(neighbor_mask, expected_mask, "case {case_index}");
        assert_eq!(
            pe_get_highway_tile_462910(&state.city, junction.0, junction.1, trunk_axis),
            0x0c,
            "three-arm lookup mask {expected_mask:#04x}, case {case_index}"
        );
        for &(x, y) in &highway_segment_tiles(
            junction,
            match trunk_axis {
                NetworkAxis::GridY => NetworkAxis::GridX,
                NetworkAxis::GridX => NetworkAxis::GridY,
            },
        ) {
            assert_eq!(
                state.city.building_at(x, y),
                0x69,
                "T junction class 0xc at ({x},{y}), case {case_index}"
            );
        }
    }
}

#[test]
fn pe_highway_terminal_flat_four_way_crossing_resolves_mask_f() {
    // The Win95 `0x4e8950` lookup also maps all four cardinal arms (mask 0xf)
    // to class 0xc / XBLD 0x69. Keep this case separate from a T: the east arm
    // already exists before the new west approach reaches the crossing.
    let mut city =
        City::new_blank("/tmp/sc2000-hwy-four-way.sc2", "HwyFourWay").expect("blank city");
    city.set_city_funds(50_000);
    for origin_y in [18, 20, 22] {
        for &(x, y) in &highway_segment_tiles((22, origin_y), NetworkAxis::GridY) {
            city.set_building(x, y, 0x49);
            city.set_zone(x, y, 0xf0);
        }
    }
    for &(x, y) in &highway_segment_tiles((24, 20), NetworkAxis::GridX) {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.tool = EditTool::Highway;
    let plan = state.highway_drag_plan_for_city((20, 20), (30, 20));
    assert_eq!(plan.path, vec![(20, 20), (22, 20)]);
    assert_eq!(state.place_highway_plan(&plan), 2);

    let mask = [
        (0x01, 22isize, 18isize, true),
        (0x02, 24isize, 20isize, false),
        (0x04, 22isize, 22isize, true),
        (0x08, 20isize, 20isize, false),
    ]
    .into_iter()
    .fold(0, |mask, (bit, x, y, ns)| {
        if pe_highway_neighbor_low_bit_462990(&state.city, x, y, ns) {
            mask | bit
        } else {
            mask
        }
    });
    assert_eq!(mask, 0x0f);
    assert_eq!(
        pe_get_highway_tile_462910(&state.city, 22, 20, NetworkAxis::GridY),
        0x0c
    );
    for &(x, y) in &highway_segment_tiles((22, 20), NetworkAxis::GridX) {
        assert_eq!(state.city.building_at(x, y), 0x69);
    }
}

#[test]
fn pe_highway_get_tile_special_gate_selects_alternate_junction_table() {
    // `FUN_00462910` selects DAT_004e8970 for a special terrain gate after
    // collecting the same four cardinal neighbors. The ordinary flat table
    // gives mask 0xf class 0xc; the XTER 0x0d gate selects class 2 instead.
    let make_fixture = |special_terrain: bool| {
        let mut city = City::new_blank("/tmp/sc2000-hwy-alternate-mask-table.sc2", "HwyMaskTable")
            .expect("blank city");
        city.set_building(20, 20, 0x65);
        if special_terrain {
            for y in 20..=21 {
                for x in 20..=21 {
                    city.set_terrain(x, y, 0x0d);
                }
            }
        }
        for (origin, axis, building) in [
            ((20, 18), NetworkAxis::GridY, 0x49),
            ((22, 20), NetworkAxis::GridX, 0x4a),
            ((20, 22), NetworkAxis::GridY, 0x49),
            ((18, 20), NetworkAxis::GridX, 0x4a),
        ] {
            for &(x, y) in &highway_segment_tiles(origin, axis) {
                city.set_building(x, y, building);
                city.set_zone(x, y, 0xf0);
            }
        }
        city
    };

    let ordinary = make_fixture(false);
    assert_eq!(pe_highway_classify_463c40(&ordinary, 20, 20), 8);
    assert_eq!(
        pe_highway_route_gate_low_word_464040(&ordinary, 20, 20),
        0x000f
    );
    assert_eq!(
        pe_get_highway_tile_462910(&ordinary, 20, 20, NetworkAxis::GridY),
        0x0c
    );

    let special = make_fixture(true);
    assert_eq!(pe_highway_classify_463c40(&special, 20, 20), 8);
    assert_eq!(
        pe_highway_route_gate_low_word_464040(&special, 20, 20),
        0x4000
    );
    assert_eq!(
        pe_get_highway_tile_462910(&special, 20, 20, NetworkAxis::GridY),
        2,
        "the special gate uses DAT_004e8970[0xf], not the ordinary table"
    );
}

#[test]
fn pe_highway_route_nonordinary_gate_follows_saved_direction() {
    let mut city =
        City::new_blank("/tmp/sc2000-hwy-route-saved-dir.sc2", "HwyRouteDir").expect("blank");
    city.set_terrain(20, 20, 1);
    assert_eq!(pe_highway_route_gate_low_word_464040(&city, 20, 20), 0x0003);
    let mut direction = 1;
    assert_eq!(
        pe_highway_route_step_461ae0(&city, (20, 20), (24, 24), (20, 20, 24, 24), &mut direction,),
        PeHighwayRouteStep461ae0::Advanced((22, 20)),
        "non-0x0f gate follows saved east direction instead of primary south"
    );
    assert_eq!(direction, 1);
}

#[test]
fn pe_highway_route_late_xbld_reject_keeps_selected_direction() {
    let mut city =
        City::new_blank("/tmp/sc2000-hwy-route-late-xbld.sc2", "HwyRouteLate").expect("blank");
    city.set_building(20, 22, 5);
    city.set_building(22, 20, 0x2d);
    let mut direction = 2;
    assert_eq!(
        pe_highway_route_step_461ae0(&city, (20, 20), (24, 24), (20, 20, 24, 24), &mut direction,),
        PeHighwayRouteStep461ae0::LateXbldReject
    );
    assert_eq!(
        direction, 1,
        "the PE writes the alternate direction before the late XBLD refusal"
    );
}

#[test]
fn pe_highway_route_late_xbld_reject_stamps_terminal_origin_with_saved_axis() {
    let mut city =
        City::new_blank("/tmp/sc2000-hwy-route-late-plan.sc2", "HwyRouteLatePlan").expect("blank");
    city.set_building(20, 22, 5);
    city.set_building(22, 20, 0x2d);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    let plan = state.highway_drag_plan_for_city((20, 20), (24, 24));
    assert_eq!(plan.path, vec![(20, 20)]);
    assert_eq!(plan.terminal_direction, Some(1));
    assert_eq!(
        highway_axis_for_drag_plan_tile(EngineFidelity::Win95, &plan, 0),
        NetworkAxis::GridX,
        "the late refusal changes the terminal stamp to the newly saved east axis"
    );
    let preview = state.highway_drag_placement_for_plan(&plan);
    assert!(preview.invalid_tiles.is_empty());
    assert_eq!(
        preview.valid_tiles,
        highway_segment_tiles((20, 20), NetworkAxis::GridX)
    );

    state.highway_drag = Some(HighwayDrag {
        start: (20, 20),
        current: (24, 24),
    });
    assert_eq!(state.highway_drag_cost(), Some(HIGHWAY_COST_PER_SEGMENT));

    state.city.set_city_funds(50_000);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_plan(&plan), 1);
}

#[test]
fn pe_highway_route_records_neighbor_link_when_next_segment_leaves_map() {
    let city =
        City::new_blank("/tmp/sc2000-hwy-neighbor-edge.sc2", "HwyNeighborEdge").expect("blank");
    let plan =
        pe_highway_route_path_461ae0(&city, (122, 20), (126, 20)).expect("simple dry edge route");
    assert_eq!(plan.path, vec![(122, 20), (124, 20), (126, 20)]);
    assert_eq!(
        plan.neighbor_link,
        Some(HighwayNeighborLinkCandidate {
            start: (122, 20),
            endpoint: (126, 20),
        })
    );

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.tool = EditTool::Highway;
    state.city.set_city_funds(10_000);
    state.begin_highway_drag(122, 20);
    state.update_highway_drag_to((126, 20));
    let (placed, candidate) = state.finish_highway_drag();
    assert_eq!(placed, 3, "all three ordinary route segments are committed");
    assert_eq!(candidate, plan.neighbor_link);
    let funds_after_route = state.city.city_funds();
    assert!(!apply_highway_neighbor_link_confirmation(
        &mut state,
        candidate.expect("neighbor-link offer"),
        false,
    ));
    assert_eq!(state.city.city_funds(), funds_after_route);
    assert_eq!(state.city.pe_nonnetwork_neighbor_links, 0);
    assert!(is_highway_tile(state.city.building_at(126, 20)));
}

#[test]
fn pe_highway_route_offers_neighbor_link_for_no_progress_on_outer_edge() {
    let city = City::new_blank(
        "/tmp/sc2000-hwy-neighbor-no-progress.sc2",
        "HwyNeighborStart",
    )
    .expect("blank");
    let plan = pe_highway_route_path_461ae0(&city, (0, 20), (0, 20)).expect("edge singleton route");
    assert_eq!(plan.path, vec![(0, 20)]);
    assert_eq!(
        plan.neighbor_link,
        Some(HighwayNeighborLinkCandidate {
            start: (0, 20),
            endpoint: (0, 20),
        })
    );
}

#[test]
fn confirmed_highway_neighbor_link_charges_marks_and_increments_only_on_yes() {
    let mut city =
        City::new_blank("/tmp/sc2000-hwy-neighbor-confirm.sc2", "HwyNeighbor").expect("blank");
    city.set_city_funds(4_000);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    let candidate = HighwayNeighborLinkCandidate {
        start: (124, 20),
        endpoint: (126, 20),
    };

    assert!(!apply_highway_neighbor_link_confirmation(
        &mut state, candidate, false
    ));
    assert_eq!(state.city.city_funds(), 4_000);
    assert_eq!(state.city.text_overlay_at(126, 20).unwrap_or(0), 0);
    assert_eq!(state.city.pe_nonnetwork_neighbor_links, 0);

    assert!(apply_highway_neighbor_link_confirmation(
        &mut state, candidate, true
    ));
    assert_eq!(state.city.city_funds(), 2_500);
    assert_eq!(state.city.text_overlay_at(126, 20), Some(0xfa));
    assert_eq!(state.city.pe_nonnetwork_neighbor_links, 1);

    state.city.set_city_funds(1_499);
    let no_funds = HighwayNeighborLinkCandidate {
        start: (124, 22),
        endpoint: (126, 22),
    };
    assert!(!highway_neighbor_link_is_available(&state, no_funds));
    state.city.set_city_funds(2_000);
    state.city.set_text_overlay(124, 22, 0xfa);
    assert!(!highway_neighbor_link_is_available(&state, no_funds));
}

#[test]
fn win95_road_no_progress_prompts_only_at_outer_edge() {
    let mut city = City::new_blank(
        "/tmp/sc2000-road-neighbor-no-progress.sc2",
        "RoadNeighborNoProgress",
    )
    .expect("blank city");
    city.set_city_funds(10_000);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.tool = EditTool::Road;
    state.begin_network_drag(0, 20, NetworkKind::Road);
    let edge_outcome = state.release_left_mouse_drag(false);
    assert_eq!(edge_outcome.network, Some((NetworkKind::Road, 1)));
    assert_eq!(
        edge_outcome
            .road_neighbor_link
            .map(|candidate| candidate.endpoint),
        Some((0, 20))
    );

    let mut interior_city = City::new_blank(
        "/tmp/sc2000-road-neighbor-no-progress-interior.sc2",
        "RoadNeighborInterior",
    )
    .expect("blank city");
    interior_city.set_city_funds(10_000);
    let mut interior = test_state(interior_city);
    interior.engine_fidelity = EngineFidelity::Win95;
    interior.tool = EditTool::Road;
    interior.begin_network_drag(20, 20, NetworkKind::Road);
    let interior_outcome = interior.release_left_mouse_drag(false);
    assert_eq!(interior_outcome.road_neighbor_link, None);
}

#[test]
fn win95_rail_no_progress_edge_click_offers_and_applies_neighbor_link() {
    let mut city = City::new_blank("/tmp/sc2000-rail-neighbor-confirm.sc2", "RailNeighbor")
        .expect("blank city");
    city.set_city_funds(5_000);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.tool = EditTool::Rail;

    state.begin_network_drag(0, 20, NetworkKind::Rail);
    let outcome = state.release_left_mouse_drag(false);
    assert_eq!(outcome.network, Some((NetworkKind::Rail, 1)));
    let candidate = outcome
        .rail_neighbor_link
        .expect("no-progress edge click offers neighbor link");
    assert_eq!(candidate.endpoint, (0, 20));
    assert_eq!(state.city.building_at(0, 20), 0x2c);

    let funds_after_rail = state.city.city_funds();
    let traffic_before = state.city.traffic_density_slice().map(|v| v.to_vec());
    assert!(!apply_rail_neighbor_link_confirmation(
        &mut state, candidate, false
    ));
    assert_eq!(state.city.city_funds(), funds_after_rail);
    assert_eq!(state.city.text_overlay_at(0, 20).unwrap_or(0), 0);
    assert_eq!(state.city.pe_nonnetwork_neighbor_links, 0);

    assert!(apply_rail_neighbor_link_confirmation(
        &mut state, candidate, true
    ));
    assert_eq!(state.city.city_funds(), funds_after_rail - 1_500);
    assert_eq!(state.city.text_overlay_at(0, 20), Some(0xfa));
    assert_eq!(state.city.building_at(0, 20), 0x2d);
    assert_eq!(state.city.pe_nonnetwork_neighbor_links, 1);
    assert_eq!(
        state.city.traffic_density_slice().map(|v| v.to_vec()),
        traffic_before,
        "accepted-link updates do not alter the established XTRF overlay"
    );

    let not_available = RailNeighborLinkCandidate { endpoint: (0, 22) };
    state.city.set_city_funds(1_499);
    assert!(!rail_neighbor_link_is_available(&state, not_available));
    state.city.set_city_funds(2_000);
    state.city.set_text_overlay(0, 22, 0xfa);
    assert!(!rail_neighbor_link_is_available(&state, not_available));
}

#[test]
fn win95_rail_progressed_route_prompts_only_when_terminal_lookahead_exits_map() {
    let mut city =
        City::new_blank("/tmp/sc2000-rail-route-edge.sc2", "RailRouteEdge").expect("blank city");
    city.set_city_funds(5_000);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.tool = EditTool::Rail;
    let traffic_before = state
        .city
        .traffic_density_slice()
        .map(|values| values.to_vec());

    state.begin_network_drag(125, 30, NetworkKind::Rail);
    state.update_network_drag_to((127, 30));
    let outcome = state.release_left_mouse_drag(false);
    assert_eq!(outcome.network, Some((NetworkKind::Rail, 3)));
    let candidate = outcome
        .rail_neighbor_link
        .expect("eastward final cursor looks one cell off-map");
    assert_eq!(candidate.endpoint, (127, 30));
    assert_eq!(
        state
            .city
            .traffic_density_slice()
            .map(|values| values.to_vec()),
        traffic_before,
        "Rail route replay preserves the established XTRF overlay"
    );
    assert!(
        rail_neighbor_link_is_available(&state, candidate),
        "candidate {:?}, fidelity {:?}, funds {}, overlay {:?}",
        candidate,
        state.engine_fidelity,
        state.city.city_funds(),
        state.city.text_overlay_at(127, 30),
    );
    let funds_after_rail = state.city.city_funds();
    assert!(!apply_rail_neighbor_link_confirmation(
        &mut state, candidate, false
    ));
    assert_eq!(state.city.city_funds(), funds_after_rail);
    assert_eq!(state.city.text_overlay_at(127, 30), Some(0));
    assert!(apply_rail_neighbor_link_confirmation(
        &mut state, candidate, true
    ));
    assert_eq!(state.city.city_funds(), funds_after_rail - 1_500);
    assert_eq!(state.city.text_overlay_at(127, 30), Some(0xfa));
    assert_eq!(state.city.pe_nonnetwork_neighbor_links, 1);
    assert_eq!(
        state
            .city
            .traffic_density_slice()
            .map(|values| values.to_vec()),
        traffic_before,
        "neighbor confirmation preserves the established XTRF overlay"
    );

    let mut inward_city = City::new_blank("/tmp/sc2000-rail-route-inward.sc2", "RailRouteInward")
        .expect("blank city");
    inward_city.set_city_funds(5_000);
    let mut inward = test_state(inward_city);
    inward.engine_fidelity = EngineFidelity::Win95;
    inward.tool = EditTool::Rail;
    inward.begin_network_drag(127, 30, NetworkKind::Rail);
    inward.update_network_drag_to((125, 30));
    let inward_outcome = inward.release_left_mouse_drag(false);
    assert_eq!(inward_outcome.network, Some((NetworkKind::Rail, 3)));
    assert_eq!(
        inward_outcome.rail_neighbor_link, None,
        "starting at the edge does not prompt when the terminal direction points inward"
    );
}

#[test]
fn highway_stamp_refuses_when_gate_8000_like_4626eb() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-st8.sc2", "HwySt8").expect("blank");
    city.set_building(20, 20, 5);
    city.set_zone(20, 20, 0x01);
    assert!(pe_highway_gate_early_8000_4640a1(&city, 20, 20));
    pe_city_tool_place_highway_stamp_body_4626d0(&mut city, 20, 20, NetworkAxis::GridY);
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(
        city.zone_at(20, 20).map(|z| z.raw_bits()).unwrap_or(0),
        0x01
    );
}

#[test]
fn highway_get_tile_ns_class2_and_ew_class3_connect_like_4629d8() {
    // NS class 2 still sets bit 0 unless `FUN_00464040` returns `0x4000`
    // (`0x4629d8`). EW class 3 sets bit 2 the same way (`0x462a5a`).
    // The shipped gate is `0xf`, not `0x4000`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-c23.sc2", "HwyC23").expect("blank");
    for &(x, y) in &[(20, 18), (21, 18), (20, 19), (21, 19)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }
    assert_eq!(pe_highway_classify_463c40(&city, 20, 18), 2);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2
    );
    for &(x, y) in &[(20, 18), (21, 18), (20, 19), (21, 19)] {
        city.set_building(x, y, 0);
        city.set_zone(x, y, 0);
    }
    for &(x, y) in &[(22, 20), (23, 20), (22, 21), (23, 21)] {
        city.set_building(x, y, 0x49);
        city.set_zone(x, y, 0xf0);
    }
    assert_eq!(pe_highway_classify_463c40(&city, 22, 20), 3);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        3
    );
}

#[test]
fn highway_get_tile_gate_8000_uses_4e8970_like_4640c1() {
    // Building `5` in the footprint returns gate `0x8000`. Mask 3 is class 3
    // on `0x4e8970`, not class 8 on `0x4e8950`. Empty mask is class 2.
    let mut city = City::new_blank("/tmp/sc2000-hwy-g8.sc2", "HwyG8").expect("blank");
    city.set_building(21, 20, 5);
    city.set_building(20, 18, 0x4b);
    city.set_zone(20, 18, 0xf0);
    city.set_building(22, 20, 0x4c);
    city.set_zone(22, 20, 0xf0);
    assert!(pe_highway_gate_early_8000_4640a1(&city, 20, 20));
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        3
    );
    city.set_building(20, 18, 0);
    city.set_building(22, 20, 0);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2
    );
}

#[test]
fn highway_get_tile_class2_gate_4000_returns_minus1_like_462c28() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-g4.sc2", "HwyG4").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
        city.set_terrain(x, y, 0x0d);
    }
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 2);
    assert!(pe_highway_gate_returns_4000_4642e7(&city, 20, 20));
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        -1
    );
}

#[test]
fn highway_get_tile_skips_class2_north_and_class3_east_when_gate_4000_like_4629d8() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-n4k.sc2", "HwyN4k").expect("blank");
    for &(x, y) in &[(20, 18), (21, 18), (20, 19), (21, 19)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
        city.set_terrain(x, y, 0x0d);
    }
    assert_eq!(pe_highway_classify_463c40(&city, 20, 18), 2);
    assert!(pe_highway_gate_returns_4000_4642e7(&city, 20, 18));
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        3,
        "north class 2 with gate 0x4000 must not set bit 1"
    );
    city.set_terrain(20, 18, 0);
    city.set_terrain(21, 18, 0);
    city.set_terrain(20, 19, 0);
    city.set_terrain(21, 19, 0);
    assert!(!pe_highway_gate_returns_4000_4642e7(&city, 20, 18));
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2,
        "north class 2 still connects when the gate is not 0x4000"
    );

    let mut east = City::new_blank("/tmp/sc2000-hwy-e4k.sc2", "HwyE4k").expect("blank");
    for &(x, y) in &[(22, 20), (23, 20), (22, 21), (23, 21)] {
        east.set_building(x, y, 0x49);
        east.set_zone(x, y, 0xf0);
        east.set_terrain(x, y, 0x0d);
    }
    assert_eq!(pe_highway_classify_463c40(&east, 22, 20), 3);
    assert!(pe_highway_gate_returns_4000_4642e7(&east, 22, 20));
    assert_eq!(
        pe_get_highway_tile_462910(&east, 20, 20, NetworkAxis::GridY),
        2,
        "east class 3 with gate 0x4000 must not set bit 2"
    );
}

#[test]
fn highway_get_tile_equal_alt_sw_cat1_gate_mask_returns_7_like_464310() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-si0.sc2", "HwySi0").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }
    city.set_terrain(20, 21, 1);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 2);
    assert_eq!(
        pe_highway_gate_si0_fallthrough_464310(&city, 20, 20),
        Some(9)
    );
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        7
    );
}

#[test]
fn highway_get_tile_low_bit_plus_si0_gate_returns_5_like_462cfe() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-lo.sc2", "HwyLo").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }
    city.set_building(20, 18, 0x4b);
    city.set_zone(20, 18, 0xf0);
    city.set_altitude(20, 18, 9, 0);
    for &(x, y) in &[(22, 20), (23, 20), (22, 21), (23, 21)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
        city.set_altitude(x, y, 9, 0);
    }
    city.set_terrain(20, 20, 1);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 18), 1);
    assert_eq!(pe_highway_classify_463c40(&city, 22, 20), 2);
    assert_eq!(
        pe_highway_gate_si0_fallthrough_464310(&city, 20, 20),
        Some(3)
    );
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        5
    );
}

#[test]
fn highway_get_tile_unequal_alt_si_gate_mask_returns_4_like_464362() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-si.sc2", "HwySi").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }
    city.set_terrain(21, 20, 1);
    city.set_terrain(21, 21, 1);
    city.set_altitude(20, 21, 9, 0);
    assert_eq!(
        pe_highway_gate_si0_fallthrough_464310(&city, 20, 20),
        Some(1)
    );
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        4
    );
}

#[test]
fn highway_get_tile_class8_gate_4000_empty_mask_returns_2_like_462d62() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-c8g4.sc2", "HwyC8").expect("blank");
    city.set_building(20, 20, 0x65);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 8);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        3
    );
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_terrain(x, y, 0x0d);
    }
    assert!(pe_highway_gate_returns_4000_4642e7(&city, 20, 20));
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2
    );
}

#[test]
fn highway_get_tile_cx7_cat3_gate_bit_returns_6_like_4643db() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-cx7.sc2", "HwyCx7").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }
    city.set_terrain(20, 20, 9);
    city.set_terrain(21, 20, 5);
    city.set_terrain(21, 21, 5);
    city.set_altitude(20, 21, 9, 0);
    assert_eq!(
        pe_highway_gate_si0_fallthrough_464310(&city, 20, 20),
        Some(4)
    );
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        6
    );
}

#[test]
fn highway_get_tile_cx_0xb_cat3_gate_bit_returns_4_like_4643f3() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-cxb.sc2", "HwyCxb").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }
    city.set_terrain(20, 20, 5);
    city.set_terrain(21, 20, 9);
    city.set_terrain(20, 21, 5);
    city.set_altitude(21, 21, 9, 0);
    assert_eq!(
        pe_highway_gate_si0_fallthrough_464310(&city, 20, 20),
        Some(1)
    );
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        4
    );
}

#[test]
fn highway_get_tile_cx_0xd_and_0xe_cat3_gate_bits_like_46440b() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-cxd.sc2", "HwyCxd").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }
    city.set_terrain(20, 20, 5);
    city.set_terrain(21, 21, 9);
    city.set_terrain(20, 21, 5);
    city.set_altitude(21, 20, 9, 0);
    assert_eq!(
        pe_highway_gate_si0_fallthrough_464310(&city, 20, 20),
        Some(1)
    );
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        4
    );

    let mut east = City::new_blank("/tmp/sc2000-hwy-cxe.sc2", "HwyCxe").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        east.set_building(x, y, 0x4a);
        east.set_zone(x, y, 0xf0);
    }
    east.set_terrain(21, 20, 9);
    east.set_terrain(21, 21, 5);
    east.set_terrain(20, 21, 5);
    east.set_altitude(20, 20, 9, 0);
    assert_eq!(
        pe_highway_gate_si0_fallthrough_464310(&east, 20, 20),
        Some(8)
    );
    assert_eq!(
        pe_get_highway_tile_462910(&east, 20, 20, NetworkAxis::GridX),
        7
    );
}

#[test]
fn highway_get_tile_neighbor_table_ge4_like_4e89d8() {
    // NS table index 4 is 0 (no bit); index 5 is 1. EW index 4 is 1.
    let mut city = City::new_blank("/tmp/sc2000-hwy-ntab.sc2", "HwyNtab").expect("blank");
    city.set_building(20, 18, 0x61);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 18), 4);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        3,
        "NS class 4 does not connect"
    );
    city.set_building(20, 18, 0x62);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 18), 5);
    // Equal altitude and class 5 ORs `0x40` → class 7, not the low table.
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        7
    );
    city.set_building(20, 18, 0x65);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 18), 8);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2
    );
    city.set_building(20, 18, 0);
    city.set_building(22, 20, 0x61);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        3,
        "EW class 4 connects"
    );
}

#[test]
fn highway_classify_origin_xbit_bit4_returns_0x0d_or_0x0e_like_463cdc() {
    // PE `FUN_00463c40` @ `0x463cdc`–`0x463d10`: XBIT bit 0x04 on the origin
    // after the crossover arm. `0x49` → `0x0d`; anything else in range → `0x0e`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-xbit4.sc2", "HwyXbit4").expect("blank");
    city.set_zone(20, 20, 0xf0);
    city.set_building(20, 20, 0x49);
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0x04));
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 0x0d);
    city.set_building(20, 20, 0x4a);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 0x0e);
    city.set_building(20, 20, 0x61);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 0x0e);
    // Crossover arm wins before the water-bit test.
    city.set_building(20, 20, 0x4b);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 1);
    // No bit and vacant companions: PE fallthrough class comes from SW `(x, y+1)`,
    // which is empty here, so classify is `-1` rather than origin class 3.
    city.set_building(20, 20, 0x4a);
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0));
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), -1);
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0x04));
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        -1
    );
}

#[test]
fn highway_classify_footprint_companions_like_463d12() {
    // PE `0x463d12`–`0x463e55`: NE/SE water or crossover, then SW `0x49`/`0x4a` class.
    let mut city = City::new_blank("/tmp/sc2000-hwy-cls-fp.sc2", "HwyClsFp").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (21, 21), (20, 21)] {
        city.set_zone(x, y, 0xf0);
        city.set_building(x, y, 0x49);
    }
    // PE `0x463e49`: class is `2 + (SW & 1)`, so `0x49` → 3 and `0x4a` → 2.
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 3);
    city.set_building(20, 21, 0x4a);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 2);
    city.set_building(21, 20, 0x4c);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 0);
    city.set_building(21, 20, 0x49);
    city.set_tile_bits(21, 21, crate::city::TileBits::new(0x04));
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 0x0d);
    city.set_tile_bits(21, 21, crate::city::TileBits::new(0));
    city.set_building(21, 21, 0x10);
    city.set_tile_bits(21, 21, crate::city::TileBits::new(0x04));
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 0x0e);
}

#[test]
fn highway_classify_missing_corners_returns_xbld_minus_0x5d_like_463e5f() {
    // PE `0x463e5f`: corners not `0xf0` and `xbld-0x5d < 0x0d` → that signed delta.
    let mut city = City::new_blank("/tmp/sc2000-hwy-nocorner.sc2", "HwyNoCrn").expect("blank");
    city.set_building(12, 12, 0x49);
    assert_eq!(pe_highway_classify_463c40(&city, 12, 12), 0x49 - 0x5d);
    city.set_building(12, 12, 0x61);
    assert_eq!(pe_highway_classify_463c40(&city, 12, 12), 0x61 - 0x5d);
    city.set_building(12, 12, 0x69);
    assert_eq!(pe_highway_classify_463c40(&city, 12, 12), 0x69 - 0x5d);
    // `0x6a`/`0x6b` take the neighbor XBIT bit `0x02` arm (`0x463e69`).
    city.set_building(12, 12, 0x6a);
    assert_eq!(pe_highway_classify_463c40(&city, 12, 12), 0x0f);
    city.set_tile_bits(12, 13, crate::city::TileBits::new(0x02));
    assert_eq!(pe_highway_classify_463c40(&city, 12, 12), 0x10);
    city.set_building(12, 12, 0x6b);
    assert_eq!(pe_highway_classify_463c40(&city, 12, 12), 0x10);
    // Full corners do not take this arm.
    city.set_building(12, 12, 0x49);
    city.set_zone(12, 12, 0xf0);
    city.set_building(12, 13, 0x49);
    assert_eq!(pe_highway_classify_463c40(&city, 12, 12), 3);
}

#[test]
fn highway_bridge_extension_alternates_against_existing_neighbor() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..=11 {
        for x in 10..=13 {
            city.set_terrain(x, y, 0x10);
        }
    }
    let mut state = test_state(city);
    let existing_tiles = highway_segment_tiles((10, 10), NetworkAxis::GridX);
    state.stamp_flat_highway_tiles(106, &existing_tiles);

    assert_eq!(
        state.place_highway_path(&highway_drag_path((12, 10), (12, 10))),
        1
    );

    for y in 10..=11 {
        for x in 10..=11 {
            assert_eq!(state.city.building_at(x, y), 106);
        }
        for x in 12..=13 {
            assert_eq!(state.city.building_at(x, y), 107);
        }
    }
}

#[test]
fn highway_drag_preview_uses_highway_footprint_and_cost() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.begin_highway_drag(10, 10);
    state.update_highway_drag_to((20, 10));

    let path = highway_drag_path((10, 10), (20, 10));
    let placement = state.highway_drag_placement_for_path(&path);

    assert_eq!(placement.preview, ToolPreview::Highway);
    assert!(!ToolPreview::Highway.draws_valid_diamond());
    assert_eq!(placement.valid_tiles.len(), 6 * HIGHWAY_TILES_PER_SEGMENT);
    assert_eq!(state.highway_drag_cost(), Some(600));
}

#[test]
fn perpendicular_highway_drag_through_existing_highway_builds_interchange() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(
        state.place_highway_path(&highway_drag_path((10, 10), (14, 10))),
        3
    );
    let drag_path = highway_drag_path((12, 8), (12, 12));
    let preview = state.highway_drag_placement_for_path(&drag_path);
    assert!(!preview.valid_tiles.contains(&(12, 12)));
    assert_eq!(preview.valid_tiles.len(), 2 * HIGHWAY_TILES_PER_SEGMENT);

    assert_eq!(state.place_highway_path(&drag_path), 2);

    for y in 10..=11 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                105,
                "interchange at ({x},{y})"
            );
            assert_ne!(
                state.city.zone_at(x, y).unwrap().raw_bits(),
                0,
                "interchange corner bits at ({x},{y})"
            );
        }
    }
    for y in 8..=9 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                73,
                "vertical flat approach at ({x},{y})"
            );
        }
    }
    for y in 12..=13 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                0,
                "drag-through stops before beyond-intersection segment at ({x},{y})"
            );
        }
    }
}

#[test]
fn highway_elbow_stamps_class_0xb_not_four_way_105() {
    // NS ending at (12,10) plus a west approach is Get mask 9 → `0x4e8950`
    // class `0xb` → PlaceTile `0x68` (104), not invent 105.
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert_eq!(
        state.place_highway_path(&highway_drag_path((12, 8), (12, 10))),
        2
    );
    assert_eq!(
        state.place_highway_path(&highway_drag_path((10, 10), (12, 10))),
        2
    );
    for y in 10..=11 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                104,
                "elbow class 0xb at ({x},{y})"
            );
            assert_ne!(
                state.city.zone_at(x, y).unwrap().raw_bits() & 0xf0,
                0,
                "elbow corners at ({x},{y})"
            );
        }
    }
    for y in 10..=11 {
        for x in 10..=11 {
            assert_eq!(
                state.city.building_at(x, y),
                74,
                "west approach at ({x},{y})"
            );
        }
    }
    for y in 8..=9 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                73,
                "north approach at ({x},{y})"
            );
        }
    }
    for y in 10..=11 {
        for x in 14..=15 {
            assert_eq!(
                state.city.building_at(x, y),
                0,
                "drag stops at the elbow ({x},{y})"
            );
        }
    }
}

#[test]
fn highway_elbow_other_corners_follow_4e8950() {
    // Mask 3/6/12 are `0x4e8950` classes 8/9/`0xa` → buildings 101/102/103.
    let cases = [
        ((12, 8), (12, 10), (14, 10), (12, 10), 101u8),
        ((12, 10), (12, 12), (14, 10), (12, 10), 102),
        ((12, 10), (12, 12), (10, 10), (12, 10), 103),
    ];
    for (ns_start, ns_end, ew_start, ew_end, building) in cases {
        let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        let mut state = test_state(city);
        assert_eq!(
            state.place_highway_path(&highway_drag_path(ns_start, ns_end)),
            2,
            "ns approach for {building}"
        );
        assert_eq!(
            state.place_highway_path(&highway_drag_path(ew_start, ew_end)),
            2,
            "ew approach for {building}"
        );
        for y in 10..=11 {
            for x in 12..=13 {
                assert_eq!(
                    state.city.building_at(x, y),
                    building,
                    "elbow {building} at ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn highway_interchange_snaps_to_existing_two_tile_band() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(
        state.place_highway_path(&highway_drag_path((10, 10), (14, 10))),
        3
    );
    let drag_path = highway_drag_path((12, 9), (12, 13));
    let preview = state.highway_drag_placement_for_path(&drag_path);
    assert_eq!(drag_path, vec![(12, 8), (12, 10), (12, 12)]);
    assert!(!preview.valid_tiles.contains(&(12, 12)));
    assert_eq!(preview.valid_tiles.len(), 2 * HIGHWAY_TILES_PER_SEGMENT);
    assert_eq!(state.place_highway_path(&drag_path), 2);

    for y in 10..=11 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                105,
                "offset interchange at ({x},{y})"
            );
        }
    }
    for y in 8..=9 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                73,
                "odd cursor snaps to canonical flat approach at ({x},{y})"
            );
        }
    }
    for y in 12..=13 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                0,
                "drag-through stops before beyond-intersection segment at ({x},{y})"
            );
        }
    }
}

#[test]
fn placing_network_path_commits_dragged_straight_and_curve() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    let path = network_drag_path((10, 10), (13, 10));

    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 4);
    for x in 10..=13 {
        assert_eq!(state.city.building_at(x, 10), 30);
    }

    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    let path = network_drag_path((20, 20), (22, 21));

    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 4);
    assert_eq!(state.city.building_at(20, 20), 30);
    assert_eq!(state.city.building_at(21, 20), 37);
    assert_eq!(state.city.building_at(21, 21), 35);
    assert_eq!(state.city.building_at(22, 21), 30);
}

#[test]
fn power_line_drag_builds_only_the_valid_prefix_until_blocked() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // PlacePowerLines admission ignores water terrain; a non-network building blocks it.
    city.set_building(12, 10, 0x70);
    let mut state = test_state(city);
    let path = network_drag_path_for_kind((10, 10), (14, 10), NetworkKind::Power);

    let placement = state.network_drag_placement_for_path(&path, NetworkKind::Power);
    assert_eq!(placement.valid_tiles, vec![(10, 10), (11, 10)]);
    assert_eq!(placement.invalid_tiles, vec![(12, 10)]);

    assert_eq!(state.place_network_path(&path, NetworkKind::Power), 2);
    assert_eq!(state.city.building_at(10, 10), 0x0f);
    assert_eq!(state.city.building_at(11, 10), 0x0f);
    assert_eq!(state.city.building_at(12, 10), 0x70);
    for x in 13..=14 {
        assert_eq!(state.city.building_at(x, 10), 0);
    }
}

#[test]
fn power_line_drag_cost_uses_the_buildable_prefix() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(13, 10, 0x70);
    let mut state = test_state(city);

    state.begin_network_drag(10, 10, NetworkKind::Power);
    state.update_network_drag_to((15, 10));

    assert_eq!(state.network_drag_cost(), Some((NetworkKind::Power, 6)));
}

#[test]
fn road_and_rail_drag_costs_use_original_per_tile_prices() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.begin_network_drag(10, 10, NetworkKind::Road);
    state.update_network_drag_to((13, 10));
    assert_eq!(state.network_drag_cost(), Some((NetworkKind::Road, 40)));
    state.refresh_status_line();
    assert_eq!(state.query_line, "Road $40");

    state.cancel_network_drag();
    state.begin_network_drag(20, 20, NetworkKind::Rail);
    state.update_network_drag_to((23, 20));
    assert_eq!(state.network_drag_cost(), Some((NetworkKind::Rail, 100)));
    state.refresh_status_line();
    assert_eq!(state.query_line, "Rail $100");
}

#[test]
fn city_tool_behavior_matrix_matches_runtime_routes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    enable_all_conditional_palette_tools(&mut city);
    let mut state = test_state(city);

    for &tool in ALL_EDIT_TOOLS {
        let behavior = expected_city_tool_behavior(tool);
        state.tool = tool;

        assert!(
            !tool.label().is_empty(),
            "{tool:?} has no user-facing label"
        );
        assert_eq!(
            tool_routes_to_city_action(tool),
            !matches!(behavior, CityToolBehavior::EditMapOnly),
            "{tool:?} city action route does not match its behavior class"
        );

        match behavior {
            CityToolBehavior::Query => {
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::Query),
                    "{tool:?}"
                );
                assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
            }
            CityToolBehavior::ChooserDialog => {
                assert!(is_chooser_menu_tool(tool), "{tool:?}");
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::ChooserDialog),
                    "{tool:?}"
                );
                assert_eq!(placeable_building_for_tool(tool), None, "{tool:?}");
                assert_eq!(tool_build_cost(tool), None, "{tool:?}");
                assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
            }
            CityToolBehavior::EditMapOnly => {
                assert_eq!(test_mouse_input_route_for_tool(tool), None, "{tool:?}");
                assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
            }
            CityToolBehavior::Network(kind) => {
                assert_eq!(network_kind_for_tool(tool), Some(kind), "{tool:?}");
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::Drag),
                    "{tool:?}"
                );
                assert!(state.tool_placement_at(20, 20).is_some(), "{tool:?}");
            }
            CityToolBehavior::Underground(kind) => {
                assert_eq!(underground_kind_for_tool(tool), Some(kind), "{tool:?}");
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::Drag),
                    "{tool:?}"
                );
                assert!(state.tool_placement_at(20, 20).is_some(), "{tool:?}");
            }
            CityToolBehavior::Zone(zone_id) => {
                assert_eq!(zone_id_for_tool(tool), Some(zone_id), "{tool:?}");
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::Drag),
                    "{tool:?}"
                );
                assert!(state.tool_placement_at(20, 20).is_some(), "{tool:?}");
            }
            CityToolBehavior::DirectBuilding(building) => {
                assert_eq!(
                    placeable_building_for_tool(tool),
                    Some(building),
                    "{tool:?}"
                );
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::DirectClick),
                    "{tool:?}"
                );
                assert!(state.tool_placement_at(20, 20).is_some(), "{tool:?}");
            }
            CityToolBehavior::HydroPower => {
                assert_eq!(placeable_building_for_tool(tool), Some(198), "{tool:?}");
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::DirectClick),
                    "{tool:?}"
                );
                assert!(state.tool_placement_at(20, 20).is_some(), "{tool:?}");
            }
            CityToolBehavior::Dispatch(kind) => {
                assert_eq!(dispatch_kind_for_tool(tool), Some(kind), "{tool:?}");
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::DirectClick),
                    "{tool:?}"
                );
                assert!(state.tool_placement_at(20, 20).is_some(), "{tool:?}");
            }
            CityToolBehavior::TerrainEdit
            | CityToolBehavior::SurfaceWater
            | CityToolBehavior::Trees
            | CityToolBehavior::Highway => {
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::Drag),
                    "{tool:?}"
                );
                assert!(state.tool_placement_at(20, 20).is_some(), "{tool:?}");
            }
            CityToolBehavior::Bulldoze
            | CityToolBehavior::Sign
            | CityToolBehavior::Tunnel
            | CityToolBehavior::Onramp
            | CityToolBehavior::SubToRail
            | CityToolBehavior::Forest => {
                assert_eq!(
                    test_mouse_input_route_for_tool(tool),
                    Some(TestMouseInputRoute::DirectClick),
                    "{tool:?}"
                );
                assert!(state.tool_placement_at(20, 20).is_some(), "{tool:?}");
            }
        }
    }
}

#[test]
fn drag_placement_sounds_keep_wires_and_pipes_distinct() {
    assert_eq!(
        network_placement_sound(NetworkKind::Power),
        PlacementSound::PowerLine
    );
    assert_eq!(
        network_placement_sound(NetworkKind::Road),
        PlacementSound::Build
    );
    assert_eq!(
        network_placement_sound(NetworkKind::Rail),
        PlacementSound::Build
    );
    assert_eq!(
        underground_placement_sound(UndergroundKind::Pipes),
        PlacementSound::Pipe
    );
    assert_eq!(
        underground_placement_sound(UndergroundKind::Subway),
        PlacementSound::Build
    );
}

#[test]
fn one_click_placement_sounds_follow_recovered_tool_groups() {
    for (tool, effect) in [
        (EditTool::PowerPlantCoal, PlacementSound::Build),
        (EditTool::PowerPlantHydro, PlacementSound::Build),
        (EditTool::WaterPump, PlacementSound::Build),
        (EditTool::Onramp, PlacementSound::Build),
        (EditTool::SubToRail, PlacementSound::Build),
        (EditTool::Tunnel, PlacementSound::Build),
        (EditTool::BusDepot, PlacementSound::Horns),
        (EditTool::RailDepot, PlacementSound::Train),
        (EditTool::Police, PlacementSound::Police),
        (EditTool::FireStation, PlacementSound::FireTruck),
        (EditTool::Hospital, PlacementSound::Police),
        (EditTool::Prison, PlacementSound::Prison),
        (EditTool::School, PlacementSound::School),
        (EditTool::Library, PlacementSound::School),
        (EditTool::SmallPark, PlacementSound::Cheers),
        (EditTool::Zoo, PlacementSound::Monster),
        (EditTool::MayorHouse, PlacementSound::Cheers),
        (EditTool::ArcologyLaunch, PlacementSound::Cheers),
        (EditTool::DispatchPolice, PlacementSound::Police),
        (EditTool::DispatchFirefighters, PlacementSound::FireTruck),
        (EditTool::DispatchMilitary, PlacementSound::Military),
    ] {
        assert_eq!(one_click_placement_sound(tool), Some(effect), "{tool:?}");
    }

    for tool in [
        EditTool::Road,
        EditTool::Rail,
        EditTool::Power,
        EditTool::Pipes,
        EditTool::Subway,
        EditTool::Bulldoze,
        EditTool::Query,
    ] {
        assert_eq!(one_click_placement_sound(tool), None);
    }
}

#[test]
fn zoning_and_nature_sounds_follow_recovered_tool_groups() {
    for tool in [
        EditTool::ZoneResidential,
        EditTool::ZoneResidentialDense,
        EditTool::ZoneCommercial,
        EditTool::ZoneCommercialDense,
        EditTool::ZoneIndustrial,
        EditTool::ZoneIndustrialDense,
    ] {
        assert_eq!(zone_placement_sound(tool), PlacementSound::Plop, "{tool:?}");
    }

    assert_eq!(
        zone_placement_sound(EditTool::Airport),
        PlacementSound::Build
    );
    assert_eq!(
        zone_placement_sound(EditTool::Seaport),
        PlacementSound::Build
    );
    assert_eq!(
        zone_placement_sound(EditTool::DeZone),
        PlacementSound::Build
    );
}

#[test]
fn bulldoze_clears_whole_bridge_span_like_opensc2k() {
    // Preserve the current non-Win95 OpenSC2K bridge approximation. This test
    // does not claim original Mac bulldozer parity. Selector 0/state 0 is now
    // known to reach a map-removal worker, but its complete bridge and tail
    // behavior has not been compared with this fallback.
    let mut city = City::new_blank("/tmp/sc2000-bridge-span.sc2", "BridgeSpan").expect("blank");
    for x in 11..=13 {
        city.set_terrain(x, 10, 0x10);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let path = vec![(10, 10), (11, 10), (12, 10), (13, 10), (14, 10)];
    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 5);
    assert!(is_bridge_tile(state.city.building_at(11, 10)));
    assert!(is_bridge_tile(state.city.building_at(12, 10)));
    assert!(is_bridge_tile(state.city.building_at(13, 10)));
    let bank_land_before = [
        state.city.altitude_at(10, 10) & 0x1f,
        state.city.altitude_at(14, 10) & 0x1f,
    ];

    state.bulldoze_at(12, 10);

    assert_eq!(state.city.building_at(11, 10), 0);
    assert_eq!(state.city.building_at(12, 10), 0);
    assert_eq!(state.city.building_at(13, 10), 0);
    // OpenSC2K bank retile: both dry tips clear, land--, keep water-tile.
    assert_eq!(state.city.building_at(10, 10), 0);
    assert_eq!(state.city.building_at(14, 10), 0);
    assert!(
        state
            .city
            .tile_bits_at(10, 10)
            .is_some_and(TileBits::water_tile)
    );
    assert!(
        state
            .city
            .tile_bits_at(14, 10)
            .is_some_and(TileBits::water_tile)
    );
    assert_eq!(
        state.city.altitude_at(10, 10) & 0x1f,
        bank_land_before[0].saturating_sub(1)
    );
    assert_eq!(
        state.city.altitude_at(14, 10) & 0x1f,
        bank_land_before[1].saturating_sub(1)
    );
}

#[test]
fn blocked_or_incompatible_road_drag_cancels_without_partial_placement() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(11, 10, 112);
    let mut state = test_state(city);
    let path = network_drag_path((10, 10), (13, 10));

    let placement = state.network_placement_for_path(&path, NetworkKind::Road);
    assert_eq!(placement.invalid_tiles, vec![(11, 10)]);
    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 0);
    for x in 10..=13 {
        let expected = if x == 11 { 112 } else { 0 };
        assert_eq!(state.city.building_at(x, 10), expected);
    }

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(11, 10, 0x01);
    let mut state = test_state(city);
    let path = network_drag_path((10, 10), (12, 10));

    let placement = state.network_placement_for_path(&path, NetworkKind::Road);
    assert_eq!(placement.invalid_tiles, vec![(11, 10)]);
    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 0);
    for x in 10..=12 {
        assert_eq!(state.city.building_at(x, 10), 0);
    }
}

#[test]
fn canceling_network_drag_leaves_city_unchanged() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.begin_network_drag(10, 10, NetworkKind::Road);
    state.update_network_drag_to((12, 10));
    state.cancel_network_drag();
    state.finish_network_drag();

    for x in 10..=12 {
        assert_eq!(state.city.building_at(x, 10), 0);
    }
}

#[test]
fn cancel_active_drag_also_covers_zone_and_nature_drags() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    state.begin_zone_drag(10, 10, ZONE_INDUSTRIAL_LIGHT);
    state.update_zone_drag_to((12, 10));
    assert!(state.cancel_active_drag());
    assert_eq!(state.finish_zone_drag(), 0);
    for x in 10..=12 {
        assert_eq!(state.city.zone_at(x, 10).unwrap().zone_id(), 0);
    }

    state.begin_nature_drag(20, 20, EditTool::Trees);
    assert!(
        matches!(state.city.building_at(20, 20), 6 | 7),
        "PE MapToolPlaceTree stamps CRT 6|7"
    );
    assert!(state.cancel_active_drag());
    assert_eq!(state.update_nature_drag_to((22, 20)), 0);
    assert_eq!(state.finish_nature_drag(), None);
    assert_eq!(state.city.building_at(21, 20), 0);
    assert_eq!(state.city.building_at(22, 20), 0);

    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::StretchTerrain;
    assert!(state.begin_edit_map_stretch_drag_at(30, 30, 100.0));
    assert!(state.cancel_active_drag());
    assert_eq!(state.finish_edit_map_stretch_drag(), 0);

    assert!(!state.cancel_active_drag());
}

#[test]
fn shift_release_cancels_zone_drag_before_it_places() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.begin_zone_drag(10, 10, ZONE_INDUSTRIAL_LIGHT);
    state.update_zone_drag_to((12, 10));

    let outcome = state.release_left_mouse_drag(true);

    assert_eq!(outcome, DragReleaseOutcome::default());
    assert_eq!(state.finish_zone_drag(), 0);
    for x in 10..=12 {
        assert_eq!(state.city.zone_at(x, 10).unwrap().zone_id(), 0);
    }
}

#[test]
fn zone_tools_use_sc2k_zone_ids_and_stamp_rectangles() {
    assert_eq!(zone_id_for_tool(EditTool::ZoneResidential), Some(1));
    assert_eq!(zone_id_for_tool(EditTool::ZoneResidentialDense), Some(2));
    assert_eq!(zone_id_for_tool(EditTool::ZoneCommercial), Some(3));
    assert_eq!(zone_id_for_tool(EditTool::ZoneCommercialDense), Some(4));
    assert_eq!(zone_id_for_tool(EditTool::ZoneIndustrial), Some(5));
    assert_eq!(zone_id_for_tool(EditTool::ZoneIndustrialDense), Some(6));
    assert_eq!(zone_id_for_tool(EditTool::Airport), Some(8));
    assert_eq!(zone_id_for_tool(EditTool::Seaport), Some(9));
    assert_eq!(zone_id_for_tool(EditTool::DeZone), Some(0));

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.tool = EditTool::ZoneCommercial;
    assert_eq!(state.place_zone_rect((40, 40), (42, 41), 3), 6);

    for y in 40..=41 {
        for x in 40..=42 {
            assert_eq!(
                state.city.zone_at(x, y).unwrap().raw_bits(),
                3 | ZONE_CORNER_ALL,
                "zone at ({x},{y})"
            );
        }
    }
}

#[test]
fn pe_447490_destroys_xbld_ge_0x0d_then_mode0() {
    // PE `FUN_00447020` @ `0x447085`: DestroyBuilding then slope/sea on cleared tile.
    let mut city = City::new_blank("/tmp/sc2000-447490-destroy.sc2", "D447490").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_building(20, 20, 0x0d);
    pe_terrain_neighbor_fun_00447020_447490(&mut city, None, 20, 20);
    assert!(city.building_at(20, 20) < 0x0d);
}

#[test]
fn pe_447490_elevated_destroy_building_arm() {
    // PE @ `0x447091`: DestroyBuilding mode 1 — elevated `0x3f..=0x42` arm, not
    // DestroyStructure early-out.
    let mut city = City::new_blank("/tmp/sc2000-447490-elev.sc2", "D447490El").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_building(20, 20, 0x3f);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    city.set_tunnel_levels(20, 20, 0x1f);
    city.set_text_overlay(20, 20, 0x55);
    pe_terrain_neighbor_fun_00447020_447490(&mut city, None, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(
        city.zone_at(20, 20).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0
    );
    assert_eq!(city.tunnel_levels_at(20, 20), 0);
    // Elevated arm never reaches XTXT clear @ `0x45a0c4`.
    assert_eq!(city.text_overlay_at(20, 20), Some(0x55));
}

#[test]
fn pe_447490_destroy_with_crt_places_rubble() {
    // `FUN_00447020` reaches DestroyBuilding(1). For this size-1 generic
    // selector, its two `0x459DEB`/`0x459E19` render draws precede the
    // conditional `0x45A023` rubble draw.
    let mut city = City::new_blank("/tmp/sc2000-447490-crt.sc2", "D447490Crt").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_building(20, 20, 0x70);
    let mut crt = Win95CrtRand::new(1);
    let mut preview = crt;
    for _ in 0..crate::city::pe_destroy_building_mode1_render_scatter_draw_count(0x70) {
        let _ = preview.random_u15();
    }
    let expected = preview.rubble_tile_id();
    pe_terrain_neighbor_fun_00447020_447490(&mut city, Some(&mut crt), 20, 20);
    assert_eq!(city.building_at(20, 20), expected);
}

#[test]
fn pe_destroy_building_special_runway_components_use_connected_worklist_and_native_rng() {
    let mut runway =
        City::new_blank("/tmp/sc2000-destroy-runway-stack.sc2", "RunwayStack").expect("blank city");
    let runway_cells = [(20, 20, 0xdd), (20, 21, 0xde), (20, 22, 0xdd)];
    for (index, &(x, y, building)) in runway_cells.iter().enumerate() {
        runway.set_building(x, y, building);
        runway.set_terrain(x, y, 0x11 + index as u8);
        runway.set_zone(x, y, 0xa0 | (index as u8 + 1));
        runway.set_tile_bits(x, y, TileBits::new(0xff));
        runway.set_text_overlay(x, y, 0x33);
    }
    // The PE DD/DE worklist is cardinal and never enters a different special
    // family, even when those tiles touch the component.
    runway.set_building(21, 23, 0xdd);
    runway.set_building(20, 23, 0xdf);

    let seed = 0x45dd_de01;
    let mut actual = Win95CrtRand::new(seed);
    let mut expected = actual;
    let mut rubble = Vec::new();
    for _ in runway_cells {
        rubble.push(expected.rubble_tile_id());
        let _ = expected.random_u15();
        let _ = expected.random_u15();
    }

    pe_destroy_building_mode1_optional_crt(&mut runway, Some(&mut actual), 20, 20);
    assert_eq!(
        actual, expected,
        "three DD/DE records consume three draws each"
    );
    for (index, &(x, y, _)) in runway_cells.iter().enumerate() {
        assert_eq!(runway.building_at(x, y), rubble[index]);
        assert_eq!(
            runway.zone_at(x, y).map(|zone| zone.raw_bits()),
            Some(1 + index as u8)
        );
        assert_eq!(
            runway.tile_bits_at(x, y).map(TileBits::raw_bits),
            Some(0x3d)
        );
        assert_eq!(runway.text_overlay_at(x, y), Some(0x33));
    }
    assert_eq!(
        runway.building_at(21, 23),
        0xdd,
        "diagonal runway is not connected"
    );
    assert_eq!(
        runway.building_at(20, 23),
        0xdf,
        "DF/E0 is a separate family"
    );

    // The native stack does not keep a visited set. In a 2x2 loop, the PE can
    // queue a cell that is already pending, so that coordinate is processed
    // twice and consumes another complete three-draw DD/DE record.
    let mut loop_city =
        City::new_blank("/tmp/sc2000-destroy-runway-loop.sc2", "RunwayLoop").expect("blank city");
    for (x, y, building) in [
        (50, 50, 0xdd),
        (51, 50, 0xde),
        (50, 51, 0xde),
        (51, 51, 0xdd),
    ] {
        loop_city.set_building(x, y, building);
    }
    let mut actual = Win95CrtRand::new(seed);
    let mut expected = actual;
    for _ in 0..5 * 3 {
        let _ = expected.random_u15();
    }
    pe_destroy_building_mode1_optional_crt(&mut loop_city, Some(&mut actual), 50, 50);
    assert_eq!(
        actual, expected,
        "duplicate pending cell consumes a second record"
    );
    for (x, y) in [(50, 50), (51, 50), (50, 51), (51, 51)] {
        assert!(matches!(loop_city.building_at(x, y), 1..=4));
    }

    let mut crossing = City::new_blank("/tmp/sc2000-destroy-runway-cross-stack.sc2", "CrossStack")
        .expect("blank city");
    let cross_cells = [(30, 30, 0xdf), (30, 31, 0xe0), (30, 32, 0xdf)];
    for (index, &(x, y, building)) in cross_cells.iter().enumerate() {
        crossing.set_building(x, y, building);
        crossing.set_zone(x, y, 0xb0 | (index as u8 + 3));
        crossing.set_tile_bits(x, y, TileBits::new(0xff));
        crossing.set_text_overlay(x, y, 0x33);
    }
    crossing.set_building(30, 33, 0xdd);
    let mut actual = Win95CrtRand::new(seed);
    let mut expected = actual;
    for _ in 0..cross_cells.len() * 2 {
        let _ = expected.random_u15();
    }
    pe_destroy_building_mode1_optional_crt(&mut crossing, Some(&mut actual), 30, 30);
    assert_eq!(
        actual, expected,
        "three DF/E0 records consume two draws each"
    );
    for (index, &(x, y, _)) in cross_cells.iter().enumerate() {
        assert_eq!(crossing.building_at(x, y), 0);
        assert_eq!(
            crossing.zone_at(x, y).map(|zone| zone.raw_bits()),
            Some(3 + index as u8)
        );
        assert_eq!(
            crossing.tile_bits_at(x, y).map(TileBits::raw_bits),
            Some(0x3d)
        );
        assert_eq!(crossing.text_overlay_at(x, y), Some(0x33));
    }
    assert_eq!(
        crossing.building_at(30, 33),
        0xdd,
        "DF/E0 does not cross into DD/DE"
    );

    let mut mode0 =
        City::new_blank("/tmp/sc2000-destroy-runway-mode0.sc2", "RunwayMode0").expect("blank city");
    mode0.set_building(40, 40, 0xde);
    mode0.set_terrain(40, 40, 0x11);
    let mut actual = Win95CrtRand::new(seed);
    let mut expected = actual;
    let rubble = expected.rubble_tile_id();
    pe_destroy_building_mode0_optional_crt(&mut mode0, Some(&mut actual), 40, 40);
    assert_eq!(
        actual, expected,
        "DD/DE mode 0 still spends its unconditional rubble draw"
    );
    assert_eq!(mode0.building_at(40, 40), rubble);
}

#[test]
fn pe_destroy_building_mode1_selector2_scatter_uses_snapped_nw_highway_gate() {
    let mut ordinary =
        City::new_blank("/tmp/sc2000-destroy-scatter-sel2.sc2", "ScatterSel2").expect("blank city");
    for y in 40..42 {
        for x in 40..42 {
            ordinary.set_building(x, y, 0x8c); // ordinary table-sized 2×2
            ordinary.set_terrain(x, y, 1); // suppress later rubble draws
        }
    }
    assert_eq!(pe_highway_classify_463c40(&ordinary, 40, 41), -1);
    assert_eq!(
        pe_destroy_building_mode1_scatter_draw_count_for_hit(&ordinary, 0x8c, 41, 41),
        8
    );
    let mut expected = Win95CrtRand::new(0x45b1_c201);
    for _ in 0..8 {
        let _ = expected.random_u15();
    }
    let mut crt = Win95CrtRand::new(0x45b1_c201);
    pe_destroy_building_mode1_optional_crt(&mut ordinary, Some(&mut crt), 41, 41);
    assert_eq!(crt, expected, "selector-2 generic route spends eight draws");
    for y in 40..42 {
        for x in 40..42 {
            assert_eq!(ordinary.building_at(x, y), 0);
        }
    }

    let mut network = City::new_blank("/tmp/sc2000-destroy-scatter-network-gate.sc2", "ScatterNet")
        .expect("blank city");
    for y in 40..42 {
        for x in 40..42 {
            network.set_building(x, y, 0x49);
            network.set_terrain(x, y, 1);
        }
    }
    // PE decrements the second snapped coordinate at `0x458e43`, so the
    // classifier examines the snapped NW `(x,y-1)` window, not `(x-1,y)`.
    network.set_zone(40, 40, 0xf0);
    network.set_tile_bits(40, 40, TileBits::new(0x04));
    assert_eq!(pe_highway_classify_463c40(&network, 40, 40), 0x0d);
    assert_eq!(pe_highway_classify_463c40(&network, 39, 41), -1);
    assert_eq!(
        pe_destroy_building_mode1_scatter_draw_count_for_hit(&network, 0x49, 41, 41),
        0,
        "high-class selector-2 route bypasses generic scatter; network CRT remains separate"
    );
    assert!(
        pe_destroy_building_selector2_low_class_cleanup_order(&network, 0x49, 41, 41).is_none(),
        "high-class selector-2 belongs to the separate dynamic walker"
    );
}

#[test]
fn pe_destroy_building_selector2_highclass_walk_follows_classifier_only() {
    let mut city =
        City::new_blank("/tmp/sc2000-destroy-sel2-high-walk.sc2", "Sel2Walk").expect("blank city");
    // Selector 2 follows classifier>=0x0d in both directions; unlike
    // selector 1, neither walk requires a bridge/support XBLD ID. The earlier
    // 0x49 block exercises the backscan, the clicked 0x49 block exercises the
    // first forward pass, and 0x6a exercises the next forward pass.
    for y in 38..42 {
        for x in 40..42 {
            city.set_building(x, y, 0x49);
            city.set_zone(x, y, 0xf0);
            city.set_terrain(x, y, 1);
        }
    }
    city.set_tile_bits(40, 38, TileBits::new(0x04));
    city.set_tile_bits(40, 40, TileBits::new(0x04));

    // The first forward block is the 0x6a member of the PE bridge/support band
    // and has a high classifier. Its XBIT bit 0x02 must clear while metadata
    // outside the exact masks and the generic XTXT/XTRF planes remain intact.
    for y in 42..44 {
        for x in 40..42 {
            city.set_building(x, y, 0x6a);
            city.set_zone(x, y, 0xf3);
            city.set_tile_bits(x, y, TileBits::new(0x02));
            city.set_terrain(x, y, 1);
        }
    }
    city.set_tile_bits(40, 42, TileBits::new(0x06));
    city.set_text_overlay(41, 43, 0x28);
    city.set_traffic_density(40, 42, 73);
    city.pe_dw_tile_count[0x49] = 8;
    city.pe_dw_tile_count[0x6a] = 4;
    let blank_count = city.pe_dw_tile_count[0];

    assert_eq!(pe_highway_classify_463c40(&city, 40, 38), 0x0d);
    assert_eq!(pe_highway_classify_463c40(&city, 40, 40), 0x0d);
    assert_eq!(pe_highway_classify_463c40(&city, 40, 42), 0x0e);
    assert_eq!(
        pe_destroy_building_selector2_high_class_walk_plan(&city, 0x49, 41, 41),
        Some(vec![
            [(40, 38), (40, 39), (41, 39), (41, 38)],
            [(40, 40), (40, 41), (41, 41), (41, 40)],
            [(40, 42), (40, 43), (41, 43), (41, 42)],
        ]),
        "classifier-high 0x49 blocks participate in the backscan and forward walk"
    );

    let seed = 0x4592_a049;
    let mut mode0_city = city.clone();
    let mut mode0_crt = Win95CrtRand::new(seed);
    pe_destroy_building_mode0_optional_crt(&mut mode0_city, Some(&mut mode0_crt), 41, 41);
    assert_eq!(mode0_crt, Win95CrtRand::new(seed), "mode 0 consumes no CRT");
    for y in 38..44 {
        for x in 40..42 {
            assert_eq!(mode0_city.building_at(x, y), 0);
        }
    }

    let mut expected_crt = Win95CrtRand::new(seed);
    for _ in 0..15 {
        let _ = expected_crt.random_u15();
    }
    let mut crt = Win95CrtRand::new(seed);
    pe_destroy_building_mode1_optional_crt(&mut city, Some(&mut crt), 41, 41);

    assert_eq!(
        crt, expected_crt,
        "each admitted pass consumes five CRT draws"
    );
    for y in 38..42 {
        for x in 40..42 {
            assert_eq!(city.building_at(x, y), 0, "admitted 0x49 block is cleared");
        }
    }
    for (x, y) in [(40, 42), (40, 43), (41, 43), (41, 42)] {
        assert_eq!(city.building_at(x, y), 0, "classifier-high cell is cleared");
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0x03);
    }
    assert_eq!(city.tile_bits_at(40, 42).unwrap().raw_bits(), 0x04);
    for (x, y) in [(40, 43), (41, 43), (41, 42)] {
        assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits(), 0);
    }
    assert_eq!(city.text_overlay_at(41, 43), Some(0x28));
    for y in 42..44 {
        for x in 40..42 {
            assert_eq!(city.traffic_density_at(x, y), Some(73));
        }
    }
    assert_eq!(city.pe_dw_tile_count[0x49], 0);
    assert_eq!(city.pe_dw_tile_count[0x6a], 0);
    assert_eq!(city.pe_dw_tile_count[0], blank_count.wrapping_add(12));
}

#[test]
fn pe_destroy_building_selector2_lowclass_clears_fixed_square_and_preserves_crt_order() {
    let mut city =
        City::new_blank("/tmp/sc2000-destroy-sel2-incomplete.sc2", "Sel2Gap").expect("blank city");
    // Leave the southeast tile of the snapped 2×2 highway footprint empty.
    // The generic selector-2 route still runs its fixed size×size cleanup,
    // independently of whether all cells repeat the original XBLD byte.
    let cleanup_order = [(40, 41), (40, 40), (41, 41), (41, 40)];
    for &(x, y) in &cleanup_order[..3] {
        city.set_building(x, y, 0x49);
    }
    assert_eq!(
        pe_destroy_building_selector2_low_class_cleanup_order(&city, 0x49, 40, 40),
        Some(cleanup_order.to_vec())
    );

    let seed = 0x45a1_7a49;
    let mut expected_crt = Win95CrtRand::new(seed);
    for _ in 0..8 {
        let _ = expected_crt.random_u15();
    }
    let expected_rubble: Vec<_> = cleanup_order
        .iter()
        .map(|&(x, y)| ((x, y), expected_crt.rubble_tile_id()))
        .collect();

    let mut crt = Win95CrtRand::new(seed);
    pe_destroy_building_mode1_optional_crt(&mut city, Some(&mut crt), 40, 40);

    assert_eq!(
        crt, expected_crt,
        "eight scatter draws precede x-major rubble"
    );
    for &((x, y), expected_tile) in &expected_rubble {
        assert_eq!(
            city.building_at(x, y),
            expected_tile,
            "fixed selector-2 cleanup writes ({x},{y}) in native order"
        );
    }
}

#[test]
fn pe_spread_nesw_442a30_with_crt_destroys_via_447490() {
    // PE tool-4 NESW `0x442bbd`→`0x447490`: process CRT rubble when XBLD `>= 0x0d`.
    let mut city = City::new_blank("/tmp/sc2000-spread-crt.sc2", "SpreadCrt").expect("blank");
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_altitude(x, y, 0x10, 0);
        }
    }
    city.set_building(20, 19, 0x70);
    let mut crt = Win95CrtRand::new(1);
    let mut preview = crt;
    let expected = expected_single_tile_mode1_rubble_after_generic_scatter(&mut preview);
    let _ = pe_terrain_nesw_land_copy_442a30_with_crt(&mut city, Some(&mut crt), 20, 20);
    assert_eq!(city.building_at(20, 19), expected);
}

#[test]
fn pe_spread_tool4_sets_ui_timer_4cad4c() {
    // PE terrain tool-4 @ `0x442a88`: `[0x4cad4c] = 0xfffffc18` (-1000).
    assert_eq!(PE_SPREAD_TOOL4_UI_TIMER_INIT, -1000);
    assert_eq!(PE_SPREAD_TOOL4_UI_TIMER_INIT as u32, 0xfffffc18);
    let mut city = City::new_blank("/tmp/sc2000-spread-timer.sc2", "SpreadTimer").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    state.pe_spread_tool4_ui_timer = 123;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.pe_spread_tool4_ui_timer, -1000);
    assert_eq!(state.pe_raise_ui_tool_id, 4);
}

#[test]
fn pe_spread_tool4_entry_zeros_ui_bytes_442a9b() {
    // PE tool-4 @ `0x442a9b`–`0x442ad1`: entry zeros UI +1/+5/+6/+7/+8/+9.
    let mut city = City::new_blank("/tmp/sc2000-spread-ui0.sc2", "SpreadUi0").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    state.pe_raise_ui_press_y_byte = 9;
    state.pe_raise_xter_ui_byte = 9;
    state.pe_raise_ui_byte6 = 9;
    state.pe_raise_ui_byte7 = 9;
    state.pe_raise_ui_byte8 = 9;
    state.pe_raise_ui_byte9 = 9;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(
        state.pe_raise_ui_press_y_byte, state.pe_raise_ui_tile_y,
        "tool-4 entry zeros UI +1, then the click path writes current UI Y"
    );
    assert_eq!(state.pe_raise_xter_ui_byte, 0);
    assert_eq!(state.pe_raise_ui_byte6, 0);
    assert_eq!(state.pe_raise_ui_byte7, 0);
    assert_eq!(state.pe_raise_ui_byte8, 0);
    assert_eq!(state.pe_raise_ui_byte9, 0);
}

#[test]
fn pe_spread_tool4_same_tile_skips_nesw_copy_442b31() {
    // PE tool-4 @ `0x442b31`: `0x445da0` → `0xffff` skips NESW land copy.
    let mut city = City::new_blank("/tmp/sc2000-spread-same.sc2", "SpreadSame").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_altitude(20, 20, 12, 0);
    assert_eq!(
        pe_terrain_nesw_land_copy_442a30_if_moved(&mut city, 20, 20, 20, 20),
        (0, 0)
    );
    assert_eq!(city.altitude_at(21, 20), 9);
    // Moved onto (20,20): copy center land 12 onto NESW neighbors.
    city.set_text_overlay(19, 20, 0x55);
    let (n447, _) = pe_terrain_nesw_land_copy_442a30_if_moved(&mut city, 20, 20, 19, 20);
    assert_eq!(n447, 4);
    assert_eq!(city.altitude_at(21, 20), 12);
    assert_eq!(city.text_overlay_at(19, 20), Some(0));
    assert_eq!(city.text_overlay_at(20, 20), Some(0xc9));
    // Packed path: `0x1490` → `(0x10,0x14)`; same prev skips.
    assert_eq!(
        pe_spread_terrain_from_packed_442a30(&mut city, 0x1490, 0x10, 0x14),
        (0, 0)
    );
    let (n447, _) = pe_spread_terrain_from_packed_442a30(&mut city, 0x1490, 0x0f, 0x14);
    assert_eq!(n447, 4);
    assert_eq!(city.text_overlay_at(0x10, 0x14), Some(0xc9));
}

#[test]
fn pe_spread_tool4_stamps_xtxt_0xc9_on_current() {
    // PE tool-4 @ `0x442c33`: after NESW copy, current tile XTXT = `0xc9`.
    let mut city = City::new_blank("/tmp/sc2000-spread-xtxt.sc2", "SpreadXtxt").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xc9));
    assert_eq!(state.pe_raise_ui_tile_x, 20);
    assert_eq!(state.pe_raise_ui_tile_y, 20);
}

#[test]
fn pe_spread_tool4_sets_ui_tile_xy_442c4c() {
    // PE tool-4 @ `0x442c4c`/`0x442c57`: UI `+3`/`+4` = current tile X/Y.
    let mut city = City::new_blank("/tmp/sc2000-spread-xy.sc2", "SpreadXy").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    assert!(state.apply_tool_at_tile(21, 19));
    assert_eq!(state.pe_raise_ui_tile_x, 21);
    assert_eq!(state.pe_raise_ui_tile_y, 19);
    assert_eq!(state.pe_raise_xter_ui_byte, 0);
}

#[test]
fn pe_spread_tool4_clears_ui_byte5_after_stamp_442c5f() {
    // PE tool-4 @ `0x442c5f`: after tile XY write, UI `+5` = 0.
    let mut city = City::new_blank("/tmp/sc2000-spread-b5.sc2", "SpreadB5").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    state.pe_raise_xter_ui_byte = 0x55;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.pe_raise_xter_ui_byte, 0);
    assert_eq!(state.pe_raise_ui_press_y_byte, 20);
}

#[test]
fn pe_spread_tool4_sets_ui_byte1_to_current_y_442c44() {
    // PE tool-4 @ `0x442c44`: UI `+1` = current tile Y.
    let mut city = City::new_blank("/tmp/sc2000-spread-b1.sc2", "SpreadB1").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    assert!(state.apply_tool_at_tile(20, 22));
    assert_eq!(state.pe_raise_ui_press_y_byte, 22);
    assert_eq!(state.pe_raise_ui_tile_y, 22);
}

#[test]
fn pe_spread_tool4_stores_prev_tile_words_442c7e() {
    // PE tool-4 @ `0x442c7e`/`0x442c83`: after stamp, `[esp+0x14]`/`[esp+0x16]` = current XY.
    let mut city = City::new_blank("/tmp/sc2000-spread-prev.sc2", "SpreadPrev").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_altitude(20, 20, 12, 0);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.pe_spread_prev_tile_x, 20);
    assert_eq!(state.pe_spread_prev_tile_y, 20);
    let (applied, _) = state.paint_edit_map_tool_drag_to(Some((20, 20)), (21, 20), false);
    assert_eq!(applied, 1);
    assert_eq!(state.pe_spread_prev_tile_x, 21);
    assert_eq!(state.pe_spread_prev_tile_y, 20);
}

#[test]
fn pe_spread_tool4_hold_refreshes_ui_on_move_442c44() {
    // PE tool-4 hold @ `0x442c44`/`0x442c4c`/`0x442c57`: UI tracks current tile after move.
    let mut city =
        City::new_blank("/tmp/sc2000-spread-hold-ui.sc2", "SpreadHoldUi").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_altitude(20, 20, 12, 0);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.pe_spread_hold_cursor_id, 0x1fc);
    assert_eq!(state.pe_spread_hold_cursor_arg, -1);
    assert_eq!(state.pe_spread_ui_cursor_4519e0_count, 2);
    assert_eq!(state.pe_spread_ui_cursor_4519e0_last_arg, 0);
    // Hold UI waits (2) + NESW `0x445b50` adjusts + four `0x447490` ×18.
    assert!(state.pe_spread_ui_wait_4012c1_count >= 2 + 4 * 18);
    assert_eq!(state.pe_raise_ui_tile_x, 20);
    assert_eq!(state.pe_raise_ui_tile_y, 20);
    let (applied, _) = state.paint_edit_map_tool_drag_to(Some((20, 20)), (21, 22), false);
    assert_eq!(applied, 1);
    assert_eq!(state.pe_raise_ui_tile_x, 21);
    assert_eq!(state.pe_raise_ui_tile_y, 22);
    assert_eq!(state.pe_raise_ui_press_y_byte, 22);
    assert_eq!(state.pe_raise_xter_ui_byte, 0);
    assert!(state.pe_spread_hold_ui_refresh_count >= 1);
    let before = state.pe_spread_hold_ui_refresh_count;
    let before_cursor = state.pe_spread_ui_cursor_4519e0_count;
    let before_wait = state.pe_spread_ui_wait_4012c1_count;
    assert_eq!(state.city.text_overlay_at(21, 22), Some(0xc9));
    state.finish_edit_map_spread_hold();
    // PE Spread exit @ `0x442cd1`: clear XTXT on prev-tile words; @ `0x442cf0`: UI.
    assert_eq!(state.city.text_overlay_at(21, 22), Some(0));
    assert_eq!(state.pe_spread_hold_ui_refresh_count, before + 1);
    assert_eq!(state.pe_spread_ui_cursor_4519e0_count, before_cursor + 1);
    assert_eq!(state.pe_spread_ui_cursor_4519e0_last_arg, 0);
    assert_eq!(state.pe_spread_ui_wait_4012c1_count, before_wait + 1);
    assert_eq!(state.pe_spread_hold_cursor_id, 0);
    assert_eq!(state.pe_spread_hold_cursor_arg, 0);
}

#[test]
fn pe_microwave_plant_tile_scans_x_then_y() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(pe_microwave_plant_tile(&city), None);
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    stamp_complete_building_for_test(&mut city, 10, 80, MICROWAVE_POWER_BUILDING_ID, 4);
    assert_eq!(pe_microwave_plant_tile(&city), Some((10, 80)));
}

#[test]
fn microwave_walk_queues_zap_per_non_plant_step() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    let mut state = test_state(city);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let zaps = pe_microwave_walk_with_rng(&mut preview_city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .zap_count;
    assert!(zaps > 1, "0x27 walk should zap off the 4×4 plant");
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    assert!(state.start_disaster(DisasterKind::Microwave));
    assert_eq!(
        state.pending_disaster_extra_sounds,
        vec![PlacementSound::PowerLine; zaps - 1]
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_microwave_walk_ignite_mid_xtxt_destroy_before_paint() {
    // PE @ `0x45f6bc` → FUN_00460e10 flag 1: mid-XTXT DestroyStructure(1,1) then fire paint.
    let mut city = City::new_blank("/tmp/sc2000-mw-mid.sc2", "MwMid").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    stamp_complete_building_for_test(&mut city, 48, 48, 210, 3);
    city.set_text_overlay(49, 49, 0x55);
    for &(tx, ty) in &[(48, 48), (49, 48), (48, 49), (49, 49)] {
        city.set_traffic_density(tx, ty, 0x27);
    }
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_firestorm_can_ignite(&city, 49, 49));
    pe_fun_00460e10_ignite(&mut city, None, &mut lfsr, 49, 49, 1);
    for dy in 0..3 {
        for dx in 0..3 {
            assert_eq!(city.building_at(48 + dx, 48 + dy), 0);
            assert_eq!(city.text_overlay_at(48 + dx, 48 + dy), Some(0xff));
        }
    }
    assert_eq!(city.traffic_density_at(49, 49), Some(0));
}

#[test]
fn pe_nonnetwork_link_counter_decrements_on_demolition_and_fire() {
    let mut city = City::new_blank("/tmp/pe-nonnetwork-destroy.sc2", "Links").unwrap();
    city.set_building(20, 20, 0x0d);
    city.set_text_overlay(20, 20, 0xfa);
    city.pe_nonnetwork_neighbor_links = 1;
    pe_destroy_building_mode0_optional_crt(&mut city, None, 20, 20);
    assert_eq!(city.pe_nonnetwork_neighbor_links, 0);

    let mut fire_city = City::new_blank("/tmp/pe-nonnetwork-fire.sc2", "Links").unwrap();
    fire_city.set_building(20, 20, 0x0d);
    fire_city.set_text_overlay(20, 20, 0xfa);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_fun_00460e10_ignite(
        &mut fire_city,
        None,
        &mut lfsr,
        20,
        20,
        1
    ));
    assert_eq!(
        fire_city.pe_nonnetwork_neighbor_links,
        u16::MAX,
        "native WORD decrement wraps"
    );

    fire_city.set_building(21, 20, 0x1d);
    fire_city.set_text_overlay(21, 20, 0xfa);
    assert!(pe_fun_00460e10_ignite(
        &mut fire_city,
        None,
        &mut lfsr,
        21,
        20,
        1
    ));
    assert_eq!(
        fire_city.pe_nonnetwork_neighbor_links,
        u16::MAX,
        "network markers use a different counter"
    );
}

#[test]
fn pe_fun_00460e10_ignite_fa_clears_non_network_marker_460f71() {
    // PE FUN_00460e10 @ `0x460f43`–`0x460f78`: XTXT 0xfa → clear, dec non-network `0x4ca3f0`, paint 0xff.
    let mut city = City::new_blank("/tmp/sc2000-ignite-fa.sc2", "IgniteFa").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_fun_00460e10_ignite(
        &mut city,
        Some(&mut crt),
        &mut lfsr,
        20,
        20,
        1,
    ));
    assert_eq!(city.text_overlay_at(20, 20), Some(0xff));
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 0);
}

#[test]
fn pe_fb_zone_protect_table_matches_win95() {
    assert!(pe_fb_zone_degradable(0x70));
    assert!(pe_fb_zone_degradable(0x87));
    assert!(!pe_fb_zone_degradable(0x88));
    assert!(!pe_fb_zone_degradable(0x8b));
    assert!(pe_fb_zone_degradable(0x8c));
    assert!(!pe_fb_zone_degradable(0xa6));
    assert!(!pe_fb_zone_degradable(0xc2));
    assert!(!pe_fb_zone_degradable(0xc5));
    assert!(!pe_fb_zone_degradable(0x6f));
    assert!(!pe_fb_zone_degradable(0xc6));
}

#[test]
fn pe_fb_tick_degrades_1x1_zone_via_change_building_abandon() {
    // PE `0x460061` → ChangeBuilding replace=0 → CAP mode 4 abandon (not LFSR rubble).
    let mut city = City::new_blank("/tmp/sc2000-fb-abandon.sc2", "FbAbandon").expect("blank");
    city.set_text_overlay(40, 40, 0xfb);
    city.set_building(40, 40, 0x70);
    city.set_traffic_density(40, 40, 0x44);
    let src_alt = city.altitude_at(40, 40) & 0x1f;
    city.set_altitude(41, 40, src_alt.saturating_sub(1), 0);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0f);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_fb_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.degraded > 0 {
        assert!(
            ABANDONED_POP1_BUILDINGS.contains(&expected.building_at(40, 40)),
            "expected CAP abandon pop1, got {:#04x}",
            expected.building_at(40, 40)
        );
        assert_eq!(stats.degraded, 1);
        assert_eq!(expected.traffic_density_at(40, 40), Some(0x44));
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    assert_eq!(state.tick_pe_disaster_overlays(), stats.changed());
    assert_eq!(state.city.building_at(40, 40), expected.building_at(40, 40));
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            assert_eq!(
                state.city.text_overlay_at(x, y),
                expected.text_overlay_at(x, y),
                "XTXT at ({x},{y})"
            );
        }
    }
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_fb_degrade_destroy_building_consumes_crt_after_abandon_pick() {
    // PE ChangeBuilding: CRT abandon pick, selector-2 generic scatter, then
    // DestroyBuilding CRT PlaceTile rubble.
    let mut city = City::new_blank("/tmp/sc2000-fb-crt.sc2", "FbCrt").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, RESIDENTIAL_2X2_BUILDINGS[0], 2);
    or_rci_building_zone_corners_for_test(&mut city, (40, 40), 2);
    let mut preview = Win95CrtRand::new(0x45ff_9a02);
    let abandoned = abandoned_building_for_pop_with_rng(2, &mut preview).expect("abandon");
    for _ in 0..8 {
        let _ = preview.random_u15(); // selector-2 generic scatter at `0x459C40`
    }
    for _ in 0..4 {
        let _ = preview.rubble_tile_id();
    }
    let mut crt = Win95CrtRand::new(0x45ff_9a02);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_fb_degrade_under_cloud(
        &mut city, 40, 40, &mut crt, &mut lfsr
    ));
    assert_eq!(city.building_at(40, 40), abandoned);
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn pe_fb_tick_skips_degrade_for_protected_zone() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfb);
    city.set_building(40, 40, 0x88);
    let src_alt = city.altitude_at(40, 40) & 0x1f;
    city.set_altitude(41, 40, src_alt.saturating_sub(1), 0);
    let stats = pe_fb_tick_city(&mut city);
    assert_eq!(stats.degraded, 0);
    assert_eq!(city.building_at(40, 40), 0x88);
}

#[test]
fn pe_destroy_building_size_matches_win95_table() {
    assert_eq!(pe_destroy_building_size(0x0d), 1);
    assert_eq!(pe_destroy_building_size(0x48), 1);
    assert_eq!(pe_destroy_building_size(0x49), 2);
    assert_eq!(pe_destroy_building_size(0x50), 2);
    assert_eq!(pe_destroy_building_size(0x51), 1);
    assert_eq!(pe_destroy_building_size(0x61), 2);
    assert_eq!(pe_destroy_building_size(0x6b), 2);
    assert_eq!(pe_destroy_building_size(0x6c), 1);
    assert_eq!(pe_destroy_building_size(0x70), 1);
    assert_eq!(pe_destroy_building_size(COMMERCIAL_2X2_BUILDINGS[0]), 2);
    assert_eq!(pe_destroy_building_size(COMMERCIAL_3X3_BUILDINGS[0]), 3);
    assert_eq!(pe_destroy_building_size(NUCLEAR_POWER_BUILDING_ID), 4);
}

#[test]
fn pe_destroy_building_footprint_treats_highways_as_2x2() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 0x49, 2);
    let mut tiles = pe_destroy_building_footprint(&city, 11, 11);
    tiles.sort_unstable();
    assert_eq!(tiles, vec![(10, 10), (10, 11), (11, 10), (11, 11)]);
}

#[test]
fn pe_destroy_building_size_matches_city_multi_tile_table() {
    // Keep app DestroyStructure size and city microsim demolish size in lockstep.
    for building in 0u8..=255 {
        let pe = pe_destroy_building_size(building);
        let city = if matches!(building, 0x49..=0x50 | 0x61..=0x6b) {
            2
        } else if building < 0x70 {
            1
        } else {
            usize::from(crate::city::multi_tile_building_size(building)).max(1)
        };
        assert_eq!(
            pe, city,
            "size drift at XBLD {building:#04x}: pe={pe} city={city}"
        );
    }
}

#[test]
fn pe_place_tile_size_park_ors_xbit_piped_only() {
    // PE @ `0x440e65`/`0x440e72`: XBLD 0x0D/0xD5 use XBIT OR mask 0x20 (not 0xe0).
    let mut city = City::new_blank("/tmp/sc2000-pts-park.sc2", "PtsPark").expect("blank");
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0).with_watered(true).with_powered(true),
    );
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 0x0d, 1));
    assert_eq!(city.building_at(20, 20), 0x0d);
    let bits = city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped() && bits.watered());
    assert!(!bits.powered() && !bits.powerable());
    assert_eq!(bits.raw_bits() & 0xe0, 0x20);
    assert_eq!(city.zone_at(20, 20).unwrap().raw_bits(), ZONE_CORNER_ALL);
}

#[test]
fn pe_place_tile_size_0xd5_ors_xbit_piped_only() {
    // Same PE mask arm for XBLD 0xD5 as 0x0D (`0x440e5e`/`0x440e72`).
    let mut city = City::new_blank("/tmp/sc2000-pts-d5.sc2", "PtsD5").expect("blank");
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0).with_watered(true).with_powerable(true),
    );
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 0xd5, 1));
    assert_eq!(city.building_at(20, 20), 0xd5);
    let bits = city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped() && bits.watered());
    assert!(!bits.powerable() && !bits.powered());
    assert_eq!(bits.raw_bits() & 0xe0, 0x20);
}

#[test]
fn citytool_small_park_uses_place_tile_size_xbit_and_corner_all() {
    // PE CityToolPlaceSelectedBuilding @ `0x440731` → PlaceTile-size: park XBIT `|=0x20`,
    // size-1 epilogue ORs `ZONE_CORNER_ALL` (`0x440fe3`).
    let mut city = City::new_blank("/tmp/sc2000-ct-park.sc2", "CtPark").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(
        30,
        30,
        TileBits::new(0).with_watered(true).with_powered(true),
    );
    city.set_zone(30, 30, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_traffic_density(30, 30, 0x2a);
    let mut state = test_state(city);
    state.tool = EditTool::SmallPark;
    assert_eq!(state.place_building_at(30, 30, SMALL_PARK_BUILDING_ID), 1);
    assert_eq!(state.city.building_at(30, 30), SMALL_PARK_BUILDING_ID);
    let bits = state.city.tile_bits_at(30, 30).unwrap();
    assert!(bits.piped() && !bits.watered());
    assert!(!bits.powered() && !bits.powerable());
    assert_eq!(bits.raw_bits() & 0xe0, 0x20);
    assert_eq!(
        state.city.zone_at(30, 30).unwrap().raw_bits(),
        ZONE_CORNER_ALL
    );
    assert_eq!(state.city.traffic_density_at(30, 30), Some(0x2a));
}

#[test]
fn citytool_small_park_skips_microsim_xtxt_below_0xc6() {
    // PE microsim allocate `0x44cf60`: tile `< 0xc6` returns 0 — no XTXT write.
    let mut city = City::new_blank("/tmp/sc2000-ct-park-xtxt.sc2", "CtParkX").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(30, 30, 0x55);
    let mut state = test_state(city);
    state.tool = EditTool::SmallPark;
    assert_eq!(state.place_building_at(30, 30, SMALL_PARK_BUILDING_ID), 1);
    assert_eq!(state.city.building_at(30, 30), SMALL_PARK_BUILDING_ID);
    assert_eq!(state.city.text_overlay_at(30, 30), Some(0x55));
}

#[test]
fn maptool_sub_to_rail_place_rails_reshape_only_adjacency_neighbor() {
    // PE @ `0x441da1` → PlaceRails reshape once on adjacency-hit `(edi,ebx)`.
    let mut city = City::new_blank("/tmp/sc2000-str-rails.sc2", "StrRails").expect("blank");
    city.set_city_funds(20_000);
    // Straight EW rail east (hit first in E/S/W/N) and west (must stay untouched).
    city.set_building(21, 20, 0x2c);
    city.set_building(19, 20, 0x2c);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    assert!(state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), 108); // NETWORK_RIGHT
    let zone = state.city.zone_at(20, 20).unwrap().raw_bits();
    assert_eq!(zone & 0xf0, ZONE_CORNER_ALL);
    let east = state.city.building_at(21, 20);
    assert_ne!(east, 0x2c, "east adjacency rail should PlaceRails-reshape");
    assert!(
        (0x2c..0x3b).contains(&east),
        "reshaped east tile should stay in PlaceRails band, got {east:#04x}"
    );
    assert_eq!(
        state.city.building_at(19, 20),
        0x2c,
        "west non-hit neighbor must not invent-refresh"
    );
}

#[test]
fn maptool_sub_to_rail_or_xzon_corners_after_placetile() {
    // PE MapTool SubToRail @ `0x441d8e`: `XZON |= 0xf0` after PlaceTile (type nibble kept).
    let mut city = City::new_blank("/tmp/sc2000-str-xzon.sc2", "StrXzon").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(20, 20, 0x01); // residential type, corners clear
    city.set_building(21, 20, 0x2c);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    assert!(state.place_sub_to_rail_at(20, 20));
    assert!((108..=111).contains(&state.city.building_at(20, 20)));
    let zone = state.city.zone_at(20, 20).unwrap().raw_bits();
    assert_eq!(zone & 0xf0, ZONE_CORNER_ALL);
    assert_eq!(zone & 0x0f, 0x01);
}

#[test]
fn maptool_sub_to_rail_refuses_xbld_ge_0x0d() {
    // PE @ `0x441ba8`: XBLD >= 0x0d → return 0 (power 0x0e would pass PlaceTile-size gate).
    let mut city = City::new_blank("/tmp/sc2000-str-0d.sc2", "StrGe0d").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 0x0e);
    city.set_building(21, 20, 0x2c);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    assert!(state.sub_to_rail_plan_at(20, 20).is_none());
    assert!(!state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0x0e);
    assert_eq!(state.city.building_at(21, 20), 0x2c);
}

#[test]
fn maptool_sub_to_rail_refuses_xbld_eq_5() {
    // PE @ `0x441bb6`: XBLD == 5 → return 0 (local SubToRail gate before adjacency).
    let mut city = City::new_blank("/tmp/sc2000-str-xbld5.sc2", "StrXbld5").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 5);
    city.set_building(21, 20, 0x2c);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    assert!(state.sub_to_rail_plan_at(20, 20).is_none());
    assert!(!state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), 5);
    assert_eq!(state.city.building_at(21, 20), 0x2c);

    // Control: vacant flat tile with same adjacency still plans.
    state.city.set_building(20, 20, 0);
    assert!(state.sub_to_rail_plan_at(20, 20).is_some());
}

#[test]
fn citytool_size3_nw_edge_matches_pe_440c64_post_dec() {
    // PE CityTool size≠2 click edge @ `0x4405c8`: click≥2, click+size≤0x80, then
    // PlaceTile-size `dec x/y` @ `0x440c64`. Rust NW = click−1 → nw≥1, nw+size≤0x7f.
    let mut city = City::new_blank("/tmp/sc2000-sz3-edge.sc2", "Sz3Edge").expect("blank");
    city.set_city_funds(50_000);
    let state = test_state(city);
    let building = 0xae; // size 3 (174..=197)

    let edge_x0 = state.building_placement_at(0, 20, building);
    assert!(
        edge_x0.valid_tiles.is_empty(),
        "NW x=0 is pre-dec click=1, below PE click≥2"
    );
    assert!(!edge_x0.invalid_tiles.is_empty());

    let edge_hi = state.building_placement_at(0x7d, 20, building);
    assert!(
        edge_hi.valid_tiles.is_empty(),
        "NW 0x7d + size 3 > 0x7f (PE click+size > 0x80)"
    );

    let ok = state.building_placement_at(1, 20, building);
    assert_eq!(
        ok.valid_tiles.len(),
        9,
        "NW (1,20) is post-dec of PE click (2,20)"
    );
    assert!(ok.invalid_tiles.is_empty());
}

#[test]
fn maptool_sub_to_rail_skips_fun_00443270_preclear_and_epilogue() {
    // PE SubToRail place @ `0x441d00` never calls `FUN_00443270` / `0x44378e`.
    // Same adjacency setup as PlaceRails-neighbor test; also assert power north
    // stays put (epilogue would Power-refresh neighbors).
    let mut city = City::new_blank("/tmp/sc2000-str-no43270.sc2", "StrNo43270").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(21, 20, 0x2c); // adjacency rail east
    city.set_building(19, 20, 0x2c); // non-hit rail west
    city.set_building(20, 19, 0x0e); // non-hit power north
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    let plan = state.sub_to_rail_plan_at(20, 20);
    assert!(plan.is_some(), "expected SubToRail plan, got None");
    assert!(state.place_sub_to_rail_at(20, 20));
    assert!((108..=111).contains(&state.city.building_at(20, 20)));
    assert_eq!(state.city.underground_at(20, 20), UNDER_TILE_GREY_BOX_2);
    assert_eq!(
        state.city.building_at(19, 20),
        0x2c,
        "non-adjacency rail must not get 0x44378e reshape"
    );
    assert_eq!(
        state.city.building_at(20, 19),
        0x0e,
        "non-adjacency power must not get 0x44378e reshape"
    );
}

#[test]
fn maptool_sub_to_rail_underground_neighbor_flips_side() {
    // PE @ `0x441ce8`: subway XUND neighbor → `si = (si+2)&3` (east und → west stamp 110).
    let mut city = City::new_blank("/tmp/sc2000-str-undflip.sc2", "StrUndFlip").expect("blank");
    city.set_city_funds(20_000);
    city.set_underground(21, 20, 0x01);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    assert!(state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), 110);
}

#[test]
fn citytool_place_refuses_military_zone_type_7_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440d40`: XZON type nibble 7 refuses before stamp.
    let mut city = City::new_blank("/tmp/sc2000-ct-z7.sc2", "CtZ7").expect("blank");
    city.set_city_funds(20_000);
    for dx in 0..3usize {
        for dy in 0..3usize {
            city.set_zone(40 + dx, 40 + dy, ZONE_MILITARY | ZONE_CORNER_ALL);
        }
    }
    let mut state = test_state(city);
    state.tool = EditTool::Police;
    let place = state.tool_placement_at(40, 40).unwrap();
    assert!(place.valid_tiles.is_empty());
    assert_eq!(place.invalid_tiles.len(), 9);
    assert_eq!(state.place_building_at(40, 40, 210), 0);
    assert_eq!(state.city.building_at(40, 40), 0);
}

#[test]
fn citytool_place_overwrites_mid_xbld_like_place_tile_size() {
    // PE PlaceTile-size allows XBLD `0x0E..0x1C` (not 5 / 0x0D / >=0x1D) then overwrites.
    // Small park `0x0D` has an extra origin `>=0x0D` refuse — use statue (`0xDB`).
    let mut city = City::new_blank("/tmp/sc2000-ct-mid.sc2", "CtMid").expect("blank");
    city.set_city_funds(20_000);
    city.set_rewards_available_mask(reward_mask_for_tool(EditTool::Statue).unwrap_or(0));
    city.set_building(42, 42, 0x10);
    let mut state = test_state(city);
    state.tool = EditTool::Statue;
    assert_eq!(state.place_building_at(42, 42, 219), 1);
    assert_eq!(state.city.building_at(42, 42), 219);
}

#[test]
fn citytool_place_refuses_existing_small_park_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440d37`: existing 0x0D refuses any PlaceTile-size stamp.
    let mut city = City::new_blank("/tmp/sc2000-ct-expark.sc2", "CtExPark").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(43, 43, SMALL_PARK_BUILDING_ID);
    let mut state = test_state(city);
    state.tool = EditTool::SmallPark;
    assert_eq!(state.place_building_at(43, 43, SMALL_PARK_BUILDING_ID), 0);
    assert_eq!(state.city.building_at(43, 43), SMALL_PARK_BUILDING_ID);
}

#[test]
fn citytool_small_park_refuses_mid_xbld_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440e79`: SmallPark refuses origin XBLD `>=0x0D`
    // (power line `0x0E` passes general can_place but fails the park arm).
    let mut city = City::new_blank("/tmp/sc2000-ct-park-mid.sc2", "CtPkMid").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(44, 44, 0x0e);
    let mut state = test_state(city);
    state.tool = EditTool::SmallPark;
    let place = state.building_placement_at(44, 44, SMALL_PARK_BUILDING_ID);
    assert!(place.valid_tiles.is_empty());
    assert_eq!(place.invalid_tiles, vec![(44, 44)]);
    assert_eq!(state.place_building_at(44, 44, SMALL_PARK_BUILDING_ID), 0);
    assert_eq!(state.city.building_at(44, 44), 0x0e);
}

#[test]
fn citytool_big_park_uses_place_tile_size_xbit_and_corners() {
    // PE CityToolPlaceSelectedBuilding @ `0x440731` → PlaceTile-size for 0xD5:
    // XBIT `|=0x20`; size>1 epilogue ORs `0x4dc4d0` rotation corners.
    let mut city = City::new_blank("/tmp/sc2000-ct-bigpark.sc2", "CtBigPk").expect("blank");
    city.set_city_funds(20_000);
    city.set_view_rotation(0);
    for dx in 0..3usize {
        for dy in 0..3usize {
            city.set_tile_bits(
                30 + dx,
                30 + dy,
                TileBits::new(0).with_watered(true).with_powered(true),
            );
            city.set_zone(30 + dx, 30 + dy, ZONE_RESIDENTIAL_LIGHT | 0x80);
        }
    }
    city.set_traffic_density(31, 31, 0x2b);
    let mut state = test_state(city);
    state.tool = EditTool::BigPark;
    assert_eq!(state.place_building_at(30, 30, BIG_PARK_BUILDING_ID), 9);
    let corner_bits = pe_choose_and_place_corner_bits(0);
    for dx in 0..3usize {
        for dy in 0..3usize {
            assert_eq!(
                state.city.building_at(30 + dx, 30 + dy),
                BIG_PARK_BUILDING_ID
            );
            let bits = state.city.tile_bits_at(30 + dx, 30 + dy).unwrap();
            assert!(bits.piped() && !bits.watered());
            assert!(!bits.powered() && !bits.powerable());
            assert_eq!(bits.raw_bits() & 0xe0, 0x20);
            let expect = building_zone_bits_for_tile(30, 30, 30 + dx, 30 + dy, 3, corner_bits);
            assert_eq!(
                state.city.zone_at(30 + dx, 30 + dy).unwrap().raw_bits(),
                expect
            );
        }
    }
    assert_eq!(state.city.traffic_density_at(31, 31), Some(0x2b));
    let xtxt = state.city.text_overlay_at(30, 30).unwrap_or(0);
    assert!(
        xtxt >= MIN_SIM_TEXT_ENTRIES,
        "big park microsim XTXT {xtxt}"
    );
    for dx in 0..3usize {
        for dy in 0..3usize {
            assert_eq!(
                state.city.text_overlay_at(30 + dx, 30 + dy),
                Some(xtxt),
                "big park footprint XTXT"
            );
        }
    }
}

#[test]
fn pe_fun_00447500_inner_zero_skips_place_tile() {
    // PE @ `0x44753c`: inner `rand()&0x3f == 0` skips the jittered PlaceTile loop.
    // CRT seed 103 yields base picks then inner 0 on the first outer iteration.
    let mut city = City::new_blank("/tmp/sc2000-tree-inner0.sc2", "TreeIn0").expect("blank");
    let mut rng = Win95CrtRand::new(103);
    pe_fun_00447500(&mut city, &mut rng, 1);
    let trees = (0..CITY_WIDTH)
        .flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)))
        .filter(|&(x, y)| (6..=0x0c).contains(&city.building_at(x, y)))
        .count();
    assert_eq!(trees, 0, "inner==0 must not PlaceTile trees");
}

#[test]
fn pe_place_tile_size_2x2_refuses_when_any_footprint_tile_blocked() {
    // PE validation walks the whole size×size block before any stamp.
    let mut city = City::new_blank("/tmp/sc2000-pts-2x2-block.sc2", "Pts2Blk").expect("blank");
    city.set_building(21, 21, 5); // SE of origin
    city.set_tile_bits(20, 20, TileBits::new(0).with_watered(true));
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0xe0, 2));
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.building_at(21, 21), 5);
    assert!(city.tile_bits_at(20, 20).unwrap().watered());
    assert!(!city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn pe_place_tile_size_2x2_refuses_edge_tiles_outside_1_through_0x7e() {
    // PE @ `0x440cc5`–`0x440cdb`: size≥2 each footprint tile must be in `1..=0x7e`.
    let mut city = City::new_blank("/tmp/sc2000-pts-2x2-edge.sc2", "Pts2Edge").expect("blank");
    assert!(!pe_place_tile_size_440c50(&mut city, 0, 5, 0xe0, 2));
    assert!(!pe_place_tile_size_440c50(&mut city, 5, 0, 0xe0, 2));
    // Origin 0x7e fits the map (0x7e+2≤0x80) but SE tile x=0x7f is outside 1..=0x7e.
    assert!(!pe_place_tile_size_440c50(&mut city, 0x7e, 5, 0xe0, 2));
    assert!(!pe_place_tile_size_440c50(&mut city, 5, 0x7e, 0xe0, 2));
    assert_eq!(city.building_at(0, 5), 0);
    assert_eq!(city.building_at(0x7e, 5), 0);
    assert_eq!(city.building_at(0x7f, 5), 0);

    assert!(pe_place_tile_size_440c50(&mut city, 1, 5, 0xe0, 2));
    assert_eq!(city.building_at(1, 5), 0xe0);
    assert_eq!(city.building_at(2, 6), 0xe0);
}

#[test]
fn pe_citytool_size2_edge_bounds_44057e() {
    // PE CityTool @ `0x44057e`–`0x4405a6`: size==2 click needs x,y≥1 and x+3,y+3≤0x80.
    // XBLD 140 is a real 2×2 (`xbld_tile_size`); 0xe0 is size-1 in the table.
    let city = City::new_blank("/tmp/sc2000-ct-s2-edge.sc2", "CtS2Edge").expect("blank");
    let state = test_state(city);
    let b2 = 140u8;
    assert_eq!(xbld_tile_size(b2), 2);
    for &(ox, oy) in &[(0usize, 5), (5, 0), (0x7e, 5), (5, 0x7e)] {
        let place = state.building_placement_at(ox, oy, b2);
        assert!(
            place.valid_tiles.is_empty(),
            "size-2 at ({ox},{oy}) should be invalid preview"
        );
        assert!(!place.invalid_tiles.is_empty());
    }
    let ok = state.building_placement_at(1, 5, b2);
    assert_eq!(ok.valid_tiles.len(), 4);
    assert!(ok.invalid_tiles.is_empty());
    let ok_hi = state.building_placement_at(0x7d, 5, b2);
    assert_eq!(ok_hi.valid_tiles.len(), 4);
    assert!(ok_hi.invalid_tiles.is_empty());
    // Size-1 still allowed at map edge (`0x440ce7`).
    let s1 = state.building_placement_at(0, 20, 0xc8);
    assert_eq!(s1.valid_tiles, vec![(0, 20)]);
    let s1b = state.building_placement_at(0x7f, 20, 0xc8);
    assert_eq!(s1b.valid_tiles, vec![(0x7f, 20)]);
}

#[test]
fn citytool_post_place_power_recalc_440906_clears_stale_powered_when_pop_low() {
    // PE @ `0x440906`–`0x440949` → `FUN_00467860`: map-wide `XBIT &= 0xb7`.
    let mut city = City::new_blank("/tmp/sc2000-ct-pwr-low.sc2", "CtPwrLow").expect("blank");
    city.set_city_funds(50_000);
    city.set_tile_bits(40, 40, TileBits::new(0).with_powered(true));
    let mut state = test_state(city);
    state.tool = EditTool::Police;
    assert!(state.place_building_at(20, 20, 210) > 0);
    assert!(state.city.tile_bits_at(20, 20).unwrap().powerable());
    assert!(
        !state.city.tile_bits_at(40, 40).unwrap().powered(),
        "FUN_00467860 should clear stale XBIT 0x40"
    );
}

#[test]
fn citytool_post_place_power_recalc_440906_skips_when_pop_ge_0xc350() {
    let mut city = City::new_blank("/tmp/sc2000-ct-pwr-hi.sc2", "CtPwrHi").expect("blank");
    city.set_city_funds(50_000);
    city.persist_pe_day22_population_leftover(0xc350);
    city.set_tile_bits(40, 40, TileBits::new(0).with_powered(true));
    let mut state = test_state(city);
    state.tool = EditTool::Police;
    assert!(state.place_building_at(20, 20, 210) > 0);
    assert!(
        state.city.tile_bits_at(40, 40).unwrap().powered(),
        "pop >= 0xc350 skips FUN_00467860"
    );
}

#[test]
fn citytool_post_place_power_recalc_440906_skips_when_statue_clears_powerable() {
    // PE statue arm @ `0x440835` clears XBIT 0x80 before the `0x440906` epilogue.
    let mut city = City::new_blank("/tmp/sc2000-ct-pwr-statue.sc2", "CtPwrSt").expect("blank");
    city.set_city_funds(50_000);
    city.set_tile_bits(40, 40, TileBits::new(0).with_powered(true));
    let mut state = test_state(city);
    state.tool = EditTool::Statue;
    assert!(state.place_building_at(20, 20, 219) > 0);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());
    assert!(
        state.city.tile_bits_at(40, 40).unwrap().powered(),
        "cleared 0x80 skips FUN_00467860"
    );
}

#[test]
fn maptool_hydro_post_place_power_recalc_4416d4_clears_stale_powered_when_pop_low() {
    // PE @ `0x4416d4`–`0x4416e0` → `FUN_00467860`: pop-only gate, map-wide `&=0xb7`.
    let mut city = City::new_blank("/tmp/sc2000-hydro-pwr-low.sc2", "HyPwrLow").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_tile_bits(
        40,
        40,
        TileBits::new(0).with_powered(true).with_flatten_mark(true),
    );
    city.set_tile_bits(41, 41, TileBits::new(0).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantHydro;
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert!(
        !state.city.tile_bits_at(40, 40).unwrap().powered(),
        "FUN_00467860 should clear stale XBIT 0x40"
    );
    assert!(
        !state.city.tile_bits_at(40, 40).unwrap().flatten_mark(),
        "FUN_00467860 should clear work XBIT 0x08"
    );
    assert!(
        state.city.tile_bits_at(41, 41).unwrap().watered(),
        "hydro path must not call FUN_00464550"
    );
}

#[test]
fn maptool_hydro_post_place_power_recalc_4416d4_skips_when_pop_ge_0xc350() {
    let mut city = City::new_blank("/tmp/sc2000-hydro-pwr-hi.sc2", "HyPwrHi").expect("blank");
    city.set_city_funds(50_000);
    city.persist_pe_day22_population_leftover(0xc350);
    city.set_terrain(20, 20, 0x3e);
    city.set_tile_bits(40, 40, TileBits::new(0).with_powered(true));
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantHydro;
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert!(
        state.city.tile_bits_at(40, 40).unwrap().powered(),
        "pop >= 0xc350 skips FUN_00467860"
    );
}

#[test]
fn powerline_post_place_power_recalc_43f396_clears_stale_powered_when_pop_low() {
    // PE @ `0x43f396`–`0x43f3aa` → `FUN_00467860`: pop-only gate after power-line place.
    let mut city = City::new_blank("/tmp/sc2000-pline-pwr-low.sc2", "PlPwrLow").expect("blank");
    city.set_city_funds(50_000);
    city.set_tile_bits(
        40,
        40,
        TileBits::new(0).with_powered(true).with_flatten_mark(true),
    );
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 1);
    assert!(
        !state.city.tile_bits_at(40, 40).unwrap().powered(),
        "FUN_00467860 should clear stale XBIT 0x40"
    );
    assert!(
        !state.city.tile_bits_at(40, 40).unwrap().flatten_mark(),
        "FUN_00467860 should clear work XBIT 0x08"
    );
}

#[test]
fn powerline_post_place_power_recalc_43f396_skips_when_pop_ge_0xc350() {
    let mut city = City::new_blank("/tmp/sc2000-pline-pwr-hi.sc2", "PlPwrHi").expect("blank");
    city.set_city_funds(50_000);
    city.persist_pe_day22_population_leftover(0xc350);
    city.set_tile_bits(40, 40, TileBits::new(0).with_powered(true));
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 1);
    assert!(
        state.city.tile_bits_at(40, 40).unwrap().powered(),
        "pop >= 0xc350 skips FUN_00467860"
    );
}

#[test]
fn place_power_lines_skips_when_xbit_already_powerable() {
    // PE PlacePowerLines AtCoordinates @ `0x44a8d3`: test XBIT 0x80 / jnz return.
    let mut city = City::new_blank("/tmp/sc2000-pwr-already.sc2", "PwrAlready").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x11);
    city.set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
    city.set_text_overlay(20, 20, 2);
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 0);
    assert_eq!(state.city.building_at(20, 20), 0x11);
    assert!(state.city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn place_power_lines_places_when_powerable_clear() {
    let mut city = City::new_blank("/tmp/sc2000-pwr-fresh.sc2", "PwrFresh").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 1);
    assert!((0x0e..0x1d).contains(&state.city.building_at(20, 20)));
    assert!(state.city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn place_power_lines_table_hit_placetile_then_xbit_or_44a968() {
    // PE @ `0x44a940` PlaceTile then `0x44a968` XBIT `|=0x80`; leave XZON/watered.
    let mut city = City::new_blank("/tmp/sc2000-pwr-or.sc2", "PwrOr").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x1d);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 40);
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(20, 20), 0x43);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.powerable());
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
    assert_eq!(state.city.traffic_density_at(20, 20), Some(40));
}

#[test]
fn place_roads_skips_when_xbld_already_flat_road() {
    // PE PlaceRoads AtCoordinates @ `0x44acf4`: XBLD in `0x1d..0x2c` → return.
    let mut city = City::new_blank("/tmp/sc2000-road-already.sc2", "RoadAlready").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x1d);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 0);
    assert_eq!(state.city.building_at(20, 20), 0x1d);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_roads_skips_when_xbld_already_elevated_road() {
    // PE PlaceRoads AtCoordinates @ `0x44acfa`: XBLD in `0x3f..0x47` → return.
    let mut city = City::new_blank("/tmp/sc2000-road-elev.sc2", "RoadElev").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x3f);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 0);
    assert_eq!(state.city.building_at(20, 20), 0x3f);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_roads_skips_when_xbld_already_road_rail_cross() {
    // PE PlaceRoads AtCoordinates @ `0x44ad0a`/`0x44ad14`: XBLD `0x4b`/`0x4c` → return.
    let mut city = City::new_blank("/tmp/sc2000-road-cross.sc2", "RoadCross").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x4b);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 0);
    assert_eq!(state.city.building_at(20, 20), 0x4b);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_roads_skips_when_xbld_already_highway() {
    // PE PlaceRoads AtCoordinates @ `0x44ad1e`: XBLD in `0x5d..0x61` → return.
    let mut city = City::new_blank("/tmp/sc2000-road-hwy.sc2", "RoadHwy").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x5d);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 0);
    assert_eq!(state.city.building_at(20, 20), 0x5d);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_roads_places_on_vacant_and_clears_zone_type() {
    let mut city = City::new_blank("/tmp/sc2000-road-fresh.sc2", "RoadFresh").expect("blank");
    city.set_city_funds(50_000);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 1);
    assert!((0x1d..0x2c).contains(&state.city.building_at(20, 20)));
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits() & 0x0f),
        Some(0)
    );
}

#[test]
fn place_rails_skips_when_xbld_already_flat_rail() {
    // PE PlaceRails AtCoordinates @ `0x44b1ca`: XBLD in `0x2c..0x3f` → return.
    let mut city = City::new_blank("/tmp/sc2000-rail-already.sc2", "RailAlready").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x2c);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Rail;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Rail), 0);
    assert_eq!(state.city.building_at(20, 20), 0x2c);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_rails_skips_when_xbld_already_elevated_rail() {
    // PE PlaceRails AtCoordinates @ `0x44b1da`: XBLD in `0x45..0x49` → return.
    let mut city = City::new_blank("/tmp/sc2000-rail-elev.sc2", "RailElev").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x45);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Rail;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Rail), 0);
    assert_eq!(state.city.building_at(20, 20), 0x45);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_rails_skips_when_xbld_already_tunnel_rail() {
    // PE PlaceRails AtCoordinates @ `0x44b1ea`: XBLD in `0x6c..0x70` → return.
    let mut city = City::new_blank("/tmp/sc2000-rail-tunnel.sc2", "RailTunnel").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x6c);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Rail;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Rail), 0);
    assert_eq!(state.city.building_at(20, 20), 0x6c);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_rails_skips_when_xbld_already_rail_road_cross() {
    // PE PlaceRails AtCoordinates @ `0x44b1fa`/`0x44b204`: XBLD `0x4d`/`0x4e` → return.
    let mut city = City::new_blank("/tmp/sc2000-rail-cross.sc2", "RailCross").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x4d);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Rail;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Rail), 0);
    assert_eq!(state.city.building_at(20, 20), 0x4d);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_rails_skips_outside_crossover_sources() {
    // PE @ `0x44b20e`–`0x44b21d`: not `<0x0e` and not table hit → return.
    let mut city = City::new_blank("/tmp/sc2000-rail-window.sc2", "RailWin").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x4b);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Rail;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Rail), 0);
    assert_eq!(state.city.building_at(20, 20), 0x4b);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );

    // Control: road `0x1d` → crossover `0x45`.
    state.city.set_building(22, 22, 0x1d);
    assert_eq!(state.place_network_at(22, 22, NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(22, 22), 0x45);
}

#[test]
fn place_rails_places_on_vacant_and_clears_zone_type() {
    let mut city = City::new_blank("/tmp/sc2000-rail-fresh.sc2", "RailFresh").expect("blank");
    city.set_city_funds(50_000);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Rail;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Rail), 1);
    assert!((0x2c..0x3f).contains(&state.city.building_at(20, 20)));
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits() & 0x0f),
        Some(0)
    );
}

#[test]
fn pe_place_tile_size_refuses_radioactivity_xbld_5() {
    // PE @ `0x440d2e`: footprint XBLD 5 → return 0, no stamp.
    let mut city = City::new_blank("/tmp/sc2000-pts-rad.sc2", "PtsRad").expect("blank");
    city.set_building(20, 20, 5);
    city.set_tile_bits(20, 20, TileBits::new(0).with_watered(true));
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0xc8, 1));
    assert_eq!(city.building_at(20, 20), 5);
    assert!(city.tile_bits_at(20, 20).unwrap().watered());
    assert!(!city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn pe_place_tile_size_refuses_existing_small_park() {
    // PE @ `0x440d37`: existing XBLD 0x0D refuses any PlaceTile-size stamp.
    let mut city = City::new_blank("/tmp/sc2000-pts-expark.sc2", "PtsExPark").expect("blank");
    city.set_building(20, 20, 0x0d);
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0xc8, 1));
    assert_eq!(city.building_at(20, 20), 0x0d);
}

#[test]
fn pe_place_tile_size_refuses_existing_xbld_ge_0x1d() {
    // PE @ `0x440d25`: existing XBLD >= 0x1D refuses before stamp.
    let mut city = City::new_blank("/tmp/sc2000-pts-ge1d.sc2", "PtsGe1d").expect("blank");
    city.set_building(20, 20, 0x1d);
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0xc8, 1));
    assert_eq!(city.building_at(20, 20), 0x1d);
}

#[test]
fn pe_place_tile_size_overwrites_rubble_1_to_4() {
    // PE preflight allows XBLD 1..=4 (not 5 / 0x0D / >=0x1D) then PlaceTile overwrites.
    let mut city = City::new_blank("/tmp/sc2000-pts-rubble.sc2", "PtsRubble").expect("blank");
    city.set_building(20, 20, 3);
    city.set_text_overlay(20, 20, 0x21);
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 0xc8, 1));
    assert_eq!(city.building_at(20, 20), 0xc8);
    // Tile 0xc8 owns system microsim slot 4, so the allocator returns XTXT 4+0x33.
    // PlaceTile-size writes that returned marker over the prior overlay on success.
    assert_eq!(city.text_overlay_at(20, 20), Some(0x37));
}

#[test]
fn pe_army_crane_place_tile_size_stamps_crane_on_deep_dry_tile() {
    // PE @ `0x4387e7`: PlaceTile-size(0xe0,1) after earlier ray/ALTM gates.
    let mut city =
        City::new_blank("/tmp/sc2000-army-crane-place.sc2", "ArmyCranePlace").expect("blank");
    city.set_tile_bits(20, 20, TileBits::new(0).with_watered(true));
    city.set_traffic_density(20, 20, 0x44);
    pe_army_crane_place_tile_size_4387e7(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0xe0);
    assert!(city.tile_bits_at(20, 20).unwrap().watered());
    assert_eq!(city.traffic_density_at(20, 20), Some(0x44));
}

#[test]
fn pe_army_crane_write_zone_keeps_corners_replaces_type() {
    // PE @ `0x438816`–`0x438823`: xor/and type nibble; keep existing corners.
    let mut city = City::new_blank("/tmp/sc2000-army-crane-zn.sc2", "ArmyCraneZn").expect("blank");
    city.set_zone(21, 21, ZONE_CORNER_ALL | 0x01);
    pe_army_crane_write_zone_after_place(&mut city, 21, 21, 7);
    assert_eq!(
        city.zone_at(21, 21).unwrap().raw_bits(),
        ZONE_CORNER_ALL | ZONE_MILITARY
    );
}

#[test]
fn pe_army_crane_write_zone_military_clears_xbit_hi_nibble() {
    // PE @ `0x438851`: type 7 → XBIT &= 0x0f.
    let mut city =
        City::new_blank("/tmp/sc2000-army-crane-xbit.sc2", "ArmyCraneXb").expect("blank");
    city.set_tile_bits(
        22,
        22,
        TileBits::new(0)
            .with_powered(true)
            .with_powerable(true)
            .with_piped(true),
    );
    pe_army_crane_write_zone_after_place(&mut city, 22, 22, 7);
    assert_eq!(city.tile_bits_at(22, 22).unwrap().raw_bits() & 0xf0, 0);
}

#[test]
fn pe_army_crane_place_and_zone_stamps_military_crane() {
    let mut city =
        City::new_blank("/tmp/sc2000-army-crane-andzn.sc2", "ArmyCraneAZ").expect("blank");
    // Even crane_x for south-dir parity; five-tile ray + ALTM on fifth.
    city.set_zone(24, 24, ZONE_CORNER_ALL);
    city.set_tile_bits(
        24,
        24,
        TileBits::new(0x01).with_watered(true).with_piped(true),
    );
    for y in 25..=29 {
        city.set_zone(24, y, ZONE_MILITARY | ZONE_CORNER_ALL);
        city.set_tile_bits(24, y, TileBits::new(0).with_water_tile(true));
    }
    city.set_altitude(24, 29, 5, 7);
    assert!(pe_army_crane_place_and_zone_438809(&mut city, 24, 24, 7));
    assert_eq!(city.building_at(24, 24), 0xe0);
    // PlaceTile-size size-1 ORs ZONE_CORNER_ALL before the type-nibble write.
    assert_eq!(
        city.zone_at(24, 24).unwrap().raw_bits(),
        ZONE_CORNER_ALL | ZONE_MILITARY
    );
    // PE type-7 XBIT &= 0x0f clears watered/piped/power; low nibble (salt) survives.
    assert_eq!(city.tile_bits_at(24, 24).unwrap().raw_bits() & 0xf0, 0);
    assert!(city.tile_bits_at(24, 24).unwrap().salt_water());
}

#[test]
fn stamp_zone_development_military_2x2_uses_army_place_tile_size() {
    // PE SimulationGrow `0x4376b0` type-7 allow → PlaceTile-size stamps parking.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-army-2x2.sc2", "StampArmy2x2").expect("blank");
    for (x, y) in [(30, 30), (31, 30), (30, 31), (31, 31)] {
        city.set_zone(x, y, ZONE_MILITARY | ZONE_CORNER_ALL);
        city.set_altitude(x, y, 5, 5);
        city.set_tile_bits(x, y, TileBits::new(0xf0));
    }
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (30, 30),
        zone_id: ZONE_MILITARY,
        building: MILITARY_PARKING_LOT_BUILDING_ID,
        footprint: vec![(30, 30), (31, 30), (30, 31), (31, 31)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    for (x, y) in [(30, 30), (31, 30), (30, 31), (31, 31)] {
        assert_eq!(
            state.city.building_at(x, y),
            MILITARY_PARKING_LOT_BUILDING_ID
        );
        assert_eq!(
            state.city.zone_at(x, y).unwrap().raw_bits() & 0x0f,
            ZONE_MILITARY
        );
        assert_eq!(
            state.city.tile_bits_at(x, y).unwrap().raw_bits() & 0xf0,
            0,
            "type-7 XBIT &= 0x0f"
        );
    }
}

#[test]
fn stamp_zone_development_military_2x2_retries_hangar1_when_place_fails() {
    // PE SimulationGrow army @ `0x43634a`/`0x43635e`: Place fail → hangar1 `0xe8`.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-army-hangar-fb.sc2", "StampHangarFb").expect("blank");
    for (x, y) in [(30, 30), (31, 30), (30, 31), (31, 31)] {
        city.set_zone(x, y, ZONE_MILITARY | ZONE_CORNER_ALL);
        city.set_altitude(x, y, 5, 5);
    }
    // PlaceTile-size refuses XBLD==5 (radioactivity); clear-before only runs for `>=0x0d`.
    city.set_building(30, 30, 5);
    set_military_base_type_for_test(&mut city, MILITARY_BASE_ARMY);
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (30, 30),
        zone_id: ZONE_MILITARY,
        building: MILITARY_PARKING_LOT_BUILDING_ID,
        footprint: vec![(30, 30), (31, 30), (30, 31), (31, 31)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    // Both parking and hangar1 hit the same radioactivity refuse — stay vacant.
    assert_eq!(state.city.building_at(30, 30), 5);
    assert_eq!(state.city.building_at(31, 30), 0);
    assert_eq!(
        state.city.zone_at(30, 30).unwrap().raw_bits() & 0x0f,
        ZONE_MILITARY
    );
}

#[test]
fn stamp_zone_development_military_2x2_hangar2_place_fail_skips_warehouse_retry() {
    // PE AF ladder Place @ `0x436450` ignores return — no warehouse/hangar1 retry.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-af-h2-noretry.sc2", "StampAfH2").expect("blank");
    for (x, y) in [(36, 36), (37, 36), (36, 37), (37, 37)] {
        city.set_zone(x, y, ZONE_MILITARY | ZONE_CORNER_ALL);
        city.set_altitude(x, y, 5, 5);
    }
    city.set_building(36, 36, 5);
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (36, 36),
        zone_id: ZONE_MILITARY,
        building: MILITARY_HANGAR2_BUILDING_ID,
        footprint: vec![(36, 36), (37, 36), (36, 37), (37, 37)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(36, 36), 5);
    assert_eq!(state.city.building_at(37, 36), 0);
}

#[test]
fn pe_army_2x2_place_returns_accepted_false_when_place_tile_size_refuses() {
    let mut city = City::new_blank("/tmp/sc2000-army-2x2-refuse.sc2", "Army2x2Ref").expect("blank");
    for &(x, y) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_zone(x, y, ZONE_MILITARY | ZONE_CORNER_ALL);
    }
    city.set_building(40, 40, 5);
    assert_eq!(
        pe_army_2x2_place_tile_size_438b8e(
            &mut city,
            40,
            40,
            MILITARY_PARKING_LOT_BUILDING_ID,
            ZONE_MILITARY
        ),
        PeArmy2x2Result::Accepted { placed: false }
    );
    assert_eq!(city.building_at(40, 40), 5);
}

#[test]
fn stamp_zone_development_military_size1_uses_army_place_tile_size() {
    // PE SimulationGrow type-7 allow → size-1 PlaceTile-size stamps radar.
    let mut city = City::new_blank("/tmp/sc2000-stamp-army-1x1.sc2", "StampArmy1").expect("blank");
    city.set_zone(32, 32, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_altitude(32, 32, 5, 5);
    city.set_tile_bits(32, 32, TileBits::new(0xf0));
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (32, 32),
        zone_id: ZONE_MILITARY,
        building: MILITARY_RADAR_BUILDING_ID,
        footprint: vec![(32, 32)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(32, 32), MILITARY_RADAR_BUILDING_ID);
    assert_eq!(
        state.city.zone_at(32, 32).unwrap().raw_bits() & 0x0f,
        ZONE_MILITARY
    );
    assert_eq!(
        state.city.tile_bits_at(32, 32).unwrap().raw_bits() & 0xf0,
        0
    );
}

#[test]
fn stamp_zone_development_military_army_refuse_skips_choose_and_place() {
    // PE @ `0x43892c`: XBLD `>= 0x0D` refuses size-1 PlaceTile-size; no CAP fallthrough.
    let mut city = City::new_blank("/tmp/sc2000-army-refuse-cap.sc2", "ArmyRefuse").expect("blank");
    city.set_zone(33, 33, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_building(33, 33, 0x0d);
    city.set_altitude(33, 33, 5, 5);
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (33, 33),
        zone_id: ZONE_MILITARY,
        building: MILITARY_RADAR_BUILDING_ID,
        footprint: vec![(33, 33)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(33, 33), 0x0d);
}

#[test]
fn stamp_zone_development_military_silo_uses_place_tile_and_xund() {
    // PE DoArmyBaseGrowth silo @ `0x438def`: PlaceTile(0xF9)+PlaceXUND(0x22); no CAP XBIT.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-army-silo.sc2", "StampArmySilo").expect("blank");
    let mut footprint = Vec::new();
    for y in 40..43 {
        for x in 40..43 {
            city.set_zone(x, y, ZONE_MILITARY | ZONE_CORNER_ALL);
            city.set_altitude(x, y, 6, 6);
            city.set_building(x, y, 0x05); // small XBLD cleared before PlaceTile
            footprint.push((x, y));
        }
    }
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (40, 40),
        zone_id: ZONE_MILITARY,
        building: MILITARY_SILO_BUILDING_ID,
        footprint,
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    for y in 40..43 {
        for x in 40..43 {
            assert_eq!(state.city.building_at(x, y), MILITARY_SILO_BUILDING_ID);
            assert_eq!(state.city.underground_at(x, y), UNDER_TILE_GREY_BOX_1);
            assert_eq!(
                state.city.zone_at(x, y).unwrap().raw_bits() & 0x0f,
                ZONE_MILITARY
            );
            assert_eq!(
                state.city.tile_bits_at(x, y).unwrap().raw_bits() & XBIT_CHOOSE_AND_PLACE,
                0,
                "silo PE path must not OR ChooseAndPlace XBIT"
            );
        }
    }
}

#[test]
fn pe_army_place_tile_size_refuses_xbld_ge_0x0d() {
    // PE @ `0x43892c`: XBLD >= 0x0D skips PlaceTile-size arm.
    let mut city = City::new_blank("/tmp/sc2000-army-pts-skip.sc2", "ArmyPtsSkip").expect("blank");
    city.set_building(25, 25, 0x0d);
    assert_eq!(
        pe_army_place_tile_size_if_xbld_below_0x0d_43893f(&mut city, 25, 25, 0xe0, 7),
        PeArmySize1Result::NoopXbldAtOrAbove0x0d
    );
    assert_eq!(city.building_at(25, 25), 0x0d);
}

#[test]
fn pe_army_2x2_place_probe_even_aligns_and_checks_zone() {
    let mut city = City::new_blank("/tmp/sc2000-army-2x2-probe.sc2", "Army2x2").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_zone(x, y, ZONE_MILITARY | 0x80);
    }
    assert_eq!(
        pe_army_2x2_place_probe_4389bf(&city, 21, 21, 7),
        PeArmy2x2Probe::Ready {
            origin_x: 20,
            origin_y: 20
        }
    );
    city.set_zone(21, 21, 0x01);
    assert_eq!(
        pe_army_2x2_place_probe_4389bf(&city, 20, 20, 7),
        PeArmy2x2Probe::Refuse
    );
}

#[test]
fn pe_army_2x2_place_probe_refuses_xbld_ge_0xeb() {
    let mut city = City::new_blank("/tmp/sc2000-army-2x2-eb.sc2", "Army2x2Eb").expect("blank");
    city.set_building(30, 30, 0xeb);
    for &(x, y) in &[(30, 30), (31, 30), (30, 31), (31, 31)] {
        city.set_zone(x, y, ZONE_MILITARY);
    }
    assert_eq!(
        pe_army_2x2_place_probe_4389bf(&city, 30, 30, 7),
        PeArmy2x2Probe::Refuse
    );
}

#[test]
fn pe_army_2x2_place_probe_refuses_existing_crane_or_runway_cross() {
    // PE @ `0x4389f6`.. → `0x438d61`: XBLD 0xDD/0xDE/0xE0 returns 0 (refuse).
    let mut city = City::new_blank("/tmp/sc2000-army-2x2-sp.sc2", "Army2x2Sp").expect("blank");
    for &(x, y) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_zone(x, y, ZONE_MILITARY);
    }
    city.set_building(41, 41, 0xe0);
    assert_eq!(
        pe_army_2x2_place_probe_4389bf(&city, 40, 40, 7),
        PeArmy2x2Probe::Refuse
    );
    city.set_building(41, 41, 0xdd);
    assert_eq!(
        pe_army_2x2_place_probe_4389bf(&city, 40, 40, 7),
        PeArmy2x2Probe::Refuse
    );
}

#[test]
fn pe_army_2x2_place_tile_size_stamps_on_matching_non_military_zone() {
    // Probe requires matching type nibble; PlaceTile-size refuses type 7.
    let mut city = City::new_blank("/tmp/sc2000-army-2x2-place.sc2", "Army2x2Pl").expect("blank");
    city.set_view_rotation(0);
    for &(x, y) in &[(70, 70), (71, 70), (70, 71), (71, 71)] {
        city.set_zone(x, y, 0x01);
        city.set_tile_bits(x, y, TileBits::new(0).with_watered(true));
    }
    city.set_traffic_density(70, 70, 0x2a);
    assert_eq!(
        pe_army_2x2_place_tile_size_438b8e(&mut city, 71, 71, 0xe1, 1),
        PeArmy2x2Result::Accepted { placed: true }
    );
    let corner_bits = pe_choose_and_place_corner_bits(0);
    for &(tx, ty) in &building_footprint_from_origin(70, 70, 2).expect("2x2") {
        assert_eq!(city.building_at(tx, ty), 0xe1);
        let expect = 0x01 | building_zone_bits_for_tile(70, 70, tx, ty, 2, corner_bits);
        assert_eq!(city.zone_at(tx, ty).unwrap().raw_bits(), expect);
        assert!(city.tile_bits_at(tx, ty).unwrap().watered());
    }
    assert_eq!(city.traffic_density_at(70, 70), Some(0x2a));
}

#[test]
fn pe_army_2x2_place_tile_size_military_zone_stamps_and_writes_type() {
    // PE SimulationGrow `0x4376b0` type-7 allow → PlaceTile-size stamps + zone epilogue.
    let mut city = City::new_blank("/tmp/sc2000-army-2x2-mil.sc2", "Army2x2Mil").expect("blank");
    for &(x, y) in &[(72, 72), (73, 72), (72, 73), (73, 73)] {
        city.set_zone(x, y, ZONE_MILITARY | 0x80);
    }
    assert_eq!(
        pe_army_2x2_place_tile_size_438b8e(&mut city, 72, 72, 0xe1, 7),
        PeArmy2x2Result::Accepted { placed: true }
    );
    for &(x, y) in &[(72, 72), (73, 72), (72, 73), (73, 73)] {
        assert_eq!(city.building_at(x, y), 0xe1);
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits() & 0x0f, ZONE_MILITARY);
    }
}

#[test]
fn mac68k_army_2x2_place_tile_size_stamps_on_matching_zone() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-army-2x2-place.sc2", "MacArmy2x2").expect("blank");
    city.set_view_rotation(1);
    for &(x, y) in &[(74, 74), (75, 74), (74, 75), (75, 75)] {
        city.set_zone(x, y, 0x02);
        city.set_tile_bits(x, y, TileBits::new(0).with_watered(true));
    }
    assert_eq!(
        pe_army_2x2_place_tile_size_438b8e(&mut city, 74, 74, 0xe1, 2),
        PeArmy2x2Result::Accepted { placed: true }
    );
    let corner_bits = pe_choose_and_place_corner_bits(1);
    for &(tx, ty) in &building_footprint_from_origin(74, 74, 2).expect("2x2") {
        assert_eq!(city.building_at(tx, ty), 0xe1);
        let expect = 0x02 | building_zone_bits_for_tile(74, 74, tx, ty, 2, corner_bits);
        assert_eq!(city.zone_at(tx, ty).unwrap().raw_bits(), expect);
    }
}

#[test]
fn pe_place_tile_size_refuses_military_zone_type() {
    // PE @ `0x440d49`: XZON type nibble 7 refuses.
    let mut city = City::new_blank("/tmp/sc2000-pts-mil.sc2", "PtsMil").expect("blank");
    city.set_zone(20, 20, ZONE_MILITARY | ZONE_CORNER_ALL);
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0xc8, 1));
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(
        city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_MILITARY | ZONE_CORNER_ALL
    );
}

#[test]
fn pe_place_tile_refuses_out_of_bounds_like_pe() {
    // PE PlaceTile @ `0x441f08`/`0x441f12`: x/y outside `0..0x80` return without write.
    let mut city = City::new_blank("/tmp/sc2000-pt-oob.sc2", "PtOob").expect("blank");
    city.set_building(0, 0, 7);
    pe_place_tile_441f00(&mut city, 0x80, 0, 0xc8);
    pe_place_tile_441f00(&mut city, 0, 0x80, 0xc8);
    pe_place_tile_441f00(&mut city, 0x80, 0x80, 0xc8);
    assert_eq!(city.building_at(0, 0), 7);
    pe_place_tile_441f00(&mut city, 0, 0, 0xc8);
    assert_eq!(city.building_at(0, 0), 0xc8);
}

#[test]
fn pe_place_tile_still_writes_on_military_zone_type_7() {
    // PE PlaceTile @ `0x441f4e`: XZON type 7 takes the military counter path but still
    // writes XBLD (`0x442097`); it does not refuse the stamp.
    let mut city = City::new_blank("/tmp/sc2000-pt-type7.sc2", "PtT7").expect("blank");
    city.set_zone(10, 10, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_traffic_density(10, 10, 0x2a);
    pe_place_tile_441f00(&mut city, 10, 10, 0xdd);
    assert_eq!(city.building_at(10, 10), 0xdd);
    assert_eq!(
        city.zone_at(10, 10).unwrap().raw_bits() & 0x0f,
        ZONE_MILITARY
    );
    assert_eq!(city.traffic_density_at(10, 10), Some(0x2a));
}

#[test]
fn pe_place_tile_size_park_refuses_existing_road() {
    // PE @ `0x440e8f`: placing 0x0D refuses existing XBLD >= 0x0D (road 0x1D).
    let mut city = City::new_blank("/tmp/sc2000-pts-park-road.sc2", "PtsPRoad").expect("blank");
    city.set_building(20, 20, 0x1d);
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0x0d, 1));
    assert_eq!(city.building_at(20, 20), 0x1d);
}

#[test]
fn pe_place_tile_size_park_refuses_existing_power_line() {
    // Park-only gate @ `0x440e8f`: existing 0x0E..=0x1C passes general XBLD checks but still refuses.
    let mut city = City::new_blank("/tmp/sc2000-pts-park-pwr.sc2", "PtsPPwr").expect("blank");
    city.set_building(20, 20, 0x0e);
    // Non-park PlaceTile-size may overwrite mid-range XBLD.
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 0xc8, 1));
    assert_eq!(city.building_at(20, 20), 0xc8);
    city.set_building(20, 20, 0x0e);
    city.set_zone(20, 20, 0);
    city.set_tile_bits(20, 20, TileBits::new(0));
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0x0d, 1));
    assert_eq!(city.building_at(20, 20), 0x0e);
}

#[test]
fn place_zone_rect_dezone_place_tile_clears_rubble() {
    // PE @ `0x4499fb`/`0x449a26`: dezone PlaceTile(0) for XBLD 1..=4; XZON unchanged.
    let mut city = City::new_blank("/tmp/sc2000-dezone-rubble.sc2", "DezoneR").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(40, 40, 4);
    city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_traffic_density(40, 40, 0x33);
    let mut state = test_state(city);
    assert_eq!(state.place_zone_rect((40, 40), (40, 40), 0), 1);
    assert_eq!(state.city.building_at(40, 40), 0);
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x33));
}

#[test]
fn mac68k_place_zone_rect_dezone_place_tile_clears_rubble() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-dezone-rubble.sc2", "MacDezoneR").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(40, 40, 1);
    city.set_zone(40, 40, ZONE_COMMERCIAL_DENSE | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_zone_rect((40, 40), (40, 40), 0), 1);
    assert_eq!(state.city.building_at(40, 40), 0);
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.raw_bits()),
        Some(ZONE_COMMERCIAL_DENSE | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_zone_rect_dezone_skips_xzon_write_on_vacant() {
    // PE @ `0x449a26`: type ≤0 skips XZON nibble replace.
    let mut city = City::new_blank("/tmp/sc2000-dezone-xzon.sc2", "DezoneXzon").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(40, 40, 0);
    city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    assert_eq!(state.place_zone_rect((40, 40), (40, 40), 0), 1);
    assert_eq!(state.city.building_at(40, 40), 0);
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_zone_rect_rezone_keeps_existing_corner_bits() {
    // PE @ 0x449a39: type-nibble replace preserves XZON corner bits.
    let mut city = City::new_blank("/tmp/sc2000-zone-corners.sc2", "ZoneCorn").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut state = test_state(city);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_COMMERCIAL_DENSE),
        1
    );
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.raw_bits()),
        Some(ZONE_COMMERCIAL_DENSE | 0x80)
    );
}

#[test]
fn mac68k_place_zone_rect_rezone_keeps_existing_corner_bits() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-zone-corners.sc2", "MacZoneCorn").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(40, 40, ZONE_INDUSTRIAL_LIGHT | 0x40);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        1
    );
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x40)
    );
}

#[test]
fn place_zone_rect_place_tile_clears_rubble_1_to_4() {
    // PE @ `0x4499fb`: paint (`[0x4c7d5c]!=0`) skips PlaceTile(0); rubble stays.
    let mut city = City::new_blank("/tmp/sc2000-zone-rubble.sc2", "ZoneRubble").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(40, 40, 3);
    city.set_text_overlay(40, 40, 0x2a);
    city.set_traffic_density(40, 40, 0x55);
    city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        1
    );
    assert_eq!(state.city.building_at(40, 40), 3);
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0x2a));
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x55));
    let bits = state.city.tile_bits_at(40, 40).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn mac68k_place_zone_rect_place_tile_clears_rubble_1_to_4() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-zone-rubble.sc2", "MacZoneRubble").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(40, 40, 2);
    city.set_text_overlay(40, 40, 0x2a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_COMMERCIAL_DENSE),
        1
    );
    assert_eq!(state.city.building_at(40, 40), 2);
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0x2a));
}

#[test]
fn place_zone_rect_still_refuses_radioactivity() {
    let mut city = City::new_blank("/tmp/sc2000-zone-rad.sc2", "ZoneRad").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(40, 40, 5);
    let mut state = test_state(city);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
    assert_eq!(state.city.building_at(40, 40), 5);
}

#[test]
fn place_zone_rect_allows_power_line_xbld_keeps_building() {
    // PE @ `0x4499d0`–`0x4499e7`: XBLD `0x0e..=0x1c` eligible; paint is XZON-only.
    let mut city = City::new_blank("/tmp/sc2000-zone-pwrline.sc2", "ZonePwrLn").expect("blank");
    city.set_city_funds(20_000);
    city.set_terrain(40, 40, 0);
    city.set_building(40, 40, 0x0e);
    let mut state = test_state(city);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        1
    );
    assert_eq!(state.city.building_at(40, 40), 0x0e);
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn place_zone_rect_still_refuses_road_and_small_park() {
    let mut city = City::new_blank("/tmp/sc2000-zone-road-park.sc2", "ZoneRdPk").expect("blank");
    city.set_city_funds(20_000);
    city.set_terrain(40, 40, 0);
    city.set_building(40, 40, 0x1d);
    let mut state = test_state(city);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
    state.city.set_building(40, 40, SMALL_PARK_BUILDING_ID);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
}

#[test]
fn place_zone_rect_refuses_military_zone_and_small_park() {
    let mut city = City::new_blank("/tmp/sc2000-zone-mil.sc2", "ZoneMil").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(40, 40, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_building(41, 40, SMALL_PARK_BUILDING_ID);
    city.set_building(42, 40, 0x1d); // road
    let mut state = test_state(city);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
    assert_eq!(
        state.place_zone_rect((41, 40), (41, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
    assert_eq!(
        state.place_zone_rect((42, 40), (42, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
}

#[test]
fn mac68k_place_zone_rect_refuses_military_zone_and_small_park() {
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-mil.sc2", "MacZoneMil").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(40, 40, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_building(41, 40, SMALL_PARK_BUILDING_ID);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
    assert_eq!(
        state.place_zone_rect((41, 40), (41, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
}

#[test]
fn braun_llama_dome_uses_xbld_255_as_real_building() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mask = reward_mask_for_tool(EditTool::BraunLlamaDome).expect("reward mask");
    city.set_rewards_available_mask(mask);
    let mut state = test_state(city);
    state.tool = EditTool::BraunLlamaDome;

    assert!(state.apply_tool_at_tile(20, 20));

    assert_eq!(building_sprite_id(255), Some(1255));
    assert_eq!(
        xbld_sprite_info(255, 0).expect("sprite info").sprite_id,
        1255
    );
    assert_eq!(surface_building_query_label(255), Some("Braun Llama Dome"));
    assert_eq!(state.city.building_at(20, 20), 255);
    assert_eq!(state.city.building_at(23, 23), 255);
    let footprint = state.building_footprint_at(22, 22).expect("footprint");
    assert_eq!(footprint.len(), 16);
    assert_eq!(
        footprint
            .iter()
            .filter(|&&(x, y)| state.city.building_should_draw_at(x, y))
            .count(),
        1
    );
    assert!(!state.can_place_building_tile(20, 20));
    assert_eq!(state.city.rewards_available_mask() & mask, 0);
}

#[test]
fn successful_one_shot_reward_placement_consumes_only_its_grant() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_rewards_available_mask(0b1_1111);
    let mut state = test_state(city);
    state.tool = EditTool::MayorHouse;

    assert!(state.apply_tool_at_tile(20, 20));

    assert_eq!(state.city.building_at(20, 20), 243);
    assert_eq!(state.city.rewards_available_mask(), 0b1_1110);
    assert_eq!(state.tool, EditTool::Query);
}

#[test]
fn stale_unavailable_reward_tools_cannot_preview_or_place() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    for tool in [EditTool::MayorHouse, EditTool::ArcologyForest] {
        state.tool = tool;
        assert!(
            !state.ui_action_enabled(UiAction::SelectTool(tool)),
            "{tool:?}"
        );
        assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
        assert!(!state.apply_tool_at_tile(20, 20), "{tool:?}");
        assert_eq!(state.city.building_at(20, 20), 0, "{tool:?}");
    }
}

#[test]
fn future_technology_tools_cannot_preview_or_place() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(50_000);
    city.set_technology_discovery_year(Technology::GasPower, 1950);
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantGas;

    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::PowerPlantGas)));
    assert_eq!(state.tool_placement_at(20, 20), None);
    assert!(!state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn mac_technology_tools_wait_for_the_native_monthly_timer_scan_on_both_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-tech-unlock.sc2", "Mac Tech").expect("blank city");
        city.set_technology_discovery_year(Technology::WaterTreatment, 1900);
        city.kind = kind;
        city.set_mac_state_misc_u32_at(Technology::WaterTreatment.misc_offset(), 1900);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;

        assert!(
            !state.ui_action_enabled(UiAction::SelectTool(EditTool::WaterTreatment)),
            "calendar arrival alone must not clear the Mac deadline for {kind:?}"
        );

        // A nonzero timer gate leaves the due date in place, so the command
        // remains unavailable even though the displayed year is already 1900.
        let mut skipped_draws = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7].into_iter();
        super::super::rci::mac_consume_phase21_pre_weather_event_draws(
            &mut state.city,
            1900,
            || skipped_draws.next().expect("exact Mac QuickDraw sequence"),
        );
        assert!(
            !state.ui_action_enabled(UiAction::SelectTool(EditTool::WaterTreatment)),
            "a skipped timer scan must keep the command unavailable for {kind:?}"
        );

        // The successful 1-in-8 timer scan clears the first due timer; the
        // shared technology cache then exposes the corresponding tool.
        let mut due_draws = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].into_iter();
        assert_eq!(
            super::super::rci::mac_consume_phase21_pre_weather_event_draws(
                &mut state.city,
                1900,
                || due_draws.next().expect("exact Mac QuickDraw sequence"),
            ),
            Some(10),
            "the native scan should clear the Water Treatment timer for {kind:?}"
        );
        assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::WaterTreatment)));
    }
}

#[test]
fn mac_phase21_unlocks_native_toolbar_tools_from_matching_dates_on_both_layouts() {
    let tool_groups = [
        (Technology::GasPower, &[EditTool::PowerPlantGas][..]),
        (Technology::NuclearPower, &[EditTool::PowerPlantNuclear][..]),
        (Technology::SolarPower, &[EditTool::PowerPlantSolar][..]),
        (Technology::WindPower, &[EditTool::PowerPlantWind][..]),
        (
            Technology::MicrowavePower,
            &[EditTool::PowerPlantMicrowave][..],
        ),
        (Technology::FusionPower, &[EditTool::PowerPlantFusion][..]),
        (Technology::Airport, &[EditTool::Airport][..]),
        (
            Technology::Highways,
            &[EditTool::Highway, EditTool::Onramp][..],
        ),
        (Technology::Buses, &[EditTool::BusDepot][..]),
        (
            Technology::Subways,
            &[EditTool::Subway, EditTool::SubStation, EditTool::SubToRail][..],
        ),
        (Technology::WaterTreatment, &[EditTool::WaterTreatment][..]),
        (Technology::Desalinization, &[EditTool::Desalinization][..]),
    ];

    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (technology, tools) in tool_groups {
            let mut city = City::new_blank("/tmp/sc2000-mac-tech-map.sc2", "Mac Tech Map")
                .expect("blank city");
            city.kind = kind;
            for technology in Technology::ALL {
                city.set_technology_discovery_year(technology, 0);
                city.set_mac_state_misc_u32_at(technology.misc_offset(), 0);
            }
            // Arcology commands also require their separate reward bit;
            // keep that independent gate open so this test isolates tech dates.
            city.set_rewards_available_mask(1 << 4);
            city.set_technology_discovery_year(technology, 1900);
            city.set_mac_state_misc_u32_at(technology.misc_offset(), 1900);
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;

            for tool in tools {
                assert_eq!(
                    super::super::tool_controller::technology_for_tool(*tool),
                    Some(technology),
                    "native technology mapping for {tool:?} on {kind:?}"
                );
                assert!(
                    !state.ui_action_enabled(UiAction::SelectTool(*tool)),
                    "{tool:?} must remain gated while {technology:?} is due on {kind:?}"
                );
            }

            // A successful native 1-in-8 scan clears the first due timer.
            assert_eq!(
                super::super::rci::mac_consume_phase21_pre_weather_event_draws(
                    &mut state.city,
                    1900,
                    || 0,
                ),
                Some(
                    Technology::ALL
                        .iter()
                        .position(|candidate| *candidate == technology)
                        .unwrap()
                ),
                "the scan must clear the matching timer for {technology:?} on {kind:?}"
            );
            for tool in tools {
                assert!(
                    state.ui_action_enabled(UiAction::SelectTool(*tool)),
                    "{tool:?} must unlock once {technology:?} is cleared on {kind:?}"
                );
            }
        }
    }
}

#[test]
fn invented_or_missing_technology_fields_keep_tools_available() {
    for discovery_year in [None, Some(0), Some(1900)] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(50_000);
        if let Some(year) = discovery_year {
            city.set_technology_discovery_year(Technology::GasPower, year);
        }
        let mut state = test_state(city);
        state.tool = EditTool::PowerPlantGas;

        assert!(
            state.ui_action_enabled(UiAction::SelectTool(EditTool::PowerPlantGas)),
            "discovery year {discovery_year:?}"
        );
        assert!(
            state.tool_placement_at(20, 20).is_some(),
            "discovery year {discovery_year:?}"
        );
    }
}

#[test]
fn future_technology_lock_is_specific_to_matching_tools() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_technology_discovery_year(Technology::Highways, 1950);
    let state = test_state(city);

    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::Road)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::Highway)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::Onramp)));
}

#[test]
fn failed_reward_placement_preserves_grant_and_selected_tool() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_rewards_available_mask(1 << 1);
    city.set_building(20, 20, 112);
    let mut state = test_state(city);
    state.tool = EditTool::CityHall;

    assert!(!state.apply_tool_at_tile(20, 20));

    assert_eq!(state.city.rewards_available_mask(), 1 << 1);
    assert_eq!(state.tool, EditTool::CityHall);
}

#[test]
fn arcology_placement_keeps_shared_grant_and_selected_type() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);
    state.tool = EditTool::ArcologyForest;

    assert!(state.apply_tool_at_tile(20, 20));

    assert_eq!(state.city.building_at(20, 20), 252);
    assert_eq!(state.city.rewards_available_mask(), 1 << 4);
    assert_eq!(state.tool, EditTool::ArcologyForest);
}

#[test]
fn mac68k_sign_tool_preserves_xtrf_like_pe_sign_write() {
    // Shared PE sign writes XTXT/XLAB only; Mac68k must not invent an XTRF clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-sign-xtrf.sc2", "MacSignX").expect("blank");
    city.set_traffic_density(10, 10, 0x5a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Sign;
    assert!(state.place_sign_at(10, 10));
    assert_eq!(state.city.traffic_density_at(10, 10), Some(0x5a));
}

#[test]
fn mac68k_pe_fun_00460e10_ignite_fa_clears_non_network_marker_460f71() {
    let mut city = City::new_blank("/tmp/sc2000-mac-ignite-fa.sc2", "MacIgniteFa").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_text_overlay(20, 20, 0xfa);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        citygame_fa_non_network_neighbor_marker_count(&state.city),
        1
    );
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_fun_00460e10_ignite(
        &mut state.city,
        Some(&mut crt),
        &mut lfsr,
        20,
        20,
        1,
    ));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xff));
    assert_eq!(
        citygame_fa_non_network_neighbor_marker_count(&state.city),
        0
    );
}

#[test]
fn pe_maptool_surface_destroy_building_on_xbld_ge_0x0d() {
    // PE @ `0x443453`/`0x443461`: selector-1 scatter (2 draws), then CRT rubble.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-db.sc2", "MapSurfDb").expect("blank");
    city.set_building(33, 33, 0x10);
    city.set_text_overlay(33, 33, 0x40);
    let mut crt = Win95CrtRand::new(0x4434_6101);
    let mut preview = Win95CrtRand::new(0x4434_6101);
    let _ = preview.random_u15(); // selector-1 render scatter @ `0x459DEB`
    let _ = preview.random_u15(); // selector-1 render scatter @ `0x459E19`
    let expected = preview.rubble_tile_id();
    assert!(pe_maptool_surface_destroy_building_443461(
        &mut city,
        Some(&mut crt),
        33,
        33
    ));
    assert_eq!(city.building_at(33, 33), expected);
    assert_eq!(city.text_overlay_at(33, 33), Some(0));
    assert_eq!(crt.seed(), preview.seed());
    assert!(!pe_maptool_surface_destroy_building_443461(
        &mut city, None, 33, 33
    ));
}

#[test]
fn pe_maptool_surface_destroy_building_elevated_arm() {
    // PE @ `0x443461`: DestroyBuilding mode 1 — elevated `0x3f..=0x42` arm, not
    // DestroyStructure early-out; no CRT rubble.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-elev.sc2", "MapSurfEl").expect("blank");
    city.set_building(33, 33, 0x3f);
    city.set_zone(33, 33, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    city.set_tunnel_levels(33, 33, 0x1f);
    let mut crt = Win95CrtRand::new(0x4434_6101);
    let seed_before = crt.seed();
    assert!(pe_maptool_surface_destroy_building_443461(
        &mut city,
        Some(&mut crt),
        33,
        33
    ));
    assert_eq!(city.building_at(33, 33), 0);
    assert_eq!(
        city.zone_at(33, 33).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0
    );
    assert_eq!(city.tunnel_levels_at(33, 33), 0);
    assert_eq!(crt.seed(), seed_before, "elevated arm does not consume CRT");
}

#[test]
fn pe_maptool_surface_placetile_zero_on_xbld_below_0x0d() {
    // PE @ `0x4434d2`: XBLD < 0x0D → PlaceTile(0); leave XTXT/XTRF/watered.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-pt0.sc2", "MapSurfPt0").expect("blank");
    city.set_building(34, 34, 8);
    city.set_text_overlay(34, 34, 0x2a);
    city.set_tile_bits(34, 34, TileBits::new(0).with_watered(true));
    city.set_traffic_density(34, 34, 0x33);
    assert!(pe_maptool_surface_placetile_zero_4434d2(&mut city, 34, 34));
    assert_eq!(city.building_at(34, 34), 0);
    assert_eq!(city.text_overlay_at(34, 34), Some(0x2a));
    assert!(city.tile_bits_at(34, 34).unwrap().watered());
    assert_eq!(city.traffic_density_at(34, 34), Some(0x33));
    city.set_building(34, 34, 0x10);
    assert!(!pe_maptool_surface_placetile_zero_4434d2(&mut city, 34, 34));
    assert_eq!(city.building_at(34, 34), 0x10);
}

#[test]
fn citytool_substation_skips_fun_00443270_epilogue_neighbors() {
    // PE CityTool SubStation @ `0x44088d` never runs MapTool `0x44378e` reshape.
    let mut city = City::new_blank("/tmp/sc2000-sub-no-epi.sc2", "SubNoEpi").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 21, 0x1d); // road south
    city.set_building(19, 20, 0x1c); // power west
    city.set_tile_bits(19, 20, TileBits::new(0).with_powerable(true));
    let mut state = test_state(city);
    state.tool = EditTool::SubStation;
    assert!(state.place_building_at(20, 20, 233) > 0);
    assert_eq!(state.city.building_at(20, 20), 233);
    assert_eq!(
        state.city.building_at(20, 21),
        0x1d,
        "road neighbor unchanged"
    );
    assert_eq!(
        state.city.building_at(19, 20),
        0x1c,
        "power neighbor unchanged"
    );
}

#[test]
fn citytool_substation_place_tile_size_refuses_xbld5() {
    // PE PlaceTile-size @ `0x440d25`: XBLD == 5 refuse (not MapTool `0x4432fd`).
    let mut city = City::new_blank("/tmp/sc2000-sub-surf-5.sc2", "SubSurf5").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 5);
    let mut state = test_state(city);
    state.show_underground = false;
    state.tool = EditTool::SubStation;
    let funds = state.city.city_funds();
    assert_eq!(state.place_building_at(20, 20, 233), 0);
    assert_eq!(state.city.building_at(20, 20), 5);
    assert_eq!(state.city.city_funds(), funds);
}

#[test]
fn pe_maptool_refuse_xzon_type7() {
    // PE @ `0x4432d2`: XZON type nibble 7 → refuse before DisplayLayer arms.
    let mut city = City::new_blank("/tmp/sc2000-map-type7.sc2", "MapType7").expect("blank");
    city.set_zone(41, 41, ZONE_MILITARY | 0x80);
    assert!(pe_maptool_refuse_xzon_type7_4432d2(&city, 41, 41));
    city.set_zone(41, 41, ZONE_RESIDENTIAL_DENSE | 0x80);
    assert!(!pe_maptool_refuse_xzon_type7_4432d2(&city, 41, 41));
}

#[test]
fn maptool_sub_to_rail_refuses_xzon_type7() {
    let mut city = City::new_blank("/tmp/sc2000-str-type7.sc2", "StrType7").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(20, 20, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_building(21, 20, 0x2c);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    let funds = state.city.city_funds();
    assert!(!state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.city_funds(), funds);
}

#[test]
fn pe_maptool_surface_refuse_xbld5() {
    // PE @ `0x4432fd`: DisplayLayer clear + XBLD==5 → refuse.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-5.sc2", "MapSurf5").expect("blank");
    city.set_building(39, 39, 5);
    assert!(pe_maptool_surface_refuse_xbld5_4432fd(&city, 39, 39));
    city.set_building(39, 39, 6);
    assert!(!pe_maptool_surface_refuse_xbld5_4432fd(&city, 39, 39));
}

#[test]
fn pe_maptool_underground_empty_refuse_without_xund_tunnel_or_piped() {
    // PE @ `0x443350`: XUND==0, tunnel≠1, piped clear → refuse (DisplayLayer set).
    let mut city = City::new_blank("/tmp/sc2000-map-und-empty.sc2", "MapUndEmp").expect("blank");
    city.set_building(38, 38, 0x10);
    city.set_underground(38, 38, 0);
    city.set_tunnel_levels(38, 38, 0);
    assert!(pe_maptool_underground_empty_refuse_443350(&city, 38, 38));
    city.set_tile_bits(38, 38, TileBits::new(0).with_piped(true));
    assert!(!pe_maptool_underground_empty_refuse_443350(&city, 38, 38));
    city.set_tile_bits(38, 38, TileBits::new(0));
    city.set_tunnel_levels(38, 38, 1);
    assert!(!pe_maptool_underground_empty_refuse_443350(&city, 38, 38));
    city.set_tunnel_levels(38, 38, 0);
    city.set_underground(38, 38, 0x05);
    assert!(!pe_maptool_underground_empty_refuse_443350(&city, 38, 38));
}

#[test]
fn pe_maptool_underground_clear_xund_without_destroy_when_no_gate() {
    // PE @ `0x4433ce`–`0x44342f`: underground view always PlaceXUND(0); DestroyBuilding
    // only when tunnel==1 or XUND==0x23.
    let mut city = City::new_blank("/tmp/sc2000-map-und-xund.sc2", "MapUndXund").expect("blank");
    city.set_building(35, 35, 0x20);
    city.set_underground(35, 35, 0x05);
    city.set_tunnel_levels(35, 35, 0);
    assert!(pe_maptool_underground_clear_subway_443427(
        &mut city, None, 35, 35, true
    ));
    assert_eq!(
        city.building_at(35, 35),
        0x20,
        "surface XBLD stays without destroy gate"
    );
    assert_eq!(city.underground_at(35, 35), 0);
}

#[test]
fn pe_maptool_underground_clear_piped_bit_when_xbld_below_0x70() {
    // PE @ `0x4433fa`: underground path clears XBIT piped before the destroy gate.
    let mut city = City::new_blank("/tmp/sc2000-map-und-piped.sc2", "MapUndPipe").expect("blank");
    city.set_building(36, 36, 0x6c);
    city.set_underground(36, 36, UNDER_TILE_PIPES_LR);
    city.set_tunnel_levels(36, 36, 0);
    city.set_tile_bits(36, 36, TileBits::new(0).with_piped(true).with_watered(true));
    assert!(pe_maptool_underground_clear_subway_443427(
        &mut city, None, 36, 36, true
    ));
    assert_eq!(city.building_at(36, 36), 0x6c);
    assert_eq!(city.underground_at(36, 36), 0);
    let bits = city.tile_bits_at(36, 36).unwrap();
    assert!(!bits.piped());
    assert!(bits.watered());
}

#[test]
fn pe_maptool_underground_clear_skips_piped_bit_when_xbld_ge_0x70() {
    let mut city = City::new_blank("/tmp/sc2000-map-und-nopipe.sc2", "MapUndNoP").expect("blank");
    city.set_building(37, 37, 0xe9);
    city.set_underground(37, 37, UNDER_TILE_PIPES_LR);
    city.set_tunnel_levels(37, 37, 0);
    city.set_tile_bits(37, 37, TileBits::new(0).with_piped(true));
    assert!(pe_maptool_underground_clear_subway_443427(
        &mut city, None, 37, 37, true
    ));
    assert!(city.tile_bits_at(37, 37).unwrap().piped());
    assert_eq!(city.underground_at(37, 37), 0);
    assert_eq!(city.building_at(37, 37), 0xe9);
}

#[test]
fn clear_building_underground_side_effects_surface_clears_non_grey_xund() {
    // Wired PE surface clear: bulldoze SubStation PlaceXUND(0) without 0x23 gate.
    let mut city = City::new_blank("/tmp/sc2000-clear-side-xu.sc2", "ClrSideXu").expect("blank");
    city.set_building(35, 35, 233);
    city.set_underground(35, 35, UNDER_TILE_PIPES_LR);
    let mut state = test_state(city);
    state.clear_building_underground_side_effects(35, 35, 233);
    assert_eq!(state.city.underground_at(35, 35), 0);
}

#[test]
fn pe_place_xund_writes_on_military_zone_without_refuse() {
    // PE @ `0x44238e`/`0x442407`: zone type 7 skips pipe counter but still writes XUND.
    let mut city = City::new_blank("/tmp/sc2000-xund-mil.sc2", "XundMil").expect("blank");
    city.set_zone(40, 40, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_underground(40, 40, UNDER_TILE_PIPES_LR);
    pe_place_xund_442370(&mut city, 40, 40, 0);
    assert_eq!(city.underground_at(40, 40), 0);
    assert_eq!(
        city.zone_at(40, 40).unwrap().raw_bits(),
        ZONE_MILITARY | ZONE_CORNER_ALL
    );
}

#[test]
fn pe_place_pipes_xtxt_reshape_places_table_plus_0x10() {
    // PE @ `0x44b961`: XTXT nibble 1 → PlaceXUND(0x2+0x10).
    let mut city = City::new_blank("/tmp/sc2000-pipe-xtxt.sc2", "PipeXtxt").expect("blank");
    city.set_underground(42, 42, UNDER_TILE_PIPES_LR);
    city.set_text_overlay(42, 42, 0x01);
    city.set_traffic_density(42, 42, 0x55);
    city.set_tile_bits(42, 42, TileBits::new(0).with_watered(true));
    assert!(pe_place_pipes_xtxt_reshape_44b961(&mut city, 42, 42));
    assert_eq!(city.underground_at(42, 42), 0x12);
    assert_eq!(city.traffic_density_at(42, 42), Some(0x55));
    assert!(city.tile_bits_at(42, 42).unwrap().watered());
}

#[test]
fn pe_place_pipes_xtxt_reshape_skips_ungated_xtxt() {
    let mut city = City::new_blank("/tmp/sc2000-pipe-xtxt0.sc2", "PipeXtxt0").expect("blank");
    city.set_underground(43, 43, UNDER_TILE_PIPES_LR);
    city.set_text_overlay(43, 43, 0);
    assert!(!pe_place_pipes_xtxt_reshape_44b961(&mut city, 43, 43));
    assert_eq!(city.underground_at(43, 43), UNDER_TILE_PIPES_LR);
}

#[test]
fn refresh_underground_tile_pipes_uses_xtxt_early_out() {
    let mut city = City::new_blank("/tmp/sc2000-pipe-refresh-xtxt.sc2", "PipeRefX").expect("blank");
    city.set_underground(44, 44, UNDER_TILE_PIPES_LR);
    city.set_text_overlay(44, 44, 0x04);
    let mut state = test_state(city);
    state.refresh_underground_tile(44, 44, UndergroundKind::Pipes);
    assert_eq!(state.city.underground_at(44, 44), 0x15); // 5+0x10
}

#[test]
fn pe_place_pipes_mask_tile_matches_rust_offsets() {
    // PE `0x4e7c40`+0x10 agrees with underground_tile_for_mask for masks 1..=15.
    // Mask 0 is PE-forced to index 0xf (tile 0x1e); Rust legacy map used offset 0.
    for mask in 1u8..16 {
        assert_eq!(
            pe_place_pipes_mask_tile_44ba9a(mask),
            underground_tile_for_mask(UndergroundKind::Pipes, mask),
            "mask {mask:#x}"
        );
    }
}

#[test]
fn pe_place_pipes_mask_tile_zero_mask_uses_index_0xf() {
    // PE @ `0x44ba84`: mask 0 → si=0xf → offset 0xe → tile 0x1e.
    assert_eq!(pe_place_pipes_mask_tile_44ba9a(0), 0x1e);
    assert_ne!(
        underground_tile_for_mask(UndergroundKind::Pipes, 0),
        0x1e,
        "legacy mask-0 path differed; pipes refresh now uses PE table"
    );
}

#[test]
fn pe_network_xtxt_fa_edge_mask_bits_match_pe() {
    // PE @ `0x44b0ac` / `0x44b665`: only when XTXT == 0xfa.
    assert_eq!(
        pe_network_xtxt_fa_edge_mask_bits_44b0ac(0, 40, 0xfa),
        NETWORK_LEFT
    );
    assert_eq!(
        pe_network_xtxt_fa_edge_mask_bits_44b0ac(CITY_WIDTH - 1, 40, 0xfa),
        NETWORK_RIGHT
    );
    assert_eq!(
        pe_network_xtxt_fa_edge_mask_bits_44b0ac(40, 0, 0xfa),
        NETWORK_TOP
    );
    assert_eq!(
        pe_network_xtxt_fa_edge_mask_bits_44b0ac(40, CITY_HEIGHT - 1, 0xfa),
        NETWORK_BOTTOM
    );
    assert_eq!(
        pe_network_xtxt_fa_edge_mask_bits_44b0ac(0, 0, 0xfa),
        NETWORK_LEFT | NETWORK_TOP
    );
    assert_eq!(pe_network_xtxt_fa_edge_mask_bits_44b0ac(0, 40, 0), 0);
    assert_eq!(pe_network_xtxt_fa_edge_mask_bits_44b0ac(40, 40, 0xfa), 0);
}

#[test]
fn pe_place_roads_rails_crossover_tables_match_pe() {
    // PE PlaceRoads `0x44ae24` / PlaceRails `0x44b2f4`.
    assert_eq!(
        pe_place_roads_crossover_from_existing_44ae24(0x0e),
        Some(0x44)
    );
    assert_eq!(
        pe_place_roads_crossover_from_existing_44ae24(0x0f),
        Some(0x43)
    );
    assert_eq!(
        pe_place_roads_crossover_from_existing_44ae24(0x2c),
        Some(0x46)
    );
    assert_eq!(
        pe_place_roads_crossover_from_existing_44ae24(0x2d),
        Some(0x45)
    );
    assert_eq!(
        pe_place_roads_crossover_from_existing_44ae24(0x49),
        Some(0x4b)
    );
    assert_eq!(
        pe_place_roads_crossover_from_existing_44ae24(0x4a),
        Some(0x4c)
    );
    assert_eq!(pe_place_roads_crossover_from_existing_44ae24(0x1d), None);
    assert_eq!(
        pe_place_rails_crossover_from_existing_44b2f4(0x0e),
        Some(0x48)
    );
    assert_eq!(
        pe_place_rails_crossover_from_existing_44b2f4(0x0f),
        Some(0x47)
    );
    assert_eq!(
        pe_place_rails_crossover_from_existing_44b2f4(0x1d),
        Some(0x45)
    );
    assert_eq!(
        pe_place_rails_crossover_from_existing_44b2f4(0x1e),
        Some(0x46)
    );
    assert_eq!(
        pe_place_rails_crossover_from_existing_44b2f4(0x49),
        Some(0x4d)
    );
    assert_eq!(
        pe_place_rails_crossover_from_existing_44b2f4(0x4a),
        Some(0x4e)
    );
    assert_eq!(pe_place_rails_crossover_from_existing_44b2f4(0x2c), None);
}

#[test]
fn pe_place_power_crossover_table_match_pe() {
    // PE PlacePowerLines `0x44a9e8`.
    assert_eq!(
        pe_place_power_crossover_from_existing_44a9e8(0x1d),
        Some(0x43)
    );
    assert_eq!(
        pe_place_power_crossover_from_existing_44a9e8(0x1e),
        Some(0x44)
    );
    assert_eq!(
        pe_place_power_crossover_from_existing_44a9e8(0x2c),
        Some(0x47)
    );
    assert_eq!(
        pe_place_power_crossover_from_existing_44a9e8(0x2d),
        Some(0x48)
    );
    assert_eq!(
        pe_place_power_crossover_from_existing_44a9e8(0x49),
        Some(0x4f)
    );
    assert_eq!(
        pe_place_power_crossover_from_existing_44a9e8(0x4a),
        Some(0x50)
    );
    assert_eq!(pe_place_power_crossover_from_existing_44a9e8(0x0e), None);
}

#[test]
fn place_road_on_highway_uses_pe_crossover_table() {
    // PE PlaceRoads: highway 0x49/0x4a → road×highway crossover 0x4b/0x4c.
    let mut city =
        City::new_blank("/tmp/sc2000-road-hwy-cross.sc2", "RoadHwyCross").expect("blank");
    city.set_building(30, 30, 0x49);
    city.set_building(31, 30, 0x4a);
    let mut state = test_state(city);
    assert_eq!(
        pe_place_network_crossover_from_existing(NetworkKind::Road, 0x49),
        Some(0x4b)
    );
    assert_eq!(state.place_network_path(&[(30, 30)], NetworkKind::Road), 1);
    assert_eq!(
        state.city.building_at(30, 30),
        0x4b,
        "after place on 0x49 got {:#x}",
        state.city.building_at(30, 30)
    );
    assert_eq!(state.place_network_path(&[(31, 30)], NetworkKind::Road), 1);
    assert_eq!(
        state.city.building_at(31, 30),
        0x4c,
        "after place on 0x4a got {:#x}",
        state.city.building_at(31, 30)
    );
}

#[test]
fn place_road_on_power_and_rail_uses_pe_crossover_table() {
    // PE PlaceRoads `0x44ae24`: power 0x0e/0x0f and rail 0x2c/0x2d underlays.
    let mut city =
        City::new_blank("/tmp/sc2000-road-pwr-rail-cross.sc2", "RoadPwrRail").expect("blank");
    city.set_building(50, 50, 0x0e);
    city.set_building(51, 50, 0x0f);
    city.set_building(52, 50, 0x2c);
    city.set_building(53, 50, 0x2d);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(50, 50)], NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(50, 50), 0x44);
    assert_eq!(state.place_network_path(&[(51, 50)], NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(51, 50), 0x43);
    assert_eq!(state.place_network_path(&[(52, 50)], NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(52, 50), 0x46);
    assert_eq!(state.place_network_path(&[(53, 50)], NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(53, 50), 0x45);
}

#[test]
fn place_rail_on_power_and_road_uses_pe_crossover_table() {
    // PE PlaceRails `0x44b2f4`: power 0x0e/0x0f and road 0x1d/0x1e underlays.
    let mut city =
        City::new_blank("/tmp/sc2000-rail-pwr-road-cross.sc2", "RailPwrRoad").expect("blank");
    city.set_building(54, 54, 0x0e);
    city.set_building(55, 54, 0x0f);
    city.set_building(56, 54, 0x1d);
    city.set_building(57, 54, 0x1e);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(54, 54)], NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(54, 54), 0x48);
    assert_eq!(state.place_network_path(&[(55, 54)], NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(55, 54), 0x47);
    assert_eq!(state.place_network_path(&[(56, 54)], NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(56, 54), 0x45);
    assert_eq!(state.place_network_path(&[(57, 54)], NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(57, 54), 0x46);
}

#[test]
fn mac68k_place_road_rail_on_power_underlay_uses_pe_crossover_table() {
    // Shared PlaceRoads/Rails power underlay crossovers (`0x44ae24`/`0x44b2f4`).
    let mut city = City::new_blank("/tmp/sc2000-pwr-under-mac.sc2", "PwrUnderMac").expect("blank");
    city.set_building(58, 58, 0x0e);
    city.set_building(59, 58, 0x0f);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_path(&[(58, 58)], NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(58, 58), 0x44);
    assert_eq!(state.place_network_path(&[(59, 58)], NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(59, 58), 0x47);
}

#[test]
fn place_power_on_road_rail_highway_uses_pe_crossover_table() {
    // PE PlacePowerLines: road/rail/highway → power×network crossovers.
    let mut city = City::new_blank("/tmp/sc2000-power-cross.sc2", "PowerCross").expect("blank");
    city.set_building(40, 40, 0x1d);
    city.set_building(41, 40, 0x2c);
    city.set_building(42, 40, 0x49);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(40, 40)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(40, 40), 0x43);
    assert!(state.city.tile_bits_at(40, 40).unwrap().powerable());
    assert_eq!(state.place_network_path(&[(41, 40)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(41, 40), 0x47);
    assert_eq!(state.place_network_path(&[(42, 40)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(42, 40), 0x4f);
}

#[test]
fn mac68k_place_power_on_road_rail_highway_uses_pe_crossover_table() {
    // Shared PlacePowerLines existing→crossover table (`0x44a9e8`).
    let mut city =
        City::new_blank("/tmp/sc2000-power-cross-mac.sc2", "PowerCrossMac").expect("blank");
    city.set_building(44, 44, 0x1e);
    city.set_building(45, 44, 0x2d);
    city.set_building(46, 44, 0x4a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_path(&[(44, 44)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(44, 44), 0x44);
    assert_eq!(state.place_network_path(&[(45, 44)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(45, 44), 0x48);
    assert_eq!(state.place_network_path(&[(46, 44)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(46, 44), 0x50);
}

#[test]
fn place_rail_on_highway_uses_pe_crossover_table() {
    let mut city =
        City::new_blank("/tmp/sc2000-rail-hwy-cross.sc2", "RailHwyCross").expect("blank");
    city.set_building(32, 32, 0x49);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(32, 32)], NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(32, 32), 0x4d);
}

#[test]
fn mac68k_place_road_rail_on_highway_uses_pe_crossover_table() {
    // Shared PlaceRoads/Rails existing→crossover tables (`0x44ae24`/`0x44b2f4`).
    let mut city = City::new_blank("/tmp/sc2000-hwy-cross-mac.sc2", "HwyCrossMac").expect("blank");
    city.set_building(34, 34, 0x49);
    city.set_building(35, 35, 0x4a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_path(&[(34, 34)], NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(34, 34), 0x4b);
    assert_eq!(state.place_network_path(&[(35, 35)], NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(35, 35), 0x4e);
}

#[test]
fn pe_place_rail_neighbor_gates_match_pe_range() {
    // PE @ `0x44b485`: rail ranges then NS/EW gates; helper only (not live-wired).
    assert!(pe_place_rail_reshape_neighbor_connects_44b485(
        0x2c,
        NETWORK_TOP
    ));
    assert!(pe_place_rail_reshape_neighbor_connects_44b485(
        0x2c,
        NETWORK_LEFT
    ));
    assert!(!pe_place_rail_reshape_neighbor_connects_44b485(
        0x2f,
        NETWORK_TOP
    )); // gate 0
    assert!(pe_place_rail_reshape_neighbor_connects_44b485(
        0x4d,
        NETWORK_LEFT
    )); // EW-only
    assert!(!pe_place_rail_reshape_neighbor_connects_44b485(
        0x4d,
        NETWORK_TOP
    ));
    assert!(!pe_place_rail_reshape_neighbor_connects_44b485(
        0x1d,
        NETWORK_TOP
    )); // road
}

#[test]
fn pe_place_road_neighbor_gates_match_pe_range() {
    // PE @ `0x44af6e`: road ranges then NS/EW gates; 0x21 NS-only, 0x1f never.
    // These gates are live-wired only while the Win95 Road tool reshapes a stroke.
    assert!(pe_place_road_reshape_neighbor_connects_44af6e(
        0x1d,
        NETWORK_TOP
    ));
    assert!(pe_place_road_reshape_neighbor_connects_44af6e(
        0x1d,
        NETWORK_LEFT
    ));
    assert!(pe_place_road_reshape_neighbor_connects_44af6e(
        0x21,
        NETWORK_TOP
    ));
    assert!(!pe_place_road_reshape_neighbor_connects_44af6e(
        0x21,
        NETWORK_LEFT
    ));
    assert!(!pe_place_road_reshape_neighbor_connects_44af6e(
        0x1f,
        NETWORK_TOP
    ));
    assert!(pe_place_road_reshape_neighbor_connects_44af6e(
        0x4b,
        NETWORK_LEFT
    )); // EW-only
    assert!(!pe_place_road_reshape_neighbor_connects_44af6e(
        0x4b,
        NETWORK_TOP
    ));
    assert!(!pe_place_road_reshape_neighbor_connects_44af6e(
        0x2c,
        NETWORK_TOP
    )); // rail base
}

#[test]
fn pe_road_tool_reshape_uses_original_neighbor_gate_table() {
    let mut city = City::new_blank("/tmp/sc2000-road-pe-gate.sc2", "RoadPeGate").expect("blank");
    city.set_building(72, 72, 0x1d);
    city.set_building(73, 72, 0x1f); // PE gate-0 slope road never connects.
    let state = test_state(city);
    assert_eq!(
        state.network_reshape_neighbor_mask(72, 72, NetworkKind::Road, true, false),
        0
    );
    // Other consumers retain the geometry-derived network connection policy.
    assert_eq!(
        state.network_reshape_neighbor_mask(72, 72, NetworkKind::Road, false, false),
        NETWORK_RIGHT
    );
}

#[test]
fn pe_road_refused_coordinate_does_not_run_reshape_worker() {
    // PE PlaceRoads rejects existing road tiles before its neighbor worker.
    // The XTXT arm would otherwise turn 0x1d into a different art variant.
    let mut city = City::new_blank("/tmp/sc2000-road-pe-refuse.sc2", "RoadPeRef").expect("blank");
    city.set_building(72, 72, 0x1d);
    city.set_text_overlay(72, 72, 2);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.place_network_path(&[(72, 72)], NetworkKind::Road), 0);
    assert_eq!(state.city.building_at(72, 72), 0x1d);
}

#[test]
fn pe_rail_tool_reshape_uses_original_neighbor_gate_and_writer_return() {
    let mut city = City::new_blank("/tmp/sc2000-rail-pe-gate.sc2", "RailPeGate").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(73, 72, 0x2f); // PE rail gate 0: no direction contributes.
    let mut state = test_state(city);
    state.tool = EditTool::Rail;
    assert_eq!(
        state.network_reshape_neighbor_mask(72, 72, NetworkKind::Rail, false, true),
        0
    );
    assert_eq!(
        state.network_reshape_neighbor_mask(72, 72, NetworkKind::Rail, false, false),
        NETWORK_RIGHT
    );
    // Isolate the actual placement assertion from the slope tile used by the
    // gate-table probe above.
    state.city.set_building(73, 72, 0);
    assert_eq!(state.place_network_path(&[(72, 72)], NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(72, 72), 0x2c);

    // PE PlaceRails refuses an existing rail before its neighbor reshapes. The
    // XTXT early arm would retile 0x2c to 0x2f if the refused path refreshed.
    let mut city = City::new_blank("/tmp/sc2000-rail-pe-refuse.sc2", "RailPeRef").expect("blank");
    city.set_building(72, 72, 0x2c);
    city.set_text_overlay(72, 72, 2);
    let mut state = test_state(city);
    state.tool = EditTool::Rail;
    assert_eq!(state.place_network_path(&[(72, 72)], NetworkKind::Rail), 0);
    assert_eq!(state.city.building_at(72, 72), 0x2c);
}

#[test]
fn pe_place_rail_xtxt_and_mask_tables() {
    // PE @ `0x44b41d`/`0x44b6a0`: table+0x2c; no mask0→0xf force.
    assert_eq!(pe_place_rail_mask_tile_44b6a0(0), 0x2c);
    assert_eq!(pe_place_rail_mask_tile_44b6a0(NETWORK_RIGHT), 0x2d);
    assert_eq!(
        pe_place_rail_mask_tile_44b6a0(NETWORK_TOP | NETWORK_RIGHT),
        0x32
    );
    let mut city = City::new_blank("/tmp/sc2000-rail-xtxt.sc2", "RailXtxt").expect("blank");
    city.set_building(80, 80, 0x2d);
    city.set_text_overlay(80, 80, 1); // → 2+0x2c = 0x2e
    assert!(pe_place_rail_xtxt_reshape_44b41d(&mut city, 80, 80));
    assert_eq!(city.building_at(80, 80), 0x2e);
    city.set_building(81, 81, 0x2d);
    city.set_text_overlay(81, 81, 5);
    assert!(!pe_place_rail_xtxt_reshape_44b41d(&mut city, 81, 81));
    assert_eq!(city.building_at(81, 81), 0x2d);
}

#[test]
fn refresh_rail_uses_xtxt_early_out_and_mask_table() {
    let mut city = City::new_blank("/tmp/sc2000-rail-ref.sc2", "RailRef").expect("blank");
    city.set_building(82, 82, 0x2d);
    city.set_text_overlay(82, 82, 2); // → 3+0x2c = 0x2f
    let mut state = test_state(city);
    state.refresh_network_tile(82, 82, NetworkKind::Rail);
    assert_eq!(state.city.building_at(82, 82), 0x2f);
    state.city.set_text_overlay(82, 82, 0);
    state.city.set_building(83, 82, 0x2c);
    state.refresh_network_tile(82, 82, NetworkKind::Rail);
    assert_eq!(state.city.building_at(82, 82), 0x2d); // RIGHT → table[2]+0x2c
}

#[test]
fn mac68k_refresh_road_uses_xtxt_early_out_and_mask_table() {
    let mut city = City::new_blank("/tmp/sc2000-road-ref-mac.sc2", "RoadRefMac").expect("blank");
    city.set_building(72, 72, 0x1e);
    city.set_text_overlay(72, 72, 2);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_network_tile(72, 72, NetworkKind::Road);
    assert_eq!(state.city.building_at(72, 72), 0x20);
    state.city.set_text_overlay(72, 72, 0);
    state.city.set_building(73, 72, 0x1d);
    state.refresh_network_tile(72, 72, NetworkKind::Road);
    assert_eq!(state.city.building_at(72, 72), 0x1e);
}

#[test]
fn mac68k_refresh_rail_uses_xtxt_early_out_and_mask_table() {
    let mut city = City::new_blank("/tmp/sc2000-rail-ref-mac.sc2", "RailRefMac").expect("blank");
    city.set_building(82, 82, 0x2d);
    city.set_text_overlay(82, 82, 2);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_network_tile(82, 82, NetworkKind::Rail);
    assert_eq!(state.city.building_at(82, 82), 0x2f);
    state.city.set_text_overlay(82, 82, 0);
    state.city.set_building(83, 82, 0x2c);
    state.refresh_network_tile(82, 82, NetworkKind::Rail);
    assert_eq!(state.city.building_at(82, 82), 0x2d);
}

#[test]
fn pe_place_road_xtxt_and_mask_tables() {
    // PE @ `0x44af50`/`0x44b0e5`: table+0x1d; no mask0→0xf force.
    assert_eq!(pe_place_road_mask_tile_44b0e5(0), 0x1d);
    assert_eq!(pe_place_road_mask_tile_44b0e5(NETWORK_RIGHT), 0x1e);
    assert_eq!(
        pe_place_road_mask_tile_44b0e5(NETWORK_TOP | NETWORK_RIGHT),
        0x23
    );
    let mut city = City::new_blank("/tmp/sc2000-road-xtxt.sc2", "RoadXtxt").expect("blank");
    city.set_building(70, 70, 0x1e);
    city.set_text_overlay(70, 70, 1); // → 2+0x1d = 0x1f
    assert!(pe_place_road_xtxt_reshape_44af50(&mut city, 70, 70));
    assert_eq!(city.building_at(70, 70), 0x1f);
    city.set_building(71, 71, 0x1e);
    city.set_text_overlay(71, 71, 5);
    assert!(!pe_place_road_xtxt_reshape_44af50(&mut city, 71, 71));
    assert_eq!(city.building_at(71, 71), 0x1e);
}

#[test]
fn refresh_road_uses_xtxt_early_out_and_mask_table() {
    let mut city = City::new_blank("/tmp/sc2000-road-ref.sc2", "RoadRef").expect("blank");
    city.set_building(72, 72, 0x1e);
    city.set_text_overlay(72, 72, 2); // → 3+0x1d = 0x20
    let mut state = test_state(city);
    state.refresh_network_tile(72, 72, NetworkKind::Road);
    assert_eq!(state.city.building_at(72, 72), 0x20);
    state.city.set_text_overlay(72, 72, 0);
    state.city.set_building(73, 72, 0x1d);
    state.refresh_network_tile(72, 72, NetworkKind::Road);
    assert_eq!(state.city.building_at(72, 72), 0x1e); // RIGHT → table[2]+0x1d
}

#[test]
fn pe_place_power_xtxt_and_mask_tables() {
    // PE @ `0x44aafb`/`0x44ac2c`: table+0x0e; no mask0→0xf force.
    assert_eq!(pe_place_power_mask_tile_44ac2c(0), 0x0e);
    assert_eq!(pe_place_power_mask_tile_44ac2c(NETWORK_RIGHT), 0x0f); // table[2]=1
    assert_eq!(
        pe_place_power_mask_tile_44ac2c(NETWORK_TOP | NETWORK_RIGHT),
        0x14
    ); // table[3]=6
    let mut city = City::new_blank("/tmp/sc2000-pwr-xtxt.sc2", "PwrXtxt").expect("blank");
    city.set_building(33, 33, 0x11);
    city.set_text_overlay(33, 33, 1); // gate on → table[1]+0x0e = 0x10
    assert!(pe_place_power_xtxt_reshape_44aafb(&mut city, 33, 33));
    assert_eq!(city.building_at(33, 33), 0x10);
    city.set_building(34, 34, 0x11);
    city.set_text_overlay(34, 34, 5); // gate off
    assert!(!pe_place_power_xtxt_reshape_44aafb(&mut city, 34, 34));
    assert_eq!(city.building_at(34, 34), 0x11);
}

#[test]
fn refresh_power_uses_xtxt_early_out_and_mask_table() {
    let mut city = City::new_blank("/tmp/sc2000-pwr-ref2.sc2", "PwrRef2").expect("blank");
    city.set_building(60, 60, 0x11);
    stamp_xbit_powerable(&mut city, 60, 60);
    city.set_text_overlay(60, 60, 2); // gate → 0x3+0x0e = 0x11
    let mut state = test_state(city);
    state.refresh_network_tile(60, 60, NetworkKind::Power);
    assert_eq!(state.city.building_at(60, 60), 0x11);
    // Clear XTXT; east powerable 0x0e neighbor → RIGHT → 0x0f.
    state.city.set_text_overlay(60, 60, 0);
    state.city.set_building(61, 60, 0x0e);
    stamp_xbit_powerable(&mut state.city, 61, 60);
    state.refresh_network_tile(60, 60, NetworkKind::Power);
    assert_eq!(state.city.building_at(60, 60), 0x0f);
}

#[test]
fn mac68k_refresh_power_uses_xtxt_early_out_and_mask_table() {
    let mut city = City::new_blank("/tmp/sc2000-pwr-ref2-mac.sc2", "PwrRef2Mac").expect("blank");
    city.set_building(60, 60, 0x11);
    stamp_xbit_powerable(&mut city, 60, 60);
    city.set_text_overlay(60, 60, 2);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_network_tile(60, 60, NetworkKind::Power);
    assert_eq!(state.city.building_at(60, 60), 0x11);
    state.city.set_text_overlay(60, 60, 0);
    state.city.set_building(61, 60, 0x0e);
    stamp_xbit_powerable(&mut state.city, 61, 60);
    state.refresh_network_tile(60, 60, NetworkKind::Power);
    assert_eq!(state.city.building_at(60, 60), 0x0f);
}

#[test]
fn pe_place_power_neighbor_requires_powerable_and_gates() {
    // PE @ `0x44ab2c`: XBIT 0x80 then gates; XBLD 0x11 is NS-only, 0x0f never.
    let mut city = City::new_blank("/tmp/sc2000-pwr-gates.sc2", "PwrGates").expect("blank");
    city.set_building(40, 40, 0x11);
    assert!(!pe_place_power_reshape_neighbor_connects_44ab30(
        &city,
        40,
        40,
        NETWORK_TOP
    ));
    stamp_xbit_powerable(&mut city, 40, 40);
    assert!(pe_place_power_reshape_neighbor_connects_44ab30(
        &city,
        40,
        40,
        NETWORK_TOP
    ));
    assert!(!pe_place_power_reshape_neighbor_connects_44ab30(
        &city,
        40,
        40,
        NETWORK_LEFT
    ));
    city.set_building(41, 41, 0x0f);
    stamp_xbit_powerable(&mut city, 41, 41);
    assert!(!pe_place_power_reshape_neighbor_connects_44ab30(
        &city,
        41,
        41,
        NETWORK_TOP
    ));
    assert!(!pe_place_power_reshape_neighbor_connects_44ab30(
        &city,
        41,
        41,
        NETWORK_LEFT
    ));
    city.set_building(42, 42, 0x5c);
    stamp_xbit_powerable(&mut city, 42, 42);
    for side in [NETWORK_TOP, NETWORK_RIGHT, NETWORK_BOTTOM, NETWORK_LEFT] {
        assert!(!pe_place_power_reshape_neighbor_connects_44ab30(
            &city, 42, 42, side
        ));
    }
}

#[test]
fn power_lines_connect_on_both_axes_in_both_drag_directions() {
    for file_kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (start, end, expected) in [
            ((40, 40), (46, 40), 0x0f),
            ((46, 40), (40, 40), 0x0f),
            ((40, 40), (40, 46), 0x0e),
            ((40, 46), (40, 40), 0x0e),
        ] {
            for one_at_a_time in [false, true] {
                let mut city = City::new_blank("/tmp/power-axes.sc2", "Power axes").unwrap();
                city.kind = file_kind;
                city.set_city_funds(100_000);
                let mut state = test_state(city);
                state.tool = EditTool::Power;
                let path = network_drag_path_for_kind(start, end, NetworkKind::Power);
                if one_at_a_time {
                    for &tile in &path {
                        assert_eq!(state.place_network_path(&[tile], NetworkKind::Power), 1);
                    }
                } else {
                    assert_eq!(
                        state.place_network_path(&path, NetworkKind::Power),
                        path.len()
                    );
                }
                for &(x, y) in &path {
                    assert_eq!(
                        state.city.building_at(x, y),
                        expected,
                        "{file_kind:?} {start:?}->{end:?}, single={one_at_a_time}, ({x},{y})"
                    );
                    assert!(state.city.tile_bits_at(x, y).unwrap().powerable());
                }
            }
        }
    }
}

#[test]
fn power_lines_connect_new_branches_to_existing_straight_wires() {
    let mut city = City::new_blank("/tmp/power-junction.sc2", "Power junction").unwrap();
    city.set_city_funds(100_000);
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    let path = network_drag_path_for_kind((40, 40), (44, 40), NetworkKind::Power);
    assert_eq!(state.place_network_path(&path, NetworkKind::Power), 5);
    assert_eq!(state.place_network_path(&[(42, 39)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(42, 40), 0x18); // top + left + right
    assert_eq!(state.city.building_at(42, 39), 0x0e);
    assert_eq!(state.place_network_path(&[(42, 41)], NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(42, 40), 0x1c); // all four sides
    assert_eq!(state.city.building_at(42, 41), 0x0e);
}

#[test]
fn refresh_power_reshapes_using_adjacent_powerable_wires() {
    let mut city = City::new_blank("/tmp/sc2000-pwr-refresh.sc2", "PwrRef").expect("blank");
    // East neighbor starts with perpendicular T/B wire art.
    city.set_building(55, 55, 0x11);
    stamp_xbit_powerable(&mut city, 55, 55);
    city.set_building(56, 55, 0x0e);
    stamp_xbit_powerable(&mut city, 56, 55);
    city.set_text_overlay(55, 55, 0);
    let mut state = test_state(city);
    state.refresh_network_tile(55, 55, NetworkKind::Power);
    // RIGHT only → network_building_for_mask with mask 2.
    let right_only = network_building_for_tile(NetworkKind::Power, NETWORK_RIGHT, 0);
    assert_eq!(state.city.building_at(55, 55), right_only);
    // Non-powerable east must not count.
    state.city.set_tile_bits(56, 55, TileBits::new(0));
    state.city.set_building(55, 55, 0x11);
    stamp_xbit_powerable(&mut state.city, 55, 55);
    state.refresh_network_tile(55, 55, NetworkKind::Power);
    let isolated = network_building_for_tile(NetworkKind::Power, 0, 0);
    assert_eq!(state.city.building_at(55, 55), isolated);
}

#[test]
fn pe_place_pipes_neighbor_gates_match_pe_range() {
    // PE @ `0x44b987`: pipes `0x10..0x1f`/`0x1f`/`0x20` then NS/EW gates.
    assert!(pe_place_pipes_reshape_neighbor_connects_44b98a(
        0x10,
        NETWORK_TOP
    ));
    assert!(pe_place_pipes_reshape_neighbor_connects_44b98a(
        0x10,
        NETWORK_LEFT
    )); // NS+EW
    assert!(pe_place_pipes_reshape_neighbor_connects_44b98a(
        0x11,
        NETWORK_TOP
    ));
    assert!(!pe_place_pipes_reshape_neighbor_connects_44b98a(
        0x11,
        NETWORK_LEFT
    )); // NS only
    assert!(!pe_place_pipes_reshape_neighbor_connects_44b98a(
        0x12,
        NETWORK_TOP
    )); // NS gate 0
    assert!(pe_place_pipes_reshape_neighbor_connects_44b98a(
        0x12,
        NETWORK_LEFT
    ));
    assert!(pe_place_pipes_reshape_neighbor_connects_44b98a(
        0x20,
        NETWORK_TOP
    ));
    assert!(!pe_place_pipes_reshape_neighbor_connects_44b98a(
        0x21,
        NETWORK_TOP
    ));
    assert!(!pe_place_pipes_reshape_neighbor_connects_44b98a(
        0x22,
        NETWORK_LEFT
    ));
}

#[test]
fn refresh_pipes_reshapes_using_pe_neighbor_gates() {
    let mut city = City::new_blank("/tmp/sc2000-pipe-gates.sc2", "PipeGates").expect("blank");
    // Center TB-named tile 0x11 (PE NS-only). East neighbor 0x10 (PE connects EW).
    city.set_underground(50, 50, 0x11);
    city.set_underground(51, 50, 0x10);
    city.set_text_overlay(50, 50, 0);
    let mut state = test_state(city);
    state.refresh_underground_tile(50, 50, UndergroundKind::Pipes);
    // East connects → RIGHT only → pe mask tile table[2]+0x10 → 0x11 (unchanged offset 1).
    // NETWORK_RIGHT=2 → table[2]=1 +0x10 = 0x11.
    assert_eq!(state.city.underground_at(50, 50), 0x11);
    // North neighbor 0x12 fails NS gate → still no TOP; add north 0x10 (NS ok).
    state.city.set_underground(50, 49, 0x10);
    state.refresh_underground_tile(50, 50, UndergroundKind::Pipes);
    // TOP|RIGHT → mask 1|2=3 → table[3]=6 +0x10 = 0x16.
    assert_eq!(state.city.underground_at(50, 50), 0x16);
}

#[test]
fn mac68k_refresh_pipes_reshapes_using_pe_neighbor_gates() {
    let mut city =
        City::new_blank("/tmp/sc2000-pipe-gates-mac.sc2", "PipeGatesMac").expect("blank");
    city.set_underground(50, 50, 0x11);
    city.set_underground(51, 50, 0x10);
    city.set_text_overlay(50, 50, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_underground_tile(50, 50, UndergroundKind::Pipes);
    assert_eq!(state.city.underground_at(50, 50), 0x11);
    state.city.set_underground(50, 49, 0x10);
    state.refresh_underground_tile(50, 50, UndergroundKind::Pipes);
    assert_eq!(state.city.underground_at(50, 50), 0x16);
}

#[test]
fn refresh_underground_tile_pipes_mask0_places_0x1e() {
    // Isolated pipe with no neighbors: PE forces mask 0 → tile 0x1e.
    let mut city = City::new_blank("/tmp/sc2000-pipe-mask0.sc2", "PipeMask0").expect("blank");
    city.set_underground(45, 45, UNDER_TILE_PIPES_LR);
    city.set_text_overlay(45, 45, 0);
    city.set_traffic_density(45, 45, 0x41);
    city.set_tile_bits(45, 45, TileBits::new(0).with_watered(true));
    let mut state = test_state(city);
    state.refresh_underground_tile(45, 45, UndergroundKind::Pipes);
    assert_eq!(state.city.underground_at(45, 45), 0x1e);
    assert_eq!(state.city.traffic_density_at(45, 45), Some(0x41));
    assert!(state.city.tile_bits_at(45, 45).unwrap().watered());
}

#[test]
fn mac68k_refresh_underground_tile_pipes_mask0_places_0x1e() {
    let mut city = City::new_blank("/tmp/sc2000-mac-pipe-mask0.sc2", "MacPipeM0").expect("blank");
    city.set_underground(46, 46, UNDER_TILE_PIPES_TB);
    city.set_text_overlay(46, 46, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_underground_tile(46, 46, UndergroundKind::Pipes);
    assert_eq!(state.city.underground_at(46, 46), 0x1e);
}

#[test]
fn stamp_building_underground_footprint_preserves_xtxt() {
    // Underground stamp is XUND only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-und-foot-xtxt.sc2", "UndFootT").expect("blank");
    city.set_building(53, 53, 108);
    city.set_text_overlay(53, 53, 0x50);
    let mut state = test_state(city);
    state.stamp_building_underground_footprint(53, 53, UNDER_TILE_GREY_BOX_2);
    assert_eq!(state.city.text_overlay_at(53, 53), Some(0x50));
}

#[test]
fn mac68k_stamp_building_underground_footprint_preserves_xtxt() {
    // Shared underground stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-und-foot-xtxt.sc2", "MacUndFootT").expect("blank");
    city.set_building(53, 53, 108);
    city.set_text_overlay(53, 53, 0x50);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_building_underground_footprint(53, 53, UNDER_TILE_GREY_BOX_2);
    assert_eq!(state.city.text_overlay_at(53, 53), Some(0x50));
}

#[test]
fn mac68k_stamp_building_underground_footprint_preserves_xtrf() {
    // Shared underground stamp is XUND only; Mac68k must not invent an XTRF clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-und-foot-xtrf.sc2", "MacUndFoot").expect("blank");
    city.set_building(53, 53, 108);
    city.set_traffic_density(53, 53, 0x24);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_building_underground_footprint(53, 53, UNDER_TILE_GREY_BOX_2);
    assert_eq!(state.city.traffic_density_at(53, 53), Some(0x24));
}

#[test]
fn clear_building_underground_footprint_preserves_xtxt() {
    // Underground clear is XUND only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-und-clear-xtxt.sc2", "UndClrT").expect("blank");
    city.set_building(54, 54, 249);
    city.set_underground(54, 54, UNDER_TILE_GREY_BOX_1);
    city.set_text_overlay(54, 54, 0x51);
    let mut state = test_state(city);
    state.clear_building_underground_footprint(54, 54, UNDER_TILE_GREY_BOX_1);
    assert_eq!(state.city.text_overlay_at(54, 54), Some(0x51));
}

#[test]
fn mac68k_clear_building_underground_footprint_preserves_xtxt() {
    // Shared underground clear leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-und-clear-xtxt.sc2", "MacUndClrT").expect("blank");
    city.set_building(54, 54, 249);
    city.set_underground(54, 54, UNDER_TILE_GREY_BOX_1);
    city.set_text_overlay(54, 54, 0x51);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.clear_building_underground_footprint(54, 54, UNDER_TILE_GREY_BOX_1);
    assert_eq!(state.city.text_overlay_at(54, 54), Some(0x51));
}

#[test]
fn mac68k_clear_building_underground_footprint_preserves_xtrf() {
    // Shared underground clear is XUND only; Mac68k must not invent an XTRF clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-und-clear-xtrf.sc2", "MacUndClr").expect("blank");
    city.set_building(54, 54, 249);
    city.set_underground(54, 54, UNDER_TILE_GREY_BOX_1);
    city.set_traffic_density(54, 54, 0x26);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.clear_building_underground_footprint(54, 54, UNDER_TILE_GREY_BOX_1);
    assert_eq!(state.city.underground_at(54, 54), 0);
    assert_eq!(state.city.traffic_density_at(54, 54), Some(0x26));
}

#[test]
fn refresh_network_tile_preserves_xtxt_like_pe_place_tile() {
    // Network reshape PlaceTile leaves XTXT.
    let mut city = City::new_blank("/tmp/sc2000-net-refresh-xtxt.sc2", "NetRefT").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(56, 56, 29);
    city.set_text_overlay(56, 56, 0x4c);
    let mut state = test_state(city);
    state.refresh_network_tile(56, 56, NetworkKind::Road);
    assert_eq!(state.city.text_overlay_at(56, 56), Some(0x4c));
}

#[test]
fn mac68k_refresh_network_tile_preserves_xtxt_like_pe_place_tile() {
    // Shared network reshape leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-net-refresh-xtxt.sc2", "MacNetRefT").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(56, 56, 29);
    city.set_text_overlay(56, 56, 0x4c);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_network_tile(56, 56, NetworkKind::Road);
    assert_eq!(state.city.text_overlay_at(56, 56), Some(0x4c));
}

#[test]
fn mac68k_refresh_network_tile_preserves_xtrf() {
    // Shared network reshape is PlaceTile XBLD only; Mac68k must not invent an XTRF clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-net-refresh-xtrf.sc2", "MacNetRef").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(56, 56, 29);
    city.set_traffic_density(56, 56, 0x2d);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_network_tile(56, 56, NetworkKind::Road);
    assert_eq!(state.city.traffic_density_at(56, 56), Some(0x2d));
}

#[test]
fn sign_tool_marks_blank_user_sign_slots_without_overwriting_microsim_labels() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(9, 10, 0x01);
    city.set_text_overlay(11, 10, 0x20);
    city.set_text_overlay(12, 10, 0x3d);
    let mut state = test_state(city);
    state.tool = EditTool::Sign;

    let blank = state.tool_placement_at(10, 10).expect("blank sign");
    assert_eq!(blank.valid_tiles, vec![(10, 10)]);
    assert!(blank.invalid_tiles.is_empty());
    assert!(state.place_sign_at(10, 10));
    assert_eq!(state.city.text_overlay_at(10, 10), Some(2));
    assert_eq!(state.city.label_at(2), Some("Sign"));
    assert_eq!(
        state.sign_dialog.as_ref().map(|dialog| dialog.sign_id),
        Some(2)
    );
    state.commit_sign_dialog();

    state.city.set_label(0x20, "Existing");
    assert!(state.place_sign_at(11, 10));
    assert_eq!(state.city.text_overlay_at(11, 10), Some(0x20));
    assert_eq!(state.city.label_at(0x20), Some("Existing"));
    assert_eq!(
        state
            .sign_dialog
            .as_ref()
            .map(|dialog| dialog.text.as_str()),
        Some("Existing")
    );
    state.cancel_sign_dialog();

    let microsim = state.tool_placement_at(12, 10).expect("microsim sign");
    assert!(microsim.valid_tiles.is_empty());
    assert_eq!(microsim.invalid_tiles, vec![(12, 10)]);
    assert!(!state.place_sign_at(12, 10));
    assert_eq!(state.city.text_overlay_at(12, 10), Some(0x3d));
}

#[test]
fn sign_tool_rejects_new_signs_when_user_sign_slots_are_full() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for sign_id in 0x01..=0x32 {
        city.set_text_overlay(usize::from(sign_id), 10, sign_id);
    }
    let mut state = test_state(city);
    state.tool = EditTool::Sign;

    let placement = state.tool_placement_at(80, 80).expect("sign placement");
    assert!(placement.valid_tiles.is_empty());
    assert_eq!(placement.invalid_tiles, vec![(80, 80)]);
    assert!(!state.place_sign_at(80, 80));
    assert_eq!(state.city.text_overlay_at(80, 80), Some(0));

    assert!(state.place_sign_at(1, 10));
    assert_eq!(state.city.text_overlay_at(1, 10), Some(1));
}

#[test]
fn rail_depot_connects_neighboring_rail_without_becoming_a_rail_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(
        19,
        20,
        network_building_for_mask(NetworkKind::Rail, NETWORK_LEFT),
    );
    let mut state = test_state(city);
    state.tool = EditTool::RailDepot;

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 237);
    assert_eq!(state.city.building_at(21, 21), 237);
    assert!(!is_rail_tile(state.city.building_at(20, 20)));
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Rail),
        NETWORK_RIGHT
    );
    assert_eq!(
        state.city.building_at(19, 20),
        network_building_for_mask(NetworkKind::Rail, NETWORK_RIGHT)
    );

    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert_eq!(state.network_neighbor_mask(19, 20, NetworkKind::Rail), 0);
}

#[test]
fn bus_depot_connects_neighboring_road_without_becoming_a_road_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(
        19,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT),
    );
    let mut state = test_state(city);
    state.tool = EditTool::BusDepot;

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 236);
    assert_eq!(state.city.building_at(21, 21), 236);
    assert!(!is_road_tile(state.city.building_at(20, 20)));
    assert_eq!(
        state.network_neighbor_mask(19, 20, NetworkKind::Road),
        NETWORK_RIGHT
    );
    assert_eq!(
        state.city.building_at(19, 20),
        network_building_for_mask(NetworkKind::Road, NETWORK_RIGHT)
    );

    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert_eq!(state.network_neighbor_mask(19, 20, NetworkKind::Road), 0);
}

#[test]
fn hover_cost_uses_selected_tool_price_only_when_placement_is_valid() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(11, 11, 112);
    let mut state = test_state(city);

    state.tool = EditTool::PowerPlantCoal;
    state.hover_tile = Some((20, 20));
    assert_eq!(state.hover_cost_overlay(), Some((4_000, (20, 20))));

    state.hover_tile = Some((10, 10));
    assert_eq!(state.hover_cost_overlay(), None);

    state.tool = EditTool::PowerPlantWind;
    state.hover_tile = Some((30, 30));
    assert_eq!(state.hover_cost_overlay(), Some((100, (30, 30))));

    state.tool = EditTool::Police;
    state.hover_tile = Some((40, 40));
    assert_eq!(state.hover_cost_overlay(), Some((500, (40, 40))));

    for y in 50..=51 {
        for x in 50..=51 {
            state.city.set_building(x, y, 141);
        }
    }
    state.tool = EditTool::Bulldoze;
    state.hover_tile = Some((51, 51));
    // Win95 charges one tool unit for the accepted click coordinate even when
    // DestroyBuilding clears the whole 2×2 footprint.
    assert_eq!(state.hover_cost_overlay(), Some((1, (51, 51))));

    state.tool = EditTool::Pipes;
    assert_eq!(state.hover_cost_overlay(), None);

    state = test_state(tunnel_test_city());
    state.tool = EditTool::Tunnel;
    state.hover_tile = Some((20, 20));
    assert_eq!(state.hover_cost_overlay(), Some((600, (20, 20))));
}

#[test]
fn building_placement_debits_selected_tool_cost() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    for x in 10..=13 {
        city.set_terrain(x, 10, 0);
        city.set_building(x, 10, 0);
        city.set_altitude(x, 10, 5, 0);
    }
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantCoal;

    assert_eq!(state.place_building_at(20, 20, 207), 16);
    assert_eq!(state.city.city_funds(), 16_000);

    assert_eq!(state.place_building_at(20, 20, 207), 0);
    assert_eq!(state.city.city_funds(), 16_000);
}

#[test]
fn citytool_marina_writes_microsim_xtxt_like_place_tile_size() {
    // Dedicated pin: PlaceTile-size marina (`0xF8`) provisions microsim XTXT across 3×3.
    let mut city = City::new_blank("/tmp/sc2000-ct-marina-xtxt.sc2", "CtMarX").expect("blank");
    city.set_city_funds(20_000);
    for x in 20..23 {
        city.set_terrain(x, 20, 0x30);
        city.set_tile_bits(x, 20, TileBits::new(0).with_water_tile(true));
    }
    let mut state = test_state(city);
    state.tool = EditTool::Marina;
    assert_eq!(state.place_building_at(20, 20, MARINA_BUILDING_ID), 9);
    let xtxt = state.city.text_overlay_at(20, 20).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES);
    for &(tx, ty) in &building_footprint_from_origin(20, 20, 3).expect("3x3") {
        assert_eq!(state.city.text_overlay_at(tx, ty), Some(xtxt));
    }
}

#[test]
fn citytool_stadium_and_zoo_use_place_tile_size_xbit_and_corners() {
    // PE CityToolPlaceSelectedBuilding @ `0x440731` → PlaceTile-size for 0xD7/0xDA:
    // XBIT `|=0xe0`; size>1 epilogue ORs `0x4dc4d0` rotation corners.
    for &(building, label) in &[(STADIUM_BUILDING_ID, "Stadium"), (ZOO_BUILDING_ID, "Zoo")] {
        let mut city =
            City::new_blank(format!("/tmp/sc2000-ct-{label}.sc2"), label).expect("blank");
        city.set_city_funds(50_000);
        city.set_view_rotation(0);
        for dx in 0..4usize {
            for dy in 0..4usize {
                city.set_tile_bits(24 + dx, 24 + dy, TileBits::new(0).with_watered(true));
                city.set_zone(24 + dx, 24 + dy, ZONE_RESIDENTIAL_LIGHT | 0x80);
            }
        }
        city.set_traffic_density(25, 25, 0x55);
        let mut state = test_state(city);
        state.tool = if building == STADIUM_BUILDING_ID {
            EditTool::Stadium
        } else {
            EditTool::Zoo
        };
        assert_eq!(state.place_building_at(24, 24, building), 16, "{label}");
        let corner_bits = pe_choose_and_place_corner_bits(0);
        for dx in 0..4usize {
            for dy in 0..4usize {
                assert_eq!(
                    state.city.building_at(24 + dx, 24 + dy),
                    building,
                    "{label}"
                );
                let bits = state.city.tile_bits_at(24 + dx, 24 + dy).unwrap();
                assert_eq!(
                    bits.raw_bits() & XBIT_CHOOSE_AND_PLACE,
                    0xa0,
                    "{label} remains piped-capable/powerable but unserved after empty-map recalculation"
                );
                assert!(!bits.watered() && !bits.powered(), "{label} {bits:?}");
                let expect = building_zone_bits_for_tile(24, 24, 24 + dx, 24 + dy, 4, corner_bits);
                assert_eq!(
                    state.city.zone_at(24 + dx, 24 + dy).unwrap().raw_bits(),
                    expect,
                    "{label}"
                );
            }
        }
        assert_eq!(state.city.traffic_density_at(25, 25), Some(0x55), "{label}");
        // PE PlaceTile-size @ `0x440ea0` → microsim allocate; write XTXT on success.
        let xtxt = state.city.text_overlay_at(24, 24).unwrap_or(0);
        assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "{label} microsim XTXT {xtxt}");
        for dx in 0..4usize {
            for dy in 0..4usize {
                assert_eq!(
                    state.city.text_overlay_at(24 + dx, 24 + dy),
                    Some(xtxt),
                    "{label} footprint XTXT"
                );
            }
        }
    }
}

#[test]
fn citytool_stadium_zoo_write_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: stadium/zoo provision XTXT across 4×4.
    for &(building, tool, label) in &[
        (STADIUM_BUILDING_ID, EditTool::Stadium, "Stadium"),
        (ZOO_BUILDING_ID, EditTool::Zoo, "Zoo"),
    ] {
        let mut city =
            City::new_blank(format!("/tmp/sc2000-ct-{label}-xtxt.sc2"), label).expect("blank");
        city.set_city_funds(50_000);
        let mut state = test_state(city);
        state.tool = tool;
        assert_eq!(state.place_building_at(24, 24, building), 16, "{label}");
        let xtxt = state.city.text_overlay_at(24, 24).unwrap_or(0);
        assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "{label}");
        for dx in 0..4usize {
            for dy in 0..4usize {
                assert_eq!(
                    state.city.text_overlay_at(24 + dx, 24 + dy),
                    Some(xtxt),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn citytool_police_uses_place_tile_size_xbit_and_corners() {
    // PE CityTool @ `0x440731` → PlaceTile-size for civic 0xD2 (police, size 3).
    let mut city = City::new_blank("/tmp/sc2000-ct-police.sc2", "CtPolice").expect("blank");
    city.set_city_funds(20_000);
    city.set_view_rotation(0);
    for dx in 0..3usize {
        for dy in 0..3usize {
            city.set_tile_bits(28 + dx, 28 + dy, TileBits::new(0).with_watered(true));
            city.set_zone(28 + dx, 28 + dy, ZONE_RESIDENTIAL_LIGHT | 0x80);
        }
    }
    city.set_traffic_density(29, 29, 0x17);
    let mut state = test_state(city);
    state.tool = EditTool::Police;
    assert_eq!(state.place_building_at(28, 28, 210), 9);
    let corner_bits = pe_choose_and_place_corner_bits(0);
    for dx in 0..3usize {
        for dy in 0..3usize {
            assert_eq!(state.city.building_at(28 + dx, 28 + dy), 210);
            let bits = state.city.tile_bits_at(28 + dx, 28 + dy).unwrap();
            assert_eq!(bits.raw_bits() & 0xe0, 0xa0);
            let expect = building_zone_bits_for_tile(28, 28, 28 + dx, 28 + dy, 3, corner_bits);
            assert_eq!(
                state.city.zone_at(28 + dx, 28 + dy).unwrap().raw_bits(),
                expect
            );
        }
    }
    assert_eq!(state.city.traffic_density_at(29, 29), Some(0x17));
    let xtxt = state.city.text_overlay_at(28, 28).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "police microsim XTXT {xtxt}");
    for dx in 0..3usize {
        for dy in 0..3usize {
            assert_eq!(
                state.city.text_overlay_at(28 + dx, 28 + dy),
                Some(xtxt),
                "footprint XTXT"
            );
        }
    }
}

#[test]
fn citytool_bus_depot_uses_place_tile_size_xbit_and_corners() {
    // PE CityTool @ `0x440731` → PlaceTile-size for bus depot 0xEC (size 2).
    let mut city = City::new_blank("/tmp/sc2000-ct-bus.sc2", "CtBus").expect("blank");
    city.set_city_funds(20_000);
    city.set_view_rotation(0);
    for dx in 0..2usize {
        for dy in 0..2usize {
            city.set_tile_bits(40 + dx, 40 + dy, TileBits::new(0).with_watered(true));
        }
    }
    city.set_traffic_density(40, 40, 0x22);
    let mut state = test_state(city);
    state.tool = EditTool::BusDepot;
    assert_eq!(state.place_building_at(40, 40, 236), 4);
    let corner_bits = pe_choose_and_place_corner_bits(0);
    for dx in 0..2usize {
        for dy in 0..2usize {
            assert_eq!(state.city.building_at(40 + dx, 40 + dy), 236);
            assert_eq!(
                state
                    .city
                    .tile_bits_at(40 + dx, 40 + dy)
                    .unwrap()
                    .raw_bits()
                    & 0xe0,
                0xa0
            );
            let expect = building_zone_bits_for_tile(40, 40, 40 + dx, 40 + dy, 2, corner_bits);
            assert_eq!(
                state.city.zone_at(40 + dx, 40 + dy).unwrap().raw_bits(),
                expect
            );
        }
    }
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x22));
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: bus depot class writes XTXT.
    let xtxt = state.city.text_overlay_at(40, 40).unwrap_or(0);
    assert!(
        xtxt >= MIN_SIM_TEXT_ENTRIES,
        "bus depot microsim XTXT {xtxt}"
    );
    for dx in 0..2usize {
        for dy in 0..2usize {
            assert_eq!(
                state.city.text_overlay_at(40 + dx, 40 + dy),
                Some(xtxt),
                "bus depot footprint XTXT"
            );
        }
    }
}

#[test]
fn citytool_bus_depot_writes_microsim_xtxt_like_place_tile_size() {
    // Dedicated pin: PlaceTile-size bus depot (`0xEC`) provisions microsim XTXT across 2×2.
    let mut city = City::new_blank("/tmp/sc2000-ct-bus-xtxt.sc2", "CtBusX").expect("blank");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.tool = EditTool::BusDepot;
    assert_eq!(state.place_building_at(40, 40, 236), 4);
    let xtxt = state.city.text_overlay_at(40, 40).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES);
    for dx in 0..2usize {
        for dy in 0..2usize {
            assert_eq!(state.city.text_overlay_at(40 + dx, 40 + dy), Some(xtxt));
        }
    }
}

#[test]
fn citytool_rail_depot_writes_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: rail depot (`0xED`) provisions XTXT across 2×2.
    let mut city = City::new_blank("/tmp/sc2000-ct-rail-xtxt.sc2", "CtRailX").expect("blank");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.tool = EditTool::RailDepot;
    assert_eq!(state.place_building_at(42, 42, 237), 4);
    let xtxt = state.city.text_overlay_at(42, 42).unwrap_or(0);
    assert!(
        xtxt >= MIN_SIM_TEXT_ENTRIES,
        "rail depot microsim XTXT {xtxt}"
    );
    for dx in 0..2usize {
        for dy in 0..2usize {
            assert_eq!(state.city.text_overlay_at(42 + dx, 42 + dy), Some(xtxt));
        }
    }
}

#[test]
fn citytool_library_writes_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: library (`0xF5`) provisions XTXT across 2×2.
    let mut city = City::new_blank("/tmp/sc2000-ct-lib-xtxt.sc2", "CtLibX").expect("blank");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.tool = EditTool::Library;
    assert_eq!(state.place_building_at(46, 46, 245), 4);
    let xtxt = state.city.text_overlay_at(46, 46).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "library microsim XTXT {xtxt}");
    for dx in 0..2usize {
        for dy in 0..2usize {
            assert_eq!(state.city.text_overlay_at(46 + dx, 46 + dy), Some(xtxt));
        }
    }
}

#[test]
fn citytool_mayor_house_writes_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: mayor house (`0xF3`) provisions XTXT across 2×2.
    let mut city = City::new_blank("/tmp/sc2000-ct-mayor-xtxt.sc2", "CtMayorX").expect("blank");
    city.set_city_funds(20_000);
    city.set_rewards_available_mask(reward_mask_for_tool(EditTool::MayorHouse).unwrap_or(0));
    let mut state = test_state(city);
    state.tool = EditTool::MayorHouse;
    assert_eq!(state.place_building_at(48, 48, 243), 4);
    let xtxt = state.city.text_overlay_at(48, 48).unwrap_or(0);
    assert!(
        xtxt >= MIN_SIM_TEXT_ENTRIES,
        "mayor house microsim XTXT {xtxt}"
    );
    for dx in 0..2usize {
        for dy in 0..2usize {
            assert_eq!(state.city.text_overlay_at(48 + dx, 48 + dy), Some(xtxt));
        }
    }
}

#[test]
fn citytool_coal_plant_writes_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: coal (`0xCF`) provisions XTXT across 4×4.
    let mut city = City::new_blank("/tmp/sc2000-ct-coal-xtxt.sc2", "CtCoalX").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantCoal;
    assert_eq!(state.place_building_at(50, 50, 207), 16);
    let xtxt = state.city.text_overlay_at(50, 50).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "coal microsim XTXT {xtxt}");
    for dx in 0..4usize {
        for dy in 0..4usize {
            assert_eq!(state.city.text_overlay_at(50 + dx, 50 + dy), Some(xtxt));
        }
    }
}

#[test]
fn citytool_advanced_power_plants_write_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: wind/solar/microwave/fusion provision XTXT.
    for &(building, tool, label, edge) in &[
        (200u8, EditTool::PowerPlantWind, "Wind", 1usize),
        (204u8, EditTool::PowerPlantSolar, "Solar", 4usize),
        (205u8, EditTool::PowerPlantMicrowave, "Microwave", 4usize),
        (206u8, EditTool::PowerPlantFusion, "Fusion", 4usize),
    ] {
        let mut city =
            City::new_blank(format!("/tmp/sc2000-ct-{label}-xtxt.sc2"), label).expect("blank");
        city.set_city_funds(100_000);
        let mut state = test_state(city);
        state.tool = tool;
        assert_eq!(
            state.place_building_at(64, 64, building),
            edge * edge,
            "{label}"
        );
        let xtxt = state.city.text_overlay_at(64, 64).unwrap_or(0);
        assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "{label} microsim XTXT {xtxt}");
        for dx in 0..edge {
            for dy in 0..edge {
                assert_eq!(
                    state.city.text_overlay_at(64 + dx, 64 + dy),
                    Some(xtxt),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn citytool_museum_writes_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: museum (`0xD4`) provisions XTXT across 3×3.
    let mut city = City::new_blank("/tmp/sc2000-ct-museum-xtxt.sc2", "CtMusX").expect("blank");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.tool = EditTool::Museum;
    assert_eq!(state.place_building_at(52, 52, 212), 9);
    let xtxt = state.city.text_overlay_at(52, 52).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "museum microsim XTXT {xtxt}");
    for dx in 0..3usize {
        for dy in 0..3usize {
            assert_eq!(state.city.text_overlay_at(52 + dx, 52 + dy), Some(xtxt));
        }
    }
}

#[test]
fn citytool_nimby_gate_refuses_prison_when_residential_count_beats_lfsr() {
    // PE CityTool @ `0x440624`/`0x4406e7`: prison `0xD8` counts XZON type 1/2 in
    // origin±8+size window; `0x401cad(0xc8) < count` refuses (dialog skipped).
    let mut city = City::new_blank("/tmp/sc2000-ct-nimby.sc2", "CtNimby").expect("blank");
    city.set_city_funds(50_000);
    // Fill a dense residential neighborhood so count > 0xc8 → any LFSR roll refuses.
    for x in 10..30usize {
        for y in 10..30usize {
            city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
        }
    }
    let mut state = test_state(city);
    state.tool = EditTool::Prison;
    assert_eq!(state.place_building_at(16, 16, 216), 0);
    assert_eq!(state.city.building_at(16, 16), 0);
}

#[test]
fn citytool_nimby_gate_allows_prison_without_nearby_residential() {
    // PE: residential count 0 → `0x401cad(0xc8) < 0` never true → PlaceTile-size runs.
    let mut city = City::new_blank("/tmp/sc2000-ct-nimby-ok.sc2", "CtNimbyOk").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Prison;
    assert_eq!(state.place_building_at(40, 40, 216), 16);
    assert_eq!(state.city.building_at(40, 40), 216);
}

#[test]
fn citytool_prison_writes_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: prison `0xD8` provisions XTXT across 4×4
    // when the NIMBY gate passes (no nearby residential).
    let mut city = City::new_blank("/tmp/sc2000-ct-prison-xtxt.sc2", "CtPrisX").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Prison;
    assert_eq!(state.place_building_at(60, 60, 216), 16);
    let xtxt = state.city.text_overlay_at(60, 60).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "prison microsim XTXT {xtxt}");
    for dx in 0..4usize {
        for dy in 0..4usize {
            assert_eq!(state.city.text_overlay_at(60 + dx, 60 + dy), Some(xtxt));
        }
    }
}

#[test]
fn citytool_gas_and_oil_write_microsim_xtxt_when_nimby_passes() {
    // PE PlaceTile-size microsim after NIMBY gate passes (no nearby residential).
    for &(building, tool, label) in &[
        (201u8, EditTool::PowerPlantGas, "Gas"),
        (202u8, EditTool::PowerPlantOil, "Oil"),
    ] {
        let mut city =
            City::new_blank(format!("/tmp/sc2000-ct-{label}-xtxt.sc2"), label).expect("blank");
        city.set_city_funds(50_000);
        let mut state = test_state(city);
        state.tool = tool;
        assert_eq!(state.place_building_at(70, 70, building), 16, "{label}");
        let xtxt = state.city.text_overlay_at(70, 70).unwrap_or(0);
        assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "{label} microsim XTXT {xtxt}");
        for dx in 0..4usize {
            for dy in 0..4usize {
                assert_eq!(
                    state.city.text_overlay_at(70 + dx, 70 + dy),
                    Some(xtxt),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn citytool_nuclear_writes_microsim_xtxt_when_nimby_passes() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: nuclear `0xCB` XTXT 4×4 when NIMBY passes.
    let mut city = City::new_blank("/tmp/sc2000-ct-nuke-xtxt.sc2", "CtNukeX").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantNuclear;
    assert_eq!(state.place_building_at(72, 72, 203), 16);
    let xtxt = state.city.text_overlay_at(72, 72).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "nuclear microsim XTXT {xtxt}");
    for dx in 0..4usize {
        for dy in 0..4usize {
            assert_eq!(state.city.text_overlay_at(72 + dx, 72 + dy), Some(xtxt));
        }
    }
}

#[test]
fn citytool_statue_uses_place_tile_size_then_clears_powerable() {
    // PE CityTool @ `0x440731` PlaceTile-size then statue arm @ `0x440835` clears XBIT 0x80.
    let mut city = City::new_blank("/tmp/sc2000-ct-statue.sc2", "CtStatue").expect("blank");
    city.set_city_funds(20_000);
    city.set_rewards_available_mask(reward_mask_for_tool(EditTool::Statue).unwrap_or(0));
    city.set_tile_bits(50, 50, TileBits::new(0).with_watered(true));
    city.set_traffic_density(50, 50, 0x09);
    let mut state = test_state(city);
    state.tool = EditTool::Statue;
    assert_eq!(state.place_building_at(50, 50, 219), 1);
    assert_eq!(state.city.building_at(50, 50), 219);
    let bits = state.city.tile_bits_at(50, 50).unwrap();
    assert!(bits.piped() && !bits.watered());
    assert!(!bits.powerable());
    assert!(bits.powered());
    assert_eq!(bits.raw_bits() & 0xe0, 0x60);
    assert_eq!(
        state.city.zone_at(50, 50).unwrap().raw_bits(),
        ZONE_CORNER_ALL
    );
    assert_eq!(state.city.traffic_density_at(50, 50), Some(0x09));
}

#[test]
fn citytool_coal_plant_uses_place_tile_size_xbit() {
    // PE CityTool @ `0x440731` → PlaceTile-size for coal plant 0xCF (size 4).
    let mut city = City::new_blank("/tmp/sc2000-ct-coal.sc2", "CtCoal").expect("blank");
    city.set_city_funds(50_000);
    city.set_view_rotation(0);
    for dx in 0..4usize {
        for dy in 0..4usize {
            city.set_tile_bits(10 + dx, 10 + dy, TileBits::new(0).with_watered(true));
        }
    }
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantCoal;
    assert_eq!(state.place_building_at(10, 10, 207), 16);
    for dx in 0..4usize {
        for dy in 0..4usize {
            assert_eq!(state.city.building_at(10 + dx, 10 + dy), 207);
            assert_eq!(
                state
                    .city
                    .tile_bits_at(10 + dx, 10 + dy)
                    .unwrap()
                    .raw_bits()
                    & 0xe0,
                XBIT_CHOOSE_AND_PLACE
            );
        }
    }
}

#[test]
fn zoning_debits_per_placed_tile_cost() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    assert_eq!(
        state.place_zone_rect((10, 10), (11, 10), ZONE_INDUSTRIAL_DENSE),
        2
    );
    assert_eq!(state.city.city_funds(), 19_980);

    assert_eq!(state.place_zone_rect((20, 20), (21, 20), ZONE_AIRPORT), 2);
    assert_eq!(state.city.city_funds(), 19_480);
}

#[test]
fn zone_drag_cost_uses_only_buildable_rectangle_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(11, 10, 112);
    let mut state = test_state(city);
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);
    state.update_zone_drag_to((12, 11));

    assert_eq!(state.zone_drag_cost(), Some(50));
    state.refresh_status_line();
    assert_eq!(state.query_line, "Dense Commercial $50");
    assert_eq!(state.finish_zone_drag(), 5);
}

#[test]
fn win95_zone_drag_charges_only_tiles_whose_zone_type_changes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_zone(10, 10, ZONE_COMMERCIAL_DENSE | 0x80);
    city.set_zone(11, 10, ZONE_RESIDENTIAL_LIGHT | 0x40);
    let mut state = test_state(city);
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);
    state.update_zone_drag_to((12, 10));

    assert_eq!(state.zone_drag_cost(), Some(20));
    state.refresh_status_line();
    assert_eq!(state.query_line, "Dense Commercial $20");
    assert_eq!(state.finish_zone_drag(), 3);
    assert_eq!(state.city.city_funds(), 19_980);
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().raw_bits(),
        ZONE_COMMERCIAL_DENSE | 0x80
    );
    assert_eq!(
        state.city.zone_at(11, 10).unwrap().raw_bits(),
        ZONE_COMMERCIAL_DENSE | 0x40
    );
    assert_eq!(
        state.city.zone_at(12, 10).unwrap().zone_id(),
        ZONE_COMMERCIAL_DENSE
    );
}

#[test]
fn mac68k_zone_drag_charges_only_tiles_whose_zone_type_changes() {
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-price-change.sc2", "MacZonePrice")
        .expect("blank city");
    city.set_city_funds(20_000);
    city.set_zone(10, 10, ZONE_COMMERCIAL_DENSE | 0x80);
    city.set_zone(11, 10, ZONE_RESIDENTIAL_LIGHT | 0x40);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);
    state.update_zone_drag_to((12, 10));

    assert_eq!(state.zone_drag_cost(), Some(20));
    assert_eq!(state.finish_zone_drag(), 3);
    assert_eq!(state.city.city_funds(), 19_980);
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().raw_bits(),
        ZONE_COMMERCIAL_DENSE | 0x80
    );
    assert_eq!(
        state.city.zone_at(11, 10).unwrap().raw_bits(),
        ZONE_COMMERCIAL_DENSE | 0x40
    );
    assert_eq!(
        state.city.zone_at(12, 10).unwrap().zone_id(),
        ZONE_COMMERCIAL_DENSE
    );
}

#[test]
fn mac68k_zone_drag_rejects_zero_price_same_type_work() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-zone-price-zero.sc2", "MacZoneZero").expect("blank city");
    city.set_city_funds(0);
    city.set_zone(10, 10, ZONE_COMMERCIAL_DENSE | 0x80);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);

    assert_eq!(state.zone_drag_cost(), Some(0));
    assert_eq!(state.finish_zone_drag(), 0);
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().raw_bits(),
        ZONE_COMMERCIAL_DENSE | 0x80
    );
    assert_eq!(state.city.city_funds(), 0);
}

#[test]
fn win95_zone_drag_rejects_zero_price_same_type_work() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(0);
    city.set_zone(10, 10, ZONE_COMMERCIAL_DENSE | 0x80);
    let mut state = test_state(city);
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);

    assert_eq!(state.zone_drag_cost(), Some(0));
    assert_eq!(state.finish_zone_drag(), 0);
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().raw_bits(),
        ZONE_COMMERCIAL_DENSE | 0x80
    );
    assert_eq!(state.city.city_funds(), 0);
}

#[test]
fn win95_dezone_zero_price_does_not_clear_unzoned_rubble() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(0);
    city.set_building(10, 10, 2);
    let mut state = test_state(city);
    state.tool = EditTool::DeZone;

    state.begin_zone_drag(10, 10, 0);

    assert_eq!(state.zone_drag_cost(), Some(0));
    assert_eq!(state.finish_zone_drag(), 0);
    assert_eq!(state.city.building_at(10, 10), 2);
    assert_eq!(state.city.city_funds(), 0);
}

#[test]
fn linear_infrastructure_debits_only_committed_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(13, 10, 0x70);
    let mut state = test_state(city);
    let path = [(10, 10), (11, 10), (12, 10), (13, 10), (14, 10)];

    assert_eq!(state.place_network_path(&path, NetworkKind::Power), 3);
    assert_eq!(state.city.city_funds(), 19_994);

    assert_eq!(
        state.place_underground_path(&[(30, 30), (31, 30)], UndergroundKind::Pipes),
        2
    );
    assert_eq!(state.city.city_funds(), 19_988);

    assert_eq!(
        state.place_underground_path(&[(34, 34), (35, 34)], UndergroundKind::Subway),
        2
    );
    assert_eq!(state.city.city_funds(), 19_788);

    assert_eq!(
        state.place_network_path(&[(40, 40), (41, 40)], NetworkKind::Road),
        2
    );
    assert_eq!(state.city.city_funds(), 19_768);

    assert_eq!(
        state.place_network_path(&[(50, 50), (51, 50)], NetworkKind::Rail),
        2
    );
    assert_eq!(state.city.city_funds(), 19_718);
}

#[test]
fn highway_placement_debits_per_successful_segment() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    assert_eq!(state.place_highway_path(&[(40, 40)]), 1);
    assert_eq!(state.city.city_funds(), 19_900);
}

#[test]
fn selected_network_tool_rejects_unaffordable_stroke_as_a_whole() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    let path = [(10, 10), (11, 10), (12, 10), (13, 10)];

    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 0);
    assert_eq!(state.city.city_funds(), 20);
    assert_eq!(state.city.building_at(10, 10), 0);
    assert_eq!(state.city.building_at(11, 10), 0);
    assert_eq!(state.city.building_at(12, 10), 0);
    assert_eq!(state.city.building_at(13, 10), 0);
}

#[test]
fn selected_pipe_tool_rejects_an_unaffordable_stroke_as_a_whole() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(6);
    let mut state = test_state(city);
    state.tool = EditTool::Pipes;
    let path = [(10, 10), (11, 10), (12, 10)];

    assert_eq!(
        state.place_underground_path(&path, UndergroundKind::Pipes),
        0
    );
    assert_eq!(state.city.city_funds(), 6);
    assert_eq!(state.city.underground_at(10, 10), 0);
    assert_eq!(state.city.underground_at(11, 10), 0);
    assert_eq!(state.city.underground_at(12, 10), 0);
}

#[test]
fn pe_network_stroke_rejects_unaffordable_surface_work_without_mutation() {
    let mut city =
        City::new_blank("/tmp/sc2000-network-xter-poor.sc2", "NetworkXterPoor").expect("blank");
    city.set_city_funds(44);
    city.set_terrain(10, 10, 0x05);
    city.set_tile_bits(
        10,
        10,
        TileBits::new(0)
            .with_water_tile(true)
            .with_piped(true)
            .with_watered(true),
    );
    let mut state = test_state(city);
    state.tool = EditTool::Road;

    assert_eq!(
        state.place_network_path(&[(10, 10), (11, 10)], NetworkKind::Road),
        0
    );
    assert_eq!(state.city.city_funds(), 44);
    assert_eq!(state.city.building_at(10, 10), 0);
    assert_eq!(state.city.building_at(11, 10), 0);
    assert_eq!(state.city.terrain_at(10, 10), 0x05);
    assert_eq!(
        state.city.tile_bits_at(10, 10).expect("XBIT").raw_bits(),
        0x34
    );
}

#[test]
fn pe_pipe_stroke_class12_is_surcharged_and_remapped() {
    let mut city = City::new_blank("/tmp/sc2000-pipe-xter-12.sc2", "PipeXter12").expect("blank");
    city.set_city_funds(28);
    city.set_terrain(10, 10, 0x0c);
    let mut state = test_state(city);
    state.tool = EditTool::Pipes;
    state.begin_underground_drag(10, 10, UndergroundKind::Pipes);

    assert_eq!(
        state.underground_drag_cost(),
        Some((UndergroundKind::Pipes, 28))
    );
    assert_eq!(
        state.finish_underground_drag(),
        Some((UndergroundKind::Pipes, 1))
    );
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.terrain_at(10, 10), 0x04);
}

#[test]
fn pe_pipe_stroke_commit_rewalks_after_neighbor_xund_reshape() {
    let mut city =
        City::new_blank("/tmp/sc2000-pipe-walk-reshape.sc2", "PipeWalkReshape").expect("blank");
    city.set_city_funds(100);
    city.set_terrain(11, 10, 0x05);
    city.set_underground(10, 11, 0x11);
    let mut state = test_state(city);
    state.tool = EditTool::Pipes;
    state.begin_underground_drag(10, 10, UndergroundKind::Pipes);
    state.update_underground_drag_to((13, 12));

    // The read-only pricing pass sees the preexisting `0x11` orientation and
    // keeps going south. Stamping the start tile reshapes its neighbor to
    // `0x10`; the commit walk then chooses east from that changed XUND state.
    assert_eq!(
        state.underground_drag_path_for_kind((10, 10), (13, 12), UndergroundKind::Pipes, true,),
        vec![(10, 10), (10, 11), (10, 12), (11, 12), (12, 12), (13, 12),]
    );
    assert_eq!(
        state.underground_drag_cost(),
        Some((UndergroundKind::Pipes, 18))
    );
    assert_eq!(
        state.finish_underground_drag(),
        Some((UndergroundKind::Pipes, 5))
    );
    assert_eq!(state.city.city_funds(), 82);
    assert_eq!(state.city.underground_at(10, 12), 0);
    for (x, y) in [(10, 10), (10, 11), (11, 11), (12, 11), (12, 12), (13, 12)] {
        assert_ne!(state.city.underground_at(x, y), 0, "({x}, {y})");
    }
}

#[test]
fn pe_pipe_stroke_walk_stops_on_tunnel_level_or_incompatible_xund() {
    let mut city =
        City::new_blank("/tmp/sc2000-pipe-walk-tunnel.sc2", "PipeWalkTunnel").expect("blank");
    city.set_tunnel_levels(11, 10, 1);
    assert_eq!(
        pe_pipe_stroke_path_448bc0_448f50(&city, (10, 10), (13, 10)),
        vec![(10, 10)]
    );

    city.set_tunnel_levels(11, 10, 0);
    city.set_underground(10, 11, 0x10);
    assert_eq!(
        pe_pipe_stroke_path_448bc0_448f50(&city, (10, 10), (10, 13)),
        vec![(10, 10)]
    );
}

#[test]
fn pe_pipe_walk_late_refusal_keeps_selected_direction() {
    let mut city =
        City::new_blank("/tmp/sc2000-pipe-walk-late-gate.sc2", "PipeLateGate").expect("blank");
    city.set_terrain(11, 10, 0x02);
    city.set_tunnel_levels(10, 11, 1);
    let walk = pe_pipe_stroke_walk_init_448bc0(&city, (10, 10), (13, 12)).expect("walk");

    let step =
        pe_pipe_next_stroke_tile_448f50(&city, (10, 10), walk.target, walk.bounds, walk.direction);
    assert_eq!(step.next, None);
    assert_eq!(
        step.direction, 2,
        "late tunnel refusal follows the alternate axis"
    );
}

#[test]
fn pe_pipe_stroke_refuses_military_zone_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-pipe-military.sc2", "PipeMilitary").expect("blank");
    city.set_zone(10, 10, ZONE_MILITARY | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Pipes;
    let path = [(10, 10)];

    let placement = state.underground_placement_for_path(&path, UndergroundKind::Pipes);
    assert!(placement.valid_tiles.is_empty());
    assert_eq!(placement.invalid_tiles, path);
    assert_eq!(
        state.place_underground_path(&path, UndergroundKind::Pipes),
        0
    );
    assert_eq!(state.city.underground_at(10, 10), 0);
}

#[test]
fn selected_win95_zone_tool_rejects_unaffordable_rectangle() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(10);
    let mut state = test_state(city);
    state.tool = EditTool::ZoneCommercial;

    assert_eq!(
        state.place_zone_rect((10, 10), (12, 10), ZONE_COMMERCIAL_LIGHT),
        0
    );
    assert_eq!(state.city.city_funds(), 10);
    assert_eq!(state.city.zone_at(10, 10).unwrap().zone_id(), 0);
    assert_eq!(state.city.zone_at(11, 10).unwrap().zone_id(), 0);
    assert_eq!(state.city.zone_at(12, 10).unwrap().zone_id(), 0);
}

#[test]
fn selected_highway_tool_builds_only_affordable_segments() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(100);
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    let path = highway_drag_path((10, 10), (14, 10));

    assert_eq!(state.place_highway_path(&path), 1);
    assert_eq!(state.city.city_funds(), 0);
    for y in 10..=11 {
        for x in 10..=11 {
            assert!(is_highway_tile(state.city.building_at(x, y)));
        }
        for x in 12..=15 {
            assert_eq!(state.city.building_at(x, y), 0);
        }
    }
}

#[test]
fn highway_refuses_xbld_ge_0x0e_radioactivity_and_xund() {
    // PE PlaceHighway AtCoordinates @ `0x44bef9`/`0x44bf25`/`0x44bf29`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-refuse.sc2", "HwyRefuse").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    // Flat XTER `0` is an invent CityTool retainer (PE AtCoordinates refuses).
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));

    state.city.set_building(20, 20, 0x0e);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_building(20, 20, 5);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, true, false));
    state.city.set_building(20, 20, 0);
    state.city.set_underground(20, 20, 0x10);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, true, false));
    state.city.set_underground(20, 20, 0);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    // Trees still OK (`< 0x0e`, not 5).
    state.city.set_building(20, 20, 6);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
}

#[test]
fn highway_one_category1_face_returns_class5_like_462c5b() {
    // NW `XTER 1` sets gate `al` bits 1 and 2 (`0x464310`). Empty mask
    // returns class 5 before the mask table (`0x462c5b`).
    let mut city = City::new_blank("/tmp/sc2000-hwy-cat1.sc2", "HwyCat1").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 1);
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    assert_eq!(state.city.building_at(20, 20), 0x62);
    assert_eq!(state.city.terrain_at(20, 20), 0x0d);
    assert_eq!(state.city.terrain_at(21, 20), 0x0d);
    assert_eq!(state.city.terrain_at(20, 21), 2);
    assert_eq!(state.city.terrain_at(21, 21), 2);
}

#[test]
fn highway_se_category1_face_returns_class7_like_462c6b() {
    // SE bit 4 is category 1, so gate `al` is `0x0c`. Empty mask hits bit 3
    // first and returns class 7 (`0x462c6b`), not class 5.
    let mut city = City::new_blank("/tmp/sc2000-hwy-se.sc2", "HwySE").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(21, 21, 1);
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    assert_eq!(state.city.building_at(20, 20), 0x64);
    assert_eq!(state.city.terrain_at(20, 20), 4);
    assert_eq!(state.city.terrain_at(21, 20), 4);
    assert_eq!(state.city.terrain_at(20, 21), 0x0d);
    assert_eq!(state.city.terrain_at(21, 21), 0x0d);
}

#[test]
fn highway_unequal_alt_si1_skips_dx_setup_returns_class4() {
    // `si != 0` jumps from `0x4642f9` to `0x464362`, skipping the equal-altitude
    // `dx` setup. NW higher with NE/SE category 1 sets only gate bit 0, so an
    // empty mask is class 4. The same faces at one altitude are `dx == 6`
    // (class 6). Stamp trailing Place equalizes land, so lock Get, not the stamp.
    let mut city = City::new_blank("/tmp/sc2000-hwy-si1.sc2", "HwySi1").expect("blank");
    for (x, y) in [(20, 20), (21, 20), (21, 21), (20, 21)] {
        city.set_altitude(x, y, 8, 8);
    }
    city.set_altitude(20, 20, 9, 8);
    city.set_terrain(21, 20, 1);
    city.set_terrain(21, 21, 1);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        4
    );
}

#[test]
fn highway_equal_alt_cx0e_category3_returns_class7() {
    // `cx == 0x0e` @ `0x464423`: NE category 3 sets gate bit 3. `dx == 6` alone
    // is empty-mask class 6; bit 3 is checked first, so the class is 7.
    let mut city = City::new_blank("/tmp/sc2000-hwy-cx0e.sc2", "HwyCx0e").expect("blank");
    city.set_terrain(21, 20, 9);
    city.set_terrain(21, 21, 1);
    city.set_terrain(20, 21, 5);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        7
    );
}

#[test]
fn highway_equal_alt_cx0d_category3_returns_class7() {
    // `cx == 0x0d` @ `0x46440b`: NW category 3 sets gate bit 3. `dx == 0x0d`
    // matches no equal-altitude mask, so without the OR the empty mask is class 2.
    let mut city = City::new_blank("/tmp/sc2000-hwy-cx0d.sc2", "HwyCx0d").expect("blank");
    city.set_terrain(20, 20, 9);
    city.set_terrain(21, 21, 1);
    city.set_terrain(20, 21, 1);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        7
    );
}

#[test]
fn highway_equal_alt_cx0b_category3_returns_class5() {
    // `cx == 0x0b` category-3 OR @ `0x4643f9` sets gate bit 1 when SW is
    // category 3. `dx == 8` alone would be class 7; the extra bit wins as class 5.
    let mut city = City::new_blank("/tmp/sc2000-hwy-cx0b.sc2", "HwyCx0b").expect("blank");
    city.set_terrain(20, 20, 5);
    city.set_terrain(21, 20, 5);
    city.set_terrain(20, 21, 9);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        5
    );
}

#[test]
fn highway_equal_alt_cx7_category3_returns_class6() {
    // Equal altitudes still fall through to the `cx == 7` category-3 OR
    // (`0x4643db`). NW `XTER 9` sets bit 2 of `al`, so empty mask is class 6.
    let mut city = City::new_blank("/tmp/sc2000-hwy-cx7.sc2", "HwyCx7").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 9);
    city.set_terrain(21, 20, 1);
    city.set_terrain(21, 21, 1);
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    assert_eq!(state.city.building_at(20, 20), 0x63);
    assert_eq!(state.city.terrain_at(20, 20), 3);
    assert_eq!(state.city.terrain_at(21, 20), 0x0d);
    assert_eq!(state.city.terrain_at(20, 21), 3);
    assert_eq!(state.city.terrain_at(21, 21), 0x0d);
}

#[test]
fn highway_sw_category1_face_returns_class7_not_4() {
    // SW bit 8 is `dx == 8`, which sets gate bits 0 and 3 (`al = 9`).
    // Empty mask checks bit 3 before bit 0, so this is class 7 not 4.
    let mut city = City::new_blank("/tmp/sc2000-hwy-sw.sc2", "HwySW").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 21, 1);
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    assert_eq!(state.city.building_at(20, 20), 0x64);
    assert_eq!(state.city.terrain_at(20, 20), 4);
    assert_eq!(state.city.terrain_at(21, 20), 4);
    assert_eq!(state.city.terrain_at(20, 21), 0x0d);
    assert_eq!(state.city.terrain_at(21, 21), 0x0d);
}

#[test]
fn highway_flat_refuses_xbld_0x0d_like_citytool_464040() {
    // PE CityToolPlaceHighway `0x464040` @ `0x4640c1`/`0x4622e4`: XBLD `0x0d` → refuse.
    // Slope AtCoordinates still allows `0x0d` (`< 0x0e`).
    let mut city = City::new_blank("/tmp/sc2000-hwy-0d.sc2", "Hwy0d").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x0d);
    let mut state = test_state(city);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_terrain(20, 20, 2);
    state.city.set_terrain(20, 19, 4);
    assert!(state.can_place_highway_slope_atcoordinates(20, 20));
}

#[test]
fn highway_flat_allows_xbld_0x0e_through_0x1e_like_citytool_464040() {
    // PE `0x464040` @ `0x4640d5`: `cmp ax,0x1e` / `jle` continue — flat allows
    // `0x0e..=0x1e`. Slope AtCoordinates still refuses `>= 0x0e`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-0e1e.sc2", "Hwy0e1e").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.city.set_building(20, 20, 0x0e);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x1e);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x1f);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x0e);
    state.city.set_terrain(20, 20, 2);
    state.city.set_terrain(20, 19, 4);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
}

#[test]
fn highway_flat_allows_xbld_0x2c_0x2d_like_citytool_464040() {
    // PE `0x464040` @ `0x4640e5`: `cmp ax,0x2d` / `jle` continue — flat allows
    // `0x2c`/`0x2d`; `0x1f..=0x2b` still refuse. Slope refuses `>= 0x0e`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-2c2d.sc2", "Hwy2c2d").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.city.set_building(20, 20, 0x2b);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x2c);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x2d);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x2c);
    state.city.set_terrain(20, 20, 2);
    state.city.set_terrain(20, 19, 4);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
}

#[test]
fn highway_flat_allows_xbld_0x61_through_0x6b_like_citytool_464040() {
    // PE `0x464040` @ `0x4640a9`–`0x4640b3`: early continue `0x61..=0x6b`.
    // Neighbors `0x60`/`0x6c` refuse; slope still refuses `>= 0x0e`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-61.sc2", "Hwy61").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.city.set_building(20, 20, 0x60);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x61);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x6b);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x6c);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x61);
    state.city.set_terrain(20, 20, 2);
    state.city.set_terrain(20, 19, 4);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
}

#[test]
fn highway_flat_allows_xbld_0x49_through_0x50_like_citytool_464040() {
    // PE `0x464040` @ `0x4640b5`–`0x4640bf`: early continue `0x49..=0x50`.
    // Neighbors `0x48`/`0x51` refuse; slope still refuses `>= 0x0e`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-49.sc2", "Hwy49").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.city.set_building(20, 20, 0x48);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x49);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x50);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x51);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 0x49);
    state.city.set_terrain(20, 20, 2);
    state.city.set_terrain(20, 19, 4);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
}

#[test]
fn place_pipes_xund_writes_on_military_zone() {
    // PE PlacePipes → PlaceXUND (`0x44b801`): military skips counter but still stamps.
    let mut city = City::new_blank("/tmp/sc2000-pipes-mil-xund.sc2", "PipeMilXu").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(20, 20, ZONE_MILITARY | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        1
    );
    assert_ne!(state.city.underground_at(20, 20), 0);
    assert_eq!(
        state.city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_MILITARY | ZONE_CORNER_ALL
    );
}

#[test]
fn pipe_drag_preserves_generated_plumbing_and_respects_saved_pipe_gate() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_zone(10, 10, 1);
    state.city.set_building(10, 10, 112);
    assert_eq!(building_internal_pipe_tile(&state.city, 10, 10), Some(16));

    let path = [(9, 10), (10, 10), (11, 10)];
    let preview = state.underground_drag_placement_for_path(&path, UndergroundKind::Pipes);
    assert_eq!(preview.valid_tiles, path);
    assert!(preview.invalid_tiles.is_empty());

    assert_eq!(
        state.place_underground_path(&path, UndergroundKind::Pipes),
        3
    );
    // PE mask table: one-sided endpoints reshape with mask 0 to 0x1e; the
    // center sees the completed EW pair and maps mask 0x0a to 0x11.
    assert_eq!(state.city.underground_at(9, 10), 0x1e);
    assert_eq!(state.city.underground_at(10, 10), 0x11);
    assert_eq!(state.city.underground_at(11, 10), 0x1e);
    assert_eq!(
        underground_render_tile(&state.city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: 0x11,
            source: UndergroundTileSource::UserLaid,
            watered: false,
        })
    );
    assert_eq!(
        underground_building_pipe_underlay_tile(&state.city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );
    assert_eq!(
        underground_building_pipe_overlay_tile(&state.city, 10, 10, 0),
        None
    );
    for side in [NETWORK_TOP, NETWORK_RIGHT, NETWORK_BOTTOM, NETWORK_LEFT] {
        assert!(water_pipe_tile_connects_to(&state.city, 10, 10, side));
    }

    state
        .city
        .set_underground(20, 10, UNDER_TILE_BUILDING_PIPES);
    state.city.set_building(20, 10, 112);
    let saved_path = [(19, 10), (20, 10), (21, 10)];
    let preview = state.underground_drag_placement_for_path(&saved_path, UndergroundKind::Pipes);
    assert_eq!(preview.valid_tiles, [(19, 10)]);
    assert_eq!(preview.invalid_tiles, [(20, 10)]);

    assert_eq!(
        state.place_underground_path(&saved_path, UndergroundKind::Pipes),
        1,
        "PE refuses occupied saved XUND 0x21 and stops the drag before it"
    );
    assert_eq!(state.city.underground_at(19, 10), 0x1e);
    assert_eq!(state.city.underground_at(20, 10), UNDER_TILE_BUILDING_PIPES);
    assert_eq!(
        underground_render_tile(&state.city, 20, 10, 0).map(|tile| tile.source),
        Some(UndergroundTileSource::BuildingInternal)
    );
    assert_eq!(state.city.underground_at(21, 10), 0);
}

#[test]
fn pipe_placement_preserves_saved_internal_building_plumbing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 112);
    city.set_tile_bits(40, 40, crate::city::TileBits::new(0b0010_0000));
    assert_eq!(
        underground_render_tile(&city, 40, 40, 0).map(|tile| tile.source),
        Some(UndergroundTileSource::BuildingInternal)
    );

    let mut state = test_state(city);
    assert_eq!(
        state.place_underground_path(&[(10, 10), (11, 10)], UndergroundKind::Pipes),
        2
    );

    assert!(state.city.tile_bits_at(40, 40).unwrap().piped());
    assert_eq!(
        underground_render_tile(&state.city, 40, 40, 0).map(|tile| tile.source),
        Some(UndergroundTileSource::BuildingInternal)
    );
}

#[test]
fn pipe_placement_preserves_generated_building_plumbing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 112);

    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    assert_eq!(
        underground_render_tile(&state.city, 10, 10, 0).map(|tile| tile.source),
        Some(UndergroundTileSource::BuildingInternal)
    );
    assert_eq!(
        state.place_underground_path(&[(9, 10), (10, 10), (11, 10)], UndergroundKind::Pipes),
        3
    );

    assert_eq!(state.city.underground_at(9, 10), 0x1e);
    assert_eq!(state.city.underground_at(10, 10), 0x11);
    assert_eq!(state.city.underground_at(11, 10), 0x1e);
    assert_eq!(
        underground_render_tile(&state.city, 10, 10, 0).map(|tile| tile.source),
        Some(UndergroundTileSource::UserLaid)
    );
    assert_eq!(
        underground_building_pipe_overlay_tile(&state.city, 10, 10, 0),
        None
    );
    assert_eq!(state.city.underground_at(9, 10), 0x1e);
    assert_eq!(state.city.underground_at(11, 10), 0x1e);
}

#[test]
fn maxis_land_drawing_disconnected_pipes_keeps_building_plumbing() {
    let path = paths::default_city_path();
    if !path.exists() {
        return;
    }
    let mut state = test_state(City::load(&path).expect("load default city"));
    state.refresh_simulation_service_bits();
    let before_mask = visible_building_pipe_mask(&state.city);

    let drawn = state.place_underground_path(
        &underground_preview_path_for_kind((45, 25), (54, 20), UndergroundKind::Pipes),
        UndergroundKind::Pipes,
    );

    let after_mask = visible_building_pipe_mask(&state.city);
    let drawn_visible =
        underground_preview_path_for_kind((45, 25), (54, 20), UndergroundKind::Pipes)
            .into_iter()
            .filter(|&(x, y)| {
                underground_render_tile(&state.city, x, y, state.city.rotation())
                    .is_some_and(|tile| tile.source == UndergroundTileSource::UserLaid)
            })
            .count();
    assert!(drawn > 0);
    let changed: Vec<_> = before_mask
        .iter()
        .zip(after_mask.iter())
        .enumerate()
        .filter(|&(_index, (&before, &after))| before != after)
        .map(|(index, (&before, &after))| {
            let x = index % CITY_WIDTH;
            let y = index / CITY_WIDTH;
            (
                x,
                y,
                before,
                after,
                state.city.building_at(x, y),
                state.city.underground_at(x, y),
                state
                    .city
                    .tile_bits_at(x, y)
                    .map(|bits| bits.raw_bits())
                    .unwrap_or(0),
                underground_render_tile(&state.city, x, y, state.city.rotation()),
                underground_building_pipe_underlay_tile(&state.city, x, y, state.city.rotation()),
                underground_building_pipe_overlay_tile(&state.city, x, y, state.city.rotation()),
            )
        })
        .collect();
    assert!(
        changed.is_empty(),
        "visible building plumbing changed after drawing disconnected pipe: {changed:?}"
    );
    assert_eq!(drawn_visible, drawn);
}

#[test]
fn maxis_land_drawing_through_building_plumbing_keeps_base_pipe_bit() {
    let path = paths::default_city_path();
    if !path.exists() {
        return;
    }
    let mut state = test_state(City::load(&path).expect("load default city"));
    state.refresh_simulation_service_bits();

    let Some((x, y)) = (1..CITY_HEIGHT - 1).find_map(|y| {
        (1..CITY_WIDTH - 1).find_map(|x| {
            let render = underground_render_tile(&state.city, x, y, state.city.rotation());
            (state.city.underground_at(x, y) == 0
                && render
                    .is_some_and(|tile| tile.source == UndergroundTileSource::BuildingInternal))
            .then_some((x, y))
        })
    }) else {
        return;
    };
    let before_bits = state.city.tile_bits_at(x, y).unwrap_or_default();

    assert_eq!(
        state.place_underground_path(&[(x - 1, y), (x, y), (x + 1, y)], UndergroundKind::Pipes),
        3
    );

    assert!(underground_user_pipe_tile(state.city.underground_at(x, y)));
    assert!(before_bits.piped());
    assert_eq!(
        underground_render_tile(&state.city, x, y, state.city.rotation()).map(|tile| tile.source),
        Some(UndergroundTileSource::UserLaid)
    );
}

#[test]
fn unzoned_multitile_utility_footprints_generate_building_plumbing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 20..24 {
        for x in 20..24 {
            city.set_building(x, y, 201);
            city.set_zone(x, y, 0);
        }
    }
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    for y in 20..24 {
        for x in 20..24 {
            assert!(
                building_internal_pipe_present(&state.city, x, y),
                "missing internal pipe presence at ({x},{y})"
            );
            assert_eq!(
                underground_render_tile(&state.city, x, y, 0).map(|tile| tile.source),
                Some(UndergroundTileSource::BuildingInternal),
                "missing rendered building pipe at ({x},{y})"
            );
        }
    }

    let mut anchored_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    anchored_city.set_building(43, 43, 201);
    anchored_city.set_zone(43, 43, 0);
    let mut anchored_state = test_state(anchored_city);
    anchored_state.refresh_simulation_service_bits();
    for y in 40..44 {
        for x in 40..44 {
            assert!(
                building_internal_pipe_present(&anchored_state.city, x, y),
                "missing anchored internal pipe presence at ({x},{y})"
            );
            assert_eq!(
                underground_render_tile(&anchored_state.city, x, y, 0).map(|tile| tile.source),
                Some(UndergroundTileSource::BuildingInternal),
                "missing anchored rendered building pipe at ({x},{y})"
            );
        }
    }
}

#[test]
fn user_laid_pipe_tiles_on_empty_land_take_precedence_over_saved_base_pipes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_underground(10, 10, 16);
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0b0010_0000));

    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: 16,
            source: UndergroundTileSource::UserLaid,
            watered: false,
        })
    );
    assert_eq!(
        underground_building_pipe_overlay_tile(&city, 10, 10, 0),
        None
    );
}

#[test]
fn user_laid_pipe_tiles_under_buildings_keep_base_pipe_underlay() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 112);
    city.set_underground(10, 10, 17);
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0b0010_0000));

    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: 17,
            source: UndergroundTileSource::UserLaid,
            watered: false,
        })
    );
    assert_eq!(
        underground_building_pipe_underlay_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );
    assert_eq!(
        underground_building_pipe_overlay_tile(&city, 10, 10, 0),
        None
    );
    assert!(city.tile_bits_at(10, 10).unwrap().piped());
}

#[test]
fn user_pipe_drag_stops_before_saved_building_pipe_xund() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.set_building(10, 10, 112);
    state
        .city
        .set_underground(10, 10, UNDER_TILE_BUILDING_PIPES);
    state
        .city
        .set_tile_bits(10, 10, crate::city::TileBits::new(0b0010_0000));

    assert_eq!(
        underground_render_tile(&state.city, 10, 10, 0).map(|tile| tile.source),
        Some(UndergroundTileSource::BuildingInternal)
    );

    assert_eq!(
        state.place_underground_path(&[(9, 10), (10, 10), (11, 10)], UndergroundKind::Pipes),
        1,
        "PE refuses saved building-pipe XUND 0x21 and commits only the valid prefix"
    );
    assert_eq!(state.city.underground_at(9, 10), 0x1e);
    assert_eq!(state.city.underground_at(10, 10), UNDER_TILE_BUILDING_PIPES);
    assert_eq!(state.city.underground_at(11, 10), 0);
    assert_eq!(
        underground_render_tile(&state.city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );
    assert_eq!(
        underground_building_pipe_overlay_tile(&state.city, 10, 10, 0),
        None
    );
}

#[test]
fn developed_special_zone_tiles_generate_building_plumbing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, ZONE_AIRPORT);
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();

    assert!(!building_internal_pipe_present(&state.city, 10, 10));
    assert_eq!(underground_render_tile(&state.city, 10, 10, 0), None);

    state
        .city
        .set_building(10, 10, AIRPORT_DEVELOPMENT_BUILDINGS[0]);
    state.refresh_simulation_service_bits();

    assert!(building_internal_pipe_present(&state.city, 10, 10));
    assert_eq!(
        underground_render_tile(&state.city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );
}

#[test]
fn underground_building_pipes_are_generated_only_when_xund_is_blank() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_zone(10, 10, 1);
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), None);

    city.set_building(10, 10, 112);
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), Some(16));

    city.set_underground(10, 10, 16);
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), None);

    city.set_building(20, 20, 140);
    city.set_building(21, 20, 140);
    city.set_building(20, 21, 140);
    city.set_building(21, 21, 140);
    assert_eq!(building_internal_pipe_tile(&city, 20, 20), Some(23));
}

#[test]
fn underground_empty_zoned_tiles_do_not_generate_building_pipes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for zone in 1..=9 {
        city.set_zone(10, 10, zone);
        assert_eq!(building_internal_pipe_tile(&city, 10, 10), None);
    }

    city.set_building(10, 10, 7);
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), None);

    city.set_building(10, 10, 112);
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), Some(16));

    city.set_zone(10, 10, 1);
    city.set_building(
        10,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), None);
}

#[test]
fn underground_building_pipes_shape_from_neighboring_plumbing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 10..=12 {
        for x in 10..=12 {
            city.set_zone(x, y, 1);
            city.set_building(x, y, 112);
        }
    }

    assert_eq!(building_internal_pipe_tile(&city, 11, 11), Some(30));
    assert_eq!(building_internal_pipe_tile(&city, 10, 11), Some(27));
    assert_eq!(building_internal_pipe_tile(&city, 11, 10), Some(28));
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), Some(23));
}

#[test]
fn cornered_multitile_building_footprints_generate_plumbing() {
    let cases = [
        (20, 20, 0x11, "top-left"),
        (23, 20, 0x21, "top-right"),
        (20, 23, 0x81, "bottom-left"),
        (23, 23, 0x41, "bottom-right"),
    ];
    for (anchor_x, anchor_y, zone, label) in cases {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_building(anchor_x, anchor_y, 201);
        city.set_zone(anchor_x, anchor_y, zone);

        assert!(
            building_internal_pipe_tile(&city, 20, 20).is_some(),
            "missing top-left footprint pipe for {label} anchor"
        );
        assert!(
            building_internal_pipe_tile(&city, 23, 23).is_some(),
            "missing bottom-right footprint pipe for {label} anchor"
        );
    }
}

#[test]
fn simulation_day1_persists_power_used_percent() {
    let mut state = water_consumer_fixture_for_calendar_test();
    state.refresh_live_misc_current_values();
    let water_used_before = state.city.misc_u32_at(MISC_WATER_USED_PERCENT_OFFSET);
    state.game_clock = GameClock::new_with_day(1900, 1, 0);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 1);
    assert_eq!(
        state.city.misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET),
        Some(power_used_percent_for_city(&state.city))
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_WATER_USED_PERCENT_OFFSET),
        water_used_before,
        "day 1 must preserve the saved water used% field"
    );
}

#[test]
fn pe_fa_road_band_exclusive_upper_0x2b_vs_0x2c_like_pe_45a110() {
    // PE DestroyBuilding @ `0x45a110`: network when XBLD `>= 0x1d && < 0x2c` (jl 0x2c).
    // Last road 0x2b → `0x4cc4d8`; first rail 0x2c → else-arm `0x4ca3f0`.
    let mut city = City::new_blank("/tmp/sc2000-fa-road-bound.sc2", "FaRoadBound").expect("blank");
    city.set_building(20, 20, 0x2b);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x2c);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn mac68k_pe_fa_road_band_exclusive_upper_0x2b_vs_0x2c_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-fa-road-bound-mac.sc2", "FaRoadBoundMac").expect("blank");
    city.set_building(20, 20, 0x2b);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x2c);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn pe_fa_power_band_exclusive_upper_0x60_vs_0x61_like_pe_45a140() {
    // PE DestroyBuilding @ `0x45a140`: network when XBLD `>= 0x5d && < 0x61` (jl 0x61).
    let mut city = City::new_blank("/tmp/sc2000-fa-pwr-bound.sc2", "FaPwrBound").expect("blank");
    city.set_building(20, 20, 0x60);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x61);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn mac68k_pe_fa_power_band_exclusive_upper_0x60_vs_0x61_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-fa-pwr-bound-mac.sc2", "FaPwrBoundMac").expect("blank");
    city.set_building(20, 20, 0x60);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x61);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn pe_fa_network_exact_tiles_0x4b_0x4c_like_pe_45a130() {
    // PE DestroyBuilding @ `0x45a130`/`0x45a138`: XBLD 0x4b / 0x4c jz → `0x4cc4d8`.
    let mut city = City::new_blank("/tmp/sc2000-fa-exact-4b4c.sc2", "FaExact4b4c").expect("blank");
    city.set_building(20, 20, 0x4b);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x4c);
    city.set_text_overlay(21, 20, 0xfa);
    city.set_building(22, 20, 0x4a); // neighbor outside exact jz arms
    city.set_text_overlay(22, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 2);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn mac68k_pe_fa_network_exact_tiles_0x4b_0x4c_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-fa-exact-4b4c-mac.sc2", "FaExact4b4cMac").expect("blank");
    city.set_building(20, 20, 0x4b);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x4c);
    city.set_text_overlay(21, 20, 0xfa);
    city.set_building(22, 20, 0x4d);
    city.set_text_overlay(22, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 2);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn bulldoze_clears_fa_xtxt_and_drops_4cc4d8_network_count() {
    // PE DestroyBuilding @ `0x45a109`: XTXT 0xfa cleared; network bands dec `0x4cc4d8`.
    let mut city = City::new_blank("/tmp/sc2000-bull-fa.sc2", "BullFa").expect("blank");
    city.set_building(20, 20, 0x1d);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&state.city), 0);
}

#[test]
fn bulldoze_clears_non_network_xtxt_fa_marker() {
    // PE DestroyBuilding else-arm @ `0x45a150`: FA outside network bands dec `0x4ca3f0`.
    let mut city = City::new_blank("/tmp/sc2000-bull-fa-sea.sc2", "BullFaSea").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(
        citygame_fa_non_network_neighbor_marker_count(&state.city),
        0
    );
}

#[test]
fn mac68k_bulldoze_clears_non_network_xtxt_fa_marker() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-fa-sea-mac.sc2", "BullFaSeaMac").expect("blank");
    city.set_building(20, 20, 0x0e);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(
        citygame_fa_non_network_neighbor_marker_count(&state.city),
        0
    );
}

#[test]
fn simulation_day22_offers_military_base_at_fourth_progression() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state.city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 3);
    state.city.set_rewards_available_mask(0);
    let population = PopulationStats {
        residential_population: 60_001,
        total_population: 60_001,
        populated_tiles: 1,
        ..PopulationStats::default()
    };
    let outcomes = live_city_service_outcomes_for_city(&state.city);

    let day22 = state.sim_calendar_day22_scenario(false, population, outcomes);

    assert!(day22.status_announced);
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET),
        Some(4)
    );
    assert_eq!(state.city.rewards_available_mask(), 0);
    assert!(state.military_base_dialog.is_some());
    assert_eq!(state.query_line, "Military Base Offered");
}

#[test]
fn stamp_military_base_plot_transfers_dw_to_military_word0() {
    // PE @ `0x4144d8`/`0x414512`: dec `dwTileCount[old]`, inc `0x4ca934` word 0.
    let mut city = City::new_blank("/tmp/sc2000-mil-word0.sc2", "MilW0").expect("blank");
    let vacant_before = city.pe_dw_tile_count[0];
    let mut state = test_state(city);
    state.stamp_military_base_plot(40, 40);
    let stamped = (0..8)
        .flat_map(|dy| (0..8).map(move |dx| (40 + dx, 40 + dy)))
        .filter(|&(x, y)| {
            state
                .city
                .zone_at(x, y)
                .is_some_and(|z| z.zone_id() == ZONE_MILITARY)
        })
        .count();
    assert!(stamped > 0);
    assert_eq!(
        state.city.pe_dw_tile_count[0],
        vacant_before.wrapping_sub(stamped as u16)
    );
    assert_eq!(state.city.pe_military_tile_count[0], stamped as u16);
}

#[test]
fn stamp_military_base_plot_preserves_xtxt() {
    // Military zone stamp clears small XBLD only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-mil-xtxt.sc2", "MilT").expect("blank");
    city.set_text_overlay(40, 40, 0x44);
    let mut state = test_state(city);
    state.stamp_military_base_plot(40, 40);
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0x44));
}

#[test]
fn mac68k_stamp_military_base_plot_preserves_xtxt() {
    // Shared military zone stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-mil-xtxt.sc2", "MacMilT").expect("blank");
    city.set_text_overlay(40, 40, 0x44);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_military_base_plot(40, 40);
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0x44));
}

#[test]
fn mac68k_stamp_military_base_plot_preserves_xtrf() {
    // Shared military zone stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-mil-xtrf.sc2", "MacMilX").expect("blank");
    city.set_traffic_density(40, 40, 0x55);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_military_base_plot(40, 40);
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.zone_id()),
        Some(ZONE_MILITARY)
    );
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x55));
}

#[test]
fn try_place_missile_silos_place_tile_and_xund_footprint() {
    // PE @ `0x438def`/`0x438e0a`: 3×3 PlaceTile(0xF9) + PlaceXUND(0x22) + rotation corners.
    let mut city = City::new_blank("/tmp/sc2000-silo-stamp.sc2", "SiloStamp").expect("blank");
    for y in 0..3 {
        for x in 0..3 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    city.set_view_rotation(0);
    let vacant_before = city.pe_dw_tile_count[0];
    let mut state = test_state(city);
    assert!(state.try_place_missile_silos(true));
    let corner_bits = pe_choose_and_place_corner_bits(0);
    for y in 0..3usize {
        for x in 0..3usize {
            assert_eq!(state.city.building_at(x, y), MILITARY_SILO_BUILDING_ID);
            assert_eq!(state.city.underground_at(x, y), UNDER_TILE_GREY_BOX_1);
            let expect = ZONE_MILITARY | building_zone_bits_for_tile(0, 0, x, y, 3, corner_bits);
            assert_eq!(state.city.zone_at(x, y).unwrap().raw_bits(), expect);
        }
    }
    // Military PlaceTile path: silo `0xf9` → `0x4ca934` word 15, not dwTileCount.
    let mut silo_tiles = 0u16;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if state.city.building_at(x, y) == MILITARY_SILO_BUILDING_ID {
                silo_tiles = silo_tiles.wrapping_add(1);
            }
        }
    }
    assert!(silo_tiles >= 9);
    assert_eq!(
        state.city.pe_dw_tile_count[0],
        vacant_before.wrapping_sub(silo_tiles)
    );
    assert_eq!(
        state.city.pe_dw_tile_count[MILITARY_SILO_BUILDING_ID as usize],
        0
    );
    assert_eq!(state.city.pe_military_tile_count[15], silo_tiles);
    assert_eq!(state.city.pe_military_tile_count[0], 0);
}

#[test]
fn mac68k_try_place_missile_silos_place_tile_and_xund_footprint() {
    // Shared PE silo PlaceTile/PlaceXUND stamp; Mac68k must not invent clears.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-silo-stamp.sc2", "MacSiloStamp").expect("blank");
    for y in 0..3 {
        for x in 0..3 {
            city.set_altitude(x, y, 8, 0);
            city.set_text_overlay(x, y, 0x2a);
            city.set_traffic_density(x, y, 0x18);
            city.set_tile_bits(x, y, TileBits::new(0).with_watered(true));
        }
    }
    city.set_view_rotation(1);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.try_place_missile_silos(true));
    let corner_bits = pe_choose_and_place_corner_bits(1);
    for y in 0..3usize {
        for x in 0..3usize {
            assert_eq!(state.city.building_at(x, y), MILITARY_SILO_BUILDING_ID);
            assert_eq!(state.city.underground_at(x, y), UNDER_TILE_GREY_BOX_1);
            assert_eq!(state.city.text_overlay_at(x, y), Some(0x2a));
            assert_eq!(state.city.traffic_density_at(x, y), Some(0x18));
            assert!(state.city.tile_bits_at(x, y).unwrap().watered());
            let expect = ZONE_MILITARY | building_zone_bits_for_tile(0, 0, x, y, 3, corner_bits);
            assert_eq!(state.city.zone_at(x, y).unwrap().raw_bits(), expect);
        }
    }
}

#[test]
fn try_place_missile_silos_preserves_xtxt() {
    // Missile-silo PlaceTile leaves XTXT.
    let mut city = City::new_blank("/tmp/sc2000-silo-xtxt.sc2", "SiloT").expect("blank");
    for y in 0..3 {
        for x in 0..3 {
            city.set_altitude(x, y, 8, 0);
            city.set_text_overlay(x, y, 0x45);
        }
    }
    let mut state = test_state(city);
    assert!(state.try_place_missile_silos(true));
    assert_eq!(state.city.text_overlay_at(0, 0), Some(0x45));
}

#[test]
fn mac68k_try_place_missile_silos_preserves_xtxt() {
    // Shared missile-silo zone stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-silo-xtxt.sc2", "MacSiloT").expect("blank");
    for y in 0..3 {
        for x in 0..3 {
            city.set_altitude(x, y, 8, 0);
            city.set_text_overlay(x, y, 0x45);
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.try_place_missile_silos(true));
    assert_eq!(state.city.text_overlay_at(0, 0), Some(0x45));
}

#[test]
fn mac68k_try_place_missile_silos_preserves_xtrf() {
    // Shared missile-silo zone stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-silo-xtrf.sc2", "MacSiloX").expect("blank");
    for y in 0..3 {
        for x in 0..3 {
            city.set_altitude(x, y, 8, 0);
            city.set_traffic_density(x, y, 0x61);
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.try_place_missile_silos(true));
    assert_eq!(state.city.traffic_density_at(0, 0), Some(0x61));
    assert_eq!(
        state.city.zone_at(0, 0).map(|z| z.zone_id()),
        Some(ZONE_MILITARY)
    );
}

#[test]
fn pe_military_base_fallback_matches_gates_and_allows_repeated_plots() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-military-fallback.sc2", "MilFallback").expect("blank");
    for y in 20..23 {
        for x in 30..33 {
            city.set_altitude(x, y, 8, 0);
            city.set_building(x, y, 1);
        }
    }
    // The PE checks underground only at the sampled origin, not each footprint tile.
    city.set_underground(31, 21, 0x22);
    city.set_zone(31, 21, 0x51);
    assert!(pe_military_base_fallback_plot_is_candidate(
        &city, 30, 20, 8
    ));
    assert!(!pe_military_base_fallback_plot_is_candidate(
        &city, 31, 21, 8
    ));

    // Candidate collection is completed before stamping, so identical/overlapping
    // random samples remain six entries, matching the PE's local array behavior.
    let plots = select_pe_military_base_fallback_plots(
        &city,
        8,
        std::iter::repeat_n((30, 20), MILITARY_BASE_FALLBACK_PATCH_COUNT),
    )
    .expect("six repeated plots satisfy the six-entry threshold");
    assert_eq!(plots.len(), MILITARY_BASE_FALLBACK_PATCH_COUNT);
    assert!(plots.iter().all(|&plot| plot == (30, 20)));

    let vacant_before = city.pe_dw_tile_count[1];
    let military_before = city.pe_military_tile_count[0];
    for &(x, y) in &plots {
        stamp_pe_military_base_fallback_plot(&mut city, x, y);
    }
    assert_eq!(city.pe_dw_tile_count[1], vacant_before.wrapping_sub(54));
    assert_eq!(
        city.pe_military_tile_count[0],
        military_before.wrapping_add(54)
    );
    assert_eq!(
        city.building_at(30, 20),
        1,
        "fallback stamps zone without PlaceTile"
    );
    assert_eq!(city.zone_at(31, 21).unwrap().raw_bits(), 0x57);
}

#[test]
fn pe_military_base_fallback_rejects_every_tile_gate_and_origin_underground() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-military-fallback-gates.sc2", "MilGates").expect("blank");
    for y in 20..23 {
        for x in 30..33 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    assert!(pe_military_base_fallback_plot_is_candidate(
        &city, 30, 20, 8
    ));
    city.set_building(31, 21, SMALL_PARK_BUILDING_ID);
    assert!(!pe_military_base_fallback_plot_is_candidate(
        &city, 30, 20, 8
    ));
    city.set_building(31, 21, 0);
    city.set_text_overlay(31, 21, 1);
    assert!(!pe_military_base_fallback_plot_is_candidate(
        &city, 30, 20, 8
    ));
    city.set_text_overlay(31, 21, 0);
    city.set_tile_bits(31, 21, TileBits::new(0x04));
    assert!(!pe_military_base_fallback_plot_is_candidate(
        &city, 30, 20, 8
    ));
    city.set_tile_bits(31, 21, TileBits::new(0));
    city.set_altitude(31, 21, 9, 0);
    assert!(!pe_military_base_fallback_plot_is_candidate(
        &city, 30, 20, 8
    ));
    city.set_altitude(31, 21, 8, 0);
    city.set_zone(31, 21, ZONE_MILITARY);
    assert!(!pe_military_base_fallback_plot_is_candidate(
        &city, 30, 20, 8
    ));
    city.set_zone(31, 21, 0);
    city.set_underground(30, 20, 1);
    assert!(!pe_military_base_fallback_plot_is_candidate(
        &city, 30, 20, 8
    ));
}

#[test]
fn pe_military_base_sampler_returns_last_underfilled_sample_for_fallback() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-military-underfilled.sc2", "MilUnder").expect("blank");
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_building(x, y, SMALL_PARK_BUILDING_ID);
        }
    }
    let mut game_rng = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
    let sampled = sample_military_base_plot(&city, &mut game_rng)
        .expect("the final 8×8 sample is retained after 24 underfilled probes");
    assert_eq!((sampled.2, sampled.3), (0, 0));
}

#[test]
fn military_base_offer_accept_places_air_force_plot() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.open_military_base_dialog();

    assert!(state.accept_military_base_offer());
    assert!(state.military_base_dialog.is_none());
    assert_eq!(
        state.city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET),
        Some(MILITARY_BASE_AIR_FORCE)
    );
    let military_tiles = (0..CITY_HEIGHT)
        .flat_map(|y| (0..CITY_WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            state
                .city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
        })
        .count();
    assert!(
        military_tiles >= MILITARY_BASE_MIN_VALID_TILES,
        "expected zoned military plot, got {military_tiles} tiles"
    );
    assert!(
        (0..CITY_HEIGHT)
            .flat_map(|y| (0..CITY_WIDTH).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                state
                    .city
                    .zone_at(x, y)
                    .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
            })
            .all(|(x, y)| {
                state
                    .city
                    .zone_at(x, y)
                    .is_some_and(|zone| zone.raw_bits() & ZONE_CORNER_ALL == ZONE_CORNER_ALL)
            }),
        "military plot cells need ZONE_CORNER_ALL like vacant RCI"
    );
    assert_eq!(state.query_line, "Air Force Base Established");
}

#[test]
fn military_base_offer_decline_marks_declined() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.open_military_base_dialog();

    state.decline_military_base_offer();

    assert!(state.military_base_dialog.is_none());
    assert_eq!(
        state.city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET),
        Some(MILITARY_BASE_DECLINED)
    );
    assert_eq!(state.query_line, "Military Base Declined");
}

#[test]
fn military_base_offer_accept_places_missile_silos_when_no_land_plot() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    // Fill the map so no 8×8 PlotCheck can succeed, but leave six spaced 3×3 clearings.
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            state.city.set_building(x, y, SMALL_PARK_BUILDING_ID);
        }
    }
    let silo_origins = [(0usize, 0usize), (8, 0), (16, 0), (0, 8), (8, 8), (16, 8)];
    for &(ox, oy) in &silo_origins {
        for y in oy..oy + MILITARY_SILO_PLOT_SIZE {
            for x in ox..ox + MILITARY_SILO_PLOT_SIZE {
                state.city.set_building(x, y, 0);
            }
        }
    }
    state.open_military_base_dialog();

    assert!(state.accept_military_base_offer());
    assert_eq!(
        state.city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET),
        Some(MILITARY_BASE_MISSILE_SILOS)
    );
    assert_eq!(state.query_line, "Missile Silos Established");
    let military_tiles = (0..CITY_HEIGHT)
        .flat_map(|y| (0..CITY_WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            state
                .city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
        })
        .count();
    assert_eq!(
        military_tiles,
        MILITARY_MAX_SILOS * MILITARY_SILO_PLOT_SIZE * MILITARY_SILO_PLOT_SIZE
    );
}

#[test]
fn military_base_offer_accept_skips_navy_when_coin_fails() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    for y in 0..CITY_HEIGHT {
        for x in 0..4 {
            state.city.set_terrain(x, y, 0x10);
            state
                .city
                .set_tile_bits(x, y, crate::city::TileBits::new(0b0000_0001));
        }
    }
    // CRT seed 3 → first `random_mod(2) == 0` (naval coin loses → land AF).
    state.pe_process_crt = crate::win95_random::Win95CrtRand::new(3);
    state.open_military_base_dialog();

    assert!(state.accept_military_base_offer());
    assert_eq!(
        state.city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET),
        Some(MILITARY_BASE_AIR_FORCE)
    );
    assert_eq!(state.query_line, "Air Force Base Established");
}

#[test]
fn military_base_offer_accept_queues_auto_goto_center() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.auto_goto_enabled = true;
    state.open_military_base_dialog();

    assert!(state.accept_military_base_offer());
    let goto = state
        .take_pending_disaster_auto_goto()
        .expect("military AutoGoto");
    let military_tiles: Vec<_> = (0..CITY_HEIGHT)
        .flat_map(|y| (0..CITY_WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            state
                .city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
        })
        .collect();
    assert!(!military_tiles.is_empty());
    let min_x = military_tiles.iter().map(|t| t.0).min().unwrap();
    let min_y = military_tiles.iter().map(|t| t.1).min().unwrap();
    let max_x = military_tiles.iter().map(|t| t.0).max().unwrap();
    let max_y = military_tiles.iter().map(|t| t.1).max().unwrap();
    assert!(
        goto.0 >= min_x && goto.0 <= max_x && goto.1 >= min_y && goto.1 <= max_y,
        "AutoGoto {goto:?} outside military plot ({min_x},{min_y})..({max_x},{max_y})"
    );
}

#[test]
fn military_base_accept_then_grows_air_force_buildings() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.open_military_base_dialog();
    assert!(state.accept_military_base_offer());
    assert_eq!(
        state.city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET),
        Some(MILITARY_BASE_AIR_FORCE)
    );

    let (ox, oy) = (0..CITY_HEIGHT)
        .flat_map(|y| (0..CITY_WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            state
                .city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
                && state.city.building_at(x, y) < SMALL_PARK_BUILDING_ID
        })
        .min_by_key(|&(x, y)| (y, x))
        .expect("vacant military tile");
    power_adjacent_building_for_test(&mut state.city, ox, oy.saturating_sub(1));

    assert!(
        state.develop_special_zones() > 0,
        "accept→grow should stamp military buildings"
    );
    assert!(
        (0..CITY_HEIGHT).any(|y| {
            (0..CITY_WIDTH).any(|x| {
                state
                    .city
                    .zone_at(x, y)
                    .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
                    && state.city.building_at(x, y) == RUNWAY_BUILDING_ID
            })
        }),
        "empty air-force base should grow a runway"
    );
}

#[test]
fn pe_military_count_sar_div4_matches_43631a() {
    // PE @ `0x436319`/`0x435e25`: `cdq; and edx,3; add; sar 2` → truncating `/4`.
    assert_eq!(pe_military_count_sar_div4(0), 0);
    assert_eq!(pe_military_count_sar_div4(1), 0);
    assert_eq!(pe_military_count_sar_div4(3), 0);
    assert_eq!(pe_military_count_sar_div4(4), 1);
    assert_eq!(pe_military_count_sar_div4(5), 1);
    assert_eq!(pe_military_count_sar_div4(7), 1);
    assert_eq!(pe_military_count_sar_div4(8), 2);
}

#[test]
fn january_aged_plant_demolish_uses_directional_square_from_interior_xtxt() {
    // PE's DestroyBuilding mode-1 rubble pass is anchored at the first XTXT
    // hit coordinate (x..x+3, y..y-3), not a normalized footprint origin.
    let mut city = City::new_blank("/tmp/sc2000-jan-plant.sc2", "JanPlant").expect("blank");
    city.set_city_funds(50_000);
    let coal = city.provision_microsim_for_building(207).expect("coal");
    let coal_index = usize::from(coal - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(coal_index, 50));
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_building(10 + dx, 20 + dy, 207);
        }
    }
    // Only the interior cell carries the microsim XTXT (hit tile).
    city.set_text_overlay(11, 21, coal);
    city.set_traffic_density(11, 21, 0x22);
    city.set_tile_bits(11, 21, TileBits::new(0).with_piped(true).with_watered(true));
    city.refresh_structure_microsim_yearly(0, 100, 100, 100, 100, 100);
    assert_eq!(city.microsim_at(coal_index).map(|r| r.tile_id), Some(0));
    for dy in 18..=21 {
        for dx in 11..=14 {
            assert!(
                (1..=4).contains(&city.building_at(dx, dy)),
                "PE directional rubble cell ({dx},{dy})"
            );
        }
    }
    for dy in 22..=23 {
        for dx in 10..=13 {
            assert_eq!(
                city.building_at(dx, dy),
                207,
                "PE hit-anchored square leaves ({dx},{dy}) untouched"
            );
        }
    }
    assert_eq!(city.text_overlay_at(11, 21), Some(0));
    assert_eq!(city.traffic_density_at(11, 21), Some(0x22));
    let bits = city.tile_bits_at(11, 21).unwrap();
    assert!(bits.piped() && bits.watered());
}

#[test]
fn simulation_day0_january_opens_extra_when_aging_plant_story_queued() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    ensure_default_city_options(&mut city);
    assert!(set_city_auto_budget_enabled(&mut city, true));
    assert!(newspaper_extra_enabled_for_city(&city));
    let coal = city.provision_microsim_for_building(207).expect("coal");
    let coal_index = usize::from(coal - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(coal_index, 48));
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Llama);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.month(), 1);
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Extra)),
        "PE Extra opens for story 0x24 when bNewspaperExtra is on"
    );
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x24));
    assert!(!state.city.take_pe_newspaper_extra_popup_pending());
}

#[test]
fn pe_attitude_developed_tiles_count_buildings_and_zones() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "devtiles").expect("blank");
    city.set_zone(10, 10, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(12, 10, RESIDENTIAL_1X1_BUILDINGS[0]);
    let population = live_population_stats_for_city(&city);
    let (developed, pollution, land, crime) = pe_attitude_demand_inputs_for_city(&city, population);
    assert_eq!(
        developed, 2,
        "vacant XZON nibble + XBLD>=0x1D each increment 0x4c93b4"
    );
    let outcomes = live_city_service_outcomes_for_city(&city);
    let counters = live_city_scenario_counters_for_city(&city, population, outcomes);
    assert_eq!(
        pollution, city.pe_dw_city_pollution,
        "without XPLT, attitude score 1 reads the PE pollution BSS total"
    );
    assert_eq!(
        crime, city.pe_dw_city_crime,
        "without XCRM, attitude score 3 reads the PE crime BSS total"
    );
    assert_eq!(land, 0, "no XVAL map or MISC land value");

    city.set_pe_attitude_demand_inputs(0, 0, 0, 0);
    city.recompute_pe_attitude_scores(100, 80, 70);
    assert_eq!(
        [
            city.pe_attitude_score_at(1),
            city.pe_attitude_score_at(2),
            city.pe_attitude_score_at(3)
        ],
        [0, 0, 0],
        "stored zeros must not fall back to funding stand-ins"
    );
}

#[test]
fn january_marina_boats_scale_by_tiles_and_pop() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 2, 2, 248, 3);
    city.provision_microsim_for_building(248).expect("marina");
    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let marina = city
        .microsim_at(crate::city::MICROSIM_SYSTEM_MARINA)
        .expect("marina row");
    assert_eq!(
        marina.stat1, 46,
        "min(pop/150, LFSR%20 + 9×8); 72+jitter exceeds 46"
    );
}

#[test]
fn rail_depot_spawn_capacity_uses_dw_tile_count_like_pe_435e1e() {
    // PE @ `0x435e1e`: capacity from `dwTileCount[0xed]/4`, not a live XBLD rescan.
    let mut city = City::new_blank("/tmp/sc2000-depot-census.sc2", "DepCensus").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, RAIL_STATION_BUILDING_ID, 2);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_saved_powered_xbit_for_test(&mut city, 20, 20);
    city.set_building(22, 20, 45);
    city.set_building(23, 20, 45);
    assert!(city.pe_dw_tile_count[RAIL_STATION_BUILDING_ID as usize] >= 4);
    city.pe_dw_tile_count[RAIL_STATION_BUILDING_ID as usize] = 0;
    let mut state = test_state(city);
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(20, 20, true),
        0,
        "stale zero census must refuse even when depot tiles remain on the map"
    );
    state.city.pe_dw_tile_count[RAIL_STATION_BUILDING_ID as usize] = 4;
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(20, 20, true),
        1,
        "census units >=1 must allow spawn when track/power gates pass"
    );
}

#[test]
fn marina_spawn_capacity_uses_dw_tile_count_like_pe_435e1e_twin() {
    // PE marina twin: capacity from `dwTileCount[0xf8]/9`, not a live XBLD rescan.
    let mut city = City::new_blank("/tmp/sc2000-marina-census.sc2", "MarCensus").expect("blank");
    stamp_complete_building_for_test(&mut city, 30, 30, MARINA_BUILDING_ID, 3);
    power_adjacent_building_for_test(&mut city, 30, 29);
    set_saved_powered_xbit_for_test(&mut city, 30, 30);
    city.set_tile_bits(29, 30, crate::city::TileBits::new(0x04));
    city.set_terrain(29, 30, 0x10);
    city.set_building(29, 30, 0);
    assert!(city.pe_dw_tile_count[MARINA_BUILDING_ID as usize] >= 9);
    city.pe_dw_tile_count[MARINA_BUILDING_ID as usize] = 0;
    let mut state = test_state(city);
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(30, 30, true),
        0,
        "stale zero census must refuse even when marina tiles remain on the map"
    );
    state.city.pe_dw_tile_count[MARINA_BUILDING_ID as usize] = 9;
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(30, 30, true),
        1,
        "census units >=1 must allow spawn when water/power gates pass"
    );
}

#[test]
fn rail_depot_spawn_retries_next_face_when_direction_fails() {
    // PE `0x453590` loops faces until SpawnTrain succeeds. North rail with no
    // outbound direction must not block east-face spawn (and must not be the
    // sole candidate the way a first-face-only picker would stop).
    let mut city = City::new_blank("/tmp/sc2000-train-retry.sc2", "Retry").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, RAIL_STATION_BUILDING_ID, 2);
    power_adjacent_building_for_test(&mut city, 21, 19);
    set_saved_powered_xbit_for_test(&mut city, 20, 20);
    // North face: rail + clear XTXT, but no neighbor rail for FUN_00402671.
    city.set_building(20, 19, 45);
    city.set_text_overlay(20, 19, 0);
    // East face: clear rail with continuation.
    city.set_building(22, 20, 45);
    city.set_building(23, 20, 45);
    city.set_text_overlay(22, 20, 0);
    let mut state = test_state(city);
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(20, 20, true),
        1
    );
    assert!(
        state.city.ambient_port_actor_at(22, 20).is_some(),
        "must spawn on east after north direction fail"
    );
    assert!(
        state.city.ambient_port_actor_at(20, 19).is_none(),
        "north face must not receive the consist"
    );
}

#[test]
fn marina_spawn_retries_next_face_when_xbld_blocks() {
    // PE `0x453380` loops faces; north water with XBLD!=0 must not block west water.
    let mut city = City::new_blank("/tmp/sc2000-marina-retry.sc2", "MarRetry").expect("blank");
    stamp_complete_building_for_test(&mut city, 30, 30, MARINA_BUILDING_ID, 3);
    power_adjacent_building_for_test(&mut city, 30, 29);
    set_saved_powered_xbit_for_test(&mut city, 30, 30);
    // North face kept as coal feeder (nonzero XBLD) — scanner still sees water? No —
    // feeder is not water. Plant occupied west? Use west built water + south clear.
    // Faces: N(30,29)=coal, E(31,30)=built water, S(30,31)=clear water, W(29,30).
    city.set_tile_bits(31, 30, crate::city::TileBits::new(0x04));
    city.set_terrain(31, 30, 0x10);
    city.set_building(31, 30, 45);
    city.set_text_overlay(31, 30, 0);
    city.set_tile_bits(30, 31, crate::city::TileBits::new(0x04));
    city.set_terrain(30, 31, 0x10);
    city.set_building(30, 31, 0);
    city.set_text_overlay(30, 31, 0);
    let mut state = test_state(city);
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(30, 30, true),
        1
    );
    assert!(state.city.ambient_port_actor_at(30, 31).is_some());
    assert!(state.city.ambient_port_actor_at(31, 30).is_none());
}

#[test]
fn unpowered_rail_depot_skips_random_and3_lfsr_pe_435e09() {
    // PE @ `0x435e09` tests XBIT before `0x402261` RandomAnd3 @ `0x435e0f`.
    // Unpowered depot must not advance the process LFSR.
    let mut city = City::new_blank("/tmp/sc2000-rail-lfsr.sc2", "RailLfsr").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, RAIL_STATION_BUILDING_ID, 2);
    city.set_building(22, 20, 45);
    let mut state = test_state(city);
    let before = state.pe_process_lfsr_state();
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(20, 20, false),
        0
    );
    assert_eq!(
        state.pe_process_lfsr_state(),
        before,
        "unpowered rail depot must not call RandomAnd3"
    );
}

#[test]
fn unpowered_marina_skips_random_and3_lfsr_pe_435e73() {
    // PE @ `0x435e73` tests XBIT before RandomAnd3 @ `0x435e79`.
    let mut city = City::new_blank("/tmp/sc2000-marina-lfsr.sc2", "MarLfsr").expect("blank");
    stamp_complete_building_for_test(&mut city, 30, 30, MARINA_BUILDING_ID, 3);
    let mut state = test_state(city);
    let before = state.pe_process_lfsr_state();
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(30, 30, false),
        0
    );
    assert_eq!(
        state.pe_process_lfsr_state(),
        before,
        "unpowered marina must not call RandomAnd3"
    );
}

#[test]
fn rail_depot_spawn_requires_saved_xbit_even_on_derived_energized_network() {
    // PE @ `0x435e09`: test the station's saved XBIT directly. A connected
    // derived network cannot substitute while that published bit is clear.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    power_adjacent_building_for_test(&mut city, 5, 20);
    // 40 structural consumers in BFS order before the depot (coal capacity = 40).
    for x in 6..46 {
        city.set_building(x, 20, 0x70);
    }
    stamp_complete_building_for_test(&mut city, 46, 20, RAIL_STATION_BUILDING_ID, 2);
    // East rail for SpawnTrain; rails are non-consumers so they do not steal ration.
    city.set_building(48, 20, 45);
    city.set_building(49, 20, 45);
    let power_maps = power_service_maps(&city);
    assert!(
        power_maps.energized_tiles[20 * CITY_WIDTH + 46],
        "depot must be on the energized network"
    );
    assert!(
        !power_maps.powered_tiles[20 * CITY_WIDTH + 46],
        "depot must be past coal capacity ration"
    );
    let mut state = test_state(city);
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(46, 20, true),
        0,
        "derived connectivity must not override saved XBIT 0x40"
    );
    let saved_bits = state.city.tile_bits_at(46, 20).unwrap_or_default();
    state
        .city
        .set_tile_bits(46, 20, saved_bits.with_powered(true));
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(46, 20, true),
        1,
        "saved XBIT 0x40 must pass even when consumer capacity is exhausted"
    );
}

#[test]
fn mac68k_extent2_bridge_connected_run_uses_topology_stride_and_per_cell_draws() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-bridge-extent2-run.sc2", "MacBridgeRun").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 0);
    city.set_mac_state_misc_u32_at(0x64, 255);
    // The probe at (20,20) returns 0x0f; its low bit chooses the two-cell Y
    // stride. A second ID at (20,18) makes the backward search stop at y=16.
    city.set_building(20, 20, 0x6a);
    city.set_building(20, 18, 0x6b);

    let seed = 0x2468_1357;
    let mut expected_rng = MacToolboxRng::new(seed);
    for _ in 0..11 {
        expected_rng.random_u16();
    }
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    for (x, y) in [
        (20, 18),
        (20, 19),
        (21, 19),
        (21, 18),
        (20, 20),
        (20, 21),
        (21, 21),
        (21, 20),
    ] {
        assert_eq!(city.building_at(x, y), 0, "cell ({x},{y})");
    }
    assert_eq!(city.mac_xbld_tile_count(0x6a), 0);
    assert_eq!(city.mac_xbld_tile_count(0x6b), 0);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac68k_extent2_bridge_special_topology_adds_three_flip_draws_per_cell() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-bridge-extent2-special.sc2",
            "MacBridgeSpec",
        )
        .expect("blank");
        city.kind = kind;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 7, 0);
        city.set_mac_state_misc_u32_at(0x64, 255);
        city.set_building(20, 20, 0x6a);
        city.set_building(18, 20, 0x6b);
        for (x, y) in [(20, 20), (18, 20)] {
            city.set_zone(x, y, 0xf0);
            city.set_tile_bits(x, y, TileBits::new(0x04));
        }
        // The 0x0e connected-run class refreshes the other three cells through
        // export 138 before PlaceTile; its XUND cleanup is a gameplay side effect.
        city.set_underground(19, 20, 1);

        let seed = 0x1020_3040;
        let mut expected_rng = MacToolboxRng::new(seed);
        for _ in 0..11 {
            expected_rng.random_u16();
        }
        let mut rng = MacToolboxRng::new(seed);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        for (x, y) in [
            (18, 20),
            (18, 21),
            (19, 21),
            (19, 20),
            (20, 20),
            (20, 21),
            (21, 21),
            (21, 20),
        ] {
            assert_eq!(city.building_at(x, y), 0, "{kind:?}, cell ({x},{y})");
        }
        assert_eq!(city.underground_at(19, 20), 0, "{kind:?}");
        assert_eq!(rng.seed(), expected_rng.seed(), "{kind:?}");
    }
}

#[test]
fn mac68k_extent2_bridge_topology_0f_10_skips_secondary_terrain_refresh() {
    // The native per-stride worker calls export 138 on the other three cells
    // only for topology classes 0x0d/0x0e. Classes 0x0f/0x10 proceed directly
    // to the four PlaceTile stamps, leaving XUND on those secondary cells.
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (topology_bit, expected_code) in [(0, 0x0f), (0x02, 0x10)] {
            let mut city = City::new_blank(
                "/tmp/sc2000-mac-bridge-skip-secondary-refresh.sc2",
                "MacBridgeSkipRefresh",
            )
            .expect("blank");
            city.kind = kind;
            city.set_building(20, 20, 0x6a);
            // Set a retained low XZON nibble and unrelated XBIT bit on each
            // cell so this checks the full PlaceTile cleanup masks, not just
            // that the bridge buildings were removed.
            for (x, y, zone) in [
                (20, 20, 0xa5),
                (20, 21, 0xb6),
                (21, 21, 0xc7),
                (21, 20, 0xd8),
            ] {
                city.set_zone(x, y, zone);
                city.set_tile_bits(x, y, TileBits::new(0x03));
            }
            city.set_tile_bits(20, 21, TileBits::new(0x01 | topology_bit));
            city.set_underground(21, 20, 1);
            assert_eq!(
                mac_export180_bridge_connection_code(&city, 20, 20),
                expected_code
            );

            let mut rng = MacToolboxRng::new(0x1020_3040);
            mac_budget_decay_extent2_bridge_at(&mut city, 20, 20, &mut rng, true);

            assert_eq!(
                city.building_at(20, 20),
                0,
                "{kind:?}, class {expected_code:#04x}"
            );
            assert_eq!(
                city.building_at(21, 20),
                0,
                "{kind:?}, class {expected_code:#04x}"
            );
            assert_eq!(
                city.underground_at(21, 20),
                1,
                "{kind:?}, class {expected_code:#04x}"
            );
            for (x, y, expected_zone_low) in [
                (20, 20, 0x05),
                (20, 21, 0x06),
                (21, 21, 0x07),
                (21, 20, 0x08),
            ] {
                assert_eq!(
                    city.zone_at(x, y).map(|zone| zone.raw_bits()),
                    Some(expected_zone_low),
                    "{kind:?}, class {expected_code:#04x}, XZON ({x},{y})"
                );
                assert_eq!(
                    city.tile_bits_at(x, y).map(|bits| bits.raw_bits()),
                    Some(0x01),
                    "{kind:?}, class {expected_code:#04x}, XBIT ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn mac68k_extent2_bridge_49_4a_skip_initial_edge_terrain_refresh() {
    // The original XBLD selector, not the classifier class, gates the initial
    // four-cell export-138 refresh. Put XUND on the cursor tile: export 138
    // clears it before classifying terrain. The forward per-stride classifier
    // still runs on the separate footprint ahead.
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (root, previous) in [(0x49, (20, 18)), (0x4a, (18, 20))] {
            let mut city = City::new_blank(
                "/tmp/sc2000-mac-bridge-root-refresh-gate.sc2",
                "MacBridgeRootRefreshGate",
            )
            .expect("blank");
            city.kind = kind;
            city.set_building(20, 20, root);
            city.set_zone(20, 20, 0xf0);
            city.set_tile_bits(20, 20, TileBits::new(0x04));
            city.set_building(previous.0, previous.1, root);
            city.set_zone(previous.0, previous.1, 0xf0);
            city.set_tile_bits(previous.0, previous.1, TileBits::new(0x04));
            city.set_underground(previous.0, previous.1, 1);

            let mut rng = MacToolboxRng::new(0x1020_3040);
            mac_budget_decay_extent2_bridge_at(&mut city, 20, 20, &mut rng, false);

            assert_eq!(
                city.underground_at(previous.0, previous.1),
                1,
                "{kind:?}, root {root:#04x}, cursor ({},{})",
                previous.0,
                previous.1
            );
        }
    }
}

#[test]
fn mac68k_extent2_bridge_classifier_returns_0d_for_water_marked_49_neighbor() {
    // In the native XZON-high-nibble F mode, an XBIT water bit on XBLD 0x49
    // returns class 0x0d. The full removal walk for this synthetic overlap is
    // intentionally not invoked here: its nested classifier/removal cycle
    // overflows the Rust test thread. The recovered Mac picker option-5 writer
    // uses high-nibble F with uniform 0x49/0x4a footprints; it does not
    // establish this mixed 0x6a-root/0x49-secondary placement. Other writers
    // and imported or edited states for 0x6a/0x6b remain untraced.
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-bridge-topology-0d.sc2",
            "MacBridgeTopology0d",
        )
        .expect("blank");
        city.kind = kind;
        city.set_building(20, 20, 0x6a);
        city.set_zone(20, 20, 0xf0);
        city.set_building(21, 20, 0x49);
        city.set_tile_bits(21, 20, TileBits::new(0x04));
        assert_eq!(mac_export180_bridge_connection_code(&city, 20, 20), 0x0d);
    }
}

#[test]
fn mac68k_extent2_bridge_corner_marked_water_footprint_completes() {
    // Candidate 2x2 state with one XZON corner marker per cell. Keep this
    // distinct from the all-0xf topology fixture above: export 180's all-corners
    // mode must not be inferred from one tile's water bit alone. OpenSC2K
    // independently stamps this corner layout; the recovered Mac option-5
    // writer instead emits uniform 0xf0 XZON markers and uniform 0x49/0x4a
    // XBLD footprints. The Mac 0x6a/0x6b writer remains untraced.
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-bridge-corner-marked-water.sc2",
            "MacBridgeCorners",
        )
        .expect("blank");
        city.kind = kind;
        for (x, y, corner) in [
            (20, 20, 0x10),
            (21, 20, 0x20),
            (21, 21, 0x40),
            (20, 21, 0x80),
        ] {
            city.set_building(x, y, 0x6a);
            city.set_zone(x, y, corner);
            city.set_tile_bits(x, y, TileBits::new(0x04));
        }

        let mut rng = MacToolboxRng::new(0x1020_3040);
        mac_budget_decay_extent2_bridge_at(&mut city, 20, 20, &mut rng, false);

        for (x, y) in [(20, 20), (21, 20), (21, 21), (20, 21)] {
            assert_eq!(city.building_at(x, y), 0, "{kind:?}, ({x},{y})");
        }
    }
}

#[test]
fn mac68k_late_xund_runway_destroy_uses_connected_lifo_and_duplicate_draws() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-xund-runway-component.sc2", "Runway").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 10, 0);

    // A 2x2 component creates a duplicate pending cell in the native LIFO
    // walk: root -> south -> southeast -> east -> the already-pending east.
    for (x, y, building) in [
        (20, 20, 0xdd),
        (21, 20, 0xde),
        (20, 21, 0xdd),
        (21, 21, 0xde),
    ] {
        city.set_building(x, y, building);
        city.set_zone(x, y, 0xda);
        city.set_tile_bits(x, y, TileBits::new(0xf3));
    }
    city.set_underground(20, 20, 0x23);
    city.set_text_overlay(20, 20, 0x55);
    city.set_label(0xdd, "runway label remains");
    city.set_building(19, 20, 0xdf); // neighboring family is not traversed

    let seed = 0x2468_ace0;
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_u16(); // late XUND funding admission
    let draws = std::array::from_fn::<_, 5, _>(|_| (expected_rng.random_u16() as u8 & 3) + 1);
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.building_at(20, 20), draws[0]);
    assert_eq!(city.building_at(20, 21), draws[1]);
    assert_eq!(city.building_at(21, 21), draws[2]);
    assert_eq!(city.building_at(21, 20), draws[4]);
    assert_eq!(city.building_at(19, 20), 0xdf);
    assert_eq!(city.zone_at(20, 20).unwrap().raw_bits(), 0x0a);
    assert_eq!(city.tile_bits_at(20, 20).unwrap().raw_bits(), 0x31);
    for (x, y) in [(21, 20), (20, 21), (21, 21)] {
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0x0a);
        assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits(), 0x31);
    }
    assert_eq!(city.underground_at(20, 20), 0x23);
    assert_eq!(city.text_overlay_at(20, 20), Some(0x55));
    assert_eq!(city.label_at(0xdd), Some("runway label remains"));
    assert_eq!(city.mac_xbld_tile_count(0xdd), 0);
    assert_eq!(city.mac_xbld_tile_count(0xde), 0);
    let mut expected_rubble_counts = [0u16; 4];
    for tile in [draws[0], draws[1], draws[2], draws[4]] {
        expected_rubble_counts[usize::from(tile - 1)] += 1;
    }
    for tile in 1..=4 {
        assert_eq!(
            city.mac_xbld_tile_count(tile),
            expected_rubble_counts[usize::from(tile - 1)],
            "rubble XBLD {tile}"
        );
    }
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac68k_late_xund_generic_extent2_removes_native_rectangle() {
    // Export 81's table gives 0x8c extent 2 without coordinate realignment.
    // Generic export 80 removes x..x+1 and y..y-1 in column-major order.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-late-xund-generic2.sc2", "Generic2").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 100);
    set_budget_funding_for_city_test(&mut city, 10, 0);
    for (x, y, building, terrain) in [
        (20, 20, 0x8c, 0),
        (20, 19, 0x8d, 1),
        (21, 20, 0x8e, 0),
        (21, 19, 0x8f, 1),
    ] {
        city.set_building(x, y, building);
        city.set_terrain(x, y, terrain);
        city.set_zone(x, y, 0xfa);
        city.set_tile_bits(x, y, TileBits::new(0xc2));
    }
    city.set_underground(20, 20, 0x23);
    city.set_text_overlay(21, 20, 77);
    city.set_text_overlay(21, 19, 0xc9);
    assert!(city.set_microsim_tile_id(26, 0x56));
    assert!(city.set_microsim_stat0(26, 42));
    city.set_label(77, "temporary structure label");
    city.mac_object_slot_records_mut()[0].saved_a = 0x55;

    let seed = 0x6a09_e667;
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_u16(); // late XUND funding admission
    let mut rubble_counts = [0u8; 4];
    let mut rubble_by_cell = [(20, 20, 0u8), (21, 20, 0u8)];
    for (_, _, rubble) in &mut rubble_by_cell {
        *rubble = (expected_rng.random_u16() as u8 & 3) + 1;
        rubble_counts[usize::from(*rubble - 1)] += 1;
    }
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    for (x, y, building) in [
        (20, 20, 0x8c),
        (20, 19, 0x8d),
        (21, 20, 0x8e),
        (21, 19, 0x8f),
    ] {
        let expected_building = rubble_by_cell
            .iter()
            .find_map(|&(rx, ry, rubble)| (rx == x && ry == y).then_some(rubble))
            .unwrap_or(0);
        assert_eq!(
            city.building_at(x, y),
            expected_building,
            "cell ({x},{y}), XBLD {building:#04x}"
        );
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits() & 0x0f, 0x0a);
        assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits() & 0xc2, 0);
        assert_eq!(city.mac_xbld_tile_count(building), 0);
    }
    assert_eq!(city.text_overlay_at(21, 20), Some(0));
    assert_eq!(city.microsim_at(26).unwrap().tile_id, 0);
    assert_eq!(city.microsim_at(26).unwrap().stat0, 42);
    assert_eq!(city.label_at(77), Some(""));
    assert_eq!(city.text_overlay_at(21, 19), Some(0xc9));
    assert_eq!(city.traffic_overlays().unwrap()[0].saved_a, 0);
    assert_eq!(city.mac_xbld_tile_count(1), u16::from(rubble_counts[0]));
    assert_eq!(city.mac_xbld_tile_count(2), u16::from(rubble_counts[1]));
    assert_eq!(city.mac_xbld_tile_count(3), u16::from(rubble_counts[2]));
    assert_eq!(city.mac_xbld_tile_count(4), u16::from(rubble_counts[3]));
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac68k_late_xund_generic_extent3_and4_remove_aligned_rectangles() {
    for (extent, first_building, selected_marker, name) in [
        (3usize, 0xaeu8, 0x80u8, "Generic3"),
        (4, 0xc9, 0x80, "Generic4"),
    ] {
        let mut city =
            City::new_blank(format!("/tmp/sc2000-mac-late-xund-{name}.sc2"), name).expect("blank");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 7, 100);
        set_budget_funding_for_city_test(&mut city, 10, 0);

        let anchor_x = 32;
        let anchor_y = 32;
        for column in 0..extent {
            for row in 0..extent {
                let building = if column == 0 && row == 0 {
                    first_building
                } else {
                    first_building.saturating_add(((column * extent + row) % 4) as u8)
                };
                city.set_building(anchor_x + column, anchor_y - row, building);
                city.set_terrain(anchor_x + column, anchor_y - row, 1);
                city.set_zone(anchor_x + column, anchor_y - row, 0xfa);
                city.set_tile_bits(anchor_x + column, anchor_y - row, TileBits::new(0xc2));
            }
        }
        let rubble_x = anchor_x + 1;
        let rubble_y = anchor_y - 1;
        city.set_terrain(rubble_x, rubble_y, 0);
        if extent == 3 {
            city.set_text_overlay(rubble_x, rubble_y, 91);
            assert!(city.set_microsim_tile_id(40, 0x62)); // XTXT 91 -> XMIC index 40
            assert!(city.set_microsim_stat0(40, 17));
            city.set_label(91, "temporary structure label");
        }
        // With rotation zero, 0x80 is export 81's fourth marker. No neighbor
        // marker is present, so the candidate coordinate remains the origin.
        city.set_zone(anchor_x, anchor_y, (selected_marker & 0xf0) | 0x0a);
        city.set_underground(anchor_x, anchor_y, 0x23);
        let seed = 0xbb67_ae85;
        let mut expected_rng = MacToolboxRng::new(seed);
        expected_rng.random_u16(); // late XUND funding admission
        let rubble = (expected_rng.random_u16() as u8 & 3) + 1;
        let mut rng = MacToolboxRng::new(seed);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        for column in 0..extent {
            for row in 0..extent {
                let (x, y) = (anchor_x + column, anchor_y - row);
                let expected_building = if (x, y) == (rubble_x, rubble_y) {
                    rubble
                } else {
                    0
                };
                assert_eq!(
                    city.building_at(x, y),
                    expected_building,
                    "{name} cell ({x},{y})"
                );
                assert_eq!(city.zone_at(x, y).unwrap().raw_bits() & 0xf0, 0);
                assert_eq!(city.zone_at(x, y).unwrap().raw_bits() & 0x0f, 0x0a);
                assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits() & 0xc2, 0);
            }
        }
        if extent == 3 {
            assert_eq!(city.text_overlay_at(rubble_x, rubble_y), Some(0));
            assert_eq!(city.microsim_at(40).unwrap().tile_id, 0);
            assert_eq!(city.microsim_at(40).unwrap().stat0, 17);
            assert_eq!(city.label_at(91), Some(""));
        }
        assert_eq!(rng.seed(), expected_rng.seed(), "{name} random cadence");
    }
}

#[test]
fn mac68k_late_xund_generic_xtxt_fa_decrements_root_class_counter() {
    for (root_building, network_count, non_network_count) in
        [(0x5d, 0u16, 3u16), (0xe1, 2u16, 1u16)]
    {
        let mut city = City::new_blank("/tmp/sc2000-mac-late-xund-fa-counter.sc2", "FaCounter")
            .expect("blank");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 7, 100);
        set_budget_funding_for_city_test(&mut city, 10, 0);
        city.set_building(20, 20, root_building);
        city.set_underground(20, 20, 0x23);
        city.set_terrain(20, 20, 1);
        city.set_zone(20, 20, 0xfa);
        city.set_tile_bits(20, 20, TileBits::new(0xc2));
        city.set_text_overlay(20, 20, 0xfa);
        city.set_mac_state_misc_u32_at(0x1050, u32::from(network_count));
        city.set_mac_state_misc_u32_at(0x1024, u32::from(non_network_count));

        let seed = 0x3c6e_f372;
        let mut expected_rng = MacToolboxRng::new(seed);
        expected_rng.random_u16(); // late XUND funding admission; XTER is nonzero
        let mut rng = MacToolboxRng::new(seed);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        assert_eq!(city.building_at(20, 20), 0);
        assert_eq!(city.text_overlay_at(20, 20), Some(0));
        assert_eq!(
            city.mac_state_misc_u32_at(0x1050).unwrap() as u16,
            network_count.wrapping_sub(u16::from(root_building == 0x5d))
        );
        assert_eq!(
            city.mac_state_misc_u32_at(0x1024).unwrap() as u16,
            non_network_count.wrapping_sub(u16::from(root_building != 0x5d))
        );
        assert_eq!(rng.seed(), expected_rng.seed());
    }
}

#[test]
fn mac68k_late_xund_stadium_destroy_clears_saved_choice_mask_bit() {
    // Mac CODE 5 export 80's 0xd7 arm reads the anchor XTXT's XMIC +4 choice
    // and subtracts that bit from fffffe02 before the 4x4 map cleanup.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-late-xund-stadium-mask.sc2", "Stadium").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 100);
    set_budget_funding_for_city_test(&mut city, 10, 0);

    let anchor_x = 32;
    let anchor_y = 32;
    for column in 0..4 {
        for row in 0..4 {
            city.set_building(anchor_x + column, anchor_y - row, 0xd7);
            city.set_terrain(anchor_x + column, anchor_y - row, 1);
            city.set_zone(anchor_x + column, anchor_y - row, 0xfa);
            city.set_tile_bits(anchor_x + column, anchor_y - row, TileBits::new(0xc2));
        }
    }
    city.set_zone(anchor_x, anchor_y, 0x8a);
    city.set_underground(anchor_x, anchor_y, 0x23);
    let xtxt = city
        .provision_microsim_for_building(0xd7)
        .expect("stadium XMIC row");
    let index = usize::from(xtxt - 0x33);
    city.set_text_overlay(anchor_x, anchor_y, xtxt);
    assert!(city.set_microsim_stat2(index, 2));
    city.set_mac_state_misc_u32_at(0x1028, 0x0000_001f);

    let seed = 0x510e_527f;
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_u16(); // late XUND funding admission
    // Every footprint cell has nonzero XTER, so PlaceTile(0) consumes no
    // rubble-selection RNG, including the anchor with its XMIC record.
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.mac_state_misc_u32_at(0x1028), Some(0x1b));
    for column in 0..4 {
        for row in 0..4 {
            assert_eq!(city.building_at(anchor_x + column, anchor_y - row), 0);
        }
    }
    assert_eq!(rng.seed(), expected_rng.seed());

    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-stadium-choice-mask-{}.mac",
        std::process::id()
    ));
    city.save(&path).expect("save Mac stadium state");
    let loaded = City::load(&path).expect("reload Mac stadium state");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.mac_state_misc_u32_at(0x1028), Some(0x1b));
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac68k_late_xund_stadium_word_shift_over_15_clears_no_mask_bit() {
    // A 68k LSL.W with a register count of 16..=63 yields zero. Rust's
    // wrapping_shl would wrap the count modulo 16 and mutate this mask.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-late-xund-stadium-shift.sc2", "Stadium").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 100);
    set_budget_funding_for_city_test(&mut city, 10, 0);

    let anchor_x = 32;
    let anchor_y = 32;
    for column in 0..4 {
        for row in 0..4 {
            city.set_building(anchor_x + column, anchor_y - row, 0xd7);
            city.set_terrain(anchor_x + column, anchor_y - row, 1);
            city.set_zone(anchor_x + column, anchor_y - row, 0xfa);
            city.set_tile_bits(anchor_x + column, anchor_y - row, TileBits::new(0xc2));
        }
    }
    city.set_zone(anchor_x, anchor_y, 0x8a);
    city.set_underground(anchor_x, anchor_y, 0x23);
    let xtxt = city
        .provision_microsim_for_building(0xd7)
        .expect("stadium XMIC row");
    let index = usize::from(xtxt - 0x33);
    city.set_text_overlay(anchor_x, anchor_y, xtxt);
    assert!(city.set_microsim_stat2(index, 16));
    city.set_mac_state_misc_u32_at(0x1028, 0x0000_001f);

    let seed = 0x510e_527f;
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_u16();
    // All 16 cells use nonzero XTER and therefore skip rubble selection.
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.mac_state_misc_u32_at(0x1028), Some(0x1f));
    for column in 0..4 {
        for row in 0..4 {
            assert_eq!(city.building_at(anchor_x + column, anchor_y - row), 0);
        }
    }
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac68k_late_xund_special_civic_latches_and_clears_generic_footprints() {
    for (building, extent, latch_bit) in [
        (0xf3u8, 2usize, 0u32),
        (0xd0, 3, 1),
        (0xdb, 1, 2),
        (0xff, 4, 3),
    ] {
        let mut city = City::new_blank(
            format!("/tmp/sc2000-mac-late-xund-civic-{building:02x}.sc2"),
            "Civic",
        )
        .expect("blank");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 7, 100);
        set_budget_funding_for_city_test(&mut city, 10, 0);
        let anchor_x = 32;
        let anchor_y = 32;
        for column in 0..extent {
            for row in 0..extent {
                let (x, y) = (anchor_x + column, anchor_y - row);
                city.set_building(x, y, building);
                city.set_terrain(x, y, 1); // nonzero XTER selects PlaceTile(0)
                city.set_zone(x, y, 0xfa);
                city.set_tile_bits(x, y, TileBits::new(0xc2));
            }
        }
        city.set_underground(anchor_x, anchor_y, 0x23);

        let seed = 0x3c6e_f372;
        let mut expected_rng = MacToolboxRng::new(seed);
        expected_rng.random_u16(); // late XUND funding admission
        // All footprint cells use nonzero XTER, selecting PlaceTile(0).
        let mut rng = MacToolboxRng::new(seed);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        for column in 0..extent {
            for row in 0..extent {
                let (x, y) = (anchor_x + column, anchor_y - row);
                assert_eq!(city.building_at(x, y), 0, "XBLD {building:#04x}");
            }
        }
        assert_eq!(city.mac_state_misc_u32_at(0x78), Some(1 << latch_bit));
        assert_eq!(rng.seed(), expected_rng.seed());
    }
}

#[test]
fn mac_scan_bridge_upkeep_changes_later_zone_nine_placement() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-bridge-zone-nine-interleave.sc2",
        "MacScanBridgeZoneNine",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 0);
    city.set_building(42, 42, 0x51);
    city.set_zone(42, 42, 0);
    city.set_tile_bits(42, 42, TileBits::new(0x40));
    city.set_building(42, 43, 0x51);
    city.set_zone(42, 43, 9);
    assert_eq!(city.building_at(42, 43), 0x51);

    // Phase 10's unzoned bridge strip removes the phase-11 zone-9 bridge tile.
    // Select a phase-11 accepted e0 selector so its failed water-ray attempt
    // falls back to e3, which can now occupy the newly emptied cell.
    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe_city = city.clone();
            let mut probe_rng = MacToolboxRng::new(seed);
            for phase in 0..=10 {
                mac_budget_accumulate_monthly_rci_scan_partition(
                    &mut probe_city,
                    phase / 4,
                    phase % 4,
                    &mut probe_rng,
                );
            }
            probe_city.building_at(42, 43) == 0 && probe_rng.random_i16() as u16 & 3 == 0
        })
        .expect("find phase-11 accepted selector after bridge removal");
    let mut expected_city = city.clone();
    let mut expected_rng = MacToolboxRng::new(seed);
    for phase in 0..=10 {
        mac_budget_accumulate_monthly_rci_scan_partition(
            &mut expected_city,
            phase / 4,
            phase % 4,
            &mut expected_rng,
        );
    }
    let mut bridge_intact_city = expected_city.clone();
    bridge_intact_city.set_building(42, 43, 0x51);
    let mut bridge_intact_rng = expected_rng;
    mac_budget_accumulate_monthly_rci_scan_partition(
        &mut bridge_intact_city,
        2,
        3,
        &mut bridge_intact_rng,
    );
    assert_eq!(
        bridge_intact_city.building_at(42, 43),
        0x51,
        "without the earlier bridge edit, e3 fallback cannot replace bridge XBLD"
    );
    expected_rng.random_i16(); // phase-11 e0 selector; failed ray falls back without another draw
    assert_eq!(bridge_intact_rng.seed(), expected_rng.seed());

    let mut rng = MacToolboxRng::new(seed);
    for phase in 0..=11 {
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, phase / 4, phase % 4, &mut rng);
    }

    assert_eq!(city.building_at(42, 42), 0);
    assert_eq!(city.building_at(42, 43), 0xe3);
    assert_eq!(city.zone_at(42, 43).unwrap().raw_bits() & 0x0f, 9);
    assert_eq!(city.mac_xbld_tile_count(0x51), 0);
    assert_eq!(city.mac_xbld_tile_count(0xe3), 1);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_scan_extent2_bridge_upkeep_changes_later_zone_nine_rng_path() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-extent2-bridge-zone-nine-interleave.sc2",
        "MacScanExtent2BridgeZoneNine",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 0);
    city.set_mac_state_misc_u32_at(0x64, 0);

    // Phase 10's unzoned 0x6a bridge enters export 80's high-zone-marker
    // fallback. Its 2x2 rectangle contains the phase-11 zone-9 0xe0 cell.
    let (x, y) = (42usize, 42usize);
    city.set_building(x, y, 0x6a);
    city.set_zone(x, y, 0xf0);
    city.set_building(x, y + 1, 0xe0);
    city.set_zone(x, y + 1, 9);
    for (tx, ty) in [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)] {
        city.set_tile_bits(tx, ty, TileBits::new(0));
        city.set_terrain(tx, ty, 1); // generic export 80 clears without a rubble draw
    }

    // Find a real full-scan prefix where upkeep reaches the fallback, then
    // the later e0 cell's nonzero selector and actor gate both skip placement.
    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe_city = city.clone();
            let mut probe_rng = MacToolboxRng::new(seed);
            for phase in 0..=10 {
                mac_budget_accumulate_monthly_rci_scan_partition(
                    &mut probe_city,
                    phase / 4,
                    phase % 4,
                    &mut probe_rng,
                );
            }
            let selector = probe_rng.random_i16() as u16;
            let actor_gate = probe_rng.random_i16() as u16;
            probe_city.building_at(x, y + 1) == 0 && selector & 3 != 0 && actor_gate & 3 != 0
        })
        .expect("find upkeep and phase-11 actor-gate draws");

    let mut after_phase_ten = city.clone();
    let mut prefix_rng = MacToolboxRng::new(seed);
    for phase in 0..=10 {
        mac_budget_accumulate_monthly_rci_scan_partition(
            &mut after_phase_ten,
            phase / 4,
            phase % 4,
            &mut prefix_rng,
        );
    }
    assert_eq!(after_phase_ten.building_at(x, y), 0);
    assert_eq!(after_phase_ten.building_at(x, y + 1), 0);

    let mut intact_control = after_phase_ten.clone();
    intact_control.set_building(x, y + 1, 0xe0);
    let mut control_rng = prefix_rng;
    let mut expected_control_rng = prefix_rng;
    expected_control_rng.random_i16(); // zone-9 selector
    expected_control_rng.random_i16(); // old 0xe0 actor gate
    mac_budget_accumulate_monthly_rci_scan_partition(&mut intact_control, 2, 3, &mut control_rng);
    assert_eq!(intact_control.building_at(x, y + 1), 0xe0);
    assert_eq!(control_rng.seed(), expected_control_rng.seed());

    let mut expected_rng = prefix_rng;
    expected_rng.random_i16(); // rubble cell no longer takes the e0 actor gate
    mac_budget_accumulate_monthly_rci_scan_partition(&mut after_phase_ten, 2, 3, &mut prefix_rng);
    assert_eq!(after_phase_ten.building_at(x, y + 1), 0);
    assert_eq!(after_phase_ten.mac_xbld_tile_count(0x6a), 0);
    assert_eq!(after_phase_ten.mac_xbld_tile_count(0xe0), 0);
    assert_eq!(prefix_rng.seed(), expected_rng.seed());
}

#[test]
fn mac_scan_late_xund_extent2_destroy_changes_later_zone_nine_rng_path() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-late-xund-extent2-zone-nine.sc2",
        "MacScanLateXundExtent2ZoneNine",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 10, 0);

    // Phase 10's category-10 subway gate calls zero-render export 80. The
    // generic 0x8c extent-2 rectangle covers the phase-14 zone-9 e0 at (43,42).
    let (x, y) = (42usize, 42usize);
    city.set_building(x, y, 0x8c);
    city.set_zone(x, y, 10);
    city.set_underground(x, y, 0x23);
    city.set_building(x + 1, y, 0xe0);
    city.set_zone(x + 1, y, 9);
    city.set_tile_bits(x + 1, y, TileBits::new(0x02));
    for (tx, ty) in [(x, y), (x + 1, y), (x, y - 1), (x + 1, y - 1)] {
        city.set_terrain(tx, ty, 1); // generic export 80 clears without rubble draws
    }

    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe_city = city.clone();
            let mut probe_rng = MacToolboxRng::new(seed);
            for phase in 0..=13 {
                mac_budget_accumulate_monthly_rci_scan_partition(
                    &mut probe_city,
                    phase / 4,
                    phase % 4,
                    &mut probe_rng,
                );
            }
            let selector = probe_rng.random_i16() as u16;
            let actor_gate = probe_rng.random_i16() as u16;
            probe_city.building_at(x + 1, y) == 0 && selector & 3 != 0 && actor_gate & 3 != 0
        })
        .expect("find late XUND destroy and phase-14 actor-gate draws");

    let mut after_phase_thirteen = city.clone();
    let mut prefix_rng = MacToolboxRng::new(seed);
    for phase in 0..=13 {
        mac_budget_accumulate_monthly_rci_scan_partition(
            &mut after_phase_thirteen,
            phase / 4,
            phase % 4,
            &mut prefix_rng,
        );
    }
    assert_eq!(after_phase_thirteen.building_at(x, y), 0);
    assert_eq!(after_phase_thirteen.building_at(x + 1, y), 0);
    assert_eq!(
        after_phase_thirteen
            .tile_bits_at(x + 1, y)
            .unwrap()
            .raw_bits()
            & 0x02,
        0
    );

    let mut intact_control = after_phase_thirteen.clone();
    intact_control.set_building(x + 1, y, 0xe0);
    intact_control.set_tile_bits(x + 1, y, TileBits::new(0x02));
    let mut control_rng = prefix_rng;
    let mut expected_control_rng = prefix_rng;
    expected_control_rng.random_i16(); // zone-9 selector
    expected_control_rng.random_i16(); // old 0xe0 actor gate
    mac_budget_accumulate_monthly_rci_scan_partition(&mut intact_control, 3, 2, &mut control_rng);
    assert_eq!(intact_control.building_at(x + 1, y), 0xe0);
    assert_eq!(control_rng.seed(), expected_control_rng.seed());

    let mut expected_rng = prefix_rng;
    expected_rng.random_i16(); // cleared candidate no longer takes the e0 actor gate
    mac_budget_accumulate_monthly_rci_scan_partition(
        &mut after_phase_thirteen,
        3,
        2,
        &mut prefix_rng,
    );
    assert_eq!(after_phase_thirteen.building_at(x + 1, y), 0);
    assert_eq!(after_phase_thirteen.mac_xbld_tile_count(0x8c), 0);
    assert_eq!(after_phase_thirteen.mac_xbld_tile_count(0xe0), 0);
    assert_eq!(prefix_rng.seed(), expected_rng.seed());
}

#[test]
fn mac_scan_late_xund_large_generic_destroy_changes_later_zone_nine_rng_path() {
    for (extent, source_building) in [(3usize, 0xaeu8), (4, 0xc9)] {
        let mut city = City::new_blank(
            format!("/tmp/sc2000-mac-scan-late-xund-large-{extent}.sc2"),
            "MacScanLateXundLarge",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 10, 0);

        let (x, y) = (42usize, 42usize);
        city.set_building(x, y, source_building);
        city.set_zone(x, y, 10); // no alignment marker: export 81 keeps (42,42)
        city.set_underground(x, y, 0x23);
        let (target_x, target_y) = (x + 1, y);
        city.set_building(target_x, target_y, 0xe0);
        city.set_zone(target_x, target_y, 9);
        city.set_tile_bits(target_x, target_y, TileBits::new(0x02));
        for column in 0..extent {
            for row in 0..extent {
                city.set_terrain(x + column, y - row, 1);
            }
        }

        let seed = (1..100_000)
            .find(|&seed| {
                let mut probe_city = city.clone();
                let mut probe_rng = MacToolboxRng::new(seed);
                for phase in 0..=13 {
                    mac_budget_accumulate_monthly_rci_scan_partition(
                        &mut probe_city,
                        phase / 4,
                        phase % 4,
                        &mut probe_rng,
                    );
                }
                let selector = probe_rng.random_i16() as u16;
                let actor_gate = probe_rng.random_i16() as u16;
                probe_city.building_at(target_x, target_y) == 0
                    && selector & 3 != 0
                    && actor_gate & 3 != 0
            })
            .expect("find aligned large-footprint cleanup and phase-14 draws");

        let mut after_phase_thirteen = city.clone();
        let mut prefix_rng = MacToolboxRng::new(seed);
        for phase in 0..=13 {
            mac_budget_accumulate_monthly_rci_scan_partition(
                &mut after_phase_thirteen,
                phase / 4,
                phase % 4,
                &mut prefix_rng,
            );
        }
        assert_eq!(after_phase_thirteen.building_at(x, y), 0, "extent={extent}");
        assert_eq!(
            after_phase_thirteen.building_at(target_x, target_y),
            0,
            "extent={extent}"
        );
        assert_eq!(
            after_phase_thirteen
                .tile_bits_at(target_x, target_y)
                .unwrap()
                .raw_bits()
                & 0x02,
            0,
            "extent={extent} marker"
        );

        let mut intact_control = after_phase_thirteen.clone();
        intact_control.set_building(target_x, target_y, 0xe0);
        intact_control.set_tile_bits(target_x, target_y, TileBits::new(0x02));
        let mut control_rng = prefix_rng;
        let mut expected_control_rng = prefix_rng;
        expected_control_rng.random_i16(); // zone-9 selector
        expected_control_rng.random_i16(); // old 0xe0 actor gate
        mac_budget_accumulate_monthly_rci_scan_partition(
            &mut intact_control,
            3,
            2,
            &mut control_rng,
        );
        assert_eq!(intact_control.building_at(target_x, target_y), 0xe0);
        assert_eq!(control_rng.seed(), expected_control_rng.seed());

        let mut expected_rng = prefix_rng;
        expected_rng.random_i16(); // cleared candidate no longer takes the e0 actor gate
        mac_budget_accumulate_monthly_rci_scan_partition(
            &mut after_phase_thirteen,
            3,
            2,
            &mut prefix_rng,
        );
        assert_eq!(after_phase_thirteen.building_at(target_x, target_y), 0);
        assert_eq!(
            after_phase_thirteen.mac_xbld_tile_count(source_building),
            0,
            "extent={extent}"
        );
        assert_eq!(after_phase_thirteen.mac_xbld_tile_count(0xe0), 0);
        assert_eq!(prefix_rng.seed(), expected_rng.seed(), "extent={extent}");
    }
}

#[test]
fn mac_scan_late_xund_rotated_marker_alignment_reaches_later_zone_nine() {
    for extent in [3usize, 4] {
        let source_building = if extent == 3 { 0xae } else { 0xc9 };
        let delta = extent - 1;
        for marker_index in 0..4 {
            for rotation in 0..4u8 {
                let markers = match rotation {
                    0 => [0x10, 0x20, 0x40, 0x80],
                    1 => [0x20, 0x40, 0x80, 0x10],
                    2 => [0x40, 0x80, 0x10, 0x20],
                    _ => [0x80, 0x10, 0x20, 0x40],
                };
                let marker = markers[marker_index];
                // Each target is in the aligned rectangle but outside the
                // unshifted one. Marker 3 is the no-shift control: its target
                // stays outside both rectangles and retains the actor draw.
                let (source, target, target_phase, anchor, aligned) = match marker_index {
                    0 => (
                        (42usize, 42usize),
                        (43usize, 43usize),
                        15usize,
                        (42usize, 42usize + delta),
                        true,
                    ),
                    1 => ((44, 44), (43, 45), 13, (44 - delta, 44 + delta), true),
                    2 => ((44, 44), (43, 44), 12, (44 - delta, 44), true),
                    _ => ((42, 42), (43, 43), 15, (42, 42), false),
                };
                let mut city = City::new_blank(
                    format!(
                        "/tmp/sc2000-mac-scan-aligned-large-{extent}-{marker_index}-{rotation}.sc2"
                    ),
                    "MacScanAlignedLarge",
                )
                .expect("blank city");
                city.kind = CityFileKind::MacCity;
                city.saved_rotation = rotation;
                city.set_view_rotation((rotation + 1) & 3);
                for index in 0..BUDGET_SERVICE_COUNT {
                    set_budget_funding_for_city_test(&mut city, index, 100);
                }
                set_budget_funding_for_city_test(&mut city, 10, 0);

                city.set_building(source.0, source.1, source_building);
                city.set_zone(source.0, source.1, marker | 10);
                city.set_underground(source.0, source.1, 0x23);
                city.set_building(target.0, target.1, 0xe0);
                city.set_zone(target.0, target.1, 9);
                city.set_tile_bits(target.0, target.1, TileBits::new(0x02));
                for column in 0..extent {
                    for row in 0..extent {
                        city.set_terrain(anchor.0 + column, anchor.1 - row, 1);
                    }
                }

                let expected_target = if aligned { 0 } else { 0xe0 };
                let seed = (1..100_000)
                    .find(|&seed| {
                        let mut probe_city = city.clone();
                        let mut probe_rng = MacToolboxRng::new(seed);
                        for phase in 0..target_phase {
                            mac_budget_accumulate_monthly_rci_scan_partition(
                                &mut probe_city,
                                phase / 4,
                                phase % 4,
                                &mut probe_rng,
                            );
                        }
                        let selector = probe_rng.random_i16() as u16;
                        let actor_gate = probe_rng.random_i16() as u16;
                        probe_city.building_at(target.0, target.1) == expected_target
                            && selector & 3 != 0
                            && actor_gate & 3 != 0
                    })
                    .expect("find marker-aligned cleanup and later actor-gate draws");

                let mut after_prefix = city.clone();
                let mut prefix_rng = MacToolboxRng::new(seed);
                for phase in 0..target_phase {
                    mac_budget_accumulate_monthly_rci_scan_partition(
                        &mut after_prefix,
                        phase / 4,
                        phase % 4,
                        &mut prefix_rng,
                    );
                }
                assert_eq!(after_prefix.building_at(source.0, source.1), 0);
                assert_eq!(
                    after_prefix.building_at(target.0, target.1),
                    expected_target,
                    "extent={extent}, marker={marker:#04x}, rotation={rotation}"
                );

                let mut intact_control = after_prefix.clone();
                intact_control.set_building(target.0, target.1, 0xe0);
                intact_control.set_tile_bits(target.0, target.1, TileBits::new(0x02));
                let mut control_rng = prefix_rng;
                let mut expected_control_rng = prefix_rng;
                expected_control_rng.random_i16(); // zone-9 selector
                expected_control_rng.random_i16(); // restored 0xe0 actor gate
                mac_budget_accumulate_monthly_rci_scan_partition(
                    &mut intact_control,
                    target_phase / 4,
                    target_phase % 4,
                    &mut control_rng,
                );
                assert_eq!(intact_control.building_at(target.0, target.1), 0xe0);
                assert_eq!(control_rng.seed(), expected_control_rng.seed());

                let mut expected_rng = prefix_rng;
                expected_rng.random_i16(); // zone-9 selector
                if !aligned {
                    expected_rng.random_i16(); // unshifted target retains actor gate
                }
                mac_budget_accumulate_monthly_rci_scan_partition(
                    &mut after_prefix,
                    target_phase / 4,
                    target_phase % 4,
                    &mut prefix_rng,
                );
                assert_eq!(
                    after_prefix.building_at(target.0, target.1),
                    expected_target,
                    "extent={extent}, marker={marker:#04x}, rotation={rotation}"
                );
                assert_eq!(after_prefix.mac_xbld_tile_count(source_building), 0);
                assert_eq!(after_prefix.mac_xbld_tile_count(0xe0), u16::from(!aligned));
                assert_eq!(
                    prefix_rng.seed(),
                    expected_rng.seed(),
                    "extent={extent}, marker={marker:#04x}, rotation={rotation}"
                );
            }
        }
    }
}

#[test]
fn mac_scan_late_xund_runway_destroy_suppresses_later_zone_eight_timing_draws() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-late-xund-runway-zone-eight.sc2",
        "MacScanLateXundRunwayZoneEight",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 10, 0);

    let (x, y) = (42usize, 42usize);
    city.set_building(x, y, 0xdd);
    city.set_zone(x, y, 10);
    city.set_underground(x, y, 0x23);
    city.set_building(x, y + 1, 0xdd);
    city.set_zone(x, y + 1, 8);
    city.set_tile_bits(x, y + 1, TileBits::new(0x40));

    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe_city = city.clone();
            let mut probe_rng = MacToolboxRng::new(seed);
            for phase in 0..=10 {
                mac_budget_accumulate_monthly_rci_scan_partition(
                    &mut probe_city,
                    phase / 4,
                    phase % 4,
                    &mut probe_rng,
                );
            }
            let selector = probe_rng.random_i16() as u16;
            let existing_runway_timing_gate = probe_rng.random_i16() as u16;
            (1..=4).contains(&probe_city.building_at(x, y + 1))
                && selector & 3 != 0
                && existing_runway_timing_gate % 30 != 0
        })
        .expect("find late runway destroy and phase-11 timing-gate draws");

    let mut after_phase_ten = city.clone();
    let mut prefix_rng = MacToolboxRng::new(seed);
    for phase in 0..=10 {
        mac_budget_accumulate_monthly_rci_scan_partition(
            &mut after_phase_ten,
            phase / 4,
            phase % 4,
            &mut prefix_rng,
        );
    }
    assert!((1..=4).contains(&after_phase_ten.building_at(x, y)));
    assert!((1..=4).contains(&after_phase_ten.building_at(x, y + 1)));
    assert_eq!(after_phase_ten.mac_xbld_tile_count(0xdd), 0);
    assert_eq!(
        after_phase_ten.tile_bits_at(x, y + 1).unwrap().raw_bits() & 0x40,
        0
    );

    let mut intact_control = after_phase_ten.clone();
    intact_control.set_building(x, y + 1, 0xdd);
    intact_control.set_tile_bits(x, y + 1, TileBits::new(0x40));
    let mut control_rng = prefix_rng;
    let mut expected_control_rng = prefix_rng;
    expected_control_rng.random_i16(); // zone-8 selector
    expected_control_rng.random_i16(); // existing 0xdd timing gate
    mac_budget_accumulate_monthly_rci_scan_partition(&mut intact_control, 2, 3, &mut control_rng);
    assert_eq!(intact_control.building_at(x, y + 1), 0xdd);
    assert_eq!(control_rng.seed(), expected_control_rng.seed());

    let mut expected_rng = prefix_rng;
    expected_rng.random_i16(); // rubble is not an existing 0xdd runway
    mac_budget_accumulate_monthly_rci_scan_partition(&mut after_phase_ten, 2, 3, &mut prefix_rng);
    assert!((1..=4).contains(&after_phase_ten.building_at(x, y + 1)));
    assert_eq!(prefix_rng.seed(), expected_rng.seed());
}

#[test]
fn mac_scan_late_xund_generic_destroy_suppresses_later_arcology_xmic_age_write() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-late-xund-xtxt-arcology-interleave.sc2",
        "MacScanLateXundXtxtArcology",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 10, 0);

    // The phase-10 generic cleanup releases a shared XMIC row before phase 11
    // reaches the arcology that uses the same XTXT handle.
    let (x, y) = (42usize, 42usize);
    let xtxt = 0x56;
    let microsim_index = usize::from(xtxt - MIN_SIM_TEXT_ENTRIES);
    city.set_building(x, y, 0xe1);
    city.set_zone(x, y, 10);
    city.set_underground(x, y, 0x23);
    city.set_terrain(x, y, 1);
    city.set_text_overlay(x, y, xtxt);
    city.set_building(x, y + 1, 0xfb);
    city.set_zone(x, y + 1, 0x8a);
    city.set_tile_bits(x, y + 1, TileBits::new(0x50)); // powered and watered
    city.set_text_overlay(x, y + 1, xtxt);
    assert!(city.set_microsim_tile_id(microsim_index, 0xfb));
    assert!(city.set_microsim_stat0(microsim_index, 99));
    city.set_label(xtxt, "shared structure label");

    let seed = 0x6c62_272e;
    let mut after_phase_ten = city.clone();
    let mut prefix_rng = MacToolboxRng::new(seed);
    for phase in 0..=10 {
        mac_budget_accumulate_monthly_rci_scan_partition(
            &mut after_phase_ten,
            phase / 4,
            phase % 4,
            &mut prefix_rng,
        );
    }
    assert_eq!(after_phase_ten.building_at(x, y), 0);
    assert_eq!(after_phase_ten.building_at(x, y + 1), 0xfb);
    assert_eq!(after_phase_ten.text_overlay_at(x, y), Some(0));
    assert_eq!(after_phase_ten.text_overlay_at(x, y + 1), Some(xtxt));
    assert_eq!(
        after_phase_ten.microsim_at(microsim_index).unwrap().tile_id,
        0
    );
    assert_eq!(
        after_phase_ten.microsim_at(microsim_index).unwrap().stat0,
        99
    );
    assert_eq!(after_phase_ten.label_at(xtxt), Some(""));

    // Restore only the shared resource class after the same prefix to isolate
    // the later arcology's XMIC age-write predicate.
    let mut intact_control = after_phase_ten.clone();
    assert!(intact_control.set_microsim_tile_id(microsim_index, 0xfb));
    let mut control_rng = prefix_rng;
    mac_budget_accumulate_monthly_rci_scan_partition(&mut intact_control, 2, 3, &mut control_rng);
    assert_eq!(intact_control.building_at(x, y + 1), 0xfb);
    assert_eq!(
        intact_control.microsim_at(microsim_index).unwrap().stat0,
        12
    );
    assert_eq!(control_rng.seed(), prefix_rng.seed());

    mac_budget_accumulate_monthly_rci_scan_partition(&mut after_phase_ten, 2, 3, &mut prefix_rng);
    assert_eq!(after_phase_ten.building_at(x, y + 1), 0xfb);
    assert_eq!(
        after_phase_ten.microsim_at(microsim_index).unwrap().stat0,
        99
    );
    assert_eq!(prefix_rng.seed(), control_rng.seed());
}

#[test]
fn rubble_highway_block_preserves_xtxt_place_tile_only() {
    // PE highway @ `0x435b6a`+: per-cell PlaceTile only — XTXT kept (not invent-cleared,
    // not pe_flood_destroy footprint).
    let mut city = City::new_blank("/tmp/sc2000-hwy-xtxt-cell.sc2", "HwyXtxt").expect("blank");
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 74);
            city.set_text_overlay(x, y, 0x21);
        }
    }
    city.set_traffic_density(20, 20, 0x44);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(state.rubble_highway_block(20, 20, &mut crt), 4);
    for y in 20..=21 {
        for x in 20..=21 {
            assert!(is_rubble_tile(state.city.building_at(x, y)), "x={x} y={y}");
            assert_eq!(state.city.text_overlay_at(x, y), Some(0x21), "x={x} y={y}");
        }
    }
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x44));
}

#[test]
fn rubble_highway_block_place_tiles_pure_bridge_in_footprint() {
    // PE @ `0x435b30`+: 2×2 PlaceTile has no per-cell XBLD filter — pure bridge
    // inside a highway footprint is rubbled here (bridge DestroyBuilding is only
    // when the budget walker lands on a bridge as the primary tile).
    let mut city = City::new_blank("/tmp/sc2000-hwy-brg-foot.sc2", "HwyBrgFoot").expect("blank");
    city.set_building(20, 20, 74);
    city.set_building(21, 20, 74);
    city.set_building(20, 21, 74);
    city.set_building(21, 21, 87); // pure bridge inside highway 2×2
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(state.rubble_highway_block(20, 20, &mut crt), 4);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert!(is_rubble_tile(state.city.building_at(21, 20)));
    assert!(is_rubble_tile(state.city.building_at(20, 21)));
    assert!(is_rubble_tile(state.city.building_at(21, 21)));
}

#[test]
fn rubble_highway_block_place_tiles_vacant_in_footprint() {
    // PE PlaceTiles every in-bounds 2×2 cell, including vacant.
    let mut city = City::new_blank("/tmp/sc2000-hwy-vacant-foot.sc2", "HwyVacFoot").expect("blank");
    city.set_building(20, 20, 74);
    city.set_building(21, 20, 74);
    city.set_building(20, 21, 74);
    city.set_building(21, 21, 0);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(state.rubble_highway_block(20, 20, &mut crt), 4);
    for y in 20..=21 {
        for x in 20..=21 {
            assert!(is_rubble_tile(state.city.building_at(x, y)), "x={x} y={y}");
        }
    }
}

#[test]
fn rubble_highway_block_still_decays_highway_bridge_overlap_106() {
    // When the highway arm fires, footprint PlaceTile still clears XBLD 106/107
    // inside the 2×2 (PE PlaceTiles every cell). Primary-tile walker dispatch for
    // 106/107 is BridgeCheck — see `underfunded_bridge_overlap_106_uses_destroy_building`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-overlap.sc2", "HwyOverlap").expect("blank");
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 106);
        }
    }
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(state.rubble_highway_block(20, 20, &mut crt), 4);
    for y in 20..=21 {
        for x in 20..=21 {
            assert!(is_rubble_tile(state.city.building_at(x, y)), "x={x} y={y}");
        }
    }
}

#[test]
fn destroy_bridge_tile_elevated_uses_destroy_building_arm_not_structure_early_out() {
    // PE DestroyBuilding elevated @ `0x45994e`: PlaceTile(0), keep XTXT.
    // DestroyStructure @ `0x46137b` would early-out and leave XBLD/XTXT.
    let mut city = City::new_blank("/tmp/sc2000-brg-elev.sc2", "BrgElev").expect("blank");
    city.set_building(20, 20, 0x3f);
    city.set_text_overlay(20, 20, 0x55);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    state.destroy_bridge_tile(20, 20, &mut crt);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x55));
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT)
    );
}

#[test]
fn destroy_bridge_tile_leaves_neighbor_road_mask() {
    // PE @ `0x435cce`→`0x435cdf`→`0x435dda`: no generic PlaceRoads reshape.
    // The support-tip walk clears the north/south endpoints for this orientation,
    // so assert the perpendicular east/west road masks remain untouched.
    let mut city = City::new_blank("/tmp/sc2000-brg-mask.sc2", "BrgMask").expect("blank");
    let lr = network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT);
    city.set_building(20, 20, 87);
    city.set_tile_bits(20, 20, TileBits::new(0));
    city.set_building(19, 20, lr);
    city.set_building(21, 20, lr);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    state.destroy_bridge_tile(20, 20, &mut crt);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.building_at(19, 20), lr);
    assert_eq!(state.city.building_at(21, 20), lr);
}

#[test]
fn simulation_day24_refreshes_toolbar_and_neighbor_connections() {
    let mut city = reward_unlock_test_city();
    city.set_building(
        0,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    // Seed a stale neighbor count only after budget/service MISC defaults exist.
    state.refresh_live_misc_current_values();
    state
        .city
        .set_misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET, 99);
    state.city.clear_dirty();
    assert_eq!(state.city.rewards_available_mask(), 0);

    state.handle_simulation_month_advanced();

    assert_eq!(state.city.rewards_available_mask() & 0b0_1111, 0b0_1111);
    assert_eq!(state.query_line, "Rewards Available");
    assert_eq!(
        state.city.misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET),
        Some(1),
        "day-24 SimNation refresh should persist live neighbor connections"
    );
    assert!(state.city.has_dirty_tiles());
}

#[test]
fn pe_seed_power_plant_xval_preserves_existing_land_plane() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    pe_seed_power_plant_xval_for_test(&mut city, 9, 10, 30);
    pe_seed_power_plant_xval_for_test(&mut city, 49, 50, 18);
    assert_eq!(city.land_value_at(9, 10), Some(30));
    assert_eq!(city.land_value_at(49, 50), Some(18));
}

#[test]
fn status_line_omits_treatment_warning_when_enough_plants_exist() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, 1);
    for plant in 0..9 {
        let origin_x = 20 + plant * 3;
        for y in 20..=21 {
            for x in origin_x..=origin_x + 1 {
                city.set_building(x, y, 244);
            }
        }
    }
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(!state.query_line.contains("Treatment Missing"));
}

#[test]
fn pe_power_xbld_mapping_covers_rust_plant_ids() {
    assert_eq!(pe_power_xbld_for_building(200), Some(0xC8));
    assert_eq!(pe_power_xbld_for_building(204), Some(0xCC));
    assert_eq!(pe_power_xbld_for_building(206), Some(0xCE));
    assert_eq!(pe_power_xbld_for_building(199), Some(0xC7));
}

#[test]
fn pe_power_c8_output_tiles_scales_pe_units_by_sixteen() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut land = vec![0_u8; CITY_TILES];
    land[10 * CITY_HEIGHT + 10] = 20;
    city.sync_land_value_from_slice(&land);
    assert_eq!(
        pe_power_c8_output_tiles(&city, 10, 10, 3),
        pe_power_c8_pe_units(&city, 10, 10, 3).saturating_mul(16)
    );
}

#[test]
fn pe_power_c8_crt_rand_remainder_matches_pe_disassembly() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_WIND_OFFSET, 0x0a);
    let mut land = vec![0_u8; CITY_TILES];
    land[5 * CITY_HEIGHT + 5] = 20;
    city.sync_land_value_from_slice(&land);
    let mut rng = Win95CrtRand::new(0x0046_79B0);
    let remainder = pe_power_crt_rand_u16(&mut rng);
    let modulus = pe_power_c8_random_modulus(&city);
    let xval = u32::from(city.land_value_at(5, 5).unwrap_or(0)) & 0x1f;
    assert_eq!(
        pe_power_c8_pe_units(&city, 5, 5, remainder),
        (xval + remainder % modulus) / 2
    );
}

#[test]
fn pe_power_c8_xval_pop_level_low_masks_to_zero_low_five_bits() {
    // `XVAL_POPLEVEL_LOW` (32) satisfies residential pop1 gates but `XVAL & 0x1f == 0`,
    // so wind plants contribute only the CRT remainder term — often zero capacity.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_WIND_OFFSET, 0); // modulus = 1 → remainder % 1 == 0
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, XVAL_POPLEVEL_LOW);
    assert_eq!(u32::from(XVAL_POPLEVEL_LOW) & 0x1f, 0);
    assert_eq!(pe_power_c8_pe_units(&city, 10, 10, 0), 0);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 30);
    assert_eq!(pe_power_c8_pe_units(&city, 10, 10, 0), 15);
}

#[test]
fn pe_power_cc_crt_rand_remainder_matches_pe_disassembly() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_HUMID_OFFSET, 0x0f);
    let mut rng = Win95CrtRand::new(0x0046_79CC);
    let remainder = pe_power_crt_rand_u16(&mut rng);
    let span = pe_power_cc_random_span(&city);
    assert_eq!(pe_power_cc_pe_units(&city, remainder), 5 + remainder % span);
}

#[test]
fn pe_power_solar_guards_native_zero_divisor_for_custom_humidity() {
    let mut city =
        City::new_blank("/tmp/sc2000-solar-zero-span.sc2", "SolarZeroSpan").expect("blank city");
    for humidity in 91..=109 {
        city.pe_b_weather_humid = humidity;
        assert_eq!(pe_power_cc_random_span(&city), 0);
        assert_eq!(
            pe_power_cc_pe_units(&city, 0xffff),
            5,
            "custom humidity {humidity} is kept playable instead of taking the PE #DE path"
        );
    }
}

#[test]
fn pe_power_tile_source_eligible_reads_xbit_powerable() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_power_tile_source_eligible(&city, 10, 10));
    city.set_tile_bits(10, 10, TileBits::new(0).with_powerable(true));
    assert!(pe_power_tile_source_eligible(&city, 10, 10));
}

#[test]
fn pe_power_tile_source_eligible_falls_back_to_plant_origin() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 204);
        }
    }
    assert!(pe_power_tile_source_eligible(&city, 20, 20));
    assert!(!pe_power_tile_source_eligible(&city, 21, 20));
}

#[test]
fn pe_power_coal_one_by_one_origin_is_source_eligible_without_xbit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 198);
    assert!(pe_power_source_xbld(198));
    assert!(pe_power_tile_source_eligible(&city, 20, 20));
}

#[test]
fn power_tile_connects_to_treats_power_lines_and_plants_as_omnidirectional() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 14);
    for side in [NETWORK_TOP, NETWORK_RIGHT, NETWORK_BOTTOM, NETWORK_LEFT] {
        assert!(power_tile_connects_to(&city, 20, 20, side));
    }
    city.set_building(20, 20, 198);
    assert!(power_tile_connects_to(&city, 20, 20, NETWORK_TOP));
    city.set_building(20, 20, 0x1D);
    assert!(!power_tile_connects_to(&city, 20, 20, NETWORK_TOP));
}

#[test]
fn pe_power_solar_output_uses_cc_humidity_formula() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 204);
        }
    }
    let mut rng = Win95CrtRand::new(pe_power_recalc_seed(&city));
    let solar = power_plant_output_tiles_with_rng(&city, 20, 20, &mut rng);
    let remainder = pe_power_crt_rand_u16(&mut Win95CrtRand::new(pe_power_recalc_seed(&city)));
    assert_eq!(solar, pe_power_cc_output_tiles(&city, remainder));
}

#[test]
fn pe_power_solar_default_humid_average_matches_legacy_constant() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    initialize_original_new_city_national_misc(&mut city);
    let span = pe_power_cc_random_span(&city);
    assert_eq!(span, 8);
    let total_pe_units: u32 = (0..span)
        .map(|remainder| pe_power_cc_pe_units(&city, remainder))
        .sum();
    assert_eq!(total_pe_units, 68);
    assert_eq!((total_pe_units * 16 + span / 2) / span, 136);
}

#[test]
fn road_tiles_do_not_join_power_network() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 0x1D); // TILE_ROAD_LR
    assert!(!structural_powerable_tile(&city, 20, 20));
    assert!(!power_network_tile(&city, 20, 20));
    city.set_building(20, 20, 44); // ordinary rail (not power/rail crossover)
    assert!(!structural_powerable_tile(&city, 20, 20));
    assert!(!power_network_tile(&city, 20, 20));
    city.set_building(20, 20, 73); // highway
    assert!(!structural_powerable_tile(&city, 20, 20));
    assert!(!power_network_tile(&city, 20, 20));
}

#[test]
fn power_line_tiles_join_network_but_are_not_consumers() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 14); // straight power line
    assert!(NetworkKind::Power.contains(14));
    assert!(power_network_tile(&city, 20, 20));
    assert!(!power_consuming_tile(&city, 20, 20));
    assert!(!pe_power_flood_fill_consumer_tile(&city, 20, 20));
}

#[test]
fn place_power_line_stamps_xbit_powerable_like_pe_44a8a0() {
    // PE FUN_0044A8A0 / PlacePowerLineTile: after committing XBLD, OR XBIT 0x80.
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert!(!pe_power_flood_tile_accepted(&state.city, 20, 20));
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Power), 1);
    assert!(NetworkKind::Power.contains(state.city.building_at(20, 20)));
    assert!(pe_power_flood_tile_accepted(&state.city, 20, 20));
    assert!(
        state
            .city
            .tile_bits_at(20, 20)
            .is_some_and(TileBits::powerable)
    );
}

#[test]
fn place_network_path_preserves_xtxt_like_pe_place_tile() {
    // PE PlaceTile `0x441f00`: road stamp leaves XTXT.
    let mut city = City::new_blank("/tmp/sc2000-net-xtxt.sc2", "NetT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(20, 20, 0x32);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Road), 1);
    assert!(is_road_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x32));
}

#[test]
fn mac68k_place_network_path_preserves_xtxt_like_pe_place_tile() {
    // Shared PlaceTile road stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-net-xtxt.sc2", "MacNetT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(20, 20, 0x32);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Road), 1);
    assert!(is_road_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x32));
}

#[test]
fn mac68k_place_network_path_preserves_xtrf_like_pe_place_tile() {
    // Shared PlaceTile road stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-net-xtrf.sc2", "MacNetX").expect("blank");
    city.set_city_funds(20_000);
    city.set_traffic_density(20, 20, 0x2a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Road), 1);
    assert!(is_road_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x2a));
}

#[test]
fn place_power_network_preserves_xtxt_like_pe_place_tile() {
    // PE PlaceTile `0x441f00` + powerable XBIT: leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-pwr-xtxt.sc2", "PwrT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(22, 22, 0x33);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(22, 22)], NetworkKind::Power), 1);
    assert!(state.city.tile_bits_at(22, 22).unwrap().powerable());
    assert_eq!(state.city.text_overlay_at(22, 22), Some(0x33));
}

#[test]
fn mac68k_place_power_network_preserves_xtxt_like_pe_place_tile() {
    // Shared PlaceTile power stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-pwr-xtxt.sc2", "MacPwrT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(22, 22, 0x33);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_path(&[(22, 22)], NetworkKind::Power), 1);
    assert!(state.city.tile_bits_at(22, 22).unwrap().powerable());
    assert_eq!(state.city.text_overlay_at(22, 22), Some(0x33));
}

#[test]
fn mac68k_place_power_network_preserves_xtrf_like_pe_place_tile() {
    // Shared PlaceTile power stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-pwr-xtrf.sc2", "MacPwrX").expect("blank");
    city.set_city_funds(20_000);
    city.set_traffic_density(22, 22, 0x31);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_path(&[(22, 22)], NetworkKind::Power), 1);
    assert!(NetworkKind::Power.contains(state.city.building_at(22, 22)));
    assert_eq!(state.city.traffic_density_at(22, 22), Some(0x31));
}

#[test]
fn place_rail_network_preserves_xtxt_like_pe_place_tile() {
    // PE PlaceTile `0x441f00`: rail stamp leaves XTXT.
    let mut city = City::new_blank("/tmp/sc2000-rail-xtxt.sc2", "RailT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(24, 24, 0x35);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(24, 24)], NetworkKind::Rail), 1);
    assert!(NetworkKind::Rail.contains(state.city.building_at(24, 24)));
    assert_eq!(state.city.text_overlay_at(24, 24), Some(0x35));
}

#[test]
fn mac68k_place_rail_network_preserves_xtxt_like_pe_place_tile() {
    // Shared PlaceTile rail stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-rail-xtxt.sc2", "MacRailT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(24, 24, 0x35);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_path(&[(24, 24)], NetworkKind::Rail), 1);
    assert!(NetworkKind::Rail.contains(state.city.building_at(24, 24)));
    assert_eq!(state.city.text_overlay_at(24, 24), Some(0x35));
}

#[test]
fn mac68k_place_rail_network_preserves_xtrf_like_pe_place_tile() {
    // Shared PlaceTile rail stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-rail-xtrf.sc2", "MacRailX").expect("blank");
    city.set_city_funds(20_000);
    city.set_traffic_density(24, 24, 0x27);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_network_path(&[(24, 24)], NetworkKind::Rail), 1);
    assert!(NetworkKind::Rail.contains(state.city.building_at(24, 24)));
    assert_eq!(state.city.traffic_density_at(24, 24), Some(0x27));
}

#[test]
fn bulldoze_clears_xbit_powerable_from_power_line() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Power), 1);
    assert!(pe_power_flood_tile_accepted(&state.city, 20, 20));
    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert!(!pe_power_flood_tile_accepted(&state.city, 20, 20));
}

#[test]
fn place_power_over_road_crossover_stamps_xbit_powerable() {
    // PE FUN_0044A8A0 road arms (XBLD 0x1D/0x1E -> 0x43/0x44) also OR XBIT 0x80.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 30); // road TB
    let mut state = test_state(city);
    assert_eq!(
        state.place_network_path(&[(20, 19), (20, 20), (20, 21)], NetworkKind::Power),
        3
    );
    assert_eq!(state.city.building_at(20, 20), 68); // power/road crossover
    assert!(pe_power_flood_tile_accepted(&state.city, 20, 20));
    assert!(
        state
            .city
            .tile_bits_at(20, 20)
            .is_some_and(TileBits::powerable)
    );
}

#[test]
fn pe_power_used_percent_ignores_bare_zone_consumers() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 30);
    stamp_power_plant_xbit_powerable(&mut city, &[(10, 10)]);
    city.set_zone(11, 10, 1);
    city.set_zone(12, 10, 1);
    let capacity = power_supply_capacity_tiles(&city);
    assert!(capacity > 0);
    assert_eq!(power_service_summary(&city).served, 2);
    assert_eq!(power_service_maps(&city).pe_powered_consumers(&city), 0);
    assert_eq!(power_used_percent_for_city(&city), 0);
}

#[test]
fn power_used_percent_maps_powered_consumers_from_same_crt_entry() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 18);
    stamp_power_plant_xbit_powerable(&mut city, &[(10, 10)]);
    city.set_building(12, 10, 0x70);
    let entry = pe_power_recalc_seed(&city);
    let maps = power_service_maps_with_entry_seed(&city, entry);
    let mut capacity_rng = Win95CrtRand::new(entry);
    let capacity = power_supply_capacity_tiles_with_rng(&city, &mut capacity_rng);
    assert_eq!(
        power_used_percent_for_city(&city),
        service_utilization_percent(maps.pe_powered_consumers(&city), capacity)
    );
}

#[test]
fn power_consuming_tile_includes_zones_beyond_pe_xbld_consumer_band() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, 1);
    city.set_building(11, 10, 112);
    assert!(power_consuming_tile(&city, 10, 10));
    assert!(power_consuming_tile(&city, 11, 10));
    assert!(pe_power_flood_fill_consumer_tile(&city, 11, 10));
    assert!(!pe_power_flood_fill_consumer_tile(&city, 10, 10));
}

#[test]
fn pe_power_consumer_and_source_xbld_ranges_match_disassembly() {
    assert!(pe_power_consumer_xbld(0x70));
    assert!(pe_power_consumer_xbld(0xC5));
    assert!(!pe_power_consumer_xbld(0xC6));
    assert!(pe_power_source_xbld(0xC6));
    assert!(pe_power_source_xbld(0xCF));
    assert!(!pe_power_source_xbld(0xD0));
    assert_eq!(pe_power_xbld_for_building(198), Some(0xC6));
}

#[test]
fn pe_power_recalc_seed_is_stable_for_blank_city() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let a = pe_power_recalc_seed(&city);
    let b = pe_power_recalc_seed(&city);
    assert_eq!(a, b);
    assert_eq!(power_supply_capacity_tiles(&city), 0);
}

#[test]
fn pe_power_recalc_seed_varies_with_saved_city_month() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(crate::city::CityDate {
        year: 1900,
        month: 1,
    });
    let january = pe_power_recalc_seed(&city);
    city.set_saved_date(crate::city::CityDate {
        year: 1900,
        month: 2,
    });
    let february = pe_power_recalc_seed(&city);
    assert_ne!(january, february);
}

#[test]
fn earned_reward_mask_power_maps_use_process_crt_seed_not_city_name() {
    // Day-24 reward availability shares FUN_004679B0 process CRT with ServiceStatus.
    let mut city = City::new_blank("/tmp/sc2000-reward-crt.sc2", "RewardCrt").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xc3c3_3c3c);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let process_outcomes = live_city_service_outcomes_for_city_with_power(
        &state.city,
        &power_service_maps_with_entry_seed(&state.city, process),
    );
    let expected = earned_reward_mask_for_city_with_outcomes(&state.city, process_outcomes);
    let city_name_outcomes = city_service_outcomes_for_city(&state.city);
    let _ = earned_reward_mask_for_city_with_outcomes(&state.city, city_name_outcomes);
    assert_eq!(
        earned_reward_mask_for_city_with_outcomes(
            &state.city,
            state.live_city_service_outcomes_with_process_crt(),
        ),
        expected
    );
    let _ = state.refresh_reward_availability();
}

#[test]
fn pe_reward_population_gate_uses_shared_scalar_on_mac_layout() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-reward-population-mac-layout-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "RewardCrossLayout").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 12_000);
    stamp_complete_building_for_test(&mut city, 20, 20, 0xae, 3);
    let outcomes = CityServiceOutcomes {
        service_score: 100,
        city_value: 1_000_000,
        ..CityServiceOutcomes::default()
    };

    let win95_mask = earned_reward_mask_for_city_with_outcomes_and_fidelity(
        &city,
        outcomes,
        EngineFidelity::Win95,
    );
    let mac_mask = earned_reward_mask_for_city_with_outcomes_and_fidelity(
        &city,
        outcomes,
        EngineFidelity::Mac68k,
    );
    const ARCOLOGIES_REWARD: u32 = 1 << 4;
    assert_ne!(win95_mask & ARCOLOGIES_REWARD, 0);
    assert_eq!(mac_mask & ARCOLOGIES_REWARD, 0);
}

#[test]
fn military_zone_stamp_small_xbld_clear_preserves_xtrf() {
    // Shared military-zone small-XBLD clear; leave XTRF (and parks) alone.
    let mut city = City::new_blank("/tmp/sc2000-mil-small-xbld.sc2", "MilSmall").expect("blank");
    city.set_building(55, 55, 5); // under SMALL_PARK
    city.set_building(56, 55, SMALL_PARK_BUILDING_ID); // must keep
    city.set_traffic_density(55, 55, 0x61);
    city.set_traffic_density(56, 55, 0x62);
    let mut state = test_state(city);
    state.stamp_military_base_plot(55, 55);
    assert_eq!(state.city.building_at(55, 55), 0);
    assert_eq!(state.city.building_at(56, 55), SMALL_PARK_BUILDING_ID);
    assert_eq!(
        state.city.zone_at(55, 55).map(|z| z.raw_bits()),
        Some(ZONE_MILITARY | ZONE_CORNER_ALL)
    );
    assert_eq!(state.city.traffic_density_at(55, 55), Some(0x61));
    assert_eq!(state.city.traffic_density_at(56, 55), Some(0x62));
}

#[test]
fn pe_clear_small_xbld_place_tile_clears_rubble_1_to_4() {
    // PlaceTile(0) clears rubble/construction `1..=4` like PE zone-stamp PlaceTile arms.
    let mut city = City::new_blank("/tmp/sc2000-mil-rubble-pt.sc2", "MilRubblePT").expect("blank");
    for (i, b) in [1u8, 2, 3, 4].into_iter().enumerate() {
        let x = 50 + i;
        city.set_building(x, 50, b);
        city.set_text_overlay(x, 50, 0x21);
        city.set_traffic_density(x, 50, 0x44);
        pe_clear_small_xbld_for_military_zone(&mut city, x, 50);
        assert_eq!(city.building_at(x, 50), 0);
        assert_eq!(city.text_overlay_at(x, 50), Some(0x21));
        assert_eq!(city.traffic_density_at(x, 50), Some(0x44));
    }
}

#[test]
fn mac68k_military_zone_stamp_small_xbld_clear_preserves_xtrf() {
    // Shared military-zone small-XBLD clear leaves XTRF; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-mil-small-xbld.sc2", "MacMilSmall").expect("blank");
    city.set_building(55, 55, 5);
    city.set_building(56, 55, SMALL_PARK_BUILDING_ID);
    city.set_traffic_density(55, 55, 0x61);
    city.set_traffic_density(56, 55, 0x62);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_military_base_plot(55, 55);
    assert_eq!(state.city.building_at(55, 55), 0);
    assert_eq!(state.city.building_at(56, 55), SMALL_PARK_BUILDING_ID);
    assert_eq!(state.city.traffic_density_at(55, 55), Some(0x61));
    assert_eq!(state.city.traffic_density_at(56, 55), Some(0x62));
}

#[test]
fn pe_army_after_runway_dd_writes_xzon_corners_and_xbit_flags() {
    // PE @ `0x4384e6`: XZON `|=0xf0`; type!=7 sets XBIT `0xc0`, flag sets XBIT `0x02`.
    let mut city = City::new_blank("/tmp/sc2000-dd-zone-or.sc2", "DdZoneOr").expect("blank");
    city.set_zone(55, 55, 0x01);
    pe_army_after_runway_dd_zone_4384e6(&mut city, 55, 55, 1, true);
    assert_eq!(city.zone_at(55, 55).unwrap().raw_bits() & 0xf0, 0xf0);
    assert_eq!(city.tile_bits_at(55, 55).unwrap().raw_bits() & 0xc2, 0xc2);
    city.set_zone(55, 55, 0x01);
    city.set_tile_bits(55, 55, TileBits::new(0));
    pe_army_after_runway_dd_zone_4384e6(&mut city, 55, 55, 1, false);
    assert_eq!(city.zone_at(55, 55).unwrap().raw_bits() & 0xf0, 0xf0);
    assert_eq!(city.tile_bits_at(55, 55).unwrap().raw_bits() & 0xc2, 0xc0);
}

#[test]
fn pe_army_runway_dd_to_de_upgrades_when_xbit02_mismatches_flag() {
    // PE @ `0x43857b`: XBIT bit0x02 differs from flag → PlaceTile(0xDE).
    let mut city = City::new_blank("/tmp/sc2000-dd-to-de.sc2", "DdToDe").expect("blank");
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_building(55, 55, RUNWAY_BUILDING_ID);
    city.set_tile_bits(55, 55, TileBits::new(0x02));
    assert!(pe_army_runway_dd_to_de_43857b(
        &mut city,
        55,
        55,
        ZONE_MILITARY,
        false
    ));
    assert_eq!(city.building_at(55, 55), AIRPORT_RUNWAY_CROSS_BUILDING_ID);
    assert_eq!(city.tile_bits_at(55, 55).unwrap().raw_bits() & 0xf0, 0);
    assert_eq!(city.zone_at(55, 55).unwrap().raw_bits() & 0xf0, 0xf0);
    assert_eq!(
        city.zone_at(55, 55).unwrap().raw_bits() & 0x02,
        ZONE_MILITARY & 0x02,
        "PlaceTile preserves the XZON zone-type nibble"
    );
    // Matching flag refuses.
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_building(55, 55, RUNWAY_BUILDING_ID);
    city.set_tile_bits(55, 55, TileBits::new(0x02));
    assert!(!pe_army_runway_dd_to_de_43857b(
        &mut city,
        55,
        55,
        ZONE_MILITARY,
        true
    ));
    assert_eq!(city.building_at(55, 55), RUNWAY_BUILDING_ID);
}

#[test]
fn pe_army_runway_dd_to_de_ors_xzon_corners_when_type_not_7() {
    // PE @ `0x4385ee`: XZON `|= 0xf0`, XBIT `|= 0xc0` after PlaceTile(0xDE).
    let mut city = City::new_blank("/tmp/sc2000-dd-to-de-c0.sc2", "DdToDeC0").expect("blank");
    city.set_zone(56, 56, 0x01 | 0x02); // type 1 with bit0x02
    city.set_building(56, 56, RUNWAY_BUILDING_ID);
    city.set_tile_bits(56, 56, TileBits::new(0x02));
    assert!(pe_army_runway_dd_to_de_43857b(
        &mut city, 56, 56, 0x01, false
    ));
    assert_eq!(city.building_at(56, 56), AIRPORT_RUNWAY_CROSS_BUILDING_ID);
    assert_eq!(city.zone_at(56, 56).unwrap().raw_bits() & 0xf0, 0xf0);
    assert_eq!(city.tile_bits_at(56, 56).unwrap().raw_bits() & 0xc0, 0xc0);
    assert_eq!(city.zone_at(56, 56).unwrap().raw_bits() & 0x02, 0x02);
    assert_eq!(city.tile_bits_at(56, 56).unwrap().raw_bits() & 0x02, 0);
}

#[test]
fn army_runway_cross_midrange_clears_even_2x2() {
    // PE FUN_004392A0 @ 0x43935f: XBLD in [0xC6,0xDB) aligns to even NW and clears 2×2.
    let mut city = City::new_blank("/tmp/sc2000-runway-2x2.sc2", "Runway2x2").expect("blank");
    city.set_building(55, 55, 0xc6);
    city.set_building(54, 54, 7);
    city.set_building(55, 54, 8);
    city.set_building(54, 55, 9);
    city.set_zone(54, 54, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_zone(55, 54, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_zone(54, 55, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    for &(x, y) in &[(54, 54), (55, 54), (54, 55), (55, 55)] {
        city.set_tile_bits(
            x,
            y,
            TileBits::new(0)
                .with_piped(true)
                .with_watered(true)
                .with_powered(true)
                .with_powerable(true),
        );
        city.set_text_overlay(x, y, 0x31);
    }
    let mut state = test_state(city);
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.building_at(54, 54), 0);
    assert_eq!(state.city.building_at(55, 54), 0);
    assert_eq!(state.city.building_at(54, 55), 0);
    for &(x, y) in &[(54, 54), (55, 54), (54, 55)] {
        let zone = state.city.zone_at(x, y).unwrap();
        assert_eq!(zone.raw_bits() & 0xf0, 0, "corners cleared at ({x},{y})");
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped());
        assert!(bits.watered());
        assert!(!bits.powered());
        assert!(!bits.powerable());
        assert_eq!(state.city.text_overlay_at(x, y), Some(0x31));
    }
    let zone = state.city.zone_at(55, 55).unwrap();
    assert_eq!(zone.zone_id(), ZONE_MILITARY);
    // The clear helper masks old corners, then FormArmy restores them on the runway.
    assert_eq!(zone.raw_bits() & 0xf0, ZONE_CORNER_ALL);
}

#[test]
fn army_runway_cross_above_ea_also_clears_even_2x2() {
    // PE: XBLD > 0xEA takes the same 2×2 arm as mid-range.
    let mut city = City::new_blank("/tmp/sc2000-runway-hi.sc2", "RunwayHi").expect("blank");
    city.set_building(40, 41, 0xeb);
    city.set_building(40, 40, 3);
    city.set_building(41, 40, 4);
    city.set_building(41, 41, 5);
    city.set_zone(40, 41, ZONE_MILITARY | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.maybe_stamp_army_runway_cross(40, 41, false);
    assert_eq!(state.city.building_at(40, 41), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.building_at(40, 40), 0);
    assert_eq!(state.city.building_at(41, 40), 0);
    assert_eq!(state.city.building_at(41, 41), 0);
}

#[test]
fn mac68k_army_runway_cross_midrange_clears_even_2x2() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-runway-2x2.sc2", "MacRunway2x2").expect("blank");
    city.set_building(55, 55, 0xc8);
    city.set_building(54, 54, 2);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.building_at(54, 54), 0);
}

#[test]
fn army_runway_cross_park_clear_is_noop_then_place() {
    // FUN_004392A0 no-ops XBLD < 0xC6; PlaceTile(0xDD) still stamps over park.
    let mut city = City::new_blank("/tmp/sc2000-runway-park.sc2", "RunwayPark").expect("blank");
    city.set_building(55, 55, SMALL_PARK_BUILDING_ID);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_traffic_density(55, 55, 0x22);
    let mut state = test_state(city);
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.traffic_density_at(55, 55), Some(0x22));
    let zone = state.city.zone_at(55, 55).unwrap();
    assert_eq!(zone.raw_bits() & 0xf0, ZONE_CORNER_ALL);
}

#[test]
fn army_runway_end_place_tile_dd_ors_xzon_f0() {
    // PE @ `0x43848a`/`0x4384b9`: PlaceTile(0xDD) then XZON `|= 0xf0`.
    let mut city = City::new_blank("/tmp/sc2000-runway-dd-f0.sc2", "RunwayDdF0").expect("blank");
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_tile_bits(55, 55, TileBits::new(0));
    let mut state = test_state(city);
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.zone_at(55, 55).unwrap().raw_bits() & 0xf0, 0xf0);
    assert_eq!(
        state.city.tile_bits_at(55, 55).unwrap().raw_bits() & 0xf0,
        0
    );
}

#[test]
fn army_runway_cross_preserves_xtxt_like_pe_place_tile() {
    // PE PlaceTile runway-cross stamp leaves XTXT.
    let mut city = City::new_blank("/tmp/sc2000-runway-xtxt.sc2", "RunwayT").expect("blank");
    city.set_building(55, 55, 29);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_text_overlay(55, 55, 0x3b);
    let mut state = test_state(city);
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.text_overlay_at(55, 55), Some(0x3b));
}

#[test]
fn mac68k_army_runway_cross_preserves_xtxt_like_pe_place_tile() {
    // Shared runway-cross PlaceTile stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-runway-xtxt.sc2", "MacRunwayT").expect("blank");
    city.set_building(55, 55, 29);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_text_overlay(55, 55, 0x3b);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.text_overlay_at(55, 55), Some(0x3b));
}

#[test]
fn mac68k_army_runway_cross_preserves_xtrf() {
    // Shared runway-cross stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-runway-xtrf.sc2", "MacRunwayX").expect("blank");
    city.set_building(55, 55, 29);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_traffic_density(55, 55, 0x46);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.traffic_density_at(55, 55), Some(0x46));
}

#[test]
fn stamp_power_plant_xbit_powerable_preserves_xtxt() {
    // PE plant XBIT OR does not write XTXT.
    let mut city = City::new_blank("/tmp/sc2000-plant-xtxt.sc2", "PlantT").expect("blank");
    city.set_text_overlay(10, 10, 0x41);
    stamp_power_plant_xbit_powerable(&mut city, &[(10, 10)]);
    assert!(city.tile_bits_at(10, 10).unwrap().powerable());
    assert_eq!(city.text_overlay_at(10, 10), Some(0x41));
}

#[test]
fn mac68k_stamp_power_plant_xbit_powerable_preserves_xtxt() {
    // Shared plant XBIT OR leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-plant-xtxt.sc2", "MacPlantT").expect("blank");
    city.set_text_overlay(10, 10, 0x41);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    stamp_power_plant_xbit_powerable(&mut state.city, &[(10, 10)]);
    assert_eq!(state.city.text_overlay_at(10, 10), Some(0x41));
}

#[test]
fn mac68k_stamp_power_plant_xbit_powerable_preserves_xtrf() {
    // Shared plant XBIT OR leaves XTRF; Mac68k path must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-plant-xtrf.sc2", "MacPlantX").expect("blank");
    city.set_traffic_density(10, 10, 0x29);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    stamp_power_plant_xbit_powerable(&mut state.city, &[(10, 10)]);
    assert!(state.city.tile_bits_at(10, 10).unwrap().powerable());
    assert_eq!(state.city.traffic_density_at(10, 10), Some(0x29));
}

#[test]
fn stamp_xbit_piped_preserves_xtxt() {
    // PE XBIT piped OR does not write XTXT.
    let mut city = City::new_blank("/tmp/sc2000-piped-xtxt.sc2", "PipedT").expect("blank");
    city.set_text_overlay(12, 12, 0x42);
    stamp_xbit_piped(&mut city, 12, 12);
    assert!(city.tile_bits_at(12, 12).unwrap().piped());
    assert_eq!(city.text_overlay_at(12, 12), Some(0x42));
}

#[test]
fn mac68k_stamp_xbit_piped_preserves_xtxt() {
    // Shared XBIT piped OR leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-piped-xtxt.sc2", "MacPipedT").expect("blank");
    city.set_text_overlay(12, 12, 0x42);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    stamp_xbit_piped(&mut state.city, 12, 12);
    assert_eq!(state.city.text_overlay_at(12, 12), Some(0x42));
}

#[test]
fn mac68k_stamp_xbit_piped_preserves_xtrf() {
    // Shared XBIT piped OR leaves XTRF; Mac68k path must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-piped-xtrf.sc2", "MacPipedX").expect("blank");
    city.set_traffic_density(12, 12, 0x1f);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    stamp_xbit_piped(&mut state.city, 12, 12);
    assert!(state.city.tile_bits_at(12, 12).unwrap().piped());
    assert_eq!(state.city.traffic_density_at(12, 12), Some(0x1f));
}

#[test]
fn pe_process_crt_carries_weather_into_next_month_power() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.refresh_live_misc_current_values();
    let entry = state.pe_process_crt_seed();
    state.advance_simulation_weather();
    assert_ne!(state.pe_process_crt_seed(), entry);
    state.sim_calendar_day1_power();
    assert_ne!(state.pe_process_crt_seed(), entry);
}

#[test]
fn pe_power_fixed_units_match_rust_tile_scaling() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (building, pe_units) in PE_POWER_FIXED_PE_UNITS {
        for y in 20..=23 {
            for x in 20..=23 {
                city.set_building(x, y, building);
            }
        }
        let expected = if matches!(building, 198 | 199) {
            pe_units
        } else {
            pe_units * 16
        };
        assert_eq!(
            power_plant_output_tiles(&city, 20, 20),
            expected,
            "building {building}"
        );
        for y in 20..=23 {
            for x in 20..=23 {
                city.set_building(x, y, 0);
            }
        }
    }
}

#[test]
fn power_supply_capacity_counts_complete_power_plants_once() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 10..=13 {
        for x in 10..=13 {
            city.set_building(x, y, 207);
        }
    }
    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 203);
        }
    }
    city.set_building(30, 30, 201);

    assert_eq!(power_supply_capacity_tiles(&city), 2_480);
}

#[test]
fn power_plant_footprint_origin_matches_complete_multi_tile_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 207);
        }
    }

    assert_eq!(power_plant_footprint_origin(&city, 20, 20), Some((20, 20)));
    assert_eq!(power_plant_footprint_origin(&city, 21, 20), Some((20, 20)));
    assert_eq!(power_plant_footprint_origin(&city, 23, 23), Some((20, 20)));
    assert_eq!(power_plant_footprint_origin(&city, 19, 20), None);
}

#[test]
fn power_supply_capacity_uses_deterministic_wind_and_solar_outputs() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 18);
    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 204);
        }
    }

    let mut rng = Win95CrtRand::new(pe_power_recalc_seed(&city));
    let remainder = pe_power_crt_rand_u16(&mut rng);
    let wind = pe_power_c8_output_tiles(&city, 10, 10, remainder);
    let mut check = Win95CrtRand::new(pe_power_recalc_seed(&city));
    assert_eq!(
        power_plant_output_tiles_with_rng(&city, 10, 10, &mut check),
        wind
    );
    let solar = power_plant_output_tiles_with_rng(&city, 20, 20, &mut check);
    assert_eq!(power_supply_capacity_tiles(&city), wind + solar);
}

#[test]
fn pe_power_wind_output_uses_c8_xval_formula() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 18);
    let mut rng = Win95CrtRand::new(pe_power_recalc_seed(&city));
    let output = power_plant_output_tiles_with_rng(&city, 10, 10, &mut rng);
    let remainder = pe_power_crt_rand_u16(&mut Win95CrtRand::new(pe_power_recalc_seed(&city)));
    assert_eq!(output, pe_power_c8_output_tiles(&city, 10, 10, remainder));
}

#[test]
fn place_building_stamps_power_plant_xbit_powerable() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));
    assert!(state.place_building_at(20, 20, 201) > 0);
    assert!(state.city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn place_statue_clears_xbit_powerable_like_pe_440835() {
    // PE CityToolPlaceSelectedBuilding statue arm @ 0x440835: AND XBIT with 0x7F.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_xbit_powerable(&mut city, 20, 20);
    let mut state = test_state(city);
    assert!(pe_power_flood_tile_accepted(&state.city, 20, 20));
    assert!(state.place_building_at(20, 20, 219) > 0);
    assert_eq!(state.city.building_at(20, 20), 219);
    assert!(!pe_power_flood_tile_accepted(&state.city, 20, 20));
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn stamp_zone_development_preserves_xtrf_like_pe_choose_and_place() {
    // PE ChooseAndPlace / PlaceTile never write XTRF (`0x4ca940`).
    let mut city = City::new_blank("/tmp/sc2000-cap-xtrf.sc2", "CapXtrf").expect("blank");
    city.set_traffic_density(20, 20, 40);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.stamp_zone_development(&ZoneDevelopmentPlan {
        origin: (20, 20),
        zone_id: ZONE_RESIDENTIAL_LIGHT,
        building: 1,
        footprint: vec![(20, 20)],
        place_as_construction: true,
    });
    assert_eq!(state.city.traffic_density_at(20, 20), Some(40));
    assert_eq!(
        state.city.tile_bits_at(20, 20).unwrap().raw_bits() & 0xe0,
        0xe0
    );
}

#[test]
fn mac68k_stamp_zone_development_preserves_xtrf_like_pe_choose_and_place() {
    // Shared ChooseAndPlace / PlaceTile leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-cap-xtrf.sc2", "MacCapX").expect("blank");
    city.set_traffic_density(20, 20, 40);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_zone_development(&ZoneDevelopmentPlan {
        origin: (20, 20),
        zone_id: ZONE_RESIDENTIAL_LIGHT,
        building: 1,
        footprint: vec![(20, 20)],
        place_as_construction: true,
    });
    assert_eq!(state.city.traffic_density_at(20, 20), Some(40));
    assert_eq!(
        state.city.tile_bits_at(20, 20).unwrap().raw_bits() & 0xe0,
        0xe0
    );
}

#[test]
fn stamp_zone_development_preserves_xtxt_like_pe_place_tile() {
    // PE ChooseAndPlace → PlaceTile `0x441f00`: stamp XBLD only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-cap-xtxt.sc2", "CapXtxt").expect("blank");
    city.set_text_overlay(20, 20, 0x2a);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.stamp_zone_development(&ZoneDevelopmentPlan {
        origin: (20, 20),
        zone_id: ZONE_RESIDENTIAL_LIGHT,
        building: 1,
        footprint: vec![(20, 20)],
        place_as_construction: true,
    });
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
    assert_ne!(state.city.building_at(20, 20), 0);
}

#[test]
fn mac68k_stamp_zone_development_preserves_xtxt_like_pe_place_tile() {
    // Shared ChooseAndPlace → PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-cap-xtxt.sc2", "MacCapT").expect("blank");
    city.set_text_overlay(20, 20, 0x2a);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_zone_development(&ZoneDevelopmentPlan {
        origin: (20, 20),
        zone_id: ZONE_RESIDENTIAL_LIGHT,
        building: 1,
        footprint: vec![(20, 20)],
        place_as_construction: true,
    });
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
    assert_ne!(state.city.building_at(20, 20), 0);
}

#[test]
fn direct_sub_to_rail_place_skips_maptool_preclear_crt() {
    // This invokes the dedicated SubToRail worker, not MapTool
    // FUN_00443270; the unrelated surface-preclear CRT stream stays untouched.
    let mut city = City::new_blank("/tmp/sc2000-s2r-map-crt.sc2", "S2rMapCrt").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(
        21,
        20,
        network_building_for_mask(NetworkKind::Rail, NETWORK_LEFT),
    );
    city.set_building(20, 20, 6);
    city.set_underground(20, 20, UNDER_TILE_GREY_BOX_2);
    let mut state = test_state(city);
    state.show_underground = true;
    state.tool = EditTool::SubToRail;
    let crt_before = state.pe_process_crt_seed();
    assert!(state.place_sub_to_rail_at(20, 20));
    assert!(matches!(state.city.building_at(20, 20), 108..=111));
    assert_eq!(state.city.underground_at(20, 20), UNDER_TILE_GREY_BOX_2);
    assert_eq!(state.pe_process_crt_seed(), crt_before);
}

#[test]
fn stamp_building_footprint_preserves_xtrf_like_pe_place_tile() {
    // PE PlaceTile (`0x441f00`) never writes XTRF (`0x4ca940`).
    let mut city = City::new_blank("/tmp/sc2000-place-xtrf.sc2", "PlaceXtrf").expect("blank");
    city.set_city_funds(50_000);
    city.set_traffic_density(20, 20, 48);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 112) > 0); // small residential
    assert_eq!(state.city.building_at(20, 20), 112);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(48));
}

#[test]
fn mac68k_stamp_building_footprint_preserves_xtrf_like_pe_place_tile() {
    // Shared PlaceTile building stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-place-xtrf.sc2", "MacPlaceX").expect("blank");
    city.set_city_funds(50_000);
    city.set_traffic_density(20, 20, 48);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_building_at(20, 20, 112) > 0); // small residential
    assert_eq!(state.city.building_at(20, 20), 112);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(48));
}

#[test]
fn place_building_at_preserves_xtxt_like_pe_place_tile() {
    // PE PlaceTile `0x441f00`: stamp XBLD only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-place-at-xtxt.sc2", "PlaceAtT").expect("blank");
    city.set_city_funds(50_000);
    city.set_text_overlay(20, 20, 0x2a);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 112) > 0);
    assert_eq!(state.city.building_at(20, 20), 112);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
}

#[test]
fn mac68k_place_building_at_preserves_xtxt_like_pe_place_tile() {
    // Shared PlaceTile stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-place-at-xtxt.sc2", "MacPlaceAtT").expect("blank");
    city.set_city_funds(50_000);
    city.set_text_overlay(20, 20, 0x2a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_building_at(20, 20, 112) > 0);
    assert_eq!(state.city.building_at(20, 20), 112);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
}

#[test]
fn mac68k_place_hydro_power_stamps_xzon_corner_all_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-hydro-xzon-mac.sc2", "HydroXzonMac").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_CORNER_ALL)
    );
}

#[test]
fn place_hydro_power_east_higher_neighbor_picks_c6_delta() {
    // PE neighbor mask bit1 (x+1 higher) → table delta 0 → PlaceTile `0xC6` at rot0.
    let mut city =
        City::new_blank("/tmp/sc2000-hydro-orient-e.sc2", "HydroOrientE").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_view_rotation(0);
    let land = city.altitude_at(20, 20);
    city.set_altitude(21, 20, land.saturating_add(1), 0);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), 198);
}

#[test]
fn place_hydro_power_west_and_north_higher_neighbors_pick_table_deltas() {
    // PE mask bit3 (x-1 higher) → delta 0 → `0xC6`; bit0 (y-1 higher) → delta 1 → `0xC7`.
    let mut city =
        City::new_blank("/tmp/sc2000-hydro-orient-w.sc2", "HydroOrientW").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_view_rotation(0);
    let land = city.altitude_at(20, 20);
    city.set_altitude(19, 20, land.saturating_add(1), 0);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), 198);

    let mut city =
        City::new_blank("/tmp/sc2000-hydro-orient-n.sc2", "HydroOrientN").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(22, 22, 0x3e);
    city.set_view_rotation(0);
    let land = city.altitude_at(22, 22);
    city.set_altitude(22, 21, land.saturating_add(1), 0);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(22, 22), 1);
    assert_eq!(state.city.building_at(22, 22), 199);
}

#[test]
fn place_hydro_power_south_higher_neighbor_picks_c7_delta() {
    // PE mask bit2 (y+1 higher) → table delta 1 → PlaceTile `0xC7` at rot0.
    let mut city =
        City::new_blank("/tmp/sc2000-hydro-orient-s.sc2", "HydroOrientS").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(24, 24, 0x3e);
    city.set_view_rotation(0);
    let land = city.altitude_at(24, 24);
    city.set_altitude(24, 25, land.saturating_add(1), 0);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(24, 24), 1);
    assert_eq!(state.city.building_at(24, 24), 199);
}

#[test]
fn place_hydro_power_west_higher_rot1_xors_delta_to_c7() {
    // PE @ `0x441606`: rotation bit0 xors delta; west mask delta 0 ^ 1 → `0xC7`.
    let mut city =
        City::new_blank("/tmp/sc2000-hydro-orient-w1.sc2", "HydroOrientW1").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(26, 26, 0x3e);
    city.set_view_rotation(1);
    let land = city.altitude_at(26, 26);
    city.set_altitude(25, 26, land.saturating_add(1), 0);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(26, 26), 1);
    assert_eq!(state.city.building_at(26, 26), 199);
}

#[test]
fn place_hydro_power_ors_xbit_powerable_before_place_tile() {
    // PE MapTool hydro @ `0x441640`: OR XBIT `0x80` before PlaceTile; leave watered/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-hydro-xbit.sc2", "HydroXbit").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_tile_bits(20, 20, TileBits::new(0).with_watered(true));
    city.set_traffic_density(20, 20, 0x44);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), 199);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.powerable() && bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x44));
    // PE @ `0x4416b7`→`0x44cf60`: system hydro microsim XTXT on success.
    let xtxt = state.city.text_overlay_at(20, 20).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "hydro microsim XTXT {xtxt}");
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_CORNER_ALL)
    );
}

#[test]
fn place_hydro_power_stamps_xzon_corner_all_like_pe() {
    // PE MapTool plant/hydro @ `0x441679`/`0x44169e`: XZON &= 0xf0; |= 0xf0.
    let mut city = City::new_blank("/tmp/sc2000-hydro-xzon.sc2", "HydroXzon").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), 199);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_CORNER_ALL)
    );
}

#[test]
fn mac68k_place_onramp_leaves_xzon_like_pe_maptool_onramp() {
    let mut city = City::new_blank("/tmp/sc2000-onramp-xzon-mac.sc2", "OnrampZMac").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    state
        .city
        .set_zone(ramp.0, ramp.1, ZONE_RESIDENTIAL_LIGHT | 0x80);
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(
        state.city.zone_at(ramp.0, ramp.1).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x80)
    );
}

#[test]
fn place_onramp_leaves_xzon_like_pe_maptool_onramp() {
    // PE MapToolOnramp never writes XZON (unlike PlaceRoads/Rails type nibble clear).
    let mut city = City::new_blank("/tmp/sc2000-onramp-xzon.sc2", "OnrampZ").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    state
        .city
        .set_zone(ramp.0, ramp.1, ZONE_RESIDENTIAL_LIGHT | 0x80);
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(
        state.city.zone_at(ramp.0, ramp.1).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x80)
    );
    assert!(matches!(state.city.building_at(ramp.0, ramp.1), 93..=96));
}

#[test]
fn onramp_refuses_when_xbld_ge_0x0e() {
    // PE MapTool Onramp @ `0x44183f`: XBLD `>= 0x0e` → refuse (power lines included).
    let mut city = City::new_blank("/tmp/sc2000-onramp-pwr.sc2", "OnrampPwr").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    assert!(state.onramp_plan_at(ramp.0, ramp.1).is_some());
    state.city.set_building(ramp.0, ramp.1, 0x0e);
    assert!(state.onramp_plan_at(ramp.0, ramp.1).is_none());
    assert!(!state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(state.city.building_at(ramp.0, ramp.1), 0x0e);
}

#[test]
fn onramp_allows_small_park_0x0d_unlike_place_tile_size() {
    // PE Onramp early gates refuse `>=0x0e` and `5` only — not PlaceTile-size `0x0D`.
    let mut city = City::new_blank("/tmp/sc2000-onramp-park.sc2", "OnrampPark").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    state.city.set_building(ramp.0, ramp.1, 0x0d);
    assert!(state.onramp_plan_at(ramp.0, ramp.1).is_some());
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    assert!(matches!(state.city.building_at(ramp.0, ramp.1), 93..=96));
}

#[test]
fn pe_onramp_compat_table_index_15_is_zero_unlike_mac() {
    // PE MapTool Onramp stack table index `0x0f` is `0`; Mac 68k keeps `0x01`.
    assert_eq!(pe_onramp_compatible_road_mask(0x0f, 0xff), 0);
    assert_eq!(mac_onramp_compatible_road_mask(0x0f, 0xff), 0x01);
    assert_eq!(pe_onramp_compatible_road_mask(0x01, 0x0a), 0x0a);
    assert_eq!(pe_onramp_compatible_road_mask(0x01, 0x01), 0);
}

#[test]
fn onramp_debits_0x4dc260_cost_before_place_tile() {
    // PE @ `0x44197b`: debit `0x4dc260[3]=25` before PlaceTile; funds < 25 refuse.
    let mut city = City::new_blank("/tmp/sc2000-onramp-funds.sc2", "OnrampFunds").expect("blank");
    city.set_city_funds(24);
    let mut state = test_state(city);
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    assert!(!state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(state.city.city_funds(), 24);
    assert_eq!(state.city.building_at(ramp.0, ramp.1), 0);

    state.city.set_city_funds(25);
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(state.city.city_funds(), 0);
    assert!(matches!(state.city.building_at(ramp.0, ramp.1), 93..=96));
}

#[test]
fn onramp_ors_xbit_rotate_only_for_ns_road_arms() {
    // PE @ `0x4419db`/`0x441a65`: NS-road arms OR click XBIT `|=0x02`; EW arms leave alone.
    let mut city = City::new_blank("/tmp/sc2000-onramp-xbit.sc2", "OnrampXbit").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);

    // Highway EW + road NS → PE sets rotate.
    stamp_onramp_fixture(&mut state, ramp, NETWORK_LEFT, NETWORK_TOP);
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    assert!(state.city.tile_bits_at(ramp.0, ramp.1).unwrap().rotate());

    // Highway NS + road EW → PE leaves XBIT alone (preserve pre-set rotate).
    let ramp2 = (70, 70);
    stamp_onramp_fixture(&mut state, ramp2, NETWORK_TOP, NETWORK_RIGHT);
    state
        .city
        .set_tile_bits(ramp2.0, ramp2.1, TileBits::new(0).with_rotate(true));
    assert!(state.place_onramp_at(ramp2.0, ramp2.1));
    assert!(state.city.tile_bits_at(ramp2.0, ramp2.1).unwrap().rotate());
}

#[test]
fn place_onramp_preserves_xtxt_like_pe_place_tile() {
    // PE MapToolOnramp → PlaceTile `0x441f00`: ramp/road XBLD only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-onramp-xtxt.sc2", "OnrampT").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    state.city.set_text_overlay(ramp.0, ramp.1, 0x2c);
    let plan = state.onramp_plan_at(ramp.0, ramp.1).expect("onramp plan");
    state
        .city
        .set_text_overlay(plan.road_tile.0, plan.road_tile.1, 0x2d);
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(state.city.text_overlay_at(ramp.0, ramp.1), Some(0x2c));
    assert_eq!(
        state
            .city
            .text_overlay_at(plan.road_tile.0, plan.road_tile.1),
        Some(0x2d)
    );
}

#[test]
fn mac68k_place_onramp_preserves_xtxt_like_pe_place_tile() {
    // Shared onramp PlaceTile stamps leave XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-onramp-xtxt.sc2", "MacOnrampT").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    state.city.set_text_overlay(ramp.0, ramp.1, 0x2c);
    let plan = state.onramp_plan_at(ramp.0, ramp.1).expect("onramp plan");
    state
        .city
        .set_text_overlay(plan.road_tile.0, plan.road_tile.1, 0x2d);
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(state.city.text_overlay_at(ramp.0, ramp.1), Some(0x2c));
    assert_eq!(
        state
            .city
            .text_overlay_at(plan.road_tile.0, plan.road_tile.1),
        Some(0x2d)
    );
}

#[test]
fn mac68k_place_hydro_power_preserves_xtrf_like_pe_place_tile() {
    // Shared PlaceTile hydro stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-hydro-xtrf.sc2", "MacHydroX").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_traffic_density(20, 20, 0x43);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), 199);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x43));
}

#[test]
fn place_underground_preserves_xtxt_like_pe_place_pipes() {
    // PE PlacePipes / subway reshape: leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-under-xtxt.sc2", "UnderT").expect("blank");
    city.set_city_funds(50_000);
    city.set_text_overlay(20, 20, 0x3e);
    city.set_text_overlay(22, 20, 0x3f);
    let mut state = test_state(city);
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Subway),
        1
    );
    assert_eq!(
        state.place_underground_at(22, 20, UndergroundKind::Pipes),
        1
    );
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x3e));
    assert_eq!(state.city.text_overlay_at(22, 20), Some(0x3f));
}

#[test]
fn mac68k_place_underground_preserves_xtrf_like_pe_place_pipes() {
    // Shared PlacePipes/subway reshape leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-under-xtrf.sc2", "MacUnderX").expect("blank");
    city.set_city_funds(50_000);
    city.set_traffic_density(20, 20, 0x77);
    city.set_traffic_density(22, 20, 0x88);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Subway),
        1
    );
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x77));
    assert_eq!(
        state.place_underground_at(22, 20, UndergroundKind::Pipes),
        1
    );
    assert_eq!(state.city.traffic_density_at(22, 20), Some(0x88));
}

#[test]
fn mac68k_place_underground_preserves_xtxt_like_pe_place_pipes() {
    // Shared PlacePipes/subway reshape leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-under-xtxt.sc2", "MacUnderT").expect("blank");
    city.set_city_funds(50_000);
    city.set_text_overlay(20, 20, 0x3e);
    city.set_text_overlay(22, 20, 0x3f);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Subway),
        1
    );
    assert_eq!(
        state.place_underground_at(22, 20, UndergroundKind::Pipes),
        1
    );
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x3e));
    assert_eq!(state.city.text_overlay_at(22, 20), Some(0x3f));
}

#[test]
fn onramp_and_flat_highway_preserve_xtrf_like_pe_place_tile() {
    // PE PlaceTile (`0x441f00`) never writes XTRF (`0x4ca940`).
    let mut city = City::new_blank("/tmp/sc2000-hwy-xtrf.sc2", "HwyXtrf").expect("blank");
    city.set_city_funds(50_000);
    city.set_traffic_density(64, 64, 52);
    city.set_traffic_density(30, 30, 53);
    let mut state = test_state(city);
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(state.city.traffic_density_at(64, 64), Some(52));

    state.stamp_flat_highway_tiles(106, &[(30, 30), (31, 30)]);
    assert_eq!(state.city.building_at(30, 30), 106);
    assert_eq!(state.city.traffic_density_at(30, 30), Some(53));
}

#[test]
fn mac68k_place_onramp_preserves_xtrf_like_pe_place_tile() {
    // Shared PlaceTile onramp stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-onramp-xtrf.sc2", "MacOnX").expect("blank");
    city.set_city_funds(50_000);
    city.set_traffic_density(64, 64, 52);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(state.city.traffic_density_at(64, 64), Some(52));
}

#[test]
fn mac68k_stamp_flat_highway_preserves_xtrf_like_pe_place_tile() {
    // Shared PlaceTile flat-highway stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-flat-hwy-xtrf.sc2", "MacHwyX").expect("blank");
    city.set_city_funds(50_000);
    city.set_traffic_density(30, 30, 53);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_flat_highway_tiles(106, &[(30, 30), (31, 30)]);
    assert_eq!(state.city.building_at(30, 30), 106);
    assert_eq!(state.city.traffic_density_at(30, 30), Some(53));
}

#[test]
fn stamp_flat_highway_preserves_xtxt_like_pe_place_tile() {
    // PE PlaceTile flat-highway stamp leaves XTXT.
    let mut city = City::new_blank("/tmp/sc2000-flat-hwy-xtxt.sc2", "FlatHwyT").expect("blank");
    city.set_city_funds(50_000);
    city.set_text_overlay(30, 30, 0x3c);
    city.set_text_overlay(31, 30, 0x3d);
    let mut state = test_state(city);
    state.stamp_flat_highway_tiles(106, &[(30, 30), (31, 30)]);
    assert_eq!(state.city.building_at(30, 30), 106);
    assert_eq!(state.city.text_overlay_at(30, 30), Some(0x3c));
    assert_eq!(state.city.text_overlay_at(31, 30), Some(0x3d));
}

#[test]
fn mac68k_stamp_flat_highway_preserves_xtxt_like_pe_place_tile() {
    // Shared flat-highway PlaceTile stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-flat-hwy-xtxt.sc2", "MacFlatHwyT").expect("blank");
    city.set_city_funds(50_000);
    city.set_text_overlay(30, 30, 0x3c);
    city.set_text_overlay(31, 30, 0x3d);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_flat_highway_tiles(106, &[(30, 30), (31, 30)]);
    assert_eq!(state.city.building_at(30, 30), 106);
    assert_eq!(state.city.text_overlay_at(30, 30), Some(0x3c));
    assert_eq!(state.city.text_overlay_at(31, 30), Some(0x3d));
}

#[test]
fn place_highway_path_preserves_xtxt_like_pe_place_tile() {
    // PE PlaceTile `0x441f00`: highway stamp leaves XTXT.
    let mut city = City::new_blank("/tmp/sc2000-hwy-path-xtxt.sc2", "HwyPathT").expect("blank");
    city.set_city_funds(50_000);
    city.set_text_overlay(20, 20, 0x38);
    let mut state = test_state(city);
    assert_eq!(
        state.place_highway_path(&highway_drag_path((20, 20), (20, 20))),
        1
    );
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x38));
}

#[test]
fn mac68k_place_highway_path_preserves_xtxt_like_pe_place_tile() {
    // Shared PlaceTile highway path leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-hwy-path-xtxt.sc2", "MacHwyPT").expect("blank");
    city.set_city_funds(50_000);
    city.set_text_overlay(20, 20, 0x38);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_highway_path(&highway_drag_path((20, 20), (20, 20))),
        1
    );
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x38));
}

#[test]
fn mac68k_place_highway_path_preserves_xtrf_like_pe_place_tile() {
    // Shared PlaceTile highway path leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-hwy-path-xtrf.sc2", "MacHwyPX").expect("blank");
    city.set_city_funds(50_000);
    city.set_traffic_density(20, 20, 0x4c);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_highway_path(&highway_drag_path((20, 20), (20, 20))),
        1
    );
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x4c));
}

#[test]
fn place_user_pipe_ors_xbit_piped_like_pe_44b829() {
    // PE PlacePipesAtCoordinates @ 0x44b829: OR XBIT 0x20 after committing pipe XUND.
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert!(
        state
            .city
            .tile_bits_at(30, 30)
            .map(|bits| !bits.piped())
            .unwrap_or(true)
    );
    assert_eq!(
        state.place_underground_at(30, 30, UndergroundKind::Pipes),
        1
    );
    assert!(
        state.city.tile_bits_at(30, 30).unwrap().piped(),
        "user PlacePipes must OR XBIT piped like PE 0x44b829"
    );
}

#[test]
fn underground_building_pipes_follow_xbit_piped_state() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    assert_eq!(building_internal_pipe_tile(&city, 10, 10), None);

    city.set_tile_bits(10, 10, crate::city::TileBits::new(0b0010_0000));
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), None);

    city.set_building(10, 10, 112);
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), Some(16));
    city.set_building(
        10,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT),
    );
    assert_eq!(building_internal_pipe_tile(&city, 10, 10), None);
}

#[test]
fn underground_surface_roads_resolve_neighbor_connections_for_draw() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.set_building(
        10,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.city.set_building(
        10,
        9,
        network_building_for_mask(NetworkKind::Road, NETWORK_TOP | NETWORK_BOTTOM),
    );
    state.city.set_building(
        10,
        11,
        network_building_for_mask(NetworkKind::Road, NETWORK_TOP | NETWORK_BOTTOM),
    );
    state.city.set_building(
        9,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.city.set_building(
        11,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );

    assert_eq!(
        state.underground_surface_building_at(10, 10),
        network_building_for_mask(
            NetworkKind::Road,
            NETWORK_TOP | NETWORK_BOTTOM | NETWORK_LEFT | NETWORK_RIGHT
        )
    );
}

#[test]
fn recovered_xund_special_tiles_keep_distinct_network_roles() {
    let water_network_tile =
        |tile| underground_user_pipe_tile(tile) || underground_building_pipe_tile(tile);

    assert!(underground_building_pipe_tile(UNDER_TILE_BUILDING_PIPES));
    assert!(water_network_tile(UNDER_TILE_BUILDING_PIPES));
    assert!(!UndergroundKind::Subway.contains(UNDER_TILE_BUILDING_PIPES));

    for tile in [UNDER_TILE_GREY_BOX_1, UNDER_TILE_GREY_BOX_2] {
        assert!(!underground_building_pipe_tile(tile));
        assert!(!water_network_tile(tile));
        assert!(!UndergroundKind::Pipes.contains(tile));
    }
}

#[test]
fn tunnel_tool_scans_hillside_and_stamps_depths_and_opposite_entrances() {
    let mut city = tunnel_test_city();
    city.set_building(19, 20, 112);
    let mut state = test_state(city);
    state.tool = EditTool::Tunnel;

    let placement = state.tool_placement_at(20, 20).expect("tunnel placement");
    assert_eq!(placement.preview, ToolPreview::Building(63));
    assert_eq!(
        placement.valid_tiles,
        vec![(20, 20), (19, 20), (18, 20), (17, 20)]
    );
    assert!(placement.invalid_tiles.is_empty());

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 63);
    assert_eq!(state.city.building_at(17, 20), 65);
    assert_eq!(state.city.building_at(19, 20), 112);
    assert_eq!(state.city.tunnel_levels_at(20, 20), 1);
    assert_eq!(state.city.tunnel_levels_at(19, 20), 3);
    assert_eq!(state.city.tunnel_levels_at(18, 20), 2);
    assert_eq!(state.city.tunnel_levels_at(17, 20), 1);
    assert_eq!(state.city.underground_at(19, 20), 0);
}

#[test]
fn mac68k_place_tunnel_leaves_xzon_like_pe_maptool_tunnel() {
    let mut city = tunnel_test_city();
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_zone(17, 20, ZONE_RESIDENTIAL_LIGHT | 0x40);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Tunnel;
    assert_eq!(state.place_tunnel_at(20, 20), 4);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x80)
    );
    assert_eq!(
        state.city.zone_at(17, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x40)
    );
}

#[test]
fn place_tunnel_leaves_xzon_like_pe_maptool_tunnel() {
    // PE MapToolTunnel PlaceTile portals never clear XZON.
    let mut city = tunnel_test_city();
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_zone(17, 20, ZONE_RESIDENTIAL_LIGHT | 0x40);
    let mut state = test_state(city);
    state.tool = EditTool::Tunnel;
    assert_eq!(state.place_tunnel_at(20, 20), 4);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x80)
    );
    assert_eq!(
        state.city.zone_at(17, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x40)
    );
}

#[test]
fn tunnel_tool_preserves_xtrf_like_pe_place_tile() {
    // PE PlaceTile (`0x441f00`) never writes XTRF (`0x4ca940`).
    let mut city = tunnel_test_city();
    city.set_traffic_density(20, 20, 61);
    city.set_traffic_density(17, 20, 62);
    let mut state = test_state(city);
    state.tool = EditTool::Tunnel;
    assert_eq!(state.place_tunnel_at(20, 20), 4);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(61));
    assert_eq!(state.city.traffic_density_at(17, 20), Some(62));
}

#[test]
fn tunnel_tool_preserves_xtxt_like_pe_place_tile() {
    // PE PlaceTile portal stamps leave XTXT.
    let mut city = tunnel_test_city();
    city.set_text_overlay(20, 20, 0x36);
    city.set_text_overlay(17, 20, 0x37);
    let mut state = test_state(city);
    state.tool = EditTool::Tunnel;
    assert_eq!(state.place_tunnel_at(20, 20), 4);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x36));
    assert_eq!(state.city.text_overlay_at(17, 20), Some(0x37));
}

#[test]
fn mac68k_place_tunnel_preserves_xtrf_like_pe_place_tile() {
    // Shared PlaceTile portal stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = tunnel_test_city();
    city.set_traffic_density(20, 20, 61);
    city.set_traffic_density(17, 20, 62);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Tunnel;
    assert_eq!(state.place_tunnel_at(20, 20), 4);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(61));
    assert_eq!(state.city.traffic_density_at(17, 20), Some(62));
}

#[test]
fn mac68k_place_tunnel_preserves_xtxt_like_pe_place_tile() {
    // Shared PlaceTile portal stamps leave XTXT; Mac68k must not invent a clear.
    let mut city = tunnel_test_city();
    city.set_text_overlay(20, 20, 0x36);
    city.set_text_overlay(17, 20, 0x37);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Tunnel;
    assert_eq!(state.place_tunnel_at(20, 20), 4);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x36));
    assert_eq!(state.city.text_overlay_at(17, 20), Some(0x37));
}

#[test]
fn tunnel_tool_obeys_shallow_underground_and_existing_tunnel_conflicts() {
    let mut city = tunnel_test_city();
    city.set_underground(18, 20, 16);
    let mut state = test_state(city);
    state.tool = EditTool::Tunnel;

    assert!(state.tunnel_plan_at(20, 20).is_none());
    assert!(!state.apply_tool_at_tile(20, 20));

    state
        .city
        .set_underground(18, 20, UNDER_TILE_BUILDING_PIPES);
    assert!(state.tunnel_plan_at(20, 20).is_some());

    state.city.set_tunnel_levels(19, 20, 2);
    assert!(state.tunnel_plan_at(20, 20).is_none());
}

#[test]
fn zone_rectangles_skip_unavailable_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(10, 10, 112);
    city.set_terrain(11, 10, 0x30);
    let mut state = test_state(city);
    state.tool = EditTool::ZoneResidential;

    let placement = state.zone_placement_for_rect((10, 10), (12, 11), 1);
    assert_eq!(placement.invalid_tiles, vec![(10, 10), (11, 10)]);
    assert_eq!(
        placement.valid_tiles,
        vec![(12, 10), (10, 11), (11, 11), (12, 11)]
    );

    assert_eq!(state.place_zone_rect((10, 10), (12, 11), 1), 4);
    assert_eq!(state.city.zone_at(10, 10).unwrap().raw_bits(), 0);
    assert_eq!(state.city.zone_at(11, 10).unwrap().raw_bits(), 0);
    assert_eq!(
        state.city.zone_at(12, 10).unwrap().raw_bits(),
        1 | ZONE_CORNER_ALL
    );
    assert_eq!(
        state.city.zone_at(10, 11).unwrap().raw_bits(),
        1 | ZONE_CORNER_ALL
    );
    assert_eq!(
        state.city.zone_at(11, 11).unwrap().raw_bits(),
        1 | ZONE_CORNER_ALL
    );
    assert_eq!(
        state.city.zone_at(12, 11).unwrap().raw_bits(),
        1 | ZONE_CORNER_ALL
    );
}

#[test]
fn place_zone_sets_all_xzon_corner_bits_on_vacant_cells() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.tool = EditTool::ZoneResidential;

    assert_eq!(state.place_zone_at(30, 30, ZONE_RESIDENTIAL_LIGHT), 1);
    assert_eq!(
        state.city.zone_at(30, 30).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL
    );
    assert_eq!(
        state.city.zone_at(30, 30).unwrap().zone_id(),
        ZONE_RESIDENTIAL_LIGHT
    );

    assert_eq!(state.place_zone_at(30, 30, 0), 1);
    // PE @ `0x449a26`: dezone skips XZON nibble replace.
    assert_eq!(
        state.city.zone_at(30, 30).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL
    );
}

#[test]
fn mac68k_zone_tool_preserves_xtrf_like_pe_zone_write() {
    // Shared PE zone/dezone writes XZON only; Mac68k must not invent an XTRF clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-xtrf.sc2", "MacZoneX").expect("blank");
    city.set_city_funds(20_000);
    city.set_traffic_density(30, 30, 0x39);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_zone_at(30, 30, ZONE_RESIDENTIAL_LIGHT), 1);
    assert_eq!(state.city.traffic_density_at(30, 30), Some(0x39));
    assert_eq!(state.place_zone_at(30, 30, 0), 1);
    assert_eq!(state.city.traffic_density_at(30, 30), Some(0x39));
}

#[test]
fn neighbor_connection_count_uses_live_edge_infrastructure() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let east = CITY_WIDTH - 1;
    let south = CITY_HEIGHT - 1;

    city.set_building(
        0,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_building(
        0,
        11,
        network_building_for_mask(NetworkKind::Road, NETWORK_TOP | NETWORK_BOTTOM),
    );
    city.set_building(
        east,
        12,
        network_building_for_mask(NetworkKind::Rail, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_building(
        30,
        0,
        network_building_for_mask(NetworkKind::Power, NETWORK_TOP | NETWORK_BOTTOM),
    );
    city.set_building(60, 0, 73);
    city.set_underground(
        40,
        south,
        underground_tile_for_mask(UndergroundKind::Subway, NETWORK_TOP | NETWORK_BOTTOM),
    );
    city.set_underground(
        50,
        0,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_TOP | NETWORK_BOTTOM),
    );
    city.set_building(
        10,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );

    assert_eq!(neighbor_connection_tiles_for_city(&city), 6);
}

#[test]
fn mac_xval_zone_sample_uses_even_even_then_opposite_odd_odd() {
    // Mac FUN_00027AB4 @ 0x27f88 first reads XZON at (x & !1, y & !1),
    // then falls back to the opposite diagonal cell only when its low nibble is 0.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-xval-zone-sample.sc2", "MacXval").expect("blank city");
    city.kind = CityFileKind::MacCity;

    city.set_zone(10, 12, ZONE_RESIDENTIAL_LIGHT);
    city.set_zone(11, 13, ZONE_INDUSTRIAL_DENSE);
    for x in 10..=11 {
        for y in 12..=13 {
            assert_eq!(mac_xval_zone_at_output(&city, x, y), ZONE_RESIDENTIAL_LIGHT);
        }
    }

    // Verify the recovered selector affects the actual XVAL formula too.
    // The zone chooses a separate scratch grid as well as different center
    // and population terms, so derive the expected delta from all three.
    city.set_building(10, 12, 0x1d);
    let water_maps = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let zero = vec![0; CITY_TILES];
    let (primary, industrial) = mac_xval_terrain_grids(&city);
    let grid_x = 10 / 4;
    let grid_y = 12 / 4;
    let terrain_delta = mac_land_value_neighbor_average_terrain_grid(&primary, grid_x, grid_y, 32)
        - mac_land_value_neighbor_average_terrain_grid(&industrial, grid_x, grid_y, 32);
    let expected_zone_delta = terrain_delta
        + mac_land_value_center_bonus(ZONE_RESIDENTIAL_LIGHT, 5, 6, (32, 32))
        - mac_land_value_center_bonus(ZONE_INDUSTRIAL_DENSE, 5, 6, (32, 32))
        + mac_land_value_population_adjustment(ZONE_RESIDENTIAL_LIGHT, 0)
        - mac_land_value_population_adjustment(ZONE_INDUSTRIAL_DENSE, 0)
        - 0x15; // Mac's dense-industrial arm adds this fixed term.
    let residential_xval = live_land_value_density_map_for_city_at_center(
        &city,
        &water_maps,
        &zero,
        &zero,
        &zero,
        (32, 32),
        false,
    )[density_map_index(10, 12)];

    city.set_zone(10, 12, 0);
    for x in 10..=11 {
        for y in 12..=13 {
            assert_eq!(mac_xval_zone_at_output(&city, x, y), ZONE_INDUSTRIAL_DENSE);
        }
    }

    let industrial_xval = live_land_value_density_map_for_city_at_center(
        &city,
        &water_maps,
        &zero,
        &zero,
        &zero,
        (32, 32),
        false,
    )[density_map_index(10, 12)];
    assert_eq!(
        i32::from(residential_xval) - i32::from(industrial_xval),
        expected_zone_delta
    );

    city.set_zone(11, 13, 0);
    assert_eq!(mac_xval_zone_at_output(&city, 10, 12), 0);
}

#[test]
fn live_city_fame_people_prefer_pe_dw_when_zone_pops_ready() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_pe_zone_pops([200, 0, 0]);
    city.persist_pe_day22_population_leftover(2_000);
    let population = PopulationStats {
        total_population: 1,
        arcology_population: 0,
        ..PopulationStats::default()
    };
    let outcomes = CityServiceOutcomes::default();
    let counters = CityScenarioCounters::default();
    let fame = live_city_fame_value(&city, population, outcomes, counters);
    assert!(
        fame > 0,
        "pe_dw 2000 should score fame even when census total_population is 1"
    );
    city.clear_pe_zone_pops();
    assert_eq!(
        live_city_fame_value(&city, population, outcomes, counters),
        graph_points_for_scale(1, 2_500).min(30),
        "without pe_zone_pops, fame falls back to census total"
    );
}

#[test]
fn refresh_live_stats_persists_building_tile_counts_to_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // PlaceTile-style census for a single non-military XBLD 7 (tree).
    let old = city.building_at(10, 10) as usize;
    city.pe_dw_tile_count[old] = city.pe_dw_tile_count[old].wrapping_sub(1);
    city.pe_dw_tile_count[7] = city.pe_dw_tile_count[7].wrapping_add(1);
    city.set_building(10, 10, 7);
    stamp_complete_building_for_test(&mut city, 20, 20, 210, 3);
    let mut state = test_state(city);

    state.refresh_live_misc_stats();

    let counts = state
        .city
        .misc_u32_values(MISC_BUILDING_TILE_COUNTS_OFFSET, BUILDING_TILE_COUNT_VALUES);
    assert_eq!(counts.len(), BUILDING_TILE_COUNT_VALUES);
    assert_eq!(counts[7], 1);
    assert_eq!(counts[210], 9);
    assert_eq!(counts[0], (CITY_TILES - 10) as u32);
    assert_eq!(counts, building_tile_counts_for_city(&state.city).to_vec());
}

#[test]
fn refresh_live_stats_building_tile_counts_use_dw_tile_count() {
    // MISC `0x01f0` dumps BSS `dwTileCount`, not a live XBLD walk.
    let mut city = City::new_blank("/tmp/sc2000-misc-census.sc2", "MiscCensus").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 210, 3);
    assert_eq!(city.pe_dw_tile_count[210], 9);
    city.pe_dw_tile_count[210] = 0;
    let mut state = test_state(city);
    state.refresh_live_misc_stats();
    let counts = state
        .city
        .misc_u32_values(MISC_BUILDING_TILE_COUNTS_OFFSET, BUILDING_TILE_COUNT_VALUES);
    assert_eq!(
        counts[210], 0,
        "stale zero census must persist even when police tiles remain on the map"
    );
    assert_eq!(building_tile_count_for_city(&state.city, 210), 9);
}

#[test]
fn refresh_live_stats_persists_military_tile_counts_to_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // PE MISC `0x0fa8` serializes the live PlaceTile BSS census, not an XBLD recount.
    city.pe_military_tile_count[0] = 16;
    city.pe_military_tile_count[1] = 2;
    for index in 2..MILITARY_TILE_COUNT_VALUES {
        city.pe_military_tile_count[index] = 1;
    }
    let mut state = test_state(city);

    state.refresh_live_misc_stats();

    let counts = state
        .city
        .misc_u32_values(MISC_MILITARY_TILE_COUNTS_OFFSET, MILITARY_TILE_COUNT_VALUES);
    assert_eq!(counts.len(), MILITARY_TILE_COUNT_VALUES);
    assert_eq!(counts, military_tile_counts_for_city(&state.city).to_vec());
    assert_eq!(counts[0], 16);
    assert_eq!(counts[1], 2);
    for index in 2..MILITARY_TILE_COUNT_VALUES {
        assert_eq!(counts[index], 1, "military count index {index}");
    }
}

#[test]
fn pe_xund_occupancy_bss_4ca41c_hydrate_and_persist() {
    // PE load @ `0x431a40` / save @ `0x4340b8`: word `[MISC+0xfe8]` ↔ `[0x4ca41c]`.
    let path = "/tmp/sc2000-xund-occ-bss.sc2";
    let mut city = City::new_blank(path, "XundOccBss").expect("blank");
    city.set_misc_u32_at(MISC_SUBWAY_COUNT_OFFSET, 0x1234_002a);
    assert_eq!(city.pe_xund_occupancy, 0x002a);
    city.pe_xund_occupancy = 0x0011;
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_xund_occupancy, 0x0011);
    assert_eq!(loaded.misc_u32_at(MISC_SUBWAY_COUNT_OFFSET), Some(0x0011));
    let _ = std::fs::remove_file(path);
}

#[test]
fn xgrp_history_power_percent_uses_published_bss_without_crt_draws() {
    let mut city = City::new_blank("/tmp/sc2000-xgrp-crt.sc2", "XgrpCrt").expect("blank city");
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &vec![0u32; XGRP_VALUES]);
    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 18);
    city.set_pe_power_used_percent(37);
    let mut state = test_state(city);
    let before = state.pe_process_crt.seed();
    let population = population_stats_for_city(&state.city);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let expected = xgrp_history_latest_values(&state.city, population, outcomes);
    assert_eq!(expected[8], 63, "the graph reads 100 - published BSS 37");
    state.refresh_xgrp_history(population, outcomes);
    assert_eq!(
        state.pe_process_crt.seed(),
        before,
        "UpdateGraphData reads the published percentage and does not recalculate power"
    );
    let updated = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    let power_series_base = 8 * XGRP_SAMPLES_PER_SERIES;
    assert_eq!(updated[power_series_base], expected[8]);
}

#[test]
fn xgrp_latest_values_prefer_ready_p_zone_pops_and_pe_dw() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_pe_zone_pops([15, 8, 4]);
    city.persist_pe_day22_population_leftover(150);
    city.set_pe_dw_arcology_population(40);
    let population = PopulationStats {
        residential_population: 9_999,
        commercial_population: 9_999,
        industrial_population: 9_999,
        arcology_population: 40,
        ..PopulationStats::default()
    };
    let outcomes = CityServiceOutcomes::default();
    let latest = xgrp_history_latest_values(&city, population, outcomes);
    let arco = 40;
    assert_eq!(latest[0], 150 + arco, "GRP_CITYSIZE uses pe_dw + arco");
    assert_eq!(latest[1], 150 + arco / 2);
    assert_eq!(latest[2], 80 + arco / 4);
    assert_eq!(latest[3], 40 + arco / 4);
}

#[test]
fn residential_recreation_cap_counts_original_recreation_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    assert_eq!(
        residential_recreation_population_cap_for_city(&city),
        RESIDENTIAL_RECREATION_BASE_POPULATION_CAP
    );

    stamp_complete_building_for_test(&mut city, 10, 10, SMALL_PARK_BUILDING_ID, 1);
    stamp_complete_building_for_test(&mut city, 12, 10, BIG_PARK_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut city, 16, 10, MARINA_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut city, 20, 10, STADIUM_BUILDING_ID, 4);
    stamp_complete_building_for_test(&mut city, 25, 10, ZOO_BUILDING_ID, 4);

    assert_eq!(
        residential_recreation_population_cap_for_city(&city),
        RESIDENTIAL_RECREATION_BASE_POPULATION_CAP + 1_000 + 3_000 + 9_000 + 16_000 + 16_000
    );
}

#[test]
fn decline_change_building_preserves_xtxt_like_pe_place_tile() {
    // PE ChangeBuilding @ `0x437d40` → ChooseAndPlace → PlaceTile: no DestroyBuilding; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-decline-xtxt.sc2", "DeclineT").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_text_overlay(20, 20, 0x2a);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    city.set_tile_bits(
        20,
        20,
        city.tile_bits_at(20, 20)
            .unwrap_or_default()
            .with_piped(true)
            .with_watered(true),
    );
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = false;

    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
}

#[test]
fn mac68k_decline_change_building_preserves_xtxt_like_pe_place_tile() {
    // Shared ChangeBuilding → PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-decline-xtxt.sc2", "MacDecT").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_text_overlay(20, 20, 0x2a);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    city.set_tile_bits(
        20,
        20,
        city.tile_bits_at(20, 20)
            .unwrap_or_default()
            .with_piped(true)
            .with_watered(true),
    );
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bypass_growth_trip_gate = false;

    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
}

#[test]
fn mac68k_decline_change_building_preserves_xtrf_like_pe_choose_and_place() {
    // Mac68k shares ChangeBuilding → ChooseAndPlace: OR XBIT 0xe0 only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-mac-decline-xtrf.sc2", "MacDecX").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    city.set_tile_bits(
        20,
        20,
        city.tile_bits_at(20, 20)
            .unwrap_or_default()
            .with_piped(true)
            .with_watered(true),
    );
    city.set_traffic_density(20, 20, 0x71);
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bypass_growth_trip_gate = false;

    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x71));
}

#[test]
fn get_populated_tile_vacant_surface_matches_tile_road_lr() {
    assert!(rci_tile_is_vacant_growth_surface(0));
    assert!(rci_tile_is_vacant_growth_surface(1));
    assert!(rci_tile_is_vacant_growth_surface(5));
    assert!(rci_tile_is_vacant_growth_surface(6));
    assert!(rci_tile_is_vacant_growth_surface(SMALL_PARK_BUILDING_ID));
    assert!(rci_tile_is_vacant_growth_surface(14));
    assert!(rci_tile_is_vacant_growth_surface(TILE_ROAD_LR - 1));
    assert!(!rci_tile_is_vacant_growth_surface(TILE_ROAD_LR));
    assert!(!rci_tile_is_vacant_growth_surface(
        RESIDENTIAL_1X1_BUILDINGS[0]
    ));
    assert!(!rci_tile_is_vacant_growth_surface(
        ABANDONED_1X1_BUILDINGS[0]
    ));
}

#[test]
fn abandoned_redevelop_power_gate_uses_fun_00436d00_cross() {
    let mut city = City::new_blank("/tmp/sc2000-abandon-pwr.sc2", "AbnPwr").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, ABANDONED_POP1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 20);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = true;
    let reserved = vec![false; CITY_TILES];
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    set_saved_xbit_growth_power_for_test(&mut power_maps, 20, 19, true);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let services = rci_development_services_for_city(&state.city);
    assert!(
                state
                    .rci_development_candidate_at(
                        20,
                        20,
                        &reserved,
                        &power_maps,
                        outcomes,
                        &services,
                        None,
                    )
                    .is_some(),
                "abandoned redevelop must accept FUN_00436D00 cardinal neighbor power"
            );

    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 19, 19, true);
    assert!(
                state
                    .rci_development_candidate_at(
                        20,
                        20,
                        &reserved,
                        &power_maps,
                        outcomes,
                        &services,
                        None,
                    )
                    .is_none(),
                "diagonal-only power must not satisfy abandoned redevelop FUN_00436D00"
            );
}

#[test]
fn changebuilding_blocks_same_tick_startbuilding() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(24, 20, ZONE_RESIDENTIAL_DENSE | ZONE_CORNER_ALL);
    stamp_transport_access_for_test(&mut city, 19, 20);
    stamp_transport_access_for_test(&mut city, 23, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 24, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    water_footprint_from_below_for_test(&mut city, 24, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    state.sim_calendar_one_growth_tick(0, 0);

    assert!(
        is_abandoned_building(state.city.building_at(20, 20)),
        "PE GROWTH_CHANGE does not fall through to StartBuilding or BUILD_ABANDON"
    );
    assert!(
        is_construction_building(state.city.building_at(24, 20)),
        "same-lattice vacant tiles still StartBuilding, got {:#04x}",
        state.city.building_at(24, 20)
    );
    assert_eq!(
        state.pe_growth_aggregate.slots()[2],
        pe_growth_aggregate_weight(pe_growth_level_for_xbld(RESIDENTIAL_1X1_BUILDINGS[0])),
        "successful GROWTH_CHANGE still contributes the original visited XBLD"
    );
}

#[test]
fn class0_aggregate_credits_zone_slot_on_kind_mismatch() {
    // PE @ `0x436112` uses XZON type nibble, not building kind.
    let mut city = City::new_blank("/tmp/sc2000-class0-mismatch.sc2", "Class0Mis").expect("blank");
    city.set_zone(20, 20, ZONE_COMMERCIAL_DENSE | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let _fail = FailGrowthChangeRolls::arm();
    state.sim_calendar_one_growth_tick(0, 0);
    let weight = pe_growth_aggregate_weight(pe_growth_level_for_xbld(RESIDENTIAL_1X1_BUILDINGS[0]));
    assert_eq!(
        state.pe_growth_aggregate.slots()[4],
        weight,
        "mismatched residential XBLD still credits commercial zone slot 4"
    );
    assert_eq!(state.pe_growth_aggregate.slots()[2], 0);
}

#[test]
fn completeconstruction_blocks_same_tick_startbuilding() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    // Make the completed class-1 tile eligible for the shared corner/level
    // pre-roll gates; PE BUILD_THINK still has no aggregate write.
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE | ZONE_CORNER_ALL);
    city.set_building(20, 20, CONSTRUCTION_POP1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    state.sim_calendar_one_growth_tick(0, 0);

    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "PE GROWTH_COMPLETECONSTRUCTION does not fall through to StartBuilding, got {:#04x}",
        state.city.building_at(20, 20)
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        if (x, y) == (20, 20) {
            continue;
        }
        assert_eq!(
            state.city.building_at(x, y),
            0,
            "same-visit size-up would stamp 2×2 construction at ({x},{y})"
        );
    }
    assert_eq!(
        *state.pe_growth_aggregate.slots(),
        [0; 8],
        "PE class-1 BUILD_THINK @ 0x43615b has no pre-roll aggregate write"
    );
}

#[test]
fn failed_build_think_falls_through_to_startbuilding() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    // Let the class-1 tile pass PE's shared view-corner aggregate gate.
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE | ZONE_CORNER_ALL);
    city.set_building(20, 20, CONSTRUCTION_POP1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    let _fail = FailConstructionCompleteRolls::arm();
    state.sim_calendar_one_growth_tick(0, 0);

    let expected = construction_building_for_pop(2, 20, 20).unwrap();
    assert_eq!(
        state.city.building_at(20, 20),
        expected,
        "PE GROWTH_CONSIDERCONSTRUCTION falls through to StartBuilding size-up"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), expected);
    }
    assert_eq!(
        *state.pe_growth_aggregate.slots(),
        [0; 8],
        "failed class-1 BUILD_THINK has no pre-roll aggregate write before StartBuilding"
    );
}

#[test]
fn failed_build_think_falls_back_to_startbuilding_from_view_corner_only_for_multitile_construction()
{
    for rotation in 0..4u8 {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        stamp_zoned_building_footprint_for_test(
            &mut city,
            (20, 20),
            2,
            ZONE_RESIDENTIAL_DENSE,
            CONSTRUCTION_POP2_BUILDINGS[0],
        );
        city.set_view_rotation(rotation);
        let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2);
        let (non_x, non_y) = building_footprint_from_origin(20, 20, 2)
            .unwrap()
            .into_iter()
            .find(|&(x, y)| (x, y) != (sx, sy))
            .expect("finds a non-corner tile");

        assert!(
            !rci_redevelopment_origin_can_start_at(
                &city,
                non_x,
                non_y,
                ZONE_RESIDENTIAL_DENSE,
                RciKind::Residential
            ),
            "rotation {rotation} should reject non-view-angle corner at ({non_x},{non_y})"
        );
        assert!(
            rci_redevelopment_origin_can_start_at(
                &city,
                sx,
                sy,
                ZONE_RESIDENTIAL_DENSE,
                RciKind::Residential
            ),
            "rotation {rotation} should accept view-angle corner ({sx},{sy})"
        );
    }
}

#[test]
fn abandoned_tiles_do_not_startbuilding_same_visit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    city.set_building(20, 20, ABANDONED_POP1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    state.sim_calendar_one_growth_tick(0, 0);

    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "PE BUILD_ABANDON ChooseAndPlaces at the current pop, got {:#04x}",
        state.city.building_at(20, 20)
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        if (x, y) == (20, 20) {
            continue;
        }
        assert_eq!(
            state.city.building_at(x, y),
            0,
            "CONSIDERABANDON does not fall through to StartBuilding size-up at ({x},{y})"
        );
    }
}

#[test]
fn choose_and_place_residential_pop1_xval_uses_microsim_cell_like_pe_436ea0() {
    // PE ChooseAndPlace @ 0x436ea0: x&=~1, y>>=1 into XVAL microsim — odd tiles share the NW cell.
    let mut city = City::new_blank("/tmp/sc2000-xval-micro.sc2", "XvalMicro").expect("blank");
    let mut xval = vec![0u8; CITY_TILES];
    xval[20 * CITY_WIDTH + 20] = 0; // band 0
    xval[20 * CITY_WIDTH + 21] = 192; // band 2 if read raw
    xval[21 * CITY_WIDTH + 20] = 192;
    xval[21 * CITY_WIDTH + 21] = 192;
    city.sync_land_value_from_slice(&xval);
    let at_even =
        rci_choose_and_place_for_city(&city, ZONE_RESIDENTIAL_DENSE, 1, 20, 20, 0).unwrap();
    let at_odd =
        rci_choose_and_place_for_city(&city, ZONE_RESIDENTIAL_DENSE, 1, 21, 20, 0).unwrap();
    assert_eq!(
        at_even & !3,
        at_odd & !3,
        "odd city tile must use microsim NW XVAL wealth band, not raw odd-tile byte"
    );
    assert!(
        at_odd < 0x74,
        "microsim cell XVAL 0 → wealth band 0 (0x70..0x73), got {at_odd:#04x}"
    );
    assert!(
        !land_value_allows_start_from_pop(&city, 21, 20, ZONE_RESIDENTIAL_DENSE, 2),
        "odd tile must gate StartBuilding on microsim XVAL 0, not raw 192"
    );
}

#[test]
fn pe_startbuilding_pop4_mode3_picker_uses_c2_table_and_one_draw() {
    // Win95 StartBuilding pop-3 → ChooseAndPlace mode 3 uses table index 16:
    // base 0xC2, count 2. Its construction picker takes no XVAL or quality input.
    // The adjacent pop-4 finished-building picker also ignores those inputs.
    let mut low_xval_city =
        City::new_blank("/tmp/sc2000-test.sc2", "low-xval").expect("blank city");
    let mut high_xval_city =
        City::new_blank("/tmp/sc2000-test.sc2", "high-xval").expect("blank city");
    stamp_xval_for_test(&mut low_xval_city, XVAL_POPLEVEL_LOW);
    stamp_xval_for_test(&mut high_xval_city, XVAL_POPLEVEL_HIGH);
    let low_xval_pick =
        rci_choose_and_place_for_city(&low_xval_city, ZONE_RESIDENTIAL_DENSE, 4, 21, 22, 0);
    let high_xval_pick =
        rci_choose_and_place_for_city(&high_xval_city, ZONE_RESIDENTIAL_DENSE, 4, 21, 22, 4);
    assert_eq!(
        low_xval_pick, high_xval_pick,
        "pop-4 finished-building picker ignores both XVAL and quality band"
    );
    assert!(RESIDENTIAL_3X3_BUILDINGS.contains(&low_xval_pick.unwrap()));

    let mut actual_crt = Win95CrtRand::new(0x0043_75C0);
    let mut expected_crt = actual_crt;
    let expected_building =
        construction_building_for_pop_with_rng(4, &mut expected_crt).expect("pop-4 mode 3");
    assert!((0xC2..=0xC3).contains(&expected_building));

    let placement = pe_startbuilding_pop4_direct_placement_with_picker(3, 12, || {
        construction_building_for_pop_with_rng(4, &mut actual_crt)
    });

    assert_eq!(placement, Some((3, 10, expected_building)));
    assert_eq!(
        actual_crt.seed(),
        expected_crt.seed(),
        "one CRT rand()%2 draw"
    );

    let stamp_coordinate_reroll = construction_building_for_pop(4, 3, 10).unwrap();
    assert_ne!(
        expected_building, stamp_coordinate_reroll,
        "fixture distinguishes the selected raw-call tile from a second stamp-time pick"
    );
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let footprint = building_footprint_from_origin(3, 10, 3).unwrap();
    for &(x, y) in &footprint {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (3, 10),
        zone_id: ZONE_RESIDENTIAL_DENSE,
        building: expected_building,
        footprint: footprint.clone(),
        place_as_construction: true,
    };
    let seed_before_stamp = actual_crt.seed();
    state.stamp_zone_development_with_crt(&plan, &mut actual_crt);
    assert_eq!(
        actual_crt.seed(),
        seed_before_stamp,
        "stamping must not pick again"
    );
    for (x, y) in footprint {
        assert_eq!(state.city.building_at(x, y), expected_building);
    }
}

#[test]
fn pe_startbuilding_pop3_mode3_picker_uses_a8_table_and_one_draw() {
    // Win95 current-pop-2 StartBuilding calls ChooseAndPlace(pop=3, mode=3):
    // table index 15, base 0xA8, count 2. Retain that pick through stamping.
    let mut actual_crt = Win95CrtRand::new(3);
    let mut expected_crt = actual_crt;
    let expected_building =
        construction_building_for_pop_with_rng(3, &mut expected_crt).expect("pop-3 mode 3");
    assert!((0xA8..=0xA9).contains(&expected_building));

    let placement = pe_startbuilding_pop3_direct_placement_with_picker(21, 22, || {
        construction_building_for_pop_with_rng(3, &mut actual_crt)
    });
    assert_eq!(placement, Some((21, 21, expected_building)));
    assert_eq!(
        actual_crt.seed(),
        expected_crt.seed(),
        "one CRT rand()%2 draw"
    );

    let mut rejected_crt = Win95CrtRand::new(3);
    let mut expected_rejected_crt = rejected_crt;
    let _ = construction_building_for_pop_with_rng(3, &mut expected_rejected_crt);
    assert_eq!(
        pe_startbuilding_pop3_direct_placement_with_picker(1, 22, || {
            construction_building_for_pop_with_rng(3, &mut rejected_crt)
        }),
        None,
        "PE rejects out-of-bounds pop-3 ChooseAndPlace after the picker draw"
    );
    assert_eq!(
        rejected_crt.seed(),
        expected_rejected_crt.seed(),
        "rejected raw call still consumes the mode-3 picker draw"
    );

    let stamp_coordinate_reroll = construction_building_for_pop(3, 21, 21).unwrap();
    assert_ne!(
        expected_building, stamp_coordinate_reroll,
        "fixture distinguishes the selected raw-call tile from a second stamp-time pick"
    );
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let footprint = building_footprint_from_origin(21, 21, 2).unwrap();
    for &(x, y) in &footprint {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (21, 21),
        zone_id: ZONE_RESIDENTIAL_DENSE,
        building: expected_building,
        footprint: footprint.clone(),
        place_as_construction: true,
    };
    let seed_before_stamp = actual_crt.seed();
    state.stamp_zone_development_with_crt(&plan, &mut actual_crt);
    assert_eq!(
        actual_crt.seed(),
        seed_before_stamp,
        "stamping must not pick again"
    );
    for (x, y) in footprint {
        assert_eq!(state.city.building_at(x, y), expected_building);
    }
}

#[test]
fn choose_and_place_optional_keeps_test_isolation() {
    let mut rng = Win95CrtRand::new(0x0043_6D03);
    let before = rng.seed();
    assert_eq!(
        construction_building_for_pop_optional(1, 20, 20, Some(&mut rng)),
        construction_building_for_pop(1, 20, 20),
    );
    assert_eq!(rng.seed(), before);
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_COMMERCIAL_DENSE);
    stamp_xval_for_test(&mut city, 64);
    assert_eq!(
        rci_choose_and_place_for_city_optional(
            &city,
            ZONE_COMMERCIAL_DENSE,
            2,
            20,
            20,
            0,
            Some(&mut rng),
        ),
        rci_choose_and_place_for_city(&city, ZONE_COMMERCIAL_DENSE, 2, 20, 20, 0),
    );
    assert_eq!(
        rng.seed(),
        before,
        "cfg(test) ChooseAndPlace helpers must keep isolation"
    );
}

#[test]
fn construction_building_for_pop_with_rng_advances_crt() {
    let mut rng = Win95CrtRand::new(0x0043_6D04);
    let before = rng.seed();
    let building = construction_building_for_pop_with_rng(1, &mut rng).expect("pop1");
    assert!(CONSTRUCTION_POP1_BUILDINGS.contains(&building));
    assert_ne!(rng.seed(), before);
}

#[test]
fn stamp_zone_development_with_crt_shares_caller_stream_contract() {
    // PE ChooseAndPlace mode-3 construction CRT is process `0x48cdd8`.
    // Growth-tick must stamp on budget_rng/growth_rng so writeback keeps those draws.
    // cfg(test) isolation still uses construction_building_for_pop; pin the with_rng contract.
    let mut crt = Win95CrtRand::new(0x0043_6e80);
    let before = crt.seed();
    let mut preview = crt;
    let expected = construction_building_for_pop_with_rng(1, &mut preview).expect("pop1");
    let got = construction_building_for_pop_with_rng(1, &mut crt).expect("pop1");
    assert_eq!(got, expected);
    assert_ne!(crt.seed(), before);
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn abandoned_building_for_pop_optional_keeps_test_isolation() {
    let mut rng = Win95CrtRand::new(0x0043_6D05);
    let before = rng.seed();
    assert_eq!(
        abandoned_building_for_pop_optional(1, 20, 20, Some(&mut rng)),
        abandoned_building_for_pop(1, 20, 20),
    );
    assert_eq!(
        rci_change_building_replace_tile_optional(20, 20, Some(&mut rng)),
        rci_change_building_replace_tile(20, 20),
    );
    assert_eq!(rng.seed(), before);
    let footprint = building_footprint_from_origin(10, 10, 2).unwrap();
    let with_crt =
        rci_decline_change_building_optional(0x8C, &footprint, 10, 10, true, Some(&mut rng));
    let isolated = rci_decline_change_building(0x8C, &footprint, 10, 10, true);
    assert_eq!(with_crt, isolated);
    assert_eq!(rng.seed(), before);
}

#[test]
fn abandoned_building_for_pop_with_rng_advances_crt() {
    let mut rng = Win95CrtRand::new(0x0043_6D06);
    let before = rng.seed();
    let building = abandoned_building_for_pop_with_rng(1, &mut rng).expect("pop1");
    assert!(ABANDONED_POP1_BUILDINGS.contains(&building));
    assert_ne!(rng.seed(), before);
}

#[test]
fn industrial_does_not_grow_with_rail_track_only_access() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_INDUSTRIAL_LIGHT);
    city.set_building(
        19,
        20,
        network_building_for_mask(NetworkKind::Rail, NETWORK_LEFT | NETWORK_RIGHT),
    );
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 0, 0, 1_000);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_zones(), 0);
    assert_eq!(
        state.city.building_at(20, 20),
        0,
        "PE Game_IsValidTransitItems does not treat rail tracks as vacant-growth transit"
    );
}

#[test]
fn vacant_startbuilding_power_gate_uses_fun_00436d00_cross() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = true;
    let reserved = vec![false; CITY_TILES];
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    // Cardinal neighbor powered; origin neither powered nor energized.
    set_saved_xbit_growth_power_for_test(&mut power_maps, 20, 19, true);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let services = rci_development_services_for_city(&state.city);
    assert!(
                state
                    .rci_development_candidate_at(
                        20,
                        20,
                        &reserved,
                        &power_maps,
                        outcomes,
                        &services,
                        None,
                    )
                    .is_some(),
                "vacant StartBuilding must accept FUN_00436D00 cardinal neighbor power"
            );

    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 19, 19, true); // diagonal only
    assert!(
                state
                    .rci_development_candidate_at(
                        20,
                        20,
                        &reserved,
                        &power_maps,
                        outcomes,
                        &services,
                        None,
                    )
                    .is_none(),
                "diagonal-only power must not satisfy vacant StartBuilding FUN_00436D00"
            );
}

#[test]
fn sizeup_startbuilding_power_gate_uses_fun_00436d00_cross() {
    let mut city = City::new_blank("/tmp/sc2000-sizeup-power.sc2", "SizeUpPwr").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 21);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = true;
    let reserved = vec![false; CITY_TILES];
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    // Cardinal neighbor of the visited lattice tile; footprint tiles unpowered.
    set_saved_xbit_growth_power_for_test(&mut power_maps, 20, 19, true);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let services = rci_development_services_for_city(&state.city);
    assert!(
        state
            .rci_expansion_redevelopment_candidate_at(
                20,
                20,
                &reserved,
                &power_maps,
                outcomes,
                &services,
                None,
            )
            .is_some(),
        "size-up StartBuilding must accept FUN_00436D00 cardinal neighbor power"
    );

    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 19, 19, true); // diagonal only
    assert!(
        state
            .rci_expansion_redevelopment_candidate_at(
                20,
                20,
                &reserved,
                &power_maps,
                outcomes,
                &services,
                None,
            )
            .is_none(),
        "diagonal-only power must not satisfy size-up StartBuilding FUN_00436D00"
    );
}

#[test]
fn pop3_startbuilding_perimeter_uses_fun_437f90_abandon_not_rubble() {
    // PE `0x437a88` → `0x402180` / `FUN_00437f90`: mode-4 pop1 abandons, not DestroyStructure rubble.
    let mut city = City::new_blank("/tmp/sc2000-pop3-437f90.sc2", "Pop3437f90").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE | ZONE_CORNER_ALL);
    }
    // Edge probe (22,20) is on the E side of the 3×3; plant a large XBLD there.
    city.set_building(22, 20, 0x90);
    city.set_zone(22, 20, ZONE_RESIDENTIAL_DENSE); // no single-corner adjust (case 4)
    let mut state = test_state(city);
    let mut crt = state.pe_process_crt;
    state.destroy_pop3_size_up_xbld_at_least_0x8c_with_crt((20, 20), &mut crt);
    state.pe_process_crt = crt;
    let b = state.city.building_at(22, 20);
    assert!(
        matches!(b, 0x8a | 0x8b),
        "hit tile should be abandon pop1, got {b:#04x}"
    );
    assert!(
        !is_rubble_tile(b),
        "must not CRT-rubble via DestroyStructure"
    );
    // SE-based 2×2 without adjust: (22,20),(23,20),(23,19),(22,19)
    for &(tx, ty) in &[(22, 20), (23, 20), (23, 19), (22, 19)] {
        let tile = state.city.building_at(tx, ty);
        assert!(
            matches!(tile, 0x8a | 0x8b),
            "FUN_00437f90 2x2 abandon at ({tx},{ty}), got {tile:#04x}"
        );
    }
}

#[test]
fn pop3_destroy_building_clears_xtxt_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-destroy-xtxt.sc2", "DestroyXtxt").expect("blank");
    let building = RESIDENTIAL_2X2_BUILDINGS[4];
    assert!(building >= CHECK_TILE_AREA_POP1_MAX_XBLD);
    stamp_complete_building_for_test(&mut city, 20, 20, building, 2);
    city.set_text_overlay(20, 20, 0x55);
    city.set_text_overlay(21, 21, 0x66);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.text_overlay_at(21, 21), Some(0));
}

#[test]
fn successful_build_think_with_rejected_choose_and_place_consumes_scan_tile() {
    // A 2x2 at the west map edge is a valid construction footprint, but the
    // PE ChooseAndPlace raw call still rejects x=0 after making its selection.
    let origin = (0, 20);
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zoned_building_footprint_for_test(
        &mut city,
        origin,
        2,
        ZONE_RESIDENTIAL_DENSE,
        CONSTRUCTION_POP2_BUILDINGS[0],
    );
    let (x, y) = rci_xzon_view_corner_in_footprint_for_test(&city, origin, 2);
    assert_eq!(x, 0);
    let mut state = test_state(city);
    let mut reserved = vec![false; CITY_TILES];
    let mut crt = Win95CrtRand::new(Win95CrtRand::DEFAULT_SEED);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let services = rci_development_services_for_city(&state.city);

    let plan = state.complete_rci_construction_site_at(
        x,
        y,
        outcomes,
        &services,
        false,
        &mut reserved,
        &mut crt,
    );

    assert!(plan.is_none(), "ChooseAndPlace rejects the west-edge call");
    assert!(reserved[y * CITY_WIDTH + x], "PE exits this scan visit");
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        CONSTRUCTION_POP2_BUILDINGS[0]
    );
}

#[test]
fn special_zones_wait_for_power_before_developing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    enable_special_zone_demand_for_test(&mut city);
    let origin = (10, 11);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_AIRPORT);
    stamp_airport_runway_zone_ray_for_test(&mut city, origin);
    stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1);
    water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 2);
    let mut state = test_state(city);

    let started = std::time::Instant::now();
    state.handle_simulation_month_advanced();
    assert_eq!(state.city.building_at(origin.0, origin.1), 0);

    power_adjacent_building_for_test(&mut state.city, origin.0, origin.1 - 1);
    enable_special_zone_demand_for_test(&mut state.city);
    // Airport growth draws `rand() & 3`; seed until the CRT gate admits a plan.
    seed_pe_process_crt_until_special_zone_develops(&mut state);
    assert!(state.develop_special_zones() > 0);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        RUNWAY_BUILDING_ID
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(15),
        "growth-tick must not rebuild power maps on every lattice tile (elapsed {:?})",
        started.elapsed()
    );
}

#[test]
fn special_zone_development_plan_refuses_existing_multitile_containing_origin() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let origin = (20, 20);
    let building = AIRPORT_LARGE_DEVELOPMENT_BUILDINGS[0];
    let size = usize::from(xbld_tile_size(building).max(1));
    stamp_zoned_building_footprint_for_test(&mut city, origin, size, ZONE_AIRPORT, building);
    enable_airport_demand_for_test(&mut city);
    stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1);
    power_adjacent_building_for_test(&mut city, origin.0, origin.1 - 1);
    water_footprint_from_below_for_test(&mut city, origin.0, origin.1, size);
    let state = test_state(city);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);

    for &(x, y) in &[(origin.0 + 1, origin.1), (origin.0, origin.1 + 1), origin] {
        let plan = state.special_zone_development_plan_for_building(
            x,
            y,
            &reserved,
            &power_maps,
            ZONE_AIRPORT,
            building,
        );
        assert!(
            plan.is_none(),
            "existing multi-tile special buildings must not re-plan via containing-origin at ({x},{y}); plan={plan:?}"
        );
    }

    // Vacant even-aligned empty footprint still plans (GetItemPlacementAreaAndFarPosition).
    let mut vacant = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for &(x, y) in &building_footprint_from_origin(22, 22, size).expect("in bounds") {
        vacant.set_zone(x, y, ZONE_AIRPORT);
    }
    enable_airport_demand_for_test(&mut vacant);
    stamp_transport_access_for_test(&mut vacant, 21, 22);
    power_adjacent_building_for_test(&mut vacant, 24, 23);
    water_footprint_from_below_for_test(&mut vacant, 22, 22, size);
    let vacant_state = test_state(vacant);
    let vacant_maps = power_service_maps(&vacant_state.city);
    let plan = vacant_state
        .special_zone_development_plan_for_building(
            23,
            23,
            &reserved,
            &vacant_maps,
            ZONE_AIRPORT,
            building,
        )
        .expect("vacant even-align visit should still plan");
    assert_eq!(plan.origin, (22, 22));
}

#[test]
fn special_zone_large_development_even_aligns_when_scan_visits_off_grid() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // Demand must cover the pre-seeded crane's capacity plus room to grow.
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Industrial,
        INDUSTRIAL_3X3_BUILDINGS[0],
        (80, 104),
        SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION + SEAPORT_CRANE_POPULATION_CAPACITY + 1,
    );
    enable_airport_demand_for_test(&mut city);
    city.set_zone(10, 10, ZONE_SEAPORT);
    city.set_building(10, 10, SEAPORT_CRANE_BUILDING_ID);

    let seaport_origin = (24, 20);
    stamp_transport_access_for_test(&mut city, seaport_origin.0 - 1, seaport_origin.1);
    // The scanner visits the off-grid south-east footprint cell; this powered plant
    // is adjacent to that raw candidate but outside the even-aligned 2×2 footprint.
    power_adjacent_building_for_test(&mut city, seaport_origin.0 + 2, seaport_origin.1 + 1);
    stamp_zone_footprint_for_test(&mut city, seaport_origin, 2, ZONE_SEAPORT);

    let airport_origin = (21, 21);
    stamp_transport_access_for_test(&mut city, airport_origin.0 - 1, airport_origin.1);
    power_adjacent_building_for_test(&mut city, airport_origin.0, airport_origin.1 - 1);
    stamp_airport_runway_zone_ray_for_test(&mut city, airport_origin);
    let mut state = test_state(city);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    let airport_plan = state
        .special_zone_development_plan_for_building(
            airport_origin.0,
            airport_origin.1,
            &reserved,
            &power_maps,
            ZONE_AIRPORT,
            RUNWAY_BUILDING_ID,
        )
        .expect("valid odd-parity runway candidate should plan");
    state.stamp_zone_development(&airport_plan);
    assert_eq!(
        state.city.building_at(airport_origin.0, airport_origin.1),
        RUNWAY_BUILDING_ID
    );

    let seaport_plan = state
        .special_zone_development_plan_for_building(
            seaport_origin.0 + 1,
            seaport_origin.1 + 1,
            &reserved,
            &power_maps,
            ZONE_SEAPORT,
            CIV_LOADING_BAY_BUILDING_ID,
        )
        .expect("off-grid scan coordinate should map to an even-aligned seaport footprint");
    assert_eq!(seaport_plan.origin, seaport_origin);
    state.stamp_zone_development(&seaport_plan);
    assert_eq!(
        state.city.building_at(seaport_origin.0, seaport_origin.1),
        CIV_LOADING_BAY_BUILDING_ID
    );
    for (x, y) in building_footprint_from_origin(seaport_origin.0, seaport_origin.1, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), CIV_LOADING_BAY_BUILDING_ID);
        assert_eq!(
            state.city.zone_at(x, y).map(|zone| zone.zone_id()),
            Some(ZONE_SEAPORT)
        );
    }
}

#[test]
fn special_zone_development_accepts_singles_and_prefers_large_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Industrial,
        INDUSTRIAL_3X3_BUILDINGS[0],
        (80, 104),
        SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION + SEAPORT_CRANE_POPULATION_CAPACITY + 1,
    );
    enable_airport_demand_for_test(&mut city);
    let airport_origin = (20, 20);
    city.set_zone(airport_origin.0, airport_origin.1, ZONE_AIRPORT);
    stamp_transport_access_for_test(&mut city, airport_origin.0, airport_origin.1 - 1);
    power_adjacent_building_for_test(&mut city, airport_origin.0 - 1, airport_origin.1);
    water_footprint_from_below_for_test(&mut city, airport_origin.0, airport_origin.1, 1);

    city.set_zone(10, 10, ZONE_SEAPORT);
    city.set_building(10, 10, SEAPORT_CRANE_BUILDING_ID);
    let large_origin = (40, 30);
    stamp_transport_access_for_test(&mut city, large_origin.0 - 1, large_origin.1);
    power_adjacent_building_for_test(&mut city, large_origin.0, large_origin.1 - 1);
    water_footprint_from_below_for_test(&mut city, large_origin.0, large_origin.1, 2);
    for (x, y) in building_footprint_from_origin(large_origin.0, large_origin.1, 2).unwrap() {
        city.set_zone(x, y, ZONE_SEAPORT);
    }
    let mut state = test_state(city);

    let expected_plans = expected_special_zone_development_plans(&state);
    let expected_developments: usize = expected_plans.iter().map(|plan| plan.footprint.len()).sum();
    assert_eq!(state.develop_special_zones(), expected_developments);

    let large_plan = expected_plans
        .iter()
        .find(|plan| plan.zone_id == ZONE_SEAPORT && plan.footprint.len() == 4)
        .or_else(|| {
            expected_plans
                .iter()
                .find(|plan| plan.zone_id == ZONE_SEAPORT && plan.footprint.len() == 1)
        });
    if let Some(plan) = large_plan {
        for &(x, y) in &plan.footprint {
            assert_eq!(state.city.building_at(x, y), plan.building);
        }
    } else {
        for (x, y) in building_footprint_from_origin(large_origin.0, large_origin.1, 2).unwrap() {
            assert_eq!(state.city.building_at(x, y), 0);
        }
    }

    if let Some(plan) = expected_plans
        .iter()
        .find(|plan| plan.zone_id == ZONE_AIRPORT)
    {
        assert_eq!(
            state.city.building_at(airport_origin.0, airport_origin.1),
            plan.building
        );
    } else {
        assert_eq!(
            state.city.building_at(airport_origin.0, airport_origin.1),
            0
        );
    }
}

#[test]
fn special_zone_building_classification_matches_reference_ids() {
    let airport_buildings = [221, 222, 225, 226, 228, 229, 230, 232, 234, 238, 239, 246];
    let seaport_buildings = [223, 224, 227, 240, 242];
    let nearby_non_special_buildings = [
        231, // F-15b
        233, // subway station
        235, // water tower
        236, // bus station
        237, // rail station
        241, // top secret
        243, // mayor's house
        244, // water treatment
        245, // library
    ];

    for building in airport_buildings {
        assert!(
            special_zone_developed_building(ZONE_AIRPORT, building),
            "building {building} should count as airport development"
        );
        assert!(
            !special_zone_developed_building(ZONE_SEAPORT, building),
            "airport building {building} should not count as seaport development"
        );
    }

    for building in seaport_buildings {
        assert!(
            special_zone_developed_building(ZONE_SEAPORT, building),
            "building {building} should count as seaport development"
        );
        assert!(
            !special_zone_developed_building(ZONE_AIRPORT, building),
            "seaport building {building} should not count as airport development"
        );
    }

    for building in nearby_non_special_buildings {
        assert!(
            !special_zone_developed_building(ZONE_AIRPORT, building),
            "nearby non-special building {building} should not count as airport development"
        );
        assert!(
            !special_zone_developed_building(ZONE_SEAPORT, building),
            "nearby non-special building {building} should not count as seaport development"
        );
    }
}

#[test]
fn zone_development_preserves_existing_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(10, 10, 112);
    city.set_zone(11, 10, ZONE_AIRPORT);
    city.set_building(11, 10, 112);
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();
    state.develop_rci_until_built_for_test();
    state.develop_special_zones();

    assert_eq!(state.city.building_at(10, 10), 112);
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().zone_id(),
        ZONE_RESIDENTIAL_LIGHT
    );
    assert_eq!(state.city.building_at(11, 10), 112);
    assert_eq!(state.city.zone_at(11, 10).unwrap().zone_id(), ZONE_AIRPORT);
}

#[test]
fn loaded_maxis_land_starts_with_developed_buildings_in_matching_zones() {
    let Some(path) = paths::test_mac_city() else {
        return;
    };
    let city = City::load(path).expect("load default city");

    assert_developed_buildings_match_zone_kinds(&city);
}

#[test]
fn dezone_drag_removes_small_buildings_and_preserves_zone_nibbles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    for (x, y) in tile_rectangle((20, 20), (22, 21)) {
        city.set_zone(x, y, 4);
    }
    city.set_building(20, 20, 2);
    city.set_building(21, 20, 112);
    let mut state = test_state(city);
    state.tool = EditTool::DeZone;

    state.begin_zone_drag(22, 21, zone_id_for_tool(state.tool).expect("de-zone id"));
    state.update_zone_drag_to((20, 20));
    let drag = state.zone_drag.expect("drag");
    let placement = state.zone_placement_for_rect(drag.start, drag.current, drag.zone_id);
    assert_eq!(placement.preview, ToolPreview::Zone(0));
    assert_eq!(placement.invalid_tiles, vec![(21, 20)]);

    state.zone_drag = Some(drag);
    assert_eq!(state.finish_zone_drag(), 5);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 4);
    assert_eq!(state.city.zone_at(21, 20).unwrap().raw_bits(), 4);
    assert_eq!(state.city.zone_at(22, 21).unwrap().raw_bits(), 4);
}

#[test]
fn dezone_single_tile_preview_rejects_occupied_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, 4);
    city.set_building(20, 20, 112);
    let mut state = test_state(city);
    state.tool = EditTool::DeZone;

    let placement = state.tool_placement_at(20, 20).expect("de-zone placement");
    assert_eq!(placement.preview, ToolPreview::Zone(0));
    assert!(placement.valid_tiles.is_empty());
    assert_eq!(placement.invalid_tiles, vec![(20, 20)]);
}

#[test]
fn mac68k_bulldoze_clears_xbit_rotate_powered_powerable_like_pe() {
    let mut city = City::new_blank("/tmp/sc2000-bull-xbit-mac.sc2", "BullXbitMac").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0)
            .with_rotate(true)
            .with_watered(true)
            .with_piped(true)
            .with_powered(true)
            .with_powerable(true),
    );
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert_eq!(bits.raw_bits() & !0x3d, 0);
    assert!(bits.watered());
    assert!(!bits.powerable());
}

#[test]
fn bulldoze_clears_xbit_rotate_powered_powerable_like_pe_destroy_building() {
    // PE DestroyBuilding @ `0x45a07f`: XBIT &= 0x3d.
    let mut city = City::new_blank("/tmp/sc2000-bull-xbit.sc2", "BullXbit").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0)
            .with_rotate(true)
            .with_watered(true)
            .with_piped(true)
            .with_powered(true)
            .with_powerable(true),
    );
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert_eq!(bits.raw_bits() & !0x3d, 0);
    assert!(bits.watered());
    assert!(bits.piped());
    assert!(!bits.rotate());
    assert!(!bits.powered());
    assert!(!bits.powerable());
}

#[test]
fn bulldoze_updates_dw_tile_count_via_place_tile() {
    // Bulldoze DestroyBuilding clears XBLD through PlaceTile (census coherent).
    let mut city = City::new_blank("/tmp/sc2000-bull-census.sc2", "BullCensus").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 210, 3);
    assert_eq!(city.pe_dw_tile_count[210], 9);
    let mut state = test_state(city);
    state.bulldoze_at(21, 21);
    assert_eq!(
        state.city.pe_dw_tile_count[210], 0,
        "police footprint cells must leave dwTileCount[210]"
    );
}

#[test]
fn bulldoze_elevated_preserves_xbit_like_pe_no_45a07f() {
    // PE @ `0x45997e` is XZON &= 0x0f (table `0x4ca1f0`), not XBIT. Elevated arm
    // leaves XBIT unchanged (no `0x45a07f` `&= 0x3d`).
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-xbit.sc2", "BullElevXbit").expect("blank");
    city.set_building(20, 20, 0x41);
    city.set_tile_bits(20, 20, TileBits::new(0x40 | 0x80 | 0x10 | 0x02 | 0x04));
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert_eq!(bits.raw_bits() & 0xf0, 0x40 | 0x80 | 0x10);
    assert!(bits.rotate());
    assert!(bits.water_tile());
    assert!(bits.powered() && bits.powerable());
}

#[test]
fn mac68k_bulldoze_elevated_preserves_xbit_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-xbit-mac.sc2", "BullElevXbitMac").expect("blank");
    city.set_building(20, 20, 0x42);
    city.set_tile_bits(20, 20, TileBits::new(0x40 | 0x80 | 0x10 | 0x02 | 0x04));
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert_eq!(bits.raw_bits() & 0xf0, 0x40 | 0x80 | 0x10);
    assert!(bits.rotate());
    assert!(bits.water_tile());
    assert!(bits.powered() && bits.powerable());
}

#[test]
fn bulldoze_elevated_skips_xtxt_clear_like_pe_elevated_arm() {
    // PE elevated arm never reaches XTXT clear @ `0x45a0c4`.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-xtxt.sc2", "BullElevXtxt").expect("blank");
    city.set_building(20, 20, 0x40);
    city.set_text_overlay(20, 20, 0x20);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x20));
}

#[test]
fn bulldoze_elevated_skips_nonflat_fun_00447020_like_pe_epilogue() {
    // PE elevated arm jumps to epilogue before `0x45a1ee` size-1 nonflat FUN.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-nofun.sc2", "BullElevNoFun").expect("blank");
    city.set_building(20, 20, 0x3f);
    city.set_terrain(20, 20, 3);
    city.set_underground(20, 20, 0x10);
    city.set_altitude(20, 20, 9, 0);
    city.set_altitude(21, 20, 10, 0);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.underground_at(20, 20), 0x10);
    assert_eq!(state.city.terrain_at(20, 20), 3);
}

#[test]
fn mac68k_bulldoze_elevated_skips_nonflat_fun_00447020_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-nofun-mac.sc2", "BullElevNoFunMac").expect("blank");
    city.set_building(20, 20, 0x41);
    city.set_terrain(20, 20, 2);
    city.set_underground(20, 20, 0x18);
    city.set_altitude(20, 20, 7, 0);
    city.set_altitude(21, 20, 8, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.underground_at(20, 20), 0x18);
    assert_eq!(state.city.terrain_at(20, 20), 2);
}

#[test]
fn bulldoze_elevated_clears_xzon_corners_like_pe_45997e() {
    // PE elevated arm @ `0x45997e`: XZON &= 0x0f (table `0x4ca1f0`).
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-xzon.sc2", "BullElevXzon").expect("blank");
    city.set_building(20, 20, 0x40);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80 | 0x40);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT)
    );
}

#[test]
fn pe_destroy_building_size1_nonflat_skips_when_size_not_1_like_45a1f4() {
    // PE DestroyBuilding @ `0x45a1ee`: size ≠ 1 → skip FUN_00447020 (`jne 0x45a839`).
    let mut city =
        City::new_blank("/tmp/sc2000-size2-skip-fun.sc2", "Size2SkipFun").expect("blank");
    city.set_building(20, 20, 0x61);
    city.set_terrain(20, 20, 1);
    city.set_underground(20, 20, 0x14);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    assert_eq!(pe_destroy_building_size(0x61), 2);
    pe_destroy_building_size1_nonflat_fun_00447020_45a1ee(&mut city, 20, 20, 0x61, 9);
    assert_eq!(city.underground_at(20, 20), 0x14);
    assert_eq!(city.building_at(20, 20), 0x61);
}

#[test]
fn mac68k_pe_destroy_building_size1_nonflat_skips_when_size_not_1_like_45a1f4() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-size2-skip-fun.sc2", "MacSize2SkipFun").expect("blank");
    city.set_building(20, 20, 0x62);
    city.set_terrain(20, 20, 2);
    city.set_underground(20, 20, 0x13);
    pe_destroy_building_size1_nonflat_fun_00447020_45a1ee(&mut city, 20, 20, 0x62, 9);
    assert_eq!(city.underground_at(20, 20), 0x13);
    assert_eq!(city.building_at(20, 20), 0x62);
}

#[test]
fn pe_destroy_building_size1_runs_fun_when_xbld_0x6f_like_45a1fa() {
    // PE @ `0x45a1fa`: XBLD < 0x70 with size 1 and XTER ≠ 0 → FUN_00447020 clears XUND.
    let mut city = City::new_blank("/tmp/sc2000-size1-6f.sc2", "Size16f").expect("blank");
    city.set_building(20, 20, 0x6f);
    city.set_terrain(20, 20, 1);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_underground(20, 20, 0x12);
    assert_eq!(pe_destroy_building_size(0x6f), 1);
    let mut crt = Win95CrtRand::new(1);
    pe_flood_destroy_tile_with_crt(&mut city, &mut crt, 20, 20);
    assert_eq!(city.underground_at(20, 20), 0);
    assert_ne!(city.building_at(20, 20), 0x6f);
}

#[test]
fn mac68k_pe_destroy_building_size1_runs_fun_when_xbld_0x6f_like_45a1fa() {
    let mut city = City::new_blank("/tmp/sc2000-mac-size1-6f.sc2", "MacSize16f").expect("blank");
    city.set_building(20, 20, 0x6f);
    city.set_terrain(20, 20, 2);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_underground(20, 20, 0x11);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let mut crt = Win95CrtRand::new(2);
    pe_flood_destroy_tile_with_crt(&mut state.city, &mut crt, 20, 20);
    assert_eq!(state.city.underground_at(20, 20), 0);
}

#[test]
fn pe_place_tile_441f00_refuses_oob_like_441f08() {
    // PE PlaceTile @ `0x441f08`/`0x441f12`: x/y outside `0..0x80` return without write.
    let mut city = City::new_blank("/tmp/sc2000-placetile-oob.sc2", "PlaceOob").expect("blank");
    city.set_building(20, 20, 0x0d);
    pe_place_tile_441f00(&mut city, 0x80, 20, 0);
    pe_place_tile_441f00(&mut city, 20, 0x80, 0);
    assert_eq!(city.building_at(20, 20), 0x0d);
    pe_place_tile_441f00(&mut city, 20, 20, 0);
    assert_eq!(city.building_at(20, 20), 0);
}

#[test]
fn place_xund_occupancy_dec_on_clear() {
    let mut city = City::new_blank("/tmp/sc2000-xund-occ-dec.sc2", "XundOccDec").expect("blank");
    pe_place_xund_442370(&mut city, 20, 20, 1);
    assert_eq!(city.pe_xund_occupancy, 1);
    pe_place_xund_442370(&mut city, 20, 20, 0);
    assert_eq!(city.underground_at(20, 20), 0);
    assert_eq!(city.pe_xund_occupancy, 0);
}

#[test]
fn place_xund_occupancy_skips_pipes_0x10() {
    // Pipes `0x10..0x1e` are outside the PlaceXUND occupancy set.
    let mut city = City::new_blank("/tmp/sc2000-xund-occ-pipe.sc2", "XundOccPipe").expect("blank");
    pe_place_xund_442370(&mut city, 20, 20, 0x10);
    assert_eq!(city.underground_at(20, 20), 0x10);
    assert_eq!(city.pe_xund_occupancy, 0);
}

#[test]
fn place_xund_occupancy_counts_0x23() {
    let mut city = City::new_blank("/tmp/sc2000-xund-occ-23.sc2", "XundOcc23").expect("blank");
    pe_place_xund_442370(&mut city, 20, 20, 0x23);
    assert_eq!(city.underground_at(20, 20), 0x23);
    assert_eq!(city.pe_xund_occupancy, 1);
}

#[test]
fn place_xund_occupancy_military_type7_skips() {
    // PE @ `0x44238c`/`0x442407`: XZON type 7 writes XUND but skips `0x4ca41c`.
    let mut city = City::new_blank("/tmp/sc2000-xund-occ-mil.sc2", "XundOccMil").expect("blank");
    city.set_zone(20, 20, 7);
    city.pe_xund_occupancy = 5;
    pe_place_xund_442370(&mut city, 20, 20, 1);
    assert_eq!(city.underground_at(20, 20), 1);
    assert_eq!(city.pe_xund_occupancy, 5);
    pe_place_xund_442370(&mut city, 20, 20, 0);
    assert_eq!(city.underground_at(20, 20), 0);
    assert_eq!(city.pe_xund_occupancy, 5);
}

#[test]
fn pe_xund_occupancy_on_mac_layout_isolated_from_mac_counter_and_survives_reload() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-pe-xund-cross-layout-{}.city",
        std::process::id()
    ));
    let path_string = path.to_string_lossy().into_owned();
    let mut city = City::new_blank(&path_string, "XundCrossLayout").expect("blank Mac city");
    city.kind = CityFileKind::MacCity;
    assert_eq!(city.kind, CityFileKind::MacCity);
    city.set_mac_state_misc_u32_at(0x0fe8, 5);
    city.set_zone(20, 20, 7);
    city.prepare_pe_fidelity_from_saved_city();
    assert_eq!(city.pe_xund_occupancy, 5);

    // PE skips its occupancy update for military-zone cells, while Mac
    // export 106 counts the same XUND class regardless of XZON.
    pe_place_xund_442370(&mut city, 20, 20, 1);
    assert_eq!(city.pe_xund_occupancy, 5);
    assert_eq!(city.mac_state_misc_u32_at(0x0fe8), Some(6));

    city.save(&path)
        .expect("save with separate PE extension state");
    let mut loaded = City::load(&path).expect("reload Mac city");
    assert_eq!(loaded.mac_state_misc_u32_at(0x0fe8), Some(6));
    loaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded.pe_xund_occupancy, 5);

    pe_place_xund_442370(&mut loaded, 20, 20, 0);
    assert_eq!(loaded.pe_xund_occupancy, 5);
    assert_eq!(loaded.mac_state_misc_u32_at(0x0fe8), Some(5));
    loaded.save(&path).expect("save after PE XUND removal");
    let mut reloaded = City::load(&path).expect("reload after PE XUND removal");
    assert_eq!(reloaded.mac_state_misc_u32_at(0x0fe8), Some(5));
    reloaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reloaded.pe_xund_occupancy, 5);
    let _ = std::fs::remove_file(path);
}

#[test]
fn place_xund_occupancy_replace_same_band_net_zero() {
    let mut city = City::new_blank("/tmp/sc2000-xund-occ-net0.sc2", "XundOccNet0").expect("blank");
    pe_place_xund_442370(&mut city, 20, 20, 2);
    assert_eq!(city.pe_xund_occupancy, 1);
    pe_place_xund_442370(&mut city, 20, 20, 3);
    assert_eq!(city.underground_at(20, 20), 3);
    assert_eq!(city.pe_xund_occupancy, 1);
}

#[test]
fn place_tile_dw_tile_count_inc() {
    // PE PlaceTile @ `0x441f66`/`0x441f73`: non-military incs `0x4ca4c8[new]`.
    let mut city = City::new_blank("/tmp/sc2000-dw-tile-inc.sc2", "DwTileInc").expect("blank");
    assert_eq!(city.pe_dw_tile_count[0], 0x4000);
    assert_eq!(city.pe_dw_tile_count[0x0d], 0);
    pe_place_tile_441f00(&mut city, 20, 20, 0x0d);
    assert_eq!(city.building_at(20, 20), 0x0d);
    assert_eq!(city.pe_dw_tile_count[0], 0x3fff);
    assert_eq!(city.pe_dw_tile_count[0x0d], 1);
}

#[test]
fn place_tile_dw_tile_count_dec_on_clear() {
    let mut city = City::new_blank("/tmp/sc2000-dw-tile-dec.sc2", "DwTileDec").expect("blank");
    pe_place_tile_441f00(&mut city, 20, 20, 0x0d);
    pe_place_tile_441f00(&mut city, 20, 20, 0);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.pe_dw_tile_count[0x0d], 0);
    assert_eq!(city.pe_dw_tile_count[0], 0x4000);
}

#[test]
fn place_tile_dw_tile_count_replace_net_zero() {
    let mut city = City::new_blank("/tmp/sc2000-dw-tile-net0.sc2", "DwTileNet0").expect("blank");
    pe_place_tile_441f00(&mut city, 20, 20, 0x0d);
    pe_place_tile_441f00(&mut city, 20, 20, 0x0e);
    assert_eq!(city.building_at(20, 20), 0x0e);
    assert_eq!(city.pe_dw_tile_count[0x0d], 0);
    assert_eq!(city.pe_dw_tile_count[0x0e], 1);
}

#[test]
fn place_tile_dw_tile_count_military_type7_skips() {
    // PE @ `0x441f4c`/`0x441f50`: XZON type 7 skips `0x4ca4c8` but still writes XBLD.
    let mut city = City::new_blank("/tmp/sc2000-dw-tile-mil.sc2", "DwTileMil").expect("blank");
    city.set_zone(20, 20, 7);
    city.pe_dw_tile_count[0x0d] = 9;
    // Vacant military tiles already live in word 0 (base placer @ `0x41450d`).
    city.pe_military_tile_count[0] = 3;
    pe_place_tile_441f00(&mut city, 20, 20, 0x0d);
    assert_eq!(city.building_at(20, 20), 0x0d);
    assert_eq!(city.pe_dw_tile_count[0x0d], 9);
    assert_eq!(city.pe_dw_tile_count[0], 0x4000);
    // Non-special XBLD stays on catch-all word 0 (net zero).
    assert_eq!(city.pe_military_tile_count[0], 3);
}

#[test]
fn military_tile_counts_misc_dumps_pe_military_bss() {
    let mut city = City::new_blank("/tmp/sc2000-mil-misc.sc2", "MilMisc").expect("blank");
    city.pe_military_tile_count[1] = 4;
    city.pe_military_tile_count[3] = 2;
    city.pe_military_tile_count[0] = 7;
    let counts = military_tile_counts_for_city(&city);
    assert_eq!(counts[0], 7);
    assert_eq!(counts[1], 4);
    assert_eq!(counts[3], 2);
    assert_eq!(counts[2], 0);
}

#[test]
fn pe_city_value_infra_subtotal_seeds_neg_xund_occ_like_46a275() {
    // PE @ `0x46a275`: `ecx = sx(0x4ca41c) * [0x4dc2b8]` with `[0x4dc2b8] == -1`.
    let mut city = City::new_blank("/tmp/sc2000-cityval-xund.sc2", "CityValXund").expect("blank");
    assert_eq!(pe_city_value_infra_subtotal_46a270(&city), 0);
    city.pe_xund_occupancy = 3;
    assert_eq!(pe_city_value_infra_subtotal_46a270(&city), -3);
}

#[test]
fn pe_city_value_infra_subtotal_bands_like_46a286() {
    // PE band factors: power*2, road*10, rail*25, tunnel*15, bridge*100, hwy*100, subt*250.
    let mut city = City::new_blank("/tmp/sc2000-cityval-bands.sc2", "CityValBands").expect("blank");
    city.pe_dw_tile_count[0x0e] = 1; // power
    city.pe_dw_tile_count[0x1d] = 2; // road
    city.pe_dw_tile_count[0x2c] = 1; // rail
    city.pe_dw_tile_count[0x3f] = 1; // tunnel band
    city.pe_dw_tile_count[0x51] = 1; // bridge band
    city.pe_dw_tile_count[0x61] = 1; // highway
    city.pe_dw_tile_count[0x6c] = 1; // subtorail
    assert_eq!(
        pe_city_value_infra_subtotal_46a270(&city),
        2 + 20 + 25 + 15 + 100 + 100 + 250
    );
}

#[test]
fn pe_city_value_infra_subtotal_ignores_buildings_above_0x6f() {
    let mut city = City::new_blank("/tmp/sc2000-cityval-bld.sc2", "CityValBld").expect("blank");
    city.pe_dw_tile_count[0x70] = 50;
    city.pe_dw_tile_count[0xcb] = 9;
    assert_eq!(pe_city_value_infra_subtotal_46a270(&city), 0);
}

#[test]
fn pe_city_value_building_skips_statue_0xd0_like_46a6dc() {
    let mut city =
        City::new_blank("/tmp/sc2000-cityval-statue.sc2", "CityValStatue").expect("blank");
    city.pe_dw_tile_count[0xd0] = 9;
    assert_eq!(pe_city_value_building_contrib_46a34a(&city, 0xd0), 0);
}

#[test]
fn pe_city_value_building_hydro_case0_like_46a36f() {
    // PE case 0 @ `0x46a36f`: count * `[0x4dbec0 + tile*4]`; hydro1 `0xc6` cost 4000.
    let mut city = City::new_blank("/tmp/sc2000-cityval-hydro.sc2", "CityValHydro").expect("blank");
    city.pe_dw_tile_count[0xc6] = 2;
    assert_eq!(pe_city_value_building_contrib_46a34a(&city, 0xc6), 8000);
}

#[test]
fn pe_city_value_building_nuclear_case1_div16_like_46a385() {
    // PE case 1 @ `0x46a385`: (count sar-div 16) * cost[tile-1]; `0xcb` → cost `0xca`=15000.
    let mut city = City::new_blank("/tmp/sc2000-cityval-nuke.sc2", "CityValNuke").expect("blank");
    city.pe_dw_tile_count[0xcb] = 16;
    assert_eq!(pe_city_value_building_contrib_46a34a(&city, 0xcb), 15000);
}

#[test]
fn sc2kfix_pe_city_value_uses_placement_costs_and_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-cityval-fix.sc2", "CityValFix").expect("blank");
    city.pe_xund_occupancy = 2; // subway $100 per XUND tile
    city.pe_dw_tile_count[0x1d] = 3; // road $10 per tile
    city.pe_dw_tile_count[0xc6] = 1; // hydro tool cost $400
    city.pe_dw_tile_count[0xd1] = 9; // hospital 3x3 footprint: $500
    city.pe_dw_tile_count[0xf4] = 4; // water treatment 2x2 footprint: $500
    city.pe_dw_tile_count[0xdd] = 2; // airport runway tiles: $250 each
    city.pe_dw_tile_count[0xdb] = 16; // statue is excluded
    city.pe_dw_tile_count[0xd0] = 16; // City Hall has no valuation arm

    assert_eq!(
        sc2kfix_pe_city_value_total(&city),
        2 * 100 + 3 * 10 + 400 + 500 + 500 + 2 * 250
    );
}

#[test]
fn sc2kfix_pe_city_value_tool_costs_match_installed_pe_table() {
    // Hash-matched Win95 SIMCITY.EXE stores `costFromSubTool` at 0x4DC140
    // as an 18×12 dword matrix in read-only .rdata. Indices below are
    // group*12+subtool from SC2KFix's CITY_MENUTOOL_POS mapping.
    let expected_tool_costs = [
        (38, EditTool::PowerPlantCoal, 4_000),
        (39, EditTool::PowerPlantHydro, 400),
        (40, EditTool::PowerPlantOil, 6_600),
        (41, EditTool::PowerPlantGas, 2_000),
        (42, EditTool::PowerPlantNuclear, 15_000),
        (43, EditTool::PowerPlantWind, 100),
        (44, EditTool::PowerPlantSolar, 1_300),
        (45, EditTool::PowerPlantMicrowave, 28_000),
        (46, EditTool::PowerPlantFusion, 40_000),
        (49, EditTool::WaterPump, 100),
        (50, EditTool::WaterTower, 250),
        (51, EditTool::WaterTreatment, 500),
        (52, EditTool::Desalinization, 1_000),
        (65, EditTool::ArcologyPlymouth, 100_000),
        (66, EditTool::ArcologyForest, 120_000),
        (67, EditTool::ArcologyDarco, 150_000),
        (68, EditTool::ArcologyLaunch, 200_000),
        (76, EditTool::BusDepot, 250),
        (86, EditTool::RailDepot, 500),
        (87, EditTool::SubStation, 250),
        (88, EditTool::SubToRail, 250),
        (144, EditTool::School, 250),
        (145, EditTool::College, 1_000),
        (146, EditTool::Library, 500),
        (147, EditTool::Museum, 1_000),
        (156, EditTool::Police, 500),
        (157, EditTool::FireStation, 500),
        (158, EditTool::Hospital, 500),
        (159, EditTool::Prison, 3_000),
        (169, EditTool::BigPark, 150),
        (170, EditTool::Zoo, 3_000),
        (171, EditTool::Stadium, 5_000),
        (172, EditTool::Marina, 1_000),
        (97, EditTool::Airport, 250),
        (96, EditTool::Seaport, 150),
    ];
    for (index, tool, expected) in expected_tool_costs {
        assert_eq!(
            tool_build_cost(tool),
            Some(expected),
            "PE costFromSubTool[{index}] for {tool:?}"
        );
    }

    // The hook uses these same table positions for network layers and subway
    // XUND occupancy; Rust represents them with typed network kinds.
    assert_eq!(NetworkKind::Power.cost_per_tile(), Some(2)); // index 36
    assert_eq!(NetworkKind::Road.cost_per_tile(), Some(10)); // index 72
    assert_eq!(NetworkKind::Rail.cost_per_tile(), Some(25)); // index 84
    assert_eq!(UndergroundKind::Subway.cost_per_tile(), 100); // index 85
    assert_eq!(HIGHWAY_COST_PER_SEGMENT, 100); // roads group, subtool 1: index 73
}

#[test]
fn pe_power_used_percent_bss_4caa50_hydrate_and_readers() {
    let path = "/tmp/sc2000-power-used-bss.sc2";
    let mut city = City::new_blank(path, "PwrUsed").expect("blank");
    city.set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 77);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_power_used_percent(), 77);
    let mut city = City::new_blank("/tmp/sc2000-power-used-bss2.sc2", "PwrUsed2").expect("blank");
    city.set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 1);
    city.set_pe_power_used_percent(88);
    assert_eq!(city.pe_power_used_percent(), 88);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rebuild_dw_tile_count_434f80_skips_military_type7() {
    let mut city =
        City::new_blank("/tmp/sc2000-dw-rebuild-mil.sc2", "DwRebuildMil").expect("blank");
    city.set_zone(20, 20, 7);
    city.set_building(20, 20, 0x0d);
    city.pe_rebuild_dw_tile_count_434f80();
    assert_eq!(city.pe_dw_tile_count[0x0d], 0);
    assert_eq!(city.pe_dw_tile_count[0], CITY_TILES as u16 - 1);
}

#[test]
fn pe_destroy_building_size1_skips_fun_when_xbld_ge_0x70_like_45a1fa() {
    // PE DestroyBuilding @ `0x45a1fa`: saved XBLD >= 0x70 → skip FUN_00447020 even if size==1.
    // XBLD 0x70 is table size 1 (`multi_tile` `_ => 1`) but still gated by the 0x70 compare.
    let mut city = City::new_blank("/tmp/sc2000-size1-ge70.sc2", "Size1Ge70").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_terrain(20, 20, 1);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_underground(20, 20, 0x12);
    assert_eq!(pe_destroy_building_size(0x70), 1);
    let mut crt = Win95CrtRand::new(1);
    pe_flood_destroy_tile_with_crt(&mut city, &mut crt, 20, 20);
    // If FUN ran it would clear XUND; gate skips → XUND remains.
    assert_eq!(city.underground_at(20, 20), 0x12);
    assert_ne!(city.building_at(20, 20), 0x70);
}

#[test]
fn mac68k_pe_destroy_building_size1_skips_fun_when_xbld_ge_0x70_like_45a1fa() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-size1-ge70.sc2", "MacSize1Ge70").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_terrain(20, 20, 2);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_underground(20, 20, 0x11);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let mut crt = Win95CrtRand::new(2);
    pe_flood_destroy_tile_with_crt(&mut state.city, &mut crt, 20, 20);
    assert_eq!(state.city.underground_at(20, 20), 0x11);
}

#[test]
fn pe_fun_00447020_clears_xund_even_on_radioactivity_like_4470c4() {
    // PE FUN_00447020 @ `0x4470b4`–`0x4470c4`: XUND ≠ 0 → PlaceXUND(0) before sea arms.
    let mut city = City::new_blank("/tmp/sc2000-fun-rad-xund.sc2", "FunRadXund").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    city.set_underground(20, 20, 0x12);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 9);
    assert_eq!(city.underground_at(20, 20), 0);
    assert_eq!(city.building_at(20, 20), 5);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn bulldoze_fun_sea_uses_saved_selected_word_like_4ca818() {
    // DestroyBuilding reads the saved selected WORD at `0x4ca818`.
    let mut city = City::new_blank("/tmp/sc2000-bull-sea-map.sc2", "BullSeaMap").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_terrain(20, 20, 1);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    // Unrelated high ALTM water must not replace the selected WORD.
    city.set_altitude(50, 50, 25, 20);
    city.set_mac_selected_water_height(9);
    let mut state = test_state(city);
    assert!(state.edit_map_sea_level() < 9 || state.edit_map_sea_level() == BLANK_MAP_BASE_HEIGHT);
    let pe_sea = pe_selected_water_height_for_fun_00447020(&state.city);
    assert_eq!(pe_sea, 9);
    state.bulldoze_at(20, 20);
    let slope = pe_fun_00447020_slope_xter(&state.city, 20, 20);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x32);
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(20, 20), pe_sea as u8);
}

#[test]
fn bulldoze_lowclass_selector2_postcleanup_processes_southwest_footprint() {
    // This fixture takes the traced <0x0d route. The post-cleanup helper runs
    // on the snapped 2x2 after its PlaceTile cleanup; this tests the shared
    // Rust bulldoze adapter, not a separately rooted Win95 UI caller.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-hwy-south.sc2", "BullHwySouth").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    for x in 19..=22 {
        for y in 19..=22 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(19, 21, 10, 0);
    city.set_altitude(22, 21, 10, 0);
    for x in 20..=21 {
        city.set_terrain(x, 21, 1);
        city.set_underground(x, 21, 0x10);
    }
    assert!(pe_destroy_building_selector2_low_class_cleanup_order(&city, 0x61, 20, 20).is_some());
    let slope_20 = pe_fun_00447020_slope_xter(&city, 20, 21);
    let slope_21 = pe_fun_00447020_slope_xter(&city, 21, 21);
    assert_ne!(slope_20, 0);
    assert_ne!(slope_21, 0);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.terrain_at(20, 21), slope_20);
    assert_eq!(state.city.terrain_at(21, 21), slope_21);
    assert_eq!(state.city.underground_at(20, 21), 0);
    assert_eq!(state.city.underground_at(21, 21), 0);
}

#[test]
fn pe_destroy_building_size2_snap_hit_aligns_sw_like_pe_45b1c0() {
    // PE @ `0x45b1ef`: highway size-2 → x&=~1, y=(y&~1)+1; other XBLD unchanged.
    assert_eq!(
        pe_destroy_building_size2_snap_hit_45b1c0(0x61, 21, 20),
        (20, 21)
    );
    assert_eq!(
        pe_destroy_building_size2_snap_hit_45b1c0(0x49, 20, 21),
        (20, 21)
    );
    assert_eq!(
        pe_destroy_building_size2_snap_hit_45b1c0(0x0d, 21, 20),
        (21, 20)
    );
    // Table-sized 2×2 (e.g. RCI) must not take the highway snap.
    assert_eq!(
        pe_destroy_building_size2_snap_hit_45b1c0(141, 21, 20),
        (21, 20)
    );
}

#[test]
fn pe_destroy_building_footprint_uses_highway_snap_like_pe_45b1c0() {
    // Hit SE of a highway 2×2 snaps to SW; footprint still returns the full block.
    let mut city = City::new_blank("/tmp/sc2000-foot-snap.sc2", "FootSnap").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    let tiles = pe_destroy_building_footprint(&city, 21, 20);
    assert_eq!(tiles.len(), 4);
    for &(tx, ty) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        assert!(tiles.contains(&(tx, ty)), "missing {tx},{ty}");
    }
}

#[test]
fn pe_destroy_building_footprint_incomplete_highway_keeps_hit() {
    // Snap runs, but without a full same-XBLD 2×2 the fallback is the original hit.
    let mut city = City::new_blank("/tmp/sc2000-foot-incomp.sc2", "FootIncomp").expect("blank");
    city.set_building(21, 20, 0x61);
    let tiles = pe_destroy_building_footprint(&city, 21, 20);
    assert_eq!(tiles, vec![(21, 20)]);
}

#[test]
fn bulldoze_lowclass_selector2_postcleanup_runs_in_mac_fidelity_mode() {
    // Shared Rust map-tool path only; EngineFidelity::Mac68k here is not
    // evidence for the original Mac 68k demolition routine.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-hwy-south-mac.sc2", "BullHwySouthMac").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x4b, 2);
    for x in 19..=22 {
        for y in 19..=22 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(19, 21, 10, 0);
    city.set_altitude(22, 21, 10, 0);
    for x in 20..=21 {
        city.set_terrain(x, 21, 2);
        city.set_underground(x, 21, 0x11);
    }
    assert!(pe_destroy_building_selector2_low_class_cleanup_order(&city, 0x4b, 21, 21).is_some());
    let slope_20 = pe_fun_00447020_slope_xter(&city, 20, 21);
    let slope_21 = pe_fun_00447020_slope_xter(&city, 21, 21);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(21, 21);
    assert_eq!(state.city.terrain_at(20, 21), slope_20);
    assert_eq!(state.city.terrain_at(21, 21), slope_21);
    assert_eq!(state.city.underground_at(20, 21), 0);
}

#[test]
fn bulldoze_xbld_lt_6_skips_crt_rubble_restamp_like_pe_458d8a() {
    // PE DestroyBuilding @ `0x458d8a`: XBLD < 6 early-out — no CRT PlaceTile restamp.
    // Rust still clears rubble for free; CRT seed must not advance.
    let mut city = City::new_blank("/tmp/sc2000-bull-lt6.sc2", "BullLt6").expect("blank");
    city.set_building(20, 20, 5);
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    let seed_before = state.pe_process_crt_seed();
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.pe_process_crt_seed(), seed_before);
}

#[test]
fn mac68k_bulldoze_xbld_lt_6_skips_crt_rubble_restamp_like_pe() {
    let mut city = City::new_blank("/tmp/sc2000-bull-lt6-mac.sc2", "BullLt6Mac").expect("blank");
    city.set_building(20, 20, 4);
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let seed_before = state.pe_process_crt_seed();
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.pe_process_crt_seed(), seed_before);
}

#[test]
fn bulldoze_xbld_6_places_crt_rubble_like_pe_458d8a() {
    // Direct core fixture: player Bulldozer uses PlaceTile(0) for XBLD 6..=12.
    let mut city = City::new_blank("/tmp/sc2000-bull-xbld6.sc2", "BullXbld6").expect("blank");
    city.set_building(20, 20, 6); // tree — minimum DestroyBuilding body entry
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    for _ in 0..pe_destroy_building_mode1_scatter_draw_count_for_hit(&state.city, 6, 20, 20) {
        preview.random_u16();
    }
    let expected = preview.rubble_tile_id();
    pe_destroy_building_mode1_optional_crt(
        &mut state.city,
        Some(&mut state.pe_process_crt),
        20,
        20,
    );
    assert_eq!(state.city.building_at(20, 20), expected);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn mac68k_bulldoze_xbld_6_uses_native_placetile_zero() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xbld6-mac.sc2", "BullXbld6Mac").expect("blank");
    city.set_building(20, 20, 6);
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_process_rng = MacToolboxRng::new(1);
    let mut expected_rng = state.mac_process_rng;
    assert_ne!(expected_rng.random_u16() % 20, 0);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.mac_process_rng.seed(), expected_rng.seed());
}

#[test]
fn bulldoze_elevated_skips_rubble_placetile_like_pe() {
    // Elevated arm PlaceTile(0) @ `0x45994e` — not the XTER rubble arm.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-norubble.sc2", "BullElevNoRubble").expect("blank");
    city.set_building(20, 20, 0x41);
    city.set_underground(20, 20, 0x10); // PlaceTile(0) should not restamp rubble
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    let seed_before = state.pe_process_crt_seed();
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.pe_process_crt_seed(), seed_before);
}

#[test]
fn mac68k_bulldoze_elevated_placetile0_skips_rubble_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-norubble-mac.sc2", "BullElevMac").expect("blank");
    city.set_building(20, 20, 0x3f);
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let seed_before = state.pe_process_crt_seed();
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.pe_process_crt_seed(), seed_before);
}

#[test]
fn pe_destroy_structure_elevated_early_outs_keeps_road() {
    // PE DestroyStructure @ `0x46137b`: XBLD `0x3f..=0x42` returns before DestroyBuilding.
    let mut city = City::new_blank("/tmp/sc2000-flood-elev.sc2", "FloodElev").expect("blank");
    city.set_building(20, 20, 0x42);
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    let seed_before = state.pe_process_crt_seed();
    assert!(!state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0x42);
    assert_eq!(state.pe_process_crt_seed(), seed_before);
}

#[test]
fn mac68k_pe_destroy_structure_elevated_early_outs_keeps_road() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-elev-mac.sc2", "FloodElevMac").expect("blank");
    city.set_building(20, 20, 0x40);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let seed_before = state.pe_process_crt_seed();
    assert!(!state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0x40);
    assert_eq!(state.pe_process_crt_seed(), seed_before);
}

#[test]
fn bulldoze_clears_sign_xlab_like_pe_destroy_building_401dca() {
    // PE DestroyBuilding @ `0x45a0c4`/`0x45a101`: XTXT `< 0xc9` → clear + `0x401dca`.
    let mut city = City::new_blank("/tmp/sc2000-bull-xlab.sc2", "BullXlab").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, MIN_USER_TEXT_ENTRIES);
    city.set_label(MIN_USER_TEXT_ENTRIES, "Harbor Rd");
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.label_at(MIN_USER_TEXT_ENTRIES), Some(""));
}

#[test]
fn mac68k_bulldoze_clears_sign_xlab_like_pe_401dca() {
    let mut city = City::new_blank("/tmp/sc2000-bull-xlab-mac.sc2", "BullXlabMac").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, MIN_USER_TEXT_ENTRIES);
    city.set_label(MIN_USER_TEXT_ENTRIES, "Pier Ave");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.label_at(MIN_USER_TEXT_ENTRIES), Some(""));
}

#[test]
fn bulldoze_skips_xlab_clear_for_xtxt_f1_f9_like_pe() {
    // PE DestroyBuilding skips XTXT `0xf1..=0xf9` at the `0x45a0e1` gate.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-skip.sc2", "BullXtxtSkip").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xf5);
    city.set_label(2, "KeepMe");
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    // PE leaves XTXT `0xf1..=0xf9` on the tile; XLAB[2] untouched.
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xf5));
    assert_eq!(state.city.label_at(2), Some("KeepMe"));
}

#[test]
fn mac68k_bulldoze_skips_xlab_clear_for_xtxt_f1_f9_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-skip-mac.sc2", "BullXtxtSkipMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xf3);
    city.set_label(3, "MacKeep");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xf3));
    assert_eq!(state.city.label_at(3), Some("MacKeep"));
}

#[test]
fn bulldoze_clears_xtxt_0xfa_like_pe_destroy_building() {
    // PE DestroyBuilding @ `0x45a0fd`: XTXT 0xfa is cleared (and `0x401dca` called).
    let mut city = City::new_blank("/tmp/sc2000-bull-xtxt-fa.sc2", "BullXtxtFa").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfa);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
}

#[test]
fn mac68k_bulldoze_clears_xtxt_0xfa_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-fa-mac.sc2", "BullXtxtFaMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfa);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
}

#[test]
fn bulldoze_clears_sign_xtxt_and_xlabel_like_pe() {
    // PE DestroyBuilding @ `0x45a0fd`/`0x45a101`: XTXT < 0xc9 cleared; `0x401dca` clears XLAB.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-sign.sc2", "BullXtxtSign").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0x12);
    city.set_label(0x12, "UserSign");
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.label_at(0x12), Some(""));
}

#[test]
fn mac68k_bulldoze_clears_sign_xtxt_and_xlabel_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-sign-mac.sc2", "BullXtxtSignMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0x08);
    city.set_label(0x08, "MacSign");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.label_at(0x08), Some(""));
}

#[test]
fn bulldoze_clears_midband_xtxt_without_xlabel_like_pe() {
    // PE: XTXT 0x33..=0xc8 clears overlay; system 0x33..=0x3c `0x401dca` no-ops.
    // Unrelated sign slot XLAB stays.
    let mut city = City::new_blank("/tmp/sc2000-bull-xtxt-mid.sc2", "BullXtxtMid").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0x80);
    city.set_label(0x10, "KeepSignSlot");
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.label_at(0x10), Some("KeepSignSlot"));
}

#[test]
fn bulldoze_frees_structure_microsim_like_pe_401dca() {
    // PE `0x401dca` @ `0x44ced0`: XTXT 0x3d..=0xc8 → clear XMIC tile_id + XLAB.
    let mut city = City::new_blank("/tmp/sc2000-bull-xmic.sc2", "BullXmic").expect("blank");
    let overlay = city
        .provision_microsim_for_building(210)
        .expect("police microsim");
    let index = usize::from(overlay.saturating_sub(MIN_SIM_TEXT_ENTRIES));
    assert!(index >= MICROSIM_STRUCTURE_FIRST);
    assert_eq!(city.microsim_at(index).map(|r| r.tile_id), Some(210));
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, overlay);
    city.set_label(overlay, "PoliceSlot");
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.microsim_at(index).map(|r| r.tile_id), Some(0));
    assert_eq!(state.city.label_at(overlay), Some(""));
}

#[test]
fn mac68k_bulldoze_clears_midband_xtxt_without_xlabel_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-mid-mac.sc2", "BullXtxtMidMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0x55);
    city.set_label(0x11, "MacKeepSlot");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.label_at(0x11), Some("MacKeepSlot"));
}

#[test]
fn bulldoze_road_with_xtxt_0xfa_drops_recomputed_network_fa_count() {
    // PE DestroyBuilding @ `0x45a159` decs `0x4cc4d8` for FA-on-network; Rust recomputes.
    let mut city = City::new_blank("/tmp/sc2000-bull-fa-net.sc2", "BullFaNet").expect("blank");
    city.set_building(20, 20, 0x1d); // road network band
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 0);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&state.city), 0);
}

#[test]
fn mac68k_bulldoze_road_with_xtxt_0xfa_drops_recomputed_network_fa_count() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-fa-net-mac.sc2", "BullFaNetMac").expect("blank");
    city.set_building(20, 20, 0x1e);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&state.city), 0);
}

#[test]
fn bulldoze_park_with_xtxt_0xfa_drops_recomputed_non_network_fa_count() {
    // PE DestroyBuilding @ `0x45a150` decs `0x4ca3f0` for FA-on-non-network; Rust recomputes.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-fa-nonnet.sc2", "BullFaNonNet").expect("blank");
    city.set_building(20, 20, 0x0d); // park — non-network
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 0);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(
        citygame_fa_non_network_neighbor_marker_count(&state.city),
        0
    );
}

#[test]
fn mac68k_bulldoze_park_with_xtxt_0xfa_drops_recomputed_non_network_fa_count() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-fa-nonnet-mac.sc2", "BullFaNonNetMac").expect("blank");
    city.set_building(20, 20, 0x0e);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(
        citygame_fa_non_network_neighbor_marker_count(&state.city),
        0
    );
}

#[test]
fn bulldoze_skips_xtxt_arm_for_0xff_like_pe() {
    // PE @ `0x45a0e8`: XTXT 0xff (>0xfa) skips the whole XTXT/XLAB arm.
    let mut city = City::new_blank("/tmp/sc2000-bull-xtxt-ff.sc2", "BullXtxtFf").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xff);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xff));
}

#[test]
fn mac68k_bulldoze_skips_xtxt_arm_for_0xff_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-ff-mac.sc2", "BullXtxtFfMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xff);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xff));
}

#[test]
fn bulldoze_skips_xtxt_arm_for_0xfc_like_pe() {
    // PE @ `0x45a0e8`: XTXT 0xfc (>0xfa, <0xff) skips the whole XTXT/XLAB arm.
    let mut city = City::new_blank("/tmp/sc2000-bull-xtxt-fc.sc2", "BullXtxtFc").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfc);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xfc));
}

#[test]
fn mac68k_bulldoze_skips_xtxt_arm_for_0xfc_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-fc-mac.sc2", "BullXtxtFcMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfc);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xfc));
}

#[test]
fn bulldoze_skips_xtxt_arm_for_0xfd_like_pe() {
    // PE @ `0x45a0e8`: XTXT 0xfd (>0xfa) skips the whole XTXT/XLAB arm.
    let mut city = City::new_blank("/tmp/sc2000-bull-xtxt-fd.sc2", "BullXtxtFd").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfd);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xfd));
}

#[test]
fn mac68k_bulldoze_skips_xtxt_arm_for_0xfd_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-fd-mac.sc2", "BullXtxtFdMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfd);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xfd));
}

#[test]
fn bulldoze_skips_xtxt_arm_for_0xfe_like_pe() {
    // PE @ `0x45a0e8`: XTXT 0xfe (>0xfa) skips the whole XTXT/XLAB arm.
    let mut city = City::new_blank("/tmp/sc2000-bull-xtxt-fe.sc2", "BullXtxtFe").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfe);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xfe));
}

#[test]
fn mac68k_bulldoze_skips_xtxt_arm_for_0xfe_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-fe-mac.sc2", "BullXtxtFeMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfe);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xfe));
}

#[test]
fn mac68k_bulldoze_keeps_xzon_type_clears_corners() {
    let mut city = City::new_blank("/tmp/sc2000-bull-xzon-mac.sc2", "BullXzonMac").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT)
    );
}

#[test]
fn bulldoze_clears_multitile_building_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..=11 {
        for x in 10..=11 {
            city.set_building(x, y, 141);
            city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT | 0x80);
        }
    }
    let mut state = test_state(city);

    state.bulldoze_at(11, 11);

    for y in 10..=11 {
        for x in 10..=11 {
            assert!(matches!(state.city.building_at(x, y), 1..=4));
            // PE DestroyBuilding @ `0x45997e`: XZON &= 0x0f — keep type, clear corners.
            assert_eq!(
                state.city.zone_at(x, y).unwrap().raw_bits(),
                ZONE_RESIDENTIAL_LIGHT
            );
        }
    }
}

#[test]
fn mac68k_bulldoze_charges_one_unit_for_multitile_removal() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-bull-cost.sc2", "MacBullCost").expect("blank city");
    city.set_city_funds(100);
    let building = RESIDENTIAL_2X2_BUILDINGS[4];
    stamp_complete_building_for_test(&mut city, 20, 20, building, 2);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;

    assert_eq!(state.bulldoze_billable_tiles(20, 20).len(), 4);
    assert_eq!(state.bulldoze_cost_units_for_click(20, 20, 4), 1);
    assert!(state.apply_tool_at_tile(20, 20));

    assert_eq!(state.city.city_funds(), 99);
    for (x, y) in building_footprint_from_origin(20, 20, 2).expect("2x2 footprint") {
        assert!(matches!(state.city.building_at(x, y), 1..=4));
    }
}

#[test]
fn mac68k_bulldoze_lowclass_random_refusal_debits_and_preserves_tile() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-bull-refuse.sc2", "MacBullRefuse").expect("blank city");
    city.set_city_funds(100);
    city.set_building(20, 20, 6);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;
    state.mac_process_rng = MacToolboxRng::new(20);
    let mut expected_rng = state.mac_process_rng;
    assert_eq!(expected_rng.random_u16() % 20, 0);

    assert!(state.apply_tool_at_tile(20, 20));

    assert_eq!(state.city.city_funds(), 99);
    assert_eq!(state.city.building_at(20, 20), 6);
    assert_eq!(state.mac_process_rng.seed(), expected_rng.seed());
}

#[test]
fn mac68k_bulldoze_lowclass_nonrefusal_consumes_one_quickdraw_value() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-bull-accept.sc2", "MacBullAccept").expect("blank city");
    city.set_city_funds(100);
    city.set_building(20, 20, 6);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;
    state.mac_process_rng = MacToolboxRng::new(1);
    let mut expected_rng = state.mac_process_rng;
    assert_ne!(expected_rng.random_u16() % 20, 0);

    assert!(state.apply_tool_at_tile(20, 20));

    assert_eq!(state.city.city_funds(), 99);
    assert_ne!(state.city.building_at(20, 20), 6);
    assert_eq!(state.mac_process_rng.seed(), expected_rng.seed());
}

#[test]
fn bulldoze_clears_xtxt_footprint() {
    // PE DestroyStructure / demolish clears XTXT occupancy with XBLD.
    let mut city = City::new_blank("/tmp/sc2000-bulldoze-xtxt.sc2", "BullXtxt").expect("blank");
    let building = RESIDENTIAL_2X2_BUILDINGS[4];
    stamp_complete_building_for_test(&mut city, 20, 20, building, 2);
    city.set_text_overlay(20, 20, 0x55);
    city.set_text_overlay(21, 21, 0x66);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert!(matches!(state.city.building_at(21, 21), 1..=4));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.text_overlay_at(21, 21), Some(0));
}

#[test]
fn bulldoze_preserves_xtrf_like_pe_401604() {
    // PE DestroyStructure (`0x401604` → `0x461330`) never writes XTRF (`0x4ca940`).
    let mut city = City::new_blank("/tmp/sc2000-bulldoze-xtrf.sc2", "BullXtrf").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_traffic_density(20, 20, 66);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(66));
}

#[test]
fn bulldoze_clears_highway_2x2_pe_destroy_footprint() {
    // PE DestroyStructure size table: highway 0x49 is 2×2 (`0x45b1c0`).
    let mut city = City::new_blank("/tmp/sc2000-bull-hwy.sc2", "BullHwy").expect("blank");
    city.set_city_funds(50_000);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_building(x, y, 0x49);
        city.set_text_overlay(x, y, 0x55);
        city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_watered(true));
        city.set_traffic_density(x, y, 0x19);
    }
    let mut state = test_state(city);
    state.bulldoze_at(21, 21);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert!(
            matches!(state.city.building_at(x, y), 1..=4),
            "highway cell ({x},{y})"
        );
        assert_eq!(state.city.text_overlay_at(x, y), Some(0));
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped() && bits.watered());
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x19));
    }
}

#[test]
fn mac68k_bulldoze_clears_highway_2x2_preserves_xtrf() {
    // Shared PE destroy footprint; Mac68k must not invent XTRF clear on highway demolish.
    let mut city = City::new_blank("/tmp/sc2000-mac-bull-hwy.sc2", "MacBullHwy").expect("blank");
    city.set_city_funds(50_000);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_building(x, y, 0x49);
        city.set_traffic_density(x, y, 0x19);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(21, 21);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert!(matches!(state.city.building_at(x, y), 1..=4));
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x19));
    }
}

#[test]
fn mac68k_bulldoze_preserves_xtrf_like_shared_destroy_structure() {
    // Shared DestroyStructure path leaves XTRF; do not invent Mac CODE10/247 clear here.
    let mut city = City::new_blank("/tmp/sc2000-mac-bull-xtrf.sc2", "MacBullX").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_traffic_density(20, 20, 66);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(66));
}

#[test]
fn win95_bulldozer_charges_once_for_multitile_structure_click() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(1);
    let building = RESIDENTIAL_2X2_BUILDINGS[4];
    stamp_complete_building_for_test(&mut city, 20, 20, building, 2);
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.city_funds(), 0);
    for (x, y) in building_footprint_from_origin(20, 20, 2).expect("2x2 footprint") {
        assert!(matches!(state.city.building_at(x, y), 1..=4));
    }
}

#[test]
fn win95_bulldozer_respects_military_and_reserved_tile_gates() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 112);
    city.set_zone(20, 20, 0x07);
    city.set_building(30, 30, 5);
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;

    assert!(!state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 112);
    assert!(!state.apply_tool_at_tile(30, 30));
    assert_eq!(state.city.building_at(30, 30), 5);
    assert_eq!(state.city.city_funds(), 20_000);
}

#[test]
fn win95_bulldozer_rubble_refusal_charges_and_preserves_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20);
    city.set_building(20, 20, 6);
    city.set_zone(20, 20, 0x32);
    city.set_tile_bits(20, 20, TileBits::new(0x84));
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;
    state.pe_process_crt = Win95CrtRand::new(19); // first rand is 100, and 100 % 20 == 0
    let mut expected_crt = Win95CrtRand::new(19);
    assert_eq!(expected_crt.random_mod(20), 0);

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.city_funds(), 19);
    assert_eq!(state.city.building_at(20, 20), 6);
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 0x32);
    assert_eq!(state.city.tile_bits_at(20, 20).unwrap().raw_bits(), 0x84);
    assert_eq!(state.pe_process_crt, expected_crt);
}

#[test]
fn win95_bulldozer_clears_lowclass_tile_without_destroy_building_rubble() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20);
    city.set_building(20, 20, 6);
    city.set_zone(20, 20, 0x32);
    city.set_tile_bits(20, 20, TileBits::new(0x84));
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;
    state.pe_process_crt = Win95CrtRand::new(1); // first rand is 41, and 41 % 20 != 0
    let mut expected_crt = Win95CrtRand::new(1);
    assert_ne!(expected_crt.random_mod(20), 0);

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.city_funds(), 19);
    assert_eq!(state.city.building_at(20, 20), 0);
    // XTER < 0x30 returns before the PE's conditional surface-water cleanup.
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 0x32);
    assert_eq!(state.city.tile_bits_at(20, 20).unwrap().raw_bits(), 0x84);
    assert_eq!(state.pe_process_crt, expected_crt);
}

#[test]
fn bulldoze_drag_fills_path_between_mouse_samples() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    for x in 10..=13 {
        city.set_building(x, 10, 112);
    }
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;

    let (placed, previous) = state.paint_city_tool_drag_to(None, (10, 10));
    assert_eq!(placed, 1);
    let (placed, previous) = state.paint_city_tool_drag_to(previous, (13, 10));
    assert_eq!(placed, 3);
    assert_eq!(previous, Some((13, 10)));

    for x in 10..=13 {
        assert!(
            matches!(state.city.building_at(x, 10), 1..=4),
            "building at {x},10"
        );
    }
    assert_eq!(state.city.city_funds(), 19_996);
}

#[test]
fn direct_paint_drag_keeps_non_bulldoze_tools_on_current_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    for x in 10..=13 {
        city.set_terrain(x, 10, 0);
        city.set_building(x, 10, 0);
        city.set_altitude(x, 10, 5, 0);
    }
    let mut state = test_state(city);
    state.tool = EditTool::Trees;

    let (placed, previous) = state.paint_city_tool_drag_to(Some((10, 10)), (13, 10));
    assert_eq!(placed, 1);
    assert_eq!(previous, Some((13, 10)));
    assert_eq!(state.city.building_at(10, 10), 0);
    assert_eq!(state.city.building_at(11, 10), 0);
    assert_eq!(state.city.building_at(12, 10), 0);
    assert!(matches!(state.city.building_at(13, 10), 6 | 7));
}

#[test]
fn bulldoze_preview_and_apply_only_accept_clearable_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(10, 10, 112);
    city.set_terrain(20, 20, 0x30);
    city.set_terrain(30, 30, 0x20);
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;

    let building = state.tool_placement_at(10, 10).expect("building preview");
    assert_eq!(building.preview, ToolPreview::Bulldoze);
    assert_eq!(building.valid_tiles, vec![(10, 10)]);
    assert!(building.invalid_tiles.is_empty());
    assert!(state.apply_tool_at_tile(10, 10));

    let decorative_water = state
        .tool_placement_at(20, 20)
        .expect("decorative water preview");
    assert_eq!(decorative_water.valid_tiles, vec![(20, 20)]);
    assert!(decorative_water.invalid_tiles.is_empty());
    assert!(state.apply_tool_at_tile(20, 20));

    let natural_water = state
        .tool_placement_at(30, 30)
        .expect("natural water preview");
    assert!(natural_water.valid_tiles.is_empty());
    assert_eq!(natural_water.invalid_tiles, vec![(30, 30)]);
    assert!(!state.apply_tool_at_tile(30, 30));

    let empty = state.tool_placement_at(40, 40).expect("empty preview");
    assert!(empty.valid_tiles.is_empty());
    assert_eq!(empty.invalid_tiles, vec![(40, 40)]);
    assert!(!state.apply_tool_at_tile(40, 40));
}

#[test]
fn bulldoze_clears_only_building_owned_underground_side_effects() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    // Preserve the non-Win95 compatibility route's building-owned XUND cleanup;
    // original Mac Bulldozer behavior remains unverified.
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert_eq!(state.place_building_at(10, 10, 220), 1);
    assert_eq!(state.city.underground_at(10, 10), UNDER_TILE_PIPES_LR);
    state.bulldoze_at(10, 10);
    assert_eq!(state.city.underground_at(10, 10), 0);

    // Foreign non-pipe XUND under a pump is left alone on clear.
    assert_eq!(state.place_building_at(20, 20, 220), 1);
    state.city.set_underground(20, 20, UNDER_TILE_GREY_BOX_2);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.underground_at(20, 20), UNDER_TILE_GREY_BOX_2);

    assert_eq!(state.place_building_at(25, 25, 108), 1);
    assert_eq!(state.city.underground_at(25, 25), UNDER_TILE_GREY_BOX_2);
    state.bulldoze_at(25, 25);
    assert_eq!(state.city.underground_at(25, 25), 0);

    assert_eq!(state.place_building_at(30, 30, 249), 9);
    state.bulldoze_at(31, 31);
    for y in 30..=32 {
        for x in 30..=32 {
            assert_eq!(state.city.underground_at(x, y), 0);
        }
    }

    state.city.set_terrain(39, 40, 0x30);
    state
        .city
        .set_tile_bits(39, 40, crate::city::TileBits::new(0b0000_0001));
    assert_eq!(state.place_building_at(40, 40, 250), 9);
    state.bulldoze_at(41, 41);
    for y in 40..=42 {
        for x in 40..=42 {
            assert_eq!(state.city.underground_at(x, y), 0);
        }
    }
}

#[test]
fn pe_pipe_stroke_rejects_military_start_but_can_cross_military_zone() {
    let mut city =
        City::new_blank("/tmp/sc2000-pipe-military-walk.sc2", "PipeMilitaryWalk").expect("blank");
    city.set_zone(21, 20, ZONE_MILITARY);
    city.set_zone(22, 20, ZONE_MILITARY);

    assert_eq!(
        pe_pipe_stroke_path_448bc0_448f50(&city, (20, 20), (22, 20)),
        vec![(20, 20), (21, 20), (22, 20)]
    );

    let mut state = test_state(city);
    state.apply_selected_tool(EditTool::Pipes);
    let prefix = [(20, 20), (21, 20)];
    assert!(state.can_place_underground_path_tile(&prefix, 1, UndergroundKind::Pipes));
    state.city.set_zone(20, 20, ZONE_MILITARY);
    assert!(!state.can_place_underground_path_tile(&prefix, 0, UndergroundKind::Pipes));
    assert!(pe_pipe_stroke_path_448bc0_448f50(&state.city, (20, 20), (22, 20)).is_empty());
}

#[test]
fn pe_road_stroke_walk_uses_dominant_axis_and_rejects_native_start_gates() {
    let mut city =
        City::new_blank("/tmp/sc2000-road-walk-axis.sc2", "RoadWalkAxis").expect("blank city");
    flatten_road_walk_fixture(&mut city, 19, 23, 19, 22);
    assert_eq!(
        pe_road_stroke_plan_449bb0(&city, (20, 20), (22, 21))
            .expect("walk")
            .path,
        vec![(20, 20), (21, 20), (21, 21), (22, 21)]
    );

    city.set_building(20, 20, 0x05);
    assert!(pe_road_stroke_plan_449bb0(&city, (20, 20), (22, 21)).is_none());
    city.set_building(20, 20, 0);
    city.set_building(20, 20, 0x51);
    assert!(pe_road_stroke_plan_449bb0(&city, (20, 20), (22, 21)).is_none());
    city.set_building(20, 20, 0);
    city.set_terrain(20, 20, 0x10);
    assert!(pe_road_stroke_plan_449bb0(&city, (20, 20), (22, 21)).is_none());
    city.set_terrain(20, 20, 0);
    city.set_zone(20, 20, ZONE_MILITARY);
    assert!(pe_road_stroke_plan_449bb0(&city, (20, 20), (22, 21)).is_none());
}

#[test]
fn pe_rail_mode1_walk_keeps_direction_and_applies_rail_only_candidate_gates() {
    let mut city =
        City::new_blank("/tmp/sc2000-rail-mode1-walk.sc2", "RailMode1Walk").expect("blank city");
    flatten_road_walk_fixture(&mut city, 19, 22, 19, 22);

    // Mode 1 refuses an XZON type-7 candidate after selecting the direction.
    city.set_zone(21, 20, ZONE_MILITARY);
    let step = pe_rail_next_stroke_tile_449f50(&city, (20, 20), (21, 20), (20, 20, 21, 20), 1);
    assert_eq!(step.next, None);
    assert_eq!(step.direction, 1);

    // A saved-direction Rail tile rejects a nonzero-XTER step if ALTM land
    // changes, even when the common XTER direction gate would allow it.
    city.set_zone(21, 20, 0);
    city.set_building(20, 20, 0x2c);
    city.set_altitude(20, 20, 5, 5);
    city.set_altitude(21, 20, 6, 6);
    city.set_terrain(21, 20, 0x20);
    let step = pe_rail_next_stroke_tile_449f50(&city, (20, 20), (21, 20), (20, 20, 21, 20), 1);
    assert_eq!(step.next, None);
    assert_eq!(step.direction, 1);

    // In the dominant-axis branch, a change from the saved direction onto a
    // nonzero-XTER candidate is refused before replacing the saved direction.
    city.set_building(20, 20, 0);
    city.set_terrain(21, 20, 0x20);
    let step = pe_rail_next_stroke_tile_449f50(&city, (20, 20), (21, 20), (20, 20, 21, 20), 2);
    assert_eq!(step.next, None);
    assert_eq!(step.direction, 2);
}

#[test]
fn pe_road_bridge_preflight_masks_directions_and_fixed_interior_families() {
    assert_eq!(pe_road_bridge_selector_mask(4), 0x04);
    assert_eq!(pe_road_bridge_selector_mask(5), 0x0c);
    assert_eq!(pe_road_bridge_selector_mask(7), 0x1c);
    assert_eq!(pe_road_bridge_selector_mask(12), 0x14);
    assert_eq!(pe_road_bridge_selectors(0x1c), vec![2, 3, 4]);
    assert_eq!(pe_road_bridge_selector_cost(2), 25);
    assert_eq!(pe_road_bridge_selector_cost(3), 50);
    assert_eq!(pe_road_bridge_selector_cost(4), 75);
    assert_eq!(
        (1..=5)
            .map(|index| pe_road_bridge_interior_tile(3, 7, index, 1))
            .collect::<Vec<_>>(),
        vec![0x57, 0x56, 0x58, 0x56, 0x57]
    );
    assert_eq!(
        (1..=5)
            .map(|index| pe_road_bridge_interior_tile(4, 7, index, 0))
            .collect::<Vec<_>>(),
        vec![0x51, 0x52, 0x53, 0x54, 0x55]
    );
    assert_eq!(
        (1..=5)
            .map(|index| pe_road_bridge_interior_tile(4, 7, index, 1))
            .collect::<Vec<_>>(),
        vec![0x55, 0x54, 0x53, 0x52, 0x51]
    );
    assert_eq!(
        pe_road_bridge_interior_tile(4, 7, 2, 2),
        0x54,
        "direction 2 uses the descending family"
    );
    assert_eq!(
        pe_road_bridge_interior_tile(4, 7, 2, 3),
        0x52,
        "direction 3 uses the ascending family"
    );
    assert_eq!(pe_road_bridge_direction_mask(0x21), Some(0x02));
    assert_eq!(pe_road_bridge_direction_mask(0x28), Some(0x03));
    assert_eq!(pe_road_bridge_direction_mask(0x31), Some(0x02));
    assert_eq!(pe_road_bridge_direction_mask(0x29), None);
    assert_eq!(pe_road_bridge_direction_mask(0x39), None);
}

#[test]
fn pe_rail_bridge_selector_transaction_builds_centered_span_and_preserves_xtrf() {
    let mut city =
        City::new_blank("/tmp/sc2000-rail-bridge-pe.sc2", "RailBridgePe").expect("blank city");
    city.set_city_funds(10_000);
    for x in 10..=16 {
        city.set_terrain(x, 10, if x == 10 { 0x21 } else { 0x20 });
        city.set_altitude(x, 10, 5, 5);
        city.set_tile_bits(x, 10, TileBits::new(0).with_water_tile(true));
        if x % 2 == 0 {
            city.set_traffic_density(x, 10, 0x80 | x as u8);
        }
    }
    city.set_terrain(17, 10, 0x40);
    city.set_altitude(17, 10, 5, 5);

    let plan = pe_rail_bridge_span_plan(&city, (10, 10), 1).expect("valid Rail span");
    assert_eq!(plan.span_count, 7);
    assert_eq!(plan.selector_mask, 0x02);
    assert_eq!(pe_rail_bridge_selector_cost(), 75);
    assert_eq!(
        (1..=5)
            .map(|index| pe_rail_bridge_interior_tile(7, index))
            .collect::<Vec<_>>(),
        vec![0x5b, 0x5b, 0x5a, 0x5b, 0x5b]
    );

    let mut state = test_state(city);
    state.tool = EditTool::Rail;
    assert_eq!(state.begin_win95_rail_bridge((10, 10), 1), 0);
    assert!(matches!(
        state.city_info_dialog,
        Some(CityInfoDialog::RailBridge {
            span_count: 7,
            direction: 1,
            ..
        })
    ));
    let dialog = state.city_info_dialog.expect("Rail bridge chooser");
    let rect = city_info_dialog_rect_for(dialog, (1000, 700));
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(
                road_bridge_choice_rect(rect, 0).x + 4.0,
                road_bridge_choice_rect(rect, 0).y + 4.0,
            ),
            (1000, 700),
            dialog,
        ),
        Some(CityInfoDialogAction::SelectRailBridge)
    );

    assert_eq!(state.finish_win95_rail_bridge((10, 10), 1, 7), 7);
    assert_eq!(state.city.city_funds(), 9_475);
    assert!(NetworkKind::Rail.contains(state.city.building_at(10, 10)));
    assert!(NetworkKind::Rail.contains(state.city.building_at(16, 10)));
    assert_eq!(state.city.terrain_at(10, 10), 3);
    assert_eq!(state.city.terrain_at(16, 10), 1);
    assert_eq!(state.city.altitude_at(10, 10) & 0x1f, 6);
    assert_eq!(state.city.altitude_at(16, 10) & 0x1f, 6);
    assert_eq!(
        (11..=14)
            .map(|x| state.city.building_at(x, 10))
            .collect::<Vec<_>>(),
        vec![0x5b, 0x5b, 0x5a, 0x5b]
    );
    assert!(NetworkKind::Rail.contains(state.city.building_at(15, 10)));
    for x in 10..=16 {
        let bits = state.city.tile_bits_at(x, 10).expect("bridge XBIT");
        assert_eq!(
            bits.rotate(),
            (11..=15).contains(&x),
            "odd direction sets XBIT bit 1 on interiors only"
        );
        assert_eq!(
            bits.water_tile(),
            (11..=15).contains(&x),
            "only endpoints clear XBIT water"
        );
        assert_eq!(
            state.city.traffic_density_at(x, 10),
            Some(0x80 | (x & !1) as u8),
            "Rail bridge construction must leave XTRF unchanged"
        );
    }
}

#[test]
fn pe_road_bridge_selector_transaction_builds_span_and_charges_native_rate() {
    let mut city =
        City::new_blank("/tmp/sc2000-road-bridge-pe.sc2", "RoadBridgePe").expect("blank city");
    city.set_city_funds(10_000);
    for x in 10..=16 {
        city.set_terrain(x, 10, if x == 10 { 0x21 } else { 0x20 });
        city.set_altitude(x, 10, 5, 5);
        city.set_tile_bits(x, 10, TileBits::new(0).with_water_tile(true));
        if x % 2 == 0 {
            city.set_traffic_density(x, 10, 0x80 | x as u8);
        }
    }
    city.set_terrain(17, 10, 0x40);
    city.set_altitude(17, 10, 5, 5);

    let plan = pe_road_bridge_span_plan(&city, (10, 10), 1).expect("valid eastward span");
    assert_eq!(plan.span_count, 7);
    assert_eq!(plan.terminal, (17, 10));
    assert_eq!(plan.selector_mask, 0x1c);
    assert_eq!(plan.cells.len(), 7);

    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.begin_win95_road_bridge((10, 10), 1), 0);
    assert!(matches!(
        state.city_info_dialog,
        Some(CityInfoDialog::RoadBridge {
            span_count: 7,
            selector_mask: 0x1c,
            ..
        })
    ));
    let rect =
        city_info_dialog_rect_for(state.city_info_dialog.expect("bridge chooser"), (1000, 700));
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(
                road_bridge_choice_rect(rect, 2).x + 4.0,
                road_bridge_choice_rect(rect, 2).y + 4.0,
            ),
            (1000, 700),
            state.city_info_dialog.expect("bridge chooser"),
        ),
        Some(CityInfoDialogAction::SelectRoadBridge(4))
    );

    assert_eq!(state.finish_win95_road_bridge((10, 10), 1, 7, 4), 7);
    assert_eq!(state.city.city_funds(), 9_475);
    assert!(NetworkKind::Road.contains(state.city.building_at(10, 10)));
    assert!(NetworkKind::Road.contains(state.city.building_at(16, 10)));
    assert_eq!(state.city.terrain_at(10, 10), 3);
    assert_eq!(state.city.terrain_at(16, 10), 1);
    assert_eq!(state.city.altitude_at(10, 10) & 0x1f, 6);
    assert_eq!(state.city.altitude_at(16, 10) & 0x1f, 6);
    assert_eq!(
        (11..=15)
            .map(|x| state.city.building_at(x, 10))
            .collect::<Vec<_>>(),
        vec![0x55, 0x54, 0x53, 0x52, 0x51]
    );
    for x in 11..=15 {
        let bits = state.city.tile_bits_at(x, 10).expect("tile bits");
        assert!(bits.water_tile());
        assert!(bits.rotate());
    }
    for x in 10..=17 {
        assert_eq!(
            state.city.traffic_density_at(x, 10),
            Some(0x80 | (x & !1) as u8),
            "bridge construction must preserve the saved XTRF byte at ({x}, 10)"
        );
    }
    assert!(
        !state
            .city
            .tile_bits_at(10, 10)
            .expect("endpoint bits")
            .water_tile()
    );
    assert!(
        !state
            .city
            .tile_bits_at(16, 10)
            .expect("endpoint bits")
            .water_tile()
    );
}

#[test]
fn pe_road_one_tile_bridge_uses_terminal_as_far_endpoint() {
    let mut city = City::new_blank("/tmp/sc2000-road-bridge-one-tile.sc2", "RoadBridgeOneTile")
        .expect("blank city");
    city.set_city_funds(10_000);
    city.set_terrain(10, 10, 0x21);
    city.set_altitude(10, 10, 5, 5);
    city.set_tile_bits(10, 10, TileBits::new(0).with_water_tile(true));
    city.set_terrain(11, 10, 0x40);
    city.set_altitude(11, 10, 5, 5);

    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.begin_win95_road_bridge((10, 10), 1), 0);
    assert!(matches!(
        state.city_info_dialog,
        Some(CityInfoDialog::RoadBridge {
            span_count: 1,
            selector_mask: 0x04,
            ..
        })
    ));
    assert_eq!(state.finish_win95_road_bridge((10, 10), 1, 1, 2), 2);
    assert_eq!(state.city.city_funds(), 9_975);
    assert!(NetworkKind::Road.contains(state.city.building_at(10, 10)));
    assert!(NetworkKind::Road.contains(state.city.building_at(11, 10)));
    assert_eq!(state.city.terrain_at(10, 10), 3);
    assert_eq!(state.city.terrain_at(11, 10), 1);
    assert_eq!(state.city.altitude_at(10, 10) & 0x1f, 6);
    assert_eq!(state.city.altitude_at(11, 10) & 0x1f, 5);
}

#[test]
fn pe_road_bridge_span_rejects_occupied_interior_and_requires_native_seed_class() {
    let mut city = City::new_blank("/tmp/sc2000-road-bridge-gates.sc2", "RoadBridgeGates")
        .expect("blank city");
    for x in 10..=13 {
        city.set_terrain(x, 10, if x == 10 { 0x21 } else { 0x20 });
    }
    city.set_terrain(14, 10, 0x40);
    assert!(pe_road_bridge_span_plan(&city, (10, 10), 1).is_some());
    assert!(pe_road_bridge_span_plan(&city, (10, 10), 0).is_none());
    city.set_building(12, 10, 0x1d);
    assert!(pe_road_bridge_span_plan(&city, (10, 10), 1).is_none());
    city.set_building(12, 10, 0);
    city.set_terrain(10, 10, 0x20);
    assert!(pe_road_bridge_span_plan(&city, (10, 10), 1).is_none());
}

#[test]
fn pe_road_stroke_candidate_xbld_gate_uses_saved_direction_parity() {
    assert!(pe_road_candidate_xbld_allows_step_448f50(0x00, 0));
    assert!(!pe_road_candidate_xbld_allows_step_448f50(0x05, 0));
    assert!(!pe_road_candidate_xbld_allows_step_448f50(0x0d, 1));
    assert!(pe_road_candidate_xbld_allows_step_448f50(0x0e, 1));
    assert!(!pe_road_candidate_xbld_allows_step_448f50(0x0e, 0));
    assert!(pe_road_candidate_xbld_allows_step_448f50(0x1d, 1));
    assert!(!pe_road_candidate_xbld_allows_step_448f50(0x1d, 0));
    assert!(!pe_road_candidate_xbld_allows_step_448f50(0x51, 1));
}

#[test]
fn pe_road_stroke_commit_rewalks_after_neighbor_road_reshape() {
    let mut city =
        City::new_blank("/tmp/sc2000-road-live-walk.sc2", "RoadLiveWalk").expect("blank city");
    flatten_road_walk_fixture(&mut city, 19, 24, 19, 22);
    city.set_city_funds(10_000);
    let straight_road = network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT);
    assert_eq!(straight_road, 0x1e);
    city.set_building(21, 20, 0x1d);
    city.set_building(22, 20, 0x1d);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    state.begin_network_drag(20, 20, NetworkKind::Road);
    state.update_network_drag_to((22, 21));

    // The read-only pass follows the pre-existing `0x1d` direction pair east.
    assert_eq!(
        pe_road_stroke_plan_449bb0(&state.city, (20, 20), (22, 21))
            .expect("walk")
            .path,
        vec![(20, 20), (21, 20), (22, 20)]
    );
    // Writing the start tile reshapes that neighbor to `0x1e`, so the live
    // commit no longer follows the saved east direction at the tie.
    assert_eq!(
        state.finish_network_drag(),
        Some((NetworkKind::Road, 3, None, None))
    );
    assert!(NetworkKind::Road.contains(state.city.building_at(21, 21)));
    assert!(NetworkKind::Road.contains(state.city.building_at(22, 21)));
}

#[test]
fn underground_bulldoze_clears_xund_pipe_tile() {
    let mut city = City::new_blank("/tmp/sc2000-ug-bulldoze.sc2", "ug").expect("blank");
    city.set_city_funds(50_000);
    city.set_underground(20, 20, UNDER_TILE_PIPES_LR);
    let mut state = test_state(city);
    state.show_underground = true;
    state.tool = EditTool::Bulldoze;

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.underground_at(20, 20), 0);
}

#[test]
fn pe_idle_drag_gate_does_not_infer_mac_callback_or_hold_selected_tool() {
    let city = blank_city_layout_fixture("pe-idle-unheld.sc2", "unheld input");
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.tool = EditTool::Road;
    state.traffic.set_speed(GameSpeed::Cheetah);
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(200))
            .days,
        1
    );

    let mut mac = test_state(blank_city_layout_fixture(
        "pe-idle-mac-drag.city",
        "Mac input",
    ));
    mac.engine_fidelity = EngineFidelity::Mac68k;
    mac.no_disasters_enabled = true;
    mac.traffic.set_speed(GameSpeed::Cheetah);
    mac.begin_network_drag(20, 20, NetworkKind::Road);
    let mut control = test_state(blank_city_layout_fixture(
        "pe-idle-mac-control.city",
        "Mac control",
    ));
    control.engine_fidelity = EngineFidelity::Mac68k;
    control.no_disasters_enabled = true;
    control.traffic.set_speed(GameSpeed::Cheetah);
    assert_eq!(
        mac.advance_frame_simulation(Duration::from_millis(200))
            .days,
        control
            .advance_frame_simulation(Duration::from_millis(200))
            .days
    );
}

#[test]
fn win95_xval_output_samples_one_native_cell_per_two_by_two_area() {
    let mut city = City::new_blank("/tmp/win95-xval-paired.sc2", "Xval").unwrap();
    city.set_zone(11, 13, ZONE_INDUSTRIAL_DENSE);
    let water = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let zero = vec![0; CITY_TILES];
    let map = live_land_value_density_map_for_city_at_center(
        &city,
        &water,
        &zero,
        &zero,
        &zero,
        (32, 32),
        true,
    );
    let native = map[density_map_index(10, 12)];
    assert!(native > 0, "odd/odd zone makes the native cell eligible");
    for x in 10..=11 {
        for y in 12..=13 {
            assert_eq!(map[density_map_index(x, y)], native, "({x}, {y})");
        }
    }
}

#[test]
fn win95_xval_developed_marker_preserves_only_uncleared_bits_and_projects_sources() {
    let mut city = City::new_blank("/tmp/win95-xval-mask.sc2", "Xval").unwrap();
    city.set_tile_bits(50, 50, TileBits::new(0x08));
    assert!(pe_xval_output_is_eligible(&city, 100, 100));
    city.set_building(50, 50, 0x70);
    assert!(!pe_xval_output_is_eligible(&city, 100, 100));
    city.set_building(101, 100, 0x1c);
    assert!(!pe_xval_output_is_eligible(&city, 100, 100));
    city.set_building(101, 100, 0x1d);
    assert!(pe_xval_output_is_eligible(&city, 100, 100));
}
