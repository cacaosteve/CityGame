use super::*;

#[test]
fn win95_new_city_seeds_native_fixed_microsim_rows() {
    let city = City::new_blank("/tmp/new-city-fixed-xmic.sc2", "Before").unwrap();
    let mut state = test_state(city);

    state.new_city().expect("Win95 new city");

    let expected_rows = [
        (236, None),
        (236, Some("Bus Depot")),
        (237, Some("Rail Depot")),
        (233, Some("Subway Station")),
        (200, Some("Wind Power")),
        (198, Some("Hydro Power")),
        (213, Some("SimPark System")),
        (212, Some("Museum")),
        (245, Some("Library")),
        (248, Some("Marina")),
    ];
    for (slot, (expected_tile, expected_label)) in expected_rows.into_iter().enumerate() {
        let row = state
            .city
            .microsim_at(slot)
            .unwrap_or_else(|| panic!("fixed XMIC row {slot}"));
        assert_eq!(row.tile_id, expected_tile, "fixed XMIC row {slot} tag");
        assert_eq!(
            (row.stat0, row.stat1, row.stat2, row.stat3),
            (0, 0, 0, 0),
            "fixed XMIC row {slot} reset state"
        );
        if let Some(expected_label) = expected_label {
            assert_eq!(
                state.city.label_at(MIN_SIM_TEXT_ENTRIES + slot as u8),
                Some(expected_label),
                "fixed XMIC row {slot} fallback label"
            );
        }
    }
    assert_eq!(
        state.city.microsim_at(10).map(|row| row.tile_id),
        Some(0),
        "ordinary structure rows remain free"
    );
}

#[test]
fn startup_newspaper_opens_after_named_generated_city_setup() {
    let city = City::new_blank("/tmp/startup-newspaper.sc2", "Before").unwrap();
    let mut state = test_state(city);
    state.menu_checks.citygame_newspaper = true;
    state
        .new_city_from_title(
            "Oak Bay".into(),
            20_000,
            1900,
            1,
            12345,
            title_screen::NewCityStartFlow::GenerateAndPlay,
        )
        .unwrap();
    assert_eq!(state.city.name, "Oak Bay");
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding))
    );
    assert!(!state.founding_newspaper_pending);
    state.close_city_info_dialog();
    state.maybe_show_founding_newspaper();
    assert_eq!(state.city_info_dialog, None, "welcome opens only once");
    state
        .new_city_from_title(
            "Cedar Bay".into(),
            20_000,
            1950,
            1,
            12345,
            title_screen::NewCityStartFlow::GenerateAndEdit,
        )
        .unwrap();
    assert_eq!(state.mode, AppMode::EditNewMap);
    assert_eq!(state.city_info_dialog, None);
    assert!(state.founding_newspaper_pending);
    state.finish_edit_map_state();
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding))
    );
    assert_eq!(state.city.name, "Cedar Bay");
}

#[test]
fn startup_newspaper_waits_for_editor_and_respects_suppression_without_save_writes() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/startup-newspaper-editor.sc2", "Oak Bay").unwrap();
        city.kind = kind;
        let mut state = test_state(city);
        state.menu_checks.citygame_newspaper = true;
        state.founding_newspaper_pending = true;
        state.mode = AppMode::EditNewMap;
        state.maybe_show_founding_newspaper();
        assert!(state.founding_newspaper_pending);
        assert_eq!(state.city_info_dialog, None);
        state.mode = AppMode::CityView;
        let before: Vec<_> = state.city.chunks.iter().map(|c| c.data.clone()).collect();
        let crt = state.pe_process_crt;
        state.maybe_show_founding_newspaper();
        assert_eq!(
            state.city_info_dialog,
            Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding))
        );
        assert_eq!(
            before,
            state
                .city
                .chunks
                .iter()
                .map(|c| c.data.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(crt, state.pe_process_crt);
        state.close_city_info_dialog();
        state.menu_checks.citygame_newspaper = false;
        state.founding_newspaper_pending = true;
        state.maybe_show_founding_newspaper();
        assert_eq!(state.city_info_dialog, None);
        assert!(!state.founding_newspaper_pending);
    }
}

#[test]
fn startup_status_panel_shows_tool_notice_and_weather_within_window_bounds() {
    let city = City::new_blank("/tmp/startup-status.sc2", "Oak Bay").unwrap();
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    state.refresh_idle_status_line();
    assert_eq!(state.status_panel_notice(), "Power Plant Needed");
    let title = state.window_title_base();
    assert!(!title.contains("engine="));
    assert!(!title.contains("anim"));
    assert!(!title.contains("Road"));
    assert!(title.contains("Oak Bay"));
    state.set_sim_status_announcement("Power shortage");
    assert_eq!(state.status_panel_notice(), "Power shortage");
    for size in [(320, 200), (1280, 800), (2624, 1644)] {
        for ps in [1.0, 2.0] {
            for dark in [false, true] {
                state.dark_appearance = dark;
                let mut solids = Vec::new();
                state.push_status_panel(size, ps, &mut solids);
                assert!(!solids.is_empty());
                for solid in solids {
                    assert!(solid.x >= 0.0 && solid.y >= 0.0);
                    assert!(solid.x + solid.w <= size.0 as f32 + 0.1);
                    assert!(solid.y + solid.h <= size.1 as f32 + 0.1);
                }
            }
        }
    }
}

#[test]
fn city_information_actions_map_to_the_recovered_dialogs() {
    let cases = [
        (UiAction::OpenCityMap, CityInfoDialog::Map),
        (UiAction::OpenCityPopulation, CityInfoDialog::Population),
        (UiAction::OpenCityNeighbors, CityInfoDialog::Neighbors),
        (UiAction::OpenCityGraphs, CityInfoDialog::Graphs),
        (UiAction::OpenCityIndustry, CityInfoDialog::Industry),
        (UiAction::OpenBudget, CityInfoDialog::Budget),
        (UiAction::OpenHelp, CityInfoDialog::Help),
    ];
    for (action, dialog) in cases {
        assert_eq!(CityInfoDialog::for_action(action), Some(dialog));
    }
    assert_eq!(CityInfoDialog::for_action(UiAction::ZoomIn), None);
}

#[test]
fn native_window_menu_commands_open_city_information_dialogs() {
    let cases = [
        (40, CityInfoDialog::Map),
        (41, CityInfoDialog::Budget),
        (42, CityInfoDialog::Population),
        (43, CityInfoDialog::Industry),
        (44, CityInfoDialog::Graphs),
        (45, CityInfoDialog::Neighbors),
        (46, CityInfoDialog::Ordinances),
        (47, CityInfoDialog::Scenario),
    ];

    for (command, dialog) in cases {
        assert_eq!(
            app_command_for_menu_command(command),
            Some(AppCommand::OpenCityInfo(dialog))
        );
    }
    assert_eq!(app_command_for_menu_command(48), None);
}

#[test]
fn native_file_menu_commands_cover_city_and_tile_set_actions() {
    let cases = [
        (1, AppCommand::OpenCityDialog),
        (2, AppCommand::ReloadAssets),
        (13, AppCommand::SaveCity),
        (14, AppCommand::SaveCityAs),
        (15, AppCommand::NewCity),
        (16, AppCommand::ToggleEditMap),
        (17, AppCommand::OpenTileSetDialog),
        (18, AppCommand::OpenScenarioDialog),
    ];
    for (command, app_command) in cases {
        assert_eq!(app_command_for_menu_command(command), Some(app_command));
    }
    assert_eq!(app_command_for_menu_command(19), None);
}

#[test]
fn native_speed_menu_commands_cover_game_speeds() {
    let cases = [
        (20, GameSpeed::Paused),
        (21, GameSpeed::Turtle),
        (22, GameSpeed::Llama),
        (23, GameSpeed::Cheetah),
        (24, GameSpeed::AfricanSwallow),
    ];
    for (command, speed) in cases {
        assert_eq!(
            app_command_for_menu_command(command),
            Some(AppCommand::SetSpeed(speed))
        );
    }
    assert_eq!(app_command_for_menu_command(25), None);
}

#[test]
fn native_newspaper_menu_commands_toggle_extra_and_subscription_and_open_courier() {
    assert_eq!(
        app_command_for_menu_command(70),
        Some(AppCommand::ToggleNewspaperSubscription)
    );
    assert_eq!(
        app_command_for_menu_command(71),
        Some(AppCommand::OpenCityInfo(CityInfoDialog::Newspaper(
            NewspaperEdition::Courier
        )))
    );
    assert_eq!(
        app_command_for_menu_command(72),
        Some(AppCommand::ToggleNewspaperExtra)
    );
    assert_eq!(app_command_for_menu_command(73), None);
}

#[test]
fn newspaper_courier_masthead_uses_pe_paper_choice_name() {
    assert_eq!(
        NewspaperEdition::Courier.masthead("Maxis Land", 0),
        "The Maxis Land Times"
    );
    assert_eq!(
        NewspaperEdition::Courier.masthead("Maxis Land", 2),
        "The Maxis Land Courier"
    );
    assert_eq!(
        NewspaperEdition::Courier.masthead("Maxis Land", 5),
        "The Maxis Land Journal"
    );
    assert_eq!(
        NewspaperEdition::Courier.masthead("Maxis Land", 99),
        "The Maxis Land Journal"
    );
    assert_eq!(
        NewspaperEdition::Extra.masthead("Maxis Land", 2),
        "Maxis Land Extra!!!"
    );
}

#[test]
fn newspaper_reward_stories_count_available_reward_bits() {
    let population = PopulationStats {
        total_population: 12_345,
        ..Default::default()
    };

    assert!(
        NewspaperEdition::Extra
            .story("Maxis Land", &population, 0, 0b0_0101)
            .contains("2 available mayoral rewards")
    );
    assert!(
        NewspaperEdition::Courier
            .story("Maxis Land", &population, 0, 0b0_1000)
            .contains("1 available reward")
    );
}

#[test]
fn native_help_menu_command_opens_the_help_dialog() {
    assert_eq!(
        app_command_for_menu_command(80),
        Some(AppCommand::OpenCityInfo(CityInfoDialog::Help))
    );
    assert_eq!(
        app_command_for_menu_command(81),
        Some(AppCommand::ToggleBalloonHelp)
    );
    assert_eq!(app_command_for_menu_command(82), None);
}

#[test]
fn native_options_menu_command_toggles_auto_budget() {
    let cases = [
        (50, AppCommand::ToggleAutoBudget),
        (51, AppCommand::ToggleAutoGoto),
        (52, AppCommand::ToggleSoundEffects),
        (53, AppCommand::ToggleMusic),
    ];
    for (command, app_command) in cases {
        assert_eq!(app_command_for_menu_command(command), Some(app_command));
    }
    assert_eq!(app_command_for_menu_command(54), None);
}

#[test]
fn state_app_commands_apply_runtime_side_effects_without_sdl() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.apply_state_app_command(&AppCommand::ZoomIn), None);

    assert_eq!(
        state.apply_state_app_command(&AppCommand::OpenCityInfo(CityInfoDialog::Ordinances)),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Ordinances));

    assert!(!state.show_grid);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleGrid),
        Some(AppCommandStateEffect::default())
    );
    assert!(state.show_grid);

    assert!(!state.show_zone_overlay);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleZoneOverlay),
        Some(AppCommandStateEffect::default())
    );
    assert!(state.show_zone_overlay);

    assert!(state.show_traffic);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleTraffic),
        Some(AppCommandStateEffect {
            log_traffic_stats: true,
            ..Default::default()
        })
    );
    assert!(!state.show_traffic);

    assert!(!state.show_underground);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleUnderground),
        Some(AppCommandStateEffect::default())
    );
    assert!(state.show_underground);
    assert!(!state.show_traffic);

    state.query_is_inspect = true;
    assert_eq!(
        state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Paused)),
        Some(AppCommandStateEffect {
            update_title: true,
            sync_renderer_to_window: true,
            sync_palette_stacking: true,
            ..Default::default()
        })
    );
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(1));
    assert!(!state.query_is_inspect);
    assert!(state.query_line.contains("*PAUSED*"));

    assert!(state.auto_goto_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleAutoGoto),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(!state.auto_goto_enabled);
    assert!(!auto_goto_enabled_for_city(&state.city));

    assert!(state.sound_effects_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleSoundEffects),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(!state.sound_effects_enabled);
    assert!(!sound_effects_enabled_for_city(&state.city));

    assert!(state.music_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleMusic),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(!state.music_enabled);
    assert!(!music_enabled_for_city(&state.city));

    assert!(!auto_budget_enabled_for_city(&state.city));
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleAutoBudget),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(auto_budget_enabled_for_city(&state.city));

    assert!(!state.balloon_help_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleBalloonHelp),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(state.balloon_help_enabled);

    assert_eq!(
        state.apply_state_app_command(&AppCommand::SetTool(EditTool::Road)),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert_eq!(state.tool, EditTool::Road);

    assert_eq!(
        state.apply_state_app_command(&AppCommand::RotateView(1)),
        Some(AppCommandStateEffect::default())
    );
    assert_eq!(state.city.rotation(), 1);

    state.city.set_building(24, 24, 0x70);
    state.hover_tile = Some((12, 34));
    assert_eq!(
        state.apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Fire)),
        Some(AppCommandStateEffect {
            update_title: true,
            placement_sound: Some(PlacementSound::Fire),
            disaster_log: Some((DisasterKind::Fire, true)),
            ..Default::default()
        })
    );
    assert!(state.city.has_active_disaster());

    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleNoDisasters),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(state.no_disasters_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Flood)),
        Some(AppCommandStateEffect {
            disaster_log: Some((DisasterKind::Flood, false)),
            ..Default::default()
        })
    );
}

#[test]
fn city_information_dialogs_toggle_and_close_from_the_original_close_button() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_city_info_dialog(CityInfoDialog::Map);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Map));
    assert!(
        state
            .checked_palette_actions()
            .contains(&UiAction::OpenCityMap)
    );
    state.open_city_info_dialog(CityInfoDialog::Graphs);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Graphs));
    assert!(
        state
            .checked_palette_actions()
            .contains(&UiAction::OpenCityGraphs)
    );
    state.open_city_info_dialog(CityInfoDialog::Graphs);
    assert_eq!(state.city_info_dialog, None);

    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let close = city_info_close_rect(dialog);
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(close.x + close.w * 0.5, close.y + close.h * 0.5),
            window_size,
            CityInfoDialog::Map,
        ),
        Some(CityInfoDialogAction::Close)
    );
}

#[test]
fn newspaper_close_button_tracks_the_scaled_page_at_all_window_sizes() {
    let dialog = CityInfoDialog::Newspaper(NewspaperEdition::Courier);
    for window in [(400, 360), (1024, 768), (2624, 1644)] {
        let page = city_info_dialog_rect_for(dialog, window);
        let close = newspaper::close_rect(page);
        let center = Vec2::new(close.x + close.w * 0.5, close.y + close.h * 0.5);
        assert!(point_in_float_rect(center, page));
        assert_eq!(
            city_info_dialog_hit_test(center, window, dialog),
            Some(CityInfoDialogAction::Close)
        );
        assert_eq!(
            city_info_dialog_hit_test(
                Vec2::new(page.x + 5.0, page.y + page.h * 0.5),
                window,
                dialog
            ),
            None
        );
    }
}

#[test]
fn ordinances_dialog_hit_tests_rows_only_for_ordinances() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let row = ordinance_row_rect(dialog, 3);
    let point = Vec2::new(row.x + 14.0, row.y + 14.0);

    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Ordinances),
        Some(CityInfoDialogAction::ToggleOrdinance(3))
    );
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Budget),
        None
    );
}

#[test]
fn graphs_dialog_hit_tests_legend_toggles_only_for_graphs() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let legend = graph_legend_rect(dialog, 5);
    let point = Vec2::new(legend.x + 8.0, legend.y + 8.0);

    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Graphs),
        Some(CityInfoDialogAction::ToggleGraphSeries(5))
    );
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Map),
        None
    );
}

#[test]
fn graphs_dialog_hit_tests_scale_radios_only_for_graphs() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let ten_year = graph_scale_rect(dialog, 1);
    let point = Vec2::new(ten_year.x + 8.0, ten_year.y + 8.0);

    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Graphs),
        Some(CityInfoDialogAction::SetGraphScale(GraphScale::Years10))
    );
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Map),
        None
    );
    let hundred_year = graph_scale_rect(dialog, 2);
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(hundred_year.x + 8.0, hundred_year.y + 8.0),
            window_size,
            CityInfoDialog::Graphs
        ),
        Some(CityInfoDialogAction::SetGraphScale(GraphScale::Years100))
    );
}

#[test]
fn budget_dialog_hit_tests_tax_and_service_controls_only_for_budget() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect_for(CityInfoDialog::Budget, window_size);
    let tax = budget_tax_adjust_rect(dialog, 1, true);
    let service = budget_service_funding_adjust_rect(dialog, 7, false);
    let auto_budget = budget_auto_budget_rect(dialog);
    let issue_bond = budget_bond_button_rect(dialog, true);
    let repay_bond = budget_bond_button_rect(dialog, false);

    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(tax.x + 8.0, tax.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::AdjustBudgetTax {
            tax_index: 1,
            delta: BUDGET_TAX_STEP
        })
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(service.x + 8.0, service.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::AdjustBudgetFunding {
            service_index: 7,
            delta: -BUDGET_FUNDING_STEP
        })
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(auto_budget.x + 8.0, auto_budget.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::ToggleAutoBudget)
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(issue_bond.x + 8.0, issue_bond.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::IssueBond)
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(repay_bond.x + 8.0, repay_bond.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::RepayBond)
    );
    // Different dialogs can place their own controls at the same screen point.
    // The ordinances dialog must never dispatch a budget-tax action there.
    assert!(!matches!(
        city_info_dialog_hit_test(
            Vec2::new(tax.x + 8.0, tax.y + 8.0),
            window_size,
            CityInfoDialog::Ordinances
        ),
        Some(CityInfoDialogAction::AdjustBudgetTax { .. })
    ));
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(auto_budget.x + 8.0, auto_budget.y + 8.0),
            window_size,
            CityInfoDialog::Map
        ),
        None
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(issue_bond.x + 8.0, issue_bond.y + 8.0),
            window_size,
            CityInfoDialog::Population
        ),
        None
    );
}

#[test]
fn industry_dialog_hit_tests_tax_controls_only_for_industry() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let tax = industry_tax_adjust_rect(dialog, 4, false);

    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(tax.x + 8.0, tax.y + 8.0),
            window_size,
            CityInfoDialog::Industry
        ),
        Some(CityInfoDialogAction::AdjustIndustryTax {
            industry_index: 4,
            delta: -BUDGET_TAX_STEP
        })
    );
    // This screen point overlaps a Budget service button in Budget's own,
    // independently scaled layout. Hit testing must follow the active dialog.
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(tax.x + 8.0, tax.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::AdjustBudgetFunding {
            service_index: 1,
            delta: BUDGET_FUNDING_STEP
        })
    );
}

#[test]
fn newspaper_subscription_and_extra_read_and_toggle_saved_misc_flags() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!newspaper_subscription_enabled_for_city(&city));
    assert!(newspaper_extra_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );

    ensure_default_city_options(&mut city);
    assert!(!newspaper_subscription_enabled_for_city(&city));
    assert!(newspaper_extra_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert!(!set_city_newspaper_extra_enabled(&mut city, true));

    assert!(set_city_newspaper_subscription_enabled(&mut city, true));
    assert!(newspaper_subscription_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert!(!set_city_newspaper_subscription_enabled(&mut city, true));
    assert!(newspaper_extra_enabled_for_city(&city));

    assert!(set_city_newspaper_subscription_enabled(&mut city, false));
    assert!(!newspaper_subscription_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert!(newspaper_extra_enabled_for_city(&city));

    assert!(set_city_newspaper_extra_enabled(&mut city, false));
    assert!(!newspaper_extra_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
}

#[test]
fn city_state_toggles_newspaper_subscription_and_extra_saved_flags() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert!(!newspaper_subscription_enabled_for_city(&state.city));
    assert!(newspaper_extra_enabled_for_city(&state.city));

    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleNewspaperSubscription),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(newspaper_subscription_enabled_for_city(&state.city));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert!(newspaper_extra_enabled_for_city(&state.city));

    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleNewspaperExtra),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(!newspaper_extra_enabled_for_city(&state.city));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );

    state.toggle_newspaper_subscription();
    state.toggle_newspaper_extra();
    assert!(!newspaper_subscription_enabled_for_city(&state.city));
    assert!(newspaper_extra_enabled_for_city(&state.city));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
}

#[test]
fn mac_special_zone_single_tile_helper_preserves_acceptance_and_map_writes() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-special-single.sc2", "SpecialSingle").expect("blank city");
    city.kind = CityFileKind::MacCity;

    assert_eq!(
        mac_budget_try_place_zone_single_tile(&mut city, 40, 40, 8, 0xe1),
        MacSpecialTileAttempt::Refused
    );
    city.set_tile_bits(40, 40, TileBits::new(0x5b));
    assert_eq!(
        mac_budget_try_place_zone_single_tile(&mut city, 40, 40, 8, 0xe1),
        MacSpecialTileAttempt::Placed
    );
    assert_eq!(city.building_at(40, 40), 0xe1);
    assert_eq!(city.zone_at(40, 40).unwrap().raw_bits(), 0xf8);
    assert_eq!(city.tile_bits_at(40, 40).unwrap().raw_bits(), 0xfb);
    assert_eq!(city.mac_xbld_tile_count(0xe1), 1);

    // At or above 0x0d the original helper accepts but leaves all map planes
    // untouched. It does not report a failed placement to the caller.
    city.set_tile_bits(42, 40, TileBits::new(0x40));
    city.set_building(42, 40, 0x0d);
    city.set_zone(42, 40, 0x08);
    assert_eq!(
        mac_budget_try_place_zone_single_tile(&mut city, 42, 40, 8, 0xea),
        MacSpecialTileAttempt::AcceptedWithoutPlacement
    );
    assert_eq!(city.building_at(42, 40), 0x0d);
    assert_eq!(city.zone_at(42, 40).unwrap().raw_bits(), 0x08);
    assert_eq!(city.tile_bits_at(42, 40).unwrap().raw_bits(), 0x40);
}

#[test]
fn mac_special_zone_square_helper_clears_and_stamps_exact_2x2_planes() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-special-square.sc2", "SpecialSquare").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.saved_rotation = 0;
    city.set_view_rotation(1);
    for (x, y) in [(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_zone(x, y, 0x28);
        city.set_tile_bits(x, y, TileBits::new(0x5b));
    }
    // A high ordinary building in the NW cell makes FUN_0003844a clear its
    // aligned 2x2 footprint before the new marker/building writes.
    city.set_building(40, 40, 0xc6);
    city.set_building(41, 40, 0x03);
    city.set_building(40, 41, 0x04);
    city.set_building(41, 41, 0x05);

    assert_eq!(
        mac_budget_try_place_zone_square(&mut city, 41, 41, 8, 0xf2),
        MacSpecialTileAttempt::Placed
    );
    for (x, y, marker) in [
        (40, 40, 0x10),
        (41, 40, 0x20),
        (40, 41, 0x80),
        (41, 41, 0x40),
    ] {
        assert_eq!(city.building_at(x, y), 0xf2);
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), marker | 0x08);
        assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits(), 0xfb);
    }
    assert_eq!(city.mac_xbld_tile_count(0xc6), 0);
    assert_eq!(city.mac_xbld_tile_count(0xf2), 4);

    for (rotation, markers) in [
        [0x10, 0x20, 0x40, 0x80],
        [0x20, 0x40, 0x80, 0x10],
        [0x40, 0x80, 0x10, 0x20],
        [0x80, 0x10, 0x20, 0x40],
    ]
    .into_iter()
    .enumerate()
    {
        city.saved_rotation = rotation as u8;
        city.set_view_rotation((rotation as u8 + 1) & 3);
        for (x, y) in [(40, 40), (41, 40), (40, 41), (41, 41)] {
            city.set_building(x, y, 0);
        }
        assert_eq!(
            mac_budget_try_place_zone_square(&mut city, 41, 41, 8, 0xf2),
            MacSpecialTileAttempt::Placed
        );
        for (x, y, marker) in [
            (40, 40, markers[0]),
            (41, 40, markers[1]),
            (41, 41, markers[2]),
            (40, 41, markers[3]),
        ] {
            assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), marker | 0x08);
        }
    }

    // The special ray sentinel rejects the whole footprint without writes.
    city.set_building(41, 41, 0xe0);
    assert_eq!(
        mac_budget_try_place_zone_square(&mut city, 40, 40, 8, 0xee),
        MacSpecialTileAttempt::Refused
    );
    assert_eq!(city.building_at(40, 40), 0xf2);
    assert_eq!(city.building_at(41, 41), 0xe0);
}

#[test]
fn mac_special_zone_square_helper_refuses_each_rooted_admission_failure_without_writes() {
    for failure in [
        "no power",
        "zone mismatch",
        "northwest limit",
        "ray sentinel",
    ] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-special-square-admission.sc2",
            "SquareAdmission",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        for (x, y) in [(40, 40), (41, 40), (40, 41), (41, 41)] {
            city.set_zone(x, y, 8);
            city.set_tile_bits(x, y, TileBits::new(0x1b));
        }
        if failure != "no power" {
            city.set_tile_bits(42, 41, TileBits::new(0x40));
        }
        match failure {
            "zone mismatch" => city.set_zone(41, 41, 9),
            "northwest limit" => city.set_building(40, 40, 0xeb),
            "ray sentinel" => city.set_building(40, 41, 0xe0),
            _ => {}
        }
        let before = [(40, 40), (41, 40), (40, 41), (41, 41)].map(|(x, y)| {
            (
                city.building_at(x, y),
                city.zone_at(x, y).unwrap().raw_bits(),
                city.tile_bits_at(x, y).unwrap().raw_bits(),
            )
        });

        assert_eq!(
            mac_budget_try_place_zone_square(&mut city, 41, 41, 8, 0xf6),
            MacSpecialTileAttempt::Refused,
            "{failure}"
        );
        let after = [(40, 40), (41, 40), (40, 41), (41, 41)].map(|(x, y)| {
            (
                city.building_at(x, y),
                city.zone_at(x, y).unwrap().raw_bits(),
                city.tile_bits_at(x, y).unwrap().raw_bits(),
            )
        });
        assert_eq!(after, before, "{failure} must be an all-or-nothing refusal");
    }
}

#[test]
fn mac_special_zone_runway_helper_extends_existing_markers_until_five_new_tiles() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-special-runway.sc2", "SpecialRunway").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.saved_rotation = 0;
    city.set_view_rotation(1);
    let (x, y) = (41, 40); // odd x selects the positive vertical ray
    for ray_y in y..=y + 5 {
        city.set_zone(x, ray_y, 0x08);
    }
    city.set_tile_bits(x, y, TileBits::new(0x40)); // powered cardinal gate
    city.set_building(x, y + 1, 0xdd);
    city.set_tile_bits(x, y + 1, TileBits::new(0x40)); // opposite orientation marker

    assert_eq!(
        mac_budget_try_place_zone_runway_ray(&mut city, x, y, 8),
        MacSpecialTileAttempt::Placed
    );
    assert_eq!(city.building_at(x, y), 0xdd);
    assert_eq!(city.building_at(x, y + 1), 0xde);
    for ray_y in y + 2..=y + 5 {
        assert_eq!(city.building_at(x, ray_y), 0xdd);
        assert_eq!(city.zone_at(x, ray_y).unwrap().raw_bits(), 0xf8);
        assert_eq!(city.tile_bits_at(x, ray_y).unwrap().raw_bits(), 0xc2);
    }
    assert_eq!(city.zone_at(x, y + 1).unwrap().raw_bits(), 0xf8);
    assert_eq!(city.tile_bits_at(x, y + 1).unwrap().raw_bits(), 0xc0);
    assert_eq!(city.mac_xbld_tile_count(0xdd), 5);
    assert_eq!(city.mac_xbld_tile_count(0xde), 1);
}

#[test]
fn ensure_default_city_options_seeds_pe_newspaper_lists() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(
        city.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS),
        vec![0; NEWSPAPER_LIST_MISC_DWORDS]
    );
    assert_eq!(
        city.misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            NEWSPAPER_STORY_LIST_MISC_DWORDS
        ),
        vec![0; NEWSPAPER_STORY_LIST_MISC_DWORDS]
    );

    ensure_default_city_options(&mut city);

    let papers = city.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS);
    assert_eq!(papers, pe_newspaper_paper_list_misc_values(&city));
    let mut identity: Vec<u32> = pe_newspaper_paper_list_identity_bytes()
        .into_iter()
        .map(u32::from)
        .collect();
    assert_ne!(
        papers, identity,
        "PE newspaper init shuffles the identity 6×5 list"
    );
    let mut shuffled = papers.clone();
    shuffled.sort_unstable();
    identity.sort_unstable();
    assert_eq!(shuffled, identity);

    let stories = city.misc_u32_values(
        MISC_NEWSPAPER_STORY_LIST_OFFSET,
        NEWSPAPER_STORY_LIST_MISC_DWORDS,
    );
    assert_eq!(stories, pe_newspaper_story_list_misc_values());
    assert_eq!(stories[0], 0x0B);
    assert_eq!(stories[5], 0xFF);
    assert_eq!(stories[48], 0x13);
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(0));

    city.set_misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET, 9);
    ensure_default_city_options(&mut city);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET), Some(9));
    assert_eq!(
        city.misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            NEWSPAPER_STORY_LIST_MISC_DWORDS
        ),
        pe_newspaper_story_list_misc_values()
    );
}

#[test]
fn new_city_newspaper_shuffle_consumes_process_crt() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let expected = pe_newspaper_paper_list_misc_values_with_rng(&mut preview);
    // new_city also reseeds fame ranks @ `0x42e585` after newspaper options.
    let mut preview_city =
        City::new_blank("/tmp/sc2000-preview-fame.sc2", "preview").expect("blank");
    preview_city.pe_reseed_neighbor_fame_ranks_42e585(&mut preview);
    state.new_city().expect("new city");
    let papers = state
        .city
        .misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS);
    assert_eq!(papers, expected);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn newspaper_option_writes_seed_pe_newspaper_lists_before_high_misc() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    // New blank cities start with Extra enabled; turning it off still exercises
    // the option writer's newspaper-table initialization path.
    assert!(set_city_newspaper_extra_enabled(&mut city, false));
    assert_eq!(
        city.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS),
        pe_newspaper_paper_list_misc_values(&city)
    );
    assert_eq!(
        city.misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            NEWSPAPER_STORY_LIST_MISC_DWORDS
        ),
        pe_newspaper_story_list_misc_values()
    );
}

