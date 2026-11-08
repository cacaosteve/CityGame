use super::*;

#[test]
fn city_name_font_displays_lowercase_distinctly_and_keeps_descenders_in_the_field() {
    let pixels = |value: &str, preserve_case: bool| {
        let mut solids = Vec::new();
        if preserve_case {
            push_pixel_text_preserving_case(&mut solids, value, 0.0, 0.0, 1.0, [1.0; 4], 0.0);
        } else {
            push_pixel_text(&mut solids, value, 0.0, 0.0, 1.0, [1.0; 4], 0.0);
        }
        solids
            .into_iter()
            .map(|s| (s.x, s.y, s.w, s.h))
            .collect::<Vec<_>>()
    };
    for ch in 'a'..='z' {
        let lower = ch.to_string();
        let upper = ch.to_ascii_uppercase().to_string();
        assert_ne!(pixels(&lower, true), pixels(&upper, true), "{ch}");
        assert_eq!(pixels(&lower, false), pixels(&upper, false));
        assert!(
            pixels(&lower, true)
                .iter()
                .all(|&(x, y, w, h)| x >= 0.0 && x + w <= 3.0 && y >= 0.0 && y + h <= 7.0)
        );
    }
}

#[test]
fn city_palette_logical_size_is_twice_mac_at_reference() {
    let (w, h) = palette_logical_size(AppMode::CityView);
    assert_eq!(w, 142);
    assert_eq!(h, 624);
}

#[test]
fn edit_palette_logical_size_is_twice_mac_at_reference() {
    let (w, h) = palette_logical_size(AppMode::EditNewMap);
    assert_eq!(w, 100);
    assert_eq!(h, 612);
}

#[test]
fn clicking_rotate_arrow_centers_returns_rotate_actions() {
    let logical = palette_logical_size(AppMode::CityView);
    let m = UiMetrics::for_logical_height(logical.1);
    let s = m.palette_scale;
    // Left arrow src [9,153,26,19] center, right arrow src [36,153,26,19] center.
    let ccw = Vec2::new((9.0 + 13.0) * s, (153.0 + 9.5) * s);
    let cw = Vec2::new((36.0 + 13.0) * s, (153.0 + 9.5) * s);
    assert_eq!(
        palette_hit_test(ccw, logical, 1.0, AppMode::CityView, None),
        Some(UiAction::RotateViewCcw)
    );
    assert_eq!(
        palette_hit_test(cw, logical, 1.0, AppMode::CityView, None),
        Some(UiAction::RotateViewCw)
    );
}

#[test]
fn clicking_zoom_button_centers_returns_zoom_actions() {
    let logical = palette_logical_size(AppMode::CityView);
    let m = UiMetrics::for_logical_height(logical.1);
    let s = m.palette_scale;
    let zoom_out = Vec2::new((1.0 + 11.5) * s, (177.0 + 12.0) * s);
    let zoom_in = Vec2::new((24.0 + 11.5) * s, (177.0 + 12.0) * s);
    let center = Vec2::new((47.0 + 11.5) * s, (177.0 + 12.0) * s);
    assert_eq!(
        palette_hit_test(zoom_out, logical, 1.0, AppMode::CityView, None),
        Some(UiAction::ZoomOut)
    );
    assert_eq!(
        palette_hit_test(zoom_in, logical, 1.0, AppMode::CityView, None),
        Some(UiAction::ZoomIn)
    );
    assert_eq!(
        palette_hit_test(center, logical, 1.0, AppMode::CityView, None),
        Some(UiAction::CenterMap)
    );
}

#[test]
fn city_tool_buttons_map_inside_palette() {
    let logical = palette_logical_size(AppMode::CityView);
    let buttons = palette_button_rects(logical, 1.0, AppMode::CityView);
    assert_eq!(buttons.len(), CITY_PALETTE_BUTTONS.len());
    let m = UiMetrics::for_logical_height(logical.1);
    let palette_right = m.palette_width(AppMode::CityView);
    let palette_bottom = m.palette_height(AppMode::CityView);
    for button in buttons {
        assert!(button.x >= 0.0);
        assert!(button.y >= 0.0);
        assert!(button.x + button.w <= palette_right + 0.5);
        assert!(button.y + button.h <= palette_bottom + 0.5);
    }
}

