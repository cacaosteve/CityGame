use super::*;
use sha2::Digest;

#[test]
#[ignore = "requires private Win95 sampled scheduler capture"]
fn win95_sampled_paused_cycles_then_pending_calendar() {
    let capture = PathBuf::from(std::env::var_os("CITYGAME_WIN95_TIMER_CAPTURE").unwrap());
    let observations: Vec<serde_json::Value> =
        std::fs::read_to_string(capture.join("bracket-samples.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|row| row["type"] == "observation")
            .collect();
    assert!(!observations.is_empty());
    assert!(observations.iter().all(|row| {
        row["cycle_changed_during_read"] == false && row["pending"] == 1 && row["age"] == 28237
    }));
    assert_eq!(observations[0]["cycle_after"], 1);
    assert_eq!(observations.last().unwrap()["cycle_after"], 6);
    assert_eq!(observations.last().unwrap()["speed"], 2);
    assert!(
        observations[..observations.len() - 1]
            .iter()
            .all(|row| row["speed"] == 1)
    );
    let mut cycle = 1_u64;
    let mut transitions = 0;
    for row in &observations[1..] {
        let next = row["cycle_after"].as_u64().unwrap();
        if next != cycle {
            assert_eq!(next, (cycle + 1) % 8);
            cycle = next;
            transitions += 1;
        }
    }
    assert_eq!(transitions, 37);

    let read_manifest = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(capture.join(name).join("manifest.json")).unwrap())
            .unwrap()
    };
    let open = read_manifest("pre-state");
    let result = read_manifest("return-state");
    let _live_rng = LiveGrowthRng::enable();
    let mut state = test_state(City::new_blank("unused.sc2", "Empty").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state
        .load_city(&capture.join("a.sc2"), (1280, 800), 1.0)
        .unwrap();
    state.pe_process_crt =
        Win95CrtRand::new(open["crt_tls_candidates"][0]["holdrand"].as_u64().unwrap() as u32);
    state.pe_process_lfsr = Win95Lfsr::new(open["lfsr_raw"].as_u64().unwrap() as u16);
    state.pe_process_game_rand = Win95GameRand::new(open["game_lcg_raw"].as_u64().unwrap() as u32);
    state.traffic.set_speed(GameSpeed::Paused);
    // Synthetic priming to the first observed cycle and pending latch.
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(1800))
            .days,
        0
    );
    for _ in 0..transitions {
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(200))
                .days,
            0
        );
    }
    state.traffic.set_speed(GameSpeed::Turtle);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
    assert_eq!(
        u64::from(state.city.city_age_days().unwrap()),
        result["city_days"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(state.pe_process_crt.seed()),
        result["crt_tls_candidates"][0]["holdrand"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(
        u64::from(state.pe_process_lfsr.state()),
        result["lfsr_raw"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(state.pe_process_game_rand.state()),
        result["game_lcg_raw"].as_u64().unwrap()
    );
    let actors =
        crate::win95_overlay::encode_xthg_records(state.city.traffic_overlays().unwrap()).unwrap();
    assert_eq!(
        actors,
        std::fs::read(capture.join("return-state/xthg-records.bin")).unwrap()
    );
}

#[test]
#[ignore = "requires private Win95 bracketed phase-13 capture"]
fn win95_paused_open_things_sound_gate_matches_bracketed_rng() {
    let capture = PathBuf::from(std::env::var_os("CITYGAME_WIN95_PHASE13_CAPTURE").unwrap());
    let manifest = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(capture.join(name).join("manifest.json")).unwrap())
            .unwrap()
    };
    let open = manifest("opened3-state");
    let entry = manifest("entry-state");
    let mut state = test_state(City::new_blank("unused.sc2", "Empty").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state
        .load_city(&capture.join("a.sc2"), (1280, 800), 1.0)
        .unwrap();
    assert_eq!(state.city.pe_attitude_score_at(0), 26);
    assert_eq!(state.city.pe_attitude_score_at(3), 41);
    assert_eq!(
        state.city.pe_dw_tile_count[POLICE_STATION_BUILDING_ID as usize],
        9
    );
    state.pe_process_crt =
        Win95CrtRand::new(open["crt_tls_candidates"][0]["holdrand"].as_u64().unwrap() as u32);
    state.pe_process_lfsr = Win95Lfsr::new(open["lfsr_raw"].as_u64().unwrap() as u16);
    state.pe_process_game_rand = Win95GameRand::new(open["game_lcg_raw"].as_u64().unwrap() as u32);
    state.tick_pe_disaster_xthg_actors_in_slot_order();
    assert_eq!(
        u64::from(state.pe_process_crt.seed()),
        entry["crt_tls_candidates"][0]["holdrand"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(state.pe_process_lfsr.state()),
        entry["lfsr_raw"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(state.pe_process_game_rand.state()),
        entry["game_lcg_raw"].as_u64().unwrap()
    );
    let actors =
        crate::win95_overlay::encode_xthg_records(state.city.traffic_overlays().unwrap()).unwrap();
    assert_eq!(
        actors,
        std::fs::read(capture.join("entry-state/xthg-records.bin")).unwrap()
    );

    // Exercise the frame scheduler with a deliberately selected timer
    // sequence. The native timer deliveries were not captured, so this is a
    // bounded integration check, not a replay of the original event stream.
    let _live_rng = LiveGrowthRng::enable();
    let mut scheduled = test_state(City::new_blank("unused.sc2", "Empty").unwrap());
    scheduled.engine_fidelity = EngineFidelity::Win95;
    scheduled
        .load_city(&capture.join("a.sc2"), (1280, 800), 1.0)
        .unwrap();
    scheduled.pe_process_crt =
        Win95CrtRand::new(open["crt_tls_candidates"][0]["holdrand"].as_u64().unwrap() as u32);
    scheduled.pe_process_lfsr = Win95Lfsr::new(open["lfsr_raw"].as_u64().unwrap() as u16);
    scheduled.pe_process_game_rand =
        Win95GameRand::new(open["game_lcg_raw"].as_u64().unwrap() as u32);
    scheduled.traffic.set_speed(GameSpeed::Paused);
    assert_eq!(
        scheduled
            .advance_frame_simulation(Duration::from_millis(1600))
            .days,
        0
    );
    scheduled.traffic.set_speed(GameSpeed::Turtle);
    assert_eq!(
        scheduled
            .advance_frame_simulation(Duration::from_millis(200))
            .days,
        1
    );
    let result = manifest("return-state");
    assert_eq!(
        u64::from(scheduled.city.city_age_days().unwrap()),
        result["city_days"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(scheduled.pe_process_crt.seed()),
        result["crt_tls_candidates"][0]["holdrand"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(
        u64::from(scheduled.pe_process_lfsr.state()),
        result["lfsr_raw"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(scheduled.pe_process_game_rand.state()),
        result["game_lcg_raw"].as_u64().unwrap()
    );
    let actors =
        crate::win95_overlay::encode_xthg_records(scheduled.city.traffic_overlays().unwrap())
            .unwrap();
    assert_eq!(
        actors,
        std::fs::read(capture.join("return-state/xthg-records.bin")).unwrap()
    );
}

#[test]
#[ignore = "requires private Win95 same-session second-city capture"]
fn win95_native_second_city_open_preserves_calendar_globals() {
    let capture = PathBuf::from(std::env::var_os("CITYGAME_WIN95_SECOND_CITY_CAPTURE").unwrap());
    let mut state = test_state(City::new_blank("unused.sc2", "Empty").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    let check = |state: &ViewerState, boundary: &str| {
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(capture.join(boundary).join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            u64::from(state.city.city_age_days().unwrap()),
            manifest["city_days"].as_u64().unwrap(),
            "{boundary} age"
        );
        assert_eq!(
            u64::from(state.city.pe_dw_city_population()),
            manifest["city_population"].as_u64().unwrap(),
            "{boundary} population"
        );
        assert_eq!(
            u64::from(state.city.pe_w_calendar_month_mod()),
            manifest["calendar_month_mod_word"].as_u64().unwrap(),
            "{boundary} month"
        );
        assert_eq!(
            u64::from(state.city.pe_w_local_fame_base()),
            manifest["local_fame_base_word"].as_u64().unwrap(),
            "{boundary} fame"
        );
        let live_actors = std::fs::read(capture.join(boundary).join("xthg-records.bin"))
            .expect("missing native XTHG capture");
        let actors = crate::win95_overlay::encode_xthg_records(
            state
                .city
                .traffic_overlays()
                .expect("missing CityGame XTHG"),
        )
        .unwrap();
        assert_eq!(actors, live_actors, "{boundary} XTHG");
        let live_xmic = std::fs::read(capture.join(boundary).join("xmic-records.bin"))
            .expect("missing native XMIC capture");
        let mut xmic = Vec::with_capacity(1200);
        for index in 0..150 {
            let record = state
                .city
                .microsim_at(index)
                .expect("missing CityGame XMIC row");
            xmic.extend([record.tile_id, record.stat0]);
            for value in [record.stat1, record.stat2, record.stat3] {
                xmic.extend(value.to_le_bytes());
            }
        }
        assert_eq!(xmic, live_xmic, "{boundary} XMIC");
    };
    let check_planes = |state: &ViewerState, boundary: &str| {
        for (name, side) in [
            ("altm", 128),
            ("xter", 128),
            ("xbld", 128),
            ("xzon", 128),
            ("xund", 128),
            ("xtxt", 128),
            ("xbit", 128),
            ("xtrf", 64),
            ("xplt", 64),
            ("xval", 64),
            ("xcrm", 64),
            ("xplc", 32),
            ("xfir", 32),
            ("xpop", 32),
            ("xrog", 32),
        ] {
            let live = std::fs::read(capture.join(boundary).join(format!("{name}-x-major.bin")))
                .expect("missing native plane capture");
            for x in 0..side {
                for y in 0..side {
                    let scale = 128 / side;
                    let (tile_x, tile_y) = (x * scale, y * scale);
                    let actual: u16 = match name {
                        "altm" => {
                            let v = state.city.altm_at(tile_x, tile_y);
                            u16::from(v.land)
                                | (u16::from(v.water) << 5)
                                | (u16::from(v.tunnel_levels) << 10)
                        }
                        "xter" => state.city.terrain_at(tile_x, tile_y).into(),
                        "xbld" => state.city.building_at(tile_x, tile_y).into(),
                        "xzon" => state
                            .city
                            .zone_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xund" => state.city.underground_at(tile_x, tile_y).into(),
                        "xtxt" => state.city.text_overlay_at(tile_x, tile_y).unwrap().into(),
                        "xbit" => state
                            .city
                            .tile_bits_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xtrf" => state
                            .city
                            .traffic_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xplt" => state
                            .city
                            .pollution_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xval" => state.city.land_value_at(tile_x, tile_y).unwrap().into(),
                        "xcrm" => state.city.crime_density_at(tile_x, tile_y).unwrap().into(),
                        "xplc" => state
                            .city
                            .police_coverage_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xfir" => state.city.fire_coverage_at(tile_x, tile_y).unwrap().into(),
                        "xpop" => state
                            .city
                            .population_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xrog" => state.city.rate_of_growth_at(tile_x, tile_y).unwrap().into(),
                        _ => unreachable!(),
                    };
                    let index = x * side + y;
                    let expected = if name == "altm" {
                        u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                    } else {
                        u16::from(live[index])
                    };
                    assert_eq!(actual, expected, "{boundary} {name} at ({x}, {y})");
                }
            }
        }
    };
    state
        .load_city(&capture.join("a.sc2"), (1280, 800), 1.0)
        .unwrap();
    check(&state, "first-open-state");
    check_planes(&state, "first-open-state");
    state.game_clock = state.game_clock.wrapping_add_days(1);
    let date = state.game_clock;
    state.handle_simulation_day_advanced(date.year(), date.month(), date.day());
    check(&state, "first-advanced-state");
    state
        .load_city(&capture.join("b.sc2"), (1280, 800), 1.0)
        .unwrap();
    check(&state, "second-open-state");
    check_planes(&state, "second-open-state");
}

#[test]
fn win95_city_open_retains_process_calendar_words_until_next_dispatch() {
    let path = std::env::temp_dir().join(format!("pe-calendar-open-{}.sc2", std::process::id()));
    let mut incoming = City::new_blank(&path, "Incoming").unwrap();
    incoming.set_saved_date_and_day(
        CityDate {
            year: 1942,
            month: 10,
        },
        12,
    );
    incoming.save(&path).unwrap();

    let mut previous = City::new_blank("previous.sc2", "Previous").unwrap();
    previous.set_saved_date_and_day(
        CityDate {
            year: 1907,
            month: 4,
        },
        4,
    );
    let mut state = test_state(previous);
    state.engine_fidelity = EngineFidelity::Win95;
    let old = (
        state.city.pe_w_calendar_month_mod(),
        state.city.pe_w_season(),
        state.city.pe_w_local_fame_base(),
    );
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert_eq!(
        (
            state.city.pe_w_calendar_month_mod(),
            state.city.pe_w_season(),
            state.city.pe_w_local_fame_base()
        ),
        old
    );
    state.game_clock = state.game_clock.wrapping_add_days(1);
    let date = state.game_clock;
    state.handle_simulation_day_advanced(date.year(), date.month(), date.day());
    let age = state.city.city_age_days().unwrap();
    assert_eq!(
        u32::from(state.city.pe_w_calendar_month_mod()),
        (age / 25) % 12
    );
    assert_eq!(u32::from(state.city.pe_w_local_fame_base()), age / 300);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_growth_counters_round_trip_mid_month_on_both_layouts() {
    let initial = [18, 1, 2, 3, 4, 5, 6, 7];
    let mut expected = initial;
    expected[2] += crate::app::calendar::pe_growth_aggregate_weight(1) as u32;
    for filename in ["growth-roundtrip.sc2", "growth-roundtrip.city"] {
        let path = std::env::temp_dir().join(format!("{}-{filename}", std::process::id()));
        let mut city = blank_city_layout_fixture(filename, "Partial month");
        city.set_misc_u32_values(MISC_PE_GROWTH_AGGREGATE_OFFSET, &initial);
        city.save(&path).unwrap();

        let mut state = test_state(City::new_blank("unused.sc2", "Empty").unwrap());
        state.load_city(&path, (800, 600), 1.0).unwrap();
        assert_eq!(
            *state.pe_growth_aggregate.slots(),
            initial.map(|value| value as i32)
        );
        state.pe_growth_aggregate.add_normal_candidate(2, 1);
        state.refresh_live_misc_current_values();
        state.sim_calendar_day23_stats_dialogs();
        let snapshot = state.city_save_snapshot((800, 600), 1.0);
        assert_eq!(
            snapshot.misc_u32_values(MISC_PE_GROWTH_AGGREGATE_OFFSET, 8),
            expected
        );
        assert_eq!(
            state
                .city
                .misc_u32_values(MISC_PE_GROWTH_AGGREGATE_OFFSET, 8),
            initial
        );
        state.save_city(&path, (800, 600), 1.0).unwrap();

        state.load_city(&path, (800, 600), 1.0).unwrap();
        assert_eq!(
            *state.pe_growth_aggregate.slots(),
            expected.map(|value| value as i32)
        );
        state.sim_calendar_day0_budget_and_date_at(1900, 2, 0);
        assert_eq!(*state.pe_growth_aggregate.slots(), [0; 8]);
        assert_eq!(
            state
                .city_save_snapshot((800, 600), 1.0)
                .misc_u32_values(MISC_PE_GROWTH_AGGREGATE_OFFSET, 8),
            [0; 8]
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn mac_engine_win_building_census_uses_saved_values_and_survives_edits() {
    let path = std::env::temp_dir().join(format!(
        "citygame-mac-win-buildings-{}.sc2",
        std::process::id()
    ));
    let mut fixture = City::new_blank(&path, "Building census").unwrap();
    fixture.set_misc_u32_at(0x01f0, 60);
    fixture.set_misc_u32_at(0x01f0 + 0xdd * 4, 20);
    fixture.set_misc_u32_at(0x01f0 + 0xee * 4, 30);
    fixture.set_misc_u32_at(MISC_MILITARY_TILE_COUNTS_OFFSET, 5);
    fixture.save(&path).unwrap();
    let mut state =
        test_state(City::new_blank("/tmp/citygame-buildings-initial.sc2", "Initial").unwrap());
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert_eq!(state.city.mac_xbld_tile_count(0), 60);
    assert_eq!(state.city.mac_xbld_tile_count(0xdd), 20);
    assert_ne!(
        state.city.pe_dw_tile_count[0], 60,
        "PE's open-time map census stays independent"
    );
    state.city.set_building(20, 20, 0xdd);
    pe_place_tile_441f00(&mut state.city, 21, 20, 0xee);
    assert!(state.city.mac_mark_military_zone_candidate(22, 20));
    assert!(state.city.mac_mark_military_zone_candidate(22, 20));
    assert_eq!(state.city.mac_xbld_tile_count(0), 56);
    assert_eq!(state.city.mac_xbld_tile_count(0xdd), 21);
    assert_eq!(state.city.mac_xbld_tile_count(0xee), 31);
    assert_eq!(state.city.mac_zone7_census_count(0), 7);
    state.city.save(&path).unwrap();
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert_eq!(state.city.mac_xbld_tile_count(0), 56);
    assert_eq!(state.city.mac_xbld_tile_count(0xdd), 21);
    assert_eq!(state.city.mac_xbld_tile_count(0xee), 31);
    assert_eq!(state.city.mac_zone7_census_count(0), 7);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_engine_new_blank_census_starts_with_all_vacant_cells() {
    let mut city = City::new_blank("/tmp/citygame-mac-blank-census.sc2", "Blank").unwrap();
    let pe_misc_sentinel = 0x1234_5678;
    city.set_misc_u32_at(0x01f0, pe_misc_sentinel);
    city.prepare_mac_fidelity_from_saved_city();
    city.initialize_mac_fidelity_blank_xbld_census();
    assert_eq!(city.mac_xbld_tile_count(0), 16384);
    assert_eq!(city.misc_u32_at(0x01f0), Some(pe_misc_sentinel));
    let terrain = vec![0; CITY_TILES];
    let altitudes = vec![city.altm_at(0, 0); CITY_TILES];
    let mut buildings = vec![0; CITY_TILES];
    buildings[0] = 0x7a;
    city.replace_edit_map_canvas_with_buildings(terrain, altitudes, buildings);
    assert_eq!(city.mac_xbld_tile_count(0), 16383);
    assert_eq!(city.mac_xbld_tile_count(0x7a), 1);
    assert_eq!(city.misc_u32_at(0x01f0), Some(pe_misc_sentinel));
}

#[test]
fn mac_engine_win_military_census_tracks_placement_and_reload() {
    let path = std::env::temp_dir().join(format!(
        "citygame-mac-win-military-{}.sc2",
        std::process::id()
    ));
    let mut fixture = City::new_blank(&path, "Military").unwrap();
    fixture.set_zone(20, 20, 7);
    fixture.set_building(20, 20, 0xdd);
    for class in 0..16 {
        fixture.set_misc_u32_at(
            MISC_MILITARY_TILE_COUNTS_OFFSET + class * 4,
            100 + class as u32,
        );
    }
    fixture.save(&path).unwrap();
    let mut state =
        test_state(City::new_blank("/tmp/citygame-military-initial.sc2", "Initial").unwrap());
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert!(state.city.mac_cross_layout_active);
    assert_eq!(state.city.mac_zone7_census_count(1), 101);
    state.city.set_building(20, 20, 0xef);
    assert_eq!(state.city.mac_zone7_census_count(1), 100);
    assert_eq!(state.city.mac_zone7_census_count(3), 104);
    pe_place_tile_441f00(&mut state.city, 20, 20, 0xdd);
    assert_eq!(state.city.mac_zone7_census_count(1), 101);
    assert_eq!(state.city.mac_zone7_census_count(3), 103);
    state.city.set_building(20, 20, 0xef);
    state.city.save(&path).unwrap();
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert_eq!(state.city.mac_zone7_census_count(1), 100);
    assert_eq!(state.city.mac_zone7_census_count(3), 104);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_engine_uses_saved_win_xund_census_and_native_military_rule() {
    let path =
        std::env::temp_dir().join(format!("citygame-mac-win-xund-{}.sc2", std::process::id()));
    let mut fixture = City::new_blank(&path, "Cross Xund").unwrap();
    fixture.set_zone(21, 20, 7);
    fixture.set_misc_u32_at(MISC_SUBWAY_COUNT_OFFSET, 10);
    fixture.save(&path).unwrap();
    let mut state =
        test_state(City::new_blank("/tmp/citygame-xund-initial.sc2", "Initial").unwrap());
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert!(state.city.mac_cross_layout_active);
    assert_eq!(
        state.city.mac_xund_class_count(),
        10,
        "the saved count is authoritative even when the map is empty"
    );
    state.city.set_underground(20, 20, 1);
    pe_place_xund_442370(&mut state.city, 21, 20, 1);
    assert_eq!(state.city.mac_xund_class_count(), 12);
    assert_eq!(state.city.misc_u32_at(MISC_SUBWAY_COUNT_OFFSET), Some(12));
    state.city.save(&path).unwrap();
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert_eq!(state.city.mac_xund_class_count(), 12);
    state.city.set_underground(21, 20, 0);
    assert_eq!(state.city.mac_xund_class_count(), 11);
    state.city.set_misc_u32_at(MISC_SUBWAY_COUNT_OFFSET, 0xffff);
    state.city.set_underground(22, 20, 1);
    assert_eq!(state.city.mac_xund_class_count(), 0);
    state.city.set_underground(22, 20, 0);
    assert_eq!(state.city.mac_xund_class_count(), 0xffff);
    let _ = std::fs::remove_file(path);
}

#[test]
fn win95_xund_loader_discards_saved_high_word() {
    let path = std::env::temp_dir().join(format!("citygame-xund-word-{}.sc2", std::process::id()));
    let mut fixture = City::new_blank(&path, "XundWord").unwrap();
    // Keep the raw synthetic MISC dword intact while saving; loading the .sc2
    // then exercises the PE loader's low-WORD interpretation.
    fixture.kind = CityFileKind::MacCity;
    fixture.set_mac_state_misc_u32_at(MISC_SUBWAY_COUNT_OFFSET, 0x1234_002a);
    fixture.save(&path).unwrap();
    let loaded = City::load(&path).unwrap();
    assert_eq!(loaded.kind, CityFileKind::WinSc2);
    assert_eq!(loaded.pe_xund_occupancy, 0x002a);
    let _ = std::fs::remove_file(path);
}

#[test]
fn win95_xund_census_on_mac_layout_respects_military_zone_gate() {
    let path = std::env::temp_dir().join(format!("citygame-xund-mac-{}.city", std::process::id()));
    let mut city = City::new_blank(&path, "Xund").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_zone(21, 20, 7);
    city.set_mac_state_misc_u32_at(MISC_SUBWAY_COUNT_OFFSET, 10);
    city.save(&path).unwrap();
    let mut mac = City::load(&path).unwrap();
    mac.prepare_pe_fidelity_from_saved_city();
    assert_eq!(mac.pe_xund_occupancy, 10);
    let mut win = mac.clone();
    win.kind = CityFileKind::WinSc2;
    win.pe_cross_layout_active = false;
    for city in [&mut mac, &mut win] {
        pe_place_xund_442370(city, 20, 20, 1);
        pe_place_xund_442370(city, 21, 20, 1);
        pe_place_xund_442370(city, 21, 20, 0);
    }
    assert_eq!(mac.pe_xund_occupancy, 11);
    assert_eq!(mac.pe_xund_occupancy, win.pe_xund_occupancy);
    assert_eq!(
        mac.mac_state_misc_u32_at(MISC_SUBWAY_COUNT_OFFSET),
        Some(11)
    );
    mac.save(&path).unwrap();
    let mut loaded = City::load(&path).unwrap();
    loaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded.pe_xund_occupancy, 11);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_xund_census_remains_native_in_shared_underground_helpers() {
    let mut city = City::new_blank("/tmp/citygame-native-xund.city", "MacXund").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_zone(21, 20, 7);
    city.set_mac_state_misc_u32_at(MISC_SUBWAY_COUNT_OFFSET, 10);
    pe_place_xund_442370(&mut city, 21, 20, 1);
    assert_eq!(
        city.mac_state_misc_u32_at(MISC_SUBWAY_COUNT_OFFSET),
        Some(11)
    );
}

#[test]
fn win95_military_census_on_mac_layout_survives_placement_and_base_stamps() {
    let path =
        std::env::temp_dir().join(format!("citygame-military-mac-{}.city", std::process::id()));
    let mut city = City::new_blank(&path, "Military").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_zone(20, 20, 7);
    city.set_building(20, 20, 0xdd);
    for index in 0..16 {
        city.set_mac_state_misc_u32_at(
            MISC_MILITARY_TILE_COUNTS_OFFSET + index * 4,
            100 + index as u32,
        );
    }
    city.set_mac_state_misc_u32_at(0x01f0, 200);
    city.save(&path).unwrap();
    let mut mac = City::load(&path).unwrap();
    assert_eq!(mac.pe_military_tile_count, [0; 16]);
    mac.prepare_pe_fidelity_from_saved_city();
    for index in 0..16 {
        assert_eq!(mac.pe_military_tile_count[index], 100 + index as u16);
    }
    let mut win = mac.clone();
    win.kind = CityFileKind::WinSc2;
    for city in [&mut mac, &mut win] {
        pe_place_tile_441f00(city, 20, 20, 0xef);
        pe_stamp_military_zone_tile_4144d4(city, 30, 30, 7);
        stamp_pe_military_base_fallback_plot(city, 40, 40);
    }
    assert_eq!(mac.pe_military_tile_count, win.pe_military_tile_count);
    assert_eq!(mac.pe_military_tile_count[0], 110);
    assert_eq!(mac.pe_military_tile_count[1], 100);
    assert_eq!(mac.pe_military_tile_count[3], 104);
    assert_eq!(mac.mac_state_misc_u32_at(0x01f0), Some(190));
    for index in 0..16 {
        assert_eq!(
            mac.mac_state_misc_u32_at(MISC_MILITARY_TILE_COUNTS_OFFSET + index * 4)
                .unwrap() as u16,
            mac.pe_military_tile_count[index]
        );
    }
    mac.save(&path).unwrap();
    let mut loaded = City::load(&path).unwrap();
    loaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded.pe_military_tile_count, win.pe_military_tile_count);
    assert_eq!(loaded.mac_state_misc_u32_at(0x01f0), Some(190));
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_sound_and_music_options_use_native_state_only_for_mac_city_fidelity() {
    let path = PathBuf::from("/tmp/sc2000-mac-sound-music-state");
    let mut city = blank_city_layout_fixture(&path, "MacAudioOptions");
    city.set_mac_state_misc_u32_at(AUTO_GOTO_MISC_OFFSET, 0xAABB_CC01);
    city.set_mac_state_misc_u32_at(SOUND_EFFECTS_MISC_OFFSET, 0x1122_3300);
    city.set_mac_state_misc_u32_at(MUSIC_MISC_OFFSET, 0x4455_6601);

    assert!(!sound_effects_enabled_for_engine(
        &city,
        EngineFidelity::Mac68k
    ));
    assert!(music_enabled_for_engine(&city, EngineFidelity::Mac68k));
    assert!(sound_effects_enabled_for_engine(
        &city,
        EngineFidelity::Win95
    ));
    assert!(music_enabled_for_engine(&city, EngineFidelity::Win95));

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sound_effects_enabled =
        sound_effects_enabled_for_engine(&state.city, state.engine_fidelity);
    state.music_enabled = music_enabled_for_engine(&state.city, state.engine_fidelity);
    state.toggle_sound_effects();
    state.toggle_music();
    assert!(state.sound_effects_enabled);
    assert!(!state.music_enabled);
    assert_eq!(
        state.city.mac_state_misc_u32_at(SOUND_EFFECTS_MISC_OFFSET),
        Some(1),
        "the native Sound Effects byte is zero-extended when enabled"
    );
    assert_eq!(
        state.city.mac_state_misc_u32_at(MUSIC_MISC_OFFSET),
        Some(0),
        "the native Music byte is zero-extended when disabled"
    );
    assert_eq!(
        state.city.mac_state_misc_u32_at(AUTO_GOTO_MISC_OFFSET),
        Some(0xAABB_CC01),
        "the adjacent Auto-Goto dword is not changed"
    );

    state.city.save(&path).expect("save native audio options");
    let loaded = City::load(&path).expect("reload native audio options");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert!(sound_effects_enabled_for_engine(
        &loaded,
        EngineFidelity::Mac68k
    ));
    assert!(!music_enabled_for_engine(&loaded, EngineFidelity::Mac68k));
    assert_eq!(
        loaded.mac_state_misc_u32_at(AUTO_GOTO_MISC_OFFSET),
        Some(0xAABB_CC01)
    );

    let mut pe_city = City::new_blank("/tmp/sc2000-mac-engine-pe-audio.sc2", "PE layout")
        .expect("blank PE-layout city");
    pe_city.set_misc_u32_at(SOUND_EFFECTS_MISC_OFFSET, CITY_OPTION_DISABLED_VALUE);
    pe_city.set_misc_u32_at(MUSIC_MISC_OFFSET, CITY_OPTION_DISABLED_VALUE);
    assert!(sound_effects_enabled_for_engine(
        &pe_city,
        EngineFidelity::Mac68k
    ));
    assert!(music_enabled_for_engine(&pe_city, EngineFidelity::Mac68k));
    assert!(!set_sound_effects_enabled_for_engine(
        &mut pe_city,
        EngineFidelity::Mac68k,
        false
    ));
    assert!(!set_music_enabled_for_engine(
        &mut pe_city,
        EngineFidelity::Mac68k,
        false
    ));
    assert_eq!(
        pe_city.misc_u32_at(SOUND_EFFECTS_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        pe_city.misc_u32_at(MUSIC_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    std::fs::remove_file(path).expect("remove round-trip fixture");
}

#[test]
fn win95_ui_extensions_preserve_mac_state_in_mac_layout_city() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-win95-ui-state.sc2", "MacUiState").expect("blank city");
    city.kind = CityFileKind::MacCity;

    let owned_slots = [
        (AUTO_GOTO_MISC_OFFSET, 0x1010_0001),
        (SOUND_EFFECTS_MISC_OFFSET, 0x2020_0002),
        (MUSIC_MISC_OFFSET, 0x3030_0003),
        (MISC_NEWSPAPER_SUBSCRIPTION_OFFSET, 0x5050_0005),
        (MISC_NEWSPAPER_EXTRA_OFFSET, 0x6060_0006),
        (MISC_NEWSPAPER_CHOICE_OFFSET, 0x7070_0007),
        (EDIT_MAP_COAST_MISC_OFFSET, 0x9090_0009),
        (EDIT_MAP_RIVER_MISC_OFFSET, 0xa0a0_000a),
        (EDIT_MAP_RIVER_MISC_OFFSET, 0xa0a0_000a),
    ];
    for (offset, value) in owned_slots {
        city.set_mac_state_misc_u32_at(offset, value);
    }
    city.set_mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, 0x8080_0008);
    // Explicit Win95 selection interprets the raw Mac slot once, then keeps
    // the divergent rewards mask in CGEX while the Mac command state stays put.
    city.prepare_pe_fidelity_from_saved_city();
    // The native Mac option is stored in the low byte; upper bytes are not
    // part of its Boolean value.
    city.set_mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET, 0x4040_0000);
    let mut state = test_state(city);
    assert_eq!(state.engine_fidelity, EngineFidelity::Win95);
    assert!(!state.no_disasters_enabled);

    ensure_newspaper_list_misc_defaults_before_extension(&mut state.city);
    ensure_default_city_options_for_engine(&mut state.city, EngineFidelity::Win95);

    // These PE controls may change for the current viewer session, but their
    // Win95 MISC persistence must not overwrite the Mac CODE 4 state vector.
    state.toggle_auto_goto();
    state.toggle_sound_effects();
    state.toggle_music();
    state.toggle_no_disasters();
    assert!(!state.auto_goto_enabled);
    assert!(!state.sound_effects_enabled);
    assert!(!state.music_enabled);
    assert!(state.no_disasters_enabled);
    assert_eq!(
        state.city.mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    state.city.set_rewards_available_mask(0x1f);
    assert_eq!(state.city.rewards_available_mask(), 0x1f);
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
        Some(0x8080_0008)
    );
    assert!(!state.edit_map_option_enabled(EDIT_MAP_COAST_MISC_OFFSET));
    assert!(!state.edit_map_option_enabled(EDIT_MAP_RIVER_MISC_OFFSET));
    state.toggle_edit_map_option(EDIT_MAP_COAST_MISC_OFFSET);
    state.toggle_edit_map_option(EDIT_MAP_RIVER_MISC_OFFSET);

    assert!(!set_city_newspaper_subscription_enabled(
        &mut state.city,
        true
    ));
    assert!(!set_city_newspaper_extra_enabled(&mut state.city, false));
    assert!(!set_city_newspaper_choice(&mut state.city, 5));
    assert_eq!(newspaper_choice_for_city(&state.city), 0);
    state.city.queue_pe_newspaper_story(3, 0);
    assert!(!state.city.take_pe_newspaper_extra_popup_pending());
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Subscription));
    assert_eq!(state.city_info_dialog, None);

    state.city.set_pe_disaster_end_newspaper_pending();
    state.maybe_open_pe_disaster_end_newspaper();
    assert_eq!(state.city_info_dialog, None);
    assert!(!state.city.take_pe_disaster_end_newspaper_pending());

    for (offset, expected) in owned_slots {
        assert_eq!(
            state.city.mac_state_misc_u32_at(offset),
            Some(expected),
            "Win95 UI extension changed Mac-owned MISC slot {offset:#x}"
        );
    }
    assert!(
        (0..NEWSPAPER_LIST_MISC_DWORDS).all(|index| state
            .city
            .mac_state_misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET + index * 4)
            == Some(0)),
        "Win95 newspaper initialization must not populate Mac-owned paper-list bytes"
    );
    assert!(
        (0..NEWSPAPER_STORY_LIST_MISC_DWORDS).all(|index| state
            .city
            .mac_state_misc_u32_at(MISC_NEWSPAPER_STORY_LIST_OFFSET + index * 4)
            == Some(0)),
        "Win95 newspaper initialization must not populate Mac-owned story-list bytes"
    );
}

#[test]
fn original_default_on_options_read_and_toggle_saved_misc_flags() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(auto_goto_enabled_for_city(&city));
    assert!(sound_effects_enabled_for_city(&city));
    assert!(music_enabled_for_city(&city));

    assert!(set_city_auto_goto_enabled(&mut city, false));
    assert!(set_city_sound_effects_enabled(&mut city, false));
    assert!(set_city_music_enabled(&mut city, false));
    assert!(!auto_goto_enabled_for_city(&city));
    assert!(!sound_effects_enabled_for_city(&city));
    assert!(!music_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(AUTO_GOTO_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(SOUND_EFFECTS_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(MUSIC_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );

    assert!(set_city_auto_goto_enabled(&mut city, true));
    assert!(set_city_sound_effects_enabled(&mut city, true));
    assert!(set_city_music_enabled(&mut city, true));
    assert_eq!(
        city.misc_u32_at(AUTO_GOTO_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(SOUND_EFFECTS_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(MUSIC_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
}

#[test]
fn city_state_toggles_original_option_flags() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert!(state.auto_goto_enabled);
    assert!(state.sound_effects_enabled);
    assert!(state.music_enabled);

    state.toggle_auto_goto();
    state.toggle_sound_effects();
    state.toggle_music();
    assert!(!state.auto_goto_enabled);
    assert!(!state.sound_effects_enabled);
    assert!(!state.music_enabled);
    assert_eq!(
        state.city.misc_u32_at(AUTO_GOTO_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        state.city.misc_u32_at(SOUND_EFFECTS_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        state.city.misc_u32_at(MUSIC_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );

    state.toggle_auto_goto();
    state.toggle_sound_effects();
    state.toggle_music();
    assert!(state.auto_goto_enabled);
    assert!(state.sound_effects_enabled);
    assert!(state.music_enabled);
    assert_eq!(
        state.city.misc_u32_at(AUTO_GOTO_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert_eq!(
        state.city.misc_u32_at(SOUND_EFFECTS_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert_eq!(
        state.city.misc_u32_at(MUSIC_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
}

#[test]
fn mac_fidelity_skips_win95_misc_defaults_that_overlap_native_state() {
    let mut city = City::new_blank("/tmp/sc2000-mac-misc-defaults.sc2", "mac").expect("blank city");
    for offset in [
        AUTO_GOTO_MISC_OFFSET,
        SOUND_EFFECTS_MISC_OFFSET,
        MUSIC_MISC_OFFSET,
        MISC_NEWSPAPER_SUBSCRIPTION_OFFSET,
        MISC_NEWSPAPER_EXTRA_OFFSET,
        MISC_NEWSPAPER_CHOICE_OFFSET,
    ] {
        city.set_misc_u32_at(offset, 0xA000_0000 | offset as u32);
    }
    city.set_misc_u32_at(
        NO_DISASTERS_MISC_OFFSET,
        0xA000_0001 | NO_DISASTERS_MISC_OFFSET as u32,
    );
    let original_option_slots: Vec<_> = [
        AUTO_GOTO_MISC_OFFSET,
        SOUND_EFFECTS_MISC_OFFSET,
        MUSIC_MISC_OFFSET,
        MISC_NEWSPAPER_SUBSCRIPTION_OFFSET,
        MISC_NEWSPAPER_EXTRA_OFFSET,
        MISC_NEWSPAPER_CHOICE_OFFSET,
    ]
    .into_iter()
    .map(|offset| (offset, city.misc_u32_at(offset)))
    .collect();
    let original_paper_slots =
        city.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS);
    let original_story_slots = city.misc_u32_values(
        MISC_NEWSPAPER_STORY_LIST_OFFSET,
        NEWSPAPER_STORY_LIST_MISC_DWORDS,
    );

    ensure_default_city_options_for_engine(&mut city, EngineFidelity::Mac68k);
    assert_eq!(
        city.misc_u32_at(NO_DISASTERS_MISC_OFFSET),
        Some(0xA000_0001 | NO_DISASTERS_MISC_OFFSET as u32),
        "the shared option remains unchanged until the user toggles it"
    );

    for &(offset, value) in &original_option_slots {
        assert_eq!(city.misc_u32_at(offset), value, "MISC {offset:#x}");
    }
    assert_eq!(
        city.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS),
        original_paper_slots,
        "Win95's paper permutation must not replace Mac CODE 4 state slots"
    );
    assert_eq!(
        city.misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            NEWSPAPER_STORY_LIST_MISC_DWORDS
        ),
        original_story_slots,
        "Win95's story table must not replace Mac CODE 4 state slots"
    );

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.toggle_auto_goto();
    state.toggle_sound_effects();
    state.toggle_music();
    state.toggle_no_disasters();
    state.toggle_newspaper_subscription();
    state.toggle_newspaper_extra();
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Extra));
    assert!(!state.no_disasters_enabled);
    assert_eq!(state.city.misc_u32_at(NO_DISASTERS_MISC_OFFSET), Some(0));
    let expected_after_toggle = |offset, value| {
        if matches!(
            offset,
            MISC_NEWSPAPER_EXTRA_OFFSET | MISC_NEWSPAPER_SUBSCRIPTION_OFFSET
        ) {
            Some(0)
        } else {
            value
        }
    };
    for &(offset, value) in &original_option_slots {
        assert_eq!(
            state.city.misc_u32_at(offset),
            expected_after_toggle(offset, value),
            "MISC {offset:#x}"
        );
    }
    assert!(state.city_info_dialog.is_none());

    let path = PathBuf::from("/tmp/sc2000-mac-misc-defaults-roundtrip.sc2");
    state.city.save(&path).expect("save preserved Mac MISC");
    let loaded = City::load(path).expect("reload preserved Mac MISC");
    assert_eq!(loaded.misc_u32_at(NO_DISASTERS_MISC_OFFSET), Some(0));
    for &(offset, value) in &original_option_slots {
        assert_eq!(
            loaded.misc_u32_at(offset),
            expected_after_toggle(offset, value),
            "MISC {offset:#x}"
        );
    }
    assert_eq!(
        loaded.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS),
        original_paper_slots
    );
    assert_eq!(
        loaded.misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            NEWSPAPER_STORY_LIST_MISC_DWORDS
        ),
        original_story_slots
    );
}

#[test]
fn saved_city_underground_tile_ids_are_classified() {
    let files = available_city_files();
    if files.is_empty() {
        return;
    }

    let mut observed = BTreeMap::<u8, usize>::new();
    let mut unclassified = BTreeMap::<u8, usize>::new();
    for path in files {
        let Ok(city) = City::load(&path) else {
            continue;
        };
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let tile = city.underground_at(x, y);
                if tile == 0 {
                    continue;
                }
                *observed.entry(tile).or_default() += 1;
                let source = underground_tile_source(&city, x, y, tile);
                if underground_tile_sprite_id(UndergroundRenderTile {
                    tile,
                    source,
                    watered: city.tile_bits_at(x, y).is_some_and(|bits| bits.watered()),
                })
                .is_none()
                {
                    *unclassified.entry(tile).or_default() += 1;
                }
            }
        }
    }

    assert!(
        !observed.is_empty(),
        "expected nonzero XUND tiles in fixtures"
    );
    assert!(
        unclassified.is_empty(),
        "unclassified XUND ids: {unclassified:?}; observed: {observed:?}"
    );
}

#[test]
fn pe_selected_height_is_published_by_win95_generation_for_both_layouts() {
    for extension in ["sc2", "city"] {
        for (water, ocean, river, expected) in [
            (0, false, false, 0),
            (0, true, false, 4),
            (0, false, true, 4),
            (47, false, false, 6),
        ] {
            let mut city = blank_city_layout_fixture(
                format!("/tmp/pe-selected-generation.{extension}"),
                "SelectedGeneration",
            );
            city.set_mac_state_misc_u32_at(0x0e40, 0xabcd_ffff);
            let mut crt = Win95CrtRand::new(1);
            let mut game_rng = Win95GameRand::new(1);
            terrain_generation::generate_win95(
                &mut city,
                ui::EditMapSliderValues {
                    hills: 0,
                    water,
                    trees: 0,
                },
                ocean,
                river,
                &mut crt,
                &mut game_rng,
            );
            assert_eq!(city.mac_state_misc_u32_at(0x0e40), Some(expected));
            assert_eq!(
                pe_selected_water_height_for_fun_00447020(&city),
                expected as i16
            );
        }
    }
}

#[test]
fn pe_447490_citymode_controls_followup_tile_clear_without_touching_xtrf() {
    for (city_mode, expected_building) in [
        (CITY_MODE_EDIT_MAP as u16, 6),
        (CITY_MODE_GAMEPLAY as u16, 0),
    ] {
        let mut city =
            City::new_blank("/tmp/sc2000-447490-citymode.sc2", "FunCityMode").expect("blank");
        for y in 18..23 {
            for x in 18..23 {
                city.set_altitude(x, y, 8, 0);
                city.set_terrain(x, y, 0);
            }
        }
        city.pe_w_city_mode = city_mode;
        city.set_building(20, 20, 6);
        city.set_text_overlay(20, 20, 0x4b);
        city.set_traffic_density(20, 20, 0x37);

        pe_terrain_neighbor_fun_00447020_447490(&mut city, None, 20, 20);

        assert_eq!(city.building_at(20, 20), expected_building);
        assert_eq!(city.text_overlay_at(20, 20), Some(0x4b));
        assert_eq!(city.traffic_density_at(20, 20), Some(0x37));
    }
}

#[test]
fn pe_city_mode_misc_matches_win95_wcitymode() {
    assert_eq!(
        pe_city_mode_misc_value(AppMode::EditNewMap, false),
        CITY_MODE_EDIT_MAP
    );
    assert_eq!(
        pe_city_mode_misc_value(AppMode::EditNewMap, true),
        CITY_MODE_EDIT_MAP
    );
    assert_eq!(
        pe_city_mode_misc_value(AppMode::CityView, false),
        CITY_MODE_GAMEPLAY
    );
    assert_eq!(
        pe_city_mode_misc_value(AppMode::CityView, true),
        CITY_MODE_DISASTER
    );
}

#[test]
fn win95_mode_on_mac_city_uses_gameplay_cache_without_overwriting_mac_run_state() {
    let path = std::env::temp_dir().join(format!(
        "citygame-pe-mode-mac-layout-{}.city",
        std::process::id()
    ));
    let mut fixture = City::new_blank(&path, "PE mode on Mac layout").unwrap();
    fixture.kind = CityFileKind::MacCity;
    fixture.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, 2);
    fixture.save(&path).unwrap();

    let initial = City::new_blank("/tmp/citygame-pe-mode-initial.sc2", "Initial").unwrap();
    let mut state = test_state(initial);
    state.engine_fidelity = EngineFidelity::Win95;
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert_eq!(state.city.pe_w_city_mode, CITY_MODE_GAMEPLAY as u16);
    assert_eq!(
        state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(2),
        "PE runtime mode must not replace Mac run-state"
    );

    state.city.set_building(20, 20, 6);
    pe_fun_00447020_mode1_destroy_and_city_clear_447085(&mut state.city, None, 20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    state.city.save(&path).unwrap();
    let reloaded = City::load(&path).unwrap();
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(2)
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_global_sea_level_uses_saved_word_and_edition_surface_rules() {
    // land, XTER, XBLD, XBIT, resulting XTER, water height, XBIT
    let cases: [(u8, u8, u8, u8, u8, u8, u8); 8] = [
        (4, 0x03, 0x70, 0xa1, 0x23, 5, 0xa5),
        (3, 0x03, 0x70, 0xa1, 0x13, 5, 0xa5),
        (4, 0x00, 5, 0xa1, 0x10, 5, 0xa5),
        (4, 0x23, 6, 0xa5, 0x23, 5, 0xa5),
        (4, 0x35, 0x70, 0xa1, 0x10, 5, 0xa5),
        (7, 0x23, 0x70, 0xa5, 0x03, 5, 0xa1),
        (7, 0x35, 0x70, 0xa5, 0x35, 9, 0xa1),
        (7, 0x13, 0x70, 0xa1, 0x13, 9, 0xa1),
    ];
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
            let mut city = City::new_blank("/tmp/native-sea-level.sc2", "NativeSeaLevel").unwrap();
            city.kind = kind;
            city.set_mac_state_misc_u32_at(0x0e40, 0x1234_0004);
            city.set_altitude(0, 0, 1, 20); // Conflicting visible waterline must not own the control.
            city.set_terrain(0, 0, 0x10);
            city.set_label(0x21, "Keep");
            for (i, &(land, terrain, building, bits, _, _, _)) in cases.iter().enumerate() {
                let x = 20 + i;
                city.set_altm_word(x, 20, (0x21 << 10) | (9 << 5) | u16::from(land));
                city.set_terrain(x, 20, terrain);
                city.set_building(x, 20, building);
                city.set_tile_bits(x, 20, TileBits::new(bits));
                city.set_zone(x, 20, 0x82);
                city.set_underground(x, 20, 0x23);
                city.set_text_overlay(x, 20, 0x21);
                city.set_traffic_density(x, 20, 0x63);
            }
            city.pe_rebuild_dw_tile_count_434f80();
            let count = city.pe_dw_tile_count[0x70];
            let mut state = test_state(city);
            state.mode = AppMode::EditNewMap;
            state.engine_fidelity = engine;
            let crt = state.pe_process_crt.seed();
            let qd = state.mac_process_rng.seed();
            let funds = state.city.city_funds();

            assert_eq!(
                state.adjust_edit_map_sea_level(1),
                8,
                "{kind:?}, {engine:?}"
            );

            assert_eq!(state.city.mac_selected_water_height(), Some(5));
            assert_eq!(state.city.misc_water_table_level(), Some(5));
            assert_eq!(state.city.mac_state_misc_u32_at(0x0e40), Some(5));
            for (i, &(land, _, building, _, terrain, water, bits)) in cases.iter().enumerate() {
                let x = 20 + i;
                let cleared =
                    i < 5 && (engine == EngineFidelity::Mac68k || (6..=13).contains(&building));
                assert_eq!(
                    state.city.building_at(x, 20),
                    if cleared { 0 } else { building }
                );
                assert_eq!(state.city.altitude_at(x, 20), land);
                assert_eq!(state.city.water_height_at(x, 20), water);
                assert_eq!(state.city.altm_at(x, 20).tunnel_levels, 0x21);
                assert_eq!(state.city.terrain_at(x, 20), terrain);
                assert_eq!(state.city.tile_bits_at(x, 20), Some(TileBits::new(bits)));
                assert_eq!(state.city.zone_at(x, 20).unwrap().raw_bits(), 0x82);
                assert_eq!(state.city.underground_at(x, 20), 0x23);
                assert_eq!(state.city.text_overlay_at(x, 20), Some(0x21));
                assert_eq!(state.city.traffic_density_at(x, 20), Some(0x63));
            }
            assert_eq!(
                state.city.pe_dw_tile_count[0x70],
                count
                    - if engine == EngineFidelity::Mac68k {
                        3
                    } else {
                        0
                    }
            );
            assert_eq!(state.city.label_at(0x21), Some("Keep"));
            assert_eq!(state.city.city_funds(), funds);
            assert_eq!(state.mac_process_rng.seed(), qd);
            assert_eq!(state.pe_process_crt.seed(), crt);
        }
    }
}

#[test]
fn simulation_day22_increments_pe_city_garbage() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank");
    city.set_building(10, 10, 0x70);
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_building(x, y, 0xB2);
    }
    for (x, y) in building_footprint_from_origin(30, 20, 2).unwrap() {
        city.set_building(x, y, 0x9E);
    }
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    // Growth-disabled so day 22 keeps the census stand-in (no day-21 publish).
    state.zone_growth_enabled = false;
    let population = live_population_stats_for_city(&state.city);
    let increment = pe_zone_pops_scaled_population(population);
    assert_eq!(increment, 12 + 9 * 64 + 4 * 36);

    let _ = state.sim_calendar_day22_scenario(population);
    assert_eq!(state.city.misc_u32_at(MISC_GARBAGE_OFFSET), Some(increment));
    assert_eq!(state.city.pe_w_city_garbage, increment);
    assert_eq!(state.city.pe_dw_city_population(), increment);
    assert_eq!(
        state.city.pe_eq_leftover_deaths(),
        increment,
        "first day 22 parks the whole zone-pop snapshot as immigration leftover"
    );
    assert_eq!(state.city.pe_eq_leftover_births(), 0);

    let _ = state.sim_calendar_day22_scenario(population);
    assert_eq!(
        state.city.misc_u32_at(MISC_GARBAGE_OFFSET),
        Some(increment.saturating_mul(2)),
        "PE accumulates wCityGarbage every month"
    );
    assert_eq!(state.city.pe_w_city_garbage, increment.saturating_mul(2));
    assert_eq!(state.city.pe_eq_leftover_deaths(), 0);
    assert_eq!(state.city.pe_dw_city_population(), increment);
}

#[test]
fn day23_skips_misc_le_eq_overwrite_when_pe_eq_averages_ready() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank");
    city.set_pe_life_expectancy(70);
    city.test_seed_pe_age_cohorts_from_pop(20_000);
    city.advance_pe_eq_le_month(
        20_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    assert!(city.pe_eq_averages_ready());
    // Pin known PE workforce globals so coverage stand-ins would be visible if day 23 clobbers.
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 77);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 88);

    let mut state = test_state(city);
    state.sim_calendar_day23_stats_dialogs();
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET),
        Some(77),
        "day 23 must not replace workforce LE with coverage stand-in"
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET),
        Some(88),
        "day 23 must not replace workforce EQ with coverage stand-in"
    );
}

#[test]
fn structure_microsim_init_seeds_capacity_stats() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1955,
        month: 3,
    });

    let coal = city
        .provision_microsim_for_building(207)
        .expect("coal microsim");
    let coal_index = usize::from(coal - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(
        city.microsim_at(coal_index),
        Some(crate::city::MicrosimRecord {
            tile_id: 207,
            stat0: 0,
            stat1: 200,
            stat2: 0,
            stat3: 0,
        })
    );

    let arco = city
        .provision_microsim_for_building(251)
        .expect("arco microsim");
    let arco_index = usize::from(arco - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(
        city.microsim_at(arco_index),
        Some(crate::city::MicrosimRecord {
            tile_id: 251,
            stat0: 5,
            stat1: 55,
            stat2: 0,
            stat3: 1955,
        })
    );

    let school = city
        .provision_microsim_for_building(214)
        .expect("school microsim");
    let school_index = usize::from(school - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(
        city.microsim_at(school_index).map(|r| (r.tile_id, r.stat0)),
        Some((214, 6))
    );

    let police = city
        .provision_microsim_for_building(210)
        .expect("police microsim");
    let police_index = usize::from(police - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(
        city.microsim_at(police_index)
            .map(|r| (r.tile_id, r.stat0, r.stat1)),
        Some((210, 0, 0)),
        "InitializeMicrosim zeros police; FUN_0044f7d0 is 0 at leftover 0"
    );

    city.refresh_structure_microsim_yearly(0, 100, 100, 100, 100, 100);
    assert_eq!(
        city.microsim_at(coal_index).map(|r| (r.stat0, r.stat1)),
        Some((1, 200)),
        "January ages power plants"
    );
    assert_eq!(
        city.microsim_at(arco_index)
            .map(|r| (r.stat1, r.stat2, r.stat3)),
        Some((55, 2, 1955)),
        "PE arco: low-pop occupancy is LFSR jitter only"
    );
}

#[test]
fn loaded_zero_eq_le_summary_fields_are_authoritative() {
    let path = "/tmp/sc2000-eq-zero-summary.sc2";
    let mut city = City::new_blank(path, "ZeroSummary").expect("blank");
    // Force a save/load boundary while leaving all three summary slots at zero.
    city.set_misc_u32_at(MISC_CITY_FUNDS_OFFSET, 0);
    assert!(!city.pe_eq_averages_ready());
    city.save(path).expect("save");

    let loaded = City::load(path).expect("load");
    assert!(loaded.pe_eq_averages_ready());
    assert_eq!(loaded.pe_eq_workforce_percent(), 0);
    assert_eq!(loaded.pe_eq_life_average(), 0);
    assert_eq!(loaded.pe_eq_education_average(), 0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_opinion_poll_weights_match_win95_layout() {
    let (weights, total) = crate::city::pe_opinion_poll_weights(
        10,  // *0x4cc480
        20,  // *0x4cc484
        30,  // *0x4cc48c
        40,  // *0x4cc488 seed
        100, // residential tax factor → ×3
        12,  // unemployment
        25,  // crime ≤100 → 100-25
        10,  // pollution ≤70 → 70-10
    );
    assert_eq!(weights, [10, 20, 30, 300, 12, 75, 60]);
    assert_eq!(total, 40 + 0x32 + 10 + 20 + 30 + 300 + 12 + 75 + 60);

    let (wrapped_words, wrapped_total) = crate::city::pe_opinion_poll_weights(
        0x1_0001, 0x1_0002, 0x1_0003, 0xffff, 0x1_5555, 0x1_0004, 101, 71,
    );
    assert_eq!(wrapped_words, [1, 2, 3, 0xffff, 4, 0, 0]);
    assert_eq!(
        wrapped_total, 58,
        "the PE local total is accumulated as a WORD"
    );

    let (high_crime, _) = crate::city::pe_opinion_poll_weights(0, 0, 0, 0, 0, 0, 101, 0);
    assert_eq!(high_crime[5], 0, "crime > 100 zeroes the safety weight");
    let (high_pollution, _) = crate::city::pe_opinion_poll_weights(0, 0, 0, 0, 0, 0, 0, 71);
    assert_eq!(
        high_pollution[6], 0,
        "pollution > 70 zeroes the pollution weight"
    );
}

#[test]
fn pe_rebuild_w_city_developed_tiles_stores_4c93b4_like_46ab56() {
    // PE clears `0x4c93b4` @ `0x46ab58` then increments qualify tiles @ `0x46ac82`.
    let mut city = City::new_blank("/tmp/sc2000-dev-rebuild.sc2", "DevRebuild").expect("blank");
    city.set_zone(5, 5, ZONE_COMMERCIAL_LIGHT);
    city.set_building(7, 5, 0x1d);
    city.set_building(8, 5, 0x1c); // below min — not developed alone
    assert_eq!(pe_rebuild_w_city_developed_tiles_46ab56(&mut city), 2);
    assert_eq!(city.pe_w_city_developed_tiles, 2);
}

#[test]
fn weather_state_for_city_prefers_pe_w_season_bss_4cb3e8() {
    // PE weather @ `0x468a28` reads season BSS; age publish fills it.
    let mut city = City::new_blank("/tmp/sc2000-weather-season.sc2", "WxSeason").expect("blank");
    city.pe_dw_city_age = 25 * 14 + 3; // month_mod 2 → season 1
    city.pe_publish_graph_calendar_words_from_city_age_41354e();
    let state = weather_state_for_city(&city, 1); // calendar Jan would be season 0
    assert_eq!(city.pe_w_season(), 1);
    assert_eq!(state.season, 1);
}

#[test]
fn win95_weather_cache_and_write_respect_mac_low_byte_layout() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-win95-weather-mac-misc-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "MacWeatherBridge").expect("blank");
    city.kind = CityFileKind::MacCity;
    let saved = [
        (MISC_HEAT_OFFSET, 0x1234_560a),
        (MISC_WIND_OFFSET, 0xabcd_ef55),
        (MISC_HUMID_OFFSET, 0xdead_be29),
        (MISC_WEATHER_TREND_OFFSET, 0x9876_0004),
    ];
    for (offset, value) in saved {
        city.set_mac_state_misc_u32_at(offset, value);
    }
    city.save(&path).expect("save Mac-layout weather state");

    let mut loaded = City::load(&path).expect("load Mac-layout weather state");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.pe_b_weather_heat, 10);
    assert_eq!(loaded.pe_b_weather_wind, 0x55);
    assert_eq!(loaded.pe_b_weather_humid, 0x29);
    assert_eq!(loaded.pe_b_weather_trend, 4);
    assert_eq!(loaded.misc_humidity(), Some(0x29));
    assert_eq!(
        loaded.mac_weather_humidity(),
        Some(0x29),
        "native Mac readers keep using the saved low byte"
    );

    let next = WeatherState {
        heat: 31,
        wind: 42,
        humidity: 53,
        trend: WEATHER_TREND_SNOW,
        season: 2,
    };
    persist_weather_state_to_city(&mut loaded, next);
    for ((offset, before), value) in
        saved
            .into_iter()
            .zip([31, 42, 53, u32::from(WEATHER_TREND_SNOW)])
    {
        assert_eq!(
            loaded.misc_u32_at(offset),
            Some((before & !0xff) | value),
            "Win95 weather write must retain Mac MISC upper bytes at {offset:#x}"
        );
    }
    assert_eq!(loaded.pe_b_weather_heat, 31);
    assert_eq!(loaded.pe_b_weather_wind, 42);
    assert_eq!(loaded.pe_b_weather_humid, 53);
    assert_eq!(loaded.pe_b_weather_trend, WEATHER_TREND_SNOW);
    assert_eq!(loaded.misc_humidity(), Some(53));

    loaded
        .save(&path)
        .expect("save updated Mac-layout weather state");
    let round_trip = City::load(&path).expect("reload updated Mac-layout weather state");
    for ((offset, before), value) in
        saved
            .into_iter()
            .zip([31, 42, 53, u32::from(WEATHER_TREND_SNOW)])
    {
        assert_eq!(
            round_trip.misc_u32_at(offset),
            Some((before & !0xff) | value)
        );
    }
    assert_eq!(round_trip.pe_b_weather_heat, 31);
    assert_eq!(round_trip.pe_b_weather_wind, 42);
    assert_eq!(round_trip.pe_b_weather_humid, 53);
    assert_eq!(round_trip.pe_b_weather_trend, WEATHER_TREND_SNOW);
    assert_eq!(round_trip.misc_humidity(), Some(53));
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_fidelity_weather_survives_win_layout_save_mirrors() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-weather-win-layout-{}.sc2",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "Mac Weather").expect("blank city");
    city.kind = CityFileKind::WinSc2;
    city.save(&path).expect("write Win-layout fixture");
    let mut loaded = City::load(&path).expect("load Win-layout fixture");
    let state = WeatherState {
        heat: 31,
        wind: 42,
        humidity: 53,
        trend: WEATHER_TREND_SNOW,
        season: 2,
    };
    persist_mac_weather_state_to_city(&mut loaded, state);
    assert_eq!(loaded.pe_b_weather_heat, 31);
    assert_eq!(loaded.pe_b_weather_wind, 42);
    assert_eq!(loaded.pe_b_weather_humid, 53);
    assert_eq!(loaded.pe_b_weather_trend, WEATHER_TREND_SNOW);
    loaded.save(&path).expect("save Mac-engine weather");

    let round_trip = City::load(&path).expect("reload Mac-engine weather");
    for (offset, value) in [
        (MISC_HEAT_OFFSET, 31),
        (MISC_WIND_OFFSET, 42),
        (MISC_HUMID_OFFSET, 53),
        (MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_SNOW)),
    ] {
        assert_eq!(round_trip.misc_u32_at(offset), Some(value));
    }
    assert_eq!(round_trip.pe_b_weather_heat, 31);
    assert_eq!(round_trip.pe_b_weather_humid, 53);
    let _ = std::fs::remove_file(path);
}

#[test]
fn simulation_day21_advances_weather_misc_fields() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank");
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 40);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 15);
    city.set_misc_u32_at(MISC_HUMID_OFFSET, 0);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_COLD));
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.game_clock = GameClock::new_with_day(1900, 1, 20);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 21);
    let trend = state
        .city
        .misc_u32_at(MISC_WEATHER_TREND_OFFSET)
        .expect("trend");
    assert!(trend <= u32::from(WEATHER_TREND_TORNADO));
    assert!(WEATHER_TRANSITIONS[0][0].contains(&(trend as u8)));
    assert_ne!(
        state.city.misc_u32_at(MISC_HEAT_OFFSET),
        Some(40),
        "day 21 should ease heat toward the new trend target"
    );
}

#[test]
fn new_city_misc_seeds_pe_weather_defaults() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    initialize_original_new_city_national_misc(&mut city);
    assert_eq!(
        city.misc_u32_at(MISC_HEAT_OFFSET),
        Some(DEFAULT_WEATHER_HEAT)
    );
    assert_eq!(
        city.misc_u32_at(MISC_WIND_OFFSET),
        Some(DEFAULT_WEATHER_WIND)
    );
    assert_eq!(
        city.misc_u32_at(MISC_HUMID_OFFSET),
        Some(DEFAULT_WEATHER_HUMID)
    );
    assert_eq!(
        city.misc_u32_at(MISC_WEATHER_TREND_OFFSET),
        Some(DEFAULT_WEATHER_TREND)
    );
    assert_eq!(city.pe_dw_city_has_ocean(), 0);
    assert_eq!(city.pe_dw_city_has_river(), 1);
}

#[test]
fn pe_sim_mutable_inputs_use_misc_weather_slots() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_WIND_OFFSET, 0x18);
    city.set_misc_u32_at(MISC_HUMID_OFFSET, 0x1e);
    assert_eq!(pe_power_c8_random_modulus(&city), 4);
    assert_eq!(pe_power_cc_random_span(&city), 7);
    city.set_altitude(10, 10, 0, 0);
    assert_eq!(pe_power_c8_pe_units(&city, 10, 10, 3), 1);
    city.set_altitude(10, 10, 20, 0);
    assert_eq!(pe_power_c8_pe_units(&city, 10, 10, 3), (20 + 3) / 2);
    assert_eq!(pe_power_cc_pe_units(&city, 11), 9);
}

