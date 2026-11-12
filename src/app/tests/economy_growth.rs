use super::*;

#[test]
fn day22_impeaches_mayor_when_funds_below_negative_100000() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(-100_000);
    let mut state = test_state(city);
    let population = live_population_stats_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(population);
    assert!(
        !state.announced_bankruptcy,
        "PE gate is strict: funds < -100000 (equal is solvent)"
    );

    state.city.set_city_funds(-100_001);
    let population = live_population_stats_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(population);
    assert!(state.announced_bankruptcy);
    assert_eq!(state.query_line, "Mayor Impeached - City Bankrupt");

    state.query_line.clear();
    let population = live_population_stats_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(population);
    assert_eq!(state.query_line, "", "bankruptcy announces once per load");
}

#[test]
fn saved_misc_speed_values_cover_original_file_spec() {
    let cases = [
        (1, GameSpeed::Paused),
        (2, GameSpeed::Turtle),
        (3, GameSpeed::Llama),
        (4, GameSpeed::Cheetah),
        (5, GameSpeed::AfricanSwallow),
    ];
    for (value, speed) in cases {
        assert_eq!(game_speed_from_sc2k_misc(value), Some(speed));
        assert_eq!(sc2k_speed_misc_value(speed), value);

        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_misc_u32_at(MISC_SPEED_OFFSET, value);
        assert_eq!(
            traffic_system_for_city(&city, EngineFidelity::Win95).speed(),
            speed
        );
    }

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_SPEED_OFFSET, 99);
    assert_eq!(
        traffic_system_for_city(&city, EngineFidelity::Win95).speed(),
        GameSpeed::Paused
    );
}

#[test]
fn mac_engine_speed_uses_four_native_selectors_and_persists_them() {
    let cases = [
        (1, GameSpeed::Paused),
        (2, GameSpeed::Turtle),
        (3, GameSpeed::Llama),
        (4, GameSpeed::Cheetah),
        (5, GameSpeed::Cheetah),
    ];
    for (value, speed) in cases {
        let mut city = City::new_blank("/tmp/sc2000-mac-speed.sc2", "test").expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_misc_u32_at(MISC_SPEED_OFFSET, 0xcafe_0000 | value);
        assert_eq!(
            traffic_system_for_city(&city, EngineFidelity::Mac68k).speed(),
            speed
        );
        assert_eq!(
            traffic_system_for_city(&city, EngineFidelity::Win95).speed(),
            if value == 5 {
                GameSpeed::AfricanSwallow
            } else {
                speed
            }
        );
    }

    let mut city = City::new_blank("/tmp/sc2000-mac-speed.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_misc_u32_at(MISC_SPEED_OFFSET, 0xcafe_0002);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Cheetah));
    assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(0xcafe_0004));

    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::AfricanSwallow));
    assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(0xcafe_0004));

    state.open_january_budget_review();
    assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(0xcafe_0004));
    state.close_city_info_dialog();
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Paused));
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(0xcafe_0001));

    let path = PathBuf::from("/tmp/sc2000-mac-speed-selector-roundtrip.city");
    state.city.save(&path).expect("save Mac speed selector");
    let loaded = City::load(path).expect("reload Mac speed selector");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.misc_u32_at(MISC_SPEED_OFFSET), Some(0xcafe_0001));
}

#[test]
fn ordinance_flags_read_and_toggle_misc_bits_preserving_other_bits() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, 0xabc0_0001);

    assert_eq!(ordinance_flags_for_city(&city), 0x00001);
    assert!(ordinance_enabled_for_city(&city, 0));
    assert!(!ordinance_enabled_for_city(&city, 2));

    assert!(set_city_ordinance_enabled(&mut city, 2, true));
    assert_eq!(
        city.misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET),
        Some(0xabc0_0005)
    );
    assert_eq!(ordinance_flags_for_city(&city), 0x00005);
    // PE @ `0x419348` rebuilds tech-unlock after ordinance flag walk.
    assert_ne!(city.pe_w_tech_unlock_mask(), 0);

    assert!(set_city_ordinance_enabled(&mut city, 0, false));
    assert_eq!(
        city.misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET),
        Some(0xabc0_0004)
    );
    assert!(!set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_COUNT,
        true
    ));
    assert_eq!(
        city.misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET),
        Some(0xabc0_0004)
    );
}

#[test]
fn ordinance_masks_follow_file_layout_across_engine_selection() {
    let mut mac = City::new_blank("/tmp/sc2000-mac-ordinance-layout.sc2", "MacOrdinances")
        .expect("blank city");
    mac.kind = CityFileKind::MacCity;
    mac.set_mac_state_misc_u32_at(MAC_ORDINANCE_FLAGS_MISC_OFFSET, 0xabc0_0001);
    mac.set_mac_state_misc_u32_at(0x0fa8, 0xfeed_fa00);

    assert_eq!(ordinance_flags_for_city(&mac), 1);
    assert_eq!(
        ordinance_flags_for_city_with_fidelity(&mac, EngineFidelity::Win95),
        1,
        "PE formulas use the Mac file's native ordinance mask in a hybrid session"
    );
    assert_eq!(
        ordinance_flags_for_city_with_fidelity(&mac, EngineFidelity::Mac68k),
        1
    );
    assert!(set_city_ordinance_enabled_with_fidelity(
        &mut mac,
        ORDINANCE_LEGALIZED_GAMBLING,
        true,
        EngineFidelity::Win95,
    ));
    assert_eq!(
        mac.mac_state_misc_u32_at(MAC_ORDINANCE_FLAGS_MISC_OFFSET),
        Some(0xabc0_0005),
        "a Win95 engine toggle writes the Mac file's native ordinance slot"
    );
    assert!(set_city_ordinance_enabled_with_fidelity(
        &mut mac,
        ORDINANCE_PARKING_FINES,
        true,
        EngineFidelity::Mac68k,
    ));
    assert_eq!(
        mac.mac_state_misc_u32_at(MAC_ORDINANCE_FLAGS_MISC_OFFSET),
        Some(0xabc0_000d)
    );
    assert_eq!(
        mac.mac_state_misc_u32_at(0x0fa8),
        Some(0xfeed_fa00),
        "ordinance toggles must preserve the adjacent Mac-owned field"
    );

    let mut pe =
        City::new_blank("/tmp/sc2000-pe-ordinance-layout.sc2", "PeOrdinances").expect("blank city");
    pe.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, 0xabc0_0001);
    pe.set_misc_u32_at(0x0f98, 0xaabb_ccdd);

    assert_eq!(ordinance_flags_for_city(&pe), 1);
    assert_eq!(
        ordinance_flags_for_city_with_fidelity(&pe, EngineFidelity::Mac68k),
        1,
        "Mac formulas use the Win95 file's native ordinance mask in a hybrid session"
    );
    assert!(set_city_ordinance_enabled_with_fidelity(
        &mut pe,
        ORDINANCE_LEGALIZED_GAMBLING,
        true,
        EngineFidelity::Mac68k,
    ));
    assert_eq!(
        pe.misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET),
        Some(0xabc0_0005)
    );
    assert!(set_city_ordinance_enabled_with_fidelity(
        &mut pe,
        ORDINANCE_PARKING_FINES,
        true,
        EngineFidelity::Win95,
    ));
    assert_eq!(
        pe.misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET),
        Some(0xabc0_000d)
    );
    assert_eq!(pe.misc_u32_at(0x0f98), Some(0xaabb_ccdd));
}

#[test]
fn auto_budget_reads_and_toggles_saved_misc_flag() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!auto_budget_enabled_for_city(&city));

    assert!(set_city_auto_budget_enabled(&mut city, true));
    assert!(auto_budget_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(AUTO_BUDGET_ENABLED_VALUE)
    );
    assert!(!set_city_auto_budget_enabled(&mut city, true));

    assert!(set_city_auto_budget_enabled(&mut city, false));
    assert!(!auto_budget_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(AUTO_BUDGET_DISABLED_VALUE)
    );
}

#[test]
fn high_misc_option_writes_preserve_blank_city_budget_defaults() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    assert!(set_city_auto_budget_enabled(&mut city, true));

    for index in 0..BUDGET_TAX_COUNT {
        assert_eq!(tax_rate_for_city(&city, index), DEFAULT_TAX_RATE);
        assert_eq!(
            city.misc_u32_at(tax_rate_misc_offset(index).expect("tax offset")),
            Some(DEFAULT_TAX_RATE)
        );
    }
    for index in 0..BUDGET_SERVICE_COUNT {
        assert_eq!(budget_service_funding_percent_for_city(&city, index), 100);
        assert_eq!(
            city.misc_u32_at(
                budget_service_funding_misc_offset(index).expect("service funding offset")
            ),
            Some(100)
        );
    }
}

#[test]
fn zero_filled_misc_extension_reads_as_new_city_budget_defaults() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_YEAR_END_OFFSET, 0);

    assert_eq!(tax_rate_for_city(&city, 0), DEFAULT_TAX_RATE);
    assert_eq!(tax_rate_for_city(&city, 1), DEFAULT_TAX_RATE);
    assert_eq!(tax_rate_for_city(&city, 2), DEFAULT_TAX_RATE);
    assert_eq!(budget_service_funding_percent_for_city(&city, 0), 100);
    assert_eq!(
        city.misc_u32_at(tax_rate_misc_offset(0).expect("tax offset")),
        Some(0),
        "extension zero-fills the dword; readers must not treat that as a saved 0% rate"
    );
}

#[test]
fn explicit_zero_tax_and_funding_on_nonempty_rows_are_preserved() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    stamp_complete_building_for_test(&mut city, 20, 20, 0xae, 3);
    set_budget_funding_for_test(&mut city, 0, 0);
    let tax_offset = budget_row_misc_offset(0, 0).expect("tax row");
    let mut tax_values = city.misc_u32_values(tax_offset, BUDGET_SERVICE_MISC_STRIDE);
    tax_values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
    tax_values[BUDGET_SERVICE_CURRENT_FIELD] = 648;
    tax_values[BUDGET_SERVICE_FUNDING_FIELD] = 0;
    city.set_misc_u32_values(tax_offset, &tax_values);

    assert_eq!(tax_rate_for_city(&city, 0), 0);
    assert_eq!(budget_service_funding_percent_for_city(&city, 0), 0);
}

#[test]
fn raw_budget_funding_remains_available_to_pe_annual_selectors() {
    let mut city =
        City::new_blank("/tmp/sc2000-annual-raw-budget.sc2", "RawBudget").expect("blank city");
    for budget_index in [0, 1, 2, 3, 4] {
        let row_offset = budget_service_misc_offset(budget_index, 0).expect("service row offset");
        let mut row = vec![0; BUDGET_SERVICE_MISC_STRIDE];
        row[BUDGET_SERVICE_FUNDING_FIELD] = 120;
        city.set_budget_misc_u32_values(row_offset, &row);

        assert_eq!(
            budget_service_funding_raw_for_city(&city, budget_index),
            120
        );
        assert_eq!(
            budget_service_funding_percent_for_city(&city, budget_index),
            100
        );
    }
}

#[test]
fn city_info_budget_adjustments_noop_at_clamps_without_side_effects() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.clear_dirty();

    assert!(!state.adjust_budget_tax_rate(0, 0));
    assert!(!state.adjust_budget_service_funding(0, 1));
    assert_eq!(
        state
            .city
            .misc_u32_at(tax_rate_misc_offset(0).expect("tax offset")),
        Some(0)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_funding_misc_offset(0).expect("funding offset")),
        Some(0)
    );
    assert!(!state.city.has_dirty_tiles());

    let tax_offset = tax_rate_misc_offset(0).expect("tax offset");
    let funding_offset = budget_service_funding_misc_offset(0).expect("funding offset");
    set_misc_u32_for_test(&mut state.city, tax_offset, MAX_TAX_RATE);
    set_budget_funding_for_test(&mut state.city, 0, 0);
    state.city.clear_dirty();

    assert!(!state.adjust_budget_tax_rate(0, 1));
    assert!(!state.adjust_budget_service_funding(0, -1));
    assert_eq!(state.city.misc_u32_at(tax_offset), Some(MAX_TAX_RATE));
    assert_eq!(state.city.misc_u32_at(funding_offset), Some(0));
    assert!(!state.city.has_dirty_tiles());
}

#[test]
fn city_state_toggles_auto_budget_saved_flag() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.toggle_auto_budget();
    assert!(auto_budget_enabled_for_city(&state.city));
    assert_eq!(
        state.city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(AUTO_BUDGET_ENABLED_VALUE)
    );

    state.toggle_auto_budget();
    assert!(!auto_budget_enabled_for_city(&state.city));
    assert_eq!(
        state.city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(AUTO_BUDGET_DISABLED_VALUE)
    );
}

#[test]
fn mac_city_auto_budget_uses_known_native_slot_in_win95_engine() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-auto-budget-state.sc2", "MacBudget").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let sentinel = 0x5a3c_91e7;
    city.set_mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET, sentinel);
    let mut state = test_state(city);

    assert!(auto_budget_enabled_for_engine(
        &state.city,
        state.engine_fidelity
    ));
    assert!(
        !set_city_auto_budget_enabled(&mut state.city, true),
        "the PE auto-budget setter must reject a Mac-owned slot"
    );
    state.sync_edit_map_menu();
    assert!(state.menu_checks.auto_budget);
    state.toggle_auto_budget();
    assert_eq!(
        state.city.mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(0),
        "the known Mac byte is cleared through its native state codec"
    );
}

#[test]
fn mac_engine_auto_budget_uses_native_option_without_pe_funding_changes() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-auto-budget-native.sc2", "MacBudget").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET, AUTO_BUDGET_DISABLED_VALUE);
    let funding_offset = budget_service_funding_misc_offset_for_city(&city, 0)
        .expect("Mac service-funding row offset");
    city.set_budget_misc_u32_values(funding_offset, &[0]);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert!(!auto_budget_enabled_for_engine(
        &state.city,
        state.engine_fidelity
    ));
    state.toggle_auto_budget();
    assert!(auto_budget_enabled_for_engine(
        &state.city,
        state.engine_fidelity
    ));
    assert_eq!(
        state.city.mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(AUTO_BUDGET_ENABLED_VALUE)
    );
    assert_eq!(
        state.city.budget_misc_u32_at(funding_offset),
        Some(0),
        "the Mac window option must not run CityGame's PE service-funding policy"
    );

    let path = PathBuf::from("/tmp/sc2000-mac-auto-budget-roundtrip.city");
    state
        .city
        .save(&path)
        .expect("save native auto-budget option");
    let loaded = City::load(path).expect("reload native auto-budget option");
    assert!(mac_auto_budget_enabled_for_city(&loaded));

    state.toggle_auto_budget();
    assert!(!mac_auto_budget_enabled_for_city(&state.city));
}

#[test]
fn mac_engine_auto_budget_reads_and_toggles_win_layout_option() {
    let mut city = City::new_blank("/tmp/sc2000-mac-engine-win-layout-auto-budget.sc2", "Mixed")
        .expect("blank Win-layout city");
    city.set_misc_u32_at(AUTO_BUDGET_MISC_OFFSET, AUTO_BUDGET_DISABLED_VALUE);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert!(!auto_budget_enabled_for_engine(
        &state.city,
        state.engine_fidelity
    ));
    state.toggle_auto_budget();
    assert!(auto_budget_enabled_for_engine(
        &state.city,
        state.engine_fidelity
    ));
    assert_eq!(
        state.city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(AUTO_BUDGET_ENABLED_VALUE),
        "Mac engine toggle uses the Win-layout MISC DWORD codec"
    );

    let path = PathBuf::from("/tmp/sc2000-mac-engine-win-layout-auto-budget-roundtrip.sc2");
    state.city.save(&path).expect("save Win-layout option");
    let loaded = City::load(path).expect("reload Win-layout option");
    assert!(auto_budget_enabled_for_engine(
        &loaded,
        EngineFidelity::Mac68k
    ));

    state.toggle_auto_budget();
    assert!(!auto_budget_enabled_for_engine(
        &state.city,
        state.engine_fidelity
    ));
    assert_eq!(
        state.city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
        Some(AUTO_BUDGET_DISABLED_VALUE)
    );
}

#[test]
fn win95_year_end_microsims_preserve_mac_arrest_and_prison_state() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-win95-year-end.sc2", "MacYearEnd").expect("blank city");
    city.provision_microsim_for_building(214)
        .expect("provision an XMIC record so the annual worker runs");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_OLD_ARRESTS_OFFSET, 0xaaaa_1038);
    city.set_mac_state_misc_u32_at(MISC_PRISON_BONUS_OFFSET, 0xbbbb_103c);

    city.refresh_structure_microsim_yearly(0, 100, 100, 100, 100, 100);

    assert_eq!(city.pe_w_old_arrests(), 0);
    assert_eq!(city.pe_w_prison_bonus(), 0);
    assert_eq!(
        city.mac_state_misc_u32_at(MISC_OLD_ARRESTS_OFFSET),
        Some(0xaaaa_1038),
        "Win95 annual arrest output must remain runtime-only on a Mac-layout save"
    );
    assert_eq!(
        city.mac_state_misc_u32_at(MISC_PRISON_BONUS_OFFSET),
        Some(0xbbbb_103c),
        "Win95 annual prison output must remain runtime-only on a Mac-layout save"
    );
}

#[test]
fn win95_yearly_arcology_total_persists_to_shared_mac_layout_slot() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-win95-arcology-mac-layout-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "CrossLayoutArcology").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 600);
    city.set_mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 7_000);
    stamp_complete_building_for_test(&mut city, 14, 2, 251, 4);
    let arco = city
        .provision_microsim_for_building(251)
        .expect("arcology XMIC");
    let arco_index = usize::from(arco - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(arco_index, 12));
    city.prepare_pe_fidelity_from_saved_city();

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.sim_calendar_yearly_updates();

    let updated = state.city.pe_dw_arcology_population();
    assert_ne!(updated, 600, "the PE annual worker updates the total");
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET),
        Some(updated),
        "the Mac container saves the shared arcology-population role"
    );
    state.city.save(&path).expect("save Mac-layout city");
    let loaded = City::load(&path).expect("reload Mac-layout city");
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET),
        Some(updated),
        "PE-computed population survives Mac-layout save/reload"
    );
}

#[test]
fn city_state_auto_budget_toggle_does_not_fund_or_snapshot_a_month() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    set_budget_funding_for_test(&mut city, 0, 80);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 3);

    state.toggle_auto_budget();

    assert!(auto_budget_enabled_for_city(&state.city));
    assert_eq!(budget_service_funding_percent_for_city(&state.city, 0), 80);
    let values = state.city.misc_u32_values(
        budget_service_misc_offset(0, 0).expect("police budget offset"),
        BUDGET_SERVICE_MISC_STRIDE,
    );
    assert_eq!(values[4 + 2 * 2], 0);
}

#[test]
fn city_state_issues_and_repays_budget_bonds() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(5_000);
    let mut state = test_state(city);

    assert!(state.city_info_dialog_action_enabled(CityInfoDialogAction::IssueBond));
    assert!(!state.city_info_dialog_action_enabled(CityInfoDialogAction::RepayBond));
    assert!(state.issue_budget_bond());
    assert_eq!(state.city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(1));
    assert_eq!(state.city.city_funds(), 15_000);
    assert!(state.city_info_dialog_action_enabled(CityInfoDialogAction::RepayBond));

    assert!(state.repay_budget_bond());
    assert_eq!(state.city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(0));
    assert_eq!(state.city.city_funds(), 5_000);
    assert!(!state.city_info_dialog_action_enabled(CityInfoDialogAction::RepayBond));

    assert!(!state.repay_budget_bond());
    assert_eq!(state.city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(0));
    assert_eq!(state.city.city_funds(), 5_000);

    state.city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 1);
    state.city.set_city_funds(BUDGET_BOND_AMOUNT - 1);
    assert!(!state.city_info_dialog_action_enabled(CityInfoDialogAction::RepayBond));
    assert!(!state.repay_budget_bond());
    assert_eq!(state.city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(1));
    assert_eq!(state.city.city_funds(), BUDGET_BOND_AMOUNT - 1);
}

#[test]
fn mac_growth_selector_matches_recovered_a5_table_for_all_entries() {
    // Expected growth classes for the complete building selector table.
    let recovered_mac: [i16; 88] = [
        // XBLD 0x70..=0x7f
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // XBLD 0x80..=0x8f
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, // XBLD 0x90..=0x9f
        3, 3, 3, 3, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 2, 2, // XBLD 0xa0..=0xaf
        2, 2, 3, 3, 3, 3, 2, 2, 3, 3, 2, 2, 3, 3, 4, 4, // XBLD 0xb0..=0xbf
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, // XBLD 0xc0..=0xc7
        4, 4, 4, 4, 4, 4, 0, 0,
    ];

    for (index, expected) in recovered_mac.into_iter().enumerate() {
        let tile = 0x70 + index as u8;
        assert_eq!(
            mac_growth_level_for_xbld(tile),
            expected,
            "Mac selector for XBLD {tile:#04x}"
        );
        if tile <= 0xc4 {
            assert_eq!(
                mac_growth_level_for_xbld(tile),
                pe_growth_level_for_xbld(tile),
                "shared Mac/PE selector for XBLD {tile:#04x}"
            );
        }
    }
    assert_eq!(mac_growth_level_for_xbld(0xc5), 4);
    assert_eq!(pe_growth_level_for_xbld(0xc5), 4);
    assert_eq!(mac_growth_level_for_xbld(0x6f), 0);
    assert_eq!(mac_growth_level_for_xbld(0xc8), 0);
}

#[test]
fn mac_rci_scan_accumulates_class_two_weight_in_slot_seven() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-rci-class-two.sc2", "MacClassTwo").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, 0x81);
    city.set_building(40, 40, 0xc5);
    let mut rng = MacToolboxRng::new(1);

    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.mac_state_misc_u32_at(0x05f0 + 7 * 4), Some(36));
    assert_eq!(
        city.mac_state_misc_u32_at(0x05f0 + 1 * 4),
        Some(0),
        "class 2 feeds the separate slot-7 aggregate, not RCI category 1"
    );
    let mut expected_rng = MacToolboxRng::new(1);
    expected_rng.random_i16(); // failed route still reaches the class-2 roll
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_growth_selector_matches_all_reconstructed_code8_words() {
    // CODE 0 DATA/ZERO/DREL reconstruction, CODE 8 class bytes at A5-0x34a0
    // and selector WORDs at A5-0x354c. Both tables use XBLD - 0x70 and cover
    // 0x70..=0xff. The class-table SHA-256 is
    // e9543b209bde52ae8ffd0940bdbbd1934ffb3edef205f4496eae4d40893948e4.
    const EXPECTED_CLASSES: [u8; 144] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0x70..=0x7f
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 0, 0, 0, 0, // 0x80..=0x8f
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0x90..=0x9f
        0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 0, 0, // 0xa0..=0xaf
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0xb0..=0xbf
        0, 0, 1, 1, 2, 2, 0, 112, 0, 140, 0, 144, 0, 174, 0, 124, // 0xc0..=0xcf
        0, 148, 0, 153, 0, 178, 0, 132, 0, 158, 0, 162, 0, 188, 0, 136, // 0xd0..=0xdf
        0, 166, 0, 168, 0, 194, 0, 138, 0, 170, 0, 172, 0, 196, 0, 12, // 0xe0..=0xef
        0, 4, 0, 4, 0, 4, 0, 8, 0, 5, 0, 5, 0, 10, 0, 4, // 0xf0..=0xff
    ];

    // CODE 0 DATA/ZERO/DREL reconstruction, CODE 8 selector WORDs at
    // A5-0x354c, indexed by XBLD - 0x70. The 144 values cover 0x70..=0xff.
    const EXPECTED: [u16; 144] = [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2,
        2, 2, 3, 3, 3, 3, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 2, 2, 2, 2, 3, 3, 3, 3, 2, 2, 3, 3, 2, 2,
        3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0x0101, 0x0202, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0101,
        0x0101, 0x0202, 0x0202, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0101, 0x0202, 112, 140, 144, 174,
        124, 148, 153, 178, 132, 158, 162, 188, 136, 166, 168,
    ];
    const LOOKUPS: [(u16, i16); 22] = [
        (0, 0),
        (1, 1),
        (2, 8),
        (3, 12),
        (4, 36),
        (112, 0),
        (124, 0),
        (132, 0),
        (136, 0),
        (140, 0),
        (144, 0),
        (148, 0),
        (153, 0),
        (158, 0),
        (162, 0),
        (166, 20),
        (168, 20),
        (174, 15),
        (178, 0),
        (188, 30),
        (257, 0),
        (514, -1),
    ];

    for (index, &expected) in EXPECTED.iter().enumerate() {
        let tile = 0x70 + index as u8;
        let actual_class = super::super::budget::mac_growth_class_for_xbld(tile);
        assert_eq!(
            actual_class, EXPECTED_CLASSES[index],
            "class table XBLD {tile:#04x}"
        );
        let actual = super::super::budget::mac_growth_coefficient_for_xbld(tile);
        assert_eq!(actual, expected, "XBLD {tile:#04x}");
    }
    for (coefficient, expected) in LOOKUPS {
        assert_eq!(
            super::super::budget::mac_growth_counter_delta(coefficient),
            expected,
            "counter lookup for selector {coefficient}"
        );
    }
}

#[test]
fn mac_rci_special_class_zero_tiles_use_their_reconstructed_signed_weights() {
    for (tile, expected_delta) in [(0xf4, 15), (0xfc, 30), (0xfe, 20)] {
        let mut city = City::new_blank("/tmp/sc2000-mac-rci-special-weight.sc2", "MacRciWeight")
            .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.saved_rotation = 0;
        city.set_zone(40, 40, 0x81);
        city.set_building(40, 40, tile);

        let mut rng = MacToolboxRng::new(1);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        assert_eq!(
            city.mac_state_misc_u32_at(0x05f0 + 4),
            Some(expected_delta),
            "XBLD {tile:#04x} must contribute its reconstructed Mac RCI weight"
        );
        assert_eq!(city.building_at(40, 40), tile);
        let mut expected_rng = MacToolboxRng::new(1);
        expected_rng.random_i16(); // class-0 threshold, after the failed route
        assert_eq!(rng.seed(), expected_rng.seed(), "XBLD {tile:#04x}");
    }
}

#[test]
fn mac_high_xbld_values_match_reconstructed_rci_counter_table() {
    const EXPECTED_CLASSES: [u8; 39] = [
        0, 144, 0, 174, 0, 124, 0, 148, 0, 153, 0, 178, 0, 132, 0, 158, 0, 162, 0, 188, 0, 136, 0,
        166, 0, 168, 0, 194, 0, 138, 0, 170, 0, 172, 0, 196, 0, 12, 0,
    ];
    const NONZERO_COEFFICIENTS: [(u8, u16); 8] = [
        (0xd2, 257),
        (0xd3, 514),
        (0xe1, 257),
        (0xe2, 257),
        (0xe3, 514),
        (0xe4, 514),
        (0xef, 257),
        (0xf0, 514),
    ];

    let mut city = City::new_blank(
        "/tmp/sc2000-mac-high-xbld-counter-table.sc2",
        "MacHighXbldCounterTable",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.saved_rotation = 0;
    // Synthetic XBLD table coverage only. Export 116's actor marker is XTXT,
    // so these values do not describe type-3 actor placement.
    let (x, y) = (0, 0);
    city.set_zone(x, y, 0x81); // category 1 plus the rotation-0 scan corner

    for tile in 0xca_u8..=0xf0 {
        city.set_building(x, y, tile);

        let expected_coefficient = NONZERO_COEFFICIENTS
            .iter()
            .find_map(|&(candidate, coefficient)| (candidate == tile).then_some(coefficient))
            .unwrap_or(0);
        assert_eq!(
            super::super::budget::mac_growth_class_for_xbld(tile),
            EXPECTED_CLASSES[usize::from(tile - 0xca)]
        );
        assert_eq!(
            super::super::budget::mac_growth_coefficient_for_xbld(tile),
            expected_coefficient,
            "XBLD {tile:#04x}"
        );
        assert_eq!(
            super::super::budget::mac_growth_counter_delta(expected_coefficient),
            if expected_coefficient == 514 { -1 } else { 0 },
            "XBLD {tile:#04x} signed counter lookup"
        );

        city.set_mac_state_misc_u32_at(0x05f0 + 4, 100);
        let mut rng = MacToolboxRng::new(1);
        super::super::budget::mac_budget_accumulate_monthly_rci_scan_partition(
            &mut city, 0, 0, &mut rng,
        );

        // Only class-0 XBLD values with coefficient 514 write -1. This is a
        // synthetic map-plane check, unrelated to XTXT actor markers.
        let expected_counter = if matches!(tile, 0xe4 | 0xf0) { 99 } else { 100 };
        assert_eq!(
            city.mac_state_misc_u32_at(0x05f0 + 4),
            Some(expected_counter),
            "XBLD {tile:#04x}"
        );
        assert_eq!(
            city.building_at(x, y),
            tile,
            "XBLD {tile:#04x} remains live"
        );
    }
}

#[test]
fn mac_rci_class_two_threshold_rejection_and_accepted_square_draws() {
    let mut rejected = City::new_blank(
        "/tmp/sc2000-mac-rci-class-two-reject.sc2",
        "MacClassTwoReject",
    )
    .expect("blank city");
    let mut reject_rng = MacToolboxRng::new(7);
    let mut expected_reject_rng = MacToolboxRng::new(7);
    expected_reject_rng.random_i16();
    super::super::budget::mac_budget_try_class_two_growth(
        &mut rejected,
        40,
        40,
        1,
        4,
        0,
        &mut reject_rng,
    );
    assert_eq!(reject_rng.seed(), expected_reject_rng.seed());
    assert_eq!(rejected.building_at(40, 40), 0);

    let mut accepted = City::new_blank(
        "/tmp/sc2000-mac-rci-class-two-accept.sc2",
        "MacClassTwoAccept",
    )
    .expect("blank city");
    let seed = (1..10_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            (rng.random_i16() as u16) < 3750
        })
        .expect("find a class-2 acceptance seed");
    let mut expected_rng = MacToolboxRng::new(seed);
    let _threshold = expected_rng.random_i16();
    let tile_draw = expected_rng.random_i16() as u16;
    let expected_tile = 0xae + (tile_draw % 4) as u8;
    let mut rng = MacToolboxRng::new(seed);
    super::super::budget::mac_budget_try_class_two_growth(
        &mut accepted,
        40,
        40,
        1,
        4,
        1000,
        &mut rng,
    );
    // Coefficient 4 stamps a 3x3 square (extent coefficient / 2).
    for x in 40..=42 {
        for y in 38..=40 {
            assert_eq!(accepted.building_at(x, y), expected_tile);
            assert_eq!(accepted.tile_bits_at(x, y).unwrap().raw_bits() & 0xe0, 0xe0);
        }
    }
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_rci_generic_coefficient_two_uses_land_value_gate_and_mode_two_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-mac-rci-generic-two.sc2", "MacGenericTwo")
        .expect("blank city");
    stamp_xval_for_test(&mut city, 0x60);
    let seed = (1..10_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            (rng.random_i16() as u16) < 1000
        })
        .expect("find generic coefficient-2 acceptance seed");
    let mut expected_rng = MacToolboxRng::new(seed);
    let _threshold = expected_rng.random_i16();
    let tile_draw = expected_rng.random_i16() as u16;
    let expected_tile = 0xa8 + (tile_draw % 2) as u8;
    let mut rng = MacToolboxRng::new(seed);
    super::super::budget::mac_budget_try_generic_growth(
        &mut city, 40, 40, 0x82, 2, 2, 1000, &mut rng,
    );
    for x in 40..=41 {
        for y in 39..=40 {
            assert_eq!(city.building_at(x, y), expected_tile);
            assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits() & 0xe0, 0xe0);
        }
    }
    assert_eq!(rng.seed(), expected_rng.seed());

    let mut gated_city =
        City::new_blank("/tmp/sc2000-mac-rci-generic-two-gate.sc2", "MacGenericGate")
            .expect("blank city");
    let mut gated_rng = MacToolboxRng::new(seed);
    let before = gated_rng.seed();
    super::super::budget::mac_budget_try_generic_growth(
        &mut gated_city,
        40,
        40,
        0x82,
        2,
        2,
        1000,
        &mut gated_rng,
    );
    assert_eq!(gated_rng.seed(), before, "density gate precedes the roll");
    assert_eq!(gated_city.building_at(40, 40), 0);
}

#[test]
fn mac_export242_selector_zero_uses_native_residential_building_bands() {
    for (land_value, band) in [(0, 0), (0x40, 1), (0x80, 2), (0xff, 2)] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-export242-land-value-bands.sc2",
            "MacExport242Bands",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_zone(40, 40, 1);
        stamp_xval_for_test(&mut city, land_value);

        let mut expected_rng = MacToolboxRng::new(0x2a42);
        let draw = expected_rng.random_u16();
        let expected_tile = 0x70 + band * 4 + (draw as u8 & 3);
        let mut rng = MacToolboxRng::new(0x2a42);
        super::super::budget::mac_budget_place_growth_pattern(&mut city, 40, 40, 1, 0, &mut rng);

        assert_eq!(city.building_at(40, 40), expected_tile);
        assert_eq!(rng.seed(), expected_rng.seed());
    }
}

#[test]
fn mac_rci_generic_mode_above_three_consumes_roll_without_placement() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-rci-generic-unknown-mode.sc2",
        "MacGenericUnknown",
    )
    .expect("blank city");
    city.set_building(40, 40, 0xf1);
    let seed = (1..10_000)
        .find(|seed| u32::from(MacToolboxRng::new(*seed).random_u16()) < 32767 * 3 / 113)
        .unwrap();
    let mut rng = MacToolboxRng::new(seed);
    let mut expected_rng = MacToolboxRng::new(seed);
    let _threshold_roll = expected_rng.random_i16();

    // This signed-WORD route result and seed accept the caller threshold.
    // FUN_000370fa has map modes only for 0..=3 and returns for coefficient 112.
    super::super::budget::mac_budget_try_generic_growth(
        &mut city, 40, 40, 0x82, 2, 112, 32767, &mut rng,
    );

    assert_eq!(city.building_at(40, 40), 0xf1);
    assert_eq!(
        rng.seed(),
        expected_rng.seed(),
        "only the caller roll is used"
    );
}

#[test]
fn mac_rci_scan_runs_low_xbld_override_without_corner_marker() {
    let make_city = || {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-rci-low-override-scan.sc2",
            "MacLowOverrideScan",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_zone(40, 40, 0x82); // category 2; no active scan-corner marker
        city.set_building(40, 40, 0x10); // early low-XBLD override
        city.set_tile_bits(40, 40, TileBits::new(0x40)); // powered, no route marker
        city.set_building(40, 41, 0x20); // adjacent network supplies route seed/query
        city.set_zone(40, 42, 3); // acceptable commercial-route destination
        city
    };
    let seed = (1..10_000)
        .find(|seed| {
            let city = make_city();
            let mut rng = MacToolboxRng::new(*seed);
            let route = mac_transit_route_search(
                40,
                40,
                2,
                0,
                100,
                |x, y| MacTransitRouteCell {
                    building: city.building_at(x, y),
                    zone_low: city.zone_at(x, y).map_or(0, |zone| zone.raw_bits() & 0x0f),
                    tunnel_levels: city.tunnel_levels_at(x, y),
                    underground: city.underground_at(x, y),
                    text_overlay: city.text_overlay_at(x, y).unwrap_or(0),
                },
                || rng.random_i16(),
            );
            route.is_some() && (rng.random_i16() as u16) < 6000
        })
        .expect("find route plus coefficient-zero acceptance seed");

    let mut city = make_city();
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert!(matches!(city.building_at(40, 40), 0x88 | 0x89));
    assert_eq!(city.zone_at(40, 40).unwrap().raw_bits() & 0xf0, 0xf0);
}

#[test]
fn mac_rci_low_xbld_override_rejects_missing_network_and_failed_power() {
    let make_city = || {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-rci-low-override-reject.sc2",
            "MacLowOverrideReject",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_zone(40, 40, 0x82);
        city.set_building(40, 40, 0x10);
        city
    };

    let mut no_network = make_city();
    let mut no_network_rng = MacToolboxRng::new(17);
    let no_network_seed = no_network_rng.seed();
    mac_budget_accumulate_monthly_rci_scan_partition(&mut no_network, 0, 0, &mut no_network_rng);
    assert_eq!(no_network_rng.seed(), no_network_seed);
    assert_eq!(no_network.building_at(40, 40), 0x10);

    let mut unpowered = make_city();
    unpowered.set_building(40, 41, 0x20); // query succeeds, route power fails
    let mut rng = MacToolboxRng::new(23);
    let mut expected_rng = MacToolboxRng::new(23);
    expected_rng.random_i16(); // coefficient-zero threshold still runs at local_c=0
    mac_budget_accumulate_monthly_rci_scan_partition(&mut unpowered, 0, 0, &mut rng);
    assert_eq!(rng.seed(), expected_rng.seed());
    assert_eq!(unpowered.building_at(40, 40), 0x10);
}

#[test]
fn mac_rci_generic_modes_one_and_three_match_neighbor_and_rotated_gates() {
    let mut neighbor_city = City::new_blank("/tmp/sc2000-mac-rci-generic-one.sc2", "MacGenericOne")
        .expect("blank city");
    for (x, y) in [(50, 50), (51, 50), (50, 51), (51, 51)] {
        neighbor_city.set_zone(x, y, 0x82);
        neighbor_city.set_altitude(x, y, 0, 0);
    }
    stamp_xval_for_test(&mut neighbor_city, 0x20);
    let neighbor_seed = (1..10_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            (rng.random_i16() as u16) < 1500
        })
        .expect("find generic coefficient-1 acceptance seed");
    let mut expected_neighbor_rng = MacToolboxRng::new(neighbor_seed);
    let _threshold = expected_neighbor_rng.random_i16();
    let neighbor_draw = expected_neighbor_rng.random_i16() as u16;
    let neighbor_tile = 0xa6 + (neighbor_draw % 2) as u8;
    let mut neighbor_rng = MacToolboxRng::new(neighbor_seed);
    super::super::budget::mac_budget_try_generic_growth(
        &mut neighbor_city,
        50,
        50,
        0x82,
        2,
        1,
        1000,
        &mut neighbor_rng,
    );
    for x in 50..=51 {
        for y in 50..=51 {
            assert_eq!(neighbor_city.building_at(x, y), neighbor_tile);
        }
    }
    assert_eq!(neighbor_rng.seed(), expected_neighbor_rng.seed());

    let mut alternate_neighbor_city = City::new_blank(
        "/tmp/sc2000-mac-rci-generic-one-alternate.sc2",
        "MacGenericOneAlt",
    )
    .expect("blank city");
    for (x, y) in [(50, 50), (50, 51), (49, 51), (49, 50)] {
        alternate_neighbor_city.set_zone(x, y, 0x82);
        alternate_neighbor_city.set_altitude(x, y, 0, 0);
    }
    stamp_xval_for_test(&mut alternate_neighbor_city, 0x20);
    let mut alternate_rng = MacToolboxRng::new(neighbor_seed);
    let mut expected_alternate_rng = MacToolboxRng::new(neighbor_seed);
    expected_alternate_rng.random_i16();
    let alternate_draw = expected_alternate_rng.random_i16() as u16;
    let alternate_tile = 0xa6 + (alternate_draw % 2) as u8;
    super::super::budget::mac_budget_try_generic_growth(
        &mut alternate_neighbor_city,
        50,
        50,
        0x82,
        2,
        1,
        1000,
        &mut alternate_rng,
    );
    for x in 49..=50 {
        for y in 50..=51 {
            assert_eq!(alternate_neighbor_city.building_at(x, y), alternate_tile);
        }
    }
    assert_eq!(alternate_rng.seed(), expected_alternate_rng.seed());

    let mut rotating_city =
        City::new_blank("/tmp/sc2000-mac-rci-generic-three.sc2", "MacGenericThree")
            .expect("blank city");
    for (x, y) in [
        (50, 50),
        (50, 49),
        (50, 48),
        (51, 48),
        (52, 48),
        (52, 49),
        (52, 50),
        (51, 50),
    ] {
        rotating_city.set_zone(x, y, 0x82);
        rotating_city.set_altitude(x, y, 0, 0);
    }
    rotating_city.set_building(49, 51, 0x23);
    stamp_xval_for_test(&mut rotating_city, 0xc0);
    let rotating_seed = (1..10_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            (rng.random_i16() as u16) < 750
        })
        .expect("find generic coefficient-3 acceptance seed");
    let mut expected_rotating_rng = MacToolboxRng::new(rotating_seed);
    let _threshold = expected_rotating_rng.random_i16();
    let rotating_draw = expected_rotating_rng.random_i16() as u16;
    let rotating_tile = 0xc2 + (rotating_draw % 2) as u8;
    let mut rotating_rng = MacToolboxRng::new(rotating_seed);
    super::super::budget::mac_budget_try_generic_growth(
        &mut rotating_city,
        50,
        50,
        0x82,
        2,
        3,
        1000,
        &mut rotating_rng,
    );
    for x in 50..=52 {
        for y in 48..=50 {
            assert_eq!(rotating_city.building_at(x, y), rotating_tile);
        }
    }
    assert_eq!(rotating_rng.seed(), expected_rotating_rng.seed());
}

#[test]
fn mac_rci_rotating_gate_accepts_native_building_range_below_ae() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-rci-rotating-native-ceiling.sc2",
        "MacRotatingNativeCeiling",
    )
    .expect("blank city");
    for (x, y) in [
        (50, 50),
        (50, 49),
        (50, 48),
        (51, 48),
        (52, 48),
        (52, 49),
        (52, 50),
        (51, 50),
    ] {
        city.set_zone(x, y, 0x82);
        city.set_altitude(x, y, 0, 0);
    }
    // Native FUN_000370fa mode 3 admits perimeter XBLD < 0xae, including
    // developed tiles that the mode-1 helper's stricter 0x8c ceiling rejects.
    city.set_building(50, 50, 0xa9);
    city.set_building(49, 51, 0x23);

    let mut rng = MacToolboxRng::new(17);
    let mut expected_rng = MacToolboxRng::new(17);
    for _ in 0..5 {
        expected_rng.random_i16();
    }

    super::super::budget::mac_budget_try_generic_rotating_footprint(&mut city, 50, 50, 2, &mut rng);

    assert_ne!(city.building_at(50, 50), 0xa9);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_rci_rotated_edge_places_native_marker_oriented_anchors() {
    let cases = [
        (0x10, [(-1, 1), (0, 1), (0, 0), (-1, 0)]),
        (0x20, [(-1, 0), (0, 0), (0, -1), (-1, -1)]),
        (0x40, [(0, 0), (1, 0), (1, -1), (0, -1)]),
    ];
    for (marker, offsets) in cases {
        let mut city = City::new_blank(
            format!("/tmp/sc2000-mac-rci-edge-marker-{marker:02x}.sc2"),
            "MacEdgeMarker",
        )
        .expect("blank city");
        city.set_zone(50, 50, marker | 2);
        let mut rng = MacToolboxRng::new(17);
        let mut expected_rng = MacToolboxRng::new(17);
        for _ in 0..4 {
            expected_rng.random_i16();
        }

        super::super::budget::mac_budget_place_generic_rotated_edge(&mut city, 50, 50, &mut rng);

        let expected = offsets.map(|(dx, dy)| ((50isize + dx) as usize, (50isize + dy) as usize));
        for x in 49..=51 {
            for y in 49..=51 {
                assert_eq!(
                    city.building_at(x, y) != 0,
                    expected.contains(&(x, y)),
                    "marker {marker:#04x} at ({x},{y})",
                );
            }
        }
        assert_eq!(rng.seed(), expected_rng.seed());
    }
}

#[test]
fn mac_scan_acceptance_church_does_not_require_power_or_a_successful_route() {
    let mut city = blank_city_layout_fixture("mac-scan-unpowered-church.city", "unpowered church");
    city.set_zone(40, 40, 0x81);
    city.set_building(40, 40, 0xa6);
    city.set_tile_bits(40, 40, TileBits::new(0));
    let before = u32::from(city.mac_xbld_tile_count(0xf7));
    city.set_mac_state_misc_u32_at(0x102c, before * 2500 + 1);
    let seed = (1..100_000)
        .find(|seed| MacToolboxRng::new(*seed).random_u16() < 8192)
        .unwrap();
    let mut expected = MacToolboxRng::new(seed);
    expected.random_i16();
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);
    for x in 40..=41 {
        for y in 39..=40 {
            assert_eq!(city.building_at(x, y), 0xf7, "church at {x},{y}");
            assert_eq!(city.zone_at(x, y).unwrap().zone_id(), 0);
            assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits() & 0xe0, 0xe0);
        }
    }
    assert_eq!(u32::from(city.mac_xbld_tile_count(0xf7)), before + 4);
    assert_eq!(
        rng.seed(),
        expected.seed(),
        "the church helper draws no Random value"
    );
}

#[test]
fn mac_scan_acceptance_church_gate_is_captured_once_per_partition() {
    let mut city =
        blank_city_layout_fixture("mac-scan-church-snapshot.city", "church gate snapshot");
    for x in [40, 44] {
        city.set_zone(x, 40, 0x81);
        city.set_building(x, 40, 0xa6);
        city.set_tile_bits(x, 40, TileBits::new(0x40));
    }
    let before = u32::from(city.mac_xbld_tile_count(0xf7));
    city.set_mac_state_misc_u32_at(0x102c, before * 2500 + 10000);
    let seed = (1..100_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            rng.random_u16() < 8192 && rng.random_u16() < 8192
        })
        .unwrap();
    let mut expected = MacToolboxRng::new(seed);
    expected.random_i16();
    expected.random_i16();
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);
    for anchor in [40, 44] {
        for x in anchor..=anchor + 1 {
            for y in 39..=40 {
                assert_eq!(
                    city.building_at(x, y),
                    0xf7,
                    "captured gate permits the second church"
                );
            }
        }
    }
    assert_eq!(u32::from(city.mac_xbld_tile_count(0xf7)), before + 8);
    assert_eq!(rng.seed(), expected.seed());
}

#[test]
fn mac_rci_generic_growth_runs_after_class_zero_for_shared_candidate() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-rci-generic-after-class-zero.sc2",
        "MacGenericAfterClassZero",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, 0x92); // category 2 and active rotation-0 corner
    city.set_building(40, 40, 0x8c); // coefficient 2, class-table byte 0
    stamp_xval_for_test(&mut city, 0x60); // generic coefficient-2 XVAL gate
    let seed = (1..10_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            (rng.random_i16() as u16) >= 2000
        })
        .expect("find a seed rejecting class-0 threshold");
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_i16(); // class-0 threshold draw
    expected_rng.random_i16(); // independent generic threshold draw

    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.building_at(40, 40), 0x8c);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_rci_class_zero_route_threshold_uses_saved_signed_channel_demand() {
    let mut base = City::new_blank(
        "/tmp/sc2000-mac-rci-class-zero-demand.sc2",
        "MacClassZeroDemand",
    )
    .expect("blank city");
    base.kind = CityFileKind::MacCity;
    base.set_zone(40, 40, 0x81);
    base.set_building(40, 40, 0x70);
    base.set_tile_bits(40, 40, TileBits::new(0xc0)); // route marker and power
    base.set_building(40, 41, 0x20);
    base.set_zone(40, 42, 3); // adjacent destination for the route search

    let route_and_threshold_roll = |seed| {
        let mut probe = MacToolboxRng::new(seed);
        let route = mac_transit_route_search(
            40,
            40,
            1,
            mac_growth_coefficient_for_xbld(0x70),
            100,
            |x, y| MacTransitRouteCell {
                building: base.building_at(x, y),
                zone_low: base.zone_at(x, y).map_or(0, |zone| zone.raw_bits() & 0x0f),
                tunnel_levels: base.tunnel_levels_at(x, y),
                underground: base.underground_at(x, y),
                text_overlay: base.text_overlay_at(x, y).unwrap_or(0),
            },
            || probe.random_i16(),
        );
        route.map(|_| probe.random_i16() as u16)
    };
    let seed = (1..100_000)
        .find(|seed| {
            route_and_threshold_roll(*seed).is_some_and(|roll| (1000..3000).contains(&roll))
        })
        .expect("find a routed class-0 candidate between the two thresholds");

    let mut low_demand = base.clone();
    low_demand.set_mac_state_misc_u32_at(0x718, (-1000_i32) as u32);
    let mut high_demand = base;
    high_demand.set_mac_state_misc_u32_at(0x718, 1000);
    let mut low_rng = MacToolboxRng::new(seed);
    let mut high_rng = MacToolboxRng::new(seed);

    mac_budget_accumulate_monthly_rci_scan_partition(&mut low_demand, 0, 0, &mut low_rng);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut high_demand, 0, 0, &mut high_rng);

    assert_ne!(low_demand.building_at(40, 40), 0x70);
    assert_eq!(high_demand.building_at(40, 40), 0x70);
    assert_ne!(low_rng.seed(), high_rng.seed());
    assert_eq!(low_demand.mac_state_misc_u32_at(0x05f0 + 4), Some(1));
    assert_eq!(high_demand.mac_state_misc_u32_at(0x05f0 + 4), Some(1));
}

#[test]
fn mac_rci_runway_direction_uses_raw_parity_and_dd_census_bit_eight() {
    let mut city = City::new_blank("/tmp/sc2000-mac-rci-runway-direction.sc2", "RunwayDir")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;

    assert_eq!(mac_budget_zone_dd_positive_ray(&city, 40, 40), None);
    assert_eq!(mac_budget_zone_dd_positive_ray(&city, 40, 41), Some((1, 0)));
    assert_eq!(mac_budget_zone_dd_positive_ray(&city, 41, 40), Some((0, 1)));
    assert_eq!(mac_budget_zone_dd_positive_ray(&city, 41, 41), Some((0, 1)));

    // Mac PlaceTile keeps the saved WORD census. Its high-byte bit zero is
    // the exact byte/bit read by FUN_00037ab0 at fffff07a+0x1bb.
    for index in 0..256 {
        city.set_building(index % CITY_WIDTH, index / CITY_WIDTH, 0xdd);
    }
    assert_eq!(city.mac_xbld_tile_count(0xdd), 256);
    assert_eq!(mac_budget_zone_dd_positive_ray(&city, 41, 41), Some((1, 0)));
}

#[test]
fn mac_rci_zone_eight_and_nine_special_type_selectors_follow_word_comparisons() {
    let mut counts = [0u16; 256];
    let select = |zone, counts: &[u16; 256]| {
        mac_budget_zone_special_growth_xbld(zone, |tile| counts[usize::from(tile)])
    };
    assert_eq!(select(8, &counts), Some(0xdd));
    assert_eq!(select(9, &counts), Some(0xe0));
    assert_eq!(select(7, &counts), None);

    // Zone 9 keeps the binary's asymmetric ordered comparisons: f2's
    // quartered count can select e0; otherwise a low f0 count selects f0.
    counts[0xe0] = 8;
    counts[0xf2] = 32;
    assert_eq!(select(9, &counts), Some(0xe0));
    counts[0xf2] = 28; // f2 / 4 = 7, below e0 = 8.
    assert_eq!(select(9, &counts), Some(0xf0));
    // When f0 is sufficiently large, e3's one-third threshold decides before
    // the final f2 fallback.
    counts[0xf0] = 32;
    counts[0xe3] = 30;
    assert_eq!(select(9, &counts), Some(0xe3));
    counts[0xe3] = 21;
    assert_eq!(select(9, &counts), Some(0xf2));

    // Zone 8 chooses dd when ee/4 is greater than or equal to the threshold.
    // Equality is native's default-to-runway path.
    counts = [0; 256];
    counts[0xdd] = 25;
    counts[0xde] = 25; // threshold = 10
    counts[0xee] = 40; // ee / 4 = threshold.
    counts[0xe1] = 4; // 2*e1 = 8 < 10
    counts[0xea] = 1;
    assert_eq!(select(8, &counts), Some(0xdd));

    // Once ee/4 drops below the threshold, the ordered alternatives apply.
    counts[0xee] = 36; // ee / 4 = 9 < threshold.
    assert_eq!(select(8, &counts), Some(0xe1));

    // The 68k ADD.W wraps before division, and a selector threshold is WORD
    // sized throughout this helper.
    counts[0xdd] = u16::MAX;
    counts[0xde] = 1;
    counts[0xee] = 0;
    assert_eq!(select(8, &counts), Some(0xdd));
}

#[test]
fn mac_rci_zone_seven_special_type_selectors_follow_saved_class_words() {
    let mut counts = [0u16; 16];
    let select = |event, counts: &[u16; 16]| {
        mac_budget_zone_seven_special_growth_xbld(event, |class| counts[class])
    };
    assert_eq!(select(4, &counts), Some(0xe0));
    assert_eq!(select(2, &counts), Some(0xef));
    assert_eq!(select(3, &counts), Some(0xdd));
    assert_eq!(select(6, &counts), None);

    counts[10] = 8;
    assert_eq!(select(4, &counts), Some(0xf1));
    counts[9] = 32;
    assert_eq!(select(4, &counts), Some(0xe3));
    counts[6] = 24;
    assert_eq!(select(4, &counts), Some(0xf2));
    counts[4] = 32;
    assert_eq!(select(4, &counts), Some(0xe0));

    counts = [0; 16];
    counts[3] = 4;
    assert_eq!(select(2, &counts), Some(0xe8));
    counts[13] = 48;
    assert_eq!(select(2, &counts), Some(0xef));

    counts = [0; 16];
    counts[1] = 10;
    counts[2] = 10;
    counts[3] = 8;
    counts[11] = 1;
    assert_eq!(select(3, &counts), Some(0xe2));
    counts[11] = 2;
    counts[5] = 1;
    assert_eq!(select(3, &counts), Some(0xea));
    counts[5] = 2;
    counts[12] = 3;
    assert_eq!(select(3, &counts), Some(0xe7));
    counts[12] = 4;
    counts[7] = 6;
    assert_eq!(select(3, &counts), Some(0xe4));
    counts[7] = 8;
    counts[8] = 6;
    assert_eq!(select(3, &counts), Some(0xe5));
    counts[8] = 8;
    counts[14] = 12;
    assert_eq!(select(3, &counts), Some(0xf6));
    counts[14] = 16;
    assert_eq!(select(3, &counts), Some(0xef));
    counts[3] = 16;
    assert_eq!(select(3, &counts), Some(0xdd));

    // The 68k adds the threshold inputs as a WORD before dividing.
    counts[1] = u16::MAX;
    counts[2] = 1;
    counts[3] = 0;
    assert_eq!(select(3, &counts), Some(0xdd));
}

#[test]
fn mac_zone_seven_scan_branch_uses_saved_census_and_typed_fallback() {
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-seven-scan.sc2", "ZoneSevenScan")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y) = (40, 40);
    city.set_zone(x, y, 7);
    city.set_mac_state_misc_u32_at(915 * 4, 2); // persisted event selector
    city.set_mac_state_misc_u32_at(0x0fa8 + 3 * 4, 0); // ef selector comparator
    city.set_mac_state_misc_u32_at(0x0fa8 + 13 * 4, 120); // ef / 12 >= 0

    let seed = (1..100_000)
        .find(|&seed| MacToolboxRng::new(seed).random_u16() & 3 == 0)
        .expect("accepted zone-seven selector seed");
    let mut rng = MacToolboxRng::new(seed);
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_i16();
    assert_eq!(
        mac_budget_process_zone_seven_special_growth(&mut city, x, y, &mut rng),
        MacSpecialTileAttempt::Placed
    );
    assert_eq!(rng.seed(), expected_rng.seed());
    // The selected ef footprint is refused because the other three cells are
    // outside zone 7; the caller retries its typed one-cell e8 placement.
    assert_eq!(city.building_at(x, y), 0xe8);
    assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0xf7);
    assert_eq!(city.mac_zone7_census_count(13), 121);
    assert_eq!(city.mac_zone7_census_count(0), u16::MAX);
}

#[test]
fn mac_scan_zone_seven_square_census_changes_later_selector() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-zone-seven-square-census-interleave.sc2",
        "MacScanZoneSevenSquareCensus",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(915 * 4, 2); // persisted event selector
    // With both selector inputs initially zero, event 2 selects the ef square.
    city.set_mac_state_misc_u32_at(0x0fa8 + 3 * 4, 0);
    city.set_mac_state_misc_u32_at(0x0fa8 + 13 * 4, 0);
    for &(left, top) in &[(42usize, 42usize), (46, 42)] {
        for dy in 0..2 {
            for dx in 0..2 {
                city.set_zone(left + dx, top + dy, 7);
            }
        }
    }

    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            probe.random_i16() as u16 & 3 == 0 && probe.random_i16() as u16 & 3 == 0
        })
        .expect("find two accepted event-2 selector gates");
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_i16();
    expected_rng.random_i16();

    let mut rng = MacToolboxRng::new(seed);
    for phase in 0..=10 {
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, phase / 4, phase % 4, &mut rng);
    }

    for y in 42..=43 {
        for x in 42..=43 {
            assert_eq!(city.building_at(x, y), 0xef, "first square ({x},{y})");
        }
    }
    // The first square raises class 3 to four. At the later same-phase
    // candidate, class 3 >> 2 is now one while class 13 / 12 is zero, so the
    // selector changes from another ef square to a one-cell e8.
    assert_eq!(city.building_at(46, 42), 0xe8);
    assert_eq!(city.zone_at(46, 42).unwrap().raw_bits(), 0xf7);
    assert_eq!(city.building_at(47, 42), 0);
    assert_eq!(city.building_at(46, 43), 0);
    assert_eq!(city.mac_zone7_census_count(3), 4);
    assert_eq!(city.mac_zone7_census_count(13), 1);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_scan_zone_seven_event_three_census_changes_later_selector() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-zone-seven-event-three-census-interleave.sc2",
        "MacScanZoneSevenEventThreeCensus",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(915 * 4, 3); // persisted event selector
    // Event 3's threshold is (class 1 + class 2) / 5 = 1. With class 3 and
    // class 11 initially zero, its first accepted candidate selects XBLD e2.
    city.set_mac_state_misc_u32_at(0x0fa8 + 1 * 4, 3);
    city.set_mac_state_misc_u32_at(0x0fa8 + 2 * 4, 2);
    for (x, y) in [(42usize, 42usize), (46, 42)] {
        city.set_zone(x, y, 7);
    }

    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            probe.random_i16() as u16 & 3 == 0 && probe.random_i16() as u16 & 3 == 0
        })
        .expect("find two accepted event-3 selector gates");
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_i16();
    expected_rng.random_i16();

    let mut rng = MacToolboxRng::new(seed);
    for phase in 0..=11 {
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, phase / 4, phase % 4, &mut rng);
    }

    assert_eq!(city.building_at(42, 42), 0xe2);
    // The first e2 increments event-3's class 11 from zero to one. With the
    // threshold still one, the e2 predicate fails and the next selector is ea.
    assert_eq!(city.building_at(46, 42), 0xea);
    assert_eq!(city.zone_at(42, 42).unwrap().raw_bits(), 0xf7);
    assert_eq!(city.zone_at(46, 42).unwrap().raw_bits(), 0xf7);
    assert_eq!(city.mac_zone7_census_count(11), 1);
    assert_eq!(city.mac_zone7_census_count(5), 1);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_zone_nine_type_three_creator_uses_transient_counter_not_slot_census() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-zone-nine-type-three-counter.sc2",
        "ZoneNineCounter",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, 9);
    city.set_building(40, 40, 0xe0);
    city.set_tile_bits(40, 40, TileBits::new(0x40));
    city.set_terrain(2, 127, 0x10);
    city.mac_object_slot_records_mut()[1].kind = 3;

    // The loaded actor table has a kind-3 slot, but native ffffa096 remains 0
    // until the sweep visits it, so export 116 is still admitted once.
    let seed = (1..1_000_000)
        .find(|&seed| {
            let mut rng = MacToolboxRng::new(seed);
            let first = rng.random_i16() as u16;
            let second = rng.random_i16() as u16;
            first & 3 != 0 && second & 3 == 0 && (rng.random_i16() as u16 & 3) == 0
        })
        .expect("find alternate-arm seed selecting X row 2");
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    assert_eq!(city.mac_kind3_actor_counter, 1);
    assert_eq!(city.traffic_overlays().unwrap()[1].kind, 3);
    assert_eq!(city.traffic_overlays().unwrap()[2].kind, 3);
    assert_eq!(city.text_overlay_at(2, 127), Some(0xcb));
}

#[test]
fn mac_rci_class_one_growth_places_the_recovered_selector_square() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-rci-class-one-square.sc2",
        "MacClassOneSquare",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, 0x84); // category 4 and rotation-0 scan corner
    city.set_building(40, 40, 0xa6); // class 1, coefficient 2
    let seed = (1..10_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            (rng.random_i16() as u16) < 0x2000
        })
        .expect("find a class-1 acceptance seed");
    let mut expected_rng = MacToolboxRng::new(seed);
    let _acceptance = expected_rng.random_i16();
    let tile_draw = expected_rng.random_i16() as u16;
    let expected_tile = 0x94 + (tile_draw % 5) as u8;

    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    // Coefficient 2 stamps a 2x2 square with the scan anchor at the lower
    // right of the footprint, per CODE 8 export 242.
    for x in 40..=41 {
        for y in 39..=40 {
            assert_eq!(city.building_at(x, y), expected_tile);
            let expected_zone = if (x, y) == (40, 40) { 4 } else { 0 };
            assert_eq!(city.zone_at(x, y).unwrap().zone_id(), expected_zone);
            assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits() & 0xe0, 0xe0);
        }
    }
    assert_eq!(city.zone_at(40, 39).unwrap().raw_bits() & 0xf0, 0x10);
    assert_eq!(city.zone_at(41, 39).unwrap().raw_bits() & 0xf0, 0x20);
    assert_eq!(city.zone_at(41, 40).unwrap().raw_bits() & 0xf0, 0x40);
    assert_eq!(city.zone_at(40, 40).unwrap().raw_bits() & 0xf0, 0x80);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_rci_class_one_growth_uses_powered_church_alternative() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-rci-class-one-church.sc2",
        "MacClassOneChurch",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, 0x81); // category 1 and rotation-0 scan corner
    city.set_building(40, 40, 0xa6); // class 1, coefficient 2
    city.set_tile_bits(40, 40, TileBits::new(0x40));
    city.set_mac_state_misc_u32_at(0x102c, 10_000);
    let seed = (1..10_000)
        .find(|seed| {
            let mut rng = MacToolboxRng::new(*seed);
            (rng.random_i16() as u16) < 0x2000
        })
        .expect("find a class-1 acceptance seed");
    let mut expected_rng = MacToolboxRng::new(seed);
    let _acceptance = expected_rng.random_i16();

    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    for x in 40..=41 {
        for y in 39..=40 {
            assert_eq!(city.building_at(x, y), 0xf7);
            assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits() & 0xe0, 0xe0);
        }
    }
    assert_eq!(
        rng.seed(),
        expected_rng.seed(),
        "church helper draws no RNG"
    );
}

#[test]
fn mac_phase1_power_output_uses_native_growth_adjacency_at_low_edge() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-route-power-phase1-edge.sc2",
        "MacRoutePowerPhase1Edge",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_building(0, 32, 0xc6);
    city.set_tile_bits(0, 32, TileBits::new(0x80));
    city.set_building(1, 32, 0x70);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day1_power();

    assert!(state.city.tile_bits_at(0, 32).unwrap().powered());
    assert!(!state.city.tile_bits_at(1, 32).unwrap().powered());
    assert!(
        !mac_transit_has_powered_adjacency(&state.city, 1, 32),
        "phase 1's powered XBIT row 0 must not satisfy phase 3-18's native x>1 probe"
    );
    let growth_power = power_service_maps(&state.city);
    assert!(
        !power_water::growth_powered_cross_for_engine(&growth_power, 1, 32),
        "Mac growth must read phase 1's saved XBIT with the same strict x>1 probe"
    );
}

#[test]
fn saved_xbit_growth_power_cross_uses_origin_and_cardinals_not_diagonal() {
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let index = |x: usize, y: usize| y * CITY_WIDTH + x;

    set_saved_xbit_growth_power_for_test(&mut power_maps, 20, 20, true);
    assert!(growth_powered_cross_for_engine(&power_maps, 20, 20));

    // Both editions read published XBIT `0x40`; geometry energization by itself
    // is not the saved bit tested by either native helper.
    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    power_maps.energized_tiles[index(20, 19)] = true;
    assert!(
        !growth_powered_cross_for_engine(&power_maps, 20, 20),
        "geometry energization alone must not satisfy the saved-XBIT cross"
    );
    power_maps.energized_tiles.fill(false);

    for (x, y) in [(19, 20), (21, 20), (20, 19), (20, 21)] {
        set_saved_xbit_growth_power_for_test(&mut power_maps, x, y, true);
        assert!(growth_powered_cross_for_engine(&power_maps, 20, 20));
        set_saved_xbit_growth_power_for_test(&mut power_maps, x, y, false);
    }

    set_saved_xbit_growth_power_for_test(&mut power_maps, 19, 19, true);
    assert!(!growth_powered_cross_for_engine(&power_maps, 20, 20));

    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 20, 22, true);
    assert!(
        growth_powered_cross_for_engine(&power_maps, 20, 22),
        "a 3x3 view corner uses its visited tile"
    );
    assert!(
        !growth_powered_cross_for_engine(&power_maps, 20, 20),
        "the same power must not be attributed to the 3x3 NW origin"
    );

    // PE west/north probes require coord > 1 (FUN_00436D00), so a powered tile
    // at x=0 or y=0 does not feed the cross at x=1 / y=1.
    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 0, 1, true);
    assert!(
        !growth_powered_cross_for_engine(&power_maps, 1, 1),
        "PE skips west probe when x <= 1"
    );
    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 1, 0, true);
    assert!(
        !growth_powered_cross_for_engine(&power_maps, 1, 1),
        "PE skips north probe when y <= 1"
    );
    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 1, 2, true);
    assert!(
        growth_powered_cross_for_engine(&power_maps, 2, 2),
        "west probe when x > 1 still sees the west neighbor"
    );
    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 0, 2, true);
    assert!(
        !growth_powered_cross_for_engine(&power_maps, 1, 2),
        "powered tile at x=0 is invisible from x == 1 (no west probe)"
    );
}

#[test]
fn pe_raise_land_inc_443c60_clears_flatten_mark_before_funds() {
    // PE `0x44425d`: bit 8 cleared before `[0x4dc148]` compare, even if funds fail.
    let mut city = City::new_blank("/tmp/sc2000-raise-mark.sc2", "RaiseMark").expect("blank");
    city.set_altitude(20, 20, 5, 0);
    city.set_tile_bits(20, 20, TileBits::new(0).with_flatten_mark(true));
    assert_eq!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2), Some(0));
    assert_eq!(city.altitude_at(20, 20), 5);
    assert!(!city.tile_bits_at(20, 20).unwrap().flatten_mark());
    city.set_tile_bits(20, 20, TileBits::new(0).with_flatten_mark(true));
    city.set_city_funds(0x19);
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2).is_some());
    assert!(!city.tile_bits_at(20, 20).unwrap().flatten_mark());
}

#[test]
fn pe_lower_444440_mode_zero_skips_funds_path() {
    // PE `0x444480`: mode 0 skips funds; Raise adjust dig uses mode 1.
    assert!(pe_lower_terrain_444440_mode_skips_funds(0));
    assert!(!pe_lower_terrain_444440_mode_skips_funds(1));
    assert_eq!(pe_lower_444440_mode_cost(0), 1);
    assert_eq!(pe_lower_444440_mode_cost(1), 0x19);
    assert_eq!(pe_lower_444440_mode_cost(4), 1);
    assert_eq!(pe_lower_444440_mode_cost(5), -1);
    assert_eq!(pe_lower_444440_mode_cost(99), -1);
    let mut city = City::new_blank("/tmp/sc2000-dig-mode1.sc2", "DigMode1").expect("blank");
    city.set_city_funds(100);
    city.set_altitude(20, 20, 4, 0);
    assert!(pe_lower_terrain_land_dec_444440_mode_with_submode(
        &mut city, 20, 20, 1, 1
    ));
    assert_eq!(city.altitude_at(20, 20), 7);
    assert_eq!(city.city_funds(), 0);
    city.set_city_funds(10);
    assert!(!pe_lower_terrain_land_dec_444440_mode_with_submode(
        &mut city, 20, 20, 1, 1
    ));
    assert_eq!(city.altitude_at(20, 20), 7);
    assert_eq!(city.city_funds(), 10);
    // Unknown Raise submode → cost -1 → mode-1 dig refuses without debit.
    city.set_city_funds(100);
    assert!(!pe_lower_terrain_land_dec_444440_mode_with_submode(
        &mut city, 20, 20, 1, 5
    ));
    assert_eq!(city.altitude_at(20, 20), 7);
    assert_eq!(city.city_funds(), 100);
}

#[test]
fn pe_rci_demand_updates_nature_sample_skips_spread_onto_radioactivity() {
    // Tree band skips 1-in-16 (`0x46869d`); third CRT roll is NESW dir.
    let mut hit = None;
    for seed in 1..50_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        let dir = crt.random_u16() & 3;
        let neighbor = match dir {
            0 if x < 0x7f => Some((x + 1, y)),
            1 if x > 0 => Some((x - 1, y)),
            2 if y < 0x7f => Some((x, y + 1)),
            3 if y > 0 => Some((x, y - 1)),
            _ => None,
        };
        let Some((nx, ny)) = neighbor else {
            continue;
        };
        hit = Some((seed, x, y, nx, ny));
        break;
    }
    let (seed, x, y, nx, ny) = hit.expect("seed with interior NESW spread");
    let mut city = City::new_blank("/tmp/sc2000-rci-nrad.sc2", "RciNRad").expect("blank");
    city.set_building(x, y, 6);
    city.set_building(nx, ny, 5);
    let mut crt = Win95CrtRand::new(seed);
    assert!(pe_rci_demand_updates_nature_sample(&mut city, &mut crt).is_some());
    assert_eq!(city.building_at(x, y), 7);
    assert_eq!(city.building_at(nx, ny), 5, "PE skips spread onto XBLD 5");
}

#[test]
fn pe_rci_demand_updates_nature_sample_skips_spread_onto_dense_forest() {
    let mut hit = None;
    for seed in 1..50_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        let dir = crt.random_u16() & 3;
        let neighbor = match dir {
            0 if x < 0x7f => Some((x + 1, y)),
            1 if x > 0 => Some((x - 1, y)),
            2 if y < 0x7f => Some((x, y + 1)),
            3 if y > 0 => Some((x, y - 1)),
            _ => None,
        };
        let Some((nx, ny)) = neighbor else {
            continue;
        };
        hit = Some((seed, x, y, nx, ny));
        break;
    }
    let (seed, x, y, nx, ny) = hit.expect("seed with interior NESW spread");
    let mut city = City::new_blank("/tmp/sc2000-rci-ndense.sc2", "RciNDen").expect("blank");
    city.set_building(x, y, 6);
    city.set_building(nx, ny, 0x0c);
    let mut crt = Win95CrtRand::new(seed);
    assert!(pe_rci_demand_updates_nature_sample(&mut city, &mut crt).is_some());
    assert_eq!(city.building_at(nx, ny), 0x0c, "PE skips neighbor >= 0x0C");
}

#[test]
fn pe_rci_demand_updates_nature_sample_ages_neighbor_tree() {
    // PE @ 0x46873f: neighbor XBLD >= 6 and < 0x0C → PlaceTile(n+1).
    let mut hit = None;
    for seed in 1..50_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        let dir = crt.random_u16() & 3;
        let neighbor = match dir {
            0 if x < 0x7f => Some((x + 1, y)),
            1 if x > 0 => Some((x - 1, y)),
            2 if y < 0x7f => Some((x, y + 1)),
            3 if y > 0 => Some((x, y - 1)),
            _ => None,
        };
        let Some((nx, ny)) = neighbor else {
            continue;
        };
        hit = Some((seed, x, y, nx, ny));
        break;
    }
    let (seed, x, y, nx, ny) = hit.expect("seed with interior NESW spread");
    let mut city = City::new_blank("/tmp/sc2000-rci-nage.sc2", "RciNAge").expect("blank");
    city.set_building(x, y, 6);
    city.set_building(nx, ny, 8);
    city.set_text_overlay(nx, ny, 0x4b);
    let mut crt = Win95CrtRand::new(seed);
    assert!(pe_rci_demand_updates_nature_sample(&mut city, &mut crt).is_some());
    assert_eq!(city.building_at(x, y), 7);
    assert_eq!(city.building_at(nx, ny), 9);
    assert_eq!(city.text_overlay_at(nx, ny), Some(0x4b));
}

#[test]
fn pe_rci_demand_updates_nature_sample_small_park_spreads_without_aging() {
    // PE: XBLD 0x0D is in the no-gate band; skips source age (`>=0x0C`); NESW spreads.
    let mut hit = None;
    for seed in 1..50_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        let dir = crt.random_u16() & 3;
        let neighbor = match dir {
            0 if x < 0x7f => Some((x + 1, y)),
            1 if x > 0 => Some((x - 1, y)),
            2 if y < 0x7f => Some((x, y + 1)),
            3 if y > 0 => Some((x, y - 1)),
            _ => None,
        };
        let Some((nx, ny)) = neighbor else {
            continue;
        };
        hit = Some((seed, x, y, nx, ny));
        break;
    }
    let (seed, x, y, nx, ny) = hit.expect("seed with interior NESW spread");
    let mut city = City::new_blank("/tmp/sc2000-rci-park.sc2", "RciPark").expect("blank");
    city.set_building(x, y, SMALL_PARK_BUILDING_ID);
    let mut crt = Win95CrtRand::new(seed);
    assert_eq!(
        pe_rci_demand_updates_nature_sample(&mut city, &mut crt),
        Some((nx, ny, 6))
    );
    assert_eq!(city.building_at(x, y), SMALL_PARK_BUILDING_ID);
    assert_eq!(city.building_at(nx, ny), 6);
}

#[test]
fn pe_rci_demand_updates_nature_sample_dense_forest_spreads_without_aging() {
    // PE @ `0x4686bc`: XBLD 0x0C skips age (`jnl`) but still NESW-spreads (no 1-in-16).
    let mut hit = None;
    for seed in 1..50_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        let dir = crt.random_u16() & 3;
        let neighbor = match dir {
            0 if x < 0x7f => Some((x + 1, y)),
            1 if x > 0 => Some((x - 1, y)),
            2 if y < 0x7f => Some((x, y + 1)),
            3 if y > 0 => Some((x, y - 1)),
            _ => None,
        };
        let Some((nx, ny)) = neighbor else {
            continue;
        };
        hit = Some((seed, x, y, nx, ny));
        break;
    }
    let (seed, x, y, nx, ny) = hit.expect("seed with interior NESW spread");
    let mut city = City::new_blank("/tmp/sc2000-rci-df.sc2", "RciDF").expect("blank");
    city.set_building(x, y, 0x0c);
    let mut crt = Win95CrtRand::new(seed);
    assert_eq!(
        pe_rci_demand_updates_nature_sample(&mut city, &mut crt),
        Some((nx, ny, 6))
    );
    assert_eq!(city.building_at(x, y), 0x0c, "0x0C does not age to 0x0D");
    assert_eq!(city.building_at(nx, ny), 6);
}

#[test]
fn pe_rci_demand_updates_nature_sample_vacant_spreads_on_one_in_sixteen() {
    // PE @ `0x4686a7`: XBLD outside `6..=0x0D` needs rand&0xf==0, then NESW plant.
    let mut hit = None;
    for seed in 1..50_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        if crt.random_u16() & 0x0f != 0 {
            continue;
        }
        let dir = crt.random_u16() & 3;
        let neighbor = match dir {
            0 if x < 0x7f => Some((x + 1, y)),
            1 if x > 0 => Some((x - 1, y)),
            2 if y < 0x7f => Some((x, y + 1)),
            3 if y > 0 => Some((x, y - 1)),
            _ => None,
        };
        let Some((nx, ny)) = neighbor else {
            continue;
        };
        hit = Some((seed, x, y, nx, ny));
        break;
    }
    let (seed, x, y, nx, ny) = hit.expect("seed with vacant 1-in-16 NESW");
    let mut city = City::new_blank("/tmp/sc2000-rci-vac.sc2", "RciVac").expect("blank");
    // Blank city is vacant at (x,y); neighbor stays vacant for PlaceTile(6).
    let mut crt = Win95CrtRand::new(seed);
    assert_eq!(
        pe_rci_demand_updates_nature_sample(&mut city, &mut crt),
        Some((nx, ny, 6))
    );
    assert_eq!(city.building_at(x, y), 0);
    assert_eq!(city.building_at(nx, ny), 6);
}

#[test]
fn pe_rci_demand_updates_nature_sample_spreads_tree_to_neighbor() {
    // PE @ 0x4686d0: after tree band (no 1-in-16), rand&3 NESW; vacant → PlaceTile(6).
    let mut hit = None;
    for seed in 1..50_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        let dir = crt.random_u16() & 3;
        let neighbor = match dir {
            0 if x < 0x7f => Some((x + 1, y)),
            1 if x > 0 => Some((x - 1, y)),
            2 if y < 0x7f => Some((x, y + 1)),
            3 if y > 0 => Some((x, y - 1)),
            _ => None,
        };
        let Some((nx, ny)) = neighbor else {
            continue;
        };
        hit = Some((seed, x, y, nx, ny));
        break;
    }
    let (seed, x, y, nx, ny) = hit.expect("seed with interior NESW spread");
    let mut city = City::new_blank("/tmp/sc2000-rci-spread.sc2", "RciSpr").expect("blank");
    city.set_building(x, y, 6);
    city.set_text_overlay(nx, ny, 0x4b);
    let mut crt = Win95CrtRand::new(seed);
    assert!(pe_rci_demand_updates_nature_sample(&mut city, &mut crt).is_some());
    assert_eq!(city.building_at(x, y), 7, "source ages");
    assert_eq!(city.building_at(nx, ny), 6, "neighbor plants");
    assert_eq!(city.text_overlay_at(nx, ny), Some(0x4b));
}

#[test]
fn pe_nature_sample_revisits_map_edge_after_outward_direction() {
    // The original PE jump table selects +x, -x, +y, -y for 0..3; at the
    // map edge its coordinate branch falls through to the neighbor re-read.
    for (seed, x, y, dir) in [
        (968, 127, 16, 0),
        (694, 0, 105, 1),
        (118, 39, 127, 2),
        (474, 50, 0, 3),
    ] {
        let mut expected_rng = Win95CrtRand::new(seed);
        assert_eq!(usize::from(expected_rng.random_u16() & 127), x);
        assert_eq!(usize::from(expected_rng.random_u16() & 127), y);
        assert_eq!(expected_rng.random_u16() & 3, dir);
        for initial in [6, 11] {
            let mut city = City::new_blank("/tmp/pe-edge-nature.sc2", "edge").unwrap();
            city.set_building(x, y, initial);
            city.set_traffic_density(x, y, 0x37);
            let mut actual_rng = Win95CrtRand::new(seed);
            pe_rci_demand_updates_nature_sample(&mut city, &mut actual_rng);
            assert_eq!(
                city.building_at(x, y),
                if initial == 6 { 8 } else { 12 },
                "direction {dir}"
            );
            assert_eq!(city.traffic_density_at(x, y), Some(0x37));
            assert_eq!(actual_rng.seed(), expected_rng.seed(), "no extra draws");
        }
    }
}

#[test]
fn pe_rci_demand_updates_nature_sample_ages_0x0b_to_0x0c() {
    // PE @ `0x4686b4`/`0x4686bc`: age while `6 <= XBLD < 0x0C` → PlaceTile(XBLD+1).
    let mut hit = None;
    for seed in 1..20_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        hit = Some((seed, x, y));
        break;
    }
    let (seed, x, y) = hit.expect("any seed picks a tile");
    let mut city = City::new_blank("/tmp/sc2000-rci-0b.sc2", "Rci0b").expect("blank");
    city.set_building(x, y, 0x0b);
    let mut crt = Win95CrtRand::new(seed);
    assert_eq!(
        pe_rci_demand_updates_nature_sample(&mut city, &mut crt),
        Some((x, y, 0x0c))
    );
    assert_eq!(city.building_at(x, y), 0x0c);
}

#[test]
fn pe_rci_demand_updates_nature_sample_ages_tree_xbld() {
    // PE @ 0x4686b4: XBLD 6..=0x0B → PlaceTile(XBLD+1); no 1-in-16 in tree band.
    let mut hit = None;
    for seed in 1..20_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        hit = Some((seed, x, y));
        break;
    }
    let (seed, x, y) = hit.expect("any seed picks a tile");
    let mut city = City::new_blank("/tmp/sc2000-rci-tree.sc2", "RciTree").expect("blank");
    city.set_building(x, y, 6);
    city.set_text_overlay(x, y, 0x4b);
    city.set_traffic_density(x, y, 0x37);
    let mut crt = Win95CrtRand::new(seed);
    assert_eq!(
        pe_rci_demand_updates_nature_sample(&mut city, &mut crt),
        Some((x, y, 7))
    );
    assert_eq!(city.building_at(x, y), 7);
    assert_eq!(city.text_overlay_at(x, y), Some(0x4b));
    assert_eq!(city.traffic_density_at(x, y), Some(0x37));
}

#[test]
fn pe_rci_demand_updates_nature_sample_skips_tree_on_water_tile() {
    let mut hit = None;
    for seed in 1..20_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        hit = Some((seed, x, y));
        break;
    }
    let (seed, x, y) = hit.unwrap();
    let mut city = City::new_blank("/tmp/sc2000-rci-tree-wet.sc2", "RciTreeW").expect("blank");
    city.set_building(x, y, 6);
    city.set_tile_bits(x, y, TileBits::new(0).with_water_tile(true));
    let mut crt = Win95CrtRand::new(seed);
    assert!(pe_rci_demand_updates_nature_sample(&mut city, &mut crt).is_none());
    assert_eq!(city.building_at(x, y), 6);
}

#[test]
fn pe_rci_demand_updates_nature_sample_radio_clear_may_spread_neighbor() {
    // PE radio PlaceTile(0) falls through with ebx=5 → 1-in-16 then NESW plant.
    let mut hit = None;
    for seed in 1..100_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        if crt.random_u16() & 0x0f != 0 {
            continue; // radio clear gate
        }
        if crt.random_u16() & 0x0f != 0 {
            continue; // post-radio tree gate for ebx=5
        }
        let dir = crt.random_u16() & 3;
        let neighbor = match dir {
            0 if x < 0x7f => Some((x + 1, y)),
            1 if x > 0 => Some((x - 1, y)),
            2 if y < 0x7f => Some((x, y + 1)),
            3 if y > 0 => Some((x, y - 1)),
            _ => None,
        };
        let Some((nx, ny)) = neighbor else {
            continue;
        };
        hit = Some((seed, x, y, nx, ny));
        break;
    }
    let (seed, x, y, nx, ny) = hit.expect("seed with radio clear + spread");
    let mut city = City::new_blank("/tmp/sc2000-rci-radsp.sc2", "RciRadSp").expect("blank");
    city.set_building(x, y, 5);
    let mut crt = Win95CrtRand::new(seed);
    assert_eq!(
        pe_rci_demand_updates_nature_sample(&mut city, &mut crt),
        Some((x, y, 0)),
        "radio clear reported first"
    );
    assert_eq!(city.building_at(x, y), 0);
    assert_eq!(city.building_at(nx, ny), 6, "fallthrough NESW plant");
}

#[test]
fn mac68k_day21_skips_rci_nature_radioactivity_sample() {
    // PE nature sample is Win95 SimulationRCIDemandUpdates-only.
    let mut city = City::new_blank("/tmp/sc2000-mac-day21-rad.sc2", "MacD21Rad").expect("blank");
    city.set_building(40, 40, 5);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let _ = state.sim_calendar_day21_demand_and_graphs();
    assert_eq!(
        state.city.building_at(40, 40),
        5,
        "Mac68k must not run PE nature PlaceTile clear"
    );
}

#[test]
fn pe_rci_demand_updates_radioactivity_sample_place_tile_clears_xbld_5() {
    // PE SimulationRCIDemandUpdates @ 0x468669: XBLD 5 + rand&0xf==0 → PlaceTile(0).
    let mut hit = None;
    for seed in 1..20_000u32 {
        let mut crt = Win95CrtRand::new(seed);
        let x = usize::from(crt.random_u16() & 0x7f);
        let y = usize::from(crt.random_u16() & 0x7f);
        if crt.random_u16() & 0x0f != 0 {
            continue;
        }
        hit = Some((seed, x, y));
        break;
    }
    let (seed, x, y) = hit.expect("some seed should pass 1-in-16");
    let mut city = City::new_blank("/tmp/sc2000-rci-rad.sc2", "RciRad").expect("blank");
    city.set_building(x, y, 5);
    city.set_text_overlay(x, y, 0x4b);
    city.set_traffic_density(x, y, 0x37);
    let mut crt = Win95CrtRand::new(seed);
    assert_eq!(
        pe_rci_demand_updates_radioactivity_sample(&mut city, &mut crt),
        Some((x, y))
    );
    assert_eq!(city.building_at(x, y), 0);
    assert_eq!(city.text_overlay_at(x, y), Some(0x4b));
    assert_eq!(city.traffic_density_at(x, y), Some(0x37));
}

#[test]
fn pe_rci_demand_updates_radioactivity_sample_skips_when_not_xbld_5() {
    let mut city = City::new_blank("/tmp/sc2000-rci-rad-skip.sc2", "RciRadSk").expect("blank");
    // Vacant map: first two rands pick a tile, third rand must not run.
    let mut crt = Win95CrtRand::new(1);
    let before = crt.seed();
    assert!(pe_rci_demand_updates_radioactivity_sample(&mut city, &mut crt).is_none());
    // Two rand() calls only (x and y); seed must differ from a third roll.
    let mut two = Win95CrtRand::new(before);
    let _ = two.random_u16();
    let _ = two.random_u16();
    assert_eq!(crt.seed(), two.seed());
}

#[test]
fn pe_pollution_accident_ready_uses_industrial_tax_current() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let offset =
        budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("industrial tax current");
    assert!(!pe_pollution_accident_ready(&city));

    city.set_misc_u32_at(offset, 0x270f);
    assert!(
        !pe_pollution_accident_ready(&city),
        "industrial tax 9999 misses the PE type-0xF arm"
    );

    city.set_misc_u32_at(offset, 0x2710);
    assert!(pe_pollution_accident_ready(&city));
}

#[test]
fn pe_pollution_accident_cloud_count_uses_dw_city_population() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(pe_population_jitter_attempt_count(&city), 5);

    let offset =
        budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("industrial tax current");
    city.set_misc_u32_at(offset, 1_000_000);
    assert_eq!(pe_population_jitter_attempt_count(&city), 5);

    city.persist_pe_day22_population_leftover(9_999);
    assert_eq!(pe_population_jitter_attempt_count(&city), 5);
    city.persist_pe_day22_population_leftover(10_000);
    assert_eq!(pe_population_jitter_attempt_count(&city), 6);
    city.persist_pe_day22_population_leftover(20_000);
    assert_eq!(pe_population_jitter_attempt_count(&city), 7);

    city.set_misc_u32_at(offset, 0);
    assert_eq!(pe_population_jitter_attempt_count(&city), 7);
}

#[test]
fn pe_pollution_accident_cloud_offset_is_rand_and_7_minus_4() {
    let mut rng = Win95CrtRand::new(0x45D8_B001);
    let mut expected = Win95CrtRand::new(0x45D8_B001);
    let (dx, dy) = pe_pollution_accident_cloud_offset(&mut rng);
    assert_eq!(dx, (expected.random_u16() & 7) as isize - 4);
    assert_eq!(dy, (expected.random_u16() & 7) as isize - 4);
    assert!((-4..=3).contains(&dx));
    assert!((-4..=3).contains(&dy));
}

#[test]
fn pe_pollution_accident_cloud_tiles_paint_fb_live() {
    // PE @ `0x45d93f`: each cloud attempt writes XTXT `0xfb` onto the map.
    let mut city = City::new_blank("/tmp/sc2000-pollution-live.sc2", "PollLive").expect("blank");
    city.persist_pe_day22_population_leftover(20_000);
    let mut rng = Win95CrtRand::new(0x45D8_B001);
    let clouds = pe_pollution_accident_cloud_tiles_with_rng(&mut city, 12, 34, &mut rng);
    assert_eq!(clouds.len(), 7);
    for &(x, y) in &clouds {
        assert_eq!(city.text_overlay_at(x, y), Some(0xfb));
    }
}

#[test]
fn mac_pollution_accident_target_jitters_city_center_by_1f_minus_0f() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // Give the centroid a known building mass away from map mid.
    for y in 10..20 {
        for x in 10..20 {
            city.set_building(x, y, 0x70);
        }
    }
    let (cx, cy) = mac_developed_tile_anchor(&city);
    assert_eq!((cx, cy), (14, 14));

    let mut rng = MacToolboxRng::new(0x35BA_A001);
    let mut expected = MacToolboxRng::new(0x35BA_A001);
    let tile = mac_pollution_accident_target_tile(&city, &mut rng);
    let want_x = (cx as isize + (expected.random_u16() & 0x1f) as isize - 0x0f)
        .clamp(0, CITY_WIDTH as isize - 1) as usize;
    let want_y = (cy as isize + (expected.random_u16() & 0x1f) as isize - 0x0f)
        .clamp(0, CITY_HEIGHT as isize - 1) as usize;
    assert_eq!(tile, (want_x, want_y));
    assert!(
        (cx as isize - 15..=cx as isize + 16).contains(&(tile.0 as isize)),
        "Mac &0x1F-0x0F stays near center on x"
    );
    assert!(
        (cy as isize - 15..=cy as isize + 16).contains(&(tile.1 as isize)),
        "Mac &0x1F-0x0F stays near center on y"
    );
}

#[test]
fn pe_population_jitter_attempt_count_uses_dw_city_population() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    assert_eq!(pe_population_jitter_attempt_count(&city), 5);
    city.persist_pe_day22_population_leftover(9_999);
    assert_eq!(pe_population_jitter_attempt_count(&city), 5);
    city.persist_pe_day22_population_leftover(10_000);
    assert_eq!(pe_population_jitter_attempt_count(&city), 6);
    city.persist_pe_day22_population_leftover(20_000);
    assert_eq!(pe_population_jitter_attempt_count(&city), 7);

    // The type-D/E/F start loops use `dwCityPopulation`; type-F's separate
    // readiness gate still uses industrial tax.
    let offset =
        budget_row_misc_offset(2, BUDGET_SERVICE_CURRENT_FIELD).expect("industrial tax current");
    city.set_misc_u32_at(offset, 1_000_000);
    assert_eq!(pe_population_jitter_attempt_count(&city), 7);

    // PE tests the computed low word with signed `jle` before its 16-bit loop.
    city.persist_pe_day22_population_leftover(327_620_000);
    assert_eq!(pe_population_jitter_attempt_count(&city), 32_767);
    city.persist_pe_day22_population_leftover(327_630_000);
    assert_eq!(pe_population_jitter_attempt_count(&city), 0);
}

#[test]
fn stamp_zone_development_military_runway_uses_class0_and_keeps_service_xbit() {
    // The military DD path uses class 0; type 7 skips the nonmilitary XBIT `|= 0xc0`.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-army-runway.sc2", "StampArmyRwy").expect("blank");
    let origin = (32, 33);
    for x in origin.0..origin.0 + 5 {
        city.set_zone(x, origin.1, ZONE_MILITARY | ZONE_CORNER_ALL);
    }
    city.set_altitude(origin.0, origin.1, 5, 5);
    city.set_tile_bits(
        origin.0,
        origin.1,
        TileBits::new(0).with_watered(true).with_piped(true),
    );
    let mut state = test_state(city);
    let plan = ZoneDevelopmentPlan {
        origin,
        zone_id: ZONE_MILITARY,
        building: RUNWAY_BUILDING_ID,
        footprint: vec![origin],
        place_as_construction: false,
    };
    state.stamp_zone_development(&plan);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        RUNWAY_BUILDING_ID
    );
    assert_eq!(
        state
            .city
            .tile_bits_at(origin.0, origin.1)
            .unwrap()
            .raw_bits()
            & 0xf0,
        0x30,
        "class 0 preserves the preexisting piped and watered bits"
    );
    assert_eq!(
        state
            .city
            .tile_bits_at(origin.0, origin.1)
            .unwrap()
            .raw_bits()
            & XBIT_CHOOSE_AND_PLACE,
        0x20,
        "class 0 preserves piped XBIT without adding ChooseAndPlace power flags"
    );
}

#[test]
fn monthly_fb_cloud_does_not_destroy_non_zone_utility_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 220);
    city.set_tile_bits(
        20,
        20,
        crate::city::TileBits::new(0)
            .with_piped(true)
            .with_watered(true),
    );
    city.set_traffic_density(20, 20, 99);
    city.set_text_overlay(20, 20, 0xfb);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    assert_eq!(
        state.city.building_at(20, 20),
        220,
        "PE 0xfb degrades eligible zone buildings; it does not DestroyStructure a water pump"
    );
    // Month advance also runs day-20 water; piped/watered preservation is covered by
    // `damage_disaster_tile_preserves_watered_like_pe_401604`.
    assert_eq!(
        state.city.traffic_density_at(20, 20),
        Some(75),
        "DestroyStructure leaves XTRF; monthly decay applies value-(value>>2) to 99"
    );
}

#[test]
fn pinned_sim_status_survives_clock_refresh() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.set_sim_status_announcement("Bridge Collapsed");

    state.refresh_status_line();
    state.sim_calendar_day1_power();

    assert_eq!(state.query_line, "Bridge Collapsed");
    assert!(state.query_status_pinned);
}

#[test]
fn pe_maptool_funds_debit_44338a_tool0_cost_one() {
    // PE @ `0x44338a`: tool 0 cost 1; refuse when funds < cost (signed).
    let mut city = City::new_blank("/tmp/sc2000-map-funds0.sc2", "MapFunds0").expect("blank");
    city.set_city_funds(0);
    assert!(!pe_maptool_funds_debit_44338a(&mut city, 0));
    assert_eq!(city.city_funds(), 0);

    city.set_city_funds(1);
    assert!(pe_maptool_funds_debit_44338a(&mut city, 0));
    assert_eq!(city.city_funds(), 0);

    city.set_city_funds(2);
    assert!(pe_maptool_funds_debit_44338a(&mut city, 0));
    assert_eq!(city.city_funds(), 1);
}

#[test]
fn direct_building_placement_requires_available_funds() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(3_999);
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantCoal;

    assert_eq!(state.place_building_at(20, 20, 207), 0);
    assert_eq!(state.city.city_funds(), 3_999);
    assert_eq!(state.city.building_at(20, 20), 0);

    state.city.set_city_funds(4_000);
    assert_eq!(state.place_building_at(20, 20, 207), 16);
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.building_at(20, 20), 207);
}

#[test]
fn hydro_power_placement_requires_available_funds() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(399);
    city.set_terrain(20, 20, 0x3e);
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantHydro;

    assert_eq!(state.place_hydro_power_at(20, 20), 0);
    assert_eq!(state.city.city_funds(), 399);
    assert_eq!(state.city.building_at(20, 20), 0);

    state.city.set_city_funds(400);
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(state.city.city_funds(), 0);
    // PE @ `0x441610`: rot0 + flat neighbors → delta 1 → XBLD `0xC7`.
    assert_eq!(state.city.building_at(20, 20), 199);
}

#[test]
fn terrain_and_nature_placement_require_available_funds() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(24);
    set_flat_terrain_neighborhood_for_test(&mut city, 10, 10, 5);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;

    assert!(!state.place_city_terrain_edit_at(10, 10));
    assert_eq!(state.city.city_funds(), 24);
    assert_eq!(state.city.altitude_at(10, 10), 5);

    state.city.set_city_funds(25);
    assert!(state.place_city_terrain_edit_at(10, 10));
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.altitude_at(10, 10), 6);

    state.tool = EditTool::Trees;
    state.city.set_city_funds(2);
    let seed_before_unaffordable_tree = state.pe_process_crt_seed();
    assert_eq!(state.place_tree_at(11, 10), 0);
    assert_eq!(state.city.building_at(11, 10), 0);
    assert_eq!(state.city.city_funds(), 2);
    assert_eq!(
        state.pe_process_crt_seed(),
        seed_before_unaffordable_tree,
        "PE controller checks the $3 tree cost before entering the CRT helper"
    );

    state.city.set_city_funds(3);
    let mut expected_tree_crt = state.pe_process_crt;
    let expected_tree = (expected_tree_crt.random_u16() & 1) as u8 + 6;
    assert_eq!(state.place_tree_at(11, 10), 1);
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.building_at(11, 10), expected_tree);
    assert_eq!(state.pe_process_crt_seed(), expected_tree_crt.seed());

    state.tool = EditTool::Water;
    state.city.set_city_funds(99);
    assert_eq!(state.place_surface_water_at(12, 10), 0);
    assert_eq!(state.city.terrain_at(12, 10), 0);

    state.city.set_city_funds(100);
    assert_eq!(state.place_surface_water_at(12, 10), 1);
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.terrain_at(12, 10), 0x3d);
}

#[test]
fn paid_special_tools_require_available_funds() {
    let mut city = tunnel_test_city();
    city.set_city_funds(599);
    let mut state = test_state(city);
    state.tool = EditTool::Tunnel;

    assert_eq!(state.place_tunnel_at(20, 20), 0);
    assert_eq!(state.city.city_funds(), 599);
    assert_eq!(state.city.building_at(20, 20), 0);

    state.city.set_city_funds(600);
    assert_eq!(state.place_tunnel_at(20, 20), 4);
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.building_at(20, 20), 63);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
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
    assert_ne!(state.city.building_at(ramp.0, ramp.1), 0);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(249);
    city.set_building(21, 20, 0x2c);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;

    assert!(!state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.city_funds(), 249);
    assert_eq!(state.city.building_at(20, 20), 0);

    state.city.set_city_funds(250);
    assert!(state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.city_funds(), 0);
    assert_ne!(state.city.building_at(20, 20), 0);
}

#[test]
fn bulldoze_apply_requires_available_funds() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 112);
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;

    assert!(!state.apply_tool_at_tile(10, 10));
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.building_at(10, 10), 112);

    state.city.set_city_funds(1);
    assert!(state.apply_tool_at_tile(10, 10));
    assert_eq!(state.city.city_funds(), 0);
    assert!(matches!(state.city.building_at(10, 10), 1..=4));
}

#[test]
fn citytool_school_college_write_microsim_xtxt_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440ea0`→`0x44cf60`: school `0xD6` 3×3, college `0xD9` 4×4.
    for &(building, tool, label, edge) in &[
        (214u8, EditTool::School, "School", 3usize),
        (217u8, EditTool::College, "College", 4usize),
    ] {
        let mut city =
            City::new_blank(format!("/tmp/sc2000-ct-{label}-xtxt.sc2"), label).expect("blank");
        city.set_city_funds(50_000);
        let mut state = test_state(city);
        state.tool = tool;
        assert_eq!(
            state.place_building_at(56, 56, building),
            edge * edge,
            "{label}"
        );
        let xtxt = state.city.text_overlay_at(56, 56).unwrap_or(0);
        assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "{label} microsim XTXT {xtxt}");
        for dx in 0..edge {
            for dy in 0..edge {
                assert_eq!(
                    state.city.text_overlay_at(56 + dx, 56 + dy),
                    Some(xtxt),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn citytool_nimby_census_uses_click_not_post_dec_nw() {
    // PE @ `0x440648` counts from CityTool click before PlaceTile-size size>2 dec.
    // NW (16,16) → click (17,17): PE window includes (28,*), NW-window does not.
    let mut city = City::new_blank("/tmp/sc2000-ct-nimby-click.sc2", "CtNimbyC").expect("blank");
    city.set_city_funds(50_000);
    city.set_zone(28, 20, ZONE_RESIDENTIAL_LIGHT);
    assert_eq!(
        pe_citytool_nimby_residential_count_440648(&city, 17, 17, 4),
        1,
        "PE click window"
    );
    assert_eq!(
        pe_citytool_nimby_residential_count_440648(&city, 16, 16, 4),
        0,
        "post-dec NW window must not count click-exclusive tile"
    );
    let mut seed = 1u32;
    loop {
        let mut probe = Win95Lfsr::from_u32_seed(seed);
        if probe.random_mod(0xc8) == 0 {
            break;
        }
        seed = seed.wrapping_add(1);
    }
    let mut state = test_state(city);
    state.pe_process_lfsr = Win95Lfsr::from_u32_seed(seed);
    state.tool = EditTool::Prison;
    // place_building_at args are post-dec NW; census must restore click.
    assert_eq!(state.place_building_at(16, 16, 216), 0);
    assert_eq!(state.city.building_at(16, 16), 0);
}

#[test]
fn citytool_nimby_refuse_does_not_debit_funds() {
    // PE NIMBY refuse @ `0x4406e9` jumps out before the fund debit @ `0x440746`.
    let mut city = City::new_blank("/tmp/sc2000-ct-nimby-funds.sc2", "CtNimbyF").expect("blank");
    city.set_city_funds(50_000);
    for x in 10..30usize {
        for y in 10..30usize {
            city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
        }
    }
    let mut state = test_state(city);
    state.tool = EditTool::Prison;
    assert_eq!(state.place_building_at(16, 16, 216), 0);
    assert_eq!(state.city.city_funds(), 50_000);
}

#[test]
fn mac68k_zone_drag_with_insufficient_funds_places_nothing() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-zone-price-insufficient.sc2",
        "MacZoneFunds",
    )
    .expect("blank city");
    city.set_city_funds(10);
    city.set_zone(10, 10, ZONE_COMMERCIAL_DENSE);
    city.set_zone(11, 10, ZONE_RESIDENTIAL_LIGHT);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);
    state.update_zone_drag_to((12, 10));

    assert_eq!(state.zone_drag_cost(), Some(20));
    assert_eq!(state.finish_zone_drag(), 0);
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().zone_id(),
        ZONE_COMMERCIAL_DENSE
    );
    assert_eq!(
        state.city.zone_at(11, 10).unwrap().zone_id(),
        ZONE_RESIDENTIAL_LIGHT
    );
    assert_eq!(state.city.zone_at(12, 10).unwrap().zone_id(), 0);
    assert_eq!(state.city.city_funds(), 10);
}

#[test]
fn win95_zone_drag_with_insufficient_funds_places_nothing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(10);
    city.set_zone(10, 10, ZONE_COMMERCIAL_DENSE);
    city.set_zone(11, 10, ZONE_RESIDENTIAL_LIGHT);
    let mut state = test_state(city);
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);
    state.update_zone_drag_to((12, 10));

    assert_eq!(state.zone_drag_cost(), Some(20));
    assert_eq!(state.finish_zone_drag(), 0);
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().zone_id(),
        ZONE_COMMERCIAL_DENSE
    );
    assert_eq!(
        state.city.zone_at(11, 10).unwrap().zone_id(),
        ZONE_RESIDENTIAL_LIGHT
    );
    assert_eq!(state.city.zone_at(12, 10).unwrap().zone_id(), 0);
    assert_eq!(state.city.city_funds(), 10);
}

#[test]
fn highway_slope_funds_debit_uses_4dc260_times_steps_plus_one() {
    // PE @ `0x44c110`–`0x44c295`: cost = `0x4dc260[tool_id] * (steps+1)`.
    // Tool-id 1 → 100; no-climb walk takes 1 step → debit 200.
    let mut city = City::new_blank("/tmp/sc2000-hwy-funds.sc2", "HwyFunds").expect("blank");
    city.set_terrain(20, 20, 2);
    city.set_terrain(20, 19, 4);
    city.set_city_funds(250);
    let mut state = test_state(city);
    let steps = state
        .place_highway_slope_walk_steps(20, 20, 2)
        .expect("paired slope walk");
    assert_eq!(steps, 1);
    assert!(pe_place_highway_atcoordinates_funds_debit_44c267(
        &mut state.city,
        1,
        steps
    ));
    assert_eq!(state.city.city_funds(), 50);
    assert!(!pe_place_highway_atcoordinates_funds_debit_44c267(
        &mut state.city,
        1,
        steps
    ));
    assert_eq!(state.city.city_funds(), 50);

    // One climb step: start → higher → end ⇒ steps 2 → cost 300.
    state.city.set_city_funds(300);
    state.city.set_altitude(20, 20, 5, 0);
    state.city.set_altitude(20, 19, 6, 0);
    state.city.set_altitude(20, 18, 5, 0);
    state.city.set_terrain(20, 18, 4);
    state.city.set_terrain(20, 19, 0);
    let steps = state
        .place_highway_slope_walk_steps(20, 20, 2)
        .expect("climb walk");
    assert_eq!(steps, 2);
    assert!(pe_place_highway_atcoordinates_funds_debit_44c267(
        &mut state.city,
        1,
        steps
    ));
    assert_eq!(state.city.city_funds(), 0);
}

#[test]
fn simulation_service_refresh_initializes_loaded_water_bits() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.city.set_zone(11, 10, 1);
    state
        .city
        .set_tile_bits(10, 10, crate::city::TileBits::new(0));
    state
        .city
        .set_tile_bits(11, 10, crate::city::TileBits::new(0));

    state.city.set_building(11, 10, 112);
    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_simulation_service_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn simulation_service_refresh_does_not_create_empty_xbit_map() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.clear_tile_bits_for_test();
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();

    assert!(!state.city.has_tile_bits_map());
}

#[test]
fn simulation_service_refresh_creates_xbit_map_for_new_service_bits() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.clear_tile_bits_for_test();
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.city.set_zone(11, 10, 1);
    state.city.set_building(11, 10, 112);

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_simulation_service_bits();

    assert!(state.city.has_tile_bits_map());
    assert!(state.city.tile_bits_at(10, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn simulation_service_refresh_keeps_power_computed_without_xbit_power_flag() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut state.city, 10, 10, 4);
    state.city.set_zone(11, 10, 1);
    state
        .city
        .set_tile_bits(11, 10, crate::city::TileBits::new(0b0000_0010));

    state.refresh_simulation_service_bits();

    let bits = state.city.tile_bits_at(11, 10).unwrap();
    assert!(bits.rotate());
    assert_eq!(bits.raw_bits() & 0b0000_0010, 0b0000_0010);
    assert_eq!(tile_power_status_text(&state.city, 11, 10), " pwr=on");
    assert_eq!(ServiceStatus::for_city(&state.city).power_shortage(), 0);
}

#[test]
fn simulation_month_advance_refreshes_service_bits() {
    let mut state = water_consumer_fixture_for_calendar_test();

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.handle_simulation_month_advanced();

    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn simulation_month_refreshes_water_bits_without_zone_growth() {
    let mut state = water_consumer_fixture_for_calendar_test();
    state.zone_growth_enabled = false;

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.handle_simulation_month_advanced();

    assert!(
        state.city.tile_bits_at(11, 10).unwrap().piped(),
        "day-20 water pass should run even when zone growth is off"
    );
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn win95_service_stats_keep_mac_zero_fill_and_native_slots() {
    let mut state = water_consumer_fixture_for_calendar_test();
    state.city.kind = CityFileKind::MacCity;
    state
        .city
        .set_misc_u32_at(GRAPH_EXTRA_HISTORY_MISC_OFFSET, 1);
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x1111);
    state
        .city
        .set_misc_u32_at(MISC_WATER_USED_PERCENT_OFFSET, 0x2222);
    state.city.set_misc_u32_at(MISC_SEWER_BONUS_OFFSET, 0x3333);
    // State slot 1048 is the native Mac power-service percentage. Its upper
    // bytes are opaque and must survive the Win95 writer in this hybrid run.
    state
        .city
        .set_misc_u32_at(MAC_POWER_SERVICE_PERCENT_STATE_OFFSET, 0xaabb_ccdd);

    // Win95 simulation may run over Mac-format city data. Runtime service
    // caches still advance, but none of these PE snapshots owns Mac bytes.
    state.sim_calendar_day1_power();
    state.sim_calendar_day20_water();

    assert_eq!(
        state.city.misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET),
        Some(0x1111)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_WATER_USED_PERCENT_OFFSET),
        Some(0x2222)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_SEWER_BONUS_OFFSET),
        Some(0x3333)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(MAC_POWER_SERVICE_PERCENT_STATE_OFFSET),
        Some(0xaabb_ccdd)
    );

    // The direct sewer publisher also retains its PE cache result without
    // writing the overlapping Mac state slot.
    state.city.publish_pe_sewer_bonus(2_000, 4);
    assert_eq!(state.city.pe_w_sewer_bonus(), 1);
    assert_eq!(
        state.city.misc_u32_at(MISC_SEWER_BONUS_OFFSET),
        Some(0x3333)
    );
}

#[test]
fn win95_service_snapshots_do_not_extend_short_misc() {
    let mut state = water_consumer_fixture_for_calendar_test();
    let short_misc = vec![0; MISC_SPEED_OFFSET];
    let encoded = crate::city::sc2_rle_encode(&short_misc);
    let chunk = state
        .city
        .chunks
        .iter_mut()
        .find(|chunk| chunk.name == "MISC")
        .expect("blank city has MISC");
    chunk.data = encoded;
    chunk.raw_len = short_misc.len();

    // Runtime service values still update when a truncated Win95 record has
    // no viewer-extension slots, but the missing snapshots stay absent.
    state.sim_calendar_day20_water();
    assert_eq!(state.city.misc_u32_at(MISC_WATER_USED_PERCENT_OFFSET), None);
    assert_eq!(state.city.misc_u32_at(MISC_SEWER_BONUS_OFFSET), None);

    // The City-level writer also keeps its PE cache without creating the
    // missing sewer slot as a side effect.
    state.city.publish_pe_sewer_bonus(2_000, 4);
    assert_eq!(state.city.pe_w_sewer_bonus(), 1);
    assert_eq!(state.city.misc_u32_at(MISC_SEWER_BONUS_OFFSET), None);
}

#[test]
fn service_utilization_percent_matches_win95_capacity_rules() {
    assert_eq!(service_utilization_percent(0, 0), 100);
    assert_eq!(service_utilization_percent(0, 50), 0);
    assert_eq!(service_utilization_percent(25, 50), 50);
    assert_eq!(service_utilization_percent(50, 50), 100);
    assert_eq!(service_utilization_percent(75, 50), 100);
}

#[test]
fn pe_zone_pops_scaled_population_sums_rci_without_arcologies() {
    let population = PopulationStats {
        residential_population: 100,
        commercial_population: 20,
        industrial_population: 3,
        arcology_population: 85_000,
        normal_population: 100,
        total_population: 85_123,
        ..PopulationStats::default()
    };
    assert_eq!(pe_zone_pops_scaled_population(population), 123);
}

#[test]
fn publish_pe_growth_aggregate_mirrors_dw_city_population_to_misc() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank");
    let mut state = test_state(city);
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
    state.publish_pe_growth_aggregate();
    let scaled = state.city.pe_dw_city_population();
    assert!(scaled > 0);
    assert_eq!(
        state.city.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET),
        Some(scaled),
        "day-21 publish keeps MISC 0x102c = pe_dw / dwCityPopulation"
    );
}

#[test]
fn day23_skips_misc_normal_population_overwrite_when_pe_zone_pops_ready() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank");
    city.set_building(10, 10, 0x70);
    let census = live_population_stats_for_city(&city).normal_population;
    assert!(census > 0);
    let mut state = test_state(city);
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
    state.publish_pe_growth_aggregate();
    let scaled = state.city.pe_dw_city_population();
    assert!(scaled > 0);
    assert_ne!(scaled, census);
    assert_eq!(
        state.city.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET),
        Some(scaled)
    );

    state.sim_calendar_day23_stats_dialogs();
    assert_eq!(
        state.city.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET),
        Some(scaled),
        "day 23 must not replace published dwCityPopulation MISC with census"
    );
}

#[test]
fn publish_pe_growth_aggregate_adds_garbage_and_skips_day22_census_overwrite() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank");
    city.set_building(10, 10, 0x70);
    let census = pe_zone_pops_scaled_population(live_population_stats_for_city(&city));
    assert!(census > 0);
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
    let scaled = {
        let mut probe = PeGrowthAggregate::default();
        probe.add_xbld_candidate(1, 0x70);
        probe.fold_normal_slots();
        probe.scaled_city_population().max(0) as u32
    };
    assert!(scaled > 0);
    assert_ne!(scaled, census);

    state.publish_pe_growth_aggregate();
    assert!(state.city.pe_zone_pops_ready());
    assert_eq!(state.city.pe_dw_city_population(), scaled);
    assert_eq!(state.city.misc_u32_at(MISC_GARBAGE_OFFSET), Some(scaled));
    assert_eq!(state.city.pe_eq_leftover_deaths(), scaled);

    let population = live_population_stats_for_city(&state.city);
    let _ = state.sim_calendar_day22_scenario(population);

    assert_eq!(
        state.city.pe_dw_city_population(),
        scaled,
        "day 22 must not replace aggregate dwCityPopulation with census"
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_GARBAGE_OFFSET),
        Some(scaled),
        "day 22 must not double-add census garbage after day-21 publish"
    );
    assert_eq!(state.city.pe_eq_leftover_deaths(), scaled);
}

#[test]
fn pe_monthly_condition_story_queues_on_power_used_over_98() {
    // PE FUN_00471BC0 @ 0x471c01: trend < 9 and dwPowerUsedPercentage > 0x62 → story 0x2E.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-pwr-story.sc2", "PwrStory").expect("blank"));
    state.no_disasters_enabled = true;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x63);
    // Transit arm would also match on a blank city; power arm must win first.
    state.run_pe_phase24_disaster_chooser();
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x2e));
}

#[test]
fn pe_monthly_condition_story_queues_police_prison_shortage_0x30() {
    // PE @ 0x471c65: (police/9 + prison/16) <= pop/0x4e20 → story 0x30.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-police-story.sc2", "PoliceT").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    // Set the live population used by the phase-24 chooser.
    state.city.set_pe_zone_pops([2_000, 0, 0]);
    state.city.persist_pe_day22_population_leftover(20_000);
    // Cover transit so arm 1 does not win: 300 subway tiles > 200.
    // PE transit arm @ `0x471c13` reads `dwTileCount[0xe9]`, not a map rescan.
    state.city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 300;
    // 0 police/prison <= 1 → story 0x30.
    state.run_pe_phase24_disaster_chooser();
    assert_eq!(state.city.pe_dw_city_population(), 20_000);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x30));
}

#[test]
fn pe_monthly_condition_police_uses_dw_tile_count_like_471c65() {
    // PE @ `0x471c65`: police/prison from `0x4ca4c8`, not a live map rescan.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-police-dw.sc2", "PoliceDw").expect("blank"));
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
    state.city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 300;
    // Map has two police stations but census words stay 0 → still shortage.
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 34, 30, 210, 3);
    state.city.pe_dw_tile_count[210] = 0;
    state.run_pe_phase24_disaster_chooser();
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x30));
}

#[test]
fn pe_monthly_condition_story_queues_water_coverage_0x32() {
    // PE @ 0x471cdb: `0x4CC4B8` water coverage > 0x62 → story 0x32.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-water-cov-story.sc2", "WaterCov").expect("blank"));
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
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 34, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 40, 30, FIRE_STATION_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 44, 30, FIRE_STATION_BUILDING_ID, 3);
    state.city.set_pe_water_coverage_percent(0x63);
    state.run_pe_phase24_disaster_chooser();
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x32));
}

#[test]
fn pe_monthly_condition_story_skips_water_coverage_at_98() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-water-cov98.sc2", "Water98").expect("blank"));
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
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 34, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 40, 30, FIRE_STATION_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 44, 30, FIRE_STATION_BUILDING_ID, 3);
    state.city.set_pe_water_coverage_percent(0x62);
    // Water fails; pop 20000/25000=0 so 0 hospitals → hospital story 0x33.
    state.run_pe_phase24_disaster_chooser();
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x32));
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x33));
}

#[test]
fn pe_monthly_condition_story_queues_hospital_shortage_0x33() {
    // PE @ 0x471d03: hospital(209)/9 <= pop/0x61a8 → story 0x33.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-hospital-story.sc2", "HospT").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.set_pe_zone_pops([2_500, 0, 0]);
    state.city.persist_pe_day22_population_leftover(25_000);
    // PE transit arm @ `0x471c13` reads `dwTileCount[0xe9]`, not a map rescan.
    state.city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 300;
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 34, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 40, 30, FIRE_STATION_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 44, 30, FIRE_STATION_BUILDING_ID, 3);
    state.city.set_pe_water_coverage_percent(0x62);
    // 0 hospitals <= 25000/25000=1 → story 0x33.
    state.run_pe_phase24_disaster_chooser();
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x33));
}

#[test]
fn pe_monthly_condition_story_skips_hospital_when_above_demand() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-hospital-ok.sc2", "HospOk").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.set_pe_zone_pops([2_500, 0, 0]);
    state.city.persist_pe_day22_population_leftover(25_000);
    // PE transit arm @ `0x471c13` reads `dwTileCount[0xe9]`, not a map rescan.
    state.city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 300;
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 34, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 40, 30, FIRE_STATION_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 44, 30, FIRE_STATION_BUILDING_ID, 3);
    state.city.set_pe_water_coverage_percent(0x62);
    // Need hospital/9 > 1 → two hospitals.
    stamp_complete_building_for_test(&mut state.city, 50, 30, HOSPITAL_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 54, 30, HOSPITAL_BUILDING_ID, 3);
    // Hospital covered; 0 schools <= pop/20000=1 → school story 0x34.
    state.run_pe_phase24_disaster_chooser();
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x33));
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x34));
}

#[test]
fn pe_monthly_condition_story_queues_school_shortage_0x34() {
    // PE @ 0x471d36: school(214)/9 <= esi(pop/0x4e20) → story 0x34.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-school-story.sc2", "SchoolT").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.set_pe_zone_pops([2_500, 0, 0]);
    state.city.persist_pe_day22_population_leftover(25_000);
    // PE transit arm @ `0x471c13` reads `dwTileCount[0xe9]`, not a map rescan.
    state.city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 300;
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 34, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 40, 30, FIRE_STATION_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 44, 30, FIRE_STATION_BUILDING_ID, 3);
    state.city.set_pe_water_coverage_percent(0x62);
    stamp_complete_building_for_test(&mut state.city, 50, 30, HOSPITAL_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 54, 30, HOSPITAL_BUILDING_ID, 3);
    // 0 schools <= 1 → story 0x34.
    state.run_pe_phase24_disaster_chooser();
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x34));
}

#[test]
fn pe_monthly_condition_story_skips_school_when_above_demand() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-school-ok.sc2", "SchoolOk").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.set_pe_zone_pops([2_500, 0, 0]);
    state.city.persist_pe_day22_population_leftover(25_000);
    // PE transit arm @ `0x471c13` reads `dwTileCount[0xe9]`, not a map rescan.
    state.city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 300;
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 34, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 40, 30, FIRE_STATION_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 44, 30, FIRE_STATION_BUILDING_ID, 3);
    state.city.set_pe_water_coverage_percent(0x62);
    stamp_complete_building_for_test(&mut state.city, 50, 30, HOSPITAL_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 54, 30, HOSPITAL_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 60, 30, SCHOOL_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut state.city, 64, 30, SCHOOL_BUILDING_ID, 3);
    // Industrial tax 0 → seaport arm does not fire; later arms not invented.
    state.run_pe_phase24_disaster_chooser();
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x34));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x35));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x3b));
}

#[test]
fn citygame_non_network_marker_census_uses_destroy_building_class_filter() {
    // PE DestroyBuilding else-arm `0x45a150`: XTXT 0xfa outside network bands → `0x4ca3f0`.
    let mut city = City::new_blank("/tmp/sc2000-fa-nonnet.sc2", "FaNN").expect("blank");
    city.set_building(20, 20, 0x0d); // small park — non-network
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0xfa); // desal XBLD 250 — non-network
    city.set_text_overlay(21, 20, 0xfa);
    city.set_building(22, 20, 0x1d); // road — network → 4cc4d8 only
    city.set_text_overlay(22, 20, 0xfa);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 2);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
}

#[test]
fn pe_monthly_condition_recreation_uses_dw_tile_count_like_471e13() {
    // PE @ `0x471e13`: parks from `dwTileCount[0xd5]`, not a live map rescan.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-recreation-dw.sc2", "RecDw").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    let res = budget_row_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).expect("res tax");
    state.city.set_misc_u32_at(res, 2_000); // threshold 2
    stamp_complete_building_for_test(&mut state.city, 70, 30, BIG_PARK_BUILDING_ID, 3);
    state.city.pe_dw_tile_count[BIG_PARK_BUILDING_ID as usize] = 0;
    let mut preview = state.pe_process_crt;
    let expected = 0x2eu16.saturating_add(u16::from(preview.random_u15() & 3).saturating_add(9));
    state.run_pe_phase24_disaster_chooser();
    assert_eq!(
        state.city.pe_newspaper_story_id_at(0),
        Some(u32::from(expected))
    );
}

#[test]
fn citygame_non_network_marker_census_rebuilds_from_saved_xtxt() {
    // CityGame's seaport-story census derives from persisted XTXT; it is not
    // the Win95 DAT_004CA3F0 counter, which resets before saved-city loading.
    let path = "/tmp/sc2000-neighbor-marker-save-rebuild.sc2";
    let mut city = City::new_blank(path, "NeighborMarkerReload").expect("blank");
    city.set_building(20, 20, 0x0d); // non-network XBLD class
    city.set_text_overlay(20, 20, 0xfa);
    city.save(path).expect("save marker city");

    let loaded = City::load(path).expect("reload marker city");
    assert_eq!(loaded.text_overlay_at(20, 20), Some(0xfa));
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&loaded), 1);
    std::fs::remove_file(path).expect("remove marker city fixture");
}

#[test]
fn pe_monthly_condition_story_queues_recreation_shortage_0x37_to_0x3a() {
    // PE @ 0x471e13: parks/stadium/zoo/marina < (R_tax − 2*arco/12)/1000 →
    // stories 0x37..=0x3A from CRT rand()&3.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-recreation-story.sc2", "RecT").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    let res = budget_row_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).expect("res tax");
    state.city.set_misc_u32_at(res, 2_000); // /1000 = 2; 0 recreation < 2
    // Predict CRT pick before phase 24 consumes it (chooser is first CRT consumer here).
    let mut preview = state.pe_process_crt;
    let expected = 0x2eu16.saturating_add(u16::from(preview.random_u15() & 3).saturating_add(9));
    state.run_pe_phase24_disaster_chooser();
    assert_eq!(
        state.city.pe_newspaper_story_id_at(0),
        Some(u32::from(expected))
    );
    assert!(matches!(expected, 0x37..=0x3a), "expected {expected:#x}");
}

#[test]
fn pe_monthly_condition_story_skips_recreation_when_parks_cover_tax() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-recreation-ok.sc2", "RecOk").expect("blank"));
    pe_monthly_condition_cover_through_school(&mut state);
    let res = budget_row_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).expect("res tax");
    state.city.set_misc_u32_at(res, 2_000); // threshold 2
    // Big park 3×3 → /3 = 3 units >= 2.
    stamp_complete_building_for_test(&mut state.city, 70, 30, BIG_PARK_BUILDING_ID, 3);
    state.run_pe_phase24_disaster_chooser();
    assert!(
        !matches!(state.city.pe_newspaper_story_id_at(0), Some(0x37..=0x3a)),
        "recreation covered"
    );
}

#[test]
fn pe_monthly_condition_story_skips_school_below_pop_8000_after_cover() {
    // Cover school so index 6 fails; pop < 8000 → no story at @ 0x471d5b.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-school-lowpop.sc2", "SchLo").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.set_pe_zone_pops([799, 0, 0]);
    state.city.persist_pe_day22_population_leftover(7_999);
    // PE transit arm @ `0x471c13` reads `dwTileCount[0xe9]`, not a map rescan.
    state.city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 300;
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 40, 30, FIRE_STATION_BUILDING_ID, 3);
    state.city.set_pe_water_coverage_percent(0x62);
    // hospital demand 7999/25000=0 → need >0 hospitals.
    stamp_complete_building_for_test(&mut state.city, 50, 30, HOSPITAL_BUILDING_ID, 3);
    // school demand 7999/20000=0 → need >0 schools to pass arm.
    stamp_complete_building_for_test(&mut state.city, 60, 30, SCHOOL_BUILDING_ID, 3);
    state.run_pe_phase24_disaster_chooser();
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x34));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x33));
}

#[test]
fn pe_monthly_condition_story_skips_hospital_below_pop_3000() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-hospital-lowpop.sc2", "HospLo").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    state.city.set_pe_zone_pops([299, 0, 0]);
    state.city.persist_pe_day22_population_leftover(2_999);
    // PE transit arm @ `0x471c13` reads `dwTileCount[0xe9]`, not a map rescan.
    state.city.pe_dw_tile_count[SUBWAY_STATION_BUILDING_ID as usize] = 300;
    stamp_complete_building_for_test(&mut state.city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut state.city, 40, 30, FIRE_STATION_BUILDING_ID, 3);
    state.city.set_pe_water_coverage_percent(0x62);
    state.run_pe_phase24_disaster_chooser();
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x33));
}

#[test]
fn pe_monthly_condition_story_skips_at_power_used_98() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-pwr-story98.sc2", "Pwr98").expect("blank"));
    state.no_disasters_enabled = true;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_CLEAR));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x62);
    // Blank city still hits transit arm at pop 0 (0 <= 0) → 0x2F, not power.
    state.city.persist_pe_day22_population_leftover(0);
    for x in 0..5 {
        state.city.set_building(x, 10, SUBWAY_STATION_BUILDING_ID);
    }
    // 5 stations > 0 → skip transit; pop < 1000 → no story.
    state.run_pe_phase24_disaster_chooser();
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x2e));
}

#[test]
fn pe_monthly_condition_story_skips_when_weather_trend_severe() {
    // PE trend >= 9 takes the alternate paper path; power arm does not fire.
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-pwr-trend.sc2", "PwrTrend").expect("blank"));
    state.no_disasters_enabled = true;
    state
        .city
        .set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(WEATHER_TREND_BLIZZARD));
    state
        .city
        .set_misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET, 0x63);
    state.run_pe_phase24_disaster_chooser();
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x2e));
    assert_ne!(state.city.pe_newspaper_story_id_at(0), Some(0x2f));
}

#[test]
fn simulation_day22_requires_population_strictly_above_threshold() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state.city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 0);
    let population = PopulationStats {
        residential_population: 2_000,
        total_population: 2_000,
        populated_tiles: 1,
        ..PopulationStats::default()
    };

    let day22 = state.sim_calendar_day22_scenario(population);

    assert!(!day22.status_announced);
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET),
        Some(0)
    );
    assert_eq!(state.city.rewards_available_mask(), 0);
}

#[test]
fn simulation_day22_progression_uses_pe_dw_not_census_total() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.no_disasters_enabled = true;
    state.city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 0);
    state.city.set_rewards_available_mask(0);
    // Aggregate already published: day 22 must not re-derive from census.
    state.city.persist_pe_day22_population_leftover(2_001);
    state.city.set_pe_zone_pops([200, 0, 0]);
    let population = PopulationStats {
        total_population: 0,
        residential_population: 0,
        populated_tiles: 1,
        ..PopulationStats::default()
    };
    let day22 = state.sim_calendar_day22_scenario(population);

    assert_eq!(state.city.pe_dw_city_population(), 2_001);
    assert!(day22.status_announced);
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET),
        Some(1)
    );

    // Census total alone must not advance when pe_dw stays below the threshold.
    let mut low = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    low.no_disasters_enabled = true;
    low.city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 0);
    low.city.persist_pe_day22_population_leftover(1_999);
    low.city.set_pe_zone_pops([199, 0, 0]);
    let fat_census = PopulationStats {
        total_population: 100_000,
        residential_population: 100_000,
        populated_tiles: 1,
        ..PopulationStats::default()
    };
    let _ = low.sim_calendar_day22_scenario(fat_census);
    assert_eq!(low.city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET), Some(0));
    assert_eq!(low.city.pe_dw_city_population(), 1_999);
}

#[test]
fn military_air_force_growth_uses_pe_military_tile_count() {
    // PE @ `0x436389`: runway_units from `0x4ca934` words 1+2, not a map rescan.
    let mut city = City::new_blank("/tmp/sc2000-mil-af-growth.sc2", "MilAf").expect("blank");
    city.pe_military_tile_count[1] = 10; // runway
    city.pe_military_tile_count[2] = 0; // cross
    // parking/4 < runway/5 → control tower path starts; keep other words 0 so
    // multiplied/divided gates stay below runway_units and select tower.
    assert_eq!(
        select_military_air_force_growth_building(&city),
        MILITARY_CONTROL_TOWER_BUILDING_ID
    );
    // Map still vacant — BSS alone drives the picker.
    assert_eq!(city.building_at(0, 0), 0);
}

#[test]
fn military_navy_growth_uses_pe_military_crane_word() {
    // PE @ `0x43646f`: crane count is `0x4ca934` word 10 (`0xe0`).
    let mut city = City::new_blank("/tmp/sc2000-mil-navy-growth.sc2", "MilNavy").expect("blank");
    city.pe_military_tile_count[10] = 4; // crane
    assert_eq!(
        select_military_navy_growth_building(&city),
        MILITARY_TOP_SECRET_BUILDING_ID
    );
}

#[test]
fn pe_military_base_eight_by_eight_census_ignores_zone_and_underground() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-military-census.sc2", "MilCensus").expect("blank");
    for y in 40..48 {
        for x in 50..58 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    city.set_zone(51, 41, ZONE_RESIDENTIAL_LIGHT);
    city.set_underground(52, 42, 0x22);
    city.set_building(53, 43, SMALL_PARK_BUILDING_ID);
    city.set_tile_bits(54, 44, TileBits::new(0x04));
    city.set_text_overlay(55, 45, 0x17);

    // PE counts all cells regardless of XZON/XUND, but excludes the three
    // XBLD/XBIT/XTXT failures above. Elevation is compared as low five bits.
    assert_eq!(evaluate_military_base_plot(&city, 50, 40), Some((61, 61)));
}

#[test]
fn military_zone_growth_develops_army_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_ARMY);
    stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_MILITARY);
    power_adjacent_building_for_test(&mut city, 20, 19);
    let mut state = test_state(city);
    // PE SimulationGrow army @ `0x436304`: `test al, 3` must be 0 before the picker.
    seed_pe_process_crt_until_special_zone_develops(&mut state);

    state.develop_special_zones();
    // PE SimulationGrow `0x4376b0` type-7 allow: empty army base starts with parking.
    assert_eq!(
        state.city.building_at(20, 20),
        MILITARY_PARKING_LOT_BUILDING_ID
    );
    assert!(
        state
            .city
            .zone_at(20, 20)
            .is_some_and(|z| z.zone_id() == ZONE_MILITARY)
    );
}

#[test]
fn military_zone_growth_preserves_watered_and_xtrf_like_pe_place_tile() {
    // PE SimulationGrowSpecificZone → PlaceTile: OR XBIT only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-mil-xtrf.sc2", "MilX").expect("blank");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_ARMY);
    stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_MILITARY);
    power_adjacent_building_for_test(&mut city, 20, 19);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_tile_bits(
            x,
            y,
            city.tile_bits_at(x, y)
                .unwrap_or_default()
                .with_piped(true)
                .with_watered(true),
        );
        city.set_traffic_density(x, y, 0x2e);
    }
    let mut state = test_state(city);
    seed_pe_process_crt_until_special_zone_develops(&mut state);
    let expected_developments = expected_special_zone_development_count(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    // Type-7 zone epilogue `XBIT &= 0x0f` clears piped/watered; XTRF untouched.
    assert_eq!(
        state.city.building_at(20, 20),
        MILITARY_PARKING_LOT_BUILDING_ID
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(
            !bits.piped(),
            "type-7 epilogue should clear piped at ({x},{y})"
        );
        assert!(
            !bits.watered(),
            "type-7 epilogue should clear watered at ({x},{y})"
        );
        assert_eq!(
            state.city.traffic_density_at(x, y),
            Some(0x2e),
            "XTRF mutated at ({x},{y})"
        );
    }
}

#[test]
fn military_zone_growth_preserves_xtxt_like_pe_place_tile() {
    // PE SimulationGrowSpecificZone → PlaceTile `0x441f00`: leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-mil-xtxt.sc2", "MilT").expect("blank");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_ARMY);
    stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_MILITARY);
    power_adjacent_building_for_test(&mut city, 20, 19);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_text_overlay(x, y, 0x2a);
    }
    let mut state = test_state(city);
    seed_pe_process_crt_until_special_zone_develops(&mut state);
    let expected_developments = expected_special_zone_development_count(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    assert_eq!(
        state.city.building_at(20, 20),
        MILITARY_PARKING_LOT_BUILDING_ID
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_military_zone_growth_preserves_xtxt_like_pe_place_tile() {
    // Shared special-zone PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-mil-xtxt.sc2", "MacMilT").expect("blank");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_ARMY);
    stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_MILITARY);
    power_adjacent_building_for_test(&mut city, 20, 19);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_text_overlay(x, y, 0x2a);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    seed_pe_process_crt_until_special_zone_develops(&mut state);
    let expected_developments = expected_special_zone_development_count(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    assert_eq!(
        state.city.building_at(20, 20),
        MILITARY_PARKING_LOT_BUILDING_ID
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_military_zone_growth_preserves_xtrf_like_pe_place_tile() {
    let mut city = City::new_blank("/tmp/sc2000-mac-mil-xtrf.sc2", "MacMilX").expect("blank");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_ARMY);
    stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_MILITARY);
    power_adjacent_building_for_test(&mut city, 20, 19);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_traffic_density(x, y, 0x2e);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let expected_developments = expected_special_zone_development_count(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x2e));
    }
}

#[test]
fn military_zone_growth_develops_air_force_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_AIR_FORCE);
    stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_MILITARY);
    for y in 20..=24 {
        city.set_zone(21, y, ZONE_MILITARY);
    }
    // The valid class-0 raw candidate is (21,20), whose power gate checks this
    // cardinal neighbor; (20,19) would be diagonal to that candidate.
    power_adjacent_building_for_test(&mut city, 21, 19);
    let mut state = test_state(city);
    seed_special_zone_targets_for_test(&mut state, &[((21, 20), RUNWAY_BUILDING_ID)]);

    assert!(state.develop_special_zones() > 0);
    // The selected class-0 candidate lays its full five-cell ray. The plan count is
    // one tile, so count its selector side effects separately from planned footprint.
    for y in 20..=24 {
        assert!(
            matches!(
                state.city.building_at(21, y),
                RUNWAY_BUILDING_ID | AIRPORT_RUNWAY_CROSS_BUILDING_ID
            ),
            "class-0 runway ray missing at (21,{y})"
        );
    }
}

#[test]
fn military_zone_growth_develops_navy_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_NAVY);
    stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_MILITARY);
    power_adjacent_building_for_test(&mut city, 20, 19);
    let mut state = test_state(city);
    seed_pe_process_crt_until_special_zone_develops(&mut state);

    let expected_developments = expected_special_zone_development_count(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    // Crane arm needs ALTM water depth; dry 2×2 pads may only run the zone epilogue.
    let crane = building_footprint_from_origin(20, 20, 2)
        .unwrap()
        .into_iter()
        .any(|(x, y)| state.city.building_at(x, y) == MILITARY_CRANE_BUILDING_ID);
    assert!(
        crane
            || building_footprint_from_origin(20, 20, 2)
                .unwrap()
                .iter()
                .all(|&(x, y)| {
                    state
                        .city
                        .zone_at(x, y)
                        .is_some_and(|z| z.zone_id() == ZONE_MILITARY)
                }),
        "navy growth should stamp a crane or keep military zoning"
    );
}

#[test]
fn military_army_growth_thresholds_prefer_hangar_like_43633d() {
    // PE @ `0x436334`: parking_units <= hangar/12 → parking; else hangar1.
    assert_eq!(
        select_army_growth_building(&City::new_blank("/tmp/sc2000-test.sc2", "t").unwrap()),
        MILITARY_PARKING_LOT_BUILDING_ID
    );

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // parking word3=1 → sar/4=0; hangar=0 → still parking (not the old (1+3)>>2=1 hangar path).
    city.pe_military_tile_count[3] = 1;
    assert_eq!(
        select_army_growth_building(&city),
        MILITARY_PARKING_LOT_BUILDING_ID
    );

    // parking word3=4 → sar/4=1; hangar word13=0 → pick hangar1.
    city.pe_military_tile_count[3] = 4;
    assert_eq!(
        select_army_growth_building(&city),
        MILITARY_HANGAR1_BUILDING_ID
    );

    // hangar/12 = 1 with word13=12 → parking_units(1) <= 1 → parking.
    city.pe_military_tile_count[13] = 12;
    assert_eq!(
        select_army_growth_building(&city),
        MILITARY_PARKING_LOT_BUILDING_ID
    );
}

#[test]
fn military_air_force_growth_thresholds_advance_from_runway() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(
        select_military_air_force_growth_building(&city),
        RUNWAY_BUILDING_ID
    );

    // PE reads `0x4ca934` words: 5 runway tiles → runway_units=1; parking/4=0 < 1 → tower.
    city.pe_military_tile_count[1] = 5;
    assert_eq!(
        select_military_air_force_growth_building(&city),
        MILITARY_CONTROL_TOWER_BUILDING_ID
    );
}

#[test]
fn military_navy_growth_thresholds_advance_from_crane() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(
        select_military_navy_growth_building(&city),
        MILITARY_CRANE_BUILDING_ID
    );

    // PE @ `0x43646f`: crane word 10 = 1; cargo/4=0 < 1 → top secret.
    city.pe_military_tile_count[10] = 1;
    assert_eq!(
        select_military_navy_growth_building(&city),
        MILITARY_TOP_SECRET_BUILDING_ID
    );
}

#[test]
fn january_rebuilds_aged_power_plant_when_funded() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(50_000);
    // PE rebuild path requires No Disasters (`0x4CC4D4` / MISC 0x1000).
    city.set_misc_u32_at(0x1000, 1);
    let coal = city.provision_microsim_for_building(207).expect("coal");
    let coal_index = usize::from(coal - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(coal_index, 50)); // next January → 51
    // The PE caller requires the row's XTXT marker before entering the age
    // threshold branch; provisioning XMIC alone is not enough.
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_building(10 + dx, 20 + dy, 207);
            city.set_text_overlay(10 + dx, 20 + dy, coal);
        }
    }

    city.refresh_structure_microsim_yearly(0, 100, 100, 100, 100, 100);
    assert_eq!(
        city.microsim_at(coal_index).map(|r| r.stat0),
        Some(0),
        "PE age>50 with No Disasters + funds >= coal rebuild cost ($4000) resets age"
    );
    assert_eq!(city.city_funds(), 46_000);

    // Broke city: age>50 demolishes the plant (DestroyStructure-lite).
    assert!(city.set_microsim_stat0(coal_index, 50));
    city.set_city_funds(100);
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_building(10 + dx, 20 + dy, 207);
            city.set_text_overlay(10 + dx, 20 + dy, coal);
        }
    }
    city.refresh_structure_microsim_yearly(0, 100, 100, 100, 100, 100);
    assert_eq!(
        city.microsim_at(coal_index).map(|r| r.tile_id),
        Some(0),
        "unaffordable age>50 clears the XMIC slot"
    );
    assert_eq!(city.city_funds(), 100);
    assert!((1..=4).contains(&city.building_at(10, 20)));
    assert_eq!(city.text_overlay_at(10, 20), Some(0));
    assert_eq!(city.building_at(13, 23), 207);
}

#[test]
fn january_writes_eq_le_school_college_cohort_proxies() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);
    // PE occupancy reads restored MISC `0x7c` tables; empty cities stay empty.
    assert_eq!(city.pe_age_cohort_at(1) + city.pe_age_cohort_at(2), 0);
    assert_eq!(city.pe_age_cohort_at(3), 0);
    city.test_seed_pe_age_cohorts_from_pop(10_000);
    assert_eq!(city.pe_age_cohort_at(1) + city.pe_age_cohort_at(2), 1_200);
    assert_eq!(city.pe_age_cohort_at(3), 500);
    assert!(
        city.pe_age_cohort_at(0) > 0 || city.pe_age_cohort_at(5) > 0,
        "test seed fills infant/fertile bands like a loaded city"
    );
}

#[test]
fn january_eq_le_ages_school_cohorts_instead_of_rewriting() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_pe_life_expectancy(80);
    city.test_seed_pe_age_cohorts_from_pop(10_000);
    let school_year0 = city.pe_age_cohort_at(1) + city.pe_age_cohort_at(2);
    let college_year0 = city.pe_age_cohort_at(3);
    assert_eq!(school_year0, 1_200);
    assert_eq!(college_year0, 500);
    assert!(
        city.pe_eq_life_points_at(1) > 0,
        "seed installs parallel 0x4caa70 life points"
    );

    let mut rng = crate::win95_random::Win95CrtRand::new(1);
    city.advance_pe_age_cohorts_yearly(10_000, 0, 0, 0, 100, 100, 100, &mut rng);
    let school_year1 = city.pe_age_cohort_at(1) + city.pe_age_cohort_at(2);
    let college_year1 = city.pe_age_cohort_at(3);
    // Age-forward moves some school-age into college/older; not a full 12%/5% rewrite.
    assert_ne!(
        (school_year1, college_year1),
        (school_year0, college_year0),
        "12 monthly EQ_LE steps must advance instead of reseeding"
    );
    assert!(
        city.pe_eq_births() > 0,
        "fertile births run during the yearly advance"
    );
}

#[test]
fn eq_le_month_wipes_tables_when_city_population_is_zero() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "wipe").expect("blank");
    city.test_seed_pe_age_cohorts_from_pop(20_000);
    assert!(city.pe_age_cohort_at(1) > 0);
    city.set_misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET, 39);
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 61);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 72);
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 20_000);
    city.persist_pe_day22_population_leftover(20_000);
    city.persist_pe_day22_population_leftover(0);
    assert_eq!(city.pe_eq_leftover_births(), 20_000);
    city.advance_pe_eq_le_month(
        0,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    assert_eq!(city.pe_eq_cohort_population(), 0);
    assert!(city.pe_eq_averages_ready());
    assert_eq!(city.pe_eq_workforce_percent(), 39);
    assert_eq!(city.pe_eq_life_average(), 61);
    assert_eq!(city.pe_eq_education_average(), 72);
    assert_eq!(city.pe_eq_leftover_births(), 20_000);
}

#[test]
fn simulation_month_skips_eq_le_until_growth_aggregate_population() {
    let mut state = test_state(reward_unlock_test_city());
    state.no_disasters_enabled = true;
    state.handle_simulation_month_advanced();
    assert!(
        !state.city.pe_eq_averages_ready(),
        "service estimates must not publish the PE EQ/LE summary globals"
    );
    assert_eq!(state.city.pe_eq_cohort_population(), 0);
    assert!(
        state.city.pe_zone_pops_ready(),
        "day-21 publish still marks pZonePops ready"
    );
    let census = pe_zone_pops_scaled_population(live_population_stats_for_city(&state.city));
    assert!(census > 0);
    assert_eq!(
        state.city.pe_dw_city_population(),
        0,
        "day 22 must not overwrite aggregate slot0*10 with census"
    );
    assert_eq!(state.city.pe_eq_leftover_deaths(), 0);
    assert_eq!(
        state.city.rewards_available_mask() & 0b0_1111,
        0,
        "monthly service publication does not grant milestone rewards"
    );
}

#[test]
fn published_growth_population_leftover_immigrates_into_eq_le_same_month() {
    // PE `FUN_00468150` then `FUN_00468D30`: leftover from slot0*10 is consumed
    // in the same phase-21 EQ_LE pass, not by a later census day-22 rewrite.
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.no_disasters_enabled = true;
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
    state.publish_pe_growth_aggregate();
    let scaled = state.city.pe_dw_city_population();
    assert!(scaled > 0);
    assert_eq!(state.city.pe_eq_leftover_deaths(), scaled);

    let _ = state.sim_calendar_day21_demand_and_graphs();

    assert!(
        state.city.pe_eq_cohort_population() > 0,
        "same-month EQ_LE immigrates published leftover into empty tables"
    );
    assert!(state.city.pe_eq_averages_ready());
    assert_eq!(state.city.pe_eq_leftover_deaths(), 0);
}

#[test]
fn january_eq_le_seed_life_points_use_ordinance_le() {
    let mut coverage = City::new_blank("/tmp/sc2000-test.sc2", "cover").expect("blank");
    coverage.set_pe_life_expectancy(35);
    coverage.test_seed_pe_age_cohorts_from_pop(20_000);
    let school = coverage.pe_age_cohort_at(1);
    assert!(school > 0);
    assert_eq!(
        coverage.pe_eq_life_points_at(1),
        school * 85,
        "empty-table seed uses PE ordinance floor 85, not coverage LE"
    );

    let mut ordinances = City::new_blank("/tmp/sc2000-test.sc2", "ord").expect("blank");
    ordinances.set_pe_life_expectancy(35);
    ordinances.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, 0x200 | 0x400 | 0x20);
    ordinances.test_seed_pe_age_cohorts_from_pop(20_000);
    assert_eq!(
        ordinances.pe_eq_life_points_at(1),
        ordinances.pe_age_cohort_at(1) * 100,
        "Anti-Drug + CPR + Smoking Ban seed at ordinance LE 100"
    );
}

#[test]
fn january_eq_le_birth_life_points_follow_hospital_param() {
    assert_eq!(
        crate::city::pe_eq_birth_life_points(10, 70, 10),
        700,
        "full hospital cover → LE * births"
    );
    assert_eq!(
        crate::city::pe_eq_birth_life_points(10, 70, 0),
        350,
        "no hospitals → 35 * births"
    );
    assert_eq!(
        crate::city::pe_eq_hospital_birth_param(4, 100),
        100,
        "4 hospitals ×100% ×25/100"
    );
    assert_eq!(
        crate::city::pe_eq_hospital_birth_param(4, 120),
        120,
        "PE uses the raw funding dword; 120% raises the hospital cap"
    );
    assert_eq!(
        crate::city::pe_eq_hospital_birth_param(1, 0xffff_ff38),
        (-50_i32) as u32,
        "PE imul wraps and signed-divides the resulting dword"
    );
    assert_eq!(
        crate::city::pe_eq_free_clinics_hospital_bump(0, 8_000),
        0,
        "Free Clinics off → no R-tax bump"
    );
    assert_eq!(
        crate::city::pe_eq_free_clinics_hospital_bump(0x40, 8_000),
        20,
        "bit 0x40 adds pBudgetArr[0].iCurrentCosts / 400"
    );
    assert_eq!(crate::city::pe_eq_free_clinics_hospital_bump(0x40, 399), 0);
    assert_eq!(
        crate::city::pe_eq_free_clinics_hospital_bump(0x40, (-400_i32) as u32),
        u32::MAX,
        "PE Free Clinics uses signed IDIV before the wrapped hospital-cap add"
    );
    assert_eq!(crate::city::pe_eq_school_aux_param(2, 120), 36);
    assert_eq!(crate::city::pe_eq_college_aux_param(1, 120), 60);
    assert_eq!(
        crate::city::pe_eq_birth_life_points(0x8000_0000, 85, 0x8000_0000),
        0x8000_0000,
        "PE newborn life-point multiplies/adds wrap at dword width"
    );
    assert_eq!(crate::city::pe_eq_school_aux_param(2, 100), 30);
    assert_eq!(crate::city::pe_eq_college_aux_param(1, 100), 50);
    assert_eq!(
        crate::city::pe_eq_education_life_penalty(1_000, 100, 0),
        3,
        "pollution/pop capped at 3"
    );
    assert_eq!(
        crate::city::pe_eq_education_life_penalty(151, 100, 5),
        1,
        "PE denom is city_pop + 10*slot7 + 1 (100+50+1)"
    );
    assert_eq!(
        crate::city::pe_eq_education_life_penalty(40, 100, 0),
        0,
        "below unit quotient stays 0"
    );
    assert_eq!(crate::city::pe_eq_ordinance_life_expectancy(0), 85);
    assert_eq!(crate::city::pe_eq_ordinance_life_expectancy(0x200), 90);
    assert_eq!(
        crate::city::pe_eq_ordinance_life_expectancy(0x200 | 0x400 | 0x20),
        100
    );
    assert!(!crate::city::pe_eq_ordinance_pro_reading(0));
    assert!(crate::city::pe_eq_ordinance_pro_reading(0x100));
    assert_eq!(
        crate::city::pe_eq_birth_life_points(10, 85, 10),
        850,
        "ordinance LE 85 with full hospitals → 85 * births"
    );

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "births").expect("blank");
    city.set_pe_life_expectancy(70);
    city.test_seed_pe_age_cohorts_from_pop(12_000);
    let mut rng = crate::win95_random::Win95CrtRand::new(1);
    city.advance_pe_age_cohorts_yearly(12_000, 0, 8, 0, 100, 100, 100, &mut rng);
    assert!(
        city.pe_eq_births() > 0,
        "fertile band should produce births"
    );
}

#[test]
fn january_eq_le_free_clinics_boost_newborn_life_from_r_tax_base() {
    let seed = |name: &str, clinics: bool| {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", name).expect("blank");
        city.set_pe_life_expectancy(70);
        city.set_pe_attitude_tax_bases([40_000, 0, 0]);
        if clinics {
            city.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, 0x40);
        }
        city.test_seed_pe_age_cohorts_from_pop(20_000);
        city
    };
    let mut bare = seed("clinic0", false);
    let mut clinics = seed("clinic1", true);
    let mut rng_a = crate::win95_random::Win95CrtRand::new(3);
    let mut rng_b = crate::win95_random::Win95CrtRand::new(3);
    // 0 hospitals: only Free Clinics `[0x4cc4cc]+0x60 / 400` supplies esp+0x48.
    bare.advance_pe_age_cohorts_yearly(20_000, 0, 0, 0, 100, 100, 100, &mut rng_a);
    clinics.advance_pe_age_cohorts_yearly(20_000, 0, 0, 0, 100, 100, 100, &mut rng_b);
    assert_eq!(bare.pe_eq_births(), clinics.pe_eq_births());
    assert!(
        clinics.pe_eq_life_points_at(0) > bare.pe_eq_life_points_at(0),
        "clinics life={} bare life={}",
        clinics.pe_eq_life_points_at(0),
        bare.pe_eq_life_points_at(0)
    );
}

#[test]
fn day21_eq_le_reads_and_preserves_raw_funding_without_a_budget_phase() {
    let seeded_state = |name: &str, health_funding: u32| {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", name).expect("blank city");
        stamp_complete_building_for_test(&mut city, 2, 2, 209, 3);
        city.test_seed_pe_age_cohorts_from_pop(20_000);
        city.persist_pe_day22_population_leftover(20_000);
        set_budget_funding_for_test(&mut city, 2, health_funding);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.no_disasters_enabled = true;
        // The city-name seed differs between fixtures; share the PE CRT seed
        // so this comparison isolates the hospital funding input.
        state.pe_process_crt = crate::win95_random::Win95CrtRand::new(1);
        state
    };

    let mut normal = seeded_state("birth-funding-100", 100);
    let mut overfunded = seeded_state("birth-funding-120", 120);
    normal.sim_calendar_day21_demand_and_graphs();
    overfunded.sim_calendar_day21_demand_and_graphs();

    let births = normal.city.pe_eq_births();
    assert!(
        births > 30,
        "fixture should exceed both hospital caps: {births}"
    );
    assert_eq!(overfunded.city.pe_eq_births(), births);
    assert_eq!(
        overfunded.city.pe_eq_life_points_at(0) - normal.city.pe_eq_life_points_at(0),
        5 * (85 - 35),
        "one hospital increases the birth cap from 25 to 30 at raw funding 120"
    );
    assert_eq!(
        budget_service_funding_raw_for_city(&overfunded.city, 2),
        120,
        "the day-21 worker does not own the phase-zero budget rebuild"
    );
}

#[test]
fn january_eq_le_rebalance_holds_population_after_deaths_and_births() {
    assert_eq!(crate::city::pe_eq_immigrant_chunk(0), 1);
    assert_eq!(crate::city::pe_eq_immigrant_chunk(16), 2);
    assert_eq!(crate::city::pe_eq_immigrant_chunk(160), 11);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "rebal").expect("blank");
    city.test_seed_pe_age_cohorts_from_pop(20_000);
    // Seeded buckets survive ordinance LE 85; put people in the oldest divisor so
    // deaths run and leftover rebalance has work.
    city.test_add_pe_eq_cohort(19, 2_000);
    let seeded = city.pe_eq_cohort_population();
    assert_eq!(seeded, 22_000);

    city.advance_pe_age_cohorts_yearly(
        22_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    assert_eq!(
        city.pe_eq_cohort_population(),
        seeded,
        "0x469945 rebalance holds EQ_LE pop after deaths/births"
    );
    assert!(city.pe_eq_deaths() > 0);
    assert!(city.pe_eq_births() > 0);
}

#[test]
fn january_eq_le_pro_reading_preserves_cohort_population() {
    let mut bare = City::new_blank("/tmp/sc2000-test.sc2", "aux0").expect("blank");
    bare.set_pe_life_expectancy(70);
    bare.test_seed_pe_age_cohorts_from_pop(12_000);
    let mut reading = City::new_blank("/tmp/sc2000-test.sc2", "aux1").expect("blank");
    reading.set_pe_life_expectancy(70);
    reading.test_seed_pe_age_cohorts_from_pop(12_000);
    reading.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, 0x100);

    let mut rng_a = crate::win95_random::Win95CrtRand::new(7);
    let mut rng_b = crate::win95_random::Win95CrtRand::new(7);
    bare.advance_pe_age_cohorts_yearly(12_000, 0, 0, 0, 100, 100, 100, &mut rng_a);
    reading.advance_pe_age_cohorts_yearly(12_000, 0, 0, 0, 100, 100, 100, &mut rng_b);

    assert!(
        reading.pe_eq_cohort_population() == bare.pe_eq_cohort_population(),
        "Pro Reading changes the auxiliary-score branch, not population movement"
    );
}

#[test]
fn pe_opinion_poll_uses_unemployment_weight_slot() {
    let mut low = City::new_blank("/tmp/sc2000-test.sc2", "unemp0").expect("blank city");
    low.provision_microsim_for_building(243).expect("mayor");
    low.set_pe_unemployment_percent(0);
    low.set_pe_dw_city_population(10_000);
    let mut low_rng = Win95CrtRand::new(0x4725_1001);
    low.run_pe_opinion_poll_with_rng(100, &mut low_rng);
    let low_unemp = low.pe_mayor_approval();

    let mut high = City::new_blank("/tmp/sc2000-test.sc2", "unemp100").expect("blank city");
    let mayor = high.provision_microsim_for_building(243).expect("mayor");
    let mayor_index = usize::from(mayor - MIN_SIM_TEXT_ENTRIES);
    high.set_pe_unemployment_percent(100);
    high.set_pe_dw_city_population(10_000);
    let mut high_rng = Win95CrtRand::new(0x4725_1001);
    high.run_pe_opinion_poll_with_rng(100, &mut high_rng);
    let high_unemp = high.pe_mayor_approval();
    // The extra unemployment weight changes the CRT remainder divisor, so a
    // particular sequence of 100 draws need not be monotonically ordered.
    assert_eq!((low_unemp, high_unemp), (9, 13));
    high.refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);
    assert_eq!(
        high.microsim_at(mayor_index).map(|r| r.stat2),
        Some(high.pe_mayor_approval())
    );
}

#[test]
fn january_water_microsim_uses_pe_coverage_percent() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let water = city.provision_microsim_for_building(244).expect("water");
    let water_index = usize::from(water - MIN_SIM_TEXT_ENTRIES);
    city.set_pe_water_coverage_percent(40);
    let mut rng = Win95CrtRand::new(Win95CrtRand::DEFAULT_SEED);
    city.refresh_structure_microsim_yearly_with_rng(
        10_000,
        100,
        100,
        100,
        100,
        100,
        &mut rng,
        &mut Win95Lfsr::new(Win95Lfsr::DEFAULT_STATE),
    );
    let record = city.microsim_at(water_index).expect("water row");
    assert!(
        (40..48).contains(&record.stat0),
        "PE stat0 = coverage+(rand&7); got {}",
        record.stat0
    );
}

#[test]
fn january_fills_school_hospital_and_arco_occupancy() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 7_000);
    stamp_complete_building_for_test(&mut city, 2, 2, SCHOOL_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut city, 8, 2, HOSPITAL_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut city, 14, 2, 251, 4);

    let school = city.provision_microsim_for_building(214).expect("school");
    let school_index = usize::from(school - MIN_SIM_TEXT_ENTRIES);
    let hospital = city.provision_microsim_for_building(209).expect("hospital");
    let hospital_index = usize::from(hospital - MIN_SIM_TEXT_ENTRIES);
    let arco = city.provision_microsim_for_building(251).expect("arco");
    let arco_index = usize::from(arco - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(arco_index, 12));

    city.test_seed_pe_age_cohorts_from_pop(7_000);
    let arco_residents = city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    // PE school @ `0x44e978`: restored MISC `0x7c` pool then `FUN_0044f7d0` ÷20; teachers÷100.
    // PE hospital @ `0x44e662`: patients=pop/(n*25) then ÷30; doctors÷120.
    assert_eq!(
        city.microsim_at(school_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((12, 350, 57, 25)),
        "PE school: min(pop/20, 12% pool+jitter) students; A-ratio grade"
    );
    assert_eq!(
        city.microsim_at(hospital_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((12, 233, 58, 50)),
        "PE hospital: min(pop/30, pop/25+jitter) patients; A-range grade"
    );
    assert_eq!(
        city.microsim_at(arco_index).map(|r| r.stat2),
        Some(352),
        "PE arco: site-count divisor gives 350 residents, then the LFSR adds 2"
    );
    assert_eq!(arco_residents, 352);
}

#[test]
fn leftover_eq_le_feeds_january_school_and_college_occupancy() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 2, 2, SCHOOL_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut city, 8, 2, 217, 4);
    let school = city.provision_microsim_for_building(214).expect("school");
    let school_index = usize::from(school - MIN_SIM_TEXT_ENTRIES);
    let college = city.provision_microsim_for_building(217).expect("college");
    let college_index = usize::from(college - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(
        city.microsim_at(school_index).map(|r| r.stat1),
        Some(0),
        "fresh school stays empty until leftover fills EQ_LE"
    );

    city.persist_pe_day22_population_leftover(10_000);
    city.advance_pe_eq_le_month(
        10_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    let school_pool = city.pe_age_cohort_at(1) + city.pe_age_cohort_at(2);
    let college_pool = city.pe_age_cohort_at(3);
    assert!(
        school_pool > 500,
        "leftover immigration must reach school-age buckets 1+2, got {school_pool}"
    );
    assert!(
        college_pool > 333,
        "leftover immigration must reach college bucket 3, got {college_pool}"
    );

    city.refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);
    assert_eq!(
        city.microsim_at(school_index).map(|r| r.stat1),
        Some(500),
        "PE school students: min(dwCityPopulation/20, EQ_LE[1]+[2]+jitter)"
    );
    assert_eq!(
        city.microsim_at(college_index).map(|r| r.stat1),
        Some(333),
        "PE college students: min(dwCityPopulation/30, EQ_LE[3]+jitter)"
    );
}

#[test]
fn january_hospital_patients_divides_by_hospital_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 2, 2, HOSPITAL_BUILDING_ID, 3);
    stamp_complete_building_for_test(&mut city, 8, 2, HOSPITAL_BUILDING_ID, 3);
    let hospital = city.provision_microsim_for_building(209).expect("hospital");
    let hospital_index = usize::from(hospital - MIN_SIM_TEXT_ENTRIES);

    let mut rng = Win95CrtRand::new(Win95CrtRand::DEFAULT_SEED);
    city.refresh_structure_microsim_yearly_with_rng(
        7_000,
        100,
        100,
        100,
        100,
        100,
        &mut rng,
        &mut Win95Lfsr::new(Win95Lfsr::DEFAULT_STATE),
    );
    let record = city.microsim_at(hospital_index).expect("hospital row");
    // PE: two 3×3 hospitals → (18/9)*25=50; 7000/50=140+jitter, below the ÷30 cap of 233.
    assert_eq!(record.stat1, 149, "pop / (2*25) patients + CRT jitter");
}

#[test]
fn january_police_crimes_divide_by_police_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 9_496);
    stamp_complete_building_for_test(&mut city, 2, 2, 210, 3);
    let police = city.provision_microsim_for_building(210).expect("police");
    let police_index = usize::from(police - MIN_SIM_TEXT_ENTRIES);

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
    let record = city.microsim_at(police_index).expect("police row");
    assert_eq!(record.stat2, 9_496 / 9, "dwCityCrime / police tiles");
    assert_eq!(record.stat3, 9_496 / 9 / 5 + (41 & 0xf));
}

#[test]
fn january_police_uses_live_zero_crime_total_over_stale_saved_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 9_496);
    stamp_complete_building_for_test(&mut city, 2, 2, 210, 3);
    let police = city.provision_microsim_for_building(210).expect("police");
    let police_index = usize::from(police - MIN_SIM_TEXT_ENTRIES);

    // The January publisher has authoritatively rebuilt the live PE BSS total
    // to zero, while the previous serialized snapshot is still nonzero.
    city.set_pe_attitude_demand_inputs(0, 0, 0, 0);
    assert_eq!(city.misc_u32_at(MISC_CITY_CRIME_OFFSET), Some(9_496));
    assert_eq!(city.pe_attitude_crime_total(), 0);

    city.refresh_structure_microsim_yearly(55_860, 100, 100, 100, 100, 100);
    let record = city.microsim_at(police_index).expect("police row");
    assert_eq!(record.stat2, 0, "PE reads the live zero crime total");
}

#[test]
fn january_year_roll_passes_raw_saved_service_funding() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 2, 2, 210, 3);
    stamp_complete_building_for_test(&mut city, 10, 2, 211, 3);
    stamp_complete_building_for_test(&mut city, 18, 2, 209, 3);
    stamp_complete_building_for_test(&mut city, 26, 2, 214, 3);
    stamp_complete_building_for_test(&mut city, 34, 2, 217, 4);
    city.persist_pe_day22_population_leftover(100_000);
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        31,
    );
    let police = city.provision_microsim_for_building(210).expect("police");
    let police_index = usize::from(police - MIN_SIM_TEXT_ENTRIES);
    let fire = city.provision_microsim_for_building(211).expect("fire");
    let fire_index = usize::from(fire - MIN_SIM_TEXT_ENTRIES);
    let hospital = city.provision_microsim_for_building(209).expect("hospital");
    let hospital_index = usize::from(hospital - MIN_SIM_TEXT_ENTRIES);
    let school = city.provision_microsim_for_building(214).expect("school");
    let school_index = usize::from(school - MIN_SIM_TEXT_ENTRIES);
    let college = city.provision_microsim_for_building(217).expect("college");
    let college_index = usize::from(college - MIN_SIM_TEXT_ENTRIES);
    for budget_index in 0..5 {
        set_budget_funding_for_test(&mut city, budget_index, 120);
    }

    let mut state = test_state(city);
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);

    let police = state.city.microsim_at(police_index).expect("police row");
    assert_eq!(
        police.stat0, 120,
        "January stores the raw police funding byte"
    );
    assert_eq!(
        police.stat1, 240,
        "January passes raw police funding to the selector"
    );
    let fire = state.city.microsim_at(fire_index).expect("fire row");
    assert_eq!(fire.stat0, 120, "January stores the raw fire funding byte");
    assert_eq!(
        fire.stat1, 60,
        "January passes raw fire funding to the selector"
    );
    let hospital = state
        .city
        .microsim_at(hospital_index)
        .expect("hospital row");
    assert_eq!(hospital.stat3, 60, "January passes raw health funding");
    let school = state.city.microsim_at(school_index).expect("school row");
    assert_eq!(school.stat3, 30, "January passes raw school funding");
    let college = state.city.microsim_at(college_index).expect("college row");
    assert_eq!(college.stat3, 120, "January passes raw college funding");
}

#[test]
fn military_zone_growth_places_missile_silos() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_MISSILE_SILOS);
    stamp_zone_footprint_for_test(&mut city, (20, 20), 3, ZONE_MILITARY);
    power_adjacent_building_for_test(&mut city, 20, 19);
    let mut state = test_state(city);
    seed_pe_process_crt_until_special_zone_develops(&mut state);

    let expected_developments = expected_special_zone_development_count(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    assert_eq!(state.city.building_at(20, 20), MILITARY_SILO_BUILDING_ID);
    assert_eq!(
        state.city.underground_at(20, 20),
        UNDER_TILE_GREY_BOX_1,
        "missile silo growth should stamp underground footprint"
    );
}

#[test]
fn military_zone_growth_waits_for_active_base_type() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_MILITARY);
    power_adjacent_building_for_test(&mut city, 20, 19);
    let mut state = test_state(city);

    let expected_developments = expected_special_zone_development_count(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn underfunded_roads_decay_to_rubble_on_growth_tick() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // Seed full budget MISC so funding writes stick.
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_text_overlay(20, 20, 0x55);
    let mut state = test_state(city);
    let before = state.pe_process_crt_seed();

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_ne!(
        state.pe_process_crt_seed(),
        before,
        "transport budget CRT rolls should advance the shared process stream"
    );
    assert!(
        is_rubble_tile(state.city.building_at(20, 20)),
        "expected rubble, got {}",
        state.city.building_at(20, 20)
    );
    // PE PlaceTile rubble keeps XTXT (no invent clear).
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x55));
}

#[test]
fn win95_surface_upkeep_uses_raw_funding_word_and_keeps_crt_draw() {
    let mut city =
        City::new_blank("/tmp/sc2000-raw-road-budget.sc2", "RawRoad").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 120);
    let road = network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT);
    city.set_building(20, 20, road);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(1);
    let mut expected_rng = Win95CrtRand::new(1);
    assert!(!expected_rng.roll_pe_underfunding_decay(120));

    assert_eq!(state.decay_surface_transport_at_for_test(20, 20), 0);
    assert_eq!(state.city.building_at(20, 20), road);
    assert_eq!(
        state.pe_process_crt_seed(),
        expected_rng.seed(),
        "PE reads raw funding 120 and consumes rand despite not collapsing the road"
    );
}

#[test]
fn win95_bridge_upkeep_compares_wind_against_raw_funding_word() {
    let mut city =
        City::new_blank("/tmp/sc2000-raw-bridge-budget.sc2", "RawBridge").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 120);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 60);
    city.set_building(20, 20, 87);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(1);
    let mut expected_rng = Win95CrtRand::new(1);
    assert!(!expected_rng.roll_pe_bridge_underfunding_decay(120, 60));

    // First CRT result is 41: PE checks 60 + (41 % 50) = 101 against raw 120.
    // Clamping to 100 would incorrectly destroy this bridge.
    assert_eq!(state.decay_surface_transport_at_for_test(20, 20), 0);
    assert_eq!(state.city.building_at(20, 20), 87);
    assert_eq!(state.pe_process_crt_seed(), expected_rng.seed());
}

#[test]
fn underfunded_roads_decay_on_growth_tick_when_zone_zero() {
    // PE @ `0x435988`: XZON nibble 0 → DoBudgetOvergroundTransportCheck. Live growth
    // wrongly gated on `zone_at.is_none()`, which blank/loaded cities never hit.
    let mut city = City::new_blank("/tmp/sc2000-zone0-budget.sc2", "Zone0Bd").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    assert_eq!(city.zone_at(20, 20).map(|z| z.zone_id()), Some(0));
    let mut state = test_state(city);
    assert!(
        state.decay_surface_via_growth_zone_gate_for_test(20, 20) >= 1,
        "zone-0 road should reach DoBudget through the growth XZON gate"
    );
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
}

#[test]
fn underfunded_roads_skip_growth_tick_when_zone_nonzero() {
    // PE @ `0x43598e`: non-zero XZON nibble jumps past overground DoBudget.
    let mut city = City::new_blank("/tmp/sc2000-zone1-budget.sc2", "Zone1Bd").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    for (x, y) in sim_calendar_growth_phase_tiles(0, 0) {
        city.set_zone(x, y, 7);
    }
    city.set_zone(20, 20, 1);
    let road = network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT);
    city.set_building(20, 20, road);
    let mut state = test_state(city);
    assert_eq!(state.decay_surface_via_growth_zone_gate_for_test(20, 20), 0);
    state.pe_process_lfsr = Win95Lfsr::new(64);
    state.sim_calendar_one_growth_tick(0, 0);
    assert_eq!(state.city.building_at(20, 20), road);
}

#[test]
fn underfunded_surface_budget_lfsr_probe_is_random_and_7f() {
    // PE @ `0x43599e`: RandomAnd7F (`0x402b3f` → `state & 0x7f`), not `% 128`.
    // Admit when rem == 0; refuse when rem != 0 without PlaceTile.
    let road = network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT);
    for (seed, expect_rubble) in [(64u16, true), (1u16, false)] {
        let mut city = City::new_blank("/tmp/sc2000-and7f.sc2", "And7f").expect("blank");
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_test(&mut city, index, 100);
        }
        set_budget_funding_for_test(&mut city, 5, 0);
        city.set_building(20, 20, road);
        let mut state = test_state(city);
        state.pe_process_lfsr = Win95Lfsr::new(seed);
        let mut probe = Win95Lfsr::new(seed);
        let admit = probe.random_mask_7f() == 0;
        assert_eq!(admit, expect_rubble, "seed {seed}");
        let decayed = state.decay_surface_transport_at_for_test_noforce(20, 20);
        if expect_rubble {
            assert!(decayed >= 1);
            assert!(is_rubble_tile(state.city.building_at(20, 20)));
        } else {
            assert_eq!(decayed, 0);
            assert_eq!(state.city.building_at(20, 20), road);
        }
    }
}

#[test]
fn underfunded_onramp_uses_road_placetile_not_highway_2x2() {
    // PE DoBudgetRoadCheck @ `0x4359e0`: XBLD `0x5d..=0x60` → single-tile CRT PlaceTile
    // + XBIT `&=0x7f`, not highway 2×2 PlaceTile @ `0x435add`.
    let mut city = City::new_blank("/tmp/sc2000-onramp-budget.sc2", "OnrampBd").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(20, 20, 0x5d);
    city.set_building(21, 21, 6);
    city.set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
    let mut state = test_state(city);
    let before = state.pe_process_crt_seed();
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.building_at(21, 21), 6);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());
    assert_ne!(state.pe_process_crt_seed(), before);
}

#[test]
fn underfunded_highway_0x4b_uses_road_placetile_not_2x2() {
    // PE DoBudgetRoadCheck @ `0x4359cc`: XBLD `0x4b`/`0x4c` → road PlaceTile arm,
    // not highway 2×2 @ `0x435add`.
    let mut city = City::new_blank("/tmp/sc2000-hwy4b-budget.sc2", "Hwy4bBd").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(20, 20, 0x4b);
    city.set_building(21, 21, 6);
    city.set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.building_at(21, 21), 6);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn underfunded_highway_0x4d_uses_rail_placetile_not_2x2() {
    // PE DoBudgetRailCheck @ `0x435a20`: XBLD `0x4d`/`0x4e` → rail PlaceTile arm
    // (`0x435ce4`), not highway 2×2 @ `0x435add`.
    let mut city = City::new_blank("/tmp/sc2000-hwy4d-budget.sc2", "Hwy4dBd").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 8, 0);
    city.set_building(20, 20, 0x4d);
    city.set_building(21, 21, 6);
    city.set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.building_at(21, 21), 6);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn underfunded_crossover_0x45_uses_road_budget_not_rail() {
    // PE DoBudgetRoadCheck @ `0x4359bc`–`0x4359c6`: XBLD `0x3f..=0x46` → road
    // PlaceTile arm before rail `0x435a06`. Road×rail `0x45` takes road funding.
    let mut city = City::new_blank("/tmp/sc2000-xo45-budget.sc2", "Xo45Bd").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 8, 0);
    city.set_building(20, 20, 0x45);
    let mut state = test_state(city);
    assert_eq!(state.decay_transport_budget_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), 0x45);

    set_budget_funding_for_test(&mut state.city, 8, 100);
    set_budget_funding_for_test(&mut state.city, 5, 0);
    state.city.set_building(21, 21, 6);
    state
        .city
        .set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.building_at(21, 21), 6);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn underfunded_power_road_0x43_uses_road_budget() {
    // PE DoBudgetRoadCheck @ `0x4359bc`–`0x4359c6`: XBLD `0x43`/`0x44` (power×road)
    // are inside `0x3f..=0x46` → road PlaceTile. Not a power-only no-budget tile.
    let mut city = City::new_blank("/tmp/sc2000-pwr43-budget.sc2", "Pwr43Bd").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(20, 20, 0x43);
    city.set_building(21, 21, 6);
    city.set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.building_at(21, 21), 6);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());
}

#[test]
fn growth_tick_does_not_advance_shared_process_lfsr_for_blank_city() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    let before = state.pe_process_lfsr_state();
    state.sim_calendar_one_growth_tick(0, 0);
    assert_eq!(
        state.pe_process_lfsr_state(),
        before,
        "blank XZON cells exit before the PE subway budget check at 0x43680b"
    );
}

#[test]
fn growth_tick_aggregate_credits_visited_abandoned_xbld_not_plan_building() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, 0x8A);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = true;

    state.sim_calendar_one_growth_tick(0, 0);

    assert_ne!(
        state.city.building_at(20, 20),
        0x8A,
        "abandoned tile should redevelop on the growth lattice"
    );
    assert_eq!(
        state.pe_growth_aggregate.slots()[7],
        pe_growth_aggregate_weight(1),
        "PE credits flag-2 visited XBLD 0x8A into slot 7 before the roll"
    );
    assert_eq!(
        state.pe_growth_aggregate.slots()[1],
        0,
        "normal category slot must not receive the abandoned pre-roll weight"
    );
}

#[test]
fn growth_tick_aggregate_skips_vacant_visited_xbld() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = true;

    state.sim_calendar_one_growth_tick(0, 0);

    assert!(
        state.city.building_at(20, 20) != 0,
        "vacant residential should develop"
    );
    assert_eq!(
        *state.pe_growth_aggregate.slots(),
        [0; 8],
        "vacant pre-roll XBLD must not credit the chosen plan building"
    );
}

#[test]
fn growth_tick_aggregate_credits_eligible_tile_before_nonflat_redevelopment() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zoned_building_footprint_for_test(&mut city, (20, 20), 2, ZONE_COMMERCIAL_DENSE, 0xAA);
    // Rotation zero's XZONCornerCheck source is SW, visited in phase (0, 1).
    stamp_transport_access_for_test(&mut city, 20, 22);
    power_adjacent_building_for_test(&mut city, 19, 21);
    city.set_altitude(21, 20, 1, 0);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    let mut state = test_state(city);

    state.sim_calendar_one_growth_tick(0, 1);

    let rebuilt = state.city.building_at(20, 20);
    assert!(COMMERCIAL_2X2_BUILDINGS.contains(&rebuilt));
    assert_eq!(state.city.building_at(21, 20), rebuilt);
    let level = pe_growth_level_for_xbld(0xAA);
    let weight = pe_growth_aggregate_weight(level);
    assert!(weight > 0, "fixture must use a weighted XBLD level");
    assert_eq!(
        state.pe_growth_aggregate.slots()[7],
        weight,
        "PE records the eligible flag-2 tile before replacing the footprint"
    );
    assert_eq!(
        state.pe_growth_aggregate.slots()[4],
        0,
        "flag-2 tiles credit slot 7 rather than a normal category slot"
    );
}

#[test]
fn underfunded_road_clears_powerable_keeps_powered_like_pe_435dd6() {
    // PE @ `0x435dd6`: XBIT `&= 0x7f` clears powerable only — powered `0x40` stays.
    // Contrast DestroyBuilding bridge `&= 0x3d` which clears both.
    let mut city = City::new_blank("/tmp/sc2000-road-powered.sc2", "RoadPwr").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0)
            .with_piped(true)
            .with_watered(true)
            .with_powered(true)
            .with_powerable(true),
    );
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert!(bits.powered(), "road PlaceTile must keep powered");
    assert!(!bits.powerable(), "road PlaceTile must clear powerable");
}

#[test]
fn underfunded_road_skips_invent_budget_misc_refresh() {
    // PE road PlaceTile → `XBIT &= 0x7f` → `0x435dda`: no invent MISC refresh.
    let mut city = City::new_blank("/tmp/sc2000-road-noref.sc2", "RoadNoRef").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    let offset = budget_service_misc_offset(5, BUDGET_SERVICE_CURRENT_FIELD).expect("road");
    let mut values = city.misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
    values[BUDGET_SERVICE_CURRENT_FIELD] = 0xD44;
    city.set_misc_u32_values(offset, &values);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(
        state.city.misc_u32_at(offset),
        Some(0xD44),
        "road PlaceTile must not invent-refresh budget CURRENT"
    );
}

#[test]
fn underfunded_road_decay_preserves_watered_and_xtrf() {
    // PE DoBudgetRoadCheck PlaceTile rubble: no invent water/XTRF wipe.
    let mut city = City::new_blank("/tmp/sc2000-road-watered.sc2", "RoadWater").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x22);
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x22));
}

#[test]
fn mac68k_underfunded_road_decay_preserves_xtrf() {
    // Shared DoBudgetRoadCheck rubble path leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-road-xtrf.sc2", "MacRoadX").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 5, 0);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_traffic_density(20, 20, 0x22);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x22));
}

#[test]
fn fully_funded_roads_do_not_decay_on_growth_tick() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    let road = network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT);
    city.set_building(20, 20, road);
    let mut state = test_state(city);

    assert_eq!(state.decay_transport_budget_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), road);
}

#[test]
fn underfunded_rails_decay_to_rubble_on_growth_tick() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 8, 0);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Rail, NETWORK_TOP | NETWORK_BOTTOM),
    );
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
}

#[test]
fn underfunded_power_rail_0x47_uses_rail_placetile() {
    // PE DoBudgetRailCheck @ `0x435a06`–`0x435a0a`: XBLD `0x45..=0x48` → rail
    // PlaceTile @ `0x435ce4`. Road band @ `0x4359bc` already took `0x45`/`0x46`,
    // so only power×rail `0x47`/`0x48` still reach the rail arm (plus `0x4d`/`0x4e`).
    let mut city = City::new_blank("/tmp/sc2000-pwr-rail-47.sc2", "PwrRail47").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 8, 0);
    city.set_building(20, 20, 0x47);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0).with_powered(true).with_powerable(true),
    );
    city.set_text_overlay(20, 20, 0x55);
    let mut state = test_state(city);
    assert_eq!(transport_surface_budget_index(0x47), Some(8));
    assert_eq!(transport_surface_budget_index(0x48), Some(8));
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    // Rail PlaceTile-only — XTXT kept; XBIT &=0x7f clears powerable, keeps powered.
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x55));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.powered());
    assert!(!bits.powerable());
}

#[test]
fn underfunded_rail_decay_preserves_watered_and_xtrf() {
    // PE DoBudgetRailCheck rubble → DestroyStructure sibling: no invent water/XTRF wipe.
    let mut city = City::new_blank("/tmp/sc2000-rail-watered.sc2", "RailWater").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 8, 0);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Rail, NETWORK_TOP | NETWORK_BOTTOM),
    );
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x11);
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x11));
}

#[test]
fn mac68k_underfunded_rail_decay_preserves_xtrf() {
    // Shared DoBudgetRailCheck rubble path leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-rail-xtrf.sc2", "MacRailX").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 8, 0);
    city.set_building(
        20,
        20,
        network_building_for_mask(NetworkKind::Rail, NETWORK_TOP | NETWORK_BOTTOM),
    );
    city.set_traffic_density(20, 20, 0x11);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x11));
}

#[test]
fn underfunded_bridges_collapse_with_wind_and_clear_water_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_terrain(20, 20, 0x10);
    city.set_building(20, 20, 87);
    city.set_text_overlay(20, 20, 0x55);
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    // PE queues newspaper story 39 + AutoGoto — no invent status string.
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(39));
    assert_ne!(state.query_line, "Bridge Collapsed");
}

#[test]
fn fully_funded_bridges_ignore_high_wind() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    city.set_misc_u32_at(MISC_WIND_OFFSET, 99);
    city.set_terrain(20, 20, 0x10);
    city.set_building(20, 20, 87);
    let mut state = test_state(city);

    assert_eq!(state.decay_transport_budget_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), 87);
}

#[test]
fn mac68k_extent2_bridge_fallback_keeps_rubble_and_place_tile_census() {
    let mut city = City::new_blank("/tmp/sc2000-mac-bridge-extent2-fallback.sc2", "MacBridge2")
        .expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 0);
    city.set_mac_state_misc_u32_at(0x64, 255);
    city.set_building(20, 20, 0x6a);
    // Force export 180 into its XZON-high-marker branch with no marked
    // neighboring tile, so the classifier returns -1 and selects fallback.
    city.set_zone(20, 20, 0xf0);
    for (x, y) in [(20, 20), (21, 20), (21, 21), (20, 21)] {
        city.set_tile_bits(x, y, TileBits::new(0));
    }
    city.set_text_overlay(21, 21, 49);
    city.set_label(49, "separate XTXT marker");

    let seed = 0x1357_2468;
    let mut expected_rng = MacToolboxRng::new(seed);
    for _ in 0..17 {
        expected_rng.random_u16();
    }
    let mut expected_tiles = Vec::new();
    for (x, y) in [(20, 21), (20, 20), (21, 21), (21, 20)] {
        expected_tiles.push((x, y, (expected_rng.random_u16() & 3) as u8 + 1));
    }
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    for (x, y, rubble) in expected_tiles {
        assert_eq!(city.building_at(x, y), rubble, "cell ({x},{y})");
    }
    assert_eq!(city.text_overlay_at(21, 21), Some(0));
    assert_eq!(city.label_at(49), Some(""));
    assert_eq!(city.mac_xbld_tile_count(0x6a), 0);
    assert_eq!(
        (1..=4)
            .map(|tile| city.mac_xbld_tile_count(tile))
            .sum::<u16>(),
        4,
        "PlaceTile census increments describe the retained rubble tiles"
    );
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_late_xund_scan_gate_skips_unzoned_and_ordinary_rci_cells() {
    // CODE 8 `export_241` routes category 0 through upkeep/actor checks and
    // categories 1..=6 through RCI growth; both paths branch to the scan tail
    // without reaching the late XUND funding gate at `0x36bc6`.
    for (zone, building) in [(0x00, 0x5d), (0xf1, 0x5d), (0xf1, 0x6d)] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-late-xund-dispatch-gates.sc2",
            "LateXundDispatch",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 10, 0);
        city.set_zone(20, 20, zone);
        city.set_building(20, 20, building);
        city.set_underground(20, 20, 0x23);

        let seed = 0x4a31_58c7;
        let mut rng = MacToolboxRng::new(seed);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        assert_eq!(city.building_at(20, 20), building, "XZON={zone:#04x}");
        assert_eq!(
            rng.seed(),
            MacToolboxRng::new(seed).seed(),
            "XZON={zone:#04x}"
        );
    }
}

#[test]
fn mac_generic_growth_edge_uses_saved_selector_not_display_compass() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-growth-edge-saved-rotation.sc2",
        "GrowthEdge",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.saved_rotation = 3;
    city.set_view_rotation(0);
    city.set_zone(50, 50, 0x18); // marker 0x10 plus zone 8
    let mut rng = MacToolboxRng::new(0x81);

    super::super::budget::mac_budget_place_generic_rotated_edge(&mut city, 50, 50, &mut rng);

    assert_ne!(city.building_at(50, 51), 0);
    assert_ne!(city.building_at(50, 50), 0);
    assert_eq!(city.building_at(50, 49), 0);
}

#[test]
fn destroy_tunnel_structure_skips_invent_budget_misc_refresh() {
    // PE DoBudgetTunnelCheck @ `0x435ab3` → `jmp 0x435dda`: DestroyBuilding only —
    // no invent MISC/budget/reward refresh (same class as subway @ `0x43689b`).
    let mut city = City::new_blank("/tmp/sc2000-tun-noref.sc2", "TunNoRef").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    let offset = budget_service_misc_offset(10, BUDGET_SERVICE_CURRENT_FIELD).expect("tunnel");
    let mut values = city.misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
    values[BUDGET_SERVICE_CURRENT_FIELD] = 0xB22;
    city.set_misc_u32_values(offset, &values);
    city.set_building(20, 20, 0x3f);
    city.set_building(17, 20, 0x41);
    for x in 17..=20 {
        city.set_tunnel_levels(x, 20, 1);
    }
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    state.destroy_tunnel_structure_at(20, 20, &mut crt);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.building_at(17, 20), 0);
    for x in 17..=20 {
        assert_eq!(state.city.tunnel_levels_at(x, 20), 0);
    }
    assert_eq!(crt.seed(), 3_884_216_597);
    assert_eq!(
        state.city.misc_u32_at(offset),
        Some(0xB22),
        "tunnel DestroyBuilding must not invent-refresh budget CURRENT"
    );
}

#[test]
fn underfunded_highway_decay_preserves_watered_and_xtrf() {
    // PE highway budget decay → PlaceTile `0x441f00`: no invent water refresh / XTRF write.
    let mut city = City::new_blank("/tmp/sc2000-hwy-watered.sc2", "HwyWater").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 6, 0);
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 74);
            city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_watered(true));
        }
    }
    city.set_traffic_density(20, 20, 0x44);
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    for y in 20..=21 {
        for x in 20..=21 {
            assert!(is_rubble_tile(state.city.building_at(x, y)), "x={x} y={y}");
            let bits = state.city.tile_bits_at(x, y).unwrap();
            assert!(bits.piped(), "x={x} y={y}");
            assert!(bits.watered(), "x={x} y={y}");
        }
    }
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x44));
}

#[test]
fn mac_monthly_highway_upkeep_is_limited_to_unzoned_cells() {
    fn highway_fixture(zone_low: u8) -> City {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-monthly-highway-zone-gate.sc2",
            "MacHwyZoneGate",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 6, 0);
        for y in 20..=21 {
            for x in 20..=21 {
                city.set_building(x, y, 0x49);
                city.set_zone(x, y, zone_low);
            }
        }
        city
    }

    let seed = 0x361c_8abc;
    let mut unzoned = highway_fixture(0);
    let mut expected_rng = MacToolboxRng::new(seed);
    for _ in 0..5 {
        expected_rng.random_u16();
    }
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut unzoned, 0, 0, &mut rng);
    for y in 20..=21 {
        for x in 20..=21 {
            assert!(is_rubble_tile(unzoned.building_at(x, y)), "x={x} y={y}");
        }
    }
    assert_eq!(rng.seed(), expected_rng.seed());

    let mut zoned = highway_fixture(ZONE_RESIDENTIAL_LIGHT);
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut zoned, 0, 0, &mut rng);
    for y in 20..=21 {
        for x in 20..=21 {
            assert_eq!(zoned.building_at(x, y), 0x49, "x={x} y={y}");
        }
    }
    assert_eq!(
        rng.seed(),
        seed,
        "zoned highway must not draw during XUND-free scan"
    );
}

#[test]
fn mac_scan_zone_nine_square_census_changes_later_selector() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-scan-zone-nine-square-census-interleave.sc2",
        "MacScanZoneNineSquareCensus",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    // e0 is the ratio denominator. The first candidate selects f0; after
    // that square raises f0 to 16, the later candidate selects e3 instead.
    for (tile, count) in [(0xe0u8, 4u32), (0xf0, 12), (0xe3, 12), (0xf2, 0)] {
        city.set_mac_state_misc_u32_at(0x01f0 + usize::from(tile) * 4, count);
    }
    for &(left, top) in &[(42usize, 42usize), (46, 42)] {
        for dy in 0..2 {
            for dx in 0..2 {
                city.set_zone(left + dx, top + dy, 9);
            }
        }
        city.set_tile_bits(left, top, TileBits::new(0x40));
    }

    let seed = (1..100_000)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            probe.random_i16() as u16 & 3 == 0 && probe.random_i16() as u16 & 3 == 0
        })
        .expect("find two accepted zone-nine selector draws");
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_i16();
    expected_rng.random_i16();

    let mut rng = MacToolboxRng::new(seed);
    for phase in 0..=10 {
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, phase / 4, phase % 4, &mut rng);
    }

    for y in 42..=43 {
        for x in 42..=43 {
            assert_eq!(city.building_at(x, y), 0xf0, "first square ({x},{y})");
        }
    }
    assert_eq!(city.building_at(46, 42), 0xe3);
    assert_eq!(city.zone_at(46, 42).unwrap().raw_bits(), 0xf9);
    assert_eq!(city.building_at(47, 42), 0);
    assert_eq!(city.building_at(46, 43), 0);
    assert_eq!(city.mac_xbld_tile_count(0xf0), 16);
    assert_eq!(city.mac_xbld_tile_count(0xe3), 13);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_monthly_road_rail_crossover_uses_road_budget_and_preserves_xtrf() {
    fn crossover_fixture(road_funding: u32, rail_funding: u32) -> City {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-monthly-road-rail-crossover.sc2",
            "MacRoadRail",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 5, road_funding);
        set_budget_funding_for_city_test(&mut city, 8, rail_funding);
        city.set_building(20, 20, 0x45);
        city.set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
        city.set_traffic_density(20, 20, 0x44);
        city
    }

    let seed = 0x3608_4abc;
    let mut road_funded = crossover_fixture(100, 0);
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut road_funded, 0, 0, &mut rng);
    assert_eq!(road_funded.building_at(20, 20), 0x45);
    assert_eq!(
        rng.seed(),
        seed,
        "fully funded Mac road upkeep consumes no draws"
    );

    let mut road_underfunded = crossover_fixture(0, 100);
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_u16();
    expected_rng.random_u16();
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut road_underfunded, 0, 0, &mut rng);
    assert!(is_rubble_tile(road_underfunded.building_at(20, 20)));
    assert!(!road_underfunded.tile_bits_at(20, 20).unwrap().powerable());
    assert_eq!(road_underfunded.traffic_density_at(20, 20), Some(0x44));
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac68k_underfunded_highway_decay_preserves_xtrf() {
    // Shared highway budget decay leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-hwy-xtrf.sc2", "MacHwyX").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 6, 0);
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 74);
        }
    }
    city.set_traffic_density(20, 20, 0x44);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert!(state.decay_transport_budget_for_test() >= 1);
    for y in 20..=21 {
        for x in 20..=21 {
            assert!(is_rubble_tile(state.city.building_at(x, y)), "x={x} y={y}");
        }
    }
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x44));
}

#[test]
fn mac_monthly_upkeep_reads_raw_funding_word_and_keeps_native_draws() {
    // CODE 8 upkeep reads a WORD. The road/rail arm skips its draw at exactly
    // 100%; other values retain the native threshold draw. Values above 100
    // keep their raw low WORD and can pass the wind-adjusted bridge check.
    let cases = [
        ("ordinary road", 0x1e, 5, 0x0001_0078, 60, false, 1),
        ("road at 100", 0x24, 5, 100, 0, false, 0),
        ("road", 0x5d, 5, 0x0001_0078, 60, false, 1),
        ("road crossover at 100", 0x5d, 5, 100, 0, false, 0),
        ("rail", 0x2c, 8, 120, 60, false, 1),
        ("bridge", 0x51, 7, 120, 60, false, 1),
        ("portal", 0x41, 10, 120, 60, false, 1),
        ("highway", 0x49, 6, 120, 60, false, 1),
        ("late portal", 0x41, 10, 120, 60, true, 1),
    ];

    for (name, building, budget_index, raw_funding, wind, late_xund, draws) in cases {
        let mut city = City::new_blank(
            format!("/tmp/sc2000-mac-raw-upkeep-{name}.sc2"),
            "MacRawUpkeep",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, budget_index, raw_funding);
        city.set_mac_state_misc_u32_at(0x64, wind);
        city.set_building(20, 20, building);
        if building == 0x49 {
            for (x, y) in [(21, 20), (20, 21), (21, 21)] {
                city.set_building(x, y, 0x49);
            }
        }
        if late_xund {
            // A nonzero XZON class bypasses the unzoned portal-upkeep arm and
            // reaches the late XUND funding gate at CODE 8 `0x36bc6`.
            city.set_zone(20, 20, 0xfa);
            city.set_underground(20, 20, 0x23);
        }

        let seed = 0x3412_5678;
        let mut expected_rng = MacToolboxRng::new(seed);
        for _ in 0..draws {
            expected_rng.random_u16();
        }
        let mut rng = MacToolboxRng::new(seed);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        assert_eq!(
            city.building_at(20, 20),
            building,
            "{name} should not decay"
        );
        assert_eq!(rng.seed(), expected_rng.seed(), "{name} gate draw count");
    }
}

#[test]
fn underfunded_bridge_overlap_106_uses_destroy_building() {
    // PE @ `0x435a44`: XBLD `0x6a` → BridgeCheck DestroyBuilding, not highway
    // budget PlaceTile rubble. DestroyBuilding's selector-2 high-class walker
    // then clears its classified 2×2 body with PlaceTile(0), including the low
    // class SE cell, without stamping rubble.
    let mut city = City::new_blank("/tmp/sc2000-brg-106.sc2", "Brg106").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_building(20, 20, 106);
    city.set_building(21, 21, 6);
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.building_at(21, 21), 0);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(39));
    assert_ne!(state.query_line, "Bridge Collapsed");
}

#[test]
fn underfunded_bridge_0x5c_uses_destroy_building() {
    // PE DoBudgetBridgeCheck @ `0x435a34`–`0x435a3e`: XBLD `0x51..=0x5c` →
    // DestroyBuilding. Rust `is_bridge_tile` previously stopped at `0x5b`, so
    // power-line label tile `0x5c` skipped surface budget decay entirely.
    let mut city = City::new_blank("/tmp/sc2000-brg-5c.sc2", "Brg5c").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_building(20, 20, 0x5c);
    let mut state = test_state(city);
    assert_eq!(transport_surface_budget_index(0x5c), Some(7));
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(39));
    assert_ne!(state.query_line, "Bridge Collapsed");
}

#[test]
fn underfunded_bridge_queues_auto_goto_like_pe_4019ec() {
    // PE @ `0x435cbe`: AutoGoto `0x4019ec(x,y)` before DestroyBuilding.
    let mut city = City::new_blank("/tmp/sc2000-brg-goto.sc2", "BrgGoto").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_building(20, 20, 87);
    let mut state = test_state(city);
    assert!(state.auto_goto_enabled);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert_eq!(state.take_pending_disaster_auto_goto(), Some((20, 20)));
}

#[test]
fn underfunded_bridge_skips_auto_goto_when_option_off() {
    let mut city = City::new_blank("/tmp/sc2000-brg-nogoto.sc2", "BrgNoGo").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_building(20, 20, 87);
    let mut state = test_state(city);
    state.auto_goto_enabled = false;
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert!(state.take_pending_disaster_auto_goto().is_none());
}

#[test]
fn underfunded_bridge_skips_invent_status_announcement() {
    // PE bridge arm has no status-line write after newspaper `0x402900`.
    let mut city = City::new_blank("/tmp/sc2000-brg-nostatus.sc2", "BrgNoSt").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_building(20, 20, 87);
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(39));
    assert_ne!(state.query_line, "Bridge Collapsed");
    assert!(!state.query_status_pinned);
}

#[test]
fn underfunded_bridge_decay_preserves_watered_and_xtrf() {
    // PE DoBudgetBridgeCheck → DestroyBuilding: no invent water refresh / XTRF write.
    let mut city = City::new_blank("/tmp/sc2000-brg-watered.sc2", "BrgWater").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_building(20, 20, 87);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x33);
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    // PE DestroyBuilding PlaceTile CRT rubble on XTER0 (not invent-clear to 0).
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x33));
}

#[test]
fn mac68k_underfunded_bridge_decay_preserves_xtrf() {
    // Shared DoBudgetBridgeCheck leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-brg-xtrf.sc2", "MacBrgX").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_building(20, 20, 87);
    city.set_traffic_density(20, 20, 0x33);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x33));
}

#[test]
fn underfunded_bridge_destroy_consumes_crt_on_xter0_like_pe() {
    // PE @ `0x435cce`: DestroyBuilding CRT PlaceTile rubble when XTER==0.
    let mut city = City::new_blank("/tmp/sc2000-brg-crt.sc2", "BrgCrt").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_building(20, 20, 87);
    let mut state = test_state(city);
    let before = state.pe_process_crt_seed();
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_ne!(state.pe_process_crt_seed(), before);
}

#[test]
fn destroy_bridge_tile_skips_invent_budget_misc_refresh() {
    // PE @ `0x435cce`→`0x435cdf`→`jmp 0x435dda`: DestroyBuilding + newspaper only —
    // no invent MISC/budget/reward refresh (same class as tunnel @ `0x435dda`).
    let mut city = City::new_blank("/tmp/sc2000-brg-noref.sc2", "BrgNoRef").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    let offset = budget_service_misc_offset(7, BUDGET_SERVICE_CURRENT_FIELD).expect("bridge");
    let mut values = city.misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
    values[BUDGET_SERVICE_CURRENT_FIELD] = 0xC33;
    city.set_misc_u32_values(offset, &values);
    city.set_building(20, 20, 87);
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    state.destroy_bridge_tile(20, 20, &mut crt);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(
        state.city.misc_u32_at(offset),
        Some(0xC33),
        "bridge DestroyBuilding must not invent-refresh budget CURRENT"
    );
}

#[test]
fn underfunded_bridge_clears_xbit_powered_and_powerable_like_destroy_building() {
    // PE DestroyBuilding @ `0x45a07f`: XBIT `&= 0x3d` (clear rotate/powered/powerable).
    // Contrast highway budget PlaceTile which skips road/rail `XBIT &= 0x7f`.
    let mut city = City::new_blank("/tmp/sc2000-brg-xbit.sc2", "BrgXbit").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 7, 0);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 40);
    city.set_building(20, 20, 87);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0)
            .with_piped(true)
            .with_watered(true)
            .with_powered(true)
            .with_powerable(true),
    );
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert!(!bits.powered(), "DestroyBuilding must clear powered");
    assert!(!bits.powerable(), "DestroyBuilding must clear powerable");
}

#[test]
fn underfunded_highways_rubble_even_even_2x2_block() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 6, 0);
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 74);
            city.set_text_overlay(x, y, 0x40 + (x + y) as u8);
        }
    }
    let mut state = test_state(city);

    assert!(state.decay_transport_budget_for_test() >= 1);
    for y in 20..=21 {
        for x in 20..=21 {
            assert!(
                is_rubble_tile(state.city.building_at(x, y)),
                "expected rubble highway at ({x},{y}), got {}",
                state.city.building_at(x, y)
            );
            // PE highway PlaceTile-only — XTXT kept.
            assert_eq!(
                state.city.text_overlay_at(x, y),
                Some(0x40 + (x + y) as u8),
                "highway PlaceTile must keep XTXT at ({x},{y})"
            );
        }
    }
}

#[test]
fn underfunded_highway_odd_anchor_skips_funding_crt() {
    // PE DoBudgetHighwayCheck @ `0x435add`–`0x435aed`: odd SI/DI jumps to the
    // lattice step before funding CRT @ `0x435b0e`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-odd-crt.sc2", "HwyOddCrt").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 6, 0);
    city.set_building(21, 20, 74);
    let mut state = test_state(city);
    let before = state.pe_process_crt_seed();
    assert_eq!(state.decay_surface_transport_at_for_test(21, 20), 0);
    assert_eq!(state.city.building_at(21, 20), 74);
    assert_eq!(state.pe_process_crt_seed(), before);
}

#[test]
fn underfunded_highway_preserves_xbit_powerable() {
    // PE highway arm ends `jmp 0x435dda` — skips road/rail `XBIT &= 0x7f` @ `0x435dcc`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-powerable.sc2", "HwyPwr").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 6, 0);
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 74);
            city.set_tile_bits(x, y, TileBits::new(0).with_powerable(true));
        }
    }
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    for y in 20..=21 {
        for x in 20..=21 {
            assert!(is_rubble_tile(state.city.building_at(x, y)), "x={x} y={y}");
            assert!(
                state.city.tile_bits_at(x, y).unwrap().powerable(),
                "highway PlaceTile must keep powerable at ({x},{y})"
            );
        }
    }
}

#[test]
fn underfunded_highway_keeps_powered_and_skips_invent_misc() {
    // PE highway PlaceTile then `jmp 0x435dda` — no XBIT write (powered stays) and
    // no invent MISC/budget refresh (same epilogue class as road/rail skip).
    let mut city = City::new_blank("/tmp/sc2000-hwy-powered-misc.sc2", "HwyPwrM").expect("blank");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    set_budget_funding_for_test(&mut city, 6, 0);
    let offset = budget_service_misc_offset(6, BUDGET_SERVICE_CURRENT_FIELD).expect("hwy");
    let mut values = city.misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
    values[BUDGET_SERVICE_CURRENT_FIELD] = 0xE55;
    city.set_misc_u32_values(offset, &values);
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 74);
            city.set_tile_bits(
                x,
                y,
                TileBits::new(0).with_powered(true).with_powerable(true),
            );
        }
    }
    let mut state = test_state(city);
    assert!(state.decay_transport_budget_for_test() >= 1);
    for y in 20..=21 {
        for x in 20..=21 {
            assert!(is_rubble_tile(state.city.building_at(x, y)), "x={x} y={y}");
            let bits = state.city.tile_bits_at(x, y).unwrap();
            assert!(bits.powered(), "highway must keep powered at ({x},{y})");
            assert!(bits.powerable(), "highway must keep powerable at ({x},{y})");
        }
    }
    assert_eq!(
        state.city.misc_u32_at(offset),
        Some(0xE55),
        "highway PlaceTile must not invent-refresh budget CURRENT"
    );
}

#[test]
fn fully_funded_highways_do_not_decay() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_test(&mut city, index, 100);
    }
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 74);
        }
    }
    let mut state = test_state(city);

    assert_eq!(state.decay_transport_budget_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), 74);
}

#[test]
fn simulation_day23_preserves_growth_counters_and_industry_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_building(21, 20, INDUSTRIAL_1X1_BUILDINGS[0]);
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 999);
    city.set_misc_u32_at(MISC_PE_GROWTH_AGGREGATE_OFFSET, 99);
    let mut industry = vec![0_u32; INDUSTRY_VALUES];
    industry[0] = 7;
    city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &industry);
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 1, 22);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 23);
    assert_ne!(
        state.city.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET),
        Some(999)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_PE_GROWTH_AGGREGATE_OFFSET),
        Some(99)
    );
    assert_eq!(
        state
            .city
            .misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES)[0],
        7,
        "day-23 census must not rewrite PE industry demand from day 21"
    );
    assert!(state.city.has_dirty_tiles());
}

#[test]
fn simulation_month_advance_writes_clock_date_to_city_misc() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1901, 3);

    state.handle_simulation_month_advanced();

    assert_eq!(
        state.city.saved_date(),
        Some(CityDate {
            year: 1901,
            month: 3
        })
    );
    assert_eq!(state.city.misc_u32_at(0x0c), Some(1900));
    assert_eq!(state.city.misc_u32_at(0x10), Some(350));
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(0));
}

#[test]
fn pe_year_end_flag_matches_win95_december_month() {
    assert_eq!(pe_year_end_flag(1), 0);
    assert_eq!(pe_year_end_flag(11), 0);
    assert_eq!(pe_year_end_flag(12), 1);
}

#[test]
fn simulation_month_advance_sets_pe_year_end_flag_in_december() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1901, 12);

    state.handle_simulation_month_advanced();

    assert_eq!(
        state.city.saved_date(),
        Some(CityDate {
            year: 1901,
            month: 12
        })
    );
    assert_eq!(state.city.misc_u32_at(MISC_YEAR_END_OFFSET), Some(1));
}

#[test]
fn simulation_month_advance_writes_sim_calendar_day_into_city_age() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1901, 3, 13);

    state.handle_simulation_month_advanced();

    assert_eq!(state.city.city_age_days(), Some(350 + 13));
    assert_eq!(state.game_clock.day(), 13);
    assert_eq!(state.game_clock.status_line(), "Mar 14 1901");
}

#[test]
fn llama_clock_wraps_month_after_twenty_five_sim_days() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));
    let advance = state.game_clock.tick(2.0, GameSpeed::Llama);
    assert_eq!(advance.days, 25);
    assert_eq!(advance.months, 1);
    assert_eq!(state.game_clock.month(), 2);
    assert_eq!(state.game_clock.day(), 0);

    state.handle_simulation_month_advanced();

    assert_eq!(
        state.city.saved_date(),
        Some(CityDate {
            year: 1900,
            month: 2
        })
    );
    assert_eq!(state.city.city_age_days(), Some(25));
}

#[test]
fn simulation_month_advance_refreshes_visible_service_status() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.refresh_status_line();
    assert!(!state.query_line.contains("Water Shortage"));

    state.city.set_zone(10, 10, 1);
    state.handle_simulation_month_advanced();

    assert!(state.query_line.contains("Water Shortage 1"));
    assert!(state.query_line.contains("Power Shortage 1"));
}

#[test]
fn water_demand_counts_developed_tiles_not_user_pipe_runs() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_zone(10, 10, 1);
    city.set_building(11, 10, 112);
    city.set_underground(
        12,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );

    assert_eq!(water_demand_tiles(&city), 2);
}

#[test]
fn abandoned_rci_buildings_do_not_count_as_utility_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_zone(10, 10, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(10, 10, ABANDONED_1X1_BUILDINGS[0]);

    assert_eq!(water_demand_tiles(&city), 0);
    assert_eq!(power_demand_tiles(&city), 0);
    assert_eq!(
        water_service_summary(&city),
        WaterServiceSummary {
            served: 0,
            demand: 0
        }
    );
    assert_eq!(
        power_service_summary(&city),
        PowerServiceSummary {
            served: 0,
            demand: 0
        }
    );
}

#[test]
fn water_service_summary_counts_only_watered_consumers_as_served() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(20, 20, 220);
    power_water_source_for_test(&mut city, 20, 20);
    city.set_zone(21, 20, 1);
    city.set_zone(22, 20, 1);
    city.set_zone(23, 20, 1);
    city.set_zone(30, 30, 1);
    for x in 21..=23 {
        city.set_building(x, 20, 112);
    }
    seed_saved_water_worker_bits_for_test(&mut city);

    assert_eq!(
        water_service_summary(&city),
        WaterServiceSummary {
            served: 3,
            demand: 4
        }
    );
}

#[test]
fn status_line_omits_water_shortage_when_capacity_covers_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 220);
    power_water_source_for_test(&mut city, 20, 20);
    city.set_zone(21, 20, 1);
    city.set_zone(22, 20, 1);
    city.set_zone(23, 20, 1);
    for x in 21..=23 {
        city.set_building(x, 20, 112);
    }
    seed_saved_water_worker_bits_for_test(&mut city);
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(!state.query_line.contains("Water Shortage"));
}

#[test]
fn service_status_omits_suffix_when_no_warnings_exist() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    assert!(!ServiceStatus::for_city(&city).has_warnings());
    assert_eq!(
        ServiceStatus::for_city(&city).warning_parts(),
        Vec::<String>::new()
    );
    assert_eq!(ServiceStatus::for_city(&city).status_line_suffix(), "");
}

#[test]
fn status_line_combines_service_warnings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, 1);
    city.set_zone(11, 10, 1);
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(ServiceStatus::for_city(&state.city).has_warnings());
    assert!(state.query_line.contains("Water Shortage 2"));
    assert!(state.query_line.contains("Power Shortage 2"));
    assert!(state.query_line.contains("Treatment Missing 9"));
    assert_eq!(
        ServiceStatus::for_city(&state.city).status_line_suffix(),
        "  Water Shortage 2  Power Shortage 2  Treatment Missing 9"
    );
}

#[test]
fn service_status_summarizes_current_shortages() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, 1);
    city.set_zone(11, 10, 1);

    assert_eq!(
        ServiceStatus::for_city(&city),
        ServiceStatus {
            water: WaterServiceSummary {
                served: 0,
                demand: 2
            },
            power: PowerServiceSummary {
                served: 0,
                demand: 2
            },
            treatment: WaterTreatmentSummary {
                plants: 0,
                required_plants: 9
            },
            water_capacity_warning: false,
            power_capacity_warning: false,
        }
    );
    assert_eq!(
        ServiceStatus::for_city(&city).warning_parts(),
        vec![
            "Water Shortage 2".to_owned(),
            "Power Shortage 2".to_owned(),
            "Treatment Missing 9".to_owned(),
        ]
    );
}

#[test]
fn bare_rci_zones_are_mac_consumers_but_not_pe_xbld_consumers() {
    // Mac-style `power_consuming_tile` counts bare zones; PE flood-fill counts XBLD
    // `[0x70,0xC6)` only (`pe_power_flood_fill_consumer_tile`).
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(power_consuming_tile(&city, 20, 20));
    assert!(!pe_power_flood_fill_consumer_tile(&city, 20, 20));
    assert!(power_network_tile(&city, 20, 20));
}

#[test]
fn trees_are_powerable_only_on_serviceable_zones() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 6); // tree
    assert!(!structural_powerable_tile(&city, 20, 20));
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE);
    assert!(structural_powerable_tile(&city, 20, 20));
    assert!(power_consuming_tile(&city, 20, 20));
    assert!(!pe_power_flood_fill_consumer_tile(&city, 20, 20));
}

#[test]
fn abandoned_rci_tiles_conduct_power_but_are_not_pe_consumers() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE);
    city.set_building(20, 20, ABANDONED_POP1_BUILDINGS[0]);
    assert!(structural_powerable_tile(&city, 20, 20));
    assert!(power_network_tile(&city, 20, 20));
    assert!(!power_consuming_tile(&city, 20, 20));
    assert!(!pe_power_flood_fill_consumer_tile(&city, 20, 20));
    assert!(
        !water_consuming_tile(&city, 20, 20),
        "abandoned tiles are also excluded from water demand"
    );
}

#[test]
fn sim_calendar_day21_eq_le_advances_pe_process_crt() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.refresh_live_misc_current_values();
    let before_weather = state.pe_process_crt_seed();
    let before_lfsr = state.pe_process_lfsr_state();
    let _ = state.sim_calendar_day21_demand_and_graphs();
    assert_ne!(
        state.pe_process_crt_seed(),
        before_weather,
        "day-21 weather + EQ_LE should consume the shared process CRT stream"
    );
    assert_ne!(
        state.pe_process_lfsr_state(),
        before_lfsr,
        "day-21 industry mix should consume pe_process_lfsr"
    );
}

#[test]
fn mac68k_day21_uses_mac_service_history_without_pe_publishers() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-day21-publishers.sc2", "MacD21P").expect("blank city");
    city.set_misc_u32_at(RCI_DEMAND_MISC_OFFSET, 111);
    city.set_misc_u32_at(RCI_DEMAND_MISC_OFFSET + 4, 222);
    city.set_misc_u32_at(RCI_DEMAND_MISC_OFFSET + 8, 333);
    city.pe_w_city_garbage = 444;
    city.set_misc_u32_at(MISC_GARBAGE_OFFSET, 444);
    city.set_pe_unemployment_percent(55);
    city.set_misc_u32_at(MISC_UNEMPLOYMENT_OFFSET, 55);
    city.set_pe_zone_pops([10, 20, 30]);
    city.set_pe_eq_growth_slot7(66);
    let xgrp = vec![0x1234_5678; XGRP_VALUES];
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &xgrp);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let pe_crt_before = state.pe_process_crt_seed();
    let _ = state.sim_calendar_day21_demand_and_graphs();

    assert_eq!(
        state.city.misc_i32_values(RCI_DEMAND_MISC_OFFSET, 3),
        [-489, -378, 2000],
        "Mac publishes its own demand even when PE publishers are disabled"
    );
    assert_eq!(state.city.pe_w_city_garbage, 444);
    assert_eq!(state.city.misc_u32_at(MISC_GARBAGE_OFFSET), Some(444));
    assert_eq!(state.city.pe_unemployment_percent(), 0);
    assert_eq!(state.city.misc_u32_at(MISC_UNEMPLOYMENT_OFFSET), Some(0));
    assert_eq!(state.city.pe_zone_pops(), [10, 20, 30]);
    assert_eq!(state.city.pe_eq_growth_slot7(), 66);
    let mut expected_xgrp = xgrp;
    let age = state.city.city_age_days().unwrap();
    let month = (age / 25) % 12;
    let elapsed_year = (age / 300) as u16 as i16;
    for (row, sample) in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 0),
        (12, 0),
        (
            10,
            state
                .city
                .mac_state_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET)
                .unwrap_or(0),
        ),
        (
            11,
            state
                .city
                .mac_state_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET)
                .unwrap_or(0),
        ),
        (
            8,
            100_u32.wrapping_sub(state.city.mac_power_service_summary()),
        ),
        (
            9,
            100_u32.wrapping_sub(state.city.mac_water_service_summary()),
        ),
        (13, state.city.pe_dw_national_population),
        (14, state.city.pe_dw_national_value),
        (15, i32::from(state.city.pe_w_national_tax as i16) as u32),
    ] {
        let start = row * XGRP_SAMPLES_PER_SERIES;
        expected_xgrp.copy_within(start..start + 11, start + 1);
        expected_xgrp[start] = sample;
        if month == 0 || month == 6 {
            expected_xgrp.copy_within(start + 12..start + 31, start + 13);
            expected_xgrp[start + 12] = sample;
        }
        if month == 0 && elapsed_year % 5 == 0 {
            expected_xgrp.copy_within(start + 32..start + 51, start + 33);
            expected_xgrp[start + 32] = sample;
        }
    }
    assert_eq!(
        state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES),
        expected_xgrp
    );
    assert_eq!(
        state.pe_process_crt_seed(),
        pe_crt_before,
        "Mac phase 21 must not consume Win95 EQ/LE random state"
    );
}

#[test]
fn mac_day21_publishes_saved_riot_ratio_from_current_scan_slots() {
    let mut city = City::new_blank("/tmp/mac-riot-ratio.city", "Ratio").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x05f4, 1);
    city.set_mac_state_misc_u32_at(0x060c, 3);
    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 99);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day21_demand_and_graphs();
    assert_eq!(state.city.mac_state_misc_u32_at(0x05f0), Some(1));
    for (row, sample) in [10, 10, 0, 0].into_iter().enumerate() {
        assert_eq!(
            state
                .city
                .chunk_u32_values(XGRP_CHUNK_NAME, row * XGRP_SAMPLES_PER_SERIES * 4, 1),
            vec![sample]
        );
    }
    assert_eq!(
        state
            .city
            .chunk_u32_values(XGRP_CHUNK_NAME, 12 * XGRP_SAMPLES_PER_SERIES * 4, 1),
        vec![60]
    );
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET),
        Some(60),
        "100 * 3 / (1 + 3 + 1)"
    );
    let path = std::env::temp_dir().join(format!(
        "citygame-mac-riot-ratio-{}.city",
        std::process::id()
    ));
    state.city.save(&path).expect("save ratio");
    let loaded = City::load(&path).expect("reload ratio");
    for (row, sample) in [10, 10, 0, 0].into_iter().enumerate() {
        assert_eq!(
            loaded.chunk_u32_values(XGRP_CHUNK_NAME, row * XGRP_SAMPLES_PER_SERIES * 4, 1),
            vec![sample]
        );
    }
    assert_eq!(
        loaded.mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET),
        Some(60)
    );
    assert_eq!(
        loaded.chunk_u32_values(XGRP_CHUNK_NAME, 12 * XGRP_SAMPLES_PER_SERIES * 4, 1),
        vec![60]
    );
    std::fs::remove_file(path).expect("remove temporary city");
}

#[test]
fn mac_riot_ratio_uses_wrapping_unsigned_arithmetic_and_guards_zero() {
    let mut city = City::new_blank("/tmp/mac-ratio-wrap.city", "Ratio").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x05f0, 0);
    city.set_mac_state_misc_u32_at(0x060c, 0x4000_0000);
    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 99);
    assert_eq!(mac_publish_monthly_riot_ratio(&mut city), Some(0));
    assert_eq!(
        city.mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET),
        Some(0)
    );
    city.set_mac_state_misc_u32_at(0x060c, u32::MAX);
    city.set_mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 99);
    assert_eq!(mac_publish_monthly_riot_ratio(&mut city), None);
    assert_eq!(
        city.mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET),
        Some(99)
    );
    city.kind = CityFileKind::WinSc2;
    city.set_mac_rci_counter(0, 1);
    city.set_mac_rci_counter(7, 3);
    city.set_mac_state_misc_low_byte_at(MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xaa);
    assert_eq!(mac_publish_monthly_riot_ratio(&mut city), Some(60));
    assert_eq!(
        city.mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET),
        Some(60)
    );
    assert_eq!(city.pe_unemployment_percent(), 60);
    assert!(mac_riot_disaster_ready(&city));
    persist_mac_weather_state_to_city(
        &mut city,
        WeatherState {
            heat: 0xaa,
            wind: 20,
            humidity: 30,
            trend: WEATHER_TREND_COLD,
            season: 0,
        },
    );
    let path = std::env::temp_dir().join(format!(
        "citygame-cross-layout-mac-riot-ratio-{}.sc2",
        std::process::id()
    ));
    city.save(&path).expect("save shared ratio");
    let loaded = City::load(&path).expect("reload shared ratio");
    assert_eq!(
        loaded.mac_state_misc_u32_at(MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET),
        Some(60)
    );
    assert_eq!(loaded.pe_unemployment_percent(), 60);
    assert!(mac_riot_disaster_ready(&loaded));
    std::fs::remove_file(path).expect("remove temporary city");
}

#[test]
fn mac_monthly_growth_scalar_wraps_and_round_trips_both_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-scalar-wrap.city", "Scalar").expect("blank city");
        city.kind = kind;
        city.set_mac_rci_counter(1, u32::MAX);
        city.set_mac_rci_counter(2, 2);
        city.set_mac_state_misc_u32_at(0x102c, 1234);
        city.set_mac_state_misc_u32_at(0x0040, u32::MAX - 6);
        mac_publish_monthly_growth_scalar(&mut city);
        assert_eq!(city.mac_rci_counter(0), 1);
        assert_eq!(city.mac_state_misc_u32_at(0x102c), Some(10));
        if kind == CityFileKind::MacCity {
            assert_eq!(city.mac_state_misc_u32_at(0x0040), Some(3));
        } else {
            assert_eq!(city.mac_state_misc_u32_at(0x0040), Some(u32::MAX - 6));
            assert_eq!(city.pe_dw_city_population(), 10);
        }

        let path = std::env::temp_dir().join(format!(
            "citygame-mac-growth-scalar-{}-{}",
            std::process::id(),
            if kind == CityFileKind::MacCity {
                "mac.city"
            } else {
                "win.sc2"
            }
        ));
        city.save(&path).expect("save scalar");
        let mut loaded = City::load(&path).expect("reload scalar");
        assert_eq!(loaded.mac_state_misc_u32_at(0x102c), Some(10));
        if kind == CityFileKind::MacCity {
            assert_eq!(loaded.mac_state_misc_u32_at(0x0040), Some(3));
            // A later phase-21 invocation adds again to the saved running
            // accumulator; the direct native reference census found no
            // annual reset in the decoded instruction operands.
            mac_publish_monthly_growth_scalar(&mut loaded);
            assert_eq!(loaded.mac_state_misc_u32_at(0x102c), Some(10));
            assert_eq!(loaded.mac_state_misc_u32_at(0x0040), Some(13));
        } else {
            assert_eq!(loaded.pe_dw_city_population(), 10);
            assert_eq!(loaded.mac_state_misc_u32_at(0x0040), Some(u32::MAX - 6));
        }
    }
}

#[test]
fn mac_monthly_scan_feeds_day21_budget_and_native_growth_scalar() {
    for (extension, expected_kind) in [
        ("city", CityFileKind::MacCity),
        ("sc2", CityFileKind::WinSc2),
    ] {
        let path = std::env::temp_dir().join(format!(
            "mac-scan-budget-{}.{extension}",
            std::process::id()
        ));
        let mut city = blank_city_layout_fixture(&path, "ScanBudget");
        assert_eq!(city.kind, expected_kind);
        city.prepare_mac_fidelity_from_saved_city();
        city.saved_rotation = 0;
        // Exercise all six scan inputs with recovered class-zero selector
        // weights 1, 8, 12, 36, 8, and 12 before phase-21 publication.
        // Separate candidates by more than a growth footprint: an earlier
        // candidate can otherwise consume a later candidate's zone corner.
        for (category, building) in [
            (1, 0x70),
            (2, 0x8c),
            (3, 0x90),
            (4, 0xae),
            (5, 0x94),
            (6, 0x99),
        ] {
            let x = 12 + (category - 1) * 16;
            city.set_zone(x, 40, 0x80 | category as u8);
            city.set_building(x, 40, building);
        }
        // Fresh New City starts with an empty cohort table and a zero prior
        // scalar. The eligible scan cells below exercise the native growth
        // delta that seeds the deficit/refill balance before cohort work.
        city.set_mac_state_misc_u32_at(0x102c, 0);
        // These are Win95-owned scan counters at the same serialized offsets
        // as Mac's six weighted counters. Mac fidelity must keep its scan
        // state separate when the selected city container is WinSc2.
        if expected_kind.is_win95() {
            for slot in 1..=6 {
                city.set_misc_u32_at(0x05f0 + slot * 4, 0x5a00 + slot as u32);
            }
        }
        let row0 = budget_row_misc_offset_for_city(&city, 0, BUDGET_SERVICE_CURRENT_FIELD)
            .expect("Mac residential budget row");
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.sim_calendar_mac_days_3_18_rci_scan();
        assert_eq!(
            (1..=6)
                .map(|slot| state.city.mac_rci_counter(slot))
                .collect::<Vec<_>>(),
            vec![1, 8, 12, 36, 8, 12],
            "six Mac weighted counters on {expected_kind:?}"
        );

        state.sim_calendar_day21_demand_and_graphs();
        for (row, expected) in [
            (row0, 90),
            (
                budget_row_misc_offset_for_city(&state.city, 1, BUDGET_SERVICE_CURRENT_FIELD)
                    .unwrap(),
                480,
            ),
            (
                budget_row_misc_offset_for_city(&state.city, 2, BUDGET_SERVICE_CURRENT_FIELD)
                    .unwrap(),
                200,
            ),
        ] {
            assert_eq!(
                state.city.budget_misc_u32_at(row),
                Some(expected),
                "three paired Mac budget projections on {expected_kind:?}"
            );
        }
        assert_eq!(
            state.city.mac_state_misc_u32_at(0x102c),
            Some(770),
            "phase 21 publishes ten times the weighted-counter sum on {expected_kind:?}"
        );
        assert_eq!(state.city.mac_rci_counter(0), 77);
        assert_eq!(state.city.mac_population_change_balances, [0, 0]);
        assert_eq!(
            state
                .city
                .misc_u32_values(0x7c, 60)
                .into_iter()
                .step_by(3)
                .sum::<u32>(),
            770,
            "the native fresh-city refill puts all 770 residents into cohorts"
        );
        if expected_kind == CityFileKind::MacCity {
            assert_eq!(state.city.mac_state_misc_u32_at(0x0040), Some(770));
        } else {
            assert_eq!(state.city.pe_dw_city_population(), 770);
            for slot in 1..=6 {
                assert_eq!(
                    state.city.misc_u32_at(0x05f0 + slot * 4),
                    Some(0x5a00 + slot as u32),
                    "Mac scan overwrote Win95 counter slot {slot}"
                );
            }
        }
    }
}

#[test]
fn mac_live_publication_uses_arcology_residents_on_win_layout() {
    let mut city = City::new_blank("/tmp/mac-live-arco-win.sc2", "Mixed").expect("blank city");
    let id = city
        .provision_microsim_for_building(0xfb)
        .expect("arcology row");
    city.set_microsim_stat2(usize::from(id - MIN_SIM_TEXT_ENTRIES), 321);
    city.republish_mac_arcology_resident_total();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        live_population_stats_for_city_with_fidelity(&state.city, EngineFidelity::Mac68k)
            .arcology_population,
        321,
    );
    let (population, _) = state.refresh_live_misc_current_values();
    assert_eq!(population.arcology_population, 321);
    assert_eq!(
        population.total_population,
        population.normal_population + 321
    );
    state.engine_fidelity = EngineFidelity::Win95;
    let (population, _) = state.refresh_live_misc_current_values();
    assert_eq!(population.arcology_population, 0);
}

#[test]
fn mac_monthly_phase_preserves_win_layout_simulation_fields() {
    let mut city = City::new_blank("/tmp/mac-month-win.sc2", "Mixed").expect("blank city");
    for (offset, value) in [(0x019c, 0x1234_5678), (0x05f4, 123), (0x103c, 99)] {
        city.set_misc_u32_at(offset, value);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.finish_sim_calendar_day0_publication(1900, 2, 1);
    for (offset, value) in [(0x019c, 0x1234_5678), (0x05f4, 123), (0x103c, 99)] {
        assert_eq!(
            state.city.misc_u32_at(offset),
            Some(value),
            "offset {offset:#x}"
        );
    }
}

#[test]
fn mac_tax_bases_use_runtime_arcology_residents_on_win_layout() {
    let mut city = City::new_blank("/tmp/mac-tax-arco-win.sc2", "Mixed").expect("blank city");
    let id = city
        .provision_microsim_for_building(0xfb)
        .expect("arcology");
    city.set_microsim_stat2(usize::from(id - MIN_SIM_TEXT_ENTRIES), 321);
    city.republish_mac_arcology_resident_total();
    let population = PopulationStats {
        arcology_population: 12_000,
        ..Default::default()
    };
    assert_eq!(
        budget_tax_bases_for_city_with_fidelity(&city, population, EngineFidelity::Mac68k),
        [53, 26, 26],
    );
    assert_eq!(
        budget_tax_bases_for_city_with_fidelity(&city, population, EngineFidelity::Win95),
        [53, 26, 26],
    );
}

#[test]
fn mac_ordinance_budget_reads_win_layout_ordinance_mask() {
    let mut city = City::new_blank("/tmp/mac-ordinance-win.sc2", "Mixed").expect("blank city");
    assert!(set_city_ordinance_enabled_with_fidelity(
        &mut city,
        ORDINANCE_LEGALIZED_GAMBLING,
        true,
        EngineFidelity::Mac68k,
    ));
    city.set_misc_u32_at(0x0f98, 0);
    assert_eq!(mac_ordinance_raw_cash_total(&city, 0, 150, 0), 300);
    city.set_misc_u32_at(0x0f98, 1 << ORDINANCE_SALES_TAX);
    assert_eq!(mac_ordinance_raw_cash_total(&city, 0, 150, 0), 300);
}

#[test]
fn mac_arcology_publication_updates_shared_win_population_field() {
    let mut city = City::new_blank("/tmp/mac-arco-win.sc2", "Mixed").expect("blank city");
    let id = city
        .provision_microsim_for_building(0xfb)
        .expect("arcology");
    city.set_microsim_stat2(usize::from(id - MIN_SIM_TEXT_ENTRIES), 321);
    city.set_misc_u32_at(0x019c, 0x1234_5678);
    city.set_misc_u32_at(0x1020, 876);
    assert_eq!(city.mac_arcology_resident_total(), 876);
    city.republish_mac_arcology_resident_total();
    assert_eq!(city.misc_u32_at(0x019c), Some(0x1234_5678));
    assert_eq!(city.misc_u32_at(0x1020), Some(321));
    assert_eq!(city.pe_dw_arcology_population(), 321);
    assert_eq!(city.mac_arcology_resident_total(), 321);

    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x1020, 777);
    assert_eq!(city.mac_arcology_resident_total(), 777);
    city.republish_mac_arcology_resident_total();
    assert_eq!(city.mac_state_misc_u32_at(0x1020), Some(321));
}

#[test]
fn mac_arcology_runtime_total_reconstructs_from_win_save() {
    let path = std::env::temp_dir().join(format!(
        "citygame-mac-arcology-win-{}.sc2",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "Mixed").expect("blank city");
    let id = city
        .provision_microsim_for_building(0xfb)
        .expect("arcology");
    city.set_microsim_stat2(usize::from(id - MIN_SIM_TEXT_ENTRIES), 321);
    city.set_misc_u32_at(0x019c, 0x1234_5678);
    city.set_misc_u32_at(0x1020, 654);
    assert_eq!(city.mac_arcology_resident_total(), 654);
    city.republish_mac_arcology_resident_total();
    assert_eq!(city.misc_u32_at(0x1020), Some(321));
    city.save(&path).expect("save Win95 city");
    let loaded = City::load(&path).expect("reload Win95 city");
    assert_eq!(loaded.kind, CityFileKind::WinSc2);
    assert_eq!(loaded.misc_u32_at(0x019c), Some(0x1234_5678));
    assert_eq!(loaded.misc_u32_at(0x1020), Some(321));
    assert_eq!(loaded.pe_dw_arcology_population(), 321);
    assert_eq!(loaded.mac_arcology_resident_total(), 321);
    std::fs::remove_file(path).expect("remove temporary city");
}

#[test]
fn mac_subway_budget_counts_underground_classes_on_win_layout() {
    let mut city = City::new_blank("/tmp/mac-subway-win-layout.sc2", "Mixed").expect("blank city");
    city.set_underground(1, 1, 0x01);
    city.set_underground(2, 1, 0x1f);
    city.set_underground(3, 1, 0x23);
    city.set_underground(4, 1, 0x21); // outside the Mac census set
    city.set_misc_u32_at(0x0fe0, 9000); // unrelated Win95 military field
    mac_budget_refresh_derived_currents(&mut city);
    let offset = budget_row_misc_offset_for_city(&city, 14, BUDGET_SERVICE_CURRENT_FIELD)
        .expect("subway budget row");
    assert_eq!(city.budget_misc_u32_at(offset), Some(3));
    assert_eq!(city.misc_u32_at(0x0fe0), Some(9000));
    city.set_underground(2, 1, 0);
    mac_budget_refresh_derived_currents(&mut city);
    assert_eq!(city.budget_misc_u32_at(offset), Some(2));

    // A native Mac save's persisted counter remains authoritative even when
    // it differs from the map; do not silently rebuild that saved state.
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x0fe8, 27);
    mac_budget_refresh_derived_currents(&mut city);
    let mac_offset = budget_row_misc_offset_for_city(&city, 14, BUDGET_SERVICE_CURRENT_FIELD)
        .expect("Mac subway budget row");
    assert_eq!(city.budget_misc_u32_at(mac_offset), Some(27));
}

#[test]
fn mac_monthly_counter_reset_preserves_win_layout_fields() {
    let mut city = City::new_blank("/tmp/mac-reset-win-layout.sc2", "Mixed").expect("blank city");
    for slot in 0..=7 {
        city.set_misc_u32_at(0x05f0 + slot * 4, 100 + slot as u32);
        city.set_mac_rci_counter(slot, 200 + slot as u32);
    }
    for slot in 0..=7 {
        assert_eq!(city.mac_rci_counter(slot), 200 + slot as u32);
    }
    mac_budget_clear_monthly_rci_scan_counters(&mut city);
    for slot in 0..=7 {
        assert_eq!(city.misc_u32_at(0x05f0 + slot * 4), Some(100 + slot as u32));
        assert_eq!(city.mac_rci_counter(slot), if slot == 0 { 200 } else { 0 });
    }

    city.kind = CityFileKind::MacCity;
    for slot in 0..=7 {
        city.set_mac_state_misc_u32_at(0x05f0 + slot * 4, 100 + slot as u32);
    }
    mac_budget_clear_monthly_rci_scan_counters(&mut city);
    assert_eq!(city.mac_state_misc_u32_at(0x05f0), Some(100));
    for slot in 1..=7 {
        assert_eq!(city.mac_state_misc_u32_at(0x05f0 + slot * 4), Some(0));
    }
}

#[test]
fn mac_rci_budget_projection_uses_mac_runtime_state_on_win_layout() {
    let mut city = City::new_blank("/tmp/mac-engine-win-layout.sc2", "Mixed").expect("blank city");
    assert_eq!(city.kind, CityFileKind::WinSc2);
    city.prepare_mac_fidelity_from_saved_city();

    // The same saved MISC offsets belong to PE state on this layout. Give
    // them deliberately different values from the Mac engine's runtime book.
    for index in 0..6 {
        city.set_misc_u32_at(0x05f4 + index * 4, 1000 + index as u32);
    }
    for (index, count) in [11_i32, -4, -3, 1, 2, 5].into_iter().enumerate() {
        city.set_mac_rci_counter(index + 1, count as u32);
    }
    city.set_mac_arcology_resident_total((-13_i32) as u32);

    let offsets: Vec<_> = (0..3)
        .map(|row| {
            budget_row_misc_offset_for_city(&city, row, BUDGET_SERVICE_CURRENT_FIELD)
                .expect("budget row")
        })
        .collect();
    for &offset in &offsets {
        city.set_budget_misc_u32_values(offset, &[0x5a5a_5a5a]);
    }

    mac_budget_refresh_rci_budget_currents(&mut city);

    assert_eq!(city.budget_misc_u32_at(offsets[0]), Some(68));
    assert_eq!(city.budget_misc_u32_at(offsets[1]), Some((-21_i32) as u32));
    assert_eq!(city.budget_misc_u32_at(offsets[2]), Some(69));
    for index in 0..6 {
        assert_eq!(
            city.misc_u32_at(0x05f4 + index * 4),
            Some(1000 + index as u32)
        );
    }
}

#[test]
fn mac68k_day21_publishes_native_rci_budget_currents_only() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-day21-rci-budget.city", "MacD21B").expect("blank city");
    city.kind = CityFileKind::MacCity;
    // CODE 14's six monthly weighted counters are MISC dwords 381..=386.
    for (index, count) in [11_i32, -4, -3, 1, 2, 5].into_iter().enumerate() {
        city.set_mac_state_misc_u32_at(0x05f4 + index * 4, count as u32);
    }
    // export_237 uses signed long division here: -13 / 6 = -2, -13 / 12 = -1.
    city.set_mac_state_misc_u32_at(0x1020, (-13_i32) as u32);
    let row_offsets = (0..4)
        .map(|row| {
            budget_row_misc_offset_for_city(&city, row, BUDGET_SERVICE_CURRENT_FIELD)
                .expect("Mac budget current offset")
        })
        .collect::<Vec<_>>();
    for &offset in &row_offsets {
        city.set_budget_misc_u32_values(offset, &[0x5a5a_5a5a]);
    }
    let row0_funding = budget_row_misc_offset_for_city(&city, 0, BUDGET_SERVICE_FUNDING_FIELD)
        .expect("Mac row-0 funding offset");
    city.set_budget_misc_u32_values(row0_funding, &[3]);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let _ = state.sim_calendar_day21_demand_and_graphs();

    let actual = row_offsets
        .iter()
        .map(|offset| state.city.budget_misc_u32_at(*offset).expect("row current"))
        .collect::<Vec<_>>();
    assert_eq!(actual[0], 68); // (11 - 4) * 10 + (-13 / 6)
    assert_eq!(actual[1], (-21_i32) as u32); // (-3 + 1) * 10 + (-13 / 12)
    assert_eq!(actual[2], 69); // (2 + 5) * 10 + (-13 / 12)
    assert_eq!(actual[3], 0x5a5a_5a5a, "phase 21 writes only R/C/I rows");

    mac_budget_month_snapshot_and_accrue(&mut state.city, 0);
    let row0_ytd =
        budget_row_misc_offset_for_city(&state.city, 0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD)
            .expect("Mac row-0 accumulator offset");
    assert_eq!(
        state.city.budget_misc_u32_at(row0_ytd),
        Some(204),
        "the next phase-0 cash snapshot consumes the current published at phase 21"
    );
}

#[test]
fn pe_day21_eq_le_and_graph_match_native_worker_replay() {
    let fingerprint = |words: &[u32]| {
        words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            })
    };
    for layout in ["sc2", "city"] {
        for (influx, crt_seed, construction, cohort_hash, graph_hash) in [
            (
                0,
                3_386_963_152,
                499,
                0xf27d_e1f2_60b3_9d14,
                0xfb60_1850_237c_4d1a,
            ),
            (
                90,
                3_355_440_100,
                924,
                0x8251_aaae_cc31_3646,
                0xea34_593e_e2e9_110f,
            ),
        ] {
            let path = format!("/tmp/pe-day21-{layout}.{layout}");
            let mut city = blank_city_layout_fixture(path, "Day 21");
            city.prepare_pe_fidelity_from_saved_city();
            city.set_saved_date_and_day(
                CityDate {
                    year: 1900,
                    month: 1,
                },
                21,
            );
            for (offset, value) in [
                (MISC_NATIONAL_POPULATION_OFFSET, 12_000),
                (MISC_NATIONAL_VALUE_OFFSET, 3_600),
                (MISC_NATIONAL_TAX_OFFSET, 3),
                (MISC_NATIONAL_TREND_OFFSET, 1),
                (MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 100),
                (0x0fa0, 0x140),
            ] {
                city.set_misc_u32_at(offset, value);
            }
            city.set_misc_u32_values(0x06d8, &[0; 16]);
            city.pe_dw_city_pollution = 9_134;
            city.set_pe_power_used_percent(40);
            city.set_pe_water_coverage_percent(65);
            city.pe_dw_tile_count[209] = 18;
            city.pe_dw_tile_count[214] = 18;
            city.pe_dw_tile_count[217] = 16;
            for (index, population) in [
                1_000, 1_240, 960, 876, 2_701, 2_690, 2_809, 3_021, 2_567, 2_345, 1_900, 787, 192,
                112, 10, 0, 0, 0, 0, 300,
            ]
            .into_iter()
            .enumerate()
            {
                city.test_add_pe_eq_cohort(index, population);
            }
            let mut industry = [0; INDUSTRY_VALUES];
            for index in 0..INDUSTRY_COUNT {
                let base = index * INDUSTRY_FIELD_COUNT;
                if matches!(index, 0 | 4) {
                    industry[base] = 1_000;
                } else {
                    industry[base + 1] = 2_000;
                }
            }
            city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &industry);
            let history = (0..XGRP_VALUES as u32).collect::<Vec<_>>();
            city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &history);
            city.persist_pe_day22_population_leftover(30_000 - influx);
            let mut state = test_state(city);
            for category in [1, 3, 5] {
                for _ in 0..1_000 {
                    state.pe_growth_aggregate.add_normal_candidate(category, 1);
                }
            }
            for _ in 0..27 {
                state.pe_growth_aggregate.add_special_candidate(1);
            }
            state.publish_pe_growth_aggregate();
            assert_eq!(state.city.pe_eq_leftover_deaths(), influx);

            // Enter at the native EQ/LE call with the same RNG state and
            // funding as the worker replay. History then consumes its outputs.
            let mut crt = Win95CrtRand::new(17);
            let mut lfsr = Win95Lfsr::new(29);
            state
                .city
                .advance_pe_eq_le_calendar_month(120, 91, 83, 1_000, &mut crt, &mut lfsr);
            assert_eq!(crt.seed(), crt_seed);
            assert_eq!(lfsr.state(), 21_912);
            assert_eq!(
                state
                    .city
                    .misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES)
                    [4 * INDUSTRY_FIELD_COUNT + 2],
                construction
            );
            let cohorts = state.city.misc_u32_values(0x007c, 60);
            assert_eq!(fingerprint(&cohorts), cohort_hash);

            state.refresh_xgrp_history(PopulationStats::default(), CityServiceOutcomes::default());
            let history = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
            assert_eq!(fingerprint(&history), graph_hash);
        }
    }
}

#[test]
fn pe_power_demand_excludes_bare_zones_from_xbld_consumer_band() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, 1);
    city.set_building(11, 10, 112);
    assert_eq!(power_demand_tiles(&city), 2);
    assert_eq!(pe_power_demand_tiles(&city), 1);
}

#[test]
fn zone_receives_water_pipe_service_without_becoming_conductive_node() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 220);
    city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_zone(11, 10, 1);
    assert!(!water_pipe_network_tile(&city, 11, 10));
    assert!(zone_receives_water_pipe_service(&city, 11, 10));
}

#[test]
fn sim_calendar_day1_power_uses_live_game_clock_for_capacity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 200);
    stamp_power_plant_xbit_powerable(&mut city, &[(10, 10)]);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    city.set_zone(11, 10, 1);

    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 6, 1);
    state.pe_process_crt = init_pe_process_crt(&state.city, state.game_clock);
    state.refresh_live_misc_current_values();
    state.sim_calendar_day1_power();
    assert_eq!(
        state.city.misc_u32_at(MISC_POWER_USED_PERCENT_OFFSET),
        Some(power_used_percent_for_city_at_clock(
            &state.city,
            state.game_clock
        )),
    );
}

#[test]
fn sim_calendar_day1_power_advances_pe_process_crt() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 200);
    stamp_power_plant_xbit_powerable(&mut city, &[(10, 10)]);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 6, 1);
    state.pe_process_crt = init_pe_process_crt(&state.city, state.game_clock);
    state.refresh_live_misc_current_values();
    let before = state.pe_process_crt_seed();
    state.sim_calendar_day1_power();
    assert_ne!(state.pe_process_crt_seed(), before);
}

#[test]
fn growth_tick_power_maps_use_process_crt_seed_not_city_name() {
    // PE FUN_004679B0 plant capacity CRT is the process stream; growth phase
    // power maps must share pe_process_crt.seed(), not pe_power_recalc_seed.
    let mut city = City::new_blank("/tmp/sc2000-growth-power.sc2", "GrowthPower").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xa5a5_5a5a);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    // Growth `run_growth_tick_tile_sequence_for_phase` builds maps with this entry seed.
    let _ = power_service_maps_with_entry_seed(&state.city, process);
}

#[test]
fn microsim_growth_tick_power_maps_use_process_crt_seed_not_city_name() {
    // Same FUN_004679B0 process-CRT pin as trips/RCI growth maps: microsim
    // train/marina/arco growth tick must not use pe_power_recalc_seed.
    let mut city = City::new_blank("/tmp/sc2000-micro-power.sc2", "MicroPower").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xb1b1_1b1b);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let _ = power_service_maps_with_entry_seed(&state.city, process);
}

#[test]
fn day2_density_power_maps_use_process_crt_seed_not_city_name() {
    // Day-2 police/fire halve on unpowered stations using the live process
    // CRT plant network (FUN_004679B0), not city-name pe_power_recalc_seed.
    let mut city = City::new_blank("/tmp/sc2000-day2-power.sc2", "Day2Power").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xc3c3_3c3c);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let population = live_population_stats_for_city(&state.city);
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = live_density_maps_for_city_with_power_seed(&state.city, population, outcomes, process);
}

#[test]
fn mac_day2_density_publishes_native_crime_and_pollution_aggregates() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-day2-aggregates.sc2", "MacDay2").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_CITY_CRIME_OFFSET, 0xdead_beef);
    city.set_mac_state_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, 0xcafe_babe);
    city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(40, 40, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_traffic_density(40, 40, 100);
    assert!(set_city_ordinance_enabled_with_fidelity(
        &mut city,
        ORDINANCE_LEGALIZED_GAMBLING,
        true,
        EngineFidelity::Mac68k,
    ));
    assert_eq!(
        city.mac_state_misc_u32_at(0x0fa0).unwrap() & (1 << 2),
        1 << 2,
        "Mac legalized gambling lives in native ordinance slot 1000"
    );
    assert_eq!(mac_ordinance_raw_cash_total(&city, 0, 150, 0), 300);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    let population = live_population_stats_for_city(&state.city);
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    state.sim_calendar_day2_density(population, outcomes);

    let crime = density_map_microsim64_sum(state.city.crime_density_slice().expect("Mac XCRM map"));
    let pollution =
        density_map_microsim64_sum(state.city.pollution_density_slice().expect("Mac XPLT map"));
    assert!(pollution > 0, "traffic contributes to Mac XPLT");
    assert_eq!(
        state.city.mac_state_misc_u32_at(MISC_CITY_CRIME_OFFSET),
        Some(crime),
        "Mac phase 2 must publish the final 64x64 XCRM sum to MISC +0x2c"
    );
    assert_eq!(
        state.city.mac_state_misc_u32_at(MISC_CITY_POLLUTION_OFFSET),
        Some(pollution),
        "Mac phase 2 must publish the final 64x64 XPLT sum to MISC +0x34"
    );
}

#[test]
fn mac_ordinance_raw_cash_matches_all_rooted_ordinal_formulas() {
    let mut city = City::new_blank("/tmp/sc2000-mac-ordinance-table.sc2", "MacOrdinances")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    // CODE 11 ordinal 16 reads these native Mac globals directly; neither is
    // interchangeable with the PE population inputs.
    city.set_mac_state_misc_u32_at(0x102c, 37);
    city.set_mac_state_misc_u32_at(0x1020, 11);

    let expected = [
        77, -101, 154, -50, 33, -12, 50, 25, 16, 20, 16, 33, -77, 59, 25, -25, -48, 0, 50, 59,
    ];
    for (ordinal, expected_amount) in expected.into_iter().enumerate() {
        city.set_mac_state_misc_u32_at(0x0fa0, 1 << ordinal);
        assert_eq!(
            mac_ordinance_raw_cash_total(&city, -101, 77, -59),
            expected_amount,
            "Mac ordinance ordinal {ordinal}"
        );
    }

    // The CODE 11 longword result wraps: Gambling doubles its C base, and the
    // enabled-ordinance accumulator also adds in signed 32-bit arithmetic.
    city.set_mac_state_misc_u32_at(0x0fa0, 1 << ORDINANCE_LEGALIZED_GAMBLING);
    assert_eq!(mac_ordinance_raw_cash_total(&city, 0, i32::MAX, 0), -2);
    city.set_mac_state_misc_u32_at(
        0x0fa0,
        (1 << ORDINANCE_SALES_TAX) | (1 << ORDINANCE_INCOME_TAX),
    );
    assert_eq!(
        mac_ordinance_raw_cash_total(&city, i32::MAX, i32::MAX, 0),
        -2
    );
}

#[test]
fn mac_crime_map_reads_native_ordinance_mask_at_slot_1000() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-crime-ordinance.sc2", "MacCrime").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(40, 40, RESIDENTIAL_1X1_BUILDINGS[0]);
    assert!(set_city_ordinance_enabled_with_fidelity(
        &mut city,
        ORDINANCE_LEGALIZED_GAMBLING,
        true,
        EngineFidelity::Mac68k,
    ));

    let empty = vec![0; CITY_TILES];
    let with_gambling = live_crime_density_map_for_city_with_fidelity(
        &city,
        &empty,
        &empty,
        &empty,
        EngineFidelity::Mac68k,
    );
    assert!(
        density_map_microsim64_sum(&with_gambling) > 0,
        "native MISC slot 1000 bit 2 contributes the Mac +0x10 crime term"
    );

    assert!(set_city_ordinance_enabled_with_fidelity(
        &mut city,
        ORDINANCE_LEGALIZED_GAMBLING,
        false,
        EngineFidelity::Mac68k,
    ));
    assert_eq!(mac_ordinance_raw_cash_total(&city, 0, 150, 0), 0);
    let without_gambling = live_crime_density_map_for_city_with_fidelity(
        &city,
        &empty,
        &empty,
        &empty,
        EngineFidelity::Mac68k,
    );
    assert_eq!(density_map_microsim64_sum(&without_gambling), 0);
}

#[test]
fn growth_tick_rci_development_services_use_process_crt_seed_not_city_name() {
    // Growth-tick police/fire powered coverage for RCI development must share
    // FUN_004679B0 process CRT (same seed as sibling growth power maps).
    let mut city = City::new_blank("/tmp/sc2000-rci-power.sc2", "RciPower").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xd4d4_4d4d);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let power_maps = power_service_maps_with_entry_seed(&state.city, process);
    let _ = rci_development_services_for_city_with_power(&state.city, &power_maps);
}

#[test]
fn develop_rci_zones_power_maps_use_process_crt_seed_not_city_name() {
    // Batch develop_rci_zones_for_phase / complete_rci_construction_sites_for_phase
    // share FUN_004679B0 process CRT with the live growth tick.
    let mut city = City::new_blank("/tmp/sc2000-batch-rci.sc2", "BatchRci").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xe5e5_5e5e);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let power_maps = power_service_maps_with_entry_seed(&state.city, process);
    let _ = rci_development_services_for_city_with_power(&state.city, &power_maps);
}

#[test]
fn day2_density_water_maps_use_process_crt_seed_not_city_name() {
    // Day-2 land-value watered bits share FUN_004679B0 process CRT with
    // sibling police/fire power maps (same entry seed).
    let mut city = City::new_blank("/tmp/sc2000-day2-water.sc2", "Day2Water").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x1717_7171);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let _ = water_service_maps_with_entry_seed(&state.city, process);
    let population = live_population_stats_for_city(&state.city);
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    let _ = live_density_maps_for_city_with_power_seed(&state.city, population, outcomes, process);
}

#[test]
fn decline_unserved_rci_power_maps_use_process_crt_seed_not_city_name() {
    // Unserved RCI decline powered checks share growth-tick process CRT.
    let mut city = City::new_blank("/tmp/sc2000-decline-power.sc2", "DeclinePwr").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x3939_9393);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let _ = power_service_maps_with_entry_seed(&state.city, process);
    let _ = state.decline_unserved_rci_zones();
}

#[test]
fn january_year_roll_water_coverage_uses_process_crt_seed_not_city_name() {
    // January `0x4CC4B8` coverage before structure microsims shares day-20
    // FUN_004679B0 process CRT (not city-name pe_power_recalc_seed).
    let mut city = City::new_blank("/tmp/sc2000-jan-water.sc2", "JanWater").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        1,
    );
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x4a4a_a4a4);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let _ = water_service_summary_with_entry_seed(&state.city, process);
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);
}

#[test]
fn live_city_service_outcomes_power_maps_use_process_crt_seed_not_city_name() {
    // Live LE/EQ police/fire coverage halves unpowered stations via FUN_004679B0
    // process CRT (same seed as growth-tick RCI development services).
    let mut city = City::new_blank("/tmp/sc2000-svc-power.sc2", "SvcPower").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x5b5b_b5b5);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let power_maps = power_service_maps_with_entry_seed(&state.city, process);
    let _ = live_city_service_outcomes_for_city_with_power(&state.city, &power_maps);
    let _ = state.live_city_service_outcomes_with_process_crt();
}

#[test]
fn mac68k_january_leaves_pe_water_coverage_alone() {
    // Mac has no recovered 0x4CC4B8 water-coverage January writer.
    let mut city = City::new_blank("/tmp/sc2000-mac-jan-water.sc2", "MacJanW").expect("blank");
    city.set_pe_water_coverage_percent(0x2a);
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
    assert_eq!(state.city.pe_water_coverage_percent(), 0x2a);
}

#[test]
fn mac68k_day20_leaves_pe_water_coverage_alone() {
    let mut city = City::new_blank("/tmp/sc2000-mac-d20-water.sc2", "MacD20W").expect("blank");
    city.set_pe_water_coverage_percent(0x33);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day20_water();
    assert_eq!(state.city.pe_water_coverage_percent(), 0x33);
}

#[test]
fn mac68k_day20_refreshes_water_bits_skips_pe_coverage() {
    // Mac export_209 / FUN_00026106 rebuilds water bits; PE 0x4CC4B8 stays Win95.
    let city = City::new_blank("/tmp/sc2000-mac-d20-bits.sc2", "MacD20B").expect("blank");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.city.set_pe_water_coverage_percent(0x33);
    state
        .city
        .set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    // Isolated piped tile with no pump: refresh should clear watered.
    state.sim_calendar_day20_water();
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(
        !bits.watered(),
        "Mac day-20 water refresh clears unserved watered"
    );
    assert_eq!(state.city.pe_water_coverage_percent(), 0x33);
}

#[test]
fn mac68k_day20_clears_water_work_bit_and_keeps_unpiped_tower_service() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-d20-water-work.sc2", "MacD20Work").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_building(20, 20, 0xeb);
    // The Water Tower's saved served mark is carried even without conductive
    // membership; 0x08 is worker scratch and is cleared on every wrapper entry.
    city.set_tile_bits(20, 20, TileBits::new(0x1a));
    // An unserved ordinary conductive cell loses served + scratch, keeping
    // its unrelated low bits and XBIT 0x20 membership.
    city.set_tile_bits(21, 20, TileBits::new(0x3b));

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day20_water();

    assert_eq!(
        state.city.tile_bits_at(20, 20).unwrap().raw_bits(),
        0x12,
        "un-piped Water Tower keeps XBIT 0x10 while the worker clears 0x08"
    );
    assert_eq!(
        state.city.tile_bits_at(21, 20).unwrap().raw_bits(),
        0x23,
        "unserved conductive cell clears XBIT 0x10 and worker scratch 0x08"
    );
}

#[test]
fn mac_annual_arcology_uses_saved_xbit_water_result_for_population_and_map() {
    let mut city = City::new_blank("/tmp/sc2000-mac-arco-water-worker.sc2", "MacArcoWater")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    city.set_mac_state_misc_u32_at(0x102c, 8_000);
    city.set_mac_state_misc_u32_at(0x68, 4);
    city.set_mac_state_misc_u32_at(912 * 4, 0);
    for (tile_id, count) in [(0xfb_u8, 16), (0xfc, 16), (0xfd, 16), (0xfe, 16)] {
        city.set_mac_state_misc_u32_at(0x01f0 + usize::from(tile_id) * 4, count);
    }

    // The first arcology is in saved XBIT water membership despite having no
    // inferred XUND pipe; the second has XUND geometry but no saved membership.
    city.set_building(40, 40, 220);
    city.set_building(41, 40, 0xfb);
    city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_powered(true));
    city.set_tile_bits(41, 40, TileBits::new(0).with_piped(true));

    city.set_building(60, 60, 220);
    city.set_building(61, 60, 0xfc);
    city.set_underground(60, 60, 16);
    city.set_underground(61, 60, 16);
    city.set_tile_bits(60, 60, TileBits::new(0).with_piped(true).with_powered(true));
    city.set_tile_bits(61, 60, TileBits::new(0));

    let first = city
        .provision_microsim_for_building(0xfb)
        .expect("first arcology row");
    let second = city
        .provision_microsim_for_building(0xfc)
        .expect("second arcology row");
    let first = usize::from(first - MIN_SIM_TEXT_ENTRIES);
    let second = usize::from(second - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(first, 5);
    city.set_microsim_stat2(first, 100);
    city.set_microsim_stat0(second, 5);
    city.set_microsim_stat2(second, 200);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);

    let served_arcology = state.city.tile_bits_at(41, 40).expect("served arcology");
    let geometry_only_arcology = state.city.tile_bits_at(61, 60).expect("other arcology");
    assert!(served_arcology.piped() && served_arcology.watered());
    assert!(!geometry_only_arcology.piped() && !geometry_only_arcology.watered());
    assert_eq!(state.city.microsim_at(first).unwrap().stat2, 227);
    assert_eq!(state.city.microsim_at(second).unwrap().stat2, 329);
    assert_eq!(state.city.mac_state_misc_u32_at(1032 * 4), Some(556));
}

#[test]
fn mac_january_xmic_caller_preserves_raw_funding_dwords() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-monthly-raw-xmic-funding.sc2",
        "MacRawXmicFunding",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        1,
    );
    city.set_mac_state_misc_u32_at(0x102c, 30_000);
    const MAC_XBLD_CENSUS_BASE: usize = 0x01f0;
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_BASE + 0xd1 * 4, 9);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_BASE + 0xd2 * 4, 9);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_BASE + 0xd6 * 4, 18);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_BASE + 0xd9 * 4, 32);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_BASE + 0xf5 * 4, 9);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_BASE + 0xd4 * 4, 30);
    for budget_index in 0..=4 {
        let offset = budget_service_funding_misc_offset_for_city(&city, budget_index)
            .expect("Mac service-funding row offset");
        city.set_budget_misc_u32_values(offset, &[0x0001_0000]);
        assert_eq!(
            budget_service_funding_raw_for_city(&city, budget_index),
            0x0001_0000
        );
        assert_eq!(
            budget_service_funding_percent_for_city(&city, budget_index),
            100
        );
    }

    let library = city
        .provision_microsim_for_building(0xf5)
        .expect("library XMIC row");
    let library = usize::from(library - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat2(library, 0x1234);
    let museum = city
        .provision_microsim_for_building(0xd4)
        .expect("museum XMIC row");
    let museum = usize::from(museum - MIN_SIM_TEXT_ENTRIES);
    let police = city
        .provision_microsim_for_building(0xd2)
        .expect("police XMIC row");
    let police = usize::from(police - MIN_SIM_TEXT_ENTRIES);
    let fire = city
        .provision_microsim_for_building(0xd3)
        .expect("fire XMIC row");
    let fire = usize::from(fire - MIN_SIM_TEXT_ENTRIES);
    let hospital = city
        .provision_microsim_for_building(0xd1)
        .expect("hospital XMIC row");
    let hospital = usize::from(hospital - MIN_SIM_TEXT_ENTRIES);
    let school = city
        .provision_microsim_for_building(0xd6)
        .expect("school XMIC row");
    let school = usize::from(school - MIN_SIM_TEXT_ENTRIES);
    let college = city
        .provision_microsim_for_building(0xd9)
        .expect("college XMIC row");
    let college = usize::from(college - MIN_SIM_TEXT_ENTRIES);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);

    let library_row = state.city.microsim_at(library).expect("library row");
    assert_eq!(library_row.stat1, 0, "the raw low funding word is zero");
    assert_eq!(
        library_row.stat2, 0x1234,
        "out-of-range raw book candidate preserves the saved count"
    );
    let museum_row = state.city.microsim_at(museum).expect("museum row");
    assert_eq!(museum_row.stat1, 0, "the raw low funding word is zero");
    assert_eq!(
        museum_row.stat2, 0xffee,
        "museum books divide the full funding dword before low-word storage"
    );
    assert_eq!(
        state.city.microsim_at(police).unwrap().stat0,
        0,
        "police row receives raw funding's low byte, not the UI-clamped 100"
    );
    assert_eq!(
        state.city.microsim_at(fire).unwrap().stat1,
        0x8000,
        "fire row shifts the raw dword before narrowing the capacity word"
    );
    assert_eq!(
        state.city.microsim_at(hospital).unwrap().stat3,
        0x8000,
        "hospital row divides the raw health-funding dword by two"
    );
    assert_eq!(
        state.city.microsim_at(school).unwrap().stat3,
        0x4000,
        "school row divides the raw school-funding dword by four"
    );
    assert_eq!(
        state.city.microsim_at(college).unwrap().stat3,
        0,
        "college row stores the raw funding low word"
    );
}

#[test]
fn mac68k_edit_map_vegetation_tree_writes_xbld_and_maintains_census() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-edit-tree.sc2", "MacTree").unwrap();
        city.kind = kind;
        city.set_terrain(35, 35, 0x06);
        city.set_zone(35, 35, 0x82);
        city.set_altitude(35, 35, 7, 3);
        city.set_tile_bits(35, 35, TileBits::new(0xa0));
        city.set_text_overlay(35, 35, 0x49);
        city.set_traffic_density(35, 35, 0x63);
        city.set_underground(35, 35, 0x20);
        let empty_before = city.mac_xbld_tile_count(0);
        let tree_before = city.mac_xbld_tile_count(7);
        let funds_before = city.city_funds();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::EditNewMap;
        state.tool = EditTool::Trees;
        state.mac_process_rng = MacToolboxRng::new(3);

        assert!(state.apply_tool_at_tile_with_shift(35, 35, false));

        assert_eq!(state.city.building_at(35, 35), 7, "{kind:?}");
        assert_eq!(state.city.terrain_at(35, 35), 0x06);
        let altitude = state.city.altm_at(35, 35);
        assert_eq!((altitude.land, altitude.water), (7, 3));
        assert_eq!(state.city.zone_at(35, 35).unwrap().raw_bits(), 0x82);
        assert_eq!(state.city.tile_bits_at(35, 35), Some(TileBits::new(0xa0)));
        assert_eq!(state.city.text_overlay_at(35, 35), Some(0x49));
        assert_eq!(state.city.traffic_density_at(35, 35), Some(0x63));
        assert_eq!(state.city.underground_at(35, 35), 0x20);
        assert_eq!(state.city.city_funds(), funds_before);
        assert_eq!(
            state.city.mac_xbld_tile_count(0),
            empty_before.wrapping_sub(1)
        );
        assert_eq!(
            state.city.mac_xbld_tile_count(7),
            tree_before.wrapping_add(1)
        );
        assert_eq!(state.mac_process_rng.seed(), 50_421);
    }
}

#[test]
fn mac68k_edit_map_vegetation_tree_native_gates_growth_and_shift() {
    // FUN_00003474 gates XBIT/XBLD, not XTER; Shift clears an eligible
    // cell even above the ordinary vegetation-placement class ceiling.
    for (old, wet, clear, accepted, new, draws) in [
        (0, false, false, true, 7, 1),
        (5, false, false, false, 5, 0),
        (6, false, false, true, 7, 0),
        (10, false, false, true, 11, 0),
        (11, false, false, true, 12, 1),
        (12, false, false, true, 12, 1),
        (13, false, false, false, 13, 0),
        (255, false, false, false, 255, 0),
        (13, false, true, true, 0, 0),
        (5, false, true, false, 5, 0),
        (7, true, true, false, 7, 0),
        (0, true, false, false, 0, 0),
    ] {
        let mut city = City::new_blank("/tmp/mac-edit-tree-gates.sc2", "MacTreeGates").unwrap();
        city.set_terrain(35, 35, 0x3e);
        city.set_building(35, 35, old);
        city.set_tile_bits(35, 35, TileBits::new(if wet { 4 } else { 0 }));
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(3);

        let actual = if clear {
            state.remove_edit_map_tree_at(35, 35)
        } else {
            state.place_edit_map_tree_at(35, 35, 0)
        };

        assert_eq!(actual, accepted, "old={old}, wet={wet}, clear={clear}");
        assert_eq!(state.city.building_at(35, 35), new);
        assert_eq!(state.city.terrain_at(35, 35), 0x3e);
        assert_eq!(
            state.mac_process_rng.seed(),
            if draws == 1 { 50_421 } else { 3 }
        );
    }
}

#[test]
fn mac68k_edit_map_water_single_cell_uses_native_planes_and_census() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-editor-water.sc2", "MacWater").unwrap();
        city.kind = kind;
        city.set_building(64, 64, 13);
        city.set_terrain(64, 64, 0);
        city.set_altm_word(64, 64, (0x21 << 10) | (3 << 5) | 7);
        city.set_tile_bits(64, 64, TileBits::new(0xa1));
        city.set_zone(64, 64, 0x82);
        city.set_text_overlay(64, 64, 0x49);
        city.set_traffic_density(64, 64, 0x63);
        city.set_underground(64, 64, 0x20);
        city.set_terrain(64, 63, 0x30);
        city.set_tile_bits(64, 63, TileBits::new(4));
        city.set_altitude(64, 63, 8, 5);
        city.set_building(64, 63, 7);
        city.pe_rebuild_dw_tile_count_434f80();
        let old_count = city.mac_xbld_tile_count(13);
        let empty_count = city.mac_xbld_tile_count(0);
        let funds = city.city_funds();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::EditNewMap;
        state.tool = EditTool::Water;
        state.mac_process_rng = MacToolboxRng::new(3);

        assert!(state.apply_tool_at_tile_with_shift(64, 64, false));

        assert_eq!(state.city.terrain_at(64, 64), 0x45, "{kind:?}");
        assert_eq!(state.city.terrain_at(64, 63), 0x43);
        assert_eq!(state.city.building_at(64, 64), 0);
        assert_eq!(
            state.city.altm_at(64, 64),
            crate::city::AltmSample {
                land: 7,
                water: 7,
                tunnel_levels: 0x21
            }
        );
        assert_eq!(state.city.tile_bits_at(64, 64), Some(TileBits::new(0xa5)));
        assert_eq!(state.city.zone_at(64, 64).unwrap().raw_bits(), 0x80);
        assert_eq!(state.city.text_overlay_at(64, 64), Some(0x49));
        assert_eq!(state.city.traffic_density_at(64, 64), Some(0x63));
        assert_eq!(state.city.underground_at(64, 64), 0x20);
        assert_eq!(state.city.building_at(64, 63), 7);
        assert_eq!(state.city.water_height_at(64, 63), 5);
        assert_eq!(
            state.city.mac_xbld_tile_count(13),
            old_count.wrapping_sub(1)
        );
        assert_eq!(
            state.city.mac_xbld_tile_count(0),
            empty_count.wrapping_add(1)
        );
        assert_eq!(state.city.city_funds(), funds);
        assert_eq!(state.mac_process_rng.seed(), 3);
    }
}

#[test]
fn january_attitude_crime_uses_saved_bss_when_xcrm_is_absent() {
    // Rooted contract: when XCRM is absent the January path consumes saved BSS crime.
    let mut city = City::new_blank("/tmp/sc2000-jan-crime.sc2", "JanCrime").expect("blank");
    city.set_pe_attitude_demand_inputs(0, 0, 0, 20);
    assert!(city.crime_density_slice().is_none());
    let mut state = test_state(city);
    state.city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        1,
    );
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);
    assert_eq!(state.city.pe_attitude_crime_total(), 20);
}

#[test]
fn service_status_line_power_maps_use_process_crt_seed_not_city_name() {
    // Status-line ServiceStatus shortages/capacity share FUN_004679B0 process CRT.
    let mut city = City::new_blank("/tmp/sc2000-status-crt.sc2", "StatusCrt").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x7d7d_d7d7);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let expected =
        ServiceStatus::for_city_with_entry_seed(&state.city, process).status_line_suffix();
    assert_eq!(state.service_status_line(), expected);
}

#[test]
fn city_service_coverage_power_maps_use_process_crt_seed_not_city_name() {
    // City-info/budget police/fire coverage halves unpowered stations via
    // FUN_004679B0 process CRT (same seed as live ServiceStatus).
    let mut city = City::new_blank("/tmp/sc2000-cov-crt.sc2", "CovCrt").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x8e8e_e8e8);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let power_maps = power_service_maps_with_entry_seed(&state.city, process);
    let expected = city_service_coverage_for_city_with_power(&state.city, &power_maps);
    assert_eq!(
        state.live_city_service_coverage_with_process_crt(),
        expected
    );
}

#[test]
fn zone_pipe_service_mask_chains_along_zoned_row() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 220);
    city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    for x in 11..=15 {
        city.set_zone(x, 10, 1);
    }
    let mask = zone_pipe_service_mask(&city);
    for x in 11..=15 {
        assert!(
            mask[10 * CITY_WIDTH + x],
            "zone at ({x},10) should be piped"
        );
    }
    assert!(!water_pipe_network_tile(&city, 13, 10));
}

#[test]
fn pe_power_recalc_seed_uses_saved_city_calendar() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(crate::city::CityDate {
        year: 1900,
        month: 3,
    });
    let clock = GameClock::new_with_day(1900, 3, 1);
    assert_eq!(
        pe_power_recalc_seed(&city),
        pe_power_recalc_seed_for_clock(&city, clock)
    );
}

#[test]
fn power_demand_counts_developed_tiles_not_lines_or_plants() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_zone(10, 10, 1);
    city.set_building(11, 10, 112);
    city.set_building(
        12,
        10,
        network_building_for_mask(NetworkKind::Power, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_building(
        13,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 207);
        }
    }

    assert_eq!(power_demand_tiles(&city), 2);
}

#[test]
fn power_service_summary_counts_only_powered_consumers_as_served() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 4);
    city.set_zone(11, 10, 1);
    city.set_zone(12, 10, 1);
    city.set_zone(13, 10, 1);

    assert_eq!(
        power_service_summary(&city),
        PowerServiceSummary {
            served: 3,
            demand: 3
        }
    );
    let maps = power_service_maps(&city);
    assert_eq!(
        maps.pe_summary(&city),
        PowerServiceSummary {
            served: 0,
            demand: 0
        },
        "bare zones are Mac demand only; PE flood-fill consumers need XBLD [0x70,0xC6)"
    );
}

#[test]
fn pe_power_service_summary_counts_xbld_band_consumers_only() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 30);
    stamp_power_plant_xbit_powerable(&mut city, &[(10, 10)]);
    city.set_zone(11, 10, 1);
    city.set_building(12, 10, 0x70);
    city.set_tile_bits(
        12,
        10,
        city.tile_bits_at(12, 10)
            .unwrap_or_default()
            .with_powerable(true),
    );
    let maps = power_service_maps(&city);
    assert_eq!(
        maps.summary(&city),
        PowerServiceSummary {
            served: 2,
            demand: 2
        }
    );
    assert_eq!(
        maps.pe_summary(&city),
        PowerServiceSummary {
            served: 1,
            demand: 1
        }
    );
}

#[test]
fn status_line_omits_power_shortage_when_capacity_covers_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 207);
        }
    }
    for y in 20..=22 {
        city.set_zone(24, y, 1);
    }
    let mut state = test_state(city);

    state.refresh_status_line();

    assert!(!state.query_line.contains("Power Shortage"));
}

#[test]
fn power_service_maps_mark_only_consumers_served_by_connected_capacity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 4);
    city.set_zone(11, 10, 1);
    city.set_zone(12, 10, 1);
    city.set_zone(13, 10, 1);

    let maps = power_service_maps(&city);

    assert!(maps.powerable_tiles[11 + 10 * CITY_WIDTH]);
    assert!(maps.powerable_tiles[12 + 10 * CITY_WIDTH]);
    assert!(maps.powerable_tiles[13 + 10 * CITY_WIDTH]);
    assert!(maps.powered_tiles[11 + 10 * CITY_WIDTH]);
    assert!(maps.powered_tiles[12 + 10 * CITY_WIDTH]);
    assert!(maps.powered_tiles[13 + 10 * CITY_WIDTH]);
    assert_eq!(
        power_service_summary(&city),
        PowerServiceSummary {
            served: 3,
            demand: 3
        }
    );
}

#[test]
fn power_service_maps_mark_disconnected_consumers_powerable_but_unpowered() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 207);
        }
    }
    city.set_zone(10, 10, 1);

    let maps = power_service_maps(&city);

    assert!(maps.powerable_tiles[10 + 10 * CITY_WIDTH]);
    assert!(!maps.powered_tiles[10 + 10 * CITY_WIDTH]);
    assert_eq!(
        power_service_summary(&city),
        PowerServiceSummary {
            served: 0,
            demand: 1
        }
    );
}

#[test]
fn mac_fidelity_new_city_seeds_shared_industry_defaults_only_in_win_layout() {
    let mut fallback = City::new_blank("/tmp/sc2000-mac-engine-new-city-industry.sc2", "test")
        .expect("blank Win-format city");
    assert_eq!(fallback.kind, CityFileKind::WinSc2);

    initialize_mac_fidelity_new_city_industry_rows(&mut fallback, EngineFidelity::Mac68k);
    let industry = fallback.misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES);
    assert_eq!(industry.len(), INDUSTRY_VALUES);
    for (index, row) in industry.chunks_exact(INDUSTRY_FIELD_COUNT).enumerate() {
        assert_eq!(row, [0, 7, 0], "industry row {index}");
    }

    let mut native_mac = City::new_blank("/tmp/sc2000-native-mac-new-city-industry.sc2", "test")
        .expect("blank city for Mac-layout guard");
    native_mac.kind = CityFileKind::MacCity;
    for index in 0..INDUSTRY_VALUES {
        native_mac.set_misc_u32_at(INDUSTRY_MISC_OFFSET + index * 4, 0xA500_0000 | index as u32);
    }
    let before = native_mac.misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES);
    initialize_mac_fidelity_new_city_industry_rows(&mut native_mac, EngineFidelity::Mac68k);
    assert_eq!(
        native_mac.misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES),
        before,
        "native Mac-layout industry vector stays untouched"
    );
}

#[test]
fn rci_development_stamps_matching_zone_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, zone_id) in [
        (10, ZONE_RESIDENTIAL_LIGHT),
        (11, ZONE_RESIDENTIAL_DENSE),
        (12, ZONE_COMMERCIAL_LIGHT),
        (13, ZONE_COMMERCIAL_DENSE),
        (14, ZONE_INDUSTRIAL_LIGHT),
        (15, ZONE_INDUSTRIAL_DENSE),
    ] {
        city.set_zone(x, 10, zone_id);
        stamp_transport_access_for_test(&mut city, x, 11);
    }
    powered_line_corridor_for_test(&mut city, 10, 15, 9);
    water_footprint_from_below_for_test(&mut city, 10, 10, 1);
    set_rci_demand_for_test(&mut city, 1_000, 1_000, 1_000);
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();
    state.develop_rci_until_built_for_test();

    for (x, zone_id, buildings) in [
        (
            10,
            ZONE_RESIDENTIAL_LIGHT,
            RESIDENTIAL_1X1_BUILDINGS.as_slice(),
        ),
        (
            11,
            ZONE_RESIDENTIAL_DENSE,
            RESIDENTIAL_1X1_BUILDINGS.as_slice(),
        ),
        (
            12,
            ZONE_COMMERCIAL_LIGHT,
            COMMERCIAL_1X1_BUILDINGS.as_slice(),
        ),
        (
            13,
            ZONE_COMMERCIAL_DENSE,
            COMMERCIAL_1X1_BUILDINGS.as_slice(),
        ),
        (
            14,
            ZONE_INDUSTRIAL_LIGHT,
            INDUSTRIAL_1X1_BUILDINGS.as_slice(),
        ),
        (
            15,
            ZONE_INDUSTRIAL_DENSE,
            INDUSTRIAL_1X1_BUILDINGS.as_slice(),
        ),
    ] {
        let building = state.city.building_at(x, 10);
        assert!(buildings.contains(&building));
        assert_eq!(state.city.zone_at(x, 10).unwrap().zone_id(), zone_id);
        assert_eq!(
            state.city.zone_at(x, 10).unwrap().raw_bits() & 0xf0,
            ZONE_CORNER_ALL
        );
        assert!(building_internal_pipe_present(&state.city, x, 10));
        assert!(
            state
                .city
                .tile_bits_at(x, 10)
                .is_some_and(
                    |bits| bits.raw_bits() & XBIT_CHOOSE_AND_PLACE == XBIT_CHOOSE_AND_PLACE
                )
        );
    }
}

#[test]
fn light_rci_zones_only_develop_one_by_one() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
        }
    }
    stamp_transport_access_for_test(&mut city, 19, 20);
    stamp_transport_access_for_test(&mut city, 19, 21);
    stamp_transport_access_for_test(&mut city, 22, 20);
    stamp_transport_access_for_test(&mut city, 22, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();
    state.develop_rci_until_built_for_test();

    for y in 20..=21 {
        for x in 20..=21 {
            assert!(RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(x, y)));
            assert_eq!(
                state.city.zone_at(x, y).unwrap().zone_id(),
                ZONE_RESIDENTIAL_LIGHT
            );
            assert_eq!(
                state.city.zone_at(x, y).unwrap().raw_bits() & 0xf0,
                ZONE_CORNER_ALL
            );
        }
    }
}

#[test]
fn rci_choose_and_place_pop1_ors_xzon_corner_all() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(10, 10, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 10, 9);
    power_adjacent_building_for_test(&mut city, 9, 10);
    water_footprint_from_below_for_test(&mut city, 10, 10, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();
    state.develop_rci_zones();
    assert!(is_construction_building(state.city.building_at(10, 10)));
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL
    );
    assert_eq!(
        state.city.tile_bits_at(10, 10).unwrap().raw_bits() & XBIT_CHOOSE_AND_PLACE,
        XBIT_CHOOSE_AND_PLACE
    );

    state.complete_rci_construction_sites();
    assert!(RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(10, 10)));
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL
    );
    assert_eq!(
        state.city.tile_bits_at(10, 10).unwrap().raw_bits() & XBIT_CHOOSE_AND_PLACE,
        XBIT_CHOOSE_AND_PLACE
    );
}

#[test]
fn rci_choose_and_place_multitile_ors_xbit_e0() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let existing = *RESIDENTIAL_1X1_BUILDINGS.last().unwrap();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, existing);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    state.develop_rci_zones();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert!(
            is_construction_building(state.city.building_at(x, y)),
            "expected pop2 construction at ({x},{y}), got {:#04x}",
            state.city.building_at(x, y)
        );
        assert_eq!(
            state.city.tile_bits_at(x, y).unwrap().raw_bits() & XBIT_CHOOSE_AND_PLACE,
            XBIT_CHOOSE_AND_PLACE
        );
    }

    state.complete_rci_construction_sites();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert!(
            RESIDENTIAL_2X2_BUILDINGS.contains(&state.city.building_at(x, y)),
            "expected 2x2 at ({x},{y}), got {:#04x}",
            state.city.building_at(x, y)
        );
        assert_eq!(
            state.city.tile_bits_at(x, y).unwrap().raw_bits() & XBIT_CHOOSE_AND_PLACE,
            XBIT_CHOOSE_AND_PLACE
        );
    }
}

#[test]
fn dense_rci_zones_develop_large_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (zone_id, origin) in [
        (ZONE_RESIDENTIAL_DENSE, (21, 21)),
        (ZONE_COMMERCIAL_DENSE, (27, 21)),
        (ZONE_INDUSTRIAL_DENSE, (33, 21)),
    ] {
        stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1 + 1);
        water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 3);
        for (x, y) in building_footprint_from_origin(origin.0, origin.1, 3).unwrap() {
            city.set_zone(x, y, zone_id);
        }
        powered_line_corridor_for_test(&mut city, origin.0, origin.0 + 2, origin.1 - 1);
    }
    seed_pe_rci_demand_to_survive_months(&mut city, 1);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    // Win95 StartBuilding(0) always begins vacant growth at 1×1 construction.
    // A road at (origin-1, origin+1) is in the PE 24-ring for 7 of the 9 tiles.
    for (zone_id, origin) in [
        (ZONE_RESIDENTIAL_DENSE, (21, 21)),
        (ZONE_COMMERCIAL_DENSE, (27, 21)),
        (ZONE_INDUSTRIAL_DENSE, (33, 21)),
    ] {
        let developed: Vec<_> = building_footprint_from_origin(origin.0, origin.1, 3)
            .unwrap()
            .into_iter()
            .filter(|&(x, y)| is_construction_building(state.city.building_at(x, y)))
            .collect();
        assert_eq!(
            developed.len(),
            7,
            "expected 1x1 {zone_id} construction on the PE transit ring in {origin:?}"
        );
        for &(x, y) in &developed {
            assert!(
                CONSTRUCTION_POP1_BUILDINGS.contains(&state.city.building_at(x, y)),
                "vacant StartBuilding stays 1×1, got {:#04x} at ({x},{y})",
                state.city.building_at(x, y)
            );
            assert!(building_internal_pipe_present(&state.city, x, y));
        }
    }
}

#[test]
fn rci_census_counts_developed_buildings_by_type() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 0x70);
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_building(x, y, 0xB2);
    }
    for (x, y) in building_footprint_from_origin(30, 20, 2).unwrap() {
        city.set_building(x, y, 0x9E);
    }

    let census = rci_census(&city);

    assert_eq!(census.residential_tiles, 1);
    assert_eq!(census.commercial_tiles, 9);
    assert_eq!(census.industrial_tiles, 4);
    assert_eq!(census.population, 12);
    assert_eq!(census.commercial_population, 9 * 64);
    assert_eq!(census.industrial_population, 4 * 36);
    assert_eq!(census.jobs, 9 * 64 + 4 * 36);

    let population = population_stats_for_city(&city);
    assert_eq!(population.normal_population, census.population);
    assert_eq!(population.residential_population, census.population);
    assert_eq!(
        population.commercial_population,
        census.commercial_population
    );
    assert_eq!(
        population.industrial_population,
        census.industrial_population
    );
    assert_eq!(population.work_force, census.jobs);
    assert_eq!(population.populated_tiles, 14);
}

#[test]
fn live_population_stats_ignore_stale_saved_misc_counts() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (offset, value) in [
        (MISC_WORK_FORCE_PERCENT_OFFSET, 66),
        (MISC_PE_GROWTH_AGGREGATE_OFFSET, 99),
        (MISC_PE_GROWTH_AGGREGATE_OFFSET + 8, 33),
        (MISC_PE_GROWTH_AGGREGATE_OFFSET + 16, 44),
        (MISC_PE_GROWTH_AGGREGATE_OFFSET + 24, 55),
        (MISC_ARCOLOGY_POPULATION_OFFSET, 88_000),
        (MISC_NORMAL_POPULATION_OFFSET, 999),
    ] {
        city.set_misc_u32_at(offset, value);
    }

    let saved = population_stats_for_city(&city);
    assert_eq!(saved.normal_population, 999);
    assert_eq!(saved.arcology_population, 88_000);
    assert_eq!(saved.populated_tiles, 0);
    assert_eq!(saved.work_force, (999 + 88_000) * 66 / 100);

    assert_eq!(
        live_population_stats_for_city(&city),
        PopulationStats::default()
    );
}

#[test]
fn live_population_stats_count_complete_arcologies_from_map() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 251, 4);
    stamp_complete_building_for_test(&mut city, 20, 10, 252, 4);

    let population = live_population_stats_for_city(&city);

    assert_eq!(population.arcology_population, 85_000);
    assert_eq!(population.total_population, 85_000);
}

#[test]
fn refresh_live_misc_preserves_saved_growth_counters() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (offset, value) in [
        (MISC_WORK_FORCE_PERCENT_OFFSET, 66),
        (MISC_PE_GROWTH_AGGREGATE_OFFSET, 99),
        (MISC_PE_GROWTH_AGGREGATE_OFFSET + 8, 33),
        (MISC_PE_GROWTH_AGGREGATE_OFFSET + 16, 44),
        (MISC_PE_GROWTH_AGGREGATE_OFFSET + 24, 55),
        (MISC_ARCOLOGY_POPULATION_OFFSET, 88_000),
        (MISC_NORMAL_POPULATION_OFFSET, 999),
    ] {
        city.set_misc_u32_at(offset, value);
    }
    city.set_building(10, 10, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_building(11, 10, COMMERCIAL_1X1_BUILDINGS[0]);
    stamp_complete_building_for_test(&mut city, 20, 10, 251, 4);
    let mut state = test_state(city);

    let (population, outcomes) = state.refresh_live_misc_current_values();
    let live = live_population_stats_for_city(&state.city);

    assert_eq!(population, live);
    let mut expected_outcomes = live_city_service_outcomes_for_city(&state.city);
    // Refresh publishes the finance-owned city value, which may differ from
    // the non-persisting land-value fallback used by the standalone helper.
    expected_outcomes.city_value = state.city.pe_city_value.max(0) as u32;
    let preview = live_density_maps_for_city_with_fidelity(
        &state.city,
        population,
        expected_outcomes,
        state.pe_process_crt.seed(),
        true,
        state.engine_fidelity,
    );
    expected_outcomes.land_value = land_value_total_from_density(&preview.land_value);
    assert_eq!(outcomes, expected_outcomes);
    assert_eq!(
        state.city.misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET),
        Some(66),
        "live census refresh preserves the published Win95 workforce summary"
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_PE_GROWTH_AGGREGATE_OFFSET),
        Some(99)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_PE_GROWTH_AGGREGATE_OFFSET + 8),
        Some(33)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_PE_GROWTH_AGGREGATE_OFFSET + 16),
        Some(44)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_PE_GROWTH_AGGREGATE_OFFSET + 24),
        Some(55)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET),
        Some(88_000),
        "display capacity must not replace the native arcology resident total"
    );
    assert_eq!(state.city.pe_dw_arcology_population(), 88_000);
    assert_eq!(
        state.city.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET),
        Some(live.normal_population)
    );
    assert_eq!(live.normal_population, 12);
    assert_eq!(live.arcology_population, 55_000);
    assert_eq!(live.populated_tiles, 2);
    assert_eq!(live.residential_tiles, 1);
    assert_eq!(live.commercial_tiles, 1);
    assert_eq!(live.industrial_tiles, 0);
    assert_eq!(live.work_force, 8);
}

#[test]
fn budget_civic_built_uses_dw_tile_count_not_live_complete() {
    // Civic built units: `dwTileCount[id]/9` (college `/16`), not complete-footprint rescan.
    let mut city = City::new_blank("/tmp/sc2000-budget-civic.sc2", "BudCivic").expect("blank");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    assert_eq!(budget_service_built_counts_for_city(&city).police, 1);
    city.pe_dw_tile_count[210] = 0;
    assert_eq!(
        budget_service_built_counts_for_city(&city).police,
        0,
        "stale zero census must ignore police tiles still on the map"
    );
    assert_eq!(building_tile_count_for_city(&city, 210), 9);
    city.pe_dw_tile_count[210] = 18;
    assert_eq!(budget_service_built_counts_for_city(&city).police, 2);
}

#[test]
fn budget_service_rows_use_live_counts_for_generated_rows() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    stamp_complete_building_for_test(&mut city, 20, 10, 214, 3);
    set_building_and_pe_count_for_test(
        &mut city,
        30,
        30,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );

    let rows = budget_service_rows_for_city(&city);

    assert_eq!(
        rows[0],
        BudgetServiceRow {
            current: 1,
            funding: 100,
            year_to_date: 1
        }
    );
    assert_eq!(
        rows[3],
        BudgetServiceRow {
            current: 1,
            funding: 100,
            year_to_date: 1
        }
    );
    assert_eq!(
        rows[5],
        BudgetServiceRow {
            current: 1,
            funding: 100,
            year_to_date: 1
        }
    );
}

#[test]
fn budget_service_rows_preserve_saved_current_funding_and_ytd() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    let mut values = [0_u32; BUDGET_SERVICE_MISC_STRIDE];
    values[BUDGET_SERVICE_CURRENT_FIELD] = 12;
    values[BUDGET_SERVICE_FUNDING_FIELD] = 80;
    values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] = 34;
    city.set_misc_u32_values(
        budget_service_misc_offset(0, 0).expect("budget service offset"),
        &values,
    );

    let rows = budget_service_rows_for_city(&city);

    assert_eq!(
        rows[0],
        BudgetServiceRow {
            current: 12,
            funding: 80,
            year_to_date: 34
        }
    );
}

#[test]
fn pe_new_city_budget_reset_seeds_first_month_service_accrual() {
    let expected = [
        7, 7, 7, 1, 0, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100,
    ];
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.new_city().expect("PE new city setup");

    for (row, value) in expected.into_iter().enumerate() {
        let offset =
            budget_row_misc_offset_for_city(&state.city, row, 0).expect("PE budget row offset");
        let values = state
            .city
            .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
        assert_eq!(
            values[BUDGET_SERVICE_CURRENT_FIELD], value,
            "row {row} current"
        );
        assert_eq!(
            values[BUDGET_SERVICE_FUNDING_FIELD], value,
            "row {row} funding"
        );
    }

    // The PE's first monthly pass snapshots the initialized service currents
    // before rebuilding them from the (still empty) map census.
    state.refresh_monthly_budget_rows(PopulationStats::default());
    let (history_current, history_funding) = pe_budget_month_history_fields(0);
    for row in BUDGET_SERVICE_ROW_START..BUDGET_TOTAL_COUNT {
        let offset =
            budget_row_misc_offset_for_city(&state.city, row, 0).expect("PE budget row offset");
        let values = state
            .city
            .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
        assert_eq!(values[history_current], 100, "row {row} month current");
        assert_eq!(values[history_funding], 100, "row {row} month funding");
        assert_eq!(
            values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD], 10_000,
            "row {row} YTD"
        );
        assert_eq!(
            values[BUDGET_SERVICE_CURRENT_FIELD], 0,
            "row {row} rebuilt current"
        );
    }
}

#[test]
fn new_city_budget_reset_selects_engine_table_and_respects_save_layout() {
    let expected = [
        7, 7, 7, 1, 0, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100,
    ];
    let mut mac_engine_city =
        City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank PE-layout city");
    initialize_new_city_budget_rows(&mut mac_engine_city, EngineFidelity::Mac68k);
    for (row, value) in expected.into_iter().enumerate() {
        let offset = budget_row_misc_offset_for_city(&mac_engine_city, row, 0)
            .expect("Mac budget row offset");
        let values = mac_engine_city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
        assert_eq!(
            values[BUDGET_SERVICE_CURRENT_FIELD], value,
            "row {row} current"
        );
        assert_eq!(
            values[BUDGET_SERVICE_FUNDING_FIELD], value,
            "row {row} funding"
        );
    }

    let mut native_mac =
        City::new_blank("/tmp/sc2000-test-mac-layout.city", "test").expect("blank city");
    native_mac.kind = CityFileKind::MacCity;
    native_mac.set_misc_u32_at(TAX_MISC_OFFSET, 0xA500_0001);
    native_mac.set_misc_u32_at(TAX_MISC_OFFSET + 4, 0xA500_0002);
    initialize_new_city_budget_rows(&mut native_mac, EngineFidelity::Mac68k);
    assert_eq!(native_mac.misc_u32_at(TAX_MISC_OFFSET), Some(0xA500_0001));
    assert_eq!(
        native_mac.misc_u32_at(TAX_MISC_OFFSET + 4),
        Some(0xA500_0002)
    );
    for (row, value) in expected.into_iter().enumerate() {
        let offset =
            budget_row_misc_offset_for_city(&native_mac, row, 0).expect("Mac budget row offset");
        let values = native_mac.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
        assert_eq!(
            values[BUDGET_SERVICE_CURRENT_FIELD], value,
            "row {row} current"
        );
        assert_eq!(
            values[BUDGET_SERVICE_FUNDING_FIELD], value,
            "row {row} funding"
        );
    }
    mac_budget_month_snapshot_and_accrue(&mut native_mac, 0);
    let (history_current, history_funding) = pe_budget_month_history_fields(0);
    for row in BUDGET_SERVICE_ROW_START..BUDGET_TOTAL_COUNT {
        let offset =
            budget_row_misc_offset_for_city(&native_mac, row, 0).expect("Mac budget row offset");
        let values = native_mac.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
        assert_eq!(values[history_current], 100, "row {row} Mac month current");
        assert_eq!(values[history_funding], 100, "row {row} Mac month funding");
        assert_eq!(
            values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD], 10_000,
            "row {row} Mac YTD"
        );
    }
}

#[test]
fn service_building_placement_refreshes_persisted_budget_rows() {
    for (tool, service_index) in [
        (EditTool::Police, 0),
        (EditTool::FireStation, 1),
        (EditTool::Hospital, 2),
        (EditTool::School, 3),
        (EditTool::College, 4),
    ] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(500_000);
        let mut state = test_state(city);
        state.tool = tool;

        assert!(state.apply_tool_at_tile(20, 20), "{tool:?}");
        assert_budget_service_row_for_test(&state.city, service_index, 1, 100, 0);

        set_budget_funding_for_test(&mut state.city, service_index, 73);
        assert!(state.apply_tool_at_tile(40, 40), "{tool:?}");
        assert_budget_service_row_for_test(&state.city, service_index, 2, 73, 0);
    }
}

#[test]
fn infrastructure_placement_refreshes_all_persisted_budget_rows_and_preserves_funding() {
    let mut road_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    road_city.set_city_funds(500_000);
    let mut road_state = test_state(road_city);
    road_state.tool = EditTool::Road;
    set_budget_funding_for_test(&mut road_state.city, 5, 73);
    assert_eq!(
        road_state.place_network_path(&[(10, 10), (11, 10)], NetworkKind::Road),
        2
    );
    assert_budget_service_row_for_test(&road_state.city, 5, 2, 73, 0);

    let mut highway_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    highway_city.set_city_funds(500_000);
    let mut highway_state = test_state(highway_city);
    highway_state.tool = EditTool::Highway;
    set_budget_funding_for_test(&mut highway_state.city, 6, 73);
    assert_eq!(highway_state.place_highway_path(&[(20, 20)]), 1);
    // The PE budget census sums the four XBLD cells in one highway segment.
    assert_budget_service_row_for_test(&highway_state.city, 6, 4, 73, 0);

    let mut bridge_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    bridge_city.set_city_funds(500_000);
    for x in 30..=31 {
        bridge_city.set_terrain(x, 30, 0x30);
    }
    let mut bridge_state = test_state(bridge_city);
    bridge_state.tool = EditTool::Road;
    set_budget_funding_for_test(&mut bridge_state.city, 7, 73);
    assert_eq!(
        bridge_state.place_network_path(&[(30, 30), (31, 30)], NetworkKind::Road),
        2
    );
    assert_budget_service_row_for_test(&bridge_state.city, 7, 2, 73, 0);

    let mut rail_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    rail_city.set_city_funds(500_000);
    let mut rail_state = test_state(rail_city);
    rail_state.tool = EditTool::Rail;
    set_budget_funding_for_test(&mut rail_state.city, 8, 73);
    assert_eq!(
        rail_state.place_network_path(&[(40, 40), (41, 40)], NetworkKind::Rail),
        2
    );
    assert_budget_service_row_for_test(&rail_state.city, 8, 2, 73, 0);

    let mut subway_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    subway_city.set_city_funds(500_000);
    let mut subway_state = test_state(subway_city);
    subway_state.tool = EditTool::Subway;
    set_budget_funding_for_test(&mut subway_state.city, 9, 73);
    assert_eq!(
        subway_state.place_underground_path(&[(50, 50), (51, 50)], UndergroundKind::Subway),
        2
    );
    assert_budget_service_row_for_test(&subway_state.city, 9, 2, 73, 0);

    let mut tunnel_city = tunnel_test_city();
    tunnel_city.set_city_funds(500_000);
    let mut tunnel_state = test_state(tunnel_city);
    tunnel_state.tool = EditTool::Tunnel;
    set_budget_funding_for_test(&mut tunnel_state.city, 10, 73);
    assert_eq!(tunnel_state.place_tunnel_at(20, 20), 4);
    assert_budget_service_row_for_test(&tunnel_state.city, 10, 2, 73, 0);
}

#[test]
fn city_service_coverage_uses_complete_facilities_and_developed_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 0x70);
    city.set_building(80, 80, 0x70);
    city.set_building(50, 50, 0x84);

    stamp_complete_building_for_test(&mut city, 6, 6, 210, 3);
    stamp_complete_building_for_test(&mut city, 6, 12, 211, 3);
    stamp_complete_building_for_test(&mut city, 12, 6, 209, 3);
    stamp_complete_building_for_test(&mut city, 12, 12, 214, 3);
    stamp_complete_building_for_test(&mut city, 11, 10, 13, 1);
    stamp_complete_building_for_test(&mut city, 48, 48, 236, 2);
    city.set_zone(78, 78, ZONE_AIRPORT);
    city.set_building(78, 78, 221);

    let coverage = city_service_coverage_for_city(&city);

    assert_eq!(
        coverage.police,
        ServiceCoverage {
            served: 1,
            demand: 3,
        }
    );
    assert_eq!(
        coverage.fire,
        ServiceCoverage {
            served: 1,
            demand: 3,
        }
    );
    assert_eq!(
        coverage.health,
        ServiceCoverage {
            served: 1,
            demand: 2,
        }
    );
    assert_eq!(
        coverage.education,
        ServiceCoverage {
            served: 1,
            demand: 2,
        }
    );
    assert_eq!(
        coverage.recreation,
        ServiceCoverage {
            served: 1,
            demand: 2,
        }
    );
    assert_eq!(
        coverage.transit,
        ServiceCoverage {
            served: 2,
            demand: 3,
        }
    );
}

#[test]
fn city_service_coverage_ignores_incomplete_service_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 0x70);
    city.set_building(10, 8, 210);
    city.set_building(8, 10, 211);
    city.set_building(12, 10, 209);
    city.set_building(10, 12, 214);

    let coverage = city_service_coverage_for_city(&city);

    assert_eq!(
        coverage.police,
        ServiceCoverage {
            served: 0,
            demand: 1,
        }
    );
    assert_eq!(
        coverage.fire,
        ServiceCoverage {
            served: 0,
            demand: 1,
        }
    );
    assert_eq!(
        coverage.health,
        ServiceCoverage {
            served: 0,
            demand: 1,
        }
    );
    assert_eq!(
        coverage.education,
        ServiceCoverage {
            served: 0,
            demand: 1,
        }
    );
}

#[test]
fn citizen_service_required_count_uses_original_population_steps() {
    let mut population = PopulationStats::default();

    population.normal_population = FIRE_STATION_DEMAND_POPULATION_STEP - 1;
    assert_eq!(
        citizen_service_required_count(population, FIRE_STATION_DEMAND_POPULATION_STEP),
        0
    );

    population.normal_population = FIRE_STATION_DEMAND_POPULATION_STEP;
    assert_eq!(
        citizen_service_required_count(population, FIRE_STATION_DEMAND_POPULATION_STEP),
        1
    );

    population.normal_population = FIRE_STATION_DEMAND_POPULATION_STEP * 2 - 1;
    assert_eq!(
        citizen_service_required_count(population, FIRE_STATION_DEMAND_POPULATION_STEP),
        1
    );

    population.normal_population = FIRE_STATION_DEMAND_POPULATION_STEP * 2;
    assert_eq!(
        citizen_service_required_count(population, FIRE_STATION_DEMAND_POPULATION_STEP),
        2
    );
}

#[test]
fn thresholded_service_coverage_waits_until_citizens_demand_service() {
    let population = PopulationStats {
        normal_population: HOSPITAL_DEMAND_POPULATION_STEP - 1,
        populated_tiles: 10,
        ..Default::default()
    };
    let coverage = ServiceCoverage {
        served: 0,
        demand: 10,
    };

    assert_eq!(
        thresholded_service_coverage_percent(
            coverage,
            population,
            HOSPITAL_DEMAND_POPULATION_STEP,
            0,
        ),
        100
    );
}

#[test]
fn thresholded_service_coverage_scales_with_required_facility_count() {
    let population = PopulationStats {
        normal_population: SCHOOL_DEMAND_POPULATION_STEP * 2,
        populated_tiles: 20,
        ..Default::default()
    };
    let coverage = ServiceCoverage {
        served: 20,
        demand: 20,
    };

    assert_eq!(
        thresholded_service_coverage_percent(
            coverage,
            population,
            SCHOOL_DEMAND_POPULATION_STEP,
            0,
        ),
        0
    );
    assert_eq!(
        thresholded_service_coverage_percent(
            coverage,
            population,
            SCHOOL_DEMAND_POPULATION_STEP,
            1,
        ),
        50
    );
    assert_eq!(
        thresholded_service_coverage_percent(
            coverage,
            population,
            SCHOOL_DEMAND_POPULATION_STEP,
            2,
        ),
        100
    );
}

#[test]
fn city_service_outcomes_improve_with_covered_development() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 0x70);
    city.set_building(11, 10, 0x70);
    city.set_building(12, 10, 0x84);

    let underserved = city_service_outcomes_for_city(&city);

    stamp_complete_building_for_test(&mut city, 6, 6, 210, 3);
    stamp_complete_building_for_test(&mut city, 6, 12, 211, 3);
    stamp_complete_building_for_test(&mut city, 12, 6, 209, 3);
    stamp_complete_building_for_test(&mut city, 12, 12, 214, 3);
    stamp_complete_building_for_test(&mut city, 9, 12, 13, 1);
    stamp_complete_building_for_test(&mut city, 13, 10, 236, 2);

    let covered = city_service_outcomes_for_city(&city);

    assert!(covered.life_expectancy > underserved.life_expectancy);
    assert!(covered.education_quotient > underserved.education_quotient);
    assert!(covered.service_score > underserved.service_score);
    assert!(covered.city_value > underserved.city_value);
    assert!(covered.land_value > underserved.land_value);
    assert!(covered.land_value_quality > underserved.land_value_quality);
}

#[test]
fn land_value_quality_ignores_sparse_xval_average() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 0x70);
    let mut xval = vec![0_u8; CITY_TILES];
    xval[20 * CITY_WIDTH + 20] = 255;
    city.sync_land_value_from_slice(&xval);
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 12_345);

    assert_eq!(land_value_quality_for_city(&city, 82, true), 82);
    assert_eq!(land_value_quality_for_city(&city, 82, false), 82);
}

#[test]
fn land_value_quality_keeps_legacy_misc_percent() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 77);

    assert_eq!(land_value_quality_for_city(&city, 40, true), 77);
    assert_eq!(land_value_quality_for_city(&city, 40, false), 40);
}

#[test]
fn day2_replaces_stale_density_chunks_after_generic_refresh_preserves_them() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.sync_land_value_from_slice(&vec![255; CITY_TILES]);
    city.sync_crime_density_from_slice(&vec![255; CITY_TILES]);
    city.sync_pollution_density_from_slice(&vec![255; CITY_TILES]);
    let mut state = test_state(city);

    let (population, outcomes) = state.refresh_live_misc_current_values();
    assert_eq!(state.city.land_value_at(20, 20), Some(255));
    assert_eq!(state.city.crime_density_at(20, 20), Some(255));
    assert_eq!(state.city.pollution_density_at(20, 20), Some(255));
    state.sim_calendar_day2_density(population, outcomes);

    assert!(
        state.city.land_value_at(20, 20).unwrap() < 255,
        "stale XVAL should be replaced by live land value"
    );
    assert!(
        state.city.crime_density_at(20, 20).unwrap() < 255,
        "stale XCRM should be replaced by live crime"
    );
    assert!(
        state.city.pollution_density_at(20, 20).unwrap() < 255,
        "stale XPLT should be replaced by live pollution"
    );
}

#[test]
fn live_density_maps_reflect_local_services_and_polluters() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in [(20, 20), (80, 80)] {
        city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    stamp_complete_building_for_test(&mut city, 14, 14, 210, 3);
    stamp_complete_building_for_test(&mut city, 22, 20, SMALL_PARK_BUILDING_ID, 1);
    // Adjacent microsim cell to (80,80) so PE blur reaches the dirty tile.
    stamp_complete_building_for_test(&mut city, 76, 80, 207, 4);
    let mut state = test_state(city);

    let (population, outcomes) = state.refresh_live_misc_current_values();
    state.sim_calendar_day2_density(population, outcomes);

    let clean_land = state.city.land_value_at(20, 20).unwrap();
    let dirty_land = state.city.land_value_at(80, 80).unwrap();
    let clean_crime = state.city.crime_density_at(20, 20).unwrap();
    let dirty_crime = state.city.crime_density_at(80, 80).unwrap();
    let clean_pollution = state.city.pollution_density_at(20, 20).unwrap();
    let dirty_pollution = state.city.pollution_density_at(80, 80).unwrap();

    assert!(
        clean_land > dirty_land,
        "serviced clean tile should have better land value"
    );
    assert!(
        dirty_pollution > clean_pollution,
        "tile near coal plant should have more pollution"
    );
    // Crime is now 64×64-blurred like Win95; with sparse fixtures the local
    // police gap still usually raises crime on the unsupported tile.
    if dirty_crime != clean_crime {
        assert!(
            dirty_crime > clean_crime,
            "unsupported low-value tile should have more crime"
        );
    }
}

#[test]
fn pe_land_value_consumes_saved_xpop_and_xcrm_before_rebuilding_them() {
    let mut base =
        City::new_blank("/tmp/sc2000-pe-xval-prior-maps.sc2", "test").expect("blank city");
    base.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    base.set_building(20, 20, 0x84);
    base.sync_population_density_from_slice(&vec![0; CITY_TILES]);
    base.sync_crime_density_from_slice(&vec![0; CITY_TILES]);

    let maps_for = |city: &City| {
        super::super::density::live_density_maps_for_city_with_fidelity(
            city,
            PopulationStats::default(),
            CityServiceOutcomes::default(),
            1,
            false,
            EngineFidelity::Win95,
        )
    };
    let index = density_map_index(20, 20);
    let baseline = maps_for(&base).land_value[index];

    let mut prior_population = base.clone();
    prior_population.sync_population_density_from_slice(&vec![0xff; CITY_TILES]);
    let high_prior_population = maps_for(&prior_population).land_value[index];
    assert_eq!(baseline - high_prior_population, 0x15);

    let mut prior_crime = base;
    prior_crime.sync_crime_density_from_slice(&vec![60; CITY_TILES]);
    let high_prior_crime = maps_for(&prior_crime).land_value[index];
    assert_eq!(baseline - high_prior_crime, 20);
}

#[test]
fn live_land_value_map_scales_local_services_by_budget_funding() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_complete_building_for_test(&mut city, 18, 18, 210, 3);
    seed_service_coverage_footprint_for_test(&mut city, 18, 18, 3, true);
    power_adjacent_building_for_test(&mut city, 18, 17);
    let population = PopulationStats {
        normal_population: 1_000,
        total_population: 1_000,
        populated_tiles: 1,
        residential_tiles: 1,
        ..Default::default()
    };
    let outcomes = CityServiceOutcomes::default();
    let index = density_map_index(20, 20);
    let water_maps = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let pollution = vec![0_u8; CITY_TILES];

    let funded_maps = live_density_maps_for_city_with_power_seed_and_pe_xbit(
        &city,
        population,
        outcomes,
        pe_power_recalc_seed(&city),
        true,
    );
    set_budget_funding_for_test(&mut city, 0, 0);
    let unfunded_maps = live_density_maps_for_city_with_power_seed_and_pe_xbit(
        &city,
        population,
        outcomes,
        pe_power_recalc_seed(&city),
        true,
    );

    assert!(
        funded_maps.police[index] > unfunded_maps.police[index],
        "police funding should raise live XPLC coverage"
    );

    // Land value consumes crime; hold other inputs fixed and vary crime like XPLC would.
    let population_density = live_population_density_map_for_city(&city);
    let funded = live_land_value_density_map_for_city(
        &city,
        &water_maps,
        &pollution,
        &funded_maps.crime,
        &population_density,
    )[index];
    let mut high_crime = funded_maps.crime.clone();
    high_crime[index] = high_crime[index].saturating_add(80).min(255);
    let unfunded = live_land_value_density_map_for_city(
        &city,
        &water_maps,
        &pollution,
        &high_crime,
        &population_density,
    )[index];
    assert!(
        funded > unfunded,
        "higher crime from missing police coverage should reduce land value ({funded} vs {unfunded})"
    );
}

#[test]
fn live_land_value_uses_saved_watered_xbit_and_falls_with_pollution() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    let water_maps = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let clean = vec![0_u8; CITY_TILES];
    let population_density = live_population_density_map_for_city(&city);
    let index = density_map_index(20, 20);

    let dry = live_land_value_density_map_for_city(
        &city,
        &water_maps,
        &clean,
        &clean,
        &population_density,
    )[index];

    // The current service allocator may predict water for this cell, but the
    // native XVAL pass reads its saved XBIT bit 0x10 and does not use that map.
    let mut recomputed_service = water_maps.clone();
    recomputed_service.watered_tiles[20 * CITY_WIDTH + 20] = true;
    let recomputed_only = live_land_value_density_map_for_city(
        &city,
        &recomputed_service,
        &clean,
        &clean,
        &population_density,
    )[index];
    assert_eq!(
        recomputed_only, dry,
        "fresh water assignment is not XVAL input"
    );

    city.set_tile_bits(20, 20, TileBits::new(0).with_watered(true));
    let wet = live_land_value_density_map_for_city(
        &city,
        &water_maps,
        &clean,
        &clean,
        &population_density,
    )[index];

    let mut polluted = clean.clone();
    polluted[index] = 200;
    let dirty = live_land_value_density_map_for_city(
        &city,
        &water_maps,
        &polluted,
        &clean,
        &population_density,
    )[index];

    assert!(
        wet > dry,
        "watered tiles should raise Win95 terrain land value"
    );
    assert!(
        dirty < wet,
        "pollution should reduce land value on the residential formula"
    );
    assert_eq!(
        wet.saturating_sub(dirty),
        land_value_pollution_penalty(ZONE_RESIDENTIAL_LIGHT, 200) as u8,
        "residential land value should fall by XPLT/5"
    );
}

#[test]
fn pe_xval_does_not_add_population_weights_to_terrain_fold() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // PE writes 0x28 to the shared full-resolution scratch for either
    // developed tile. It does not add the population weight in the later
    // terrain fold; these pollution seed coordinates lie away from this cell.
    city.set_zone(20, 20, ZONE_COMMERCIAL_LIGHT);
    let water_maps = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let clean = vec![0_u8; CITY_TILES];
    let index = density_map_index(20, 20);
    let zero_pop = vec![0_u8; CITY_TILES];

    let vacant =
        live_land_value_density_map_for_city(&city, &water_maps, &clean, &clean, &zero_pop)[index];
    city.set_building(20, 20, 0x84); // PE table weight 1
    let light =
        live_land_value_density_map_for_city(&city, &water_maps, &clean, &clean, &zero_pop)[index];
    city.set_building(20, 20, 0xae); // PE table weight 4
    let heavier =
        live_land_value_density_map_for_city(&city, &water_maps, &clean, &clean, &zero_pop)[index];

    assert!(
        light > vacant,
        "Win95 0x4e9648 weights should raise land value for RCI buildings ({light} vs {vacant})"
    );
    assert_eq!(heavier, light, "the terrain fold must not add XPOP weights");
}

#[test]
fn land_value_penalties_match_win95_microsim_byte_math() {
    assert_eq!(land_value_pollution_penalty(ZONE_RESIDENTIAL_LIGHT, 50), 10);
    assert_eq!(land_value_pollution_penalty(ZONE_RESIDENTIAL_DENSE, 50), 10);
    assert_eq!(land_value_pollution_penalty(ZONE_COMMERCIAL_LIGHT, 40), 10);
    assert_eq!(land_value_pollution_penalty(ZONE_COMMERCIAL_DENSE, 40), 10);
    assert_eq!(land_value_pollution_penalty(ZONE_INDUSTRIAL_LIGHT, 64), 4);
    assert_eq!(land_value_pollution_penalty(ZONE_INDUSTRIAL_DENSE, 64), 4);

    assert_eq!(land_value_crime_penalty(ZONE_RESIDENTIAL_LIGHT, 30), 10);
    assert_eq!(land_value_crime_penalty(ZONE_COMMERCIAL_LIGHT, 30), 10);
    assert_eq!(land_value_crime_penalty(ZONE_INDUSTRIAL_LIGHT, 40), 10);
    assert_eq!(land_value_crime_penalty(ZONE_INDUSTRIAL_DENSE, 40), 10);
}

#[test]
fn land_value_population_adjustment_matches_pe_zone_arms() {
    assert_eq!(
        land_value_population_adjustment(ZONE_RESIDENTIAL_LIGHT, 0x3f),
        0x15
    );
    assert_eq!(
        land_value_population_adjustment(ZONE_RESIDENTIAL_DENSE, 0x40),
        0
    );
    assert_eq!(
        land_value_population_adjustment(ZONE_COMMERCIAL_LIGHT, 48),
        16
    );
    assert_eq!(
        land_value_population_adjustment(ZONE_COMMERCIAL_DENSE, 3),
        1
    );
    assert_eq!(
        land_value_population_adjustment(ZONE_INDUSTRIAL_LIGHT, 0),
        0
    );
    assert_eq!(
        land_value_population_adjustment(ZONE_INDUSTRIAL_DENSE, 0xff),
        0
    );
}

#[test]
fn pe_crime_source_cell_matches_pe_and_mac_27ab4() {
    // PE FUN_0046A840 / Mac FUN_00027AB4 @ 0x286bc: XPOP - (XVAL>>2) - (XPLC>>1).
    assert_eq!(pe_crime_source_cell(100, 40, 20, false), 100 - 10 - 10);
    assert_eq!(
        pe_crime_source_cell(100, 40, 20, true),
        100 - 10 - 10 + 0x10
    );
    assert_eq!(pe_crime_source_cell(0, 0xff, 0xff, false), 0 - 63 - 127);
}

#[test]
fn win95_density_phase_publishes_projected_crime_mask_in_xbit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(80, 80, 0x70);
    city.set_tile_bits(10, 10, TileBits::new(0x44));
    city.set_tile_bits(80, 80, TileBits::new(0x88));
    city.set_tile_bits(70, 70, TileBits::new(0x08));

    refresh_crime_eligibility_xbit_for_density_phase(&mut city, EngineFidelity::Win95);

    assert_eq!(city.tile_bits_at(10, 10).unwrap().raw_bits(), 0x4c);
    assert_eq!(
        city.tile_bits_at(40, 40).unwrap().raw_bits() & 0x08,
        0x08,
        "the large-building tile projects eligibility to its 64x64 source cell"
    );
    assert_eq!(
        city.tile_bits_at(80, 80).unwrap().raw_bits(),
        0x80,
        "PE clears only bit 0x08 at XBLD > 0x6f full-resolution cells"
    );
    assert_eq!(
        city.tile_bits_at(70, 70).unwrap().raw_bits(),
        0x08,
        "PE does not globally clear the transient marker in this worker"
    );
}

#[test]
fn mac_density_phase_clears_and_rebuilds_projected_crime_mask_in_xbit() {
    let mut city = City::new_blank("/tmp/sc2000-test.city", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(22, 22, 0x1d);
    city.set_tile_bits(10, 10, TileBits::new(0x44));
    city.set_tile_bits(70, 70, TileBits::new(0x48));

    refresh_crime_eligibility_xbit_for_density_phase(&mut city, EngineFidelity::Mac68k);

    assert_eq!(city.tile_bits_at(10, 10).unwrap().raw_bits(), 0x4c);
    assert_eq!(
        city.tile_bits_at(70, 70).unwrap().raw_bits(),
        0x40,
        "Mac clears bit 0x08 across the full plane before projecting eligibility"
    );
}

#[test]
fn land_value_abandoned_halve_matches_pe_toward_zero_subtract() {
    // PE `0x46b0fc`: ebx -= toward_zero(ebx/2). Odd positives keep the ceiling half.
    assert_eq!(land_value_apply_abandoned_halve(0), 0);
    assert_eq!(land_value_apply_abandoned_halve(1), 1);
    assert_eq!(land_value_apply_abandoned_halve(4), 2);
    assert_eq!(land_value_apply_abandoned_halve(5), 3);
    assert_eq!(land_value_apply_abandoned_halve(-5), -3);
    assert_ne!(land_value_apply_abandoned_halve(5), 5 / 2);
}

#[test]
fn land_value_center_bonus_scales_match_pe_toward_zero_divisors() {
    let center = (64, 64);
    // raw = 0x40 - 1 - 0 = 0x3f (odd): residential /2 → 0x1f, industrial /4 → 0x0f.
    assert_eq!(
        land_value_center_bonus(ZONE_RESIDENTIAL_LIGHT, 62, 64, center),
        0x3f / 2
    );
    assert_eq!(
        land_value_center_bonus(ZONE_COMMERCIAL_DENSE, 62, 64, center),
        0x3f
    );
    assert_eq!(
        land_value_center_bonus(ZONE_INDUSTRIAL_LIGHT, 62, 64, center),
        0x3f / 4
    );
    assert_eq!(
        land_value_center_bonus(ZONE_RESIDENTIAL_LIGHT, 400, 64, center),
        0
    );
}

#[test]
fn industrial_land_value_ignores_parks_in_its_own_row() {
    // The PE industrial arm uses 0x4cc778 for its center and column neighbors;
    // those do not include park weights. Neighboring rows use wTMap instead.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, 1);
    city.set_zone(20, 24, ZONE_INDUSTRIAL_LIGHT);
    city.set_building(20, 24, 1);
    let water_maps = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let clean = vec![0_u8; CITY_TILES];
    let population = vec![0_u8; CITY_TILES];

    let without_park = live_land_value_density_map_for_city_at_center(
        &city,
        &water_maps,
        &clean,
        &clean,
        &population,
        (64, 64),
        true,
    );
    let res_before = without_park[density_map_index(20, 20)];
    let ind_before = without_park[density_map_index(20, 24)];

    city.set_building(21, 21, 0xd5); // park shares the residential 32×32 cell neighborhood
    city.set_building(21, 25, 0xd5); // park shares the industrial cell neighborhood
    let with_park = live_land_value_density_map_for_city_at_center(
        &city,
        &water_maps,
        &clean,
        &clean,
        &population,
        (64, 64),
        true,
    );
    assert!(
        with_park[density_map_index(20, 20)] > res_before,
        "residential land value should rise from nearby park weights on wTMap"
    );
    assert_eq!(
        with_park[density_map_index(20, 24)],
        ind_before,
        "industrial land value must ignore park weights in its current industrial row"
    );
}

#[test]
fn live_land_value_map_applies_commercial_xpop_bonus() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // Far from city center so center bonus does not swamp the XPOP/3 term.
    city.set_zone(100, 100, ZONE_COMMERCIAL_LIGHT);
    city.set_building(100, 100, 0xae); // weight 4 → XPOP 16
    let water_maps = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let clean = vec![0_u8; CITY_TILES];
    let population_density = live_population_density_map_for_city(&city);
    let index = density_map_index(100, 100);
    assert_eq!(population_density[index], 16);

    let with_pop = live_land_value_density_map_for_city(
        &city,
        &water_maps,
        &clean,
        &clean,
        &population_density,
    )[index];
    let without_pop = live_land_value_density_map_for_city(
        &city,
        &water_maps,
        &clean,
        &clean,
        &vec![0_u8; CITY_TILES],
    )[index];
    assert_eq!(
        with_pop.saturating_sub(without_pop),
        5,
        "commercial land value should rise by XPOP/3 (16/3=5)"
    );
}

#[test]
fn live_crime_map_scales_local_police_by_budget_funding() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_complete_building_for_test(&mut city, 18, 18, 210, 3);
    seed_service_coverage_footprint_for_test(&mut city, 18, 18, 3, true);
    power_adjacent_building_for_test(&mut city, 18, 17);
    let land_value = vec![0_u8; CITY_TILES];
    let population_density = live_population_density_map_for_city(&city);
    let index = density_map_index(20, 20);
    let power_maps = power_service_maps(&city);

    let services = rci_development_services_for_city_with_power(&city, &power_maps);
    let funded_police =
        live_police_coverage_map_for_city_with_pe_xbit(&city, &power_maps, &services, true);
    let funded =
        live_crime_density_map_for_city(&city, &population_density, &funded_police, &land_value)
            [index];

    set_budget_funding_for_test(&mut city, 0, 0);
    let services = rci_development_services_for_city_with_power(&city, &power_maps);
    let unfunded_police =
        live_police_coverage_map_for_city_with_pe_xbit(&city, &power_maps, &services, true);
    let unfunded =
        live_crime_density_map_for_city(&city, &population_density, &unfunded_police, &land_value)
            [index];

    assert!(
        funded_police[index] > unfunded_police[index],
        "police funding should raise XPLC coverage"
    );
    assert!(
        unfunded > funded,
        "police funding should suppress crime via XPLC ({unfunded} vs {funded})"
    );
}

#[test]
fn live_crime_map_blurs_into_adjacent_microsim_cells() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    // Arcology weight 0x0C → XPOP 48 so neighbor blur survives integer divisor dilution.
    city.set_building(20, 20, 0xfb);
    city.set_tile_bits(20, 20, TileBits::new(0x08));
    let land_value = vec![0_u8; CITY_TILES];
    let police = vec![0_u8; CITY_TILES];
    let population_density = live_population_density_map_for_city(&city);
    let crime = live_crime_density_map_for_city(&city, &population_density, &police, &land_value);

    let origin = crime[density_map_index(20, 20)];
    // XCRM microcells are 2×2 tiles, so x=22 is one blur step from x=20.
    // Both remain inside the same 4×4 XPOP cell.
    let adjacent = crime[density_map_index(22, 20)];
    let far = crime[density_map_index(40, 40)];
    assert!(origin > 0, "developed microsim cell should have crime");
    assert_eq!(
        population_density[density_map_index(22, 20)],
        population_density[density_map_index(20, 20)],
        "adjacent XCRM cells share this source XPOP cell"
    );
    assert!(
        adjacent > 0,
        "Win95 XCRM blur should leak into the neighboring microsim cell ({adjacent})"
    );
    assert!(
        adjacent <= origin,
        "blurred neighbor should not exceed the source cell ({adjacent} vs {origin})"
    );
    assert_eq!(
        far, 0,
        "far microsim cells stay clear without a second blur pass"
    );
}

#[test]
fn live_population_density_map_matches_pe_weight_shift() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // Place away from city-edge quirks; one weight-1 R tile → XPOP = 1<<2 = 4.
    city.set_building(20, 20, 0x70);
    let pop = live_population_density_map_for_city(&city);
    assert_eq!(pop[density_map_index(20, 20)], 4);
    assert_eq!(
        pop[density_map_index(23, 23)],
        4,
        "XPOP is constant across the 32×32 / 4×4 tile cell"
    );
    assert_eq!(pop[density_map_index(24, 24)], 0);

    // Weight-4 building → 16; arcology arm adds 0x0C → 48.
    city.set_building(20, 20, 0xae);
    assert_eq!(
        live_population_density_map_for_city(&city)[density_map_index(20, 20)],
        16
    );
    city.set_building(20, 20, 0xfb);
    assert_eq!(
        live_population_density_map_for_city(&city)[density_map_index(20, 20)],
        48
    );

    // Abandoned IDs have PE table weight 0, so they do not seed XPOP/crime.
    city.set_building(20, 20, ABANDONED_1X1_BUILDINGS[0]);
    assert_eq!(
        live_population_density_map_for_city(&city)[density_map_index(20, 20)],
        0
    );
}

#[test]
fn live_population_density_map_matches_mac_high_tile_special_weights() {
    let mut city =
        City::new_blank("/tmp/sc2000-xpop-special-weights.sc2", "test").expect("blank city");
    for building in 0xc6..=0xff {
        city.set_building(20, 20, building);
        let expected = if (0xfb..=0xfe).contains(&building) {
            0x0c * 4
        } else {
            2 * 4
        };
        assert_eq!(
            live_population_density_map_for_city(&city)[density_map_index(20, 20)],
            expected,
            "native Mac XPOP weight for XBLD {building:#04x}"
        );
    }
}

#[test]
fn pe_rate_of_growth_step_matches_win95_formula() {
    // Neutral + zero delta stays neutral: (0x7F*7 + 128) / 8 = (889 + 128) / 8 = 127.
    assert_eq!(pe_rate_of_growth_step(0x7f, 10, 10), 0x7f);
    // Positive XPOP delta lifts growth above neutral.
    assert!(pe_rate_of_growth_step(0x7f, 0, 48) > 0x7f);
    // Negative delta pulls growth below neutral.
    assert!(pe_rate_of_growth_step(0x7f, 48, 0) < 0x7f);
    assert_eq!(pe_rate_of_growth_step(0xff, 0, 255), 0xff);
    assert_eq!(pe_rate_of_growth_step(0, 255, 0), 0);
}

#[test]
fn live_population_density_map_excludes_outer_tile_border() {
    let mut city = City::new_blank("/tmp/sc2000-xpop-border.sc2", "test").expect("blank city");
    // Native Mac FUN_00027AB4 and PE FUN_0046A840 both scan 1..=126.
    // Put one tile on each edge; two interior neighbors share coarse cells
    // with the top/left edge samples so an accidental edge contribution
    // changes their expected value from 4 to 8.
    for (x, y) in [(0, 10), (127, 10), (10, 0), (10, 127)] {
        city.set_building(x, y, 0x70);
    }
    city.set_building(1, 10, 0x70);
    city.set_building(10, 1, 0x70);

    let population = live_population_density_map_for_city(&city);
    assert_eq!(population[density_map_index(1, 10)], 4);
    assert_eq!(population[density_map_index(10, 1)], 4);
    assert_eq!(population[density_map_index(127, 10)], 0);
    assert_eq!(population[density_map_index(10, 127)], 0);
}

#[test]
fn live_rate_of_growth_map_tracks_xpop_delta() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 0x70);
    let first_pop = live_population_density_map_for_city(&city);
    let first_growth = live_rate_of_growth_map_for_city(&city, &first_pop);
    let index = density_map_index(20, 20);
    assert_eq!(
        first_growth[index], RATE_OF_GROWTH_NEUTRAL,
        "first pass with no prior XPOP keeps neutral XROG"
    );

    city.sync_population_density_from_slice(&first_pop);
    city.sync_rate_of_growth_from_slice(&first_growth);

    // Growth: weight-1 → weight-4 raises XPOP 4 → 16.
    city.set_building(20, 20, 0xae);
    let second_pop = live_population_density_map_for_city(&city);
    let second_growth = live_rate_of_growth_map_for_city(&city, &second_pop);
    assert!(
        second_growth[index] > RATE_OF_GROWTH_NEUTRAL,
        "rising XPOP should raise XROG ({})",
        second_growth[index]
    );
    assert_eq!(
        second_growth[index],
        pe_rate_of_growth_step(RATE_OF_GROWTH_NEUTRAL, first_pop[index], second_pop[index])
    );

    // Decline back toward empty should cool growth.
    city.sync_population_density_from_slice(&second_pop);
    city.sync_rate_of_growth_from_slice(&second_growth);
    city.set_building(20, 20, 0);
    let third_pop = live_population_density_map_for_city(&city);
    let third_growth = live_rate_of_growth_map_for_city(&city, &third_pop);
    assert!(
        third_growth[index] < second_growth[index],
        "falling XPOP should lower XROG ({} vs {})",
        third_growth[index],
        second_growth[index]
    );
}

#[test]
fn density_phase_publishes_population_and_growth_in_native_order_for_both_engines() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for fidelity in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
            let mut city =
                City::new_blank("/tmp/density-phase-order.sc2", "Growth").expect("blank city");
            city.kind = kind;
            city.set_building(20, 20, 0xae); // XPOP weight 4 -> 16.
            city.set_traffic_density(20, 20, 29);
            city.sync_population_density_from_slice(&vec![64; CITY_TILES]);
            city.sync_rate_of_growth_from_slice(&vec![127; CITY_TILES]);
            let mut state = test_state(city);
            state.engine_fidelity = fidelity;

            state.sim_calendar_day2_density(
                PopulationStats::default(),
                CityServiceOutcomes::default(),
            );
            assert_eq!(
                state.city.population_density_at(20, 20),
                Some(16),
                "{kind:?} {fidelity:?}"
            );
            assert_eq!(
                state.city.rate_of_growth_at(20, 20),
                Some(79),
                "{kind:?} {fidelity:?}"
            );
            assert_eq!(state.city.traffic_density_at(20, 20), Some(29));

            // With the new XPOP now persisted, a second scan has zero delta;
            // XROG moves from 79 towards its neutral baseline, to 85.
            state.sim_calendar_day2_density(
                PopulationStats::default(),
                CityServiceOutcomes::default(),
            );
            assert_eq!(state.city.population_density_at(20, 20), Some(16));
            assert_eq!(
                state.city.rate_of_growth_at(20, 20),
                Some(85),
                "{kind:?} {fidelity:?}"
            );
            assert_eq!(state.city.traffic_density_at(20, 20), Some(29));

            let extension = if kind == CityFileKind::WinSc2 {
                "sc2"
            } else {
                "city"
            };
            let path = std::env::temp_dir().join(format!(
                "citygame-density-phase-{}-{kind:?}-{fidelity:?}.{extension}",
                std::process::id(),
            ));
            state.city.save(&path).expect("save published planes");
            let loaded = City::load(&path).expect("reload published planes");
            assert_eq!(
                loaded.population_density_at(20, 20),
                Some(16),
                "{kind:?} {fidelity:?}"
            );
            assert_eq!(
                loaded.rate_of_growth_at(20, 20),
                Some(85),
                "{kind:?} {fidelity:?}"
            );
            std::fs::remove_file(path).expect("remove temporary city");
        }
    }
}

#[test]
fn pe_coverage_rings_decay_from_station_center() {
    // Mac `0x28884` / PE `FUN_0046ba90`: 100 → 80 → 60 → 40 → 20.
    let mut micro = vec![0_u8; 32 * 32];
    paint_pe_coverage_rings(&mut micro, 10, 10, 100);
    let at = |x: i32, y: i32| micro[(x as usize) * 32 + (y as usize)];
    assert_eq!(at(10, 10), 100);
    assert_eq!(at(11, 10), 80); // *4/5 cardinals
    assert_eq!(at(9, 10), 80);
    assert_eq!(at(10, 11), 80);
    assert_eq!(at(10, 9), 80);
    assert_eq!(at(11, 11), 60); // *3/4 diagonals
    assert_eq!(at(12, 10), 40); // *2/3 r=2
    assert_eq!(at(12, 11), 40);
    assert_eq!(at(13, 10), 20); // /2 outer
    assert_eq!(at(12, 12), 20); // outer corner (±2,±2)
    assert_eq!(at(0, 0), 0);
}

#[test]
fn pe_coverage_rings_saturate_like_mac_28814() {
    let mut micro = vec![0_u8; 32 * 32];
    paint_pe_coverage_rings(&mut micro, 10, 10, 200);
    paint_pe_coverage_rings(&mut micro, 10, 10, 200);
    assert_eq!(
        micro[10 * 32 + 10],
        255,
        "Mac 0x28814 clamps summed coverage to 255"
    );
    // Center outside [0,32): only in-bounds ring cells paint; OOB samples are skipped.
    let before = micro[31 * 32 + 31];
    paint_pe_coverage_rings(&mut micro, 40, 40, 100);
    assert_eq!(micro[31 * 32 + 31], before);

    let mut negative = vec![0_u8; 32 * 32];
    negative[10 * 32 + 10] = 80;
    paint_pe_coverage_rings(&mut negative, 10, 10, -120);
    assert_eq!(
        negative[10 * 32 + 10],
        0,
        "signed additions clamp below zero"
    );
}

#[test]
fn coverage_ring_addition_wraps_signed_word_before_clamping() {
    let mut micro = vec![0_u8; 32 * 32];
    micro[10 * 32 + 10] = 1;
    paint_pe_coverage_rings(&mut micro, 10, 10, i16::MAX);
    assert_eq!(micro[10 * 32 + 10], 0, "32767 + 1 wraps negative");

    let mut below_boundary = vec![0_u8; 32 * 32];
    below_boundary[10 * 32 + 10] = 1;
    paint_pe_coverage_rings(&mut below_boundary, 10, 10, i16::MAX - 1);
    assert_eq!(below_boundary[10 * 32 + 10], 255);
}

#[test]
fn pe_service_coverage_uses_raw_funding_and_signed_prison_word() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-raw-service-funding.sc2", "test").expect("blank city");
    city.set_building(24, 20, 211);
    city.set_zone(24, 20, 0x80);
    city.set_tile_bits(24, 20, TileBits::new(0xc0));
    set_budget_funding_for_test(&mut city, 1, 120);
    let power_maps = power_service_maps(&city);
    let services = rci_development_services_for_city_with_power(&city, &power_maps);
    let fire = super::super::density::live_fire_coverage_map_for_city_with_pe_xbit(
        &city,
        &power_maps,
        &services,
        true,
    );
    // The PE ring writer receives signed WORD 300. Its center saturates,
    // while the first cardinal ring retains 4/5 of the unclamped input.
    assert_eq!(fire[density_map_index(20, 20)], 240);

    // The PE unpowered branch sign-extends the base's low WORD before halving:
    // 26,334% produces DWORD 65,835, low WORD 299, then strength 149.
    city.set_tile_bits(24, 20, TileBits::new(0x80));
    set_budget_funding_for_test(&mut city, 1, 26_334);
    let power_maps = power_service_maps(&city);
    let services = rci_development_services_for_city_with_power(&city, &power_maps);
    let fire = super::super::density::live_fire_coverage_map_for_city_with_pe_xbit(
        &city,
        &power_maps,
        &services,
        true,
    );
    assert_eq!(fire[density_map_index(24, 20)], 149);

    city.set_building(24, 20, 210);
    city.set_tile_bits(24, 20, TileBits::new(0xc0));
    city.set_pe_w_prison_bonus(u16::MAX); // PE MOVSX reads this as -1.
    set_budget_funding_for_test(&mut city, 0, 120);
    let power_maps = power_service_maps(&city);
    let services = rci_development_services_for_city_with_power(&city, &power_maps);
    let police = super::super::density::live_police_coverage_map_for_city_with_pe_xbit(
        &city,
        &power_maps,
        &services,
        true,
    );
    assert_eq!(police[density_map_index(20, 20)], 192);
}

#[test]
fn mac_service_coverage_uses_saved_anchor_and_same_tile_power_bit() {
    let mut city = City::new_blank("/tmp/sc2000-mac-service.sc2", "test").expect("blank city");
    city.set_building(24, 20, 210);
    city.set_zone(24, 20, 0x81);
    let center = density_map_index(24, 20);

    let mut unmarked = city.clone();
    unmarked.set_zone(24, 20, 0x01);
    let unmarked_coverage = super::super::density::live_mac_service_coverage_map(
        &unmarked,
        210,
        EngineFidelity::Mac68k,
    );
    assert!(unmarked_coverage.iter().all(|value| *value == 0));

    let unpowered_coverage =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert!(unpowered_coverage[center] > 0);

    city.set_tile_bits(24, 20, TileBits::new(0x40));
    let powered_coverage =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert!(powered_coverage[center] > unpowered_coverage[center]);
    assert_eq!(powered_coverage[center], unpowered_coverage[center] * 2);
}

#[test]
fn mac_service_coverage_matches_native_68020_worker_planes() {
    const NATIVE: &str =
        include_str!("../../../tests/fixtures/simulation/mac/mac_service_worker.csv");
    const MICRO: usize = 32;
    let rows: Vec<_> = NATIVE.lines().skip(1).collect();
    let cases = [
        (
            "police_powered",
            210_u8,
            true,
            None,
            0xd2_u8,
            &[(44_usize, 20_usize)][..],
        ),
        ("police_unpowered", 210, false, None, 0xd2, &[(44, 20)]),
        ("fire_powered", 211, true, None, 0xd3, &[(44, 20)]),
        ("fire_unpowered", 211, false, None, 0xd3, &[(44, 20)]),
        (
            "neighborhood_watch_baseline",
            210,
            false,
            Some(ORDINANCE_NEIGHBORHOOD_WATCH),
            0x70,
            &[(44, 20)],
        ),
        (
            "volunteer_fire_baseline",
            211,
            false,
            Some(ORDINANCE_VOLUNTEER_FIRE_DEPT),
            0x70,
            &[(44, 20)],
        ),
        ("police_northwest_clip", 210, true, None, 0xd2, &[(4, 4)]),
        ("fire_southeast_clip", 211, true, None, 0xd3, &[(124, 124)]),
        (
            "police_overlapping_stations",
            210,
            true,
            None,
            0xd2,
            &[(44, 20), (48, 20)],
        ),
    ];
    let decode_hex = |value: &str| -> Vec<u8> {
        assert_eq!(value.len(), MICRO * MICRO * 2);
        (0..value.len())
            .step_by(2)
            .map(|offset| u8::from_str_radix(&value[offset..offset + 2], 16).expect("native byte"))
            .collect()
    };

    for (name, building, powered, ordinance, tile, anchors) in cases {
        let native_row = rows
            .iter()
            .find(|row| row.split(',').next() == Some(name))
            .unwrap_or_else(|| panic!("missing native fixture {name}"));
        let fields: Vec<_> = native_row.split(',').collect();
        let expected_xplc = decode_hex(fields[6]);
        let expected_xfir = decode_hex(fields[7]);

        let mut city =
            City::new_blank("/tmp/sc2000-native-service-parity.sc2", "test").expect("blank city");
        city.kind = CityFileKind::MacCity;
        let budget_index = usize::from(building == 211);
        set_budget_funding_for_city_test(&mut city, budget_index, 100);
        for &(x, y) in anchors {
            city.set_building(x, y, tile);
            if tile == building {
                city.set_zone(x, y, 0x80);
                if powered {
                    city.set_tile_bits(x, y, TileBits::new(0x40));
                }
            }
        }
        if let Some(ordinance) = ordinance {
            assert!(set_city_ordinance_enabled_with_fidelity(
                &mut city,
                ordinance,
                true,
                EngineFidelity::Mac68k,
            ));
        }

        for (service_building, expected) in [(210_u8, &expected_xplc), (211_u8, &expected_xfir)] {
            let actual = super::super::density::live_mac_service_coverage_map(
                &city,
                service_building,
                EngineFidelity::Mac68k,
            );
            let mut actual_micro = Vec::with_capacity(MICRO * MICRO);
            for x in 0..MICRO {
                for y in 0..MICRO {
                    actual_micro.push(actual[density_map_index(x * 4, y * 4)]);
                }
            }
            assert_eq!(
                &actual_micro, expected,
                "CityGame Mac service {service_building} differs from native CODE 14: {name}"
            );
        }
    }
}

#[test]
fn mac_service_coverage_interleaves_ordinance_and_station_in_scan_order() {
    let coverage = |station_y: usize| {
        let mut city =
            City::new_blank("/tmp/mac-service-order.sc2", "Service").expect("blank city");
        city.kind = CityFileKind::MacCity;
        assert!(set_city_ordinance_enabled_with_fidelity(
            &mut city,
            ORDINANCE_NEIGHBORHOOD_WATCH,
            true,
            EngineFidelity::Mac68k,
        ));
        set_budget_funding_for_city_test(&mut city, 0, 100);
        for y in 20..24 {
            if y == station_y {
                city.set_building(24, y, 210);
                city.set_zone(24, y, 0x81);
                city.set_tile_bits(24, y, TileBits::new(0x40));
            } else {
                city.set_building(24, y, 0x70);
            }
        }
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k)
            [density_map_index(24, 20)]
    };
    // A funded station adds 250; three developed tiles add 2 apiece only
    // while the cell is below 254. Order is visible at the saturation edge.
    assert_eq!(coverage(20), 254);
    assert_eq!(coverage(23), 255);

    // The first coordinate is the outer loop: this station at x=24 is
    // visited before the three x=25 tiles despite its larger y coordinate.
    let mut city =
        City::new_blank("/tmp/mac-service-axis-order.sc2", "Service").expect("blank city");
    city.kind = CityFileKind::MacCity;
    assert!(set_city_ordinance_enabled_with_fidelity(
        &mut city,
        ORDINANCE_NEIGHBORHOOD_WATCH,
        true,
        EngineFidelity::Mac68k,
    ));
    set_budget_funding_for_city_test(&mut city, 0, 100);
    city.set_building(24, 23, 210);
    city.set_zone(24, 23, 0x81);
    city.set_tile_bits(24, 23, TileBits::new(0x40));
    for y in 20..23 {
        city.set_building(25, y, 0x70);
    }
    let map =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert_eq!(map[density_map_index(24, 20)], 254);
}

#[test]
fn mac_service_coverage_uses_saved_funding_dword_without_ui_clamp() {
    let coverage_at = |funding: u32, powered: bool| {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-service-raw-funding.sc2", "test").expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_building(24, 20, 210);
        city.set_zone(24, 20, 0x81);
        if powered {
            city.set_tile_bits(24, 20, TileBits::new(0x40));
        }

        let row_offset = budget_row_misc_offset_for_city(&city, BUDGET_SERVICE_ROW_START, 0)
            .expect("Mac police budget row");
        let mut row = vec![0; BUDGET_SERVICE_MISC_STRIDE];
        row[BUDGET_SERVICE_FUNDING_FIELD] = funding;
        city.set_budget_misc_u32_values(row_offset, &row);

        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k)
            [density_map_index(24, 20)]
    };

    // The Mac coverage worker multiplies the saved row dword directly. The
    // UI would clamp this value to 100, but imported/raw state at 120 produces
    // 300 full strength (saturated to 255 by AddQMap), or 150 when unpowered.
    assert_eq!(coverage_at(100, true), 250);
    assert_eq!(coverage_at(120, true), 255);
    assert_eq!(coverage_at(100, false), 125);
    assert_eq!(coverage_at(120, false), 150);
    // Native passes the low WORD even when the wrapped long result is
    // negative: (5 * 0x20000002 as i32) / 2 has low WORD 5.
    assert_eq!(coverage_at(0x2000_0002, true), 5);
}

#[test]
fn mac_phase0_prison_gate_feeds_phase2_police_coverage() {
    const MAC_XBLD_CENSUS_BASE: usize = 0x01f0;
    const MAC_POLICE_GATE_MISC_OFFSET: usize = 1039 * 4;

    let mut city =
        City::new_blank("/tmp/sc2000-mac-prison-police-order.sc2", "test").expect("blank city");
    city.set_building(24, 20, 210);
    city.set_zone(24, 20, 0x81);
    city.set_tile_bits(24, 20, TileBits::new(0x40));
    for y in 40..44 {
        for x in 40..44 {
            city.set_building(x, y, 0xd8);
        }
    }
    // The fixture is authored in PE layout to avoid decrementing an
    // uninitialized Mac census, then switches to the native Mac document
    // layout and supplies the two source-owned census entries explicitly.
    city.kind = CityFileKind::MacCity;
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_BASE + 0xd2 * 4, 1);
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS_BASE + 0xd8 * 4, 16);
    city.set_mac_state_misc_u32_at(0x102c, 10_000);

    let police_row = city
        .provision_microsim_for_building(0xd2)
        .expect("police XMIC row");
    let prison_row = city
        .provision_microsim_for_building(0xd8)
        .expect("prison XMIC row");
    assert!(city.set_microsim_stat1(usize::from(prison_row - MIN_SIM_TEXT_ENTRIES), 10_000));

    let police_budget = budget_row_misc_offset_for_city(&city, BUDGET_SERVICE_ROW_START, 0)
        .expect("Mac police budget row");
    let mut budget_values = vec![0; BUDGET_SERVICE_MISC_STRIDE];
    budget_values[BUDGET_SERVICE_FUNDING_FIELD] = 100;
    city.set_budget_misc_u32_values(police_budget, &budget_values);

    assert_eq!(
        city.microsim_at(usize::from(police_row - MIN_SIM_TEXT_ENTRIES))
            .unwrap()
            .tile_id,
        0xd2
    );
    assert_eq!(
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k)
            [density_map_index(24, 20)],
        250,
        "zero gate gives five times funding divided by two"
    );

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    // At the December-to-January rollover, phase 0 runs the CODE 3 XMIC
    // pass. The prison row publishes the gate consumed in phase 2.
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MAC_POLICE_GATE_MISC_OFFSET),
        Some(1),
        "the low-density prison row publishes the police coverage gate"
    );

    state.sim_calendar_day2_density(PopulationStats::default(), CityServiceOutcomes::default());
    assert_eq!(
        state.city.police_coverage_at(24, 20),
        Some(255),
        "phase 2 consumes the phase-0 gate; the native 300 strength saturates to 255"
    );
}

#[test]
fn mac_phase1_power_bit_feeds_phase2_police_coverage() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-power-police-order.sc2", "test").expect("blank city");
        city.kind = kind;
        // On a Win-layout file this slot belongs to PE history. Mac fidelity
        // must keep its power summary in engine-owned state instead.
        let pe_history_sentinel = 0x1357_9bdf;
        if kind == CityFileKind::WinSc2 {
            city.set_misc_u32_at(0x1060, pe_history_sentinel);
        }
        city.set_building(10, 20, 0xc6);
        city.set_tile_bits(10, 20, TileBits::new(0x80));
        for x in 11..=12 {
            city.set_building(x, 20, if x == 12 { 210 } else { 0x60 });
            city.set_tile_bits(x, 20, TileBits::new(0x80));
        }
        city.set_zone(12, 20, 0x81);
        let police_budget = budget_row_misc_offset_for_city(&city, BUDGET_SERVICE_ROW_START, 0)
            .expect("Mac police budget row");
        let mut budget_values = vec![0; BUDGET_SERVICE_MISC_STRIDE];
        budget_values[BUDGET_SERVICE_FUNDING_FIELD] = 100;
        city.set_budget_misc_u32_values(police_budget, &budget_values);

        assert_eq!(
            super::super::density::live_mac_service_coverage_map(
                &city,
                210,
                EngineFidelity::Mac68k,
            )[density_map_index(12, 20)],
            125,
            "the powerable but not-yet-powered anchor receives half strength ({kind:?})"
        );

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        // CODE 14 phase 1 rebuilds transient XBIT before phase 2 scans XZON
        // and uses the resulting same-tile 0x40 marker for full strength.
        state.sim_calendar_day1_power();
        assert!(
            state
                .city
                .tile_bits_at(12, 20)
                .is_some_and(|bits| bits.raw_bits() & 0x40 != 0),
            "the connected Mac power flood must mark the station powered ({kind:?})"
        );
        state.sim_calendar_day2_density(PopulationStats::default(), CityServiceOutcomes::default());
        assert_eq!(
            state.city.police_coverage_at(12, 20),
            Some(250),
            "phase 2 consumes phase 1's powered marker and doubles the ring strength ({kind:?})"
        );
        if kind == CityFileKind::WinSc2 {
            assert_eq!(
                state.city.misc_u32_at(0x1060),
                Some(pe_history_sentinel),
                "Mac power publication must preserve the PE-owned MISC slot"
            );
        }
    }
}

#[test]
fn mac_native_phase1_to_phase2_join_matches_power_and_police_planes() {
    const C8_NATIVE: &str =
        include_str!("../../../tests/fixtures/simulation/mac/mac_power_c8_to_service.csv");
    const CC_NATIVE: &str =
        include_str!("../../../tests/fixtures/simulation/mac/mac_power_cc_to_service.csv");
    assert_mac_native_phase1_to_phase2_join(
        C8_NATIVE,
        "c8_phase0_prefix_power_to_police",
        0xc8,
        57,
        45,
    );
    assert_mac_native_phase1_to_phase2_join(
        CC_NATIVE,
        "cc_phase0_prefix_power_to_police",
        0xcc,
        57,
        45,
    );
}

fn assert_mac_native_phase1_to_phase2_join(
    native: &str,
    expected_case: &str,
    source_building: u8,
    wind: u32,
    humidity: u32,
) {
    const MICRO: usize = 32;
    const ENTRY_SEED: u32 = 1_889_252_988;
    let mut lines = native.lines();
    let headers: Vec<_> = lines
        .next()
        .expect("native replay header")
        .split(',')
        .collect();
    let fields: Vec<_> = lines
        .next()
        .expect("native replay row")
        .split(',')
        .collect();
    let value = |name: &str| {
        let index = headers
            .iter()
            .position(|header| *header == name)
            .expect("CSV column");
        fields[index]
    };
    assert_eq!(value("case"), expected_case);
    assert_eq!(value("entry_seed").parse::<u32>().unwrap(), ENTRY_SEED);
    if headers.contains(&"random_site_counts") {
        let expected_site = if source_building == 0xc8 {
            "0x25b8a:1"
        } else {
            "0x25b52:1"
        };
        assert_eq!(value("random_site_counts"), expected_site);
    }
    let expected_summary = value("power_summary").parse::<u32>().unwrap();
    let expected_final_seed = value("final_seed").parse::<u32>().unwrap();
    let expected_xbit_hash = u64::from_str_radix(value("xbit_fnv1a64"), 16).unwrap();
    let expected_xplc_hash = u64::from_str_radix(value("xplc_fnv1a64"), 16).unwrap();
    let expected_xfir_hash = u64::from_str_radix(value("xfir_fnv1a64"), 16).unwrap();
    let expected_xbit = (0..CITY_TILES)
        .map(|index| {
            let raw = value("xbit_hex_row_major");
            u8::from_str_radix(&raw[index * 2..index * 2 + 2], 16).unwrap()
        })
        .collect::<Vec<_>>();
    let decode_hex = |value: &str| -> Vec<u8> {
        assert_eq!(value.len(), MICRO * MICRO * 2);
        (0..value.len())
            .step_by(2)
            .map(|offset| u8::from_str_radix(&value[offset..offset + 2], 16).unwrap())
            .collect()
    };
    let expected_xplc = decode_hex(value("xplc_hex_row_major"));
    let expected_xfir = decode_hex(value("xfir_hex_row_major"));
    let fnv1a64 = |bytes: &[u8]| {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    assert_eq!(fnv1a64(&expected_xplc), expected_xplc_hash);
    assert_eq!(fnv1a64(&expected_xfir), expected_xfir_hash);

    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/sc2000-mac-native-power-service-join.sc2", "Mac join")
            .expect("blank city");
        city.kind = kind;
        city.set_building(10, 20, source_building);
        city.set_tile_bits(10, 20, TileBits::new(0x80));
        city.set_altitude(10, 20, 27, 0);
        city.set_mac_state_misc_u32_at(0x64, wind);
        city.set_mac_state_misc_u32_at(0x68, humidity);
        city.set_building(11, 20, 0x60);
        city.set_tile_bits(11, 20, TileBits::new(0x80));
        city.set_building(12, 20, 0xd2);
        city.set_zone(12, 20, 0x81);
        city.set_tile_bits(12, 20, TileBits::new(0x80));
        set_budget_funding_for_city_test(&mut city, 0, 100);

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(ENTRY_SEED);
        state.sim_calendar_day1_power();
        assert_eq!(state.mac_process_rng.seed(), expected_final_seed);

        let mut xbit = Vec::with_capacity(CITY_TILES);
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                xbit.push(state.city.tile_bits_at(x, y).map_or(0, TileBits::raw_bits));
            }
        }
        assert_eq!(xbit, expected_xbit, "phase-1 XBIT plane ({kind:?})");
        assert_eq!(
            fnv1a64(&xbit),
            expected_xbit_hash,
            "phase-1 XBIT ({kind:?})"
        );
        assert_eq!(state.city.mac_power_service_summary(), expected_summary);

        state.sim_calendar_day2_density(PopulationStats::default(), CityServiceOutcomes::default());
        let mut xplc = Vec::with_capacity(MICRO * MICRO);
        let mut xfir = Vec::with_capacity(MICRO * MICRO);
        for x in 0..MICRO {
            for y in 0..MICRO {
                xplc.push(state.city.police_coverage_at(x * 4, y * 4).unwrap_or(0));
                xfir.push(state.city.fire_coverage_at(x * 4, y * 4).unwrap_or(0));
            }
        }
        assert_eq!(xplc, expected_xplc, "phase-2 XPLC ({kind:?})");
        assert_eq!(xfir, expected_xfir, "phase-2 XFIR ({kind:?})");
        assert_eq!(fnv1a64(&xplc), expected_xplc_hash);
        assert_eq!(fnv1a64(&xfir), expected_xfir_hash);
    }
}

#[test]
fn mac_service_coverage_preserves_asymmetric_row_table_axes() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-service-asymmetric.sc2", "test").expect("blank city");
    // Keep the X and Y coordinates distinct: the native Mac row table indexes
    // its first coordinate as the plane row and its second as the byte offset.
    city.set_building(44, 20, 210);
    city.set_zone(44, 20, 0x81);

    let coverage =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);

    assert!(coverage[density_map_index(44, 20)] > 0);
    assert_eq!(
        coverage[density_map_index(20, 44)],
        0,
        "coverage must not transpose the native row-table coordinates"
    );
}

#[test]
fn mac_service_coverage_uses_one_rotated_anchor_even_for_partial_footprints() {
    let mut reference =
        City::new_blank("/tmp/sc2000-mac-service-anchor.sc2", "test").expect("blank city");
    reference.set_building(40, 40, 210);
    reference.set_zone(40, 40, 0x81);
    let expected = super::super::density::live_mac_service_coverage_map(
        &reference,
        210,
        EngineFidelity::Mac68k,
    );

    // The original Mac worker checks XZON bit 7 on each tile independently;
    // it does not validate a complete 3x3 station footprint. The four marker
    // rotations all contribute exactly one ring, even when the other station
    // tiles are present.
    for (anchor_x, anchor_y) in [(40, 40), (42, 40), (40, 42), (42, 42)] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-service-rotated.sc2", "test").expect("blank city");
        for y in 40..43 {
            for x in 40..43 {
                city.set_building(x, y, 210);
                city.set_zone(x, y, 0x01);
            }
        }
        city.set_zone(anchor_x, anchor_y, 0x81);
        assert_eq!(
            super::super::density::live_mac_service_coverage_map(
                &city,
                210,
                EngineFidelity::Mac68k,
            ),
            expected,
            "rotated anchor ({anchor_x}, {anchor_y}) should paint one ring"
        );

        // Removing the marked tile leaves an incomplete footprint with no
        // eligible anchor. Other surviving station tiles must not paint rings.
        city.set_building(anchor_x, anchor_y, 0);
        assert!(
            super::super::density::live_mac_service_coverage_map(
                &city,
                210,
                EngineFidelity::Mac68k,
            )
            .iter()
            .all(|value| *value == 0)
        );
    }
}

#[test]
fn mac_service_coverage_applies_ordinance_gated_developed_tile_baseline() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-service-baseline.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_building(20, 20, 0x70);
    city.set_building(21, 20, 0xc5);
    city.set_building(22, 20, 0xc6);
    city.set_building(23, 20, 0x6f);
    city.set_building(24, 20, 0xc7);
    let sample = density_map_index(20, 20);

    // Mac police baseline uses ordinance 11; fire uses ordinance 4. Each
    // XBLD 0x70 and 0xc5 are included; 0x6f, 0xc6 and 0xc7 are excluded.
    // The included developed tiles each add two to their coarse map cell.
    for (building, ordinance) in [(210, 11), (211, 4)] {
        let without = super::super::density::live_mac_service_coverage_map(
            &city,
            building,
            EngineFidelity::Mac68k,
        );
        assert_eq!(without[sample], 0);

        // Native Mac ordinance mask is MISC slot 1000 (+0x0fa0).
        city.set_mac_state_misc_u32_at(0x0fa0, 1 << ordinance);
        let with = super::super::density::live_mac_service_coverage_map(
            &city,
            building,
            EngineFidelity::Mac68k,
        );
        assert_eq!(with[sample], 4);
        assert!(
            with.iter()
                .filter(|value| **value != 0)
                .all(|value| *value == 4)
        );

        city.set_mac_state_misc_u32_at(0x0fa0, 0);
    }
}

#[test]
fn pe_service_coverage_projects_positive_tile_coordinates_by_floor_division() {
    let mut city =
        City::new_blank("/tmp/pe-service-projection.sc2", "Service").expect("blank city");
    city.set_building(25, 21, 210);
    city.set_zone(25, 21, 0x80);
    city.set_tile_bits(25, 21, TileBits::new(0xc0));
    set_budget_funding_for_test(&mut city, 0, 20);
    let power = power_service_maps(&city);
    let services = rci_development_services_for_city_with_power(&city, &power);
    let station = super::super::density::live_police_coverage_map_for_city_with_pe_xbit(
        &city, &power, &services, true,
    );
    assert_eq!(station[density_map_index(24, 20)], 50);
    assert_eq!(station[density_map_index(28, 24)], 30);

    city.set_building(25, 21, 0x70);
    city.set_misc_u32_at(
        ORDINANCE_FLAGS_MISC_OFFSET,
        1 << ORDINANCE_NEIGHBORHOOD_WATCH,
    );
    let baseline = super::super::density::live_police_coverage_map_for_city_with_pe_xbit(
        &city, &power, &services, true,
    );
    assert_eq!(baseline[density_map_index(24, 20)], 2);
    assert_eq!(baseline[density_map_index(28, 24)], 0);

    // The scan includes tile 126: floor division keeps it in microcell 31.
    city.set_building(126, 126, 0x70);
    let edge = super::super::density::live_police_coverage_map_for_city_with_pe_xbit(
        &city, &power, &services, true,
    );
    assert_eq!(edge[density_map_index(124, 124)], 2);
}

#[test]
fn pe_service_coverage_adds_ordinance_baseline_only_for_developed_tiles() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-ordinance-service.sc2", "test").expect("blank city");
    // No stations or funding: the native ordinance baseline is independent
    // of the police/fire station-ring funding inputs.
    set_budget_funding_for_test(&mut city, 0, 0);
    set_budget_funding_for_test(&mut city, 1, 0);
    city.set_misc_u32_at(
        ORDINANCE_FLAGS_MISC_OFFSET,
        (1 << ORDINANCE_NEIGHBORHOOD_WATCH) | (1 << ORDINANCE_VOLUNTEER_FIRE_DEPT),
    );

    let cell_tiles = |city: &mut City, tile: u8| {
        for x in 20..=23 {
            for y in 20..=23 {
                city.set_building(x, y, tile);
            }
        }
    };
    let coverage = |city: &City| {
        let power_maps = power_service_maps(city);
        let services = rci_development_services_for_city_with_power(city, &power_maps);
        let police = super::super::density::live_police_coverage_map_for_city_with_pe_xbit(
            city,
            &power_maps,
            &services,
            true,
        );
        let fire = super::super::density::live_fire_coverage_map_for_city_with_pe_xbit(
            city,
            &power_maps,
            &services,
            true,
        );
        (police, fire)
    };
    let sample = density_map_index(20, 20);

    cell_tiles(&mut city, 0x70);
    let (police, fire) = coverage(&city);
    assert_eq!(police[sample], 32);
    assert_eq!(fire[sample], 32);

    cell_tiles(&mut city, 0xc5);
    let (police, fire) = coverage(&city);
    assert_eq!(police[sample], 32);
    assert_eq!(fire[sample], 32);

    cell_tiles(&mut city, 0x70);
    city.set_misc_u32_at(
        ORDINANCE_FLAGS_MISC_OFFSET,
        1 << ORDINANCE_NEIGHBORHOOD_WATCH,
    );
    let (police, fire) = coverage(&city);
    assert_eq!(police[sample], 32);
    assert_eq!(fire[sample], 0);

    city.set_misc_u32_at(
        ORDINANCE_FLAGS_MISC_OFFSET,
        1 << ORDINANCE_VOLUNTEER_FIRE_DEPT,
    );
    let (police, fire) = coverage(&city);
    assert_eq!(police[sample], 0);
    assert_eq!(fire[sample], 32);

    city.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, 0);
    let (police, fire) = coverage(&city);
    assert_eq!(police[sample], 0);
    assert_eq!(fire[sample], 0);

    city.set_misc_u32_at(
        ORDINANCE_FLAGS_MISC_OFFSET,
        (1 << ORDINANCE_NEIGHBORHOOD_WATCH) | (1 << ORDINANCE_VOLUNTEER_FIRE_DEPT),
    );
    cell_tiles(&mut city, 0x6f);
    let (police, fire) = coverage(&city);
    assert_eq!(police[sample], 0);
    assert_eq!(fire[sample], 0);

    cell_tiles(&mut city, 0xc6);
    let (police, fire) = coverage(&city);
    assert_eq!(police[sample], 0);
    assert_eq!(fire[sample], 0);
}

#[test]
fn pe_service_coverage_reads_xzon_corner_and_same_tile_xbit_powered_bit() {
    let mut city = City::new_blank("/tmp/sc2000-pe-service.sc2", "test").expect("blank city");
    city.set_building(24, 20, 210);
    let center = density_map_index(24, 20);
    let coverage = |city: &City| {
        let power_maps = power_service_maps(city);
        let services = rci_development_services_for_city_with_power(city, &power_maps);
        super::super::density::live_police_coverage_map_for_city_with_pe_xbit(
            city,
            &power_maps,
            &services,
            true,
        )
    };

    let ineligible = coverage(&city);
    assert_eq!(ineligible[center], 0);

    city.set_tile_bits(24, 20, TileBits::new(0xc0));
    assert_eq!(coverage(&city)[center], 0);

    city.set_zone(24, 20, 0x80);
    city.set_tile_bits(24, 20, TileBits::new(0));
    let unpowered = coverage(&city);
    assert_eq!(unpowered[center], 125);

    city.set_tile_bits(24, 20, TileBits::new(0x40));
    let powered = coverage(&city);
    assert_eq!(powered[center], 250);
}

#[test]
fn pe_service_coverage_accumulates_each_marked_station_corner() {
    let make_coverage = |city: &City| {
        let power_maps = power_service_maps(city);
        let services = rci_development_services_for_city_with_power(city, &power_maps);
        super::super::density::live_police_coverage_map_for_city_with_pe_xbit(
            city,
            &power_maps,
            &services,
            true,
        )
    };

    let mut one_tile =
        City::new_blank("/tmp/sc2000-pe-service-single.sc2", "test").expect("blank city");
    one_tile.set_building(25, 21, 210);
    one_tile.set_zone(25, 21, 0x80);
    one_tile.set_tile_bits(25, 21, TileBits::new(0xc0));
    set_budget_funding_for_test(&mut one_tile, 0, 20);
    let one = make_coverage(&one_tile);

    let mut full_footprint = one_tile.clone();
    stamp_complete_building_for_test(&mut full_footprint, 24, 20, 210, 3);
    seed_service_coverage_footprint_for_test(&mut full_footprint, 24, 20, 3, true);
    let center = density_map_index(24, 20);
    assert_eq!(one[center], 50);
    assert_eq!(make_coverage(&full_footprint), one);

    stamp_complete_building_for_test(&mut full_footprint, 27, 20, 210, 3);
    seed_service_coverage_footprint_for_test(&mut full_footprint, 27, 20, 3, true);
    assert!(make_coverage(&full_footprint)[center] > one[center]);
}

#[test]
fn city_service_outcomes_scale_with_budget_funding() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 0x70);
    city.set_building(11, 10, 0x70);
    city.set_building(12, 10, 0x84);
    stamp_complete_building_for_test(&mut city, 6, 6, 210, 3);
    stamp_complete_building_for_test(&mut city, 6, 12, 211, 3);
    stamp_complete_building_for_test(&mut city, 12, 6, 209, 3);
    stamp_complete_building_for_test(&mut city, 12, 12, 214, 3);
    stamp_complete_building_for_test(&mut city, 9, 12, 13, 1);
    stamp_complete_building_for_test(&mut city, 13, 10, 236, 2);

    assert_eq!(budget_service_funding_percent_for_city(&city, 0), 100);
    let funded = city_service_outcomes_for_city(&city);

    for index in [0, 1, 2, 3, 4] {
        set_budget_funding_for_test(&mut city, index, 0);
    }
    for index in TRANSPORT_BUDGET_INDICES {
        set_budget_funding_for_test(&mut city, index, 0);
    }

    let unfunded = city_service_outcomes_for_city(&city);
    assert_eq!(budget_service_funding_percent_for_city(&city, 0), 0);
    assert!(unfunded.life_expectancy < funded.life_expectancy);
    assert!(unfunded.education_quotient < funded.education_quotient);
    assert!(unfunded.service_score < funded.service_score);
    assert!(unfunded.city_value < funded.city_value);
    assert!(unfunded.land_value < funded.land_value);
    assert!(unfunded.land_value_quality < funded.land_value_quality);
}

#[test]
fn ordinances_influence_live_service_outcomes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 0x70);
    city.set_building(11, 10, 0x70);
    city.set_building(12, 10, 0x84);
    let population = PopulationStats {
        normal_population: 50_000,
        total_population: 50_000,
        populated_tiles: 3,
        residential_tiles: 2,
        commercial_tiles: 1,
        work_force: 12_000,
        ..Default::default()
    };

    let base = city_service_outcomes_for_population(&city, population, true);
    for index in [
        ORDINANCE_VOLUNTEER_FIRE_DEPT,
        ORDINANCE_FREE_CLINICS,
        ORDINANCE_PRO_READING_CAMPAIGN,
        ORDINANCE_NEIGHBORHOOD_WATCH,
        ORDINANCE_CITY_BEAUTIFICATION,
    ] {
        assert!(set_city_ordinance_enabled(&mut city, index, true));
    }
    let improved = city_service_outcomes_for_population(&city, population, true);

    assert!(improved.life_expectancy > base.life_expectancy);
    assert!(improved.education_quotient > base.education_quotient);
    assert!(improved.service_score > base.service_score);
    assert!(
        improved.city_value > base.city_value,
        "base={base:?} improved={improved:?}"
    );
    assert!(
        improved.land_value > base.land_value,
        "base={base:?} improved={improved:?}"
    );
    assert!(
        improved.land_value_quality > base.land_value_quality,
        "base={base:?} improved={improved:?}"
    );
}

#[test]
fn tax_rates_default_to_original_starting_rate_until_saved() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(tax_rate_for_city(&city, 0), DEFAULT_TAX_RATE);
    assert_eq!(tax_rate_for_city(&city, 1), DEFAULT_TAX_RATE);
    assert_eq!(tax_rate_for_city(&city, 2), DEFAULT_TAX_RATE);

    for (index, rate) in [(0, 6), (1, 9), (2, 11)] {
        set_misc_u32_for_test(&mut city, tax_rate_misc_offset(index).unwrap(), rate);
    }

    assert_eq!(tax_rate_for_city(&city, 0), 6);
    assert_eq!(tax_rate_for_city(&city, 1), 9);
    assert_eq!(tax_rate_for_city(&city, 2), 11);
}

#[test]
fn missing_industry_stats_default_tax_to_industrial_rate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    set_misc_u32_for_test(&mut city, tax_rate_misc_offset(2).unwrap(), 12);

    let stats = industry_stats_for_city(&city);

    assert!(stats.iter().all(|row| {
        *row == IndustryStats {
            ratio: 0,
            tax: 12,
            demand: 0,
            population: 0,
        }
    }));
}

#[test]
fn mac_industry_rows_do_not_alias_native_summaries() {
    let mut city = City::new_blank("/tmp/mac-slot-separation.city", "Slots").unwrap();
    city.kind = CityFileKind::MacCity;
    for index in 0..INDUSTRY_VALUES {
        city.set_mac_state_misc_u32_at(INDUSTRY_MISC_OFFSET + 4 * index, 1000 + index as u32);
    }
    city.set_mac_arcology_resident_total(1234);
    city.set_mac_state_misc_u32_at(0x102c, 5678);
    city.set_mac_treatment_gate(1);
    city.set_mac_power_service_summary(89);
    city.set_mac_water_service_summary(76);
    city.set_mac_scenario_active(true);
    for index in 0..INDUSTRY_VALUES {
        assert_eq!(
            city.mac_state_misc_u32_at(INDUSTRY_MISC_OFFSET + 4 * index),
            Some(1000 + index as u32)
        );
    }
    assert_eq!(city.mac_state_misc_u32_at(0x1020), Some(1234));
    assert_eq!(city.mac_state_misc_u32_at(0x102c), Some(5678));
    assert_eq!(city.mac_state_misc_u32_at(0x104c), Some(1));
    assert_eq!(city.mac_state_misc_u32_at(0x1060), Some(89));
    assert_eq!(city.mac_state_misc_u32_at(0x1064), Some(76));
    assert!(city.mac_scenario_active());
}

#[test]
fn mac_phase21_industry_demand_matches_68020_word_overflow() {
    // Replayed original CODE 8 at 0x3912..0x3974, with four zero LFSR draws.
    let prior = [
        -32768_i32, -20000, -10923, -1, 0, 1, 10922, 10923, 20000, 32767, 100,
    ];
    let expected = [-8192_i32, 1384, 8191, 0, 0, 0, 8191, -8191, -1384, 8191, 75];
    let mut city = City::new_blank("/tmp/mac-industry-phase21.city", "Industry").unwrap();
    city.kind = CityFileKind::MacCity;
    // Use a target equal to the seeded row total to isolate WORD smoothing.
    city.set_mac_rci_counter(5, 1155);
    for (index, value) in prior.into_iter().enumerate() {
        let base = INDUSTRY_MISC_OFFSET + index * INDUSTRY_FIELD_COUNT * 4;
        city.set_mac_state_misc_u32_at(base, value as u32);
        city.set_mac_state_misc_u32_at(base + 4, 7);
        city.set_mac_state_misc_u32_at(base + 8, index as u32 + 100);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day21_demand_and_graphs();
    let rows = mac_industry_stats_for_city(&state.city);
    assert_eq!(rows.map(|row| row.demand), expected);
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.tax, 7);
        assert_eq!(row.population, index as i32 + 100);
    }
}

#[test]
fn mac_phase21_industry_demand_uses_both_city_layouts_and_survives_save() {
    for (name, expected_kind) in [
        ("mac-industry-cross-layout.sc2", CityFileKind::WinSc2),
        ("mac-industry-cross-layout.city", CityFileKind::MacCity),
    ] {
        let mut city = blank_city_layout_fixture(name, "Industry");
        city.set_mac_rci_counter(5, 1155);
        for index in 0..INDUSTRY_COUNT {
            let base = INDUSTRY_MISC_OFFSET + index * INDUSTRY_FIELD_COUNT * 4;
            city.set_misc_u32_at(base, 20_000);
            city.set_misc_u32_at(base + 4, 7 + index as u32);
            city.set_misc_u32_at(base + 8, 100 + index as u32);
        }
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.sim_calendar_day21_demand_and_graphs();
        assert!(
            mac_industry_stats_for_city(&state.city)
                .iter()
                .all(|row| row.demand == -1384)
        );
        state.sim_calendar_day21_demand_and_graphs();
        let rows = mac_industry_stats_for_city(&state.city);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                (row.demand, row.tax, row.population),
                (-1038, 7 + index as i32, 100 + index as i32),
                "{expected_kind:?} row {index}"
            );
        }
        let path = std::env::temp_dir().join(format!("citygame-{}-{name}", std::process::id()));
        state.city.save(&path).unwrap();
        let reloaded = City::load(&path).unwrap();
        assert_eq!(reloaded.kind, expected_kind);
        assert_eq!(
            mac_industry_stats_for_city(&reloaded).map(|row| row.demand),
            [-1038; INDUSTRY_COUNT]
        );
        for (index, row) in mac_industry_stats_for_city(&reloaded).iter().enumerate() {
            assert_eq!(
                (row.tax, row.population),
                (7 + index as i32, 100 + index as i32)
            );
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn mac_industry_tax_edit_updates_native_rows_and_rounded_budget_rate() {
    for (kind, extension) in [
        (CityFileKind::MacCity, "city"),
        (CityFileKind::WinSc2, "sc2"),
    ] {
        let mut city = City::new_blank("/tmp/mac-industry-edit.sc2", "MacIndustry").unwrap();
        city.kind = kind;
        for index in 0..INDUSTRY_COUNT {
            let base = INDUSTRY_MISC_OFFSET + index * INDUSTRY_FIELD_COUNT * 4;
            city.set_mac_state_misc_u32_at(base, (index as u32) + 100);
            city.set_mac_state_misc_u32_at(base + 4, 7);
            city.set_mac_state_misc_u32_at(base + 8, (index as u32) + 200);
        }
        city.set_mac_state_misc_u32_at(industry_tax_misc_offset(0).unwrap(), u32::MAX);
        city.set_mac_state_misc_u32_at(industry_tax_misc_offset(1).unwrap(), 25);
        if kind == CityFileKind::WinSc2 {
            // The 33-dword industry vector ends immediately before the PE
            // XBLD census; the Mac editor must preserve that adjacent owner.
            city.set_misc_u32_at(INDUSTRY_MISC_OFFSET + INDUSTRY_VALUES * 4, 0x1234_5678);
        }
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;

        assert!(state.adjust_industry_tax_rate(3, -1), "{kind:?}");
        let rows = mac_industry_stats_for_city(&state.city);
        assert_eq!(rows.map(|row| row.tax), [0, 20, 7, 6, 7, 7, 7, 7, 7, 7, 7]);
        assert_eq!(tax_rate_for_city(&state.city, 2), 8);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.demand, index as i32 + 100);
            assert_eq!(row.population, index as i32 + 200);
        }
        assert!(!state.adjust_industry_tax_rate(1, 1));
        let path = std::env::temp_dir().join(format!(
            "citygame-mac-industry-{}.{extension}",
            std::process::id()
        ));
        state.city.save(&path).expect("save Mac industry rows");
        let loaded = City::load(&path).expect("reload Mac industry rows");
        assert_eq!(loaded.kind, kind);
        assert_eq!(mac_industry_stats_for_city(&loaded), rows, "{kind:?}");
        assert_eq!(tax_rate_for_city(&loaded, 2), 8, "{kind:?}");
        if kind == CityFileKind::WinSc2 {
            assert_eq!(
                loaded.misc_u32_at(INDUSTRY_MISC_OFFSET + INDUSTRY_VALUES * 4),
                Some(0x1234_5678),
                "PE XBLD census remains intact"
            );
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn win95_industry_panel_and_editor_use_shared_mac_layout_rows() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-industry-vector.city", "MacIndustry").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let saved = (0..INDUSTRY_VALUES)
        .map(|index| 0x5a00_0000_u32.wrapping_add(index as u32))
        .collect::<Vec<_>>();
    for (index, value) in saved.iter().copied().enumerate() {
        city.set_mac_state_misc_u32_at(INDUSTRY_MISC_OFFSET + index * 4, value);
    }

    let rows = industry_stats_for_city(&city);
    let native_rows = mac_industry_stats_for_city(&city);
    assert_eq!(
        rows, native_rows,
        "shared saved rows decode through WORD lanes"
    );
    assert_eq!(
        rows.map(|row| row.ratio),
        [9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10]
    );

    ensure_industry_misc_defaults_before_extension(&mut city);
    let mut state = test_state(city);
    assert!(state.adjust_industry_tax_rate(3, 2));
    let actual = (0..INDUSTRY_VALUES)
        .map(|index| {
            state
                .city
                .mac_state_misc_u32_at(INDUSTRY_MISC_OFFSET + index * 4)
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let tax_lane = industry_tax_misc_offset(3).expect("valid row");
    let tax_index = (tax_lane - INDUSTRY_MISC_OFFSET) / 4;
    assert_eq!(actual[tax_index], 12);
    for index in 0..INDUSTRY_VALUES {
        if index != tax_index {
            assert_eq!(actual[index], saved[index], "preserve MISC dword {index}");
        }
    }
    let path = std::env::temp_dir().join(format!(
        "citygame-win95-mac-industry-{}.city",
        std::process::id()
    ));
    state.city.save(&path).expect("save edited Mac-layout city");
    let loaded = City::load(&path).expect("reload edited Mac-layout city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(industry_stats_for_city(&loaded)[3].tax, 12);
    std::fs::remove_file(path).unwrap();

    let mut empty_mac =
        City::new_blank("/tmp/sc2000-mac-industry-empty.sc2", "MacEmpty").expect("blank city");
    empty_mac.kind = CityFileKind::MacCity;
    ensure_industry_misc_defaults_before_extension(&mut empty_mac);
    for row in 0..INDUSTRY_COUNT {
        let base = INDUSTRY_MISC_OFFSET + row * INDUSTRY_FIELD_COUNT * 4;
        assert_eq!(empty_mac.mac_state_misc_u32_at(base), Some(0));
        assert_eq!(empty_mac.mac_state_misc_u32_at(base + 4), Some(7));
        assert_eq!(empty_mac.mac_state_misc_u32_at(base + 8), Some(0));
    }
}

#[test]
fn live_industry_stats_track_industrial_building_mix_and_preserve_taxes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut values = vec![0_u32; INDUSTRY_VALUES];
    for index in 0..INDUSTRY_COUNT {
        let base = index * INDUSTRY_FIELD_COUNT;
        values[base] = 99;
        values[base + 1] = (5 + index as i32) as u32;
        values[base + 2] = (-999_i32) as u32;
    }
    city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &values);
    city.set_building(10, 10, 0x84);
    city.set_building(11, 10, 0x84);
    city.set_building(12, 10, 0x86);
    city.set_building(13, 10, 0x9E);

    let population = population_stats_for_city(&city);
    let outcomes = city_service_outcomes_for_city(&city);
    let stats = live_industry_stats_for_city(&city, population, outcomes);

    assert_eq!(stats[0].ratio, 50);
    assert_eq!(stats[2].ratio, 25);
    assert_eq!(stats[4].ratio, 25);
    assert_eq!(stats.iter().map(|row| row.ratio).sum::<i32>(), 100);
    for (index, row) in stats.iter().enumerate() {
        assert_eq!(row.tax, 5 + index as i32);
        assert_ne!(row.demand, -999);
        assert_ne!(row.population, -999);
    }
    assert_eq!(stats[0].population, 2);
    assert_eq!(stats[2].population, 1);
    assert_eq!(stats[4].population, 1);
}

#[test]
fn live_industry_stats_apply_relative_industry_tax_pressure() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut values = vec![0_u32; INDUSTRY_VALUES];
    for index in 0..INDUSTRY_COUNT {
        let base = index * INDUSTRY_FIELD_COUNT;
        values[base] = 1;
        values[base + 1] = DEFAULT_TAX_RATE;
    }
    values[0 * INDUSTRY_FIELD_COUNT + 1] = MAX_TAX_RATE;
    city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &values);
    for x in 20..28 {
        city.set_zone(x, 20, ZONE_INDUSTRIAL_LIGHT);
    }

    let stats = live_industry_stats_for_city(
        &city,
        population_stats_for_city(&city),
        city_service_outcomes_for_city(&city),
    );

    assert_eq!(stats[0].tax, MAX_TAX_RATE as i32);
    assert_eq!(stats[1].tax, DEFAULT_TAX_RATE as i32);
    assert!(
        stats[0].demand < stats[1].demand,
        "higher industry-specific tax should suppress that industry's demand"
    );
}

#[test]
fn refresh_live_misc_preserves_industry_demand_pop_and_taxes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut values = vec![0_u32; INDUSTRY_VALUES];
    for index in 0..INDUSTRY_COUNT {
        let base = index * INDUSTRY_FIELD_COUNT;
        values[base] = 77;
        values[base + 1] = (3 + index as i32) as u32;
        values[base + 2] = 44;
    }
    city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &values);
    stamp_complete_building_for_test(&mut city, 20, 20, 0xBC, 3);
    let mut state = test_state(city);

    let (_population, _outcomes) = state.refresh_live_misc_current_values();
    let industry = state
        .city
        .misc_i32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES);

    for (index, row) in industry.chunks_exact(INDUSTRY_FIELD_COUNT).enumerate() {
        assert_eq!(row[0], 77);
        assert_eq!(row[1], 3 + index as i32);
        assert_eq!(row[2], 44);
    }
}

#[test]
fn industry_tax_adjustment_materializes_full_table_and_preserves_other_taxes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    set_misc_u32_for_test(&mut city, tax_rate_misc_offset(2).unwrap(), 12);
    city.set_building(10, 10, 0x87);
    let mut state = test_state(city);

    assert!(state.adjust_industry_tax_rate(3, 2));

    let industry = state
        .city
        .misc_i32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES);
    assert_eq!(industry.len(), INDUSTRY_VALUES);
    for index in 0..INDUSTRY_COUNT {
        let base = index * INDUSTRY_FIELD_COUNT;
        assert_eq!(industry[base + 1], if index == 3 { 14 } else { 12 });
    }
    assert_eq!(industry[3 * INDUSTRY_FIELD_COUNT + 1], 14);
    let live = live_industry_stats_for_city(
        &state.city,
        population_stats_for_city(&state.city),
        city_service_outcomes_for_city(&state.city),
    );
    assert_eq!(live[3].ratio, 100);

    assert!(state.adjust_industry_tax_rate(3, -99));
    assert_eq!(
        state
            .city
            .misc_i32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES)[3 * INDUSTRY_FIELD_COUNT + 1],
        0
    );
}

#[test]
fn city_info_industry_tax_adjustment_noop_at_clamps_without_side_effects() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.clear_dirty();
    let zero_delta_before = state
        .city
        .misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES);

    assert!(!state.adjust_industry_tax_rate(3, 0));
    assert!(
        state
            .city
            .misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES)
            == zero_delta_before,
        "no-op at the initial tax rate must leave its zero-filled MISC row intact"
    );
    assert!(!state.city.has_dirty_tiles());

    assert!(state.adjust_industry_tax_rate(3, MAX_TAX_RATE as i32));
    state.city.clear_dirty();
    let before = state
        .city
        .misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES);

    assert!(!state.adjust_industry_tax_rate(3, 1));
    assert_eq!(
        state
            .city
            .misc_u32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES),
        before
    );
    assert!(!state.city.has_dirty_tiles());
}

#[test]
fn live_crime_and_pollution_counts_prefer_density_map_sums() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, RESIDENTIAL_1X1_BUILDINGS[0]);
    let mut crime = vec![0u8; CITY_TILES];
    let mut pollution = vec![0u8; CITY_TILES];
    crime[density_map_index(10, 10)] = 40;
    pollution[density_map_index(12, 12)] = 80;
    city.sync_crime_density_from_slice(&crime);
    city.sync_pollution_density_from_slice(&pollution);
    let population = live_population_stats_for_city(&city);
    let outcomes = live_city_service_outcomes_for_city(&city);

    assert_eq!(
        live_crime_count_for_city(&city, population, outcomes),
        40,
        "crime MISC path should use XCRM microsim cells, not the pressure heuristic"
    );
    assert_eq!(
        live_pollution_count_for_city(&city, population),
        80,
        "pollution MISC path should use XPLT microsim cells, not the tile heuristic"
    );
}

#[test]
fn city_service_outcomes_fall_back_to_saved_misc_without_developed_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_CITY_VALUE_OFFSET, 123_456);
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 77);
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 81);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 99);

    assert_eq!(
        city_service_outcomes_for_city(&city),
        CityServiceOutcomes {
            life_expectancy: 81,
            education_quotient: 99,
            service_score: 0,
            city_value: 123_456,
            land_value: 77,
            land_value_quality: 77,
        }
    );
}

#[test]
fn dense_rci_development_updates_live_population_stats() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (zone_id, origin) in [
        (ZONE_RESIDENTIAL_DENSE, (21, 21)),
        (ZONE_COMMERCIAL_DENSE, (27, 21)),
        (ZONE_INDUSTRIAL_DENSE, (33, 21)),
    ] {
        stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1 + 1);
        water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 3);
        for (x, y) in building_footprint_from_origin(origin.0, origin.1, 3).unwrap() {
            city.set_zone(x, y, zone_id);
        }
        powered_line_corridor_for_test(&mut city, origin.0, origin.0 + 2, origin.1 - 1);
    }
    seed_pe_rci_demand_to_survive_months(&mut city, 2);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();
    state.handle_simulation_month_advanced();
    let population = population_stats_for_city(&state.city);

    // Vacant dense growth starts at 1×1 on every PE transit-ring tile.
    assert_eq!(population.residential_tiles, 7);
    assert_eq!(population.commercial_tiles, 7);
    assert_eq!(population.industrial_tiles, 7);
    assert_eq!(population.populated_tiles, 21);
    assert!(population.normal_population > 0);
    assert!(population.work_force > 0);
    assert_eq!(
        population.total_population,
        population.normal_population + population.arcology_population
    );
}

#[test]
fn monthly_simulation_persists_live_stats_to_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // The MISC-to-BSS loader and EQ/LE worker own these published values.
    // Live census/service estimates must not replace them during the month.
    city.set_misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET, 39);
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 61);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 72);
    for (zone_id, origin) in [
        (ZONE_RESIDENTIAL_DENSE, (21, 21)),
        (ZONE_COMMERCIAL_DENSE, (27, 21)),
        (ZONE_INDUSTRIAL_DENSE, (33, 21)),
    ] {
        stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1 + 1);
        power_adjacent_building_for_test(&mut city, origin.0, origin.1 - 1);
        water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 3);
        for (x, y) in building_footprint_from_origin(origin.0, origin.1, 3).unwrap() {
            city.set_zone(x, y, zone_id);
        }
    }
    let mut state = test_state(city);
    let saved_pe_city_value = state.city.pe_city_value.max(0) as u32;
    let saved_land_misc = state.city.misc_u32_at(MISC_LAND_VALUE_OFFSET);

    state.handle_simulation_month_advanced();
    let population = population_stats_for_city(&state.city);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let live_land_value = state
        .city
        .land_value_density_sum()
        .expect("live land value density");

    assert_eq!(outcomes.land_value, live_land_value);
    assert_eq!(
        outcomes.city_value,
        city_value_for_land_value(live_land_value)
    );
    assert!(outcomes.land_value_quality <= 100);
    // Land-value UI fallback is live, but no recurring city-value publisher
    // was found; preserve the finance-owned PE BSS value in MISC.
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_VALUE_OFFSET),
        Some(saved_pe_city_value)
    );
    assert_eq!(state.city.pe_city_value as u32, saved_pe_city_value);
    assert_eq!(
        state.city.misc_u32_at(MISC_LAND_VALUE_OFFSET),
        saved_land_misc
    );
    // Monthly advancement includes the day-2 density publication.
    assert!(state.city.pe_dw_city_land_value > 0);
    assert_eq!(
        state.city.misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET),
        Some(39)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET),
        Some(61)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET),
        Some(72)
    );
    assert_eq!(
        state
            .city
            .misc_u32_values(MISC_BUILDING_TILE_COUNTS_OFFSET, BUILDING_TILE_COUNT_VALUES),
        building_tile_counts_for_city(&state.city).to_vec()
    );
    assert_eq!(
        state
            .city
            .misc_u32_values(MISC_MILITARY_TILE_COUNTS_OFFSET, MILITARY_TILE_COUNT_VALUES),
        military_tile_counts_for_city(&state.city).to_vec()
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_NORMAL_POPULATION_OFFSET),
        Some(population.normal_population)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_SPORTS_TEAMS_OFFSET),
        Some(sports_teams_for_city(&state.city))
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_SUBWAY_COUNT_OFFSET),
        Some(u32::from(state.city.pe_xund_occupancy))
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_SPEED_OFFSET),
        Some(sc2k_speed_misc_value(state.traffic.speed()))
    );
    assert_eq!(
        state.city.misc_i32_values(RCI_DEMAND_MISC_OFFSET, 3),
        pe_rci_demand_values_from_previous(&state.city, population, [0, 0, 0]).to_vec(),
        "day 21 writes one PE demand step from the pre-month triple"
    );
    for index in 0..3 {
        let offset = tax_rate_misc_offset(index).expect("tax offset");
        assert_eq!(state.city.misc_u32_at(offset), Some(DEFAULT_TAX_RATE));
    }
    for index in 0..BUDGET_SERVICE_COUNT {
        let offset = budget_service_funding_misc_offset(index).expect("budget offset");
        assert_eq!(state.city.misc_u32_at(offset), Some(100));
    }
    let industry = state
        .city
        .misc_i32_values(INDUSTRY_MISC_OFFSET, INDUSTRY_VALUES);
    assert_eq!(
        industry.len(),
        INDUSTRY_VALUES,
        "day 21 PE industry mix must persist the 11 demand/tax/pop triples"
    );
}

#[test]
fn refresh_live_stats_persists_current_game_speed_to_misc() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Cheetah);

    state.refresh_live_misc_stats();

    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(4));
}

#[test]
fn monthly_simulation_persists_live_neighbor_connection_count() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET, 99);
    city.set_building(
        0,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_building(
        10,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);

    state.refresh_live_misc_stats();

    assert_eq!(
        state.city.misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET),
        Some(1)
    );
}

#[test]
fn refresh_live_stats_persists_generated_budget_service_rows() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    set_building_and_pe_count_for_test(
        &mut city,
        30,
        30,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);

    state.refresh_live_misc_stats();

    for (index, expected_current) in [(0, 1), (5, 1)] {
        assert_eq!(
            state.city.misc_u32_at(
                budget_service_misc_offset(index, BUDGET_SERVICE_CURRENT_FIELD).unwrap()
            ),
            Some(expected_current)
        );
        assert_eq!(
            state.city.misc_u32_at(
                budget_service_misc_offset(index, BUDGET_SERVICE_FUNDING_FIELD).unwrap()
            ),
            Some(100)
        );
        assert_eq!(
            state.city.misc_u32_at(
                budget_service_misc_offset(index, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap()
            ),
            Some(0)
        );
    }
}

#[test]
fn monthly_simulation_accrues_budget_rows_without_immediate_funds_settlement() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    stamp_complete_building_for_test(&mut city, 20, 20, 0xae, 3);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 3);

    state.handle_simulation_month_advanced();

    assert_eq!(state.city.city_funds(), 20_000);
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).unwrap()),
        Some(1)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap()),
        Some(0),
        "phase zero snapshots the entry row before rebuilding the new station cost"
    );
    assert_eq!(budget_service_funding_percent_for_city(&state.city, 0), 100);
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_row_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).unwrap()),
        Some(0),
        "a city with no live R/C/I population has a zero PE tax current"
    );
    let tax_values = state.city.misc_u32_values(
        budget_row_misc_offset(0, 0).unwrap(),
        BUDGET_SERVICE_MISC_STRIDE,
    );
    assert_eq!(tax_rate_for_city(&state.city, 0), DEFAULT_TAX_RATE);
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_row_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap()),
        Some(budget_row_year_to_date(0, &tax_values, 3))
    );
    state.game_clock = GameClock::new(1900, 4);
    state.handle_simulation_month_advanced();
    assert_eq!(state.city.city_funds(), 20_000);
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap()),
        Some(100),
        "the rebuilt service current enters the following monthly snapshot"
    );
}

#[test]
fn budget_year_rollover_settles_saved_year_to_date_into_city_funds() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(1_000);
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    assert!(set_city_auto_budget_enabled(&mut city, true));
    city.set_misc_u32_at(
        budget_row_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        900,
    );
    city.set_misc_u32_at(
        budget_service_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        1_200,
    );
    for (row, month, current, funding) in [
        (0, 1, 123, 7),
        (0, 11, 999, 9),
        (BUDGET_SERVICE_ROW_START, 1, 4, 63),
        (BUDGET_SERVICE_ROW_START, 11, 8, 87),
    ] {
        let (current_field, funding_field) = pe_budget_month_history_fields(month);
        city.set_misc_u32_at(budget_row_misc_offset(row, current_field).unwrap(), current);
        city.set_misc_u32_at(budget_row_misc_offset(row, funding_field).unwrap(), funding);
    }
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1901, 1);

    state.handle_simulation_month_advanced();

    assert_eq!(
        state.city.city_funds(),
        901,
        "tax contributes +1 and the signed police expense contributes -100"
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_row_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap()),
        Some(0)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap()),
        Some(0)
    );
    for (row, month, expected_current, expected_funding) in [
        (0, 1, 123, 7),
        (0, 11, 999, 9),
        (BUDGET_SERVICE_ROW_START, 1, 4, 63),
        (BUDGET_SERVICE_ROW_START, 11, 8, 87),
    ] {
        let (current_field, funding_field) = pe_budget_month_history_fields(month);
        assert_eq!(
            state
                .city
                .misc_u32_at(budget_row_misc_offset(row, current_field).unwrap()),
            Some(expected_current),
            "PE January settlement preserves month {month} current for row {row}"
        );
        assert_eq!(
            state
                .city
                .misc_u32_at(budget_row_misc_offset(row, funding_field).unwrap()),
            Some(expected_funding),
            "PE January settlement preserves month {month} funding for row {row}"
        );
    }
}

#[test]
fn win95_budget_settlement_wraps_city_funds_like_pe_add() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-budget-wrap.sc2", "BudgetWrap").expect("blank city");
    city.set_city_funds(i32::MAX - 5);
    city.set_misc_u32_at(
        budget_row_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        120 * 900,
    );
    let mut state = test_state(city);

    state.apply_budget_year_end_settlement();

    assert_eq!(
        state.city.city_funds(),
        (i32::MAX - 5).wrapping_add(120),
        "PE adds the settled tax amount to the funds DWORD with 32-bit wrap"
    );
}

#[test]
fn mac_budget_year_reset_clears_accumulators_and_preserves_month_history() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-budget-history.sc2", "MacBudget").expect("blank city");
    city.kind = CityFileKind::MacCity;
    for row in [0, BUDGET_SERVICE_ROW_START] {
        let raw_offset =
            budget_row_misc_offset_for_city(&city, row, BUDGET_SERVICE_YEAR_TO_DATE_FIELD)
                .expect("Mac budget accumulator offset");
        city.set_budget_misc_u32_values(raw_offset, &[0x1234_5678]);
        for (month, current, funding) in [(1, 123, 7), (11, 999, 9)] {
            let (current_field, funding_field) = pe_budget_month_history_fields(month);
            let current_offset = budget_row_misc_offset_for_city(&city, row, current_field)
                .expect("Mac month-current offset");
            let funding_offset = budget_row_misc_offset_for_city(&city, row, funding_field)
                .expect("Mac month-funding offset");
            city.set_budget_misc_u32_values(current_offset, &[current]);
            city.set_budget_misc_u32_values(funding_offset, &[funding]);
        }
    }

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.clear_budget_year_history();

    for row in [0, BUDGET_SERVICE_ROW_START] {
        let raw_offset =
            budget_row_misc_offset_for_city(&state.city, row, BUDGET_SERVICE_YEAR_TO_DATE_FIELD)
                .expect("Mac budget accumulator offset");
        assert_eq!(state.city.budget_misc_u32_at(raw_offset), Some(0));
        for (month, expected_current, expected_funding) in [(1, 123, 7), (11, 999, 9)] {
            let (current_field, funding_field) = pe_budget_month_history_fields(month);
            let current_offset = budget_row_misc_offset_for_city(&state.city, row, current_field)
                .expect("Mac month-current offset");
            let funding_offset = budget_row_misc_offset_for_city(&state.city, row, funding_field)
                .expect("Mac month-funding offset");
            assert_eq!(
                state.city.budget_misc_u32_at(current_offset),
                Some(expected_current)
            );
            assert_eq!(
                state.city.budget_misc_u32_at(funding_offset),
                Some(expected_funding)
            );
        }
    }
}

#[test]
fn budget_year_end_delta_subtracts_settled_bond_accumulator() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(
        budget_row_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        900,
    );
    city.set_misc_u32_at(
        budget_service_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        0,
    );
    let base = budget_year_end_delta_for_city(&city);

    assert!(append_bond_rate_for_city(
        &mut city,
        4 * BOND_RATE_SCALE // 4% → $400/year
    ));
    assert!(append_bond_rate_for_city(&mut city, 5 * BOND_RATE_SCALE));
    sync_bond_budget_row_for_city(&mut city, 0);
    let mut state = test_state(city);
    for month in 1..=12 {
        state.game_clock = GameClock::new(1900, month);
        state.accrue_current_budget_month();
    }

    assert_eq!(annual_bond_interest_for_city(&state.city), 900);
    assert_eq!(
        budget_year_end_delta_for_city(&state.city),
        base - annual_bond_interest_for_city(&state.city) as i32
    );
}

#[test]
fn simulation_day0_january_opens_budget_when_auto_budget_is_off() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    assert!(!auto_budget_enabled_for_city(&city));
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Llama);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.month(), 1);
    assert_eq!(state.game_clock.day(), 0);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
}

#[test]
fn simulation_january_month_keeps_budget_status_line() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    assert!(!auto_budget_enabled_for_city(&city));
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Llama);
    state.game_clock = GameClock::new(1901, 1);

    state.handle_simulation_month_advanced();

    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    assert_eq!(state.query_line, "Budget: review January funding");
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
}

#[test]
fn mac_native_auto_budget_turns_off_after_negative_annual_settlement() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/citygame-negative-budget.sc2", "MacBudget").unwrap();
        city.kind = kind;
        city.set_saved_date(CityDate {
            year: 1900,
            month: 12,
        });
        city.set_city_funds(1);
        let expense = budget_row_misc_offset_for_city(&city, 5, 2).unwrap();
        city.set_budget_misc_u32_values(expense, &[24]);
        assert!(set_auto_budget_enabled_for_engine(
            &mut city,
            EngineFidelity::Mac68k,
            true,
        ));

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.traffic.set_speed(GameSpeed::Llama);
        state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);

        assert_eq!(state.city.city_funds(), -1);
        assert!(!mac_auto_budget_enabled_for_city(&state.city));
        assert!(!state.menu_checks.auto_budget);
        assert_eq!(state.city_info_dialog, None);
        assert_eq!(state.traffic.speed(), GameSpeed::Llama);
        assert_eq!(state.query_line, "Fiscal crisis: Auto-Budget turned off");
    }
}

#[test]
fn issue_budget_bond_stores_integer_percent_and_syncs_scaled_bond_book() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.game_clock = GameClock::new(1900, 3);
    let expected_rate = bond_issue_rate_scaled_for_city(&state.city);

    assert!(state.issue_budget_bond());
    assert_eq!(state.city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(1));
    assert_eq!(
        state.city.misc_u32_at(BOND_DATA_MISC_OFFSET),
        Some(expected_rate / BOND_RATE_SCALE)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_row_misc_offset(BUDGET_BOND_ROW, 0).unwrap()),
        Some(1)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_row_misc_offset(BUDGET_BOND_ROW, 1).unwrap()),
        Some(expected_rate)
    );
}

#[test]
fn mac_issue_budget_bond_uses_native_city_value_and_tax_rate_formula() {
    const MAC_XBLD_CENSUS: usize = 0x01f0;
    let mut city = City::new_blank("/tmp/sc2000-mac-bond-issue-rate.sc2", "MacBondRate")
        .expect("blank Mac city");
    city.kind = CityFileKind::MacCity;
    // Two native $4,000 hydro census entries give city value 8,000. With one
    // existing bond the CODE 4 quotient is floor(25,000 / 8,001) = 3.
    city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS + 0xc6 * 4, 2);
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 7);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.issue_budget_bond());

    // FUN_0002b668 passes national-tax WORD + quotient + 1 to export_226.
    assert_eq!(state.city.pe_city_value, 8_000);
    assert_eq!(state.city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(2));
    assert_eq!(
        state.city.misc_u32_at(BOND_DATA_MISC_OFFSET + 4),
        Some(11),
        "7 national tax + 3 quotient + 1 percentage point"
    );
}

#[test]
fn bond_rate_storage_uses_low_word_for_mac_and_pe_native_tables() {
    let rate_offset = BOND_DATA_MISC_OFFSET;
    let mut pe =
        City::new_blank("/tmp/sc2000-pe-bond-rate-word.sc2", "PeBondRate").expect("blank PE city");
    pe.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 1);
    pe.set_misc_u32_at(rate_offset, 0xa5a5_0005);
    assert_eq!(bond_rates_scaled_for_city(&pe), vec![5 * BOND_RATE_SCALE]);

    let mut mac = City::new_blank("/tmp/sc2000-mac-bond-rate-word.sc2", "MacBondRate")
        .expect("blank Mac city");
    mac.kind = CityFileKind::MacCity;
    mac.set_mac_state_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 1);
    mac.set_mac_state_misc_u32_at(rate_offset, 0xbeef_0005);
    let mac_bond_row =
        budget_row_misc_offset_for_city(&mac, BUDGET_BOND_ROW, 0).expect("Mac bond row current");
    mac.set_budget_misc_u32_values(mac_bond_row, &[0x1357_9bdf]);

    // CODE 4 reads the low WORD from each serialized slot, then issue and
    // repayment mutate this same 50-entry native Mac rate table.
    assert_eq!(bond_rates_scaled_for_city(&mac), vec![5 * BOND_RATE_SCALE]);
    assert!(append_bond_rate_for_city(&mut mac, 6 * BOND_RATE_SCALE));
    assert_eq!(
        bond_rates_scaled_for_city(&mac),
        vec![5 * BOND_RATE_SCALE, 6 * BOND_RATE_SCALE]
    );
    assert!(remove_oldest_bond_rate_for_city(&mut mac));
    assert_eq!(bond_rates_scaled_for_city(&mac), vec![6 * BOND_RATE_SCALE]);
    assert_eq!(mac.budget_misc_u32_at(mac_bond_row), Some(0x1357_9bdf));

    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-bond-rate-word-{}.city",
        std::process::id()
    ));
    mac.save(&path).expect("save Mac bond-rate table");
    let loaded = City::load(&path).expect("reload Mac bond-rate table");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(
        bond_rates_scaled_for_city(&loaded),
        vec![6 * BOND_RATE_SCALE]
    );
    assert_eq!(loaded.budget_misc_u32_at(mac_bond_row), Some(0x1357_9bdf));
    let _ = fs::remove_file(path);
}

#[test]
fn pe_year_end_uses_the_saved_ordinance_row_accumulator() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // FUN_004734A0 settles row 3 through the same signed-divisor loop as all
    // other rows. Toggling ordinances after the year cannot recompute that
    // saved historical cash result from today's population/ordinance state.
    city.set_misc_u32_at(
        budget_row_misc_offset(BUDGET_ORDINANCE_ROW, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        1_800,
    );
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_SALES_TAX,
        true
    ));
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_INCOME_TAX,
        true
    ));
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_FREE_CLINICS,
        true
    ));

    assert_eq!(budget_year_end_delta_for_city(&city), 2);
}

#[test]
fn pe_ordinance_raw_cash_matches_win95_fun_0042bca0_table() {
    let r = 3_000;
    let c = 1_200;
    let i = 900;
    let arco = 50;
    let city_pop = 4_000;
    let cases = [
        (ORDINANCE_SALES_TAX, c),
        (ORDINANCE_INCOME_TAX, r),
        (ORDINANCE_LEGALIZED_GAMBLING, c * 2),
        (ORDINANCE_PARKING_FINES, r / 2),
        (ORDINANCE_VOLUNTEER_FIRE_DEPT, -(r / 3)),
        (ORDINANCE_PUBLIC_SMOKING_BAN, -(c / 6)),
        (ORDINANCE_FREE_CLINICS, -(r / 2)),
        (ORDINANCE_JUNIOR_SPORTS, -(r / 4)),
        (ORDINANCE_PRO_READING_CAMPAIGN, -(r / 6)),
        (ORDINANCE_ANTI_DRUG_CAMPAIGN, -(r / 5)),
        (ORDINANCE_CPR_TRAINING, -(r / 6)),
        (ORDINANCE_NEIGHBORHOOD_WATCH, -(r / 3)),
        (ORDINANCE_TOURIST_ADVERTISING, -c),
        (ORDINANCE_BUSINESS_ADVERTISING, -i),
        (ORDINANCE_CITY_BEAUTIFICATION, -(r / 4)),
        (ORDINANCE_ANNUAL_CARNIVAL, -(c / 3)),
        (ORDINANCE_ENERGY_CONSERVATION, -(arco + city_pop)),
        (ORDINANCE_NUCLEAR_FREE_ZONE, 0),
        (ORDINANCE_HOMELESS_SHELTER, -(r / 2)),
        (ORDINANCE_POLLUTION_CONTROLS, -i),
    ];
    for (index, expected_raw) in cases {
        assert_eq!(
            pe_ordinance_raw_cash_amount(index, r, c, i, arco, city_pop),
            expected_raw,
            "ordinance index {index}"
        );
        assert_eq!(
            i64::from(expected_raw) / i64::from(TAX_REVENUE_DIVISOR),
            i64::from(expected_raw) / 75,
            "cash divisor for index {index}"
        );
    }
}

#[test]
fn pe_ordinance_annual_cash_divides_enabled_raws_by_75() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 20, 20, 0xae, 3);
    city.set_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 100);

    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_PARKING_FINES,
        true
    ));
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_ENERGY_CONSERVATION,
        true
    ));
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_NUCLEAR_FREE_ZONE,
        true
    ));

    let population = population_stats_for_city(&city);
    let bases = budget_tax_bases_for_city(&city, population);
    let r = bases[0] as i32;
    let city_pop = pe_zone_pops_scaled_population(population) as i32;
    let arco = population.arcology_population as i32;
    city.set_pe_dw_arcology_population(arco as u32);
    city.set_pe_dw_city_population(city_pop as u32);
    let expected = i64::from(r / 2) / 75 + i64::from(-(arco + city_pop)) / 75;
    assert_eq!(
        pe_ordinance_annual_cash_for_city(&city, population),
        expected
    );
    assert_eq!(TAX_REVENUE_DIVISOR, 75);
}

#[test]
fn pe_ordinance_energy_conservation_uses_pe_dw_city_population() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 20, 20, 0xae, 3);
    city.set_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 60);
    city.set_pe_zone_pops([12, 0, 0]);
    city.persist_pe_day22_population_leftover(120);
    assert_eq!(city.pe_dw_city_population(), 120);
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_ENERGY_CONSERVATION,
        true
    ));

    let population = population_stats_for_city(&city);
    let arco = population.arcology_population as i32;
    let expected = i64::from(-(arco + 120)) / 75;
    assert_eq!(
        pe_ordinance_annual_cash_for_city(&city, population),
        expected,
        "ENERGY_CONSERVATION city_pop is dwCityPopulation, not census R+C+I"
    );
    assert_ne!(
        pe_zone_pops_scaled_population(population),
        120,
        "census people must differ so the pin is meaningful"
    );
}

#[test]
fn pe_ordinance_energy_conservation_uses_shared_population_on_mac_layout() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-pe-ordinance-mac-layout-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "CrossLayoutOrdinance").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 2_000);
    city.set_mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 12_000);
    city.set_mac_state_misc_u32_at(
        ORDINANCE_FLAGS_MISC_OFFSET,
        1 << ORDINANCE_ENERGY_CONSERVATION,
    );
    stamp_complete_building_for_test(&mut city, 20, 20, 0xae, 3);
    city.prepare_pe_fidelity_from_saved_city();

    assert_eq!(city.pe_dw_arcology_population(), 2_000);
    assert_eq!(city.pe_dw_city_population(), 12_000);
    assert!(
        !city.pe_zone_pops_ready(),
        "saved PE city population is independent of day-21 pZonePops readiness"
    );

    let population = population_stats_for_city(&city);
    let actual = pe_ordinance_annual_cash_for_city(&city, population);
    assert_eq!(actual, i64::from(-14_000_i32) / 75);
    assert_ne!(
        actual,
        i64::from(
            -(population.arcology_population as i32
                + pe_zone_pops_scaled_population(population) as i32)
        ) / 75,
        "PE formula must use its loaded shared scalar, not Mac census inputs"
    );
}

#[test]
fn win95_population_stats_use_shared_saved_population_on_mac_layout() {
    let path = std::env::temp_dir().join(format!(
        "sc2000-pe-population-mac-layout-{}.city",
        std::process::id()
    ));
    let mut city = City::new_blank(&path, "CrossLayoutPopulation").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 12_000);
    city.set_mac_state_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 2_000);
    stamp_complete_building_for_test(&mut city, 20, 20, 0xae, 3);
    city.prepare_pe_fidelity_from_saved_city();

    let census = population_stats_for_city(&city);
    let pe = population_stats_for_city_with_fidelity(&city, EngineFidelity::Win95);

    assert!(census.normal_population < 12_000);
    assert_eq!(pe.normal_population, 12_000);
    assert_eq!(pe.arcology_population, 2_000);
    assert_eq!(pe.total_population, 14_000);
    assert_eq!(pe.populated_tiles, census.populated_tiles);
    assert_eq!(pe.residential_tiles, census.residential_tiles);
    assert_eq!(pe.commercial_tiles, census.commercial_tiles);
    assert_eq!(pe.industrial_tiles, census.industrial_tiles);

    let mut win_city = City::new_blank(
        "/tmp/sc2000-mac-population-win-layout.sc2",
        "MacOnWinLayout",
    )
    .expect("blank Win-layout city");
    win_city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 90_000);
    win_city.set_misc_u32_at(MISC_PE_GROWTH_AGGREGATE_OFFSET, 4_000);
    stamp_complete_building_for_test(&mut win_city, 20, 20, 0xae, 3);
    let mac = population_stats_for_city_with_fidelity(&win_city, EngineFidelity::Mac68k);
    let live = live_population_stats_for_city(&win_city);
    assert_eq!(mac.normal_population, live.normal_population);
    assert_eq!(mac.populated_tiles, live.populated_tiles);
}

#[test]
fn budget_tax_annual_amount_uses_pe_divisor_75() {
    assert_eq!(budget_row_annual_amount(0, 7_500, 7), 7_500 * 7 / 75);
    assert_eq!(budget_row_annual_amount(1, 1_200, 20), 1_200 * 20 / 75);
    assert_eq!(budget_row_annual_amount(2, 900, 0), 0);
}

#[test]
fn pe_budget_year_to_date_retains_raw_month_products_until_settlement() {
    let mut values = [0_u32; BUDGET_SERVICE_MISC_STRIDE];
    // PE stores raw current*rate products in +0x68; normalization happens
    // only in the January row-settlement loop.
    values[3] = 47;
    values[4] = 19;
    for month in 1..12 {
        let (count_field, funding_field) = pe_budget_month_history_fields(month);
        values[count_field] = 37;
        values[funding_field] = 2;
    }

    assert_eq!(budget_row_year_to_date(0, &values, 12), 1_707);
}

#[test]
fn pe_budget_tax_bases_use_zone_pops_scaled_plus_arco_splits() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 0xae, 3); // R capacity 9×72
    stamp_complete_building_for_test(&mut city, 20, 20, 0xb2, 3); // C capacity 9×64
    stamp_complete_building_for_test(&mut city, 30, 30, 0xbc, 3); // I capacity 9×80
    city.set_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 600);
    // MISC normal/workforce must not invent tax bases when map census is present.
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 99_000);
    city.set_misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET, 90);

    let population = population_stats_for_city(&city);
    assert_eq!(population.residential_population, 648);
    assert_eq!(population.commercial_population, 576);
    assert_eq!(population.industrial_population, 720);
    assert_eq!(population.arcology_population, 600);
    assert_eq!(
        budget_tax_bases_for_city(&city, population),
        [648 + 600 / 6, 576 + 600 / 12, 720 + 600 / 12]
    );
}

#[test]
fn pe_budget_tax_bases_prefer_ready_p_zone_pops_over_census() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 0xae, 3);
    city.set_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 120);
    city.set_pe_zone_pops([40, 5, 7]);

    let population = population_stats_for_city(&city);
    assert!(population.residential_population > 400);
    assert_eq!(
        budget_tax_bases_for_city(&city, population),
        [40 * 10 + 120 / 6, 5 * 10 + 120 / 12, 7 * 10 + 120 / 12],
        "attitude / tax currents must use pZonePops*10 when day-21 rows are ready"
    );
}

#[test]
fn mac_ordinance_estimate_ignores_pe_arcology_population_mirror() {
    fn estimate(pe_arcology_cache: u32) -> i64 {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-ordinance-owner.sc2", "MacOwner").expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_mac_state_misc_u32_at(0x0fa0, 1 << ORDINANCE_ENERGY_CONSERVATION);
        city.set_mac_state_misc_u32_at(0x1020, 600);
        city.set_pe_dw_arcology_population(pe_arcology_cache);
        let population = live_population_stats_for_city(&city);
        ordinance_effects_for_city_with_fidelity(&city, population, EngineFidelity::Mac68k)
            .annual_expense
    }

    let native_population_source = estimate(0);
    assert!(native_population_source > 0);
    assert_eq!(
        estimate(12_000),
        native_population_source,
        "PE runtime cache must not replace the Mac resident total"
    );
}

#[test]
fn january_year_roll_attitude_tax_bases_use_ready_p_zone_pops() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        1,
    );
    city.set_pe_zone_pops([50, 20, 10]);
    city.set_traffic_density(10, 10, 100);
    let mut state = test_state(city);
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);

    let population = population_stats_for_city(&state.city);
    assert_eq!(
        budget_tax_bases_for_city(&state.city, population),
        [500, 200, 100]
    );
    let traffic = trip_generator::traffic_density_microsim_sum(&state.city);
    assert_eq!(state.city.pe_traffic_count(), traffic);
    state.city.recompute_pe_attitude_scores(100, 100, 100);
    assert_eq!(
        state.city.pe_attitude_score_at(0),
        traffic / 801,
        "score0 = pe_traffic / (pZonePops*10 sum + 1)"
    );
}

#[test]
fn pe_budget_tax_bases_empty_map_are_zero_without_arco() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let population = PopulationStats {
        normal_population: 50_000,
        work_force: 20_000,
        ..PopulationStats::default()
    };
    assert_eq!(budget_tax_bases_for_city(&city, population), [0, 0, 0]);
}

#[test]
fn pe_budget_tax_bases_arco_only_splits_across_rci() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_pe_dw_arcology_population(120);
    let population = PopulationStats {
        arcology_population: 120,
        ..PopulationStats::default()
    };
    assert_eq!(
        budget_tax_bases_for_city(&city, population),
        [120 / 6, 120 / 12, 120 / 12]
    );
}

#[test]
fn pe_budget_row_divisors_match_win95_table() {
    assert_eq!(
        PE_BUDGET_ROW_DIVISORS,
        [
            75, 75, 75, 75, -100, -1, -1, -2, -4, -1, -1000, -500, -400, -250, -250, -250
        ]
    );
    assert_eq!(TAX_REVENUE_DIVISOR, PE_BUDGET_ROW_DIVISORS[0] as u32);
}

#[test]
fn pe_budget_month_history_slots_match_fun_004734a0() {
    assert_eq!(pe_budget_month_index(1), 0);
    assert_eq!(pe_budget_month_index(3), 2);
    assert_eq!(pe_budget_month_index(12), 11);
    assert_eq!(pe_budget_month_history_fields(0), (3, 4));
    assert_eq!(pe_budget_month_history_fields(2), (7, 8));
    assert_eq!(pe_budget_month_history_fields(11), (25, 26));
}

#[test]
fn budget_tax_adjustment_persists_without_reaccruing_saved_months() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 20, 20, 0xae, 3);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 3);

    assert!(state.adjust_budget_tax_rate(0, 1));

    let offset = budget_row_misc_offset(0, 0).expect("tax row offset");
    let values = state
        .city
        .misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    assert_eq!(
        state
            .city
            .misc_u32_at(tax_rate_misc_offset(0).expect("tax rate offset")),
        Some(DEFAULT_TAX_RATE + 1)
    );
    assert_eq!(values[BUDGET_SERVICE_FUNDING_FIELD], DEFAULT_TAX_RATE + 1);
    let (_count_field, funding_field) = pe_budget_month_history_fields(pe_budget_month_index(3));
    assert_eq!(
        values[funding_field], 0,
        "monthly history is written by the PE month worker"
    );
    assert_eq!(values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD], 0);

    assert!(state.adjust_budget_tax_rate(0, 99));
    assert_eq!(tax_rate_for_city(&state.city, 0), MAX_TAX_RATE);
}

#[test]
fn budget_service_funding_adjustment_persists_without_reaccruing_current_month() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 3);

    assert!(state.adjust_budget_service_funding(0, -BUDGET_FUNDING_STEP));

    let offset = budget_service_misc_offset(0, 0).expect("police budget offset");
    let values = state
        .city
        .misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    assert_eq!(values[BUDGET_SERVICE_CURRENT_FIELD], 1);
    assert_eq!(
        values[BUDGET_SERVICE_FUNDING_FIELD],
        100 - BUDGET_FUNDING_STEP as u32
    );
    let (count_field, funding_field) = pe_budget_month_history_fields(pe_budget_month_index(3));
    assert_eq!(
        values[count_field], 0,
        "history waits for the monthly PE row worker"
    );
    assert_eq!(values[funding_field], 0);
    assert_eq!(values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD], 0);

    assert!(state.adjust_budget_service_funding(0, -999));
    assert_eq!(budget_service_funding_percent_for_city(&state.city, 0), 0);
}

#[test]
fn monthly_budget_preserves_manual_service_funding_when_auto_budget_is_off() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    set_budget_funding_for_test(&mut city, 0, 70);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 3);

    state.handle_simulation_month_advanced();

    assert!(!auto_budget_enabled_for_city(&state.city));
    assert_eq!(budget_service_funding_percent_for_city(&state.city, 0), 70);
    let values = state.city.misc_u32_values(
        budget_service_misc_offset(0, 0).expect("police budget offset"),
        BUDGET_SERVICE_MISC_STRIDE,
    );
    assert_eq!(values[4 + 2 * 2], 70);
}

#[test]
fn auto_budget_preserves_manual_service_funding_without_revenue() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    set_budget_funding_for_test(&mut city, 0, 80);
    assert!(set_city_auto_budget_enabled(&mut city, true));
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 3);

    state.handle_simulation_month_advanced();

    assert_eq!(budget_service_funding_percent_for_city(&state.city, 0), 80);
    let values = state.city.misc_u32_values(
        budget_service_misc_offset(0, 0).expect("police budget offset"),
        BUDGET_SERVICE_MISC_STRIDE,
    );
    assert_eq!(
        values[4 + 2 * 2],
        80,
        "phase zero snapshots the unchanged manual funding"
    );
    state.game_clock = GameClock::new(1900, 4);
    state.handle_simulation_month_advanced();
    let values = state.city.misc_u32_values(
        budget_service_misc_offset(0, 0).unwrap(),
        BUDGET_SERVICE_MISC_STRIDE,
    );
    assert_eq!(values[4 + 2 * 2], 80);
    assert_eq!(values[4 + 3 * 2], 80);
}

#[test]
fn auto_budget_preserves_manual_service_funding_when_revenue_covers_expense() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // PE pZonePops rows ×10 give a $1,290 tax base; 7% ÷75 yields $120,
    // enough to cover one police station's $100 annual cost.
    city.set_pe_zone_pops([129, 0, 0]);
    stamp_complete_building_for_test(&mut city, 20, 20, 0xae, 3);
    stamp_complete_building_for_test(&mut city, 30, 30, 0xae, 3);
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    set_budget_funding_for_test(&mut city, 0, 30);
    assert!(set_city_auto_budget_enabled(&mut city, true));
    let mut state = test_state(city);
    state.zone_growth_enabled = false;
    state.game_clock = GameClock::new(1900, 3);

    state.handle_simulation_month_advanced();

    assert_eq!(budget_service_funding_percent_for_city(&state.city, 0), 30);
    let values = state.city.misc_u32_values(
        budget_service_misc_offset(0, 0).expect("police budget offset"),
        BUDGET_SERVICE_MISC_STRIDE,
    );
    assert_eq!(
        values[4 + 2 * 2],
        30,
        "phase zero retains its original funding snapshot"
    );
    state.game_clock = GameClock::new(1900, 4);
    state.handle_simulation_month_advanced();
    let values = state.city.misc_u32_values(
        budget_service_misc_offset(0, 0).unwrap(),
        BUDGET_SERVICE_MISC_STRIDE,
    );
    assert_eq!(values[4 + 2 * 2], 30);
    assert_eq!(values[4 + 3 * 2], 30);
}

#[test]
fn population_panel_fame_uses_process_crt_counters_when_misc_missing() {
    let mut city = City::new_blank("/tmp/sc2000-pop-fame.sc2", "PopFame").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.clear_traffic_density_for_test();
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xa7a7_7a7a);
    let process = state.pe_process_crt_seed();
    let power_maps = power_service_maps_with_entry_seed(&state.city, process);
    let population = live_population_stats_for_city(&state.city);
    let outcomes = live_city_service_outcomes_for_city_with_power(&state.city, &power_maps);
    let counters = live_city_scenario_counters_for_city_with_power(
        &state.city,
        population,
        outcomes,
        &power_maps,
    );
    let expected_fame = live_city_fame_value(&state.city, population, outcomes, counters);
    let rows = population_panel_demographic_rows_with_power(
        &state.city,
        population,
        outcomes,
        &power_maps,
    );
    let fame = rows
        .iter()
        .find(|(label, _)| *label == "City Fame")
        .map(|(_, value)| *value)
        .expect("City Fame row");
    assert_eq!(fame, expected_fame);
}

#[test]
fn win95_rci_demand_publication_updates_only_shared_channels_in_mac_layout() {
    let mut city = City::new_blank("/tmp/sc2000-mac-rci-demand-cross-layout.sc2", "MacRci")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for (index, value) in [-125, 240, -375, 0x111, 0x222, 0x333, 0x444, 0x555]
        .into_iter()
        .enumerate()
    {
        city.set_misc_u32_at(RCI_DEMAND_MISC_OFFSET + index * 4, value as u32);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    let population = PopulationStats::default();
    let outcomes = CityServiceOutcomes::default();
    let expected = rci_demand_values_for_city(&state.city, population, outcomes);

    state.refresh_live_rci_demand_values(population, outcomes);

    for (index, value) in expected.into_iter().enumerate() {
        assert_eq!(
            state
                .city
                .mac_state_misc_u32_at(RCI_DEMAND_MISC_OFFSET + index * 4),
            Some(value as u32),
            "shared prior-demand channel {index}"
        );
    }
    for (index, value) in [0x111, 0x222, 0x333, 0x444, 0x555].into_iter().enumerate() {
        assert_eq!(
            state
                .city
                .mac_state_misc_u32_at(RCI_DEMAND_MISC_OFFSET + (index + 3) * 4),
            Some(value),
            "remaining Mac demand-block word {index}"
        );
    }
}

#[test]
fn win95_budget_phase_inputs_and_publication_preserve_mac_state_outside_row_book() {
    let mut city = blank_city_layout_fixture("sc2000-mac-budget-cross-layout.city", "MacBudgetX");
    let preserved = [
        (0x0e3c, 0x1234_5678), // Mac saved year-end slot immediately after the row book.
        (0x0ec8, 0x2345_6789), // Mac CODE 4 state under the PE newspaper queue.
        (0x0fa8, 0x3456_789a), // Mac zone-7 selector census.
        (0x1028, 0x4567_89ab), // Mac stadium/XMIC selector mask.
    ];
    for &(offset, value) in &preserved {
        city.set_mac_state_misc_u32_at(offset, value);
    }
    let mac_tax_current =
        budget_row_misc_offset_for_city(&city, 0, 0).expect("Mac budget row current offset");
    let mut old_tax_row = vec![0; BUDGET_SERVICE_MISC_STRIDE];
    old_tax_row[BUDGET_SERVICE_CURRENT_FIELD] = 123;
    old_tax_row[BUDGET_SERVICE_FUNDING_FIELD] = DEFAULT_TAX_RATE;
    city.set_budget_misc_u32_values(mac_tax_current, &old_tax_row);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.game_clock = GameClock::new(1900, 1);
    let population = live_population_stats_for_city(&state.city);
    let expected_tax_rows =
        budget_tax_bases_for_city_with_fidelity(&state.city, population, EngineFidelity::Win95);
    state.refresh_monthly_budget_inputs(population);

    assert_eq!(
        state.city.budget_misc_u32_at(mac_tax_current),
        Some(expected_tax_rows[0]),
        "the Win95 day-21 refresh writes through the Mac row-book view"
    );
    state.refresh_monthly_budget_rows(population);
    let save_path = std::env::temp_dir().join(format!(
        "sc2000-mac-budget-cross-layout-{}.city",
        std::process::id()
    ));
    state.city.save(&save_path).expect("save Mac-layout city");
    let reloaded = City::load(&save_path).expect("reload Mac-layout budget book");
    assert_eq!(reloaded.kind, CityFileKind::MacCity);
    for (offset, value) in preserved {
        assert_eq!(
            state.city.mac_state_misc_u32_at(offset),
            Some(value),
            "day-21 row sync must not overwrite Mac MISC {offset:#06x}"
        );
        assert_eq!(reloaded.mac_state_misc_u32_at(offset), Some(value));
    }
    assert_eq!(
        reloaded.budget_misc_u32_at(mac_tax_current),
        Some(expected_tax_rows[0])
    );
    assert_eq!(
        state.city.misc_u32_at(mac_tax_current),
        Some(expected_tax_rows[0]),
        "the row-book side view must serialize its update"
    );
    let _ = std::fs::remove_file(save_path);
}

#[test]
fn outstanding_bond_count_reads_bss_4cc4e8() {
    let mut city = City::new_blank("/tmp/sc2000-bonds-bss.sc2", "BondsBss").expect("blank");
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 1);
    city.pe_dw_city_bonds = 4;
    assert_eq!(
        outstanding_bond_count_for_city(&city),
        4,
        "budget panel / bond helpers read BSS dwCityBonds @ 0x4cc4e8"
    );
}

#[test]
fn xgrp_history_water_percent_uses_pe_coverage_bss_not_used_percent() {
    // PE `UpdateGraphData` @ `0x414065`: GRP_WATERPERCENT = 100 - [0x4cc4b8].
    let mut city = City::new_blank("/tmp/sc2000-xgrp-water.sc2", "XgrpWater").expect("blank");
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &vec![0u32; XGRP_VALUES]);
    city.set_pe_water_coverage_percent(0x2a);
    // Used-% would differ if XGRP still read dwWaterUsedPercentage / CRT seed.
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x6c6c_c6c6);
    let used =
        water_used_percent_for_city_with_entry_seed(&state.city, state.pe_process_crt_seed());
    assert_ne!(used, u32::from(state.city.pe_water_coverage_percent()));
    let expected_water = 100_u32.saturating_sub(u32::from(state.city.pe_water_coverage_percent()));
    let population = population_stats_for_city(&state.city);
    let outcomes = city_service_outcomes_for_city(&state.city);
    state.refresh_xgrp_history(population, outcomes);
    let updated = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    let water_series_base = 9 * XGRP_SAMPLES_PER_SERIES;
    assert_eq!(updated[water_series_base], expected_water);
    assert_ne!(updated[water_series_base], 100_u32.saturating_sub(used));
}

#[test]
fn rci_zones_wait_for_power_before_developing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(21, 21, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_transport_access_for_test(&mut city, 20, 22);
    water_footprint_from_below_for_test(&mut city, 21, 21, 3);
    seed_pe_rci_demand_to_survive_months(&mut city, 3);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();
    assert_eq!(population_stats_for_city(&state.city).populated_tiles, 0);

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
fn rci_zones_build_dry_internal_pipes_without_water_service() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let origin = (21, 21);
    let size = 3;
    for (x, y) in building_footprint_from_origin(origin.0, origin.1, size).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_transport_access_for_test(&mut city, 20, 22);
    power_adjacent_building_for_test(&mut city, 21, 20);
    seed_pe_rci_demand_to_survive_months(&mut city, 2);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();
    let developed: Vec<_> = building_footprint_from_origin(origin.0, origin.1, size)
        .unwrap()
        .into_iter()
        .filter(|&(x, y)| is_construction_building(state.city.building_at(x, y)))
        .collect();
    assert_eq!(developed.len(), 7);
    for &(x, y) in &developed {
        assert!(building_internal_pipe_present(&state.city, x, y));
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped());
        assert!(!bits.watered());
    }
}

#[test]
fn rci_demand_tracks_jobs_residents_vacant_zones_and_taxes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for index in 0..7 {
        stamp_complete_building_for_test(
            &mut city,
            10 + index * 3,
            10,
            COMMERCIAL_3X3_BUILDINGS[0],
            3,
        );
    }
    for x in 10..15 {
        city.set_zone(x, 20, ZONE_RESIDENTIAL_LIGHT);
    }

    let population = population_stats_for_city(&city);
    let outcomes = city_service_outcomes_for_city(&city);
    let normal_demand = rci_demand_values_for_city(&city, population, outcomes);
    assert_eq!(
        normal_demand,
        [RCI_DEMAND_MIN, -600, RCI_DEMAND_MAX],
        "PE FTOL + ADDW wraps the large residential balance before clamping"
    );

    let mut empty = City::new_blank("/tmp/sc2000-test-empty.sc2", "test").expect("blank city");
    let empty_demand = rci_demand_values_for_city(
        &empty,
        population_stats_for_city(&empty),
        city_service_outcomes_for_city(&empty),
    );
    assert_eq!(
        empty_demand,
        [-600, -600, 2_000],
        "empty-city PE first step: R/C ftol(-1*600), I 15-floor clamps at 2000"
    );
    set_misc_u32_for_test(
        &mut empty,
        tax_rate_misc_offset(0).expect("residential tax offset"),
        MAX_TAX_RATE,
    );
    let taxed_demand = rci_demand_values_for_city(
        &empty,
        population_stats_for_city(&empty),
        city_service_outcomes_for_city(&empty),
    )[0];
    assert!(taxed_demand < empty_demand[0]);
    assert!(taxed_demand < 0);
}

#[test]
fn pe_rci_demand_uses_and_refreshes_saved_previous_residential_zone_population() {
    let path = "/tmp/sc2000-rci-prev-res-pop.sc2";
    let mut source = City::new_blank(path, "RciPrevRes").expect("blank city");
    source.pe_dw_residential_population = 120;
    source.set_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET, 120);
    source.save(path).expect("save source");

    let mut loaded = City::load(path).expect("load source");
    assert_eq!(loaded.pe_dw_residential_population, 120);
    loaded.set_pe_zone_pops([200, 80, 40]);
    let loaded_scores = pe_rci_zone_balance_scores(&loaded, PopulationStats::default());

    let mut zero_previous = City::new_blank("/tmp/sc2000-rci-prev-res-zero.sc2", "RciPrevZero")
        .expect("blank comparison city");
    zero_previous.set_pe_zone_pops([200, 80, 40]);
    let zero_scores = pe_rci_zone_balance_scores(&zero_previous, PopulationStats::default());
    assert!(
        loaded_scores[1] > zero_scores[1],
        "the previous residential zone-population MISC value affects the PE ratio"
    );

    let mut state = test_state(loaded);
    state.refresh_live_misc_current_values();
    state.sim_calendar_day23_stats_dialogs();
    assert_eq!(state.city.pe_dw_residential_population, 120);
    state
        .refresh_live_rci_demand_values(PopulationStats::default(), CityServiceOutcomes::default());
    assert_eq!(state.city.pe_dw_residential_population, 200);
    state.city.save(path).expect("save updated state");
    assert_eq!(
        state.city.misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET),
        Some(200),
        "the native prior-month field is persisted at MISC +0x74"
    );
    let reopened = City::load(path).expect("reload updated state");
    assert_eq!(reopened.pe_dw_residential_population, 200);

    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_rci_demand_adds_as_signed_word_before_clamping() {
    let mut city =
        City::new_blank("/tmp/sc2000-rci-word-add.sc2", "RciWordAdd").expect("blank city");
    city.set_pe_zone_pops([1_000, 0, 1]);
    city.pe_dw_residential_population = 1_000;
    let score = pe_rci_zone_balance_scores(&city, PopulationStats::default())[1];
    let rate = pe_rci_ordinance_adjusted_tax_rate(
        &city,
        RciKind::Commercial,
        tax_rate_for_city(&city, rci_kind_index(RciKind::Commercial)) as i32,
    );
    let delta = ((score * PE_RCI_DEMAND_BALANCE_SCALE).trunc() as i32)
        .wrapping_add(pe_rci_tax_demand_delta(rate));
    assert!(delta > i32::from(i16::MAX));
    let expected = i32::from((delta as i16).clamp(RCI_DEMAND_MIN as i16, RCI_DEMAND_MAX as i16));
    let actual =
        pe_rci_demand_values_from_previous(&city, PopulationStats::default(), [0, 0, 0])[1];
    assert_eq!(expected, RCI_DEMAND_MIN);
    assert_eq!(actual, expected);
}

#[test]
fn pe_rci_demand_adds_tax_table_value_before_ftol() {
    let mut city =
        City::new_blank("/tmp/sc2000-rci-tax-before-ftol.sc2", "RciTaxFtol").expect("blank city");
    set_misc_u32_for_test(
        &mut city,
        tax_rate_misc_offset(rci_kind_index(RciKind::Commercial)).expect("commercial tax offset"),
        8,
    );

    let fractional_balance = 0.5 / f64::from(PE_RCI_DEMAND_BALANCE_SCALE);
    let actual = pe_rci_demand_monthly_delta(&city, RciKind::Commercial, fractional_balance);

    // At tax index 8, the table contributes -25. Native x87 forms
    // 0.5 + -25 = -24.5 before FTOL truncates toward zero.
    assert_eq!(pe_rci_tax_demand_delta(8), -25);
    assert_eq!(
        fractional_balance * f64::from(PE_RCI_DEMAND_BALANCE_SCALE),
        0.5
    );
    assert_eq!(actual, -24);
}

#[test]
fn pe_rci_demand_keeps_x87_precision_across_score_and_tax_ftol() {
    let mut city =
        City::new_blank("/tmp/sc2000-rci-x87-score.sc2", "RciX87Score").expect("blank city");
    city.set_pe_zone_pops([2, 1, 0]);

    let scores = pe_rci_zone_balance_scores(&city, PopulationStats::default());
    let actual = pe_rci_demand_values_from_previous(&city, PopulationStats::default(), [0, 0, 0]);

    // PE stores the x87 result of `1 / (2 + 1) - 1` as the nearest f64
    // (-0.6666666666666666), then multiplies by 600 in x87. At the default
    // tax rate 7, the table adds zero. The product is just above -400, so
    // `_ftol` truncates it to -399. Separate binary64 operations produce
    // -0.6666666666666667 and then -400.
    assert_eq!(scores[0], -0.6666666666666666);
    assert_eq!(pe_rci_tax_demand_delta(DEFAULT_TAX_RATE as i32), 0);
    assert_eq!(actual[0], -399);
    let binary64_score = 1.0_f64 / 3.0 - 1.0;
    assert_eq!((binary64_score * 600.0).trunc() as i32, -400);
}

#[test]
fn rci_demand_steps_once_on_day_21_not_day_2() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    set_rci_demand_for_test(&mut city, 0, 0, 0);
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 1, 1);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });
    assert_eq!(state.game_clock.day(), 2);
    assert_eq!(
        state.city.misc_i32_values(RCI_DEMAND_MISC_OFFSET, 3),
        vec![0, 0, 0],
        "day 2 density must not advance wCityDemand"
    );

    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(19);
    state.apply_game_clock_advance(
        start,
        GameClockAdvance {
            days: 19,
            months: 0,
        },
    );
    assert_eq!(state.game_clock.day(), 21);
    assert_eq!(
        state.city.misc_i32_values(RCI_DEMAND_MISC_OFFSET, 3),
        vec![-600, -600, 2_000],
        "day 21 SimulationRCIDemandUpdates writes one empty-city step"
    );
}

#[test]
fn refresh_live_misc_does_not_step_rci_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    set_rci_demand_for_test(&mut city, 500, -100, 200);
    let mut state = test_state(city);
    state.refresh_live_misc_current_values();
    assert_eq!(
        state.city.misc_i32_values(RCI_DEMAND_MISC_OFFSET, 3),
        vec![500, -100, 200],
        "census persist is not SimulationRCIDemandUpdates"
    );
}

#[test]
fn individual_industry_taxes_do_not_adjust_aggregate_industrial_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 20..28 {
        city.set_zone(x, 20, ZONE_INDUSTRIAL_LIGHT);
    }
    let population = PopulationStats {
        industrial_population: 20_000,
        total_population: 20_000,
        ..Default::default()
    };
    let outcomes = city_service_outcomes_for_city(&city);
    let base_demand = rci_demand_values_for_city(&city, population, outcomes)
        [rci_kind_index(RciKind::Industrial)];

    let mut high_tax_values = vec![0_u32; INDUSTRY_VALUES];
    let mut low_tax_values = vec![0_u32; INDUSTRY_VALUES];
    for index in 0..INDUSTRY_COUNT {
        let base = index * INDUSTRY_FIELD_COUNT;
        high_tax_values[base] = 1;
        high_tax_values[base + 1] = MAX_TAX_RATE;
        low_tax_values[base] = 1;
    }

    city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &high_tax_values);
    let high_tax_demand = rci_demand_values_for_city(&city, population, outcomes)
        [rci_kind_index(RciKind::Industrial)];

    city.set_misc_u32_values(INDUSTRY_MISC_OFFSET, &low_tax_values);
    let low_tax_demand = rci_demand_values_for_city(&city, population, outcomes)
        [rci_kind_index(RciKind::Industrial)];

    assert_eq!(high_tax_demand, base_demand);
    assert_eq!(low_tax_demand, base_demand);
}

#[test]
fn ordinances_adjust_rci_demand_pressure() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 10..22 {
        city.set_zone(x, 20, ZONE_COMMERCIAL_LIGHT);
        city.set_zone(x, 22, ZONE_INDUSTRIAL_LIGHT);
    }
    let population = PopulationStats {
        commercial_population: 10_000,
        industrial_population: 10_000,
        total_population: 20_000,
        ..Default::default()
    };
    let outcomes = city_service_outcomes_for_city(&city);
    let base = rci_demand_values_for_city(&city, population, outcomes);

    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_TOURIST_ADVERTISING,
        true
    ));
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_BUSINESS_ADVERTISING,
        true
    ));
    let advertised = rci_demand_values_for_city(&city, population, outcomes);

    assert!(advertised[1] > base[1]);
    assert!(advertised[2] > base[2]);

    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_POLLUTION_CONTROLS,
        true
    ));
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_NUCLEAR_FREE_ZONE,
        true
    ));
    let restricted = rci_demand_values_for_city(&city, population, outcomes);

    assert!(restricted[2] < advertised[2]);
}

#[test]
fn service_outcome_ordinance_masks_follow_city_file_layout() {
    let mut pe_city =
        City::new_blank("/tmp/sc2000-pe-ordinance-source.sc2", "PeCity").expect("blank PE city");
    pe_city.set_misc_u32_at(
        ORDINANCE_FLAGS_MISC_OFFSET,
        1 << ORDINANCE_VOLUNTEER_FIRE_DEPT,
    );
    let mut mac_city =
        City::new_blank("/tmp/sc2000-mac-ordinance-source.sc2", "MacCity").expect("blank Mac city");
    mac_city.kind = CityFileKind::MacCity;
    mac_city.set_mac_state_misc_u32_at(
        MAC_ORDINANCE_FLAGS_MISC_OFFSET,
        1 << ORDINANCE_NEIGHBORHOOD_WATCH,
    );

    let population = PopulationStats::default();
    let pe = ordinance_effects_for_city_with_fidelity(&pe_city, population, EngineFidelity::Win95);
    let mac =
        ordinance_effects_for_city_with_fidelity(&mac_city, population, EngineFidelity::Mac68k);
    assert_eq!(
        ordinance_flags_for_city_with_fidelity(&pe_city, EngineFidelity::Mac68k),
        1 << ORDINANCE_VOLUNTEER_FIRE_DEPT
    );
    assert_eq!(
        ordinance_flags_for_city_with_fidelity(&mac_city, EngineFidelity::Win95),
        1 << ORDINANCE_NEIGHBORHOOD_WATCH
    );

    assert_eq!(pe.fire_bonus, 15);
    assert_eq!(pe.police_bonus, 0);
    assert_eq!(mac.fire_bonus, 0);
    assert_eq!(mac.police_bonus, 12);
}

#[test]
fn abandoned_rci_zones_count_as_vacant_capacity_for_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_COMMERCIAL_DENSE);
    city.set_building(20, 20, 0xAA);

    let vacant_zones = vacant_rci_zone_capacity_for_city(&city);

    assert_eq!(vacant_zones[rci_kind_index(RciKind::Commercial)], 2);
    assert_eq!(vacant_zones[rci_kind_index(RciKind::Residential)], 0);
    assert_eq!(vacant_zones[rci_kind_index(RciKind::Industrial)], 0);
}

#[test]
fn rubble_and_power_line_rci_zones_count_as_vacant_capacity() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, 1);
    city.set_zone(21, 20, ZONE_INDUSTRIAL_LIGHT);
    city.set_building(21, 20, 14);

    let vacant_zones = vacant_rci_zone_capacity_for_city(&city);

    assert_eq!(vacant_zones[rci_kind_index(RciKind::Residential)], 1);
    assert_eq!(vacant_zones[rci_kind_index(RciKind::Industrial)], 1);
    assert_eq!(vacant_zones[rci_kind_index(RciKind::Commercial)], 0);
}

#[test]
fn residential_recreation_cap_blocks_positive_demand_above_threshold() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let outcomes = CityServiceOutcomes::default();
    // Base recreation numerator is 1500. R zone-pop 2000 (> 1500) with enough C
    // for the jobs term to hit that cap makes (cap / (R+1) - 1) negative.
    let over_cap_population = PopulationStats {
        residential_population: 20_000,
        commercial_population: 20_000,
        normal_population: 20_000,
        total_population: 40_000,
        ..Default::default()
    };
    let under_cap_population = PopulationStats {
        residential_population: 5_000,
        commercial_population: 20_000,
        normal_population: 5_000,
        total_population: 25_000,
        ..Default::default()
    };

    let over_cap_demand = rci_demand_values_for_city(&city, over_cap_population, outcomes)
        [rci_kind_index(RciKind::Residential)];
    let under_cap_demand = rci_demand_values_for_city(&city, under_cap_population, outcomes)
        [rci_kind_index(RciKind::Residential)];

    assert!(under_cap_demand > 0);
    assert!(over_cap_demand < 0);
}

#[test]
fn residential_recreation_buildings_restore_over_cap_demand() {
    let uncapped_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut marina_city = uncapped_city.clone();
    stamp_complete_building_for_test(&mut marina_city, 50, 50, MARINA_BUILDING_ID, 3);
    let outcomes = CityServiceOutcomes::default();
    let population = PopulationStats {
        residential_population: 16_000,
        commercial_population: 20_000,
        normal_population: 16_000,
        total_population: 36_000,
        ..Default::default()
    };

    let uncapped_demand = rci_demand_values_for_city(&uncapped_city, population, outcomes)
        [rci_kind_index(RciKind::Residential)];
    let marina_demand = rci_demand_values_for_city(&marina_city, population, outcomes)
        [rci_kind_index(RciKind::Residential)];

    assert!(uncapped_demand < 0);
    assert!(marina_demand > uncapped_demand);
}

#[test]
fn unserved_negative_demand_lowest_quality_rci_buildings_become_abandoned() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_traffic_density(20, 20, 160);
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);

    assert_eq!(state.decline_unserved_rci_zones(), 1);

    assert!([0x8A, 0x8B].contains(&state.city.building_at(20, 20)));
    assert_eq!(
        state.city.traffic_density_at(20, 20),
        Some(160),
        "ChooseAndPlace abandon stamp must leave XTRF alone"
    );
    assert_eq!(
        state.city.zone_at(20, 20).map(|zone| zone.zone_id()),
        Some(ZONE_RESIDENTIAL_LIGHT)
    );
    assert_eq!(
        state.city.tile_bits_at(20, 20).unwrap().raw_bits() & XBIT_CHOOSE_AND_PLACE,
        XBIT_CHOOSE_AND_PLACE
    );
}

#[test]
fn unserved_negative_demand_higher_quality_rci_buildings_become_abandoned() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let building = RESIDENTIAL_1X1_BUILDINGS[10];
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, building);
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);

    assert_eq!(state.decline_unserved_rci_zones(), 1);

    let expected = rci_decline_building(1, 20, 20).unwrap();
    assert_eq!(state.city.building_at(20, 20), expected);
    assert!(is_abandoned_building(expected));
    assert_eq!(
        state.city.zone_at(20, 20).map(|zone| zone.zone_id()),
        Some(ZONE_RESIDENTIAL_LIGHT)
    );
    assert_eq!(
        state.city.tile_bits_at(20, 20).unwrap().raw_bits() & XBIT_CHOOSE_AND_PLACE,
        XBIT_CHOOSE_AND_PLACE
    );
}

#[test]
fn unserved_negative_demand_large_rci_buildings_abandon_as_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let building = *COMMERCIAL_2X2_BUILDINGS.last().unwrap();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE);
        city.set_building(x, y, building);
    }
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    set_rci_demand_for_test(&mut city, 50, -50, 50);
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2);
    let replace = rci_change_building_replace_tile(sx, sy);
    let mut state = test_state(city);

    assert_eq!(state.decline_unserved_rci_zones(), 4);

    let pop = rci_building_pop_level(building).unwrap();
    let footprint = building_footprint_from_origin(20, 20, pop_level_footprint_size(pop)).unwrap();
    let placements = rci_decline_change_building(building, &footprint, 20, 20, replace).unwrap();
    let corner_bits = state.city.footprint_corner_bits();
    let mut stamped = vec![false; footprint.len()];
    for &(ox, oy, expected) in &placements {
        let size = usize::from(xbld_tile_size(expected).max(1));
        for (x, y) in building_footprint_from_origin(ox, oy, size).unwrap() {
            if let Some(index) = footprint.iter().position(|&tile| tile == (x, y)) {
                stamped[index] = true;
            }
            assert_eq!(state.city.building_at(x, y), expected);
            assert!(is_abandoned_building(expected));
            assert_eq!(
                state.city.zone_at(x, y).map(|zone| zone.raw_bits()),
                Some(
                    ZONE_COMMERCIAL_DENSE
                        | building_zone_bits_for_tile(ox, oy, x, y, size, corner_bits)
                )
            );
            assert_eq!(
                state.city.tile_bits_at(x, y).unwrap().raw_bits() & XBIT_CHOOSE_AND_PLACE,
                XBIT_CHOOSE_AND_PLACE
            );
        }
    }
    assert!(stamped.iter().all(|&hit| hit));
}

#[test]
fn decline_abandons_dense_rci_buildings_on_light_zones() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let building = RESIDENTIAL_2X2_BUILDINGS[0];
    let corner_bits = city.footprint_corner_bits();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(
            x,
            y,
            ZONE_RESIDENTIAL_LIGHT | building_zone_bits_for_tile(20, 20, x, y, 2, corner_bits),
        );
        city.set_building(x, y, building);
    }
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2);
    let replace = rci_change_building_replace_tile(sx, sy);
    let mut state = test_state(city);

    assert_eq!(state.decline_unserved_rci_zones(), 4);
    let footprint = building_footprint_from_origin(20, 20, 2).unwrap();
    let placements = rci_decline_change_building(building, &footprint, 20, 20, replace).unwrap();
    for &(ox, oy, expected) in &placements {
        let size = usize::from(xbld_tile_size(expected).max(1));
        for (x, y) in building_footprint_from_origin(ox, oy, size).unwrap() {
            assert_eq!(state.city.building_at(x, y), expected);
            assert!(is_abandoned_building(expected));
            assert_eq!(
                state.city.zone_at(x, y).map(|zone| zone.zone_id()),
                Some(ZONE_RESIDENTIAL_LIGHT)
            );
        }
    }
}

#[test]
fn vacant_rci_growth_starts_as_construction_before_completion() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_zones(), 1);
    assert!(
        is_construction_building(state.city.building_at(20, 20)),
        "empty-zone StartBuilding should stamp construction first"
    );
    assert_eq!(state.complete_rci_construction_sites(), 1);
    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "BUILD_THINK completion should ChooseAndPlace a finished residential"
    );
}

#[test]
fn develop_rci_start_building_preserves_watered_and_xtrf() {
    // PE StartBuilding → ChooseAndPlace: OR XBIT 0xe0 only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-start-xtrf.sc2", "StartX").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    city.set_tile_bits(
        20,
        20,
        city.tile_bits_at(20, 20)
            .unwrap_or_default()
            .with_piped(true)
            .with_watered(true),
    );
    city.set_traffic_density(20, 20, 0x52);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x52));
}

#[test]
fn develop_rci_start_building_preserves_xtxt_like_pe_place_tile() {
    // PE StartBuilding → ChooseAndPlace → PlaceTile `0x441f00`: leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-start-xtxt.sc2", "StartT").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_text_overlay(20, 20, 0x2a);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
}

#[test]
fn mac68k_develop_rci_start_building_preserves_xtxt_like_pe_place_tile() {
    // Shared StartBuilding → PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-start-xtxt.sc2", "MacStartT").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_text_overlay(20, 20, 0x2a);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
}

#[test]
fn mac68k_develop_rci_start_building_preserves_xtrf_like_pe_choose_and_place() {
    // Mac68k shares StartBuilding → ChooseAndPlace: OR XBIT 0xe0 only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-mac-start-xtrf.sc2", "MacStartX").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    city.set_tile_bits(
        20,
        20,
        city.tile_bits_at(20, 20)
            .unwrap_or_default()
            .with_piped(true)
            .with_watered(true),
    );
    city.set_traffic_density(20, 20, 0x52);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x52));
}

#[test]
fn rci_construction_complete_preserves_watered_and_xtrf() {
    // PE ChooseAndPlace completion: OR XBIT 0xe0 only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-build-xtrf.sc2", "BuildX").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
    state.city.set_tile_bits(
        20,
        20,
        state
            .city
            .tile_bits_at(20, 20)
            .unwrap_or_default()
            .with_piped(true)
            .with_watered(true),
    );
    state.city.set_traffic_density(20, 20, 0x61);
    assert_eq!(state.complete_rci_construction_sites(), 1);
    assert!(RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x61));
}

#[test]
fn rci_construction_complete_preserves_xtxt_like_pe_place_tile() {
    // PE ChooseAndPlace completion → PlaceTile `0x441f00`: stamp XBLD only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-build-xtxt.sc2", "BuildT").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
    state.city.set_text_overlay(20, 20, 0x2a);
    assert_eq!(state.complete_rci_construction_sites(), 1);
    assert!(RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
}

#[test]
fn mac68k_rci_construction_complete_preserves_xtxt_like_pe_place_tile() {
    // Shared ChooseAndPlace completion leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-build-xtxt.sc2", "MacBuildT").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
    state.city.set_text_overlay(20, 20, 0x2a);
    assert_eq!(state.complete_rci_construction_sites(), 1);
    assert!(RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2a));
}

#[test]
fn mac68k_rci_construction_complete_preserves_xtrf_like_pe_choose_and_place() {
    // Mac68k shares ChooseAndPlace completion: OR XBIT 0xe0 only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-mac-build-xtrf.sc2", "MacBuildX").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
    state.city.set_tile_bits(
        20,
        20,
        state
            .city
            .tile_bits_at(20, 20)
            .unwrap_or_default()
            .with_piped(true)
            .with_watered(true),
    );
    state.city.set_traffic_density(20, 20, 0x61);
    assert_eq!(state.complete_rci_construction_sites(), 1);
    assert!(RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x61));
}

#[test]
fn vacant_rci_startbuilding_overwrites_tiles_below_road() {
    for existing in [1_u8, 5, 6, SMALL_PARK_BUILDING_ID, 14] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
        city.set_building(20, 20, existing);
        stamp_transport_access_for_test(&mut city, 19, 20);
        power_adjacent_building_for_test(&mut city, 20, 19);
        water_footprint_from_below_for_test(&mut city, 20, 20, 1);
        set_rci_demand_for_test(&mut city, 1_000, 0, 0);
        let mut state = test_state(city);

        assert_eq!(
            state.develop_rci_zones(),
            1,
            "PE pop-NONE StartBuilding should overwrite {existing:#04x}"
        );
        assert!(
            is_construction_building(state.city.building_at(20, 20)),
            "rubble/power-line vacant growth must stamp construction, not ChooseAndPlace, existing {existing:#04x}"
        );
        assert!(!is_abandoned_building(state.city.building_at(20, 20)));
    }
}

#[test]
fn vacant_rci_startbuilding_skips_road_and_above() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, TILE_ROAD_LR);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_zones(), 0);
    assert_eq!(state.city.building_at(20, 20), TILE_ROAD_LR);
}

#[test]
fn growth_tick_refreshes_development_services_after_placement_power_maps() {
    // After growth-tick stamps, power maps refresh and police/fire radii must follow.
    let mut city = City::new_blank("/tmp/sc2000-svc-pwr.sc2", "SvcPwr").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 210, 3); // police 3×3
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let cold = rci_development_services_for_city_with_power(&city, &power_maps);
    assert_eq!(cold.police.len(), 1);
    let cold_radius = cold.police[0].radius;

    power_maps.powered_tiles[20 * CITY_WIDTH + 20] = true;
    let hot = rci_development_services_for_city_with_power(&city, &power_maps);
    assert_eq!(hot.police.len(), 1);
    assert!(
        hot.police[0].radius > cold_radius,
        "powered police coverage radius must increase after power-map refresh"
    );
}

#[test]

fn abandoned_rci_redevelop_places_finished_building_without_construction() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE);
        city.set_building(x, y, 0xAA);
        // PE ChooseAndPlace stamps XBIT 0xe0 on an occupied RCI tile. The
        // Win95 powered-adjacency gate reads the day-1-published 0x40 bit.
        city.set_tile_bits(x, y, TileBits::new(XBIT_CHOOSE_AND_PLACE));
    }
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    state.sim_calendar_day1_power();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        state.trip_served_tiles[y * CITY_WIDTH + x] = true;
    }

    assert_eq!(state.develop_rci_zones(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        COMMERCIAL_2X2_BUILDINGS.contains(&building),
        "BUILD_ABANDON redevelop should ChooseAndPlace finished commercial, got {building:#04x}"
    );
    assert!(!is_construction_building(building));
}

#[test]
fn abandoned_rci_redevelop_preserves_watered_and_xtrf_like_pe_choose_and_place() {
    // PE BUILD_ABANDON → ChooseAndPlace (`0x436e80`): OR XBIT 0xe0 only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-abandon-xtrf.sc2", "AbnX").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE);
        city.set_building(x, y, 0xAA);
        city.set_tile_bits(x, y, TileBits::new(XBIT_CHOOSE_AND_PLACE));
    }
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
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
        state.city.set_traffic_density(x, y, 0x44);
    }
    state.sim_calendar_day1_power();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        state.trip_served_tiles[y * CITY_WIDTH + x] = true;
    }

    assert_eq!(state.develop_rci_zones(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        COMMERCIAL_2X2_BUILDINGS.contains(&building),
        "BUILD_ABANDON redevelop should ChooseAndPlace finished commercial, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped(), "piped cleared at ({x},{y})");
        assert!(bits.watered(), "watered cleared at ({x},{y})");
        assert_eq!(
            state.city.traffic_density_at(x, y),
            Some(0x44),
            "XTRF mutated at ({x},{y})"
        );
    }
}

#[test]
fn abandoned_rci_redevelop_preserves_xtxt_like_pe_place_tile() {
    // PE BUILD_ABANDON → ChooseAndPlace → PlaceTile `0x441f00`: leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-abandon-xtxt.sc2", "AbnT").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE);
        city.set_building(x, y, 0xAA);
        city.set_tile_bits(x, y, TileBits::new(XBIT_CHOOSE_AND_PLACE));
        city.set_text_overlay(x, y, 0x2a);
    }
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    state.sim_calendar_day1_power();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        state.trip_served_tiles[y * CITY_WIDTH + x] = true;
    }

    assert_eq!(state.develop_rci_zones(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        COMMERCIAL_2X2_BUILDINGS.contains(&building),
        "BUILD_ABANDON redevelop should ChooseAndPlace finished commercial, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_abandoned_rci_redevelop_preserves_xtxt_like_pe_place_tile() {
    // Shared BUILD_ABANDON → PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-abandon-xtxt.sc2", "MacAbnT").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE);
        city.set_building(x, y, 0xAA);
        city.set_tile_bits(x, y, TileBits::new(XBIT_CHOOSE_AND_PLACE));
        city.set_text_overlay(x, y, 0x2a);
    }
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_zones(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        COMMERCIAL_2X2_BUILDINGS.contains(&building),
        "BUILD_ABANDON redevelop should ChooseAndPlace finished commercial, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_abandoned_rci_redevelop_preserves_xtrf_like_pe_choose_and_place() {
    // Shared BUILD_ABANDON → ChooseAndPlace leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-abandon-xtrf.sc2", "MacAbnX").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE);
        city.set_building(x, y, 0xAA);
    }
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_simulation_service_bits();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        state
            .city
            .set_tile_bits(x, y, TileBits::new(XBIT_CHOOSE_AND_PLACE));
        state.city.set_traffic_density(x, y, 0x44);
    }

    assert_eq!(state.develop_rci_zones(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        COMMERCIAL_2X2_BUILDINGS.contains(&building),
        "BUILD_ABANDON redevelop should ChooseAndPlace finished commercial, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(
            state.city.traffic_density_at(x, y),
            Some(0x44),
            "XTRF mutated at ({x},{y})"
        );
    }
}

#[test]
fn occupied_rci_rolls_changebuilding_without_negative_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    set_rci_demand_for_test(&mut city, 50, 50, 50);
    let mut state = test_state(city);

    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
}

#[test]
fn served_negative_demand_rci_buildings_still_roll_changebuilding() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    state.sim_calendar_day1_power();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        state.trip_served_tiles[y * CITY_WIDTH + x] = true;
    }

    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
}

#[test]
fn served_negative_demand_rci_buildings_decline_under_high_local_pollution() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    let layer_index = |x: usize, y: usize| x * CITY_HEIGHT + y;
    let mut pollution = vec![0u8; CITY_TILES];
    pollution[layer_index(20, 20)] = 255;
    city.sync_pollution_density_from_slice(&pollution);
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
}

#[test]
fn rci_changebuilding_rolls_every_occupied_lattice_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 20..30 {
        city.set_zone(x, 20, ZONE_RESIDENTIAL_LIGHT);
        city.set_building(x, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);

    assert_eq!(state.decline_unserved_rci_zones(), 10);
    let abandoned = (20..30)
        .filter(|&x| is_abandoned_building(state.city.building_at(x, 20)))
        .count();
    assert_eq!(abandoned, 10);
}

#[test]
fn rci_changebuilding_does_not_skip_later_scan_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 20..24 {
        city.set_zone(x, 20, ZONE_RESIDENTIAL_LIGHT);
        city.set_building(x, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
        power_adjacent_building_for_test(&mut city, x, 19);
    }
    city.set_zone(30, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(30, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);

    assert_eq!(state.decline_unserved_rci_zones(), 5);
    assert!(is_abandoned_building(state.city.building_at(30, 20)));
    assert!(is_abandoned_building(state.city.building_at(23, 20)));
}

#[test]
fn failed_growth_change_can_reach_same_tick_startbuilding() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 20..22 {
        for x in 20..22 {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE | ZONE_CORNER_ALL);
        }
    }
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    let _fail_growth_change = FailGrowthChangeRolls::arm();
    state.sim_calendar_one_growth_tick(0, 0);

    assert!(
        _fail_growth_change.was_used(),
        "the occupied tile must execute the forced GROWTH_CHANGE failure"
    );
    assert!(
        is_construction_building(state.city.building_at(20, 20)),
        "a failed GROWTH_CHANGE roll returns to normal StartBuilding size-up gates"
    );
    assert_eq!(
        state.pe_growth_aggregate.slots()[2],
        pe_growth_aggregate_weight(pe_growth_level_for_xbld(RESIDENTIAL_1X1_BUILDINGS[0])),
        "failed GROWTH_CHANGE contributes its pre-roll XBLD exactly once"
    );
}

#[test]
fn dense_rci_startbuilding_origin_gate_separates_construction_from_abandon() {
    let mut construction = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    construction.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE);
    construction.set_building(20, 20, CONSTRUCTION_POP1_BUILDINGS[0]);
    assert!(rci_redevelopment_origin_can_start_at(
        &construction,
        20,
        20,
        ZONE_RESIDENTIAL_DENSE,
        RciKind::Residential,
    ));

    let mut abandoned = construction;
    abandoned.set_building(20, 20, ABANDONED_POP1_BUILDINGS[0]);
    assert!(!rci_redevelopment_origin_can_start_at(
        &abandoned,
        20,
        20,
        ZONE_RESIDENTIAL_DENSE,
        RciKind::Residential,
    ));
    assert_eq!(
        rci_building_pop_level(abandoned.building_at(20, 20)),
        Some(1)
    );
    // `abandoned_tiles_do_not_startbuilding_same_visit` covers the separate
    // BUILD_ABANDON same-population ChooseAndPlace path.
}

#[test]
fn dense_rci_redevelopment_origin_can_start_at_from_view_corner_only_for_live_pop2() {
    for rotation in 0..4u8 {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_RESIDENTIAL_DENSE);
        stamp_complete_building_for_test(&mut city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[0], 2);
        or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
        city.set_view_rotation(rotation);
        let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2);
        for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
            if (x, y) == (sx, sy) {
                continue;
            }
            assert!(
                !rci_redevelopment_origin_can_start_at(
                    &city,
                    x,
                    y,
                    ZONE_RESIDENTIAL_DENSE,
                    RciKind::Residential
                ),
                "rotation {rotation}: live 2×2 growth should not start from non-angle tile ({x},{y})"
            );
        }
        assert!(
            rci_redevelopment_origin_can_start_at(
                &city,
                sx,
                sy,
                ZONE_RESIDENTIAL_DENSE,
                RciKind::Residential
            ),
            "rotation {rotation}: live 2×2 growth should start from the view-angle corner"
        );
    }
}

#[test]
fn dense_rci_pop2_startbuilding_uses_direct_pe_gates_all_rotations() {
    let expected_call_tiles = [(20, 21), (20, 20), (21, 20), (21, 21)];
    let expected_origins = [(20, 20), (20, 19), (21, 19), (21, 20)];

    for rotation in 0..4u8 {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        for y in 18..=23 {
            for x in 19..=24 {
                city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
            }
        }
        stamp_complete_building_for_test(&mut city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[0], 2);
        or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
        set_rci_demand_for_test(&mut city, 1_000, 0, 0);
        stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);
        city.set_view_rotation(rotation);
        let (expected_x, expected_y) = expected_origins[usize::from(rotation)];
        // The direct pop-2 branch does not run the later candidate flat-land
        // or adjacent-transport heuristics before ChooseAndPlace.
        city.set_terrain(expected_x, expected_y, 0x01);

        let mut state = test_state(city);
        state.bypass_growth_trip_gate = true;
        state.refresh_simulation_service_bits();
        let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&state.city, (20, 20), 2);
        assert_eq!((sx, sy), expected_call_tiles[usize::from(rotation)]);

        let unpowered_maps = PowerServiceMaps {
            conductive_tiles: vec![false; CITY_TILES],
            powerable_tiles: vec![false; CITY_TILES],
            powered_tiles: vec![false; CITY_TILES],
            energized_tiles: vec![false; CITY_TILES],
            ..Default::default()
        };
        let reserved = vec![false; CITY_TILES];
        assert!(
            state
                .rci_expansion_redevelopment_candidate_at(
                    sx,
                    sy,
                    &reserved,
                    &unpowered_maps,
                    city_service_outcomes_for_city(&state.city),
                    &rci_development_services_for_city(&state.city),
                    None,
                )
                .is_none(),
            "rotation {rotation}: current-pop-2 growth still requires PE powered-cross"
        );

        let power_maps = PowerServiceMaps {
            conductive_tiles: vec![true; CITY_TILES],
            powerable_tiles: vec![true; CITY_TILES],
            powered_tiles: vec![true; CITY_TILES],
            saved_xbit_powered_tiles: vec![true; CITY_TILES],
            energized_tiles: vec![true; CITY_TILES],
            ..Default::default()
        };
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
            .expect("PE current-pop-2 should place directly from the view-angle tile");

        assert_eq!(
            candidate.plan.origin,
            expected_origins[usize::from(rotation)],
            "rotation {rotation} should apply ChooseAndPlace(pop=3)'s fixed Y shift"
        );
        assert_eq!(
            candidate.plan.footprint,
            building_footprint_from_origin(
                expected_origins[usize::from(rotation)].0,
                expected_origins[usize::from(rotation)].1,
                2,
            )
            .unwrap()
        );
        assert!(CONSTRUCTION_POP3_BUILDINGS.contains(&candidate.plan.building));
        assert!(candidate.plan.place_as_construction);

        let selected_building = candidate.plan.building;
        let mut stamp_crt = Win95CrtRand::new(0x0043_75C0);
        let seed_before_stamp = stamp_crt.seed();
        state.stamp_zone_development_with_crt(&candidate.plan, &mut stamp_crt);
        assert_eq!(stamp_crt.seed(), seed_before_stamp, "do not pick twice");
        for (x, y) in candidate.plan.footprint {
            assert_eq!(state.city.building_at(x, y), selected_building);
        }
    }
}

#[test]
fn dense_rci_redevelopment_origin_can_start_at_from_view_corner_only_for_live_pop3() {
    for rotation in 0..4u8 {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        stamp_zoned_building_footprint_for_test(
            &mut city,
            (20, 20),
            3,
            ZONE_RESIDENTIAL_DENSE,
            RESIDENTIAL_3X3_BUILDINGS[0],
        );
        city.set_view_rotation(rotation);
        let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 3);
        for (x, y) in building_footprint_from_origin(20, 20, 3)
            .expect("3×3 footprint for live pop3 growth test")
            .into_iter()
        {
            if (x, y) == (sx, sy) {
                continue;
            }
            assert!(
                !rci_redevelopment_origin_can_start_at(
                    &city,
                    x,
                    y,
                    ZONE_RESIDENTIAL_DENSE,
                    RciKind::Residential
                ),
                "rotation {rotation}: live 3×3 growth should not start from non-view-angle corner ({x},{y})"
            );
        }
        assert!(
            rci_redevelopment_origin_can_start_at(
                &city,
                sx,
                sy,
                ZONE_RESIDENTIAL_DENSE,
                RciKind::Residential
            ),
            "rotation {rotation}: live 3×3 growth should start from the view-angle corner"
        );
    }
}

#[test]
fn rci_changebuilding_does_not_prefer_pollution_over_scan_order() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 20..24 {
        city.set_zone(x, 20, ZONE_RESIDENTIAL_LIGHT);
        city.set_building(x, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
        power_adjacent_building_for_test(&mut city, x, 19);
    }
    city.set_zone(30, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(30, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    power_adjacent_building_for_test(&mut city, 30, 19);
    let layer_index = |x: usize, y: usize| x * CITY_HEIGHT + y;
    let mut pollution = vec![0u8; CITY_TILES];
    pollution[layer_index(30, 20)] = 255;
    city.sync_pollution_density_from_slice(&pollution);
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);

    assert_eq!(state.decline_unserved_rci_zones(), 5);
    assert!(is_abandoned_building(state.city.building_at(30, 20)));
    assert!(is_abandoned_building(state.city.building_at(23, 20)));
}

#[test]
fn rci_startbuilding_rolls_every_vacant_lattice_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (y, zone_id) in [
        (20, ZONE_RESIDENTIAL_LIGHT),
        (24, ZONE_COMMERCIAL_LIGHT),
        (28, ZONE_INDUSTRIAL_LIGHT),
    ] {
        for x in 20..25 {
            city.set_zone(x, y, zone_id);
            stamp_transport_access_for_test(&mut city, x, y + 1);
        }
        water_footprint_from_below_for_test(&mut city, 20, y, 1);
        powered_line_corridor_for_test(&mut city, 20, 24, y - 1);
    }
    set_rci_demand_for_test(&mut city, 1_000, 1_000, 1_000);
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();
    state.develop_rci_until_built_for_test();

    for (y, buildings) in [
        (20, RESIDENTIAL_1X1_BUILDINGS.as_slice()),
        (24, COMMERCIAL_1X1_BUILDINGS.as_slice()),
        (28, INDUSTRIAL_1X1_BUILDINGS.as_slice()),
    ] {
        let developed = (20..25)
            .filter(|&x| buildings.contains(&state.city.building_at(x, y)))
            .count();
        assert_eq!(
            developed, 5,
            "unexpected developed footprint count for row {y}"
        );
        let undeveloped = (20..25)
            .filter(|&x| state.city.building_at(x, y) == 0)
            .count();
        assert_eq!(undeveloped, 0);
    }
}

#[test]
fn low_positive_rci_demand_still_develops_every_vacant_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 20..25 {
        city.set_zone(x, 20, ZONE_RESIDENTIAL_LIGHT);
        stamp_transport_access_for_test(&mut city, x, 21);
        water_footprint_from_below_for_test(&mut city, x, 20, 1);
    }
    powered_line_corridor_for_test(&mut city, 20, 24, 19);
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_until_built_for_test(), 5);

    let developed = (20..25)
        .filter(|&x| RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(x, 20)))
        .count();
    assert_eq!(developed, 5);
}

#[test]
fn low_positive_dense_rci_demand_develops_one_by_one_only() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);

    let expected = vacant_rci_transit_ring_count_for_test(&state.city, (20, 20), 3);
    assert_eq!(state.develop_rci_until_built_for_test(), expected);

    let footprint = building_footprint_from_origin(20, 20, 3).unwrap();
    let developed = footprint
        .iter()
        .filter(|&&(x, y)| RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(x, y)))
        .count();
    assert_eq!(developed, expected);
    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "origin still grows; vacant StartBuilding stays 1×1"
    );
}

#[test]
fn medium_positive_dense_rci_demand_develops_two_by_two_first() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE);
    }
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 0, RCI_DENSE_2X2_DEMAND_THRESHOLD, 0);
    let mut state = test_state(city);

    let expected = vacant_rci_transit_ring_count_for_test(&state.city, (20, 20), 3);
    // Vacant growth is always 1×1; demand no longer jumps to 2×2.
    assert_eq!(state.develop_rci_until_built_for_test(), expected);

    let building = state.city.building_at(20, 20);
    assert!(
        COMMERCIAL_1X1_BUILDINGS.contains(&building),
        "expected vacant commercial growth to start as 1x1, got {building:#04x}"
    );
    assert_eq!(state.city.building_at(22, 22), 0);
}

#[test]
fn rci_growth_rejects_sloped_zone_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_terrain(20, 20, 0x01);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_until_built_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn rci_startbuilding_uses_demand_plus_2000_threshold() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, zone_id) in [
        (20, ZONE_RESIDENTIAL_LIGHT),
        (21, ZONE_COMMERCIAL_LIGHT),
        (22, ZONE_INDUSTRIAL_LIGHT),
    ] {
        city.set_zone(x, 20, zone_id);
        stamp_transport_access_for_test(&mut city, x, 19);
    }
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    // Vacant StartBuilding: 3 * (demand+2000) / 1. Demand 0 → 6000; floor -2000 → 0.
    set_rci_demand_for_test(&mut city, 0, 50, -2_000);
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();
    state.develop_rci_until_built_for_test();

    assert!(RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)));
    assert!(COMMERCIAL_1X1_BUILDINGS.contains(&state.city.building_at(21, 20)));
    assert_eq!(state.city.building_at(22, 20), 0);
}

#[test]
fn simulation_month_applies_pe_incremental_demand_to_saved_value() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 20..28 {
        city.set_zone(x, 20, ZONE_RESIDENTIAL_LIGHT);
    }
    city.set_zone(10, 10, ZONE_INDUSTRIAL_LIGHT);
    city.set_building(10, 10, INDUSTRIAL_1X1_BUILDINGS[0]);
    city.set_zone(11, 10, ZONE_INDUSTRIAL_LIGHT);
    city.set_building(11, 10, INDUSTRIAL_1X1_BUILDINGS[0]);
    set_rci_demand_for_test(&mut city, -1_000, 0, 0);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    let demand =
        saved_rci_demand_for_kind(&state.city, RciKind::Residential).expect("demand written");
    assert_eq!(
        demand, -1_600,
        "day-21 PE balance adds its rooted -600 delta to saved -1000 demand"
    );
}

#[test]
fn simulation_month_grows_vacant_zone_when_demand_starts_non_negative() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    city.set_zone(10, 10, ZONE_INDUSTRIAL_LIGHT);
    city.set_building(10, 10, INDUSTRIAL_1X1_BUILDINGS[0]);
    city.set_zone(11, 10, ZONE_INDUSTRIAL_LIGHT);
    city.set_building(11, 10, INDUSTRIAL_1X1_BUILDINGS[0]);
    set_rci_demand_for_test(&mut city, 0, 0, 0);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    assert!(
        is_construction_building(state.city.building_at(20, 20)),
        "vacant R with non-negative demand should StartBuilding after the monthly PE demand step, got {:#04x}",
        state.city.building_at(20, 20)
    );
}

#[test]
fn pe_rci_tax_demand_table_matches_win95_4e9328() {
    assert_eq!(pe_rci_tax_demand_delta(7), 0);
    assert_eq!(pe_rci_tax_demand_delta(0), 200);
    assert_eq!(pe_rci_tax_demand_delta(8), -25);
    assert_eq!(pe_rci_tax_demand_delta(20), -600);
}

#[test]
fn mac_phase21_native_demand_finalizer_matches_flint_68020_trace() {
    let cases = [
        (0x43b0_8000, 302, 0x3e28_fa16, -25, -1424, -1349),
        (0x438e_cb4f, 172, 0x3f26_9acf, 25, -2000, -1584),
        (0x445a_0556, 175, 0x407d_1f09, -25, -2000, 348),
    ];
    for (candidate, pair_total, normalized, adjustment, previous, expected) in cases {
        let score = super::super::rci::mac_rci_normalize_demand_candidate(
            f32::from_bits(candidate),
            pair_total,
        );
        assert_eq!(score.to_bits(), normalized, "candidate {candidate:#010x}");
        let actual = super::super::rci::mac_rci_demand_from_candidate(
            f32::from_bits(candidate),
            pair_total,
            adjustment,
            previous,
        );
        assert_eq!(actual, expected, "candidate {candidate:#010x}");
    }
}

#[test]
fn mac_phase21_publishes_flint_demand_industry_and_cohorts_on_both_layouts() {
    const COHORT_TABLE: usize = 0x7c;
    let input = include_str!(
        "../../../tests/fixtures/simulation/mac/mac_phase21_scheduler_cohort_transition.csv"
    )
    .lines()
    .nth(1)
    .unwrap()
    .split(',')
    .collect::<Vec<_>>();
    let lanes = [10, 11, 12].map(|column| {
        input[column]
            .split(':')
            .map(|word| u32::from_str_radix(word, 16).unwrap())
            .collect::<Vec<_>>()
    });
    let packed: [u32; 60] = std::array::from_fn(|i| lanes[i % 3][i / 3]);
    // Original CODE 8 shrinking case and a second replay with a larger
    // industrial scan total that distributes growth and changes both modifiers.
    let cases = [
        (
            996,
            [-1436, -1975, -1977, 0x111, 0x222, 0x333, 0x444, 0x555],
            [246, 23, 7, 1, 192, 486, 11, 33, 7, 29, 25],
            [1, 5],
            [60, 59, 86],
            0x80b0_5d7e_b35a_f67d,
            2_109_433_906,
        ),
        (
            2936,
            [-1046, -1624, -2000, 0x111, 0x222, 0x333, 0x444, 0x555],
            [282, 23, 40, 1, 341, 621, 191, 214, 73, 471, 156],
            [0, 0],
            [63, 59, 85],
            0xa177_4b11_cff2_a01b,
            64_747_698,
        ),
    ];
    for (
        name,
        (
            dense_industry,
            expected,
            population_rows,
            [pollution, mix],
            cohort_summary,
            cohort_hash,
            outgoing_seed,
        ),
    ) in ["mac-rci-phase21.city", "mac-rci-phase21.sc2"]
        .into_iter()
        .flat_map(|name| cases.into_iter().map(move |case| (name, case)))
    {
        let mut city = blank_city_layout_fixture(name, "Flint");
        let kind = city.kind;
        city.set_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET, 1508);
        city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, 2);
        city.set_mac_state_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 31_410);
        city.set_mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET, 5);
        city.set_mac_state_misc_u32_at(0x1024, 13);
        city.set_mac_state_misc_u32_at(0x1050, 1);
        city.set_mac_state_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 86);
        // Reproduce the draw-count inputs from the bounded Flint replay. The
        // first map sample lands in water and the four neighbor rows are active.
        city.set_tile_bits(94, 107, TileBits::new(0x04));
        city.set_misc_u32_values(
            0x06d8,
            &[
                7, 2852, 2945, 0, 36, 473, 461, 0, 32, 1947, 2064, 0, 11, 313, 188, 0,
            ],
        );
        city.set_misc_u32_at(0x01f0 + 0xd5 * 4, 36);
        city.set_misc_u32_at(0x01f0 + 0xd7 * 4, 16);
        for (tile, count) in [(0xd6, 63), (0xd9, 16), (0xd1, 27)] {
            city.set_misc_u32_at(0x01f0 + tile * 4, count);
        }
        for service in [2, 3, 4] {
            set_budget_funding_for_city_test(&mut city, service, 100);
        }
        city.set_misc_u32_values(COHORT_TABLE, &packed);
        city.prepare_mac_fidelity_from_saved_city();
        for (index, count) in [578, 1023, 102, 442, 64, dense_industry]
            .into_iter()
            .enumerate()
        {
            city.set_mac_rci_counter(index + 1, count);
        }
        for (index, value) in [-1424_i32, -2000, -2000, 0x111, 0x222, 0x333, 0x444, 0x555]
            .into_iter()
            .enumerate()
        {
            city.set_misc_u32_at(RCI_DEMAND_MISC_OFFSET + index * 4, value as u32);
        }
        for index in 0..3 {
            let offset = tax_rate_misc_offset_for_city(&city, index).unwrap();
            city.set_budget_misc_u32_values(offset, &[7]);
        }
        for ordinance in [
            ORDINANCE_INCOME_TAX,
            ORDINANCE_SALES_TAX,
            ORDINANCE_ANNUAL_CARNIVAL,
            ORDINANCE_HOMELESS_SHELTER,
            ORDINANCE_POLLUTION_CONTROLS,
        ] {
            assert!(set_city_ordinance_enabled(&mut city, ordinance, true));
        }
        let industry_demand = [33, 20, 26, 9, 21, 35, 24, 24, 15, 46, 21];
        let industry_tax = [6, 7, 7, 7, 7, 6, 7, 7, 7, 7, 7];
        let industry_population = [249, 23, 7, 1, 194, 491, 11, 34, 7, 29, 25];
        for (index, ((demand, tax), population)) in industry_demand
            .into_iter()
            .zip(industry_tax)
            .zip(industry_population)
            .enumerate()
        {
            city.set_misc_u32_values(
                INDUSTRY_MISC_OFFSET + index * INDUSTRY_FIELD_COUNT * 4,
                &[demand, tax, population],
            );
        }

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(1_055_876_497);
        state.sim_calendar_day21_demand_and_graphs();
        assert_eq!(state.mac_process_rng.seed(), outgoing_seed);
        assert_eq!(state.city.mac_population_change_balances, [0, 48]);
        assert_eq!(
            state.city.misc_i32_values(RCI_DEMAND_MISC_OFFSET, 8),
            expected
        );
        assert_eq!(state.city.mac_residential_pair_total(), 1601);
        assert_eq!(state.city.pe_dw_residential_population, 1508);

        let path = std::env::temp_dir().join(format!("citygame-{}-{name}", std::process::id()));
        state.city.save(&path).unwrap();
        let reloaded = City::load(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(reloaded.kind, kind);
        assert_eq!(reloaded.mac_residential_pair_total(), 1601);
        assert_eq!(
            reloaded.misc_i32_values(RCI_DEMAND_MISC_OFFSET, 8),
            expected
        );
        if kind.is_win95() {
            assert_eq!(reloaded.pe_dw_residential_population, 1508);
        }
        for city in [&state.city, &reloaded] {
            assert_eq!(city.misc_u32_values(0x44, 3), cohort_summary);
            let history = city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
            assert_eq!(history[10 * XGRP_SAMPLES_PER_SERIES], cohort_summary[1]);
            assert_eq!(history[11 * XGRP_SAMPLES_PER_SERIES], cohort_summary[2]);
            let hash = city
                .misc_u32_values(COHORT_TABLE, 60)
                .into_iter()
                .flat_map(u32::to_be_bytes)
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
                });
            assert_eq!(hash, cohort_hash);
            let rows = mac_industry_stats_for_city(city);
            assert_eq!(
                rows.map(|row| row.demand),
                [24, 15, 19, 6, 15, 26, 18, 18, 11, 34, 15]
            );
            assert_eq!(rows.map(|row| row.tax as u32), industry_tax);
            assert_eq!(rows.map(|row| row.population), population_rows);
            assert_eq!(
                city.mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
                Some(pollution)
            );
            assert_eq!(
                city.mac_state_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET),
                Some(mix)
            );
        }
    }
}

#[test]
fn pe_rci_ordinances_nudge_tax_rate_index_before_table_lookup() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(
        pe_rci_ordinance_adjusted_tax_rate(&city, RciKind::Commercial, 7),
        7
    );
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_TOURIST_ADVERTISING,
        true
    ));
    assert_eq!(
        pe_rci_ordinance_adjusted_tax_rate(&city, RciKind::Commercial, 7),
        6
    );
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_POLLUTION_CONTROLS,
        true
    ));
    assert_eq!(
        pe_rci_ordinance_adjusted_tax_rate(&city, RciKind::Industrial, 7),
        8
    );
}

#[test]
fn pe_rci_zone_balance_empty_city_seeds_industrial_before_r_and_c() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let scores = pe_rci_zone_balance_scores(&city, PopulationStats::default());
    // I numerator floors at 15; R/C numerators start at 0 → (0/1 - 1) = -1.
    assert!((scores[0] + 1.0).abs() < 1e-9);
    assert!((scores[1] + 1.0).abs() < 1e-9);
    assert!((scores[2] - 14.0).abs() < 1e-9);
    let demand = rci_demand_values_for_city(
        &city,
        PopulationStats::default(),
        CityServiceOutcomes::default(),
    );
    assert_eq!(demand[0], -600);
    assert_eq!(demand[1], -600);
    assert_eq!(demand[2], RCI_DEMAND_MAX);
}

#[test]
fn pe_rci_recreation_numerator_uses_dw_tile_count_like_471e13() {
    // PE `0x468321` twin of FUN_00471BC0 @ `0x471e13`: park/stadium/zoo/marina
    // from `dwTileCount`, not a live XBLD rescan.
    let mut city = City::new_blank("/tmp/sc2000-rci-rec-census.sc2", "RecCensus").expect("blank");
    stamp_complete_building_for_test(&mut city, 30, 30, MARINA_BUILDING_ID, 3);
    assert!(city.pe_dw_tile_count[MARINA_BUILDING_ID as usize] >= 9);
    assert!(building_tile_count_for_city(&city, MARINA_BUILDING_ID) >= 9);
    let with_census = pe_rci_residential_recreation_numerator(&city);
    city.pe_dw_tile_count[MARINA_BUILDING_ID as usize] = 0;
    let stale_zero = pe_rci_residential_recreation_numerator(&city);
    assert_eq!(stale_zero, 10 * 150, "base recreation when census is zero");
    assert!(
        with_census > stale_zero,
        "marina census tiles must lift the recreation numerator"
    );
    assert!(
        building_tile_count_for_city(&city, MARINA_BUILDING_ID) >= 9,
        "map still has marina tiles; numerator must ignore the live rescan"
    );
}

#[test]
fn pe_rci_recreation_numerator_uses_signed_word_inputs() {
    // `FUN_00468150` reads the four cached counts as signed WORDs and uses
    // signed IDIV for big parks; ordinary 128x128 maps cannot reach this
    // high-bit state, but the Rust helper should still preserve the PE width.
    let mut city = City::new_blank("/tmp/sc2000-rci-rec-word.sc2", "RecWord").expect("blank");
    city.pe_dw_tile_count[BIG_PARK_BUILDING_ID as usize] = 0x8001;
    city.pe_dw_tile_count[STADIUM_BUILDING_ID as usize] = 0x8000;
    city.pe_dw_tile_count[ZOO_BUILDING_ID as usize] = 0xffff;
    city.pe_dw_tile_count[MARINA_BUILDING_ID as usize] = 0x7fff;

    assert_eq!(pe_rci_residential_recreation_numerator(&city), -1_637_100);
}

#[test]
fn pe_rci_commercial_infra_numerator_uses_runway_dw_tile_count() {
    let mut city = City::new_blank("/tmp/sc2000-rci-com-census.sc2", "ComCensus").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, RUNWAY_BUILDING_ID, 1);
    assert_eq!(city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize], 1);
    let with_census = pe_rci_commercial_infra_numerator(&city);
    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = 0;
    let stale_zero = pe_rci_commercial_infra_numerator(&city);
    assert_eq!(stale_zero, 1_500, "no runway/conn → (0/5+1)*1500");
    assert!(with_census >= stale_zero);
    assert_eq!(building_tile_count_for_city(&city, RUNWAY_BUILDING_ID), 1);
}

#[test]
fn pe_rci_commercial_infra_uses_network_fa_markers_not_edge_geometry() {
    let mut city =
        City::new_blank("/tmp/sc2000-rci-com-neighbor.sc2", "ComNeighbor").expect("blank");
    city.set_building(
        0,
        20,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    assert!(road_neighbor_connection_tiles_for_city(&city) > 0);
    assert_eq!(pe_rci_commercial_infra_numerator(&city), 1_500);

    for x in 5..9 {
        city.set_building(x, 30, 0x1d);
        city.set_text_overlay(x, 30, 0xfa);
    }
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 4);
    assert_eq!(pe_rci_commercial_infra_numerator(&city), 1_500);

    city.set_building(9, 30, 0x1d);
    city.set_text_overlay(9, 30, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 5);
    assert_eq!(pe_rci_commercial_infra_numerator(&city), 3_000);

    city.set_building(10, 30, 0x70);
    city.set_text_overlay(10, 30, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 5);
    assert_eq!(pe_rci_commercial_infra_numerator(&city), 3_000);
}

#[test]
fn pe_rci_commercial_infra_sign_extends_building_count_words() {
    let mut city =
        City::new_blank("/tmp/sc2000-rci-com-word-width.sc2", "ComWordWidth").expect("blank");
    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = 0xffff;
    assert_eq!(pe_rci_commercial_infra_numerator(&city), 1_500);

    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = 0x8000;
    assert_eq!(
        pe_rci_commercial_infra_numerator(&city),
        -9_828_000,
        "PE MOVSX then signed IDIV truncates -32768 / 5 toward zero"
    );
}

#[test]
fn pe_rci_industrial_infra_numerator_uses_crane_dw_tile_count() {
    let mut city = City::new_blank("/tmp/sc2000-rci-ind-census.sc2", "IndCensus").expect("blank");
    stamp_complete_building_for_test(&mut city, 50, 50, SEAPORT_CRANE_BUILDING_ID, 1);
    assert_eq!(city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize], 1);
    let with_census = pe_rci_industrial_infra_numerator(&city);
    city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize] = 0;
    let stale_zero = pe_rci_industrial_infra_numerator(&city);
    assert_eq!(stale_zero, 1_500, "no crane/conn → (0+1)*1500");
    assert_eq!(with_census, 3_000);
    assert_eq!(
        building_tile_count_for_city(&city, SEAPORT_CRANE_BUILDING_ID),
        1
    );
}

#[test]
fn pe_rci_industrial_infra_uses_accepted_links_not_edge_geometry() {
    let mut city =
        City::new_blank("/tmp/sc2000-rci-ind-links.sc2", "IndLinks").expect("blank city");
    city.set_building(0, 40, 29); // road reaching the west edge
    city.set_building(127, 42, 45); // rail reaching the east edge
    city.set_building(50, 0, 0x4b); // highway footprint at the north edge
    assert!(
        highway_neighbor_connection_tiles_for_city(&city) > 0,
        "fixture should contain actual trip-edge geometry"
    );
    assert_eq!(city.pe_nonnetwork_neighbor_links, 0);
    assert_eq!(
        pe_rci_industrial_infra_numerator(&city),
        1_500,
        "ordinary edge geometry enables trips but is not an accepted paid link"
    );

    city.pe_nonnetwork_neighbor_links = 2;
    assert_eq!(
        pe_rci_industrial_infra_numerator(&city),
        4_500,
        "the native WORD counts accepted links independently of map geometry"
    );

    let saved_path = "/tmp/sc2000-rci-ind-links-reload.sc2";
    city.save(saved_path).expect("save fixture");
    let loaded = City::load(saved_path).expect("reload fixture");
    std::fs::remove_file(saved_path).expect("remove fixture");
    assert_eq!(
        loaded.pe_nonnetwork_neighbor_links, 0,
        "PE clears this BSS on load"
    );
    assert_eq!(pe_rci_industrial_infra_numerator(&loaded), 1_500);
}

#[test]
fn pe_rci_industrial_infra_sign_extends_crane_count_word() {
    let mut city =
        City::new_blank("/tmp/sc2000-rci-ind-word-width.sc2", "IndWordWidth").expect("blank");
    city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize] = 0xffff;
    assert_eq!(pe_rci_industrial_infra_numerator(&city), 0);

    city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize] = 0x8000;
    assert_eq!(
        pe_rci_industrial_infra_numerator(&city),
        -49_150_500,
        "PE MOVSX crane count is incremented and scaled by 1500"
    );
}

#[test]
fn pe_rci_industrial_numerator_matches_x87_instruction_order() {
    let mut city = City::new_blank("/tmp/sc2000-rci-x87-num.sc2", "RciX87Num").expect("blank city");
    city.set_pe_zone_pops([15_699, 27_632, 7_078]);
    // Isolate x87 operand order with this explicitly supplied global. The live
    // monthly publisher normally sets it from all six aggregate slots.
    city.persist_pe_day22_population_leftover(15_699 * 10);
    city.pe_dw_residential_population = 25_973;
    city.pe_w_city_difficulty = CITY_DIFFICULTY_HARD as u16;
    city.set_pe_w_industrial_mix_bonus(60);
    // Keep both PE infrastructure caps above their recovered numerators.
    city.pe_dw_tile_count[SEAPORT_CRANE_BUILDING_ID as usize] = 5;
    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = 25;

    let numerators = pe_rci_zone_balance_numerators(&city, PopulationStats::default());
    let binary64_ratio = 25_973.0 / (1.0 + 27_632.0 + 7_078.0);
    let binary64_reordered = 7_078.0 * binary64_ratio * (60.0 * 0.01 + 0.95);
    let old_total_population = (15_699 + 27_632 + 7_078) * 10;
    let old_commercial =
        7_078.0 * (f64::from(old_total_population) + 50_000.0) * (1.0 / 150_000.0) * binary64_ratio;

    assert_eq!(numerators[1], 7_308.422630493696);
    assert_eq!(numerators[2], 8_209.132139667541);
    assert_eq!(binary64_reordered, 8_209.13213966754);
    assert_eq!(old_commercial.min(9_000.0), 9_000.0);
    assert_ne!(
        numerators[1].to_bits(),
        old_commercial.min(9_000.0).to_bits()
    );
    assert_ne!(numerators[2].to_bits(), binary64_reordered.to_bits());
    assert_eq!(
        pe_rci_demand_values_from_previous(&city, PopulationStats::default(), [0; 3])[1],
        -441,
        "PE uses the explicitly supplied 0x4caa74 population snapshot"
    );
}

#[test]
fn pe_commercial_demand_uses_published_total_instead_of_residential_row() {
    let mut city = City::new_blank("/tmp/sc2000-rci-total.sc2", "RciTotal").unwrap();
    city.set_pe_zone_pops([20, 30, 40]);
    city.pe_dw_residential_population = 10;
    city.persist_pe_day22_population_leftover((20 + 30 + 40) * 10);
    let actual = pe_rci_zone_balance_numerators(&city, PopulationStats::default())[1];
    // Original PE x86+x87 replay, starting with growth slots [20,0,30,0,40,0].
    assert_eq!(actual.to_bits(), 1.911737089201878_f64.to_bits());
    let expected = (50_000.0 + 900.0) / 150_000.0 * (10.0 / 71.0) * 40.0;
    assert!((actual - expected).abs() < 1e-12);

    city.persist_pe_day22_population_leftover(200);
    let different_global = pe_rci_zone_balance_numerators(&city, PopulationStats::default())[1];
    assert!(
        actual > different_global,
        "the original reads the published global directly"
    );
}

#[test]
fn pe_commercial_numerator_matches_original_x87_replay() {
    let fixture =
        include_str!("../../../tests/fixtures/simulation/win95/pe_commercial_numerator.csv");
    let mut city = City::new_blank("/tmp/pe-commercial-replay.sc2", "Replay").unwrap();
    // The replay stops before the infrastructure cap; keep it out of range.
    city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = 5_000;
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let values: Vec<u64> = line.split(',').map(|v| v.parse().unwrap()).collect();
        assert_eq!(values.len(), 12);
        let [r, c, i] = [values[8] as i32, values[9] as i32, values[10] as i32];
        city.set_pe_zone_pops([r, c, i]);
        city.pe_dw_residential_population = values[6] as u32;
        city.persist_pe_day22_population_leftover(values[7] as u32);
        let actual = pe_rci_zone_balance_numerators(&city, PopulationStats::default())[1];
        assert_eq!(actual.to_bits(), values[11], "{line}");
        count += 1;
    }
    assert_eq!(count, 132);
}

#[test]
fn pe_commercial_cap_matches_original_x87_replay() {
    let fixture = include_str!("../../../tests/fixtures/simulation/win95/pe_commercial_cap.csv");
    let mut city = City::new_blank("/tmp/pe-commercial-cap.sc2", "Replay").unwrap();
    for x in 10..15 {
        city.set_building(x, 30, 0x1d);
    }
    let mut count = 0;
    let mut capped = 0;
    let mut unchanged = 0;
    for line in fixture.lines().skip(1) {
        let values: Vec<u64> = line.split(',').map(|v| v.parse().unwrap()).collect();
        assert_eq!(values.len(), 16);
        city.set_pe_zone_pops([values[11] as i32, values[12] as i32, values[13] as i32]);
        city.pe_dw_residential_population = values[6] as u32;
        city.persist_pe_day22_population_leftover(values[10] as u32);
        city.pe_dw_tile_count[RUNWAY_BUILDING_ID as usize] = values[7] as u16;
        city.pe_dw_tile_count[AIRPORT_RUNWAY_CROSS_BUILDING_ID as usize] = values[8] as u16;
        for x in 10..15 {
            city.set_text_overlay(x, 30, if x - 10 < values[9] as usize { 0xfa } else { 0 });
        }
        let actual = pe_rci_zone_balance_numerators(&city, PopulationStats::default())[1];
        assert_eq!(actual.to_bits(), values[15], "{line}");
        if values[14] == values[15] {
            unchanged += 1;
        } else {
            capped += 1;
        }
        count += 1;
    }
    assert_eq!((count, capped, unchanged), (132, 128, 4));
}

#[test]
fn pe_rci_zone_balance_uses_win95_difficulty_table() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_EASY);
    city.pe_dw_residential_population = 100;
    let population = PopulationStats {
        industrial_population: 100,
        ..Default::default()
    };
    let easy = pe_rci_zone_balance_scores(&city, population)[2];
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
    let hard = pe_rci_zone_balance_scores(&city, population)[2];
    assert!(easy > hard);
}

#[test]
fn dense_rci_development_does_not_require_world_grid_alignment() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    let developed: Vec<_> = building_footprint_from_origin(20, 20, 3)
        .unwrap()
        .into_iter()
        .filter(|&(x, y)| is_construction_building(state.city.building_at(x, y)))
        .collect();
    assert_eq!(
        developed.len(),
        vacant_rci_transit_ring_count_for_test(&state.city, (20, 20), 3),
        "expected dense vacant growth to start as 1x1 construction on the PE transit ring"
    );
}

#[test]
fn dense_rci_development_variant_uses_city_quality_band() {
    fn developed_residential_variant_for_land_value(land_value: u32, xval: u8) -> u8 {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, land_value);
        stamp_xval_for_test(&mut city, xval);
        for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
        stamp_transport_access_for_test(&mut city, 19, 21);
        power_adjacent_building_for_test(&mut city, 20, 19);
        water_footprint_from_below_for_test(&mut city, 20, 20, 3);
        set_rci_demand_for_test(&mut city, 1_000, 0, 0);
        let expected = vacant_rci_transit_ring_count_for_test(&city, (20, 20), 3);
        let mut state = test_state(city);

        assert_eq!(state.develop_rci_until_built_for_test(), expected);
        let developed: Vec<_> = building_footprint_from_origin(20, 20, 3)
            .unwrap()
            .into_iter()
            .filter(|&(x, y)| RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(x, y)))
            .collect();
        assert_eq!(developed.len(), expected);
        state.city.building_at(developed[0].0, developed[0].1)
    }

    // PE's pop-1 chooser reads the tile XVAL byte (`XVAL >> 6`), not the
    // city-wide percentage stored in MISC.
    let low_value = developed_residential_variant_for_land_value(10, 10);
    let high_value = developed_residential_variant_for_land_value(90, 160);

    assert!(
        RESIDENTIAL_1X1_BUILDINGS[..4].contains(&low_value),
        "expected low-value city to use lower-class 1x1 residential, got {low_value:#04x}"
    );
    assert!(
        RESIDENTIAL_1X1_BUILDINGS[8..].contains(&high_value),
        "expected high-value city to use upper-class 1x1 residential, got {high_value:#04x}"
    );
}

#[test]
fn rci_quality_bands_match_sc2k_residential_classes() {
    for &(building, size, expected_band) in &[
        (0x70, 1, 0usize),
        (0x73, 1, 0),
        (0x74, 1, 1),
        (0x77, 1, 1),
        (0x78, 1, 2),
        (0x7B, 1, 2),
        (0x8C, 2, 0),
        (0x8E, 2, 0),
        (0x8F, 2, 1),
        (0x90, 2, 1),
        (0x91, 2, 2),
        (0x93, 2, 2),
        (0xAE, 3, 0),
        (0xAF, 3, 0),
        (0xB0, 3, 2),
        (0xB1, 3, 2),
    ] {
        let zone_id = if size == 1 {
            ZONE_RESIDENTIAL_LIGHT
        } else {
            ZONE_RESIDENTIAL_DENSE
        };
        let buildings = rci_development_buildings_for_zone_and_size(zone_id, size).unwrap();
        let position = buildings
            .iter()
            .position(|&candidate| candidate == building)
            .expect("residential class table should contain building");
        let (start, end) =
            rci_development_variant_band_range(zone_id, size, buildings, expected_band);
        assert!(
            (start..end).contains(&position),
            "building {building:#04x} size {size} should be quality band {expected_band}"
        );
    }

    let dense = ZONE_RESIDENTIAL_DENSE;
    let buildings_2 = rci_development_buildings_for_zone_and_size(dense, 2).unwrap();
    let buildings_3 = rci_development_buildings_for_zone_and_size(dense, 3).unwrap();
    assert_eq!(
        rci_development_variant_band_range(dense, 2, buildings_2, 0),
        (0, 3)
    );
    assert_eq!(
        rci_development_variant_band_range(dense, 2, buildings_2, 1),
        (3, 5)
    );
    assert_eq!(
        rci_development_variant_band_range(dense, 2, buildings_2, 2),
        (5, 8)
    );
    assert_eq!(
        rci_development_variant_band_range(dense, 3, buildings_3, 0),
        (0, 2)
    );
    assert_eq!(
        rci_development_variant_band_range(dense, 3, buildings_3, 1),
        (0, 2)
    );
    assert_eq!(
        rci_development_variant_band_range(dense, 3, buildings_3, 2),
        (2, 4)
    );
}

#[test]
fn rci_choose_and_place_uses_xval_band_for_residential_pop1() {
    // Win95: building = 0x70 + min(XVAL>>6, 2)*4 + (rand&3)
    let mut low = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(4)
            .wrapping_add(0_u32 << 8)
            .wrapping_add(1 << 16)
            .wrapping_add(u32::from(ZONE_RESIDENTIAL_DENSE) << 24),
    );
    let low_variant = low.random_u16_mask(3) as u8;
    assert_eq!(
        rci_choose_and_place_building(ZONE_RESIDENTIAL_DENSE, 1, 4, 0, Some(0)),
        Some(0x70 + low_variant)
    );
    assert_eq!(
        rci_choose_and_place_building(ZONE_RESIDENTIAL_DENSE, 1, 4, 0, Some(64)),
        Some(0x74 + low_variant),
        "same isolated CRT draw; XVAL band shifts the 4-wide wealth class"
    );
    assert_eq!(
        rci_choose_and_place_building(ZONE_RESIDENTIAL_DENSE, 1, 4, 0, Some(192)),
        Some(0x78 + low_variant)
    );
    let commercial = rci_choose_and_place_building(ZONE_COMMERCIAL_DENSE, 2, 1, 2, None).unwrap();
    assert!(
        COMMERCIAL_2X2_BUILDINGS[0..5].contains(&commercial),
        "PE pop2 commercial is base 0x94 count 5, got {commercial:#04x}"
    );
}

#[test]
fn arco_microsim_stat0_reads_density_via_pe_microsim_coords() {
    // PE DoUpdateMicrosimGrowthTick arco @ 0x435f35: XVAL/XCRM/XPLT after x&=~1, y>>=1.
    let mut city = City::new_blank("/tmp/sc2000-arco-micro.sc2", "ArcoMicro").expect("blank");
    let arco_text = city
        .provision_microsim_for_building(251)
        .expect("arco microsim");
    let arco_index = usize::from(arco_text - MIN_SIM_TEXT_ENTRIES);
    // 2×2 arco; update from SE tile (21,21) with TRIGHT.
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 251);
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_text_overlay(x, y, arco_text);
        city.set_tile_bits(x, y, TileBits::new(0).with_watered(true));
    }
    city.set_zone(21, 21, ZONE_RESIDENTIAL_DENSE | ZONE_CORNER_TRIGHT);
    let xval = vec![0u8; CITY_TILES];
    let mut crime = vec![0u8; CITY_TILES];
    let pollution = vec![0u8; CITY_TILES];
    // NW microsim cell clean → (0-0-0)+12 = 12.
    // SE tile alone has maximum crime, but must not affect the coarse NW sample.
    crime[21 * CITY_WIDTH + 21] = 255;
    city.sync_land_value_from_slice(&xval);
    city.sync_crime_density_from_slice(&crime);
    city.sync_pollution_density_from_slice(&pollution);
    let mut state = test_state(city.clone());
    assert!(state.update_arco_microsim_at_for_test(21, 21));
    assert_eq!(
        state.city.microsim_at(arco_index).map(|r| r.stat0),
        Some(6),
        "SE update must score clean NW microsim (12) then /2 unpowered — not raw SE (0)"
    );

    // PE @ `0x435fb2` reads the saved XBIT on the actual update tile. No connected
    // source is needed when that published bit is set.
    let mut saved_powered_city = city.clone();
    let bits = saved_powered_city.tile_bits_at(21, 21).unwrap_or_default();
    saved_powered_city.set_tile_bits(21, 21, bits.with_powered(true));
    let mut saved_powered_state = test_state(saved_powered_city);
    assert!(saved_powered_state.update_arco_microsim_at_for_test(21, 21));
    assert_eq!(
        saved_powered_state
            .city
            .microsim_at(arco_index)
            .map(|r| r.stat0),
        Some(12),
        "saved XBIT 0x40 must avoid the unpowered half penalty"
    );

    // XCRM itself must affect the score when the coarse NW cell is dirty. With
    // zero XVAL/XPLT, PE's 255 >> 5 crime penalty makes 12 - 7 = 5, then the
    // unpowered network halves it to 2.
    crime[20 * CITY_WIDTH + 20] = 255;
    let mut nw_crime_city = city;
    nw_crime_city.sync_crime_density_from_slice(&crime);
    let mut nw_crime_state = test_state(nw_crime_city);
    assert!(nw_crime_state.update_arco_microsim_at_for_test(21, 21));
    assert_eq!(
        nw_crime_state.city.microsim_at(arco_index).map(|r| r.stat0),
        Some(2),
        "NW XCRM must reduce the SE arcology score by the PE crime penalty"
    );
}

#[test]
fn rci_development_building_uses_crt_not_xor() {
    assert_eq!(
        rci_development_building(ZONE_RESIDENTIAL_DENSE, 1, 4, 0, 0),
        rci_choose_and_place_building(ZONE_RESIDENTIAL_DENSE, 1, 4, 0, Some(0)),
        "quality band 0 maps to the PE selector's XVAL band 0; this does not model a missing PE chunk"
    );
    assert_eq!(
        rci_development_building(ZONE_RESIDENTIAL_DENSE, 1, 4, 0, 2),
        rci_choose_and_place_building(ZONE_RESIDENTIAL_DENSE, 1, 4, 0, Some(128)),
        "quality band 2 is ChooseAndPlace with XVAL>>6 == 2"
    );

    let zone_id = ZONE_RESIDENTIAL_DENSE;
    let buildings = rci_development_buildings_for_zone_and_size(zone_id, 2).unwrap();
    let (start, end) = rci_development_variant_band_range(zone_id, 2, buildings, 0);
    let mut rng = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(4)
            .wrapping_add(0_u32 << 8)
            .wrapping_add(2 << 16)
            .wrapping_add(u32::from(zone_id) << 24),
    );
    let count = u16::try_from(end - start).unwrap().max(1);
    let variant = usize::from(rng.random_mod(count));
    assert_eq!(
        rci_development_building(zone_id, 2, 4, 0, 0),
        Some(buildings[start + variant]),
        "2x2 quality slice uses isolated CRT rand%count, not XOR"
    );
}

#[test]
fn rci_start_building_construction_uses_crt_mod_count() {
    // Win95 StartBuilding vacant: ChooseAndPlace(x,y,1,3) → 0x88 + rand%2.
    let mut rng = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(20)
            .wrapping_add(20_u32 << 8)
            .wrapping_add(1 << 16)
            .wrapping_add(u32::from(CHOOSE_AND_PLACE_CONSTRUCTION_MODE) << 24),
    );
    let variant = rng.random_mod(2) as u8;
    assert_eq!(
        construction_building_for_pop(1, 20, 20),
        Some(0x88 + variant)
    );
    let mut size_up = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(4)
            .wrapping_add(0_u32 << 8)
            .wrapping_add(4 << 16)
            .wrapping_add(u32::from(CHOOSE_AND_PLACE_CONSTRUCTION_MODE) << 24),
    );
    let size_up_variant = size_up.random_mod(2) as u8;
    assert_eq!(
        construction_building_for_pop(4, 4, 0),
        Some(0xC2 + size_up_variant)
    );
    assert!(
        CONSTRUCTION_POP1_BUILDINGS.contains(&construction_building_for_pop(1, 21, 20).unwrap())
    );
}

#[test]
fn rci_change_building_abandon_uses_crt_mod_count() {
    // Win95 ChangeBuilding: ChooseAndPlace(x,y,pop,4) → 0x8A/0xC4 + rand%2.
    let mut rng = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(20)
            .wrapping_add(20_u32 << 8)
            .wrapping_add(1 << 16)
            .wrapping_add(u32::from(CHOOSE_AND_PLACE_ABANDONED_MODE) << 24),
    );
    let variant = rng.random_mod(2) as u8;
    assert_eq!(abandoned_building_for_pop(1, 20, 20), Some(0x8A + variant));
    let mut large = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(4)
            .wrapping_add(0_u32 << 8)
            .wrapping_add(4 << 16)
            .wrapping_add(u32::from(CHOOSE_AND_PLACE_ABANDONED_MODE) << 24),
    );
    let large_variant = large.random_mod(2) as u8;
    assert_eq!(
        abandoned_building_for_pop(4, 4, 0),
        Some(0xC4 + large_variant)
    );
    assert!(ABANDONED_POP1_BUILDINGS.contains(&abandoned_building_for_pop(1, 21, 20).unwrap()));
}

#[test]
fn rci_change_building_replace_tile_uses_crt_lsb() {
    // Win95 growth tick: iReplaceTile = rand() & 1.
    let mut rng = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(20)
            .wrapping_add(20_u32 << 8)
            .wrapping_add(u32::from(CHANGEBUILDING_REPLACE_KEY) << 24),
    );
    assert_eq!(
        rci_change_building_replace_tile(20, 20),
        rng.random_u16_mask(1) != 0
    );
}

#[test]
fn rci_growth_demand_thresholds_match_win95_formulas() {
    // iCurrentDemand = demand + 2000
    assert_eq!(rci_growth_current_demand(0), 2000);
    assert_eq!(rci_growth_current_demand(-2000), 0);
    assert_eq!(rci_growth_current_demand(2500), 4000);
    // StartBuilding: 3 * current / (pop+1)
    assert_eq!(rci_growth_start_threshold(0, 0), 6000);
    assert_eq!(rci_growth_start_threshold(0, 1), 3000);
    // Construction: 16384 / pop
    assert_eq!(rci_construction_complete_threshold(1), 16384);
    assert_eq!(rci_construction_complete_threshold(2), 8192);
    // Abandoned redevelop: 15 * current / pop
    assert_eq!(rci_abandoned_redevelop_threshold(0, 1), 30_000);
    assert_eq!(rci_growth_remainder_demand(0, false), 4000);
    assert_eq!(rci_growth_remainder_demand(0, true), 2000);
    assert_eq!(rci_growth_remainder_demand(-2000, true), 4000);
    assert_eq!(rci_growth_remainder_demand(2000, true), 0);
    assert_eq!(rci_growth_change_threshold(4000, 1), 4000);
    assert_eq!(rci_growth_change_threshold(4000, 2), 2000);
    assert!(rci_growth_roll_succeeds(1, 0, 0, 0));
    assert!(!rci_growth_roll_succeeds(0, 0, 0, 0));
    assert!(
        !rci_growth_roll_succeeds(
            rci_growth_change_threshold(0, 1),
            0,
            0,
            GROWTH_ROLL_CHANGE_KEY
        ),
        "remainder 0 never ChangeBuilding"
    );
}

#[test]
fn pe_rci_growth_roll_succeeds_with_rng_advances_crt() {
    let mut rng = Win95CrtRand::new(0x0043_6D00);
    let before = rng.seed();
    assert!(pe_rci_growth_roll_succeeds_with_rng(
        u32::from(u16::MAX) + 1,
        &mut rng
    ));
    assert_ne!(rng.seed(), before);
    let mut low = Win95CrtRand::new(0x0043_6D00);
    assert!(!pe_rci_growth_roll_succeeds_with_rng(0, &mut low));
    assert_eq!(low.seed(), 0x0043_6D00);
}

#[test]
fn startbuilding_growth_roll_matches_pe_boundaries_and_zero_draw() {
    fn seed_for_first_crt_result(value: u16) -> u32 {
        (u32::from(value) << 16)
            .wrapping_sub(0x0026_9EC3)
            .wrapping_mul(0xB9B3_3155)
    }

    for (pop, expected_threshold) in [(0, 6_000), (1, 3_000), (2, 2_000), (3, 1_500)] {
        let threshold = rci_growth_start_threshold(0, pop);
        assert_eq!(threshold, expected_threshold, "pop={pop}");

        for (roll, expected_success) in [(threshold - 1, true), (threshold, false)] {
            let seed = seed_for_first_crt_result(roll as u16);
            let mut probe = Win95CrtRand::new(seed);
            assert_eq!(probe.random_u15(), roll as u16);

            let mut actual = Win95CrtRand::new(seed);
            assert_eq!(
                pe_rci_growth_roll_succeeds_with_rng_always_draw(threshold, &mut actual),
                expected_success,
                "pop={pop}, roll={roll}"
            );
            assert_eq!(
                actual.seed(),
                probe.seed(),
                "one CRT rand() draw for pop={pop}, roll={roll}"
            );
        }
    }

    assert_eq!(rci_growth_current_demand(-2_000), 0);
    for pop in 0..=3 {
        assert_eq!(rci_growth_start_threshold(-2_000, pop), 0, "pop={pop}");
    }
    let mut actual = Win95CrtRand::new(0x0043_6D05);
    let mut probe = actual;
    assert!(!pe_rci_growth_roll_succeeds_with_rng_always_draw(
        0,
        &mut actual
    ));
    probe.random_u15();
    assert_eq!(
        actual.seed(),
        probe.seed(),
        "zero threshold still draws once"
    );

    let mut isolated = Win95CrtRand::new(0x0043_6D06);
    let before = isolated.seed();
    assert!(!pe_rci_startbuilding_growth_roll_succeeds_optional(
        0,
        20,
        20,
        GROWTH_ROLL_START_VACANT_KEY,
        Some(&mut isolated),
    ));
    assert_eq!(
        isolated.seed(),
        before,
        "test isolation stays deterministic"
    );
}

#[test]
fn growth_change_roll_matches_pe_boundary_and_picker_order() {
    fn seed_for_first_crt_result(value: u16) -> u32 {
        (u32::from(value) << 16)
            .wrapping_sub(0x0026_9EC3)
            .wrapping_mul(0xB9B3_3155)
    }

    for (pop, expected_threshold) in [(1, 4_000), (2, 2_000), (3, 1_333), (4, 1_000)] {
        let threshold = rci_growth_change_threshold(4_000, pop);
        assert_eq!(threshold, expected_threshold, "pop={pop}");

        for roll in [threshold - 1, threshold] {
            let seed = seed_for_first_crt_result(roll as u16);
            let mut probe = Win95CrtRand::new(seed);
            assert_eq!(probe.random_u15(), roll as u16);
            let expected_replace = if roll < threshold {
                Some(probe.random_u16_mask(1) != 0)
            } else {
                None
            };

            let mut actual = Win95CrtRand::new(seed);
            assert_eq!(
                pe_rci_growth_change_roll_then_replace_with_rng(threshold, &mut actual),
                expected_replace,
                "pop={pop}, roll={roll}"
            );
            assert_eq!(
                actual.seed(),
                probe.seed(),
                "roll precedes conditional iReplaceTile draw"
            );

            if pop == 1 && expected_replace.is_some() {
                let expected_picker =
                    abandoned_building_for_pop_with_rng(1, &mut probe).expect("pop1 picker");
                let actual_picker =
                    abandoned_building_for_pop_with_rng(1, &mut actual).expect("pop1 picker");
                assert_eq!(
                    actual_picker, expected_picker,
                    "ChangeBuilding picker follows roll and iReplaceTile"
                );
                assert_eq!(actual.seed(), probe.seed());
            }
        }
    }

    assert_eq!(rci_growth_remainder_demand(2_000, true), 0);
    for pop in 1..=4 {
        assert_eq!(rci_growth_change_threshold(0, pop), 0, "pop={pop}");
    }
    let mut actual = Win95CrtRand::new(0x0043_6D07);
    let mut probe = actual;
    assert_eq!(
        pe_rci_growth_change_roll_then_replace_with_rng(0, &mut actual),
        None,
        "zero threshold fails before iReplaceTile or ChangeBuilding"
    );
    probe.random_u15();
    assert_eq!(
        actual.seed(),
        probe.seed(),
        "zero threshold still consumes roll draw"
    );

    let mut isolated = Win95CrtRand::new(0x0043_6D08);
    let before = isolated.seed();
    assert_eq!(
        pe_rci_growth_change_roll_then_replace_optional(
            0,
            20,
            20,
            GROWTH_ROLL_CHANGE_KEY,
            Some(&mut isolated),
        ),
        None
    );
    assert_eq!(
        isolated.seed(),
        before,
        "test isolation stays deterministic"
    );
}

#[test]
fn pe_rci_growth_roll_succeeds_optional_keeps_test_isolation() {
    let mut rng = Win95CrtRand::new(0x0043_6D01);
    let before = rng.seed();
    assert!(pe_rci_growth_roll_succeeds_optional(
        1,
        10,
        10,
        GROWTH_ROLL_START_VACANT_KEY,
        Some(&mut rng),
    ));
    assert_eq!(
        rng.seed(),
        before,
        "cfg(test) must keep isolation/zero-roll and not consume process CRT"
    );
}

#[test]
fn pe_growth_keeps_startbuilding_draws_when_power_or_trip_fails() {
    let _live_rng = LiveGrowthRng::enable();
    let mut city = City::new_blank("/tmp/citygame-unserved-growth.sc2", "Unserved").unwrap();
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE);
    city.set_building(21, 20, 0x1d);
    city.set_zone(24, 24, ZONE_INDUSTRIAL_DENSE | ZONE_CORNER_ALL);
    city.set_building(24, 24, 0x85);
    city.set_zone(32, 32, ZONE_RESIDENTIAL_DENSE);
    set_rci_demand_for_test(&mut city, 2000, 2000, 1408);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = false;
    state.pe_process_crt = Win95CrtRand::new(2_503_793_637);

    state.sim_calendar_one_growth_tick(0, 0);

    // Native 0x4358b0: 0x4362b7 at (20,20), then 0x43611a and
    // 0x4362b7 at (24,24). The vacancy with no nearby transit exits earlier.
    assert_eq!(state.pe_process_crt_seed(), 255_755_822);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.building_at(24, 24), 0x85);
    assert_eq!(state.city.building_at(32, 32), 0);
}

#[test]
fn rci_candidate_growth_rolls_keep_test_isolation_with_process_crt_arg() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE);
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    let mut rng = Win95CrtRand::new(0x0043_6D02);
    let before = rng.seed();
    let _ = state.rci_development_candidate_at(
        20,
        20,
        &reserved,
        &power_maps,
        city_service_outcomes_for_city(&state.city),
        &rci_development_services_for_city(&state.city),
        Some(&mut rng),
    );
    assert_eq!(
        rng.seed(),
        before,
        "candidate helpers must not consume process CRT under cfg(test)"
    );
}

#[test]
fn develop_rci_stamp_writeback_keeps_growth_rng_stream() {
    // After StartBuilding stamp, pe_process_crt must be the growth_rng writeback
    // (not a discarded fork that stamped on a stale process copy).
    let mut city = City::new_blank("/tmp/sc2000-stamp-crt.sc2", "StampCrt").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0x1111_2222);
    let before = state.pe_process_crt_seed();
    assert_eq!(state.develop_rci_zones(), 1);
    assert!(is_construction_building(state.city.building_at(20, 20)));
    // cfg(test) growth rolls keep isolation (no CRT advance); writeback still round-trips.
    assert_eq!(state.pe_process_crt_seed(), before);
}

#[test]
fn rci_growth_roll_crt_uses_choose_and_place_isolation() {
    let mut rng = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(20)
            .wrapping_add(20_u32 << 8)
            .wrapping_add(u32::from(GROWTH_ROLL_START_VACANT_KEY) << 24),
    );
    assert_eq!(
        rci_growth_roll_crt(20, 20, GROWTH_ROLL_START_VACANT_KEY),
        rng.random_u16()
    );
    assert_eq!(
        rci_growth_roll(20, 20, GROWTH_ROLL_START_VACANT_KEY),
        0,
        "cfg(test) still forces demand-threshold commits"
    );
    let mut change = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(20)
            .wrapping_add(20_u32 << 8)
            .wrapping_add(u32::from(GROWTH_ROLL_CHANGE_KEY) << 24),
    );
    assert_eq!(
        rci_growth_roll_crt(20, 20, GROWTH_ROLL_CHANGE_KEY),
        change.random_u16()
    );
}

#[test]
fn rci_place_church_on_complete_matches_pe_tick_pop_and_zone_gate() {
    for (zone, pop, expected) in [
        (ZONE_RESIDENTIAL_LIGHT, 1, false),
        (ZONE_RESIDENTIAL_LIGHT, 2, true),
        (ZONE_RESIDENTIAL_LIGHT, 3, true),
        (ZONE_RESIDENTIAL_LIGHT, 4, false),
        (ZONE_RESIDENTIAL_DENSE, 1, false),
        (ZONE_RESIDENTIAL_DENSE, 2, true),
        (ZONE_RESIDENTIAL_DENSE, 3, true),
        (ZONE_RESIDENTIAL_DENSE, 4, false),
        (ZONE_COMMERCIAL_LIGHT, 2, false), // PE's exact `zone >= 3` boundary
        (ZONE_COMMERCIAL_DENSE, 2, false),
        (ZONE_INDUSTRIAL_LIGHT, 3, false),
        (ZONE_INDUSTRIAL_DENSE, 3, false),
    ] {
        assert_eq!(
            rci_place_church_on_construction_complete(true, zone, pop),
            expected,
            "tick=true, zone={zone}, pop={pop}"
        );
    }
    assert!(!rci_place_church_on_construction_complete(
        false,
        ZONE_RESIDENTIAL_DENSE,
        2,
    ));
    // church_tiles * 2500 < leftover 0x4caa74. 0 churches need leftover > 0; 4 tiles need > 10000.
    assert!(!pe_b_place_church(0, 0));
    assert!(pe_b_place_church(0, 1));
    assert!(!pe_b_place_church(4, 10_000));
    assert!(pe_b_place_church(4, 10_001));
}

#[test]
fn rci_decline_change_building_splits_and_scatters() {
    let footprint_2 = building_footprint_from_origin(10, 10, 2).unwrap();
    let split = rci_decline_change_building(0x8C, &footprint_2, 10, 10, true).unwrap();
    assert_eq!(split.len(), 4);
    assert!(
        split
            .iter()
            .all(|&(_, _, b)| ABANDONED_POP1_BUILDINGS.contains(&b))
    );

    let downgrade = rci_decline_change_building(0x90, &footprint_2, 10, 10, true).unwrap();
    assert_eq!(downgrade.len(), 1);
    assert!(ABANDONED_POP2_BUILDINGS.contains(&downgrade[0].2));

    let footprint_3 = building_footprint_from_origin(20, 20, 3).unwrap();
    let scatter = rci_decline_change_building(0xAE, &footprint_3, 20, 20, true).unwrap();
    assert!(scatter.len() >= 8);
    assert!(
        scatter
            .iter()
            .any(|&(_, _, b)| ABANDONED_POP3_BUILDINGS.contains(&b))
    );
    assert!(
        scatter
            .iter()
            .filter(|&&(_, _, b)| ABANDONED_POP1_BUILDINGS.contains(&b))
            .count()
            >= 8
    );
    let mut scatter_rng = Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(20)
            .wrapping_add(20_u32 << 8)
            .wrapping_add(4 << 16)
            .wrapping_add(u32::from(CHANGEBUILDING_SCATTER_KEY) << 24),
    );
    let r = scatter_rng.random_u16_mask(3);
    let pop3 = scatter
        .iter()
        .find(|&&(_, _, b)| ABANDONED_POP3_BUILDINGS.contains(&b))
        .unwrap();
    assert_eq!(pop3.0, 20 + usize::from(r & 1));
    assert_eq!(pop3.1, 20 + usize::from(r >> 1));

    let same = rci_decline_change_building(0xAE, &footprint_3, 20, 20, false).unwrap();
    assert_eq!(same.len(), 1);
    assert!(ABANDONED_POP4_BUILDINGS.contains(&same[0].2));
}

#[test]
fn missing_rci_demand_misc_does_not_force_growth() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    // Leave RCI demand MISC unset.
    assert_eq!(saved_rci_demand_for_kind(&city, RciKind::Residential), None);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_until_built_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn construction_completes_without_saved_rci_demand() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, CONSTRUCTION_POP1_BUILDINGS[0]);
    assert_eq!(saved_rci_demand_for_kind(&city, RciKind::Residential), None);
    let mut state = test_state(city);

    assert_eq!(state.complete_rci_construction_sites(), 1);
    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "BUILD_THINK should complete from 16384/pop even before demand MISC is written"
    );
}

#[test]
fn simulation_month_uses_live_outcomes_for_rci_quality_band() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 90);
    city.set_zone(80, 80, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(80, 80, RESIDENTIAL_1X1_BUILDINGS[0]);
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();
    state.handle_simulation_month_advanced();

    let building = state.city.building_at(20, 20);
    assert!(
        !RESIDENTIAL_3X3_BUILDINGS[2..].contains(&building),
        "expected monthly growth to avoid stale saved high land value, got high-band building {building:#04x}"
    );
}

#[test]
fn dense_rci_development_variant_uses_xval_after_local_service_quality_is_scored() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 65);
    for (x, y) in building_footprint_from_origin(50, 50, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_transport_access_for_test(&mut city, 49, 51);
    powered_line_corridor_for_test(&mut city, 50, 52, 49);
    water_footprint_from_below_for_test(&mut city, 50, 50, 3);
    stamp_complete_building_for_test(&mut city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut city, 30, 36, 211, 3);
    stamp_complete_building_for_test(&mut city, 30, 42, 209, 3);
    stamp_complete_building_for_test(&mut city, 36, 34, 214, 3);
    power_adjacent_building_for_test(&mut city, 30, 29);
    power_adjacent_building_for_test(&mut city, 30, 35);
    power_adjacent_building_for_test(&mut city, 30, 41);
    power_adjacent_building_for_test(&mut city, 36, 33);
    stamp_complete_building_for_test(&mut city, 46, 50, 13, 1);
    stamp_complete_building_for_test(&mut city, 34, 50, 233, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, 160);
    let transit_eligible = vacant_rci_transit_ring_count_for_test(&city, (50, 50), 3);
    let mut state = test_state(city);
    mark_transit_ring_served_for_test(&mut state, (50, 50), 3);

    let footprint = building_footprint_from_origin(50, 50, 3).unwrap();
    let quality = rci_development_quality_score_for_footprint(
        &state.city,
        ZONE_RESIDENTIAL_DENSE,
        &footprint,
        city_service_outcomes_for_city(&state.city),
        &rci_development_services_for_city(&state.city),
    );
    assert!(
        quality >= 70,
        "local funded services should raise quality, got {quality}"
    );

    state.develop_rci_until_built_for_test();
    let developed: Vec<_> = building_footprint_from_origin(50, 50, 3)
        .unwrap()
        .into_iter()
        .filter(|&(x, y)| RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(x, y)))
        .collect();
    assert!(
        !developed.is_empty() && developed.len() <= transit_eligible,
        "per-tile growth rolls should select a nonempty subset of transit-eligible cells"
    );
    let building = state.city.building_at(developed[0].0, developed[0].1);
    assert!(
        RESIDENTIAL_1X1_BUILDINGS[8..].contains(&building),
        "expected locally covered footprint to use high 1x1 residential variant, got {building:#04x}"
    );
}

#[test]
fn unfunded_local_services_do_not_raise_rci_quality_band() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 65);
    for (x, y) in building_footprint_from_origin(50, 50, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_transport_access_for_test(&mut city, 49, 51);
    powered_line_corridor_for_test(&mut city, 50, 52, 49);
    water_footprint_from_below_for_test(&mut city, 50, 50, 3);
    stamp_complete_building_for_test(&mut city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut city, 30, 36, 211, 3);
    stamp_complete_building_for_test(&mut city, 30, 42, 209, 3);
    stamp_complete_building_for_test(&mut city, 36, 34, 214, 3);
    power_adjacent_building_for_test(&mut city, 30, 29);
    power_adjacent_building_for_test(&mut city, 30, 35);
    power_adjacent_building_for_test(&mut city, 30, 41);
    power_adjacent_building_for_test(&mut city, 36, 33);
    stamp_complete_building_for_test(&mut city, 46, 50, 13, 1);
    stamp_complete_building_for_test(&mut city, 34, 50, 233, 1);
    for index in [0, 1, 2, 3, 4] {
        set_budget_funding_for_test(&mut city, index, 0);
    }
    for index in TRANSPORT_BUDGET_INDICES {
        set_budget_funding_for_test(&mut city, index, 0);
    }
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let transit_eligible = vacant_rci_transit_ring_count_for_test(&city, (50, 50), 3);
    let mut state = test_state(city);
    mark_transit_ring_served_for_test(&mut state, (50, 50), 3);

    state.develop_rci_until_built_for_test();
    let developed: Vec<_> = building_footprint_from_origin(50, 50, 3)
        .unwrap()
        .into_iter()
        .filter(|&(x, y)| RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(x, y)))
        .collect();
    assert!(
        !developed.is_empty() && developed.len() <= transit_eligible,
        "per-tile growth rolls should select a nonempty subset of transit-eligible cells"
    );
    let building = state.city.building_at(developed[0].0, developed[0].1);
    assert!(
        !RESIDENTIAL_1X1_BUILDINGS[8..].contains(&building),
        "unfunded local services should not lift residential growth into high band, got {building:#04x}"
    );
}

#[test]
fn occupied_finished_rci_is_not_quality_swapped() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 90);
    let existing = RESIDENTIAL_3X3_BUILDINGS[0];
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, existing);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    stamp_complete_building_for_test(&mut city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut city, 30, 36, 211, 3);
    stamp_complete_building_for_test(&mut city, 30, 42, 209, 3);
    stamp_complete_building_for_test(&mut city, 36, 34, 214, 3);
    stamp_complete_building_for_test(&mut city, 24, 20, 13, 1);
    stamp_complete_building_for_test(&mut city, 34, 20, 233, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 0);
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        assert_eq!(state.city.building_at(x, y), existing);
        assert_eq!(
            state.city.zone_at(x, y).map(|zone| zone.zone_id()),
            Some(ZONE_RESIDENTIAL_DENSE)
        );
    }
}

#[test]
fn positive_demand_redevelops_abandoned_rci_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE);
        city.set_building(x, y, 0xAA);
        city.set_tile_bits(x, y, TileBits::new(XBIT_CHOOSE_AND_PLACE));
    }
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    state.sim_calendar_day1_power();
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        state.trip_served_tiles[y * CITY_WIDTH + x] = true;
    }

    assert_eq!(state.develop_rci_until_built_for_test(), 4);

    let building = state.city.building_at(20, 20);
    assert!(
        COMMERCIAL_2X2_BUILDINGS.contains(&building),
        "expected abandoned commercial footprint to redevelop, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), building);
        assert_eq!(
            state.city.zone_at(x, y).map(|zone| zone.zone_id()),
            Some(ZONE_COMMERCIAL_DENSE)
        );
    }
}

#[test]
fn dense_rci_expansion_replaces_mixed_abandoned_and_small_rci_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    city.set_building(21, 21, 0x8A);
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 4);

    let building = state.city.building_at(20, 20);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&building),
        "expected mixed abandoned/residential footprint to size-up one pop step, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), building);
        assert_eq!(
            state.city.zone_at(x, y).map(|zone| zone.zone_id()),
            Some(ZONE_RESIDENTIAL_DENSE)
        );
    }
}

#[test]
fn dense_rci_expansion_does_not_startbuilding_from_abandoned_origin() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    city.set_building(20, 20, ABANDONED_POP1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    assert!(
        state
            .rci_expansion_redevelopment_candidate_at(
                20,
                20,
                &reserved,
                &power_maps,
                city_service_outcomes_for_city(&state.city),
                &rci_development_services_for_city(&state.city),
                None,
            )
            .is_none(),
        "PE CONSIDERABANDON does not fall through to StartBuilding size-up"
    );
}

#[test]
fn dense_rci_expansion_can_start_on_abandoned_origin_with_live_rci_inside_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    // Live 1×1 origin; abandoned neighbor inside the pop2 2×2 footprint.
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_building(21, 20, 0x8A);
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    // 2×2 size-up (4) plus vacant 1×1s on the remaining PE transit-ring tiles.
    assert_eq!(state.develop_rci_until_built_for_test(), 7);

    let building = state.city.building_at(20, 20);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&building),
        "expected size-up to absorb abandoned neighbor, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), building);
        assert_eq!(
            state.city.zone_at(x, y).map(|zone| zone.zone_id()),
            Some(ZONE_RESIDENTIAL_DENSE)
        );
    }
}

#[test]
fn positive_demand_does_not_redevelop_equal_quality_rci_building() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 90);
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, RESIDENTIAL_3X3_BUILDINGS[3]);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 0);
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        assert_eq!(state.city.building_at(x, y), RESIDENTIAL_3X3_BUILDINGS[3]);
    }
}

#[test]
fn land_value_start_gates_skip_industrial_zones() {
    // This pins CityGame's absent-plane zero fallback; PE missing-XVAL memory
    // initialization is unresolved and is not asserted by this test.
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(
        city.land_value_at(20, 20).is_none(),
        "blank city has no XVAL map"
    );
    assert!(land_value_allows_start_from_pop(
        &city,
        20,
        20,
        ZONE_INDUSTRIAL_DENSE,
        1
    ));
    assert!(!land_value_allows_start_from_pop(
        &city,
        20,
        20,
        ZONE_RESIDENTIAL_DENSE,
        1
    ));
    assert!(!land_value_allows_start_from_pop(
        &city,
        20,
        20,
        ZONE_COMMERCIAL_DENSE,
        2
    ));

    let mut city = city;
    city.sync_land_value_from_slice(&vec![0; CITY_TILES]);
    assert!(land_value_allows_start_from_pop(
        &city,
        20,
        20,
        ZONE_INDUSTRIAL_LIGHT,
        3
    ));
    assert!(!land_value_allows_start_from_pop(
        &city,
        20,
        20,
        ZONE_RESIDENTIAL_DENSE,
        1
    ));
    city.sync_land_value_from_slice(&vec![XVAL_POPLEVEL_LOW; CITY_TILES]);
    assert!(land_value_allows_start_from_pop(
        &city,
        20,
        20,
        ZONE_RESIDENTIAL_DENSE,
        1
    ));
}

#[test]
fn dense_rci_positive_demand_expands_small_buildings_to_larger_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 4);

    let building = state.city.building_at(20, 20);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&building),
        "expected residential pop-ladder size-up to 2x2, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), building);
        assert_eq!(
            state.city.zone_at(x, y).map(|zone| zone.zone_id()),
            Some(ZONE_RESIDENTIAL_DENSE)
        );
    }
}

#[test]
fn dense_rci_sizeup_preserves_watered_and_xtrf_like_pe_choose_and_place() {
    // PE size-up StartBuilding → ChooseAndPlace: OR XBIT 0xe0 only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-sizeup-xtrf.sc2", "SizeUpX").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
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
        state.city.set_traffic_density(x, y, 0x48);
    }

    assert_eq!(state.develop_rci_until_built_for_test(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&building),
        "expected residential pop-ladder size-up to 2x2, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped(), "piped cleared at ({x},{y})");
        assert!(bits.watered(), "watered cleared at ({x},{y})");
        assert_eq!(
            state.city.traffic_density_at(x, y),
            Some(0x48),
            "XTRF mutated at ({x},{y})"
        );
    }
}

#[test]
fn dense_rci_sizeup_preserves_xtxt_like_pe_place_tile() {
    // PE size-up StartBuilding → ChooseAndPlace → PlaceTile: leave XTXT (no DestroyBuilding on <0x8C).
    let mut city = City::new_blank("/tmp/sc2000-sizeup-xtxt.sc2", "SizeUpT").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
        city.set_text_overlay(x, y, 0x2a);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&building),
        "expected residential pop-ladder size-up to 2x2, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_dense_rci_sizeup_preserves_xtxt_like_pe_place_tile() {
    // Shared size-up PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-sizeup-xtxt.sc2", "MacSizeT").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
        city.set_text_overlay(x, y, 0x2a);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&building),
        "expected residential pop-ladder size-up to 2x2, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x2a),
            "XTXT cleared at ({x},{y})"
        );
    }
}

#[test]
fn mac68k_dense_rci_sizeup_preserves_xtrf_like_pe_choose_and_place() {
    // Mac68k shares ChooseAndPlace / PlaceTile: OR XBIT 0xe0 only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-mac-sizeup-xtrf.sc2", "MacSizeX").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_simulation_service_bits();
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
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
        state.city.set_traffic_density(x, y, 0x48);
    }

    assert_eq!(state.develop_rci_until_built_for_test(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&building),
        "expected residential pop-ladder size-up to 2x2, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(
            state.city.traffic_density_at(x, y),
            Some(0x48),
            "XTRF mutated at ({x},{y})"
        );
    }
}

#[test]
fn dense_rci_expansion_sizes_up_high_quality_one_by_ones() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let existing = *RESIDENTIAL_1X1_BUILDINGS.last().unwrap();
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, existing);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&building),
        "PE StartBuilding sizes up on XVAL pop gates, not occupied class, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), building);
        assert_eq!(
            state.city.zone_at(x, y).map(|zone| zone.zone_id()),
            Some(ZONE_RESIDENTIAL_DENSE)
        );
    }
}

#[test]
fn dense_rci_pop1_size_up_refuses_choose_and_place_west_edge() {
    let mut state = pop1_size_up_state_at_for_test((1, 20));
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    assert!(
        state
            .rci_expansion_redevelopment_candidate_at(
                1,
                20,
                &reserved,
                &power_maps,
                city_service_outcomes_for_city(&state.city),
                &rci_development_services_for_city(&state.city),
                None,
            )
            .is_none(),
        "PE ChooseAndPlace pop2 requires call x>=2"
    );
    assert_eq!(state.develop_rci_until_built_for_test(), 0);
    assert_eq!(
        state.city.building_at(1, 20),
        *RESIDENTIAL_1X1_BUILDINGS.last().unwrap()
    );
}

#[test]
fn dense_rci_pop1_size_up_allows_choose_and_place_at_min_call_coords() {
    let mut state = pop1_size_up_state_at_for_test((2, 20));
    assert_eq!(state.develop_rci_until_built_for_test(), 4);
    let building = state.city.building_at(2, 20);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&building),
        "call (2,21) is the first in-bounds pop2 stamp, got {building:#04x}"
    );
}

#[test]
fn dense_rci_expansion_uses_two_by_two_when_three_by_three_is_unavailable() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE);
        city.set_building(x, y, COMMERCIAL_1X1_BUILDINGS[0]);
    }
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 4);

    let building = state.city.building_at(20, 20);
    assert!(
        COMMERCIAL_2X2_BUILDINGS.contains(&building),
        "expected commercial 2x2 expansion, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), building);
        assert_eq!(
            state.city.zone_at(x, y).map(|zone| zone.zone_id()),
            Some(ZONE_COMMERCIAL_DENSE)
        );
    }
}

#[test]
fn dense_rci_expansion_does_not_size_up_over_straddling_2x2() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_complete_building_for_test(&mut city, 21, 20, RESIDENTIAL_2X2_BUILDINGS[0], 2);
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    assert!(
        state
            .rci_expansion_redevelopment_candidate_at(
                20,
                20,
                &reserved,
                &power_maps,
                city_service_outcomes_for_city(&state.city),
                &rci_development_services_for_city(&state.city),
                None,
            )
            .is_none(),
        "PE CheckTileArea rejects XBLD >= 0x8C, including a straddling 2×2"
    );
}

#[test]
fn dense_rci_expansion_does_not_size_up_over_xbld_at_least_0x8c() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let residential = *RESIDENTIAL_1X1_BUILDINGS.last().unwrap();
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, residential);
    }
    city.set_building(21, 21, AIRPORT_DEVELOPMENT_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 0);
    assert_eq!(
        state.city.building_at(21, 21),
        AIRPORT_DEVELOPMENT_BUILDINGS[0],
        "PE CheckTileArea rejects XBLD >= 0x8C, so size-up must not overwrite airport"
    );
    assert_eq!(state.city.building_at(20, 20), residential);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        if (x, y) == (21, 21) {
            continue;
        }
        assert_eq!(state.city.building_at(x, y), residential);
    }
}

#[test]
fn dense_rci_expansion_does_not_size_up_over_road_in_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    city.set_building(21, 21, TILE_ROAD_LR);
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    assert!(
        state
            .rci_expansion_redevelopment_candidate_at(
                20,
                20,
                &reserved,
                &power_maps,
                city_service_outcomes_for_city(&state.city),
                &rci_development_services_for_city(&state.city),
                None,
            )
            .is_none(),
        "PE CheckTileArea rejects road tiles inside the pop1 2×2"
    );
}

#[test]
fn dense_rci_pop1_size_up_uses_sw_quadrant_when_se_is_unzoned() {
    let candidate = pop1_quadrant_size_up_candidate_for_test((21, 20), (21, 21))
        .expect("PE searches north when the SE 2×2 is unzoned");
    assert_eq!(candidate.plan.origin, (21, 20));
    assert_eq!(
        candidate.plan.footprint,
        building_footprint_from_origin(21, 20, 2).unwrap()
    );
}

#[test]
fn dense_rci_pop1_size_up_uses_ne_quadrant_when_earlier_are_unzoned() {
    let candidate = pop1_quadrant_size_up_candidate_for_test((20, 21), (21, 21))
        .expect("PE searches west when SE and NE 2×2s are unzoned");
    assert_eq!(candidate.plan.origin, (20, 21));
    assert_eq!(
        candidate.plan.footprint,
        building_footprint_from_origin(20, 21, 2).unwrap()
    );
}

#[test]
fn dense_rci_pop1_size_up_uses_nw_quadrant_when_earlier_are_unzoned() {
    let candidate = pop1_quadrant_size_up_candidate_for_test((20, 20), (21, 21))
        .expect("PE searches northwest last");
    assert_eq!(candidate.plan.origin, (20, 20));
    assert_eq!(
        candidate.plan.footprint,
        building_footprint_from_origin(20, 20, 2).unwrap()
    );
}

#[test]
fn dense_rci_pop1_size_up_prefers_se_quadrant_when_several_fit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    city.set_building(21, 21, RESIDENTIAL_1X1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 20, 22);
    power_adjacent_building_for_test(&mut city, 21, 20);
    water_footprint_from_below_for_test(&mut city, 21, 21, 2);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    let candidate = state
        .rci_expansion_redevelopment_candidate_at(
            21,
            21,
            &reserved,
            &power_maps,
            city_service_outcomes_for_city(&state.city),
            &rci_development_services_for_city(&state.city),
            None,
        )
        .expect("SE 2×2 is first in PE order");
    assert_eq!(candidate.plan.origin, (21, 21));
}

#[test]
fn dense_rci_pop1_returns_first_checkarea_quadrant_with_water_xter() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    city.set_building(21, 21, RESIDENTIAL_1X1_BUILDINGS[0]);
    // CheckTileArea ignores XTER; this water-class cell only belongs to the
    // first (21,21) quadrant and must not make StartBuilding try a later one.
    city.set_terrain(22, 22, 0x10);
    stamp_transport_access_for_test(&mut city, 20, 22);
    power_adjacent_building_for_test(&mut city, 21, 20);
    water_footprint_from_below_for_test(&mut city, 21, 21, 2);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    let candidate = state
        .rci_expansion_redevelopment_candidate_at(
            21,
            21,
            &reserved,
            &power_maps,
            city_service_outcomes_for_city(&state.city),
            &rci_development_services_for_city(&state.city),
            None,
        )
        .expect("PE places on the first quadrant after CheckTileArea passes");

    assert_eq!(candidate.plan.origin, (21, 21));
    assert!(is_water_terrain(state.city.terrain_at(22, 22)));
    assert!(check_tile_area(
        &state.city,
        22,
        22,
        state.city.altitude_at(21, 21),
        ZONE_RESIDENTIAL_DENSE,
        CHECK_TILE_AREA_POP1_MAX_XBLD,
    ));
    assert!(CONSTRUCTION_POP2_BUILDINGS.contains(&candidate.plan.building));
    assert!(
        !candidate.plan.place_as_construction,
        "the mode-3 selector already ran before the plan was returned"
    );

    let selected_building = candidate.plan.building;
    let mut stamp_crt = Win95CrtRand::new(0x0043_75C0);
    let seed_before_stamp = stamp_crt.seed();
    state.stamp_zone_development_with_crt(&candidate.plan, &mut stamp_crt);
    assert_eq!(stamp_crt.seed(), seed_before_stamp, "do not pick twice");
    assert_eq!(state.city.building_at(21, 21), selected_building);
}

#[test]
fn dense_rci_pop3_size_up_grows_into_empty_three_by_three() {
    let mut state = pop3_size_up_state_for_test(None);
    let candidate = rci_expansion_from_2x2_view_corner_for_test(&mut state, (20, 20))
        .expect("PE CheckTileArea max 0xAE allows empty tiles in the 3×3");
    assert_eq!(candidate.plan.origin, (20, 20));
    assert_eq!(
        candidate.plan.footprint,
        building_footprint_from_origin(20, 20, 3).unwrap()
    );
    assert!(CONSTRUCTION_POP4_BUILDINGS.contains(&candidate.plan.building));
    assert!(candidate.plan.place_as_construction);

    let selected_building = candidate.plan.building;
    let mut stamp_crt = Win95CrtRand::new(0x0043_75C0);
    let seed_before_stamp = stamp_crt.seed();
    state.stamp_zone_development_with_crt(&candidate.plan, &mut stamp_crt);
    assert_eq!(stamp_crt.seed(), seed_before_stamp, "do not pick twice");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        assert_eq!(state.city.building_at(x, y), selected_building);
    }
}

#[test]
fn dense_rci_pop3_size_up_power_cross_stays_at_visited_view_corner() {
    let mut state = pop3_size_up_state_for_test(None);
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&state.city, (20, 20), 2);
    assert!(sx > 1, "PE west power probe uses a strict x > 1 bound");
    let reserved = vec![false; CITY_TILES];
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let outcomes = city_service_outcomes_for_city(&state.city);
    let services = rci_development_services_for_city(&state.city);

    assert!(
        state
            .rci_expansion_redevelopment_candidate_at(
                sx,
                sy,
                &reserved,
                &power_maps,
                outcomes,
                &services,
                None,
            )
            .is_none(),
        "pop3 StartBuilding requires FUN_00436D00 power at the visited view-corner tile"
    );

    let west_neighbor = (sx - 1, sy);
    set_saved_xbit_growth_power_for_test(&mut power_maps, west_neighbor.0, west_neighbor.1, true);
    let candidate = state
        .rci_expansion_redevelopment_candidate_at(
            sx,
            sy,
            &reserved,
            &power_maps,
            outcomes,
            &services,
            None,
        )
        .expect("cardinal power at the visited view-corner satisfies FUN_00436D00");
    assert!(
        !candidate.plan.footprint.contains(&west_neighbor),
        "power is read at the visited tile's cross, not reassigned to the 3×3 target footprint"
    );
}

#[test]
fn dense_rci_pop3_size_up_does_not_overwrite_xbld_at_least_0xae() {
    let mut state = pop3_size_up_state_for_test(Some(((22, 22), AIRPORT_DEVELOPMENT_BUILDINGS[0])));
    assert!(
        rci_expansion_from_2x2_view_corner_for_test(&mut state, (20, 20)).is_none(),
        "PE CheckTileArea rejects XBLD >= 0xAE in the pop3 3×3"
    );
}

#[test]
fn dense_rci_pop3_size_up_starts_from_view_rotation_corner_not_northwest() {
    let mut state = pop3_size_up_state_for_test(None);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&state.city, (20, 20), 2);
    assert_ne!(
        (sx, sy),
        (20, 20),
        "rotation 0 selects TRIGHT, the 2×2 SW tile"
    );
    assert!(
        state
            .rci_expansion_redevelopment_candidate_at(
                20,
                20,
                &reserved,
                &power_maps,
                city_service_outcomes_for_city(&state.city),
                &rci_development_services_for_city(&state.city),
                None,
            )
            .is_none(),
        "NW is not the wPositionAngle[0] corner"
    );
    assert!(
        rci_expansion_from_2x2_view_corner_for_test(&mut state, (20, 20)).is_some(),
        "the view-rotation corner of the 2×2 is the StartBuilding origin"
    );
}

#[test]
fn dense_rci_pop3_size_up_selects_first_road_valid_origin_from_view_corner() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 18..=28 {
        for x in 18..=28 {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
    }
    stamp_complete_building_for_test(&mut city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[4], 2);
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    city.set_view_rotation(2);
    assert_eq!(
        rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2),
        (21, 20),
        "rotation 2 should make the 2×2 NE tile the StartBuilding corner"
    );
    // The first raw-call candidate is (21,18), but only the later (21,19)
    // candidate has a matching diagonal probe at (20,22).
    city.set_building(20, 22, 0x2B);
    city.set_building(22, 22, AIRPORT_DEVELOPMENT_BUILDINGS[0]);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    let reserved = vec![false; CITY_TILES];
    let power_maps = PowerServiceMaps {
        conductive_tiles: vec![true; CITY_TILES],
        powerable_tiles: vec![true; CITY_TILES],
        powered_tiles: vec![true; CITY_TILES],
        saved_xbit_powered_tiles: vec![true; CITY_TILES],
        energized_tiles: vec![true; CITY_TILES],
        ..Default::default()
    };
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&state.city, (20, 20), 2);
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
        .expect("PE should start pop3 growth from the 2×2 view-corner");
    assert_eq!((sx, sy), (21, 20));
    assert_eq!(
        pop3_startbuilding_quadrant_origins(sx, sy).next(),
        Some((21, 18))
    );
    assert!(!pop3_startbuilding_has_road_orientation(
        &state.city,
        21,
        18
    ));
    assert!(pop3_startbuilding_has_road_orientation(&state.city, 21, 19));
    assert_eq!(candidate.plan.origin, (21, 19));
}

#[test]
fn dense_rci_pop3_size_up_starts_from_view_angle_corner_sw_when_old_path_only() {
    let mut state = pop3_size_up_state_for_test(None);
    let mut city = state.city;
    city.set_view_rotation(2);
    assert_eq!(
        rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2),
        (21, 20),
        "rotation 2 should make the 2×2 NE tile the StartBuilding corner"
    );
    for y in 19..=23 {
        for x in 19..=23 {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
    }
    // Old logic probes from the visited corner and can still succeed here.
    city.set_building(
        21,
        19,
        network_building_for_mask(NetworkKind::Road, NETWORK_TOP | NETWORK_RIGHT),
    );
    city.set_building(
        24,
        22,
        network_building_for_mask(
            NetworkKind::Road,
            NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT,
        ),
    );
    // Old logic can probe from the visited corner; block all source-derived
    // 3×3 candidates while leaving that visited-origin path available.
    city.set_building(21, 22, AIRPORT_DEVELOPMENT_BUILDINGS[0]);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let reserved = vec![false; CITY_TILES];
    let power_maps = PowerServiceMaps {
        conductive_tiles: vec![true; CITY_TILES],
        powerable_tiles: vec![true; CITY_TILES],
        powered_tiles: vec![true; CITY_TILES],
        saved_xbit_powered_tiles: vec![true; CITY_TILES],
        energized_tiles: vec![true; CITY_TILES],
        ..Default::default()
    };
    assert!(
        state
            .rci_expansion_redevelopment_candidate_at(
                21,
                20,
                &reserved,
                &power_maps,
                city_service_outcomes_for_city(&state.city),
                &rci_development_services_for_city(&state.city),
                None,
            )
            .is_none(),
        "all source-derived 3×3 start candidates should fail, so no growth should be returned"
    );
}

#[test]
fn dense_rci_pop3_size_up_starts_from_view_angle_corner_all_rotations() {
    let mut base_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 16..=27 {
        for x in 16..=27 {
            base_city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
    }
    stamp_complete_building_for_test(&mut base_city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[4], 2);
    or_rci_building_zone_corners_for_test(&mut base_city, (20, 20), 2);
    set_rci_demand_for_test(&mut base_city, 1_000, 0, 0);
    stamp_xval_for_test(&mut base_city, XVAL_POPLEVEL_HIGH);
    for rotation in 0..4u8 {
        let mut city = base_city.clone();
        city.set_view_rotation(rotation);
        // Zone corners are view-relative; stamp the active rotation's
        // corner after changing the view instead of reusing rotation 0.
        or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
        let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2);
        let expected_origin = pop3_startbuilding_quadrant_origins(sx, sy)
            .next()
            .expect("each rotated raw call has a first candidate");
        stamp_pop3_first_candidate_road_for_test(&mut city, expected_origin);
        let mut state = test_state(city);
        state.refresh_simulation_service_bits();
        state.bypass_growth_trip_gate = true;

        let reserved = vec![false; CITY_TILES];
        let power_maps = PowerServiceMaps {
            conductive_tiles: vec![true; CITY_TILES],
            powerable_tiles: vec![true; CITY_TILES],
            powered_tiles: vec![true; CITY_TILES],
            saved_xbit_powered_tiles: vec![true; CITY_TILES],
            energized_tiles: vec![true; CITY_TILES],
            ..Default::default()
        };
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
            .expect("PE should select the first zone/road-valid origin for this raw call");
        assert_eq!(
            candidate.plan.origin, expected_origin,
            "rotation {rotation} should preserve the PE raw-call candidate order"
        );
        assert_eq!(
            candidate.plan.footprint,
            building_footprint_from_origin(expected_origin.0, expected_origin.1, 3).unwrap()
        );
    }
}

#[test]
fn dense_rci_pop3_size_up_requires_oriented_road_at_diagonal() {
    let mut state = pop3_size_up_state_for_test(None);
    let mut city = state.city;
    city.set_building(19, 19, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    assert!(
        rci_expansion_from_2x2_view_corner_for_test(&mut state, (20, 20)).is_none(),
        "cardinal LR road is not a PE pop3 orientation probe"
    );
}

#[test]
fn dense_rci_pop3_road_miss_does_not_destroy_perimeter_building() {
    let mut state = pop3_size_up_state_for_test(None);
    let mut city = state.city;
    city.set_building(19, 19, 0); // remove the accepted NW diagonal road
    city.set_building(22, 20, 0x90); // DestroyBuilding perimeter; still below pop3 CheckTileArea max
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert!(check_tile_area_xbld_allowed(
        0x90,
        CHECK_TILE_AREA_POP3_MAX_XBLD
    ));
    assert!(
        rci_expansion_from_2x2_view_corner_for_test(&mut state, (20, 20)).is_none(),
        "PE road-orientation refusal occurs before the pop3 perimeter DestroyBuilding pass"
    );
    assert_eq!(
        state.city.building_at(22, 20),
        0x90,
        "a rejected candidate must not clear its perimeter tile"
    );
}

#[test]
fn dense_rci_pop3_size_up_rejects_wrong_road_orientation_on_nw_probe() {
    let mut state = pop3_size_up_state_for_test(None);
    let mut city = state.city;
    city.set_building(19, 19, 0x23);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    assert!(
        rci_expansion_from_2x2_view_corner_for_test(&mut state, (20, 20)).is_none(),
        "0x23 feeds the SW probe, not the NW probe"
    );
}

#[test]
fn dense_rci_pop3_size_up_accepts_sw_curve_on_sw_probe() {
    let mut state = pop3_size_up_state_for_test(None);
    let mut city = state.city;
    city.set_building(19, 19, 0);
    city.set_building(19, 23, 0x23);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let candidate = rci_expansion_from_2x2_view_corner_for_test(&mut state, (20, 20))
        .expect("0x23 at (di-1, si+1) is the PE SW orientation");
    assert_eq!(candidate.plan.origin, (20, 20));
}

#[test]
fn dense_rci_pop3_size_up_destroys_straddling_xbld_at_least_0x8c() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    city.set_zone(23, 20, ZONE_RESIDENTIAL_DENSE);
    city.set_zone(23, 21, ZONE_RESIDENTIAL_DENSE);
    stamp_complete_building_for_test(&mut city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[4], 2);
    stamp_complete_building_for_test(&mut city, 22, 20, RESIDENTIAL_2X2_BUILDINGS[0], 2);
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    // Occupied empty 3×3 tiles so vacant 1×1 growth cannot reserve them first.
    for &(x, y) in &[(20, 22), (21, 22), (22, 22)] {
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    stamp_pop3_road_orientation_for_test(&mut city, 20, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert!(state.develop_rci_zones() > 0);
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        assert!(
            is_construction_building(state.city.building_at(x, y)),
            "expected 3x3 construction at ({x},{y}), got {:#04x}",
            state.city.building_at(x, y)
        );
    }
    assert!(
        state.city.building_at(23, 20) < CHECK_TILE_AREA_POP1_MAX_XBLD,
        "PE FUN_00437f90 abandon on XBLD >= 0x8C must not leave the straddling 2x2 outside the 3x3"
    );
    assert!(state.city.building_at(23, 21) < CHECK_TILE_AREA_POP1_MAX_XBLD);
    assert_eq!(
        state.city.zone_at(23, 20).unwrap().zone_id(),
        ZONE_RESIDENTIAL_DENSE
    );
}

#[test]
fn dense_rci_pop3_size_up_abandon_preserves_xtxt_on_straddling_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-pop3-xtxt.sc2", "Pop3Xtxt").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    city.set_zone(23, 20, ZONE_RESIDENTIAL_DENSE);
    city.set_zone(23, 21, ZONE_RESIDENTIAL_DENSE);
    stamp_complete_building_for_test(&mut city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[4], 2);
    stamp_complete_building_for_test(&mut city, 22, 20, RESIDENTIAL_2X2_BUILDINGS[0], 2);
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    for &(x, y) in &[(20, 22), (21, 22), (22, 22)] {
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    for (x, y) in building_footprint_from_origin(22, 20, 2).unwrap() {
        city.set_text_overlay(x, y, 0x55);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    stamp_pop3_road_orientation_for_test(&mut city, 20, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert!(state.develop_rci_zones() > 0);
    for (x, y) in building_footprint_from_origin(22, 20, 2).unwrap() {
        assert_eq!(
            state.city.text_overlay_at(x, y),
            Some(0x55),
            "FUN_00437f90 abandon uses PlaceTile and does not clear XTXT at ({x},{y})"
        );
    }
}

#[test]
fn dense_rci_pop3_abandons_perimeter_before_pop4_picker_on_shared_crt() {
    let mut state = pop3_size_up_state_for_test(None);
    // This low-0x8c structure passes CheckTileArea (< 0xae) but triggers
    // the perimeter FUN_00437f90 abandon sequence.
    state.city.set_building(22, 20, 0x8c);
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&state.city, (20, 20), 2);
    let reserved = vec![false; CITY_TILES];
    let power_maps = PowerServiceMaps {
        conductive_tiles: vec![true; CITY_TILES],
        powerable_tiles: vec![true; CITY_TILES],
        powered_tiles: vec![true; CITY_TILES],
        saved_xbit_powered_tiles: vec![true; CITY_TILES],
        energized_tiles: vec![true; CITY_TILES],
        ..Default::default()
    };
    let outcomes = city_service_outcomes_for_city(&state.city);
    let services = rci_development_services_for_city(&state.city);
    let probe = state
        .rci_expansion_redevelopment_candidate_at(
            sx,
            sy,
            &reserved,
            &power_maps,
            outcomes,
            &services,
            None,
        )
        .expect("fixture has an accepted pop3 candidate");
    assert_eq!(probe.plan.origin, (20, 20));
    let call = pop3_startbuilding_candidate_call_from_origin(20, 20).unwrap();

    let demand = rci_demand_for_kind(&state.city, RciKind::Residential);
    let threshold = rci_growth_start_threshold(demand, 3);
    let seed = (1..10_000)
        .find(|&seed| {
            let mut rng = Win95CrtRand::new(seed);
            pe_rci_startbuilding_growth_roll_succeeds_optional(
                threshold,
                sx,
                sy,
                GROWTH_ROLL_START_EXISTING_KEY,
                Some(&mut rng),
            )
        })
        .expect("find a seed that passes the growth roll");

    // Independent expected sequence: growth roll → perimeter abandon draws →
    // mode-3/pop-4 picker. The candidate path must leave the same CRT state.
    let mut expected = test_state(state.city.clone());
    let mut expected_crt = Win95CrtRand::new(seed);
    assert!(pe_rci_startbuilding_growth_roll_succeeds_optional(
        threshold,
        sx,
        sy,
        GROWTH_ROLL_START_EXISTING_KEY,
        Some(&mut expected_crt),
    ));
    expected.destroy_pop3_size_up_xbld_at_least_0x8c_with_crt((20, 20), &mut expected_crt);
    let expected_building =
        construction_building_for_pop_optional(4, call.0, call.1, Some(&mut expected_crt))
            .expect("pop4 construction picker returns a tile");

    let mut actual_crt = Win95CrtRand::new(seed);
    let candidate = state
        .rci_expansion_redevelopment_candidate_at(
            sx,
            sy,
            &reserved,
            &power_maps,
            outcomes,
            &services,
            Some(&mut actual_crt),
        )
        .expect("live candidate includes the perimeter abandon pass");
    assert_eq!(candidate.plan.building, expected_building);
    assert_eq!(actual_crt.seed(), expected_crt.seed());
    for &(x, y) in &[(22, 20), (23, 20), (23, 19), (22, 19)] {
        assert_eq!(
            state.city.building_at(x, y),
            expected.city.building_at(x, y)
        );
    }
}

#[test]
fn dense_rci_pop3_size_up_uses_sw_quadrant_when_it_fits() {
    let candidate = pop3_quadrant_size_up_candidate_for_test((20, 21), (20, 21))
        .expect("PE tries 2×2 as SW of the 3×3 first");
    assert_eq!(candidate.plan.origin, (20, 21));
    assert_eq!(
        candidate.plan.footprint,
        building_footprint_from_origin(20, 21, 3).unwrap()
    );
}

#[test]
fn dense_rci_pop3_size_up_uses_se_quadrant_when_earlier_are_unzoned() {
    let candidate = pop3_quadrant_size_up_candidate_for_test((20, 21), (21, 21))
        .expect("PE searches SE when SW 3×3 is unzoned");
    assert_eq!(candidate.plan.origin, (20, 21));
    assert_eq!(
        candidate.plan.footprint,
        building_footprint_from_origin(20, 21, 3).unwrap()
    );
}

#[test]
fn dense_rci_pop3_size_up_uses_ne_quadrant_when_earlier_are_unzoned() {
    let candidate =
        pop3_quadrant_size_up_candidate_for_test((20, 20), (21, 20)).expect("PE searches NE last");
    assert_eq!(candidate.plan.origin, (20, 20));
    assert_eq!(
        candidate.plan.footprint,
        building_footprint_from_origin(20, 20, 3).unwrap()
    );
}

#[test]
fn dense_rci_pop3_size_up_rot3_uses_raw_call_candidate_order() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 16..=27 {
        for x in 16..=27 {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
    }
    stamp_complete_building_for_test(&mut city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[4], 2);
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    city.set_view_rotation(3);
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2);
    assert_eq!((sx, sy), (21, 21));
    let expected_origin = pop3_startbuilding_quadrant_origins(sx, sy)
        .next()
        .expect("rotation-3 raw call has a first candidate");
    stamp_pop3_first_candidate_road_for_test(&mut city, expected_origin);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    state.bypass_growth_trip_gate = true;

    let reserved = vec![false; CITY_TILES];
    let power_maps = PowerServiceMaps {
        conductive_tiles: vec![true; CITY_TILES],
        powerable_tiles: vec![true; CITY_TILES],
        powered_tiles: vec![true; CITY_TILES],
        saved_xbit_powered_tiles: vec![true; CITY_TILES],
        energized_tiles: vec![true; CITY_TILES],
        ..Default::default()
    };
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
        .expect("PE should try the first raw-call candidate with a valid road");
    assert_eq!(candidate.plan.origin, expected_origin);
}

#[test]
fn dense_rci_pop3_size_up_rotation1_uses_raw_call_candidate_order() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 16..=27 {
        for x in 16..=27 {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
    }
    stamp_complete_building_for_test(&mut city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[4], 2);
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    city.set_view_rotation(1);
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2);
    assert_eq!((sx, sy), (20, 20));
    let expected_origin = pop3_startbuilding_quadrant_origins(sx, sy)
        .next()
        .expect("rotation-1 raw call has a first candidate");
    stamp_pop3_first_candidate_road_for_test(&mut city, expected_origin);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    state.bypass_growth_trip_gate = true;
    let reserved = vec![false; CITY_TILES];
    let power_maps = PowerServiceMaps {
        conductive_tiles: vec![true; CITY_TILES],
        powerable_tiles: vec![true; CITY_TILES],
        powered_tiles: vec![true; CITY_TILES],
        saved_xbit_powered_tiles: vec![true; CITY_TILES],
        energized_tiles: vec![true; CITY_TILES],
        ..Default::default()
    };
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
        .expect("the first rotation-1 raw-call origin is zone and road valid");
    assert_eq!(candidate.plan.origin, expected_origin);
}

#[test]
fn dense_rci_pop3_size_up_prefers_sw_quadrant_when_several_fit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 21, 4).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_complete_building_for_test(&mut city, 21, 21, RESIDENTIAL_2X2_BUILDINGS[4], 2);
    or_rci_building_zone_corners_for_test(&mut city, (21, 21), 2);
    stamp_transport_access_for_test(&mut city, 20, 23);
    for (x, y) in [(20, 24), (20, 20), (24, 20), (24, 24)] {
        city.set_building(x, y, 0x2B);
    }
    power_adjacent_building_for_test(&mut city, 21, 20);
    water_footprint_from_below_for_test(&mut city, 21, 21, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let candidate = rci_expansion_from_2x2_view_corner_for_test(&mut state, (21, 21))
        .expect("SW 3×3 is first in PE pop3 order");
    assert_eq!(candidate.plan.origin, (21, 21));
}

#[test]
fn dense_rci_pop3_size_up_checks_later_road_orientation_after_first_candidate_misses() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 18..=27 {
        for x in 18..=27 {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
    }
    stamp_complete_building_for_test(&mut city, 21, 21, RESIDENTIAL_2X2_BUILDINGS[4], 2);
    or_rci_building_zone_corners_for_test(&mut city, (21, 21), 2);
    stamp_transport_access_for_test(&mut city, 19, 22);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);

    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (21, 21), 2);
    let candidate_origins = pop3_startbuilding_quadrant_origins(sx, sy)
        .take(2)
        .collect::<Vec<_>>();
    let (first_origin, second_origin) = (candidate_origins[0], candidate_origins[1]);
    let (road_x, road_y) = (second_origin.0 - 1, second_origin.1 - 1);
    city.set_building(road_x, road_y, 0x2B);

    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let power_maps = PowerServiceMaps {
        conductive_tiles: vec![true; CITY_TILES],
        powerable_tiles: vec![true; CITY_TILES],
        powered_tiles: vec![true; CITY_TILES],
        saved_xbit_powered_tiles: vec![true; CITY_TILES],
        energized_tiles: vec![true; CITY_TILES],
        ..Default::default()
    };
    let reserved = vec![false; CITY_TILES];
    let first_footprint = building_footprint_from_origin(first_origin.0, first_origin.1, 3)
        .expect("first candidate footprint");
    assert!(first_footprint.iter().all(|&(x, y)| {
        check_tile_area(
            &state.city,
            x,
            y,
            state.city.altitude_at(sx, sy),
            ZONE_RESIDENTIAL_DENSE,
            CHECK_TILE_AREA_POP3_MAX_XBLD,
        )
    }));
    assert!(
        !pop3_startbuilding_has_road_orientation(&state.city, first_origin.0, first_origin.1),
        "first candidate passes CheckTileArea but has no accepted diagonal road"
    );
    assert!(pop3_startbuilding_has_road_orientation(
        &state.city,
        second_origin.0,
        second_origin.1
    ));

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
        .expect("PE tries the next candidate after the first road-orientation miss");
    assert_eq!(candidate.plan.origin, second_origin);
}

#[test]
fn rci_startbuilding_does_not_skip_later_scan_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 65);
    for (x, y) in [(20, 20), (52, 52)] {
        city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
        stamp_transport_access_for_test(&mut city, x, y - 1);
        power_adjacent_building_for_test(&mut city, x - 1, y);
        water_footprint_from_below_for_test(&mut city, x, y, 1);
    }
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_until_built_for_test(), 2);

    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "expected earlier scan-order residential tile to develop"
    );
    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(52, 52)),
        "expected later scan-order residential tile to develop"
    );
}

#[test]
fn rci_startbuilding_does_not_prefer_pollution_over_scan_order() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 65);
    for (x, y) in [(20, 20), (52, 52)] {
        city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
        stamp_transport_access_for_test(&mut city, x, y - 1);
        power_adjacent_building_for_test(&mut city, x - 1, y);
        water_footprint_from_below_for_test(&mut city, x, y, 1);
    }
    let layer_index = |x: usize, y: usize| x * CITY_HEIGHT + y;
    let mut crime = vec![0u8; CITY_TILES];
    let mut pollution = vec![0u8; CITY_TILES];
    crime[layer_index(20, 20)] = 240;
    pollution[layer_index(20, 20)] = 240;
    city.sync_crime_density_from_slice(&crime);
    city.sync_pollution_density_from_slice(&pollution);
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_until_built_for_test(), 2);

    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "polluted earlier tile should still develop"
    );
    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(52, 52)),
        "later cleaner tile should still develop"
    );
}

#[test]
fn rci_growth_does_not_skip_later_win95_phase() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in [(20, 20), (21, 21)] {
        city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
        stamp_transport_access_for_test(&mut city, x, y - 1);
        power_adjacent_building_for_test(&mut city, x - 1, y);
        water_footprint_from_below_for_test(&mut city, x, y, 1);
    }
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.develop_rci_until_built_for_test(), 2);
    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "iStep=0 iSubStep=0 should develop"
    );
    assert!(
        RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(21, 21)),
        "later lattice tiles should still develop without a monthly cap"
    );
}

#[test]
fn simulation_month_uses_live_density_maps_before_rci_growth() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 65);
    for (x, y) in [(20, 20), (52, 52)] {
        city.set_zone(x, y, ZONE_RESIDENTIAL_LIGHT);
        stamp_transport_access_for_test(&mut city, x, y - 1);
        power_adjacent_building_for_test(&mut city, x - 1, y);
        water_footprint_from_below_for_test(&mut city, x, y, 1);
    }
    // Anchor the city center away from the coal so center-bonus does not
    // outweigh the live XPLT penalty on (20,20). Trees are ignored by the
    // centroid pass, so use power-line tiles.
    for y in 60..68 {
        for x in 60..68 {
            city.set_building(x, y, 14);
        }
    }
    // Coal in an adjacent microsim cell to (20,20) for PE one-step blur.
    stamp_complete_building_for_test(&mut city, 16, 18, 207, 4);
    set_rci_demand_for_test(&mut city, 50, 0, 0);
    let mut state = test_state(city);

    let population = live_population_stats_for_city(&state.city);
    let outcomes = live_city_service_outcomes_for_city(&state.city);
    state.sim_calendar_day2_density(population, outcomes);
    assert_eq!(
        state.city.tile_bits_at(10, 10).unwrap().raw_bits() & 0x08,
        0x08,
        "the Win95 day-2 density phase should publish the projected crime mask"
    );
    set_rci_demand_for_test(&mut state.city, 50, 0, 0);
    state.reset_monthly_growth_budget();
    state.sim_calendar_days_3_18_growth();

    assert!(
        is_construction_building(state.city.building_at(20, 20)),
        "polluted earlier tile should still StartBuilding without a monthly cap"
    );
    assert!(
        is_construction_building(state.city.building_at(52, 52)),
        "expected monthly growth to still StartBuilding the distant candidate"
    );
    assert!(
        state.city.pollution_density_at(20, 20).unwrap()
            > state.city.pollution_density_at(52, 52).unwrap(),
        "coal plant should create a stronger live pollution penalty near (20,20)"
    );
}

#[test]
fn live_pollution_map_uses_xtrf_divided_by_five() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(
        40,
        40,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_traffic_density(40, 40, 100);
    let hot = live_pollution_density_map_for_city(&city)[density_map_index(40, 40)];

    city.set_traffic_density(40, 40, 0);
    let cold = live_pollution_density_map_for_city(&city)[density_map_index(40, 40)];

    assert!(
        hot > cold,
        "XTRF/5 should raise live pollution on congested road tiles ({hot} vs {cold})"
    );
    // Interior microsim cell: (2*(100/5)) / (blur_base + 4 zero neighbors).
    let blur_base = pollution_blur_divisor_base_for_city(&city);
    assert_eq!(hot, (40 / (blur_base + 4)) as u8);
    assert_eq!(cold, 0);
}

#[test]
fn live_pollution_map_carries_prior_xplt_and_tile5_term() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut prior = vec![0u8; CITY_TILES];
    prior[density_map_index(30, 30)] = 40;
    city.sync_pollution_density_from_slice(&prior);
    let carried = live_pollution_density_map_for_city(&city)[density_map_index(30, 30)];
    assert!(
        carried > 0,
        "prior XPLT should seed the next pollution scan"
    );

    city.sync_pollution_density_from_slice(&vec![0u8; CITY_TILES]);
    city.set_building(30, 30, 5);
    let tile5 = live_pollution_density_map_for_city(&city)[density_map_index(30, 30)];
    assert!(
        tile5 >= 40,
        "Win95 tile-5 pollution term (+0xC8 before blur) should leave a strong local signal ({tile5})"
    );
}

#[test]
fn mac_pollution_map_matches_native_inputs_weights_and_cardinal_blur() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-xplt-contract.sc2", "MacXPLT").expect("blank city");
    city.kind = CityFileKind::MacCity;
    // One native 64x64 cell samples the upper-left of its 2x2 tile footprint
    // for XTRF/XPLT and sums all four XBLD weights. Use both the mapped 0xcf
    // weight (50) and Mac's special tile-5 contribution (0xc8).
    city.set_traffic_density(40, 40, 104);
    city.set_building(40, 40, 0xcf);
    city.set_building(41, 40, 5);
    let mut prior = vec![0_u8; CITY_TILES];
    prior[density_map_index(40, 40)] = 20;
    city.sync_pollution_density_from_slice(&prior);

    // FUN_00027AB4 uses 4 - signed_word(MISC[1037]) + signed_word(MISC[1043]),
    // then adds one for each in-bounds cardinal neighbor. High halves are
    // deliberately nonzero to pin the persisted low-WORD interpretation.
    city.set_mac_state_misc_u32_at(1037 * 4, 0xabcd_0002);
    city.set_mac_state_misc_u32_at(1043 * 4, 0x1234_0001);

    let pollution = live_mac_pollution_density_map_for_city(&city);
    let center = pollution[density_map_index(40, 40)];
    let east = pollution[density_map_index(42, 40)];
    let south = pollution[density_map_index(40, 42)];
    let diagonal = pollution[density_map_index(42, 42)];

    // Source WORD = floor(104/5) + prior 20 + table weight 50 + tile-5 200
    // = 290. The center is floor(2*290/7)=82; each cardinal neighbor is
    // floor(290/7)=41; diagonal cells receive no direct blur contribution.
    assert_eq!(center, 82);
    assert_eq!(east, 41);
    assert_eq!(south, 41);
    assert_eq!(diagonal, 0);
}

#[test]
fn mac_pollution_controls_change_native_blur_divisor() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city =
            City::new_blank("/tmp/mac-pollution-controls.sc2", "Pollution").expect("blank city");
        city.kind = kind;
        city.set_traffic_density(40, 40, 104);
        city.set_building(40, 40, 0xcf);
        city.set_building(41, 40, 5);
        let mut prior = vec![0_u8; CITY_TILES];
        prior[density_map_index(40, 40)] = 20;
        city.sync_pollution_density_from_slice(&prior);
        city.set_mac_pollution_adjustment_for_test(2);
        city.set_mac_treatment_gate(1);

        let read = |city: &City| {
            let map = live_mac_pollution_density_map_for_city(city);
            (
                map[density_map_index(40, 40)],
                map[density_map_index(42, 40)],
            )
        };
        assert_eq!(read(&city), (82, 41));
        assert!(set_city_ordinance_enabled_with_fidelity(
            &mut city,
            ORDINANCE_POLLUTION_CONTROLS,
            true,
            EngineFidelity::Mac68k,
        ));
        assert_eq!(read(&city), (72, 36), "pollution controls in {kind:?}");
        assert!(set_city_ordinance_enabled_with_fidelity(
            &mut city,
            ORDINANCE_POLLUTION_CONTROLS,
            false,
            EngineFidelity::Mac68k,
        ));
        assert_eq!(read(&city), (82, 41));
    }
}

#[test]
fn mac_pollution_weights_match_reconstructed_native_table_for_all_xbld_ids() {
    // The relocated Mac table at A5-0x4606 matches the PE pollution lookup
    // for every ID 0x70..=0xff. These sparse nonzero entries are copied from
    // the reconstructed DATA/ZERO image; all omitted table entries are zero.
    let native_table_nonzero: &[(u8, u8)] = &[
        (0x84, 6),
        (0x85, 6),
        (0x86, 6),
        (0x87, 6),
        (0x9e, 12),
        (0x9f, 12),
        (0xa0, 12),
        (0xa1, 12),
        (0xa2, 18),
        (0xa3, 18),
        (0xa4, 18),
        (0xa5, 18),
        (0xbc, 24),
        (0xbd, 24),
        (0xbe, 24),
        (0xbf, 24),
        (0xc0, 24),
        (0xc1, 24),
        (0xc9, 10),
        (0xca, 25),
        (0xcb, 2),
        (0xce, 2),
        (0xcf, 50),
        (0xd7, 4),
        (0xd8, 10),
        (0xdc, 2),
        (0xdd, 10),
        (0xde, 10),
        (0xdf, 10),
        (0xe0, 5),
        (0xe3, 5),
        (0xe4, 5),
        (0xe5, 5),
        (0xe6, 10),
        (0xe7, 10),
        (0xe9, 5),
        (0xec, 3),
        (0xed, 4),
        (0xee, 2),
        (0xef, 2),
        (0xf0, 2),
        (0xf1, 2),
        (0xf2, 10),
        (0xf4, 10),
        (0xf6, 5),
        (0xfb, 25),
        (0xfc, 10),
        (0xfd, 12),
        (0xfe, 15),
    ];
    let mut expected = [0_u8; 256];
    for &(tile, weight) in native_table_nonzero {
        expected[usize::from(tile)] = weight;
    }

    for tile in 0..=u8::MAX {
        let expected_weight = if tile == 5 {
            0xc8
        } else {
            u32::from(expected[usize::from(tile)])
        };
        assert_eq!(
            pollution_weight_for_building(tile),
            expected_weight,
            "Mac/PE pollution weight mismatch at XBLD {tile:#04x}"
        );
    }
}

#[test]
fn pollution_blur_divisor_base_matches_pe_bonus_math() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // PE new-city init leaves sewer/pollution-mix BSS at 0 → 0-0+4 = 4.
    assert_eq!(sewer_bonus_for_pollution_blur(&city), 0);
    assert_eq!(industrial_mix_pollution_bonus_for_blur(&city), 0);
    assert_eq!(pollution_blur_divisor_base_for_city(&city), 4);

    publish_pe_sewer_bonus(&mut city);
    assert_eq!(sewer_bonus_for_pollution_blur(&city), 1);
    assert_eq!(pollution_blur_divisor_base_for_city(&city), 5);

    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_POLLUTION_CONTROLS,
        true
    ));
    assert_eq!(
        pollution_blur_divisor_base_for_city(&city),
        6,
        "pollution controls add +1 to the blur divisor base"
    );
    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_POLLUTION_CONTROLS,
        false
    ));

    // Dirty industrial mix raises the pollution-bonus word and shrinks the divisor.
    city.set_building(10, 10, 0x84); // steel
    city.set_building(11, 10, 0x85); // textiles
    city.set_building(12, 10, 0x86); // petro
    city.set_building(13, 10, 0x9f); // automotive
    let dirty_bonus = industrial_mix_pollution_bonus_from_tiles(&city);
    city.set_pe_w_industrial_mix_pollution_bonus(dirty_bonus);
    let sewer = sewer_bonus_for_pollution_blur(&city);
    assert!(
        dirty_bonus >= 0,
        "dirty mix should leave the -1 floor ({dirty_bonus})"
    );
    assert_eq!(
        pollution_blur_divisor_base_for_city(&city),
        (i32::from(sewer) - i32::from(dirty_bonus) + 4).max(1)
    );
    assert!(
        pollution_blur_divisor_base_for_city(&city) < 5,
        "dirty industrial mix should shrink the empty-city blur base"
    );
}

#[test]
fn pollution_blur_divisor_dilutes_source_with_pollution_controls() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(40, 40, 5); // +0xC8 tile-5 term
    let concentrated = live_pollution_density_map_for_city(&city)[density_map_index(40, 40)];

    assert!(set_city_ordinance_enabled(
        &mut city,
        ORDINANCE_POLLUTION_CONTROLS,
        true
    ));
    let spread = live_pollution_density_map_for_city(&city)[density_map_index(40, 40)];
    // Ordinance adds +1 blur divisor and a 75% post-scale; either lowers the center.
    assert!(
        spread < concentrated,
        "pollution-controls path should dilute the source cell ({spread} vs {concentrated})"
    );
}

#[test]
fn abandoned_census_unemployment_percent_formula() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let empty = live_population_stats_for_city(&city);
    assert_eq!(abandoned_population_for_city(&city), 0);
    assert_eq!(abandoned_census_unemployment_percent(&city, empty), 0);

    city.set_zone(10, 10, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(10, 10, ABANDONED_1X1_BUILDINGS[0]);
    let ghost = live_population_stats_for_city(&city);
    assert_eq!(abandoned_population_for_city(&city), 12);
    assert_eq!(ghost.normal_population, 0);
    assert_eq!(
        abandoned_census_unemployment_percent(&city, ghost),
        12 * 100 / (12 + 1),
        "ghost town: abandoned*100/(abandoned+0+1)"
    );

    city.set_zone(12, 10, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(12, 10, 0x70);
    let mixed = live_population_stats_for_city(&city);
    assert_eq!(mixed.normal_population, 12);
    assert_eq!(
        abandoned_census_unemployment_percent(&city, mixed),
        12 * 100 / (12 + 12 + 1),
        "abandoned-census stand-in (not PE dwCityUnemployment)"
    );
}

#[test]
fn unemployment_percent_reads_pe_global() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let population = live_population_stats_for_city(&city);
    assert_eq!(unemployment_percent_for_city(&city, population), 0);
    city.set_pe_unemployment_percent(37);
    assert_eq!(unemployment_percent_for_city(&city, population), 37);
}

#[test]
fn live_misc_persists_pe_unemployment() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_pe_unemployment_percent(48);
    let mut state = test_state(city);

    state.refresh_live_misc_current_values();

    assert_eq!(
        state.city.misc_u32_at(MISC_UNEMPLOYMENT_OFFSET),
        Some(48),
        "MISC 0x0FA4 mirrors pe_unemployment (PE 0x4CA5F8)"
    );
}

#[test]
fn publish_pe_growth_aggregate_snaps_slot7_for_eq_le_penalty() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x8A);
    let slot7 = pe_growth_aggregate_weight(1);
    assert!(slot7 > 0);

    state.publish_pe_growth_aggregate();

    assert_eq!(
        state.city.pe_eq_growth_slot7(),
        slot7,
        "day-21 publish snaps slot 7 for FUN_00468D30 esp+0x10"
    );
    assert_eq!(
        crate::city::pe_eq_education_life_penalty(slot7 as u32 * 10 + 1, 0, slot7),
        1,
        "with city_pop 0: pollution / (10*slot7 + 1)"
    );
}

#[test]
fn publish_pe_growth_aggregate_writes_zone_population_rows() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
    state.pe_growth_aggregate.add_xbld_candidate(3, 0x70);
    state.pe_growth_aggregate.add_xbld_candidate(5, 0x70);
    let expected = {
        let mut probe = PeGrowthAggregate::default();
        probe.add_xbld_candidate(1, 0x70);
        probe.add_xbld_candidate(3, 0x70);
        probe.add_xbld_candidate(5, 0x70);
        probe.fold_normal_slots();
        probe.zone_population_rows()
    };

    state.publish_pe_growth_aggregate();

    assert!(state.city.pe_zone_pops_ready());
    assert_eq!(state.city.pe_zone_pops(), expected);
    assert_eq!(
        pe_rci_zone_pops_for_demand(&state.city, live_population_stats_for_city(&state.city)),
        (expected[0], expected[1], expected[2]),
        "RCI demand prefers published pZonePops over live census"
    );
}

#[test]
fn pe_unemployment_is_latched_until_monthly_graph_publication() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank("/tmp/pe-unemployment.sc2", "test").unwrap();
        city.kind = kind;
        city.set_pe_unemployment_percent(37);
        let mut state = test_state(city);
        state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
        state.pe_growth_aggregate.add_xbld_candidate(1, 0x8A);
        let mut probe = state.pe_growth_aggregate;
        probe.fold_normal_slots();
        let expected = probe.unemployment_ratio_percent();
        assert!(expected > 0);
        state.publish_pe_growth_aggregate();
        assert_eq!(
            state.city.pe_unemployment_percent(),
            37,
            "demand/news see prior value"
        );
        state.refresh_xgrp_history(PopulationStats::default(), CityServiceOutcomes::default());
        assert_eq!(state.city.pe_unemployment_percent(), expected, "{kind:?}");
        let history = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
        assert_eq!(history[12 * XGRP_SAMPLES_PER_SERIES], expected, "{kind:?}");
        assert_eq!(
            unemployment_percent_for_city(&state.city, PopulationStats::default()),
            expected,
            "{kind:?}"
        );
    }
}

#[test]
fn prison_bonus_raises_live_police_coverage() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // One marked source tile keeps the center below byte saturation so the
    // native +1 wPrisonBonus contribution is observable.
    city.set_building(20, 20, 210);
    city.set_zone(20, 20, 0x80);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0).with_powerable(true).with_powered(true),
    );
    power_adjacent_building_for_test(&mut city, 20, 19);
    let population = PopulationStats {
        populated_tiles: 1,
        ..Default::default()
    };
    let outcomes = CityServiceOutcomes::default();
    let index = density_map_index(20, 20);

    let without = live_density_maps_for_city_with_power_seed_and_pe_xbit(
        &city,
        population,
        outcomes,
        pe_power_recalc_seed(&city),
        true,
    );
    city.provision_microsim_for_building(216).expect("prison");
    city.set_pe_w_prison_bonus(1);
    assert_eq!(prison_bonus_for_city(&city), 1);
    let with = live_density_maps_for_city_with_power_seed_and_pe_xbit(
        &city,
        population,
        outcomes,
        pe_power_recalc_seed(&city),
        true,
    );

    assert!(
        with.police[index] > without.police[index],
        "wPrisonBonus should raise XPLC paint strength ({} vs {})",
        with.police[index],
        without.police[index]
    );
}

#[test]
fn automatic_churches_wait_for_population_threshold() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zoned_building_footprint_for_test(
        &mut city,
        (20, 20),
        CHURCH_BUILDING_SIZE,
        ZONE_RESIDENTIAL_DENSE,
        CONSTRUCTION_POP2_BUILDINGS[0],
    );
    // Leftover 0x4caa74 is 0 on a new city: 0*2500 < 0 is false.
    assert!(!pe_b_place_church(
        church_tile_count_for_city(&city),
        city.pe_dw_city_population()
    ));
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.complete_rci_construction_sites(), 4);
    assert_ne!(state.city.building_at(20, 20), CHURCH_BUILDING_ID);
    assert!(
        RESIDENTIAL_2X2_BUILDINGS.contains(&state.city.building_at(20, 20)),
        "pop2 construction under church threshold should ChooseAndPlace residential"
    );
}

#[test]
fn rci_construction_complete_requires_xzon_view_corner_on_multitile() {
    for rotation in 0..4u8 {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        stamp_zoned_building_footprint_for_test(
            &mut city,
            (20, 20),
            2,
            ZONE_RESIDENTIAL_DENSE,
            CONSTRUCTION_POP2_BUILDINGS[0],
        );
        set_rci_demand_for_test(&mut city, 1_000, 0, 0);
        city.set_view_rotation(rotation);
        let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2);

        let mut blocked = city.clone();
        blocked.set_zone(sx, sy, ZONE_RESIDENTIAL_DENSE);
        let mut state = test_state(blocked);
        assert_eq!(
            state.complete_rci_construction_sites(),
            0,
            "rotation {rotation}: wrong-corner completion must be blocked by XZON"
        );

        let mut state = test_state(city);
        assert_eq!(
            state.complete_rci_construction_sites(),
            4,
            "rotation {rotation}: view-angle corner should complete"
        );
        assert!(
            RESIDENTIAL_2X2_BUILDINGS.contains(&state.city.building_at(20, 20)),
            "rotation {rotation} should complete to a 2×2 building"
        );
    }
}

#[test]
fn rci_decline_requires_xzon_view_corner_on_multitile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let building = RESIDENTIAL_2X2_BUILDINGS[0];
    stamp_zoned_building_footprint_for_test(
        &mut city,
        (20, 20),
        2,
        ZONE_RESIDENTIAL_DENSE,
        building,
    );
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    city.set_view_rotation(0);
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&city, (20, 20), 2);
    assert_eq!((sx, sy), (20, 21));

    let mut blocked = city.clone();
    blocked.set_zone(20, 21, ZONE_RESIDENTIAL_DENSE);
    let mut state = test_state(blocked);
    assert_eq!(
        state.decline_unserved_rci_zones(),
        0,
        "NW-only XZON is not wPositionAngle[0]"
    );

    let mut state = test_state(city);
    assert_eq!(state.decline_unserved_rci_zones(), 4);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
}

#[test]
fn win95_abandoned_redevelopment_uses_raw_corner_and_keeps_cell_zones() {
    // Native 0x43620f calls ChooseAndPlace with the scanned corner and current pop.
    // The fixture supplies the same successful trip result as the native replay.
    let _live_rng = LiveGrowthRng::enable();
    for (x, y, abandoned, size, expected) in [
        (20, 21, 0xaa, 2, Some(0x96)),
        (20, 22, 0xc4, 3, Some(0xb9)),
        (1, 21, 0xaa, 2, None),
    ] {
        let mut city = City::new_blank("/tmp/citygame-redevelop.sc2", "Redevelop").unwrap();
        city.set_view_rotation(0);
        let origin = (x, y + 1 - size);
        let footprint = building_footprint_from_origin(origin.0, origin.1, size).unwrap();
        for (i, &(tx, ty)) in footprint.iter().enumerate() {
            city.set_building(tx, ty, abandoned);
            city.set_zone(tx, ty, ZONE_INDUSTRIAL_DENSE);
            city.set_terrain(tx, ty, (i & 1) as u8);
        }
        city.set_zone(x, y, ZONE_COMMERCIAL_DENSE | PE_POSITION_ANGLE[0]);
        set_rci_demand_for_test(&mut city, 1_000, 1_000, 1_000);
        let mut state = test_state(city);
        state.bypass_growth_trip_gate = false;
        state.trip_served_tiles[y * CITY_WIDTH + x] = true;
        let mut power = PowerServiceMaps::default();
        set_saved_xbit_growth_power_for_test(&mut power, x, y - 1, true);
        let mut crt = Win95CrtRand::new(1);
        let candidate = state.rci_development_candidate_at(
            x,
            y,
            &vec![false; CITY_TILES],
            &power,
            city_service_outcomes_for_city(&state.city),
            &rci_development_services_for_city(&state.city),
            Some(&mut crt),
        );
        assert_eq!(crt.seed(), 3_357_800_067, "roll and picker at ({x},{y})");
        assert_eq!(candidate.as_ref().map(|c| c.plan.building), expected);
        if let Some(candidate) = candidate {
            assert_eq!(candidate.plan.origin, origin);
            assert_eq!(candidate.plan.footprint, footprint);
            state.stamp_zone_development_with_crt(&candidate.plan, &mut crt);
            for (tx, ty) in footprint {
                assert_eq!(state.city.building_at(tx, ty), expected.unwrap());
                let zone = if (tx, ty) == (x, y) {
                    ZONE_COMMERCIAL_DENSE
                } else {
                    ZONE_INDUSTRIAL_DENSE
                };
                assert_eq!(state.city.zone_at(tx, ty).unwrap().zone_id(), zone);
            }
            assert_eq!(crt.seed(), 3_357_800_067);
        }
    }
}

#[test]
fn rci_abandoned_redevelop_requires_xzon_view_corner_on_multitile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zoned_building_footprint_for_test(&mut city, (20, 20), 2, ZONE_COMMERCIAL_DENSE, 0xAA);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 2);
    set_rci_demand_for_test(&mut city, 0, 1_000, 0);
    city.set_view_rotation(0);

    let mut blocked = city.clone();
    blocked.set_zone(20, 21, ZONE_COMMERCIAL_DENSE);
    let mut state = test_state(blocked);
    state.refresh_simulation_service_bits();
    assert_eq!(
        state.develop_rci_zones(),
        0,
        "abandoned 2x2 without the view-rotation corner does not redevelop"
    );

    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    assert_eq!(state.develop_rci_zones(), 4);
    assert!(COMMERCIAL_2X2_BUILDINGS.contains(&state.city.building_at(20, 20)));
}

#[test]
fn pe_place_church_refuses_sw_outside_1_through_0x7e() {
    // PE @ `0x43731b`–`0x437343`: SW (si,di) must be in 1..=0x7e.
    let mut city = City::new_blank("/tmp/sc2000-church-bounds.sc2", "ChurchBnd").expect("blank");
    pe_place_church_437310(&mut city, 0, 10);
    assert_eq!(city.building_at(0, 9), 0);
    assert_eq!(city.building_at(0, 10), 0);
    pe_place_church_437310(&mut city, 10, 0x7f);
    assert_eq!(city.building_at(10, 0x7e), 0);
    assert_eq!(city.building_at(10, 0x7f), 0);
    pe_place_church_437310(&mut city, 0x7f, 10);
    assert_eq!(city.building_at(0x7f, 9), 0);
    assert_eq!(city.building_at(0x7f, 10), 0);
    pe_place_church_437310(&mut city, 1, 1);
    assert_eq!(city.building_at(1, 0), CHURCH_BUILDING_ID);
    assert_eq!(city.building_at(1, 1), CHURCH_BUILDING_ID);
    assert_eq!(city.building_at(2, 0), CHURCH_BUILDING_ID);
    assert_eq!(city.building_at(2, 1), CHURCH_BUILDING_ID);
}

#[test]
fn automatic_churches_replace_complete_two_by_two_residential_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zoned_building_footprint_for_test(
        &mut city,
        (20, 20),
        CHURCH_BUILDING_SIZE,
        ZONE_RESIDENTIAL_DENSE,
        CONSTRUCTION_POP2_BUILDINGS[0],
    );
    city.persist_pe_day22_population_leftover(1);
    assert!(pe_b_place_church(
        church_tile_count_for_city(&city),
        city.pe_dw_city_population()
    ));
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.complete_rci_construction_sites(), 4);

    // PE PlaceChurch `FUN_00437310`: XBLD 0xF7, XZON type cleared, CAP corners, XBIT |= 0xe0.
    let corner_bits = pe_choose_and_place_corner_bits(state.city.rotation());
    for &(x, y) in &building_footprint_from_origin(20, 20, CHURCH_BUILDING_SIZE).unwrap() {
        assert_eq!(state.city.building_at(x, y), CHURCH_BUILDING_ID);
        let zone = state.city.zone_at(x, y).unwrap();
        assert_eq!(
            zone.zone_id(),
            ZONE_RESIDENTIAL_DENSE,
            "PlaceChurch preserves XZON type nibble"
        );
        assert_eq!(
            zone.raw_bits(),
            ZONE_RESIDENTIAL_DENSE
                | building_zone_bits_for_tile(20, 20, x, y, CHURCH_BUILDING_SIZE, corner_bits)
        );
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert_eq!(
            bits.raw_bits() & XBIT_CHOOSE_AND_PLACE,
            XBIT_CHOOSE_AND_PLACE
        );
    }
}

#[test]
fn build_think_place_church_rejected_at_edge_exits_without_stamping() {
    let origin = (0, 20);
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zoned_building_footprint_for_test(
        &mut city,
        origin,
        CHURCH_BUILDING_SIZE,
        ZONE_RESIDENTIAL_DENSE,
        CONSTRUCTION_POP2_BUILDINGS[0],
    );
    let (x, y) = rci_xzon_view_corner_in_footprint_for_test(&city, origin, CHURCH_BUILDING_SIZE);
    assert_eq!(x, 0, "the Church SW call is outside FUN_00437310's bounds");
    let footprint = building_footprint_from_origin(origin.0, origin.1, CHURCH_BUILDING_SIZE)
        .expect("construction footprint fits the map");
    let mut state = test_state(city);
    let before: Vec<_> = footprint
        .iter()
        .map(|&(tx, ty)| {
            (
                state.city.building_at(tx, ty),
                state.city.zone_at(tx, ty).unwrap().raw_bits(),
                state
                    .city
                    .tile_bits_at(tx, ty)
                    .unwrap_or_default()
                    .raw_bits(),
            )
        })
        .collect();
    let mut reserved = vec![false; CITY_TILES];
    let mut crt = Win95CrtRand::new(Win95CrtRand::DEFAULT_SEED);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let services = rci_development_services_for_city(&state.city);

    let plan = state
        .complete_rci_construction_site_at(x, y, outcomes, &services, true, &mut reserved, &mut crt)
        .expect("successful BUILD_THINK selects the PlaceChurch branch");
    assert_eq!(plan.building, CHURCH_BUILDING_ID);
    assert!(
        footprint
            .iter()
            .all(|&(tx, ty)| reserved[ty * CITY_WIDTH + tx])
    );

    // PlaceChurch rejects SW x=0 before any PlaceTile/XZON/XBIT writes, but
    // BUILD_THINK still exits the scan visit after the call.
    state.stamp_zone_development(&plan);
    for (index, &(tx, ty)) in footprint.iter().enumerate() {
        assert_eq!(
            (
                state.city.building_at(tx, ty),
                state.city.zone_at(tx, ty).unwrap().raw_bits(),
                state
                    .city
                    .tile_bits_at(tx, ty)
                    .unwrap_or_default()
                    .raw_bits(),
            ),
            before[index],
            "rejected PlaceChurch must leave ({tx},{ty}) untouched"
        );
    }
}

#[test]
fn simulation_month_uses_leftover_population_not_misc_census_for_church_threshold() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, CHURCH_POPULATION_STEP);
    city.persist_pe_day22_population_leftover(1);
    stamp_zoned_building_footprint_for_test(
        &mut city,
        (20, 20),
        CHURCH_BUILDING_SIZE,
        ZONE_RESIDENTIAL_DENSE,
        RESIDENTIAL_2X2_BUILDINGS[0],
    );
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, CHURCH_BUILDING_SIZE);
    assert!(pe_b_place_church(0, city.pe_dw_city_population()));
    assert!(live_population_stats_for_city(&city).normal_population < CHURCH_POPULATION_STEP);
    set_rci_demand_for_test(&mut city, 2_000, 0, 0);
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    // PlaceChurch only runs on BUILD_THINK complete. A served finished 2×2 can size-up
    // to construction this visit, but leftover `bPlaceChurch` must not convert it.
    for (x, y) in building_footprint_from_origin(20, 20, CHURCH_BUILDING_SIZE).unwrap() {
        let building = state.city.building_at(x, y);
        assert_ne!(building, CHURCH_BUILDING_ID);
        assert!(
            RESIDENTIAL_2X2_BUILDINGS.contains(&building) || is_construction_building(building),
            "expected housing or StartBuilding construction, got {building:#04x} at ({x},{y})"
        );
    }
}

#[test]
fn simulation_month_uses_post_decline_population_for_automatic_church_threshold() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let candidate = (20, 20);
    stamp_zoned_building_footprint_for_test(
        &mut city,
        candidate,
        CHURCH_BUILDING_SIZE,
        ZONE_RESIDENTIAL_DENSE,
        RESIDENTIAL_2X2_BUILDINGS[0],
    );
    stamp_transport_access_for_test(&mut city, candidate.0 - 1, candidate.1);
    power_adjacent_building_for_test(&mut city, candidate.0, candidate.1 - 1);
    water_footprint_from_below_for_test(&mut city, candidate.0, candidate.1, CHURCH_BUILDING_SIZE);
    for index in 0..4 {
        stamp_zoned_building_footprint_for_test(
            &mut city,
            (80 + index * 4, 20),
            3,
            ZONE_RESIDENTIAL_DENSE,
            RESIDENTIAL_3X3_BUILDINGS[0],
        );
    }
    set_misc_u32_for_test(
        &mut city,
        tax_rate_misc_offset(0).expect("residential tax offset"),
        MAX_TAX_RATE,
    );
    assert!(
        rci_demand_values_for_city(
            &city,
            live_population_stats_for_city(&city),
            live_city_service_outcomes_for_city(&city),
        )[rci_kind_index(RciKind::Residential)]
            < 0
    );
    let mut state = test_state(city);

    state.handle_simulation_month_advanced();

    // PlaceChurch only runs on BUILD_THINK complete. Negative demand can ChangeBuilding
    // the served 2×2 this visit; leftover pop must not convert it to a church.
    for (x, y) in
        building_footprint_from_origin(candidate.0, candidate.1, CHURCH_BUILDING_SIZE).unwrap()
    {
        let building = state.city.building_at(x, y);
        assert_ne!(building, CHURCH_BUILDING_ID);
        assert!(
            RESIDENTIAL_2X2_BUILDINGS.contains(&building)
                || is_construction_building(building)
                || is_abandoned_building(building),
            "expected housing, construction, or ChangeBuilding abandon, got {building:#04x} at ({x},{y})"
        );
    }
}

#[test]
fn automatic_churches_do_not_overbuild_current_population_tier() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zoned_building_footprint_for_test(
        &mut city,
        (20, 20),
        CHURCH_BUILDING_SIZE,
        ZONE_RESIDENTIAL_DENSE,
        CONSTRUCTION_POP2_BUILDINGS[0],
    );
    stamp_zoned_building_footprint_for_test(
        &mut city,
        (30, 20),
        CHURCH_BUILDING_SIZE,
        ZONE_RESIDENTIAL_DENSE,
        CHURCH_BUILDING_ID,
    );
    city.persist_pe_day22_population_leftover(1);
    assert!(!pe_b_place_church(
        church_tile_count_for_city(&city),
        city.pe_dw_city_population()
    ));
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);

    assert_eq!(state.complete_rci_construction_sites(), 4);
    for (x, y) in building_footprint_from_origin(20, 20, CHURCH_BUILDING_SIZE).unwrap() {
        assert!(
            RESIDENTIAL_2X2_BUILDINGS.contains(&state.city.building_at(x, y)),
            "existing church should satisfy the tier so completion ChooseAndPlaces residential"
        );
    }
    assert_eq!(
        complete_building_count_for_city(&state.city, CHURCH_BUILDING_ID, CHURCH_BUILDING_SIZE),
        1
    );
}

#[test]
fn automatic_churches_only_replace_two_by_two_residential_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_zoned_building_footprint_for_test(
        &mut city,
        (20, 20),
        2,
        ZONE_COMMERCIAL_DENSE,
        CONSTRUCTION_POP2_BUILDINGS[0],
    );
    stamp_zoned_building_footprint_for_test(
        &mut city,
        (30, 20),
        CHURCH_BUILDING_SIZE,
        ZONE_RESIDENTIAL_DENSE,
        CONSTRUCTION_POP2_BUILDINGS[0],
    );
    city.set_zone(40, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(40, 20, CONSTRUCTION_POP1_BUILDINGS[0]);
    city.persist_pe_day22_population_leftover(1);
    set_rci_demand_for_test(&mut city, 1_000, 1_000, 0);
    let mut state = test_state(city);

    assert!(state.complete_rci_construction_sites() >= 4);

    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_ne!(state.city.building_at(x, y), CHURCH_BUILDING_ID);
        assert!(COMMERCIAL_2X2_BUILDINGS.contains(&state.city.building_at(x, y)));
    }
    assert_ne!(state.city.building_at(40, 20), CHURCH_BUILDING_ID);
    assert!(RESIDENTIAL_1X1_BUILDINGS.contains(&state.city.building_at(40, 20)));
    for (x, y) in building_footprint_from_origin(30, 20, CHURCH_BUILDING_SIZE).unwrap() {
        assert_eq!(state.city.building_at(x, y), CHURCH_BUILDING_ID);
        assert_eq!(
            state.city.zone_at(x, y).map(|zone| zone.zone_id()),
            Some(ZONE_RESIDENTIAL_DENSE),
            "PlaceChurch preserves the XZON type nibble"
        );
    }
}

#[test]
fn special_zone_growth_power_gate_uses_fun_00436d00_cross() {
    // PE SimulationGrowSpecificZone: IsZonedTilePowered / FUN_00436D00 at visited tile.
    let mut city = City::new_blank("/tmp/sc2000-special-pwr.sc2", "SpecPwr").expect("blank");
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, (20, 20), 2, ZONE_AIRPORT);
    stamp_airport_runway_zone_ray_for_test(&mut city, (21, 20));
    stamp_transport_access_for_test(&mut city, 20, 20);
    let mut state = test_state(city);
    let reserved = vec![false; CITY_TILES];
    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    set_saved_xbit_growth_power_for_test(&mut power_maps, 21, 19, true); // north of (21,20)
    assert!(
        state
            .special_zone_development_plan_at(21, 20, &reserved, &power_maps)
            .is_some(),
        "special-zone growth must accept FUN_00436D00 cardinal neighbor power"
    );

    clear_saved_xbit_growth_power_for_test(&mut power_maps);
    set_saved_xbit_growth_power_for_test(&mut power_maps, 20, 19, true);
    assert!(
        state
            .special_zone_development_plan_at(21, 20, &reserved, &power_maps)
            .is_none(),
        "diagonal-only power must not satisfy special-zone FUN_00436D00"
    );
}

#[test]
fn special_zone_runway_has_internal_pipes_without_service_flags() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let origin = (10, 11);
    let size = 2;
    enable_special_zone_demand_for_test(&mut city);
    stamp_zone_footprint_for_test(&mut city, origin, size, ZONE_AIRPORT);
    stamp_airport_runway_zone_ray_for_test(&mut city, origin);
    stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1);
    power_adjacent_building_for_test(&mut city, origin.0, origin.1 - 1);
    let mut state = test_state(city);

    seed_pe_process_crt_until_special_zone_develops(&mut state);
    assert!(state.develop_special_zones() > 0);
    assert_eq!(
        state.city.building_at(origin.0, origin.1),
        RUNWAY_BUILDING_ID
    );
    assert!(building_internal_pipe_present(
        &state.city,
        origin.0,
        origin.1
    ));
    let bits = state.city.tile_bits_at(origin.0, origin.1).unwrap();
    assert!(!bits.piped());
    assert!(!bits.watered());
}

#[test]
fn mac_fallback_special_zones_keep_the_citygame_demand_gate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let airport_origin = (10, 11);
    let seaport_origin = (20, 10);
    for (zone_id, origin) in [
        (ZONE_AIRPORT, airport_origin),
        (ZONE_SEAPORT, seaport_origin),
    ] {
        stamp_zone_footprint_for_test(&mut city, origin, 2, zone_id);
        if zone_id == ZONE_AIRPORT {
            stamp_airport_runway_zone_ray_for_test(&mut city, origin);
        } else {
            stamp_seaport_crane_water_ray_for_test(&mut city, origin);
        }
        stamp_transport_access_for_test(&mut city, origin.0 - 1, origin.1);
        power_adjacent_building_for_test(&mut city, origin.0, origin.1 - 1);
        water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 2);
    }
    let mut state = test_state(city);
    // This demand threshold is not present in the rooted Win95 caller. Keep
    // coverage for the legacy non-Win95 fallback without presenting it as PE
    // behavior.
    state.engine_fidelity = EngineFidelity::Mac68k;

    let expected_developments = expected_special_zone_development_count(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    assert_eq!(
        state.city.building_at(airport_origin.0, airport_origin.1),
        0
    );
    assert_eq!(
        state.city.building_at(seaport_origin.0, seaport_origin.1),
        0
    );

    enable_seaport_demand_for_test(&mut state.city);
    let expected_developments = expected_special_zone_development_count(&state);
    let expected_plans = expected_special_zone_development_plans(&state);
    assert_eq!(state.develop_special_zones(), expected_developments);
    assert_eq!(
        state.city.building_at(airport_origin.0, airport_origin.1),
        expected_plans
            .iter()
            .find(|plan| {
                plan.zone_id == ZONE_AIRPORT && plan.footprint.contains(&airport_origin)
            })
            .map_or(0, |plan| plan.building)
    );
    assert_eq!(
        state.city.building_at(seaport_origin.0, seaport_origin.1),
        if let Some(plan) = expected_plans.iter().find(|plan| {
            plan.zone_id == ZONE_SEAPORT && plan.footprint.contains(&seaport_origin)
        }) {
            plan.building
        } else {
            0
        }
    );

    enable_airport_demand_for_test(&mut state.city);
    let expected_developments = expected_special_zone_development_count(&state);
    let expected_plans = expected_special_zone_development_plans(&state);
    // Only the transport-adjacent tile of the 2x2 pad can place a 1x1 runway.
    assert_eq!(state.develop_special_zones(), expected_developments);
    assert_eq!(
        state.city.building_at(airport_origin.0, airport_origin.1),
        expected_plans
            .iter()
            .find(|plan| {
                plan.zone_id == ZONE_AIRPORT && plan.footprint.contains(&airport_origin)
            })
            .map_or(0, |plan| plan.building)
    );
}

#[test]
fn pe_military_zone_without_an_active_base_keeps_the_crt_seed() {
    let mut city = City::new_blank("/tmp/citygame-military-roll.sc2", "Military").unwrap();
    city.set_zone(20, 20, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_building(20, 20, 0xe7);
    let mut state = test_state(city);
    for base in [MILITARY_BASE_NONE, MILITARY_BASE_DECLINED, 6, 255] {
        set_military_base_type_for_test(&mut state.city, base);
        let before = state.pe_process_crt_seed();
        state.sim_calendar_one_growth_tick(0, 0);
        assert_eq!(state.pe_process_crt_seed(), before, "selector {base}");
        assert_eq!(state.city.building_at(20, 20), 0xe7);
    }
}

#[test]
fn special_zone_growth_roll_silo_skips_crt_gate_pe_4364f8() {
    // PE SimulationGrow silo arm @ `0x4364f8`: no `call rand` / `test al,3` —
    // only `cmp cl, 0xf9` then PlaceTile `0xf9` via `0x4026b2` → `0x4382b0`.
    let mut city = City::new_blank("/tmp/sc2000-silo-roll.sc2", "SiloRoll").expect("blank");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_MISSILE_SILOS);
    let state = test_state(city);
    for seed in 0..64u32 {
        let mut rng = Win95CrtRand::new(seed);
        let before = rng.seed();
        assert!(
            state.special_zone_growth_roll_allows(&mut rng, ZONE_MILITARY),
            "silo seed {seed} must always admit"
        );
        assert_eq!(
            rng.seed(),
            before,
            "silo monthly must not consume CRT seed {seed}"
        );
    }
}

#[test]
fn reserved_special_zone_tile_still_consumes_pe_growth_roll() {
    // The PE visits each lattice tile even if a prior multi-tile placement covers it.
    // The zone branch still consumes its rand() & 3 gate; reservation can suppress
    // stamping, but must not shift the process CRT used by later growth/actor calls.
    let state = test_state(
        City::new_blank("/tmp/sc2000-reserved-zone-roll.sc2", "ReservedRoll").expect("blank city"),
    );

    for seed in 0..64u32 {
        let mut pe_rng = Win95CrtRand::new(seed);
        let admitted = (pe_rng.random_u16() & 3) == 0;
        for reserved in [false, true] {
            let mut actual_rng = Win95CrtRand::new(seed);
            let (selected, may_stamp) = state
                .special_zone_growth_tick_branch(ZONE_AIRPORT, reserved, &mut actual_rng)
                .expect("airport uses a special-zone branch");

            assert_eq!(selected, admitted, "seed={seed}, reserved={reserved}");
            assert_eq!(
                actual_rng.seed(),
                pe_rng.seed(),
                "seed={seed}, reserved={reserved}"
            );
            assert_eq!(
                may_stamp,
                admitted && !reserved,
                "seed={seed}, reserved={reserved}"
            );
        }
    }
}

#[test]
fn special_zone_growth_rolls_every_vacant_military_tile() {
    // PE SimulationGrow army @ `0x436304`: every vacant military tile draws `test al,3`.
    let mut city = City::new_blank("/tmp/sc2000-mil-roll.sc2", "MilRoll").expect("blank");
    set_military_base_type_for_test(&mut city, MILITARY_BASE_ARMY);
    for x in [20, 24, 28, 32, 36] {
        stamp_zone_footprint_for_test(&mut city, (x, 20), 2, ZONE_MILITARY);
        power_adjacent_building_for_test(&mut city, x, 19);
    }
    let mut state = test_state(city);

    let mut expected_growth = 0usize;
    let mut expected_origins = Vec::<(usize, usize)>::new();
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
            if zone_id != ZONE_MILITARY {
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
            expected_origins.push(plan.origin);
        }
    }

    let developed = state.develop_special_zones();
    assert!(developed > 0);
    for origin in [(20, 20), (24, 20), (28, 20), (32, 20), (36, 20)] {
        if state.city.building_at(origin.0, origin.1) == MILITARY_PARKING_LOT_BUILDING_ID {
            assert!(expected_origins.contains(&origin));
        }
    }
    assert!(developed <= expected_growth);
}

#[test]
fn placeable_buildings_are_not_special_zone_growth_outputs() {
    for &tool in DIRECT_PLACEABLE_BUILDING_TOOLS_FOR_TEST {
        let building = placeable_building_for_tool(tool).expect("direct placeable building");
        assert!(
            !special_zone_developed_building(ZONE_AIRPORT, building),
            "{tool:?} building {building} should not count as airport zone growth"
        );
        assert!(
            !special_zone_developed_building(ZONE_SEAPORT, building),
            "{tool:?} building {building} should not count as seaport zone growth"
        );
    }
}

#[test]
fn rci_growth_tables_only_emit_matching_zone_buildings() {
    let cases = [
        (ZONE_RESIDENTIAL_LIGHT, 1, RciKind::Residential),
        (ZONE_RESIDENTIAL_DENSE, 1, RciKind::Residential),
        (ZONE_RESIDENTIAL_DENSE, 2, RciKind::Residential),
        (ZONE_RESIDENTIAL_DENSE, 3, RciKind::Residential),
        (ZONE_COMMERCIAL_LIGHT, 1, RciKind::Commercial),
        (ZONE_COMMERCIAL_DENSE, 1, RciKind::Commercial),
        (ZONE_COMMERCIAL_DENSE, 2, RciKind::Commercial),
        (ZONE_COMMERCIAL_DENSE, 3, RciKind::Commercial),
        (ZONE_INDUSTRIAL_LIGHT, 1, RciKind::Industrial),
        (ZONE_INDUSTRIAL_DENSE, 1, RciKind::Industrial),
        (ZONE_INDUSTRIAL_DENSE, 2, RciKind::Industrial),
        (ZONE_INDUSTRIAL_DENSE, 3, RciKind::Industrial),
    ];

    for (zone_id, size, expected_kind) in cases {
        let buildings =
            rci_development_buildings_for_zone_and_size(zone_id, size).expect("RCI growth table");
        assert!(
            !buildings.is_empty(),
            "zone {zone_id} size {size} should have growth candidates"
        );
        for &building in buildings {
            assert!(
                rci_development_building_allowed(zone_id, size, building),
                "building {building} should be allowed for zone {zone_id} size {size}"
            );
            assert_eq!(
                rci_building_kind(building),
                Some(expected_kind),
                "building {building} should match zone {zone_id}"
            );
            assert_eq!(
                usize::from(xbld_tile_size(building).max(1)),
                size,
                "building {building} should match requested growth size"
            );
            assert!(
                !special_zone_developed_building(ZONE_AIRPORT, building),
                "RCI growth building {building} must not be an airport development"
            );
            assert!(
                !special_zone_developed_building(ZONE_SEAPORT, building),
                "RCI growth building {building} must not be a seaport development"
            );
            for &tool in DIRECT_PLACEABLE_BUILDING_TOOLS_FOR_TEST {
                assert_ne!(
                    placeable_building_for_tool(tool),
                    Some(building),
                    "RCI growth building {building} must not be direct-placeable {tool:?}"
                );
            }
        }
    }

    for zone_id in [0, 7, ZONE_AIRPORT, ZONE_SEAPORT] {
        for size in 1..=4 {
            assert!(
                rci_development_buildings_for_zone_and_size(zone_id, size).is_none(),
                "non-RCI zone {zone_id} should not have RCI growth size {size}"
            );
        }
    }
}

#[test]
fn rci_growth_selector_only_returns_allowed_buildings() {
    let cases = [
        (ZONE_RESIDENTIAL_LIGHT, 1),
        (ZONE_RESIDENTIAL_DENSE, 1),
        (ZONE_RESIDENTIAL_DENSE, 2),
        (ZONE_RESIDENTIAL_DENSE, 3),
        (ZONE_COMMERCIAL_LIGHT, 1),
        (ZONE_COMMERCIAL_DENSE, 1),
        (ZONE_COMMERCIAL_DENSE, 2),
        (ZONE_COMMERCIAL_DENSE, 3),
        (ZONE_INDUSTRIAL_LIGHT, 1),
        (ZONE_INDUSTRIAL_DENSE, 1),
        (ZONE_INDUSTRIAL_DENSE, 2),
        (ZONE_INDUSTRIAL_DENSE, 3),
    ];

    for (zone_id, size) in cases {
        for x in [0, 1, 7, 31, 63, 127] {
            for y in [0, 2, 11, 29, 71, 127] {
                for quality_band in 0..=4 {
                    let building = rci_development_building(zone_id, size, x, y, quality_band)
                        .expect("RCI growth building");
                    assert!(
                        rci_development_building_allowed(zone_id, size, building),
                        "selector returned building {building} for zone {zone_id} size {size}"
                    );
                }
            }
        }
    }

    for zone_id in [0, 7, ZONE_AIRPORT, ZONE_SEAPORT] {
        for size in 1..=4 {
            assert_eq!(
                rci_development_building(zone_id, size, 10, 20, 1),
                None,
                "non-RCI zone {zone_id} should not select size {size} growth"
            );
        }
    }
}

#[test]
fn monthly_growth_does_not_reclassify_direct_placeables_inside_special_zones() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut direct_placeable_tiles = Vec::new();
    for (index, &tool) in DIRECT_PLACEABLE_BUILDING_TOOLS_FOR_TEST.iter().enumerate() {
        let building = placeable_building_for_tool(tool).expect("direct placeable building");
        let origin_x = 10 + (index % 10) * 6;
        let origin_y = 10 + (index / 10) * 6;
        let zone_id = if index % 2 == 0 {
            ZONE_AIRPORT
        } else {
            ZONE_SEAPORT
        };
        let footprint = building_footprint_from_origin(
            origin_x,
            origin_y,
            usize::from(xbld_tile_size(building)),
        )
        .expect("test building footprint");
        for (x, y) in footprint {
            city.set_zone(x, y, zone_id);
            city.set_building(x, y, building);
            direct_placeable_tiles.push((x, y));
        }
    }
    let before: Vec<_> = direct_placeable_tiles
        .iter()
        .map(|&(x, y)| {
            (
                x,
                y,
                city.building_at(x, y),
                city.zone_at(x, y).map_or(0, |zone| zone.raw_bits()),
            )
        })
        .collect();
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    set_city_no_disasters_enabled(&mut state.city, true);

    for _ in 0..12 {
        state.handle_simulation_month_advanced();
    }

    let after: Vec<_> = direct_placeable_tiles
        .iter()
        .map(|&(x, y)| {
            (
                x,
                y,
                state.city.building_at(x, y),
                state.city.zone_at(x, y).map_or(0, |zone| zone.raw_bits()),
            )
        })
        .collect();
    assert_eq!(
        after, before,
        "direct placeables changed during monthly growth"
    );
}

#[test]
fn loaded_maxis_land_growth_keeps_developed_buildings_in_matching_zones() {
    let Some(path) = paths::test_mac_city() else {
        return;
    };
    let mut state = test_state(City::load(path).expect("load default city"));
    ensure_default_city_options(&mut state.city);
    state.refresh_simulation_service_bits();

    for _ in 0..12 {
        state.handle_simulation_month_advanced();
    }

    assert_developed_buildings_match_zone_kinds(&state.city);
}

#[test]
fn loaded_win95_city_keeps_native_growth_scan_enabled() {
    let path = std::env::temp_dir().join("sc2000_rust_loaded_zone_growth.sc2");
    let mut city = City::new_blank(&path, "Saved zone growth").expect("blank city");
    city.set_city_funds(20_000);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    // A native PE ChooseAndPlace zone write persists the 0xe0 XBIT marker.
    // Direct `City::set_zone` only writes the zone plane, so reproduce the
    // saved gameplay state before exercising the load path.
    city.set_tile_bits(20, 20, TileBits::new(XBIT_CHOOSE_AND_PLACE));
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    city.save(&path).expect("save fixture city");

    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state
        .load_city(&path, (800, 600), 1.0)
        .expect("load saved city");
    state.no_disasters_enabled = true;
    set_city_no_disasters_enabled(&mut state.city, true);
    assert!(state.zone_growth_enabled);
    assert!(!state.zone_growth_uses_allowlist);
    assert!(state.zone_growth_can_touch_tile(20, 20));
    assert_eq!(
        state.city.zone_at(20, 20).map(|zone| zone.zone_id()),
        Some(ZONE_RESIDENTIAL_LIGHT)
    );
    assert_eq!(
        state.city.tile_bits_at(20, 20),
        Some(TileBits::new(XBIT_CHOOSE_AND_PLACE))
    );
    state.handle_simulation_month_advanced();
    assert!(state.zone_growth_enabled);
    assert!(!state.zone_growth_uses_allowlist);
    assert!(state.zone_growth_can_touch_tile(20, 20));
    let _ = std::fs::remove_file(path);
}

#[test]
fn new_city_starts_native_growth_scan_without_allowlist() {
    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));
    assert!(state.zone_growth_enabled);

    state.new_city().expect("new city");

    assert!(state.zone_growth_enabled);
    assert!(!state.zone_growth_uses_allowlist);
    assert!(
        state
            .zone_growth_allowed_tiles
            .iter()
            .all(|&allowed| !allowed)
    );
}

#[test]
fn manual_zoning_uses_native_win95_growth_without_an_allowlist() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.reset_zone_growth_for_city();
    state.tool = EditTool::ZoneResidential;

    assert_eq!(state.place_zone_at(20, 20, ZONE_RESIDENTIAL_LIGHT), 1);

    assert!(state.zone_growth_enabled);
    assert!(!state.zone_growth_uses_allowlist);
    assert!(state.zone_growth_can_touch_tile(20, 20));
}

#[test]
fn direct_building_over_player_zone_keeps_native_growth_scan_enabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.reset_zone_growth_for_city();
    state.tool = EditTool::ZoneResidential;
    assert_eq!(state.place_zone_at(20, 20, ZONE_RESIDENTIAL_LIGHT), 1);
    assert!(state.zone_growth_can_touch_tile(20, 20));

    state.tool = EditTool::SmallPark;
    assert!(state.apply_tool_at_tile(20, 20));

    assert_eq!(
        state.city.zone_at(20, 20).map(|zone| zone.zone_id()),
        Some(0)
    );
    assert!(state.zone_growth_can_touch_tile(20, 20));
    assert!(state.zone_growth_enabled);
}

#[test]
fn network_over_player_zone_keeps_native_growth_scan_enabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.reset_zone_growth_for_city();
    state.tool = EditTool::ZoneResidential;
    assert_eq!(state.place_zone_at(20, 20, ZONE_RESIDENTIAL_LIGHT), 1);
    assert!(state.zone_growth_can_touch_tile(20, 20));

    state.tool = EditTool::Road;
    assert!(state.apply_tool_at_tile(20, 20));

    assert_eq!(
        state.city.zone_at(20, 20).map(|zone| zone.zone_id()),
        Some(0)
    );
    assert!(state.zone_growth_can_touch_tile(20, 20));
    assert!(state.zone_growth_enabled);
}

#[test]
fn manually_zoned_served_tile_develops_with_native_growth_scan() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    stamp_transport_access_for_test(&mut city, 20, 19);
    power_adjacent_building_for_test(&mut city, 19, 20);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.reset_zone_growth_for_city();
    state.tool = EditTool::ZoneResidential;

    assert_eq!(state.place_zone_at(20, 20, ZONE_RESIDENTIAL_LIGHT), 1);
    state.handle_simulation_month_advanced();
    assert!(
        is_construction_building(state.city.building_at(20, 20)),
        "one monthly StartBuilding visit leaves the first-stage construction tile"
    );
}

#[test]
fn manual_dezoning_preserves_native_growth_and_pe_zone_byte_behavior() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    let mut state = test_state(city);
    state.reset_zone_growth_for_city();
    state.tool = EditTool::DeZone;

    assert_eq!(state.place_zone_at(20, 20, 0), 1);

    assert!(state.zone_growth_enabled);
    assert!(state.zone_growth_can_touch_tile(20, 20));
    assert_eq!(
        state.city.zone_at(20, 20).map(|zone| zone.zone_id()),
        Some(ZONE_RESIDENTIAL_LIGHT),
        "PE dezone preserves the stored XZON low nibble"
    );
}

#[test]
fn dezone_keeps_pipe_service_when_pe_preserves_the_zone_type_nibble() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.place_zone_at(11, 10, 1);
    state.city.set_tile_bits(
        11,
        10,
        crate::city::TileBits::new(0)
            .with_piped(true)
            .with_watered(true),
    );
    seed_saved_water_worker_bits_for_test(&mut state.city);

    state.place_zone_at(11, 10, 0);
    // Dezone no longer invent-clears watered/piped; network refresh is separate.
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
    state.refresh_simulation_service_bits();
    // The PE dezone branch leaves the XZON low nibble in place. The tile is
    // therefore still attached to this pipe, but an empty XBLD-0 zone does not
    // add a generic water-demand counter or receive a served bit by itself.
    assert_eq!(state.city.zone_at(11, 10).unwrap().zone_id(), 1);
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn bulldozed_residential_zone_keeps_connected_pipe_service() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.place_zone_at(11, 10, 1);
    state.city.set_building(11, 10, 112);
    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_simulation_service_bits();
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());

    state.bulldoze_at(11, 10);
    // PE DestroyBuilding keeps XZON type nibble (`&= 0x0f`).
    assert_eq!(state.city.zone_at(11, 10).unwrap().raw_bits(), 1);
    // PE DestroyStructure leaves XBIT watered/piped; network refresh is separate.
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
    state.refresh_simulation_service_bits();
    // Bulldozing removed the only generic water-demand tile. The zone and pipe
    // remain connected, but the empty XBLD-0 cell no longer receives service.
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn network_placement_refreshes_zone_pipe_service_bits() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.place_zone_at(11, 10, 1);
    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_simulation_service_bits();
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());

    assert_eq!(state.place_network_at(11, 10, NetworkKind::Road), 1);
    // PlaceRoads clears the XZON type nibble and preserves the corner nibble.
    assert_eq!(
        state.city.zone_at(11, 10).unwrap().raw_bits(),
        ZONE_CORNER_ALL
    );
    assert!(is_road_tile(state.city.building_at(11, 10)));
    // PE PlaceTile leaves XBIT unchanged; the empty zoned cell had no served bit.
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());
    state.refresh_simulation_service_bits();
    assert!(!state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn highway_placement_refreshes_zone_pipe_service_bits() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    for y in 10..=11 {
        for x in 11..=12 {
            state.place_zone_at(x, y, 1);
        }
    }
    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_simulation_service_bits();
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());
    // Odd click even-snaps to (10,10); building 220 is gate `0x8000` there.
    state.city.set_building(10, 10, 0);

    assert_eq!(state.place_highway_path(&[(11, 10)]), 1);
    for y in 10..=11 {
        for x in 11..=12 {
            assert_eq!(state.city.zone_at(x, y).unwrap().raw_bits(), 0xf0);
            assert!(is_highway_tile(state.city.building_at(x, y)));
            assert!(state.city.tile_bits_at(x, y).unwrap().piped());
            assert!(!state.city.tile_bits_at(x, y).unwrap().watered());
        }
    }
    state.refresh_simulation_service_bits();
    for y in 10..=11 {
        for x in 11..=12 {
            assert!(!state.city.tile_bits_at(x, y).unwrap().piped());
            assert!(!state.city.tile_bits_at(x, y).unwrap().watered());
        }
    }
}

#[test]
fn place_tile_military_census_runway_word1() {
    // PE @ `0x441fba`/`0x44209b`: XBLD `0xdd` (runway) → `0x4ca934` word 1.
    let mut city = City::new_blank("/tmp/sc2000-mil-runway.sc2", "MilRunway").expect("blank");
    city.set_zone(20, 20, 7);
    city.pe_military_tile_count[0] = 1;
    pe_place_tile_441f00(&mut city, 20, 20, 0xdd);
    assert_eq!(city.building_at(20, 20), 0xdd);
    assert_eq!(city.pe_dw_tile_count[0xdd], 0);
    assert_eq!(city.pe_military_tile_count[0], 0);
    assert_eq!(city.pe_military_tile_count[1], 1);
    pe_place_tile_441f00(&mut city, 20, 20, 0);
    assert_eq!(city.pe_military_tile_count[1], 0);
    assert_eq!(city.pe_military_tile_count[0], 1);
}

#[test]
fn place_tile_military_census_parking_word3() {
    // PE map: XBLD `0xef` (239) → case `0x0a` → word 3.
    let mut city = City::new_blank("/tmp/sc2000-mil-park.sc2", "MilPark").expect("blank");
    city.set_zone(20, 20, 7);
    city.pe_military_tile_count[0] = 1;
    pe_place_tile_441f00(&mut city, 20, 20, 0xef);
    assert_eq!(city.pe_military_tile_count[0], 0);
    assert_eq!(city.pe_military_tile_count[3], 1);
    assert_eq!(City::pe_military_tile_count_index(0xef), 3);
    assert_eq!(City::pe_military_tile_count_index(0xdd), 1);
    assert_eq!(City::pe_military_tile_count_index(0x10), 0);
}

#[test]
fn pe_city_value_building_hospital_div9_like_46a3a7() {
    let mut city = City::new_blank("/tmp/sc2000-cityval-hosp.sc2", "CityValHosp").expect("blank");
    city.pe_dw_tile_count[0xd1] = 18;
    assert_eq!(pe_city_value_building_contrib_46a34a(&city, 0xd1), 1000);
}

#[test]
fn pe_bond_issue_city_value_gate_allows_when_ratio_below_6_like_41a679() {
    // SC2KFix recalculates from placed values before the PE (bonds*2500)/(value+1) gate.
    let mut city = City::new_blank("/tmp/sc2000-bond-gate-ok.sc2", "BondGateOk").expect("blank");
    city.pe_dw_tile_count[0xc6] = 10; // 10 hydro tiles × $400 = $4,000
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 5); // 12500 / 4001 = 3
    assert!(pe_bond_issue_city_value_allows_41a679(&mut city));
    assert_eq!(city.pe_city_value, 4000);
}

#[test]
fn pe_bond_issue_city_value_gate_refuses_when_ratio_ge_6_like_41a6a5() {
    let mut city = City::new_blank("/tmp/sc2000-bond-gate-no.sc2", "BondGateNo").expect("blank");
    // Blank census → value 0; one bond → 2500 / 1 = 2500 >= 6.
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 1);
    assert!(!pe_bond_issue_city_value_allows_41a679(&mut city));
    assert!(!pe_bond_issue_city_value_would_allow_41a679(&city));
}

#[test]
fn mac_city_value_3a714_uses_saved_census_bands_and_building_costs() {
    const MAC_XBLD_CENSUS: usize = 0x01f0;
    let mut city =
        City::new_blank("/tmp/sc2000-mac-city-value-3a714.sc2", "MacCityValue").expect("blank");
    city.kind = CityFileKind::MacCity;
    let set_count = |city: &mut City, tile: u8, count: u32| {
        city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS + usize::from(tile) * 4, count);
    };

    // Native infrastructure bands: 2, 10, 25, 15, 100, 100, and 250.
    set_count(&mut city, 0x0e, 2);
    set_count(&mut city, 0x1d, 3);
    set_count(&mut city, 0x2c, 4);
    set_count(&mut city, 0x3f, 5);
    set_count(&mut city, 0x51, 6);
    set_count(&mut city, 0x61, 7);
    set_count(&mut city, 0x6c, 8);
    // Special buildings retain their own census shifts and per-ID costs.
    set_count(&mut city, 0xc6, 2); // 2 * 4,000
    set_count(&mut city, 0xc9, 31); // (31 >> 4) * 6,600
    set_count(&mut city, 0xd1, 18); // (18 / 9) * 500
    set_count(&mut city, 0xd7, 31); // (31 >> 4) * 3,000
    set_count(&mut city, 0xeb, 7); // (7 >> 2) * 250
    set_count(&mut city, 0xfb, 16); // (16 >> 4) * 100,000
    set_count(&mut city, 0xd0, 9); // deliberately unhandled City Hall row
    // The selected XUND class count at `+0xfe8` has a zero formula coefficient.
    city.set_mac_state_misc_u32_at(0x0fe8, 1234);

    let expected = 3_509 + 8_000 + 6_600 + 1_000 + 3_000 + 250 + 100_000;
    assert_eq!(mac_city_value_total_3a714(&city), expected);
}

#[test]
fn mac_bond_issue_uses_25000_gate_against_sc2kfix_pe_valuation() {
    const MAC_XBLD_CENSUS: usize = 0x01f0;
    let mut mac_city =
        City::new_blank("/tmp/sc2000-mac-bond-value-gate.sc2", "MacBondGate").expect("blank");
    mac_city.kind = CityFileKind::MacCity;
    mac_city.pe_dw_tile_count[0xc6] = 1;
    mac_city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS + 0xc6 * 4, 1);
    mac_city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 1);

    // Retail Mac values this hydro census at 4,000 and uses 25,000; SC2KFix
    // PE values it at its $400 placement cost and uses 2,500. Both exact ratios
    // are 6 here, so the strict gate refuses in both engines.
    assert!(!mac_bond_issue_city_value_allows_3a714(&mut mac_city));
    assert_eq!(mac_city.pe_city_value, 4_000);
    let mut pe_city = mac_city.clone();
    assert!(!pe_bond_issue_city_value_allows_41a679(&mut pe_city));
    assert_eq!(pe_city.pe_city_value, 400);

    // A larger Mac census lowers its ratio below the strict six-bond limit.
    mac_city.set_mac_state_misc_u32_at(MAC_XBLD_CENSUS + 0xc6 * 4, 2);
    assert!(mac_bond_issue_city_value_allows_3a714(&mut mac_city));
    assert_eq!(mac_city.pe_city_value, 8_000);
}

#[test]
fn mac_bond_issue_gate_compares_the_quotient_as_a_signed_word() {
    assert!(mac_bond_city_value_ratio_allows_issue_3a714(5));
    assert!(!mac_bond_city_value_ratio_allows_issue_3a714(6));
    assert!(!mac_bond_city_value_ratio_allows_issue_3a714(32_767));
    assert!(mac_bond_city_value_ratio_allows_issue_3a714(32_768));
    assert!(mac_bond_city_value_ratio_allows_issue_3a714(65_536));
    assert!(!mac_bond_city_value_ratio_allows_issue_3a714(65_542));
    assert_eq!(mac_bond_city_value_ratio_3a714(u32::MAX, 1), None);
}

#[test]
fn pe_bond_city_value_ratio_score_clamps_at_6_like_41d9b0() {
    // PE `0x41d9a7`: min((bonds * 25000) / (value + 1), 6).
    assert_eq!(pe_bond_city_value_ratio_score_from(0, 0), 0);
    assert_eq!(pe_bond_city_value_ratio_score_from(24_999, 1), 1);
    assert_eq!(pe_bond_city_value_ratio_score_from(4_999, 1), 5);
    assert_eq!(pe_bond_city_value_ratio_score_from(0, 1), 6);
    assert_eq!(pe_bond_city_value_ratio_score_from(4_000, 5), 6);
}

#[test]
fn pe_bond_city_value_ratio_score_recalculates_like_41d98e() {
    let mut city = City::new_blank("/tmp/sc2000-bond-score.sc2", "BondScore").expect("blank");
    city.pe_dw_tile_count[0xc6] = 10; // 10 hydro tiles × $400 = $4,000
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 1); // 25000 / 4001 = 6
    assert_eq!(pe_bond_city_value_ratio_score_41d9a7(&mut city), 6);
    assert_eq!(city.pe_city_value, 4000);
    assert_eq!(pe_bond_city_value_ratio_score_would_41d9a7(&city), 6);
}

#[test]
fn pe_bond_credit_rating_captions_match_4e6a78() {
    // PE `.data` string table @ `0x4e6a78`: AAA AA A B C D F.
    assert_eq!(pe_bond_credit_rating_caption_4e6a78(0), "AAA");
    assert_eq!(pe_bond_credit_rating_caption_4e6a78(1), "AA");
    assert_eq!(pe_bond_credit_rating_caption_4e6a78(2), "A");
    assert_eq!(pe_bond_credit_rating_caption_4e6a78(3), "B");
    assert_eq!(pe_bond_credit_rating_caption_4e6a78(4), "C");
    assert_eq!(pe_bond_credit_rating_caption_4e6a78(5), "D");
    assert_eq!(pe_bond_credit_rating_caption_4e6a78(6), "F");
    assert_eq!(pe_bond_credit_rating_caption_4e6a78(99), "F");
}

#[test]
fn city_load_hydrates_pe_dw_city_land_value_from_misc_430881() {
    // PE MISC→BSS @ `0x430881`: `[MISC+0x28]` → `[0x4ca440]`.
    let path = "/tmp/sc2000-landval-hydrate.sc2";
    let mut city = City::new_blank(path, "LandValHydrate").expect("blank");
    city.pe_dw_city_land_value = 0x00ab_cdef;
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 0x00ab_cdef);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_land_value, 0x00ab_cdef);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_city_land_value_to_misc_432eee() {
    // PE BSS→MISC @ `0x432eee`: `[0x4ca440]` → `[MISC+0x28]`.
    let path = "/tmp/sc2000-landval-persist.sc2";
    let mut city = City::new_blank(path, "LandValPersist").expect("blank");
    city.pe_dw_city_land_value = 42_000;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_LAND_VALUE_OFFSET), Some(42_000));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_land_value, 42_000);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_eq_month_pollution_param_prefers_bss_4cc910() {
    let mut city = City::new_blank("/tmp/sc2000-eq-poll-bss.sc2", "EqPoll").expect("blank");
    city.set_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, 11);
    city.pe_dw_city_pollution = 4_444;
    city.sync_pollution_density_from_slice(&vec![1; CITY_TILES]);
    assert_eq!(city.pollution_density_sum(), Some(16_384));
    assert_eq!(city.pe_eq_month_pollution_param_for_test(), 4_444);
}

#[test]
fn pe_attitude_land_value_fallback_reads_bss_4ca440() {
    let mut city = City::new_blank("/tmp/sc2000-lv-bss.sc2", "LvBss").expect("blank");
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 11);
    city.pe_dw_city_land_value = 22_000;
    let population = PopulationStats::default();
    let (_, _, land_value, _) = pe_attitude_demand_inputs_for_city(&city, population);
    assert_eq!(land_value, 22_000);
}

#[test]
fn pe_attitude_pollution_crime_fallback_reads_bss() {
    let mut city = City::new_blank("/tmp/sc2000-att-ctp-bss.sc2", "AttCtp").expect("blank");
    city.pe_dw_city_pollution = 5_555;
    city.pe_dw_city_crime = 6_666;
    let (_, pollution, _, crime) =
        pe_attitude_demand_inputs_for_city(&city, PopulationStats::default());
    assert_eq!(pollution, 5_555);
    assert_eq!(crime, 6_666);
}

#[test]
fn pe_dw_arcology_population_bss_4c94c4_hydrate_and_readers() {
    let path = "/tmp/sc2000-arco-bss.sc2";
    let mut city = City::new_blank(path, "ArcoBss").expect("blank");
    city.set_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 55_000);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_arcology_population(), 55_000);
    let mut city = City::new_blank("/tmp/sc2000-arco-bss2.sc2", "ArcoBss2").expect("blank");
    city.set_misc_u32_at(MISC_ARCOLOGY_POPULATION_OFFSET, 1);
    city.set_pe_dw_arcology_population(12_345);
    assert_eq!(city.pe_dw_arcology_population(), 12_345);
    assert_eq!(
        budget_tax_bases_for_city(&city, PopulationStats::default()),
        [12_345 / 6, 12_345 / 12, 12_345 / 12],
        "budget tax bases read BSS 0x4c94c4"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_w_industrial_mix_pollution_bonus_bss_4c9428_hydrate_and_readers() {
    let path = "/tmp/sc2000-mix-pol-bss.sc2";
    let mut city = City::new_blank(path, "MixPolBss").expect("blank");
    city.set_misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET, (-1_i32) as u32);
    assert_eq!(city.pe_w_industrial_mix_pollution_bonus(), -1);
    city.set_pe_w_industrial_mix_pollution_bonus(2);
    city.save(path).expect("save");
    let mut loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_industrial_mix_pollution_bonus(), 2);
    assert_eq!(
        loaded.misc_u32_at(MISC_INDUSTRIAL_MIX_POLLUTION_BONUS_OFFSET),
        Some(2)
    );
    assert_eq!(industrial_mix_pollution_bonus_for_blur(&loaded), 2);
    loaded.set_pe_w_sewer_bonus(1);
    assert_eq!(
        pollution_blur_divisor_base_for_city(&loaded),
        (1 - 2 + 4).max(1),
        "PE pollution blur @ 0x46a9b2 reads BSS 0x4c9428"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_rci_zone_balance_reads_bss_difficulty_4cb404() {
    let mut city = City::new_blank("/tmp/sc2000-rci-diff-bss.sc2", "RciDiff").expect("blank");
    city.set_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_EASY);
    city.pe_w_city_difficulty = CITY_DIFFICULTY_HARD as u16;
    city.pe_dw_residential_population = 100;
    let population = PopulationStats {
        industrial_population: 100,
        ..Default::default()
    };
    let hard_bss = pe_rci_zone_balance_scores(&city, population)[2];
    city.pe_w_city_difficulty = CITY_DIFFICULTY_EASY as u16;
    let easy_bss = pe_rci_zone_balance_scores(&city, population)[2];
    assert!(
        easy_bss > hard_bss,
        "PE FUN_00468150 @ 0x4682e9 reads BSS wCityDifficulty @ 0x4cb404"
    );
}

#[test]
fn pe_bond_issue_rate_uses_national_tax_word_plus_one() {
    let mut city = City::new_blank("/tmp/pe-bond-tax.sc2", "BondRate").unwrap();
    city.pe_w_national_tax = 3;
    city.pe_w_national_trend = 8;
    city.pe_city_value = 0;
    assert_eq!(bond_issue_rate_percent_for_city(&city), 4);
    city.pe_w_national_trend = 0;
    city.pe_city_value = 1_000_000;
    assert_eq!(
        bond_issue_rate_percent_for_city(&city),
        4,
        "trend and city value do not alter the PE issue rate"
    );
    city.pe_w_national_tax = 7;
    assert_eq!(bond_issue_rate_percent_for_city(&city), 8);
    city.pe_w_national_tax = u16::MAX;
    assert_eq!(
        bond_issue_rate_percent_for_city(&city),
        0,
        "INC AX wraps the WORD"
    );
}

#[test]
fn saved_city_service_outcomes_prefer_pe_city_value_bss() {
    let mut city = City::new_blank("/tmp/sc2000-svc-bss.sc2", "SvcBss").expect("blank");
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 66);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 77);
    city.set_misc_u32_at(MISC_CITY_VALUE_OFFSET, 11);
    city.pe_city_value = 55_000;
    let outcomes = saved_city_service_outcomes_for_city(&city);
    assert_eq!(outcomes.life_expectancy, 66);
    assert_eq!(outcomes.education_quotient, 77);
    assert_eq!(outcomes.city_value, 55_000);
}

#[test]
fn city_load_hydrates_pe_dw_residential_population_from_misc_431530() {
    // PE MISC→BSS @ `0x431530`: `[MISC+0x74]` → `[0x4ca194]`.
    let path = "/tmp/sc2000-respop-hydrate.sc2";
    let mut city = City::new_blank(path, "ResPopHydrate").expect("blank");
    city.pe_dw_residential_population = 12_345;
    city.set_misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET, 12_345);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_residential_population, 12_345);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_residential_population_to_misc_433c11() {
    // Engine mirrors BSS `0x4ca194` → MISC `+0x74` (PE save uses `*[0x4cc474]+4`).
    let path = "/tmp/sc2000-respop-persist.sc2";
    let mut city = City::new_blank(path, "ResPopPersist").expect("blank");
    city.pe_dw_residential_population = 9_001;
    city.save(path).expect("save");
    assert_eq!(
        city.misc_u32_at(MISC_RESIDENTIAL_POPULATION_OFFSET),
        Some(9_001)
    );
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_residential_population, 9_001);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_dw_national_population_from_misc_4314d6() {
    // PE MISC→BSS @ `0x4314d6`: `[MISC+0x50]` → `[0x4ca928]`.
    let path = "/tmp/sc2000-natpop-hydrate.sc2";
    let mut city = City::new_blank(path, "NatPopHydrate").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 4_200_000);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_national_population, 4_200_000);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_national_population_to_misc_433b78() {
    // PE save MISC mirror @ `0x433b78`: `[0x4ca928]` → `[MISC+0x50]`.
    let path = "/tmp/sc2000-natpop-persist.sc2";
    let mut city = City::new_blank(path, "NatPopPersist").expect("blank");
    city.pe_dw_national_population = 3_750_000;
    city.save(path).expect("save");
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(3_750_000)
    );
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_national_population, 3_750_000);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_w_national_tax_from_misc_4314e8() {
    // PE MISC→BSS @ `0x4314e8`: word `[MISC+0x58]` → `[0x4ca938]`.
    let path = "/tmp/sc2000-nattax-hydrate.sc2";
    let mut city = City::new_blank(path, "NatTaxHydrate").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 7);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_national_tax, 7);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_w_national_tax_to_misc_433b9d() {
    // PE save MISC mirror @ `0x433b9d`: word `[0x4ca938]` → `[MISC+0x58]`.
    let path = "/tmp/sc2000-nattax-persist.sc2";
    let mut city = City::new_blank(path, "NatTaxPersist").expect("blank");
    city.pe_w_national_tax = 9;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TAX_OFFSET), Some(9));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_national_tax, 9);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_i32_city_funds_from_misc_43144d() {
    // PE MISC→BSS @ `0x43144d`: `[MISC+0x14]` → `[0x4ca444]`.
    let path = "/tmp/sc2000-funds-hydrate.sc2";
    let mut city = City::new_blank(path, "FundsHydrate").expect("blank");
    city.set_city_funds(20_000);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_i32_city_funds, 20_000);
    assert_eq!(loaded.city_funds(), 20_000);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_i32_city_funds_to_misc_433aaa() {
    // PE save MISC mirror @ `0x433aaa`: `[0x4ca444]` → `[MISC+0x14]`.
    let path = "/tmp/sc2000-funds-persist.sc2";
    let mut city = City::new_blank(path, "FundsPersist").expect("blank");
    city.pe_i32_city_funds = -5_000;
    city.save(path).expect("save");
    assert_eq!(city.misc_i32_at(MISC_CITY_FUNDS_OFFSET), Some(-5_000));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_i32_city_funds, -5_000);
    assert_eq!(loaded.city_funds(), -5_000);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_dw_city_bonds_from_misc_431456() {
    // PE MISC→BSS @ `0x431456`: `[MISC+0x18]` → `[0x4cc4e8]`.
    let path = "/tmp/sc2000-bonds-hydrate.sc2";
    let mut city = City::new_blank(path, "BondsHydrate").expect("blank");
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 3);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_bonds, 3);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_city_bonds_to_misc_433ab8() {
    // PE save MISC mirror @ `0x433ab8`: `[0x4cc4e8]` → `[MISC+0x18]`.
    let path = "/tmp/sc2000-bonds-persist.sc2";
    let mut city = City::new_blank(path, "BondsPersist").expect("blank");
    city.pe_dw_city_bonds = 4;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(4));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_bonds, 4);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_dw_city_crime_pollution_from_misc_43088a() {
    // PE MISC→BSS crime `0x43088a` / pollution `0x43089c`.
    let path = "/tmp/sc2000-crime-poll-hydrate.sc2";
    let mut city = City::new_blank(path, "CrimePollHydrate").expect("blank");
    city.pe_dw_city_crime = 111;
    city.pe_dw_city_pollution = 222;
    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 111);
    city.set_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, 222);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_crime, 111);
    assert_eq!(loaded.pe_dw_city_pollution, 222);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_dw_city_crime_pollution_to_misc_432efc() {
    // PE BSS→MISC crime `0x432efc` / pollution `0x432f18`.
    let path = "/tmp/sc2000-crime-poll-persist.sc2";
    let mut city = City::new_blank(path, "CrimePollPersist").expect("blank");
    city.pe_dw_city_crime = 333;
    city.pe_dw_city_pollution = 444;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_CITY_CRIME_OFFSET), Some(333));
    assert_eq!(city.misc_u32_at(MISC_CITY_POLLUTION_OFFSET), Some(444));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_dw_city_crime, 333);
    assert_eq!(loaded.pe_dw_city_pollution, 444);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_load_hydrates_pe_city_crime_pollution_rates_from_misc_4308c9() {
    // PE MISC→BSS @ `0x4308c9`/`0x4308d2`: LE/EQ dwords → rate words.
    let path = "/tmp/sc2000-rate-hydrate.sc2";
    let mut city = City::new_blank(path, "RateHydrate").expect("blank");
    city.pe_city_pollution = 55;
    city.pe_city_crime = 66;
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 55);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 66);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_city_pollution(), 55);
    assert_eq!(loaded.pe_city_crime(), 66);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_city_crime_pollution_rates_to_misc_432f5e() {
    // PE BSS→MISC @ `0x432f5e`/`0x432f6c`.
    let path = "/tmp/sc2000-rate-persist.sc2";
    let mut city = City::new_blank(path, "RatePersist").expect("blank");
    city.pe_city_pollution = 71;
    city.pe_city_crime = 82;
    city.save(path).expect("save");
    assert_eq!(city.misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET), Some(71));
    assert_eq!(
        city.misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET),
        Some(82)
    );
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_city_pollution(), 71);
    assert_eq!(loaded.pe_city_crime(), 82);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_fiscal_notice_gates_follow_native_january_return() {
    for (month, funds, enabled, title) in [
        (12, 0, true, None),
        (12, 1, true, None),
        (12, -1, false, Some("Budget")),
        (6, -1, true, None),
    ] {
        let mut city = blank_city_layout_fixture("pe-fiscal-gates.sc2", "fiscal gates");
        city.set_city_funds(funds);
        city.set_saved_date_and_day(CityDate { year: 1900, month }, 24);
        set_city_auto_budget_enabled(&mut city, enabled);
        let mut state = test_state(city);
        state.no_disasters_enabled = true;
        state.game_clock = GameClock::new_with_day(1900, month, 24);
        state.traffic.set_speed(GameSpeed::Cheetah);
        state.advance_frame_simulation(Duration::from_millis(200));
        assert_eq!(state.city_info_dialog.map(|dialog| dialog.title()), title);
        if title.is_some() {
            state.close_city_info_dialog();
            state.advance_frame_simulation(Duration::ZERO);
            assert_eq!(
                state.city_info_dialog, None,
                "disabled Auto-Budget never raises the notice"
            );
        }
    }
    let mut city = blank_city_layout_fixture("mac-fiscal-gate.city", "Mac fidelity");
    city.set_city_funds(-1);
    set_mac_auto_budget_enabled(&mut city, true);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.open_pe_negative_auto_budget_notice();
    assert_eq!(
        state.city_info_dialog, None,
        "Mac's native fallback remains separate"
    );
    assert!(mac_auto_budget_enabled_for_city(&state.city));
}

#[test]
fn pe_fiscal_notice_live_january_waits_after_settlement_without_pausing_speed() {
    for path in ["pe-fiscal-notice.sc2", "pe-fiscal-notice.city"] {
        let mut city = blank_city_layout_fixture(path, "fiscal crisis");
        city.set_saved_date_and_day(
            CityDate {
                year: 1900,
                month: 12,
            },
            24,
        );
        city.set_city_funds(1);
        let row = budget_row_misc_offset_for_city(&city, 0, 0).unwrap();
        let mut values = vec![0; BUDGET_SERVICE_MISC_STRIDE];
        values[BUDGET_SERVICE_CURRENT_FIELD] = 90;
        values[BUDGET_SERVICE_FUNDING_FIELD] = 7;
        values[BUDGET_SERVICE_YEAR_TO_DATE_FIELD] = (-1800_i32) as u32;
        city.set_budget_misc_u32_values(row, &values);
        if city.kind.is_win95() {
            set_city_auto_budget_enabled(&mut city, true);
        } else {
            set_mac_auto_budget_enabled(&mut city, true);
        }
        let mut state = test_state(city);
        state.no_disasters_enabled = true;
        state.game_clock = GameClock::new_with_day(1900, 12, 24);
        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(200))
                .days,
            1
        );
        assert_eq!(
            state.city.city_funds(),
            -1,
            "notice follows settlement: {path}"
        );
        assert_eq!(
            state.city_info_dialog.map(|dialog| dialog.title()),
            Some("Fiscal Crisis")
        );
        assert!(!auto_budget_enabled_for_engine(
            &state.city,
            state.engine_fidelity
        ));
        assert!(!state.menu_checks.auto_budget);
        assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
        assert!(
            !state.pe_modal_active(),
            "MessageBox does not enter the speed-pause wrapper"
        );
        let annual = row + BUDGET_SERVICE_YEAR_TO_DATE_FIELD * 4;
        assert_eq!(
            state.city.budget_misc_u32_at(annual),
            Some(0),
            "rows wait behind the message"
        );
        let seed = state.pe_process_crt.seed();
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(1600))
                .days,
            0
        );
        assert_eq!(state.game_clock, GameClock::new(1901, 1));
        assert_eq!(state.pe_process_crt.seed(), seed);
        assert_eq!(state.city.budget_misc_u32_at(annual), Some(0));
        state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Turtle));
        assert_eq!(
            state.traffic.speed(),
            GameSpeed::Cheetah,
            "parent speed command is disabled"
        );
        state.close_city_info_dialog();
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(
            state.city.budget_misc_u32_at(annual),
            Some(630),
            "rows accrue exactly once after return"
        );
        assert_eq!(state.city.city_funds(), -1);
        assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(state.city.budget_misc_u32_at(annual), Some(630));
    }
}

#[test]
fn pe_auto_budget_toggle_preserves_all_financial_rows_and_process_rng() {
    for (path, fidelity) in [
        ("pe-auto-toggle.sc2", EngineFidelity::Win95),
        ("pe-auto-toggle.city", EngineFidelity::Win95),
        ("mac-auto-toggle.city", EngineFidelity::Mac68k),
    ] {
        let mut city = blank_city_layout_fixture(path, "Auto-Budget option");
        city.set_city_funds(-500);
        for index in 0..BUDGET_TOTAL_COUNT {
            let offset = budget_row_misc_offset_for_city(&city, index, 0).unwrap();
            let values: Vec<u32> = (0..BUDGET_SERVICE_MISC_STRIDE)
                .map(|field| 0xcafe_0000 | ((index as u32) << 8) | field as u32)
                .collect();
            city.set_budget_misc_u32_values(offset, &values);
        }
        if city.kind.is_win95() {
            set_city_auto_budget_enabled(&mut city, false);
        } else {
            set_mac_auto_budget_enabled(&mut city, false);
        }
        let mut state = test_state(city);
        state.engine_fidelity = fidelity;
        state.pe_process_crt = Win95CrtRand::new(0x12345678);
        let book = |city: &City| {
            (0..BUDGET_TOTAL_COUNT)
                .map(|index| {
                    let offset = budget_row_misc_offset_for_city(city, index, 0).unwrap();
                    city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE)
                })
                .collect::<Vec<_>>()
        };
        let before = book(&state.city);
        for enabled in [true, false] {
            state.toggle_auto_budget();
            assert_eq!(
                auto_budget_enabled_for_engine(&state.city, fidelity),
                enabled,
                "{path}"
            );
            assert_eq!(
                book(&state.city),
                before,
                "option toggle must not fund/rebuild/accrue rows: {path}"
            );
            assert_eq!(state.city.city_funds(), -500);
            assert_eq!(state.pe_process_crt.seed(), 0x12345678);
        }
    }
}

#[test]
fn pe_auto_budget_live_day_twenty_one_keeps_manual_funding() {
    for (path, funds) in [
        ("pe-auto-poor.sc2", -100_000),
        ("pe-auto-rich.sc2", 1_000_000),
    ] {
        let mut city = blank_city_layout_fixture(path, "manual service funding");
        stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
        set_budget_funding_for_test(&mut city, 0, 70);
        city.set_city_funds(funds);
        set_city_auto_budget_enabled(&mut city, true);
        city.set_saved_date_and_day(
            CityDate {
                year: 1900,
                month: 3,
            },
            20,
        );
        let mut state = test_state(city);
        state.no_disasters_enabled = true;
        state.game_clock = GameClock::new_with_day(1900, 3, 20);
        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(200))
                .days,
            1
        );
        assert_eq!(
            budget_service_funding_percent_for_city(&state.city, 0),
            70,
            "funds {funds}"
        );
    }
}

#[test]
fn pe_budget_tail_live_phase_zero_adopts_in_both_save_layouts() {
    for path in ["pe-budget-tail.sc2", "pe-budget-tail.city"] {
        for already_enabled in [false, true] {
            let mut city = blank_city_layout_fixture(path, "ordinance adoption");
            let bit = 1 << ORDINANCE_NUCLEAR_FREE_ZONE;
            let initial = 0x8000_0000 | if already_enabled { bit } else { 0 };
            if city.kind.is_win95() {
                city.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, initial);
            } else {
                city.set_mac_state_misc_u32_at(MAC_ORDINANCE_FLAGS_MISC_OFFSET, initial);
                city.set_mac_state_misc_u32_at(0x0fa8, 0xcafe_1234);
                for offset in (0x0ec8..0x0ee0).step_by(4) {
                    city.set_mac_state_misc_u32_at(offset, 0xbeef_0000 | offset as u32);
                }
            }
            city.set_city_funds(100_000);
            city.set_saved_date_and_day(
                CityDate {
                    year: 1900,
                    month: 2,
                },
                24,
            );
            let mut state = test_state(city);
            state.no_disasters_enabled = false;
            state.game_clock = GameClock::new_with_day(1900, 2, 24);
            state.traffic.set_speed(GameSpeed::Cheetah);
            state.pe_process_crt = Win95CrtRand::new(226);
            let tech_mask = state.city.pe_rebuild_tech_unlock_mask_46a0be(0);
            assert_ne!(tech_mask & 0x10, 0, "nuclear tech is initially available");

            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(200))
                    .days,
                1
            );
            assert_eq!(state.game_clock, GameClock::new(1900, 3));
            assert_eq!(state.pe_process_crt.seed(), 1_840_061_311, "{path}");
            assert_eq!(
                state.city.city_funds(),
                100_000,
                "adoption does not charge funds"
            );
            assert_eq!(
                state.city.pe_w_tech_unlock_mask(),
                tech_mask,
                "random adoption does not invoke the manual technology rebuild"
            );
            if state.city.kind.is_win95() {
                assert_eq!(
                    state.city.misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET),
                    Some(initial | bit)
                );
                assert_eq!(
                    state.city.pe_newspaper_story_id_at(0),
                    Some(0x29),
                    "an already-enabled choice still inserts a story"
                );
                assert_eq!(state.city.pe_newspaper_story_param_at(0), Some(17));
            } else {
                assert_eq!(
                    state
                        .city
                        .mac_state_misc_u32_at(MAC_ORDINANCE_FLAGS_MISC_OFFSET),
                    Some(initial | bit)
                );
                assert_eq!(state.city.mac_state_misc_u32_at(0x0fa8), Some(0xcafe_1234));
                for offset in (0x0ec8..0x0ee0).step_by(4) {
                    assert_eq!(
                        state.city.mac_state_misc_u32_at(offset),
                        Some(0xbeef_0000 | offset as u32),
                        "PE insertion must preserve the native Mac paper record"
                    );
                }
            }
        }
    }
}

#[test]
fn pe_budget_tail_new_cost_waits_for_the_next_monthly_rebuild() {
    let mut city = blank_city_layout_fixture("pe-budget-tail-cost.sc2", "adoption cost");
    city.set_city_funds(100_000);
    city.set_pe_zone_pops([100, 0, 0]);
    city.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, 0);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 3);
    state.no_disasters_enabled = false;
    state.pe_process_crt = Win95CrtRand::new(47); // Native third draw selects Free Clinics.
    // The ordinance rebuild consumes the saved current tax bases. Model the
    // preceding tax-base refresh so this fixture starts from the same native
    // state that the monthly calendar produces.
    state.refresh_tax_budget_rows(population_stats_for_city(&state.city));
    let current =
        budget_row_misc_offset(BUDGET_ORDINANCE_ROW, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
    state.refresh_monthly_budget_rows(population_stats_for_city(&state.city));
    assert_eq!(state.pe_process_crt.seed(), 2_729_863_776);
    assert!(ordinance_enabled_for_city(
        &state.city,
        ORDINANCE_FREE_CLINICS
    ));
    assert_eq!(
        state.city.budget_misc_u32_at(current),
        Some(0),
        "current rebuilt before adoption"
    );

    state.no_disasters_enabled = true;
    state.game_clock = GameClock::new(1900, 4);
    state.refresh_monthly_budget_rows(population_stats_for_city(&state.city));
    assert_eq!(
        state.city.budget_misc_u32_at(current),
        Some((-500_i32) as u32)
    );
    assert_eq!(state.city.city_funds(), 100_000);
}

#[test]
fn pe_idle_placement_drags_hold_calendar_and_resume_one_pending_step() {
    for path in ["pe-idle-drag.sc2", "pe-idle-drag.city"] {
        for drag in 0..7 {
            let mut city = blank_city_layout_fixture(path, "held input");
            city.set_city_funds(1_000_000);
            let mut state = test_state(city);
            state.no_disasters_enabled = true;
            state.traffic.set_speed(GameSpeed::Cheetah);
            let before = state.game_clock;
            match drag {
                0 => state.begin_network_drag(20, 20, NetworkKind::Road),
                1 => state.begin_network_drag(20, 20, NetworkKind::Rail),
                2 => state.begin_network_drag(20, 20, NetworkKind::Power),
                3 => state.begin_underground_drag(20, 20, UndergroundKind::Pipes),
                4 => state.begin_underground_drag(20, 20, UndergroundKind::Subway),
                5 => state.begin_highway_drag(20, 20),
                6 => state.begin_zone_drag(20, 20, 1),
                _ => unreachable!(),
            }
            for _ in 0..8 {
                assert_eq!(
                    state
                        .advance_frame_simulation(Duration::from_millis(200))
                        .days,
                    0,
                    "{path}, drag {drag}: input owns the idle consumer"
                );
            }
            assert_eq!(state.game_clock, before);
            state.cancel_network_drag();
            state.cancel_underground_drag();
            state.cancel_highway_drag();
            state.cancel_zone_drag();
            assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
            assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
            // Eight pulses elapsed while held. Phase was not paused/reset:
            // Turtle's next admitted pulse is 12, not the next frame.
            state.traffic.set_speed(GameSpeed::Turtle);
            for _ in 0..3 {
                assert_eq!(
                    state
                        .advance_frame_simulation(Duration::from_millis(200))
                        .days,
                    0
                );
            }
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(200))
                    .days,
                1
            );
        }
    }
}

#[test]
fn pe_idle_finished_network_stroke_releases_pending_calendar() {
    let mut city = blank_city_layout_fixture("pe-idle-finish.sc2", "finished input");
    city.set_city_funds(1_000_000);
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.traffic.set_speed(GameSpeed::Cheetah);
    state.begin_network_drag(20, 20, NetworkKind::Road);
    state.update_network_drag_to((22, 20));
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(400))
            .days,
        0
    );
    assert_eq!(
        state.finish_network_drag(),
        Some((NetworkKind::Road, 3, None, None))
    );
    for x in 20..=22 {
        assert!(NetworkKind::Road.contains(state.city.building_at(x, 20)));
    }
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
}

#[test]
fn pe_idle_calendar_rates_follow_native_pulses_in_both_city_layouts() {
    for path in ["/tmp/pe-idle-rates.sc2", "/tmp/pe-idle-rates.city"] {
        for (speed, expected) in [
            (GameSpeed::Turtle, 5),
            (GameSpeed::Llama, 10),
            (GameSpeed::Cheetah, 20),
        ] {
            let mut city = blank_city_layout_fixture(path, "pulse rate");
            city.set_city_funds(1_000_000);
            set_city_auto_budget_enabled(&mut city, true);
            let mut state = test_state(city);
            state.no_disasters_enabled = true;
            state.traffic.set_speed(speed);
            let mut days = 0;
            for _ in 0..20 {
                days += state
                    .advance_frame_simulation(Duration::from_millis(200))
                    .days;
            }
            assert_eq!(days, expected, "{path} {speed:?}");
            assert_eq!(state.traffic.speed(), speed);
        }
    }
}

#[test]
fn pe_idle_edit_map_does_not_advance_calendar() {
    let city = blank_city_layout_fixture("/tmp/pe-idle-editor.sc2", "editor");
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.traffic.set_speed(GameSpeed::Cheetah);
    let before = state.game_clock;
    assert_eq!(
        state.advance_frame_simulation(Duration::from_secs(1)).days,
        0
    );
    assert_eq!(state.game_clock, before);
}

#[test]
fn pe_idle_pause_resume_and_speed_changes_preserve_native_phase() {
    let city = blank_city_layout_fixture("/tmp/pe-idle-pause.sc2", "pause");
    let mut state = test_state(city);
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Llama));
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(200))
            .days,
        0
    );
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Turtle));
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(400))
            .days,
        0
    );
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Llama));
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(200))
            .days,
        1
    );
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Paused));
    let before = state.game_clock;
    let crt = state.pe_process_crt_seed();
    let lfsr = state.pe_process_lfsr_state();
    // Four more timer messages reach phase zero. Paused cannot consume the latch.
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(800))
            .days,
        0
    );
    assert_eq!(state.game_clock, before);
    assert_eq!(state.pe_process_crt_seed(), crt);
    assert_eq!(state.pe_process_lfsr_state(), lfsr);
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Llama));
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
}

#[test]
fn win95_industrial_xval_reads_primary_grid_from_adjacent_rows() {
    let mut city = City::new_blank("/tmp/win95-industrial-neighbors.sc2", "Xval").unwrap();
    city.set_zone(20, 24, ZONE_INDUSTRIAL_LIGHT);
    let water = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let zero = vec![0; CITY_TILES];
    let read = |city: &City| {
        live_land_value_density_map_for_city_at_center(
            city,
            &water,
            &zero,
            &zero,
            &zero,
            (64, 64),
            true,
        )[density_map_index(20, 24)]
    };
    let before = read(&city);
    city.set_building(16, 24, 0xd5); // Previous 32x32-grid row, primary +40.
    let after = read(&city);
    assert!(
        after > before,
        "adjacent primary row must contribute: {before} -> {after}"
    );
}

#[test]
fn win95_industrial_xval_uses_mixed_grid_rows_and_respects_edges() {
    let mut primary = vec![0_i16; 32 * 32];
    let mut industrial = vec![0_i16; 32 * 32];
    industrial[4 * 32 + 6] = 10;
    primary[3 * 32 + 6] = 20;
    primary[5 * 32 + 6] = 30;
    industrial[4 * 32 + 5] = 40;
    industrial[4 * 32 + 7] = 50;
    assert_eq!(
        pe_industrial_land_value_neighbor_average(&primary, &industrial, 4, 6, 32),
        30
    );
    industrial[0] = -5;
    primary[32] = -3;
    industrial[1] = -2;
    assert_eq!(
        pe_industrial_land_value_neighbor_average(&primary, &industrial, 0, 0, 32),
        -3
    );
}

#[test]
fn win95_xval_terrain_reuses_preblur_pollution_scratch() {
    let mut city = City::new_blank("/tmp/win95-xval-scratch.sc2", "Xval").unwrap();
    city.set_zone(20, 24, ZONE_COMMERCIAL_LIGHT);
    let water = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let zero = vec![0; CITY_TILES];
    let value = |city: &City| {
        live_land_value_density_map_for_city_at_center(
            city,
            &water,
            &zero,
            &zero,
            &zero,
            (64, 64),
            true,
        )[density_map_index(20, 24)]
    };
    let before = value(&city);
    city.set_traffic_density(10, 12, 200); // pollution WORD (5,6), reused as terrain center
    let after = value(&city);
    assert_eq!(
        after,
        before + 8,
        "200/5 enters one of five scratch neighbors"
    );
}

#[test]
fn crime_source_respects_edition_specific_preserved_xbit_marker() {
    let mut city = City::new_blank("/tmp/crime-marker.sc2", "Crime").unwrap();
    city.set_tile_bits(10, 10, TileBits::new(0x08));
    let population = vec![100_u8; CITY_TILES];
    let zero = vec![0_u8; CITY_TILES];
    let read = |city: &City, fidelity| {
        live_crime_density_map_for_city_with_fidelity(city, &population, &zero, &zero, fidelity)
            [density_map_index(20, 20)]
    };
    assert_eq!(
        read(&city, EngineFidelity::Win95),
        20,
        "PE retains this marker"
    );
    assert_eq!(
        read(&city, EngineFidelity::Mac68k),
        0,
        "Mac clears old markers"
    );
    city.set_building(10, 10, 0x70);
    assert_eq!(
        read(&city, EngineFidelity::Win95),
        0,
        "PE clears at a developed mask coordinate"
    );
    city.set_zone(21, 20, ZONE_RESIDENTIAL_LIGHT);
    assert_eq!(
        read(&city, EngineFidelity::Win95),
        20,
        "new source restores marker"
    );
    assert_eq!(
        read(&city, EngineFidelity::Mac68k),
        20,
        "Mac also projects the new source"
    );
}

#[test]
fn win95_xval_unzoned_cells_use_residential_population_bonus() {
    let mut city = City::new_blank("/tmp/win95-xval-unzoned.sc2", "Xval").unwrap();
    city.set_building(20, 24, 0x1d); // qualifies for XVAL without a zone
    let water = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let zero = vec![0; CITY_TILES];
    let value = |city: &City| {
        live_land_value_density_map_for_city_at_center(
            city,
            &water,
            &zero,
            &zero,
            &zero,
            (64, 64),
            true,
        )[density_map_index(20, 24)]
    };
    let unzoned = value(&city);
    city.set_zone(20, 24, ZONE_RESIDENTIAL_LIGHT);
    let residential = value(&city);
    assert_eq!(
        unzoned, residential,
        "PE default and residential use the same formula"
    );
    assert_eq!(land_value_population_adjustment(0, 63), 21);
    assert_eq!(land_value_population_adjustment(7, 63), 21);
    assert_eq!(land_value_population_adjustment(0, 64), 0);
}

#[test]
fn mac_land_value_total_is_latched_until_phase2_for_both_city_formats() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank("/tmp/mac-land-latch.sc2", "Mac Land").unwrap();
        city.kind = kind;
        city.set_mac_state_misc_u32_at(MISC_LAND_VALUE_OFFSET, 123_456);
        city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT);
        city.set_building(40, 40, RESIDENTIAL_1X1_BUILDINGS[0]);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.refresh_live_misc_current_values();
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_LAND_VALUE_OFFSET),
            Some(123_456),
            "MISC {kind:?}"
        );
        assert_eq!(
            state.city.pe_dw_city_land_value, 123_456,
            "shared cache {kind:?}"
        );
        let population = live_population_stats_for_city(&state.city);
        let outcomes = live_city_service_outcomes_for_city(&state.city);
        state.sim_calendar_day2_density(population, outcomes);
        let total = density_map_microsim64_sum(state.city.land_value_slice().unwrap());
        assert!(total > 0);
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_LAND_VALUE_OFFSET),
            Some(total),
            "MISC {kind:?}"
        );
        assert_eq!(
            state.city.pe_dw_city_land_value, total,
            "shared cache {kind:?}"
        );
        let path = std::env::temp_dir().join(format!(
            "citygame-mac-land-{kind:?}-{}.sc2",
            std::process::id()
        ));
        state.city.save(&path).expect("save phase-2 total");
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_LAND_VALUE_OFFSET),
            Some(total)
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn mac_environment_totals_latch_until_phase2_for_both_city_formats() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank("/tmp/mac-env-latch.sc2", "Mac Environment").unwrap();
        city.kind = kind;
        city.set_mac_state_misc_u32_at(MISC_CITY_CRIME_OFFSET, 123_456);
        city.set_mac_state_misc_u32_at(MISC_CITY_POLLUTION_OFFSET, 654_321);
        city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT);
        city.set_building(40, 40, RESIDENTIAL_1X1_BUILDINGS[0]);
        city.set_traffic_density(40, 40, 100);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.refresh_live_misc_current_values();
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_CRIME_OFFSET),
            Some(123_456),
            "crime {kind:?}"
        );
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_POLLUTION_OFFSET),
            Some(654_321),
            "pollution {kind:?}"
        );
        let pre_phase_path = std::env::temp_dir().join(format!(
            "citygame-mac-env-before-{kind:?}-{}.sc2",
            std::process::id()
        ));
        state
            .city
            .save(&pre_phase_path)
            .expect("save latched totals");
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_CRIME_OFFSET),
            Some(123_456),
            "pre-phase save crime {kind:?}"
        );
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_POLLUTION_OFFSET),
            Some(654_321),
            "pre-phase save pollution {kind:?}"
        );
        std::fs::remove_file(pre_phase_path).unwrap();
        let population = live_population_stats_for_city(&state.city);
        let outcomes = live_city_service_outcomes_for_city(&state.city);
        state.sim_calendar_day2_density(population, outcomes);
        let crime = density_map_microsim64_sum(state.city.crime_density_slice().unwrap());
        let pollution = density_map_microsim64_sum(state.city.pollution_density_slice().unwrap());
        assert!(pollution > 0);
        let path = std::env::temp_dir().join(format!(
            "citygame-mac-env-{kind:?}-{}.sc2",
            std::process::id()
        ));
        state.city.save(&path).expect("save phase-2 totals");
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_CRIME_OFFSET),
            Some(crime),
            "saved crime {kind:?}"
        );
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_POLLUTION_OFFSET),
            Some(pollution),
            "saved pollution {kind:?}"
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn xval_abandoned_selector_matches_both_original_class_tables() {
    // PE reads byte 0x4e7498 + XBLD at 0x46b0f1. Mac's
    // reconstructed A5 class table agrees over the 0x70..=0xc5 range.
    for tile in 0_u8..=u8::MAX {
        let expected = matches!(tile, 0x8a | 0x8b | 0xaa | 0xab | 0xac | 0xad | 0xc4 | 0xc5);
        assert_eq!(is_abandoned_building(tile), expected, "tile {tile:#04x}");
        assert_eq!(
            super::super::calendar::pe_growth_tile_class(tile) == 2,
            expected,
            "PE tile {tile:#04x}"
        );
        assert_eq!(
            super::super::budget::mac_growth_class_for_xbld(tile) == 2,
            expected,
            "Mac tile {tile:#04x}"
        );
    }
}

#[test]
fn mac_xval_general_arm_uses_one_fifth_pollution_and_threshold_only_population() {
    let mut city = City::new_blank("/tmp/mac-xval-general.sc2", "Mac Xval").unwrap();
    city.set_zone(40, 40, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(40, 40, 0x1d);
    let water = WaterServiceMaps {
        pipe_tiles: vec![false; CITY_TILES],
        watered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    let zero = vec![0_u8; CITY_TILES];
    let pollution = vec![100_u8; CITY_TILES];
    let population = vec![90_u8; CITY_TILES];
    let value = |pollution: &[u8], population: &[u8]| {
        live_land_value_density_map_for_city_at_center(
            &city,
            &water,
            pollution,
            &zero,
            population,
            (20, 20),
            false,
        )[density_map_index(40, 40)]
    };
    let baseline = value(&zero, &population);
    assert_eq!(
        baseline - value(&pollution, &population),
        20,
        "Mac general arm subtracts XPLT/5"
    );
    assert_eq!(
        value(&zero, &population),
        value(&zero, &vec![64_u8; CITY_TILES]),
        "XPOP above threshold has no general-arm additive term"
    );
    assert_eq!(value(&zero, &zero) - baseline, 21, "XPOP below 64 adds 21");
}

#[test]
fn mac_monthly_nature_sample_preserves_order_and_edge_revisit() {
    fn run(city: &mut City, words: &[u16]) {
        let mut draws = words.iter().copied();
        super::super::rci::mac_rci_nature_sample(city, || {
            draws.next().expect("unexpected random draw")
        });
        assert_eq!(
            draws.next(),
            None,
            "expected every supplied draw to be consumed"
        );
    }
    let mut city = City::new_blank("/tmp/mac-nature.sc2", "test").unwrap();
    city.set_building(0, 20, 6);
    run(&mut city, &[0, 20, 1]);
    assert_eq!(
        city.building_at(0, 20),
        8,
        "edge revisit ages the source twice"
    );
    city.set_building(10, 10, 5);
    run(&mut city, &[10, 10, 0, 0, 0]);
    assert_eq!(city.building_at(10, 10), 0);
    assert_eq!(city.building_at(11, 10), 6);
    city.set_building(20, 20, 13);
    run(&mut city, &[20, 20, 0]);
    assert_eq!(city.building_at(20, 20), 13);
    assert_eq!(city.building_at(21, 20), 6);
    city.set_building(30, 30, 14);
    run(&mut city, &[30, 30, 1]);
    assert_eq!(city.building_at(30, 30), 14);
    assert_eq!(city.building_at(31, 30), 0);
}

#[test]
fn mac_phase21_pre_weather_event_prefix_consumes_native_draw_count() {
    fn consumed(city: &mut City, words: &[u16]) -> usize {
        let mut draws = words.iter().copied();
        super::super::rci::mac_consume_phase21_pre_weather_event_draws(city, 2000, || {
            draws.next().expect("unexpected QuickDraw request")
        });
        words.len() - draws.len()
    }

    let mut city = City::new_blank("/tmp/mac-phase21-event-draws.sc2", "test").unwrap();
    city.kind = CityFileKind::MacCity;
    let words = [
        1, // selector remainder 1: no selector-zero gates
        0, 1, 2, 3, 4, 5, 6, 7, // eight unconditional alert draws
        8, 9,      // one history draw per field
        7,      // technology-date scan gate
        0xbeef, // sentinel proves the exact stopping position
    ];
    assert_eq!(consumed(&mut city, &words), 12);

    let selector_zero = [
        0, 1, 2, // selector plus both sequential optional draws
        3, 4, 5, 6, 7, 8, 9, 10, // eight unconditional alert draws
        11, 12, // history draws
        7,  // technology-date scan gate
        0xbeef,
    ];
    assert_eq!(consumed(&mut city, &selector_zero), 14);

    city.set_building(10, 10, 0xd7);
    assert_eq!(
        consumed(&mut city, &words),
        13,
        "a nonzero stadium census adds exactly one mask-choice draw"
    );
}

#[test]
fn mac_phase21_recovered_prefix_runs_before_weather_on_both_layouts() {
    const SEED: u32 = 8;
    let technology = Technology::ALL[0];
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-phase21-prefix-before-weather.sc2", "Phase 21")
            .expect("blank city");
        city.kind = kind;
        city.set_mac_state_misc_u32_at(technology.misc_offset(), 1900);

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(SEED);
        let mut expected_city = state.city.clone();
        let mut expected_rng = MacToolboxRng::new(SEED);
        super::super::rci::mac_rci_nature_sample(&mut expected_city, || {
            expected_rng.random_i16() as u16
        });
        let timer_result = super::super::rci::mac_consume_phase21_pre_weather_event_draws(
            &mut expected_city,
            state.game_clock.year(),
            || expected_rng.random_i16() as u16,
        );
        assert_eq!(timer_result, Some(0), "timer gate for {kind:?}");
        assert_eq!(
            expected_city.mac_state_misc_u32_at(technology.misc_offset()),
            Some(0),
            "the first due technology date is consumed before weather"
        );
        // export_237 makes exactly one further QuickDraw call for its weather
        // choice after the map and event prefix.
        expected_rng.random_i16();
        // export_238 consumes its first draw after weather. This seed skips
        // the national tax and trend rolls.
        assert_ne!(expected_rng.random_u16() % 10, 0);
        // The blank city's four empty neighbor rows consume no row draws.
        assert_ne!(expected_rng.random_u16() & 63, 0);

        state.sim_calendar_day21_demand_and_graphs();
        assert_eq!(
            state.mac_process_rng.seed(),
            expected_rng.seed(),
            "Mac export_237 through the neighbor crash gate for {kind:?}"
        );
        assert_eq!(
            state.city.mac_state_misc_u32_at(technology.misc_offset()),
            Some(0),
            "phase 21 clears the first due technology date for {kind:?}"
        );
    }
}

#[test]
fn mac_phase21_publishes_national_and_neighbor_economy_on_both_layouts() {
    const NEIGHBOR_TABLE: usize = 0x06d8;
    let initial_neighbors = [
        (0, 250_001),
        (1, 123_456),
        (5_000_001, 3_500_001),
        (5_000_000, 3_500_000),
    ];
    // Original CODE 8, 0x39124..0x39770, with the QuickDraw seed at the
    // end of the blank-map export_237 prefix. Each case starts at trend 0.
    let cases = [
        // The neighbors see the new tax and trend in the same month.
        (
            11,
            4,
            5,
            2,
            1_236_935_388,
            [
                (0, 250_001),
                (2, 123_044),
                (4_987_501, 3_511_667),
                (5_008_333, 3_485_416),
            ],
        ),
        // The crash selects a populated row.
        (
            662,
            4,
            4,
            0,
            1_775_648_490,
            [
                (0, 250_001),
                (2, 123_764),
                (3_746_876, 1_745_625),
                (5_004_166, 3_511_666),
            ],
        ),
        // The crash can also select an empty row with a saved value.
        (
            202,
            4,
            4,
            0,
            196_774_752,
            [
                (0, 125_000),
                (2, 123_353),
                (5_000_001, 3_500_001),
                (5_000_000, 3_511_666),
            ],
        ),
        // A negative SANE Comp result writes its low DWORD.
        (
            662,
            2_000,
            2_000,
            0,
            1_775_648_490,
            [
                (0, 250_001),
                (2, 4_294_885_713),
                (3_746_876, 4_656_459),
                (5_004_166, 4_292_657_296),
            ],
        ),
    ];
    for name in ["mac-national-phase21.city", "mac-national-phase21.sc2"] {
        for (seed, initial_tax, tax, trend, outgoing_seed, neighbors) in cases {
            let mut city = blank_city_layout_fixture(name, "National economy");
            let national = [
                (MISC_NATIONAL_POPULATION_OFFSET, 1_000_000, 1_000_000),
                (MISC_NATIONAL_VALUE_OFFSET, 650_000, 653_250),
                (MISC_NATIONAL_TAX_OFFSET, initial_tax, tax),
                (MISC_NATIONAL_TREND_OFFSET, 0, trend),
            ];
            for (offset, initial, _) in national {
                city.set_mac_state_misc_u32_at(offset, initial);
            }
            for (index, (population, value)) in initial_neighbors.into_iter().enumerate() {
                let base = NEIGHBOR_TABLE + index * 16;
                city.set_misc_u32_values(
                    base,
                    &[index as u32 + 1, population, value, 100 + index as u32],
                );
            }
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.mac_process_rng = MacToolboxRng::new(seed);
            state.sim_calendar_day21_demand_and_graphs();
            assert_eq!(
                state.mac_process_rng.seed(),
                outgoing_seed,
                "{name}, seed {seed}"
            );
            assert_eq!(u32::from(state.city.pe_w_national_tax), tax);
            assert_eq!(u32::from(state.city.pe_w_national_trend), trend);

            let path = std::env::temp_dir().join(format!("citygame-{}-{name}", std::process::id()));
            state.city.save(&path).unwrap();
            let reloaded = City::load(&path).unwrap();
            std::fs::remove_file(path).unwrap();
            assert_eq!(reloaded.kind, state.city.kind);
            for city in [&state.city, &reloaded] {
                for (offset, _, expected) in national {
                    assert_eq!(city.mac_state_misc_u32_at(offset), Some(expected));
                }
                for (index, (population, value)) in neighbors.into_iter().enumerate() {
                    let base = NEIGHBOR_TABLE + index * 16;
                    assert_eq!(
                        city.misc_u32_values(base, 4),
                        [index as u32 + 1, population, value, 100 + index as u32],
                        "{name}, seed {seed}, tax {initial_tax}, neighbor {index}"
                    );
                }
            }
        }
    }
}

#[test]
fn mac_phase21_technology_timer_scans_in_order_and_clears_first_due_date() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-phase21-tech-timer.sc2", "test").unwrap();
        city.kind = kind;
        for technology in Technology::ALL {
            city.set_mac_state_misc_u32_at(technology.misc_offset(), 0);
        }
        city.set_mac_state_misc_u32_at(Technology::ALL[0].misc_offset(), 2025);
        city.set_mac_state_misc_u32_at(Technology::ALL[1].misc_offset(), 1990);
        city.set_mac_state_misc_u32_at(Technology::ALL[3].misc_offset(), 1980);

        // selector, eight alerts, two histories, then a successful timer gate
        let mut draws = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].into_iter();
        let discovered =
            super::super::rci::mac_consume_phase21_pre_weather_event_draws(&mut city, 2000, || {
                draws.next().expect("exact native QuickDraw count")
            });

        assert_eq!(discovered, Some(1), "first due timer wins for {kind:?}");
        assert_eq!(
            city.mac_state_misc_u32_at(Technology::ALL[1].misc_offset()),
            Some(0),
            "clear the saved technology-date slot for {kind:?}"
        );
        assert_eq!(city.technology_discovery_year(Technology::ALL[1]), Some(0));
        assert_eq!(
            city.mac_state_misc_u32_at(Technology::ALL[3].misc_offset()),
            Some(1980),
            "stop scanning after clearing one due date for {kind:?}"
        );
        assert_eq!(draws.next(), None);
    }
}

#[test]
fn mac_monthly_nature_sample_matches_native_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_nature.csv");
    let mut city = City::new_blank("/tmp/mac-nature-replay.sc2", "test").unwrap();
    for (line, row) in csv.lines().enumerate().skip(1) {
        let fields: Vec<usize> = row.split(',').map(|value| value.parse().unwrap()).collect();
        let [
            x,
            y,
            initial,
            neighbor,
            sw,
            nw,
            radio,
            gate,
            direction,
            source_after,
            neighbor_after,
            expected_draws,
        ] = fields.as_slice()
        else {
            panic!("bad fixture line {line}")
        };
        let (x, y, initial, neighbor, sw, nw, radio, gate, direction) = (
            *x, *y, *initial, *neighbor, *sw, *nw, *radio, *gate, *direction,
        );
        let (nx, ny) = match direction {
            0 if x < 127 => (x + 1, y),
            1 if x > 0 => (x - 1, y),
            2 if y < 127 => (x, y + 1),
            3 if y > 0 => (x, y - 1),
            _ => (x, y),
        };
        city.set_building(x, y, initial as u8);
        city.set_tile_bits(x, y, TileBits::new(if sw != 0 { 4 } else { 0 }));
        if (nx, ny) != (x, y) {
            city.set_building(nx, ny, neighbor as u8);
            city.set_tile_bits(nx, ny, TileBits::new(if nw != 0 { 4 } else { 0 }));
        }
        let mut draws = vec![x as u16, y as u16];
        if initial == 5 {
            draws.push(radio as u16);
        }
        if sw == 0 && !(6..=13).contains(&initial) {
            draws.push(gate as u16);
        }
        if sw == 0 && ((6..=13).contains(&initial) || gate == 0) {
            draws.push(direction as u16);
        }
        let mut used = 0;
        super::super::rci::mac_rci_nature_sample(&mut city, || {
            let draw = draws[used];
            used += 1;
            draw
        });
        assert_eq!(used, *expected_draws, "draw count, fixture line {line}");
        assert_eq!(
            city.building_at(x, y) as usize,
            *source_after,
            "source, fixture line {line}"
        );
        assert_eq!(
            city.building_at(nx, ny) as usize,
            *neighbor_after,
            "neighbor, fixture line {line}"
        );
    }
}

#[test]
fn mac_monthly_nature_sample_census_matches_native_placement() {
    let csv = include_str!("../../../tests/fixtures/simulation/mac/mac_nature_placetile.csv");
    let mut city = City::new_blank("/tmp/mac-nature-census.city", "test").unwrap();
    city.kind = CityFileKind::MacCity;
    for (line, row) in csv.lines().enumerate().skip(1) {
        let fields: Vec<&str> = row.split(',').collect();
        assert_eq!(fields.len(), 13, "fixture line {line}");
        let n: Vec<usize> = fields[..12].iter().map(|v| v.parse().unwrap()).collect();
        let (x, y, initial, neighbor, sw, nw, radio, gate, dir) =
            (n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7], n[8]);
        let (nx, ny) = match dir {
            0 if x < 127 => (x + 1, y),
            1 if x > 0 => (x - 1, y),
            2 if y < 127 => (x, y + 1),
            3 if y > 0 => (x, y - 1),
            _ => (x, y),
        };
        city.set_building_without_mac_census(x, y, initial as u8);
        city.set_tile_bits(x, y, TileBits::new(if sw != 0 { 4 } else { 0 }));
        if (nx, ny) != (x, y) {
            city.set_building_without_mac_census(nx, ny, neighbor as u8);
            city.set_tile_bits(nx, ny, TileBits::new(if nw != 0 { 4 } else { 0 }));
        }
        for id in 0..15 {
            city.set_mac_state_misc_u32_at(0x01f0 + id * 4, 100);
        }
        let mut draws = vec![x as u16, y as u16];
        if initial == 5 {
            draws.push(radio as u16);
        }
        if sw == 0 && !(6..=13).contains(&initial) {
            draws.push(gate as u16);
        }
        if sw == 0 && ((6..=13).contains(&initial) || gate == 0) {
            draws.push(dir as u16);
        }
        let mut used = 0;
        super::super::rci::mac_rci_nature_sample(&mut city, || {
            let v = draws[used];
            used += 1;
            v
        });
        assert_eq!(used, n[11], "draws, line {line}");
        assert_eq!(
            usize::from(city.building_at(x, y)),
            n[9],
            "source, line {line}"
        );
        assert_eq!(
            usize::from(city.building_at(nx, ny)),
            n[10],
            "neighbor, line {line}"
        );
        for (id, value) in fields[12].split(';').enumerate() {
            assert_eq!(
                city.mac_xbld_tile_count(id as u8),
                value.parse::<u16>().unwrap(),
                "counter {id}, line {line}"
            );
        }
    }
}

#[test]
fn mac_phase21_runs_replayed_nature_sample_for_both_city_layouts() {
    for (name, expected_kind) in [
        ("mac-phase21-nature-win.sc2", CityFileKind::WinSc2),
        ("mac-phase21-nature-mac.city", CityFileKind::MacCity),
    ] {
        let mut city = blank_city_layout_fixture(name, "Nature");
        let seed = 0x1234_5678;
        let mut expected_rng = MacToolboxRng::new(seed);
        let x = usize::from(expected_rng.random_u16() & 0x7f);
        let y = usize::from(expected_rng.random_u16() & 0x7f);
        city.set_building(x, y, 6);

        let mut expected_city = city.clone();
        let mut expected_nature_rng = MacToolboxRng::new(seed);
        super::super::rci::mac_rci_nature_sample(&mut expected_city, || {
            expected_nature_rng.random_u16()
        });
        let expected_source = expected_city.building_at(x, y);
        let expected_cells: Vec<_> = (0..128)
            .flat_map(|sample_y| (0..128).map(move |sample_x| (sample_x, sample_y)))
            .filter(|&(sample_x, sample_y)| {
                city.building_at(sample_x, sample_y)
                    != expected_city.building_at(sample_x, sample_y)
            })
            .map(|(sample_x, sample_y)| {
                (
                    sample_x,
                    sample_y,
                    expected_city.building_at(sample_x, sample_y),
                )
            })
            .collect();
        assert_eq!(expected_source, 7);
        assert!(
            !expected_cells.is_empty(),
            "fixture must exercise a map write"
        );

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(seed);
        state.sim_calendar_day21_demand_and_graphs();

        assert_eq!(state.city.kind, expected_kind);
        for (sample_x, sample_y, expected) in expected_cells {
            assert_eq!(
                state.city.building_at(sample_x, sample_y),
                expected,
                "native nature sample at ({sample_x},{sample_y}) in {expected_kind:?}"
            );
        }
        assert_ne!(state.mac_process_rng.seed(), seed);
    }
}

#[test]
fn pe_nature_sample_matches_original_x86_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/win95/pe_nature.csv");
    let mut city = City::new_blank("/tmp/pe-nature-native.sc2", "test").unwrap();
    for (line, row) in csv.lines().enumerate().skip(1) {
        let n: Vec<i64> = row.split(',').map(|v| v.parse().unwrap()).collect();
        assert_eq!(n.len(), 13, "fixture line {line}");
        let (seed, x, y, initial, neighbor, sw, nw) = (
            n[0] as u32,
            n[1] as usize,
            n[2] as usize,
            n[3] as u8,
            n[4] as u8,
            n[5],
            n[6],
        );
        let neighbors = [
            (x as isize + 1, y as isize),
            (x as isize - 1, y as isize),
            (x as isize, y as isize + 1),
            (x as isize, y as isize - 1),
        ];
        for &(a, b) in &neighbors {
            if (0..128).contains(&a) && (0..128).contains(&b) {
                city.set_building(a as usize, b as usize, neighbor);
                city.set_tile_bits(
                    a as usize,
                    b as usize,
                    TileBits::new(if nw != 0 { 4 } else { 0 }),
                );
            }
        }
        city.set_building(x, y, initial);
        city.set_tile_bits(x, y, TileBits::new(if sw != 0 { 4 } else { 0 }));
        city.set_traffic_density(x, y, 0x37);
        let mut rng = Win95CrtRand::new(seed);
        pe_rci_demand_updates_nature_sample(&mut city, &mut rng);
        assert_eq!(
            i64::from(city.building_at(x, y)),
            n[7],
            "source, line {line}"
        );
        for (i, &(a, b)) in neighbors.iter().enumerate() {
            if (0..128).contains(&a) && (0..128).contains(&b) {
                assert_eq!(
                    i64::from(city.building_at(a as usize, b as usize)),
                    n[8 + i],
                    "neighbor {i}, line {line}"
                );
            } else {
                assert_eq!(n[8 + i], -1);
            }
        }
        assert_eq!(i64::from(rng.seed()), n[12], "CRT state, line {line}");
        assert_eq!(
            city.traffic_density_at(x, y),
            Some(0x37),
            "XTRF, line {line}"
        );
    }
}

#[test]
fn pe_issue_bond_stores_national_rate_and_wraps_funds() {
    for funds in [0, i32::MAX - 5_000] {
        let mut city = City::new_blank("/tmp/pe-live-bond.sc2", "Bond").unwrap();
        city.set_city_funds(funds);
        city.pe_w_national_tax = 3;
        city.pe_w_national_trend = 8;
        let mut state = test_state(city);
        assert!(state.issue_budget_bond());
        assert_eq!(state.city.misc_u32_at(BOND_DATA_MISC_OFFSET), Some(4));
        assert_eq!(state.city.city_funds(), funds.wrapping_add(10_000));
        assert_eq!(state.city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(1));
    }
}

#[test]
fn pe_bond_issue_gate_matches_original_x86_signed_word_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/win95/pe_bond_gate.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let values: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(values.len(), 3, "line {line}");
        assert_eq!(
            super::super::city_valuation::pe_bond_issue_gate_from(values[1] as i32, values[0]),
            Some(values[2] != 0),
            "line {line}"
        );
    }
    assert_eq!(
        super::super::city_valuation::pe_bond_issue_gate_from(-1, 1),
        None,
        "zero divisor remains fail-closed"
    );
}

#[test]
fn pe_bond_issue_action_uses_signed_low_word_gate() {
    let mut city = City::new_blank("/tmp/pe-bond-word-gate.sc2", "Bond").unwrap();
    city.set_misc_u32_at(BUDGET_BONDS_MISC_OFFSET, 14);
    city.pe_w_national_tax = 3;
    let mut state = test_state(city);
    assert!(state.budget_bond_action_enabled(true));
    assert!(
        state.issue_budget_bond(),
        "35000 has negative signed low WORD"
    );
    assert_eq!(state.city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(15));
}

#[test]
fn mac_issue_bond_wraps_funds_like_code4() {
    for funds in [0, i32::MAX - 5_000] {
        let mut city = City::new_blank("/tmp/mac-bond-funds.city", "MacBond").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_city_funds(funds);
        city.pe_w_national_tax = 3;
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        assert!(state.issue_budget_bond());
        assert_eq!(state.city.city_funds(), funds.wrapping_add(10_000));
        assert_eq!(state.city.misc_u32_at(BUDGET_BONDS_MISC_OFFSET), Some(1));
        assert_eq!(state.city.misc_u32_at(BOND_DATA_MISC_OFFSET), Some(4));
    }
}

#[test]
fn pe_yearly_attitude_preserves_published_totals_when_maps_differ() {
    for (pollution, land, crime) in [(0, 0, 0), (10, 20, 30)] {
        let mut city = City::new_blank("/tmp/pe-attitude-totals.sc2", "test").unwrap();
        city.sync_pollution_density_from_slice(&vec![1; CITY_TILES]);
        city.sync_land_value_from_slice(&vec![2; CITY_TILES]);
        city.sync_crime_density_from_slice(&vec![3; CITY_TILES]);
        city.pe_dw_city_pollution = pollution;
        city.pe_dw_city_land_value = land;
        city.pe_dw_city_crime = crime;
        let (_, actual_pollution, actual_land, actual_crime) =
            pe_attitude_demand_inputs_for_city(&city, PopulationStats::default());
        assert_eq!(
            (actual_pollution, actual_land, actual_crime),
            (pollution, land, crime)
        );
        let mut state = test_state(city);
        state.sim_calendar_yearly_updates();
        assert_eq!(
            (
                state.city.pe_dw_city_pollution,
                state.city.pe_dw_city_land_value,
                state.city.pe_dw_city_crime
            ),
            (pollution, land, crime),
            "yearly poll must not republish display-map sums"
        );
    }
}

#[test]
fn pe_monthly_graph_publishes_attitude_scores_from_saved_globals() {
    let mut city = City::new_blank("/tmp/pe-monthly-attitude.sc2", "test").unwrap();
    city.pe_w_city_developed_tiles = 4;
    city.pe_dw_city_pollution = 200;
    city.pe_dw_city_land_value = 60;
    city.pe_dw_city_crime = 40;
    city.set_pe_traffic_count(610);
    city.set_traffic_density(20, 20, 0x37);
    for (row, value) in [10, 20, 30].into_iter().enumerate() {
        let offset = budget_row_misc_offset(row, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
        city.set_budget_misc_u32_values(offset, &[value]);
    }
    for (row, value) in [(10, 30), (11, 40), (12, 51)] {
        let offset = budget_row_misc_offset(row, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
        city.set_budget_misc_u32_values(offset, &[value]);
    }
    let mut state = test_state(city);
    let population = PopulationStats::default();
    let outcomes = CityServiceOutcomes::default();
    let seed = state.pe_process_crt.seed();
    state.refresh_xgrp_history(population, outcomes);
    assert_eq!(
        (0..4)
            .map(|i| state.city.pe_attitude_score_at(i))
            .collect::<Vec<_>>(),
        [5, 100, 30, 20]
    );
    let history = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    assert_eq!(
        (4..8)
            .map(|series| history[series * XGRP_SAMPLES_PER_SERIES])
            .collect::<Vec<_>>(),
        [5, 100, 30, 20],
        "saved graph series must equal the live native score pointers"
    );
    assert_eq!(state.pe_process_crt.seed(), seed);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x37));
    state.city.pe_dw_city_pollution = 20;
    assert_eq!(
        state.city.pe_attitude_score_at(1),
        100,
        "score remains latched until the next graph update"
    );
    state.refresh_xgrp_history(population, outcomes);
    assert_eq!(state.city.pe_attitude_score_at(1), 10);
}

#[test]
fn pe_developed_tile_count_is_published_on_day2_and_latched_until_next_scan() {
    let mut city = City::new_blank("/tmp/pe-developed-cadence.sc2", "test").unwrap();
    city.set_zone(5, 5, ZONE_COMMERCIAL_LIGHT);
    city.set_building(7, 5, 0x1d);
    city.set_traffic_density(20, 20, 0x37);
    city.pe_w_city_developed_tiles = 9;
    let mut state = test_state(city);
    state.sim_calendar_day2_density(PopulationStats::default(), CityServiceOutcomes::default());
    assert_eq!(state.city.pe_w_city_developed_tiles, 2);
    state.city.set_building(8, 5, 0x1d);
    state.sim_calendar_yearly_updates();
    assert_eq!(
        state.city.pe_w_city_developed_tiles, 2,
        "January keeps the day-2 count"
    );
    state.sim_calendar_day2_density(PopulationStats::default(), CityServiceOutcomes::default());
    assert_eq!(state.city.pe_w_city_developed_tiles, 3);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x37));
}

#[test]
fn mac_density_phase_does_not_publish_pe_developed_tile_count() {
    let mut city = City::new_blank("/tmp/mac-developed-cadence.sc2", "test").unwrap();
    city.set_building(7, 5, 0x1d);
    city.set_misc_u32_at(0x10ac, 0x1234_5678);
    city.pe_w_city_developed_tiles = 9;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day2_density(PopulationStats::default(), CityServiceOutcomes::default());
    assert_eq!(state.city.pe_w_city_developed_tiles, 9);
    assert_eq!(state.city.mac_developed_cells(), Some(1));
    assert_eq!(state.city.misc_u32_at(0x10ac), Some(0x1234_5678));
    state.city.set_zone(8, 5, ZONE_COMMERCIAL_LIGHT);
    assert_eq!(state.city.mac_developed_cells(), Some(1));
    state.sim_calendar_day2_density(PopulationStats::default(), CityServiceOutcomes::default());
    assert_eq!(state.city.mac_developed_cells(), Some(2));
    assert_eq!(state.city.misc_u32_at(0x10ac), Some(0x1234_5678));
}

#[test]
fn mac_density_phase_latches_and_saves_native_developed_word() {
    let mut city = City::new_blank("/tmp/mac-native-developed.sc2", "test").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x10ac, 9);
    assert_eq!(city.mac_developed_cells(), Some(9));
    city.set_building(7, 5, 0x1d);
    city.set_zone(8, 5, ZONE_COMMERCIAL_LIGHT);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day2_density(PopulationStats::default(), CityServiceOutcomes::default());
    assert_eq!(state.city.mac_developed_cells(), Some(2));
    assert_eq!(state.city.mac_state_misc_u32_at(0x10ac), Some(2));
}

#[test]
fn pe_graph_cadence_follows_saved_age_even_when_display_date_differs() {
    let mut city = City::new_blank("/tmp/pe-graph-age.sc2", "Age").unwrap();
    let history = (1..=XGRP_VALUES as u32).collect::<Vec<_>>();
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &history);
    city.pe_dw_city_age = 150; // PE month 6, regardless of displayed January.
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1950, 1);
    state.refresh_xgrp_history(PopulationStats::default(), CityServiceOutcomes::default());
    let updated = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    assert_eq!(updated[XGRP_HALF_YEAR_OFFSET], updated[0]);
    assert_eq!(
        updated[XGRP_FIVE_YEAR_OFFSET],
        history[XGRP_FIVE_YEAR_OFFSET]
    );
    assert_eq!(
        updated[XGRP_FIVE_YEAR_OFFSET + 1],
        history[XGRP_FIVE_YEAR_OFFSET + 1]
    );
}

#[test]
fn pe_xgrp_population_matches_original_x86_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/win95/pe_graph_population.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<i64> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 13, "line {line}");
        let mut city = City::new_blank("/tmp/pe-graph-pop.sc2", "test").unwrap();
        city.set_pe_dw_arcology_population(v[1] as u32);
        city.set_pe_zone_pops([v[2] as i32, v[3] as i32, v[4] as i32]);
        for index in 0..4 {
            city.pe_dw_tile_count[251 + index] = v[5 + index] as u16;
        }
        let actual = crate::app::graph_history::pe_xgrp_population_values(&city, v[0] as i32);
        assert_eq!(
            actual,
            [v[9] as u32, v[10] as u32, v[11] as u32, v[12] as u32],
            "line {line}"
        );
    }
}

#[test]
fn pe_graph_population_uses_current_growth_slot_not_leftover_snapshot() {
    let mut city = City::new_blank("/tmp/pe-graph-growth.sc2", "test").unwrap();
    city.set_pe_dw_arcology_population(45);
    city.persist_pe_day22_population_leftover(9_999);
    city.set_traffic_density(20, 20, 0x37);
    let mut state = test_state(city);
    state.pe_growth_aggregate.add_xbld_candidate(1, 0x70);
    state.publish_pe_growth_aggregate();
    let slot0 = state.pe_growth_aggregate.slots()[0];
    state.city.persist_pe_day22_population_leftover(9_999);
    state.refresh_xgrp_history(PopulationStats::default(), CityServiceOutcomes::default());
    let history = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    assert_eq!(history[0], (slot0 as u32) * 10 + 45);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x37));
}

#[test]
fn pe_graph_service_values_match_original_x86_replay() {
    let csv = include_str!("../../../tests/fixtures/simulation/win95/pe_graph_service.csv");
    for (line, row) in csv.lines().enumerate().skip(1) {
        let v: Vec<u32> = row.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(v.len(), 4, "line {line}");
        let mut city = City::new_blank("/tmp/pe-graph-service.sc2", "test").unwrap();
        city.set_pe_power_used_percent(v[0]);
        city.set_pe_water_coverage_percent(v[1]);
        let mut state = test_state(city);
        state.refresh_xgrp_history(PopulationStats::default(), CityServiceOutcomes::default());
        let history = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
        assert_eq!(
            history[8 * XGRP_SAMPLES_PER_SERIES],
            v[2],
            "power line {line}"
        );
        assert_eq!(
            history[9 * XGRP_SAMPLES_PER_SERIES],
            v[3],
            "water line {line}"
        );
        assert_eq!(state.city.pe_water_coverage_percent(), v[1]);
        assert_eq!(state.city.pe_power_used_percent(), v[0]);
    }
}

#[test]
fn mac_phase21_growth_and_change_balances_match_original_68020() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_phase21_scalar.csv");
    for line in fixture.lines().skip(1) {
        let columns: Vec<_> = line.split(',').collect();
        let counts: Vec<u32> = columns[0].split(':').map(|v| v.parse().unwrap()).collect();
        let values: Vec<u32> = columns[1..].iter().map(|v| v.parse().unwrap()).collect();
        let mut city = City::new_blank("/tmp/mac-phase21-oracle.city", "oracle").unwrap();
        city.kind = CityFileKind::MacCity;
        for (index, value) in counts.into_iter().enumerate() {
            city.set_mac_rci_counter(index + 1, value);
        }
        city.set_mac_state_misc_u32_at(0x102c, values[0]);
        city.set_mac_state_misc_u32_at(0x0040, values[1]);
        city.mac_population_change_balances = [0x1234_5678, 0x8765_4321];
        mac_publish_monthly_growth_scalar(&mut city);
        assert_eq!(city.mac_rci_counter(0), values[2], "{line}");
        assert_eq!(
            city.mac_state_misc_u32_at(0x102c),
            Some(values[3]),
            "{line}"
        );
        assert_eq!(
            city.mac_population_change_balances,
            [values[4], values[5]],
            "{line}"
        );
        assert_eq!(
            city.mac_state_misc_u32_at(0x0040),
            Some(values[6]),
            "{line}"
        );
    }
}

#[test]
fn mac_day21_zero_growth_clears_saved_cohorts_and_preserves_summaries() {
    for (extension, expected_kind) in [
        ("city", CityFileKind::MacCity),
        ("sc2", CityFileKind::WinSc2),
    ] {
        let path = std::env::temp_dir().join(format!(
            "citygame-mac-zero-cohort-{}.{extension}",
            std::process::id()
        ));
        let mut city = blank_city_layout_fixture(&path, "zero growth");
        assert_eq!(city.kind, expected_kind);
        for i in 0..60 {
            city.set_mac_state_misc_u32_at(0x7c + 4 * i, i as u32 + 1);
        }
        for offset in [0x44, 0x48, 0x4c] {
            city.set_mac_state_misc_u32_at(offset, 55);
        }
        // Reload after writing the sentinels so the Win95 layout hydrates its
        // cached EQ/LE lanes from the saved values. Mac fidelity must clear
        // both those caches and the shared MISC lanes at phase 21.
        city.save(&path).unwrap();
        let mut city = City::load(&path).unwrap();
        assert_eq!(city.kind, expected_kind);
        if city.kind.is_win95() {
            city.prepare_mac_fidelity_from_saved_city();
        }

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.sim_calendar_day21_demand_and_graphs();
        for i in 0..60 {
            assert_eq!(
                state.city.mac_state_misc_u32_at(0x7c + 4 * i),
                Some(0),
                "{expected_kind:?} cohort lane {i} before save"
            );
        }
        for offset in [0x44, 0x48, 0x4c] {
            assert_eq!(
                state.city.mac_state_misc_u32_at(offset),
                Some(55),
                "{expected_kind:?} summary offset {offset:#x}"
            );
        }
        state.city.save(&path).unwrap();
        let loaded = City::load(&path).unwrap();
        assert_eq!(loaded.kind, expected_kind);
        for i in 0..60 {
            assert_eq!(
                loaded.mac_state_misc_u32_at(0x7c + 4 * i),
                Some(0),
                "{expected_kind:?} cohort lane {i} after save/reload"
            );
        }
        for offset in [0x44, 0x48, 0x4c] {
            assert_eq!(
                loaded.mac_state_misc_u32_at(offset),
                Some(55),
                "{expected_kind:?} summary offset {offset:#x} after save/reload"
            );
        }
        std::fs::remove_file(path).unwrap();
    }
}