#[test]
fn every_city_palette_button_center_returns_its_declared_action() {
    let logical = palette_logical_size(AppMode::CityView);
    for button in palette_button_rects(logical, 1.0, AppMode::CityView) {
        assert_eq!(
            palette_hit_test(
                Vec2::new(button.x + button.w * 0.5, button.y + button.h * 0.5),
                logical,
                1.0,
                AppMode::CityView,
                None,
            ),
            Some(button.action),
            "{:?}",
            button.action
        );
    }
}

#[test]
fn sign_and_query_use_the_complete_two_column_palette_cells() {
    let logical = palette_logical_size(AppMode::CityView);
    let buttons = palette_button_rects(logical, 1.0, AppMode::CityView);
    let sign = buttons
        .iter()
        .find(|button| button.action == UiAction::SelectTool(EditTool::Sign))
        .expect("sign button");
    let query = buttons
        .iter()
        .find(|button| button.action == UiAction::SelectTool(EditTool::Query))
        .expect("query button");

    assert_eq!(sign.src, [1, 121, 34, 26]);
    assert_eq!(query.src, [36, 121, 33, 26]);
    assert!(sign.x + sign.w < query.x);
}

#[test]
fn every_edit_palette_button_center_returns_its_declared_action() {
    let logical = palette_logical_size(AppMode::EditNewMap);
    for button in palette_button_rects(logical, 1.0, AppMode::EditNewMap) {
        assert_eq!(
            palette_hit_test(
                Vec2::new(button.x + button.w * 0.5, button.y + button.h * 0.5),
                logical,
                1.0,
                AppMode::EditNewMap,
                None,
            ),
            Some(button.action),
            "{:?}",
            button.action
        );
    }

    let done = backing_rect(logical, 1.0, Vec2::ZERO, EDIT_DONE_BUTTON);
    assert_eq!(
        palette_hit_test(
            Vec2::new(done.x + done.w * 0.5, done.y + done.h * 0.5),
            logical,
            1.0,
            AppMode::EditNewMap,
            None,
        ),
        Some(UiAction::DoneEditMap)
    );
}

#[test]
fn edit_palette_view_buttons_match_original_palette_rows() {
    let logical = palette_logical_size(AppMode::EditNewMap);
    let scale = UiMetrics::for_logical_height(logical.1).palette_scale;
    let cases = [
        ((13.0, 228.5), UiAction::ZoomOut),
        ((37.0, 228.5), UiAction::ZoomIn),
        ((13.0, 249.5), UiAction::RotateViewCcw),
        ((37.0, 249.5), UiAction::RotateViewCw),
        ((13.0, 270.5), UiAction::CenterMap),
        ((37.0, 270.5), UiAction::OpenHelp),
    ];

    for ((x, y), expected) in cases {
        assert_eq!(
            palette_hit_test(
                Vec2::new(x * scale, y * scale),
                logical,
                1.0,
                AppMode::EditNewMap,
                None,
            ),
            Some(expected)
        );
    }
}

#[test]
fn edit_palette_generator_buttons_match_original_palette_rows() {
    let logical = palette_logical_size(AppMode::EditNewMap);
    let scale = UiMetrics::for_logical_height(logical.1).palette_scale;
    let cases = [
        ((25.0, 9.0), UiAction::ToggleEditCoast),
        ((25.0, 25.0), UiAction::ToggleEditRiver),
        ((25.0, 96.0), UiAction::GenerateEditMap),
    ];

    for ((x, y), expected) in cases {
        assert_eq!(
            palette_hit_test(
                Vec2::new(x * scale, y * scale),
                logical,
                1.0,
                AppMode::EditNewMap,
                None,
            ),
            Some(expected)
        );
    }
}