#[test]
fn pe_newspaper_identity_records_are_i_imod3() {
    let recs = pe_newspaper_paper_list_identity_bytes();
    for index in 0..NEWSPAPER_LIST_RECORD_COUNT {
        let rec = index * NEWSPAPER_LIST_RECORD_BYTES;
        let i = index as u8;
        assert_eq!(recs[rec], i);
        assert_eq!(recs[rec + 1], i % 3);
        assert_eq!(recs[rec + 2], i % 3);
        assert_eq!(recs[rec + 3], i);
        assert_eq!(recs[rec + 4], i);
    }
}

#[test]
fn pe_newspaper_shuffle_first_swaps_follow_random_bits() {
    let mut rng = MacToolboxRng::new(0x4797_3001);
    let mut expected = MacToolboxRng::new(0x4797_3001);
    let recs = pe_newspaper_paper_list_shuffled_bytes(&mut rng);
    let mut ident = pe_newspaper_paper_list_identity_bytes();
    let bit = usize::from(expected.random_u16() & 1);
    ident.swap(3, 0x12 + bit * NEWSPAPER_LIST_RECORD_BYTES);
    if expected.random_u16() & 1 != 0 {
        ident.swap(1, 6);
    }
    for _ in 0..PE_NEWSPAPER_SHUFFLE_ROUNDS {
        let a = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES;
        let b = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES;
        ident.swap(a, b);
        let a = pe_newspaper_random_mod(&mut expected, 5) * NEWSPAPER_LIST_RECORD_BYTES + 6;
        let b = pe_newspaper_random_mod(&mut expected, 5) * NEWSPAPER_LIST_RECORD_BYTES + 6;
        ident.swap(a, b);
        let a = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES + 2;
        let b = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES + 2;
        ident.swap(a, b);
        let a = pe_newspaper_random_mod(&mut expected, 5) * NEWSPAPER_LIST_RECORD_BYTES + 8;
        let b = pe_newspaper_random_mod(&mut expected, 5) * NEWSPAPER_LIST_RECORD_BYTES + 8;
        ident.swap(a, b);
        let a = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES + 4;
        let b = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES + 4;
        ident.swap(a, b);
    }
    assert_eq!(recs, ident);
    assert_eq!(PE_NEWSPAPER_SHUFFLE_ROUNDS, 0x0C);
}

#[test]
fn pe_newspaper_shuffle_differs_by_city_name() {
    let times = City::new_blank("/tmp/sc2000-test.sc2", "Times").expect("blank city");
    let herald = City::new_blank("/tmp/sc2000-test.sc2", "Herald").expect("blank city");
    assert_ne!(
        pe_newspaper_paper_list_misc_values(&times),
        pe_newspaper_paper_list_misc_values(&herald)
    );
}

#[test]
fn newspaper_max_choice_follows_pe_city_progression() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(newspaper_max_choice_for_city(&city), 0);

    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 0);
    assert_eq!(newspaper_max_choice_for_city(&city), 0);
    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 1);
    assert_eq!(newspaper_max_choice_for_city(&city), 1);
    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 2);
    assert_eq!(newspaper_max_choice_for_city(&city), 2);
    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 5);
    assert_eq!(newspaper_max_choice_for_city(&city), NEWSPAPER_CHOICE_MAX);
    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 9);
    assert_eq!(newspaper_max_choice_for_city(&city), NEWSPAPER_CHOICE_MAX);
}

#[test]
fn newspaper_choice_reads_and_clamps_saved_misc_index() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(newspaper_choice_for_city(&city), 0);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(0));
    assert_eq!(newspaper_paper_name(0), "Times");
    assert_eq!(newspaper_paper_name(2), "Courier");

    ensure_default_city_options(&mut city);
    assert_eq!(newspaper_choice_for_city(&city), 0);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(0));

    assert!(
        !set_city_newspaper_choice(&mut city, 3),
        "locked Picayune should stay Times (progression 0, already seeded)"
    );
    assert_eq!(newspaper_choice_for_city(&city), 0);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(0));

    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, NEWSPAPER_CHOICE_MAX);
    assert!(set_city_newspaper_choice(&mut city, 3));
    assert_eq!(newspaper_choice_for_city(&city), 3);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(3));
    assert!(!set_city_newspaper_choice(&mut city, 3));
    assert_eq!(
        newspaper_paper_name(newspaper_choice_for_city(&city)),
        "Picayune"
    );

    assert!(set_city_newspaper_choice(&mut city, 99));
    assert_eq!(newspaper_choice_for_city(&city), NEWSPAPER_CHOICE_MAX);
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(NEWSPAPER_CHOICE_MAX)
    );
    assert_eq!(
        newspaper_paper_name(newspaper_choice_for_city(&city)),
        "Journal"
    );

    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 1);
    assert_eq!(newspaper_choice_for_city(&city), 1);
    assert!(set_city_newspaper_choice(&mut city, 5));
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(1));
}

#[test]
fn opening_newspaper_dialog_persists_pe_choice_misc() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(0)
    );

    assert_eq!(
        state.apply_state_app_command(&AppCommand::OpenCityInfo(CityInfoDialog::Newspaper(
            NewspaperEdition::Courier
        ))),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(0)
    );
    assert_eq!(newspaper_choice_for_city(&state.city), 0);
    assert_eq!(
        NewspaperEdition::Courier
            .masthead(&state.city.name, newspaper_choice_for_city(&state.city)),
        format!("The {} Times", state.city.name)
    );

    state
        .city
        .set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, NEWSPAPER_COURIER_CHOICE);
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Courier));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(NEWSPAPER_COURIER_CHOICE)
    );
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Extra));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(NEWSPAPER_COURIER_CHOICE)
    );
    assert_eq!(
        NewspaperEdition::Courier
            .masthead(&state.city.name, newspaper_choice_for_city(&state.city)),
        format!("The {} Courier", state.city.name)
    );
}

#[test]
fn city_state_toggles_balloon_help() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert!(!state.balloon_help_enabled);
    assert!(!state.set_palette_balloon_help(Some("Road")));
    assert_eq!(state.balloon_help_line, None);

    state.toggle_balloon_help();
    assert!(state.balloon_help_enabled);
    assert!(state.set_palette_balloon_help(Some("Road")));
    assert_eq!(state.balloon_help_line, Some("Road"));
    assert!(!state.set_palette_balloon_help(Some("Road")));
    assert!(state.set_palette_balloon_help(None));
    assert_eq!(state.balloon_help_line, None);

    assert!(state.set_palette_balloon_help(Some("Pipes")));
    assert_eq!(state.balloon_help_line, Some("Pipes"));
    state.toggle_balloon_help();
    assert!(!state.balloon_help_enabled);
    assert_eq!(state.balloon_help_line, None);
}

#[test]
fn city_state_toggles_ordinances_through_dialog_action() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.toggle_ordinance(12);
    assert!(ordinance_enabled_for_city(&state.city, 12));

    state.toggle_ordinance(12);
    assert!(!ordinance_enabled_for_city(&state.city, 12));
}

#[test]
fn city_state_toggles_graph_series_without_dirtying_city() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.clear_dirty();

    assert!(state.graph_series_visible.iter().all(|visible| *visible));
    assert!(state.toggle_graph_series(2));
    assert!(!state.graph_series_visible[2]);
    assert!(state.toggle_graph_series(2));
    assert!(state.graph_series_visible[2]);
    assert!(!state.toggle_graph_series(GRAPH_HISTORY_TOTAL_SERIES));
    assert!(!state.city.has_dirty_tiles());
}

#[test]
fn city_state_sets_graph_scale_without_dirtying_city() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.clear_dirty();

    assert_eq!(state.graph_scale, GraphScale::Year1);
    assert!(state.set_graph_scale(GraphScale::Years10));
    assert_eq!(state.graph_scale, GraphScale::Years10);
    assert!(!state.set_graph_scale(GraphScale::Years10));
    assert!(state.set_graph_scale(GraphScale::Years100));
    assert_eq!(state.graph_scale, GraphScale::Years100);
    assert!(!state.city.has_dirty_tiles());
}

#[test]
fn graph_panel_reads_xgrp_monthly_samples_oldest_first() {
    let mut values = vec![0_u32; XGRP_VALUES];
    for month in 0..XGRP_MONTH_SAMPLES {
        values[month] = (XGRP_MONTH_SAMPLES - month) as u32;
    }
    let crime_base = 7 * XGRP_SAMPLES_PER_SERIES;
    values[crime_base] = 99;
    values[crime_base + XGRP_MONTH_SAMPLES - 1] = 1;

    let samples = xgrp_samples_oldest_first(&values, 0, GraphScale::Year1);
    assert_eq!(samples, (1..=XGRP_MONTH_SAMPLES as u32).collect::<Vec<_>>());
    assert_eq!(samples[0], 1, "left/oldest is PE monthly index 11");
    assert_eq!(
        samples[XGRP_MONTH_SAMPLES - 1],
        XGRP_MONTH_SAMPLES as u32,
        "right/newest is PE monthly index 0"
    );
    assert_eq!(
        xgrp_samples_oldest_first(&values, 7, GraphScale::Year1)[0],
        1
    );
    assert_eq!(
        xgrp_samples_oldest_first(&values, 7, GraphScale::Year1)[XGRP_MONTH_SAMPLES - 1],
        99
    );
}

#[test]
fn graph_panel_reads_xgrp_half_year_and_five_year_samples_oldest_first() {
    let mut values = vec![0_u32; XGRP_VALUES];
    let half_year = GraphScale::Years10.xgrp_offset();
    let five_year = GraphScale::Years100.xgrp_offset();
    for sample in 0..XGRP_HALF_YEAR_SAMPLES {
        values[half_year + sample] = (XGRP_HALF_YEAR_SAMPLES - sample) as u32;
    }
    for sample in 0..XGRP_FIVE_YEAR_SAMPLES {
        values[five_year + sample] = 200 - sample as u32;
    }

    let ten_year = xgrp_samples_oldest_first(&values, 0, GraphScale::Years10);
    assert_eq!(ten_year.len(), XGRP_HALF_YEAR_SAMPLES);
    assert_eq!(ten_year[0], 1, "left/oldest is PE half-year index 19");
    assert_eq!(
        ten_year[XGRP_HALF_YEAR_SAMPLES - 1],
        XGRP_HALF_YEAR_SAMPLES as u32,
        "right/newest is PE half-year index 0"
    );

    let hundred_year = xgrp_samples_oldest_first(&values, 0, GraphScale::Years100);
    assert_eq!(hundred_year.len(), XGRP_FIVE_YEAR_SAMPLES);
    assert_eq!(
        hundred_year[0],
        200 - (XGRP_FIVE_YEAR_SAMPLES as u32 - 1),
        "left/oldest is PE five-year index 19"
    );
    assert_eq!(
        hundred_year[XGRP_FIVE_YEAR_SAMPLES - 1],
        200,
        "right/newest is PE five-year index 0"
    );
}

#[test]
fn graph_panel_maps_legend_to_pe_xgrp_series() {
    assert_eq!(XGRP_GRAPH_LEGEND_SERIES, [0, 10, 11, 7, 4, 5]);
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut values = vec![0_u32; XGRP_VALUES];
    for (legend, series_index) in XGRP_GRAPH_LEGEND_SERIES.into_iter().enumerate() {
        let base = series_index * XGRP_SAMPLES_PER_SERIES;
        values[base] = 100 + legend as u32;
        values[base + GraphScale::Years10.xgrp_offset()] = 200 + legend as u32;
        values[base + GraphScale::Years100.xgrp_offset()] = 300 + legend as u32;
    }
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &values);

    let monthly = graph_panel_xgrp_series(&city, GraphScale::Year1);
    assert_eq!(monthly.len(), GRAPH_HISTORY_TOTAL_SERIES);
    for (legend, samples) in monthly.iter().enumerate() {
        assert_eq!(samples.len(), XGRP_MONTH_SAMPLES);
        assert_eq!(samples[XGRP_MONTH_SAMPLES - 1], 100 + legend as u32);
    }

    let ten_year = graph_panel_xgrp_series(&city, GraphScale::Years10);
    for (legend, samples) in ten_year.iter().enumerate() {
        assert_eq!(samples.len(), XGRP_HALF_YEAR_SAMPLES);
        assert_eq!(samples[XGRP_HALF_YEAR_SAMPLES - 1], 200 + legend as u32);
    }

    let hundred_year = graph_panel_xgrp_series(&city, GraphScale::Years100);
    for (legend, samples) in hundred_year.iter().enumerate() {
        assert_eq!(samples.len(), XGRP_FIVE_YEAR_SAMPLES);
        assert_eq!(samples[XGRP_FIVE_YEAR_SAMPLES - 1], 300 + legend as u32);
    }
}

#[test]
fn city_information_dialogs_check_their_palette_buttons() {
    let cases = [
        (CityInfoDialog::Map, UiAction::OpenCityMap),
        (CityInfoDialog::Population, UiAction::OpenCityPopulation),
        (CityInfoDialog::Neighbors, UiAction::OpenCityNeighbors),
        (CityInfoDialog::Graphs, UiAction::OpenCityGraphs),
        (CityInfoDialog::Industry, UiAction::OpenCityIndustry),
        (CityInfoDialog::Budget, UiAction::OpenBudget),
        (CityInfoDialog::Help, UiAction::OpenHelp),
    ];

    for (dialog, action) in cases {
        let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        let mut state = test_state(city);

        state.open_city_info_dialog(dialog);

        assert!(
            state.checked_palette_actions().contains(&action),
            "{dialog:?} did not check {action:?}"
        );
    }
}

#[test]
fn opening_city_information_dialog_clears_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_tool_menu = Some(ui::ToolMenu::Water);
    seed_all_modal_dialogs(&mut state);
    seed_all_drag_states(&mut state);

    state.open_city_info_dialog(CityInfoDialog::Help);

    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.power_plant_dialog, None);
    assert_eq!(state.arcology_dialog, None);
    assert_eq!(state.sign_dialog, None);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Help));
    assert_eq!(state.city.label_at(2), Some("Original Sign"));
    assert_drag_states_cleared(&state);
}

#[test]
fn city_map_dialog_hit_tests_overview_cells_as_city_tiles() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let (map, cell) = city_map_panel_rect_and_cell(dialog);
    let point = Vec2::new(map.x + cell * 7.5, map.y + cell * 10.5);

    assert_eq!(city_map_tile_for_point(point, dialog), Some((30, 42)));
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Map),
        Some(CityInfoDialogAction::CenterMapOn { x: 30, y: 42 })
    );
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Graphs),
        None
    );

    let bottom_right = Vec2::new(map.x + map.w - 0.5, map.y + map.h - 0.5);
    assert_eq!(
        city_map_tile_for_point(bottom_right, dialog),
        Some((CITY_WIDTH - 2, CITY_HEIGHT - 2))
    );

    let marker = city_map_cell_rect_for_tile(30, 42, map, cell);
    assert_eq!(marker.x, map.x + cell * 7.0);
    assert_eq!(marker.y, map.y + cell * 10.0);
    assert_eq!(marker.w, cell);
    assert_eq!(marker.h, cell);
}

#[test]
fn population_panel_demographics_include_city_fame() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_CITY_FAME_OFFSET, 73);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 123_000);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 456_000);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 7);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 12);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 72);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 15);
    city.set_misc_u32_at(MISC_HUMID_OFFSET, 42);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 3);
    let population = PopulationStats {
        work_force: 456,
        residential_population: 1_000,
        total_population: 1_000,
        ..Default::default()
    };
    let outcomes = CityServiceOutcomes {
        life_expectancy: 78,
        education_quotient: 91,
        service_score: 64,
        city_value: 5_000,
        land_value: 82,
        ..Default::default()
    };

    assert_eq!(
        population_panel_demographic_rows(&city, population, outcomes),
        vec![
            ("Work Force %", 45),
            ("Unemployment %", 0),
            ("Life Expectancy", 78),
            ("Education Quotient", 91),
            ("Service Score", 64),
            ("City Fame", 73),
            ("National Population", 123_000),
            ("National Value", 456_000),
            ("National Tax", 7),
            ("National Trend", 12),
            ("Heat", 72),
            ("Wind", 15),
            ("Humid", 42),
            ("Weather Trend", 3),
            ("City Value", 5_000),
            ("Land Value", 82),
        ]
    );
    assert_eq!(
        population_panel_transit_rows(TransitPassengerStats {
            bus: 12,
            rail: 4,
            subway: 7,
        }),
        [
            ("Bus Passengers", 12),
            ("Rail Passengers", 4),
            ("Subway Passengers", 7),
        ]
    );
}

#[test]
fn zoom_palette_buttons_disable_at_zoom_bounds() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.zoom = MIN_ZOOM;
    assert!(!state.ui_action_enabled(UiAction::ZoomOut));
    assert!(state.ui_action_enabled(UiAction::ZoomIn));
    assert!(
        state
            .disabled_palette_actions()
            .contains(&UiAction::ZoomOut)
    );
    assert!(!state.disabled_palette_actions().contains(&UiAction::ZoomIn));

    state.zoom = MAX_ZOOM;
    assert!(state.ui_action_enabled(UiAction::ZoomOut));
    assert!(!state.ui_action_enabled(UiAction::ZoomIn));
    assert!(state.disabled_palette_actions().contains(&UiAction::ZoomIn));
    assert!(
        !state
            .disabled_palette_actions()
            .contains(&UiAction::ZoomOut)
    );

    state.zoom = (MIN_ZOOM + MAX_ZOOM) * 0.5;
    assert!(state.ui_action_enabled(UiAction::ZoomOut));
    assert!(state.ui_action_enabled(UiAction::ZoomIn));
    assert!(
        !state
            .disabled_palette_actions()
            .contains(&UiAction::ZoomOut)
    );
    assert!(!state.disabled_palette_actions().contains(&UiAction::ZoomIn));
}

#[test]
fn selecting_city_tool_refreshes_status_line_and_clears_stale_query() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.query_line = "Tile 10,10".to_owned();
    state.query_is_inspect = true;

    assert!(state.handle_ui_action_state(UiAction::SelectTool(EditTool::ZoneCommercialDense,)));

    assert_eq!(state.tool, EditTool::ZoneCommercialDense);
    assert!(!state.query_is_inspect);
    assert!(
        state.query_line.starts_with("Dense Commercial $10  "),
        "status line was {}",
        state.query_line
    );
}

#[test]
fn query_tile_status_line_names_palette_buildings_and_zones() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for (x, y, building, expected) in [
        (10, 10, 207, "Coal Power"),
        (11, 10, 210, "Police"),
        (12, 10, 214, "School"),
        (13, 10, 218, "Zoo"),
        (14, 10, 220, "Water Pump"),
        (15, 10, 236, "Bus Depot"),
        (16, 10, 221, "Runway"),
        (17, 10, 224, "Crane"),
        (18, 10, 251, "Plymouth Arco"),
        (19, 10, 0x70, "Residential Building"),
        (20, 10, 0x7c, "Commercial Building"),
        (21, 10, 0x84, "Industrial Building"),
        (22, 10, 230, "Tarmac"),
    ] {
        city.set_building(x, y, building);
        assert!(
            query_tile_status_line(&city, x, y, false).starts_with(expected),
            "building {building} query was {}",
            query_tile_status_line(&city, x, y, false)
        );
    }

    city.set_zone(30, 30, ZONE_INDUSTRIAL_DENSE);
    assert!(
        query_tile_status_line(&city, 30, 30, false).starts_with("Dense Industrial"),
        "{}",
        query_tile_status_line(&city, 30, 30, false)
    );
    city.set_zone(31, 30, ZONE_AIRPORT);
    assert!(
        query_tile_status_line(&city, 31, 30, false).starts_with("Airport Zone"),
        "{}",
        query_tile_status_line(&city, 31, 30, false)
    );
}

#[test]
fn query_tile_status_line_prefers_underground_when_requested() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 210);

    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Police"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );
    assert!(
        query_tile_status_line(&city, 10, 10, true).starts_with("Building Pipes"),
        "{}",
        query_tile_status_line(&city, 10, 10, true)
    );

    city.set_underground(10, 10, 16);
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Police"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );
    assert!(
        query_tile_status_line(&city, 10, 10, true).starts_with("Pipes + Building Pipes"),
        "{}",
        query_tile_status_line(&city, 10, 10, true)
    );

    city.set_underground(40, 40, UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR);
    assert!(
        query_tile_status_line(&city, 40, 40, true).starts_with("Pipes/Subway Crossing"),
        "{}",
        query_tile_status_line(&city, 40, 40, true)
    );

    city.set_text_overlay(40, 40, 0xfc);
    assert!(
        query_tile_status_line(&city, 40, 40, true).starts_with("Pipes/Subway Crossing"),
        "{}",
        query_tile_status_line(&city, 40, 40, true)
    );
    assert!(
        query_tile_status_line(&city, 40, 40, false).starts_with("Flood"),
        "{}",
        query_tile_status_line(&city, 40, 40, false)
    );
}

#[test]
fn query_tile_status_line_falls_back_for_unknown_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 199);

    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Building 199"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );
}

#[test]
fn query_uses_xlabel_title_when_microsim_is_live() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 210);
    let police = city
        .provision_microsim_for_building(210)
        .expect("police xlab");
    city.set_text_overlay(10, 10, police);
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Police Station"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_label(police, "");
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Police"),
        "empty XLAB falls through to the palette name: {}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_building(12, 12, 199);
    let hydro = city
        .provision_microsim_for_building(199)
        .expect("hydro 199");
    city.set_text_overlay(12, 12, hydro);
    assert!(
        query_tile_status_line(&city, 12, 12, false).starts_with("Hydro Power"),
        "{}",
        query_tile_status_line(&city, 12, 12, false)
    );

    let bus = city.provision_microsim_for_building(236).expect("bus");
    city.set_building(14, 14, 236);
    city.set_text_overlay(14, 14, bus);
    city.set_label(bus, "Downtown Transit");
    assert!(
        query_tile_status_line(&city, 14, 14, false).starts_with("Downtown Transit"),
        "{}",
        query_tile_status_line(&city, 14, 14, false)
    );
}

#[test]
fn query_uses_xlabel_title_for_user_signs() {
    let mut city = City::new_blank("/tmp/sc2000-query-sign.sc2", "test").expect("blank city");
    city.set_text_overlay(10, 10, MIN_USER_TEXT_ENTRIES);
    city.set_label(MIN_USER_TEXT_ENTRIES, "Main Street");
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Main Street"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_building(10, 10, 29);
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Main Street"),
        "user-sign XLAB wins over the road palette name: {}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_label(MIN_USER_TEXT_ENTRIES, "");
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Road"),
        "empty sign XLAB falls through to the palette: {}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_label(0, "Marvin Maxis");
    city.set_text_overlay(11, 11, 0);
    assert!(
        query_tile_status_line(&city, 11, 11, false).starts_with("Terrain"),
        "XLAB[0] is the mayor name, not a map sign: {}",
        query_tile_status_line(&city, 11, 11, false)
    );

    city.set_text_overlay(12, 12, MAX_USER_TEXT_ENTRIES);
    city.set_label(MAX_USER_TEXT_ENTRIES, "North Gate");
    assert!(
        query_tile_status_line(&city, 12, 12, false).starts_with("North Gate"),
        "{}",
        query_tile_status_line(&city, 12, 12, false)
    );
}

#[test]
fn query_tile_status_line_shows_microsim_stats() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);

    assert!(state.place_building_at(20, 20, 207) > 0); // coal
    assert!(state.place_building_at(30, 30, 210) > 0); // police
    assert!(state.place_building_at(40, 40, 251) > 0); // plymouth arco
    let arco_text = state.city.text_overlay_at(40, 40).expect("arco xtxt");
    let arco_index = usize::from(arco_text - MIN_SIM_TEXT_ENTRIES);
    assert!(state.city.set_microsim_stat0(arco_index, 12));
    state.city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 9_496);
    state
        .city
        .refresh_structure_microsim_yearly(55_860, 100, 100, 100, 100, 100);

    let coal = query_tile_status_line(&state.city, 20, 20, false);
    assert!(coal.starts_with("Coal Power"), "{coal}");
    assert!(coal.contains("Max 200 MW"), "{coal}");
    assert!(coal.contains("Age 1"), "{coal}");

    let police = query_tile_status_line(&state.city, 30, 30, false);
    assert!(police.starts_with("Police Station"), "{police}");
    assert!(police.contains("Officers 200"), "{police}");
    assert!(police.contains("Crimes 1,055"), "{police}");
    assert!(police.contains("Arrests"), "{police}");

    let arco = query_tile_status_line(&state.city, 40, 40, false);
    assert!(arco.contains("Design Capacity 55,000"), "{arco}");
    let arco_record = state.city.microsim_at(arco_index).expect("arcology XMIC");
    assert!(
        arco.contains(&format!(
            "Residents {}",
            format_number(u32::from(arco_record.stat2))
        )),
        "query should show the resident word stored in XMIC: {arco}"
    );
    assert!(arco.contains("Built 1,900"), "{arco}");
    assert!(arco.contains("Conditions 12"), "{arco}");
}

#[test]
fn query_dialog_shows_general_and_microsim_fields() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 207) > 0);
    state
        .city
        .sync_crime_density_from_slice(&vec![90u8; CITY_TILES]);
    state
        .city
        .sync_pollution_density_from_slice(&vec![30u8; CITY_TILES]);
    state.city.set_traffic_density(20, 20, 150);
    state
        .city
        .refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);

    let (general, specific) = query_dialog_row_groups(
        &state.city,
        20,
        20,
        false,
        TransitPassengerStats::default(),
        pe_power_recalc_seed(&state.city),
    );
    let general_map: std::collections::HashMap<_, _> = general.into_iter().collect();
    assert_eq!(
        general_map.get("Location").map(String::as_str),
        Some("(20,20)")
    );
    assert!(
        general_map
            .get("Altitude")
            .is_some_and(|v| v.contains("feet")),
        "{general_map:?}"
    );
    assert!(
        general_map
            .get("Land Value")
            .is_some_and(|v| v.contains(",000/acre")),
        "{general_map:?}"
    );
    assert_eq!(general_map.get("Crime").map(String::as_str), Some("Medium"));
    assert_eq!(
        general_map.get("Pollution").map(String::as_str),
        Some("Low")
    );
    assert_eq!(general_map.get("Traffic").map(String::as_str), Some("High"));

    let specific_map: std::collections::HashMap<_, _> = specific.into_iter().collect();
    assert!(
        specific_map.contains_key("Max") && specific_map.contains_key("Age"),
        "{specific_map:?}"
    );

    state.begin_tile_inspect(20, 20);
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Query { x: 20, y: 20 })
    );
    assert!(state.query_is_inspect);
    assert!(
        state.query_line.starts_with("Coal Power"),
        "{}",
        state.query_line
    );

    let mut solids = Vec::new();
    state.push_city_info_dialog(None, (1024, 768), 1.0, &mut solids, &mut Vec::new());
    assert!(solids.len() > 10);

    state.close_city_info_dialog();
    assert_eq!(state.city_info_dialog, None);
}

#[test]
fn query_shows_letter_grades_for_school_hospital_college() {
    assert_eq!(microsim_query_letter_grade(214, 12), 'A');
    assert_eq!(microsim_query_letter_grade(214, 10), 'B');
    assert_eq!(microsim_query_letter_grade(214, 6), 'D');
    assert_eq!(microsim_query_letter_grade(214, 0), 'F');
    assert_eq!(microsim_query_letter_grade(217, 8), 'C');
    assert_eq!(microsim_query_letter_grade(209, 12), 'A');
    assert_eq!(microsim_query_letter_grade(209, 6), 'C');
    assert_eq!(microsim_query_letter_grade(209, 0), 'F');

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 7_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 214) > 0); // school
    assert!(state.place_building_at(30, 30, 209) > 0); // hospital
    assert!(state.place_building_at(40, 40, 217) > 0); // college

    let seed_school = query_tile_status_line(&state.city, 20, 20, false);
    assert!(
        seed_school.contains("Grade D"),
        "fresh school seed stat0=6 → D, got {seed_school}"
    );

    state.city.persist_pe_day22_population_leftover(7_000);
    state.city.advance_pe_eq_le_month(
        7_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    state
        .city
        .refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let school = query_tile_status_line(&state.city, 20, 20, false);
    let hospital = query_tile_status_line(&state.city, 30, 30, false);
    let college = query_tile_status_line(&state.city, 40, 40, false);
    assert!(
        school.contains("Grade A") && school.contains("Students 350"),
        "leftover-filled EQ_LE[1]+[2] occupancy, not a 12% pyramid: {school}"
    );
    assert!(hospital.contains("Grade A"), "{hospital}");
    assert!(college.contains("Grade A"), "{college}");

    let (_, specific) = query_dialog_row_groups(
        &state.city,
        20,
        20,
        false,
        TransitPassengerStats::default(),
        pe_power_recalc_seed(&state.city),
    );
    assert!(
        specific
            .iter()
            .any(|(label, value)| label == "Grade" && value == "A"),
        "{specific:?}"
    );
}

#[test]
fn query_shows_stadium_team_name_only_after_team_selection() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 215) > 0);

    let seeded = query_tile_status_line(&state.city, 20, 20, false);
    assert!(
        seeded.contains("Team Football"),
        "an unselected team uses the generic football label: {seeded}"
    );

    state.city.set_label(0xFB, "Nexatrode Knights");
    state
        .city
        .refresh_structure_microsim_yearly(55_860, 100, 100, 100, 100, 100);
    let january = query_tile_status_line(&state.city, 20, 20, false);
    assert!(january.contains("Attendance 3,716"), "{january}");
    assert!(
        january.contains("Team Football"),
        "yearly attendance must not select a team: {january}"
    );
    let xtxt = state
        .city
        .text_overlay_at(20, 20)
        .expect("placed stadium XTXT");
    let index = usize::from(xtxt - MIN_SIM_TEXT_ENTRIES);
    assert!(state.city.set_microsim_stat3(index, 0xfb));
    let line = query_tile_status_line(&state.city, 20, 20, false);
    assert!(line.contains("Team Nexatrode Knights"), "{line}");
    assert_eq!(state.city.label_at(0xFB), Some("Nexatrode Knights"));

    let (_, specific) = query_dialog_row_groups(
        &state.city,
        20,
        20,
        false,
        TransitPassengerStats::default(),
        pe_power_recalc_seed(&state.city),
    );
    assert!(
        specific
            .iter()
            .any(|(label, value)| label == "Team" && value == "Nexatrode Knights"),
        "{specific:?}"
    );
}

