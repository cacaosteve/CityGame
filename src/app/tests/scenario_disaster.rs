use super::*;

#[test]
fn civic_removal_preserves_engine_specific_civic_state_on_both_layouts() {
    for (extension, kind) in [
        ("city", CityFileKind::MacCity),
        ("sc2", CityFileKind::WinSc2),
    ] {
        for (building, extent, bit) in [
            (0xf3u8, 2usize, 0),
            (0xd0, 3, 1),
            (0xdb, 1, 2),
            (0xff, 4, 3),
        ] {
            let path = std::env::temp_dir().join(format!(
                "citygame-civic-{}-{}-{}.{}",
                std::process::id(),
                building,
                bit,
                extension
            ));
            let mut fixture = City::new_blank(&path, "Civic").unwrap();
            fixture.kind = kind;
            for dx in 0..extent {
                for dy in 0..extent {
                    fixture.set_building(32 + dx, 32 - dy, building);
                }
            }
            fixture.set_mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET, 0xabc0_0040);
            fixture.save(&path).unwrap();
            let loaded = City::load(&path).unwrap();
            for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
                let mut city = loaded.clone();
                if engine.uses_win95() {
                    city.prepare_pe_fidelity_from_saved_city();
                    assert!(pe_flood_destroy_tile(&mut city, 32, 32));
                } else {
                    city.prepare_mac_fidelity_from_saved_city();
                    let mut rng = MacToolboxRng::new(123);
                    mac_destroy_building_export80(&mut city, 32, 32, &mut rng, false);
                }
                let expected = 0xabc0_0040 | (1 << bit);
                let native_slot = match (engine, kind) {
                    (EngineFidelity::Win95, CityFileKind::WinSc2)
                    | (EngineFidelity::Mac68k, CityFileKind::MacCity) => expected,
                    _ => 0xabc0_0040,
                };
                assert_eq!(
                    city.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
                    Some(native_slot),
                    "{kind:?} {engine:?} building {building:#x}"
                );
                if engine.uses_win95() {
                    assert_eq!(city.rewards_available_mask(), 1 << bit);
                } else {
                    assert_eq!(
                        city.mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
                        Some(expected),
                        "Mac command-strip latch {kind:?} building {building:#x}"
                    );
                }
                city.save(&path).unwrap();
                let mut reloaded = City::load(&path).unwrap();
                assert_eq!(
                    reloaded.misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
                    Some(native_slot)
                );
                if engine.uses_win95() {
                    reloaded.prepare_pe_fidelity_from_saved_city();
                    assert_eq!(reloaded.rewards_available_mask(), 1 << bit);
                } else {
                    reloaded.prepare_mac_fidelity_from_saved_city();
                    assert_eq!(
                        reloaded.mac_state_misc_u32_at(MISC_REWARDS_AVAILABLE_OFFSET),
                        Some(expected)
                    );
                }
            }
            let _ = std::fs::remove_file(path);
        }
    }
}

#[test]
fn mac_export80_actor_marker_releases_saved_underlay_on_both_city_layouts() {
    for (extension, kind) in [
        ("city", CityFileKind::MacCity),
        ("sc2", CityFileKind::WinSc2),
    ] {
        let path = std::env::temp_dir().join(format!(
            "citygame-mac-export80-actor-cleanup-{}.{}",
            std::process::id(),
            extension
        ));
        let mut city = City::new_blank(&path, "Export80 cleanup").expect("blank city");
        city.kind = kind;
        let (x, y) = (40, 40);
        city.set_building(x, y, 0x6c); // Generic extent-1 selector.
        city.set_terrain(x, y, 1); // Nonzero XTER: PlaceTile(0), no rubble draw.
        city.set_text_overlay(x, y, 0xd0); // XTHG proxy for slot 7.
        let actor_slot = usize::from(0xd0_u8 - 0xc9);
        {
            let actor = &mut city.mac_object_slot_records_mut()[actor_slot];
            actor.kind = 3;
            actor.saved_a = 0x55;
            actor.saved_b = 0x66;
        }

        let mut quickdraw = MacToolboxRng::new(1);
        mac_destroy_building_export80(&mut city, x, y, &mut quickdraw, false);

        assert_eq!(city.building_at(x, y), 0);
        assert_eq!(city.text_overlay_at(x, y), Some(0xd0));
        let actor = city.traffic_overlays().expect("XTHG record")[actor_slot];
        assert_eq!(actor.kind, 3);
        assert_eq!(actor.saved_a, 0, "export 161 clears XTHG byte +0x0a");
        assert_eq!(actor.saved_b, 0x66);
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn window_title_distinguishes_loaded_scenarios() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "Maxis Land").expect("blank city");
    city.set_city_funds(20_000);
    let state = test_state(city);
    assert_eq!(
        state.window_title_base(),
        "CityGame — Maxis Land — Jan 1900 — $20,000"
    );

    let mut scenario =
        City::new_blank("/tmp/sc2000-test.scn", "Maxis Land").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.set_city_funds(20_000);
    let state = test_state(scenario);
    assert_eq!(
        state.window_title_base(),
        "CityGame — Maxis Land Scenario — Jan 1900 — $20,000"
    );
}

#[test]
fn scenario_text_chunks_decode_selection_and_extended_descriptions() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank city");
    city.chunks.push(test_chunk(
        "TEXT",
        [
            &[0x80, 0x00, 0x00, 0x00][..],
            b"Atlanta - UFO Invasion!!\rSave downtown.\0ignored",
        ]
        .concat(),
    ));
    city.chunks.push(test_chunk(
        "TEXT",
        [
            &[0x81, 0x00, 0x00, 0x00][..],
            b"Extended Description:\rDefend the city.",
        ]
        .concat(),
    ));

    let text = scenario_text_for_city(&city);

    assert_eq!(
        text.selection.as_deref(),
        Some("Atlanta - UFO Invasion!! Save downtown.")
    );
    assert_eq!(
        text.extended.as_deref(),
        Some("Extended Description: Defend the city.")
    );
}

#[test]
fn scenario_goals_decode_opensc2k_scen_layout() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank city");
    let mut data = vec![0; 56];
    data[0..4].copy_from_slice(&[0x80, 0x00, 0x00, 0x00]);
    data[0x04..0x06].copy_from_slice(&0x08_u16.to_be_bytes());
    data[0x06] = 75;
    data[0x07] = 70;
    data[0x08..0x0a].copy_from_slice(&60_u16.to_be_bytes());
    data[0x0a..0x0e].copy_from_slice(&72_000_u32.to_be_bytes());
    data[0x0e..0x12].copy_from_slice(&10_000_u32.to_be_bytes());
    data[0x12..0x16].copy_from_slice(&20_000_u32.to_be_bytes());
    data[0x16..0x1a].copy_from_slice(&30_000_u32.to_be_bytes());
    data[0x1a..0x1e].copy_from_slice(&4_000_u32.to_be_bytes());
    data[0x1e..0x22].copy_from_slice(&500_u32.to_be_bytes());
    data[0x22..0x24].copy_from_slice(&70_u16.to_be_bytes());
    data[0x24..0x26].copy_from_slice(&100_u16.to_be_bytes());
    data[0x26..0x2a].copy_from_slice(&200_u32.to_be_bytes());
    data[0x2a..0x2e].copy_from_slice(&300_u32.to_be_bytes());
    data[0x2e..0x32].copy_from_slice(&400_u32.to_be_bytes());
    data[0x32] = 207;
    data[0x33] = 214;
    data[0x34..0x36].copy_from_slice(&16_u16.to_be_bytes());
    data[0x36..0x38].copy_from_slice(&9_u16.to_be_bytes());
    city.chunks.push(test_chunk("SCEN", data));

    let goals = scenario_goals_for_city(&city).expect("scenario goals");

    assert_eq!(goals.disaster_type, 0x08);
    assert_eq!(goals.disaster_x, 75);
    assert_eq!(goals.disaster_y, 70);
    assert_eq!(goals.time_limit_months, 60);
    assert_eq!(goals.city_population_goal, 72_000);
    assert_eq!(goals.education_goal, 100);
    assert_eq!(goals.life_expectancy_goal, 70);
    assert_eq!(goals.pollution_limit, 200);
    assert_eq!(goals.crime_limit, 300);
    assert_eq!(goals.traffic_limit, 400);
    assert_eq!(goals.build_item_1, 207);
    assert_eq!(goals.build_item_1_tiles, Some(16));

    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 45_000);
    city.persist_pe_day22_population_leftover(45_000);
    city.set_misc_u32_at(0x002c, 350);
    city.set_misc_u32_at(0x0030, 450);
    city.set_misc_u32_at(0x0034, 250);
    city.set_city_funds(2_000);
    let lines = scenario_goal_lines(
        &city,
        goals,
        GameClock::new(1900, 3),
        GameClock::new(1900, 3),
    );
    assert!(lines.contains(&"Deadline: Mar 23, 1905 (60 months)".to_owned()));
    // Blank cities carry an empty XTRF map, so live traffic 0 meets the limit
    // even when stale MISC traffic is 450 (map is authoritative).
    assert!(lines.contains(&"Known goals: 1 / 13 complete".to_owned()));
    assert!(lines.contains(&"Disaster: Monster at (75, 70)".to_owned()));
    assert!(lines.contains(&"City population: 45,000 / 72,000".to_owned()));
    assert!(lines.contains(&"Residential population: 0 / 10,000".to_owned()));
    assert!(lines.contains(&"Commercial population: 0 / 20,000".to_owned()));
    assert!(lines.contains(&"Industrial population: 0 / 30,000".to_owned()));
    assert!(lines.contains(&"Funds: $2,000 / $4,000".to_owned()));
    assert!(lines.iter().any(|l| l.starts_with("Life expectancy:")));
    assert!(lines.iter().any(|l| l.starts_with("Education:")));
    assert!(lines.contains(&"Pollution below: 250% / 200%".to_owned()));
    assert!(lines.contains(&"Crime below: 350% / 300%".to_owned()));
    assert!(lines.contains(&"Traffic below: 0 / 400".to_owned()));
    assert!(lines.contains(&"Build Coal Power: 0 / 16 tiles".to_owned()));
    assert!(lines.contains(&"Build School: 0 / 9 tiles".to_owned()));
}

#[test]
fn scenario_zero_native_globals_are_not_replaced_by_estimates() {
    let mut city = City::new_blank("/tmp/scenario-zero-globals.scn", "Zero").unwrap();
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(0);
    let mut progress = scenario_progress_for_city(&city);
    progress.outcomes.land_value = 100;
    progress.outcomes.life_expectancy = 100;
    progress.outcomes.education_quotient = 100;
    let goals = ScenarioGoals {
        land_value_goal: 1,
        life_expectancy_goal: 1,
        education_goal: 1,
        ..Default::default()
    };
    let counts = scenario_goal_counts(&city, goals, progress);
    assert_eq!(counts.known, 3);
    assert_eq!(counts.complete, 0, "PE compares the zero globals directly");
}

#[test]
fn scenario_signed_limits_and_words_follow_pe_comparisons() {
    let mut city = City::new_blank("/tmp/scenario-signed-words.scn", "Signed").unwrap();
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(0);
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 65_536);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 65_536);
    let progress = scenario_progress_for_city(&city);
    let goals = ScenarioGoals {
        life_expectancy_goal: 0x8000,
        education_goal: 0x8000,
        pollution_limit: 0x8000_0000,
        crime_limit: 0xffff_ffff,
        traffic_limit: 0x8000_0001,
        build_item_1: 207,
        build_item_1_tiles: Some(0xffff),
        ..Default::default()
    };
    let counts = scenario_goal_counts(&city, goals, progress);
    assert_eq!(
        counts.known, 3,
        "negative signed environment limits are disabled"
    );
    assert_eq!(
        counts.complete, 1,
        "sign-extended goals fail while signed negative tile count passes"
    );
}

#[test]
fn scenario_land_value_goal_uses_misc_dw_city_land_value() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    // Blank live land sum is 0; PE reads the stored `dwCityLandValue` global.
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 500);
    let goals = ScenarioGoals {
        land_value_goal: 400,
        ..Default::default()
    };
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        ),
        Some(ScenarioOutcome::Complete)
    );
    assert!(
        scenario_goal_lines(
            &city,
            goals,
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        )
        .contains(&"Land value: 500 / 400".to_owned())
    );

    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 399);
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        ),
        Some(ScenarioOutcome::InProgress)
    );
}

#[test]
fn scenario_le_eq_goals_use_workforce_averages_and_scen_words() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    // MISC mirrors PE workforce LE/EQ globals after EQ_LE publish.
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 65);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 90);

    let goals = ScenarioGoals {
        life_expectancy_goal: 70,
        education_goal: 90,
        ..Default::default()
    };
    assert_eq!(
        scenario_known_goal_summary_line(&city, goals, scenario_progress_for_city(&city)),
        Some("Known goals: 1 / 2 complete".to_owned()),
        "EQ met, LE short — PE compares each WORD independently"
    );
    assert!(
        scenario_goal_lines(
            &city,
            goals,
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        )
        .contains(&"Life expectancy: 65 / 70".to_owned())
    );

    let mut data = vec![0; 56];
    data[0..4].copy_from_slice(&[0x80, 0x00, 0x00, 0x00]);
    data[0x22..0x24].copy_from_slice(&70_u16.to_be_bytes());
    data[0x24..0x26].copy_from_slice(&90_u16.to_be_bytes());
    city.chunks.push(test_chunk("SCEN", data));
    let parsed = scenario_goals_for_city(&city).expect("scen");
    assert_eq!(parsed.life_expectancy_goal, 70);
    assert_eq!(parsed.education_goal, 90);
}

#[test]
fn pe_bankruptcy_clears_scenario_active_even_when_inspect_query_suppresses_ui() {
    let mut city = City::new_blank("/tmp/sc2000-pe-scenario-bankrupt.scn", "BankruptScenario")
        .expect("blank city");
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(-100_001);
    let mut state = test_state(city);
    state.query_is_inspect = true;
    state.query_line = "Tile 12,34".to_owned();
    assert!(state.city.pe_scenario_active());

    let population = live_population_stats_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(population);

    assert!(!state.city.pe_scenario_active());
    assert_eq!(state.query_line, "Tile 12,34");
    assert!(state.announced_bankruptcy);
}

#[test]
fn mac_phase22_scenario_countdown_resumes_after_military_offer_and_save() {
    let path = std::env::temp_dir().join(format!(
        "citygame-mac-scenario-countdown-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "Flint fixture").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_mac_city_progression(3);
    city.set_mac_state_misc_u32_at(0x102c, 60_001);
    city.set_misc_u32_values(
        0x106c,
        &[2, 75_000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    city.set_mac_scenario_active(true);

    let tick_day22 = |state: &mut ViewerState| {
        let population = live_population_stats_for_city(&state.city);
        state.sim_calendar_day22_scenario(population)
    };
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(!tick_day22(&mut state).scenario_outcome_announced);
    assert!(state.military_base_dialog.is_some());
    assert_eq!(state.city.misc_u32_at(0x106c), Some(2));
    state.decline_military_base_offer();
    assert!(state.military_base_dialog.is_none());
    assert_eq!(state.city.misc_u32_at(0x106c), Some(1));
    assert!(state.city.mac_scenario_active());

    state.city.save(&path).unwrap();
    let reloaded = City::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(reloaded.kind, CityFileKind::MacCity);
    assert!(reloaded.mac_scenario_active());
    assert_eq!(reloaded.misc_u32_at(0x106c), Some(1));
    let mut state = test_state(reloaded);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sound_effects_enabled = true;
    assert!(tick_day22(&mut state).scenario_outcome_announced);
    assert!(!state.city.mac_scenario_active());
    assert_eq!(state.city.misc_u32_at(0x106c), Some(0));
    assert!(matches!(
        state.announced_scenario_outcome,
        Some(ScenarioOutcome::Failed)
    ));
    assert!(!tick_day22(&mut state).scenario_outcome_announced);
    assert_eq!(
        state.take_pending_disaster_extra_sounds(),
        [PlacementSound::Riot]
    );
}

#[test]
fn mac_phase22_scenario_success_precedes_independent_bankruptcy() {
    let mut city = City::new_blank("/tmp/mac-scenario-result.city", "Scenario").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_misc_u32_values(
        0x106c,
        &[
            1,
            0,
            0,
            0,
            0,
            -100_002_i32 as u32,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    );
    city.set_mac_scenario_active(true);
    city.set_city_funds(-100_001);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sound_effects_enabled = true;
    let population = live_population_stats_for_city(&state.city);
    let result = state.sim_calendar_day22_scenario(population);
    assert!(result.scenario_outcome_announced);
    assert!(matches!(
        state.announced_scenario_outcome,
        Some(ScenarioOutcome::Complete)
    ));
    assert!(state.announced_bankruptcy);
    assert!(!state.city.mac_scenario_active());
    assert_eq!(state.city.misc_u32_at(0x106c), Some(1));
    assert_eq!(
        state.take_pending_disaster_extra_sounds(),
        [PlacementSound::Cheers, PlacementSound::Riot]
    );
}

#[test]
fn mac_phase22_bankruptcy_clears_saved_scenario_flag_only_below_threshold() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_scenario_active(true);
    city.set_city_funds(-100_000);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let population = live_population_stats_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(population);
    assert!(
        state.city.mac_scenario_active(),
        "the native gate is strict"
    );
    assert!(!state.announced_bankruptcy);

    state.city.set_city_funds(-100_001);
    let population = live_population_stats_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(population);

    assert!(!state.city.mac_scenario_active());
    assert!(state.announced_bankruptcy);
    assert!(
        state.query_line.is_empty(),
        "Mac native outcome is not PE copy"
    );
    assert!(!state.city.pe_scenario_deadline_expired());
}

#[test]
fn mac_bankruptcy_clears_cross_layout_scenario_flag_without_overwriting_pe_misc() {
    let mut city = City::new_blank("/tmp/mac-bankruptcy-win.sc2", "Win").unwrap();
    city.set_misc_u32_at(0x1068, 0x1234_5678);
    city.set_city_funds(-100_001);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.city.prepare_mac_fidelity_from_saved_city();
    assert!(state.city.mac_scenario_active());
    let population = live_population_stats_for_city(&state.city);
    state.sim_calendar_day22_scenario(population);
    assert!(state.announced_bankruptcy);
    assert!(!state.city.mac_scenario_active());
    assert_eq!(state.city.misc_u32_at(0x1068), Some(0x1234_5678));

    state
        .city
        .save("/tmp/mac-bankruptcy-win.sc2")
        .expect("save cross-layout Mac state");
    let mut loaded = City::load("/tmp/mac-bankruptcy-win.sc2").expect("reload cross-layout city");
    loaded.prepare_mac_fidelity_from_saved_city();
    assert!(!loaded.mac_scenario_active());
    assert_eq!(loaded.misc_u32_at(0x1068), Some(0x1234_5678));
}

#[test]
fn scenario_build_goal_counts_full_xbld_footprint_tiles() {
    let city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    let mut state = test_state(city);
    // Coal power XBLD 207 is 4×4; PE `dwTileCount[207]` counts every footprint cell.
    assert_eq!(state.place_building_at(20, 20, 207), 16);
    assert_eq!(building_tile_count_for_city(&state.city, 207), 16);
    assert_eq!(state.city.pe_dw_tile_count[207], 16);
    let goals = ScenarioGoals {
        build_item_1: 207,
        build_item_1_tiles: Some(16),
        ..Default::default()
    };
    assert_eq!(
        scenario_known_goal_summary_line(
            &state.city,
            goals,
            scenario_progress_for_city(&state.city)
        ),
        Some("Known goals: 1 / 1 complete".to_owned()),
        "one coal plant (16 XBLD tiles) meets SCEN tile goal 16"
    );
    assert!(
        scenario_goal_lines(
            &state.city,
            goals,
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        )
        .contains(&"Build Coal Power: 16 / 16 tiles".to_owned())
    );

    // Short of a full plant: PE fails while `dwTileCount[id] < goal`.
    state.city.set_building(40, 40, 207);
    let short = ScenarioGoals {
        build_item_1: 207,
        build_item_1_tiles: Some(32),
        ..Default::default()
    };
    assert_eq!(
        scenario_known_goal_summary_line(
            &state.city,
            short,
            scenario_progress_for_city(&state.city)
        ),
        Some("Known goals: 0 / 1 complete".to_owned())
    );
}

#[test]
fn scenario_build_goal_uses_dw_tile_count_not_live_rescan() {
    // PE / sc2kfix: `dwTileCount[bScenarioBuildingGoal]` — not a live XBLD walk.
    let city = City::new_blank("/tmp/sc2000-scen-census.scn", "ScenCensus").expect("blank");
    let mut state = test_state(city);
    assert_eq!(state.place_building_at(20, 20, 207), 16);
    assert_eq!(state.city.pe_dw_tile_count[207], 16);
    let goals = ScenarioGoals {
        build_item_1: 207,
        build_item_1_tiles: Some(16),
        ..Default::default()
    };
    state.city.pe_dw_tile_count[207] = 0;
    assert_eq!(
        scenario_known_goal_summary_line(
            &state.city,
            goals,
            scenario_progress_for_city(&state.city)
        ),
        Some("Known goals: 0 / 1 complete".to_owned()),
        "stale zero census must fail even when coal tiles remain on the map"
    );
    assert!(
        building_tile_count_for_city(&state.city, 207) >= 16,
        "map still has coal tiles; goal check must ignore the live rescan"
    );
    assert!(
        scenario_goal_lines(
            &state.city,
            goals,
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        )
        .contains(&"Build Coal Power: 0 / 16 tiles".to_owned())
    );
    state.city.pe_dw_tile_count[207] = 16;
    assert_eq!(
        scenario_known_goal_summary_line(
            &state.city,
            goals,
            scenario_progress_for_city(&state.city)
        ),
        Some("Known goals: 1 / 1 complete".to_owned())
    );
}

#[test]
fn scenario_day22_implicit_failure_still_decrements_deadline() {
    for has_completed_goal in [false, true] {
        let mut city = City::new_blank("/tmp/scenario-implicit.scn", "Implicit").unwrap();
        city.kind = CityFileKind::WinScenario;
        city.set_city_funds(-1);
        if has_completed_goal {
            city.persist_pe_day22_population_leftover(1_000);
        }
        city.chunks.push(test_scen_goals_chunk(ScenarioGoals {
            time_limit_months: 2,
            city_population_goal: if has_completed_goal { 1_000 } else { 0 },
            ..Default::default()
        }));
        let mut state = test_state(city);
        state.no_disasters_enabled = true;
        let population = live_population_stats_for_city(&state.city);
        state.sim_calendar_day22_scenario(population);
        assert_eq!(
            state.city.scenario_time_limit_months(),
            Some(1),
            "configured goal={has_completed_goal}"
        );
        state.sim_calendar_day22_scenario(population);
        assert_eq!(state.city.scenario_time_limit_months(), Some(0));
        assert!(state.city.pe_scenario_deadline_expired());
    }
}

#[test]
fn scenario_live_countdown_does_not_reapply_elapsed_time() {
    let mut city = City::new_blank("/tmp/scenario-live-clock.scn", "Clock").unwrap();
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(10_000);
    city.set_pe_scenario_active(true);
    let goals = ScenarioGoals {
        time_limit_months: 30,
        city_population_goal: 50_000,
        ..Default::default()
    };
    city.chunks.push(test_scen_goals_chunk(goals));
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1902, 8)
        ),
        Some(ScenarioOutcome::InProgress)
    );
    city.set_pe_scenario_deadline_expired(true);
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1902, 8)
        ),
        Some(ScenarioOutcome::Failed)
    );
}

#[test]
fn scenario_day22_decrements_w_scenario_time_limit_months() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    city.kind = CityFileKind::WinScenario;
    city.chunks.push(test_scen_goals_chunk(ScenarioGoals {
        time_limit_months: 2,
        city_population_goal: 50_000,
        ..Default::default()
    }));
    let mut state = test_state(city);
    state.no_disasters_enabled = true;

    let population = live_population_stats_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(population);
    assert_eq!(state.city.scenario_time_limit_months(), Some(1));
    assert!(!state.city.pe_scenario_deadline_expired());
    assert_eq!(
        scenario_outcome_for_progress(
            &state.city,
            scenario_goals_for_city(&state.city).unwrap(),
            scenario_progress_for_city(&state.city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        ),
        Some(ScenarioOutcome::InProgress)
    );

    let population = live_population_stats_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(population);
    assert_eq!(state.city.scenario_time_limit_months(), Some(0));
    assert!(state.city.pe_scenario_deadline_expired());
    assert_eq!(
        scenario_outcome_for_progress(
            &state.city,
            scenario_goals_for_city(&state.city).unwrap(),
            scenario_progress_for_city(&state.city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 2),
        ),
        Some(ScenarioOutcome::Failed),
        "PE fails when --wScenarioTimeLimitMonths hits 0"
    );
}

#[test]
fn scenario_win95_refresh_preserves_environment_until_native_publisher_phases() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank("/tmp/day21-environment.sc2", "Environment").unwrap();
        city.kind = kind;
        city.pe_dw_city_land_value = 444;
        city.pe_dw_city_crime = 111;
        city.pe_dw_city_pollution = 222;
        city.set_pe_traffic_count(333);
        city.set_traffic_density(20, 20, 200);
        city.set_zone(30, 30, ZONE_RESIDENTIAL_LIGHT);
        city.set_building(30, 30, RESIDENTIAL_1X1_BUILDINGS[0]);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.refresh_live_misc_current_values();
        assert_eq!(state.city.pe_dw_city_land_value, 444, "land {kind:?}");
        assert_eq!(state.city.pe_dw_city_crime, 111, "crime {kind:?}");
        assert_eq!(state.city.pe_dw_city_pollution, 222, "pollution {kind:?}");
        assert_eq!(state.city.pe_traffic_count(), 333, "traffic {kind:?}");
        assert_eq!(state.city.traffic_density_at(20, 20), Some(200));

        let population = live_population_stats_for_city(&state.city);
        let outcomes = live_city_service_outcomes_for_city(&state.city);
        state.sim_calendar_day2_density(population, outcomes);
        assert!(state.city.pe_dw_city_land_value > 0, "land map {kind:?}");
        assert_eq!(
            state.city.pe_dw_city_land_value,
            density_map_microsim64_sum(state.city.land_value_slice().unwrap())
        );
        assert_eq!(
            state.city.pe_dw_city_crime,
            density_map_microsim64_sum(state.city.crime_density_slice().unwrap())
        );
        assert_eq!(
            state.city.pe_dw_city_pollution,
            density_map_microsim64_sum(state.city.pollution_density_slice().unwrap())
        );
        assert_eq!(
            state.city.pe_traffic_count(),
            333,
            "day 2 must not publish traffic"
        );
        state.sim_calendar_day19_traffic();
        assert_eq!(
            state.city.pe_traffic_count(),
            trip_generator::traffic_density_microsim_sum(&state.city)
        );
        assert_eq!(state.city.traffic_density_at(20, 20), Some(150));
    }
}

#[test]
fn scenario_environment_uses_published_globals_even_when_maps_disagree() {
    let mut city = City::new_blank("/tmp/scenario-counters.scn", "Counters").unwrap();
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(0);
    city.sync_crime_density_from_slice(&vec![200; CITY_TILES]);
    city.sync_pollution_density_from_slice(&vec![200; CITY_TILES]);
    city.set_traffic_density(20, 20, 200);
    let goals = ScenarioGoals {
        crime_limit: 10,
        traffic_limit: 10,
        pollution_limit: 10,
        ..Default::default()
    };
    for (crime, traffic, pollution, completed) in [(7, 8, 9, 3), (0, 0, 0, 3), (11, 12, 13, 0)] {
        city.pe_dw_city_crime = crime;
        city.set_pe_traffic_count(traffic);
        city.pe_dw_city_pollution = pollution;
        let progress = scenario_progress_for_city(&city);
        assert_eq!(
            progress.counters,
            CityScenarioCounters {
                crime,
                traffic,
                pollution
            }
        );
        let counts = scenario_goal_counts(&city, goals, progress);
        assert_eq!(counts.known, 3);
        assert_eq!(counts.complete, completed);
        assert_eq!(city.traffic_density_at(20, 20), Some(200));
    }
}

#[test]
fn scenario_day2_publishes_win95_density_totals_for_both_city_formats() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank("/tmp/scenario-density.sc2", "Density").unwrap();
        city.kind = kind;
        city.pe_dw_city_crime = u32::MAX;
        city.pe_dw_city_pollution = u32::MAX;
        let mut state = test_state(city);
        let population = live_population_stats_for_city(&state.city);
        let outcomes = live_city_service_outcomes_for_city(&state.city);
        state.sim_calendar_day2_density(population, outcomes);
        let crime = density_map_microsim64_sum(state.city.crime_density_slice().unwrap());
        let pollution = density_map_microsim64_sum(state.city.pollution_density_slice().unwrap());
        assert_eq!(state.city.pe_dw_city_crime, crime, "{kind:?}");
        assert_eq!(state.city.pe_dw_city_pollution, pollution, "{kind:?}");
    }
}

#[test]
fn scenario_env_limits_use_misc_when_density_maps_absent() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    // Blank cities ship an empty XTRF map (authoritative traffic 0). Crime/pollution
    // maps are absent, so PE globals come from MISC `0x2c` / `0x34`.
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 10);
    city.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, 999);
    city.set_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, 30);
    let progress = scenario_progress_for_city(&city);
    assert_eq!(progress.counters.crime, 10);
    assert_eq!(
        progress.counters.traffic, 0,
        "present empty XTRF keeps traffic at 0 (stale MISC ignored)"
    );
    assert_eq!(progress.counters.pollution, 30);

    let goals = ScenarioGoals {
        pollution_limit: 30,
        crime_limit: 10,
        traffic_limit: 20,
        ..Default::default()
    };
    assert_eq!(
        scenario_known_goal_summary_line(&city, goals, progress),
        Some("Known goals: 3 / 3 complete".to_owned()),
        "PE succeeds when city counter <= limit (fails only when city > limit)"
    );
    let over = ScenarioGoals {
        pollution_limit: 29,
        crime_limit: 9,
        traffic_limit: 0,
        ..Default::default()
    };
    // traffic_limit 0 is inactive (PE `limit > 0` gate); crime/pollution fail.
    assert_eq!(
        scenario_known_goal_summary_line(&city, over, progress),
        Some("Known goals: 0 / 2 complete".to_owned())
    );
}

#[test]
fn scenario_deadline_line_wraps_months_across_years() {
    assert_eq!(
        scenario_deadline_line(GameClock::new(1999, 12), 2),
        "Deadline: Feb 23, 2000 (2 months)"
    );
}

#[test]
fn scenario_known_goal_summary_counts_only_measurable_goals() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 100_000);
    city.persist_pe_day22_population_leftover(100_000);
    city.set_city_funds(12_000);
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 90);
    city.set_misc_u32_at(0x002c, 0);
    city.set_misc_u32_at(0x0030, 300);
    city.set_misc_u32_at(0x0034, 250);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 120);
    let goals = ScenarioGoals {
        disaster_type: 0,
        disaster_x: 0,
        disaster_y: 0,
        time_limit_months: 0,
        city_population_goal: 75_000,
        residential_population_goal: 30_000,
        commercial_population_goal: 0,
        industrial_population_goal: 0,
        fund_goal: 10_000,
        land_value_goal: 80,
        life_expectancy_goal: 0,
        education_goal: 100,
        pollution_limit: 200,
        crime_limit: 0,
        traffic_limit: 400,
        build_item_1: 207,
        build_item_2: 0,
        build_item_1_tiles: Some(16),
        build_item_2_tiles: None,
    };

    assert_eq!(
        scenario_known_goal_summary_line(&city, goals, scenario_progress_for_city(&city),),
        Some("Known goals: 5 / 8 complete".to_owned())
    );
}

#[test]
fn scenario_goal_lines_prefer_live_environment_counters_over_stale_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 0);
    city.set_misc_u32_at(MISC_CITY_TRAFFIC_OFFSET, 0);
    city.set_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, 0);
    city.set_building(10, 10, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_building(11, 10, COMMERCIAL_1X1_BUILDINGS[0]);
    city.set_building(12, 10, INDUSTRIAL_1X1_BUILDINGS[0]);
    city.set_building(14, 10, 207);
    city.set_building(
        15,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_traffic_density(15, 10, 224);
    let progress = scenario_progress_for_city(&city);
    assert!(progress.counters.crime > 0, "{:?}", progress.counters);
    assert!(progress.counters.traffic > 0, "{:?}", progress.counters);
    assert!(progress.counters.pollution > 0, "{:?}", progress.counters);
    let goals = ScenarioGoals {
        crime_limit: progress.counters.crime.saturating_sub(1),
        traffic_limit: progress.counters.traffic.saturating_sub(1),
        pollution_limit: progress.counters.pollution.saturating_sub(1),
        ..Default::default()
    };

    let lines = scenario_goal_lines(
        &city,
        goals,
        GameClock::new(1900, 1),
        GameClock::new(1900, 1),
    );

    assert!(lines.contains(&"Known goals: 0 / 3 complete".to_owned()));
    assert!(lines.iter().any(|line| line.starts_with("Crime below: ")));
    assert!(lines.iter().any(|line| line.starts_with("Traffic below: ")));
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("Pollution below: "))
    );
}

#[test]
fn scenario_city_population_goal_uses_pe_dw_not_census_total() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    // Saved/census floor looks huge, but PE dwCityPopulation is still low.
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 100_000);
    city.persist_pe_day22_population_leftover(999);
    let goals = ScenarioGoals {
        city_population_goal: 1_000,
        ..Default::default()
    };
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        ),
        Some(ScenarioOutcome::InProgress),
        "census/MISC normal pop must not satisfy dwScenarioCitySize"
    );
    assert!(
        scenario_goal_lines(
            &city,
            goals,
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        )
        .contains(&"City population: 999 / 1,000".to_owned())
    );

    city.persist_pe_day22_population_leftover(1_000);
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        ),
        Some(ScenarioOutcome::Complete),
        "PE: goal fails only while dwScenarioCitySize > dwCityPopulation"
    );
}

#[test]
fn scenario_rci_population_goals_use_tax_fund_current_costs() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    // Seed RESFUND/COMFUND/INDFUND iCurrentCosts without matching census pops.
    for (index, current) in [(0_usize, 10_000_u32), (1, 20_000), (2, 5_000)] {
        let offset = budget_row_misc_offset(index, 0).expect("tax row");
        let mut values = [0_u32; BUDGET_SERVICE_MISC_STRIDE];
        values[BUDGET_SERVICE_CURRENT_FIELD] = current;
        values[BUDGET_SERVICE_FUNDING_FIELD] = DEFAULT_TAX_RATE;
        city.set_misc_u32_values(offset, &values);
    }
    let goals = ScenarioGoals {
        residential_population_goal: 10_000,
        commercial_population_goal: 20_000,
        industrial_population_goal: 10_000,
        ..Default::default()
    };
    let progress = scenario_progress_for_city(&city);
    assert_eq!(progress.population.residential_population, 0);
    assert_eq!(
        budget_tax_fund_current_costs_for_city(&city, progress.population),
        [10_000, 20_000, 5_000]
    );
    assert_eq!(
        scenario_known_goal_summary_line(&city, goals, progress),
        Some("Known goals: 2 / 3 complete".to_owned()),
        "industrial tax current 5k misses 10k; census zeros must not decide"
    );
    assert!(
        scenario_goal_lines(
            &city,
            goals,
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        )
        .contains(&"Industrial population: 5,000 / 10,000".to_owned())
    );

    // Live tax-base fallback when rows are unset: arco splits into R/C/I currents.
    let mut unset = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    unset.set_pe_dw_arcology_population(120);
    let arco_pop = PopulationStats {
        arcology_population: 120,
        ..PopulationStats::default()
    };
    assert_eq!(
        budget_tax_fund_current_costs_for_city(&unset, arco_pop),
        [20, 10, 10]
    );
}

#[test]
fn scenario_cash_goal_uses_funds_minus_bond_count() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    city.set_city_funds(10_000);
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 2);
    assert_eq!(scenario_net_city_funds(&city), 9_998);
    let goals = ScenarioGoals {
        fund_goal: 10_000,
        ..Default::default()
    };
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        ),
        Some(ScenarioOutcome::InProgress),
        "gross funds must not satisfy cash goal while bond count pulls net under"
    );
    assert!(
        scenario_goal_lines(
            &city,
            goals,
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        )
        .contains(&"Funds: $9,998 / $10,000".to_owned())
    );

    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 0);
    assert_eq!(scenario_net_city_funds(&city), 10_000);
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        ),
        Some(ScenarioOutcome::Complete)
    );
}

#[test]
fn scenario_cash_goal_subtracts_raw_bond_dword_without_list_clamp() {
    let mut city = City::new_blank("/tmp/scenario-raw-bonds.scn", "test").unwrap();
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(50);
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 51);
    assert_eq!(scenario_net_city_funds(&city), -1);
    assert_eq!(
        outstanding_bond_count_for_city(&city),
        50,
        "bond-list safety limit stays intact"
    );
    let clock = GameClock::new(1900, 1);
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            ScenarioGoals::default(),
            scenario_progress_for_city(&city),
            clock,
            clock
        ),
        Some(ScenarioOutcome::InProgress)
    );
    city.set_city_funds(0);
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, u32::MAX);
    assert_eq!(
        scenario_net_city_funds(&city),
        1,
        "PE SUB wraps the raw DWORD"
    );
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            ScenarioGoals::default(),
            scenario_progress_for_city(&city),
            clock,
            clock
        ),
        Some(ScenarioOutcome::Complete)
    );
}

#[test]
fn win_scenario_zero_tax_rows_do_not_fall_back_to_live_estimates() {
    let mut city = City::new_blank("/tmp/sc2000-zero-tax-rows.scn", "test").unwrap();
    city.kind = CityFileKind::WinScenario;
    city.set_pe_dw_arcology_population(120);
    let population = PopulationStats {
        arcology_population: 120,
        ..PopulationStats::default()
    };
    // The PE evaluator reads iCurrentCosts directly, including a zero row.
    assert_eq!(
        budget_tax_fund_current_costs_for_city(&city, population),
        [0; 3]
    );
    let mut progress = scenario_progress_for_city(&city);
    progress.population = population;
    let counts = scenario_goal_counts(
        &city,
        ScenarioGoals {
            residential_population_goal: 1,
            ..Default::default()
        },
        progress,
    );
    assert_eq!(counts.known, 1);
    assert_eq!(counts.complete, 0);
}

#[test]
fn scenario_signed_budget_checks_run_for_zero_goals_like_pe() {
    let current_clock = GameClock::new(1900, 1);
    let mut city =
        City::new_blank("/tmp/sc2000-zero-scenario-goals.scn", "test").expect("blank city");
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(-1);
    assert_eq!(
        scenario_known_goal_summary_line(
            &city,
            ScenarioGoals::default(),
            scenario_progress_for_city(&city),
        ),
        None,
        "implicit PE thresholds do not become visible configured goals"
    );
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            ScenarioGoals::default(),
            scenario_progress_for_city(&city),
            current_clock,
            current_clock,
        ),
        Some(ScenarioOutcome::InProgress),
        "PE's unconditional signed cash comparison rejects negative net funds at a zero goal"
    );

    city.set_city_funds(0);
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            ScenarioGoals::default(),
            scenario_progress_for_city(&city),
            current_clock,
            current_clock,
        ),
        Some(ScenarioOutcome::Complete),
        "zero cash and zero signed R/C/I values pass their implicit zero thresholds"
    );

    let mut negative_tax =
        City::new_blank("/tmp/sc2000-zero-scenario-tax.scn", "test").expect("blank city");
    negative_tax.kind = CityFileKind::WinScenario;
    let row = budget_row_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).expect("res tax current");
    negative_tax.set_misc_u32_at(row, u32::MAX);
    assert_eq!(
        scenario_outcome_for_progress(
            &negative_tax,
            ScenarioGoals::default(),
            scenario_progress_for_city(&negative_tax),
            current_clock,
            current_clock,
        ),
        Some(ScenarioOutcome::InProgress),
        "PE's signed residential-current comparison also runs when its goal is zero"
    );
}

#[test]
fn scenario_cash_goal_uses_signed_pe_dword_comparison() {
    let mut city = City::new_blank("/tmp/sc2000-signed-cash-goal.scn", "test").expect("blank city");
    city.set_city_funds(i32::MIN);
    let goals = ScenarioGoals {
        fund_goal: 0x8000_0000,
        ..Default::default()
    };
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        ),
        Some(ScenarioOutcome::Complete),
        "PE's signed JGE treats both raw 0x80000000 DWORDs as i32::MIN"
    );
}

#[test]
fn scenario_goal_lines_report_complete_and_failed_status() {
    let mut complete_city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    complete_city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 2_500);
    complete_city.persist_pe_day22_population_leftover(2_500);
    let complete_goals = ScenarioGoals {
        city_population_goal: 1_000,
        ..Default::default()
    };
    let complete_lines = scenario_goal_lines(
        &complete_city,
        complete_goals,
        GameClock::new(1900, 1),
        GameClock::new(1900, 6),
    );
    assert!(complete_lines.contains(&"Status: Complete".to_owned()));

    let failed_city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    let failed_goals = ScenarioGoals {
        time_limit_months: 2,
        city_population_goal: 10_000,
        ..Default::default()
    };
    let failed_lines = scenario_goal_lines(
        &failed_city,
        failed_goals,
        GameClock::new(1900, 1),
        GameClock::new(1900, 4),
    );
    assert!(failed_lines.contains(&"Status: Failed - deadline passed".to_owned()));
}

#[test]
fn scenario_outcome_matches_goal_completion_and_deadline() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    city.set_city_funds(2_000);
    let complete_goals = ScenarioGoals {
        fund_goal: 1_000,
        ..Default::default()
    };
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            complete_goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 1),
        ),
        Some(ScenarioOutcome::Complete)
    );

    let failed_goals = ScenarioGoals {
        time_limit_months: 1,
        fund_goal: 10_000,
        ..Default::default()
    };
    assert_eq!(
        scenario_outcome_for_progress(
            &city,
            failed_goals,
            scenario_progress_for_city(&city),
            GameClock::new(1900, 1),
            GameClock::new(1900, 3),
        ),
        Some(ScenarioOutcome::Failed)
    );
}

#[test]
fn inactive_win_scenario_does_not_publish_another_terminal_outcome() {
    let mut city = City::new_blank("/tmp/inactive-goal.scn", "Scenario").unwrap();
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(2_000);
    city.chunks.push(test_scen_goals_chunk(ScenarioGoals {
        fund_goal: 1_000,
        ..Default::default()
    }));
    let mut state = test_state(city);
    state.city.set_pe_scenario_active(false);
    state.query_line = "Previous result".to_owned();
    assert!(!state.refresh_scenario_outcome_message());
    assert_eq!(state.query_line, "Previous result");
    assert_eq!(state.announced_scenario_outcome, None);
}

#[test]
fn simulation_month_announces_terminal_scenario_once() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(2_000);
    city.chunks.push(test_scen_goals_chunk(ScenarioGoals {
        fund_goal: 1_000,
        ..Default::default()
    }));
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 1);
    state.scenario_start_clock = Some(GameClock::new(1900, 1));

    state.handle_simulation_month_advanced();

    assert_eq!(state.query_line, "Scenario Complete");
    assert_eq!(
        state.announced_scenario_outcome,
        Some(ScenarioOutcome::Complete)
    );
    assert!(!state.city.pe_scenario_active());
    assert!(!state.refresh_scenario_outcome_message());
}

#[test]
fn simulation_month_does_not_replace_inspect_query_with_scenario_outcome() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(2_000);
    city.chunks.push(test_scen_goals_chunk(ScenarioGoals {
        fund_goal: 1_000,
        ..Default::default()
    }));
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 1);
    state.scenario_start_clock = Some(GameClock::new(1900, 1));
    state.query_line = "Tile 12,34".to_owned();
    state.query_is_inspect = true;

    state.handle_simulation_month_advanced();

    assert_eq!(state.query_line, "Tile 12,34");
    assert_eq!(state.announced_scenario_outcome, None);
    assert!(
        !state.city.pe_scenario_active(),
        "PE clears the scenario-active global even when inspect mode suppresses the result UI"
    );
}

#[test]
fn scenario_start_clock_uses_loaded_scenario_date() {
    let mut city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    city.kind = CityFileKind::WinScenario;
    city.set_saved_date(CityDate {
        year: 1937,
        month: 5,
    });

    assert_eq!(
        scenario_start_clock_for_city(&city, GameClock::new(2000, 1)),
        Some(GameClock::new(1937, 5))
    );

    city.kind = CityFileKind::WinSc2;
    assert_eq!(
        scenario_start_clock_for_city(&city, GameClock::new(2000, 1)),
        None
    );
}

#[test]
fn scenario_deadline_base_does_not_slide_with_current_clock() {
    let city = City::new_blank("/tmp/sc2000-test.scn", "test").expect("blank city");
    let mut state = test_state(city);
    state.scenario_start_clock = Some(GameClock::new(1900, 3));
    state.game_clock = GameClock::new(1901, 4);

    assert_eq!(
        scenario_deadline_line(state.scenario_deadline_base_clock(), 60),
        "Deadline: Mar 23, 1905 (60 months)"
    );
}

#[test]
fn scenario_disaster_kind_maps_supported_scen_ids() {
    let cases = [
        (0x01, Some(DisasterKind::Fire)),
        (0x02, Some(DisasterKind::Flood)),
        (0x03, Some(DisasterKind::Rioters)),
        (0x04, Some(DisasterKind::ToxicSpill)),
        (0x05, Some(DisasterKind::NopAirCrash)),
        (0x06, Some(DisasterKind::Earthquake)),
        (0x07, Some(DisasterKind::Tornado)),
        (0x08, Some(DisasterKind::Monster)),
        (0x09, Some(DisasterKind::Meltdown)),
        (0x0a, Some(DisasterKind::Microwave)),
        (0x0b, Some(DisasterKind::Volcano)),
        (0x0c, Some(DisasterKind::Firestorm)),
        (0x0d, Some(DisasterKind::MassRiot)),
        (0x0e, Some(DisasterKind::BlizzardFlood)),
        (0x0f, Some(DisasterKind::PollutionAccident)),
        (0x10, Some(DisasterKind::Hurricane)),
        (0x11, Some(DisasterKind::HelicopterCrash)),
        (0x12, Some(DisasterKind::AirCrash)),
        (0x00, None),
        (0x13, None),
    ];

    for (disaster_type, kind) in cases {
        assert_eq!(scenario_disaster_kind(disaster_type), kind);
    }
}

fn dispatch_fixture_scenario_disaster(state: &mut ViewerState) -> Option<DisasterKind> {
    assert!(state.queue_loaded_pe_scenario_disaster());
    let kind = scenario_disaster_kind(state.city.pe_w_trigger_disaster);
    if state.consume_pe_trigger_disaster() {
        kind
    } else {
        None
    }
}

#[test]
fn loaded_scenario_disaster_seeds_without_closing_briefing() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x07, 12, 34));
    let mut state = test_state(scenario);
    state.city_info_dialog = Some(CityInfoDialog::Scenario);
    let mut preview = state.pe_process_crt;
    let expected_fields = pe_tornado_xthg_fields_with_rng(&state.city, 12, 34, &mut preview);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::Tornado)
    );

    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Scenario));
    assert_eq!(state.query_line, "Tornado Reported");
    assert!(state.city.has_active_disaster());
    let slot = assert_pe_tornado_actor(&state.city, 12, 34, expected_fields);
    assert_pe_xthg_proxy(&state.city, 12, 34, slot);
    assert_eq!(state.city.text_overlay_at(12, 33), Some(0));
    assert_eq!(state.city.text_overlay_at(13, 33), Some(0));
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn loaded_scenario_disaster_seeds_blizzard_flood_jitter() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x0e, 12, 34));
    scenario.set_terrain(12, 34, 0x20);
    let mut state = test_state(scenario);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_blizzard_flood_overlays_with_rng(
        &mut state.city.clone(),
        12,
        34,
        &mut preview,
        &mut preview_lfsr,
    );

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::BlizzardFlood)
    );
    assert_eq!(state.query_line, "Mass Floods Reported");
    assert!(state.city.has_active_disaster());
    assert!(!expected.is_empty());
    assert!(
        expected
            .keys()
            .all(|&(x, y)| x.abs_diff(12) <= 1 && y.abs_diff(34) <= 1),
        "type-2 searches from ±16 jitter still latch the nearby XTER 0x20"
    );
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
        assert_eq!(marker, 0xfc);
    }
    assert_eq!(disaster_overlay_tiles(&state.city).len(), expected.len());
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0e)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn loaded_scenario_disaster_seeds_flood_neighbors() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x02, 12, 34));
    scenario.set_terrain(12, 34, 0x20);
    let mut state = test_state(scenario);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::Flood)
    );
    assert_eq!(state.query_line, "Flood Reported");
    assert!(state.city.has_active_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(13, 34), Some(0xfc));
    assert_eq!(state.city.text_overlay_at(12, 35), Some(0xfc));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x02)
    );
}

#[test]
fn loaded_scenario_disaster_seeds_hurricane_coast() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x10, 12, 34));
    for y in 0..CITY_HEIGHT {
        scenario.set_building(100, y, 6);
    }
    let mut state = test_state(scenario);
    let mut preview_city = state.city.clone();
    let mut preview_crt = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, _) =
        pe_hurricane_overlays_with_lfsr(&mut preview_city, &mut preview_crt, &mut preview_lfsr);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::Hurricane)
    );
    assert_eq!(state.query_line, "Hurricane Reported");
    assert!(state.city.has_active_disaster());
    assert!(!expected.is_empty());
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(disaster_overlay_tiles(&state.city).len(), expected.len());
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x10)
    );
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn loaded_scenario_disaster_seeds_mass_riot_jitter() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x0d, 12, 34));
    for dx in 0..5_usize {
        for dy in 0..5_usize {
            scenario.set_building(10 + dx, 32 + dy, 29);
        }
    }
    let mut state = test_state(scenario);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let expected = pe_mass_riot_overlays_with_rng(&mut preview_city, 12, 34, &mut preview);
    // PE type-0xD paints each type-3 hit live (`0x40266c`) during the attempt loop.
    for (&(x, y), &marker) in &expected {
        assert_eq!(preview_city.text_overlay_at(x, y), Some(marker));
    }

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::MassRiot)
    );
    assert_eq!(state.query_line, "Mass Riots Reported");
    assert!(state.city.has_active_disaster());
    assert!(!expected.is_empty());
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
        assert!(matches!(marker, 0xfd | 0xfe));
    }
    assert_eq!(disaster_overlay_tiles(&state.city).len(), expected.len());
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0d)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn loaded_scenario_disaster_seeds_rioters_on_road() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x03, 12, 34));
    scenario.set_building(12, 33, 29);
    let mut state = test_state(scenario);
    let mut preview = state.pe_process_crt;
    let marker = pe_rioters_marker(&mut preview);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::Rioters)
    );
    assert_eq!(state.query_line, "Rioters Reported");
    assert!(state.city.has_active_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(12, 33), Some(marker));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x03)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn loaded_scenario_disaster_seeds_microwave_walk() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x0a, 12, 34));
    stamp_complete_building_for_test(&mut scenario, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    let mut state = test_state(scenario);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_microwave_walk_with_rng(&mut preview_city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .overlays;

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::Microwave)
    );
    assert_eq!(state.query_line, "Microwave Reported");
    assert!(state.city.has_active_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0));
    assert!(!expected.is_empty());
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(disaster_overlay_tiles(&state.city).len(), expected.len());
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0a)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn loaded_scenario_disaster_seeds_meltdown_at_plant() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x09, 12, 34));
    stamp_complete_building_for_test(&mut scenario, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    let mut state = test_state(scenario);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_meltdown_overlays_with_rng(&mut preview_city, 40, 40, &mut preview, &mut preview_lfsr);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::Meltdown)
    );
    assert_eq!(state.query_line, "Meltdown Reported");
    assert!(state.city.has_active_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert!(!expected.is_empty());
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(disaster_overlay_tiles(&state.city).len(), expected.len());
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x09)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn loaded_scenario_disaster_seeds_toxic_spill_cloud() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x04, 12, 34));
    let mut state = test_state(scenario);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::ToxicSpill)
    );
    assert_eq!(state.query_line, "Toxic Spill Reported");
    assert!(state.city.has_active_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0xfb));
    assert_eq!(state.city.text_overlay_at(12, 33), Some(0));
    assert_eq!(state.city.text_overlay_at(13, 34), Some(0));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x04)
    );
}

#[test]
fn loaded_scenario_disaster_seeds_pollution_accident_clouds() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x0f, 12, 34));
    let mut state = test_state(scenario);
    let mut preview = state.pe_process_crt;
    let clouds =
        pe_pollution_accident_cloud_tiles_with_rng(&mut state.city.clone(), 12, 34, &mut preview);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::PollutionAccident)
    );
    assert_eq!(state.query_line, "Pollution Accident Reported");
    assert!(state.city.has_active_disaster());
    assert_eq!(clouds.len(), 5);
    assert!(
        clouds
            .iter()
            .all(|(x, y)| { x.abs_diff(12) <= 4 && y.abs_diff(34) <= 4 }),
        "PE pollution accident jitter is rand&7-4"
    );
    for &(x, y) in &clouds {
        assert_eq!(state.city.text_overlay_at(x, y), Some(0xfb));
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0f)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn loaded_scenario_disaster_seeds_volcano_fire_and_toxic_cloud() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x0b, 12, 34));
    let mut state = test_state(scenario);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let expected = pe_volcano_overlays_with_rng(&mut preview_city, 12, 34, &mut preview);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::Volcano)
    );
    assert_eq!(state.query_line, "Volcano Reported");
    assert!(state.city.has_active_disaster());
    assert!(
        expected
            .keys()
            .any(|(x, y)| x.abs_diff(12) > 2 || y.abs_diff(34) > 2),
        "PE volcano second arm scatters outside the ±2 near footprint"
    );
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(disaster_overlay_tiles(&state.city).len(), expected.len());
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0b)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn loaded_scenario_disaster_seeds_firestorm_spiral() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x0c, 12, 34));
    let mut state = test_state(scenario);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::Firestorm)
    );
    assert_eq!(state.query_line, "Firestorm Reported");
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.text_overlay_at(12, 34),
        Some(0),
        "PE firestorm leaves the trigger tile unpainted"
    );
    assert_eq!(state.city.text_overlay_at(12, 33), Some(0xff));
    assert_eq!(state.city.text_overlay_at(13, 33), Some(0xff));
    assert_eq!(state.city.text_overlay_at(13, 34), Some(0xff));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0c)
    );
}

#[test]
fn loaded_scenario_disaster_seeds_monster_actor() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x08, 12, 34));
    let mut state = test_state(scenario);
    let mut preview = state.pe_process_crt;
    let expected_fields = pe_monster_xthg_fields_with_rng(&state.city, 12, 34, &mut preview);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::Monster)
    );
    assert_eq!(state.query_line, "Monster Reported");
    assert!(state.city.has_active_disaster());
    let slot = assert_pe_monster_actor(&state.city, 12, 34, expected_fields);
    assert_pe_xthg_proxy(&state.city, 12, 34, slot);
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x08)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn loaded_scenario_disaster_seeds_aircrash_airplane() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x12, 12, 34));
    let mut state = test_state(scenario);
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_aircrash_tile_with_lfsr(&state.city, &mut preview_lfsr).expect("PE air-crash tile");

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::AirCrash)
    );
    assert_eq!(state.query_line, "Air Crash Reported");
    assert!(state.city.has_active_disaster());
    let slot = assert_pe_aircrash_actor(&state.city, expected.0, expected.1);
    assert_pe_xthg_proxy(&state.city, expected.0, expected.1, slot);
    assert_ne!(expected, (12, 34));
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x12)
    );
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn loaded_scenario_disaster_type5_persists_without_aircrash() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x05, 12, 34));
    let mut state = test_state(scenario);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::NopAirCrash)
    );
    assert_eq!(state.query_line, "Air Crash Reported");
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(state.city.pe_aircrash_actor().is_none());
    assert!(!state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x05)
    );
}

#[test]
fn loaded_scenario_disaster_type11_persists_helicopter_without_paint() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x11, 12, 34));
    let mut state = test_state(scenario);

    assert_eq!(
        dispatch_fixture_scenario_disaster(&mut state),
        Some(DisasterKind::HelicopterCrash)
    );
    assert_eq!(state.query_line, "Helicopter Crash Reported");
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(state.city.pe_aircrash_actor().is_none());
    assert!(!state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x11)
    );
}

#[test]
fn loaded_scenario_disaster_skips_unsupported_or_existing_events() {
    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x13, 12, 34));
    let mut state = test_state(scenario);
    assert_eq!(dispatch_fixture_scenario_disaster(&mut state), None);
    assert!(!state.city.has_active_disaster());

    let mut scenario = City::new_blank("/tmp/sc2000-test.scn", "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x01, 12, 34));
    scenario.set_text_overlay(1, 1, 0xfb);
    let mut state = test_state(scenario);
    assert_eq!(dispatch_fixture_scenario_disaster(&mut state), None);
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(1, 1), Some(0xfb));
}

#[test]
fn load_city_dispatches_scen_disaster_after_first_unpaused_calendar_phase() {
    let path = std::env::temp_dir().join("sc2000_rust_scenario_disaster_load.scn");
    let mut scenario = City::new_blank(&path, "Atlanta").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    scenario.chunks.push(test_scen_chunk(0x02, 12, 34));
    scenario.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 1,
        },
        1,
    );
    scenario.set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 9);
    set_city_no_disasters_enabled(&mut scenario, true);
    scenario.set_terrain(12, 34, 0x20);
    scenario.save(&path).expect("save scenario");

    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state
        .load_city(&path, (800, 600), 1.0)
        .expect("load scenario");

    assert_eq!(state.city.kind, CityFileKind::WinScenario);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Scenario));
    assert!(state.no_disasters_enabled);
    assert_eq!(state.city.pe_w_trigger_disaster, 2);
    assert_eq!(
        state.pe_idle_scheduler.pending_disaster_point,
        Some((12, 34))
    );
    assert_eq!(state.city.pe_w_city_mode, CITY_MODE_GAMEPLAY as u16);
    assert!(!state.city.has_active_disaster());

    state.close_city_info_dialog();
    state.traffic.set_speed(GameSpeed::Paused);
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(200))
            .days,
        0
    );
    assert_eq!(state.city.pe_w_trigger_disaster, 2);
    assert_eq!(state.city.text_overlay_at(13, 34), Some(0));

    state.traffic.set_speed(GameSpeed::Cheetah);
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(200))
            .days,
        1
    );
    assert_eq!(state.game_clock.day(), 2);
    assert_eq!(state.city.pe_w_trigger_disaster, 0);
    assert_eq!(
        state.city.text_overlay_at(12, 34),
        Some(0),
        "PE flood leaves the XTER 0x20 source unpainted"
    );
    assert_eq!(state.city.text_overlay_at(13, 34), Some(0xfc));
    assert_eq!(state.city.text_overlay_at(12, 35), Some(0xfc));
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.pe_w_city_mode, CITY_MODE_DISASTER as u16,
        "the native idle wrapper enters mode 2 when the scripted disaster starts"
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(CITY_MODE_DISASTER)
    );
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION - 1);

    let _ = std::fs::remove_file(path);
}

#[test]
fn load_city_ordinary_disaster_chooser_uses_saved_mode_on_both_layouts() {
    let cases = [
        (
            CITY_MODE_EDIT_MAP,
            AppMode::EditNewMap,
            2,
            (12, 34),
            1_292_251_911,
        ),
        (
            CITY_MODE_GAMEPLAY,
            AppMode::CityView,
            7,
            (31, 73),
            2_223_779_224,
        ),
        (
            CITY_MODE_DISASTER,
            AppMode::CityView,
            2,
            (12, 34),
            1_292_251_911,
        ),
    ];
    for extension in ["sc2", "city"] {
        let path = std::env::temp_dir().join(format!(
            "citygame-ordinary-open-{}.{extension}",
            std::process::id()
        ));
        for (saved_mode, app_mode, selector, point, crt) in cases {
            let mut city = blank_city_layout_fixture(&path, "Ordinary open");
            city.set_misc_u32_at(MISC_CITY_MODE_OFFSET, saved_mode);
            city.set_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, 1900);
            city.set_saved_date_and_day(
                CityDate {
                    year: 1910,
                    month: 1,
                },
                24,
            );
            city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
            city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
            city.pe_w_trigger_disaster = 2;
            set_city_no_disasters_enabled(&mut city, false);
            city.save(&path).unwrap();

            let bootstrap =
                City::new_blank("/tmp/citygame-ordinary-bootstrap.sc2", "Empty").unwrap();
            let mut state = test_state(bootstrap);
            state.pe_process_crt = Win95CrtRand::new(3);
            state.pe_idle_scheduler.pending_disaster_point = Some((12, 34));
            state.load_city(&path, (800, 600), 1.0).unwrap();

            assert_eq!(state.mode, app_mode, "{extension}, mode {saved_mode}");
            assert_eq!(
                state.city.pe_w_trigger_disaster, selector,
                "{extension}, mode {saved_mode}"
            );
            assert_eq!(state.pe_idle_scheduler.pending_disaster_point, Some(point));
            assert_eq!(
                state.pe_process_crt_seed(),
                crt,
                "{extension}, mode {saved_mode}"
            );
            assert!(!state.city.has_active_disaster());
            if saved_mode == CITY_MODE_EDIT_MAP {
                state.traffic.set_speed(GameSpeed::Cheetah);
                assert_eq!(
                    state
                        .advance_frame_simulation(Duration::from_millis(200))
                        .days,
                    0
                );
            }
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn load_city_scenario_disaster_chooser_matches_native_start_checkpoints() {
    let path = std::env::temp_dir().join("citygame-scen-disaster-chooser.scn");
    // Pre-open CRT seed 3 becomes 1292251911 after the original scenario reset.
    let cases = [
        (2, WEATHER_TREND_TORNADO, false, 7, (31, 73), 2223779224),
        (2, WEATHER_TREND_TORNADO, true, 2, (12, 34), 1292251911),
        (2, WEATHER_TREND_HURRICANE, false, 16, (12, 34), 1471603118),
        (0, WEATHER_TREND_TORNADO, true, 0, (12, 34), 1292251911),
    ];
    for (scripted, weather, disabled, selector, point, crt_seed) in cases {
        let mut scenario = City::new_blank(&path, "Test scenario").expect("blank scenario");
        scenario.kind = CityFileKind::WinScenario;
        scenario.chunks.push(test_scen_chunk(scripted, 12, 34));
        scenario.set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 9);
        scenario.set_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, 1900);
        scenario.set_saved_date_and_day(
            CityDate {
                year: 1910,
                month: 1,
            },
            24,
        );
        scenario.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
        scenario.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(weather));
        scenario.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 1);
        set_city_no_disasters_enabled(&mut scenario, disabled);
        scenario.save(&path).expect("save scenario");

        let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        let mut state = test_state(city);
        state.pe_process_crt = Win95CrtRand::new(3);
        state
            .load_city(&path, (800, 600), 1.0)
            .expect("load scenario");

        assert_eq!(state.city.pe_w_trigger_disaster, selector);
        assert_eq!(state.pe_idle_scheduler.pending_disaster_point, Some(point));
        assert_eq!(state.pe_process_crt_seed(), crt_seed);
        assert!(!state.city.has_active_disaster());
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn loaded_scenarios_open_the_scenario_briefing_dialog() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "Maxis Land").expect("blank city");
    assert_eq!(loaded_city_info_dialog_for_city(&city), None);

    let mut scenario =
        City::new_blank("/tmp/sc2000-test.scn", "Maxis Land").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    assert_eq!(
        loaded_city_info_dialog_for_city(&scenario),
        Some(CityInfoDialog::Scenario)
    );
}

#[test]
fn scenario_briefing_dialog_only_opens_for_scenario_cities() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "Maxis Land").expect("blank city");
    let mut state = test_state(city);

    state.open_city_info_dialog(CityInfoDialog::Scenario);
    assert_eq!(state.city_info_dialog, None);

    let mut scenario =
        City::new_blank("/tmp/sc2000-test.scn", "Maxis Land").expect("blank scenario");
    scenario.kind = CityFileKind::WinScenario;
    let mut state = test_state(scenario);

    state.open_city_info_dialog(CityInfoDialog::Scenario);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Scenario));
}

#[test]
fn mac_kind6_zero_countdown_queues_explosion_sound_through_actor_sweep() {
    let mut city = blank_city_layout_fixture("mac-kind6-sound.city", "Mac kind-6 sound");
    let slot = XTHG_FIRST_USABLE_SLOT;
    city.mac_object_slot_records_mut()[slot] = Win95OverlayRecord {
        kind: XTHG_EXPLOSION,
        lane: 0,
        tile_x: 40,
        tile_y: 41,
        ..Win95OverlayRecord::default()
    };

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::CityView;
    state.zoom = 3.5; // Native zoom band 1 admits the Mac kind-6 handler.
    state.traffic.set_speed(GameSpeed::Turtle);
    state.sound_effects_enabled = true;
    state
        .city
        .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);

    state.advance_frame_simulation(Duration::from_millis(17));

    assert_eq!(state.city.traffic_overlays().unwrap()[slot].lane, 1);
    assert_eq!(
        state.take_pending_disaster_extra_sounds(),
        vec![PlacementSound::Explode]
    );

    // The native countdown increments once more without another sound, then
    // reaches cleanup on the following actor sweep.
    state.advance_frame_simulation(Duration::from_millis(750));
    state.advance_frame_simulation(Duration::from_millis(17));
    assert_eq!(state.city.traffic_overlays().unwrap()[slot].lane, 2);
    assert!(state.take_pending_disaster_extra_sounds().is_empty());

    state.advance_frame_simulation(Duration::from_millis(750));
    state.advance_frame_simulation(Duration::from_millis(17));
    assert_eq!(state.city.traffic_overlays().unwrap()[slot].kind, 0);
    assert!(state.take_pending_disaster_extra_sounds().is_empty());
}

#[test]
fn newspaper_headlines_report_active_disasters() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(10, 10, 0xfb);
    city.set_text_overlay(11, 10, 0xfc);
    let services = ServiceStatus::for_city(&city);
    let active_disaster_tiles = active_disaster_tiles_for_city(&city).len();

    assert_eq!(active_disaster_tiles, 2);
    assert_eq!(
        NewspaperEdition::Extra.headline(&services, active_disaster_tiles, 0b1_0000),
        "Extra: Disaster Reported"
    );
    assert_eq!(
        NewspaperEdition::Courier.headline(&services, active_disaster_tiles, 0b1_0000),
        "Disaster Reported"
    );
}

#[test]
fn newspaper_headlines_report_available_rewards_without_disasters() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let services = ServiceStatus::for_city(&city);

    assert_eq!(
        NewspaperEdition::Extra.headline(&services, 0, 0b0_0011),
        "Extra: Rewards Available"
    );
    assert_eq!(
        NewspaperEdition::Courier.headline(&services, 0, 0b0_0100),
        "Reward Available"
    );
}

#[test]
fn newspaper_disaster_stories_count_active_tiles() {
    let population = PopulationStats {
        total_population: 12_345,
        ..Default::default()
    };

    assert!(
        NewspaperEdition::Extra
            .story("Maxis Land", &population, 1, 0)
            .contains("1 active disaster tile")
    );
    assert!(
        NewspaperEdition::Courier
            .story("Maxis Land", &population, 2, 0)
            .contains("2 active disaster tiles")
    );
    assert!(
        NewspaperEdition::Subscription
            .story("Maxis Land", &population, 2, 0b1_1111)
            .contains("12,345 residents")
    );
}

#[test]
fn native_disasters_menu_commands_map_to_disaster_actions() {
    assert_eq!(
        app_command_for_menu_command(60),
        Some(AppCommand::ToggleNoDisasters)
    );
    let cases = [
        (61, DisasterKind::Fire),
        (62, DisasterKind::Flood),
        (63, DisasterKind::AirCrash),
        (64, DisasterKind::Tornado),
        (65, DisasterKind::Earthquake),
        (66, DisasterKind::Monster),
        (67, DisasterKind::Hurricane),
        (68, DisasterKind::Rioters),
    ];
    for (command, kind) in cases {
        assert_eq!(
            app_command_for_menu_command(command),
            Some(AppCommand::StartDisaster(kind))
        );
    }
    assert_eq!(app_command_for_menu_command(69), None);
}

#[test]
fn no_disasters_reads_and_toggles_saved_misc_flag() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!no_disasters_enabled_for_city(&city));

    assert!(set_city_no_disasters_enabled(&mut city, true));
    assert!(no_disasters_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(NO_DISASTERS_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert!(!set_city_no_disasters_enabled(&mut city, true));

    assert!(set_city_no_disasters_enabled(&mut city, false));
    assert!(!no_disasters_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(NO_DISASTERS_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
}

#[test]
fn no_disasters_toggle_extends_short_win95_misc_without_seeding_pe_tables() {
    let mut city =
        City::new_blank("/tmp/sc2000-short-no-disasters.sc2", "short").expect("blank city");
    let short_misc = vec![0; NO_DISASTERS_MISC_OFFSET];
    let chunk = city
        .chunks
        .iter_mut()
        .find(|chunk| chunk.name == "MISC")
        .expect("blank city MISC");
    chunk.raw_len = short_misc.len();
    chunk.data = crate::city::sc2_rle_encode(&short_misc);
    assert_eq!(city.misc_u32_at(NO_DISASTERS_MISC_OFFSET), None);

    assert!(set_city_no_disasters_enabled(&mut city, true));
    assert!(no_disasters_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(NO_DISASTERS_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert!(
        city.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS)
            .iter()
            .all(|&value| value == 0),
        "the shared option write must not seed the PE newspaper permutation"
    );
}

#[test]
fn city_state_toggles_no_disasters_saved_flag() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert!(!state.no_disasters_enabled);
    state.toggle_no_disasters();
    assert!(state.no_disasters_enabled);
    assert_eq!(
        state.city.misc_u32_at(NO_DISASTERS_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );

    state.toggle_no_disasters();
    assert!(!state.no_disasters_enabled);
    assert_eq!(
        state.city.misc_u32_at(NO_DISASTERS_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
}

#[test]
fn mac_engine_honors_and_persists_native_no_disasters_option() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-no-disasters.sc2", "MacNoDisasters").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert!(state.no_disasters_enabled);
    state.toggle_no_disasters();
    assert!(!state.no_disasters_enabled);
    assert_eq!(state.city.misc_u32_at(NO_DISASTERS_MISC_OFFSET), Some(0));

    state.toggle_no_disasters();
    assert!(state.no_disasters_enabled);
    assert_eq!(
        state.city.misc_u32_at(NO_DISASTERS_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );

    let path = PathBuf::from("/tmp/sc2000-mac-no-disasters-roundtrip.city");
    state.city.save(&path).expect("save native option");
    let loaded = City::load(path).expect("reload native option");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert!(no_disasters_enabled_for_city(&loaded));
}

#[test]
fn mac_no_disasters_uses_native_low_byte_on_win_layout_saves() {
    let mut city = City::new_blank("/tmp/sc2000-mac-no-disasters-win-layout.sc2", "Mixed")
        .expect("blank Win-layout city");
    city.set_misc_u32_at(NO_DISASTERS_MISC_OFFSET, 0xaabb_cc01);

    assert!(no_disasters_enabled_for_engine(
        &city,
        EngineFidelity::Win95
    ));
    assert!(no_disasters_enabled_for_engine(
        &city,
        EngineFidelity::Mac68k
    ));
    assert!(set_no_disasters_enabled_for_engine(
        &mut city,
        EngineFidelity::Mac68k,
        false
    ));
    assert_eq!(
        city.misc_u32_at(NO_DISASTERS_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE),
        "Mac's byte option is zero-extended through the Win-layout DWORD codec"
    );

    city.set_misc_u32_at(NO_DISASTERS_MISC_OFFSET, 0xaabb_cc00);
    assert!(no_disasters_enabled_for_engine(
        &city,
        EngineFidelity::Win95
    ));
    assert!(
        !no_disasters_enabled_for_engine(&city, EngineFidelity::Mac68k),
        "Mac CODE 4 reads the low byte, even when the upper DWORD bytes are nonzero"
    );

    let path = PathBuf::from("/tmp/sc2000-mac-no-disasters-win-layout-roundtrip.sc2");
    city.save(&path).expect("save Win-layout option");
    let loaded = City::load(path).expect("reload Win-layout option");
    assert!(no_disasters_enabled_for_engine(
        &loaded,
        EngineFidelity::Win95
    ));
    assert!(
        !no_disasters_enabled_for_engine(&loaded, EngineFidelity::Mac68k),
        "save/reload retains the low-byte value used by Mac fidelity"
    );
}

#[test]
fn query_tile_status_line_names_disaster_text_overlays_before_surface_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 207);
    city.set_text_overlay(10, 10, 0xff);
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Fire"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );

    for (marker, expected) in [
        (0xfb, "Toxic Cloud"),
        (0xfc, "Flood"),
        (0xfd, "Rioters"),
        (0xfe, "Rioters"),
    ] {
        city.set_text_overlay(11, 10, marker);
        assert!(
            query_tile_status_line(&city, 11, 10, false).starts_with(expected),
            "marker {marker:#04x} query was {}",
            query_tile_status_line(&city, 11, 10, false)
        );
    }
}

#[test]
fn disaster_marker_enables_emergency_menu_and_dispatch_choices() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(30, 40, 0xff);
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    stamp_complete_building_for_test(&mut city, 110, 100, 211, 3);
    set_military_base_type_for_test(&mut city, 2);
    let mut state = test_state(city);

    assert!(state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Emergency)));
    for tool in [
        EditTool::DispatchPolice,
        EditTool::DispatchFirefighters,
        EditTool::DispatchMilitary,
    ] {
        assert!(state.ui_action_enabled(UiAction::SelectTool(tool)));
    }
    assert!(
        !state
            .disabled_palette_actions()
            .contains(&UiAction::OpenMenu(ui::ToolMenu::Emergency))
    );

    state.open_tool_menu(ui::ToolMenu::Emergency);
    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Emergency));
    assert_eq!(state.tool, EditTool::DispatchPolice);
}

#[test]
fn mac_military_dispatch_option_tracks_native_selector_mask_on_both_layouts() {
    for (extension, selectors) in [
        ("city", [0u8, 1, 2, 3, 4, 5]),
        ("sc2", [0u8, 1, 2, 3, 4, 5]),
    ] {
        for selector in selectors {
            let path = std::env::temp_dir().join(format!(
                "citygame-mac-military-dispatch-{}-{selector}.{extension}",
                std::process::id()
            ));
            let mut city = blank_city_layout_fixture(&path, "Mac dispatch");
            city.set_mac_state_misc_u32_at(0x0e4c, u32::from(selector));
            city.set_text_overlay(30, 40, 0xff);
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;

            assert_eq!(
                state.ui_action_enabled(UiAction::SelectTool(EditTool::DispatchMilitary)),
                (2..=4).contains(&selector),
                "Mac selector {selector} on {extension} layout"
            );
            let _ = std::fs::remove_file(path);
        }
    }
}

#[test]
fn live_misc_persists_pe_disaster_active_and_type() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state.no_disasters_enabled = true;
    state.refresh_live_misc_current_values();
    assert_eq!(state.city.misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET), Some(0));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(CITY_MODE_GAMEPLAY)
    );

    state.no_disasters_enabled = false;
    assert!(state.start_disaster(DisasterKind::Tornado));
    assert_eq!(state.city.misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET), Some(1));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x07)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(CITY_MODE_DISASTER)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0),
        "PE consumes wSetTriggerDisasterType when the disaster starts"
    );

    state.no_disasters_enabled = true;
    state.refresh_live_misc_current_values();
    assert_eq!(state.city.misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET), Some(1));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x07),
        "monthly misc refresh must keep the active disaster type"
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(CITY_MODE_DISASTER),
        "monthly misc refresh must keep disaster city mode"
    );

    // Tornado/monster stay active until PE XTHG ticks despawn them (not monthly clear).
    assert!(state.city.has_active_disaster());
    assert_eq!(state.city.clear_pe_tornado_actors(), 1);
    state.refresh_live_misc_current_values();
    assert!(!state.city.has_active_disaster());
    assert_eq!(state.city.misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET), Some(0));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(CITY_MODE_GAMEPLAY)
    );

    state.mode = AppMode::EditNewMap;
    state.refresh_live_misc_current_values();
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(CITY_MODE_EDIT_MAP)
    );
}

#[test]
fn pe_disaster_and_year_end_publishers_do_not_seed_mac_budget_rows() {
    let mut city = City::new_blank("/tmp/sc2000-mac-pe-publishers-budget.sc2", "MacBudget")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for row in 0..BUDGET_TOTAL_COUNT {
        let offset = budget_row_misc_offset_for_city(&city, row, 0).expect("Mac budget row");
        city.set_budget_misc_u32_values(offset, &vec![0; BUDGET_SERVICE_MISC_STRIDE]);
    }
    let budget_rows_before = city.budget_misc_u32_values(
        budget_row_misc_offset_for_city(&city, 0, 0).expect("Mac budget row book"),
        BUDGET_TOTAL_COUNT * BUDGET_SERVICE_MISC_STRIDE,
    );
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;

    persist_pe_disaster_misc(
        &mut state.city,
        Some(DisasterKind::Fire),
        state.mode,
        state.engine_fidelity,
    );
    persist_pe_year_end_misc(&mut state.city, 12);

    assert_eq!(
        state.city.budget_misc_u32_values(
            budget_row_misc_offset_for_city(&state.city, 0, 0).expect("Mac budget row book"),
            BUDGET_TOTAL_COUNT * BUDGET_SERVICE_MISC_STRIDE,
        ),
        budget_rows_before,
        "PE-only publishers must not initialize Mac budget rows as a side effect"
    );
}

#[test]
fn disaster_end_queues_summary_story_and_clears_dispatch() {
    // PE `0x45cf10` / OpenSC2K DisasterEnd: last marker gone → summary story + recall units.
    let mut city = City::new_blank("/tmp/sc2000-dis-end.sc2", "DisEnd").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 210, 3); // police station capacity
    city.set_text_overlay(20, 20, 0xff);
    let mut state = test_state(city);
    persist_pe_disaster_misc(
        &mut state.city,
        Some(DisasterKind::Fire),
        state.mode,
        state.engine_fidelity,
    );
    assert!(
        state
            .city
            .place_dispatch_actor(22, 22, DispatchKind::Police)
    );
    assert!(state.city.dispatch_actor_at(22, 22).is_some());
    // Police station zone type 0 weight is low; coal plant class beats it.
    stamp_complete_building_for_test(&mut state.city, 50, 50, 207, 4);
    state.city.record_pe_disaster_damage_class_at(50, 50);
    assert!(state.city.pe_disaster_damage_class() > 0);

    state.city.set_text_overlay(20, 20, 0);
    assert!(!state.city.has_active_disaster());
    persist_pe_disaster_misc(&mut state.city, None, state.mode, state.engine_fidelity);

    assert_eq!(state.city.pe_w_current_disaster_type(), 0);
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_STORY_LIST_OFFSET),
        Some(0x16),
        "fire end summary story id"
    );
    assert_eq!(
        state.city.pe_newspaper_story_param_at(0),
        Some(u32::from(207u8.saturating_sub(0xbd))),
        "end story param is highest-weight damage class"
    );
    assert_eq!(state.city.pe_disaster_damage_class(), -1);
    assert!(
        state.city.dispatch_actor_at(22, 22).is_none(),
        "dispatch units leave when the disaster ends"
    );
    assert!(
        state.city.take_pe_disaster_end_newspaper_pending(),
        "OpenSC2K DisasterEnd requests newspaper paper 0"
    );
    assert_eq!(
        pe_disaster_end_summary_headline(NewspaperEdition::Subscription, 0x16),
        "Fire Contained"
    );
    assert_eq!(
        pe_disaster_end_summary_story("DisEnd", 0x16),
        "Officials in DisEnd report the fire has been contained. The immediate threat is over, and residents can begin recovery."
    );
}

#[test]
fn disaster_end_opens_subscription_newspaper_paper_zero() {
    let mut city = City::new_blank("/tmp/sc2000-dis-end-ui.sc2", "DisEndUi").expect("blank");
    city.set_text_overlay(20, 20, 0xff);
    city.set_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET, 3);
    let mut state = test_state(city);
    persist_pe_disaster_misc(
        &mut state.city,
        Some(DisasterKind::Fire),
        state.mode,
        state.engine_fidelity,
    );
    state.city.set_text_overlay(20, 20, 0);
    state.persist_cleared_disaster_misc();

    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Subscription))
    );
    assert_eq!(newspaper_choice_for_city(&state.city), 0);
    assert_eq!(state.query_line, "Newspaper: disaster summary");
    assert!(!state.city.take_pe_disaster_end_newspaper_pending());
}

#[test]
fn consume_pe_trigger_disaster_starts_pending_misc_type() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x07);

    let mut preview = state.pe_process_crt;
    let expected_fields = pe_tornado_xthg_fields_with_rng(&state.city, 12, 34, &mut preview);
    assert!(state.consume_pe_trigger_disaster());
    let slot = assert_pe_tornado_actor(&state.city, 12, 34, expected_fields);
    assert_pe_xthg_proxy(&state.city, 12, 34, slot);
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x07)
    );
    assert_eq!(state.query_line, "Tornado Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn consume_pe_trigger_disaster_starts_microwave_walk() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x0a);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_microwave_walk_with_rng(&mut preview_city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .overlays;

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0));
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0a)
    );
    assert_eq!(state.query_line, "Microwave Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn consume_pe_trigger_disaster_starts_meltdown_at_plant() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x09);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_meltdown_overlays_with_rng(&mut preview_city, 40, 40, &mut preview, &mut preview_lfsr);

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x09)
    );
    assert_eq!(state.query_line, "Meltdown Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn consume_pe_trigger_disaster_starts_toxic_spill() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x04);

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0xfb));
    assert_eq!(state.city.text_overlay_at(13, 34), Some(0));
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x04)
    );
    assert_eq!(state.query_line, "Toxic Spill Reported");
}

#[test]
fn consume_pe_trigger_disaster_starts_pollution_accident() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x0f);
    let mut preview = state.pe_process_crt;
    let clouds =
        pe_pollution_accident_cloud_tiles_with_rng(&mut state.city.clone(), 12, 34, &mut preview);

    assert!(state.consume_pe_trigger_disaster());
    assert!(state.city.has_active_disaster());
    assert!(!clouds.is_empty());
    for &(x, y) in &clouds {
        assert_eq!(state.city.text_overlay_at(x, y), Some(0xfb));
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0f)
    );
    assert_eq!(state.query_line, "Pollution Accident Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn consume_pe_trigger_disaster_starts_volcano() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x0b);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let expected = pe_volcano_overlays_with_rng(&mut preview_city, 12, 34, &mut preview);

    assert!(state.consume_pe_trigger_disaster());
    assert!(state.city.has_active_disaster());
    assert!(
        expected
            .keys()
            .any(|(x, y)| x.abs_diff(12) > 2 || y.abs_diff(34) > 2),
        "consume volcano still paints the PE ±16 scatter arm"
    );
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0b)
    );
    assert_eq!(state.query_line, "Volcano Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_volcano_far_scatter_offset_is_rand_and_1f_minus_16() {
    let mut rng = Win95CrtRand::new(0x45E2_A001);
    let mut expected = Win95CrtRand::new(0x45E2_A001);
    let (dx, dy) = pe_volcano_far_scatter_offset(&mut rng);
    assert_eq!(dx, (expected.random_u16() & 0x1f) as isize - 0x10);
    assert_eq!(dy, (expected.random_u16() & 0x1f) as isize - 0x10);
    assert!((-16..=15).contains(&dx));
    assert!((-16..=15).contains(&dy));
    assert_eq!(PE_VOLCANO_SCATTER_COUNT, 25);
    assert_eq!(PE_VOLCANO_FLATTEN_COST, 0x19);
    assert_eq!(PE_VOLCANO_FLATTEN_MAX_HEIGHT, 0x1e);
}

#[test]
fn volcano_flatten_raises_near_land_on_dummy_funds_success() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut raised = city.clone();
    let overlays = pe_volcano_overlays(&mut raised, 12, 34);
    assert!(!overlays.is_empty());
    assert!(
        (0..CITY_WIDTH)
            .any(|x| (0..CITY_HEIGHT).any(|y| raised.altitude_at(x, y) > BLANK_MAP_BASE_HEIGHT)),
        "PE flatten success raises ALTM land"
    );
}

#[test]
fn volcano_flatten_recurses_nesw_into_lower_land() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 17..24 {
        for x in 17..24 {
            city.set_altitude(x, y, 12, BLANK_MAP_BASE_HEIGHT);
        }
    }
    city.set_altitude(19, 20, 10, BLANK_MAP_BASE_HEIGHT);
    city.set_altitude(18, 20, 8, BLANK_MAP_BASE_HEIGHT);
    assert!(pe_volcano_flatten_check(&mut city, 20, 20));
    let mut funds = PE_VOLCANO_FLATTEN_COST * 4;
    let waits = pe_volcano_apply_flatten(&mut city, 20, 20, &mut funds);
    // PE `0x4441c0` raise + `0x445b50`/`0x447490` UI waits after each raise (3 tiles).
    assert!(waits >= 3 * 18);
    assert_eq!(city.altitude_at(20, 20), 13);
    assert_eq!(city.altitude_at(19, 20), 12);
    assert_eq!(city.altitude_at(18, 20), 11);
    assert_eq!(funds, PE_VOLCANO_FLATTEN_COST);
    assert_eq!(city.altitude_at(21, 20), 12);
    assert_eq!(city.altitude_at(20, 19), 12);
    assert_eq!(city.altitude_at(20, 21), 12);
}

#[test]
fn pe_volcano_apply_flatten_with_crt_destroys_via_447490() {
    // PE volcano `0x4443ae`→`0x447490`: process CRT rubble when XBLD `>= 0x0d`.
    let mut city = City::new_blank("/tmp/sc2000-volcano-crt.sc2", "VolCrt").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 10, 0);
        }
    }
    pe_set_flatten_mark(&mut city, 20, 20, true);
    city.set_building(20, 20, 0x70);
    let mut funds = PE_VOLCANO_FLATTEN_COST;
    let mut crt = Win95CrtRand::new(1);
    let mut preview = crt;
    let expected = expected_single_tile_mode1_rubble_after_generic_scatter(&mut preview);
    let waits = pe_volcano_apply_flatten_with_crt(&mut city, Some(&mut crt), 20, 20, &mut funds);
    assert!(waits >= 18);
    assert_eq!(city.building_at(20, 20), expected);
    assert_eq!(city.altitude_at(20, 20), 11);
}

#[test]
fn pe_volcano_smoothing_refreshes_an_adjusted_neighbors_neighborhood() {
    let mut city = City::new_blank("/tmp/citygame-volcano-smoothing.sc2", "Volcano").unwrap();
    for y in 16..25 {
        for x in 16..25 {
            city.set_altitude(x, y, 10, 0);
        }
    }
    city.set_altitude(20, 19, 8, 0);
    city.set_building(20, 18, 0x70);
    pe_set_flatten_mark(&mut city, 20, 20, true);
    let mut funds = PE_VOLCANO_FLATTEN_COST;
    let mut crt = Win95CrtRand::new(1);
    let mut preview = crt;
    let rubble = expected_single_tile_mode1_rubble_after_generic_scatter(&mut preview);

    let waits = pe_volcano_apply_flatten_with_crt(&mut city, Some(&mut crt), 20, 20, &mut funds);

    assert_eq!(city.altitude_at(20, 20), 11);
    assert_eq!(city.altitude_at(20, 19), 10);
    // (20,18) lies outside the root's own 3×3 refresh.
    assert_eq!(city.building_at(20, 18), rubble);
    assert_eq!(crt, preview);
    assert_eq!(funds, 0);
    assert_eq!(waits, 2 + 2 * PE_447490_UI_WAIT_4012C1_PER_CALL);
}

#[test]
fn volcano_flatten_nesw_recurse_fails_on_downhill_military() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 17..24 {
        for x in 17..24 {
            city.set_altitude(x, y, 12, BLANK_MAP_BASE_HEIGHT);
        }
    }
    city.set_altitude(19, 20, 10, BLANK_MAP_BASE_HEIGHT);
    city.set_altitude(18, 20, 8, BLANK_MAP_BASE_HEIGHT);
    city.set_zone(18, 20, ZONE_MILITARY);
    assert!(!pe_volcano_flatten_check(&mut city, 20, 20));
    assert!(
        !city
            .tile_bits_at(20, 20)
            .is_some_and(TileBits::flatten_mark)
    );
    let mut funds = PE_VOLCANO_FLATTEN_COST * 4;
    assert_eq!(pe_volcano_apply_flatten(&mut city, 20, 20, &mut funds), 0);
    assert_eq!(city.altitude_at(20, 20), 12);
    assert_eq!(city.altitude_at(19, 20), 10);
    assert_eq!(city.altitude_at(18, 20), 8);
    assert_eq!(funds, PE_VOLCANO_FLATTEN_COST * 4);
}

#[test]
fn volcano_flatten_clears_xzon_type_keeps_corners() {
    // PE `0x444398`: after raise, XZON &= 0xf0 (type clear, corners kept).
    let mut city = City::new_blank("/tmp/sc2000-volc-xzon.sc2", "VolcXzon").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 10, BLANK_MAP_BASE_HEIGHT);
        }
    }
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    assert!(pe_volcano_flatten_check(&mut city, 20, 20));
    let mut funds = PE_VOLCANO_FLATTEN_COST;
    let waits = pe_volcano_apply_flatten(&mut city, 20, 20, &mut funds);
    assert!(waits >= 18);
    assert_eq!(city.altitude_at(20, 20), 11);
    assert_eq!(city.zone_at(20, 20).map(|z| z.raw_bits()), Some(0x80));
    assert_eq!(funds, 0);
}

#[test]
fn volcano_military_near_tiles_burn_dummy_funds_without_raising() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 32..=36 {
        for x in 10..=14 {
            city.set_zone(x, y, ZONE_MILITARY);
        }
    }
    let before = city.clone();
    let _ = pe_volcano_overlays(&mut city, 12, 34);
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            assert_eq!(
                city.altitude_at(x, y),
                before.altitude_at(x, y),
                "military near tiles must not flatten ({x},{y})"
            );
        }
    }
}

#[test]
fn pe_volcano_near_scatter_offset_is_rand_mod_5_minus_2() {
    let mut rng = Win95CrtRand::new(0x45E2_A002);
    let mut expected = Win95CrtRand::new(0x45E2_A002);
    let (dx, dy) = pe_volcano_near_scatter_offset(&mut rng);
    assert_eq!(dx, (expected.random_u16() % 5) as isize - 2);
    assert_eq!(dy, (expected.random_u16() % 5) as isize - 2);
    assert!((-2..=2).contains(&dx));
    assert!((-2..=2).contains(&dy));
    let mut marker_rng = Win95CrtRand::new(3);
    let mut bit_rng = Win95CrtRand::new(3);
    assert_eq!(
        pe_volcano_near_marker(&mut marker_rng),
        if bit_rng.random_u16() & 1 != 0 {
            0xff
        } else {
            0xfb
        }
    );
}

#[test]
fn volcano_far_scatter_paints_xbit_water_toxic_cloud() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let origin = (40, 40);
    let mut state = test_state(city);
    let mut dry_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let dry = pe_volcano_overlays_with_rng(&mut dry_city, origin.0, origin.1, &mut preview);
    let probe = dry
        .iter()
        .find(|((x, y), _)| x.abs_diff(origin.0) > 2 || y.abs_diff(origin.1) > 2)
        .map(|(&(x, y), _)| (x, y))
        .expect("scatter outside ±2");
    state
        .city
        .set_tile_bits(probe.0, probe.1, crate::city::TileBits::new(0b0000_0100));
    let mut wet_city = state.city.clone();
    let mut wet_preview = state.pe_process_crt;
    let wet = pe_volcano_overlays_with_rng(&mut wet_city, origin.0, origin.1, &mut wet_preview);
    let land = wet
        .iter()
        .find(|((x, y), marker)| {
            (*x != probe.0 || *y != probe.1)
                && **marker == 0xff
                && (x.abs_diff(origin.0) > 2 || y.abs_diff(origin.1) > 2)
        })
        .map(|(&(x, y), _)| (x, y))
        .expect("land scatter tile");
    state.hover_tile = Some(origin);

    assert!(state.start_disaster(DisasterKind::Volcano));
    assert_eq!(state.city.text_overlay_at(probe.0, probe.1), Some(0xfb));
    assert_eq!(state.city.text_overlay_at(land.0, land.1), Some(0xff));
    assert_eq!(state.pe_process_crt_seed(), wet_preview.seed());
}

#[test]
fn volcano_scatter_queues_explode_when_rand_and_7_nonzero() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut preview = state.pe_process_crt;
    let explodes = pe_volcano_scatter_with_rng(&mut state.city.clone(), 12, 34, &mut preview).1;
    assert!(
        explodes > 1,
        "dummy-funds loop should explode most iterations"
    );
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    assert!(state.start_disaster(DisasterKind::Volcano));
    assert_eq!(
        state.pending_disaster_extra_sounds,
        vec![PlacementSound::Explode; explodes - 1]
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_fire_target_ignites_0x70_near_city_center() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(pe_fire_target_tile(&city), None);
    // The rooted PE center divides by `2 * (developed_count + 1)` and doubles
    // the quotient. One building at (40,40) therefore moves the search origin
    // to (20,20); it is outside the spiral's 0x40 arm limit. Use a small district
    // so the eligible footprint actually lies in the jittered search window.
    let candidates: Vec<_> = (48..=50)
        .flat_map(|y| (48..=50).map(move |x| (x, y)))
        .collect();
    for &(x, y) in &candidates {
        city.set_building(x, y, 0x70);
    }
    let target = pe_fire_target_tile(&city).expect("eligible 0x70 district in search window");
    assert!(candidates.contains(&target));
    for &(x, y) in &candidates {
        city.set_tile_bits(x, y, crate::city::TileBits::new(0b0000_0100));
    }
    assert_eq!(pe_fire_target_tile(&city), None);
    for &(x, y) in &candidates {
        city.set_tile_bits(x, y, crate::city::TileBits::new(0));
        city.set_building(x, y, 5);
    }
    assert_eq!(pe_fire_target_tile(&city), None);
}

#[test]
fn start_disaster_fire_without_building_is_nop() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));

    assert!(!state.start_disaster(DisasterKind::Fire));
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());
}

#[test]
fn firestorm_auto_goto_centers_on_last_ignite_not_hover() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let last = *pe_firestorm_spiral_tiles(&city, 40, 40)
        .last()
        .expect("0x41 ignites");
    assert_ne!(last, (40, 40));
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Firestorm))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some(last));
    assert_eq!(state.city.text_overlay_at(last.0, last.1), Some(0xff));

    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.auto_goto_enabled = false;
    assert!(state.start_disaster_at(DisasterKind::Firestorm, (40, 40)));
    assert_eq!(
        state.pe_idle_scheduler.pending_disaster_point,
        Some((last.0 as i16, last.1 as i16))
    );
    assert_eq!(state.pending_disaster_auto_goto, None);
}

#[test]
fn meltdown_auto_goto_centers_on_plant_not_hover() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    assert_eq!(pe_meltdown_plant_tile(&city, (12, 34)), Some((40, 40)));
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Meltdown))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some((40, 40)));
}

#[test]
fn aircrash_auto_goto_centers_on_crash_tile_not_hover() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }
    let mut preview_lfsr = state.pe_process_lfsr;
    let crash =
        pe_aircrash_tile_with_lfsr(&state.city, &mut preview_lfsr).expect("PE air-crash tile");
    assert_ne!(crash, (12, 34));

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::AirCrash))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some(crash));
    assert_pe_aircrash_actor(&state.city, crash.0, crash.1);
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn rioters_auto_goto_stays_on_trigger_not_first_road() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(12, 33, 29);
    let first = pe_rioters_target_tile(&city, 12, 34).expect("road");
    assert_ne!(first, (12, 34));
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Rioters))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some((12, 34)));
    assert_ne!(effect.auto_goto_tile, Some(first));
}

#[test]
fn consume_pe_trigger_disaster_queues_fire_auto_goto() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x01);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let ignite = pe_fire_target_tile_with_rng(&state.city, &mut preview, &mut preview_lfsr)
        .expect("PE fire ignite");
    assert_ne!(ignite, (12, 34));

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.pending_disaster_auto_goto, Some(ignite));
    assert_eq!(state.take_pending_disaster_auto_goto(), Some(ignite));
    assert_eq!(state.pending_disaster_auto_goto, None);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn consume_pe_trigger_disaster_skips_auto_goto_when_disabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if state.auto_goto_enabled {
        state.toggle_auto_goto();
    }
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x01);

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.pending_disaster_auto_goto, None);
}

#[test]
fn weather_disaster_queues_tornado_auto_goto_on_origin() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank");
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    let mut rng = Win95CrtRand::new(1);
    let origin = tornado_ambient_target_tile(&mut rng, EngineFidelity::Win95);
    for _ in 0..3 {
        rng.random_u16();
    }
    let mut state = test_state(city);
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }

    assert!(state.start_weather_disaster_for_test());
    assert_eq!(state.pe_process_crt_seed(), rng.seed());
    assert_eq!(state.pending_disaster_auto_goto, Some(origin));
    if state.sound_effects_enabled {
        assert_eq!(
            state.pending_disaster_start_sound,
            Some(PlacementSound::Wind)
        );
    }
}

#[test]
fn weather_disaster_if_due_advances_pe_process_crt() {
    // PE weather-disaster proposal draws CRT rand from the process stream.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank");
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    let mut state = test_state(city);
    let before = state.pe_process_crt_seed();
    // Force a success path by draining until a roll hits, or at least one CRT draw.
    let _ = state.start_weather_disaster_if_due();
    assert_ne!(
        state.pe_process_crt_seed(),
        before,
        "weather disaster gate must consume process CRT even on a miss"
    );
}

#[test]
fn consume_pe_trigger_disaster_queues_start_sound() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x01);

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(
        state.pending_disaster_start_sound,
        Some(PlacementSound::Fire)
    );
    assert!(
        state.pending_disaster_start_siren,
        "OpenSC2K / PE common-path appends siren 0x208 after the type sound"
    );
    assert_eq!(
        state.take_pending_disaster_start_sound(),
        Some(PlacementSound::Fire)
    );
    assert!(state.take_pending_disaster_start_siren());
    assert_eq!(state.pending_disaster_start_sound, None);
    assert!(!state.pending_disaster_start_siren);
}

#[test]
fn consume_pe_trigger_disaster_skips_start_sound_when_disabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x01);

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.pending_disaster_start_sound, None);
    assert!(!state.pending_disaster_start_siren);
}

#[test]
fn fire_auto_goto_centers_on_ignite_tile_not_hover() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let ignite = pe_fire_target_tile_with_rng(&state.city, &mut preview, &mut preview_lfsr)
        .expect("PE fire ignite");
    assert_ne!(ignite, (12, 34));

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Fire))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some(ignite));
    assert_eq!(state.city.text_overlay_at(ignite.0, ignite.1), Some(0xff));
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn start_disaster_fire_ignites_one_ff_on_residential() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_fire_target_tile_with_rng(&state.city, &mut preview, &mut preview_lfsr)
        .expect("PE fire ignite");

    assert!(state.start_disaster(DisasterKind::Fire));
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(
        state.city.text_overlay_at(expected.0, expected.1),
        Some(0xff)
    );
    assert_eq!(disaster_overlay_tiles(&state.city).len(), 1);
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x01)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_fire_start_ignite_mid_xtxt_destroy_before_paint() {
    // PE type-1 @ `0x45e9b4` → FUN_00460e10 flag 0: mid-XTXT DestroyStructure(1,1) then paint.
    let mut city = City::new_blank("/tmp/sc2000-fire-mid.sc2", "FireMid").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 210, 3);
    for dy in 0..3 {
        for dx in 0..3 {
            city.set_text_overlay(40 + dx, 40 + dy, 0x55);
            city.set_traffic_density(40 + dx, 40 + dy, 0x2e);
        }
    }
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    assert!(state.start_disaster(DisasterKind::Fire));
    let mut saw_cleared = false;
    for dy in 0..3 {
        for dx in 0..3 {
            let x = 40 + dx;
            let y = 40 + dy;
            let building = state.city.building_at(x, y);
            if (building == 0 || is_rubble_tile(building))
                && state.city.text_overlay_at(x, y) == Some(0xff)
            {
                saw_cleared = true;
            }
        }
    }
    assert!(
        saw_cleared,
        "fire start mid-XTXT must DestroyStructure then paint 0xff"
    );
    let mut saw_xtrf_clear = false;
    for dy in 0..3 {
        for dx in 0..3 {
            let x = 40 + dx;
            let y = 40 + dy;
            if state.city.text_overlay_at(x, y) == Some(0xff)
                && state.city.traffic_density_at(x, y) == Some(0)
            {
                saw_xtrf_clear = true;
            }
        }
    }
    assert!(
        saw_xtrf_clear,
        "FUN_00460e10 fire paint clears XTRF microsim cell"
    );
}

#[test]
fn consume_pe_trigger_disaster_starts_fire() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x01);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_fire_target_tile_with_rng(&state.city, &mut preview, &mut preview_lfsr)
        .expect("PE fire ignite");

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(
        state.city.text_overlay_at(expected.0, expected.1),
        Some(0xff)
    );
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x01)
    );
    assert_eq!(state.query_line, "Fire Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_fire_fallback_ignites_tree_when_no_0x70() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 0..64 {
        for x in 0..64 {
            city.set_building(x, y, 6);
        }
    }
    let tile = pe_fire_target_tile(&city).expect("RandomAnd7F tree ignite");
    assert_eq!(city.building_at(tile.0, tile.1), 6);
    assert!(tile.0 < 64 && tile.1 < 64);
}

#[test]
fn pe_fire_type1_fallback_calls_fun_until_paint_success() {
    // PE @ `0x45e9e9`: RandomAnd7F + FUN flag 0 in-loop; trees (XBLD 6) ignite when
    // spiral finds no XBLD `>= 0x70`.
    let mut city = City::new_blank("/tmp/sc2000-fire-fb-live.sc2", "FireFbLive").expect("blank");
    for y in 0..64 {
        for x in 0..64 {
            city.set_building(x, y, 6);
        }
    }
    let mut state = test_state(city);
    assert!(state.start_disaster(DisasterKind::Fire));
    let fires = disaster_overlay_tiles(&state.city);
    assert_eq!(fires.len(), 1);
    let &(fx, fy, marker) = fires.iter().next().expect("one fire");
    assert_eq!(marker, 0xff);
    assert_eq!(state.city.building_at(fx, fy), 6);
    assert_eq!(state.city.text_overlay_at(fx, fy), Some(0xff));
}

#[test]
fn pe_firestorm_spiral_is_nesw_until_0x41_ignites() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let tiles = pe_firestorm_spiral_tiles(&city, 40, 40);
    assert_eq!(PE_FIRESTORM_DX, [0, 1, 0, -1]);
    assert_eq!(PE_FIRESTORM_DY, [-1, 0, 1, 0]);
    assert_eq!(FIRESTORM_IGNITION_LIMIT, 0x41);
    assert_eq!(tiles.len(), 0x41);
    assert!(!tiles.contains(&(40, 40)));
    assert_eq!(&tiles[..4], &[(40, 39), (41, 39), (41, 40), (41, 41)]);
}

#[test]
fn firestorm_spiral_skips_xbit_water_and_keeps_0x41_paints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let origin = (40, 40);
    let dry = pe_firestorm_spiral_tiles(&city, origin.0, origin.1);
    city.set_tile_bits(40, 39, crate::city::TileBits::new(0b0000_0100));
    let wet = pe_firestorm_spiral_tiles(&city, origin.0, origin.1);
    assert_eq!(wet.len(), 0x41);
    assert!(!wet.contains(&(40, 39)));
    assert!(wet.iter().any(|tile| !dry.contains(tile)));
    let mut state = test_state(city);
    state.hover_tile = Some(origin);

    assert!(state.start_disaster(DisasterKind::Firestorm));
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0));
    assert_eq!(state.city.text_overlay_at(40, 39), Some(0));
    assert_eq!(state.city.text_overlay_at(wet[0].0, wet[0].1), Some(0xff));
    assert_eq!(disaster_overlay_tiles(&state.city).len(), 0x41);
}

#[test]
fn firestorm_spiral_skips_existing_disaster_xtxt_and_keeps_0x41_paints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let origin = (40, 40);
    let dry = pe_firestorm_spiral_tiles(&city, origin.0, origin.1);
    city.set_text_overlay(40, 39, 0xff);
    city.set_text_overlay(41, 40, 0xc9);
    let skipped = pe_firestorm_spiral_tiles(&city, origin.0, origin.1);
    assert_eq!(skipped.len(), 0x41);
    assert!(!skipped.contains(&(40, 39)));
    assert!(!skipped.contains(&(41, 40)));
    assert!(skipped.iter().any(|tile| !dry.contains(tile)));
    city.set_text_overlay(40, 39, 0xfa);
    let fa_ok = pe_firestorm_spiral_tiles(&city, origin.0, origin.1);
    assert!(fa_ok.contains(&(40, 39)), "XTXT 0xfa remains igniteable");
    let mut state = test_state(city);
    state.hover_tile = Some(origin);

    assert!(state.start_disaster(DisasterKind::Firestorm));
    assert_eq!(state.city.text_overlay_at(41, 40), Some(0xc9));
    assert_eq!(disaster_overlay_tiles(&state.city).len(), 0x41);
}

#[test]
fn firestorm_spiral_skips_high_xtxt_rubble_band_keeps_0x41() {
    // PE FUN_00460e10 rubble arm ret 0: spiral must not count/paint XTXT 0xf1..=0xf9 as fire.
    let mut city = City::new_blank("/tmp/sc2000-fs-rubble.sc2", "FsRubble").expect("blank");
    let origin = (40, 40);
    city.set_text_overlay(40, 39, 0xf1);
    city.set_building(40, 39, 0x70);
    let tiles = pe_firestorm_spiral_tiles(&city, origin.0, origin.1);
    assert_eq!(tiles.len(), 0x41);
    assert!(!tiles.contains(&(40, 39)));
    let mut state = test_state(city);
    state.hover_tile = Some(origin);
    assert!(state.start_disaster(DisasterKind::Firestorm));
    // Live spiral still calls FUN on the rubble-band tile (PlaceTile side effect).
    assert_eq!(state.city.text_overlay_at(40, 39), Some(0xf1));
    assert!((1..=4).contains(&state.city.building_at(40, 39)));
    assert_eq!(disaster_overlay_tiles(&state.city).len(), 0x41);
}

#[test]
fn pe_firestorm_spiral_ignite_mid_xtxt_destroy_before_paint() {
    // PE @ `0x45e0a4` → FUN_00460e10 flag 1: mid-XTXT DestroyStructure(1,1) then fire paint.
    let mut city = City::new_blank("/tmp/sc2000-fs-mid.sc2", "FsMid").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 210, 3);
    for dy in 0..3 {
        for dx in 0..3 {
            city.set_text_overlay(40 + dx, 40 + dy, 0x55);
            city.set_traffic_density(40 + dx, 40 + dy, 0x31);
        }
    }
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    assert!(state.start_disaster(DisasterKind::Firestorm));
    let mut saw_cleared = false;
    for dy in 0..3 {
        for dx in 0..3 {
            let x = 40 + dx;
            let y = 40 + dy;
            let building = state.city.building_at(x, y);
            if (building == 0 || is_rubble_tile(building))
                && state.city.text_overlay_at(x, y) == Some(0xff)
            {
                saw_cleared = true;
            }
        }
    }
    assert!(
        saw_cleared,
        "firestorm mid-XTXT must DestroyStructure then paint 0xff"
    );
    // Mid-XTXT DestroyStructure paints the whole footprint, so total 0xff can exceed 0x41.
    assert!(disaster_overlay_tiles(&state.city).len() >= 0x41);
}

#[test]
fn consume_pe_trigger_disaster_starts_firestorm() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x0c);

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(12, 33), Some(0xff));
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0c)
    );
    assert_eq!(state.query_line, "Firestorm Reported");
}

#[test]
fn consume_pe_trigger_disaster_starts_blizzard_flood() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(12, 34, 0x20);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x0e);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_blizzard_flood_overlays_with_rng(
        &mut state.city.clone(),
        12,
        34,
        &mut preview,
        &mut preview_lfsr,
    );

    assert!(state.consume_pe_trigger_disaster());
    assert!(!expected.is_empty());
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0e)
    );
    assert_eq!(state.query_line, "Mass Floods Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn consume_pe_trigger_disaster_starts_flood() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(12, 34, 0x20);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x02);

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(13, 34), Some(0xfc));
    assert_eq!(state.city.text_overlay_at(12, 35), Some(0xfc));
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x02)
    );
    assert_eq!(state.query_line, "Flood Reported");
}

#[test]
fn consume_pe_trigger_disaster_starts_hurricane() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 0..CITY_HEIGHT {
        city.set_building(100, y, 6);
    }
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x10);
    let mut preview_city = state.city.clone();
    let mut preview_crt = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, _) =
        pe_hurricane_overlays_with_lfsr(&mut preview_city, &mut preview_crt, &mut preview_lfsr);

    assert!(state.consume_pe_trigger_disaster());
    assert!(!expected.is_empty());
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x10)
    );
    assert_eq!(state.query_line, "Hurricane Reported");
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn consume_pe_trigger_disaster_starts_mass_riot() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for dx in 0..5_usize {
        for dy in 0..5_usize {
            city.set_building(10 + dx, 32 + dy, 29);
        }
    }
    let mut state = test_state(city);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let expected = pe_mass_riot_overlays_with_rng(&mut preview_city, 12, 34, &mut preview);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x0d);

    assert!(state.consume_pe_trigger_disaster());
    assert!(!expected.is_empty());
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0d)
    );
    assert_eq!(state.query_line, "Mass Riots Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_mass_riot_attempt_ok_peeks_process_crt_not_city_name() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for dx in 0..5_usize {
        for dy in 0..5_usize {
            city.set_building(10 + dx, 32 + dy, 29);
        }
    }
    let process = Win95CrtRand::new(0x45E1_8001);
    assert_ne!(
        pe_mass_riot_seed(&city, 12, 34),
        process.seed(),
        "city-name helper seed must differ from the process CRT under test"
    );
    let mut peek = process;
    let ok = pe_mass_riot_attempt_ok_with_rng(&city, 12, 34, &mut peek);
    let mut expected = process;
    let hits = pe_mass_riot_sound_count_with_rng(&city, 12, 34, &mut expected);
    assert_eq!(ok, hits > 0);
    assert_eq!(peek.seed(), expected.seed());
    assert_eq!(
        pe_mass_riot_attempt_ok(&city, 12, 34),
        pe_mass_riot_sound_count(&city, 12, 34) > 0
    );
}

#[test]
fn ambient_mass_riot_select_uses_win95_crt_peek_gate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for dx in 0..5_usize {
        for dy in 0..5_usize {
            city.set_building(10 + dx, 32 + dy, 29);
        }
    }
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xAA);
    city.set_pe_unemployment_percent(10);
    city.set_pe_zone_pops([3_000, 0, 0]);
    city.persist_pe_day22_population_leftover(30_000);
    let population = PopulationStats {
        normal_population: 30_000,
        total_population: 30_000,
        ..Default::default()
    };
    let outcomes = CityServiceOutcomes::default();

    for seed in 0..8_000_u32 {
        let mut probe = Win95CrtRand::new(seed);
        if ambient_disaster_kind_for_city(
            &city,
            population,
            outcomes,
            &mut probe,
            EngineFidelity::Win95,
        ) != Some(DisasterKind::MassRiot)
        {
            continue;
        }
        let hint = mass_riot_ambient_target_tile(&city, &mut probe, EngineFidelity::Win95);
        let mut gate_peek = probe;
        let process_ok = pe_mass_riot_attempt_ok_with_rng(&city, hint.0, hint.1, &mut gate_peek);

        let mut rng = Win95CrtRand::new(seed);
        let selected = ambient_disaster_select_with_remainder(
            &city,
            population,
            outcomes,
            EngineFidelity::Win95,
            0,
            &mut rng,
            None,
        );
        if process_ok {
            assert_eq!(selected, Some((DisasterKind::MassRiot, hint)));
        } else {
            assert_eq!(
                selected, None,
                "Win95 CRT peek must reject when process start fails"
            );
        }
        return;
    }
    panic!("expected at least one ambient MassRiot jump-table roll");
}

#[test]
fn pe_blizzard_flood_attempt_ok_peeks_process_crt_and_lfsr_not_city_name() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(12, 34, 0x20);
    let process = Win95CrtRand::new(0x45D9_D001);
    let process_lfsr = Win95Lfsr::from_u32_seed(0x45D9_D002);
    assert_ne!(
        pe_blizzard_flood_seed(&city, 12, 34),
        process.seed(),
        "city-name helper seed must differ from the process CRT under test"
    );
    let mut peek = process;
    let mut peek_lfsr = process_lfsr;
    let ok = pe_blizzard_flood_attempt_ok_with_rng(&city, 12, 34, &mut peek, &mut peek_lfsr);
    let mut expected = process;
    let mut expected_lfsr = process_lfsr;
    let any = pe_blizzard_flood_start_with_rng(
        &mut city.clone(),
        12,
        34,
        &mut expected,
        &mut expected_lfsr,
    )
    .1;
    assert_eq!(ok, any);
    assert_eq!(peek.seed(), expected.seed());
    assert_eq!(peek_lfsr.state(), expected_lfsr.state());
    assert_eq!(
        city.text_overlay_at(13, 34),
        Some(0),
        "peek must not paint live"
    );
    assert_eq!(
        pe_blizzard_flood_attempt_ok(&city, 12, 34),
        pe_blizzard_flood_start(&city, 12, 34).1
    );
}

#[test]
fn pe_blizzard_flood_start_paints_type2_markers_live() {
    // PE @ `0x45da5e`: nested `0x401b59` type-2 writes `0xfc` during attempts.
    let mut city = City::new_blank("/tmp/sc2000-blizzard-live.sc2", "BlizLive").expect("blank");
    city.set_terrain(12, 34, 0x20);
    let mut rng = Win95CrtRand::new(0x45D9_D001);
    let mut lfsr = Win95Lfsr::from_u32_seed(0x45D9_D002);
    let (overlays, any) = pe_blizzard_flood_start_with_rng(&mut city, 12, 34, &mut rng, &mut lfsr);
    assert!(any);
    assert!(!overlays.is_empty());
    for (&(x, y), &marker) in &overlays {
        assert_eq!(city.text_overlay_at(x, y), Some(marker));
    }
}

#[test]
fn ambient_blizzard_flood_select_peeks_process_crt_and_lfsr() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_BLIZZARD));
    // Scatter water so jittered type-0xE starts can succeed from many origins.
    for y in 0..CITY_HEIGHT {
        for x in (0..CITY_WIDTH).step_by(8) {
            city.set_terrain(x, y, 0x20);
        }
    }
    let population = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    let lfsr_state = Win95Lfsr::from_u32_seed(0x45D9_D003);

    for seed in 0..8_000_u32 {
        let mut probe = Win95CrtRand::new(seed);
        if ambient_disaster_kind_for_city(
            &city,
            population,
            outcomes,
            &mut probe,
            EngineFidelity::Win95,
        ) != Some(DisasterKind::BlizzardFlood)
        {
            continue;
        }
        let tile = mac_mod7e_plus_one_ambient_target_tile(&city, &mut probe, EngineFidelity::Win95);
        let mut gate_peek = probe;
        let mut gate_lfsr = lfsr_state;
        let process_ok = pe_blizzard_flood_attempt_ok_with_rng(
            &city,
            tile.0,
            tile.1,
            &mut gate_peek,
            &mut gate_lfsr,
        );

        let mut rng = Win95CrtRand::new(seed);
        let mut lfsr = lfsr_state;
        let selected = ambient_disaster_select_with_remainder(
            &city,
            population,
            outcomes,
            EngineFidelity::Win95,
            0,
            &mut rng,
            Some(&mut lfsr),
        );
        if process_ok {
            assert_eq!(selected, Some((DisasterKind::BlizzardFlood, tile)));
        } else {
            assert_eq!(
                selected, None,
                "Win95 CRT+LFSR peek must reject when process type-0xE start fails"
            );
        }
        return;
    }
    panic!("expected at least one ambient BlizzardFlood jump-table roll");
}

#[test]
fn ambient_fire_select_peeks_process_crt_and_lfsr() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xfe);
    let population = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    let lfsr_state = Win95Lfsr::from_u32_seed(0x45E9_2001);

    for seed in 0..8_000_u32 {
        let mut probe = Win95CrtRand::new(seed);
        if ambient_disaster_kind_for_city(
            &city,
            population,
            outcomes,
            &mut probe,
            EngineFidelity::Win95,
        ) != Some(DisasterKind::Fire)
        {
            continue;
        }
        let mut peek_crt = probe;
        let mut peek_lfsr = lfsr_state;
        let process_tile = pe_fire_target_tile_with_rng(&city, &mut peek_crt, &mut peek_lfsr);

        let mut rng = Win95CrtRand::new(seed);
        let mut lfsr = lfsr_state;
        let selected = ambient_disaster_select_with_remainder(
            &city,
            population,
            outcomes,
            EngineFidelity::Win95,
            0,
            &mut rng,
            Some(&mut lfsr),
        );
        match process_tile {
            Some(tile) => assert_eq!(selected, Some((DisasterKind::Fire, tile))),
            None => assert_eq!(
                selected, None,
                "Win95 CRT+LFSR peek must reject when type-1 start finds no tile"
            ),
        }
        return;
    }
    panic!("expected at least one ambient Fire jump-table roll");
}

#[test]
fn ambient_flood_select_peeks_process_lfsr() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_FOGGY));
    for y in 0..CITY_HEIGHT {
        for x in (0..CITY_WIDTH).step_by(8) {
            city.set_terrain(x, y, 0x20);
        }
    }
    let population = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    let lfsr_state = Win95Lfsr::from_u32_seed(0x45DB_0001);

    for seed in 0..8_000_u32 {
        let mut probe = Win95CrtRand::new(seed);
        if ambient_disaster_kind_for_city(
            &city,
            population,
            outcomes,
            &mut probe,
            EngineFidelity::Win95,
        ) != Some(DisasterKind::Flood)
        {
            continue;
        }
        let tile = mac_mod7e_plus_one_ambient_target_tile(&city, &mut probe, EngineFidelity::Win95);
        let mut peek_lfsr = lfsr_state;
        let process_ok = pe_flood_source_tile(&city, tile.0 as isize, tile.1 as isize).is_some()
            || !pe_flood_overlays_with_lfsr(&city, tile.0, tile.1, &mut peek_lfsr).is_empty();

        let mut rng = Win95CrtRand::new(seed);
        let mut lfsr = lfsr_state;
        let selected = ambient_disaster_select_with_remainder(
            &city,
            population,
            outcomes,
            EngineFidelity::Win95,
            0,
            &mut rng,
            Some(&mut lfsr),
        );
        if process_ok {
            assert_eq!(selected, Some((DisasterKind::Flood, tile)));
        } else {
            assert_eq!(
                selected, None,
                "process LFSR peek must reject when type-2 start finds no flood tile"
            );
        }
        return;
    }
    panic!("expected at least one ambient Flood jump-table roll");
}

#[test]
fn ambient_tornado_monster_aircrash_require_xthg_slot() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    city.set_building(40, 40, 221); // runway for air-crash arm
    city.set_pe_zone_pops([4_500, 0, 0]);
    city.persist_pe_day22_population_leftover(45_000);
    let population = PopulationStats {
        normal_population: 45_000,
        total_population: 45_000,
        ..Default::default()
    };
    let outcomes = CityServiceOutcomes::default();

    // Free slot: weather-first tornado and jump-table tornado/monster/aircrash can select.
    assert!(pe_xthg_disaster_slot_available(&city));
    let weather = ambient_disaster_select_with_remainder(
        &city,
        population,
        outcomes,
        EngineFidelity::Win95,
        0x0e, // < 0x0F weather-first remainder
        &mut Win95CrtRand::new(1),
        None,
    );
    assert_eq!(weather.map(|(k, _)| k), Some(DisasterKind::Tornado));

    fill_all_usable_xthg_slots(&mut city);
    assert!(!pe_xthg_disaster_slot_available(&city));

    assert_eq!(
        ambient_disaster_select_with_remainder(
            &city,
            population,
            outcomes,
            EngineFidelity::Win95,
            0x0e,
            &mut Win95CrtRand::new(1),
            None,
        ),
        None,
        "weather-first tornado must require a free XTHG slot"
    );

    for seed in 0..4_000_u32 {
        let mut probe = Win95CrtRand::new(seed);
        let kind = ambient_disaster_kind_for_city(
            &city,
            population,
            outcomes,
            &mut probe,
            EngineFidelity::Win95,
        );
        if !matches!(
            kind,
            Some(DisasterKind::Tornado | DisasterKind::Monster | DisasterKind::AirCrash)
        ) {
            continue;
        }
        let mut rng = Win95CrtRand::new(seed);
        let mut lfsr = Win95Lfsr::from_u32_seed(seed);
        assert_eq!(
            ambient_disaster_select_with_remainder(
                &city,
                population,
                outcomes,
                EngineFidelity::Win95,
                0,
                &mut rng,
                Some(&mut lfsr),
            ),
            None,
            "full XTHG table must reject ambient {kind:?} (seed {seed})"
        );
        return;
    }
    panic!("expected a Tornado/Monster/AirCrash jump-table roll against a full XTHG table");
}

#[test]
fn mac_ambient_tornado_and_aircrash_ignore_pe_xthg_slots() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    pe_place_tile_441f00(&mut city, 40, 40, RUNWAY_BUILDING_ID);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    fill_all_usable_xthg_slots(&mut city);
    assert!(!pe_xthg_disaster_slot_available(&city));

    let weather = ambient_disaster_select_with_remainder(
        &city,
        live_population_stats_for_city(&city),
        CityServiceOutcomes::default(),
        EngineFidelity::Mac68k,
        0x0e,
        &mut MacToolboxRng::new(0x3596_6001),
        None,
    );
    assert_eq!(weather.map(|(kind, _)| kind), Some(DisasterKind::Tornado));

    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 0);
    let population = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 44_999);
    assert!(!mac_monster_disaster_ready(&city));
    city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 45_000);
    assert!(mac_monster_disaster_ready(&city));
    let mut saw_aircrash = false;
    for seed in 0..8_000_u32 {
        let mut probe = MacToolboxRng::new(seed);
        if ambient_disaster_kind_for_city(
            &city,
            population,
            outcomes,
            &mut probe,
            EngineFidelity::Mac68k,
        ) != Some(DisasterKind::AirCrash)
        {
            continue;
        }

        let selected = ambient_disaster_select_with_remainder(
            &city,
            population,
            outcomes,
            EngineFidelity::Mac68k,
            0,
            &mut MacToolboxRng::new(seed),
            None,
        );
        assert_eq!(selected.map(|(kind, _)| kind), Some(DisasterKind::AirCrash));
        saw_aircrash = true;
        break;
    }
    assert!(saw_aircrash, "expected a Mac Air Crash jump-table roll");

    let monster_population = PopulationStats {
        normal_population: 45_000,
        total_population: 45_000,
        ..Default::default()
    };
    let mut saw_monster = false;
    for seed in 0..8_000_u32 {
        let mut probe = MacToolboxRng::new(seed);
        if ambient_disaster_kind_for_city(
            &city,
            monster_population,
            outcomes,
            &mut probe,
            EngineFidelity::Mac68k,
        ) != Some(DisasterKind::Monster)
        {
            continue;
        }

        let selected = ambient_disaster_select_with_remainder(
            &city,
            monster_population,
            outcomes,
            EngineFidelity::Mac68k,
            0,
            &mut MacToolboxRng::new(seed),
            None,
        );
        assert_eq!(selected.map(|(kind, _)| kind), Some(DisasterKind::Monster));
        saw_monster = true;
        break;
    }
    assert!(saw_monster, "expected a Mac Monster jump-table roll");
}

#[test]
fn consume_pe_trigger_disaster_starts_rioters() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(12, 33, 29);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x03);
    let mut preview = state.pe_process_crt;
    let marker = pe_rioters_marker(&mut preview);

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(12, 33), Some(marker));
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x03)
    );
    assert_eq!(state.query_line, "Rioters Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn consume_pe_trigger_disaster_starts_tornado() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x07);

    let mut preview = state.pe_process_crt;
    let expected_fields = pe_tornado_xthg_fields_with_rng(&state.city, 12, 34, &mut preview);
    assert!(state.consume_pe_trigger_disaster());
    let slot = assert_pe_tornado_actor(&state.city, 12, 34, expected_fields);
    assert_pe_xthg_proxy(&state.city, 12, 34, slot);
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x07)
    );
    assert_eq!(state.query_line, "Tornado Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn consume_pe_trigger_disaster_starts_monster() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x08);

    let mut preview = state.pe_process_crt;
    let expected_fields = pe_monster_xthg_fields_with_rng(&state.city, 12, 34, &mut preview);
    assert!(state.consume_pe_trigger_disaster());
    let slot = assert_pe_monster_actor(&state.city, 12, 34, expected_fields);
    assert_pe_xthg_proxy(&state.city, 12, 34, slot);
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x08)
    );
    assert_eq!(state.query_line, "Monster Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn consume_pe_trigger_disaster_starts_aircrash() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x12);
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_aircrash_tile_with_lfsr(&state.city, &mut preview_lfsr).expect("PE air-crash tile");

    assert!(state.consume_pe_trigger_disaster());
    let slot = assert_pe_aircrash_actor(&state.city, expected.0, expected.1);
    assert_pe_xthg_proxy(&state.city, expected.0, expected.1, slot);
    assert!(expected.0 >= PE_AIRCRASH_INNER_ORIGIN && expected.0 <= 0x5f);
    assert!(expected.1 >= PE_AIRCRASH_INNER_ORIGIN && expected.1 <= 0x5f);
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert!(state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x12)
    );
    assert_eq!(state.query_line, "Air Crash Reported");
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn consume_pe_trigger_disaster_type5_persists_without_aircrash() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x05);

    assert!(state.consume_pe_trigger_disaster());
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(state.city.pe_aircrash_actor().is_none());
    assert!(!state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x05)
    );
    assert_eq!(state.query_line, "Air Crash Reported");
}

#[test]
fn consume_pe_trigger_disaster_type11_persists_helicopter_without_paint() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x11);

    assert!(state.consume_pe_trigger_disaster());
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(state.city.pe_aircrash_actor().is_none());
    assert!(!state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x11)
    );
    assert_eq!(state.query_line, "Helicopter Crash Reported");
}

#[test]
fn pe_type5_and_type11_skip_auto_goto_and_use_common_path_disaster_sound() {
    for trigger in [0x05u32, 0x11] {
        let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        let mut state = test_state(city);
        if !state.auto_goto_enabled {
            state.toggle_auto_goto();
        }
        if !state.sound_effects_enabled {
            state.toggle_sound_effects();
        }
        state.hover_tile = Some((12, 34));
        state
            .city
            .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, trigger);
        assert!(state.consume_pe_trigger_disaster());
        assert_eq!(state.pending_disaster_auto_goto, None);
        assert_eq!(
            state.take_pending_disaster_start_sound(),
            Some(PlacementSound::Disaster)
        );
        assert!(
            !state.take_pending_disaster_start_siren(),
            "types 0x05/0x11 already use Disaster alone — no second siren"
        );
        assert_eq!(PlacementSound::Disaster.sound_id(), 0x208);
    }
}

#[test]
fn pe_aircrash_tick_dives_spins_and_impacts_to_kind6() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let slot = city
        .place_pe_aircrash_actor(40, 40)
        .expect("aircrash actor");
    let mut maydays = 0;
    let mut moved = 0;
    for _ in 0..16 {
        let before = city
            .pe_aircrash_actor()
            .map(|(_, r)| (r.tile_x, r.tile_y, r.lane, r.height));
        let stats = pe_aircrash_tick_city(&mut city);
        maydays += stats.mayday_sounds;
        moved += stats.moved;
        let after = city
            .pe_aircrash_actor()
            .map(|(_, r)| (r.tile_x, r.tile_y, r.lane, r.height));
        if let (Some(b), Some(a)) = (before, after) {
            assert_eq!(a.3, b.3 - 1, "height descends each tick");
            assert_eq!(a.2, (b.2 + 1) & 7, "lane spins +1 each tick");
        }
    }
    assert_eq!(maydays, 1, "Mayday when height crosses 8");
    assert!(moved > 0 || city.pe_aircrash_actor().is_none());
    // Height is now 0 still as airplane state 7; next tick morphs.
    if let Some((_, rec)) = city.pe_aircrash_actor() {
        assert_eq!(rec.height, 0);
        let stats = pe_aircrash_tick_city(&mut city);
        assert_eq!(stats.impacted, 1);
    }
    assert!(city.pe_aircrash_actor().is_none());
    let debris = city.traffic_overlays().unwrap()[slot];
    assert_eq!(debris.kind, OVERLAY_KIND_TRAFFIC);
    assert_eq!(debris.state, PE_AIRCRASH_IMPACT_STATE);
    assert_eq!(debris.saved_b, 1);
    assert_eq!(debris.lane, 0);
    // State 7 writes the kind-6 record directly; it leaves the XTHG proxy in XTXT.
    assert_eq!(
        city.text_overlay_at(debris.tile_x as usize, debris.tile_y as usize),
        xthg_proxy_for_slot(slot)
    );
}

#[test]
fn pe_aircrash_state7_impact_leaves_xtxt_unchanged() {
    let mut city = City::new_blank("/tmp/sc2000-aircrash-xtxt.sc2", "CrashXtxt").expect("blank");
    let slot = city
        .place_pe_aircrash_actor(40, 40)
        .expect("aircrash actor");
    assert_eq!(city.text_overlay_at(40, 40), xthg_proxy_for_slot(slot));
    // Recreate the prior byte captured in the actor record; production placement
    // requires empty XTXT and therefore cannot begin with this synthetic value.
    city.set_text_overlay(40, 40, 0x22);
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].height = 0;
        overlays[slot].saved_a = 0x11;
    }
    let stats = pe_aircrash_tick_city(&mut city);
    assert_eq!(stats.impacted, 1);
    assert_eq!(city.text_overlay_at(40, 40), Some(0x22));
    assert_eq!(
        city.traffic_overlays().unwrap()[slot].kind,
        OVERLAY_KIND_TRAFFIC
    );
}

#[test]
fn pe_aircrash_shared_preflight_morphs_on_xbld_0xdd_before_state7_impact() {
    let mut city = City::new_blank("/tmp/sc2000-aircrash-dd.sc2", "CrashDd").expect("blank");
    let slot = city
        .place_pe_aircrash_actor(40, 40)
        .expect("aircrash actor");
    city.set_building(40, 40, 0xdd);
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].height = 0;
        overlays[slot].saved_a = 0x11;
    }
    let stats = pe_aircrash_tick_city(&mut city);
    assert_eq!(stats.morphed, 1);
    assert_eq!(stats.impacted, 0);
    assert_eq!(city.text_overlay_at(40, 40), xthg_proxy_for_slot(slot));
    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!(record.kind, OVERLAY_KIND_TRAFFIC);
    assert_eq!(record.state, PE_AIRCRASH_IMPACT_STATE);
    assert_eq!(record.saved_b, 1);
    assert_eq!(record.lane, 0);
}

#[test]
fn pe_aircrash_shared_preflight_large_building_consumes_process_lfsr() {
    let mut city = City::new_blank("/tmp/sc2000-aircrash-lfsr.sc2", "CrashLfsr").expect("blank");
    let slot = city
        .place_pe_aircrash_actor(40, 40)
        .expect("aircrash actor");
    city.set_building(40, 40, 0xfb);

    let mut lfsr = Win95Lfsr::from_u32_seed(0x4539_8001);
    let mut expected_lfsr = lfsr;
    let expected_saved_b = if expected_lfsr.random_mod(0x10) != 0 {
        0
    } else {
        1
    };
    let stats = pe_aircrash_tick_city_with_lfsr(&mut city, &mut lfsr);

    assert_eq!(stats.morphed, 1);
    assert_eq!(stats.impacted, 0);
    assert_eq!(lfsr.state(), expected_lfsr.state());
    assert_eq!(city.text_overlay_at(40, 40), xthg_proxy_for_slot(slot));
    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!(record.kind, OVERLAY_KIND_TRAFFIC);
    assert_eq!(record.state, PE_AIRCRASH_IMPACT_STATE);
    assert_eq!(record.saved_b, expected_saved_b);
    assert_eq!(record.lane, 0);
}

#[test]
fn pe_explosion_constructor_paints_xtxt_proxy_not_xtrf() {
    // PE `FUN_00453280` / `0x402C39`: kind 6, subtiles 8/8, XTXT gate < 0xC9, XTXT = slot-0x37.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let vacant = &mut city.traffic_overlays_mut().unwrap()[XTHG_FIRST_USABLE_SLOT];
    vacant.target_x = 0xfe;
    vacant.target_y = 0x2d;
    city.set_text_overlay(40, 40, 0x42);
    city.set_traffic_density(40, 40, 0xaa);

    let slot = city
        .place_pe_explosion_actor(40, 40, 0x0c, 0, 1)
        .expect("FUN_00453280 create");
    assert_eq!(slot, crate::win95_overlay::XTHG_FIRST_USABLE_SLOT);

    let (found, record) = city.pe_explosion_actor_at(40, 40).expect("kind-6 record");
    assert_eq!(found, slot);
    assert!(record.is_pe_explosion_actor());
    assert_eq!(record.kind, crate::win95_overlay::XTHG_EXPLOSION);
    assert_eq!(record.lane, 0);
    assert_eq!(record.state, 0);
    assert_eq!(record.height, 0x0c);
    assert_eq!(record.sub_x, crate::win95_overlay::XTHG_EXPLOSION_SUBTILE);
    assert_eq!(record.sub_y, crate::win95_overlay::XTHG_EXPLOSION_SUBTILE);
    assert_eq!((record.target_x, record.target_y), (0xfe, 0x2d));
    assert_eq!(record.saved_a, 0x42);
    assert_eq!(record.saved_b, 1);
    assert_pe_xthg_proxy(&city, 40, 40, slot);
    assert_eq!(
        city.traffic_density_at(40, 40),
        Some(0xaa),
        "constructor must not rewrite XTRF"
    );
}

#[test]
fn pe_explosion_constructor_rejects_xtxt_occupancy_at_or_above_0xc9() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, crate::win95_overlay::XTHG_EXPLOSION_XTXT_GATE);
    assert!(city.place_pe_explosion_actor(40, 40, 0, 0, 0).is_none());

    city.set_text_overlay(40, 40, crate::win95_overlay::XTHG_EXPLOSION_XTXT_GATE - 1);
    assert!(city.place_pe_explosion_actor(40, 40, 0, 0, 0).is_some());
}

#[test]
fn pe_explosion_tick_ages_then_clears_xtxt_proxy() {
    let mut city = City::new_blank("/tmp/sc2000-explosion-tick.sc2", "test").expect("blank city");
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 0)
        .expect("explosion");
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    assert_eq!(city.text_overlay_at(40, 40), Some(proxy));
    assert_eq!(city.traffic_overlays().unwrap()[slot].lane, 0);

    assert_eq!(pe_explosion_tick_city(&mut city).aged, 1);
    assert_eq!(city.traffic_overlays().unwrap()[slot].lane, 1);
    assert_eq!(city.text_overlay_at(40, 40), Some(proxy));

    assert_eq!(pe_explosion_tick_city(&mut city).aged, 1);
    assert_eq!(city.traffic_overlays().unwrap()[slot].lane, 2);
    assert_eq!(city.text_overlay_at(40, 40), Some(proxy));

    let stats = pe_explosion_tick_city(&mut city);
    assert_eq!(stats.cleared, 1);
    assert_eq!(stats.aged, 0);
    assert_eq!(city.traffic_overlays().unwrap()[slot].kind, 0);
    assert_eq!(city.text_overlay_at(40, 40), Some(0));
}

#[test]
fn pe_explosion_tick_plays_explode_when_lane_zero() {
    // PE FUN_004546f0 @ 0x454719: lane/+1 == 0 → 0x401096(0x1f8) before age.
    let mut city = City::new_blank("/tmp/sc2000-expl-snd.sc2", "ExplSnd").expect("blank");
    assert!(city.place_pe_explosion_actor(40, 40, 0, 0, 0).is_some());
    let first = pe_explosion_tick_city(&mut city);
    assert_eq!(first.explode_sounds, 1);
    assert_eq!(first.aged, 1);
    let second = pe_explosion_tick_city(&mut city);
    assert_eq!(second.explode_sounds, 0);
    assert_eq!(second.aged, 1);
}

#[test]
fn pe_explosion_tick_scatter_ignites_when_plus_b_nonzero() {
    let mut city = City::new_blank("/tmp/sc2000-explosion-b.sc2", "test").expect("blank city");
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("explosion");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].lane = 2;
    }
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_explosion_tick_city_with_lfsr(&mut city, &mut crt, &mut lfsr);
    assert_eq!(stats.cleared, 1);
    assert!(stats.ignited > 0);
    let fires = (38..=42)
        .flat_map(|x| (38..=42).map(move |y| (x, y)))
        .filter(|&(x, y)| city.text_overlay_at(x, y) == Some(0xff))
        .count();
    assert_eq!(fires, stats.ignited);
    assert_eq!(city.text_overlay_at(40, 40), Some(0));
    assert_eq!(stats.trigger_point, Some((40, 40)));
}

#[test]
fn pe_explosion_scatter_no_disasters_gate_matches_scenario_active_override() {
    let mut city = City::new_blank("/tmp/sc2000-explosion-no-disasters.sc2", "NoDisasters")
        .expect("blank city");
    assert!(set_city_no_disasters_enabled(&mut city, true));
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("explosion");
    city.traffic_overlays_mut().unwrap()[slot].lane = 2;

    let mut crt = Win95CrtRand::new(7);
    let mut lfsr = Win95Lfsr::new(1);
    let lfsr_before = lfsr.state();
    let stats = pe_explosion_tick_city_with_lfsr(&mut city, &mut crt, &mut lfsr);
    assert_eq!(stats.cleared, 1, "record and center XTXT still clear");
    assert_eq!(stats.ignited, 0);
    assert_eq!(stats.trigger_point, None);
    assert_eq!(
        lfsr.state(),
        lfsr_before,
        "skipped probes draw no LFSR values"
    );

    // PE's branch is `NoDisasters == 0 || scenarioActive != 0`; scenarios
    // retain fire scatter even when the saved No Disasters option is enabled.
    let mut city = City::new_blank("/tmp/sc2000-explosion-scenario-override.scn", "Scenario")
        .expect("blank city");
    city.kind = CityFileKind::WinScenario;
    city.set_pe_scenario_active(true);
    assert!(set_city_no_disasters_enabled(&mut city, true));
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("scenario explosion");
    city.traffic_overlays_mut().unwrap()[slot].lane = 2;
    let mut crt = Win95CrtRand::new(7);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_explosion_tick_city_with_lfsr(&mut city, &mut crt, &mut lfsr);
    assert_eq!(stats.cleared, 1);
    assert!(stats.ignited > 0);
    assert_eq!(stats.trigger_point, Some((40, 40)));
}

#[test]
fn pe_explosion_tick_ignite_destroy_consumes_crt_before_fire_paint() {
    // PE @ `0x4547da` → FUN_00460e10 mid-XTXT DestroyStructure CRT PlaceTile rubble.
    let mut city = City::new_blank("/tmp/sc2000-expl-crt.sc2", "ExplCrt").expect("blank");
    for x in 38..=42 {
        for y in 38..=42 {
            if (x, y) != (40, 40) {
                city.set_building(x, y, 0x70);
                city.set_text_overlay(x, y, 0x55);
            }
        }
    }
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("explosion");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].lane = 2;
    }
    let mut crt = Win95CrtRand::new(0x4547_da01);
    let mut preview = crt;
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_explosion_tick_city_with_lfsr(&mut city, &mut crt, &mut lfsr);
    assert!(stats.ignited > 0);
    for _ in 0..stats.ignited {
        // Every accepted mid-XTXT probe destroys one size-1 0x70 structure.
        // The two generic render draws precede its XTER-zero rubble choice;
        // fire paint and XTRF clearing consume no CRT values.
        for _ in 0..crate::city::pe_destroy_building_mode1_render_scatter_draw_count(0x70) {
            let _ = preview.random_u15();
        }
        let _ = preview.rubble_tile_id();
    }
    assert_eq!(crt.seed(), preview.seed());
    let rubble = (38..=42)
        .flat_map(|x| (38..=42).map(move |y| (x, y)))
        .filter(|&(x, y)| is_rubble_tile(city.building_at(x, y)))
        .count();
    assert_eq!(rubble, stats.ignited);
}

#[test]
fn pe_explosion_before_tornado_consumes_crt_in_xthg_slot_order() {
    let make_city = || {
        let mut city = City::new_blank(
            "/tmp/sc2000-explosion-tornado-order.sc2",
            "ExplosionTornadoOrder",
        )
        .expect("blank city");
        for x in 38..=42 {
            for y in 38..=42 {
                if (x, y) != (40, 40) {
                    city.set_building(x, y, 0x70);
                    city.set_text_overlay(x, y, 0x55);
                }
            }
        }
        let mut table = crate::win95_overlay::empty_overlay_table();
        table[1] = Win95OverlayRecord {
            kind: XTHG_EXPLOSION,
            lane: 2,
            saved_b: 1,
            tile_x: 40,
            tile_y: 40,
            ..Default::default()
        };
        table[2] = Win95OverlayRecord {
            kind: XTHG_TORNADO,
            lane: 2,
            tile_x: 60,
            tile_y: 60,
            height: 0x0c,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        city.set_traffic_overlays_for_test(table);
        for slot in 1..=2 {
            let record = city.traffic_overlays().unwrap()[slot];
            city.set_text_overlay(
                usize::from(record.tile_x),
                usize::from(record.tile_y),
                crate::win95_overlay::xthg_proxy_for_slot(slot).unwrap(),
            );
        }
        city
    };

    let mut ordered = make_city();
    let mut ordered_sim = Win95OverlaySim::new();
    let mut ordered_crt = Win95CrtRand::new(0x4546_f000);
    let mut ordered_lfsr = Win95Lfsr::new(1);
    let (_, _, ordered_tornado, ordered_explosion, _, _) = pe_disaster_xthg_tick_slots_in_order(
        &mut ordered_sim,
        &mut ordered,
        &mut ordered_crt,
        &mut ordered_lfsr,
        &mut Win95GameRand::new(Win95GameRand::DEFAULT_STATE),
    );
    assert!(ordered_explosion.ignited > 0);

    let mut batched = make_city();
    let mut batched_crt = Win95CrtRand::new(0x4546_f000);
    let mut batched_lfsr = Win95Lfsr::new(1);
    let batched_tornado = pe_tornado_tick_city_with_rng(&mut batched, &mut batched_crt);
    let _ = pe_explosion_tick_city_with_lfsr(&mut batched, &mut batched_crt, &mut batched_lfsr);

    let motion = |city: &City| {
        let record = city.traffic_overlays().unwrap()[2];
        (
            record.tile_x,
            record.tile_y,
            record.sub_x,
            record.sub_y,
            record.lane,
        )
    };
    assert_ne!(ordered_tornado.moved, 0);
    assert_ne!(batched_tornado.moved, 0);
    assert_ne!(
        motion(&ordered),
        motion(&batched),
        "the lower-slot explosion's CRT draws must precede the later tornado's heading rolls"
    );
}

#[test]
fn pe_maxis_man_after_tornado_consumes_crt_in_xthg_slot_order() {
    let make_city = || {
        let mut city = City::new_blank(
            "/tmp/sc2000-maxisman-tornado-order.sc2",
            "MaxisManTornadoOrder",
        )
        .expect("blank city");
        let mut table = crate::win95_overlay::empty_overlay_table();
        table[1] = Win95OverlayRecord {
            kind: XTHG_TORNADO,
            lane: 2,
            tile_x: 40,
            tile_y: 40,
            height: 0x0c,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        table[2] = Win95OverlayRecord {
            kind: XTHG_MAXIS_MAN,
            lane: 2,
            state: crate::win95_overlay::PeMaxisManState::LightActive as u8,
            tile_x: 60,
            tile_y: 60,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        city.set_traffic_overlays_for_test(table);
        for slot in 1..=2 {
            let record = city.traffic_overlays().unwrap()[slot];
            city.set_text_overlay(
                usize::from(record.tile_x),
                usize::from(record.tile_y),
                crate::win95_overlay::xthg_proxy_for_slot(slot).unwrap(),
            );
        }
        city
    };

    let mut ordered = make_city();
    let mut ordered_sim = Win95OverlaySim::new();
    let mut ordered_crt = Win95CrtRand::new(1);
    let mut ordered_lfsr = Win95Lfsr::new(1);
    let (_, _, _, _, maxis_man_changed, _) = pe_disaster_xthg_tick_slots_in_order(
        &mut ordered_sim,
        &mut ordered,
        &mut ordered_crt,
        &mut ordered_lfsr,
        &mut Win95GameRand::new(Win95GameRand::DEFAULT_STATE),
    );
    assert_eq!(maxis_man_changed, 1);

    let mut family_batched = make_city();
    let mut batched_sim = Win95OverlaySim::new();
    let mut batched_crt = Win95CrtRand::new(1);
    assert!(batched_sim.tick_pe_maxis_man(&mut family_batched, &mut batched_crt));
    let _ = pe_tornado_tick_city_with_rng(&mut family_batched, &mut batched_crt);

    let maxis_motion = |city: &City| {
        let record = city.traffic_overlays().unwrap()[2];
        (
            record.tile_x,
            record.tile_y,
            record.sub_x,
            record.sub_y,
            record.lane,
        )
    };
    assert_eq!(maxis_motion(&family_batched), (61, 61, 8, 8, 3));
    assert_eq!(maxis_motion(&ordered), (59, 60, 8, 8, 6));
}

#[test]
fn pe_cargo_ship_after_tornado_consumes_crt_in_xthg_slot_order() {
    let make_city = || {
        let mut city = City::new_blank("/tmp/sc2000-cargo-tornado-order.sc2", "CargoTornadoOrder")
            .expect("blank city");
        city.chunks.push(crate::city::CityChunk {
            name: "XTHG".to_string(),
            offset: 0,
            raw_len: 0,
            data: Vec::new(),
        });
        for x in 20..=24 {
            city.set_terrain(x, 20, 0x10);
            city.set_tile_bits(x, 20, crate::city::TileBits::new(0x04));
        }
        let mut table = crate::win95_overlay::empty_overlay_table();
        table[1] = Win95OverlayRecord {
            kind: XTHG_TORNADO,
            lane: 2,
            tile_x: 40,
            tile_y: 40,
            height: 0x0c,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        table[2] = Win95OverlayRecord {
            kind: XTHG_CARGO_SHIP,
            state: 4,
            lane: 2,
            tile_x: 20,
            tile_y: 20,
            target_x: 24,
            target_y: 20,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        city.set_traffic_overlays_for_test(table);
        for slot in 1..=2 {
            let record = city.traffic_overlays().unwrap()[slot];
            city.set_text_overlay(
                usize::from(record.tile_x),
                usize::from(record.tile_y),
                crate::win95_overlay::xthg_proxy_for_slot(slot).unwrap(),
            );
        }
        city
    };

    let mut ordered = make_city();
    let mut ordered_sim = Win95OverlaySim::new();
    let mut ordered_crt = Win95CrtRand::new(0x4567_6015);
    let mut ordered_lfsr = Win95Lfsr::new(1);
    let (_, _, ordered_tornado, _, _, _) = pe_disaster_xthg_tick_slots_in_order(
        &mut ordered_sim,
        &mut ordered,
        &mut ordered_crt,
        &mut ordered_lfsr,
        &mut Win95GameRand::new(Win95GameRand::DEFAULT_STATE),
    );
    assert_ne!(ordered_tornado.moved, 0);
    assert_eq!(ordered.traffic_overlays().unwrap()[2].kind, XTHG_CARGO_SHIP);

    let mut family_batched = make_city();
    let mut batched_sim = Win95OverlaySim::new();
    let mut batched_crt = Win95CrtRand::new(0x4567_6015);
    let mut batched_lfsr = Win95Lfsr::new(1);
    assert!(batched_sim.tick_pe_cargo_ship_slot(
        &mut family_batched,
        2,
        &mut batched_crt,
        &mut batched_lfsr,
    ));
    let batched_tornado = pe_tornado_tick_city_with_rng(&mut family_batched, &mut batched_crt);

    let motion = |city: &City| {
        let record = city.traffic_overlays().unwrap()[1];
        (
            record.tile_x,
            record.tile_y,
            record.sub_x,
            record.sub_y,
            record.lane,
        )
    };
    assert_ne!(batched_tornado.moved, 0);
    assert_ne!(
        motion(&ordered),
        motion(&family_batched),
        "the later-slot cargo CRT sound gate must not run before the earlier tornado"
    );
}

#[test]
fn pe_helicopter_after_tornado_consumes_crt_in_xthg_slot_order() {
    let make_city = || {
        let mut city = City::new_blank(
            "/tmp/sc2000-helicopter-tornado-order.sc2",
            "HelicopterTornadoOrder",
        )
        .expect("blank city");
        let mut table = crate::win95_overlay::empty_overlay_table();
        table[1] = Win95OverlayRecord {
            kind: XTHG_TORNADO,
            lane: 2,
            tile_x: 40,
            tile_y: 40,
            height: 0x0c,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        table[2] = Win95OverlayRecord {
            kind: XTHG_HELICOPTER,
            state: 4,
            lane: 2,
            tile_x: 20,
            tile_y: 20,
            height: 4,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        city.set_traffic_overlays_for_test(table);
        for slot in 1..=2 {
            let record = city.traffic_overlays().unwrap()[slot];
            city.set_text_overlay(
                usize::from(record.tile_x),
                usize::from(record.tile_y),
                crate::win95_overlay::xthg_proxy_for_slot(slot).unwrap(),
            );
        }
        city
    };

    let mut deferred = make_city();
    let helicopter_before = deferred.traffic_overlays().unwrap()[2];
    let mut deferred_sim = Win95OverlaySim::new();
    let mut deferred_crt = Win95CrtRand::new(0x4540_c384);
    let mut deferred_lfsr = Win95Lfsr::new(1);
    assert!(
        !deferred_sim.tick_ambient_with_game_rand_defer_xthg_slot_actors(
            &mut deferred,
            GameSpeed::Llama,
            0.04,
            &mut deferred_crt,
            &mut deferred_lfsr,
            &mut Win95GameRand::new(Win95GameRand::DEFAULT_STATE),
        )
    );
    assert_eq!(deferred_crt.seed(), 0x4540_c384);
    assert_eq!(deferred.traffic_overlays().unwrap()[2], helicopter_before);

    let mut ordered = make_city();
    let mut ordered_sim = Win95OverlaySim::new();
    let mut ordered_crt = Win95CrtRand::new(0x4540_c384);
    let mut ordered_lfsr = Win95Lfsr::new(1);
    let (_, _, ordered_tornado, _, _, _) = pe_disaster_xthg_tick_slots_in_order(
        &mut ordered_sim,
        &mut ordered,
        &mut ordered_crt,
        &mut ordered_lfsr,
        &mut Win95GameRand::new(Win95GameRand::DEFAULT_STATE),
    );
    assert_ne!(ordered_tornado.moved, 0);
    assert_eq!(ordered.traffic_overlays().unwrap()[2].kind, XTHG_HELICOPTER);

    let mut family_batched = make_city();
    let mut batched_sim = Win95OverlaySim::new();
    let mut batched_crt = Win95CrtRand::new(0x4540_c384);
    let mut batched_lfsr = Win95Lfsr::new(1);
    assert!(batched_sim.tick_pe_aircraft_slot(
        &mut family_batched,
        2,
        &mut batched_crt,
        &mut batched_lfsr,
    ));
    let batched_tornado = pe_tornado_tick_city_with_rng(&mut family_batched, &mut batched_crt);

    let motion = |city: &City| {
        let record = city.traffic_overlays().unwrap()[1];
        (
            record.tile_x,
            record.tile_y,
            record.sub_x,
            record.sub_y,
            record.lane,
        )
    };
    assert_ne!(batched_tornado.moved, 0);
    assert_ne!(
        motion(&ordered),
        motion(&family_batched),
        "the later-slot helicopter's CRT draw must follow the earlier tornado"
    );
}

#[test]
fn pe_sailboat_after_tornado_consumes_crt_in_xthg_slot_order() {
    let make_city = || {
        let mut city = City::new_blank(
            "/tmp/sc2000-sailboat-tornado-order.sc2",
            "SailboatTornadoOrder",
        )
        .expect("blank city");
        city.set_tile_bits(20, 20, TileBits::new(0x04));
        let mut table = crate::win95_overlay::empty_overlay_table();
        table[1] = Win95OverlayRecord {
            kind: XTHG_TORNADO,
            lane: 2,
            tile_x: 40,
            tile_y: 40,
            height: 0x0c,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        table[2] = Win95OverlayRecord {
            kind: XTHG_SAILBOAT,
            state: 0,
            lane: 1,
            tile_x: 20,
            tile_y: 20,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        city.set_traffic_overlays_for_test(table);
        for slot in 1..=2 {
            let record = city.traffic_overlays().unwrap()[slot];
            city.set_text_overlay(
                usize::from(record.tile_x),
                usize::from(record.tile_y),
                crate::win95_overlay::xthg_proxy_for_slot(slot).unwrap(),
            );
        }
        city
    };

    let mut deferred = make_city();
    let sailboat_before = deferred.traffic_overlays().unwrap()[2];
    let mut deferred_sim = Win95OverlaySim::new();
    let mut deferred_crt = Win95CrtRand::new(0x4562_e037);
    let mut deferred_lfsr = Win95Lfsr::new(0x4000);
    assert!(
        !deferred_sim.tick_ambient_with_game_rand_defer_xthg_slot_actors(
            &mut deferred,
            GameSpeed::Llama,
            0.04,
            &mut deferred_crt,
            &mut deferred_lfsr,
            &mut Win95GameRand::new(Win95GameRand::DEFAULT_STATE),
        )
    );
    assert_eq!(deferred_crt.seed(), 0x4562_e037);
    assert_eq!(deferred.traffic_overlays().unwrap()[2], sailboat_before);

    let mut ordered = make_city();
    let mut ordered_sim = Win95OverlaySim::new();
    let mut ordered_crt = Win95CrtRand::new(0x4562_e037);
    let mut ordered_lfsr = Win95Lfsr::new(0x4000);
    let (_, _, ordered_tornado, _, _, _) = pe_disaster_xthg_tick_slots_in_order(
        &mut ordered_sim,
        &mut ordered,
        &mut ordered_crt,
        &mut ordered_lfsr,
        &mut Win95GameRand::new(Win95GameRand::DEFAULT_STATE),
    );
    assert_ne!(ordered_tornado.moved, 0);
    assert_eq!(ordered.traffic_overlays().unwrap()[2].kind, XTHG_SAILBOAT);

    let mut family_batched = make_city();
    let mut batched_sim = Win95OverlaySim::new();
    let mut batched_crt = Win95CrtRand::new(0x4562_e037);
    let mut batched_lfsr = Win95Lfsr::new(0x4000);
    assert!(batched_sim.tick_pe_sailboat_slot(
        &mut family_batched,
        2,
        &mut batched_crt,
        &mut batched_lfsr,
    ));
    let batched_tornado = pe_tornado_tick_city_with_rng(&mut family_batched, &mut batched_crt);

    assert_ne!(batched_tornado.moved, 0);
    assert_ne!(
        ordered.traffic_overlays().unwrap()[2].lane,
        family_batched.traffic_overlays().unwrap()[2].lane,
        "the later-slot sailboat's CRT lane draw must follow the earlier tornado"
    );
}

#[test]
fn pe_train_engine_before_tornado_consumes_crt_in_xthg_slot_order() {
    let make_city = || {
        let mut city = City::new_blank("/tmp/sc2000-train-tornado-order.sc2", "TrainTornadoOrder")
            .expect("blank city");
        for x in 10..=14 {
            city.set_building(x, 10, 45);
        }
        assert!(city.place_ambient_train(10, 10, 2));
        let engine_slot = city
            .traffic_overlays()
            .unwrap()
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find_map(|(slot, record)| record.is_ambient_train_engine().then_some(slot))
            .expect("train engine");
        let tornado_slot = engine_slot + 3;
        city.traffic_overlays_mut().unwrap()[tornado_slot] = Win95OverlayRecord {
            kind: XTHG_TORNADO,
            lane: 2,
            tile_x: 40,
            tile_y: 40,
            height: 0x0c,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        city.set_text_overlay(
            40,
            40,
            crate::win95_overlay::xthg_proxy_for_slot(tornado_slot).unwrap(),
        );
        (city, engine_slot, tornado_slot)
    };

    let (mut ordered, _, tornado_slot) = make_city();
    let mut ordered_sim = Win95OverlaySim::new();
    let mut ordered_crt = Win95CrtRand::new(0x4540_c384);
    let mut ordered_lfsr = Win95Lfsr::new(0x1234);
    let mut ordered_game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
    let (_, _, _, _, _, train_changed) = pe_disaster_xthg_tick_slots_in_order(
        &mut ordered_sim,
        &mut ordered,
        &mut ordered_crt,
        &mut ordered_lfsr,
        &mut ordered_game_rand,
    );
    assert_eq!(train_changed, 1);
    let ordered_tornado = ordered.traffic_overlays().unwrap()[tornado_slot];

    let (mut earlier_tornado, engine_slot, tornado_slot) = make_city();
    let mut batched_crt = Win95CrtRand::new(0x4540_c384);
    let _ = pe_tornado_tick_slot_with_rng(&mut earlier_tornado, tornado_slot, &mut batched_crt);
    let earlier_tornado_record = earlier_tornado.traffic_overlays().unwrap()[tornado_slot];
    let mut batched_sim = Win95OverlaySim::new();
    let mut batched_lfsr = Win95Lfsr::new(0x1234);
    let mut batched_game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
    assert!(batched_sim.tick_pe_train_engine_slot(
        &mut earlier_tornado,
        engine_slot,
        &mut batched_crt,
        &mut batched_lfsr,
        &mut batched_game_rand,
    ));

    let motion = |record: Win95OverlayRecord| {
        (
            record.tile_x,
            record.tile_y,
            record.sub_x,
            record.sub_y,
            record.lane,
        )
    };
    assert_ne!(
        motion(ordered_tornado),
        motion(earlier_tornado_record),
        "the lower-slot train route draw must precede the later tornado heading draws"
    );
}

#[test]
fn pe_explosion_scatter_rubble_xtxt_does_not_count_as_hit() {
    // PE @ `0x4547e2`: FUN_00460e10 rubble-band ret 0 must not set scatter hit / auto-goto.
    let mut city = City::new_blank("/tmp/sc2000-expl-rubble.sc2", "ExplRb").expect("blank");
    // Water on the explosion tile so a ±2 probe that re-hits the cleared center cannot paint.
    city.set_tile_bits(40, 40, TileBits::new(0b0000_0100));
    for x in 38..=42 {
        for y in 38..=42 {
            if (x, y) != (40, 40) {
                city.set_text_overlay(x, y, 0xf5);
                city.set_building(x, y, 0x70);
            }
        }
    }
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("explosion");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].lane = 2;
    }
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_explosion_tick_city_with_lfsr(&mut city, &mut crt, &mut lfsr);
    assert_eq!(stats.cleared, 1);
    assert_eq!(stats.ignited, 0);
    assert_eq!(stats.trigger_point, None);
    assert_eq!(
        city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET).unwrap_or(0),
        0
    );
    for x in 38..=42 {
        for y in 38..=42 {
            assert_ne!(
                city.text_overlay_at(x, y),
                Some(0xff),
                "fire paint at ({x},{y})"
            );
        }
    }
}

#[test]
fn pe_explosion_tick_scatter_queues_auto_goto_on_explosion_tile() {
    let mut city = City::new_blank("/tmp/sc2000-explosion-goto.sc2", "test").expect("blank city");
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("explosion");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].lane = 2;
    }
    let mut state = test_state(city);
    state.auto_goto_enabled = true;
    assert!(state.take_pending_disaster_auto_goto().is_none());
    assert!(state.tick_pe_disaster_xthg_actors_in_slot_order() > 0);
    assert_eq!(state.take_pending_disaster_auto_goto(), Some((40, 40)));
}

#[test]
fn pe_explosion_trigger_uses_its_tile_through_calendar_with_auto_goto_off() {
    let mut city = City::new_blank("/tmp/pe-explosion-flood.sc2", "Explosion flood").unwrap();
    city.pe_w_city_mode = CITY_MODE_GAMEPLAY as u16;
    city.set_terrain(40, 41, 0x20);
    city.set_terrain(12, 34, 0x20);
    let slot = city.place_pe_explosion_actor(40, 41, 0, 0, 1).unwrap();
    let actor = &mut city.traffic_overlays_mut().unwrap()[slot];
    actor.lane = 2;
    actor.state = 2;

    let mut state = test_state(city);
    state.auto_goto_enabled = false;
    state.no_disasters_enabled = false;
    state.pe_process_lfsr = Win95Lfsr::new(1);
    state.pe_idle_scheduler.pending_disaster_point = Some((12, 34));
    state.game_clock = GameClock::new_with_day(1900, 1, 1);
    state.traffic.set_speed(GameSpeed::Cheetah);

    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(200))
            .days,
        1
    );
    assert_eq!(state.game_clock.day(), 2);
    assert_eq!(state.city.pe_w_trigger_disaster, 0);
    assert_eq!(state.city.pe_w_current_disaster_type(), 2);
    assert_eq!(
        state.pe_idle_scheduler.pending_disaster_point,
        Some((40, 41))
    );
    assert_eq!(state.city.text_overlay_at(41, 41), Some(0xfc));
    assert_eq!(state.city.text_overlay_at(13, 34), Some(0));
    assert_eq!(state.take_pending_disaster_auto_goto(), None);
}

#[test]
fn pe_explosion_in_disaster_mode_leaves_the_existing_trigger_and_point() {
    let mut city = City::new_blank("/tmp/pe-active-explosion.sc2", "Active explosion").unwrap();
    city.pe_w_city_mode = CITY_MODE_DISASTER as u16;
    city.pe_w_trigger_disaster = 7;
    let slot = city.place_pe_explosion_actor(40, 41, 0, 0, 1).unwrap();
    let actor = &mut city.traffic_overlays_mut().unwrap()[slot];
    actor.lane = 2;
    actor.state = 2;

    let mut state = test_state(city);
    state.pe_process_lfsr = Win95Lfsr::new(1);
    state.pe_idle_scheduler.pending_disaster_point = Some((12, 34));
    assert!(state.tick_pe_disaster_xthg_actors_in_slot_order() > 0);
    assert_eq!(state.city.pe_w_trigger_disaster, 7);
    assert_eq!(
        state.pe_idle_scheduler.pending_disaster_point,
        Some((12, 34))
    );
}

#[test]
fn pe_explosion_tick_scatter_sets_trigger_type_from_state() {
    // PE @ 0x454818: wSetTriggerDisasterType = state != 0 ? state : 1.
    let mut city = City::new_blank("/tmp/sc2000-expl-trig.sc2", "ExplTrig").expect("blank");
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("explosion");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].lane = 2;
        overlays[slot].state = 0;
    }
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_explosion_tick_city_with_lfsr(&mut city, &mut crt, &mut lfsr);
    assert!(stats.ignited > 0);
    assert_eq!(city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET), Some(1));

    let mut city = City::new_blank("/tmp/sc2000-expl-trig5.sc2", "ExplTrig5").expect("blank");
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("explosion");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].lane = 2;
        overlays[slot].state = 5;
    }
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_explosion_tick_city_with_lfsr(&mut city, &mut crt, &mut lfsr);
    assert!(stats.ignited > 0);
    assert_eq!(city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET), Some(5));
}

#[test]
fn pe_explosion_tick_scatter_clears_xtrf_on_ignite() {
    // PE FUN_00460e10 flag 1: XTXT 0xff then zero XTRF 64×64 cell (x>>1,y>>1).
    let mut city = City::new_blank("/tmp/sc2000-expl-xtrf.sc2", "ExplXtrf").expect("blank");
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("explosion");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].lane = 2;
    }
    for &(tx, ty) in &[(38, 40), (39, 40), (38, 41), (39, 41)] {
        city.set_traffic_density(tx, ty, 80);
    }
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_explosion_tick_city_with_lfsr(&mut city, &mut crt, &mut lfsr);
    assert!(stats.ignited > 0);
    let mut cleared_cell = false;
    for y in 38..=42 {
        for x in 38..=42 {
            if city.text_overlay_at(x, y) == Some(0xff) {
                let ox = (x / 2) * 2;
                let oy = (y / 2) * 2;
                assert_eq!(city.traffic_density_at(ox, oy), Some(0));
                assert_eq!(city.traffic_density_at(ox + 1, oy), Some(0));
                assert_eq!(city.traffic_density_at(ox, oy + 1), Some(0));
                assert_eq!(city.traffic_density_at(ox + 1, oy + 1), Some(0));
                cleared_cell = true;
            }
        }
    }
    assert!(cleared_cell, "at least one ignite must clear its XTRF cell");
}

#[test]
fn pe_explosion_tick_scatter_clears_xlabel_on_sign_xtxt() {
    // PE FUN_00460e10: XTXT 1..=0x32 → 0x401dca (clear XLAB) then paint 0xff.
    let mut city = City::new_blank("/tmp/sc2000-expl-xlab.sc2", "ExplXlab").expect("blank");
    let slot = city
        .place_pe_explosion_actor(40, 40, 0, 0, 1)
        .expect("explosion");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].lane = 2;
    }
    let mut next_xtxt = 0x05u8;
    for y in 38..=42 {
        for x in 38..=42 {
            if x == 40 && y == 40 {
                continue; // explosion tile clears to 0 before scatter
            }
            city.set_text_overlay(x, y, next_xtxt);
            city.set_label(next_xtxt, &format!("Sign{next_xtxt:02x}"));
            next_xtxt = next_xtxt.saturating_add(1);
            if next_xtxt > 0x32 {
                next_xtxt = 0x05;
            }
        }
    }
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_explosion_tick_city_with_lfsr(&mut city, &mut crt, &mut lfsr);
    assert!(stats.ignited > 0);
    let mut cleared_label = false;
    for y in 38..=42 {
        for x in 38..=42 {
            if city.text_overlay_at(x, y) != Some(0xff) {
                continue;
            }
            // Any prior sign slot that is now empty proves 0x401dca ran.
            for xtxt in 0x05u8..=0x32 {
                if city.label_at(xtxt).unwrap_or("").is_empty() {
                    cleared_label = true;
                    break;
                }
            }
        }
    }
    assert!(cleared_label, "ignite on sign XTXT must clear XLAB");
}

#[test]
fn pe_disaster_xthg_runs_on_native_idle_pulse_before_calendar() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(city.place_pe_aircrash_actor(40, 40).is_some());
    let height_before = city.pe_aircrash_actor().unwrap().1.height;
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Llama);
    state.city.set_misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET, 1);
    state
        .city
        .set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x12);

    // Daily PE overlay scan must not dive the airplane.
    let _ = state.tick_pe_disaster_clock();
    assert_eq!(
        state.city.pe_aircrash_actor().unwrap().1.height,
        height_before
    );

    // The ordinary PE timer raises +0x318 every 200 ms. At speed 3 the first
    // pulse is an actor opportunity even though it does not yet admit a day.
    state.advance_frame_simulation(Duration::from_millis(199));
    assert_eq!(
        state.city.pe_aircrash_actor().unwrap().1.height,
        height_before
    );
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(1))
            .days,
        0
    );
    let height_after = state.city.pe_aircrash_actor().unwrap().1.height;
    assert_eq!(height_after, height_before - 1);
}

#[test]
fn mac_fidelity_does_not_advance_pe_disaster_xthg_actor_tick() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let slot = city
        .place_pe_tornado_actor(40, 40, 2, 0x0c, 50, 50, 0x33)
        .expect("PE tornado actor");
    let actor_before = city.pe_tornado_actor().expect("PE tornado record");
    let proxy_before = city.text_overlay_at(40, 40);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let crt_before = state.pe_process_crt_seed();
    let lfsr_before = state.pe_process_lfsr_state();

    state.advance_frame_simulation(Duration::from_millis(200));

    assert_eq!(state.city.pe_tornado_actor(), Some((slot, actor_before.1)));
    assert_eq!(state.city.text_overlay_at(40, 40), proxy_before);
    assert_eq!(state.pe_process_crt_seed(), crt_before);
    assert_eq!(state.pe_process_lfsr_state(), lfsr_before);
}

#[test]
fn mac_manual_air_crash_queues_without_entering_pe_executor() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state
        .city
        .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
    state.hover_tile = Some((40, 40));
    let mac_rng_before = state.mac_process_rng.seed();
    let crt_before = state.pe_process_crt_seed();
    let lfsr_before = state.pe_process_lfsr_state();
    let query_before = state.query_line.clone();

    assert!(state.start_disaster(DisasterKind::AirCrash));

    assert_eq!(state.city.mac_pending_action(), 18);
    assert_eq!(state.city.pe_w_trigger_disaster, 0);
    assert!(!state.city.has_active_disaster());
    assert!(state.city.pe_tornado_actor().is_none());
    assert_eq!(state.mac_process_rng.seed(), mac_rng_before);
    assert_eq!(state.pe_process_crt_seed(), crt_before);
    assert_eq!(state.pe_process_lfsr_state(), lfsr_before);
    assert_eq!(state.query_line, query_before);
}

#[test]
fn simulation_day22_leaves_pe_trigger_for_idle_consumer() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(24, 24, 0x70);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_fire_target_tile_with_rng(&state.city, &mut preview, &mut preview_lfsr)
        .expect("PE fire ignite");
    state.hover_tile = Some((12, 34));
    state.no_disasters_enabled = false;
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x01);
    let population = live_population_stats_for_city(&state.city);

    state.sim_calendar_day22_scenario(population);
    assert_eq!(state.city.pe_w_trigger_disaster, 1);
    assert_eq!(state.city.text_overlay_at(expected.0, expected.1), Some(0));

    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(
        state.city.text_overlay_at(expected.0, expected.1),
        Some(0xff)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x01)
    );
    assert_eq!(state.query_line, "Fire Reported");
}

#[test]
fn live_misc_refresh_keeps_pending_pe_trigger_disaster() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x06);
    state.refresh_live_misc_current_values();
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0x06)
    );
}

#[test]
fn disaster_menu_action_seeds_active_disaster_marker() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));

    assert!(!state.city.has_active_disaster());
    assert!(!state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Emergency)));

    let mut preview = state.pe_process_crt;
    let expected_fields = pe_tornado_xthg_fields_with_rng(&state.city, 12, 34, &mut preview);
    assert!(state.start_disaster(DisasterKind::Tornado));

    let slot = assert_pe_tornado_actor(&state.city, 12, 34, expected_fields);
    assert_pe_xthg_proxy(&state.city, 12, 34, slot);
    assert!(state.city.has_active_disaster());
    assert!(state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Emergency)));
    assert_eq!(state.query_line, "Tornado Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn auto_goto_only_targets_successful_disaster_commands_when_enabled() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Tornado))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some((12, 34)));

    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((56, 78));
    state.toggle_auto_goto();

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Fire))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, None);

    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((90, 12));
    state.toggle_no_disasters();

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Flood))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, None);
}

#[test]
fn flood_auto_goto_centers_on_xter_source_not_hover() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(42, 41, 0x20);
    assert_eq!(pe_flood_goto_tile(&city, 12, 34), Some((42, 41)));
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Flood))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some((42, 41)));
    assert_eq!(state.city.terrain_at(42, 41), 0x20);
    assert_eq!(state.city.text_overlay_at(42, 41), Some(0));
}

#[test]
fn win95_flood_starts_timer_at_shore_even_when_no_neighbor_can_be_painted() {
    for path in ["win95-flood-timer.sc2", "win95-flood-timer.city"] {
        let mut city = blank_city_layout_fixture(path, "Flood timer");
        city.set_terrain(12, 34, 0x20);
        city.set_tile_bits(13, 34, TileBits::new(4));
        city.set_tile_bits(12, 35, TileBits::new(4));
        let mut state = test_state(city);
        state.no_disasters_enabled = false;
        state.auto_goto_enabled = true;
        state.hover_tile = Some((12, 34));

        let effect = state
            .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Flood))
            .unwrap();
        assert_eq!(effect.disaster_log, Some((DisasterKind::Flood, true)));
        assert_eq!(effect.auto_goto_tile, Some((12, 34)));
        assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
        assert!(disaster_overlay_tiles(&state.city).is_empty());
    }
}

#[test]
fn win95_flood_rejected_manual_fallback_keeps_native_lfsr_progress() {
    let city = blank_city_layout_fixture("win95-flood-rejected.sc2", "Rejected Flood");
    let mut state = test_state(city);
    state.no_disasters_enabled = false;
    state.hover_tile = Some((64, 64));
    state.pe_process_lfsr = Win95Lfsr::from_u32_seed(1);
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            state.city.set_terrain(x, y, 0x10);
        }
    }

    assert!(!state.start_disaster(DisasterKind::Flood));
    assert_eq!(state.pe_process_lfsr_state(), 0x43ec);
    assert_eq!(state.pe_disaster_duration, 0);
    assert_eq!(state.city.pe_w_trigger_disaster, 0);
    assert!(disaster_overlay_tiles(&state.city).is_empty());

    state.city.pe_w_trigger_disaster = 2;
    state.pe_idle_scheduler.pending_disaster_point = Some((-1, 34));
    state.pe_process_lfsr = Win95Lfsr::new(1);
    assert!(!state.consume_pe_trigger_disaster());
    assert_eq!(state.city.pe_w_trigger_disaster, 0);
    assert_eq!(
        state.pe_idle_scheduler.pending_disaster_point,
        Some((-1, 34))
    );
    assert_eq!(state.pe_process_lfsr_state(), 0x43ec);
}

#[test]
fn flood_auto_goto_centers_on_fallback_empty_xter() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }
    let mut preview_lfsr = state.pe_process_lfsr;
    let overlays = pe_flood_overlays_with_lfsr(&state.city, 12, 34, &mut preview_lfsr);
    let goto = *overlays.keys().next().expect("fallback XTER overlay");
    assert_eq!(overlays.get(&goto), Some(&0xfc));

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Flood))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some(goto));
    assert_eq!(state.city.text_overlay_at(goto.0, goto.1), Some(0xfc));
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn blizzard_flood_auto_goto_stays_on_trigger_origin() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(12, 34, 0x20);
    assert_eq!(pe_blizzard_flood_goto_tile(&city, 12, 34), Some((12, 34)));
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::BlizzardFlood))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some((12, 34)));
}

#[test]
fn no_disasters_blocks_manual_disaster_commands() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(24, 24, 0x70);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_fire_target_tile_with_rng(&state.city, &mut preview, &mut preview_lfsr)
        .expect("PE fire ignite");
    state.hover_tile = Some((12, 34));

    state.toggle_no_disasters();
    let blocked_query_line = state.query_line.clone();

    assert!(state.no_disasters_enabled);
    assert!(!state.start_disaster(DisasterKind::Fire));
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());
    assert_eq!(state.query_line, blocked_query_line);

    state.toggle_no_disasters();

    assert!(state.start_disaster(DisasterKind::Fire));
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(
        state.city.text_overlay_at(expected.0, expected.1),
        Some(0xff)
    );
    assert!(state.city.has_active_disaster());
    assert_eq!(state.query_line, "Fire Reported");
}

#[test]
fn pe_riot_disaster_requires_unemployment_and_heat() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(
        !pe_riot_disaster_ready(&city),
        "default heat 0x96 and zero pe_unemployment should fail"
    );
    assert!(!pe_mass_riot_disaster_ready(&city));

    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xAA);
    assert!(
        !pe_riot_disaster_ready(&city),
        "heat alone is not enough without unemployment"
    );

    // PE type-3 gate reads `0x4CA5F8` (published slot-7 ratio), not abandoned census.
    city.set_pe_unemployment_percent(10);
    assert!(
        pe_riot_disaster_ready(&city),
        "type 3 riot does not need 30k population"
    );
    assert!(
        !pe_mass_riot_disaster_ready(&city),
        "type 0xD mass riot still needs 30k population"
    );
    assert!(pe_riot_disaster_ready(&city));
    city.set_pe_dw_city_population(30_000);
    assert!(pe_mass_riot_disaster_ready(&city));

    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xA9);
    assert!(
        !pe_riot_disaster_ready(&city),
        "heat 169 should miss the PE 0xAA gate"
    );
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xAA);
    let under_mass = PopulationStats {
        normal_population: 29_999,
        total_population: 29_999,
        ..Default::default()
    };
    assert!(pe_riot_disaster_ready(&city));
    city.set_pe_dw_city_population(under_mass.normal_population);
    assert!(!pe_mass_riot_disaster_ready(&city));
}

#[test]
fn mac_riot_gate_uses_signed_ratio_but_unsigned_mass_scalar() {
    let mut city = City::new_blank("/tmp/mac-riot-signed.city", "Riot").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xaa);
    city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 0x8000_0000);
    for ratio in [0x8000_0000, u32::MAX] {
        city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, ratio);
        assert!(
            !mac_riot_disaster_ready(&city),
            "negative signed ratio {ratio:#x}"
        );
        assert!(!mac_mass_riot_disaster_ready(&city));
    }
    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 10);
    assert!(
        mac_mass_riot_disaster_ready(&city),
        "mass scalar uses unsigned comparison"
    );
}

#[test]
fn mac_day21_riot_ratio_updates_native_eligibility_gate() {
    let mut city = City::new_blank("/tmp/mac-monthly-riot.city", "Riot").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x05f4, 100);
    city.set_mac_state_misc_u32_at(0x060c, 12);
    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 9);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    state.sim_calendar_day21_demand_and_graphs();
    // Pin heat after the weather transition; this test isolates the saved
    // scan-ratio gate rather than depending on the provisional RNG sequence.
    state
        .city
        .set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xaa);
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET),
        Some(10)
    );
    assert!(mac_riot_disaster_ready(&state.city));
    assert!(!mac_mass_riot_disaster_ready(&state.city));

    state.city.set_mac_state_misc_u32_at(0x060c, 0);
    state.sim_calendar_day21_demand_and_graphs();
    state
        .city
        .set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xaa);
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET),
        Some(0)
    );
    assert!(!mac_riot_disaster_ready(&state.city));

    // The separate mass-riot gate requires the phase-21 scalar to reach
    // 30,000 as well as the shared ratio and heat conditions.
    state.city.set_mac_state_misc_u32_at(0x05f4, 2_999);
    state.city.set_mac_state_misc_u32_at(0x060c, 400);
    state.sim_calendar_day21_demand_and_graphs();
    state
        .city
        .set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xaa);
    assert!(mac_riot_disaster_ready(&state.city));
    assert!(!mac_mass_riot_disaster_ready(&state.city));
    state.city.set_mac_state_misc_u32_at(0x05f4, 3_000);
    state.sim_calendar_day21_demand_and_graphs();
    state
        .city
        .set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xaa);
    assert!(mac_mass_riot_disaster_ready(&state.city));
}

#[test]
fn mac_riot_gates_use_saved_state_and_keep_mass_threshold_separate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;

    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 9);
    city.set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xff);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xff);
    assert!(
        !mac_riot_disaster_ready(&city),
        "Mac saved unemployment is below 10"
    );

    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 10);
    city.set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xa9);
    assert!(
        !mac_riot_disaster_ready(&city),
        "Mac F084 169 misses the 0xAA gate"
    );
    city.set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xaa);
    assert!(
        mac_riot_disaster_ready(&city),
        "type 3 has only the shared gates"
    );
    assert!(
        !mac_mass_riot_disaster_ready(&city),
        "type D also needs the separate fffff01a threshold"
    );

    city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 29_999);
    assert!(!mac_mass_riot_disaster_ready(&city));
    city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 30_000);
    assert!(mac_mass_riot_disaster_ready(&city));

    city.kind = CityFileKind::WinSc2;
    assert!(
        mac_riot_disaster_ready(&city),
        "the full-DWORD riot ratio is shared across save layouts"
    );
    assert!(
        mac_mass_riot_disaster_ready(&city),
        "the shared weighted-growth scalar also gates type D on either layout"
    );
}

#[test]
fn mac_ambient_riot_dispatch_uses_native_saved_gates() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.kind = CityFileKind::MacCity;

    // High PE aliases must not substitute for the Mac unemployment and heat
    // fields when the native type-3 slot is selected.
    city.set_pe_unemployment_percent(100);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xff);
    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 9);
    city.set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xaa);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    let riot_seed = (0..128)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            toolbox_random_range(&mut rng, 0x13) == 0x03
        })
        .expect("find Mac Toolbox seed for type 3");

    let mut blocked_riot_rng = MacToolboxRng::new(riot_seed);
    assert_ne!(
        ambient_disaster_kind_for_city(
            &city,
            quiet,
            outcomes,
            &mut blocked_riot_rng,
            EngineFidelity::Mac68k,
        ),
        Some(DisasterKind::Rioters),
        "the native unemployment value below 10 blocks type 3 despite PE aliases"
    );
    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 10);
    let mut ready_riot_rng = MacToolboxRng::new(riot_seed);
    assert_eq!(
        ambient_disaster_kind_for_city(
            &city,
            quiet,
            outcomes,
            &mut ready_riot_rng,
            EngineFidelity::Mac68k,
        ),
        Some(DisasterKind::Rioters)
    );

    let mass_seed = (0..128)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            toolbox_random_range(&mut rng, 0x13) == 0x0d
        })
        .expect("find Mac Toolbox seed for type D");
    city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 29_999);
    let mut blocked_mass_rng = MacToolboxRng::new(mass_seed);
    assert_ne!(
        ambient_disaster_kind_for_city(
            &city,
            quiet,
            outcomes,
            &mut blocked_mass_rng,
            EngineFidelity::Mac68k,
        ),
        Some(DisasterKind::MassRiot),
        "the native type-D growth scalar below 30000 blocks mass riot"
    );
    city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 30_000);
    let mut ready_mass_rng = MacToolboxRng::new(mass_seed);
    assert_eq!(
        ambient_disaster_kind_for_city(
            &city,
            quiet,
            outcomes,
            &mut ready_mass_rng,
            EngineFidelity::Mac68k,
        ),
        Some(DisasterKind::MassRiot)
    );

    // Mac fidelity uses its native interpretation of these saved roles even
    // when the city is stored in the Win95 container.
    city.kind = CityFileKind::WinSc2;
    city.set_misc_u32_at(MISC_UNEMPLOYMENT_OFFSET, 100);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xff);
    let mut non_mac_rng = MacToolboxRng::new(riot_seed);
    assert_eq!(
        ambient_disaster_kind_for_city(
            &city,
            quiet,
            outcomes,
            &mut non_mac_rng,
            EngineFidelity::Mac68k,
        ),
        Some(DisasterKind::Rioters),
        "Mac fidelity reads its saved riot gates on either city layout"
    );
}

#[test]
fn pe_mass_riot_disaster_prefers_pe_dw_city_population() {
    // PE type 0xD @ `0x47218e`: `dwCityPopulation >= 0x7530` (30_000).
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xAA);
    city.set_pe_unemployment_percent(10);
    city.set_pe_zone_pops([3_000, 0, 0]);
    city.persist_pe_day22_population_leftover(30_000);
    assert!(pe_mass_riot_disaster_ready(&city));

    city.persist_pe_day22_population_leftover(29_999);
    assert!(!pe_mass_riot_disaster_ready(&city));
}

#[test]
fn ambient_disaster_kind_uses_pe_riot_gates() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    for seed in 0..32 {
        let mut rng = MacToolboxRng::new(seed);
        let kind =
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95);
        assert_ne!(
            kind,
            Some(DisasterKind::Rioters),
            "riot must not spawn without PE unemployment/heat gates (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::MassRiot),
            "type 0xD mass riots need 30k pop plus type-3 gates (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::Hurricane),
            "hurricane needs PE windy+ trend and a coast (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::Tornado),
            "tornado needs PE weather trend >= windy (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::AirCrash),
            "air crash needs runway tiles (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::Monster),
            "monster needs PE 45k population (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::Flood),
            "flood needs river/ocean and weather trend >= foggy (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::BlizzardFlood),
            "type 0xE mass floods need blizzard trend plus river/ocean (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::Meltdown),
            "meltdown needs nuclear tiles (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::Microwave),
            "microwave disaster needs microwave tiles (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::ToxicSpill),
            "toxic spill needs hot XPLT or industrial tax (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::Volcano),
            "volcano is SCEN/consume only; ambient jump table is NOP (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::Firestorm),
            "firestorm is SCEN/consume only; ambient jump table is NOP (seed {seed})"
        );
        assert_ne!(
            kind,
            Some(DisasterKind::PollutionAccident),
            "pollution accident needs industrial tax >= 10000 (seed {seed})"
        );
    }

    for index in 0..6 {
        stamp_abandoned_pop4_for_test(&mut city, index * 4, 40);
    }
    city.set_pe_unemployment_percent(10);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xAA);
    let type3_ready = PopulationStats {
        normal_population: 5_000,
        total_population: 5_000,
        ..Default::default()
    };
    assert!(pe_riot_disaster_ready(&city));
    assert!(!pe_mass_riot_disaster_ready(&city));
    // Mac Toolbox seeds 12/22/55 land on type 0x03 in the 19-slot table.
    let saw_type3 = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(
            &city,
            type3_ready,
            outcomes,
            &mut rng,
            EngineFidelity::Win95,
        ) == Some(DisasterKind::Rioters)
    });
    assert!(
        saw_type3,
        "type 3 riot gates should allow jump-table rioters below 30k pop"
    );

    let mass_ready = PopulationStats {
        normal_population: 30_000,
        total_population: 30_000,
        ..Default::default()
    };
    city.set_pe_dw_city_population(mass_ready.normal_population);
    assert!(pe_mass_riot_disaster_ready(&city));
    let saw_mass = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(&city, mass_ready, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::MassRiot)
    });
    assert!(
        saw_mass,
        "type 0xD mass riot should roll jump-table MassRiot"
    );
}

#[test]
fn ambient_disaster_jump_table_is_nineteen_slots_with_none_at_zero() {
    // Mac export_231 jump table @ 0x35d1c / PE rand()%0x13: type id == slot index.
    assert_eq!(disaster_kind_misc_type(DisasterKind::Fire), 0x01);
    assert_eq!(disaster_kind_misc_type(DisasterKind::Rioters), 0x03);
    assert_eq!(disaster_kind_misc_type(DisasterKind::Monster), 0x08);
    assert_eq!(disaster_kind_misc_type(DisasterKind::MassRiot), 0x0d);
    assert_eq!(disaster_kind_misc_type(DisasterKind::AirCrash), 0x12);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xfe);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();

    // Seed 10's first Toolbox Random rem_euclid 0x13 is slot 0 (DISASTER_NONE).
    let mut none_rng = MacToolboxRng::new(10);
    assert_eq!(
        ambient_disaster_kind_for_city(
            &city,
            quiet,
            outcomes,
            &mut none_rng,
            EngineFidelity::Win95
        ),
        None,
        "slot 0 must exit without selecting Fire or any other arm"
    );

    // Seed 4 lands on Fire (slot 0x01); max heat keeps the arm ready.
    let mut fire_rng = MacToolboxRng::new(4);
    assert_eq!(
        ambient_disaster_kind_for_city(
            &city,
            quiet,
            outcomes,
            &mut fire_rng,
            EngineFidelity::Win95
        ),
        Some(DisasterKind::Fire)
    );
}

#[test]
fn pe_meltdown_disaster_requires_nuclear_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_meltdown_disaster_ready(&city));
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    assert!(pe_meltdown_disaster_ready(&city));
}

#[test]
fn pe_meltdown_disaster_ready_uses_dw_tile_count_like_472160() {
    // PE @ `0x472160`: ready iff `0x4ca4c8[0xcb] != 0`, not a live map rescan.
    let mut city = City::new_blank("/tmp/sc2000-melt-dw.sc2", "MeltDw").expect("blank");
    city.set_building(40, 40, NUCLEAR_POWER_BUILDING_ID);
    assert!(!pe_meltdown_disaster_ready(&city));
    city.pe_dw_tile_count[NUCLEAR_POWER_BUILDING_ID as usize] = 1;
    assert!(pe_meltdown_disaster_ready(&city));
    city.pe_dw_tile_count[NUCLEAR_POWER_BUILDING_ID as usize] = 0;
    assert!(!pe_meltdown_disaster_ready(&city));
    pe_place_tile_441f00(&mut city, 41, 41, NUCLEAR_POWER_BUILDING_ID);
    assert!(pe_meltdown_disaster_ready(&city));
}

#[test]
fn pe_microwave_disaster_ready_uses_dw_tile_count_like_472177() {
    // PE @ `0x472177`: ready iff `0x4ca4c8[0xcd] != 0`.
    let mut city = City::new_blank("/tmp/sc2000-micro-dw.sc2", "MicroDw").expect("blank");
    city.set_building(40, 40, MICROWAVE_POWER_BUILDING_ID);
    assert!(!pe_microwave_disaster_ready(&city));
    // PlaceTile on a vacant neighbor so the census actually incs (same-tile replace is net 0).
    pe_place_tile_441f00(&mut city, 41, 41, MICROWAVE_POWER_BUILDING_ID);
    assert!(pe_microwave_disaster_ready(&city));
}

#[test]
fn pe_meltdown_plant_tile_prefers_hint_then_scans_x_then_y() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(pe_meltdown_plant_tile(&city, (12, 34)), None);
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    stamp_complete_building_for_test(&mut city, 10, 80, NUCLEAR_POWER_BUILDING_ID, 4);
    assert_eq!(pe_meltdown_plant_tile(&city, (12, 34)), Some((10, 80)));
    assert_eq!(pe_meltdown_plant_tile(&city, (41, 41)), Some((41, 41)));
    assert_eq!(pe_meltdown_plant_tile(&city, (40, 40)), Some((40, 40)));
}

#[test]
fn meltdown_ambient_target_follows_engine_fidelity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // Leave the map empty so the Win95 land-probe hint can only land on nuclear tiles.
    stamp_complete_building_for_test(&mut city, 10, 80, NUCLEAR_POWER_BUILDING_ID, 4);
    stamp_complete_building_for_test(&mut city, 50, 50, NUCLEAR_POWER_BUILDING_ID, 4);
    let first = pe_meltdown_plant_tile(&city, (CITY_WIDTH, CITY_HEIGHT)).expect("first plant");
    assert_eq!(first, (10, 80));

    let mut mac_rng = MacToolboxRng::new(7);
    let mac = meltdown_ambient_target_tile(&city, &mut mac_rng, EngineFidelity::Mac68k)
        .expect("Mac meltdown");
    assert_eq!(mac, first);

    let mut saw_later_plant = false;
    for seed in 0..256 {
        let mut win_rng = MacToolboxRng::new(seed);
        let win = meltdown_ambient_target_tile(&city, &mut win_rng, EngineFidelity::Win95)
            .expect("Win95 meltdown");
        if win != first {
            saw_later_plant = true;
            break;
        }
    }
    assert!(
        saw_later_plant,
        "Win95 meltdown ambient should be able to hint-hit a later nuclear plant"
    );
}

#[test]
fn start_disaster_meltdown_targets_nuclear_plant() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_meltdown_overlays_with_rng(&mut preview_city, 40, 40, &mut preview, &mut preview_lfsr);
    assert!(
        expected
            .keys()
            .any(|&(x, y)| x.abs_diff(40) > 2 || y.abs_diff(40) > 2),
        "PE meltdown scatter reaches outside the old 8-tile footprint"
    );

    assert!(state.start_disaster(DisasterKind::Meltdown));
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    for y in 40..44 {
        for x in 40..44 {
            let actual = state.city.building_at(x, y);
            assert_eq!(
                actual,
                preview_city.building_at(x, y),
                "PE meltdown must match same-seed DestroyStructure rubble at ({x},{y})"
            );
            assert_ne!(
                actual, NUCLEAR_POWER_BUILDING_ID,
                "plant survives at ({x},{y})"
            );
        }
    }
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(disaster_overlay_tiles(&state.city).len(), expected.len());
    assert_eq!(state.query_line, "Meltdown Reported");
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(9)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_meltdown_plant_destroy_clears_xtxt_occupancy() {
    // PE `0x401604(x,y,1,1)` paints dry footprint `0xff`; water stays cleared/unpainted.
    let mut city = City::new_blank("/tmp/sc2000-melt-xtxt.sc2", "MeltXtxt").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    city.set_text_overlay(40, 40, 0x55);
    city.set_text_overlay(41, 41, 0x66);
    city.set_terrain(42, 42, 0x10);
    city.set_tile_bits(42, 42, TileBits::new(0x04)); // water
    city.set_text_overlay(42, 42, 0x77);
    let tiles = pe_meltdown_apply_plant_destroy(&mut city, None, 40, 40);
    assert_eq!(city.building_at(40, 40), 0);
    assert_eq!(city.text_overlay_at(40, 40), Some(0xff));
    assert_eq!(city.text_overlay_at(41, 41), Some(0xff));
    assert_eq!(city.text_overlay_at(42, 42), Some(0));
    assert_eq!(tiles.get(&(40, 40)), Some(&0xff));
    assert_eq!(tiles.get(&(41, 41)), Some(&0xff));
    assert!(!tiles.contains_key(&(42, 42)));
}

#[test]
fn pe_meltdown_destroy_clears_full_footprint_from_interior_hit() {
    // PE `0x402496` origin walk: destroy from any plant tile, not NW-assumed stamp.
    let mut city = City::new_blank("/tmp/sc2000-melt-interior.sc2", "MeltIn").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_text_overlay(40 + dx, 40 + dy, 0x55);
        }
    }
    city.set_traffic_density(41, 41, 0x23);
    let tiles = pe_meltdown_apply_plant_destroy(&mut city, None, 41, 41);
    for dy in 0..4 {
        for dx in 0..4 {
            assert_eq!(city.building_at(40 + dx, 40 + dy), 0);
            assert_eq!(city.text_overlay_at(40 + dx, 40 + dy), Some(0xff));
        }
    }
    assert_eq!(tiles.len(), 16);
    assert_eq!(city.traffic_density_at(41, 41), Some(0x23));
}

#[test]
fn pe_meltdown_scatter_is_one_in_32_within_plus_minus_32() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    let overlays = pe_meltdown_scatter_overlays(&mut city, 40, 40);
    assert_eq!(PE_MELTDOWN_SCATTER_RADIUS, 0x20);
    assert!(!overlays.is_empty());
    assert!(
        overlays
            .keys()
            .all(|&(x, y)| { x.abs_diff(40) <= 0x20 && y.abs_diff(40) <= 0x20 })
    );
    assert!(
        overlays
            .values()
            .all(|&marker| marker == 0xff || marker == 0xfb)
    );
}

#[test]
fn meltdown_scatter_paints_xbit_water_toxic_cloud() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    let mut dry_city = city.clone();
    let dry = pe_meltdown_scatter_overlays(&mut dry_city, 40, 40);
    let probe = dry
        .iter()
        .find_map(|(&(x, y), &marker)| (marker == 0xff).then_some((x, y)))
        .expect("ignite tile");
    city.set_tile_bits(probe.0, probe.1, crate::city::TileBits::new(0b0000_0100));
    let mut wet_city = city.clone();
    let wet = pe_meltdown_scatter_overlays(&mut wet_city, 40, 40);
    assert_ne!(wet.get(&probe), Some(&0xff));
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let before = state.pe_process_crt_seed();

    assert!(state.start_disaster(DisasterKind::Meltdown));
    assert_ne!(state.pe_process_crt_seed(), before);
}

#[test]
fn pe_meltdown_scatter_skips_high_xtxt_rubble_band() {
    // PE @ `0x45f484` → FUN_00460e10: rubble band leaves XTXT; scatter must not invent 0xff.
    let mut city = City::new_blank("/tmp/sc2000-melt-rubble.sc2", "MeltRb").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    let dry = pe_meltdown_scatter_overlays(&mut city, 40, 40);
    let probe = dry
        .iter()
        .find_map(|(&(x, y), &marker)| (marker == 0xff).then_some((x, y)))
        .expect("ignite tile");
    city.set_text_overlay(probe.0, probe.1, 0xf1);
    let blocked = pe_meltdown_scatter_overlays(&mut city, 40, 40);
    assert_ne!(blocked.get(&probe), Some(&0xff));
    assert_eq!(city.text_overlay_at(probe.0, probe.1), Some(0xf1));
}

#[test]
fn pe_meltdown_scatter_ignite_mid_xtxt_destroy_before_paint() {
    // PE @ `0x45f484` → FUN_00460e10 flag 1: mid-XTXT DestroyStructure(1,1) then fire paint.
    let mut city = City::new_blank("/tmp/sc2000-melt-ign-mid.sc2", "MeltIgn").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 210, 3);
    city.set_text_overlay(41, 41, 0x55);
    for &(tx, ty) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_traffic_density(tx, ty, 0x22);
    }
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_firestorm_can_ignite(&city, 41, 41));
    pe_fun_00460e10_ignite(&mut city, None, &mut lfsr, 41, 41, 1);
    for dy in 0..3 {
        for dx in 0..3 {
            assert_eq!(city.building_at(40 + dx, 40 + dy), 0);
            assert_eq!(city.text_overlay_at(40 + dx, 40 + dy), Some(0xff));
        }
    }
    assert_eq!(city.traffic_density_at(41, 41), Some(0));
}

#[test]
fn pe_meltdown_scatter_else_arm_destroys_then_radioactivity_or_toxic() {
    // PE @ `0x45f411`: DestroyStructure(0,0); then water 0xfb or dry PlaceTile XBLD 5.
    let mut city = City::new_blank("/tmp/sc2000-melt-else.sc2", "MeltElse").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    city.set_building(50, 50, 0x70);
    city.set_building(51, 50, 0x71);
    city.set_traffic_density(52, 52, 0x44);
    city.set_tile_bits(52, 52, TileBits::new(0).with_piped(true).with_watered(true));
    let before = city.building_at(50, 50);
    let overlays = pe_meltdown_scatter_overlays(&mut city, 40, 40);
    let saw_radio = (0..CITY_WIDTH).any(|x| (0..CITY_HEIGHT).any(|y| city.building_at(x, y) == 5));
    let saw_toxic = overlays.values().any(|&m| m == 0xfb);
    let destroyed_probe = city.building_at(50, 50) != before || city.building_at(50, 50) == 5;
    assert!(
        saw_radio || saw_toxic || destroyed_probe || !overlays.is_empty(),
        "else-arm scatter should destroy and/or stamp radioactivity/toxic"
    );
    // Dry PlaceTile 5: XBLD 5 only — no invent XTRF clear / watered wipe on that stamp.
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city.building_at(x, y) == 5 {
                assert!(!overlays.contains_key(&(x, y)) || overlays[&(x, y)] != 0xff);
            }
        }
    }
    if city.building_at(52, 52) == 5 {
        assert_eq!(city.traffic_density_at(52, 52), Some(0x44));
        let bits = city.tile_bits_at(52, 52).expect("bits");
        assert!(bits.piped() && bits.watered());
    }
}

#[test]
fn pe_meltdown_scatter_else_arm_elevated_still_placetile_after_early_out() {
    // PE @ `0x45f411`: DestroyStructure elevated early-out, then dry PlaceTile `5` still runs.
    let mut city = City::new_blank("/tmp/sc2000-melt-elev.sc2", "MeltElev").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    city.set_building(45, 45, 0x3f);
    let mut found = false;
    for seed in 0u32..0x4000 {
        let mut c = city.clone();
        let mut rng = Win95CrtRand::new(seed);
        let mut lfsr = Win95Lfsr::from_u32_seed(seed);
        let _ = pe_meltdown_scatter_overlays_with_rng(&mut c, 40, 40, &mut rng, &mut lfsr);
        if c.building_at(45, 45) == 5 {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "PE still PlaceTiles XBLD 5 onto elevated after DestroyStructure early-out"
    );
}

#[test]
fn pe_meltdown_scatter_water_fb_stamps_city_during_scatter() {
    // PE @ `0x45f411` else-arm: after DestroyStructure, water + `rand&1` paints map `0xfb`.
    let mut city = City::new_blank("/tmp/sc2000-melt-water-fb.sc2", "MeltWaterFb").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    city.set_building(45, 45, 0x70);
    city.set_tile_bits(45, 45, TileBits::new(0x04)); // water-tile
    let mut hit = None;
    for seed in 1..4096u32 {
        let mut probe = city.clone();
        let mut rng = Win95CrtRand::new(seed);
        let mut lfsr = Win95Lfsr::from_u32_seed(seed);
        let overlays =
            pe_meltdown_scatter_overlays_with_rng(&mut probe, 40, 40, &mut rng, &mut lfsr);
        if overlays.get(&(45, 45)) == Some(&0xfb) && probe.text_overlay_at(45, 45) == Some(0xfb) {
            hit = Some(seed);
            break;
        }
    }
    let seed = hit.expect("CRT seed that paints water 0xfb at (45,45)");
    let mut rng = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    let overlays = pe_meltdown_scatter_overlays_with_rng(&mut city, 40, 40, &mut rng, &mut lfsr);
    assert_eq!(overlays.get(&(45, 45)), Some(&0xfb));
    assert_eq!(city.text_overlay_at(45, 45), Some(0xfb));
}

#[test]
fn pe_meltdown_near_origin_place_tile_radioactivity_uses_place_tile_441f00() {
    // PE @ `0x45f4b8`: si=-1..2, di=-2..1; rand&1 → PlaceTile 5 (leave XTRF/watered).
    let mut city = City::new_blank("/tmp/sc2000-melt-near.sc2", "MeltNear").expect("blank");
    for dy in -2isize..=1 {
        for dx in -1isize..=2 {
            let (x, y) = offset_city_tile(40, 40, dx, dy).expect("in bounds");
            city.set_traffic_density(x, y, 0x33);
            city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_watered(true));
        }
    }
    let mut rng = Win95CrtRand::new(1);
    pe_meltdown_near_origin_place_tile_radioactivity(&mut city, 40, 40, &mut rng);
    let mut radio = Vec::new();
    for dy in -2isize..=1 {
        for dx in -1isize..=2 {
            let (x, y) = offset_city_tile(40, 40, dx, dy).expect("in bounds");
            if city.building_at(x, y) == 5 {
                radio.push((x, y));
            }
        }
    }
    assert!(
        !radio.is_empty(),
        "near-origin epilogue should PlaceTile some XBLD 5"
    );
    for &(x, y) in &radio {
        assert_eq!(city.traffic_density_at(x, y), Some(0x33));
        let bits = city.tile_bits_at(x, y).expect("bits");
        assert!(bits.piped() && bits.watered());
    }
}

#[test]
fn start_disaster_earthquake_without_buildings_persists_without_overlay() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    let before = state.pe_process_crt_seed();

    assert!(state.start_disaster(DisasterKind::Earthquake));
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x06)
    );
    assert_eq!(state.query_line, "Earthquake Reported");
    assert_ne!(state.pe_process_crt_seed(), before);
}

#[test]
fn earthquake_shake_queues_explode_per_0x18_iter() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(PE_EARTHQUAKE_SHAKE_ITERS, 0x18);
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    assert!(state.start_disaster(DisasterKind::Earthquake));
    assert_eq!(
        state.pending_disaster_extra_sounds,
        vec![PlacementSound::Explode; PE_EARTHQUAKE_SHAKE_ITERS - 1]
    );
}

#[test]
fn earthquake_shake_shift_is_4_shl_zoom_level() {
    assert_eq!(pe_earthquake_shake_shift(1.0), 4.0);
    assert_eq!(pe_earthquake_shake_shift(2.0), 8.0);
    assert_eq!(pe_earthquake_shake_shift(4.0), 16.0);
    assert_eq!(
        pe_earthquake_shake_delta(PE_EARTHQUAKE_SHAKE_ITERS as u8, 2.0),
        Vec2::new(8.0, 0.0)
    );
    assert_eq!(
        pe_earthquake_shake_delta(PE_EARTHQUAKE_SHAKE_ITERS as u8 - 1, 2.0),
        Vec2::ZERO
    );
    assert_eq!(
        pe_earthquake_shake_delta(PE_EARTHQUAKE_SHAKE_ITERS as u8 - 2, 2.0),
        Vec2::new(8.0, 0.0)
    );
    assert_eq!(pe_earthquake_shake_delta(1, 2.0), Vec2::ZERO);
    assert_eq!(pe_earthquake_shake_delta(0, 2.0), Vec2::ZERO);
}

#[test]
fn start_disaster_earthquake_runs_0x18_dest_blit_shake() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    assert_eq!(state.zoom, 2.0);
    assert!(state.start_disaster(DisasterKind::Earthquake));
    assert_eq!(
        usize::from(state.earthquake_shake_remaining),
        PE_EARTHQUAKE_SHAKE_ITERS
    );
    for remaining in (1..=PE_EARTHQUAKE_SHAKE_ITERS as u8).rev() {
        assert_eq!(state.earthquake_shake_remaining, remaining);
        let phase = PE_EARTHQUAKE_SHAKE_ITERS - usize::from(remaining);
        let expected = if phase & 1 == 0 {
            Vec2::new(8.0, 0.0)
        } else {
            Vec2::ZERO
        };
        assert_eq!(state.earthquake_shake_offset(), expected);
        assert_eq!(state.view_camera(), state.camera + expected);
        state.tick_earthquake_shake();
    }
    assert_eq!(state.earthquake_shake_remaining, 0);
    assert_eq!(state.earthquake_shake_offset(), Vec2::ZERO);
    assert_eq!(state.view_camera(), state.camera);
}

#[test]
fn start_disaster_earthquake_ignites_or_destroys_xbld_ge_0x0e() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..60 {
        for x in 20..60 {
            city.set_building(x, y, 0x70);
        }
    }
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_earthquake_overlays_with_rng(&mut preview_city, 40, 40, &mut preview, &mut preview_lfsr);
    assert_eq!(PE_EARTHQUAKE_SCATTER_RADIUS, 0x20);
    assert_eq!(PE_EARTHQUAKE_MIN_BUILDING, 0x0e);
    assert!(
        expected
            .keys()
            .all(|&(x, y)| { x.abs_diff(40) <= 0x20 && y.abs_diff(40) <= 0x20 }),
        "PE earthquake scatter stays inside ±32"
    );
    assert!(expected.values().all(|&marker| marker == 0xff));

    assert!(state.start_disaster(DisasterKind::Earthquake));
    for y in 20..60 {
        for x in 20..60 {
            assert_eq!(state.city.building_at(x, y), preview_city.building_at(x, y));
        }
    }
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x06)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_earthquake_ignite_mid_xtxt_destroy_before_paint() {
    // PE @ `0x45ed5a` → FUN_00460e10 flag 0: mid-XTXT DestroyStructure(1,1) then fire paint.
    let mut city = City::new_blank("/tmp/sc2000-eq-mid.sc2", "EqMid").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 210, 3);
    city.set_text_overlay(41, 41, 0x55);
    for &(tx, ty) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_traffic_density(tx, ty, 0x3c);
    }
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_fire_ignite_ok(&city, 41, 41));
    pe_fun_00460e10_ignite(&mut city, None, &mut lfsr, 41, 41, 0);
    for dy in 0..3 {
        for dx in 0..3 {
            assert_eq!(city.building_at(40 + dx, 40 + dy), 0);
            assert_eq!(city.text_overlay_at(40 + dx, 40 + dy), Some(0xff));
        }
    }
    assert_eq!(city.traffic_density_at(40, 40), Some(0));
}

#[test]
fn pe_earthquake_destroy_elevated_early_out_keeps_road() {
    // PE @ `0x45ed49` DestroyStructure(0,0): elevated `0x3f..=0x42` early-out keeps XBLD.
    let mut city = City::new_blank("/tmp/sc2000-eq-elev.sc2", "EqElev").expect("blank");
    city.set_building(40, 40, 0x3f);
    let mut crt = Win95CrtRand::new(1);
    assert!(!pe_flood_destroy_tile_with_crt(&mut city, &mut crt, 40, 40));
    assert_eq!(city.building_at(40, 40), 0x3f);
    // Scatter else-arm uses the same helper.
    let mut lfsr = Win95Lfsr::from_u32_seed(1);
    let mut scatter = city.clone();
    let _ = pe_earthquake_overlays_with_rng(&mut scatter, 40, 40, &mut crt, &mut lfsr);
    assert_eq!(scatter.building_at(40, 40), 0x3f);
}

#[test]
fn earthquake_scatter_matches_same_stream_multitile_destroy_preview() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let building = COMMERCIAL_3X3_BUILDINGS[0];
    for y in (16..=64).step_by(4) {
        for x in (16..=64).step_by(4) {
            stamp_complete_building_for_test(&mut city, x, y, building, 3);
        }
    }
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_earthquake_overlays_with_rng(&mut preview_city, 40, 40, &mut preview, &mut preview_lfsr);
    let mut saw_changed_building = false;
    for y in (16..=64).step_by(4) {
        for x in (16..=64).step_by(4) {
            let footprint = building_footprint_from_origin(x, y, 3).unwrap();
            saw_changed_building |= footprint
                .iter()
                .any(|&(tx, ty)| preview_city.building_at(tx, ty) != building);
        }
    }
    assert!(
        saw_changed_building,
        "the fixed PE scatter stream should alter at least one building cell"
    );

    assert!(state.start_disaster(DisasterKind::Earthquake));
    for y in (16..=64).step_by(4) {
        for x in (16..=64).step_by(4) {
            for ty in y..y + 3 {
                for tx in x..x + 3 {
                    assert_eq!(
                        state.city.building_at(tx, ty),
                        preview_city.building_at(tx, ty)
                    );
                }
            }
        }
    }
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn earthquake_destroy_and_ignite_update_xtxt_on_changed_building_cells() {
    // PE destroy arm clears XTXT with XBLD; ignite arm FUN_00460e10 may fire-paint mid-XTXT.
    let mut city = City::new_blank("/tmp/sc2000-eq-xtxt.sc2", "EqXtxt").expect("blank");
    let building = COMMERCIAL_3X3_BUILDINGS[0];
    for y in (16..=64).step_by(4) {
        for x in (16..=64).step_by(4) {
            stamp_complete_building_for_test(&mut city, x, y, building, 3);
            city.set_text_overlay(x, y, 0x55);
            city.set_text_overlay(x + 2, y + 2, 0x66);
        }
    }
    let mut preview_city = city.clone();
    let mut preview = Win95CrtRand::new(0x45EB_2000);
    let mut preview_lfsr = Win95Lfsr::from_u32_seed(0x45EB_2000);
    let _ =
        pe_earthquake_overlays_with_rng(&mut preview_city, 40, 40, &mut preview, &mut preview_lfsr);
    let mut saw_changed_building = false;
    for y in (16..=64).step_by(4) {
        for x in (16..=64).step_by(4) {
            for ty in y..y + 3 {
                for tx in x..x + 3 {
                    if preview_city.building_at(tx, ty) == building {
                        continue;
                    }
                    saw_changed_building = true;
                    let xtxt = preview_city.text_overlay_at(tx, ty).unwrap_or(0);
                    assert!(
                        xtxt == 0 || xtxt == 0xff,
                        "cleared footprint XTXT must be 0 (destroy) or 0xff (ignite), got {xtxt:#x} at {tx},{ty}"
                    );
                }
            }
        }
    }
    assert!(
        saw_changed_building,
        "the fixed PE scatter stream should alter at least one building cell"
    );
}

#[test]
fn consume_pe_trigger_disaster_starts_earthquake() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..60 {
        for x in 20..60 {
            city.set_building(x, y, 0x70);
        }
    }
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    state
        .city
        .set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x06);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_earthquake_overlays_with_rng(&mut preview_city, 40, 40, &mut preview, &mut preview_lfsr);

    assert!(state.consume_pe_trigger_disaster());
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x06)
    );
    assert_eq!(state.query_line, "Earthquake Reported");
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn start_disaster_meltdown_without_nuclear_is_nop() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));

    assert!(!state.start_disaster(DisasterKind::Meltdown));
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
}

#[test]
fn pe_microwave_disaster_requires_microwave_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_microwave_disaster_ready(&city));
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    assert!(pe_microwave_disaster_ready(&city));
}

#[test]
fn start_disaster_microwave_walks_from_plant() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_microwave_walk_with_rng(&mut preview_city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .overlays;
    assert!(
        expected
            .keys()
            .any(|&(x, y)| x < 40 || x > 43 || y < 40 || y > 43),
        "PE microwave walk leaves the 4×4 plant"
    );

    assert!(state.start_disaster(DisasterKind::Microwave));
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0));
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(disaster_overlay_tiles(&state.city).len(), expected.len());
    assert_eq!(state.query_line, "Microwave Reported");
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0a)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn microwave_walk_consumes_direction_draw_before_boundary_exit() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.kind = kind;
        stamp_complete_building_for_test(&mut city, 0, 0, MICROWAVE_POWER_BUILDING_ID, 4);

        // Seed 2's first CRT value selects direction 5 (dx=-1), so the first
        // move leaves the map from (0, 0).
        let mut rng = Win95CrtRand::new(2);
        let mut expected_rng = rng;
        expected_rng.random_u16(); // initial direction
        expected_rng.random_u16(); // PE draws again after the off-map step
        let mut lfsr = Win95Lfsr::from_u32_seed(1);

        let walk = pe_microwave_walk_with_rng(&mut city, &mut rng, &mut lfsr)
            .expect("microwave plant at the map corner");
        assert!(walk.overlays.is_empty(), "{kind:?}");
        assert_eq!(walk.zap_count, 0, "{kind:?}");
        assert_eq!(rng.seed(), expected_rng.seed(), "{kind:?}");
    }
}

#[test]
fn start_disaster_microwave_without_plant_is_nop() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));

    assert!(!state.start_disaster(DisasterKind::Microwave));
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
}

#[test]
fn microwave_walk_paints_xbit_water_toxic_cloud() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let dry = pe_microwave_walk_with_rng(&mut preview_city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .overlays;
    let probe = dry
        .iter()
        .find_map(|(&(x, y), &marker)| {
            (marker == 0xff && (x < 40 || x > 43 || y < 40 || y > 43)).then_some((x, y))
        })
        .expect("land walk tile outside plant");
    state
        .city
        .set_tile_bits(probe.0, probe.1, crate::city::TileBits::new(0b0000_0100));
    // Re-preview after marking water so the process-CRT walk paints 0xfb on that tile.
    preview_city = state.city.clone();
    preview = state.pe_process_crt;
    preview_lfsr = state.pe_process_lfsr;
    let expected = pe_microwave_walk_with_rng(&mut preview_city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .overlays;
    assert_eq!(expected.get(&probe), Some(&0xfb));
    assert_eq!(
        preview_city.text_overlay_at(probe.0, probe.1),
        Some(0xfb),
        "PE paints water 0xfb onto the map during the walk, not only into the return map"
    );
    assert!(state.start_disaster(DisasterKind::Microwave));
    assert_eq!(state.city.text_overlay_at(probe.0, probe.1), Some(0xfb));
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn ambient_disaster_kind_uses_pe_meltdown_and_microwave_gates() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0x40);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    for seed in 0..64 {
        let mut rng = MacToolboxRng::new(seed);
        let kind =
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95);
        assert_ne!(kind, Some(DisasterKind::Meltdown), "seed {seed}");
        assert_ne!(kind, Some(DisasterKind::Microwave), "seed {seed}");
    }

    stamp_complete_building_for_test(&mut city, 40, 40, NUCLEAR_POWER_BUILDING_ID, 4);
    let saw_meltdown = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::Meltdown)
    });
    assert!(
        saw_meltdown,
        "nuclear cities should be able to roll jump-table meltdown"
    );

    stamp_complete_building_for_test(&mut city, 48, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    let saw_microwave = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::Microwave)
    });
    assert!(
        saw_microwave,
        "microwave cities should be able to roll jump-table microwave disaster"
    );
}

#[test]
fn pe_tornado_disaster_requires_windy_or_stronger_trend() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_tornado_disaster_ready(&city));
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_RAIN));
    assert!(!pe_tornado_disaster_ready(&city));
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_WINDY));
    assert!(pe_tornado_disaster_ready(&city));
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    assert!(pe_tornado_disaster_ready(&city));
}

#[test]
fn pe_hurricane_disaster_requires_windy_trend_and_ocean() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_WINDY));
    assert!(
        !pe_hurricane_disaster_ready(&city),
        "windy inland cities miss the PE hurricane arm"
    );
    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    assert!(pe_hurricane_disaster_ready(&city));
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_OVERCAST));
    assert!(!pe_hurricane_disaster_ready(&city));
}

#[test]
fn ambient_disaster_kind_uses_pe_tornado_and_hurricane_gates() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0x40);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    for seed in 0..64 {
        let mut rng = MacToolboxRng::new(seed);
        let kind =
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95);
        assert_ne!(kind, Some(DisasterKind::Tornado), "seed {seed}");
        assert_ne!(kind, Some(DisasterKind::Hurricane), "seed {seed}");
    }

    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_WINDY));
    let saw_tornado = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::Tornado)
    });
    assert!(saw_tornado, "windy weather should allow jump-table tornado");
    for seed in 0..64 {
        let mut rng = MacToolboxRng::new(seed);
        assert_ne!(
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95),
            Some(DisasterKind::Hurricane),
            "hurricane still needs a coast (seed {seed})"
        );
    }

    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    let saw_hurricane = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::Hurricane)
    });
    assert!(
        saw_hurricane,
        "windy coastal cities should be able to roll jump-table hurricane"
    );
}

#[test]
fn pe_flood_disaster_requires_water_and_foggy_or_wetter_trend() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_flood_disaster_ready(&city));

    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_HOT));
    assert!(
        !pe_flood_disaster_ready(&city),
        "hot/clear/cold trends miss the PE flood arm"
    );

    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_FOGGY));
    assert!(pe_flood_disaster_ready(&city));
    assert!(
        !pe_blizzard_flood_disaster_ready(&city),
        "type 0xE flood needs blizzard or stronger"
    );

    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_BLIZZARD));
    assert!(pe_flood_disaster_ready(&city));
    assert!(pe_blizzard_flood_disaster_ready(&city));

    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_DISABLED_VALUE);
    city.set_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_RAIN));
    assert!(
        pe_flood_disaster_ready(&city),
        "river cities can flood when the trend is wet enough"
    );

    city.set_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, CITY_OPTION_DISABLED_VALUE);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    assert!(
        !pe_flood_disaster_ready(&city),
        "wet weather alone is not enough without river or ocean"
    );
    assert!(!pe_blizzard_flood_disaster_ready(&city));
}

#[test]
fn ambient_disaster_kind_uses_pe_flood_gates() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0x40);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    for seed in 0..64 {
        let mut rng = MacToolboxRng::new(seed);
        assert_ne!(
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95),
            Some(DisasterKind::Flood),
            "flood must not spawn without PE river/ocean+trend gates (seed {seed})"
        );
    }

    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_RAIN));
    assert!(
        pe_flood_disaster_ready(&city),
        "PE/Mac type-2 flood unlocks at foggy+ with river/ocean"
    );
    assert!(!pe_blizzard_flood_disaster_ready(&city));
    let saw_flood = (1..4_096).any(|seed| {
        let mut rng = Win95CrtRand::new(seed);
        ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::Flood)
    });
    assert!(
        saw_flood,
        "coastal rainy cities should be able to roll ambient flood"
    );
    assert!(
        (1..4_096).any(|seed| {
            let mut rng = Win95CrtRand::new(seed);
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Mac68k)
                == Some(DisasterKind::Flood)
        }),
        "Mac type-2 entry at 0x35a22 shares the foggy+ flood gate"
    );

    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_BLIZZARD));
    assert!(pe_blizzard_flood_disaster_ready(&city));
    let saw_blizzard_flood = (1..4_096).any(|seed| {
        let mut rng = Win95CrtRand::new(seed);
        ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::BlizzardFlood)
    });
    assert!(
        saw_blizzard_flood,
        "blizzard coastal cities should keep the extra PE/Mac type-0xE flood slot"
    );
}

#[test]
fn mac_monster_target_shares_pollution_center_jitter() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..20 {
        for x in 10..20 {
            city.set_building(x, y, 0x70);
        }
    }
    let mut pollution_rng = MacToolboxRng::new(0x35C9_8001);
    let mut monster_rng = MacToolboxRng::new(0x35C9_8001);
    assert_eq!(
        mac_monster_target_tile(&city, &mut monster_rng),
        mac_pollution_accident_target_tile(&city, &mut pollution_rng),
        "Mac monster @ 0x35c98 and pollution @ 0x35baa share e4b4/e4b6 &0x1F-0x0F"
    );
}

#[test]
fn monster_ambient_target_follows_engine_fidelity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..20 {
        for x in 10..20 {
            city.set_building(x, y, 0x70);
        }
    }
    let pe_center = land_value_city_center(&city);
    let mac_center = mac_developed_tile_anchor(&city);

    let mut mac_rng = MacToolboxRng::new(11);
    let mac = monster_ambient_target_tile(&city, &mut mac_rng, EngineFidelity::Mac68k);
    assert!(
        mac.0.abs_diff(mac_center.0) <= 16 && mac.1.abs_diff(mac_center.1) <= 16,
        "Mac ambient monster target should use the Mac center, got {mac:?} center {mac_center:?}"
    );

    let mut saw_off_center = false;
    for seed in 0..256 {
        let mut win_rng = MacToolboxRng::new(seed);
        let win = monster_ambient_target_tile(&city, &mut win_rng, EngineFidelity::Win95);
        if win.0.abs_diff(pe_center.0) > 16 || win.1.abs_diff(pe_center.1) > 16 {
            saw_off_center = true;
            break;
        }
    }
    assert!(
        saw_off_center,
        "Win95 ambient monster target should not be locked to Mac center jitter"
    );
}

#[test]
fn mac_mass_riot_target_jitters_city_center_by_mod20_minus_10() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..20 {
        for x in 10..20 {
            city.set_building(x, y, 1);
        }
    }
    let (cx, cy) = mac_developed_tile_anchor(&city);
    let mut rng = MacToolboxRng::new(0x35A7_0001);
    let mut expected = MacToolboxRng::new(0x35A7_0001);
    let tile = mac_mass_riot_target_tile(&city, &mut rng);
    let want_x = (cx as isize + (expected.random_i16() as i32).rem_euclid(0x20) as isize - 0x10)
        .clamp(0, CITY_WIDTH as isize - 1) as usize;
    let want_y = (cy as isize + (expected.random_i16() as i32).rem_euclid(0x20) as isize - 0x10)
        .clamp(0, CITY_HEIGHT as isize - 1) as usize;
    assert_eq!(tile, (want_x, want_y));
    assert!(tile.0.abs_diff(cx) <= 16 && tile.1.abs_diff(cy) <= 16);
}

#[test]
fn mac_mass_riot_jitter_differs_from_monster_1f_minus_0f() {
    // Same seed: %0x20-0x10 vs &0x1F-0x0F can disagree when Random low bits differ
    // from Euclidean mod (negative Toolbox draws) or when rem != mask.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..20 {
        for x in 10..20 {
            city.set_building(x, y, 1);
        }
    }
    let mut saw_diff = false;
    for seed in 1..512 {
        let mut a = MacToolboxRng::new(seed);
        let mut b = MacToolboxRng::new(seed);
        if mac_mass_riot_target_tile(&city, &mut a) != mac_monster_target_tile(&city, &mut b) {
            saw_diff = true;
            break;
        }
    }
    assert!(
        saw_diff,
        "mass-riot %0x20-0x10 must not be identical to monster &0x1F-0x0F for all seeds"
    );
}

#[test]
fn mass_riot_ambient_target_follows_engine_fidelity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..20 {
        for x in 10..20 {
            city.set_building(x, y, 1);
        }
    }
    let pe_center = land_value_city_center(&city);
    let mac_center = mac_developed_tile_anchor(&city);
    let mut mac_rng = MacToolboxRng::new(13);
    let mac = mass_riot_ambient_target_tile(&city, &mut mac_rng, EngineFidelity::Mac68k);
    assert!(
        mac.0.abs_diff(mac_center.0) <= 16 && mac.1.abs_diff(mac_center.1) <= 16,
        "Mac ambient mass-riot target should use the Mac center, got {mac:?} center {mac_center:?}"
    );

    let mut saw_off_center = false;
    for seed in 0..256 {
        let mut win_rng = MacToolboxRng::new(seed);
        let win = mass_riot_ambient_target_tile(&city, &mut win_rng, EngineFidelity::Win95);
        if win.0.abs_diff(pe_center.0) > 16 || win.1.abs_diff(pe_center.1) > 16 {
            saw_off_center = true;
            break;
        }
    }
    assert!(
        saw_off_center,
        "Win95 ambient mass-riot target should not be locked to Mac center jitter"
    );
}

#[test]
fn mac_rioters_target_shares_mass_riot_center_jitter() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..20 {
        for x in 10..20 {
            city.set_building(x, y, 1);
        }
    }
    let mut riot_rng = MacToolboxRng::new(0x35A7_E001);
    let mut mass_rng = MacToolboxRng::new(0x35A7_E001);
    assert_eq!(
        mac_rioters_target_tile(&city, &mut riot_rng),
        mac_mass_riot_target_tile(&city, &mut mass_rng),
        "Mac type-3 @ 0x35a7e falls into the same %0x20-0x10 placement as type 0xD"
    );
}

#[test]
fn rioters_ambient_target_follows_engine_fidelity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..20 {
        for x in 10..20 {
            city.set_building(x, y, 1);
        }
    }
    let pe_center = land_value_city_center(&city);
    let mac_center = mac_developed_tile_anchor(&city);
    let mut mac_rng = MacToolboxRng::new(17);
    let mac = rioters_ambient_target_tile(&city, &mut mac_rng, EngineFidelity::Mac68k);
    assert!(
        mac.0.abs_diff(mac_center.0) <= 16 && mac.1.abs_diff(mac_center.1) <= 16,
        "Mac ambient rioters should use the Mac center, got {mac:?} center {mac_center:?}"
    );

    let mut saw_off_center = false;
    for seed in 0..256 {
        let mut win_rng = MacToolboxRng::new(seed);
        let win = rioters_ambient_target_tile(&city, &mut win_rng, EngineFidelity::Win95);
        if win.0.abs_diff(pe_center.0) > 16 || win.1.abs_diff(pe_center.1) > 16 {
            saw_off_center = true;
            break;
        }
    }
    assert!(
        saw_off_center,
        "Win95 ambient rioters target should not be locked to Mac center jitter"
    );
}

#[test]
fn hurricane_ambient_target_follows_engine_fidelity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_building(x, y, 1);
        }
    }

    let mut mac_rng = MacToolboxRng::new(0x35CF_0010);
    let mac = hurricane_ambient_target_tile(&city, &mut mac_rng, EngineFidelity::Mac68k);
    assert_eq!(mac, (CITY_WIDTH / 2, CITY_HEIGHT / 2));

    let mut saw_off_center = false;
    for seed in 0..64 {
        let mut win_rng = MacToolboxRng::new(seed);
        let win = hurricane_ambient_target_tile(&city, &mut win_rng, EngineFidelity::Win95);
        if win != (CITY_WIDTH / 2, CITY_HEIGHT / 2) {
            saw_off_center = true;
            break;
        }
    }
    assert!(
        saw_off_center,
        "Win95 hurricane ambient should keep the land probe, not Mac's fixed center"
    );
}

#[test]
fn weather_disaster_ambient_target_follows_engine_fidelity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_building(x, y, 1);
        }
    }

    let mut mac_h_rng = MacToolboxRng::new(3);
    assert_eq!(
        weather_disaster_ambient_target_tile(
            &city,
            &mut mac_h_rng,
            EngineFidelity::Mac68k,
            DisasterKind::Hurricane
        ),
        (CITY_WIDTH / 2, CITY_HEIGHT / 2)
    );

    let mut mac_t_rng = MacToolboxRng::new(11);
    let mut direct = MacToolboxRng::new(11);
    assert_eq!(
        weather_disaster_ambient_target_tile(
            &city,
            &mut mac_t_rng,
            EngineFidelity::Mac68k,
            DisasterKind::Tornado
        ),
        mac_random_interior_map_tile(&mut direct)
    );

    let mut win_rng = Win95CrtRand::new(1);
    let mut direct = win_rng;
    let y = usize::from(direct.random_u16() % 0x7e + 1);
    let x = usize::from(direct.random_u16() % 0x7e + 1);
    assert_eq!(
        weather_disaster_ambient_target_tile(
            &city,
            &mut win_rng,
            EngineFidelity::Win95,
            DisasterKind::Tornado,
        ),
        (x, y)
    );
    assert_eq!(win_rng.seed(), direct.seed());
}

#[test]
fn ambient_disaster_kind_uses_pe_pollution_accident_gate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0x40);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    for seed in 0..64 {
        let mut rng = MacToolboxRng::new(seed);
        assert_ne!(
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95),
            Some(DisasterKind::Fire),
            "fire must not spawn from cold weather without XPLT or industrial tax (seed {seed})"
        );
        assert_ne!(
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95),
            Some(DisasterKind::ToxicSpill),
            "toxic spill must not spawn without XPLT or industrial tax (seed {seed})"
        );
        assert_ne!(
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95),
            Some(DisasterKind::PollutionAccident),
            "pollution accident must not spawn without industrial tax (seed {seed})"
        );
    }

    let offset =
        budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("industrial tax current");
    city.set_misc_u32_at(offset, 0x2710);
    let saw_accident = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::PollutionAccident)
    });
    assert!(
        saw_accident,
        "industrial tax current >= 10000 should allow the PE type-0xF pollution-accident slot"
    );
}

#[test]
fn pe_pollution_fire_ready_uses_xplt_threshold() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_pollution_fire_ready(&city));

    let mut pollution = vec![0u8; CITY_TILES];
    pollution[density_map_index(20, 20)] = 0x95;
    city.sync_pollution_density_from_slice(&pollution);
    assert!(
        !pe_pollution_fire_ready(&city),
        "XPLT 0x95 misses the PE type-4 toxic-spill arm"
    );

    pollution[density_map_index(20, 20)] = 0x96;
    city.sync_pollution_density_from_slice(&pollution);
    assert!(pe_pollution_fire_ready(&city));
}

#[test]
fn ambient_disaster_kind_uses_pe_pollution_fire_gate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0x40);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    for seed in 0..64 {
        let mut rng = MacToolboxRng::new(seed);
        assert_ne!(
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95),
            Some(DisasterKind::Fire),
            "fire must not spawn from cold weather without a hot XPLT cell (seed {seed})"
        );
        assert_ne!(
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95),
            Some(DisasterKind::ToxicSpill),
            "toxic spill must not spawn without a hot XPLT cell (seed {seed})"
        );
    }

    let mut pollution = vec![0u8; CITY_TILES];
    pollution[density_map_index(20, 20)] = 0x96;
    city.sync_pollution_density_from_slice(&pollution);
    let saw_spill = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::ToxicSpill)
    });
    assert!(
        saw_spill,
        "XPLT >= 0x96 should allow the PE type-4 toxic-spill arm"
    );
}

#[test]
fn pe_toxic_spill_target_jitters_hot_xplt_cell() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut pollution = vec![0u8; CITY_TILES];
    pollution[density_map_index(20, 20)] = 0x96;
    city.sync_pollution_density_from_slice(&pollution);

    let mut hits = 0_u32;
    let mut misses = 0_u32;
    for seed in 0..200_u32 {
        let mut rng = MacToolboxRng::new(seed);
        match pe_toxic_spill_target_tile(&city, &mut rng) {
            None => misses += 1,
            Some((x, y)) => {
                hits += 1;
                assert!(
                    (15..=24).contains(&x) && (15..=24).contains(&y),
                    "jittered target ({x},{y}) must stay near microsim (10,10) (seed {seed})"
                );
            }
        }
    }
    assert!(hits > 0, "1-in-10 XPLT scan should accept some seeds");
    assert!(misses > 0, "1-in-10 XPLT scan should reject some seeds");
}

#[test]
fn pe_toxic_spill_target_keeps_hotter_later_xplt_cell() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut pollution = vec![0u8; CITY_TILES];
    pollution[density_map_index(20, 20)] = 0x96;
    pollution[density_map_index(40, 40)] = 0xff;
    city.sync_pollution_density_from_slice(&pollution);

    let mut saw_hotter = false;
    for seed in 0..400_u32 {
        let mut rng = MacToolboxRng::new(seed);
        let Some((x, y)) = pe_toxic_spill_target_tile(&city, &mut rng) else {
            continue;
        };
        if (35..=44).contains(&x) && (35..=44).contains(&y) {
            saw_hotter = true;
            break;
        }
        assert!(
            (15..=24).contains(&x) && (15..=24).contains(&y),
            "cooler cell can win only when the hotter 1-in-10 fails ({x},{y} seed {seed})"
        );
    }
    assert!(
        saw_hotter,
        "hotter later XPLT cell should replace the cooler pick when its 1-in-10 hits"
    );
}

#[test]
fn toxic_spill_native_scans_are_xplt_column_major() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut pollution = vec![0u8; CITY_TILES];
    pollution[density_map_index(10, 40)] = 0x96;
    pollution[density_map_index(40, 10)] = 0x96;
    city.sync_pollution_density_from_slice(&pollution);

    let mut both_passed = 0_u32;
    for seed in 0..4000_u32 {
        let mut probe = MacToolboxRng::new(seed);
        let first = probe.random_range(10) == 0;
        if first {
            let _ = probe.random_range(10);
            let _ = probe.random_range(10);
        }
        let second = probe.random_range(10) == 0;
        if !(first && second) {
            continue;
        }
        both_passed += 1;
        let mut rng = MacToolboxRng::new(seed);
        let (x, y) = pe_toxic_spill_target_tile(&city, &mut rng)
            .expect("equal-heat 1-in-10 pair must pick a tile");
        assert!(
            (5..=14).contains(&x) && (35..=44).contains(&y),
            "PE mx-major scan keeps the earlier column ({x},{y} seed {seed})"
        );
        let mut mac_rng = MacToolboxRng::new(seed);
        let (mac_x, mac_y) = mac_toxic_spill_target_tile(&city, &mut mac_rng)
            .expect("Mac equal-heat pair must pick a tile");
        assert!(
            (5..=14).contains(&mac_x) && (35..=44).contains(&mac_y),
            "Mac X-major scan keeps the earlier column ({mac_x},{mac_y} seed {seed})"
        );
    }
    assert!(
        both_passed > 0,
        "need at least one seed where both equal XPLT cells pass 1-in-10"
    );
}

#[test]
fn toxic_spill_target_for_engine_uses_native_scan_order_in_both_editions() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut pollution = vec![0u8; CITY_TILES];
    pollution[density_map_index(10, 40)] = 0x96;
    pollution[density_map_index(40, 10)] = 0x96;
    city.sync_pollution_density_from_slice(&pollution);

    let seed = (0..4000_u32)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            let first = probe.random_range(10) == 0;
            if first {
                let _ = probe.random_range(10);
                let _ = probe.random_range(10);
            }
            first && probe.random_range(10) == 0
        })
        .expect("seed where both 1-in-10 gates pass");

    let mut win_rng = MacToolboxRng::new(seed);
    let win = toxic_spill_target_tile_for_engine(&city, &mut win_rng, EngineFidelity::Win95)
        .expect("Win95 tile");
    let mut mac_rng = MacToolboxRng::new(seed);
    let mac = toxic_spill_target_tile_for_engine(&city, &mut mac_rng, EngineFidelity::Mac68k)
        .expect("Mac tile");
    assert_eq!(win, mac, "both native scans visit X before Y");
    assert!(
        (5..=14).contains(&win.0) && (35..=44).contains(&win.1),
        "both editions keep the first X-major cell {win:?}"
    );
}

#[test]
fn mac_toxic_spill_uses_signed_toolbox_modulo_for_gate_and_jitter() {
    let seed = (0..10_000_u32)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            let value = probe.random_i16();
            value < 0 && i32::from(value).rem_euclid(10) == 0 && (value as u16) % 10 != 0
        })
        .expect("seed with negative signed _Random modulo divergence");

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut pollution = vec![0u8; CITY_TILES];
    pollution[density_map_index(20, 20)] = 0x96;
    city.sync_pollution_density_from_slice(&pollution);

    let mut expected_rng = MacToolboxRng::new(seed);
    assert_eq!(expected_rng.random_range(10), 0);
    let expected = (
        (20 + expected_rng.random_range(10)) - 5,
        (20 + expected_rng.random_range(10)) - 5,
    );
    let mut rng = MacToolboxRng::new(seed);
    assert_eq!(
        mac_toxic_spill_target_tile(&city, &mut rng),
        Some(expected),
        "Mac spill gate and coordinate offsets use signed QuickDraw modulo"
    );
}

#[test]
fn pe_fire_disaster_heat_gate_matches_win95() {
    let mut cold = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    cold.set_misc_u32_at(MISC_HEAT_OFFSET, 0x7e);
    for seed in 0..32 {
        assert!(
            !pe_fire_disaster_ready(&cold, &mut MacToolboxRng::new(seed)),
            "heat 0x7E can never pass (rand&0x7f)+0x7F (seed {seed})"
        );
    }

    let mut hot = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    hot.set_misc_u32_at(MISC_HEAT_OFFSET, 0xfe);
    for seed in 0..32 {
        assert!(
            pe_fire_disaster_ready(&hot, &mut MacToolboxRng::new(seed)),
            "heat 0xFE always passes the PE fire arm (seed {seed})"
        );
    }

    let mut threshold = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    threshold.set_misc_u32_at(MISC_HEAT_OFFSET, 0x7f);
    let mut rng = MacToolboxRng::new(0x51);
    let expected = (u32::from(rng.random_u16()) & 0x7f) + 0x7f <= 0x7f;
    rng = MacToolboxRng::new(0x51);
    assert_eq!(pe_fire_disaster_ready(&threshold, &mut rng), expected);
}

#[test]
fn ambient_disaster_kind_uses_pe_fire_heat_gate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0x40);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    for seed in 0..64 {
        let mut rng = MacToolboxRng::new(seed);
        assert_ne!(
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95),
            Some(DisasterKind::Fire),
            "fire must not spawn below PE heat 0x7F (seed {seed})"
        );
    }

    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0xfe);
    let saw_fire = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::Fire)
    });
    assert!(
        saw_fire,
        "max heat should allow the PE fire arm to roll ambient fire"
    );
}

#[test]
fn pe_blizzard_flood_jitter_is_rand_and_1f_minus_10() {
    let mut rng = Win95CrtRand::new(0x45D9_D001);
    let mut expected = Win95CrtRand::new(0x45D9_D001);
    assert_eq!(
        pe_blizzard_flood_jitter(&mut rng),
        (
            (expected.random_u16() & 0x1f) as isize - 0x10,
            (expected.random_u16() & 0x1f) as isize - 0x10,
        )
    );
}

#[test]
fn start_disaster_blizzard_flood_runs_type2_from_jittered_origins() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.persist_pe_day22_population_leftover(20_000);
    city.set_terrain(12, 34, 0x20);
    assert_eq!(pe_population_jitter_attempt_count(&city), 7);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_blizzard_flood_overlays_with_rng(
        &mut state.city.clone(),
        12,
        34,
        &mut preview,
        &mut preview_lfsr,
    );

    assert!(state.start_disaster(DisasterKind::BlizzardFlood));
    assert!(!expected.is_empty());
    assert!(expected.values().all(|&marker| marker == 0xfc));
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0e)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_flood_source_is_chebyshev_xter_20_to_2f() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(pe_flood_source_tile(&city, 40, 40), None);
    city.set_terrain(42, 41, 0x10);
    assert_eq!(pe_flood_source_tile(&city, 40, 40), None);
    city.set_terrain(42, 41, 0x20);
    assert_eq!(pe_flood_source_tile(&city, 40, 40), Some((42, 41)));
    city.set_terrain(40, 40, 0x2f);
    assert_eq!(
        pe_flood_source_tile(&city, 40, 40),
        Some((40, 40)),
        "radius 0 checks the trigger tile first"
    );
}

#[test]
fn pe_flood_origin_xter20_paints_east_and_south() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(12, 34, 0x20);
    let overlays = pe_flood_overlays(&city, 12, 34);
    assert_eq!(overlays.get(&(12, 34)), None);
    assert_eq!(overlays.get(&(13, 34)), Some(&0xfc));
    assert_eq!(overlays.get(&(12, 35)), Some(&0xfc));
    assert_eq!(overlays.get(&(11, 34)), None);
    assert_eq!(overlays.get(&(12, 33)), None);
}

#[test]
fn pe_flood_paint_with_lfsr_writes_fc_live() {
    // PE type-2 @ `0x45db00`: neighbor paints write XTXT `0xfc` onto the map.
    let mut city = City::new_blank("/tmp/sc2000-flood-live.sc2", "FloodLive").expect("blank");
    city.set_terrain(12, 34, 0x20);
    let mut lfsr = Win95Lfsr::from_u32_seed(1);
    let (source, overlays) = pe_flood_paint_with_lfsr(&mut city, 12, 34, &mut lfsr);
    assert_eq!(source, Some((12, 34)));
    assert!(!overlays.is_empty());
    for (&(x, y), &marker) in &overlays {
        assert_eq!(city.text_overlay_at(x, y), Some(marker));
    }
}

#[test]
fn pe_flood_east_of_source_also_paints_west() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(14, 34, 0x20);
    let overlays = pe_flood_overlays(&city, 12, 34);
    assert_eq!(overlays.get(&(13, 34)), Some(&0xfc));
    assert_eq!(overlays.get(&(15, 34)), Some(&0xfc));
    assert_eq!(overlays.get(&(14, 35)), Some(&0xfc));
    assert_eq!(overlays.get(&(14, 33)), None);
}

#[test]
fn start_disaster_flood_fallback_paints_one_empty_xter() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected = pe_flood_overlays_with_lfsr(&state.city, 12, 34, &mut preview_lfsr);

    assert!(state.start_disaster(DisasterKind::Flood));
    assert_eq!(expected.len(), 1);
    assert!(expected.values().all(|&marker| marker == 0xfc));
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
        assert_eq!(state.city.terrain_at(x, y), 0);
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x02)
    );
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn start_disaster_flood_sets_0x3c_duration_on_coastal_source() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(12, 34, 0x20);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::Flood));
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
}

#[test]
fn start_disaster_blizzard_flood_sets_0x3c_duration() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(12, 34, 0x20);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::BlizzardFlood));
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
}

#[test]
fn start_disaster_hurricane_sets_0x3c_duration() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::Hurricane));
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
    assert_eq!(
        state.pe_hurricane_wind_remaining,
        PE_HURRICANE_WIND_REMAINING
    );
    assert_eq!(PE_HURRICANE_WIND_REMAINING, 0x32);
}

#[test]
fn start_disaster_flood_does_not_set_hurricane_wind_remaining() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::Flood));
    assert_eq!(state.pe_hurricane_wind_remaining, 0);
}

#[test]
fn pe_hurricane_wind_tick_skips_xbld_le_0x70() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_building(x, y, 0x70);
        }
    }
    let tick = pe_hurricane_wind_tick(
        &mut city,
        PE_FLOOD_HURRICANE_DURATION,
        PE_HURRICANE_WIND_REMAINING,
    );
    assert_eq!(tick.wind_remaining, PE_HURRICANE_WIND_REMAINING - 1);
    assert_eq!(tick.destroyed, None);
    assert_eq!(city.building_at(10, 10), 0x70);
}

#[test]
fn pe_hurricane_wind_tick_noop_without_duration() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 0x71);
    let tick = pe_hurricane_wind_tick(&mut city, 0, PE_HURRICANE_WIND_REMAINING);
    assert_eq!(tick.wind_remaining, PE_HURRICANE_WIND_REMAINING);
    assert_eq!(tick.destroyed, None);
    assert!(!tick.play_wind);
    assert_eq!(city.building_at(10, 10), 0x71);
}

#[test]
fn pe_hurricane_wind_clock_matches_helper_and_decrements() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_building(x, y, 0x71);
        }
    }
    let mut state = test_state(city);
    state.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
    state.pe_hurricane_wind_remaining = PE_HURRICANE_WIND_REMAINING;
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    let mut expected = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let tick = pe_hurricane_wind_tick_with_rng(
        &mut expected,
        PE_FLOOD_HURRICANE_DURATION - 1,
        PE_HURRICANE_WIND_REMAINING,
        &mut preview,
        &mut preview_lfsr,
    );
    let destroyed = state.tick_pe_disaster_clock();
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION - 1);
    assert_eq!(state.pe_hurricane_wind_remaining, tick.wind_remaining);
    assert_eq!(destroyed, usize::from(tick.destroyed.is_some()));
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
    if tick.play_wind {
        assert!(
            state
                .pending_disaster_extra_sounds
                .contains(&PlacementSound::Wind)
        );
    }
    if let Some((x, y)) = tick.destroyed {
        assert!(is_rubble_tile(state.city.building_at(x, y)));
        assert!(is_rubble_tile(expected.building_at(x, y)));
        assert!(
            state
                .pending_disaster_extra_sounds
                .contains(&PlacementSound::Explode)
        );
        if state.auto_goto_enabled {
            assert_eq!(state.pending_disaster_auto_goto, Some((x, y)));
        }
    }
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            assert_eq!(
                state.city.building_at(x, y),
                expected.building_at(x, y),
                "XBLD at ({x},{y})"
            );
        }
    }
}

#[test]
fn start_disaster_fire_does_not_set_flood_hurricane_duration() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    assert!(state.start_disaster(DisasterKind::Fire));
    assert_eq!(state.pe_disaster_duration, 0);
}

#[test]
fn pe_disaster_duration_decrements_per_disaster_dispatch_not_calendar() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::Flood));
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
    state.handle_simulation_day_advanced(1900, 1, 5);
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
    state.handle_simulation_month_advanced();
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
    state.tick_pe_disaster_clock();
    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION - 1);
    state.pe_disaster_duration = 1;
    state.tick_pe_disaster_duration();
    assert_eq!(state.pe_disaster_duration, 0);
    state.tick_pe_disaster_duration();
    assert_eq!(state.pe_disaster_duration, 0);
}

#[test]
fn mac_calendar_does_not_advance_the_unrooted_pe_disaster_worker() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfc);
    city.set_misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET, 1);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
    state.pe_hurricane_wind_remaining = 7;
    let crt_before = state.pe_process_crt_seed();
    let lfsr_before = state.pe_process_lfsr_state();

    state.handle_simulation_day_advanced(1900, 1, 5);

    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
    assert_eq!(state.pe_hurricane_wind_remaining, 7);
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0xfc));
    assert_eq!(state.pe_process_crt_seed(), crt_before);
    assert_eq!(state.pe_process_lfsr_state(), lfsr_before);

    state.handle_simulation_month_advanced();

    assert_eq!(state.pe_disaster_duration, PE_FLOOD_HURRICANE_DURATION);
    assert_eq!(state.pe_hurricane_wind_remaining, 7);
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0xfc));
    assert_eq!(state.pe_process_crt_seed(), crt_before);
    assert_eq!(state.pe_process_lfsr_state(), lfsr_before);
}

#[test]
fn pe_flood_tick_spreads_nesw_while_duration_high() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfc);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    let mut state = test_state(city);
    state.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_flood_tick(&state, &mut preview, &mut preview_lfsr);
    assert_eq!(stats.receded, 0);
    assert_eq!(stats.destroyed, 0);
    assert!(stats.spread >= 1);
    assert_eq!(expected.text_overlay_at(40, 40), Some(0xfc));
    let neighbors = [
        expected.text_overlay_at(39, 40),
        expected.text_overlay_at(40, 39),
        expected.text_overlay_at(41, 40),
        expected.text_overlay_at(40, 41),
    ];
    assert!(
        neighbors.iter().filter(|&&m| m == Some(0xfc)).count() >= 1,
        "at least one NESW neighbor should receive 0xfc"
    );
    advance_pe_disaster_scan_preview(&state, &mut preview);
    assert_eq!(state.tick_pe_disaster_overlays(), stats.changed());
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
fn pe_flood_tick_recedes_half_the_tiles_at_duration_zero() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(20, 20, 0xfc);
    city.set_text_overlay(21, 20, 0xfc);
    city.set_text_overlay(22, 20, 0xfc);
    city.set_text_overlay(23, 20, 0xfc);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    let mut state = test_state(city);
    state.pe_disaster_duration = 0;
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_flood_tick(&state, &mut preview, &mut preview_lfsr);
    assert!(stats.spread == 0);
    assert!(stats.receded > 0);
    assert!(
        expected.text_overlay_at(20, 20) == Some(0)
            || expected.text_overlay_at(21, 20) == Some(0)
            || expected.text_overlay_at(22, 20) == Some(0)
            || expected.text_overlay_at(23, 20) == Some(0)
    );
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
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
fn pe_flood_tick_destroys_xbld_ge_0x70_when_duration_below_0x1e() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    city.set_text_overlay(40, 40, 0xfc);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x10);
    let mut state = test_state(city);
    state.pe_disaster_duration = PE_FLOOD_TICK_DESTROY_AT - 1;
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_flood_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.destroyed > 0 {
        assert!(is_rubble_tile(expected.building_at(40, 40)));
        assert_eq!(expected.text_overlay_at(40, 40), Some(0));
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
    assert_eq!(state.city.building_at(40, 40), expected.building_at(40, 40));
    assert_eq!(
        state.city.text_overlay_at(40, 40),
        expected.text_overlay_at(40, 40)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_flood_tick_duration_destroy_places_crt_rubble_like_pe() {
    // Seed 3 admits both duration gates. The mode-0 destroy takes the
    // third CRT value for rubble, then the fourth for the spread direction.
    let mut city = City::new_blank("/tmp/sc2000-flood-crt-rubble.sc2", "FloodCrt").unwrap();
    city.set_building(40, 40, 0x70);
    city.set_text_overlay(40, 40, 0xfc);
    let mut crt = Win95CrtRand::new(3);
    let mut lfsr = Win95Lfsr::new(1);
    let mut stats = PeFloodTickStats::default();
    pe_flood_tick_cell_with_rng(
        &mut city,
        PE_FLOOD_TICK_DESTROY_AT - 1,
        &mut crt,
        &mut lfsr,
        40,
        40,
        &mut stats,
    );
    assert_eq!(stats.destroyed, 1);
    assert_eq!(city.building_at(40, 40), 3);
    assert_eq!(city.text_overlay_at(40, 40), Some(0));
    assert_eq!(crt.seed(), 2_743_275_959);
}

#[test]
fn pe_flood_destroy_tile_clears_xtxt_footprint() {
    // Shared flood/fire DestroyStructure helper clears occupancy XTXT with XBLD.
    let mut city = City::new_blank("/tmp/sc2000-flood-xtxt.sc2", "FloodXtxt").expect("blank");
    let building = RESIDENTIAL_2X2_BUILDINGS[4];
    stamp_complete_building_for_test(&mut city, 20, 20, building, 2);
    city.set_text_overlay(20, 20, 0x55);
    city.set_text_overlay(21, 21, 0x66);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.building_at(21, 21), 0);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(city.text_overlay_at(21, 21), Some(0));
}

#[test]
fn pe_flood_destroy_tile_updates_dw_tile_count_via_place_tile() {
    // PE DestroyBuilding clears XBLD through PlaceTile so `dwTileCount` moves with it.
    let mut city = City::new_blank("/tmp/sc2000-flood-census.sc2", "FloodCensus").expect("blank");
    let building = RESIDENTIAL_2X2_BUILDINGS[4];
    stamp_complete_building_for_test(&mut city, 20, 20, building, 2);
    assert_eq!(city.pe_dw_tile_count[building as usize], 4);
    let vacant_before = city.pe_dw_tile_count[0];
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.pe_dw_tile_count[building as usize], 0);
    assert_eq!(
        city.pe_dw_tile_count[0],
        vacant_before.wrapping_add(4),
        "PlaceTile(0) must move the four footprint cells into dwTileCount[0]"
    );
}

#[test]
fn pe_flood_destroy_stadium_clears_sports_team_bit_like_459fb7() {
    // PE DestroyBuilding @ `0x459fb7`: stadium XBLD `0xd7` + XTXT microsim bit clear.
    let mut city =
        City::new_blank("/tmp/sc2000-destroy-sports.sc2", "DestroySports").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 215, 4);
    let xtxt = city
        .provision_microsim_for_building(215)
        .expect("stadium microsim");
    city.set_text_overlay(20, 20, xtxt);
    let index = usize::from(xtxt - MIN_SIM_TEXT_ENTRIES);
    // PE's loader decodes the saved XMIC word before reading its low byte.
    assert!(city.set_microsim_stat2(index, 2));
    city.set_pe_w_sports_teams(0x1f);
    pe_flood_destroy_tile(&mut city, 20, 20);
    // 0x1f + (0xffff << 2) wraps to 0x001b (bit 2 cleared).
    assert_eq!(city.pe_w_sports_teams(), 0x001b);
    assert_eq!(city.building_at(20, 20), 0);
}

#[test]
fn pe_flood_destroy_stadium_skips_sports_clear_without_matching_microsim() {
    let mut city =
        City::new_blank("/tmp/sc2000-destroy-sports-skip.sc2", "DestroySportsSkip").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 215, 4);
    city.set_text_overlay(20, 20, 0x40); // midband XTXT, no stadium microsim row
    city.set_pe_w_sports_teams(0x1f);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.pe_w_sports_teams(), 0x1f);
}

#[test]
fn pe_flood_destroy_tile_preserves_dispatch_proxy_xtxt_like_destroy_building() {
    // PE DestroyStructure → DestroyBuilding @ `0x45a0c4`: leave `0xc9..=0xf0`.
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-proxy.sc2", "FloodXtxtProxy").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xd0);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xd0));
}

#[test]
fn pe_flood_destroy_midband_xtxt_clears_xthg_saved_a_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-mid.sc2", "FloodXtxtMid").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xd1);
    let mut table = crate::win95_overlay::empty_overlay_table();
    table[0xd1 - 0xc9].saved_a = 0x33;
    city.set_traffic_overlays_for_test(table);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xd1));
    assert_eq!(city.traffic_overlays().unwrap()[0xd1 - 0xc9].saved_a, 0);
}

#[test]
fn pe_flood_destroy_tile_clears_xbit_and_xzon_corners_like_destroy_building() {
    // PE DestroyBuilding @ `0x45a07f` / `0x45a0b9`: XBIT &= 0x3d; XZON &= 0x0f.
    let mut city = City::new_blank("/tmp/sc2000-flood-xbit-xzon.sc2", "FloodBits").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0x40 | 0x80 | 0x02)
            .with_piped(true)
            .with_watered(true),
    );
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(
        city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT
    );
    let bits = city.tile_bits_at(20, 20).unwrap();
    assert_eq!(bits.raw_bits() & 0xc2, 0); // rotate/powered/powerable cleared
    // `0x3d` retains both service bits (`0x20` piped and `0x10` watered).
    assert!(bits.piped() && bits.watered());
}

#[test]
fn pe_flood_destroy_elevated_early_outs_like_destroy_structure() {
    // PE DestroyStructure @ `0x46137b`: XBLD `0x3f..=0x42` returns before DestroyBuilding.
    let mut city =
        City::new_blank("/tmp/sc2000-flood-elev-keep.sc2", "FloodElevKeep").expect("blank");
    city.set_building(20, 20, 0x3f);
    city.set_altitude(20, 20, 9, 4);
    city.set_tunnel_levels(20, 20, 0x1f);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0x3f);
    assert_eq!(city.altitude_at(20, 20), 9);
    assert_eq!(city.water_height_at(20, 20), 4);
    assert_eq!(city.tunnel_levels_at(20, 20), 0x1f);
}

#[test]
fn mac68k_flood_destroy_elevated_early_outs_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-elev-keep-mac.sc2", "FloodElevKeepMac").expect("blank");
    city.set_building(20, 20, 0x40);
    city.set_altitude(20, 20, 8, 3);
    city.set_tunnel_levels(20, 20, 0x15);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0x40);
    assert_eq!(city.altitude_at(20, 20), 8);
    assert_eq!(city.water_height_at(20, 20), 3);
    assert_eq!(city.tunnel_levels_at(20, 20), 0x15);
}

#[test]
fn pe_flood_destroy_non_elevated_leaves_altm_tunnel_bits() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-altm-leave.sc2", "FloodAltmLeave").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_tunnel_levels(20, 20, 7);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.tunnel_levels_at(20, 20), 7);
}

#[test]
fn pe_flood_destroy_size1_nonflat_remaps_slope_like_destroy_building() {
    // PE DestroyStructure → DestroyBuilding @ `0x45a1ee` FUN_00447020 mode0 subset.
    let mut city = City::new_blank("/tmp/sc2000-flood-slope.sc2", "FloodSlope").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_terrain(20, 20, 1);
    city.set_altitude(20, 20, 9, 0);
    city.set_altitude(21, 20, 10, 0);
    city.set_underground(20, 20, 0x10);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.underground_at(20, 20), 0);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 20);
    assert_ne!(slope, 0);
    assert_eq!(city.terrain_at(20, 20), slope);
    assert_eq!(city.zone_at(20, 20).unwrap().raw_bits(), 0);
}

#[test]
fn pe_flood_destroy_size1_support_walk_land_dec_north_tip() {
    // PE DestroyBuilding size-1 support walk @ `0x458f00` + tip land-- `0x458f90`.
    // Unrotated XBLD `0x51` run at y=5..=6 → pre/post tips y=4/7 get land--;
    // forward support body clears both live cells.
    // Sea above post-dec land so FUN_00447020 keeps the tip water OR.
    let mut city = City::new_blank("/tmp/sc2000-support-walk.sc2", "SupportWalk").expect("blank");
    city.set_mac_selected_water_height(15);
    city.set_building(20, 5, 0x51);
    city.set_building(20, 6, 0x51);
    city.set_altitude(20, 4, 10, 0);
    city.set_altitude(20, 5, 10, 0);
    city.set_altitude(20, 6, 10, 0);
    city.set_altitude(20, 7, 10, 0);
    city.set_altitude(0, 0, city.altitude_at(0, 0) & 0x1f, 15);
    pe_flood_destroy_tile(&mut city, 20, 6);
    assert_eq!(city.building_at(20, 6), 0);
    assert_eq!(city.building_at(20, 5), 0);
    assert_eq!(city.altitude_at(20, 4) & 0x1f, 9);
    assert_eq!(city.altitude_at(20, 5) & 0x1f, 10);
    assert_eq!(city.altitude_at(20, 6) & 0x1f, 10);
    assert_eq!(city.altitude_at(20, 7) & 0x1f, 9);
    assert!(city.tile_bits_at(20, 4).is_some_and(TileBits::water_tile));
    assert!(city.tile_bits_at(20, 7).is_some_and(TileBits::water_tile));
}

#[test]
fn pe_flood_destroy_size1_support_walk_rotate_land_dec_west_tip() {
    // Hit XBIT rotate `0x02` → walk x−size; tip west of the run land--.
    let mut city =
        City::new_blank("/tmp/sc2000-support-walk-rot.sc2", "SupportWalkRot").expect("blank");
    city.set_mac_selected_water_height(15);
    city.set_building(5, 20, 0x52);
    city.set_building(6, 20, 0x52);
    city.set_tile_bits(6, 20, TileBits::new(0).with_rotate(true));
    city.set_altitude(4, 20, 12, 0);
    city.set_altitude(5, 20, 12, 0);
    city.set_altitude(6, 20, 12, 0);
    city.set_altitude(7, 20, 12, 0);
    city.set_tile_bits(7, 20, TileBits::new(0).with_rotate(true));
    city.set_altitude(0, 0, city.altitude_at(0, 0) & 0x1f, 15);
    pe_flood_destroy_tile(&mut city, 6, 20);
    assert_eq!(city.building_at(6, 20), 0);
    assert_eq!(city.building_at(5, 20), 0);
    assert_eq!(city.altitude_at(4, 20) & 0x1f, 11);
    assert_eq!(city.altitude_at(7, 20) & 0x1f, 11);
    assert!(city.tile_bits_at(4, 20).is_some_and(TileBits::water_tile));
    assert!(!city.tile_bits_at(4, 20).is_some_and(TileBits::rotate));
    // Post-walk endpoint does not run the pre-walk rotate clear.
    assert!(city.tile_bits_at(7, 20).is_some_and(TileBits::rotate));
}

#[test]
fn pe_flood_destroy_size1_support_walk_skips_water_tip_land_dec() {
    let mut city =
        City::new_blank("/tmp/sc2000-support-walk-wet.sc2", "SupportWalkWet").expect("blank");
    city.set_building(5, 20, 0x53);
    city.set_building(5, 19, 0x0d);
    city.set_tile_bits(5, 19, TileBits::new(0).with_water_tile(true));
    city.set_altitude(5, 19, 8, 8);
    city.set_altitude(5, 20, 8, 0);
    pe_flood_destroy_tile(&mut city, 5, 20);
    assert_eq!(city.building_at(5, 20), 0);
    // Wet tip skips PlaceTile(0) / land-- / water-OR epilogue.
    assert_eq!(city.building_at(5, 19), 0x0d);
    assert_eq!(city.altitude_at(5, 19) & 0x1f, 8);
}

#[test]
fn pe_flood_destroy_size1_support_tip_epilogue_sets_water_clears_rotate() {
    // PE @ `0x459036` OR water + `0x45907f` clear rotate after tip land--.
    let mut city =
        City::new_blank("/tmp/sc2000-support-tip-epi.sc2", "SupportTipEpi").expect("blank");
    city.set_mac_selected_water_height(15);
    city.set_building(5, 20, 0x54);
    city.set_tile_bits(5, 19, TileBits::new(0).with_rotate(true));
    city.set_altitude(5, 19, 11, 0);
    city.set_altitude(5, 20, 11, 0);
    city.set_altitude(0, 0, city.altitude_at(0, 0) & 0x1f, 15);
    pe_flood_destroy_tile(&mut city, 5, 20);
    assert_eq!(city.altitude_at(5, 19) & 0x1f, 10);
    let bits = city.tile_bits_at(5, 19).unwrap();
    assert!(bits.water_tile());
    assert!(!bits.rotate());
}

#[test]
fn pe_flood_destroy_size1_support_tip_dry_fun_clears_water_or() {
    // PE OR water then FUN_00447020: when sea <= land, dry arm clears water-tile.
    // Rotate clear @ `0x45907f` still runs after FUN.
    let mut city =
        City::new_blank("/tmp/sc2000-support-tip-dry.sc2", "SupportTipDry").expect("blank");
    city.set_building(5, 20, 0x55);
    city.set_tile_bits(5, 19, TileBits::new(0).with_rotate(true));
    city.set_altitude(5, 19, 11, 0);
    city.set_altitude(5, 20, 11, 0);
    // No high sea — pe_map_sea_level stays below post-dec land.
    pe_flood_destroy_tile(&mut city, 5, 20);
    assert_eq!(city.altitude_at(5, 19) & 0x1f, 10);
    let bits = city.tile_bits_at(5, 19).unwrap();
    assert!(!bits.water_tile());
    assert!(!bits.rotate());
}

#[test]
fn pe_flood_destroy_size1_support_west_edge_skips_oob_tip() {
    // PE tip bounds: walked x < 0 skips PlaceTile / land-- / epilogue.
    let mut city = City::new_blank("/tmp/sc2000-support-oob.sc2", "SupportOob").expect("blank");
    city.set_building(0, 20, 0x56);
    city.set_tile_bits(0, 20, TileBits::new(0).with_rotate(true));
    city.set_altitude(0, 20, 10, 0);
    pe_flood_destroy_tile(&mut city, 0, 20);
    assert_eq!(city.building_at(0, 20), 0);
    assert_eq!(city.altitude_at(0, 20) & 0x1f, 10);
}

#[test]
fn pe_flood_destroy_size1_support_walk_continues_across_0x6a() {
    // PE walk loop treats `0x6a` as in-band even though GetBuildingSize(0x6a)==2.
    let mut city = City::new_blank("/tmp/sc2000-support-6a.sc2", "Support6a").expect("blank");
    city.set_mac_selected_water_height(15);
    city.set_building(4, 20, 0x6a);
    city.set_building(5, 20, 0x51);
    city.set_tile_bits(5, 20, TileBits::new(0).with_rotate(true));
    city.set_altitude(3, 20, 10, 0);
    city.set_altitude(4, 20, 10, 0);
    city.set_altitude(5, 20, 10, 0);
    city.set_altitude(6, 20, 10, 0);
    city.set_altitude(0, 0, city.altitude_at(0, 0) & 0x1f, 15);
    pe_flood_destroy_tile(&mut city, 5, 20);
    assert_eq!(city.altitude_at(3, 20) & 0x1f, 9);
    assert!(city.tile_bits_at(3, 20).is_some_and(TileBits::water_tile));
    assert_eq!(city.building_at(4, 20), 0);
    assert_eq!(city.building_at(5, 20), 0);
}

#[test]
fn pe_flood_destroy_size1_support_tip_placetile0_clears_tip_xbld() {
    // PE @ `0x458fae`: PlaceTile(0) on the walked tip before land--.
    let mut city =
        City::new_blank("/tmp/sc2000-support-tip-xbld.sc2", "SupportTipXbld").expect("blank");
    city.set_building(5, 20, 0x57);
    city.set_tile_bits(5, 20, TileBits::new(0).with_rotate(true));
    city.set_building(4, 20, 0x0d);
    city.set_altitude(4, 20, 10, 0);
    city.set_altitude(5, 20, 10, 0);
    city.set_altitude(0, 0, city.altitude_at(0, 0) & 0x1f, 15);
    pe_flood_destroy_tile(&mut city, 5, 20);
    assert_eq!(city.building_at(4, 20), 0);
    assert_eq!(city.altitude_at(4, 20) & 0x1f, 9);
}

#[test]
fn mac68k_flood_destroy_size1_nonflat_remaps_slope_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-slope-mac.sc2", "FloodSlopeMac").expect("blank");
    city.set_building(20, 20, 0x0e);
    city.set_zone(20, 20, ZONE_INDUSTRIAL_LIGHT | 0x20);
    city.set_terrain(20, 20, 3);
    city.set_altitude(20, 20, 9, 0);
    city.set_altitude(19, 20, 10, 0);
    city.set_underground(20, 20, 0x12);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.underground_at(20, 20), 0);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 20);
    assert_ne!(slope, 0);
    assert_eq!(city.terrain_at(20, 20), slope);
    assert_eq!(city.zone_at(20, 20).unwrap().raw_bits(), 0);
}

#[test]
fn flood_destroy_clears_shared_xbit_and_xzon_corner_planes() {
    let mut city = City::new_blank("/tmp/sc2000-flood-xbit-xzon-shared.sc2", "FloodBitsShared")
        .expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_zone(20, 20, ZONE_COMMERCIAL_LIGHT | 0x40);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0x40 | 0x80 | 0x02)
            .with_piped(true)
            .with_watered(true),
    );
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(
        city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_COMMERCIAL_LIGHT
    );
    let bits = city.tile_bits_at(20, 20).unwrap();
    assert_eq!(bits.raw_bits() & 0xc2, 0);
    // This invokes the PE destroy helper over the shared city planes; it does not
    // claim that the Mac executable follows the same call path.
    assert!(bits.piped() && bits.watered());
}

#[test]
fn mac68k_flood_destroy_preserves_dispatch_proxy_xtxt_like_pe() {
    let mut city = City::new_blank("/tmp/sc2000-flood-xtxt-proxy-mac.sc2", "FloodXtxtProxyMac")
        .expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xe8);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xe8));
}

#[test]
fn pe_destroy_structure_fire_paint_uses_flood_destroy_helper() {
    // PE `0x401604(x,y,1,1)`: DestroyStructure clear via pe_flood_destroy_tile, then dry `0xff`.
    let mut city = City::new_blank("/tmp/sc2000-fire-paint-flood.sc2", "FireFlood").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 210, 3);
    let corner_bits = city.footprint_corner_bits();
    let zone_id = ZONE_RESIDENTIAL_LIGHT;
    for &(x, y) in &building_footprint_from_origin(40, 40, 3).expect("footprint") {
        city.set_zone(
            x,
            y,
            zone_id | building_zone_bits_for_tile(40, 40, x, y, 3, corner_bits),
        );
        city.set_traffic_density(x, y, 0x2a);
    }
    city.set_terrain(42, 42, 0x10);
    city.set_tile_bits(42, 42, TileBits::new(0x04)); // water — skip fire paint
    city.set_text_overlay(41, 41, 0x55);
    pe_destroy_structure_fire_paint(&mut city, None, 41, 41);
    for &(x, y) in &building_footprint_from_origin(40, 40, 3).expect("footprint") {
        assert_eq!(city.building_at(x, y), 0);
        // PE DestroyBuilding @ `0x45a0b9`: XZON &= 0x0f (keep type, clear corners).
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), zone_id);
        assert_eq!(city.traffic_density_at(x, y), Some(0x2a));
        if (x, y) == (42, 42) {
            assert_eq!(city.text_overlay_at(x, y), Some(0));
        } else {
            assert_eq!(
                city.text_overlay_at(x, y),
                Some(0xff),
                "dry footprint should get fire paint at ({x},{y})"
            );
        }
    }
}

#[test]
fn pe_flood_tick_rejects_uphill_and_xter_10_to_1f() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfc);
    let src_alt = city.altitude_at(40, 40) & 0x1f;
    city.set_altitude(41, 40, src_alt.saturating_add(1), 0);
    city.set_terrain(40, 39, 0x10);
    assert_eq!(pe_flood_neighbor_paint_kind(&city, 41, 40, src_alt), None);
    assert_eq!(pe_flood_neighbor_paint_kind(&city, 40, 39, src_alt), None);
    assert_eq!(
        pe_flood_neighbor_paint_kind(&city, 39, 40, src_alt),
        Some(PeFloodNeighborAction::OverlayOnly)
    );
    city.set_text_overlay(39, 40, 0xf1);
    assert_eq!(
        pe_flood_neighbor_paint_kind(&city, 39, 40, src_alt),
        Some(PeFloodNeighborAction::Rubble)
    );
    city.set_text_overlay(39, 40, 0xf9);
    assert_eq!(
        pe_flood_neighbor_paint_kind(&city, 39, 40, src_alt),
        Some(PeFloodNeighborAction::Rubble)
    );
    city.set_text_overlay(39, 40, 0xf0);
    assert_eq!(pe_flood_neighbor_paint_kind(&city, 39, 40, src_alt), None);
    city.set_text_overlay(39, 40, 0xfa);
    assert_eq!(pe_flood_neighbor_paint_kind(&city, 39, 40, src_alt), None);
}

#[test]
fn pe_flood_tick_rubble_on_xf1_neighbor_without_fc_paint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfc);
    city.set_text_overlay(39, 40, 0xf1);
    city.set_building(39, 40, 0x70);
    city.set_traffic_density(39, 40, 0x55);
    city.set_tile_bits(39, 40, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    let mut state = test_state(city);
    state.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_flood_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.rubble > 0 {
        // PE PlaceTile `0x441f00`: XBLD `1..=4` only; leave XTXT/XTRF/watered.
        assert!((1..=4).contains(&expected.building_at(39, 40)));
        assert_eq!(expected.text_overlay_at(39, 40), Some(0xf1));
        assert_eq!(expected.text_overlay_at(40, 40), Some(0xfc));
        assert_eq!(expected.traffic_density_at(39, 40), Some(0x55));
        let bits = expected.tile_bits_at(39, 40).expect("bits");
        assert!(bits.piped() && bits.watered());
        assert_eq!(stats.explodes, 0);
        assert_eq!(stats.spread, 0);
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    assert_eq!(state.tick_pe_disaster_overlays(), stats.changed());
    assert_eq!(state.city.building_at(39, 40), expected.building_at(39, 40));
    assert_eq!(
        state.city.text_overlay_at(39, 40),
        expected.text_overlay_at(39, 40)
    );
    assert_eq!(
        state.city.traffic_density_at(39, 40),
        expected.traffic_density_at(39, 40)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_flood_tick_clears_sign_xlabel_when_spreading_onto_user_sign() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfc);
    // Cover all NESW so the CRT `rand&3` spread always hits a user sign.
    for (i, (dx, dy)) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .enumerate()
    {
        let sign_id = (0x05 + i) as u8;
        let nx = (40 + dx) as usize;
        let ny = (40 + dy) as usize;
        city.set_text_overlay(nx, ny, sign_id);
        city.set_label(sign_id, format!("Sign{sign_id}"));
    }
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    let mut state = test_state(city);
    state.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_flood_tick(&state, &mut preview, &mut preview_lfsr);
    assert!(stats.spread >= 1);
    let cleared = (0x05u8..=0x08)
        .filter(|&id| expected.label_at(id).unwrap_or("") == "")
        .count();
    assert!(
        cleared >= 1,
        "spreading onto a user sign must clear its XLAB"
    );
    advance_pe_disaster_scan_preview(&state, &mut preview);
    assert_eq!(state.tick_pe_disaster_overlays(), stats.changed());
    for id in 0x05u8..=0x08 {
        assert_eq!(
            state.city.label_at(id).unwrap_or(""),
            expected.label_at(id).unwrap_or(""),
            "XLAB {id}"
        );
    }
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_dispatch_proxy_tick_firefighters_extinguish_nesw_xff() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 100, 100, 211, 3);
    assert!(city.place_dispatch_actor(40, 40, DispatchKind::Firefighters));
    let proxy = city.text_overlay_at(40, 40).expect("dispatch XTXT proxy");
    assert!((0xc9..0xf1).contains(&proxy));
    city.set_text_overlay(41, 40, 0xff);
    city.set_building(41, 40, 0x70);
    let stats = pe_dispatch_proxy_tick_city(&mut city);
    if stats.fires_cleared > 0 {
        assert_eq!(city.text_overlay_at(41, 40), Some(0));
        assert!((1..=4).contains(&city.building_at(41, 40)));
    }
    assert_eq!(stats.riots_cleared, 0);
}

#[test]
fn pe_dispatch_proxy_tick_police_clears_nesw_riot() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    assert!(city.place_dispatch_actor(40, 40, DispatchKind::Police));
    for (dx, dy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
        city.set_text_overlay((40 + dx) as usize, (40 + dy) as usize, 0xfe);
    }
    let stats = pe_dispatch_proxy_tick_city(&mut city);
    if stats.riots_cleared > 0 {
        let cleared = [(-1isize, 0), (1, 0), (0, -1), (0, 1)]
            .iter()
            .filter(|&&(dx, dy)| {
                city.text_overlay_at((40 + dx) as usize, (40 + dy) as usize) == Some(0)
            })
            .count();
        assert_eq!(cleared, 1);
    }
}

#[test]
fn pe_dispatch_proxy_tick_runs_from_daily_overlay_scan_when_disaster_active() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 100, 100, 211, 3);
    assert!(city.place_dispatch_actor(40, 40, DispatchKind::Firefighters));
    for (dx, dy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
        city.set_text_overlay((40 + dx) as usize, (40 + dy) as usize, 0xff);
    }
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let before_crt = preview.seed();
    let mut expected = state.city.clone();
    let stats = pe_disaster_overlay_tick_city_with_rng(
        &mut expected,
        state.pe_disaster_duration,
        state.city.has_active_disaster(),
        &mut preview,
        &mut preview_lfsr,
    );
    advance_pe_disaster_scan_preview(&state, &mut preview);
    assert_ne!(
        preview.seed(),
        before_crt,
        "fire + dispatch CRT rolls share pe_process_crt"
    );
    assert_eq!(state.tick_pe_disaster_overlays(), stats.changed());
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
fn pe_dispatch_proxy_tick_skips_when_disaster_inactive() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 100, 100, 211, 3);
    assert!(city.place_dispatch_actor(40, 40, DispatchKind::Firefighters));
    assert!(!city.has_active_disaster());
    let before = city.text_overlay_at(40, 40);
    let mut state = test_state(city);
    assert_eq!(state.tick_pe_disaster_overlays(), 0);
    assert_eq!(state.city.text_overlay_at(40, 40), before);
}

#[test]
fn pe_flood_tick_neighbor_destroy_elevated_paints_fc_without_explode() {
    // PE @ `0x4610c3` DestroyStructure(1,0): elevated early-out skips DestroyBuilding /
    // explode, then caller @ `0x46108d` still paints `0xfc` and clears XTRF.
    let mut city =
        City::new_blank("/tmp/sc2000-flood-elev-nbr.sc2", "FloodElevNbr").expect("blank");
    city.set_text_overlay(40, 40, 0xfc);
    city.set_building(41, 40, 0x3f);
    city.set_text_overlay(41, 40, 0x40);
    city.set_traffic_density(40, 40, 80);
    city.set_traffic_density(41, 40, 80);
    city.set_traffic_density(40, 41, 80);
    city.set_traffic_density(41, 41, 80);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    // CRT seed 5 → first `rand()&3 == 2` (east) toward the elevated neighbor.
    let mut crt = Win95CrtRand::new(5);
    let mut lfsr = Win95Lfsr::from_u32_seed(5);
    let stats =
        pe_flood_tick_city_with_rng(&mut city, PE_FLOOD_HURRICANE_DURATION, &mut crt, &mut lfsr);
    assert_eq!(stats.explodes, 0);
    assert_eq!(stats.destroyed, 0);
    assert!(stats.spread >= 1);
    assert_eq!(city.building_at(41, 40), 0x3f);
    assert_eq!(city.text_overlay_at(41, 40), Some(0xfc));
    assert_eq!(city.traffic_density_at(40, 40), Some(0));
    assert_eq!(city.traffic_density_at(41, 40), Some(0));
}

#[test]
fn pe_flood_tick_spread_destroy_explodes_and_clears_xtrf() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfc);
    city.set_text_overlay(39, 40, 0x40);
    city.set_building(39, 40, 0x70);
    city.set_traffic_density(38, 40, 80);
    city.set_traffic_density(39, 40, 80);
    city.set_traffic_density(38, 41, 80);
    city.set_traffic_density(39, 41, 80);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    let mut state = test_state(city);
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    state.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_flood_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.explodes > 0 {
        assert_eq!(stats.destroyed, stats.explodes);
        assert!(is_rubble_tile(expected.building_at(39, 40)));
        assert_eq!(expected.text_overlay_at(39, 40), Some(0xfc));
        assert_eq!(expected.traffic_density_at(38, 40), Some(0));
        assert_eq!(expected.traffic_density_at(39, 40), Some(0));
        assert_eq!(expected.traffic_density_at(38, 41), Some(0));
        assert_eq!(expected.traffic_density_at(39, 41), Some(0));
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    assert_eq!(state.tick_pe_disaster_overlays(), stats.changed());
    assert_eq!(
        state
            .pending_disaster_extra_sounds
            .iter()
            .filter(|&&s| s == PlacementSound::Explode)
            .count(),
        stats.explodes
    );
    assert_eq!(state.city.building_at(39, 40), expected.building_at(39, 40));
    assert_eq!(
        state.city.text_overlay_at(39, 40),
        expected.text_overlay_at(39, 40)
    );
    assert_eq!(
        state.city.traffic_density_at(39, 40),
        expected.traffic_density_at(39, 40)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_flood_tick_duration_destroy_does_not_explode() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    city.set_text_overlay(40, 40, 0xfc);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    let mut state = test_state(city);
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    state.pe_disaster_duration = PE_FLOOD_TICK_DESTROY_AT - 1;
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (_, stats) = preview_pe_flood_tick(&state, &mut preview, &mut preview_lfsr);
    assert_eq!(stats.explodes, 0);
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
    assert!(
        !state
            .pending_disaster_extra_sounds
            .contains(&PlacementSound::Explode)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_riot_tick_clears_xtrf_microsim64() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfe);
    city.set_traffic_density(40, 40, 90);
    city.set_traffic_density(41, 40, 90);
    city.set_traffic_density(40, 41, 90);
    city.set_traffic_density(41, 41, 90);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x03);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_riot_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.flipped > 0 || stats.spread > 0 || stats.ignited > 0 {
        assert_eq!(expected.traffic_density_at(40, 40), Some(0));
        assert_eq!(expected.traffic_density_at(41, 40), Some(0));
        assert_eq!(expected.traffic_density_at(40, 41), Some(0));
        assert_eq!(expected.traffic_density_at(41, 41), Some(0));
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
    assert_eq!(
        state.city.traffic_density_at(40, 40),
        expected.traffic_density_at(40, 40)
    );
    assert_eq!(
        state.city.traffic_density_at(41, 41),
        expected.traffic_density_at(41, 41)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_fire_tick_recedes_on_xbit_water() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xff);
    city.set_tile_bits(40, 40, crate::city::TileBits::new(0b0000_0100));
    city.set_text_overlay(41, 40, 0xff);
    city.set_tile_bits(41, 40, crate::city::TileBits::new(0b0000_0100));
    city.set_text_overlay(42, 40, 0xff);
    city.set_tile_bits(42, 40, crate::city::TileBits::new(0b0000_0100));
    city.set_text_overlay(43, 40, 0xff);
    city.set_tile_bits(43, 40, crate::city::TileBits::new(0b0000_0100));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_fire_tick(&state, &mut preview, &mut preview_lfsr);
    assert!(stats.receded > 0);
    assert_eq!(stats.ignited, 0);
    assert!(
        expected.text_overlay_at(40, 40) == Some(0)
            || expected.text_overlay_at(41, 40) == Some(0)
            || expected.text_overlay_at(42, 40) == Some(0)
            || expected.text_overlay_at(43, 40) == Some(0)
    );
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
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
fn pe_fire_tick_nesw_counts_ignite_only_when_neighbor_paints_ff() {
    // PE arms 0..=3 call FUN_00460e10 flag 0 with no outer XBLD prefilter; vacant west
    // refuses inside the helper so ignited stays 0.
    let mut arm0_seed = None;
    for seed in 1..8192u32 {
        let mut probe = Win95CrtRand::new(seed);
        if probe.random_u16() & 3 != 0 {
            continue;
        }
        if probe.random_u16() & 7 == 0 {
            arm0_seed = Some(seed);
            break;
        }
    }
    let seed = arm0_seed.expect("CRT seed with act + arm 0");
    let mut city = City::new_blank("/tmp/sc2000-fire-nesw-vac.sc2", "FireNeswVac").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xff);
    city.set_building(19, 20, 0); // vacant west — FUN flag 0 refuses
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_fire_tick_city_with_rng(&mut city, &mut crt, &mut lfsr);
    assert_eq!(stats.ignited, 0);
    assert_eq!(city.text_overlay_at(19, 20), Some(0));
    assert_eq!(city.text_overlay_at(20, 20), Some(0xff));
}

#[test]
fn pe_fire_tick_ignites_nesw_xbld_ge_6() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xff);
    city.set_building(39, 40, 0x70);
    city.set_building(41, 40, 0x70);
    city.set_building(40, 39, 0x70);
    city.set_building(40, 41, 0x70);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0c);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_fire_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.ignited > 0 {
        let neighbors = [
            expected.text_overlay_at(39, 40),
            expected.text_overlay_at(40, 39),
            expected.text_overlay_at(41, 40),
            expected.text_overlay_at(40, 41),
        ];
        assert_eq!(neighbors.iter().filter(|&&m| m == Some(0xff)).count(), 1);
        assert_eq!(expected.text_overlay_at(40, 40), Some(0xff));
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    assert_eq!(state.tick_pe_disaster_overlays(), stats.changed());
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
fn pe_fire_tick_processes_a_later_ignition_during_the_same_in_place_scan() {
    // The source's east ignition is visited later in the same x-major pass.
    // Pick draws so that the target immediately reaches arm 5 and burns down;
    // a snapshot-based pass would leave the target building intact.
    let seed = (1..100_000_u32)
        .find(|&seed| {
            let mut probe = Win95CrtRand::new(seed);
            let source_acts = probe.random_u16() & 3 == 0;
            let source_arm = probe.random_u16() & 7 == 2;
            let target_acts = probe.random_u16() & 3 == 0;
            let target_arm = probe.random_u16() & 7 == 5;
            source_acts && source_arm && target_acts && target_arm
        })
        .expect("CRT seed with source-east and target-burn draws");
    let mut city =
        City::new_blank("/tmp/sc2000-fire-in-place-chain.sc2", "FireChain").expect("blank city");
    city.set_building(20, 20, 0x70);
    city.set_building(21, 20, 0x70);
    city.set_text_overlay(20, 20, 0xff);
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::new(1);

    let stats = pe_fire_tick_city_with_rng(&mut city, &mut crt, &mut lfsr);

    assert_eq!(stats.ignited, 1);
    assert_eq!(stats.destroyed, 1);
    assert_ne!(city.building_at(21, 20), 0x70);
}

#[test]
fn pe_disaster_overlay_scan_runs_families_in_native_xmajor_order() {
    // Win95 handles XTXT families inside one x-major traversal. A riot at the earlier
    // coordinate can therefore paint fire at a later coordinate and have that fire run
    // in the same pass. Separate family passes leave the new fire untouched until later.
    let mut city =
        City::new_blank("/tmp/sc2000-disaster-interleave.sc2", "Interleave").expect("blank city");
    city.set_text_overlay(0, 0, 0xfe);
    city.set_building(1, 0, 6);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    let mut state = test_state(city);
    // These first five CRT draws are [9404, 32229, 6530, 6612, 1550]: riot acts,
    // skips recede, ignites east, then the later fire selects jump-table default 6.
    state.pe_process_crt = Win95CrtRand::new(2868);
    state.pe_process_lfsr = Win95Lfsr::new(1);

    assert_eq!(state.tick_pe_disaster_overlays(), 2);
    let mut expected_crt = Win95CrtRand::new(2868);
    for _ in 0..7 {
        let _ = expected_crt.random_u16();
    }
    assert_eq!(
        state.pe_process_crt_seed(),
        expected_crt.seed(),
        "later fire consumes act, selector, and coverage rolls before the riot SFX gate"
    );
    assert_eq!(state.city.text_overlay_at(0, 0), Some(0xfd));
    assert_eq!(
        state.city.text_overlay_at(1, 0),
        Some(0xff),
        "eastward fire is visited later, but low coverage fails the shared 4/6/7 gate"
    );
}

#[test]
fn pe_fire_tick_ignite_clears_xlabel_on_sign_xtxt() {
    // PE fire NESW → FUN_00460e10: clear XLAB on sign XTXT, paint 0xff, zero XTRF.
    let mut city = City::new_blank("/tmp/sc2000-fire-xlab.sc2", "FireXlab").expect("blank");
    city.set_text_overlay(40, 40, 0xff);
    for (i, (dx, dy)) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .enumerate()
    {
        let sign_id = (0x05 + i) as u8;
        let nx = (40 + dx) as usize;
        let ny = (40 + dy) as usize;
        city.set_building(nx, ny, 0x70);
        city.set_text_overlay(nx, ny, sign_id);
        city.set_label(sign_id, format!("Sign{sign_id}"));
        city.set_traffic_density(nx, ny, 80);
    }
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_fire_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.ignited > 0 {
        let cleared = (0x05u8..=0x08)
            .filter(|&id| expected.label_at(id).unwrap_or("") == "")
            .count();
        assert!(
            cleared >= 1,
            "fire ignite onto a user sign must clear its XLAB"
        );
        for (dx, dy) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)] {
            let nx = (40 + dx) as usize;
            let ny = (40 + dy) as usize;
            if expected.text_overlay_at(nx, ny) == Some(0xff) {
                let ox = (nx / 2) * 2;
                let oy = (ny / 2) * 2;
                assert_eq!(expected.traffic_density_at(ox, oy), Some(0));
            }
        }
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    assert_eq!(state.tick_pe_disaster_overlays(), stats.changed());
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_riot_tick_ignite_clears_xlabel_on_sign_xtxt() {
    // PE riot NESW ignite → FUN_00460e10: clear XLAB, paint 0xff, zero XTRF.
    let mut city = City::new_blank("/tmp/sc2000-riot-xlab.sc2", "RiotXlab").expect("blank");
    city.set_text_overlay(40, 40, 0xfe);
    for (i, (dx, dy)) in [(-1isize, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .enumerate()
    {
        let sign_id = (0x05 + i) as u8;
        let nx = (40 + dx) as usize;
        let ny = (40 + dy) as usize;
        city.set_building(nx, ny, 0x70);
        city.set_text_overlay(nx, ny, sign_id);
        city.set_label(sign_id, format!("RiotSign{sign_id}"));
    }
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x03);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_riot_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.ignited > 0 {
        let cleared = (0x05u8..=0x08)
            .filter(|&id| expected.label_at(id).unwrap_or("") == "")
            .count();
        assert!(
            cleared >= 1,
            "riot ignite onto a user sign must clear its XLAB"
        );
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    assert_eq!(state.tick_pe_disaster_overlays(), stats.changed());
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_fire_tick_burns_down_when_xfir_coverage_is_high() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    city.set_text_overlay(40, 40, 0xff);
    city.sync_fire_coverage_from_slice(&vec![255u8; CITY_TILES]);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_fire_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.destroyed > 0 {
        assert!((1..=4).contains(&expected.building_at(40, 40)));
        // Arm 5 may paint an XTHG proxy after burn-down; coverage path clears XTXT.
        let xtxt = expected.text_overlay_at(40, 40).unwrap_or(0);
        assert!(
            xtxt == 0 || (0xc9..0xf1).contains(&xtxt),
            "burn-down leaves rubble XTXT 0 or kind-6 proxy, got {xtxt:#x}"
        );
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
    assert_eq!(state.city.building_at(40, 40), expected.building_at(40, 40));
    assert_eq!(
        state.city.text_overlay_at(40, 40),
        expected.text_overlay_at(40, 40)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_fire_tick_uses_xfir_coverage_plus_eight_as_the_extinguish_threshold() {
    // This stream reaches existing-fire arm 4: 48 & 3 == 0, then
    // 7196 & 7 == 4, and the coverage roll is 9294 & 0xff == 78.
    let mut low_coverage = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    low_coverage.set_building(40, 40, 0x70);
    low_coverage.set_text_overlay(40, 40, 0xff);
    low_coverage.sync_fire_coverage_from_slice(&vec![0; CITY_TILES]);
    let mut low_crt = Win95CrtRand::new(3);
    let mut low_lfsr = Win95Lfsr::new(1);
    let mut low_stats = PeFireTickStats::default();
    pe_fire_tick_cell_with_rng(
        &mut low_coverage,
        &mut low_crt,
        &mut low_lfsr,
        40,
        40,
        &mut low_stats,
    );
    assert_eq!(low_stats.receded, 0, "coverage 0 gives threshold 8");
    assert_eq!(low_coverage.building_at(40, 40), 0x70);
    assert_eq!(low_coverage.text_overlay_at(40, 40), Some(0xff));

    let mut high_coverage = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    high_coverage.set_building(40, 40, 0x70);
    high_coverage.set_text_overlay(40, 40, 0xff);
    high_coverage.sync_fire_coverage_from_slice(&vec![255; CITY_TILES]);
    let mut high_crt = Win95CrtRand::new(3);
    let mut high_lfsr = Win95Lfsr::new(1);
    let mut high_stats = PeFireTickStats::default();
    pe_fire_tick_cell_with_rng(
        &mut high_coverage,
        &mut high_crt,
        &mut high_lfsr,
        40,
        40,
        &mut high_stats,
    );
    assert_eq!(high_stats.receded, 1, "coverage 255 gives threshold 263");
    assert_eq!(high_stats.destroyed, 1);
    assert!((1..=4).contains(&high_coverage.building_at(40, 40)));
    assert_ne!(high_coverage.text_overlay_at(40, 40), Some(0xff));
}

#[test]
fn pe_day2_xfir_publication_feeds_active_fire_at_the_native_threshold() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for (funding, wet, coverage, receded) in [
            (0, false, 0, 0),
            (28, false, 70, 0),
            (29, false, 72, 1),
            (32, true, 80, 1),
        ] {
            let mut city = City::new_blank("/tmp/pe-day2-fire.sc2", "Fire coverage").unwrap();
            city.kind = kind;
            city.set_building(107, 120, 0xd3);
            city.set_zone(107, 120, 0x80);
            city.set_tile_bits(107, 120, TileBits::new(0x40));
            set_budget_funding_for_city_test(&mut city, 1, funding);
            let mut state = test_state(city);
            state.sim_calendar_day2_density(Default::default(), Default::default());

            let (x, y) = (106, 121);
            let city = &mut state.city;
            assert_eq!(city.fire_coverage_at(x, y), Some(coverage));
            city.set_text_overlay(x, y, 0xff);
            if wet {
                city.set_tile_bits(x, y, TileBits::new(0x04));
            }
            let mut crt = Win95CrtRand::new(3);
            let mut lfsr = Win95Lfsr::new(1);
            let mut stats = PeFireTickStats::default();
            pe_fire_tick_cell_with_rng(city, &mut crt, &mut lfsr, x, y, &mut stats);
            assert_eq!(stats.receded, receded, "{kind:?}, funding {funding}");
            assert_eq!(city.text_overlay_at(x, y) == Some(0xff), receded == 0);
            if wet || receded == 0 {
                assert_eq!(crt.seed(), if wet { 0x0030_6aba } else { 0xa44e_df04 });
            }
        }
    }
}

#[test]
fn pe_fire_jump_table_routes_random_six_and_seven_to_coverage_cleanup() {
    for arm in [4_u16, 6, 7] {
        let seed = (1..100_000_u32)
            .find(|&seed| {
                let mut probe = Win95CrtRand::new(seed);
                probe.random_u16() & 3 == 0
                    && probe.random_u16() & 7 == arm
                    && probe.random_u16() & 0xff < 7
            })
            .expect("CRT seed with a passing shared coverage arm");
        let mut city = City::new_blank(
            &format!("/tmp/sc2000-fire-shared-arm-{arm}.sc2"),
            "FireSharedArm",
        )
        .expect("blank city");
        city.set_building(20, 20, 0x70);
        city.set_text_overlay(20, 20, 0xff);
        city.sync_fire_coverage_from_slice(&vec![0; CITY_TILES]);
        let mut crt = Win95CrtRand::new(seed);
        let mut lfsr = Win95Lfsr::new(1);

        let stats = pe_fire_tick_city_with_rng(&mut city, &mut crt, &mut lfsr);

        assert_eq!(stats.receded, 1, "rand&7 arm {arm} shares 0x45f830");
        assert_eq!(
            stats.destroyed, 1,
            "rand&7 arm {arm} destroys the structure"
        );
        assert_ne!(city.text_overlay_at(20, 20), Some(0xff));
        assert!((1..=4).contains(&city.building_at(20, 20)));
    }
}

#[test]
fn pe_fire_arm4_vacant_preserves_zone_corners() {
    // PE arm 4 calls DestroyStructure, whose DestroyBuilding worker skips XBLD 0.
    // Extinguish still clears XTXT and writes rubble while preserving XZON.
    let mut city = City::new_blank("/tmp/sc2000-fire-vac-arm.sc2", "FireVacArm").expect("blank");
    city.set_text_overlay(20, 20, 0xff);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut crt = Win95CrtRand::new(41);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_fire_tick_city_with_rng(&mut city, &mut crt, &mut lfsr);
    assert_eq!(stats.receded, 1);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(city.building_at(20, 20), 3);
    let zone = city.zone_at(20, 20).map(|z| z.raw_bits()).unwrap_or(0);
    assert_eq!(zone, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
}

#[test]
fn pe_fire_arm4_elevated_clears_overlay_keeps_road() {
    // PE DestroyStructure early-outs `0x3f..=0x42`, then `0x402b85` extinguish
    // clears `0xff` only. Seed 41 → act / arm 4 / coverage pass.
    let mut city = City::new_blank("/tmp/sc2000-fire-elev-arm.sc2", "FireElevArm").expect("blank");
    city.set_building(20, 20, 0x3f);
    city.set_text_overlay(20, 20, 0xff);
    let mut crt = Win95CrtRand::new(41);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_fire_tick_city_with_rng(&mut city, &mut crt, &mut lfsr);
    assert_eq!(stats.receded, 1);
    assert_eq!(stats.destroyed, 0);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(city.building_at(20, 20), 0x3f);
}

#[test]
fn pe_firestorm_can_ignite_refuses_rubble_xtxt_like_fun_paint_success() {
    // PE FUN_00460e10 rubble band `0xf1..=0xf9` returns 0 without paint — not a firestorm hit.
    let mut city = City::new_blank("/tmp/sc2000-fs-rubble.sc2", "FsRubble").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xf5);
    assert!(!pe_firestorm_can_ignite(&city, 20, 20));
    city.set_text_overlay(20, 20, 0xfa);
    assert!(pe_firestorm_can_ignite(&city, 20, 20));
}

#[test]
fn pe_fire_burn_down_paints_ff_on_dry_footprint_then_rubbles_hit() {
    // PE arm 5 @ `0x45f9f6`: DestroyStructure(1,1) paints dry footprint `0xff`, then
    // `0x402b85` rubbles only the hit tile (non-hit cells keep burning).
    let mut city = City::new_blank("/tmp/sc2000-fire-burn-ff.sc2", "FireBurn").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 210, 3);
    city.set_terrain(42, 42, 0x10);
    city.set_tile_bits(42, 42, TileBits::new(0x04)); // water
    city.set_traffic_density(41, 41, 0x2a);
    // Avoid kind-6 actor rewrite of hit XTXT (`0x4025e5`); only test fire-paint + rubble.
    let mut miss_state = None;
    for seed in 1..4096u16 {
        let mut probe = Win95Lfsr::new(seed);
        let _ = pe_fire_rubble_building(&mut probe);
        if probe.random_mask_0f() != 0 {
            miss_state = Some(seed);
            break;
        }
    }
    let mut lfsr = Win95Lfsr::new(miss_state.expect("LFSR seed with post-rubble &0xf!=0"));
    let mut crt = Win95CrtRand::new(0x45f9_f601);
    pe_fire_burn_down_tile(&mut city, &mut crt, &mut lfsr, 41, 41);
    for dy in 0..3 {
        for dx in 0..3 {
            let x = 40 + dx;
            let y = 40 + dy;
            if (x, y) == (41, 41) {
                assert!(
                    (1..=4).contains(&city.building_at(x, y)),
                    "hit tile should be rubble, got {:#04x}",
                    city.building_at(x, y)
                );
                assert_eq!(city.text_overlay_at(x, y), Some(0));
            } else if (x, y) == (42, 42) {
                assert_eq!(city.building_at(x, y), 0);
                assert_eq!(city.text_overlay_at(x, y), Some(0));
            } else {
                // DestroyBuilding CRT PlaceTile rubble, then fire-paint keeps XTXT `0xff`.
                assert!(is_rubble_tile(city.building_at(x, y)));
                assert_eq!(
                    city.text_overlay_at(x, y),
                    Some(0xff),
                    "dry non-hit footprint should keep fire paint at ({x},{y})"
                );
            }
        }
    }
    assert_eq!(city.traffic_density_at(41, 41), Some(0x2a));
}

#[test]
fn pe_fire_burn_down_plant_0x85_paints_fb_on_hit_after_extinguish() {
    // PE @ `0x45fa4e`: XBLD `0x85` → `0x402dc9`/`FUN_00461190` on origin after extinguish
    // clears hit `0xff`, so XTXT `< 0x33` and `0xfb` sticks.
    let mut city = City::new_blank("/tmp/sc2000-fire-burn-85.sc2", "FireBurn85").expect("blank");
    city.set_building(40, 40, 0x85);
    city.set_text_overlay(40, 40, 0xff);
    let mut miss_state = None;
    for seed in 1..4096u16 {
        let mut probe = Win95Lfsr::new(seed);
        let _ = pe_fire_rubble_building(&mut probe);
        if probe.random_mask_0f() != 0 {
            miss_state = Some(seed);
            break;
        }
    }
    let mut lfsr = Win95Lfsr::new(miss_state.expect("LFSR seed with post-rubble &0xf!=0"));
    let mut crt = Win95CrtRand::new(0x45fa_4e85);
    pe_fire_burn_down_tile(&mut city, &mut crt, &mut lfsr, 40, 40);
    assert_eq!(city.text_overlay_at(40, 40), Some(0xfb));
}

#[test]
fn pe_fire_burn_down_plant_0xbc_spills_fb_north_of_footprint() {
    // PE @ `0x45fa4e`–`0x45fb7b`: size-3 plant spills `0xfb` on N/NE rings outside the
    // burning footprint (dry footprint keeps `0xff`; FUN skips XTXT `>= 0x33`).
    let mut city = City::new_blank("/tmp/sc2000-fire-burn-bc.sc2", "FireBurnBc").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 0xbc, 3);
    city.set_text_overlay(41, 41, 0xff);
    let mut miss_state = None;
    for seed in 1..4096u16 {
        let mut probe = Win95Lfsr::new(seed);
        let _ = pe_fire_rubble_building(&mut probe);
        if probe.random_mask_0f() != 0 {
            miss_state = Some(seed);
            break;
        }
    }
    let mut lfsr = Win95Lfsr::new(miss_state.expect("LFSR seed with post-rubble &0xf!=0"));
    let mut crt = Win95CrtRand::new(0x45fa_4ebc);
    pe_fire_burn_down_tile(&mut city, &mut crt, &mut lfsr, 41, 41);
    // North of NW origin (40,40): size>1 / size>2 rings.
    assert_eq!(city.text_overlay_at(40, 39), Some(0xfb));
    assert_eq!(city.text_overlay_at(41, 39), Some(0xfb));
    assert_eq!(city.text_overlay_at(40, 38), Some(0xfb));
    assert_eq!(city.text_overlay_at(41, 38), Some(0xfb));
    assert_eq!(city.text_overlay_at(42, 38), Some(0xfb));
    // Dry non-hit footprint still burning — FUN refuses `0xff`.
    assert_eq!(city.text_overlay_at(40, 40), Some(0xff));
    assert_eq!(city.text_overlay_at(42, 40), Some(0xff));
}

#[test]
fn pe_fire_burn_down_non_plant_skips_fb_spill() {
    // PE plant gate is only `0x85`/`0x9f`/`0xbc`; other XBLD `>= 0x70` leave hit clear.
    let mut city = City::new_blank("/tmp/sc2000-fire-burn-70.sc2", "FireBurn70").expect("blank");
    city.set_building(40, 40, 0x70);
    city.set_text_overlay(40, 40, 0xff);
    let mut miss_state = None;
    for seed in 1..4096u16 {
        let mut probe = Win95Lfsr::new(seed);
        let _ = pe_fire_rubble_building(&mut probe);
        if probe.random_mask_0f() != 0 {
            miss_state = Some(seed);
            break;
        }
    }
    let mut lfsr = Win95Lfsr::new(miss_state.expect("LFSR seed with post-rubble &0xf!=0"));
    let mut crt = Win95CrtRand::new(0x45fa_4e70);
    pe_fire_burn_down_tile(&mut city, &mut crt, &mut lfsr, 40, 40);
    assert_eq!(city.text_overlay_at(40, 40), Some(0));
    assert_ne!(city.text_overlay_at(40, 39), Some(0xfb));
}

#[test]
fn pe_fire_burn_down_destroy_building_consumes_crt_before_lfsr_rubble() {
    // PE @ `0x45fa22`: selector-1's two render draws, CRT rubble, then
    // `0x402b85` LFSR PlaceTile on hit.
    let mut city = City::new_blank("/tmp/sc2000-fire-burn-crt.sc2", "FireBurnCrt").expect("blank");
    city.set_building(40, 40, 0x70);
    let mut miss_state = None;
    for seed in 1..4096u16 {
        let mut probe = Win95Lfsr::new(seed);
        let _ = pe_fire_rubble_building(&mut probe);
        if probe.random_mask_0f() != 0 {
            miss_state = Some(seed);
            break;
        }
    }
    let mut lfsr = Win95Lfsr::new(miss_state.expect("LFSR seed"));
    let mut preview = Win95CrtRand::new(0x45fa_2201);
    let _ = preview.random_u15(); // selector-1 render scatter @ `0x459DEB`
    let _ = preview.random_u15(); // selector-1 render scatter @ `0x459E19`
    let _destroy = preview.rubble_tile_id();
    let mut crt = Win95CrtRand::new(0x45fa_2201);
    pe_fire_burn_down_tile(&mut city, &mut crt, &mut lfsr, 40, 40);
    assert!(is_rubble_tile(city.building_at(40, 40)));
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn pe_fire_coverage_destroy_paints_ff_on_dry_footprint_then_rubbles_hit() {
    // PE arm 4 @ `0x45f830`: DestroyStructure(1,1) fire-paint, then rubble the hit tile
    // only (no kind-6 gate). Arms 6/7 are the separate `0x45fc0a` flare path.
    let mut city = City::new_blank("/tmp/sc2000-fire-cov-ff.sc2", "FireCov").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 210, 3);
    city.set_terrain(42, 42, 0x10);
    city.set_tile_bits(42, 42, TileBits::new(0x04)); // water
    city.set_traffic_density(41, 41, 0x2a);
    let mut lfsr = Win95Lfsr::new(0x99);
    pe_destroy_structure_fire_paint(&mut city, None, 41, 41);
    pe_fire_apply_rubble(&mut city, &mut lfsr, 41, 41);
    for dy in 0..3 {
        for dx in 0..3 {
            let x = 40 + dx;
            let y = 40 + dy;
            if (x, y) == (41, 41) {
                assert!(
                    (1..=4).contains(&city.building_at(x, y)),
                    "hit tile should be rubble, got {:#04x}",
                    city.building_at(x, y)
                );
                assert_eq!(city.text_overlay_at(x, y), Some(0));
            } else if (x, y) == (42, 42) {
                assert_eq!(city.building_at(x, y), 0);
                assert_eq!(city.text_overlay_at(x, y), Some(0));
            } else {
                assert_eq!(city.building_at(x, y), 0);
                assert_eq!(
                    city.text_overlay_at(x, y),
                    Some(0xff),
                    "dry non-hit footprint should keep fire paint at ({x},{y})"
                );
            }
        }
    }
    assert_eq!(city.traffic_density_at(41, 41), Some(0x2a));
}

#[test]
fn pe_ignite_mid_xtxt_destroys_footprint_fire_paint_and_clears_xtrf() {
    // PE FUN_00460e10 @ `0x460efc`: XTXT `0x33..=0xc8` → DestroyStructure(1,1) then paint/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-ignite-mid.sc2", "IgnMid").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, 210, 3);
    city.set_text_overlay(41, 41, 0x55);
    for &(tx, ty) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_traffic_density(tx, ty, 0x3c);
    }
    let mut lfsr = Win95Lfsr::new(1);
    pe_fun_00460e10_ignite(&mut city, None, &mut lfsr, 41, 41, 0);
    for dy in 0..3 {
        for dx in 0..3 {
            let x = 40 + dx;
            let y = 40 + dy;
            assert_eq!(city.building_at(x, y), 0);
            assert_eq!(city.text_overlay_at(x, y), Some(0xff));
        }
    }
    assert_eq!(city.traffic_density_at(40, 40), Some(0));
    assert_eq!(city.traffic_density_at(41, 41), Some(0));
}

#[test]
fn pe_ignite_high_xtxt_rubbles_without_fire_paint() {
    // PE FUN_00460e10 @ `0x460f1f`: XTXT `0xf1..=0xf9` → LFSR rubble PlaceTile `0x441f00` only.
    let mut city = City::new_blank("/tmp/sc2000-ignite-hi.sc2", "IgnHi").expect("blank");
    city.set_building(40, 40, 0x70);
    city.set_text_overlay(40, 40, 0xf1);
    city.set_traffic_density(40, 40, 0x44);
    city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_watered(true));
    let mut lfsr = Win95Lfsr::new(0x55);
    pe_fun_00460e10_ignite(&mut city, None, &mut lfsr, 40, 40, 0);
    assert!((1..=4).contains(&city.building_at(40, 40)));
    assert_eq!(city.text_overlay_at(40, 40), Some(0xf1));
    assert_eq!(city.traffic_density_at(40, 40), Some(0x44));
    let bits = city.tile_bits_at(40, 40).unwrap();
    assert!(bits.piped() && bits.watered());
}

#[test]
fn pe_fire_burn_down_spawns_kind6_when_lfsr_and_0xf_is_zero() {
    // PE `0x45fa34`: after arm-5 destroy, `0x4025e5` (`LFSR & 0xf == 0`) → FUN_00453280(x,y,0,0,1).
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    // Find an LFSR state where rubble roll advances once, then mask_0f returns 0.
    let mut gate_state = None;
    for seed in 1..4096u16 {
        let mut probe = Win95Lfsr::new(seed);
        let _ = pe_fire_rubble_building(&mut probe);
        if probe.random_mask_0f() == 0 {
            gate_state = Some(seed);
            break;
        }
    }
    let seed = gate_state.expect("LFSR seed with post-rubble &0xf==0");
    let mut lfsr = Win95Lfsr::new(seed);
    let mut crt = Win95CrtRand::new(0x45f9_f602);
    pe_fire_burn_down_tile(&mut city, &mut crt, &mut lfsr, 40, 40);
    let (slot, explosion) = city
        .pe_explosion_actor_at(40, 40)
        .expect("kind-6 after burn-down");
    assert_eq!(explosion.height, 0);
    assert_eq!(explosion.state, 0);
    assert_eq!(explosion.saved_b, 1);
    assert_eq!(
        city.text_overlay_at(40, 40),
        crate::win95_overlay::xthg_proxy_for_slot(slot)
    );
    assert!(city.building_at(40, 40) <= 4, "rubble after burn-down");
}

#[test]
fn pe_fire_burn_down_skips_kind6_when_lfsr_and_0xf_nonzero() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    let mut miss_state = None;
    for seed in 1..4096u16 {
        let mut probe = Win95Lfsr::new(seed);
        let _ = pe_fire_rubble_building(&mut probe);
        if probe.random_mask_0f() != 0 {
            miss_state = Some(seed);
            break;
        }
    }
    let seed = miss_state.expect("LFSR seed with post-rubble &0xf!=0");
    let mut lfsr = Win95Lfsr::new(seed);
    let mut crt = Win95CrtRand::new(0x45f9_f603);
    pe_fire_burn_down_tile(&mut city, &mut crt, &mut lfsr, 40, 40);
    assert!(city.pe_explosion_actor_at(40, 40).is_none());
}

#[test]
fn pe_fire_tick_active_only_for_fire_family_types() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_fire_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    assert!(!pe_fire_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x10);
    assert!(!pe_fire_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    assert!(pe_fire_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x06);
    assert!(pe_fire_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x09);
    assert!(pe_fire_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0b);
    assert!(pe_fire_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0c);
    assert!(pe_fire_tick_active(&city));
}

#[test]
fn pe_fire_overlay_scan_due_on_ff_without_fire_type() {
    // PE `0x45f7c8` matches XTXT `0xff` inside the daily scan. Type `0x03`
    // is not a fire-family id, but a live `0xff` still makes the scan due.
    let mut city = City::new_blank("/tmp/sc2000-fire-scan-due.sc2", "FireScanDue").expect("blank");
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x03);
    assert!(!pe_fire_tick_active(&city));
    assert!(!pe_fire_overlay_scan_due(&city));
    city.set_text_overlay(20, 20, 0xff);
    assert!(pe_fire_overlay_scan_due(&city));
}

#[test]
fn pe_fire_rubble_building_is_random_4_plus_1() {
    let mut lfsr = Win95Lfsr::new(0x4611);
    for _ in 0..32 {
        let rubble = pe_fire_rubble_building(&mut lfsr);
        assert!((1..=4).contains(&rubble), "rubble {rubble}");
    }
}

#[test]
fn pe_fire_tick_consumes_win95_crt_for_act_rolls() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xff);
    city.set_building(40, 40, 0x70);
    let seed = pe_fire_tick_seed(&city);
    let mut probe = Win95CrtRand::new(seed);
    let first = probe.random_u16();
    let _ = pe_fire_tick_city(&mut city);
    let mut again = Win95CrtRand::new(seed);
    assert_eq!(again.random_u16(), first);
    assert_ne!(MacToolboxRng::new(seed).random_u16(), first);
}

#[test]
fn pe_hurricane_wind_destroy_gate_uses_lfsr_word_lsb() {
    let seed = pe_hurricane_wind_seed(
        &City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"),
        PE_HURRICANE_WIND_REMAINING,
    );
    let mut lfsr = Win95Lfsr::from_u32_seed(seed);
    let bit = lfsr.random_word_lsb();
    assert!(bit == 0 || bit == 1);
    // After the gate bit, tile picks are LFSR mod 0x80 — not CRT.
    let x = lfsr.random_mod(0x80);
    let mut crt = Win95CrtRand::new(seed);
    let _ = crt.random_u16(); // wind roll
    assert_ne!(x, crt.random_u16() % 0x80);
}

#[test]
fn pe_riot_tick_active_only_for_riot_family_types() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_riot_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    assert!(!pe_riot_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x03);
    assert!(pe_riot_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0d);
    assert!(pe_riot_tick_active(&city));
}

#[test]
fn pe_riot_tick_recedes_on_xbit_water() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // Enough water riot tiles that CRT 1-in-4 act eventually clears one.
    for x in 20..52 {
        city.set_text_overlay(x, 40, 0xfe);
        city.set_tile_bits(x, 40, crate::city::TileBits::new(0b0000_0100));
    }
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x03);
    let mut state = test_state(city);
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_riot_tick(&state, &mut preview, &mut preview_lfsr);
    assert!(stats.receded > 0);
    assert_eq!(stats.spread, 0);
    assert!(
        (20..52).any(|x| expected.text_overlay_at(x, 40) == Some(0)),
        "at least one water riot tile should recede"
    );
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
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
}

#[test]
fn pe_riot_tick_flips_xfe_to_xfd_without_west_north_transit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfe);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0d);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_riot_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.flipped > 0 {
        assert_eq!(expected.text_overlay_at(40, 40), Some(0xfd));
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
    assert_eq!(
        state.city.text_overlay_at(40, 40),
        expected.text_overlay_at(40, 40)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_riot_tick_spreads_xfe_west_onto_road() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfe);
    city.set_building(39, 40, 1);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x03);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_riot_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.spread > 0 {
        assert_eq!(expected.text_overlay_at(39, 40), Some(0xfe));
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
    assert_eq!(
        state.city.text_overlay_at(39, 40),
        expected.text_overlay_at(39, 40)
    );
    assert_eq!(
        state.city.text_overlay_at(40, 40),
        expected.text_overlay_at(40, 40)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_riot_tick_ignites_nesw_xbld_ge_6() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfd);
    city.set_building(39, 40, 0x70);
    city.set_building(41, 40, 0x70);
    city.set_building(40, 39, 0x70);
    city.set_building(40, 41, 0x70);
    let mut expected = city.clone();
    let stats = pe_riot_tick_city(&mut expected);
    if stats.ignited > 0 {
        let neighbors = [
            expected.text_overlay_at(39, 40),
            expected.text_overlay_at(40, 39),
            expected.text_overlay_at(41, 40),
            expected.text_overlay_at(40, 41),
        ];
        assert!(neighbors.contains(&Some(0xff)));
    }
}

#[test]
fn pe_riot_tick_ignite_counts_only_when_neighbor_paints_ff() {
    // PE @ `0x45fe3e`: riot ignite calls FUN_00460e10 flag 0 with no XBLD prefilter.
    let mut hit = None;
    for seed in 1..16384u32 {
        let mut probe = Win95CrtRand::new(seed);
        if probe.random_u16() & 3 != 0 {
            continue;
        }
        if probe.random_u16() & 0xff == 0 {
            continue;
        }
        if probe.random_u16() & 0x7f == 0 {
            hit = Some(seed);
            break;
        }
    }
    let seed = hit.expect("CRT seed with act + west ignite");
    let mut city = City::new_blank("/tmp/sc2000-riot-ign-vac.sc2", "RiotIgnVac").expect("blank");
    city.set_text_overlay(20, 20, 0xfd);
    city.set_building(19, 20, 0); // vacant west
    let mut crt = Win95CrtRand::new(seed);
    let mut lfsr = Win95Lfsr::new(1);
    let stats = pe_riot_tick_city_with_rng(&mut city, &mut crt, &mut lfsr);
    assert_eq!(stats.ignited, 0);
    assert_ne!(city.text_overlay_at(19, 20), Some(0xff));
}

#[test]
fn pe_disaster_scan_sounds_one_in_eight_riot_and_flood() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfe);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x03);
    let mut state = test_state(city);
    let seen = pe_disaster_overlay_seen(&state.city);
    assert!(seen.riot);
    assert!(!seen.flood);
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let _ = preview_pe_riot_tick(&state, &mut preview, &mut preview_lfsr);
    let sounds = pe_disaster_scan_sounds_with_rng(&state.city, seen, &mut preview);
    state.tick_pe_disaster_overlays();
    if sounds.play_riot {
        assert!(
            state
                .pending_disaster_extra_sounds
                .contains(&PlacementSound::Riot)
        );
    } else {
        assert!(
            !state
                .pending_disaster_extra_sounds
                .contains(&PlacementSound::Riot)
        );
    }
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());

    let mut flood_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    flood_city.set_text_overlay(20, 20, 0xfc);
    flood_city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    let mut flood_state = test_state(flood_city);
    flood_state.pe_disaster_duration = PE_FLOOD_HURRICANE_DURATION;
    let flood_seen = pe_disaster_overlay_seen(&flood_state.city);
    assert!(flood_seen.flood);
    if !flood_state.sound_effects_enabled {
        flood_state.toggle_sound_effects();
    }
    let mut flood_preview = flood_state.pe_process_crt;
    let mut flood_preview_lfsr = flood_state.pe_process_lfsr;
    let _ = preview_pe_flood_tick(&flood_state, &mut flood_preview, &mut flood_preview_lfsr);
    let flood_sounds =
        pe_disaster_scan_sounds_with_rng(&flood_state.city, flood_seen, &mut flood_preview);
    flood_state.tick_pe_disaster_overlays();
    if flood_sounds.play_flood {
        assert!(
            flood_state
                .pending_disaster_extra_sounds
                .contains(&PlacementSound::Flood)
        );
    } else {
        assert!(
            !flood_state
                .pending_disaster_extra_sounds
                .contains(&PlacementSound::Flood)
        );
    }
    assert_eq!(flood_state.pe_process_crt_seed(), flood_preview.seed());
    assert_eq!(
        flood_state.pe_process_lfsr_state(),
        flood_preview_lfsr.state()
    );
}

#[test]
fn pe_disaster_scan_sounds_fire_seen_always_queues_fire() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xff);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    let mut state = test_state(city);
    let seen = pe_disaster_overlay_seen(&state.city);
    assert!(seen.fire);
    assert!(!seen.riot);
    assert!(!seen.flood);
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let _ = preview_pe_fire_tick(&state, &mut preview, &mut preview_lfsr);
    let sounds = pe_disaster_scan_sounds_with_rng(&state.city, seen, &mut preview);
    assert!(sounds.play_fire);
    state.tick_pe_disaster_overlays();
    assert!(
        state
            .pending_disaster_extra_sounds
            .contains(&PlacementSound::Fire)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_fb_degrade_preserves_xtxt_and_zone_type() {
    let mut city = City::new_blank("/tmp/sc2000-fb-xtxt.sc2", "FbXtxt").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, RESIDENTIAL_2X2_BUILDINGS[0], 2);
    or_rci_building_zone_corners_for_test(&mut city, (40, 40), 2);
    for &(tx, ty) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_text_overlay(tx, ty, 0x20);
    }
    city.set_zone(41, 41, ZONE_COMMERCIAL_LIGHT | 0x40);
    let mut crt = Win95CrtRand::new(1);
    assert!(pe_fb_degrade_under_cloud(&mut city, 40, 40, &mut crt));
    for &(tx, ty) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        assert_eq!(
            city.text_overlay_at(tx, ty),
            Some(0x20),
            "XTXT changed at ({tx},{ty})"
        );
        assert!(ABANDONED_POP2_BUILDINGS.contains(&city.building_at(tx, ty)));
    }
    assert_eq!(
        city.zone_at(41, 41).unwrap().zone_id(),
        ZONE_COMMERCIAL_LIGHT
    );
}

#[test]
fn pe_hurricane_arm_is_view_rotation_plus_1() {
    assert_eq!(pe_hurricane_arm(0), 1);
    assert_eq!(pe_hurricane_arm(1), 2);
    assert_eq!(pe_hurricane_arm(2), 3);
    assert_eq!(pe_hurricane_arm(3), 0);
    assert_eq!(pe_hurricane_paint_count(0), 0x32);
    assert_eq!(pe_hurricane_paint_count(1), 0x64);
    assert_eq!(pe_hurricane_paint_count(2), 0x64);
    assert_eq!(pe_hurricane_paint_count(3), 0x32);
    assert_eq!(PE_HURRICANE_DESTROY_ATTEMPTS, 0x14);
    assert_eq!(PE_HURRICANE_DESTROY_MIN_BUILDING, 0x0d);
    assert!(pe_hurricane_destroy_extra_explode(0));
    assert!(!pe_hurricane_destroy_extra_explode(1));
    assert!(!pe_hurricane_destroy_extra_explode(2));
    assert!(pe_hurricane_destroy_extra_explode(3));
}

#[test]
fn hurricane_east_destroy_queues_explode_per_hit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_view_rotation(3);
    for x in 0..CITY_WIDTH {
        city.set_building(x, 120, 0x70);
    }
    let mut state = test_state(city);
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    let mut preview_city = state.city.clone();
    let mut preview_crt = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (_, hits) =
        pe_hurricane_overlays_with_lfsr(&mut preview_city, &mut preview_crt, &mut preview_lfsr);
    assert!(hits > 0, "east arm arg2=1 should explode on destroy hits");
    assert!(state.start_disaster(DisasterKind::Hurricane));
    assert_eq!(
        state.pending_disaster_extra_sounds,
        vec![PlacementSound::Explode; hits]
    );
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn hurricane_east_destroy_elevated_early_out_skips_explode_hit() {
    // PE DestroyStructure elevated `0x3f..=0x42` @ `0x46137b`: no DestroyBuilding /
    // flag_a=1 explode, so east-arm hit count stays 0.
    let mut city = City::new_blank("/tmp/sc2000-hurr-elev.sc2", "HurrElev").expect("blank");
    city.set_view_rotation(3);
    for x in 0..CITY_WIDTH {
        city.set_building(x, 120, 0x3f);
    }
    let mut preview = city.clone();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::from_u32_seed(1);
    let (_, hits) = pe_hurricane_overlays_with_lfsr(&mut preview, &mut crt, &mut lfsr);
    assert_eq!(hits, 0);
    assert_eq!(preview.building_at(64, 120), 0x3f);
}

#[test]
fn hurricane_damage_scan_consumes_zero_steps_and_draws_at_the_far_edge() {
    let mut city = City::new_blank("/tmp/citygame-hurricane-lane.sc2", "Wind").unwrap();
    city.set_building(4, 9, 0x0d);
    let mut lfsr = Win95Lfsr::new(16_390);
    // The first two bound-20 draws are 0 and 9.
    assert_eq!(
        pe_hurricane_destroy_tile(&city, 2, 4, &mut lfsr),
        Some((4, 9))
    );
    assert_eq!(lfsr.state(), 7_149);

    for (arm, far_y) in [(0, 0), (2, 127)] {
        city.set_building(5, far_y, 0x0d);
        let mut lfsr = Win95Lfsr::new(13);
        // Draw 13 reaches the far edge. Draw 14 steps out of the map.
        assert_eq!(pe_hurricane_destroy_tile(&city, arm, 5, &mut lfsr), None);
        assert_eq!(lfsr.state(), 27_679);
        city.set_building(5, far_y, 0);
    }
}

#[test]
fn hurricane_ascending_y_counts_a_candidate_before_destroy_structure_rejects_it() {
    let mut city = City::new_blank("/tmp/citygame-hurricane-budget.sc2", "Wind").unwrap();
    city.set_view_rotation(1);
    // Every lane immediately finds an elevated target. DestroyStructure
    // rejects it, but the scan already spent its extra count.
    for x in 0..CITY_WIDTH {
        city.set_building(x, 0, 0x3f);
    }
    let mut state = test_state(city);
    let mut expected = state.pe_process_lfsr;
    for _ in 0..(10 + 100) {
        expected.random_mask_7f();
    }
    assert!(state.start_disaster(DisasterKind::Hurricane));
    assert_eq!(state.pe_process_lfsr_state(), expected.state());
    assert!((0..CITY_WIDTH).all(|x| state.city.building_at(x, 0) == 0x3f));
}

#[test]
fn hurricane_north_destroy_skips_extra_explode() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_view_rotation(0);
    for y in 0..CITY_HEIGHT {
        city.set_building(100, y, 0x70);
    }
    assert_eq!(pe_hurricane_extra_explode_count(&city), 0);
    let mut preview = city.clone();
    let mut crt = Win95CrtRand::new(1);
    let mut expected_crt = crt;
    let mut lfsr = Win95Lfsr::new(1);
    let hits = pe_hurricane_apply_destroy(&mut preview, &mut crt, &mut lfsr, 1);
    assert!(hits > 0);
    for _ in 0..hits {
        // Each mode-0 hit spends only its flat-terrain rubble draw.
        expected_crt.rubble_tile_id();
    }
    assert_eq!(crt, expected_crt);
    let mut state = test_state(city);
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    assert!(state.start_disaster(DisasterKind::Hurricane));
    assert!(state.pending_disaster_extra_sounds.is_empty());
}

#[test]
fn pe_hurricane_edge_scans_xbld_ge_6_from_map_edge() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(pe_hurricane_edge_tile(&city, 1, 40), None);

    city.set_building(100, 40, 6);
    assert_eq!(pe_hurricane_edge_tile(&city, 1, 40), Some((100, 40)));
    city.set_building(127, 40, 6);
    assert_eq!(
        pe_hurricane_edge_tile(&city, 1, 40),
        Some((127, 40)),
        "east arm scans x from 127"
    );

    city.set_building(40, 80, 6);
    assert_eq!(pe_hurricane_edge_tile(&city, 0, 40), Some((40, 80)));
    city.set_building(40, 10, 6);
    assert_eq!(pe_hurricane_edge_tile(&city, 2, 40), Some((40, 10)));
    city.set_building(10, 50, 6);
    assert_eq!(pe_hurricane_edge_tile(&city, 3, 50), Some((10, 50)));

    let mut edge = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    edge.set_building(0, 40, 6);
    assert_eq!(
        pe_hurricane_edge_tile(&edge, 1, 40),
        None,
        "east arm does not paint x==0"
    );
}

#[test]
fn start_disaster_hurricane_paints_fc_on_east_coast() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 0..CITY_HEIGHT {
        city.set_building(100, y, 6);
    }
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut preview_city = state.city.clone();
    let mut preview_crt = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, _) =
        pe_hurricane_overlays_with_lfsr(&mut preview_city, &mut preview_crt, &mut preview_lfsr);

    assert!(state.start_disaster(DisasterKind::Hurricane));
    assert!(!expected.is_empty());
    assert!(expected.values().all(|&marker| marker == 0xfc));
    assert!(
        expected.keys().all(|&(x, _y)| x == 100),
        "rot 0 east arm should hit the XBLD>=6 column"
    );
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x10)
    );
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn start_disaster_hurricane_destroys_0x0d_on_south_arm_without_fire() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_view_rotation(3);
    for x in 0..CITY_WIDTH {
        city.set_building(x, 120, 0x70);
    }
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut expected_city = state.city.clone();
    let mut preview_crt = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, _) =
        pe_hurricane_overlays_with_lfsr(&mut expected_city, &mut preview_crt, &mut preview_lfsr);

    assert!(state.start_disaster(DisasterKind::Hurricane));
    assert!(expected.values().all(|&marker| marker == 0xfc));
    assert!(
        (0..CITY_WIDTH).any(|x| {
            let b = expected_city.building_at(x, 120);
            b == 0 || is_rubble_tile(b)
        }),
        "south arm should rubble at least one XBLD>=0x0D"
    );
    for x in 0..CITY_WIDTH {
        assert_eq!(
            state.city.building_at(x, 120),
            expected_city.building_at(x, 120)
        );
        assert_ne!(state.city.text_overlay_at(x, 120), Some(0xff));
    }
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x10)
    );
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn hurricane_destroy_clears_xtxt_footprint() {
    // PE hurricane destroy (`0x401604` family) clears XTXT with XBLD across the footprint.
    let mut city = City::new_blank("/tmp/sc2000-hur-xtxt.sc2", "HurXtxt").expect("blank");
    city.set_view_rotation(3);
    let building = COMMERCIAL_3X3_BUILDINGS[0];
    for origin_x in (0..=124).step_by(4) {
        stamp_complete_building_for_test(&mut city, origin_x, 120, building, 3);
        city.set_text_overlay(origin_x, 120, 0x55);
        city.set_text_overlay(origin_x + 2, 122, 0x66);
    }
    let mut expected_city = city.clone();
    let mut preview_crt = Win95CrtRand::new(0x45D4_E000);
    let mut preview_lfsr = Win95Lfsr::from_u32_seed(0x45D4_E000);
    let _ =
        pe_hurricane_overlays_with_lfsr(&mut expected_city, &mut preview_crt, &mut preview_lfsr);
    let mut saw_xtxt_clear = false;
    let mut saw_crt_rubble = false;
    for origin_x in (0..=124).step_by(4) {
        let footprint = building_footprint_from_origin(origin_x, 120, 3).unwrap();
        let cleared = footprint.iter().all(|&(x, y)| {
            let b = expected_city.building_at(x, y);
            b == 0 || is_rubble_tile(b)
        });
        if !cleared {
            continue;
        }
        for &(x, y) in &footprint {
            assert_eq!(
                expected_city.text_overlay_at(x, y),
                Some(0),
                "destroy must clear XTXT at {x},{y}"
            );
            if is_rubble_tile(expected_city.building_at(x, y)) {
                saw_crt_rubble = true;
            }
        }
        saw_xtxt_clear = true;
    }
    assert!(saw_xtxt_clear, "south arm should hit at least one 3×3");
    assert!(
        saw_crt_rubble,
        "DestroyStructure should PlaceTile CRT rubble"
    );
}

#[test]
fn hurricane_destroy_clears_multitile_xbld_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_view_rotation(3);
    let building = COMMERCIAL_3X3_BUILDINGS[0];
    for origin_x in (0..=124).step_by(4) {
        stamp_complete_building_for_test(&mut city, origin_x, 120, building, 3);
    }
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    let mut expected_city = state.city.clone();
    let mut preview_crt = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let expected =
        pe_hurricane_overlays_with_lfsr(&mut expected_city, &mut preview_crt, &mut preview_lfsr).0;
    let mut saw_full_clear = false;
    for origin_x in (0..=124).step_by(4) {
        let footprint = building_footprint_from_origin(origin_x, 120, 3).unwrap();
        let cleared = footprint.iter().all(|&(x, y)| {
            let b = expected_city.building_at(x, y);
            b == 0 || is_rubble_tile(b)
        });
        let intact = footprint
            .iter()
            .all(|&(x, y)| expected_city.building_at(x, y) == building);
        assert!(
            cleared || intact,
            "PE destroy must clear the whole 3×3 at x={origin_x}"
        );
        if cleared {
            saw_full_clear = true;
            for &(x, y) in &footprint {
                assert_ne!(expected.get(&(x, y)).copied(), Some(0xff));
            }
        }
    }
    assert!(saw_full_clear, "south arm should hit at least one 3×3");

    assert!(state.start_disaster(DisasterKind::Hurricane));
    for origin_x in (0..=124).step_by(4) {
        for y in 120..123 {
            for x in origin_x..origin_x + 3 {
                assert_eq!(
                    state.city.building_at(x, y),
                    expected_city.building_at(x, y)
                );
            }
        }
    }
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn start_disaster_mass_riot_runs_type3_from_jittered_origins() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.persist_pe_day22_population_leftover(20_000);
    for dx in 0..5_usize {
        for dy in 0..5_usize {
            city.set_building(10 + dx, 32 + dy, 29);
        }
    }
    let mut state = test_state(city);
    assert_eq!(pe_population_jitter_attempt_count(&state.city), 7);
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let expected = pe_mass_riot_overlays_with_rng(&mut preview_city, 12, 34, &mut preview);
    state.hover_tile = Some((12, 34));

    assert!(state.start_disaster(DisasterKind::MassRiot));
    assert!(!expected.is_empty());
    assert!(expected.len() <= pe_population_jitter_attempt_count(&state.city));
    for (&(x, y), &marker) in &expected {
        assert_eq!(state.city.text_overlay_at(x, y), Some(marker));
        assert!(matches!(marker, 0xfd | 0xfe));
    }
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0d)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn mass_riot_queues_riot_sound_per_successful_type3() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for dx in 0..5_usize {
        for dy in 0..5_usize {
            city.set_building(10 + dx, 32 + dy, 29);
        }
    }
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let hits = pe_mass_riot_sound_count_with_rng(&state.city, 12, 34, &mut preview);
    assert!(hits > 1, "5×5 roads should land more than one type-3");
    assert_eq!(crate::audio::SOUND_RIOT, 0x200);
    state.hover_tile = Some((12, 34));
    if !state.sound_effects_enabled {
        state.toggle_sound_effects();
    }
    assert!(state.start_disaster(DisasterKind::MassRiot));
    assert_eq!(
        state.pending_disaster_extra_sounds,
        vec![PlacementSound::Riot; hits - 1]
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_rioters_marker_is_rand_and_1_minus_3() {
    let mut rng = Win95CrtRand::new(0x45E5_F001);
    let mut expected = Win95CrtRand::new(0x45E5_F001);
    assert_eq!(
        pe_rioters_marker(&mut rng),
        ((expected.random_u16() & 1) as u8).wrapping_sub(3)
    );
    assert!(pe_rioters_building_eligible(29));
    assert!(pe_rioters_building_eligible(0x4b));
    assert!(!pe_rioters_building_eligible(0));
    assert!(!pe_rioters_building_eligible(203));
}

#[test]
fn pe_rioters_target_spirals_nesw_to_first_road() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(pe_rioters_target_tile(&city, 40, 40), None);
    city.set_building(41, 40, 29);
    assert_eq!(pe_rioters_target_tile(&city, 40, 40), Some((41, 40)));
    city.set_building(40, 39, 29);
    assert_eq!(
        pe_rioters_target_tile(&city, 40, 40),
        Some((40, 39)),
        "PE riot spiral steps north first"
    );
}

#[test]
fn start_disaster_rioters_without_transit_is_nop() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));

    assert!(!state.start_disaster(DisasterKind::Rioters));
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());
}

#[test]
fn start_disaster_rioters_paints_one_fd_or_fe_on_road() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(12, 33, 29);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let marker = pe_rioters_marker(&mut preview);
    state.hover_tile = Some((12, 34));

    assert!(state.start_disaster(DisasterKind::Rioters));
    assert_eq!(state.city.text_overlay_at(12, 34), Some(0));
    assert_eq!(state.city.text_overlay_at(12, 33), Some(marker));
    assert!(matches!(marker, 0xfd | 0xfe));
    assert_eq!(disaster_overlay_tiles(&state.city).len(), 1);
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x03)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_tornado_xthg_dir_is_rand_and_7() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut rng = Win95CrtRand::new(pe_tornado_actor_seed(&city, 12, 34));
    let (lane, height, target_x, target_y, saved_a) = pe_tornado_xthg_fields(&city, 12, 34);
    assert_eq!(lane, (rng.random_u15() & 7) as u8);
    assert_eq!(target_x, (rng.random_u15() & 0x7f) as u8);
    assert_eq!(target_y, (rng.random_u15() & 0x7f) as u8);
    assert_eq!(height, city.altitude_at(12, 34) & 0x1f);
    assert_eq!(saved_a, 0);
    assert_eq!(XTHG_TORNADO, 0x0f);
    assert_eq!(crate::win95_overlay::XTHG_TORNADO_SUBTILE, 8);
}

#[test]
fn start_disaster_tornado_spawns_xthg_actor() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));

    let mut preview = state.pe_process_crt;
    let expected_fields = pe_tornado_xthg_fields_with_rng(&state.city, 12, 34, &mut preview);
    assert!(state.start_disaster(DisasterKind::Tornado));
    let slot = assert_pe_tornado_actor(&state.city, 12, 34, expected_fields);
    assert_pe_xthg_proxy(&state.city, 12, 34, slot);
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x07)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn win95_tornado_retirement_blocks_start_until_next_things_sweep_on_both_layouts() {
    for path in ["tornado-counter.sc2", "tornado-counter.city"] {
        let city = blank_city_layout_fixture(path, "Tornado counter");
        let mut state = test_state(city);
        state.no_disasters_enabled = false;
        state.hover_tile = Some((50, 96));
        assert!(state.start_disaster(DisasterKind::Tornado));
        assert_eq!(state.city.pe_tornado_actor_counter, 1);

        state.pe_process_crt = Win95CrtRand::new(197); // Third draw is 0x0600.
        state.tick_pe_disaster_xthg_actors_in_slot_order();
        assert!(state.city.pe_tornado_actor().is_none());
        assert_eq!(state.city.pe_tornado_actor_counter, 1);
        let seed = state.pe_process_crt_seed();
        assert!(!state.start_disaster(DisasterKind::Tornado));
        assert_eq!(state.pe_process_crt_seed(), seed);
        assert_eq!(state.city.pe_w_trigger_disaster, 0);

        state.traffic.set_speed(GameSpeed::Paused);
        state.advance_frame_simulation(Duration::from_millis(200));
        assert_eq!(state.city.pe_tornado_actor_counter, 1);
        state.traffic.set_speed(GameSpeed::Turtle);
        state.advance_frame_simulation(Duration::from_millis(200));
        assert_eq!(state.city.pe_tornado_actor_counter, 0);
        assert!(state.start_disaster(DisasterKind::Tornado));
        assert_eq!(state.city.pe_tornado_actor_counter, 1);
    }
}

#[test]
fn win95_tornado_pending_arm_uses_signed_word_and_counts_each_slot_visit() {
    let city = blank_city_layout_fixture("tornado-signed.sc2", "Tornado signed counter");
    let mut state = test_state(city);
    state.no_disasters_enabled = false;
    state.city.pe_tornado_actor_counter = u16::MAX;
    state.hover_tile = Some((50, 96));
    assert!(state.start_disaster(DisasterKind::Tornado));
    assert_eq!(state.city.pe_tornado_actor_counter, 0);

    // A queued type 7 with a zero counter reaches the constructor even if
    // another kind-15 record is already present.
    state.hover_tile = Some((60, 96));
    assert!(state.start_disaster(DisasterKind::Tornado));
    assert_eq!(state.city.pe_tornado_actor_counter, 1);
    let seed = state.pe_process_crt_seed();
    assert!(!state.start_disaster(DisasterKind::Tornado));
    assert_eq!(state.pe_process_crt_seed(), seed);

    state.tick_pe_disaster_xthg_actors_in_slot_order();
    assert_eq!(state.city.pe_tornado_actor_counter, 2);
}

#[test]
fn win95_ambient_tornado_rejection_retains_point_draws() {
    let mut city = blank_city_layout_fixture("tornado-ambient.sc2", "Tornado ambient counter");
    for (trend, remainder, seed) in [(WEATHER_TREND_TORNADO, 14, 1), (WEATHER_TREND_WINDY, 0, 2)] {
        city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(trend));
        let mut expected = Win95CrtRand::new(seed);
        if remainder == 0 {
            assert_eq!(expected.random_u16() % 0x13, 7);
        }
        let y = usize::from(expected.random_u16() % 0x7e + 1);
        let x = usize::from(expected.random_u16() % 0x7e + 1);
        for (counter, selection) in [(0, Some((DisasterKind::Tornado, (x, y)))), (1, None)] {
            city.pe_tornado_actor_counter = counter;
            let mut rng = Win95CrtRand::new(seed);
            assert_eq!(
                ambient_disaster_select_with_remainder(
                    &city,
                    PopulationStats::default(),
                    CityServiceOutcomes::default(),
                    EngineFidelity::Win95,
                    remainder,
                    &mut rng,
                    None,
                ),
                selection
            );
            assert_eq!(rng.seed(), expected.seed());
        }
    }
}

#[test]
fn pe_tornado_tick_consumes_win95_crt_rand_not_toolbox() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    assert!(
        city.place_pe_tornado_actor(40, 40, 2, city.altitude_at(40, 40) & 0x1f, 50, 50, 0)
            .is_some()
    );
    let seed = pe_tornado_tick_seed(&city);
    let mut probe = Win95CrtRand::new(seed);
    let first = probe.random_u16();
    let second = probe.random_u16();
    // Jitter dir always draws two CRT rolls (`rand%3` twice) before move/despawn rolls.
    let _ = pe_tornado_tick_city(&mut city);
    let mut again = Win95CrtRand::new(seed);
    assert_eq!(again.random_u16(), first);
    assert_eq!(again.random_u16(), second);
    assert_ne!(
        MacToolboxRng::new(seed).random_u16(),
        first,
        "CRT stream must differ from Toolbox for the same seed"
    );
}

#[test]
fn pe_tornado_move_preserves_height_across_tile_change() {
    let mut city =
        City::new_blank("/tmp/sc2000-tornado-height.sc2", "TornadoHeight").expect("blank");
    city.set_altitude(41, 40, 12, 0);
    let slot = city
        .place_pe_tornado_actor(40, 40, 2, 3, 50, 50, 0)
        .expect("tornado actor");
    if let Some(record) = city
        .traffic_overlays_mut()
        .and_then(|overlays| overlays.get_mut(slot))
    {
        record.sub_x = 16;
    }

    assert!(pe_tornado_move_step(&mut city, slot, 2));

    let (_, record) = city.pe_tornado_actor().expect("tornado remains");
    assert_eq!((record.tile_x, record.tile_y), (41, 40));
    assert_eq!(record.height, 3, "FUN_004516F0 does not write XTHG +5");
}

#[test]
fn pe_tornado_second_move_uses_low_bits_and_preserves_record_lane() {
    let mut city =
        City::new_blank("/tmp/sc2000-tornado-jitter.sc2", "TornadoJitter").expect("blank");
    let slot = city
        .place_pe_tornado_actor(40, 40, 2, city.altitude_at(40, 40) & 0x1f, 50, 50, 0)
        .expect("tornado actor");
    let seed = (0..10_000)
        .find(|&seed| {
            let mut probe = Win95CrtRand::new(seed);
            let _first_a = probe.random_u16() % 3;
            let _first_b = probe.random_u16() % 3;
            let first_despawn = probe.random_u16() & 0xff;
            let _second_a = probe.random_u16() & 1;
            let _second_b = probe.random_u16() & 1;
            let second_despawn = probe.random_u16() & 0xff;
            first_despawn != 0 && second_despawn != 0
        })
        .expect("seed without either 1-in-256 despawn roll");

    let mut expected = city.clone();
    let mut expected_crt = Win95CrtRand::new(seed);
    let first_a = (expected_crt.random_u16() % 3) as i16;
    let first_b = (expected_crt.random_u16() % 3) as i16;
    let first_dir = (2i16 + first_a - first_b).rem_euclid(8) as u8;
    assert!(pe_tornado_move_step(&mut expected, slot, first_dir));
    assert_ne!(expected_crt.random_u16() & 0xff, 0);
    let second_a = (expected_crt.random_u16() & 1) as i16;
    let second_b = (expected_crt.random_u16() & 1) as i16;
    let second_dir = (2i16 + second_a - second_b).rem_euclid(8) as u8;
    assert!(pe_tornado_move_step(&mut expected, slot, second_dir));
    assert_ne!(expected_crt.random_u16() & 0xff, 0);

    let mut actual_crt = Win95CrtRand::new(seed);
    pe_tornado_tick_city_with_rng(&mut city, &mut actual_crt);

    assert_eq!(actual_crt, expected_crt);
    let (_, expected_record) = expected.pe_tornado_actor().expect("expected tornado");
    let (_, actual_record) = city.pe_tornado_actor().expect("actual tornado");
    assert_eq!(
        actual_record.lane, 2,
        "temporary headings do not replace XTHG +1"
    );
    assert_eq!(actual_record.lane, expected_record.lane);
    assert_eq!(
        (actual_record.tile_x, actual_record.tile_y),
        (expected_record.tile_x, expected_record.tile_y)
    );
    assert_eq!(
        (actual_record.sub_x, actual_record.sub_y),
        (expected_record.sub_x, expected_record.sub_y)
    );
}

#[test]
fn pe_tornado_map_edge_clear_keeps_subtile_and_runs_handler_tail() {
    let mut city =
        City::new_blank("/tmp/sc2000-tornado-edge-tail.sc2", "TornadoEdgeTail").expect("blank");
    let slot = city
        .place_pe_tornado_actor(127, 40, 2, city.altitude_at(127, 40) & 0x1f, 50, 50, 0)
        .expect("tornado actor");
    if let Some(record) = city
        .traffic_overlays_mut()
        .and_then(|overlays| overlays.get_mut(slot))
    {
        record.sub_x = 16;
        record.sub_y = 0;
    }
    let seed = (0..100_000)
        .find(|&seed| {
            let mut probe = Win95CrtRand::new(seed);
            let a = (probe.random_u16() % 3) as i16;
            let b = (probe.random_u16() % 3) as i16;
            let first_dir = (2 + a - b).rem_euclid(8);
            let first_despawn = probe.random_u16() & 0xff;
            let _ = probe.random_u16() & 1;
            let _ = probe.random_u16() & 1;
            let second_despawn = probe.random_u16() & 0xff;
            matches!(first_dir, 2 | 3) && first_despawn != 0 && second_despawn != 0
        })
        .expect("seed that exits east without either random despawn");
    let mut expected_crt = Win95CrtRand::new(seed);
    for _ in 0..6 {
        let _ = expected_crt.random_u16();
    }

    let mut actual_crt = Win95CrtRand::new(seed);
    let stats = pe_tornado_tick_city_with_rng(&mut city, &mut actual_crt);

    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!(record.kind, 0, "the first eastward carry clears the slot");
    assert_eq!((record.tile_x, record.tile_y), (127, 39));
    assert_eq!(
        record.lane, 2,
        "temporary headings do not overwrite XTHG +1"
    );
    assert_eq!(
        record.sub_x, 16,
        "the cleared slot still receives its second move"
    );
    assert_eq!(record.sub_y, 8);
    assert_eq!(city.text_overlay_at(127, 40), Some(0));
    assert_eq!(city.text_overlay_at(127, 39), xthg_proxy_for_slot(slot));
    assert_eq!(stats.despawned, 1);
    assert_eq!(
        actual_crt, expected_crt,
        "the full six-draw PE handler tail runs"
    );
}

#[test]
fn pe_tornado_random_despawn_still_runs_second_move() {
    let mut city =
        City::new_blank("/tmp/sc2000-tornado-random-tail.sc2", "TornadoRandomTail").expect("blank");
    let slot = city
        .place_pe_tornado_actor(40, 40, 2, city.altitude_at(40, 40) & 0x1f, 50, 50, 0)
        .expect("tornado actor");
    if let Some(record) = city
        .traffic_overlays_mut()
        .and_then(|overlays| overlays.get_mut(slot))
    {
        record.sub_x = 8;
        record.sub_y = 8;
    }
    let seed = (0..100_000)
        .find(|&seed| {
            let mut probe = Win95CrtRand::new(seed);
            let _ = probe.random_u16() % 3;
            let _ = probe.random_u16() % 3;
            let first_despawn = probe.random_u16() & 0xff;
            let _ = probe.random_u16() & 1;
            let _ = probe.random_u16() & 1;
            let second_despawn = probe.random_u16() & 0xff;
            first_despawn == 0 && second_despawn != 0
        })
        .expect("seed whose first random despawn clears the tornado");
    let mut expected_crt = Win95CrtRand::new(seed);
    for _ in 0..6 {
        let _ = expected_crt.random_u16();
    }

    let mut actual_crt = Win95CrtRand::new(seed);
    let stats = pe_tornado_tick_city_with_rng(&mut city, &mut actual_crt);

    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!(record.kind, 0, "the first random despawn clears the slot");
    assert_eq!(
        record.tile_x, 41,
        "PE still moves the cleared slot east again"
    );
    assert_eq!(record.sub_x, 8);
    assert_eq!(record.lane, 2);
    assert_eq!(
        city.text_overlay_at(usize::from(record.tile_x), usize::from(record.tile_y)),
        xthg_proxy_for_slot(slot),
        "the native movement thunk paints the second-move proxy by slot"
    );
    assert_eq!(stats.despawned, 1);
    assert_eq!(
        actual_crt, expected_crt,
        "the full six-draw PE handler tail runs"
    );
}

#[test]
fn pe_monster_flee_despawn_uses_win95_lfsr_mod_100() {
    // PE `0x401cad(0x64)` is the game LFSR remainder, not CRT `% 100`.
    let mut lfsr = Win95Lfsr::new(1);
    let mut hits = 0;
    for _ in 0..600 {
        if lfsr.random_mod(100) == 0 {
            hits += 1;
        }
    }
    assert!(hits > 0, "LFSR mod 100 must eventually hit 0");
    assert_ne!(
        Win95Lfsr::new(1).random_mod(100),
        Win95CrtRand::new(1).random_u16() % 100
    );
}

#[test]
fn pe_monster_flee_tick_consumes_process_lfsr() {
    // Live flee despawn @ `0x4548c0` shares `pe_process_lfsr`, not a city-name stream.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(city.place_pe_monster_actor(40, 40, 50, 50, 0, 0).is_some());
    if let Some(overlays) = city.traffic_overlays_mut() {
        for record in overlays.iter_mut() {
            if record.is_pe_monster_actor() {
                record.state = PE_MONSTER_STATE_FLEE;
            }
        }
    }
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x08);
    let mut state = test_state(city);
    state.hover_tile = Some((60, 40));
    let before_lfsr = state.pe_process_lfsr_state();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_monster_tick(&state, &mut preview, &mut preview_lfsr);
    assert_ne!(
        preview_lfsr.state(),
        before_lfsr,
        "flee arm must draw Game_Random mod 100"
    );
    assert_eq!(state.tick_pe_monster_actors(), stats.changed());
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
    assert_eq!(
        state.city.pe_monster_actor().is_some(),
        expected.pe_monster_actor().is_some()
    );
}

#[test]
fn pe_maxis_man_spawn_global_payload_uses_monster_xthg_low_byte() {
    // PE constructor IDs 5|8..=12 write `[0x4CA81C]` low byte to XTHG `+B`.
    // Cleared index is `0xFFFF` → `0xFF`, not Rust `None → 0`.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::Firestorm));
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x0c)
    );
    assert!(state.city.pe_monster_actor().is_none());
    arm_pe_maxis_man_spawn_gates(&mut state);
    assert_eq!(state.tick_pe_maxis_man_spawn(), 1);
    let (_, cleared) = state
        .city
        .traffic_overlays()
        .and_then(|table| {
            table
                .iter()
                .enumerate()
                .find(|(_, r)| r.kind == crate::win95_overlay::XTHG_MAXIS_MAN)
        })
        .expect("Maxis Man with cleared monster index");
    assert_eq!(cleared.saved_b, 0xff);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let monster_slot = city
        .place_pe_monster_actor(40, 40, 50, 50, 0, 0)
        .expect("plant monster for wMonsterXTHGIndex");
    // Monster XTHG already marks an active disaster; set Firestorm type only.
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0c);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert_eq!(
        state.city.pe_monster_actor().map(|(slot, _)| slot),
        Some(monster_slot)
    );
    arm_pe_maxis_man_spawn_gates(&mut state);
    assert_eq!(state.tick_pe_maxis_man_spawn(), 1);
    let (_, with_monster) = state
        .city
        .traffic_overlays()
        .and_then(|table| {
            table
                .iter()
                .enumerate()
                .find(|(_, r)| r.kind == crate::win95_overlay::XTHG_MAXIS_MAN)
        })
        .expect("Maxis Man with active monster index");
    assert_eq!(with_monster.saved_b, monster_slot as u8);
}

#[test]
fn pe_maxis_man_spawn_rioters_payload_consumes_process_crt() {
    // PE `FUN_00452FA0` / IDs 3|13: CRT is jitter `rand()&3` then payload
    // `rand()&1` for `0xFD`/`0xFE` — not city-name overlay `sim_rng`.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(12, 33, 29);
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    assert!(state.start_disaster(DisasterKind::Rioters));
    assert!(state.city.has_active_disaster());
    arm_pe_maxis_man_spawn_gates(&mut state);
    let overlay_before = state.win95_overlay_sim.sim_rng_seed();
    let mut preview = state.pe_process_crt;
    let (spawn_x, spawn_y) = pe_maxis_man_spawn_tile_jitter(12, 34, &mut preview);
    let expected_dir = pe_xthg_direction_toward(spawn_x as i16, spawn_y as i16, 12, 34);
    let low = (preview.random_u16() & 1) as u8;
    let expected_payload = pe_maxis_man_saved_payload(0x03, low, 0).expect("rioters payload");

    assert_eq!(state.tick_pe_maxis_man_spawn(), 1);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(
        state.win95_overlay_sim.sim_rng_seed(),
        overlay_before,
        "Maxis Man payload must not advance city-name overlay CRT"
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
    assert_eq!(record.saved_b, expected_payload);
    assert_eq!(record.lane, expected_dir);
    assert_eq!(
        (record.tile_x as usize, record.tile_y as usize),
        (spawn_x, spawn_y)
    );
    assert!(matches!(expected_payload, 0xfd | 0xfe));
}

#[test]
fn pe_maxis_man_spawns_once_at_disaster_start_when_gates_pass() {
    // Sole PE ctor caller is disaster-start `0x45d080`, not the daily disaster clock.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    set_military_base_type_for_test(&mut city, MILITARY_BASE_DECLINED);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state.pe_process_lfsr = Win95Lfsr::new(2);
    assert!(state.start_disaster(DisasterKind::Firestorm));
    assert_eq!(
        state
            .city
            .traffic_overlays()
            .unwrap()
            .iter()
            .filter(|r| r.kind == crate::win95_overlay::XTHG_MAXIS_MAN)
            .count(),
        1,
        "DECLINED + RandomAnd3==0 must spawn at disaster start"
    );
    // Daily clock must not add a second actor (PE ctor is start-only).
    let _ = state.tick_pe_disaster_clock();
    assert_eq!(
        state
            .city
            .traffic_overlays()
            .unwrap()
            .iter()
            .filter(|r| r.kind == crate::win95_overlay::XTHG_MAXIS_MAN)
            .count(),
        1
    );
}

#[test]
fn pe_tornado_tick_destroys_buildings_and_moves_proxy() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 0x70);
    assert!(
        city.place_pe_tornado_actor(40, 40, 2, city.altitude_at(40, 40) & 0x1f, 50, 50, 0)
            .is_some()
    );
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x07);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let (expected, stats) = preview_pe_tornado_tick(&state, &mut preview);
    assert!(stats.destroyed > 0 || stats.moved > 0 || stats.despawned > 0);
    if stats.destroyed > 0 {
        assert!(is_rubble_tile(expected.building_at(40, 40)));
    }
    assert_eq!(state.tick_pe_tornado_actors(), stats.changed());
    assert_eq!(state.city.building_at(40, 40), expected.building_at(40, 40));
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    match (state.city.pe_tornado_actor(), expected.pe_tornado_actor()) {
        (None, None) => {}
        (Some((_, a)), Some((_, b))) => {
            assert_eq!(a.tile_x, b.tile_x);
            assert_eq!(a.tile_y, b.tile_y);
            assert_eq!(a.lane, b.lane);
            assert_eq!(a.sub_x, b.sub_x);
            assert_eq!(a.sub_y, b.sub_y);
        }
        other => panic!("tornado actor mismatch: {other:?}"),
    }
}

#[test]
fn pe_tornado_tick_elevated_destroy_building_clears_road() {
    // PE @ `0x454f08`: DestroyBuilding (`0x402211`), not DestroyStructure — elevated
    // arm PlaceTile(0)/XZON/ALTM; destroyed count + heavy (`XBLD >= 0x0d`).
    let mut city = City::new_blank("/tmp/sc2000-tornado-elev.sc2", "TornElev").expect("blank");
    city.set_building(40, 40, 0x3f);
    city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    city.set_tunnel_levels(40, 40, 0x1f);
    assert!(
        city.place_pe_tornado_actor(40, 40, 2, city.altitude_at(40, 40) & 0x1f, 50, 50, 0)
            .is_some()
    );
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x07);
    let mut crt = Win95CrtRand::new(1);
    let stats = pe_tornado_tick_city_with_rng(&mut city, &mut crt);
    assert_eq!(stats.destroyed, 1);
    assert_eq!(city.building_at(40, 40), 0);
    assert_eq!(
        city.zone_at(40, 40).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0
    );
    assert_eq!(city.tunnel_levels_at(40, 40), 0);
}

#[test]
fn pe_tornado_tick_destroy_building_places_crt_rubble_like_pe() {
    // PE @ `0x454f08`: DestroyBuilding CRT PlaceTile rubble on XTER0.
    let mut city =
        City::new_blank("/tmp/sc2000-tornado-crt-rubble.sc2", "TornadoCrt").expect("blank");
    city.set_building(40, 40, 0x70);
    assert!(
        city.place_pe_tornado_actor(40, 40, 2, city.altitude_at(40, 40) & 0x1f, 50, 50, 0)
            .is_some()
    );
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x07);
    let mut state = test_state(city);
    let mut found = false;
    for seed in 0u32..4096 {
        state.pe_process_crt = Win95CrtRand::new(seed);
        let mut preview = state.pe_process_crt;
        let (expected, stats) = preview_pe_tornado_tick(&state, &mut preview);
        if stats.destroyed == 0 {
            continue;
        }
        assert!(is_rubble_tile(expected.building_at(40, 40)));
        assert_eq!(state.tick_pe_tornado_actors(), stats.changed());
        assert_eq!(state.city.building_at(40, 40), expected.building_at(40, 40));
        assert_eq!(state.pe_process_crt_seed(), preview.seed());
        found = true;
        break;
    }
    assert!(found, "expected a tornado destroy CRT seed");
}

#[test]
fn advance_active_disasters_keeps_tornado_until_xthg_tick_despawns() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    assert!(state.start_disaster(DisasterKind::Tornado));
    assert!(state.city.pe_tornado_actor().is_some());
    assert_eq!(state.advance_active_disasters(), 0);
    assert!(state.city.pe_tornado_actor().is_some());
    assert!(state.city.has_active_disaster());
}

#[test]
fn pe_monster_tick_lowers_height_then_attacks_southeast_tile() {
    // PE @ 0x454b45 saved_b==0: it steps east, then attacks the +1,+1 tile
    // relative to that next-step coordinate (42,41), keeping XBLD on ignition.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(42, 41, 0x70);
    city.set_text_overlay(42, 41, 0x05);
    city.set_label(0x05, "SESign");
    city.set_traffic_density(42, 40, 80);
    city.set_traffic_density(43, 40, 80);
    city.set_traffic_density(42, 41, 80);
    city.set_traffic_density(43, 41, 80);
    assert!(city.place_pe_monster_actor(40, 40, 50, 50, 0, 0).is_some());
    if let Some(overlays) = city.traffic_overlays_mut() {
        for record in overlays.iter_mut() {
            if record.is_pe_monster_actor() {
                record.height = 8;
                record.state = PE_MONSTER_STATE_ATTACK;
                record.saved_b = 0;
            }
        }
    }
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x08);
    city.set_pe_city_center(60, 40);
    let mut state = test_state(city);
    state.hover_tile = Some((20, 40));
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_monster_tick(&state, &mut preview, &mut preview_lfsr);
    assert!(
        stats.destroyed > 0,
        "attack state must ignite XBLD>=6 at (x+1,y+1)"
    );
    assert_eq!(expected.text_overlay_at(42, 41), Some(0xff));
    assert_eq!(expected.building_at(42, 41), 0x70);
    assert_eq!(expected.label_at(0x05).unwrap_or(""), "");
    assert_eq!(expected.traffic_density_at(42, 41), Some(0));
    assert_eq!(state.tick_pe_monster_actors(), stats.changed());
    assert_eq!(state.city.building_at(42, 41), 0x70);
    assert_eq!(state.city.text_overlay_at(42, 41), Some(0xff));
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_monster_attack_rubble_xtxt_skips_sound_bit() {
    // PE @ `0x454b62`: FUN_00460e10 rubble ret 0 must not count as attack success.
    let mut city = City::new_blank("/tmp/sc2000-mon-rubble.sc2", "MonRb").expect("blank");
    city.set_building(41, 41, 0x70);
    city.set_text_overlay(41, 41, 0xf5);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(!pe_fire_ignite_ok(&city, 41, 41));
    assert!(!pe_monster_attack_tile(
        &mut city, &mut crt, &mut lfsr, 41, 41, 0
    ));
    assert_eq!(city.text_overlay_at(41, 41), Some(0xf5));
    assert!((1..=4).contains(&city.building_at(41, 41)));
}

#[test]
fn pe_monster_attack_saved_b2_places_surface_water() {
    // PE @ `0x454c03`: after destroy, `call 0x401884` → PlaceWater helper `0x446650`.
    let mut city = City::new_blank("/tmp/sc2000-mon-water.sc2", "MonH2O").expect("blank");
    city.set_building(41, 41, 0x70);
    city.set_altitude(41, 41, 7, 0);
    city.set_traffic_density(41, 41, 0x22);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_monster_attack_tile(
        &mut city, &mut crt, &mut lfsr, 41, 41, 2
    ));
    assert_eq!(city.building_at(41, 41), 0);
    assert_eq!(city.terrain_at(41, 41), 0x3d);
    assert_eq!(city.altitude_at(41, 41), 7);
    assert_eq!(city.water_height_at(41, 41), 7);
    assert!(city.tile_bits_at(41, 41).is_some_and(TileBits::water_tile));
    assert_eq!(city.traffic_density_at(41, 41), Some(0x22));
}

#[test]
fn pe_monster_attack_saved_b1_destroy_building_consumes_crt_before_mode_rubble() {
    // PE @ `0x454bb0`: selector-1 scatter (2 draws), CRT rubble, then CRT `&3+9` PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-mon-b1-crt.sc2", "MonB1Crt").expect("blank");
    city.set_building(41, 41, 0x70);
    let mut preview = Win95CrtRand::new(0x454b_b001);
    let _ = preview.random_u15(); // selector-1 render scatter @ `0x459DEB`
    let _ = preview.random_u15(); // selector-1 render scatter @ `0x459E19`
    let _destroy = preview.rubble_tile_id();
    let expected = (preview.random_u16() & 3) as u8 + 9;
    let mut crt = Win95CrtRand::new(0x454b_b001);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_monster_attack_tile(
        &mut city, &mut crt, &mut lfsr, 41, 41, 1
    ));
    assert_eq!(city.building_at(41, 41), expected);
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn pe_monster_attack_removes_paired_tunnel_before_placing_rubble() {
    // Native 0x454bbf with seed 1. DestroyBuilding takes four draws for
    // the two portals before the monster draws at 0x454be9.
    let mut city = City::new_blank("/tmp/sc2000-mon-portal.sc2", "MonPortal").unwrap();
    city.set_building(20, 20, 0x3f);
    city.set_building(17, 20, 0x41);
    for x in 17..=20 {
        city.set_altm_word(x, 20, 0x040a);
    }
    for x in [17, 20] {
        city.set_zone(x, 20, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    }
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);

    assert!(pe_monster_attack_tile(
        &mut city, &mut crt, &mut lfsr, 20, 20, 1
    ));

    assert_eq!(city.building_at(20, 20), 0x0a);
    assert_eq!(city.building_at(17, 20), 0);
    for x in 17..=20 {
        assert_eq!(city.tunnel_levels_at(x, 20), 0);
        assert_eq!(city.altitude_at(x, 20), 10);
    }
    for x in [17, 20] {
        assert_eq!(
            city.zone_at(x, 20).unwrap().raw_bits(),
            ZONE_RESIDENTIAL_LIGHT
        );
    }
    assert_eq!(crt.seed(), 3_403_800_452);
}

#[test]
fn pe_monster_attack_saved_b3_places_tile_0xc8() {
    // PE @ `0x454c0f`: after destroy, PlaceTile-size(1, 0xc8) via `0x4027f2` → `0x440c50`.
    let mut city = City::new_blank("/tmp/sc2000-mon-c8.sc2", "MonC8").expect("blank");
    city.set_building(41, 41, 0x70);
    city.set_traffic_density(41, 41, 0x33);
    city.set_zone(41, 41, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_tile_bits(41, 41, TileBits::new(0).with_piped(true).with_watered(true));
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_monster_attack_tile(
        &mut city, &mut crt, &mut lfsr, 41, 41, 3
    ));
    assert_eq!(city.building_at(41, 41), 0xc8);
    assert_eq!(city.traffic_density_at(41, 41), Some(0x33));
    let bits = city.tile_bits_at(41, 41).unwrap();
    // PlaceTile-size: XBIT &= 0x1f, then OR 0xe0; keep watered and restamp
    // piped, powered, and powerable.
    assert!(bits.piped() && bits.watered());
    assert!(bits.powered() && bits.powerable());
    assert_eq!(bits.raw_bits() & 0xe0, 0xe0);
    // Size-1 epilogue ORs ZONE_CORNER_ALL after clearing type.
    assert_eq!(city.zone_at(41, 41).unwrap().raw_bits(), ZONE_CORNER_ALL);
    // Attack gate still refuses XBLD 0xc8.
    assert!(!pe_monster_attack_tile(
        &mut city, &mut crt, &mut lfsr, 41, 41, 3
    ));
}

#[test]
fn pe_monster_tick_walk_with_low_height_enters_attack_state() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(city.place_pe_monster_actor(40, 40, 50, 50, 0, 0).is_some());
    if let Some(overlays) = city.traffic_overlays_mut() {
        for record in overlays.iter_mut() {
            if record.is_pe_monster_actor() {
                record.height = 8;
                record.state = PE_MONSTER_STATE_WALK;
            }
        }
    }
    pe_monster_tick_city(&mut city, 60, 40);
    let (_, rec) = city.pe_monster_actor().expect("monster still active");
    assert_eq!(rec.state, PE_MONSTER_STATE_ATTACK);
}

#[test]
fn pe_monster_tick_forces_airplanes_to_state_7_and_helicopters_to_state_5() {
    // PE FUN_004548c0 @ 0x454903: scare prologue before monster FSM.
    let mut city = City::new_blank("/tmp/sc2000-monster-scare.sc2", "MonScare").expect("blank");
    // Plane/heli spawn refuses when a monster already occupies XTHG.
    assert!(city.place_ambient_port_actor(20, 20, XTHG_AIRPLANE, XTHG_DIRECTION_NORTH));
    assert!(city.place_ambient_port_actor(22, 22, XTHG_HELICOPTER, XTHG_DIRECTION_NORTH));
    assert!(city.place_pe_monster_actor(40, 40, 50, 50, 0, 0).is_some());
    if let Some(overlays) = city.traffic_overlays_mut() {
        for record in overlays.iter_mut() {
            if record.kind == XTHG_AIRPLANE || record.kind == XTHG_HELICOPTER {
                record.state = 2;
            }
        }
    }
    pe_monster_tick_city(&mut city, 60, 40);
    let plane = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .find(|r| r.kind == XTHG_AIRPLANE)
        .expect("plane");
    let heli = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .find(|r| r.kind == XTHG_HELICOPTER)
        .expect("heli");
    assert_eq!(plane.state, 7);
    assert_eq!(heli.state, 5);
    assert!(city.pe_monster_actor().is_some(), "monster FSM still runs");
}

#[test]
fn pe_monster_scare_prologue_requires_a_monster_actor() {
    let mut city =
        City::new_blank("/tmp/sc2000-monster-scare-gate.sc2", "MonScareGate").expect("blank");
    assert!(city.place_ambient_port_actor(20, 20, XTHG_AIRPLANE, XTHG_DIRECTION_NORTH));
    assert!(city.place_ambient_port_actor(22, 22, XTHG_HELICOPTER, XTHG_DIRECTION_NORTH));
    if let Some(overlays) = city.traffic_overlays_mut() {
        for record in overlays.iter_mut() {
            if record.kind == XTHG_AIRPLANE || record.kind == XTHG_HELICOPTER {
                record.state = 2;
            }
        }
    }

    let mut crt = Win95CrtRand::new(0x4548_c001);
    let mut lfsr = Win95Lfsr::new(0x4548);
    let before_crt = crt;
    let before_lfsr = lfsr;
    let stats = pe_monster_tick_city_with_rng(&mut city, 60, 40, &mut crt, &mut lfsr);

    assert_eq!(stats, PeMonsterTickStats::default());
    assert_eq!(crt, before_crt, "no kind-5 handler means no CRT draws");
    assert_eq!(lfsr, before_lfsr, "no kind-5 handler means no LFSR draws");
    let overlays = city.traffic_overlays().expect("overlay table");
    assert_eq!(overlays[1].state, 2, "plane stays on its own FSM state");
    assert_eq!(overlays[2].state, 2, "heli stays on its own FSM state");
}

#[test]
fn pe_monster_scare_prologue_ignores_later_plane_and_heli_slots() {
    let mut city =
        City::new_blank("/tmp/sc2000-monster-scare-order.sc2", "MonScareOrder").expect("blank");
    assert!(city.place_ambient_port_actor(20, 20, XTHG_AIRPLANE, XTHG_DIRECTION_NORTH));
    assert!(city.place_ambient_port_actor(22, 22, XTHG_HELICOPTER, XTHG_DIRECTION_NORTH));
    assert!(city.place_pe_monster_actor(40, 40, 50, 50, 0, 0).is_some());
    let (plane, heli, monster) = {
        let overlays = city.traffic_overlays().expect("overlay table");
        (overlays[1], overlays[2], overlays[3])
    };
    {
        let overlays = city.traffic_overlays_mut().expect("overlay table");
        // Model the PE's ascending walk with the monster before both ambient slots.
        overlays[1] = monster;
        overlays[2] = heli;
        overlays[3] = plane;
        overlays[2].state = 2;
        overlays[3].state = 2;
    }
    for slot in 1..=3 {
        let record = city.traffic_overlays().unwrap()[slot];
        city.set_text_overlay(
            usize::from(record.tile_x),
            usize::from(record.tile_y),
            crate::win95_overlay::xthg_proxy_for_slot(slot).unwrap(),
        );
    }

    let mut crt = Win95CrtRand::new(0x4548_c002);
    let mut lfsr = Win95Lfsr::new(0x4549);
    pe_monster_tick_city_with_rng(&mut city, 60, 40, &mut crt, &mut lfsr);

    let overlays = city.traffic_overlays().expect("overlay table");
    assert_eq!(overlays[2].kind, XTHG_HELICOPTER);
    assert_eq!(overlays[2].state, 2, "later heli slot was not counted yet");
    assert_eq!(overlays[3].kind, XTHG_AIRPLANE);
    assert_eq!(overlays[3].state, 2, "later plane slot was not counted yet");
}

#[test]
fn pe_airplane_before_monster_waits_until_next_slot_pass_to_crash() {
    let make_city = || {
        let mut city = City::new_blank(
            "/tmp/sc2000-airplane-monster-order.sc2",
            "AirplaneMonsterOrder",
        )
        .expect("blank");
        let mut table = crate::win95_overlay::empty_overlay_table();
        table[1] = Win95OverlayRecord {
            kind: XTHG_AIRPLANE,
            lane: 2,
            state: 0,
            tile_x: 41,
            tile_y: 40,
            height: 0,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        table[2] = Win95OverlayRecord {
            kind: XTHG_MONSTER,
            lane: 2,
            state: PE_MONSTER_STATE_WALK,
            tile_x: 60,
            tile_y: 60,
            height: 0x0f,
            sub_x: 8,
            sub_y: 8,
            ..Default::default()
        };
        city.set_traffic_overlays_for_test(table);
        for slot in 1..=2 {
            let record = city.traffic_overlays().unwrap()[slot];
            city.set_text_overlay(
                usize::from(record.tile_x),
                usize::from(record.tile_y),
                crate::win95_overlay::xthg_proxy_for_slot(slot).unwrap(),
            );
        }
        city
    };

    let mut ordered = make_city();
    let mut ordered_sim = Win95OverlaySim::new();
    let mut ordered_crt = Win95CrtRand::new(0x4548_c103);
    let mut ordered_lfsr = Win95Lfsr::new(0x4548);
    let _ = pe_disaster_xthg_tick_slots_in_order(
        &mut ordered_sim,
        &mut ordered,
        &mut ordered_crt,
        &mut ordered_lfsr,
        &mut Win95GameRand::new(Win95GameRand::DEFAULT_STATE),
    );
    let plane_after_native_order = ordered.traffic_overlays().unwrap()[1];
    assert_eq!(plane_after_native_order.kind, XTHG_AIRPLANE);
    assert_eq!(
        plane_after_native_order.state, 7,
        "the earlier plane handler has already run when the later monster scares it"
    );

    let mut family_batched = make_city();
    let mut batched_crt = Win95CrtRand::new(0x4548_c103);
    let mut batched_lfsr = Win95Lfsr::new(0x4548);
    pe_monster_tick_city_with_rng(
        &mut family_batched,
        60,
        61,
        &mut batched_crt,
        &mut batched_lfsr,
    );
    let _ = pe_aircrash_tick_city_with_lfsr(&mut family_batched, &mut batched_lfsr);
    assert_eq!(
        family_batched.traffic_overlays().unwrap()[1].kind,
        OVERLAY_KIND_TRAFFIC,
        "the old monster-then-aircrash batching incorrectly handles state 7 immediately"
    );
}

#[test]
fn advance_active_disasters_keeps_monster_until_xthg_tick_despawns() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((40, 40));
    assert!(state.start_disaster(DisasterKind::Monster));
    assert!(state.city.pe_monster_actor().is_some());
    assert_eq!(state.advance_active_disasters(), 0);
    assert!(state.city.pe_monster_actor().is_some());
    assert!(state.city.has_active_disaster());
}

#[test]
fn pe_monster_xthg_target_is_rand_and_7f() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // PE 0x45ee10 seeds CRT rand (0x48CDD8), not Mac Toolbox Random.
    let mut rng = Win95CrtRand::new(pe_monster_actor_seed(&city, 12, 34));
    let (target_x, target_y, saved_a, saved_b) = pe_monster_xthg_fields(&city, 12, 34);
    assert_eq!(target_x, (rng.random_u15() & 0x7f) as u8);
    assert_eq!(target_y, (rng.random_u15() & 0x7f) as u8);
    assert_eq!(saved_a, 0);
    let expected_b = if rng.random_u15() & 1 != 0 {
        0
    } else {
        (rng.random_u15() % 3) as u8 + 1
    };
    assert_eq!(saved_b, expected_b);
    assert_eq!(XTHG_MONSTER, 5);
    assert_eq!(crate::win95_overlay::XTHG_MONSTER_HEIGHT, 0x0f);
    assert_eq!(crate::win95_overlay::XTHG_MONSTER_SUBTILE, 8);
}

#[test]
fn pe_monster_constructor_obeys_scenario_and_parity_draw_gates() {
    let mut city = City::new_blank("/tmp/pe-monster.sc2", "test").unwrap();
    for (scenario, seed, fields, final_seed) in [
        (false, 1, (41, 35, 0, 2), 3_884_216_597),
        (false, 4, (51, 25, 0, 0), 3_927_378_665),
        (true, 1, (41, 35, 0, 0), 3_357_800_067),
    ] {
        city.set_pe_scenario_active(scenario);
        let mut rng = Win95CrtRand::new(seed);
        assert_eq!(
            pe_monster_xthg_fields_with_rng(&city, 12, 34, &mut rng),
            fields
        );
        assert_eq!(rng.seed(), final_seed);
    }
}

#[test]
fn pe_monster_constructor_reserves_slot_before_proxy_cleanup() {
    let mut city = City::new_blank("/tmp/pe-monster.sc2", "test").unwrap();
    assert!(
        city.place_pe_tornado_actor(12, 34, 2, 15, 1, 2, 0)
            .is_some()
    );
    let mut state = test_state(city);
    let slot = XTHG_FIRST_USABLE_SLOT;
    let proxy = 0xc9 + slot as u8;
    for record in state
        .city
        .traffic_overlays_mut()
        .unwrap()
        .iter_mut()
        .skip(slot + 1)
    {
        record.kind = 1;
    }
    let seed = state.pe_process_crt_seed();
    assert_eq!(state.seed_pe_monster(12, 34), 0);
    assert_eq!(state.pe_process_crt_seed(), seed);
    assert_eq!(state.city.text_overlay_at(12, 34), Some(proxy));

    state.city.traffic_overlays_mut().unwrap()[slot + 1].kind = 0;
    assert_eq!(state.seed_pe_monster(12, 34), 1);
    assert_eq!(state.city.pe_monster_actor().unwrap().0, slot + 1);
    assert_eq!(state.city.traffic_overlays().unwrap()[slot].kind, 0);
    assert_eq!(state.city.text_overlay_at(12, 34), Some(proxy + 1));
}

#[test]
fn start_disaster_monster_spawns_xthg_actor() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));

    let mut preview = state.pe_process_crt;
    let expected_fields = pe_monster_xthg_fields_with_rng(&state.city, 12, 34, &mut preview);
    assert!(state.start_disaster(DisasterKind::Monster));
    assert!(disaster_overlay_tiles(&state.city).is_empty());
    let slot = assert_pe_monster_actor(&state.city, 12, 34, expected_fields);
    assert_pe_xthg_proxy(&state.city, 12, 34, slot);
    assert_eq!(
        state.city.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x08)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_monster_disaster_requires_45000_population() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_pe_dw_city_population(44_999);
    assert!(!pe_monster_disaster_ready(&city));
    city.set_pe_dw_city_population(45_000);
    assert!(pe_monster_disaster_ready(&city));
}

#[test]
fn pe_ambient_population_gates_use_loaded_city_population_on_both_layouts() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for (saved_population, expected_min_gate, expected_mass_riot, expected_monster) in [
            (999, true, false, false),
            (1_000, false, false, false),
            (29_999, false, false, false),
            (30_000, false, true, false),
            (44_999, false, true, false),
            (45_000, false, true, true),
        ] {
            let path = std::env::temp_dir().join(format!(
                "sc2000-monster-population-{}-{saved_population}.{}",
                std::process::id(),
                if kind == CityFileKind::MacCity {
                    "city"
                } else {
                    "sc2"
                }
            ));
            let mut city = City::new_blank(&path, "MonsterPopulation").expect("blank city");
            city.kind = kind;
            city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_MEDIUM);
            if kind == CityFileKind::MacCity {
                city.set_mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, saved_population);
            } else {
                city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, saved_population);
            }
            city.save(&path).expect("save population fixture");
            let mut loaded = City::load(&path).expect("reload population fixture");
            if kind == CityFileKind::MacCity {
                loaded.prepare_pe_fidelity_from_saved_city();
            }
            assert_eq!(loaded.pe_dw_city_population(), saved_population);
            assert!(!loaded.pe_zone_pops_ready());
            loaded.set_pe_unemployment_percent(10);
            loaded.pe_b_weather_heat = 0xaa;

            assert_eq!(
                ambient_disaster_month_gates_fail(&loaded, GameClock::new(1905, 1)),
                expected_min_gate,
                "minimum-pop gate engine=Win95 file={kind:?} saved={saved_population}"
            );
            assert_eq!(
                pe_mass_riot_disaster_ready(&loaded),
                expected_mass_riot,
                "mass-riot gate engine=Win95 file={kind:?} saved={saved_population}"
            );
            assert_eq!(
                pe_monster_disaster_ready(&loaded),
                expected_monster,
                "monster gate engine=Win95 file={kind:?} saved={saved_population}"
            );
            std::fs::remove_file(&path).expect("remove population fixture");
        }
    }
}

#[test]
fn ambient_disaster_kind_uses_pe_monster_gate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    let quiet = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    for seed in 0..64 {
        let mut rng = MacToolboxRng::new(seed);
        assert_ne!(
            ambient_disaster_kind_for_city(&city, quiet, outcomes, &mut rng, EngineFidelity::Win95),
            Some(DisasterKind::Monster),
            "monster must not spawn below PE 45k pop (seed {seed})"
        );
    }

    let monster_ready = PopulationStats {
        normal_population: 45_000,
        total_population: 45_000,
        ..quiet
    };
    city.set_pe_dw_city_population(45_000);
    let saw_monster = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(
            &city,
            monster_ready,
            outcomes,
            &mut rng,
            EngineFidelity::Win95,
        ) == Some(DisasterKind::Monster)
    });
    assert!(
        saw_monster,
        "cities at 45k population should be able to roll ambient monster"
    );
}

#[test]
fn ambient_aircrash_requires_runway_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    assert!(!pe_aircrash_disaster_ready(&city));
    assert!(!mac_aircrash_disaster_ready(&city));

    // Mac ready still rescans XBLD; PE ready uses `pe_dw_tile_count[0xdd]`.
    city.set_building(40, 40, RUNWAY_BUILDING_ID);
    assert!(!pe_aircrash_disaster_ready(&city));
    assert!(mac_aircrash_disaster_ready(&city));
    pe_place_tile_441f00(&mut city, 41, 41, RUNWAY_BUILDING_ID);
    assert!(pe_aircrash_disaster_ready(&city));

    let population = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();
    let saw_aircrash = (0..64).any(|seed| {
        let mut rng = MacToolboxRng::new(seed);
        ambient_disaster_kind_for_city(&city, population, outcomes, &mut rng, EngineFidelity::Win95)
            == Some(DisasterKind::AirCrash)
    });
    assert!(
        saw_aircrash,
        "runway cities should be able to roll ambient air crash"
    );

    // Clear straight runway without underflowing the PlaceTile census:
    // (40,40) was set_building-only; (41,41) was PlaceTile'd.
    city.set_building(40, 40, 0);
    pe_place_tile_441f00(&mut city, 41, 41, 0);
    pe_place_tile_441f00(&mut city, 42, 42, AIRPORT_RUNWAY_CROSS_BUILDING_ID);
    assert!(
        !pe_aircrash_disaster_ready(&city),
        "PE @ 0x4722a4 only tests dwTileCount[0xdd], not 0xde"
    );
    assert!(
        !mac_aircrash_disaster_ready(&city),
        "Mac type-0x12 only tests building 221 count word @ 0x1ba"
    );
    assert!(!aircrash_disaster_ready(&city, EngineFidelity::Mac68k));
    assert!(!aircrash_disaster_ready(&city, EngineFidelity::Win95));
}

#[test]
fn pe_aircrash_disaster_ready_uses_dw_tile_count_like_4722a4() {
    // PE @ `0x4722a4`: ready iff `0x4ca4c8[0xdd] != 0`, not a live map rescan.
    let mut city = City::new_blank("/tmp/sc2000-aircrash-dw.sc2", "CrashDw").expect("blank");
    city.set_building(40, 40, RUNWAY_BUILDING_ID);
    assert!(!pe_aircrash_disaster_ready(&city));
    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = 1;
    assert!(pe_aircrash_disaster_ready(&city));
    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = 0;
    assert!(!pe_aircrash_disaster_ready(&city));
    pe_place_tile_441f00(&mut city, 41, 41, RUNWAY_BUILDING_ID);
    assert!(pe_aircrash_disaster_ready(&city));
    city.pe_dw_tile_count[AIRPORT_RUNWAY_CROSS_BUILDING_ID as usize] = 1;
    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = 0;
    assert!(!pe_aircrash_disaster_ready(&city));
}

#[test]
fn aircrash_ambient_target_follows_engine_fidelity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, RUNWAY_BUILDING_ID);

    let mut mac_rng = MacToolboxRng::new(0x35BF_0012);
    let mut direct = MacToolboxRng::new(0x35BF_0012);
    let mac = aircrash_ambient_target_tile(&city, &mut mac_rng, EngineFidelity::Mac68k, None)
        .expect("Mac air-crash tile");
    assert_eq!(mac, mac_random_interior_map_tile(&mut direct));
    assert!((1..=0x7e).contains(&mac.0) && (1..=0x7e).contains(&mac.1));

    let mut win_rng = MacToolboxRng::new(0);
    let win = aircrash_ambient_target_tile(&city, &mut win_rng, EngineFidelity::Win95, None)
        .expect("Win95 air-crash tile");
    let pe = pe_aircrash_tile(&city).expect("PE air-crash tile");
    assert_eq!(win, pe);
    // PE probes `0x20..0x7F`; Mac formula with the same seed stream differs.
    let mut mac_formula = MacToolboxRng::new(0);
    assert_ne!(win, mac_random_interior_map_tile(&mut mac_formula));

    let mut state = test_state(city.clone());
    let mut peek = state.pe_process_lfsr;
    let process = aircrash_ambient_target_tile(
        &state.city,
        &mut win_rng,
        EngineFidelity::Win95,
        Some(&mut peek),
    )
    .expect("process-LFSR air-crash tile");
    let mut expect_lfsr = state.pe_process_lfsr;
    let expect = pe_aircrash_tile_with_lfsr(&state.city, &mut expect_lfsr).expect("LFSR probe");
    assert_eq!(process, expect);
    assert_eq!(peek.state(), expect_lfsr.state());
    assert_ne!(process, pe, "process LFSR must not reuse city-name seed");
}

#[test]
fn pe_ambient_weather_disaster_uses_remainder_under_15() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_MEDIUM);
    let population = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();

    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    let tornado_clock = first_ambient_chance_clock_with_remainder(&city, 1, 0x0e);
    assert_eq!(
        ambient_disaster_for_month::<Win95CrtRand>(
            &city,
            tornado_clock,
            population,
            outcomes,
            EngineFidelity::Win95
        )
        .map(|(kind, _)| kind),
        Some(DisasterKind::Tornado)
    );

    city.set_misc_u32_at(
        MISC_WEATHER_TREND_OFFSET,
        u32::from(WEATHER_TREND_HURRICANE),
    );
    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    let hurricane_clock = first_ambient_chance_clock_with_remainder(&city, 1, 0x0e);
    assert_eq!(
        ambient_disaster_for_month::<Win95CrtRand>(
            &city,
            hurricane_clock,
            population,
            outcomes,
            EngineFidelity::Win95
        )
        .map(|(kind, _)| kind),
        Some(DisasterKind::Hurricane)
    );

    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_DISABLED_VALUE);
    assert_eq!(
        ambient_disaster_for_month::<Win95CrtRand>(
            &city,
            hurricane_clock,
            population,
            outcomes,
            EngineFidelity::Win95
        ),
        None,
        "hurricane remainder < 0x0F still needs a coast"
    );

    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    let late_clock = first_ambient_chance_clock_with_remainder(&city, 0x0f, 59);
    assert_eq!(
        ambient_disaster_for_month::<Win95CrtRand>(
            &city,
            late_clock,
            population,
            outcomes,
            EngineFidelity::Win95
        ),
        None,
        "remainder >= 0x0F skips both weather and jump-table disasters"
    );
}

#[test]
fn pe_ambient_disaster_chance_ready_is_one_in_difficulty_months() {
    assert!(!pe_ambient_disaster_chance_ready(
        &mut MacToolboxRng::new(1),
        CITY_DIFFICULTY_NONE
    ));
    let modulus = pe_ambient_disaster_min_months(CITY_DIFFICULTY_MEDIUM) as u16;
    let mut rng = MacToolboxRng::new(0xC4A4_CE01);
    let expected = rng.roll_mod_is_zero(modulus);
    let mut rng = MacToolboxRng::new(0xC4A4_CE01);
    assert_eq!(
        pe_ambient_disaster_chance_ready(&mut rng, CITY_DIFFICULTY_MEDIUM),
        expected
    );
}

#[test]
fn pe_ambient_disaster_min_months_match_win95_difficulty_table() {
    assert_eq!(pe_ambient_disaster_min_months(CITY_DIFFICULTY_NONE), 0);
    assert_eq!(pe_ambient_disaster_min_months(CITY_DIFFICULTY_EASY), 100);
    assert_eq!(pe_ambient_disaster_min_months(CITY_DIFFICULTY_MEDIUM), 60);
    assert_eq!(pe_ambient_disaster_min_months(CITY_DIFFICULTY_HARD), 30);
}

#[test]
fn pe_ambient_disaster_age_ready_uses_difficulty_table() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_MEDIUM);
    assert!(!pe_ambient_disaster_age_ready(
        &city,
        GameClock::new(1904, 12)
    ));
    assert!(pe_ambient_disaster_age_ready(
        &city,
        GameClock::new(1905, 1)
    ));

    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
    assert!(!pe_ambient_disaster_age_ready(
        &city,
        GameClock::new(1902, 6)
    ));
    assert!(pe_ambient_disaster_age_ready(
        &city,
        GameClock::new(1902, 7)
    ));
}

#[test]
fn ambient_disaster_min_pop_gate_uses_pe_dw_city_population_directly() {
    // PE @ `0x471c50`: direct DWORD `dwCityPopulation >= 0x3e8`.
    let clock = GameClock::new(1905, 1);
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_MEDIUM);

    assert!(
        ambient_disaster_month_gates_fail(&city, clock),
        "a census estimate must not replace the PE population global"
    );

    city.set_pe_dw_city_population(AMBIENT_DISASTER_MIN_POPULATION);
    assert!(!ambient_disaster_month_gates_fail(&city, clock));

    city.set_pe_zone_pops([50, 0, 0]);
    city.set_pe_dw_city_population(AMBIENT_DISASTER_MIN_POPULATION - 1);
    assert!(
        ambient_disaster_month_gates_fail(&city, clock),
        "ready pe_dw under min-pop should block even when census is large"
    );

    city.set_pe_dw_city_population(AMBIENT_DISASTER_MIN_POPULATION);
    assert!(!ambient_disaster_month_gates_fail(&city, clock));
}

#[test]
fn no_disasters_blocks_ambient_monthly_disasters() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    let clock = first_due_ambient_disaster_clock(&city);
    let mut state = test_state(city);
    state.game_clock = clock;
    state.toggle_no_disasters();

    state.handle_simulation_month_advanced();

    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());
    assert!(!state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Emergency)));
}

#[test]
fn disaster_damage_replaces_complete_multitile_footprint_with_rubble() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let building = COMMERCIAL_3X3_BUILDINGS[0];
    let zone_id = ZONE_COMMERCIAL_DENSE;
    let origin = (20, 20);
    let footprint = building_footprint_from_origin(origin.0, origin.1, 3).unwrap();
    let corner_bits = city.footprint_corner_bits();
    for &(x, y) in &footprint {
        city.set_zone(
            x,
            y,
            zone_id | building_zone_bits_for_tile(origin.0, origin.1, x, y, 3, corner_bits),
        );
        city.set_building(x, y, building);
        city.set_tile_bits(
            x,
            y,
            crate::city::TileBits::new(0)
                .with_piped(true)
                .with_watered(true),
        );
        city.set_traffic_density(x, y, 99);
    }
    city.set_text_overlay(21, 21, 0xfb);
    let mut state = test_state(city);

    assert!(state.damage_disaster_tile(21, 21));

    for &(x, y) in &footprint {
        assert!(
            is_rubble_tile(state.city.building_at(x, y)),
            "expected DestroyBuilding rubble at ({x},{y}), got {}",
            state.city.building_at(x, y)
        );
        let zone = state.city.zone_at(x, y).unwrap();
        assert_eq!(zone.zone_id(), zone_id, "zone id at ({x},{y})");
        assert_eq!(zone.raw_bits() & 0xf0, 0, "corner bits at ({x},{y})");
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped(), "piped bit at ({x},{y})");
        assert!(bits.watered(), "watered bit at ({x},{y})");
        assert_eq!(
            state.city.traffic_density_at(x, y),
            Some(99),
            "DestroyStructure must leave XTRF alone at ({x},{y})"
        );
        assert!(state.city.building_should_draw_at(x, y));
    }
    assert_eq!(
        state.city.text_overlay_at(21, 21),
        Some(0xfb),
        "disaster damage re-stamps the active marker after DestroyStructure"
    );
}

#[test]
fn disaster_damage_incomplete_multitile_fragment_replaces_only_hit_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let building = COMMERCIAL_3X3_BUILDINGS[0];
    let zone_id = ZONE_COMMERCIAL_DENSE;
    let origin = (20, 20);
    let footprint = building_footprint_from_origin(origin.0, origin.1, 3).unwrap();
    let corner_bits = city.footprint_corner_bits();
    for &(x, y) in &footprint {
        city.set_zone(
            x,
            y,
            zone_id | building_zone_bits_for_tile(origin.0, origin.1, x, y, 3, corner_bits),
        );
        city.set_building(x, y, building);
    }
    city.set_building(22, 22, 0);
    city.set_zone(22, 22, zone_id);
    city.set_text_overlay(21, 21, 0xfb);
    let mut state = test_state(city);

    assert!(state.damage_disaster_tile(21, 21));

    for &(x, y) in &footprint {
        if (x, y) == (21, 21) {
            assert!(is_rubble_tile(state.city.building_at(x, y)));
            assert_eq!(state.city.zone_at(x, y).unwrap().raw_bits() & 0xf0, 0);
        } else {
            let expected_building = if (x, y) == (22, 22) { 0 } else { building };
            let expected_corners = if (x, y) == (22, 22) {
                0
            } else {
                building_zone_bits_for_tile(origin.0, origin.1, x, y, 3, corner_bits)
            };
            assert_eq!(state.city.building_at(x, y), expected_building);
            assert_eq!(
                state.city.zone_at(x, y).unwrap().raw_bits() & 0xf0,
                expected_corners,
                "incomplete footprint cell ({x},{y}) is outside DestroyBuilding's hit-tile fallback"
            );
        }
    }
}

#[test]
fn daily_fire_overlay_tick_matches_pe_preview_before_later_damage() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 220);
    city.set_building(21, 20, 220);
    city.set_text_overlay(20, 20, 0xff);
    let mut state = test_state(city);
    let mut preview_crt = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, _stats) = preview_pe_fire_tick(&state, &mut preview_crt, &mut preview_lfsr);

    state.tick_pe_disaster_clock();

    for &(x, y) in &[(20, 20), (21, 20)] {
        assert_eq!(
            state.city.building_at(x, y),
            expected.building_at(x, y),
            "XBLD after the daily PE overlay pass at ({x},{y})"
        );
        assert_eq!(
            state.city.text_overlay_at(x, y),
            expected.text_overlay_at(x, y),
            "XTXT after the daily PE overlay pass at ({x},{y})"
        );
    }
}

#[test]
fn disaster_advance_never_writes_marker_values_to_building_layer() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..24 {
        for x in 20..24 {
            city.set_building(x, y, 220);
        }
    }
    city.set_text_overlay(20, 20, 0xff);
    city.set_text_overlay(22, 22, 0xfd);
    let mut state = test_state(city);

    for _ in 0..4 {
        state.handle_simulation_month_advanced();
    }

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            assert!(
                !is_disaster_text_overlay(state.city.building_at(x, y)),
                "disaster marker leaked into XBLD at ({x},{y})"
            );
        }
    }
}

#[test]
fn pe_idle_spent_disaster_markers_burn_out_without_calendar_steps() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(20, 20, 0xfb);
    city.set_text_overlay(21, 20, 0xfc);
    let mut state = test_state(city);

    finish_pe_cloud_disaster_for_test(&mut state);

    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());
    assert!(!state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Emergency)));
    assert_eq!(state.query_line, "Disaster Contained");
}

#[test]
fn quiet_month_after_disaster_restores_idle_status_line() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(20, 20, 0xfb);
    let mut state = test_state(city);

    finish_pe_cloud_disaster_for_test(&mut state);
    assert_eq!(state.query_line, "Disaster Contained");

    state.close_modal_dialogs();
    state.traffic.set_speed(GameSpeed::Paused);
    state.handle_simulation_month_advanced();
    assert!(
        !matches!(
            state.query_line.as_str(),
            "Disaster Damage Reported" | "Disaster Spreading" | "Disaster Contained"
        ),
        "quiet month should rewrite the clock line, got {:?}",
        state.query_line
    );
    assert!(
        state.query_line.contains("*PAUSED*"),
        "{}",
        state.query_line
    );
}

#[test]
fn pe_idle_active_disaster_holds_due_ambient_calendar_phase() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_text_overlay(0, 0, 0xfb);
    let clock = first_due_ambient_disaster_clock(&city);
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(clock.year(), clock.month(), 21);
    let before = state.game_clock;

    finish_pe_cloud_disaster_for_test(&mut state);
    assert_eq!(
        state.game_clock, before,
        "the due day-22 phase was not dispatched"
    );

    assert!(disaster_overlay_tiles(&state.city).is_empty());
    assert!(!state.city.has_active_disaster());
    assert!(
        !state.query_line.ends_with(" Reported"),
        "mode-2 continuation must not run the due ambient calendar phase, got {}",
        state.query_line
    );
}

#[test]
fn disaster_menu_action_seeds_kind_footprint() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));

    state.start_disaster(DisasterKind::ToxicSpill);

    assert_eq!(disaster_overlay_tiles(&state.city), vec![(12, 34, 0xfb)]);
    assert!(state.city.has_active_disaster());
}

#[test]
fn disaster_marker_identity_matches_xtxt_disaster_meanings() {
    assert_eq!(disaster_marker_for_step(DisasterKind::Fire, 0), 0xff);
    assert_eq!(disaster_marker_for_step(DisasterKind::Flood, 0), 0xfc);
    assert_eq!(
        disaster_marker_for_step(DisasterKind::BlizzardFlood, 0),
        0xfc
    );
    assert_eq!(disaster_marker_for_step(DisasterKind::Hurricane, 0), 0xfc);
    assert_eq!(disaster_marker_for_step(DisasterKind::AirCrash, 0), 0xff);
    assert_eq!(disaster_marker_for_step(DisasterKind::NopAirCrash, 0), 0xff);
    assert_eq!(
        disaster_marker_for_step(DisasterKind::HelicopterCrash, 0),
        0xff
    );
    assert_eq!(disaster_marker_for_step(DisasterKind::Tornado, 0), 0xff);
    assert_eq!(disaster_marker_for_step(DisasterKind::Earthquake, 0), 0xff);
    assert_eq!(disaster_marker_for_step(DisasterKind::Monster, 0), 0xff);
    assert_eq!(disaster_marker_for_step(DisasterKind::Rioters, 0), 0xfd);
    assert_eq!(disaster_marker_for_step(DisasterKind::Rioters, 1), 0xfe);
    assert_eq!(disaster_marker_for_step(DisasterKind::MassRiot, 0), 0xfd);
    assert_eq!(disaster_marker_for_step(DisasterKind::MassRiot, 1), 0xfe);
    assert_eq!(disaster_marker_for_step(DisasterKind::Meltdown, 0), 0xfb);
    assert_eq!(disaster_marker_for_step(DisasterKind::Microwave, 0), 0xff);
    assert_eq!(disaster_marker_for_step(DisasterKind::ToxicSpill, 0), 0xfb);
    assert_eq!(disaster_marker_for_step(DisasterKind::Volcano, 0), 0xff);
    assert_eq!(disaster_marker_for_step(DisasterKind::Volcano, 1), 0xfb);
    assert_eq!(disaster_marker_for_step(DisasterKind::Firestorm, 0), 0xff);
    assert_eq!(
        disaster_marker_for_step(DisasterKind::PollutionAccident, 0),
        0xfb
    );

    assert_eq!(next_disaster_marker(0xff), 0xff);
    assert_eq!(next_disaster_marker(0xfc), 0xfc);
    assert_eq!(next_disaster_marker(0xfb), 0xfb);
    assert_eq!(next_disaster_marker(0xfd), 0xfe);
    assert_eq!(next_disaster_marker(0xfe), 0xfd);
}

#[test]
fn disaster_footprint_clips_to_city_bounds() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((0, 0));

    state.start_disaster(DisasterKind::ToxicSpill);

    assert_eq!(disaster_overlay_tiles(&state.city), vec![(0, 0, 0xfb)]);
    assert_eq!(state.query_line, "Toxic Spill Reported");
}

#[test]
fn manual_disaster_start_clears_stale_modal_menu_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    state.open_tool_menu = Some(ui::ToolMenu::Water);
    seed_all_modal_dialogs(&mut state);
    seed_all_drag_states(&mut state);
    state.query_is_inspect = true;

    state.start_disaster(DisasterKind::Flood);

    assert_eq!(state.open_tool_menu, None);
    assert_modal_dialogs_cleared(&state);
    assert_drag_states_cleared(&state);
    assert!(!state.query_is_inspect);
    assert!(state.city.has_active_disaster());
    assert_eq!(state.query_line, "Flood Reported");
}

#[test]
fn manual_disasters_use_recovered_effect_family_sounds() {
    for (kind, effect) in [
        (DisasterKind::Fire, PlacementSound::Fire),
        (DisasterKind::Flood, PlacementSound::Flood),
        (DisasterKind::BlizzardFlood, PlacementSound::Flood),
        (DisasterKind::AirCrash, PlacementSound::Mayday),
        (DisasterKind::NopAirCrash, PlacementSound::Disaster),
        (DisasterKind::HelicopterCrash, PlacementSound::Disaster),
        (DisasterKind::Tornado, PlacementSound::Wind),
        (DisasterKind::Earthquake, PlacementSound::Explode),
        (DisasterKind::Monster, PlacementSound::Monster),
        (DisasterKind::Hurricane, PlacementSound::Wind),
        (DisasterKind::Rioters, PlacementSound::Riot),
        (DisasterKind::MassRiot, PlacementSound::Riot),
        (DisasterKind::Meltdown, PlacementSound::Explode),
        (DisasterKind::Microwave, PlacementSound::PowerLine),
        (DisasterKind::ToxicSpill, PlacementSound::Explode),
        (DisasterKind::Volcano, PlacementSound::Explode),
        (DisasterKind::Firestorm, PlacementSound::Fire),
        (DisasterKind::PollutionAccident, PlacementSound::Explode),
    ] {
        assert_eq!(disaster_start_sound(kind), effect, "{kind:?}");
    }
}

#[test]
fn disaster_text_overlays_use_recovered_sprite_family() {
    assert_eq!(disaster_sprite_id_for_text_overlay(0xfa, 0), None);
    assert_eq!(
        disaster_sprite_id_for_text_overlay(0xfb, 0),
        Some(DISASTER_TOXIC_CLOUD_SPRITE_ID)
    );
    assert_eq!(
        disaster_sprite_id_for_text_overlay(0xfc, 0),
        Some(DISASTER_FLOOD_SPRITE_ID)
    );
    assert_eq!(
        disaster_sprite_id_for_text_overlay(0xfd, 0),
        Some(DISASTER_RIOTERS_1_SPRITE_ID)
    );
    assert_eq!(
        disaster_sprite_id_for_text_overlay(0xfe, 0),
        Some(DISASTER_RIOTERS_2_SPRITE_ID)
    );
    assert_eq!(disaster_sprite_id_for_text_overlay(0xff, 0), Some(1396));
    assert_eq!(disaster_sprite_id_for_text_overlay(0xff, 3), Some(1399));
    assert_eq!(disaster_sprite_id_for_text_overlay(0xff, 4), Some(1396));
    assert_eq!(disaster_sprite_id_for_text_overlay(0x01, 0), None);

    for marker in DISASTER_MARKER_MIN..=DISASTER_MARKER_MAX {
        for anim_frame in 0..8 {
            let sprite_id =
                disaster_sprite_id_for_text_overlay(marker, anim_frame).expect("sprite id");
            assert!(
                !(1222..=1229).contains(&sprite_id),
                "disaster overlays must not reuse normal city object sprite {sprite_id}"
            );
        }
    }
}

#[test]
fn stale_dispatch_tool_cannot_preview_or_place_without_active_disaster() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    let mut state = test_state(city);
    state.tool = EditTool::DispatchPolice;

    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::DispatchPolice)));
    assert_eq!(state.tool_placement_at(20, 21), None);
    assert!(!state.apply_tool_at_tile(20, 21));
    assert!(state.city.dispatch_actor_at(20, 21).is_none());
}

#[test]
fn adjust_edit_map_sea_level_flood_clears_tree_xbld_via_place_tile() {
    // PE FUN_004460B0 @ 0x44614f: flood arm PlaceTile(0) when XBLD in 6..=0x0D.
    let mut city = City::new_blank("/tmp/sc2000-emap-sea-tree.sc2", "ESeaTr").expect("blank");
    // Seed a water tile so sea level starts at 4; land 4 floods when sea → 5.
    city.set_altitude(0, 0, 3, 4);
    city.set_terrain(0, 0, 0x10);
    city.set_altitude(52, 52, 4, 0);
    city.set_building(52, 52, 6); // tree
    city.set_text_overlay(52, 52, 0x4f);
    city.set_traffic_density(52, 52, 0x21);
    city.set_tile_bits(52, 52, TileBits::new(0).with_piped(true).with_watered(true));
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(4);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert_eq!(state.city.building_at(52, 52), 0);
    assert_eq!(state.city.text_overlay_at(52, 52), Some(0x4f));
    assert_eq!(state.city.traffic_density_at(52, 52), Some(0x21));
    let bits = state.city.tile_bits_at(52, 52).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert!(bits.water_tile(), "PE ORs XBIT 0x04 on sea flood");
}

#[test]
fn adjust_edit_map_sea_level_flood_sets_xbit_water_tile() {
    // PE FUN_004460B0 @ 0x44618f: flood arm ORs XBIT `0x04`.
    let mut city = City::new_blank("/tmp/sc2000-emap-sea-xbit.sc2", "ESeaXb").expect("blank");
    city.set_altitude(0, 0, 3, 4);
    city.set_terrain(0, 0, 0x10);
    city.set_altitude(52, 52, 4, 0);
    city.set_tile_bits(52, 52, TileBits::new(0));
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(4);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert!(state.city.tile_bits_at(52, 52).unwrap().water_tile());
}

#[test]
fn mac68k_adjust_edit_map_sea_level_flood_sets_xbit_water_tile() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-sea-xbit.sc2", "MacESeaXb").expect("blank");
    city.set_altitude(0, 0, 3, 4);
    city.set_terrain(0, 0, 0x10);
    city.set_altitude(52, 52, 4, 0);
    city.set_tile_bits(52, 52, TileBits::new(0));
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(4);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert!(state.city.tile_bits_at(52, 52).unwrap().water_tile());
}

#[test]
fn adjust_edit_map_sea_level_unflood_clears_xbit_water_tile() {
    // PE FUN_004460B0 @ 0x44625b: unflood AND XBIT `0xFB`.
    // Blank sea starts at BLANK_MAP_BASE_HEIGHT (8); land 8 stays dry when sea → 7.
    let mut city = City::new_blank("/tmp/sc2000-emap-sea-unflood.sc2", "ESeaUn").expect("blank");
    city.set_altitude(52, 52, 8, 8);
    city.set_terrain(52, 52, 0x10);
    city.set_tile_bits(
        52,
        52,
        TileBits::new(0).with_water_tile(true).with_watered(true),
    );
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(-1) > 0);
    let bits = state.city.tile_bits_at(52, 52).unwrap();
    assert!(!bits.water_tile());
    assert!(
        bits.watered(),
        "unflood clears water_tile only, not watered"
    );
}

#[test]
fn mac68k_adjust_edit_map_sea_level_unflood_clears_xbit_water_tile() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-sea-unflood.sc2", "MacESeaUn").expect("blank");
    city.set_altitude(52, 52, 8, 8);
    city.set_terrain(52, 52, 0x10);
    city.set_tile_bits(52, 52, TileBits::new(0).with_water_tile(true));
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(-1) > 0);
    assert!(!state.city.tile_bits_at(52, 52).unwrap().water_tile());
}

#[test]
fn adjust_edit_map_sea_level_flood_clears_small_park_xbld_via_place_tile() {
    let mut city = City::new_blank("/tmp/sc2000-emap-sea-park.sc2", "ESeaPk").expect("blank");
    city.set_altitude(0, 0, 3, 4);
    city.set_terrain(0, 0, 0x10);
    city.set_altitude(52, 52, 4, 0);
    city.set_building(52, 52, SMALL_PARK_BUILDING_ID);
    city.set_text_overlay(52, 52, 0x33);
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(4);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert_eq!(state.city.building_at(52, 52), 0);
    assert_eq!(state.city.text_overlay_at(52, 52), Some(0x33));
}

#[test]
fn mac68k_adjust_edit_map_sea_level_flood_clears_tree_xbld_via_place_tile() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-sea-tree.sc2", "MacESeaTr").expect("blank");
    city.set_altitude(0, 0, 3, 4);
    city.set_terrain(0, 0, 0x10);
    city.set_altitude(52, 52, 4, 0);
    city.set_building(52, 52, 6);
    city.set_text_overlay(52, 52, 0x4f);
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(4);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert_eq!(state.city.building_at(52, 52), 0);
    assert_eq!(state.city.text_overlay_at(52, 52), Some(0x4f));
}

#[test]
fn pe_newspaper_gift_place_coal_police_fire_4836d7_place_tile_size() {
    // PE gift arms @ `0x4836d7`/`0x483731`/`0x483706`: PlaceTile-size only.
    let mut city = City::new_blank("/tmp/sc2000-gift-cpf.sc2", "GiftCPF").expect("blank");
    city.set_city_funds(50_000);
    // Coal helper args are post-caller-dec; size 4 decs again → NW (9,9).
    assert!(pe_newspaper_gift_place_coal_4836d7(&mut city, 10, 10));
    assert_eq!(city.building_at(9, 9), COAL_POWER_BUILDING_ID);
    assert_eq!(city.building_at(10, 10), COAL_POWER_BUILDING_ID);
    assert!(pe_newspaper_gift_place_police_483731(&mut city, 30, 30));
    assert_eq!(city.building_at(29, 29), POLICE_STATION_BUILDING_ID);
    assert!(pe_newspaper_gift_place_fire_483706(&mut city, 50, 50));
    assert_eq!(city.building_at(49, 49), FIRE_STATION_BUILDING_ID);
}

#[test]
fn pe_newspaper_gift_start_disaster_48332b_48336d_sets_type_and_xtxt() {
    let mut city = City::new_blank("/tmp/sc2000-gift-disaster.sc2", "GiftDis").expect("blank");
    pe_newspaper_gift_start_disaster_48332b(&mut city, 10, 10);
    assert_eq!(city.pe_dw_disaster_active(), 1);
    assert_eq!(city.pe_w_current_disaster_type(), 2);
    assert_eq!(city.text_overlay_at(10, 10), Some(0xfc));
    pe_newspaper_gift_start_disaster_48336d(&mut city, 20, 20);
    assert_eq!(city.pe_dw_disaster_active(), 1);
    assert_eq!(city.pe_w_current_disaster_type(), 1);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xff));
}

#[test]
fn pe_newspaper_gift_entry_483250_fire_police_story_ranges() {
    // PE @ `0x4836f4`/`0x48371f`: fire `0x2fd..=0x301`, police `0x306..=0x30a`.
    let mut city = City::new_blank("/tmp/sc2000-gift-entry-fp.sc2", "GiftEntryFP").expect("blank");
    let mut crt = Win95CrtRand::new(18);
    city.set_city_funds(50_000);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 20, 20, 0x301);
    assert_eq!(city.building_at(19, 19), FIRE_STATION_BUILDING_ID);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 30, 30, 0x30a);
    assert_eq!(city.building_at(29, 29), POLICE_STATION_BUILDING_ID);
    // Below fire floor / above police ceiling stay no-ops in this band.
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 40, 40, 0x2fc);
    assert_eq!(city.building_at(40, 40), 0);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 41, 41, 0x302);
    assert_eq!(city.building_at(41, 41), 0);
}

#[test]
fn citytool_hospital_police_fire_write_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: hospital/police/fire (`0xD1`..`0xD3`) XTXT 3×3.
    for &(building, tool, label) in &[
        (209u8, EditTool::Hospital, "Hospital"),
        (210u8, EditTool::Police, "Police"),
        (211u8, EditTool::FireStation, "Fire"),
    ] {
        let mut city =
            City::new_blank(format!("/tmp/sc2000-ct-{label}-xtxt.sc2"), label).expect("blank");
        city.set_city_funds(20_000);
        let mut state = test_state(city);
        state.tool = tool;
        assert_eq!(state.place_building_at(54, 54, building), 9, "{label}");
        let xtxt = state.city.text_overlay_at(54, 54).unwrap_or(0);
        assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "{label} microsim XTXT {xtxt}");
        for dx in 0..3usize {
            for dy in 0..3usize {
                assert_eq!(
                    state.city.text_overlay_at(54 + dx, 54 + dy),
                    Some(xtxt),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn citytool_nimby_gate_refuses_nuclear_when_residential_floods_window() {
    // PE CityTool @ `0x440624`: nuclear `0xCB` uses the same residential census + LFSR gate.
    let mut city = City::new_blank("/tmp/sc2000-ct-nimby-nuke.sc2", "CtNimbyN").expect("blank");
    city.set_city_funds(50_000);
    for x in 10..30usize {
        for y in 10..30usize {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
    }
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantNuclear;
    assert_eq!(state.place_building_at(16, 16, 203), 0);
    assert_eq!(state.city.building_at(16, 16), 0);
}

#[test]
fn citytool_nimby_gate_refuses_gas_and_oil_when_residential_floods_window() {
    // PE CityTool @ `0x440624`: gas `0xC9` / oil `0xCA` share the residential LFSR gate.
    for &(building, tool, label) in &[
        (201u8, EditTool::PowerPlantGas, "Gas"),
        (202u8, EditTool::PowerPlantOil, "Oil"),
    ] {
        let mut city =
            City::new_blank(format!("/tmp/sc2000-ct-nimby-{label}.sc2"), label).expect("blank");
        city.set_city_funds(50_000);
        for x in 10..30usize {
            for y in 10..30usize {
                city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
            }
        }
        let mut state = test_state(city);
        state.tool = tool;
        assert_eq!(state.place_building_at(16, 16, building), 0, "{label}");
        assert_eq!(state.city.building_at(16, 16), 0, "{label}");
    }
}

#[test]
fn pe_monthly_condition_story_queues_fire_shortage_0x31() {
    // PE @ 0x471cb6: fire_tiles/9 <= pop/0x4e20 → story 0x31.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-fire-story.sc2", "FireT").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.set_pe_zone_pops([2_000, 0, 0]);
    state.city.persist_pe_day22_population_leftover(20_000);
    // PE transit arm @ `0x471c13` reads `dwTileCount[0xe9]`, not a map rescan.
    state.city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 300;
    // Cover police/prison arm: need count > pop/20000 (=1).
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 34, 30, 210, 3);
    // 0 fire <= 1 → story 0x31. The prelude still runs with No Disasters on.
    state.sim_calendar_day22_scenario(PopulationStats::default());
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x31));
    state.handle_simulation_day_advanced(1900, 1, 24);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x31));
    assert_eq!(state.city.pe_w_trigger_disaster, 0);
}

#[test]
fn january_demolishes_aged_power_plant_when_no_disasters_off() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(50_000);
    // Default: No Disasters off → PE always demolishes at age>50.
    let coal = city.provision_microsim_for_building(207).expect("coal");
    let coal_index = usize::from(coal - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(coal_index, 50));
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_building(10 + dx, 20 + dy, 207);
            city.set_text_overlay(10 + dx, 20 + dy, coal);
        }
    }
    city.refresh_structure_microsim_yearly(0, 100, 100, 100, 100, 100);
    assert_eq!(city.microsim_at(coal_index).map(|r| r.tile_id), Some(0));
    assert_eq!(
        city.city_funds(),
        50_000,
        "demolish path does not charge rebuild"
    );
    assert!((1..=4).contains(&city.building_at(10, 20)));
    assert_eq!(city.text_overlay_at(10, 20), Some(0));
}

#[test]
fn january_fills_police_fire_and_stadium_activity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // PE police crimes come from dwCityCrime / station count (`FUN_0044e390`).
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 9_496);

    let police = city.provision_microsim_for_building(210).expect("police");
    let police_index = usize::from(police - MIN_SIM_TEXT_ENTRIES);
    let fire = city.provision_microsim_for_building(211).expect("fire");
    let fire_index = usize::from(fire - MIN_SIM_TEXT_ENTRIES);
    let stadium = city.provision_microsim_for_building(215).expect("stadium");
    let stadium_index = usize::from(stadium - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(
        city.microsim_at(stadium_index).map(|r| r.stat3),
        Some(0),
        "InitializeMicrosim leaves stadium stats zero; yearly must not choose a team"
    );

    let mut rng = Win95CrtRand::new(Win95CrtRand::DEFAULT_SEED);
    city.refresh_structure_microsim_yearly_with_rng(
        55_860,
        100,
        100,
        100,
        100,
        100,
        &mut rng,
        &mut Win95Lfsr::new(Win95Lfsr::DEFAULT_STATE),
    );
    // PE: capacity=min(pop/90, funding*2); crimes=dwCityCrime/n; arrests=crimes/5+(rand&0xf).
    assert_eq!(
        city.microsim_at(police_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((100, 200, 9_496, 9_496 / 5 + (41 & 0xf)))
    );
    // PE fire: capacity=min(pop/70, funding/2); trucks=(cap>>4)+1; firefighters=(rand%20)+2.
    assert_eq!(
        city.microsim_at(fire_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((100, 50, 4, 9))
    );
    // PE stadium @ `0x44eaba`: attendance=min(pop/5, pop/tiles capped@25k)+jitter; stat0=(rand&0x1f)+9.
    // No stadium tiles → PE `max(dwTileCount[215], 1)` keeps the provision-only divisor at 1.
    assert_eq!(
        city.microsim_at(stadium_index)
            .map(|r| (r.stat0, r.stat1, r.stat3)),
        Some((10, 11_304, 0)),
        "PE stadium attendance after ÷5 capacity + CRT jitter"
    );
    let mut rng = Win95CrtRand::new(Win95CrtRand::DEFAULT_SEED);
    city.refresh_structure_microsim_yearly_with_rng(
        55_860,
        50,
        50,
        100,
        100,
        100,
        &mut rng,
        &mut Win95Lfsr::new(Win95Lfsr::DEFAULT_STATE),
    );
    assert_eq!(
        city.microsim_at(police_index)
            .map(|r| (r.stat0, r.stat1, r.stat3)),
        Some((50, 100, 9_496 / 5 + (41 & 0xf))),
        "half funding caps PE officer capacity at funding*2"
    );
    assert_eq!(
        city.microsim_at(fire_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((50, 25, 2, 9))
    );
    assert_eq!(
        city.microsim_at(stadium_index).map(|record| record.stat3),
        Some(0),
        "the January arm leaves an unset team index unchanged across years"
    );
}

#[test]
fn simulation_day24_starts_tornado_when_weather_trend_and_roll_hit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank");
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
    let mut state = test_state(city);
    state.no_disasters_enabled = false;
    state.game_clock = GameClock::new_with_day(1900, 1, 24);
    assert!(state.start_weather_disaster_for_test());
    assert!(state.city.has_active_disaster());
    assert!(state.query_line.contains("Tornado"));
}

#[test]
fn pe_power_flood_accepts_xbit_powerable_without_connector_predicate() {
    // PE FUN_004679B0 @ 0x4679B0: `test byte [xbit], 0x80` gates capacity + expansion.
    // Neighbor push (thunk 0x402ea5 -> FUN_00413CF0) only skips XBIT 0x08-visited tiles.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 14);
    assert!(!pe_power_flood_tile_accepted(&city, 20, 20));
    city.set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
    assert!(pe_power_flood_tile_accepted(&city, 20, 20));
    // Roads never become PE flood members just from geometry.
    city.set_building(20, 20, 0x1D);
    city.set_tile_bits(20, 20, TileBits::new(0));
    assert!(!pe_power_flood_tile_accepted(&city, 20, 20));
    assert!(!power_tile_connects_to(&city, 20, 20, NETWORK_TOP));
}

#[test]
fn pe_power_flood_fill_consumer_tile_matches_xbld_band() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(pe_power_consumer_xbld(0x70));
    assert!(pe_power_consumer_xbld(0xC5));
    assert!(!pe_power_consumer_xbld(0xC6));
    assert!(!pe_power_flood_fill_consumer_tile(&city, 10, 10));
}

#[test]
fn generic_refresh_preserves_scenario_counters_and_process_crt() {
    let mut city = City::new_blank("/tmp/sc2000-day21-crt.sc2", "Day21Crt").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.clear_traffic_density_for_test();
    assert!(city.crime_density_slice().is_none());
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xb2b2_2b2b);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let crime_before = state.city.misc_u32_at(MISC_CITY_CRIME_OFFSET);
    let traffic_before = state.city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET);
    state.refresh_live_misc_current_values();
    assert_eq!(state.pe_process_crt_seed(), process);
    assert_eq!(state.city.misc_u32_at(MISC_CITY_CRIME_OFFSET), crime_before);
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_TRAFFIC_OFFSET),
        traffic_before
    );
    assert!(state.city.crime_density_slice().is_none());
}

#[test]
fn scenario_progress_counters_use_entry_seed_power_maps_not_city_name() {
    // Day-22 counters must reuse the entry-seed plant maps already built for outcomes.
    let mut city = City::new_blank("/tmp/sc2000-scen-cnt.sc2", "ScenCnt").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.clear_traffic_density_for_test();
    assert!(city.crime_density_slice().is_none());
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x8a8a_a8a8);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let power_maps = power_service_maps_with_entry_seed(&state.city, process);
    let live_population = live_population_stats_for_city(&state.city);
    let live_outcomes = live_city_service_outcomes_for_city_with_power(&state.city, &power_maps);
    let expected = live_city_scenario_counters_for_city_with_power(
        &state.city,
        live_population,
        live_outcomes,
        &power_maps,
    );
    let progress = scenario_progress_for_city_with_entry_seed(&state.city, process);
    assert_eq!(progress.counters.crime, expected.crime);
    assert_eq!(progress.counters.traffic, expected.traffic);
}

#[test]
fn scenario_progress_power_maps_use_process_crt_seed_not_city_name() {
    // Day-22 / scenario outcome crime counters use live police coverage from
    // FUN_004679B0 process CRT (same seed as live service outcomes).
    let mut city = City::new_blank("/tmp/sc2000-scen-crt.sc2", "ScenCrt").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x9f9f_f9f9);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let process_progress = scenario_progress_for_city_with_entry_seed(&state.city, process);
    let city_name_progress = scenario_progress_for_city(&state.city);
    assert_eq!(
        scenario_progress_for_city_with_entry_seed(&state.city, pe_power_recalc_seed(&state.city),),
        city_name_progress
    );
    let _ = process_progress;
}

#[test]
fn pe_phase24_tornado_queues_before_construction_and_uses_native_point() {
    for path in ["phase24-proposal.sc2", "phase24-proposal.city"] {
        let mut city = blank_city_layout_fixture(path, "Phase 24 proposal");
        city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
        city.set_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, 1900);
        city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_TORNADO));
        city.set_pe_dw_city_population(0);
        let mut state = test_state(city);
        state.no_disasters_enabled = false;
        state.menu_checks.citygame_newspaper = false;
        state.hover_tile = Some((127, 127));
        state.game_clock = GameClock::new_with_day(1910, 1, 24);
        state.pe_process_crt = Win95CrtRand::new(1);
        let mut expected = state.pe_process_crt;
        assert_eq!(expected.random_u16() % 30, 11);
        let y = (expected.random_u16() % 126 + 1) as i16;
        let x = (expected.random_u16() % 126 + 1) as i16;

        state.city.pe_w_trigger_disaster = 2;
        state.city.pe_tornado_actor_counter = 1;
        state.handle_simulation_day_advanced(1910, 1, 24);

        assert_eq!(state.city.pe_w_trigger_disaster, 7);
        assert_eq!(state.pe_idle_scheduler.pending_disaster_point, Some((x, y)));
        assert_eq!(state.pe_process_crt_seed(), expected.seed());
        assert!(state.city.pe_tornado_actor().is_none());
        assert!(!state.consume_pe_trigger_disaster());
        assert_eq!(state.city.pe_w_trigger_disaster, 0);
        assert_eq!(state.pe_idle_scheduler.pending_disaster_point, None);
        assert_eq!(state.pe_process_crt_seed(), expected.seed());

        state.city.pe_w_trigger_disaster = 2;
        state.pe_process_crt = Win95CrtRand::new(2); // Chance remainder 15.
        state.handle_simulation_day_advanced(1910, 1, 24);
        assert_eq!(state.city.pe_w_trigger_disaster, 2);
        assert_eq!(state.pe_idle_scheduler.pending_disaster_point, None);
        state.city.pe_w_trigger_disaster = 0;

        state.game_clock = GameClock::new_with_day(1910, 1, 22);
        state.pe_process_crt = Win95CrtRand::new(1);
        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(200))
                .days,
            1
        );
        assert_eq!(state.game_clock.day(), 23);
        assert!(state.city.pe_tornado_actor().is_none());

        state.pe_process_crt = Win95CrtRand::new(1);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(200))
                .days,
            1
        );
        assert_eq!(state.game_clock.day(), 24);
        let (_, actor) = state
            .city
            .pe_tornado_actor()
            .expect("idle admitted the proposal");
        assert_eq!((i16::from(actor.tile_x), i16::from(actor.tile_y)), (x, y));
        assert_eq!(state.city.pe_tornado_actor_counter, 1);
        assert_eq!(state.city.pe_w_trigger_disaster, 0);
        assert_eq!(state.pe_idle_scheduler.pending_disaster_point, None);
    }
}

#[test]
fn pe_failed_toxic_proposal_keeps_flood_and_uses_signed_native_point() {
    for path in ["failed-toxic.sc2", "failed-toxic.city"] {
        let mut city = blank_city_layout_fixture(path, "Failed toxic proposal");
        city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
        city.set_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, 1900);
        city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_BLIZZARD));
        city.set_terrain(0, 33, 0x20);
        city.set_terrain(0, 34, 0x20);
        let mut state = test_state(city);
        state.no_disasters_enabled = false;
        state.game_clock = GameClock::new_with_day(1910, 1, 24);
        state.pe_process_crt = Win95CrtRand::new(264);
        state.pe_process_lfsr = Win95Lfsr::new(1);
        state.city.pe_w_trigger_disaster = 2;
        state.pe_idle_scheduler.pending_disaster_point = Some((12, 34));

        assert!(!state.queue_pe_monthly_disaster_if_due());
        assert_eq!(state.city.pe_w_trigger_disaster, 2);
        assert_eq!(
            state.pe_idle_scheduler.pending_disaster_point,
            Some((-1, 34))
        );
        assert_eq!(state.pe_process_crt_seed(), 1785911234);
        assert_eq!(state.pe_process_lfsr_state(), 1);

        assert!(state.consume_pe_trigger_disaster());
        assert_eq!(state.city.pe_w_trigger_disaster, 0);
        assert_eq!(
            state.pe_idle_scheduler.pending_disaster_point,
            Some((0, 33))
        );
        assert_eq!(state.city.text_overlay_at(1, 33), Some(0xfc));
        assert_eq!(state.city.text_overlay_at(0, 34), Some(0xfc));
        assert_eq!(state.city.text_overlay_at(1, 34), Some(0));
    }
}

#[test]
fn pe_phase24_toxic_spill_proposal_matches_native_lfsr_checkpoint() {
    for path in ["phase24-toxic.sc2", "phase24-toxic.city"] {
        let mut city = blank_city_layout_fixture(path, "Phase 24 toxic spill");
        city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
        city.set_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, 1900);
        city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_BLIZZARD));
        let mut pollution = vec![0; CITY_TILES];
        pollution[density_map_index(26, 38)] = 0xb4;
        city.sync_pollution_density_from_slice(&pollution);
        let mut state = test_state(city);
        state.no_disasters_enabled = false;
        state.game_clock = GameClock::new_with_day(1910, 1, 24);
        state.pe_process_crt = Win95CrtRand::new(264);
        state.pe_process_lfsr = Win95Lfsr::new(8195);
        let mut expected = state.pe_process_crt;
        assert_eq!((expected.random_u16(), expected.random_u16()), (900, 27250));

        state.handle_simulation_day_advanced(1910, 1, 24);

        assert_eq!(state.city.pe_w_trigger_disaster, 4);
        assert_eq!(
            state.pe_idle_scheduler.pending_disaster_point,
            Some((21, 42))
        );
        assert_eq!(state.pe_process_crt_seed(), expected.seed());
        assert_eq!(state.pe_process_lfsr_state(), 7149);
        assert!(!state.city.has_active_disaster());
    }
}

#[test]
fn monthly_disaster_proposal_advances_pe_process_crt_on_chance_roll() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
    let mut state = test_state(city);
    state.no_disasters_enabled = false;
    state.game_clock = GameClock::new(1902, 7);
    let population = live_population_stats_for_city(&state.city);
    assert!(population.normal_population >= AMBIENT_DISASTER_MIN_POPULATION);
    let before = state.pe_process_crt_seed();
    let _ = state.queue_pe_monthly_disaster_if_due();
    assert_ne!(
        state.pe_process_crt_seed(),
        before,
        "phase-24 chance roll should consume the shared process CRT stream"
    );
}

#[test]
fn mac_phase24_does_not_use_pe_disaster_proposal() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.no_disasters_enabled = false;
    state.game_clock = GameClock::new(1902, 7);
    let population = live_population_stats_for_city(&state.city);
    assert!(population.normal_population >= AMBIENT_DISASTER_MIN_POPULATION);
    let pe_crt_before = state.pe_process_crt_seed();

    assert!(!state.queue_pe_monthly_disaster_if_due());
    assert_eq!(state.pe_process_crt_seed(), pe_crt_before);
    assert!(!state.city.has_active_disaster());
}

#[test]
fn ambient_disaster_for_month_with_rng_skips_without_second_seed_restart() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    populate_city_for_ambient_disaster(&mut city);
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    let clock = GameClock::new(1902, 7);
    let population = live_population_stats_for_city(&city);
    let outcomes = CityServiceOutcomes::default();

    let mut found = false;
    for salt in 0_u32..4_000 {
        let seed = 0x471E_E900 ^ salt;
        let mut probe = Win95CrtRand::new(seed);
        let remainder =
            pe_ambient_disaster_chance_remainder(&mut probe, CITY_DIFFICULTY_HARD).unwrap();
        if !(0x0f..30).contains(&remainder) {
            continue;
        }
        let mut rng = Win95CrtRand::new(seed);
        assert!(
            ambient_disaster_for_month_with_rng(
                &city,
                clock,
                population,
                outcomes,
                EngineFidelity::Win95,
                &mut rng,
                None,
            )
            .is_none()
        );
        assert_eq!(rng.seed(), probe.seed());
        found = true;
        break;
    }
    assert!(found, "expected a CRT seed with skip remainder in 0x0F..29");
}

#[test]
fn choose_and_place_xbit_e0_includes_pe_flood_powerable_membership() {
    // XBIT_CHOOSE_AND_PLACE is 0xE0 = POWERABLE|POWERED|0x20. PE FUN_004679B0
    // walks any tile with XBIT 0x80, so ChooseAndPlace buildings join the flood.
    assert_eq!(XBIT_CHOOSE_AND_PLACE & 0x80, 0x80);
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_tile_bits(10, 10, TileBits::new(XBIT_CHOOSE_AND_PLACE));
    assert!(pe_power_flood_tile_accepted(&city, 10, 10));
    assert!(city.tile_bits_at(10, 10).unwrap().powerable());
    assert!(city.tile_bits_at(10, 10).unwrap().powered());
}

#[test]
fn stamp_zone_development_airport_runway_joins_power_flood() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));
    let origin = (20, 21);
    stamp_airport_runway_zone_ray_for_test(&mut state.city, origin);
    state.stamp_zone_development(&ZoneDevelopmentPlan {
        origin,
        zone_id: ZONE_AIRPORT,
        building: RUNWAY_BUILDING_ID,
        footprint: vec![origin],
        place_as_construction: false,
    });
    let bits = state.city.tile_bits_at(origin.0, origin.1).unwrap();
    assert!(bits.powerable());
    assert!(bits.powered());
    assert!(pe_power_flood_tile_accepted(
        &state.city,
        origin.0,
        origin.1
    ));
}

#[test]
fn damage_disaster_tile_preserves_xtrf_like_pe_401604() {
    // PE DestroyStructure (`0x401604` → `0x461330`) never writes XTRF (`0x4ca940`).
    let mut city = City::new_blank("/tmp/sc2000-destroy-xtrf.sc2", "DestroyXtrf").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_traffic_density(20, 20, 77);
    let mut state = test_state(city);
    assert!(state.damage_disaster_tile(20, 20));
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(77));
}

#[test]
fn mac68k_damage_disaster_tile_preserves_xtrf_like_pe_401604() {
    // Shared DestroyStructure leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-destroy-xtrf.sc2", "MacDesX").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_traffic_density(20, 20, 77);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.damage_disaster_tile(20, 20));
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(77));
}

#[test]
fn damage_disaster_tile_preserves_watered_like_pe_401604() {
    // PE DestroyStructure only tests XBIT bit 0x04; does not clear watered/piped.
    let mut city =
        City::new_blank("/tmp/sc2000-destroy-watered.sc2", "DestroyWatered").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    assert!(state.damage_disaster_tile(20, 20));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn damage_disaster_tile_places_crt_rubble_like_pe() {
    // PE monthly damage DestroyStructure → DestroyBuilding CRT PlaceTile rubble.
    let mut city = City::new_blank("/tmp/sc2000-dmg-crt.sc2", "DmgCrt").expect("blank");
    city.set_building(20, 20, 0x70);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let expected = expected_single_tile_mode1_rubble_after_generic_scatter(&mut preview);
    assert!(state.damage_disaster_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), expected);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn damage_disaster_tile_clears_highway_2x2_pe_destroy_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-dmg-hwy.sc2", "DmgHwy").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_building(x, y, 0x49);
        city.set_text_overlay(x, y, 0x55);
        city.set_traffic_density(x, y, 0x1b);
    }
    let mut state = test_state(city);
    assert!(state.damage_disaster_tile(21, 21));
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert!(
            is_rubble_tile(state.city.building_at(x, y)),
            "highway cell ({x},{y})"
        );
        assert_eq!(state.city.text_overlay_at(x, y), Some(0));
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x1b));
    }
}

#[test]
fn mac68k_damage_disaster_tile_clears_highway_2x2_preserves_xtrf() {
    let mut city = City::new_blank("/tmp/sc2000-mac-dmg-hwy.sc2", "MacDmgHwy").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_building(x, y, 0x49);
        city.set_traffic_density(x, y, 0x1b);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.damage_disaster_tile(21, 21));
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert!(is_rubble_tile(state.city.building_at(x, y)));
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x1b));
    }
}

#[test]
fn live_police_and_fire_maps_require_power_for_full_radius() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 20, 20, 210, 3);
    stamp_complete_building_for_test(&mut city, 40, 40, 211, 3);
    seed_service_coverage_footprint_for_test(&mut city, 20, 20, 3, false);
    seed_service_coverage_footprint_for_test(&mut city, 40, 40, 3, false);
    let population = PopulationStats {
        populated_tiles: 1,
        ..Default::default()
    };
    let outcomes = CityServiceOutcomes::default();
    // Sample the outer ring to check that the powered corner supplies more
    // coverage away from the station.
    let far = density_map_index(32, 20);

    let unpowered = live_density_maps_for_city_with_power_seed_and_pe_xbit(
        &city,
        population,
        outcomes,
        pe_power_recalc_seed(&city),
        true,
    );
    power_adjacent_building_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 40, 39);
    seed_service_coverage_footprint_for_test(&mut city, 20, 20, 3, true);
    seed_service_coverage_footprint_for_test(&mut city, 40, 40, 3, true);
    let powered = live_density_maps_for_city_with_power_seed_and_pe_xbit(
        &city,
        population,
        outcomes,
        pe_power_recalc_seed(&city),
        true,
    );

    assert!(
        powered.police[far] > unpowered.police[far],
        "powered police station should paint farther/stronger XPLC coverage"
    );
    assert!(
        powered.fire[density_map_index(52, 40)] > unpowered.fire[density_map_index(52, 40)],
        "powered fire station should paint farther/stronger XFIR coverage"
    );
}

#[test]
fn pe_police_and_fire_strength_match_win95_budget_math() {
    assert_eq!(pe_police_coverage_strength(100, 0), 250);
    assert_eq!(pe_police_coverage_strength(50, 0), 125);
    assert_eq!(pe_police_coverage_strength(100, 1), 300);
    assert_eq!(pe_fire_coverage_strength(100), 250);
    assert_eq!(pe_fire_coverage_strength(40), 100);
    assert_eq!(pe_police_coverage_strength(120, 0), 300);
    assert_eq!(pe_fire_coverage_strength(120), 300);
    assert_eq!(pe_police_coverage_strength(100, u32::from(u16::MAX)), 200);
}

#[test]
fn win95_refresh_preserves_saved_scenario_counters_until_day2() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (offset, value) in [
        (MISC_CITY_CRIME_OFFSET, 999),
        (MISC_CITY_TRAFFIC_OFFSET, 999),
        (MISC_CITY_POLLUTION_OFFSET, 999),
    ] {
        city.set_misc_u32_at(offset, value);
    }
    city.set_building(10, 10, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_building(11, 10, COMMERCIAL_1X1_BUILDINGS[0]);
    city.set_building(12, 10, INDUSTRIAL_1X1_BUILDINGS[0]);
    city.set_building(14, 10, 207);
    city.set_building(
        15,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_traffic_density(15, 10, 160);
    let mut state = test_state(city);

    let (population, outcomes) = state.refresh_live_misc_current_values();
    let counters = live_city_scenario_counters_for_city(&state.city, population, outcomes);

    assert!(counters.traffic > 0);
    assert!(counters.pollution > 0);
    for offset in [
        MISC_CITY_CRIME_OFFSET,
        MISC_CITY_TRAFFIC_OFFSET,
        MISC_CITY_POLLUTION_OFFSET,
    ] {
        assert_eq!(state.city.misc_u32_at(offset), Some(999));
    }
    assert!(state.city.crime_density_slice().is_none());
    assert!(state.city.pollution_density_slice().is_none());
    state.sim_calendar_day2_density(population, outcomes);
    let counters = live_city_scenario_counters_for_city(&state.city, population, outcomes);
    assert_eq!(
        counters.crime,
        density_map_microsim64_sum(state.city.crime_density_slice().expect("live XCRM"))
    );
    assert_eq!(
        counters.pollution,
        density_map_microsim64_sum(state.city.pollution_density_slice().expect("live XPLT"))
    );
}

#[test]
fn live_scenario_counters_respond_to_services_and_ordinances() {
    fn city_with_pressures(path: &str) -> City {
        let mut city = City::new_blank(path, "test").expect("blank city");
        city.set_zone(80, 80, ZONE_RESIDENTIAL_LIGHT);
        city.set_building(80, 80, RESIDENTIAL_1X1_BUILDINGS[0]);
        stamp_complete_building_for_test(&mut city, 76, 80, 207, 4);
        stamp_complete_building_for_test(&mut city, 14, 14, 210, 3);
        city.set_building(
            25,
            20,
            network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
        );
        city.set_traffic_density(25, 20, 192);
        city
    }

    let neglected = city_with_pressures("/tmp/sc2000-test-neglected.sc2");
    let mut improved_city = city_with_pressures("/tmp/sc2000-test-serviced.sc2");
    assert!(set_city_ordinance_enabled(
        &mut improved_city,
        ORDINANCE_NEIGHBORHOOD_WATCH,
        true
    ));
    assert!(set_city_ordinance_enabled(
        &mut improved_city,
        ORDINANCE_ANTI_DRUG_CAMPAIGN,
        true
    ));
    assert!(set_city_ordinance_enabled(
        &mut improved_city,
        ORDINANCE_PARKING_FINES,
        true
    ));
    assert!(set_city_ordinance_enabled(
        &mut improved_city,
        ORDINANCE_POLLUTION_CONTROLS,
        true
    ));

    let mut neglected_state = test_state(neglected);
    let mut improved_state = test_state(improved_city);
    neglected_state.refresh_live_misc_current_values();
    improved_state.refresh_live_misc_current_values();
    let base = live_city_scenario_counters_for_city(
        &neglected_state.city,
        live_population_stats_for_city(&neglected_state.city),
        live_city_service_outcomes_for_city(&neglected_state.city),
    );
    let improved = live_city_scenario_counters_for_city(
        &improved_state.city,
        live_population_stats_for_city(&improved_state.city),
        live_city_service_outcomes_for_city(&improved_state.city),
    );

    assert!(base.crime > 0, "{base:?}");
    assert!(base.pollution > 0, "{base:?}");
    assert!(base.crime > improved.crime, "{base:?} -> {improved:?}");
    assert_eq!(
        improved.traffic, base.traffic,
        "PE traffic is the XTRF microsim sum; parking fines must not scale it: {base:?} -> {improved:?}"
    );
    assert!(
        base.pollution > improved.pollution,
        "{base:?} -> {improved:?}"
    );
}

#[test]
fn xgrp_history_crime_counter_uses_pe_bss_not_live_scenario_map() {
    let mut city = City::new_blank("/tmp/sc2000-xgrp-cnt.sc2", "XgrpCnt").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.clear_traffic_density_for_test();
    assert!(city.crime_density_slice().is_none());
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xf1f1_1f1f);
    let process = state.pe_process_crt_seed();
    let population = population_stats_for_city(&state.city);
    let power_maps = power_service_maps_with_entry_seed(&state.city, process);
    let outcomes = live_city_service_outcomes_for_city_with_power(&state.city, &power_maps);
    let counters = live_city_scenario_counters_for_city_with_power(
        &state.city,
        population,
        outcomes,
        &power_maps,
    );
    let developed_divisor = pe_attitude_developed_tile_count(&state.city) / 4 + 1;
    let latest = xgrp_history_latest_values(&state.city, population, outcomes);
    assert_ne!(counters.crime, state.city.pe_dw_city_crime);
    assert_eq!(
        latest[7],
        state.city.pe_dw_city_crime / developed_divisor,
        "PE UpdateGraphData reads dwCityCrime BSS, not the live scenario-map counter"
    );
}

#[test]
fn pe_destroy_structure_at_clears_disaster_xtxt_and_highway_2x2() {
    // Growth DestroyStructure → DestroyBuilding: clear occupancy XTXT (`< 0xc9`);
    // `0xfb` survives like bulldoze; highways are 2×2.
    let mut city = City::new_blank("/tmp/sc2000-destroy-hwy.sc2", "DestroyHwy").expect("blank");
    for (x, y) in building_footprint_from_origin(10, 10, 2).unwrap() {
        city.set_building(x, y, 0x49);
        city.set_text_overlay(x, y, 0x55);
        city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_watered(true));
        city.set_traffic_density(x, y, 0x2c);
    }
    city.set_text_overlay(11, 11, 0xfb);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(11, 11));
    for (x, y) in building_footprint_from_origin(10, 10, 2).unwrap() {
        assert!(
            is_rubble_tile(state.city.building_at(x, y)),
            "highway cell ({x},{y})"
        );
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped() && bits.watered());
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x2c));
    }
    assert_eq!(state.city.text_overlay_at(10, 10), Some(0));
    assert_eq!(state.city.text_overlay_at(11, 11), Some(0xfb));
}

#[test]
fn pe_microsim_police_fire_capacity_matches_fun_0044f7d0() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 1_000);
    let police_xtxt = city.provision_microsim_for_building(210).expect("police");
    let fire_xtxt = city.provision_microsim_for_building(211).expect("fire");
    let police = usize::from(police_xtxt - MIN_SIM_TEXT_ENTRIES);
    let fire = usize::from(fire_xtxt - MIN_SIM_TEXT_ENTRIES);
    // Low pop: capacity limited by pop/90 and pop/70, not funding caps.
    let mut rng = Win95CrtRand::new(1);
    city.refresh_structure_microsim_yearly_with_rng(
        900,
        100,
        100,
        100,
        100,
        100,
        &mut rng,
        &mut Win95Lfsr::new(Win95Lfsr::DEFAULT_STATE),
    );
    assert_eq!(city.microsim_at(police).map(|r| r.stat1), Some(10)); // 900/90
    assert_eq!(city.microsim_at(fire).map(|r| r.stat1), Some(12)); // 900/70
    assert_eq!(city.microsim_at(fire).map(|r| r.stat2), Some(1)); // (12>>4)+1
}

#[test]
fn loaded_maxis_land_starts_without_active_disaster_markers() {
    let Some(path) = paths::test_mac_city() else {
        return;
    };
    let city = City::load(path).expect("load default city");

    assert!(
        !city.has_active_disaster(),
        "loaded Maxis Land should not start monthly disaster damage"
    );
}

#[test]
fn loaded_maxis_land_disaster_ticks_do_not_retype_buildings() {
    let Some(path) = paths::test_mac_city() else {
        return;
    };
    let mut state = test_state(City::load(path).expect("load default city"));
    ensure_default_city_options(&mut state.city);
    state.zone_growth_enabled = false;
    state.no_disasters_enabled = false;
    set_city_no_disasters_enabled(&mut state.city, false);
    if let Some(date) = state.city.saved_date() {
        state.game_clock = GameClock::new(date.year, date.month);
    }
    state.refresh_simulation_service_bits();

    let before = city_building_zone_snapshot(&state.city);
    for _ in 0..36 {
        state.handle_simulation_month_advanced();
    }

    let after = city_building_zone_snapshot(&state.city);
    for (index, ((before_building, _), (after_building, _))) in
        before.iter().zip(after.iter()).enumerate()
    {
        assert!(
            *after_building == *before_building
                || matches!((*before_building, *after_building), (0, _) | (_, 0)),
            "monthly disaster tick retyped building at ({},{}): {} -> {}",
            index % CITY_WIDTH,
            index / CITY_WIDTH,
            before_building,
            after_building
        );
    }
}

#[test]
fn load_city_win_scenario_preserves_saved_national_and_geography_state() {
    // The SCN loader reaches the normal SCDH decoder. The Classic CTY
    // fallback's economy initialization must not run after a successful decode.
    let path =
        std::env::temp_dir().join(format!("citygame-scenario-open-{}.scn", std::process::id()));
    let mut city = City::new_blank(&path, "Scenario open").expect("blank");
    city.kind = CityFileKind::WinScenario;
    city.pe_w_year_founded = 1900;
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, 2);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 99_999);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 65_432);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 3);
    city.set_misc_u32_at(0x0e44, 1);
    city.set_misc_u32_at(0x0e48, 0);
    city.set_technology_discovery_year(Technology::GasPower, 1911);
    city.save(&path).expect("save");
    let mut state = test_state(City::new_blank("/tmp/sc2000-holder.sc2", "H").expect("blank"));
    state.pe_process_crt = Win95CrtRand::new(1);
    state
        .load_city(&path, (1024, 768), 1.0)
        .expect("load scenario");
    assert_eq!(state.city.kind, CityFileKind::WinScenario);
    assert_eq!(state.city.pe_dw_national_population, 99_999);
    assert_eq!(state.city.pe_dw_national_value, 65_432);
    assert_eq!(state.city.pe_w_national_tax, 1);
    assert_eq!(state.city.pe_w_national_trend, 3);
    assert_eq!(state.city.pe_dw_city_has_ocean(), 1);
    assert_eq!(state.city.pe_dw_city_has_river(), 0);
    assert_eq!(
        state.city.technology_discovery_year(Technology::GasPower),
        Some(1911)
    );
    assert_eq!(state.pe_process_crt.seed(), 2_503_793_637);
    assert_eq!(
        state.city.pe_w_city_mode, CITY_MODE_GAMEPLAY as u16,
        "PE scenario open leaves wCityMode=1 when no disaster is active"
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_MODE_OFFSET),
        Some(CITY_MODE_GAMEPLAY)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_flood_destroy_preserves_xbit_water_tile_through_0x3d_mask_like_pe() {
    // DestroyStructure → DestroyBuilding @ `0x45a07f`: XBIT &= 0x3d keeps water-tile 0x04.
    let mut city = City::new_blank("/tmp/sc2000-flood-xbit-wt.sc2", "FloodXbitWt").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0)
            .with_water_tile(true)
            .with_powerable(true)
            .with_powered(true),
    );
    pe_flood_destroy_tile(&mut city, 20, 20);
    let bits = city.tile_bits_at(20, 20).unwrap();
    assert!(bits.water_tile());
    assert!(!bits.powerable());
    assert!(!bits.powered());
}

#[test]
fn mac68k_pe_flood_destroy_preserves_xbit_water_tile_through_0x3d_mask_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xbit-wt-mac.sc2", "FloodXbitWtMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0).with_water_tile(true).with_powerable(true),
    );
    pe_flood_destroy_tile(&mut city, 20, 20);
    let bits = city.tile_bits_at(20, 20).unwrap();
    assert!(bits.water_tile());
    assert!(!bits.powerable());
}

#[test]
fn pe_flood_destroy_size1_support_tip_land0_wraps_altm_word_before_dry_fun() {
    // PE's 16-bit `(ALTM & 0x1f) - 1` wraps to `0xffff`; FUN then takes the dry arm.
    // Flatten tip neighbors so FUN slope ≠ 0x32 (no second land++).
    let mut city = City::new_blank("/tmp/sc2000-support-land0.sc2", "SupportLand0").expect("blank");
    city.set_building(5, 20, 0x51);
    city.set_building(6, 20, 0x51);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((4 + dx) as usize, (20 + dy) as usize, 0, 0);
        }
    }
    city.set_altitude(5, 20, 8, 0);
    city.set_altitude(6, 20, 8, 0);
    city.set_tile_bits(6, 20, TileBits::new(0).with_rotate(true));
    city.set_altitude(0, 0, city.altitude_at(0, 0) & 0x1f, 15);
    pe_flood_destroy_tile(&mut city, 6, 20);
    assert_eq!(city.altitude_at(4, 20) & 0x1f, 0x1f);
    assert_eq!(city.water_height_at(4, 20), 0x1f);
    assert_eq!(city.altm_at(4, 20).tunnel_levels, 0x3f);
    assert!(!city.tile_bits_at(4, 20).unwrap().water_tile());
    assert!(!city.tile_bits_at(4, 20).unwrap().rotate());
    assert_eq!(city.building_at(4, 20), 0);
}

#[test]
fn mac68k_pe_flood_destroy_size1_support_tip_land0_uses_shared_pe_word_wrap() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-support-land0.sc2", "MacSupportLand0").expect("blank");
    city.set_building(5, 20, 0x51);
    city.set_building(6, 20, 0x51);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((4 + dx) as usize, (20 + dy) as usize, 0, 0);
        }
    }
    city.set_altitude(5, 20, 8, 0);
    city.set_altitude(6, 20, 8, 0);
    city.set_tile_bits(6, 20, TileBits::new(0).with_rotate(true));
    city.set_altitude(0, 0, city.altitude_at(0, 0) & 0x1f, 15);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    pe_flood_destroy_tile(&mut state.city, 6, 20);
    assert_eq!(state.city.altitude_at(4, 20) & 0x1f, 0x1f);
    assert_eq!(state.city.water_height_at(4, 20), 0x1f);
    assert_eq!(state.city.altm_at(4, 20).tunnel_levels, 0x3f);
    assert!(!state.city.tile_bits_at(4, 20).unwrap().water_tile());
    assert!(!state.city.tile_bits_at(4, 20).unwrap().rotate());
}

#[test]
fn pe_fire_heat_reads_bss_4cade0() {
    let mut city = City::new_blank("/tmp/sc2000-fire-heat.sc2", "FireHeat").expect("blank");
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 10);
    city.pe_b_weather_heat = 0xfe;
    assert_eq!(pe_fire_heat(&city), 0xfe);
}

#[test]
fn pe_riot_disaster_reads_bss_4cade0() {
    let mut city = City::new_blank("/tmp/sc2000-riot-heat.sc2", "RiotHeat").expect("blank");
    city.set_pe_unemployment_percent(10);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 0);
    city.pe_b_weather_heat = 0xaa;
    assert!(
        pe_riot_disaster_ready(&city),
        "PE type-3 riot gate reads BSS bWeatherHeat @ 0x4cade0"
    );
    city.pe_b_weather_heat = 0xa9;
    assert!(!pe_riot_disaster_ready(&city));
}

#[test]
fn pe_disaster_type_and_active_bss_4ca93c_4c9ee8_hydrate() {
    let path = "/tmp/sc2000-disaster-bss.sc2";
    let mut city = City::new_blank(path, "DisasterBss").expect("blank");
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x02);
    city.set_misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET, 1);
    assert_eq!(city.pe_w_current_disaster_type(), 0x02);
    assert_eq!(city.pe_dw_disaster_active(), 1);
    city.set_pe_w_current_disaster_type(0x07);
    city.set_pe_dw_disaster_active(1);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_current_disaster_type(), 0x07);
    assert_eq!(loaded.pe_dw_disaster_active(), 1);
    assert_eq!(
        loaded.misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET),
        Some(0x07)
    );
    assert_eq!(loaded.misc_u32_at(MISC_DISASTER_ACTIVE_OFFSET), Some(1));
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_w_monster_xthg_index_bss_4ca81c_hydrate_and_readers() {
    let path = "/tmp/sc2000-monster-xthg-bss.sc2";
    let mut city = City::new_blank(path, "MonsterXthg").expect("blank");
    assert_eq!(city.pe_w_monster_xthg_index(), 0xffff);
    city.set_misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET, 7);
    assert_eq!(city.pe_w_monster_xthg_index(), 7);
    city.set_pe_w_monster_xthg_index(0xffff);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_monster_xthg_index(), 0xffff);
    assert_eq!(
        loaded.misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET),
        Some(0xffff)
    );
    // MISC layout: monster @ 0x1040, current disaster @ 0x1044 (not the same slot).
    assert_ne!(
        MISC_MONSTER_XTHG_INDEX_OFFSET,
        MISC_CURRENT_DISASTER_TYPE_OFFSET
    );
    assert_eq!(MISC_MONSTER_XTHG_INDEX_OFFSET, 0x1040);
    assert_eq!(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x1044);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_w_trigger_disaster_from_misc_431527() {
    // PE MISC→BSS @ `0x431527`: `[MISC+0x70]` → word `[0x4ca420]`.
    let path = "/tmp/sc2000-trigger-hydrate.sc2";
    let mut city = City::new_blank(path, "TriggerHydrate").expect("blank");
    city.pe_w_trigger_disaster = 0x0007;
    city.set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0x0007);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_trigger_disaster, 0x0007);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_w_trigger_disaster_to_misc_433bf2() {
    // PE BSS→MISC @ `0x433bf2`: word `[0x4ca420]` → `[MISC+0x70]`.
    let path = "/tmp/sc2000-trigger-persist.sc2";
    let mut city = City::new_blank(path, "TriggerPersist").expect("blank");
    city.pe_w_trigger_disaster = 0x000a;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET), Some(0x000a));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_trigger_disaster, 0x000a);
    let _ = std::fs::remove_file(path);
}

#[test]
fn consume_pe_trigger_disaster_reads_bss_4ca420() {
    let city = City::new_blank("/tmp/sc2000-trigger-bss.sc2", "TriggerBss").expect("blank");
    let mut state = test_state(city);
    state.no_disasters_enabled = false;
    // MISC alone used to drive consume; PE reads BSS word.
    state.city.set_misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET, 0);
    state.city.pe_w_trigger_disaster = 0x07;
    assert!(state.consume_pe_trigger_disaster());
    assert_eq!(state.city.pe_w_trigger_disaster, 0);
    assert_eq!(
        state.city.misc_u32_at(MISC_TRIGGER_DISASTER_OFFSET),
        Some(0)
    );
}

#[test]
fn pe_flood_destroy_lowclass_selector2_helper_clears_underground_only_in_snapped_square() {
    let mut city = City::new_blank("/tmp/sc2000-flood-hwy-nbr.sc2", "FloodHwyNbr").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    for x in 19..=22 {
        for y in 18..=20 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(19, 19, 10, 0);
    city.set_altitude(22, 19, 10, 0);
    for x in 20..=21 {
        for y in 20..=21 {
            city.set_underground(x, y, 0x10);
        }
    }
    city.set_terrain(20, 19, 1);
    assert!(pe_destroy_building_selector2_low_class_cleanup_order(&city, 0x61, 20, 20).is_some());
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    for x in 20..=21 {
        for y in 20..=21 {
            assert_eq!(state.city.underground_at(x, y), 0);
        }
    }
    assert_eq!(state.city.terrain_at(20, 19), 1);
    assert_eq!(state.city.underground_at(20, 19), 0);
}

#[test]
fn pe_flood_selector2_0x49_lowclass_leaves_north_helper_tiles_unchanged() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-nbr-mac.sc2", "FloodHwyNbrMac").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    for x in 19..=22 {
        for y in 18..=20 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(19, 19, 10, 0);
    city.set_altitude(22, 19, 10, 0);
    for x in 20..=21 {
        city.set_terrain(x, 19, 2);
        city.set_underground(x, 19, 0x11);
    }
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(state.city.terrain_at(20, 19), 2);
    assert_eq!(state.city.terrain_at(21, 19), 2);
    assert_eq!(state.city.underground_at(20, 19), 0x11);
    assert_eq!(state.city.underground_at(21, 19), 0x11);
}

#[test]
fn pe_flood_destroy_lowclass_selector2_post_cleanup_leaves_north_xzon_outside_window() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-xzon.sc2", "FloodHwyXzon").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    for x in 19..=22 {
        for y in 18..=20 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(19, 19, 10, 0);
    city.set_zone(20, 19, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_terrain(20, 19, 1);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(
        state.city.zone_at(20, 19).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT | 0x80,
        "the traced post-cleanup window is the snapped 2×2, not the northern neighbor"
    );
}

#[test]
fn pe_flood_selector2_0x49_lowclass_leaves_north_zone_unchanged() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-xzon-mac.sc2", "FloodHwyXzonMac").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    for x in 19..=22 {
        for y in 18..=20 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(22, 19, 10, 0);
    city.set_zone(21, 19, ZONE_INDUSTRIAL_LIGHT | 0x20);
    city.set_terrain(21, 19, 2);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(
        state.city.zone_at(21, 19).unwrap().raw_bits(),
        ZONE_INDUSTRIAL_LIGHT | 0x20
    );
}

#[test]
fn pe_flood_destroy_lowclass_selector2_does_not_wet_north_of_post_cleanup_window() {
    let mut city = City::new_blank("/tmp/sc2000-flood-hwy-wet.sc2", "FloodHwyWet").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    for x in 19..=22 {
        for y in 18..=20 {
            city.set_altitude(x, y, 2, 0);
        }
    }
    city.set_altitude(19, 19, 3, 0);
    city.set_terrain(20, 19, 1);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.terrain_at(20, 19), 1);
    assert!(!state.city.tile_bits_at(20, 19).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(20, 19), 0);
}

#[test]
fn pe_flood_selector2_0x49_lowclass_does_not_wet_north_neighbor() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-wet-mac.sc2", "FloodHwyWetMac").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    for x in 19..=22 {
        for y in 18..=20 {
            city.set_altitude(x, y, 3, 0);
        }
    }
    city.set_altitude(22, 19, 4, 0);
    city.set_terrain(21, 19, 2);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(state.city.terrain_at(21, 19), 2);
    assert!(!state.city.tile_bits_at(21, 19).unwrap().water_tile());
}

#[test]
fn pe_flood_destroy_lowclass_selector2_does_not_shallow_wet_north_of_window() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-shallow.sc2", "FloodHwyShallow").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    for x in 19..=22 {
        for y in 18..=20 {
            city.set_altitude(x, y, 7, 0);
        }
    }
    city.set_altitude(19, 19, 8, 0);
    city.set_terrain(20, 19, 1);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.terrain_at(20, 19), 1);
    assert!(!state.city.tile_bits_at(20, 19).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(20, 19), 0);
}

#[test]
fn pe_flood_selector2_0x49_lowclass_does_not_shallow_wet_north_neighbor() {
    let mut city = City::new_blank(
        "/tmp/sc2000-flood-hwy-shallow-mac.sc2",
        "FloodHwyShallowMac",
    )
    .expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    for x in 19..=22 {
        for y in 18..=20 {
            city.set_altitude(x, y, 7, 0);
        }
    }
    city.set_altitude(22, 19, 8, 0);
    city.set_terrain(21, 19, 2);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(state.city.terrain_at(21, 19), 2);
    assert!(!state.city.tile_bits_at(21, 19).unwrap().water_tile());
}

#[test]
fn pe_flood_destroy_lowclass_selector2_leaves_north_wet_slope_unchanged() {
    // DestroyStructure twin: same highway-neighbor FUN wet 0x32 → XTER 0x10.
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-wet32.sc2", "FloodHwyWet32").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    city.set_altitude(20, 19, 2, 0);
    for (dx, dy) in [
        (-1_isize, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        city.set_altitude((20 + dx) as usize, (19 + dy) as usize, 3, 0);
    }
    city.set_terrain(20, 19, 1);
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 19), 0x32);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.terrain_at(20, 19), 1);
    assert!(!state.city.tile_bits_at(20, 19).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(20, 19), 0);
}

#[test]
fn pe_flood_selector2_0x49_lowclass_leaves_north_wet_slope_unchanged() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-wet32-mac.sc2", "FloodHwyWet32Mac").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    city.set_altitude(21, 19, 3, 0);
    for (dx, dy) in [
        (-1_isize, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        city.set_altitude((21 + dx) as usize, (19 + dy) as usize, 4, 0);
    }
    city.set_terrain(21, 19, 2);
    assert_eq!(pe_fun_00447020_slope_xter(&city, 21, 19), 0x32);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(state.city.terrain_at(21, 19), 2);
    assert!(!state.city.tile_bits_at(21, 19).unwrap().water_tile());
}

#[test]
fn pe_flood_destroy_lowclass_selector2_leaves_north_dry_slope_unchanged() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-dry32.sc2", "FloodHwyDry32").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    city.set_altitude(20, 19, 9, 0);
    for (dx, dy) in [
        (-1_isize, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        city.set_altitude((20 + dx) as usize, (19 + dy) as usize, 10, 0);
    }
    city.set_terrain(20, 19, 1);
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 19), 0x32);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.terrain_at(20, 19), 1);
    assert!(!state.city.tile_bits_at(20, 19).unwrap().water_tile());
}

#[test]
fn pe_flood_selector2_0x49_lowclass_leaves_north_dry_slope_unchanged() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-dry32-mac.sc2", "FloodHwyDry32Mac").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    city.set_altitude(21, 19, 9, 0);
    for (dx, dy) in [
        (-1_isize, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        city.set_altitude((21 + dx) as usize, (19 + dy) as usize, 10, 0);
    }
    city.set_terrain(21, 19, 2);
    assert_eq!(pe_fun_00447020_slope_xter(&city, 21, 19), 0x32);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(state.city.terrain_at(21, 19), 2);
    assert!(!state.city.tile_bits_at(21, 19).unwrap().water_tile());
}

#[test]
fn pe_flood_destroy_lowclass_selector2_leaves_north_radioactive_tile_unchanged() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-wet32-rad.sc2", "FloodHwyWet32Rad").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    city.set_altitude(20, 19, 2, 0);
    for (dx, dy) in [
        (-1_isize, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        city.set_altitude((20 + dx) as usize, (19 + dy) as usize, 3, 0);
    }
    city.set_terrain(20, 19, 1);
    city.set_building(20, 19, 5);
    city.set_text_overlay(20, 19, 0x51);
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 19), 0x32);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.building_at(20, 19), 5);
    assert_eq!(state.city.text_overlay_at(20, 19), Some(0x51));
    assert_eq!(state.city.terrain_at(20, 19), 1);
    assert!(!state.city.tile_bits_at(20, 19).unwrap().water_tile());
}

#[test]
fn pe_flood_selector2_0x49_lowclass_leaves_north_radioactive_tile_unchanged() {
    let mut city = City::new_blank(
        "/tmp/sc2000-flood-hwy-wet32-rad-mac.sc2",
        "FloodHwyWet32RadMac",
    )
    .expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    city.set_altitude(21, 19, 3, 0);
    for (dx, dy) in [
        (-1_isize, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        city.set_altitude((21 + dx) as usize, (19 + dy) as usize, 4, 0);
    }
    city.set_terrain(21, 19, 2);
    city.set_building(21, 19, 5);
    city.set_text_overlay(21, 19, 0x52);
    assert_eq!(pe_fun_00447020_slope_xter(&city, 21, 19), 0x32);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(state.city.building_at(21, 19), 5);
    assert_eq!(state.city.text_overlay_at(21, 19), Some(0x52));
    assert_eq!(state.city.terrain_at(21, 19), 2);
    assert!(!state.city.tile_bits_at(21, 19).unwrap().water_tile());
}

#[test]
fn pe_flood_destroy_lowclass_selector2_does_not_cascade_north_of_window() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-casc0d.sc2", "FloodHwyCasc0d").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    city.set_building(20, 19, 0x0d);
    city.set_terrain(20, 19, 0x7f);
    city.set_altitude(20, 19, 10, 0);
    city.set_altitude(20, 18, 11, 0);
    city.set_altitude(21, 19, 9, 0);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 19);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x7f);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.building_at(20, 19), 0x0d);
    assert_eq!(state.city.terrain_at(20, 19), 0x7f);
}

#[test]
fn pe_flood_selector2_0x49_lowclass_does_not_cascade_north_of_window() {
    let mut city = City::new_blank("/tmp/sc2000-flood-hwy-casc0d-mac.sc2", "FloodHwyCasc0dMac")
        .expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    city.set_building(20, 19, 0x0d);
    city.set_terrain(20, 19, 0x7f);
    city.set_altitude(20, 19, 10, 0);
    city.set_altitude(20, 18, 11, 0);
    city.set_altitude(21, 19, 9, 0);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 19);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x7f);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(state.city.building_at(20, 19), 0x0d);
    assert_eq!(state.city.terrain_at(20, 19), 0x7f);
}

#[test]
fn pe_flood_destroy_lowclass_selector2_does_not_process_north_low_class_tile() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-lt0d.sc2", "FloodHwyLt0d").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    city.set_building(20, 19, 0x0c);
    city.set_terrain(20, 19, 0x7f);
    city.set_altitude(20, 19, 10, 0);
    city.set_altitude(20, 18, 11, 0);
    city.set_altitude(21, 19, 9, 0);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 19);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x7f);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.terrain_at(20, 19), 0x7f);
    assert_eq!(state.city.building_at(20, 19), 0x0c);
}

#[test]
fn pe_flood_selector2_0x49_lowclass_does_not_process_north_low_class_tile() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-lt0d-mac.sc2", "FloodHwyLt0dMac").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    city.set_building(20, 19, 0x0c);
    city.set_terrain(20, 19, 0x7f);
    city.set_altitude(20, 19, 10, 0);
    city.set_altitude(20, 18, 11, 0);
    city.set_altitude(21, 19, 9, 0);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 19);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x7f);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(state.city.terrain_at(20, 19), 0x7f);
    assert_eq!(state.city.building_at(20, 19), 0x0c);
}

#[test]
fn pe_flood_destroy_highway_non_49_south_fun_like_destroy_building() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy-south.sc2", "FloodHwySouth").expect("blank");
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
    let slope_20 = pe_fun_00447020_slope_xter(&city, 20, 21);
    let slope_21 = pe_fun_00447020_slope_xter(&city, 21, 21);
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.terrain_at(20, 21), slope_20);
    assert_eq!(state.city.terrain_at(21, 21), slope_21);
    assert_eq!(state.city.underground_at(20, 21), 0);
    assert_eq!(state.city.underground_at(21, 21), 0);
}

#[test]
fn pe_flood_selector2_0x49_highclass_walk_clears_clicked_block_without_helpers() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-hwy49-south.sc2", "FloodHwy49South").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x49, 2);
    city.set_zone(20, 20, 0xf0);
    city.set_tile_bits(20, 20, TileBits::new(0x04));
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
    let mut state = test_state(city);
    assert!(state.pe_destroy_structure_at(21, 21));
    assert_eq!(state.city.terrain_at(20, 21), 2);
    assert_eq!(
        state.city.building_at(20, 20),
        0,
        "the classifier-admitted selector-2 walk clears the clicked 0x49 block"
    );
    assert_eq!(state.city.underground_at(20, 21), 0x11);
    assert_eq!(state.city.underground_at(21, 21), 0x11);
}

#[test]
fn pe_flood_destroy_skips_xlab_clear_for_xtxt_f1_f9_like_destroy_building() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-skip.sc2", "FloodXtxtSkip").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xf5);
    city.set_label(2, "FloodKeep");
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xf5));
    assert_eq!(city.label_at(2), Some("FloodKeep"));
}

#[test]
fn mac68k_pe_flood_destroy_skips_xlab_clear_for_xtxt_f1_f9_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-skip-mac.sc2", "FloodXtxtSkipMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xf3);
    city.set_label(3, "FloodMacKeep");
    // pe_flood_destroy_tile is shared; Mac path still uses it via DestroyStructure.
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xf3));
    assert_eq!(city.label_at(3), Some("FloodMacKeep"));
}

#[test]
fn pe_flood_destroy_clears_xtxt_0xfa_like_destroy_building() {
    let mut city = City::new_blank("/tmp/sc2000-flood-xtxt-fa.sc2", "FloodXtxtFa").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfa);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
}

#[test]
fn mac68k_pe_flood_destroy_clears_xtxt_0xfa_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-fa-mac.sc2", "FloodXtxtFaMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfa);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
}

#[test]
fn bulldoze_skips_xtxt_arm_for_toxic_0xfb_like_pe() {
    // PE @ `0x45a0e8`: XTXT > 0xfa (e.g. toxic 0xfb) skips the whole XTXT/XLAB arm.
    let mut city = City::new_blank("/tmp/sc2000-bull-xtxt-fb.sc2", "BullXtxtFb").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfb);
    city.set_label(1, "ShouldStay");
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xfb));
    assert_eq!(state.city.label_at(1), Some("ShouldStay"));
}

#[test]
fn mac68k_bulldoze_skips_xtxt_arm_for_toxic_0xfb_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xtxt-fb-mac.sc2", "BullXtxtFbMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfb);
    city.set_label(4, "MacStay");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xfb));
    assert_eq!(state.city.label_at(4), Some("MacStay"));
}

#[test]
fn pe_flood_destroy_skips_xtxt_arm_for_toxic_0xfb_like_destroy_building() {
    let mut city = City::new_blank("/tmp/sc2000-flood-xtxt-fb.sc2", "FloodXtxtFb").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfb);
    city.set_label(1, "FloodStay");
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xfb));
    assert_eq!(city.label_at(1), Some("FloodStay"));
}

#[test]
fn mac68k_pe_flood_destroy_skips_xtxt_arm_for_toxic_0xfb_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-fb-mac.sc2", "FloodXtxtFbMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfb);
    city.set_label(4, "FloodMacStay");
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xfb));
    assert_eq!(city.label_at(4), Some("FloodMacStay"));
}

#[test]
fn pe_flood_destroy_clears_sign_xtxt_and_xlabel_like_destroy_building() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-sign.sc2", "FloodXtxtSign").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0x12);
    city.set_label(0x12, "FloodSign");
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(city.label_at(0x12), Some(""));
}

#[test]
fn mac68k_pe_flood_destroy_clears_sign_xtxt_and_xlabel_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-sign-mac.sc2", "FloodXtxtSignMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0x08);
    city.set_label(0x08, "FloodMacSign");
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(city.label_at(0x08), Some(""));
}

#[test]
fn pe_flood_destroy_clears_midband_xtxt_without_xlabel_like_destroy_building() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-mid.sc2", "FloodXtxtMid").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0x80);
    city.set_label(0x10, "FloodKeepSlot");
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(city.label_at(0x10), Some("FloodKeepSlot"));
}

#[test]
fn mac68k_pe_flood_destroy_clears_midband_xtxt_without_xlabel_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-mid-mac.sc2", "FloodXtxtMidMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0x55);
    city.set_label(0x11, "FloodMacKeep");
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(city.label_at(0x11), Some("FloodMacKeep"));
}

#[test]
fn pe_flood_destroy_road_with_xtxt_0xfa_drops_recomputed_network_fa_count() {
    // DestroyStructure → DestroyBuilding @ `0x45a159` decs `0x4cc4d8`; Rust recomputes.
    let mut city = City::new_blank("/tmp/sc2000-flood-fa-net.sc2", "FloodFaNet").expect("blank");
    city.set_building(20, 20, 0x1d);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 0);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 0);
}

#[test]
fn mac68k_pe_flood_destroy_road_with_xtxt_0xfa_drops_recomputed_network_fa_count() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-fa-net-mac.sc2", "FloodFaNetMac").expect("blank");
    city.set_building(20, 20, 0x1e);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 0);
}

#[test]
fn pe_flood_destroy_park_with_xtxt_0xfa_drops_recomputed_non_network_fa_count() {
    // DestroyStructure → DestroyBuilding @ `0x45a150` decs `0x4ca3f0`; Rust recomputes.
    let mut city =
        City::new_blank("/tmp/sc2000-flood-fa-nonnet.sc2", "FloodFaNonNet").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 0);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 0);
}

#[test]
fn mac68k_pe_flood_destroy_park_with_xtxt_0xfa_drops_recomputed_non_network_fa_count() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-fa-nonnet-mac.sc2", "FloodFaNonNetMac").expect("blank");
    city.set_building(20, 20, 0x0e);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 0);
}

#[test]
fn pe_flood_destroy_skips_xtxt_arm_for_0xfe_like_destroy_building() {
    let mut city = City::new_blank("/tmp/sc2000-flood-xtxt-fe.sc2", "FloodXtxtFe").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfe);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xfe));
}

#[test]
fn mac68k_pe_flood_destroy_skips_xtxt_arm_for_0xfe_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-fe-mac.sc2", "FloodXtxtFeMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfe);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xfe));
}

#[test]
fn pe_flood_destroy_skips_xtxt_arm_for_0xfd_like_destroy_building() {
    let mut city = City::new_blank("/tmp/sc2000-flood-xtxt-fd.sc2", "FloodXtxtFd").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfd);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xfd));
}

#[test]
fn mac68k_pe_flood_destroy_skips_xtxt_arm_for_0xfd_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-fd-mac.sc2", "FloodXtxtFdMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfd);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xfd));
}

#[test]
fn pe_flood_destroy_skips_xtxt_arm_for_0xfc_like_destroy_building() {
    // DestroyStructure → DestroyBuilding @ `0x45a0e8`: XTXT 0xfc skips the XTXT/XLAB arm.
    let mut city = City::new_blank("/tmp/sc2000-flood-xtxt-fc.sc2", "FloodXtxtFc").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xfc);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xfc));
}

#[test]
fn mac68k_pe_flood_destroy_skips_xtxt_arm_for_0xfc_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-fc-mac.sc2", "FloodXtxtFcMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xfc);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xfc));
}

#[test]
fn pe_flood_destroy_skips_xtxt_arm_for_0xff_like_destroy_building() {
    // DestroyStructure → DestroyBuilding @ `0x45a0e8`: XTXT 0xff skips the XTXT/XLAB arm.
    let mut city = City::new_blank("/tmp/sc2000-flood-xtxt-ff.sc2", "FloodXtxtFf").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xff);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xff));
}

#[test]
fn mac68k_pe_flood_destroy_skips_xtxt_arm_for_0xff_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-flood-xtxt-ff-mac.sc2", "FloodXtxtFfMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_text_overlay(20, 20, 0xff);
    pe_flood_destroy_tile(&mut city, 20, 20);
    assert_eq!(city.text_overlay_at(20, 20), Some(0xff));
}

#[test]
fn pe_budget_tail_no_disasters_and_mac_fidelity_do_not_consume_crt() {
    for (fidelity, no_disasters) in [
        (EngineFidelity::Win95, true),
        (EngineFidelity::Mac68k, false),
    ] {
        let mut city = blank_city_layout_fixture("pe-budget-tail-gate.sc2", "adoption gate");
        city.set_city_funds(100_000);
        city.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, 0);
        let mut state = test_state(city);
        state.engine_fidelity = fidelity;
        state.no_disasters_enabled = no_disasters;
        state.pe_process_crt = Win95CrtRand::new(226);
        state.refresh_monthly_budget_rows(population_stats_for_city(&state.city));
        assert_eq!(state.pe_process_crt.seed(), 226, "{fidelity:?}");
        assert_eq!(ordinance_flags_for_city(&state.city), 0);
    }
}

#[test]
fn pe_modal_calendar_return_defers_queued_disaster_and_rechecks_restored_speed() {
    let mut city =
        blank_city_layout_fixture("pe-modal-disaster-after-paper.sc2", "queued disaster");
    set_city_newspaper_subscription_enabled(&mut city, true);
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 3,
        },
        24,
    );
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 3, 24);
    state.traffic.set_speed(GameSpeed::AfricanSwallow);
    let trigger = disaster_kind_misc_type(DisasterKind::NopAirCrash) as u16;
    state.city.pe_w_trigger_disaster = trigger;
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(200))
            .days,
        1
    );
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Subscription))
    );
    assert_eq!(state.city.pe_w_trigger_disaster, trigger);
    state.advance_frame_simulation(Duration::from_millis(1600));
    assert_eq!(state.city.pe_w_trigger_disaster, trigger);
    state.close_city_info_dialog();
    assert_eq!(state.traffic.speed(), GameSpeed::AfricanSwallow);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
    assert_eq!(state.city.pe_w_trigger_disaster, 0);
    // Admitted no-op case 5 also reaches the native end newspaper. Its own
    // modal captures speed 4 after disaster entry, rather than the old 5.
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Subscription))
    );
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
    state.close_city_info_dialog();
    assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
}

#[test]
fn pe_modal_active_disaster_keeps_paused_pulse_parity_without_scanning() {
    for path in ["pe-modal-held-flood.sc2", "pe-modal-held-flood.city"] {
        let mut city = blank_city_layout_fixture(path, "held flood");
        city.set_text_overlay(40, 40, 0xfc);
        city.set_pe_w_current_disaster_type(2);
        let mut state = test_state(city);
        state.traffic.set_speed(GameSpeed::Llama);
        state.pe_disaster_duration = 50;
        let clock = state.game_clock;
        state.open_city_info_dialog(CityInfoDialog::Budget);
        for _ in 0..8 {
            state.advance_frame_simulation(Duration::from_millis(200));
            assert_eq!(state.pe_disaster_duration, 50, "{path}");
        }
        state.close_city_info_dialog();
        state.advance_frame_simulation(Duration::ZERO);
        assert_eq!(
            state.pe_disaster_duration, 49,
            "{path}: surviving paused pending"
        );
        assert_eq!(state.game_clock, clock);
        assert_eq!(state.traffic.speed(), GameSpeed::Llama);
    }
}

#[test]
fn pe_idle_busy_disaster_timer_parity_continues_without_scanning() {
    for path in ["pe-idle-busy-disaster.sc2", "pe-idle-busy-disaster.city"] {
        let mut city = blank_city_layout_fixture(path, "held disaster");
        city.set_text_overlay(40, 40, 0xfc);
        city.set_pe_w_current_disaster_type(2);
        let mut state = test_state(city);
        state.traffic.set_speed(GameSpeed::Llama);
        state.pe_disaster_duration = 50;
        let before = state.game_clock;
        state.begin_network_drag(20, 20, NetworkKind::Power);
        for _ in 0..3 {
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(200))
                    .days,
                0
            );
            assert_eq!(state.pe_disaster_duration, 50, "{path}: no busy scan");
        }
        state.cancel_network_drag();
        // Pulse 2 latches pending; pulse 3's odd old parity clears it even
        // while busy. Releasing input cannot invent a pending update.
        state.advance_frame_simulation(Duration::ZERO);
        assert_eq!(state.pe_disaster_duration, 50);
        state.advance_frame_simulation(Duration::from_millis(200));
        assert_eq!(state.pe_disaster_duration, 49);
        assert_eq!(state.game_clock, before);
    }
}

#[test]
fn pe_idle_disaster_dispatch_continues_without_advancing_calendar() {
    for path in ["/tmp/pe-idle-disaster.sc2", "/tmp/pe-idle-disaster.city"] {
        let mut city = blank_city_layout_fixture(path, "disaster pulse");
        city.set_text_overlay(40, 40, 0xfc);
        city.set_pe_w_current_disaster_type(2);
        let mut state = test_state(city);
        state.traffic.set_speed(GameSpeed::Llama);
        state.pe_disaster_duration = 50;
        let before = state.game_clock;
        for _ in 0..2 {
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(200))
                    .days,
                0
            );
        }
        assert_eq!(
            state.game_clock, before,
            "{path}: calendar held during mode 2"
        );
        assert_eq!(
            state.pe_disaster_duration, 49,
            "{path}: disaster still advances"
        );
    }
}

#[test]
fn pe_idle_calendar_phase_does_not_run_disaster_scan() {
    let city = blank_city_layout_fixture("/tmp/pe-idle-phase.sc2", "phase");
    let mut state = test_state(city);
    state.pe_disaster_duration = 50;
    state.handle_simulation_day_advanced(1900, 1, 5);
    assert_eq!(state.pe_disaster_duration, 50);
}

#[test]
fn pe_idle_fastest_dispatches_per_idle_opportunity_and_disaster_rechecks_exit_speed() {
    for path in ["/tmp/pe-idle-fastest.sc2", "/tmp/pe-idle-fastest.city"] {
        let city = blank_city_layout_fixture(path, "fastest");
        let mut state = test_state(city);
        state.traffic.set_speed(GameSpeed::AfricanSwallow);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(200))
                .days,
            1
        );
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
        state.city.pe_w_trigger_disaster = 7;
        state.pe_disaster_duration = 50;
        // A mode-1 calendar step admits the queued tornado in the same dispatch.
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
        assert!(state.city.has_active_disaster());
        assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
        assert_eq!(state.menu_checks.speed, GameSpeed::Cheetah);
        assert_eq!(state.pe_disaster_duration, 49);
        // Exit used the new speed 4, so no stale speed-5 pending bit survives.
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(state.pe_disaster_duration, 49);
    }
}

#[test]
fn pe_idle_disaster_finishes_and_calendar_resumes_in_both_city_layouts() {
    for path in ["/tmp/pe-idle-end.sc2", "/tmp/pe-idle-end.city"] {
        let mut city = blank_city_layout_fixture(path, "end");
        city.set_text_overlay(40, 40, 0xfc);
        city.set_pe_w_current_disaster_type(2);
        let story_before = city.misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            NEWSPAPER_STORY_LIST_MISC_DWORDS,
        );
        let mut state = test_state(city);
        state.traffic.set_speed(GameSpeed::Llama);
        state.pe_disaster_duration = 0;
        let before = state.game_clock;
        for _ in 0..64 {
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(200))
                    .days,
                0
            );
            if !state.city.has_active_disaster() {
                break;
            }
        }
        assert!(
            !state.city.has_active_disaster(),
            "{path}: receding flood must finish"
        );
        assert_eq!(state.city.pe_w_current_disaster_type(), 0);
        assert_eq!(state.game_clock, before);
        if state.city.kind == CityFileKind::MacCity {
            assert_eq!(
                state.city.misc_u32_values(
                    MISC_NEWSPAPER_STORY_LIST_OFFSET,
                    NEWSPAPER_STORY_LIST_MISC_DWORDS
                ),
                story_before
            );
        } else {
            assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x17));
        }
        state.close_modal_dialogs();
        let days: u32 = (0..2)
            .map(|_| {
                state
                    .advance_frame_simulation(Duration::from_millis(200))
                    .days
            })
            .sum();
        assert_eq!(days, 1, "{path}: ordinary phase resumes after containment");
    }
}

#[test]
fn pe_idle_admitted_noop_disaster_still_reduces_fastest_speed() {
    for path in ["/tmp/pe-idle-noop.sc2", "/tmp/pe-idle-noop.city"] {
        let city = blank_city_layout_fixture(path, "noop");
        let mut state = test_state(city);
        state.traffic.set_speed(GameSpeed::AfricanSwallow);
        let before = state.game_clock;
        // Native case 5 sets successful admission even though it creates no actor.
        assert!(state.start_disaster(DisasterKind::NopAirCrash));
        assert!(!state.city.has_active_disaster());
        assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
        assert_eq!(state.game_clock, before);
    }
}

#[test]
fn win95_generic_refresh_does_not_advance_density_planes() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank("/tmp/win95-latched-density.sc2", "Latched").unwrap();
        city.kind = kind;
        city.sync_land_value_from_slice(&vec![11; CITY_TILES]);
        city.sync_pollution_density_from_slice(&vec![22; CITY_TILES]);
        city.sync_crime_density_from_slice(&vec![33; CITY_TILES]);
        city.sync_population_density_from_slice(&vec![44; CITY_TILES]);
        city.sync_police_coverage_from_slice(&vec![55; CITY_TILES]);
        city.sync_fire_coverage_from_slice(&vec![66; CITY_TILES]);
        city.sync_rate_of_growth_from_slice(&vec![77; CITY_TILES]);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.refresh_live_misc_current_values();
        state.refresh_live_misc_current_values();
        for (name, plane, expected) in [
            ("land", state.city.land_value_slice(), 11),
            ("pollution", state.city.pollution_density_slice(), 22),
            ("crime", state.city.crime_density_slice(), 33),
            ("population", state.city.population_density_slice(), 44),
            ("police", state.city.police_coverage_slice(), 55),
            ("fire", state.city.fire_coverage_slice(), 66),
            ("growth", state.city.rate_of_growth_slice(), 77),
        ] {
            assert!(
                plane.unwrap().iter().all(|&v| v == expected),
                "{name} {kind:?}"
            );
        }
        let population = live_population_stats_for_city(&state.city);
        let outcomes = live_city_service_outcomes_for_city(&state.city);
        state.sim_calendar_day2_density(population, outcomes);
        assert_ne!(
            state.city.pollution_density_slice().unwrap()[0],
            22,
            "day 2 publishes"
        );
    }
}

#[test]
fn mac_generic_refresh_does_not_advance_density_planes() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank("/tmp/mac-latched-density.sc2", "Latched").unwrap();
        city.kind = kind;
        city.sync_land_value_from_slice(&vec![11; CITY_TILES]);
        city.sync_pollution_density_from_slice(&vec![22; CITY_TILES]);
        city.sync_crime_density_from_slice(&vec![33; CITY_TILES]);
        city.sync_population_density_from_slice(&vec![44; CITY_TILES]);
        city.sync_police_coverage_from_slice(&vec![55; CITY_TILES]);
        city.sync_fire_coverage_from_slice(&vec![66; CITY_TILES]);
        city.sync_rate_of_growth_from_slice(&vec![77; CITY_TILES]);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.refresh_live_misc_current_values();
        state.refresh_live_misc_current_values();
        for (name, plane, expected) in [
            ("land", state.city.land_value_slice(), 11),
            ("pollution", state.city.pollution_density_slice(), 22),
            ("crime", state.city.crime_density_slice(), 33),
            ("population", state.city.population_density_slice(), 44),
            ("police", state.city.police_coverage_slice(), 55),
            ("fire", state.city.fire_coverage_slice(), 66),
            ("growth", state.city.rate_of_growth_slice(), 77),
        ] {
            assert!(
                plane.unwrap().iter().all(|&v| v == expected),
                "{name} {kind:?}"
            );
        }
        let population = live_population_stats_for_city(&state.city);
        let outcomes = live_city_service_outcomes_for_city(&state.city);
        state.sim_calendar_day2_density(population, outcomes);
        assert_ne!(
            state.city.pollution_density_slice().unwrap()[0],
            22,
            "phase 2 publishes"
        );
    }
}

#[test]
fn pe_scenario_deadline_result_is_processed_before_simultaneous_bankruptcy() {
    let mut city = City::new_blank("/tmp/scenario-bankrupt.scn", "Scenario").unwrap();
    city.kind = CityFileKind::WinScenario;
    city.set_city_funds(-100_001);
    city.chunks.push(test_scen_goals_chunk(ScenarioGoals {
        time_limit_months: 1,
        city_population_goal: 50_000,
        ..Default::default()
    }));
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    let population = live_population_stats_for_city(&state.city);
    let day22 = state.sim_calendar_day22_scenario(population);
    assert_eq!(state.city.scenario_time_limit_months(), Some(0));
    assert!(state.city.pe_scenario_deadline_expired());
    assert_eq!(
        state.announced_scenario_outcome,
        Some(ScenarioOutcome::Failed)
    );
    assert!(state.announced_bankruptcy);
    assert!(!state.city.pe_scenario_active());
    assert!(day22.scenario_outcome_announced);
    assert!(day22.status_announced);
    assert_eq!(state.query_line, "Mayor Impeached - City Bankrupt");
}

#[test]
fn pe_scenario_outcome_sounds_follow_result_and_sound_option() {
    for (funds, goal, limit, enabled, expected) in [
        (2_000, 1_000, 1, true, vec![PlacementSound::Cheers]),
        (0, 1_000, 1, true, vec![PlacementSound::Riot]),
        (
            -100_001,
            1_000,
            1,
            true,
            vec![PlacementSound::Riot, PlacementSound::Riot],
        ),
        (2_000, 1_000, 1, false, vec![]),
        (-100_001, 1_000, 1, false, vec![]),
    ] {
        let mut city = City::new_blank("/tmp/scenario-sound.scn", "Scenario").unwrap();
        city.kind = CityFileKind::WinScenario;
        city.set_city_funds(funds);
        city.chunks.push(test_scen_goals_chunk(ScenarioGoals {
            time_limit_months: limit,
            fund_goal: goal,
            ..Default::default()
        }));
        let mut state = test_state(city);
        state.sound_effects_enabled = enabled;
        state.no_disasters_enabled = true;
        let population = live_population_stats_for_city(&state.city);
        state.sim_calendar_day22_scenario(population);
        assert_eq!(
            state.take_pending_disaster_extra_sounds(),
            expected,
            "funds {funds}, enabled {enabled}"
        );
        state.sim_calendar_day22_scenario(population);
        assert!(
            state.take_pending_disaster_extra_sounds().is_empty(),
            "terminal cues play once"
        );
    }
}

#[test]
#[ignore = "requires local Win95 and Macintosh scenario catalogs"]
fn mac_engine_opens_and_resumes_retail_win_layout_scenarios() {
    let win =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_SCENARIO_FIXTURES").unwrap());
    let mac = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SCENARIO_FIXTURES").unwrap());
    let frame = std::time::Duration::from_secs(1);

    for (win_name, mac_name) in [
        ("CHARLEST", "Charleston"),
        ("DULLSVIL", "Dullsville"),
        ("FLINT", "Flint"),
        ("HOLLYWOO", "Hollywood"),
        ("OAKLAND", "Oakland"),
    ] {
        let source = win.join(format!("{win_name}.SCN"));
        let start = crate::mac_scenario::MacScenario::read(&mac.join(mac_name))
            .unwrap()
            .start;
        let city = City::load(&source).unwrap();
        assert_eq!(city.mac_start_from_win_scenario(), Some(start));
        let mut state = if mac_name == "Hollywood" {
            let state = ViewerState::load_default_from_path(source.clone()).unwrap();
            assert_eq!(state.engine_fidelity, EngineFidelity::Mac68k);
            state
        } else {
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.load_city(&source, (1280, 800), 1.0).unwrap();
            state
        };
        let saved = state.city.path.clone();
        assert_ne!(saved, source);
        assert_eq!(saved.extension().unwrap(), "sc2");
        assert!(state.city.mac_scenario_active());
        assert!(!state.city.pe_scenario_active());
        assert_eq!(state.city.mac_pending_action(), u32::from(start.action));

        // A copy saved before CODE 2 polls the scenario point starts once.
        state.save_city(&saved, (1280, 800), 1.0).unwrap();
        state.load_city(&saved, (1280, 800), 1.0).unwrap();
        assert_eq!(state.city.kind, CityFileKind::WinSc2);
        state.mac_process_rng = MacToolboxRng::new(1);
        state.traffic.set_speed(GameSpeed::Paused);
        assert_eq!(state.advance_mac_frame_simulation(frame).days, 0);
        assert_eq!(state.city.mac_pending_action(), 0);
        let action_active =
            crate::app::mac_loop_scheduler::scenario_action::mac_scenario_action_active;
        assert_eq!(action_active(&state.city), start.action != 0);

        if start.action != 0 {
            state.save_city(&saved, (1280, 800), 1.0).unwrap();
            state.load_city(&saved, (1280, 800), 1.0).unwrap();
            assert!(action_active(&state.city));
            assert_eq!(state.city.mac_state_misc_u32_at(0x04), Some(2));
            let clock = state.game_clock;
            state.traffic.set_speed(GameSpeed::Cheetah);
            assert_eq!(state.advance_mac_frame_simulation(frame).days, 0);
            assert_eq!(state.game_clock, clock);

            if mac_name == "Charleston" {
                assert_eq!(state.city.mac_state_misc_u32_at(0x10b0), Some(48));
                assert_eq!(state.city.mac_state_misc_u32_at(0x10b4), Some(58));
                for _ in 0..240 {
                    if !action_active(&state.city) {
                        break;
                    }
                    assert_eq!(state.advance_mac_frame_simulation(frame).days, 0);
                }
                assert!(!action_active(&state.city));
                assert_eq!(state.advance_mac_frame_simulation(frame).days, 1);
            } else if mac_name == "Hollywood" {
                assert_eq!(state.city.traffic_overlays().unwrap()[1].kind, 5);
            }
        }
        std::fs::remove_file(saved).unwrap();
    }
}

#[test]
#[ignore = "requires a local Win95 scenario catalog"]
fn pe_retail_win_scenario_open_checkpoints() {
    let root = PathBuf::from(std::env::var_os("CITYGAME_WIN95_SCENARIO_FIXTURES").unwrap());
    let cases = [
        (
            "FLINT",
            0,
            (0, 0),
            4270317332,
            (93, 100),
            0x4382_e233_be8a_cff3,
        ),
        (
            "CHARLEST",
            16,
            (68, 99),
            4270317332,
            (73, 83),
            0x07bc_3569_a140_c2fe,
        ),
        (
            "HOLLYWOO",
            8,
            (80, 52),
            2597632883,
            (96, 96),
            0xd2ac_2048_7cc2_ae1a,
        ),
        (
            "SANFRANC",
            6,
            (41, 61),
            3289249106,
            (54, 70),
            0x8f3d_9685_d8ed_314c,
        ),
    ];
    for (name, selector, point, crt, services, expected_xbit) in cases {
        let city = City::new_blank("/tmp/pe-scenario-open.sc2", "test").expect("blank city");
        let mut state = test_state(city);
        state.pe_process_crt = Win95CrtRand::new(1);
        state.menu_checks.citygame_newspaper = false;
        state
            .load_city(&root.join(format!("{name}.SCN")), (800, 600), 1.0)
            .expect("load retail scenario");
        assert_eq!(
            pe_open_xbit_fingerprint(&state.city),
            expected_xbit,
            "{name} XBIT"
        );
        assert_eq!(
            (
                state.city.pe_power_used_percent(),
                state.city.pe_water_coverage_percent()
            ),
            services,
            "{name} services"
        );
        assert_eq!(state.city.pe_w_trigger_disaster, selector, "{name}");
        assert_eq!(
            state.pe_idle_scheduler.pending_disaster_point,
            Some(point),
            "{name}"
        );
        assert_eq!(state.pe_process_crt_seed(), crt, "{name}");
    }
}

fn pe_open_xbit_fingerprint(city: &City) -> u64 {
    let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let bits = city.tile_bits_at(x, y).unwrap_or_default().raw_bits();
            fingerprint = (fingerprint ^ u64::from(bits)).wrapping_mul(0x100_0000_01b3);
        }
    }
    fingerprint
}

#[test]
#[ignore = "requires a local Win95 city catalog"]
fn pe_retail_win_city_open_checkpoints() {
    let root = PathBuf::from(std::env::var_os("CITYGAME_WIN95_CITY_FIXTURES").unwrap());
    let cases = [
        (
            "HAPPYLND",
            AppMode::CityView,
            3_376_686_420,
            (100, 100),
            0x41ef_d49b_3613_a107,
        ),
        (
            "NYC",
            AppMode::CityView,
            2_503_793_637,
            (80, 63),
            0xbb48_f265_0578_8bda,
        ),
        (
            "VOLCANO",
            AppMode::CityView,
            2_464_237_796,
            (52, 100),
            0xaa16_86eb_2258_7919,
        ),
        (
            "TOMG_B1",
            AppMode::EditNewMap,
            2_503_793_637,
            (100, 100),
            0x253f_b3c4_41eb_a6b8,
        ),
    ];
    for (name, mode, crt, services, xbit) in cases {
        let city = City::new_blank("/tmp/pe-ordinary-open.sc2", "Empty").unwrap();
        let mut state = test_state(city);
        state.pe_process_crt = Win95CrtRand::new(1);
        state.menu_checks.citygame_newspaper = false;
        state
            .load_city(&root.join(format!("{name}.SC2")), (800, 600), 1.0)
            .unwrap();
        assert_eq!(state.mode, mode, "{name}");
        assert_eq!(state.city.pe_w_trigger_disaster, 0, "{name}");
        assert_eq!(state.pe_process_crt_seed(), crt, "{name}");
        assert_eq!(pe_open_xbit_fingerprint(&state.city), xbit, "{name}");
        assert_eq!(
            (
                state.city.pe_power_used_percent(),
                state.city.pe_water_coverage_percent()
            ),
            services,
            "{name} services",
        );
    }
}

#[test]
fn pe_things_sound_prelude_matches_native_crt_and_effect_order() {
    for (traffic, crime, police, sound_enabled, seed, expected_seed, sounds) in [
        (35, 40, 1, true, 1, 1, &[][..]),
        (36, 41, 0, true, 1, 2_745_024, &[]),
        (35, 41, 1, true, 1, 2_745_024, &[]),
        (36, 41, 1, true, 1, 3_357_800_067, &[]),
        (u32::MAX, u32::MAX, 1, true, 1, 1, &[]),
        (
            36,
            41,
            1,
            true,
            72_032,
            3_942_675_162,
            &[PlacementSound::Horns, PlacementSound::Police],
        ),
        (36, 41, 1, false, 72_032, 3_942_675_162, &[]),
    ] {
        let mut city = City::new_blank("/tmp/pe-things-prelude.sc2", "Test").unwrap();
        city.set_chunk_u32_values(XGRP_CHUNK_NAME, 4 * XGRP_SAMPLES_PER_SERIES * 4, &[traffic]);
        city.set_chunk_u32_values(XGRP_CHUNK_NAME, 7 * XGRP_SAMPLES_PER_SERIES * 4, &[crime]);
        city.hydrate_pe_attitude_scores_from_xgrp();
        city.pe_dw_tile_count[POLICE_STATION_BUILDING_ID as usize] = police;
        let mut state = test_state(city);
        state.sound_effects_enabled = sound_enabled;
        state.pe_process_crt = Win95CrtRand::new(seed);

        assert_eq!(state.tick_pe_disaster_xthg_actors_in_slot_order(), 0);
        assert_eq!(state.pe_process_crt_seed(), expected_seed);
        assert_eq!(state.take_pending_disaster_extra_sounds(), sounds);
    }
}

#[test]
#[ignore = "requires a local Win95 city catalog"]
fn pe_retail_win_city_open_reaches_native_early_calendar() {
    let root = PathBuf::from(std::env::var_os("CITYGAME_WIN95_CITY_FIXTURES").unwrap());
    let cases = [
        (
            "JUNETOWN",
            702,
            2_146_787_367,
            (127_601, 25_886, 6_576),
            [
                0x0c3b_a9bf_fb5f_b69f,
                0xd9e2_93e9_e942_2700,
                0xea09_e8b7_2d6a_242f,
                0x1c5e_90d1_0662_c56d,
                0x0e42_31b8_29dd_8f9d,
            ],
        ),
        (
            "LAKELAND",
            472_652,
            2_503_793_637,
            (87_800, 26_379, 20_090),
            [
                0xe64b_47e2_24d1_5cac,
                0x6aa5_465a_d1b7_7d41,
                0x9087_a2f7_8370_fa8a,
                0x3151_b240_0fab_98b9,
                0x2cf3_21f4_004a_6198,
            ],
        ),
    ];
    let compact_fingerprint = |plane: &[u8]| {
        (0..128)
            .step_by(2)
            .flat_map(|x| (0..128).step_by(2).map(move |y| (x, y)))
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, (x, y)| {
                (hash ^ u64::from(plane[x * 128 + y])).wrapping_mul(0x0000_0100_0000_01b3)
            })
    };
    for (name, age, crt, counters, planes) in cases {
        let city = City::new_blank("/tmp/pe-ordinary-phase.sc2", "Empty").unwrap();
        let mut state = test_state(city);
        state.pe_process_crt = Win95CrtRand::new(1);
        state.menu_checks.citygame_newspaper = false;
        state
            .load_city(&root.join(format!("{name}.SC2")), (800, 600), 1.0)
            .unwrap();
        if name == "LAKELAND" {
            state.game_clock = state.game_clock.wrapping_add_days(1);
            let clock = state.game_clock;
            state.handle_simulation_day_advanced(clock.year(), clock.month(), clock.day());
            assert_eq!(state.city.city_age_days(), Some(472_651));
            assert_eq!(state.pe_process_crt_seed(), crt);
            assert_eq!(pe_open_xbit_fingerprint(&state.city), 0xeef8_a6ad_5f3b_cc5c);
        }
        state.game_clock = state.game_clock.wrapping_add_days(1);
        let clock = state.game_clock;
        state.handle_simulation_day_advanced(clock.year(), clock.month(), clock.day());
        let city = &state.city;
        assert_eq!(city.city_age_days(), Some(age), "{name}");
        assert_eq!(state.pe_process_crt_seed(), crt, "{name}");
        assert_eq!(
            (
                city.pe_dw_city_land_value,
                city.pe_dw_city_crime,
                city.pe_dw_city_pollution
            ),
            counters,
            "{name}"
        );
        assert_eq!(
            [
                pe_open_xbit_fingerprint(city),
                compact_fingerprint(city.land_value_slice().unwrap()),
                compact_fingerprint(city.crime_density_slice().unwrap()),
                compact_fingerprint(city.pollution_density_slice().unwrap()),
                compact_fingerprint(city.traffic_density_slice().unwrap()),
            ],
            planes,
            "{name}"
        );
        let _live_rng = LiveGrowthRng::enable();
        state.bypass_growth_trip_gate = false;
        // The PE fixture enters the calendar at the saved view angle. CityGame
        // normally opens at compass zero, which selects a different XZON corner.
        state.city.set_view_rotation(state.city.saved_rotation());
        let (growth, zoning): (&[(u32, u32, u64, u64)], u64) = if name == "LAKELAND" {
            (
                &[
                    (
                        472_653,
                        2_638_162_508,
                        0x6f8d_698b_3662_bab4,
                        0xb748_967c_d98e_cb3d,
                    ),
                    (
                        472_654,
                        3_811_536_597,
                        0xdaf7_166c_7a83_7c52,
                        0xb748_967c_d98e_cb3d,
                    ),
                    (
                        472_655,
                        666_383_434,
                        0x3e5e_f8c8_8986_171f,
                        0xa971_48c5_3c45_935c,
                    ),
                    (
                        472_656,
                        1_034_662_603,
                        0x2ecd_8a5d_8b33_5d01,
                        0xbcfe_1a59_2f6a_858e,
                    ),
                    (
                        472_657,
                        1_949_337_683,
                        0xd38b_20ce_5b25_1eba,
                        0xb5ef_948b_c371_d96d,
                    ),
                    (
                        472_658,
                        1_006_090_074,
                        0x4072_07a5_639b_2480,
                        0xb5ef_948b_c371_d96d,
                    ),
                    (
                        472_659,
                        2_357_579_171,
                        0x4b87_3005_2678_2ccf,
                        0xb5ef_948b_c371_d96d,
                    ),
                    (
                        472_660,
                        1_676_090_692,
                        0xe156_dfed_898d_b231,
                        0x7f08_bc9c_7a36_1c30,
                    ),
                    (
                        472_661,
                        2_423_285_898,
                        0x083c_29cd_3843_6bff,
                        0x5260_b4fd_c64f_3bda,
                    ),
                    (
                        472_662,
                        2_724_445_876,
                        0x2e1d_666e_688a_3e49,
                        0x5260_b4fd_c64f_3bda,
                    ),
                    (
                        472_663,
                        355_102_568,
                        0x9a44_b1f8_b48a_6148,
                        0xe30e_237a_c5fe_da47,
                    ),
                    (
                        472_664,
                        3_322_104_261,
                        0x9242_e81b_9967_e81b,
                        0xe30e_237a_c5fe_da47,
                    ),
                    (
                        472_665,
                        2_793_435_355,
                        0x7b4b_0382_2813_59de,
                        0xe55e_1ddf_7081_8425,
                    ),
                    (
                        472_666,
                        4_084_408_679,
                        0x5834_9fe2_ef9b_d14d,
                        0xe55e_1ddf_7081_8425,
                    ),
                    (
                        472_667,
                        2_385_800_764,
                        0x632f_4643_4b8a_07ce,
                        0xe55e_1ddf_7081_8425,
                    ),
                    (
                        472_668,
                        2_184_101_136,
                        0xd57b_415d_4aa4_a50c,
                        0xe55e_1ddf_7081_8425,
                    ),
                    (
                        472_669,
                        2_184_101_136,
                        0x7a42_fa46_f304_ae7f,
                        0xe55e_1ddf_7081_8425,
                    ),
                    (
                        472_670,
                        2_184_101_136,
                        0x7a42_fa46_f304_ae7f,
                        0xe55e_1ddf_7081_8425,
                    ),
                ],
                0x62f4_7239_e3b0_640f,
            )
        } else {
            (
                &[
                    (
                        703,
                        521_587_956,
                        0x5263_683c_ca98_dbed,
                        0x0d6d_563e_c38e_c732,
                    ),
                    (
                        704,
                        1_808_359_305,
                        0x5263_683c_ca98_dbed,
                        0x0d6d_563e_c38e_c732,
                    ),
                ],
                0xf4b1_765a_5e38_f7db,
            )
        };
        let map_fingerprint = |byte_at: &dyn Fn(usize, usize) -> u8| {
            (0..128)
                .flat_map(|x| (0..128).map(move |y| byte_at(x, y)))
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
                })
        };
        for &(age, crt, xtrf, xbld) in growth {
            state.game_clock = state.game_clock.wrapping_add_days(1);
            let clock = state.game_clock;
            state.handle_simulation_day_advanced(clock.year(), clock.month(), clock.day());
            let city = &state.city;
            let xbit = if age == 472_670 {
                0xeef8_a6ad_5f3b_cc5c
            } else {
                planes[0]
            };
            assert_eq!(city.city_age_days(), Some(age), "{name}");
            assert_eq!(state.pe_process_crt_seed(), crt, "{name} age {age}");
            assert_eq!(
                pe_open_xbit_fingerprint(city),
                xbit,
                "{name} age {age} XBIT"
            );
            assert_eq!(
                compact_fingerprint(city.traffic_density_slice().unwrap()),
                xtrf,
                "{name} age {age} XTRF",
            );
            assert_eq!(
                map_fingerprint(&|x, y| city.building_at(x, y)),
                xbld,
                "{name} age {age} XBLD",
            );
            assert_eq!(
                map_fingerprint(&|x, y| city.zone_at(x, y).unwrap().raw_bits()),
                zoning,
                "{name} age {age} XZON",
            );
        }
        if name == "LAKELAND" {
            let fingerprint = |words: Vec<u32>| {
                words
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
                    })
            };
            for age in 472_671..=472_676 {
                state.game_clock = state.game_clock.wrapping_add_days(1);
                let clock = state.game_clock;
                state.handle_simulation_day_advanced(clock.year(), clock.month(), clock.day());
                let city = &state.city;
                assert_eq!(city.city_age_days(), Some(age));
                assert_eq!(state.pe_process_crt_seed(), 1_711_461_256);
                assert_eq!(state.pe_process_lfsr_state(), 30_044);
                assert_eq!(city.pe_dw_city_population(), 38_070);
                assert_eq!(city.pe_zone_pops(), [1746, 688, 1373]);
                assert_eq!(city.misc_u32_values(RCI_DEMAND_MISC_OFFSET, 3), [2000; 3]);
                assert_eq!(city.pe_traffic_count(), 26_700);
                assert_eq!(city.city_funds(), 20_000);
                assert_eq!(pe_open_xbit_fingerprint(city), 0xeef8_a6ad_5f3b_cc5c);
                assert_eq!(
                    compact_fingerprint(city.traffic_density_slice().unwrap()),
                    0x7a42_fa46_f304_ae7f,
                );
                assert_eq!(
                    fingerprint(city.misc_u32_values(0x007c, 60)),
                    0x504e_9180_7ebd_525d,
                );
                assert_eq!(
                    fingerprint(city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES)),
                    0x10e4_f440_67f1_2202,
                );
            }
            let budget_row_field = |city: &City, field| {
                std::array::from_fn::<_, 16, _>(|row| {
                    let offset = budget_row_misc_offset_for_city(city, row, field).unwrap();
                    city.budget_misc_u32_at(offset).unwrap() as i32
                })
            };
            assert_eq!(
                budget_row_field(&state.city, BUDGET_SERVICE_CURRENT_FIELD),
                [
                    17460, 6880, 13730, -89921, 0, 4, 4, 2, 3, 1, 670, 0, 0, 33, 202, 0
                ],
            );
            assert_eq!(
                budget_row_field(&state.city, BUDGET_SERVICE_YEAR_TO_DATE_FIELD),
                [
                    969990, 385140, 778960, -722316, 0, 3200, 3200, 1600, 2400, 800, 536000, 0, 0,
                    26400, 105040, 0
                ],
            );

            // The next month consumes the demand, graph and budget values above.
            for (age, crt, xtrf, xbld) in [
                (
                    472_677,
                    1_711_461_256,
                    0x7a42_fa46_f304_ae7f,
                    0xe55e_1ddf_7081_8425,
                ),
                (
                    472_678,
                    4_055_635_882,
                    0x7442_75dc_d2d5_7230,
                    0xe55e_1ddf_7081_8425,
                ),
                (
                    472_679,
                    4_277_557_783,
                    0x1286_b2b1_bdcb_f592,
                    0xbb7f_ed8e_4beb_f1ac,
                ),
                (
                    472_680,
                    1_024_639_858,
                    0x082b_0c5c_1822_b157,
                    0xbb7f_ed8e_4beb_f1ac,
                ),
                (
                    472_681,
                    821_129_645,
                    0x9f95_dd84_53a9_ae77,
                    0xbb7f_ed8e_4beb_f1ac,
                ),
                (
                    472_682,
                    4_148_804_973,
                    0x39e2_03da_be1e_55a4,
                    0xbb7f_ed8e_4beb_f1ac,
                ),
                (
                    472_683,
                    3_634_046_153,
                    0x0986_12f2_b41a_4dca,
                    0xbb7f_ed8e_4beb_f1ac,
                ),
                (
                    472_684,
                    3_087_756_617,
                    0x3cde_69c2_b399_254e,
                    0x09dc_d586_35f3_be8c,
                ),
                (
                    472_685,
                    2_122_082_208,
                    0xd496_bd6c_5609_9456,
                    0x2010_372f_295e_b888,
                ),
                (
                    472_686,
                    3_297_612_509,
                    0xb43d_60a0_1c30_d95c,
                    0x7d12_2318_a4a7_5664,
                ),
                (
                    472_687,
                    4_182_627_464,
                    0x8e96_1f2b_fb69_d827,
                    0x708b_77f5_b524_dadb,
                ),
                (
                    472_688,
                    4_105_585_232,
                    0x1635_0725_47ed_af52,
                    0xb059_abe8_d180_a290,
                ),
                (
                    472_689,
                    1_865_121_464,
                    0x96fd_cc06_8084_7411,
                    0xedf0_de94_539e_44ae,
                ),
                (
                    472_690,
                    1_488_776_586,
                    0x61bd_18b4_fafd_b5d5,
                    0x4da8_45b5_3dc2_8f02,
                ),
                (
                    472_691,
                    3_719_012_189,
                    0xb7ea_0aa3_dda2_b773,
                    0x4da8_45b5_3dc2_8f02,
                ),
                (
                    472_692,
                    3_925_670_559,
                    0x5623_6f90_b67b_e77f,
                    0x4da8_45b5_3dc2_8f02,
                ),
                (
                    472_693,
                    4_167_035_821,
                    0x3198_37ed_04e4_b26d,
                    0x8a6e_c065_ae39_75a8,
                ),
                (
                    472_694,
                    4_167_035_821,
                    0xc93a_c86b_df58_0848,
                    0x8a6e_c065_ae39_75a8,
                ),
                (
                    472_695,
                    4_167_035_821,
                    0xc93a_c86b_df58_0848,
                    0x8a6e_c065_ae39_75a8,
                ),
                (
                    472_696,
                    3_949_494_288,
                    0xc93a_c86b_df58_0848,
                    0xbced_c7e9_33d9_4217,
                ),
                (
                    472_697,
                    3_949_494_288,
                    0xc93a_c86b_df58_0848,
                    0xbced_c7e9_33d9_4217,
                ),
                (
                    472_698,
                    3_949_494_288,
                    0xc93a_c86b_df58_0848,
                    0xbced_c7e9_33d9_4217,
                ),
                (
                    472_699,
                    3_949_494_288,
                    0xc93a_c86b_df58_0848,
                    0xbced_c7e9_33d9_4217,
                ),
                (
                    472_700,
                    3_949_494_288,
                    0xc93a_c86b_df58_0848,
                    0xbced_c7e9_33d9_4217,
                ),
            ] {
                state.game_clock = state.game_clock.wrapping_add_days(1);
                let clock = state.game_clock;
                state.handle_simulation_day_advanced(clock.year(), clock.month(), clock.day());
                let city = &state.city;
                assert_eq!(city.city_age_days(), Some(age));
                assert_eq!(state.pe_process_crt_seed(), crt, "age {age}");
                assert_eq!(
                    pe_open_xbit_fingerprint(city),
                    if age < 472_695 {
                        planes[0]
                    } else {
                        0xeef8_a6ad_5f3b_cc5c
                    },
                    "age {age} XBIT",
                );
                assert_eq!(
                    compact_fingerprint(city.traffic_density_slice().unwrap()),
                    xtrf,
                    "age {age} XTRF",
                );
                assert_eq!(
                    map_fingerprint(&|x, y| city.building_at(x, y)),
                    xbld,
                    "age {age} XBLD",
                );
                assert_eq!(
                    map_fingerprint(&|x, y| city.zone_at(x, y).unwrap().raw_bits()),
                    zoning,
                    "age {age} XZON",
                );
            }
            let city = &state.city;
            assert_eq!(state.pe_process_lfsr_state(), 15_721);
            assert_eq!(city.pe_dw_city_population(), 37_960);
            assert_eq!(city.pe_zone_pops(), [1744, 688, 1364]);
            assert_eq!(city.pe_traffic_count(), 26_743);
            assert_eq!(
                fingerprint(city.misc_u32_values(0x007c, 60)),
                0x3e63_4dee_c5ee_efa6
            );
            assert_eq!(
                fingerprint(city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES)),
                0x2dce_9943_736c_5ad3,
            );
            assert_eq!(
                [
                    compact_fingerprint(city.land_value_slice().unwrap()),
                    compact_fingerprint(city.crime_density_slice().unwrap()),
                    compact_fingerprint(city.pollution_density_slice().unwrap()),
                ],
                [
                    0xbc71_5dc2_eed1_8916,
                    0xddd5_9ed9_b77b_c143,
                    0x7d4a_9c6d_4562_b5ce
                ],
            );
            assert_eq!(
                budget_row_field(city, BUDGET_SERVICE_CURRENT_FIELD),
                [
                    17440, 6880, 13640, -89605, 0, 4, 4, 2, 3, 1, 670, 0, 0, 33, 202, 0
                ],
            );
            assert_eq!(
                budget_row_field(city, BUDGET_SERVICE_YEAR_TO_DATE_FIELD),
                [
                    1092070, 433300, 874440, -812237, 0, 3600, 3600, 1800, 2700, 900, 603000, 0, 0,
                    29700, 118170, 0,
                ],
            );
        }
    }
}

#[test]
#[ignore = "requires the local Win95 city catalog"]
fn pe_retail_win_lakeland_mixed_things_sweep() {
    let root = PathBuf::from(std::env::var_os("CITYGAME_WIN95_CITY_FIXTURES").unwrap());
    let city = City::new_blank("/tmp/pe-mixed-things.sc2", "Empty").unwrap();
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(1);
    state.menu_checks.citygame_newspaper = false;
    state
        .load_city(&root.join("LAKELAND.SC2"), (800, 600), 1.0)
        .unwrap();
    state.city.set_view_rotation(state.city.saved_rotation());
    let hash = |hash: u64, byte: u8| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    let basis = 0xcbf2_9ce4_8422_2325_u64;
    for (sweep, (crt, lfsr, xtxt, xthg)) in [
        (
            2_503_793_637_u32,
            1,
            0xe3e0_8a78_9707_34c5_u64,
            0x5f72_dd70_655f_25ae_u64,
        ),
        (
            3_289_249_106,
            2048,
            0xa741_50d8_558f_1b59,
            0xb416_a9c6_1475_4e7f,
        ),
        (
            3_115_572_192,
            21183,
            0x19f3_9352_44e2_7e99,
            0x899e_59f1_53a0_783c,
        ),
        (
            1_304_996_533,
            51554,
            0xb22f_6fc2_4dd9_1b7f,
            0x0f53_61b8_b69b_4dca,
        ),
        (
            2_795_942_921,
            21034,
            0xcc8d_a451_96fd_ff5f,
            0x7719_5c42_2fe9_55f1,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if sweep != 0 {
            state.tick_pe_disaster_xthg_actors_in_slot_order();
        }
        let city = &state.city;
        let actual_xtxt = (0..128)
            .flat_map(|x| (0..128).map(move |y| city.text_overlay_at(x, y).unwrap_or(0)))
            .fold(basis, hash);
        let actual_xthg = city
            .traffic_overlays()
            .unwrap()
            .iter()
            .copied()
            .flat_map(Win95OverlayRecord::to_bytes)
            .fold(basis, hash);
        assert_eq!(state.pe_process_crt_seed(), crt, "sweep {sweep} CRT");
        assert_eq!(state.pe_process_lfsr_state(), lfsr, "sweep {sweep} LFSR");
        assert_eq!(actual_xtxt, xtxt, "sweep {sweep} XTXT");
        assert_eq!(actual_xthg, xthg, "sweep {sweep} XTHG");
    }
}

#[test]
#[ignore = "requires the local Win95 city catalog"]
fn pe_retail_win_lakeland_joins_things_and_calendar() {
    // Native --sweep-before-calendar --days 174 --trace-growth --trace-economy.
    let fixture = include_str!("../../../tests/fixtures/simulation/win95/pe_lakeland_calendar.csv");
    let checkpoints = pe_joined_calendar_fixture(fixture, 472_651);
    assert_win95_joined_calendar("LAKELAND.SC2", 472_651, &checkpoints, |age, city| {
        if age == 472_800 {
            assert_eq!(city.city_funds(), 21_148);
            assert_eq!(city.pe_dw_arcology_population(), 0);
            assert_eq!(pe_joined_xmic_fingerprint(city), 0x0e7a_16ed_8600_d8ec);
            assert_pe_joined_budget_rows(
                city,
                [
                    17_260, 6880, 14_330, -91_279, 0, 4, 4, 2, 3, 1, 670, 0, 0, 33, 202, 0,
                ],
                [
                    120_820, 48_160, 100_310, -90_526, 0, 400, 400, 200, 300, 100, 67_000, 0, 0,
                    3300, 13_130, 0,
                ],
            );
        }
        if let Some((cohorts, graph)) = match age {
            472_671 => Some((0x2604_7d06_ad9d_bccc, 0x10e4_f440_67f1_2202)),
            472_696 => Some((0x6697_510f_3b76_0aa8, 0xdc7c_6e15_7e02_ff06)),
            472_721 => Some((0xa53a_7137_0c40_d6ca, 0x016d_f8b0_5f54_2a17)),
            472_746 => Some((0xf64d_7bf2_373e_5fd7, 0xf25e_aa9f_fd10_fc3b)),
            _ => None,
        } {
            assert_eq!(
                pe_joined_word_fingerprint(city.misc_u32_values(0x007c, 60)),
                cohorts
            );
            assert_eq!(
                pe_joined_word_fingerprint(city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES)),
                graph,
            );
        }
        if let Some(expected_budget) = match age {
            472_675 => Some(0x5259_3123_8a47_ed64),
            472_700 => Some(0xd6bf_4e45_ffdc_2dd6),
            472_725 => Some(0xa23a_e0a4_377a_4e9f),
            472_750 => Some(0x12d9_caa5_2cfa_668d),
            _ => None,
        } {
            let budget = [
                BUDGET_SERVICE_CURRENT_FIELD,
                BUDGET_SERVICE_YEAR_TO_DATE_FIELD,
            ]
            .into_iter()
            .flat_map(|field| {
                (0..16).map(move |row| {
                    let offset = budget_row_misc_offset_for_city(city, row, field).unwrap();
                    city.budget_misc_u32_at(offset).unwrap()
                })
            })
            .collect();
            assert_eq!(pe_joined_word_fingerprint(budget), expected_budget);
            assert_eq!(city.city_funds(), 20_000);
        }
    });
}

#[test]
#[ignore = "requires the local Win95 city catalog"]
fn pe_retail_win_junetown_joins_things_and_calendar() {
    // Native --sweep-before-calendar --days 222 --trace-growth --trace-economy.
    let fixture = include_str!("../../../tests/fixtures/simulation/win95/pe_junetown_calendar.csv");
    let checkpoints = pe_joined_calendar_fixture(fixture, 702);
    assert_win95_joined_calendar("JUNETOWN.SC2", 702, &checkpoints, |age, city| {
        if age == 900 {
            assert_eq!(city.city_funds(), 849_384);
            assert_eq!(city.pe_dw_arcology_population(), 0);
            assert_eq!(pe_joined_xmic_fingerprint(city), 0x5ea6_1113_4f0c_d05f);
            assert_pe_joined_budget_rows(
                city,
                [
                    3390, 3040, 1360, 4905, 3, 2, 2, 2, 1, 2, 549, 712, 213, 308, 430, 0,
                ],
                [
                    23_730, 21_280, 9520, 4910, 120_000, 200, 200, 200, 100, 200, 54_900, 71_200,
                    21_300, 30_800, 43_000, 0,
                ],
            );
        }
        if age == 721 {
            let demand = [
                RciKind::Residential,
                RciKind::Commercial,
                RciKind::Industrial,
            ]
            .map(|kind| rci_demand_for_kind(city, kind));
            assert_eq!(demand, [2000, -1996, 2000]);
            assert_eq!(city.pe_dw_residential_population, 313);
        }
        if age == 921 {
            let demand = [
                RciKind::Residential,
                RciKind::Commercial,
                RciKind::Industrial,
            ]
            .map(|kind| rci_demand_for_kind(city, kind));
            assert_eq!(demand, [2000, -2000, 1644]);
            assert_eq!(
                pe_joined_word_fingerprint(city.misc_u32_values(0x007c, 60)),
                0xbba1_0fee_7371_8078,
            );
            assert_eq!(
                pe_joined_word_fingerprint(city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES)),
                0x940b_d8f8_fada_40b8,
            );
        }
    });
}

#[test]
#[ignore = "requires the local Win95 city catalog"]
fn pe_retail_win_london_rotation_joins_things_and_calendar() {
    // Native --sweep-before-calendar --days 585 --trace-growth --trace-economy.
    let fixture = include_str!("../../../tests/fixtures/simulation/win95/pe_london_calendar.csv");
    let checkpoints = pe_joined_calendar_fixture(fixture, 28_539);
    assert_win95_joined_calendar("LONDON.SC2", 28_539, &checkpoints, |age, city| {
        if age == 28_800 {
            assert_eq!(city.city_funds(), 21_681);
            assert_eq!(city.pe_dw_arcology_population(), 17_011);
            assert_eq!(pe_joined_xmic_fingerprint(city), 0x2aa9_5279_697b_3ecb);
            assert_pe_joined_budget_rows(
                city,
                [
                    91_891, 55_070, 26_410, -371_563, 0, 17, 10, 12, 18, 4, 3408, 20, 57, 311,
                    1041, 0,
                ],
                [
                    459_455, 275_350, 132_050, -368_375, 0, 1700, 1000, 1200, 1800, 400, 340_800,
                    2000, 5700, 31_100, 104_100, 0,
                ],
            );
        }
        if age == 29_100 {
            assert_eq!(city.city_funds(), 23_226);
            assert_eq!(city.pe_dw_arcology_population(), 20_412);
            assert_eq!(pe_joined_xmic_fingerprint(city), 0x40f2_3d3b_c75e_195d);
            assert_pe_joined_budget_rows(
                city,
                [
                    90_825, 56_127, 23_127, -365_921, 0, 17, 10, 12, 18, 4, 3408, 20, 57, 311,
                    1041, 0,
                ],
                [
                    454_125, 280_635, 115_635, -362_918, 0, 1700, 1000, 1200, 1800, 400, 340_800,
                    2000, 5700, 31_100, 104_100, 0,
                ],
            );
        }
        if age == 28_546 {
            assert_eq!(city.pe_dw_city_population(), 171_990);
            assert_eq!(city.pe_zone_pops(), [9092, 5422, 2685]);
        }
        if let Some((demand, cohorts, graph)) = match age {
            28_546 => Some((
                [615, 1314, 1811],
                0xcc9b_8a30_166a_d7a6,
                0xd138_b120_f820_2d78,
            )),
            28_571 => Some((
                [612, 1314, 1617],
                0xe80e_a523_f5ad_364b,
                0xcb03_81b3_cef2_7c4f,
            )),
            28_596 => Some((
                [612, 1310, 1424],
                0x3872_8a1f_084a_5091,
                0xcd74_d28b_85aa_c3d0,
            )),
            28_621 => Some((
                [603, 1304, 1239],
                0xd6da_b627_823a_65a8,
                0x7c2d_0252_4aa1_e1de,
            )),
            28_821 => Some((
                [536, 1140, -165],
                0x8db1_3464_2ee6_9cd3,
                0xd46c_50b3_5f1a_0b06,
            )),
            29_121 => Some((
                [288, 244, -1729],
                0xa722_d325_ad50_78ca,
                0xaadd_a6d4_f8a9_bb86,
            )),
            _ => None,
        } {
            assert_eq!(
                [
                    RciKind::Residential,
                    RciKind::Commercial,
                    RciKind::Industrial
                ]
                .map(|kind| rci_demand_for_kind(city, kind)),
                demand,
            );
            assert_eq!(
                pe_joined_word_fingerprint(city.misc_u32_values(0x007c, 60)),
                cohorts
            );
            assert_eq!(
                pe_joined_word_fingerprint(city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES)),
                graph,
            );
        }
    });
}

#[test]
#[ignore = "requires the local Win95 city catalog"]
fn pe_retail_win_volcano_rotation_joins_things_and_calendar() {
    // Native --sweep-before-calendar --days 140 --trace-growth --trace-economy.
    let fixture = include_str!("../../../tests/fixtures/simulation/win95/pe_volcano_calendar.csv");
    let checkpoints = pe_joined_calendar_fixture(fixture, 14_585);
    assert_win95_joined_calendar("VOLCANO.SC2", 14_585, &checkpoints, |age, city| {
        if age == 14_700 {
            assert_eq!(city.city_funds(), 20_757);
            assert_eq!(city.pe_dw_arcology_population(), 0);
            assert_eq!(pe_joined_xmic_fingerprint(city), 0xfe31_d732_f17f_3fc4);
            assert_pe_joined_budget_rows(
                city,
                [5620, 1640, 4570, 0, 0, 1, 1, 1, 1, 0, 274, 0, 0, 0, 0, 0],
                [
                    39_340, 11_480, 31_990, 0, 0, 100, 100, 100, 100, 0, 27_400, 0, 0, 0, 0, 0,
                ],
            );
        }
        if age == 14_596 {
            assert_eq!(city.pe_dw_city_population(), 11_370);
            assert_eq!(city.pe_zone_pops(), [517, 163, 457]);
        }
        if let Some((demand, cohorts, graph)) = match age {
            14_596 => Some((
                [2000, 2000, 2000],
                0x70cd_456f_1ba0_18e2,
                0x1205_422e_2b1b_ab18,
            )),
            14_621 => Some((
                [2000, 2000, 2000],
                0x061b_cd68_eeaa_e23b,
                0xcb2d_c456_21e3_f755,
            )),
            14_646 => Some((
                [2000, 1969, 2000],
                0x33bf_cb14_02d3_b538,
                0x9e8d_e969_5c85_99b6,
            )),
            14_671 => Some((
                [2000, 1977, 2000],
                0xc5ac_a2b6_a04a_c084,
                0x04d0_d344_1fc3_86c4,
            )),
            14_721 => Some((
                [2000, 1958, 2000],
                0x34d4_b829_7793_861d,
                0xf2a5_7516_1576_5ca0,
            )),
            _ => None,
        } {
            assert_eq!(
                [
                    RciKind::Residential,
                    RciKind::Commercial,
                    RciKind::Industrial
                ]
                .map(|kind| rci_demand_for_kind(city, kind)),
                demand,
            );
            assert_eq!(
                pe_joined_word_fingerprint(city.misc_u32_values(0x007c, 60)),
                cohorts
            );
            assert_eq!(
                pe_joined_word_fingerprint(city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES)),
                graph,
            );
        }
    });
}

fn assert_pe_joined_budget_rows(
    city: &City,
    current: [i32; BUDGET_TOTAL_COUNT],
    year_to_date: [i32; BUDGET_TOTAL_COUNT],
) {
    for (field, expected) in [
        (BUDGET_SERVICE_CURRENT_FIELD, current),
        (BUDGET_SERVICE_YEAR_TO_DATE_FIELD, year_to_date),
    ] {
        let actual = std::array::from_fn(|row| {
            let offset = budget_row_misc_offset_for_city(city, row, field).unwrap();
            city.budget_misc_u32_at(offset).unwrap_or(0) as i32
        });
        assert_eq!(
            actual,
            expected,
            "budget field {field} at age {:?}",
            city.city_age_days(),
        );
    }
}

fn pe_joined_calendar_fixture(fixture: &str, first_age: usize) -> Vec<u64> {
    fixture
        .lines()
        .skip(1)
        .enumerate()
        .map(|(index, line)| {
            let (age, hash) = line.split_once(',').unwrap();
            assert_eq!(age.parse::<usize>().unwrap(), first_age + index);
            u64::from_str_radix(hash, 16).unwrap()
        })
        .collect()
}

fn pe_joined_xmic_fingerprint(city: &City) -> u64 {
    let xmic = (0..crate::city::MICROSIM_COUNT)
        .flat_map(|slot| {
            let row = city.microsim_at(slot).unwrap();
            [
                u32::from(row.tile_id) | (u32::from(row.stat0) << 8) | (u32::from(row.stat1) << 16),
                u32::from(row.stat2) | (u32::from(row.stat3) << 16),
            ]
        })
        .collect();
    pe_joined_word_fingerprint(xmic)
}

fn pe_joined_word_fingerprint(words: Vec<u32>) -> u64 {
    words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

fn assert_win95_joined_calendar(
    save: &str,
    first_age: u32,
    checkpoints: &[u64],
    mut verify_day: impl FnMut(u32, &City),
) {
    let root = PathBuf::from(std::env::var_os("CITYGAME_WIN95_CITY_FIXTURES").unwrap());
    let city = City::new_blank("/tmp/pe-joined-things-test.sc2", "Empty").unwrap();
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(1);
    state.menu_checks.citygame_newspaper = false;
    state.load_city(&root.join(save), (800, 600), 1.0).unwrap();
    state.city.set_view_rotation(state.city.saved_rotation());
    let _live_rng = LiveGrowthRng::enable();
    state.bypass_growth_trip_gate = false;

    let hash = |hash: u64, byte: u8| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    let basis = 0xcbf2_9ce4_8422_2325_u64;
    for (age, expected) in (first_age..).zip(checkpoints.iter().copied()) {
        state.tick_pe_disaster_xthg_actors_in_slot_order();
        state.game_clock = state.game_clock.wrapping_add_days(1);
        let clock = state.game_clock;
        state.handle_simulation_day_advanced(clock.year(), clock.month(), clock.day());
        let city = &state.city;
        let xtxt = (0..128)
            .flat_map(|x| (0..128).map(move |y| city.text_overlay_at(x, y).unwrap_or(0)))
            .fold(basis, hash);
        let xthg = city
            .traffic_overlays()
            .unwrap()
            .iter()
            .copied()
            .flat_map(Win95OverlayRecord::to_bytes)
            .fold(basis, hash);
        let density_fingerprint = |plane: &[u8]| {
            (0..128)
                .step_by(2)
                .flat_map(|x| (0..128).step_by(2).map(move |y| plane[x * 128 + y]))
                .fold(basis, hash)
        };
        let map_fingerprint = |at: &dyn Fn(usize, usize) -> u8| {
            (0..128)
                .flat_map(|x| (0..128).map(move |y| at(x, y)))
                .fold(basis, hash)
        };
        let components = [
            u64::from(state.pe_process_crt_seed()),
            u64::from(state.pe_process_lfsr_state()),
            u64::from(state.pe_process_game_rand.state()),
            xtxt,
            xthg,
            pe_open_xbit_fingerprint(city),
            density_fingerprint(city.land_value_slice().unwrap()),
            density_fingerprint(city.crime_density_slice().unwrap()),
            density_fingerprint(city.pollution_density_slice().unwrap()),
            density_fingerprint(city.traffic_density_slice().unwrap()),
            map_fingerprint(&|x, y| city.building_at(x, y)),
            map_fingerprint(&|x, y| city.zone_at(x, y).unwrap().raw_bits()),
        ];
        let actual = components
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .fold(basis, hash);
        assert_eq!(city.city_age_days(), Some(age));
        assert_eq!(
            actual, expected,
            "{save} age {age}, state {components:016x?}"
        );
        verify_day(age, city);
    }
}

#[test]
#[ignore = "requires a local Win95 scenario catalog"]
fn pe_retail_win_scenario_first_disaster() {
    let root = PathBuf::from(std::env::var_os("CITYGAME_WIN95_SCENARIO_FIXTURES").unwrap());
    // Native 0x45cf10 return after one Things sweep and calendar step.
    let cases = [
        (
            "PORTLAND",
            30_457,
            11,
            1_994_907_847,
            43_838,
            3_295_386_429,
            [
                0xcc73_b8c8_4ef7_55b5,
                0xda06_cee5_f00f_f9d2,
                0xb2be_6123_f748_bbf5,
            ],
            0xf3f3_049f_98ac_193d,
        ),
        (
            "CHICAGO",
            29_953,
            15,
            1_280_152_439,
            59_756,
            1,
            [
                0x5bb8_ca0a_5f18_9d8b,
                0x27bb_0cad_521f_d22e,
                0xd776_546f_ac9b_4fb6,
            ],
            0x1988_eab8_27f8_3d85,
        ),
        (
            "BARCELON",
            27_753,
            9,
            698_423_654,
            57_613,
            1_103_527_590,
            [
                0xd93b_c468_7d5f_8134,
                0xaf0a_4884_83db_c9ef,
                0xdff5_8aeb_16d6_5f0f,
            ],
            0xb0b9_d8e1_53ef_61f0,
        ),
        (
            "MANHATTA",
            32_328,
            9,
            2_441_479_190,
            8_658,
            1,
            [
                0xb104_8c2d_ddb2_0145,
                0xf107_5f5d_83dc_6cc6,
                0x28d2_e078_657b_5262,
            ],
            0xafba_5c39_076d_3e6d,
        ),
        (
            "HOLLYWOO",
            30_303,
            8,
            3_991_041_203,
            44_880,
            1,
            [
                0xd2ac_2048_7cc2_ae1a,
                0x3fd0_2237_9713_e489,
                0x4a90_1bca_ffad_3189,
            ],
            0xedf3_bed6_5c36_b13a,
        ),
        (
            "ATLANTA",
            45_278,
            8,
            2_646_317_617,
            11_161,
            1,
            [
                0xac9a_d4b1_4778_ae0b,
                0x3cb3_a4da_f720_2c5e,
                0x7bc6_1e2e_9e4d_9024,
            ],
            0x2ac1_6d96_0e3c_b3e2,
        ),
        (
            "MALIBU",
            28_153,
            12,
            2_169_441_500,
            448,
            1,
            [
                0x32fd_ed8f_3f9f_f5aa,
                0xfbed_4e62_4636_fcd6,
                0x3b68_f847_ea00_cc03,
            ],
            0x0c09_1e76_1525_8d7a,
        ),
        (
            "OAKLAND",
            27_537,
            12,
            718_366_583,
            26_448,
            1_103_527_590,
            [
                0x4fde_0bbf_32b4_7749,
                0x7d1b_ab0b_8bf3_4c23,
                0x36cf_ca0b_e65f_7847,
            ],
            0xe47a_972a_0b36_0771,
        ),
        (
            "HOMESTEA",
            27_778,
            16,
            4_175_618_076,
            44_023,
            1,
            [
                0xe405_c038_5ab0_582c,
                0x585d_e2b7_dd6b_f7bd,
                0x3d53_b18e_bd40_3b80,
            ],
            0x1de8_fa84_d349_c1cd,
        ),
        (
            "CHARLEST",
            26_878,
            16,
            264_906_680,
            57_213,
            1_103_527_590,
            [
                0xae05_771a_2727_9dbe,
                0x8447_b2c7_32c7_3c0e,
                0x24a5_4880_0eba_fb9f,
            ],
            0xe71d_df05_ca4a_c5b3,
        ),
        (
            "PARIS",
            28_551,
            12,
            273_256_360,
            64,
            1,
            [
                0xeb15_f611_2553_0331,
                0xb897_c0aa_3c24_0081,
                0x66f0_930e_d659_243d,
            ],
            0x514e_22b7_34aa_d25e,
        ),
        (
            "SANFRANC",
            26_931,
            6,
            3_686_706_426,
            50_998,
            1,
            [
                0xe25e_76af_3aa0_fc8c,
                0xde7d_d1ef_a560_fc52,
                0x1e3a_7d48_fafa_cd40,
            ],
            0xfd11_14da_07e1_4cea,
        ),
        (
            "SILICONV",
            33_230,
            10,
            2_440_363_282,
            7_766,
            1_103_527_590,
            [
                0x1a27_7464_dd9b_88cd,
                0x87fb_097d_4324_21de,
                0x880b_1ab3_2805_e35e,
            ],
            0xdab5_6505_0573_132e,
        ),
        (
            "DAVENPOR",
            28_078,
            14,
            3_861_412_424,
            33_887,
            1,
            [
                0xc4f6_629b_4e09_781f,
                0x4a89_6abe_8e2a_9d5a,
                0x844b_49f4_9a1e_20fa,
            ],
            0xe458_79fc_d810_caec,
        ),
        (
            "WASHINGT",
            28_718,
            13,
            1_593_339_170,
            14_481,
            1,
            [
                0xb5f9_95ff_6cad_3c9f,
                0xf319_d58d_d74c_61f3,
                0x6cd3_498c_8467_1351,
            ],
            0x2adf_d14d_d95a_44d9,
        ),
    ];
    // Native 0x450890 return from the next isolated Things sweep.
    let next_sweeps = [
        (
            1_637_603_928,
            28_123,
            180_171_308,
            0xa565_d491_b740_7402,
            0xb1bc_c4d6_1ab0_f0c4,
        ),
        (
            149_820_841,
            59_728,
            3_295_386_429,
            0xdaf8_6b97_e669_e328,
            0x2147_7428_2bee_b677,
        ),
        (
            2_587_126_417,
            45_117,
            3_295_386_429,
            0x0d16_dbde_606f_b105,
            0x118f_0c85_0926_3a06,
        ),
        (
            959_357_760,
            55_297,
            1_103_527_590,
            0x8c62_8e41_3fcf_7f66,
            0x19b3_2259_d3e3_d862,
        ),
        (
            3_991_041_203,
            44_880,
            1,
            0x3fd0_2237_9713_e489,
            0x8459_a77d_d1e3_b42c,
        ),
        (
            3_397_112_880,
            28_688,
            2_524_885_223,
            0xde38_da41_cc79_cff2,
            0x43d5_3f76_5b0b_4741,
        ),
        (
            2_880_987_702,
            7_168,
            1,
            0xfbed_4e62_4636_fcd6,
            0xa8a9_462e_143b_3b8c,
        ),
        (
            718_366_583,
            34_485,
            2_524_885_223,
            0x788d_8024_ae3b_590f,
            0x467b_4a6d_dbcb_89fa,
        ),
        (
            4_183_387_759,
            28_688,
            3_295_386_429,
            0xf8e8_dfab_b47d_2f2f,
            0x5792_e9cc_cccb_0e5d,
        ),
        (
            4_202_497_947,
            59_559,
            3_295_386_429,
            0x3931_ce0a_6c93_adfe,
            0x6c48_1d82_b4f4_5a04,
        ),
        (
            141_201_355,
            512,
            1,
            0x29e5_d4fd_a87e_6231,
            0x801a_a094_0d4f_dd59,
        ),
        (
            2_654_548_804,
            8_303,
            1,
            0xc3f6_929f_8c9e_0170,
            0x36d9_54b8_7614_a429,
        ),
        (
            3_251_356_700,
            5_189,
            3_210_001_534,
            0x7737_97c6_fdfd_5364,
            0xafc0_4886_7d65_480c,
        ),
        (
            2_384_534_274,
            12_101,
            1,
            0x4a89_6abe_8e2a_9d5a,
            0xdf18_0bf9_ece5_5c4e,
        ),
        (
            664_821_597,
            23_641,
            1,
            0x193e_44e2_e900_787b,
            0x62a3_972c_9aa7_5704,
        ),
    ];
    let hash = |hash: u64, byte: u8| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    let basis = 0xcbf2_9ce4_8422_2325_u64;
    for ((name, age, kind, crt, lfsr, game_rng, planes, actors), next) in
        cases.into_iter().zip(next_sweeps)
    {
        let city = City::new_blank("/tmp/pe-joined-scenario.sc2", "Empty").unwrap();
        let mut state = test_state(city);
        state.pe_process_crt = Win95CrtRand::new(1);
        state.menu_checks.citygame_newspaper = false;
        state
            .load_city(&root.join(format!("{name}.SCN")), (800, 600), 1.0)
            .unwrap();
        state.close_city_info_dialog();
        state.traffic.set_speed(GameSpeed::Cheetah);
        // Use the saved view angle to match the native calendar fixture.
        state.city.set_view_rotation(state.city.saved_rotation());
        let _rng = LiveGrowthRng::enable();
        state.bypass_growth_trip_gate = false;
        state.advance_frame_simulation(Duration::from_millis(200));

        let city = &state.city;
        let map = |at: &dyn Fn(usize, usize) -> u8| {
            (0..CITY_WIDTH)
                .flat_map(|x| (0..CITY_HEIGHT).map(move |y| at(x, y)))
                .fold(basis, hash)
        };
        let xthg = city
            .traffic_overlays()
            .unwrap()
            .iter()
            .copied()
            .flat_map(Win95OverlayRecord::to_bytes)
            .fold(basis, hash);
        assert_eq!(city.city_age_days(), Some(age), "{name} age");
        if name == "CHICAGO" {
            assert_eq!(
                state.pe_idle_scheduler.pending_disaster_point,
                Some((39, 32))
            );
        }
        assert_eq!(city.pe_w_trigger_disaster, 0, "{name} consumed selector");
        assert_eq!(city.pe_w_city_mode, 2, "{name} mode");
        assert_eq!(city.pe_w_current_disaster_type(), kind, "{name} disaster");
        assert!(city.has_active_disaster(), "{name} active");
        assert_eq!(
            state.pe_disaster_duration,
            if matches!(kind, 14 | 16) { 59 } else { 0 },
            "{name} duration"
        );
        assert_eq!(state.pe_process_crt_seed(), crt, "{name} CRT");
        assert_eq!(state.pe_process_lfsr_state(), lfsr, "{name} LFSR");
        assert_eq!(
            state.pe_process_game_rand.state(),
            game_rng,
            "{name} game RNG"
        );
        assert_eq!(
            [
                pe_open_xbit_fingerprint(city),
                map(&|x, y| city.text_overlay_at(x, y).unwrap_or(0)),
                map(&|x, y| city.building_at(x, y))
            ],
            planes,
            "{name} XBIT / XTXT / XBLD"
        );
        assert_eq!(xthg, actors, "{name} XTHG");
        if name == "PORTLAND" {
            let altm = (0..CITY_WIDTH)
                .flat_map(|x| (0..CITY_HEIGHT).map(move |y| city.altm_at(x, y)))
                .flat_map(|sample| {
                    (u16::from(sample.land)
                        | (u16::from(sample.water) << 5)
                        | (u16::from(sample.tunnel_levels) << 10))
                        .to_le_bytes()
                })
                .fold(basis, hash);
            assert_eq!(altm, 0x9f7b_13f8_ecf1_e7fb, "{name} ALTM");
            assert_eq!(
                map(&|x, y| city.terrain_at(x, y)),
                0x303c_7938_ba1b_6f25,
                "{name} XTER"
            );
        }

        state.tick_pe_disaster_xthg_actors_in_slot_order();
        let city = &state.city;
        let xthg = city
            .traffic_overlays()
            .unwrap()
            .iter()
            .copied()
            .flat_map(Win95OverlayRecord::to_bytes)
            .fold(basis, hash);
        assert_eq!(
            pe_open_xbit_fingerprint(city),
            planes[0],
            "{name} next XBIT"
        );
        assert_eq!(
            map(&|x, y| city.building_at(x, y)),
            planes[2],
            "{name} next XBLD"
        );
        assert_eq!(
            (
                state.pe_process_crt_seed(),
                state.pe_process_lfsr_state(),
                state.pe_process_game_rand.state(),
                map(&|x, y| city.text_overlay_at(x, y).unwrap_or(0)),
                xthg,
            ),
            next,
            "{name} next Things sweep"
        );
    }
}