#[test]
fn edit_palette_terrain_buttons_match_original_palette_rows() {
    let logical = palette_logical_size(AppMode::EditNewMap);
    let scale = UiMetrics::for_logical_height(logical.1).palette_scale;
    let cases = [
        ((13.0, 117.5), UiAction::SelectTool(EditTool::RaiseTerrain)),
        ((37.0, 117.5), UiAction::SelectTool(EditTool::LowerTerrain)),
        (
            (13.0, 138.5),
            UiAction::SelectTool(EditTool::StretchTerrain),
        ),
        ((37.0, 138.5), UiAction::SelectTool(EditTool::LevelTerrain)),
        ((13.0, 159.5), UiAction::RaiseEditSeaLevel),
        ((37.0, 159.5), UiAction::LowerEditSeaLevel),
        ((13.0, 180.5), UiAction::SelectTool(EditTool::Water)),
        ((37.0, 180.5), UiAction::SelectTool(EditTool::Stream)),
        ((13.0, 201.5), UiAction::SelectTool(EditTool::Trees)),
        ((37.0, 201.5), UiAction::SelectTool(EditTool::Forest)),
    ];

    for ((x, y), expected) in cases {
        assert_eq!(
            palette_hit_test(
                Vec2::new(x * scale, y * scale),
                logical,
                1.0,
                AppMode::EditNewMap,
                None,
            ),
            Some(expected)
        );
    }
}

#[test]
fn edit_map_slider_values_default_to_sc2k_documented_settings() {
    let mut values = EditMapSliderValues::default();
    assert_eq!(values.value(EditMapSlider::Hills), 12);
    assert_eq!(values.value(EditMapSlider::Water), 5);
    assert_eq!(values.value(EditMapSlider::Trees), 15);

    values.set(EditMapSlider::Water, 99);
    assert_eq!(values.value(EditMapSlider::Water), EDIT_MAP_SLIDER_MAX);
}

#[test]
fn edit_map_slider_hit_test_matches_original_track_range() {
    let logical = palette_logical_size(AppMode::EditNewMap);
    let scale = UiMetrics::for_logical_height(logical.1).palette_scale;
    let cases = [
        ((8.0, 30.0), Some((EditMapSlider::Hills, 47))),
        ((8.0, 77.0), Some((EditMapSlider::Hills, 0))),
        ((24.0, 72.0), Some((EditMapSlider::Water, 5))),
        ((40.0, 62.0), Some((EditMapSlider::Trees, 15))),
        ((8.0, 30.0), None),
    ];

    for (index, ((x, y), expected)) in cases.into_iter().enumerate() {
        let mode = if expected.is_some() {
            AppMode::EditNewMap
        } else {
            AppMode::CityView
        };
        assert_eq!(
            edit_map_slider_hit_test(Vec2::new(x * scale, y * scale), logical, 1.0, mode, None),
            expected,
            "case {index}"
        );
    }
}

#[test]
fn every_tool_menu_row_center_returns_its_declared_tool() {
    let logical = palette_logical_size(AppMode::CityView);
    for menu in [
        ToolMenu::Residential,
        ToolMenu::Commercial,
        ToolMenu::Industrial,
        ToolMenu::Parks,
        ToolMenu::Power,
        ToolMenu::Civic,
        ToolMenu::Education,
        ToolMenu::Water,
        ToolMenu::Terrain,
        ToolMenu::TerrainEdit,
        ToolMenu::Road,
        ToolMenu::Rail,
        ToolMenu::Transport,
        ToolMenu::Emergency,
        ToolMenu::Rewards,
    ] {
        let rows = menu_item_rects(logical, 1.0, AppMode::CityView, menu);
        assert_eq!(rows.len(), tool_menu_items(menu).len(), "{menu:?}");
        for (item, rect) in rows {
            assert_eq!(
                palette_hit_test(
                    Vec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5),
                    logical,
                    1.0,
                    AppMode::CityView,
                    Some(menu),
                ),
                Some(UiAction::SelectTool(item.tool)),
                "{menu:?} {:?}",
                item.tool
            );
        }
    }
}