#[test]
fn every_city_palette_tool_has_a_handled_preview_or_dialog_route() {
    let tools = visible_city_menu_tools();

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    enable_all_conditional_palette_tools(&mut city);
    let mut state = test_state(city);

    for tool in tools {
        assert_ne!(
            GameCursor::for_tool(tool),
            GameCursor::Arrow,
            "{tool:?} fell back to the arrow cursor"
        );

        state.tool = tool;
        if tool == EditTool::Query {
            assert_eq!(state.tool_placement_at(20, 20), None);
            continue;
        }
        if matches!(tool, EditTool::PowerPlant | EditTool::Arcologies) {
            assert!(
                state.selected_tool_status_line().is_none(),
                "{tool:?} chooser button should open a dialog instead of showing a build price"
            );
            continue;
        }

        assert!(
            state.tool_placement_at(20, 20).is_some(),
            "{tool:?} has no city-view preview route"
        );
    }
}

#[test]
fn city_palette_menus_do_not_expose_edit_map_only_tools() {
    for menu in ALL_CITY_TOOL_MENUS {
        for item in ui::tool_menu_items(*menu) {
            assert_ne!(
                expected_city_tool_behavior(item.tool),
                CityToolBehavior::EditMapOnly,
                "{menu:?} exposes {:?}, but that tool only has edit-map behavior",
                item.tool
            );
        }
    }
}

#[test]
fn visible_city_palette_buttons_apply_expected_runtime_actions() {
    let actions = ui::city_palette_actions_for_test().collect::<Vec<_>>();
    assert_eq!(actions.len(), 34);

    for action in actions {
        drain_command_queue_for_test();
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(500_000);
        enable_all_conditional_palette_tools(&mut city);
        let mut state = test_state(city);

        match action {
            UiAction::RotateViewCcw => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.city.rotation(), 3);
                assert_eq!(state.camera_rotation, 3);
                assert_eq!(pop_command(), None);
            }
            UiAction::RotateViewCw => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.city.rotation(), 1);
                assert_eq!(state.camera_rotation, 1);
                assert_eq!(pop_command(), None);
            }
            UiAction::ZoomOut => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(pop_command(), Some(AppCommand::ZoomOut));
                assert_eq!(pop_command(), None);
            }
            UiAction::ZoomIn => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(pop_command(), Some(AppCommand::ZoomIn));
                assert_eq!(pop_command(), None);
            }
            UiAction::CenterMap => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(pop_command(), Some(AppCommand::CenterMap));
                assert_eq!(pop_command(), None);
            }
            UiAction::OpenCityMap
            | UiAction::OpenCityPopulation
            | UiAction::OpenCityNeighbors
            | UiAction::OpenCityGraphs
            | UiAction::OpenCityIndustry
            | UiAction::OpenBudget
            | UiAction::OpenHelp => {
                let expected = CityInfoDialog::for_action(action).expect("city dialog action");
                assert!(state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.city_info_dialog, Some(expected), "{action:?}");
                assert_eq!(pop_command(), None);
            }
            UiAction::OpenMenu(menu) => {
                let expected_tool = ui::tool_menu_items(menu)
                    .iter()
                    .find(|item| {
                        !is_chooser_menu_tool(item.tool)
                            && state.ui_action_enabled(UiAction::SelectTool(item.tool))
                    })
                    .map(|item| item.tool);
                assert!(state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.open_tool_menu, Some(menu), "{action:?}");
                if let Some(expected_tool) = expected_tool {
                    assert_eq!(state.tool, expected_tool, "{action:?}");
                }
                assert_eq!(pop_command(), None);
            }
            UiAction::SelectTool(tool) => {
                assert!(state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.tool, tool, "{action:?}");
                assert_eq!(state.open_tool_menu, None, "{action:?}");
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleBuildings => {
                assert!(state.show_buildings);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(!state.show_buildings);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleSigns => {
                assert!(state.show_signs);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(!state.show_signs);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleInfrastructure => {
                assert!(state.show_infrastructure);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(!state.show_infrastructure);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleZones => {
                assert!(state.show_zones);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(!state.show_zones);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleUnderground => {
                assert!(!state.show_underground);
                assert!(state.show_traffic);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(state.show_underground);
                assert!(!state.show_traffic);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleEditCoast
            | UiAction::ToggleEditRiver
            | UiAction::GenerateEditMap
            | UiAction::RaiseEditSeaLevel
            | UiAction::LowerEditSeaLevel
            | UiAction::DoneEditMap => {
                panic!("{action:?} should not be on the city palette")
            }
        }
    }

    drain_command_queue_for_test();
}

#[test]
fn visible_city_palette_tools_execute_expected_first_action() {
    for tool in visible_city_menu_tools() {
        let behavior = expected_city_tool_behavior(tool);
        let (mut state, tile) = city_tool_execution_fixture(tool);

        match behavior {
            CityToolBehavior::Query => {
                assert_eq!(state.tool_placement_at(tile.0, tile.1), None, "{tool:?}");
                assert!(
                    !state.apply_tool_at_tile(tile.0, tile.1),
                    "{tool:?} should inspect, not build"
                );
            }
            CityToolBehavior::ChooserDialog => {
                assert!(
                    !state.apply_tool_at_tile(tile.0, tile.1),
                    "{tool:?} should not place without a chooser selection"
                );
                match tool {
                    EditTool::PowerPlant => {
                        state.open_power_plant_dialog();
                        assert!(state.power_plant_dialog.is_some(), "{tool:?}");
                    }
                    EditTool::Arcologies => {
                        state.open_arcology_dialog();
                        assert!(state.arcology_dialog.is_some(), "{tool:?}");
                    }
                    _ => unreachable!("{tool:?} is not a chooser tool"),
                }
            }
            CityToolBehavior::EditMapOnly => {
                panic!("{tool:?} should not be visible in the city palette");
            }
            CityToolBehavior::Network(kind) => {
                state.begin_network_drag(tile.0, tile.1, kind);
                let (_, placed, _, _) = state.finish_network_drag().expect("network drag");
                assert!(placed > 0, "{tool:?}");
                assert!(
                    kind.contains(state.city.building_at(tile.0, tile.1)),
                    "{tool:?}"
                );
            }
            CityToolBehavior::Underground(kind) => {
                state.begin_underground_drag(tile.0, tile.1, kind);
                let (_, placed) = state.finish_underground_drag().expect("underground drag");
                assert!(placed > 0, "{tool:?}");
                assert_ne!(state.city.underground_at(tile.0, tile.1), 0, "{tool:?}");
            }
            CityToolBehavior::Zone(zone_id) => {
                state.begin_zone_drag(tile.0, tile.1, zone_id);
                let placed = state.finish_zone_drag();
                if zone_id == 0 {
                    assert_eq!(placed, 1, "{tool:?}");
                    assert_eq!(state.city.building_at(tile.0, tile.1), 0, "{tool:?}");
                    // Native dezone clears small rubble but skips the XZON
                    // nibble write for type zero.
                    assert_eq!(
                        state.city.zone_at(tile.0, tile.1).unwrap().zone_id(),
                        ZONE_RESIDENTIAL_LIGHT,
                        "{tool:?}"
                    );
                } else {
                    assert!(placed > 0, "{tool:?}");
                    assert_eq!(
                        state.city.zone_at(tile.0, tile.1).unwrap().zone_id(),
                        zone_id,
                        "{tool:?}"
                    );
                }
            }
            CityToolBehavior::TerrainEdit => {
                state.begin_terrain_drag(tile.0, tile.1);
                assert!(state.finish_terrain_drag() > 0, "{tool:?}");
            }
            CityToolBehavior::SurfaceWater => {
                state.begin_nature_drag(tile.0, tile.1, tool);
                assert_eq!(state.finish_nature_drag(), Some((tool, 1)), "{tool:?}");
                assert!(is_water_terrain(state.city.terrain_at(tile.0, tile.1)));
            }
            CityToolBehavior::Trees => {
                state.begin_nature_drag(tile.0, tile.1, tool);
                assert_eq!(state.finish_nature_drag(), Some((tool, 1)), "{tool:?}");
                assert!(
                    matches!(state.city.building_at(tile.0, tile.1), 6 | 7),
                    "{tool:?}"
                );
            }
            CityToolBehavior::Forest => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                let mut trees = 0;
                for y in tile.1.saturating_sub(7)..=(tile.1 + 7).min(CITY_HEIGHT - 1) {
                    for x in tile.0.saturating_sub(7)..=(tile.0 + 7).min(CITY_WIDTH - 1) {
                        if is_tree_building(state.city.building_at(x, y)) {
                            trees += 1;
                        }
                    }
                }
                assert!(trees > 0, "{tool:?} PE forest scatter stamps trees");
            }
            CityToolBehavior::Highway => {
                state.begin_highway_drag(tile.0, tile.1);
                assert!(state.finish_highway_drag().0 > 0, "{tool:?}");
            }
            CityToolBehavior::Bulldoze => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert!(
                    matches!(state.city.building_at(tile.0, tile.1), 1..=4),
                    "{tool:?} leaves PE DestroyBuilding rubble"
                );
            }
            CityToolBehavior::Sign => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert!(state.sign_dialog.is_some(), "{tool:?}");
            }
            CityToolBehavior::Tunnel => {
                let plan = state.tunnel_plan_at(tile.0, tile.1).expect("tunnel plan");
                let tiles = plan.tiles.clone();
                let exit = *tiles.last().expect("tunnel exit");
                let entrance_building = plan.entrance_building;
                let exit_building = plan.exit_building;

                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert_eq!(
                    state.city.building_at(tile.0, tile.1),
                    entrance_building,
                    "{tool:?}"
                );
                assert_eq!(
                    state.city.building_at(exit.0, exit.1),
                    exit_building,
                    "{tool:?}"
                );
                assert!(
                    tiles
                        .iter()
                        .all(|&(x, y)| state.city.tunnel_levels_at(x, y) > 0),
                    "{tool:?} did not stamp tunnel depth"
                );
                assert_tool_debited_units(&state, tool, tiles.len() as u32);
            }
            CityToolBehavior::Onramp => {
                let plan = state.onramp_plan_at(tile.0, tile.1).expect("onramp plan");
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert_eq!(
                    state.city.building_at(tile.0, tile.1),
                    plan.building,
                    "{tool:?}"
                );
                assert_eq!(
                    state.city.tile_bits_at(tile.0, tile.1).unwrap().rotate(),
                    plan.rotate,
                    "{tool:?}"
                );
                assert_ne!(
                    state.network_neighbor_mask(
                        plan.road_tile.0,
                        plan.road_tile.1,
                        NetworkKind::Road,
                    ) & opposite_network_side(NETWORK_RIGHT),
                    0,
                    "{tool:?} did not connect the adjacent road"
                );
                assert_tool_debited_units(&state, tool, 1);
            }
            CityToolBehavior::SubToRail => {
                let plan = state
                    .sub_to_rail_plan_at(tile.0, tile.1)
                    .expect("sub-to-rail plan");
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert_eq!(
                    state.city.building_at(tile.0, tile.1),
                    plan.building,
                    "{tool:?}"
                );
                assert_eq!(
                    state.city.underground_at(tile.0, tile.1),
                    UNDER_TILE_GREY_BOX_2,
                    "{tool:?}"
                );
                assert_tool_debited_units(&state, tool, 1);
            }
            CityToolBehavior::Dispatch(kind) => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                let record = state
                    .city
                    .traffic_overlays()
                    .expect("dispatch overlay table")
                    .iter()
                    .skip(crate::win95_overlay::XTHG_FIRST_USABLE_SLOT)
                    .find(|record| record.kind == kind.overlay_kind())
                    .expect("dispatch overlay record");
                assert_eq!((record.tile_x as usize, record.tile_y as usize), tile);
                // PE FUN_00450690: dispatch kinds use fixed XTHG spawn height 0, not ALTM.
                assert_eq!(
                    record.height,
                    crate::win95_overlay::xthg_spawn_height(kind.overlay_kind()),
                    "{tool:?}"
                );
                assert_eq!(record.saved_a, 0);
            }
            CityToolBehavior::DirectBuilding(building) => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert_tool_stamped_building(&state, tool, tile, building);
            }
            CityToolBehavior::HydroPower => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                // PE @ `0x441610`: rot0 flat → `0xC7` (199).
                let stamped = state.city.building_at(tile.0, tile.1);
                assert!(
                    stamped == 198 || stamped == 199,
                    "{tool:?} hydro XBLD {stamped}"
                );
            }
        }
    }
}

#[test]
fn every_city_palette_tool_has_a_mouse_input_route() {
    let tools = visible_city_menu_tools();

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    enable_all_conditional_palette_tools(&mut city);
    let mut state = test_state(city);

    for tool in tools {
        let route = test_mouse_input_route_for_tool(tool)
            .unwrap_or_else(|| panic!("{tool:?} has no known city-view mouse route"));
        state.tool = tool;

        match route {
            TestMouseInputRoute::Query => {
                assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
            }
            TestMouseInputRoute::ChooserDialog => {
                assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
            }
            TestMouseInputRoute::Drag | TestMouseInputRoute::DirectClick => {
                assert!(
                    state.tool_placement_at(20, 20).is_some(),
                    "{tool:?} has a mouse route but no preview route"
                );
            }
        }
    }
}

#[test]
fn visible_city_tool_matrix_covers_cursor_route_preview_cost_and_sound() {
    let tools = visible_city_menu_tools();
    let mut seen_tools = Vec::new();

    for tool in tools {
        assert!(
            !seen_tools.contains(&tool),
            "{tool:?} appears more than once in the visible city palette"
        );
        seen_tools.push(tool);

        let behavior = expected_city_tool_behavior(tool);
        let (state, tile) = city_tool_execution_fixture(tool);
        let input_route = test_mouse_input_route_for_tool(tool)
            .unwrap_or_else(|| panic!("{tool:?} has no city palette input route"));

        assert_ne!(
            GameCursor::for_tool(tool),
            GameCursor::Arrow,
            "{tool:?} should have a recovered SC2K cursor"
        );
        assert_ne!(
            behavior,
            CityToolBehavior::EditMapOnly,
            "{tool:?} should not be exposed in the city palette"
        );
        assert_eq!(
            state.selected_tool_status_line(),
            expected_tool_status_line(tool, behavior),
            "{tool:?} status-line cost does not match its placement class"
        );
        if input_route == TestMouseInputRoute::DirectClick {
            assert_eq!(
                one_click_placement_sound(tool),
                expected_success_placement_sound(tool, behavior),
                "{tool:?} one-click sound route drifted"
            );
        }
        match behavior {
            CityToolBehavior::Network(kind) => assert_eq!(
                expected_success_placement_sound(tool, behavior),
                Some(network_placement_sound(kind)),
                "{tool:?} drag sound route drifted"
            ),
            CityToolBehavior::Underground(kind) => assert_eq!(
                expected_success_placement_sound(tool, behavior),
                Some(underground_placement_sound(kind)),
                "{tool:?} underground sound route drifted"
            ),
            CityToolBehavior::Trees | CityToolBehavior::Forest => assert_eq!(
                expected_success_placement_sound(tool, behavior),
                Some(PlacementSound::Plop),
                "{tool:?} nature sound route drifted"
            ),
            CityToolBehavior::SurfaceWater => assert_eq!(
                expected_success_placement_sound(tool, behavior),
                Some(PlacementSound::Flood),
                "{tool:?} surface-water sound route drifted"
            ),
            _ => {}
        }

        let expected_preview = expected_tool_preview_for_fixture(behavior, &state, tile);
        let actual_placement = state.tool_placement_at(tile.0, tile.1);
        assert_eq!(
            actual_placement.as_ref().map(|placement| placement.preview),
            expected_preview,
            "{tool:?} preview class does not match its placement class"
        );
        if let Some(placement) = actual_placement {
            assert!(
                !placement.is_empty(),
                "{tool:?} produced an empty placement at its fixture tile"
            );
            if !matches!(
                behavior,
                CityToolBehavior::Query
                    | CityToolBehavior::ChooserDialog
                    | CityToolBehavior::EditMapOnly
            ) {
                assert!(
                    !placement.valid_tiles.is_empty(),
                    "{tool:?} should have valid tiles in its placement fixture"
                );
            }
        }

        match input_route {
            TestMouseInputRoute::Query => {
                assert_eq!(behavior, CityToolBehavior::Query, "{tool:?}");
            }
            TestMouseInputRoute::ChooserDialog => {
                assert_eq!(behavior, CityToolBehavior::ChooserDialog, "{tool:?}");
            }
            TestMouseInputRoute::Drag => {
                assert!(
                    matches!(
                        behavior,
                        CityToolBehavior::TerrainEdit
                            | CityToolBehavior::SurfaceWater
                            | CityToolBehavior::Trees
                            | CityToolBehavior::Network(_)
                            | CityToolBehavior::Highway
                            | CityToolBehavior::Underground(_)
                            | CityToolBehavior::Zone(_)
                    ),
                    "{tool:?} has a drag route but behavior {behavior:?}"
                );
            }
            TestMouseInputRoute::DirectClick => {
                assert!(
                    matches!(
                        behavior,
                        CityToolBehavior::Bulldoze
                            | CityToolBehavior::Sign
                            | CityToolBehavior::Tunnel
                            | CityToolBehavior::Onramp
                            | CityToolBehavior::SubToRail
                            | CityToolBehavior::DirectBuilding(_)
                            | CityToolBehavior::HydroPower
                            | CityToolBehavior::Dispatch(_)
                            | CityToolBehavior::Forest
                    ),
                    "{tool:?} has a direct-click route but behavior {behavior:?}"
                );
            }
        }
    }
}

#[test]
fn opening_city_tool_menus_selects_first_buildable_choice() {
    let cases = [
        (ui::ToolMenu::Residential, EditTool::ZoneResidential),
        (ui::ToolMenu::Commercial, EditTool::ZoneCommercial),
        (ui::ToolMenu::Industrial, EditTool::ZoneIndustrial),
        (ui::ToolMenu::Parks, EditTool::SmallPark),
        (ui::ToolMenu::Power, EditTool::Power),
        (ui::ToolMenu::Civic, EditTool::Police),
        (ui::ToolMenu::Education, EditTool::School),
        (ui::ToolMenu::Water, EditTool::Pipes),
        (ui::ToolMenu::Terrain, EditTool::Trees),
        (ui::ToolMenu::TerrainEdit, EditTool::Bulldoze),
        (ui::ToolMenu::Road, EditTool::Road),
        (ui::ToolMenu::Rail, EditTool::Rail),
        (ui::ToolMenu::Transport, EditTool::Seaport),
    ];

    for (menu, expected_tool) in cases {
        let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        let mut state = test_state(city);
        state.tool = EditTool::Query;
        state.query_is_inspect = true;

        state.open_tool_menu(menu);

        assert_eq!(state.open_tool_menu, Some(menu), "{menu:?}");
        assert_eq!(state.tool, expected_tool, "{menu:?}");
        assert!(
            !state.query_is_inspect,
            "{menu:?} should leave inspect mode after selecting a build tool"
        );
    }
}

#[test]
fn opening_tool_menu_clears_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    seed_all_modal_dialogs(&mut state);
    seed_all_drag_states(&mut state);

    state.open_tool_menu(ui::ToolMenu::Water);

    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Water));
    assert_eq!(state.tool, EditTool::Pipes);
    assert_modal_dialogs_cleared(&state);
    assert_drag_states_cleared(&state);
}

#[test]
fn selecting_direct_tool_clears_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_tool_menu = Some(ui::ToolMenu::Road);
    seed_all_modal_dialogs(&mut state);
    seed_all_drag_states(&mut state);

    state.select_tool_state(EditTool::Road);

    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.tool, EditTool::Road);
    assert_modal_dialogs_cleared(&state);
    assert_drag_states_cleared(&state);
}

#[test]
fn opening_zone_menu_selects_its_light_tool_default() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.tool = EditTool::ZoneResidential;

    state.open_tool_menu(ui::ToolMenu::Industrial);

    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Industrial));
    assert_eq!(state.tool, EditTool::ZoneIndustrial);
    assert_eq!(zone_id_for_tool(state.tool), Some(5));
}

#[test]
fn opening_power_menu_selects_power_lines_not_power_plant_chooser() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.tool = EditTool::Query;

    state.open_tool_menu(ui::ToolMenu::Power);

    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Power));
    assert_eq!(state.tool, EditTool::Power);
    assert_eq!(state.power_plant_dialog, None);
}

#[test]
fn opening_chooser_dialogs_clear_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_tool_menu = Some(ui::ToolMenu::Power);
    seed_all_modal_dialogs(&mut state);
    state.query_is_inspect = true;
    seed_all_drag_states(&mut state);

    state.open_power_plant_dialog();

    assert_eq!(
        state.power_plant_dialog,
        Some(PowerPlantDialogState {
            info_tool: None,
            pressed_tool: None
        })
    );
    assert_eq!(state.arcology_dialog, None);
    assert_eq!(state.sign_dialog, None);
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.city.label_at(2), Some("Original Sign"));
    assert!(!state.query_is_inspect);
    assert_drag_states_cleared(&state);

    state.open_tool_menu = Some(ui::ToolMenu::Rewards);
    seed_all_modal_dialogs(&mut state);
    state.query_is_inspect = true;
    seed_all_drag_states(&mut state);

    state.open_arcology_dialog();

    assert_eq!(
        state.arcology_dialog,
        Some(ArcologyDialogState {
            info_tool: None,
            pressed_tool: None
        })
    );
    assert_eq!(state.power_plant_dialog, None);
    assert_eq!(state.sign_dialog, None);
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.city.label_at(2), Some("Original Sign"));
    assert!(!state.query_is_inspect);
    assert_drag_states_cleared(&state);
}

#[test]
fn emergency_and_rewards_buttons_are_disabled_by_default() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert!(
        state
            .disabled_palette_actions()
            .contains(&UiAction::OpenMenu(ui::ToolMenu::Emergency))
    );
    assert!(
        state
            .disabled_palette_actions()
            .contains(&UiAction::OpenMenu(ui::ToolMenu::Rewards))
    );
    assert!(!state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Emergency)));
    assert!(!state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Rewards)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::DispatchPolice)));

    state.open_tool_menu(ui::ToolMenu::Emergency);
    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.tool, EditTool::Query);
}

#[test]
fn pe_destroy_building_selector2_highclass_walk_runs_low_endpoint_helpers_in_order() {
    let mut city = City::new_blank(
        "/tmp/sc2000-destroy-sel2-high-dynamic-helpers.sc2",
        "Sel2Helpers",
    )
    .expect("blank city");

    // The initial 0x61 window is class 0x0e (XBIT water-tile), selecting the
    // X axis. Each terminal 0x61 tile has missing XZON corners, so its helper
    // re-enters selector 2 through the low-class fixed-square cleanup.
    for &(x, y) in &[(38, 40), (40, 40), (41, 40), (40, 41), (41, 41), (42, 40)] {
        city.set_building(x, y, 0x61);
        city.set_terrain(x, y, 0);
    }
    for &(x, y) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_zone(x, y, 0xf3);
    }
    city.set_tile_bits(40, 40, TileBits::new(0x04));
    assert_eq!(pe_highway_classify_463c40(&city, 40, 40), 0x0e);
    assert_eq!(pe_highway_classify_463c40(&city, 38, 40), 4);
    assert_eq!(pe_highway_classify_463c40(&city, 42, 40), 4);

    let seed = 0x4590_c061;
    let mut expected = Win95CrtRand::new(seed);
    // Low-class pre-helper destruction: eight scatter draws and four rubble
    // draws. Then five high-walk draws, followed by the same low-class exit
    // helper sequence.
    for _ in 0..29 {
        let _ = expected.random_u15();
    }
    let mut crt = Win95CrtRand::new(seed);
    assert!(pe_flood_destroy_tile_mode_maybe_crt(
        &mut city,
        Some(&mut crt),
        40,
        41,
        true,
    ));
    assert_eq!(
        crt, expected,
        "helper and body draws stay in PE phase order"
    );

    for y in 40..42 {
        for x in 38..40 {
            assert!(
                (1..=4).contains(&city.building_at(x, y)),
                "pre-walk helper leaves the nested low-class rubble at ({x},{y})"
            );
        }
        for x in 40..42 {
            assert_eq!(city.building_at(x, y), 0, "high body pass clears ({x},{y})");
        }
        for x in 42..44 {
            assert!(
                (1..=4).contains(&city.building_at(x, y)),
                "walk-exit helper leaves the nested low-class rubble at ({x},{y})"
            );
        }
    }
    assert_eq!(city.tile_bits_at(40, 40).unwrap().raw_bits(), 0x04);
    for &(x, y) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0x03);
    }
}

#[test]
fn edit_map_stretch_cancel_restores_cursor_401ce9() {
    // PE Stretch exit `0x401ce9(0x1fc)` also runs when the hold is cancelled.
    let mut city =
        City::new_blank("/tmp/sc2000-stretch-cancel.sc2", "StretchCancel").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 5, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::StretchTerrain;
    assert!(state.begin_edit_map_stretch_drag_at(20, 20, 100.0));
    assert_eq!(state.pe_stretch_hold_cursor_id, 0x1fc);
    state.cancel_edit_map_stretch_drag();
    assert_eq!(state.pe_stretch_hold_cursor_id, 0);
    assert_eq!(state.pe_stretch_hold_cursor_arg, 0);
    assert!(!state.edit_map_stretch_drag_active());
}

#[test]
fn edit_map_level_cancel_restores_cursor_401ce9() {
    let mut city = City::new_blank("/tmp/sc2000-level-cancel.sc2", "LevelCancel").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::LevelTerrain;
    assert!(state.begin_edit_map_level_drag_at(20, 20, 100.0));
    assert_eq!(state.pe_level_hold_cursor_id, 0x1fc);
    state.cancel_edit_map_level_drag();
    assert_eq!(state.pe_level_hold_cursor_id, 0);
    assert_eq!(state.pe_level_hold_cursor_arg, 0);
}

#[test]
fn edit_map_lower_cancel_restores_cursor_401ce9() {
    let mut city = City::new_blank("/tmp/sc2000-lower-cancel.sc2", "LowerCancel").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 10, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::LowerTerrain;
    state.pe_process_crt = Win95CrtRand::new(17);
    assert!(state.begin_edit_map_lower_drag_at(20, 20, 100.0));
    assert_eq!(state.pe_lower_hold_cursor_id, 0x1fc);
    state.cancel_edit_map_lower_drag();
    assert_eq!(state.pe_lower_hold_cursor_id, 0);
    assert_eq!(state.pe_lower_hold_cursor_arg, 0);
}

#[test]
fn pe_terrain_tool_ids_match_edit_menu_writes() {
    // PE `0x47deaf`..`0x47def7`: menu slots write Stretch/Lower/Level/NESW-copy ids.
    assert_eq!(PE_TERRAIN_TOOL_STRETCH, 1);
    assert_eq!(PE_TERRAIN_TOOL_LOWER, 2);
    assert_eq!(PE_TERRAIN_TOOL_LEVEL, 3);
    assert_eq!(PE_TERRAIN_TOOL_NESW_COPY, 4);
    // Tool-4 body is `0x442a30`; EditTool::SpreadTerrain binds the UI path.
    let mut city = City::new_blank("/tmp/sc2000-tool4-id.sc2", "Tool4Id").expect("blank");
    for y in 15..26 {
        for x in 15..26 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(20, 19, 4, 0);
    pe_terrain_nesw_land_copy_442a30(&mut city, 20, 20);
    assert_eq!(city.altitude_at(20, 19), 9);
}

#[test]
fn edit_map_raise_midband_refuse_restores_cursor_401ce9() {
    // PE midband refuse @ `0x44348a`: `0x401ce9(0x1fc)` clears hold cursor.
    let mut city =
        City::new_blank("/tmp/sc2000-raise-mid-cursor.sc2", "RaiseMidCur").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_building(20, 20, PE_RAISE_SUBMODE0_MIDBAND_XBLD_MIN);
    city.set_city_funds(1);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::RaiseTerrain;
    state.pe_process_crt = Win95CrtRand::new(19);
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.altitude_at(20, 20), 9);
    assert_eq!(state.pe_raise_hold_cursor_id, 0);
    assert_eq!(state.pe_raise_hold_cursor_arg, 0);
    // PE Raise tool-0 still runs hold UI @ `0x442f15` after midband refuse returns.
    assert_eq!(state.pe_raise_hold_ui_refresh_count, 1);
    assert_eq!(state.pe_raise_hold_blit_count, 1);
    // PE @ `0x4433bf` post-debit wait + hold pair `0x44303e`/`0x443048`.
    assert_eq!(
        state.pe_raise_ui_wait_4012c1_count,
        PE_RAISE_SUBMODE0_POST_DEBIT_UI_WAIT_4012C1 + 2
    );
    // PE @ `0x4434a3`: `0x401096(0x200)` == SOUND_RIOT.
    assert_eq!(crate::audio::SOUND_RIOT, 0x200);
    assert_eq!(state.pe_raise_midband_refuse_sound_401096_count, 1);
    // PE @ `0x4434b4` / `0x4434c0`: dialog + UI thunks with fixed args.
    assert_eq!(state.pe_raise_midband_refuse_dialog_402c4d_count, 1);
    assert_eq!(
        state.pe_raise_midband_refuse_dialog_402c4d_a,
        PE_RAISE_MIDBAND_REFUSE_DIALOG_402C4D_A
    );
    assert_eq!(
        state.pe_raise_midband_refuse_dialog_402c4d_b,
        PE_RAISE_MIDBAND_REFUSE_DIALOG_402C4D_B
    );
    assert_eq!(
        state.pe_raise_midband_refuse_dialog_402c4d_c,
        PE_RAISE_MIDBAND_REFUSE_DIALOG_402C4D_C
    );
    assert_eq!(state.pe_raise_midband_refuse_ui_402900_count, 1);
    assert_eq!(
        state.pe_raise_midband_refuse_ui_402900_a,
        PE_RAISE_MIDBAND_REFUSE_UI_402900_A
    );
    assert_eq!(
        state.pe_raise_midband_refuse_ui_402900_b,
        PE_RAISE_MIDBAND_REFUSE_UI_402900_B
    );
}

#[test]
fn mac_title_generate_and_play_persists_gameplay_run_state_for_both_city_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-title-run-state.sc2", "MacTitle").expect("blank city");
        city.kind = kind;
        city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, 0);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::EditNewMap;

        state.apply_new_city_start_flow(
            title_screen::NewCityStartFlow::GenerateAndPlay,
            MacToolboxRng::DEFAULT_SEED,
        );

        assert_eq!(state.mode, AppMode::CityView);
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(1),
            "{kind:?}"
        );
    }
}