#[test]
fn mac68k_day21_runs_sane_national_drift_for_loaded_layout() {
    // The selected Mac engine owns phase behavior even when the active city
    // uses the default Win95 file layout. This deterministic writer uses the
    // shared national MISC slots and must not advance PE's CRT stream.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-day21-national.sc2", "MacD21Nat").expect("blank city");
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 5_000_001);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 3_499_999);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 1);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let pe_crt_before = state.pe_process_crt_seed();

    let _ = state.sim_calendar_day21_demand_and_graphs();

    assert_eq!(
        state.city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(4_995_834)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET),
        Some(3_508_748)
    );
    assert_eq!(
        state.pe_process_crt_seed(),
        pe_crt_before,
        "Mac national base drift has no CRT draws"
    );
}

#[test]
fn refresh_live_misc_rewrites_stale_city_fame() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_CITY_FAME_OFFSET, 999);
    let mut state = test_state(city);

    state.refresh_live_misc_current_values();

    assert_eq!(state.city.misc_u32_at(MISC_CITY_FAME_OFFSET), Some(0));
}

#[test]
fn live_city_fame_responds_to_city_quality() {
    let mut neglected = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_city_fame_population_for_test(&mut neglected);
    neglected.set_building(52, 50, 207);
    neglected.set_building(
        50,
        49,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    neglected.set_traffic_density(50, 49, 192);

    let mut serviced = neglected.clone();
    stamp_complete_building_for_test(&mut serviced, 42, 42, 210, 3);
    stamp_complete_building_for_test(&mut serviced, 46, 42, 211, 3);
    stamp_complete_building_for_test(&mut serviced, 42, 46, 209, 3);
    stamp_complete_building_for_test(&mut serviced, 46, 46, 214, 3);
    stamp_complete_building_for_test(&mut serviced, 50, 42, 213, 3);
    stamp_complete_building_for_test(&mut serviced, 50, 46, 236, 2);

    let mut neglected_state = test_state(neglected);
    let mut serviced_state = test_state(serviced);
    neglected_state.refresh_live_misc_current_values();
    serviced_state.refresh_live_misc_current_values();

    let neglected_fame = neglected_state
        .city
        .misc_u32_at(MISC_CITY_FAME_OFFSET)
        .unwrap();
    let serviced_fame = serviced_state
        .city
        .misc_u32_at(MISC_CITY_FAME_OFFSET)
        .unwrap();
    assert!(serviced_fame > neglected_fame);
    assert!(serviced_fame <= 100);
}

#[test]
fn win95_cache_publishers_preserve_mac_misc_when_running_a_mac_city() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-misc-pe-publishers.sc2", "MacMisc").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.city.test_seed_pe_age_cohorts_from_pop(20_000);
    state.city.advance_pe_eq_le_month(
        20_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );

    let offsets = [
        MISC_CITY_MODE_OFFSET,
        MISC_CITY_ROTATION_OFFSET,
        MISC_CITY_AGE_OFFSET,
        MISC_DIFFICULTY_OFFSET,
        MISC_CITY_PROGRESSION_OFFSET,
        MISC_CITY_CRIME_OFFSET,
        MISC_CITY_TRAFFIC_OFFSET,
        MISC_CITY_POLLUTION_OFFSET,
        MISC_CITY_FAME_OFFSET,
        MISC_CITY_ADVERTISING_OFFSET,
        MISC_GARBAGE_OFFSET,
        MISC_WORK_FORCE_PERCENT_OFFSET,
        MISC_CITY_LIFE_EXPECTANCY_OFFSET,
        MISC_CITY_EDUCATION_QUOTIENT_OFFSET,
        0x007c,
        MISC_HEAT_OFFSET,
        MISC_WIND_OFFSET,
        MISC_HUMID_OFFSET,
        MISC_WEATHER_TREND_OFFSET,
        MISC_TRIGGER_DISASTER_OFFSET,
        MISC_RESIDENTIAL_POPULATION_OFFSET,
        MISC_REWARDS_AVAILABLE_OFFSET,
        MISC_BUILDING_TILE_COUNTS_OFFSET,
        MISC_PE_GROWTH_AGGREGATE_OFFSET,
        MISC_PE_GROWTH_AGGREGATE_OFFSET + 8,
        MISC_PE_GROWTH_AGGREGATE_OFFSET + 16,
        MISC_PE_GROWTH_AGGREGATE_OFFSET + 24,
        MISC_UNEMPLOYMENT_OFFSET,
        MISC_MILITARY_TILE_COUNTS_OFFSET,
        MISC_ARCOLOGY_POPULATION_OFFSET,
        MISC_NORMAL_POPULATION_OFFSET,
        MISC_SUBWAY_COUNT_OFFSET,
        MISC_SPEED_OFFSET,
        MISC_SPORTS_TEAMS_OFFSET,
        MISC_INDUSTRIAL_MIX_BONUS_OFFSET,
        MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET,
        MISC_OLD_ARRESTS_OFFSET,
        MISC_PRISON_BONUS_OFFSET,
        MISC_MONSTER_XTHG_INDEX_OFFSET,
        MISC_CURRENT_DISASTER_TYPE_OFFSET,
        MISC_DISASTER_ACTIVE_OFFSET,
        MISC_SEWER_BONUS_OFFSET,
        0x0e40,
        0x0e44,
        0x0e48,
        0x0e3c,
        0x0f98,
        0x0fa0,
        0x0fe0,
        0x1024,
    ];
    let expected = offsets
        .iter()
        .enumerate()
        .map(|(index, offset)| {
            let value = 0x5a00_0000_u32.wrapping_add(index as u32);
            state.city.set_misc_u32_at(*offset, value);
            (*offset, value)
        })
        .collect::<Vec<_>>();

    persist_pe_garbage_day22(&mut state.city, PopulationStats::default());
    persist_pe_year_end_misc(&mut state.city, 12);
    state.refresh_live_misc_current_values();
    state.sim_calendar_day23_stats_dialogs();
    state.zone_growth_enabled = true;
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
    state.publish_pe_growth_aggregate();

    for (offset, value) in expected {
        assert_eq!(
            state.city.misc_u32_at(offset),
            Some(if offset == MISC_NORMAL_POPULATION_OFFSET {
                state.city.pe_dw_city_population()
            } else {
                value
            }),
            "PE cache publisher must preserve Mac-owned MISC {offset:#06x}"
        );
    }
    assert!(state.city.pe_dw_city_population() > 0);
    assert!(state.city.pe_zone_pops_ready());
}

