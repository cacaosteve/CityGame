use super::*;

#[test]
fn native_view_menu_commands_cover_camera_and_overlay_actions() {
    let cases = [
        (3, AppCommand::ZoomIn),
        (4, AppCommand::ZoomOut),
        (5, AppCommand::ActualSize),
        (6, AppCommand::CenterMap),
        (7, AppCommand::ToggleGrid),
        (8, AppCommand::InspectTile),
        (9, AppCommand::ToggleZoneOverlay),
        (10, AppCommand::FitMap),
        (11, AppCommand::ToggleTraffic),
        (12, AppCommand::InspectHoverTile),
        (30, AppCommand::RotateView(-1)),
        (31, AppCommand::RotateView(1)),
    ];
    for (command, app_command) in cases {
        assert_eq!(app_command_for_menu_command(command), Some(app_command));
    }
    assert_eq!(app_command_for_menu_command(32), None);
}

#[test]
fn mac_kind9_water_activation_queues_roar1_once_through_actor_sweep() {
    let mut city = blank_city_layout_fixture("mac-kind9-roar.city", "Mac kind-9 Roar1");
    let slot = XTHG_FIRST_USABLE_SLOT;
    let (x, y) = (40, 41);
    city.set_tile_bits(x, y, TileBits::new(0x04));
    city.mac_object_slot_records_mut()[slot] = Win95OverlayRecord {
        kind: XTHG_SAILBOAT,
        tile_x: y as u8,
        tile_y: x as u8,
        sub_x: 4,
        sub_y: 4,
        ..Win95OverlayRecord::default()
    };

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::CityView;
    state.zoom = 5.0; // Native zoom band 2 admits the Mac kind-9 handler.
    state.traffic.set_speed(GameSpeed::Turtle);
    state.sound_effects_enabled = true;
    state
        .city
        .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);

    state.advance_frame_simulation(Duration::from_millis(17));

    assert_eq!(state.city.traffic_overlays().unwrap()[slot].state, 1);
    assert_eq!(
        state.take_pending_disaster_extra_sounds(),
        vec![PlacementSound::MacSailboatRoar]
    );

    // With the reconstructed zero A5-2 LFSR, the next due sweep removes the
    // state-1 record and does not request Roar1 again.
    state.advance_frame_simulation(Duration::from_millis(750));
    state.advance_frame_simulation(Duration::from_millis(17));
    assert_eq!(state.city.traffic_overlays().unwrap()[slot].kind, 0);
    assert!(state.take_pending_disaster_extra_sounds().is_empty());
}

#[test]
fn win95_engine_persists_native_speed_word_over_mac_layout_only_for_supported_values() {
    let mut city = City::new_blank("/tmp/sc2000-mac-speed.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_misc_u32_at(MISC_SPEED_OFFSET, 0xbeef_0002);
    let mut state = test_state(city);

    assert_eq!(state.traffic.speed(), GameSpeed::Turtle);
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Cheetah));
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(0xbeef_0004));

    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::AfricanSwallow));
    assert_eq!(state.traffic.speed(), GameSpeed::AfricanSwallow);
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(0xbeef_0004));
}

#[test]
fn mac_auto_goto_uses_native_option_and_round_trips_only_for_mac_city_fidelity() {
    let path = PathBuf::from("/tmp/sc2000-mac-auto-goto-state");
    let mut city = blank_city_layout_fixture(&path, "MacAutoGoto");
    city.set_mac_state_misc_u32_at(AUTO_GOTO_MISC_OFFSET, 0xAABB_CC00);
    city.set_mac_state_misc_u32_at(SOUND_EFFECTS_MISC_OFFSET, 0x5151_5101);
    city.set_mac_state_misc_u32_at(MUSIC_MISC_OFFSET, 0x6262_6200);

    assert!(!auto_goto_enabled_for_engine(&city, EngineFidelity::Mac68k));
    assert!(auto_goto_enabled_for_engine(&city, EngineFidelity::Win95));

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.auto_goto_enabled = auto_goto_enabled_for_engine(&state.city, state.engine_fidelity);
    assert!(!state.auto_goto_enabled);
    state.toggle_auto_goto();
    assert!(state.auto_goto_enabled);
    assert_eq!(
        state.city.mac_state_misc_u32_at(AUTO_GOTO_MISC_OFFSET),
        Some(1),
        "the native byte is zero-extended when Auto-Goto is enabled"
    );
    state.toggle_auto_goto();
    assert!(!state.auto_goto_enabled);
    assert_eq!(
        state.city.mac_state_misc_u32_at(AUTO_GOTO_MISC_OFFSET),
        Some(0),
        "the native byte is zero-extended when Auto-Goto is disabled"
    );

    state
        .city
        .save(&path)
        .expect("save native Auto-Goto option");
    let loaded = City::load(&path).expect("reload native Auto-Goto option");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert!(!auto_goto_enabled_for_engine(
        &loaded,
        EngineFidelity::Mac68k
    ));
    assert_eq!(
        loaded.mac_state_misc_u32_at(SOUND_EFFECTS_MISC_OFFSET),
        Some(0x5151_5101),
        "the adjacent sound slot remains native and untouched"
    );
    assert_eq!(
        loaded.mac_state_misc_u32_at(MUSIC_MISC_OFFSET),
        Some(0x6262_6200),
        "the adjacent music slot remains native and untouched"
    );

    let mut pe_city = City::new_blank("/tmp/sc2000-mac-engine-pe-layout.sc2", "PE layout")
        .expect("blank PE-layout city");
    pe_city.set_misc_u32_at(AUTO_GOTO_MISC_OFFSET, CITY_OPTION_DISABLED_VALUE);
    assert!(auto_goto_enabled_for_engine(
        &pe_city,
        EngineFidelity::Mac68k
    ));
    assert!(!set_auto_goto_enabled_for_engine(
        &mut pe_city,
        EngineFidelity::Mac68k,
        false
    ));
    assert_eq!(
        pe_city.misc_u32_at(AUTO_GOTO_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE),
        "Mac fidelity must not reinterpret a PE-layout option slot"
    );
    std::fs::remove_file(path).expect("remove round-trip fixture");
}

#[test]
fn mac_transit_route_switch_matches_documented_transition_families() {
    use MacTransitRouteTransition::{Accept, Reject, Success};

    let cases = [
        (
            0,
            0x40,
            0,
            0,
            Accept {
                next_state: 2,
                cost: 3,
            },
        ),
        (
            0,
            0x53,
            0,
            0,
            Accept {
                next_state: 3,
                cost: 3,
            },
        ),
        (
            0,
            0x5d,
            0,
            0,
            Accept {
                next_state: 1,
                cost: 2,
            },
        ),
        (
            1,
            0x61,
            0,
            0,
            Accept {
                next_state: 1,
                cost: 1,
            },
        ),
        (
            2,
            0x70,
            1,
            0,
            Accept {
                next_state: 2,
                cost: 3,
            },
        ),
        (2, 0x70, 0, 0, Reject),
        (
            3,
            0x52,
            0,
            0,
            Accept {
                next_state: 3,
                cost: 3,
            },
        ),
        (
            4,
            0x52,
            0,
            0,
            Accept {
                next_state: 7,
                cost: 2,
            },
        ),
        (
            5,
            0x49,
            0,
            0,
            Accept {
                next_state: 5,
                cost: 1,
            },
        ),
        (
            6,
            0x70,
            1,
            0,
            Accept {
                next_state: 6,
                cost: 2,
            },
        ),
        (
            7,
            0x20,
            0,
            0,
            Accept {
                next_state: 4,
                cost: 2,
            },
        ),
        (
            8,
            0xec,
            0,
            0,
            Accept {
                next_state: 8,
                cost: 4,
            },
        ),
        (
            9,
            0xed,
            0,
            0,
            Accept {
                next_state: 9,
                cost: 4,
            },
        ),
        (
            10,
            0xed,
            0,
            0,
            Accept {
                next_state: 10,
                cost: 4,
            },
        ),
        (
            10,
            0x2c,
            0,
            0,
            Accept {
                next_state: 12,
                cost: 1,
            },
        ),
        (
            11,
            0,
            0,
            0x22,
            Accept {
                next_state: 13,
                cost: 1,
            },
        ),
        (
            12,
            0xed,
            0,
            0,
            Accept {
                next_state: 9,
                cost: 4,
            },
        ),
        (
            12,
            0x2c,
            0,
            0,
            Accept {
                next_state: 12,
                cost: 1,
            },
        ),
        (
            13,
            0xe9,
            0,
            0,
            Accept {
                next_state: 9,
                cost: 4,
            },
        ),
        (13, 0, 0, 0x21, Reject),
    ];
    for (state, building, tunnel_levels, underground, expected) in cases {
        assert_eq!(
            mac_transit_route_transition(state, 1, 0, building, tunnel_levels, underground),
            expected,
            "transition from state {state} through XBLD {building:#04x}"
        );
    }

    for state in [0, 4, 8, 9] {
        assert_eq!(
            mac_transit_route_transition(state, 1, 3, 0x40, 0, 0),
            Success,
            "set zone-mask bit terminates route from state {state}"
        );
    }
    assert_eq!(
        mac_transit_route_transition(0, 1, 0, 0x40, 0, 0),
        Accept {
            next_state: 2,
            cost: 3
        },
        "clear zone-mask bit falls through to the XBLD transition"
    );
    assert_eq!(mac_transit_route_transition(0, 1, 0, 0xfb, 0, 0), Success);
    assert_eq!(mac_transit_route_transition(12, 1, 0, 0xfc, 0, 0), Success);
}

#[test]
fn mac_transit_route_seed_uses_native_offset_priority_without_randomness() {
    let mut tiles = vec![0_u8; CITY_TILES];
    let center = (40, 40);
    tiles[(center.1 + 1) * CITY_WIDTH + center.0] = 0xed;
    tiles[center.1 * CITY_WIDTH + center.0 + 1] = 0xec;
    assert_eq!(
        mac_transit_route_seed(center.0, center.1, |x, y| tiles[y * CITY_WIDTH + x]),
        Some((center.0, center.1 + 1, 10)),
        "the first seed offset wins over later special networks"
    );
    assert_eq!(
        mac_transit_route_seed(0, 0, |_, _| 0xe9),
        Some((0, 1, 11)),
        "out-of-bounds offsets are skipped while preserving scan order"
    );
    assert_eq!(mac_transit_route_seed(64, 64, |_, _| 0), None);
}

#[test]
fn mac_transit_route_search_reaches_zone_and_keeps_native_draw_order() {
    let mut cells = vec![MacTransitRouteCell::default(); CITY_TILES];
    cells[41 * CITY_WIDTH + 40].building = 0x20;
    cells[42 * CITY_WIDTH + 40].zone_low = 3;
    let mut random_values = [0_i16, 0_i16].into_iter();
    let mut probes = Vec::new();
    let result = mac_transit_route_search(
        40,
        40,
        1,
        2,
        100,
        |x, y| {
            probes.push((x, y));
            cells[y * CITY_WIDTH + x]
        },
        || random_values.next().expect("two route draws"),
    )
    .expect("zone-mask match completes a route");
    assert_eq!(result.route, [(40, 41, 0)]);
    assert_eq!(result.cost, 0);
    assert_eq!(result.random_draws, 2);
    assert_eq!(
        probes,
        [(40, 41), (40, 42)],
        "with stride 1 and base 0, native tests south first (base + stride)",
    );

    let mut draws = 0;
    assert!(
        mac_transit_route_search(
            40,
            40,
            1,
            2,
            100,
            |_, _| MacTransitRouteCell::default(),
            || {
                draws += 1;
                0
            }
        )
        .is_none()
    );
    assert_eq!(draws, 0, "no route seed means no QuickDraw calls");
}

#[test]
fn mac_transit_route_unwinds_empty_parent_masks_without_random_draws() {
    let mut cells = vec![MacTransitRouteCell::default(); CITY_TILES];
    // Seed -> south -> west forms a dead-end branch. Entering the west tile
    // consumes the south tile's last direction, so its mask is already empty
    // when the child exhausts. Native CODE 14 unwinds that parent before the
    // next QuickDraw call.
    cells[41 * CITY_WIDTH + 40].building = 0x20;
    cells[42 * CITY_WIDTH + 40].building = 0x20;
    cells[42 * CITY_WIDTH + 39].building = 0x20;

    let mut draws = 0;
    assert!(
        mac_transit_route_search(
            40,
            40,
            1,
            2,
            100,
            |x, y| cells[y * CITY_WIDTH + x],
            || {
                draws += 1;
                0
            }
        )
        .is_none()
    );
    assert_eq!(draws, 5, "no random word is used for the empty parent mask");
}

#[test]
fn mac_transit_route_success_updates_density_and_mode_accumulators() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-route-effects.sc2", "RouteEffects").expect("blank city");
    city.set_traffic_density(40, 40, 254);
    city.set_traffic_density(42, 40, 10);
    let result = MacTransitRouteResult {
        route: vec![
            (40, 40, 0),
            (41, 40, 0),
            (42, 40, 0),
            (44, 40, 8),
            (45, 40, 8),
            (46, 40, 10),
            (47, 40, 11),
        ],
        cost: 9,
        random_draws: 4,
    };
    mac_transit_apply_route_success(&mut city, &result, 4);
    assert_eq!(city.traffic_density_at(40, 40), Some(0xff));
    assert_eq!(city.traffic_density_at(42, 40), Some(14));
    assert_eq!(city.mac_transit_accumulator(0), 4);
    assert_eq!(city.mac_transit_accumulator(1), 4);
    assert_eq!(city.mac_transit_accumulator(2), 4);
}

#[test]
fn mac_transit_route_updates_saved_totals_without_touching_industry() {
    let path = PathBuf::from("/tmp/citygame-mac-transit-accumulators.city");
    let mut city = City::new_blank(&path, "Route").expect("blank city");
    city.kind = CityFileKind::MacCity;
    for slot in 97..=99 {
        city.set_mac_state_misc_u32_at(slot * 4, 0x1234_0000 + slot as u32);
    }
    let route = MacTransitRouteResult {
        route: vec![(30, 30, 8), (31, 30, 10), (32, 30, 11)],
        cost: 0,
        random_draws: 0,
    };
    mac_transit_apply_route_success(&mut city, &route, 7);
    city.save(&path).expect("save Mac city");
    let loaded = City::load(&path).expect("reload Mac city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    for mode in 0..3 {
        assert_eq!(loaded.mac_transit_accumulator(mode), 7);
    }
    for slot in 97..=99 {
        assert_eq!(
            loaded.mac_state_misc_u32_at(slot * 4),
            Some(0x1234_0000 + slot as u32)
        );
    }
}

#[test]
fn mac_transit_density_floors_route_coordinates_to_native_half_grid() {
    let mut city = City::new_blank("/tmp/sc2000-mac-route-density-grid.sc2", "RouteDensityGrid")
        .expect("blank city");
    let result = MacTransitRouteResult {
        route: vec![(40, 40, 0), (41, 40, 1), (41, 41, 3), (42, 41, 0)],
        cost: 4,
        random_draws: 0,
    };

    // The native worker indexes ffffe740 by y/2 then x/2. The first three
    // route coordinates therefore increment one shared native byte.
    mac_transit_apply_route_success(&mut city, &result, 5);
    for y in 40..=41 {
        for x in 40..=41 {
            assert_eq!(city.traffic_density_at(x, y), Some(15));
        }
    }
    for y in 40..=41 {
        for x in 42..=43 {
            assert_eq!(city.traffic_density_at(x, y), Some(5));
        }
    }
}

#[test]
fn mac_rci_scan_runs_powered_transit_route_before_growth_counters() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-rci-route-scan.sc2", "MacRouteScan").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.saved_rotation = 1;
    city.set_view_rotation(0); // renderer compass must not choose native Mac markers
    city.set_zone(40, 40, 0x11); // saved selector 1's XZON marker + category 1
    city.set_building(40, 40, 0x70);
    city.set_tile_bits(40, 40, TileBits::new(0x40)); // XBIT power; route marker is in XZON
    city.set_building(40, 41, 0x20);
    city.set_zone(40, 42, 3); // CODE 14 accepts zone ids 3..=15 for category 1

    let route_for_seed = |seed| {
        let mut rng = MacToolboxRng::new(seed);
        mac_transit_route_search(
            40,
            40,
            1,
            mac_growth_coefficient_for_xbld(0x70),
            100,
            |x, y| MacTransitRouteCell {
                building: city.building_at(x, y),
                zone_low: city.zone_at(x, y).map_or(0, |zone| zone.raw_bits() & 0x0f),
                tunnel_levels: city.tunnel_levels_at(x, y),
                underground: city.underground_at(x, y),
                text_overlay: city.text_overlay_at(x, y).unwrap_or(0),
            },
            || rng.random_i16(),
        )
    };
    let seed = (1..10_000)
        .find(|seed| route_for_seed(*seed).is_some())
        .expect("at least one test seed finds the adjacent zone");
    let mut expected_rng = MacToolboxRng::new(seed);
    assert!(
        mac_transit_route_search(
            40,
            40,
            1,
            mac_growth_coefficient_for_xbld(0x70),
            100,
            |x, y| MacTransitRouteCell {
                building: city.building_at(x, y),
                zone_low: city.zone_at(x, y).map_or(0, |zone| zone.raw_bits() & 0x0f),
                tunnel_levels: city.tunnel_levels_at(x, y),
                underground: city.underground_at(x, y),
                text_overlay: city.text_overlay_at(x, y).unwrap_or(0),
            },
            || expected_rng.random_i16(),
        )
        .is_some()
    );
    // The newly connected class-0 caller consumes its threshold after
    // the route worker; accepted growth then consumes orientation and one
    // export-243/export-242 placement draw.
    let class_zero_roll = expected_rng.random_i16() as u16;
    if class_zero_roll < 2000 {
        expected_rng.random_i16(); // export_243 orientation bit
        expected_rng.random_i16(); // one selector-1 square selection
    }
    let expected_rng_state = expected_rng.seed();

    let calendar_city = city.clone();
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.traffic_density_at(40, 41), Some(1));
    assert_eq!(city.mac_transit_accumulator(0), 0);
    assert_eq!(city.mac_state_misc_u32_at(0x05f0 + 4), Some(1));
    assert_eq!(
        rng.seed(),
        expected_rng_state,
        "the scan shares route draws in place"
    );

    let mut calendar = test_state(calendar_city);
    calendar.engine_fidelity = EngineFidelity::Mac68k;
    calendar.mac_process_rng = MacToolboxRng::new(seed);
    calendar.sim_calendar_mac_days_3_18_rci_scan();
    assert!(
        calendar
            .city
            .traffic_density_at(40, 41)
            .is_some_and(|density| density > 0),
        "the production Mac phases 3–18 dispatcher reaches the native route XTRF writer"
    );
}

#[test]
fn mac_rci_weighted_counter_uses_xbld_not_actor_xtxt_proxy() {
    for text_overlay in [0, 0xc9] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-rci-counter-xtxt-proxy.sc2",
            "MacRciCounterXtxtProxy",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.saved_rotation = 0;
        city.set_zone(40, 40, 0x81);
        city.set_building(40, 40, 0xf4);
        city.set_text_overlay(40, 40, text_overlay);

        let mut rng = MacToolboxRng::new(1);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        assert_eq!(
            city.mac_state_misc_u32_at(0x05f0 + 4),
            Some(15),
            "the phase-21 base counter reads XBLD/XZON class state, not XTXT proxy {text_overlay:#04x}"
        );
    }
}

#[test]
fn mac_rci_low_xbld_override_queries_transit_stencil_and_places_mode_zero() {
    let mut query_city =
        City::new_blank("/tmp/sc2000-mac-rci-low-query.sc2", "MacLowQuery").expect("blank city");
    query_city.set_building(41, 40, 0x20);
    assert!(super::super::budget::mac_budget_has_nearby_transit(
        &query_city,
        40,
        40
    ));
    query_city.set_building(41, 40, 0);
    query_city.set_building(43, 43, 0x20);
    assert!(
        !super::super::budget::mac_budget_has_nearby_transit(&query_city, 40, 40),
        "the query is the recovered 24-cell stencil, not a 3x3 square"
    );
    query_city.set_building(127, 127, 0x20);
    assert!(super::super::budget::mac_budget_has_nearby_transit(
        &query_city,
        124,
        127
    ));

    let mut city = City::new_blank("/tmp/sc2000-mac-rci-low-mode-zero.sc2", "MacLowModeZero")
        .expect("blank city");
    let seed = (1..10_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            (rng.random_i16() as u16) < 3000
        })
        .expect("find coefficient-zero acceptance seed");
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_i16(); // generic threshold, local_c=1000, /1
    let tile_draw = expected_rng.random_i16() as u16;
    let expected_tile = 0x88 + (tile_draw % 2) as u8;
    let mut rng = MacToolboxRng::new(seed);
    super::super::budget::mac_budget_try_generic_growth(
        &mut city, 40, 40, 0x82, 2, 0, 1000, &mut rng,
    );
    assert_eq!(city.building_at(40, 40), expected_tile);
    assert_eq!(city.zone_at(40, 40).unwrap().raw_bits() & 0xf0, 0xf0);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_rci_class_zero_growth_uses_route_failure_threshold_and_native_export243_shape() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-rci-class-zero-growth.sc2",
        "MacClassZeroGrowth",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, 0x81); // category 1 and rotation-0 scan corner
    city.set_building(40, 40, 0x70); // class 0, growth level 1
    let seed = (1..10_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            (rng.random_i16() as u16) < 4000
        })
        .expect("find a class-0 acceptance seed with failed route");
    let mut expected_rng = MacToolboxRng::new(seed);
    let _acceptance = expected_rng.random_i16();
    let _orientation = expected_rng.random_i16();
    let tile_draw = expected_rng.random_i16() as u16;
    let expected_tile = 0x8a + (tile_draw % 2) as u8;

    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.building_at(40, 40), expected_tile);
    assert_eq!(city.tile_bits_at(40, 40).unwrap().raw_bits() & 0xe0, 0xe0);
    assert_eq!(city.zone_at(40, 40).unwrap().raw_bits() & 0xf0, 0xf0);
    for (x, y) in [(39, 39), (41, 39), (41, 41), (39, 41)] {
        assert_eq!(city.building_at(x, y), 0);
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits() & 0xf0, 0);
    }
    assert_eq!(city.mac_state_misc_u32_at(0x05f0 + 4), Some(1));
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_export243_replaces_occupied_tiles_and_updates_xbld_census() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-export243-occupied-footprint.sc2",
        "MacExport243OccupiedFootprint",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y) = (40, 40);

    // The accepted class-0 candidate occupies the anchor. The other three
    // cells in level-2/orientation-1's native 2x2 footprint already contain a
    // different XBLD, with a matching census count and category-1 zones.
    city.set_zone(x, y, 0x11);
    city.set_building(x, y, 0x8c);
    for (tx, ty) in [(x + 1, y), (x + 1, y - 1), (x, y - 1)] {
        city.set_zone(tx, ty, 0x01);
        city.set_building(tx, ty, 0x10);
    }
    assert_eq!(city.mac_xbld_tile_count(0x8c), 1);
    assert_eq!(city.mac_xbld_tile_count(0x10), 3);
    assert_eq!(city.mac_xbld_tile_count(0x8a), 0);

    let mut expected_rng = MacToolboxRng::new(0x243);
    let expected_tiles: Vec<u8> = (0..4)
        .map(|_| 0x8a + (expected_rng.random_i16() as u16 % 2) as u8)
        .collect();
    let mut rng = MacToolboxRng::new(0x243);
    mac_budget_dispatch_class_zero_footprint(&mut city, x, y, 2, true, &mut rng);

    for ((tx, ty), expected_tile) in [(x, y), (x + 1, y), (x + 1, y - 1), (x, y - 1)]
        .into_iter()
        .zip(expected_tiles.iter().copied())
    {
        assert_eq!(city.building_at(tx, ty), expected_tile);
        assert_eq!(city.zone_at(tx, ty).unwrap().raw_bits(), 0xf1);
        assert_eq!(city.tile_bits_at(tx, ty).unwrap().raw_bits(), 0xe0);
    }
    assert_eq!(city.mac_xbld_tile_count(0x8c), 0);
    assert_eq!(city.mac_xbld_tile_count(0x10), 0);
    assert_eq!(
        city.mac_xbld_tile_count(0x8a),
        expected_tiles.iter().filter(|&&tile| tile == 0x8a).count() as u16
    );
    assert_eq!(
        city.mac_xbld_tile_count(0x8b),
        expected_tiles.iter().filter(|&&tile| tile == 0x8b).count() as u16
    );
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_rci_export243_footprints_keep_native_nested_draw_counts() {
    for (level, orientation, expected_draws) in [
        (1, false, 1),
        (2, false, 1),
        (2, true, 4),
        (3, false, 1),
        (3, true, 1),
        (4, false, 1),
        (4, true, 10),
    ] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-export243-draw-count.sc2",
            "MacExport243Draws",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.saved_rotation = 1;
        city.set_view_rotation(0);
        let mut expected_rng = MacToolboxRng::new(0x243);
        for _ in 0..expected_draws {
            expected_rng.random_i16();
        }
        let center_draw = if level == 4 && orientation {
            let mut center_rng = MacToolboxRng::new(0x243);
            for _ in 0..8 {
                center_rng.random_i16();
            }
            Some(center_rng.random_i16() as u16 & 3)
        } else {
            None
        };
        let mut rng = MacToolboxRng::new(0x243);

        mac_budget_dispatch_class_zero_footprint(&mut city, 40, 40, level, orientation, &mut rng);

        let mut expected_footprint = std::collections::BTreeSet::new();
        match (level, orientation) {
            (1, _) => {
                expected_footprint.insert((40, 40));
            }
            (2, false) => {
                for x in 40..=41 {
                    for y in 39..=40 {
                        expected_footprint.insert((x, y));
                    }
                }
            }
            (2, true) => {
                for (x, y) in [(40, 40), (41, 40), (41, 39), (40, 39)] {
                    expected_footprint.insert((x, y));
                }
            }
            (3, false) => {
                for x in 40..=41 {
                    for y in 39..=40 {
                        expected_footprint.insert((x, y));
                    }
                }
            }
            (3, true) => {
                for x in 40..=41 {
                    for y in 39..=40 {
                        expected_footprint.insert((x, y));
                    }
                }
            }
            (4, false) => {
                for x in 40..=42 {
                    for y in 38..=40 {
                        expected_footprint.insert((x, y));
                    }
                }
            }
            (4, true) => {
                for (dx, dy) in [
                    (0isize, 0isize),
                    (1, 0),
                    (2, 0),
                    (2, -1),
                    (2, -2),
                    (1, -2),
                    (0, -2),
                    (0, -1),
                ] {
                    expected_footprint.insert(((40isize + dx) as usize, (40isize + dy) as usize));
                }
                // The ninth word chooses the center anchor; coefficient 3
                // stamps a 2x2 square and the tenth word picks its tile.
                let draw = center_draw.unwrap();
                let center_x = 40 + usize::from(draw & 1);
                let center_y = 40 - usize::from(draw / 2);
                for x in center_x..=center_x + 1 {
                    for y in center_y - 1..=center_y {
                        expected_footprint.insert((x, y));
                    }
                }
            }
            _ => unreachable!(),
        }
        let observed_footprint: std::collections::BTreeSet<_> = (35..=49)
            .flat_map(|x| (34..=46).map(move |y| (x, y)))
            .filter(|&(x, y)| city.building_at(x, y) != 0)
            .collect();
        assert_eq!(
            observed_footprint, expected_footprint,
            "level={level}, orientation={orientation}"
        );
        let expected_tile_families: &[(u8, u8)] = match (level, orientation) {
            (1, _) | (2, true) => &[(0x8a, 0x8b)],
            (2, false) | (3, true) => &[(0xaa, 0xab)],
            (3, false) => &[(0xac, 0xad)],
            (4, false) => &[(0xc4, 0xc5)],
            (4, true) => &[(0x8a, 0x8b), (0xac, 0xad)],
            _ => unreachable!(),
        };
        assert!(
            observed_footprint.iter().all(|&(x, y)| {
                let tile = city.building_at(x, y);
                expected_tile_families
                    .iter()
                    .any(|&(first, last)| (first..=last).contains(&tile))
            }),
            "level={level}, orientation={orientation}: native selector-4 tile bands"
        );

        assert_eq!(
            rng.seed(),
            expected_rng.seed(),
            "level={level}, orientation={orientation}"
        );
    }
}

#[test]
fn mac_export242_square_bounds_match_native_edge_matrix() {
    for (coefficient, extent, valid_max) in [(2, 1, 125), (3, 1, 125), (4, 2, 124)] {
        for (x, y, accepted) in [
            (1, 40, false),
            (2, 40, true),
            (valid_max, valid_max, true),
            (valid_max + 1, valid_max, false),
            (valid_max, valid_max + 1, false),
        ] {
            let mut city = City::new_blank(
                "/tmp/sc2000-mac-export242-edge-bounds.sc2",
                "MacExport242EdgeBounds",
            )
            .expect("blank city");
            city.kind = CityFileKind::MacCity;
            city.set_zone(x, y, 0x01);
            city.set_building(x, y, 0x70);
            let mut expected_rng = MacToolboxRng::new(0x242);
            expected_rng.random_i16();
            let mut rng = MacToolboxRng::new(0x242);

            super::super::budget::mac_budget_place_growth_pattern(
                &mut city,
                x,
                y,
                coefficient,
                4,
                &mut rng,
            );

            let actual: std::collections::BTreeSet<_> = (0..CITY_WIDTH)
                .flat_map(|tx| (0..CITY_HEIGHT).map(move |ty| (tx, ty)))
                .filter(|&(tx, ty)| city.building_at(tx, ty) >= 0x8a)
                .collect();
            let expected: std::collections::BTreeSet<_> = if accepted {
                (x..=x + extent)
                    .flat_map(|tx| (y - extent..=y).map(move |ty| (tx, ty)))
                    .collect()
            } else {
                std::collections::BTreeSet::new()
            };
            assert_eq!(actual, expected, "coefficient={coefficient}, ({x},{y})");
            assert_eq!(rng.seed(), expected_rng.seed());
            if !accepted {
                assert_eq!(city.building_at(x, y), 0x70);
            }
        }
    }
}

#[test]
fn mac_export242_square_corner_markers_follow_native_rotation_table() {
    let marker_rows = [
        [0x10, 0x20, 0x40, 0x80],
        [0x20, 0x40, 0x80, 0x10],
        [0x40, 0x80, 0x10, 0x20],
        [0x80, 0x10, 0x20, 0x40],
    ];
    for (coefficient, extent) in [(2, 1), (3, 1), (4, 2)] {
        for (rotation, markers) in marker_rows.into_iter().enumerate() {
            let mut city = City::new_blank(
                "/tmp/sc2000-mac-export242-corner-rotation.sc2",
                "MacExport242CornerRotation",
            )
            .expect("blank city");
            city.kind = CityFileKind::MacCity;
            city.saved_rotation = rotation as u8;
            let corners = [
                (40, 40 - extent, 0x01),
                (40 + extent, 40 - extent, 0x02),
                (40 + extent, 40, 0x03),
                (40, 40, 0x04),
            ];
            for &(x, y, zone) in &corners {
                city.set_zone(x, y, zone);
            }
            let mut rng = MacToolboxRng::new(0x242);

            super::super::budget::mac_budget_place_growth_pattern(
                &mut city,
                40,
                40,
                coefficient,
                4,
                &mut rng,
            );

            for ((x, y, low_zone), marker) in corners.into_iter().zip(markers) {
                assert_eq!(
                    city.zone_at(x, y).unwrap().raw_bits(),
                    low_zone | marker,
                    "coefficient={coefficient}, rotation={rotation}, ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn mac_export243_oriented_growth_consumes_off_map_draws_without_stamping_edges() {
    for (growth_level, valid_cells, total_draws) in [(2, 2, 4), (4, 3, 10)] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-export243-oriented-edge.sc2",
            "MacExport243OrientedEdge",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        let seed = (1..u32::from(u16::MAX))
            .find(|seed| {
                let mut probe = MacToolboxRng::new(*seed);
                let draws: Vec<_> = (0..total_draws)
                    .map(|_| probe.random_i16() as u16 & 1)
                    .collect();
                if growth_level == 2 {
                    draws[0] != draws[3] && draws[1] != draws[2]
                } else {
                    draws[0] != draws[7] && draws[1] != draws[5] && draws[2] != draws[4]
                }
            })
            .expect("seed whose clipped calls would visibly overwrite edge tiles");
        let mut expected_rng = MacToolboxRng::new(seed);
        let expected_tiles: Vec<u8> = (0..valid_cells)
            .map(|_| 0x8a + (expected_rng.random_i16() as u16 % 2) as u8)
            .collect();
        for _ in valid_cells..total_draws {
            expected_rng.random_i16();
        }
        let mut rng = MacToolboxRng::new(seed);

        super::super::budget::mac_budget_dispatch_class_zero_footprint(
            &mut city,
            40,
            0,
            growth_level,
            true,
            &mut rng,
        );

        for (dx, expected_tile) in expected_tiles.into_iter().enumerate() {
            assert_eq!(city.building_at(40 + dx, 0), expected_tile);
            assert_eq!(city.zone_at(40 + dx, 0).unwrap().raw_bits() & 0xf0, 0xf0);
        }
        assert!((40..=44).all(|x| city.building_at(x, 1) == 0));
        assert_eq!(rng.seed(), expected_rng.seed());
    }
}

#[test]
fn mac_export243_edge_matrix_matches_native_growth_to_placetile_outputs() {
    const NATIVE_EDGE_MATRIX: &str =
        include_str!("../../../tests/fixtures/simulation/mac/mac_export243_edge_rotation.csv");

    for (case_index, row) in NATIVE_EDGE_MATRIX.lines().skip(1).enumerate() {
        let mut fields = Vec::new();
        let mut start = 0;
        let mut quoted = false;
        for (index, byte) in row.bytes().enumerate() {
            match byte {
                b'"' => quoted = !quoted,
                b',' if !quoted => {
                    fields.push(&row[start..index]);
                    start = index + 1;
                }
                _ => {}
            }
        }
        fields.push(&row[start..]);
        assert_eq!(
            fields.len(),
            19,
            "malformed native fixture row {case_index}"
        );
        let x = fields[0].parse::<usize>().expect("native X");
        let y = fields[1].parse::<usize>().expect("native Y");
        let saved_rotation = fields[2].parse::<u8>().expect("native saved rotation");
        let level = fields[3].parse::<u16>().expect("native growth level");
        let orientation = fields[4].parse::<u8>().expect("native orientation") != 0;
        assert_eq!(fields[5], "0", "fixture uses zero placement draws");
        assert!(
            fields[6].is_empty() || fields[6] == "0",
            "fixture uses zero or no center draw"
        );

        let mut expected = vec![(0, 0, 0); CITY_WIDTH * CITY_HEIGHT];
        expected[x * CITY_HEIGHT + y] = (0x70, 0x01, 0x00);
        let details = fields[12].trim_matches('"');
        for cell in details.split(';').filter(|cell| !cell.is_empty()) {
            let (coordinates, rest) = cell.split_once(':').expect("native cell coordinates");
            let (cell_x, cell_y) = coordinates.split_once(',').expect("native cell X/Y");
            let (tile, rest) = rest.split_once(':').expect("native tile/zone");
            let (zone, bits) = rest.split_once(':').expect("native zone/XBIT");
            let cell_x = cell_x.parse::<usize>().expect("native cell X");
            let cell_y = cell_y.parse::<usize>().expect("native cell Y");
            expected[cell_x * CITY_HEIGHT + cell_y] = (
                u8::from_str_radix(tile, 16).expect("native XBLD"),
                u8::from_str_radix(zone, 16).expect("native XZON"),
                u8::from_str_radix(bits, 16).expect("native XBIT"),
            );
        }

        let mut city = City::new_blank(
            "/tmp/sc2000-mac-export243-native-edge-matrix.sc2",
            "NativeEdgeMatrix",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.saved_rotation = saved_rotation;
        city.set_building(x, y, 0x70);
        city.set_zone(x, y, 0x01);
        city.set_tile_bits(x, y, TileBits::new(0x00));
        let mut rng = MacToolboxRng::new(0);
        super::super::budget::mac_budget_dispatch_class_zero_footprint(
            &mut city,
            x,
            y,
            level,
            orientation,
            &mut rng,
        );

        for cell_x in 0..CITY_WIDTH {
            for cell_y in 0..CITY_HEIGHT {
                let (expected_tile, expected_zone, expected_bits) =
                    expected[cell_x * CITY_HEIGHT + cell_y];
                assert_eq!(
                    city.building_at(cell_x, cell_y),
                    expected_tile,
                    "native XBLD mismatch in case {case_index} at ({cell_x},{cell_y})"
                );
                assert_eq!(
                    city.zone_at(cell_x, cell_y).unwrap().raw_bits(),
                    expected_zone,
                    "native XZON mismatch in case {case_index} at ({cell_x},{cell_y})"
                );
                assert_eq!(
                    city.tile_bits_at(cell_x, cell_y).unwrap().raw_bits(),
                    expected_bits,
                    "native XBIT mismatch in case {case_index} at ({cell_x},{cell_y})"
                );
            }
        }
    }
}

#[test]
fn mac_special_zone_water_ray_helper_stamps_anchor_and_four_cell_coastal_strip() {
    let mut city = City::new_blank("/tmp/sc2000-mac-special-water-ray.sc2", "SpecialWaterRay")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y) = (40, 40);
    city.set_tile_bits(x, y, TileBits::new(0x40)); // powered gate at the anchor
    city.set_zone(x, y, 0x08);
    for ray_y in y + 1..=y + 5 {
        city.set_tile_bits(x, ray_y, TileBits::new(0x04)); // XBIT water bit
        city.set_zone(x, ray_y, 0x08);
    }
    city.set_altitude(x, y + 5, 4, 6); // endpoint water is at least land + 2

    assert_eq!(
        mac_budget_try_place_zone_water_ray(&mut city, x, y, 8),
        MacSpecialTileAttempt::Placed
    );
    assert_eq!(city.building_at(x, y), 0xe0);
    assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0xf8);
    assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits(), 0xe0);
    assert_eq!(city.mac_xbld_tile_count(0xe0), 1);
    for ray_y in y + 1..=y + 4 {
        assert_eq!(city.building_at(x, ray_y), 0xdf);
        assert_eq!(city.zone_at(x, ray_y).unwrap().raw_bits(), 0xf8);
        assert_eq!(city.tile_bits_at(x, ray_y).unwrap().raw_bits(), 0x06);
    }
    assert_eq!(city.building_at(x, y + 5), 0);
    assert_eq!(city.mac_xbld_tile_count(0xdf), 4);

    // A too-shallow endpoint refuses before any anchor/strip writes.
    let mut shallow = City::new_blank(
        "/tmp/sc2000-mac-special-water-ray-shallow.sc2",
        "ShallowRay",
    )
    .expect("blank city");
    shallow.kind = CityFileKind::MacCity;
    shallow.set_tile_bits(x, y, TileBits::new(0x40));
    shallow.set_zone(x, y, 0x08);
    for ray_y in y + 1..=y + 5 {
        shallow.set_tile_bits(x, ray_y, TileBits::new(0x04));
        shallow.set_zone(x, ray_y, 0x08);
    }
    shallow.set_altitude(x, y + 5, 4, 5);
    assert_eq!(
        mac_budget_try_place_zone_water_ray(&mut shallow, x, y, 8),
        MacSpecialTileAttempt::Refused
    );
    assert_eq!(shallow.building_at(x, y), 0);
    assert_eq!(shallow.mac_xbld_tile_count(0xe0), 0);

    // On big-endian 68k, ffffe0c7 aliases the low byte of the saved rotation
    // WORD at ffffe0c6. Rotation parity flips the horizontal strip marker.
    let mut rotated = City::new_blank(
        "/tmp/sc2000-mac-special-water-ray-rotation.sc2",
        "RotatedRay",
    )
    .expect("blank city");
    rotated.kind = CityFileKind::MacCity;
    rotated.saved_rotation = 1;
    rotated.set_view_rotation(0);
    rotated.set_tile_bits(x, y, TileBits::new(0x40));
    rotated.set_zone(x, y, 8);
    rotated.set_tile_bits(x, y + 1, TileBits::new(0));
    rotated.set_tile_bits(x, y - 1, TileBits::new(0));
    rotated.set_tile_bits(x - 1, y, TileBits::new(0));
    for ray_x in x + 1..=x + 5 {
        rotated.set_tile_bits(ray_x, y, TileBits::new(0x04));
        rotated.set_zone(ray_x, y, 8);
    }
    rotated.set_altitude(x + 5, y, 4, 6);
    let mut unrotated = rotated.clone();
    unrotated.saved_rotation = 0;
    unrotated.set_view_rotation(1);
    assert_eq!(
        mac_budget_try_place_zone_water_ray(&mut unrotated, x, y, 8),
        MacSpecialTileAttempt::Placed
    );
    assert_eq!(unrotated.tile_bits_at(x + 1, y).unwrap().raw_bits(), 0x04);
    assert_eq!(
        mac_budget_try_place_zone_water_ray(&mut rotated, x, y, 8),
        MacSpecialTileAttempt::Placed
    );
    assert_eq!(rotated.tile_bits_at(x + 1, y).unwrap().raw_bits(), 0x06);
}

#[test]
fn mac_monthly_scan_dispatches_zone_seven_e0_coastal_ray() {
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-seven-e0-ray.sc2", "ZoneSevenRay")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y) = (40, 40); // one candidate in residue partition (0, 0)
    city.set_zone(x, y, 7);
    city.set_mac_state_misc_u32_at(915 * 4, 4); // event selector 4 chooses e0 at zero census
    for water_y in y + 1..=y + 5 {
        city.set_tile_bits(x, water_y, TileBits::new(0x04));
        city.set_zone(x, water_y, 7);
    }
    city.set_altitude(x, y + 5, 4, 6);

    let seed = (1..100_000)
        .find(|&seed| MacToolboxRng::new(seed).random_u16() & 3 == 0)
        .expect("accepted zone-seven selector seed");
    let mut rng = MacToolboxRng::new(seed);
    let mut expected_rng = MacToolboxRng::new(seed);
    // The scan later visits the newly-written y+4 continuation cell as a
    // second zone-7 candidate; it consumes its gate draw, then refuses because
    // the next five-cell ray is not all water.
    expected_rng.random_i16();
    expected_rng.random_i16();

    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(rng.seed(), expected_rng.seed());
    assert_eq!(city.building_at(x, y), 0xe0);
    for water_y in y + 1..=y + 4 {
        assert_eq!(city.building_at(x, water_y), 0xdf);
    }
}

#[test]
fn mac_phase_scan_spawns_marina_kind_nine_actor_with_native_xy_byte_order() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-marina-kind-nine-spawn.sc2",
        "MacMarinaSpawn",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;

    // XBLD census is saved Mac state, and its f8 slot must clear the /9 gate.
    for index in 0..9 {
        city.set_building(20 + index * 4, 20, 0xf8);
    }
    city.set_tile_bits(20, 20, TileBits::new(0x40));
    city.set_tile_bits(19, 20, TileBits::new(0x04));
    city.set_mac_object_slot_record(
        1,
        Win95OverlayRecord {
            target_x: 0xf8,
            target_y: 0x88,
            saved_a: 0xdd,
            ..Win95OverlayRecord::default()
        },
    );

    let mut rng = MacToolboxRng::new(0x1234_5678);
    let seed_before = rng.seed();
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(
        rng.seed(),
        seed_before,
        "Mac app LFSR gates do not use QD RNG"
    );
    assert_eq!(city.mac_marina_actor_counter, 1);
    assert_eq!(city.text_overlay_at(19, 20), Some(0xca));
    let record = city.traffic_overlays().unwrap()[1];
    assert_eq!(record.kind, 9);
    assert_eq!(record.lane, 0);
    assert_eq!(record.state, 0);
    assert_eq!((record.tile_x, record.tile_y), (19, 20)); // Native bytes +3/+4 are X/Y.
    assert_eq!((record.sub_x, record.sub_y), (4, 4));
    assert_eq!((record.target_x, record.target_y), (0xf8, 0x88));
    assert_eq!(record.saved_a, 0, "native constructor writes prior XTXT");
}

#[test]
fn mac_zone_nine_actor_proxy_can_mask_later_arcology_age_update() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-zone-nine-arcology-proxy.sc2",
        "ZoneNineArcologyProxy",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;

    // The zone-9 alternate branch runs in residue phase 0. Selector 0 scans
    // first-coordinate 2 and may place its proxy over a qualifying arcology
    // at (2, 127), which is visited in phase 11.
    city.set_zone(40, 40, 9);
    city.set_building(40, 40, 0xe0);
    city.set_terrain(2, 127, 0x10);
    city.set_zone(2, 127, 0x80);
    city.set_building(2, 127, 0xfb);
    city.set_text_overlay(2, 127, 0x33);
    assert!(city.set_microsim_tile_id(0, 0xfb));
    assert!(city.set_microsim_stat1(0, 7));

    let seed = (1..1_000_000)
        .find(|&seed| {
            let mut rng = MacToolboxRng::new(seed);
            let first = rng.random_i16() as u16;
            let second = rng.random_i16() as u16;
            let edge = rng.random_i16() as u16 & 3;
            first & 3 != 0 && second & 3 == 0 && edge == 0
        })
        .expect("find a zone-9 alternate branch with selector-0 selection");
    let mut rng = MacToolboxRng::new(seed);
    for phase in 0..16 {
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, phase / 4, phase % 4, &mut rng);
    }

    assert_eq!(city.text_overlay_at(2, 127), Some(0xca));
    assert_eq!(city.traffic_overlays().unwrap()[1].kind, 3);
    assert_eq!(
        (
            city.traffic_overlays().unwrap()[1].tile_x,
            city.traffic_overlays().unwrap()[1].tile_y,
        ),
        (2, 127),
        "native +3/+4 preserve the first/second map-plane coordinates"
    );
    assert_eq!(
        city.microsim_at(0).map(|record| record.stat1),
        Some(7),
        "phase 11 sees the actor proxy and skips the native XMIC age write"
    );
}

#[test]
fn mac_transit_power_gate_checks_current_and_cardinal_cells_only() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-route-power.sc2", "MacRoutePower").expect("blank city");
    assert!(!mac_transit_has_powered_adjacency(&city, 32, 32));
    city.set_tile_bits(32, 32, TileBits::new(0x40));
    assert!(mac_transit_has_powered_adjacency(&city, 32, 32));
    city.set_tile_bits(32, 32, TileBits::new(0));
    city.set_tile_bits(33, 32, TileBits::new(0x40));
    assert!(mac_transit_has_powered_adjacency(&city, 32, 32));
    city.set_tile_bits(33, 32, TileBits::new(0));
    city.set_tile_bits(34, 32, TileBits::new(0x40));
    assert!(!mac_transit_has_powered_adjacency(&city, 32, 32));
    city.set_tile_bits(0, 0, TileBits::new(0x40));
    assert!(mac_transit_has_powered_adjacency(&city, 0, 0));
    assert!(!mac_transit_has_powered_adjacency(&city, CITY_WIDTH, 0));
}

#[test]
fn mac_transit_power_gate_preserves_native_low_edge_neighbor_guard() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-route-power-low-edge.sc2",
        "MacRoutePowerEdge",
    )
    .expect("blank city");

    city.set_tile_bits(0, 32, TileBits::new(0x40));
    assert!(mac_transit_has_powered_adjacency(&city, 0, 32));
    assert!(
        !mac_transit_has_powered_adjacency(&city, 1, 32),
        "native x-1 probe skips XBIT row 0 when candidate x is 1"
    );
    city.set_tile_bits(0, 32, TileBits::new(0));
    city.set_tile_bits(1, 32, TileBits::new(0x40));
    assert!(
        mac_transit_has_powered_adjacency(&city, 0, 32),
        "native x+1 probe accepts row 1 from candidate x 0"
    );

    city.set_tile_bits(32, 0, TileBits::new(0x40));
    assert!(
        !mac_transit_has_powered_adjacency(&city, 32, 1),
        "native y-1 probe skips byte 0 when candidate y is 1"
    );
    city.set_tile_bits(32, 0, TileBits::new(0));
    city.set_tile_bits(32, 1, TileBits::new(0x40));
    assert!(
        mac_transit_has_powered_adjacency(&city, 32, 0),
        "native y+1 probe accepts byte 1 from candidate y 0"
    );
}

#[test]
fn city_map_cells_prioritize_zone_and_transport_categories() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 0..4 {
        for y in 0..4 {
            city.set_zone(x, y, 1);
        }
    }
    assert_eq!(city_map_cell_color(&city, 0, 0), [0.15, 0.68, 0.18, 1.0]);
    for x in 0..4 {
        for y in 0..4 {
            city.set_building(x, y, 29);
        }
    }
    assert_eq!(city_map_cell_color(&city, 0, 0), [0.22, 0.22, 0.22, 1.0]);
}

#[test]
fn every_city_information_dialog_builds_a_visible_overlay() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    for dialog in [
        CityInfoDialog::Map,
        CityInfoDialog::Population,
        CityInfoDialog::Neighbors,
        CityInfoDialog::Graphs,
        CityInfoDialog::Industry,
        CityInfoDialog::Budget,
        CityInfoDialog::Ordinances,
        CityInfoDialog::Newspaper(NewspaperEdition::Courier),
        CityInfoDialog::Scenario,
        CityInfoDialog::Help,
        CityInfoDialog::Query { x: 10, y: 10 },
    ] {
        state.city_info_dialog = Some(dialog);
        let mut solids = Vec::new();
        state.push_city_info_dialog(None, (1024, 768), 1.0, &mut solids, &mut Vec::new());
        assert!(solids.len() > 10, "{dialog:?} should render content");
    }
}

#[test]
fn growth_tick_vacant_trip_accepts_cardinal_neighbor_power_cross() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    // North cardinal only — origin itself unpowered; energized alone must not pass.
    set_saved_xbit_growth_power_for_test(&mut power_maps, 20, 19, true);
    power_maps.energized_tiles[20 * CITY_WIDTH + 20] = true;
    assert!(growth_powered_cross_for_engine(&power_maps, 20, 20));
    assert!(!power_maps.powered_tiles[20 * CITY_WIDTH + 20]);

    let mut served = vec![false; CITY_TILES];
    let stats = trip_generator::simulate_growth_lattice_trips(
        &mut city,
        &mut served,
        [(20, 20)],
        |x, y| growth_powered_cross_for_engine(&power_maps, x, y),
        true,
    );
    assert_eq!(stats.trips_attempted, 1);
    assert!(
        stats.trips_succeeded > 0,
        "vacant growth trip must accept FUN_00436D00 cardinal neighbor power"
    );

    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 19, 19, true); // diagonal NW
    served.fill(false);
    let diagonal = trip_generator::simulate_growth_lattice_trips(
        &mut city,
        &mut served,
        [(20, 20)],
        |x, y| growth_powered_cross_for_engine(&power_maps, x, y),
        true,
    );
    assert_eq!(
        diagonal.trips_attempted, 0,
        "diagonal-only power must not satisfy FUN_00436D00"
    );
}

#[test]
fn center_tile_at_viewport_places_tile_on_viewport_center() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.zoom = 3.25;
    state.camera = Vec2::new(75.0, -42.0);
    state.camera_rotation = 2;
    let window_size = (1800, 1200);
    let pixel_scale = 2.0;
    let target = (37, 82);

    state.center_tile_at_viewport(target.0, target.1, window_size, pixel_scale);

    let (sx, sy) = rotate_tile_coords(target.0, target.1, state.camera_rotation);
    let screen = viewport_origin(window_size)
        + state.camera
        + projected_tile_units(sx, sy, state.city.altitude_at(target.0, target.1) as f32)
            * render_zoom(state.zoom, pixel_scale);

    assert!((screen - viewport_center(window_size)).length() < 0.001);
}

#[test]
fn rotation_preserves_fractional_viewport_center_on_land_water_and_underground() {
    let window_size = (1800, 1200);
    for (land, water, underground) in [(8, 8, false), (3, 4, false), (8, 8, true)] {
        let mut city = City::new_blank("/tmp/rotation-center.sc2", "RotationCenter").unwrap();
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                city.set_altitude(x, y, land, water);
            }
        }
        for pixel_scale in [1.0, 2.0] {
            for rotation in 0..4 {
                for delta in [-1, 1] {
                    let mut state = test_state(city.clone());
                    state.show_underground = underground;
                    state.zoom = 3.25;
                    state.city.set_view_rotation(rotation);
                    state.camera_rotation = rotation;
                    let elevation = if underground { land } else { land.max(water) };
                    let point = Vec2::new(37.25, 82.125);
                    let rotated_point = |rotation| {
                        let max = (CITY_WIDTH - 1) as f32;
                        match rotation & 3 {
                            0 => point,
                            1 => Vec2::new(max - point.y, point.x),
                            2 => Vec2::new(max - point.x, max - point.y),
                            _ => Vec2::new(point.y, max - point.x),
                        }
                    };
                    let project = |point: Vec2| {
                        projected_tile_units_f32(point.x, point.y, f32::from(elevation))
                    };
                    state.place_map_units_at_screen(
                        project(rotated_point(rotation)),
                        viewport_center(window_size),
                        window_size,
                        pixel_scale,
                    );
                    let original_camera = state.camera;
                    for _ in 0..4 {
                        state.rotate_view(delta);
                        let screen = viewport_origin(window_size)
                            + state.camera
                            + project(rotated_point(state.camera_rotation))
                                * render_zoom(state.zoom, pixel_scale);
                        assert!(
                            (screen - viewport_center(window_size)).length() < 0.01,
                            "rotation={} delta={delta} pixel_scale={pixel_scale} land={land} water={water} underground={underground}",
                            state.camera_rotation
                        );
                    }
                    assert!((state.camera - original_camera).length() < 0.01);
                }
            }
        }
    }
}

#[test]
fn tile_picking_round_trips_at_non_default_zoom_and_retina_scale() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.zoom = 3.25;
    state.camera = Vec2::new(75.0, -42.0);
    state.camera_rotation = 1;
    let window_size = (1800, 1200);
    let pixel_scale = 2.0;
    let target = (37, 82);
    let (sx, sy) = rotate_tile_coords(target.0, target.1, state.camera_rotation);
    let screen = viewport_origin(window_size)
        + state.camera
        + projected_tile_units(sx, sy, state.city.altitude_at(target.0, target.1) as f32)
            * render_zoom(state.zoom, pixel_scale);

    assert_eq!(
        state.tile_at_screen(screen, window_size, pixel_scale),
        Some(target)
    );
}

#[test]
fn network_drag_path_snaps_to_straight_or_proportional_line() {
    assert_eq!(
        network_drag_path((10, 10), (13, 10)),
        vec![(10, 10), (11, 10), (12, 10), (13, 10)]
    );
    assert_eq!(
        network_drag_path((10, 10), (12, 11)),
        vec![(10, 10), (11, 10), (11, 11), (12, 11)]
    );
    assert_eq!(
        network_drag_path((12, 11), (10, 10)),
        vec![(12, 11), (11, 11), (11, 10), (10, 10)]
    );
}

#[test]
fn placing_rail_reorients_an_existing_sub_to_rail_transition() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 108);
    city.set_underground(20, 20, UNDER_TILE_GREY_BOX_2);
    let mut state = test_state(city);

    assert_eq!(state.place_network_path(&[(20, 19)], NetworkKind::Rail), 1);
    assert_eq!(state.city.building_at(20, 20), 111);
    assert_eq!(
        state.network_neighbor_mask(20, 19, NetworkKind::Rail),
        NETWORK_BOTTOM
    );
}

#[test]
fn subway_connects_to_sub_to_rail_access_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_underground(20, 19, 1);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.underground_at(20, 20), UNDER_TILE_GREY_BOX_2);
    assert_eq!(
        state.underground_neighbor_mask(20, 19, UndergroundKind::Subway),
        NETWORK_BOTTOM
    );
    assert!(underground_tile_connects_to(
        UNDER_TILE_GREY_BOX_2,
        UndergroundKind::Subway,
        NETWORK_TOP
    ));
    assert!(!underground_tile_connects_to(
        UNDER_TILE_GREY_BOX_2,
        UndergroundKind::Pipes,
        NETWORK_TOP
    ));
}

#[test]
fn highway_drag_geometry_dispatch_keeps_pe_and_legacy_mac_paths_separate() {
    assert_eq!(
        highway_drag_path_for_fidelity(EngineFidelity::Win95, (10, 10), (16, 16)),
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
        highway_drag_path_for_fidelity(EngineFidelity::Mac68k, (10, 10), (16, 16)),
        vec![(10, 10), (12, 10), (14, 10), (16, 10)]
    );
}

#[test]
fn highway_drag_alternates_reinforced_bridge_support_segments() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..=11 {
        for x in 10..=15 {
            city.set_terrain(x, y, 0x10);
        }
    }
    let mut state = test_state(city);

    assert_eq!(
        state.place_highway_path(&highway_drag_path((10, 10), (14, 10))),
        3
    );

    for (origin_x, expected) in [(10, 107), (12, 106), (14, 107)] {
        for y in 10..=11 {
            for x in origin_x..=origin_x + 1 {
                assert_eq!(
                    state.city.building_at(x, y),
                    expected,
                    "bridge segment at ({origin_x},10) tile ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn win95_road_endpoint_confirmation_is_separate_from_trip_overlay() {
    let mut city = City::new_blank("/tmp/sc2000-road-neighbor-confirm.sc2", "RoadNeighbor")
        .expect("blank city");
    city.set_city_funds(10_000);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.tool = EditTool::Road;
    let traffic_before = state
        .city
        .traffic_density_slice()
        .map(|values| values.to_vec());

    state.begin_network_drag(125, 30, NetworkKind::Road);
    state.update_network_drag_to((127, 30));
    let outcome = state.release_left_mouse_drag(false);
    assert_eq!(outcome.network, Some((NetworkKind::Road, 3)));
    let candidate = outcome
        .road_neighbor_link
        .expect("eastward terminal lookahead exits the map");
    assert_eq!(candidate.endpoint, (127, 30));
    assert_eq!(candidate.initial_xtxt, 0);
    assert_eq!(state.city.text_overlay_at(127, 30), Some(0xfa));
    assert!(road_neighbor_link_is_available(&state, candidate));

    let funds_after_road = state.city.city_funds();
    assert!(!apply_road_neighbor_link_confirmation(
        &mut state, candidate, false
    ));
    assert_eq!(state.city.city_funds(), funds_after_road);
    assert_eq!(state.city.text_overlay_at(127, 30), Some(0xfa));
    assert!(apply_road_neighbor_link_confirmation(
        &mut state, candidate, true
    ));
    assert_eq!(state.city.city_funds(), funds_after_road - 1_000);
    assert_eq!(state.city.text_overlay_at(127, 30), Some(0xfa));
    assert_eq!(state.city.pe_nonnetwork_neighbor_links, 0);
    assert_eq!(
        state
            .city
            .traffic_density_slice()
            .map(|values| values.to_vec()),
        traffic_before,
        "road confirmation leaves the established XTRF traffic sprites unchanged"
    );

    let mut blocked_city = City::new_blank(
        "/tmp/sc2000-road-neighbor-already-linked.sc2",
        "RoadAlreadyLinked",
    )
    .expect("blank city");
    blocked_city.set_city_funds(10_000);
    blocked_city.set_text_overlay(127, 30, 0xfa);
    let mut blocked = test_state(blocked_city);
    blocked.engine_fidelity = EngineFidelity::Win95;
    blocked.tool = EditTool::Road;
    blocked.begin_network_drag(125, 30, NetworkKind::Road);
    blocked.update_network_drag_to((127, 30));
    let blocked_outcome = blocked.release_left_mouse_drag(false);
    assert!(
        !road_neighbor_link_is_available(
            &blocked,
            blocked_outcome.road_neighbor_link.expect("edge candidate")
        ),
        "a marker that predates the stroke suppresses the paid confirmation"
    );
}

#[test]
fn pipe_and_subway_drag_costs_use_original_per_tile_prices() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.begin_underground_drag(10, 10, UndergroundKind::Pipes);
    state.update_underground_drag_to((13, 10));
    assert_eq!(
        state.underground_drag_cost(),
        Some((UndergroundKind::Pipes, 12))
    );
    state.refresh_status_line();
    assert_eq!(state.query_line, "Pipes $12");

    state.cancel_underground_drag();
    state.begin_underground_drag(20, 20, UndergroundKind::Subway);
    state.update_underground_drag_to((23, 20));
    assert_eq!(
        state.underground_drag_cost(),
        Some((UndergroundKind::Subway, 400))
    );
    state.refresh_status_line();
    assert_eq!(state.query_line, "Subway $400");
}

#[test]
fn selected_path_tool_status_line_reports_original_unit_prices() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    for (tool, expected_prefix) in [
        (EditTool::Road, "Road $10  "),
        (EditTool::Rail, "Rail $25  "),
        (EditTool::Power, "Power Lines $2  "),
        (EditTool::Highway, "Highway $100  "),
        (EditTool::Pipes, "Pipes $3  "),
        (EditTool::Subway, "Subway $100  "),
    ] {
        state.tool = tool;
        state.refresh_status_line();

        assert!(
            state.query_line.starts_with(expected_prefix),
            "{tool:?} status line was {}",
            state.query_line
        );
    }
}

#[test]
fn selected_build_tool_status_line_reports_original_build_prices() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    for (tool, expected_prefix) in [
        (EditTool::Bulldoze, "Bulldoze $1  "),
        (EditTool::RaiseTerrain, "Raise Terrain $25  "),
        (EditTool::LowerTerrain, "Lower Terrain $25  "),
        (EditTool::StretchTerrain, "Stretch Terrain $25  "),
        (EditTool::LevelTerrain, "Level Terrain $25  "),
        (EditTool::SpreadTerrain, "Spread Terrain $25  "),
        (EditTool::Trees, "Trees $3  "),
        (EditTool::Forest, "Forest $3  "),
        (EditTool::Water, "Water $100  "),
        (EditTool::Stream, "Stream $100  "),
        (EditTool::ZoneResidential, "Light Residential $5  "),
        (EditTool::ZoneResidentialDense, "Dense Residential $10  "),
        (EditTool::ZoneCommercial, "Light Commercial $5  "),
        (EditTool::ZoneCommercialDense, "Dense Commercial $10  "),
        (EditTool::ZoneIndustrial, "Light Industrial $5  "),
        (EditTool::ZoneIndustrialDense, "Dense Industrial $10  "),
        (EditTool::Tunnel, "Tunnel $150  "),
        (EditTool::Onramp, "Onramp $25  "),
        (EditTool::BusDepot, "Bus Depot $250  "),
        (EditTool::RailDepot, "Rail Depot $500  "),
        (EditTool::SubStation, "Sub Station $250  "),
        (EditTool::SubToRail, "Sub<-->Rail $250  "),
        (EditTool::WaterPump, "Water Pump $100  "),
        (EditTool::WaterTower, "Water Tower $250  "),
        (EditTool::WaterTreatment, "Treatment $500  "),
        (EditTool::Desalinization, "Desalinization $1,000  "),
        (EditTool::Police, "Police $500  "),
        (EditTool::FireStation, "Fire Station $500  "),
        (EditTool::Hospital, "Hospital $500  "),
        (EditTool::Prison, "Prison $3,000  "),
        (EditTool::School, "School $250  "),
        (EditTool::College, "College $1,000  "),
        (EditTool::Library, "Library $500  "),
        (EditTool::Museum, "Museum $1,000  "),
        (EditTool::SmallPark, "Small Park $20  "),
        (EditTool::BigPark, "Big Park $150  "),
        (EditTool::Zoo, "Zoo $3,000  "),
        (EditTool::Stadium, "Stadium $5,000  "),
        (EditTool::Marina, "Marina $1,000  "),
        (EditTool::PowerPlantCoal, "Coal Power $4,000  "),
        (EditTool::PowerPlantHydro, "Hydro Electric $400  "),
        (EditTool::PowerPlantOil, "Oil Power $6,600  "),
        (EditTool::PowerPlantGas, "Gas Power $2,000  "),
        (EditTool::PowerPlantNuclear, "Nuclear Power $15,000  "),
        (EditTool::PowerPlantWind, "Wind Power $100  "),
        (EditTool::PowerPlantSolar, "Solar Power $1,300  "),
        (EditTool::PowerPlantMicrowave, "Microwave Power $28,000  "),
        (EditTool::PowerPlantFusion, "Fusion Power $40,000  "),
        (EditTool::Seaport, "Seaport $150  "),
        (EditTool::Airport, "Airport $250  "),
        (EditTool::DeZone, "De-zone $5  "),
        (EditTool::MayorHouse, "Mayor's House $0  "),
        (EditTool::CityHall, "City Hall $0  "),
        (EditTool::Statue, "Statue $0  "),
        (EditTool::BraunLlamaDome, "Braun Llama Dome $0  "),
        (EditTool::ArcologyPlymouth, "Plymouth Arco $100,000  "),
        (EditTool::ArcologyForest, "Forest Arco $120,000  "),
        (EditTool::ArcologyDarco, "Darco $150,000  "),
        (EditTool::ArcologyLaunch, "Launch Arco $200,000  "),
    ] {
        state.tool = tool;
        state.refresh_status_line();

        assert!(
            state.query_line.starts_with(expected_prefix),
            "{tool:?} status line was {}",
            state.query_line
        );
    }
}

#[test]
fn query_overlay_111_recalculates_mayors_house_stats() {
    let mut city = City::new_blank("/tmp/sc2000-query-mayor.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1955,
        month: 6,
    });
    let mayor = city.provision_microsim_for_building(243).expect("mayor");
    let mayor_index = usize::from(mayor - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(mayor, MIN_SIM_TEXT_ENTRIES + MICROSIM_STRUCTURE_FIRST as u8);
    city.set_text_overlay(20, 20, mayor);
    city.set_text_overlay(4, 4, QUERY_SPECIFIC_MAYOR_STATS_OVERLAY);
    city.persist_pe_day22_population_leftover(10_000);

    let mut state = test_state(city);
    state.begin_tile_inspect(20, 20);
    assert_eq!(
        state
            .city
            .microsim_at(mayor_index)
            .map(|r| (r.stat0, r.stat3)),
        Some((0, 47)),
        "QuerySpecificItem only refreshes mayors when XTXT is 0x6f, not the first mayor overlay"
    );

    state.begin_tile_inspect(4, 4);
    let approval = state.city.pe_mayor_approval();
    assert!(approval > 0);
    assert_eq!(
        state
            .city
            .microsim_at(mayor_index)
            .map(|r| (r.stat0, r.stat2, r.stat3)),
        Some((1, approval, 46))
    );

    state.begin_tile_inspect(4, 4);
    assert_eq!(
        state
            .city
            .microsim_at(mayor_index)
            .map(|r| (r.stat0, r.stat3)),
        Some((2, 45)),
        "each overlay-111 query ages the Rating countdown"
    );

    state.begin_tile_inspect(20, 20);
    assert_eq!(
        state
            .city
            .microsim_at(mayor_index)
            .map(|r| (r.stat0, r.stat3)),
        Some((2, 45)),
        "querying the mayor house itself (overlay 61) must not age again"
    );
}

#[test]
fn query_transit_buildings_show_citywide_passengers_per_day() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, BUS_DEPOT_BUILDING_ID);
    city.set_building(12, 10, RAIL_STATION_BUILDING_ID);
    city.set_building(14, 10, SUBWAY_STATION_BUILDING_ID);
    let passengers = TransitPassengerStats {
        bus: 1_200,
        rail: 45,
        subway: 9,
    };

    let bus = query_tile_status_line_with_passengers(
        &city,
        10,
        10,
        false,
        passengers,
        pe_power_recalc_seed(&city),
    );
    assert!(bus.starts_with("Bus Depot"), "{bus}");
    assert!(bus.contains("Passengers/day: 1,200"), "{bus}");
    assert!(bus.contains("Bus Stations: 1"), "{bus}");

    let rail = query_tile_status_line_with_passengers(
        &city,
        12,
        10,
        false,
        passengers,
        pe_power_recalc_seed(&city),
    );
    assert!(rail.contains("Passengers/day: 45"), "{rail}");
    assert!(rail.contains("Rail Stations: 1"), "{rail}");

    let subway = query_tile_status_line_with_passengers(
        &city,
        14,
        10,
        false,
        passengers,
        pe_power_recalc_seed(&city),
    );
    assert!(subway.contains("Passengers/day: 9"), "{subway}");
    assert!(subway.contains("Subway Stations: 1"), "{subway}");

    assert!(
        !query_tile_status_line_with_passengers(
            &city,
            11,
            10,
            false,
            passengers,
            pe_power_recalc_seed(&city),
        )
        .contains("Passengers/day"),
        "non-transit tiles should not show passenger totals"
    );

    stamp_complete_building_for_test(&mut city, 30, 30, BUS_DEPOT_BUILDING_ID, 2);
    let complete = query_tile_status_line_with_passengers(
        &city,
        30,
        30,
        false,
        passengers,
        pe_power_recalc_seed(&city),
    );
    assert!(
        complete.contains("Bus Stations: 1"),
        "complete 2x2 bus depot should count as one station, got {complete}"
    );
}

#[test]
fn single_tile_apply_reports_false_when_path_or_zone_tool_places_nothing() {
    for tool in [EditTool::Road, EditTool::Rail, EditTool::Power] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_terrain(20, 20, 0x30);
        let mut state = test_state(city);
        state.tool = tool;

        assert!(
            !state.apply_tool_at_tile(20, 20),
            "{tool:?} reported success on blocked water terrain"
        );
    }

    for tool in [
        EditTool::ZoneResidential,
        EditTool::ZoneCommercialDense,
        EditTool::ZoneIndustrial,
        EditTool::DeZone,
        EditTool::Airport,
        EditTool::Seaport,
    ] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_building(20, 20, 112);
        let mut state = test_state(city);
        state.tool = tool;

        assert!(
            !state.apply_tool_at_tile(20, 20),
            "{tool:?} reported success on an occupied tile"
        );
    }
}

#[test]
fn normal_release_finishes_zone_drag_and_reports_outcome() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    state.begin_zone_drag(10, 10, ZONE_INDUSTRIAL_LIGHT);
    state.update_zone_drag_to((12, 10));

    let outcome = state.release_left_mouse_drag(false);

    assert_eq!(outcome.placed_zones, 3);
    for x in 10..=12 {
        assert_eq!(
            state.city.zone_at(x, 10).unwrap().zone_id(),
            ZONE_INDUSTRIAL_LIGHT
        );
    }
}

#[test]
fn edit_map_raise_dispatches_pe_raise_submode() {
    // PE tool-0 submode word drives Raise; default 0 bills `0x4dc140[0]==1` then
    // FUN_00443270 land++/`0x445b50`. Vacant dry refuses; stamp a small XBLD.
    // Keep land above sea (`BLANK_MAP_BASE_HEIGHT`) so `0x447490` does not water-mark
    // neighbors and poison a later submode-2 raise.
    let mut city = City::new_blank("/tmp/sc2000-raise-sub.sc2", "RaiseSub").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(20, 19, 13, 0);
    city.set_building(20, 20, 4);
    city.set_city_funds(1);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::RaiseTerrain;
    assert_eq!(state.pe_raise_submode, PE_RAISE_SUBMODE_DEFAULT);
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.altitude_at(20, 20), 10);
    assert_eq!(state.city.altitude_at(20, 19), 11); // 445b50 pulled neighbor 13→11
    assert_eq!(state.city.city_funds(), 0);
    state.pe_raise_submode = 2;
    state.city.set_altitude(20, 20, 9, 0);
    state.city.set_altitude(20, 19, 13, 0);
    state.city.set_building(20, 20, 4);
    state.city.set_city_funds(0x19);
    state.pe_raise_ui_wait_4012c1_count = 0;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.altitude_at(20, 20), 10);
    assert_eq!(state.city.altitude_at(20, 19), 11); // 445b50 pulled neighbor 13→11
    // PE `0x443c60` `0x445b50`/`0x447490` waits + Raise hold pair.
    assert!(state.pe_raise_ui_wait_4012c1_count >= 1 + 18 + 2);
    state.pe_raise_submode = 5;
    assert!(state.apply_tool_at_tile(20, 20)); // apply still true; step no-ops
    assert_eq!(state.city.altitude_at(20, 20), 10);
}

#[test]
fn pe_raise_submode_jump_table_matches_tool0_dispatch() {
    // PE Raise `0x442db0` @ `0x442ea3`: `cmp [0x4cc950],4` / `ja`; table `0x443104`.
    assert_eq!(PE_RAISE_SUBMODE_MAX, 4);
    assert_eq!(PE_RAISE_SUBMODE_DEFAULT, 0);
    assert_eq!(PE_RAISE_SUBMODE_ADJUST_TOWARD, 1);
    assert_eq!(
        PE_RAISE_SUBMODE_ARM_VA,
        [0x442eb3, 0x442ec3, 0x442ed8, 0x442ee8, 0x442efa]
    );
    assert_eq!(PE_RAISE_SUBMODE0_FUND_COST, 1);
    assert_eq!(
        pe_lower_444440_mode_cost(PE_RAISE_SUBMODE_DEFAULT),
        PE_RAISE_SUBMODE0_FUND_COST
    );
}

#[test]
fn pe_raise_adjust_toward_dig_reports_447490_waits() {
    // PE dig arm `0x44323e`→`0x444440` runs the queue relaxation and padded `0x447490` refresh.
    let mut city = City::new_blank("/tmp/sc2000-raise-adj-dig.sc2", "RaiseAdjDig").expect("blank");
    city.set_altitude(20, 20, 5, 0);
    city.set_city_funds(0x19);
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(
        pe_raise_adjust_land_toward_443200_with_crt(&mut city, Some(&mut crt), 20, 20, 3),
        Some(458)
    );
    assert_eq!(city.altitude_at(20, 20), 7);
    assert_eq!(city.city_funds(), 0);
}

#[test]
fn pe_raise_adjust_toward_equal_land_reports_zero_waits() {
    // PE `0x443249`: equal land returns 1 with no raise/dig / no `0x447490`.
    let mut city = City::new_blank("/tmp/sc2000-raise-adj-eq.sc2", "RaiseAdjEq").expect("blank");
    city.set_altitude(20, 20, 5, 0);
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(
        pe_raise_adjust_land_toward_443200_with_crt(&mut city, Some(&mut crt), 20, 20, 5),
        Some(0)
    );
    assert_eq!(city.altitude_at(20, 20), 5);
}

#[test]
fn air_crash_clears_pe_airplane_without_touching_ambient() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(city.place_ambient_port_actor(20, 20, XTHG_AIRPLANE, XTHG_DIRECTION_NORTH));
    let ambient_before = city
        .ambient_port_actor_at(20, 20)
        .expect("ambient airplane before crash")
        .1;
    assert_eq!(ambient_before.state, 0, "offline airport-arm spawn state");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_aircrash_tile_with_lfsr(&state.city, &mut preview_lfsr).expect("PE air-crash tile");

    assert!(state.start_disaster(DisasterKind::AirCrash));
    assert_pe_aircrash_actor(&state.city, expected.0, expected.1);
    assert_eq!(state.city.ambient_port_actor_count(XTHG_AIRPLANE), 2);
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x12)
    );
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());

    // Monthly path must not clear the diving airplane; PE state-7 tick finishes it.
    assert_eq!(state.advance_active_disasters(), 0);
    assert!(state.city.pe_aircrash_actor().is_some());

    for _ in 0..20 {
        state.tick_pe_aircrash_actors();
        if state.city.pe_aircrash_actor().is_none() {
            break;
        }
    }
    assert!(
        state.city.pe_aircrash_actor().is_none(),
        "state-7 dive must finish without monthly clear"
    );
    // The airplane morphs into kind-6 debris. This test checks that the unrelated ambient
    // airport-arm actor itself remains byte-for-byte unchanged; explosion cleanup is covered
    // by `pe_explosion_tick_ages_then_clears_xtxt_proxy`.
    assert_eq!(state.city.ambient_port_actor_count(XTHG_AIRPLANE), 1);
    let (_, ambient) = state
        .city
        .ambient_port_actor_at(20, 20)
        .expect("ambient airplane");
    assert_eq!(ambient, ambient_before);
}

#[test]
fn pe_xthg_advance_subtile_leaves_exact_0x10() {
    // PE FUN_004516F0: cmp/jle → carry only when strict > 0x10.
    assert_eq!(pe_xthg_advance_subtile(16), (16, 0));
    assert_eq!(pe_xthg_advance_subtile(17), (1, 1));
    assert_eq!(pe_xthg_advance_subtile(-1), (15, -1));
    assert_eq!(pe_xthg_advance_subtile(0), (0, 0));
}

#[test]
fn pe_xthg_move_restores_xtxt_unconditionally_on_cross() {
    // PE FUN_004516F0 @ 0x451820 writes +A back to the old tile without a proxy-match gate.
    let mut city = City::new_blank("/tmp/sc2000-xthg-xtxt.sc2", "XthgXtxt").expect("blank");
    let slot = city.place_pe_aircrash_actor(40, 40).expect("aircrash");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].saved_a = 0x33;
        overlays[slot].sub_x = 15;
        overlays[slot].lane = 2;
        overlays[slot].height = 0x10;
    }
    // Overwrite occupancy with a non-proxy byte so a gated restore would no-op.
    city.set_text_overlay(40, 40, 0x55);
    assert!(pe_aircrash_move_step(&mut city, slot, 2));
    assert_eq!(city.text_overlay_at(40, 40), Some(0x33));
    assert_eq!(city.text_overlay_at(41, 40), xthg_proxy_for_slot(slot));
    let rec = city.traffic_overlays().unwrap()[slot];
    assert_eq!((rec.tile_x, rec.tile_y), (41, 40));
    assert_eq!(rec.saved_a, 0); // dest was 0 before paint
}

#[test]
fn pe_xthg_move_heavy_dest_restores_old_and_zeros_dest_xtxt() {
    // PE FUN_004516F0: heavy dest commits tile then re-adds carry; OOB next
    // candidate → FUN_00401528 zeros the committed heavy tile.
    let mut city = City::new_blank("/tmp/sc2000-xthg-heavy.sc2", "XthgHeavy").expect("blank");
    let slot = city.place_pe_aircrash_actor(126, 40).expect("aircrash");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].saved_a = 0x33;
        overlays[slot].sub_x = 15;
        overlays[slot].lane = 2;
        overlays[slot].height = 0x10;
    }
    city.set_text_overlay(127, 40, 0xc9);
    assert!(pe_aircrash_move_step(&mut city, slot, 2));
    assert_eq!(city.text_overlay_at(126, 40), Some(0x33));
    assert_eq!(city.text_overlay_at(127, 40), Some(0));
    let rec = city.traffic_overlays().unwrap()[slot];
    assert_eq!(rec.kind, 0);
    assert_eq!((rec.tile_x, rec.tile_y), (127, 40));
}

#[test]
fn pe_xthg_move_skips_heavy_xtxt_to_clear_neighbor() {
    // PE @ 0x45185c: walk past XTXT >= 0xC9 along the tile delta onto a clear cell.
    let mut city = City::new_blank("/tmp/sc2000-xthg-skip.sc2", "XthgSkip").expect("blank");
    let slot = city.place_pe_aircrash_actor(40, 40).expect("aircrash");
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].saved_a = 0x33;
        overlays[slot].sub_x = 15;
        overlays[slot].lane = 2;
        overlays[slot].height = 0x10;
    }
    city.set_text_overlay(41, 40, 0xc9);
    // (42,40) stays clear (0).
    assert!(pe_aircrash_move_step(&mut city, slot, 2));
    assert_eq!(city.text_overlay_at(40, 40), Some(0x33));
    assert_eq!(city.text_overlay_at(41, 40), Some(0xc9)); // heavy left alone
    assert_eq!(city.text_overlay_at(42, 40), Some(proxy));
    let rec = city.traffic_overlays().unwrap()[slot];
    assert_ne!(rec.kind, 0);
    assert_eq!((rec.tile_x, rec.tile_y), (42, 40));
    assert_eq!(rec.saved_a, 0);
}

#[test]
fn pe_xthg_despawn_clears_kind_only_and_zeros_xtxt() {
    // PE FUN_004507D0 zeros kind + XTXT; height/saved_b remain for linked tails.
    let mut city = City::new_blank("/tmp/sc2000-xthg-despawn.sc2", "Despawn").expect("blank");
    let slot = city
        .place_pe_tornado_actor(40, 40, 2, 0x0c, 0, 0, 0x33)
        .expect("tornado");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].saved_b = 7;
        overlays[slot].height = 0x0c;
    }
    pe_tornado_despawn(&mut city, slot, 40, 40);
    let rec = city.traffic_overlays().unwrap()[slot];
    assert_eq!(rec.kind, 0);
    assert_eq!(rec.height, 0x0c);
    assert_eq!(rec.saved_b, 7);
    assert_eq!(city.text_overlay_at(40, 40), Some(0));
}

#[test]
fn clear_pe_maxis_man_actors_kind_only_and_zeros_xtxt() {
    // Disaster-end bulk clear matches FUN_00401528 → FUN_004507D0.
    let mut city = City::new_blank("/tmp/sc2000-maxis-clear.sc2", "MaxisClear").expect("blank");
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let slot = crate::win95_overlay::XTHG_FIRST_USABLE_SLOT;
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: crate::win95_overlay::XTHG_MAXIS_MAN,
            state: crate::win95_overlay::PeMaxisManState::LightActive as u8,
            tile_x: 20,
            tile_y: 21,
            height: 0x0b,
            saved_a: 0x44,
            saved_b: 9,
            ..Default::default()
        };
    }
    city.set_text_overlay(20, 21, proxy);
    assert_eq!(clear_pe_maxis_man_actors(&mut city), 1);
    let rec = city.traffic_overlays().unwrap()[slot];
    assert_eq!(rec.kind, 0);
    assert_eq!(rec.height, 0x0b);
    assert_eq!(rec.saved_b, 9);
    assert_eq!(rec.saved_a, 0x44);
    assert_eq!((rec.tile_x, rec.tile_y), (20, 21));
    assert_eq!(city.text_overlay_at(20, 21), Some(0));
}

#[test]
fn mac_engine_does_not_run_pe_ambient_actor_tick_on_win95_city_file() {
    let mut city = City::new_blank("/tmp/sc2000-mac-engine-overlay.sc2", "MacEngineOverlay")
        .expect("blank Win95-layout city");
    city.set_building(20, 20, 224);
    for x in 20..=24 {
        city.set_tile_bits(x, 20, TileBits::new(0x04));
        city.set_terrain(x, 20, 0x10);
    }
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let slot = city
        .traffic_overlays()
        .expect("XTHG table")
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo ship");
    let marker_before = city.text_overlay_at(20, 20);

    let before = city.traffic_overlays().unwrap()[slot];
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.traffic.set_speed(GameSpeed::Llama);
    let crt_before = state.pe_process_crt_seed();
    let lfsr_before = state.pe_process_lfsr_state();

    assert!(!state.tick_win95_overlay_sim_frame(1.0));

    assert_eq!(state.city.kind, CityFileKind::WinSc2);
    assert_eq!(state.city.traffic_overlays().unwrap()[slot], before);
    assert_eq!(state.city.text_overlay_at(20, 20), marker_before);
    assert_eq!(state.pe_process_crt_seed(), crt_before);
    assert_eq!(state.pe_process_lfsr_state(), lfsr_before);
}

#[test]
fn win95_engine_advances_its_xthg_actor_on_both_city_layouts() {
    let mut outcomes = Vec::new();
    for path in ["pe-xthg-engine.sc2", "pe-xthg-engine.city"] {
        let mut city = blank_city_layout_fixture(path, "PE actor layout");
        city.set_building(20, 20, 224);
        for x in 20..=24 {
            city.set_tile_bits(x, 20, TileBits::new(0x04));
            city.set_terrain(x, 20, 0x10);
        }
        assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
        let slot = city
            .traffic_overlays()
            .unwrap()
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
            .expect("cargo ship");

        let mut state = test_state(city);
        assert_eq!(state.engine_fidelity, EngineFidelity::Win95);
        state.traffic.set_speed(GameSpeed::Llama);
        state.advance_frame_simulation(Duration::from_millis(200));

        let ship = state.city.traffic_overlays().unwrap()[slot];
        assert!(
            ship.tile_x != 20 || ship.sub_x != 8,
            "Win95 XTHG actor should advance for {path}; got tile={}, subtile={}",
            ship.tile_x,
            ship.sub_x
        );
        outcomes.push((
            state.city.traffic_overlays().unwrap().to_vec(),
            (0..128)
                .flat_map(|y| (0..128).map(move |x| (x, y)))
                .map(|(x, y)| state.city.text_overlay_at(x, y))
                .collect::<Vec<_>>(),
            state.pe_process_crt_seed(),
            state.pe_process_lfsr_state(),
            state.pe_process_game_rand.state(),
        ));
    }
    assert_eq!(
        outcomes[0], outcomes[1],
        "the same Win95 actor input must evolve identically after decoding either ordinary city container"
    );
}

#[test]
fn microwave_ambient_target_scans_plant_for_both_engines() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 50, 50, MICROWAVE_POWER_BUILDING_ID, 4);
    stamp_complete_building_for_test(&mut city, 10, 80, MICROWAVE_POWER_BUILDING_ID, 4);
    let want = pe_microwave_plant_tile(&city).expect("microwave plant");
    assert_eq!(want, (10, 80));

    let mut rng = MacToolboxRng::new(0x35CE_000A);
    assert_eq!(
        microwave_ambient_target_tile(&city, &mut rng, EngineFidelity::Mac68k),
        Some(want)
    );
    assert_eq!(
        microwave_ambient_target_tile(&city, &mut rng, EngineFidelity::Win95),
        Some(want)
    );
}

#[test]
fn mac_export_231_weather_branch_matches_pe_remainder_gate() {
    // Mac 0x35932 / PE 0x471ef6: remainder < 0x0F, weather 0x0A+ocean → hurricane,
    // weather 0x0B → tornado. Shared helper keeps that contract for both engines.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(
        MISC_WEATHER_TREND_OFFSET,
        u32::from(WEATHER_TREND_HURRICANE),
    );
    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    assert_eq!(
        pe_ambient_weather_disaster_kind(&city, 0x0e),
        Some(DisasterKind::Hurricane)
    );
    assert_eq!(pe_ambient_weather_disaster_kind(&city, 0x0f), None);

    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_DISABLED_VALUE);
    assert_eq!(pe_ambient_weather_disaster_kind(&city, 0x0e), None);

    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    assert_eq!(
        pe_ambient_weather_disaster_kind(&city, 0x00),
        Some(DisasterKind::Tornado)
    );
}

#[test]
fn pe_hurricane_geography_survives_mac_layout_save_without_replacing_mac_options() {
    let path = std::env::temp_dir().join(format!(
        "citygame-pe-hurricane-geography-{}.city",
        std::process::id()
    ));
    let mut source = City::new_blank(&path, "MacCoast").expect("blank city");
    source.kind = CityFileKind::MacCity;
    // Mac CODE 4 saves the coast and river options at these native MISC
    // offsets. The PE loader reads the same raw dwords as its geography flags.
    source.set_mac_state_misc_u32_at(0x0e44, 1);
    source.set_mac_state_misc_u32_at(0x0e48, 1);
    source
        .save(&path)
        .expect("save Mac-layout coast and river options");

    let mut city = City::load(&path).expect("open saved Mac-layout city");
    assert_eq!(city.kind, CityFileKind::MacCity);
    assert_eq!(city.pe_dw_city_has_ocean(), 0, "engine cache is deferred");
    city.set_misc_u32_at(
        MISC_WEATHER_TREND_OFFSET,
        u32::from(WEATHER_TREND_HURRICANE),
    );
    city.prepare_pe_fidelity_from_saved_city();
    assert_eq!(city.pe_dw_city_has_ocean(), 1);
    assert_eq!(city.pe_dw_city_has_river(), 1);
    assert_eq!(
        pe_ambient_weather_disaster_kind(&city, 0x0e),
        Some(DisasterKind::Hurricane),
        "PE hurricane selection uses the saved coast flag on either layout"
    );

    // The PE engine can subsequently change its geography cache without
    // mutating the Mac option bytes. CGEX carries the PE continuation state.
    city.set_pe_dw_city_has_ocean(0);
    city.set_pe_dw_city_has_river(0);
    city.save(&path)
        .expect("save PE state in Mac-layout container");
    let mut reloaded = City::load(&path).expect("reload cross-layout city");
    reloaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reloaded.pe_dw_city_has_ocean(), 0);
    assert_eq!(reloaded.pe_dw_city_has_river(), 0);
    assert_eq!(
        reloaded.mac_state_misc_u32_at(0x0e44),
        Some(1),
        "PE state must not replace Mac coast option"
    );
    assert_eq!(
        reloaded.mac_state_misc_u32_at(0x0e48),
        Some(1),
        "PE state must not replace Mac river option"
    );
    assert_eq!(
        pe_ambient_weather_disaster_kind(&reloaded, 0x0e),
        None,
        "reloaded PE coast state gates hurricanes independently of Mac bytes"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pollution_accident_ambient_target_follows_engine_fidelity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..20 {
        for x in 10..20 {
            city.set_building(x, y, 0x70);
        }
    }
    let pe_center = land_value_city_center(&city);
    let mac_center = mac_developed_tile_anchor(&city);

    let mut mac_rng = MacToolboxRng::new(7);
    let mac = pollution_accident_ambient_target_tile(&city, &mut mac_rng, EngineFidelity::Mac68k);
    assert!(
        mac.0.abs_diff(mac_center.0) <= 16 && mac.1.abs_diff(mac_center.1) <= 16,
        "Mac ambient pollution target should use the Mac center, got {mac:?} center {mac_center:?}"
    );

    // Win95 keeps the land-probe helper; with only a small building block near
    // (14,14), probes can still land elsewhere on the map.
    let mut saw_off_center = false;
    for seed in 0..256 {
        let mut win_rng = MacToolboxRng::new(seed);
        let win =
            pollution_accident_ambient_target_tile(&city, &mut win_rng, EngineFidelity::Win95);
        if win.0.abs_diff(pe_center.0) > 16 || win.1.abs_diff(pe_center.1) > 16 {
            saw_off_center = true;
            break;
        }
    }
    assert!(
        saw_off_center,
        "Win95 ambient pollution target should not be locked to Mac center jitter"
    );
}

#[test]
fn mac_mod7e_plus_one_ambient_target_follows_engine_fidelity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // Fill the map so the Win95 land probe has legal targets everywhere.
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_building(x, y, 1);
        }
    }

    let mut mac_rng = MacToolboxRng::new(19);
    let mac = mac_mod7e_plus_one_ambient_target_tile(&city, &mut mac_rng, EngineFidelity::Mac68k);
    assert!((1..=0x7e).contains(&mac.0) && (1..=0x7e).contains(&mac.1));

    let mut mac_rng2 = MacToolboxRng::new(23);
    let mut direct = MacToolboxRng::new(23);
    assert_eq!(
        mac_mod7e_plus_one_ambient_target_tile(&city, &mut mac_rng2, EngineFidelity::Mac68k),
        mac_random_interior_map_tile(&mut direct)
    );

    let mut win_matches_mac_formula = true;
    for seed in 0..64 {
        let mut win_rng = MacToolboxRng::new(seed);
        let mut mac_formula = MacToolboxRng::new(seed);
        let win =
            mac_mod7e_plus_one_ambient_target_tile(&city, &mut win_rng, EngineFidelity::Win95);
        let mac_tile = mac_random_interior_map_tile(&mut mac_formula);
        if win != mac_tile {
            win_matches_mac_formula = false;
            break;
        }
    }
    assert!(
        !win_matches_mac_formula,
        "Win95 fire/flood/quake/tornado ambient should keep the land probe, not Mac %0x7E+1"
    );
}

#[test]
fn bulldoze_midband_xtxt_clears_xthg_saved_a_like_pe_401dca() {
    // PE @ `0x45a0ef`/`0x45a101`: XTXT `0xc9..=0xf0` keeps the byte but still
    // calls `0x401dca` → clear XTHG `saved_a` for slot `xtxt-0xc9`.
    let mut city = City::new_blank("/tmp/sc2000-bull-xtxt-mid.sc2", "BullXtxtMid").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_terrain(20, 20, 1); // non-flat → PlaceTile(0), not CRT rubble
    city.set_text_overlay(20, 20, 0xd0);
    let mut table = crate::win95_overlay::empty_overlay_table();
    table[0xd0 - 0xc9].saved_a = 0x55;
    city.set_traffic_overlays_for_test(table);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xd0));
    assert_eq!(
        state.city.traffic_overlays().unwrap()[0xd0 - 0xc9].saved_a,
        0
    );
}

#[test]
fn mac68k_bulldoze_midband_xtxt_clears_xthg_saved_a_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-mid-mac.sc2", "BullXtxtMidMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_terrain(20, 20, 2);
    city.set_text_overlay(20, 20, 0xce);
    let mut table = crate::win95_overlay::empty_overlay_table();
    table[0xce - 0xc9].saved_a = 0x44;
    city.set_traffic_overlays_for_test(table);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xce));
    assert_eq!(
        state.city.traffic_overlays().unwrap()[0xce - 0xc9].saved_a,
        0
    );
}

#[test]
fn pe_destroy_building_selector1_support_walk_clears_live_band_and_consumes_two_draws_per_cell() {
    let mut city =
        City::new_blank("/tmp/sc2000-support-body-crt.sc2", "SupportBodyCrt").expect("blank");
    city.set_building(20, 4, 0x6a);
    city.set_building(20, 5, 0x51);
    city.set_zone(20, 4, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    city.set_zone(20, 5, ZONE_INDUSTRIAL_LIGHT | 0xf0);
    city.set_tile_bits(20, 4, TileBits::new(0xff));
    // Clear hit rotate so the selector-1 scan runs down the Y axis.
    city.set_tile_bits(20, 5, TileBits::new(0xfd));
    let mut actual = Win95CrtRand::new(0x1234);
    let mut expected = Win95CrtRand::new(0x1234);
    for _ in 0..4 {
        let _ = expected.random_u15();
    }

    pe_destroy_building_selector1_support_walk_458e0f(&mut city, Some(&mut actual), 20, 5, true);

    assert_eq!(city.building_at(20, 4), 0);
    assert_eq!(city.building_at(20, 5), 0);
    assert_eq!(city.zone_at(20, 4).unwrap().raw_bits() & 0xf0, 0);
    assert_eq!(city.zone_at(20, 5).unwrap().raw_bits() & 0xf0, 0);
    assert_eq!(city.tile_bits_at(20, 4).unwrap().raw_bits(), 0xfd);
    assert_eq!(city.tile_bits_at(20, 5).unwrap().raw_bits(), 0xfd);
    assert_eq!(actual.random_u15(), expected.random_u15());
}

#[test]
fn pe_dispatch_extinguish_destroys_then_place_tile_rubble() {
    // PE `0x4605e0`: DestroyBuilding mode 0 then PlaceTile rubble (not invent-only set_building).
    // XBLD `>= 0x61` and highway bands `0x49..=0x50` take other PE arms; use mid-range `0x55`.
    let mut city = City::new_blank("/tmp/sc2000-disp-ext.sc2", "DispExt").expect("blank");
    city.set_building(41, 41, 0x55);
    city.set_building(42, 41, 0x55);
    city.set_text_overlay(41, 41, 0xff);
    city.set_traffic_density(41, 41, 0x28);
    let mut crt = Win95CrtRand::new(0x4612_1001);
    let mut preview = Win95CrtRand::new(0x4612_1001);
    let _destroy = preview.rubble_tile_id();
    let mut lfsr = Win95Lfsr::new(0x42);
    assert!(pe_dispatch_extinguish_fire(
        &mut city, &mut crt, &mut lfsr, 41, 41
    ));
    assert!((1..=4).contains(&city.building_at(41, 41)));
    assert_eq!(city.text_overlay_at(41, 41), Some(0));
    assert_eq!(city.traffic_density_at(41, 41), Some(0x28));
    assert_eq!(crt.seed(), preview.seed());
    // Neighbor is not part of a PE multi-tile footprint for size-1 `0x55`.
    assert_eq!(city.building_at(42, 41), 0x55);
}

#[test]
fn pe_dispatch_extinguish_vacant_still_place_tile_rubble() {
    // PE `0x46065f`: XBLD 0 DestroyBuilding mode 0 early-outs (`0x458d8a`) then PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-disp-vac.sc2", "DispVac").expect("blank");
    city.set_text_overlay(41, 41, 0xff);
    let mut crt = Win95CrtRand::new(0x4612_5501);
    let seed = crt.seed();
    let mut lfsr = Win95Lfsr::new(0x42);
    let mut preview = Win95Lfsr::new(0x42);
    let rubble = pe_fire_rubble_building(&mut preview);
    assert!(pe_dispatch_extinguish_fire(
        &mut city, &mut crt, &mut lfsr, 41, 41
    ));
    assert_eq!(city.building_at(41, 41), rubble);
    assert_eq!(city.text_overlay_at(41, 41), Some(0));
    assert_eq!(crt.seed(), seed);
    assert_eq!(lfsr.state(), preview.state());
}

#[test]
fn pe_dispatch_extinguish_vacant_keeps_zone_corners_like_destroy_building_early_out() {
    // PE DestroyBuilding mode 0 @ `0x458d8a` early-outs XBLD `< 6` — no XZON clear.
    // DestroyStructure would `XZON &= 0x0f` before the follow-on PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-disp-vac-zon.sc2", "DispVacZ").expect("blank");
    city.set_text_overlay(41, 41, 0xff);
    city.set_zone(41, 41, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(0x42);
    assert!(pe_dispatch_extinguish_fire(
        &mut city, &mut crt, &mut lfsr, 41, 41
    ));
    assert!((1..=4).contains(&city.building_at(41, 41)));
    assert_eq!(
        city.zone_at(41, 41).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
}

#[test]
fn pe_dispatch_extinguish_high_xbld_destroys_when_xzon_corners_all() {
    // PE @ `0x46067f`: XBLD `>= 0x61` + XZON corners `0xf0` → DestroyBuilding mode 0, no follow-on PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-disp-hi.sc2", "DispHi").expect("blank");
    city.set_building(41, 41, 0x70);
    city.set_zone(41, 41, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_text_overlay(41, 41, 0xff);
    city.set_traffic_density(41, 41, 0x19);
    let mut crt = Win95CrtRand::new(0x4612_7f01);
    let mut preview = Win95CrtRand::new(0x4612_7f01);
    let expected = preview.rubble_tile_id();
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_dispatch_extinguish_fire(
        &mut city, &mut crt, &mut lfsr, 41, 41
    ));
    assert_eq!(city.building_at(41, 41), expected);
    assert_eq!(city.text_overlay_at(41, 41), Some(0));
    assert_eq!(city.traffic_density_at(41, 41), Some(0x19));
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn pe_dispatch_extinguish_high_xbld_keeps_building_without_corner_all() {
    let mut city = City::new_blank("/tmp/sc2000-disp-noc.sc2", "DispNoC").expect("blank");
    city.set_building(40, 40, 0x70);
    city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT | 0x10); // not ZONE_CORNER_ALL
    city.set_text_overlay(40, 40, 0xff);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_dispatch_extinguish_fire(
        &mut city, &mut crt, &mut lfsr, 40, 40
    ));
    assert_eq!(city.building_at(40, 40), 0x70);
    assert_eq!(city.text_overlay_at(40, 40), Some(0));
}

#[test]
fn pe_dispatch_police_fight_chance_is_lfsr_nibble_not_crt() {
    // PE `0x460408` calls `0x4025e5` (LFSR `& 0xf`) before the CRT direction.
    // Seed 3: CRT nibble is 0 (old chance would fight) but LFSR nibble is 6.
    let mut skip = City::new_blank("/tmp/sc2000-disp-pol-skip.sc2", "PolSkip").expect("blank");
    stamp_complete_building_for_test(&mut skip, 100, 100, 210, 3);
    assert!(skip.place_dispatch_actor(40, 40, DispatchKind::Police));
    skip.set_text_overlay(41, 40, 0xff);
    skip.set_building(41, 40, 0x50);
    let mut crt = Win95CrtRand::new(3);
    let mut lfsr = Win95Lfsr::new(3);
    let stats = pe_dispatch_proxy_tick_city_with_rng(&mut skip, &mut crt, &mut lfsr);
    assert_eq!(stats.fires_cleared, 0);
    assert_eq!(skip.building_at(41, 40), 0x50);
    assert_eq!(skip.text_overlay_at(41, 40), Some(0xff));

    // Seed 16: CRT nibble is 10 (old chance would skip) but LFSR nibble is 0.
    // First CRT `& 3` is east.
    let mut fight = City::new_blank("/tmp/sc2000-disp-pol-fight.sc2", "PolFight").expect("blank");
    stamp_complete_building_for_test(&mut fight, 100, 100, 210, 3);
    assert!(fight.place_dispatch_actor(40, 40, DispatchKind::Police));
    fight.set_text_overlay(41, 40, 0xff);
    fight.set_building(41, 40, 0x50);
    let mut crt = Win95CrtRand::new(16);
    let mut lfsr = Win95Lfsr::new(16);
    let stats = pe_dispatch_proxy_tick_city_with_rng(&mut fight, &mut crt, &mut lfsr);
    assert_eq!(stats.fires_cleared, 1);
    assert_eq!(fight.text_overlay_at(41, 40), Some(0));
    assert!((1..=4).contains(&fight.building_at(41, 40)));
}

#[test]
fn place_dispatch_actor_paints_and_restores_xtxt_proxy() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    // PE `FUN_00450690` only places when XTXT is already 0; restore is that zero.
    assert_eq!(city.text_overlay_at(20, 21).unwrap_or(0), 0);
    assert!(city.place_dispatch_actor(20, 21, DispatchKind::Police));
    let proxy = city.text_overlay_at(20, 21).expect("proxy");
    assert!((0xc9..0xf1).contains(&proxy));
    assert!(city.remove_dispatch_actor_at(20, 21));
    assert_eq!(city.text_overlay_at(20, 21), Some(0));
}

#[test]
fn pe_fun_00460e10_skips_dispatch_proxy_xtxt_without_paint() {
    // PE @ `0x460f12`: XTXT `0xc9..=0xf0` returns 0 — leave proxy, no `0xff`.
    let mut city = City::new_blank("/tmp/sc2000-ignite-proxy.sc2", "IgniteProxy").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xc9);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    pe_fun_00460e10_ignite(&mut city, Some(&mut crt), &mut lfsr, 20, 20, 0);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xc9));
    assert_eq!(city.building_at(20, 20), 0x70);
}

#[test]
fn pe_overlay_scan_due_on_marker_without_family_type() {
    // PE `0x45f760` matches `0xfc` / `0xfe` / `0xfb` with no type gate.
    let mut city = City::new_blank("/tmp/sc2000-scan-due.sc2", "ScanDue").expect("blank");
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    assert!(!pe_flood_overlay_scan_due(&city));
    assert!(!pe_riot_overlay_scan_due(&city));
    assert!(!pe_fb_overlay_scan_due(&city));
    city.set_text_overlay(20, 20, 0xfc);
    assert!(pe_flood_overlay_scan_due(&city));
    city.set_text_overlay(20, 20, 0xfe);
    assert!(pe_riot_overlay_scan_due(&city));
    city.set_text_overlay(20, 20, 0xfb);
    assert!(pe_fb_overlay_scan_due(&city));
}

#[test]
fn pe_maxis_man_daily_clears_dispatch_xtxt_proxies() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    assert!(city.place_dispatch_actor(20, 21, DispatchKind::Police));
    let (slot, _) = city.dispatch_actor_at(20, 21).expect("police");
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    assert_eq!(city.text_overlay_at(20, 21), Some(proxy));
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::Firestorm));
    arm_pe_maxis_man_spawn_gates(&mut state);
    assert_eq!(state.tick_pe_maxis_man_spawn(), 1);
    assert!(state.city.dispatch_actor_at(20, 21).is_none());
    assert_eq!(state.city.traffic_overlays().unwrap()[slot].kind, 0);
    assert_eq!(state.city.text_overlay_at(20, 21), Some(0));
    assert!(
        state
            .city
            .traffic_overlays()
            .unwrap()
            .iter()
            .any(|r| r.kind == crate::win95_overlay::XTHG_MAXIS_MAN),
        "Maxis Man must survive the dispatch-only sweep"
    );
}

#[test]
fn pe_maxis_man_constructor_saves_xtxt_not_xtrf_to_plus_a() {
    // PE FUN_00452FA0 @ 0x4530d9: +A = spawn-tile XTXT (0x4CA600), not XTRF.
    let mut city = City::new_blank("/tmp/maxis-xtxt-a.sc2", "MaxisXtxtA").expect("blank city");
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let mut state = test_state(city);
    state.hover_tile = Some((20, 20));
    assert!(state.start_disaster(DisasterKind::Firestorm));
    arm_pe_maxis_man_spawn_gates(&mut state);
    let mut preview = state.pe_process_crt;
    let (spawn_x, spawn_y) = pe_maxis_man_spawn_tile_jitter(20, 20, &mut preview);
    state.city.set_text_overlay(spawn_x, spawn_y, 0x42);
    state.city.set_traffic_density(spawn_x, spawn_y, 0x80);
    assert_eq!(state.tick_pe_maxis_man_spawn(), 1);
    let (slot, record) = state
        .city
        .traffic_overlays()
        .and_then(|table| {
            table
                .iter()
                .enumerate()
                .find(|(_, r)| r.kind == crate::win95_overlay::XTHG_MAXIS_MAN)
        })
        .map(|(s, r)| (s, *r))
        .expect("maxis");
    assert_eq!(record.saved_a, 0x42);
    assert_eq!(
        state.city.text_overlay_at(spawn_x, spawn_y),
        xthg_proxy_for_slot(slot)
    );
    assert_eq!(
        state.city.traffic_density_at(spawn_x, spawn_y),
        Some(0x80),
        "constructor must not consume XTRF into +A"
    );
}

#[test]
fn pe_maxis_man_spawn_tile_jitters_by_crt_rand_and_3() {
    // PE `FUN_00452FA0`: after loading `0x4CAA58/5C`, `rand()&3` nudges spawn
    // ±0x10 (clamp 0..=0x7f). Direction is `0x402dbf` toward the event tile.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::Firestorm));
    arm_pe_maxis_man_spawn_gates(&mut state);
    let overlay_before = state.win95_overlay_sim.sim_rng_seed();
    let mut preview = state.pe_process_crt;
    let (spawn_x, spawn_y) = pe_maxis_man_spawn_tile_jitter(12, 34, &mut preview);
    let expected_dir = pe_xthg_direction_toward(spawn_x as i16, spawn_y as i16, 12, 34);
    assert_ne!((spawn_x, spawn_y), (12, 34));
    assert!(
        (spawn_x == 12 && spawn_y.abs_diff(34) == 0x10)
            || (spawn_y == 34 && spawn_x.abs_diff(12) == 0x10)
    );

    assert_eq!(state.tick_pe_maxis_man_spawn(), 1);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(
        state.win95_overlay_sim.sim_rng_seed(),
        overlay_before,
        "spawn-tile jitter must not advance city-name overlay CRT"
    );
    let (_, record) = state
        .city
        .traffic_overlays()
        .and_then(|table| {
            table
                .iter()
                .enumerate()
                .find(|(_, r)| r.kind == crate::win95_overlay::XTHG_MAXIS_MAN)
        })
        .expect("Maxis Man actor");
    assert_eq!(
        (record.tile_x as usize, record.tile_y as usize),
        (spawn_x, spawn_y)
    );
    assert_eq!(
        (record.target_x as usize, record.target_y as usize),
        (12, 34)
    );
    assert_eq!(record.lane, expected_dir);
    assert_eq!(
        state.city.text_overlay_at(spawn_x, spawn_y),
        xthg_proxy_for_slot(
            state
                .city
                .traffic_overlays()
                .unwrap()
                .iter()
                .position(|r| r.kind == crate::win95_overlay::XTHG_MAXIS_MAN)
                .unwrap()
        )
    );
}

#[test]
fn pe_maxis_man_handler_tick_consumes_process_crt() {
    // PE `FUN_00455020` handler rolls share process CRT, not city-name overlay sim_rng.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    assert!(state.start_disaster(DisasterKind::Fire));
    arm_pe_maxis_man_spawn_gates(&mut state);
    assert_eq!(state.tick_pe_maxis_man_spawn(), 1);
    if let Some(overlays) = state.city.traffic_overlays_mut() {
        for record in overlays.iter_mut() {
            if record.kind == crate::win95_overlay::XTHG_MAXIS_MAN {
                record.state = crate::win95_overlay::PeMaxisManState::LightActive as u8;
            }
        }
    }
    let overlay_before = state.win95_overlay_sim.sim_rng_seed();
    let mut preview = state.pe_process_crt;
    let mut expected = state.city.clone();
    let preview_changed = Win95OverlaySim::new().tick_pe_maxis_man(&mut expected, &mut preview);
    assert!(
        preview.seed() != state.pe_process_crt_seed(),
        "light-active handler must draw CRT"
    );
    let live_changed = state
        .win95_overlay_sim
        .tick_pe_maxis_man(&mut state.city, &mut state.pe_process_crt);
    assert_eq!(live_changed, preview_changed);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(
        state.win95_overlay_sim.sim_rng_seed(),
        overlay_before,
        "Maxis Man handler must not advance city-name overlay CRT"
    );
}

#[test]
fn pe_maxis_man_spawn_requires_declined_and_random_and3_zero() {
    // PE `0x45d066`: `0x4cc4e4 == 1` then `0x402261` RandomAnd3==0 before ctor.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::Firestorm));

    let lfsr_before = state.pe_process_lfsr.state();
    assert_eq!(
        military_base_type_for_city(&state.city),
        MILITARY_BASE_NONE,
        "blank city must not be DECLINED"
    );
    assert_eq!(state.tick_pe_maxis_man_spawn(), 0);
    assert_eq!(
        state.pe_process_lfsr.state(),
        lfsr_before,
        "non-DECLINED must not consume RandomAnd3 LFSR"
    );
    assert!(
        state
            .city
            .traffic_overlays()
            .unwrap()
            .iter()
            .all(|r| r.kind != crate::win95_overlay::XTHG_MAXIS_MAN)
    );

    set_military_base_type_for_test(&mut state.city, MILITARY_BASE_DECLINED);
    // LFSR `0x00FF` → mask 2 (refuse); advances state.
    state.pe_process_lfsr = Win95Lfsr::new(0x00FF);
    let mut refuse_preview = state.pe_process_lfsr;
    assert_ne!(refuse_preview.random_mask_03(), 0);
    assert_eq!(state.tick_pe_maxis_man_spawn(), 0);
    assert_eq!(state.pe_process_lfsr.state(), refuse_preview.state());
    assert!(
        state
            .city
            .traffic_overlays()
            .unwrap()
            .iter()
            .all(|r| r.kind != crate::win95_overlay::XTHG_MAXIS_MAN)
    );

    arm_pe_maxis_man_spawn_gates(&mut state);
    assert_eq!(state.tick_pe_maxis_man_spawn(), 1);
    assert!(
        state
            .city
            .traffic_overlays()
            .unwrap()
            .iter()
            .any(|r| r.kind == crate::win95_overlay::XTHG_MAXIS_MAN)
    );
}

#[test]
fn pe_xthg_direction_toward_matches_pe_4515e0_octants() {
    assert_eq!(pe_xthg_direction_toward(40, 40, 40, 20), 0); // N
    assert_eq!(pe_xthg_direction_toward(40, 40, 60, 40), 2); // E
    assert_eq!(pe_xthg_direction_toward(40, 40, 40, 60), 4); // S
    assert_eq!(pe_xthg_direction_toward(40, 40, 20, 40), 6); // W
    assert_eq!(pe_xthg_direction_toward(40, 40, 60, 20), 1); // NE
    assert_eq!(pe_xthg_direction_toward(40, 40, 60, 60), 3); // SE
    assert_eq!(pe_xthg_direction_toward(40, 40, 20, 60), 5); // SW
    assert_eq!(pe_xthg_direction_toward(40, 40, 20, 20), 7); // NW
}

#[test]
fn citytool_substation_uses_place_tile_size_then_place_subway() {
    // PE CityTool @ `0x44088d`: PlaceSubwayAtCoordinates `0x44bb20`, then PlaceXUND(0x23),
    // XBIT &= 0xDF (clear piped).
    let mut city = City::new_blank("/tmp/sc2000-ct-sub.sc2", "CtSub").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(36, 36, TileBits::new(0).with_watered(true));
    city.set_traffic_density(36, 36, 0x2c);
    let mut state = test_state(city);
    state.tool = EditTool::SubStation;
    assert_eq!(state.place_building_at(36, 36, 233), 1);
    assert_eq!(state.city.building_at(36, 36), 233);
    assert_eq!(state.city.underground_at(36, 36), UNDER_TILE_GREY_BOX_2);
    let bits = state.city.tile_bits_at(36, 36).unwrap();
    // SubStation clears the piped bit in PlaceSubway, so the outer CityTool
    // water-recalc gate does not run and the old served-water bit survives.
    assert!(bits.watered() && !bits.powered() && bits.powerable());
    assert!(!bits.piped());
    assert_eq!(bits.raw_bits() & 0xe0, 0x80);
    assert_eq!(
        state.city.zone_at(36, 36).unwrap().raw_bits(),
        ZONE_CORNER_ALL
    );
    assert_eq!(state.city.traffic_density_at(36, 36), Some(0x2c));
}

#[test]
fn citytool_substation_place_subway_reshapes_adjacent_subway() {
    // PE PlaceSubwayAtCoordinates @ `0x44bba4`+: reshape commit + neighbors via `0x44bc40`
    // before PlaceXUND(0x23). Seed an EW stub (tile 2) north so NS gate pickup is visible;
    // grey-box `0x23` NS-gates (EW gate at `0x4e7c70[0x23]` is 0).
    let mut city = City::new_blank("/tmp/sc2000-ct-sub-adj.sc2", "CtSubAdj").expect("blank");
    city.set_city_funds(20_000);
    city.set_underground(36, 35, 2);
    let mut state = test_state(city);
    let before = state.city.underground_at(36, 35);
    assert_eq!(before, 2);
    assert!(!underground_tile_connects_to(
        before,
        UndergroundKind::Subway,
        NETWORK_BOTTOM
    ));
    state.tool = EditTool::SubStation;
    assert_eq!(state.place_building_at(36, 36, 233), 1);
    assert_eq!(state.city.underground_at(36, 36), UNDER_TILE_GREY_BOX_2);
    let after = state.city.underground_at(36, 35);
    assert_ne!(after, before, "EW stub should reshape toward the station");
    assert!(
        underground_tile_connects_to(after, UndergroundKind::Subway, NETWORK_BOTTOM),
        "subway north of station should gain a south (bottom) connection"
    );
}

#[test]
fn maptool_sub_to_rail_place_subway_reshapes_adjacent_subway() {
    // PE MapTool SubToRail @ `0x441d06` → PlaceSubwayAtCoordinates before PlaceXUND(0x23).
    let mut city = City::new_blank("/tmp/sc2000-str-sub-adj.sc2", "StrSubAdj").expect("blank");
    city.set_city_funds(20_000);
    city.set_underground(20, 19, 2);
    let mut state = test_state(city);
    let before = state.city.underground_at(20, 19);
    assert_eq!(before, 2);
    assert!(!underground_tile_connects_to(
        before,
        UndergroundKind::Subway,
        NETWORK_BOTTOM
    ));
    state.tool = EditTool::SubToRail;
    assert!(state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.underground_at(20, 20), UNDER_TILE_GREY_BOX_2);
    let after = state.city.underground_at(20, 19);
    assert_ne!(after, before, "EW stub should reshape toward sub-rail");
    assert!(
        underground_tile_connects_to(after, UndergroundKind::Subway, NETWORK_BOTTOM),
        "subway north of sub-rail should gain a south (bottom) connection"
    );
}

#[test]
fn maptool_sub_to_rail_refuses_without_adjacent_rail_or_subway() {
    // PE @ `0x441cf0`: neither rail XBLD nor subway XUND neighbor → return 0.
    let mut city = City::new_blank("/tmp/sc2000-str-refuse.sc2", "StrRefuse").expect("blank");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    assert!(!state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn pe_army_crane_place_tile_size_clears_airport_xbld_before_stamp() {
    // PE @ `0x4387ce`: XBLD >= 0x0D → FUN_004392A0 then PlaceTile-size(0xe0).
    let mut city =
        City::new_blank("/tmp/sc2000-army-crane-clear.sc2", "ArmyCraneClear").expect("blank");
    city.set_building(20, 20, 0xdc);
    city.set_text_overlay(20, 20, 0x55);
    pe_army_crane_place_tile_size_4387e7(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0xe0);
}

#[test]
fn stamp_zone_development_military_crane_strip_ors_xbit_02_on_odd_rotation() {
    // PE @ `0x438854`: rotation `0x4c942c` bit0 odd + DX==0 → XBIT |= 0x02 on strip.
    let mut city =
        City::new_blank("/tmp/sc2000-crane-strip-rot.sc2", "CraneStripRot").expect("blank");
    city.set_rotation(1);
    city.set_zone(24, 24, ZONE_MILITARY | ZONE_CORNER_ALL);
    for y in 25..=29 {
        city.set_zone(24, y, ZONE_MILITARY | ZONE_CORNER_ALL);
        city.set_tile_bits(24, y, TileBits::new(0).with_water_tile(true));
    }
    city.set_altitude(24, 29, 5, 7);
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (24, 24),
        zone_id: ZONE_MILITARY,
        building: MILITARY_CRANE_BUILDING_ID,
        footprint: vec![(24, 24)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(24, 25), 0xdf);
    assert_eq!(
        state.city.tile_bits_at(24, 25).unwrap().raw_bits() & 0x02,
        0x02
    );
}

#[test]
fn stamp_zone_development_military_crane_places_runway_cross_strip() {
    // PE after crane zone write: dir probe `0x43862d` then strip `0x438890`.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-crane-strip.sc2", "CraneStrip").expect("blank");
    city.set_zone(24, 24, ZONE_MILITARY | ZONE_CORNER_ALL);
    for y in 25..=29 {
        city.set_zone(24, y, ZONE_MILITARY | ZONE_CORNER_ALL);
        city.set_tile_bits(24, y, TileBits::new(0).with_water_tile(true));
    }
    city.set_altitude(24, 29, 5, 7);
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (24, 24),
        zone_id: ZONE_MILITARY,
        building: MILITARY_CRANE_BUILDING_ID,
        footprint: vec![(24, 24)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(24, 24), MILITARY_CRANE_BUILDING_ID);
    for y in 25..29 {
        assert_eq!(state.city.building_at(24, y), 0xdf, "strip y={y}");
    }
    // dir0 DX==0 + rotation even → no XBIT |= 0x02
    assert_eq!(
        state.city.tile_bits_at(24, 25).unwrap().raw_bits() & 0x02,
        0
    );
}

#[test]
fn pe_army_crane_select_strip_dir_picks_first_xbit_water_neighbor() {
    let mut city = City::new_blank("/tmp/sc2000-crane-dir.sc2", "CraneDir").expect("blank");
    city.set_zone(40, 40, ZONE_MILITARY);
    assert_eq!(pe_army_crane_select_strip_dir_43862d(&city, 40, 40), None);
    city.set_tile_bits(41, 40, TileBits::new(0).with_water_tile(true)); // east = dir 1
    assert_eq!(
        pe_army_crane_select_strip_dir_43862d(&city, 40, 40),
        Some(1)
    );
    // rot0 even: DX!=0 → or_xbit; DX==0 → no or_xbit
    assert!(pe_army_crane_strip_or_xbit_02(1, 0));
    assert!(!pe_army_crane_strip_or_xbit_02(0, 0));
    // rot1 odd: inverted
    assert!(!pe_army_crane_strip_or_xbit_02(1, 1));
    assert!(pe_army_crane_strip_or_xbit_02(0, 1));
}

#[test]
fn stamp_zone_development_military_2x2_retries_warehouse_when_cargo_place_fails() {
    // PE SimulationGrow AF/navy @ `0x4364e4`/`0x4365dc`: cargo/warehouse Place fail → `0xe3`.
    let mut city = City::new_blank("/tmp/sc2000-stamp-navy-wh-fb.sc2", "StampWhFb").expect("blank");
    for (x, y) in [(34, 34), (35, 34), (34, 35), (35, 35)] {
        city.set_zone(x, y, ZONE_MILITARY | ZONE_CORNER_ALL);
        city.set_altitude(x, y, 5, 5);
    }
    city.set_building(34, 34, 5);
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (34, 34),
        zone_id: ZONE_MILITARY,
        building: MILITARY_CARGO_YARD_BUILDING_ID,
        footprint: vec![(34, 34), (35, 34), (34, 35), (35, 35)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(34, 34), 5);
    assert_eq!(state.city.building_at(35, 34), 0);
    assert_eq!(
        state.city.zone_at(34, 34).unwrap().raw_bits() & 0x0f,
        ZONE_MILITARY
    );
}

#[test]
fn stamp_zone_development_seaport_cargo_retries_warehouse_when_place_fails() {
    // PE SimulationGrow seaport @ `0x4365c7`/`0x4365dc`: Place fail → warehouse `0xe3`.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-seaport-wh-fb.sc2", "StampSeaWh").expect("blank");
    for (x, y) in [(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_zone(x, y, ZONE_SEAPORT | ZONE_CORNER_ALL);
        city.set_altitude(x, y, 5, 5);
    }
    city.set_building(40, 40, 5);
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (40, 40),
        zone_id: ZONE_SEAPORT,
        building: MILITARY_CARGO_YARD_BUILDING_ID,
        footprint: vec![(40, 40), (41, 40), (40, 41), (41, 41)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(40, 40), 5);
    assert_eq!(state.city.building_at(41, 40), 0);
}

#[test]
fn stamp_zone_development_seaport_cargo_places_via_place_tile_size() {
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-seaport-cargo.sc2", "StampSeaCargo").expect("blank");
    for (x, y) in [(42, 42), (43, 42), (42, 43), (43, 43)] {
        city.set_zone(x, y, ZONE_SEAPORT | ZONE_CORNER_ALL);
        city.set_altitude(x, y, 5, 5);
    }
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (42, 42),
        zone_id: ZONE_SEAPORT,
        building: MILITARY_CARGO_YARD_BUILDING_ID,
        footprint: vec![(42, 42), (43, 42), (42, 43), (43, 43)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    for (x, y) in [(42, 42), (43, 42), (42, 43), (43, 43)] {
        assert_eq!(
            state.city.building_at(x, y),
            MILITARY_CARGO_YARD_BUILDING_ID
        );
    }
}

#[test]
fn stamp_zone_development_airport_2x2_uses_place_tile_size_without_retry() {
    // PE SimulationGrow airport Place @ `0x4367fe` ignores return — no warehouse retry.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-airport-2x2.sc2", "StampAir2").expect("blank");
    for (x, y) in [(44, 44), (45, 44), (44, 45), (45, 45)] {
        city.set_zone(x, y, ZONE_AIRPORT | ZONE_CORNER_ALL);
        city.set_altitude(x, y, 5, 5);
    }
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (44, 44),
        zone_id: ZONE_AIRPORT,
        building: MILITARY_HANGAR2_BUILDING_ID,
        footprint: vec![(44, 44), (45, 44), (44, 45), (45, 45)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    for (x, y) in [(44, 44), (45, 44), (44, 45), (45, 45)] {
        assert_eq!(state.city.building_at(x, y), MILITARY_HANGAR2_BUILDING_ID);
    }
}

#[test]
fn stamp_zone_development_airport_2x2_place_fail_skips_retry_and_cap() {
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-airport-2x2-ref.sc2", "StampAirRef").expect("blank");
    for (x, y) in [(46, 46), (47, 46), (46, 47), (47, 47)] {
        city.set_zone(x, y, ZONE_AIRPORT | ZONE_CORNER_ALL);
        city.set_altitude(x, y, 5, 5);
    }
    city.set_building(46, 46, 5);
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (46, 46),
        zone_id: ZONE_AIRPORT,
        building: MILITARY_HANGAR2_BUILDING_ID,
        footprint: vec![(46, 46), (47, 46), (46, 47), (47, 47)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(46, 46), 5);
    assert_eq!(state.city.building_at(47, 46), 0);
}

#[test]
fn stamp_zone_development_seaport_loading_bay_retries_warehouse_when_place_fails() {
    // PE seaport loading-bay `0xf0` jmp @ `0x436597`→`0x4365bf` into Place+warehouse retry.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-seaport-lb-fb.sc2", "StampSeaLb").expect("blank");
    for (x, y) in [(48, 48), (49, 48), (48, 49), (49, 49)] {
        city.set_zone(x, y, ZONE_SEAPORT | ZONE_CORNER_ALL);
        city.set_altitude(x, y, 5, 5);
    }
    city.set_building(48, 48, 5);
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (48, 48),
        zone_id: ZONE_SEAPORT,
        building: CIV_LOADING_BAY_BUILDING_ID,
        footprint: vec![(48, 48), (49, 48), (48, 49), (49, 49)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.building_at(49, 48), 0);
}

#[test]
fn stamp_zone_development_seaport_crane_places_via_place_tile_size() {
    // PE seaport crane `0xe0` jmp @ `0x436577`→`0x4365bf` PlaceTile prologue.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-seaport-crane.sc2", "StampSeaCrane").expect("blank");
    city.set_zone(50, 50, ZONE_SEAPORT | ZONE_CORNER_ALL);
    city.set_altitude(50, 50, 5, 5);
    stamp_seaport_crane_water_ray_for_test(&mut city, (50, 50));
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin: (50, 50),
        zone_id: ZONE_SEAPORT,
        building: SEAPORT_CRANE_BUILDING_ID,
        footprint: vec![(50, 50)],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(state.city.building_at(50, 50), SEAPORT_CRANE_BUILDING_ID);
}

#[test]
fn pe_fun_004392a0_airport_range_clears_single_tile() {
    // PE @ `0x4392dd`: XBLD `0xDB..=0xEA` → PlaceTile(0) + XBIT`&=0x3f` + XZON`&=0x0f`.
    let mut city = City::new_blank("/tmp/sc2000-392a0-air.sc2", "Fun392Air").expect("blank");
    city.set_building(21, 21, 0xdd);
    city.set_building(20, 20, 0xcc);
    city.set_building(22, 22, 0xcc);
    city.set_tile_bits(21, 21, TileBits::new(0xff));
    city.set_zone(21, 21, ZONE_MILITARY | ZONE_CORNER_ALL);
    pe_fun_004392a0_clear_before_runway(&mut city, 21, 21);
    assert_eq!(city.building_at(21, 21), 0);
    assert_eq!(city.tile_bits_at(21, 21).unwrap().raw_bits(), 0x3f);
    assert_eq!(city.zone_at(21, 21).unwrap().raw_bits(), ZONE_MILITARY);
    assert_eq!(city.building_at(20, 20), 0xcc, "neighbors untouched");
    assert_eq!(city.building_at(22, 22), 0xcc);
}

#[test]
fn pe_army_crane_runway_cross_strip_places_four_df_south() {
    // PE @ `0x438890`: four PlaceTile(0xdf) along neighbor delta (dir0 = south).
    let mut city =
        City::new_blank("/tmp/sc2000-army-crane-strip.sc2", "ArmyCraneSt").expect("blank");
    city.set_tile_bits(40, 40, TileBits::new(0).with_watered(true));
    city.set_traffic_density(40, 41, 0x33);
    pe_army_crane_place_runway_cross_strip_438890(&mut city, 40, 40, 0, false);
    for dy in 1..=4 {
        assert_eq!(city.building_at(40, 40 + dy), 0xdf);
        assert_eq!(
            city.zone_at(40, 40 + dy).unwrap().raw_bits() & ZONE_CORNER_ALL,
            ZONE_CORNER_ALL
        );
    }
    assert_eq!(city.building_at(40, 40), 0);
    assert!(city.tile_bits_at(40, 40).unwrap().watered());
    assert_eq!(city.traffic_density_at(40, 41), Some(0x33));
}

#[test]
fn pe_army_crane_runway_cross_strip_ors_xbit_02_when_flagged() {
    let mut city = City::new_blank("/tmp/sc2000-army-crane-x02.sc2", "ArmyCraneX2").expect("blank");
    pe_army_crane_place_runway_cross_strip_438890(&mut city, 50, 50, 1, true); // east
    for dx in 1..=4 {
        assert_eq!(city.building_at(50 + dx, 50), 0xdf);
        assert_eq!(
            city.tile_bits_at(50 + dx, 50).unwrap().raw_bits() & 0x02,
            0x02
        );
    }
}

#[test]
fn mac68k_army_crane_runway_cross_strip_places_four_df_west() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-army-crane-strip.sc2", "MacArmySt").expect("blank");
    city.set_tile_bits(60, 60, TileBits::new(0).with_watered(true));
    city.set_traffic_density(59, 60, 0x22);
    pe_army_crane_place_runway_cross_strip_438890(&mut city, 60, 60, 3, false); // west
    for dx in 1..=4 {
        assert_eq!(city.building_at(60 - dx, 60), 0xdf);
        assert_eq!(
            city.zone_at(60 - dx, 60).unwrap().raw_bits() & ZONE_CORNER_ALL,
            ZONE_CORNER_ALL
        );
    }
    assert!(city.tile_bits_at(60, 60).unwrap().watered());
    assert_eq!(city.traffic_density_at(59, 60), Some(0x22));
}

#[test]
fn pe_army_2x2_clear_before_place_clears_airport_xbld() {
    let mut city = City::new_blank("/tmp/sc2000-army-2x2-clr.sc2", "Army2x2Clr").expect("blank");
    city.set_building(50, 50, 0xdc);
    city.set_building(51, 50, 0x0a);
    pe_army_2x2_clear_before_place_438b1c(&mut city, 50, 50);
    assert_eq!(city.building_at(50, 50), 0);
    assert_eq!(city.building_at(51, 50), 0x0a); // <0x0D: skip FUN_004392A0 call
}

#[test]
fn pe_helicopter_state5_queues_mayday_sound_at_height_four() {
    let mut city = City::new_blank("/tmp/sc2000-heli-state5-mayday.sc2", "HeliState5Mayday")
        .expect("blank city");
    assert!(city.place_ambient_port_actor(22, 22, XTHG_HELICOPTER, XTHG_DIRECTION_NORTH,));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .position(|record| record.kind == XTHG_HELICOPTER)
        .expect("helicopter");
    {
        let record = &mut city.traffic_overlays_mut().unwrap()[slot];
        record.state = 5;
        record.height = 5;
    }

    let mut state = test_state(city);
    state.sound_effects_enabled = true;
    state.tick_pe_disaster_xthg_actors_in_slot_order();
    assert!(state.take_pending_disaster_extra_sounds().is_empty());
    assert_eq!(state.city.traffic_overlays().unwrap()[slot].height, 4);

    state.tick_pe_disaster_xthg_actors_in_slot_order();
    assert_eq!(
        state.take_pending_disaster_extra_sounds(),
        vec![PlacementSound::Mayday]
    );
    assert_eq!(state.city.traffic_overlays().unwrap()[slot].height, 3);
}

#[test]
fn win95_xthg_actor_sweep_uses_selected_engine_on_both_city_layouts() {
    for path in ["pe-xthg-layout.sc2", "pe-xthg-layout.city"] {
        let mut city = blank_city_layout_fixture(path, "PE actor layout");
        assert!(city.place_ambient_port_actor(22, 22, XTHG_HELICOPTER, XTHG_DIRECTION_NORTH,));
        let slot = city
            .traffic_overlays()
            .unwrap()
            .iter()
            .position(|record| record.kind == XTHG_HELICOPTER)
            .expect("helicopter");
        {
            let record = &mut city.traffic_overlays_mut().unwrap()[slot];
            record.state = 5;
            record.height = 5;
        }

        let mut state = test_state(city);
        assert_eq!(state.engine_fidelity, EngineFidelity::Win95);
        state.mode = AppMode::CityView;
        state.traffic.set_speed(GameSpeed::Turtle);
        // One PE timer pulse signals the Things walker even though Turtle does
        // not admit a calendar day on its first pulse.
        state.advance_frame_simulation(Duration::from_millis(200));

        assert_eq!(
            state.city.traffic_overlays().unwrap()[slot].height,
            4,
            "{path}"
        );
    }
}

#[test]
fn monthly_ambient_clock_seed_does_not_replace_live_process_rng() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    let clock = first_due_ambient_disaster_clock(&city);
    let mut clock_seed = Win95CrtRand::new(ambient_disaster_chance_seed(&city, clock));
    assert_eq!(
        pe_ambient_disaster_chance_remainder(&mut clock_seed, city_difficulty_for_city(&city)),
        Some(0),
        "the offline clock-derived chance seed is due"
    );
    let mut state = test_state(city);
    state.game_clock = clock;
    state.open_tool_menu = Some(ui::ToolMenu::Water);

    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());

    state.handle_simulation_month_advanced();

    assert!(
        disaster_overlay_tiles(&state.city).is_empty() && !state.city.has_active_disaster(),
        "the live month stream must decide the event; a clock-derived offline chance seed is not a forced event"
    );
    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Water));
    assert!(!state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Emergency)));
}

#[test]
fn emergency_tools_place_preview_and_bulldoze_dispatch_actors() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_text_overlay(30, 40, 0xff);
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    let mut state = test_state(city);
    state.tool = EditTool::DispatchPolice;

    let placement = state.tool_placement_at(20, 21).expect("dispatch preview");
    assert_eq!(placement.valid_tiles, vec![(20, 21)]);
    assert!(placement.invalid_tiles.is_empty());
    assert!(state.apply_tool_at_tile(20, 21));
    assert_eq!(
        state
            .city
            .dispatch_actor_at(20, 21)
            .unwrap()
            .1
            .dispatch_kind(),
        Some(DispatchKind::Police)
    );

    state.tool = EditTool::Bulldoze;
    assert!(state.apply_tool_at_tile(20, 21));
    assert_eq!(state.city.building_at(20, 21), 0);
    assert!(state.city.dispatch_actor_at(20, 21).is_none());
}

#[test]
fn dispatch_actor_blocks_non_zone_surface_edit_tools_until_bulldozed() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    assert!(city.place_dispatch_actor(20, 21, DispatchKind::Police));
    let mut state = test_state(city);

    state.tool = EditTool::Trees;
    assert_eq!(state.place_tree_at(20, 21), 0);

    state.tool = EditTool::Water;
    assert_eq!(state.place_surface_water_at(20, 21), 0);
    assert_eq!(state.city.terrain_at(20, 21), 0);

    state.tool = EditTool::RaiseTerrain;
    assert!(!state.place_city_terrain_edit_at(20, 21));
    assert_eq!(state.city.altitude_at(20, 21), BLANK_MAP_BASE_HEIGHT);

    state.tool = EditTool::LevelTerrain;
    assert!(!state.place_city_terrain_edit_at(20, 21));

    state.tool = EditTool::PowerPlantWind;
    assert_eq!(state.place_building_at(20, 21, 200), 0);
    assert!(state.city.dispatch_actor_at(20, 21).is_some());

    state.tool = EditTool::Bulldoze;
    assert!(state.apply_tool_at_tile(20, 21));
    assert!(state.city.dispatch_actor_at(20, 21).is_none());
}

#[test]
fn emergency_tool_preview_rejects_existing_dispatch_actor_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(30, 40, 0xff);
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    assert!(city.place_dispatch_actor(20, 21, DispatchKind::Police));
    let mut state = test_state(city);
    state.tool = EditTool::DispatchPolice;

    let placement = state.tool_placement_at(20, 21).expect("dispatch preview");
    assert!(placement.valid_tiles.is_empty());
    assert_eq!(placement.invalid_tiles, vec![(20, 21)]);
    assert!(!state.apply_tool_at_tile(20, 21));
    assert_eq!(
        state
            .city
            .dispatch_actor_at(20, 21)
            .and_then(|(_, record)| record.dispatch_kind()),
        Some(DispatchKind::Police)
    );
}

#[test]
fn opening_emergency_menu_clears_previous_dispatches() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(30, 40, 0xff);
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    assert!(city.place_dispatch_actor(20, 21, DispatchKind::Police));
    let mut state = test_state(city);

    state.open_tool_menu(ui::ToolMenu::Emergency);

    assert!(state.city.dispatch_actor_at(20, 21).is_none());
    assert_eq!(state.city.building_at(20, 21), 0);
}

#[test]
fn dispatch_tool_mapping_matches_recovered_xthg_kinds() {
    assert_eq!(
        dispatch_kind_for_tool(EditTool::DispatchPolice),
        Some(DispatchKind::Police)
    );
    assert_eq!(
        dispatch_kind_for_tool(EditTool::DispatchFirefighters),
        Some(DispatchKind::Firefighters)
    );
    assert_eq!(
        dispatch_kind_for_tool(EditTool::DispatchMilitary),
        Some(DispatchKind::Military)
    );
    assert_eq!(dispatch_kind_for_tool(EditTool::Police), None);
}

#[test]
fn mac68k_place_pipes_subway_early_out_skips_own_kind() {
    let mut city = City::new_blank("/tmp/sc2000-ug-early-mac.sc2", "UgEarlyMac").expect("blank");
    city.set_underground(20, 20, 0x10);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        0
    );
    assert_eq!(state.city.underground_at(20, 20), 0x10);
}

#[test]
fn place_pipes_subway_early_out_skips_own_kind_and_exclusive() {
    // PE PlacePipes/Subway AtCoordinates early ret: no re-stamp / no debit.
    let mut city = City::new_blank("/tmp/sc2000-ug-early.sc2", "UgEarly").expect("blank");
    city.set_city_funds(20_000);
    city.set_underground(20, 20, 0x10);
    let mut state = test_state(city);
    let funds = state.city.city_funds();
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        0
    );
    assert_eq!(state.city.underground_at(20, 20), 0x10);
    assert_eq!(state.city.city_funds(), funds);

    let mut city = City::new_blank("/tmp/sc2000-ug-early2.sc2", "UgEarly2").expect("blank");
    city.set_city_funds(20_000);
    city.set_underground(20, 20, 0x1f);
    let mut state = test_state(city);
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        0
    );
    assert_eq!(state.city.underground_at(20, 20), 0x1f);

    let mut city = City::new_blank("/tmp/sc2000-ug-early3.sc2", "UgEarly3").expect("blank");
    city.set_city_funds(20_000);
    city.set_underground(20, 20, 0x05);
    let mut state = test_state(city);
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Subway),
        0
    );
    assert_eq!(state.city.underground_at(20, 20), 0x05);

    let mut city = City::new_blank("/tmp/sc2000-ug-refuse.sc2", "UgRefuse").expect("blank");
    city.set_underground(20, 20, 7); // subway curve — PlacePipes refuse
    let state = test_state(city);
    assert!(!state.can_place_underground_at(20, 20, UndergroundKind::Pipes));
}

#[test]
fn pe_place_pipes_subway_crossover_tables_match_exe() {
    // PE PlacePipesAtCoordinates `0x44b7e3` / PlaceSubway `0x44bb7e`.
    assert_eq!(
        pe_place_pipes_crossover_from_existing_44b7f5(1),
        Some(UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR)
    );
    assert_eq!(
        pe_place_pipes_crossover_from_existing_44b7f5(2),
        Some(UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB)
    );
    assert_eq!(pe_place_pipes_crossover_from_existing_44b7f5(0), None);
    assert_eq!(pe_place_pipes_crossover_from_existing_44b7f5(7), None);
    assert_eq!(
        pe_place_subway_crossover_from_existing_44bb8e(0x10),
        Some(UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB)
    );
    assert_eq!(
        pe_place_subway_crossover_from_existing_44bb8e(0x11),
        Some(UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR)
    );
    assert_eq!(pe_place_subway_crossover_from_existing_44bb8e(0), None);
    assert_eq!(pe_place_subway_crossover_from_existing_44bb8e(0x16), None);
}

#[test]
fn mac68k_placing_pipes_on_straight_subway_uses_pe_crossover_table() {
    let mut city = City::new_blank("/tmp/sc2000-pipe-sub-mac.sc2", "PipeSubMac").expect("blank");
    city.set_underground(20, 20, 1);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        1
    );
    assert_eq!(
        state.city.underground_at(20, 20),
        UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR
    );
}

#[test]
fn placing_pipes_on_straight_subway_uses_pe_crossover_table() {
    let mut city = City::new_blank("/tmp/sc2000-pipe-sub-x.sc2", "PipeSubX").expect("blank");
    city.set_underground(20, 20, 1); // subway NS
    let mut state = test_state(city);
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        1
    );
    assert_eq!(
        state.city.underground_at(20, 20),
        UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR
    );
}

#[test]
fn placing_subway_on_straight_pipes_uses_pe_crossover_table() {
    let mut city = City::new_blank("/tmp/sc2000-sub-pipe-x.sc2", "SubPipeX").expect("blank");
    city.set_underground(20, 20, 0x10); // pipes LR
    let mut state = test_state(city);
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Subway),
        1
    );
    assert_eq!(
        state.city.underground_at(20, 20),
        UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB
    );
}

#[test]
fn pe_maptool_surface_clear_subway_xund_without_grey_box_gate() {
    // PE @ `0x443434`/`0x44344b`: SubStation clears XUND even when not 0x23.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-xund.sc2", "MapSurfXu").expect("blank");
    city.set_building(30, 30, 0xe9);
    city.set_underground(30, 30, UNDER_TILE_PIPES_LR);
    city.set_tile_bits(30, 30, TileBits::new(0).with_watered(true));
    city.set_traffic_density(30, 30, 0x21);
    assert!(pe_maptool_surface_clear_subway_xund_44344b(
        &mut city, 30, 30, 0xe9
    ));
    assert_eq!(city.underground_at(30, 30), 0);
    assert_eq!(city.building_at(30, 30), 0xe9);
    assert!(city.tile_bits_at(30, 30).unwrap().watered());
    assert_eq!(city.traffic_density_at(30, 30), Some(0x21));
}

#[test]
fn pe_maptool_surface_clear_subway_transition_xbld() {
    // PE @ `0x44343b`: XBLD 0x6c..=0x70 also PlaceXUND(0).
    let mut city = City::new_blank("/tmp/sc2000-map-surf-6c.sc2", "MapSurf6c").expect("blank");
    city.set_underground(31, 31, UNDER_TILE_GREY_BOX_2);
    assert!(pe_maptool_surface_clear_subway_xund_44344b(
        &mut city, 31, 31, 0x6c
    ));
    assert_eq!(city.underground_at(31, 31), 0);
    assert!(!pe_maptool_surface_clear_subway_xund_44344b(
        &mut city, 31, 31, 0x6b
    ));
}

#[test]
fn pe_maptool_underground_clear_subway_on_grey_box() {
    // PE @ `0x443410`/`0x443427`: XUND 0x23 → DestroyBuilding + PlaceXUND(0).
    let mut city = City::new_blank("/tmp/sc2000-map-und-23.sc2", "MapUnd23").expect("blank");
    city.set_building(32, 32, 0xe9);
    city.set_text_overlay(32, 32, 0x44);
    city.set_underground(32, 32, UNDER_TILE_GREY_BOX_2);
    let mut crt = Win95CrtRand::new(0x4434_2701);
    let mut preview = Win95CrtRand::new(0x4434_2701);
    let expected = expected_single_tile_mode1_rubble_after_generic_scatter(&mut preview);
    assert!(pe_maptool_underground_clear_subway_443427(
        &mut city,
        Some(&mut crt),
        32,
        32,
        true,
    ));
    assert_eq!(city.building_at(32, 32), expected);
    assert_eq!(city.text_overlay_at(32, 32), Some(0));
    assert_eq!(city.underground_at(32, 32), 0);
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn pe_maptool_underground_clear_subway_on_tunnel_level_one() {
    // PE @ `0x4433fe`: tunnel_levels==1 triggers even when XUND ≠ 0x23.
    let mut city = City::new_blank("/tmp/sc2000-map-und-tl1.sc2", "MapUndTl1").expect("blank");
    city.set_building(33, 33, 0xc8);
    city.set_underground(33, 33, UNDER_TILE_PIPES_LR);
    city.set_tunnel_levels(33, 33, 1);
    let mut crt = Win95CrtRand::new(0x4433_fe01);
    let mut preview = Win95CrtRand::new(0x4433_fe01);
    let expected = expected_single_tile_mode1_rubble_after_generic_scatter(&mut preview);
    assert!(pe_maptool_underground_clear_subway_443427(
        &mut city,
        Some(&mut crt),
        33,
        33,
        true,
    ));
    assert_eq!(city.building_at(33, 33), expected);
    assert_eq!(city.underground_at(33, 33), 0);
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn pe_maptool_underground_clear_subway_elevated_on_tunnel_level_one() {
    // PE @ `0x44341e`: DestroyBuilding mode 1 — elevated arm when tunnel_levels==1.
    let mut city = City::new_blank("/tmp/sc2000-map-und-elev.sc2", "MapUndEl").expect("blank");
    city.set_building(33, 33, 0x40);
    city.set_zone(33, 33, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    city.set_underground(33, 33, UNDER_TILE_PIPES_LR);
    city.set_tunnel_levels(33, 33, 1);
    let mut crt = Win95CrtRand::new(0x4433_fe01);
    let seed_before = crt.seed();
    assert!(pe_maptool_underground_clear_subway_443427(
        &mut city,
        Some(&mut crt),
        33,
        33,
        true,
    ));
    assert_eq!(city.building_at(33, 33), 0);
    assert_eq!(
        city.zone_at(33, 33).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0
    );
    assert_eq!(city.tunnel_levels_at(33, 33), 0);
    assert_eq!(city.underground_at(33, 33), 0);
    assert_eq!(crt.seed(), seed_before, "elevated arm does not consume CRT");
}

#[test]
fn pe_subway_xtxt_reshape_places_table_plus_one() {
    // PE @ `0x44bcc5`/`0x44be07`: XTXT nibble 1 → PlaceXUND(2+1).
    let mut city = City::new_blank("/tmp/sc2000-sub-xtxt.sc2", "SubXtxt").expect("blank");
    city.set_underground(47, 47, 0x01);
    city.set_text_overlay(47, 47, 0x01);
    city.set_traffic_density(47, 47, 0x11);
    assert!(pe_subway_xtxt_reshape_44bcc5(&mut city, 47, 47));
    assert_eq!(city.underground_at(47, 47), 0x03);
    assert_eq!(city.traffic_density_at(47, 47), Some(0x11));
}

#[test]
fn pe_subway_mask_tile_matches_rust_offsets() {
    for mask in 0u8..16 {
        assert_eq!(
            pe_subway_mask_tile_44be07(mask),
            underground_tile_for_mask(UndergroundKind::Subway, mask),
            "mask {mask:#x}"
        );
    }
}

#[test]
fn pe_subway_mask_tile_zero_is_tile_one() {
    // PE subway has no mask0→0xf force: table[0]+1 → 1.
    assert_eq!(pe_subway_mask_tile_44be07(0), 0x01);
}

#[test]
fn pe_subway_neighbor_gates_grey_box_1_ew_and_2_ns() {
    // PE @ `0x44bcf6` + gates `0x4e7cb8`/`0x4e7c70`: 0x22 EW-only, 0x23 NS-only.
    assert!(!pe_subway_reshape_neighbor_connects_44bcf6(
        UNDER_TILE_GREY_BOX_1,
        NETWORK_TOP
    ));
    assert!(pe_subway_reshape_neighbor_connects_44bcf6(
        UNDER_TILE_GREY_BOX_1,
        NETWORK_LEFT
    ));
    assert!(pe_subway_reshape_neighbor_connects_44bcf6(
        UNDER_TILE_GREY_BOX_2,
        NETWORK_TOP
    ));
    assert!(!pe_subway_reshape_neighbor_connects_44bcf6(
        UNDER_TILE_GREY_BOX_2,
        NETWORK_LEFT
    ));
    assert!(!underground_tile_connects_to(
        UNDER_TILE_GREY_BOX_1,
        UndergroundKind::Pipes,
        NETWORK_LEFT
    ));
}

#[test]
fn pe_subway_neighbor_gate_tables_match_pe_bytes() {
    assert_eq!(PE_XUND_NS_GATE_4E7CB8[0x22], 0);
    assert_eq!(PE_XUND_EW_GATE_4E7C70[0x22], 1);
    assert_eq!(PE_XUND_NS_GATE_4E7CB8[0x23], 1);
    assert_eq!(PE_XUND_EW_GATE_4E7C70[0x23], 0);
    assert_eq!(PE_XUND_NS_GATE_4E7CB8[0x01], 1);
    assert_eq!(PE_XUND_EW_GATE_4E7C70[0x01], 0);
    assert_eq!(PE_XUND_NS_GATE_4E7CB8[0x02], 0);
    assert_eq!(PE_XUND_EW_GATE_4E7C70[0x02], 1);
}

#[test]
fn refresh_subway_reshapes_toward_grey_box_1_east_only() {
    let mut city = City::new_blank("/tmp/sc2000-sub-grey1.sc2", "SubGrey1").expect("blank");
    city.set_underground(48, 48, 0x01); // TB
    city.set_underground(48, 47, UNDER_TILE_GREY_BOX_1); // 0x22: NS gate 0
    city.set_text_overlay(48, 48, 0);
    let mut state = test_state(city);
    state.refresh_underground_tile(48, 48, UndergroundKind::Subway);
    // North 0x22 does not connect (NS gate 0) → still isolated TB.
    assert_eq!(state.city.underground_at(48, 48), 0x01);
    // East 0x22 connects (EW gate 1) → RIGHT only → tile 2 (LR).
    state.city.set_underground(49, 48, UNDER_TILE_GREY_BOX_1);
    state.refresh_underground_tile(48, 48, UndergroundKind::Subway);
    assert_eq!(state.city.underground_at(48, 48), 0x02);
}

#[test]
fn mac68k_refresh_subway_reshapes_toward_grey_box_1_east_only() {
    let mut city = City::new_blank("/tmp/sc2000-sub-grey1-mac.sc2", "SubGrey1Mac").expect("blank");
    city.set_underground(48, 48, 0x01);
    city.set_underground(48, 47, UNDER_TILE_GREY_BOX_1);
    city.set_text_overlay(48, 48, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_underground_tile(48, 48, UndergroundKind::Subway);
    assert_eq!(state.city.underground_at(48, 48), 0x01);
    state.city.set_underground(49, 48, UNDER_TILE_GREY_BOX_1);
    state.refresh_underground_tile(48, 48, UndergroundKind::Subway);
    assert_eq!(state.city.underground_at(48, 48), 0x02);
}

#[test]
fn native_global_sea_level_round_trips_for_both_saved_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
            let mut city = City::new_blank("/tmp/native-sea-save.sc2", "NativeSeaSave").unwrap();
            city.kind = kind;
            city.set_mac_state_misc_u32_at(0x0e40, 0x1234_0004);
            city.set_altitude(20, 20, 4, 3);
            city.set_terrain(20, 20, 3);
            let mut state = test_state(city);
            state.mode = AppMode::EditNewMap;
            state.engine_fidelity = engine;
            assert_eq!(state.adjust_edit_map_sea_level(1), 1);
            // The existing loader uses .sc2 versus .city as its layout hint.
            let extension = if kind == CityFileKind::MacCity {
                "city"
            } else {
                "sc2"
            };
            let path = std::env::temp_dir().join(format!(
                "citygame-native-sea-{}-{kind:?}-{engine:?}.{extension}",
                std::process::id()
            ));
            state.city.save(&path).unwrap();
            let loaded = City::load(&path).unwrap();
            let _ = std::fs::remove_file(path);
            assert_eq!(loaded.kind, kind);
            assert_eq!(loaded.mac_selected_water_height(), Some(5));
            assert_eq!(loaded.mac_state_misc_u32_at(0x0e40), Some(5));
            assert_eq!(loaded.water_height_at(20, 20), 5);
            assert_eq!(loaded.terrain_at(20, 20), 0x23);
        }
    }
}

#[test]
fn transport_palette_buildings_preview_and_place_through_selected_tools() {
    let cases = [
        (EditTool::BusDepot, 236, 2, None),
        (EditTool::RailDepot, 237, 2, None),
        (EditTool::SubStation, 233, 1, Some(UNDER_TILE_GREY_BOX_2)),
        (EditTool::SubToRail, 108, 1, Some(UNDER_TILE_GREY_BOX_2)),
    ];

    for (index, (tool, building, size, underground)) in cases.into_iter().enumerate() {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(20_000);
        let origin = (20 + index * 8, 20);
        if tool == EditTool::SubToRail {
            city.set_building(
                origin.0 - 1,
                origin.1,
                network_building_for_mask(NetworkKind::Rail, NETWORK_LEFT),
            );
        }
        let mut state = test_state(city);
        state.tool = tool;
        let placement = state
            .tool_placement_at(origin.0, origin.1)
            .expect("transport placement");
        let expected_building = match (tool, placement.preview) {
            (EditTool::SubToRail, ToolPreview::Building(id @ 108..=111)) => id,
            (_, ToolPreview::Building(id)) => id,
            _ => panic!("{tool:?} should preview a building"),
        };

        if tool == EditTool::SubToRail {
            assert!(matches!(expected_building, 108..=111));
        } else {
            assert_eq!(expected_building, building, "{tool:?}");
        }
        assert_eq!(placement.valid_tiles.len(), size * size, "{tool:?}");
        assert!(placement.invalid_tiles.is_empty(), "{tool:?}");
        assert!(state.apply_tool_at_tile(origin.0, origin.1), "{tool:?}");

        for y in origin.1..origin.1 + size {
            for x in origin.0..origin.0 + size {
                assert_eq!(state.city.building_at(x, y), expected_building, "{tool:?}");
            }
        }
        if let Some(underground) = underground {
            assert_eq!(
                state.city.underground_at(origin.0, origin.1),
                underground,
                "{tool:?}"
            );
        } else {
            assert_eq!(state.city.underground_at(origin.0, origin.1), 0);
            assert!(
                state
                    .city
                    .tile_bits_at(origin.0, origin.1)
                    .is_some_and(|bits| bits.piped()),
                "{tool:?} does not have its building plumbing"
            );
        }
    }
}

#[test]
fn subway_station_refreshes_neighboring_subway_access() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_underground(20, 19, 1);
    let mut state = test_state(city);
    state.tool = EditTool::SubStation;

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 233);
    assert_eq!(state.city.underground_at(20, 20), UNDER_TILE_GREY_BOX_2);
    assert_eq!(
        state.underground_neighbor_mask(20, 19, UndergroundKind::Subway),
        NETWORK_BOTTOM
    );

    state.bulldoze_at(20, 20);
    assert_eq!(state.city.underground_at(20, 20), 0);
    assert_eq!(
        state.underground_neighbor_mask(20, 19, UndergroundKind::Subway),
        0
    );
}

#[test]
fn pe_newspaper_gift_place_stadium_483767_advances_sports_teams() {
    // PE @ `0x483767`: PlaceTile-size stadium then sports ladder/microsim write.
    let mut city = City::new_blank("/tmp/sc2000-gift-stadium.sc2", "GiftStad").expect("blank");
    let mut crt = Win95CrtRand::new(1);
    city.set_city_funds(50_000);
    assert_eq!(city.pe_w_sports_teams(), 0);
    assert!(pe_newspaper_gift_place_stadium_483767(
        &mut city, &mut crt, 20, 20
    ));
    // Click (20,20) → NW (19,19); XTXT/sports at click.
    assert_eq!(city.building_at(19, 19), STADIUM_BUILDING_ID);
    assert_eq!(city.building_at(20, 20), STADIUM_BUILDING_ID);
    assert_eq!(city.pe_w_sports_teams(), 1);
    let xtxt = city.text_overlay_at(20, 20).expect("xtxt");
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES);
}

#[test]
fn pe_newspaper_gift_entry_483250_hospital_church_stadium_sports() {
    // PE zone-paint JT hospital/church @ `0x483df1`/`0x483e14` (stories `0x199`/`0x1a2`);
    // stadium PlaceTile-size + sports join @ `0x483767` (story `0x30b`); zone-type-only
    // band `0x1a7..0x1af` sets type 4 then selector `ax==0` skips paint.
    let mut city =
        City::new_blank("/tmp/sc2000-gift-entry-hcs.sc2", "GiftEntryHCS").expect("blank");
    let mut crt = Win95CrtRand::new(17);
    city.set_city_funds(50_000);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 20, 20, 0x199);
    assert_eq!(city.building_at(19, 19), HOSPITAL_BUILDING_ID);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 40, 40, 0x1a2);
    assert_eq!(city.building_at(39, 39), CHURCH_BUILDING_ID);
    // Church PlaceTile-size 2×2 from (39,39) covers the click tile; zone type cleared.
    assert_eq!(city.pe_w_sports_teams(), 0);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 60, 60, 0x30b);
    // Stadium click (61,58) → NW (60,57); XTXT/sports at click.
    assert_eq!(city.building_at(60, 57), STADIUM_BUILDING_ID);
    assert_eq!(city.building_at(61, 58), STADIUM_BUILDING_ID);
    assert_eq!(city.pe_w_sports_teams(), 1);
    let xtxt = city.text_overlay_at(61, 58).expect("stadium xtxt");
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 70, 70, 0x1a7);
    assert_eq!(city.zone_at(70, 70).expect("t4").raw_bits() & 0x0f, 4);
    assert_eq!(city.building_at(70, 70), 0, "0x1a7 paints nothing");
    // Hospital band paint≠4: zone-type 2 only (story 0x195 → ax=5, paint=0).
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 75, 75, 0x195);
    assert_eq!(city.zone_at(75, 75).expect("h0").raw_bits() & 0x0f, 2);
    assert_eq!(city.building_at(75, 75), 0);
}

#[test]
fn transport_zone_tools_place_through_selected_tool_path() {
    for (tool, zone_id, cost) in [
        (EditTool::Seaport, ZONE_SEAPORT, 150),
        (EditTool::Airport, ZONE_AIRPORT, 250),
    ] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(20_000);
        let mut state = test_state(city);
        state.tool = tool;

        let placement = state.tool_placement_at(20, 20).expect("zone placement");
        assert_eq!(placement.preview, ToolPreview::Zone(zone_id), "{tool:?}");
        assert_eq!(placement.valid_tiles, vec![(20, 20)], "{tool:?}");
        assert!(placement.invalid_tiles.is_empty(), "{tool:?}");

        assert!(state.apply_tool_at_tile(20, 20), "{tool:?}");
        assert_eq!(state.city.zone_at(20, 20).unwrap().zone_id(), zone_id);
        assert_eq!(state.city.building_at(20, 20), 0);
        assert_eq!(state.city.city_funds(), 20_000 - cost);
    }
}

#[test]
fn transport_zone_tools_drag_rectangles_like_other_zones() {
    for (tool, zone_id, cost) in [
        (EditTool::Seaport, ZONE_SEAPORT, 150),
        (EditTool::Airport, ZONE_AIRPORT, 250),
    ] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(20_000);
        let mut state = test_state(city);
        state.tool = tool;

        state.begin_zone_drag(20, 20, zone_id_for_tool(tool).expect("transport zone id"));
        state.update_zone_drag_to((22, 21));

        assert_eq!(state.zone_drag_cost(), Some(cost * 6), "{tool:?}");
        assert_eq!(state.finish_zone_drag(), 6, "{tool:?}");
        for y in 20..=21 {
            for x in 20..=22 {
                assert_eq!(
                    state.city.zone_at(x, y).unwrap().zone_id(),
                    zone_id,
                    "{tool:?} did not zone ({x},{y})"
                );
                assert_eq!(state.city.building_at(x, y), 0, "{tool:?}");
            }
        }
        assert_eq!(state.city.city_funds(), 20_000 - cost as i32 * 6);
    }
}

#[test]
fn zone_drag_paints_under_dispatch_actor_without_replacing_it() {
    for fidelity in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
        let mut city = City::new_blank("/tmp/sc2000-zone-under-dispatch-actor.sc2", "ZoneActor")
            .expect("blank city");
        city.set_city_funds(1_000);
        stamp_complete_building_for_test(&mut city, 40, 40, 210, 3);
        city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
        assert!(city.place_dispatch_actor_for_engine(20, 20, DispatchKind::Police, fidelity));
        let (slot, actor_before) = city.dispatch_actor_at(20, 20).expect("dispatch actor");
        let actor_proxy = crate::win95_overlay::xthg_proxy_for_slot(slot).expect("proxy");

        let mut state = test_state(city);
        state.engine_fidelity = fidelity;
        state.tool = EditTool::ZoneResidential;
        state.begin_zone_drag(20, 20, ZONE_RESIDENTIAL_LIGHT);

        assert_eq!(
            state.zone_drag_cost(),
            Some(zone_build_cost(ZONE_RESIDENTIAL_LIGHT))
        );
        assert_eq!(state.finish_zone_drag(), 1);
        assert_eq!(
            state.city.city_funds(),
            1_000 - zone_build_cost(ZONE_RESIDENTIAL_LIGHT) as i32
        );
        assert_eq!(
            state.city.zone_at(20, 20).unwrap().zone_id(),
            ZONE_RESIDENTIAL_LIGHT
        );
        assert_eq!(state.city.text_overlay_at(20, 20), Some(actor_proxy));
        assert_eq!(
            state.city.dispatch_actor_at(20, 20),
            Some((slot, actor_before))
        );
    }
}

#[test]
fn pe_subway_stroke_charges_and_applies_shared_surface_work() {
    let mut city = City::new_blank("/tmp/sc2000-subway-xter.sc2", "SubwayXter").expect("blank");
    city.set_city_funds(125);
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
    state.tool = EditTool::Subway;

    state.begin_underground_drag(10, 10, UndergroundKind::Subway);
    assert_eq!(
        state.underground_drag_cost(),
        Some((UndergroundKind::Subway, 125))
    );
    state.cancel_underground_drag();
    assert_eq!(
        state.place_underground_path(&[(10, 10)], UndergroundKind::Subway),
        1
    );
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.terrain_at(10, 10), 0x0d);
    let bits = state.city.tile_bits_at(10, 10).expect("XBIT");
    assert!(!bits.water_tile());
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn underground_palette_tools_create_pipe_subway_crossovers() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.place_underground_at(20, 19, UndergroundKind::Subway);
    state.place_underground_at(20, 20, UndergroundKind::Subway);
    state.place_underground_at(20, 21, UndergroundKind::Subway);
    assert_eq!(state.city.underground_at(20, 20), 1);

    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        1
    );
    assert_eq!(
        state.city.underground_at(20, 20),
        UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR
    );

    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.place_underground_path(&[(19, 20), (20, 20), (21, 20)], UndergroundKind::Pipes);
    assert_eq!(state.city.underground_at(20, 20), 17);

    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Subway),
        1
    );
    assert_eq!(
        state.city.underground_at(20, 20),
        UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR
    );
}

#[test]
fn underground_drag_paths_match_pipe_and_subway_tool_rules() {
    assert_eq!(
        underground_preview_path_for_kind((10, 10), (13, 12), UndergroundKind::Pipes),
        power_line_drag_path((10, 10), (13, 12))
    );
    assert_eq!(
        underground_commit_path_for_kind((10, 10), (13, 12), UndergroundKind::Pipes),
        power_line_drag_path((10, 10), (13, 12))
    );
    assert_eq!(
        underground_preview_path_for_kind((10, 10), (13, 12), UndergroundKind::Subway),
        power_line_drag_path((10, 10), (13, 12))
    );
    assert_eq!(
        underground_commit_path_for_kind((10, 10), (13, 12), UndergroundKind::Subway),
        network_drag_path((10, 10), (13, 12))
    );
}

#[test]
fn placing_diagonal_subway_drag_uses_road_style_diagonal_tunnels() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    let path = underground_commit_path_for_kind((20, 20), (23, 18), UndergroundKind::Subway);
    assert_eq!(path, network_drag_path((20, 20), (23, 18)));

    assert_eq!(
        state.place_underground_path(&path, UndergroundKind::Subway),
        path.len()
    );
    for (x, y) in path {
        assert!(
            UndergroundKind::Subway.contains(state.city.underground_at(x, y)),
            "expected subway tunnel at ({x},{y})"
        );
    }
}

#[test]
fn underground_drag_places_path_and_reports_cost() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.begin_underground_drag(10, 10, UndergroundKind::Pipes);
    state.update_underground_drag_to((10, 13));
    assert_eq!(
        state.underground_drag_cost(),
        Some((UndergroundKind::Pipes, 12))
    );

    let preview = {
        let drag = state.underground_drag.expect("drag");
        let path = underground_preview_path_for_kind(drag.start, drag.current, drag.kind);
        state.underground_drag_placement_for_path(&path, drag.kind)
    };
    assert_eq!(preview.valid_tiles.len(), 4);
    assert!(preview.invalid_tiles.is_empty());

    assert_eq!(
        state.finish_underground_drag(),
        Some((UndergroundKind::Pipes, 4))
    );
    for y in 10..=13 {
        assert_eq!(state.city.underground_at(10, y), 16);
    }
}

#[test]
fn underground_pipe_drag_uses_pe_subway_crossovers() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.place_underground_path(&[(10, 11), (10, 12), (10, 13)], UndergroundKind::Subway);

    let path = underground_preview_path_for_kind((10, 10), (10, 13), UndergroundKind::Pipes);
    let preview = state.underground_drag_placement_for_path(&path, UndergroundKind::Pipes);
    assert_eq!(preview.valid_tiles, path);
    assert!(preview.invalid_tiles.is_empty());

    assert_eq!(
        state.place_underground_path(&path, UndergroundKind::Pipes),
        4
    );
    for y in 10..=13 {
        assert!((0x10..=0x20).contains(&state.city.underground_at(10, y)));
    }
}

#[test]
fn maxis_land_developed_tiles_with_pipe_xund_keep_building_pipe_overlay() {
    let path = paths::default_city_path();
    if !path.exists() {
        return;
    }
    let mut state = test_state(City::load(&path).expect("load default city"));
    state.refresh_simulation_service_bits();
    let city = &state.city;
    let mut missing = Vec::new();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = city.building_at(x, y);
            if building == 0
                || is_tree_building(building)
                || is_road_tile(building)
                || is_rail_tile(building)
                || is_highway_tile(building)
                || is_power_tile(building)
            {
                continue;
            }
            if !building_has_internal_pipes(building) {
                continue;
            }
            let render_tile = underground_render_tile(city, x, y, city.rotation());
            let underlay_tile =
                underground_building_pipe_underlay_tile(city, x, y, city.rotation());
            let overlay_tile = underground_building_pipe_overlay_tile(city, x, y, city.rotation());
            let visible =
                render_tile.is_some() || underlay_tile.is_some() || overlay_tile.is_some();
            if visible {
                continue;
            }
            missing.push((
                x,
                y,
                building,
                city.zone_at(x, y).map(|zone| zone.raw_bits()).unwrap_or(0),
                city.underground_at(x, y),
                city.tile_bits_at(x, y)
                    .map(|bits| bits.raw_bits())
                    .unwrap_or(0),
            ));
            if missing.len() >= 80 {
                break;
            }
        }
        if missing.len() >= 80 {
            break;
        }
    }
    assert!(
        missing.is_empty(),
        "Maxis Land developed tiles without visible building plumbing: {missing:?}"
    );
}

#[test]
fn saved_xbit_piped_overlays_non_pipe_underground_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 112);
    city.set_underground(10, 10, UNDER_TILE_GREY_BOX_2);
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0b0010_0000));

    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_GREY_BOX_2,
            source: UndergroundTileSource::UserLaid,
            watered: false,
        })
    );
    assert_eq!(
        underground_building_pipe_overlay_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );
}

#[test]
fn underground_tile_fallback_tints_are_neutral_not_blue_overlay() {
    assert_eq!(underground_tile_tint(0), None);
    assert_eq!(underground_tile_tint(1), Some([0.20, 0.20, 0.20, 0.95]));
    assert_eq!(underground_tile_tint(16), Some([0.15, 0.15, 0.15, 0.95]));
    assert_eq!(
        underground_tile_tint(UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR),
        Some([0.18, 0.18, 0.18, 0.95])
    );
    assert_eq!(
        underground_tile_tint(UNDER_TILE_GREY_BOX_2),
        Some([0.28, 0.28, 0.28, 0.95])
    );
}

#[test]
fn pe_monthly_condition_story_queues_transit_shortage_0x2f() {
    // PE @ 0x471c13: subway+rail tiles + roads current <= pop/100 → story 0x2F.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-transit-story.sc2", "TransitT").expect("blank"));
    state.no_disasters_enabled = true;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.persist_pe_day22_population_leftover(10_000);
    // 0 stations + 0 roads current <= 100 → story fires.
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x2f));
}

#[test]
fn pe_monthly_condition_story_skips_transit_when_stations_cover_pop() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-transit-ok.sc2", "TransitOk").expect("blank"));
    state.no_disasters_enabled = true;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.persist_pe_day22_population_leftover(1_000);
    // PE @ `0x471c13` reads `dwTileCount[0xe9]`, not a live map rescan.
    for x in 0..20 {
        pe_place_tile_441f00(&mut state.city, x, 10, SUBWAY_STATION_BUILDING_ID);
    }
    // Police arm: one 3×3 station → count 1 > pop/20000 (=0) → skip index 2.
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    // Fire arm: one station → skip index 3.
    stamp_complete_building_for_test(&mut state.city, 40, 30, FIRE_STATION_BUILDING_ID, 3);
    // Water-coverage arm: keep <= 0x62 so index 4 does not win on the default 100.
    state.city.set_pe_water_coverage_percent(0x62);
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x2f));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x2e));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x30));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x31));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x32));
}

#[test]
fn pe_monthly_condition_transit_uses_dw_tile_count_like_471c13() {
    // PE @ `0x471c13`: `dwTileCount[0xe9]+[0xed]` — map-only stations without census do not cover.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-transit-dw.sc2", "TransitDw").expect("blank"));
    state.no_disasters_enabled = true;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.persist_pe_day22_population_leftover(1_000);
    for x in 0..20 {
        state.city.set_building(x, 10, SUBWAY_STATION_BUILDING_ID);
    }
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x2f));
}

#[test]
fn pe_monthly_condition_story_queues_seaport_coast_0x35() {
    // PE @ 0x471d70: crane+desal < (I_tax − arco/12)/10000 and coast → story 0x35.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-seaport-coast.sc2", "SeaCo").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 1);
    let ind = budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("ind tax");
    state.city.set_misc_u32_at(ind, 20_000); // /10000 = 2; 0 cranes < 2
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x35));
}

#[test]
fn pe_monthly_condition_story_queues_seaport_inland_0x3b() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-seaport-inland.sc2", "SeaIn").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 0);
    state.city.set_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, 0);
    let ind = budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("ind tax");
    state.city.set_misc_u32_at(ind, 20_000);
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x3b));
}

#[test]
fn pe_monthly_condition_story_queues_seaport_river_0x35() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-seaport-river.sc2", "SeaRi").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 0);
    state.city.set_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, 1);
    let ind = budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("ind tax");
    state.city.set_misc_u32_at(ind, 20_000);
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x35));
}

#[test]
fn pe_monthly_condition_story_skips_seaport_when_cranes_cover_tax() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-seaport-ok.sc2", "SeaOk").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 1);
    let ind = budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("ind tax");
    state.city.set_misc_u32_at(ind, 20_000); // threshold 2
    // Three crane tiles → score 9 >= 2.
    stamp_complete_building_for_test(&mut state.city, 70, 30, SEAPORT_CRANE_BUILDING_ID, 3);
    // Commercial tax 0 → airport arm does not fire.
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x35));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x3b));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x3c));
}

#[test]
fn pe_monthly_condition_story_skips_seaport_when_fa_non_network_covers_tax() {
    // PE `0x4ca3f0` FA-on-non-network tiles count toward seaport_score with cranes.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-seaport-fa.sc2", "SeaFa").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 1);
    let ind = budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("ind tax");
    state.city.set_misc_u32_at(ind, 20_000); // threshold 2
    state.city.set_building(20, 20, 0x0d);
    state.city.set_text_overlay(20, 20, 0xfa);
    state.city.set_building(21, 20, SMALL_PARK_BUILDING_ID);
    state.city.set_text_overlay(21, 20, 0xfa);
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x35));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x3b));
}

#[test]
fn mac68k_monthly_condition_story_skips_seaport_when_fa_non_network_covers_tax() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-seaport-fa-mac.sc2", "SeaFaMac").expect("blank"));
    state.engine_fidelity = EngineFidelity::Mac68k;
    pe_monthly_condition_cover_through_school(&mut state);
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 1);
    let ind = budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("ind tax");
    state.city.set_misc_u32_at(ind, 20_000);
    state.city.set_building(20, 20, 0x0d);
    state.city.set_text_overlay(20, 20, 0xfa);
    state.city.set_building(21, 20, SMALL_PARK_BUILDING_ID);
    state.city.set_text_overlay(21, 20, 0xfa);
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x35));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x3b));
}

#[test]
fn pe_monthly_condition_story_desal_tiles_without_fa_do_not_cover_seaport() {
    // Building 250 (desal) is not PE word `0x4ca3f0`; only XTXT 0xfa non-network is.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-seaport-desal.sc2", "SeaDe").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 1);
    let ind = budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("ind tax");
    state.city.set_misc_u32_at(ind, 20_000); // threshold 2
    stamp_complete_building_for_test(&mut state.city, 70, 30, 250, 3); // nine desal tiles, no FA
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x35));
}

#[test]
fn pe_monthly_condition_story_queues_airport_shortage_0x3c() {
    // PE @ 0x471dc4: runway+cross < (C_tax − arco/12)/2000; Free Clinics off → 0x3C.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-airport-story.sc2", "AirT").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    // Cover seaport: keep industrial tax 0 so arm does not win.
    let com = budget_row_misc_offset(1, BUDGET_SERVICE_CURRENT_FIELD).expect("com tax");
    state.city.set_misc_u32_at(com, 4_000); // /2000 = 2; 0 runways < 2
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x3c));
}

#[test]
fn pe_monthly_condition_story_queues_airport_free_clinics_0x36() {
    // PE @ `0x471dfd`: `0x4e9a30` bit2 → story 0x36 (tech-unlock arm 6 ORs it).
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-airport-clinics.sc2", "AirCl").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    let com = budget_row_misc_offset(1, BUDGET_SERVICE_CURRENT_FIELD).expect("com tax");
    state.city.set_misc_u32_at(com, 4_000);
    // Zero fame-rank 6 → rebuild ORs `0x4e9a30` bit2.
    for i in 0..17 {
        state.city.set_pe_w_neighbor_fame_rank(i, 1900);
    }
    state.city.set_pe_w_neighbor_fame_rank(6, 0);
    let _ = state.city.pe_rebuild_tech_unlock_mask_46a0be(0);
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x36));
}

#[test]
fn pe_monthly_condition_story_skips_airport_when_runways_cover_tax() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-airport-ok.sc2", "AirOk").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    let com = budget_row_misc_offset(1, BUDGET_SERVICE_CURRENT_FIELD).expect("com tax");
    state.city.set_misc_u32_at(com, 4_000); // threshold 2
    // PE @ `0x471dc4` reads `dwTileCount[0xdd]`, not a map rescan.
    pe_place_tile_441f00(&mut state.city, 70, 30, RUNWAY_BUILDING_ID);
    pe_place_tile_441f00(&mut state.city, 71, 30, RUNWAY_BUILDING_ID);
    // Residential tax 0 → recreation arm does not fire.
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x3c));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x36));
    assert!(
        !matches!(state.city.pe_newspaper_story_id_at(0), Some(0x37..=0x3a)),
        "unexpected recreation story"
    );
}

#[test]
fn pe_monthly_condition_seaport_uses_dw_tile_count_like_471d70() {
    // PE @ `0x471d70`: crane score from `dwTileCount[0xe0]`, not a live map rescan.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-seaport-dw.sc2", "SeaDw").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 1);
    let ind = budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("ind tax");
    state.city.set_misc_u32_at(ind, 20_000); // threshold 2
    stamp_complete_building_for_test(&mut state.city, 70, 30, SEAPORT_CRANE_BUILDING_ID, 3);
    state.city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize] = 0;
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x35));
}

#[test]
fn pe_monthly_condition_airport_uses_dw_tile_count_like_471dc4() {
    // PE @ `0x471dc4`: runway score from `dwTileCount[0xdd]`/`[0xde]`.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-airport-dw.sc2", "AirDw").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    let com = budget_row_misc_offset(1, BUDGET_SERVICE_CURRENT_FIELD).expect("com tax");
    state.city.set_misc_u32_at(com, 4_000); // threshold 2
    state.city.set_building(70, 30, RUNWAY_BUILDING_ID);
    state.city.set_building(71, 30, RUNWAY_BUILDING_ID);
    assert_eq!(state.city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize], 0);
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x3c));
}

#[test]
fn pe_fa_network_overlay_count_matches_destroy_building_4cc4d8_bands() {
    // PE load census / DestroyBuilding `0x4cc4d8`: XTXT 0xfa on network-class XBLD.
    let mut city = City::new_blank("/tmp/sc2000-fa-net.sc2", "FaNet").expect("blank");
    city.set_building(20, 20, 0x1d);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x70); // not a network band
    city.set_text_overlay(21, 20, 0xfa);
    city.set_building(22, 20, 0x3f);
    city.set_text_overlay(22, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 2);
}

#[test]
fn pe_monthly_condition_story_skips_airport_when_fa_network_covers_tax() {
    // PE `0x4cc4d8` FA-on-network tiles count toward airport_score with runways.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-airport-fa.sc2", "AirFa").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    let com = budget_row_misc_offset(1, BUDGET_SERVICE_CURRENT_FIELD).expect("com tax");
    state.city.set_misc_u32_at(com, 4_000); // threshold 2
    state.city.set_building(20, 20, 0x1d);
    state.city.set_text_overlay(20, 20, 0xfa);
    state.city.set_building(21, 20, 0x1e);
    state.city.set_text_overlay(21, 20, 0xfa);
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x3c));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x36));
}

#[test]
fn mac68k_monthly_condition_story_skips_airport_when_fa_network_covers_tax() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-airport-fa-mac.sc2", "AirFaMac").expect("blank"));
    state.engine_fidelity = EngineFidelity::Mac68k;
    pe_monthly_condition_cover_through_school(&mut state);
    let com = budget_row_misc_offset(1, BUDGET_SERVICE_CURRENT_FIELD).expect("com tax");
    state.city.set_misc_u32_at(com, 4_000);
    state.city.set_building(20, 20, 0x40);
    state.city.set_text_overlay(20, 20, 0xfa);
    state.city.set_building(21, 20, 0x41);
    state.city.set_text_overlay(21, 20, 0xfa);
    let population = PopulationStats::default();
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x3c));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x36));
}

#[test]
fn military_base_offer_accept_places_army_plot_with_road_strips() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    // Mixed altitude everywhere so any MilitaryBasePlotCheck sample is Army (not AF).
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if (x + y) % 3 == 0 {
                state
                    .city
                    .set_altitude(x, y, BLANK_MAP_BASE_HEIGHT + 1, BLANK_MAP_BASE_HEIGHT + 1);
            }
        }
    }
    state.open_military_base_dialog();

    assert!(state.accept_military_base_offer());
    assert_eq!(
        state.city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET),
        Some(MILITARY_BASE_ARMY)
    );
    assert_eq!(state.query_line, "Army Base Established");

    let (ox, oy) = (0..CITY_HEIGHT)
        .flat_map(|y| (0..CITY_WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            state
                .city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
        })
        .min_by_key(|&(x, y)| (y, x))
        .expect("army plot");

    let strip_roads = (oy..oy.saturating_add(MILITARY_BASE_PLOT_SIZE).min(CITY_HEIGHT))
        .flat_map(|y| {
            (ox..ox.saturating_add(MILITARY_BASE_PLOT_SIZE).min(CITY_WIDTH)).map(move |x| (x, y))
        })
        .filter(|&(x, y)| {
            let building = state.city.building_at(x, y);
            is_road_tile(building)
                || matches!(
                    building,
                    RUNWAY_BUILDING_ID | AIRPORT_RUNWAY_CROSS_BUILDING_ID
                )
        })
        .count();
    assert!(
        strip_roads >= 16,
        "expected army road strips, got {strip_roads} road/runway tiles"
    );
    assert!(
        (oy..oy.saturating_add(MILITARY_BASE_PLOT_SIZE).min(CITY_HEIGHT)).any(|y| {
            (ox..ox.saturating_add(MILITARY_BASE_PLOT_SIZE).min(CITY_WIDTH)).any(|x| {
                matches!(
                    state.city.building_at(x, y),
                    RUNWAY_BUILDING_ID | AIRPORT_RUNWAY_CROSS_BUILDING_ID
                )
            })
        }),
        "expected runway markers on army strip ends"
    );
}

#[test]
fn airport_growth_thresholds_advance_from_runway() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(select_airport_growth_building(&city), RUNWAY_BUILDING_ID);

    // PE civilian DoAirPortGrowth reads `dwTileCount` runway words.
    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = 5;
    assert_eq!(
        select_airport_growth_building(&city),
        CIV_CONTROL_TOWER_BUILDING_ID
    );
}

#[test]
fn seaport_growth_thresholds_advance_from_crane() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(
        select_seaport_growth_building(&city),
        SEAPORT_CRANE_BUILDING_ID
    );

    city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize] = 1;
    assert_eq!(
        select_seaport_growth_building(&city),
        CIV_LOADING_BAY_BUILDING_ID
    );
}

#[test]
fn ambient_airplane_takeoff_direction_follows_rotation_and_flip() {
    assert_eq!(
        ambient_airplane_takeoff_direction(0, false),
        XTHG_DIRECTION_NORTH
    );
    assert_eq!(
        ambient_airplane_takeoff_direction(0, true),
        XTHG_DIRECTION_EAST
    );
    assert_eq!(
        ambient_airplane_takeoff_direction(1, false),
        XTHG_DIRECTION_EAST
    );
    assert_eq!(
        ambient_airplane_takeoff_direction(1, true),
        XTHG_DIRECTION_NORTH
    );
}

#[test]
fn pe_helicopter_spawn_forces_lane_2() {
    // PE SpawnHelicopter `FUN_00452ba0` @ `0x452c0d`: lane/`+1` is always 2.
    for direction in 0u8..8 {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_zone(20, 20, ZONE_AIRPORT);
        city.set_building(20, 20, RUNWAY_BUILDING_ID);
        assert!(city.place_ambient_port_actor(20, 20, XTHG_HELICOPTER, direction));
        let (_, record) = city
            .ambient_port_actor_at(20, 20)
            .expect("ambient helicopter");
        assert_eq!(
            record.lane, 2,
            "caller direction {direction} must not stick; PE forces lane 2"
        );
    }

    for rotation in 0u8..4 {
        for &flipped in &[false, true] {
            let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
            city.set_zone(20, 20, ZONE_AIRPORT);
            city.set_building(20, 20, RUNWAY_BUILDING_ID);
            power_adjacent_building_for_test(&mut city, 20, 19);
            city.set_view_rotation(rotation);
            city.set_tile_bits(
                20,
                20,
                TileBits::new(if flipped { 0b0000_0010 } else { 0 }).with_powered(true),
            );

            let mut seed_case = None;
            for day in 0..512u32 {
                let clock = GameClock::default_start().wrapping_add_days(day);
                let mut expected = MacToolboxRng::new(
                    weather_advance_seed(&city, clock).wrapping_add(0x5854_4847),
                );
                if !expected.roll_ambient_spawn_branch() {
                    continue;
                }
                if !expected.roll_mod_is_zero(30) {
                    continue;
                }
                if !expected.roll_mod_less_than(10, 4) {
                    continue;
                }
                seed_case = Some(clock);
                break;
            }

            let Some(clock) = seed_case else {
                panic!("no helicopter spawn case for rotation={rotation}, flipped={flipped}");
            };
            let mut state = test_state(city);
            state.game_clock = clock;

            assert_eq!(
                state.spawn_ambient_port_traffic_at_for_test(20, 20, false),
                1
            );
            let (_, record) = state
                .city
                .ambient_port_actor_at(20, 20)
                .expect("ambient helicopter actor");
            assert_eq!(record.kind, XTHG_HELICOPTER);
            assert_eq!(
                record.lane, 2,
                "airport heli spawn ignores rotation={rotation} flip={flipped}"
            );
        }
    }
}

#[test]
fn powered_runway_can_spawn_ambient_airplane() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_AIRPORT);
    city.set_building(20, 20, RUNWAY_BUILDING_ID);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_saved_powered_xbit_for_test(&mut city, 20, 20);
    let mut state = test_state(city);

    assert_eq!(
        state.spawn_ambient_port_traffic_at_for_test(20, 20, true),
        1
    );
    let (_, record) = state
        .city
        .ambient_port_actor_at(20, 20)
        .expect("airplane actor");
    assert_eq!(record.kind, XTHG_AIRPLANE);
    assert_eq!(state.city.ambient_port_actor_count(XTHG_AIRPLANE), 1);
}

#[test]
fn growth_tick_port_spawn_airplane_advances_shared_process_crt() {
    // PE SpawnAeroplane edge/airport arms consume process CRT (`0x48cdd8`).
    // Growth-tick must pass budget_rng into place_ambient, not a discarded fork.
    let mut city = City::new_blank("/tmp/sc2000-port-crt.sc2", "PortCrt").expect("blank");
    city.set_zone(20, 20, ZONE_AIRPORT);
    city.set_building(20, 20, RUNWAY_BUILDING_ID);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_saved_powered_xbit_for_test(&mut city, 20, 20);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(0x4528_3001);
    let before = crt.seed();
    assert_eq!(
        state.spawn_ambient_port_traffic_at_with_crt_for_test(20, 20, true, &mut crt),
        1
    );
    assert_ne!(
        crt.seed(),
        before,
        "SpawnAeroplane must consume process CRT"
    );
    assert_eq!(state.city.ambient_port_actor_count(XTHG_AIRPLANE), 1);
}

#[test]
fn unpowered_runway_does_not_spawn_ambient_airplane() {
    // PE @ `0x43664e`: no XBIT `0x40` → skip spawn.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_AIRPORT);
    city.set_building(20, 20, RUNWAY_BUILDING_ID);
    let mut state = test_state(city);

    assert_eq!(
        state.spawn_ambient_port_traffic_at_for_test(20, 20, true),
        0
    );
    assert_eq!(state.city.ambient_port_actor_count(XTHG_AIRPLANE), 0);
}

#[test]
fn transport_xbit_gate_reads_saved_tile_bit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_budget_transport_xbit_powered(&city, 20, 20));
    city.set_tile_bits(20, 20, TileBits::new(0).with_powered(true));
    assert!(pe_budget_transport_xbit_powered(&city, 20, 20));
}

#[test]
fn crane_can_spawn_ambient_cargo_ship() {
    // Offline place_ambient (no CRT) still seeds on the crane tile.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(22, 20, ZONE_SEAPORT);
    city.set_building(22, 20, SEAPORT_CRANE_BUILDING_ID);
    city.set_terrain(22, 21, 0x10); // water south of crane
    assert!(city.place_ambient_port_actor(
        22,
        20,
        XTHG_CARGO_SHIP,
        ambient_spawn_direction(XTHG_CARGO_SHIP, &city, 22, 20, XTHG_DIRECTION_EAST),
    ));
    let (_, record) = city.ambient_port_actor_at(22, 20).expect("ship actor");
    assert_eq!(record.kind, XTHG_CARGO_SHIP);
    assert_eq!(record.lane, 4, "spawn heading should face adjacent water");
}

#[test]
fn spawn_ship_crt_edge_arm_picks_map_border_water() {
    // PE SpawnShip `FUN_00452d00` @ `0x452d18`: rand()&3 border XTER 0x10, not crane.
    let mut city = City::new_blank("/tmp/sc2000-ship-edge.sc2", "ShipEdge").expect("blank");
    city.set_zone(22, 20, ZONE_SEAPORT);
    city.set_building(22, 20, SEAPORT_CRANE_BUILDING_ID);
    // Arm 3 scans x=0x7e; last y with XTER 0x10 wins.
    city.set_terrain(0x7e, 40, 0x10);
    city.set_terrain(0x7e, 55, 0x10);
    let mut state = test_state(city);
    // Force CRT stream: first cargo roll is rand()&3 → arm 3 when low bits == 3.
    // Win95CrtRand::new seed chosen so first random_u16()&3 == 3.
    let mut probe = Win95CrtRand::new(1);
    let mut seed = 1u32;
    while (probe.random_u16() & 3) != 3 {
        seed = seed.wrapping_add(1);
        probe = Win95CrtRand::new(seed);
    }
    state.pe_process_crt = Win95CrtRand::new(seed);
    assert_eq!(
        state.spawn_ambient_port_traffic_at_for_test(22, 20, true),
        1
    );
    let mut expected_crt = Win95CrtRand::new(seed);
    let _ = expected_crt.random_u16();
    assert_eq!(
        state.pe_process_crt.seed(),
        expected_crt.seed(),
        "SpawnShip consumes exactly its one process-CRT border-arm draw"
    );
    assert!(
        state.city.ambient_port_actor_at(22, 20).is_none(),
        "ship must not remain on the crane tile"
    );
    let (_, record) = state
        .city
        .ambient_port_actor_at(0x7e, 55)
        .expect("border ship");
    assert_eq!(record.kind, XTHG_CARGO_SHIP);
    assert_eq!(record.tile_x, 0x7e);
    assert_eq!(record.tile_y, 55);
    assert_eq!(
        record.lane,
        pe_xthg_direction_toward(0x7e, 55, 22, 20),
        "lane aims at crane via FUN_004515e0"
    );
}

#[test]
fn seaport_crane_spawn_inner_crt_gate_matches_pe_43653f() {
    // PE SimulationGrow seaport @ `0x43652d`: nonzero outer CRT falls through to crane
    // `0xe0` (`0x43652f`); inner `test al, 3` @ `0x43653f` `jne`s away unless == 0,
    // then `0x402829` → SpawnShip. Growth-tick already spent the outer draw.
    let mut city = City::new_blank("/tmp/sc2000-seaport-crt.sc2", "SeaCrt").expect("blank");
    city.set_zone(22, 20, ZONE_SEAPORT);
    city.set_building(22, 20, SEAPORT_CRANE_BUILDING_ID);
    // Water on every SpawnShip border arm so the post-gate arm roll always succeeds.
    city.set_terrain(40, 2, 0x10);
    city.set_terrain(40, 0x7e, 0x10);
    city.set_terrain(2, 40, 0x10);
    city.set_terrain(0x7e, 40, 0x10);

    let mut admit_seed = None;
    let mut refuse_seed = None;
    for seed in 1..4096u32 {
        let mut probe = Win95CrtRand::new(seed);
        let inner = probe.random_u16() & 3;
        if admit_seed.is_none() && inner == 0 {
            admit_seed = Some(seed);
        }
        if refuse_seed.is_none() && inner != 0 {
            refuse_seed = Some(seed);
        }
        if admit_seed.is_some() && refuse_seed.is_some() {
            break;
        }
    }
    let admit_seed = admit_seed.expect("admit seed");
    let refuse_seed = refuse_seed.expect("refuse seed");

    let mut state = test_state(city.clone());
    let mut crt = Win95CrtRand::new(refuse_seed);
    assert_eq!(
        state.spawn_ambient_port_traffic_at_with_crt_branch_for_test(22, 20, false, true, &mut crt),
        0,
        "inner (rand&3)!=0 must refuse SpawnShip"
    );
    assert_eq!(state.city.ambient_port_actor_count(XTHG_CARGO_SHIP), 0);

    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(admit_seed);
    assert_eq!(
        state.spawn_ambient_port_traffic_at_with_crt_branch_for_test(22, 20, false, true, &mut crt),
        1,
        "inner (rand&3)==0 must call SpawnShip"
    );
    assert_eq!(state.city.ambient_port_actor_count(XTHG_CARGO_SHIP), 1);
}

#[test]
fn seaport_non_crane_tile_skips_spawn_branch_pe_43652f() {
    // PE @ `0x43652f`: `cmp bx, 0xe0` / `jne` exit — only crane tiles spawn.
    let mut city = City::new_blank("/tmp/sc2000-seaport-bay.sc2", "SeaBay").expect("blank");
    city.set_zone(22, 20, ZONE_SEAPORT);
    city.set_building(22, 20, MILITARY_WAREHOUSE_BUILDING_ID);
    city.set_terrain(0x7e, 55, 0x10);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(
        state.spawn_ambient_port_traffic_at_with_crt_branch_for_test(22, 20, false, true, &mut crt),
        0
    );
    assert_eq!(state.city.ambient_port_actor_count(XTHG_CARGO_SHIP), 0);
}

#[test]
fn military_runway_does_not_spawn_civilian_ambient_aircraft() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_MILITARY);
    city.set_building(20, 20, RUNWAY_BUILDING_ID);
    power_adjacent_building_for_test(&mut city, 20, 19);
    let mut state = test_state(city);

    assert_eq!(
        state.spawn_ambient_port_traffic_at_for_test(20, 20, true),
        0
    );
    assert_eq!(state.city.ambient_port_actor_count(XTHG_AIRPLANE), 0);
    assert_eq!(state.city.ambient_port_actor_count(XTHG_HELICOPTER), 0);
}

#[test]
fn pe_attitude_score0_is_traffic_over_tax_bases_plus_one() {
    assert_eq!(
        crate::city::pe_attitude_education_divisor(0),
        1,
        "0x413fb6: 0/4 + 1"
    );
    assert_eq!(
        crate::city::pe_attitude_education_divisor(1),
        1,
        "trunc /4, not ceil; 1/4 + 1"
    );
    assert_eq!(crate::city::pe_attitude_education_divisor(4), 2);
    assert_eq!(
        crate::city::pe_attitude_education_divisor(5),
        2,
        "5/4 + 1 (old ceil formula wrongly gave 3)"
    );
    assert_eq!(crate::city::pe_attitude_education_divisor(1_000), 251);

    assert_eq!(
        crate::city::pe_attitude_scores_from_inputs(1_000, [100, 100, 100], 100, 100, 100, 0)[0],
        3,
        "1000 / (300+1) = 3"
    );
    assert_eq!(
        crate::city::pe_attitude_scores_from_inputs(50, [0, 0, 0], 40, 60, 50, 0),
        [50, 40, 60, 50],
        "sites=0 → divisor 1; pollution/land/crime totals pass through"
    );
    assert_eq!(
        crate::city::pe_attitude_scores_from_inputs(0, [0, 0, 0], 1_000, 500, 250, 1_000),
        [0, 3, 1, 0],
        "demands ÷ (1000/4)+1 = ÷251"
    );
    assert_eq!(
        crate::city::pe_attitude_scores_from_inputs(0, [0, 0, 0], 40, 60, 50, 1),
        [0, 40, 60, 50],
        "developed=1 → divisor 1 (not ceil 2)"
    );
    assert_eq!(
        crate::city::pe_attitude_scores_from_inputs(
            u32::MAX,
            [u32::MAX, 1, 0],
            u32::MAX,
            80_000,
            70_000,
            0,
        ),
        [u32::MAX, u32::MAX, 80_000, 70_000],
        "UpdateGraphData uses wrapping dword sums and keeps each divided score as a dword"
    );

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_pe_traffic_count(1_000);
    city.set_pe_attitude_tax_bases([200, 50, 50]);
    city.recompute_pe_attitude_scores(100, 80, 70);
    assert_eq!(city.pe_attitude_score_at(0), 3, "1000/(300+1)");
    assert_eq!(city.pe_attitude_score_at(1), 100, "school funding fallback");
    assert_eq!(
        city.pe_attitude_score_at(2),
        70,
        "police funding → other demand"
    );
    assert_eq!(city.pe_attitude_score_at(3), 80, "health funding → seed");

    city.set_pe_attitude_demand_inputs(4, 200, 100, 50);
    city.recompute_pe_attitude_scores(100, 100, 100);
    assert_eq!(city.pe_attitude_score_at(1), 100, "pollution 200 / 2");
    assert_eq!(city.pe_attitude_score_at(2), 50, "land value 100 / 2");
    assert_eq!(city.pe_attitude_score_at(3), 25, "crime 50 / 2");

    city.refresh_structure_microsim_yearly(10_000, 70, 100, 80, 100, 100);
    assert_eq!(
        city.pe_attitude_score_at(0),
        3,
        "January poll recomputes attitude 0 from stored traffic/tax bases"
    );
}

#[test]
fn january_transit_occupancy_copies_passenger_words() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 2, 2, BUS_DEPOT_BUILDING_ID, 2);
    stamp_complete_building_for_test(&mut city, 8, 2, BUS_DEPOT_BUILDING_ID, 2);
    stamp_complete_building_for_test(&mut city, 14, 2, RAIL_STATION_BUILDING_ID, 2);
    stamp_complete_building_for_test(&mut city, 20, 2, RAIL_STATION_BUILDING_ID, 2);
    city.set_building(26, 2, SUBWAY_STATION_BUILDING_ID);
    city.set_building(27, 2, SUBWAY_STATION_BUILDING_ID);
    // Raw set_building writes XBLD only; PE subway microsim reads the BSS
    // dwTileCount word directly and does not derive it from XBLD at dispatch.
    city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 2;
    city.provision_microsim_for_building(236).expect("bus");
    city.provision_microsim_for_building(237).expect("rail");
    city.provision_microsim_for_building(233).expect("subway");
    city.set_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, 99);
    city.set_misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET, 99);
    city.set_misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET, 99);
    // Microsim prefers PE BSS over a stale MISC snapshot.
    city.set_pe_dw_bus_passengers(1_200);
    city.set_pe_dw_rail_passengers(45);
    city.set_pe_dw_subway_passengers(9);

    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let bus = city
        .microsim_at(crate::city::MICROSIM_SYSTEM_BUS)
        .expect("bus row");
    let rail = city
        .microsim_at(crate::city::MICROSIM_SYSTEM_RAIL)
        .expect("rail row");
    let subway = city
        .microsim_at(crate::city::MICROSIM_SYSTEM_SUBWAY)
        .expect("subway row");
    assert_eq!(
        (bus.stat1, bus.stat2, bus.stat3),
        (2, 8, 1_200),
        "PE bus: tiles/4 stations, raw tiles buses, dwBusPassengers"
    );
    assert_eq!(
        (rail.stat1, rail.stat3),
        (2, 45),
        "PE rail: tiles/4 stations, dwRailPassengers"
    );
    assert_eq!(
        (subway.stat1, subway.stat3),
        (2, 9),
        "PE subway: raw tiles, dwSubwayPassengers"
    );
}

#[test]
fn query_shows_transit_occupancy_from_xmic() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, BUS_DEPOT_BUILDING_ID, 2);
    stamp_complete_building_for_test(&mut city, 20, 10, RAIL_STATION_BUILDING_ID, 2);
    city.set_building(30, 10, SUBWAY_STATION_BUILDING_ID);
    // The PE subway annual arm reads its BSS tile counter directly; setting
    // this one XBLD byte does not update that counter.
    city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 1;
    let bus_xtxt = city.provision_microsim_for_building(236).expect("bus");
    let rail_xtxt = city.provision_microsim_for_building(237).expect("rail");
    let subway_xtxt = city.provision_microsim_for_building(233).expect("subway");
    city.set_text_overlay(10, 10, bus_xtxt);
    city.set_text_overlay(20, 10, rail_xtxt);
    city.set_text_overlay(30, 10, subway_xtxt);
    city.set_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, 1_200);
    city.set_misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET, 45);
    city.set_misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET, 9);
    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);

    let bus = query_tile_status_line(&city, 10, 10, false);
    let rail = query_tile_status_line(&city, 20, 10, false);
    let subway = query_tile_status_line(&city, 30, 10, false);
    assert!(
        bus.contains("Bus Stations 1")
            && bus.contains("Buses 4")
            && bus.contains("Passengers/day 1,200"),
        "{bus}"
    );
    assert!(
        rail.contains("Rail Stations 1") && rail.contains("Passengers/day 45"),
        "{rail}"
    );
    assert!(
        subway.contains("Subway Stations 1") && subway.contains("Passengers/day 9"),
        "{subway}"
    );
}

#[test]
fn powered_rail_depot_can_spawn_ambient_train() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 20, 20, RAIL_STATION_BUILDING_ID, 2);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_saved_powered_xbit_for_test(&mut city, 20, 20);
    // PE tries the station faces N/E/S/W, so the east rail starts the train.
    city.set_building(19, 20, 45);
    city.set_building(22, 20, 45);
    city.set_building(23, 20, 45);
    let mut state = test_state(city);
    let before_lfsr = state.pe_process_lfsr_state();

    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(20, 20, true),
        1
    );
    assert_ne!(
        state.pe_process_lfsr_state(),
        before_lfsr,
        "train spawn cardinal should consume pe_process_lfsr"
    );
    let (_, record) = state
        .city
        .ambient_port_actor_at(22, 20)
        .expect("train actor");
    assert_eq!(record.kind, XTHG_TRAIN_ENGINE);
    assert_eq!(
        record.lane, 1,
        "PE train lane stores east as cardinal index 1"
    );
    assert_eq!(state.city.ambient_active_train_count(), 1);
    assert_eq!(state.city.ambient_port_actor_count(XTHG_TRAIN_CAR), 2);
}

#[test]
fn rail_depot_train_spawn_respects_active_train_capacity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 20, 20, RAIL_STATION_BUILDING_ID, 2);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_saved_powered_xbit_for_test(&mut city, 20, 20);
    let mut state = test_state(city);
    assert!(
        state
            .city
            .place_ambient_port_actor(20, 20, XTHG_TRAIN_ENGINE, XTHG_DIRECTION_EAST)
    );

    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(20, 20, true),
        0,
        "one 2x2 depot only supports one active train"
    );
    assert_eq!(state.city.ambient_port_actor_count(XTHG_TRAIN_ENGINE), 1);
}

#[test]
fn rail_depot_spawn_hard_cap_is_inside_spawn_train_not_budget() {
    // PE DoBudget @ `0x435e2e` only does station_units > wActiveTrains; the
    // `cmp [0x4e99d8], 5` refuse is inside SpawnTrain (`0x453603`). With a fat
    // census and five live engines, budget still reaches SpawnTrain which fails.
    let mut city = City::new_blank("/tmp/sc2000-train-budget5.sc2", "Budg5").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, RAIL_STATION_BUILDING_ID, 2);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_saved_powered_xbit_for_test(&mut city, 20, 20);
    city.set_building(22, 20, 45);
    city.set_building(23, 20, 45);
    city.pe_dw_tile_count[RAIL_STATION_BUILDING_ID as usize] = 40; // 10 station units
    for i in 0..5 {
        let x = 40 + i * 8;
        city.set_building(x, 40, 45);
        assert!(city.place_ambient_port_actor(x, 40, XTHG_TRAIN_ENGINE, XTHG_DIRECTION_EAST));
    }
    let mut state = test_state(city);
    assert_eq!(state.city.ambient_active_train_count(), 5);
    let before_lfsr = state.pe_process_lfsr_state();
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(20, 20, true),
        0,
        "budget census gate passes; SpawnTrain hard cap refuses"
    );
    assert_eq!(state.city.ambient_active_train_count(), 5);
    assert_eq!(
        state.pe_process_lfsr_state(),
        before_lfsr,
        "SpawnTrain hard cap refuses before drawing a face direction"
    );
}

#[test]
fn powered_marina_can_spawn_ambient_sailboat() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 30, 30, MARINA_BUILDING_ID, 3);
    power_adjacent_building_for_test(&mut city, 30, 29);
    set_saved_powered_xbit_for_test(&mut city, 30, 30);
    // PE scanner `0x453380` places on a cardinal water neighbor, not the marina tile.
    city.set_tile_bits(29, 30, crate::city::TileBits::new(0x04));
    city.set_terrain(29, 30, 0x10);
    city.set_building(29, 30, 0);
    let mut state = test_state(city);
    let before_lfsr = state.pe_process_lfsr_state();
    let mut expect = crate::win95_random::Win95Lfsr::new(before_lfsr);
    let expected_lane = expect.random_mod(3) as u8;

    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(30, 30, true),
        1
    );
    assert_ne!(state.pe_process_lfsr_state(), before_lfsr);
    let (_, record) = state
        .city
        .ambient_port_actor_at(29, 30)
        .expect("sailboat actor");
    assert_eq!(record.kind, XTHG_SAILBOAT);
    assert_eq!(record.lane, expected_lane);
    assert!(record.lane < 3, "PE 0x401cad(3) remainder");
}

#[test]
fn marina_sailboat_capacity_gates_on_budget_not_live_count() {
    // PE @ `0x435298`: marina boats/9 compares to word[0x4e99d0], not live XTHG count.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 30, 30, MARINA_BUILDING_ID, 3);
    power_adjacent_building_for_test(&mut city, 30, 29);
    set_saved_powered_xbit_for_test(&mut city, 30, 30);
    city.set_tile_bits(29, 30, crate::city::TileBits::new(0x04));
    city.set_terrain(29, 30, 0x10);
    city.set_building(29, 30, 0);
    // Four live sailboats with budget still 0 must not block marina spawn.
    for x in 10..14 {
        city.set_terrain(x, 20, 0x10);
        let slot = city
            .traffic_overlays_mut()
            .expect("xthg")
            .iter()
            .enumerate()
            .skip(crate::win95_overlay::XTHG_FIRST_USABLE_SLOT)
            .find(|(_, r)| r.kind == 0)
            .map(|(s, _)| s)
            .expect("free slot");
        if let Some(overlays) = city.traffic_overlays_mut() {
            overlays[slot] = crate::win95_overlay::Win95OverlayRecord {
                kind: XTHG_SAILBOAT,
                lane: 1,
                tile_x: x as u8,
                tile_y: 20,
                sub_x: 4,
                sub_y: 4,
                ..Default::default()
            };
        }
    }
    assert_eq!(city.ambient_port_actor_count(XTHG_SAILBOAT), 4);
    assert_eq!(city.pe_sailboat_budget, 0);
    let mut state = test_state(city);
    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(30, 30, true),
        1,
        "budget 0 must allow marina spawn despite 4 live sailboats"
    );
    assert_eq!(state.city.pe_sailboat_budget, 1);

    // Budget already at marina capacity refuses even with no live actors on the water tile.
    let mut city2 = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city2, 30, 30, MARINA_BUILDING_ID, 3);
    power_adjacent_building_for_test(&mut city2, 30, 29);
    set_saved_powered_xbit_for_test(&mut city2, 30, 30);
    city2.set_tile_bits(29, 30, crate::city::TileBits::new(0x04));
    city2.set_terrain(29, 30, 0x10);
    city2.set_building(29, 30, 0);
    // One 3x3 marina → boats/9 == 1; budget 1 → jng refuse.
    city2.pe_sailboat_budget = 1;
    let mut state2 = test_state(city2);
    assert_eq!(
        state2.spawn_microsim_transit_actor_at_for_test(30, 30, true),
        0,
        "budget >= marina_units must refuse"
    );
    assert_eq!(state2.city.ambient_port_actor_at(29, 30), None);
}

#[test]
fn unpowered_marina_does_not_spawn_sailboat() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 30, 30, MARINA_BUILDING_ID, 3);
    let mut state = test_state(city);

    assert_eq!(
        state.spawn_microsim_transit_actor_at_for_test(30, 30, true),
        0
    );
    assert_eq!(state.city.ambient_port_actor_count(XTHG_SAILBOAT), 0);
}

#[test]
fn win95_subway_upkeep_uses_raw_funding_word_and_keeps_crt_draw() {
    // PE DoBudgetSubwayCheck @ `0x436855..0x43687c` reads the raw low WORD;
    // clamping 120 to 100 would incorrectly skip the shared CRT draw.
    let mut city =
        City::new_blank("/tmp/sc2000-raw-subway-budget.sc2", "RawSubway").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 120);
    let subway = underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT);
    city.set_underground(20, 20, subway);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(1);
    let mut expected_rng = Win95CrtRand::new(1);
    assert!(!expected_rng.roll_pe_underfunding_decay(120));

    assert_eq!(budget_service_funding_percent_for_city(&state.city, 9), 100);
    assert_eq!(budget_service_funding_raw_for_city(&state.city, 9), 120);
    assert_eq!(state.decay_subway_at_for_test(20, 20), 0);
    assert_eq!(state.city.underground_at(20, 20), subway);
    assert_eq!(
        state.pe_process_crt_seed(),
        expected_rng.seed(),
        "PE consumes rand for raw funding 120 even though the subway remains intact"
    );
}

#[test]
fn growth_tick_aggregate_credits_abandoned_without_demand_or_trip() {
    // PE @ `0x4361c9` credits slot 7 before the redevelop roll; trip/demand are not gates.
    let mut city = City::new_blank("/tmp/sc2000-class2-nodemand.sc2", "Class2NoD").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, 0x8A);
    set_rci_demand_for_test(&mut city, 0, 0, 0);
    let mut state = test_state(city);
    state.sim_calendar_one_growth_tick(0, 0);
    assert_eq!(
        state.city.building_at(20, 20),
        0x8A,
        "no demand → no redevelop"
    );
    assert_eq!(
        state.pe_growth_aggregate.slots()[7],
        pe_growth_aggregate_weight(1),
        "class-2 still credits slot 7 without StartBuilding gates"
    );
    assert_eq!(state.pe_growth_aggregate.slots()[1], 0);
}

#[test]
fn underfunded_transport_rubble_clears_xbit_powerable_like_pe_435dd6() {
    // PE DoBudgetRoad/RailCheck: PlaceTile rubble (`0x441f00`) then AND XBIT with 0x7F.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    stamp_xbit_powerable(&mut city, 20, 20);
    city.set_traffic_density(20, 20, 0x41);
    city.set_tile_bits(
        20,
        20,
        city.tile_bits_at(20, 20)
            .unwrap_or_default()
            .with_piped(true)
            .with_watered(true),
    );
    assert!(pe_power_flood_tile_accepted(&city, 20, 20));
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert!(!pe_power_flood_tile_accepted(&state.city, 20, 20));
    assert!(
        !state
            .city
            .tile_bits_at(20, 20)
            .is_some_and(TileBits::powerable)
    );
    // PlaceTile does not invent-clear watered/piped or write XTRF.
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x41));
    let bits = state.city.tile_bits_at(20, 20).expect("bits");
    assert!(bits.piped() && bits.watered());
}

#[test]
fn rubble_transport_tile_preserves_xtxt_place_tile_only() {
    // PE DoBudgetRoad/RailCheck @ `0x435d9c`/`0x435d2f`: PlaceTile rubble only —
    // no DestroyBuilding, so XTXT survives.
    let mut city = City::new_blank("/tmp/sc2000-rubble-xtxt.sc2", "RubbleXtxt").expect("blank");
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_text_overlay(20, 20, 0x20);
    city.set_traffic_density(20, 20, 0x22);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    state.rubble_transport_tile(20, 20, &mut crt);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x20));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x22));
}

#[test]
fn rubble_transport_tile_single_crt_rubble_place_tile() {
    // PE @ `0x435d9c`: one CRT `&3+1` PlaceTile — no DestroyBuilding roll, no
    // highway E/S/SE neighbor PlaceTiles (`0x435b74`+ is highway-only).
    let mut city =
        City::new_blank("/tmp/sc2000-rubble-crt-order.sc2", "RubbleCrtOrder").expect("blank");
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);
    let mut preview = Win95CrtRand::new(0x435d_9c01);
    let expected_main = preview.rubble_tile_id();
    let mut crt = Win95CrtRand::new(0x435d_9c01);
    state.rubble_transport_tile(20, 20, &mut crt);
    assert_eq!(state.city.building_at(20, 20), expected_main);
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn rubble_transport_tile_water_still_gets_crt_rubble() {
    // PE road/rail arms have no XBIT water → PlaceTile(0) branch (that is highway
    // @ `0x435b55`). Road still CRT-rubble PlaceTiles the center.
    let mut city = City::new_blank("/tmp/sc2000-rubble-water.sc2", "RubbleWat").expect("blank");
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0).with_water_tile(true).with_piped(true),
    );
    city.set_text_overlay(20, 20, 0x20);
    city.set_traffic_density(20, 20, 0x22);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    let expected = Win95CrtRand::new(1).rubble_tile_id();
    state.rubble_transport_tile(20, 20, &mut crt);
    assert_eq!(state.city.building_at(20, 20), expected);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x20));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x22));
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
    assert!(state.city.tile_bits_at(20, 20).unwrap().piped());
}

#[test]
fn rubble_transport_tile_does_not_place_tile_neighbors() {
    // Highway @ `0x435b74`+ PlaceTiles E/S/SE of the NW cell; road/rail do not.
    let mut city = City::new_blank("/tmp/sc2000-rubble-neigh.sc2", "RubbleN").expect("blank");
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_building(21, 20, 6);
    city.set_building(20, 21, 7);
    city.set_building(21, 21, 8);
    city.set_text_overlay(21, 20, 0x4b);
    city.set_traffic_density(21, 20, 0x37);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    state.rubble_transport_tile(20, 20, &mut crt);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.building_at(21, 20), 6);
    assert_eq!(state.city.building_at(20, 21), 7);
    assert_eq!(state.city.building_at(21, 21), 8);
    assert_eq!(state.city.text_overlay_at(21, 20), Some(0x4b));
    assert_eq!(state.city.traffic_density_at(21, 20), Some(0x37));
}

#[test]
fn rubble_transport_tile_leaves_neighbor_road_mask() {
    // PE @ `0x435daa`→`0x435dcc`→`0x435dda`: no invent PlaceRoads reshape on NESW.
    let mut city = City::new_blank("/tmp/sc2000-rubble-mask.sc2", "RubbleM").expect("blank");
    let lr = network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT);
    city.set_building(20, 20, lr);
    city.set_building(21, 20, lr);
    city.set_building(19, 20, lr);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    state.rubble_transport_tile(20, 20, &mut crt);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.building_at(21, 20), lr);
    assert_eq!(state.city.building_at(19, 20), lr);
}

#[test]
fn rubble_transport_tile_neighbor_water_untouched() {
    let mut city = City::new_blank("/tmp/sc2000-rubble-nw.sc2", "RubbleNW").expect("blank");
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_building(21, 20, 6);
    city.set_tile_bits(21, 20, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    state.rubble_transport_tile(20, 20, &mut crt);
    assert_eq!(state.city.building_at(21, 20), 6);
    assert!(state.city.tile_bits_at(21, 20).unwrap().water_tile());
}

#[test]
fn mac68k_rubble_transport_tile_does_not_place_tile_neighbors() {
    let mut city = City::new_blank("/tmp/sc2000-mac-rubble-neigh.sc2", "MacRubN").expect("blank");
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_building(21, 20, 6);
    city.set_text_overlay(21, 20, 0x4b);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let mut crt = Win95CrtRand::new(1);
    state.rubble_transport_tile(20, 20, &mut crt);
    assert_eq!(state.city.building_at(21, 20), 6);
    assert_eq!(state.city.text_overlay_at(21, 20), Some(0x4b));
}

#[test]
fn mac68k_rubble_transport_tile_water_still_gets_crt_rubble() {
    let mut city = City::new_blank("/tmp/sc2000-mac-rubble-water.sc2", "MacRubWat").expect("blank");
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let mut crt = Win95CrtRand::new(1);
    let expected = Win95CrtRand::new(1).rubble_tile_id();
    state.rubble_transport_tile(20, 20, &mut crt);
    assert_eq!(state.city.building_at(20, 20), expected);
}

#[test]
fn underfunded_subway_clears_underground_on_growth_tick() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    city.set_underground(
        20,
        20,
        underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(state.city.underground_at(20, 20), 0);
}

#[test]
fn pe_growth_skips_subway_budget_check_on_airport_and_seaport_tiles() {
    // PE zone 9 takes its port branch at `0x43651a..0x4365f0`; zone 8 takes
    // its airport branch at `0x4365f0..0x43680b`. Both reach the tile epilogue
    // without calling DoBudgetSubwayCheck (`0x43680b`).
    for zone_id in [ZONE_AIRPORT, ZONE_SEAPORT] {
        let mut city = City::new_blank("/tmp/sc2000-port-subway-budget-gate.sc2", "PortSubwayGate")
            .expect("blank city");
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_test(&mut city, index, 100);
        }
        set_budget_funding_for_test(&mut city, 9, 0);
        let subway =
            underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT);
        city.set_zone(20, 20, zone_id | ZONE_CORNER_ALL);
        city.set_underground(20, 20, subway);
        let mut state = test_state(city);
        let crt_before = state.pe_process_crt_seed();
        let lfsr_before = state.pe_process_lfsr_state();

        assert_eq!(
            state.decay_subway_at_for_test_noforce(20, 20),
            0,
            "zone {zone_id}"
        );
        assert_eq!(state.city.underground_at(20, 20), subway, "zone {zone_id}");
        assert_eq!(state.pe_process_crt_seed(), crt_before, "zone {zone_id}");
        assert_eq!(state.pe_process_lfsr_state(), lfsr_before, "zone {zone_id}");
    }
}

#[test]
fn pe_growth_only_reaches_subway_budget_check_after_zone_dispatch() {
    // `SimulationGrowthTick` jumps directly to its epilogue for unzoned tiles
    // and handled zone IDs 1..=9. Only an unclassified nonzero XZON nibble
    // falls through the final dispatch to `DoBudgetSubwayCheck` at 0x43680b.
    // This checks the caller's LFSR chronology, not just the helper's port guard.
    let mut city = City::new_blank("/tmp/sc2000-growth-subway-dispatch.sc2", "SubwayDispatch")
        .expect("blank city");
    for (x, zone_id) in [
        (20, ZONE_RESIDENTIAL_LIGHT),
        (24, ZONE_RESIDENTIAL_DENSE),
        (28, ZONE_COMMERCIAL_LIGHT),
        (32, ZONE_COMMERCIAL_DENSE),
        (36, ZONE_INDUSTRIAL_LIGHT),
        (40, ZONE_INDUSTRIAL_DENSE),
        (44, ZONE_MILITARY),
        (48, ZONE_AIRPORT),
        (52, ZONE_SEAPORT),
    ] {
        city.set_zone(x, 20, zone_id | ZONE_CORNER_ALL);
    }
    let mut state = test_state(city);
    state.pe_process_lfsr = Win95Lfsr::new(0x1234);
    let before = state.pe_process_lfsr_state();

    state.sim_calendar_one_growth_tick(0, 0);

    assert_eq!(
        state.pe_process_lfsr_state(),
        before,
        "blank, RCI, military, airport, and seaport dispatches exit before the subway check"
    );
}

#[test]
fn pe_growth_unclassified_zone_reaches_subway_budget_check_once() {
    // Zone nibble 10 misses the RCI, military, seaport, and airport arms and
    // reaches `0x43680b`. All other tiles in this phase are unzoned and exit
    // before that check.
    let mut city = City::new_blank(
        "/tmp/sc2000-growth-subway-fallthrough.sc2",
        "SubwayFallthrough",
    )
    .expect("blank city");
    city.set_zone(20, 20, 10 | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.pe_process_lfsr = Win95Lfsr::new(0x1234);
    let mut expected = Win95Lfsr::new(0x1234);
    let _ = expected.random_mask_7f();

    state.sim_calendar_one_growth_tick(0, 0);

    assert_eq!(
        state.pe_process_lfsr_state(),
        expected.state(),
        "only the unclassified zone's subway dispatch consumes RandomAnd7F"
    );
}

#[test]
fn underfunded_subway_crossover_place_xund_zero_not_pipe_stub() {
    // PE @ `0x436893`: non-entrance (incl. crossover `0x1f`/`0x20`) PlaceXUND(0) —
    // not invent pipe stub `0x11`/`0x12`.
    let mut city = City::new_blank("/tmp/sc2000-subway-xo.sc2", "SubXo").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    city.set_underground(20, 20, UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR);
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(state.city.underground_at(20, 20), 0);
}

#[test]
fn underfunded_subway_grey_box1_place_xund_zero() {
    // PE @ `0x43684a` admits XUND `0x22`; non-entrance @ `0x436893` PlaceXUND(0).
    let mut city = City::new_blank("/tmp/sc2000-subway-gb1.sc2", "SubGb1").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    city.set_underground(20, 20, UNDER_TILE_GREY_BOX_1);
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(state.city.underground_at(20, 20), 0);
}

#[test]
fn underfunded_subway_decay_leaves_neighbor_subway_mask() {
    // PE @ `0x436893`→`0x43689b`: PlaceXUND(0) then lattice step — no invent
    // neighbor `refresh_underground_tile` reshape. Probe one cell so the
    // neighbor is not also budget-cleared in the same force-walk.
    let mut city = City::new_blank("/tmp/sc2000-subway-nbr.sc2", "SubNbr").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    let lr = underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT);
    city.set_underground(20, 20, lr);
    city.set_underground(21, 20, lr);
    let mut state = test_state(city);
    assert_eq!(state.decay_subway_at_for_test(20, 20), 1);
    assert_eq!(state.city.underground_at(20, 20), 0);
    assert_eq!(state.city.underground_at(21, 20), lr);
}

#[test]
fn underfunded_subway_skips_invent_budget_misc_refresh() {
    // PE @ `0x436893`/`0x43688c` → `jmp 0x43689b`: no MISC/budget row refresh.
    let mut city = City::new_blank("/tmp/sc2000-subway-noref.sc2", "SubNoRef").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    let offset = budget_service_misc_offset(9, BUDGET_SERVICE_CURRENT_FIELD).expect("subway");
    let mut values = city.misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
    values[BUDGET_SERVICE_CURRENT_FIELD] = 0xA11;
    city.set_misc_u32_values(offset, &values);
    city.set_underground(
        20,
        20,
        underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);
    assert_eq!(state.decay_subway_at_for_test(20, 20), 1);
    assert_eq!(state.city.underground_at(20, 20), 0);
    assert_eq!(
        state.city.misc_u32_at(offset),
        Some(0xA11),
        "DoBudgetSubwayCheck must not invent-refresh budget CURRENT"
    );
}

#[test]
fn underfunded_subway_place_xund_clears_on_military_zone() {
    // PE @ `0x436893` PlaceXUND(0): military skips counter but still clears XUND.
    let mut city = City::new_blank("/tmp/sc2000-subway-mil-xund.sc2", "SubMilXu").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    city.set_zone(20, 20, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_underground(
        20,
        20,
        underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(state.city.underground_at(20, 20), 0);
}

#[test]
fn underfunded_subway_entrance_elevated_uses_destroy_building_arm_not_structure_early_out() {
    // PE DestroyBuilding elevated @ `0x45994e`: PlaceTile(0), keep XTXT.
    // DestroyStructure @ `0x46137b` would early-out and leave XBLD/XTXT.
    // PE @ `0x43687e`: entrance DestroyBuilding then jmp past PlaceXUND — leave XUND 0x23.
    let mut city = City::new_blank("/tmp/sc2000-sub-elev.sc2", "SubElev").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    city.set_underground(20, 20, UNDER_TILE_SUBWAY_ENTRANCE);
    city.set_building(20, 20, 0x3f);
    city.set_text_overlay(20, 20, 0x55);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(
        state.city.underground_at(20, 20),
        UNDER_TILE_SUBWAY_ENTRANCE
    );
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x55));
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT)
    );
}

#[test]
fn underfunded_subway_entrance_destroy_clears_xtxt() {
    // PE DoBudgetSubwayCheck @ `0x43687e`: DestroyBuilding (push 0 → `0x402211`) clears
    // surface XBLD/XTXT; jmp past PlaceXUND leaves XUND `0x23`.
    let mut city = City::new_blank("/tmp/sc2000-subway-xtxt.sc2", "SubXtxt").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    city.set_underground(20, 20, UNDER_TILE_SUBWAY_ENTRANCE);
    city.set_building(20, 20, SUBWAY_STATION_BUILDING_ID);
    city.set_text_overlay(20, 20, 0x55);
    city.set_traffic_density(20, 20, 0x77);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(
        state.city.underground_at(20, 20),
        UNDER_TILE_SUBWAY_ENTRANCE
    );
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x77));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped() && bits.watered());
}

#[test]
fn underfunded_subway_entrance_destroy_consumes_crt_like_pe() {
    // PE @ `0x43687e`: DestroyBuilding CRT PlaceTile rubble on subway entrance surface.
    let mut city = City::new_blank("/tmp/sc2000-subway-crt.sc2", "SubCrt").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    city.set_underground(20, 20, UNDER_TILE_SUBWAY_ENTRANCE);
    city.set_building(20, 20, SUBWAY_STATION_BUILDING_ID);
    let mut state = test_state(city);
    let before = state.pe_process_crt_seed();
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_ne!(state.pe_process_crt_seed(), before);
}

#[test]
fn underfunded_subway_decay_preserves_watered_and_xtrf() {
    // PE DoBudgetSubwayCheck → DestroyStructure: no invent citywide water refresh / XTRF write.
    let mut city = City::new_blank("/tmp/sc2000-subway-watered.sc2", "SubWater").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    city.set_underground(
        20,
        20,
        underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x66);
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(state.city.underground_at(20, 20), 0);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x66));
}

#[test]
fn mac68k_underfunded_subway_decay_preserves_xtrf() {
    // Shared DoBudgetSubwayCheck leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-subway-xtrf.sc2", "MacSubX").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 9, 0);
    city.set_underground(
        20,
        20,
        underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_traffic_density(20, 20, 0x66);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(state.city.underground_at(20, 20), 0);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x66));
}

#[test]
fn zoned_roads_are_not_decayed_by_transport_budget() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    let road = network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT);
    city.set_building(20, 20, road);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    let mut state = test_state(city);

    assert_eq!(state.decay_transport_budget_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), road);
}

#[test]
fn mac68k_underfunded_cardinal_portal_run_is_removed_by_monthly_scan() {
    // Mac CODE 8 `export_241` @ `0x36156` → CODE 5 `export_80`:
    // subway funding admits the cardinal walk, then each portal stamp consumes
    // two QuickDraw calls even with the caller's zero render flag.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-cardinal-portal.sc2", "MacPortal").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 10, 0);

    // XBLD 0x41 scans east; any cardinal portal ID terminates the run.
    city.set_building(20, 20, 0x41);
    city.set_building(24, 20, 0x3f);
    city.set_zone(20, 20, 0xa0);
    city.set_zone(24, 20, 0xb0);
    for x in 20..=24 {
        city.set_tunnel_levels(x, 20, 0x1f);
    }
    assert_eq!(city.kind, CityFileKind::MacCity);
    assert_eq!(city.zone_at(20, 20).unwrap().raw_bits() & 0x0f, 0);
    assert_eq!(budget_service_funding_percent_for_city(&city, 10), 0);

    let seed = 0x1234_5678;
    let mut expected_rng = MacToolboxRng::new(seed);
    for _ in 0..5 {
        expected_rng.random_u16();
    }
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.building_at(24, 20), 0);
    assert_eq!(city.zone_at(20, 20).unwrap().raw_bits(), 0);
    assert_eq!(city.zone_at(24, 20).unwrap().raw_bits(), 0);
    for x in 20..=24 {
        assert_eq!(city.tunnel_levels_at(x, 20), 0, "x={x}");
    }
    assert_eq!(
        rng.seed(),
        expected_rng.seed(),
        "one gate plus two stamp pairs"
    );
}

#[test]
fn mac68k_fully_funded_cardinal_portals_skip_upkeep_and_random_draws() {
    let mut city = City::new_blank("/tmp/sc2000-mac-cardinal-portal-funded.sc2", "MacPortal100")
        .expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    city.set_building(20, 20, 0x41);
    city.set_building(24, 20, 0x42);
    for x in 20..=24 {
        city.set_tunnel_levels(x, 20, 0x1f);
    }

    let seed = 0x8765_4321;
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.building_at(20, 20), 0x41);
    assert_eq!(city.building_at(24, 20), 0x42);
    for x in 20..=24 {
        assert_eq!(city.tunnel_levels_at(x, 20), 0x1f, "x={x}");
    }
    assert_eq!(rng.seed(), MacToolboxRng::new(seed).seed());
}

#[test]
fn mac68k_late_xund_subway_gate_removes_funded_cardinal_portal_run() {
    // CODE 8 `export_241` @ `0x36c14` accepts XUND `0x23` after one subway
    // funding roll, then CODE 5 export 80 removes the portal endpoints with
    // two direct QuickDraw calls per stamp despite render flag zero.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-late-xund-portal.sc2", "MacLateXund").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 10, 0);
    city.set_building(20, 20, 0x41); // late export-80 selector walks east
    city.set_building(24, 20, 0x3f);
    city.set_zone(20, 20, 0xaa);
    city.set_zone(24, 20, 0xba);
    city.set_underground(20, 20, 0x23);
    for x in 20..=24 {
        city.set_tunnel_levels(x, 20, 0x1f);
    }

    let seed = 0x1234_5678;
    let mut expected_rng = MacToolboxRng::new(seed);
    for _ in 0..5 {
        expected_rng.random_u16();
    }
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.building_at(24, 20), 0);
    assert_eq!(city.zone_at(20, 20).unwrap().raw_bits(), 0x0a);
    assert_eq!(city.zone_at(24, 20).unwrap().raw_bits(), 0x0a);
    for x in 20..=24 {
        assert_eq!(city.tunnel_levels_at(x, 20), 0, "x={x}");
    }
    assert_eq!(rng.seed(), expected_rng.seed());

    let mut funded = City::new_blank(
        "/tmp/sc2000-mac-late-xund-portal-funded.sc2",
        "MacLateXund100",
    )
    .expect("blank");
    funded.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut funded, index, 100);
    }
    funded.set_building(20, 20, 0x41);
    funded.set_zone(20, 20, 0xaa);
    funded.set_underground(20, 20, 0x23);
    funded.set_tunnel_levels(20, 20, 0x1f);
    let seed = 0x8765_4321;
    let mut funded_rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut funded, 0, 0, &mut funded_rng);
    assert_eq!(funded.building_at(20, 20), 0x41);
    assert_eq!(funded.tunnel_levels_at(20, 20), 0x1f);
    assert_eq!(funded_rng.seed(), MacToolboxRng::new(seed).seed());
}

#[test]
fn mac_export80_cardinal_portal_teardown_covers_all_directions() {
    // CODE 5 export 80's four selector arms walk cardinally until the next
    // portal endpoint. Each endpoint and the origin use two direct QuickDraw
    // redraw calls even when invoked through the zero-render monthly caller.
    let cases = [
        (0x3f, 0x41, (-1_isize, 0_isize)), // west, then east-facing endpoint
        (0x40, 0x42, (0_isize, -1_isize)), // north, then south-facing endpoint
        (0x41, 0x3f, (1_isize, 0_isize)),  // east, then west-facing endpoint
        (0x42, 0x40, (0_isize, 1_isize)),  // south, then north-facing endpoint
    ];

    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (case_index, (origin_id, endpoint_id, (dx, dy))) in cases.into_iter().enumerate() {
            let extension = if kind == CityFileKind::MacCity {
                "city"
            } else {
                "sc2"
            };
            let mut city = City::new_blank(
                format!("/tmp/sc2000-mac-export80-portal-{kind:?}-{case_index}.{extension}"),
                "MacPortalDirection",
            )
            .expect("blank city");
            city.kind = kind;

            let origin = (64_isize, 64_isize);
            let endpoint = (origin.0 + dx * 3, origin.1 + dy * 3);
            city.set_building(origin.0 as usize, origin.1 as usize, origin_id);
            city.set_building(endpoint.0 as usize, endpoint.1 as usize, endpoint_id);
            city.set_zone(origin.0 as usize, origin.1 as usize, 0xa5);
            city.set_zone(endpoint.0 as usize, endpoint.1 as usize, 0xb6);
            for step in 0..=3 {
                let x = (origin.0 + dx * step) as usize;
                let y = (origin.1 + dy * step) as usize;
                city.set_tunnel_levels(x, y, 0x1f);
            }

            let seed = 0x51a7_20c3_u32
                .wrapping_add(case_index as u32)
                .wrapping_add((kind == CityFileKind::WinSc2) as u32 * 0x100);
            let mut expected_rng = MacToolboxRng::new(seed);
            for _ in 0..4 {
                expected_rng.random_i16();
            }
            let mut rng = MacToolboxRng::new(seed);
            assert!(mac_budget_remove_cardinal_portal_run(
                &mut city,
                origin.0 as usize,
                origin.1 as usize,
                &mut rng,
            ));

            assert_eq!(city.building_at(origin.0 as usize, origin.1 as usize), 0);
            assert_eq!(
                city.building_at(endpoint.0 as usize, endpoint.1 as usize),
                0
            );
            assert_eq!(
                city.zone_at(origin.0 as usize, origin.1 as usize)
                    .unwrap()
                    .raw_bits(),
                0x05
            );
            assert_eq!(
                city.zone_at(endpoint.0 as usize, endpoint.1 as usize)
                    .unwrap()
                    .raw_bits(),
                0x06
            );
            for step in 0..=3 {
                let x = (origin.0 + dx * step) as usize;
                let y = (origin.1 + dy * step) as usize;
                assert_eq!(
                    city.tunnel_levels_at(x, y),
                    0,
                    "layout={kind:?}, case={case_index}, step={step}"
                );
            }
            assert_eq!(
                rng.seed(),
                expected_rng.seed(),
                "layout={kind:?}, case={case_index}"
            );
        }
    }
}

#[test]
fn mac68k_late_xund_subway_gate_uses_only_the_recovered_xund_classes() {
    let accepted = [0x01, 0x0f, 0x1f, 0x20, 0x22, 0x23];
    let rejected = [0x00, 0x10, 0x11, 0x21, 0x24, 0xff];
    for (value, draws) in accepted
        .into_iter()
        .map(|value| (value, 1))
        .chain(rejected.into_iter().map(|value| (value, 0)))
    {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-late-xund-classes.sc2",
            "MacLateXundClasses",
        )
        .expect("blank");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 10, 0);
        city.set_zone(20, 20, 0xfa);
        city.set_underground(20, 20, value);

        let seed = 0x2468_1357;
        let mut expected_rng = MacToolboxRng::new(seed);
        for _ in 0..draws {
            expected_rng.random_u16();
        }
        let mut rng = MacToolboxRng::new(seed);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);
        assert_eq!(rng.seed(), expected_rng.seed(), "XUND={value:#04x}");
    }
}

#[test]
fn mac68k_late_xund_subway_gate_removes_stock_single_cell_selectors() {
    // Lakeland's XUND=0x23/XBLD=0xe9 subway stations carry XTXT=54 and take
    // PlaceTile(0), clearing XTXT and its XMIC resources. Oakland's XBLD=0x6d
    // /XTXT=0 on zero XTER takes one rubble draw. Its following export-138
    // classification clears XUND; the mode-zero fixture retains XBLD rubble.
    let mut station =
        City::new_blank("/tmp/sc2000-mac-xund-station.sc2", "Station").expect("blank");
    station.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut station, index, 100);
    }
    set_budget_funding_for_city_test(&mut station, 10, 0);
    station.set_building(20, 20, 0xe9);
    station.set_terrain(20, 20, 1); // nonzero XTER selects PlaceTile(0)
    station.set_underground(20, 20, 0x23);
    station.set_zone(20, 20, 0xfa);
    station.set_text_overlay(20, 20, 54);
    station.set_tile_bits(20, 20, TileBits::new(0xc2));

    let seed = 0x1357_2468;
    let mut expected_station_rng = MacToolboxRng::new(seed);
    expected_station_rng.random_u16(); // funding admission only
    let mut station_rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut station, 0, 0, &mut station_rng);
    assert_eq!(station.building_at(20, 20), 0);
    assert_eq!(station.underground_at(20, 20), 0x23);
    assert_eq!(station.text_overlay_at(20, 20), Some(0));
    assert_eq!(station.zone_at(20, 20).unwrap().raw_bits(), 0x0a);
    assert_eq!(station.tile_bits_at(20, 20).unwrap().raw_bits(), 0);
    assert_eq!(station_rng.seed(), expected_station_rng.seed());

    let mut elevated_road =
        City::new_blank("/tmp/sc2000-mac-xund-elevated-road.sc2", "Road").expect("blank");
    elevated_road.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut elevated_road, index, 100);
    }
    set_budget_funding_for_city_test(&mut elevated_road, 10, 0);
    elevated_road.set_building(20, 20, 0x6d);
    elevated_road.set_underground(20, 20, 0x23);
    elevated_road.set_zone(20, 20, 0xfa);
    elevated_road.set_tile_bits(20, 20, TileBits::new(0xc2));
    for label_id in 1..=4 {
        elevated_road.set_label(label_id, "temporary rubble label");
    }

    let mut expected_road_rng = MacToolboxRng::new(seed);
    expected_road_rng.random_u16(); // funding admission
    let rubble = (expected_road_rng.random_u16() as u8 & 3) + 1;
    let mut road_rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut elevated_road, 0, 0, &mut road_rng);
    assert_eq!(elevated_road.building_at(20, 20), rubble);
    assert_eq!(elevated_road.underground_at(20, 20), 0);
    assert_eq!(elevated_road.zone_at(20, 20).unwrap().raw_bits(), 0x0a);
    assert_eq!(elevated_road.tile_bits_at(20, 20).unwrap().raw_bits(), 0);
    assert_eq!(
        elevated_road.label_at(rubble),
        Some("temporary rubble label")
    );
    assert_eq!(elevated_road.mac_xbld_tile_count(rubble), 1);
    assert_eq!(road_rng.seed(), expected_road_rng.seed());
}

#[test]
fn mac_export161_resource_cleanup_preserves_unwritten_record_bytes() {
    // CODE 3 export 161 has four XTXT classes: clear a sign label for 1..=0x32,
    // leave 0x33..=0x3c alone, clear XMIC +0 and the label for 0x3d..=0xc8,
    // and clear XTHG +0x0a for 0xc9..=0xf0.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-export161-cleanup.sc2", "Export161").expect("blank");
    city.kind = CityFileKind::MacCity;

    city.set_label(1, "temporary sign");
    city.set_label(0x33, "system entry");
    city.set_label(0x56, "structure label");
    let microsim_index = usize::from(0x56_u8 - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_tile_id(microsim_index, 0x8c));
    assert!(city.set_microsim_stat0(microsim_index, 0xa5));
    {
        let record = &mut city.mac_object_slot_records_mut()[0];
        record.kind = 3;
        record.tile_x = 27;
        record.tile_y = 49;
        record.saved_a = 0x9a;
        record.saved_b = 0xbc;
    }

    city.mac_export_161_clear_xtxt_resources(1);
    city.mac_export_161_clear_xtxt_resources(0x33);
    city.mac_export_161_clear_xtxt_resources(0x56);
    city.mac_export_161_clear_xtxt_resources(0xc9);
    city.mac_export_161_clear_xtxt_resources(0xf1);

    assert_eq!(city.label_at(1), Some(""));
    assert_eq!(city.label_at(0x33), Some("system entry"));
    assert_eq!(city.label_at(0x56), Some(""));
    let microsim = city.microsim_at(microsim_index).expect("XMIC row");
    assert_eq!(microsim.tile_id, 0);
    assert_eq!(microsim.stat0, 0xa5);
    let actor = city.traffic_overlays().expect("Mac XTHG slots")[0];
    assert_eq!(actor.kind, 3);
    assert_eq!((actor.tile_x, actor.tile_y), (27, 49));
    assert_eq!(actor.saved_a, 0);
    assert_eq!(actor.saved_b, 0xbc);
}

#[test]
fn mac68k_export81_extent3_and4_alignment_uses_rotated_markers() {
    for rotation in 0..4u8 {
        let markers = match rotation {
            0 => [0x10, 0x20, 0x40, 0x80],
            1 => [0x20, 0x40, 0x80, 0x10],
            2 => [0x40, 0x80, 0x10, 0x20],
            _ => [0x80, 0x10, 0x20, 0x40],
        };
        for extent in [3usize, 4] {
            let delta = extent - 1;
            for (index, marker) in markers.into_iter().enumerate() {
                let mut city = City::new_blank(
                    format!("/tmp/sc2000-mac-export81-align-{rotation}-{extent}-{index}.sc2"),
                    "Align",
                )
                .expect("blank");
                city.saved_rotation = rotation;
                city.set_view_rotation((rotation + 1) & 3);
                city.set_zone(48, 48, marker);
                let expected = match index {
                    0 => (48, 48 + delta),
                    1 => (48 - delta, 48 + delta),
                    2 => (48 - delta, 48),
                    _ => (48, 48),
                };
                assert_eq!(
                    super::super::budget::mac_export81_align_large_generic_anchor(
                        &city, 48, 48, extent
                    ),
                    Some(expected),
                    "rotation={rotation}, extent={extent}, marker={marker:#04x}"
                );
            }
        }
    }
}

#[test]
fn mac68k_late_xund_subway_gate_runs_zero_render_extent2_edit() {
    // XUND=0x23 also reaches export 80's extent-2 bridge/topology arm. Its
    // map replacements and census writes happen with render=0; only the
    // funding and four conditional PlaceTile rubble draws remain.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-late-xund-extent2.sc2", "Bridge").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 100); // suppress early bridge upkeep
    set_budget_funding_for_city_test(&mut city, 10, 0); // admit late XUND upkeep
    city.set_building(20, 20, 0x6a);
    city.set_underground(20, 20, 0x23);
    city.set_zone(20, 20, 0xfa);
    let seed = 0x1020_3040;
    let mut expected_rng = MacToolboxRng::new(seed);
    for _ in 0..5 {
        expected_rng.random_u16();
    }
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    for (x, y) in [(20, 20), (21, 20), (21, 21), (20, 21)] {
        assert!(is_rubble_tile(city.building_at(x, y)), "cell ({x},{y})");
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits() & 0xf0, 0);
    }
    assert_eq!(city.mac_xbld_tile_count(0x6a), 0);
    assert_eq!(
        (1..=4)
            .map(|tile| city.mac_xbld_tile_count(tile))
            .sum::<u16>(),
        4,
        "the native PlaceTile census describes the retained rubble"
    );
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac68k_late_xund_bridge_strip_removes_run_without_render_draws() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-late-xund-strip.sc2", "BridgeStrip").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 100);
    set_budget_funding_for_city_test(&mut city, 10, 0);
    for y in 18..=20 {
        city.set_building(20, y, 0x51);
        city.set_zone(20, y, 0xfa);
    }
    city.set_underground(20, 20, 0x23);

    // Extent-1 export 80 still removes the connected strip when render=0;
    // only the late XUND funding roll consumes a word.
    let seed = 0x2718_2818;
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_u16();
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    for y in 18..=20 {
        assert_eq!(city.building_at(20, y), 0, "bridge y={y}");
        assert_eq!(city.zone_at(20, y).unwrap().raw_bits() & 0xf0, 0);
        assert_eq!(city.zone_at(20, y).unwrap().raw_bits() & 0x0f, 0x0a);
    }
    assert_eq!(city.underground_at(20, 20), 0x23);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_player_underground_bulldozer_uses_rendered_export80_before_export106() {
    // CODE 5 `FUN_0000884e` calls export 80 with render flag -1 when the
    // tunnel-level field is 1, then always calls export 106(0). For an
    // extent-1 XBLD with XTER zero, export 80 consumes two render draws and
    // one rubble draw before the caller clears XUND.
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-underground-export80.city",
        "MacUnderground",
    )
    .expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(100);
    city.set_building(60, 60, 0x06);
    city.set_underground(60, 60, 0x1f);
    city.set_tunnel_levels(60, 60, 1);
    city.set_tile_bits(60, 60, TileBits::new(0x62));
    city.set_zone(60, 60, 0x2f);
    city.set_mac_state_misc_u32_at(0x0fe8, 1);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;
    state.show_underground = true;
    state.mac_process_rng = MacToolboxRng::new(0x4321);
    let mut expected_rng = MacToolboxRng::new(0x4321);
    expected_rng.random_u16();
    expected_rng.random_u16();
    let expected_tunnel_rubble = (expected_rng.random_u16() & 3) as u8 + 1;
    expected_rng.random_u16();
    expected_rng.random_u16();
    let expected_xund23_rubble = (expected_rng.random_u16() & 3) as u8 + 1;

    assert!(state.apply_tool_at_tile(60, 60));
    assert_eq!(state.city.building_at(60, 60), expected_tunnel_rubble);
    assert_eq!(state.city.underground_at(60, 60), 0);
    assert_eq!(state.city.mac_state_misc_u32_at(0x0fe8), Some(0));
    assert_eq!(state.city.tile_bits_at(60, 60).unwrap().raw_bits(), 0x00);
    assert_eq!(state.city.zone_at(60, 60).unwrap().raw_bits(), 0x0f);

    // XUND 0x23 is the second native export-80 gate when ALTM's tunnel field
    // is not 1. This separate click proves that branch follows the same
    // rendered worker before export 106 clears the selected XUND byte.
    state.city.set_building(62, 60, 0x06);
    state.city.set_underground(62, 60, 0x23);
    state.city.set_tile_bits(62, 60, TileBits::new(0x62));
    state.city.set_zone(62, 60, 0x2f);
    assert!(state.apply_tool_at_tile(62, 60));
    assert_eq!(state.city.building_at(62, 60), expected_xund23_rubble);
    assert_eq!(state.city.underground_at(62, 60), 0);
    assert_eq!(state.city.mac_state_misc_u32_at(0x0fe8), Some(0));
    assert_eq!(state.city.tile_bits_at(62, 60).unwrap().raw_bits(), 0x00);
    assert_eq!(state.city.zone_at(62, 60).unwrap().raw_bits(), 0x0f);
    assert_eq!(state.mac_process_rng.seed(), expected_rng.seed());
    assert_eq!(state.city.city_funds(), 98);
}

#[test]
fn mac_player_underground_bulldozer_export198_retiles_surface_without_pe_xund_refresh() {
    // The native common tail calls CODE 13 export 198, then export 199. The
    // first retile-checks surface-network neighbors; the rooted export-199
    // helpers have no direct XUND writes and call timing/redraw gateways,
    // unlike the PE pipe/subway refresh.
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-underground-tail.city",
        "MacUndergroundTail",
    )
    .expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(100);
    city.set_building(58, 60, NetworkKind::Power.base_tile());
    city.set_building(59, 60, NetworkKind::Power.base_tile());
    stamp_xbit_powerable(&mut city, 58, 60);
    stamp_xbit_powerable(&mut city, 59, 60);
    city.set_underground(58, 60, 0x10);
    city.set_underground(59, 60, 0x10);
    city.set_underground(60, 60, 0x1f);
    city.set_tunnel_levels(60, 60, 1);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;
    state.show_underground = true;

    assert!(state.apply_tool_at_tile(60, 60));
    assert_eq!(
        state.city.building_at(59, 60),
        network_building_for_mask(NetworkKind::Power, NETWORK_LEFT)
    );
    assert_eq!(state.city.underground_at(58, 60), 0x10);
    assert_eq!(state.city.underground_at(59, 60), 0x10);
    assert_eq!(state.city.underground_at(60, 60), 0);
    assert_eq!(state.city.city_funds(), 99);
}

#[test]
fn mac68k_monthly_bridge_upkeep_removes_strip_and_classifies_endpoints() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-monthly-bridge-strip.sc2", "BridgeStrip").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 0);
    city.set_mac_state_misc_u32_at(0x64, 0);

    for x in 18..=20 {
        city.set_building(x, 20, 0x51);
        city.set_zone(x, 20, 0xf0);
        city.set_traffic_density(x, 20, 0x33);
    }
    city.set_tile_bits(20, 20, TileBits::new(0).with_rotate(true));
    city.set_zone(17, 20, 0x37);
    city.set_altitude(17, 20, 5, 0);
    for (x, y) in [
        (16, 19),
        (17, 19),
        (18, 19),
        (16, 20),
        (18, 20),
        (16, 21),
        (17, 21),
        (18, 21),
    ] {
        city.set_altitude(x, y, 0, 0);
    }
    city.set_altitude(17, 19, 6, 0);
    city.set_zone(21, 20, 0x27);
    city.set_altitude(21, 20, 5, 0);

    // The monthly caller consumes one admission word; rendered export 80
    // consumes two more words for each of the three removed bridge tiles.
    let seed = 0x5a17_2026;
    let mut expected_rng = MacToolboxRng::new(seed);
    for _ in 0..7 {
        expected_rng.random_u16();
    }
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    for x in 18..=20 {
        assert_eq!(city.building_at(x, 20), 0, "bridge x={x}");
        assert_eq!(city.zone_at(x, 20).unwrap().raw_bits(), 0);
        assert!(!city.tile_bits_at(x, 20).unwrap().rotate());
        assert_eq!(city.traffic_density_at(x, 20), Some(0x33));
    }
    assert_eq!(city.altm_at(17, 20).land & 0x1f, 4);
    assert_eq!(city.zone_at(17, 20).unwrap().raw_bits() & 0x0f, 0);
    assert_eq!(city.terrain_at(17, 20), 0x02);
    // The east endpoint's all-higher-neighbor mask selects class 0x32, which
    // restores its original land level after lowering.
    assert_eq!(city.altm_at(21, 20).land & 0x1f, 5);
    assert_eq!(city.terrain_at(21, 20), 0);
    assert_eq!(city.zone_at(21, 20).unwrap().raw_bits() & 0x0f, 0);
    assert_eq!(rng.seed(), expected_rng.seed());

    let mut funded = City::new_blank("/tmp/sc2000-mac-monthly-bridge-funded.sc2", "BridgeFunded")
        .expect("blank");
    funded.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut funded, index, 100);
    }
    funded.set_mac_state_misc_u32_at(0x64, 255);
    funded.set_building(20, 20, 0x51);
    let seed = 0x5a17_2027;
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut funded, 0, 0, &mut rng);
    assert_eq!(funded.building_at(20, 20), 0x51);
    assert_eq!(rng.seed(), seed, "100% funding skips the admission draw");
}

#[test]
fn underfunded_tunnel_portal_0x3f_uses_road_placetile_not_tunnel() {
    // PE DoBudgetRoadCheck @ `0x4359bc`–`0x4359c6`: XBLD `0x3f..=0x46` → road
    // PlaceTile before the unreachable tunnel arm @ `0x435a58`. Portals take
    // road funding CRT rubble + XBIT `&=0x7f`, not DestroyBuilding / tunnel row.
    let mut city = City::new_blank("/tmp/sc2000-tun-portal-road.sc2", "TunRd").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 10, 0);
    city.set_building(20, 20, 0x3f);
    city.set_building(24, 20, 0x41);
    city.set_text_overlay(20, 20, 0x55);
    city.set_text_overlay(24, 20, 0x66);
    for x in 20..=24 {
        city.set_tunnel_levels(x, 20, 2);
    }
    let mut state = test_state(city);
    assert_eq!(state.decay_transport_budget_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), 0x3f);
    assert_eq!(state.city.building_at(24, 20), 0x41);

    set_budget_funding_for_test(&mut state.city, 10, 100);
    set_budget_funding_for_test(&mut state.city, 5, 0);
    state
        .city
        .set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
    state
        .city
        .set_tile_bits(24, 20, TileBits::new(0).with_powerable(true));
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert!(is_rubble_tile(state.city.building_at(24, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x55));
    assert_eq!(state.city.text_overlay_at(24, 20), Some(0x66));
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());
    assert!(!state.city.tile_bits_at(24, 20).unwrap().powerable());
    // Road PlaceTile does not run DestroyBuilding elevated ALTM clear.
    for x in 20..=24 {
        assert_eq!(
            state.city.tunnel_levels_at(x, 20),
            2,
            "tunnel levels should remain at x={x}"
        );
    }
    assert_ne!(state.query_line, "Tunnel Collapsed");
}

#[test]
fn underfunded_tunnel_portal_road_decay_preserves_watered_and_xtrf() {
    // PE road PlaceTile @ `0x435daa` + XBIT `&=0x7f`: no invent water refresh / XTRF.
    let mut city = City::new_blank("/tmp/sc2000-tunnel-watered.sc2", "TunWater").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(20, 20, 0x3f);
    city.set_tunnel_levels(20, 20, 2);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x55);
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.tunnel_levels_at(20, 20), 2);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x55));
}

#[test]
fn mac68k_underfunded_tunnel_portal_road_decay_preserves_xtrf() {
    // Shared road PlaceTile leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-tunnel-xtrf.sc2", "MacTunX").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(20, 20, 0x3f);
    city.set_tunnel_levels(20, 20, 2);
    city.set_traffic_density(20, 20, 0x55);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x55));
}

#[test]
fn mac_scan_highway_edit_suppresses_later_zone_nine_actor_random_branch() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-highway-zone-nine-interleave.sc2",
        "MacScanHighwayZoneNine",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 6, 0);

    // Phase 0's unzoned 2x2 highway upkeep includes the zone-9 e0 tile at
    // (21,20). Phase 4 would otherwise encounter that cell and can take the
    // alternate type-3 actor path after its first two Random gates.
    city.set_building(20, 20, 0x49);
    city.set_zone(21, 20, 9);
    city.set_building(21, 20, 0xe0);

    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            for _ in 0..5 {
                probe.random_i16(); // highway gate and four dry-cell variants
            }
            let first = probe.random_i16() as u16;
            let second = probe.random_i16() as u16;
            first & 3 != 0 && second & 3 == 0
        })
        .expect("find the zone-9 alternate gates after highway upkeep");
    let mut expected_rng = MacToolboxRng::new(seed);
    for _ in 0..6 {
        expected_rng.random_i16();
    }

    let mut rng = MacToolboxRng::new(seed);
    for phase in 0..=4 {
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, phase / 4, phase % 4, &mut rng);
    }

    assert!((1..=4).contains(&city.building_at(21, 20)));
    assert_eq!(city.mac_xbld_tile_count(0xe0), 0);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_scan_extent2_bridge_run_suppresses_later_zone_nine_actor_draw() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-extent2-bridge-run-zone-nine.sc2",
        "MacScanExtent2BridgeRunZoneNine",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 0);
    city.set_mac_state_misc_u32_at(0x64, 0);

    // The one-cell 0x6a run is admitted at (42,42) in phase 10. Its topology
    // code comes from the rotated marker on the phase-11 target beneath it.
    let (x, y) = (42usize, 42usize);
    city.set_building(x, y, 0x6a);
    city.set_zone(x, y, 0);
    city.set_building(x, y + 1, 0xe0);
    city.set_zone(x, y + 1, 9);
    city.set_tile_bits(x, y + 1, TileBits::new(0x02));

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
            probe_city.building_at(x, y) == 0
                && probe_city.building_at(x, y + 1) == 0
                && selector & 3 != 0
                && actor_gate & 3 != 0
        })
        .expect("find phase-10 connected upkeep and phase-11 selector draws");

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
    assert_eq!(
        after_phase_ten.tile_bits_at(x, y + 1).unwrap().raw_bits() & 0x02,
        0
    );

    let mut intact_control = after_phase_ten.clone();
    intact_control.set_building(x, y + 1, 0xe0);
    intact_control.set_tile_bits(x, y + 1, TileBits::new(0x02));
    let mut control_rng = prefix_rng;
    let mut expected_control_rng = prefix_rng;
    expected_control_rng.random_i16(); // zone-9 selector
    expected_control_rng.random_i16(); // old 0xe0 actor gate
    mac_budget_accumulate_monthly_rci_scan_partition(&mut intact_control, 2, 3, &mut control_rng);
    assert_eq!(intact_control.building_at(x, y + 1), 0xe0);
    assert_eq!(control_rng.seed(), expected_control_rng.seed());

    let mut expected_rng = prefix_rng;
    expected_rng.random_i16(); // cleared candidate no longer takes the e0 actor gate
    mac_budget_accumulate_monthly_rci_scan_partition(&mut after_phase_ten, 2, 3, &mut prefix_rng);
    assert_eq!(after_phase_ten.building_at(x, y + 1), 0);
    assert_eq!(after_phase_ten.mac_xbld_tile_count(0x6a), 0);
    assert_eq!(after_phase_ten.mac_xbld_tile_count(0xe0), 0);
    assert_eq!(prefix_rng.seed(), expected_rng.seed());
}

#[test]
fn mac_scan_late_xund_coastal_destroy_suppresses_later_zone_nine_actor_draw() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-late-xund-coastal-zone-nine.sc2",
        "MacScanLateXundCoastalZoneNine",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 10, 0);

    let (x, y) = (42usize, 42usize);
    city.set_building(x, y, 0xe0);
    city.set_zone(x, y, 10);
    city.set_underground(x, y, 0x23);
    city.set_building(x, y + 1, 0xe0);
    city.set_zone(x, y + 1, 9);
    city.set_tile_bits(x, y + 1, TileBits::new(0x02));

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
            probe_city.building_at(x, y) == 0
                && probe_city.building_at(x, y + 1) == 0
                && selector & 3 != 0
                && actor_gate & 3 != 0
        })
        .expect("find late coastal destroy and phase-11 actor-gate draws");

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
    assert_eq!(
        after_phase_ten.tile_bits_at(x, y + 1).unwrap().raw_bits() & 0x02,
        0
    );

    let mut intact_control = after_phase_ten.clone();
    intact_control.set_building(x, y + 1, 0xe0);
    intact_control.set_tile_bits(x, y + 1, TileBits::new(0x02));
    let mut control_rng = prefix_rng;
    let mut expected_control_rng = prefix_rng;
    expected_control_rng.random_i16(); // zone-9 selector
    expected_control_rng.random_i16(); // old 0xe0 actor gate
    mac_budget_accumulate_monthly_rci_scan_partition(&mut intact_control, 2, 3, &mut control_rng);
    assert_eq!(intact_control.building_at(x, y + 1), 0xe0);
    assert_eq!(control_rng.seed(), expected_control_rng.seed());

    let mut expected_rng = prefix_rng;
    expected_rng.random_i16(); // coastal cleanup removed the e0 actor gate
    mac_budget_accumulate_monthly_rci_scan_partition(&mut after_phase_ten, 2, 3, &mut prefix_rng);
    assert_eq!(after_phase_ten.building_at(x, y + 1), 0);
    assert_eq!(after_phase_ten.mac_xbld_tile_count(0xe0), 0);
    assert_eq!(prefix_rng.seed(), expected_rng.seed());
}

#[test]
fn mac_scan_late_xund_portal_destroy_suppresses_later_portal_stamp_draws() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-late-xund-portal-interleave.sc2",
        "MacScanLateXundPortalInterleave",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 10, 0);

    // Phase 10's south-facing portal reaches phase 11's north-facing portal.
    let (x, y) = (42usize, 42usize);
    city.set_building(x, y, 0x42);
    city.set_zone(x, y, 10);
    city.set_underground(x, y, 0x23);
    city.set_building(x, y + 1, 0x40);
    city.set_zone(x, y + 1, 10);
    city.set_underground(x, y + 1, 0x23);
    city.set_tunnel_levels(x, y, 0x1f);
    city.set_tunnel_levels(x, y + 1, 0x1f);

    let seed = 0x51a7_20c3;
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
    assert_eq!(after_phase_ten.underground_at(x, y + 1), 0x23);
    assert_eq!(after_phase_ten.tunnel_levels_at(x, y + 1), 0);

    // Restoring the portal endpoints after the same phase-10 prefix isolates
    // what phase 11 would do if its portal candidate had survived the sweep.
    let mut intact_control = after_phase_ten.clone();
    intact_control.set_building(x, y, 0x42);
    intact_control.set_building(x, y + 1, 0x40);
    intact_control.set_tunnel_levels(x, y, 0x1f);
    intact_control.set_tunnel_levels(x, y + 1, 0x1f);
    let mut control_rng = prefix_rng;
    let mut expected_control_rng = prefix_rng;
    expected_control_rng.random_i16(); // phase-11 subway-funding admission
    for _ in 0..4 {
        expected_control_rng.random_i16(); // two portal redraw stamp pairs
    }
    mac_budget_accumulate_monthly_rci_scan_partition(&mut intact_control, 2, 3, &mut control_rng);
    assert_eq!(intact_control.building_at(x, y), 0);
    assert_eq!(intact_control.building_at(x, y + 1), 0);
    assert_eq!(control_rng.seed(), expected_control_rng.seed());

    let mut expected_rng = prefix_rng;
    expected_rng.random_i16(); // phase-11 gate remains because XUND is unchanged
    mac_budget_accumulate_monthly_rci_scan_partition(&mut after_phase_ten, 2, 3, &mut prefix_rng);
    assert_eq!(after_phase_ten.building_at(x, y + 1), 0);
    assert_eq!(prefix_rng.seed(), expected_rng.seed());
}

#[test]
fn mac_scan_zone_nine_water_ray_suppresses_later_actor_random_branch() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-zone-nine-water-ray-interleave.sc2",
        "MacScanZoneNineWaterRay",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y) = (42, 42); // phase 10: X residue 2, Y residue 2
    city.set_zone(x, y, 9);
    city.set_tile_bits(x, y, TileBits::new(0x40)); // powered coastal anchor
    for ray_y in y + 1..=y + 5 {
        city.set_zone(x, ray_y, 9);
        city.set_tile_bits(x, ray_y, TileBits::new(0x04));
    }
    city.set_altitude(x, y + 5, 4, 6);
    let mut direct_probe = city.clone();
    assert_eq!(
        mac_budget_try_place_zone_water_ray(&mut direct_probe, x, y, 9),
        MacSpecialTileAttempt::Placed
    );

    // Search against the full scan through phase 10 so any earlier random
    // branches stay in their native order. The phase-11 candidate's first draw
    // then takes the actor alternative, and its new df marker suppresses the
    // second draw that the old e0 candidate would have consumed.
    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe_city = city.clone();
            let mut probe = MacToolboxRng::new(seed);
            for phase in 0..=10 {
                mac_budget_accumulate_monthly_rci_scan_partition(
                    &mut probe_city,
                    phase / 4,
                    phase % 4,
                    &mut probe,
                );
            }
            let later_candidate = probe.random_i16() as u16;
            let old_e0_actor_gate = probe.random_i16() as u16;
            probe_city.building_at(x, y) == 0xe0
                && probe_city.building_at(x, y + 1) == 0xdf
                && later_candidate & 3 != 0
                && old_e0_actor_gate & 3 != 0
        })
        .expect("find scan draws for the ray and later zone-9 candidate");
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
    expected_rng.random_i16(); // phase-11 later candidate first draw
    expected_rng.random_i16(); // endpoint candidate first draw; e0's extra draw is skipped

    let mut rng = MacToolboxRng::new(seed);
    for phase in 0..=11 {
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, phase / 4, phase % 4, &mut rng);
    }

    assert_eq!(city.building_at(x, y), 0xe0);
    for ray_y in y + 1..=y + 4 {
        assert_eq!(city.building_at(x, ray_y), 0xdf, "y={ray_y}");
        assert_eq!(city.zone_at(x, ray_y).unwrap().raw_bits(), 0xf9);
    }
    assert_eq!(city.building_at(x, y + 5), 0);
    assert_eq!(city.mac_xbld_tile_count(0xe0), 1);
    assert_eq!(city.mac_xbld_tile_count(0xdf), 4);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn neighbor_connection_overlay_lets_rci_trips_leave_the_map() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let edge_x = CITY_WIDTH - 1;
    city.set_zone(edge_x - 3, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(edge_x - 3, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    for x in (edge_x - 2)..=edge_x {
        city.set_building(
            x,
            20,
            network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
        );
    }
    assert!(
        !crate::trip_generator::run_trip(&city, edge_x - 3, 20, ZONE_RESIDENTIAL_LIGHT, 1).success
    );

    persist_pe_neighbor_connection_overlays(&mut city);
    assert_eq!(
        city.text_overlay_at(edge_x, 20),
        Some(crate::trip_generator::TRIP_NEIGHBOR_CONNECTION_OVERLAY)
    );
    assert!(
        crate::trip_generator::run_trip(&city, edge_x - 3, 20, ZONE_RESIDENTIAL_LIGHT, 1).success,
        "XTXT 250 on an edge road completes PE trips off the map"
    );
}

#[test]
fn synthetic_trip_markers_do_not_count_as_paid_neighbor_links() {
    let mut city = City::new_blank("/tmp/sc2000-synthetic-links.sc2", "Links").unwrap();
    city.set_city_funds(10_000);
    for y in 20..25 {
        city.set_building(
            127,
            y,
            network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
        );
    }
    city.set_traffic_density(126, 20, 73);
    persist_pe_neighbor_connection_overlays(&mut city);
    persist_pe_neighbor_connection_overlays(&mut city);
    assert_eq!(city.text_overlay_at(127, 20), Some(0xfa));
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 0);
    assert_eq!(pe_rci_commercial_infra_numerator(&city), 1_500);
    let traffic = city.traffic_density_slice().map(|v| v.to_vec());

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    let candidate = RoadNeighborLinkCandidate {
        endpoint: (127, 20),
        initial_xtxt: 0,
    };
    assert!(apply_road_neighbor_link_confirmation(
        &mut state, candidate, true
    ));
    assert_eq!(state.city.text_overlay_at(127, 20), Some(0xfa));
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&state.city), 1);
    assert_eq!(
        state.city.traffic_density_slice().map(|v| v.to_vec()),
        traffic
    );
    let path = std::env::temp_dir().join(format!(
        "citygame-synthetic-links-{}.sc2",
        std::process::id()
    ));
    state.city.save(&path).unwrap();
    let loaded = City::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        pe_fa_network_overlay_count_4cc4d8(&loaded),
        5,
        "provenance is session-only; original-style load recounts saved XTXT markers"
    );

    let mut rail = City::new_blank("/tmp/sc2000-synthetic-rail.sc2", "Rail").unwrap();
    rail.set_building(
        0,
        30,
        network_building_for_mask(NetworkKind::Rail, NETWORK_LEFT | NETWORK_RIGHT),
    );
    persist_pe_neighbor_connection_overlay(&mut rail, 0, 30);
    assert_eq!(rail.text_overlay_at(0, 30), Some(0xfa));
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_fun_00460e10_ignite(&mut rail, None, &mut lfsr, 0, 30, 1));
    assert_eq!(
        rail.pe_nonnetwork_neighbor_links, 0,
        "a synthetic marker was not a paid link"
    );
}

#[test]
fn persist_pe_neighbor_connection_overlay_skips_power_and_signs() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(
        0,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_text_overlay(0, 10, 5);
    city.set_building(
        30,
        0,
        network_building_for_mask(NetworkKind::Power, NETWORK_TOP | NETWORK_BOTTOM),
    );
    persist_pe_neighbor_connection_overlays(&mut city);
    assert_eq!(city.text_overlay_at(0, 10), Some(5), "signs must keep XTXT");
    assert_eq!(
        city.text_overlay_at(30, 0).unwrap_or(0),
        0,
        "power lines are not trip neighbor connections"
    );
}

#[test]
fn trip_overlay_refresh_keeps_confirmed_highway_marker_at_inner_edge_origin() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-neighbor-marker.sc2", "HwyNeighborMarker")
        .expect("blank city");
    let origin_x = CITY_WIDTH - 2;
    city.set_building(origin_x, 20, 0x49);
    city.set_building(CITY_WIDTH - 1, 20, 0x49);
    city.set_text_overlay(origin_x, 20, 0xfa);

    persist_pe_neighbor_connection_overlays(&mut city);

    assert_eq!(city.text_overlay_at(origin_x, 20), Some(0xfa));
    assert_eq!(
        city.text_overlay_at(CITY_WIDTH - 1, 20),
        Some(crate::trip_generator::TRIP_NEIGHBOR_CONNECTION_OVERLAY),
        "the actual border tile still receives the traffic overlay"
    );
}

#[test]
fn game_clock_day_dispatch_persists_each_sim_calendar_day() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(13);
    state.apply_game_clock_advance(
        start,
        GameClockAdvance {
            days: 13,
            months: 0,
        },
    );

    assert_eq!(state.city.city_age_days(), Some(13));
    assert_eq!(state.game_clock.day(), 13);
    assert_eq!(state.game_clock.status_line(), "Jan 14 1900");
}

#[test]
fn game_clock_month_catchup_dispatches_all_sim_calendar_days() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));
    let start = state.game_clock;
    let advance = state.game_clock.tick(2.0, GameSpeed::Llama);
    assert_eq!(advance.days, 25);
    assert_eq!(advance.months, 1);

    state.apply_game_clock_advance(start, advance);

    assert_eq!(
        state.city.saved_date(),
        Some(CityDate {
            year: 1900,
            month: 2
        })
    );
    assert_eq!(state.city.city_age_days(), Some(25));
    assert_eq!(state.game_clock.day(), 0);
    assert_eq!(state.game_clock.status_line(), "Feb 1 1900");
}

#[test]
fn game_clock_day_dispatch_applies_one_win95_growth_lattice() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in [(20, 20), (21, 21)] {
        city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
        stamp_transport_access_for_test(&mut city, x, y - 1);
        power_adjacent_building_for_test(&mut city, x - 1, y);
        water_footprint_from_below_for_test(&mut city, x, y, 1);
    }
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 1, 2);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 3);
    assert!(
        is_construction_building(state.city.building_at(20, 20)),
        "day 3 iStep=0 iSubStep=0 should StartBuilding (20,20), got {:#04x}",
        state.city.building_at(20, 20)
    );
    assert_eq!(state.city.building_at(21, 21), 0);
}

#[test]
fn pe_water_worker_uses_saved_xbit_membership_without_direction_recheck() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.city.set_underground(
        11,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_TOP | NETWORK_BOTTOM),
    );
    state.city.set_zone(12, 10, 1);
    state.city.set_building(12, 10, 112);

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(12, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(12, 10).unwrap().watered());
}

#[test]
fn water_supply_summary_reports_shortage_without_counting_water_utilities_as_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(20, 20, 220);
    power_water_source_for_test(&mut city, 20, 20);
    city.set_zone(21, 20, 1);
    city.set_zone(22, 20, 1);
    city.set_zone(23, 20, 1);

    let enough = water_supply_summary(&city, 0, 2);
    assert_eq!(
        enough,
        WaterSupplySummary {
            capacity: 1,
            demand: 3
        }
    );
    assert_eq!(enough.shortage(), 2);
    assert!(enough.has_shortage());

    let enough = water_supply_summary(&city, 1, 2);
    assert_eq!(enough.capacity, 6);
    assert_eq!(enough.demand, 3);
    assert_eq!(enough.shortage(), 0);
    assert!(!enough.has_shortage());
}

#[test]
fn water_service_summary_reports_disconnected_demand_as_shortage() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(20, 20, 220);
    city.set_zone(10, 10, 1);
    city.set_zone(11, 10, 1);

    assert_eq!(
        water_service_summary(&city),
        WaterServiceSummary {
            served: 0,
            demand: 2
        }
    );
    assert_eq!(ServiceStatus::for_city(&city).water_shortage(), 2);
}

#[test]
fn tile_water_status_text_reports_disconnected_and_non_pipe_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_zone(12, 10, 1);
    city.set_zone(13, 10, 1);

    assert_eq!(tile_water_status_text(&city, 12, 10), " water=off");
    assert_eq!(tile_water_status_text(&city, 13, 10), " water=off");
    assert_eq!(tile_water_status_text(&city, 14, 10), "");
}

#[test]
fn tile_water_status_text_reports_connected_consumers_watered() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 220);
    power_water_source_for_test(&mut city, 10, 10);
    city.set_underground(11, 10, 17);
    city.set_zone(12, 10, 1);
    city.set_zone(13, 10, 1);
    city.set_building(12, 10, 112);
    city.set_building(13, 10, 112);
    seed_saved_water_worker_bits_for_test(&mut city);

    assert_eq!(tile_water_status_text(&city, 11, 10), " water=on");
    assert_eq!(tile_water_status_text(&city, 12, 10), " water=on");
    assert_eq!(tile_water_status_text(&city, 13, 10), " water=on");
}

#[test]
fn status_line_reports_water_shortage_when_demand_exceeds_capacity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, 1);
    city.set_zone(11, 10, 1);
    city.set_zone(12, 10, 1);
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(state.query_line.contains("Water Shortage 3"));
}

#[test]
fn status_line_reports_water_capacity_warning_at_original_98_percent_threshold() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 220);
    power_water_source_for_test(&mut city, 20, 20);
    city.set_terrain(20, 19, 0x30);
    city.set_tile_bits(20, 19, crate::city::TileBits::new(0b0000_0100));
    for x in 21..=69 {
        city.set_zone(x, 20, 1);
        city.set_building(x, 20, 112);
    }
    seed_saved_water_worker_bits_for_test(&mut city);

    assert_eq!(water_supply_capacity_tiles(&city, 8, 0), 50);
    assert_eq!(
        water_service_summary(&city),
        WaterServiceSummary {
            served: 49,
            demand: 49
        }
    );
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(
        state.query_line.contains("Water Capacity 98%"),
        "{}",
        state.query_line
    );
    assert!(!state.query_line.contains("Water Shortage"));
}

#[test]
fn status_line_reports_water_shortage_when_capacity_is_disconnected() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 220);
    city.set_zone(10, 10, 1);
    city.set_zone(11, 10, 1);
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(state.query_line.contains("Water Shortage 2"));
}

#[test]
fn highway_traffic_is_sorted_above_the_deck_and_below_foreground_buildings() {
    for building in (73..=80).chain(93..=107) {
        let surface = xbld_sprite_info(building, 0).unwrap();
        let deck_depth = sprite_depth(20, 30, surface.layer_depth, surface.depth_adjustment);
        let traffic_depth = sprite_depth(20, 30, traffic_surface_layer_depth(building), 2);
        let building_depth = sprite_depth(20, 30, DEPTH_BUILDING, 0);
        assert!(
            deck_depth < traffic_depth && traffic_depth < building_depth,
            "building {building}: deck={deck_depth}, traffic={traffic_depth}, foreground={building_depth}"
        );
    }
    // Road traffic still stays behind an overhead power-line overlay.
    assert!(
        sprite_depth(20, 30, traffic_surface_layer_depth(67), 2)
            < sprite_depth(20, 30, DEPTH_POWER, 0)
    );
}

#[test]
fn status_line_reports_active_traffic_path_for_diagnostics() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "diagnostic-status").expect("blank city");
    let mut state = test_state(city);

    state.engine_fidelity = EngineFidelity::Win95;
    state.refresh_status_line();
    assert!(
        state
            .query_line
            .contains("traffic=win95-xtrf-animated-overlay"),
        "{}",
        state.query_line
    );

    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_status_line();
    assert!(
        state
            .query_line
            .contains("traffic=win95-xtrf-animated-overlay"),
        "{}",
        state.query_line
    );
}

#[test]
fn status_line_reports_treatment_only_when_city_has_water_demand() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.refresh_status_line();
    assert!(!state.query_line.contains("Treatment Missing"));

    state.city.set_zone(10, 10, 1);
    state.refresh_status_line();
    assert!(state.query_line.contains("Treatment Missing 9"));
}

#[test]
fn service_status_reports_treatment_need_even_when_water_supply_is_short() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, 1);
    city.set_zone(11, 10, 1);

    let status = ServiceStatus::for_city(&city);

    assert_eq!(status.water_shortage(), 2);
    assert_eq!(status.treatment_missing(), 9);
}

#[test]
fn refresh_live_trips_power_maps_use_process_crt_seed_not_city_name() {
    // Monthly trip refresh powered gates share FUN_004679B0 process CRT.
    let mut city = City::new_blank("/tmp/sc2000-trip-power.sc2", "TripPower").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x2828_8282);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let _ = power_service_maps_with_entry_seed(&state.city, process);
    state.refresh_live_trips();
}

#[test]
fn january_traffic_count_heuristic_uses_process_crt_seed_not_city_name() {
    // Map-less January pe_traffic_count shares FUN_004679B0 process CRT with attitude maps.
    let mut city = City::new_blank("/tmp/sc2000-jan-traffic.sc2", "JanTraffic").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 18);
    // Roads so the map-less network_pressure path is non-trivial.
    for x in 12..=20 {
        city.set_building(x, 10, 29);
    }
    city.clear_traffic_density_for_test();
    assert!(!city.has_traffic_density_map());
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xd4d4_4d4d);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let population = population_stats_for_city(&state.city);
    let expected = live_traffic_count_for_city_with_power(
        &state.city,
        population,
        &power_service_maps_with_entry_seed(&state.city, process),
    );
    let city_name = live_traffic_count_for_city(&state.city, population);
    let _ = city_name;
    state.city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        1,
    );
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);
    assert_eq!(state.city.pe_traffic_count(), expected);
}

#[test]
fn mac68k_january_leaves_pe_traffic_count_alone() {
    // Mac has no recovered dwCityTrafficCount / 0x4CC6F4 January writer.
    let mut city = City::new_blank("/tmp/sc2000-mac-jan-traf.sc2", "MacJan").expect("blank");
    city.set_pe_traffic_count(0xdead_beef);
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        1,
    );
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);
    assert_eq!(state.city.pe_traffic_count(), 0xdead_beef);
}

#[test]
fn mac_live_misc_refresh_does_not_write_win95_transit_passenger_extensions() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-transit-ext.sc2", "MacTransitExt").expect("blank city");
    city.kind = CityFileKind::MacCity;
    // Make the optional viewer-extension region present without relying on
    // any Mac-owned state-vector slot. The passenger snapshots are in CODE 4's
    // zero-filled tail and must remain untouched by a shared live refresh.
    city.set_misc_u32_at(GRAPH_EXTRA_HISTORY_MISC_OFFSET, 1);
    city.set_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, 0x1111);
    city.set_misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET, 0x2222);
    city.set_misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET, 0x3333);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.trip_bus_passengers = 10;
    state.trip_rail_passengers = 20;
    state.trip_subway_passengers = 30;

    state.refresh_live_misc_current_values();

    assert_eq!(
        state.city.misc_u32_at(MISC_BUS_PASSENGERS_OFFSET),
        Some(0x1111)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET),
        Some(0x2222)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET),
        Some(0x3333)
    );
    assert_eq!(state.city.pe_dw_bus_passengers(), 0x1111);
    assert_eq!(state.city.pe_dw_rail_passengers(), 0x2222);
    assert_eq!(state.city.pe_dw_subway_passengers(), 0x3333);
}

#[test]
fn pe_transit_passenger_reset_preserves_mac_zero_fill_tail() {
    let mut mac_city = City::new_blank("/tmp/sc2000-mac-transit-reset.sc2", "MacTransitReset")
        .expect("blank city");
    mac_city.kind = CityFileKind::MacCity;
    // CODE 4 zero-fills this region, so these sentinels model untouched tail
    // bytes and ensure the annual PE finalizer does not persist its snapshots.
    mac_city.set_misc_u32_at(GRAPH_EXTRA_HISTORY_MISC_OFFSET, 1);
    mac_city.set_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, 0x1111);
    mac_city.set_misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET, 0x2222);
    mac_city.set_misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET, 0x3333);
    mac_city.set_pe_dw_bus_passengers(10);
    mac_city.set_pe_dw_rail_passengers(20);
    mac_city.set_pe_dw_subway_passengers(30);

    mac_city.reset_pe_transit_passenger_totals();

    assert_eq!(mac_city.pe_dw_bus_passengers(), 0);
    assert_eq!(mac_city.pe_dw_rail_passengers(), 0);
    assert_eq!(mac_city.pe_dw_subway_passengers(), 0);
    assert_eq!(
        mac_city.misc_u32_at(MISC_BUS_PASSENGERS_OFFSET),
        Some(0x1111)
    );
    assert_eq!(
        mac_city.misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET),
        Some(0x2222)
    );
    assert_eq!(
        mac_city.misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET),
        Some(0x3333)
    );

    let mut win95_city =
        City::new_blank("/tmp/sc2000-win95-transit-reset.sc2", "Win95TransitReset")
            .expect("blank city");
    win95_city.set_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, 0x1111);
    win95_city.set_misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET, 0x2222);
    win95_city.set_misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET, 0x3333);

    win95_city.reset_pe_transit_passenger_totals();

    assert_eq!(win95_city.misc_u32_at(MISC_BUS_PASSENGERS_OFFSET), Some(0));
    assert_eq!(win95_city.misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET), Some(0));
    assert_eq!(
        win95_city.misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET),
        Some(0)
    );
}

#[test]
fn mac68k_day20_preserves_saved_xbit_membership_when_geometry_disagrees() {
    let mut city = City::new_blank("/tmp/sc2000-mac-d20-xbit-membership.sc2", "MacD20Xbit")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_building(40, 40, 220);
    city.set_underground(40, 40, 16);
    city.set_underground(41, 40, 16);
    city.set_building(41, 40, 0x70);
    city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_powered(true));
    // The underground geometry connects these cells, but the saved Mac XBIT
    // membership plane omits the consumer. export_209 must leave it outside
    // the worker flood and the phase-20 refresh must not synthesize XBIT 0x20.
    city.set_tile_bits(41, 40, TileBits::new(0));
    city.set_misc_u32_at(0x68, 4);
    city.set_mac_state_misc_u32_at(912 * 4, 0);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day20_water();

    let pump = state.city.tile_bits_at(40, 40).expect("pump bits");
    let consumer = state.city.tile_bits_at(41, 40).expect("consumer bits");
    assert!(pump.piped());
    assert!(pump.watered());
    assert!(
        !consumer.piped(),
        "geometry must not create Mac XBIT membership"
    );
    assert!(
        !consumer.watered(),
        "the native worker did not reach this cell"
    );
}

#[test]
fn form_army_base_strip_horizontal_rot0_keeps_end_dd() {
    // Horizontal strip + rot0 → flag true → type-7 bit02 matches → stay DD.
    let mut city = City::new_blank("/tmp/sc2000-form-army-dd.sc2", "FormArmyDd").expect("blank");
    city.set_rotation(0);
    for x in 50..58 {
        city.set_zone(x, 50, ZONE_MILITARY | ZONE_CORNER_ALL);
    }
    let mut state = test_state(city);
    state.form_army_base_strip(50, 50, 57, 50);
    assert_eq!(state.city.building_at(50, 50), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.building_at(57, 50), RUNWAY_BUILDING_ID);
}

#[test]
fn form_army_base_strip_vertical_rot0_upgrades_end_dd_to_de() {
    // A newly formed strip first stamps DD; the DE-upgrade path applies only
    // when the cell already contains DD before the strip operation.
    let mut city = City::new_blank("/tmp/sc2000-form-army-de.sc2", "FormArmyDe").expect("blank");
    city.set_rotation(0);
    for y in 50..58 {
        city.set_zone(50, y, ZONE_MILITARY | ZONE_CORNER_ALL);
    }
    let mut state = test_state(city);
    state.form_army_base_strip(50, 50, 50, 57);
    assert_eq!(state.city.building_at(50, 50), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.building_at(50, 57), RUNWAY_BUILDING_ID);
}

#[test]
fn form_army_base_strip_preserves_watered_and_xtrf() {
    // Army road stamp is PlaceTile-family — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-army-xtrf.sc2", "ArmyX").expect("blank");
    city.set_tile_bits(50, 50, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(50, 50, 0x48);
    let mut state = test_state(city);
    state.form_army_base_strip(50, 50, 50, 52);
    let building = state.city.building_at(50, 50);
    assert!(
        is_road_tile(building)
            || matches!(
                building,
                RUNWAY_BUILDING_ID | AIRPORT_RUNWAY_CROSS_BUILDING_ID
            ),
        "army strip stamps road or runway-cross, got {building}"
    );
    let bits = state.city.tile_bits_at(50, 50).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(50, 50), Some(0x48));
}

#[test]
fn form_army_base_strip_preserves_xtxt_like_pe_place_tile() {
    // Army PlaceTile-family road stamp leaves XTXT.
    let mut city = City::new_blank("/tmp/sc2000-army-xtxt.sc2", "ArmyT").expect("blank");
    city.set_text_overlay(50, 50, 0x40);
    let mut state = test_state(city);
    state.form_army_base_strip(50, 50, 50, 52);
    assert_eq!(state.city.text_overlay_at(50, 50), Some(0x40));
}

#[test]
fn mac68k_form_army_base_strip_preserves_xtxt_like_pe_place_tile() {
    // Shared army PlaceTile stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-army-xtxt.sc2", "MacArmyT").expect("blank");
    city.set_text_overlay(50, 50, 0x40);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.form_army_base_strip(50, 50, 50, 52);
    assert_eq!(state.city.text_overlay_at(50, 50), Some(0x40));
}

#[test]
fn mac68k_form_army_base_strip_preserves_xtrf() {
    // Shared army road stamp is PlaceTile-family; Mac68k must not invent an XTRF clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-army-xtrf.sc2", "MacArmyX").expect("blank");
    city.set_traffic_density(50, 50, 0x48);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.form_army_base_strip(50, 50, 50, 52);
    let building = state.city.building_at(50, 50);
    assert!(
        is_road_tile(building)
            || matches!(
                building,
                RUNWAY_BUILDING_ID | AIRPORT_RUNWAY_CROSS_BUILDING_ID
            ),
        "army strip stamps road or runway-cross, got {building}"
    );
    assert_eq!(state.city.traffic_density_at(50, 50), Some(0x48));
}

#[test]
fn form_army_base_strip_skips_invent_clear_before_place_tile() {
    // PlaceTile overwrites XBLD; do not invent-clear first. Leave XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-army-skip-clear.sc2", "ArmyClr").expect("blank");
    city.set_building(50, 50, 1); // small rubble under SMALL_PARK
    city.set_building(50, 51, 2);
    city.set_building(50, 52, 3);
    // XTRF is 2×2 microsim — seed one cell away from sibling overwrites.
    city.set_traffic_density(50, 50, 0x48);
    let mut state = test_state(city);
    state.form_army_base_strip(50, 50, 50, 52);
    for y in 50..=52 {
        let building = state.city.building_at(50, y);
        assert!(
            is_road_tile(building)
                || matches!(
                    building,
                    RUNWAY_BUILDING_ID | AIRPORT_RUNWAY_CROSS_BUILDING_ID
                ),
            "army strip stamps road or runway-cross at (50,{y}), got {building}"
        );
    }
    assert_eq!(state.city.traffic_density_at(50, 50), Some(0x48));
}

#[test]
fn stamp_naval_yard_strip_preserves_watered_and_xtrf() {
    // PE naval zone stamp: XZON (+ clear small XBLD) — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-naval-xtrf.sc2", "NavalX").expect("blank");
    city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(40, 40, 0x52);
    let mut state = test_state(city);
    state.stamp_naval_yard_strip(NavalCoastStrip {
        start_x: 40,
        start_y: 40,
        along_dx: 1,
        along_dy: 0,
        inland_dx: 0,
        inland_dy: 1,
        run_len: MILITARY_NAVAL_ZONE_LENGTH,
    });
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.zone_id()),
        Some(ZONE_MILITARY)
    );
    let bits = state.city.tile_bits_at(40, 40).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x52));
}

#[test]
fn stamp_naval_yard_strip_preserves_xtxt() {
    // Naval zone stamp clears small XBLD only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-naval-xtxt.sc2", "NavalT").expect("blank");
    city.set_text_overlay(40, 40, 0x43);
    let mut state = test_state(city);
    state.stamp_naval_yard_strip(NavalCoastStrip {
        start_x: 40,
        start_y: 40,
        along_dx: 1,
        along_dy: 0,
        inland_dx: 0,
        inland_dy: 1,
        run_len: MILITARY_NAVAL_ZONE_LENGTH,
    });
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0x43));
}

#[test]
fn mac68k_stamp_naval_yard_strip_preserves_xtxt() {
    // Shared naval zone stamp leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-naval-xtxt.sc2", "MacNavalT").expect("blank");
    city.set_text_overlay(40, 40, 0x43);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_naval_yard_strip(NavalCoastStrip {
        start_x: 40,
        start_y: 40,
        along_dx: 1,
        along_dy: 0,
        inland_dx: 0,
        inland_dy: 1,
        run_len: MILITARY_NAVAL_ZONE_LENGTH,
    });
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0x43));
}

#[test]
fn mac68k_stamp_naval_yard_strip_preserves_xtrf() {
    // Shared naval zone stamp leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-naval-xtrf.sc2", "MacNavalX").expect("blank");
    city.set_traffic_density(40, 40, 0x52);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.stamp_naval_yard_strip(NavalCoastStrip {
        start_x: 40,
        start_y: 40,
        along_dx: 1,
        along_dy: 0,
        inland_dx: 0,
        inland_dy: 1,
        run_len: MILITARY_NAVAL_ZONE_LENGTH,
    });
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.zone_id()),
        Some(ZONE_MILITARY)
    );
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x52));
}

#[test]
fn mac_edit_map_level_tool_change_and_done_finish_marker_before_transition() {
    for done in [false, true] {
        let city = City::new_blank("/tmp/mac-level-transition.sc2", "MacLevelTransition").unwrap();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::EditNewMap;
        state.tool = EditTool::LevelTerrain;
        assert!(state.begin_edit_map_level_drag_at(64, 64, 100.0));
        assert_eq!(state.city.text_overlay_at(64, 64), Some(0xc9));
        if done {
            state.finish_edit_map_state();
            assert_eq!(state.mode, AppMode::CityView);
        } else {
            state.select_tool_state(EditTool::Trees);
            assert_eq!(state.tool, EditTool::Trees);
        }
        assert_eq!(state.city.text_overlay_at(64, 64), Some(0));
        assert_eq!(state.city.traffic_overlays().unwrap()[0].kind, 4);
        assert!(!state.edit_map_level_drag_active());
    }
}

#[test]
fn army_runway_cross_upgrades_runway_when_strip_flag_differs() {
    let mut city = City::new_blank("/tmp/sc2000-runway-skip.sc2", "RunwaySkip").expect("blank");
    city.set_building(55, 55, RUNWAY_BUILDING_ID);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_traffic_density(55, 55, 0x46);
    let mut state = test_state(city);
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(
        state.city.building_at(55, 55),
        AIRPORT_RUNWAY_CROSS_BUILDING_ID
    );
    assert_eq!(state.city.traffic_density_at(55, 55), Some(0x46));
}

#[test]
fn army_runway_cross_clears_airport_range_via_fun_4392a0() {
    // PE FUN_004392A0 for XBLD in 0xDB..=0xEA: PlaceTile(0), XBIT&=0x3F, XZON&=0x0F.
    let mut city = City::new_blank("/tmp/sc2000-runway-clr.sc2", "RunwayClr").expect("blank");
    city.set_terrain(55, 55, 0);
    city.set_building(55, 55, 0xe0);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_tile_bits(
        55,
        55,
        TileBits::new(0)
            .with_piped(true)
            .with_watered(true)
            .with_powered(true)
            .with_powerable(true),
    );
    city.set_text_overlay(55, 55, 0x3c);
    let mut state = test_state(city);
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    let zone = state.city.zone_at(55, 55).unwrap();
    assert_eq!(zone.zone_id(), ZONE_MILITARY);
    // The clear helper masks the old corners, then the FormArmy caller restores them.
    assert_eq!(zone.raw_bits() & 0xf0, ZONE_CORNER_ALL);
    let bits = state.city.tile_bits_at(55, 55).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert!(!bits.powered());
    assert!(!bits.powerable());
    assert_eq!(state.city.text_overlay_at(55, 55), Some(0x3c));
}

#[test]
fn place_subway_station_clears_xbit_piped_like_pe_4408dd() {
    // PE subway-station arm: after underground stamp, AND XBIT with 0xDF.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true));
    let mut state = test_state(city);
    assert!(state.city.tile_bits_at(20, 20).unwrap().piped());
    assert!(state.place_building_at(20, 20, SUBWAY_STATION_BUILDING_ID) > 0);
    assert_eq!(state.city.building_at(20, 20), SUBWAY_STATION_BUILDING_ID);
    assert!(
        !state.city.tile_bits_at(20, 20).unwrap().piped(),
        "subway station must not keep the piped bit"
    );
}

#[test]
fn place_subway_station_clears_piped_keeps_watered_and_xtrf_like_pe_4408dd() {
    // PE subway-station arm @ 0x4408dd: AND XBIT 0xDF (clear piped), leave watered/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-subway-xtrf.sc2", "SubXtrf").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x44);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, SUBWAY_STATION_BUILDING_ID) > 0);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(!bits.piped(), "PE clears piped on subway station");
    assert!(bits.watered(), "invent water refresh must not wipe watered");
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x44));
}

#[test]
fn direct_subway_station_place_skips_maptool_preclear_crt() {
    // This invokes the direct CityTool placement worker. The MapTool surface
    // preclear at 0x442eb9 is a separate caller and must not consume its CRT.
    let mut city = City::new_blank("/tmp/sc2000-sub-map-crt.sc2", "SubMapCrt").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 6); // placeable tree; XBLD >= 6 → CRT rubble
    city.set_underground(20, 20, UNDER_TILE_GREY_BOX_2);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true));
    let mut state = test_state(city);
    state.show_underground = true;
    state.tool = EditTool::SubStation;
    let crt_before = state.pe_process_crt_seed();
    assert!(state.place_building_at(20, 20, SUBWAY_STATION_BUILDING_ID) > 0);
    assert_eq!(state.city.building_at(20, 20), SUBWAY_STATION_BUILDING_ID);
    assert_eq!(state.city.underground_at(20, 20), UNDER_TILE_GREY_BOX_2);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().piped());
    assert_eq!(state.pe_process_crt_seed(), crt_before);
}

#[test]
fn mac68k_place_subway_station_preserves_xtrf_like_pe_4408dd() {
    // Shared subway-station arm leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-subway-xtrf.sc2", "MacSubX").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x44);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_building_at(20, 20, SUBWAY_STATION_BUILDING_ID) > 0);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(!bits.piped(), "PE clears piped on subway station");
    assert!(bits.watered(), "invent water refresh must not wipe watered");
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x44));
}

#[test]
fn power_supply_summary_reports_shortage() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 10..=12 {
        city.set_zone(x, 10, 1);
    }

    assert_eq!(
        power_supply_summary(&city),
        PowerSupplySummary {
            capacity: 0,
            demand: 3
        }
    );
    assert_eq!(power_supply_summary(&city).shortage(), 3);
    assert!(power_supply_summary(&city).has_shortage());
}

#[test]
fn status_line_reports_power_shortage_when_demand_exceeds_capacity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, 1);
    city.set_zone(11, 10, 1);
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(state.query_line.contains("Power Shortage 2"));
}

#[test]
fn status_line_reports_power_capacity_warning_at_original_98_percent_threshold() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 10..=14 {
        city.set_building(x, 10, 198);
    }
    for y in 10..=23 {
        for x in 15..=28 {
            city.set_zone(x, y, 1);
        }
    }

    assert_eq!(power_supply_capacity_tiles(&city), 200);
    assert_eq!(
        power_service_summary(&city),
        PowerServiceSummary {
            served: 196,
            demand: 196
        }
    );
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(
        state.query_line.contains("Power Capacity 98%"),
        "{}",
        state.query_line
    );
    assert!(!state.query_line.contains("Power Shortage"));
}

#[test]
fn status_line_reports_power_shortage_when_capacity_is_disconnected() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 207);
        }
    }
    city.set_zone(10, 10, 1);
    city.set_zone(11, 10, 1);
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(state.query_line.contains("Power Shortage 2"));
}

#[test]
fn power_service_summary_reports_disconnected_demand_as_shortage() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 207);
        }
    }
    for x in 10..=12 {
        city.set_zone(x, 10, 1);
    }

    let summary = power_service_summary(&city);

    assert_eq!(
        summary,
        PowerServiceSummary {
            served: 0,
            demand: 3
        }
    );
    assert_eq!(summary.shortage(), 3);
    assert_eq!(power_supply_summary(&city).capacity, 704);

    let (_, connected_tiles) = power_connection_maps(&city);
    assert_eq!(
        connected_power_supply_capacity_tiles(&city, &connected_tiles),
        704
    );
    assert!(!connected_tiles[10 * CITY_WIDTH + 10]);
}

#[test]
fn tile_power_status_text_reports_powered_unpowered_and_non_consumers() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 4);
    city.set_zone(11, 10, 1);
    city.set_zone(12, 10, 1);
    city.set_zone(13, 10, 1);

    assert_eq!(tile_power_status_text(&city, 11, 10), " pwr=on");
    assert_eq!(tile_power_status_text(&city, 12, 10), " pwr=on");
    assert_eq!(tile_power_status_text(&city, 13, 10), " pwr=on");
    assert_eq!(tile_power_status_text(&city, 14, 10), "");
}

#[test]
fn tile_power_status_text_reports_power_lines_and_plants() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 4);
    city.set_building(
        11,
        10,
        network_building_for_mask(NetworkKind::Power, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Power, NETWORK_LEFT | NETWORK_RIGHT),
    );

    let maps = power_service_maps(&city);

    assert!(maps.conductive_tiles[10 + 10 * CITY_WIDTH]);
    assert!(maps.energized_tiles[10 + 10 * CITY_WIDTH]);
    assert!(maps.conductive_tiles[11 + 10 * CITY_WIDTH]);
    assert!(maps.energized_tiles[11 + 10 * CITY_WIDTH]);
    assert!(maps.conductive_tiles[20 + 20 * CITY_WIDTH]);
    assert!(!maps.energized_tiles[20 + 20 * CITY_WIDTH]);
    assert_eq!(tile_power_status_text(&city, 10, 10), " pwr=on");
    assert_eq!(tile_power_status_text(&city, 11, 10), " pwr=on");
    assert_eq!(tile_power_status_text(&city, 20, 20), " pwr=off");
}

#[test]
fn tile_power_status_text_reports_disconnected_consumers_unpowered() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 207);
        }
    }
    city.set_zone(10, 10, 1);

    assert_eq!(tile_power_status_text(&city, 10, 10), " pwr=off");
}

#[test]
fn subway_placement_uses_pe_pipe_crossover_on_water_source_underground_stamp() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(10, 10, 220), 1);
    assert_eq!(state.city.underground_at(10, 10), UNDER_TILE_PIPES_LR);

    assert_eq!(
        state.place_underground_path(&[(9, 10), (10, 10), (11, 10)], UndergroundKind::Subway),
        3
    );

    assert!(matches!(state.city.underground_at(9, 10), 1 | 2));
    // PE PlaceSubwayAtCoordinates `0x44bb8e` converts XUND 0x10 to 0x20.
    assert_eq!(
        state.city.underground_at(10, 10),
        UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB
    );
    assert!(matches!(state.city.underground_at(11, 10), 1 | 2));
    assert!(state.can_place_underground_at(10, 10, UndergroundKind::Subway));
}

#[test]
fn recovered_building_underground_stamps_roundtrip_through_save() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(10, 10, 220), 1);
    assert_eq!(state.place_building_at(20, 20, 233), 1);
    assert_eq!(state.place_building_at(30, 30, 249), 9);
    state.city.set_terrain(39, 40, 0x30);
    state
        .city
        .set_tile_bits(39, 40, crate::city::TileBits::new(0b0000_0001));
    assert_eq!(state.place_building_at(40, 40, 250), 9);

    let path = std::env::temp_dir().join("sc2000_rust_building_xund_roundtrip.sc2");
    state.city.save(&path).expect("save city");
    let loaded = City::load(&path).expect("reload city");
    let _ = std::fs::remove_file(path);

    assert_eq!(loaded.underground_at(10, 10), UNDER_TILE_PIPES_LR);
    assert_eq!(loaded.underground_at(20, 20), UNDER_TILE_GREY_BOX_2);
    for y in 30..=32 {
        for x in 30..=32 {
            assert_eq!(loaded.underground_at(x, y), UNDER_TILE_GREY_BOX_1);
        }
    }
    for y in 40..=42 {
        for x in 40..=42 {
            assert_eq!(loaded.underground_at(x, y), UNDER_TILE_BUILDING_PIPE_TEE);
        }
    }
}

#[test]
fn budget_subway_built_uses_xund_occ_plus_dw_tile_like_473807() {
    // PE @ `0x473807`–`0x473815`: subway = sx(`[0x4ca41c]`) + sx(`dwTileCount[0xe9]`).
    let mut city = City::new_blank("/tmp/sc2000-budget-subway.sc2", "BudSub").expect("blank");
    assert_eq!(budget_service_built_counts_for_city(&city).subway, 0);
    pe_place_xund_442370(&mut city, 20, 20, 1);
    pe_place_xund_442370(&mut city, 21, 20, 2);
    assert_eq!(budget_service_built_counts_for_city(&city).subway, 2);
    pe_place_tile_441f00(&mut city, 22, 20, SUBWAY_STATION_BUILDING_ID);
    assert_eq!(budget_service_built_counts_for_city(&city).subway, 3);
    // Raw set_underground must not affect the PE word.
    city.set_underground(23, 20, 3);
    assert_eq!(budget_service_built_counts_for_city(&city).subway, 3);
}

#[test]
fn surface_query_labels_generated_transport_and_military_tiles() {
    let cases = [
        (221, "Runway"),
        (222, "Runway Intersection"),
        (223, "Seaport Pier"),
        (224, "Crane"),
        (226, "Military Control Tower"),
        (231, "F-15b"),
        (232, "Military Hangar"),
        (239, "Military Parking Lot"),
        (240, "Loading Bay"),
        (241, "Top Secret"),
        (242, "Cargo Yard"),
        (247, "Church"),
        (249, "Missile Silo"),
    ];

    for (building, label) in cases {
        assert_eq!(surface_building_query_label(building), Some(label));
    }
}

#[test]
fn transit_coverage_uses_developed_ports_not_empty_zones() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 0x70);
    city.set_zone(21, 20, ZONE_AIRPORT);

    let empty_zone_coverage = city_service_coverage_for_city(&city);
    assert_eq!(
        empty_zone_coverage.transit,
        ServiceCoverage {
            served: 0,
            demand: 1,
        }
    );

    city.set_building(21, 20, 221);

    let developed_zone_coverage = city_service_coverage_for_city(&city);
    assert_eq!(
        developed_zone_coverage.transit,
        ServiceCoverage {
            served: 1,
            demand: 1,
        }
    );
}

#[test]
fn developed_port_coverage_sources_use_complete_footprints_once() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zoned_building_footprint_for_test(
        &mut city,
        (20, 20),
        2,
        ZONE_AIRPORT,
        AIRPORT_LARGE_DEVELOPMENT_BUILDINGS[0],
    );
    city.set_zone(26, 20, ZONE_AIRPORT);
    city.set_building(26, 20, AIRPORT_DEVELOPMENT_BUILDINGS[0]);

    city.set_zone(30, 20, ZONE_AIRPORT);
    city.set_building(30, 20, AIRPORT_LARGE_DEVELOPMENT_BUILDINGS[1]);

    assert_eq!(
        coverage_sources_for_developed_zone(&city, ZONE_AIRPORT, 30),
        vec![
            CoverageSource {
                origin_x: 20,
                origin_y: 20,
                size: 2,
                radius: 30,
            },
            CoverageSource {
                origin_x: 26,
                origin_y: 20,
                size: 1,
                radius: 30,
            },
        ]
    );
}

#[test]
fn pe_land_value_zone_nibble_dispatch_matches_jump_table() {
    // `nibble - 3` in `0..3` → table `[0x46aef9, 0x46aef9, 0x46afe7, 0x46afe7]`;
    // otherwise residential path @ `0x46ae09`.
    assert_eq!(ZONE_RESIDENTIAL_LIGHT, 1);
    assert_eq!(ZONE_RESIDENTIAL_DENSE, 2);
    assert_eq!(ZONE_COMMERCIAL_LIGHT, 3);
    assert_eq!(ZONE_COMMERCIAL_DENSE, 4);
    assert_eq!(ZONE_INDUSTRIAL_LIGHT, 5);
    assert_eq!(ZONE_INDUSTRIAL_DENSE, 6);
    assert_eq!(
        land_value_pollution_penalty(ZONE_COMMERCIAL_LIGHT, 40),
        land_value_pollution_penalty(ZONE_COMMERCIAL_DENSE, 40)
    );
    assert_eq!(
        land_value_pollution_penalty(ZONE_INDUSTRIAL_LIGHT, 64),
        land_value_pollution_penalty(ZONE_INDUSTRIAL_DENSE, 64)
    );
    assert_ne!(
        land_value_pollution_penalty(ZONE_RESIDENTIAL_LIGHT, 50),
        land_value_pollution_penalty(ZONE_COMMERCIAL_LIGHT, 50)
    );
}

#[test]
fn live_density_dispatch_retains_shared_ordinance_baseline_across_engines() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-density-dispatch.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_building(20, 20, 0x70);
    // Native Mac ordinance mask is MISC slot 1000 (+0x0fa0).
    city.set_mac_state_misc_u32_at(0x0fa0, (1 << 11) | (1 << 4));
    let sample = density_map_index(20, 20);

    let mac = super::super::density::live_density_maps_for_city_with_fidelity(
        &city,
        PopulationStats::default(),
        CityServiceOutcomes::default(),
        1,
        false,
        EngineFidelity::Mac68k,
    );
    let pe = super::super::density::live_density_maps_for_city_with_fidelity(
        &city,
        PopulationStats::default(),
        CityServiceOutcomes::default(),
        1,
        false,
        EngineFidelity::Win95,
    );

    assert_eq!(mac.police[sample], 2);
    assert_eq!(mac.fire[sample], 2);
    // The PE developed-tile +2 baseline was recovered on 2026-10-22.
    // Both engines read the Mac layout's native saved ordinance mask here;
    // this no-station fixture does not distinguish their station-ring math.
    assert_eq!(pe.police[sample], 2);
    assert_eq!(pe.fire[sample], 2);
}

#[test]
fn industry_stats_read_signed_misc_table_triples() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut values = vec![0_u32; INDUSTRY_VALUES];
    values[0] = (-400_i32) as u32;
    values[1] = 5;
    values[2] = 12;
    values[3] = 250;
    values[4] = 0;
    values[5] = 4;
    city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &values);

    let stats = industry_stats_for_city(&city);

    assert_eq!(
        stats[0],
        IndustryStats {
            ratio: 75,
            tax: 5,
            demand: -400,
            population: 12,
        }
    );
    assert_eq!(
        stats[1],
        IndustryStats {
            ratio: 25,
            tax: 0,
            demand: 250,
            population: 4,
        }
    );
}

#[test]
fn industry_tables_round_trip_through_misc_0x16c() {
    let path = std::env::temp_dir().join(format!("sc2000-industry-{}.sc2", std::process::id()));
    let mut city = City::new_blank(&path, "ind").expect("blank");
    let mut values = vec![0_u32; INDUSTRY_VALUES];
    values[0] = (-40_i32) as u32;
    values[1] = 9;
    values[2] = 1_200;
    values[3] = 80;
    values[4] = 4;
    values[5] = 300;
    city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &values);
    city.set_misc_u32_at(MISC_BUILDING_TILE_COUNTS_OFFSET, 0xAABBCCDD);
    city.save(&path).expect("save");

    let loaded = City::load(&path).expect("reload");
    let stats = industry_stats_for_city(&loaded);
    assert_eq!(stats[0].demand, -40);
    assert_eq!(stats[0].tax, 9);
    assert_eq!(stats[0].population, 1_200);
    assert_eq!(stats[1].demand, 80);
    assert_eq!(stats[1].tax, 4);
    assert_eq!(stats[1].population, 300);
    assert_eq!(
        loaded.misc_u32_at(MISC_BUILDING_TILE_COUNTS_OFFSET),
        Some(0xAABBCCDD),
        "industry 0x16c must end before dwTileCount at 0x1F0"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn budget_service_coverage_mapping_tracks_transport_rows() {
    let coverage = CityServiceCoverage {
        police: ServiceCoverage {
            served: 1,
            demand: 10,
        },
        fire: ServiceCoverage {
            served: 2,
            demand: 10,
        },
        health: ServiceCoverage {
            served: 3,
            demand: 10,
        },
        education: ServiceCoverage {
            served: 4,
            demand: 10,
        },
        recreation: ServiceCoverage {
            served: 5,
            demand: 10,
        },
        transit: ServiceCoverage {
            served: 6,
            demand: 10,
        },
    };

    assert_eq!(
        coverage_for_budget_service_index(coverage, 0),
        Some(coverage.police)
    );
    assert_eq!(
        coverage_for_budget_service_index(coverage, 3),
        Some(coverage.education)
    );
    assert_eq!(
        coverage_for_budget_service_index(coverage, 4),
        Some(coverage.education)
    );
    for index in TRANSPORT_BUDGET_INDICES {
        assert_eq!(
            coverage_for_budget_service_index(coverage, index),
            Some(coverage.transit)
        );
    }
    assert_eq!(coverage_for_budget_service_index(coverage, 11), None);
}

#[test]
fn refresh_live_stats_persists_sports_teams_bss_to_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(
        &mut city,
        10,
        10,
        STADIUM_BUILDING_ID,
        STADIUM_BUILDING_SIZE,
    );
    stamp_complete_building_for_test(
        &mut city,
        20,
        10,
        STADIUM_BUILDING_ID,
        STADIUM_BUILDING_SIZE,
    );
    city.set_building(40, 40, STADIUM_BUILDING_ID);
    city.set_pe_w_sports_teams(0x0007);
    let mut state = test_state(city);

    state.refresh_live_misc_stats();

    assert_eq!(sports_teams_for_city(&state.city), 7);
    assert_eq!(state.city.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET), Some(7));
    assert_eq!(state.city.pe_w_sports_teams(), 7);
}

#[test]
fn refresh_live_stats_persists_subway_tile_count_to_misc() {
    // PE MISC `0xfe8` / BSS `0x4ca41c`: PlaceXUND occupancy word, not a map rescan.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.pe_xund_occupancy = 3;
    let mut state = test_state(city);

    state.refresh_live_misc_stats();

    assert_eq!(state.city.misc_u32_at(MISC_SUBWAY_COUNT_OFFSET), Some(3));
    assert_eq!(state.city.pe_xund_occupancy, 3);
}

#[test]
fn budget_tax_bases_respect_arcology_and_rci_field_ownership() {
    let population = PopulationStats {
        arcology_population: 600,
        residential_population: 100,
        commercial_population: 200,
        industrial_population: 300,
        ..PopulationStats::default()
    };
    let cases = [
        (
            CityFileKind::MacCity,
            EngineFidelity::Mac68k,
            [200, 250, 350],
        ),
        (
            CityFileKind::MacCity,
            EngineFidelity::Win95,
            [200, 250, 350],
        ),
        (
            CityFileKind::WinSc2,
            EngineFidelity::Mac68k,
            [200, 250, 350],
        ),
        (
            CityFileKind::WinSc2,
            EngineFidelity::Win95,
            [2_100, 1_200, 1_300],
        ),
    ];

    for (kind, fidelity, expected) in cases {
        let mut city =
            City::new_blank("/tmp/sc2000-budget-owner.sc2", "BudgetOwner").expect("blank city");
        city.kind = kind;
        if kind == CityFileKind::MacCity {
            city.set_mac_state_misc_u32_at(0x1020, 600);
            if fidelity == EngineFidelity::Win95 {
                city.prepare_pe_fidelity_from_saved_city();
            }
        } else if fidelity == EngineFidelity::Mac68k {
            let id = city
                .provision_microsim_for_building(0xfb)
                .expect("arcology row");
            city.set_microsim_stat2(usize::from(id - MIN_SIM_TEXT_ENTRIES), 600);
            city.republish_mac_arcology_resident_total();
        } else {
            city.set_pe_dw_arcology_population(12_000);
        }
        city.set_pe_zone_pops([10, 20, 30]);
        assert_eq!(
            budget_tax_bases_for_city_with_fidelity(&city, population, fidelity),
            expected,
            "engine={fidelity:?} file={kind:?}"
        );
    }
}

#[test]
fn mac_budget_service_funding_adjustment_updates_native_row_and_roundtrips() {
    let mut city = City::new_blank("/tmp/sc2000-mac-budget-funding-edit.sc2", "MacFundingEdit")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET, AUTO_BUDGET_DISABLED_VALUE);

    let row_offset = budget_row_misc_offset_for_city(&city, BUDGET_SERVICE_ROW_START, 0)
        .expect("Mac police row");
    let mut row = vec![0; BUDGET_SERVICE_MISC_STRIDE];
    row[BUDGET_SERVICE_CURRENT_FIELD] = 9;
    row[BUDGET_SERVICE_FUNDING_FIELD] = 60;
    row[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] = 0x1234;
    row[4] = 0x55;
    row[5] = 0x66;
    city.set_budget_misc_u32_values(row_offset, &row);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.adjust_budget_service_funding(0, 5));

    let values = state
        .city
        .budget_misc_u32_values(row_offset, BUDGET_SERVICE_MISC_STRIDE);
    assert_eq!(values[BUDGET_SERVICE_CURRENT_FIELD], 9);
    assert_eq!(values[BUDGET_SERVICE_FUNDING_FIELD], 65);
    assert_eq!(values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD], 0x1234);
    assert_eq!((values[4], values[5]), (0x55, 0x66));
    assert_eq!(
        state.city.mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(AUTO_BUDGET_DISABLED_VALUE)
    );

    let path = PathBuf::from("/tmp/sc2000-mac-budget-funding-edit-roundtrip.city");
    state.city.save(&path).expect("save Mac city");
    let loaded = City::load(path).expect("reload Mac city");
    let loaded_values = loaded.budget_misc_u32_values(row_offset, BUDGET_SERVICE_MISC_STRIDE);
    assert_eq!(loaded_values[BUDGET_SERVICE_CURRENT_FIELD], 9);
    assert_eq!(loaded_values[BUDGET_SERVICE_FUNDING_FIELD], 65);
    assert_eq!(loaded_values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD], 0x1234);
    assert_eq!((loaded_values[4], loaded_values[5]), (0x55, 0x66));
    assert_eq!(
        loaded.mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(AUTO_BUDGET_DISABLED_VALUE)
    );
}

#[test]
fn population_panel_saved_caches_respect_engine_and_file_ownership() {
    let make_city = |kind| {
        let mut city =
            City::new_blank("/tmp/sc2000-pop-fame-owner.sc2", "FameOwner").expect("blank");
        city.kind = kind;
        // PE's fame lives at +0x38, but Mac CODE 4 leaves that slot opaque.
        city.set_misc_u32_at(MISC_CITY_FAME_OFFSET, 0x1234_5678);
        city.set_misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET, 81);
        city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 9);
        city.pe_dw_city_fame = 111;
        city.set_pe_unemployment_percent(37);
        city.set_pe_zone_pops([200, 0, 0]);
        city.persist_pe_day22_population_leftover(25_000);
        city
    };
    let population = PopulationStats {
        total_population: 1,
        ..PopulationStats::default()
    };
    let outcomes = CityServiceOutcomes::default();
    let cases = [
        (CityFileKind::MacCity, EngineFidelity::Mac68k, false),
        (CityFileKind::MacCity, EngineFidelity::Win95, false),
        (CityFileKind::WinSc2, EngineFidelity::Mac68k, false),
        (CityFileKind::WinSc2, EngineFidelity::Win95, true),
    ];

    for (kind, fidelity, uses_saved_pe_fame) in cases {
        let city = make_city(kind);
        let power_maps = power_service_maps(&city);
        let rows = population_panel_demographic_rows_with_power_and_fidelity(
            &city,
            population,
            outcomes,
            &power_maps,
            fidelity,
        );
        let fame = rows
            .iter()
            .find(|(label, _)| *label == "City Fame")
            .map(|(_, value)| *value)
            .expect("City Fame row");
        if uses_saved_pe_fame {
            assert_eq!(fame, 111, "Win95 engine + PE file uses the PE cache");
        } else if kind == CityFileKind::MacCity && fidelity.uses_win95() {
            assert_eq!(
                fame,
                live_city_fame_value_with_fidelity(
                    &city,
                    population,
                    outcomes,
                    CityScenarioCounters::default(),
                    fidelity,
                ),
                "Win95 fame must use its ready population cache on Mac-layout saves"
            );
            assert_eq!(
                fame, 10,
                "the PE population is 25,000, not the census value 1"
            );
        } else {
            let mut without_pe_population = city.clone();
            without_pe_population.clear_pe_zone_pops();
            let expected = live_city_fame_value_with_fidelity(
                &without_pe_population,
                population,
                outcomes,
                CityScenarioCounters::default(),
                fidelity,
            );
            assert_eq!(fame, expected, "Mac-owned state must not supply PE fame");
            assert_ne!(fame, 111);
        }

        let workforce = rows
            .iter()
            .find(|(label, _)| *label == "Work Force %")
            .map(|(_, value)| *value)
            .expect("Work Force % row");
        assert_eq!(
            workforce,
            if fidelity.uses_win95() { 81 } else { 0 },
            "Win95 fidelity reads shared workforce state regardless of file layout"
        );

        let unemployment = rows
            .iter()
            .find(|(label, _)| *label == "Unemployment %")
            .map(|(_, value)| *value)
            .expect("Unemployment % row");
        let expected_unemployment = match fidelity {
            EngineFidelity::Mac68k => 9,
            EngineFidelity::Win95 => 37,
        };
        assert_eq!(
            unemployment, expected_unemployment,
            "the row must use the cache owned by this engine and file layout"
        );
    }
}

#[test]
fn rci_zones_wait_for_transport_access_before_developing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(21, 21, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    water_footprint_from_below_for_test(&mut city, 21, 21, 3);
    seed_pe_rci_demand_to_survive_months(&mut city, 3);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();
    assert_eq!(population_stats_for_city(&state.city).populated_tiles, 0);

    stamp_transport_access_for_test(&mut state.city, 20, 22);
    power_adjacent_building_for_test(&mut state.city, 21, 20);
    state.handle_simulation_month_advanced();
    state.handle_simulation_month_advanced();
    assert_eq!(
        population_stats_for_city(&state.city).residential_tiles,
        7,
        "PE IsValidTransitItems 24-ring covers 7 of the 3×3 tiles from (20,22)"
    );
}

#[test]
fn empty_vacant_rci_zones_still_report_growth_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 20..24 {
        city.set_zone(x, 20, ZONE_INDUSTRIAL_LIGHT);
    }

    let demand = rci_demand_values_for_city(
        &city,
        population_stats_for_city(&city),
        city_service_outcomes_for_city(&city),
    );

    assert!(demand[2] > 0);
    assert!(demand[2] > demand[0]);
    assert!(demand[2] > demand[1]);
}

#[test]
fn failed_trip_gate_allows_decline_when_other_services_are_ok() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    // Power + water + road stub, but no complementary destination — trip fails.
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = false;

    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
}

#[test]
fn successful_trip_with_max_demand_leaves_remainder_zero() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    // Complementary destination zone only — a finished C building would also roll ChangeBuilding.
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 2_000, 50, 50);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = false;
    state.refresh_live_trips();
    assert!(
        state.tile_has_growth_trip_access(20, 20),
        "fixture must complete a powered trip before the remainder-0 gate matters"
    );

    assert_eq!(state.decline_unserved_rci_zones(), 0);
    assert_eq!(
        state.city.building_at(20, 20),
        RESIDENTIAL_1X1_BUILDINGS[0],
        "PE remainder 4000-(demand+2000) is 0 at max demand, so ChangeBuilding cannot fire"
    );
}

#[test]
fn simulation_month_persists_transit_passengers_in_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(40, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(40, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    city.set_building(21, 20, 29);
    city.set_building(22, 20, RAIL_STATION_BUILDING_ID);
    for x in 23..=37 {
        city.set_building(x, 20, 45);
    }
    city.set_building(38, 20, RAIL_STATION_BUILDING_ID);
    city.set_building(39, 20, 29);
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 40, 19);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    assert!(state.trip_rail_passengers > 0);
    assert_eq!(
        state.city.misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET),
        Some(state.trip_rail_passengers)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_BUS_PASSENGERS_OFFSET),
        Some(state.trip_bus_passengers)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET),
        Some(state.trip_subway_passengers)
    );

    let path = std::env::temp_dir().join("sc2000-passenger-persist.sc2");
    state.city.save(&path).expect("save city");
    let mut reloaded = test_state(City::load(&path).expect("reload city"));
    reloaded.load_transit_passengers_from_city();
    assert_eq!(reloaded.trip_rail_passengers, state.trip_rail_passengers);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn powered_watered_negative_demand_rci_buildings_decline_without_transport_access() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
    assert_eq!(
        state.city.zone_at(20, 20).map(|zone| zone.zone_id()),
        Some(ZONE_RESIDENTIAL_LIGHT)
    );
}

#[test]
fn simulation_month_runs_day19_traffic_pass() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(28, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 28, 19);
    city.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, 999);
    let mut state = test_state(city);

    state.sim_calendar_days_3_18_growth();
    assert!(
        state.trip_served_tiles[20 * CITY_WIDTH + 20],
        "growth-tick trips should mark the origin served before day 19"
    );
    assert!(
        state.city.traffic_density_at(22, 20).unwrap_or(0) > 0,
        "growth-tick trips should bump XTRF along the road corridor"
    );

    state.sim_calendar_day19_traffic();
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        Some(trip_generator::traffic_density_microsim_sum(&state.city)),
        "day-19 traffic pass should store the sum of decayed XTRF microsim cells"
    );
    assert_ne!(
        state.city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        Some(999),
        "day-19 traffic pass should rewrite MISC traffic from live XTRF"
    );
}

#[test]
fn mac68k_day19_decays_xtrf_and_misc_skips_pe_traffic_count() {
    // Mac FUN_0002997c decays/sums XTRF; PE-only 0x4CC6F4 stays Win95.
    let mut city = City::new_blank("/tmp/sc2000-mac-day19.sc2", "MacDay19").expect("blank");
    city.set_traffic_density(20, 20, 100);
    city.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, 999);
    city.set_pe_traffic_count(0x77);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day19_traffic();
    assert_eq!(
        state.city.traffic_density_at(20, 20),
        Some(100 - (100 >> 2)),
        "Mac day-19 decays XTRF like FUN_0002997c"
    );
    assert_ne!(
        state.city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        Some(999),
        "Mac day-19 publishes traffic total"
    );
    assert_eq!(
        state.city.pe_traffic_count(),
        0x77,
        "Mac must not invent PE dwCityTrafficCount"
    );
}

#[test]
fn win95_day19_snaps_pe_traffic_count_from_decayed_xtrf() {
    // Win95 SimulationUpdateMonthlyTrafficData: decay XTRF then snap 0x4CC6F4.
    let mut city = City::new_blank("/tmp/sc2000-win-day19.sc2", "WinDay19").expect("blank");
    city.set_traffic_density(20, 20, 100);
    city.set_pe_traffic_count(0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.sim_calendar_day19_traffic();
    assert_eq!(
        state.city.traffic_density_at(20, 20),
        Some(100 - (100 >> 2))
    );
    let traffic = trip_generator::traffic_density_microsim_sum(&state.city);
    assert_eq!(state.city.pe_traffic_count(), traffic);
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        Some(traffic)
    );
}

#[test]
fn shared_growth_trip_helper_under_mac_fidelity_leaves_xtrf_alone() {
    // This directly exercises the shared PE-style helper with XTRF writes
    // disabled. The production Mac calendar has a separate route-scan writer.
    let mut city = City::new_blank("/tmp/sc2000-mac-trips.sc2", "MacTrips").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(28, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 28, 19);
    city.set_traffic_density(22, 20, 0x55);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.zone_growth_enabled = true;
    state.sim_calendar_days_3_18_growth();
    assert!(
        state.trip_served_tiles[20 * CITY_WIDTH + 20],
        "Mac growth trips still gate origins"
    );
    assert_eq!(
        state.city.traffic_density_at(22, 20),
        Some(0x55),
        "Mac must not invent Win95 trip XTRF bumps"
    );
}

#[test]
fn simulation_month_decays_xtrf_on_day19_after_growth_tick_trips() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_traffic_density(50, 51, 100);
    let mut state = test_state(city);

    // Month-start / growth days must leave leftover congestion intact.
    state.reset_monthly_growth_budget();
    assert_eq!(state.city.traffic_density_at(50, 51), Some(100));

    state.sim_calendar_day19_traffic();
    assert_eq!(
        state.city.traffic_density_at(50, 51),
        Some(75),
        "day 19 SimulationUpdateMonthlyTrafficData decays XTRF by value-(value>>2)"
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        Some(trip_generator::traffic_density_microsim_sum(&state.city))
    );
    assert_eq!(
        state.city.pe_traffic_count(),
        trip_generator::traffic_density_microsim_sum(&state.city),
        "day 19 also snaps dwCityTrafficCount for attitude / January poll"
    );
}

#[test]
fn simulation_day19_decays_post_trip_congestion() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(28, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 28, 19);
    let mut state = test_state(city);
    state.reset_monthly_growth_budget();
    state.sim_calendar_days_3_18_growth();
    let after_trips = state.city.traffic_density_at(22, 20).unwrap_or(0);
    assert!(
        after_trips > 0,
        "growth ticks should leave corridor congestion"
    );

    state.sim_calendar_day19_traffic();
    let after_decay = state.city.traffic_density_at(22, 20).unwrap_or(0);
    assert_eq!(
        after_decay,
        (u16::from(after_trips) - (u16::from(after_trips) >> 2)) as u8,
        "day 19 should decay post-trip congestion in place"
    );
}

#[test]
fn simulation_month_keeps_day19_traffic_sum_through_day21() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(28, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 28, 19);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    let expected = trip_generator::traffic_density_microsim_sum(&state.city);
    assert!(
        expected > 0,
        "month should leave decayed corridor congestion"
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        Some(expected),
        "day-21 misc refresh must keep the day-19 XTRF microsim sum"
    );
}

#[test]
fn simulation_month_does_not_double_count_growth_tick_passengers() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(40, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(40, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    city.set_building(21, 20, 29);
    city.set_building(22, 20, RAIL_STATION_BUILDING_ID);
    for x in 23..=37 {
        city.set_building(x, 20, 45);
    }
    city.set_building(38, 20, RAIL_STATION_BUILDING_ID);
    city.set_building(39, 20, 29);
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 40, 19);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    assert_eq!(
        state.trip_rail_passengers, 2,
        "R+C pop-1 rail trips should count once from growth ticks, not again on day 19"
    );
}

#[test]
fn simulation_month_runs_trips_that_write_xtrf() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(28, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 28, 19);
    let mut state = test_state(city);

    state.refresh_live_trips();

    assert!(
        state.trip_served_tiles[20 * CITY_WIDTH + 20],
        "successful residential trip should mark origin served"
    );
    assert!(
        state.city.traffic_density_at(22, 20).unwrap_or(0) > 0,
        "successful trip should bump XTRF along the road corridor"
    );
}

#[test]
fn win95_trip_simulation_writes_xtrf_on_both_city_file_layouts() {
    let mut outcomes = Vec::new();
    for path in ["/tmp/sc2000-win-trips.sc2", "/tmp/sc2000-win-trips.city"] {
        let mut city = blank_city_layout_fixture(path, "Win95TripsCrossLayout");
        city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
        city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
        city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
        city.set_building(28, 20, COMMERCIAL_1X1_BUILDINGS[0]);
        for x in 21..=27 {
            city.set_building(x, 20, 29);
        }
        power_adjacent_building_for_test(&mut city, 20, 19);
        power_adjacent_building_for_test(&mut city, 28, 19);

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.pe_process_crt = Win95CrtRand::new(0x5a17_2000);
        state.refresh_live_trips();

        outcomes.push((
            state.trip_served_tiles[20 * CITY_WIDTH + 20],
            state.city.traffic_density_at(22, 20),
            state.trip_bus_passengers,
            state.trip_rail_passengers,
            state.trip_subway_passengers,
        ));
    }

    assert_eq!(
        outcomes[0], outcomes[1],
        "engine behavior must not depend on the save container"
    );
    assert!(
        outcomes[0].0,
        "Win95 should serve the residential trip on either layout"
    );
    assert!(
        outcomes[0].1.is_some_and(|density| density > 0),
        "Win95 trips should update XTRF on either layout"
    );
}

#[test]
fn mac68k_refresh_live_trips_leaves_saved_xtrf_alone() {
    let mut city = City::new_blank("/tmp/sc2000-mac-live-trips.sc2", "MacLive").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(28, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 28, 19);
    city.set_traffic_density(22, 20, 0x66);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_live_trips();
    assert!(state.trip_served_tiles[20 * CITY_WIDTH + 20]);
    assert_eq!(state.city.traffic_density_at(22, 20), Some(0x66));
}

#[test]
fn simulation_month_traffic_pass_feeds_transit_query() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(40, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(40, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    city.set_building(21, 20, 29);
    city.set_building(22, 20, RAIL_STATION_BUILDING_ID);
    for x in 23..=37 {
        city.set_building(x, 20, 45);
    }
    city.set_building(38, 20, RAIL_STATION_BUILDING_ID);
    city.set_building(39, 20, 29);
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 40, 19);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    assert!(
        state.trip_rail_passengers > 0,
        "rail corridor trips should accumulate rail passengers"
    );
    assert_eq!(
        population_panel_transit_rows(state.transit_passenger_stats())[1],
        ("Rail Passengers", state.trip_rail_passengers)
    );
    let query = query_tile_status_line_with_passengers(
        &state.city,
        22,
        20,
        false,
        state.transit_passenger_stats(),
        pe_power_recalc_seed(&state.city),
    );
    assert!(
        query.contains(&format!(
            "Passengers/day: {}",
            format_number(state.trip_rail_passengers)
        )),
        "{query}"
    );
    assert!(query.contains("Rail Stations: 2"), "{query}");
}

#[test]
fn rci_growth_requires_successful_trip_without_test_bypass() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = false;

    assert_eq!(state.develop_rci_until_built_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn rci_growth_accepts_successful_road_trip_without_test_bypass() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT);
    city.set_building(28, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 28, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = false;
    state.refresh_live_trips();

    assert_eq!(state.develop_rci_until_built_for_test(), 1);
    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "successful residential trip should allow growth"
    );
}

#[test]
fn growth_tick_trips_mark_served_origins_before_develop() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(28, 20, COMMERCIAL_1X1_BUILDINGS[0]);
    for x in 21..=27 {
        city.set_building(x, 20, 29);
    }
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 28, 19);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = false;
    state.game_clock = GameClock::new_with_day(1900, 1, 2);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 3);
    assert!(
        state.trip_served_tiles[20 * CITY_WIDTH + 20],
        "day-3 growth lattice should run a trip for the residential origin"
    );
    assert!(
        state.city.traffic_density_at(22, 20).unwrap_or(0) > 0,
        "growth-tick trips should bump XTRF along the road corridor"
    );
}

#[test]
fn rci_growth_trip_demand_pair_matches_pe_powered_trip_gate() {
    // Hook init before trip: iCurrentDemand = 0, iRemainderDemand = 4000.
    assert_eq!(rci_growth_trip_demand_pair(1_500, false), (0, 4000));
    assert_eq!(rci_growth_trip_demand_pair(-2_000, false), (0, 4000));
    // Powered trip success: current = demand+2000, remainder = 4000-current.
    assert_eq!(rci_growth_trip_demand_pair(0, true), (2_000, 2_000));
    assert_eq!(rci_growth_trip_demand_pair(500, true), (2_500, 1_500));
    assert_eq!(rci_growth_trip_demand_pair(2_000, true), (4_000, 0));
    assert_eq!(rci_growth_trip_demand_pair(-2_000, true), (0, 4_000));
    // Failed-trip current keeps StartBuilding / abandon thresholds at zero.
    let (failed_current, failed_remainder) = rci_growth_trip_demand_pair(1_000, false);
    assert_eq!(3 * failed_current.max(0) as u32 / 1, 0);
    assert_eq!(15 * failed_current.max(0) as u32 / 1, 0);
    assert_eq!(
        rci_growth_change_threshold(failed_remainder, 1),
        4_000,
        "failed trip leaves ChangeBuilding on the init remainder path"
    );
}

#[test]
fn vacant_rci_startbuilding_accepts_road_in_trip_start_ring() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 20, 23);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_zones(), 1);
    assert!(
        is_construction_building(state.city.building_at(20, 20)),
        "orthogonal +3 is a PE trip-start offset"
    );
}

#[test]
fn vacant_rci_startbuilding_accepts_subway_station_in_trip_start_ring() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(21, 22, SUBWAY_STATION_BUILDING_ID);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
}

#[test]
fn dense_rci_pop3_startbuilding_places_after_road_without_terrain_transport_or_power_gates() {
    let mut state = pop3_size_up_state_for_test(None);
    // The road-orientation marker remains at (19,19), but remove the separate
    // cardinal transport access and put water-class XTER inside the target 3×3.
    state.city.set_building(19, 21, 0);
    state.city.set_terrain(21, 21, 0x10);
    state.bypass_growth_trip_gate = true;
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&state.city, (20, 20), 2);
    let reserved = vec![false; CITY_TILES];
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    set_saved_xbit_growth_power_for_test(&mut power_maps, sx, sy, true);
    let candidate = state
        .rci_expansion_redevelopment_candidate_at(
            sx,
            sy,
            &reserved,
            &power_maps,
            city_service_outcomes_for_city(&state.city),
            &rci_development_services_for_city(&state.city),
            None,
        )
        .expect("PE goes from the accepted road probe directly to DestroyBuilding/picker");

    assert_eq!(candidate.plan.origin, (20, 20));
    assert!(CONSTRUCTION_POP4_BUILDINGS.contains(&candidate.plan.building));
    assert!(is_water_terrain(state.city.terrain_at(21, 21)));
    assert!(check_tile_area(
        &state.city,
        21,
        21,
        state.city.altitude_at(sx, sy),
        ZONE_RESIDENTIAL_DENSE,
        CHECK_TILE_AREA_POP3_MAX_XBLD,
    ));
    assert!(!state.development_footprint_is_flat_land(&candidate.plan.footprint));
    assert!(!state.development_footprint_has_transport_access(&candidate.plan.footprint));
    assert!(power_water::growth_powered_cross_for_engine(
        &power_maps,
        sx,
        sy
    ));
}

#[test]
fn special_zone_growth_develops_airport_and_seaport_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Commercial,
        COMMERCIAL_3X3_BUILDINGS[0],
        (80, 80),
        AIRPORT_DEMAND_MIN_COMMERCIAL_POPULATION + AIRPORT_RUNWAY_CROSS_POPULATION_CAPACITY,
    );
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Industrial,
        INDUSTRIAL_3X3_BUILDINGS[0],
        (80, 104),
        SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION + SEAPORT_CRANE_POPULATION_CAPACITY,
    );
    let airport_origin = (10, 11);
    stamp_zone_footprint_for_test(&mut city, airport_origin, 2, ZONE_AIRPORT);
    stamp_airport_runway_zone_ray_for_test(&mut city, airport_origin);
    city.set_building(11, 10, 6);
    stamp_transport_access_for_test(&mut city, 9, 11);
    power_adjacent_building_for_test(&mut city, 10, 10);
    water_footprint_from_below_for_test(&mut city, 10, 11, 2);

    // Keep this 2×2 pad clear of the airport's eastbound five-cell runway ray.
    let seaport_origin = (16, 10);
    stamp_zone_footprint_for_test(&mut city, seaport_origin, 2, ZONE_SEAPORT);
    stamp_seaport_crane_water_ray_for_test(&mut city, seaport_origin);
    stamp_transport_access_for_test(&mut city, seaport_origin.0 - 1, seaport_origin.1);
    power_adjacent_building_for_test(&mut city, seaport_origin.0, seaport_origin.1 - 1);
    water_footprint_from_below_for_test(&mut city, seaport_origin.0, seaport_origin.1, 2);

    let blocked_origin = (26, 10);
    stamp_zone_footprint_for_test(&mut city, blocked_origin, 2, ZONE_AIRPORT);
    for (x, y) in building_footprint_from_origin(blocked_origin.0, blocked_origin.1, 2).unwrap() {
        city.set_terrain(x, y, 0x30);
    }
    stamp_transport_access_for_test(&mut city, 25, 10);
    power_adjacent_building_for_test(&mut city, 26, 9);
    water_footprint_from_below_for_test(&mut city, 26, 10, 2);
    let mut state = test_state(city);

    seed_special_zone_targets_for_test(
        &mut state,
        &[
            (airport_origin, RUNWAY_BUILDING_ID),
            (seaport_origin, SEAPORT_CRANE_BUILDING_ID),
        ],
    );
    state.develop_special_zones();

    // Empty airport/seaport: Win95 starts with runway / crane (1x1).
    assert_eq!(
        state.city.building_at(airport_origin.0, airport_origin.1),
        RUNWAY_BUILDING_ID
    );
    assert_eq!(
        state
            .city
            .zone_at(airport_origin.0, airport_origin.1)
            .unwrap()
            .zone_id(),
        ZONE_AIRPORT
    );
    assert!(building_internal_pipe_present(
        &state.city,
        airport_origin.0,
        airport_origin.1
    ));
    assert_eq!(
        state.city.building_at(seaport_origin.0, seaport_origin.1),
        SEAPORT_CRANE_BUILDING_ID
    );
    assert_eq!(
        state
            .city
            .zone_at(seaport_origin.0, seaport_origin.1)
            .unwrap()
            .zone_id(),
        ZONE_SEAPORT
    );
    assert!(building_internal_pipe_present(
        &state.city,
        seaport_origin.0,
        seaport_origin.1
    ));

    assert_ne!(state.city.building_at(10, 10), 0);
    // Tree on a non-adjacent pad tile stays: empty airport growth places 1x1 runways
    // only on tiles that themselves have transport access.
    assert_eq!(state.city.building_at(11, 10), 6);

    for (x, y) in building_footprint_from_origin(blocked_origin.0, blocked_origin.1, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), 0);
        assert_eq!(state.city.zone_at(x, y).unwrap().zone_id(), ZONE_AIRPORT);
    }
}

#[test]
fn special_zones_wait_for_transport_access_before_developing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let airport = (10, 11);
    let seaport = (20, 10);
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, airport, 2, ZONE_AIRPORT);
    stamp_airport_runway_zone_ray_for_test(&mut city, airport);
    stamp_zone_footprint_for_test(&mut city, seaport, 2, ZONE_SEAPORT);
    stamp_seaport_crane_water_ray_for_test(&mut city, seaport);
    water_footprint_from_below_for_test(&mut city, airport.0, airport.1, 2);
    water_footprint_from_below_for_test(&mut city, seaport.0, seaport.1, 2);
    let mut state = test_state(city);
    let reserved = vec![false; CITY_TILES];
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    for origin in [airport, seaport] {
        set_saved_xbit_growth_power_for_test(&mut power_maps, origin.0, origin.1 - 1, true);
        assert!(
            state
                .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
                .is_none()
        );
    }

    stamp_transport_access_for_test(&mut state.city, airport.0 - 1, airport.1);
    stamp_transport_access_for_test(&mut state.city, seaport.0 - 1, seaport.1);
    assert!(
        state
            .special_zone_development_plan_at(airport.0, airport.1, &reserved, &power_maps)
            .is_some()
    );
    assert!(
        state
            .special_zone_development_plan_at(seaport.0, seaport.1, &reserved, &power_maps)
            .is_some()
    );
}

#[test]
fn airport_special_zone_growth_preserves_watered_and_xtrf_like_pe_place_tile() {
    // PE SimulationGrowSpecificZone airport PlaceTile: leave watered/XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-air-xtrf.sc2", "AirX").expect("blank");
    let origin = (10, 11);
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_AIRPORT);
    stamp_airport_runway_zone_ray_for_test(&mut city, origin);
    stamp_transport_access_for_test(&mut city, 9, 11);
    power_adjacent_building_for_test(&mut city, 10, 10);
    let mut state = test_state(city);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    let plan = state
        .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
        .expect("airport plan");
    for &(x, y) in &plan.footprint {
        state.city.set_tile_bits(
            x,
            y,
            state
                .city
                .tile_bits_at(x, y)
                .unwrap_or_default()
                .with_piped(true)
                .with_watered(true),
        );
        state.city.set_traffic_density(x, y, 0x2f);
    }
    state.stamp_zone_development(&plan);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        RUNWAY_BUILDING_ID
    );
    for &(x, y) in &plan.footprint {
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped(), "piped cleared at ({x},{y})");
        assert!(bits.watered(), "watered cleared at ({x},{y})");
        assert_eq!(
            state.city.traffic_density_at(x, y),
            Some(0x2f),
            "XTRF mutated at ({x},{y})"
        );
    }
}

#[test]
fn airport_special_zone_growth_preserves_xtxt_like_pe_place_tile() {
    // PE airport SimulationGrowSpecificZone → PlaceTile `0x441f00`: leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-air-xtxt.sc2", "AirT").expect("blank");
    let origin = (10, 11);
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_AIRPORT);
    stamp_airport_runway_zone_ray_for_test(&mut city, origin);
    stamp_transport_access_for_test(&mut city, 9, 11);
    power_adjacent_building_for_test(&mut city, 10, 10);
    let mut state = test_state(city);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    let plan = state
        .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
        .expect("airport plan");
    for &(x, y) in &plan.footprint {
        state.city.set_text_overlay(x, y, 0x2a);
    }
    state.stamp_zone_development(&plan);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        RUNWAY_BUILDING_ID
    );
    for &(x, y) in &plan.footprint {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_airport_special_zone_growth_preserves_xtxt_like_pe_place_tile() {
    // Shared special-zone PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-air-xtxt.sc2", "MacAirT").expect("blank");
    let origin = (10, 10);
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_AIRPORT);
    stamp_transport_access_for_test(&mut city, 9, 10);
    power_adjacent_building_for_test(&mut city, 10, 9);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    let plan = state
        .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
        .expect("airport plan");
    for &(x, y) in &plan.footprint {
        state.city.set_text_overlay(x, y, 0x2a);
    }
    state.stamp_zone_development(&plan);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        RUNWAY_BUILDING_ID
    );
    for &(x, y) in &plan.footprint {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_airport_special_zone_growth_preserves_xtrf_like_pe_place_tile() {
    // Shared special-zone PlaceTile leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-air-xtrf.sc2", "MacAirX").expect("blank");
    let origin = (10, 10);
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_AIRPORT);
    stamp_transport_access_for_test(&mut city, 9, 10);
    power_adjacent_building_for_test(&mut city, 10, 9);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    let plan = state
        .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
        .expect("airport plan");
    for &(x, y) in &plan.footprint {
        state.city.set_traffic_density(x, y, 0x2f);
    }
    state.stamp_zone_development(&plan);
    for &(x, y) in &plan.footprint {
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x2f));
    }
}

#[test]
fn seaport_special_zone_growth_preserves_watered_and_xtrf_like_pe_place_tile() {
    // PE SimulationGrowSpecificZone seaport PlaceTile: leave watered/XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-sea-xtrf.sc2", "SeaX").expect("blank");
    let origin = (10, 10);
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_SEAPORT);
    stamp_seaport_crane_water_ray_for_test(&mut city, origin);
    stamp_transport_access_for_test(&mut city, 9, 10);
    power_adjacent_building_for_test(&mut city, 10, 9);
    let mut state = test_state(city);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    let plan = state
        .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
        .expect("seaport plan");
    for &(x, y) in &plan.footprint {
        state.city.set_tile_bits(
            x,
            y,
            state
                .city
                .tile_bits_at(x, y)
                .unwrap_or_default()
                .with_piped(true)
                .with_watered(true),
        );
        state.city.set_traffic_density(x, y, 0x30);
    }
    state.stamp_zone_development(&plan);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        SEAPORT_CRANE_BUILDING_ID
    );
    for &(x, y) in &plan.footprint {
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped(), "piped cleared at ({x},{y})");
        assert!(bits.watered(), "watered cleared at ({x},{y})");
        assert_eq!(
            state.city.traffic_density_at(x, y),
            Some(0x30),
            "XTRF mutated at ({x},{y})"
        );
    }
}

#[test]
fn seaport_special_zone_growth_preserves_xtxt_like_pe_place_tile() {
    // PE seaport SimulationGrowSpecificZone → PlaceTile `0x441f00`: leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-sea-xtxt.sc2", "SeaT").expect("blank");
    let origin = (10, 10);
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_SEAPORT);
    stamp_seaport_crane_water_ray_for_test(&mut city, origin);
    stamp_transport_access_for_test(&mut city, 9, 10);
    power_adjacent_building_for_test(&mut city, 10, 9);
    let mut state = test_state(city);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    let plan = state
        .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
        .expect("seaport plan");
    for &(x, y) in &plan.footprint {
        state.city.set_text_overlay(x, y, 0x2a);
    }
    state.stamp_zone_development(&plan);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        SEAPORT_CRANE_BUILDING_ID
    );
    for &(x, y) in &plan.footprint {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_seaport_special_zone_growth_preserves_xtxt_like_pe_place_tile() {
    // Shared seaport PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-sea-xtxt.sc2", "MacSeaT").expect("blank");
    let origin = (10, 10);
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_SEAPORT);
    stamp_transport_access_for_test(&mut city, 9, 10);
    power_adjacent_building_for_test(&mut city, 10, 9);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    let plan = state
        .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
        .expect("seaport plan");
    for &(x, y) in &plan.footprint {
        state.city.set_text_overlay(x, y, 0x2a);
    }
    state.stamp_zone_development(&plan);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        SEAPORT_CRANE_BUILDING_ID
    );
    for &(x, y) in &plan.footprint {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_seaport_special_zone_growth_preserves_xtrf_like_pe_place_tile() {
    let mut city = City::new_blank("/tmp/sc2000-mac-sea-xtrf.sc2", "MacSeaX").expect("blank");
    let origin = (10, 10);
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_SEAPORT);
    stamp_transport_access_for_test(&mut city, 9, 10);
    power_adjacent_building_for_test(&mut city, 10, 9);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    let plan = state
        .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
        .expect("seaport plan");
    for &(x, y) in &plan.footprint {
        state.city.set_traffic_density(x, y, 0x30);
    }
    state.stamp_zone_development(&plan);
    for &(x, y) in &plan.footprint {
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x30));
    }
}

#[test]
fn airport_runway_growth_refuses_without_five_matching_zone_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    enable_airport_demand_for_test(&mut city);
    city.set_zone(20, 20, ZONE_AIRPORT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    let mut state = test_state(city);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    assert_eq!(
        select_airport_growth_building(&state.city),
        RUNWAY_BUILDING_ID
    );
    assert!(
        state
            .special_zone_development_plan_at(20, 20, &reserved, &power_maps)
            .is_none(),
        "class-0 runway plan must refuse without its five-cell matching-XZON ray"
    );
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn win95_airport_growth_plan_has_no_extra_global_population_floor() {
    let mut city =
        City::new_blank("/tmp/sc2000-air-no-pop-floor.sc2", "AirNoPopFloor").expect("blank city");
    let origin = (10, 11);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_AIRPORT);
    stamp_airport_runway_zone_ray_for_test(&mut city, origin);
    stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1);
    power_adjacent_building_for_test(&mut city, origin.0, origin.1 - 1);
    city.set_technology_discovery_year(Technology::Airport, 1900);
    let state = test_state(city);

    assert_eq!(
        population_stats_for_city(&state.city).commercial_population,
        0
    );
    assert_eq!(
        select_airport_growth_building(&state.city),
        RUNWAY_BUILDING_ID
    );
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
    let plan = state
        .special_zone_development_plan_at(origin.0, origin.1, &reserved, &power_maps)
        .expect("the rooted airport selector, not a global population floor, owns admission");
    assert_eq!(plan.building, RUNWAY_BUILDING_ID);
}

#[test]
fn airport_zone_growth_waits_for_airport_technology() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_technology_discovery_year(Technology::Airport, 1938);
    enable_airport_demand_for_test(&mut city);
    let origin = (10, 11);
    stamp_zone_footprint_for_test(&mut city, origin, 2, ZONE_AIRPORT);
    stamp_airport_runway_zone_ray_for_test(&mut city, origin);
    stamp_transport_access_for_test(&mut city, 9, 11);
    power_adjacent_building_for_test(&mut city, 10, 10);
    water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 2);
    let mut state = test_state(city);

    state.game_clock = GameClock::new(1900, 1);
    let expected_developments = expected_special_zone_development_count(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    assert_eq!(state.city.building_at(origin.0, origin.1), 0);

    state.game_clock = GameClock::new(1938, 1);
    // Only the transport-adjacent tile of the 2x2 pad can place a 1x1 runway.
    let expected_developments = expected_special_zone_development_count(&state);
    let expected_plan = expected_special_zone_development_plans(&state)
        .into_iter()
        .find(|plan| plan.footprint.contains(&origin));
    assert_eq!(state.develop_special_zones(), expected_developments);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        expected_plan.map_or(0, |plan| plan.building),
        "airport special-zone growth should resume once Airport is invented"
    );
}

#[test]
fn road_neighbor_connections_satisfy_airport_commercial_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Commercial,
        COMMERCIAL_3X3_BUILDINGS[0],
        (80, 80),
        AIRPORT_DEMAND_MIN_COMMERCIAL_POPULATION,
    );

    assert!(special_zone_has_growth_demand(&city, ZONE_AIRPORT));

    city.set_building(
        0,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );

    assert!(!special_zone_has_growth_demand(&city, ZONE_AIRPORT));
}

#[test]
fn rail_and_highway_neighbor_connections_satisfy_seaport_industrial_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Industrial,
        INDUSTRIAL_3X3_BUILDINGS[0],
        (80, 80),
        SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION,
    );

    assert!(special_zone_has_growth_demand(&city, ZONE_SEAPORT));

    city.set_building(
        CITY_WIDTH - 1,
        20,
        network_building_for_mask(NetworkKind::Rail, NETWORK_LEFT | NETWORK_RIGHT),
    );
    assert!(!special_zone_has_growth_demand(&city, ZONE_SEAPORT));

    city.set_building(CITY_WIDTH - 1, 20, 0);
    assert!(special_zone_has_growth_demand(&city, ZONE_SEAPORT));

    city.set_building(CITY_WIDTH - 1, 20, highway_building_for_mask(NETWORK_LEFT));
    assert!(!special_zone_has_growth_demand(&city, ZONE_SEAPORT));
}

#[test]
fn runway_cross_tiles_satisfy_airport_commercial_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Commercial,
        COMMERCIAL_3X3_BUILDINGS[0],
        (80, 80),
        AIRPORT_DEMAND_MIN_COMMERCIAL_POPULATION,
    );

    city.set_zone(20, 20, ZONE_AIRPORT);
    city.set_building(20, 20, 221);
    assert!(special_zone_has_growth_demand(&city, ZONE_AIRPORT));

    city.set_building(20, 20, 222);
    assert!(!special_zone_has_growth_demand(&city, ZONE_AIRPORT));
}

#[test]
fn crane_tiles_satisfy_seaport_industrial_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Industrial,
        INDUSTRIAL_3X3_BUILDINGS[0],
        (80, 80),
        SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION,
    );

    city.set_zone(20, 20, ZONE_SEAPORT);
    city.set_building(20, 20, 223);
    assert!(special_zone_has_growth_demand(&city, ZONE_SEAPORT));

    city.set_building(20, 20, 224);
    assert!(!special_zone_has_growth_demand(&city, ZONE_SEAPORT));
}

#[test]
fn special_zone_growth_rechecks_airport_capacity_between_plans() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Commercial,
        COMMERCIAL_3X3_BUILDINGS[0],
        (80, 80),
        AIRPORT_DEMAND_MIN_COMMERCIAL_POPULATION,
    );
    for origin in [(17, 10), (25, 10)] {
        city.set_zone(origin.0, origin.1, ZONE_AIRPORT);
        stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1);
        power_adjacent_building_for_test(&mut city, origin.0, origin.1 - 1);
        water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 1);
    }
    let mut state = test_state(city);

    // Win95 airport growth starts with runway, which does not add demand capacity
    // (only runway-cross tiles do), so both empty tiles can develop.
    let mut expected_growth = Vec::<(usize, usize)>::new();
    let mut expected_rng = state.pe_process_crt;
    for (step, substep) in sim_calendar_growth_phases() {
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            let Some(zone_id) = state.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
                continue;
            };
            if zone_id == ZONE_AIRPORT && (expected_rng.random_u16() & 3) == 0 {
                expected_growth.push((x, y));
            }
        }
    }

    let developed = state.develop_special_zones();
    assert_eq!(developed, expected_growth.len());
    for origin in [(17, 10), (25, 10)] {
        if expected_growth.contains(&origin) {
            assert_eq!(
                state.city.building_at(origin.0, origin.1),
                RUNWAY_BUILDING_ID
            );
        } else {
            assert_eq!(state.city.building_at(origin.0, origin.1), 0);
        }
    }
}

#[test]
fn special_zone_growth_rechecks_seaport_capacity_between_plans() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Industrial,
        INDUSTRIAL_3X3_BUILDINGS[0],
        (80, 80),
        SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION,
    );
    for origin in [(11, 10), (15, 10)] {
        city.set_zone(origin.0, origin.1, ZONE_SEAPORT);
        stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1);
        power_adjacent_building_for_test(&mut city, origin.0, origin.1 - 1);
        water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 1);
    }
    let mut state = test_state(city);

    let mut expected_growth = Vec::<(usize, usize)>::new();
    let mut expected_rng = state.pe_process_crt;
    for (step, substep) in sim_calendar_growth_phases() {
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            let Some(zone_id) = state.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
                continue;
            };
            if zone_id == ZONE_SEAPORT && (expected_rng.random_u16() & 3) == 0 {
                expected_growth.push((x, y));
            }
        }
    }

    let developed = state.develop_special_zones();
    assert_eq!(developed, expected_growth.len());
    for origin in [(11, 10), (15, 10)] {
        if expected_growth.contains(&origin) {
            assert_eq!(
                state.city.building_at(origin.0, origin.1),
                SEAPORT_CRANE_BUILDING_ID
            );
        } else {
            assert_eq!(state.city.building_at(origin.0, origin.1), 0);
        }
    }
}

#[test]
fn special_zone_growth_roll_airport_polarity_matches_pe_436601() {
    // Airport @ `0x436601` `je` ladder @ `0x43670d` when `(al&3)==0` — same admit
    // shape as seaport/army/AF/navy pickers (silo monthly skips this gate).
    let state = test_state(City::new_blank("/tmp/sc2000-roll-pol.sc2", "RollPol").expect("blank"));
    for seed in 0..64u32 {
        let mut rng = Win95CrtRand::new(seed);
        let mut peek = Win95CrtRand::new(seed);
        let draw = peek.random_u16() & 3;
        assert_eq!(
            state.special_zone_growth_roll_allows(&mut rng, ZONE_AIRPORT),
            draw == 0,
            "airport seed {seed}"
        );
        let mut rng = Win95CrtRand::new(seed);
        assert_eq!(
            state.special_zone_growth_roll_allows(&mut rng, ZONE_SEAPORT),
            draw == 0,
            "seaport seed {seed}"
        );
        let mut rng = Win95CrtRand::new(seed);
        assert_eq!(
            state.special_zone_growth_roll_allows(&mut rng, ZONE_MILITARY),
            draw == 0,
            "military (no silo base) seed {seed}"
        );
    }
}

#[test]
fn special_zone_growth_rolls_every_vacant_airport_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Commercial,
        COMMERCIAL_3X3_BUILDINGS[0],
        (80, 80),
        AIRPORT_DEMAND_MIN_COMMERCIAL_POPULATION,
    );
    let airport_origins = [(20, 21), (26, 21), (32, 21), (38, 21), (44, 21)];
    for (x, y) in airport_origins {
        city.set_zone(x, y, ZONE_AIRPORT);
        stamp_airport_runway_zone_ray_for_test(&mut city, (x, y));
        stamp_transport_access_for_test(&mut city, x - 1, y);
        power_adjacent_building_for_test(&mut city, x, y - 1);
        water_footprint_from_below_for_test(&mut city, x, y, 1);
    }
    let mut state = test_state(city);

    let mut expected_growth = 0usize;
    let mut expected_airports = Vec::<(usize, usize)>::new();
    let mut expected_rng = state.pe_process_crt;
    let mut reserved = vec![false; CITY_TILES];
    for (step, substep) in sim_calendar_growth_phases() {
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            if reserved[y * CITY_WIDTH + x] {
                continue;
            }
            let Some(zone_id) = state.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
                continue;
            };
            if zone_id != ZONE_AIRPORT {
                continue;
            }
            // PE airport @ `0x436601`: `je` to ladder @ `0x43670d` when `(al&3)==0`.
            if (expected_rng.random_u16() & 3) != 0 {
                continue;
            }
            let power_maps =
                power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
            let Some(plan) = state.special_zone_development_plan_at(x, y, &reserved, &power_maps)
            else {
                continue;
            };
            for &(tx, ty) in &plan.footprint {
                reserved[ty * CITY_WIDTH + tx] = true;
            }
            expected_growth += plan.footprint.len();
            expected_airports.push(plan.origin);
        }
    }

    let developed = state.develop_special_zones();
    let mut placed = 0usize;
    for (x, y) in airport_origins {
        if state.city.building_at(x, y) == RUNWAY_BUILDING_ID {
            assert!(
                expected_airports.contains(&(x, y)),
                "runway at ({x},{y}) without CRT/plan admit"
            );
            placed += 1;
        } else {
            assert_eq!(state.city.building_at(x, y), 0);
        }
    }
    assert_eq!(developed, placed);
    assert!(placed <= expected_growth);
}

#[test]
fn special_zone_growth_rolls_every_vacant_seaport_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_special_zone_demand_population_for_test(
        &mut city,
        RciKind::Industrial,
        INDUSTRIAL_3X3_BUILDINGS[0],
        (80, 80),
        SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION,
    );
    for x in [20, 24, 28, 32, 36] {
        city.set_zone(x, 20, ZONE_SEAPORT);
        stamp_transport_access_for_test(&mut city, x - 1, 20);
        power_adjacent_building_for_test(&mut city, x, 19);
        water_footprint_from_below_for_test(&mut city, x, 20, 1);
    }
    let mut state = test_state(city);

    let mut expected_growth = 0usize;
    let mut expected_seaports = Vec::<(usize, usize)>::new();
    let mut expected_rng = state.pe_process_crt;
    let mut reserved = vec![false; CITY_TILES];
    for (step, substep) in sim_calendar_growth_phases() {
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            if reserved[y * CITY_WIDTH + x] {
                continue;
            }
            let Some(zone_id) = state.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
                continue;
            };
            if zone_id != ZONE_SEAPORT {
                continue;
            }
            if (expected_rng.random_u16() & 3) != 0 {
                continue;
            }
            let power_maps =
                power_service_maps_with_entry_seed(&state.city, state.pe_process_crt.seed());
            let Some(plan) = state.special_zone_development_plan_at(x, y, &reserved, &power_maps)
            else {
                continue;
            };
            for &(tx, ty) in &plan.footprint {
                reserved[ty * CITY_WIDTH + tx] = true;
            }
            expected_growth += plan.footprint.len();
            expected_seaports.push(plan.origin);
        }
    }

    let developed = state.develop_special_zones();
    for x in [20, 24, 28, 32, 36] {
        let building = state.city.building_at(x, 20);
        if special_zone_developed_building(ZONE_SEAPORT, building)
            && expected_seaports.contains(&(x, 20))
        {
            // CRT/plan-admitted origin developed.
        } else if !special_zone_developed_building(ZONE_SEAPORT, building) {
            assert_eq!(building, 0);
        }
    }
    assert!(developed <= expected_growth);
}

#[test]
fn special_zone_growth_develops_large_airport_and_seaport_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    enable_special_zone_demand_for_test(&mut city);
    for (zone_id, origin) in [(ZONE_AIRPORT, (20, 21)), (ZONE_SEAPORT, (28, 20))] {
        stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1);
        power_adjacent_building_for_test(&mut city, origin.0, origin.1 - 1);
        water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 2);
        for (x, y) in building_footprint_from_origin(origin.0, origin.1, 2).unwrap() {
            city.set_zone(x, y, zone_id);
        }
        if zone_id == ZONE_AIRPORT {
            stamp_airport_runway_zone_ray_for_test(&mut city, origin);
        } else {
            stamp_seaport_crane_water_ray_for_test(&mut city, origin);
        }
    }
    let mut state = test_state(city);

    seed_special_zone_targets_for_test(
        &mut state,
        &[
            ((20, 21), RUNWAY_BUILDING_ID),
            ((28, 20), SEAPORT_CRANE_BUILDING_ID),
        ],
    );
    state.develop_special_zones();

    assert_eq!(state.city.building_at(20, 21), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.zone_at(20, 21).unwrap().zone_id(), ZONE_AIRPORT);
    assert!(building_internal_pipe_present(&state.city, 20, 21));
    assert!(
        state
            .city
            .tile_bits_at(20, 21)
            .is_some_and(|bits| !bits.piped() && !bits.watered()),
        "class-0 runway PlaceTile leaves the XBIT service flags clear"
    );

    assert_eq!(state.city.building_at(28, 20), SEAPORT_CRANE_BUILDING_ID);
    assert_eq!(state.city.zone_at(28, 20).unwrap().zone_id(), ZONE_SEAPORT);
    assert!(building_internal_pipe_present(&state.city, 28, 20));
    assert!(
        state
            .city
            .tile_bits_at(28, 20)
            .is_some_and(|bits| bits.piped())
    );
}

#[test]
fn loaded_mac_city_water_worker_snapshot_reports_saved_and_predicted_state() {
    let Some(path) = paths::test_mac_city() else {
        return;
    };
    let city = City::load(path).expect("load optional Mac water fixture");
    let saved_date = city.saved_date();
    let saved_water_percent = city.mac_state_misc_u32_at(1049 * 4).unwrap_or(0);
    let selected_height = city
        .mac_selected_water_height()
        .expect("Mac selected water height");
    let powered_pump_cells = (0..CITY_TILES)
        .filter(|&index| {
            let x = index % CITY_WIDTH;
            let y = index / CITY_WIDTH;
            city.building_at(x, y) == 220 && city.tile_bits_at(x, y).is_some_and(TileBits::powered)
        })
        .count();
    let saved_served_markers = (0..CITY_TILES)
        .filter(|&index| {
            city.tile_bits_at(index % CITY_WIDTH, index / CITY_WIDTH)
                .is_some_and(TileBits::watered)
        })
        .count();
    let maps = water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Mac68k);
    let predicted_served_markers = maps.watered_tiles.iter().filter(|served| **served).count();

    assert!(maps.mac_worker_contract_complete);
    assert_eq!(saved_served_markers, predicted_served_markers);
    eprintln!(
        "Mac water fixture: saved_date={saved_date:?}, selected_height={selected_height}, powered_pump_cells={powered_pump_cells}, saved_percent={saved_water_percent}, capacity_units={}, generic_served_cells={}, predicted_percent={:?}, served_markers={saved_served_markers}",
        maps.mac_worker_capacity_units,
        maps.mac_worker_served_cells,
        maps.mac_worker_utilization_percent(),
    );
}

#[test]
fn loaded_maxis_land_has_no_rendered_dispatch_actor_proxies() {
    let Some(path) = paths::test_mac_city() else {
        return;
    };
    let city = City::load(path).expect("load default city");
    let mut actors = Vec::new();

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if let Some((slot, record)) = city.dispatch_actor_at(x, y) {
                actors.push((x, y, slot, record.kind, city.building_at(x, y)));
            }
        }
    }

    assert!(
        actors.is_empty(),
        "loaded Maxis Land should not render saved buildings as XTHG dispatch actors: {actors:?}"
    );
}

#[test]
fn load_win95_city_resumes_growth_for_imported_zones() {
    let path = std::env::temp_dir().join("sc2000_rust_load_growth_gate.sc2");
    let mut city = City::new_blank(&path, "Reloaded").expect("blank city");
    city.save(&path).expect("save city");
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));
    assert!(state.zone_growth_enabled);

    state.load_city(&path, (800, 600), 1.0).expect("load city");

    assert!(state.zone_growth_enabled);
    assert!(!state.zone_growth_uses_allowlist);
    assert!(state.zone_growth_can_touch_tile(20, 20));
    let _ = std::fs::remove_file(path);
}

#[test]
fn load_city_clears_legacy_dispatch_actor_proxies() {
    let path = std::env::temp_dir().join("sc2000_rust_load_dispatch_proxy_cleanup.sc2");
    let mut city = City::new_blank(&path, "Reloaded").expect("blank city");
    let mut table = crate::win95_overlay::empty_overlay_table();
    table[1] = crate::win95_overlay::Win95OverlayRecord {
        kind: DispatchKind::Police.overlay_kind(),
        tile_x: 20,
        tile_y: 21,
        saved_a: 0,
        saved_b: 0,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    city.set_building(20, 21, 202);
    city.save(&path).expect("save city");
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));

    state.load_city(&path, (800, 600), 1.0).expect("load city");

    assert_eq!(state.city.building_at(20, 21), 0);
    assert!(state.city.dispatch_actor_at(20, 21).is_none());
    assert_eq!(
        state.city.traffic_overlays().unwrap()[1],
        crate::win95_overlay::Win95OverlayRecord {
            tile_x: 20,
            tile_y: 21,
            ..Default::default()
        },
        "PE dispatch cleanup clears actor kind but retains the rest of the record"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn manual_zoning_does_not_hide_other_imported_zone_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    for x in 20..=22 {
        stamp_transport_access_for_test(&mut city, x, 19);
    }
    power_adjacent_building_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 23, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_zone(21, 20, ZONE_RESIDENTIAL_LIGHT);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.reset_zone_growth_for_city();
    state.tool = EditTool::ZoneResidential;

    assert_eq!(state.place_zone_at(22, 20, ZONE_RESIDENTIAL_LIGHT), 1);

    assert!(state.zone_growth_can_touch_tile(20, 20));
    assert!(state.zone_growth_can_touch_tile(21, 20));
    assert!(state.zone_growth_can_touch_tile(22, 20));
}

#[test]
fn bulldoze_size1_support_walk_land_dec_like_pe_458f90() {
    // Bulldoze shares DestroyBuilding support-walk tip land-- before clear.
    let mut city = City::new_blank("/tmp/sc2000-bull-support.sc2", "BullSupport").expect("blank");
    city.set_mac_selected_water_height(15);
    city.set_building(5, 20, 0x51);
    city.set_building(6, 20, 0x51);
    city.set_tile_bits(6, 20, TileBits::new(0).with_rotate(true));
    city.set_altitude(4, 20, 10, 0);
    city.set_altitude(5, 20, 10, 0);
    city.set_altitude(6, 20, 10, 0);
    city.set_altitude(0, 0, city.altitude_at(0, 0) & 0x1f, 15);
    let mut state = test_state(city);
    state.bulldoze_at(6, 20);
    assert_ne!(state.city.building_at(6, 20), 0x51);
    assert_eq!(state.city.altitude_at(4, 20) & 0x1f, 9);
    assert!(
        state
            .city
            .tile_bits_at(4, 20)
            .is_some_and(TileBits::water_tile)
    );
}

#[test]
fn place_xund_occupancy_inc_subway_1() {
    // PE PlaceXUND @ `0x4423d0`/`0x4423f4`: new tile in subway band incs `0x4ca41c`.
    let mut city = City::new_blank("/tmp/sc2000-xund-occ-inc.sc2", "XundOccInc").expect("blank");
    assert_eq!(city.pe_xund_occupancy, 0);
    pe_place_xund_442370(&mut city, 20, 20, 1);
    assert_eq!(city.underground_at(20, 20), 1);
    assert_eq!(city.pe_xund_occupancy, 1);
}

#[test]
fn xgrp_latest_values_prefer_crime_traffic_pollution_le_eq_land_bss() {
    let mut city = City::new_blank("/tmp/sc2000-xgrp-ctp.sc2", "XgrpCtp").expect("blank");
    city.set_pe_traffic_count(400);
    city.pe_dw_city_pollution = 500;
    city.pe_dw_city_land_value = 600;
    city.pe_dw_city_crime = 700;
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 71);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 88);
    let latest = xgrp_history_latest_values(
        &city,
        PopulationStats::default(),
        CityServiceOutcomes {
            life_expectancy: 1,
            education_quotient: 2,
            land_value: 3,
            ..Default::default()
        },
    );
    let developed_divisor = pe_attitude_developed_tile_count(&city) / 4 + 1;
    assert_eq!(latest[4], 400); // traffic / (0+0+0+1)
    assert_eq!(latest[5], 500 / developed_divisor);
    assert_eq!(latest[6], 600 / developed_divisor);
    assert_eq!(latest[7], 700 / developed_divisor);
    assert_eq!(latest[10], 71);
    assert_eq!(latest[11], 88);
}

#[test]
fn set_misc_syncs_crime_traffic_pollution_land_value_bss() {
    let mut city = City::new_blank("/tmp/sc2000-misc-ctp-sync.sc2", "MiscCtp").expect("blank");
    city.set_misc_u32_at(MISC_CITY_VALUE_OFFSET, 12_345);
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 6_789);
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 111);
    city.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, 222);
    city.set_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, 333);
    assert_eq!(city.pe_city_value, 12_345);
    assert_eq!(city.pe_dw_city_land_value, 6_789);
    assert_eq!(city.pe_dw_city_crime, 111);
    assert_eq!(city.pe_traffic_count(), 222);
    assert_eq!(city.pe_dw_city_pollution, 333);
}

#[test]
fn misc_crime_traffic_pollution_count_prefer_bss() {
    let mut city = City::new_blank("/tmp/sc2000-misc-ctp-bss.sc2", "MiscCtpBss").expect("blank");
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 1);
    city.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, 2);
    city.set_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, 3);
    city.pe_dw_city_crime = 111;
    city.set_pe_traffic_count(222);
    city.pe_dw_city_pollution = 333;
    assert_eq!(city.misc_crime_count(), Some(111));
    assert_eq!(city.misc_traffic_count(), Some(222));
    assert_eq!(city.misc_pollution_count(), Some(333));
}

#[test]
fn pe_transit_passenger_bss_4c85a0_hydrate_and_microsim() {
    let path = "/tmp/sc2000-transit-pax-bss.sc2";
    let mut city = City::new_blank(path, "PaxBss").expect("blank");
    city.set_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, 111);
    city.set_misc_u32_at(MISC_RAIL_PASSENGERS_OFFSET, 222);
    city.set_misc_u32_at(MISC_SUBWAY_PASSENGERS_OFFSET, 333);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_bus_passengers(), 111);
    assert_eq!(loaded.pe_dw_rail_passengers(), 222);
    assert_eq!(loaded.pe_dw_subway_passengers(), 333);
    let mut city = City::new_blank("/tmp/sc2000-transit-pax-bss2.sc2", "PaxBss2").expect("blank");
    city.set_misc_u32_at(MISC_BUS_PASSENGERS_OFFSET, 1);
    city.set_pe_dw_bus_passengers(9_001);
    assert_eq!(city.pe_dw_bus_passengers(), 9_001);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_w_sports_teams_bss_4cc4e0_hydrate_and_persist() {
    let path = "/tmp/sc2000-sports-bss.sc2";
    let mut city = City::new_blank(path, "SportsBss").expect("blank");
    city.set_misc_u32_at(MISC_SPORTS_TEAMS_OFFSET, 0x000f);
    assert_eq!(city.pe_w_sports_teams(), 0x000f);
    city.set_pe_w_sports_teams(0x001f);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_sports_teams(), 0x001f);
    assert_eq!(loaded.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET), Some(0x001f));
    let mut city = City::new_blank("/tmp/sc2000-sports-bss2.sc2", "SportsBss2").expect("blank");
    city.set_misc_u32_at(MISC_SPORTS_TEAMS_OFFSET, 1);
    city.set_pe_w_sports_teams(7);
    assert_eq!(city.pe_w_sports_teams(), 7);
    assert_eq!(sports_teams_for_city(&city), 7);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_advance_w_sports_teams_4837a5_milestone_ladder() {
    // PE @ `0x4837a5`: jump table `0→1→3→7→0xf→0x1f` with team indices 0..=4.
    let mut city =
        City::new_blank("/tmp/sc2000-sports-advance.sc2", "SportsAdvance").expect("blank");
    assert_eq!(city.pe_advance_w_sports_teams_4837a5(), Some(0));
    assert_eq!(city.pe_w_sports_teams(), 1);
    assert_eq!(city.pe_advance_w_sports_teams_4837a5(), Some(1));
    assert_eq!(city.pe_w_sports_teams(), 3);
    assert_eq!(city.pe_advance_w_sports_teams_4837a5(), Some(2));
    assert_eq!(city.pe_w_sports_teams(), 7);
    assert_eq!(city.pe_advance_w_sports_teams_4837a5(), Some(3));
    assert_eq!(city.pe_w_sports_teams(), 0xf);
    assert_eq!(city.pe_advance_w_sports_teams_4837a5(), Some(4));
    assert_eq!(city.pe_w_sports_teams(), 0x1f);
    // `> 0xf` → PE `ja` default: mask unchanged.
    assert_eq!(city.pe_advance_w_sports_teams_4837a5(), None);
    assert_eq!(city.pe_w_sports_teams(), 0x1f);
    // Off-ladder mask (e.g. 2) → jump-table slot 5: no mask change.
    city.set_pe_w_sports_teams(2);
    assert_eq!(city.pe_advance_w_sports_teams_4837a5(), None);
    assert_eq!(city.pe_w_sports_teams(), 2);
}

#[test]
fn pe_write_sports_team_index_to_microsim_483816_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-sports-microsim.sc2", "SportsMicro").expect("blank");
    let xtxt = city
        .provision_microsim_for_building(215)
        .expect("stadium microsim");
    assert!(city.pe_write_sports_team_index_to_microsim_483816(xtxt, 3));
    let index = usize::from(xtxt - MIN_SIM_TEXT_ENTRIES);
    let record = city.microsim_at(index).expect("record");
    assert_eq!(record.stat2, 3);
    assert_eq!(record.stat3, 0xfb + 3);
}

#[test]
fn pe_or_w_sports_teams_bit_47ce74_ors_team_bit() {
    // PE team-name dialog @ `0x47ce74`: OR 1<<index into wSportsTeams.
    let mut city = City::new_blank("/tmp/sc2000-sports-or.sc2", "SportsOr").expect("blank");
    city.set_pe_w_sports_teams(0x01);
    city.pe_or_w_sports_teams_bit_47ce74(2);
    assert_eq!(city.pe_w_sports_teams(), 0x05);
    city.pe_or_w_sports_teams_bit_47ce74(2); // idempotent
    assert_eq!(city.pe_w_sports_teams(), 0x05);
    city.pe_or_w_sports_teams_bit_47ce74(4);
    assert_eq!(city.pe_w_sports_teams(), 0x15);
    city.pe_or_w_sports_teams_bit_47ce74(17);
    assert_eq!(city.pe_w_sports_teams(), 0x15);
    city.pe_or_w_sports_teams_bit_47ce74(33);
    assert_eq!(city.pe_w_sports_teams(), 0x17);
}

#[test]
fn pe_newspaper_stadium_sports_reward_4837a5_advances_and_writes_microsim() {
    let mut city = City::new_blank("/tmp/sc2000-sports-reward.sc2", "SportsReward").expect("blank");
    let mut crt = Win95CrtRand::new(1);
    let xtxt = city
        .provision_microsim_for_building(215)
        .expect("stadium microsim");
    assert!(city.pe_newspaper_stadium_sports_reward_4837a5(xtxt, &mut crt));
    assert_eq!(city.pe_w_sports_teams(), 1);
    let index = usize::from(xtxt - MIN_SIM_TEXT_ENTRIES);
    let record = city.microsim_at(index).expect("record");
    assert_eq!(record.stat2, 0);
    assert_eq!(record.stat3, 0xfb);
    assert!(city.pe_newspaper_stadium_sports_reward_4837a5(xtxt, &mut crt));
    assert_eq!(city.pe_w_sports_teams(), 3);
    let record = city.microsim_at(index).expect("record");
    assert_eq!(record.stat2, 1);
    assert_eq!(record.stat3, 0xfc);
}

#[test]
fn pe_newspaper_stadium_sports_reward_4837bd_crt_fallback_keeps_mask() {
    // PE @ `0x4837bd`: mask `> 0xf` or off-ladder → `rand() % 5`, leave `0x4cc4e0`,
    // still write microsim @ `0x483816`.
    let mut city = City::new_blank("/tmp/sc2000-sports-crt.sc2", "SportsCrt").expect("blank");
    let xtxt = city
        .provision_microsim_for_building(215)
        .expect("stadium microsim");
    city.set_pe_w_sports_teams(0x1f);
    // Seed so first random_mod(5) yields 2.
    let mut seed = 1u32;
    let mut crt = Win95CrtRand::new(seed);
    let mut bit = crt.random_mod(5);
    while bit != 2 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
        bit = crt.random_mod(5);
    }
    crt = Win95CrtRand::new(seed);
    assert!(city.pe_newspaper_stadium_sports_reward_4837a5(xtxt, &mut crt));
    assert_eq!(city.pe_w_sports_teams(), 0x1f, "CRT arm leaves mask");
    let index = usize::from(xtxt - MIN_SIM_TEXT_ENTRIES);
    let record = city.microsim_at(index).expect("record");
    assert_eq!(record.stat2, 2);
    assert_eq!(record.stat3, 0xfb + 2);

    city.set_pe_w_sports_teams(2);
    seed = 1;
    crt = Win95CrtRand::new(seed);
    bit = crt.random_mod(5);
    while bit != 4 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
        bit = crt.random_mod(5);
    }
    crt = Win95CrtRand::new(seed);
    assert!(city.pe_newspaper_stadium_sports_reward_4837a5(xtxt, &mut crt));
    assert_eq!(city.pe_w_sports_teams(), 2, "off-ladder leaves mask");
    let record = city.microsim_at(index).expect("record");
    assert_eq!(record.stat2, 4);
    assert_eq!(record.stat3, 0xfb + 4);
}

#[test]
fn pe_rci_demand_updates_sports_newspaper_4687f3_queues_story_8() {
    // PE FUN_00468150 @ `0x4687f3`: stadium tiles + sports bit → story 8.
    let mut city = City::new_blank("/tmp/sc2000-sports-news.sc2", "SportsNews").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 215, 4);
    city.set_pe_w_sports_teams(0x1f);
    // Seed so first random_mod(5) yields bit 2 (mask 0x4).
    let mut crt = Win95CrtRand::new(1);
    let mut bit = crt.random_mod(5);
    let mut seed = 1u32;
    while bit != 2 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
        bit = crt.random_mod(5);
    }
    crt = Win95CrtRand::new(seed);
    assert!(pe_rci_demand_updates_sports_newspaper_4687f3(
        &mut city, &mut crt
    ));
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(8));
    assert_eq!(city.pe_newspaper_story_param_at(0), Some(2));
}

#[test]
fn pe_rci_demand_updates_sports_newspaper_4687f3_skips_without_stadium() {
    let mut city =
        City::new_blank("/tmp/sc2000-sports-news-skip.sc2", "SportsNewsSkip").expect("blank");
    city.set_pe_w_sports_teams(0x1f);
    let mut crt = Win95CrtRand::new(1);
    let before = crt.seed();
    assert!(!pe_rci_demand_updates_sports_newspaper_4687f3(
        &mut city, &mut crt
    ));
    assert_eq!(crt.seed(), before); // PE jle before rand
    assert_eq!(
        city.pe_newspaper_story_id_at(0),
        Some(0),
        "an empty initialized story slot is represented by ID zero"
    );
}

#[test]
fn city_load_hydrates_pe_traffic_count_from_misc_43088a() {
    // PE MISC→BSS @ `0x430893`: `[MISC+0x30]` → `[0x4cc6f4]`.
    let path = "/tmp/sc2000-traffic-hydrate.sc2";
    let mut city = City::new_blank(path, "TrafficHydrate").expect("blank");
    city.set_pe_traffic_count(0x1357_9bdf);
    city.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, 0x1357_9bdf);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_traffic_count(), 0x1357_9bdf);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_traffic_count_to_misc_432ef7() {
    // PE BSS→MISC @ `0x432f0a`: `[0x4cc6f4]` → `[MISC+0x30]`.
    let path = "/tmp/sc2000-traffic-persist.sc2";
    let mut city = City::new_blank(path, "TrafficPersist").expect("blank");
    city.set_pe_traffic_count(9_001);
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET), Some(9_001));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_traffic_count(), 9_001);
    let _ = std::fs::remove_file(path);
}

#[test]
fn bulldoze_preserves_dispatch_proxy_xtxt_like_pe_destroy_building() {
    // PE DestroyBuilding @ `0x45a0f6`: XTXT `0xc9..=0xf0` → `0x401dca` only, leave XTXT.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-proxy.sc2", "BullXtxtProxy").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xd0);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xd0));
}

#[test]
fn mac68k_bulldoze_preserves_dispatch_proxy_xtxt_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-proxy-mac.sc2", "BullXtxtProxyMac").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xe8);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xe8));
}

#[test]
fn dirty_traffic_redraw_trigger_only_matches_traffic_roads() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut dirty = HashSet::new();
    dirty.insert((10, 10));

    city.set_building(10, 10, 112);
    assert!(!dirty_tiles_repaint_traffic(
        &city,
        EngineFidelity::Win95,
        &dirty
    ));

    city.set_building(10, 10, 29);
    assert!(dirty_tiles_repaint_traffic(
        &city,
        EngineFidelity::Win95,
        &dirty
    ));
}

#[test]
fn mac_static_traffic_only_draws_on_road_or_highway_surface() {
    assert!(mac_static_traffic_has_road_surface(29));
    assert!(mac_static_traffic_has_road_surface(73));
    assert!(!mac_static_traffic_has_road_surface(44));
    assert!(!mac_static_traffic_has_road_surface(67));
    assert!(!mac_static_traffic_has_road_surface(112));
}

#[test]
fn static_traffic_epoch_only_changes_on_dirty_traffic_redraw() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 29);
    let traffic = TrafficSystem::new(&city);
    let mut win95_overlay_sim = Win95OverlaySim::new();
    win95_overlay_sim.reset_for_city(&city);
    let pe_process_crt = init_pe_process_crt(&city, GameClock::default_start());
    let pe_process_game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
    let pe_process_lfsr = init_pe_process_lfsr(&city, GameClock::default_start());
    let mut state = ViewerState {
        city,
        camera: Vec2::ZERO,
        zoom: 2.0,
        viewport_pixel_scale: 1.0,
        camera_rotation: 0,
        show_grid: false,
        dark_appearance: false,
        show_zone_overlay: false,
        show_traffic: true,
        data_map_mode: CityDataMapMode::City,
        show_buildings: true,
        show_signs: true,
        show_infrastructure: true,
        show_zones: true,
        show_underground: false,
        auto_goto_enabled: true,
        pending_disaster_auto_goto: None,
        pending_disaster_start_sound: None,
        pending_disaster_start_siren: false,
        pending_disaster_extra_sounds: Vec::new(),
        earthquake_shake_remaining: 0,
        pe_disaster_duration: 0,
        pe_hurricane_wind_remaining: 0,
        sound_effects_enabled: true,
        music_enabled: true,
        no_disasters_enabled: false,
        balloon_help_enabled: false,
        balloon_help_line: None,
        anim_time: 0.0,
        traffic_redraw_epoch: 7,
        traffic_redraw_accumulator: 0.0,
        pending_traffic_redraw: false,
        pe_idle_scheduler: pe_idle_scheduler::PeIdleScheduler::default(),
        mac_loop_scheduler: mac_loop_scheduler::MacLoopScheduler::default(),
        traffic,
        win95_overlay_sim,
        pe_process_crt,
        mac_process_rng: MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED),
        mac_kind2_sound_deadline: 0,
        mac_game_rng: crate::mac_random::MacGameLcgRng::new(
            crate::mac_random::MacGameLcgRng::INITIAL_STATE,
        ),
        pe_process_game_rand,
        pe_raise_submode: PE_RAISE_SUBMODE_DEFAULT,
        pe_raise_adjust_land: 0,
        pe_raise_xter_ui_byte: 0,
        pe_raise_ui_tool_id: 0,
        pe_raise_ui_tile_x: 0,
        pe_raise_ui_tile_y: 0,
        pe_raise_press_y: 0,
        pe_raise_ui_press_y_byte: 0,
        pe_raise_ui_byte6: 0,
        pe_raise_ui_byte7: 0,
        pe_raise_ui_byte8: 0,
        pe_raise_ui_byte9: 0,
        pe_raise_drag_dir: 0xffff,
        pe_raise_this_f8: 0,
        pe_spread_tool4_ui_timer: 0,
        pe_spread_prev_tile_x: 0,
        pe_spread_prev_tile_y: 0,
        pe_stretch_hold_si: 0,
        pe_stretch_hold_ui_refresh_count: 0,
        pe_stretch_hold_cursor_id: 0,
        pe_stretch_hold_cursor_arg: 0,
        pe_stretch_hold_ui_wait_4012c1_count: 0,
        pe_lower_hold_ui_refresh_count: 0,
        pe_lower_hold_cursor_id: 0,
        pe_lower_hold_cursor_arg: 0,
        pe_level_hold_sound_after_altm: 0,
        pe_level_hold_sound_after_smooth: 0,
        pe_level_hold_ui_refresh_count: 0,
        pe_level_hold_cursor_id: 0,
        pe_level_hold_cursor_arg: 0,
        pe_level_hold_ui_wait_4012c1_count: 0,
        pe_spread_hold_ui_refresh_count: 0,
        pe_spread_hold_cursor_id: 0,
        pe_spread_hold_cursor_arg: 0,
        pe_spread_ui_cursor_4519e0_count: 0,
        pe_spread_ui_cursor_4519e0_last_arg: 0,
        pe_spread_ui_wait_4012c1_count: 0,
        pe_raise_hold_ui_refresh_count: 0,
        pe_raise_hold_cursor_id: 0,
        pe_raise_hold_cursor_arg: 0,
        pe_raise_ui_cursor_4519e0_count: 0,
        pe_raise_ui_cursor_4519e0_last_arg: 0,
        pe_raise_ui_wait_4012c1_count: 0,
        pe_raise_midband_refuse_sound_401096_count: 0,
        pe_raise_midband_refuse_dialog_402c4d_count: 0,
        pe_raise_midband_refuse_dialog_402c4d_a: 0,
        pe_raise_midband_refuse_dialog_402c4d_b: 0,
        pe_raise_midband_refuse_dialog_402c4d_c: 0,
        pe_raise_midband_refuse_ui_402900_count: 0,
        pe_raise_midband_refuse_ui_402900_a: 0,
        pe_raise_midband_refuse_ui_402900_b: 0,
        pe_raise_hold_blit_count: 0,
        pe_lower_hold_blit_count: 0,
        pe_process_lfsr,
        engine_fidelity: EngineFidelity::Win95,
        pe_growth_aggregate: PeGrowthAggregate::default(),
        game_clock: GameClock::default_start(),
        zone_growth_enabled: false,
        zone_growth_uses_allowlist: true,
        zone_growth_allowed_tiles: vec![false; CITY_TILES],
        trip_served_tiles: vec![false; CITY_TILES],
        bypass_growth_trip_gate: false,
        trip_bus_passengers: 0,
        trip_rail_passengers: 0,
        trip_subway_passengers: 0,
        scenario_start_clock: None,
        announced_scenario_outcome: None,
        announced_bankruptcy: false,
        edit_map_generation_seed: MacToolboxRng::DEFAULT_SEED,
        edit_map_sliders: ui::EditMapSliderValues::default(),
        edit_map_slider_drag: None,
        mode: AppMode::CityView,
        tool: EditTool::Query,
        open_tool_menu: None,
        hover_tile: None,
        zone_drag: None,
        network_drag: None,
        underground_drag: None,
        highway_drag: None,
        terrain_drag: None,
        edit_map_stretch_drag: None,
        edit_map_lower_drag: None,
        edit_map_level_drag: None,
        nature_drag: None,
        power_plant_dialog: None,
        arcology_dialog: None,
        sign_dialog: None,
        military_base_dialog: None,
        city_info_dialog: None,
            status_panel_origin: None,
            status_panel_grab: None,
            info_panel_origin: None,
            info_panel_scale: 1.0,
            info_panel_drag: None,
        founding_newspaper_pending: false,
        graph_series_visible: [true; GRAPH_HISTORY_TOTAL_SERIES],
        graph_scale: GraphScale::Year1,
        query_line: String::new(),
        query_is_inspect: false,
        query_status_pinned: false,
        sprite_cache: SpriteCache::new(),
        palette_press: None,
        menu_checks: crate::menu_model::MenuCheckState::default(),
        menubar: crate::menubar::MenubarUi::new(),
    };

    state.traffic.set_speed(GameSpeed::AfricanSwallow);
    assert!(!state.consume_traffic_redraw_request(&HashSet::new()));
    assert_eq!(state.traffic_redraw_epoch, 7);

    let mut dirty = HashSet::new();
    dirty.insert((10, 10));
    assert!(state.consume_traffic_redraw_request(&dirty));
    assert_eq!(state.traffic_redraw_epoch, 8);
}

#[test]
fn static_traffic_epoch_does_not_advance_on_speed_clock_only() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 29);
    city.set_traffic_density(10, 10, crate::traffic::MAC_TRAFFIC_VISIBLE_BYTE_MIN);
    let traffic = TrafficSystem::new(&city);
    let mut win95_overlay_sim = Win95OverlaySim::new();
    win95_overlay_sim.reset_for_city(&city);
    let pe_process_crt = init_pe_process_crt(&city, GameClock::default_start());
    let pe_process_game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
    let pe_process_lfsr = init_pe_process_lfsr(&city, GameClock::default_start());
    let mut state = ViewerState {
        city,
        camera: Vec2::ZERO,
        zoom: 2.0,
        viewport_pixel_scale: 1.0,
        camera_rotation: 0,
        show_grid: false,
        show_zone_overlay: false,
        dark_appearance: false,
        show_traffic: true,
        data_map_mode: CityDataMapMode::City,
        show_buildings: true,
        show_signs: true,
        show_infrastructure: true,
        show_zones: true,
        show_underground: false,
        auto_goto_enabled: true,
        pending_disaster_auto_goto: None,
        pending_disaster_start_sound: None,
        pending_disaster_start_siren: false,
        pending_disaster_extra_sounds: Vec::new(),
        earthquake_shake_remaining: 0,
        pe_disaster_duration: 0,
        pe_hurricane_wind_remaining: 0,
        sound_effects_enabled: true,
        music_enabled: true,
        no_disasters_enabled: false,
        balloon_help_enabled: false,
        balloon_help_line: None,
        anim_time: 0.0,
        traffic_redraw_epoch: 7,
        traffic_redraw_accumulator: 0.0,
        pending_traffic_redraw: false,
        pe_idle_scheduler: pe_idle_scheduler::PeIdleScheduler::default(),
        mac_loop_scheduler: mac_loop_scheduler::MacLoopScheduler::default(),
        traffic,
        win95_overlay_sim,
        pe_process_crt,
        mac_process_rng: MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED),
        mac_kind2_sound_deadline: 0,
        mac_game_rng: crate::mac_random::MacGameLcgRng::new(
            crate::mac_random::MacGameLcgRng::INITIAL_STATE,
        ),
        pe_process_game_rand,
        pe_raise_submode: PE_RAISE_SUBMODE_DEFAULT,
        pe_raise_adjust_land: 0,
        pe_raise_xter_ui_byte: 0,
        pe_raise_ui_tool_id: 0,
        pe_raise_ui_tile_x: 0,
        pe_raise_ui_tile_y: 0,
        pe_raise_press_y: 0,
        pe_raise_ui_press_y_byte: 0,
        pe_raise_ui_byte6: 0,
        pe_raise_ui_byte7: 0,
        pe_raise_ui_byte8: 0,
        pe_raise_ui_byte9: 0,
        pe_raise_drag_dir: 0xffff,
        pe_raise_this_f8: 0,
        pe_spread_tool4_ui_timer: 0,
        pe_spread_prev_tile_x: 0,
        pe_spread_prev_tile_y: 0,
        pe_stretch_hold_si: 0,
        pe_stretch_hold_ui_refresh_count: 0,
        pe_stretch_hold_cursor_id: 0,
        pe_stretch_hold_cursor_arg: 0,
        pe_stretch_hold_ui_wait_4012c1_count: 0,
        pe_lower_hold_ui_refresh_count: 0,
        pe_lower_hold_cursor_id: 0,
        pe_lower_hold_cursor_arg: 0,
        pe_level_hold_sound_after_altm: 0,
        pe_level_hold_sound_after_smooth: 0,
        pe_level_hold_ui_refresh_count: 0,
        pe_level_hold_cursor_id: 0,
        pe_level_hold_cursor_arg: 0,
        pe_level_hold_ui_wait_4012c1_count: 0,
        pe_spread_hold_ui_refresh_count: 0,
        pe_spread_hold_cursor_id: 0,
        pe_spread_hold_cursor_arg: 0,
        pe_spread_ui_cursor_4519e0_count: 0,
        pe_spread_ui_cursor_4519e0_last_arg: 0,
        pe_spread_ui_wait_4012c1_count: 0,
        pe_raise_hold_ui_refresh_count: 0,
        pe_raise_hold_cursor_id: 0,
        pe_raise_hold_cursor_arg: 0,
        pe_raise_ui_cursor_4519e0_count: 0,
        pe_raise_ui_cursor_4519e0_last_arg: 0,
        pe_raise_ui_wait_4012c1_count: 0,
        pe_raise_midband_refuse_sound_401096_count: 0,
        pe_raise_midband_refuse_dialog_402c4d_count: 0,
        pe_raise_midband_refuse_dialog_402c4d_a: 0,
        pe_raise_midband_refuse_dialog_402c4d_b: 0,
        pe_raise_midband_refuse_dialog_402c4d_c: 0,
        pe_raise_midband_refuse_ui_402900_count: 0,
        pe_raise_midband_refuse_ui_402900_a: 0,
        pe_raise_midband_refuse_ui_402900_b: 0,
        pe_raise_hold_blit_count: 0,
        pe_lower_hold_blit_count: 0,
        pe_process_lfsr,
        engine_fidelity: EngineFidelity::Win95,
        pe_growth_aggregate: PeGrowthAggregate::default(),
        game_clock: GameClock::default_start(),
        zone_growth_enabled: false,
        zone_growth_uses_allowlist: true,
        zone_growth_allowed_tiles: vec![false; CITY_TILES],
        trip_served_tiles: vec![false; CITY_TILES],
        bypass_growth_trip_gate: false,
        trip_bus_passengers: 0,
        trip_rail_passengers: 0,
        trip_subway_passengers: 0,
        scenario_start_clock: None,
        announced_scenario_outcome: None,
        announced_bankruptcy: false,
        edit_map_generation_seed: MacToolboxRng::DEFAULT_SEED,
        edit_map_sliders: ui::EditMapSliderValues::default(),
        edit_map_slider_drag: None,
        mode: AppMode::CityView,
        tool: EditTool::Query,
        open_tool_menu: None,
        hover_tile: None,
        zone_drag: None,
        network_drag: None,
        underground_drag: None,
        highway_drag: None,
        terrain_drag: None,
        edit_map_stretch_drag: None,
        edit_map_lower_drag: None,
        edit_map_level_drag: None,
        nature_drag: None,
        power_plant_dialog: None,
        arcology_dialog: None,
        sign_dialog: None,
        military_base_dialog: None,
        city_info_dialog: None,
            status_panel_origin: None,
            status_panel_grab: None,
            info_panel_origin: None,
            info_panel_scale: 1.0,
            info_panel_drag: None,
        founding_newspaper_pending: false,
        graph_series_visible: [true; GRAPH_HISTORY_TOTAL_SERIES],
        graph_scale: GraphScale::Year1,
        query_line: String::new(),
        query_is_inspect: false,
        query_status_pinned: false,
        sprite_cache: SpriteCache::new(),
        palette_press: None,
        menu_checks: crate::menu_model::MenuCheckState::default(),
        menubar: crate::menubar::MenubarUi::new(),
    };

    state.traffic.set_speed(GameSpeed::Llama);
    assert!(!state.advance_static_traffic_redraw_phase(0.24));
    assert_eq!(state.traffic_redraw_epoch, 7);
    assert!(!state.advance_static_traffic_redraw_phase(0.01));
    assert_eq!(state.traffic_redraw_epoch, 7);
    assert!(!state.consume_pending_traffic_redraw());
}

#[test]
fn traffic_bank_uses_logical_zoom_not_physical_pixel_scale() {
    let logical_zoom = 2.0;
    let retina_render_zoom = render_zoom(logical_zoom, 2.0);

    assert_eq!(retina_render_zoom, 4.0);
    assert_eq!(
        crate::traffic::traffic_shape_scale_band_for_dash_width(logical_zoom, 8.0),
        0
    );
    assert_eq!(
        crate::traffic::traffic_shape_scale_band_for_dash_width(retina_render_zoom, 8.0),
        crate::traffic::TRAFFIC_MEDIUM_SPRITE_OFFSET,
        "this is the bank-promotion regression if render_zoom is used as the Mac bank zoom"
    );
}

#[test]
fn pe_idle_frame_dispatch_leaves_road_traffic_state_and_mac_cadence_separate() {
    let mut city = blank_city_layout_fixture("/tmp/pe-idle-traffic.sc2", "traffic");
    city.set_building(40, 40, 0x1d);
    city.set_traffic_density(40, 40, 128);
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Llama);
    state.traffic_redraw_epoch = 23;
    state.traffic_redraw_accumulator = 0.07;
    let traffic_before = state.traffic.congestion_slice().to_vec();
    let density_before = state.city.traffic_density_slice().unwrap().to_vec();
    state.advance_frame_simulation(Duration::from_millis(400));
    assert_eq!(state.traffic.speed(), GameSpeed::Llama);
    assert_eq!(state.traffic.congestion_slice(), traffic_before);
    assert_eq!(state.city.traffic_density_slice().unwrap(), density_before);
    assert_eq!(state.traffic_redraw_epoch, 23);
    assert_eq!(state.traffic_redraw_accumulator, 0.07);

    let city = blank_city_layout_fixture("/tmp/pe-idle-mac.city", "Mac");
    let mut mac = test_state(city);
    mac.engine_fidelity = EngineFidelity::Mac68k;
    mac.city
        .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
    mac.traffic.set_speed(GameSpeed::Llama);
    let pe_before = mac.pe_idle_scheduler;
    assert_eq!(
        mac.advance_frame_simulation(Duration::from_millis(400))
            .days,
        1 // Native deadline admits one phase; elapsed frames do not create a backlog.
    );
    assert_eq!(mac.pe_idle_scheduler, pe_before);
}