#[test]
fn pe_place_tile_size_writes_microsim_xtxt_inside_helper_like_pe() {
    // PE PlaceTile-size @ `0x440ea0` allocates before stamp and writes XTXT per tile in-loop.
    let mut city = City::new_blank("/tmp/sc2000-pts-xtxt.sc2", "PtsXtxt").expect("blank");
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 210, 3));
    let xtxt = city.text_overlay_at(20, 20).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES);
    for dx in 0..3usize {
        for dy in 0..3usize {
            assert_eq!(city.text_overlay_at(20 + dx, 20 + dy), Some(xtxt));
        }
    }
}

#[test]
fn place_power_lines_skips_outside_crossover_window() {
    // PE @ `0x44a906`–`0x44a90f`: XBLD `>=0x0e` and not `0x1d..=0x4a` → return.
    let mut city = City::new_blank("/tmp/sc2000-pwr-window.sc2", "PwrWin").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x0e);
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 0);
    assert_eq!(state.city.building_at(20, 20), 0x0e);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());

    state.city.set_building(21, 21, 0x4b);
    assert_eq!(state.place_network_at(21, 21, NetworkKind::Power), 0);
    assert_eq!(state.city.building_at(21, 21), 0x4b);
}

#[test]
fn place_power_lines_skips_in_window_non_crossover_slot6() {
    // PE table `0x44a9e8`: in-window XBLD not one of six sources → slot 6 return.
    let mut city = City::new_blank("/tmp/sc2000-pwr-slot6.sc2", "PwrSlot6").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x1f);
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 0);
    assert_eq!(state.city.building_at(20, 20), 0x1f);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());

    // Contrast: table hit `0x1d` → crossover `0x43`.
    state.city.set_building(22, 22, 0x1d);
    assert_eq!(state.place_network_at(22, 22, NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(22, 22), 0x43);
}

#[test]
fn place_roads_skips_outside_crossover_window() {
    // PE @ `0x44ad2e`–`0x44ad3d`: XBLD not `<0x0e` and not table hit → return.
    let mut city = City::new_blank("/tmp/sc2000-road-window.sc2", "RoadWin").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x4d);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 0);
    assert_eq!(state.city.building_at(20, 20), 0x4d);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );

    // Control: power stub `0x0e` → crossover `0x44`.
    state.city.set_building(22, 22, 0x0e);
    assert_eq!(state.place_network_at(22, 22, NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(22, 22), 0x44);
}

#[test]
fn opening_sign_dialog_clears_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_tool_menu = Some(ui::ToolMenu::Power);
    seed_all_modal_dialogs(&mut state);
    state.query_is_inspect = true;
    seed_all_drag_states(&mut state);

    state.open_sign_dialog(1);

    assert_eq!(
        state.sign_dialog.as_ref().map(|dialog| dialog.sign_id),
        Some(1)
    );
    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.power_plant_dialog, None);
    assert_eq!(state.arcology_dialog, None);
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.city.label_at(2), Some("Original Sign"));
    assert!(!state.query_is_inspect);
    assert_drag_states_cleared(&state);
}

#[test]
fn emergency_menu_disables_services_without_available_units() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(30, 40, 0xff);
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    let mut state = test_state(city);

    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::DispatchPolice)));
    for tool in [EditTool::DispatchFirefighters, EditTool::DispatchMilitary] {
        let action = UiAction::SelectTool(tool);
        assert!(!state.ui_action_enabled(action));
        assert!(state.disabled_palette_actions().contains(&action));
    }

    state.open_tool_menu(ui::ToolMenu::Emergency);
    assert_eq!(state.tool, EditTool::DispatchPolice);
}

#[test]
fn reward_mask_enables_only_granted_menu_choices() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_rewards_available_mask(0b1_0100);
    let mut state = test_state(city);

    assert!(state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Rewards)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::MayorHouse)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::CityHall)));
    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::Statue)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::BraunLlamaDome)));
    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::Arcologies)));

    state.open_tool_menu(ui::ToolMenu::Rewards);

    assert_eq!(state.tool, EditTool::Statue);
}

#[test]
fn simulation_month_unlocks_earned_reward_menu_choices() {
    let mut state = test_state(reward_unlock_test_city());

    assert_eq!(state.city.rewards_available_mask(), 0);
    state.handle_simulation_month_advanced();

    assert_eq!(state.city.rewards_available_mask() & 0b0_1111, 0b0_1111);
    assert_eq!(state.city.rewards_available_mask() & 0b1_0000, 0);
    assert_eq!(state.query_line, "Rewards Available");
    assert!(state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Rewards)));
    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::MayorHouse)));
    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::BraunLlamaDome)));

    state.tool = EditTool::MayorHouse;
    assert!(state.apply_tool_at_tile(5, 5));
    assert_eq!(state.city.rewards_available_mask() & 0b0_0001, 0);

    state.handle_simulation_month_advanced();

    assert_eq!(state.city.rewards_available_mask() & 0b0_0001, 0);
}

#[test]
fn reward_unlock_status_does_not_replace_inspect_query() {
    let mut state = test_state(reward_unlock_test_city());
    state.query_line = "Tile 12,34".to_owned();
    state.query_is_inspect = true;

    state.handle_simulation_month_advanced();

    assert_eq!(state.city.rewards_available_mask() & 0b0_1111, 0b0_1111);
    assert_eq!(state.query_line, "Tile 12,34");
}

#[test]
fn opening_rewards_menu_does_not_select_arcology_chooser_as_a_default_tool() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);
    state.tool = EditTool::Query;

    state.open_tool_menu(ui::ToolMenu::Rewards);

    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Rewards));
    assert_eq!(state.tool, EditTool::Query);
    assert_eq!(state.arcology_dialog, None);
    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::Arcologies)));
}

#[test]
fn future_technology_menu_items_are_disabled_in_palette() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_technology_discovery_year(Technology::Highways, 1950);
    city.set_technology_discovery_year(Technology::Buses, 1950);
    city.set_technology_discovery_year(Technology::WaterTreatment, 1950);
    city.set_technology_discovery_year(Technology::Airport, 1950);
    let state = test_state(city);
    let disabled = state.disabled_palette_actions();

    for tool in [
        EditTool::Highway,
        EditTool::Onramp,
        EditTool::BusDepot,
        EditTool::WaterTreatment,
        EditTool::Airport,
    ] {
        assert!(
            disabled.contains(&UiAction::SelectTool(tool)),
            "{tool:?} should draw disabled before discovery"
        );
    }

    for tool in [
        EditTool::Road,
        EditTool::Tunnel,
        EditTool::Pipes,
        EditTool::WaterPump,
        EditTool::Seaport,
    ] {
        assert!(
            !disabled.contains(&UiAction::SelectTool(tool)),
            "{tool:?} should remain enabled"
        );
    }
}

#[test]
fn refresh_network_tile_uses_place_tile_helper() {
    // PE PlaceTile (`0x441f00`) reshape stamps XBLD; leave XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-net-refresh-place.sc2", "NetPlace").expect("blank");
    city.set_city_funds(20_000);
    let path = vec![(38, 40), (39, 40), (40, 40), (41, 40)];
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 4);
    // Wrong orientation on the middle tile — refresh must PlaceTile the mask result.
    state.city.set_building(40, 40, 31); // NS stub vs EW neighbors
    state.city.set_traffic_density(40, 40, 0x2d);
    let before = state.city.building_at(40, 40);
    state.refresh_network_tile(40, 40, NetworkKind::Road);
    let after = state.city.building_at(40, 40);
    assert!(NetworkKind::Road.contains(after));
    assert_ne!(after, before);
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x2d));
}

#[test]
fn sign_dialog_edits_caps_and_restores_user_label_text() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.tool = EditTool::Sign;

    assert!(state.place_sign_at(10, 10));
    assert_eq!(state.city.text_overlay_at(10, 10), Some(1));
    assert_eq!(
        state.sign_dialog.as_ref().map(|dialog| dialog.sign_id),
        Some(1)
    );

    state
        .sign_dialog
        .as_mut()
        .expect("sign dialog")
        .text
        .clear();
    state.append_sign_dialog_text("Main Street");
    state.commit_sign_dialog();
    assert_eq!(state.city.label_at(1), Some("Main Street"));
    assert!(state.sign_dialog.is_none());

    state.open_sign_dialog(1);
    state
        .sign_dialog
        .as_mut()
        .expect("sign dialog")
        .text
        .clear();
    state.append_sign_dialog_text("123456789012345678901234567890");
    assert_eq!(
        state.sign_dialog.as_ref().expect("sign dialog").text.len(),
        SIGN_LABEL_MAX_BYTES
    );
    state.cancel_sign_dialog();
    assert_eq!(state.city.label_at(1), Some("Main Street"));
    assert!(state.sign_dialog.is_none());
}

#[test]
fn palette_surface_tools_map_to_sc2k_building_ids() {
    let cases = [
        (EditTool::Trees, 6),
        (EditTool::BusDepot, 236),
        (EditTool::RailDepot, 237),
        (EditTool::SubStation, 233),
        (EditTool::SmallPark, 13),
        (EditTool::BigPark, 213),
        (EditTool::Zoo, 218),
        (EditTool::Stadium, 215),
        (EditTool::Marina, 248),
        (EditTool::PowerPlantCoal, 207),
        (EditTool::PowerPlantHydro, 198),
        (EditTool::PowerPlantOil, 202),
        (EditTool::PowerPlantGas, 201),
        (EditTool::PowerPlantNuclear, 203),
        (EditTool::PowerPlantWind, 200),
        (EditTool::PowerPlantSolar, 204),
        (EditTool::PowerPlantMicrowave, 205),
        (EditTool::PowerPlantFusion, 206),
        (EditTool::Police, 210),
        (EditTool::FireStation, 211),
        (EditTool::Hospital, 209),
        (EditTool::Prison, 216),
        (EditTool::School, 214),
        (EditTool::College, 217),
        (EditTool::Library, 245),
        (EditTool::Museum, 212),
        (EditTool::WaterPump, 220),
        (EditTool::WaterTower, 235),
        (EditTool::WaterTreatment, 244),
        (EditTool::Desalinization, 250),
        (EditTool::MayorHouse, 243),
        (EditTool::CityHall, 208),
        (EditTool::Statue, 219),
        (EditTool::BraunLlamaDome, 255),
        (EditTool::ArcologyPlymouth, 251),
        (EditTool::ArcologyForest, 252),
        (EditTool::ArcologyDarco, 253),
        (EditTool::ArcologyLaunch, 254),
    ];

    for (tool, building) in cases {
        assert_eq!(
            placeable_building_for_tool(tool),
            Some(building),
            "{tool:?}"
        );
    }
    assert_eq!(placeable_building_for_tool(EditTool::Onramp), None);
    assert_eq!(placeable_building_for_tool(EditTool::SubToRail), None);
    assert_eq!(placeable_building_for_tool(EditTool::Pipes), None);
    assert_eq!(placeable_building_for_tool(EditTool::Subway), None);
    assert_eq!(placeable_building_for_tool(EditTool::Highway), None);
    assert_eq!(placeable_building_for_tool(EditTool::Tunnel), None);
    assert_eq!(placeable_building_for_tool(EditTool::PowerPlant), None);
    assert_eq!(placeable_building_for_tool(EditTool::Arcologies), None);
}

#[test]
fn chooser_palette_tools_do_not_place_default_buildings_directly() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    for tool in [EditTool::PowerPlant, EditTool::Arcologies] {
        state.tool = tool;

        assert_eq!(placeable_building_for_tool(tool), None, "{tool:?}");
        assert_eq!(tool_build_cost(tool), None, "{tool:?}");
        assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
        assert!(
            !state.apply_tool_at_tile(20, 20),
            "{tool:?} should require a dialog choice before placement"
        );
    }
}

#[test]
fn power_plant_palette_choices_use_sc2k_tile_sizes() {
    let cases = [
        (EditTool::PowerPlantWind, 200, 1),
        (EditTool::PowerPlantHydro, 198, 1),
        (EditTool::PowerPlantGas, 201, 4),
        (EditTool::PowerPlantOil, 202, 4),
        (EditTool::PowerPlantNuclear, 203, 4),
        (EditTool::PowerPlantSolar, 204, 4),
        (EditTool::PowerPlantMicrowave, 205, 4),
        (EditTool::PowerPlantFusion, 206, 4),
        (EditTool::PowerPlantCoal, 207, 4),
    ];

    for (tool, building, size) in cases {
        assert_eq!(
            placeable_building_for_tool(tool),
            Some(building),
            "{tool:?}"
        );
        assert_eq!(xbld_tile_size(building), size, "{tool:?}");
    }
}

#[test]
fn tool_build_cost_covers_observed_palette_build_items() {
    let cases = [
        (EditTool::Bulldoze, Some(1)),
        (EditTool::RaiseTerrain, Some(25)),
        (EditTool::LowerTerrain, Some(25)),
        (EditTool::StretchTerrain, Some(25)),
        (EditTool::LevelTerrain, Some(25)),
        (EditTool::SpreadTerrain, Some(25)),
        (EditTool::Trees, Some(3)),
        (EditTool::Forest, Some(3)),
        (EditTool::Water, Some(100)),
        (EditTool::Stream, Some(100)),
        (EditTool::ZoneResidential, Some(5)),
        (EditTool::ZoneResidentialDense, Some(10)),
        (EditTool::ZoneCommercial, Some(5)),
        (EditTool::ZoneCommercialDense, Some(10)),
        (EditTool::ZoneIndustrial, Some(5)),
        (EditTool::ZoneIndustrialDense, Some(10)),
        (EditTool::DeZone, Some(5)),
        (EditTool::Tunnel, Some(150)),
        (EditTool::Onramp, Some(25)),
        (EditTool::BusDepot, Some(250)),
        (EditTool::RailDepot, Some(500)),
        (EditTool::SubStation, Some(250)),
        (EditTool::SubToRail, Some(250)),
        (EditTool::SmallPark, Some(20)),
        (EditTool::BigPark, Some(150)),
        (EditTool::Zoo, Some(3_000)),
        (EditTool::Stadium, Some(5_000)),
        (EditTool::Marina, Some(1_000)),
        (EditTool::PowerPlant, None),
        (EditTool::PowerPlantCoal, Some(4_000)),
        (EditTool::PowerPlantHydro, Some(400)),
        (EditTool::PowerPlantOil, Some(6_600)),
        (EditTool::PowerPlantGas, Some(2_000)),
        (EditTool::PowerPlantNuclear, Some(15_000)),
        (EditTool::PowerPlantWind, Some(100)),
        (EditTool::PowerPlantSolar, Some(1_300)),
        (EditTool::PowerPlantMicrowave, Some(28_000)),
        (EditTool::PowerPlantFusion, Some(40_000)),
        (EditTool::Police, Some(500)),
        (EditTool::FireStation, Some(500)),
        (EditTool::Hospital, Some(500)),
        (EditTool::Prison, Some(3_000)),
        (EditTool::School, Some(250)),
        (EditTool::College, Some(1_000)),
        (EditTool::Library, Some(500)),
        (EditTool::Museum, Some(1_000)),
        (EditTool::WaterPump, Some(100)),
        (EditTool::WaterTower, Some(250)),
        (EditTool::WaterTreatment, Some(500)),
        (EditTool::Desalinization, Some(1_000)),
        (EditTool::Seaport, Some(150)),
        (EditTool::Airport, Some(250)),
        (EditTool::MayorHouse, Some(0)),
        (EditTool::CityHall, Some(0)),
        (EditTool::Statue, Some(0)),
        (EditTool::BraunLlamaDome, Some(0)),
        (EditTool::Arcologies, None),
        (EditTool::ArcologyPlymouth, Some(100_000)),
        (EditTool::ArcologyForest, Some(120_000)),
        (EditTool::ArcologyDarco, Some(150_000)),
        (EditTool::ArcologyLaunch, Some(200_000)),
        (EditTool::Pipes, None),
        (EditTool::Subway, None),
        (EditTool::Highway, None),
    ];

    for (tool, expected) in cases {
        assert_eq!(tool_build_cost(tool), expected, "{tool:?}");
    }
}

#[test]
fn every_menu_tool_routes_to_a_city_action() {
    for menu in ALL_CITY_TOOL_MENUS {
        for item in ui::tool_menu_items(*menu) {
            assert!(
                tool_routes_to_city_action(item.tool),
                "{menu:?} {:?} has a visible menu entry but no city action route",
                item.tool
            );
        }
    }
}

#[test]
fn palette_building_tools_place_through_apply_path() {
    let cases = [
        (EditTool::BusDepot, 236),
        (EditTool::RailDepot, 237),
        (EditTool::SubStation, 233),
        (EditTool::SmallPark, 13),
        (EditTool::BigPark, 213),
        (EditTool::Zoo, 218),
        (EditTool::Stadium, 215),
        (EditTool::Marina, 248),
        (EditTool::PowerPlantCoal, 207),
        (EditTool::PowerPlantHydro, 198),
        (EditTool::PowerPlantOil, 202),
        (EditTool::PowerPlantGas, 201),
        (EditTool::PowerPlantNuclear, 203),
        (EditTool::PowerPlantWind, 200),
        (EditTool::PowerPlantSolar, 204),
        (EditTool::PowerPlantMicrowave, 205),
        (EditTool::PowerPlantFusion, 206),
        (EditTool::Police, 210),
        (EditTool::FireStation, 211),
        (EditTool::Hospital, 209),
        (EditTool::Prison, 216),
        (EditTool::School, 214),
        (EditTool::College, 217),
        (EditTool::Library, 245),
        (EditTool::Museum, 212),
        (EditTool::WaterPump, 220),
        (EditTool::WaterTower, 235),
        (EditTool::WaterTreatment, 244),
        (EditTool::Desalinization, 250),
        (EditTool::MayorHouse, 243),
        (EditTool::CityHall, 208),
        (EditTool::Statue, 219),
        (EditTool::BraunLlamaDome, 255),
        (EditTool::ArcologyPlymouth, 251),
        (EditTool::ArcologyForest, 252),
        (EditTool::ArcologyDarco, 253),
        (EditTool::ArcologyLaunch, 254),
    ];

    for (tool, building) in cases {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(500_000);
        let origin = (20, 20);
        match tool {
            EditTool::Marina => {
                for x in origin.0..origin.0 + 3 {
                    city.set_terrain(x, origin.1, 0x30);
                    city.set_tile_bits(x, origin.1, TileBits::new(0).with_water_tile(true));
                }
            }
            EditTool::PowerPlantHydro => {
                city.set_terrain(origin.0, origin.1, 0x3e);
            }
            EditTool::Desalinization => {
                city.set_terrain(origin.0 - 1, origin.1, 0x30);
                city.set_tile_bits(
                    origin.0 - 1,
                    origin.1,
                    crate::city::TileBits::new(0b0000_0001),
                );
            }
            _ => {}
        }
        if let Some(mask) = reward_mask_for_tool(tool) {
            city.set_rewards_available_mask(mask);
        }
        let mut state = test_state(city);
        state.tool = tool;
        let size = usize::from(xbld_tile_size(building));
        let placement = state
            .tool_placement_at(origin.0, origin.1)
            .expect("tool placement");
        let expected_building = match (tool, placement.preview) {
            (EditTool::PowerPlantHydro, ToolPreview::Building(id @ (198 | 199))) => id,
            (_, ToolPreview::Building(id)) => id,
            _ => panic!("{tool:?} should preview a building"),
        };

        if tool == EditTool::PowerPlantHydro {
            assert!(matches!(expected_building, 198 | 199));
        } else {
            assert_eq!(expected_building, building, "{tool:?}");
        }
        assert_eq!(placement.valid_tiles.len(), size * size, "{tool:?}");
        assert!(placement.invalid_tiles.is_empty(), "{tool:?}");
        assert!(state.apply_tool_at_tile(origin.0, origin.1), "{tool:?}");
        assert_eq!(
            state.city.city_funds(),
            500_000 - tool_build_cost(tool).unwrap_or(0) as i32,
            "{tool:?}"
        );

        for (x, y) in
            building_footprint_from_origin(origin.0, origin.1, size).expect("building footprint")
        {
            assert_eq!(state.city.building_at(x, y), expected_building, "{tool:?}");
            if building_has_internal_pipes(expected_building) {
                assert!(
                    state
                        .city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.piped()),
                    "{tool:?} footprint tile ({x}, {y}) should have building plumbing"
                );
            }
        }
    }
}

#[test]
fn palette_building_tools_preview_as_dark_footprints_not_final_art() {
    let origin = (20, 20);
    for tool in [
        EditTool::BusDepot,
        EditTool::RailDepot,
        EditTool::SubStation,
        EditTool::SmallPark,
        EditTool::BigPark,
        EditTool::Zoo,
        EditTool::Stadium,
        EditTool::Marina,
        EditTool::PowerPlantCoal,
        EditTool::PowerPlantHydro,
        EditTool::PowerPlantOil,
        EditTool::PowerPlantGas,
        EditTool::PowerPlantNuclear,
        EditTool::PowerPlantWind,
        EditTool::PowerPlantSolar,
        EditTool::PowerPlantMicrowave,
        EditTool::PowerPlantFusion,
        EditTool::Police,
        EditTool::FireStation,
        EditTool::Hospital,
        EditTool::Prison,
        EditTool::School,
        EditTool::College,
        EditTool::Library,
        EditTool::Museum,
        EditTool::WaterPump,
        EditTool::WaterTower,
        EditTool::WaterTreatment,
        EditTool::Desalinization,
        EditTool::MayorHouse,
        EditTool::CityHall,
        EditTool::Statue,
        EditTool::BraunLlamaDome,
        EditTool::ArcologyPlymouth,
        EditTool::ArcologyForest,
        EditTool::ArcologyDarco,
        EditTool::ArcologyLaunch,
    ] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        if tool == EditTool::Marina {
            for x in origin.0..origin.0 + 3 {
                city.set_terrain(x, origin.1, 0x30);
                // PE marina counts XBIT water-tile, not bare water XTER.
                city.set_tile_bits(x, origin.1, TileBits::new(0).with_water_tile(true));
            }
        }
        if tool == EditTool::PowerPlantHydro {
            city.set_terrain(origin.0, origin.1, 0x3e);
        }
        if tool == EditTool::Desalinization {
            city.set_terrain(origin.0 - 1, origin.1, 0x30);
            city.set_tile_bits(
                origin.0 - 1,
                origin.1,
                crate::city::TileBits::new(0b0000_0001),
            );
        }
        if let Some(mask) = reward_mask_for_tool(tool) {
            city.set_rewards_available_mask(mask);
        }
        let mut state = test_state(city);
        state.tool = tool;

        let placement = state
            .tool_placement_at(origin.0, origin.1)
            .expect("tool placement");

        assert!(
            matches!(placement.preview, ToolPreview::Building(_)),
            "{tool:?}"
        );
        assert!(placement.invalid_tiles.is_empty(), "{tool:?}");
        assert!(
            !placement.preview.draws_valid_diamond(),
            "{tool:?} should use the dark footprint preview branch"
        );
        assert!(
            placement.preview.zone_sprite_id().is_none(),
            "{tool:?} should not use a zone/final-art preview sprite"
        );
    }
}

#[test]
fn pe_newspaper_gift_place_nuclear_483aab_place_tile_size() {
    let mut city = City::new_blank("/tmp/sc2000-gift-nuke.sc2", "GiftNuke").expect("blank");
    city.set_city_funds(50_000);
    assert!(pe_newspaper_gift_place_nuclear_483aab(&mut city, 20, 20));
    assert_eq!(city.building_at(19, 19), NUCLEAR_POWER_BUILDING_ID);
    assert_eq!(city.building_at(20, 20), NUCLEAR_POWER_BUILDING_ID);
}

#[test]
fn pe_newspaper_gift_shore_prep_484320_sets_water_land1_trees() {
    // PE @ `0x484320`: XBIT|=water_tile, ALTM land→1, PlaceTile XBLD=0x10.
    let mut city = City::new_blank("/tmp/sc2000-gift-shore.sc2", "GiftShore").expect("blank");
    city.set_altitude(20, 20, 9, 3);
    city.set_building(20, 20, 0);
    city.set_tile_bits(20, 20, TileBits::new(0));
    let vacant = city.pe_dw_tile_count[0];
    pe_newspaper_gift_shore_prep_484320(&mut city, 20, 20);
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
    assert_eq!(city.altitude_at(20, 20), 1);
    assert_eq!(city.altm_at(20, 20).water, 3);
    assert_eq!(city.building_at(20, 20), 0x10);
    assert_eq!(city.pe_dw_tile_count[0x10], 1);
    assert_eq!(city.pe_dw_tile_count[0], vacant.wrapping_sub(1));
}

#[test]
fn pe_newspaper_gift_place_bridge_4838a9_places_rotated_bridge_tiles() {
    // PE @ `0x4838a9`: shore-prep (x,y), PlaceTile 0x57 at (x+1,y+1) and (x+2,y+1)
    // with XBIT rotate.
    let mut city = City::new_blank("/tmp/sc2000-gift-bridge.sc2", "GiftBr").expect("blank");
    pe_newspaper_gift_place_road_stub_483874(&mut city, 8, 8);
    assert_eq!(city.building_at(8, 8), 0x0d);
    pe_newspaper_gift_place_bridge_4838a9(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0x10);
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
    assert_eq!(city.building_at(21, 21), 0x57);
    assert!(city.tile_bits_at(21, 21).expect("bits").rotate());
    assert_eq!(city.building_at(22, 21), 0x57);
    assert!(city.tile_bits_at(22, 21).expect("bits").rotate());
}