#[test]
fn live_misc_persists_pe_sewer_and_industrial_mix_bonuses() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 0x84);
    city.set_building(11, 10, 0x85);
    city.set_building(12, 10, 0x86);
    city.set_building(13, 10, 0x9f);
    let mix = industrial_mix_bonus_from_tiles(&city) as u16 as u32;
    city.set_pe_w_industrial_mix_bonus(mix as u16);
    let pollution = industrial_mix_pollution_bonus_from_tiles(&city) as i32 as u32;
    city.set_pe_w_industrial_mix_pollution_bonus(pollution as i16);
    publish_pe_sewer_bonus(&mut city);
    let sewer = sewer_bonus_for_pollution_blur(&city) as u16 as u32;
    let mut state = test_state(city);

    state.refresh_live_misc_current_values();

    assert_eq!(
        state.city.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET),
        Some(mix)
    );
    assert_eq!(state.city.pe_w_industrial_mix_bonus(), mix as u16);
    assert_eq!(
        state
            .city
            .misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
        Some(pollution)
    );
    assert_eq!(state.city.misc_u32_at(MISC_SEWER_BONUS_OFFSET), Some(sewer));
    assert_ne!(
        GRAPH_EXTRA_HISTORY_MISC_OFFSET, MISC_INDUSTRIAL_MIX_BONUS_OFFSET,
        "extra graph history must not occupy stock bonus MISC slots"
    );
    assert_ne!(
        GRAPH_HISTORY_MISC_OFFSET, 0x007c,
        "stock MISC 0x7c is EQ_LE [cohort,aux,life]×20, not graph history"
    );
}

#[test]
fn pe_sewer_blur_state_round_trips_on_mac_layout_without_replacing_mac_scenario_flag() {
    let path = std::env::temp_dir().join(format!(
        "citygame-pe-sewer-cross-layout-{}.city",
        std::process::id()
    ));
    let mut source = City::new_blank(&path, "MacSewerState").expect("blank city");
    source.kind = CityFileKind::MacCity;
    source.set_mac_state_misc_u32_at(MISC_SEWER_BONUS_OFFSET, 1);
    source.save(&path).expect("save Mac-layout city");

    let mut loaded = City::load(&path).expect("load Mac-layout city");
    assert_eq!(
        loaded.pe_w_sewer_bonus(),
        0,
        "PE cache waits for engine choice"
    );
    loaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(loaded.pe_w_sewer_bonus(), 1, "first PE open reads raw MISC");
    assert_eq!(pollution_blur_divisor_base_for_city(&loaded), 5);

    loaded.set_pe_w_sewer_bonus(0);
    loaded
        .save(&path)
        .expect("save Win95 state into Mac layout");
    let mut reloaded = City::load(&path).expect("reload cross-layout city");
    reloaded.prepare_pe_fidelity_from_saved_city();
    assert_eq!(reloaded.pe_w_sewer_bonus(), 0, "CGEX overrides Mac slot");
    assert_eq!(pollution_blur_divisor_base_for_city(&reloaded), 4);
    assert_eq!(
        reloaded.mac_state_misc_u32_at(MISC_SEWER_BONUS_OFFSET),
        Some(1),
        "PE sewer state must not clear Mac's scenario-active byte"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn live_misc_persists_old_arrests_and_prison_bonus() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 9_496);
    city.provision_microsim_for_building(210).expect("police");
    city.provision_microsim_for_building(216).expect("prison");
    city.refresh_structure_microsim_yearly(55_860, 100, 100, 100, 100, 100);
    let arrests = police_old_arrests_for_city(&city);
    let bonus = prison_bonus_for_city(&city);
    let mut state = test_state(city);

    state.refresh_live_misc_current_values();

    assert_eq!(
        state.city.misc_u32_at(MISC_OLD_ARRESTS_OFFSET),
        Some(arrests)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_PRISON_BONUS_OFFSET),
        Some(bonus)
    );
    assert_eq!(bonus, 1);
    assert!(
        arrests > 0,
        "year-end police writes arrest accumulator to BSS"
    );
}

#[test]
fn mac_game_lcg_survives_city_open_for_both_city_layouts() {
    for (index, (extension, kind)) in [
        ("city", CityFileKind::MacCity),
        ("sc2", CityFileKind::WinSc2),
    ]
    .into_iter()
    .enumerate()
    {
        let path = std::env::temp_dir().join(format!(
            "sc2000_mac_game_lcg_city_open_{}_{}.{}",
            std::process::id(),
            index,
            extension
        ));
        let mut city = City::new_blank(&path, "LCG continuation").expect("blank city");
        city.kind = kind;
        city.save(&path).expect("save city");

        let initial = City::new_blank("/tmp/sc2000-mac-game-lcg-open-test.sc2", "initial")
            .expect("initial city");
        let mut state = test_state(initial);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_game_rng =
            crate::mac_random::MacGameLcgRng::new(0x1020_3040u32.wrapping_add(index as u32));
        let before_open = state.mac_game_rng;
        let mut expected = before_open;
        let expected_roll = expected.bounded(100);

        state
            .load_city(&path, (800, 600), 1.0)
            .expect("open saved city");

        assert_eq!(state.city.kind, kind);
        assert_eq!(
            state.mac_game_rng, before_open,
            "opening {kind:?} must not rewind the process-owned Mac game LCG"
        );
        assert_eq!(state.mac_game_rng.bounded(100), expected_roll);
        assert_eq!(state.mac_game_rng.state(), expected.state());
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn new_city_applies_original_technology_schedule() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));

    state.new_city().expect("new city");

    // PE @ `0x42e585` reseeds Highways from `0x4e99e8` base 1920 + rand%0x14.
    assert_eq!(state.city.pe_w_year_founded, 1900);
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_FOUNDED_OFFSET), Some(1900));
    assert_eq!(state.city.city_age_days(), Some(0));
    let highways = state
        .city
        .technology_discovery_year(Technology::Highways)
        .expect("highways year");
    assert!(
        (1920..1940).contains(&highways),
        "expected PE Highways band 1920..1940, got {highways}"
    );
    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::Road)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::Highway)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::PowerPlantGas)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::Airport)));
}

#[test]
fn new_city_seeds_win95_xrog_to_neutral_before_first_simulation_pass() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-new-city-xrog.sc2", "test").expect("blank"));

    state.new_city().expect("new city");

    assert_eq!(
        state.city.rate_of_growth_at(0, 0),
        Some(RATE_OF_GROWTH_NEUTRAL)
    );
    assert_eq!(
        state
            .city
            .rate_of_growth_at(CITY_WIDTH - 1, CITY_HEIGHT - 1),
        Some(RATE_OF_GROWTH_NEUTRAL)
    );
    assert_eq!(state.city.population_density_at(0, 0), Some(0));
    assert_eq!(state.city.police_coverage_at(0, 0), Some(0));
    assert_eq!(state.city.fire_coverage_at(0, 0), Some(0));
}

#[test]
fn win95_new_city_allocates_and_saves_cleared_density_planes() {
    let mut state = test_state(City::new_blank("/tmp/citygame-density-init.sc2", "test").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_city().unwrap();
    for plane in [
        state.city.land_value_slice(),
        state.city.crime_density_slice(),
        state.city.pollution_density_slice(),
    ] {
        assert!(plane.unwrap().iter().all(|&value| value == 0));
    }
    state.generate_edit_map_with_seed(123);
    for plane in [
        state.city.land_value_slice(),
        state.city.crime_density_slice(),
        state.city.pollution_density_slice(),
    ] {
        assert!(plane.unwrap().iter().all(|&value| value == 0));
    }

    let path =
        std::env::temp_dir().join(format!("citygame-density-init-{}.sc2", std::process::id()));
    state.city.save(&path).unwrap();
    let reloaded = City::load(&path).unwrap();
    for plane in [
        reloaded.land_value_slice(),
        reloaded.crime_density_slice(),
        reloaded.pollution_density_slice(),
    ] {
        assert!(plane.unwrap().iter().all(|&value| value == 0));
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn new_city_initializes_original_national_misc_defaults() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));

    state.new_city().expect("new city");

    assert_eq!(
        state.city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(10_000)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET),
        Some(3_000)
    );
    assert_eq!(state.city.misc_u32_at(MISC_NATIONAL_TAX_OFFSET), Some(3));
    // Medium difficulty (2) → trend 1 @ PE `0x42e562`.
    assert_eq!(state.city.misc_u32_at(MISC_NATIONAL_TREND_OFFSET), Some(1));
    assert_eq!(state.city.pe_w_national_trend, 1);
    // PE new-city @ `0x40e173`: ocean=0 river=1.
    assert_eq!(state.city.pe_dw_city_has_ocean(), 0);
    assert_eq!(state.city.pe_dw_city_has_river(), 1);
    assert_eq!(state.city.misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET), Some(0));
    assert_eq!(state.city.misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET), Some(1));
}

#[test]
fn pe_recalculate_city_value_stores_4ca4d0_like_46a64d() {
    let mut city = City::new_blank("/tmp/sc2000-cityval-full.sc2", "CityValFull").expect("blank");
    city.pe_xund_occupancy = 1;
    city.pe_dw_tile_count[0x1d] = 1; // road *10
    city.pe_dw_tile_count[0xc6] = 1; // hydro *4000
    pe_recalculate_city_value_46a270(&mut city);
    assert_eq!(city.pe_city_value, -1 + 10 + 4000);
}

#[test]
fn win95_retail_city_value_matches_native_reference_vectors() {
    let mut all_positive = [0i16; 256];
    let mut signed_edges = [0i16; 256];
    let mut wrapping_sum = [0i16; 256];
    let signed_cases = [-32768, -17, -16, -15, -9, -1, 0, 1, 8, 9, 15, 16, 17, 32767];

    for tile in (0x0eusize..0x70).chain(0xc6..0xff) {
        all_positive[tile] = ((tile * 37 + 11) % 73) as i16;
        signed_edges[tile] = signed_cases[(tile * 7 + 3) % signed_cases.len()];
        wrapping_sum[tile] = if tile & 1 == 0 { i16::MAX } else { i16::MIN };
    }

    for (label, xund, counts, expected) in [
        ("all-positive-case-table", 19i16, all_positive, 2_353_627i32),
        (
            "signed-word-division-edges",
            i16::MIN,
            signed_edges,
            -130_337,
        ),
        (
            "signed-word-wrapping-accumulation",
            i16::MAX,
            wrapping_sum,
            443_153_418,
        ),
    ] {
        let mut city = City::new_blank(
            format!("/tmp/citygame-city-value-{label}.sc2"),
            "CityValueReference",
        )
        .expect("blank city");
        city.pe_xund_occupancy = xund as u16;
        for (slot, count) in city.pe_dw_tile_count.iter_mut().zip(counts) {
            *slot = count as u16;
        }

        pe_recalculate_city_value_46a270(&mut city);

        assert_eq!(
            city.pe_city_value, expected,
            "native reference case {label}"
        );
    }
}

#[test]
fn sc2kfix_pe_city_value_matches_full_tile_case_map() {
    let mut city =
        City::new_blank("/tmp/sc2000-cityval-case-map.sc2", "CityValCaseMap").expect("blank");
    // Use a full footprint count even on unhandled IDs so an accidental broad
    // range arm cannot be hidden by integer division rounding to zero.
    let mut counts = [16u16; 256];
    let mut expected = [0i32; 256];

    for (range, cost) in [
        (0x0e..=0x1c, 2),
        (0x1d..=0x2b, 10),
        (0x2c..=0x3e, 25),
        (0x3f..=0x50, 15),
        (0x51..=0x60, 100),
        (0x61..=0x6b, HIGHWAY_COST_PER_SEGMENT as i32),
        (0x6c..=0x6f, 250),
    ] {
        for tile in range {
            expected[tile] = cost;
        }
    }

    let mut set = |tile: u8, count: u16, value: i32| {
        counts[tile as usize] = count;
        expected[tile as usize] = value;
    };
    for tile in [0xc6, 0xc7] {
        set(tile, 1, 400);
    }
    set(0xc8, 1, 100);
    for (tile, cost) in [
        (0xc9, 2_000),
        (0xca, 6_600),
        (0xcb, 15_000),
        (0xcc, 1_300),
        (0xcd, 28_000),
        (0xce, 40_000),
        (0xcf, 4_000),
    ] {
        set(tile, 16, cost);
    }
    for (tile, cost) in [
        (0xd1, 500),
        (0xd2, 500),
        (0xd3, 500),
        (0xd4, 1_000),
        (0xd5, 150),
        (0xd6, 250),
    ] {
        set(tile, 9, cost);
    }
    for (tile, cost) in [(0xd7, 5_000), (0xd8, 3_000), (0xd9, 1_000), (0xda, 3_000)] {
        set(tile, 16, cost);
    }
    set(0xdc, 1, 100);
    for tile in [0xdd, 0xde, 0xe1, 0xe4, 0xe5, 0xe6, 0xea, 0xee, 0xf6] {
        set(tile, 1, 250);
    }
    for tile in [0xdf, 0xe0, 0xe3, 0xf0, 0xf2] {
        set(tile, 1, 150);
    }
    set(0xe9, 1, 250);
    set(0xeb, 4, 250);
    set(0xec, 4, 250);
    set(0xed, 4, 500);
    set(0xf4, 4, 500);
    set(0xf5, 4, 500);
    set(0xf8, 9, 1_000);
    set(0xfa, 9, 1_000);
    for (tile, cost) in [
        (0xfb, 100_000),
        (0xfc, 120_000),
        (0xfd, 150_000),
        (0xfe, 200_000),
    ] {
        set(tile, 16, cost);
    }

    for tile in 0x0eu8..=0x6f {
        city.pe_dw_tile_count[tile as usize] = 1;
        assert_eq!(
            sc2kfix_pe_city_value_total(&city),
            expected[tile as usize],
            "infrastructure tile {tile:#04x}"
        );
        city.pe_dw_tile_count[tile as usize] = 0;
    }
    for tile in 0xc6u8..=0xfe {
        city.pe_dw_tile_count[tile as usize] = counts[tile as usize];
        assert_eq!(
            sc2kfix_pe_city_value_total(&city),
            expected[tile as usize],
            "building tile {tile:#04x}"
        );
        city.pe_dw_tile_count[tile as usize] = 0;
    }
}

#[test]
fn sc2kfix_city_value_refreshes_each_win95_day_not_mac_day() {
    let mut city = City::new_blank("/tmp/sc2000-cityval-day.sc2", "CityValDay").expect("blank");
    city.pe_dw_tile_count[0x1d] = 3;
    city.pe_city_value = 777;

    let mut win = test_state(city.clone());
    win.engine_fidelity = EngineFidelity::Win95;
    win.handle_simulation_day_advanced(1900, 1, 5);
    assert_eq!(win.city.pe_city_value, 30);

    let mut mac = test_state(city);
    mac.engine_fidelity = EngineFidelity::Mac68k;
    mac.handle_simulation_day_advanced(1900, 1, 5);
    assert_eq!(mac.city.pe_city_value, 777);
}

#[test]
fn city_load_hydrates_pe_city_value_from_misc_430878() {
    // PE MISC→BSS @ `0x430878`: `[MISC+0x24]` → `[0x4ca4d0]`.
    let path = "/tmp/sc2000-cityval-hydrate.sc2";
    let mut city = City::new_blank(path, "CityValHydrate").expect("blank");
    city.pe_city_value = 0x0012_3456;
    city.set_misc_u32_at(MISC_CITY_VALUE_OFFSET, 0x0012_3456);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_city_value, 0x0012_3456);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_city_value_to_misc_432ee0() {
    // PE BSS→MISC @ `0x432ee0`: `[0x4ca4d0]` → `[MISC+0x24]`.
    let path = "/tmp/sc2000-cityval-persist.sc2";
    let mut city = City::new_blank(path, "CityValPersist").expect("blank");
    city.pe_city_value = -7_654;
    city.save(path).expect("save");
    assert_eq!(
        city.misc_u32_at(MISC_CITY_VALUE_OFFSET),
        Some((-7_654i32) as u32)
    );
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_city_value, -7_654);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_w_city_garbage_from_misc_4314b5() {
    // PE MISC→BSS @ `0x4314b5`: `[MISC+0x40]` → `[0x4ca5f0]`.
    let path = "/tmp/sc2000-garbage-hydrate.sc2";
    let mut city = City::new_blank(path, "GarbageHydrate").expect("blank");
    city.pe_w_city_garbage = 0x00de_ad01;
    city.set_misc_u32_at(MISC_GARBAGE_OFFSET, 0x00de_ad01);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_garbage, 0x00de_ad01);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_w_city_garbage_to_misc_433b42() {
    // PE BSS→MISC @ `0x433b42`: `[0x4ca5f0]` → `[MISC+0x40]`.
    let path = "/tmp/sc2000-garbage-persist.sc2";
    let mut city = City::new_blank(path, "GarbagePersist").expect("blank");
    city.pe_w_city_garbage = 12_345;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_GARBAGE_OFFSET), Some(12_345));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_garbage, 12_345);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_dw_city_fame_from_misc_4314a5() {
    // PE MISC→BSS @ `0x4314a5`: `[MISC+0x38]` → `[0x4cad28]`.
    let path = "/tmp/sc2000-fame-hydrate.sc2";
    let mut city = City::new_blank(path, "FameHydrate").expect("blank");
    city.pe_dw_city_fame = 73;
    city.set_misc_u32_at(MISC_CITY_FAME_OFFSET, 73);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_fame, 73);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_city_fame_to_misc_433b26() {
    // PE BSS→MISC @ `0x433b26`: `[0x4cad28]` → `[MISC+0x38]`.
    let path = "/tmp/sc2000-fame-persist.sc2";
    let mut city = City::new_blank(path, "FamePersist").expect("blank");
    city.pe_dw_city_fame = 88;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_CITY_FAME_OFFSET), Some(88));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_fame, 88);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_dw_city_advertising_from_misc_4314ae() {
    // PE MISC→BSS @ `0x4314ae`: `[MISC+0x3c]` → `[0x4cb018]`.
    let path = "/tmp/sc2000-ad-hydrate.sc2";
    let mut city = City::new_blank(path, "AdHydrate").expect("blank");
    city.pe_dw_city_advertising = 0x1111_2222;
    city.set_misc_u32_at(MISC_CITY_ADVERTISING_OFFSET, 0x1111_2222);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_advertising, 0x1111_2222);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_city_advertising_to_misc_433b34() {
    // PE BSS→MISC @ `0x433b34`: `[0x4cb018]` → `[MISC+0x3c]`.
    let path = "/tmp/sc2000-ad-persist.sc2";
    let mut city = City::new_blank(path, "AdPersist").expect("blank");
    city.pe_dw_city_advertising = 9_001;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_CITY_ADVERTISING_OFFSET), Some(9_001));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_advertising, 9_001);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_b_weather_trend_from_misc_43151e() {
    // PE MISC→BSS @ `0x43151e`: `[MISC+0x6c]` → `[0x4cb40c]`.
    let path = "/tmp/sc2000-weather-hydrate.sc2";
    let mut city = City::new_blank(path, "WeatherHydrate").expect("blank");
    city.pe_b_weather_trend = WEATHER_TREND_BLIZZARD;
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_BLIZZARD));
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_b_weather_trend, WEATHER_TREND_BLIZZARD);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_b_weather_trend_to_misc_433be1() {
    // PE BSS→MISC @ `0x433be1`: `[0x4cb40c]` → `[MISC+0x6c]`.
    let path = "/tmp/sc2000-weather-persist.sc2";
    let mut city = City::new_blank(path, "WeatherPersist").expect("blank");
    city.pe_b_weather_trend = WEATHER_TREND_WINDY;
    city.save(path).expect("save");
    assert_eq!(
        city.misc_u32_at(MISC_WEATHER_TREND_OFFSET),
        Some(u32::from(WEATHER_TREND_WINDY))
    );
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_b_weather_trend, WEATHER_TREND_WINDY);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_weather_trend_reads_bss_4cb40c_not_stale_misc() {
    let mut city = City::new_blank("/tmp/sc2000-weather-bss.sc2", "WeatherBss").expect("blank");
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    // After set_misc sync, force BSS ahead of MISC like a live PE weather tick.
    city.pe_b_weather_trend = WEATHER_TREND_FOGGY;
    assert_eq!(pe_weather_trend(&city), WEATHER_TREND_FOGGY);
    assert_ne!(
        city.misc_u32_at(MISC_WEATHER_TREND_OFFSET),
        Some(u32::from(WEATHER_TREND_FOGGY)),
        "MISC deliberately stale; PE gates use BSS"
    );
}

#[test]
fn city_load_hydrates_pe_b_weather_heat_from_misc_431505() {
    // PE MISC→BSS @ `0x431505`: `[MISC+0x60]` → `[0x4cade0]`.
    let path = "/tmp/sc2000-heat-hydrate.sc2";
    let mut city = City::new_blank(path, "HeatHydrate").expect("blank");
    city.pe_b_weather_heat = 0xaa;
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xaa);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_b_weather_heat, 0xaa);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_b_weather_heat_to_misc_433bb6() {
    // PE BSS→MISC @ `0x433bb6`: `[0x4cade0]` → `[MISC+0x60]`.
    let path = "/tmp/sc2000-heat-persist.sc2";
    let mut city = City::new_blank(path, "HeatPersist").expect("blank");
    city.pe_b_weather_heat = 0x7f;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_HEAT_OFFSET), Some(0x7f));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_b_weather_heat, 0x7f);
    let _ = std::fs::remove_file(path);
}

#[test]
fn ensure_default_city_options_reads_bss_difficulty_4cb404() {
    let mut city = City::new_blank("/tmp/sc2000-diff-bss.sc2", "DiffBss").expect("blank");
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, 0);
    city.pe_w_city_difficulty = CITY_DIFFICULTY_HARD as u16;
    ensure_default_city_options(&mut city);
    assert_eq!(
        city.pe_w_city_difficulty, CITY_DIFFICULTY_HARD as u16,
        "new-city options must not overwrite non-zero BSS difficulty"
    );
    city.pe_w_city_difficulty = 0;
    ensure_default_city_options(&mut city);
    assert_eq!(city.pe_w_city_difficulty, CITY_DIFFICULTY_MEDIUM as u16);
}

