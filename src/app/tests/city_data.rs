use super::*;

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
    assert_eq!(state.city.pe_w_city_rotation, 2);
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