#[test]
fn pe_newspaper_gift_place_bridge_same_tile_483a25_483a43() {
    // PE @ `0x483a25` plain / `0x483a43` rotated same-tile bridge gifts.
    let mut city = City::new_blank("/tmp/sc2000-gift-bridge2.sc2", "GiftBr2").expect("blank");
    pe_newspaper_gift_place_bridge_plain_483a25(&mut city, 30, 30);
    assert_eq!(city.building_at(30, 30), 0x57);
    assert!(!city.tile_bits_at(30, 30).expect("bits").rotate());
    assert!(city.tile_bits_at(30, 30).expect("bits").water_tile());
    pe_newspaper_gift_place_bridge_rotated_483a43(&mut city, 40, 40);
    assert_eq!(city.building_at(40, 40), 0x57);
    assert!(city.tile_bits_at(40, 40).expect("bits").rotate());
    assert!(city.tile_bits_at(40, 40).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_place_bridge_offset_48394d_4839c3_4839f0() {
    let mut city = City::new_blank("/tmp/sc2000-gift-bridge3.sc2", "GiftBr3").expect("blank");
    // PE @ `0x48394d`: prep (20,20), bridge at (19,21) rotated.
    pe_newspaper_gift_place_bridge_48394d(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0x10);
    assert_eq!(city.building_at(19, 21), 0x57);
    assert!(city.tile_bits_at(19, 21).expect("bits").rotate());
    // PE @ `0x4839c3`: prep (40,40), bridge at (41,39) plain.
    pe_newspaper_gift_place_bridge_4839c3(&mut city, 40, 40);
    assert_eq!(city.building_at(41, 39), 0x57);
    assert!(!city.tile_bits_at(41, 39).expect("bits").rotate());
    // PE @ `0x4839f0`: prep (60,60), bridges at (61,61) and (61,62).
    pe_newspaper_gift_place_bridge_4839f0(&mut city, 60, 60);
    assert_eq!(city.building_at(61, 61), 0x57);
    assert_eq!(city.building_at(61, 62), 0x57);
}

#[test]
fn pe_newspaper_gift_place_table_tile_483412_uses_4ea9d8() {
    let mut city = City::new_blank("/tmp/sc2000-gift-table.sc2", "GiftTbl").expect("blank");
    // index 2 → tile 29, no shore/rotate.
    pe_newspaper_gift_place_table_tile_483412(&mut city, 10, 10, 2);
    assert_eq!(city.building_at(10, 10), 29);
    assert!(!city.tile_bits_at(10, 10).expect("bits").rotate());
    // index 1 → shore-prep then tile 87 + rotate.
    pe_newspaper_gift_place_table_tile_483412(&mut city, 20, 20, 1);
    assert_eq!(city.building_at(20, 20), 87);
    assert!(city.tile_bits_at(20, 20).expect("bits").rotate());
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
    // index 0xd → OR 0xc0 then tile 67.
    pe_newspaper_gift_place_table_tile_483412(&mut city, 30, 30, 0xd);
    assert_eq!(city.building_at(30, 30), 67);
    assert_eq!(
        city.tile_bits_at(30, 30).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_place_table_tile_4834bc_uses_4ea9e8() {
    let mut city = City::new_blank("/tmp/sc2000-gift-table2.sc2", "GiftTb2").expect("blank");
    pe_newspaper_gift_place_table_tile_4834bc(&mut city, 10, 10, 2);
    assert_eq!(city.building_at(10, 10), 14);
    assert_eq!(
        city.tile_bits_at(10, 10).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
    assert!(!city.tile_bits_at(10, 10).expect("bits").rotate());
    pe_newspaper_gift_place_table_tile_4834bc(&mut city, 20, 20, 0);
    assert_eq!(city.building_at(20, 20), 92);
    assert!(city.tile_bits_at(20, 20).expect("bits").rotate());
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_place_table_tile_48358a_uses_4ea9f8() {
    let mut city = City::new_blank("/tmp/sc2000-gift-table3.sc2", "GiftTb3").expect("blank");
    pe_newspaper_gift_place_table_tile_48358a(&mut city, 10, 10, 2);
    assert_eq!(city.building_at(10, 10), 44);
    pe_newspaper_gift_place_table_tile_48358a(&mut city, 20, 20, 1);
    assert_eq!(city.building_at(20, 20), 90);
    assert!(city.tile_bits_at(20, 20).expect("bits").rotate());
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_zone_power_48362a_sets_type9_and_power_bits() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zone.sc2", "GiftZn").expect("blank");
    city.set_zone(15, 15, 0xf2); // corners + type 2
    city.set_tile_bits(15, 15, TileBits::new(0));
    pe_newspaper_gift_zone_power_48362a(&mut city, 15, 15);
    assert_eq!(city.zone_at(15, 15).expect("zone").raw_bits(), 0xf9);
    assert_eq!(
        city.tile_bits_at(15, 15).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_zone_power_483aca_sets_type8_and_power_bits() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zone8.sc2", "GiftZn8").expect("blank");
    city.set_zone(15, 15, 0xf3);
    city.set_tile_bits(15, 15, TileBits::new(0));
    pe_newspaper_gift_zone_power_483aca(&mut city, 15, 15);
    assert_eq!(city.zone_at(15, 15).expect("zone").raw_bits(), 0xf8);
    assert_eq!(
        city.tile_bits_at(15, 15).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_zone_type_power_483b48_picks_type_from_story() {
    let mut city = City::new_blank("/tmp/sc2000-gift-ztype.sc2", "GiftZT").expect("blank");
    city.set_zone(10, 10, 0xa0);
    pe_newspaper_gift_zone_type_power_483b48(&mut city, 10, 10, 0x100);
    assert_eq!(city.zone_at(10, 10).expect("z").raw_bits(), 0xa2);
    city.set_zone(20, 20, 0xb0);
    pe_newspaper_gift_zone_type_power_483b48(&mut city, 20, 20, 0x200);
    assert_eq!(city.zone_at(20, 20).expect("z").raw_bits(), 0xb4);
    city.set_zone(30, 30, 0xc0);
    pe_newspaper_gift_zone_type_power_483b48(&mut city, 30, 30, 0x300);
    assert_eq!(city.zone_at(30, 30).expect("z").raw_bits(), 0xc6);
    assert_eq!(
        city.tile_bits_at(30, 30).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_place_hospital_church_483df1_483e14() {
    let mut city = City::new_blank("/tmp/sc2000-gift-hc.sc2", "GiftHC").expect("blank");
    city.set_city_funds(50_000);
    assert!(pe_newspaper_gift_place_hospital_483df1(&mut city, 20, 20));
    // Size 3: PE click (20,20) → post-dec NW (19,19).
    assert_eq!(city.building_at(19, 19), HOSPITAL_BUILDING_ID);
    assert_eq!(city.building_at(20, 20), HOSPITAL_BUILDING_ID);
    assert!(pe_newspaper_gift_place_church_483e14(&mut city, 40, 40));
    assert_eq!(city.building_at(40, 40), CHURCH_BUILDING_ID);
}

#[test]
fn pe_newspaper_gift_mark_zone_corners_436f5f_ors_f0() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zcorn.sc2", "GiftZC").expect("blank");
    city.set_zone(10, 10, 0x03);
    pe_newspaper_gift_mark_zone_corners_436f5f(&mut city, 10, 10);
    assert_eq!(city.zone_at(10, 10).expect("z").raw_bits(), 0xf3);
}

#[test]
fn pe_newspaper_gift_place_rubble_483e37_on_empty_only() {
    let mut city = City::new_blank("/tmp/sc2000-gift-rubble.sc2", "GiftRub").expect("blank");
    let mut crt = Win95CrtRand::new(7);
    city.set_building(12, 12, 0x70);
    pe_newspaper_gift_place_rubble_483e37(&mut city, &mut crt, 12, 12);
    assert_eq!(city.building_at(12, 12), 0x70);
    pe_newspaper_gift_place_rubble_483e37(&mut city, &mut crt, 13, 13);
    let b = city.building_at(13, 13);
    assert!((1..=4).contains(&b), "building {b}");
}

#[test]
fn pe_newspaper_gift_place_trees_4832d9_4832ff_ranges() {
    let mut city = City::new_blank("/tmp/sc2000-gift-trees.sc2", "GiftTrees").expect("blank");
    let mut crt = Win95CrtRand::new(11);
    pe_newspaper_gift_place_trees_4832d9(&mut city, &mut crt, 16, 16);
    let soft = city.building_at(16, 16);
    assert!((6..=9).contains(&soft), "soft {soft}");
    pe_newspaper_gift_place_trees_4832ff(&mut city, &mut crt, 17, 17);
    let hard = city.building_at(17, 17);
    assert!((9..=12).contains(&hard), "hard {hard}");
}

#[test]
fn pe_newspaper_gift_altm_prep_483287_forces_water_2() {
    let mut city = City::new_blank("/tmp/sc2000-gift-altm.sc2", "GiftAltm").expect("blank");
    city.set_altitude(18, 18, 7, 15);
    city.set_tunnel_levels(18, 18, 3);
    pe_newspaper_gift_altm_prep_483287(&mut city, 18, 18);
    let sample = city.altm_at(18, 18);
    assert_eq!(sample.land, 7);
    assert_eq!(sample.water, 2);
    assert_eq!(city.tunnel_levels_at(18, 18), 3);
}

#[test]
fn pe_newspaper_gift_shore_only_4832c1_runs_shore_prep() {
    let mut city = City::new_blank("/tmp/sc2000-gift-shore-only.sc2", "GiftShore").expect("blank");
    city.set_altitude(19, 19, 5, 0);
    pe_newspaper_gift_shore_only_4832c1(&mut city, 19, 19);
    assert_eq!(city.building_at(19, 19), 0x10);
    assert_eq!(city.altm_at(19, 19).land, 1);
    assert!(city.tile_bits_at(19, 19).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_entry_483250_altm_gate_and_early_arms() {
    let mut city = City::new_blank("/tmp/sc2000-gift-entry.sc2", "GiftEntry").expect("blank");
    let mut crt = Win95CrtRand::new(13);
    city.set_altitude(20, 20, 4, 9);
    // story < 2: ALTM only.
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 20, 20, 1);
    assert_eq!(city.altm_at(20, 20).water, 2);
    assert_eq!(city.building_at(20, 20), 0);
    // soft trees.
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 21, 21, 0x15);
    let soft = city.building_at(21, 21);
    assert!((6..=9).contains(&soft), "soft {soft}");
    // force-tree story 0x34.
    city.set_building(22, 22, 0x70);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 22, 22, 0x34);
    assert_eq!(city.building_at(22, 22), 5);
}

#[test]
fn pe_newspaper_gift_entry_483250_network_table_bands() {
    let mut city = City::new_blank("/tmp/sc2000-gift-entry-tbl.sc2", "GiftEntryT").expect("blank");
    let mut crt = Win95CrtRand::new(14);
    // 0x42 → 4ea9d8[2] == 29
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 24, 24, 0x42);
    assert_eq!(city.building_at(24, 24), 29);
    // 0xd2 → 4ea9e8[2] == 14
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 25, 25, 0xd2);
    assert_eq!(city.building_at(25, 25), 14);
    // 0xe2 → 4ea9f8[2] == 44
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 26, 26, 0xe2);
    assert_eq!(city.building_at(26, 26), 44);
}

#[test]
fn pe_newspaper_gift_entry_483250_zone_power_and_placetile_size() {
    let mut city = City::new_blank("/tmp/sc2000-gift-entry-zps.sc2", "GiftEntryZ").expect("blank");
    let mut crt = Win95CrtRand::new(15);
    city.set_city_funds(50_000);
    // zone-power type9
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 30, 30, 0x2b5);
    assert_eq!(city.zone_at(30, 30).expect("z").raw_bits() & 0x0f, 9);
    assert_eq!(
        city.tile_bits_at(30, 30).expect("b").raw_bits() & 0xc0,
        0xc0
    );
    // zone-power type8
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 31, 31, 0x2c5);
    assert_eq!(city.zone_at(31, 31).expect("z").raw_bits() & 0x0f, 8);
    // fire story 0x2fd — size 3 post-dec NW (31,31)
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 32, 32, 0x2fd);
    assert_eq!(city.building_at(31, 31), FIRE_STATION_BUILDING_ID);
    assert_eq!(city.building_at(32, 32), FIRE_STATION_BUILDING_ID);
    // coal story 0x2f3: caller dec then size-4 dec → NW (38,38)
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 40, 40, 0x2f3);
    assert_eq!(city.building_at(38, 38), COAL_POWER_BUILDING_ID);
    assert_eq!(city.building_at(39, 39), COAL_POWER_BUILDING_ID);
    // road stub
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 33, 33, 0x348);
    assert_eq!(city.building_at(33, 33), 0x0d);
}

#[test]
fn pe_newspaper_gift_entry_483250_zone_type_paint_and_bridge() {
    let mut city = City::new_blank("/tmp/sc2000-gift-entry-ztp.sc2", "GiftEntryP").expect("blank");
    let mut crt = Win95CrtRand::new(16);
    city.set_city_funds(50_000);
    // story 0xf0: zone-type only (ax=0), nibble 2.
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 34, 34, 0xf0);
    assert_eq!(city.zone_at(34, 34).expect("z").raw_bits() & 0x0f, 2);
    assert_eq!(city.building_at(34, 34), 0);
    // story 0xf9: zone-type 2 then size-1 paint with type (2-1)/2=0.
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 35, 35, 0xf9);
    assert_eq!(city.zone_at(35, 35).expect("z").raw_bits() & 0xf0, 0xf0);
    let b = city.building_at(35, 35);
    assert!((0x70..0x70 + 0x0c).contains(&u16::from(b)), "building {b}");
    // plain bridge
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 36, 36, 0x33c);
    assert_eq!(city.building_at(36, 36), 0x57);
    assert!(city.tile_bits_at(36, 36).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_entry_483250_nuclear_stadium_twin_size4_zonetype() {
    // PE nuclear exact `0x32b` @ `0x483aa0`; stadium twin `0x31b` @ `0x48375c`;
    // size-4 paint story `0x295` (`0x26d` band ax=4 paint=4); high zone-type-only
    // `0x374` (selector ax=0 after type 6).
    let mut city =
        City::new_blank("/tmp/sc2000-gift-entry-ns4.sc2", "GiftEntryNS4").expect("blank");
    let mut crt = Win95CrtRand::new(19);
    city.set_city_funds(50_000);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 40, 40, 0x32b);
    // Nuclear click (41,38) → NW (40,37).
    assert_eq!(city.building_at(40, 37), NUCLEAR_POWER_BUILDING_ID);
    assert_eq!(city.building_at(41, 38), NUCLEAR_POWER_BUILDING_ID);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 60, 60, 0x31b);
    assert_eq!(city.building_at(60, 57), STADIUM_BUILDING_ID);
    assert_eq!(city.building_at(61, 58), STADIUM_BUILDING_ID);
    assert_eq!(city.pe_w_sports_teams(), 1);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 50, 50, 0x295);
    assert_eq!(city.zone_at(50, 50).expect("ind").raw_bits() & 0x0f, 6);
    // size4 paints from (49,51); type4 base 0xc4.
    let b = city.building_at(49, 51);
    assert!(
        (0xc4..0xc4 + 2).contains(&u16::from(b)),
        "size4 building {b}"
    );
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 70, 70, 0x374);
    assert_eq!(city.zone_at(70, 70).expect("t6").raw_bits() & 0x0f, 6);
    assert_eq!(city.building_at(70, 70), 0, "0x374 paints nothing");
    assert_eq!(
        city.tile_bits_at(70, 70).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_place_tree_483e68_destroys_then_places_5() {
    let mut city = City::new_blank("/tmp/sc2000-gift-tree.sc2", "GiftTree").expect("blank");
    city.set_building(14, 14, 0x70);
    let mut crt = Win95CrtRand::new(1);
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 14, 14);
    assert_eq!(city.building_at(14, 14), 5);
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 15, 15);
    assert_eq!(city.building_at(15, 15), 5);
}

#[test]
fn pe_newspaper_gift_place_tree_483e68_destroy_building_consumes_crt_rubble() {
    // PE DestroyBuilding mode 0 @ `0x483e7f` bypasses mode-1 scatter at
    // `0x459c40`, but still makes the conditional CRT PlaceTile rubble draw
    // @ `0x45a01f` before final PlaceTile `5`.
    let mut city = City::new_blank("/tmp/sc2000-gift-tree-crt.sc2", "GiftTCrt").expect("blank");
    city.set_building(14, 14, 0x70);
    let mut preview = Win95CrtRand::new(0x483e_6801);
    let _rubble = preview.rubble_tile_id();
    let mut crt = Win95CrtRand::new(0x483e_6801);
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 14, 14);
    assert_eq!(city.building_at(14, 14), 5);
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn pe_newspaper_gift_place_tree_483e68_elevated_destroy_building_then_5() {
    // PE @ `0x483e7f`: DestroyBuilding mode 0 — elevated arm then PlaceTile `5`.
    let mut city = City::new_blank("/tmp/sc2000-gift-tree-elev.sc2", "GiftTE").expect("blank");
    city.set_building(14, 14, 0x3f);
    city.set_zone(14, 14, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    city.set_tunnel_levels(14, 14, 0x1f);
    let mut crt = Win95CrtRand::new(1);
    let seed_before = crt.seed();
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 14, 14);
    assert_eq!(city.building_at(14, 14), 5);
    assert_eq!(
        city.zone_at(14, 14).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0
    );
    assert_eq!(city.tunnel_levels_at(14, 14), 0);
    assert_eq!(crt.seed(), seed_before, "elevated arm does not consume CRT");
}

#[test]
fn pe_newspaper_gift_place_tree_483e68_xbld_lt6_skips_destroy_building() {
    // PE DestroyBuilding `cmp XBLD,6` / `jl` @ `0x458d8a`: no clear; PlaceTile `5`.
    let mut city = City::new_blank("/tmp/sc2000-gift-tree-lt6.sc2", "GiftT6").expect("blank");
    city.set_building(14, 14, 3);
    city.set_zone(14, 14, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut crt = Win95CrtRand::new(1);
    let seed_before = crt.seed();
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 14, 14);
    assert_eq!(city.building_at(14, 14), 5);
    // Zone corners survive the DestroyBuilding no-op (PlaceTile `5` does not clear them).
    assert_eq!(
        city.zone_at(14, 14).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0x80
    );
    assert_eq!(crt.seed(), seed_before);
}

#[test]
fn pe_newspaper_gift_zone_paint_size1_436ee7_places_and_marks_corners() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint.sc2", "GiftZP").expect("blank");
    let mut crt = Win95CrtRand::new(1);
    pe_newspaper_gift_zone_paint_size1_436ee7(&mut city, &mut crt, 20, 20, 1);
    let b = city.building_at(20, 20);
    assert!((0x7c..0x7c + 8).contains(&u16::from(b)), "building {b}");
    assert_eq!(city.zone_at(20, 20).expect("z").raw_bits() & 0xf0, 0xf0);
    let bits = city.tile_bits_at(20, 20).expect("bits").raw_bits();
    assert_eq!(bits & XBIT_CHOOSE_AND_PLACE, XBIT_CHOOSE_AND_PLACE);
}

#[test]
fn pe_newspaper_gift_zone_paint_size_gt1_436fb6_stamps_2x2() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint2.sc2", "GiftZP2").expect("blank");
    let mut crt = Win95CrtRand::new(2);
    assert!(pe_newspaper_gift_zone_paint_size_gt1_436fb6(
        &mut city, &mut crt, 20, 20, 2, 0
    ));
    // size=2 → half=1 → (x..x+1, y-1..y); type0 base 0x8c mod 4.
    let b = city.building_at(20, 20);
    assert!((0x8c..0x8c + 4).contains(&u16::from(b)), "building {b}");
    assert_eq!(city.building_at(21, 20), b);
    assert_eq!(city.building_at(20, 19), b);
    assert_eq!(city.building_at(21, 19), b);
    let bits = city.tile_bits_at(20, 20).expect("bits").raw_bits();
    assert_eq!(bits & XBIT_CHOOSE_AND_PLACE, XBIT_CHOOSE_AND_PLACE);
    let corners = pe_choose_and_place_corner_bits(city.rotation());
    assert_eq!(
        city.zone_at(20, 19).expect("nw").raw_bits() & 0xf0,
        corners[0]
    );
    assert_eq!(
        city.zone_at(21, 19).expect("ne").raw_bits() & 0xf0,
        corners[1]
    );
    assert_eq!(
        city.zone_at(20, 20).expect("sw").raw_bits() & 0xf0,
        corners[2]
    );
    assert_eq!(
        city.zone_at(21, 20).expect("se").raw_bits() & 0xf0,
        corners[3]
    );
}

#[test]
fn pe_newspaper_gift_zone_paint_size2_483d08_empty_falls_to_size1() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint-s2e.sc2", "GiftS2e").expect("blank");
    let mut crt = Win95CrtRand::new(9);
    // type 1 (not 4) + empty XBLD → size-1 paint from table index 5 (base 0x7c).
    pe_newspaper_gift_zone_paint_size2_483d08(&mut city, &mut crt, 22, 22, 1);
    let b = u16::from(city.building_at(22, 22));
    assert!((0x7c..0x7c + 8).contains(&b), "building {b}");
    assert_eq!(city.zone_at(22, 22).expect("z").raw_bits() & 0xf0, 0xf0);
}

#[test]
fn pe_newspaper_gift_zone_paint_size2_483d08_type4_paints_size2() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint-s2.sc2", "GiftS2").expect("blank");
    let mut crt = Win95CrtRand::new(3);
    pe_newspaper_gift_zone_paint_size2_483d08(&mut city, &mut crt, 30, 30, 4);
    // type4 size2 base 0xaa mod 2 — at least one tile in the jittered 2×2 must match.
    let mut found = false;
    for dx in 0..3u16 {
        for dy in 0..3u16 {
            let x = 30usize.wrapping_add(dx as usize).wrapping_sub(1);
            let y = 30usize.wrapping_add(dy as usize).wrapping_sub(1);
            if x >= CITY_WIDTH || y >= CITY_HEIGHT {
                continue;
            }
            let b = u16::from(city.building_at(x, y));
            if (0xaa..0xaa + 2).contains(&b) {
                found = true;
            }
        }
    }
    assert!(found, "expected size2 type4 building near origin");
}

#[test]
fn pe_newspaper_gift_zone_paint_size3_483d6b_type4_paints_half1() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint-s3.sc2", "GiftS3").expect("blank");
    let mut crt = Win95CrtRand::new(4);
    pe_newspaper_gift_zone_paint_size3_483d6b(&mut city, &mut crt, 40, 40, 4);
    // size3 → half=1 → 2×2; type4 base 0xac mod 2.
    let mut found = false;
    for dx in 0..3usize {
        for dy in 0..3usize {
            let x = 40usize.wrapping_add(dx).wrapping_sub(1);
            let y = 40usize.wrapping_add(dy).wrapping_sub(1);
            if x >= CITY_WIDTH || y >= CITY_HEIGHT {
                continue;
            }
            let b = u16::from(city.building_at(x, y));
            if (0xac..0xac + 2).contains(&b) {
                found = true;
            }
        }
    }
    assert!(found, "expected size3 type4 building near origin");
}

#[test]
fn pe_newspaper_gift_zone_paint_size4_483dce_type4_at_offset() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint-s4.sc2", "GiftS4").expect("blank");
    let mut crt = Win95CrtRand::new(5);
    pe_newspaper_gift_zone_paint_size4_483dce(&mut city, &mut crt, 50, 50, 4);
    // origin (49, 51); size4 → half=2 → 3×3; type4 base 0xc4 mod 2.
    let b = u16::from(city.building_at(49, 51));
    assert!((0xc4..0xc4 + 2).contains(&b), "building {b}");
    assert_eq!(u16::from(city.building_at(51, 51)), b);
    assert_eq!(u16::from(city.building_at(49, 49)), b);
    pe_newspaper_gift_zone_paint_size4_483dce(&mut city, &mut crt, 50, 50, 1);
    // type≠4 skips — leave prior stamp.
    assert_eq!(u16::from(city.building_at(49, 51)), b);
}

#[test]
fn highway_xter_0x06_stamps_class2_underlay_not_dialog_0x74() {
    // PE CityTool `0x4622f6` has no AtCoordinates XTER dialog. Category-2
    // `0x06` raises origin altitude, so class 2 underlay rewrites `XTER 0x0d`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-xter6.sc2", "HwyXter6").expect("blank");
    city.set_city_funds(50_000);
    let tiles = highway_segment_tiles((20, 20), NetworkAxis::GridX);
    for &(x, y) in &tiles {
        city.set_terrain(x, y, 0x06);
    }
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    for &(x, y) in &tiles {
        assert_eq!(
            state.city.building_at(x, y),
            0x49,
            "class 2 underlay at ({x},{y})"
        );
        assert_eq!(
            state.city.terrain_at(x, y),
            0x0d,
            "nonzero XTER counts as higher altitude at ({x},{y})"
        );
    }
}

#[test]
fn power_plant_dialog_hit_tests_cancel_info_and_choice_cards() {
    let window_size = (1024, 768);
    let dialog = power_plant_dialog_rect(window_size);
    let cancel = power_plant_cancel_rect(dialog);
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(cancel.x + 4.0, cancel.y + 4.0), window_size),
        Some(PowerPlantDialogAction::Cancel)
    );

    let info = power_plant_info_rect(dialog, 0);
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(info.x + 4.0, info.y + 4.0), window_size),
        Some(PowerPlantDialogAction::Info(EditTool::PowerPlantCoal))
    );

    let hydro = power_plant_card_rect(dialog, 1);
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(hydro.x + 4.0, hydro.y + 4.0), window_size),
        Some(PowerPlantDialogAction::Select(EditTool::PowerPlantHydro))
    );
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(dialog.x - 1.0, dialog.y - 1.0), window_size),
        None
    );
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(dialog.x + 12.0, dialog.y + 52.0), window_size),
        None
    );
    assert!(point_in_power_plant_dialog(
        Vec2::new(dialog.x + 12.0, dialog.y + 52.0),
        window_size
    ));
    assert!(!point_in_power_plant_dialog(
        Vec2::new(dialog.x - 1.0, dialog.y - 1.0),
        window_size
    ));
}

#[test]
fn power_plant_dialog_selects_only_matching_pressed_card() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(4_000);
    let mut state = test_state(city);
    state.open_power_plant_dialog();

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantCoal,
    ));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").pressed_tool,
        Some(EditTool::PowerPlantCoal)
    );
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantHydro
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert_eq!(state.power_plant_dialog.expect("dialog").pressed_tool, None);

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantCoal,
    ));
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantCoal
        ))),
        Some(EditTool::PowerPlantCoal)
    );
    assert_eq!(state.tool, EditTool::PowerPlantCoal);
    assert_eq!(state.power_plant_dialog, None);
}

#[test]
fn power_plant_dialog_ignores_unaffordable_cards_but_keeps_info_enabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(399);
    let mut state = test_state(city);
    state.open_power_plant_dialog();

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Info(
        EditTool::PowerPlantHydro,
    ));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").info_tool,
        Some(EditTool::PowerPlantHydro)
    );

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantHydro,
    ));
    assert_eq!(state.power_plant_dialog.expect("dialog").pressed_tool, None);
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantHydro
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.power_plant_dialog.is_some());

    state.city.set_city_funds(400);
    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantHydro,
    ));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").pressed_tool,
        Some(EditTool::PowerPlantHydro)
    );
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantHydro
        ))),
        Some(EditTool::PowerPlantHydro)
    );
    assert_eq!(state.tool, EditTool::PowerPlantHydro);
}

#[test]
fn power_plant_dialog_ignores_future_locked_cards_but_keeps_info_enabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(50_000);
    city.set_technology_discovery_year(Technology::GasPower, 1950);
    let mut state = test_state(city);
    state.open_power_plant_dialog();

    state
        .apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Info(EditTool::PowerPlantGas));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").info_tool,
        Some(EditTool::PowerPlantGas)
    );

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantGas,
    ));
    assert_eq!(state.power_plant_dialog.expect("dialog").pressed_tool, None);
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantGas
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.power_plant_dialog.is_some());

    state
        .city
        .set_technology_discovery_year(Technology::GasPower, 1900);
    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantGas,
    ));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").pressed_tool,
        Some(EditTool::PowerPlantGas)
    );
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantGas
        ))),
        Some(EditTool::PowerPlantGas)
    );
    assert_eq!(state.tool, EditTool::PowerPlantGas);
}

#[test]
fn power_plant_dialog_selection_refreshes_status_line() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(4_000);
    let mut state = test_state(city);
    state.open_power_plant_dialog();
    state.query_line = "Tile 10,10".to_owned();
    state.query_is_inspect = true;

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantCoal,
    ));
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantCoal
        ))),
        Some(EditTool::PowerPlantCoal)
    );

    assert_eq!(state.tool, EditTool::PowerPlantCoal);
    assert!(!state.query_is_inspect);
    assert!(
        state.query_line.starts_with("Coal Power $4,000  "),
        "status line was {}",
        state.query_line
    );
}

#[test]
fn power_plant_dialog_info_and_blank_release_keep_dialog_open() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(6_600);
    let mut state = test_state(city);
    state.open_power_plant_dialog();

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantOil,
    ));
    assert_eq!(state.finish_power_plant_dialog_mouse_up(None), None);
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.power_plant_dialog.is_some());

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Info(
        EditTool::PowerPlantCoal,
    ));
    assert_eq!(
        state.power_plant_dialog,
        Some(PowerPlantDialogState {
            info_tool: Some(EditTool::PowerPlantCoal),
            pressed_tool: None
        })
    );
}

#[test]
fn power_plant_dialog_uses_original_order_and_labels() {
    assert_eq!(
        POWER_PLANT_CHOICES
            .iter()
            .map(|choice| choice.label)
            .collect::<Vec<_>>(),
        vec![
            "Coal Power",
            "Hydro Electric",
            "Oil Power",
            "Gas Power",
            "Nuclear",
            "Wind Power",
            "Solar Power",
            "Microwave",
            "Fusion"
        ]
    );
    assert_eq!(
        power_plant_choice_for_tool(EditTool::PowerPlantCoal)
            .expect("coal")
            .info,
        "COAL POWER - coal plants generate large amounts of vile black smoke that will cover your buildings in soot. Your citizens may start complaining when they have to repaint their buildings every two years."
    );
}

#[test]
fn arcology_dialog_hit_tests_cancel_info_and_choice_cards() {
    let window_size = (1024, 768);
    let dialog = arcology_dialog_rect(window_size);
    let cancel = arcology_cancel_rect(dialog);
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(cancel.x + 4.0, cancel.y + 4.0), window_size),
        Some(ArcologyDialogAction::Cancel)
    );

    let info = arcology_info_rect(dialog, 0);
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(info.x + 4.0, info.y + 4.0), window_size),
        Some(ArcologyDialogAction::Info(EditTool::ArcologyPlymouth))
    );

    let launch = arcology_card_rect(dialog, 3);
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(launch.x + 4.0, launch.y + 4.0), window_size),
        Some(ArcologyDialogAction::Select(EditTool::ArcologyLaunch))
    );
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(dialog.x - 1.0, dialog.y - 1.0), window_size),
        None
    );
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(dialog.x + 12.0, dialog.y + 52.0), window_size),
        None
    );
    assert!(point_in_arcology_dialog(
        Vec2::new(dialog.x + 12.0, dialog.y + 52.0),
        window_size
    ));
    assert!(!point_in_arcology_dialog(
        Vec2::new(dialog.x - 1.0, dialog.y - 1.0),
        window_size
    ));
}

#[test]
fn arcology_dialog_selects_only_matching_pressed_card() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(200_000);
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);
    state.open_arcology_dialog();

    state
        .apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyPlymouth));
    assert_eq!(
        state.arcology_dialog.expect("dialog").pressed_tool,
        Some(EditTool::ArcologyPlymouth)
    );
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyForest
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert_eq!(state.arcology_dialog.expect("dialog").pressed_tool, None);

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyLaunch));
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyLaunch
        ))),
        Some(EditTool::ArcologyLaunch)
    );
    assert_eq!(state.tool, EditTool::ArcologyLaunch);
    assert_eq!(state.arcology_dialog, None);
}

#[test]
fn arcology_dialog_ignores_unavailable_or_unaffordable_cards_but_keeps_info_enabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(200_000);
    let mut state = test_state(city);
    state.open_arcology_dialog();

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Info(EditTool::ArcologyLaunch));
    assert_eq!(
        state.arcology_dialog.expect("dialog").info_tool,
        Some(EditTool::ArcologyLaunch)
    );

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyLaunch));
    assert_eq!(state.arcology_dialog.expect("dialog").pressed_tool, None);

    state.city.set_rewards_available_mask(1 << 4);
    state.city.set_city_funds(199_999);
    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyLaunch));
    assert_eq!(state.arcology_dialog.expect("dialog").pressed_tool, None);
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyLaunch
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.arcology_dialog.is_some());

    state.city.set_city_funds(200_000);
    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyLaunch));
    assert_eq!(
        state.arcology_dialog.expect("dialog").pressed_tool,
        Some(EditTool::ArcologyLaunch)
    );
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyLaunch
        ))),
        Some(EditTool::ArcologyLaunch)
    );
    assert_eq!(state.tool, EditTool::ArcologyLaunch);
}

#[test]
fn arcology_dialog_selection_refreshes_status_line() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(100_000);
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);
    state.open_arcology_dialog();
    state.query_line = "Tile 10,10".to_owned();
    state.query_is_inspect = true;

    state
        .apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyPlymouth));
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyPlymouth
        ))),
        Some(EditTool::ArcologyPlymouth)
    );

    assert_eq!(state.tool, EditTool::ArcologyPlymouth);
    assert!(!state.query_is_inspect);
    assert!(
        state.query_line.starts_with("Plymouth Arco $100,000  "),
        "status line was {}",
        state.query_line
    );
}

#[test]
fn arcology_dialog_info_and_blank_release_keep_dialog_open() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(150_000);
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);
    state.open_arcology_dialog();

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyDarco));
    assert_eq!(state.finish_arcology_dialog_mouse_up(None), None);
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.arcology_dialog.is_some());

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Info(EditTool::ArcologyPlymouth));
    assert_eq!(
        state.arcology_dialog,
        Some(ArcologyDialogState {
            info_tool: Some(EditTool::ArcologyPlymouth),
            pressed_tool: None
        })
    );
}

#[test]
fn arcology_dialog_uses_original_order_ids_capacities_and_costs() {
    assert_eq!(
        ARCOLOGY_CHOICES
            .iter()
            .map(|choice| (choice.label, choice.building, choice.capacity, choice.cost))
            .collect::<Vec<_>>(),
        vec![
            ("Plymouth Arco", 251, 55_000, 100_000),
            ("Forest Arco", 252, 30_000, 120_000),
            ("Darco", 253, 45_000, 150_000),
            ("Launch Arco", 254, 65_000, 200_000),
        ]
    );
}

#[test]
fn underground_palette_tools_place_xund_tiles() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        1
    );
    // PE PlacePipes reshape mask0→0x0f → XUND 0x1e after post-place refresh.
    assert_eq!(state.city.underground_at(20, 20), 0x1e);
    assert_eq!(state.city.building_at(20, 20), 0);

    assert_eq!(
        state.place_underground_at(20, 21, UndergroundKind::Pipes),
        1
    );
    assert_eq!(state.city.underground_at(20, 20), 0x10);
    assert_eq!(state.city.underground_at(20, 21), 0x10);

    assert_eq!(
        state.place_underground_at(21, 20, UndergroundKind::Subway),
        1
    );
    assert_eq!(state.city.underground_at(21, 20), 1);
}