#[test]
fn city_load_hydrates_workforce_le_eq_bss_without_eq_tables_4314bb() {
    // PE always copies MISC+0x44/0x48/0x4c → 0x4cb460/0x4cb410/0x4cc4b4.
    let path = "/tmp/sc2000-wf-le-eq-hydrate.sc2";
    let mut city = City::new_blank(path, "WfLeEq").expect("blank");
    city.set_misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET, 55);
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 71);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 88);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_eq_workforce_percent(), 55);
    assert_eq!(loaded.pe_eq_life_average(), 71);
    assert_eq!(loaded.pe_eq_education_average(), 88);
    assert!(loaded.pe_eq_averages_ready());
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_eq_workforce_to_misc_433b49() {
    let path = "/tmp/sc2000-wf-persist.sc2";
    let mut city = City::new_blank(path, "WfPersist").expect("blank");
    city.set_misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET, 42);
    assert_eq!(city.pe_eq_workforce_percent(), 42);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET), Some(42));
    assert_eq!(loaded.pe_eq_workforce_percent(), 42);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_b_weather_wind_from_misc_43150d() {
    // PE MISC→BSS @ `0x43150d`: `[MISC+0x64]` → `[0x4cae0c]`.
    let path = "/tmp/sc2000-wind-hydrate.sc2";
    let mut city = City::new_blank(path, "WindHydrate").expect("blank");
    city.pe_b_weather_wind = 0x55;
    city.set_misc_u32_at(MISC_WIND_OFFSET, 0x55);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_b_weather_wind, 0x55);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_b_weather_wind_to_misc_433bc1() {
    // PE BSS→MISC @ `0x433bc1`: `[0x4cae0c]` → `[MISC+0x64]`.
    let path = "/tmp/sc2000-wind-persist.sc2";
    let mut city = City::new_blank(path, "WindPersist").expect("blank");
    city.pe_b_weather_wind = 0x22;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_WIND_OFFSET), Some(0x22));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_b_weather_wind, 0x22);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_b_weather_humid_from_misc_431516() {
    // PE MISC→BSS @ `0x431516`: `[MISC+0x68]` → `[0x4cb3d4]`.
    let path = "/tmp/sc2000-humid-hydrate.sc2";
    let mut city = City::new_blank(path, "HumidHydrate").expect("blank");
    city.pe_b_weather_humid = 0xcc;
    city.set_misc_u32_at(MISC_HUMID_OFFSET, 0xcc);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_b_weather_humid, 0xcc);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_b_weather_humid_to_misc_433bcf() {
    // PE BSS→MISC @ `0x433bcf`: `[0x4cb3d4]` → `[MISC+0x68]`.
    let path = "/tmp/sc2000-humid-persist.sc2";
    let mut city = City::new_blank(path, "HumidPersist").expect("blank");
    city.pe_b_weather_humid = 0x42;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_HUMID_OFFSET), Some(0x42));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_b_weather_humid, 0x42);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_dw_rewards_available_from_misc_4315a6() {
    // PE MISC→BSS @ `0x4315a6`: `[MISC+0x78]` → `[0x4e9a24]`.
    let path = "/tmp/sc2000-rewards-hydrate.sc2";
    let mut city = City::new_blank(path, "RewardsHydrate").expect("blank");
    city.set_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, 0xabc0_0015);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_rewards_available, 0xabc0_0015);
    assert_eq!(loaded.rewards_available_mask(), 0x15);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_rewards_available_to_misc_433c1f() {
    // PE save MISC mirror @ `0x433c1f`: `[0x4e9a24]` → `[MISC+0x78]`.
    let path = "/tmp/sc2000-rewards-persist.sc2";
    let mut city = City::new_blank(path, "RewardsPersist").expect("blank");
    city.pe_dw_rewards_available = 0xabc0_0016;
    city.save(path).expect("save");
    assert_eq!(
        city.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
        Some(0xabc0_0016)
    );
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_rewards_available, 0xabc0_0016);
    assert_eq!(loaded.rewards_available_mask(), 0x16);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_dw_national_value_from_misc_4314df() {
    // PE MISC→BSS @ `0x4314df`: `[MISC+0x54]` → `[0x4cb014]`.
    let path = "/tmp/sc2000-natval-hydrate.sc2";
    let mut city = City::new_blank(path, "NatValHydrate").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 2_800_000);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_national_value, 2_800_000);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_national_value_to_misc_433b86() {
    // PE save MISC mirror @ `0x433b86`: `[0x4cb014]` → `[MISC+0x54]`.
    let path = "/tmp/sc2000-natval-persist.sc2";
    let mut city = City::new_blank(path, "NatValPersist").expect("blank");
    city.pe_dw_national_value = 2_100_000;
    city.save(path).expect("save");
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET),
        Some(2_100_000)
    );
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_national_value, 2_100_000);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_w_national_trend_from_misc_4314f2() {
    // PE MISC→BSS @ `0x4314f2`: word `[MISC+0x5c]` → `[0x4ca1bc]`.
    let path = "/tmp/sc2000-nattrend-hydrate.sc2";
    let mut city = City::new_blank(path, "NatTrendHydrate").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 2);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_national_trend, 2);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_w_national_trend_to_misc_433ba5() {
    // PE save MISC mirror @ `0x433ba5`: word `[0x4ca1bc]` → `[MISC+0x5c]`.
    let path = "/tmp/sc2000-nattrend-persist.sc2";
    let mut city = City::new_blank(path, "NatTrendPersist").expect("blank");
    city.pe_w_national_trend = 3;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TREND_OFFSET), Some(3));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_national_trend, 3);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_dw_city_age_from_misc_431444() {
    // PE MISC→BSS @ `0x431444`: `[MISC+0x10]` → `[0x4cae04]`.
    let path = "/tmp/sc2000-cityage-hydrate.sc2";
    let mut city = City::new_blank(path, "CityAgeHydrate").expect("blank");
    city.set_misc_u32_at(MISC_CITY_AGE_OFFSET, 350);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_age, 350);
    assert_eq!(loaded.city_age_days(), Some(350));
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_city_age_to_misc_433a9c() {
    // PE save MISC mirror @ `0x433a9c`: `[0x4cae04]` → `[MISC+0x10]`.
    let path = "/tmp/sc2000-cityage-persist.sc2";
    let mut city = City::new_blank(path, "CityAgePersist").expect("blank");
    city.pe_dw_city_age = 777;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_CITY_AGE_OFFSET), Some(777));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_age, 777);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_w_city_difficulty_from_misc_43145f() {
    // PE MISC→BSS @ `0x43145f`: word `[MISC+0x1c]` → `[0x4cb404]`.
    let path = "/tmp/sc2000-difficulty-hydrate.sc2";
    let mut city = City::new_blank(path, "DiffHydrate").expect("blank");
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_difficulty, CITY_DIFFICULTY_HARD as u16);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_w_city_difficulty_to_misc_433ac7() {
    // PE save MISC mirror @ `0x433ac7`: word `[0x4cb404]` → `[MISC+0x1c]`.
    let path = "/tmp/sc2000-difficulty-persist.sc2";
    let mut city = City::new_blank(path, "DiffPersist").expect("blank");
    city.pe_w_city_difficulty = CITY_DIFFICULTY_EASY as u16;
    city.save(path).expect("save");
    assert_eq!(
        city.misc_u32_at(MISC_DIFFICULTY_OFFSET),
        Some(CITY_DIFFICULTY_EASY)
    );
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_difficulty, CITY_DIFFICULTY_EASY as u16);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_w_year_founded_from_misc_43143a() {
    // PE MISC→BSS @ `0x43143a`: word `[MISC+0x0c]` → `[0x4ca5f4]`.
    let path = "/tmp/sc2000-founded-hydrate.sc2";
    let mut city = City::new_blank(path, "FoundedHydrate").expect("blank");
    city.set_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, 1950);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_year_founded, 1950);
    assert_eq!(loaded.year_founded(), Some(1950));
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_w_year_founded_to_misc_433a8e() {
    // PE save MISC mirror @ `0x433a8e`: word `[0x4ca5f4]` → `[MISC+0x0c]`.
    let path = "/tmp/sc2000-founded-persist.sc2";
    let mut city = City::new_blank(path, "FoundedPersist").expect("blank");
    city.pe_w_year_founded = 1900;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_YEAR_FOUNDED_OFFSET), Some(1900));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_year_founded, 1900);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_w_city_mode_from_misc_431426() {
    // PE MISC→BSS @ `0x431426`: word `[MISC+0x04]` → `[0x4ca42c]`.
    let path = "/tmp/sc2000-mode-hydrate.sc2";
    let mut city = City::new_blank(path, "ModeHydrate").expect("blank");
    city.set_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_mode, CITY_MODE_GAMEPLAY as u16);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_w_city_mode_to_misc_433a70() {
    // PE save MISC mirror @ `0x433a70`: word `[0x4ca42c]` → `[MISC+0x04]`.
    let path = "/tmp/sc2000-mode-persist.sc2";
    let mut city = City::new_blank(path, "ModePersist").expect("blank");
    city.pe_w_city_mode = CITY_MODE_EDIT_MAP as u16;
    city.save(path).expect("save");
    assert_eq!(
        city.misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(CITY_MODE_EDIT_MAP)
    );
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_mode, CITY_MODE_EDIT_MAP as u16);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_w_city_progression_from_misc_431469() {
    // PE MISC→BSS @ `0x431469`: word `[MISC+0x20]` → `[0x4cb010]`.
    let path = "/tmp/sc2000-progression-hydrate.sc2";
    let mut city = City::new_blank(path, "ProgHydrate").expect("blank");
    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 4);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_progression, 4);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_w_city_progression_to_misc_433ad6() {
    // PE save MISC mirror @ `0x433ad6`: word `[0x4cb010]` → `[MISC+0x20]`.
    let path = "/tmp/sc2000-progression-persist.sc2";
    let mut city = City::new_blank(path, "ProgPersist").expect("blank");
    city.pe_w_city_progression = 5;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET), Some(5));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_progression, 5);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_dw_tile_count_434f80() {
    let path = "/tmp/sc2000-dw-hydrate-load.sc2";
    let mut city = City::new_blank(path, "DwHydrate").expect("blank");
    city.set_building(5, 5, 0x1d);
    city.set_building(6, 5, 0x1d);
    city.set_zone(7, 5, 7);
    city.set_building(7, 5, 0x0d);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_tile_count[0x1d], 2);
    assert_eq!(loaded.pe_dw_tile_count[0x0d], 0);
    assert_eq!(
        loaded.pe_dw_tile_count[0],
        (CITY_TILES as u16).wrapping_sub(3)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_open_initializes_pe_developed_count_by_engine_for_both_save_layouts() {
    for (index, (extension, kind)) in [
        ("city", CityFileKind::MacCity),
        ("sc2", CityFileKind::WinSc2),
    ]
    .into_iter()
    .enumerate()
    {
        let path = std::env::temp_dir().join(format!(
            "citygame_developed_open_{}_{}.{}",
            std::process::id(),
            index,
            extension
        ));
        let mut city = City::new_blank(&path, "Open census").unwrap();
        city.kind = kind;
        city.set_zone(5, 5, ZONE_COMMERCIAL_LIGHT);
        city.set_building(7, 5, 0x1d);
        city.set_building(8, 5, 0x1c);
        city.set_traffic_density(20, 20, 0x37);
        let mut history = vec![0; XGRP_VALUES];
        let scores = [123, 456, 789, 0xfedc_ba98];
        for (index, value) in scores.into_iter().enumerate() {
            history[(index + 4) * XGRP_SAMPLES_PER_SERIES] = value;
        }
        city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &history);
        city.save(&path).unwrap();
        for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
            let initial = City::new_blank("/tmp/initial-census.sc2", "Initial").unwrap();
            let mut state = test_state(initial);
            state.engine_fidelity = engine;
            state.load_city(&path, (800, 600), 1.0).unwrap();
            assert_eq!(state.city.kind, kind);
            assert_eq!(
                state.city.pe_w_city_developed_tiles,
                if engine.uses_win95() { 2 } else { 0 },
                "{kind:?} {engine:?}"
            );
            assert_eq!(state.city.traffic_density_at(20, 20), Some(0x37));
            let expected = if engine.uses_win95() { scores } else { [0; 4] };
            assert_eq!(
                std::array::from_fn::<_, 4, _>(|i| state.city.pe_attitude_score_at(i)),
                expected
            );
        }
        let startup = ViewerState::load_default_from_path(path.clone()).unwrap();
        assert_eq!(
            startup.city.pe_w_city_developed_tiles,
            if startup.engine_fidelity.uses_win95() {
                2
            } else {
                0
            },
            "startup {kind:?}"
        );
        assert_eq!(startup.city.traffic_density_at(20, 20), Some(0x37));
        let expected = if startup.engine_fidelity.uses_win95() {
            scores
        } else {
            [0; 4]
        };
        assert_eq!(
            std::array::from_fn::<_, 4, _>(|i| startup.city.pe_attitude_score_at(i)),
            expected
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn mac_city_open_restores_cohorts_only_for_selected_win95_engine() {
    let path =
        std::env::temp_dir().join(format!("citygame-cohort-open-{}.city", std::process::id()));
    let mut city = City::new_blank(&path, "Cohort open").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x7c + 12, 1_200);
    city.set_mac_state_misc_u32_at(0x7c + 16, 96_000);
    city.set_mac_state_misc_u32_at(0x7c + 20, 84_000);
    city.set_mac_state_misc_u32_at(0x44, 45);
    city.set_mac_state_misc_u32_at(0x48, 70);
    city.set_mac_state_misc_u32_at(0x4c, 80);
    city.set_mac_state_misc_u32_at(0x102c, 8_000);
    city.save(&path).unwrap();
    for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
        let mut state = test_state(City::new_blank("/tmp/cohort-open.sc2", "Initial").unwrap());
        state.engine_fidelity = engine;
        state.load_city(&path, (800, 600), 1.0).unwrap();
        assert_eq!(state.city.kind, CityFileKind::MacCity);
        if engine.uses_win95() {
            assert_eq!(state.city.pe_age_cohort_at(1), 1_200);
            assert_eq!(state.city.pe_dw_city_population(), 8_000);
            assert_eq!(state.city.pe_eq_life_average(), 70);
            assert_eq!(state.city.pe_eq_education_average(), 80);
            assert_eq!(
                work_force_percent_for_city_with_fidelity(
                    &state.city,
                    PopulationStats::default(),
                    engine
                ),
                45
            );
        } else {
            assert_eq!(state.city.pe_age_cohort_at(1), 0);
            assert_eq!(state.city.pe_dw_city_population(), 0);
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn city_open_replays_native_service_order_for_both_save_layouts() {
    let cells = [
        (40, 40, 0xcc, 0x88),
        (41, 40, 0xdc, 0xa0),
        (42, 40, 0x70, 0xa0),
        (43, 40, 0x71, 0x20),
        (70, 70, 0xdc, 0x78),
        (71, 70, 0x72, 0x30),
        (72, 70, 0xeb, 0x10),
    ];
    let native = [0xc0, 0xf0, 0xf0, 0x20, 0x20, 0x20, 0x10];
    let plane = |city: &City| cells.map(|(x, y, _, _)| city.tile_bits_at(x, y).unwrap().raw_bits());
    for extension in ["sc2", "city"] {
        let path = std::env::temp_dir().join(format!(
            "citygame-open-services-{}.{}",
            std::process::id(),
            extension,
        ));
        let mut city = blank_city_layout_fixture(&path, "Open services");
        for (x, y, building, bits) in cells {
            city.set_building(x, y, building);
            city.set_tile_bits(x, y, TileBits::new(bits));
        }
        city.set_zone(8, 8, ZONE_COMMERCIAL_LIGHT);
        city.set_mac_state_misc_u32_at(0x0e40, 0);
        city.set_mac_state_misc_u32_at(0x60, 150);
        city.set_mac_state_misc_u32_at(0x64, 10);
        city.set_mac_state_misc_u32_at(0x68, 2);
        city.set_mac_state_misc_u32_at(0x6c, 4);
        city.set_mac_state_misc_u32_at(0x1060, 37);
        city.set_mac_state_misc_u32_at(0x1064, 53);
        city.set_traffic_density(20, 20, 0x37);
        for offset in [
            MISC_BUS_PASSENGERS_OFFSET,
            MISC_RAIL_PASSENGERS_OFFSET,
            MISC_SUBWAY_PASSENGERS_OFFSET,
        ] {
            city.set_misc_u32_at(offset, 33);
        }
        ensure_default_city_options(&mut city);
        city.save(&path).unwrap();
        let before = City::load(&path).unwrap();
        let traffic = before.traffic_density_slice();

        for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
            let mut state =
                test_state(City::new_blank("/tmp/open-service-empty.sc2", "Empty").unwrap());
            state.engine_fidelity = engine;
            state.pe_process_crt = Win95CrtRand::new(1);
            state.load_city(&path, (800, 600), 1.0).unwrap();
            assert_eq!(state.city.traffic_density_slice(), traffic);
            if engine.uses_win95() {
                assert_eq!(plane(&state.city), native, "{extension}");
                // The normal pre-open reset advances seed 1 by 20
                // draws before the native service helper rolls hydro.
                assert_eq!(state.city.pe_power_used_percent(), 40);
                assert_eq!(state.city.pe_water_coverage_percent(), 100);
                assert_eq!(state.city.pe_w_sewer_bonus(), 0);
                assert_eq!(state.city.pe_w_city_developed_tiles, 8);
                assert_eq!(state.pe_process_crt.seed(), 4_270_317_332);
                assert_eq!(state.trip_bus_passengers, 0);
                assert_eq!(state.trip_rail_passengers, 0);
                assert_eq!(state.trip_subway_passengers, 0);
                for (x, y) in [(20, 20), (21, 20), (35, 35), (36, 35), (4, 4)] {
                    assert_eq!(state.city.tile_bits_at(x, y).unwrap().raw_bits(), 0x08);
                }
                let date = state.city.saved_date().unwrap();
                for day in 1..=2 {
                    state.handle_simulation_day_advanced(date.year, date.month, day);
                    assert_eq!(state.city.city_age_days(), Some(u32::from(day)));
                    assert_eq!(state.pe_process_crt.seed(), 3_104_592_263);
                    assert_eq!(state.city.pe_power_used_percent(), 18);
                    assert_eq!(state.city.pe_water_coverage_percent(), 100);
                }
                assert_eq!(state.city.pe_dw_city_land_value, 523);
                assert_eq!(state.city.pe_dw_city_crime, 0);
                assert_eq!(state.city.pe_dw_city_pollution, 6);
                // Match the original's compact, x-major plane fingerprints.
                let fingerprint = |plane: &[u8], stride| {
                    (0..128)
                        .step_by(stride)
                        .flat_map(|x| (0..128).step_by(stride).map(move |y| (x, y)))
                        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, (x, y)| {
                            (hash ^ u64::from(plane[x * 128 + y]))
                                .wrapping_mul(0x0000_0100_0000_01b3)
                        })
                };
                for (plane, expected) in [
                    (state.city.land_value_slice(), 0x3b8f_51b8_a425_8abc),
                    (state.city.crime_density_slice(), 0xb93a_0c83_ce3b_6325),
                    (state.city.pollution_density_slice(), 0x6dbd_83dc_c3db_dfab),
                ] {
                    assert_eq!(fingerprint(plane.unwrap(), 2), expected, "{extension}");
                }
                let xbit_fingerprint = |city: &City| {
                    let xbit: Vec<_> = (0..128)
                        .flat_map(|x| (0..128).map(move |y| (x, y)))
                        .map(|(x, y)| city.tile_bits_at(x, y).unwrap().raw_bits())
                        .collect();
                    fingerprint(&xbit, 1)
                };
                assert_eq!(
                    xbit_fingerprint(&state.city),
                    0xd351_cb84_c571_ba9d,
                    "{extension}"
                );
                assert_eq!(state.city.traffic_density_slice(), traffic);
                for day in 3..=20 {
                    state.handle_simulation_day_advanced(date.year, date.month, day);
                    assert_eq!(state.city.city_age_days(), Some(u32::from(day)));
                    assert_eq!(state.pe_process_crt.seed(), 3_104_592_263);
                    assert_eq!(
                        state.city.traffic_density_at(20, 20),
                        Some(if day < 19 { 55 } else { 42 })
                    );
                }
                assert_eq!(state.city.pe_traffic_count(), 42);
                assert_eq!(
                    fingerprint(state.city.traffic_density_slice().unwrap(), 2),
                    0x4c8a_7ccb_df35_5e6f
                );
                assert_eq!(
                    xbit_fingerprint(&state.city),
                    0xbe33_72e7_4b53_16f5,
                    "{extension}"
                );
                // Compare the retail day-21 weather output with its MIDI player
                // already active. The full post-weather EQ/LE state is separate.
                state.handle_simulation_day_advanced(date.year, date.month, 21);
                let weather = weather_state_for_city(&state.city, date.month);
                assert_eq!(
                    (weather.heat, weather.wind, weather.humidity, weather.trend),
                    (157, 20, 1, 1),
                    "{extension}"
                );
            } else {
                assert_eq!(plane(&state.city), plane(&before));
                assert_eq!(state.city.mac_power_service_summary(), 37);
                assert_eq!(state.city.mac_water_service_summary(), 53);
                assert_eq!(state.city.tile_bits_at(20, 20).unwrap().raw_bits(), 0);
            }
        }

        let startup = ViewerState::load_default_from_path_with_process_rng(
            path.clone(),
            Some(Win95CrtRand::new(1)),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(startup.city.traffic_density_slice(), traffic);
        if startup.engine_fidelity.uses_win95() {
            assert_eq!(plane(&startup.city), native);
            assert_eq!(startup.pe_process_crt.seed(), 2_745_024);
        } else {
            assert_eq!(plane(&startup.city), plane(&before));
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn win95_city_open_water_uses_the_saved_selector_before_display_rotation() {
    let path =
        std::env::temp_dir().join(format!("citygame-open-rotated-{}.sc2", std::process::id()));
    let mut city = blank_city_layout_fixture(&path, "Rotated open");
    city.set_mac_state_misc_u32_at(0x0e40, 0);
    city.set_mac_state_misc_u32_at(0x68, 2);
    city.saved_rotation = 1;
    let cells = [(40, 42), (41, 42), (42, 42), (42, 41), (42, 40)];
    for (index, (x, y)) in cells.into_iter().enumerate() {
        city.set_building(x, y, if matches!(index, 0 | 4) { 0xdc } else { 0x70 });
        city.set_tile_bits(x, y, TileBits::new(0xa0));
    }
    city.set_building(39, 42, 0xc6);
    city.set_tile_bits(39, 42, TileBits::new(0x80));
    city.save(&path).unwrap();

    let mut state = test_state(City::new_blank("/tmp/open-rotated-empty.sc2", "Empty").unwrap());
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert_eq!(state.city.pe_w_city_rotation, 1);
    assert_eq!(state.city.rotation(), 0);
    assert_eq!(
        cells.map(|(x, y)| state.city.tile_bits_at(x, y).unwrap().raw_bits()),
        [0xf0, 0xf0, 0xf0, 0xe0, 0xf0],
    );
    state.rotate_view(1);
    assert_eq!(state.city.pe_w_city_rotation, 0);
    assert_eq!(state.city.rotation(), 1);

    city.saved_rotation = 0;
    city.pe_w_city_rotation = 4;
    city.save(&path).unwrap();
    state.load_city(&path, (800, 600), 1.0).unwrap();
    assert_eq!(state.city.pe_w_city_rotation, 4);
    assert_eq!(
        cells.map(|(x, y)| state.city.tile_bits_at(x, y).unwrap().raw_bits()),
        [0xe0; 5],
    );
    assert_eq!(state.city.pe_w_sewer_bonus(), 1);
    std::fs::remove_file(path).unwrap();
}

/// Replays the clock-derived startup streams from a private original-build
/// capture. The files are intentionally external to the public repository.
#[test]
#[ignore = "requires private Win95 live-capture files and original bootstrap city"]
fn win95_captured_clocks_city_open_through_phase13() {
    let _live_rng = LiveGrowthRng::enable();
    let capture = std::path::PathBuf::from(
        std::env::var_os("CITYGAME_WIN95_CAPTURE_DIR")
            .expect("CITYGAME_WIN95_CAPTURE_DIR must point to the private live capture"),
    );
    let bootstrap = std::path::PathBuf::from(
        std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY")
            .expect("CITYGAME_WIN95_BOOTSTRAP_CITY must point to an original bootstrap city"),
    );
    // PE GetTickCount returns captured from the view constructor and the
    // first document/newspaper initialization, respectively.
    let view_lfsr_tick = 0x1c25a_u32;
    let view_lcg_tick = 0x1c25a_u32;
    let newspaper_tick = 0x31869_u32;
    let mut view_ticks = [view_lfsr_tick, view_lcg_tick].into_iter();
    let (lfsr, game_rand) = super::super::runtime::win95_view_randoms_from_tick_source(|| {
        u64::from(view_ticks.next().expect("unexpected view tick read"))
    });
    let (tick_word, crt, tables) =
        super::super::runtime::win95_document_randoms_from_tick_source(|| {
            u64::from(newspaper_tick)
        });
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        bootstrap,
        Some(crt),
        Some(game_rand),
        Some(lfsr),
        Some(tables),
    )
    .unwrap();
    state.pe_document_tick_word = Some(tick_word);
    assert!(state.engine_fidelity.uses_win95());
    state
        .load_city(&capture.join("maxis-paused.sc2"), (800, 600), 1.0)
        .unwrap();
    assert_eq!(state.pe_document_tick_word, Some(newspaper_tick as i16));
    assert_eq!(
        (
            state.pe_process_crt.seed(),
            state.pe_process_lfsr.state(),
            state.pe_process_game_rand.state()
        ),
        (2_943_932_078, 49_755, 115_291)
    );

    let date = state.city.saved_date().unwrap();
    state.handle_simulation_day_advanced(date.year, date.month, 13);
    assert_eq!(state.city.city_age_days(), Some(28_238));
    assert_eq!(
        (
            state.pe_process_crt.seed(),
            state.pe_process_lfsr.state(),
            state.pe_process_game_rand.state()
        ),
        (1_052_788_239, 43_836, 115_291)
    );
    for (name, side) in [
        ("altm", 128),
        ("xter", 128),
        ("xbld", 128),
        ("xzon", 128),
        ("xund", 128),
        ("xtxt", 128),
        ("xbit", 128),
        ("xtrf", 64),
        ("xplt", 64),
        ("xval", 64),
        ("xcrm", 64),
        ("xplc", 32),
        ("xfir", 32),
        ("xpop", 32),
        ("xrog", 32),
    ] {
        let live = std::fs::read(capture.join(format!("tick-phase-return/{name}-x-major.bin")))
            .expect("missing native plane capture");
        for x in 0..side {
            for y in 0..side {
                let scale = 128 / side;
                let (tile_x, tile_y) = (x * scale, y * scale);
                let actual: u16 = match name {
                    "altm" => {
                        let v = state.city.altm_at(tile_x, tile_y);
                        u16::from(v.land)
                            | (u16::from(v.water) << 5)
                            | (u16::from(v.tunnel_levels) << 10)
                    }
                    "xter" => state.city.terrain_at(tile_x, tile_y).into(),
                    "xbld" => state.city.building_at(tile_x, tile_y).into(),
                    "xzon" => state
                        .city
                        .zone_at(tile_x, tile_y)
                        .unwrap()
                        .raw_bits()
                        .into(),
                    "xund" => state.city.underground_at(tile_x, tile_y).into(),
                    "xtxt" => state.city.text_overlay_at(tile_x, tile_y).unwrap().into(),
                    "xbit" => state
                        .city
                        .tile_bits_at(tile_x, tile_y)
                        .unwrap()
                        .raw_bits()
                        .into(),
                    "xtrf" => state
                        .city
                        .traffic_density_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xplt" => state
                        .city
                        .pollution_density_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xval" => state.city.land_value_at(tile_x, tile_y).unwrap().into(),
                    "xcrm" => state.city.crime_density_at(tile_x, tile_y).unwrap().into(),
                    "xplc" => state
                        .city
                        .police_coverage_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xfir" => state.city.fire_coverage_at(tile_x, tile_y).unwrap().into(),
                    "xpop" => state
                        .city
                        .population_density_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xrog" => state.city.rate_of_growth_at(tile_x, tile_y).unwrap().into(),
                    _ => unreachable!(),
                };
                let index = x * side + y;
                let expected = if name == "altm" {
                    u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                } else {
                    u16::from(live[index])
                };
                assert_eq!(actual, expected, "{name} at ({x}, {y})");
            }
        }
    }
}

/// Fresh-launch Win95 clock inputs through a reopened city's first calendar dispatch.
#[test]
#[ignore = "requires private Win95 phase-one capture and original bootstrap city"]
fn win95_captured_clocks_reopened_city_through_phase_one() {
    let _live_rng = LiveGrowthRng::enable();
    let capture = PathBuf::from(std::env::var_os("CITYGAME_WIN95_PHASE_ONE_CAPTURE").unwrap());
    let bootstrap = PathBuf::from(std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY").unwrap());
    // These are the three GetTickCount returns in the original fresh launch.
    let mut ticks = [0x58cd0_u64, 0x58cd0].into_iter();
    let (lfsr, game) =
        super::super::runtime::win95_view_randoms_from_tick_source(|| ticks.next().unwrap());
    let (tick, crt, tables) =
        super::super::runtime::win95_document_randoms_from_tick_source(|| 0x69593);
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        bootstrap,
        Some(crt),
        Some(game),
        Some(lfsr),
        Some(tables),
    )
    .unwrap();
    state.pe_document_tick_word = Some(tick);
    state
        .load_city(
            &capture.join("../crt-editor-live-20261115/clockhard.sc2"),
            (1280, 800),
            1.0,
        )
        .unwrap();
    let check_rng = |state: &ViewerState, boundary: &str| {
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(capture.join(boundary).join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            u64::from(state.pe_process_crt.seed()),
            manifest["crt_tls_candidates"][0]["holdrand"]
                .as_u64()
                .unwrap(),
            "{boundary} CRT"
        );
        assert_eq!(
            u64::from(state.pe_process_lfsr.state()),
            manifest["lfsr_raw"].as_u64().unwrap(),
            "{boundary} LFSR"
        );
        assert_eq!(
            u64::from(state.pe_process_game_rand.state()),
            manifest["game_lcg_raw"].as_u64().unwrap(),
            "{boundary} game RNG"
        );
        let live_actors = std::fs::read(capture.join(boundary).join("xthg-records.bin"))
            .expect("missing native XTHG capture");
        let actors = crate::win95_overlay::encode_xthg_records(
            state
                .city
                .traffic_overlays()
                .expect("missing CityGame XTHG"),
        )
        .unwrap();
        assert_eq!(actors, live_actors, "{boundary} XTHG");
    };
    let check_planes = |state: &ViewerState, boundary: &str| {
        for (name, side) in [
            ("altm", 128),
            ("xter", 128),
            ("xbld", 128),
            ("xzon", 128),
            ("xund", 128),
            ("xtxt", 128),
            ("xbit", 128),
            ("xtrf", 64),
            ("xplt", 64),
            ("xval", 64),
            ("xcrm", 64),
            ("xplc", 32),
            ("xfir", 32),
            ("xpop", 32),
            ("xrog", 32),
        ] {
            let live = std::fs::read(capture.join(boundary).join(format!("{name}-x-major.bin")))
                .expect("missing native plane capture");
            for x in 0..side {
                for y in 0..side {
                    let scale = 128 / side;
                    let (tile_x, tile_y) = (x * scale, y * scale);
                    let actual: u16 = match name {
                        "altm" => {
                            let v = state.city.altm_at(tile_x, tile_y);
                            u16::from(v.land)
                                | (u16::from(v.water) << 5)
                                | (u16::from(v.tunnel_levels) << 10)
                        }
                        "xter" => state.city.terrain_at(tile_x, tile_y).into(),
                        "xbld" => state.city.building_at(tile_x, tile_y).into(),
                        "xzon" => state
                            .city
                            .zone_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xund" => state.city.underground_at(tile_x, tile_y).into(),
                        "xtxt" => state.city.text_overlay_at(tile_x, tile_y).unwrap().into(),
                        "xbit" => state
                            .city
                            .tile_bits_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xtrf" => state
                            .city
                            .traffic_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xplt" => state
                            .city
                            .pollution_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xval" => state.city.land_value_at(tile_x, tile_y).unwrap().into(),
                        "xcrm" => state.city.crime_density_at(tile_x, tile_y).unwrap().into(),
                        "xplc" => state
                            .city
                            .police_coverage_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xfir" => state.city.fire_coverage_at(tile_x, tile_y).unwrap().into(),
                        "xpop" => state
                            .city
                            .population_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xrog" => state.city.rate_of_growth_at(tile_x, tile_y).unwrap().into(),
                        _ => unreachable!(),
                    };
                    let index = x * side + y;
                    let expected = if name == "altm" {
                        u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                    } else {
                        u16::from(live[index])
                    };
                    assert_eq!(actual, expected, "{boundary} {name} at ({x}, {y})");
                }
            }
        }
    };
    check_rng(&state, "entry2-state");
    check_planes(&state, "entry2-state");
    assert_eq!(state.city.city_age_days(), Some(0));
    let date = state.city.saved_date().unwrap();
    state.handle_simulation_day_advanced(date.year, date.month, 1);
    assert_eq!(state.city.city_age_days(), Some(1));
    check_rng(&state, "return2-state");
    check_planes(&state, "return2-state");
}

/// Fresh-launch Win95 clock inputs through a mature city's phase-13 calendar dispatch.
#[test]
#[ignore = "requires private Win95 phase-13 capture and original bootstrap city"]
fn win95_captured_clocks_mature_city_through_phase_13() {
    let _live_rng = LiveGrowthRng::enable();
    let capture = PathBuf::from(std::env::var_os("CITYGAME_WIN95_PHASE13_CAPTURE").unwrap());
    let bootstrap = PathBuf::from(std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY").unwrap());
    // These are the three GetTickCount returns in the original fresh launch.
    let mut ticks = [0x2ef7c_u64, 0x2ef7c].into_iter();
    let (lfsr, game) =
        super::super::runtime::win95_view_randoms_from_tick_source(|| ticks.next().unwrap());
    let (tick, crt, tables) =
        super::super::runtime::win95_document_randoms_from_tick_source(|| 0x39e58);
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        bootstrap,
        Some(crt),
        Some(game),
        Some(lfsr),
        Some(tables),
    )
    .unwrap();
    state.pe_document_tick_word = Some(tick);
    state
        .load_city(&capture.join("maxis-paused.sc2"), (1280, 800), 1.0)
        .unwrap();
    let check_rng = |state: &ViewerState, boundary: &str| {
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(capture.join(boundary).join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            u64::from(state.pe_process_crt.seed()),
            manifest["crt_tls_candidates"][0]["holdrand"]
                .as_u64()
                .unwrap(),
            "{boundary} CRT"
        );
        assert_eq!(
            u64::from(state.pe_process_lfsr.state()),
            manifest["lfsr_raw"].as_u64().unwrap(),
            "{boundary} LFSR"
        );
        assert_eq!(
            u64::from(state.pe_process_game_rand.state()),
            manifest["game_lcg_raw"].as_u64().unwrap(),
            "{boundary} game RNG"
        );
        assert_eq!(
            u64::from(state.city.city_age_days().unwrap()),
            manifest["city_days"].as_u64().unwrap(),
            "{boundary} age"
        );
        assert_eq!(
            u64::from(state.city.pe_dw_city_population()),
            manifest["city_population"].as_u64().unwrap(),
            "{boundary} population"
        );
        assert_eq!(
            u64::from(state.city.pe_w_calendar_month_mod()),
            manifest["calendar_month_mod_word"].as_u64().unwrap(),
            "{boundary} calendar modulo"
        );
        assert_eq!(
            u64::from(state.city.pe_w_local_fame_base()),
            manifest["local_fame_base_word"].as_u64().unwrap(),
            "{boundary} fame base"
        );
        let live_actors = std::fs::read(capture.join(boundary).join("xthg-records.bin"))
            .expect("missing native XTHG capture");
        let actors = crate::win95_overlay::encode_xthg_records(
            state
                .city
                .traffic_overlays()
                .expect("missing CityGame XTHG"),
        )
        .unwrap();
        assert_eq!(actors, live_actors, "{boundary} XTHG");
        let live_xmic = std::fs::read(capture.join(boundary).join("xmic-records.bin"))
            .expect("missing native XMIC capture");
        let mut xmic = Vec::with_capacity(1200);
        for index in 0..150 {
            let record = state
                .city
                .microsim_at(index)
                .expect("missing CityGame XMIC row");
            xmic.extend([record.tile_id, record.stat0]);
            for value in [record.stat1, record.stat2, record.stat3] {
                xmic.extend(value.to_le_bytes());
            }
        }
        assert_eq!(xmic, live_xmic, "{boundary} XMIC");
    };
    let check_planes = |state: &ViewerState, boundary: &str| {
        for (name, side) in [
            ("altm", 128),
            ("xter", 128),
            ("xbld", 128),
            ("xzon", 128),
            ("xund", 128),
            ("xtxt", 128),
            ("xbit", 128),
            ("xtrf", 64),
            ("xplt", 64),
            ("xval", 64),
            ("xcrm", 64),
            ("xplc", 32),
            ("xfir", 32),
            ("xpop", 32),
            ("xrog", 32),
        ] {
            let live = std::fs::read(capture.join(boundary).join(format!("{name}-x-major.bin")))
                .expect("missing native plane capture");
            for x in 0..side {
                for y in 0..side {
                    let scale = 128 / side;
                    let (tile_x, tile_y) = (x * scale, y * scale);
                    let actual: u16 = match name {
                        "altm" => {
                            let v = state.city.altm_at(tile_x, tile_y);
                            u16::from(v.land)
                                | (u16::from(v.water) << 5)
                                | (u16::from(v.tunnel_levels) << 10)
                        }
                        "xter" => state.city.terrain_at(tile_x, tile_y).into(),
                        "xbld" => state.city.building_at(tile_x, tile_y).into(),
                        "xzon" => state
                            .city
                            .zone_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xund" => state.city.underground_at(tile_x, tile_y).into(),
                        "xtxt" => state.city.text_overlay_at(tile_x, tile_y).unwrap().into(),
                        "xbit" => state
                            .city
                            .tile_bits_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xtrf" => state
                            .city
                            .traffic_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xplt" => state
                            .city
                            .pollution_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xval" => state.city.land_value_at(tile_x, tile_y).unwrap().into(),
                        "xcrm" => state.city.crime_density_at(tile_x, tile_y).unwrap().into(),
                        "xplc" => state
                            .city
                            .police_coverage_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xfir" => state.city.fire_coverage_at(tile_x, tile_y).unwrap().into(),
                        "xpop" => state
                            .city
                            .population_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xrog" => state.city.rate_of_growth_at(tile_x, tile_y).unwrap().into(),
                        _ => unreachable!(),
                    };
                    let index = x * side + y;
                    let expected = if name == "altm" {
                        u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                    } else {
                        u16::from(live[index])
                    };
                    assert_eq!(actual, expected, "{boundary} {name} at ({x}, {y})");
                }
            }
        }
    };
    check_rng(&state, "entry-state");
    check_planes(&state, "entry-state");
    assert_eq!(state.city.city_age_days(), Some(28_237));
    let date = state.city.saved_date().unwrap();
    state.handle_simulation_day_advanced(date.year, date.month, 13);
    assert_eq!(state.city.city_age_days(), Some(28_238));
    check_rng(&state, "return-state");
    check_planes(&state, "return-state");
}

#[test]
fn win95_year_end_latch_survives_refresh_and_follows_monthly_workers() {
    let city = City::new_blank("/tmp/pe-year-end-latch.sc2", "Latch").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.city.set_misc_u32_at(MISC_YEAR_END_OFFSET, 0);
    state.city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        24,
    );
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    state.refresh_live_misc_current_values();
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(0));
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 0);
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(0));

    state.sim_calendar_day0_budget_and_date_at(1901, 12, 0);
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(1));
    state.game_clock = GameClock::new_with_day(1901, 12, 1);
    state.refresh_live_misc_current_values();
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(1));
    state.sim_calendar_day0_budget_and_date_at(1902, 1, 0);
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(0));
}