#[test]
fn palette_hover_label_names_buttons_and_open_menu_rows() {
    let logical = palette_logical_size(AppMode::CityView);
    let m = UiMetrics::for_logical_height(logical.1);
    let scale = m.palette_scale;
    let road_button = Vec2::new((1.0 + 11.5) * scale, (49.0 + 12.0) * scale);
    assert_eq!(
        palette_hover_label(road_button, logical, 1.0, AppMode::CityView, None),
        Some("Roads")
    );

    let rows = menu_item_rects(logical, 1.0, AppMode::CityView, ToolMenu::Road);
    let highway = rows[1].1;
    assert_eq!(
        palette_hover_label(
            Vec2::new(highway.x + highway.w * 0.5, highway.y + highway.h * 0.5),
            logical,
            1.0,
            AppMode::CityView,
            Some(ToolMenu::Road),
        ),
        Some("Highway")
    );
}

#[test]
fn city_display_layer_buttons_match_original_palette_rows() {
    let logical = palette_logical_size(AppMode::CityView);
    let scale = UiMetrics::for_logical_height(logical.1).palette_scale;
    let cases = [
        ((18.0, 273.0), UiAction::ToggleBuildings),
        ((52.5, 273.0), UiAction::ToggleSigns),
        ((18.0, 288.0), UiAction::ToggleInfrastructure),
        ((52.5, 288.0), UiAction::ToggleZones),
        ((23.0, 303.0), UiAction::ToggleUnderground),
    ];

    for ((x, y), expected) in cases {
        assert_eq!(
            palette_hit_test(
                Vec2::new(x * scale, y * scale),
                logical,
                1.0,
                AppMode::CityView,
                None,
            ),
            Some(expected)
        );
    }
}

#[test]
fn city_information_and_help_buttons_match_original_palette_rows() {
    let logical = palette_logical_size(AppMode::CityView);
    let scale = UiMetrics::for_logical_height(logical.1).palette_scale;
    let cases = [
        ((12.5, 211.5), UiAction::OpenCityMap),
        ((35.5, 211.5), UiAction::OpenCityGraphs),
        ((12.5, 231.5), UiAction::OpenCityPopulation),
        ((35.5, 231.5), UiAction::OpenCityIndustry),
        ((12.5, 251.5), UiAction::OpenCityNeighbors),
        ((35.5, 251.5), UiAction::OpenBudget),
        ((57.5, 303.0), UiAction::OpenHelp),
    ];

    for ((x, y), expected) in cases {
        assert_eq!(
            palette_hit_test(
                Vec2::new(x * scale, y * scale),
                logical,
                1.0,
                AppMode::CityView,
                None,
            ),
            Some(expected)
        );
    }
}

#[test]
fn clicking_industrial_button_opens_menu_and_rows_select_density() {
    let logical = palette_logical_size(AppMode::CityView);
    let m = UiMetrics::for_logical_height(logical.1);
    let s = m.palette_scale;
    let industrial_button = Vec2::new((47.0 + 11.5) * s, (73.0 + 12.0) * s);
    assert_eq!(
        palette_hit_test(industrial_button, logical, 1.0, AppMode::CityView, None),
        Some(UiAction::OpenMenu(ToolMenu::Industrial))
    );

    let rows = menu_item_rects(logical, 1.0, AppMode::CityView, ToolMenu::Industrial);
    assert_eq!(rows.len(), 2);
    let light = rows[0].1;
    let dense = rows[1].1;
    assert_eq!(
        palette_hit_test(
            Vec2::new(light.x + light.w * 0.5, light.y + light.h * 0.5),
            logical,
            1.0,
            AppMode::CityView,
            Some(ToolMenu::Industrial),
        ),
        Some(UiAction::SelectTool(EditTool::ZoneIndustrial))
    );
    assert_eq!(
        palette_hit_test(
            Vec2::new(dense.x + dense.w * 0.5, dense.y + dense.h * 0.5),
            logical,
            1.0,
            AppMode::CityView,
            Some(ToolMenu::Industrial),
        ),
        Some(UiAction::SelectTool(EditTool::ZoneIndustrialDense))
    );
}