#[test]
fn eq_le_tables_survive_graph_history_writes_and_reload() {
    let path = std::env::temp_dir().join(format!("sc2000-eq-le-graph-{}.sc2", std::process::id()));
    let mut city = City::new_blank(&path, "eqg").expect("blank");
    city.test_seed_pe_age_cohorts_from_pop(10_000);
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
    let school = city.pe_age_cohort_at(1) + city.pe_age_cohort_at(2);
    let life_avg = city.pe_eq_life_average();
    assert!(school > 0);
    assert!(city.pe_eq_averages_ready());
    let history = (0..GRAPH_HISTORY_VALUES as u32).collect::<Vec<_>>();
    let mut xgrp = vec![0; XGRP_VALUES];
    xgrp[..GRAPH_HISTORY_VALUES].copy_from_slice(&history);
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &xgrp);
    city.save(&path).expect("save");

    let loaded = City::load(&path).expect("reload");
    assert_eq!(
        loaded.pe_age_cohort_at(1) + loaded.pe_age_cohort_at(2),
        school,
        "graph ring must not occupy PE EQ_LE MISC 0x7c"
    );
    assert_eq!(loaded.pe_eq_life_average(), life_avg);
    assert!(loaded.pe_eq_averages_ready());
    assert_eq!(
        loaded.chunk_u32_values(XGRP_CHUNK_NAME, 0, GRAPH_HISTORY_VALUES),
        history
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn january_queues_newspaper_story_when_power_plant_ages_past_48() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let coal = city.provision_microsim_for_building(207).expect("coal");
    let coal_index = usize::from(coal - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(coal_index, 48)); // next January → 49
    city.refresh_structure_microsim_yearly(0, 100, 100, 100, 100, 100);
    assert_eq!(city.microsim_at(coal_index).map(|r| r.stat0), Some(49));
    // PE story 0x24 with priority 100 sorts above default priority-0 fillers.
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(0x24));
    assert_eq!(
        city.pe_newspaper_story_param_at(0),
        Some(6),
        "coal tile 207 → type index 207-201"
    );
    assert_eq!(
        pe_power_plant_aging_headline(NewspaperEdition::Courier, 6),
        "Aging coal power plant"
    );
    assert_eq!(
        pe_power_plant_aging_headline(NewspaperEdition::Extra, 6),
        "Extra: Aging coal plant"
    );
    assert!(
        city.take_pe_newspaper_extra_popup_pending(),
        "PE insert of 0x24 latches Extra auto-open"
    );
    assert!(!city.take_pe_newspaper_extra_popup_pending());
    assert!(crate::city::pe_newspaper_story_triggers_extra_popup(0x24));
    assert!(crate::city::pe_newspaper_story_triggers_extra_popup(3));
    assert!(crate::city::pe_newspaper_story_triggers_extra_popup(5));
    assert!(!crate::city::pe_newspaper_story_triggers_extra_popup(2));
    assert!(!crate::city::pe_newspaper_story_triggers_extra_popup(0x0B));
}

#[test]
fn simulation_day0_january_skips_extra_when_newspaper_extra_is_off() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    assert!(set_city_auto_budget_enabled(&mut city, true));
    // Blank cities default Extra off; keep it off (setter returns false when unchanged).
    let _ = set_city_newspaper_extra_enabled(&mut city, false);
    assert!(!newspaper_extra_enabled_for_city(&city));
    let coal = city.provision_microsim_for_building(207).expect("coal");
    let coal_index = usize::from(coal - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(coal_index, 48));
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Llama);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x24));
    assert_eq!(state.city_info_dialog, None);
    // Insert still latched; app cleared it without opening because Extra was off.
    assert!(!state.city.take_pe_newspaper_extra_popup_pending());
}

#[test]
fn january_opinion_poll_queues_high_approval_newspaper_when_crossing_80() {
    // OpenSC2K MayorApprovalPhase: previous < 80 and new > 79 → story 0x201.
    let mut city = City::new_blank("/tmp/sc2000-hi-appr.sc2", "HiAppr").expect("blank");
    city.set_pe_mayor_approval(85);
    crate::city::pe_queue_high_mayor_approval_story_if_crossed(&mut city, 50);
    assert_eq!(
        city.pe_newspaper_story_id_at(0),
        Some(0x201),
        "crossing into ≥80 should queue high-approval newspaper story"
    );
    city.set_misc_u32_at(0x0EC8, 0);
    crate::city::pe_queue_high_mayor_approval_story_if_crossed(&mut city, 85);
    assert_eq!(
        city.pe_newspaper_story_id_at(0).unwrap_or(0),
        0,
        "already-high approval must not re-queue the story"
    );
}

#[test]
fn query_shows_museum_attendance_and_library_books() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 212) > 0);
    assert!(state.place_building_at(30, 30, 245) > 0);
    state
        .city
        .refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let museum = query_tile_status_line(&state.city, 20, 20, false);
    let library = query_tile_status_line(&state.city, 30, 30, false);
    assert!(
        museum.contains("Attendance 350") && museum.contains("Employees 90"),
        "{museum}"
    );
    assert!(
        library.contains("Books 200")
            && library.contains("Visitors 388")
            && library.contains("Grade A"),
        "{library}"
    );
}

#[test]
fn query_shows_park_acres_and_marina_boats() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 20, 20, 213, 3);
    stamp_complete_building_for_test(&mut city, 30, 30, 248, 3);
    let park_xtxt = city.provision_microsim_for_building(213).expect("park");
    let marina_xtxt = city.provision_microsim_for_building(248).expect("marina");
    city.set_text_overlay(20, 20, park_xtxt);
    city.set_text_overlay(30, 30, marina_xtxt);
    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let park = query_tile_status_line(&city, 20, 20, false);
    let marina = query_tile_status_line(&city, 30, 30, false);
    assert!(
        park.contains("Attendance 1,166") && park.contains("Acres 9"),
        "{park}"
    );
    assert!(marina.contains("Boats 46"), "{marina}");
}

#[test]
fn simulation_month_advance_preserves_inspect_query_text() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.query_is_inspect = true;
    state.query_line = "Tile 10,10".to_owned();
    state.city.set_zone(10, 10, 1);

    state.handle_simulation_month_advanced();

    assert_eq!(state.query_line, "Tile 10,10");
    assert!(state.query_is_inspect);
}

#[test]
fn desalinization_palette_tool_places_through_apply_path() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_terrain(9, 10, 0x30);
    city.set_tile_bits(9, 10, crate::city::TileBits::new(0b0000_0101));
    let mut state = test_state(city);
    state.tool = EditTool::Desalinization;

    let placement = state
        .tool_placement_at(10, 10)
        .expect("desalinization placement");
    assert_eq!(placement.preview, ToolPreview::Building(250));
    assert_eq!(placement.valid_tiles.len(), 9);
    assert!(placement.invalid_tiles.is_empty());
    assert!(state.apply_tool_at_tile(10, 10));
    assert_eq!(state.city.city_funds(), 19_000);

    for (x, y) in building_footprint_from_origin(10, 10, 3).expect("building footprint") {
        assert_eq!(state.city.building_at(x, y), 250);
        assert_eq!(
            state.city.underground_at(x, y),
            UNDER_TILE_BUILDING_PIPE_TEE
        );
        assert!(
            state
                .city
                .tile_bits_at(x, y)
                .is_some_and(|bits| bits.piped()),
            "desalinization footprint tile ({x}, {y}) should have building plumbing"
        );
    }
}

#[test]
fn title_new_city_selections_set_difficulty_funds_year_and_national_population() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    apply_new_city_title_selections(&mut city, Some(10_000), Some(2050), Some(3));
    ensure_default_city_options_with_crt(&mut city, None);
    initialize_original_new_city_national_misc(&mut city);

    assert_eq!(city.city_funds(), 10_000);
    assert_eq!(city.pe_w_city_difficulty, 3);
    assert_eq!(city.saved_date().map(|date| date.year), Some(2050));
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(150_000)
    );
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET), Some(45_000));
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TREND_OFFSET), Some(2));
}

#[test]
fn title_new_city_difficulty_round_trips_in_both_city_layouts() {
    for (kind, path) in [
        (CityFileKind::WinSc2, "/tmp/title-difficulty-win.sc2"),
        (CityFileKind::MacCity, "/tmp/title-difficulty-mac.city"),
    ] {
        let mut city = City::new_blank(path, "Title Difficulty").expect("blank city");
        if kind == CityFileKind::MacCity {
            city.kind = kind;
            city.prepare_mac_fidelity_from_saved_city();
        }

        apply_new_city_title_selections(&mut city, Some(10_000), Some(2050), Some(3));
        assert_eq!(city.city_funds(), 10_000);
        assert_eq!(city.pe_w_city_difficulty, 3);
        assert_eq!(city.misc_u32_at(MISC_DIFFICULTY_OFFSET), Some(3));
        city.save(path).expect("save title city");

        let loaded = City::load(path).expect("reload title city");
        assert_eq!(loaded.kind, kind);
        assert_eq!(loaded.pe_w_city_difficulty, 3);
        assert_eq!(loaded.misc_u32_at(MISC_DIFFICULTY_OFFSET), Some(3));
    }
}

#[test]
fn title_new_city_start_year_filters_inventions_before_founding_year() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    apply_new_city_title_selections(&mut city, Some(10_000), Some(2000), Some(2));
    city.apply_default_new_city_technology_schedule();
    ensure_default_city_options_with_crt(&mut city, None);
    initialize_original_new_city_national_misc(&mut city);

    let mut crt = Win95CrtRand::new(1);
    city.pe_reseed_neighbor_fame_ranks_42e585(&mut crt);

    assert_eq!(city.pe_w_year_founded, 2000);
    assert_eq!(city.misc_u32_at(MISC_YEAR_FOUNDED_OFFSET), Some(2000));
    assert_eq!(city.city_age_days(), Some(0));
    for technology in [
        Technology::GasPower,
        Technology::Airport,
        Technology::Highways,
        Technology::WaterTreatment,
        Technology::Desalinization,
    ] {
        assert_eq!(city.technology_discovery_year(technology), Some(0));
    }

    for technology in [
        Technology::MicrowavePower,
        Technology::FusionPower,
        Technology::ForestArcology,
        Technology::DarcoArcology,
        Technology::LaunchArcology,
        Technology::Highways2,
    ] {
        assert!(
            city.technology_discovery_year(technology)
                .unwrap_or_default()
                >= 2000,
            "{technology:?} should retain its future randomized invention year"
        );
    }
}

#[test]
fn mac_title_new_city_uses_quickdraw_for_technology_dates() {
    let city = City::new_blank("/tmp/sc2000-mac-title-tech.sc2", "Before").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_process_rng = MacToolboxRng::new(MacToolboxRng::QUICKDRAW_INIT_GRAF_SEED);
    state.mac_game_rng = crate::mac_random::MacGameLcgRng::after_startup_playlist_choice();
    let mut expected_game_rng = state.mac_game_rng;
    let mut expected_rng = state.mac_process_rng;
    let mut expected = [0u16; crate::city::TECHNOLOGY_BASE_DISCOVERY_YEARS.len()];
    for (index, base_year) in crate::city::TECHNOLOGY_BASE_DISCOVERY_YEARS
        .iter()
        .copied()
        .enumerate()
    {
        let year = base_year + expected_rng.random_u16() % 20;
        expected[index] = if year < 1950 { 0 } else { year };
    }
    // New City hands the same process QuickDraw stream into Make Map after
    // the 17 invention-date draws. Reproduce that consumption independently.
    let _ = terrain_generation::generate_with_rng(
        &mut expected_rng,
        true,
        true,
        ui::EditMapSliderValues::default(),
        &mut expected_game_rng,
    );

    state
        .new_city_from_title(
            "Mac Engine City".into(),
            20_000,
            1950,
            1,
            12345,
            title_screen::NewCityStartFlow::GenerateAndEdit,
        )
        .unwrap();

    // Title-created cities currently use a Win-layout `.SC2` container even
    // when the selected simulation engine is Mac. The shared low-WORD
    // difficulty role must survive that cross-open pairing and persistence.
    assert_eq!(state.engine_fidelity, EngineFidelity::Mac68k);
    assert_eq!(state.city.kind, CityFileKind::WinSc2);
    assert_eq!(
        state.city.mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET),
        Some(1)
    );
    let cross_open_path = "/tmp/mac-engine-title-tech-cross-open.sc2";
    state.city.save(cross_open_path).unwrap();
    let mut cross_open = City::load(cross_open_path).unwrap();
    cross_open.prepare_mac_fidelity_from_saved_city();
    assert_eq!(cross_open.kind, CityFileKind::WinSc2);
    assert_eq!(
        cross_open.mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET),
        Some(1)
    );

    assert_eq!(state.mac_process_rng.seed(), expected_rng.seed());
    assert_eq!(state.mac_game_rng.state(), expected_game_rng.state());
    for index in 0..expected.len() {
        assert_eq!(
            state.city.mac_state_misc_u32_at(0x0738 + index * 4),
            Some(u32::from(expected[index])),
            "Mac engine start-year technology date {index}"
        );
    }
}

#[test]
fn query_tile_power_status_uses_process_crt_seed_not_city_name() {
    // Live Query/inspect Power/Water yes-no shares FUN_004679B0 process CRT.
    let mut city = City::new_blank("/tmp/sc2000-query-crt.sc2", "QueryCrt").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_xval_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xa1a1_1a1a);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    state.begin_tile_inspect(10, 10);
    let expected = query_tile_status_line_with_passengers(
        &state.city,
        10,
        10,
        false,
        TransitPassengerStats::default(),
        process,
    );
    assert_eq!(state.query_line, expected);
    let city_name_line = query_tile_status_line_with_passengers(
        &state.city,
        10,
        10,
        false,
        TransitPassengerStats::default(),
        pe_power_recalc_seed(&state.city),
    );
    assert_eq!(
        query_tile_status_line(&state.city, 10, 10, false),
        city_name_line,
        "offline query helper keeps city-name seed"
    );
    let (general, _) = query_dialog_row_groups(
        &state.city,
        10,
        10,
        false,
        TransitPassengerStats::default(),
        process,
    );
    let power = general
        .iter()
        .find(|(k, _)| k == "Power")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        power,
        Some(query_service_yes_no(
            tile_power_status_text_with_entry_seed(&state.city, 10, 10, process,)
        )),
    );
}

#[test]
fn palette_buildings_stamp_recovered_underground_side_effects() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(30, 30, 220), 1);
    assert_eq!(state.city.building_at(30, 30), 220);
    assert_eq!(state.city.underground_at(30, 30), UNDER_TILE_PIPES_LR);

    assert_eq!(state.place_building_at(40, 40, 233), 1);
    assert_eq!(state.city.building_at(40, 40), 233);
    assert_eq!(state.city.underground_at(40, 40), UNDER_TILE_GREY_BOX_2);

    assert_eq!(state.place_building_at(50, 50, 108), 1);
    assert_eq!(state.city.building_at(50, 50), 108);
    assert_eq!(state.city.underground_at(50, 50), UNDER_TILE_GREY_BOX_2);

    assert_eq!(state.place_building_at(60, 60, 249), 9);
    for y in 60..=62 {
        for x in 60..=62 {
            assert_eq!(state.city.underground_at(x, y), UNDER_TILE_GREY_BOX_1);
        }
    }

    state.city.set_terrain(69, 70, 0x30);
    state
        .city
        .set_tile_bits(69, 70, crate::city::TileBits::new(0b0000_0001));
    assert_eq!(state.place_building_at(70, 70, 250), 9);
    for y in 70..=72 {
        for x in 70..=72 {
            assert_eq!(
                state.city.underground_at(x, y),
                UNDER_TILE_BUILDING_PIPE_TEE
            );
        }
    }
}

#[test]
fn palette_building_placement_stamps_footprint_and_corner_bits() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 7);
    city.set_zone(10, 10, 1);
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(10, 10, 210), 9);

    for y in 10..=12 {
        for x in 10..=12 {
            assert_eq!(state.city.building_at(x, y), 210, "building at ({x},{y})");
        }
    }
    let [top_left, top_right, bottom_left, bottom_right] = state.city.footprint_corner_bits();
    assert_eq!(state.city.zone_at(10, 10).unwrap().raw_bits(), top_left);
    assert_eq!(state.city.zone_at(12, 10).unwrap().raw_bits(), top_right);
    assert_eq!(state.city.zone_at(10, 12).unwrap().raw_bits(), bottom_left);
    assert_eq!(state.city.zone_at(12, 12).unwrap().raw_bits(), bottom_right);
    assert_eq!(state.city.zone_at(11, 11).unwrap().raw_bits(), 0);

    let drawn_tiles = (10..=12)
        .flat_map(|x| (10..=12).map(move |y| (x, y)))
        .filter(|&(x, y)| state.city.building_should_draw_at(x, y))
        .count();
    assert_eq!(drawn_tiles, 1);
}

#[test]
fn palette_building_placement_rejects_blocked_or_water_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(11, 11, 112);
    city.set_terrain(20, 20, 0x30);
    let mut state = test_state(city);

    let blocked = state.building_placement_at(10, 10, 210);
    assert_eq!(blocked.preview, ToolPreview::Building(210));
    assert!(blocked.invalid_tiles.contains(&(11, 11)));
    assert_eq!(state.place_building_at(10, 10, 210), 0);
    assert_eq!(state.city.building_at(10, 10), 0);
    assert_eq!(state.city.building_at(11, 11), 112);

    let water = state.building_placement_at(20, 20, 210);
    assert!(water.invalid_tiles.contains(&(20, 20)));
    assert_eq!(state.place_building_at(20, 20, 210), 0);
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn palette_building_placement_rejects_sloped_or_uneven_footprints() {
    let mut slope_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    slope_city.set_terrain(11, 11, 0x01);
    let mut slope_state = test_state(slope_city);

    let sloped = slope_state.building_placement_at(10, 10, 210);
    assert!(sloped.invalid_tiles.contains(&(11, 11)));
    assert_eq!(slope_state.place_building_at(10, 10, 210), 0);
    assert_eq!(slope_state.city.building_at(10, 10), 0);

    let mut uneven_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).expect("building footprint") {
        uneven_city.set_altitude(x, y, 5, 0);
    }
    uneven_city.set_altitude(21, 21, 6, 0);
    let mut uneven_state = test_state(uneven_city);

    let uneven = uneven_state.building_placement_at(20, 20, 210);
    assert!(uneven.invalid_tiles.contains(&(21, 21)));
    assert_eq!(uneven_state.place_building_at(20, 20, 210), 0);
    assert_eq!(uneven_state.city.building_at(20, 20), 0);
}

#[test]
fn palette_building_placement_rebuilds_generated_pipe_service_bits() {
    // PE zone tool does not invent pipe refresh; set service bits explicitly.
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.place_zone_at(11, 10, 1);
    state
        .city
        .set_tile_bits(11, 10, TileBits::new(0).with_piped(true).with_watered(true));
    seed_saved_water_worker_bits_for_test(&mut state.city);
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());

    assert_eq!(state.place_building_at(11, 10, 112), 1);
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());

    assert_eq!(state.place_building_at(12, 10, 6), 1);
    assert!(!state.city.tile_bits_at(12, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(12, 10).unwrap().watered());
}

#[test]
fn city_view_terrain_palette_places_trees_and_water_on_clear_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(30, 30, 112);
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(20, 20, 6), 1);
    assert_eq!(state.city.building_at(20, 20), 6);

    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.terrain_at(20, 20), 0x3d);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 0);

    assert_eq!(state.place_surface_water_at(30, 30), 0);
    assert_eq!(state.city.terrain_at(30, 30), 0);
    assert_eq!(state.city.building_at(30, 30), 112);

    state.tool = EditTool::Water;
    assert!(!state.apply_tool_at_tile(30, 30));
}

#[test]
fn map_tool_place_water_existing_0x30_without_bit_rewrites_via_helper() {
    // PE MapTool accepts bare XTER 0x30; helper mask0 rewrites to 0x3D + PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-water-xter30.sc2", "WaterXter30").expect("blank");
    city.set_city_funds(20_000);
    city.set_terrain(20, 20, 0x30);
    city.set_building(20, 20, 2);
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.terrain_at(20, 20), 0x3d);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_preserves_xtxt() {
    // PE PlaceWater `0x4469ed` → PlaceTile `0x40178f`/`0x441f00` clear XBLD; leave XTXT/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-water-xtxt.sc2", "WaterXtxt").expect("blank");
    city.set_building(20, 20, 5);
    city.set_text_overlay(20, 20, 0x20);
    city.set_traffic_density(20, 20, 0x4a);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.text_overlay_at(20, 20), Some(0x20));
    assert_eq!(city.traffic_density_at(20, 20), Some(0x4a));
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_skips_when_already_water_tile() {
    // PE @ `0x4469df`: after XTER write, water-tile skips PlaceTile / ALTM / recurse.
    let mut city = City::new_blank("/tmp/sc2000-water-skip.sc2", "WaterSkip").expect("blank");
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 0);
    city.set_altitude(20, 20, 7, 0);
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 5, "no PlaceTile clear");
    assert_eq!(city.terrain_at(20, 20), 0x3d, "XTER still rewritten");
    assert_eq!(city.altitude_at(20, 20), 7);
    assert_eq!(city.water_height_at(20, 20), 0);
}

#[test]
fn pe_place_water_helper_reslopes_west_neighbor_xter() {
    // PE @ `0x446a66`: after place, recurse W neighbor with water-tile → E-mask 0x42.
    let mut city = City::new_blank("/tmp/sc2000-water-reslope.sc2", "WaterReslope").expect("blank");
    city.set_terrain(19, 20, 0x3d);
    city.set_tile_bits(19, 20, TileBits::new(0).with_water_tile(true));
    city.set_building(19, 20, 3);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x44, "new tile W-mask");
    assert_eq!(city.terrain_at(19, 20), 0x42, "neighbor re-slope E-mask");
    assert_eq!(city.building_at(19, 20), 3, "neighbor skip PlaceTile");
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_skips_waterfall_xter_2e_3e() {
    // PE @ `0x44674b`/`0x446755`: XTER 0x2E/0x3E → exit (no PlaceTile).
    for xter in [0x2e_u8, 0x3e] {
        let mut city = City::new_blank("/tmp/sc2000-water-fall.sc2", "WaterFall").expect("blank");
        city.set_building(20, 20, 3);
        city.set_terrain(20, 20, xter);
        pe_place_water_helper_446650(&mut city, 20, 20);
        assert_eq!(city.building_at(20, 20), 3, "xter {xter:#x}");
        assert_eq!(city.terrain_at(20, 20), xter);
        assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
    }
}

#[test]
fn pe_place_water_helper_slope_land_writes_waterfall_xter_3e() {
    // PE @ `0x44675f`/`0x44676a`: existing XTER `1..=0x0F` → PlaceWater XTER `0x3E`.
    let mut city = City::new_blank("/tmp/sc2000-water-slope.sc2", "WaterSlope").expect("blank");
    city.set_terrain(20, 20, 0x05);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x3e);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_keeps_shallow_water_xter_10_1f() {
    // PE @ `0x446970`/`0x4469a3`: existing `0x10..=0x1F` skips XTER store; still PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-water-shallow.sc2", "WaterShallow").expect("blank");
    city.set_terrain(20, 20, 0x14);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x14, "keep shallow-water XTER");
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mid_band_0x20_writes_xter_minus_10() {
    // PE @ `0x4469a9`: XTER 0x20 ^ nibble0x0d → low nibble ≠0 → write `0x10`.
    let mut city = City::new_blank("/tmp/sc2000-water-20.sc2", "Water20").expect("blank");
    city.set_terrain(20, 20, 0x20);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x10);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mid_band_0x2d_exits_with_mask0() {
    // PE @ `0x4469a9`: XTER 0x2D ^ nibble0x0d → low nibble 0 → full exit.
    let mut city = City::new_blank("/tmp/sc2000-water-2d.sc2", "Water2d").expect("blank");
    city.set_terrain(20, 20, 0x2d);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 2);
    assert_eq!(city.terrain_at(20, 20), 0x2d);
    assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mid_band_0x21_writes_xter_minus_10() {
    // PE @ `0x4469b5`: XTER 0x21 + mask0 → write `0x11`, then PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-water-21.sc2", "Water21").expect("blank");
    city.set_terrain(20, 20, 0x21);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x11);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_existing_0x30_rewrites_to_0x3d() {
    // PE @ `0x446981`: XTER 0x30 + nibble0x0d → not -0x30 → write 0x3D.
    let mut city = City::new_blank("/tmp/sc2000-water-30.sc2", "Water30").expect("blank");
    city.set_terrain(20, 20, 0x30);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x3d);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_existing_0x3d_exits_with_mask0() {
    // PE @ `0x44698f`: XTER 0x3D + nibble0x0d → cx-ax == -0x30 → full exit.
    let mut city = City::new_blank("/tmp/sc2000-water-3d.sc2", "Water3d").expect("blank");
    city.set_terrain(20, 20, 0x3d);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 2);
    assert_eq!(city.terrain_at(20, 20), 0x3d);
    assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_existing_0x31_rewrites_to_0x3d() {
    // PE @ `0x446998`: XTER >=0x30, mask0 nibble → write 0x3D + PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-water-31.sc2", "Water31").expect("blank");
    city.set_terrain(20, 20, 0x31);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x3d);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_flat_mask0_writes_xter_3d() {
    // PE table[0]=0x0D + 0x30 → isolated flat PlaceWater XTER 0x3D.
    let mut city = City::new_blank("/tmp/sc2000-water-flat.sc2", "WaterFlat").expect("blank");
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x3d);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_neighbor_mask_west_writes_xter_44() {
    // PE @ `0x446773`/`0x446858`: W water-tile → mask 0x08 → nibble 0x14 → XTER 0x44.
    let mut city = City::new_blank("/tmp/sc2000-water-mask-w.sc2", "WaterMaskW").expect("blank");
    city.set_tile_bits(19, 20, TileBits::new(0).with_water_tile(true));
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x44);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_neighbor_mask_east_writes_xter_42() {
    // PE mask E=0x02 → table nibble 0x12 → XTER 0x42.
    let mut city = City::new_blank("/tmp/sc2000-water-mask-e.sc2", "WaterMaskE").expect("blank");
    city.set_tile_bits(21, 20, TileBits::new(0).with_water_tile(true));
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x42);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mask_f_missing_nw_writes_xter_39() {
    // PE @ `0x446862`: NESW all water, NW diag missing → secondary 1 → nibble 9 → 0x39.
    let mut city = City::new_blank("/tmp/sc2000-water-maskf.sc2", "WaterMaskF").expect("blank");
    for (x, y) in [
        (20, 19),
        (21, 20),
        (20, 21),
        (19, 20),
        (21, 19),
        (21, 21),
        (19, 21),
    ] {
        city.set_tile_bits(x, y, TileBits::new(0).with_water_tile(true));
    }
    // NW (19,19) left dry.
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x39);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mask_f_all_diags_writes_xter_30() {
    // PE secondary 0 (all diagonals water) → nibble 0 → XTER 0x30.
    let mut city = City::new_blank("/tmp/sc2000-water-maskf0.sc2", "WaterMaskF0").expect("blank");
    for (x, y) in [
        (20, 19),
        (21, 20),
        (20, 21),
        (19, 20),
        (19, 19),
        (21, 19),
        (21, 21),
        (19, 21),
    ] {
        city.set_tile_bits(x, y, TileBits::new(0).with_water_tile(true));
    }
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x30);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn edit_map_palette_tools_apply_through_selected_tool_path() {
    for tool in [
        EditTool::RaiseTerrain,
        EditTool::LowerTerrain,
        EditTool::StretchTerrain,
        EditTool::LevelTerrain,
        EditTool::Water,
        EditTool::Stream,
        EditTool::Trees,
        EditTool::Forest,
    ] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_mac_selected_water_height(8);
        // PE EditNewMap Raise uses the same billed submode-0 worker; the blank
        // fixture otherwise starts with zero funds and correctly refuses land++.
        city.set_city_funds(20_000);
        city.set_altitude(20, 20, 5, 0);
        city.set_altitude(19, 20, 7, 0);
        city.set_altitude(21, 20, 7, 0);
        city.set_altitude(20, 19, 7, 0);
        city.set_altitude(20, 21, 7, 0);
        if tool == EditTool::RaiseTerrain {
            // PE Raise submode-0 refuses vacant dry; stamp XBLD so land++ runs.
            city.set_building(20, 20, 4);
        }
        let mut state = test_state(city);
        state.mode = AppMode::EditNewMap;
        state.tool = tool;
        if tool == EditTool::StretchTerrain || tool == EditTool::LowerTerrain {
            // PE Stretch/Lower CRT jitter: seed 17 → dx=0,dy=0 so click tile is edited.
            state.pe_process_crt = Win95CrtRand::new(17);
        }

        let placement = state.tool_placement_at(20, 20).expect("edit placement");
        assert_eq!(placement.preview, ToolPreview::Terrain, "{tool:?}");
        assert_eq!(placement.valid_tiles, vec![(20, 20)], "{tool:?}");
        assert!(placement.invalid_tiles.is_empty(), "{tool:?}");
        assert!(state.apply_tool_at_tile(20, 20), "{tool:?}");

        match tool {
            EditTool::RaiseTerrain => {
                // The successful billed land++ is followed by NESW smoothing
                // and the 3x3 slope reclassifier; this 5-vs-7 fixture ends at 7.
                assert_eq!(state.city.altitude_at(20, 20), 7)
            }
            EditTool::LowerTerrain => {
                // The full queued relaxation and expanded UnionRect refresh also lower
                // this clicked cell's 7-level cardinal neighbors; it settles at 6.
                assert_eq!(state.city.altitude_at(20, 20), 6)
            }
            EditTool::StretchTerrain => assert_eq!(state.city.altitude_at(20, 20), 7),
            EditTool::LevelTerrain => {
                // PE Level: `0x445b50` pulls NESW 7→6; `0x447490` slope 0x32 may land++ further.
                assert!(state.city.altitude_at(20, 20) >= 6);
                assert!(state.city.altitude_at(19, 20) >= 6);
                assert!(state.city.altitude_at(21, 20) >= 6);
                assert!(state.city.altitude_at(20, 19) >= 6);
                assert!(state.city.altitude_at(20, 21) >= 6);
                assert_ne!(state.city.terrain_at(21, 20), 0, "447490 remapped neighbor");
            }
            EditTool::Water | EditTool::Stream => {
                assert_eq!(state.city.terrain_at(20, 20), 0x30);
                assert!(state.city.altm_at(20, 20).water > state.city.altitude_at(20, 20));
            }
            EditTool::Trees => assert_eq!(state.city.terrain_at(20, 20), 0x0b),
            EditTool::Forest => {
                let mut tree_tiles = 0;
                for y in 17..=23 {
                    for x in 17..=23 {
                        tree_tiles +=
                            usize::from((0x06..=0x0c).contains(&state.city.terrain_at(x, y)));
                    }
                }
                assert!(tree_tiles > 20, "expected forest brush to place a patch");
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn edit_map_sea_level_buttons_raise_and_lower_global_waterline() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_altitude(10, 10, BLANK_MAP_BASE_HEIGHT.saturating_sub(1), 0);
    city.set_altitude(11, 10, BLANK_MAP_BASE_HEIGHT, 0);
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;

    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert_eq!(
        state.query_line,
        format!("Sea Level {}", BLANK_MAP_BASE_HEIGHT + 1)
    );
    assert!(is_water_terrain(state.city.terrain_at(10, 10)));
    assert!(is_water_terrain(state.city.terrain_at(11, 10)));

    assert!(state.adjust_edit_map_sea_level(-1) > 0);
    assert_eq!(
        state.query_line,
        format!("Sea Level {BLANK_MAP_BASE_HEIGHT}")
    );
    // Native lowering leaves this one-level shoreline as XTER 0x20;
    // the coarse is_water_terrain helper does not include that class.
    assert_eq!(state.city.terrain_at(10, 10), 0x20);
    assert!(state.city.tile_bits_at(10, 10).unwrap().water_tile());
    assert_eq!(state.city.terrain_at(11, 10), 0);
}

#[test]
fn title_generate_and_play_generates_terrain_and_enters_city_view() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.show_grid = true;

    state.apply_new_city_start_flow(
        title_screen::NewCityStartFlow::GenerateAndPlay,
        MacToolboxRng::DEFAULT_SEED,
    );

    assert_eq!(state.mode, AppMode::CityView);
    assert_eq!(state.tool, EditTool::Query);
    assert!(!state.show_grid);
    assert!(state.show_traffic);
    assert_eq!(state.edit_map_generation_seed, MacToolboxRng::DEFAULT_SEED);
    assert!(
        (0..CITY_HEIGHT).any(|y| {
            (0..CITY_WIDTH).any(|x| state.city.altitude_at(x, y) != BLANK_MAP_BASE_HEIGHT)
        }),
        "generated terrain should not remain flat"
    );
    assert_eq!(
        state.city.rate_of_growth_at(0, 0),
        Some(RATE_OF_GROWTH_NEUTRAL),
        "terrain replacement must preserve the PE fresh-city XROG seed"
    );
    assert_eq!(state.city.population_density_at(0, 0), Some(0));
    assert_eq!(state.city.police_coverage_at(0, 0), Some(0));
    assert_eq!(state.city.fire_coverage_at(0, 0), Some(0));
}

#[test]
fn mac_title_generated_map_preserves_neutral_xrog_seed() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-new-city-xrog.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    state.apply_new_city_start_flow(
        title_screen::NewCityStartFlow::GenerateAndPlay,
        MacToolboxRng::DEFAULT_SEED,
    );

    assert_eq!(state.city.kind, CityFileKind::MacCity);
    assert_eq!(
        state.city.rate_of_growth_at(0, 0),
        Some(RATE_OF_GROWTH_NEUTRAL)
    );
    assert_eq!(state.city.population_density_at(0, 0), Some(0));
    assert_eq!(state.city.police_coverage_at(0, 0), Some(0));
    assert_eq!(state.city.fire_coverage_at(0, 0), Some(0));
    assert_eq!(
        state
            .city
            .rate_of_growth_at(CITY_WIDTH - 1, CITY_HEIGHT - 1),
        Some(RATE_OF_GROWTH_NEUTRAL)
    );

    let mut first_population = vec![0; CITY_TILES];
    first_population[density_map_index(0, 0)] = 4;
    let first_growth = live_rate_of_growth_map_for_city(&state.city, &first_population);
    assert_eq!(
        first_growth[density_map_index(0, 0)],
        pe_rate_of_growth_step(RATE_OF_GROWTH_NEUTRAL, 0, 4),
        "the first growth update must use the native zero-XPOP prior"
    );

    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-new-city-microsims-{}.city",
        std::process::id()
    ));
    state.city.save(&path).expect("save fresh Mac city");
    let loaded = City::load(&path).expect("reload fresh Mac city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.population_density_at(0, 0), Some(0));
    assert_eq!(loaded.police_coverage_at(0, 0), Some(0));
    assert_eq!(loaded.fire_coverage_at(0, 0), Some(0));
    assert_eq!(loaded.rate_of_growth_at(0, 0), Some(RATE_OF_GROWTH_NEUTRAL));
    let _ = std::fs::remove_file(path);
}