fn assert_win95_capture_planes_match(
    state: &ViewerState,
    capture: &std::path::Path,
    boundary: &str,
) {
    for (name, side) in [
        ("altm", 128),
        ("xter", 128),
        ("xbld", 128),
        ("xzon", 128),
        ("xund", 128),
        ("xtxt", 128),
        ("xbit", 128),
        ("xtrf", 64),
        ("xplt", 64),
        ("xval", 64),
        ("xcrm", 64),
        ("xplc", 32),
        ("xfir", 32),
        ("xpop", 32),
        ("xrog", 32),
    ] {
        let live = std::fs::read(capture.join(boundary).join(format!("{name}-x-major.bin")))
            .expect("missing native plane capture");
        for x in 0..side {
            for y in 0..side {
                let scale = 128 / side;
                let (tile_x, tile_y) = (x * scale, y * scale);
                let actual: u16 = match name {
                    "altm" => {
                        let v = state.city.altm_at(tile_x, tile_y);
                        u16::from(v.land)
                            | (u16::from(v.water) << 5)
                            | (u16::from(v.tunnel_levels) << 10)
                    }
                    "xter" => state.city.terrain_at(tile_x, tile_y).into(),
                    "xbld" => state.city.building_at(tile_x, tile_y).into(),
                    "xzon" => state
                        .city
                        .zone_at(tile_x, tile_y)
                        .unwrap()
                        .raw_bits()
                        .into(),
                    "xund" => state.city.underground_at(tile_x, tile_y).into(),
                    "xtxt" => state.city.text_overlay_at(tile_x, tile_y).unwrap().into(),
                    "xbit" => state
                        .city
                        .tile_bits_at(tile_x, tile_y)
                        .unwrap()
                        .raw_bits()
                        .into(),
                    "xtrf" => state
                        .city
                        .traffic_density_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xplt" => state
                        .city
                        .pollution_density_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xval" => state.city.land_value_at(tile_x, tile_y).unwrap().into(),
                    "xcrm" => state.city.crime_density_at(tile_x, tile_y).unwrap().into(),
                    "xplc" => state
                        .city
                        .police_coverage_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xfir" => state.city.fire_coverage_at(tile_x, tile_y).unwrap().into(),
                    "xpop" => state
                        .city
                        .population_density_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xrog" => state.city.rate_of_growth_at(tile_x, tile_y).unwrap().into(),
                    _ => unreachable!(),
                };
                let index = x * side + y;
                let expected = if name == "altm" {
                    u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                } else {
                    u16::from(live[index])
                };
                assert_eq!(actual, expected, "{boundary} {name} at ({x}, {y})");
            }
        }
    }
}

fn print_win95_capture_plane_differences(
    state: &ViewerState,
    capture: &std::path::Path,
    boundary: &str,
) {
    for (name, side) in [
        ("altm", 128),
        ("xter", 128),
        ("xbld", 128),
        ("xzon", 128),
        ("xund", 128),
        ("xtxt", 128),
        ("xbit", 128),
        ("xtrf", 64),
        ("xplt", 64),
        ("xval", 64),
        ("xcrm", 64),
        ("xplc", 32),
        ("xfir", 32),
        ("xpop", 32),
        ("xrog", 32),
    ] {
        let live = std::fs::read(capture.join(boundary).join(format!("{name}-x-major.bin")))
            .expect("missing native plane capture");
        let mut differences = 0_usize;
        let mut first = None;
        for x in 0..side {
            for y in 0..side {
                let scale = 128 / side;
                let (tile_x, tile_y) = (x * scale, y * scale);
                let actual: u16 = match name {
                    "altm" => {
                        let v = state.city.altm_at(tile_x, tile_y);
                        u16::from(v.land)
                            | (u16::from(v.water) << 5)
                            | (u16::from(v.tunnel_levels) << 10)
                    }
                    "xter" => state.city.terrain_at(tile_x, tile_y).into(),
                    "xbld" => state.city.building_at(tile_x, tile_y).into(),
                    "xzon" => state
                        .city
                        .zone_at(tile_x, tile_y)
                        .unwrap()
                        .raw_bits()
                        .into(),
                    "xund" => state.city.underground_at(tile_x, tile_y).into(),
                    "xtxt" => state.city.text_overlay_at(tile_x, tile_y).unwrap().into(),
                    "xbit" => state
                        .city
                        .tile_bits_at(tile_x, tile_y)
                        .unwrap()
                        .raw_bits()
                        .into(),
                    "xtrf" => state
                        .city
                        .traffic_density_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xplt" => state
                        .city
                        .pollution_density_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xval" => state.city.land_value_at(tile_x, tile_y).unwrap().into(),
                    "xcrm" => state.city.crime_density_at(tile_x, tile_y).unwrap().into(),
                    "xplc" => state
                        .city
                        .police_coverage_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xfir" => state.city.fire_coverage_at(tile_x, tile_y).unwrap().into(),
                    "xpop" => state
                        .city
                        .population_density_at(tile_x, tile_y)
                        .unwrap()
                        .into(),
                    "xrog" => state.city.rate_of_growth_at(tile_x, tile_y).unwrap().into(),
                    _ => unreachable!(),
                };
                let index = x * side + y;
                let expected = if name == "altm" {
                    u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                } else {
                    u16::from(live[index])
                };
                if actual != expected {
                    differences += 1;
                    first.get_or_insert((x, y, actual, expected));
                }
            }
        }
        println!("Bayview {boundary} {name}: {differences} differing cells; first={first:?}");
    }
}

fn test_fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

/// Reaches the annual Statue interval through CityGame's regular Cheetah frame
/// scheduler, including timed actor ticks. It starts at a captured post-open
/// boundary and is not an unseeded launch-to-year replay.
#[test]
#[ignore = "requires private Win95 Bayview annual capture and catalog city"]
fn win95_bayview_cheetah_checkpoint_exposes_annual_statue_rng_divergence() {
    let capture = PathBuf::from(std::env::var_os("CITYGAME_WIN95_BAYVIEW_ANNUAL_CAPTURE").unwrap());
    let city_path = PathBuf::from(std::env::var_os("CITYGAME_WIN95_BAYVIEW_CITY").unwrap());
    let boundary = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(capture.join(name).join("manifest.json")).unwrap())
            .unwrap()
    };
    let opened = boundary("day41661");
    let after_year = boundary("day41788");
    assert_eq!(
        opened["image_sha256"],
        "f7a51b6244328bddcc85c2db4d205716b504eb9b6cc51b2f1556adaece96e5bb"
    );
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(std::fs::read(&city_path).unwrap())),
        "b2730e4fc6c8577d529d33ce75dd6d57c6217fb6b7e6442c870d13d127fd4678"
    );
    assert_eq!(opened["city_days"], 41_661);
    assert_eq!(after_year["city_days"], 41_788);

    let mut state = test_state(City::new_blank("unused.sc2", "Bootstrap").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.load_city(&city_path, (1280, 800), 1.0).unwrap();
    if let Some(data_dir) = std::env::var_os("CITYGAME_WIN95_DATA_DIR") {
        let data_dir = PathBuf::from(data_dir);
        state.pe_newspaper_data = Some(std::sync::Arc::new(
            crate::win95_newspaper::PeNewspaperData::load(
                &data_dir.join("data_usa.dat"),
                &data_dir.join("data_usa.idx"),
            )
            .expect("load Win95 DATA_USA archive for newspaper paint"),
        ));
    }
    if let Ok(tick) = std::env::var("CITYGAME_WIN95_DOCUMENT_TICK_WORD") {
        state.pe_document_tick_word = Some(tick.parse().expect("parse document TickCount word"));
    }
    if state.city_info_dialog
        == Some(CityInfoDialog::Newspaper(NewspaperEdition::Subscription))
    {
        state.close_city_info_dialog();
    }
    assert_eq!(state.city.city_age_days(), Some(41_661));
    assert_eq!(
        u64::from(state.city.pe_dw_city_population()),
        opened["city_population"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(state.city.pe_w_calendar_month_mod()),
        opened["calendar_month_mod_word"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(state.city.pe_w_local_fame_base()),
        opened["local_fame_base_word"].as_u64().unwrap()
    );
    assert_win95_capture_planes_match(&state, &capture, "day41661");

    let native_open_xthg = std::fs::read(capture.join("day41661/xthg-records.bin")).unwrap();
    let citygame_open_xthg = crate::win95_overlay::encode_xthg_records(
        state.city.traffic_overlays().expect("Bayview XTHG"),
    )
    .unwrap();
    assert_eq!(citygame_open_xthg, native_open_xthg, "opened XTHG state");

    let native_open_xmic = std::fs::read(capture.join("day41661/xmic-records.bin")).unwrap();
    for slot in 0..150 {
        let row = state.city.microsim_at(slot).unwrap();
        let offset = slot * 8;
        assert_eq!(
            row.tile_id, native_open_xmic[offset],
            "open XMIC slot {slot} tile"
        );
        assert_eq!(
            row.stat0,
            native_open_xmic[offset + 1],
            "open XMIC slot {slot} stat0"
        );
        assert_eq!(
            row.stat1,
            u16::from_le_bytes([native_open_xmic[offset + 2], native_open_xmic[offset + 3]]),
            "open XMIC slot {slot} stat1"
        );
        assert_eq!(
            row.stat2,
            u16::from_le_bytes([native_open_xmic[offset + 4], native_open_xmic[offset + 5]]),
            "open XMIC slot {slot} stat2"
        );
        assert_eq!(
            row.stat3,
            u16::from_le_bytes([native_open_xmic[offset + 6], native_open_xmic[offset + 7]]),
            "open XMIC slot {slot} stat3"
        );
    }
    let statue_slot = (0..150)
        .find(|&slot| {
            let row = state.city.microsim_at(slot).unwrap();
            row.tile_id == 219 && row.stat1 == 2182
        })
        .expect("Bayview Statue XMIC row");
    assert_eq!(
        state.city.microsim_at(statue_slot).unwrap().stat2,
        33,
        "captured pre-annual Statue value"
    );

    // The native run supplies only its opened-city boundary streams here. From
    // this point CityGame owns dispatch and generates every subsequent draw.
    state.pe_process_crt = Win95CrtRand::new(
        opened["crt_tls_candidates"][0]["holdrand"]
            .as_u64()
            .unwrap() as u32,
    );
    state.pe_process_game_rand =
        Win95GameRand::new(opened["game_lcg_raw"].as_u64().unwrap() as u32);
    state.pe_process_lfsr = Win95Lfsr::new(opened["lfsr_raw"].as_u64().unwrap() as u16);
    state.traffic.set_speed(GameSpeed::Cheetah);

    let mut frame_count = 0_u32;
    const FRAME_ELAPSED: Duration = Duration::from_millis(20);
    while state.city.city_age_days().unwrap() < 41_788 {
        let frame_age_before = state.city.city_age_days().unwrap();
        let crt_before_frame = state.pe_process_crt.seed();
        let lfsr_before_frame = state.pe_process_lfsr.state();
        let game_before_frame = state.pe_process_game_rand.state();
        let dialog_before_frame = state.city_info_dialog;
        let advance = state.advance_frame_simulation(FRAME_ELAPSED);
        let lfsr_after_scheduler = state.pe_process_lfsr.state();
        let crt_after_scheduler = state.pe_process_crt.seed();
        let game_after_scheduler = state.pe_process_game_rand.state();
        if matches!(
            state.city_info_dialog,
            Some(CityInfoDialog::Newspaper(_))
        ) {
            // Match the actual presentation entrypoint: native PE newspaper
            // workers run on first paint, not merely when the modal opens.
            state.prepare_pe_newspaper_first_paint();
        }
        let crt_after_newspaper_paint = state.pe_process_crt.seed();
        let newspaper_paint_ready =
            state.pe_newspaper_data.is_some() && state.pe_document_tick_word.is_some();
        let dialog_after_scheduler = state.city_info_dialog;
        frame_count += 1;
        assert!(
            frame_count < 10_000,
            "did not reach Bayview target age: age={} mode={:?} dialog={:?} speed={:?}",
            state.city.city_age_days().unwrap(),
            state.mode,
            state.city_info_dialog,
            state.traffic.speed()
        );
        let closed_dialog = matches!(
            state.city_info_dialog,
            Some(CityInfoDialog::Budget)
                | Some(CityInfoDialog::Newspaper(NewspaperEdition::Subscription))
        );
        if closed_dialog {
            // Dismiss the unedited budget review and subscription notices as
            // the player did in the native run; neither action changes city
            // data. Keep all simulation and RNG dispatch inside the scheduler.
            state.close_city_info_dialog();
        }
        let crt_after_dialog = state.pe_process_crt.seed();
        let game_after_dialog = state.pe_process_game_rand.state();
        // Match the other per-frame Win95 runtime work after the scheduler:
        // the port traffic adapter and the ambient overlay adapter use the
        // same elapsed frame time, while the PE XTHG slots remain owned by
        // advance_frame_simulation's native-order actor pass.
        state
            .traffic
            .tick_traffic(&mut state.city, FRAME_ELAPSED.as_secs_f32());
        let lfsr_after_traffic_adapter = state.pe_process_lfsr.state();
        let crt_after_traffic_adapter = state.pe_process_crt.seed();
        state.tick_win95_overlay_sim_frame(FRAME_ELAPSED.as_secs_f32());
        let crt_after_overlay = state.pe_process_crt.seed();
        let game_after_overlay = state.pe_process_game_rand.state();
        if (41_661..=41_664).contains(&frame_age_before) {
            println!(
                "Bayview frame {} age={}->{} days={} dialog={:?}->{:?} closed={} paper-paint-ready={} paper-attempted={} CRT={:#010x}->{:#010x}->{:#010x}->{:#010x}->{:#010x}->{:#010x} LFSR={:#06x}->{:#06x}->{:#06x} game={:#010x}->{:#010x}->{:#010x}->{:#010x}",
                frame_count,
                frame_age_before,
                state.city.city_age_days().unwrap(),
                advance.days,
                dialog_before_frame,
                dialog_after_scheduler,
                closed_dialog,
                newspaper_paint_ready,
                state.pe_newspaper_paint_attempted,
                crt_before_frame,
                crt_after_scheduler,
                crt_after_newspaper_paint,
                crt_after_dialog,
                crt_after_traffic_adapter,
                crt_after_overlay,
                lfsr_before_frame,
                lfsr_after_scheduler,
                state.pe_process_lfsr.state(),
                game_before_frame,
                game_after_scheduler,
                game_after_dialog,
                game_after_overlay,
            );
        }
        if advance.days != 0 && state.city.city_age_days() == Some(41_662) {
            println!(
                "Bayview first-day LFSR scheduler={} traffic={} overlay={}",
                lfsr_after_scheduler,
                lfsr_after_traffic_adapter,
                state.pe_process_lfsr.state(),
            );
        }
        if advance.days != 0 {
            let actors = crate::win95_overlay::encode_xthg_records(
                state.city.traffic_overlays().expect("Bayview XTHG daily"),
            )
            .unwrap();
            let mut xtxt = Vec::with_capacity(128 * 128);
            for x in 0..128 {
                for y in 0..128 {
                    xtxt.push(u8::try_from(state.city.text_overlay_at(x, y).unwrap()).unwrap());
                }
            }
            println!(
                "Bayview daily {} {} {} {} xtxt={:016x} xthg={:016x}",
                state.city.city_age_days().unwrap(),
                state.pe_process_crt.seed(),
                state.pe_process_lfsr.state(),
                state.pe_process_game_rand.state(),
                test_fnv1a64(&xtxt),
                test_fnv1a64(&actors),
            );
        }
    }

    let native_after_xmic = std::fs::read(capture.join("day41788/xmic-records.bin")).unwrap();
    let zoo_slot = (0..150)
        .find(|&slot| native_after_xmic[slot * 8] == 218)
        .expect("native Bayview Zoo XMIC row");
    let zoo = state
        .city
        .microsim_at(zoo_slot)
        .expect("CityGame Bayview Zoo XMIC row");
    let zoo_offset = zoo_slot * 8;
    assert_eq!(
        (zoo.tile_id, zoo.stat0, zoo.stat1, zoo.stat2, zoo.stat3),
        (
            native_after_xmic[zoo_offset],
            native_after_xmic[zoo_offset + 1],
            u16::from_le_bytes([
                native_after_xmic[zoo_offset + 2],
                native_after_xmic[zoo_offset + 3],
            ]),
            u16::from_le_bytes([
                native_after_xmic[zoo_offset + 4],
                native_after_xmic[zoo_offset + 5],
            ]),
            u16::from_le_bytes([
                native_after_xmic[zoo_offset + 6],
                native_after_xmic[zoo_offset + 7],
            ]),
        ),
        "Bayview annual Zoo XMIC row matches the native post-year capture"
    );
    assert_eq!(
        state.pe_process_game_rand.state(),
        after_year["game_lcg_raw"].as_u64().unwrap() as u32,
        "Bayview annual Zoo draws match the native game-LCG endpoint"
    );
    let xmic_differing_slots: Vec<_> = (0..150)
        .filter(|&slot| {
            let row = state.city.microsim_at(slot).unwrap();
            let offset = slot * 8;
            row.tile_id != native_after_xmic[offset]
                || row.stat0 != native_after_xmic[offset + 1]
                || row.stat1
                    != u16::from_le_bytes([
                        native_after_xmic[offset + 2],
                        native_after_xmic[offset + 3],
                    ])
                || row.stat2
                    != u16::from_le_bytes([
                        native_after_xmic[offset + 4],
                        native_after_xmic[offset + 5],
                    ])
                || row.stat3
                    != u16::from_le_bytes([
                        native_after_xmic[offset + 6],
                        native_after_xmic[offset + 7],
                    ])
        })
        .collect();
    println!("Bayview day41788 XMIC differing slots={xmic_differing_slots:?}");
    for slot in &xmic_differing_slots {
        let row = state.city.microsim_at(*slot).unwrap();
        let offset = slot * 8;
        let native_row = (
            native_after_xmic[offset],
            native_after_xmic[offset + 1],
            u16::from_le_bytes([
                native_after_xmic[offset + 2],
                native_after_xmic[offset + 3],
            ]),
            u16::from_le_bytes([
                native_after_xmic[offset + 4],
                native_after_xmic[offset + 5],
            ]),
            u16::from_le_bytes([
                native_after_xmic[offset + 6],
                native_after_xmic[offset + 7],
            ]),
        );
        println!(
            "Bayview day41788 XMIC slot={slot} CityGame=({}, {}, {}, {}, {}) native={native_row:?}",
            row.tile_id, row.stat0, row.stat1, row.stat2, row.stat3,
        );
    }
    print_win95_capture_plane_differences(&state, &capture, "day41788");
    let actual = state.city.microsim_at(statue_slot).unwrap();
    let offset = statue_slot * 8;
    let native_after_xthg = std::fs::read(capture.join("day41788/xthg-records.bin")).unwrap();
    let citygame_after_xthg = crate::win95_overlay::encode_xthg_records(
        state.city.traffic_overlays().expect("Bayview XTHG after Cheetah run"),
    )
    .unwrap();
    let actor_slots_differ: Vec<usize> = (0..40)
        .filter(|slot| {
            let begin = slot * 12;
            citygame_after_xthg[begin..begin + 12] != native_after_xthg[begin..begin + 12]
        })
        .collect();
    for slot in &actor_slots_differ {
        let begin = slot * 12;
        let citygame_row = citygame_after_xthg[begin..begin + 12]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let native_row = native_after_xthg[begin..begin + 12]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        println!("XTHG slot {slot}: CityGame={citygame_row} native={native_row}");
    }
    println!(
        "Bayview Cheetah return age={} frames={} Statue={:?} CRT={:#010x} game={:#010x} LFSR={:#06x} XTHG-differing-slots={:?}",
        state.city.city_age_days().unwrap(),
        frame_count,
        actual,
        state.pe_process_crt.seed(),
        state.pe_process_game_rand.state(),
        state.pe_process_lfsr.state(),
        actor_slots_differ
    );
    let native_stat2 =
        u16::from_le_bytes([native_after_xmic[offset + 4], native_after_xmic[offset + 5]]);
    assert_eq!(actual.stat2, 5, "current CityGame Cheetah checkpoint result");
    assert_eq!(native_stat2, 32, "native post-boundary Statue result");
    assert_ne!(
        actual.stat2, native_stat2,
        "known unresolved annual RNG divergence"
    );
    println!(
        "Bayview Cheetah mismatch CityGame stat2={} native stat2={} native CRT={:#010x} game={:#010x} LFSR={:#06x}",
        actual.stat2,
        native_stat2,
        after_year["crt_tls_candidates"][0]["holdrand"].as_u64().unwrap(),
        after_year["game_lcg_raw"].as_u64().unwrap(),
        after_year["lfsr_raw"].as_u64().unwrap()
    );
    assert_eq!(actual.tile_id, native_after_xmic[offset]);
    assert_eq!(actual.stat0, native_after_xmic[offset + 1]);
    assert_eq!(
        actual.stat1,
        u16::from_le_bytes([native_after_xmic[offset + 2], native_after_xmic[offset + 3]])
    );
    assert_eq!(
        actual.stat3,
        u16::from_le_bytes([native_after_xmic[offset + 6], native_after_xmic[offset + 7]])
    );
}

/// Replays exactly one native PE Things pass from Bayview's opened-city
/// actor table and captured process RNG values. This isolates actor behavior
/// from the wall-clock/idle cadence that is still under investigation.
#[test]
#[ignore = "requires the private Bayview Win95 catalog city"]
fn win95_bayview_single_things_pass_matches_native_actor_and_rng_endpoint() {
    let city_path = PathBuf::from(std::env::var_os("CITYGAME_WIN95_BAYVIEW_CITY").unwrap());
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(std::fs::read(&city_path).unwrap())),
        "b2730e4fc6c8577d529d33ce75dd6d57c6217fb6b7e6442c870d13d127fd4678"
    );
    let mut state = test_state(City::new_blank("unused.sc2", "Bootstrap").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.load_city(&city_path, (1280, 800), 1.0).unwrap();
    if matches!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Subscription))
    ) {
        state.close_city_info_dialog();
    }
    let before = crate::win95_overlay::encode_xthg_records(
        state.city.traffic_overlays().expect("Bayview XTHG"),
    )
    .unwrap();

    // Captured day-41661 boundary values from the original Bayview run.
    state.pe_process_crt = Win95CrtRand::new(2_267_257_818);
    state.pe_process_lfsr = Win95Lfsr::new(3_494);
    state.pe_process_game_rand = Win95GameRand::new(3_730_023_020);
    state.tick_pe_disaster_xthg_actors_in_slot_order();

    let actors = crate::win95_overlay::encode_xthg_records(
        state.city.traffic_overlays().expect("Bayview XTHG"),
    )
    .unwrap();
    let changed_slots: Vec<_> = before
        .chunks_exact(12)
        .zip(actors.chunks_exact(12))
        .enumerate()
        .filter_map(|(slot, (old, new))| (old != new).then_some(slot))
        .collect();
    assert_eq!(changed_slots, [1, 2, 3, 4, 5, 7]);
    let expected_rows = [
        (1, "0300040c51010909427d0002"),
        (2, "02020241190a08084e190001"),
        (3, "0902001f1900040414140001"),
        (4, "01000202400e08081414009f"),
        (5, "09010043500004047d410014"),
        (7, "0901001b2e000404141400d0"),
    ];
    for (slot, expected_hex) in expected_rows {
        let begin = slot * 12;
        let actual_hex = actors[begin..begin + 12]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual_hex, expected_hex, "XTHG slot {slot}");
    }
    assert_eq!(state.pe_process_crt.seed(), 894_329_687);
    assert_eq!(state.pe_process_lfsr.state(), 44_853);
    assert_eq!(state.pe_process_game_rand.state(), 3_730_023_020);
}

