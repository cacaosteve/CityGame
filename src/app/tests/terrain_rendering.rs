use super::*;

#[test]
fn mac_zone_seven_event_five_bounds_origin_walk_at_map_edge() {
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-seven-f9-edge.sc2", "ZoneSevenF9Edge")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    for y in 6..=8 {
        city.set_zone(0, y, 7);
    }
    city.set_mac_state_misc_u32_at(915 * 4, 5);
    let mut rng = MacToolboxRng::new(0x5678);
    let seed_before = rng.seed();

    assert_eq!(
        mac_budget_process_zone_seven_special_growth(&mut city, 0, 8, &mut rng),
        MacSpecialTileAttempt::Refused
    );
    assert_eq!(rng.seed(), seed_before);
    assert_eq!(city.building_at(0, 8), 0);
    assert_eq!(city.mac_zone7_census_count(15), 0);
}

#[test]
fn mac_special_zone_square_selector_matrix_preserves_zone_specific_map_effects() {
    let cases = [
        (7, 0xef),
        (7, 0xf1),
        (7, 0xf2),
        (7, 0xf6),
        (8, 0xee),
        (8, 0xf6),
        (9, 0xf0),
        (9, 0xf2),
    ];
    let markers = [
        [0x10, 0x20, 0x40, 0x80],
        [0x20, 0x40, 0x80, 0x10],
        [0x40, 0x80, 0x10, 0x20],
        [0x80, 0x10, 0x20, 0x40],
    ];

    for (case_index, (zone, tile)) in cases.into_iter().enumerate() {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-special-square-selector-matrix.sc2",
            "SquareSelectorMatrix",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        let rotation = (case_index & 3) as u8;
        city.saved_rotation = rotation;
        city.set_view_rotation((rotation + 1) & 3);
        for (x, y) in [(40, 40), (41, 40), (40, 41), (41, 41)] {
            city.set_zone(x, y, zone as u8);
            city.set_tile_bits(x, y, TileBits::new(0x1b));
        }
        if zone != 7 {
            // Powered adjacency is checked at the raw scan coordinate (41,41).
            city.set_tile_bits(42, 41, TileBits::new(0x40));
        }

        assert_eq!(
            mac_budget_try_place_zone_square(&mut city, 41, 41, zone, tile),
            MacSpecialTileAttempt::Placed,
            "zone {zone}, XBLD {tile:#04x}"
        );
        for (corner, (x, y)) in [(40, 40), (41, 40), (40, 41), (41, 41)]
            .into_iter()
            .enumerate()
        {
            let marker_index = [0, 1, 3, 2][corner];
            assert_eq!(city.building_at(x, y), tile);
            assert_eq!(
                city.zone_at(x, y).unwrap().raw_bits(),
                markers[usize::from(rotation)][marker_index] | zone as u8,
                "zone {zone}, XBLD {tile:#04x}, corner {corner}"
            );
            assert_eq!(
                city.tile_bits_at(x, y).unwrap().raw_bits(),
                if zone == 7 { 0x0b } else { 0xfb },
                "zone {zone}, XBLD {tile:#04x}, corner {corner}"
            );
        }
        if zone == 7 {
            let class = match tile {
                0xef => 3,
                0xf1 => 9,
                0xf2 => 4,
                0xf6 => 14,
                _ => unreachable!("zone-7 square selector tile"),
            };
            assert_eq!(city.mac_zone7_census_count(class), 4);
        } else {
            assert_eq!(city.mac_xbld_tile_count(tile), 4);
        }
    }
}

#[test]
fn display_layers_start_checked_and_track_toggles() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert_eq!(
        state.checked_palette_actions(),
        vec![
            UiAction::ToggleBuildings,
            UiAction::ToggleSigns,
            UiAction::ToggleInfrastructure,
            UiAction::ToggleZones,
        ]
    );

    state.sprite_cache.key = Some(state.sprite_cache_key((640, 480), 1.0));
    state.toggle_display_layer(UiAction::ToggleBuildings);
    assert!(!state.show_buildings);
    assert!(state.sprite_cache.key.is_none());
    assert!(
        !state
            .checked_palette_actions()
            .contains(&UiAction::ToggleBuildings)
    );

    state.sprite_cache.key = Some(state.sprite_cache_key((640, 480), 1.0));
    state.toggle_display_layer(UiAction::ToggleSigns);
    assert!(!state.show_signs);
    assert!(state.sprite_cache.key.is_some());

    state.sprite_cache.key = Some(state.sprite_cache_key((640, 480), 1.0));
    state.toggle_display_layer(UiAction::ToggleInfrastructure);
    assert!(!state.show_infrastructure);
    assert!(state.sprite_cache.key.is_none());
    assert!(
        !state
            .checked_palette_actions()
            .contains(&UiAction::ToggleInfrastructure)
    );

    state.sprite_cache.key = Some(state.sprite_cache_key((640, 480), 1.0));
    state.toggle_display_layer(UiAction::ToggleZones);
    assert!(!state.show_zones);
    assert!(state.sprite_cache.key.is_none());
    assert!(
        !state
            .checked_palette_actions()
            .contains(&UiAction::ToggleZones)
    );

    state.show_traffic = true;
    state.sprite_cache.key = Some(state.sprite_cache_key((640, 480), 1.0));
    state.toggle_underground_view();
    assert!(state.show_underground);
    assert!(!state.show_traffic);
    assert!(state.sprite_cache.key.is_none());
    assert!(
        state
            .checked_palette_actions()
            .contains(&UiAction::ToggleUnderground)
    );

    state.sprite_cache.key = Some(state.sprite_cache_key((640, 480), 1.0));
    state.toggle_underground_view();
    assert!(!state.show_underground);
    assert!(state.sprite_cache.key.is_none());
    assert!(
        !state
            .checked_palette_actions()
            .contains(&UiAction::ToggleUnderground)
    );
}

#[test]
fn infrastructure_layer_classifies_all_surface_network_tiles() {
    for building in [14, 29, 44, 67, 69, 71, 73, 92, 93, 108] {
        assert!(
            is_surface_infrastructure_tile(building),
            "building {building} should be infrastructure"
        );
    }
    for building in [0, 6, 112, 200, 255] {
        assert!(
            !is_surface_infrastructure_tile(building),
            "building {building} should remain outside infrastructure"
        );
    }
}

#[test]
fn loading_city_refreshes_pipe_and_water_bits() {
    let path = paths::default_city_path();
    if !path.exists() {
        return;
    }

    let loaded = City::load(&path).expect("load city");
    let mut expected_state = test_state(loaded);
    expected_state.refresh_simulation_service_bits();
    let expected_bits = tile_bits_snapshot(&expected_state.city);
    if expected_bits.iter().flatten().all(|bits| *bits == 0) {
        return;
    }

    let startup_state = ViewerState::load_default().expect("load default state");
    assert_eq!(tile_bits_snapshot(&startup_state.city), expected_bits);

    let mut state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));
    state
        .load_city(&path, (800, 600), 1.0)
        .expect("load city into state");
    assert_eq!(tile_bits_snapshot(&state.city), expected_bits);
}

#[test]
fn loaded_city_underground_view_keeps_pipe_sources_and_water_distinct() {
    let Some(path) = paths::test_win_populated_city().or_else(paths::test_mac_city) else {
        return;
    };
    let mut state = test_state(City::load(&path).expect("load populated city"));
    state.refresh_simulation_service_bits();
    let city = &state.city;
    let stats = underground_render_stats(city);

    assert!(stats.user_pipe > 0, "expected user-laid pipes in {path:?}");
    assert!(
        stats.watered_user_pipe > 0,
        "expected watered user-laid pipes in {path:?}"
    );
    assert!(
        stats.building_pipe > 0,
        "expected generated or saved building pipes in {path:?}"
    );
    assert!(
        stats.watered_building_pipe > 0,
        "expected watered building pipes in {path:?}"
    );
}

#[test]
fn generated_building_plumbing_renders_for_all_eligible_blank_xund_tiles() {
    let Some(path) = paths::test_win_populated_city().or_else(paths::test_mac_city) else {
        return;
    };
    let mut state = test_state(City::load(&path).expect("load populated city"));
    state.refresh_simulation_service_bits();
    let city = &state.city;
    let mut eligible_blank_tiles = 0usize;
    let mut rendered_building_tiles = 0usize;

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city.underground_at(x, y) == 0 && building_internal_pipe_eligible(city, x, y) {
                eligible_blank_tiles += 1;
                assert_eq!(
                    underground_render_tile(city, x, y, city.rotation()).map(|tile| tile.source),
                    Some(UndergroundTileSource::BuildingInternal),
                    "missing generated building pipe at ({x},{y}) in {path:?}"
                );
            }
            if underground_render_tile(city, x, y, city.rotation())
                .is_some_and(|tile| tile.source == UndergroundTileSource::BuildingInternal)
            {
                rendered_building_tiles += 1;
            }
        }
    }

    assert!(
        eligible_blank_tiles > 0,
        "expected generated building plumbing in {path:?}"
    );
    assert!(
        rendered_building_tiles >= eligible_blank_tiles,
        "expected saved building pipes plus generated plumbing"
    );
}

#[test]
fn saved_underground_pipe_tiles_are_water_network_tiles() {
    let files = available_city_files();
    if files.is_empty() {
        return;
    }

    let mut saved_pipe_tiles = 0usize;
    let mut bad = Vec::new();
    for path in files {
        let Ok(city) = City::load(&path) else {
            continue;
        };
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                if !saved_underground_water_pipe_tile(&city, x, y) {
                    continue;
                }
                saved_pipe_tiles += 1;
                if !water_pipe_network_tile(&city, x, y) && bad.len() < 16 {
                    let tile = city.underground_at(x, y);
                    bad.push(format!("{} ({x},{y}) XUND={tile}", path.display()));
                }
            }
        }
    }

    assert!(saved_pipe_tiles > 0, "expected saved XUND pipe tiles");
    assert!(
        bad.is_empty(),
        "saved XUND pipe tiles outside water network: {bad:?}"
    );
}

#[test]
fn maps_xter_to_terrain_and_water_sprite_ids() {
    assert_eq!(
        terrain_sprites(0x00),
        TerrainSprites {
            terrain: Some(1256),
            water: None
        }
    );
    assert_eq!(terrain_sprites(0x0d).terrain, Some(1269));
    assert_eq!(terrain_sprites(0x15).terrain, None);
    assert_eq!(terrain_sprites(0x15).water, Some(1270));
    assert_eq!(terrain_sprites(0x20).terrain, Some(1256));
    assert_eq!(terrain_sprites(0x20).water, Some(1270));
    assert_eq!(terrain_sprites(0x2d).water, Some(1283));
    assert_eq!(
        terrain_sprites(0x30),
        TerrainSprites {
            terrain: None,
            water: Some(1270)
        }
    );
    assert_eq!(terrain_sprites(0x42).terrain, None);
    assert_eq!(terrain_sprites(0x41).water, Some(1286));
}

#[test]
fn bay_view_surface_water_tiles_skip_land_terrain() {
    let Some(bayview) = paths::test_win_populated_city() else {
        return;
    };
    let city = crate::city::City::load(&bayview).expect("load bay view");
    let mut surface = 0;
    let mut bay = 0;
    let mut underwater = 0;
    for y in 0..crate::city::CITY_HEIGHT {
        for x in 0..crate::city::CITY_WIDTH {
            let code = city.terrain_at(x, y);
            if (0x10..=0x1d).contains(&code) {
                underwater += 1;
                assert_eq!(
                    terrain_sprites(code).terrain,
                    None,
                    "underwater 0x{code:02x} at ({x},{y})"
                );
            } else if (0x30..=0x3d).contains(&code) {
                surface += 1;
                assert_eq!(
                    terrain_sprites(code).terrain,
                    None,
                    "surface water 0x{code:02x} at ({x},{y})"
                );
            } else if (0x40..=0x45).contains(&code) {
                bay += 1;
                assert_eq!(
                    terrain_sprites(code).terrain,
                    None,
                    "bay water 0x{code:02x} at ({x},{y})"
                );
            }
        }
    }
    assert!(
        underwater + surface + bay > 1000,
        "expected open water in Bay View, got under={underwater} surface={surface} bay={bay}"
    );
}

#[test]
fn bay_view_xter_distribution() {
    let Some(bayview) = paths::test_win_populated_city() else {
        return;
    };
    let city = crate::city::City::load(&bayview).expect("load bay view");
    let mut counts = [0u32; 256];
    for y in 0..crate::city::CITY_HEIGHT {
        for x in 0..crate::city::CITY_WIDTH {
            counts[city.terrain_at(x, y) as usize] += 1;
        }
    }
    let sum = |lo: u8, hi: u8| counts[lo as usize..=hi as usize].iter().sum::<u32>();
    eprintln!(
        "Bay View XTER: dry={} under={} shore={} surface={} bay={}",
        sum(0x00, 0x0f),
        sum(0x10, 0x1f),
        sum(0x20, 0x2f),
        sum(0x30, 0x3f),
        sum(0x40, 0x4f),
    );
    assert!(sum(0x10, 0x4f) > 100);
}

#[test]
fn projected_tile_anchor_matches_opensc2k_top_left_half_scale() {
    let anchor = projected_tile_units(0, 0, 0.0);
    assert_eq!(anchor.x, 16.0);
    assert_eq!(anchor.y, -28.0);
    // OpenSC2K topLeft at (0,0,0) is (0, -56); our map pixels are half-scale.
    assert_eq!(anchor.y, -56.0 / 2.0);
    let raised = projected_tile_units(0, 0, 1.0);
    assert_eq!(raised.y, anchor.y - 12.0);
}

#[test]
fn scales_altitude_to_original_layer_step() {
    assert_eq!(layer_height(2.0), 24.0);
}

#[test]
fn zoom_anchor_preserves_map_position() {
    let window_size = (1200, 800);
    let anchor = Vec2::new(375.0, 260.0);
    let camera = Vec2::new(95.0, -40.0);
    let before = screen_to_map_units(anchor, window_size, camera, 2.0, 1.0);
    let new_camera = camera_for_map_units_at_screen(before, anchor, window_size, 3.0, 1.0);
    let after = screen_to_map_units(anchor, window_size, new_camera, 3.0, 1.0);

    assert!((before - after).length() < 0.001);
}

#[test]
fn zoom_anchor_preserves_map_position_with_retina_backing_scale() {
    let window_size = (2400, 1600);
    let anchor = Vec2::new(750.0, 520.0);
    let camera = Vec2::new(190.0, -80.0);
    let before = screen_to_map_units(anchor, window_size, camera, 2.0, 2.0);
    let new_camera = camera_for_map_units_at_screen(before, anchor, window_size, 3.0, 2.0);
    let after = screen_to_map_units(anchor, window_size, new_camera, 3.0, 2.0);

    assert!((before - after).length() < 0.001);
}

#[test]
fn terrain_cleanup_does_not_raise_a_loaded_maps_low_waterline_to_blank_map_height() {
    let mut city = City::new_blank("/tmp/low-waterline.sc2", "LowWaterline").unwrap();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_altitude(x, y, 4, 0);
        }
    }
    city.set_altitude(0, 0, 3, 4);
    city.set_terrain(0, 0, 0x10);
    city.set_mac_selected_water_height(4);
    let sea = pe_selected_water_height_for_fun_00447020(&city);
    assert_eq!(sea, 4);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 30, sea);
    assert_eq!(city.terrain_at(20, 30), 0);
    assert_eq!(city.altm_at(20, 30).water, 0);
    assert!(!city.tile_bits_at(20, 30).unwrap().water_tile());
    assert_eq!(test_state(city).edit_map_sea_level(), 4);
}

#[test]
fn charleston_terrain_cleanup_keeps_drum_island_dry_in_both_formats() {
    let mut fixtures = Vec::new();
    if let Some(root) = paths::test_win95_root() {
        fixtures.push(root.join("SCENARIO/CHARLEST.SCN"));
    }
    if let Some(path) = paths::test_city_from_env("SC2000_TEST_MAC_SCENARIO") {
        fixtures.push(path);
    }
    for path in fixtures {
        let mut city = City::load(&path).unwrap();
        let sea = pe_selected_water_height_for_fun_00447020(&city);
        assert_eq!(sea, 4, "{}", path.display());
        for x in 86..=87 {
            for y in 57..=62 {
                assert_eq!(city.altitude_at(x, y), 4);
                pe_fun_00447020_mode0_xund_slope_sea(&mut city, x, y, sea);
                assert!(city.terrain_at(x, y) < 0x10);
                assert_eq!(city.water_height_at(x, y), 0);
                assert!(!city.tile_bits_at(x, y).unwrap().water_tile());
            }
        }
    }
}

#[test]
fn wheel_zoom_uses_fractional_steps() {
    assert!((wheel_zoom_factor(1.0, MouseWheelDirection::Normal) - 1.25).abs() < 0.001);
    assert!((wheel_zoom_factor(-1.0, MouseWheelDirection::Normal) - (1.0 / 1.25)).abs() < 0.001);
    assert!(wheel_zoom_factor(0.5, MouseWheelDirection::Normal) > 1.0);
    assert!(wheel_zoom_factor(0.5, MouseWheelDirection::Normal) < 1.25);
}

#[test]
fn computes_retina_input_scale() {
    assert_eq!(scale_ratio(2000, 1000), 2.0);
    assert_eq!(scale_ratio(1000, 1000), 1.0);
    assert_eq!(scale_ratio(1000, 0), 1.0);
    assert_eq!(render_zoom(2.0, 2.0), 4.0);
}

#[test]
fn maps_xbld_to_sprite_ids() {
    assert_eq!(building_sprite_id(0x00), None);
    assert_eq!(building_sprite_id(0x1d), Some(1029));
    assert_eq!(building_sprite_id(0x2c), Some(1044));
    assert_eq!(building_sprite_id(0x8d), Some(1141));
    assert_eq!(building_sprite_id(0xff), Some(1255));
}

#[test]
fn classifies_xbld_anchors_and_layers() {
    let road = xbld_sprite_info(29, 0).expect("road");
    assert_eq!(road.sprite_id, 1029);
    assert_eq!(road.anchor, XbldAnchor::Tile);
    assert_eq!(road.layer_depth, DEPTH_ROAD);
    assert!(!road.corner_gated);

    let road_power = xbld_sprite_info(67, 0).expect("road power");
    assert_eq!(road_power.layer_depth, DEPTH_POWER);
    assert_eq!(xbld_base_layer_sprite_id(67, 0), Some(1029));

    let rail = xbld_sprite_info(44, 0).expect("rail");
    assert_eq!(rail.anchor, XbldAnchor::Tile);
    assert_eq!(rail.layer_depth, DEPTH_RAIL);

    let home = xbld_sprite_info(112, 0).expect("home");
    assert_eq!(home.anchor, XbldAnchor::Building);
    assert!(!home.corner_gated);

    let apartments = xbld_sprite_info(141, 0).expect("apartments");
    assert_eq!(apartments.anchor, XbldAnchor::Building);
    assert!(apartments.corner_gated);
    assert_eq!(xbld_tile_size(141), 2);
    assert_eq!(xbld_tile_size(191), 3);
    assert_eq!(xbld_tile_size(207), 4);

    for rotation in 0..4 {
        let coal = xbld_sprite_info(207, rotation).expect("coal power plant");
        assert_eq!(coal.sprite_id, 1207);
        assert_eq!(coal.anchor, XbldAnchor::Building);
        assert!(coal.corner_gated);
    }

    let church = xbld_sprite_info(247, 0).expect("church");
    assert_eq!(church.sprite_id, 1247);
    assert_eq!(church.anchor, XbldAnchor::Building);
    assert_eq!(church.layer_depth, DEPTH_BUILDING);
    assert!(church.corner_gated);
}

#[test]
fn rotates_infrastructure_sprite_ids() {
    let rotated = xbld_sprite_info(29, 1).expect("rotated road");
    assert_eq!(rotated.sprite_id, 1030);
}

#[test]
fn tile_sprite_index_is_x_major() {
    assert_eq!(tile_sprite_index(1, 0), 1);
    assert_eq!(tile_sprite_index(0, 1), CITY_WIDTH);
    assert_eq!(tile_sprite_index(3, 5), 3 + 5 * CITY_WIDTH);
}

#[test]
fn network_masks_map_to_sc2k_line_tile_ids() {
    assert_eq!(
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
        30
    );
    assert_eq!(
        network_building_for_mask(NetworkKind::Road, NETWORK_TOP | NETWORK_BOTTOM),
        29
    );
    assert_eq!(
        network_building_for_mask(NetworkKind::Road, NETWORK_TOP | NETWORK_RIGHT),
        35
    );
    assert_eq!(
        network_building_for_mask(NetworkKind::Road, NETWORK_RIGHT | NETWORK_BOTTOM),
        36
    );
    assert_eq!(
        network_building_for_mask(NetworkKind::Road, NETWORK_BOTTOM | NETWORK_LEFT),
        37
    );
    assert_eq!(
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_TOP),
        38
    );
    assert_eq!(
        network_building_for_mask(
            NetworkKind::Road,
            NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM,
        ),
        40
    );
    assert_eq!(
        network_building_for_mask(
            NetworkKind::Road,
            NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT,
        ),
        41
    );
    assert_eq!(
        network_building_for_mask(
            NetworkKind::Road,
            NETWORK_BOTTOM | NETWORK_LEFT | NETWORK_TOP,
        ),
        42
    );
    assert_eq!(
        network_building_for_mask(
            NetworkKind::Road,
            NETWORK_LEFT | NETWORK_TOP | NETWORK_RIGHT,
        ),
        39
    );
    assert_eq!(
        network_building_for_mask(
            NetworkKind::Power,
            NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT,
        ),
        28
    );
}

#[test]
fn sub_to_rail_clears_piped_keeps_watered_and_xtrf_without_invent_refresh() {
    // PE subway/sub-rail arm @ 0x4408dd: AND XBIT 0xDF (clear piped), leave watered/XTRF.
    let mut city =
        City::new_blank("/tmp/sc2000-subrail-watered.sc2", "SubRailWatered").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x44);
    city.set_building(21, 20, 0x2c); // PE rail band for adjacency @ `0x441be8`
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    assert!(state.place_sub_to_rail_at(20, 20));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(!bits.piped(), "PE clears piped on sub-rail access");
    assert!(bits.watered(), "invent water refresh must not wipe watered");
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x44));
}

#[test]
fn highway_drag_places_reinforced_bridge_segments_over_full_water_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..=11 {
        for x in 12..=13 {
            city.set_terrain(x, y, 0x10);
        }
    }
    let mut state = test_state(city);
    let path = highway_drag_path((10, 10), (14, 10));

    let placement = state.highway_drag_placement_for_path(&path);
    assert_eq!(placement.valid_tiles.len(), 3 * HIGHWAY_TILES_PER_SEGMENT);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(state.place_highway_path(&path), 3);

    for y in 10..=11 {
        for x in 10..=11 {
            assert_eq!(
                state.city.building_at(x, y),
                74,
                "land highway at ({x},{y})"
            );
        }
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                107,
                "highway bridge at ({x},{y})"
            );
        }
        for x in 14..=15 {
            assert_eq!(
                state.city.building_at(x, y),
                74,
                "land highway at ({x},{y})"
            );
        }
    }

    state.refresh_highway_segment_at_origin((12, 10));
    for y in 10..=11 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                107,
                "refreshed highway bridge at ({x},{y})"
            );
        }
    }
}

#[test]
fn pe_highway_water_worker_uses_native_xter_weights_and_saved_xbit_admission() {
    let mut city = City::new_blank("/tmp/sc2000-pe-highway-water-mode1.sc2", "WaterMode1")
        .expect("blank city");

    // The PE combines class values in coordinate order (x,y+1),
    // (x+1,y+1), (x+1,y), (x,y), with weights 8,4,2,1.
    for (tile, expected) in [((10, 11), 8), ((11, 11), 4), ((11, 10), 2), ((10, 10), 1)] {
        for y in 10..=11 {
            for x in 10..=11 {
                city.set_terrain(x, y, 0x46); // XTER lookup class 0
            }
        }
        city.set_terrain(tile.0, tile.1, 0x21); // XTER lookup class 1
        assert_eq!(
            pe_highway_water_footprint_class_45c1e0(&city, (10, 10)),
            expected
        );
    }

    // Pattern 2 selects direction 2 at compass 0. XBIT bit 0x04, rather than
    // the XTER color alone, is the native dispatch predicate.
    for y in 10..=11 {
        for x in 10..=11 {
            city.set_terrain(x, y, 0x46);
        }
    }
    city.set_terrain(11, 10, 0x21);
    city.set_tile_bits(11, 10, TileBits::new(0).with_water_tile(true));
    for y in 12..=15 {
        for x in 10..=11 {
            city.set_terrain(x, y, 0x10);
            city.set_tile_bits(x, y, TileBits::new(0).with_water_tile(true));
        }
    }

    let plan = pe_highway_route_path_461ae0(&city, (10, 10), (10, 18))
        .expect("saved-XBIT water start dispatches to mode 1");
    assert!(plan.path.is_empty());
    assert_eq!(
        plan.water_bridge,
        Some(HighwayWaterBridgeCandidate {
            seed: (10, 10),
            direction: 2,
            span_count: 3,
            selector_mask: 0x20,
            mode: 1,
        })
    );

    let mut xter_only = city.clone();
    xter_only.set_tile_bits(11, 10, TileBits::new(0));
    for y in 12..=15 {
        for x in 10..=11 {
            xter_only.set_tile_bits(x, y, TileBits::new(0));
        }
    }
    assert!(pe_highway_water_bridge_candidate_45c1e0(&xter_only, (10, 10), 1, 0).is_none());
    let ordinary_route = pe_highway_route_path_461ae0(&xter_only, (10, 10), (10, 18));
    assert!(ordinary_route.is_none_or(|plan| plan.water_bridge.is_none()));
}

#[test]
fn mac_highway_water_chooser_preserves_treasury_and_northeast_rotation() {
    let mut city = City::new_blank("/tmp/citygame-mac-highway-water.sc2", "Water").unwrap();
    city.set_city_funds(899);
    city.set_view_rotation(1);
    for x in 38..48 {
        for y in 28..30 {
            let water = (41..=44).contains(&x);
            city.set_altitude(x, y, if water { 8 } else { 9 }, 9);
            city.set_terrain(x, y, if water { 0x3e } else { 0 });
            city.set_tile_bits(x, y, TileBits::new(0).with_water_tile(water));
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Highway;
    state.begin_highway_drag(44, 28);
    assert_eq!(state.release_left_mouse_drag(false).highway_segments, 0);
    let Some(CityInfoDialog::HighwayWaterBridge { candidate }) = state.city_info_dialog else {
        panic!("Mac highway water chooser");
    };
    assert_eq!(candidate.selector_mask, 0x60);
    assert_eq!(state.finish_highway_water_bridge(candidate, 6), 0);
    assert_eq!(state.city.building_at(44, 28), 0);

    state.city.set_city_funds(900);
    assert_eq!(state.finish_highway_water_bridge(candidate, 6), 3);
    assert_eq!(state.city.city_funds(), 900);
    assert_eq!(state.city.building_at(44, 28), 0x6b);
    assert!(state.city.tile_bits_at(44, 28).unwrap().rotate());
    assert!(!state.city.tile_bits_at(45, 28).unwrap().rotate());
    assert!(state.city.tile_bits_at(45, 29).unwrap().rotate());
}

#[test]
fn pe_highway_water_selector_transaction_charges_and_stamps_native_choices() {
    fn water_city(span_count: usize) -> City {
        let mut city = City::new_blank("/tmp/sc2000-pe-highway-water-choice.sc2", "WaterChoice")
            .expect("blank city");
        city.set_city_funds(10_000);
        // The selector-6 option requires both dry endpoint footprints to be
        // no lower than the water seed. Blank-map ALTM starts at 8 and the
        // seed's nonzero XTER raises its PE highway altitude to 9.
        for (x0, y0) in [(8, 10), (10 + span_count * 2, 10)] {
            for y in y0..=y0 + 1 {
                for x in x0..=x0 + 1 {
                    city.set_altitude(x, y, 9, 0);
                }
            }
        }
        for segment in 0..span_count {
            let x0 = 10 + segment * 2;
            for y in 10..=11 {
                for x in x0..=x0 + 1 {
                    city.set_terrain(x, y, 0x10);
                    city.set_tile_bits(x, y, TileBits::new(0).with_water_tile(true));
                }
            }
            // XTRF is 64×64, so a single saved byte covers each 2×2 tile group.
            city.set_traffic_density(x0, 10, 0x80 | x0 as u8);
        }
        city
    }

    let city = water_city(2);
    let candidate = pe_highway_water_bridge_candidate_45c1e0(&city, (10, 10), 0, 1)
        .expect("mode-0 two-segment water span");
    assert_eq!(candidate.span_count, 2);
    assert_eq!(candidate.selector_mask, 0x20);
    assert_eq!(
        pe_highway_water_selectors_45c1e0(candidate.selector_mask),
        vec![5]
    );

    let mut state = test_state(city);
    state.begin_highway_water_bridge(candidate);
    assert_eq!(state.finish_highway_water_bridge(candidate, 5), 2);
    assert_eq!(state.city.city_funds(), 9_600);
    for x0 in [10, 12] {
        for y in 10..=11 {
            for x in x0..=x0 + 1 {
                assert_eq!(state.city.building_at(x, y), 0x4a);
                assert_eq!(state.city.zone_at(x, y).unwrap().raw_bits() & 0xf0, 0xf0);
                assert_eq!(state.city.traffic_density_at(x, y), Some(0x80 | x0 as u8));
            }
        }
    }

    let city = water_city(3);
    let candidate = pe_highway_water_bridge_candidate_45c1e0(&city, (10, 10), 0, 1)
        .expect("mode-0 three-segment water span");
    assert_eq!(candidate.span_count, 3);
    assert_eq!(candidate.selector_mask, 0x60);
    assert_eq!(
        pe_highway_water_selectors_45c1e0(candidate.selector_mask),
        vec![5, 6]
    );
    assert_eq!(pe_highway_water_selector_cost_45c1e0(5), Some(200));
    assert_eq!(pe_highway_water_selector_cost_45c1e0(6), Some(300));

    let mut state = test_state(city);
    state.begin_highway_water_bridge(candidate);
    assert!(matches!(
        state.city_info_dialog,
        Some(CityInfoDialog::HighwayWaterBridge { candidate: dialog_candidate })
            if dialog_candidate == candidate
    ));
    let dialog = state.city_info_dialog.expect("highway chooser");
    let rect = city_info_dialog_rect_for(dialog, (1000, 700));
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(
                road_bridge_choice_rect(rect, 1).x + 4.0,
                road_bridge_choice_rect(rect, 1).y + 4.0,
            ),
            (1000, 700),
            dialog,
        ),
        Some(CityInfoDialogAction::SelectHighwayWaterBridge(6))
    );
    assert_eq!(state.finish_highway_water_bridge(candidate, 6), 3);
    assert_eq!(state.city.city_funds(), 9_100);
    for (segment, expected_building) in [(0, 0x6b), (1, 0x6a), (2, 0x6b)] {
        let x0 = 10 + segment * 2;
        for y in 10..=11 {
            for x in x0..=x0 + 1 {
                assert_eq!(state.city.building_at(x, y), expected_building);
                let bits = state.city.tile_bits_at(x, y).expect("bridge XBIT");
                assert!(bits.water_tile());
                assert!(bits.rotate());
                assert_eq!(state.city.traffic_density_at(x, y), Some(0x80 | x0 as u8));
            }
        }
    }
}

// Partial PE `GetHighway` scan notes: retain the view delta and 2×2 XBIT probe
// as test-only evidence until a live caller supplies the native return path.
const PE_VIEW_WATER_SCAN_DX_4E8940: [i16; 4] = [0, 1, 0, -1];
const PE_VIEW_WATER_SCAN_DY_4E8948: [i16; 4] = [-1, 0, 1, 0];

fn pe_view_water_scan_tile_delta_4623cf(view: u16) -> (i16, i16) {
    let idx = (view as usize) & 3;
    (
        PE_VIEW_WATER_SCAN_DX_4E8940[idx].saturating_mul(2),
        PE_VIEW_WATER_SCAN_DY_4E8948[idx].saturating_mul(2),
    )
}

fn pe_view_water_scan_refuses_4623cf(city: &City, x: usize, y: usize, view: u16) -> bool {
    let (dx, dy) = pe_view_water_scan_tile_delta_4623cf(view);
    let ox = x as i16 + dx;
    let oy = y as i16 + dy;
    for (tx, ty) in [(ox, oy), (ox + 1, oy), (ox, oy + 1), (ox + 1, oy + 1)] {
        if !(0..0x80).contains(&tx) || !(0..0x80).contains(&ty) {
            continue;
        }
        if city
            .tile_bits_at(tx as usize, ty as usize)
            .is_some_and(TileBits::water_tile)
        {
            return true;
        }
    }
    false
}

#[test]
fn pe_view_water_scan_tile_delta_matches_4e8940_tables() {
    // PE @ `0x4623cf`: `[0x4c7d60]*2` indexes `0x4e8940`/`0x4e8948`, then `lea` ×2.
    assert_eq!(pe_view_water_scan_tile_delta_4623cf(0), (0, -2));
    assert_eq!(pe_view_water_scan_tile_delta_4623cf(1), (2, 0));
    assert_eq!(pe_view_water_scan_tile_delta_4623cf(2), (0, 2));
    assert_eq!(pe_view_water_scan_tile_delta_4623cf(3), (-2, 0));
    assert_eq!(pe_view_water_scan_tile_delta_4623cf(4), (0, -2)); // &3
}

#[test]
fn pe_view_water_scan_refuses_2x2_water_at_view_delta() {
    // PE @ `0x4623e8`+: view 0 → origin (20,20) probes 2×2 at (20,18).
    let mut city = City::new_blank("/tmp/sc2000-view-water.sc2", "ViewWater").expect("blank");
    assert!(!pe_view_water_scan_refuses_4623cf(&city, 20, 20, 0));
    let bits = city
        .tile_bits_at(20, 18)
        .unwrap_or_default()
        .with_water_tile(true);
    city.set_tile_bits(20, 18, bits);
    assert!(pe_view_water_scan_refuses_4623cf(&city, 20, 20, 0));
    // View 1 → (22,20); water at (20,18) must not refuse.
    assert!(!pe_view_water_scan_refuses_4623cf(&city, 20, 20, 1));
}

#[test]
fn pe_highway_stopped_route_preserves_native_reject_stamp_and_water_lookahead() {
    let mut blocked_start =
        City::new_blank("/tmp/sc2000-hwy-start-reject.sc2", "HwyStartReject").expect("blank");
    blocked_start.set_city_funds(50_000);
    blocked_start.set_building(21, 20, 5);
    assert_eq!(
        pe_highway_route_gate_low_word_464040(&blocked_start, 20, 20),
        0x8000
    );
    let mut state = test_state(blocked_start);
    state.engine_fidelity = EngineFidelity::Win95;
    state.tool = EditTool::Highway;
    assert!(
        state
            .highway_drag_plan_for_city((20, 20), (24, 20))
            .path
            .is_empty()
    );
    state.begin_highway_drag(20, 20);
    state.update_highway_drag_to((24, 20));
    let funds_before = state.city.city_funds();
    assert_eq!(state.finish_highway_drag(), (0, None));
    assert_eq!(state.city.city_funds(), funds_before);
    assert_eq!(state.city.building_at(21, 20), 5);

    let mut stopped = City::new_blank("/tmp/sc2000-hwy-stopped.sc2", "HwyStopped").expect("blank");
    stopped.set_city_funds(50_000);
    stopped.set_traffic_density(20, 20, 0x5a);
    stopped.set_building(20, 22, 5);
    let mut state = test_state(stopped);
    state.engine_fidelity = EngineFidelity::Win95;
    state.tool = EditTool::Highway;
    let plan = state.highway_drag_plan_for_city((20, 20), (20, 26));
    assert_eq!(plan.path, vec![(20, 20)]);
    assert_eq!(plan.neighbor_link, None);
    assert_eq!(plan.water_bridge, None);
    state.begin_highway_drag(20, 20);
    state.update_highway_drag_to((20, 26));
    assert_eq!(state.finish_highway_drag(), (1, None));
    assert_eq!(state.city.city_funds(), 49_900);
    for &(x, y) in &highway_segment_tiles((20, 20), NetworkAxis::GridY) {
        assert_eq!(state.city.building_at(x, y), 0x49);
    }
    assert_eq!(state.city.building_at(20, 22), 5);
    assert_eq!(state.city.traffic_density_at(21, 21), Some(0x5a));

    let mut stopped_at_water =
        City::new_blank("/tmp/sc2000-hwy-stopped-water.sc2", "HwyStoppedWater").expect("blank");
    for y in 22..=23 {
        for x in 20..=21 {
            stopped_at_water.set_terrain(x, y, 0x21);
            stopped_at_water.set_tile_bits(x, y, TileBits::new(0).with_water_tile(true));
        }
    }
    for y in 24..=25 {
        for x in 20..=21 {
            stopped_at_water.set_terrain(x, y, 0x46);
        }
    }
    stopped_at_water.set_building(20, 22, 5);
    let plan = pe_highway_route_path_461ae0(&stopped_at_water, (20, 20), (20, 26))
        .expect("ordinary caller still performs the final water lookahead");
    assert_eq!(plan.path, vec![(20, 20)]);
    assert_eq!(plan.neighbor_link, None);
    assert_eq!(
        plan.water_bridge,
        Some(HighwayWaterBridgeCandidate {
            seed: (20, 22),
            direction: 2,
            span_count: 1,
            selector_mask: 0x20,
            mode: 0,
        })
    );

    let mut invalid_water_start = City::new_blank(
        "/tmp/sc2000-hwy-invalid-water-start.sc2",
        "HwyInvalidWaterStart",
    )
    .expect("blank");
    invalid_water_start.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    let plan = pe_highway_route_path_461ae0(&invalid_water_start, (20, 20), (24, 20))
        .expect("XBIT water dispatch is terminal even if the worker refuses");
    assert!(plan.path.is_empty());
    assert_eq!(plan.water_bridge, None);
}

#[test]
fn highway_get_tile_north_altitude_returns_5_or_7_like_462a0f() {
    // Higher north neighbor and class != 5 → mask `0x11` → class 5.
    // Lower north neighbor → mask `0x40` → class 7.
    let mut city = City::new_blank("/tmp/sc2000-hwy-alt.sc2", "HwyAlt").expect("blank");
    city.set_building(20, 18, 0x4b);
    city.set_zone(20, 18, 0xf0);
    for &(x, y) in &[(20, 18), (21, 18), (20, 19), (21, 19)] {
        city.set_altitude(x, y, 12, 0);
    }
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        5
    );
    for &(x, y) in &[(20, 18), (21, 18), (20, 19), (21, 19)] {
        city.set_altitude(x, y, 2, 0);
    }
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        7
    );
}

#[test]
fn highway_get_tile_east_altitude_returns_6_or_4_like_462a90() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-ealt.sc2", "HwyEalt").expect("blank");
    city.set_building(22, 20, 0x4c);
    city.set_zone(22, 20, 0xf0);
    for &(x, y) in &[(22, 20), (23, 20), (22, 21), (23, 21)] {
        city.set_altitude(x, y, 12, 0);
    }
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        6
    );
    for &(x, y) in &[(22, 20), (23, 20), (22, 21), (23, 21)] {
        city.set_altitude(x, y, 2, 0);
    }
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        4
    );
}

#[test]
fn highway_get_tile_south_west_altitude_like_462b04() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-swalt.sc2", "HwySwAlt").expect("blank");
    city.set_building(20, 22, 0x4b);
    city.set_zone(20, 22, 0xf0);
    for &(x, y) in &[(20, 22), (21, 22), (20, 23), (21, 23)] {
        city.set_altitude(x, y, 12, 0);
    }
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        7
    );
    for &(x, y) in &[(20, 22), (21, 22), (20, 23), (21, 23)] {
        city.set_altitude(x, y, 8, 0);
    }
    city.set_building(18, 20, 0x4c);
    city.set_zone(18, 20, 0xf0);
    for &(x, y) in &[(18, 20), (19, 20), (18, 21), (19, 21)] {
        city.set_altitude(x, y, 2, 0);
    }
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        6
    );
}

#[test]
fn highway_get_tile_xtxt_fa_forces_map_edge_bits_like_462ba6() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-fa.sc2", "HwyFa").expect("blank");
    city.set_text_overlay(0, 20, 0xfa);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 0, 20, NetworkAxis::GridY),
        3
    );
    city.set_text_overlay(20, 0, 0xfa);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 0, NetworkAxis::GridX),
        2
    );
    city.set_text_overlay(40, 40, 0xfa);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 40, 40, NetworkAxis::GridX),
        3
    );
}

#[test]
fn highway_place_class_8_stamps_0x65_and_sloped_xter_like_462f14() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-c8.sc2", "HwyC8").expect("blank");
    city.set_terrain(20, 20, 1);
    city.set_altitude(21, 21, 2, 0);
    city.set_altitude(21, 20, 12, 0);
    pe_place_highway_tile_462ed0_class_8_to_c(&mut city, 20, 20, 8);
    assert_eq!(city.building_at(20, 20), 0x65);
    assert_eq!(city.building_at(21, 20), 0x65);
    assert_eq!(city.building_at(20, 21), 0x65);
    assert_eq!(city.building_at(21, 21), 0x65);
    assert_eq!(city.terrain_at(20, 20), 0x0d);
    assert_eq!(city.terrain_at(21, 21), 0x0d);
    assert_eq!(city.terrain_at(21, 20), 0);
    assert_eq!(
        city.zone_at(20, 20).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0x10
    );
    assert_eq!(
        city.zone_at(21, 20).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0x20
    );
    assert_eq!(
        city.zone_at(21, 21).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0x40
    );
    assert_eq!(
        city.zone_at(20, 21).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0x80
    );
}

#[test]
fn highway_place_class_4_to_7_rewrites_mismatched_altm_like_463325() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-c47.sc2", "HwyC47").expect("blank");
    pe_place_highway_tile_462ed0_class_4_to_7(&mut city, 20, 20, 4);
    assert_eq!(
        city.altitude_at(20, 20),
        8,
        "matching land skips the rewrite"
    );
    assert_eq!(city.building_at(20, 20), 0x61);
    assert_eq!(city.terrain_at(20, 20), 0x0d);
    assert_eq!(city.terrain_at(21, 20), 1);
    assert_eq!(city.terrain_at(20, 21), 0x0d);
    assert_eq!(city.terrain_at(21, 21), 1);

    let mut city = City::new_blank("/tmp/sc2000-hwy-c47b.sc2", "HwyC47b").expect("blank");
    city.set_altitude(21, 21, 12, 0);
    city.set_altitude(20, 20, 8, 3);
    pe_place_highway_tile_462ed0_class_4_to_7(&mut city, 20, 20, 5);
    assert_eq!(city.altitude_at(20, 20), 11);
    assert_eq!(city.water_height_at(20, 20), 3);
    assert_eq!(city.altitude_at(21, 20), 11);
    assert_eq!(city.altitude_at(21, 21), 11);
    assert_eq!(city.altitude_at(20, 21), 11);
    assert_eq!(city.building_at(20, 20), 0x62);
    assert_eq!(city.terrain_at(20, 20), 0x0d);
    assert_eq!(city.terrain_at(21, 21), 2);

    let mut city = City::new_blank("/tmp/sc2000-hwy-c47c.sc2", "HwyC47c").expect("blank");
    pe_place_highway_tile_462ed0_class_4_to_7(&mut city, 30, 30, 6);
    assert_eq!(city.building_at(30, 30), 0x63);
    assert_eq!(city.terrain_at(30, 30), 3);
    assert_eq!(city.terrain_at(31, 30), 0x0d);
    pe_place_highway_tile_462ed0_class_4_to_7(&mut city, 40, 40, 7);
    assert_eq!(city.building_at(40, 40), 0x64);
    assert_eq!(city.terrain_at(40, 40), 4);
    assert_eq!(city.terrain_at(40, 41), 0x0d);
    assert_eq!(
        city.zone_at(40, 40).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0x10
    );
}

#[test]
fn highway_place_class_ge_0d_stamps_without_slope_xter_like_46315d() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-c0d.sc2", "HwyC0d").expect("blank");
    city.set_terrain(20, 20, 1);
    pe_place_highway_tile_462ed0_class_ge_0d(&mut city, 20, 20, 0x0d);
    assert_eq!(city.building_at(20, 20), 0x6a);
    assert_eq!(city.building_at(21, 21), 0x6a);
    assert_eq!(
        city.terrain_at(20, 20),
        1,
        "class >= 0x0d does not rewrite XTER"
    );
    assert_eq!(
        city.zone_at(20, 21).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0x80
    );
}

#[test]
fn highway_get_tile_slope_nw_below_se_gate_8000_like_464224() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-slope.sc2", "HwySlope").expect("blank");
    city.set_terrain(20, 20, 1);
    city.set_altitude(21, 21, 2, 0);
    assert!(pe_highway_gate_early_8000_4640a1(&city, 20, 20));
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2
    );
    city.set_altitude(21, 21, 8, 0);
    assert!(!pe_highway_gate_early_8000_4640a1(&city, 20, 20));
    assert_eq!(
        pe_highway_gate_si0_fallthrough_464310(&city, 20, 20),
        Some(3)
    );
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        5,
        "equal altitude with category-1 NW is gate mask 3, not orientation"
    );
    city.set_terrain(20, 20, 0);
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        3
    );
    city.set_terrain(21, 20, 1);
    city.set_altitude(20, 21, 2, 0);
    assert!(pe_highway_gate_early_8000_4640a1(&city, 20, 20));
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridX),
        2
    );
}

#[test]
fn highway_get_tile_equal_alt_nw_xter_0d_gate_4000_like_464304() {
    let mut city = City::new_blank("/tmp/sc2000-hwy-g4d.sc2", "HwyG4d").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 0x4a);
        city.set_zone(x, y, 0xf0);
    }
    city.set_terrain(20, 20, 0x0d);
    assert_eq!(pe_highway_classify_463c40(&city, 20, 20), 2);
    assert!(pe_highway_gate_returns_4000_4642e7(&city, 20, 20));
    assert_eq!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        -1
    );
    city.set_altitude(21, 21, 9, 0);
    assert!(!pe_highway_gate_returns_4000_4642e7(&city, 20, 20));
    assert_ne!(
        pe_get_highway_tile_462910(&city, 20, 20, NetworkAxis::GridY),
        -1
    );
}

#[test]
fn highway_water_bridge_requires_the_whole_two_by_two_segment_to_be_water() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(12, 10, 0x10);
    city.set_terrain(13, 10, 0x10);
    let state = test_state(city);
    let path = highway_drag_path((10, 10), (14, 10));

    let placement = state.highway_drag_placement_for_path(&path);
    assert_eq!(placement.valid_tiles.len(), HIGHWAY_TILES_PER_SEGMENT);
    assert!(placement.invalid_tiles.contains(&(12, 10)));
    assert!(placement.invalid_tiles.contains(&(13, 10)));
}

#[test]
fn highway_drag_stamps_slope_faces_through_citytool_like_4626d0() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..=11 {
        for x in 10..=11 {
            city.set_terrain(x, y, 0x02);
        }
    }
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_terrain(x, y, 0x01);
        }
    }
    let mut state = test_state(city);

    assert_eq!(
        state.place_highway_path(&highway_drag_path((10, 10), (10, 10))),
        1
    );
    for y in 10..=11 {
        for x in 10..=11 {
            assert_eq!(
                state.city.building_at(x, y),
                73,
                "PE class-2 underlay survives refresh ({x},{y})"
            );
            assert_eq!(
                state.city.terrain_at(x, y),
                0x0d,
                "low land becomes XTER 0x0d"
            );
        }
    }

    assert_eq!(
        state.place_highway_path(&highway_drag_path((20, 20), (20, 22))),
        2
    );
    for y in 20..=21 {
        for x in 20..=21 {
            assert_eq!(
                state.city.building_at(x, y),
                73,
                "NS refresh keeps the axis tile after stamp ({x},{y})"
            );
            assert_eq!(
                state.city.terrain_at(x, y),
                0x0d,
                "low land becomes XTER 0x0d"
            );
        }
    }
}

#[test]
fn highway_drag_slope_walk_accepts_paired_xter_faces() {
    // PE slope walk ends when the stepped tile is not higher than base and
    // requires terminal XTER `((start+1)&3)+1` (dialog `0x72` otherwise).
    // XTER 2 → dir north; terminal XTER 4 at (20,19) with equal altitude.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(20, 20, 2);
    city.set_terrain(20, 19, 4);
    let mut state = test_state(city);
    assert!(state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_terrain(20, 19, 0);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
}

#[test]
fn adjacent_highway_segments_refresh_into_connected_sprite() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(
        state.place_highway_path(&highway_drag_path((10, 10), (14, 10))),
        3
    );
    assert_eq!(
        state.place_highway_path(&highway_drag_path((12, 8), (12, 10))),
        2
    );

    for y in 10..=11 {
        for x in 12..=13 {
            assert_eq!(state.city.building_at(x, y), 105, "junction at ({x},{y})");
        }
    }
}

#[test]
fn highway_elbow_on_xter1_uses_non_0xf_class_3() {
    // XTER 1 on the elbow raises origin altitude, so the west approach's
    // ±2 neighbor arm reads `0x4e8970` class 3 (underlay 74, XTER 0x0d).
    // CityTool continues onto that highway (`0x4640b5`).
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert_eq!(
        state.place_highway_path(&highway_drag_path((12, 8), (12, 10))),
        2
    );
    for y in 10..=11 {
        for x in 12..=13 {
            state.city.set_terrain(x, y, 1);
        }
    }
    // PE CityTool has no AtCoordinates `1..=4` dialog, so the drag continues
    // onto the highway the neighbor arm already flattened.
    assert_eq!(
        state.place_highway_path(&highway_drag_path((10, 10), (12, 10))),
        2
    );
    for y in 10..=11 {
        for x in 12..=13 {
            assert_eq!(state.city.building_at(x, y), 74, "xter1 elbow at ({x},{y})");
            assert_eq!(
                state.city.terrain_at(x, y),
                0x0d,
                "class < 4 rewrites low land at ({x},{y})"
            );
        }
    }
}

#[test]
fn highway_xter_0x0d_crossing_uses_4e8970_not_105() {
    // Existing highway with stamp `XTER 0x0d` is gate `0x4000`, so the
    // perpendicular stamp reads `0x4e8970` and stays underlay 74, not 105.
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert_eq!(
        state.place_highway_path(&highway_drag_path((10, 10), (14, 10))),
        3
    );
    for y in 10..=11 {
        for x in 12..=13 {
            state.city.set_terrain(x, y, 0x0d);
        }
    }
    assert_eq!(
        state.place_highway_path(&highway_drag_path((12, 8), (12, 10))),
        2
    );
    for y in 10..=11 {
        for x in 12..=13 {
            assert_eq!(
                state.city.building_at(x, y),
                74,
                "0x0d crossing stays underlay 74 at ({x},{y})"
            );
            assert_eq!(
                state.city.terrain_at(x, y),
                0x0d,
                "slope terrain stays ({x},{y})"
            );
        }
    }
}

#[test]
fn placing_road_overwrites_trees() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(11, 10, 6);
    city.set_building(12, 10, 12);
    let mut state = test_state(city);
    let path = network_drag_path((10, 10), (13, 10));

    let placement = state.network_placement_for_path(&path, NetworkKind::Road);
    assert_eq!(placement.valid_tiles, path);
    assert!(placement.invalid_tiles.is_empty());

    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 4);
    for x in 10..=13 {
        assert_eq!(state.city.building_at(x, 10), 30);
    }
}

#[test]
fn placing_roads_uses_slope_tiles_that_match_the_terrain_axis() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(10, 10, 0x01);
    city.set_terrain(20, 20, 0x02);
    let mut state = test_state(city);

    assert_eq!(
        state.place_network_path(&[(10, 9), (10, 10), (10, 11)], NetworkKind::Road),
        3
    );
    assert_eq!(state.city.building_at(10, 9), 29);
    assert_eq!(state.city.building_at(10, 10), 31);
    assert_eq!(state.city.building_at(10, 11), 29);

    assert_eq!(
        state.place_network_path(&[(19, 20), (20, 20), (21, 20)], NetworkKind::Road),
        3
    );
    assert_eq!(state.city.building_at(19, 20), 30);
    assert_eq!(state.city.building_at(20, 20), 32);
    assert_eq!(state.city.building_at(21, 20), 30);
}

#[test]
fn win95_road_strokes_match_native_terrain_reshape_on_both_layouts() {
    for path in ["road-reshape.city", "road-reshape.sc2"] {
        for (dx, dy, slope, straight, sloped) in [(1, 0, 1, 0x1e, 0x1f), (0, 1, 2, 0x1d, 0x20)] {
            let cells = [(40, 40), (40 + dx, 40 + dy), (40 + 2 * dx, 40 + 2 * dy)];
            for (start, end) in [(cells[0], cells[2]), (cells[2], cells[0])] {
                for grade in [0, slope, 9] {
                    let mut city = blank_city_layout_fixture(path, "Road reshape");
                    city.set_city_funds(100);
                    city.set_terrain(cells[1].0, cells[1].1, grade);
                    for (x, y) in cells {
                        city.set_zone(x, y, 0xa2);
                        city.set_tile_bits(x, y, TileBits::new(0x20));
                        city.set_traffic_density(x, y, 0x63);
                    }
                    city.prepare_pe_fidelity_from_saved_city();
                    let mut state = test_state(city);
                    state.tool = EditTool::Road;
                    assert_eq!(state.place_win95_road_stroke_449bb0(start, end), (3, None));
                    assert_eq!(state.city.city_funds(), if grade == 9 { 45 } else { 70 });
                    for (index, (x, y)) in cells.into_iter().enumerate() {
                        let on_slope = index == 1 && grade != 0;
                        assert_eq!(
                            state.city.building_at(x, y),
                            if on_slope { sloped } else { straight }
                        );
                        assert_eq!(
                            state.city.terrain_at(x, y),
                            if on_slope { slope } else { 0 }
                        );
                        assert_eq!(state.city.zone_at(x, y).unwrap().raw_bits(), 0xa0);
                        assert_eq!(state.city.tile_bits_at(x, y).unwrap().raw_bits(), 0x20);
                        assert_eq!(state.city.traffic_density_at(x, y), Some(0x63));
                    }
                }
            }
        }

        for (terrain, expected_center) in [(0, 0x29), (1, 0x1e)] {
            let mut city = blank_city_layout_fixture(path, "Road crossover");
            city.set_city_funds(100);
            city.set_building(41, 41, 0x44);
            city.set_terrain(41, 41, terrain);
            city.set_tile_bits(41, 41, TileBits::new(0xa0));
            city.prepare_pe_fidelity_from_saved_city();
            let mut state = test_state(city);
            state.tool = EditTool::Road;
            assert_eq!(
                state.place_win95_road_stroke_449bb0((40, 40), (42, 40)),
                (3, None)
            );
            assert_eq!(state.city.building_at(41, 40), expected_center);
            assert_eq!(state.city.building_at(41, 41), 0x44);
            assert_eq!(state.city.tile_bits_at(41, 41).unwrap().raw_bits(), 0xa0);
        }
    }
}

#[test]
fn mac68k_placing_roads_uses_slope_tiles_that_match_the_terrain_axis() {
    // Shared PlaceRoads reshape: keep terrain-slope road art through refresh.
    let mut city =
        City::new_blank("/tmp/sc2000-road-slope-mac.sc2", "RoadSlopeMac").expect("blank");
    city.set_terrain(10, 10, 0x01);
    city.set_terrain(20, 20, 0x02);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_network_path(&[(10, 9), (10, 10), (10, 11)], NetworkKind::Road),
        3
    );
    assert_eq!(state.city.building_at(10, 10), 31);
    assert_eq!(
        state.place_network_path(&[(19, 20), (20, 20), (21, 20)], NetworkKind::Road),
        3
    );
    assert_eq!(state.city.building_at(20, 20), 32);
}

#[test]
fn placing_rails_uses_slope_tiles_that_match_the_terrain_axis() {
    // PlaceRails reshape must keep terrain-slope rail art through refresh.
    let mut city =
        City::new_blank("/tmp/sc2000-rail-slope-terrain.sc2", "RailSlopeTer").expect("blank");
    city.set_terrain(10, 10, 0x01);
    city.set_terrain(20, 20, 0x02);
    let mut state = test_state(city);
    assert_eq!(
        state.place_network_path(&[(10, 9), (10, 10), (10, 11)], NetworkKind::Rail),
        3
    );
    assert_eq!(state.city.building_at(10, 10), 46); // 0x2c+2
    assert_eq!(
        state.place_network_path(&[(19, 20), (20, 20), (21, 20)], NetworkKind::Rail),
        3
    );
    assert_eq!(state.city.building_at(20, 20), 47); // 0x2c+3
}

#[test]
fn win95_rail_strokes_match_native_grades_caps_and_crossings() {
    for path in ["rail-reshape.city", "rail-reshape.sc2"] {
        for (dx, dy, slope, straight, sloped, cap) in
            [(1, 0, 1, 0x2d, 0x2e, 0x3b), (0, 1, 2, 0x2c, 0x2f, 0x3c)]
        {
            let cells = [(40, 40), (40 + dx, 40 + dy), (40 + 2 * dx, 40 + 2 * dy)];
            for (start, end) in [(cells[0], cells[2]), (cells[2], cells[0])] {
                for grade in [0, slope, 9] {
                    let mut city = blank_city_layout_fixture(path, "Rail reshape");
                    city.set_city_funds(100);
                    city.set_terrain(cells[1].0, cells[1].1, grade);
                    for (x, y) in cells {
                        city.set_altitude(x, y, 5, 5);
                        city.set_zone(x, y, 0xa2);
                        city.set_tile_bits(x, y, TileBits::new(0x20));
                        city.set_traffic_density(x, y, 0x63);
                    }
                    city.prepare_pe_fidelity_from_saved_city();
                    let mut state = test_state(city);
                    state.tool = EditTool::Rail;
                    assert_eq!(
                        state.place_win95_rail_stroke_449f50(start, end),
                        Some((3, None))
                    );
                    assert_eq!(state.city.city_funds(), if grade == 9 { 0 } else { 25 });
                    let shapes = if grade == 0 {
                        [straight; 3]
                    } else {
                        [straight, sloped, cap]
                    };
                    for (index, (x, y)) in cells.into_iter().enumerate() {
                        assert_eq!(state.city.building_at(x, y), shapes[index]);
                        assert_eq!(
                            state.city.terrain_at(x, y),
                            if index == 1 && grade != 0 { slope } else { 0 }
                        );
                        assert_eq!(state.city.zone_at(x, y).unwrap().raw_bits(), 0xa0);
                        assert_eq!(state.city.tile_bits_at(x, y).unwrap().raw_bits(), 0x20);
                        assert_eq!(state.city.traffic_density_at(x, y), Some(0x63));
                    }
                }
            }
        }
        for (terrain, expected) in [(0, 0x38), (1, 0x2d)] {
            let mut city = blank_city_layout_fixture(path, "Rail crossing");
            city.set_city_funds(100);
            city.set_building(41, 41, 0x48);
            city.set_terrain(41, 41, terrain);
            city.prepare_pe_fidelity_from_saved_city();
            let mut state = test_state(city);
            state.tool = EditTool::Rail;
            assert_eq!(
                state.place_win95_rail_stroke_449f50((40, 40), (42, 40)),
                Some((3, None))
            );
            assert_eq!(state.city.building_at(41, 40), expected);
            assert_eq!(state.city.building_at(41, 41), 0x48);
        }
    }
}

#[test]
fn mac68k_placing_rails_uses_slope_tiles_that_match_the_terrain_axis() {
    let mut city =
        City::new_blank("/tmp/sc2000-rail-slope-mac.sc2", "RailSlopeMac").expect("blank");
    city.set_terrain(10, 10, 0x01);
    city.set_terrain(20, 20, 0x02);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_network_path(&[(10, 9), (10, 10), (10, 11)], NetworkKind::Rail),
        3
    );
    assert_eq!(state.city.building_at(10, 10), 46);
    assert_eq!(
        state.place_network_path(&[(19, 20), (20, 20), (21, 20)], NetworkKind::Rail),
        3
    );
    assert_eq!(state.city.building_at(20, 20), 47);
}

#[test]
fn win95_power_strokes_match_native_terrain_reshape_on_both_layouts() {
    // Hash-checked PE 0x44a310 replay after mouse release.
    for path in ["power-reshape.city", "power-reshape.sc2"] {
        for (dx, dy, slope, straight, sloped) in [(1, 0, 1, 0x0f, 0x10), (0, 1, 2, 0x0e, 0x11)] {
            let cells = [(40, 40), (40 + dx, 40 + dy), (40 + 2 * dx, 40 + 2 * dy)];
            for (start, end) in [(cells[0], cells[2]), (cells[2], cells[0])] {
                for grade in [0, slope, 9] {
                    let mut city = blank_city_layout_fixture(path, "Power reshape");
                    city.set_city_funds(100);
                    city.set_terrain(cells[1].0, cells[1].1, grade);
                    for (x, y) in cells {
                        city.set_tile_bits(x, y, TileBits::new(0x20));
                        city.set_traffic_density(x, y, 0x63);
                    }
                    city.prepare_pe_fidelity_from_saved_city();
                    let mut state = test_state(city);
                    state.tool = EditTool::Power;
                    assert_eq!(state.place_win95_power_stroke_44a310(start, end), 3);
                    assert_eq!(state.city.city_funds(), if grade == 9 { 69 } else { 94 });
                    for (index, (x, y)) in cells.into_iter().enumerate() {
                        let on_slope = index == 1 && grade != 0;
                        assert_eq!(
                            state.city.building_at(x, y),
                            if on_slope { sloped } else { straight }
                        );
                        assert_eq!(
                            state.city.terrain_at(x, y),
                            if on_slope { slope } else { 0 }
                        );
                        assert_eq!(state.city.tile_bits_at(x, y).unwrap().raw_bits(), 0xa0);
                        assert_eq!(state.city.traffic_density_at(x, y), Some(0x63));
                    }
                }
            }
        }

        for (site_slope, start_tile, funds, expected) in [
            (Some(0), 0, 100, [0x0f, 0x1a, 0x0f]),
            (Some(1), 0, 100, [0x0f, 0x0f, 0x0f]),
            (None, 0x1d, 100, [0x43, 0x0f, 0x0f]),
            (None, 0, 5, [0, 0, 0]),
        ] {
            let mut city = blank_city_layout_fixture(path, "Power reshape");
            city.set_city_funds(funds);
            city.set_building(40, 40, start_tile);
            if let Some(slope) = site_slope {
                city.set_building(41, 41, 0xdd);
                city.set_tile_bits(41, 41, TileBits::new(0x80));
                city.set_terrain(41, 41, slope);
            }
            for x in 40..=42 {
                city.set_tile_bits(x, 40, TileBits::new(0x20));
                city.set_traffic_density(x, 40, 0x63);
            }
            city.prepare_pe_fidelity_from_saved_city();
            let mut state = test_state(city);
            state.tool = EditTool::Power;
            let admitted = funds >= 6;
            assert_eq!(
                state.place_win95_power_stroke_44a310((40, 40), (42, 40)),
                if admitted { 3 } else { 0 }
            );
            assert_eq!(
                state.city.city_funds(),
                funds - if admitted { 6 } else { 0 }
            );
            for (x, tile) in (40..=42).zip(expected) {
                assert_eq!(state.city.building_at(x, 40), tile);
                assert_eq!(
                    state.city.tile_bits_at(x, 40).unwrap().raw_bits(),
                    if admitted { 0xa0 } else { 0x20 }
                );
                assert_eq!(state.city.traffic_density_at(x, 40), Some(0x63));
            }
            if let Some(slope) = site_slope {
                assert_eq!(state.city.building_at(41, 41), 0xdd);
                assert_eq!(state.city.terrain_at(41, 41), slope);
                assert_eq!(state.city.tile_bits_at(41, 41).unwrap().raw_bits(), 0x80);
            }
        }
    }
}

#[test]
fn win95_power_stroke_connects_both_axes_in_both_drag_directions_on_both_city_layouts() {
    for file_kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (start, end, expected_building, coordinates) in [
            (
                (40, 40),
                (46, 40),
                0x0f,
                (40..=46).map(|x| (x, 40)).collect::<Vec<_>>(),
            ),
            (
                (46, 40),
                (40, 40),
                0x0f,
                (40..=46).map(|x| (x, 40)).collect::<Vec<_>>(),
            ),
            (
                (40, 40),
                (40, 46),
                0x0e,
                (40..=46).map(|y| (40, y)).collect::<Vec<_>>(),
            ),
            (
                (40, 46),
                (40, 40),
                0x0e,
                (40..=46).map(|y| (40, y)).collect::<Vec<_>>(),
            ),
        ] {
            let mut city = City::new_blank("/tmp/pe-power-stroke-directions.sc2", "PowerStroke")
                .expect("blank city");
            city.kind = file_kind;
            city.set_city_funds(100_000);
            let mut state = test_state(city);
            state.tool = EditTool::Power;

            assert_eq!(
                state.place_win95_power_stroke_44a310(start, end),
                coordinates.len(),
                "{file_kind:?} {start:?}->{end:?}"
            );
            for (x, y) in coordinates {
                assert_eq!(
                    state.city.building_at(x, y),
                    expected_building,
                    "{file_kind:?} {start:?}->{end:?} at ({x},{y})"
                );
                assert!(
                    state.city.tile_bits_at(x, y).unwrap().powerable(),
                    "{file_kind:?} {start:?}->{end:?} at ({x},{y}) must carry PE XBIT powerable"
                );
            }
        }
    }
}

#[test]
fn mac68k_placing_power_uses_slope_tiles_that_match_the_terrain_axis() {
    // Native CODE 13 @ 0x23acc/0x23c36 uses the Mac XTER axis tables before
    // choosing the slope tile. The event-boundary replay covers these slopes
    // with the selected Power tool on both save layouts.
    let mut city =
        City::new_blank("/tmp/sc2000-power-slope-mac.sc2", "PwrSlopeMac").expect("blank");
    city.set_terrain(10, 10, 0x01);
    city.set_terrain(20, 20, 0x02);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_network_path(&[(10, 9), (10, 10), (10, 11)], NetworkKind::Power),
        3
    );
    assert_eq!(state.city.building_at(10, 10), 16);
    assert_eq!(
        state.place_network_path(&[(19, 20), (20, 20), (21, 20)], NetworkKind::Power),
        3
    );
    assert_eq!(state.city.building_at(20, 20), 17);
}

#[test]
fn network_drags_place_bridge_tiles_over_straight_water_crossings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(11, 10, 0x10);
    city.set_terrain(12, 10, 0x10);
    let mut state = test_state(city);
    let path = vec![(10, 10), (11, 10), (12, 10), (13, 10)];

    let placement = state.network_placement_for_path(&path, NetworkKind::Road);
    assert_eq!(placement.valid_tiles, path);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 4);
    assert_eq!(state.city.building_at(10, 10), 30);
    assert_eq!(state.city.building_at(11, 10), 87);
    assert_eq!(state.city.building_at(12, 10), 87);
    assert_eq!(state.city.building_at(13, 10), 30);

    state.refresh_network_tile(11, 10, NetworkKind::Road);
    state.refresh_network_tile(12, 10, NetworkKind::Road);
    assert_eq!(state.city.building_at(11, 10), 87);
    assert_eq!(state.city.building_at(12, 10), 87);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(11, 10, 0x10);
    city.set_terrain(12, 10, 0x10);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&path, NetworkKind::Rail), 4);
    assert_eq!(state.city.building_at(10, 10), 45);
    assert_eq!(state.city.building_at(11, 10), 91);
    assert_eq!(state.city.building_at(12, 10), 90);
    assert_eq!(state.city.building_at(13, 10), 45);

    state.refresh_network_tile(11, 10, NetworkKind::Rail);
    state.refresh_network_tile(12, 10, NetworkKind::Rail);
    assert_eq!(state.city.building_at(11, 10), 91);
    assert_eq!(state.city.building_at(12, 10), 90);

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(11, 10, 0x10);
    city.set_terrain(12, 10, 0x10);
    let mut state = test_state(city);
    let placement = state.network_drag_placement_for_path(&path, NetworkKind::Power);
    assert_eq!(placement.valid_tiles, path);
    assert!(placement.invalid_tiles.is_empty());
    assert_eq!(state.place_network_path(&path, NetworkKind::Power), 4);
    // The PE neighbor-gate table rejects bridge XBLD 0x5c, so endpoint
    // reshaping sees no conductive neighbor and keeps the no-connection tile.
    assert_eq!(state.city.building_at(10, 10), 14);
    assert_eq!(state.city.building_at(11, 10), 92); // POWER_BRIDGE
    assert_eq!(state.city.building_at(12, 10), 92);
    assert_eq!(state.city.building_at(13, 10), 14);
    assert!(state.city.tile_bits_at(11, 10).unwrap().powerable());
}

#[test]
fn rail_bridges_alternate_decks_and_pylons_over_long_water_crossings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 11..=13 {
        city.set_terrain(x, 10, 0x10);
    }
    let mut state = test_state(city);
    let path = vec![(10, 10), (11, 10), (12, 10), (13, 10), (14, 10)];

    assert_eq!(state.place_network_path(&path, NetworkKind::Rail), 5);
    assert_eq!(state.city.building_at(10, 10), 45);
    assert_eq!(state.city.building_at(11, 10), 91);
    assert_eq!(state.city.building_at(12, 10), 90);
    assert_eq!(state.city.building_at(13, 10), 91);
    assert_eq!(state.city.building_at(14, 10), 45);
}

#[test]
fn win95_player_bridge_bulldozer_matches_destroy_building_selector1_walk() {
    for click_x in 11..=13 {
        let mut city =
            City::new_blank("/tmp/sc2000-pe-bridge-bulldoze.sc2", "PEBridge").expect("blank city");
        for x in 11..=13 {
            city.set_terrain(x, 10, 0x10);
        }
        let mut state = test_state(city);
        let path = vec![(10, 10), (11, 10), (12, 10), (13, 10), (14, 10)];
        assert_eq!(state.place_network_path(&path, NetworkKind::Road), 5);
        let traffic_before = state
            .city
            .traffic_density_slice()
            .map(|traffic| traffic.to_vec());
        let mut pe_city = state.city.clone();
        let mut pe_crt = Win95CrtRand::new(state.pe_process_crt.seed());

        pe_destroy_building_mode1_optional_crt(&mut pe_city, Some(&mut pe_crt), click_x, 10);
        pe_maptool_surface_xter_gate_4434de(&mut pe_city, click_x, 10, true);
        crate::app::demolition::pe_maptool_epilogue_44378e(&mut pe_city, click_x, 10);
        state.bulldoze_at(click_x, 10);
        if click_x == 11 {
            // The surviving west road is isolated after the bridge walk.
            assert_eq!(state.city.building_at(10, 10), 0x1d);
        }

        for y in 8..=12 {
            for x in 8..=16 {
                assert_eq!(
                    state.city.building_at(x, y),
                    pe_city.building_at(x, y),
                    "XBLD at ({x},{y}) from hit ({click_x},10)"
                );
                assert_eq!(
                    state.city.terrain_at(x, y),
                    pe_city.terrain_at(x, y),
                    "XTER at ({x},{y}) from hit ({click_x},10)"
                );
                assert_eq!(
                    state.city.altitude_at(x, y),
                    pe_city.altitude_at(x, y),
                    "ALTM at ({x},{y}) from hit ({click_x},10)"
                );
                assert_eq!(
                    state.city.tile_bits_at(x, y).map(TileBits::raw_bits),
                    pe_city.tile_bits_at(x, y).map(TileBits::raw_bits),
                    "XBIT at ({x},{y}) from hit ({click_x},10)"
                );
                assert_eq!(
                    state.city.zone_at(x, y).map(|zone| zone.raw_bits()),
                    pe_city.zone_at(x, y).map(|zone| zone.raw_bits()),
                    "XZON at ({x},{y}) from hit ({click_x},10)"
                );
            }
        }
        assert_eq!(
            state.pe_process_crt.seed(),
            pe_crt.seed(),
            "process CRT state from hit ({click_x},10)"
        );
        assert_eq!(
            state
                .city
                .traffic_density_slice()
                .map(|traffic| traffic.to_vec()),
            traffic_before,
            "XTRF remains unchanged from hit ({click_x},10)"
        );
    }
}

#[test]
fn win95_player_road_bulldozer_matches_destroy_building_mode1_core() {
    let mut city =
        City::new_blank("/tmp/sc2000-pe-road-bulldoze.sc2", "PERoad").expect("blank city");
    city.set_building(20, 20, 0x1d);
    let mut state = test_state(city);
    let traffic_before = state
        .city
        .traffic_density_slice()
        .map(|traffic| traffic.to_vec());
    let mut pe_city = state.city.clone();
    let mut pe_crt = Win95CrtRand::new(state.pe_process_crt.seed());

    pe_destroy_building_mode1_optional_crt(&mut pe_city, Some(&mut pe_crt), 20, 20);
    pe_maptool_surface_xter_gate_4434de(&mut pe_city, 20, 20, true);
    state.bulldoze_at(20, 20);

    for y in 16..=24 {
        for x in 16..=24 {
            assert_eq!(
                state.city.building_at(x, y),
                pe_city.building_at(x, y),
                "XBLD at ({x},{y})"
            );
            assert_eq!(
                state.city.terrain_at(x, y),
                pe_city.terrain_at(x, y),
                "XTER at ({x},{y})"
            );
            assert_eq!(
                state.city.altitude_at(x, y),
                pe_city.altitude_at(x, y),
                "ALTM at ({x},{y})"
            );
            assert_eq!(
                state.city.tile_bits_at(x, y).map(TileBits::raw_bits),
                pe_city.tile_bits_at(x, y).map(TileBits::raw_bits),
                "XBIT at ({x},{y})"
            );
            assert_eq!(
                state.city.zone_at(x, y).map(|zone| zone.raw_bits()),
                pe_city.zone_at(x, y).map(|zone| zone.raw_bits()),
                "XZON at ({x},{y})"
            );
        }
    }
    assert_eq!(
        state.pe_process_crt.seed(),
        pe_crt.seed(),
        "process CRT state"
    );
    assert_eq!(
        state
            .city
            .traffic_density_slice()
            .map(|traffic| traffic.to_vec()),
        traffic_before
    );
}

#[test]
fn rail_bridge_alternation_resets_between_separate_water_spans() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(11, 10, 0x10);
    city.set_terrain(13, 10, 0x10);
    let mut state = test_state(city);
    let path = vec![(10, 10), (11, 10), (12, 10), (13, 10), (14, 10)];

    assert_eq!(state.place_network_path(&path, NetworkKind::Rail), 5);
    assert_eq!(state.city.building_at(11, 10), 91);
    assert_eq!(state.city.building_at(13, 10), 91);
}

#[test]
fn network_bridges_require_straight_water_path_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(11, 10, 0x10);
    let state = test_state(city);
    let path = vec![(10, 10), (11, 10), (11, 11)];

    let placement = state.network_placement_for_path(&path, NetworkKind::Road);
    assert_eq!(placement.valid_tiles, vec![(10, 10), (11, 11)]);
    assert_eq!(placement.invalid_tiles, vec![(11, 10)]);
}

#[test]
fn pe_post_flatten_neighbor_altm_445b50_pulls_cliff_within_one() {
    // PE `0x401a78`→`0x445b50`: |center-neighbor| > 1 pulls neighbor toward center±1.
    let mut city = City::new_blank("/tmp/sc2000-445b50.sc2", "PostFlat").expect("blank");
    city.set_altitude(20, 20, 8, BLANK_MAP_BASE_HEIGHT);
    city.set_altitude(20, 19, 4, BLANK_MAP_BASE_HEIGHT);
    let waits = pe_post_flatten_neighbor_altm_445b50(&mut city, 20, 20);
    // PE @ `0x445c15`: one `0x4012c1` before the adjusted neighbor write (+ recurse).
    assert!(waits >= 1);
    assert_eq!(city.altitude_at(20, 20), 8);
    assert_eq!(city.altitude_at(20, 19), 7);
}

#[test]
fn pe_post_flatten_neighbor_altm_445b50_lowers_too_high_neighbor() {
    // PE `0x445c58`: center - neighbor < -1 → neighbor := center + 1.
    let mut city = City::new_blank("/tmp/sc2000-445b50hi.sc2", "PostFlatHi").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 5, BLANK_MAP_BASE_HEIGHT);
        }
    }
    city.set_altitude(20, 20, 5, BLANK_MAP_BASE_HEIGHT);
    city.set_altitude(20, 19, 9, BLANK_MAP_BASE_HEIGHT);
    let waits = pe_post_flatten_neighbor_altm_445b50(&mut city, 20, 20);
    assert!(waits >= 1);
    assert_eq!(city.altitude_at(20, 19), 6);
}

#[test]
fn pe_post_flatten_neighbor_altm_445b50_skips_xbld_ge_0x0d() {
    // PE `0x445b50` `cmp XBLD, 0xd` / `jae` skip.
    let mut city = City::new_blank("/tmp/sc2000-445b50b.sc2", "PostFlatB").expect("blank");
    city.set_altitude(20, 20, 8, BLANK_MAP_BASE_HEIGHT);
    city.set_altitude(20, 19, 4, BLANK_MAP_BASE_HEIGHT);
    city.set_building(20, 19, PE_POST_FLATTEN_NEIGHBOR_XBLD_GATE);
    assert_eq!(pe_post_flatten_neighbor_altm_445b50(&mut city, 20, 20), 0);
    assert_eq!(city.altitude_at(20, 19), 4);
}

#[test]
fn pe_destroy_building_mode1_generic_render_scatter_advances_crt() {
    for (building, size) in [(6, 1usize), (174, 3usize), (207, 4usize)] {
        let mut city =
            City::new_blank("/tmp/sc2000-destroy-scatter.sc2", "Scatter").expect("blank city");
        for dy in 0..size {
            for dx in 0..size {
                city.set_building(40 + dx, 40 + dy, building);
                // Nonzero XTER suppresses the later, per-cell rubble draws.
                city.set_terrain(40 + dx, 40 + dy, 1);
            }
        }

        let mut crt = Win95CrtRand::new(0x45a0_2301);
        let mut expected = crt;
        let draws = crate::city::pe_destroy_building_mode1_render_scatter_draw_count(building);
        assert_eq!(draws, 2 * size * size * size);
        for _ in 0..draws {
            let _ = expected.random_u15();
        }

        let hit = if size == 1 { (40, 40) } else { (41, 41) };
        pe_destroy_building_mode1_optional_crt(&mut city, Some(&mut crt), hit.0, hit.1);
        assert_eq!(crt, expected, "XBLD {building:#04x} scatter draw count");
        for dy in 0..size {
            for dx in 0..size {
                assert_eq!(city.building_at(40 + dx, 40 + dy), 0);
            }
        }
    }

    assert_eq!(
        crate::city::pe_destroy_building_mode1_render_scatter_draw_count(0x70),
        2
    );
    assert_eq!(
        crate::city::pe_destroy_building_mode1_render_scatter_draw_count(0x51),
        0
    );
    assert_eq!(
        crate::city::pe_destroy_building_mode1_render_scatter_draw_count(0x5d),
        2
    );
    assert_eq!(
        crate::city::pe_destroy_building_mode1_render_scatter_draw_count(0x8c),
        0,
        "the tile-only helper has no city or hit coordinate for selector 2"
    );
    assert_eq!(
        crate::city::pe_destroy_building_mode1_render_scatter_draw_count(0xdf),
        0
    );

    // Mode 0 skips the generic render-scatter branch and has no rubble draws on
    // these nonzero-XTER tiles.
    let mut city =
        City::new_blank("/tmp/sc2000-destroy-scatter-mode0.sc2", "Scatter0").expect("blank city");
    for dy in 0..4 {
        for dx in 0..4 {
            city.set_building(40 + dx, 40 + dy, 207);
            city.set_terrain(40 + dx, 40 + dy, 1);
        }
    }
    let mut crt = Win95CrtRand::new(0x45a0_2301);
    let before = crt;
    pe_destroy_building_mode0_optional_crt(&mut city, Some(&mut crt), 41, 41);
    assert_eq!(crt, before);
}

#[test]
fn pe_destroy_building_selector2_highclass_walk_stops_at_map_edge() {
    let mut city = City::new_blank(
        "/tmp/sc2000-destroy-sel2-high-walk-edge.sc2",
        "Sel2WalkEdge",
    )
    .expect("blank city");
    for y in 126..128 {
        for x in 126..128 {
            city.set_building(x, y, 0x4a);
            city.set_zone(x, y, 0xf0);
            city.set_terrain(x, y, 1);
        }
    }
    city.set_tile_bits(126, 126, TileBits::new(0x04));

    assert_eq!(pe_highway_classify_463c40(&city, 126, 126), 0x0e);
    assert_eq!(pe_highway_classify_463c40(&city, 128, 126), 0);
    assert_eq!(
        pe_destroy_building_selector2_high_class_walk_plan(&city, 0x4a, 127, 127),
        Some(vec![[(126, 126), (126, 127), (127, 127), (127, 126)]])
    );

    pe_destroy_building_mode0_optional_crt(&mut city, None, 127, 127);
    for y in 126..128 {
        for x in 126..128 {
            assert_eq!(city.building_at(x, y), 0);
        }
    }
}

#[test]
fn player_bulldoze_selector2_lowclass_uses_fixed_square_for_incomplete_highway() {
    let mut city =
        City::new_blank("/tmp/sc2000-bulldoze-sel2-incomplete.sc2", "Sel2Gap").expect("blank city");
    for &(x, y) in &[(40, 41), (40, 40), (41, 41)] {
        city.set_building(x, y, 0x49);
    }
    let mut state = test_state(city);
    let mut expected_crt = state.pe_process_crt;
    for _ in 0..16 {
        let _ = expected_crt.random_u15();
    }
    for _ in 0..4 {
        let _ = expected_crt.rubble_tile_id();
    }

    state.bulldoze_at(40, 40);

    assert_eq!(state.pe_process_crt, expected_crt);
    for (x, y) in [(40, 41), (40, 40), (41, 41), (41, 40)] {
        assert!(
            is_rubble_tile(state.city.building_at(x, y)),
            "selector-2 low-class bulldoze must clear the snapped square at ({x},{y})"
        );
    }
}

#[test]
fn terrain_classifier_slope_0x32_applies_second_altm_land_plus() {
    // PE FUN_00447020 @ `0x4472ae` via city raise: slope 0x32 → second land++.
    let mut city = City::new_blank("/tmp/sc2000-raise-s32.sc2", "RaiseS32").expect("blank");
    city.set_city_funds(20_000);
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
        }
    }
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
        city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 12, 0);
    }
    city.set_altitude(20, 20, 10, 0);
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 20), 0x32);
    let sea = pe_selected_water_height_for_fun_00447020(&city);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, sea);
    assert_eq!(city.altitude_at(20, 20), 11);
    assert_eq!(city.terrain_at(20, 20), 0);
}

#[test]
fn lower_terrain_slope_0x32_still_applies_second_altm_land_plus() {
    // PE FUN_00447020 slope `0x32` land++ is shared after raise or lower.
    let mut city = City::new_blank("/tmp/sc2000-lower-s32.sc2", "LowerS32").expect("blank");
    city.set_city_funds(20_000);
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 11, 0);
        }
    }
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
        city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 14, 0);
    }
    let mut lowered = city.clone();
    lowered.set_altitude(20, 20, 10, 0);
    assert_eq!(pe_fun_00447020_slope_xter(&lowered, 20, 20), 0x32);
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_edit_at(20, 20));
    // Lower 11→10, slope 0x32 second ++ → 11 (net unchanged); dry XTER 0.
    assert_eq!(state.city.altitude_at(20, 20), 11);
    assert_eq!(state.city.terrain_at(20, 20), 0);
}

#[test]
fn level_terrain_445b50_pulls_cliff_neighbor_within_one() {
    // PE LevelTerrain `0x444a10`→`0x445b50` after ALTM write, then `0x447490`.
    let mut city = City::new_blank("/tmp/sc2000-level-445b50.sc2", "LevelSmooth").expect("blank");
    city.set_city_funds(20_000);
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    city.set_altitude(20, 19, 4, 0);
    let mut state = test_state(city);
    state.tool = EditTool::LevelTerrain;
    assert!(state.place_city_terrain_edit_at(20, 20));
    assert_eq!(state.city.altitude_at(20, 20), 8);
    // `0x445b50` pulls 4→7; `0x447490`/`FUN_00447020` slope `0x32` then land++ → 8.
    assert_eq!(state.city.altitude_at(20, 19), 8);
    // PE @ `0x4449c4` pre-write `0x4012c1` + cliff `0x445b50` adjust + `0x447490` ×18.
    assert!(state.pe_level_hold_ui_wait_4012c1_count >= 1 + 1 + 18);
}

#[test]
fn level_terrain_445b50_skips_xbld_ge_0x0d_neighbor() {
    let mut city = City::new_blank("/tmp/sc2000-level-445b50b.sc2", "LevelSmoothB").expect("blank");
    city.set_city_funds(20_000);
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    city.set_altitude(20, 19, 4, 0);
    city.set_building(20, 19, PE_POST_FLATTEN_NEIGHBOR_XBLD_GATE);
    let mut state = test_state(city);
    state.tool = EditTool::LevelTerrain;
    assert!(state.place_city_terrain_edit_at(20, 20));
    // `0x445b50` skips XBLD `>= 0x0d`; `0x447490` DestroyBuilding then slope `0x32` land++.
    assert!(state.city.building_at(20, 19) < PE_POST_FLATTEN_NEIGHBOR_XBLD_GATE);
    assert_eq!(state.city.altitude_at(20, 19), 5);
}

#[test]
fn stretch_terrain_hill_step_raises_when_neighbors_ge_center() {
    // PE StretchTerrain `0x443a50` / gate `0x444ff0`: land++ when NESW >= center.
    let mut city = City::new_blank("/tmp/sc2000-stretch-hill.sc2", "StretchHill").expect("blank");
    for y in 19..22 {
        for x in 19..22 {
            city.set_altitude(x, y, 7, 0);
        }
    }
    city.set_altitude(20, 20, 5, 0);
    assert!(pe_stretch_terrain_hill_step_443a50(&mut city, 20, 20));
    // land++ to 6; `0x447490` FUN_00447020 slope 0x32 second land++ → 7.
    assert_eq!(city.altitude_at(20, 20), 7);
    assert_ne!(city.terrain_at(21, 20), 0, "neighbor XTER remapped");
}

#[test]
fn stretch_terrain_hill_step_refuses_lower_neighbor() {
    // PE `0x444ff0`: any NESW neighbor land < center → refuse.
    let mut city = City::new_blank("/tmp/sc2000-stretch-gate.sc2", "StretchGate").expect("blank");
    for y in 19..22 {
        for x in 19..22 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    city.set_altitude(20, 19, 7, 0);
    assert!(!pe_stretch_terrain_hill_step_443a50(&mut city, 20, 20));
    assert_eq!(city.altitude_at(20, 20), 8);
}

#[test]
fn stretch_terrain_hill_step_refuses_land_above_0x1e() {
    // PE `0x444ff0` `cmp land, 0x1e` / `jg` refuse.
    let mut city = City::new_blank("/tmp/sc2000-stretch-max.sc2", "StretchMax").expect("blank");
    for y in 19..22 {
        for x in 19..22 {
            city.set_altitude(x, y, 0x1f, 0);
        }
    }
    assert!(!pe_stretch_terrain_hill_step_443a50(&mut city, 20, 20));
    assert_eq!(city.altitude_at(20, 20), 0x1f);
}

#[test]
fn stretch_terrain_hill_step_allows_land_eq_0x1e() {
    // PE `0x444ff0` `jg` — land `== 0x1e` still raises (clamped to `0x1f`).
    let mut city = City::new_blank("/tmp/sc2000-stretch-eq1e.sc2", "StretchEq1e").expect("blank");
    for y in 19..22 {
        for x in 19..22 {
            city.set_altitude(x, y, 0x1e, 0);
        }
    }
    assert!(pe_can_raise_stretch_hill_444ff0(&city, 20, 20));
    assert!(pe_stretch_terrain_hill_step_443a50(&mut city, 20, 20));
    assert_eq!(city.altitude_at(20, 20), 0x1f);
}

#[test]
fn pe_can_raise_stretch_hill_444ff0_skips_oob_nesw() {
    // PE `0x444ff0`: out-of-bounds NESW neighbors are skipped, not treated as lower.
    let mut city = City::new_blank("/tmp/sc2000-stretch-edge.sc2", "StretchEdge").expect("blank");
    for y in 0..3 {
        for x in 0..3 {
            city.set_altitude(x, y, 6, 0);
        }
    }
    assert!(pe_can_raise_stretch_hill_444ff0(&city, 0, 0));
    assert!(pe_stretch_terrain_hill_step_443a50(&mut city, 0, 0));
    assert!(city.altitude_at(0, 0) > 6);
}

#[test]
fn stretch_terrain_hill_crt_jitter_raises_neighbor_smooths_origin() {
    // PE `0x443a50`: CRT seed 12 → dx=1,dy=0; land++ at (21,20); `0x445b50` at (20,20).
    let mut city = City::new_blank("/tmp/sc2000-stretch-jit.sc2", "StretchJit").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 5, 0);
        }
    }
    let mut crt = Win95CrtRand::new(12);
    assert!(
        pe_stretch_terrain_hill_step_443a50_with_crt(&mut city, Some(&mut crt), 20, 20).is_some()
    );
    assert_eq!(city.altitude_at(20, 20), 5);
    assert_eq!(city.altitude_at(21, 20), 6);
}

#[test]
fn city_stretch_terrain_counts_445b50_447490_4012c1_waits() {
    // PE Stretch `0x443b4a`/`0x443b94`/`0x443b9e`: city paint counts UI waits.
    let mut city = City::new_blank("/tmp/sc2000-city-stretch-wait.sc2", "CStretch").expect("blank");
    city.set_city_funds(20_000);
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 5, 0);
            city.set_terrain(x, y, 0);
        }
    }
    let mut state = test_state(city);
    state.tool = EditTool::StretchTerrain;
    state.pe_process_crt = Win95CrtRand::new(17); // dx=0,dy=0
    assert!(state.place_city_stretch_terrain_at(20, 20));
    assert!(state.pe_stretch_hold_ui_wait_4012c1_count >= 1 + 18);
}

#[test]
fn city_spread_terrain_counts_445b50_447490_4012c1_waits() {
    // PE tool-4 `0x442a30` NESW: city paint counts `0x445b50`/`0x447490` waits.
    let mut city = City::new_blank("/tmp/sc2000-city-spread-wait.sc2", "CSpread").expect("blank");
    city.set_city_funds(20_000);
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_altitude(20, 19, 4, 0);
    let mut state = test_state(city);
    state.tool = EditTool::SpreadTerrain;
    assert!(state.place_city_spread_terrain_at(20, 20));
    // Four NESW neighbors each get `0x447490` (×18); cliff also yields `0x445b50` waits.
    assert!(state.pe_spread_ui_wait_4012c1_count >= 4 * 18);
}

#[test]
fn city_level_terrain_2x2_counts_per_tile_4012c1_waits() {
    // PE LevelTerrain per footprint tile: `0x4449c4` + `0x445b50`/`0x447490` waits.
    let mut city =
        City::new_blank("/tmp/sc2000-city-level-2x2-wait.sc2", "CLevel2").expect("blank");
    city.set_city_funds(20_000);
    for y in 18..24 {
        for x in 18..24 {
            city.set_altitude(x, y, 8, 0);
            city.set_terrain(x, y, 0);
        }
    }
    // Highway footprint is 2×2 via `pe_destroy_building_footprint`.
    city.set_building(20, 20, 0x61);
    city.set_building(21, 20, 0x61);
    city.set_building(20, 21, 0x61);
    city.set_building(21, 21, 0x61);
    let mut state = test_state(city);
    state.tool = EditTool::LevelTerrain;
    assert!(state.place_city_terrain_level_at(20, 20, 8));
    // Four footprint tiles × (pre-write `0x4012c1` + `0x447490` ×18).
    assert!(state.pe_level_hold_ui_wait_4012c1_count >= 4 * (1 + 18));
}

#[test]
fn edit_map_spread_cancel_runs_exit_401ce9() {
    let mut city = City::new_blank("/tmp/sc2000-spread-cancel.sc2", "SpreadCancel").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.pe_spread_hold_cursor_id, 0x1fc);
    state.cancel_edit_map_spread_hold();
    assert_eq!(state.pe_spread_hold_cursor_id, 0);
}

#[test]
fn terrain_tool4_nesw_land_copy_sets_neighbors_to_center() {
    // PE tool-4 `0x442a30`: copy center land onto NESW, then `0x445b50` + `0x447490` each.
    let mut city = City::new_blank("/tmp/sc2000-tool4-copy.sc2", "Tool4Copy").expect("blank");
    for y in 15..26 {
        for x in 15..26 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(20, 19, 4, 0);
    city.set_altitude(21, 20, 4, 0);
    city.set_altitude(20, 21, 4, 0);
    city.set_altitude(19, 20, 4, 0);
    pe_terrain_nesw_land_copy_442a30(&mut city, 20, 20);
    assert!(city.altitude_at(20, 19) >= 9);
    assert!(city.altitude_at(21, 20) >= 9);
    assert!(city.altitude_at(20, 21) >= 9);
    assert!(city.altitude_at(19, 20) >= 9);
}

#[test]
fn terrain_tool4_nesw_land_copy_445b50_pulls_second_ring() {
    // After copy, `0x445b50` on a neighbor can pull a farther cliff within one;
    // `0x447490` may then slope-0x32 raise it again.
    let mut city = City::new_blank("/tmp/sc2000-tool4-smooth.sc2", "Tool4Smooth").expect("blank");
    for y in 17..24 {
        for x in 17..24 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    city.set_altitude(20, 20, 8, 0);
    city.set_altitude(20, 19, 8, 0);
    city.set_altitude(20, 18, 3, 0);
    pe_terrain_nesw_land_copy_442a30(&mut city, 20, 20);
    assert_eq!(city.altitude_at(20, 19), 8);
    assert!(city.altitude_at(20, 18) >= 7);
}

#[test]
fn pe_maptool_444bf0_unpack_masks_differ_for_raise_and_lower() {
    // PE Raise `0x442f5d` masks X with `0xff`; Lower `0x443964` masks with `0x7f`.
    assert_eq!(pe_raise_unpack_tile_444bf0(-1), None);
    assert_eq!(pe_raise_unpack_tile_444bf0(0x1410), Some((0x10, 0x14)));
    assert_eq!(pe_raise_unpack_tile_444bf0(0x1480), None); // x==0x80
    // Same packed word: Lower keeps X in-bounds via `0x7f`; Raise sees X `0x90` and refuses.
    assert_eq!(pe_lower_unpack_tile_444bf0(0x1490), Some((0x10, 0x14)));
    assert_eq!(pe_raise_unpack_tile_444bf0(0x1490), None);
}

#[test]
fn pe_lower_from_packed_443900_unpacks_before_jitter() {
    // PE Lower `0x443964`: `& 0x7f` unpack of `0x444bf0` before CRT jitter dig.
    // Packed `0x1490` → Raise would refuse X `0x90`; Lower keeps `(0x10, 0x14)`.
    let mut city = City::new_blank("/tmp/sc2000-lower-packed.sc2", "LowerPacked").expect("blank");
    let mut crt = Win95CrtRand::new(1);
    let mut expected_crt = crt;
    let (dx, dy) = pe_stretch_hill_crt_jitter(&mut expected_crt);
    let lower_x = (0x10isize + dx) as usize;
    let lower_y = (0x14isize + dy) as usize;
    // Keep the jitter-selected cell one level above its neighborhood, so the
    // native 0x32 slope correction does not undo the lower operation.
    for y in lower_y - 4..=lower_y + 4 {
        for x in lower_x - 4..=lower_x + 4 {
            city.set_altitude(x, y, 8, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_altitude(lower_x, lower_y, 10, 0);
    assert_eq!(pe_lower_unpack_tile_444bf0(0x1490), Some((0x10, 0x14)));
    assert_eq!(pe_raise_unpack_tile_444bf0(0x1490), None);
    assert_eq!(
        pe_lower_terrain_from_packed_443900(&mut city, &mut crt, 0x1490),
        Some(true)
    );
    assert_eq!(city.altitude_at(lower_x, lower_y), 9);
}

#[test]
fn pe_lower_from_packed_443900_refuses_negative_packed() {
    // PE `0x444bf0` negative AX → Lower packed step refuses dig.
    let mut city =
        City::new_blank("/tmp/sc2000-lower-pack-ref.sc2", "LowerPackRef").expect("blank");
    city.set_altitude(0x10, 0x14, 10, 0);
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(pe_lower_unpack_tile_444bf0(-1), None);
    assert_eq!(
        pe_lower_terrain_from_packed_443900(&mut city, &mut crt, -1),
        None
    );
    assert_eq!(city.altitude_at(0x10, 0x14), 10);
}

#[test]
fn edit_map_lower_click_counts_ui_refresh_and_blit() {
    // PE Lower success path @ `0x4439c9`/`0x4439d2`: packed click counts UI + blit.
    let mut city = City::new_blank("/tmp/sc2000-emap-lower-ui.sc2", "ELowerUI").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 10, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_altitude(21, 20, 12, 0); // preserve a high slope neighbor through 0x447490
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::LowerTerrain;
    state.pe_process_crt = Win95CrtRand::new(17); // dx=0,dy=0
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.altitude_at(20, 20), 9);
    assert_eq!(state.pe_lower_hold_ui_refresh_count, 1);
    assert_eq!(state.pe_lower_hold_blit_count, 1);
}

#[test]
fn pe_stretch_terrain_mouse_land_444951_matches_pe() {
    // PE LevelTerrain @ `0x444951`: ΔY / rot-divisor → land; clamps + base-10 floor.
    // mouse_y < press → negative delta → lower land.
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(10, 10, 100, 91, 0),
        Some(7) // (91-100)/3 = -3 → 10-3=7
    );
    // mouse_y > press → raise
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(10, 10, 100, 109, 0),
        Some(13)
    );
    // rot 1 divisor 6
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(10, 10, 100, 112, 1),
        Some(12)
    );
    // rot 2 divisor 12
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(10, 10, 100, 124, 2),
        Some(12)
    );
    // rot 3 raw delta
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(10, 10, 100, 103, 3),
        Some(13)
    );
    // clamp high / low
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(2, 2, 100, 200, 3),
        None
    );
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(0x1e, 0x1e, 100, 0, 3),
        None
    );
    // base-10 floor: new must be >= base-10
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(15, 15, 100, 100 - 11, 3),
        None
    ); // 15-11=4 < 5
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(15, 15, 100, 100 - 6, 3),
        Some(9)
    ); // 15-6=9 >= 5
    // unchanged
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(10, 10, 100, 100, 0),
        None
    );
    assert_eq!(
        pe_stretch_terrain_mouse_land_444951(10, 13, 100, 109, 0),
        None
    );
}

#[test]
fn pe_lower_crt_step_keeps_flat_neighbors_after_447490() {
    // A successful PE Lower `0x444440` mode-0 action runs `0x40215d`→`0x447490`.
    // On this flat fixture the shared terrain reclassifier leaves neighbor XTER at zero.
    let mut city = City::new_blank("/tmp/sc2000-lower-no447.sc2", "LowerNo447").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 10, 0);
            city.set_terrain(x, y, 0);
        }
    }
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(
        pe_lower_terrain_crt_step_443900(&mut city, &mut crt, 20, 20),
        Some(true)
    );
    let mut dug = None;
    for y in 17..24 {
        for x in 17..24 {
            if (0..128).contains(&x) && (0..128).contains(&y) && city.altitude_at(x, y) < 10 {
                dug = Some((x, y));
            }
        }
    }
    let (dx, dy) = dug.expect("CRT dig changed a tile");
    for (nx, ny) in [
        (dx.wrapping_sub(1), dy),
        (dx + 1, dy),
        (dx, dy.wrapping_sub(1)),
        (dx, dy + 1),
    ] {
        if nx < 128 && ny < 128 && (nx, ny) != (dx, dy) {
            assert_eq!(
                city.terrain_at(nx, ny),
                0,
                "Lower must not run neighbor 447490 at ({nx},{ny})"
            );
        }
    }
}

#[test]
fn pe_lower_worker_runs_447490_demolition_and_preserves_xtrf() {
    let mut city = City::new_blank("/tmp/sc2000-lower-447490.sc2", "Lower447490").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 10, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_building(21, 20, 0x1d); // ordinary road inside the 3×3 refresh.
    city.set_traffic_density(21, 20, 0x57);
    let mut crt = Win95CrtRand::new(0x4444);

    assert!(pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt(
        &mut city,
        20,
        20,
        0,
        PE_RAISE_SUBMODE_DEFAULT,
        Some(&mut crt),
    ));
    assert!(
        (1..=4).contains(&city.building_at(21, 20)),
        "CRT-backed DestroyBuilding leaves rubble"
    );
    assert_eq!(
        city.traffic_density_at(21, 20),
        Some(0x57),
        "terrain reclassification and PlaceTile preserve the XTRF data"
    );
}

#[test]
fn pe_raise_adjust_toward_dig_runs_447490_and_carries_crt() {
    let mut city =
        City::new_blank("/tmp/sc2000-adjust-dig-447490.sc2", "AdjustDig").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 10, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_city_funds(100);
    city.set_building(21, 20, 0x1d);
    city.set_traffic_density(21, 20, 0x57);
    let mut crt = Win95CrtRand::new(0x4444);

    let waits = pe_raise_adjust_land_toward_443200_with_crt(&mut city, Some(&mut crt), 20, 20, 9)
        .expect("above-target cell should dig");
    assert_eq!(waits, 8 + 25 * PE_447490_UI_WAIT_4012C1_PER_CALL);
    assert_eq!(city.city_funds(), 0);
    assert!(
        (1..=4).contains(&city.building_at(21, 20)),
        "adjust-toward's CRT-backed 447490 pass demolishes to rubble"
    );
    assert_eq!(city.traffic_density_at(21, 20), Some(0x57));
}

#[test]
fn edit_map_lower_uses_packed_443900_path() {
    // PE Lower CRT goes through `0x444bf0` unpack (`& 0x7f`) before jitter dig.
    let mut city =
        City::new_blank("/tmp/sc2000-emap-lower-pack.sc2", "EmapLowerPack").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 10, 0);
            city.set_terrain(x, y, 0);
        }
    }
    city.set_altitude(21, 20, 12, 0);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::LowerTerrain;
    state.pe_process_crt = Win95CrtRand::new(17); // dx=0,dy=0
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.altitude_at(20, 20), 9);
}

#[test]
fn edit_map_spread_drag_uses_packed_442a30_path() {
    // PE Spread hold @ `0x442b07`: `& 0x7f` unpack of `0x444bf0` before same-tile NESW copy.
    let mut city =
        City::new_blank("/tmp/sc2000-spread-packed-drag.sc2", "SpreadPack").expect("blank");
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
    let (applied, _) = state.paint_edit_map_tool_drag_to(Some((20, 20)), (21, 20), false);
    assert_eq!(applied, 1);
    // Neighbor of (21,20) should get land 12 from NESW copy of moved center.
    assert_eq!(state.city.altitude_at(22, 20), 12);
}

#[test]
fn pe_raise_drag_step_dir_445da0_matches_pe() {
    // PE `0x445da0`: same tile → 0xffff; else NESW by dominant delta.
    assert_eq!(pe_raise_drag_step_dir_445da0(20, 20, 20, 20), 0xffff);
    assert_eq!(pe_raise_drag_step_dir_445da0(21, 20, 20, 20), 1); // east
    assert_eq!(pe_raise_drag_step_dir_445da0(19, 20, 20, 20), 3); // west
    assert_eq!(pe_raise_drag_step_dir_445da0(20, 21, 20, 20), 2); // south
    assert_eq!(pe_raise_drag_step_dir_445da0(20, 19, 20, 20), 0); // north
    // Diagonal: |dx|==|dy| uses the dy branch (north/south).
    assert_eq!(pe_raise_drag_step_dir_445da0(21, 21, 20, 20), 2);
    assert_eq!(pe_raise_drag_step_dir_445da0(19, 19, 20, 20), 0);
    // |dx| > |dy| still east/west.
    assert_eq!(pe_raise_drag_step_dir_445da0(22, 21, 20, 20), 1);
}

#[test]
fn pe_raise_epilogue_44c7a0_44c9b0_band_gates() {
    // PE `0x44c7a0` / `0x44c9b0` exclusive-end bands for reshape dispatch.
    assert_eq!(
        pe_raise_epilogue_surface_reshape_kind_44c7a0(0x1d),
        Some(NetworkKind::Road)
    );
    assert_eq!(
        pe_raise_epilogue_surface_reshape_kind_44c7a0(0x2b),
        Some(NetworkKind::Road)
    );
    assert_eq!(
        pe_raise_epilogue_surface_reshape_kind_44c7a0(0x2c),
        Some(NetworkKind::Rail)
    );
    assert_eq!(
        pe_raise_epilogue_surface_reshape_kind_44c7a0(0x3a),
        Some(NetworkKind::Rail)
    );
    assert_eq!(
        pe_raise_epilogue_surface_reshape_kind_44c7a0(0x0e),
        Some(NetworkKind::Power)
    );
    assert_eq!(
        pe_raise_epilogue_surface_reshape_kind_44c7a0(0x1c),
        Some(NetworkKind::Power)
    );
    assert_eq!(pe_raise_epilogue_surface_reshape_kind_44c7a0(0x0d), None);
    assert_eq!(pe_raise_epilogue_surface_reshape_kind_44c7a0(0x3b), None);
    assert_eq!(
        pe_raise_epilogue_underground_reshape_kind_44c9b0(1),
        Some(UndergroundKind::Subway)
    );
    assert_eq!(
        pe_raise_epilogue_underground_reshape_kind_44c9b0(0x0f),
        Some(UndergroundKind::Subway)
    );
    assert_eq!(
        pe_raise_epilogue_underground_reshape_kind_44c9b0(0x10),
        Some(UndergroundKind::Pipes)
    );
    assert_eq!(
        pe_raise_epilogue_underground_reshape_kind_44c9b0(0x1e),
        Some(UndergroundKind::Pipes)
    );
    assert_eq!(pe_raise_epilogue_underground_reshape_kind_44c9b0(0), None);
    assert_eq!(
        pe_raise_epilogue_underground_reshape_kind_44c9b0(0x1f),
        None
    );
}

#[test]
fn pe_lower_crt_oob_jitter_refuses() {
    // PE Lower `0x443900`: out-of-bounds CRT jitter tile → no dig.
    let mut city = City::new_blank("/tmp/sc2000-lower-oob.sc2", "LowerOob").expect("blank");
    for y in 0..5 {
        for x in 0..5 {
            city.set_altitude(x, y, 5, 0);
        }
    }
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(
        pe_lower_terrain_crt_step_443900(&mut city, &mut crt, 0, 0),
        None
    );
    assert_eq!(city.altitude_at(0, 0), 5);
}

#[test]
fn pe_raise_adjust_toward_raises_or_digs_one_step() {
    let mut city = City::new_blank("/tmp/sc2000-raise-adj.sc2", "RaiseAdj").expect("blank");
    city.set_city_funds(10_000);
    city.set_altitude(20, 20, 5, 0);
    // Recursive refresh raises this one-tile depression to the surrounding grade.
    assert!(pe_raise_adjust_land_toward_443200(&mut city, 20, 20, 7));
    assert_eq!(city.altitude_at(20, 20), BLANK_MAP_BASE_HEIGHT);
    let funds = city.city_funds();
    assert!(pe_raise_adjust_land_toward_443200(
        &mut city,
        20,
        20,
        BLANK_MAP_BASE_HEIGHT
    ));
    assert_eq!(city.city_funds(), funds);
    assert!(pe_raise_adjust_land_toward_443200(&mut city, 20, 20, 3));
    assert!(city.city_funds() < funds);
}

#[test]
fn pe_raise_adjust_dig_unions_initial_seed_rect_for_refresh() {
    // The native seed RECT is [x,y,x+1,y+1], so a dig with no relaxed neighbors
    // still refreshes a 4×4 center rectangle after the one-tile expansion.
    let mut city =
        City::new_blank("/tmp/sc2000-adjust-dig-seed-rect.sc2", "AdjustSeedRect").expect("blank");
    for y in 18..24 {
        for x in 18..24 {
            city.set_altitude(x, y, 5, 0);
        }
    }
    city.set_altitude(20, 20, 6, 0);
    city.set_city_funds(1_000);
    let mut crt = Win95CrtRand::new(0x4444);

    let waits = pe_raise_adjust_land_toward_443200_with_crt(&mut city, Some(&mut crt), 20, 20, 5)
        .expect("above-target tile should dig");
    assert_eq!(waits, 16 * PE_447490_UI_WAIT_4012C1_PER_CALL);
    assert_eq!(city.city_funds(), 1_000 - 0x19);
}

#[test]
fn pe_raise_land_inc_443c60_refuses_military_zone() {
    // PE `0x443c60`→`0x443cb0` @ `0x443d05`: zone nibble 7 returns 0 (no land++).
    let mut city = City::new_blank("/tmp/sc2000-raise-mil.sc2", "RaiseMil").expect("blank");
    city.set_altitude(20, 20, 5, 0);
    city.set_zone(20, 20, ZONE_MILITARY);
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2).is_none());
    assert_eq!(city.altitude_at(20, 20), 5);
}

#[test]
fn pe_raise_land_inc_443c60_water_refuses_only_submode0() {
    // PE `0x443d3f`: water bit refuses when `[0x4cc950]==0`; submode 2 still raises.
    let mut city = City::new_blank("/tmp/sc2000-raise-water.sc2", "RaiseWater").expect("blank");
    city.set_altitude(20, 20, 5, 0);
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    city.set_city_funds(0x19);
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, PE_RAISE_SUBMODE_DEFAULT).is_none());
    assert_eq!(city.altitude_at(20, 20), 5);
    assert_eq!(city.city_funds(), 0x19);
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2).is_some());
    assert_eq!(city.city_funds(), 0);
}

#[test]
fn pe_raise_land_inc_443c60_neighbor_water_gate_uses_submode() {
    let mut city = City::new_blank("/tmp/sc2000-raise-nb.sc2", "RaiseNb").expect("blank");
    city.set_altitude(20, 20, 5, 0);
    city.set_city_funds(0x19);
    city.set_zone(21, 20, ZONE_MILITARY);
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2).is_none());
    assert_eq!(city.altitude_at(20, 20), 5);
    assert_eq!(city.city_funds(), 0x19);
    city.set_zone(21, 20, 0);
    city.set_tile_bits(19, 19, TileBits::new(0).with_water_tile(true));
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, PE_RAISE_SUBMODE_DEFAULT).is_none());
    assert_eq!(city.altitude_at(20, 20), 5);
    assert_eq!(city.city_funds(), 0x19);
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2).is_some());
    assert_eq!(city.city_funds(), 0);
}

#[test]
fn pe_raise_land_inc_443c60_preset_mark_skips_neighbor_gate() {
    // PE `0x443d7f`: bit 8 already set skips `0x443cb0` neighbor/height gates.
    // `0x4441c0` still clears the mark and refuses land `>= 0x1e`.
    let mut city = City::new_blank("/tmp/sc2000-raise-preset.sc2", "RaisePreset").expect("blank");
    city.set_altitude(20, 20, 0x1e, 0);
    city.set_tile_bits(20, 20, TileBits::new(0).with_flatten_mark(true));
    assert_eq!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2), Some(0));
    assert_eq!(city.altitude_at(20, 20), 0x1e);
    assert!(!city.tile_bits_at(20, 20).unwrap().flatten_mark());
    city.set_altitude(20, 20, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_zone(21, 20, ZONE_MILITARY);
    city.set_tile_bits(20, 20, TileBits::new(0).with_flatten_mark(true));
    city.set_city_funds(0x19);
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2).is_some());
    assert_eq!(city.altitude_at(20, 20), BLANK_MAP_BASE_HEIGHT + 1);
}

#[test]
fn pe_raise_land_inc_443c60_recurses_marked_lower_neighbor() {
    // PE `0x44427e`: lower NESW with flatten mark takes `0x4441c0` before this debit.
    let mut city = City::new_blank("/tmp/sc2000-raise-rec.sc2", "RaiseRec").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 5, 0);
        }
    }
    city.set_altitude(19, 20, 4, 0);
    city.set_tile_bits(19, 20, TileBits::new(0).with_flatten_mark(true));
    city.set_city_funds(0x19);
    assert_eq!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2), Some(18));
    assert_eq!(city.altitude_at(20, 20), 5);
    assert!(city.altitude_at(19, 20) > 4);
    assert_eq!(city.city_funds(), 0);
}

#[test]
fn pe_raise_land_inc_443c60_clears_zone_low_nibble() {
    // PE `0x44439e`: after `0x4441c0` land++, XZON byte `&= 0xf0`.
    let mut city = City::new_blank("/tmp/sc2000-raise-zon.sc2", "RaiseZon").expect("blank");
    city.set_altitude(20, 20, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_city_funds(0x19);
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2).is_some());
    assert_eq!(city.altitude_at(20, 20), BLANK_MAP_BASE_HEIGHT + 1);
    let zone = city.zone_at(20, 20).map(|z| z.raw_bits()).unwrap_or(0xff);
    assert_eq!(zone & 0x0f, 0);
    assert_eq!(zone & 0xf0, ZONE_CORNER_ALL);
}

#[test]
fn pe_raise_land_inc_443c60_smooths_high_neighbor() {
    // PE `0x4443a4`/`0x4443ae`: `0x445b50`/`0x447490` after land++ (incl. UI waits).
    let mut city = City::new_blank("/tmp/sc2000-raise-443c60.sc2", "Raise443c60").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 5, 0);
        }
    }
    city.set_altitude(20, 20, 5, 0);
    city.set_altitude(20, 19, 9, 0);
    city.set_city_funds(0x19);
    let waits = pe_raise_land_inc_443c60(&mut city, 20, 20, 2).expect("raise");
    assert!(waits >= 18);
    assert_eq!(city.altitude_at(20, 20), 6);
    assert_eq!(city.altitude_at(20, 19), 7);
    assert_ne!(city.terrain_at(21, 20), 0, "447490 remapped neighbor XTER");
    city.set_altitude(21, 21, 0x1e, 0);
    assert!(pe_raise_land_inc_443c60(&mut city, 21, 21, 2).is_none());
    assert_eq!(city.altitude_at(21, 21), 0x1e);
    assert!(city.tile_bits_at(21, 21).unwrap().flatten_mark());
}

#[test]
fn pe_raise_and_adjust_match_native_marked_terrain_raises() {
    // Retail replays through 0x443c60 and 0x443200. The latter targets land 12.
    // Counts omit the nine UI waits inside the nested demolition worker.
    const EMPTY: u64 = 0x9c1b_da7f_8c87_2325;
    let cases = [
        (
            "flat",
            (20, 20),
            &[][..],
            None,
            100,
            Some(18),
            75,
            1,
            [0xd2a8_65cf_8809_2c64, 0xe705_34f7_e0d0_9729, EMPTY],
        ),
        (
            "outside",
            (20, 20),
            &[8][..],
            Some(2),
            100,
            Some(36),
            50,
            415_139_642,
            [
                0xd2a8_65cf_8809_2c64,
                0xe705_34f7_e0d0_9729,
                0xee8a_0bb0_2078_95fe,
            ],
        ),
        (
            "recursion",
            (20, 20),
            &[6, 6][..],
            Some(3),
            100,
            Some(336),
            50,
            415_139_642,
            [
                0xd2a8_65cf_8809_2c64,
                0xe705_34f7_e0d0_9729,
                0x940a_8edd_62b4_aef4,
            ],
        ),
        (
            "edge",
            (0, 20),
            &[8][..],
            Some(2),
            100,
            Some(36),
            50,
            415_139_642,
            [
                0xd772_142a_84e6_8c64,
                0x63bb_86e7_9d32_d8d9,
                0x7379_e228_2ae4_7dfe,
            ],
        ),
        (
            "smoothing",
            (20, 20),
            &[13][..],
            Some(2),
            100,
            Some(78),
            75,
            415_139_642,
            [
                0x699c_5b03_d2b3_1b42,
                0x80ed_67f4_5436_d223,
                0xee8a_0bb0_2078_95fe,
            ],
        ),
        (
            "partial",
            (20, 20),
            &[8][..],
            None,
            25,
            Some(18),
            0,
            1,
            [0xf473_a285_a960_2325, EMPTY, EMPTY],
        ),
        (
            "blocked",
            (20, 20),
            &[8][..],
            None,
            100,
            None,
            100,
            1,
            [0x084e_5a50_daa5_1c77, EMPTY, EMPTY],
        ),
        (
            "wet",
            (20, 20),
            &[][..],
            None,
            100,
            Some(18),
            75,
            1,
            [0xd2a8_65cf_8809_2c64, 0xe705_34f7_e0d0_9729, EMPTY],
        ),
    ];

    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for adjust in [false, true] {
            for (name, (x, y), heights, building, funds, waits, cash, seed, planes) in cases {
                let context = format!("{kind:?} adjust={adjust} {name}");
                let mut city = City::new_blank("/tmp/citygame-pe-raise.sc2", "Raise").unwrap();
                city.kind = kind;
                city.set_city_funds(funds);
                city.set_mac_selected_water_height(0);
                for x in 0..CITY_WIDTH {
                    for y in 0..CITY_HEIGHT {
                        city.set_altitude(x, y, 10, 0);
                    }
                }
                for (i, &land) in heights.iter().enumerate() {
                    city.set_altitude(x, y - 1 - i, land, 0);
                }
                if let Some(distance) = building {
                    city.set_building(x, y - distance, 0x70);
                }
                if name == "blocked" {
                    city.set_zone(x, y - 2, ZONE_MILITARY);
                }
                if name == "wet" {
                    city.set_tile_bits(x, y - 1, TileBits::new(4));
                }
                let mut crt = Win95CrtRand::new(1);
                let actual = if adjust {
                    pe_raise_adjust_land_toward_443200_with_crt(&mut city, Some(&mut crt), x, y, 12)
                } else {
                    pe_raise_land_inc_443c60_with_crt(&mut city, Some(&mut crt), x, y, 2)
                };
                assert_eq!(actual, waits, "{context}");
                assert_eq!(city.city_funds(), cash, "{context}");
                assert_eq!(crt.seed(), seed, "{context}");
                assert_eq!(
                    win95_terrain_plane_fingerprints(&city),
                    [planes[0], planes[1], planes[2], EMPTY],
                    "{context}",
                );
            }
        }
    }
}

#[test]
fn win95_city_raise_tool_matches_native_billing_and_nested_refresh() {
    const EMPTY: u64 = 0x9c1b_da7f_8c87_2325;
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for (funds, remaining, altm, xter) in [
            (100, 50, 0xd2a8_65cf_8809_2c64, 0xe705_34f7_e0d0_9729),
            (25, 0, 0xf473_a285_a960_2325, EMPTY),
        ] {
            let mut city = flat_win95_terrain_fixture(kind);
            city.set_city_funds(funds);
            city.pe_w_city_mode = CITY_MODE_GAMEPLAY as u16;
            city.set_altitude(20, 19, 8, 0);
            city.set_building(20, 18, 0x70);
            let mut state = test_state(city);
            state.tool = EditTool::RaiseTerrain;
            state.pe_process_crt = Win95CrtRand::new(1);

            if funds == 100 {
                assert!(state.apply_tool_at_tile(20, 20));
            } else {
                state.begin_terrain_drag(20, 20);
                assert_eq!(state.finish_terrain_drag(), 1);
            }

            assert_eq!(state.city.city_funds(), remaining, "{kind:?} funds={funds}");
            assert_eq!(state.pe_process_crt.seed(), 415_139_642);
            assert_eq!(
                win95_terrain_plane_fingerprints(&state.city),
                [altm, xter, EMPTY, EMPTY]
            );
        }
    }
}

#[test]
fn win95_editor_raise_tool_matches_native_recursive_refresh() {
    const EMPTY: u64 = 0x9c1b_da7f_8c87_2325;
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for (name, x, changed, crt, planes) in [
            (
                "flat",
                20,
                true,
                3_884_216_597,
                [0xecde_326e_ef37_3584, 0xe45e_75ad_6e00_87c9, EMPTY, EMPTY],
            ),
            (
                "edge",
                0,
                false,
                3_884_216_597,
                [0xf473_a285_a960_2325, EMPTY, EMPTY, EMPTY],
            ),
            (
                "blocked",
                20,
                false,
                3_884_216_597,
                [0x71bd_fea6_6d59_6706, EMPTY, EMPTY, EMPTY],
            ),
            (
                "nested",
                20,
                true,
                752_224_798,
                [
                    0x27c5_a872_a92d_74ad,
                    0xdf52_78a6_55c3_2ab9,
                    0xee8a_0bb0_2078_95fe,
                    EMPTY,
                ],
            ),
        ] {
            let mut city = flat_win95_terrain_fixture(kind);
            city.set_city_funds(100);
            if name == "blocked" {
                city.set_altitude(17, 22, 9, 0);
            }
            if name == "nested" {
                city.set_altitude(20, 19, 13, 0);
                city.set_building(20, 18, 0x70);
            }
            let mut state = test_state(city);
            state.mode = AppMode::EditNewMap;
            state.tool = EditTool::RaiseTerrain;
            state.pe_process_crt = Win95CrtRand::new(1);
            let mac_seed = state.mac_process_rng.seed();

            assert_eq!(state.apply_tool_at_tile(x, 20), changed, "{kind:?} {name}");
            assert_eq!(state.city.city_funds(), 100);
            assert_eq!(state.pe_process_crt.seed(), crt, "{kind:?} {name}");
            assert_eq!(state.mac_process_rng.seed(), mac_seed);
            assert_eq!(
                win95_terrain_plane_fingerprints(&state.city),
                planes,
                "{kind:?} {name}"
            );
        }
    }
}

#[test]
fn win95_editor_stretch_matches_native_vertical_drag() {
    const EMPTY: u64 = 0x9c1b_da7f_8c87_2325;
    const FLAT: [u64; 4] = [0xf473_a285_a960_2325, EMPTY, EMPTY, EMPTY];
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for (name, samples, crt, planes) in [
            (
                "recursive",
                &[(1, true)][..],
                415_139_642,
                [
                    0xd2a8_65cf_8809_2c64,
                    0xe705_34f7_e0d0_9729,
                    0xee8a_0bb0_2078_95fe,
                    EMPTY,
                ],
            ),
            (
                "reverse",
                &[(1, true), (1, false), (-1, true), (-1, false), (0, true)][..],
                1,
                [0xb07c_5dca_a653_fef9, EMPTY, EMPTY, EMPTY],
            ),
            (
                "limit",
                &[(21, false), (-10, false), (-1, true)][..],
                1,
                FLAT,
            ),
            ("refill", &[(-2, true), (-2, false)][..], 1, FLAT),
        ] {
            let mut city = flat_win95_terrain_fixture(kind);
            city.set_city_funds(100);
            if name == "recursive" {
                city.set_altitude(20, 19, 8, 0);
                city.set_building(20, 18, 0x70);
            }
            if name == "reverse" {
                city.set_altm_word(20, 20, 0x14aa);
            }
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Win95;
            state.mode = AppMode::EditNewMap;
            state.tool = EditTool::StretchTerrain;
            state.zoom = 0.75;
            state.viewport_pixel_scale = 2.0;
            state.camera_rotation = 3;
            state.pe_process_crt = Win95CrtRand::new(1);
            let mac_seed = state.mac_process_rng.seed();

            assert!(!state.apply_tool_at_tile(20, 20));
            assert!(state.begin_edit_map_stretch_drag_at(20, 20, 500.0));
            assert!(!state.update_edit_map_stretch_drag_at(500.0));
            for &(levels, changed) in samples {
                let motion =
                    12.0 * levels as f32 * render_zoom(state.zoom, state.viewport_pixel_scale);
                assert_eq!(
                    state.update_edit_map_stretch_drag_at(500.0 - motion),
                    changed,
                    "{kind:?} {name} {levels}"
                );
            }
            assert_eq!(state.finish_edit_map_stretch_drag(), 1);
            assert_eq!(state.pe_stretch_hold_cursor_id, 0);
            assert_eq!(state.city.city_funds(), 100);
            assert_eq!(state.pe_process_crt.seed(), crt, "{kind:?} {name}");
            assert_eq!(state.mac_process_rng.seed(), mac_seed);
            assert_eq!(
                win95_terrain_plane_fingerprints(&state.city),
                planes,
                "{kind:?} {name}"
            );
        }
    }
}

#[test]
fn win95_editor_level_matches_native_cardinal_copy_and_edge_refresh() {
    const EMPTY: u64 = 0x9c1b_da7f_8c87_2325;
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for (edge, press, crt, expected) in [
            (
                false,
                (20, 22),
                1,
                [0xe964_eaf7_437d_9ae3, 0x6987_f458_77e9_fade, EMPTY, EMPTY],
            ),
            (
                true,
                (0, 20),
                415_139_642,
                [
                    0x1f1e_139c_8be7_c124,
                    0x61ef_a8f9_23aa_da79,
                    0xa5b6_44f0_42a3_06f4,
                    EMPTY,
                ],
            ),
        ] {
            let mut city = flat_win95_terrain_fixture(kind);
            city.set_city_funds(100);
            if edge {
                city.set_altitude(0, 20, 12, 0);
                city.set_altitude(0, 18, 8, 0);
                city.set_building(0, 17, 0x70);
            } else {
                city.set_altitude(20, 22, 11, 0);
                city.set_altitude(22, 22, 12, 0);
                city.set_altitude(27, 28, 9, 0);
            }
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Win95;
            state.mode = AppMode::EditNewMap;
            state.tool = EditTool::LevelTerrain;
            state.pe_process_crt = Win95CrtRand::new(1);
            let mac_seed = state.mac_process_rng.seed();

            assert!(state.begin_edit_map_level_drag_at(press.0, press.1));
            assert_eq!(state.city.text_overlay_at(press.0, press.1), Some(0xc9));
            if !edge {
                state.hover_tile = Some(press);
                assert!(!state.update_edit_map_level_drag());
                state.hover_tile = Some((28, 28));
                assert!(state.update_edit_map_level_drag());
                assert_eq!(state.city.text_overlay_at(press.0, press.1), Some(0));
            }
            assert_eq!(state.finish_edit_map_level_drag(), 1);
            let last = if edge { press } else { (28, 28) };
            assert_eq!(state.city.text_overlay_at(last.0, last.1), Some(0));
            assert_eq!(state.city.city_funds(), 100);
            assert_eq!(state.pe_process_crt.seed(), crt, "{kind:?} edge={edge}");
            assert_eq!(state.mac_process_rng.seed(), mac_seed);
            assert_eq!(
                win95_terrain_plane_fingerprints(&state.city),
                expected,
                "{kind:?} edge={edge}"
            );
        }
    }
}

#[test]
fn win95_editor_lower_matches_native_held_picks_and_map_edge() {
    const EMPTY: u64 = 0x9c1b_da7f_8c87_2325;
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for (edge, crt, expected) in [
            (
                false,
                1_844_513_277,
                [0xe9bd_25a7_dfbf_c496, 0x7945_4b09_8474_50c9, EMPTY, EMPTY],
            ),
            (
                true,
                1_924_036_713,
                [0x1c94_728c_3a8e_d316, 0x499d_5837_2f2c_7121, EMPTY, EMPTY],
            ),
        ] {
            let mut city = flat_win95_terrain_fixture(kind);
            city.set_city_funds(100);
            let mut state = test_state(city);
            state.mode = AppMode::EditNewMap;
            state.tool = EditTool::LowerTerrain;
            state.pe_process_crt = Win95CrtRand::new(1);
            let mac_seed = state.mac_process_rng.seed();
            state.hover_tile = Some(if edge { (0, 0) } else { (20, 20) });

            assert!(state.begin_edit_map_height_hold());
            assert_eq!(state.pe_process_crt.seed(), 3_884_216_597);
            assert_eq!(state.pe_lower_hold_blit_count, u32::from(!edge));
            if edge {
                state.hover_tile = Some((20, 20));
                assert!(state.tick_edit_map_height_hold(true));
            } else {
                assert!(state.tick_edit_map_height_hold(true));
                let seed = state.pe_process_crt.seed();
                state.hover_tile = None;
                assert!(!state.tick_edit_map_height_hold(true));
                assert_eq!(state.pe_process_crt.seed(), seed);
                state.hover_tile = Some((24, 24));
                assert!(state.tick_edit_map_height_hold(true));
            }
            let calls = if edge { 1 } else { 3 };
            assert_eq!(state.pe_lower_hold_blit_count, calls);
            assert_eq!(state.pe_lower_hold_ui_refresh_count, calls);
            assert_eq!(state.release_left_mouse_drag(false).terrain_tiles, 1);
            assert!(!state.tick_edit_map_height_hold(true));
            assert_eq!(state.city.city_funds(), 100);
            assert_eq!(state.pe_process_crt.seed(), crt, "{kind:?} edge={edge}");
            assert_eq!(state.mac_process_rng.seed(), mac_seed);
            assert_eq!(
                win95_terrain_plane_fingerprints(&state.city),
                expected,
                "{kind:?} edge={edge}"
            );
        }
    }
}

fn flat_win95_terrain_fixture(kind: CityFileKind) -> City {
    let mut city = City::new_blank("/tmp/citygame-pe-raise-routing.sc2", "Raise").unwrap();
    city.kind = kind;
    city.set_mac_selected_water_height(0);
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            city.set_altitude(x, y, 10, 0);
        }
    }
    city
}

fn win95_terrain_plane_fingerprints(city: &City) -> [u64; 4] {
    let mut altm = 0xcbf2_9ce4_8422_2325u64;
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let sample = city.altm_at(x, y);
            let word = u16::from(sample.land)
                | u16::from(sample.water) << 5
                | u16::from(sample.tunnel_levels) << 10;
            for byte in word.to_le_bytes() {
                altm = (altm ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    [
        altm,
        pe_density_plane_fingerprint(1, |x, y| city.terrain_at(x, y)),
        pe_density_plane_fingerprint(1, |x, y| city.building_at(x, y)),
        pe_density_plane_fingerprint(1, |x, y| city.tile_bits_at(x, y).unwrap().raw_bits()),
    ]
}

#[test]
fn pe_raise_land_inc_443c60_with_crt_destroys_via_447490() {
    // PE `0x443c60`→`0x447490`: process CRT rubble when XBLD `>= 0x0d`.
    let mut city = City::new_blank("/tmp/sc2000-443c60-crt.sc2", "RaiseCrt").expect("blank");
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_altitude(x, y, 0x10, 0);
        }
    }
    city.set_building(20, 20, 0x70);
    city.set_city_funds(0x19);
    let mut crt = Win95CrtRand::new(1);
    let mut preview = crt;
    let expected = expected_single_tile_mode1_rubble_after_generic_scatter(&mut preview);
    assert!(pe_raise_land_inc_443c60_with_crt(&mut city, Some(&mut crt), 20, 20, 2).is_some());
    assert_eq!(city.building_at(20, 20), expected);
}

#[test]
fn pe_raise_land_inc_443c60_bills_4dc148_before_land_inc() {
    // PE `0x44432e`: `[0x4ca444] < [0x4dc148]` skips land++; else debit 0x19.
    let mut city = City::new_blank("/tmp/sc2000-raise-fund.sc2", "RaiseFund").expect("blank");
    city.set_altitude(20, 20, BLANK_MAP_BASE_HEIGHT, 0);
    assert_eq!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2), Some(0));
    assert_eq!(city.altitude_at(20, 20), BLANK_MAP_BASE_HEIGHT);
    assert_eq!(city.city_funds(), 0);
    city.set_city_funds(0x19);
    assert!(pe_raise_land_inc_443c60(&mut city, 20, 20, 2).is_some());
    assert_eq!(city.altitude_at(20, 20), BLANK_MAP_BASE_HEIGHT + 1);
    assert_eq!(city.city_funds(), 0);
}

#[test]
fn pe_lower_444440_mode_cost_table_matches_4dc140() {
    // PE `0x4dc140` dig costs indexed by Raise submode; unknown → -1.
    assert_eq!(pe_lower_444440_mode_cost(0), 1);
    assert_eq!(pe_lower_444440_mode_cost(1), 0x19);
    assert_eq!(pe_lower_444440_mode_cost(2), 0x19);
    assert_eq!(pe_lower_444440_mode_cost(3), 0x19);
    assert_eq!(pe_lower_444440_mode_cost(4), 1);
    assert_eq!(pe_lower_444440_mode_cost(5), -1);
    // Mode 0 still digs with unknown Raise submode (funds path skipped).
    let mut city = City::new_blank("/tmp/sc2000-dig-cost-table.sc2", "DigCost").expect("blank");
    city.set_altitude(20, 20, 4, 0);
    city.set_city_funds(0);
    assert!(pe_lower_terrain_land_dec_444440_mode_with_submode(
        &mut city, 20, 20, 0, 5
    ));
    // The 0x447490 tail classifies slopes; this blank-map case takes the native
    // 0x32 second land++ arm, returning the center to its starting height.
    assert_eq!(city.altitude_at(20, 20), 7);
    assert_eq!(city.city_funds(), 0);
}

#[test]
fn pe_terrain_coordinate_queue_overflow_drops_oldest_entry() {
    let mut queue = PeTerrainCoordinateQueue::new();
    for n in 0..512i16 {
        queue.push((n, -n));
    }
    assert_eq!(queue.read, 1);
    assert_eq!(queue.write, 0);
    for n in 1..512i16 {
        assert_eq!(queue.pop(), Some((n, -n)));
    }
    assert!(queue.is_empty());
    assert_eq!(queue.pop(), None);

    // Both cursors wrapped; subsequent writes still preserve FIFO order.
    queue.push((512, -512));
    queue.push((513, -513));
    assert_eq!(queue.pop(), Some((512, -512)));
    assert_eq!(queue.pop(), Some((513, -513)));
    assert!(queue.is_empty());
}

#[test]
fn pe_lower_444440_terrain_fixture_overflows_native_ring() {
    // Deterministic rugged terrain reaches the worker's 511-pending-entry limit;
    // the full-worker probe guards integration of the already tested ring rule.
    let mut city =
        City::new_blank("/tmp/sc2000-lower-ring-overflow.sc2", "LowerOverflow").expect("blank");
    let mut terrain_rng = Win95CrtRand::new(33);
    let heights = [3, 7, 12, 20, 28, 31];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let land = heights[terrain_rng.random_u16() as usize % heights.len()];
            city.set_altitude(x, y, land, 0);
        }
    }

    let mut worker_crt = Win95CrtRand::new(1);
    let effects = pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt_effects(
        &mut city,
        Some(&mut worker_crt),
        64,
        64,
        0,
        PE_RAISE_SUBMODE_DEFAULT,
    )
    .expect("lower succeeds");
    assert_eq!(effects.queue_dropped_oldest, 7);
}

#[test]
fn edit_map_spread_terrain_tool_calls_nesw_land_copy() {
    // PE tool-4 EditTool bind: selected SpreadTerrain applies `0x442a30`.
    let mut city = City::new_blank("/tmp/sc2000-spread-tool.sc2", "SpreadTool").expect("blank");
    for y in 15..26 {
        for x in 15..26 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(20, 19, 4, 0);
    city.set_altitude(21, 20, 4, 0);
    city.set_altitude(20, 21, 4, 0);
    city.set_altitude(19, 20, 4, 0);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::SpreadTerrain;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.altitude_at(20, 19), 9);
    assert_eq!(state.city.altitude_at(21, 20), 9);
    assert_eq!(state.city.altitude_at(20, 21), 9);
    assert_eq!(state.city.altitude_at(19, 20), 9);
}

#[test]
fn city_spread_terrain_tool_debits_and_copies_nesw() {
    let mut city = City::new_blank("/tmp/sc2000-spread-city.sc2", "SpreadCity").expect("blank");
    city.set_city_funds(10_000);
    for y in 15..26 {
        for x in 15..26 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(20, 19, 4, 0);
    let mut state = test_state(city);
    state.mode = AppMode::CityView;
    state.tool = EditTool::SpreadTerrain;
    let funds_before = state.city.city_funds();
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.altitude_at(20, 19), 9);
    assert_eq!(state.city.city_funds(), funds_before - 25);
}

#[test]
fn lower_terrain_land_dec_refuses_land_zero() {
    // PE `0x444440` `and land,0x1f` / `je` refuse.
    let mut city = City::new_blank("/tmp/sc2000-lower-zero.sc2", "LowerZero").expect("blank");
    city.set_altitude(20, 20, 0, 0);
    assert!(!pe_lower_terrain_land_dec_444440(&mut city, 20, 20));
    assert_eq!(city.altitude_at(20, 20), 0);
}

#[test]
fn pe_lower_crt_jitter_onto_land_zero_refuses() {
    // PE `0x443900`: CRT may jitter onto land 0; `0x444440` then refuses.
    let mut city = City::new_blank("/tmp/sc2000-lower-jit0.sc2", "LowerJit0").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 6, 0);
        }
    }
    // Seed 12 → dx=1,dy=0 → (21,20).
    city.set_altitude(21, 20, 0, 0);
    let mut crt = Win95CrtRand::new(12);
    assert_eq!(
        pe_lower_terrain_crt_step_443900(&mut city, &mut crt, 20, 20),
        Some(false)
    );
    assert_eq!(city.altitude_at(20, 20), 6);
    assert_eq!(city.altitude_at(21, 20), 0);
}

#[test]
fn lower_terrain_crt_jitter_relaxes_equal_height_neighbors() {
    // PE `0x443900`: seed 12 → dx=1,dy=0; mask 0x0f lowers one-level neighbors too.
    let mut city = City::new_blank("/tmp/sc2000-lower-jit.sc2", "LowerJit").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 6, 0);
        }
    }
    let mut crt = Win95CrtRand::new(12);
    assert_eq!(
        pe_lower_terrain_crt_step_443900(&mut city, &mut crt, 20, 20),
        Some(true)
    );
    assert_eq!(city.altitude_at(20, 20), 5);
    assert_eq!(city.altitude_at(21, 20), 5);
}

#[test]
fn edit_map_level_click_copies_height_to_cardinal_neighbors() {
    // 0x442bae copies the press height before refreshing the first neighbor.
    let mut city = City::new_blank("/tmp/sc2000-emap-level-445b50.sc2", "ELevel").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    city.set_altitude(20, 20, 8, 0);
    city.set_altitude(20, 19, 4, 0);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::LevelTerrain;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.altitude_at(20, 20), 8);
    assert_eq!(state.city.altitude_at(20, 19), 8);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
}

#[test]
fn mac_edit_map_done_persists_gameplay_run_state_for_both_city_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-editor-done.sc2", "MacDone").expect("blank city");
        city.kind = kind;
        city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, 0);
        city.set_mac_state_misc_u32_at(0x78, 0x1234_5678);
        city.set_traffic_density(12, 34, 63);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::EditNewMap;
        state.show_traffic = false;
        let quickdraw_seed = state.mac_process_rng.seed();

        state.finish_edit_map_state();

        assert_eq!(state.mode, AppMode::CityView);
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(1),
            "{kind:?}"
        );
        assert_eq!(state.city.mac_state_misc_u32_at(0x78), Some(0x1234_5678));
        assert_eq!(state.city.traffic_density_at(12, 34), Some(63));
        assert!(state.show_traffic);
        assert_eq!(state.mac_process_rng.seed(), quickdraw_seed);

        // A PE snapshot publisher is not an owner of this Mac run-state.
        state
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, 2);
        persist_pe_disaster_misc(&mut state.city, None, state.mode, state.engine_fidelity);
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(2)
        );
    }
}

#[test]
fn mac_developed_tile_anchor_matches_68k_threshold_and_division() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(mac_developed_tile_anchor(&city), (0, 0));

    // Infrastructure and developed-looking IDs below 0x70 do not enter the
    // Mac center scan. One accepted cell uses the initialized divisor of one
    // and the final doubled quotient: (11,21) therefore becomes (4,10).
    city.set_building(50, 40, 0x6f);
    city.set_building(11, 21, 0x70);
    assert_eq!(mac_developed_tile_anchor(&city), (4, 10));

    // With two accepted cells, divide by 2*(N+1) before doubling the quotient.
    city.set_building(20, 20, 0xc6);
    assert_eq!(mac_developed_tile_anchor(&city), (10, 12));
}

#[test]
fn pe_fun_00460e10_refuses_water_tile_without_paint() {
    // PE @ `0x460e6b`: XBIT bit `0x04` → return 0 before fire paint.
    let mut city = City::new_blank("/tmp/sc2000-ignite-water.sc2", "IgniteWater").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    city.set_text_overlay(20, 20, 0);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    pe_fun_00460e10_ignite(&mut city, Some(&mut crt), &mut lfsr, 20, 20, 0);
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert_eq!(city.building_at(20, 20), 0x70);
}

#[test]
fn pe_fb_downhill_dir_picks_strictly_lower_nesw() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let src_alt = city.altitude_at(40, 40) & 0x1f;
    city.set_altitude(41, 40, src_alt.saturating_add(1), 0);
    city.set_altitude(39, 40, src_alt, 0);
    city.set_altitude(40, 39, src_alt.saturating_sub(1), 0);
    city.set_altitude(40, 41, src_alt, 0);
    assert_eq!(pe_fb_downhill_dir(&city, 40, 40), Some(0));
    city.set_altitude(40, 39, src_alt, 0);
    city.set_altitude(41, 40, src_alt.saturating_sub(2), 0);
    assert_eq!(pe_fb_downhill_dir(&city, 40, 40), Some(1));
}

#[test]
fn pe_fb_tick_recedes_on_water_one_in_sixteen() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for x in 20..52 {
        city.set_text_overlay(x, 40, 0xfb);
        city.set_tile_bits(x, 40, crate::city::TileBits::new(0b0000_0100));
    }
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x04);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, _) = preview_pe_fb_tick(&state, &mut preview, &mut preview_lfsr);
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
fn citytool_water_pump_uses_place_tile_size_then_place_pipes() {
    // PE CityTool PlaceTile-size then PlacePipesAtCoordinates (`0x44087b`→`0x44b790`):
    // vacant XUND → `0x10`, OR piped (`0x44b829`).
    let mut city = City::new_blank("/tmp/sc2000-ct-pump.sc2", "CtPump").expect("blank");
    city.set_city_funds(20_000);
    city.set_traffic_density(33, 33, 0x19);
    let mut state = test_state(city);
    state.tool = EditTool::WaterPump;
    assert_eq!(state.place_building_at(33, 33, 220), 1);
    assert_eq!(state.city.building_at(33, 33), 220);
    assert_eq!(state.city.underground_at(33, 33), UNDER_TILE_PIPES_LR);
    let bits = state.city.tile_bits_at(33, 33).unwrap();
    assert!(bits.piped());
    assert_eq!(bits.raw_bits() & 0xe0, 0xa0);
    assert_eq!(
        state.city.zone_at(33, 33).unwrap().raw_bits(),
        ZONE_CORNER_ALL
    );
    assert_eq!(state.city.traffic_density_at(33, 33), Some(0x19));
    // Water pump has no microsim class (`0x44cf60` / provision None).
    assert_eq!(state.city.text_overlay_at(33, 33).unwrap_or(0), 0);
}

#[test]
fn maptool_sub_to_rail_refuses_nonflat_xter() {
    // PE @ `0x441bce`–`0x441bd2`: XTER != 0 → return 0 before adjacency.
    let mut city = City::new_blank("/tmp/sc2000-str-xter.sc2", "StrXter").expect("blank");
    city.set_city_funds(20_000);
    city.set_terrain(20, 20, 1);
    city.set_building(21, 20, 0x2c);
    let mut state = test_state(city);
    state.tool = EditTool::SubToRail;
    assert!(state.sub_to_rail_plan_at(20, 20).is_none());
    assert!(!state.place_sub_to_rail_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.building_at(21, 20), 0x2c);

    // Control: flat XTER 0 with same adjacency plans.
    state.city.set_terrain(20, 20, 0);
    assert!(state.sub_to_rail_plan_at(20, 20).is_some());
}

#[test]
fn citytool_place_refuses_water_tile_xbit_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440db8`: non-marina preview/place refuse water-tile bit.
    let mut city = City::new_blank("/tmp/sc2000-ct-wetbit.sc2", "CtWetBit").expect("blank");
    city.set_city_funds(20_000);
    for dx in 0..3usize {
        for dy in 0..3usize {
            city.set_tile_bits(40 + dx, 40 + dy, TileBits::new(0).with_water_tile(true));
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
fn citytool_small_park_overwrites_trees_like_place_tile_size() {
    // PE PlaceTile-size park arm @ `0x440e8f`: existing XBLD `<0x0D` (trees) may overwrite.
    let mut city = City::new_blank("/tmp/sc2000-ct-park-tree.sc2", "CtPkTree").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(45, 45, 0x09);
    let mut state = test_state(city);
    state.tool = EditTool::SmallPark;
    assert_eq!(state.place_building_at(45, 45, SMALL_PARK_BUILDING_ID), 1);
    assert_eq!(state.city.building_at(45, 45), SMALL_PARK_BUILDING_ID);
}

#[test]
fn citytool_place_refuses_nonzero_xter_like_place_tile_size() {
    // PE PlaceTile-size @ `0x440d89`: non-marina can_place refuses any XTER ≠ 0.
    let mut city = City::new_blank("/tmp/sc2000-ct-xter.sc2", "CtXter").expect("blank");
    city.set_city_funds(20_000);
    city.set_rewards_available_mask(reward_mask_for_tool(EditTool::Statue).unwrap_or(0));
    city.set_terrain(45, 45, 0x01);
    let mut state = test_state(city);
    state.tool = EditTool::Statue;
    let place = state.building_placement_at(45, 45, 219);
    assert!(place.valid_tiles.is_empty());
    assert_eq!(place.invalid_tiles, vec![(45, 45)]);
    assert_eq!(state.place_building_at(45, 45, 219), 0);
    assert_eq!(state.city.building_at(45, 45), 0);
    assert_eq!(state.city.terrain_at(45, 45), 0x01);
}

#[test]
fn pe_fun_00447500_tree_place_tile_ages_and_plants() {
    // PE @ `0x4475f4`..`0x447629`: vacant → 6/7; mid trees ++s; 0x0B/0x0C → 0x0B/0x0C.
    let mut city = City::new_blank("/tmp/sc2000-tree-fun.sc2", "TreeFun").expect("blank");
    city.set_text_overlay(20, 20, 0x44);
    city.set_traffic_density(20, 20, 0x12);
    city.set_tile_bits(20, 20, TileBits::new(0).with_watered(true));
    let mut rng = Win95CrtRand::new(1);
    assert!(pe_fun_00447500_tree_place_tile(&mut city, &mut rng, 20, 20));
    let planted = city.building_at(20, 20);
    assert!(planted == 6 || planted == 7);
    assert_eq!(city.text_overlay_at(20, 20), Some(0x44));
    assert_eq!(city.traffic_density_at(20, 20), Some(0x12));
    assert!(city.tile_bits_at(20, 20).unwrap().watered());

    city.set_building(21, 21, 0x09);
    assert!(pe_fun_00447500_tree_place_tile(&mut city, &mut rng, 21, 21));
    assert_eq!(city.building_at(21, 21), 0x0a);

    city.set_building(22, 22, 0x0b);
    assert!(pe_fun_00447500_tree_place_tile(&mut city, &mut rng, 22, 22));
    let dense = city.building_at(22, 22);
    assert!(dense == 0x0b || dense == 0x0c);

    city.set_building(23, 23, 0x0d);
    assert!(!pe_fun_00447500_tree_place_tile(
        &mut city, &mut rng, 23, 23
    ));
    assert_eq!(city.building_at(23, 23), 0x0d);

    city.set_building(24, 24, 0);
    city.set_tile_bits(24, 24, TileBits::new(0).with_water_tile(true));
    assert!(!pe_fun_00447500_tree_place_tile(
        &mut city, &mut rng, 24, 24
    ));
    assert_eq!(city.building_at(24, 24), 0);
}

#[test]
fn pe_fun_00447500_scatter_plants_some_trees() {
    // PE `0x447500` outer count + inner `rand()&0x3f` jittered PlaceTile tree attempts.
    let mut city = City::new_blank("/tmp/sc2000-tree-scatter.sc2", "TreeScat").expect("blank");
    let mut rng = Win95CrtRand::new(0x1234_5678);
    pe_fun_00447500(&mut city, &mut rng, 8);
    let trees = (0..CITY_WIDTH)
        .flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)))
        .filter(|&(x, y)| (6..=0x0c).contains(&city.building_at(x, y)))
        .count();
    assert!(trees > 0, "scatter should PlaceTile some trees");
}

#[test]
fn pe_fun_00447500_new_city_tree_count_is_square_over_two() {
    // PE new-city @ `0x410c9a`: `imul`/`sar 1` → `(n*n)/2` before FUN_00447500.
    assert_eq!(pe_fun_00447500_new_city_tree_count(0), 0);
    assert_eq!(pe_fun_00447500_new_city_tree_count(1), 0);
    assert_eq!(pe_fun_00447500_new_city_tree_count(2), 2);
    assert_eq!(pe_fun_00447500_new_city_tree_count(3), 4);
    assert_eq!(pe_fun_00447500_new_city_tree_count(6), 18);
    assert_eq!(pe_fun_00447500_new_city_tree_count(10), 50);
}

#[test]
fn pe_place_tile_size_2x2_ors_rotation_corner_bits() {
    // PE size>1 epilogue @ `0x441030`: OR `0x4dc4d0` rotation corners; no zone type.
    let mut city = City::new_blank("/tmp/sc2000-pts-2x2.sc2", "Pts2x2").expect("blank");
    city.set_view_rotation(0);
    for dx in 0..2usize {
        for dy in 0..2usize {
            city.set_zone(20 + dx, 20 + dy, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
            city.set_tile_bits(20 + dx, 20 + dy, TileBits::new(0).with_watered(true));
        }
    }
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 0xe0, 2));
    let corner_bits = pe_choose_and_place_corner_bits(0);
    for dx in 0..2usize {
        for dy in 0..2usize {
            assert_eq!(city.building_at(20 + dx, 20 + dy), 0xe0);
            let expect = building_zone_bits_for_tile(20, 20, 20 + dx, 20 + dy, 2, corner_bits);
            assert_eq!(city.zone_at(20 + dx, 20 + dy).unwrap().raw_bits(), expect);
            let bits = city.tile_bits_at(20 + dx, 20 + dy).unwrap();
            assert!(bits.watered() && bits.piped() && bits.powered() && bits.powerable());
        }
    }
}

#[test]
fn pe_place_tile_size_1x1_allows_map_edge_0_through_0x7f() {
    // PE @ `0x440ce7`–`0x440d08`: size-1 preflight is `0 <= x,y < 0x80` (not `1..=0x7e`).
    let mut city = City::new_blank("/tmp/sc2000-pts-1x1-edge.sc2", "Pts1Edge").expect("blank");
    assert!(pe_place_tile_size_440c50(&mut city, 0, 20, 0xc8, 1));
    assert_eq!(city.building_at(0, 20), 0xc8);
    assert!(pe_place_tile_size_440c50(&mut city, 0x7f, 20, 0xc8, 1));
    assert_eq!(city.building_at(0x7f, 20), 0xc8);
    // Contrast: size-2 at those origins still refuses the multi-tile edge gate.
    assert!(!pe_place_tile_size_440c50(&mut city, 0, 30, 0xe0, 2));
    assert!(!pe_place_tile_size_440c50(&mut city, 0x7e, 30, 0xe0, 2));
}

#[test]
fn citytool_post_place_water_recalc_44094e_clears_stale_watered_when_pop_low() {
    // PE @ `0x44094e`–`0x44098d` → `FUN_00464550`: drop watered except XBLD 0xeb.
    let mut city = City::new_blank("/tmp/sc2000-ct-wat-low.sc2", "CtWatLow").expect("blank");
    city.set_city_funds(50_000);
    city.set_tile_bits(40, 40, TileBits::new(0).with_watered(true));
    city.set_building(41, 41, 0xeb);
    city.set_tile_bits(41, 41, TileBits::new(0).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::Police;
    assert!(state.place_building_at(20, 20, 210) > 0);
    assert!(state.city.tile_bits_at(20, 20).unwrap().piped());
    assert!(
        !state.city.tile_bits_at(40, 40).unwrap().watered(),
        "FUN_00464550 should clear stale XBIT 0x10"
    );
    assert!(
        state.city.tile_bits_at(41, 41).unwrap().watered(),
        "XBLD 0xeb keeps watered through &= 0xef skip"
    );
}

#[test]
fn citytool_post_place_water_recalc_44094e_skips_when_pop_ge_0xc350() {
    let mut city = City::new_blank("/tmp/sc2000-ct-wat-hi.sc2", "CtWatHi").expect("blank");
    city.set_city_funds(50_000);
    city.persist_pe_day22_population_leftover(0xc350);
    city.set_tile_bits(40, 40, TileBits::new(0).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::Police;
    assert!(state.place_building_at(20, 20, 210) > 0);
    assert!(
        state.city.tile_bits_at(40, 40).unwrap().watered(),
        "pop >= 0xc350 skips FUN_00464550"
    );
}

#[test]
fn citytool_post_place_water_recalc_44094e_skips_when_piped_clear() {
    let mut city = City::new_blank("/tmp/sc2000-ct-wat-nopipe.sc2", "CtWatNo").expect("blank");
    city.set_tile_bits(20, 20, TileBits::new(0).with_powerable(true));
    city.set_tile_bits(40, 40, TileBits::new(0).with_watered(true));
    pe_citytool_post_place_water_recalc_44094e(&mut city, 20, 20);
    assert!(
        city.tile_bits_at(40, 40).unwrap().watered(),
        "piped-clear click skips FUN_00464550"
    );
}

#[test]
fn pipetool_post_place_water_recalc_43f419_clears_stale_watered_when_pop_low() {
    // PE @ `0x43f419`–`0x43f42f` → `FUN_00464550`: pop-only gate after pipe place.
    let mut city = City::new_blank("/tmp/sc2000-pipe-wat-low.sc2", "PiWatLow").expect("blank");
    city.set_city_funds(50_000);
    city.set_tile_bits(40, 40, TileBits::new(0).with_watered(true));
    city.set_building(41, 41, 0xeb);
    city.set_tile_bits(41, 41, TileBits::new(0).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::Pipes;
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        1
    );
    assert!(
        !state.city.tile_bits_at(40, 40).unwrap().watered(),
        "FUN_00464550 should clear stale XBIT 0x10"
    );
    assert!(
        state.city.tile_bits_at(41, 41).unwrap().watered(),
        "XBLD 0xeb keeps watered through &= 0xef skip"
    );
}

#[test]
fn pipetool_post_place_water_recalc_43f419_skips_when_pop_ge_0xc350() {
    let mut city = City::new_blank("/tmp/sc2000-pipe-wat-hi.sc2", "PiWatHi").expect("blank");
    city.set_city_funds(50_000);
    city.persist_pe_day22_population_leftover(0xc350);
    city.set_tile_bits(40, 40, TileBits::new(0).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::Pipes;
    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        1
    );
    assert!(
        state.city.tile_bits_at(40, 40).unwrap().watered(),
        "pop >= 0xc350 skips FUN_00464550"
    );
}

#[test]
fn mac_pipetool_post_place_uses_export209_below_weighted_growth_gate_on_both_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/sc2000-mac-pipe-water-callback.sc2", "MacPipeWater")
            .expect("blank city");
        city.kind = kind;
        city.set_city_funds(50_000);
        city.set_mac_state_misc_u32_at(0x102c, 0xc34f);
        city.set_mac_treatment_gate(0);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.tool = EditTool::Pipes;

        assert_eq!(
            state.place_underground_at(20, 20, UndergroundKind::Pipes),
            1
        );
        assert_eq!(
            state.city.mac_treatment_gate(),
            1,
            "Mac export 209 publishes the treatment gate for {kind:?}"
        );
    }
}

#[test]
fn mac_pipetool_post_place_skips_export209_at_weighted_growth_threshold() {
    let mut city = City::new_blank("/tmp/sc2000-mac-pipe-water-threshold.sc2", "MacPipeLimit")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(50_000);
    city.set_mac_state_misc_u32_at(0x102c, 0xc350);
    city.set_mac_treatment_gate(0);
    city.set_tile_bits(40, 40, TileBits::new(0).with_watered(true));
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Pipes;

    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        1
    );
    assert_eq!(state.city.mac_treatment_gate(), 0);
    assert!(state.city.tile_bits_at(40, 40).unwrap().watered());
}

#[test]
fn pe_place_tile_size_overwrites_trees_for_non_park() {
    // General preflight allows trees 6..=0x0C; non-park PlaceTile-size overwrites them.
    let mut city = City::new_blank("/tmp/sc2000-pts-trees.sc2", "PtsTrees").expect("blank");
    city.set_building(20, 20, 0x09);
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 0xc8, 1));
    assert_eq!(city.building_at(20, 20), 0xc8);
}

#[test]
fn pe_place_tile_size_refuses_water_tile_xbit() {
    // PE @ `0x440db8`: non-0xF8 place refuses XBIT water-tile.
    let mut city = City::new_blank("/tmp/sc2000-pts-wet.sc2", "PtsWet").expect("blank");
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0xc8, 1));
    assert_eq!(city.building_at(20, 20), 0);
}

#[test]
fn pe_place_tile_size_marina_refuses_zero_or_nine_water_tiles() {
    // PE @ `0x440e3e`..`0x440e47`: 0xF8 refuses water-count 0 or 9 (dialog skipped).
    let mut dry = City::new_blank("/tmp/sc2000-pts-f8-dry.sc2", "PtsF8Dry").expect("blank");
    assert!(!pe_place_tile_size_440c50(&mut dry, 20, 20, 0xf8, 3));
    assert_eq!(dry.building_at(20, 20), 0);

    let mut full = City::new_blank("/tmp/sc2000-pts-f8-full.sc2", "PtsF8Full").expect("blank");
    for &(tx, ty) in &building_footprint_from_origin(20, 20, 3).expect("3x3") {
        full.set_tile_bits(tx, ty, TileBits::new(0).with_water_tile(true));
    }
    assert!(!pe_place_tile_size_440c50(&mut full, 20, 20, 0xf8, 3));
    assert_eq!(full.building_at(20, 20), 0);
}

#[test]
fn pe_place_tile_size_marina_refuses_dry_cell_nonzero_xter() {
    // PE @ `0x440d55`..`0x440d81`: marina dry cells require XTER 0.
    let mut city = City::new_blank("/tmp/sc2000-pts-f8-xter.sc2", "PtsF8Xter").expect("blank");
    for &(tx, ty) in &building_footprint_from_origin(20, 20, 3).expect("3x3") {
        city.set_tile_bits(tx, ty, TileBits::new(0).with_water_tile(true));
    }
    // One dry cell with bare water XTER (no water-tile) → refuse.
    city.set_tile_bits(22, 22, TileBits::new(0));
    city.set_terrain(22, 22, 0x30);
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0xf8, 3));
    assert_eq!(city.building_at(20, 20), 0);
}

#[test]
fn pe_place_tile_size_marina_allows_mixed_water_count() {
    // PE marina gate: water-count in 1..=8 proceeds to stamp (no dialog).
    let mut city = City::new_blank("/tmp/sc2000-pts-f8-mix.sc2", "PtsF8Mix").expect("blank");
    for x in 20..23 {
        city.set_tile_bits(x, 20, TileBits::new(0).with_water_tile(true));
    }
    city.set_view_rotation(0);
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 0xf8, 3));
    let corner_bits = pe_choose_and_place_corner_bits(0);
    for &(tx, ty) in &building_footprint_from_origin(20, 20, 3).expect("3x3") {
        assert_eq!(city.building_at(tx, ty), 0xf8);
        let expect = building_zone_bits_for_tile(20, 20, tx, ty, 3, corner_bits);
        assert_eq!(city.zone_at(tx, ty).unwrap().raw_bits(), expect);
    }
}

#[test]
fn pe_army_crane_altm_gate_requires_water_ge_land_plus_two() {
    // PE @ `0x438776`: refuse when water < land+2 (sampled on ray tile).
    let mut city =
        City::new_blank("/tmp/sc2000-army-crane-altm.sc2", "ArmyCraneAltm").expect("blank");
    city.set_altitude(20, 20, 5, 6);
    assert!(!pe_army_crane_altm_water_deep_enough(&city, 20, 20));

    city.set_altitude(20, 20, 5, 7);
    assert!(pe_army_crane_altm_water_deep_enough(&city, 20, 20));
}

#[test]
fn pe_army_crane_place_tile_size_altm_ok_but_water_tile_refuses_stamp() {
    // PE still calls PlaceTile-size on water-tile XBIT; that helper refuses non-marina.
    let mut city =
        City::new_blank("/tmp/sc2000-army-crane-wet.sc2", "ArmyCraneWet").expect("blank");
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    pe_army_crane_place_tile_size_4387e7(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
}

#[test]
fn stamp_zone_development_military_crane_uses_army_altm_place() {
    // PE SimulationGrow type-7 allow: crane PlaceTile-size + zone/XBIT + strip.
    let mut city =
        City::new_blank("/tmp/sc2000-stamp-army-crane.sc2", "StampArmyCrane").expect("blank");
    city.set_zone(24, 24, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_tile_bits(
        24,
        24,
        TileBits::new(0x01).with_watered(true).with_piped(true),
    );
    // South ray: five vacant XZON&4 tiles; ALTM gate on the fifth (`0x438776`).
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
    assert_eq!(
        state.city.zone_at(24, 24).unwrap().raw_bits(),
        ZONE_CORNER_ALL | ZONE_MILITARY
    );
    assert_eq!(
        state.city.tile_bits_at(24, 24).unwrap().raw_bits() & 0xf0,
        0
    );
    for y in 25..29 {
        assert_eq!(state.city.building_at(24, y), 0xdf);
    }
}

#[test]
fn pe_army_crane_ray_altm_gate_requires_five_vacant_and_deep_fifth() {
    let mut city = City::new_blank("/tmp/sc2000-crane-ray.sc2", "CraneRay").expect("blank");
    city.set_zone(24, 24, ZONE_MILITARY);
    for y in 25..=29 {
        city.set_zone(24, y, ZONE_MILITARY);
        city.set_tile_bits(24, y, TileBits::new(0).with_water_tile(true));
    }
    city.set_altitude(24, 29, 5, 6);
    assert!(!pe_army_crane_ray_altm_gate_4386e0(&city, 24, 24, 0));
    city.set_altitude(24, 29, 5, 7);
    assert!(pe_army_crane_ray_altm_gate_4386e0(&city, 24, 24, 0));
    city.set_building(24, 27, 1);
    assert!(!pe_army_crane_ray_altm_gate_4386e0(&city, 24, 24, 0));
}

#[test]
fn pe_army_place_tile_size_over_trees_writes_zone() {
    // PE @ `0x43893f`: trees (<0x0D) → PlaceTile-size then type-nibble zone write.
    let mut city = City::new_blank("/tmp/sc2000-army-pts-tree.sc2", "ArmyPtsTree").expect("blank");
    city.set_building(26, 26, 0x0a);
    city.set_zone(26, 26, 0x40);
    city.set_tile_bits(26, 26, TileBits::new(0).with_watered(true));
    city.set_traffic_density(26, 26, 0x19);
    assert_eq!(
        pe_army_place_tile_size_if_xbld_below_0x0d_43893f(&mut city, 26, 26, 0xe0, 7),
        PeArmySize1Result::Accepted { placed: true }
    );
    assert_eq!(city.building_at(26, 26), 0xe0);
    assert_eq!(
        city.zone_at(26, 26).unwrap().raw_bits(),
        ZONE_CORNER_ALL | ZONE_MILITARY
    );
    assert_eq!(city.tile_bits_at(26, 26).unwrap().raw_bits() & 0xf0, 0);
    assert_eq!(city.traffic_density_at(26, 26), Some(0x19));
}

#[test]
fn pe_place_tile_size_refuses_nonzero_xter() {
    // PE @ `0x440d89` / `0x440d95`: XTER ≠ 0 refuses before stamp.
    let mut city = City::new_blank("/tmp/sc2000-pts-xter.sc2", "PtsXter").expect("blank");
    city.set_terrain(20, 20, 0x01);
    assert!(!pe_place_tile_size_440c50(&mut city, 20, 20, 0xc8, 1));
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.terrain_at(20, 20), 0x01);
}

#[test]
fn pe_place_tile_size_park_allows_existing_trees() {
    // PE park place @ `0x440e8f`: existing XBLD < 0x0D is OK (trees 6..=0x0C).
    let mut city = City::new_blank("/tmp/sc2000-pts-park-tree.sc2", "PtsPTree").expect("blank");
    city.set_building(20, 20, 0x0a);
    city.set_tile_bits(20, 20, TileBits::new(0).with_watered(true));
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 0x0d, 1));
    assert_eq!(city.building_at(20, 20), 0x0d);
    assert!(city.tile_bits_at(20, 20).unwrap().piped());
    assert!(city.tile_bits_at(20, 20).unwrap().watered());
}

#[test]
fn place_zone_rect_over_tree_leaves_xbld() {
    // PE PlaceTile(0) only for XBLD 1..=4 — trees stay under the new zone.
    let mut city = City::new_blank("/tmp/sc2000-zone-tree.sc2", "ZoneTree").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(40, 40, 6);
    city.set_text_overlay(40, 40, 0x11);
    let mut state = test_state(city);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        1
    );
    assert_eq!(state.city.building_at(40, 40), 6);
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0x11));
}

#[test]
fn mac68k_place_zone_rect_over_tree_leaves_xbld() {
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-tree.sc2", "MacZoneTree").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(40, 40, 0x0c);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_COMMERCIAL_DENSE),
        1
    );
    assert_eq!(state.city.building_at(40, 40), 0x0c);
}

#[test]
fn place_zone_rect_refuses_xbit_water_tile() {
    // PE @ 0x449997: test XBIT 0x04 → skip tile.
    let mut city = City::new_blank("/tmp/sc2000-zone-wtile.sc2", "ZoneWTile").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(40, 40, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
    assert_eq!(state.city.zone_at(40, 40).map(|z| z.zone_id()), Some(0));
}

#[test]
fn place_zone_rect_refuses_nonzero_xter() {
    // PE @ 0x4499c6: cmp XTER,0 / jnz skip.
    let mut city = City::new_blank("/tmp/sc2000-zone-xter.sc2", "ZoneXter").expect("blank");
    city.set_city_funds(20_000);
    city.set_terrain(40, 40, 2);
    let mut state = test_state(city);
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
}

#[test]
fn mac68k_place_zone_rect_refuses_nonzero_xter() {
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-xter.sc2", "MacZoneXter").expect("blank");
    city.set_city_funds(20_000);
    city.set_terrain(40, 40, 3);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_RESIDENTIAL_LIGHT),
        0
    );
}

#[test]
fn mac68k_place_zone_rect_refuses_xbit_water_tile() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-zone-wtile.sc2", "MacZoneWTile").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(40, 40, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(
        state.place_zone_rect((40, 40), (40, 40), ZONE_INDUSTRIAL_LIGHT),
        0
    );
}

#[test]
fn emergency_tool_preview_uses_xbit_water_and_marker_gate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 100, 100, 211, 3);
    // The native dispatch input gate ignores XTER and XBLD: only surface-water
    // XBIT and an occupied XTXT marker reject the clicked cell.
    city.set_terrain(20, 21, 0x30);
    city.set_tile_bits(22, 23, TileBits::new(0).with_water_tile(true));
    city.set_building(24, 25, 112);
    city.set_text_overlay(26, 27, 0xff);
    let mut state = test_state(city);
    state.tool = EditTool::DispatchFirefighters;

    for (x, y) in [(22, 23), (26, 27)] {
        let placement = state.tool_placement_at(x, y).expect("dispatch preview");
        assert!(placement.valid_tiles.is_empty());
        assert_eq!(placement.invalid_tiles, vec![(x, y)]);
        assert!(!state.apply_tool_at_tile(x, y));
    }

    for (x, y) in [(20, 21), (24, 25)] {
        let placement = state.tool_placement_at(x, y).expect("dispatch preview");
        assert_eq!(placement.valid_tiles, vec![(x, y)]);
        assert!(placement.invalid_tiles.is_empty());
    }
}

#[test]
fn zone_previews_use_zone_sprite_without_generic_valid_diamond() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.tool = EditTool::ZoneIndustrial;

    let placement = state.tool_placement_at(20, 20).expect("zone placement");

    assert_eq!(placement.preview, ToolPreview::Zone(5));
    assert_eq!(placement.preview.zone_sprite_id(), Some(1295));
    assert!(!placement.preview.draws_valid_diamond());
    assert!(!ToolPreview::Building(207).draws_valid_diamond());
    assert!(!ToolPreview::Underground(UndergroundKind::Pipes).draws_valid_diamond());
    assert!(ToolPreview::Network(NetworkKind::Road).draws_valid_diamond());
    assert!(ToolPreview::Zone(0).draws_valid_diamond());
}

#[test]
fn place_sign_preserves_watered_and_xtrf() {
    // PE sign tool: XTXT/XLAB only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-sign-xtrf.sc2", "SignX").expect("blank");
    city.set_tile_bits(10, 10, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(10, 10, 0x5a);
    let mut state = test_state(city);
    state.tool = EditTool::Sign;
    assert!(state.place_sign_at(10, 10));
    let bits = state.city.tile_bits_at(10, 10).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(10, 10), Some(0x5a));
}

#[test]
fn remove_edit_map_tree_preserves_watered_and_xtrf() {
    // Edit-map tree clear: XTER only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-emap-rmtree-xtrf.sc2", "ERmTree").expect("blank");
    city.set_terrain(38, 38, 0x06);
    city.set_tile_bits(38, 38, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(38, 38, 0x33);
    let mut state = test_state(city);
    assert!(state.remove_edit_map_tree_at(38, 38));
    let bits = state.city.tile_bits_at(38, 38).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(38, 38), Some(0x33));
}

#[test]
fn remove_edit_map_tree_preserves_xtxt() {
    // Edit-map tree clear is XTER only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-emap-rmtree-xtxt.sc2", "ERmTreeT").expect("blank");
    city.set_terrain(38, 38, 0x06);
    city.set_text_overlay(38, 38, 0x49);
    let mut state = test_state(city);
    assert!(state.remove_edit_map_tree_at(38, 38));
    assert_eq!(state.city.text_overlay_at(38, 38), Some(0x49));
}

#[test]
fn mac68k_remove_edit_map_tree_preserves_xtxt() {
    // Mac Shift tree clears XBLD through PlaceTile, preserving XTER and XTXT.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-rmtree-xtxt.sc2", "MacERmTreeT").expect("blank");
    city.set_terrain(38, 38, 0x06);
    city.set_building(38, 38, 7);
    city.set_text_overlay(38, 38, 0x49);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.remove_edit_map_tree_at(38, 38));
    assert_eq!(state.city.building_at(38, 38), 0);
    assert_eq!(state.city.terrain_at(38, 38), 0x06);
    assert_eq!(state.city.text_overlay_at(38, 38), Some(0x49));
}

#[test]
fn mac68k_remove_edit_map_tree_preserves_xtrf() {
    // Mac Shift tree clears XBLD, preserving terrain shape and XTRF.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-rmtree-xtrf.sc2", "MacERmT").expect("blank");
    city.set_terrain(38, 38, 0x06);
    city.set_building(38, 38, 7);
    city.set_traffic_density(38, 38, 0x33);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.remove_edit_map_tree_at(38, 38));
    assert_eq!(state.city.building_at(38, 38), 0);
    assert_eq!(state.city.terrain_at(38, 38), 0x06);
    assert_eq!(state.city.traffic_density_at(38, 38), Some(0x33));
}

#[test]
fn remove_edit_map_forest_preserves_watered_and_xtrf() {
    // Edit-map forest clear reuses tree clear — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-emap-rmforest-xtrf.sc2", "ERmFor").expect("blank");
    city.set_terrain(44, 44, 0x06);
    city.set_tile_bits(44, 44, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(44, 44, 0x28);
    let mut state = test_state(city);
    assert!(state.remove_edit_map_forest_at(44, 44) > 0);
    let bits = state.city.tile_bits_at(44, 44).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(44, 44), Some(0x28));
}

#[test]
fn remove_edit_map_forest_preserves_xtxt() {
    // Edit-map forest clear reuses tree clear; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-emap-rmforest-xtxt.sc2", "ERmForT").expect("blank");
    city.set_terrain(44, 44, 0x06);
    city.set_text_overlay(44, 44, 0x4a);
    let mut state = test_state(city);
    assert!(state.remove_edit_map_forest_at(44, 44) > 0);
    assert_eq!(state.city.text_overlay_at(44, 44), Some(0x4a));
}

#[test]
fn mac68k_remove_edit_map_forest_preserves_xtxt() {
    // Mac Shift forest thins XBLD, preserving XTER and XTXT.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-rmforest-xtxt.sc2", "MacERmForT").expect("blank");
    city.set_terrain(44, 44, 0x06);
    city.set_building(44, 44, 7);
    city.set_text_overlay(44, 44, 0x4a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_process_rng = MacToolboxRng::new(0);
    assert!(state.remove_edit_map_forest_at(44, 44) > 0);
    assert_eq!(state.city.building_at(44, 44), 0);
    assert_eq!(state.city.terrain_at(44, 44), 0x06);
    assert_eq!(state.city.text_overlay_at(44, 44), Some(0x4a));
}

#[test]
fn mac68k_remove_edit_map_forest_preserves_xtrf() {
    // Mac Shift forest thins XBLD, preserving terrain shape and XTRF.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-rmforest-xtrf.sc2", "MacERmF").expect("blank");
    city.set_terrain(44, 44, 0x06);
    city.set_building(44, 44, 7);
    city.set_traffic_density(44, 44, 0x28);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_process_rng = MacToolboxRng::new(0);
    assert!(state.remove_edit_map_forest_at(44, 44) > 0);
    assert_eq!(state.city.building_at(44, 44), 0);
    assert_eq!(state.city.terrain_at(44, 44), 0x06);
    assert_eq!(state.city.traffic_density_at(44, 44), Some(0x28));
}

#[test]
fn place_city_terrain_raise_preserves_watered_and_xtrf() {
    // City-view raise: ALTM only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-raise-xtrf.sc2", "RaiseX").expect("blank");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 48, 48, 5);
    city.set_tile_bits(48, 48, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(48, 48, 0x37);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.altitude_at(48, 48), 6);
    let bits = state.city.tile_bits_at(48, 48).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(48, 48), Some(0x37));
}

#[test]
fn place_city_terrain_raise_preserves_xtxt() {
    // City-view raise PlaceTile does not write XTXT.
    let mut city = City::new_blank("/tmp/sc2000-raise-xtxt.sc2", "RaiseT").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 5, 0);
    city.set_text_overlay(48, 48, 0x4b);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4b));
}

#[test]
fn place_city_terrain_raise_clears_water_tile_when_above_sea() {
    // PE FUN_00447020 @ 0x4471d0: sea <= land → AND XBIT water-tile clear (`0xFB`).
    let mut city = City::new_blank("/tmp/sc2000-raise-dry.sc2", "RaiseDry").expect("blank");
    city.set_city_funds(20_000);
    // Blank sea scan stays at BLANK_MAP_BASE_HEIGHT (8); land 8→9 is dry.
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_tile_bits(
        48,
        48,
        TileBits::new(0)
            .with_water_tile(true)
            .with_piped(true)
            .with_watered(true),
    );
    city.set_traffic_density(48, 48, 0x37);
    city.set_text_overlay(48, 48, 0x4b);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    let bits = state.city.tile_bits_at(48, 48).unwrap();
    assert!(!bits.water_tile());
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(48, 48), Some(0x37));
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4b));
}

#[test]
fn place_city_terrain_raise_fun_sea_uses_saved_selected_word_like_4ca818() {
    // City RaiseTerrain reads the saved selected WORD, even with higher ALTM water elsewhere.
    let mut city = City::new_blank("/tmp/sc2000-raise-sea-map.sc2", "RaiseSeaMap").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 4, 0);
    // Unrelated high ALTM water must not replace the selected WORD.
    city.set_altitude(50, 50, 25, 20);
    city.set_mac_selected_water_height(9);
    let mut state = test_state(city);
    assert!(state.edit_map_sea_level() < 9 || state.edit_map_sea_level() == BLANK_MAP_BASE_HEIGHT);
    let pe_sea = pe_selected_water_height_for_fun_00447020(&state.city);
    assert_eq!(pe_sea, 9);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(48, 48), pe_sea as u8);
}

#[test]
fn mac68k_place_city_terrain_raise_fun_sea_uses_saved_selected_word_like_4ca818() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-raise-sea-map.sc2", "MacRaiseSeaMap").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 4, 0);
    city.set_altitude(50, 50, 25, 20);
    city.set_mac_selected_water_height(9);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let pe_sea = pe_selected_water_height_for_fun_00447020(&state.city);
    assert_eq!(pe_sea, 9);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(48, 48), pe_sea as u8);
}

#[test]
fn place_city_terrain_lower_fun_sea_uses_saved_selected_word_like_4ca818() {
    // LowerTerrain shares Raise's saved selected WORD at `0x4ca818`.
    let mut city = City::new_blank("/tmp/sc2000-lower-sea-map.sc2", "LowerSeaMap").expect("blank");
    city.set_city_funds(20_000);
    // Neighbors at post-lower height so slope ≠ 0x32 after the step.
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, 7, 0);
        }
    }
    city.set_altitude(48, 48, 8, 0);
    city.set_altitude(50, 50, 25, 20);
    city.set_mac_selected_water_height(9);
    let mut state = test_state(city);
    let pe_sea = pe_selected_water_height_for_fun_00447020(&state.city);
    assert_eq!(pe_sea, 9);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(48, 48), pe_sea as u8);
    assert_eq!(state.city.altitude_at(48, 48), 7);
}

#[test]
fn place_city_terrain_raise_sets_water_tile_when_below_sea() {
    // PE FUN_00447020 @ 0x447210: sea > land → OR XBIT water-tile `0x04`.
    let mut city = City::new_blank("/tmp/sc2000-raise-wet.sc2", "RaiseWet").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT.saturating_sub(3), 0);
    city.set_tile_bits(48, 48, TileBits::new(0).with_piped(true));
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert!(state.city.tile_bits_at(48, 48).unwrap().piped());
    assert!(
        state.city.altitude_at(48, 48) < BLANK_MAP_BASE_HEIGHT,
        "fixture must stay below blank sea"
    );
}

#[test]
fn place_city_terrain_raise_writes_altm_water_from_sea_when_below_sea() {
    // PE FUN_00447020 @ 0x447248: wet path ALTM `(old & 0xFC1F) | (sea << 5)`.
    // Flatten neighbors to the raised land height so slope ≠ 0x32 (second land++).
    let mut city = City::new_blank("/tmp/sc2000-raise-altm.sc2", "RaiseAltm").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    let land0 = BLANK_MAP_BASE_HEIGHT.saturating_sub(3);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, land0, 0);
        }
    }
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(
        state.city.water_height_at(48, 48),
        BLANK_MAP_BASE_HEIGHT,
        "ALTM water should snap to sea"
    );
    assert_eq!(
        state.city.altitude_at(48, 48),
        BLANK_MAP_BASE_HEIGHT.saturating_sub(2)
    );
}

#[test]
fn mac68k_place_city_terrain_raise_writes_altm_water_from_sea_when_below_sea() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-raise-altm.sc2", "MacRaiseAltm").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT.saturating_sub(3), 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.water_height_at(48, 48), BLANK_MAP_BASE_HEIGHT);
}

#[test]
fn mac68k_place_city_terrain_raise_clears_water_tile_when_above_sea() {
    let mut city = City::new_blank("/tmp/sc2000-mac-raise-dry.sc2", "MacRaiseDry").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_tile_bits(48, 48, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert!(!state.city.tile_bits_at(48, 48).unwrap().water_tile());
}

#[test]
fn place_city_terrain_raise_clears_xzon_type_keeps_corners() {
    // PE FUN_00447020 @ 0x447190: XZON &= 0xf0 after land step (terrain-mask ≠ 0).
    let mut city = City::new_blank("/tmp/sc2000-raise-xzon.sc2", "RaiseZn").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 5, 0);
    city.set_zone(48, 48, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_text_overlay(48, 48, 0x4b);
    city.set_traffic_density(48, 48, 0x37);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    let zone = state.city.zone_at(48, 48).unwrap();
    assert_eq!(zone.zone_id(), 0);
    assert_eq!(zone.raw_bits() & 0xf0, ZONE_CORNER_ALL);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4b));
    assert_eq!(state.city.traffic_density_at(48, 48), Some(0x37));
}

#[test]
fn mac68k_place_city_terrain_raise_clears_xzon_type_keeps_corners() {
    let mut city = City::new_blank("/tmp/sc2000-mac-raise-xzon.sc2", "MacRaiseZn").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 5, 0);
    city.set_zone(48, 48, ZONE_COMMERCIAL_DENSE | 0x80);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    let zone = state.city.zone_at(48, 48).unwrap();
    assert_eq!(zone.zone_id(), 0);
    assert_eq!(zone.raw_bits() & 0xf0, 0x80);
}

#[test]
fn place_city_terrain_raise_clears_xund_via_place_xund() {
    // PE FUN_00447020 @ 0x4470c4: XUND ≠ 0 → PlaceXUND(0) (`0x401e38` / `0x442370`).
    let mut city = City::new_blank("/tmp/sc2000-raise-xund.sc2", "RaiseXu").expect("blank");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 48, 48, 5);
    city.set_underground(48, 48, UNDER_TILE_PIPES_LR);
    city.set_text_overlay(48, 48, 0x4b);
    city.set_traffic_density(48, 48, 0x37);
    city.set_tile_bits(48, 48, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.underground_at(48, 48), 0);
    assert_eq!(state.city.altitude_at(48, 48), 6);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4b));
    assert_eq!(state.city.traffic_density_at(48, 48), Some(0x37));
    let bits = state.city.tile_bits_at(48, 48).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn pe_maptool_surface_xter_gate_clears_midband() {
    // PE @ `0x4434de`/`0x44352e`: XTER >= 0x30 (not 0x3E) → write XTER 0.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-xter.sc2", "MapSurfXt").expect("blank");
    city.set_building(35, 35, 8);
    city.set_terrain(35, 35, 0x3d);
    assert!(pe_maptool_surface_xter_gate_4434de(
        &mut city, 35, 35, false
    ));
    assert_eq!(city.terrain_at(35, 35), 0);
    assert_eq!(city.building_at(35, 35), 8);
    city.set_terrain(35, 35, 0x2f);
    assert!(!pe_maptool_surface_xter_gate_4434de(
        &mut city, 35, 35, false
    ));
    assert_eq!(city.terrain_at(35, 35), 0x2f);
}

#[test]
fn pe_maptool_surface_xter_gate_clears_water_tile_bit() {
    // PE @ `0x44356d`: when DestroyBuilding did not run, XBIT &= 0xFB.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-wbit.sc2", "MapSurfWb").expect("blank");
    city.set_terrain(37, 37, 0x3d);
    city.set_tile_bits(
        37,
        37,
        TileBits::new(0).with_water_tile(true).with_watered(true),
    );
    assert!(pe_maptool_surface_xter_gate_4434de(
        &mut city, 37, 37, false
    ));
    let bits = city.tile_bits_at(37, 37).unwrap();
    assert!(!bits.water_tile());
    assert!(bits.watered());
    assert_eq!(city.terrain_at(37, 37), 0);
}

#[test]
fn pe_maptool_surface_xter_gate_placewater_after_destroy_on_water() {
    // PE @ `0x44353f`: DestroyBuilding + water-tile → PlaceWater helper.
    // Use XBLD >= 0x70 so size-1 nonflat FUN does not rewrite XTER before the gate.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-pw.sc2", "MapSurfPw").expect("blank");
    city.set_building(38, 38, 0x70);
    city.set_terrain(38, 38, 0x3d);
    city.set_altitude(38, 38, 0x10, 0x10);
    city.set_tile_bits(38, 38, TileBits::new(0).with_water_tile(true));
    let mut crt = Win95CrtRand::new(0x4435_3f01);
    assert!(pe_maptool_surface_destroy_building_443461(
        &mut city,
        Some(&mut crt),
        38,
        38
    ));
    assert_eq!(city.terrain_at(38, 38), 0x3d);
    assert!(pe_maptool_surface_xter_gate_4434de(&mut city, 38, 38, true));
    assert!(city.tile_bits_at(38, 38).unwrap().water_tile());
}

#[test]
fn pe_maptool_surface_xter_gate_reslopes_water_neighbors() {
    // PE @ `0x44357b`: after XTER gate, PlaceWater water-tile neighbors.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-nbr.sc2", "MapSurfNbr").expect("blank");
    city.set_terrain(40, 40, 0x3d);
    city.set_terrain(39, 40, 0);
    city.set_altitude(39, 40, 0x12, 0x12);
    city.set_tile_bits(39, 40, TileBits::new(0).with_water_tile(true));
    assert!(pe_maptool_surface_xter_gate_4434de(
        &mut city, 40, 40, false
    ));
    assert_eq!(city.terrain_at(40, 40), 0);
    // Water-tile neighbor: PlaceWater writes XTER nibble+0x30 then exits.
    assert_eq!(city.terrain_at(39, 40), 0x3d);
    assert!(city.tile_bits_at(39, 40).unwrap().water_tile());
}

#[test]
fn pe_maptool_surface_preclear_clears_midband_xter() {
    // Surface preclear ends with XTER gate after PlaceTile(0).
    let mut city =
        City::new_blank("/tmp/sc2000-map-surf-pc-xter.sc2", "MapSurfPcX").expect("blank");
    city.set_building(36, 36, 8);
    city.set_terrain(36, 36, 0x3d);
    pe_maptool_surface_preclear_subway_443461(&mut city, None, 36, 36);
    assert_eq!(city.building_at(36, 36), 0);
    assert_eq!(city.terrain_at(36, 36), 0);
}

#[test]
fn citytool_substation_places_over_tree_without_maptool_preclear() {
    // CityTool PlaceTile-size overwrites trees; no FUN_00443270 PlaceTile(0) first.
    let mut city = City::new_blank("/tmp/sc2000-sub-surf-tree.sc2", "SubSurfTr").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 8);
    city.set_tile_bits(20, 20, TileBits::new(0).with_watered(true));
    city.set_traffic_density(20, 20, 0x41);
    let mut state = test_state(city);
    state.tool = EditTool::SubStation;
    assert!(state.place_building_at(20, 20, 233) > 0);
    assert_eq!(state.city.building_at(20, 20), 233);
    assert!(state.city.tile_bits_at(20, 20).unwrap().watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x41));
}

#[test]
fn citytool_substation_places_when_underground_view_empty() {
    // CityTool SubStation does not run MapTool und-empty refuse `0x443350`.
    let mut city = City::new_blank("/tmp/sc2000-sub-und-empty.sc2", "SubUndEmp").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 0);
    city.set_underground(20, 20, 0);
    city.set_tunnel_levels(20, 20, 0);
    let mut state = test_state(city);
    state.show_underground = true;
    state.tool = EditTool::SubStation;
    assert!(state.place_building_at(20, 20, 233) > 0);
    assert_eq!(state.city.building_at(20, 20), 233);
    assert_eq!(state.city.underground_at(20, 20), UNDER_TILE_GREY_BOX_2);
}

#[test]
fn pe_maptool_surface_vacant_refuse_xter_lt_0x30() {
    // PE @ `0x443310`–`0x443323`: XBLD==0 and XTER < 0x30 → refuse.
    let mut city = City::new_blank("/tmp/sc2000-map-surf-vac.sc2", "MapSurfVac").expect("blank");
    city.set_building(37, 37, 0);
    city.set_terrain(37, 37, 0);
    assert!(pe_maptool_surface_vacant_refuse_44331c(&city, 37, 37));
    city.set_terrain(37, 37, 0x30);
    assert!(!pe_maptool_surface_vacant_refuse_44331c(&city, 37, 37));
    city.set_terrain(37, 37, 0);
    city.set_building(37, 37, 6);
    assert!(!pe_maptool_surface_vacant_refuse_44331c(&city, 37, 37));
}

#[test]
fn pe_maptool_underground_clear_skips_when_surface_display_layer() {
    // PE @ `0x4433cc`: DisplayLayer clear → surface arm; underground helper no-ops.
    let mut city = City::new_blank("/tmp/sc2000-map-und-skip.sc2", "MapUndSkip").expect("blank");
    city.set_building(34, 34, 0xe9);
    city.set_underground(34, 34, UNDER_TILE_PIPES_LR);
    city.set_tunnel_levels(34, 34, 0);
    assert!(!pe_maptool_underground_clear_subway_443427(
        &mut city, None, 34, 34, false
    ));
    assert_eq!(city.building_at(34, 34), 0xe9);
    assert_eq!(city.underground_at(34, 34), UNDER_TILE_PIPES_LR);
}

#[test]
fn maptool_substation_underground_view_clears_xund_keeps_surface_building() {
    // PE DisplayLayer underground helper: PlaceXUND(0) without DestroyBuilding when
    // tunnel!=1 and XUND!=0x23 (surface mid-band stays until PlaceTile-size).
    let mut city = City::new_blank("/tmp/sc2000-sub-und-view.sc2", "SubUndView").expect("blank");
    city.set_building(20, 20, 0x20);
    city.set_underground(20, 20, 0x05);
    city.set_tunnel_levels(20, 20, 0);
    assert!(pe_maptool_underground_clear_subway_443427(
        &mut city, None, 20, 20, true
    ));
    assert_eq!(city.building_at(20, 20), 0x20);
    assert_eq!(city.underground_at(20, 20), 0);
    // Surface DisplayLayer still takes the surface preclear path instead.
    assert!(!pe_maptool_underground_clear_subway_443427(
        &mut city, None, 20, 20, false
    ));
}

#[test]
fn pe_place_rail_slope_neighbor_early_out_places_rotated_slopes() {
    // PE @ `0x44b467`..`0x44b5fe`: cardinal special neighbors force slope XBLD.
    let mut city = City::new_blank("/tmp/sc2000-rail-slope.sc2", "RailSlope").expect("blank");
    city.set_building(40, 40, 0x2c);
    city.set_building(40, 39, 0x2e); // north
    assert_eq!(
        pe_place_rail_slope_from_cardinal_neighbor_44b467(&city, 40, 40),
        Some(0x3b)
    );
    city.set_building(40, 39, 0);
    city.set_building(40, 41, 0x30); // south
    assert_eq!(
        pe_place_rail_slope_from_cardinal_neighbor_44b467(&city, 40, 40),
        Some(0x3d)
    );
    city.set_building(40, 41, 0);
    city.set_building(39, 40, 0x2f); // west
    assert_eq!(
        pe_place_rail_slope_from_cardinal_neighbor_44b467(&city, 40, 40),
        Some(0x3c)
    );
    city.set_building(39, 40, 0);
    city.set_building(41, 40, 0x31); // east
    assert_eq!(
        pe_place_rail_slope_from_cardinal_neighbor_44b467(&city, 40, 40),
        Some(0x3e)
    );
}

#[test]
fn refresh_road_xtxt_fa_ors_map_edge_into_mask() {
    // PE PlaceRoads @ `0x44b0ac`: XTXT 0xfa on west edge → mask|=LEFT → table[8]+0x1d=0x1e.
    let mut city = City::new_blank("/tmp/sc2000-road-fa-edge.sc2", "RoadFaEdge").expect("blank");
    city.set_building(0, 40, 0x1d);
    city.set_text_overlay(0, 40, 0xfa);
    let mut state = test_state(city);
    state.refresh_network_tile(0, 40, NetworkKind::Road);
    assert_eq!(state.city.building_at(0, 40), 0x1e);
    // Without 0xfa, isolated edge stays mask0 → 0x1d.
    state.city.set_building(0, 41, 0x1d);
    state.city.set_text_overlay(0, 41, 0);
    state.refresh_network_tile(0, 41, NetworkKind::Road);
    assert_eq!(state.city.building_at(0, 41), 0x1d);
}

#[test]
fn refresh_rail_xtxt_fa_ors_map_edge_into_mask() {
    // PE PlaceRails @ `0x44b665`: XTXT 0xfa on west edge → mask|=LEFT → table[8]+0x2c=0x2d.
    let mut city = City::new_blank("/tmp/sc2000-rail-fa-edge.sc2", "RailFaEdge").expect("blank");
    city.set_building(0, 50, 0x2c);
    city.set_text_overlay(0, 50, 0xfa);
    let mut state = test_state(city);
    state.refresh_network_tile(0, 50, NetworkKind::Rail);
    assert_eq!(state.city.building_at(0, 50), 0x2d);
}

#[test]
fn mac68k_refresh_road_rail_xtxt_fa_ors_map_edge_into_mask() {
    let mut city = City::new_blank("/tmp/sc2000-fa-edge-mac.sc2", "FaEdgeMac").expect("blank");
    city.set_building(0, 40, 0x1d);
    city.set_text_overlay(0, 40, 0xfa);
    city.set_building(0, 50, 0x2c);
    city.set_text_overlay(0, 50, 0xfa);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_network_tile(0, 40, NetworkKind::Road);
    state.refresh_network_tile(0, 50, NetworkKind::Rail);
    assert_eq!(state.city.building_at(0, 40), 0x1e);
    assert_eq!(state.city.building_at(0, 50), 0x2d);
}

#[test]
fn refresh_rail_slope_neighbor_early_out_after_xtxt_miss() {
    let mut city =
        City::new_blank("/tmp/sc2000-rail-slope-ref.sc2", "RailSlopeRef").expect("blank");
    city.set_building(42, 42, 0x2d);
    city.set_text_overlay(42, 42, 5); // gate off → fall through
    city.set_building(42, 41, 0x2e); // north → slope 0x3b
    let mut state = test_state(city);
    state.refresh_network_tile(42, 42, NetworkKind::Rail);
    assert_eq!(state.city.building_at(42, 42), 0x3b);
    // XTXT gate on beats slope.
    state.city.set_building(42, 42, 0x2d);
    state.city.set_text_overlay(42, 42, 1);
    state.refresh_network_tile(42, 42, NetworkKind::Rail);
    assert_eq!(state.city.building_at(42, 42), 0x2e);
}

#[test]
fn mac68k_refresh_rail_slope_neighbor_early_out_after_xtxt_miss() {
    let mut city =
        City::new_blank("/tmp/sc2000-rail-slope-ref-mac.sc2", "RailSlopeRefMac").expect("blank");
    city.set_building(42, 42, 0x2d);
    city.set_text_overlay(42, 42, 5);
    city.set_building(42, 41, 0x2e);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_network_tile(42, 42, NetworkKind::Rail);
    assert_eq!(state.city.building_at(42, 42), 0x3b);
    state.city.set_building(42, 42, 0x2d);
    state.city.set_text_overlay(42, 42, 1);
    state.refresh_network_tile(42, 42, NetworkKind::Rail);
    assert_eq!(state.city.building_at(42, 42), 0x2e);
}

#[test]
fn mac_surface_network_slope_retile_precedes_neighbor_mask() {
    for kind in [NetworkKind::Power, NetworkKind::Road, NetworkKind::Rail] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-slope-retile.sc2", "MacSlope").expect("blank");
        city.set_terrain(55, 55, 0x01);
        city.set_building(55, 55, kind.base_tile());
        let east_neighbor = if kind == NetworkKind::Power {
            kind.base_tile()
        } else {
            kind.base_tile() + 1
        };
        city.set_building(56, 55, east_neighbor);
        if kind == NetworkKind::Power {
            stamp_xbit_powerable(&mut city, 55, 55);
            stamp_xbit_powerable(&mut city, 56, 55);
        }

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        assert_eq!(
            state.network_neighbor_mask(55, 55, kind),
            NETWORK_RIGHT,
            "fixture should provide a horizontal neighbor for {kind:?}"
        );

        state.refresh_network_tile(55, 55, kind);
        assert_eq!(
            state.city.building_at(55, 55),
            kind.base_tile() + 2,
            "Mac XTER 1 table chooses the slope tile before the horizontal mask for {kind:?}"
        );
    }
}

#[test]
fn place_city_terrain_raise_clears_xund_on_military_zone() {
    // Raise → PlaceXUND(0) even on military (PE still writes; counter skipped).
    // Slope remap may then `XZON &= 0xf0` (@ `0x44716b`), clearing type 7.
    let mut city = City::new_blank("/tmp/sc2000-raise-xund-mil.sc2", "RaiseXuMil").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 5, 0);
    city.set_zone(48, 48, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_underground(48, 48, UNDER_TILE_PIPES_LR);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.underground_at(48, 48), 0);
}

#[test]
fn mac68k_place_city_terrain_raise_clears_xund_via_place_xund() {
    let mut city = City::new_blank("/tmp/sc2000-mac-raise-xund.sc2", "MacRaiseXu").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 5, 0);
    city.set_underground(48, 48, UNDER_TILE_PIPES_LR);
    city.set_text_overlay(48, 48, 0x4b);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.underground_at(48, 48), 0);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4b));
}

#[test]
fn place_city_terrain_raise_clears_tree_xbld_via_place_tile() {
    // PE FUN_00447020 @ 0x4470ac: PlaceTile(0) when XBLD ≠ 5 (trees/rubble).
    let mut city = City::new_blank("/tmp/sc2000-raise-tree.sc2", "RaiseTr").expect("blank");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 48, 48, 5);
    city.set_building(48, 48, 6);
    city.set_text_overlay(48, 48, 0x4b);
    city.set_traffic_density(48, 48, 0x37);
    city.set_tile_bits(48, 48, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 0);
    assert_eq!(state.city.altitude_at(48, 48), 6);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4b));
    assert_eq!(state.city.traffic_density_at(48, 48), Some(0x37));
    let bits = state.city.tile_bits_at(48, 48).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn place_city_terrain_raise_keeps_radioactivity_xbld_5() {
    // PE FUN_00447020 @ 0x4470a1: skip PlaceTile when saved XBLD == 5; still raise ALTM.
    let mut city = City::new_blank("/tmp/sc2000-raise-rad.sc2", "RaiseRad").expect("blank");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 48, 48, 5);
    city.set_building(48, 48, 5);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.altitude_at(48, 48), 6);
}

#[test]
fn place_city_terrain_raise_radioactivity_writes_dry_slope_xter() {
    // Early PlaceTile skip for XBLD 5; dry neighbor-slope arm still writes remapped XTER.
    let mut city = City::new_blank("/tmp/sc2000-raise-rad-slope.sc2", "RaiseRadS").expect("blank");
    city.set_city_funds(20_000);
    let base = BLANK_MAP_BASE_HEIGHT + 4;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, base, 0);
        }
    }
    city.set_altitude(48, 47, base + 2, 0); // north stays higher after raise
    city.set_altitude(48, 48, base, 0);
    city.set_building(48, 48, 5);
    city.set_terrain(48, 48, 0x0a);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.altitude_at(48, 48), base + 1);
    assert!(!state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.terrain_at(48, 48), 2); // north bit → remap 2
}

#[test]
fn mac68k_place_city_terrain_raise_radioactivity_writes_dry_slope_xter() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-raise-rad-slope.sc2", "MacRaiseRadS").expect("blank");
    city.set_city_funds(20_000);
    let base = BLANK_MAP_BASE_HEIGHT + 4;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, base, 0);
        }
    }
    city.set_altitude(48, 47, base + 2, 0);
    city.set_altitude(48, 48, base, 0);
    city.set_building(48, 48, 5);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.terrain_at(48, 48), 2);
}

#[test]
fn place_city_terrain_raise_destroys_small_park_via_destroy_building() {
    // PE FUN_00447020 @ 0x447091: XBLD >= 0x0D → DestroyBuilding (clears XBLD+XTXT).
    let mut city = City::new_blank("/tmp/sc2000-raise-park.sc2", "RaisePk").expect("blank");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 48, 48, 5);
    city.set_building(48, 48, SMALL_PARK_BUILDING_ID);
    city.set_text_overlay(48, 48, 0x4b);
    city.set_traffic_density(48, 48, 0x37);
    city.set_tile_bits(48, 48, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 0);
    assert_eq!(state.city.altitude_at(48, 48), 6);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0));
    assert_eq!(state.city.traffic_density_at(48, 48), Some(0x37));
    let bits = state.city.tile_bits_at(48, 48).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn place_city_terrain_raise_elevated_destroy_building_arm() {
    // PE FUN_00447020 @ `0x447091`: elevated `0x3f..=0x42` DestroyBuilding arm
    // (XZON/ALTM); then PlaceTile(0). Flatten neighbors so slope ≠ `0x32`.
    let mut city = City::new_blank("/tmp/sc2000-raise-elev.sc2", "RaiseEl").expect("blank");
    city.set_city_funds(20_000);
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, 5, 0);
            city.set_terrain((48 + dx) as usize, (48 + dy) as usize, 0);
        }
    }
    city.set_building(48, 48, 0x3f);
    city.set_zone(48, 48, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    city.set_tunnel_levels(48, 48, 0x1f);
    city.set_text_overlay(48, 48, 0x4b);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 0);
    assert_eq!(state.city.altitude_at(48, 48), 6);
    assert_eq!(
        state
            .city
            .zone_at(48, 48)
            .map(|z| z.raw_bits())
            .unwrap_or(0)
            & 0xf0,
        0
    );
    assert_eq!(state.city.tunnel_levels_at(48, 48), 0);
    // Elevated arm never reaches XTXT clear @ `0x45a0c4`.
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4b));
}

#[test]
fn place_city_terrain_lower_destroys_small_park_via_destroy_building() {
    // PE FUN_00447020 @ 0x447091: same DestroyBuilding gate for raise and lower.
    let mut city = City::new_blank("/tmp/sc2000-lower-park.sc2", "LowerPk").expect("blank");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 48, 48, 4);
    city.set_altitude(48, 48, 5, 0);
    city.set_building(48, 48, SMALL_PARK_BUILDING_ID);
    city.set_text_overlay(48, 48, 0x4b);
    city.set_traffic_density(48, 48, 0x37);
    city.set_tile_bits(48, 48, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.building_at(48, 48), 0);
    assert_eq!(state.city.altitude_at(48, 48), 4);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0));
    assert_eq!(state.city.traffic_density_at(48, 48), Some(0x37));
    let bits = state.city.tile_bits_at(48, 48).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn place_city_terrain_raise_destroys_2x2_footprint_via_destroy_building() {
    let mut city = City::new_blank("/tmp/sc2000-raise-2x2.sc2", "Raise2x2").expect("blank");
    city.set_city_funds(20_000);
    stamp_complete_building_for_test(&mut city, 40, 40, COMMERCIAL_2X2_BUILDINGS[0], 2);
    city.set_text_overlay(40, 40, 0x4b);
    city.set_text_overlay(41, 41, 0x4c);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(40, 40, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(40, 40), 0);
    // The terrain tool clears the hit tile after DestroyStructure, while the
    // other footprint cells retain the CRT-selected rubble written by that call.
    assert!(is_rubble_tile(state.city.building_at(41, 40)));
    assert!(is_rubble_tile(state.city.building_at(40, 41)));
    assert!(is_rubble_tile(state.city.building_at(41, 41)));
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0));
    assert_eq!(state.city.text_overlay_at(41, 41), Some(0));
    // Only the clicked tile's ALTM steps.
    assert_eq!(state.city.altitude_at(40, 40), 9);
    assert_eq!(state.city.altitude_at(41, 41), 8);
}

#[test]
fn mac68k_place_city_terrain_raise_destroys_small_park_via_destroy_building() {
    let mut city = City::new_blank("/tmp/sc2000-mac-raise-park.sc2", "MacRaisePk").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 5, 0);
    city.set_building(48, 48, SMALL_PARK_BUILDING_ID);
    city.set_text_overlay(48, 48, 0x4b);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 0);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0));
}

#[test]
fn mac68k_place_city_terrain_raise_clears_tree_xbld_via_place_tile() {
    let mut city = City::new_blank("/tmp/sc2000-mac-raise-tree.sc2", "MacRaiseTr").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 5, 0);
    city.set_building(48, 48, 6);
    city.set_text_overlay(48, 48, 0x4b);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 0);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4b));
}

#[test]
fn mac68k_place_city_terrain_raise_preserves_xtxt() {
    // Shared city-view raise leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-raise-xtxt.sc2", "MacRaiseT").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, 5, 0);
    city.set_text_overlay(48, 48, 0x4b);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4b));
}

#[test]
fn mac68k_place_city_terrain_raise_preserves_xtrf() {
    // Shared city-view raise is ALTM only; Mac68k must not invent an XTRF clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-raise-xtrf.sc2", "MacRaiseX").expect("blank");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 48, 48, 5);
    city.set_traffic_density(48, 48, 0x37);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.altitude_at(48, 48), 6);
    assert_eq!(state.city.traffic_density_at(48, 48), Some(0x37));
}

#[test]
fn place_city_terrain_lower_writes_dry_slope_xter() {
    // One higher neighbor after lower → dry XTER = remapped slope (not wet +0x10).
    let mut city = City::new_blank("/tmp/sc2000-lower-xter.sc2", "LowerXter").expect("blank");
    city.set_city_funds(20_000);
    let base = BLANK_MAP_BASE_HEIGHT + 4;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, base - 1, 0);
        }
    }
    city.set_altitude(48, 47, base, 0); // north stays higher after lower
    city.set_altitude(48, 48, base, 0);
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.altitude_at(48, 48), base - 1);
    assert!(!state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.terrain_at(48, 48), 2); // north bit 3 → remap 2
}

#[test]
fn place_city_terrain_lower_radioactivity_writes_dry_slope_xter() {
    // Early PlaceTile skip for XBLD 5; dry neighbor-slope arm still writes remapped XTER.
    let mut city = City::new_blank("/tmp/sc2000-lower-rad-slope.sc2", "LowerRadS").expect("blank");
    city.set_city_funds(20_000);
    let base = BLANK_MAP_BASE_HEIGHT + 4;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, base - 1, 0);
        }
    }
    city.set_altitude(48, 47, base, 0);
    city.set_altitude(48, 48, base, 0);
    city.set_building(48, 48, 5);
    city.set_terrain(48, 48, 0x0a);
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.altitude_at(48, 48), base - 1);
    assert!(!state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.terrain_at(48, 48), 2);
}

#[test]
fn mac68k_place_city_terrain_lower_radioactivity_writes_dry_slope_xter() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-lower-rad-slope.sc2", "MacLowerRadS").expect("blank");
    city.set_city_funds(20_000);
    let base = BLANK_MAP_BASE_HEIGHT + 4;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, base - 1, 0);
        }
    }
    city.set_altitude(48, 47, base, 0);
    city.set_altitude(48, 48, base, 0);
    city.set_building(48, 48, 5);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.terrain_at(48, 48), 2);
}

#[test]
fn mac68k_place_city_terrain_lower_writes_dry_slope_xter() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-lower-xter.sc2", "MacLowerXter").expect("blank");
    city.set_city_funds(20_000);
    let base = BLANK_MAP_BASE_HEIGHT + 4;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, base - 1, 0);
        }
    }
    city.set_altitude(48, 47, base, 0);
    city.set_altitude(48, 48, base, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.terrain_at(48, 48), 2);
}

#[test]
fn place_city_terrain_lower_slope_32_writes_xter_0x10_when_wet() {
    // Slope 0x32 increments land before comparing with selected WORD 9;
    // the matching waterline keeps this cell wet with XTER `0x10` (`0x447399`).
    let mut city = City::new_blank("/tmp/sc2000-lower-s32.sc2", "LowerS32").expect("blank");
    city.set_mac_selected_water_height(9);
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_altitude(10, 10, 1, BLANK_MAP_BASE_HEIGHT + 1);
    city.set_terrain(48, 48, 0x09);
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.altitude_at(48, 48), BLANK_MAP_BASE_HEIGHT);
    assert_eq!(state.city.terrain_at(48, 48), 0x10);
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(
        state.city.water_height_at(48, 48),
        BLANK_MAP_BASE_HEIGHT + 1
    );
}

#[test]
fn mac68k_place_city_terrain_lower_slope_32_writes_xter_0x10_when_wet() {
    let mut city = City::new_blank("/tmp/sc2000-mac-lower-s32.sc2", "MacLowerS32").expect("blank");
    city.set_mac_selected_water_height(9);
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_altitude(10, 10, 1, BLANK_MAP_BASE_HEIGHT + 1);
    city.set_terrain(48, 48, 0x09);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.terrain_at(48, 48), 0x10);
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
}

#[test]
fn place_city_terrain_lower_clears_water_tile_when_above_sea() {
    let mut city = City::new_blank("/tmp/sc2000-lower-dry.sc2", "LowerDry").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT + 2, 0);
    city.set_tile_bits(48, 48, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert!(!state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.altitude_at(48, 48), BLANK_MAP_BASE_HEIGHT + 1);
}

#[test]
fn place_city_terrain_lower_clears_xzon_type_keeps_corners() {
    // After lower, neighbors are higher → slope ≠ 0 → XZON &= 0xf0.
    let mut city = City::new_blank("/tmp/sc2000-lower-xzon.sc2", "LowerZn").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_zone(48, 48, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    let zone = state.city.zone_at(48, 48).unwrap();
    assert_eq!(zone.zone_id(), 0);
    assert_eq!(zone.raw_bits() & 0xf0, ZONE_CORNER_ALL);
}

#[test]
fn place_city_terrain_flat_raise_radioactivity_writes_xter_zero() {
    // Early PlaceTile skip for XBLD 5; dry slope-0 arm still writes XTER := 0.
    let mut city = City::new_blank("/tmp/sc2000-raise-rad-xter.sc2", "RaiseRadX").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_building(48, 48, 5);
    city.set_terrain(48, 48, 0x0a);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.terrain_at(48, 48), 0);
    assert_eq!(state.city.altitude_at(48, 48), BLANK_MAP_BASE_HEIGHT + 1);
}

#[test]
fn mac68k_place_city_terrain_flat_raise_radioactivity_writes_xter_zero() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-raise-rad-xter.sc2", "MacRaiseRadX").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_building(48, 48, 5);
    city.set_terrain(48, 48, 0x0a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.terrain_at(48, 48), 0);
}

#[test]
fn place_city_terrain_lower_slope_32_dry_radioactivity_writes_xter_zero() {
    // Same early PlaceTile skip for XBLD 5; after land++ reaches sea level,
    // slope 0x32 takes its dry XTER := 0 arm.
    let mut city = City::new_blank("/tmp/sc2000-lower-rad-xter.sc2", "LowerRadX").expect("blank");
    city.set_city_funds(20_000);
    // Keep all neighbors two levels higher so the lower-step slope-0x32 arm
    // increments back to sea level, then takes its dry XTER=0 branch.
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            city.set_altitude(
                (48isize + dx) as usize,
                (48isize + dy) as usize,
                BLANK_MAP_BASE_HEIGHT + 2,
                0,
            );
        }
    }
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_building(48, 48, 5);
    city.set_terrain(48, 48, 0x0a);
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.terrain_at(48, 48), 0);
    assert_eq!(state.city.altitude_at(48, 48), BLANK_MAP_BASE_HEIGHT);
    assert!(!state.city.tile_bits_at(48, 48).unwrap().water_tile());
}

#[test]
fn mac68k_place_city_terrain_lower_slope_32_dry_radioactivity_writes_xter_zero() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-lower-rad-xter.sc2", "MacLowerRadX").expect("blank");
    city.set_city_funds(20_000);
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            city.set_altitude(
                (48isize + dx) as usize,
                (48isize + dy) as usize,
                BLANK_MAP_BASE_HEIGHT + 2,
                0,
            );
        }
    }
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_building(48, 48, 5);
    city.set_terrain(48, 48, 0x0a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.terrain_at(48, 48), 0);
    assert_eq!(state.city.altitude_at(48, 48), BLANK_MAP_BASE_HEIGHT);
    assert!(!state.city.tile_bits_at(48, 48).unwrap().water_tile());
}

#[test]
fn place_city_terrain_flat_raise_writes_xter_zero_over_prior_slope() {
    // PE dry arm @ 0x4471e0: XTER := remapped slope (0 on flat) even if prior XTER was set.
    let mut city = City::new_blank("/tmp/sc2000-raise-xter0.sc2", "RaiseXter0").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_terrain(48, 48, 0x0b);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.terrain_at(48, 48), 0);
    assert!(!state.city.tile_bits_at(48, 48).unwrap().water_tile());
}

#[test]
fn mac68k_place_city_terrain_flat_raise_writes_xter_zero_over_prior_slope() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-raise-xter0.sc2", "MacRaiseXter0").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_terrain(48, 48, 0x0b);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.terrain_at(48, 48), 0);
}

#[test]
fn place_city_terrain_flat_raise_preserves_xzon_when_slope_zero() {
    // PE: remapped slope 0 skips XZON clear (`0x44716b`).
    let mut city = City::new_blank("/tmp/sc2000-raise-flat-zn.sc2", "RaiseFlatZn").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_zone(48, 48, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    let zone = state.city.zone_at(48, 48).unwrap();
    assert_eq!(zone.zone_id(), ZONE_RESIDENTIAL_LIGHT);
    assert_eq!(zone.raw_bits() & 0xf0, ZONE_CORNER_ALL);
    assert_eq!(state.city.terrain_at(48, 48), 0);
}

#[test]
fn place_city_terrain_raise_slope_32_writes_xter_0x10_when_wet() {
    // Full surround higher → remap 0x32 and land++; the sea check then selects
    // wet XTER `0x10` (`0x447399`).
    let mut city = City::new_blank("/tmp/sc2000-raise-s32.sc2", "RaiseS32").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT.saturating_sub(3), 0);
    city.set_building(48, 48, 6);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.terrain_at(48, 48), 0x10);
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.building_at(48, 48), 0);
    assert_eq!(
        state.city.altitude_at(48, 48),
        BLANK_MAP_BASE_HEIGHT.saturating_sub(1)
    );
}

#[test]
fn mac68k_place_city_terrain_raise_slope_32_writes_xter_0x10_when_wet() {
    let mut city = City::new_blank("/tmp/sc2000-mac-raise-s32.sc2", "MacRaiseS32").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT.saturating_sub(3), 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.terrain_at(48, 48), 0x10);
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
}

#[test]
fn mac68k_place_city_terrain_raise_writes_wet_xter_plus_10() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-raise-wet-xter.sc2", "MacRaiseWetX").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    let sea = BLANK_MAP_BASE_HEIGHT;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            city.set_altitude(
                (48 + dx) as usize,
                (48 + dy) as usize,
                sea.saturating_sub(2),
                0,
            );
        }
    }
    city.set_altitude(48, 47, sea, 0);
    city.set_altitude(48, 48, sea.saturating_sub(3), 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.terrain_at(48, 48), 0x12);
}

#[test]
fn place_city_terrain_raise_wet_radioactivity_writes_wet_slope_xter() {
    // Wet arm skips PlaceTile for XBLD 5 but still writes slope+0x10 XTER.
    let mut city =
        City::new_blank("/tmp/sc2000-raise-wet-rad-xter.sc2", "RaiseWetRadX").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    let sea = BLANK_MAP_BASE_HEIGHT;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            city.set_altitude(
                (48 + dx) as usize,
                (48 + dy) as usize,
                sea.saturating_sub(2),
                0,
            );
        }
    }
    city.set_altitude(48, 47, sea, 0);
    city.set_altitude(48, 48, sea.saturating_sub(3), 0);
    city.set_building(48, 48, 5);
    city.set_text_overlay(48, 48, 0x51);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x51));
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.terrain_at(48, 48), 0x12);
}

#[test]
fn place_city_terrain_lower_wet_radioactivity_writes_wet_slope_xter() {
    // Lower into wet with one higher neighbor: keep XBLD 5, write slope+0x10.
    // Avoid full-surround higher neighbors (remap 0x32 → XTER 0).
    let mut city =
        City::new_blank("/tmp/sc2000-lower-wet-rad-xter.sc2", "LowerWetRadX").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    let sea = BLANK_MAP_BASE_HEIGHT;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            city.set_altitude(
                (48 + dx) as usize,
                (48 + dy) as usize,
                sea.saturating_sub(2),
                0,
            );
        }
    }
    city.set_altitude(48, 47, sea, 0);
    city.set_altitude(48, 48, sea, 0);
    city.set_building(48, 48, 5);
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.terrain_at(48, 48), 0x22); // sea-land==1 → slope+0x20
}

#[test]
fn mac68k_place_city_terrain_lower_wet_radioactivity_writes_wet_slope_xter() {
    let mut city = City::new_blank("/tmp/sc2000-mac-lower-wet-rad-xter.sc2", "MacLowerWetRadX")
        .expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    let sea = BLANK_MAP_BASE_HEIGHT;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            city.set_altitude(
                (48 + dx) as usize,
                (48 + dy) as usize,
                sea.saturating_sub(2),
                0,
            );
        }
    }
    city.set_altitude(48, 47, sea, 0);
    city.set_altitude(48, 48, sea, 0);
    city.set_building(48, 48, 5);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.terrain_at(48, 48), 0x22);
}

#[test]
fn mac68k_place_city_terrain_raise_wet_radioactivity_writes_wet_slope_xter() {
    let mut city = City::new_blank("/tmp/sc2000-mac-raise-wet-rad-xter.sc2", "MacRaiseWetRadX")
        .expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    let sea = BLANK_MAP_BASE_HEIGHT;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            city.set_altitude(
                (48 + dx) as usize,
                (48 + dy) as usize,
                sea.saturating_sub(2),
                0,
            );
        }
    }
    city.set_altitude(48, 47, sea, 0);
    city.set_altitude(48, 48, sea.saturating_sub(3), 0);
    city.set_building(48, 48, 5);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.terrain_at(48, 48), 0x12);
}

#[test]
fn place_city_terrain_raise_wet_keeps_radioactivity_skips_place_tile() {
    // PE wet arm @ 0x44725c: XBLD 5 skips PlaceTile(0); still sets water-tile/ALTM.
    let mut city = City::new_blank("/tmp/sc2000-raise-wet-rad.sc2", "RaiseWetRad").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT.saturating_sub(3), 0);
    city.set_building(48, 48, 5);
    city.set_text_overlay(48, 48, 0x51);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x51));
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
}

#[test]
fn mac68k_place_city_terrain_raise_wet_keeps_radioactivity_skips_place_tile() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-raise-wet-rad.sc2", "MacRaiseWetRad").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT.saturating_sub(3), 0);
    city.set_building(48, 48, 5);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 5);
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
}

#[test]
fn place_city_terrain_raise_wet_place_tile_preserves_xtxt() {
    // Wet arm PlaceTile(0) @ 0x447262 leaves XTXT like PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-raise-wet-xtxt.sc2", "RaiseWetT").expect("blank");
    city.set_city_funds(20_000);
    let sea = BLANK_MAP_BASE_HEIGHT;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            city.set_altitude(
                (48 + dx) as usize,
                (48 + dy) as usize,
                sea.saturating_sub(2),
                0,
            );
        }
    }
    city.set_altitude(48, 47, sea, 0);
    city.set_altitude(48, 48, sea.saturating_sub(3), 0);
    city.set_building(48, 48, 6);
    city.set_text_overlay(48, 48, 0x4d);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 0);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4d));
}

#[test]
fn mac68k_place_city_terrain_raise_wet_place_tile_preserves_xtxt() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-raise-wet-xtxt.sc2", "MacRaiseWetT").expect("blank");
    city.set_city_funds(20_000);
    let sea = BLANK_MAP_BASE_HEIGHT;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            city.set_altitude(
                (48 + dx) as usize,
                (48 + dy) as usize,
                sea.saturating_sub(2),
                0,
            );
        }
    }
    city.set_altitude(48, 47, sea, 0);
    city.set_altitude(48, 48, sea.saturating_sub(3), 0);
    city.set_building(48, 48, 7);
    city.set_text_overlay(48, 48, 0x4d);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.building_at(48, 48), 0);
    assert_eq!(state.city.text_overlay_at(48, 48), Some(0x4d));
}

#[test]
fn place_city_terrain_raise_writes_wet_xter_plus_10() {
    // One higher neighbor → slope remap ≠ 0x32; wet XTER = slope + 0x10 (`0x44729a`).
    let mut city = City::new_blank("/tmp/sc2000-raise-wet-xter.sc2", "RaiseWetX").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    let sea = BLANK_MAP_BASE_HEIGHT;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            city.set_altitude(
                (48 + dx) as usize,
                (48 + dy) as usize,
                sea.saturating_sub(2),
                0,
            );
        }
    }
    city.set_altitude(48, 47, sea, 0); // north higher after raise
    city.set_altitude(48, 48, sea.saturating_sub(3), 0);
    city.set_building(48, 48, 6);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    let land = state.city.altitude_at(48, 48);
    assert!(sea > land);
    assert!(sea - land > 1);
    assert_eq!(state.city.building_at(48, 48), 0);
    // North-only bit 3 → remap 2 → wet 0x12.
    assert_eq!(state.city.terrain_at(48, 48), 0x12);
}

#[test]
fn place_city_terrain_raise_writes_wet_xter_plus_20_when_sea_minus_land_one() {
    let mut city = City::new_blank("/tmp/sc2000-raise-wet20.sc2", "RaiseWet20").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    let sea = BLANK_MAP_BASE_HEIGHT;
    for dx in -1i32..=1 {
        for dy in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, sea - 1, 0);
        }
    }
    city.set_altitude(48, 47, sea, 0);
    city.set_altitude(48, 48, sea.saturating_sub(2), 0);
    let mut state = test_state(city);
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    assert_eq!(state.city.altitude_at(48, 48), sea - 1);
    assert_eq!(
        state.edit_map_sea_level() - state.city.altitude_at(48, 48),
        1
    );
    assert_eq!(state.city.terrain_at(48, 48), 0x22);
}

#[test]
fn mac68k_place_city_terrain_flat_raise_preserves_xzon_when_slope_zero() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-raise-flat-zn.sc2", "MacRaiseFlatZn").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT, 0);
    city.set_zone(48, 48, ZONE_COMMERCIAL_DENSE | 0x80);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::RaiseTerrain));
    let zone = state.city.zone_at(48, 48).unwrap();
    assert_eq!(zone.zone_id(), ZONE_COMMERCIAL_DENSE);
    assert_eq!(zone.raw_bits() & 0xf0, 0x80);
}

#[test]
fn mac68k_place_city_terrain_lower_clears_water_tile_when_above_sea() {
    let mut city = City::new_blank("/tmp/sc2000-mac-lower-dry.sc2", "MacLowerDry").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(48, 48, BLANK_MAP_BASE_HEIGHT + 2, 0);
    city.set_tile_bits(48, 48, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert!(!state.city.tile_bits_at(48, 48).unwrap().water_tile());
}

#[test]
fn place_city_terrain_lower_preserves_watered_and_xtrf() {
    // City-view lower: ALTM only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-lower-xtrf.sc2", "LowerX").expect("blank");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 49, 49, 4);
    city.set_altitude(49, 49, 5, 0);
    city.set_tile_bits(49, 49, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(49, 49, 0x38);
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(49, 49, EditTool::LowerTerrain));
    assert_eq!(state.city.altitude_at(49, 49), 4);
    let bits = state.city.tile_bits_at(49, 49).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(49, 49), Some(0x38));
}

#[test]
fn place_city_terrain_lower_preserves_xtxt() {
    // City-view lower is ALTM only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-lower-xtxt.sc2", "LowerT").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(49, 49, 5, 0);
    city.set_text_overlay(49, 49, 0x4d);
    let mut state = test_state(city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(49, 49, EditTool::LowerTerrain));
    assert_eq!(state.city.text_overlay_at(49, 49), Some(0x4d));
}

#[test]
fn mac68k_place_city_terrain_lower_preserves_xtxt() {
    // Shared city-view lower leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-lower-xtxt.sc2", "MacLowerT").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(49, 49, 5, 0);
    city.set_text_overlay(49, 49, 0x4d);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(49, 49, EditTool::LowerTerrain));
    assert_eq!(state.city.text_overlay_at(49, 49), Some(0x4d));
}

#[test]
fn mac68k_place_city_terrain_lower_preserves_xtrf() {
    // Shared city-view lower is ALTM only; Mac68k must not invent an XTRF clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-lower-xtrf.sc2", "MacLowerX").expect("blank");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 49, 49, 4);
    city.set_altitude(49, 49, 5, 0);
    city.set_traffic_density(49, 49, 0x38);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(49, 49, EditTool::LowerTerrain));
    assert_eq!(state.city.altitude_at(49, 49), 4);
    assert_eq!(state.city.traffic_density_at(49, 49), Some(0x38));
}

#[test]
fn adjust_edit_map_sea_level_preserves_watered_and_xtrf() {
    // Edit-map sea level: XTER/ALTM only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-emap-sea-xtrf.sc2", "ESeaX").expect("blank");
    city.set_altitude(52, 52, 10, 0);
    city.set_tile_bits(52, 52, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(52, 52, 0x21);
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(1) > 0);
    let bits = state.city.tile_bits_at(52, 52).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(52, 52), Some(0x21));
}

#[test]
fn adjust_edit_map_sea_level_preserves_xtxt() {
    // Edit-map sea level leaves XTXT (PlaceTile tree clear does not write XTXT).
    let mut city = City::new_blank("/tmp/sc2000-emap-sea-xtxt.sc2", "ESeaT").expect("blank");
    city.set_altitude(52, 52, 10, 0);
    city.set_text_overlay(52, 52, 0x4f);
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert_eq!(state.city.text_overlay_at(52, 52), Some(0x4f));
}

#[test]
fn mac68k_adjust_edit_map_sea_level_preserves_xtxt() {
    // Shared edit-map sea level leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-emap-sea-xtxt.sc2", "MacESeaT").expect("blank");
    city.set_altitude(52, 52, 10, 0);
    city.set_text_overlay(52, 52, 0x4f);
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert_eq!(state.city.text_overlay_at(52, 52), Some(0x4f));
}

#[test]
fn mac68k_adjust_edit_map_sea_level_preserves_xtrf() {
    // Shared edit-map sea level is XTER/ALTM only; Mac68k must not invent an XTRF clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-emap-sea-xtrf.sc2", "MacESea").expect("blank");
    city.set_altitude(52, 52, 10, 0);
    city.set_traffic_density(52, 52, 0x21);
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert_eq!(state.city.traffic_density_at(52, 52), Some(0x21));
}

#[test]
fn stamp_building_underground_footprint_preserves_watered_and_xtrf() {
    // Underground stamp: XUND only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-und-foot-xtrf.sc2", "UndFoot").expect("blank");
    city.set_building(53, 53, 108); // 1x1 subway station / sub-rail access family
    city.set_tile_bits(53, 53, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(53, 53, 0x24);
    let mut state = test_state(city);
    state.stamp_building_underground_footprint(53, 53, UNDER_TILE_GREY_BOX_2);
    let bits = state.city.tile_bits_at(53, 53).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(53, 53), Some(0x24));
}

#[test]
fn clear_building_underground_footprint_preserves_watered_and_xtrf() {
    // Underground clear: XUND only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-und-clear-xtrf.sc2", "UndClr").expect("blank");
    city.set_building(54, 54, 249); // missile silo underground pieces
    city.set_underground(54, 54, UNDER_TILE_GREY_BOX_1);
    city.set_tile_bits(54, 54, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(54, 54, 0x26);
    let mut state = test_state(city);
    state.clear_building_underground_footprint(54, 54, UNDER_TILE_GREY_BOX_1);
    assert_eq!(state.city.underground_at(54, 54), 0);
    let bits = state.city.tile_bits_at(54, 54).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(54, 54), Some(0x26));
}

#[test]
fn refresh_underground_tile_preserves_watered_and_xtrf() {
    // Underground reshape: XUND only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-und-refresh-xtrf.sc2", "UndRef").expect("blank");
    city.set_underground(55, 55, 0x01); // pipe tile
    city.set_tile_bits(55, 55, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(55, 55, 0x2b);
    let mut state = test_state(city);
    state.refresh_underground_tile(55, 55, UndergroundKind::Pipes);
    let bits = state.city.tile_bits_at(55, 55).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(55, 55), Some(0x2b));
}

#[test]
fn refresh_network_tile_preserves_watered_and_xtrf() {
    // Network reshape: XBLD only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-net-refresh-xtrf.sc2", "NetRef").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(56, 56, 29);
    city.set_tile_bits(56, 56, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(56, 56, 0x2d);
    let mut state = test_state(city);
    state.refresh_network_tile(56, 56, NetworkKind::Road);
    let bits = state.city.tile_bits_at(56, 56).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(56, 56), Some(0x2d));
}

#[test]
fn map_tool_place_tree_stamps_crt_6_or_7() {
    // PE MapToolPlaceTree @ 0x43fbe1: rand()&1 + 6.
    let mut city = City::new_blank("/tmp/sc2000-tree-crt.sc2", "TreeCrt").expect("blank");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let expected = (preview.random_u16() & 1) as u8 + 6;
    assert_eq!(state.place_tree_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), expected);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn map_tool_place_tree_refuses_xbit_water_tile() {
    // PE MapToolPlaceTree @ 0x43fb73: refuse XBIT bit 0x04.
    let mut city = City::new_blank("/tmp/sc2000-tree-xbit.sc2", "TreeXbit").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0x04));
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at(20, 20), 0);
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn map_tool_place_tree_preserves_xtrf_like_pe_43fb40() {
    // PE MapToolPlaceTree @ 0x43fb40 never writes XTRF (`0x4ca940`).
    let mut city = City::new_blank("/tmp/sc2000-tree-xtrf.sc2", "TreeXtrf").expect("blank");
    city.set_city_funds(20_000);
    city.set_traffic_density(20, 20, 55);
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at(20, 20), 1);
    assert!(state.city.building_at(20, 20) >= 6);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(55));
}

#[test]
fn mac68k_map_tool_place_tree_preserves_xtrf_like_pe_43fb40() {
    // Shared PlaceTree path leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-tree-xtrf.sc2", "MacTreeX").expect("blank");
    city.set_city_funds(20_000);
    city.set_traffic_density(20, 20, 55);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_tree_at(20, 20), 1);
    assert!(state.city.building_at(20, 20) >= 6);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(55));
}

#[test]
fn map_tool_place_tree_preserves_watered_like_pe_43fb40() {
    // PE MapToolPlaceTree only tests XBIT bit 0x04; does not clear watered/piped.
    let mut city = City::new_blank("/tmp/sc2000-tree-watered.sc2", "TreeWatered").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at(20, 20), 1);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn map_tool_place_tree_preserves_xtxt_like_pe_place_tile() {
    // PE MapToolPlaceTree @ `0x43fc10` → PlaceTile `0x441f00`: leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-tree-xtxt.sc2", "TreeT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(20, 20, 0x2e);
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at(20, 20), 1);
    assert!(state.city.building_at(20, 20) >= 6);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2e));
}

#[test]
fn mac68k_map_tool_place_tree_preserves_xtxt_like_pe_place_tile() {
    // Shared PlaceTree PlaceTile path leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-tree-xtxt.sc2", "MacTreeT").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(20, 20, 0x2e);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_tree_at(20, 20), 1);
    assert!(state.city.building_at(20, 20) >= 6);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2e));
}

#[test]
fn map_tool_place_tree_skips_water_refresh_like_pe_43fb40() {
    // PE MapToolPlaceTree @ 0x43fb40: PlaceTile + redraw only — no invent water refresh.
    let mut city = City::new_blank("/tmp/sc2000-tree-skip-water.sc2", "TreeSkipW").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(50, 50, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at(20, 20), 1);
    let bits = state.city.tile_bits_at(50, 50).unwrap();
    assert!(bits.piped() && bits.watered());
}

#[test]
fn map_tool_place_tree_upgrades_existing_trees_and_refuses_pe_gates() {
    // PE MapToolPlaceTree @ 0x43fb99 / 0x43fbb4 / 0x43fbcd..0x43fc0d.
    let mut city = City::new_blank("/tmp/sc2000-tree-upgrade.sc2", "TreeUp").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 5);
    city.set_terrain(21, 20, 0x2e);
    city.set_building(22, 20, 8);
    city.set_building(23, 20, 11);
    city.set_building(24, 20, 13);
    city.set_building(25, 20, 3); // rubble < 6 → fresh CRT stamp
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at(20, 20), 0, "radioactivity 5");
    assert_eq!(state.place_tree_at(21, 20), 0, "XTER 0x2e");
    assert_eq!(state.place_tree_at(24, 20), 0, "XBLD >= 0x0d");

    assert_eq!(state.place_tree_at(22, 20), 1);
    assert_eq!(state.city.building_at(22, 20), 9, "6..10 increments");

    let mut preview = state.pe_process_crt;
    let expected_11 = (preview.random_u16() & 1) as u8 + 0x0b;
    assert_eq!(state.place_tree_at(23, 20), 1);
    assert_eq!(state.city.building_at(23, 20), expected_11);

    let mut preview = state.pe_process_crt;
    let expected_fresh = (preview.random_u16() & 1) as u8 + 6;
    assert_eq!(state.place_tree_at(25, 20), 1);
    assert_eq!(state.city.building_at(25, 20), expected_fresh);
}

#[test]
fn map_tool_place_tree_preserves_xzon() {
    // PE MapToolPlaceTree SetBuilding does not write XZON.
    let mut city = City::new_blank("/tmp/sc2000-tree-xzon.sc2", "TreeXzon").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at(20, 20), 1);
    assert!(matches!(state.city.building_at(20, 20), 6 | 7));
    assert_eq!(
        state.city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL
    );
}

#[test]
fn map_tool_place_tree_shift_clears_building_to_zero() {
    // PE MapToolPlaceTree @ 0x43fbc0: GetAsyncKeyState(VK_SHIFT) → SetBuilding(0).
    let mut city = City::new_blank("/tmp/sc2000-tree-shift.sc2", "TreeShift").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 9);
    city.set_building(21, 20, 112); // shift clears even XBLD >= 0x0d
    city.set_building(22, 20, 5);
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at_with_shift(20, 20, true), 1);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.place_tree_at_with_shift(21, 20, true), 1);
    assert_eq!(state.city.building_at(21, 20), 0);
    assert_eq!(state.place_tree_at_with_shift(22, 20, true), 0);
    assert_eq!(state.city.building_at(22, 20), 5);
}

#[test]
fn map_tool_place_tree_shift_preserves_watered_and_xtrf_like_pe_43fbc0() {
    // PE MapToolPlaceTree shift @ 0x43fbc9: SetBuilding(0) only — no watered/piped/XTRF mutate.
    let mut city = City::new_blank("/tmp/sc2000-tree-shift-xtrf.sc2", "TreeSX").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 9);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x37);
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at_with_shift(20, 20, true), 1);
    assert_eq!(state.city.building_at(20, 20), 0);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x37));
}

#[test]
fn map_tool_place_tree_shift_preserves_xtxt_like_pe_place_tile() {
    // Shift clear goes through PlaceTile `0x441f00` (XBLD 0); leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-tree-shift-xtxt.sc2", "TreeST").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 9);
    city.set_text_overlay(20, 20, 0x2f);
    let mut state = test_state(city);
    assert_eq!(state.place_tree_at_with_shift(20, 20, true), 1);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2f));
}

#[test]
fn mac68k_map_tool_place_tree_shift_preserves_xtxt_like_pe_place_tile() {
    // Shared shift PlaceTile clear leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-tree-shift-xtxt.sc2", "MacTreeST").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 9);
    city.set_text_overlay(20, 20, 0x2f);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_tree_at_with_shift(20, 20, true), 1);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x2f));
}

#[test]
fn mac68k_map_tool_place_tree_shift_preserves_xtrf_like_pe_43fbc0() {
    // Shared shift SetBuilding(0) leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-tree-sx.sc2", "MacTreeSX").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 9);
    city.set_traffic_density(20, 20, 0x37);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_tree_at_with_shift(20, 20, true), 1);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x37));
}

#[test]
fn map_tool_place_forest_scatters_crt_jittered_trees() {
    // PE FUN_0043fc80: 20 iterations of (rand&7)-(rand&7) around the click.
    let mut city = City::new_blank("/tmp/sc2000-forest.sc2", "Forest").expect("blank");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    let seed_before = state.pe_process_crt_seed();
    let stamped = state.place_forest_at_with_shift(20, 20, false);
    assert!(stamped > 0);
    assert_ne!(state.pe_process_crt_seed(), seed_before);
    assert_eq!(state.city.city_funds(), 20_000 - 3);
    let mut trees = 0;
    for y in 13..=27 {
        for x in 13..=27 {
            if is_tree_building(state.city.building_at(x, y)) {
                trees += 1;
            }
        }
    }
    assert!(trees > 0);
    assert!(trees <= stamped);
}

#[test]
fn map_tool_place_forest_skips_water_refresh_like_pe_43fc80() {
    // PE FUN_0043fc80: PlaceTile + redraw only — invent refresh_water_pipe_bits would clear.
    let mut city = City::new_blank("/tmp/sc2000-forest-water.sc2", "ForestWater").expect("blank");
    city.set_city_funds(20_000);
    // Far from forest jitter (±7 around 20,20) so the stamp loop won't touch it.
    city.set_tile_bits(50, 50, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    assert!(state.place_forest_at_with_shift(20, 20, false) > 0);
    let bits = state.city.tile_bits_at(50, 50).unwrap();
    assert!(bits.piped() && bits.watered());
}

#[test]
fn map_tool_place_forest_preserves_watered_and_xtrf_like_pe_43fc80() {
    // PE FUN_0043fc80: PlaceTile XBLD only — no watered/piped clear; no XTRF write.
    let mut city = City::new_blank("/tmp/sc2000-forest-xtrf.sc2", "ForestX").expect("blank");
    city.set_city_funds(20_000);
    for y in 13..=27 {
        for x in 13..=27 {
            city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_watered(true));
            city.set_traffic_density(x, y, 0x33);
        }
    }
    let mut state = test_state(city);
    assert!(state.place_forest_at_with_shift(20, 20, false) > 0);
    let mut checked = 0usize;
    for y in 13..=27 {
        for x in 13..=27 {
            let building = state.city.building_at(x, y);
            if !(6..=12).contains(&building) {
                continue;
            }
            checked += 1;
            let bits = state.city.tile_bits_at(x, y).unwrap();
            assert!(bits.piped(), "piped cleared at ({x},{y})");
            assert!(bits.watered(), "watered cleared at ({x},{y})");
            assert_eq!(
                state.city.traffic_density_at(x, y),
                Some(0x33),
                "XTRF mutated at ({x},{y})"
            );
        }
    }
    assert!(checked > 0, "expected at least one stamped forest tile");
}

#[test]
fn map_tool_place_forest_preserves_xtxt_like_pe_place_tile() {
    // PE FUN_0043fc80 → PlaceTile `0x441f00`: leave XTXT on stamped cells.
    let mut city = City::new_blank("/tmp/sc2000-forest-xtxt.sc2", "ForestT").expect("blank");
    city.set_city_funds(20_000);
    for y in 13..=27 {
        for x in 13..=27 {
            city.set_text_overlay(x, y, 0x31);
        }
    }
    let mut state = test_state(city);
    assert!(state.place_forest_at_with_shift(20, 20, false) > 0);
    let mut checked = 0usize;
    for y in 13..=27 {
        for x in 13..=27 {
            let building = state.city.building_at(x, y);
            if !(6..=12).contains(&building) {
                continue;
            }
            checked += 1;
            assert_eq!(
                state.city.text_overlay_at(x, y),
                Some(0x31),
                "XTXT cleared at ({x},{y})"
            );
        }
    }
    assert!(checked > 0, "expected at least one stamped forest tile");
}

#[test]
fn mac68k_map_tool_place_forest_preserves_xtxt_like_pe_place_tile() {
    // Shared PlaceForest → PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-forest-xtxt.sc2", "MacForestT").expect("blank");
    city.set_city_funds(20_000);
    for y in 13..=27 {
        for x in 13..=27 {
            city.set_text_overlay(x, y, 0x31);
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_forest_at_with_shift(20, 20, false) > 0);
    let mut checked = 0usize;
    for y in 13..=27 {
        for x in 13..=27 {
            let building = state.city.building_at(x, y);
            if !(6..=12).contains(&building) {
                continue;
            }
            checked += 1;
            assert_eq!(
                state.city.text_overlay_at(x, y),
                Some(0x31),
                "XTXT cleared at ({x},{y})"
            );
        }
    }
    assert!(checked > 0, "expected at least one stamped forest tile");
}

#[test]
fn map_tool_place_forest_shift_preserves_watered_and_xtrf_like_pe_43fc80() {
    // PE Forest shift arm @ 0x43fd95: XBLD adjust only — no watered/piped/XTRF mutate.
    let mut city = City::new_blank("/tmp/sc2000-forest-shift-xtrf.sc2", "ForestSX").expect("blank");
    city.set_city_funds(20_000);
    for y in 13..=27 {
        for x in 13..=27 {
            city.set_building(x, y, 9);
            city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_watered(true));
            city.set_traffic_density(x, y, 0x2a);
        }
    }
    let mut state = test_state(city);
    assert!(state.place_forest_at_with_shift(20, 20, true) > 0);
    let mut checked = 0usize;
    for y in 13..=27 {
        for x in 13..=27 {
            let building = state.city.building_at(x, y);
            // Shift: 9→8, or 6→0; only assert tiles the tool touched (changed from 9).
            if building == 9 {
                continue;
            }
            checked += 1;
            let bits = state.city.tile_bits_at(x, y).unwrap();
            assert!(bits.piped(), "piped cleared at ({x},{y})");
            assert!(bits.watered(), "watered cleared at ({x},{y})");
            assert_eq!(
                state.city.traffic_density_at(x, y),
                Some(0x2a),
                "XTRF mutated at ({x},{y})"
            );
        }
    }
    assert!(checked > 0, "expected at least one shift-arm forest tile");
}

#[test]
fn map_tool_place_forest_shift_preserves_xtxt_like_pe_place_tile() {
    // PE Forest shift → PlaceTile XBLD adjust; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-forest-shift-xtxt.sc2", "ForestST").expect("blank");
    city.set_city_funds(20_000);
    for y in 13..=27 {
        for x in 13..=27 {
            city.set_building(x, y, 9);
            city.set_text_overlay(x, y, 0x46);
        }
    }
    let mut state = test_state(city);
    assert!(state.place_forest_at_with_shift(20, 20, true) > 0);
    let mut checked = 0usize;
    for y in 13..=27 {
        for x in 13..=27 {
            if state.city.building_at(x, y) == 9 {
                continue;
            }
            checked += 1;
            assert_eq!(
                state.city.text_overlay_at(x, y),
                Some(0x46),
                "XTXT cleared at ({x},{y})"
            );
        }
    }
    assert!(checked > 0, "expected at least one shift-arm forest tile");
}

#[test]
fn mac68k_map_tool_place_forest_shift_preserves_xtxt_like_pe_place_tile() {
    // Shared forest shift PlaceTile leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-forest-shift-xtxt.sc2", "MacForestST").expect("blank");
    city.set_city_funds(20_000);
    for y in 13..=27 {
        for x in 13..=27 {
            city.set_building(x, y, 9);
            city.set_text_overlay(x, y, 0x46);
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_forest_at_with_shift(20, 20, true) > 0);
    let mut checked = 0usize;
    for y in 13..=27 {
        for x in 13..=27 {
            if state.city.building_at(x, y) == 9 {
                continue;
            }
            checked += 1;
            assert_eq!(
                state.city.text_overlay_at(x, y),
                Some(0x46),
                "XTXT cleared at ({x},{y})"
            );
        }
    }
    assert!(checked > 0, "expected at least one shift-arm forest tile");
}

#[test]
fn mac68k_map_tool_place_forest_shift_preserves_xtrf_like_pe_43fd95() {
    // Shared forest shift SetBuilding only; Mac68k must not invent an XTRF clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-forest-sx.sc2", "MacForestSX").expect("blank");
    city.set_city_funds(20_000);
    for y in 13..=27 {
        for x in 13..=27 {
            city.set_building(x, y, 9);
            city.set_traffic_density(x, y, 0x2a);
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_forest_at_with_shift(20, 20, true) > 0);
    let mut checked = 0usize;
    for y in 13..=27 {
        for x in 13..=27 {
            if state.city.building_at(x, y) == 9 {
                continue;
            }
            checked += 1;
            assert_eq!(
                state.city.traffic_density_at(x, y),
                Some(0x2a),
                "XTRF mutated at ({x},{y})"
            );
        }
    }
    assert!(checked > 0, "expected at least one shift-arm forest tile");
}

#[test]
fn mac68k_map_tool_place_forest_preserves_xtrf_like_pe_43fc80() {
    // Shared PlaceForest path leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-forest-xtrf.sc2", "MacForestX").expect("blank");
    city.set_city_funds(20_000);
    for y in 13..=27 {
        for x in 13..=27 {
            city.set_traffic_density(x, y, 0x41);
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_forest_at_with_shift(20, 20, false) > 0);
    let mut checked = 0usize;
    for y in 13..=27 {
        for x in 13..=27 {
            if !(6..=12).contains(&state.city.building_at(x, y)) {
                continue;
            }
            checked += 1;
            assert_eq!(
                state.city.traffic_density_at(x, y),
                Some(0x41),
                "Mac68k forest mutated XTRF at ({x},{y})"
            );
        }
    }
    assert!(checked > 0);
}

#[test]
fn map_tool_place_tree_place_arm_matches_pe_xbld_ladder() {
    // PE MapToolPlaceTree / Forest place arm @ 0x43fbcd / 0x43fd5c.
    let mut crt = Win95CrtRand::new(0x1234_5678);
    let mut preview = crt;
    assert_eq!(
        pe_tree_place_xbld(0, &mut crt),
        Some((preview.random_u16() & 1) as u8 + 6)
    );
    assert_eq!(pe_tree_place_xbld(8, &mut crt), Some(9));
    let mut preview = crt;
    assert_eq!(
        pe_tree_place_xbld(11, &mut crt),
        Some((preview.random_u16() & 1) as u8 + 0x0b)
    );
    assert_eq!(pe_tree_place_xbld(13, &mut crt), None);
}

#[test]
fn map_tool_place_forest_shift_arm_matches_pe_0x43fd95() {
    // PE Forest shift: ==6 → 0; 7..12 → dec; <6 / >=0xd skip.
    assert_eq!(pe_forest_shift_xbld(5), None);
    assert_eq!(pe_forest_shift_xbld(6), Some(0));
    assert_eq!(pe_forest_shift_xbld(9), Some(8));
    assert_eq!(pe_forest_shift_xbld(12), Some(11));
    assert_eq!(pe_forest_shift_xbld(13), None);
}

#[test]
fn map_tool_place_water_clears_zone_type_keeps_corners() {
    // PE MapToolPlaceWater @ 0x43fedf: XZON &= 0xf0.
    let mut city = City::new_blank("/tmp/sc2000-water-xzon.sc2", "WaterXzon").expect("blank");
    city.set_city_funds(20_000);
    city.set_zone(20, 20, ZONE_COMMERCIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.zone_at(20, 20).unwrap().zone_id(), 0);
    assert_eq!(
        state.city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_CORNER_ALL
    );
}

#[test]
fn marina_requires_mixed_land_and_water_footprint() {
    let origin = (20, 20);

    let mut dry_state =
        test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city"));
    dry_state.tool = EditTool::Marina;
    let dry = dry_state
        .tool_placement_at(origin.0, origin.1)
        .expect("dry marina placement");
    assert!(dry.valid_tiles.is_empty());
    assert_eq!(dry.invalid_tiles.len(), 9);
    assert!(!dry_state.apply_tool_at_tile(origin.0, origin.1));

    let mut all_water_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(origin.0, origin.1, 3).expect("building footprint")
    {
        all_water_city.set_terrain(x, y, 0x30);
        all_water_city.set_tile_bits(x, y, TileBits::new(0).with_water_tile(true));
    }
    let mut all_water_state = test_state(all_water_city);
    all_water_state.tool = EditTool::Marina;
    let all_water = all_water_state
        .tool_placement_at(origin.0, origin.1)
        .expect("all-water marina placement");
    assert!(all_water.valid_tiles.is_empty());
    assert_eq!(all_water.invalid_tiles.len(), 9);
    assert!(!all_water_state.apply_tool_at_tile(origin.0, origin.1));

    let mut shore_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    shore_city.set_city_funds(20_000);
    for x in origin.0..origin.0 + 3 {
        shore_city.set_terrain(x, origin.1, 0x30);
        shore_city.set_tile_bits(x, origin.1, TileBits::new(0).with_water_tile(true));
    }
    let mut shore_state = test_state(shore_city);
    shore_state.tool = EditTool::Marina;
    let shore = shore_state
        .tool_placement_at(origin.0, origin.1)
        .expect("shore marina placement");
    assert_eq!(shore.preview, ToolPreview::Building(248));
    assert_eq!(shore.valid_tiles.len(), 9);
    assert!(shore.invalid_tiles.is_empty());
    assert!(shore_state.apply_tool_at_tile(origin.0, origin.1));
    assert_eq!(shore_state.city.city_funds(), 19_000);

    for (x, y) in building_footprint_from_origin(origin.0, origin.1, 3).expect("building footprint")
    {
        assert_eq!(shore_state.city.building_at(x, y), 248);
    }
}

#[test]
fn citytool_marina_uses_place_tile_size_and_water_tile_gate() {
    // PE CityTool @ `0x440731` → PlaceTile-size: marina water-tile count + XBIT `|=0xe0`,
    // then the full action clears transient powered/served-water bits without sources.
    // Bare water XTER without water-tile is not a water cell (`0x440d55`).
    let mut city = City::new_blank("/tmp/sc2000-ct-marina.sc2", "CtMarina").expect("blank");
    city.set_city_funds(20_000);
    city.set_view_rotation(0);
    for x in 20..23 {
        city.set_terrain(x, 20, 0x30);
        city.set_tile_bits(
            x,
            20,
            TileBits::new(0).with_water_tile(true).with_watered(true),
        );
    }
    // Dry corner with bare water XTER must refuse PlaceTile-size / full placement.
    city.set_terrain(22, 22, 0x30);
    let mut refuse = test_state(city.clone());
    refuse.tool = EditTool::Marina;
    let refuse_place = refuse.tool_placement_at(20, 20).unwrap();
    assert!(!refuse_place.invalid_tiles.is_empty());
    assert_eq!(refuse.place_building_at(20, 20, MARINA_BUILDING_ID), 0);

    city.set_terrain(22, 22, 0);
    city.set_traffic_density(21, 21, 0x41);
    let mut state = test_state(city);
    state.tool = EditTool::Marina;
    assert_eq!(state.place_building_at(20, 20, MARINA_BUILDING_ID), 9);
    let corner_bits = pe_choose_and_place_corner_bits(0);
    for &(tx, ty) in &building_footprint_from_origin(20, 20, 3).expect("3x3") {
        assert_eq!(state.city.building_at(tx, ty), MARINA_BUILDING_ID);
        let bits = state.city.tile_bits_at(tx, ty).unwrap();
        assert_eq!(bits.raw_bits() & 0xe0, 0xa0);
        let expect = building_zone_bits_for_tile(20, 20, tx, ty, 3, corner_bits);
        assert_eq!(state.city.zone_at(tx, ty).unwrap().raw_bits(), expect);
    }
    assert_eq!(state.city.traffic_density_at(21, 21), Some(0x41));
    // PE PlaceTile-size @ `0x440ea0` → microsim allocate `0x44cf60`; marina class writes XTXT.
    let xtxt = state.city.text_overlay_at(20, 20).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES, "marina microsim XTXT {xtxt}");
    for &(tx, ty) in &building_footprint_from_origin(20, 20, 3).expect("3x3") {
        assert_eq!(
            state.city.text_overlay_at(tx, ty),
            Some(xtxt),
            "marina footprint XTXT"
        );
    }
}

#[test]
fn mac68k_zone_drag_preview_prices_water_candidate_skipped_by_commit() {
    let mut city = City::new_blank("/tmp/sc2000-mac-zone-price-water.sc2", "MacZoneWater")
        .expect("blank city");
    city.set_city_funds(100);
    city.set_tile_bits(11, 10, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);
    state.update_zone_drag_to((11, 10));

    assert_eq!(state.zone_drag_cost(), Some(20));
    assert_eq!(state.finish_zone_drag(), 1);
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().zone_id(),
        ZONE_COMMERCIAL_DENSE
    );
    assert_eq!(state.city.zone_at(11, 10).unwrap().zone_id(), 0);
    assert_eq!(state.city.city_funds(), 80);
}

#[test]
fn mac68k_zone_drag_cannot_start_on_water_radioactivity_or_military_zone() {
    for (building, zone, water) in [(0, 0, true), (5, 0, false), (0, ZONE_MILITARY, false)] {
        let mut city = City::new_blank("/tmp/citygame-mac-zone-origin.sc2", "MacZoneStart")
            .expect("blank city");
        city.set_city_funds(100);
        city.set_building(10, 10, building);
        city.set_zone(10, 10, zone);
        city.set_tile_bits(10, 10, TileBits::new(0).with_water_tile(water));
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.tool = EditTool::ZoneCommercialDense;

        state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);
        state.update_zone_drag_to((11, 10));

        assert_eq!(state.zone_drag_cost(), None);
        assert_eq!(state.finish_zone_drag(), 0);
        assert_eq!(state.city.zone_at(10, 10).unwrap().zone_id(), zone);
        assert_eq!(state.city.zone_at(11, 10).unwrap().zone_id(), 0);
        assert_eq!(state.city.city_funds(), 100);
    }
}

#[test]
fn win95_zone_drag_preview_prices_water_candidate_skipped_by_commit() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(100);
    city.set_tile_bits(10, 10, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);
    state.update_zone_drag_to((11, 10));

    assert_eq!(state.zone_drag_cost(), Some(20));
    assert_eq!(state.finish_zone_drag(), 1);
    assert_eq!(state.city.zone_at(10, 10).unwrap().zone_id(), 0);
    assert_eq!(
        state.city.zone_at(11, 10).unwrap().zone_id(),
        ZONE_COMMERCIAL_DENSE
    );
    assert_eq!(state.city.city_funds(), 80);
}

#[test]
fn win95_zone_drag_charges_preview_even_when_water_blocks_all_writes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(100);
    city.set_tile_bits(10, 10, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    state.tool = EditTool::ZoneCommercialDense;

    state.begin_zone_drag(10, 10, ZONE_COMMERCIAL_DENSE);

    assert_eq!(state.zone_drag_cost(), Some(10));
    assert_eq!(state.finish_zone_drag(), 0);
    assert_eq!(state.city.zone_at(10, 10).unwrap().zone_id(), 0);
    assert_eq!(state.city.city_funds(), 90);
}

#[test]
fn pe_network_surface_terrain_table_matches_448e20() {
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x05, 0),
        Some((0x0d, true))
    );
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x15, 0),
        Some((0x1d, true))
    );
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x25, 0),
        Some((0x0d, true))
    );
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x19, 0),
        Some((0x12, false))
    );
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x29, 1),
        Some((0x11, false))
    );
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x2a, 2),
        Some((0x12, false))
    );
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x2b, 0),
        Some((0x14, false))
    );
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x0c, 0),
        Some((0x04, false))
    );
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x0c, 1),
        Some((0x01, false))
    );
    assert_eq!(
        pe_network_surface_terrain_result_448e20(0x1c, 2),
        Some((0x14, false))
    );
    assert_eq!(pe_network_surface_terrain_result_448e20(0x04, 0), None);
    assert_eq!(pe_network_surface_terrain_result_448e20(0x30, 0), None);
}

#[test]
fn pe_network_strokes_reclassify_surface_terrain_and_charge_shared_surcharge() {
    for (kind, tool, base_cost) in [
        (NetworkKind::Power, EditTool::Power, 2),
        (NetworkKind::Road, EditTool::Road, 10),
        (NetworkKind::Rail, EditTool::Rail, 25),
    ] {
        let mut city =
            City::new_blank("/tmp/sc2000-network-xter.sc2", "NetworkXter").expect("blank");
        let stroke_cost = base_cost * 2 + PE_NETWORK_SURFACE_CLEAR_COST;
        city.set_city_funds(stroke_cost as i32);
        city.set_terrain(10, 10, 0x05);
        city.set_tile_bits(
            10,
            10,
            TileBits::new(0)
                .with_water_tile(kind != NetworkKind::Power)
                .with_piped(true)
                .with_watered(true),
        );
        city.set_traffic_density(10, 10, 0x57);
        let mut state = test_state(city);
        state.tool = tool;

        state.begin_network_drag(10, 10, kind);
        state.update_network_drag_to((11, 10));
        let preview_cost = if kind == NetworkKind::Power {
            // The Win95 mode-2 Power walker visits only the starting tile in
            // this slope fixture; Road/Rail use their shared two-cell stroke.
            base_cost + PE_NETWORK_SURFACE_CLEAR_COST
        } else {
            stroke_cost
        };
        assert_eq!(
            state.network_drag_cost(),
            Some((kind, preview_cost)),
            "{kind:?} preview includes shared terrain work"
        );
        state.cancel_network_drag();

        if kind == NetworkKind::Power {
            state.begin_network_drag(10, 10, kind);
            state.update_network_drag_to((11, 10));
            assert_eq!(state.finish_network_drag().unwrap().1, 1);
            assert_eq!(state.city.city_funds(), 2, "Power walker tile billing");
            assert_eq!(state.city.terrain_at(10, 10), 0x0d);
            let bits = state.city.tile_bits_at(10, 10).expect("XBIT");
            assert!(!bits.water_tile());
            assert!(bits.piped());
            assert!(bits.watered());
            assert_eq!(state.city.traffic_density_at(10, 10), Some(0x57));
            continue;
        }

        assert_eq!(
            state.place_network_path(&[(10, 10), (11, 10)], kind),
            2,
            "{kind:?}"
        );
        assert_eq!(state.city.city_funds(), 0, "{kind:?} stroke billing");
        assert_eq!(state.city.terrain_at(10, 10), 0x0d, "{kind:?} XTER remap");
        let bits = state.city.tile_bits_at(10, 10).expect("XBIT");
        assert!(!bits.water_tile(), "{kind:?} clears only surface-water bit");
        assert!(bits.piped(), "{kind:?} preserves piped bit");
        assert!(bits.watered(), "{kind:?} preserves watered bit");
        assert_eq!(state.city.traffic_density_at(10, 10), Some(0x57));
    }
}

#[test]
fn pe_pipe_stroke_reclassifies_xter_and_bills_terrain_surcharge() {
    let mut city = City::new_blank("/tmp/sc2000-pipe-xter.sc2", "PipeXter").expect("blank");
    city.set_city_funds(28);
    city.set_terrain(10, 10, 0x05);
    city.set_tile_bits(10, 10, TileBits::new(0x04));
    city.set_traffic_density(10, 10, 0x57);
    let mut state = test_state(city);
    state.tool = EditTool::Pipes;

    state.begin_underground_drag(10, 10, UndergroundKind::Pipes);
    assert_eq!(
        state.underground_drag_cost(),
        Some((UndergroundKind::Pipes, 28)),
        "pipe stroke includes the native surface-work surcharge"
    );
    state.cancel_underground_drag();
    assert_eq!(
        state.place_underground_path(&[(10, 10)], UndergroundKind::Pipes),
        1
    );
    assert_eq!(state.city.city_funds(), 0, "$3 pipe + $25 surface work");
    assert_eq!(state.city.terrain_at(10, 10), 0x0d);
    assert!(!state.city.tile_bits_at(10, 10).unwrap().water_tile());
    assert_eq!(state.city.traffic_density_at(10, 10), Some(0x57));
}

#[test]
fn pe_pipe_stroke_walk_uses_directional_terrain_gates() {
    let mut city =
        City::new_blank("/tmp/sc2000-pipe-walk-gate.sc2", "PipeWalkGate").expect("blank");
    city.set_terrain(11, 10, 0x02);
    city.set_city_funds(100);
    let expected = vec![(10, 10), (10, 11), (11, 11), (12, 11), (12, 12), (13, 12)];
    assert_eq!(
        pe_pipe_stroke_path_448bc0_448f50(&city, (10, 10), (13, 12)),
        expected
    );

    let mut state = test_state(city);
    state.tool = EditTool::Pipes;
    state.begin_underground_drag(10, 10, UndergroundKind::Pipes);
    state.update_underground_drag_to((13, 12));
    let state_path =
        state.underground_drag_path_for_kind((10, 10), (13, 12), UndergroundKind::Pipes, true);
    assert_eq!(state_path, expected);
    assert_eq!(
        state
            .underground_drag_placement_for_path(&state_path, UndergroundKind::Pipes)
            .valid_tiles,
        expected
    );
    assert_eq!(
        state.underground_drag_cost(),
        Some((UndergroundKind::Pipes, 18))
    );
    assert_eq!(
        state.finish_underground_drag(),
        Some((UndergroundKind::Pipes, expected.len()))
    );
    assert_eq!(state.city.underground_at(11, 10), 0);
    for &(x, y) in &expected {
        assert_ne!(state.city.underground_at(x, y), 0, "({x}, {y})");
    }
}

#[test]
fn hydro_power_only_places_on_waterfall_terrain() {
    // PE MapTool hydro @ `0x44146e`: XTER `0x2E` or `0x3E` only.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_terrain(10, 10, 0x00);
    city.set_terrain(11, 10, 0x30);
    city.set_terrain(12, 10, 0x3e);
    city.set_terrain(13, 10, 0x40);
    city.set_terrain(14, 10, 0x2e);
    let mut state = test_state(city);
    state.tool = EditTool::PowerPlantHydro;

    for tile in [(10, 10), (11, 10), (13, 10)] {
        let placement = state
            .tool_placement_at(tile.0, tile.1)
            .expect("hydro placement");
        assert!(placement.valid_tiles.is_empty(), "{tile:?}");
        assert_eq!(placement.invalid_tiles, vec![tile], "{tile:?}");
        assert!(!state.apply_tool_at_tile(tile.0, tile.1), "{tile:?}");
    }

    let placement = state
        .tool_placement_at(12, 10)
        .expect("waterfall hydro placement");
    assert_eq!(placement.preview, ToolPreview::Building(199));
    assert_eq!(placement.valid_tiles, vec![(12, 10)]);
    assert!(placement.invalid_tiles.is_empty());
    assert!(state.apply_tool_at_tile(12, 10));
    assert_eq!(state.city.building_at(12, 10), 199);
    assert_eq!(
        state.city.zone_at(12, 10).unwrap().raw_bits(),
        ZONE_CORNER_ALL
    );
    assert_eq!(state.city.city_funds(), 19_600);
    assert!(
        state
            .city
            .tile_bits_at(12, 10)
            .is_some_and(|bits| bits.powerable()),
        "PE MapTool hydro ORs XBIT powerable `0x80`"
    );

    let placement = state
        .tool_placement_at(14, 10)
        .expect("0x2e waterfall hydro placement");
    assert_eq!(placement.valid_tiles, vec![(14, 10)]);
    assert!(state.apply_tool_at_tile(14, 10));
    assert_ne!(state.city.building_at(14, 10), 0);
}

#[test]
fn highway_xter_0x0e_uses_gate_0xf_class2_not_slope_fallthrough() {
    // `0x4e8a58[0x0e]` is category 0, so the gate stays `0xf`. A one-origin
    // drag keeps FUN_0044C680's zero-delta y direction, selecting class 2
    // underlay `0x49`. Nonzero XTER still rewrites `0x0d`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-xter0e.sc2", "HwyXter0e").expect("blank");
    city.set_city_funds(50_000);
    let tiles = highway_segment_tiles((20, 20), NetworkAxis::GridX);
    for &(x, y) in &tiles {
        city.set_terrain(x, y, 0x0e);
    }
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    for &(x, y) in &tiles {
        assert_eq!(
            state.city.building_at(x, y),
            0x49,
            "gate 0xf class 2 at ({x},{y})"
        );
        assert_eq!(
            state.city.terrain_at(x, y),
            0x0d,
            "altitude bump at ({x},{y})"
        );
    }
}

#[test]
fn highway_xter_slope_gate_1_to_4_like_pe() {
    // PE PlaceHighway AtCoordinates @ `0x44bf47`–`0x44bf55`: XTER `1..=4`
    // else dialog `0x74`; walk needs paired terminal XTER on the first step.
    let mut city = City::new_blank("/tmp/sc2000-hwy-xter.sc2", "HwyXter").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);

    // start XTER → dir step → terminal XTER `((start+1)&3)+1`
    let cases = [
        (1u8, -1isize, 0isize, 3u8),
        (2, 0, -1, 4),
        (3, 1, 0, 1),
        (4, 0, 1, 2),
    ];
    for (slope, dx, dy, terminal) in cases {
        state.city.set_terrain(20, 20, slope);
        let nx = (20 + dx) as usize;
        let ny = (20 + dy) as usize;
        state.city.set_terrain(nx, ny, terminal);
        assert!(
            state.can_place_highway_slope_atcoordinates(20, 20),
            "XTER {slope:#04x} with terminal {terminal:#04x} must pass"
        );
        state.city.set_terrain(nx, ny, 0);
        assert!(
            !state.can_place_highway_slope_atcoordinates(20, 20),
            "XTER {slope:#04x} without terminal must refuse dialog 0x72"
        );
    }

    // AtCoordinates dialog `0x74` is not CityTool. Non-slope dry `0x06` stamps.
    state.city.set_terrain(20, 20, 0x06);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_terrain(20, 20, 0x0d);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state.city.set_building(20, 20, 74);
    assert!(
        state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false),
        "existing highway on XTER 0x0d continues like 0x4640b5"
    );
    state.city.set_building(20, 20, 0);

    // Water without bridge invent: refuse (PE would also fail `1..=4`).
    state.city.set_terrain(20, 20, 0x10);
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, true));
}

#[test]
fn highway_slope_refuses_altm_tunnel_bits_like_pe() {
    // PE PlaceHighway slope walk @ `0x44bfcf`: `ALTM` `ah & 0x7c` → dialog `0x6f`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-tunnel.sc2", "HwyTun").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 2);
    city.set_terrain(20, 19, 4);
    let mut state = test_state(city);
    assert!(state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_tunnel_levels(20, 20, 1);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_tunnel_levels(20, 20, 0);
    assert!(state.can_place_highway_slope_atcoordinates(20, 20));
    // Flat invent retainer ignores ALTM tunnel bits (not PlaceHighway AtCoordinates).
    state.city.set_terrain(20, 20, 0);
    state.city.set_tunnel_levels(20, 20, 3);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
}

#[test]
fn highway_slope_first_step_refuses_steep_alt_delta_like_pe() {
    // PE land-delta `> 0x1e` on a climbed walk tile → dialog `0x71`.
    // XTER 2 → dir 0 → step (0,-1). Keep post-climb tile ≤ base so walk ends.
    let mut city = City::new_blank("/tmp/sc2000-hwy-delta.sc2", "HwyDelta").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 2);
    city.set_terrain(20, 18, 4);
    city.set_altitude(20, 20, 0, 0);
    city.set_altitude(20, 19, 0x1e, 0);
    city.set_altitude(20, 18, 0, 0);
    let mut state = test_state(city);
    assert!(state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_altitude(20, 19, 0x1f, 0);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_altitude(20, 19, 5, 0);
    state.city.set_tunnel_levels(20, 19, 2);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
}

#[test]
fn highway_slope_delta1_refuses_xbld_bands_like_pe() {
    // PE @ `0x44bfea`–`0x44c057`: when land-delta == 1, refuse XBLD in
    // `1..=0x20` / `0x22` / `0x23` (dialog `0x73`).
    let mut city = City::new_blank("/tmp/sc2000-hwy-d1.sc2", "HwyD1").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 2);
    city.set_terrain(20, 18, 4);
    city.set_altitude(20, 20, 5, 0);
    city.set_altitude(20, 19, 6, 0); // delta == 1 on climbed tile
    city.set_altitude(20, 18, 5, 0);
    let mut state = test_state(city);
    assert!(state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_building(20, 19, 0x0c);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_building(20, 19, 0x21);
    assert!(state.can_place_highway_slope_atcoordinates(20, 20));
    state.city.set_building(20, 19, 0x22);
    assert!(!state.can_place_highway_slope_atcoordinates(20, 20));
}

#[test]
fn highway_slope_places_xbld_xter_plus_0x3e_and_altm_bit10() {
    // PE @ `0x44c2a6`–`0x44c2e8`: PlaceTile `XTER+0x3e`, ALTM bit10 after debit.
    // PE @ `0x44c347`–`0x44c3db`: climb ALTM + terminal `((XTER+1)&3)+0x3f`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-place.sc2", "HwyPlace").expect("blank");
    city.set_terrain(20, 20, 2);
    city.set_terrain(20, 19, 4);
    city.set_city_funds(200);
    let mut state = test_state(city);
    assert!(state.place_highway_slope_atcoordinates(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0x40); // 2 + 0x3e
    assert_eq!(state.city.tunnel_levels_at(20, 20), 1);
    // Terminal north: XBLD 0x42, ALTM levels land-base+1 == 1.
    assert_eq!(state.city.building_at(20, 19), 0x42);
    assert_eq!(state.city.tunnel_levels_at(20, 19), 1);
    assert_eq!(state.city.city_funds(), 0);
    // Tunnel bit now blocks a second PlaceHighway slope attempt.
    state.city.set_city_funds(200);
    assert!(!state.place_highway_slope_atcoordinates(20, 20));

    // XTER 1 → origin 0x3f, terminal west `((1+1)&3)+0x3f` = 0x41.
    let mut city = City::new_blank("/tmp/sc2000-hwy-place2.sc2", "HwyPlace2").expect("blank");
    city.set_terrain(20, 20, 1);
    city.set_terrain(19, 20, 3);
    city.set_city_funds(200);
    let mut state = test_state(city);
    assert!(state.place_highway_slope_atcoordinates(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0x3f);
    assert_eq!(state.city.tunnel_levels_at(20, 20), 1);
    assert_eq!(state.city.building_at(19, 20), 0x41);
    assert_eq!(state.city.tunnel_levels_at(19, 20), 1);
}

#[test]
fn highway_slope_climb_writes_altm_levels_land_minus_base_plus_one() {
    // PE climb loop @ `0x44c37d`–`0x44c3b2`: levels = land - base + 1.
    let mut city = City::new_blank("/tmp/sc2000-hwy-climb.sc2", "HwyClimb").expect("blank");
    city.set_terrain(20, 20, 2);
    city.set_terrain(20, 18, 4);
    city.set_altitude(20, 20, 5, 0);
    city.set_altitude(20, 19, 6, 0);
    city.set_altitude(20, 18, 5, 0);
    city.set_city_funds(300);
    let mut state = test_state(city);
    assert!(state.place_highway_slope_atcoordinates(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0x40);
    assert_eq!(state.city.tunnel_levels_at(20, 20), 1);
    // Mid climb tile (20,19): land 6 → levels 2; terminal (20,18): levels 1, XBLD 0x42.
    assert_eq!(state.city.tunnel_levels_at(20, 19), 2);
    assert_eq!(state.city.building_at(20, 18), 0x42);
    assert_eq!(state.city.tunnel_levels_at(20, 18), 1);
}

#[test]
fn highway_slope_terminal_altm_bit10_replaces_lower_climb_formula() {
    // PE `0x44c3f8`–`0x44c413` runs after the climb formula. Terminal land
    // below the origin base is formula 0, then bit 10 forces level 1.
    let mut city = City::new_blank("/tmp/sc2000-hwy-term-bit.sc2", "HwyTermBit").expect("blank");
    city.set_terrain(20, 20, 2);
    city.set_terrain(20, 18, 4);
    city.set_altitude(20, 20, 8, 8);
    city.set_altitude(20, 19, 9, 8);
    city.set_altitude(20, 18, 7, 8);
    city.set_city_funds(500);
    let mut state = test_state(city);
    assert!(state.place_highway_slope_atcoordinates(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0x40);
    assert_eq!(state.city.tunnel_levels_at(20, 19), 2);
    assert_eq!(state.city.building_at(20, 18), 0x42);
    assert_eq!(state.city.tunnel_levels_at(20, 18), 1);
}

#[test]
fn highway_flat_refuses_xbit_water_tile_bit_like_citytool() {
    // PE CityToolPlaceHighway @ `0x462232`: XBIT bit 2 (`water_tile`) on footprint.
    let mut city = City::new_blank("/tmp/sc2000-hwy-xbit.sc2", "HwyXbit").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    let bits = state.city.tile_bits_at(20, 20).unwrap_or_default();
    state.city.set_tile_bits(20, 20, bits.with_water_tile(true));
    assert!(!state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    state
        .city
        .set_tile_bits(20, 20, bits.with_water_tile(false));
    assert!(state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false));
    let east = state.city.tile_bits_at(21, 20).unwrap_or_default();
    state.city.set_tile_bits(21, 20, east.with_water_tile(true));
    assert!(
        !state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false),
        "sibling XBIT bit 2 refuses the even 2x2"
    );
    state
        .city
        .set_tile_bits(21, 20, east.with_water_tile(false));
    state.city.set_building(21, 20, 5);
    assert!(
        !state.can_place_highway_tile(20, 20, NetworkAxis::GridX, false, false),
        "sibling building 5 is gate 0x8000"
    );
    // Slope AtCoordinates does not use the CityTool water-bit gate.
    state.city.set_terrain(20, 20, 2);
    state.city.set_terrain(20, 19, 4);
    state.city.set_tile_bits(20, 20, bits.with_water_tile(true));
    assert!(state.can_place_highway_slope_atcoordinates(20, 20));
}

#[test]
fn user_pipe_over_building_plumbing_keeps_watered_building_component() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(10, 10, 220), 1);
    power_water_source_for_test(&mut state.city, 10, 10);
    for x in 11..=15 {
        state.city.set_building(x, 10, 112);
        state.city.set_zone(x, 10, 1);
    }
    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_simulation_service_bits();
    for x in 11..=15 {
        assert!(
            state
                .city
                .tile_bits_at(x, 10)
                .is_some_and(|bits| bits.watered()),
            "expected generated plumbing to be watered before user pipe at ({x},10)"
        );
    }

    assert_eq!(
        state.place_underground_path(&[(12, 10), (13, 10), (14, 10)], UndergroundKind::Pipes),
        3
    );

    for x in 11..=15 {
        assert!(
            state
                .city
                .tile_bits_at(x, 10)
                .is_some_and(|bits| bits.watered()),
            "expected generated plumbing to remain watered after user pipe at ({x},10)"
        );
    }
    assert_eq!(
        underground_render_tile(&state.city, 13, 10, 0).map(|tile| tile.source),
        Some(UndergroundTileSource::UserLaid)
    );
    assert_eq!(
        underground_building_pipe_underlay_tile(&state.city, 13, 10, 0).map(|tile| tile.source),
        Some(UndergroundTileSource::BuildingInternal)
    );
}

#[test]
fn pipe_placement_dirties_the_affected_render_area() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.clear_dirty();

    assert_eq!(
        state.place_underground_path(&[(10, 10), (11, 10)], UndergroundKind::Pipes),
        2
    );

    let dirty: Vec<_> = state.city.dirty_tile_coords().collect();
    assert!(dirty.contains(&(10, 10)));
    assert!(dirty.contains(&(11, 10)));
    assert!(
        dirty
            .iter()
            .all(|&(x, y)| (9..=13).contains(&x) && (9..=12).contains(&y))
    );
}

#[test]
fn saved_xbit_piped_tiles_only_render_base_pipe_on_structural_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_tile_bits(10, 10, crate::city::TileBits::new(0b0010_0000));
    assert_eq!(underground_render_tile(&city, 10, 10, 0), None);

    city.set_building(10, 10, 112);
    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );

    city.set_building(
        11,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_tile_bits(11, 10, crate::city::TileBits::new(0b0010_0000));
    assert_eq!(underground_render_tile(&city, 11, 10, 0), None);
}

#[test]
fn water_refresh_preserves_loaded_base_pipe_bits() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 112);
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0b0010_0000));
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(10, 10).unwrap().watered());
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
fn water_refresh_clears_stale_base_pipe_bits_on_empty_land() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0b0010_0000));
    let mut state = test_state(city);

    state.refresh_simulation_service_bits();

    assert!(!state.city.tile_bits_at(10, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(10, 10).unwrap().watered());
    assert_eq!(underground_render_tile(&state.city, 10, 10, 0), None);
}

#[test]
fn default_city_pipe_eligible_tiles_have_visible_plumbing_in_all_rotations() {
    let path = paths::default_city_path();
    if !path.exists() {
        return;
    }
    for rotation in 0..4 {
        let mut state = test_state(City::load(&path).expect("load default city"));
        state.city.set_view_rotation(rotation);
        state.refresh_simulation_service_bits();
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                if structural_internal_pipe_eligible(&state.city, x, y)
                    && !is_water_source_building(state.city.building_at(x, y))
                {
                    assert!(
                        underground_render_tile(&state.city, x, y, state.city.rotation()).is_some(),
                        "missing rendered building pipe at ({x},{y}) in rotation {rotation}: xbld={} xzon={:?} xbit={:?} xund={}",
                        state.city.building_at(x, y),
                        state.city.zone_at(x, y),
                        state.city.tile_bits_at(x, y),
                        state.city.underground_at(x, y),
                    );
                }
            }
        }
    }
}

#[test]
fn selecting_underground_tools_switches_to_underground_view() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.show_traffic = true;

    state.apply_selected_tool(EditTool::Pipes);

    assert!(state.show_underground);
    assert!(!state.show_traffic);

    state.apply_selected_tool(EditTool::Road);

    assert!(!state.show_underground);
}

#[test]
fn underground_view_uses_light_gray_clear_color() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.clear_color(), CITY_CLEAR_COLOR);
    state.show_underground = true;
    assert_eq!(state.clear_color(), UNDERGROUND_CLEAR_COLOR);
}

#[test]
fn underground_view_keeps_surface_roads_power_and_highways_visible() {
    let road = network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT);
    let power = network_building_for_mask(NetworkKind::Power, NETWORK_LEFT | NETWORK_RIGHT);
    let highway = highway_building_for_axis(NetworkAxis::GridX);

    assert_eq!(
        underground_surface_sprite_info(road, 0).map(|info| info.layer_depth),
        Some(DEPTH_ROAD)
    );
    assert_eq!(
        underground_surface_sprite_info(power, 0).map(|info| info.layer_depth),
        Some(DEPTH_POWER)
    );
    assert_eq!(
        underground_surface_sprite_info(highway, 0).map(|info| info.layer_depth),
        Some(DEPTH_HIGHWAY)
    );
    assert!(underground_surface_sprite_info(44, 0).is_none());
    assert!(underground_surface_sprite_info(112, 0).is_none());
}

#[test]
fn underground_water_refresh_marks_connected_pipe_grid_watered() {
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
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.city.set_building(12, 10, 112);
    state.city.set_building(20, 20, 112);

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(12, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(12, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(20, 20).unwrap().piped());
    assert!(!state.city.tile_bits_at(20, 20).unwrap().watered());
}

#[test]
fn underground_water_refresh_connects_generated_building_pipe_tiles() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    for x in 11..=13 {
        state.city.set_building(x, 10, 112);
    }

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();

    assert_eq!(building_internal_pipe_tile(&state.city, 11, 10), Some(17));
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(12, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(13, 10).unwrap().watered());
}

#[test]
fn underground_water_refresh_connects_saved_building_pipe_tiles() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state
        .city
        .set_underground(11, 10, UNDER_TILE_BUILDING_PIPES);
    state.city.set_building(11, 10, 112);
    state.city.set_zone(12, 10, 1);
    // The PE worker counts developed XBLD cells, not undeveloped zone cells.
    state.city.set_building(12, 10, 112);

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();

    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(12, 10).unwrap().watered());
}

#[test]
fn simulation_day1_power_does_not_write_water_bits() {
    let mut state = water_consumer_fixture_for_calendar_test();
    state.game_clock = GameClock::new_with_day(1900, 1, 0);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 1);
    assert!(
        !state.city.tile_bits_at(11, 10).unwrap().piped(),
        "Win95 day 1 is power consumption, not water XBIT"
    );
    assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn simulation_day20_writes_water_bits() {
    let mut state = water_consumer_fixture_for_calendar_test();
    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.game_clock = GameClock::new_with_day(1900, 1, 19);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 20);
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn simulation_day20_persists_water_used_percent() {
    let mut state = water_consumer_fixture_for_calendar_test();
    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_live_misc_current_values();
    state.game_clock = GameClock::new_with_day(1900, 1, 19);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 20);
    assert_eq!(
        state.city.misc_u32_at(MISC_WATER_USED_PERCENT_OFFSET),
        Some(water_used_percent_for_city(&state.city))
    );
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn mac_engine_publishes_native_water_summary_for_win_layout_city() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-engine-win-layout-water-summary.sc2",
        "MacEngineWinLayoutWaterSummary",
    )
    .expect("blank city");
    city.kind = CityFileKind::WinSc2;
    let raw_water_slot = 0x8765_4321;
    city.set_misc_u32_at(1049 * 4, raw_water_slot);
    city.set_building(40, 40, 220);
    city.set_building(41, 40, 0x70);
    city.set_underground(40, 40, 16);
    city.set_underground(41, 40, 16);
    city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_powered(true));
    city.set_tile_bits(41, 40, TileBits::new(0).with_piped(true));
    city.set_mac_state_misc_u32_at(0x68, 4);
    city.set_mac_state_misc_u32_at(912 * 4, 0);
    city.set_microsim_tile_id(0, 0xf4);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.sim_calendar_day20_water();

    assert_eq!(state.city.misc_u32_at(1049 * 4), Some(raw_water_slot));

    assert_eq!(
        state.city.mac_water_service_summary(),
        50,
        "Mac water summary feeds later Mac XMIC selectors for either layout"
    );

    let mut expected_rng = MacToolboxRng::new(1);
    let expected_water_stat0 = 50 + (expected_rng.random_u16() & 7) as u8;
    state
        .city
        .refresh_mac_monthly_water_microsims(&mut MacToolboxRng::new(1));
    assert_eq!(
        state.city.microsim_at(0).unwrap().stat0,
        expected_water_stat0,
        "the following Mac XMIC pass reads the newly published water percentage"
    );
}

#[test]
fn mac_first_open_water_summary_seeds_annual_xmic_for_win_layout_city() {
    let mut city = City::new_blank(
        "/tmp/sc2000-mac-first-open-win-water-summary.sc2",
        "MacFirstOpenWinWaterSummary",
    )
    .expect("blank city");
    city.kind = CityFileKind::WinSc2;
    let raw_water_slot = 0x89ab_cdef;
    city.set_misc_u32_at(1049 * 4, raw_water_slot);
    city.set_microsim_tile_id(1, 0xf4);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.city.prepare_mac_fidelity_from_saved_city();
    assert_eq!(
        state.city.mac_water_service_summary(),
        raw_water_slot,
        "a first Mac-fidelity open loads the water summary from its raw MISC slot"
    );

    let mut expected_rng = MacToolboxRng::new(1);
    let expected_stat0 = (raw_water_slot as u8)
        .wrapping_add((expected_rng.random_u16() & 7) as u8);
    let mut xmic_rng = MacToolboxRng::new(1);
    state
        .city
        .refresh_mac_monthly_random_xmic_rows(0, 0, 0, 0, 0, true, &mut xmic_rng);

    assert_eq!(
        state.city.microsim_at(1).unwrap().stat0,
        expected_stat0,
        "the annual water-treatment row consumes the prior raw summary's low BYTE before phase 20"
    );
    assert_eq!(state.city.mac_water_service_summary(), raw_water_slot);
    assert_eq!(state.city.misc_u32_at(1049 * 4), Some(raw_water_slot));
}

#[test]
fn pe_fa_road_band_exclusive_lower_0x1c_vs_0x1d_like_pe_45a110() {
    // PE DestroyBuilding @ `0x45a110`: network requires XBLD `>= 0x1d` (jl to next band check).
    let mut city = City::new_blank("/tmp/sc2000-fa-road-lo.sc2", "FaRoadLo").expect("blank");
    city.set_building(20, 20, 0x1d);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x1c);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn mac68k_pe_fa_road_band_exclusive_lower_0x1c_vs_0x1d_like_pe() {
    let mut city = City::new_blank("/tmp/sc2000-fa-road-lo-mac.sc2", "FaRoadLoMac").expect("blank");
    city.set_building(20, 20, 0x1d);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x1c);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn pe_fa_elevated_band_exclusive_lower_0x3e_vs_0x3f_like_pe_45a120() {
    // PE DestroyBuilding @ `0x45a120`: network requires XBLD `>= 0x3f` before `< 0x47`.
    let mut city = City::new_blank("/tmp/sc2000-fa-elev-lo.sc2", "FaElevLo").expect("blank");
    city.set_building(20, 20, 0x3f);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x3e);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn mac68k_pe_fa_elevated_band_exclusive_lower_0x3e_vs_0x3f_like_pe() {
    let mut city = City::new_blank("/tmp/sc2000-fa-elev-lo-mac.sc2", "FaElevLoMac").expect("blank");
    city.set_building(20, 20, 0x3f);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x3e);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn pe_fa_power_band_exclusive_lower_0x5c_vs_0x5d_like_pe_45a140() {
    // PE DestroyBuilding @ `0x45a140`: network requires XBLD `>= 0x5d` before `< 0x61`.
    let mut city = City::new_blank("/tmp/sc2000-fa-pwr-lo.sc2", "FaPwrLo").expect("blank");
    city.set_building(20, 20, 0x5d);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x5c);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn mac68k_pe_fa_power_band_exclusive_lower_0x5c_vs_0x5d_like_pe() {
    let mut city = City::new_blank("/tmp/sc2000-fa-pwr-lo-mac.sc2", "FaPwrLoMac").expect("blank");
    city.set_building(20, 20, 0x5d);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x5c);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn stamp_military_base_plot_preserves_watered_and_xtrf() {
    // PE military zone stamp: XZON (+ clear small XBLD) — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-mil-xtrf.sc2", "MilX").expect("blank");
    city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(40, 40, 0x55);
    let mut state = test_state(city);
    state.stamp_military_base_plot(40, 40);
    assert_eq!(
        state.city.zone_at(40, 40).map(|z| z.zone_id()),
        Some(ZONE_MILITARY)
    );
    let bits = state.city.tile_bits_at(40, 40).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x55));
}

#[test]
fn try_place_missile_silos_preserves_watered_and_xtrf() {
    // PE missile-silo PlaceTile/PlaceXUND stamp — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-silo-xtrf.sc2", "SiloX").expect("blank");
    for y in 0..3 {
        for x in 0..3 {
            city.set_altitude(x, y, 8, 0);
            city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_watered(true));
            city.set_traffic_density(x, y, 0x61);
        }
    }
    let mut state = test_state(city);
    assert!(state.try_place_missile_silos(true));
    let bits = state.city.tile_bits_at(0, 0).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(0, 0), Some(0x61));
    assert_eq!(
        state.city.zone_at(0, 0).map(|z| z.zone_id()),
        Some(ZONE_MILITARY)
    );
    assert_eq!(state.city.building_at(0, 0), MILITARY_SILO_BUILDING_ID);
    assert_eq!(state.city.underground_at(0, 0), UNDER_TILE_GREY_BOX_1);
}

#[test]
fn pe_military_base_plot_type_matches_altitude_counts() {
    assert_eq!(
        pe_military_base_plot_type_from_counts(64, 64),
        Some(MILITARY_BASE_AIR_FORCE)
    );
    assert_eq!(
        pe_military_base_plot_type_from_counts(64, 40),
        Some(MILITARY_BASE_ARMY)
    );
    // PE does not route a 40+ eligible but insufficiently level plot to silos.
    assert_eq!(pe_military_base_plot_type_from_counts(64, 39), None);
    assert_eq!(pe_military_base_plot_type_from_counts(39, 39), None);
    assert_eq!(pe_military_base_plot_type_from_counts(40, 41), None);
}

#[test]
fn military_base_offer_accept_places_naval_yard_on_saltwater_coast() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    for y in 0..CITY_HEIGHT {
        for x in 0..4 {
            state.city.set_terrain(x, y, 0x10);
            state
                .city
                .set_tile_bits(x, y, crate::city::TileBits::new(0b0000_0001));
        }
    }
    // CRT seed 1 → first `random_mod(2) == 1` (naval coin wins).
    state.pe_process_crt = crate::win95_random::Win95CrtRand::new(1);
    state.open_military_base_dialog();

    assert!(state.accept_military_base_offer());
    assert_eq!(
        state.city.misc_u32_at(MISC_MILITARY_BASE_TYPE_OFFSET),
        Some(MILITARY_BASE_NAVY)
    );
    assert_eq!(state.query_line, "Naval Yard Established");
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
        military_tiles >= MILITARY_NAVAL_ZONE_LENGTH,
        "expected naval military strip, got {military_tiles} tiles"
    );
}

#[test]
fn january_fills_civic_and_water_microsim_stats() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1955,
        month: 6,
    });

    let hall = city.provision_microsim_for_building(208).expect("hall");
    let hall_index = usize::from(hall - MIN_SIM_TEXT_ENTRIES);
    let statue = city.provision_microsim_for_building(219).expect("statue");
    let statue_index = usize::from(statue - MIN_SIM_TEXT_ENTRIES);
    let mayor = city.provision_microsim_for_building(243).expect("mayor");
    let mayor_index = usize::from(mayor - MIN_SIM_TEXT_ENTRIES);
    let water = city.provision_microsim_for_building(244).expect("water");
    let water_index = usize::from(water - MIN_SIM_TEXT_ENTRIES);
    let desal = city.provision_microsim_for_building(250).expect("desal");
    let desal_index = usize::from(desal - MIN_SIM_TEXT_ENTRIES);

    assert_eq!(city.microsim_at(hall_index).map(|r| r.stat2), Some(1955));
    assert_eq!(
        city.microsim_at(statue_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((0, 1955, 0, 0)),
        "PE tile DB initializer writes only current year to stat1"
    );
    assert_eq!(
        city.microsim_at(mayor_index)
            .map(|r| (r.stat1, r.stat2, r.stat3)),
        Some((1955, 21, 47))
    );
    assert_eq!(city.microsim_at(water_index).map(|r| r.stat2), Some(0));

    let slots = [
        hall_index,
        statue_index,
        mayor_index,
        water_index,
        desal_index,
    ];
    assert_eq!(slots, [10, 11, 12, 13, 14]);
    // Original January call at 0x47352a, with native MSVC CRT seed 1.
    // City Hall includes arcology residents. The two water rows do not.
    for (population, arcology, water_capacity, desal_capacity) in
        [(55_860, 0, 139, 157), (1_000, 54_860, 20, 20)]
    {
        let mut city = city.clone();
        city.set_pe_mayor_approval(63);
        city.set_pe_dw_arcology_population(arcology);
        let mut crt = Win95CrtRand::new(1);
        city.refresh_structure_microsim_yearly_with_rng(
            population,
            100,
            100,
            100,
            100,
            100,
            &mut crt,
            &mut Win95Lfsr::new(Win95Lfsr::DEFAULT_STATE),
        );
        assert_eq!(
            slots.map(|index| {
                let row = city.microsim_at(index).unwrap();
                (row.stat0, row.stat1, row.stat2, row.stat3)
            }),
            [
                (0, 62, 1955, 0),
                (0, 1955, 41, 0),
                (1, 1955, 63, 46),
                (103, 34, water_capacity, 0),
                (101, 24, desal_capacity, 0),
            ],
        );
        assert_eq!(city.pe_mayor_approval(), 63);
        assert_eq!(crt.seed(), 752_224_798);
    }
}

#[test]
fn mac68k_terrain_classifier_clears_surface_by_run_state_word() {
    for mode in [0u32, 1, 2, 0x0001_0000] {
        for building in [0u8, 4, 5, 6, 12] {
            let mut city = City::new_blank("/tmp/sc2000-mac-classify-mode.sc2", "MacMode")
                .expect("blank city");
            city.kind = CityFileKind::MacCity;
            city.set_building(32, 32, building);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, mode);
            city.set_zone(32, 32, 0x87);
            city.set_underground(32, 32, 0x23);
            city.set_altitude(32, 32, 31, 3);
            city.set_text_overlay(32, 32, 49);
            city.set_traffic_density(32, 32, 63);
            let mut rng = MacToolboxRng::new(0x1397_8123);

            super::super::budget::mac_classify_terrain_tile(&mut city, 32, 32, &mut rng);

            let expected = if mode as u16 != 0 && building != 5 {
                0
            } else {
                building
            };
            assert_eq!(
                city.building_at(32, 32),
                expected,
                "mode={mode:#x}, tile={building}"
            );
            assert_eq!(city.underground_at(32, 32), 0);
            assert_eq!(city.zone_at(32, 32).unwrap().raw_bits(), 0x87);
            assert_eq!(city.altm_at(32, 32).land, 31);
            assert_eq!(city.altm_at(32, 32).water, 3);
            assert_eq!(city.text_overlay_at(32, 32), Some(49));
            assert_eq!(city.traffic_density_at(32, 32), Some(63));
            assert_eq!(rng.seed(), 0x1397_8123);
        }
    }
}

#[test]
fn mac_terrain_water_height_is_read_for_both_saved_city_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/sc2000-mac-shared-terrain-water.sc2", "MacWater")
            .expect("blank city");
        city.kind = kind;
        city.set_mac_state_misc_u32_at(0x0e40, 0x1234_0006);
        city.set_mac_state_misc_u32_at(53 * 4, 27); // unrelated historical guess
        for x in 31..=33 {
            for y in 31..=33 {
                city.set_altitude(x, y, 5, 2);
            }
        }
        city.set_building(32, 32, 6);
        city.set_traffic_density(32, 32, 63);
        let mut rng = MacToolboxRng::new(0x1397_8123);

        super::super::budget::mac_classify_terrain_tile(&mut city, 32, 32, &mut rng);

        assert_eq!(city.terrain_at(32, 32), 0x20, "{kind:?}");
        assert_eq!(city.altm_at(32, 32).land, 5);
        assert_eq!(city.altm_at(32, 32).water, 6);
        assert!(city.tile_bits_at(32, 32).unwrap().water_tile());
        assert_eq!(city.building_at(32, 32), 0);
        assert_eq!(city.traffic_density_at(32, 32), Some(63));
        assert_eq!(city.misc_u32_at(53 * 4), Some(27));
        assert_eq!(rng.seed(), 0x1397_8123);
    }
}

#[test]
fn mac_terrain_water_height_generated_in_win_layout_survives_save_reload() {
    let mut city = City::new_blank("/tmp/sc2000-mac-generated-win-water.sc2", "MacWinWater")
        .expect("blank Win-format city");
    city.set_misc_u32_at(0x0e40, 19);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 0);
    state.city.set_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, 0);
    state.edit_map_sliders = ui::EditMapSliderValues {
        hills: 0,
        water: 8,
        trees: 0,
    };

    state.generate_edit_map_with_seed(0x1234_5678);

    assert_eq!(state.city.kind, CityFileKind::WinSc2);
    assert_eq!(state.city.mac_selected_water_height(), Some(1));
    assert_eq!(state.city.misc_water_table_level(), Some(1));
    assert_eq!(state.city.misc_u32_at(0x0e40), Some(1));
    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-generated-win-water-{}.sc2",
        std::process::id()
    ));
    state.city.save(&path).expect("save Win-layout city");
    let loaded = City::load(&path).expect("reload Win-layout city");
    assert_eq!(loaded.kind, CityFileKind::WinSc2);
    assert_eq!(loaded.mac_selected_water_height(), Some(1));
    assert_eq!(loaded.misc_water_table_level(), Some(1));
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_water_source_uses_shared_word_height_and_byte_humidity_in_both_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/sc2000-mac-shared-water-inputs.sc2", "MacInputs")
            .expect("blank city");
        city.kind = kind;
        city.set_mac_state_misc_u32_at(0x0e40, 0x1234_0006);
        city.set_mac_state_misc_u32_at(0x0068, 0x5678_0040);
        city.set_building(32, 32, 0xdc);
        city.set_tile_bits(32, 32, TileBits::new(0x40));
        let mut connected = vec![false; CITY_TILES];
        connected[density_map_index(32, 32)] = true;
        let power = power_service_maps(&city);

        let capacity = connected_water_source_capacity_tiles_with_fidelity(
            &city,
            &connected,
            &power,
            EngineFidelity::Mac68k,
        );

        assert_eq!(capacity, 5 * 6 + 64 / 2, "{kind:?}");
    }
}

#[test]
fn mac68k_terrain_classifier_rendered_generic_cleanup_preserves_draw_order() {
    // Static export-80 transcription: 2*extent^3 stamp draws precede the
    // column-major footprint's XTER-dependent rubble draws. Only the root
    // gets the outer classifier's mode-sensitive final PlaceTile(0).
    for (root, extent) in [
        (0x0du8, 1usize),
        (0x70, 1),
        (0x8c, 2),
        (0xae, 3),
        (0xc9, 4),
        (0xd0, 3),
        (0xdb, 1),
        (0xf3, 2),
        (0xff, 4),
    ] {
        for mode in [0u32, 1] {
            let mut city = City::new_blank("/tmp/sc2000-mac-classify-generic.sc2", "MacGeneric")
                .expect("blank city");
            city.kind = CityFileKind::MacCity;
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, mode);
            city.set_mac_state_misc_u32_at(0x1024, 100);
            for column in 0..extent {
                for row in 0..extent {
                    let (x, y) = (32 + column, 32 - row);
                    city.set_building(x, y, root);
                    city.set_altitude(x, y, 31, 3);
                    city.set_terrain(x, y, u8::from(column == 1 && row == 0));
                    city.set_zone(x, y, 0x0a);
                    city.set_tile_bits(x, y, TileBits::new(0xc2));
                    city.set_text_overlay(x, y, 0xfa);
                    city.set_traffic_density(x, y, 63);
                    city.set_underground(x, y, 0x23);
                }
            }
            let seed = 0x3812_1097;
            let mut expected_rng = MacToolboxRng::new(seed);
            for _ in 0..(2 * extent * extent * extent) {
                expected_rng.random_u16();
            }
            let mut expected = Vec::new();
            for column in 0..extent {
                for row in 0..extent {
                    let rubble = if column == 1 && row == 0 {
                        0
                    } else {
                        (expected_rng.random_u16() & 3) as u8 + 1
                    };
                    expected.push((
                        column,
                        row,
                        if column == 0 && row == 0 && mode != 0 {
                            0
                        } else {
                            rubble
                        },
                    ));
                }
            }
            let mut rng = MacToolboxRng::new(seed);

            super::super::budget::mac_classify_terrain_tile(&mut city, 32, 32, &mut rng);

            for (column, row, rubble) in expected {
                let (x, y) = (32 + column, 32 - row);
                assert_eq!(
                    city.building_at(x, y),
                    rubble,
                    "root={root:#x}, mode={mode}, ({x},{y})"
                );
                assert_eq!(city.text_overlay_at(x, y), Some(0));
                assert_eq!(city.traffic_density_at(x, y), Some(63));
                assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0x0a);
                assert_eq!(
                    city.underground_at(x, y),
                    if column == 0 && row == 0 { 0 } else { 0x23 }
                );
            }
            assert_eq!(city.mac_xbld_tile_count(root), 0);
            assert_eq!(
                city.mac_state_misc_u32_at(0x1024),
                Some(100 - (extent * extent) as u32)
            );
            if let Some(bit) = match root {
                0xf3 => Some(0),
                0xd0 => Some(1),
                0xdb => Some(2),
                0xff => Some(3),
                _ => None,
            } {
                assert_eq!(
                    city.mac_state_misc_u32_at(0x78).unwrap_or(0) & (1 << bit),
                    1 << bit
                );
            }
            assert_eq!(
                rng.seed(),
                expected_rng.seed(),
                "root={root:#x}, mode={mode}"
            );
        }
    }
}

#[test]
fn mac68k_terrain_classifier_rendered_connected_cleanup_keeps_duplicate_draws() {
    for root in [0xddu8, 0xdf] {
        let mut city = City::new_blank("/tmp/sc2000-mac-classify-connected.sc2", "MacConnected")
            .expect("blank city");
        city.kind = CityFileKind::MacCity;
        for (x, y) in [(32, 32), (32, 33), (33, 33), (33, 32)] {
            city.set_building(x, y, root);
            city.set_altitude(x, y, 31, 3);
            city.set_zone(x, y, 0x8a);
            city.set_tile_bits(x, y, TileBits::new(0xc2));
            city.set_text_overlay(x, y, 49);
            city.set_traffic_density(x, y, 63);
        }
        let seed = 0x17fa_2058;
        let mut expected_rng = MacToolboxRng::new(seed);
        let mut rubble = [0u8; 5];
        // West/east/north/south pushes and newest-first pops leave (33,32)
        // pending twice. Each pop spends its own stamp pair, even the duplicate.
        for tile in &mut rubble {
            if root == 0xdd {
                *tile = (expected_rng.random_u16() & 3) as u8 + 1;
            }
            expected_rng.random_u16();
            expected_rng.random_u16();
        }
        let mut rng = MacToolboxRng::new(seed);

        super::super::budget::mac_classify_terrain_tile(&mut city, 32, 32, &mut rng);

        for (index, (x, y)) in [(32, 32), (32, 33), (33, 33), (33, 32)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                city.building_at(x, y),
                rubble[if index == 3 { 4 } else { index }]
            );
            assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0x0a);
            assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits(), 0);
            assert_eq!(city.text_overlay_at(x, y), Some(49));
            assert_eq!(city.traffic_density_at(x, y), Some(63));
        }
        assert_eq!(city.mac_xbld_tile_count(root), 0);
        assert_eq!(rng.seed(), expected_rng.seed(), "root={root:#x}");
    }
}

#[test]
fn mac68k_late_xund_coastal_destroy_clears_connected_family_without_render_draws() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-xund-coastal-component.sc2", "Coastal").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 10, 0);
    for (x, y, building) in [
        (20, 20, 0xdf),
        (21, 20, 0xe0),
        (20, 21, 0xdf),
        (21, 21, 0xe0),
    ] {
        city.set_building(x, y, building);
        city.set_zone(x, y, 0xda);
        city.set_tile_bits(x, y, TileBits::new(0xf3));
    }
    city.set_underground(20, 20, 0x23);
    city.set_text_overlay(20, 20, 0x55);
    city.set_building(19, 20, 0xdd); // neighboring family is not traversed

    let seed = 0x1357_9bdf;
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_u16(); // late XUND funding admission only
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

    for (x, y) in [(20, 20), (21, 20), (20, 21), (21, 21)] {
        assert_eq!(city.building_at(x, y), 0);
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0x0a);
        assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits(), 0x31);
    }
    assert_eq!(city.building_at(19, 20), 0xdd);
    assert_eq!(city.underground_at(20, 20), 0x23);
    assert_eq!(city.text_overlay_at(20, 20), Some(0x55));
    assert_eq!(city.mac_xbld_tile_count(0xdf), 0);
    assert_eq!(city.mac_xbld_tile_count(0xe0), 0);
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac68k_late_xund_generic_extent1_selector_keeps_zero_render_map_edits() {
    // Both the direct-below-0x70 selectors and table-backed extent-1 entries
    // keep their generic export-80 map edits when rendering is disabled.
    // XTER selects rubble versus empty PlaceTile; XTXT cleanup is independent.
    for building in [0x5d, 0x70, 0xc6, 0xe1] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-late-xund-generic.sc2", "Generic").expect("blank");
        city.kind = CityFileKind::MacCity;
        for index in 0..BUDGET_SERVICE_COUNT {
            set_budget_funding_for_city_test(&mut city, index, 100);
        }
        set_budget_funding_for_city_test(&mut city, 7, 100);
        set_budget_funding_for_city_test(&mut city, 10, 0);
        city.set_building(20, 20, building);
        city.set_underground(20, 20, 0x23);
        let has_zero_terrain = building == 0x5d;
        city.set_terrain(20, 20, u8::from(!has_zero_terrain));
        city.set_zone(20, 20, 0xfa);
        city.set_tile_bits(20, 20, TileBits::new(0xc2));
        city.set_text_overlay(20, 20, 77);
        assert!(city.set_microsim_tile_id(26, 0x56)); // XTXT 77 -> XMIC index 26
        assert!(city.set_microsim_stat0(26, 42));
        city.set_label(77, "temporary structure label");

        let seed = 0x5566_7788;
        let mut expected_rng = MacToolboxRng::new(seed);
        expected_rng.random_u16(); // late XUND funding admission
        let rubble = if has_zero_terrain {
            (expected_rng.random_u16() as u8 & 3) + 1
        } else {
            0
        };
        let mut rng = MacToolboxRng::new(seed);
        mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

        assert_eq!(
            city.building_at(20, 20),
            rubble,
            "XBLD {building:#04x}, XTER={}",
            u8::from(!has_zero_terrain)
        );
        assert_eq!(
            city.underground_at(20, 20),
            if building < 0x70 { 0 } else { 0x23 }
        );
        assert_eq!(city.zone_at(20, 20).unwrap().raw_bits() & 0x0f, 0x0a);
        assert_eq!(city.tile_bits_at(20, 20).unwrap().raw_bits() & 0xc2, 0);
        assert_eq!(city.text_overlay_at(20, 20), Some(0));
        assert_eq!(city.microsim_at(26).unwrap().tile_id, 0);
        assert_eq!(city.microsim_at(26).unwrap().stat0, 42);
        assert_eq!(city.label_at(77), Some(""));
        assert_eq!(city.mac_xbld_tile_count(building), 0);
        if has_zero_terrain {
            assert_eq!(city.mac_xbld_tile_count(rubble), 1);
        } else {
            for rubble_id in 1..=4 {
                assert_eq!(city.mac_xbld_tile_count(rubble_id), 0);
            }
        }
        assert_eq!(rng.seed(), expected_rng.seed());
    }
}

#[test]
fn mac68k_late_xund_extent2_connected_run_skips_render_draws() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-late-xund-run.sc2", "BridgeRun").expect("blank");
    city.kind = CityFileKind::MacCity;
    for index in 0..BUDGET_SERVICE_COUNT {
        set_budget_funding_for_city_test(&mut city, index, 100);
    }
    set_budget_funding_for_city_test(&mut city, 7, 100);
    set_budget_funding_for_city_test(&mut city, 10, 0);
    city.set_building(20, 20, 0x6a);
    city.set_zone(20, 20, 0xfa);
    city.set_building(18, 20, 0x6b);
    for x in [19, 21] {
        // Empty water cells produce topology class 0x0e and its X-axis walk.
        // A 0x49 secondary cell takes the separately unresolved 0x0d cycle.
        city.set_tile_bits(x, 20, TileBits::new(0x04));
    }
    city.set_underground(20, 20, 0x23);
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
        city.set_zone(x, y, 0xfa);
    }

    // The connected two-cell run consumes no sprite draws in this zero-render
    // call; only the late subway-funding admission word is observable.
    let seed = 0x3141_5926;
    let mut expected_rng = MacToolboxRng::new(seed);
    expected_rng.random_u16();
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
        assert_eq!(city.building_at(x, y), 0, "cell ({x},{y})");
    }
    assert_eq!(rng.seed(), expected_rng.seed());
}

#[test]
fn mac_player_generic_bulldoze_xtxt_fa_decrements_saved_counter() {
    for (
        root_building,
        initial_network,
        initial_non_network,
        expected_network,
        expected_non_network,
    ) in [
        (0x0d_u8, 2_u16, 2_u16, 2_u16, 1_u16),
        (0x1d_u8, 2_u16, 2_u16, 1_u16, 2_u16),
        // CODE 5's SUBQ.W has no zero guard; preserve native WORD wrap.
        (0x0d_u8, 0_u16, 0_u16, 0_u16, u16::MAX),
    ] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-player-fa-counter.sc2", "FaCounter").expect("blank");
        city.kind = CityFileKind::MacCity;
        city.set_building(20, 20, root_building);
        city.set_terrain(20, 20, 1);
        city.set_text_overlay(20, 20, 0xfa);
        city.set_mac_state_misc_u32_at(0x1050, u32::from(initial_network));
        city.set_mac_state_misc_u32_at(0x1024, u32::from(initial_non_network));
        city.set_city_funds(100_000);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;

        state.bulldoze_at(20, 20);

        assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
        assert_eq!(
            state.city.mac_state_misc_u32_at(0x1050),
            Some(u32::from(expected_network))
        );
        assert_eq!(
            state.city.mac_state_misc_u32_at(0x1024),
            Some(u32::from(expected_non_network))
        );
    }
}

#[test]
fn mac_player_bulldozer_refuses_military_zone_before_cost_or_random_draw() {
    // CODE 5 `FUN_0000884e` tests XZON low nibble 7 before either mode branch,
    // cost debit, or any map mutation.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-military-bulldoze.city", "MacMilitary").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(100);
    city.set_building(20, 20, 0x0d);
    city.set_zone(20, 20, 0x27);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_mac_state_misc_u32_at(0x1024, 3);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;
    state.mac_process_rng = MacToolboxRng::new(0x1234);
    let rng_before = state.mac_process_rng.seed();

    assert!(!state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0x0d);
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 0x27);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0xfa));
    assert_eq!(state.city.mac_state_misc_u32_at(0x1024), Some(3));
    assert_eq!(state.city.city_funds(), 100);
    assert_eq!(state.mac_process_rng.seed(), rng_before);
}

#[test]
fn mac_player_bulldozer_obeys_surface_mode_cell_gates() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-surface-bulldoze-gates.city", "MacGates").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(100);
    city.set_building(20, 20, 5);
    city.set_zone(30, 30, 0x02);
    city.set_terrain(30, 30, 0x2f);
    city.set_building(40, 40, 5);
    city.set_underground(40, 40, 0x1f);
    city.set_tile_bits(50, 50, TileBits::new(0x20));
    city.set_tunnel_levels(60, 60, 1);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;
    state.mac_process_rng = MacToolboxRng::new(0x4321);
    let rng_before = state.mac_process_rng.seed();

    // Ordinary surface mode rejects the reserved XBLD and zoned, empty,
    // low-class XTER cells before charging or mutating either plane.
    assert!(!state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 5);
    assert!(!state.apply_tool_at_tile(30, 30));
    assert_eq!(state.city.building_at(30, 30), 0);
    assert_eq!(state.city.zone_at(30, 30).unwrap().raw_bits(), 0x02);
    assert_eq!(state.city.city_funds(), 100);
    assert_eq!(state.mac_process_rng.seed(), rng_before);

    // The native underground-mode branch has distinct gates; keep this
    // surface-only refusal out of that XUND operation.
    state.show_underground = true;
    assert!(state.apply_tool_at_tile(40, 40));
    assert_eq!(state.city.building_at(40, 40), 5);
    assert_eq!(state.city.underground_at(40, 40), 0);
    assert_eq!(state.city.city_funds(), 99);

    // The Mac nonzero-mode preflight also admits an empty XUND cell when
    // XBIT's piped bit is set; the shared clear worker removes that flag.
    assert!(state.apply_tool_at_tile(50, 50));
    assert!(!state.city.tile_bits_at(50, 50).unwrap().piped());
    assert_eq!(state.city.city_funds(), 98);

    // ALTM tunnel level 1 is the other native empty-XUND eligibility arm.
    assert!(state.apply_tool_at_tile(60, 60));
    assert_eq!(state.city.city_funds(), 97);

    // An empty underground cell with neither native admission marker is still
    // refused before charging.
    assert!(!state.apply_tool_at_tile(70, 70));
    assert_eq!(state.city.city_funds(), 97);
}

#[test]
fn mac_player_bulldozer_lowclass_surface_uses_native_placetile_contract() {
    // On the rooted ordinary surface route, XBLD < 0x0d reaches Mac
    // PlaceTile(0), then skips the water-normalization branch when XTER < 0x30.
    // This is not PE DestroyBuilding: XZON, XBIT, XUND, XTXT, ALTM, and XTRF
    // are left as-is before the common surface-network tail.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-lowclass-bulldoze.city", "MacLowclass").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(100);
    city.set_building(20, 20, 1);
    city.set_terrain(20, 20, 0x2f);
    city.set_altitude(20, 20, 7, 12);
    city.set_tile_bits(20, 20, TileBits::new(0x65));
    city.set_zone(20, 20, 0x25);
    city.set_underground(20, 20, 0x10);
    city.set_text_overlay(20, 20, 0x20);
    city.set_traffic_density(20, 20, 37);
    let count_zero_before = city.mac_xbld_tile_count(0);
    let count_one_before = city.mac_xbld_tile_count(1);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;
    state.mac_process_rng = MacToolboxRng::new(0x4321);
    let rng_before = state.mac_process_rng.seed();

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(
        state.city.mac_xbld_tile_count(0),
        count_zero_before.wrapping_add(1)
    );
    assert_eq!(state.city.mac_xbld_tile_count(1), count_one_before - 1);
    assert_eq!(state.city.terrain_at(20, 20), 0x2f);
    let altitude = state.city.altm_at(20, 20);
    assert_eq!((altitude.land, altitude.water), (7, 12));
    assert_eq!(state.city.tile_bits_at(20, 20).unwrap().raw_bits(), 0x65);
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 0x25);
    assert_eq!(state.city.underground_at(20, 20), 0x10);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x20));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(37));
    assert_eq!(state.city.city_funds(), 99);
    assert_eq!(state.mac_process_rng.seed(), rng_before);
}

#[test]
fn mac_player_bulldozer_lowclass_water_clears_and_repairs_wet_neighbors() {
    // CODE 5 FUN_0000884e's ordinary XTER>=0x30, XTER!=0x3e arm clears the
    // clicked XBIT water flag and XTER, then calls CODE 3 export 136 for each
    // still-wet cardinal/diagonal neighbor. The water special 0x3e classifier
    // is intentionally excluded from this bounded case.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-water-bulldoze.city", "MacWaterBulldoze").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(100);
    for ty in 18..=22 {
        for tx in 18..=22 {
            city.set_tile_bits(tx, ty, TileBits::new(0));
        }
    }
    city.set_building(20, 20, 1);
    city.set_terrain(20, 20, 0x31);
    city.set_tile_bits(20, 20, TileBits::new(0x24));

    // One surviving wet neighbor has no other wet cardinal neighbors after
    // the selected tile is cleared; export 136's zero-mask table entry is 13.
    city.set_terrain(21, 20, 0x42);
    city.set_tile_bits(21, 20, TileBits::new(0x24));
    // The traffic plane is stored as a 2x2 map, so adjacent tiles share XTRF.
    city.set_traffic_density(20, 20, 19);
    city.set_traffic_density(22, 20, 23);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.terrain_at(20, 20), 0);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(state.city.terrain_at(21, 20), 0x3d);
    assert!(state.city.tile_bits_at(21, 20).unwrap().water_tile());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(19));
    assert_eq!(state.city.traffic_density_at(21, 20), Some(19));
    assert_eq!(state.city.traffic_density_at(22, 20), Some(23));
    assert_eq!(state.city.city_funds(), 99);
}

#[test]
fn mac_player_bulldozer_lowclass_xter_3e_runs_native_classifier() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-xter-3e-bulldoze.city", "MacXter3e").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(100);
    city.set_mac_state_misc_u32_at(0x0e40, 6);
    for ty in 18..=22 {
        for tx in 18..=22 {
            city.set_tile_bits(tx, ty, TileBits::new(0));
            city.set_altitude(tx, ty, 10, 13);
        }
    }
    city.set_building(20, 20, 1);
    city.set_terrain(20, 20, 0x3e);
    city.set_tile_bits(20, 20, TileBits::new(0x24));
    city.set_altitude(20, 20, 5, 13);
    city.set_zone(20, 20, 0xa1);
    city.set_underground(20, 20, 0x10);
    city.set_text_overlay(20, 20, 49);
    city.set_traffic_density(20, 20, 63);
    let old_empty_count = city.mac_xbld_tile_count(0);
    let old_building_count = city.mac_xbld_tile_count(1);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Bulldoze;
    state.mac_process_rng = MacToolboxRng::new(0x1397_8123);
    let rng_before = state.mac_process_rng.seed();

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.mac_xbld_tile_count(1), old_building_count - 1);
    assert_eq!(
        state.city.mac_xbld_tile_count(0),
        old_empty_count.wrapping_add(1)
    );
    assert_eq!(state.city.terrain_at(20, 20), 0);
    assert_eq!(state.city.altm_at(20, 20).land, 6);
    assert_eq!(state.city.altm_at(20, 20).water, 13);
    assert_eq!(state.city.tile_bits_at(20, 20).unwrap().raw_bits(), 0x20);
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 0xa0);
    assert_eq!(state.city.underground_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(49));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(63));
    assert_eq!(state.city.city_funds(), 99);
    assert_eq!(state.mac_process_rng.seed(), rng_before);
}

#[test]
fn mac68k_terrain_classifier_upkeep_clears_endpoint_xund_and_saved_counter() {
    // Both CODE 8 callers reach export 138 after lowering dry endpoints.
    // Its export-106 clear applies to every nonzero XUND byte, while only
    // selected byte classes decrement the saved WORD at MISC 0xfe0.
    for late_xund in [false, true] {
        for (underground, counted) in [
            (1, true),
            (0x0f, true),
            (0x10, false),
            (0x11, false),
            (0x1f, true),
            (0x20, true),
            (0x22, true),
            (0x23, true),
            (0xff, false),
        ] {
            let mut city =
                City::new_blank("/tmp/mac-classifier-xund.sc2", "MacClassifyXund").unwrap();
            city.kind = CityFileKind::MacCity;
            for index in 0..BUDGET_SERVICE_COUNT {
                set_budget_funding_for_city_test(&mut city, index, 100);
            }
            set_budget_funding_for_city_test(&mut city, if late_xund { 10 } else { 7 }, 0);
            city.set_mac_state_misc_u32_at(0x64, 0);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            for x in 18..=20 {
                city.set_building(x, 20, 0x51);
                city.set_zone(x, 20, if late_xund { 0xfa } else { 0xf0 });
            }
            city.set_tile_bits(20, 20, TileBits::new(0).with_rotate(true));
            if late_xund {
                city.set_underground(20, 20, 0x23);
            }
            for x in [17, 21] {
                city.set_zone(x, 20, 0x87); // export 106 has no military-zone gate
                city.set_altm_word(x, 20, (0x21 << 10) | (3 << 5) | 5);
                city.set_tile_bits(x, 20, TileBits::new(0xa0));
                city.set_underground(x, 20, underground);
                city.set_text_overlay(x, 20, 0x49);
                city.set_traffic_density(x, 20, 0x63);
            }
            city.set_underground(18, 19, 0x20); // unclassified neighboring tile
            // A deliberately independent saved census: do not rescan it.
            city.set_mac_state_misc_u32_at(0x0fe8, 0);
            let seed = 0x5a17_2026;
            let mut rng = MacToolboxRng::new(seed);
            let mut expected_rng = rng;
            for _ in 0..if late_xund { 1 } else { 7 } {
                expected_rng.random_u16();
            }

            mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);

            for x in [17, 21] {
                assert_eq!(
                    city.underground_at(x, 20),
                    0,
                    "late={late_xund}, XUND={underground:#x}"
                );
                assert_eq!(city.altm_at(x, 20).tunnel_levels, 0x21);
                assert_eq!(city.water_height_at(x, 20), 3);
                assert_eq!(city.tile_bits_at(x, 20), Some(TileBits::new(0xa0)));
                assert_eq!(city.text_overlay_at(x, 20), Some(0x49));
                assert_eq!(city.traffic_density_at(x, 20), Some(0x63));
            }
            assert_eq!(city.underground_at(18, 19), 0x20);
            if late_xund {
                assert_eq!(city.underground_at(20, 20), 0x23);
            }
            let expected_count = if counted {
                u32::from(0u16.wrapping_sub(2))
            } else {
                0
            };
            assert_eq!(city.mac_state_misc_u32_at(0x0fe8), Some(expected_count));
            assert_eq!(rng.seed(), expected_rng.seed());
        }
    }
}

#[test]
fn rubble_highway_block_xbit_water_place_tile_zero() {
    // PE @ `0x435b52`: XBIT `0x04` (water_tile) → PlaceTile(0) on that cell.
    let mut city = City::new_blank("/tmp/sc2000-hwy-xbit-water.sc2", "HwyXbitW").expect("blank");
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 74);
        }
    }
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    let mut state = test_state(city);
    let mut crt = Win95CrtRand::new(1);
    assert_eq!(state.rubble_highway_block(20, 20, &mut crt), 4);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert!(is_rubble_tile(state.city.building_at(21, 20)));
    assert!(is_rubble_tile(state.city.building_at(20, 21)));
    assert!(is_rubble_tile(state.city.building_at(21, 21)));
}

#[test]
fn rubble_highway_block_water_terrain_without_xbit_still_rubble() {
    // PE tests XBIT only — invent water-terrain collapse must not PlaceTile(0).
    let mut city = City::new_blank("/tmp/sc2000-hwy-xter-water.sc2", "HwyXterW").expect("blank");
    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 74);
        }
    }
    city.set_terrain(20, 20, 0x10); // water XTER without XBIT water bit
    assert!(is_water_terrain(city.terrain_at(20, 20)));
    assert!(
        !city
            .tile_bits_at(20, 20)
            .is_some_and(|bits| bits.water_tile())
    );
    let mut state = test_state(city);
    let mut preview = Win95CrtRand::new(0x435b_5201);
    let expected = preview.rubble_tile_id();
    let mut crt = Win95CrtRand::new(0x435b_5201);
    assert_eq!(state.rubble_highway_block(20, 20, &mut crt), 4);
    assert_eq!(state.city.building_at(20, 20), expected);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
}

#[test]
fn underground_water_refresh_limits_consumers_by_available_supply() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    // Blank terrain has water target 7 (35 units); this fixture exercises the
    // exact 40-unit cutoff from the PE pump formula.
    state.city.set_misc_u32_at(0x0e40, 8);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    for x in 11..=51 {
        state.city.set_zone(x, 10, 1);
        state.city.set_building(x, 10, 112);
    }

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
    // The adjacent XBLD-200 power plant is itself in the saved XBIT pipe flood
    // and counts as one generic PE worker cell, leaving 39 units for consumers.
    assert!(state.city.tile_bits_at(49, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(50, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(50, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(51, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(51, 10).unwrap().watered());
}

#[test]
fn underground_water_refresh_does_not_share_supply_between_disconnected_networks() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.set_misc_u32_at(0x0e40, 8);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );

    state.city.set_building(50, 50, 220);
    power_water_source_for_test(&mut state.city, 50, 50);
    state.city.set_underground(
        50,
        50,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    for x in 51..=91 {
        state.city.set_zone(x, 50, 1);
        state.city.set_building(x, 50, 112);
    }

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(50, 50).unwrap().watered());
    assert!(state.city.tile_bits_at(89, 50).unwrap().watered());
    assert!(!state.city.tile_bits_at(90, 50).unwrap().watered());
    assert!(state.city.tile_bits_at(91, 50).unwrap().piped());
    assert!(!state.city.tile_bits_at(91, 50).unwrap().watered());
}

#[test]
fn water_status_uses_component_local_supply() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(0x0e40, 8);

    city.set_building(10, 10, 220);
    power_water_source_for_test(&mut city, 10, 10);
    city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );

    city.set_building(50, 50, 220);
    power_water_source_for_test(&mut city, 50, 50);
    city.set_underground(
        50,
        50,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    for x in 51..=91 {
        city.set_zone(x, 50, 1);
        city.set_building(x, 50, 112);
    }

    seed_saved_water_worker_bits_for_test(&mut city);
    assert_eq!(tile_water_status_text(&city, 89, 50), " water=on");
    assert_eq!(tile_water_status_text(&city, 90, 50), " water=off");
    assert_eq!(tile_water_status_text(&city, 91, 50), " water=off");
    assert_eq!(
        water_service_summary(&city),
        WaterServiceSummary {
            served: 39,
            demand: 41
        }
    );
    assert_eq!(ServiceStatus::for_city(&city).water_shortage(), 2);
}

#[test]
fn connected_water_tower_extends_available_supply_but_disconnected_tower_does_not() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    for x in 11..=51 {
        state.city.set_zone(x, 10, 1);
        state.city.set_building(x, 10, 112);
    }
    for y in 10..=11 {
        for x in 52..=53 {
            state.city.set_building(x, y, 235);
            state.city.set_tile_bits(
                x,
                y,
                state
                    .city
                    .tile_bits_at(x, y)
                    .unwrap_or_default()
                    .with_watered(true),
            );
        }
    }
    for y in 80..=81 {
        for x in 80..=81 {
            state.city.set_building(x, y, 235);
        }
    }

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();
    let (_, connected_tiles) = water_pipe_connection_maps(&state.city);

    assert_eq!(
        connected_water_storage_capacity_tiles(&state.city, &connected_tiles),
        400
    );
    assert!(state.city.tile_bits_at(51, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(52, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(80, 80).unwrap().piped());
    assert!(!state.city.tile_bits_at(80, 80).unwrap().watered());
}

#[test]
fn connected_water_storage_capacity_counts_tower_once_for_multi_tile_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 235);
        }
    }

    let mut connected_tiles = vec![false; CITY_TILES];
    for y in 20..=21 {
        for x in 20..=21 {
            connected_tiles[y * CITY_WIDTH + x] = true;
        }
    }

    assert_eq!(
        connected_water_storage_capacity_tiles(&city, &connected_tiles),
        400
    );
}

#[test]
fn connected_filled_water_storage_capacity_counts_tower_once_for_multi_tile_footprint() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 235);
            city.set_tile_bits(x, y, crate::city::TileBits::new(0b0001_0000));
        }
    }

    let mut connected_tiles = vec![false; CITY_TILES];
    for y in 20..=21 {
        for x in 20..=21 {
            connected_tiles[y * CITY_WIDTH + x] = true;
        }
    }

    assert_eq!(
        connected_filled_water_storage_capacity_tiles(&city, &connected_tiles),
        400
    );
}

#[test]
fn filled_water_tower_keeps_serving_after_source_loses_power() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    for x in 11..=45 {
        state.city.set_zone(x, 10, 1);
        state.city.set_building(x, 10, 112);
    }
    for y in 10..=11 {
        for x in 46..=47 {
            state.city.set_building(x, y, 235);
            state.city.set_tile_bits(
                x,
                y,
                state
                    .city
                    .tile_bits_at(x, y)
                    .unwrap_or_default()
                    .with_watered(true),
            );
        }
    }

    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();
    assert!(state.city.tile_bits_at(46, 10).unwrap().watered());

    state.city.set_building(9, 10, 0);
    if let Some(bits) = state.city.tile_bits_at(10, 10) {
        state.city.set_tile_bits(10, 10, bits.with_powered(false));
    }
    state.refresh_water_pipe_bits();

    assert!(!power_service_maps(&state.city).powered_tiles[10 + 10 * CITY_WIDTH]);
    assert!(!state.city.tile_bits_at(45, 10).unwrap().watered());
    for y in 10..=11 {
        for x in 46..=47 {
            assert!(
                state.city.tile_bits_at(x, y).unwrap().watered(),
                "filled water tower tile ({x},{y}) should preserve stored water"
            );
        }
    }
}

#[test]
fn filled_water_tower_storage_is_capacity_limited() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    for y in 10..=29 {
        for x in 10..=31 {
            state.city.set_zone(x, y, 1);
            state.city.set_building(x, y, 112);
        }
    }
    for y in 10..=11 {
        for x in 10..=11 {
            state.city.set_zone(x, y, 0);
            state.city.set_building(x, y, 235);
            state.city.set_tile_bits(
                x,
                y,
                state
                    .city
                    .tile_bits_at(x, y)
                    .unwrap_or_default()
                    .with_watered(true),
            );
        }
    }
    state.city.set_building(9, 10, 220);
    state.city.set_misc_u32_at(0x0e40, 0);
    seed_saved_water_worker_bits_for_test(&mut state.city);

    // Snapshot before the worker consumes the four saved 100-unit tower markers.
    // Its first pass uses all 400 units on 400 of the 436 developed cells.
    assert_eq!(
        water_service_summary(&state.city),
        WaterServiceSummary {
            served: 400,
            demand: 436
        }
    );
    state.refresh_water_pipe_bits();
    let watered = (10..=29)
        .flat_map(|y| (10..=31).map(move |x| (x, y)))
        .filter(|&(x, y)| state.city.tile_bits_at(x, y).is_some_and(TileBits::watered))
        .count();
    assert_eq!(watered, 400);
}

#[test]
fn water_pump_auto_pipe_still_feeds_adjacent_building_plumbing() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(10, 10, 220), 1);
    power_water_source_for_test(&mut state.city, 10, 10);
    assert_eq!(state.city.underground_at(10, 10), UNDER_TILE_PIPES_LR);
    state.city.set_zone(11, 10, 1);
    state.city.set_building(11, 10, 112);
    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn unpowered_water_pump_does_not_seed_water() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(10, 10, 220), 1);
    state.city.set_zone(11, 10, 1);
    state.refresh_water_pipe_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());
}

#[test]
fn unpowered_desalination_plant_does_not_seed_water() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_terrain(9, 10, 0x30);
    state
        .city
        .set_tile_bits(9, 10, crate::city::TileBits::new(0b0000_0101));
    assert_eq!(state.place_building_at(10, 10, 250), 9);
    state.city.set_zone(13, 10, 1);

    state.refresh_water_pipe_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(13, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(13, 10).unwrap().watered());
}

#[test]
fn water_tower_and_treatment_do_not_produce_water_by_themselves() {
    for building in [235, 244] {
        let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        let mut state = test_state(city);

        state.city.set_building(10, 10, building);
        state.city.set_zone(11, 10, 1);
        state.refresh_water_pipe_bits();

        assert!(state.city.tile_bits_at(10, 10).unwrap().piped());
        assert!(!state.city.tile_bits_at(10, 10).unwrap().watered());
        assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
        assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());
    }
}

#[test]
fn pe_water_worker_without_source_preserves_only_saved_tower_markers() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    for y in 10..=11 {
        for x in 10..=11 {
            state.city.set_building(x, y, 235);
        }
    }
    state
        .city
        .set_tile_bits(10, 10, crate::city::TileBits::new(0b0001_0000));
    state.city.set_zone(12, 10, 1);

    state.refresh_water_pipe_bits();

    assert!(state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(!state.city.tile_bits_at(11, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(12, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(12, 10).unwrap().watered());
}

#[test]
fn desalination_requires_adjacent_saltwater_and_produces_water() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    let dry = state.building_placement_at(10, 10, 250);
    assert!(dry.valid_tiles.is_empty());
    assert_eq!(dry.invalid_tiles.len(), 9);
    assert_eq!(state.place_building_at(10, 10, 250), 0);

    state.city.set_terrain(9, 10, 0x30);
    state
        .city
        .set_tile_bits(9, 10, crate::city::TileBits::new(0b0000_0101));

    let shore = state.building_placement_at(10, 10, 250);
    assert_eq!(shore.valid_tiles.len(), 9);
    assert!(shore.invalid_tiles.is_empty());
    assert_eq!(state.place_building_at(10, 10, 250), 9);
    power_adjacent_building_for_test(&mut state.city, 10, 13);

    state.city.set_zone(13, 10, 1);
    state.city.set_building(13, 10, 112);
    seed_saved_water_worker_bits_for_test(&mut state.city);
    state.refresh_water_pipe_bits();
    assert!(state.city.tile_bits_at(10, 10).unwrap().watered());
    assert!(state.city.tile_bits_at(13, 10).unwrap().watered());
}

#[test]
fn pe_water_capacity_reads_signed_word_and_live_byte_in_both_layouts() {
    for extension in ["sc2", "city"] {
        for (raw_height, expected_height, expected_capacity) in [
            (0x1234_0004, 4, 67),
            (0x0000_0004, 4, 67),
            (0xabcd_ffff, u32::MAX, 42),
            (0x0000_ffff, u32::MAX, 42),
            (0xffff_8000, 0xffff_8000, 4_294_803_503),
        ] {
            let mut city = blank_city_layout_fixture(
                format!("/tmp/pe-water-input-width.{extension}"),
                "PeWaterInputs",
            );
            city.set_mac_state_misc_u32_at(0x0e40, raw_height);
            city.set_mac_state_misc_u32_at(0x0060, 0x1122_0016);
            city.set_mac_state_misc_u32_at(0x0064, 0x2233_0021);
            city.set_mac_state_misc_u32_at(0x006c, 0x3344_002c);
            city.set_mac_state_misc_u32_at(0x0068, 0x5678_000e);
            city.set_building(20, 20, 0xdc);
            city.set_tile_bits(20, 20, TileBits::new(0x60));
            for (x, y) in [(19, 19), (20, 19), (21, 19), (19, 20)] {
                city.set_tile_bits(x, y, TileBits::new(4));
            }
            let mut connected = vec![false; CITY_TILES];
            connected[density_map_index(20, 20)] = true;
            assert_eq!(city.misc_water_table_level(), Some(expected_height));
            assert_eq!(
                (
                    city.pe_b_weather_heat,
                    city.pe_b_weather_wind,
                    city.pe_b_weather_humid,
                    city.pe_b_weather_trend
                ),
                (22, 33, 14, 44)
            );
            assert_eq!(
                pe_connected_water_source_capacity_tiles(&city, &connected),
                expected_capacity,
                "layout={extension} height={raw_height:#x}"
            );

            let path = std::env::temp_dir().join(format!(
                "pe-water-input-width-{}-{raw_height:x}.{extension}",
                std::process::id(),
            ));
            assert_eq!(city.mac_state_misc_u32_at(0x0e40), Some(raw_height));
            assert_eq!(city.mac_state_misc_u32_at(0x0068), Some(0x5678_000e));
            city.save(&path).unwrap();
            let mut loaded = City::load(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            assert_eq!(loaded.misc_water_table_level(), Some(expected_height));
            assert_eq!(
                (
                    loaded.pe_b_weather_heat,
                    loaded.pe_b_weather_wind,
                    loaded.pe_b_weather_humid,
                    loaded.pe_b_weather_trend
                ),
                (22, 33, 14, 44)
            );
            assert_eq!(
                pe_connected_water_source_capacity_tiles(&loaded, &connected),
                expected_capacity
            );
            // The worker consumes the runtime byte, not a fresh read of MISC.
            loaded.pe_b_weather_humid = 16;
            assert_eq!(
                pe_connected_water_source_capacity_tiles(&loaded, &connected),
                expected_capacity.wrapping_add(1)
            );
            assert_eq!(loaded.mac_weather_humidity(), Some(14));
        }
    }
}

#[test]
fn pe_water_capacity_wraps_pump_and_carried_tower_units_before_allocation() {
    for extension in ["sc2", "city"] {
        let mut city = blank_city_layout_fixture(
            format!("/tmp/pe-water-capacity-wrap.{extension}"),
            "PeWaterWrap",
        );
        city.set_mac_state_misc_u32_at(0x0e40, 0x0000_ffff); // selected WORD -1
        city.set_mac_state_misc_u32_at(0x0068, 0);
        for y in 20..30 {
            for x in 20..32 {
                city.set_building(x, y, 0x70);
                city.set_tile_bits(x, y, TileBits::new(0x20));
            }
        }
        for (x, y) in [(20, 20), (22, 20)] {
            city.set_building(x, y, 0xdc);
            city.set_tile_bits(x, y, TileBits::new(0x60));
        }
        city.set_building(21, 20, 0xeb);
        city.set_tile_bits(21, 20, TileBits::new(0x30)); // 100 carried units
        let maps = water_service_maps_with_entry_seed_and_fidelity(&city, 1, EngineFidelity::Win95);
        // Each pump adds DWORD -5; the carried tower adds 100. The original
        // ADD accumulator wraps to 90 before min(capacity, generic demand).
        assert!(maps.pe_worker_contract_complete);
        assert_eq!(maps.pe_worker_capacity_units, 90, "{extension}");
        let served = (20..30)
            .flat_map(|y| (20..32).map(move |x| (x, y)))
            .filter(|&(x, y)| city.building_at(x, y) == 0x70)
            .filter(|&(x, y)| maps.watered_tiles[y * CITY_WIDTH + x])
            .count();
        assert_eq!(served, 90);
        assert!(!maps.watered_tiles[20 * CITY_WIDTH + 21]);
        assert_eq!(city.mac_state_misc_u32_at(0x0e40), Some(0xffff));
    }
}

#[test]
fn water_pump_output_counts_freshwater_not_saltwater() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for (x, y) in [(9, 9), (10, 9), (11, 9), (9, 10)] {
        city.set_terrain(x, y, 0x30);
        city.set_tile_bits(x, y, crate::city::TileBits::new(0b0000_0100));
    }
    city.set_terrain(11, 10, 0x30);
    city.set_tile_bits(11, 10, crate::city::TileBits::new(0b0000_0101));

    assert_eq!(water_pump_output_tiles(&city, 10, 10, 4, 14), 67);
}

#[test]
fn desalination_output_counts_saltwater_around_each_plant_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 9..=13 {
        for x in 9..=13 {
            if !(10..=12).contains(&x) || !(10..=12).contains(&y) {
                city.set_terrain(x, y, 0x30);
                city.set_tile_bits(x, y, crate::city::TileBits::new(0b0000_0101));
            }
        }
    }

    assert_eq!(desalination_output_tiles(&city, 10, 10), 640);
    assert_eq!(desalination_output_tiles(&city, CITY_WIDTH - 1, 10), 0);
}

#[test]
fn water_supply_capacity_counts_pumps_and_each_desalination_plant_once() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(20, 20, 220);
    power_water_source_for_test(&mut city, 20, 20);
    for (x, y) in [(19, 19), (20, 19), (21, 19), (19, 20)] {
        city.set_terrain(x, y, 0x30);
        city.set_tile_bits(x, y, crate::city::TileBits::new(0b0000_0100));
    }
    city.set_tile_bits(19, 20, crate::city::TileBits::new(0b1100_0100));
    for y in 9..=13 {
        for x in 9..=13 {
            if (10..=12).contains(&x) && (10..=12).contains(&y) {
                city.set_building(x, y, 250);
            } else {
                city.set_terrain(x, y, 0x30);
                city.set_tile_bits(x, y, crate::city::TileBits::new(0b0000_0101));
            }
        }
    }
    power_adjacent_building_for_test(&mut city, 10, 13);
    city.set_tile_bits(10, 13, crate::city::TileBits::new(0b1100_0101));

    assert_eq!(water_supply_capacity_tiles(&city, 4, 14), 707);
}

#[test]
fn water_supply_capacity_ignores_unpowered_sources() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(20, 20, 220);
    for (x, y) in [(19, 19), (20, 19), (21, 19), (19, 20)] {
        city.set_terrain(x, y, 0x30);
    }
    for y in 9..=13 {
        for x in 9..=13 {
            if (10..=12).contains(&x) && (10..=12).contains(&y) {
                city.set_building(x, y, 250);
            } else {
                city.set_terrain(x, y, 0x30);
                city.set_tile_bits(x, y, crate::city::TileBits::new(0b0000_0101));
            }
        }
    }

    assert_eq!(water_supply_capacity_tiles(&city, 4, 14), 0);
}

#[test]
fn connected_water_source_capacity_counts_desalination_source_once_from_complete_multi_tile_footprint()
 {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 9..=13 {
        for x in 9..=13 {
            if !(10..=12).contains(&x) || !(10..=12).contains(&y) {
                city.set_terrain(x, y, 0x30);
                city.set_tile_bits(x, y, crate::city::TileBits::new(0b0000_0101));
            }
        }
    }
    for y in 10..=12 {
        for x in 10..=12 {
            city.set_building(x, y, 250);
        }
    }
    power_adjacent_building_for_test(&mut city, 11, 13);
    city.set_tile_bits(11, 13, crate::city::TileBits::new(0b1100_0101));
    let connected_tiles = vec![true; CITY_TILES];
    let power_maps = power_service_maps(&city);

    assert_eq!(
        connected_water_source_capacity_tiles(&city, &connected_tiles, &power_maps),
        640
    );
}

#[test]
fn connected_powered_water_source_accepts_powered_non_origin_footprint_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 10..=12 {
        for x in 10..=12 {
            city.set_building(x, y, 250);
        }
    }

    let mut connected_tiles = vec![false; CITY_TILES];
    for y in 10..=12 {
        for x in 10..=12 {
            connected_tiles[y * CITY_WIDTH + x] = true;
        }
    }

    let mut power_maps = PowerServiceMaps {
        conductive_tiles: vec![false; CITY_TILES],
        energized_tiles: vec![false; CITY_TILES],
        powerable_tiles: vec![false; CITY_TILES],
        powered_tiles: vec![false; CITY_TILES],
        ..Default::default()
    };
    power_maps.powered_tiles[12 * CITY_WIDTH + 12] = true;

    assert!(connected_powered_building_footprint_at_origin(
        &city,
        &connected_tiles,
        &power_maps,
        10,
        10,
        250,
        3
    ));

    connected_tiles[11 * CITY_WIDTH + 11] = false;
    assert!(!connected_powered_building_footprint_at_origin(
        &city,
        &connected_tiles,
        &power_maps,
        10,
        10,
        250,
        3
    ));
}

#[test]
fn water_treatment_plant_count_uses_contained_multitile_origin() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 244);
        }
    }

    assert_eq!(water_treatment_plant_count(&city), 1);
}

#[test]
fn water_treatment_summary_requires_one_complete_plant_per_two_thousand_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    assert_eq!(
        water_treatment_summary(&city),
        WaterTreatmentSummary {
            plants: 0,
            required_plants: 9
        }
    );

    for plant in 0..8 {
        let origin_x = 10 + plant * 3;
        for y in 10..=11 {
            for x in origin_x..=origin_x + 1 {
                city.set_building(x, y, 244);
            }
        }
    }
    assert_eq!(water_treatment_plant_count(&city), 8);
    let under = water_treatment_summary(&city);
    assert_eq!(under.missing_plants(), 1);
    assert!(!under.has_bonus());

    for y in 20..=21 {
        for x in 20..=21 {
            city.set_building(x, y, 244);
        }
    }
    let enough = water_treatment_summary(&city);
    assert_eq!(enough.plants, 9);
    assert_eq!(enough.missing_plants(), 0);
    assert!(enough.has_bonus());
}

#[test]
fn water_treatment_count_ignores_partial_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 244);
    city.set_building(11, 10, 244);
    city.set_building(10, 11, 244);

    assert_eq!(water_treatment_plant_count(&city), 0);
}

#[test]
fn place_network_path_preserves_watered_and_xtrf_like_pe_place_tile() {
    // PE PlaceTile does not invent-clear watered/piped or write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-net-watered.sc2", "NetWatered").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x2a);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(20, 20)], NetworkKind::Road), 1);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(is_road_tile(state.city.building_at(20, 20)));
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x2a));
}

#[test]
fn place_power_network_preserves_watered_and_xtrf_like_pe_place_tile() {
    // PE FUN_0044A8A0 / PlaceTile: stamp XBIT 0x80 only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-pwr-xtrf.sc2", "PwrXtrf").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(22, 22, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(22, 22, 0x31);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(22, 22)], NetworkKind::Power), 1);
    let bits = state.city.tile_bits_at(22, 22).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert!(bits.powerable());
    assert_eq!(state.city.traffic_density_at(22, 22), Some(0x31));
}

#[test]
fn place_rail_network_preserves_watered_and_xtrf_like_pe_place_tile() {
    // PE PlaceTile path: leave watered/piped/XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-rail-xtrf.sc2", "RailXtrf").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(24, 24, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(24, 24, 0x27);
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&[(24, 24)], NetworkKind::Rail), 1);
    let bits = state.city.tile_bits_at(24, 24).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(24, 24), Some(0x27));
}

#[test]
fn day20_water_power_maps_use_process_crt_seed_not_city_name() {
    // Day-20 water pump power gate shares FUN_004679B0 process CRT with day-1 power.
    let mut city = City::new_blank("/tmp/sc2000-day20-water.sc2", "Day20Water").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xf6f6_6f6f);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    let _ = water_service_maps_with_entry_seed(&state.city, process);
    let _ = water_used_percent_for_city_with_entry_seed(&state.city, process);
}

#[test]
fn mac68k_day20_does_not_write_win95_water_percent_extension() {
    let mut city = City::new_blank("/tmp/sc2000-mac-d20-water-extension.sc2", "MacD20Ext")
        .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_misc_u32_at(MISC_WATER_USED_PERCENT_OFFSET, 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    state.sim_calendar_day20_water();

    assert_eq!(
        state.city.misc_u32_at(MISC_WATER_USED_PERCENT_OFFSET),
        Some(0),
        "Mac CODE 4's serialized state vector has no PE water-percent mirror here"
    );
}

#[test]
fn mac68k_edit_map_tree_preserves_xzon_water_clears_low_nibble() {
    let mut city =
        City::new_blank("/tmp/sc2000-editmap-xzon-mac.sc2", "EditXzonMac").expect("blank");
    city.set_zone(35, 35, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_zone(36, 36, ZONE_RESIDENTIAL_LIGHT | 0x40);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_edit_map_tree_at(35, 35, 0));
    state.place_edit_map_water_at(36, 36);
    assert_eq!(
        state.city.zone_at(35, 35).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x80)
    );
    assert_eq!(state.city.zone_at(36, 36).map(|z| z.raw_bits()), Some(0x40));
}

#[test]
fn place_edit_map_water_and_tree_leave_xzon_like_pe_place_tile() {
    // Edit-map water/tree use PlaceTile XBLD clear only — leave XZON.
    let mut city = City::new_blank("/tmp/sc2000-editmap-xzon.sc2", "EditXzon").expect("blank");
    city.set_zone(35, 35, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_zone(36, 36, ZONE_RESIDENTIAL_LIGHT | 0x40);
    let mut state = test_state(city);
    assert!(state.place_edit_map_tree_at(35, 35, 0));
    state.place_edit_map_water_at(36, 36);
    assert_eq!(
        state.city.zone_at(35, 35).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x80)
    );
    assert_eq!(
        state.city.zone_at(36, 36).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | 0x40)
    );
}

#[test]
fn place_edit_map_tree_preserves_watered_and_xtrf() {
    // Edit-map tree: terrain/XBLD/XZON only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-emap-tree-xtrf.sc2", "ETreeX").expect("blank");
    city.set_tile_bits(35, 35, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(35, 35, 0x3a);
    let mut state = test_state(city);
    assert!(state.place_edit_map_tree_at(35, 35, 0));
    let bits = state.city.tile_bits_at(35, 35).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(35, 35), Some(0x3a));
}

#[test]
fn place_edit_map_tree_preserves_xtxt_like_pe_place_tile() {
    // Edit-map tree clears XBLD via PlaceTile; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-emap-tree-xtxt.sc2", "ETreeT").expect("blank");
    city.set_building(35, 35, 7);
    city.set_text_overlay(35, 35, 0x47);
    let mut state = test_state(city);
    assert!(state.place_edit_map_tree_at(35, 35, 0));
    assert_eq!(state.city.building_at(35, 35), 0);
    assert_eq!(state.city.text_overlay_at(35, 35), Some(0x47));
}

#[test]
fn mac68k_place_edit_map_tree_preserves_xtxt_like_pe_place_tile() {
    // Native Mac growth places XBLD 8 from 7 without changing XTXT.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-tree-xtxt.sc2", "MacETreeT").expect("blank");
    city.set_building(35, 35, 7);
    city.set_text_overlay(35, 35, 0x47);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_edit_map_tree_at(35, 35, 0));
    assert_eq!(state.city.building_at(35, 35), 8);
    assert_eq!(state.city.text_overlay_at(35, 35), Some(0x47));
}

#[test]
fn mac68k_place_edit_map_tree_preserves_xtrf() {
    // Shared edit-map tree leaves XTRF; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-tree-xtrf.sc2", "MacETreeX").expect("blank");
    city.set_traffic_density(35, 35, 0x3a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_edit_map_tree_at(35, 35, 0));
    assert_eq!(state.city.traffic_density_at(35, 35), Some(0x3a));
}

#[test]
fn mac68k_edit_map_vegetation_forest_matches_fixed_candidate_trace() {
    let mut city = City::new_blank("/tmp/mac-edit-forest.sc2", "MacForest").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_terrain(63, 58, 0x0d);
    city.set_text_overlay(63, 58, 0x49);
    city.set_traffic_density(63, 58, 0x63);
    let empty_before = city.mac_xbld_tile_count(0);
    let six_before = city.mac_xbld_tile_count(6);
    let seven_before = city.mac_xbld_tile_count(7);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::Forest;
    state.mac_process_rng = MacToolboxRng::new(42);

    assert!(state.apply_tool_at_tile_with_shift(64, 64, false));

    // Scalar trace from CODE 5 0x3556..0x36a2 and QuickDraw seed 42.
    // All twenty candidates are distinct, so each spends a fifth class draw.
    let expected = vec![
        (57, 61, 6),
        (60, 60, 6),
        (60, 62, 7),
        (62, 67, 6),
        (63, 57, 7),
        (63, 58, 6),
        (63, 60, 6),
        (63, 63, 6),
        (63, 65, 6),
        (63, 70, 7),
        (64, 62, 7),
        (64, 63, 7),
        (64, 66, 7),
        (65, 60, 7),
        (65, 65, 7),
        (66, 65, 6),
        (66, 66, 6),
        (67, 60, 6),
        (70, 62, 7),
        (71, 58, 7),
    ];
    let mut actual = Vec::new();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let building = state.city.building_at(x, y);
            if building != 0 {
                actual.push((x, y, building));
            }
            assert_eq!(
                state.city.terrain_at(x, y),
                if (x, y) == (63, 58) { 0x0d } else { 0 }
            );
        }
    }
    assert_eq!(actual, expected);
    assert_eq!(state.mac_process_rng.seed(), 959_010_049);
    assert_eq!(
        state.city.mac_xbld_tile_count(0),
        empty_before.wrapping_sub(20)
    );
    assert_eq!(
        state.city.mac_xbld_tile_count(6),
        six_before.wrapping_add(10)
    );
    assert_eq!(
        state.city.mac_xbld_tile_count(7),
        seven_before.wrapping_add(10)
    );
    assert_eq!(state.city.text_overlay_at(63, 58), Some(0x49));
    assert_eq!(state.city.traffic_density_at(63, 58), Some(0x63));
}

#[test]
fn mac68k_edit_map_vegetation_forest_refusals_still_spend_eighty_draws() {
    for (x, y, clear) in [(64, 64, false), (0, 0, false), (0, 0, true)] {
        let mut city = City::new_blank("/tmp/mac-edit-forest-wet.sc2", "MacForestWet").unwrap();
        for tx in 0..CITY_WIDTH {
            for ty in 0..CITY_HEIGHT {
                city.set_tile_bits(tx, ty, TileBits::new(4));
            }
        }
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(42);
        let mut expected = state.mac_process_rng;
        for _ in 0..80 {
            expected.random_u16();
        }

        let placed = if clear {
            state.remove_edit_map_forest_at(x, y)
        } else {
            state.place_edit_map_forest_at(x, y)
        };

        assert_eq!(placed, 0);
        assert_eq!(state.mac_process_rng.seed(), expected.seed());
    }
}

#[test]
fn mac_edit_map_height_raise_lower_use_native_jitter_and_classifier() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (tool, initial, expected) in [
            (EditTool::RaiseTerrain, 8, 9),
            (EditTool::LowerTerrain, 9, 8),
        ] {
            let mut city =
                City::new_blank("/tmp/mac-height-jitter.sc2", "MacHeightJitter").unwrap();
            city.kind = kind;
            city.set_mac_selected_water_height(0);
            // Independent QuickDraw seed-42 trace: first four words are
            // 50534,43919,23481,65271; candidate (64-1,64-2), seed 565444343.
            city.set_altm_word(63, 62, (0x21 << 10) | (3 << 5) | initial);
            city.set_building(63, 62, 6);
            city.set_underground(63, 62, 0x23);
            city.set_text_overlay(63, 62, 0xc9);
            city.set_traffic_density(63, 62, 0x63);
            let funds = city.city_funds();
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.mode = AppMode::EditNewMap;
            state.tool = tool;
            state.mac_process_rng = MacToolboxRng::new(42);
            let crt = state.pe_process_crt.seed();

            assert!(state.apply_tool_at_tile(64, 64), "{kind:?}, {tool:?}");

            assert_eq!(state.mac_process_rng.seed(), 565444343);
            assert_eq!(state.pe_process_crt.seed(), crt);
            assert_eq!(state.city.altitude_at(63, 62), expected);
            assert_eq!(state.city.water_height_at(63, 62), 3);
            assert_eq!(state.city.altm_at(63, 62).tunnel_levels, 0x21);
            assert_eq!(state.city.altitude_at(64, 64), 8);
            assert_eq!(state.city.building_at(63, 62), 6);
            assert_eq!(state.city.underground_at(63, 62), 0);
            assert_eq!(state.city.text_overlay_at(63, 62), Some(0xc9));
            assert_eq!(state.city.traffic_density_at(63, 62), Some(0x63));
            assert_eq!(state.city.city_funds(), funds);
            assert_eq!(state.pe_lower_hold_ui_refresh_count, 0);
        }
    }
}

#[test]
fn mac_edit_map_height_off_map_candidate_spends_four_draws_without_clamping() {
    for tool in [EditTool::RaiseTerrain, EditTool::LowerTerrain] {
        let mut city = City::new_blank("/tmp/mac-height-edge.sc2", "MacHeightEdge").unwrap();
        city.set_text_overlay(0, 0, 0xc9);
        city.set_underground(0, 0, 0x23);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::EditNewMap;
        state.tool = tool;
        state.mac_process_rng = MacToolboxRng::new(42);
        let crt = state.pe_process_crt.seed();

        // The sampled point is valid; its random candidate (-1,-2) is not.
        assert!(!state.apply_tool_at_tile(0, 0));

        assert_eq!(state.mac_process_rng.seed(), 565444343);
        assert_eq!(state.pe_process_crt.seed(), crt);
        assert_eq!(state.city.altitude_at(0, 0), 8);
        assert_eq!(state.city.text_overlay_at(0, 0), Some(0xc9));
        assert_eq!(state.city.underground_at(0, 0), 0x23);
    }
}

#[test]
fn mac_edit_map_height_hold_repeats_stationary_samples_without_pe_marker_cleanup() {
    for (tool, height) in [(EditTool::RaiseTerrain, 31), (EditTool::LowerTerrain, 0)] {
        let mut city = City::new_blank("/tmp/mac-height-hold.sc2", "MacHeightHold").unwrap();
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                city.set_altitude(x, y, height, 0);
            }
        }
        city.set_text_overlay(64, 64, 0xc9);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::EditNewMap;
        state.tool = tool;
        state.hover_tile = Some((64, 64));
        state.mac_process_rng = MacToolboxRng::new(42);
        let crt = state.pe_process_crt.seed();

        assert!(state.begin_edit_map_height_hold()); // Press: one rejected native sample.
        assert_eq!(state.mac_process_rng.seed(), 565444343);
        // Motion retargets; it must not fill the path or spend a separate sample.
        assert_eq!(
            state.paint_edit_map_tool_drag_to(Some((60, 60)), (64, 64), false),
            (0, Some((64, 64)))
        );
        assert_eq!(state.mac_process_rng.seed(), 565444343);
        assert!(!state.tick_edit_map_height_hold(true));
        assert_eq!(state.mac_process_rng.seed(), 1100194760);
        assert!(!state.tick_edit_map_height_hold(true)); // Same pointer, another sample.
        assert_eq!(state.mac_process_rng.seed(), 1751246343);
        assert!(!state.tick_edit_map_height_hold(false));
        state.hover_tile = None;
        assert!(!state.tick_edit_map_height_hold(true));
        state.hover_tile = Some((64, 64));
        state.mode = AppMode::CityView;
        assert!(!state.tick_edit_map_height_hold(true));
        state.mode = AppMode::EditNewMap;
        assert_eq!(state.mac_process_rng.seed(), 1751246343);
        state.release_left_mouse_drag(false);
        assert_eq!(state.city.text_overlay_at(64, 64), Some(0xc9));
        assert_eq!(state.pe_process_crt.seed(), crt);
        assert_eq!(state.pe_lower_hold_ui_refresh_count, 0);
    }
}

#[test]
fn mac_edit_map_level_copies_press_height_and_native_record_zero() {
    fn planes(city: &City) -> u64 {
        let mut digest = 0xcbf2_9ce4_8422_2325u64;
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                for byte in [city.altitude_at(x, y), city.terrain_at(x, y)] {
                    digest = (digest ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                }
            }
        }
        digest
    }
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-level-native.sc2", "MacLevelNative").unwrap();
        city.kind = kind;
        city.set_mac_selected_water_height(0);
        city.set_altitude(64, 64, 10, 0);
        city.set_altm_word(67, 64, (0x21 << 10) | (3 << 5) | 8);
        city.set_text_overlay(0, 0, 0x41);
        city.set_text_overlay(64, 64, 0x42);
        city.set_traffic_density(64, 64, 0x63);
        city.mac_object_slot_records_mut()[0] =
            Win95OverlayRecord::from_bytes(&[9, 7, 5, 11, 12, 13, 14, 15, 16, 17, 18, 19]);
        let funds = city.city_funds();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::EditNewMap;
        state.tool = EditTool::LevelTerrain;
        state.mac_process_rng = MacToolboxRng::new(42);
        let crt = state.pe_process_crt.seed();

        assert!(state.begin_edit_map_level_drag_at(64, 64));

        assert_eq!(
            state.city.traffic_overlays().unwrap()[0].to_bytes(),
            [4, 2, 5, 64, 64, 0, 0, 0, 0, 0, 18, 19]
        );
        assert_eq!(state.city.text_overlay_at(0, 0), Some(0));
        assert_eq!(state.city.text_overlay_at(64, 64), Some(0xc9));
        // Independent scalar source transcription, not original runtime output.
        assert_eq!(planes(&state.city), 0xb1da_f2d2_a083_0f37);

        state.hover_tile = Some((66, 64));
        assert!(state.update_edit_map_level_drag());
        assert_eq!(planes(&state.city), 0x250f_c1ed_a8e3_c9df);
        assert_eq!(state.city.altitude_at(67, 64), 10);
        assert_eq!(state.city.water_height_at(67, 64), 3);
        assert_eq!(state.city.altm_at(67, 64).tunnel_levels, 0x21);
        assert_eq!(state.city.text_overlay_at(64, 64), Some(0));
        assert_eq!(state.city.text_overlay_at(66, 64), Some(0xc9));
        assert_eq!(
            state.city.traffic_overlays().unwrap()[0].to_bytes(),
            [4, 1, 5, 66, 64, 0, 0, 0, 0, 0, 18, 19]
        );
        assert!(!state.update_edit_map_level_drag()); // Same tile: retain the marker.
        state.hover_tile = None;
        assert!(!state.update_edit_map_level_drag()); // Off-map: retain the current marker.
        assert_eq!(state.city.text_overlay_at(66, 64), Some(0xc9));
        assert_eq!(state.finish_edit_map_level_drag(), 1);
        assert_eq!(state.city.text_overlay_at(66, 64), Some(0));
        assert_eq!(state.city.traffic_overlays().unwrap()[0].kind, 4); // Exit is bounds refresh only.
        assert_eq!(state.mac_process_rng.seed(), 42);
        assert_eq!(state.pe_process_crt.seed(), crt);
        assert_eq!(state.city.city_funds(), funds);
        assert_eq!(state.city.traffic_density_at(64, 64), Some(0x63));
        assert!(!uses_win95_overlay_draw(&state.city)); // Reserved slot zero never chooses road traffic.
    }
}

#[test]
fn mac_edit_map_level_origin_cancel_retains_native_partial_record_writes() {
    let mut city = City::new_blank("/tmp/mac-level-origin.sc2", "MacLevelOrigin").unwrap();
    city.set_text_overlay(0, 0, 0x49);
    city.mac_object_slot_records_mut()[0] =
        Win95OverlayRecord::from_bytes(&[9, 7, 5, 11, 12, 13, 14, 15, 16, 17, 18, 19]);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::LevelTerrain;
    assert!(state.begin_edit_map_level_drag_at(0, 0));
    // Previous tile starts at (0,0), so the first sample there is equal and skips.
    assert_eq!(state.city.text_overlay_at(0, 0), Some(0x49));
    assert_eq!(
        state.city.traffic_overlays().unwrap()[0].to_bytes(),
        [4, 0, 5, 11, 12, 0, 0, 0, 0, 0, 18, 19]
    );
    state.cancel_edit_map_level_drag();
    assert_eq!(state.city.text_overlay_at(0, 0), Some(0)); // Native exit clears even without a move.
    assert_eq!(state.city.traffic_overlays().unwrap()[0].tile_x, 11);
    assert_eq!(state.city.traffic_overlays().unwrap()[0].kind, 4);
    assert!(!state.edit_map_level_drag_active());
}

#[test]
fn mac68k_edit_map_vegetation_forest_shift_thins_and_revisits_candidates() {
    // Zero is a valid QuickDraw fixed point: all twenty attempts hit center.
    // Forest removes one vegetation level per hit; its ordinary arm accepts
    // XBLD 5, unlike the single-tree worker's explicit radioactivity guard.
    for (old, clear, new, placements) in [(12, true, 0, 7), (13, true, 13, 0), (5, false, 11, 20)] {
        let mut city = City::new_blank("/tmp/mac-edit-forest-shift.sc2", "MacForestShift").unwrap();
        city.set_building(44, 44, old);
        city.set_terrain(44, 44, 0x06);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(0);

        let actual = if clear {
            state.remove_edit_map_forest_at(44, 44)
        } else {
            state.place_edit_map_forest_at(44, 44)
        };

        assert_eq!(actual, placements, "old={old}, clear={clear}");
        assert_eq!(state.city.building_at(44, 44), new);
        assert_eq!(state.city.terrain_at(44, 44), 0x06);
        assert_eq!(state.mac_process_rng.seed(), 0);
    }
}

#[test]
fn place_edit_map_tree_uses_place_tile_clear() {
    // Edit-map tree clears XBLD via PlaceTile `0x441f00`(0); leave XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-emap-tree-place.sc2", "ETreeP").expect("blank");
    city.set_building(35, 35, 7);
    city.set_traffic_density(35, 35, 0x3a);
    let mut state = test_state(city);
    assert!(state.place_edit_map_tree_at(35, 35, 0));
    assert_eq!(state.city.building_at(35, 35), 0);
    assert!(is_edit_map_tree_terrain(state.city.terrain_at(35, 35)));
    assert_eq!(state.city.traffic_density_at(35, 35), Some(0x3a));
}

#[test]
fn mac68k_edit_map_water_single_cell_refusals_leave_state_unchanged() {
    for (bits, xtxt, building) in [
        (4, 0, 0),
        (0, 0xfa, 0),
        (0, 0xff, 13),
        (0, 0, 5),
        (0, 0, 14),
        (0, 0, 255),
    ] {
        let mut city = City::new_blank("/tmp/mac-editor-water-gates.sc2", "MacWaterGates").unwrap();
        city.set_terrain(64, 64, 0x06);
        city.set_altitude(64, 64, 7, 3);
        city.set_building(64, 64, building);
        city.set_tile_bits(64, 64, TileBits::new(bits));
        city.set_zone(64, 64, 0x82);
        city.set_text_overlay(64, 64, xtxt);
        city.set_traffic_density(64, 64, 0x63);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(3);

        state.place_edit_map_water_at(64, 64);

        assert_eq!(
            state.city.terrain_at(64, 64),
            0x06,
            "bits={bits}, xtxt={xtxt}, building={building}"
        );
        assert_eq!(
            (
                state.city.altitude_at(64, 64),
                state.city.water_height_at(64, 64)
            ),
            (7, 3)
        );
        assert_eq!(state.city.building_at(64, 64), building);
        assert_eq!(state.city.tile_bits_at(64, 64), Some(TileBits::new(bits)));
        assert_eq!(state.city.zone_at(64, 64).unwrap().raw_bits(), 0x82);
        assert_eq!(state.city.text_overlay_at(64, 64), Some(xtxt));
        assert_eq!(state.city.traffic_density_at(64, 64), Some(0x63));
        assert_eq!(state.mac_process_rng.seed(), 3);
    }
}

#[test]
fn mac68k_edit_map_water_zone_clear_survives_component_noop() {
    for current in [0x2e, 0x3e, 0x3d, 0x25] {
        let mut city = City::new_blank("/tmp/mac-editor-water-noop.sc2", "MacWaterNoop").unwrap();
        city.set_terrain(64, 64, current);
        city.set_altitude(64, 64, 7, 3);
        city.set_building(64, 64, 13);
        city.set_zone(64, 64, 0x82);
        if current == 0x25 {
            city.set_tile_bits(64, 63, TileBits::new(4));
        }
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;

        state.place_edit_map_water_at(64, 64);

        assert_eq!(state.city.zone_at(64, 64).unwrap().raw_bits(), 0x80);
        assert_eq!(state.city.terrain_at(64, 64), current);
        assert_eq!(state.city.building_at(64, 64), 13);
        assert_eq!(
            (
                state.city.altitude_at(64, 64),
                state.city.water_height_at(64, 64)
            ),
            (7, 3)
        );
        assert!(!state.city.tile_bits_at(64, 64).unwrap().water_tile());
    }
}

#[test]
fn mac68k_edit_map_water_place_tile_precedes_zone_clear() {
    let mut city = City::new_blank("/tmp/mac-editor-water-zone7.sc2", "MacWaterZone7").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_building(64, 64, 13);
    city.set_zone(64, 64, 0x87);
    let old_count = city.mac_xbld_tile_count(13);
    let empty_count = city.mac_xbld_tile_count(0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    state.place_edit_map_water_at(64, 64);

    assert_eq!(state.city.zone_at(64, 64).unwrap().raw_bits(), 0x80);
    assert_eq!(state.city.building_at(64, 64), 0);
    // Native PlaceTile still sees type 7 and bypasses the ordinary census.
    assert_eq!(state.city.mac_xbld_tile_count(13), old_count);
    assert_eq!(state.city.mac_xbld_tile_count(0), empty_count);
}

#[test]
fn mac68k_edit_map_water_stream_uses_hundred_steps_without_single_cell_gates() {
    let mut city = City::new_blank("/tmp/mac-editor-stream-length.sc2", "MacStreamLength").unwrap();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            city.set_altitude(x, y, 8, 3);
        }
    }
    for y in 5..=105 {
        city.set_zone(64, y, 0x82);
        city.set_text_overlay(64, y, 0xfa);
        city.set_traffic_density(64, y, 0x63);
    }
    city.set_building(64, 5, 255);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_process_rng = MacToolboxRng::new(0);

    assert_eq!(state.place_edit_map_stream_at(64, 5), 101);

    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            assert_eq!(
                state.city.tile_bits_at(x, y).unwrap().water_tile(),
                x == 64 && (5..=105).contains(&y)
            );
        }
    }
    for y in 5..=105 {
        assert_eq!(state.city.water_height_at(64, y), 8);
        assert_eq!(state.city.zone_at(64, y).unwrap().raw_bits(), 0x82);
        assert_eq!(state.city.text_overlay_at(64, y), Some(0xfa));
        assert_eq!(state.city.traffic_density_at(64, y), Some(0x63));
    }
    assert_eq!(state.city.building_at(64, 5), 0);
    assert_eq!(state.city.terrain_at(64, 5), 0x43);
    assert_eq!(state.city.terrain_at(64, 6), 0x40);
    assert_eq!(state.city.terrain_at(64, 105), 0x45);
    assert_eq!(state.mac_process_rng.seed(), 0);
}

#[test]
fn mac68k_edit_map_water_stream_matches_seeded_source_trace() {
    let city = City::new_blank("/tmp/mac-editor-stream-trace.sc2", "MacStreamTrace").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_process_rng = MacToolboxRng::new(42);

    assert_eq!(state.place_edit_map_stream_at(64, 64), 52);

    let mut expected = vec![
        (64, 64),
        (64, 65),
        (63, 65),
        (62, 65),
        (61, 65),
        (61, 64),
        (60, 64),
        (59, 64),
        (58, 64),
        (57, 64),
        (56, 64),
        (56, 63),
        (55, 63),
        (55, 64),
        (55, 65),
        (56, 65),
        (57, 65),
        (57, 66),
        (57, 67),
        (57, 68),
        (56, 68),
        (56, 69),
        (56, 70),
        (56, 71),
        (56, 72),
        (56, 73),
        (57, 73),
        (57, 74),
        (58, 74),
        (59, 74),
        (59, 75),
        (58, 75),
        (57, 75),
        (56, 75),
        (56, 76),
        (55, 76),
        (55, 75),
        (54, 75),
        (53, 75),
        (53, 76),
        (53, 77),
        (52, 77),
        (52, 76),
        (52, 75),
        (52, 74),
        (53, 74),
        (53, 73),
        (53, 72),
        (52, 72),
        (51, 72),
        (51, 73),
        (52, 73),
    ];
    expected.sort_unstable();
    let mut actual = Vec::new();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            if state.city.tile_bits_at(x, y).unwrap().water_tile() {
                actual.push((x, y));
            }
        }
    }
    assert_eq!(actual, expected);
    // 82 QuickDraw heading draws; no length/y/x draws at this editor caller.
    assert_eq!(state.mac_process_rng.seed(), 569_825_713);
}

#[test]
fn place_edit_map_water_preserves_watered_and_xtrf() {
    // Edit-map water: terrain/ALTM/XBLD/XZON only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-emap-water-xtrf.sc2", "EWaterX").expect("blank");
    city.set_tile_bits(36, 36, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(36, 36, 0x41);
    let mut state = test_state(city);
    state.place_edit_map_water_at(36, 36);
    let bits = state.city.tile_bits_at(36, 36).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(36, 36), Some(0x41));
}

#[test]
fn place_edit_map_water_preserves_xtxt_like_pe_place_tile() {
    // Edit-map water clears XBLD via PlaceTile `0x441f00`; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-emap-water-xtxt.sc2", "EWaterT").expect("blank");
    city.set_building(36, 36, 5);
    city.set_text_overlay(36, 36, 0x34);
    let mut state = test_state(city);
    state.place_edit_map_water_at(36, 36);
    assert_eq!(state.city.building_at(36, 36), 0);
    assert_eq!(state.city.text_overlay_at(36, 36), Some(0x34));
}

#[test]
fn mac68k_place_edit_map_water_preserves_xtxt_like_pe_place_tile() {
    // Native single-cell water accepts XBLD 13, then leaves XTXT intact.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-water-xtxt.sc2", "MacEWaterT").expect("blank");
    city.set_building(36, 36, 13);
    city.set_text_overlay(36, 36, 0x34);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.place_edit_map_water_at(36, 36);
    assert_eq!(state.city.building_at(36, 36), 0);
    assert_eq!(state.city.text_overlay_at(36, 36), Some(0x34));
}

#[test]
fn mac68k_place_edit_map_water_preserves_xtrf() {
    // Shared edit-map water leaves XTRF; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-water-xtrf.sc2", "MacEWaterX").expect("blank");
    city.set_traffic_density(36, 36, 0x41);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.place_edit_map_water_at(36, 36);
    assert_eq!(state.city.traffic_density_at(36, 36), Some(0x41));
}

#[test]
fn place_edit_map_forest_preserves_watered_and_xtrf() {
    // Edit-map forest reuses tree stamp — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-emap-forest-xtrf.sc2", "EForestX").expect("blank");
    city.set_tile_bits(40, 40, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(40, 40, 0x2e);
    let mut state = test_state(city);
    assert!(state.place_edit_map_forest_at(40, 40) > 0);
    let bits = state.city.tile_bits_at(40, 40).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x2e));
}

#[test]
fn place_edit_map_forest_preserves_xtxt_like_pe_place_tile() {
    // Edit-map forest reuses PlaceTile clear; leave XTXT on stamped cells.
    let mut city = City::new_blank("/tmp/sc2000-emap-forest-xtxt.sc2", "EForestT").expect("blank");
    for y in 35..=45 {
        for x in 35..=45 {
            city.set_text_overlay(x, y, 0x48);
        }
    }
    let mut state = test_state(city);
    assert!(state.place_edit_map_forest_at(40, 40) > 0);
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0x48));
}

#[test]
fn mac68k_place_edit_map_forest_preserves_xtxt_like_pe_place_tile() {
    // Shared edit-map forest leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-forest-xtxt.sc2", "MacEForestT").expect("blank");
    for y in 35..=45 {
        for x in 35..=45 {
            city.set_text_overlay(x, y, 0x48);
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_edit_map_forest_at(40, 40) > 0);
    assert_eq!(state.city.text_overlay_at(40, 40), Some(0x48));
}

#[test]
fn mac68k_place_edit_map_forest_preserves_xtrf() {
    // Native Mac forest scatter changes XBLD without changing XTRF.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-forest-xtrf.sc2", "MacEForestX").expect("blank");
    city.set_traffic_density(40, 40, 0x2e);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_edit_map_forest_at(40, 40) > 0);
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x2e));
}

#[test]
fn place_edit_map_stream_preserves_watered_and_xtrf() {
    // Edit-map stream reuses water stamp — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-emap-stream-xtrf.sc2", "EStreamX").expect("blank");
    city.set_altitude(42, 42, 8, 0);
    city.set_altitude(42, 43, 6, 0);
    city.set_tile_bits(42, 42, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(42, 42, 0x35);
    let mut state = test_state(city);
    assert!(state.place_edit_map_stream_at(42, 42) > 0);
    let bits = state.city.tile_bits_at(42, 42).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(42, 42), Some(0x35));
}

#[test]
fn place_edit_map_stream_preserves_xtxt_like_pe_place_tile() {
    // Edit-map stream reuses PlaceTile clear; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-emap-stream-xtxt.sc2", "EStreamT").expect("blank");
    city.set_altitude(42, 42, 8, 0);
    city.set_altitude(42, 43, 6, 0);
    city.set_building(42, 42, 5);
    city.set_text_overlay(42, 42, 0x3a);
    let mut state = test_state(city);
    assert!(state.place_edit_map_stream_at(42, 42) > 0);
    assert_eq!(state.city.building_at(42, 42), 0);
    assert_eq!(state.city.text_overlay_at(42, 42), Some(0x3a));
}

#[test]
fn mac68k_place_edit_map_stream_preserves_xtxt_like_pe_place_tile() {
    // Shared edit-map stream reuses PlaceTile; Mac68k must not invent an XTXT clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-stream-xtxt.sc2", "MacEStreamT").expect("blank");
    city.set_altitude(42, 42, 8, 0);
    city.set_altitude(42, 43, 6, 0);
    city.set_building(42, 42, 5);
    city.set_text_overlay(42, 42, 0x3a);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_edit_map_stream_at(42, 42) > 0);
    assert_eq!(state.city.building_at(42, 42), 0);
    assert_eq!(state.city.text_overlay_at(42, 42), Some(0x3a));
}

#[test]
fn mac68k_place_edit_map_stream_preserves_xtrf() {
    // Shared edit-map stream reuses water stamp; Mac68k must not invent an XTRF clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-emap-stream-xtrf.sc2", "MacEStreamX").expect("blank");
    city.set_altitude(42, 42, 8, 0);
    city.set_altitude(42, 43, 6, 0);
    city.set_traffic_density(42, 42, 0x35);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_edit_map_stream_at(42, 42) > 0);
    assert_eq!(state.city.traffic_density_at(42, 42), Some(0x35));
}

#[test]
fn army_runway_cross_stamps_over_tree_without_road_gate() {
    // PE PlaceTile(0xDD) when XBLD < 0x0D — no road-only invent gate.
    let mut city = City::new_blank("/tmp/sc2000-runway-tree.sc2", "RunwayTree").expect("blank");
    city.set_building(55, 55, 6);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_text_overlay(55, 55, 0x3b);
    let mut state = test_state(city);
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    assert_eq!(state.city.text_overlay_at(55, 55), Some(0x3b));
}

#[test]
fn mac68k_army_runway_cross_stamps_over_tree_without_road_gate() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-runway-tree.sc2", "MacRunwayTree").expect("blank");
    city.set_building(55, 55, 6);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
}

#[test]
fn army_runway_cross_preserves_watered_and_xtrf() {
    // PE clears XBIT allocation on the type-7 runway path and leaves XTRF unchanged.
    let mut city = City::new_blank("/tmp/sc2000-runway-xtrf.sc2", "RunwayX").expect("blank");
    city.set_building(55, 55, 29);
    city.set_zone(55, 55, ZONE_MILITARY | ZONE_CORNER_ALL);
    city.set_tile_bits(55, 55, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(55, 55, 0x46);
    let mut state = test_state(city);
    state.maybe_stamp_army_runway_cross(55, 55, false);
    assert_eq!(state.city.building_at(55, 55), RUNWAY_BUILDING_ID);
    let bits = state.city.tile_bits_at(55, 55).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert!(!bits.powered());
    assert!(!bits.powerable());
    assert_eq!(state.city.traffic_density_at(55, 55), Some(0x46));
}

#[test]
fn stamp_power_plant_xbit_powerable_preserves_watered_and_xtrf() {
    // PE plant placement OR XBIT 0x80 only — leave watered/piped/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-plant-xtrf.sc2", "PlantX").expect("blank");
    city.set_tile_bits(10, 10, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(10, 10, 0x29);
    stamp_power_plant_xbit_powerable(&mut city, &[(10, 10)]);
    let bits = city.tile_bits_at(10, 10).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert!(bits.powerable());
    assert_eq!(city.traffic_density_at(10, 10), Some(0x29));
}

#[test]
fn stamp_xbit_piped_preserves_watered_and_xtrf() {
    // PE OR of XBIT 0x20 only — leave watered/XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-piped-xtrf.sc2", "PipedX").expect("blank");
    city.set_tile_bits(12, 12, TileBits::new(0).with_watered(true));
    city.set_traffic_density(12, 12, 0x1f);
    stamp_xbit_piped(&mut city, 12, 12);
    let bits = city.tile_bits_at(12, 12).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(city.traffic_density_at(12, 12), Some(0x1f));
}

#[test]
fn power_plant_anchor_at_uses_contained_origin_for_multi_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 207);
        }
    }

    assert!(power_plant_anchor_at(&city, 20, 20));
    assert!(power_plant_anchor_at(&city, 21, 20));
    assert!(power_plant_anchor_at(&city, 23, 23));
    assert!(!power_plant_anchor_at(&city, 19, 20));
    assert!(!power_plant_anchor_at(&city, 24, 24));
}

#[test]
fn power_plant_output_tiles_respects_map_scan_crt_consumption() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    for y in 20..=23 {
        for x in 20..=23 {
            city.set_building(x, y, 204);
        }
    }

    let mut rng = pe_power_recalc_rng(&city);
    let wind = pe_power_c8_output_tiles(&city, 10, 10, pe_power_crt_rand_u16(&mut rng));
    let solar = pe_power_cc_output_tiles(&city, pe_power_crt_rand_u16(&mut rng));
    assert_eq!(power_plant_output_tiles(&city, 10, 10), wind);
    assert_eq!(power_plant_output_tiles(&city, 20, 20), solar);
}

#[test]
fn power_component_links_adjacent_plant_to_water_pump() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 220);
    city.set_building(9, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 9, 10, 30);
    assert_eq!(city.altitude_at(9, 10), 30);
    let maps = power_service_maps(&city);
    assert!(maps.energized_tiles[9 + 10 * CITY_WIDTH]);
    assert!(maps.energized_tiles[10 + 10 * CITY_WIDTH]);
}

#[test]
fn pe_water_pump_uses_energized_power_not_consumer_powered() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 220);
    power_water_source_for_test(&mut city, 10, 10);
    city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let power_maps = power_service_maps(&city);
    let index = 10 + 10 * CITY_WIDTH;
    assert!(power_maps.energized_tiles[index]);
    assert!(water_pump_has_power_in_capacity_estimate(
        &power_maps,
        index
    ));
}

#[test]
fn stamp_zone_development_preserves_xbit_watered_like_pe_436fad() {
    // PE ChooseAndPlace @ 0x436fad / 0x4370cb: OR XBIT 0xe0 only — keep watered 0x10.
    let mut city = City::new_blank("/tmp/sc2000-cap-watered.sc2", "CapWatered").expect("blank");
    city.set_tile_bits(20, 20, TileBits::new(0x11)); // salt 0x01 | watered 0x10
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.stamp_zone_development(&ZoneDevelopmentPlan {
        origin: (20, 20),
        zone_id: ZONE_RESIDENTIAL_LIGHT,
        building: 1, // construction / rubble-family 1×1
        footprint: vec![(20, 20)],
        place_as_construction: true,
    });
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.watered(), "ChooseAndPlace must not clear watered");
    assert!(bits.salt_water(), "unrelated XBIT bits preserved");
    assert_eq!(bits.raw_bits() & 0xe0, 0xe0);
}

#[test]
fn choose_and_place_multitile_xzon_corners_follow_pe_rotation() {
    // PE ChooseAndPlace @ 0x4370e9 / table 0x4dc4d0: NW,NE,SE,SW words by rotation.
    // Rust building_zone_bits_for_tile wants [NW, NE, SW, SE].
    const EXPECTED: [[u8; 4]; 4] = [
        [0x10, 0x20, 0x80, 0x40],
        [0x20, 0x40, 0x10, 0x80],
        [0x40, 0x80, 0x20, 0x10],
        [0x80, 0x10, 0x40, 0x20],
    ];
    for rotation in 0u8..4 {
        let mut city = City::new_blank("/tmp/sc2000-cap-corners.sc2", "CapCorners").expect("blank");
        city.set_rotation(rotation);
        let mut state = test_state(city);
        let footprint = vec![(20, 20), (21, 20), (20, 21), (21, 21)];
        state.stamp_zone_development(&ZoneDevelopmentPlan {
            origin: (20, 20),
            zone_id: ZONE_RESIDENTIAL_LIGHT,
            building: 141, // 2×2 XBLD
            footprint,
            place_as_construction: false,
        });
        let [nw, ne, sw, se] = EXPECTED[usize::from(rotation)];
        assert_eq!(
            state.city.zone_at(20, 20).unwrap().raw_bits() & 0xf0,
            nw,
            "rot{rotation} NW"
        );
        assert_eq!(
            state.city.zone_at(21, 20).unwrap().raw_bits() & 0xf0,
            ne,
            "rot{rotation} NE"
        );
        assert_eq!(
            state.city.zone_at(20, 21).unwrap().raw_bits() & 0xf0,
            sw,
            "rot{rotation} SW"
        );
        assert_eq!(
            state.city.zone_at(21, 21).unwrap().raw_bits() & 0xf0,
            se,
            "rot{rotation} SE"
        );
    }
}

#[test]
fn stamp_building_footprint_preserves_watered_like_pe_place_tile() {
    // PE PlaceTile does not invent-clear watered/piped before stamp.
    let mut city = City::new_blank("/tmp/sc2000-place-watered.sc2", "PlaceWatered").expect("blank");
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    state.stamp_building_footprint(20, 20, 112, &[(20, 20)]);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert_eq!(state.city.building_at(20, 20), 112);
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn place_building_at_runs_citytool_water_recalc_and_keeps_xtrf() {
    // Full CityTool placement follows PlaceTile with FUN_00464550; with no
    // source the served-water bit clears, while the unrelated XTRF byte remains.
    let mut city =
        City::new_blank("/tmp/sc2000-place-at-watered.sc2", "PlaceAtWatered").expect("blank");
    city.set_city_funds(50_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x42);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 112) > 0);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert_eq!(state.city.building_at(20, 20), 112);
    assert!(bits.piped());
    assert!(!bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x42));
}

#[test]
fn place_hydro_power_uses_neighbor_altm_orientation_like_pe() {
    // PE MapTool hydro @ `0x441610`: mask 0 + rot0 → delta 1 → `0xC7`; rot1 → `0xC6`.
    let mut city = City::new_blank("/tmp/sc2000-hydro-orient.sc2", "HydroOrient").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_view_rotation(0);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), 199);

    let mut city = City::new_blank("/tmp/sc2000-hydro-orient2.sc2", "HydroOrient2").expect("blank");
    city.set_city_funds(50_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_view_rotation(1);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), 198);
}

#[test]
fn place_hydro_power_preserves_watered_without_invent_refresh() {
    let mut city = City::new_blank("/tmp/sc2000-hydro-watered.sc2", "HydroWatered").expect("blank");
    city.set_city_funds(50_000);
    // Falling-water / waterfall terrain accepted by can_place_hydro_power_tile.
    city.set_terrain(20, 20, 0x3e);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x43);
    let mut state = test_state(city);
    assert_eq!(state.place_hydro_power_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), 199);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x43));
}

#[test]
fn onramp_refuses_when_xter_nonzero() {
    // PE MapTool Onramp @ `0x441869`: XTER `!= 0` → refuse before neighbor masks.
    let mut city = City::new_blank("/tmp/sc2000-onramp-xter.sc2", "OnrampXter").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    assert!(state.onramp_plan_at(ramp.0, ramp.1).is_some());
    state.city.set_terrain(ramp.0, ramp.1, 1);
    assert!(state.onramp_plan_at(ramp.0, ramp.1).is_none());
    assert!(!state.place_onramp_at(ramp.0, ramp.1));
    assert_eq!(state.city.building_at(ramp.0, ramp.1), 0);
}

#[test]
fn underground_actions_match_caller_water_pass_and_preserve_xtrf() {
    // Subway placement is reshape-only. The full pipe-tool action follows its
    // coordinate writer with FUN_00464550, clearing unsupported served-water;
    // neither path writes XTRF.
    let mut city = City::new_blank("/tmp/sc2000-under-watered.sc2", "UnderWatered").expect("blank");
    city.set_city_funds(50_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_tile_bits(22, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x77);
    city.set_traffic_density(22, 20, 0x88);
    let mut state = test_state(city);

    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Subway),
        1
    );
    let subway_bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(subway_bits.piped());
    assert!(subway_bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x77));

    assert_eq!(
        state.place_underground_at(22, 20, UndergroundKind::Pipes),
        1
    );
    let pipe_bits = state.city.tile_bits_at(22, 20).unwrap();
    // Pipe tool's outer caller recalculates water after the coordinate writer.
    assert!(pipe_bits.piped());
    assert!(!pipe_bits.watered());
    assert_eq!(state.city.traffic_density_at(22, 20), Some(0x88));
}

#[test]
fn stamp_building_footprint_multitile_xzon_corners_follow_pe_rotation() {
    // PE CityToolPlace @ 0x440440 / table 0x4dc4d0 — same CAP rotation corners.
    const EXPECTED: [[u8; 4]; 4] = [
        [0x10, 0x20, 0x80, 0x40],
        [0x20, 0x40, 0x10, 0x80],
        [0x40, 0x80, 0x20, 0x10],
        [0x80, 0x10, 0x40, 0x20],
    ];
    for rotation in 0u8..4 {
        let mut city =
            City::new_blank("/tmp/sc2000-place-corners.sc2", "PlaceCorners").expect("blank");
        city.set_rotation(rotation);
        city.set_city_funds(50_000);
        let mut state = test_state(city);
        let footprint = vec![(20, 20), (21, 20), (20, 21), (21, 21)];
        state.stamp_building_footprint(20, 20, 141, &footprint);
        let [nw, ne, sw, se] = EXPECTED[usize::from(rotation)];
        assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits() & 0xf0, nw);
        assert_eq!(state.city.zone_at(21, 20).unwrap().raw_bits() & 0xf0, ne);
        assert_eq!(state.city.zone_at(20, 21).unwrap().raw_bits() & 0xf0, sw);
        assert_eq!(state.city.zone_at(21, 21).unwrap().raw_bits() & 0xf0, se);
    }
}

#[test]
fn onramp_preserves_watered_when_setting_rotate_bit() {
    // Onramp must set rotate without invent-wiping watered/piped XBIT.
    let mut city =
        City::new_blank("/tmp/sc2000-onramp-watered.sc2", "OnrampWatered").expect("blank");
    city.set_city_funds(50_000);
    let mut state = test_state(city);
    state.tool = EditTool::Onramp;
    let ramp = (64, 64);
    stamp_onramp_fixture(&mut state, ramp, NETWORK_TOP, NETWORK_RIGHT);
    state.city.set_tile_bits(
        ramp.0,
        ramp.1,
        TileBits::new(0).with_piped(true).with_watered(true),
    );
    assert!(state.place_onramp_at(ramp.0, ramp.1));
    let bits = state.city.tile_bits_at(ramp.0, ramp.1).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(bits.rotate(), false); // NETWORK_TOP/RIGHT → building 93, rotate false
}

#[test]
fn flat_highway_preserves_watered_like_pe_place_tile() {
    // PE PlaceTile does not invent-clear watered/piped.
    let mut city = City::new_blank("/tmp/sc2000-hwy-watered.sc2", "HwyWatered").expect("blank");
    city.set_tile_bits(30, 30, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    state.stamp_flat_highway_tiles(106, &[(30, 30)]);
    let bits = state.city.tile_bits_at(30, 30).unwrap();
    assert_eq!(state.city.building_at(30, 30), 106);
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn place_highway_path_preserves_watered_and_xtrf_like_pe_place_tile() {
    // PE PlaceTile highway stamp: leave watered/piped/XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-hwy-path-xtrf.sc2", "HwyPathX").expect("blank");
    city.set_city_funds(50_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(20, 20, 0x4c);
    let mut state = test_state(city);
    assert_eq!(
        state.place_highway_path(&highway_drag_path((20, 20), (20, 20))),
        1
    );
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x4c));
}

#[test]
fn place_water_pump_ors_xbit_piped_like_pe_44b829() {
    // PE PlacePipesAtCoordinates @ 0x44b829: after underground pipe commit, OR XBIT 0x20.
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert_eq!(state.place_building_at(20, 20, 220), 1);
    assert_eq!(state.city.underground_at(20, 20), UNDER_TILE_PIPES_LR);
    assert!(state.city.tile_bits_at(20, 20).unwrap().piped());
}

#[test]
fn place_water_pump_refreshes_adjacent_user_pipe_like_pe_4016b8() {
    // PE PlacePipes @ 0x44b82f+: call 0x4016b8 on commit + orthogonal neighbors so
    // an existing user pipe rejoins the new water-pump pipe (`0x10`).
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert_eq!(
        state.place_underground_at(21, 20, UndergroundKind::Pipes),
        1
    );
    let before = state.city.underground_at(21, 20);
    assert!(underground_user_pipe_tile(before));
    assert_eq!(state.place_building_at(20, 20, 220), 1);
    let after = state.city.underground_at(21, 20);
    assert_ne!(
        after, before,
        "adjacent user pipe should reshape toward the new pump pipe"
    );
    assert!(
        underground_tile_connects_to(after, UndergroundKind::Pipes, NETWORK_LEFT),
        "pipe east of pump should gain a west (left) connection"
    );
}

#[test]
fn underground_tile_sprites_use_xbit_water_and_pipe_source() {
    let dry_user_pipe = UndergroundRenderTile {
        tile: 16,
        source: UndergroundTileSource::UserLaid,
        watered: false,
    };
    let watered_user_pipe = UndergroundRenderTile {
        watered: true,
        ..dry_user_pipe
    };
    let dry_building_pipe = UndergroundRenderTile {
        tile: UNDER_TILE_BUILDING_PIPES,
        source: UndergroundTileSource::BuildingInternal,
        watered: false,
    };
    let watered_building_pipe = UndergroundRenderTile {
        watered: true,
        ..dry_building_pipe
    };

    assert_eq!(underground_tile_sprite_id(dry_user_pipe), Some(1334));
    assert_eq!(underground_tile_sprite_id(watered_user_pipe), Some(1450));
    assert_eq!(underground_tile_sprite_id(dry_building_pipe), Some(1351));
    assert_eq!(
        underground_tile_sprite_id(watered_building_pipe),
        Some(1467)
    );
    assert_eq!(
        underground_tile_sprite_id(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::UserLaid,
            watered: true,
        }),
        Some(1467)
    );
    assert_eq!(
        underground_tile_sprite_id(UndergroundRenderTile {
            tile: UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR,
            source: UndergroundTileSource::UserLaid,
            watered: false,
        }),
        Some(1349)
    );
    assert_eq!(
        underground_tile_sprite_id(UndergroundRenderTile {
            tile: UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR,
            source: UndergroundTileSource::UserLaid,
            watered: true,
        }),
        Some(1465)
    );
    assert_eq!(
        underground_tile_sprite_id(UndergroundRenderTile {
            tile: 1,
            source: UndergroundTileSource::UserLaid,
            watered: true,
        }),
        Some(1319)
    );
    assert_eq!(
        underground_tile_sprite_id(UndergroundRenderTile {
            tile: UNDER_TILE_GREY_BOX_2,
            source: UndergroundTileSource::UserLaid,
            watered: true,
        }),
        Some(1353)
    );
}

#[test]
fn underground_render_tile_distinguishes_user_and_building_pipes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_building(10, 10, 112);
    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );

    city.set_tile_bits(10, 10, crate::city::TileBits::new(0b0010_0000));
    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );

    city.set_tile_bits(10, 10, crate::city::TileBits::new(0b0011_0000));
    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: true,
        })
    );

    city.set_underground(10, 10, 16);
    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: 16,
            source: UndergroundTileSource::UserLaid,
            watered: true,
        })
    );

    city.set_underground(10, 10, UNDER_TILE_BUILDING_PIPES);
    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered: true,
        })
    );
}

#[test]
fn water_source_xund_pipe_tiles_render_as_building_plumbing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    city.set_underground(10, 10, UNDER_TILE_BUILDING_PIPE_TEE);
    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPE_TEE,
            source: UndergroundTileSource::UserLaid,
            watered: false,
        })
    );

    city.set_building(10, 10, 220);
    let pump_tile = underground_render_tile(&city, 10, 10, 0).expect("pump underground tile");
    assert_eq!(
        pump_tile,
        UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPE_TEE,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        }
    );
    assert_eq!(underground_tile_sprite_id(pump_tile), Some(1346));

    for y in 20..=22 {
        for x in 20..=22 {
            city.set_building(x, y, 250);
            city.set_underground(x, y, UNDER_TILE_BUILDING_PIPE_TEE);
        }
    }
    assert_eq!(
        underground_render_tile(&city, 21, 21, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPE_TEE,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );
}

#[test]
fn pipe_placement_preserves_water_source_underground_stamp() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(10, 10, 220), 1);
    assert_eq!(state.city.underground_at(10, 10), UNDER_TILE_PIPES_LR);
    assert_eq!(
        state.place_underground_at(10, 10, UndergroundKind::Pipes),
        0
    );
    assert_eq!(state.city.underground_at(10, 10), UNDER_TILE_PIPES_LR);

    // Pump tile is protected/noop pass-through; only the empty ends stamp.
    assert_eq!(
        state.place_underground_path(&[(9, 10), (10, 10), (11, 10)], UndergroundKind::Pipes),
        2
    );

    assert_eq!(state.city.underground_at(10, 10), UNDER_TILE_PIPES_LR);
    assert_eq!(
        underground_render_tile(&state.city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_PIPES_LR,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );
    assert_eq!(state.city.underground_at(9, 10), 17);
    assert_eq!(state.city.underground_at(11, 10), 17);
}

#[test]
fn underground_tile_sprites_rotate_with_view() {
    assert_eq!(rotated_underground_tile(1, 0), 1);
    assert_eq!(rotated_underground_tile(1, 1), 2);
    assert_eq!(rotated_underground_tile(2, 1), 1);
    assert_eq!(rotated_underground_tile(2, 2), 2);

    assert_eq!(rotated_underground_tile(3, 1), 4);
    assert_eq!(rotated_underground_tile(6, 1), 3);
    assert_eq!(rotated_underground_tile(7, 1), 8);
    assert_eq!(rotated_underground_tile(10, 1), 7);
    assert_eq!(rotated_underground_tile(11, 1), 12);
    assert_eq!(rotated_underground_tile(14, 1), 11);
    assert_eq!(rotated_underground_tile(15, 3), 15);

    assert_eq!(rotated_underground_tile(16, 1), 17);
    assert_eq!(rotated_underground_tile(17, 1), 16);
    assert_eq!(rotated_underground_tile(31, 1), 32);
    assert_eq!(rotated_underground_tile(32, 1), 31);
    assert_eq!(rotated_underground_tile(31, 2), 31);
    assert_eq!(rotated_underground_tile(33, 1), 33);
    assert_eq!(rotated_underground_tile(UNDER_TILE_GREY_BOX_2, 1), 35);
}

#[test]
fn underground_terrain_sprites_follow_extracted_sc2k_art_ids() {
    assert_eq!(underground_terrain_sprite_id(0x00), 1305);
    assert_eq!(underground_terrain_sprite_id(0x0d), 1318);
    assert_eq!(underground_terrain_sprite_id(0x20), 1305);
    assert_eq!(underground_terrain_sprite_id(0x2d), 1318);
    assert_eq!(underground_terrain_sprite_id(0x30), 1305);
}

#[test]
fn map_tool_place_water_preserves_piped_watered_on_zoned_tile() {
    // PE zone tool writes XZON only (no invent water-network refresh). Stamp piped/watered
    // like a prior service pass; PlaceWater ORs water-tile and keeps them (`0x446a58`).
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    state.place_zone_at(11, 10, 1);
    state
        .city
        .set_tile_bits(11, 10, TileBits::new(0).with_piped(true).with_watered(true));
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());

    assert_eq!(state.place_surface_water_at(11, 10), 1);
    assert_eq!(state.city.terrain_at(11, 10), 0x3d);
    // PE @ 0x43fedf: XZON &= 0xf0 keeps corner bits from vacant zone stamp.
    assert_eq!(
        state.city.zone_at(11, 10).unwrap().raw_bits(),
        ZONE_CORNER_ALL
    );
    let bits = state.city.tile_bits_at(11, 10).unwrap();
    assert!(bits.water_tile());
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn map_tool_place_water_refuses_xtxt_at_or_above_0xfa() {
    // PE MapToolPlaceWater @ 0x43fe30: cmp XTXT, 0xfa / jb place.
    let mut city = City::new_blank("/tmp/sc2000-water-xtxt.sc2", "WaterXtxt").expect("blank");
    city.set_city_funds(20_000);
    city.set_text_overlay(20, 20, 0xf9);
    city.set_text_overlay(21, 20, 0xfa);
    city.set_text_overlay(22, 20, 0xff);
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.terrain_at(20, 20), 0x3d);
    assert_eq!(state.place_surface_water_at(21, 20), 0);
    assert_eq!(state.city.terrain_at(21, 20), 0);
    assert_eq!(state.place_surface_water_at(22, 20), 0);
    assert_eq!(state.city.terrain_at(22, 20), 0);
}

#[test]
fn map_tool_place_water_refuses_xbit_water_tile() {
    // PE MapToolPlaceWater @ 0x43fe63: refuse XBIT bit 0x04.
    let mut city = City::new_blank("/tmp/sc2000-water-xbit.sc2", "WaterXbit").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0x04));
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 0);
    assert_eq!(state.city.terrain_at(20, 20), 0);
}

#[test]
fn map_tool_place_water_allows_water_xter_without_water_tile_bit() {
    // PE MapToolPlaceWater: no general water-XTER refuse before helper.
    let mut city = City::new_blank("/tmp/sc2000-water-xter3d.sc2", "WaterXter3d").expect("blank");
    city.set_city_funds(20_000);
    city.set_terrain(20, 20, 0x3d);
    city.set_building(20, 20, 2);
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    // Helper mask0 + existing 0x3d → exit at nibble-XTER==-0x30 (no PlaceTile).
    assert_eq!(state.city.terrain_at(20, 20), 0x3d);
    assert_eq!(state.city.building_at(20, 20), 2);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn map_tool_place_water_waterfall_xter_still_returns_success() {
    // PE MapTool always calls helper then returns 1; helper noops on 0x2E/0x3E.
    let mut city = City::new_blank("/tmp/sc2000-water-fall-mt.sc2", "WaterFallMt").expect("blank");
    city.set_city_funds(20_000);
    city.set_terrain(20, 20, 0x3e);
    city.set_building(20, 20, 3);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.terrain_at(20, 20), 0x3e);
    assert_eq!(state.city.building_at(20, 20), 3);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().water_tile());
    // PE @ `0x43fedf`: XZON &= 0xf0 even when helper noops.
    assert_eq!(
        state.city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_CORNER_ALL
    );
}

#[test]
fn map_tool_place_water_ors_xbit_water_like_pe_446a58() {
    // PE helper 0x446650 @ 0x446a58: successful PlaceWater ORs XBIT bit 0x04.
    let mut city = City::new_blank("/tmp/sc2000-water-or-xbit.sc2", "WaterOrXbit").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    let bits = state.city.tile_bits_at(20, 20).expect("bits");
    assert!(bits.water_tile(), "PE ORs XBIT 0x04 on place");
    assert!(
        bits.piped() && bits.watered(),
        "PE OR 0x04 keeps piped/watered"
    );
    // Same tile refuses a second PlaceWater via existing XBIT/terrain gates.
    assert_eq!(state.place_surface_water_at(20, 20), 0);
    // Tree tool also refuses the stamped water bit (PE @ 0x43fb73).
    assert_eq!(state.place_tree_at_with_shift(20, 20, false), 0);
}

#[test]
fn map_tool_place_water_sets_altm_water_equal_land_like_pe_446a26() {
    // PE helper 0x446650 @ 0x446a08/0x446a26: ALTM water field := land altitude.
    let mut city = City::new_blank("/tmp/sc2000-water-altm.sc2", "WaterAltm").expect("blank");
    city.set_city_funds(20_000);
    city.set_altitude(20, 20, 8, 0);
    city.set_altitude(21, 20, 0, 3);
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.altitude_at(20, 20), 8);
    assert_eq!(state.city.water_height_at(20, 20), 8);
    assert_eq!(state.place_surface_water_at(21, 20), 1);
    assert_eq!(state.city.altitude_at(21, 20), 0);
    assert_eq!(state.city.water_height_at(21, 20), 0);
}

#[test]
fn map_tool_place_water_preserves_xtrf_like_pe_43fe30() {
    // PE MapToolPlaceWater (`0x43fe30`) / helper never write XTRF (`0x4ca940`).
    let mut city = City::new_blank("/tmp/sc2000-water-xtrf.sc2", "WaterXtrf").expect("blank");
    city.set_city_funds(20_000);
    city.set_traffic_density(20, 20, 55);
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(55));
}

#[test]
fn mac68k_map_tool_place_water_preserves_xtxt_like_pe_place_tile() {
    // Shared PlaceWater → PlaceTile clear leaves XTXT; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-water-xtxt.sc2", "MacWaterT").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 3); // rubble — accepted by PE @ 0x43fe96 gate
    city.set_text_overlay(20, 20, 0x20);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0x20));
}

#[test]
fn mac68k_map_tool_place_water_preserves_xtrf_like_pe_43fe30() {
    // Shared PlaceWater helper leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-water-xtrf.sc2", "MacWaterX").expect("blank");
    city.set_city_funds(20_000);
    city.set_traffic_density(20, 20, 55);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.traffic_density_at(20, 20), Some(55));
}

#[test]
fn map_tool_place_water_preserves_watered_like_pe_446a58() {
    // PE helper @ 0x446a58: `or XBIT, 4` only — does not clear watered/piped.
    let mut city = City::new_blank("/tmp/sc2000-water-watered.sc2", "WaterWatered").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    let bits = state.city.tile_bits_at(20, 20).expect("bits");
    assert!(bits.water_tile());
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn map_tool_place_water_xbld_gate_matches_pe_0x43fe96() {
    // PE @ 0x43fe96: XBLD < 0x0e except radioactivity 5.
    let mut city = City::new_blank("/tmp/sc2000-water-xbld.sc2", "WaterXbld").expect("blank");
    city.set_city_funds(20_000);
    city.set_building(20, 20, 3); // rubble
    city.set_building(21, 20, 13); // small park
    city.set_building(22, 20, 5); // radioactivity
    city.set_building(23, 20, 0x0e); // min refuse
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.terrain_at(20, 20), 0x3d);
    assert_eq!(state.place_surface_water_at(21, 20), 1);
    // PE mask W-only on new tile; recurse re-slopes west to E-mask 0x42.
    assert_eq!(state.city.terrain_at(21, 20), 0x44);
    assert_eq!(state.city.terrain_at(20, 20), 0x42);
    assert_eq!(state.place_surface_water_at(22, 20), 0);
    assert_eq!(state.city.building_at(22, 20), 5);
    assert_eq!(state.place_surface_water_at(23, 20), 0);
    assert_eq!(state.city.building_at(23, 20), 0x0e);
}

#[test]
fn tree_tool_drag_paints_path_upgrades_existing_trees_and_charges_per_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);
    state.tool = EditTool::Trees;

    state.begin_nature_drag(10, 10, EditTool::Trees);
    assert_eq!(state.update_nature_drag_to((13, 10)), 3);
    assert_eq!(state.finish_nature_drag(), Some((EditTool::Trees, 4)));
    for x in 10..=13 {
        assert!(
            matches!(state.city.building_at(x, 10), 6 | 7),
            "PE MapToolPlaceTree selects XBLD 6 or 7 from CRT low bit"
        );
    }
    assert_eq!(state.city.city_funds(), 19_988);

    // PE MapToolPlaceTree does not treat an existing tree as already placed:
    // XBLD 6..10 is upgraded by one on each successful application.
    state.begin_nature_drag(10, 10, EditTool::Trees);
    assert_eq!(state.update_nature_drag_to((13, 10)), 3);
    assert_eq!(state.finish_nature_drag(), Some((EditTool::Trees, 4)));
    for x in 10..=13 {
        assert!(
            matches!(state.city.building_at(x, 10), 7 | 8),
            "PE MapToolPlaceTree upgrades existing tree XBLD one step"
        );
    }
    assert_eq!(state.city.city_funds(), 19_976);
}

#[test]
fn tree_tool_rejects_water_tile_and_large_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    // PE refuses water-tile bit — not bare water XTER without the bit.
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    city.set_building(21, 20, 6); // existing tree — place arm upgrades, not refuse
    city.set_building(22, 20, 112);
    let mut state = test_state(city);
    state.tool = EditTool::Trees;

    for tile in [(20, 20), (22, 20)] {
        let placement = state
            .tool_placement_at(tile.0, tile.1)
            .expect("tree placement");
        assert!(placement.valid_tiles.is_empty(), "{tile:?}");
        assert_eq!(placement.invalid_tiles, vec![tile], "{tile:?}");
        assert!(!state.apply_tool_at_tile(tile.0, tile.1), "{tile:?}");
    }
    let upgrade = state
        .tool_placement_at(21, 20)
        .expect("tree upgrade placement");
    assert_eq!(upgrade.valid_tiles, vec![(21, 20)]);
    assert!(state.apply_tool_at_tile(21, 20));
    assert!(state.city.building_at(21, 20) > 6);
}

#[test]
fn map_tool_place_tree_allows_water_xter_without_water_tile_bit() {
    // PE MapToolPlaceTree: no general water-XTER refuse; only XBIT 0x04 / 0x2E / 0x3E.
    for xter in [0x30_u8, 0x3d] {
        let mut city = City::new_blank("/tmp/sc2000-tree-xter.sc2", "TreeXter").expect("blank");
        city.set_city_funds(20_000);
        city.set_terrain(20, 20, xter);
        let mut state = test_state(city);
        assert_eq!(state.place_tree_at(20, 20), 1, "xter {xter:#x}");
        assert!(state.city.building_at(20, 20) >= 6);
        assert_eq!(state.city.terrain_at(20, 20), xter, "PlaceTile leaves XTER");
    }
}

#[test]
fn map_tool_place_forest_allows_bare_water_xter_without_water_tile() {
    // PE MapToolPlaceForest @ `0x43fd18`/`0x43fd3e`: water-tile + 0x2E/0x3E only.
    let mut city = City::new_blank("/tmp/sc2000-forest-xter30.sc2", "ForestXter").expect("blank");
    city.set_city_funds(20_000);
    for y in 17..=23 {
        for x in 17..=23 {
            city.set_terrain(x, y, 0x30);
        }
    }
    let mut state = test_state(city);
    let stamped = state.place_forest_at_with_shift(20, 20, false);
    assert!(stamped > 0, "forest should stamp on bare XTER 0x30");
    assert!(
        (17..=23).any(|y| (17..=23).any(|x| state.city.building_at(x, y) >= 6)),
        "expected tree XBLD in brush"
    );
}

#[test]
fn map_tool_place_tree_refuses_waterfall_xter_2e_3e() {
    // PE @ `0x43fbb4`/`0x43fbba`.
    for xter in [0x2e_u8, 0x3e] {
        let mut city = City::new_blank("/tmp/sc2000-tree-fall.sc2", "TreeFall").expect("blank");
        city.set_city_funds(20_000);
        city.set_terrain(20, 20, xter);
        let mut state = test_state(city);
        assert_eq!(state.place_tree_at(20, 20), 0, "xter {xter:#x}");
        assert_eq!(state.city.building_at(20, 20), 0);
    }
}

#[test]
fn water_tool_drag_paints_path_and_clears_trees() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(11, 10, 6);
    city.set_building(13, 10, 112);
    let mut state = test_state(city);
    state.tool = EditTool::Water;

    state.begin_nature_drag(10, 10, EditTool::Water);
    assert_eq!(state.update_nature_drag_to((13, 10)), 2);
    assert_eq!(state.finish_nature_drag(), Some((EditTool::Water, 3)));

    // After left→right drag with PE neighbor re-slope:
    // 10: E-only → 0x42; 11: W|E → 0x41; 12: W-only → 0x44.
    assert_eq!(state.city.terrain_at(10, 10), 0x42, "E-neighbor nibble");
    assert_eq!(state.city.terrain_at(11, 10), 0x41, "W|E-neighbor nibble");
    assert_eq!(state.city.terrain_at(12, 10), 0x44, "W-neighbor nibble");
    for x in 10..=12 {
        assert_eq!(state.city.building_at(x, 10), 0, "building at {x},10");
    }
    assert_eq!(state.city.terrain_at(13, 10), 0);
    assert_eq!(state.city.building_at(13, 10), 112);
    assert_eq!(state.city.city_funds(), 19_700);

    state.begin_nature_drag(10, 10, EditTool::Water);
    assert_eq!(state.update_nature_drag_to((12, 10)), 0);
    assert_eq!(state.finish_nature_drag(), Some((EditTool::Water, 0)));
    assert_eq!(state.city.city_funds(), 19_700);
}

#[test]
fn raise_lower_city_terrain_edit_tools_change_altitude_and_clear_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    set_flat_terrain_neighborhood_for_test(&mut city, 20, 20, 8);
    set_flat_terrain_neighborhood_for_test(&mut city, 30, 30, 5);
    city.set_building(30, 30, 112);
    let mut state = test_state(city);

    state.tool = EditTool::RaiseTerrain;
    let raise = state
        .tool_placement_at(20, 20)
        .expect("raise terrain placement");
    assert_eq!(raise.valid_tiles, vec![(20, 20)]);
    assert!(state.place_city_terrain_edit_at(20, 20));
    assert_eq!(state.city.altitude_at(20, 20), 9);

    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_edit_at(20, 20));
    assert_eq!(state.city.altitude_at(20, 20), 8);

    state.tool = EditTool::LevelTerrain;
    assert!(state.place_city_terrain_edit_at(20, 20));
    assert_eq!(state.city.altitude_at(20, 20), 8);

    state.tool = EditTool::RaiseTerrain;
    // PE FUN_00447020: building tiles are valid — DestroyBuilding then raise.
    let on_building = state
        .tool_placement_at(30, 30)
        .expect("raise on building placement");
    assert_eq!(on_building.valid_tiles, vec![(30, 30)]);
    assert!(state.place_city_terrain_edit_at(30, 30));
    // Flat neighbors isolate the structure-clear contract from slope reclassification.
    assert_eq!(state.city.altitude_at(30, 30), 6);
    assert_eq!(state.city.building_at(30, 30), 0);
}

#[test]
fn mac_edit_map_stretch_uses_native_slope_cleanup_and_zoom_units() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (zoom, backing) in [(0.5, 1.0), (1.0, 1.0), (2.0, 2.0)] {
            let mut city = City::new_blank("/tmp/sc2000-mac-stretch-native.sc2", "MacStretch")
                .expect("blank city");
            city.kind = kind;
            city.set_mac_selected_water_height(0);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, 0);
            city.set_building(64, 64, 0x70);
            city.set_text_overlay(64, 64, 49);
            city.set_label(49, "removed by actual XTXT");
            city.set_underground(64, 64, 0x23);
            city.set_traffic_density(64, 64, 63);
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.mode = AppMode::EditNewMap;
            state.tool = EditTool::StretchTerrain;
            state.zoom = zoom;
            state.viewport_pixel_scale = backing;
            let crt_seed = state.pe_process_crt.seed();
            let seed = 0x943a_1234;
            state.mac_process_rng = MacToolboxRng::new(seed);
            let mut expected_rng = state.mac_process_rng;
            for _ in 0..3 {
                expected_rng.random_u16();
            }
            let mouse_y = 200.0 - 24.0 * render_zoom(zoom, backing);

            assert!(state.begin_edit_map_stretch_drag_at(64, 64, 200.0));
            assert!(state.update_edit_map_stretch_drag_at(mouse_y));

            assert_eq!(
                state.city.altitude_at(64, 64),
                10,
                "{kind:?}, zoom={zoom}, backing={backing}"
            );
            for (x, y) in [(64, 63), (65, 64), (64, 65), (63, 64)] {
                assert_eq!(state.city.altitude_at(x, y), 9);
            }
            assert!(is_rubble_tile(state.city.building_at(64, 64)));
            assert_eq!(state.city.text_overlay_at(64, 64), Some(0));
            assert_eq!(state.city.label_at(49), Some(""));
            assert_eq!(state.city.underground_at(64, 64), 0);
            assert_eq!(state.city.traffic_density_at(64, 64), Some(63));
            assert_eq!(state.mac_process_rng.seed(), expected_rng.seed());
            assert_eq!(state.pe_process_crt.seed(), crt_seed);
            assert!(!state.update_edit_map_stretch_drag_at(mouse_y));
            assert_eq!(state.mac_process_rng.seed(), expected_rng.seed());
            assert_eq!(state.finish_edit_map_stretch_drag(), 1);
            assert_eq!(state.pe_stretch_hold_cursor_id, 0);
            assert_eq!(state.pe_stretch_hold_ui_refresh_count, 0);
        }
    }
}

#[test]
fn mac_edit_map_stretch_remembers_requested_height_after_contour_refill() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-stretch-last-target.sc2", "MacLast").expect("blank city");
    city.set_mac_selected_water_height(0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::StretchTerrain;
    state.zoom = 1.0;
    let seed = state.mac_process_rng.seed();

    assert!(state.begin_edit_map_stretch_drag_at(64, 64, 100.0));
    assert!(state.update_edit_map_stretch_drag_at(124.0));

    // The native special contour fills this isolated lowering request back
    // to 8. local_6 still holds requested height 6, so the same sample skips.
    assert_eq!(state.city.altitude_at(64, 64), 8);
    assert_eq!(state.edit_map_stretch_drag.unwrap().last_land, 6);
    assert!(!state.update_edit_map_stretch_drag_at(124.0));
    assert_eq!(state.mac_process_rng.seed(), seed);
    assert_eq!(state.finish_edit_map_stretch_drag(), 1);
}

#[test]
fn edit_new_map_bulldoze_uses_place_tile_clear() {
    // EditNewMap bulldoze clears XBLD via PlaceTile `0x441f00`(0); leave XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-emap-bulldoze-place.sc2", "EBullP").expect("blank");
    city.set_terrain(20, 20, 0x06);
    city.set_building(20, 20, 7);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    city.set_traffic_density(20, 20, 0x44);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::Bulldoze;
    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.terrain_at(20, 20), 0);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.zone_at(20, 20).map(|z| z.raw_bits()), Some(0));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x44));
}

#[test]
fn city_stretch_terrain_uses_pe_hill_step() {
    // City StretchTerrain: PE `0x443a50` hill step + funds (not Level alias).
    let mut city = City::new_blank("/tmp/sc2000-city-stretch.sc2", "CityStretch").expect("blank");
    city.set_city_funds(20_000);
    for y in 19..22 {
        for x in 19..22 {
            city.set_altitude(x, y, 7, 0);
        }
    }
    city.set_altitude(20, 20, 5, 0);
    let mut state = test_state(city);
    state.tool = EditTool::StretchTerrain;
    state.pe_process_crt = Win95CrtRand::new(17); // dx=0,dy=0
    let funds_before = state.city.city_funds();
    assert!(state.place_city_terrain_edit_at(20, 20));
    // Hill land++ then `0x447490` slope-0x32 second land++ when surrounded higher.
    assert_eq!(state.city.altitude_at(20, 20), 7);
    assert!(state.city.city_funds() < funds_before);
}

#[test]
fn edit_map_tree_and_forest_tools_shift_remove_tree_terrain() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_terrain(20, 20, 0x06);
    city.set_terrain(21, 20, 0x30);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::Trees;

    assert!(state.apply_tool_at_tile_with_shift(20, 20, true));
    assert_eq!(state.city.terrain_at(20, 20), 0);
    assert!(state.apply_tool_at_tile_with_shift(21, 20, true));
    assert_eq!(state.city.terrain_at(21, 20), 0x30);

    state.tool = EditTool::Forest;
    for y in 37usize..=43 {
        for x in 37usize..=43 {
            state.city.set_terrain(x, y, 0x06);
        }
    }
    state.city.set_terrain(40, 40, 0x30);
    state.city.set_terrain(36, 40, 0x07);

    assert!(state.apply_tool_at_tile_with_shift(40, 40, true));
    for y in 37usize..=43 {
        for x in 37usize..=43 {
            let dx = x.abs_diff(40);
            let dy = y.abs_diff(40);
            if dx * dx + dy * dy <= 10 && (x, y) != (40, 40) {
                assert_eq!(state.city.terrain_at(x, y), 0, "terrain at {x},{y}");
            }
        }
    }
    assert_eq!(state.city.terrain_at(40, 40), 0x30);
    assert_eq!(state.city.terrain_at(36, 40), 0x07);
}

#[test]
fn edit_map_paint_drag_fills_path_between_mouse_samples() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::Water;

    assert_eq!(
        state.paint_edit_map_tool_drag_to(None, (10, 10), false),
        (1, Some((10, 10)))
    );
    assert_eq!(
        state.paint_edit_map_tool_drag_to(Some((10, 10)), (13, 10), false),
        (3, Some((13, 10)))
    );
    for x in 10..=13 {
        assert_eq!(state.city.terrain_at(x, 10), 0x30, "terrain at {x},10");
    }

    state.tool = EditTool::Trees;
    for x in 20..=23 {
        state.city.set_terrain(x, 20, 0x06);
    }
    assert!(state.apply_tool_at_tile_with_shift(20, 20, true));
    assert_eq!(
        state.paint_edit_map_tool_drag_to(Some((20, 20)), (23, 20), true),
        (3, Some((23, 20)))
    );
    for x in 20..=23 {
        assert_eq!(state.city.terrain_at(x, 20), 0, "terrain at {x},20");
    }
}

#[test]
fn edit_map_generator_toggles_are_misc_backed_and_checked() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;

    assert!(state.ui_action_enabled(UiAction::ToggleEditCoast));
    assert!(state.ui_action_enabled(UiAction::ToggleEditRiver));
    assert!(
        !state
            .checked_palette_actions()
            .contains(&UiAction::ToggleEditCoast)
    );

    state.toggle_edit_map_option(EDIT_MAP_COAST_MISC_OFFSET);
    assert_eq!(
        state.city.misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert!(
        state
            .checked_palette_actions()
            .contains(&UiAction::ToggleEditCoast)
    );

    state.toggle_edit_map_option(EDIT_MAP_COAST_MISC_OFFSET);
    assert_eq!(
        state.city.misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert!(
        !state
            .checked_palette_actions()
            .contains(&UiAction::ToggleEditCoast)
    );

    state.toggle_edit_map_option(EDIT_MAP_RIVER_MISC_OFFSET);
    assert!(
        state
            .checked_palette_actions()
            .contains(&UiAction::ToggleEditRiver)
    );
    state.mode = AppMode::CityView;
    assert!(!state.ui_action_enabled(UiAction::GenerateEditMap));
    assert!(
        !state
            .checked_palette_actions()
            .contains(&UiAction::ToggleEditRiver)
    );
}

#[test]
fn mac_edit_map_generator_toggles_use_saved_native_option_bytes() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-edit-map-options.city", "Mac Map").expect("blank city");
    city.kind = CityFileKind::MacCity;
    // CODE 4 loads these flags from the low byte of MISC slots 913/914 and
    // saveback writes each zero-extended byte as a full dword.
    city.set_mac_state_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 0x1234_5600);
    city.set_mac_state_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, 0x1234_5601);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;

    assert!(!state.edit_map_option_enabled(EDIT_MAP_COAST_MISC_OFFSET));
    assert!(state.edit_map_option_enabled(EDIT_MAP_RIVER_MISC_OFFSET));
    state.toggle_edit_map_option(EDIT_MAP_COAST_MISC_OFFSET);
    state.toggle_edit_map_option(EDIT_MAP_RIVER_MISC_OFFSET);
    assert_eq!(
        state.city.mac_state_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET),
        Some(1)
    );
    assert_eq!(
        state.city.mac_state_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET),
        Some(0)
    );
}

#[test]
fn edit_map_slider_input_updates_state_and_status() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;

    state.set_edit_map_slider(ui::EditMapSlider::Water, ui::EDIT_MAP_SLIDER_MAX);
    assert_eq!(
        state.edit_map_sliders.value(ui::EditMapSlider::Water),
        ui::EDIT_MAP_SLIDER_MAX
    );
    assert_eq!(state.query_line, "100% Water");

    state.set_edit_map_slider(ui::EditMapSlider::Trees, ui::EDIT_MAP_SLIDER_MAX + 1);
    assert_eq!(
        state.edit_map_sliders.value(ui::EditMapSlider::Trees),
        ui::EDIT_MAP_SLIDER_MAX
    );
    assert_eq!(state.query_line, "100% Trees");
}

#[test]
fn edit_map_make_replaces_city_layers_and_uses_generator_options() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 112);
    city.set_underground(10, 10, 3);
    city.set_zone(10, 10, ZONE_COMMERCIAL_DENSE);
    city.set_text_overlay(10, 10, 7);
    city.set_tile_bits(
        10,
        10,
        crate::city::TileBits::new(0)
            .with_piped(true)
            .with_watered(true),
    );
    city.set_traffic_density(10, 10, 99);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state
        .city
        .set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    state
        .city
        .set_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    state
        .edit_map_sliders
        .set(ui::EditMapSlider::Hills, ui::EDIT_MAP_SLIDER_MAX);
    state
        .edit_map_sliders
        .set(ui::EditMapSlider::Water, ui::EDIT_MAP_SLIDER_MAX);
    state
        .edit_map_sliders
        .set(ui::EditMapSlider::Trees, ui::EDIT_MAP_SLIDER_MAX);

    state.generate_edit_map_with_seed(0x1234_5678);

    assert_eq!(state.city.building_at(10, 10), 0);
    assert_eq!(state.city.underground_at(10, 10), 0);
    assert_eq!(state.city.zone_at(10, 10).unwrap().raw_bits(), 0);
    assert_eq!(state.city.text_overlay_at(10, 10), Some(0));
    let generated_bits = state.city.tile_bits_at(10, 10).unwrap();
    assert!(!generated_bits.piped());
    assert!(!generated_bits.watered());
    assert_eq!(
        generated_bits.water_tile(),
        is_water_terrain(state.city.terrain_at(10, 10)),
        "map generation keeps the XBIT water-surface flag consistent with generated terrain"
    );
    assert_eq!(state.city.traffic_density_at(10, 10), Some(0));
    assert_eq!(state.query_line, "Map generated");

    let mut water_tiles = 0;
    let mut tree_tiles = 0;
    let mut changed_heights = 0;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let terrain = state.city.terrain_at(x, y);
            if is_water_terrain(terrain) {
                water_tiles += 1;
            }
            if (0x06..=0x0c).contains(&terrain) {
                tree_tiles += 1;
            }
            if state.city.altitude_at(x, y) != BLANK_MAP_BASE_HEIGHT {
                changed_heights += 1;
            }
        }
    }
    assert!(water_tiles > CITY_HEIGHT, "expected coast or river water");
    assert!(tree_tiles > 0, "expected tree slider to create trees");
    assert!(changed_heights > 0, "expected non-flat generated terrain");
}

#[test]
fn mac_make_map_persists_generated_water_height_in_packed_misc_slot_912() {
    let mut city = City::new_blank("/tmp/sc2000-mac-map-water.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(912 * 4, 0xabcd_0000);
    city.set_mac_state_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 0);
    city.set_mac_state_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, CITY_OPTION_ENABLED_VALUE);
    city.set_mac_state_misc_u32_at(0x01f0, 77);
    city.set_mac_state_misc_u32_at(0x01f0 + 6 * 4, 55);
    city.set_mac_state_misc_u32_at(0x0fa8, 33);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::EditNewMap;
    state.generate_edit_map_with_seed(0x1234_5678);

    assert_eq!(state.city.mac_selected_water_height(), Some(4));
    assert_eq!(state.city.mac_state_misc_u32_at(912 * 4), Some(4));
    let mut expected_xbld_counts = [0u16; 256];
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let building = usize::from(state.city.building_at(x, y));
            expected_xbld_counts[building] = expected_xbld_counts[building].wrapping_add(1);
        }
    }
    for (building, expected) in expected_xbld_counts.into_iter().enumerate() {
        assert_eq!(
            state.city.mac_xbld_tile_count(building as u8),
            expected,
            "Mac saved XBLD census for tile {building} must match the generated map"
        );
    }
    for class in 0..16 {
        assert_eq!(
            state.city.mac_zone7_census_count(class),
            0,
            "new map starts with an empty zone-7 census"
        );
    }

    let path =
        std::env::temp_dir().join(format!("sc2000-mac-map-water-{}.city", std::process::id()));
    state.city.save(&path).expect("save generated Mac city");
    let loaded = City::load(&path).expect("reload generated Mac city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.mac_selected_water_height(), Some(4));
    assert_eq!(loaded.mac_state_misc_u32_at(912 * 4), Some(4));
    for (building, expected) in expected_xbld_counts.into_iter().enumerate() {
        assert_eq!(
            loaded.mac_xbld_tile_count(building as u8),
            expected,
            "saved Mac XBLD census for tile {building} must survive reload"
        );
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn level_terrain_clears_surface_objects_without_dezoning() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_mac_selected_water_height(8);
    city.set_city_funds(20_000);
    city.set_building(20, 20, 112);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    // Keep the cases outside each other's DestroyBuilding/reclassifier footprints.
    city.set_building(28, 20, 6);
    city.set_zone(28, 20, ZONE_COMMERCIAL_LIGHT);
    city.set_building(
        36,
        20,
        network_building_for_mask(NetworkKind::Power, NETWORK_LEFT | NETWORK_RIGHT),
    );
    city.set_altitude(44, 20, 4, 5);
    city.set_terrain(44, 20, 0x30);

    let corner_bits = city.footprint_corner_bits();
    for (index, (x, y)) in building_footprint_from_origin(30, 30, 2)
        .expect("building footprint")
        .into_iter()
        .enumerate()
    {
        city.set_building(x, y, 141);
        city.set_zone(x, y, ZONE_INDUSTRIAL_LIGHT | corner_bits[index]);
    }

    let mut state = test_state(city);
    state.tool = EditTool::LevelTerrain;

    assert!(state.apply_tool_at_tile(20, 20));
    // `FUN_00447020` re-enters DestroyBuilding on the rubble and then its
    // gameplay-city-mode arm calls PlaceTile(0), so the final cell is empty.
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(
        state.city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT
    );

    assert!(state.apply_tool_at_tile(28, 20));
    assert_eq!(state.city.building_at(28, 20), 0);
    assert_eq!(
        state.city.zone_at(28, 20).unwrap().raw_bits(),
        ZONE_COMMERCIAL_LIGHT
    );

    assert!(state.apply_tool_at_tile(36, 20));
    assert_eq!(state.city.building_at(36, 20), 0);

    assert!(state.apply_tool_at_tile(44, 20));
    let water_sample = state.city.altm_at(44, 20);
    // Shared FUN_00447020 classifies the cleared low land as wet under sea level.
    assert_eq!(state.city.terrain_at(44, 20), 0x10);
    assert_eq!(water_sample.water, BLANK_MAP_BASE_HEIGHT);

    let placement = state.tool_placement_at(31, 31).expect("level preview");
    assert_eq!(placement.valid_tiles.len(), 4);
    assert!(placement.invalid_tiles.is_empty());
    assert!(state.apply_tool_at_tile(31, 31));
    for (index, (x, y)) in building_footprint_from_origin(30, 30, 2)
        .expect("building footprint")
        .into_iter()
        .enumerate()
    {
        assert_eq!(state.city.building_at(x, y), 0);
        // PE DestroyBuilding @ `0x45a0b9`: XZON &= 0x0f (keep type, clear corners).
        assert_eq!(
            state.city.zone_at(x, y).unwrap().raw_bits(),
            ZONE_INDUSTRIAL_LIGHT
        );
    }

    assert_eq!(state.city.city_funds(), 19_875);
}

#[test]
fn level_terrain_clears_xtxt_footprint() {
    // PE DestroyStructure / level terrain clears XTXT occupancy with XBLD.
    let mut city = City::new_blank("/tmp/sc2000-level-xtxt.sc2", "LevelXtxt").expect("blank");
    let building = RESIDENTIAL_2X2_BUILDINGS[4];
    stamp_complete_building_for_test(&mut city, 20, 20, building, 2);
    city.set_text_overlay(20, 20, 0x55);
    city.set_text_overlay(21, 21, 0x66);
    let mut state = test_state(city);
    state.clear_surface_for_level_terrain_at(20, 20);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert!(is_rubble_tile(state.city.building_at(21, 21)));
    assert_eq!(state.city.text_overlay_at(20, 20), Some(0));
    assert_eq!(state.city.text_overlay_at(21, 21), Some(0));
}

#[test]
fn level_terrain_destroy_places_crt_rubble_like_pe() {
    let mut city = City::new_blank("/tmp/sc2000-level-crt.sc2", "LevelCrt").expect("blank");
    city.set_building(20, 20, 0x70);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let expected = expected_single_tile_mode1_rubble_after_generic_scatter(&mut preview);
    state.clear_surface_for_level_terrain_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), expected);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn level_terrain_preserves_xtrf_like_pe_401604() {
    // PE DestroyStructure (`0x401604` → `0x461330`) never writes XTRF (`0x4ca940`).
    let mut city = City::new_blank("/tmp/sc2000-level-xtrf.sc2", "LevelXtrf").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_traffic_density(20, 20, 71);
    let mut state = test_state(city);
    state.clear_surface_for_level_terrain_at(20, 20);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(71));
}

#[test]
fn mac68k_level_terrain_preserves_xtrf_like_pe_401604() {
    // Shared DestroyStructure leaves XTRF; Mac68k must not invent a clear.
    let mut city = City::new_blank("/tmp/sc2000-mac-level-xtrf.sc2", "MacLvlX").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_traffic_density(20, 20, 71);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.clear_surface_for_level_terrain_at(20, 20);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    assert_eq!(state.city.traffic_density_at(20, 20), Some(71));
}

#[test]
fn level_terrain_preserves_watered_like_pe_401604() {
    // PE DestroyStructure only tests XBIT bit 0x04; does not clear watered/piped.
    let mut city = City::new_blank("/tmp/sc2000-level-watered.sc2", "LevelWatered").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    state.clear_surface_for_level_terrain_at(20, 20);
    assert!(is_rubble_tile(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn level_terrain_clears_highway_2x2_pe_destroy_footprint() {
    // PE DestroyStructure size table: highway 0x49 is 2×2 (`0x45b1c0`).
    let mut city = City::new_blank("/tmp/sc2000-level-hwy.sc2", "LevelHwy").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_building(x, y, 0x49);
        city.set_text_overlay(x, y, 0x55);
        city.set_tile_bits(x, y, TileBits::new(0).with_piped(true).with_watered(true));
        city.set_traffic_density(x, y, 0x1a);
    }
    let mut state = test_state(city);
    state.clear_surface_for_level_terrain_at(21, 21);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert!(
            is_rubble_tile(state.city.building_at(x, y)),
            "highway cell ({x},{y})"
        );
        assert_eq!(state.city.text_overlay_at(x, y), Some(0));
        let bits = state.city.tile_bits_at(x, y).unwrap();
        assert!(bits.piped() && bits.watered());
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x1a));
    }
}

#[test]
fn mac68k_level_terrain_clears_highway_2x2_preserves_xtrf() {
    let mut city = City::new_blank("/tmp/sc2000-mac-level-hwy.sc2", "MacLvlHwy").expect("blank");
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        city.set_building(x, y, 0x49);
        city.set_traffic_density(x, y, 0x1a);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.clear_surface_for_level_terrain_at(21, 21);
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert!(is_rubble_tile(state.city.building_at(x, y)));
        assert_eq!(state.city.traffic_density_at(x, y), Some(0x1a));
    }
}

#[test]
fn level_terrain_drag_uses_first_tile_altitude() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    // Local plateau so PE `0x445b50` after level does not cascade from blank-map height 8.
    for y in 8..13 {
        for x in 8..15 {
            city.set_altitude(x, y, 4, 0);
        }
    }
    city.set_altitude(11, 10, 9, 0);
    city.set_altitude(12, 10, 8, 0);
    city.set_building(12, 10, 112);
    city.set_zone(12, 10, ZONE_RESIDENTIAL_LIGHT);
    let mut state = test_state(city);
    state.tool = EditTool::LevelTerrain;

    state.begin_terrain_drag(10, 10);
    assert_eq!(state.city.altitude_at(10, 10), 4);
    assert_eq!(state.city.city_funds(), 19_975);

    assert_eq!(state.update_terrain_drag_to((12, 10)), 2);
    for x in 10..=12 {
        assert_eq!(state.city.altitude_at(x, 10), 4, "altitude at {x},10");
    }
    // The preceding tile's 3×3 reclassifier destroys this building to rubble;
    // the later drag step then clears that low rubble XBLD through PlaceTile(0).
    assert_eq!(state.city.building_at(12, 10), 0);
    assert_eq!(
        state.city.zone_at(12, 10).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT
    );
    assert_eq!(state.city.city_funds(), 19_925);

    state.cancel_terrain_drag();
    assert_eq!(state.update_terrain_drag_to((13, 10)), 0);
}

#[test]
fn raise_and_lower_terrain_drag_step_each_path_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    for y in 9..=21 {
        for x in 9..=23 {
            let land = if y < 15 { 4 } else { 8 };
            city.set_altitude(x, y, land, 0);
        }
    }
    city.set_building(22, 20, 112);
    let mut state = test_state(city);

    state.tool = EditTool::RaiseTerrain;
    state.begin_terrain_drag(10, 10);
    assert_eq!(state.update_terrain_drag_to((12, 10)), 2);
    assert_eq!(state.finish_terrain_drag(), 3);
    assert_eq!(state.city.altitude_at(10, 10), 5);
    assert_eq!(state.city.altitude_at(11, 10), 5);
    assert_eq!(state.city.altitude_at(12, 10), 5);
    assert_eq!(state.city.city_funds(), 19_925);

    state.tool = EditTool::LowerTerrain;
    state.begin_terrain_drag(20, 20);
    assert_eq!(state.update_terrain_drag_to((22, 20)), 2);
    assert_eq!(state.finish_terrain_drag(), 3);
    assert_eq!(state.city.altitude_at(20, 20), 8);
    // The 0x32 wet-slope branch's lower-then-land++ leaves these at 8 as well.
    assert_eq!(state.city.altitude_at(21, 20), 8);
    assert_eq!(state.city.altitude_at(22, 20), 8);
    assert_eq!(state.city.city_funds(), 19_850);
}

#[test]
fn city_view_terrain_edit_tools_debit_per_successful_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_altitude(20, 20, 5, 0);
    city.set_altitude(21, 20, 9, 0);
    city.set_altitude(19, 20, 7, 0);
    city.set_altitude(20, 19, 8, 0);
    city.set_altitude(20, 21, 6, 0);
    city.set_building(30, 30, 112);
    let mut state = test_state(city);

    state.tool = EditTool::RaiseTerrain;
    assert!(state.place_city_terrain_edit_at(20, 20));
    assert_eq!(state.city.city_funds(), 19_975);

    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_edit_at(20, 20));
    assert_eq!(state.city.city_funds(), 19_950);

    state.tool = EditTool::LevelTerrain;
    assert!(state.place_city_terrain_edit_at(20, 20));
    assert_eq!(state.city.city_funds(), 19_925);

    state.tool = EditTool::RaiseTerrain;
    // PE FUN_00447020: no vacant-only gate — DestroyBuilding for XBLD >= 0x0D then raise.
    assert!(state.place_city_terrain_edit_at(30, 30));
    assert_eq!(state.city.building_at(30, 30), 0);
    assert_eq!(state.city.city_funds(), 19_900);
}

#[test]
fn tunnel_tool_preserves_watered_without_invent_refresh() {
    let mut city = tunnel_test_city();
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_tile_bits(17, 20, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    state.tool = EditTool::Tunnel;
    assert_eq!(state.place_tunnel_at(20, 20), 4);
    assert!(state.city.tile_bits_at(20, 20).unwrap().piped());
    assert!(state.city.tile_bits_at(20, 20).unwrap().watered());
    assert!(state.city.tile_bits_at(17, 20).unwrap().piped());
    assert!(state.city.tile_bits_at(17, 20).unwrap().watered());
}

#[test]
fn tunnel_tool_requires_clear_hillside_and_matching_exit_slope() {
    let mut state = test_state(tunnel_test_city());
    state.city.set_building(20, 20, 5);
    assert!(state.tunnel_plan_at(20, 20).is_none());

    state.city.set_building(20, 20, 6);
    assert!(state.tunnel_plan_at(20, 20).is_some());

    state.city.set_terrain(17, 20, 4);
    assert!(state.tunnel_plan_at(20, 20).is_none());
}

#[test]
fn zoning_allows_trees_without_clearing_them() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(10, 10, 6);
    city.set_building(11, 10, 12);
    let mut state = test_state(city);

    let placement = state.zone_placement_for_rect((10, 10), (11, 10), 1);
    assert_eq!(placement.valid_tiles, vec![(10, 10), (11, 10)]);
    assert!(placement.invalid_tiles.is_empty());

    assert_eq!(state.place_zone_rect((10, 10), (11, 10), 1), 2);
    assert_eq!(state.city.building_at(10, 10), 6);
    assert_eq!(state.city.building_at(11, 10), 12);
    assert_eq!(
        state.city.zone_at(10, 10).unwrap().raw_bits(),
        1 | ZONE_CORNER_ALL
    );
    assert_eq!(
        state.city.zone_at(11, 10).unwrap().raw_bits(),
        1 | ZONE_CORNER_ALL
    );
}

#[test]
fn place_zone_and_dezone_preserve_watered_and_xtrf() {
    // PE zone tool: XZON write only — no invent water refresh / XTRF write.
    let mut city = City::new_blank("/tmp/sc2000-zone-xtrf.sc2", "ZoneX").expect("blank");
    city.set_city_funds(20_000);
    city.set_tile_bits(30, 30, TileBits::new(0).with_piped(true).with_watered(true));
    city.set_traffic_density(30, 30, 0x39);
    let mut state = test_state(city);
    assert_eq!(state.place_zone_at(30, 30, ZONE_RESIDENTIAL_LIGHT), 1);
    let bits = state.city.tile_bits_at(30, 30).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(30, 30), Some(0x39));
    assert_eq!(state.place_zone_at(30, 30, 0), 1);
    let bits = state.city.tile_bits_at(30, 30).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(30, 30), Some(0x39));
}

#[test]
fn mac_xval_low_xter_classes_add_twelve_to_primary_grid_only() {
    // Mac FUN_00027AB4 @ 0x27eb4..0x27ed6 reads the XTER row table:
    // XTER 1..=15 contributes 0x0c; XTXT is an independent plane.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-xval-low-xter.sc2", "MacXvalXter").expect("blank city");
    city.kind = CityFileKind::MacCity;

    let grid_index = 2 * 32 + 2; // the 2×2 source tile folds into coarse cell (2, 2)
    for xter in 0..=16 {
        let (before_primary, before_industrial) = mac_xval_terrain_grids(&city);
        city.set_terrain(9, 10, xter);
        city.set_text_overlay(9, 10, 1);
        let (after_primary, after_industrial) = mac_xval_terrain_grids(&city);
        let expected_delta = if (1..=15).contains(&xter) { 0x0c } else { 0 };

        assert_eq!(
            after_primary[grid_index].wrapping_sub(before_primary[grid_index]),
            expected_delta,
            "primary-grid delta for XTER {xter:#04x}"
        );
        assert_eq!(
            after_industrial[grid_index], before_industrial[grid_index],
            "XTER {xter:#04x} must not affect the industrial grid"
        );
        city.set_terrain(9, 10, 0);
    }

    let (baseline_primary, baseline_industrial) = mac_xval_terrain_grids(&city);
    for xtxt in 0..=16 {
        city.set_text_overlay(9, 10, xtxt);
        let (primary, industrial) = mac_xval_terrain_grids(&city);
        assert_eq!(
            primary[grid_index], baseline_primary[grid_index],
            "XTXT {xtxt:#04x}"
        );
        assert_eq!(
            industrial[grid_index], baseline_industrial[grid_index],
            "XTXT {xtxt:#04x}"
        );
    }
}

#[test]
fn decline_change_building_preserves_watered_and_xtrf() {
    // PE ChangeBuilding → ChooseAndPlace: OR XBIT 0xe0 only; never write XTRF.
    let mut city = City::new_blank("/tmp/sc2000-decline-xtrf.sc2", "DeclineX").expect("blank");
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
    state.bypass_growth_trip_gate = false;

    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert!(is_abandoned_building(state.city.building_at(20, 20)));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
    assert_eq!(state.city.traffic_density_at(20, 20), Some(0x71));
}

#[test]
fn development_footprint_flat_land_rejects_slopes_and_uneven_altitudes() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let footprint = building_footprint_from_origin(20, 20, 3).unwrap();
    for &(x, y) in &footprint {
        city.set_altitude(x, y, 5, 0);
    }
    let mut state = test_state(city);
    assert!(state.development_footprint_is_flat_land(&footprint));

    state.city.set_altitude(21, 21, 6, 0);
    assert!(!state.development_footprint_is_flat_land(&footprint));

    state.city.set_altitude(21, 21, 5, 0);
    state.city.set_terrain(21, 21, 0x01);
    assert!(!state.development_footprint_is_flat_land(&footprint));
}

#[test]
fn powered_zoned_tree_footprints_are_powered_for_development() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(21, 21, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, 6);
    }
    power_adjacent_building_for_test(&mut city, 21, 20);

    let maps = power_service_maps(&city);
    for (x, y) in building_footprint_from_origin(21, 21, 3).unwrap() {
        assert!(
            maps.powered_tiles[y * CITY_WIDTH + x],
            "zoned footprint tile ({x},{y}) should be powered"
        );
    }
}

#[test]
fn bulldoze_preserves_xbit_water_tile_through_0x3d_mask_like_pe() {
    // PE DestroyBuilding @ `0x45a07f`: XBIT &= 0x3d keeps water-tile bit 0x04.
    let mut city = City::new_blank("/tmp/sc2000-bull-xbit-wt.sc2", "BullXbitWt").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0)
            .with_water_tile(true)
            .with_powerable(true)
            .with_powered(true),
    );
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.water_tile());
    assert!(!bits.powerable());
    assert!(!bits.powered());
}

#[test]
fn mac68k_bulldoze_preserves_xbit_water_tile_through_0x3d_mask_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xbit-wt-mac.sc2", "BullXbitWtMac").expect("blank");
    city.set_building(20, 20, 0x71);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0).with_water_tile(true).with_powerable(true),
    );
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.water_tile());
    assert!(!bits.powerable());
}

#[test]
fn bulldoze_elevated_road_clears_altm_tunnel_bits_like_pe_destroy_building() {
    // PE DestroyBuilding elevated `0x3f..=0x42` @ `0x4599aa`: ALTM &= 0x3ff.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-altm.sc2", "BullElevAltm").expect("blank");
    city.set_building(20, 20, 0x3f);
    city.set_altitude(20, 20, 9, 4);
    city.set_tunnel_levels(20, 20, 0x1f);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.altitude_at(20, 20), 9);
    assert_eq!(state.city.water_height_at(20, 20), 4);
    assert_eq!(state.city.tunnel_levels_at(20, 20), 0);
    assert_eq!(state.city.altm_at(20, 20).tunnel_levels, 0);
}

#[test]
fn bulldoze_orphan_tunnel_portal_leaves_other_altm_words() {
    let mut city = flat_win95_terrain_fixture(CityFileKind::WinSc2);
    city.set_city_funds(100);
    city.set_building(20, 20, 0x3f);
    city.set_tunnel_levels(20, 20, 1);
    city.set_tunnel_levels(19, 20, 0x0a);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(1);

    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.tunnel_levels_at(20, 20), 0);
    assert_eq!(state.city.tunnel_levels_at(19, 20), 0x0a);
    let mut expected = Win95CrtRand::new(1);
    expected.random_u15();
    expected.random_u15();
    assert_eq!(state.pe_process_crt.seed(), expected.seed());
}

#[test]
fn win95_destroy_building_tunnel_matches_all_native_portal_directions() {
    const EMPTY: u64 = 0x9c1b_da7f_8c87_2325;
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let directions = [
            (0x3f, 0x41, -1, 0),
            (0x40, 0x42, 0, -1),
            (0x41, 0x3f, 1, 0),
            (0x42, 0x40, 0, 1),
        ];
        for (entry, exit, dx, dy) in directions {
            for flag in [false, true] {
                let mut city = flat_win95_terrain_fixture(kind);
                let end = offset_city_tile(20, 20, dx * 3, dy * 3).unwrap();
                city.set_building(20, 20, entry);
                city.set_building(end.0, end.1, exit);
                for step in 0..4 {
                    let (x, y) = offset_city_tile(20, 20, dx * step, dy * step).unwrap();
                    city.set_tunnel_levels(x, y, 1);
                }
                let mut crt = Win95CrtRand::new(1);
                if flag {
                    pe_destroy_building_mode1_optional_crt(&mut city, Some(&mut crt), 20, 20);
                } else {
                    pe_destroy_building_mode0_optional_crt(&mut city, Some(&mut crt), 20, 20);
                }
                assert_eq!(crt.seed(), 3_884_216_597);
                assert_eq!(
                    win95_terrain_plane_fingerprints(&city),
                    [0xf473_a285_a960_2325, EMPTY, EMPTY, EMPTY],
                    "{kind:?}, entry={entry:#x}, flag={flag}"
                );
            }
        }
    }
}

#[test]
fn win95_player_underground_bulldozer_matches_native_tunnel_replay() {
    const EMPTY: u64 = 0x9c1b_da7f_8c87_2325;
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = flat_win95_terrain_fixture(kind);
        city.set_city_funds(100);
        city.set_building(20, 20, 0x3f);
        city.set_building(17, 20, 0x41);
        for x in 17..=20 {
            city.set_tunnel_levels(x, 20, 1);
        }
        city.set_underground(20, 20, 0x23);
        city.pe_xund_occupancy = 1;
        city.set_tile_bits(20, 20, TileBits::new(0x60));
        city.set_tile_bits(17, 20, TileBits::new(0x60));
        let mut state = test_state(city);
        state.tool = EditTool::Bulldoze;
        state.show_underground = true;
        state.pe_process_crt = Win95CrtRand::new(1);

        assert!(state.apply_tool_at_tile(20, 20));
        assert_eq!(state.city.city_funds(), 99);
        assert_eq!(state.pe_process_crt.seed(), 3_884_216_597);
        assert_eq!(state.city.pe_xund_occupancy, 0);
        assert_eq!(
            win95_terrain_plane_fingerprints(&state.city),
            [0xf473_a285_a960_2325, EMPTY, EMPTY, 0x1d86_e6c2_dfb4_dd05],
            "{kind:?}"
        );
        for plane in [
            pe_density_plane_fingerprint(1, |x, y| state.city.underground_at(x, y)),
            pe_density_plane_fingerprint(1, |x, y| state.city.zone_at(x, y).unwrap().raw_bits()),
            pe_density_plane_fingerprint(1, |x, y| state.city.text_overlay_at(x, y).unwrap_or(0)),
        ] {
            assert_eq!(plane, EMPTY);
        }
    }
}

#[test]
fn win95_player_bulldozer_matches_native_wet_and_pipe_replays() {
    const EMPTY: u64 = 0x9c1b_da7f_8c87_2325;
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        for (case, crt, expected, xund) in [
            (
                "wet-clear",
                1,
                [
                    0xc850_9cd5_3eba_0325,
                    0x90e1_6d2d_60a5_8015,
                    EMPTY,
                    0x594d_3f2b_018e_ba61,
                ],
                EMPTY,
            ),
            (
                "wet-building",
                3_357_800_067,
                [
                    0xc850_9cd5_3eba_0325,
                    0x9e28_6e1a_a986_ab91,
                    EMPTY,
                    0x04da_837f_177b_9b25,
                ],
                EMPTY,
            ),
            (
                "pipe",
                1,
                [
                    0xf473_a285_a960_2325,
                    EMPTY,
                    0xee90_bc6e_4545_48a8,
                    0x54f0_1fc0_eb55_2f65,
                ],
                0x0445_a4f1_373e_5b56,
            ),
            (
                "und-empty",
                1,
                [0xf473_a285_a960_2325, EMPTY, EMPTY, EMPTY],
                EMPTY,
            ),
        ] {
            let mut city = flat_win95_terrain_fixture(kind);
            city.set_city_funds(100);
            if case.starts_with("wet-") {
                city.set_mac_selected_water_height(9);
                for (x, terrain) in [(20, 0x30), (19, 0x20)] {
                    city.set_altitude(x, 20, 8, 0);
                    city.set_terrain(x, 20, terrain);
                    city.set_tile_bits(x, 20, TileBits::new(0x04));
                }
                if case == "wet-building" {
                    city.set_building(20, 20, 0x70);
                }
            } else if case == "pipe" {
                city.set_building(20, 20, 0x1d);
                city.set_underground(20, 20, 0x10);
                city.set_underground(21, 20, 0x01);
                city.set_underground(22, 20, 0x01);
                city.pe_xund_occupancy = 2;
                city.set_tile_bits(20, 20, TileBits::new(0x60));
            }
            let mut state = test_state(city);
            state.tool = EditTool::Bulldoze;
            state.show_underground = !case.starts_with("wet-");
            state.pe_process_crt = Win95CrtRand::new(1);
            let accepted = case != "und-empty";

            assert_eq!(
                state.apply_tool_at_tile(20, 20),
                accepted,
                "{kind:?} {case}"
            );
            assert_eq!(
                state.city.city_funds(),
                100 - i32::from(accepted),
                "{kind:?} {case}"
            );
            assert_eq!(state.pe_process_crt.seed(), crt, "{kind:?} {case}");
            assert_eq!(
                state.city.pe_xund_occupancy,
                if case == "pipe" { 2 } else { 0 }
            );
            assert_eq!(
                win95_terrain_plane_fingerprints(&state.city),
                expected,
                "{kind:?} {case}"
            );
            assert_eq!(
                pe_density_plane_fingerprint(1, |x, y| state.city.underground_at(x, y)),
                xund,
                "{kind:?} {case}"
            );
        }
    }
}

#[test]
fn mac68k_bulldoze_elevated_road_clears_altm_tunnel_bits_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-elev-altm-mac.sc2", "BullElevAltmMac").expect("blank");
    city.set_building(20, 20, 0x40);
    city.set_altitude(20, 20, 8, 3);
    city.set_tunnel_levels(20, 20, 0x15);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.altitude_at(20, 20), 8);
    assert_eq!(state.city.water_height_at(20, 20), 3);
    assert_eq!(state.city.tunnel_levels_at(20, 20), 0);
    assert_eq!(state.city.altm_at(20, 20).tunnel_levels, 0);
}

#[test]
fn bulldoze_non_elevated_leaves_altm_tunnel_bits() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-altm-leave.sc2", "BullAltmLeave").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_tunnel_levels(20, 20, 7);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.tunnel_levels_at(20, 20), 7);
}

#[test]
fn bulldoze_size1_nonflat_remaps_slope_and_clears_xund_like_fun_00447020() {
    // PE DestroyBuilding @ `0x45a1ee` → FUN_00447020 mode 0: XUND clear, slope XTER,
    // XZON &= 0xf0 when remapped slope ≠ 0.
    let mut city = City::new_blank("/tmp/sc2000-bull-slope.sc2", "BullSlope").expect("blank");
    city.set_building(20, 20, 0x0d); // size-1, < 0x70
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_terrain(20, 20, 1);
    city.set_altitude(20, 20, 9, 0);
    city.set_altitude(21, 20, 10, 0); // higher neighbor → nonzero slope mask
    city.set_underground(20, 20, 0x10);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0); // XTER≠0 → PlaceTile(0), not rubble
    assert_eq!(state.city.underground_at(20, 20), 0);
    let slope = pe_fun_00447020_slope_xter(&state.city, 20, 20);
    assert_ne!(slope, 0);
    assert_eq!(state.city.terrain_at(20, 20), slope);
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 0);
}

#[test]
fn mac68k_bulldoze_size1_nonflat_remaps_slope_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-slope-mac.sc2", "BullSlopeMac").expect("blank");
    city.set_building(20, 20, 0x0e);
    city.set_zone(20, 20, ZONE_COMMERCIAL_LIGHT | 0x40);
    city.set_terrain(20, 20, 2);
    city.set_altitude(20, 20, 9, 0);
    city.set_altitude(20, 21, 10, 0);
    city.set_underground(20, 20, 0x11);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.underground_at(20, 20), 0);
    let slope = pe_fun_00447020_slope_xter(&state.city, 20, 20);
    assert_ne!(slope, 0);
    assert_eq!(state.city.terrain_at(20, 20), slope);
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 0);
}

#[test]
fn bulldoze_size1_flat_skips_fun_00447020_slope_remap() {
    let mut city = City::new_blank("/tmp/sc2000-bull-flat.sc2", "BullFlat").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_terrain(20, 20, 0);
    city.set_underground(20, 20, 0x10);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.underground_at(20, 20), 0x10); // size-1 flat skips FUN path
    assert_eq!(
        state.city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT
    );
}

#[test]
fn pe_fun_00447020_slope32_dry_keeps_radioactivity_and_xter0_like_44730e() {
    // PE FUN_00447020 slope `0x32` dry @ `0x44730e`: land++, XTER := 0, no PlaceTile — XBLD 5 stays.
    let mut city = City::new_blank("/tmp/sc2000-fun-rad32-dry.sc2", "FunRad32Dry").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 10, 0);
        }
    }
    city.set_altitude(20, 20, 9, 0); // after land++ → 10; sea blank ≤ 10 → dry
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 20), 0x32);
    let sea = pe_selected_water_height_for_fun_00447020(&city);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, sea);
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(city.terrain_at(20, 20), 0);
    assert_eq!(city.altitude_at(20, 20), 10);
    assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn mac68k_pe_fun_00447020_slope32_dry_keeps_radioactivity_like_44730e() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-fun-rad32-dry.sc2", "MacFunRad32Dry").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 10, 0);
        }
    }
    city.set_altitude(20, 20, 9, 0);
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 2);
    let sea = pe_selected_water_height_for_fun_00447020(&city);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, sea);
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(city.terrain_at(20, 20), 0);
}

#[test]
fn pe_fun_00447020_dry_clears_water_keeps_radioactivity_like_4471dc() {
    // PE FUN_00447020 dry @ `0x4471dc`: sea ≤ land → XBIT &= 0xfb; XTER := slope; no PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-fun-rad-dry.sc2", "FunRadDry").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0); // one higher → nonzero slope ≠ 0x32
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    city.set_tile_bits(
        20,
        20,
        TileBits::new(0).with_water_tile(true).with_piped(true),
    );
    let slope = pe_fun_00447020_slope_xter(&city, 20, 20);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x32);
    // Sea at/below land so dry arm runs.
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 4);
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(city.terrain_at(20, 20), slope);
    let bits = city.tile_bits_at(20, 20).unwrap();
    assert!(!bits.water_tile());
    assert!(bits.piped(), "dry arm only clears water-tile bit");
}

#[test]
fn mac68k_pe_fun_00447020_dry_clears_water_keeps_radioactivity_like_4471dc() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-fun-rad-dry.sc2", "MacFunRadDry").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 2);
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    let slope = pe_fun_00447020_slope_xter(&city, 20, 20);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 4);
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(city.terrain_at(20, 20), slope);
    assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn city_load_hydrates_pe_w_city_rotation_from_misc_431430() {
    // PE MISC→BSS @ `0x431430`: word `[MISC+0x08]` → `[0x4c942c]`.
    let path = "/tmp/sc2000-rotation-hydrate.sc2";
    let mut city = City::new_blank(path, "RotHydrate").expect("blank");
    city.set_misc_u32_at(MISC_CITY_ROTATION_OFFSET, 0xab03);
    city.saved_rotation = 3;
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_rotation, 0xab03);
    assert_eq!(loaded.saved_rotation(), 3);
    let _ = std::fs::remove_file(path);
}

#[test]
fn city_save_persists_pe_w_city_rotation_to_misc_433a7f() {
    // PE save MISC mirror @ `0x433a7f`: word `[0x4c942c]` → `[MISC+0x08]`.
    let path = "/tmp/sc2000-rotation-persist.sc2";
    let mut city = City::new_blank(path, "RotPersist").expect("blank");
    city.pe_w_city_rotation = 0xab01;
    city.saved_rotation = 2;
    city.save(path).expect("save");
    // saved_rotation low bits merge into BSS before persist.
    assert_eq!(city.pe_w_city_rotation, 0xab02);
    assert_eq!(city.misc_u32_at(MISC_CITY_ROTATION_OFFSET), Some(0xab02));
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_city_rotation, 0xab02);
    assert_eq!(loaded.saved_rotation(), 2);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac68k_pe_fun_00447020_slope_zero_preserves_xzon_on_radioactivity_like_44716b() {
    let mut city = City::new_blank("/tmp/sc2000-mac-fun-rad-flat-xzon.sc2", "MacFunRadFlatXzon")
        .expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 9);
    let zone = city.zone_at(20, 20).unwrap();
    assert_eq!(zone.zone_id(), ZONE_RESIDENTIAL_LIGHT);
    assert_eq!(zone.raw_bits() & 0xf0, ZONE_CORNER_ALL);
}

#[test]
fn pe_destroy_building_size1_nonflat_skips_when_xter0_like_45a21b() {
    // PE DestroyBuilding @ `0x45a217`: XTER == 0 → skip FUN_00447020 (`0x45a1ee`).
    let mut city = City::new_blank("/tmp/sc2000-size1-xter0.sc2", "Size1Xter0").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_terrain(20, 20, 0);
    // Neighbors higher — if FUN ran, slope remap would write nonzero XTER.
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_underground(20, 20, 0x10);
    let mut crt = Win95CrtRand::new(1);
    pe_flood_destroy_tile_with_crt(&mut city, &mut crt, 20, 20);
    // XTER0 → CRT rubble PlaceTile; size1 FUN skipped → XUND left (FUN would clear it).
    assert_ne!(city.building_at(20, 20), 0x0d);
    assert_eq!(city.terrain_at(20, 20), 0);
    assert_eq!(city.underground_at(20, 20), 0x10);
}

#[test]
fn pe_fun_00447020_slope32_wet_keeps_radioactivity_xbld_5_like_4472ae() {
    // PE FUN_00447020 slope `0x32` wet PlaceTile(0) @ `0x447399`: still skip XBLD == 5.
    let mut city = City::new_blank("/tmp/sc2000-fun-rad32.sc2", "FunRad32").expect("blank");
    // All neighbors higher → mask 0x0f → XTER remap 0x32.
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 8, 0);
        }
    }
    city.set_altitude(20, 20, 4, 0);
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 20), 0x32);
    let sea = 9i16;
    let land_before = city.altitude_at(20, 20);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, sea);
    assert_eq!(city.building_at(20, 20), 5);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(city.water_height_at(20, 20), sea as u8);
    // slope 0x32 bumps land once before wet compare.
    assert_eq!(
        city.altitude_at(20, 20),
        land_before.saturating_add(1).min(0x1f)
    );
    assert_eq!(city.terrain_at(20, 20), 0x10);
}

#[test]
fn mac68k_pe_fun_00447020_slope32_wet_keeps_radioactivity_xbld_5_like_4472ae() {
    let mut city = City::new_blank("/tmp/sc2000-mac-fun-rad32.sc2", "MacFunRad32").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 8, 0);
        }
    }
    city.set_altitude(20, 20, 4, 0);
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 9);
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(city.terrain_at(20, 20), 0x10);
}

#[test]
fn pe_fun_00447020_deep_wet_keeps_radioactivity_xter_0x10_like_447248() {
    // PE FUN_00447020 @ `0x447248`: sea − land > 1 → XTER slope+0x10; PlaceTile skip if XBLD==5.
    let mut city = City::new_blank("/tmp/sc2000-fun-rad-deep.sc2", "FunRadDeep").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 20);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x32);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 9); // sea − land > 1
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(city.terrain_at(20, 20), slope.wrapping_add(0x10));
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(city.water_height_at(20, 20), 9);
}

#[test]
fn pe_fun_00447020_slope_zero_preserves_xzon_on_radioactivity_like_44716b() {
    // PE @ `0x44716b`: remapped slope == 0 → skip XZON &= 0xf0 (type+corners stay).
    let mut city =
        City::new_blank("/tmp/sc2000-fun-rad-flat-xzon.sc2", "FunRadFlatXzon").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 20), 0);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 9);
    assert_eq!(city.building_at(20, 20), 5);
    let zone = city.zone_at(20, 20).unwrap();
    assert_eq!(zone.zone_id(), ZONE_RESIDENTIAL_LIGHT);
    assert_eq!(zone.raw_bits() & 0xf0, ZONE_CORNER_ALL);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_fun_00447020_shallow_wet_keeps_radioactivity_xter_0x20_like_447248() {
    // PE FUN_00447020 @ `0x447248`: sea − land == 1 → XTER slope+0x20; PlaceTile skip if XBLD==5.
    let mut city =
        City::new_blank("/tmp/sc2000-fun-rad-shallow.sc2", "FunRadShallow").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 20);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x32);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 5); // sea = land+1
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(city.terrain_at(20, 20), slope.wrapping_add(0x20));
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(city.water_height_at(20, 20), 5);
}

#[test]
fn pe_fun_00447020_slope_nonzero_clears_xzon_type_on_radioactivity_like_447190() {
    // PE @ `0x447190`: remapped slope ≠ 0 → XZON &= 0xf0 (corners kept) even when XBLD==5.
    let mut city = City::new_blank("/tmp/sc2000-fun-rad-xzon.sc2", "FunRadXzon").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    assert_ne!(pe_fun_00447020_slope_xter(&city, 20, 20), 0);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 9);
    assert_eq!(city.building_at(20, 20), 5);
    let zone = city.zone_at(20, 20).unwrap();
    assert_eq!(zone.raw_bits() & 0xf0, ZONE_CORNER_ALL);
    assert_eq!(zone.zone_id(), 0);
}

#[test]
fn mac68k_pe_destroy_building_size1_nonflat_skips_when_xter0_like_45a21b() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-size1-xter0.sc2", "MacSize1Xter0").expect("blank");
    city.set_building(20, 20, 0x0e);
    city.set_terrain(20, 20, 0);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_underground(20, 20, 0x10);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let mut crt = Win95CrtRand::new(1);
    pe_flood_destroy_tile_with_crt(&mut state.city, &mut crt, 20, 20);
    assert_eq!(state.city.terrain_at(20, 20), 0);
    assert_eq!(state.city.underground_at(20, 20), 0x10);
}

#[test]
fn mac68k_place_city_terrain_lower_fun_sea_uses_saved_selected_word_like_4ca818() {
    let mut city = City::new_blank("/tmp/sc2000-mac-lower-sea.sc2", "MacLowerSea").expect("blank");
    city.set_city_funds(20_000);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((48 + dx) as usize, (48 + dy) as usize, 7, 0);
        }
    }
    city.set_altitude(48, 48, 8, 0);
    city.set_altitude(50, 50, 25, 20);
    city.set_mac_selected_water_height(9);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let pe_sea = pe_selected_water_height_for_fun_00447020(&state.city);
    state.tool = EditTool::LowerTerrain;
    assert!(state.place_city_terrain_step_at(48, 48, EditTool::LowerTerrain));
    assert!(state.city.tile_bits_at(48, 48).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(48, 48), pe_sea as u8);
}

#[test]
fn bulldoze_size1_nonflat_wet_sea_adds_xter_water_band() {
    // PE FUN_00447020 @ `0x4471f5`: sea > land → water-tile + XTER slope+0x10/0x20.
    // Flatten blank neighbors to land height so remap ≠ 0x32 (blank alts are sea 8).
    let mut city = City::new_blank("/tmp/sc2000-bull-wet.sc2", "BullWet").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_terrain(20, 20, 1);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 2, 0);
        }
    }
    city.set_altitude(21, 20, 3, 0); // one higher neighbor
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    assert_eq!(state.edit_map_sea_level(), BLANK_MAP_BASE_HEIGHT);
    state.bulldoze_at(20, 20);
    let slope = pe_fun_00447020_slope_xter(&state.city, 20, 20);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x32);
    // sea-land == 6 → slope+0x10
    assert_eq!(state.city.terrain_at(20, 20), slope.wrapping_add(0x10));
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(20, 20), BLANK_MAP_BASE_HEIGHT);
}

#[test]
fn mac68k_bulldoze_size1_nonflat_wet_sea_adds_xter_water_band() {
    let mut city = City::new_blank("/tmp/sc2000-bull-wet-mac.sc2", "BullWetMac").expect("blank");
    city.set_building(20, 20, 0x0e);
    city.set_terrain(20, 20, 2);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 3, 0);
        }
    }
    city.set_altitude(20, 21, 4, 0);
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.edit_map_sea_level(), BLANK_MAP_BASE_HEIGHT);
    state.bulldoze_at(20, 20);
    let slope = pe_fun_00447020_slope_xter(&state.city, 20, 20);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x32);
    // sea-land == 5 → slope+0x10
    assert_eq!(state.city.terrain_at(20, 20), slope.wrapping_add(0x10));
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(20, 20), BLANK_MAP_BASE_HEIGHT);
}

#[test]
fn bulldoze_size1_nonflat_shallow_wet_adds_xter_0x20_band() {
    // PE FUN_00447020 @ `0x447281`: sea-land == 1 → XTER slope+0x20.
    let mut city = City::new_blank("/tmp/sc2000-bull-shallow.sc2", "BullShallow").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_terrain(20, 20, 1);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 7, 0);
        }
    }
    city.set_altitude(21, 20, 8, 0); // sea 8 − land 7 == 1; one higher neighbor
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    assert_eq!(state.edit_map_sea_level(), BLANK_MAP_BASE_HEIGHT);
    state.bulldoze_at(20, 20);
    let slope = pe_fun_00447020_slope_xter(&state.city, 20, 20);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x32);
    assert_eq!(state.city.terrain_at(20, 20), slope.wrapping_add(0x20));
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(20, 20), BLANK_MAP_BASE_HEIGHT);
}

#[test]
fn mac68k_bulldoze_size1_nonflat_shallow_wet_adds_xter_0x20_band() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-shallow-mac.sc2", "BullShallowMac").expect("blank");
    city.set_building(20, 20, 0x0e);
    city.set_terrain(20, 20, 2);
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 7, 0);
        }
    }
    city.set_altitude(20, 21, 8, 0);
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert_eq!(state.edit_map_sea_level(), BLANK_MAP_BASE_HEIGHT);
    state.bulldoze_at(20, 20);
    let slope = pe_fun_00447020_slope_xter(&state.city, 20, 20);
    assert_ne!(slope, 0);
    assert_ne!(slope, 0x32);
    assert_eq!(state.city.terrain_at(20, 20), slope.wrapping_add(0x20));
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(20, 20), BLANK_MAP_BASE_HEIGHT);
}

#[test]
fn bulldoze_size1_nonflat_slope_0x32_forces_xter_zero() {
    // PE FUN_00447020 @ `0x4472ae`/`0x44730e`: slope 0x32 dry → ALTM land++ then XTER := 0.
    let mut city = City::new_blank("/tmp/sc2000-bull-slope32.sc2", "BullS32").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_terrain(20, 20, 1);
    city.set_altitude(20, 20, 9, 0); // dry vs sea 8; after land++ still dry
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
        let nx = (20 + dx) as usize;
        let ny = (20 + dy) as usize;
        city.set_altitude(nx, ny, 10, 0);
    }
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 20), 0x32);
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.altitude_at(20, 20), 10);
    assert_eq!(state.city.terrain_at(20, 20), 0);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn mac68k_bulldoze_size1_nonflat_slope_0x32_forces_xter_zero() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-slope32-mac.sc2", "BullS32Mac").expect("blank");
    city.set_building(20, 20, 0x0e);
    city.set_terrain(20, 20, 2);
    city.set_altitude(20, 20, 9, 0);
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
        city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 10, 0);
    }
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 20), 0x32);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.altitude_at(20, 20), 10);
    assert_eq!(state.city.terrain_at(20, 20), 0);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn bulldoze_size1_nonflat_wet_slope_0x32_writes_xter_0x10() {
    // PE FUN_00447020 @ `0x4472ae`/`0x447399`: slope 0x32 + sea > land → land++ then XTER 0x10.
    let mut city = City::new_blank("/tmp/sc2000-bull-wet32.sc2", "BullWet32").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_building(20, 20, 0x0d);
    city.set_terrain(20, 20, 1);
    city.set_altitude(20, 20, 2, 0);
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
        city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 3, 0);
    }
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 20), 0x32);
    let mut state = test_state(city);
    assert_eq!(state.edit_map_sea_level(), BLANK_MAP_BASE_HEIGHT);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.altitude_at(20, 20), 3);
    assert_eq!(state.city.terrain_at(20, 20), 0x10);
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(state.city.water_height_at(20, 20), BLANK_MAP_BASE_HEIGHT);
}

#[test]
fn mac68k_bulldoze_size1_nonflat_wet_slope_0x32_writes_xter_0x10() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-wet32-mac.sc2", "BullWet32Mac").expect("blank");
    city.set_mac_selected_water_height(8);
    city.set_building(20, 20, 0x0e);
    city.set_terrain(20, 20, 2);
    city.set_altitude(20, 20, 3, 0);
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
        city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
    }
    assert_eq!(pe_fun_00447020_slope_xter(&city, 20, 20), 0x32);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.altitude_at(20, 20), 4);
    assert_eq!(state.city.terrain_at(20, 20), 0x10);
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn bulldoze_xter0_places_crt_rubble_like_pe_destroy_building() {
    // Mode 1 first consumes its selector scatter draws, then
    // `0x45a01f` uses CRT `rand()&3+1` for XTER==0 rubble.
    let mut city = City::new_blank("/tmp/sc2000-bull-rubble.sc2", "BullRubble").expect("blank");
    city.set_building(20, 20, 0x70);
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    for _ in 0..pe_destroy_building_mode1_scatter_draw_count_for_hit(&state.city, 0x70, 20, 20) {
        let _ = preview.random_u15();
    }
    let expected = preview.rubble_tile_id();
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), expected);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn bulldoze_xbld_6_nonzero_xter_places_tile_zero_like_pe() {
    // PE DestroyBuilding core @ `0x45a01f`: XTER!=0 → PlaceTile(0), no rubble draw.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xbld6-xter.sc2", "BullXbld6Xter").expect("blank");
    city.set_building(20, 20, 6);
    city.set_terrain(20, 20, 0x01);
    let mut state = test_state(city);
    let mut expected_crt = state.pe_process_crt;
    for _ in 0..pe_destroy_building_mode1_scatter_draw_count_for_hit(&state.city, 6, 20, 20) {
        expected_crt.random_u16();
    }
    pe_destroy_building_mode1_optional_crt(
        &mut state.city,
        Some(&mut state.pe_process_crt),
        20,
        20,
    );
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.pe_process_crt, expected_crt);
}

#[test]
fn mac68k_bulldoze_xbld_6_nonzero_xter_places_tile_zero_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-xbld6-xter-mac.sc2", "BullXbld6XterMac").expect("blank");
    city.set_building(20, 20, 6);
    city.set_terrain(20, 20, 0x02);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let seed_before = state.pe_process_crt_seed();
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.pe_process_crt_seed(), seed_before);
}

#[test]
fn mac68k_bulldoze_xter0_places_rubble_after_quickdraw_render_draws() {
    let mut city =
        City::new_blank("/tmp/sc2000-bull-rubble-mac.sc2", "BullRubbleMac").expect("blank");
    city.set_building(20, 20, 0x70);
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let crt_seed = state.pe_process_crt.seed();
    let mut preview = state.mac_process_rng;
    preview.random_u16();
    preview.random_u16();
    let expected = (preview.random_u16() as u8 & 3) + 1;
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), expected);
    assert_eq!(state.mac_process_rng.seed(), preview.seed());
    assert_eq!(state.pe_process_crt_seed(), crt_seed);
}

#[test]
fn bulldoze_nonzero_xter_places_tile_zero_like_pe_destroy_building() {
    // PE DestroyBuilding @ `0x45a01f`: XTER!=0 → PlaceTile(0), after the
    // selector's mode-1 scatter draws.
    let mut city = City::new_blank("/tmp/sc2000-bull-xter.sc2", "BullXter").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_terrain(20, 20, 0x01);
    let mut state = test_state(city);
    let mut expected_crt = state.pe_process_crt;
    for _ in 0..pe_destroy_building_mode1_scatter_draw_count_for_hit(&state.city, 0x70, 20, 20) {
        let _ = expected_crt.random_u15();
    }
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.pe_process_crt, expected_crt);
}

#[test]
fn pe_destroy_structure_xter0_places_crt_rubble_like_pe() {
    // PE DestroyStructure → DestroyBuilding: size-1 generic selector consumes
    // two render draws before `0x45a01f` selects XTER0 CRT rubble.
    let mut city =
        City::new_blank("/tmp/sc2000-destroy-crt-rubble.sc2", "DestroyCrtRubble").expect("blank");
    city.set_building(20, 20, 0x70);
    assert_eq!(city.terrain_at(20, 20), 0);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    for _ in 0..crate::city::pe_destroy_building_mode1_render_scatter_draw_count(0x70) {
        let _ = preview.random_u15();
    }
    let expected = preview.rubble_tile_id();
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), expected);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn mac68k_pe_destroy_structure_xter0_places_crt_rubble_like_pe() {
    let mut city = City::new_blank(
        "/tmp/sc2000-destroy-crt-rubble-mac.sc2",
        "DestroyCrtRubbleMac",
    )
    .expect("blank");
    city.set_building(20, 20, 0x71);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let mut preview = state.pe_process_crt;
    // This test exercises the shared Rust DestroyStructure adapter, whose
    // current clearing body is the Win95 PE path. It does not confirm the
    // original Mac implementation.
    for _ in 0..crate::city::pe_destroy_building_mode1_render_scatter_draw_count(0x71) {
        let _ = preview.random_u15();
    }
    let expected = preview.rubble_tile_id();
    assert!(state.pe_destroy_structure_at(20, 20));
    assert_eq!(state.city.building_at(20, 20), expected);
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn bulldoze_preserves_watered_like_pe_401604() {
    // PE DestroyStructure only tests XBIT bit 0x04; does not clear watered/piped.
    let mut city =
        City::new_blank("/tmp/sc2000-bulldoze-watered.sc2", "BullWatered").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_tile_bits(20, 20, TileBits::new(0).with_piped(true).with_watered(true));
    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert!(matches!(state.city.building_at(20, 20), 1..=4));
    let bits = state.city.tile_bits_at(20, 20).unwrap();
    assert!(bits.piped());
    assert!(bits.watered());
}

#[test]
fn win95_bulldozer_accepts_empty_xter_3e_waterfall_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(1);
    city.set_terrain(20, 20, 0x3e);
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;

    assert!(state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.city_funds(), 0);
}

#[test]
fn bulldoze_clears_decorative_surface_water_without_changing_altm() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_altitude(10, 10, 5, 6);
    city.set_terrain(10, 10, 0x30);
    city.set_tile_bits(10, 10, TileBits::new(0x85));
    let mut state = test_state(city);

    state.bulldoze_at(10, 10);

    let sample = state.city.altm_at(10, 10);
    assert_eq!(state.city.terrain_at(10, 10), 0);
    assert_eq!(sample.land, 5);
    // The ordinary PE low-class path clears XTER and XBIT, not ALTM water.
    assert_eq!(sample.water, 6);
    assert_eq!(state.city.tile_bits_at(10, 10).unwrap().raw_bits(), 0x81);
    assert_eq!(state.city.city_funds(), 19_999);

    state.bulldoze_at(10, 10);
    assert_eq!(state.city.city_funds(), 19_999);
}

#[test]
fn bulldoze_preserves_natural_water_terrain() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_altitude(10, 10, 5, 6);
    city.set_terrain(10, 10, 0x20);
    let mut state = test_state(city);

    state.bulldoze_at(10, 10);

    let sample = state.city.altm_at(10, 10);
    assert_eq!(state.city.terrain_at(10, 10), 0x20);
    assert_eq!(sample.water, 6);
    assert_eq!(state.city.city_funds(), 20_000);
}

#[test]
fn animation_frame_keeps_cycling_after_u8_cast_threshold() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
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
        modifiers: Mod::empty(),
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
        anim_time: 42.4,
        traffic_redraw_epoch: 0,
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
        edit_map_height_hold: None,
        pe_raise_xter_ui_byte: 0,
        pe_raise_ui_tool_id: 0,
        pe_raise_ui_tile_x: 0,
        pe_raise_ui_tile_y: 0,
        pe_raise_ui_press_y_byte: 0,
        pe_raise_ui_byte6: 0,
        pe_raise_ui_byte7: 0,
        pe_raise_ui_byte8: 0,
        pe_raise_ui_byte9: 0,
        pe_spread_tool4_ui_timer: 0,
        pe_spread_prev_tile_x: 0,
        pe_spread_prev_tile_y: 0,
        pe_stretch_hold_ui_refresh_count: 0,
        pe_stretch_hold_cursor_id: 0,
        pe_stretch_hold_cursor_arg: 0,
        pe_stretch_hold_ui_wait_4012c1_count: 0,
        pe_lower_hold_ui_refresh_count: 0,
        pe_level_hold_ui_wait_4012c1_count: 0,
        pe_spread_hold_ui_refresh_count: 0,
        pe_spread_hold_cursor_id: 0,
        pe_spread_hold_cursor_arg: 0,
        pe_spread_ui_cursor_4519e0_count: 0,
        pe_spread_ui_cursor_4519e0_last_arg: 0,
        pe_spread_ui_wait_4012c1_count: 0,
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

    let before = state.animation_frame();
    state.advance_ambient_animation(1.0);
    let after = state.animation_frame();

    assert_ne!(before, after);
    assert_eq!(after, ((43.4_f32 * ANIMATION_FPS).floor() as u64 % 8) as u8);

    state.engine_fidelity = EngineFidelity::Mac68k;
    state.anim_time = 0.0;
    state.advance_ambient_animation(0.19);
    assert_eq!(state.animation_frame(), 0);
    state.advance_ambient_animation(0.02);
    assert_eq!(state.animation_frame(), 1);
    state.advance_ambient_animation(0.21);
    assert_eq!(state.animation_frame(), 2);
}

#[test]
fn sprite_depth_preserves_layer_order_within_cell() {
    let terrain = sprite_depth(40, 60, DEPTH_TERRAIN, 0);
    let road = sprite_depth(40, 60, DEPTH_ROAD, 0);
    let building = sprite_depth(40, 60, DEPTH_BUILDING, 0);

    assert!(terrain < road);
    assert!(road < building);
}

#[test]
fn pe_road_stroke_walk_preserves_saved_direction_and_hands_water_to_bridge_planner() {
    let mut city =
        City::new_blank("/tmp/sc2000-road-walk-gates.sc2", "RoadWalkGates").expect("blank city");
    flatten_road_walk_fixture(&mut city, 19, 23, 19, 22);
    city.set_building(21, 20, 0x1d);
    assert_eq!(
        pe_road_stroke_plan_449bb0(&city, (20, 20), (22, 21))
            .expect("walk")
            .path,
        vec![(20, 20), (21, 20), (22, 20), (22, 21)]
    );

    city.set_building(21, 20, 0);
    city.set_tile_bits(21, 20, TileBits::new(0).with_water_tile(true));
    let plan = pe_road_stroke_plan_449bb0(&city, (20, 20), (22, 21)).expect("walk");
    assert_eq!(plan.path, vec![(20, 20)]);
    assert_eq!(plan.water_bridge_candidate, Some((21, 20)));
    let walk = pe_road_stroke_walk_init_448bc0(&city, (20, 20), (22, 21)).expect("walk init");
    let step =
        pe_road_next_stroke_tile_448f50(&city, (20, 20), walk.target, walk.bounds, walk.direction);
    assert_eq!(step.direction, 1);
    assert_eq!(step.water_bridge_candidate, Some((21, 20)));
}

#[test]
fn pe_rail_water_stroke_uses_direct_and_live_walk_handoffs() {
    let bridge_city = || {
        let mut city = City::new_blank("/tmp/sc2000-rail-bridge-handoff.sc2", "RailBridgeHandoff")
            .expect("blank city");
        city.set_city_funds(10_000);
        for x in 10..=16 {
            city.set_terrain(x, 10, if x == 10 { 0x21 } else { 0x20 });
            city.set_altitude(x, 10, 5, 5);
            city.set_tile_bits(x, 10, TileBits::new(0).with_water_tile(true));
        }
        city.set_terrain(17, 10, 0x40);
        city.set_altitude(17, 10, 5, 5);
        city
    };

    let mut direct = test_state(bridge_city());
    direct.tool = EditTool::Rail;
    assert_eq!(
        direct.place_win95_rail_stroke_449f50((10, 10), (10, 10)),
        Some((0, None))
    );
    assert!(matches!(
        direct.city_info_dialog,
        Some(CityInfoDialog::RailBridge {
            seed_x: 10,
            seed_y: 10,
            direction: 1,
            span_count: 7,
            neighbor_link: None,
        })
    ));
    assert_eq!(direct.city.city_funds(), 10_000, "chooser defers debit");

    let mut handoff_city = bridge_city();
    handoff_city.set_terrain(9, 10, 0);
    handoff_city.set_altitude(9, 10, 5, 5);
    let mut handoff = test_state(handoff_city);
    handoff.tool = EditTool::Rail;
    assert_eq!(
        handoff.place_win95_rail_stroke_449f50((9, 10), (11, 10)),
        Some((1, None))
    );
    assert!(NetworkKind::Rail.contains(handoff.city.building_at(9, 10)));
    assert_eq!(
        handoff.city_info_dialog,
        Some(CityInfoDialog::RailBridge {
            seed_x: 10,
            seed_y: 10,
            direction: 1,
            span_count: 7,
            neighbor_link: None,
        })
    );
    assert_eq!(handoff.city.building_at(10, 10), 0);
}

#[test]
fn win95_rail_bridge_at_edge_defers_paid_link_until_bridge_choice() {
    for path in ["rail-edge-span.city", "rail-edge-span.sc2"] {
        for build_bridge in [false, true] {
            let mut city = blank_city_layout_fixture(path, "Rail edge bridge");
            city.set_city_funds(3000);
            for x in 0..=8 {
                city.set_altitude(x, 10, 5, 5);
                city.set_terrain(
                    x,
                    10,
                    match x {
                        0 | 8 => 0x40,
                        1 => 0x22,
                        _ => 0x20,
                    },
                );
                city.set_tile_bits(
                    x,
                    10,
                    TileBits::new(if x == 0 || x == 8 { 0x20 } else { 4 }),
                );
                city.set_traffic_density(x, 10, 0x63);
            }
            city.prepare_pe_fidelity_from_saved_city();
            let mut state = test_state(city);
            state.tool = EditTool::Rail;
            state.begin_network_drag(0, 10, NetworkKind::Rail);
            state.update_network_drag_to((2, 10));
            let release = state.release_left_mouse_drag(false);
            assert_eq!(release.network, Some((NetworkKind::Rail, 1)));
            assert_eq!(release.rail_neighbor_link, None);
            assert_eq!(state.city.city_funds(), 2975);
            assert_eq!(state.city.text_overlay_at(0, 10), Some(0));
            let Some(CityInfoDialog::RailBridge {
                seed_x: 1,
                seed_y: 10,
                direction: 1,
                span_count: 7,
                neighbor_link: Some(link),
            }) = state.city_info_dialog
            else {
                panic!("expected bridge with deferred edge link");
            };
            if build_bridge {
                assert_eq!(state.finish_rail_bridge((1, 10), 1, 7), 7);
            }
            state.close_city_info_dialog();
            assert!(apply_rail_neighbor_link_confirmation(
                &mut state, link, true
            ));
            assert_eq!(
                state.city.city_funds(),
                if build_bridge { 950 } else { 1475 }
            );
            assert_eq!(
                state.city.building_at(0, 10),
                if build_bridge { 0x3d } else { 0x2d }
            );
            assert_eq!(
                state.city.building_at(1, 10),
                if build_bridge { 0x30 } else { 0 }
            );
            assert_eq!(
                state.city.building_at(7, 10),
                if build_bridge { 0x2e } else { 0 }
            );
            assert_eq!(state.city.text_overlay_at(0, 10), Some(0xfa));
            assert_eq!(state.city.pe_nonnetwork_neighbor_links, 1);
            assert!((0..=8).all(|x| state.city.traffic_density_at(x, 10) == Some(0x63)));
        }
    }
}

#[test]
fn pe_road_dry_stroke_commits_land_before_opening_water_bridge_chooser() {
    let mut city = City::new_blank("/tmp/sc2000-road-bridge-handoff.sc2", "RoadBridgeHandoff")
        .expect("blank city");
    city.set_city_funds(10_000);
    city.set_terrain(9, 10, 0x40);
    city.set_altitude(9, 10, 5, 5);
    for x in 10..=16 {
        city.set_terrain(x, 10, if x == 10 { 0x22 } else { 0x20 });
        city.set_altitude(x, 10, 5, 5);
        city.set_tile_bits(x, 10, TileBits::new(0).with_water_tile(true));
    }
    city.set_terrain(17, 10, 0x40);
    city.set_altitude(17, 10, 5, 5);

    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.place_win95_road_stroke_449bb0((9, 10), (11, 10)).0, 1);
    assert!(NetworkKind::Road.contains(state.city.building_at(9, 10)));
    assert_eq!(state.city.city_funds(), 9_990);
    assert!(matches!(
        state.city_info_dialog,
        Some(CityInfoDialog::RoadBridge {
            seed_x: 10,
            seed_y: 10,
            direction: 1,
            span_count: 7,
            selector_mask: 0x1c,
            mac_neighbor_link: None,
        })
    ));
    assert_eq!(state.city.building_at(10, 10), 0);
    assert_eq!(state.finish_road_bridge((10, 10), 1, 7, 2), 7);
    assert_eq!(state.city.city_funds(), 9_815);
    assert_eq!(state.city.building_at(9, 10), 0x1e);
    assert_eq!(state.city.building_at(10, 10), 0x21);
    assert_eq!(state.city.building_at(16, 10), 0x1f);
    assert!((11..=15).all(|x| state.city.building_at(x, 10) == 0x57));
}

#[test]
fn pe_power_direct_water_stroke_uses_selector_zero_bridge_transaction() {
    let mut city =
        City::new_blank("/tmp/sc2000-power-bridge-pe.sc2", "PowerBridgePe").expect("blank city");
    city.set_city_funds(10_000);
    for x in 10..=16 {
        city.set_terrain(x, 10, if x == 10 { 0x21 } else { 0x20 });
        city.set_altitude(x, 10, 5, 5);
        city.set_tile_bits(x, 10, TileBits::new(0).with_water_tile(true));
        if x.is_multiple_of(2) {
            city.set_traffic_density(x, 10, 0x80 | x as u8);
        }
    }
    city.set_terrain(17, 10, 0x40);
    city.set_altitude(17, 10, 5, 5);

    let plan = power_bridge_span_plan(&city, (10, 10), 1, true).expect("valid Power span");
    assert_eq!(plan.span_count, 7);
    assert_eq!(plan.terminal, (17, 10));
    assert_eq!(plan.selector_mask, PE_POWER_BRIDGE_SELECTOR_MASK);
    assert_eq!(pe_power_bridge_selector_cost_45b930(0), Some(10));
    assert_eq!(pe_power_bridge_selector_cost_45b930(1), None);

    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_win95_power_stroke_44a310((10, 10), (10, 10)), 0);
    assert_eq!(
        state.city.city_funds(),
        10_000,
        "bridge chooser defers debit"
    );
    let dialog = state.city_info_dialog.expect("Power bridge chooser");
    assert!(matches!(
        dialog,
        CityInfoDialog::PowerBridge {
            seed_x: 10,
            seed_y: 10,
            direction: 1,
            span_count: 7,
            dry_stroke: false,
        }
    ));
    assert!(state.city_info_dialog_action_enabled(CityInfoDialogAction::SelectPowerBridge));
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
        Some(CityInfoDialogAction::SelectPowerBridge)
    );

    assert_eq!(state.finish_power_bridge((10, 10), 1, 7), 7);
    assert_eq!(state.city.city_funds(), 9_930);
    assert_eq!(
        (10..=16)
            .map(|x| state.city.building_at(x, 10))
            .collect::<Vec<_>>(),
        vec![0x12, 0x5c, 0x5c, 0x5c, 0x5c, 0x5c, 0x10],
        "native PE power-bridge endpoint and interior XBLD"
    );
    assert_eq!(
        (10..=16)
            .map(|x| state.city.terrain_at(x, 10))
            .collect::<Vec<_>>(),
        vec![3, 0x20, 0x20, 0x20, 0x20, 0x20, 1],
        "native PE power-bridge endpoint and interior XTER"
    );
    assert_eq!(
        (10..=16)
            .map(|x| state.city.altitude_at(x, 10) & 0x1f)
            .collect::<Vec<_>>(),
        vec![6, 5, 5, 5, 5, 5, 6],
        "native PE raises only power-bridge bank endpoints"
    );
    assert_eq!(
        (10..=16)
            .map(|x| state.city.tile_bits_at(x, 10).unwrap().raw_bits())
            .collect::<Vec<_>>(),
        vec![0x80, 0x86, 0x86, 0x86, 0x86, 0x86, 0x80],
        "native PE clears endpoint water and marks rotated powerable interiors"
    );
    for (endpoint, tile) in [(10, 0x12), (16, 0x10)] {
        assert_eq!(state.city.building_at(endpoint, 10), tile);
        assert!(state.city.tile_bits_at(endpoint, 10).unwrap().powerable());
        assert!(!state.city.tile_bits_at(endpoint, 10).unwrap().water_tile());
        assert_eq!(state.city.altitude_at(endpoint, 10) & 0x1f, 6);
    }
    assert_eq!(state.city.terrain_at(10, 10), 3);
    assert_eq!(state.city.terrain_at(16, 10), 1);
    for x in 11..=15 {
        assert_eq!(state.city.building_at(x, 10), 0x5c);
        let bits = state.city.tile_bits_at(x, 10).unwrap();
        assert!(bits.powerable());
        assert!(bits.rotate());
        assert!(
            bits.water_tile(),
            "the PE interior worker preserves XBIT water"
        );
    }
    for x in 10..=16 {
        assert_eq!(
            state.city.traffic_density_at(x, 10),
            Some(0x80 | (x & !1) as u8),
            "Power bridge construction must leave XTRF unchanged"
        );
    }
}

#[test]
fn pe_power_dry_route_uses_mode_two_walker_and_one_cell_water_lookahead() {
    let mut city = City::new_blank("/tmp/sc2000-power-bridge-handoff.sc2", "PowerBridgeHandoff")
        .expect("blank city");
    city.set_city_funds(10_000);
    city.set_terrain(9, 10, 0x40);
    city.set_altitude(9, 10, 5, 5);
    for x in 10..=16 {
        city.set_terrain(x, 10, if x == 10 { 0x21 } else { 0x20 });
        city.set_altitude(x, 10, 5, 5);
        city.set_tile_bits(x, 10, TileBits::new(0).with_water_tile(true));
    }
    city.set_terrain(17, 10, 0x40);
    city.set_altitude(17, 10, 5, 5);
    city.set_tile_bits(40, 40, TileBits::new(0x48));

    let mut state = test_state(city.clone());
    state.tool = EditTool::Power;
    assert_eq!(state.place_win95_power_stroke_44a310((9, 10), (11, 10)), 1);
    assert_eq!(state.city.building_at(9, 10), 0x0e);
    assert!(state.city.tile_bits_at(9, 10).unwrap().powerable());
    assert_eq!(
        state.city.city_funds(),
        9_998,
        "surface charge precedes bridge choice"
    );
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::PowerBridge {
            seed_x: 10,
            seed_y: 10,
            direction: 1,
            span_count: 7,
            dry_stroke: true,
        })
    );
    assert_eq!(state.city.building_at(10, 10), 0);
    assert_eq!(state.city.tile_bits_at(40, 40).unwrap().raw_bits(), 0x48);
    state.close_city_info_dialog();
    assert_eq!(state.city.tile_bits_at(40, 40).unwrap().raw_bits(), 0);
    assert_eq!(state.city.city_funds(), 9_998);
    assert_eq!(state.city.building_at(10, 10), 0);

    // The optional span uses the approach direction even when the direct
    // water tool would choose a different axis at this shoreline.
    city.set_terrain(10, 10, 0x22);
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_win95_power_stroke_44a310((9, 10), (11, 10)), 1);
    assert!(matches!(
        state.city_info_dialog,
        Some(CityInfoDialog::PowerBridge {
            direction: 1,
            dry_stroke: true,
            ..
        })
    ));
    assert_eq!(state.finish_power_bridge((10, 10), 1, 7), 7);
    assert_eq!(state.city.tile_bits_at(40, 40).unwrap().raw_bits(), 0x48);
    state.close_city_info_dialog();
    assert_eq!(state.city.tile_bits_at(40, 40).unwrap().raw_bits(), 0);
    assert_eq!(state.city.city_funds(), 9_928);
    assert_eq!(state.city.building_at(9, 10), 0x0f);
    assert_eq!(state.city.building_at(10, 10), 0x12);
    assert_eq!(state.city.building_at(16, 10), 0x10);
}

#[test]
fn tool_allowed_in_underground_view_matches_opensc2k_gates() {
    assert!(tool_allowed_in_underground_view(EditTool::Pipes));
    assert!(tool_allowed_in_underground_view(EditTool::Subway));
    assert!(tool_allowed_in_underground_view(EditTool::Query));
    assert!(tool_allowed_in_underground_view(EditTool::Bulldoze));
    assert!(!tool_allowed_in_underground_view(EditTool::Road));
    assert!(!tool_allowed_in_underground_view(EditTool::ZoneResidential));
}

#[test]
fn mac_day20_publishes_water_and_summary_from_the_same_original_pass() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-water-tower-phase.sc2", "Tower").unwrap();
        city.kind = kind;
        city.set_mac_state_misc_u32_at(0x68, 15);
        city.set_mac_selected_water_height(20);
        city.set_building(10, 10, 0xdc);
        city.set_tile_bits(10, 10, TileBits::new(0x60));
        city.set_building(11, 10, 0xeb);
        city.set_tile_bits(11, 10, TileBits::new(0x60));
        for x in 12..62 {
            city.set_building(x, 10, 0x70);
            city.set_tile_bits(x, 10, TileBits::new(0x20));
        }
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.sim_calendar_day20_water();
        assert!(
            state.city.tile_bits_at(11, 10).unwrap().watered(),
            "{kind:?} tower fills"
        );
        assert_eq!(
            state.city.mac_water_service_summary(),
            46,
            "{kind:?}: original worker returns 50/107 before newly filled tower can carry forward"
        );
        state.sim_calendar_day20_water();
        assert_eq!(
            state.city.mac_water_service_summary(),
            24,
            "{kind:?}: next pass includes the tower's 100 carried units"
        );
    }
}

#[test]
fn mac_water_phase_publishes_treatment_gate_without_overwriting_win_misc() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-water-gate.sc2", "Gate").unwrap();
        city.kind = kind;
        city.set_mac_state_misc_u32_at(1043 * 4, 0x1234_0000);
        city.set_traffic_density(40, 40, 104);
        city.set_building(40, 40, 0xcf);
        city.set_building(41, 40, 5);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.sim_calendar_day20_water();
        assert_eq!(
            state.city.mac_treatment_gate(),
            1,
            "{kind:?}: 0 served <= 0 threshold"
        );
        let gated_pollution =
            live_mac_pollution_density_map_for_city(&state.city)[density_map_index(40, 40)];
        if kind == CityFileKind::WinSc2 {
            assert_eq!(state.city.misc_u32_at(1043 * 4), Some(0x1234_0000));
        } else {
            assert_eq!(state.city.mac_state_misc_u32_at(1043 * 4), Some(1));
        }
        state.city.set_mac_state_misc_u32_at(0x68, 15);
        state.city.set_mac_selected_water_height(0);
        state.city.set_building(10, 10, 0xdc);
        state.city.set_tile_bits(10, 10, TileBits::new(0x60));
        state.city.set_building(11, 10, 0x70);
        state.city.set_tile_bits(11, 10, TileBits::new(0x20));
        state.sim_calendar_day20_water();
        assert_eq!(
            state.city.mac_treatment_gate(),
            0,
            "{kind:?}: 1 served exceeds 0 threshold"
        );
        let ungated_pollution =
            live_mac_pollution_density_map_for_city(&state.city)[density_map_index(40, 40)];
        assert!(
            ungated_pollution > gated_pollution,
            "{kind:?}: native gate feeds the pollution divisor"
        );
        if kind == CityFileKind::WinSc2 {
            assert_eq!(state.city.misc_u32_at(1043 * 4), Some(0x1234_0000));
        }
    }
}

#[test]
fn mac_arcology_slots_each_run_water_worker_and_publish_treatment_gate() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-arcology-water-slots.sc2", "Arco").unwrap();
        city.kind = kind;
        city.set_mac_state_misc_u32_at(0x68, 15);
        city.set_mac_selected_water_height(20);
        city.set_mac_state_misc_u32_at(0x102c, 10000);
        city.set_mac_state_misc_u32_at(1043 * 4, 0x1234_0001);
        city.set_building(10, 10, 0xdc);
        city.set_tile_bits(10, 10, TileBits::new(0x60));
        city.set_building(11, 10, 0xeb);
        city.set_tile_bits(11, 10, TileBits::new(0x60));
        for x in 12..62 {
            city.set_building(x, 10, 0x70);
            city.set_tile_bits(x, 10, TileBits::new(0x20));
        }
        let first =
            usize::from(city.provision_microsim_for_building(0xfb).unwrap() - MIN_SIM_TEXT_ENTRIES);
        let second =
            usize::from(city.provision_microsim_for_building(0xfb).unwrap() - MIN_SIM_TEXT_ENTRIES);
        assert!(first < second);
        for slot in [first, second] {
            city.set_microsim_stat0(slot, 5);
            city.set_microsim_stat1(slot, 55);
            city.set_microsim_stat2(slot, 100);
        }
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.refresh_mac_xmic_with_arcologies([100; 5], [7, 7, 7]);
        let first_residents = state.city.microsim_at(first).unwrap().stat2;
        let second_residents = state.city.microsim_at(second).unwrap().stat2;
        assert_eq!(
            first_residents - second_residents,
            22,
            "{kind:?}: slots use original-worker water results 46 then 24"
        );
        assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
        assert_eq!(state.city.mac_water_service_summary(), 24);
        assert_eq!(state.city.mac_treatment_gate(), 0);
        if kind == CityFileKind::WinSc2 {
            assert_eq!(state.city.misc_u32_at(1043 * 4), Some(0x1234_0001));
        }
    }
}

#[test]
fn mac_xmic_water_rows_observe_arcology_pass_in_slot_order() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-xmic-water-interleave.sc2", "Water").unwrap();
        city.kind = kind;
        city.set_mac_water_service_summary(12);
        city.set_mac_state_misc_u32_at(0x68, 15);
        city.set_mac_selected_water_height(20);
        city.set_building(10, 10, 0xdc);
        city.set_tile_bits(10, 10, TileBits::new(0x60));
        city.set_building(11, 10, 0xeb);
        city.set_tile_bits(11, 10, TileBits::new(0x60));
        for x in 12..62 {
            city.set_building(x, 10, 0x70);
            city.set_tile_bits(x, 10, TileBits::new(0x20));
        }
        let mut add_row = |tile| {
            usize::from(city.provision_microsim_for_building(tile).unwrap() - MIN_SIM_TEXT_ENTRIES)
        };
        let before = add_row(0xf4);
        let arcology = add_row(0xfb);
        let after = add_row(0xf4);
        assert!(before < arcology && arcology < after);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        let mut expected_rng = state.mac_process_rng;
        let before_value = 12_u8.wrapping_add((expected_rng.random_u16() & 7) as u8);
        expected_rng.random_u16();
        expected_rng.random_u16();
        let after_value = 46_u8.wrapping_add((expected_rng.random_u16() & 7) as u8);
        expected_rng.random_u16();
        expected_rng.random_u16();
        state.refresh_mac_xmic_with_arcologies([100; 5], [7; 3]);
        assert_eq!(
            state.city.microsim_at(before).unwrap().stat0,
            before_value,
            "{kind:?} row before arcology"
        );
        assert_eq!(
            state.city.microsim_at(after).unwrap().stat0,
            after_value,
            "{kind:?} row after arcology"
        );
        assert_eq!(
            state.mac_process_rng.seed(),
            expected_rng.seed(),
            "{kind:?} QuickDraw stream"
        );
        assert!(state.city.tile_bits_at(11, 10).unwrap().watered());
        assert_eq!(state.city.mac_water_service_summary(), 46);
    }
}

#[test]
fn mac_annual_phase0_xmic_and_ordinance_draws_feed_phase1_power_stream() {
    // Choose a controlled incoming seed whose annual statue draw is followed
    // by all three conditional automatic-ordinance draws. The phase-1 dynamic
    // source must then consume the fifth value from this same stream.
    let seed = (1..100_000)
        .find(|&seed| {
            let mut rng = MacToolboxRng::new(seed);
            rng.random_u16(); // annual statue row
            if rng.random_u16() & 7 != 0 {
                return false;
            }
            if 50_000 + u32::from(rng.random_u16()) >= 200_000 {
                return false;
            }
            true
        })
        .expect("find a seed that takes the full automatic-ordinance path");

    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-phase0-phase1-rng-chain.city", "RngChain")
            .expect("blank city");
        city.kind = kind;
        city.set_city_funds(200_000);
        city.set_saved_date_and_day(
            crate::city::CityDate {
                year: 1900,
                month: 12,
            },
            31,
        );

        // The statue XMIC row consumes one QuickDraw value in the annual
        // CODE 3 slot walk. The C8 source consumes one in phase-1 export_218.
        city.set_microsim_tile_id(20, 0xdb);
        city.set_building(10, 10, 0xc8);
        city.set_terrain(10, 10, 27);
        city.set_tile_bits(10, 10, TileBits::new(0x80));
        city.set_building(11, 10, 0x70);
        city.set_tile_bits(11, 10, TileBits::new(0x80));

        let mut expected_rng = MacToolboxRng::new(seed);
        let expected_statue = expected_rng.random_u16() % 42;
        assert_eq!(expected_rng.random_u16() & 7, 0);
        assert!(50_000 + u32::from(expected_rng.random_u16()) < 200_000);
        let expected_ordinance = usize::from(expected_rng.random_u16() % 20);
        let expected_power_draw = expected_rng.random_u16();

        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(seed);
        state.no_disasters_enabled = false;

        // Native CODE 4 phase 0 performs the January XMIC pass, monthly
        // budget work, then automatic ordinance selection. CODE 14 phase 1
        // follows on either city-file layout.
        state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);
        assert_eq!(state.city.microsim_at(20).unwrap().stat2, expected_statue);
        assert_ne!(
            state.city.mac_state_misc_u32_at(0x0fa0).unwrap_or(0) & (1 << expected_ordinance),
            0,
            "{kind:?}: phase-0 ordinance selection must follow the XMIC row"
        );
        state.sim_calendar_day1_power();

        assert_eq!(
            state.mac_process_rng.seed(),
            expected_rng.seed(),
            "{kind:?}: phase-1 source draw must follow phase-0 XMIC and ordinance draws"
        );
        assert_ne!(
            expected_power_draw, 0,
            "fixture reaches the dynamic-source draw"
        );
    }
}

#[test]
fn mac_arcology_tax_inputs_keep_the_full_saved_dword() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-arcology-raw-tax.sc2", "Tax").unwrap();
        city.kind = kind;
        let offset = tax_rate_misc_offset_for_city(&city, 0).unwrap();
        city.set_budget_misc_u32_values(offset, &[0x0001_0007]);
        assert_eq!(raw_tax_rate_for_city(&city, 0), 0x0001_0007, "{kind:?}");
        assert_eq!(
            tax_rate_for_city(&city, 0),
            MAX_TAX_RATE,
            "{kind:?} UI remains clamped"
        );
    }
}

#[test]
fn mac_zoo_game_rng_respects_plant_expiry_slot_order() {
    for zoo_first in [true, false] {
        let mut city = City::new_blank("/tmp/mac-zoo-plant-slot.sc2", "Zoo").unwrap();
        city.kind = CityFileKind::MacCity;
        let (zoo_slot, plant_slot) = if zoo_first {
            (20_usize, 21_usize)
        } else {
            (21, 20)
        };
        city.set_microsim_tile_id(zoo_slot, 0xda);
        city.set_microsim_tile_id(plant_slot, 0xc9);
        city.set_microsim_stat0(plant_slot, 50);
        for dx in 0..4 {
            for dy in 0..4 {
                city.set_building(20 + dx, 20 - dy, 0xc9);
                city.set_terrain(20 + dx, 20 - dy, 0);
            }
        }
        city.set_text_overlay(20, 20, MIN_SIM_TEXT_ENTRIES + plant_slot as u8);
        city.set_text_overlay(21, 20, MIN_SIM_TEXT_ENTRIES + zoo_slot as u8);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.no_disasters_enabled = false;
        let mut expected = state.mac_game_rng;
        if zoo_first {
            for _ in 0..4 {
                expected.bounded(100);
            }
        }
        state.refresh_mac_xmic_with_arcologies([100; 5], [7; 3]);
        assert_eq!(state.city.microsim_at(zoo_slot).unwrap().tile_id, 0);
        assert_eq!(
            state.mac_game_rng.state(),
            expected.state(),
            "zoo_first={zoo_first}"
        );
    }
}

#[test]
fn mac_arcology_total_keeps_processed_rows_removed_later_in_walk() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for arcology_first in [true, false] {
            let mut city = City::new_blank("/tmp/mac-arcology-total-slot.sc2", "Arcology").unwrap();
            city.kind = kind;
            city.set_mac_state_misc_u32_at(0x102c, 8_000);
            let (arcology, plant) = if arcology_first {
                (20_usize, 21_usize)
            } else {
                (21, 20)
            };
            city.set_microsim_tile_id(arcology, 0xfb);
            city.set_microsim_stat0(arcology, 5);
            city.set_microsim_stat1(arcology, 55);
            city.set_microsim_stat2(arcology, 100);
            city.set_microsim_tile_id(plant, 0xc9);
            city.set_microsim_stat0(plant, 50);
            for dx in 0..4 {
                for dy in 0..4 {
                    city.set_building(20 + dx, 20 - dy, 0xc9);
                    city.set_terrain(20 + dx, 20 - dy, 0);
                }
            }
            city.set_text_overlay(20, 20, MIN_SIM_TEXT_ENTRIES + plant as u8);
            city.set_text_overlay(21, 20, MIN_SIM_TEXT_ENTRIES + arcology as u8);
            if kind == CityFileKind::MacCity {
                city.set_mac_state_misc_u32_at(0x01f0 + 0xfb * 4, 16);
                city.set_mac_state_misc_u32_at(0x01f0 + 0xfc * 4, 16);
            } else {
                city.pe_dw_tile_count[0xfb] = 16;
                city.pe_dw_tile_count[0xfc] = 16;
            }
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.no_disasters_enabled = false;
            state.refresh_mac_xmic_with_arcologies([100; 5], [7; 3]);
            assert_eq!(state.city.microsim_at(arcology).unwrap().tile_id, 0);
            // Original selector replay returns 348 with 46% water; this map
            // has no water sources, so export_209 returns 100 instead.
            assert_eq!(
                state.city.mac_arcology_resident_total(),
                if arcology_first { 402 } else { 0 },
                "{kind:?} arcology_first={arcology_first}"
            );
        }
    }
}

#[test]
fn view_terrain_rotation_preserves_corner_geometry_in_every_water_band() {
    use crate::tiles::rotated_terrain_id;
    // Corner masks run clockwise around the data grid. These encode the
    // actual raised corners, independently of the artwork's shape numbering.
    let corners: [u8; 14] = [0, 9, 3, 6, 12, 11, 7, 14, 13, 1, 2, 4, 8, 15];
    for band in [0x00, 0x10, 0x20, 0x30] {
        for shape in 0..14_u8 {
            for rotation in 0..4 {
                let rotated = rotated_terrain_id(band + shape, rotation);
                let mask = corners[shape as usize];
                let expected = ((u16::from(mask) << rotation) | (u16::from(mask) >> (4 - rotation)))
                    as u8
                    & 15;
                assert_eq!(rotated & 0xf0, band);
                assert_eq!(
                    corners[(rotated & 15) as usize],
                    expected,
                    "shape={shape} band={band:#x} rotation={rotation}"
                );
                let sprites = terrain_sprites(rotated);
                if band == 0 || band == 0x20 {
                    assert_eq!(sprites.terrain, Some(1256 + u32::from(rotated & 15)));
                }
                if band == 0x20 || band == 0x30 {
                    assert_eq!(sprites.water, Some(1270 + u32::from(rotated & 15)));
                }
                assert_eq!(
                    rotated_terrain_id(rotated, (4 - rotation) & 3),
                    band + shape
                );
            }
        }
    }
}

#[test]
fn view_terrain_rotation_turns_streams_and_bays_and_preserves_waterfalls() {
    use crate::tiles::rotated_terrain_id;
    for rotation in 0..4 {
        assert_eq!(
            rotated_terrain_id(0x40, rotation),
            [0x40, 0x41, 0x40, 0x41][rotation as usize]
        );
        assert_eq!(
            rotated_terrain_id(0x41, rotation),
            [0x41, 0x40, 0x41, 0x40][rotation as usize]
        );
        assert_eq!(
            rotated_terrain_id(0x42, rotation),
            [0x42, 0x43, 0x44, 0x45][rotation as usize]
        );
        assert_eq!(rotated_terrain_id(0x3e, rotation), 0x3e);
        assert_eq!(rotated_terrain_id(0xff, rotation), 0xff);
    }
}

#[test]
fn pe_pipe_event_publishes_native_water_tower_carry_and_refill() {
    for layout in ["pe-pipe-tower-refill.sc2", "pe-pipe-tower-refill.mac"] {
        for (pump, tower, consumer, population, expected) in [
            (0x60, 0x30, 0x20, 49_999, [0x70, 0x20, 0x30]),
            (0x60, 0x70, 0x20, 49_999, [0x70, 0x70, 0x30]),
            (0x40, 0x70, 0x20, 49_999, [0x40, 0x70, 0x20]),
            (0x60, 0x70, 0x00, 49_999, [0x70, 0x70, 0x00]),
            (0x60, 0x30, 0x20, 50_000, [0x60, 0x30, 0x20]),
        ] {
            let mut city = blank_city_layout_fixture(layout, "Water Tower refill");
            city.prepare_pe_fidelity_from_saved_city();
            city.set_mac_state_misc_u32_at(0x0e40, 0);
            city.pe_b_weather_humid = 2;
            city.set_city_funds(1000);
            city.persist_pe_day22_population_leftover(population);
            let cells = [
                (40, 40, 0xdc, pump),
                (40, 39, 0xeb, tower),
                (41, 40, 0x70, consumer),
            ];
            for (x, y, building, bits) in cells {
                city.set_building(x, y, building);
                city.set_tile_bits(x, y, TileBits::new(bits));
            }
            let mut state = test_state(city);
            state.tool = EditTool::Pipes;
            assert_eq!(
                state.place_underground_at(60, 60, UndergroundKind::Pipes),
                1
            );
            assert_eq!(
                cells.map(|(x, y, _, _)| state.city.tile_bits_at(x, y).unwrap().raw_bits()),
                expected,
                "{layout} with XBIT {pump:#04x}/{tower:#04x}/{consumer:#04x} at population {population}"
            );
        }
    }
}

#[test]
fn pe_day20_water_preserves_the_conductive_plane_and_latches_native_counters() {
    for layout in ["pe-water-calendar.sc2", "pe-water-calendar.mac"] {
        let mut city = blank_city_layout_fixture(layout, "Water calendar");
        city.prepare_pe_fidelity_from_saved_city();
        city.set_mac_state_misc_u32_at(0x0e40, 21);
        city.pe_b_weather_humid = 4;
        // Only saved XBIT supplies membership, so there is no XUND bridge
        // between the pump and the fifty consumers.
        city.set_building(40, 40, 0xdc);
        city.set_tile_bits(40, 40, TileBits::new(0x68));
        city.set_building(39, 40, 0xeb);
        city.set_tile_bits(39, 40, TileBits::new(0x60));
        for x in 41..91 {
            city.set_building(x, 40, 0x70);
            city.set_tile_bits(x, 40, TileBits::new(0x20));
        }
        let mut state = test_state(city);
        for expected in [46, 24] {
            state.game_clock = GameClock::new_with_day(1901, 1, 19);
            let start = state.game_clock;
            state.game_clock = start.wrapping_add_days(1);
            state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

            assert_eq!(state.city.pe_water_coverage_percent(), expected);
            assert_eq!(state.city.pe_w_sewer_bonus(), 0);
            assert_eq!(state.city.tile_bits_at(39, 40).unwrap().raw_bits(), 0x70);
            assert_eq!(state.city.tile_bits_at(40, 40).unwrap().raw_bits(), 0x70);
            for x in 41..91 {
                assert_eq!(state.city.tile_bits_at(x, 40).unwrap().raw_bits(), 0x30);
            }
        }

        state.sim_calendar_yearly_updates();
        assert_eq!(state.city.pe_water_coverage_percent(), 24);
        assert_eq!(state.city.pe_w_sewer_bonus(), 0);
    }
}

#[test]
fn pe_water_service_native_root_order_and_treatment_census() {
    let paths = [
        [(40, 40), (41, 40), (42, 40), (42, 41), (42, 42)],
        [(40, 42), (41, 42), (42, 42), (42, 41), (42, 40)],
    ];
    for rotation in 0..4 {
        for (path, first) in paths
            .into_iter()
            .zip([matches!(rotation, 0 | 1), matches!(rotation, 1 | 2)])
        {
            let mut city = blank_city_layout_fixture("pe-water-root-order.sc2", "Water root order");
            city.pe_w_city_rotation = rotation;
            city.set_mac_state_misc_u32_at(0x0e40, 0);
            city.pe_b_weather_humid = 2;
            for (index, (x, y)) in path.into_iter().enumerate() {
                let pump = matches!(index, 0 | 4);
                city.set_building(x, y, if pump { 0xdc } else { 0x70 });
                city.set_tile_bits(x, y, TileBits::new(if pump { 0x60 } else { 0x20 }));
            }

            let maps = pe_fun_00464550(&mut city);
            assert_eq!(maps.pe_worker_capacity_units, 2);
            assert_eq!(maps.pe_worker_served_cells, 2);
            assert_eq!(city.pe_water_coverage_percent(), 100);
            let mut expected = [0x70, 0x30, 0x30, 0x20, 0x70];
            if !first {
                expected.reverse();
            }
            assert_eq!(
                path.map(|(x, y)| city.tile_bits_at(x, y).unwrap().raw_bits()),
                expected
            );
        }
    }

    let mut city = blank_city_layout_fixture("pe-water-census.sc2", "Treatment census");
    for (count, expected) in [
        (0, 0),
        (1, 0),
        (3, 0),
        (4, 1),
        (5, 1),
        (8, 1),
        (0x8000, 1),
        (0xffff, 0),
    ] {
        city.publish_pe_sewer_bonus(1, count);
        assert_eq!(
            city.pe_w_sewer_bonus(),
            expected,
            "census WORD {count:#06x}"
        );
    }
}

#[test]
fn pe_day2_density_after_open_matches_original_saved_plane_fixture() {
    let mut city = City::new_blank("/tmp/pe-day2-open.sc2", "Density replay").unwrap();
    for (x, y, building, bits) in [
        (12, 20, 0x70, 0),
        (20, 12, 0x71, 0),
        (12, 21, 0, 4),
        (17, 13, 0xd5, 0),
        (22, 14, 0xd5, 0),
        (35, 41, 5, 0),
        (4, 11, 0, 0x0a),
        (5, 11, 0x70, 0x08),
        (125, 125, 0xfb, 0),
        (127, 125, 0xfb, 0),
    ] {
        city.set_building(x, y, building);
        city.set_tile_bits(x, y, TileBits::new(bits));
    }
    for (x, y, zone) in [(12, 20, 1), (20, 12, 4), (17, 13, 5), (22, 14, 6)] {
        city.set_zone(x, y, zone);
    }
    city.set_terrain(12, 20, 3);
    city.set_terrain(35, 41, 9);
    city.set_traffic_density(12, 20, 100);
    city.set_traffic_density(20, 12, 125);

    let plane = |step: usize, default, cells: &[(usize, usize, u8)]| {
        let mut values = vec![default; CITY_TILES];
        for &(x, y, value) in cells {
            for dy in 0..step {
                for dx in 0..step {
                    values[density_map_index(x * step + dx, y * step + dy)] = value;
                }
            }
        }
        values
    };
    city.sync_population_density_from_slice(&plane(4, 0, &[(3, 5, 20), (5, 3, 160)]));
    city.sync_crime_density_from_slice(&plane(2, 0, &[(6, 10, 17), (10, 6, 23)]));
    city.sync_pollution_density_from_slice(&plane(2, 0, &[(6, 10, 22), (10, 6, 34)]));
    city.sync_rate_of_growth_from_slice(&plane(4, 127, &[(3, 5, 130), (5, 3, 140)]));

    let mut state = test_state(city);
    let traffic = state.city.traffic_density_slice().unwrap().to_vec();
    state.rebuild_services_after_city_open();
    state.sim_calendar_day2_density(Default::default(), Default::default());

    let city = &state.city;
    let fingerprints = [
        pe_density_plane_fingerprint(2, |x, y| city.land_value_at(x, y).unwrap()),
        pe_density_plane_fingerprint(2, |x, y| city.pollution_density_at(x, y).unwrap()),
        pe_density_plane_fingerprint(2, |x, y| city.crime_density_at(x, y).unwrap()),
        pe_density_plane_fingerprint(4, |x, y| city.population_density_at(x, y).unwrap()),
        pe_density_plane_fingerprint(4, |x, y| city.rate_of_growth_at(x, y).unwrap()),
        pe_density_plane_fingerprint(1, |x, y| city.tile_bits_at(x, y).unwrap().raw_bits()),
    ];
    let samples = [(6, 10), (10, 6), (8, 6), (11, 7), (62, 62), (2, 5)]
        .map(|(x, y)| city.land_value_at(x * 2, y * 2).unwrap());
    assert_eq!(
        fingerprints,
        [
            0xd319_a608_5bd5_49ca,
            0xa0c2_12e5_7b27_7209,
            0x16c5_919d_cccc_b785,
            0x9bbc_f1e8_0f3d_9911,
            0x594c_cb40_f6ae_7bf2,
            0x68f6_59ce_5f06_7e2b,
        ],
        "XVAL samples {samples:?}, counters {:?}",
        (
            city.pe_dw_city_land_value,
            city.pe_dw_city_pollution,
            city.pe_dw_city_crime
        ),
    );
    assert_eq!(city.pe_dw_city_land_value, 627);
    assert_eq!(city.pe_dw_city_pollution, 224);
    assert_eq!(city.pe_dw_city_crime, 58);
    assert_eq!(city.traffic_density_slice(), Some(traffic.as_slice()));
}

#[test]
fn pe_day2_station_coverage_matches_original_saved_plane_fixture() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank("/tmp/pe-day2-stations.sc2", "Station replay").unwrap();
        city.kind = kind;
        for (x, y, building, zone, bits) in [
            (5, 13, 0xd2, 0x80, 0x40),
            (24, 36, 0xd2, 0, 0xc0),
            (43, 57, 0xd2, 0x80, 0),
            (63, 81, 0xd3, 0x80, 0x80),
            (79, 97, 0xd3, 0, 0xc0),
            (107, 120, 0xd3, 0x80, 0x40),
            (127, 7, 0xd2, 0x80, 0xc0),
        ] {
            city.set_building(x, y, building);
            city.set_zone(x, y, zone);
            city.set_tile_bits(x, y, TileBits::new(bits));
        }
        set_budget_funding_for_city_test(&mut city, 0, 40);
        set_budget_funding_for_city_test(&mut city, 1, 32);
        let mut state = test_state(city);
        state.sim_calendar_day2_density(Default::default(), Default::default());
        let city = &state.city;
        assert_eq!(
            pe_density_plane_fingerprint(4, |x, y| city.police_coverage_at(x, y).unwrap()),
            0x0fe9_5505_7a5b_199f,
            "{kind:?} XPLC",
        );
        assert_eq!(
            pe_density_plane_fingerprint(4, |x, y| city.fire_coverage_at(x, y).unwrap()),
            0x36d3_be58_af88_a6dd,
            "{kind:?} XFIR",
        );
    }
}

#[test]
#[ignore = "requires a local Win95 save and original phase-2 replay output"]
fn pe_open_and_phase2_saved_city_differential() {
    let source = std::env::var_os("CITYGAME_TEST_PE_PHASE2_CITY").expect("Win95 save path");
    let native = std::env::var_os("CITYGAME_TEST_PE_PHASE2_NATIVE")
        .map(std::path::PathBuf::from)
        .expect("native replay output directory");
    let mut state = test_state(City::load(source).expect("load Win95 save"));
    state.pe_process_crt = Win95CrtRand::new(3);
    state.rebuild_services_after_city_open();

    let plane = |step: usize, byte: &dyn Fn(usize, usize) -> u8| {
        let mut bytes = Vec::new();
        for x in (0..CITY_WIDTH).step_by(step) {
            for y in (0..CITY_HEIGHT).step_by(step) {
                bytes.push(byte(x, y));
            }
        }
        bytes
    };
    let xbit = |city: &City| plane(1, &|x, y| city.tile_bits_at(x, y).unwrap().raw_bits());
    let opened_xbit = xbit(&state.city);
    let opened_scalars = [
        state.pe_process_crt.seed(),
        state.city.pe_power_used_percent(),
        state.city.pe_water_coverage_percent(),
        u32::from(state.city.pe_w_sewer_bonus()),
    ];
    state.sim_calendar_day2_density(Default::default(), Default::default());
    let city = &state.city;
    let scalars = opened_scalars
        .into_iter()
        .chain([
            city.pe_dw_city_land_value,
            city.pe_dw_city_pollution,
            city.pe_dw_city_crime,
        ])
        .flat_map(u32::to_be_bytes)
        .collect();
    let results = [
        ("OPEN_XBIT", opened_xbit),
        ("XBIT", xbit(city)),
        ("XVAL", plane(2, &|x, y| city.land_value_at(x, y).unwrap())),
        (
            "XPLT",
            plane(2, &|x, y| city.pollution_density_at(x, y).unwrap()),
        ),
        (
            "XCRM",
            plane(2, &|x, y| city.crime_density_at(x, y).unwrap()),
        ),
        (
            "XPOP",
            plane(4, &|x, y| city.population_density_at(x, y).unwrap()),
        ),
        (
            "XROG",
            plane(4, &|x, y| city.rate_of_growth_at(x, y).unwrap()),
        ),
        (
            "XPLC",
            plane(4, &|x, y| city.police_coverage_at(x, y).unwrap()),
        ),
        (
            "XFIR",
            plane(4, &|x, y| city.fire_coverage_at(x, y).unwrap()),
        ),
        (
            "XTRF",
            plane(2, &|x, y| city.traffic_density_at(x, y).unwrap()),
        ),
        ("SCALARS", scalars),
    ];
    let mut differences = Vec::new();
    for (name, actual) in results {
        let expected = std::fs::read(native.join(name)).expect("read native replay output");
        assert_eq!(actual.len(), expected.len(), "{name} length");
        let mut diff = actual
            .iter()
            .zip(&expected)
            .enumerate()
            .filter(|(_, (a, b))| a != b);
        if let Some(first) = diff.next() {
            differences.push(format!(
                "{name}: {} differences, first {first:?}",
                1 + diff.count()
            ));
            if name == "SCALARS" {
                let words = |bytes: &[u8]| {
                    bytes
                        .chunks_exact(4)
                        .map(|word| u32::from_be_bytes(word.try_into().unwrap()))
                        .collect::<Vec<_>>()
                };
                differences.push(format!(
                    "Rust {:?}, native {:?}",
                    words(&actual),
                    words(&expected)
                ));
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

fn pe_density_plane_fingerprint(step: usize, byte: impl Fn(usize, usize) -> u8) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for x in (0..CITY_WIDTH).step_by(step) {
        for y in (0..CITY_HEIGHT).step_by(step) {
            hash = (hash ^ u64::from(byte(x, y))).wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}