#[test]
fn budget_service_counts_live_palette_facilities_once() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    stamp_complete_building_for_test(&mut city, 20, 10, 211, 3);
    stamp_complete_building_for_test(&mut city, 30, 10, 209, 3);
    stamp_complete_building_for_test(&mut city, 40, 10, 214, 3);
    stamp_complete_building_for_test(&mut city, 50, 10, 217, 4);
    set_building_and_pe_count_for_test(&mut city, 70, 10, 210);

    for (x, y) in highway_segment_tiles((80, 80), NetworkAxis::GridX) {
        set_building_and_pe_count_for_test(&mut city, x, y, 74);
    }
    set_building_and_pe_count_for_test(&mut city, 5, 5, 29);
    set_building_and_pe_count_for_test(&mut city, 6, 5, 63);
    set_building_and_pe_count_for_test(&mut city, 7, 5, 44);
    set_building_and_pe_count_for_test(&mut city, 8, 5, 81);
    set_building_and_pe_count_for_test(&mut city, 9, 5, 90);
    set_building_and_pe_count_for_test(&mut city, 10, 5, 106);
    // PE subway built @ `0x473807`: `0x4ca41c` + `dwTileCount[0xe9]` (not a XUND rescan).
    pe_place_xund_442370(
        &mut city,
        11,
        5,
        underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT),
    );
    pe_place_xund_442370(&mut city, 12, 5, UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR);
    pe_place_xund_442370(&mut city, 13, 5, UNDER_TILE_GREY_BOX_2);
    pe_place_tile_441f00(&mut city, 14, 5, SUBWAY_STATION_BUILDING_ID);

    let counts = budget_service_built_counts_for_city(&city);

    assert_eq!(counts.police, 1);
    assert_eq!(counts.fire, 1);
    assert_eq!(counts.health, 1);
    assert_eq!(counts.schools, 1);
    assert_eq!(counts.colleges, 1);
    assert_eq!(counts.highways, 5);
    assert_eq!(counts.roads, 2);
    assert_eq!(counts.bridges, 3);
    assert_eq!(counts.rail, 1);
    assert_eq!(counts.tunnel, 1);
    assert_eq!(counts.subway, 4); // 3 PlaceXUND occupancy + 1 SubStation
}

#[test]
fn surface_query_distinguishes_tunnel_entrances_from_bridge_tiles() {
    assert_eq!(surface_building_query_label(63), Some("Tunnel"));
    assert_eq!(surface_building_query_label(66), Some("Tunnel"));
    assert_eq!(surface_building_query_label(81), Some("Bridge"));
    assert_eq!(surface_building_query_label(89), Some("Bridge"));
    assert_eq!(surface_building_query_label(90), Some("Bridge"));
    assert_eq!(surface_building_query_label(91), Some("Bridge"));
    assert_eq!(surface_building_query_label(106), Some("Bridge"));
    assert_eq!(surface_building_query_label(107), Some("Bridge"));
}

#[test]
fn construction_refresh_does_not_roll_graph_history() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let history = (0..GRAPH_HISTORY_VALUES as u32).collect::<Vec<_>>();
    city.set_misc_u32_values(GRAPH_HISTORY_MISC_OFFSET, &history);
    let mut state = test_state(city);
    state.tool = EditTool::Road;

    assert_eq!(
        state.place_network_path(&[(10, 10), (11, 10)], NetworkKind::Road),
        2
    );

    assert_eq!(
        state
            .city
            .misc_u32_values(GRAPH_HISTORY_MISC_OFFSET, GRAPH_HISTORY_VALUES),
        history
    );
}

#[test]
fn facility_census_counts_palette_buildings_once_by_category() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    stamp_complete_building_for_test(&mut city, 10, 10, 198, 1);
    stamp_complete_building_for_test(&mut city, 14, 10, 207, 4);
    city.set_building(24, 10, 207);

    stamp_complete_building_for_test(&mut city, 10, 20, 220, 1);
    stamp_complete_building_for_test(&mut city, 14, 20, 235, 2);
    stamp_complete_building_for_test(&mut city, 18, 20, 244, 2);
    stamp_complete_building_for_test(&mut city, 22, 20, 250, 3);

    stamp_complete_building_for_test(&mut city, 10, 30, 210, 3);
    stamp_complete_building_for_test(&mut city, 16, 30, 211, 3);
    stamp_complete_building_for_test(&mut city, 22, 30, 209, 3);
    stamp_complete_building_for_test(&mut city, 28, 30, 216, 4);

    stamp_complete_building_for_test(&mut city, 10, 40, 214, 3);
    stamp_complete_building_for_test(&mut city, 16, 40, 217, 4);
    stamp_complete_building_for_test(&mut city, 24, 40, 245, 2);
    stamp_complete_building_for_test(&mut city, 30, 40, 212, 3);

    stamp_complete_building_for_test(&mut city, 10, 52, 13, 1);
    stamp_complete_building_for_test(&mut city, 14, 52, 213, 3);
    stamp_complete_building_for_test(&mut city, 20, 52, 215, 4);
    stamp_complete_building_for_test(&mut city, 28, 52, 218, 4);
    stamp_complete_building_for_test(&mut city, 36, 52, 248, 3);

    stamp_complete_building_for_test(&mut city, 10, 64, 233, 1);
    stamp_complete_building_for_test(&mut city, 14, 64, 236, 2);
    stamp_complete_building_for_test(&mut city, 18, 64, 237, 2);

    stamp_complete_building_for_test(&mut city, 10, 72, 243, 2);
    stamp_complete_building_for_test(&mut city, 16, 72, 208, 3);
    stamp_complete_building_for_test(&mut city, 22, 72, 219, 1);
    stamp_complete_building_for_test(&mut city, 28, 72, 255, 4);
    stamp_complete_building_for_test(&mut city, 36, 72, 251, 4);
    stamp_complete_building_for_test(&mut city, 44, 72, 254, 4);

    for x in 60..64 {
        city.set_zone(x, 60, ZONE_AIRPORT);
    }
    city.set_building(60, 60, 221);
    for x in 60..63 {
        city.set_zone(x, 62, ZONE_SEAPORT);
    }
    city.set_building(60, 62, 227);

    assert_eq!(
        facility_census_for_city(&city),
        FacilityCensus {
            power_plants: 2,
            water_utilities: 4,
            civic_services: 4,
            education: 4,
            parks: 5,
            transit: 3,
            rewards: 4,
            arcologies: 2,
            airport_tiles: 4,
            seaport_tiles: 3,
            developed_airport_tiles: 1,
            developed_seaport_tiles: 1,
        }
    );
}

#[test]
fn mac_police_coverage_reads_gate_from_misc_cursor_1039() {
    let mut city = City::new_blank("/tmp/sc2000-mac-police-gate.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_building(24, 20, 210);
    city.set_zone(24, 20, 0x81);
    let center = density_map_index(24, 20);

    let baseline =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);

    // Cursor 91 is the first three-lane history row (MISC +0x16c), not the
    // police gate. Changing that unrelated row must not change police strength.
    city.set_mac_state_misc_u32_at(0x16c, 1);
    let wrong_slot =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert_eq!(wrong_slot[center], baseline[center]);

    // CODE 4 serializes fffffe00 at cursor 1039: the signed WORD load reads
    // payload +0x103e and saveback writes the sign-extended dword at +0x103c.
    city.set_mac_state_misc_u32_at(1039 * 4, 1);
    let saved_gate =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert!(saved_gate[center] > baseline[center]);
}

#[test]
fn mac_police_gate_does_not_overwrite_or_read_win_prison_bonus() {
    let mut city = City::new_blank("/tmp/mac-police-win.sc2", "Mixed").expect("blank city");
    city.set_building(24, 20, 210);
    city.set_zone(24, 20, 0x81);
    let before =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    city.set_misc_u32_at(0x103c, 99);
    let after =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert_eq!(before, after, "Win95 prison bonus is not a Mac police gate");
    city.refresh_mac_monthly_random_xmic_rows(
        100,
        100,
        100,
        100,
        100,
        false,
        &mut MacToolboxRng::new(1),
    );
    assert_eq!(city.misc_u32_at(0x103c), Some(99));
    city.set_mac_police_coverage_gate(1);
    let updated =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert!(updated[density_map_index(24, 20)] > before[density_map_index(24, 20)]);
    assert_eq!(city.misc_u32_at(0x103c), Some(99));
}

#[test]
fn simulation_day0_january_skips_budget_window_when_auto_budget_is_on() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    assert!(set_city_auto_budget_enabled(&mut city, true));
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Llama);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.month(), 1);
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.traffic.speed(), GameSpeed::Llama);
}

#[test]
fn mac_native_auto_budget_controls_january_budget_window() {
    for auto_budget in [false, true] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-january-budget.sc2", "MacBudget").expect("blank city");
        city.kind = CityFileKind::MacCity;
        city.set_saved_date(CityDate {
            year: 1900,
            month: 12,
        });
        city.set_mac_state_misc_u32_at(AUTO_BUDGET_MISC_OFFSET, u32::from(auto_budget));
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.traffic.set_speed(GameSpeed::Llama);
        state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);

        assert_eq!(
            state.city_info_dialog,
            (!auto_budget).then_some(CityInfoDialog::Budget)
        );
        assert_eq!(
            state.traffic.speed(),
            if auto_budget {
                GameSpeed::Llama
            } else {
                GameSpeed::Paused
            }
        );
    }
}

#[test]
fn budget_ytd_prorates_original_help_text_september_police_example() {
    let mut values = [0_u32; BUDGET_SERVICE_MISC_STRIDE];
    let police_row = BUDGET_SERVICE_ROW_START;
    for month in 8..12 {
        values[3 + month * 2] = 1;
        values[4 + month * 2] = 100;
    }

    assert_eq!(budget_row_year_to_date(police_row, &values, 12), 400);
}

#[test]
fn monthly_simulation_does_not_roll_misc_graph_history() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let history = (0..GRAPH_HISTORY_VALUES as u32).collect::<Vec<_>>();
    let extra_history = (1_000..1_000 + GRAPH_EXTRA_HISTORY_VALUES as u32).collect::<Vec<_>>();
    city.set_misc_u32_values(GRAPH_HISTORY_MISC_OFFSET, &history);
    city.set_misc_u32_values(GRAPH_EXTRA_HISTORY_MISC_OFFSET, &extra_history);
    city.set_building(10, 10, 0x70);
    city.set_building(11, 10, 0x70);
    city.set_building(12, 10, 0x84);
    let mut state = test_state(city);

    state.refresh_live_misc_stats();
    assert_eq!(
        state
            .city
            .misc_u32_values(GRAPH_HISTORY_MISC_OFFSET, GRAPH_HISTORY_VALUES),
        history,
        "PE graph history is XGRP; leftover MISC rings must stay unread and unrolled"
    );
    assert_eq!(
        state
            .city
            .misc_u32_values(GRAPH_EXTRA_HISTORY_MISC_OFFSET, GRAPH_EXTRA_HISTORY_VALUES),
        extra_history
    );
}

#[test]
fn population_panel_demographics_prefer_bss_fame_national_weather() {
    let mut city = City::new_blank("/tmp/sc2000-pop-bss.sc2", "PopBss").expect("blank");
    city.set_misc_u32_at(MISC_CITY_FAME_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 2);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 3);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 4);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 5);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 6);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 7);
    city.set_misc_u32_at(MISC_HUMID_OFFSET, 8);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 9);
    city.pe_dw_city_fame = 111;
    city.pe_dw_national_population = 222;
    city.pe_dw_national_value = 333;
    city.pe_w_national_tax = 10;
    city.pe_w_national_trend = 11;
    city.pe_b_weather_heat = 12;
    city.pe_b_weather_wind = 13;
    city.pe_b_weather_humid = 14;
    city.pe_b_weather_trend = 2;
    let rows = population_panel_demographic_rows(
        &city,
        PopulationStats::default(),
        CityServiceOutcomes::default(),
    );
    let get = |label: &str| {
        rows.iter()
            .find(|(name, _)| *name == label)
            .map(|(_, value)| *value)
            .expect(label)
    };
    assert_eq!(get("City Fame"), 111);
    assert_eq!(get("National Population"), 222);
    assert_eq!(get("National Value"), 333);
    assert_eq!(get("National Tax"), 10);
    assert_eq!(get("National Trend"), 11);
    assert_eq!(get("Heat"), 12);
    assert_eq!(get("Wind"), 13);
    assert_eq!(get("Humid"), 14);
    assert_eq!(get("Weather Trend"), 2);
}

#[test]
fn monthly_simulation_rolls_original_xgrp_graph_history() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let xgrp_history = (0..XGRP_VALUES as u32).collect::<Vec<_>>();
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &xgrp_history);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 123_000);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 456_000);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 7);
    city.set_building(10, 10, 0x70);
    city.set_building(11, 10, 0x70);
    city.set_building(12, 10, 0x84);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 2);
    state.city.pe_dw_city_age = 25;

    state.refresh_live_misc_stats();

    let updated = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    let population = population_stats_for_city(&state.city);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let expected_latest = xgrp_history_latest_values(&state.city, population, outcomes);
    for (series_index, latest) in expected_latest.into_iter().enumerate() {
        let base = series_index * XGRP_SAMPLES_PER_SERIES;
        assert_eq!(updated[base], latest);
        assert_eq!(
            &updated[base + 1..base + XGRP_MONTH_SAMPLES],
            &xgrp_history[base..base + XGRP_MONTH_SAMPLES - 1]
        );
        assert_eq!(
            &updated[base + XGRP_HALF_YEAR_OFFSET
                ..base + XGRP_HALF_YEAR_OFFSET + XGRP_HALF_YEAR_SAMPLES],
            &xgrp_history[base + XGRP_HALF_YEAR_OFFSET
                ..base + XGRP_HALF_YEAR_OFFSET + XGRP_HALF_YEAR_SAMPLES]
        );
        assert_eq!(
            &updated[base + XGRP_FIVE_YEAR_OFFSET
                ..base + XGRP_FIVE_YEAR_OFFSET + XGRP_FIVE_YEAR_SAMPLES],
            &xgrp_history[base + XGRP_FIVE_YEAR_OFFSET
                ..base + XGRP_FIVE_YEAR_OFFSET + XGRP_FIVE_YEAR_SAMPLES]
        );
    }
}

#[test]
fn xgrp_latest_values_match_pe_update_graph_data() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 456_000);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 123_000);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 7);
    // XGRP reads land/LE/EQ/pollution/crime BSS, not outcomes / density-map fallbacks.
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 100);
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 76);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 90);
    city.pe_dw_city_land_value = 100;
    city.set_pe_water_coverage_percent(0x2a);
    city.set_pe_dw_arcology_population(8);
    city.set_zone(10, 10, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(10, 10, 0x70);
    city.set_building(
        12,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let population = PopulationStats {
        residential_population: 40,
        commercial_population: 20,
        industrial_population: 12,
        arcology_population: 8,
        ..PopulationStats::default()
    };
    let outcomes = CityServiceOutcomes {
        land_value: 100,
        life_expectancy: 76,
        education_quotient: 90,
        ..CityServiceOutcomes::default()
    };

    let traffic = live_traffic_count_for_city(&city, population);
    let developed_divisor = pe_attitude_developed_tile_count(&city) / 4 + 1;
    let counters = live_city_scenario_counters_for_city(&city, population, outcomes);
    // Seed CTP BSS so XGRP matches the live scenario counters PE would mirror.
    city.pe_dw_city_pollution = counters.pollution;
    city.pe_dw_city_crime = counters.crime;
    city.set_pe_traffic_count(traffic);
    city.set_pe_power_used_percent(37);

    let latest = xgrp_history_latest_values(&city, population, outcomes);
    let arco = 8;

    assert_eq!(latest[0], 40 + 20 + 12 + arco);
    assert_eq!(latest[1], 40 + arco / 2);
    assert_eq!(latest[2], 20 + arco / 4);
    assert_eq!(latest[3], 12 + arco / 4);
    assert_eq!(latest[4], traffic / 2, "traffic / (road+hwy+bridge+1)");
    assert_eq!(latest[5], counters.pollution / developed_divisor);
    assert_eq!(latest[6], 100 / developed_divisor);
    assert_eq!(latest[7], counters.crime / developed_divisor);
    assert_eq!(
        latest[8], 63,
        "GRP_POWERPERCENT is 100 - dwPowerUsedPercentage"
    );
    assert_eq!(
        latest[9],
        100 - 0x2a,
        "GRP_WATERPERCENT is 100 - [0x4cc4b8] coverage BSS"
    );
    assert_eq!(latest[10], 76);
    assert_eq!(latest[11], 90);
    assert_eq!(latest[12], unemployment_percent_for_city(&city, population));
    assert_eq!(latest[13], 456_000, "GNP is raw dwNationalValue");
    assert_eq!(
        latest[14], 123_000,
        "national pop is raw dwNationalPopulation"
    );
    assert_eq!(latest[15], 7);
}

#[test]
fn special_zone_development_sprites_do_not_use_palette_animation() {
    for &building in AIRPORT_DEVELOPMENT_BUILDINGS
        .iter()
        .chain(AIRPORT_LARGE_DEVELOPMENT_BUILDINGS.iter())
        .chain(SEAPORT_DEVELOPMENT_BUILDINGS.iter())
        .chain(SEAPORT_LARGE_DEVELOPMENT_BUILDINGS.iter())
    {
        let Some(sprite_id) = building_sprite_id(building) else {
            continue;
        };
        assert!(
            !should_animate_sprite(sprite_id),
            "building {building} sprite {sprite_id} must not be frame-remapped"
        );
    }
}

#[test]
fn rci_development_sprites_do_not_use_palette_animation() {
    for &building in RESIDENTIAL_1X1_BUILDINGS
        .iter()
        .chain(RESIDENTIAL_2X2_BUILDINGS.iter())
        .chain(RESIDENTIAL_3X3_BUILDINGS.iter())
        .chain(COMMERCIAL_1X1_BUILDINGS.iter())
        .chain(COMMERCIAL_2X2_BUILDINGS.iter())
        .chain(COMMERCIAL_3X3_BUILDINGS.iter())
        .chain(INDUSTRIAL_1X1_BUILDINGS.iter())
        .chain(INDUSTRIAL_2X2_BUILDINGS.iter())
        .chain(INDUSTRIAL_3X3_BUILDINGS.iter())
    {
        let Some(sprite_id) = building_sprite_id(building) else {
            continue;
        };
        assert!(
            !should_animate_sprite(sprite_id),
            "RCI building {building} sprite {sprite_id} must not be frame-remapped"
        );
    }
}

#[test]
fn water_radioactive_and_smoke_xbld_sprites_use_palette_animation_frames() {
    for building in 1..=u8::MAX {
        let Some(sprite_id) = building_sprite_id(building) else {
            continue;
        };
        let expected =
            sprite_id == 1005 || is_water_sprite(sprite_id) || is_smoke_sprite(sprite_id);
        assert_eq!(
            should_animate_sprite(sprite_id),
            expected,
            "XBLD building {building} sprite {sprite_id} animation classification mismatch"
        );
    }
}

#[test]
fn non_smoke_placeable_building_sprites_do_not_use_palette_animation() {
    let cases = [
        EditTool::BusDepot,
        EditTool::RailDepot,
        EditTool::SubStation,
        EditTool::SmallPark,
        EditTool::BigPark,
        EditTool::Zoo,
        EditTool::Stadium,
        EditTool::Marina,
        EditTool::PowerPlantCoal,
        EditTool::PowerPlantHydro,
        EditTool::PowerPlantOil,
        EditTool::PowerPlantGas,
        EditTool::PowerPlantNuclear,
        EditTool::PowerPlantWind,
        EditTool::PowerPlantSolar,
        EditTool::PowerPlantMicrowave,
        EditTool::PowerPlantFusion,
        EditTool::Police,
        EditTool::FireStation,
        EditTool::Hospital,
        EditTool::Prison,
        EditTool::School,
        EditTool::College,
        EditTool::Library,
        EditTool::Museum,
        EditTool::WaterPump,
        EditTool::WaterTower,
        EditTool::WaterTreatment,
        EditTool::Desalinization,
        EditTool::MayorHouse,
        EditTool::CityHall,
        EditTool::Statue,
        EditTool::BraunLlamaDome,
        EditTool::ArcologyPlymouth,
        EditTool::ArcologyForest,
        EditTool::ArcologyDarco,
        EditTool::ArcologyLaunch,
    ];
    let animated_buildings = [201, 202, 207, 250];

    for tool in cases {
        let building = placeable_building_for_tool(tool).expect("placeable building");
        let sprite_id = building_sprite_id(building).expect("building sprite");
        let should_animate = animated_buildings.contains(&building);
        assert_eq!(
            should_animate_sprite(sprite_id),
            should_animate,
            "{tool:?} building {building} sprite {sprite_id} animation mismatch"
        );
    }
}

#[test]
fn cursor_resources_follow_extracted_sc2k_tool_art() {
    assert_eq!(GameCursor::for_tool(EditTool::Query).resource_id(), 1023);
    assert_eq!(GameCursor::for_tool(EditTool::Bulldoze).resource_id(), 1002);
    assert_eq!(GameCursor::for_tool(EditTool::Trees).resource_id(), 1003);
    assert_eq!(GameCursor::for_tool(EditTool::Power).resource_id(), 1008);
    assert_eq!(
        GameCursor::for_tool(EditTool::PowerPlantCoal).resource_id(),
        1008
    );
    assert_eq!(GameCursor::for_tool(EditTool::Road).resource_id(), 1017);
    assert_eq!(GameCursor::for_tool(EditTool::Highway).resource_id(), 1021);
    assert_eq!(GameCursor::for_tool(EditTool::Rail).resource_id(), 1022);
    assert_eq!(GameCursor::for_tool(EditTool::Subway).resource_id(), 1022);
    assert_eq!(
        GameCursor::for_tool(EditTool::RaiseTerrain).resource_id(),
        1012
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::LowerTerrain).resource_id(),
        1013
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::LevelTerrain).resource_id(),
        1015
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::StretchTerrain).resource_id(),
        1012
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::SpreadTerrain).resource_id(),
        1015
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneResidential).resource_id(),
        1004
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneCommercial).resource_id(),
        1005
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneCommercialDense).resource_id(),
        1005
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneIndustrial).resource_id(),
        1006
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneIndustrialDense).resource_id(),
        1006
    );
    assert_eq!(GameCursor::for_tool(EditTool::Water).resource_id(), 1016);
    assert_eq!(GameCursor::for_tool(EditTool::Stream).resource_id(), 1016);
    assert_eq!(GameCursor::for_tool(EditTool::Forest).resource_id(), 1003);
    assert_eq!(GameCursor::for_tool(EditTool::Pipes).resource_id(), 1019);
    assert_eq!(
        GameCursor::for_tool(EditTool::WaterPump).resource_id(),
        1019
    );
    assert_eq!(GameCursor::for_tool(EditTool::Police).resource_id(), 1020);
    assert_eq!(GameCursor::for_tool(EditTool::School).resource_id(), 1007);
    assert_eq!(
        GameCursor::for_tool(EditTool::SmallPark).resource_id(),
        1026
    );
    assert_eq!(GameCursor::for_tool(EditTool::Marina).resource_id(), 1026);
    assert_eq!(GameCursor::for_tool(EditTool::Seaport).resource_id(), 1025);
    assert_eq!(GameCursor::for_tool(EditTool::Airport).resource_id(), 1025);
    assert_eq!(
        GameCursor::for_tool(EditTool::MayorHouse).resource_id(),
        1024
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ArcologyPlymouth).resource_id(),
        1024
    );
    assert_eq!(GameCursor::for_tool(EditTool::Sign).resource_id(), 1027);
    assert_eq!(
        GameCursor::for_tool(EditTool::DispatchPolice).resource_id(),
        1028
    );
}

#[test]
fn pe_rci_demand_updates_attitude_newspaper_46881f_queues_low_stories() {
    // PE @ `0x46881f`: rand&0x7f < score → 0x11/0x12/0x10; rand&0xf > score → high ids.
    let mut city = City::new_blank("/tmp/sc2000-attitude-news.sc2", "AttitudeNews").expect("blank");
    city.set_pe_traffic_count(400);
    city.set_pe_attitude_tax_bases([1, 0, 0]);
    city.set_pe_attitude_demand_inputs(1, 200, 0, 200);
    city.recompute_pe_attitude_scores(0, 0, 0);
    assert!(city.pe_attitude_score_at(0) >= 0x80);
    assert!(city.pe_attitude_score_at(1) >= 0x80);
    assert!(city.pe_attitude_score_at(3) >= 0x80);
    let mut crt = Win95CrtRand::new(1);
    pe_rci_demand_updates_attitude_newspaper_46881f(&mut city, &mut crt);
    // High arms need rand&0xf > score (≥128) — impossible; only low stories.
    let mut found = [false; 3];
    for slot in 0..6 {
        match city.pe_newspaper_story_id_at(slot) {
            Some(0x11) => found[0] = true,
            Some(0x12) => found[1] = true,
            Some(0x10) => found[2] = true,
            Some(0x3e) | Some(0x3f) | Some(0x3d) => panic!("unexpected high story"),
            _ => {}
        }
    }
    assert!(found.iter().all(|&f| f), "expected stories 0x11/0x12/0x10");
}

#[test]
fn pe_rci_demand_updates_unemployment_newspaper_4688a8_queues_story_15() {
    // PE @ `0x4688a8`: rand&0x3f < unemp → story 0x15; rand&3 > unemp → 0x42.
    let mut city = City::new_blank("/tmp/sc2000-unemp-news.sc2", "UnempNews").expect("blank");
    city.set_pe_unemployment_percent(0x40); // > 0x3f → always low story
    let mut crt = Win95CrtRand::new(1);
    pe_rci_demand_updates_unemployment_newspaper_4688a8(&mut city, &mut crt);
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(0x15));
    // High arm needs rand&3 > 0x40 — impossible.
    for slot in 0..6 {
        assert_ne!(city.pe_newspaper_story_id_at(slot), Some(0x42));
    }
}