/// Fresh-launch Win95 clock inputs through a date-adjusted city's January phase.
#[test]
#[ignore = "requires private Win95 annual capture and original bootstrap city"]
fn win95_captured_clocks_january_without_december_gate() {
    let _live_rng = LiveGrowthRng::enable();
    let capture = PathBuf::from(std::env::var_os("CITYGAME_WIN95_ANNUAL_CAPTURE").unwrap());
    let bootstrap = PathBuf::from(std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY").unwrap());
    // These are the three GetTickCount returns in the original fresh launch.
    let mut ticks = [0xa9be_u64, 0xa9be].into_iter();
    let (lfsr, game) =
        super::super::runtime::win95_view_randoms_from_tick_source(|| ticks.next().unwrap());
    let (tick, crt, tables) =
        super::super::runtime::win95_document_randoms_from_tick_source(|| 0x1828d);
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        bootstrap,
        Some(crt),
        Some(game),
        Some(lfsr),
        Some(tables),
    )
    .unwrap();
    state.pe_document_tick_word = Some(tick);
    state
        .load_city(&capture.join("annual.sc2"), (1280, 800), 1.0)
        .unwrap();
    let check_rng = |state: &ViewerState, boundary: &str| {
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(capture.join(boundary).join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            u64::from(state.pe_process_crt.seed()),
            manifest["crt_tls_candidates"][0]["holdrand"]
                .as_u64()
                .unwrap(),
            "{boundary} CRT"
        );
        assert_eq!(
            u64::from(state.pe_process_lfsr.state()),
            manifest["lfsr_raw"].as_u64().unwrap(),
            "{boundary} LFSR"
        );
        assert_eq!(
            u64::from(state.pe_process_game_rand.state()),
            manifest["game_lcg_raw"].as_u64().unwrap(),
            "{boundary} game RNG"
        );
        let live_actors = std::fs::read(capture.join(boundary).join("xthg-records.bin"))
            .expect("missing native XTHG capture");
        let actors = crate::win95_overlay::encode_xthg_records(
            state
                .city
                .traffic_overlays()
                .expect("missing CityGame XTHG"),
        )
        .unwrap();
        assert_eq!(actors, live_actors, "{boundary} XTHG");
        let live_xmic = std::fs::read(capture.join(boundary).join("xmic-records.bin"))
            .expect("missing native XMIC capture");
        let mut xmic = Vec::with_capacity(1200);
        for index in 0..150 {
            let record = state
                .city
                .microsim_at(index)
                .expect("missing CityGame XMIC row");
            xmic.extend([record.tile_id, record.stat0]);
            for value in [record.stat1, record.stat2, record.stat3] {
                xmic.extend(value.to_le_bytes());
            }
        }
        assert_eq!(xmic, live_xmic, "{boundary} XMIC");
    };
    let check_planes = |state: &ViewerState, boundary: &str| {
        for (name, side) in [
            ("altm", 128),
            ("xter", 128),
            ("xbld", 128),
            ("xzon", 128),
            ("xund", 128),
            ("xtxt", 128),
            ("xbit", 128),
            ("xtrf", 64),
            ("xplt", 64),
            ("xval", 64),
            ("xcrm", 64),
            ("xplc", 32),
            ("xfir", 32),
            ("xpop", 32),
            ("xrog", 32),
        ] {
            let live = std::fs::read(capture.join(boundary).join(format!("{name}-x-major.bin")))
                .expect("missing native plane capture");
            for x in 0..side {
                for y in 0..side {
                    let scale = 128 / side;
                    let (tile_x, tile_y) = (x * scale, y * scale);
                    let actual: u16 = match name {
                        "altm" => {
                            let v = state.city.altm_at(tile_x, tile_y);
                            u16::from(v.land)
                                | (u16::from(v.water) << 5)
                                | (u16::from(v.tunnel_levels) << 10)
                        }
                        "xter" => state.city.terrain_at(tile_x, tile_y).into(),
                        "xbld" => state.city.building_at(tile_x, tile_y).into(),
                        "xzon" => state
                            .city
                            .zone_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xund" => state.city.underground_at(tile_x, tile_y).into(),
                        "xtxt" => state.city.text_overlay_at(tile_x, tile_y).unwrap().into(),
                        "xbit" => state
                            .city
                            .tile_bits_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xtrf" => state
                            .city
                            .traffic_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xplt" => state
                            .city
                            .pollution_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xval" => state.city.land_value_at(tile_x, tile_y).unwrap().into(),
                        "xcrm" => state.city.crime_density_at(tile_x, tile_y).unwrap().into(),
                        "xplc" => state
                            .city
                            .police_coverage_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xfir" => state.city.fire_coverage_at(tile_x, tile_y).unwrap().into(),
                        "xpop" => state
                            .city
                            .population_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xrog" => state.city.rate_of_growth_at(tile_x, tile_y).unwrap().into(),
                        _ => unreachable!(),
                    };
                    let index = x * side + y;
                    let expected = if name == "altm" {
                        u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                    } else {
                        u16::from(live[index])
                    };
                    assert_eq!(actual, expected, "{boundary} {name} at ({x}, {y})");
                }
            }
        }
    };
    check_rng(&state, "entry-state");
    check_planes(&state, "entry-state");
    assert_eq!(state.city.city_age_days(), Some(28_499));
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(0));
    state.game_clock = state.game_clock.wrapping_add_days(1);
    assert_eq!(
        (
            state.game_clock.year(),
            state.game_clock.month(),
            state.game_clock.day()
        ),
        (1995, 1, 0)
    );
    state.handle_simulation_day_advanced(1995, 1, 0);
    assert_eq!(state.city.city_age_days(), Some(28_500));
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(0));
    check_rng(&state, "return-state");
    check_planes(&state, "return-state");
}