#[test]
fn clicking_transport_buttons_opens_observed_sc2k_menus() {
    let logical = palette_logical_size(AppMode::CityView);
    let m = UiMetrics::for_logical_height(logical.1);
    let s = m.palette_scale;

    assert_eq!(
        palette_hit_test(
            Vec2::new((1.0 + 11.5) * s, (49.0 + 12.0) * s),
            logical,
            1.0,
            AppMode::CityView,
            None,
        ),
        Some(UiAction::OpenMenu(ToolMenu::Road))
    );
    assert_eq!(
        palette_hit_test(
            Vec2::new((24.0 + 11.5) * s, (49.0 + 12.0) * s),
            logical,
            1.0,
            AppMode::CityView,
            None,
        ),
        Some(UiAction::OpenMenu(ToolMenu::Rail))
    );
    assert_eq!(
        palette_hit_test(
            Vec2::new((47.0 + 11.5) * s, (49.0 + 12.0) * s),
            logical,
            1.0,
            AppMode::CityView,
            None,
        ),
        Some(UiAction::OpenMenu(ToolMenu::Transport))
    );
}

#[test]
fn pressed_button_image_shifts_at_eight_x_source_density() {
    let button = ButtonRect {
        action: UiAction::SelectTool(EditTool::Road),
        src: [1, 49, 23, 24],
        x: 2.0,
        y: 4.0,
        w: 46.0,
        h: 48.0,
    };
    let mut images = Vec::new();
    push_button_image_clipped(
        &mut images,
        7,
        button,
        Vec2::new(2.0, 2.0),
        UI_DEPTH,
        [1.0, 1.0, 1.0, 1.0],
    );

    assert_eq!(images.len(), 1);
    let image = &images[0];
    assert_eq!(image.src_x, 8);
    assert_eq!(image.src_y, 392);
    assert_eq!(image.src_w, 176);
    assert_eq!(image.src_h, 184);
    assert_eq!(image.x, 4.0);
    assert_eq!(image.y, 6.0);
    assert_eq!(image.w, 44.0);
    assert_eq!(image.h, 46.0);
}

#[test]
fn disabled_button_uses_one_neutral_veil_instead_of_a_checker_pattern() {
    let button = ButtonRect {
        action: UiAction::ZoomOut,
        src: [1, 177, 23, 24],
        x: 2.0,
        y: 4.0,
        w: 46.0,
        h: 48.0,
    };
    let mut images = Vec::new();
    let mut solids = Vec::new();

    push_disabled_button_state(&mut images, &mut solids, 7, button);

    assert_eq!(images.len(), 1);
    assert_eq!(solids.len(), 6);
    let veil = solids.last().expect("disabled veil");
    assert_eq!((veil.x, veil.y, veil.w, veil.h), (2.0, 4.0, 46.0, 48.0));
    assert_eq!(veil.color, [0.62, 0.62, 0.62, 0.46]);
}