#[test]
fn pe_rci_demand_updates_eq_newspaper_468905_queues_story_13_when_eq_low() {
    // PE @ `0x468905`: EQ < 0x50 and rand%0x50 > EQ → story 0x13.
    let mut city = City::new_blank("/tmp/sc2000-eq-news.sc2", "EqNews").expect("blank");
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 0);
    assert_eq!(city.pe_eq_education_average(), 0);
    let mut queued = false;
    for seed in 1..200 {
        let mut c = City::new_blank("/tmp/sc2000-eq-news2.sc2", "EqNews2").expect("blank");
        c.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 0);
        let mut rng = Win95CrtRand::new(seed);
        pe_rci_demand_updates_eq_newspaper_468905(&mut c, &mut rng);
        if c.pe_newspaper_story_id_at(0) == Some(0x13) {
            queued = true;
            break;
        }
    }
    assert!(queued, "expected story 0x13 for EQ=0");
}

#[test]
fn pe_rci_demand_updates_eq_newspaper_468905_queues_story_40_when_eq_high() {
    // PE high arm: EQ >= 0x50 and rand%0x50 < (EQ-0x50) → story 0x40.
    let mut queued = false;
    for seed in 1..400 {
        let mut city = City::new_blank("/tmp/sc2000-eq-news-hi.sc2", "EqNewsHi").expect("blank");
        city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 0x7f); // threshold 0x2f
        let mut rng = Win95CrtRand::new(seed);
        pe_rci_demand_updates_eq_newspaper_468905(&mut city, &mut rng);
        if city.pe_newspaper_story_id_at(0) == Some(0x40) {
            queued = true;
            break;
        }
    }
    assert!(queued, "expected story 0x40 for high EQ");
}

#[test]
fn pe_rci_demand_updates_le_newspaper_468960_queues_story_14_when_le_low() {
    // PE @ `0x468960`: LE < 0x3c and rand%0x3c > LE → story 0x14.
    let mut queued = false;
    for seed in 1..200 {
        let mut city = City::new_blank("/tmp/sc2000-le-news.sc2", "LeNews").expect("blank");
        city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 0);
        let mut rng = Win95CrtRand::new(seed);
        pe_rci_demand_updates_le_newspaper_468960(&mut city, &mut rng);
        if city.pe_newspaper_story_id_at(0) == Some(0x14) {
            queued = true;
            break;
        }
    }
    assert!(queued, "expected story 0x14 for LE=0");
}

#[test]
fn health_amenities_newspaper_labels_match_opensc2k_status_names() {
    // OpenSC2K CityStatusBar / NEEDS packaging for LE + amenity demand papers.
    assert_eq!(
        pe_health_amenities_newspaper_label(0x11),
        Some("High traffic")
    );
    assert_eq!(pe_health_amenities_newspaper_label(0x3d), Some("Low crime"));
    assert_eq!(
        pe_health_amenities_newspaper_label(0x13),
        Some("Poor education")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x42),
        Some("Good employment")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x14),
        Some("Poor health")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x41),
        Some("Good health")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x33),
        Some("Hospitals Demanded")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x37),
        Some("Residents Demand Zoo")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x3a),
        Some("Residents Demand Park")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x2e),
        Some("Power Plant Needed")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x3c),
        Some("Commerce Needs Connections")
    );
    assert_eq!(
        pe_health_amenities_newspaper_headline(NewspaperEdition::Extra, 0x14),
        "Extra: Poor health"
    );
    assert_eq!(
        pe_health_amenities_newspaper_headline(NewspaperEdition::Courier, 0x33),
        "Hospitals Demanded"
    );
    assert_eq!(
        pe_health_amenities_newspaper_headline(NewspaperEdition::Courier, 0x2f),
        "Citizens Demand Road & Rail"
    );
    assert!(pe_health_amenities_newspaper_story("Testville", 0x41).contains("Testville"));
    assert!(pe_health_amenities_newspaper_label(0x26).is_none());
}

#[test]
fn bridge_collapse_newspaper_story_has_event_specific_copy() {
    assert_eq!(
        pe_bridge_collapse_headline(NewspaperEdition::Courier),
        "Bridge collapse disrupts traffic"
    );
    assert_eq!(
        pe_bridge_collapse_headline(NewspaperEdition::Extra),
        "Extra: Bridge collapse disrupts traffic"
    );
    assert_eq!(
        pe_bridge_collapse_story("Testville"),
        "A weakened bridge in Testville has collapsed, interrupting travel across the route. Crews must clear the damaged structure before normal traffic can return."
    );
}

#[test]
fn pe_rci_demand_updates_neighbor_fame_newspaper_4689c8_queues_story_4() {
    // PE @ `0x4689c8`: rand&7==0 and (fame_base+year) >= rank → story 4, clear slot.
    let mut city = City::new_blank("/tmp/sc2000-neighbor-fame.sc2", "NFame").expect("blank");
    city.pe_w_year_founded = 1900;
    city.set_pe_w_local_fame_base(50);
    city.set_pe_w_neighbor_fame_rank(3, 100); // threshold 1950 >= 100
    let mut seed = 1u32;
    let mut crt = Win95CrtRand::new(seed);
    while crt.random_u15() & 7 != 0 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
    }
    crt = Win95CrtRand::new(seed);
    pe_rci_demand_updates_neighbor_fame_newspaper_4689c8(&mut city, &mut crt);
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(4));
    assert_eq!(city.pe_newspaper_story_param_at(0), Some(3));
    assert_eq!(city.pe_w_neighbor_fame_rank(3), Some(0));
    assert_eq!(
        city.technology_discovery_year(Technology::WindPower),
        Some(0)
    );
    // PE @ `0x468a1a` rebuilds tech-unlock after clear.
    assert_ne!(city.pe_w_tech_unlock_mask(), 0);
}

#[test]
fn pe_rci_demand_updates_neighbor_fame_newspaper_4689c8_queues_story_5_high_index() {
    let mut city = City::new_blank("/tmp/sc2000-neighbor-fame5.sc2", "NFame5").expect("blank");
    city.pe_w_year_founded = 2000;
    city.set_pe_w_local_fame_base(0);
    city.set_pe_w_neighbor_fame_rank(9, 100); // story 5, param 2
    let mut seed = 1u32;
    let mut crt = Win95CrtRand::new(seed);
    while crt.random_u15() & 7 != 0 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
    }
    crt = Win95CrtRand::new(seed);
    pe_rci_demand_updates_neighbor_fame_newspaper_4689c8(&mut city, &mut crt);
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(5));
    assert_eq!(city.pe_newspaper_story_param_at(0), Some(2));
    assert_eq!(city.pe_w_neighbor_fame_rank(9), Some(0));
}

#[test]
fn pe_rci_demand_updates_neighbor_fame_newspaper_4689c8_skips_when_gate_fails() {
    let mut city =
        City::new_blank("/tmp/sc2000-neighbor-fame-skip.sc2", "NFameSkip").expect("blank");
    city.pe_w_year_founded = 1900;
    city.set_pe_w_local_fame_base(50);
    city.set_pe_w_neighbor_fame_rank(0, 1);
    let mut seed = 1u32;
    let mut crt = Win95CrtRand::new(seed);
    while crt.random_u15() & 7 == 0 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
    }
    crt = Win95CrtRand::new(seed);
    pe_rci_demand_updates_neighbor_fame_newspaper_4689c8(&mut city, &mut crt);
    assert_eq!(
        city.pe_newspaper_story_id_at(0),
        Some(0),
        "empty initialized story slots use the zero sentinel"
    );
    assert_eq!(city.pe_w_neighbor_fame_rank(0), Some(1));
}

#[test]
fn sim_calendar_day21_opens_extra_after_rci_newspaper_pending() {
    // PE Extra latch open after FUN_00468150 inserts (`0x47b681`); day-21 calls
    // maybe_open after the RCI newspaper arms.
    let mut city = City::new_blank("/tmp/sc2000-day21-extra.sc2", "Day21Extra").expect("blank");
    assert!(newspaper_extra_enabled_for_city(&city));
    city.queue_pe_newspaper_story(4, 0);
    assert!(city.take_pe_newspaper_extra_popup_pending());
    city.queue_pe_newspaper_story(4, 0); // re-latch for day-21
    let mut state = test_state(city);
    let _ = state.sim_calendar_day21_demand_and_graphs();
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Extra)),
    );
    assert!(!state.city.take_pe_newspaper_extra_popup_pending());
}

#[test]
fn sim_calendar_day21_inserts_baseline_newspaper_story() {
    let city =
        City::new_blank("/tmp/sc2000-day21-baseline-story.sc2", "Day21Story").expect("blank");
    let mut state = test_state(city);

    let _ = state.sim_calendar_day21_demand_and_graphs();

    assert!(
        (0..7).any(|slot| state.city.pe_newspaper_story_id_at(slot) == Some(1)),
        "Win95 day 21 inserts baseline story 1 before its conditional news"
    );
}

#[test]
fn bulldoze_lowclass_selector2_leaves_north_of_post_cleanup_window_unchanged() {
    // The traced low-class selector-2 post-cleanup calls use the snapped 2x2
    // footprint (y=20..21 for this click); y=19 is outside that window.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-hwy-window.sc2", "BullHwyWindow").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    city.set_building(20, 19, 0x0d);
    city.set_zone(20, 19, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_terrain(20, 19, 0x7f);
    assert!(pe_destroy_building_selector2_low_class_cleanup_order(&city, 0x61, 20, 20).is_some());

    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 19), 0x0d);
    assert_eq!(
        state.city.zone_at(20, 19).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT | 0x80
    );
    assert_eq!(state.city.terrain_at(20, 19), 0x7f);
}

#[test]
fn win95_bulldozer_debits_per_accepted_cursor_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(10, 10, 112);
    city.set_zone(20, 20, 0x02);
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;

    assert!(state.apply_tool_at_tile(10, 10));
    assert_eq!(state.city.city_funds(), 19_999);
    assert!(matches!(state.city.building_at(10, 10), 1..=4));

    // The player Bulldozer clears rubble with PlaceTile(0), after its $1 gate;
    // the separate DestroyBuilding early-out for XBLD < 6 does not apply here.
    assert!(state.apply_tool_at_tile(10, 10));
    assert_eq!(state.city.city_funds(), 19_998);
    assert_eq!(state.city.building_at(10, 10), 0);

    // A zone marker on an empty low-class tile fails the rooted native gate.
    assert!(!state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.city_funds(), 19_998);

    assert!(!state.apply_tool_at_tile(30, 30));
    assert_eq!(state.city.city_funds(), 19_998);
}

#[test]
fn pipes_tool_enters_underground_bulldoze_and_query_keep_it() {
    let city = City::new_blank("/tmp/sc2000-ug-view.sc2", "ug").expect("blank");
    let mut state = test_state(city);
    assert!(!state.show_underground);

    state.apply_selected_tool(EditTool::Pipes);
    assert!(state.show_underground);

    state.apply_selected_tool(EditTool::Bulldoze);
    assert!(state.show_underground, "bulldoze stays underground");

    state.apply_selected_tool(EditTool::Query);
    assert!(state.show_underground, "query stays underground");

    state.apply_selected_tool(EditTool::Road);
    assert!(!state.show_underground, "surface tools leave underground");
}

#[test]
fn budget_window_controls_hit_their_visible_positions_at_every_scale() {
    for window in [(400, 360), (1024, 768), (2624, 1644)] {
        let dialog = CityInfoDialog::Budget;
        let page = city_info_dialog_rect_for(dialog, window);
        let center = |r: FloatRect| Vec2::new(r.x + r.w * 0.5, r.y + r.h * 0.5);
        for increase in [false, true] {
            for tax_index in 0..BUDGET_TAX_COUNT {
                assert_eq!(
                    city_info_dialog_hit_test(
                        center(budget_tax_adjust_rect(page, tax_index, increase)),
                        window,
                        dialog
                    ),
                    Some(CityInfoDialogAction::AdjustBudgetTax {
                        tax_index,
                        delta: if increase {
                            BUDGET_TAX_STEP
                        } else {
                            -BUDGET_TAX_STEP
                        }
                    })
                );
            }
            for service_index in 0..BUDGET_SERVICE_COUNT {
                assert_eq!(
                    city_info_dialog_hit_test(
                        center(budget_service_funding_adjust_rect(
                            page,
                            service_index,
                            increase
                        )),
                        window,
                        dialog
                    ),
                    Some(CityInfoDialogAction::AdjustBudgetFunding {
                        service_index,
                        delta: if increase {
                            BUDGET_FUNDING_STEP
                        } else {
                            -BUDGET_FUNDING_STEP
                        }
                    })
                );
            }
        }
        assert_eq!(
            city_info_dialog_hit_test(center(budget_window::close_rect(page)), window, dialog),
            Some(CityInfoDialogAction::Close)
        );
        assert_eq!(
            city_info_dialog_hit_test(center(budget_auto_budget_rect(page)), window, dialog),
            Some(CityInfoDialogAction::ToggleAutoBudget)
        );
        assert_eq!(
            city_info_dialog_hit_test(center(budget_bond_button_rect(page, true)), window, dialog),
            Some(CityInfoDialogAction::IssueBond)
        );
        assert_eq!(
            city_info_dialog_hit_test(center(budget_bond_button_rect(page, false)), window, dialog),
            Some(CityInfoDialogAction::RepayBond)
        );
    }
}

#[test]
fn windows_menu_keeps_the_seven_information_windows_and_separate_map_views() {
    use crate::menu_model::{MenuEntry, menus};
    let windows = menus().iter().find(|menu| menu.title == "Windows").unwrap();
    let entries: Vec<_> = windows
        .items
        .iter()
        .filter_map(|entry| match entry {
            MenuEntry::Command { label, accel, .. } => Some((*label, *accel)),
            _ => None,
        })
        .collect();
    assert_eq!(
        entries,
        vec![
            ("Map", Some("Ctrl+M")),
            ("Budget", Some("Ctrl+B")),
            ("Ordinances", Some("Ctrl+O")),
            ("Population", Some("Ctrl+C")),
            ("Industry", Some("Ctrl+I")),
            ("Graphs", Some("Ctrl+G")),
            ("Neighbors", Some("Ctrl+H"))
        ]
    );
    let views = menus().iter().find(|menu| menu.title == "View").unwrap();
    assert_eq!(
        views
            .items
            .iter()
            .filter(|e| matches!(
                e,
                MenuEntry::Command {
                    checkable: true,
                    ..
                }
            ))
            .count(),
        13
    );
    assert!(
        menus()
            .iter()
            .find(|menu| menu.title == "Help")
            .unwrap()
            .items
            .iter()
            .any(|e| matches!(
                e,
                MenuEntry::Command {
                    id: crate::menu_model::CMD_OPEN_SCENARIO_PROGRESS,
                    ..
                }
            ))
    );
}

#[test]
fn windows_menu_shortcuts_preserve_plain_and_shifted_map_keys() {
    use sdl3::keyboard::Mod;
    for (key, dialog) in [
        (Keycode::M, CityInfoDialog::Map),
        (Keycode::B, CityInfoDialog::Budget),
        (Keycode::O, CityInfoDialog::Ordinances),
        (Keycode::C, CityInfoDialog::Population),
        (Keycode::I, CityInfoDialog::Industry),
        (Keycode::G, CityInfoDialog::Graphs),
        (Keycode::H, CityInfoDialog::Neighbors),
    ] {
        assert!(
            matches!(windows_menu_shortcut(key, Mod::LCTRLMOD), Some(AppCommand::OpenCityInfo(actual)) if actual == dialog)
        );
        assert!(windows_menu_shortcut(key, Mod::NOMOD).is_none());
        assert!(windows_menu_shortcut(key, Mod::LCTRLMOD | Mod::LSHIFTMOD).is_none());
    }
}

#[test]
fn pe_auto_budget_toggle_matches_native_menu_cases() {
    let mut cases = 0;
    for line in include_str!("../fixtures/pe_auto_toggle_native_20261023.csv")
        .lines()
        .skip(1)
    {
        let row: Vec<u32> = line
            .split(',')
            .map(|value| value.parse().unwrap())
            .collect();
        let mut city = City::new_blank("pe-auto-native.sc2", "native option").unwrap();
        city.set_misc_u32_at(AUTO_BUDGET_MISC_OFFSET, row[0]);
        city.set_city_funds(123456);
        let mut state = test_state(city);
        state.traffic.set_speed(GameSpeed::Cheetah);
        state.toggle_auto_budget();
        assert_eq!(
            state.city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
            Some(row[1])
        );
        assert_eq!(state.city.city_funds(), row[4] as i32);
        assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
        cases += 1;
    }
    assert_eq!(cases, 5);
}

#[test]
fn pe_modal_newspaper_holds_clock_and_restores_entry_speed() {
    for speed in [
        GameSpeed::Paused,
        GameSpeed::Llama,
        GameSpeed::AfricanSwallow,
    ] {
        let city = blank_city_layout_fixture("pe-modal-paper.sc2", "modal paper");
        let mut state = test_state(city);
        state.apply_state_app_command(&AppCommand::SetSpeed(speed));
        let saved_speed = state.city.misc_u32_at(MISC_SPEED_OFFSET);
        let before = state.game_clock;
        state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Subscription));
        assert_eq!(state.traffic.speed(), GameSpeed::Paused, "{speed:?}");
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(1600))
                .days,
            0
        );
        assert_eq!(state.game_clock, before);
        assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), saved_speed);
        state.close_city_info_dialog();
        assert_eq!(state.traffic.speed(), speed);
        assert_eq!(state.menu_checks.speed, speed);
        assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), saved_speed);
    }
}

#[test]
fn pe_modal_budget_restores_speed_in_both_layouts_without_persisting_pause() {
    for path in ["pe-modal-budget.sc2", "pe-modal-budget.city"] {
        let city = blank_city_layout_fixture(path, "modal budget");
        let mut state = test_state(city);
        state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Llama));
        let saved_speed = state.city.misc_u32_at(MISC_SPEED_OFFSET);
        state.open_january_budget_review();
        assert_eq!(state.traffic.speed(), GameSpeed::Paused);
        assert_eq!(
            state.city.misc_u32_at(MISC_SPEED_OFFSET),
            saved_speed,
            "{path}"
        );
        state.close_modal_dialogs();
        assert_eq!(state.traffic.speed(), GameSpeed::Llama, "{path}");
        assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), saved_speed);
    }
}

#[test]
fn pe_modal_live_january_waits_for_budget_before_financial_publication() {
    let mut city = blank_city_layout_fixture("pe-modal-january.sc2", "modal January");
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        24,
    );
    city.set_city_funds(1000);
    city.set_misc_u32_at(
        budget_row_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        9000,
    );
    set_city_auto_budget_enabled(&mut city, false);
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
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    assert_eq!(
        state.city.city_funds(),
        1000,
        "settlement waits for DialogBox return"
    );
    assert_eq!(
        state.city.saved_date(),
        Some(CityDate {
            year: 1901,
            month: 1
        })
    );
    state.advance_frame_simulation(Duration::from_millis(400));
    assert_eq!(state.city.city_funds(), 1000);
    state.close_city_info_dialog();
    assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
    assert_eq!(state.city.city_funds(), 1010);
    assert_eq!(state.game_clock, GameClock::new(1901, 1));
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
    assert_eq!(
        state.city.city_funds(),
        1010,
        "continuation settles only once"
    );
}

#[test]
fn pe_modal_subscription_opens_only_on_native_april_august_day_zero() {
    for month in 1..=12 {
        let mut city = blank_city_layout_fixture("pe-modal-subscription.sc2", "subscription");
        set_city_auto_budget_enabled(&mut city, true);
        set_city_newspaper_subscription_enabled(&mut city, true);
        let mut state = test_state(city);
        state.no_disasters_enabled = true;
        let previous = if month == 1 {
            GameClock::new_with_day(1899, 12, 24)
        } else {
            GameClock::new_with_day(1900, month - 1, 24)
        };
        state.game_clock = previous;
        state.city.set_saved_date_and_day(
            CityDate {
                year: previous.year(),
                month: previous.month(),
            },
            24,
        );
        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(200))
                .days,
            1
        );
        assert_eq!(
            state.city_info_dialog,
            matches!(month, 4 | 8)
                .then_some(CityInfoDialog::Newspaper(NewspaperEdition::Subscription)),
            "month {month}"
        );
        assert_eq!(
            state.traffic.speed(),
            if matches!(month, 4 | 8) {
                GameSpeed::Paused
            } else {
                GameSpeed::Cheetah
            }
        );
    }
}

#[test]
fn pe_modal_monthly_budget_snapshots_accrue_on_phase_zero_not_day_twenty_one() {
    let mut city = blank_city_layout_fixture("pe-modal-budget-phase.sc2", "budget phase");
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 2,
        },
        20,
    );
    set_city_auto_budget_enabled(&mut city, false);
    let current = budget_service_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
    let funding = budget_service_misc_offset(0, BUDGET_SERVICE_FUNDING_FIELD).unwrap();
    let annual = budget_service_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap();
    city.set_misc_u32_at(current, 9);
    city.set_misc_u32_at(funding, 100);
    city.set_misc_u32_at(annual, 0);
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.game_clock = GameClock::new_with_day(1900, 2, 20);
    state.traffic.set_speed(GameSpeed::Cheetah);
    state.advance_frame_simulation(Duration::from_millis(200));
    assert_eq!(state.game_clock.day(), 21);
    assert_eq!(state.city.misc_u32_at(annual), Some(0));
    assert_eq!(state.city.misc_u32_at(current), Some(9));
    for _ in 0..4 {
        state.advance_frame_simulation(Duration::from_millis(200));
    }
    assert_eq!(state.game_clock, GameClock::new(1900, 3));
    assert_eq!(state.city.misc_u32_at(annual), Some(900));
    let (count_field, funding_field) = pe_budget_month_history_fields(2);
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, count_field).unwrap()),
        Some(9)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, funding_field).unwrap()),
        Some(100)
    );
    state.advance_frame_simulation(Duration::ZERO);
    assert_eq!(state.city.misc_u32_at(annual), Some(900));
}

#[test]
fn pe_modal_replacement_retains_outer_speed_and_rejects_parent_speed_commands() {
    let mut city = blank_city_layout_fixture("pe-modal-replace.sc2", "replacement");
    city.set_building(40, 40, 0x1d);
    city.set_traffic_density(40, 40, 128);
    let mut state = test_state(city);
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Cheetah));
    state.traffic_redraw_epoch = 17;
    state.traffic_redraw_accumulator = 0.07;
    let density = state.city.traffic_density_slice().unwrap().to_vec();
    let traffic = state.traffic.congestion_slice().to_vec();
    let saved_speed = state.city.misc_u32_at(MISC_SPEED_OFFSET);
    state.open_city_info_dialog(CityInfoDialog::Budget);
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Courier));
    let rng = (
        state.pe_process_crt.seed(),
        state.pe_process_lfsr.state(),
        state.pe_process_game_rand.state(),
    );
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Turtle));
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
    state.advance_frame_simulation(Duration::from_millis(1600));
    assert_eq!(
        (
            state.pe_process_crt.seed(),
            state.pe_process_lfsr.state(),
            state.pe_process_game_rand.state()
        ),
        rng
    );
    assert_eq!(state.city.traffic_density_slice().unwrap(), density);
    assert_eq!(state.traffic.congestion_slice(), traffic);
    assert_eq!(state.traffic_redraw_epoch, 17);
    assert_eq!(state.traffic_redraw_accumulator, 0.07);
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), saved_speed);
    state.close_modal_dialogs();
    assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
}

#[test]
fn pe_modal_manual_fastest_preserves_paused_timer_pending_on_return() {
    let city = blank_city_layout_fixture("pe-modal-fastest.sc2", "fastest paper");
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.traffic.set_speed(GameSpeed::AfricanSwallow);
    let before = state.game_clock;
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Subscription));
    state.advance_frame_simulation(Duration::from_millis(1600));
    assert_eq!(state.game_clock, before);
    state.close_city_info_dialog();
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
}

#[test]
fn pe_modal_mac_fidelity_does_not_inherit_native_pe_pause() {
    let mut state = test_state(blank_city_layout_fixture("pe-modal-Mac.city", "Mac modal"));
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.traffic.set_speed(GameSpeed::Llama);
    state.open_city_info_dialog(CityInfoDialog::Budget);
    assert_eq!(state.traffic.speed(), GameSpeed::Llama);
    state.close_city_info_dialog();
    state.open_january_budget_review();
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
    state.close_city_info_dialog();
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
}

#[test]
fn pe_modal_loading_another_city_discards_the_old_phase_continuation() {
    let mut city = blank_city_layout_fixture("pe-modal-old-city.sc2", "old city");
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        24,
    );
    city.set_city_funds(1000);
    city.set_misc_u32_at(
        budget_row_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        9000,
    );
    set_city_auto_budget_enabled(&mut city, false);
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    state.traffic.set_speed(GameSpeed::Cheetah);
    state.advance_frame_simulation(Duration::from_millis(200));
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    let path = std::env::temp_dir().join(format!(
        "citygame-modal-new-document-{}.sc2",
        std::process::id()
    ));
    let mut next = City::new_blank(&path, "new city").unwrap();
    next.set_city_funds(777);
    next.set_saved_date_and_day(
        CityDate {
            year: 1905,
            month: 9,
        },
        3,
    );
    next.save(&path).unwrap();
    state.load_city(&path, (800, 600), 1.0).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!state.pe_modal_active());
    state.advance_frame_simulation(Duration::ZERO);
    assert_eq!(
        state.city.city_funds(),
        777,
        "old January settlement must not run in the new city"
    );
    assert_eq!(state.city.name, "new city");
}

#[test]
fn pe_modal_phase_zero_preserves_raw_funding_in_current_and_history() {
    let mut city = blank_city_layout_fixture("pe-modal-raw-funding.sc2", "raw funding");
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 2,
        },
        24,
    );
    set_city_auto_budget_enabled(&mut city, false);
    let current = budget_service_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
    let funding = budget_service_misc_offset(0, BUDGET_SERVICE_FUNDING_FIELD).unwrap();
    let annual = budget_service_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap();
    let raw = 0xcafe_0078_u32;
    city.set_misc_u32_at(current, 9);
    city.set_misc_u32_at(funding, raw);
    city.set_misc_u32_at(annual, 0);
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.game_clock = GameClock::new_with_day(1900, 2, 24);
    state.traffic.set_speed(GameSpeed::Cheetah);
    state.advance_frame_simulation(Duration::from_millis(200));
    assert_eq!(state.city.misc_u32_at(funding), Some(raw));
    assert_eq!(state.city.misc_u32_at(annual), Some(raw.wrapping_mul(9)));
    let (count_field, funding_field) = pe_budget_month_history_fields(2);
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, count_field).unwrap()),
        Some(9)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, funding_field).unwrap()),
        Some(raw)
    );
    assert_eq!(
        state.city.misc_u32_at(current),
        Some(0),
        "live cost is rebuilt without changing funding"
    );
}

#[test]
fn new_city_resets_camera_from_previous_rotated_city_in_both_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for rotation in 0..4 {
            let mut city = City::new_blank("/tmp/new-city-rotation.sc2", "Before").unwrap();
            city.kind = kind;
            let mut state = test_state(city);
            state.rotate_view(rotation);
            assert_eq!(state.camera_rotation, rotation as u8);
            state.new_city().unwrap();
            assert_eq!(state.city.rotation(), 0);
            assert_eq!(
                state.camera_rotation,
                state.city.rotation(),
                "{kind:?} old={rotation}"
            );
        }
    }
}

#[test]
fn generated_city_keeps_power_drawing_aligned_with_camera_after_rotated_city() {
    for flow in [
        title_screen::NewCityStartFlow::GenerateAndPlay,
        title_screen::NewCityStartFlow::GenerateAndEdit,
    ] {
        let city = City::new_blank("/tmp/generated-power-rotation.sc2", "Before").unwrap();
        let mut state = test_state(city);
        state.rotate_view(3);
        state
            .new_city_from_title("New grid".into(), 100_000, 1900, 1, 12345, flow)
            .unwrap();
        assert_eq!(state.camera_rotation, state.city.rotation());
        // Keep the generated map but clear two flat test runs for deterministic
        // axis checks, without imported artwork or original city fixtures.
        let runs = [((40, 40), (46, 40)), ((50, 40), (50, 46))];
        state.tool = EditTool::Power;
        for (start, end) in runs {
            let path = network_drag_path_for_kind(start, end, NetworkKind::Power);
            for &(x, y) in &path {
                state.city.set_terrain(x, y, 0);
                state.city.set_building(x, y, 0);
                state.city.set_tile_bits(x, y, TileBits::new(0));
            }
            assert_eq!(
                state.place_network_path(&path, NetworkKind::Power),
                path.len()
            );
        }
        for _ in 0..4 {
            for (start, end) in runs {
                let (a, b) = (
                    rotate_tile_coords(start.0, start.1, state.camera_rotation),
                    rotate_tile_coords(end.0, end.1, state.camera_rotation),
                );
                let expected = if a.0 != b.0 { 0x0f } else { 0x0e };
                for (x, y) in network_drag_path_for_kind(start, end, NetworkKind::Power) {
                    assert_eq!(
                        crate::tiles::rotated_building_id(
                            state.city.building_at(x, y),
                            state.city.rotation()
                        ),
                        expected
                    );
                    assert!(state.city.tile_bits_at(x, y).unwrap().powerable());
                }
            }
            state.rotate_view(1);
        }
    }
}

#[test]
fn files_export_snapshot_does_not_mark_the_live_city_saved_on_cancel() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/unsaved-export-city.sc2", "Export Test").unwrap();
        city.kind = kind;
        city.set_city_funds(12345);
        let mut state = test_state(city);
        state.fit_map((800, 600), 1.0);
        state.city.mark_all_dirty();
        let original = state.city.path.clone();
        let modified = state.city.is_modified();
        let saved_view = state.city.saved_view();
        let mut snapshot = state.city_save_snapshot((800, 600), 1.0);
        let export = std::env::temp_dir().join(format!(
            "citygame-export-snapshot-{}-{kind:?}.sc2",
            std::process::id()
        ));
        snapshot.save(&export).unwrap();
        let loaded = City::load(&export).unwrap();
        assert_eq!(loaded.city_funds(), 12345);
        if kind.is_win95() {
            assert_eq!(loaded.saved_view(), snapshot.saved_view());
        }
        assert_eq!(state.city.path, original);
        assert_eq!(state.city.is_modified(), modified);
        assert_eq!(state.city.saved_view(), saved_view);
        assert!(
            !state.city.take_dirty_tiles().is_empty(),
            "preparing a Files snapshot must not erase pending redraws"
        );
        std::fs::remove_file(export).unwrap();
    }
}