/// Fresh-launch Win95 clock inputs through a date-adjusted city's January phase.
#[test]
#[ignore = "requires private Win95 annual capture and original bootstrap city"]
fn win95_captured_clocks_january_with_december_gate() {
    let _live_rng = LiveGrowthRng::enable();
    let capture = PathBuf::from(std::env::var_os("CITYGAME_WIN95_ANNUAL_CAPTURE").unwrap());
    let bootstrap = PathBuf::from(std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY").unwrap());
    // These are the three GetTickCount returns in the original fresh launch.
    let mut ticks = [0x53290_u64, 0x53290].into_iter();
    let (lfsr, game) =
        super::super::runtime::win95_view_randoms_from_tick_source(|| ticks.next().unwrap());
    let (tick, crt, tables) =
        super::super::runtime::win95_document_randoms_from_tick_source(|| 0x8bd87);
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        bootstrap,
        Some(crt),
        Some(game),
        Some(lfsr),
        Some(tables),
    )
    .unwrap();
    state.pe_document_tick_word = Some(tick);
    state
        .load_city(&capture.join("armed.sc2"), (1280, 800), 1.0)
        .unwrap();
    let check_rng = |state: &ViewerState, boundary: &str| {
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(capture.join(boundary).join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            u64::from(state.pe_process_crt.seed()),
            manifest["crt_tls_candidates"][0]["holdrand"]
                .as_u64()
                .unwrap(),
            "{boundary} CRT"
        );
        assert_eq!(
            u64::from(state.pe_process_lfsr.state()),
            manifest["lfsr_raw"].as_u64().unwrap(),
            "{boundary} LFSR"
        );
        assert_eq!(
            u64::from(state.pe_process_game_rand.state()),
            manifest["game_lcg_raw"].as_u64().unwrap(),
            "{boundary} game RNG"
        );
        let live_actors = std::fs::read(capture.join(boundary).join("xthg-records.bin"))
            .expect("missing native XTHG capture");
        let actors = crate::win95_overlay::encode_xthg_records(
            state
                .city
                .traffic_overlays()
                .expect("missing CityGame XTHG"),
        )
        .unwrap();
        assert_eq!(actors, live_actors, "{boundary} XTHG");
        let live_xmic = std::fs::read(capture.join(boundary).join("xmic-records.bin"))
            .expect("missing native XMIC capture");
        let mut xmic = Vec::with_capacity(1200);
        for index in 0..150 {
            let record = state
                .city
                .microsim_at(index)
                .expect("missing CityGame XMIC row");
            xmic.extend([record.tile_id, record.stat0]);
            for value in [record.stat1, record.stat2, record.stat3] {
                xmic.extend(value.to_le_bytes());
            }
        }
        assert_eq!(xmic, live_xmic, "{boundary} XMIC");
    };
    let check_planes = |state: &ViewerState, boundary: &str| {
        for (name, side) in [
            ("altm", 128),
            ("xter", 128),
            ("xbld", 128),
            ("xzon", 128),
            ("xund", 128),
            ("xtxt", 128),
            ("xbit", 128),
            ("xtrf", 64),
            ("xplt", 64),
            ("xval", 64),
            ("xcrm", 64),
            ("xplc", 32),
            ("xfir", 32),
            ("xpop", 32),
            ("xrog", 32),
        ] {
            let live = std::fs::read(capture.join(boundary).join(format!("{name}-x-major.bin")))
                .expect("missing native plane capture");
            for x in 0..side {
                for y in 0..side {
                    let scale = 128 / side;
                    let (tile_x, tile_y) = (x * scale, y * scale);
                    let actual: u16 = match name {
                        "altm" => {
                            let v = state.city.altm_at(tile_x, tile_y);
                            u16::from(v.land)
                                | (u16::from(v.water) << 5)
                                | (u16::from(v.tunnel_levels) << 10)
                        }
                        "xter" => state.city.terrain_at(tile_x, tile_y).into(),
                        "xbld" => state.city.building_at(tile_x, tile_y).into(),
                        "xzon" => state
                            .city
                            .zone_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xund" => state.city.underground_at(tile_x, tile_y).into(),
                        "xtxt" => state.city.text_overlay_at(tile_x, tile_y).unwrap().into(),
                        "xbit" => state
                            .city
                            .tile_bits_at(tile_x, tile_y)
                            .unwrap()
                            .raw_bits()
                            .into(),
                        "xtrf" => state
                            .city
                            .traffic_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xplt" => state
                            .city
                            .pollution_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xval" => state.city.land_value_at(tile_x, tile_y).unwrap().into(),
                        "xcrm" => state.city.crime_density_at(tile_x, tile_y).unwrap().into(),
                        "xplc" => state
                            .city
                            .police_coverage_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xfir" => state.city.fire_coverage_at(tile_x, tile_y).unwrap().into(),
                        "xpop" => state
                            .city
                            .population_density_at(tile_x, tile_y)
                            .unwrap()
                            .into(),
                        "xrog" => state.city.rate_of_growth_at(tile_x, tile_y).unwrap().into(),
                        _ => unreachable!(),
                    };
                    let index = x * side + y;
                    let expected = if name == "altm" {
                        u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                    } else {
                        u16::from(live[index])
                    };
                    assert_eq!(actual, expected, "{boundary} {name} at ({x}, {y})");
                }
            }
        }
    };
    check_rng(&state, "armed-run/armed-entry");
    check_planes(&state, "armed-run/armed-entry");
    assert_eq!(state.city.city_age_days(), Some(28_499));
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(1));
    state.game_clock = state.game_clock.wrapping_add_days(1);
    assert_eq!(
        (
            state.game_clock.year(),
            state.game_clock.month(),
            state.game_clock.day()
        ),
        (1995, 1, 0)
    );
    state.handle_simulation_day_advanced(1995, 1, 0);
    assert_eq!(state.city.city_age_days(), Some(28_500));
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(0));
    check_rng(&state, "armed-run/armed-return");
    check_planes(&state, "armed-run/armed-return");
    let budget: serde_json::Value = serde_json::from_slice(
        &std::fs::read(capture.join("armed-run/budget-return.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        i64::from(state.city.city_funds()),
        budget["funds"].as_i64().unwrap(),
        "funds"
    );
    assert_eq!(
        u64::from(state.city.misc_u32_at(MISC_YEAR_END_OFFSET).unwrap()),
        budget["annual_flag"].as_u64().unwrap(),
        "annual latch"
    );
    let hex = budget["budget_hex"].as_str().unwrap().as_bytes();
    let bytes: Vec<u8> = hex
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(bytes.len(), 16 * 0x70);
    let mut budget_differences = Vec::new();
    for row in 0..16 {
        for field in 0..27 {
            let native_offset = row * 0x70
                + match field {
                    0 => 0x60,
                    1 => 0x64,
                    2 => 0x68,
                    _ if field % 2 == 1 => ((field - 3) / 2) * 4,
                    _ => 0x30 + ((field - 4) / 2) * 4,
                };
            let expected =
                u32::from_le_bytes(bytes[native_offset..native_offset + 4].try_into().unwrap());
            let offset = budget_row_misc_offset_for_city(&state.city, row, field).unwrap();
            let actual = state.city.budget_misc_u32_at(offset).unwrap();
            if actual != expected {
                budget_differences.push((row, field, actual, expected));
            }
        }
    }
    assert!(
        budget_differences.is_empty(),
        "budget differences: {budget_differences:?}"
    );

    // The native capture verifies the January settlement before this local
    // persistence check. Saving and reopening must retain that settled state
    // without consuming any of the process RNG streams.
    let round_trip_path = std::env::temp_dir().join(format!(
        "citygame-win95-annual-settlement-{}.sc2",
        std::process::id()
    ));
    state.city.save(&round_trip_path).unwrap();
    let reloaded = City::load(&round_trip_path).unwrap();
    assert_eq!(reloaded.kind, CityFileKind::WinSc2);
    assert_eq!(reloaded.city_age_days(), Some(28_500));
    assert_eq!(
        reloaded.misc_u32_at(MISC_YEAR_END_OFFSET),
        budget["annual_flag"].as_u64().map(|value| value as u32)
    );
    assert_eq!(
        i64::from(reloaded.city_funds()),
        budget["funds"].as_i64().unwrap()
    );
    state.city = reloaded;
    check_rng(&state, "armed-run/armed-return");
    check_planes(&state, "armed-run/armed-return");
    let mut reloaded_budget_differences = Vec::new();
    for row in 0..16 {
        for field in 0..27 {
            let native_offset = row * 0x70
                + match field {
                    0 => 0x60,
                    1 => 0x64,
                    2 => 0x68,
                    _ if field % 2 == 1 => ((field - 3) / 2) * 4,
                    _ => 0x30 + ((field - 4) / 2) * 4,
                };
            let expected =
                u32::from_le_bytes(bytes[native_offset..native_offset + 4].try_into().unwrap());
            let offset = budget_row_misc_offset_for_city(&state.city, row, field).unwrap();
            let actual = state.city.budget_misc_u32_at(offset).unwrap();
            if actual != expected {
                reloaded_budget_differences.push((row, field, actual, expected));
            }
        }
    }
    assert!(
        reloaded_budget_differences.is_empty(),
        "post-reload budget differences: {reloaded_budget_differences:?}"
    );
    let _ = std::fs::remove_file(round_trip_path);
}

/// The original reopens the same city without creating another view or reseeding.
#[test]
#[ignore = "requires private Win95 live-capture files and original bootstrap city"]
fn win95_captured_clocks_two_city_opens_keep_process_rng() {
    let capture = std::path::PathBuf::from(
        std::env::var_os("CITYGAME_WIN95_CAPTURE_DIR")
            .expect("CITYGAME_WIN95_CAPTURE_DIR must point to the private live capture"),
    );
    let bootstrap = std::path::PathBuf::from(
        std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY")
            .expect("CITYGAME_WIN95_BOOTSTRAP_CITY must point to an original bootstrap city"),
    );
    let mut ticks = [0x10987_u64, 0x10987].into_iter();
    let (lfsr, game_rand) = super::super::runtime::win95_view_randoms_from_tick_source(|| {
        ticks.next().expect("unexpected view clock read")
    });
    let (_, crt, tables) =
        super::super::runtime::win95_document_randoms_from_tick_source(|| 0x1bb7d);
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        bootstrap,
        Some(crt),
        Some(game_rand),
        Some(lfsr),
        Some(tables),
    )
    .unwrap();
    assert!(state.engine_fidelity.uses_win95());
    for (native_snapshot, expected_crt) in [
        ("lifecycle-first", 2_214_272_338),
        ("lifecycle-second", 1_906_082_161),
    ] {
        state
            .load_city(&capture.join("maxis-paused.sc2"), (800, 600), 1.0)
            .unwrap();
        assert_eq!(state.pe_process_crt.seed(), expected_crt);
        assert_eq!(state.pe_process_lfsr.state(), 2_439);
        assert_eq!(state.pe_process_game_rand.state(), 67_975);
        assert_eq!(state.city.city_age_days(), Some(28_237));
        for (name, read_city) in [
            ("xbld", City::building_at as fn(&City, usize, usize) -> u8),
            ("xbit", |city: &City, x, y| {
                city.tile_bits_at(x, y).unwrap().raw_bits()
            }),
        ] {
            let live = std::fs::read(capture.join(format!("{native_snapshot}/{name}-x-major.bin")))
                .expect("missing native plane capture");
            for x in 0..128 {
                for y in 0..128 {
                    assert_eq!(
                        read_city(&state.city, x, y),
                        live[x * 128 + y],
                        "{name} at ({x}, {y})"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires private Win95 editor clock capture and original assets"]
fn win95_captured_clocks_prior_city_through_editor_charter() {
    let root =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_EDITOR_CLOCK_CAPTURE").unwrap());
    let bootstrap =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY").unwrap());
    let exe = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_EXE").unwrap());
    let data = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
    let mut ticks = [0x303f3_u64, 0x303f3].into_iter();
    let (lfsr, game) =
        super::super::runtime::win95_view_randoms_from_tick_source(|| ticks.next().unwrap());
    let (tick, crt, tables) =
        super::super::runtime::win95_document_randoms_from_tick_source(|| 0x5d776);
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        bootstrap,
        Some(crt),
        Some(game),
        Some(lfsr),
        Some(tables),
    )
    .unwrap();
    state.pe_document_tick_word = Some(tick);
    state.pe_newspaper_data = Some(std::sync::Arc::new(
        crate::win95_newspaper::PeNewspaperData::load(
            &data.join("data_usa.dat"),
            &data.join("data_usa.idx"),
        )
        .unwrap(),
    ));
    state.menu_checks.citygame_newspaper = true;
    state.pe_registered_mayor_name = "Trace".into();
    state.pe_new_city_reserved_labels = Some(
        crate::win95_resources::new_city_reserved_labels(&std::fs::read(exe).unwrap()).unwrap(),
    );
    let check = |state: &ViewerState, name: &str| {
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(name).join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(
            u64::from(state.pe_process_crt.seed()),
            manifest["crt_tls_candidates"][0]["holdrand"]
                .as_u64()
                .unwrap(),
            "{name} CRT"
        );
        assert_eq!(
            u64::from(state.pe_process_lfsr.state()),
            manifest["lfsr_raw"].as_u64().unwrap(),
            "{name} LFSR"
        );
        assert_eq!(
            u64::from(state.pe_process_game_rand.state()),
            manifest["game_lcg_raw"].as_u64().unwrap(),
            "{name} game RNG"
        );
        for (label, read) in [
            ("xbld", City::building_at as fn(&City, usize, usize) -> u8),
            ("xbit", |city: &City, x, y| {
                city.tile_bits_at(x, y).unwrap().raw_bits()
            }),
        ] {
            let native =
                std::fs::read(root.join(name).join(format!("{label}-x-major.bin"))).unwrap();
            for x in 0..128 {
                for y in 0..128 {
                    assert_eq!(
                        read(&state.city, x, y),
                        native[x * 128 + y],
                        "{name} {label} ({x},{y})"
                    );
                }
            }
        }
    };
    state
        .load_city(
            &root.join("../budget-second-city-20261114/native-prior.sc2"),
            (1280, 800),
            1.0,
        )
        .unwrap();
    check(&state, "loaded-state");
    state.new_win95_edit_map_from_city().unwrap();
    check(&state, "editor-state");
    state.begin_win95_new_city_setup();
    check(&state, "charter-state");
    let setup = state.win95_new_city_setup.as_mut().unwrap();
    setup.name = "lake city".into();
    setup.difficulty = 2;
    setup.year = 3;
    state.commit_win95_new_city_setup();
    state.finish_edit_map_after_charter();
    state.prepare_pe_newspaper_first_paint();
    assert!(state.pe_newspaper_paint.is_some());
    check(&state, "founded-state");
    state.close_city_info_dialog();
    let tmp = std::env::temp_dir().join(format!("citygame-editor-clock-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let path = tmp.join("clockhard.sc2");
    state.save_city_as(&path, (1280, 800), 1.0).unwrap();
    let actual = City::load(&path).unwrap();
    let native = City::load(root.join("clockhard.sc2")).unwrap();
    for tag in [
        "ALTM", "MISC", "XBIT", "XBLD", "XCRM", "XFIR", "XGRP", "XLAB", "XMIC", "XPLC", "XPLT",
        "XPOP", "XROG", "XTER", "XTHG", "XTRF", "XTXT", "XUND", "XVAL", "XZON",
    ] {
        let decode = |city: &City| {
            let data = &city.chunk(tag).unwrap().data;
            if tag == "ALTM" {
                data.clone()
            } else {
                crate::city::sc2_rle_decode(data).unwrap()
            }
        };
        assert_eq!(decode(&actual), decode(&native), "saved {tag}");
    }
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(tmp).unwrap();
}

#[test]
fn win95_title_new_city_defaults_to_river_without_ocean() {
    let mut state = test_state(City::new_blank("/tmp/pe-title-river.sc2", "Empty").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state
        .new_city_from_title(
            "River City".into(),
            20_000,
            1900,
            1,
            0,
            title_screen::NewCityStartFlow::GenerateAndPlay,
        )
        .unwrap();
    assert!(!state.edit_map_option_enabled(EDIT_MAP_COAST_MISC_OFFSET));
    assert!(state.edit_map_option_enabled(EDIT_MAP_RIVER_MISC_OFFSET));
}

#[test]
#[ignore = "requires private Quadra native save and reopened RAM capture"]
fn mac_native_edited_budget_save_reopens_with_native_budget_and_facilities() {
    let root = PathBuf::from(std::env::var_os("CITYGAME_QUADRA_REOPEN_DIR").unwrap());
    let native = std::fs::read(root.join("reopened.ram")).unwrap();
    let read_u32 = |at: usize| u32::from_be_bytes(native[at..at + 4].try_into().unwrap());
    let a5 = read_u32(0x904) as usize;
    let ptr = |offset: usize| read_u32(a5 + offset - 0x10000) as usize;
    let mut state = test_state(City::new_blank("/tmp/quadra-reopen.city", "Blank").unwrap());
    state.engine_fidelity = EngineFidelity::Mac68k;
    state
        .load_city(&root.join("budgetreplay.city"), (640, 480), 1.0)
        .unwrap();

    assert_eq!(state.city.city_funds(), read_u32(a5 - 0x1054) as i32);
    let book = ptr(0xfd66);
    for row in 0..16 {
        let native_row = book + row * 0x70;
        let mut expected = vec![
            read_u32(native_row + 96),
            read_u32(native_row + 100),
            read_u32(native_row + 104),
        ];
        for month in 0..12 {
            expected.push(read_u32(native_row + month * 4));
            expected.push(read_u32(native_row + 48 + month * 4));
        }
        for (field, expected) in expected.into_iter().enumerate() {
            let offset = budget_row_misc_offset_for_city(&state.city, row, field).unwrap();
            assert_eq!(
                state.city.budget_misc_u32_at(offset),
                Some(expected),
                "budget {row}/{field}"
            );
        }
    }
    let xmic = ptr(0xfd48);
    for slot in 0..150 {
        let row = state.city.microsim_at(slot).unwrap();
        let mut actual = vec![row.tile_id, row.stat0];
        for value in [row.stat1, row.stat2, row.stat3] {
            actual.extend(value.to_be_bytes());
        }
        assert_eq!(
            actual,
            native[xmic + slot * 8..xmic + slot * 8 + 8],
            "XMIC {slot}"
        );
    }
}

#[test]
#[ignore = "requires private Quadra native edited-budget save"]
fn mac_native_edited_budget_citygame_save_and_reopen_preserves_budget_and_facilities() {
    let root = PathBuf::from(std::env::var_os("CITYGAME_QUADRA_REOPEN_DIR").unwrap());
    let path = std::env::temp_dir().join(format!(
        "citygame-edited-budget-reopen-{}.city",
        std::process::id()
    ));
    let mut state = test_state(City::new_blank("/tmp/quadra-save.city", "Blank").unwrap());
    state.engine_fidelity = EngineFidelity::Mac68k;
    state
        .load_city(&root.join("budgetreplay.city"), (640, 480), 1.0)
        .unwrap();
    let before = state.city.clone();
    state.save_city(&path, (640, 480), 1.0).unwrap();
    state.load_city(&path, (640, 480), 1.0).unwrap();
    assert_eq!(state.city.city_funds(), before.city_funds());
    for row in 0..16 {
        for field in 0..27 {
            let offset = budget_row_misc_offset_for_city(&before, row, field).unwrap();
            assert_eq!(
                state.city.budget_misc_u32_at(offset),
                before.budget_misc_u32_at(offset),
                "budget {row}/{field}"
            );
        }
    }
    for slot in 0..150 {
        let a = before.microsim_at(slot).unwrap();
        let b = state.city.microsim_at(slot).unwrap();
        assert_eq!(
            (a.tile_id, a.stat0, a.stat1, a.stat2, a.stat3),
            (b.tile_id, b.stat0, b.stat1, b.stat2, b.stat3),
            "XMIC {slot}"
        );
    }
    for offset in (0x16c..0x1f0).step_by(4) {
        assert_eq!(
            state.city.mac_state_misc_u32_at(offset),
            before.mac_state_misc_u32_at(offset),
            "industry {offset:#x}"
        );
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
#[ignore = "requires private original Mac cities and an output directory"]
fn mac_original_city_corpus_saves_and_reopens() {
    let input = PathBuf::from(std::env::var_os("CITYGAME_MAC_CORPUS_DIR").unwrap());
    let output = PathBuf::from(std::env::var_os("CITYGAME_MAC_CORPUS_OUTPUT").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    for name in [
        "Charleston",
        "Dullsville",
        "Flint",
        "Happyland",
        "Hollywood",
        "Lakeland",
        "Maxis Land",
        "Oakland",
        "Seven Spires",
        "Volcano City",
    ] {
        let mut state = test_state(City::new_blank("/tmp/corpus.city", "Blank").unwrap());
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.load_city(&input.join(name), (640, 480), 1.0).unwrap();
        match name {
            "Charleston" => assert_eq!(state.city.label_at(2), Some("Ashley River Mem. Bridge")),
            "Hollywood" => assert_eq!(state.city.label_at(3), Some("Golden State Freeway (5)")),
            _ => {}
        }
        let before = state.city.clone();
        let path = output.join(format!("{}.city", name.replace(' ', "-")));
        state.save_city(&path, (640, 480), 1.0).unwrap();
        state.load_city(&path, (640, 480), 1.0).unwrap();
        assert_eq!(
            before.city_funds(),
            state.city.city_funds(),
            "{name}: funds"
        );
        for row in 0..16 {
            for field in 0..27 {
                let offset = budget_row_misc_offset_for_city(&before, row, field).unwrap();
                assert_eq!(
                    before.budget_misc_u32_at(offset),
                    state.city.budget_misc_u32_at(offset),
                    "{name}: budget {row}/{field}"
                );
            }
        }
        for slot in 0..150 {
            let a = before.microsim_at(slot).unwrap();
            let b = state.city.microsim_at(slot).unwrap();
            assert_eq!(
                (a.tile_id, a.stat0, a.stat1, a.stat2, a.stat3),
                (b.tile_id, b.stat0, b.stat1, b.stat2, b.stat3),
                "{name}: XMIC {slot}"
            );
        }
        for label in 0..=u8::MAX {
            assert_eq!(
                before.label_at(label),
                state.city.label_at(label),
                "{name}: label {label}"
            );
        }
        for x in 0..128 {
            for y in 0..128 {
                assert_eq!(
                    before.altm_at(x, y),
                    state.city.altm_at(x, y),
                    "{name}: ALTM ({x},{y})"
                );
                assert_eq!(
                    before.terrain_at(x, y),
                    state.city.terrain_at(x, y),
                    "{name}: XTER ({x},{y})"
                );
                assert_eq!(
                    before.building_at(x, y),
                    state.city.building_at(x, y),
                    "{name}: XBLD ({x},{y})"
                );
                assert_eq!(
                    before.zone_at(x, y),
                    state.city.zone_at(x, y),
                    "{name}: XZON ({x},{y})"
                );
                assert_eq!(
                    before.underground_at(x, y),
                    state.city.underground_at(x, y),
                    "{name}: XUND ({x},{y})"
                );
                assert_eq!(
                    before.tile_bits_at(x, y),
                    state.city.tile_bits_at(x, y),
                    "{name}: XBIT ({x},{y})"
                );
                assert_eq!(
                    before.text_overlay_at(x, y),
                    state.city.text_overlay_at(x, y),
                    "{name}: XTXT ({x},{y})"
                );
            }
        }
        assert_eq!(
            before.traffic_overlays(),
            state.city.traffic_overlays(),
            "{name}: XTHG"
        );
        assert_eq!(
            before.police_coverage_slice(),
            state.city.police_coverage_slice(),
            "{name}: XPLC"
        );
        assert_eq!(
            before.fire_coverage_slice(),
            state.city.fire_coverage_slice(),
            "{name}: XFIR"
        );
        assert_eq!(
            before.population_density_slice(),
            state.city.population_density_slice(),
            "{name}: XPOP"
        );
        eprintln!("saved and reopened {name}");
    }
}

#[test]
#[ignore = "requires private Quadra fresh-launch clock and RAM capture"]
fn mac_native_edited_budget_save_launch_uses_captured_startup_clocks() {
    let root = PathBuf::from(std::env::var_os("CITYGAME_QUADRA_LAUNCH_DIR").unwrap());
    let capture = std::fs::read(root.join("open.ram")).unwrap();
    let word = |offset: usize| u32::from_be_bytes(capture[offset..offset + 4].try_into().unwrap());
    let a5 = word(0x904) as usize;
    let port = word(a5) as usize;
    let mut ticks = [2743, 2745, 2746].into_iter();
    let mut startup = super::super::title_flow::MacTitleStartup::new(|| ticks.next().unwrap());
    startup.initialize_newspaper(2751);
    let mut state = test_state(City::new_blank("/tmp/quadra-launch.city", "Blank").unwrap());
    state.engine_fidelity = EngineFidelity::Mac68k;
    startup.install(&mut state);
    state
        .load_city(
            &root.join("../budget-reopen-live/budgetreplay.city"),
            (640, 480),
            1.0,
        )
        .unwrap();
    state.mac_music_callback(4610, true);
    state.mac_music_callback(4613, true);
    assert_eq!(
        state.mac_process_rng.seed(),
        word(port - 126),
        "QuickDraw at reopened city"
    );
    assert_eq!(
        state.mac_game_rng.state(),
        word(a5 - 8),
        "game LCG at reopened city"
    );
    assert_eq!(
        state.mac_app_lfsr.state(),
        u16::from_be_bytes(capture[a5 - 2..a5].try_into().unwrap()),
        "app LFSR at reopened city"
    );
}

#[test]
#[ignore = "requires the private R20 source Flint.city"]
fn mac_r20_flint_title_and_city_open_rng_match_first_calendar_boundary() {
    let city_path = PathBuf::from(
        std::env::var_os("CITYGAME_MAC_R20_FLINT_CITY")
            .expect("CITYGAME_MAC_R20_FLINT_CITY must point to the original Flint.city"),
    );
    let mut ticks = [3163, 3165, 3165].into_iter();
    let mut startup = super::super::title_flow::MacTitleStartup::new(|| {
        ticks.next().expect("captured CODE 15 startup TickCount")
    });
    // CODE 7's captured title seed boundary at 0x49ceec.
    startup.initialize_newspaper(3173);

    let mut state = test_state(City::new_blank("/tmp/mac-r20-flint-open.sc2", "Blank").unwrap());
    state.engine_fidelity = EngineFidelity::Mac68k;
    startup.install(&mut state);
    state.load_city(&city_path, (640, 480), 1.0).unwrap();
    // Native CODE 5's two title/music callbacks advance the game LCG before
    // CODE 2 begins the first city calendar loop at TickCount 3571.
    state.mac_music_callback(3565, true);
    state.mac_music_callback(3571, true);

    assert_eq!(
        state.mac_process_rng.seed(),
        0x17fc_b8f5,
        "QuickDraw at CODE 2"
    );
    assert_eq!(
        state.mac_game_rng.state(),
        0x398a_31a0,
        "game LCG at CODE 2"
    );
    assert_eq!(state.mac_app_lfsr.state(), 0x0c5b, "app LFSR at CODE 2");
}

#[test]
#[ignore = "requires the private R20 Flint city and CODE 2 TickCount/EventRecord traces"]
fn mac_r20_flint_captured_events_reach_january_1975_review_boundary() {
    use std::io::{BufRead, BufReader, Write};

    let city_path = PathBuf::from(
        std::env::var_os("CITYGAME_MAC_R20_FLINT_CITY")
            .expect("CITYGAME_MAC_R20_FLINT_CITY must point to the original Flint.city"),
    );
    let trace_root = PathBuf::from(
        std::env::var_os("CITYGAME_MAC_R20_FLINT_TRACE_DIR")
            .expect("CITYGAME_MAC_R20_FLINT_TRACE_DIR must point to the R20 capture"),
    );
    let code3_root = PathBuf::from(
        std::env::var_os("CITYGAME_MAC_R20_FLINT_CODE3_CAPTURE_DIR")
            .expect("CITYGAME_MAC_R20_FLINT_CODE3_CAPTURE_DIR must point to R20 CODE 3 fixtures"),
    );
    let replay_end_tick = std::env::var("CITYGAME_MAC_R20_FLINT_REPLAY_END_TICK")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(5751);
    let capture_u32 = |name: &str, relative: usize| {
        let ram = std::fs::read(code3_root.join(name)).expect("R20 native RAM boundary");
        let a5 = u32::from_be_bytes(ram[0x904..0x908].try_into().unwrap()) as usize;
        let offset = a5 + relative - 0x1_0000;
        u32::from_be_bytes(ram[offset..offset + 4].try_into().unwrap())
    };
    let native_review_funds = capture_u32("r20-jan1975-budget-entry.ram", 0xefac) as i32;
    let review_ram = std::fs::read(code3_root.join("r20-jan1975-budget-entry.ram"))
        .expect("R20 January review RAM");
    let review_a5 = u32::from_be_bytes(review_ram[0x904..0x908].try_into().unwrap()) as usize;
    let review_ptr = |relative: usize| {
        let offset = review_a5 + relative - 0x1_0000;
        u32::from_be_bytes(review_ram[offset..offset + 4].try_into().unwrap()) as usize
    };
    let native_budget_table = review_ptr(0xfd66);
    let native_review_rows: Vec<[i32; 3]> = (0..16)
        .map(|row| {
            let start = native_budget_table + row * 0x70 + 0x60;
            std::array::from_fn(|field| {
                let offset = start + field * 4;
                i32::from_be_bytes(review_ram[offset..offset + 4].try_into().unwrap())
            })
        })
        .collect();
    let native_return_age = capture_u32("code3-export162-return-age22500-tick5746.ram", 0xefa4);
    let native_return_funds =
        capture_u32("code3-export162-return-age22500-tick5746.ram", 0xefac) as i32;
    let mut ticks = [3163, 3165, 3165].into_iter();
    let mut startup = super::super::title_flow::MacTitleStartup::new(|| {
        ticks.next().expect("captured CODE 15 startup TickCount")
    });
    startup.initialize_newspaper(3173);

    let mut state =
        test_state(City::new_blank("/tmp/mac-r20-flint-calendar.sc2", "Blank").unwrap());
    state.engine_fidelity = EngineFidelity::Mac68k;
    startup.install(&mut state);
    state.load_city(&city_path, (640, 480), 1.0).unwrap();
    assert_eq!(
        state.city.mac_state_misc_u32_at(0x0ff0).map(|value| value as u8),
        Some(0),
        "R20 Flint source save has native Auto-Budget disabled"
    );
    state.mac_music_callback(3565, true);
    state.mac_music_callback(3571, true);
    assert_eq!(state.mac_process_rng.seed(), 0x17fc_b8f5);
    assert_eq!(state.mac_game_rng.state(), 0x398a_31a0);
    assert_eq!(state.mac_app_lfsr.state(), 0x0c5b);
    eprintln!(
        "R20 CityGame open date={:?} speed={:?}",
        state.game_clock,
        state.traffic.speed()
    );

    // Feed captured CODE 2 TickCount samples as clock inputs. The optional
    // sample-order diagnostic also preserves the observed order of actor and
    // calendar sample sites at equal ticks; it never uses native worker
    // entries or outcomes. CityGame's own deadlines gate the dispatches and
    // CityGame generates the random draws.
    state.mac_loop_scheduler.start_captured_clock(3571);
    let mut phase_rng_trace = std::fs::File::create("/tmp/r20-citygame-phase-rng.csv")
        .expect("create R20 CityGame phase RNG trace");
    writeln!(
        phase_rng_trace,
        "tick,phase,age,qd_before,game_before,app_before,qd_after,game_after,app_after,days"
    )
    .unwrap();
    let mut actor_timer_trace = std::env::var_os("CITYGAME_MAC_R20_ACTOR_TIMER_TRACE")
        .map(|path| std::fs::File::create(path).expect("create R20 actor timer trace"));
    let actor_refresh_at_check_tick =
        std::env::var_os("CITYGAME_MAC_R20_ACTOR_REFRESH_AT_CHECK_TICK")
            .is_some_and(|value| value == "1");
    let actor_refresh_from_native_deadline_sample =
        std::env::var_os("CITYGAME_MAC_R20_ACTOR_REFRESH_FROM_NATIVE_DEADLINE_SAMPLE")
            .is_some_and(|value| value == "1");
    if let Some(trace) = actor_timer_trace.as_mut() {
        writeln!(trace, "tick,refresh_tick,last_frame_before,age_before,deadline_before,dispatch_before,smoother_before,wait_ticks,age_after,deadline_after,dispatch_after,smoother_after,qd_before,game_before,app_before,qd_after,game_after,app_after")
            .unwrap();
    }
    let ticks_duration = |ticks: u32| {
        let nanos = (u64::from(ticks) * 1_000_000_000 + 59) / 60;
        std::time::Duration::from_nanos(nanos)
    };
    let mut input_events = Vec::new();
    let event_file = std::fs::File::open(trace_root.join("toolbox-event-records-v2.csv"))
        .expect("R20 Toolbox event capture");
    for line in BufReader::new(event_file).lines().skip(1) {
        let line = line.unwrap();
        let fields: Vec<_> = line.split(',').collect();
        let (Ok(what), Ok(when), Ok(h), Ok(v)) = (
            u16::from_str_radix(fields[7], 16),
            fields[9].parse::<u32>(),
            fields[11].parse::<i16>(),
            fields[10].parse::<i16>(),
        ) else {
            continue;
        };
        if matches!(what, 1 | 2) && (3571..=replay_end_tick).contains(&when) {
            input_events.push((when, what, h, v));
        }
    }
    input_events.sort_by_key(|event| event.0);
    let mut next_input = 0;
    let mut last_frame_tick = 3571_u32;
    let mut calendar_ticks = Vec::new();
    let mut actor_check_ticks = Vec::new();
    let mut native_calendar_refresh_ticks = std::collections::HashMap::new();
    let mut last_native_calendar_check_tick = None;
    let mut captured_sample_order =
        std::collections::HashMap::<u32, (Option<usize>, Option<usize>)>::new();
    let mut post_actor_tickcounts = Vec::new();
    let mut native_calendar_dispatch_ticks = Vec::new();
    let mut native_actor_deadline_write_ticks = std::collections::HashMap::new();
    let mut last_native_actor_check_tick = None;
    // `tick` is the CODE 2 sample-A input. Native CODE 14 is observed later,
    // after sample B at the worker-entry breakpoint, so retain this as
    // an input/check label rather than calling it a worker-entry timestamp.
    let mut citygame_calendar_input_ticks = Vec::new();
    let clock_file = std::fs::File::open(trace_root.join("gdb-clock.csv"))
        .expect("R20 CODE 2 TickCount capture");
    for (capture_row_order, line) in BufReader::new(clock_file).lines().skip(1).enumerate() {
        let line = line.unwrap();
        let fields: Vec<_> = line.split(',').collect();
        let Ok(tick) = fields[2].parse::<u32>() else {
            continue;
        };
        if !(3571..=replay_end_tick).contains(&tick) {
            continue;
        }
        match fields[1] {
            "CODE2.actor_deadline_tick_return" => {
                last_native_actor_check_tick = Some(tick);
                if actor_check_ticks.last() != Some(&tick) {
                    actor_check_ticks.push(tick);
                    captured_sample_order
                        .entry(tick)
                        .or_default()
                        .1
                        .get_or_insert(capture_row_order);
                }
            }
            "CODE2.actor_timer_30tick_add" | "CODE2.actor_timer_45tick_add" => {
                // The branch marker is the first instruction after the second
                // _TickCount trap (static CODE 2 offset 0x382). Use the
                // captured live TickCount at this PC; D7 is unrelated scratch.
                if let Some(check_tick) = last_native_actor_check_tick.take() {
                    native_actor_deadline_write_ticks.insert(check_tick, tick);
                }
            }
            "CODE2.tick_sample_a" if calendar_ticks.last() != Some(&tick) => {
                calendar_ticks.push(tick);
                last_native_calendar_check_tick = Some(tick);
                captured_sample_order
                    .entry(tick)
                    .or_default()
                    .0
                    .get_or_insert(capture_row_order);
            }
            "CODE2.tick_sample_b" => {
                if let Some(check_tick) = last_native_calendar_check_tick.take() {
                    native_calendar_refresh_ticks.insert(check_tick, tick);
                }
            }
            "CODE2.post_export109_tickcount_trap"
                if post_actor_tickcounts.last() != Some(&tick) =>
            {
                post_actor_tickcounts.push(tick);
            }
            "CODE14.export210_dispatch" => native_calendar_dispatch_ticks.push(tick),
            _ => {}
        }
    }
    let mut pointer_down = false;
    let mut poll_count = 0_u32;
    let cgms_map_and_traffic_end = 20 + 9 * 128 * 128 + 2 * 128 * 128 + 128 * 128;
    let cgms_xthg_start = cgms_map_and_traffic_end + 1200 * 4;
    let native_xmic = std::fs::read(
        code3_root.join("r20-live-inputs-code3-export162-return-age22500-tick5746-xmic.bin"),
    )
    .expect("R20 native CODE 3 return XMIC");
    assert_eq!(native_xmic.len(), 150 * 8);
    let native_cgms = std::fs::read(
        code3_root.join("r20-live-inputs-code3-export162-return-age22500-tick5746.cgms"),
    )
    .expect("R20 native CODE 3 return CGMS");
    let mut previous_rng = (
        state.mac_process_rng.seed(),
        state.mac_game_rng.state(),
        state.mac_app_lfsr.state(),
    );

    // A Toolbox MouseUp's EventRecord.when records when the physical release
    // happened, even when GetNextEvent does not return that record until a
    // later CODE 2 poll. TrackControl-style modal handling can resume work on
    // that release before the queued record is returned, so merge its
    // occurrence tick into the captured actor/calendar timeline.
    let mut replay_ticks = calendar_ticks.clone();
    replay_ticks.extend(actor_check_ticks.iter().copied());
    replay_ticks.extend(input_events.iter().map(|event| event.0));
    replay_ticks.sort_unstable();
    replay_ticks.dedup();
    let replay_last_tick = replay_ticks.last().copied().unwrap_or(3571);
    let calendar_ticks: std::collections::HashSet<_> = calendar_ticks.into_iter().collect();
    let actor_check_ticks: std::collections::HashSet<_> = actor_check_ticks.into_iter().collect();
    let event_ticks: std::collections::HashSet<_> =
        input_events.iter().map(|event| event.0).collect();
    let diagnostic_calendar_first_tick = std::env::var("CITYGAME_MAC_R20_CALENDAR_FIRST_TICK")
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    let use_captured_sample_order = std::env::var_os("CITYGAME_MAC_R20_CAPTURED_SAMPLE_ORDER")
        .is_some_and(|value| value == "1");
    let refresh_calendar_from_native_sample_b = std::env::var_os(
        "CITYGAME_MAC_R20_CALENDAR_REFRESH_FROM_NATIVE_SAMPLE_B",
    )
    .is_some_and(|value| value == "1");
    if refresh_calendar_from_native_sample_b {
        assert_eq!(
            native_calendar_refresh_ticks.get(&4_284),
            Some(&4_285),
            "captured calendar check at tick 4284 must pair with native sample B at 4285"
        );
    }
    let boundary_snapshot_age = std::env::var("CITYGAME_MAC_R20_SNAPSHOT_AGE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    let boundary_snapshot_dir = std::env::var_os("CITYGAME_MAC_R20_SNAPSHOT_DIR")
        .map(PathBuf::from);
    let route_trace_output =
        std::env::var_os("CITYGAME_MAC_R20_ROUTE_TRACE_OUTPUT").map(PathBuf::from);
    let route_root_cost_override = std::env::var("CITYGAME_MAC_R20_ROUTE_ROOT_COST_BYTE")
        .ok()
        .and_then(|value| u8::from_str_radix(value.trim_start_matches("0x"), 16).ok());
    let route_root_cost_sequence = std::env::var_os("CITYGAME_MAC_R20_ROUTE_ROOT_COST_SEQUENCE")
        .map(PathBuf::from)
        .map(|path| {
            let contents = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read native R20 root-byte sequence {path:?}: {error}"));
            let mut lines = contents.lines();
            let header = lines.next().expect("native route trace has a header");
            let root_byte_column = header
                .split(',')
                .position(|field| field == "root_cost_byte")
                .expect("native route trace has root_cost_byte column");
            let sequence: Vec<u8> = lines
                .filter_map(|line| {
                    let fields: Vec<_> = line.split(',').collect();
                    if fields.first().copied() != Some("route-entry") {
                        return None;
                    }
                    Some(u8::from_str_radix(
                        fields
                            .get(root_byte_column)
                            .expect("route entry has a root byte"),
                        16,
                    )
                    .expect("route root byte is hexadecimal"))
                })
                .collect();
            assert!(!sequence.is_empty(), "native route trace has no route entries");
            sequence
        });
    let expected_route_qd = std::env::var("CITYGAME_MAC_R20_ROUTE_EXPECTED_QD")
        .ok()
        .and_then(|value| u32::from_str_radix(value.trim_start_matches("0x"), 16).ok());
    assert!(
        route_root_cost_sequence.is_none() || route_root_cost_override.is_none(),
        "choose either a native per-call sequence or one constant root byte"
    );
    let mut route_trace_written = false;
    if let Some(dir) = &boundary_snapshot_dir {
        std::fs::create_dir_all(dir).expect("create R20 boundary snapshot directory");
    }
    let capture_boundary_snapshot = |state: &super::super::ViewerState,
                                     tick: u32,
                                     boundary: &str| {
        let Some((age, dir)) = boundary_snapshot_age.zip(boundary_snapshot_dir.as_ref()) else {
            return;
        };
        let city_age = state.city.mac_state_misc_u32_at(0x10).unwrap_or(0);
        let stem = format!("age{age}-tick{tick}-{boundary}-cityage{city_age}");
        let cgms = crate::city::cgms_v4_from_city_for_test(&state.city).unwrap();
        let cgms_path = dir.join(format!("{stem}.cgms"));
        std::fs::write(&cgms_path, cgms).expect("write R20 boundary CGMS");
        for (name, plane) in [
            ("xplc", state.city.police_coverage_slice()),
            ("xfir", state.city.fire_coverage_slice()),
            ("xpop", state.city.population_density_slice()),
            ("xrog", state.city.rate_of_growth_slice()),
        ] {
            if let Some(plane) = plane {
                std::fs::write(dir.join(format!("{stem}-{name}.bin")), plane)
                    .unwrap_or_else(|_| panic!("write R20 boundary {name}"));
            }
        }
        let mut xmic = Vec::with_capacity(150 * 8);
        for index in 0..150 {
            let row = state.city.microsim_at(index).unwrap_or_default();
            xmic.extend_from_slice(&[
                row.tile_id,
                row.stat0,
                (row.stat1 >> 8) as u8,
                row.stat1 as u8,
                (row.stat2 >> 8) as u8,
                row.stat2 as u8,
                (row.stat3 >> 8) as u8,
                row.stat3 as u8,
            ]);
        }
        std::fs::write(dir.join(format!("{stem}-xmic.bin")), xmic)
            .expect("write R20 boundary XMIC");
        std::fs::write(
            dir.join(format!("{stem}.txt")),
            format!(
                "tick={tick}\nage={city_age}\ndate={:?}\nfunds={}\nquickdraw={:08x}\ngame_lcg={:08x}\napp_lfsr={:04x}\nactor_timers={:?}\n",
                state.game_clock,
                state.city.city_funds(),
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
                state.mac_app_lfsr.state(),
                state.mac_loop_scheduler.actor_timer_diagnostics(),
            ),
        )
        .expect("write R20 boundary metadata");
        eprintln!("R20 boundary snapshot {stem}");
    };

    for tick in replay_ticks {
        let calendar_rng_before_diagnostic = (
            state.mac_process_rng.seed(),
            state.mac_game_rng.state(),
            state.mac_app_lfsr.state(),
        );
        let captured_calendar_precedes_actor =
            captured_sample_order
                .get(&tick)
                .is_some_and(|(calendar, actor)| match (calendar, actor) {
                    (Some(calendar), Some(actor)) => calendar < actor,
                    _ => false,
                });
        let run_calendar_first = calendar_ticks.contains(&tick)
            && (diagnostic_calendar_first_tick == Some(tick)
                || (use_captured_sample_order && captured_calendar_precedes_actor));
        let diagnostic_calendar_advance = if run_calendar_first {
            let age_before = state.city.mac_state_misc_u32_at(0x10).unwrap_or(0);
            if boundary_snapshot_age.is_some_and(|age| age_before < age && age_before + 1 >= age) {
                capture_boundary_snapshot(&state, tick, "calendar-first-before");
            }
            let advance = state
                .finish_mac_frame_calendar_phase_with_tick_refresh_and_budget_pointer(
                    ticks_duration(tick.wrapping_sub(last_frame_tick)),
                    0,
                    pointer_down,
                    || std::time::Duration::ZERO,
                );
            let age_after = state.city.mac_state_misc_u32_at(0x10).unwrap_or(0);
            if boundary_snapshot_age.is_some_and(|age| age_before < age && age_after >= age) {
                capture_boundary_snapshot(&state, tick, "calendar-first-after");
            }
            writeln!(
                phase_rng_trace,
                "{tick},calendar-first,{}, {:08x},{:08x},{:04x},{:08x},{:08x},{:04x},{}",
                state.city.mac_state_misc_u32_at(0x10).unwrap_or(0),
                calendar_rng_before_diagnostic.0,
                calendar_rng_before_diagnostic.1,
                calendar_rng_before_diagnostic.2,
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
                state.mac_app_lfsr.state(),
                advance.days,
            )
            .unwrap();
            last_frame_tick = tick;
            Some(advance)
        } else {
            None
        };
        let rng_before_actor = (
            state.mac_process_rng.seed(),
            state.mac_game_rng.state(),
            state.mac_app_lfsr.state(),
        );
        let mut actor_wait_ticks = 0;
        if actor_check_ticks.contains(&tick) {
            let actor_timer_before = state.mac_loop_scheduler.actor_timer_diagnostics();
            let age_before_actor = state.city.mac_state_misc_u32_at(0x10).unwrap_or(0);
            let refresh_tick = if actor_refresh_from_native_deadline_sample {
                native_actor_deadline_write_ticks
                    .get(&tick)
                    .copied()
                    .unwrap_or(tick)
            } else if actor_refresh_at_check_tick {
                tick
            } else {
                post_actor_tickcounts
                    .iter()
                    .copied()
                    .find(|sample| *sample >= tick)
                    .unwrap_or(tick)
            };
            actor_wait_ticks = state.begin_mac_frame_actor_phase_with_tick_refresh(
                ticks_duration(tick.wrapping_sub(last_frame_tick)),
                || ticks_duration(refresh_tick.wrapping_sub(tick)),
            );
            if (6000..=6150).contains(&tick) {
                eprintln!(
                    "R20 phase audit actor-check tick={tick} refresh={refresh_tick} wait={actor_wait_ticks} last_frame_before={} timer={actor_timer_before:?}->{:?} age={} qd={:08x} game={:08x} app={:04x}",
                    last_frame_tick,
                    state.mac_loop_scheduler.actor_timer_diagnostics(),
                    state.city.mac_state_misc_u32_at(0x10).unwrap_or(0),
                    state.mac_process_rng.seed(),
                    state.mac_game_rng.state(),
                    state.mac_app_lfsr.state(),
                );
            }
            let rng_after_actor = (
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
                state.mac_app_lfsr.state(),
            );
            if let Some(trace) = actor_timer_trace.as_mut() {
                let actor_timer_after = state.mac_loop_scheduler.actor_timer_diagnostics();
                writeln!(
                    trace,
                    "{tick},{refresh_tick},{last_frame_tick},{age_before_actor},{},{},{},{},{},{},{},{},{:08x},{:08x},{:04x},{:08x},{:08x},{:04x}",
                    actor_timer_before.0,
                    actor_timer_before.1,
                    actor_timer_before.2,
                    actor_wait_ticks,
                    state.city.mac_state_misc_u32_at(0x10).unwrap_or(0),
                    actor_timer_after.0,
                    actor_timer_after.1,
                    actor_timer_after.2,
                    rng_before_actor.0,
                    rng_before_actor.1,
                    rng_before_actor.2,
                    rng_after_actor.0,
                    rng_after_actor.1,
                    rng_after_actor.2,
                )
                .unwrap();
            }
            if rng_after_actor != rng_before_actor {
                writeln!(
                    phase_rng_trace,
                    "{tick},actor,{}, {:08x},{:08x},{:04x},{:08x},{:08x},{:04x},0",
                    state.city.mac_state_misc_u32_at(0x10).unwrap_or(0),
                    rng_before_actor.0,
                    rng_before_actor.1,
                    rng_before_actor.2,
                    rng_after_actor.0,
                    rng_after_actor.1,
                    rng_after_actor.2,
                )
                .unwrap();
            }
            if tick == 5748 {
                assert_eq!(
                    state.mac_process_rng.seed(),
                    0x5780_b143,
                    "QuickDraw after native CODE 6 actor work"
                );
                assert_eq!(
                    state.mac_game_rng.state(),
                    0xba1f_4627,
                    "game LCG after native CODE 6 actor work"
                );
                assert_eq!(
                    state.mac_app_lfsr.state(),
                    0x4cdf,
                    "app LFSR after native CODE 6 actor work"
                );
                assert_eq!(
                    state.city.mac_state_misc_u32_at(0x10),
                    Some(native_return_age),
                    "city age after post-review actor work"
                );
            }
            if rng_after_actor.0 != rng_before_actor.0 && tick <= 3600 {
                eprintln!(
                    "R20 local actor phase tick={tick} qd={:08x}->{:08x} counts kind1={} kind2={} kind3={} kind5={} kind16={} tornado={} rail={}",
                    rng_before_actor.0,
                    rng_after_actor.0,
                    state.city.mac_kind1_actor_counter,
                    state.city.mac_kind2_actor_counter,
                    state.city.mac_kind3_actor_counter,
                    state.city.mac_kind5_actor_counter,
                    state.city.mac_kind16_actor_counter,
                    state.city.mac_tornado_actor_counter,
                    state.city.mac_rail_actor_counter,
                );
            }
            last_frame_tick = tick;
        }
        while input_events
            .get(next_input)
            .is_some_and(|event| event.0 <= tick)
        {
            let (_, what, h, v) = input_events[next_input];
            if what == 1 && (h, v) == (461, 122) {
                eprintln!(
                    "R20 captured Done MouseDown occurrence_tick={tick} date={:?} modal={:?} pending={} qd={:08x} game={:08x} app={:04x}",
                    state.game_clock,
                    state.city_info_dialog,
                    state.mac_loop_scheduler.january_budget_review_pending,
                    state.mac_process_rng.seed(),
                    state.mac_game_rng.state(),
                    state.mac_app_lfsr.state(),
                );
            }
            match what {
                1 if (h, v) == (461, 122)
                    && state.city_info_dialog == Some(CityInfoDialog::Budget) =>
                {
                    assert!(state.mac_loop_scheduler.january_budget_review_pending);
                    pointer_down = true;
                }
                2 if pointer_down && (h, v) == (461, 122) => {
                    state.close_city_info_dialog();
                    pointer_down = false;
                }
                _ => {}
            }
            next_input += 1;
        }
        if let Some(advance) = diagnostic_calendar_advance {
            citygame_calendar_input_ticks
                .extend(std::iter::repeat(tick).take(advance.days as usize));
            previous_rng = (
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
                state.mac_app_lfsr.state(),
            );
            last_frame_tick = tick.wrapping_add(actor_wait_ticks);
            if calendar_ticks.contains(&tick) {
                poll_count += 1;
            }
            continue;
        }
        if !calendar_ticks.contains(&tick) && !event_ticks.contains(&tick) {
            last_frame_tick = tick.wrapping_add(actor_wait_ticks);
            continue;
        }
        if tick == 5742 {
            let native_entry = std::fs::read(
                code3_root.join("r20-live-inputs-code3-export162-entry-age22500-tick5746.cgms"),
            )
            .expect("R20 native CODE 3 entry CGMS");
            let local_entry = crate::city::cgms_v4_from_city_for_test(&state.city).unwrap();
            assert_eq!(
                &local_entry[..cgms_map_and_traffic_end],
                &native_entry[..cgms_map_and_traffic_end],
                "Flint map layers and traffic density before native CODE 3"
            );
            assert_eq!(
                &local_entry[cgms_xthg_start..],
                &native_entry[cgms_xthg_start..],
                "Flint traffic overlays before native CODE 3"
            );
        }
        if tick == 18403 {
            let native_xmic = std::fs::read(
                code3_root
                    .join("r20-live-inputs-code3-export162-entry-age22800-tick18397-xmic.bin"),
            )
            .expect("R20 January 1976 native CODE 3 entry XMIC");
            let xmic_mismatches = native_xmic
                .chunks_exact(8)
                .enumerate()
                .filter(|(slot, native)| {
                    let row = state.city.microsim_at(*slot).unwrap();
                    let local = [
                        row.tile_id,
                        row.stat0,
                        (row.stat1 >> 8) as u8,
                        row.stat1 as u8,
                        (row.stat2 >> 8) as u8,
                        row.stat2 as u8,
                        (row.stat3 >> 8) as u8,
                        row.stat3 as u8,
                    ];
                    local != *native
                })
                .count();
            let native_cgms = std::fs::read(
                code3_root.join("r20-live-inputs-code3-export162-entry-age22800-tick18397.cgms"),
            )
            .expect("R20 January 1976 native CODE 3 entry CGMS");
            let local_cgms = crate::city::cgms_v4_from_city_for_test(&state.city).unwrap();
            std::fs::write("/tmp/r20-citygame-jan1976-pre-code3.cgms", &local_cgms)
                .expect("write R20 CityGame January 1976 pre-CODE 3 CGMS");
            let map_and_traffic_mismatches = local_cgms[..cgms_map_and_traffic_end]
                .iter()
                .zip(&native_cgms[..cgms_map_and_traffic_end])
                .filter(|(local, native)| local != native)
                .count();
            let map_plane_mismatches: Vec<_> = (0..12)
                .map(|plane| {
                    let start = 20 + plane * 128 * 128;
                    local_cgms[start..start + 128 * 128]
                        .iter()
                        .zip(&native_cgms[start..start + 128 * 128])
                        .filter(|(local, native)| local != native)
                        .count()
                })
                .collect();
            let xthg_mismatches = local_cgms[cgms_xthg_start..]
                .iter()
                .zip(&native_cgms[cgms_xthg_start..])
                .filter(|(local, native)| local != native)
                .count();
            let native_budget_rows =
                std::fs::read_to_string(code3_root.join(
                    "r20-live-inputs-code3-export162-entry-age22800-tick18397-budget-rows.csv",
                ))
                .expect("R20 January 1976 native CODE 3 entry budget rows");
            let budget_row_differences: Vec<_> = native_budget_rows
                .lines()
                .skip(1)
                .filter_map(|line| {
                    let fields: Vec<_> = line.split(',').collect();
                    let row = fields[0].parse::<usize>().ok()?;
                    let native: [u32; 3] =
                        std::array::from_fn(|field| fields[field + 1].parse().unwrap());
                    let local = state.city.budget_misc_u32_values(0x077c + row * 27 * 4, 3);
                    (local[..3] != native).then_some((row, local[..3].to_vec(), native))
                })
                .collect();
            eprintln!(
                "R20 Jan 1976 pre-CODE3 tick={tick} date={:?} funds={} xmic_mismatches={xmic_mismatches} map_traffic_byte_mismatches={map_and_traffic_mismatches} map_plane_mismatches={map_plane_mismatches:?} xthg_byte_mismatches={xthg_mismatches} budget_row_differences={budget_row_differences:?} qd={:08x} game={:08x} app={:04x}",
                state.game_clock,
                state.city.city_funds(),
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
                state.mac_app_lfsr.state(),
            );
        }
        let rng_before_calendar = state.mac_process_rng.seed();
        let full_rng_before_calendar = (
            state.mac_process_rng.seed(),
            state.mac_game_rng.state(),
            state.mac_app_lfsr.state(),
        );
        let calendar_elapsed_ticks = tick.wrapping_sub(last_frame_tick);
        let budget_pending_before = state.mac_loop_scheduler.january_budget_review_pending;
        let age_before = state.city.mac_state_misc_u32_at(0x10).unwrap_or(0);
        if boundary_snapshot_age.is_some_and(|age| age_before < age && age_before + 1 >= age) {
            capture_boundary_snapshot(&state, tick, "calendar-before");
        }
        let capture_route_trace =
            route_trace_output.is_some() && !route_trace_written && age_before == 22_602;
        let mut finish_calendar_phase = || {
            state.finish_mac_frame_calendar_phase_with_tick_refresh_and_budget_pointer(
                ticks_duration(tick.wrapping_sub(last_frame_tick)),
                actor_wait_ticks,
                pointer_down,
                || {
                    if refresh_calendar_from_native_sample_b {
                        native_calendar_refresh_ticks
                            .get(&tick)
                            .copied()
                            .map(|refresh_tick| {
                                ticks_duration(
                                    refresh_tick
                                        .wrapping_sub(tick)
                                        .saturating_sub(actor_wait_ticks),
                                )
                            })
                            .unwrap_or(std::time::Duration::ZERO)
                    } else {
                        std::time::Duration::ZERO
                    }
                },
            )
        };
        let (advance, route_calls, root_costs_consumed) = if capture_route_trace {
            let ((advance, consumed), route_calls) =
                super::super::budget::with_mac_transit_test_route_trace(|| {
                    if let Some(sequence) = route_root_cost_sequence.as_ref() {
                        let (advance, consumed) =
                            super::super::budget::with_mac_transit_test_root_cost_sequence(
                                sequence.iter().copied(),
                                0,
                                finish_calendar_phase,
                            );
                        (advance, Some(consumed))
                    } else if let Some(root_cost) = route_root_cost_override {
                        let (advance, consumed) =
                            super::super::budget::with_mac_transit_test_root_cost_sequence(
                                std::iter::repeat(root_cost).take(512),
                                root_cost,
                                finish_calendar_phase,
                            );
                        (advance, Some(consumed))
                    } else {
                        (finish_calendar_phase(), None)
                    }
                });
            (advance, route_calls, consumed)
        } else {
            (finish_calendar_phase(), Vec::new(), None)
        };
        if capture_route_trace && !route_calls.is_empty() {
            route_trace_written = true;
            let output = route_trace_output
                .as_ref()
                .expect("route trace output was checked before capture");
            let mut csv = String::from(
                "index,x,y,zone_category,coefficient,cost_limit,root_cost,quickdraw_seed_start,quickdraw_seed_end,result,cost,random_draws,path\n",
            );
            for (index, call) in route_calls.iter().enumerate() {
                let path = call
                    .route
                    .iter()
                    .map(|(x, y, state)| format!("{x}:{y}:{state}"))
                    .collect::<Vec<_>>()
                    .join(";");
                csv.push_str(&format!(
                    "{index},{},{},{},{},{},{},{:08x},{:08x},{},{},{},{}\n",
                    call.x,
                    call.y,
                    call.zone_category,
                    call.coefficient,
                    call.cost_limit,
                    call.root_cost,
                    call.quickdraw_seed_start,
                    call.quickdraw_seed_end,
                    u8::from(!call.route.is_empty()),
                    call.cost,
                    call.random_draws,
                    path,
                ));
            }
            std::fs::write(output, csv).expect("write R20 CODE 14 CityGame route trace");
            let draw_total: usize = route_calls.iter().map(|call| call.random_draws).sum();
            eprintln!(
                "R20 CODE 14 route trace tick={tick} age={age_before} calls={} QuickDraw draws={draw_total} root_override={route_root_cost_override:?} sequence_values={} consumed={root_costs_consumed:?} output={}",
                route_calls.len(),
                route_root_cost_sequence.as_ref().map_or(0, Vec::len),
                output.display(),
            );
        }
        let age_after = state.city.mac_state_misc_u32_at(0x10).unwrap_or(0);
        if capture_route_trace && !route_calls.is_empty() {
            assert_eq!(age_after, 22_603, "R20 CODE 14 return age");
            assert_eq!(state.mac_game_rng.state(), 0x2b8c_7286);
            assert_eq!(state.mac_app_lfsr.state(), 0xa92c);
            match route_root_cost_override {
                None if route_root_cost_sequence.is_none() => assert_eq!(
                    state.mac_process_rng.seed(),
                    0x7245_56b8,
                    "R20 route diagnostic with the production zero-root approximation"
                ),
                Some(0xff) => assert_eq!(
                    state.mac_process_rng.seed(),
                    0x503e_bea3,
                    "R20 all-0xff root-byte sensitivity diagnostic; matches native QD endpoint"
                ),
                None => {}
                Some(_) => {}
            }
            if let Some(expected_qd) = expected_route_qd {
                assert_eq!(
                    state.mac_process_rng.seed(),
                    expected_qd,
                    "R20 CODE 14 route return QuickDraw seed from captured inputs"
                );
            }
        }
        if boundary_snapshot_age.is_some_and(|age| age_before < age && age_after >= age) {
            capture_boundary_snapshot(&state, tick, "calendar-after");
        }
        if (6000..=6150).contains(&tick) {
            eprintln!(
                "R20 phase audit calendar tick={tick} elapsed={calendar_elapsed_ticks} actor_wait={actor_wait_ticks} age={} days={} qd={:08x}->{:08x} game={:08x}->{:08x} app={:04x}->{:04x}",
                state.city.mac_state_misc_u32_at(0x10).unwrap_or(0),
                advance.days,
                full_rng_before_calendar.0,
                state.mac_process_rng.seed(),
                full_rng_before_calendar.1,
                state.mac_game_rng.state(),
                full_rng_before_calendar.2,
                state.mac_app_lfsr.state(),
            );
        }
        let rng_after_calendar = (
            state.mac_process_rng.seed(),
            state.mac_game_rng.state(),
            state.mac_app_lfsr.state(),
        );
        if advance.days > 0 || rng_after_calendar != full_rng_before_calendar {
            writeln!(
                phase_rng_trace,
                "{tick},calendar,{}, {:08x},{:08x},{:04x},{:08x},{:08x},{:04x},{}",
                state.city.mac_state_misc_u32_at(0x10).unwrap_or(0),
                full_rng_before_calendar.0,
                full_rng_before_calendar.1,
                full_rng_before_calendar.2,
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
                state.mac_app_lfsr.state(),
                advance.days,
            )
            .unwrap();
        }
        if tick == 18403 {
            let native_funds =
                capture_u32("code3-export162-return-age22800-tick18398.ram", 0xefac) as i32;
            let native_xmic = std::fs::read(
                code3_root
                    .join("r20-live-inputs-code3-export162-return-age22800-tick18398-xmic.bin"),
            )
            .expect("R20 January 1976 native CODE 3 return XMIC");
            let xmic_mismatches = native_xmic
                .chunks_exact(8)
                .enumerate()
                .filter(|(slot, native)| {
                    let row = state.city.microsim_at(*slot).unwrap();
                    let local = [
                        row.tile_id,
                        row.stat0,
                        (row.stat1 >> 8) as u8,
                        row.stat1 as u8,
                        (row.stat2 >> 8) as u8,
                        row.stat2 as u8,
                        (row.stat3 >> 8) as u8,
                        row.stat3 as u8,
                    ];
                    local != *native
                })
                .count();
            let native_cgms = std::fs::read(
                code3_root.join("r20-live-inputs-code3-export162-return-age22800-tick18398.cgms"),
            )
            .expect("R20 January 1976 native CODE 3 return CGMS");
            let local_cgms = crate::city::cgms_v4_from_city_for_test(&state.city).unwrap();
            std::fs::write("/tmp/r20-citygame-jan1976-return-code3.cgms", &local_cgms)
                .expect("write R20 CityGame January 1976 CODE 3 return CGMS");
            let map_and_traffic_mismatches = local_cgms[..cgms_map_and_traffic_end]
                .iter()
                .zip(&native_cgms[..cgms_map_and_traffic_end])
                .filter(|(local, native)| local != native)
                .count();
            let map_plane_mismatches: Vec<_> = (0..12)
                .map(|plane| {
                    let start = 20 + plane * 128 * 128;
                    local_cgms[start..start + 128 * 128]
                        .iter()
                        .zip(&native_cgms[start..start + 128 * 128])
                        .filter(|(local, native)| local != native)
                        .count()
                })
                .collect();
            let xthg_mismatches = local_cgms[cgms_xthg_start..]
                .iter()
                .zip(&native_cgms[cgms_xthg_start..])
                .filter(|(local, native)| local != native)
                .count();
            let native_budget_rows =
                std::fs::read_to_string(code3_root.join(
                    "r20-live-inputs-code3-export162-return-age22800-tick18398-budget-rows.csv",
                ))
                .expect("R20 January 1976 native CODE 3 return budget rows");
            let budget_row_mismatches: Vec<_> = native_budget_rows
                .lines()
                .skip(1)
                .filter_map(|line| {
                    let fields: Vec<_> = line.split(',').collect();
                    let row = fields[0].parse::<usize>().ok()?;
                    let native: [u32; 3] =
                        std::array::from_fn(|field| fields[field + 1].parse::<u32>().unwrap());
                    let local = state.city.budget_misc_u32_values(0x077c + row * 27 * 4, 3);
                    (local[..3] != native).then_some(row)
                })
                .collect();
            eprintln!(
                "R20 Jan 1976 return-boundary tick={tick} local_funds={} native_funds={native_funds} xmic_mismatches={xmic_mismatches} map_traffic_byte_mismatches={map_and_traffic_mismatches} map_plane_mismatches={map_plane_mismatches:?} xthg_byte_mismatches={xthg_mismatches} budget_row_mismatches={budget_row_mismatches:?} qd={:08x} game={:08x} app={:04x} modal={:?} pending={}",
                state.city.city_funds(),
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
                state.mac_app_lfsr.state(),
                state.city_info_dialog,
                state.mac_loop_scheduler.january_budget_review_pending,
            );
        }
        if tick == 30601 {
            let entry_ram =
                std::fs::read(code3_root.join("code3-export162-entry-age23100-tick30595.ram"))
                    .expect("R20 January 1977 native CODE 3 entry RAM");
            let entry_a5 = u32::from_be_bytes(entry_ram[0x904..0x908].try_into().unwrap()) as usize;
            let auto_budget_offset = entry_a5 + 0xd606 - 0x1_0000;
            assert_eq!(
                entry_ram[auto_budget_offset],
                0,
                "native ffffd606 Auto-Budget byte at the January 1977 boundary"
            );
            let budget_suppression_gate_offset = entry_a5 + 0x8c7e - 0x1_0000;
            assert_eq!(
                &entry_ram[budget_suppression_gate_offset..budget_suppression_gate_offset + 2],
                &[0, 0],
                "native ffff8c7e January-window suppression gate"
            );
            let entry_funds_offset = entry_a5 + 0xefac - 0x1_0000;
            let entry_funds = i32::from_be_bytes(
                entry_ram[entry_funds_offset..entry_funds_offset + 4]
                    .try_into()
                    .unwrap(),
            );
            let native_xmic = std::fs::read(
                code3_root
                    .join("r20-live-inputs-code3-export162-entry-age23100-tick30595-xmic.bin"),
            )
            .expect("R20 January 1977 native CODE 3 entry XMIC");
            let xmic_mismatches = native_xmic
                .chunks_exact(8)
                .enumerate()
                .filter(|(slot, native)| {
                    let row = state.city.microsim_at(*slot).unwrap();
                    let local = [
                        row.tile_id,
                        row.stat0,
                        (row.stat1 >> 8) as u8,
                        row.stat1 as u8,
                        (row.stat2 >> 8) as u8,
                        row.stat2 as u8,
                        (row.stat3 >> 8) as u8,
                        row.stat3 as u8,
                    ];
                    local != *native
                })
                .count();
            let native_cgms = std::fs::read(
                code3_root.join("r20-live-inputs-code3-export162-entry-age23100-tick30595.cgms"),
            )
            .expect("R20 January 1977 native CODE 3 entry CGMS");
            let local_cgms = crate::city::cgms_v4_from_city_for_test(&state.city).unwrap();
            let map_and_traffic_mismatches = local_cgms[..cgms_map_and_traffic_end]
                .iter()
                .zip(&native_cgms[..cgms_map_and_traffic_end])
                .filter(|(local, native)| local != native)
                .count();
            let xthg_mismatches = local_cgms[cgms_xthg_start..]
                .iter()
                .zip(&native_cgms[cgms_xthg_start..])
                .filter(|(local, native)| local != native)
                .count();
            eprintln!(
                "R20 Jan 1977 entry-boundary tick={tick} local_funds={} native_funds={entry_funds} xmic_mismatches={xmic_mismatches} map_traffic_byte_mismatches={map_and_traffic_mismatches} xthg_byte_mismatches={xthg_mismatches} qd={:08x} game={:08x} app={:04x} modal={:?} pending={}",
                state.city.city_funds(),
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
                state.mac_app_lfsr.state(),
                state.city_info_dialog,
                state.mac_loop_scheduler.january_budget_review_pending,
            );
        }
        citygame_calendar_input_ticks.extend(std::iter::repeat(tick).take(advance.days as usize));
        if tick == 5742 {
            assert_eq!(
                state.mac_process_rng.seed(),
                0x5a42_b2f2,
                "QuickDraw at native CODE 14 return after physical Done release"
            );
            assert_eq!(
                state.mac_game_rng.state(),
                0xbd87_efc5,
                "game LCG at native CODE 14 return after physical Done release"
            );
            assert_eq!(
                state.mac_app_lfsr.state(),
                0x8b3b,
                "app LFSR at native CODE 14 return after physical Done release"
            );
            for (slot, native_row) in native_xmic.chunks_exact(8).enumerate() {
                let row = state.city.microsim_at(slot).unwrap();
                let citygame_row = [
                    row.tile_id,
                    row.stat0,
                    (row.stat1 >> 8) as u8,
                    row.stat1 as u8,
                    (row.stat2 >> 8) as u8,
                    row.stat2 as u8,
                    (row.stat3 >> 8) as u8,
                    row.stat3 as u8,
                ];
                assert_eq!(
                    &citygame_row, native_row,
                    "Flint annual CODE 3 XMIC slot {slot} at release boundary"
                );
            }
            let local_cgms = crate::city::cgms_v4_from_city_for_test(&state.city).unwrap();
            assert_eq!(
                &local_cgms[..cgms_map_and_traffic_end],
                &native_cgms[..cgms_map_and_traffic_end],
                "Flint map layers and traffic density after native CODE 14 return"
            );
            assert_eq!(
                &local_cgms[cgms_xthg_start..],
                &native_cgms[cgms_xthg_start..],
                "Flint traffic overlays after native CODE 14 return"
            );
            assert_eq!(
                state.city.mac_state_misc_u32_at(0x10),
                Some(native_return_age),
                "city age at annual settlement"
            );
            assert_eq!(
                state.city.city_funds(),
                native_return_funds,
                "funds at annual settlement"
            );
        }
        if !budget_pending_before && state.mac_loop_scheduler.january_budget_review_pending {
            eprintln!(
                "R20 local January review opened tick={tick} date={:?} modal={:?} qd={:08x} game={:08x} app={:04x}",
                state.game_clock,
                state.city_info_dialog,
                state.mac_process_rng.seed(),
                state.mac_game_rng.state(),
                state.mac_app_lfsr.state(),
            );
            if tick <= 5751 {
                assert_eq!(
                    state.city.city_funds(),
                    native_review_funds,
                    "funds at native January review boundary"
                );
                for (row, native) in native_review_rows.iter().enumerate() {
                    let offset = 0x077c + row * 27 * 4;
                    let values = state.city.budget_misc_u32_values(offset, 3);
                    let local: [i32; 3] = std::array::from_fn(|field| values[field] as i32);
                    assert_eq!(
                        &local, native,
                        "Mac budget current/funding/YTD at review-open row {row}"
                    );
                }
            }
        }
        if state.mac_process_rng.seed() != rng_before_calendar && tick <= 3600 {
            eprintln!(
                "R20 local calendar/event phase tick={tick} qd={rng_before_calendar:08x}->{:08x}",
                state.mac_process_rng.seed(),
            );
        }
        let current_rng = (
            state.mac_process_rng.seed(),
            state.mac_game_rng.state(),
            state.mac_app_lfsr.state(),
        );
        if current_rng != previous_rng {
            eprintln!(
                "R20 local RNG tick={tick} date={:?} qd={:08x} game={:08x} app={:04x}",
                state.game_clock, current_rng.0, current_rng.1, current_rng.2,
            );
        }
        previous_rng = current_rng;
        last_frame_tick = tick.wrapping_add(actor_wait_ticks);
        if calendar_ticks.contains(&tick) {
            poll_count += 1;
        }
    }

    eprintln!(
        "R20 replay polls={poll_count} date={:?} qd={:08x} game={:08x} app={:04x} modal={:?} pending={}",
        state.game_clock,
        state.mac_process_rng.seed(),
        state.mac_game_rng.state(),
        state.mac_app_lfsr.state(),
        state.city_info_dialog,
        state.mac_loop_scheduler.january_budget_review_pending,
    );
    let first_calendar_input_mismatch = citygame_calendar_input_ticks
        .iter()
        .skip(1)
        .zip(&native_calendar_dispatch_ticks)
        .position(|(citygame, native)| citygame != native);
    let first_calendar_boundary_alignment = first_calendar_input_mismatch.map(|index| {
        let citygame_input_tick = citygame_calendar_input_ticks[index + 1];
        (
            index,
            citygame_input_tick,
            native_calendar_refresh_ticks
                .get(&citygame_input_tick)
                .copied(),
            native_calendar_dispatch_ticks[index],
        )
    });
    assert_eq!(
        first_calendar_boundary_alignment,
        Some((48, 4_284, Some(4_285), 4_286)),
        "keep CityGame sample-A input, native sample B, and CODE 14 worker entry distinct"
    );
    eprintln!(
        "R20 calendar boundaries CityGame_inputs={} Quadra_worker_entries={} first_unaligned_boundary=(index,sample_a,sample_b,native_worker) {:?}",
        citygame_calendar_input_ticks.len(),
        native_calendar_dispatch_ticks.len(),
        first_calendar_boundary_alignment,
    );
    eprintln!(
        "R20 calendar boundary labels CityGame sample-A inputs={:?} Quadra CODE14 worker entries={:?}",
        &citygame_calendar_input_ticks[..citygame_calendar_input_ticks.len().min(20)],
        &native_calendar_dispatch_ticks[..native_calendar_dispatch_ticks.len().min(20)],
    );
    if replay_end_tick == 5751 {
        assert_eq!(
            state.mac_process_rng.seed(),
            0x5780_b143,
            "QuickDraw after captured post-review actor boundary"
        );
        assert_eq!(
            state.mac_game_rng.state(),
            0xba1f_4627,
            "game LCG after captured post-review actor boundary"
        );
        assert_eq!(
            state.mac_app_lfsr.state(),
            0x4cdf,
            "app LFSR after captured post-review actor boundary"
        );
        assert_eq!(state.game_clock.year(), 1975);
        assert_eq!(state.game_clock.month(), 1);
        assert_eq!(
            state.city_info_dialog, None,
            "captured Done click resumes simulation"
        );
    } else {
        eprintln!(
            "R20 extended replay end tick={replay_end_tick} last_capture_tick={replay_last_tick} last_frame={last_frame_tick} date={:?} qd={:08x} game={:08x} app={:04x} modal={:?} pending={}",
            state.game_clock,
            state.mac_process_rng.seed(),
            state.mac_game_rng.state(),
            state.mac_app_lfsr.state(),
            state.city_info_dialog,
            state.mac_loop_scheduler.january_budget_review_pending,
        );
        assert!(
            last_frame_tick >= replay_last_tick
                || (state.mac_loop_scheduler.january_budget_review_pending
                    && state.city_info_dialog == Some(CityInfoDialog::Budget)),
            "replay ended before target TickCount outside a captured January modal"
        );
        if replay_end_tick == 30_601 {
            assert_eq!(
                state.city_info_dialog,
                Some(CityInfoDialog::Budget),
                "without a captured 1977 Done input, CityGame remains in the correctly gated January review"
            );
        }
    }
}

/// The original map captures were taken at the new-city naming dialog, before
/// its later newspaper/confirmation work. They are private original-build data.
#[test]
#[ignore = "requires private Win95 live-capture files and original bootstrap city"]
fn win95_captured_clocks_new_city_generation_matches_native_planes() {
    let capture = std::path::PathBuf::from(
        std::env::var_os("CITYGAME_WIN95_CAPTURE_DIR")
            .expect("CITYGAME_WIN95_CAPTURE_DIR must point to the private live capture"),
    );
    let bootstrap = std::path::PathBuf::from(
        std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY")
            .expect("CITYGAME_WIN95_BOOTSTRAP_CITY must point to an original bootstrap city"),
    );
    // Each row is one independently captured native launch. These are clock
    // inputs, not post-initialization RNG seeds or a replayed draw schedule.
    for (snapshot, view_tick, document_tick, expected_game_lcg, newspaper_lfsr) in [
        (
            "newcity-generated",
            0x9478_u64,
            0x11970_u64,
            2_661_696_527,
            13_063,
        ),
        (
            "generator-result",
            0x33ccf_u64,
            0x3eb09_u64,
            3_083_102_840,
            31_134,
        ),
    ] {
        let (lfsr, game_rand) =
            super::super::runtime::win95_view_randoms_from_tick_source(|| view_tick);
        let (_, crt, tables) =
            super::super::runtime::win95_document_randoms_from_tick_source(|| document_tick);
        let mut state = ViewerState::load_default_from_path_with_process_rng(
            bootstrap.clone(),
            Some(crt),
            Some(game_rand),
            Some(lfsr),
            Some(tables),
        )
        .unwrap();
        assert!(state.engine_fidelity.uses_win95());
        state
            .new_city_from_title(
                "New City".into(),
                20_000,
                1900,
                1,
                0,
                title_screen::NewCityStartFlow::GenerateAndPlay,
            )
            .unwrap();
        assert_eq!(
            state.pe_process_game_rand.state(),
            expected_game_lcg,
            "{snapshot}"
        );
        // The captured map is at the naming dialog; CityGame's title path
        // has already opened the founding paper and consumed its cue draw.
        assert_eq!(state.pe_process_lfsr.state(), newspaper_lfsr, "{snapshot}");
        for plane in ["altm", "xter", "xbld", "xzon", "xund", "xtxt", "xbit"] {
            let live = std::fs::read(capture.join(format!("{snapshot}/{plane}-x-major.bin")))
                .expect("missing native plane capture");
            for x in 0..128 {
                for y in 0..128 {
                    let actual: u16 = match plane {
                        "altm" => {
                            let v = state.city.altm_at(x, y);
                            u16::from(v.land)
                                | (u16::from(v.water) << 5)
                                | (u16::from(v.tunnel_levels) << 10)
                        }
                        "xter" => state.city.terrain_at(x, y).into(),
                        "xbld" => state.city.building_at(x, y).into(),
                        "xzon" => state.city.zone_at(x, y).unwrap().raw_bits().into(),
                        "xund" => state.city.underground_at(x, y).into(),
                        "xtxt" => state.city.text_overlay_at(x, y).unwrap().into(),
                        "xbit" => state.city.tile_bits_at(x, y).unwrap().raw_bits().into(),
                        _ => unreachable!(),
                    };
                    let index = x * 128 + y;
                    let expected = if plane == "altm" {
                        u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                    } else {
                        u16::from(live[index])
                    };
                    assert_eq!(actual, expected, "{snapshot}: {plane} at ({x}, {y})");
                }
            }
        }
    }
}

#[test]
#[ignore = "requires private Win95 native generated-city capture"]
fn win95_native_generated_city_survives_citygame_save_reload() {
    let capture = std::path::PathBuf::from(
        std::env::var_os("CITYGAME_WIN95_CAPTURE_DIR")
            .expect("CITYGAME_WIN95_CAPTURE_DIR must point to the private live capture"),
    );
    let mut native = City::load(capture.join("native-generated-play.sc2")).unwrap();
    let native_name = native.name.clone();
    let native_misc = native.misc_u32_values(0, 0x12c0 / 4);
    let roundtrip_path = std::env::temp_dir().join(format!(
        "citygame-native-generated-roundtrip-{}.sc2",
        std::process::id()
    ));
    native.save(&roundtrip_path).unwrap();
    let roundtrip = City::load(&roundtrip_path).unwrap();
    std::fs::remove_file(roundtrip_path).unwrap();
    assert_eq!(
        roundtrip.misc_u32_values(0, 0x12c0 / 4),
        native_misc,
        "decoded MISC state"
    );
    assert_eq!(roundtrip.name, native_name, "city name");
    assert_eq!(
        roundtrip.traffic_density_slice(),
        native.traffic_density_slice(),
        "XTRF"
    );
    assert_eq!(
        roundtrip.land_value_slice(),
        native.land_value_slice(),
        "XVAL"
    );
    assert_eq!(
        roundtrip.crime_density_slice(),
        native.crime_density_slice(),
        "XCRM"
    );
    assert_eq!(
        roundtrip.pollution_density_slice(),
        native.pollution_density_slice(),
        "XPLT"
    );
    assert_eq!(
        roundtrip.police_coverage_slice(),
        native.police_coverage_slice(),
        "XPLC"
    );
    assert_eq!(
        roundtrip.fire_coverage_slice(),
        native.fire_coverage_slice(),
        "XFIR"
    );
    assert_eq!(
        roundtrip.population_density_slice(),
        native.population_density_slice(),
        "XPOP"
    );
    assert_eq!(
        roundtrip.rate_of_growth_slice(),
        native.rate_of_growth_slice(),
        "XROG"
    );
    for index in 0..150 {
        assert_eq!(
            roundtrip.microsim_at(index),
            native.microsim_at(index),
            "XMIC record {index}"
        );
    }
    let live_actors = std::fs::read(capture.join("generator-play/xthg-records.bin"))
        .expect("missing native XTHG capture");
    let actors = crate::win95_overlay::encode_xthg_records(
        roundtrip.traffic_overlays().expect("missing CityGame XTHG"),
    )
    .unwrap();
    assert_eq!(actors, live_actors, "XTHG records");
    for plane in ["altm", "xter", "xbld", "xzon", "xund", "xtxt", "xbit"] {
        let live = std::fs::read(capture.join(format!("generator-play/{plane}-x-major.bin")))
            .expect("missing native plane capture");
        for x in 0..128 {
            for y in 0..128 {
                let actual: u16 = match plane {
                    "altm" => {
                        let v = roundtrip.altm_at(x, y);
                        u16::from(v.land)
                            | (u16::from(v.water) << 5)
                            | (u16::from(v.tunnel_levels) << 10)
                    }
                    "xter" => roundtrip.terrain_at(x, y).into(),
                    "xbld" => roundtrip.building_at(x, y).into(),
                    "xzon" => roundtrip.zone_at(x, y).unwrap().raw_bits().into(),
                    "xund" => roundtrip.underground_at(x, y).into(),
                    "xtxt" => roundtrip.text_overlay_at(x, y).unwrap().into(),
                    "xbit" => roundtrip.tile_bits_at(x, y).unwrap().raw_bits().into(),
                    _ => unreachable!(),
                };
                let index = x * 128 + y;
                let expected = if plane == "altm" {
                    u16::from_le_bytes([live[index * 2], live[index * 2 + 1]])
                } else {
                    u16::from(live[index])
                };
                assert_eq!(actual, expected, "{plane} at ({x}, {y})");
            }
        }
    }
}