#[test]
fn submenu_tools_match_parent_palette_buttons() {
    assert!(action_matches_active_tool(
        UiAction::SelectTool(EditTool::Query),
        EditTool::Query,
    ));
    assert!(action_matches_active_tool(
        UiAction::SelectTool(EditTool::ZoneResidential),
        EditTool::ZoneResidentialDense,
    ));
    assert!(action_matches_active_tool(
        UiAction::SelectTool(EditTool::ZoneCommercial),
        EditTool::ZoneCommercialDense,
    ));
    assert!(action_matches_active_tool(
        UiAction::SelectTool(EditTool::ZoneIndustrial),
        EditTool::ZoneIndustrialDense,
    ));
    assert!(action_matches_active_tool(
        UiAction::OpenMenu(ToolMenu::Road),
        EditTool::Highway,
    ));
    assert!(action_matches_active_tool(
        UiAction::OpenMenu(ToolMenu::Rail),
        EditTool::Subway,
    ));

    let menus = [
        ToolMenu::Residential,
        ToolMenu::Commercial,
        ToolMenu::Industrial,
        ToolMenu::Parks,
        ToolMenu::Power,
        ToolMenu::Civic,
        ToolMenu::Education,
        ToolMenu::Water,
        ToolMenu::Terrain,
        ToolMenu::TerrainEdit,
        ToolMenu::Road,
        ToolMenu::Rail,
        ToolMenu::Transport,
        ToolMenu::Emergency,
        ToolMenu::Rewards,
    ];
    for menu in menus {
        for item in tool_menu_items(menu) {
            assert!(
                action_matches_active_tool(UiAction::OpenMenu(menu), item.tool),
                "{menu:?} should stay pressed for {:?}",
                item.tool
            );
        }
    }

    for active_tool in [
        EditTool::PowerPlantCoal,
        EditTool::PowerPlantHydro,
        EditTool::PowerPlantOil,
        EditTool::PowerPlantGas,
        EditTool::PowerPlantNuclear,
        EditTool::PowerPlantWind,
        EditTool::PowerPlantSolar,
        EditTool::PowerPlantMicrowave,
        EditTool::PowerPlantFusion,
    ] {
        assert!(
            action_matches_active_tool(UiAction::OpenMenu(ToolMenu::Power), active_tool),
            "Power menu should stay pressed for {active_tool:?}"
        );
    }

    for active_tool in [
        EditTool::ArcologyPlymouth,
        EditTool::ArcologyForest,
        EditTool::ArcologyDarco,
        EditTool::ArcologyLaunch,
    ] {
        assert!(
            action_matches_active_tool(UiAction::OpenMenu(ToolMenu::Rewards), active_tool),
            "Rewards menu should stay pressed for {active_tool:?}"
        );
    }
}

#[test]
fn chooser_menu_rows_match_selected_dialog_choices() {
    assert!(menu_item_matches_active_tool(
        EditTool::PowerPlant,
        EditTool::PowerPlantCoal,
    ));
    assert!(menu_item_matches_active_tool(
        EditTool::PowerPlant,
        EditTool::PowerPlantFusion,
    ));
    assert!(!menu_item_matches_active_tool(
        EditTool::Power,
        EditTool::PowerPlantCoal,
    ));

    assert!(menu_item_matches_active_tool(
        EditTool::Arcologies,
        EditTool::ArcologyForest,
    ));
    assert!(menu_item_matches_active_tool(
        EditTool::Arcologies,
        EditTool::ArcologyLaunch,
    ));
    assert!(!menu_item_matches_active_tool(
        EditTool::MayorHouse,
        EditTool::ArcologyForest,
    ));
}

#[test]
fn tool_menu_items_match_observed_sc2k_choices() {
    assert_eq!(
        tool_menu_items(ToolMenu::Residential)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Light Residential", "Dense Residential"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Commercial)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Light Commercial", "Dense Commercial"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Industrial)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Light Industrial", "Dense Industrial"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Road)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Road", "Highway", "Tunnel", "Onramp", "Bus Depot"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Rail)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Rail", "Subway", "Rail Depot", "Sub Station", "Sub<-->Rail"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Transport)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Seaport", "Airport"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Power)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Power Lines", "Power Plant..."]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Civic)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Police", "Fire Station", "Hospital", "Prison"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Education)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["School", "College", "Library", "Museum"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Water)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec![
            "Pipes",
            "Water Pump",
            "Water Tower",
            "Treatment",
            "Desalinization"
        ]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Parks)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Small Park", "Big Park", "Zoo", "Stadium", "Marina"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Terrain)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec!["Trees", "Water"]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::TerrainEdit)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec![
            "Demolish/Clear",
            "Level Terrain",
            "Raise Terrain",
            "Lower Terrain",
            "Spread Terrain",
            "De-zone"
        ]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Emergency)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec![
            "Dispatch Police",
            "Dispatch Firefighters",
            "Dispatch Military"
        ]
    );
    assert_eq!(
        tool_menu_items(ToolMenu::Rewards)
            .iter()
            .map(|item| item.label)
            .collect::<Vec<_>>(),
        vec![
            "Mayor's House",
            "City Hall",
            "Statue",
            "Braun Llama Dome",
            "Arcologies"
        ]
    );
}
