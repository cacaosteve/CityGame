use super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DIRECT_PLACEABLE_BUILDING_TOOLS_FOR_TEST: &[EditTool] = &[
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

// Synthetic gameplay fixtures, not recovered Mac New City defaults. The
// constructor always starts WinSc2, so assign the intended layout and verify
// the kind after an actual save/load before claiming a two-layout regression.
pub(super) fn blank_city_layout_fixture(path: impl AsRef<Path>, name: &str) -> City {
    let path = path.as_ref();
    let kind = if path.extension().and_then(|s| s.to_str()) == Some("sc2") {
        CityFileKind::WinSc2
    } else {
        CityFileKind::MacCity
    };
    let mut city = City::new_blank(path, name).unwrap();
    city.kind = kind;
    let saved = std::env::temp_dir().join(format!(
        "citygame-layout-{}-{}",
        std::process::id(),
        path.file_name().unwrap().to_string_lossy()
    ));
    city.save(&saved).unwrap();
    let loaded = City::load(&saved).unwrap();
    std::fs::remove_file(saved).unwrap();
    assert_eq!(loaded.kind, kind);
    loaded
}

pub(super) fn test_state(mut city: City) -> ViewerState {
    city.set_pe_scenario_active(city.kind == CityFileKind::WinScenario);
    let auto_goto_enabled = auto_goto_enabled_for_city(&city);
    let sound_effects_enabled = sound_effects_enabled_for_city(&city);
    let music_enabled = music_enabled_for_city(&city);
    let no_disasters_enabled = no_disasters_enabled_for_city(&city);
    let traffic = traffic_system_for_city(&city, EngineFidelity::Win95);
    let mut win95_overlay_sim = Win95OverlaySim::new();
    win95_overlay_sim.reset_for_city(&city);
    let game_clock = GameClock::default_start();
    let pe_process_crt = init_pe_process_crt(&city, game_clock);
    let pe_process_game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
    let pe_process_lfsr = init_pe_process_lfsr(&city, game_clock);
    ViewerState {
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
        auto_goto_enabled,
        pending_disaster_auto_goto: None,
        pending_disaster_start_sound: None,
        pending_disaster_start_siren: false,
        pending_disaster_extra_sounds: Vec::new(),
        earthquake_shake_remaining: 0,
        pe_disaster_duration: 0,
        pe_hurricane_wind_remaining: 0,
        sound_effects_enabled,
        music_enabled,
        no_disasters_enabled,
        balloon_help_enabled: false,
        balloon_help_line: None,
        anim_time: 0.0,
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
        game_clock,
        zone_growth_enabled: true,
        zone_growth_uses_allowlist: false,
        zone_growth_allowed_tiles: vec![false; CITY_TILES],
        trip_served_tiles: vec![false; CITY_TILES],
        bypass_growth_trip_gate: true,
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
    }
}

fn expected_single_tile_mode1_rubble_after_generic_scatter(crt: &mut Win95CrtRand) -> u8 {
    // The generic 1×1 DestroyBuilding(1) render loop consumes rand() at
    // 0x459DEB and 0x459E19 before PlaceTile's optional rubble draw at 0x45A023.
    let _ = crt.random_u15();
    let _ = crt.random_u15();
    crt.rubble_tile_id()
}

fn preview_pe_flood_tick(
    state: &ViewerState,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> (City, PeFloodTickStats) {
    let mut expected = state.city.clone();
    let stats = pe_flood_tick_city_with_rng(&mut expected, state.pe_disaster_duration, crt, lfsr);
    (expected, stats)
}

fn preview_pe_fire_tick(
    state: &ViewerState,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> (City, PeFireTickStats) {
    let mut expected = state.city.clone();
    let stats = pe_fire_tick_city_with_rng(&mut expected, crt, lfsr);
    (expected, stats)
}

fn preview_pe_riot_tick(
    state: &ViewerState,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> (City, PeRiotTickStats) {
    let mut expected = state.city.clone();
    let stats = pe_riot_tick_city_with_rng(&mut expected, crt, lfsr);
    (expected, stats)
}

fn preview_pe_fb_tick(
    state: &ViewerState,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> (City, PeFbTickStats) {
    let mut expected = state.city.clone();
    let stats = pe_fb_tick_city_with_rng(&mut expected, crt, lfsr);
    (expected, stats)
}

fn preview_pe_tornado_tick(
    state: &ViewerState,
    crt: &mut Win95CrtRand,
) -> (City, PeTornadoTickStats) {
    let mut expected = state.city.clone();
    let stats = pe_tornado_tick_city_with_rng(&mut expected, crt);
    (expected, stats)
}

fn preview_pe_monster_tick(
    state: &ViewerState,
    crt: &mut Win95CrtRand,
    lfsr: &mut Win95Lfsr,
) -> (City, PeMonsterTickStats) {
    let (view_x, view_y) = state.disaster_target_tile();
    let mut expected = state.city.clone();
    let stats = pe_monster_tick_city_with_rng(
        &mut expected,
        view_x.min(255) as u8,
        view_y.min(255) as u8,
        crt,
        lfsr,
    );
    (expected, stats)
}

fn advance_pe_disaster_scan_preview(state: &ViewerState, crt: &mut Win95CrtRand) {
    let seen = pe_disaster_overlay_seen(&state.city);
    let _ = pe_disaster_scan_sounds_with_rng(&state.city, seen, crt);
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| {
        let value = value.trim();
        value.eq_ignore_ascii_case("1")
            || value.eq_ignore_ascii_case("true")
            || value.eq_ignore_ascii_case("on")
            || value.eq_ignore_ascii_case("yes")
            || value.eq_ignore_ascii_case("y")
    })
}

fn env_flag_value(name: &str, value: Option<&str>) -> bool {
    let original = std::env::var_os(name);
    match value {
        Some(value) => {
            // `set_var`/`remove_var` are `unsafe` on modern Rust targets.
            // These helpers are test-only and intentionally manipulate process env.
            unsafe { std::env::set_var(name, value) }
        }
        None => unsafe { std::env::remove_var(name) },
    }
    let parsed = env_flag_enabled(name);
    match original {
        Some(value) => unsafe { std::env::set_var(name, value) },
        None => unsafe { std::env::remove_var(name) },
    }
    parsed
}

fn test_chunk(name: &str, data: Vec<u8>) -> crate::city::CityChunk {
    crate::city::CityChunk {
        name: name.to_owned(),
        offset: 0,
        raw_len: data.len(),
        data,
    }
}

fn test_scen_goals_chunk(goals: ScenarioGoals) -> crate::city::CityChunk {
    let mut data = vec![0; 56];
    data[0..4].copy_from_slice(&[0x80, 0x00, 0x00, 0x00]);
    data[0x04..0x06].copy_from_slice(&goals.disaster_type.to_be_bytes());
    data[0x06] = goals.disaster_x;
    data[0x07] = goals.disaster_y;
    data[0x08..0x0a].copy_from_slice(&goals.time_limit_months.to_be_bytes());
    data[0x0a..0x0e].copy_from_slice(&goals.city_population_goal.to_be_bytes());
    data[0x0e..0x12].copy_from_slice(&goals.residential_population_goal.to_be_bytes());
    data[0x12..0x16].copy_from_slice(&goals.commercial_population_goal.to_be_bytes());
    data[0x16..0x1a].copy_from_slice(&goals.industrial_population_goal.to_be_bytes());
    data[0x1a..0x1e].copy_from_slice(&goals.fund_goal.to_be_bytes());
    data[0x1e..0x22].copy_from_slice(&goals.land_value_goal.to_be_bytes());
    data[0x22..0x24].copy_from_slice(
        &u16::try_from(goals.life_expectancy_goal.min(u32::from(u16::MAX)))
            .unwrap_or(u16::MAX)
            .to_be_bytes(),
    );
    data[0x24..0x26].copy_from_slice(
        &u16::try_from(goals.education_goal.min(u32::from(u16::MAX)))
            .unwrap_or(u16::MAX)
            .to_be_bytes(),
    );
    data[0x26..0x2a].copy_from_slice(&goals.pollution_limit.to_be_bytes());
    data[0x2a..0x2e].copy_from_slice(&goals.crime_limit.to_be_bytes());
    data[0x2e..0x32].copy_from_slice(&goals.traffic_limit.to_be_bytes());
    data[0x32] = goals.build_item_1;
    data[0x33] = goals.build_item_2;
    data[0x34..0x36].copy_from_slice(&goals.build_item_1_tiles.unwrap_or(0).to_be_bytes());
    data[0x36..0x38].copy_from_slice(&goals.build_item_2_tiles.unwrap_or(0).to_be_bytes());
    test_chunk("SCEN", data)
}

fn test_scen_chunk(disaster_type: u16, disaster_x: u8, disaster_y: u8) -> crate::city::CityChunk {
    let mut data = vec![0; 56];
    data[0..4].copy_from_slice(&[0x80, 0x00, 0x00, 0x00]);
    data[0x04..0x06].copy_from_slice(&disaster_type.to_be_bytes());
    data[0x06] = disaster_x;
    data[0x07] = disaster_y;
    data[0x08..0x0a].copy_from_slice(&60_u16.to_be_bytes());
    test_chunk("SCEN", data)
}

fn seed_all_drag_states(state: &mut ViewerState) {
    state.zone_drag = Some(ZoneDrag {
        start: (10, 10),
        current: (11, 11),
        zone_id: ZONE_RESIDENTIAL_LIGHT,
    });
    state.network_drag = Some(NetworkDrag {
        start: (12, 12),
        current: (13, 13),
        kind: NetworkKind::Road,
    });
    state.underground_drag = Some(UndergroundDrag {
        start: (14, 14),
        current: (15, 15),
        kind: UndergroundKind::Pipes,
    });
    state.highway_drag = Some(HighwayDrag {
        start: (16, 16),
        current: (17, 17),
    });
    state.terrain_drag = Some(TerrainDrag {
        current: (18, 18),
        tool: EditTool::LevelTerrain,
        target_altitude: 3,
        placed_tiles: 1,
    });
    state.edit_map_stretch_drag = Some(EditMapStretchDrag {
        tile: (18, 19),
        start_mouse_y: 100.0,
        start_land: 3,
        water: 3,
        applied_levels: 0,
        changed: false,
    });
    state.nature_drag = Some(NatureDrag {
        current: (19, 19),
        placed_tiles: 1,
        tool: EditTool::Trees,
        shift_down: false,
    });
}

fn assert_drag_states_cleared(state: &ViewerState) {
    assert_eq!(state.zone_drag, None);
    assert_eq!(state.network_drag, None);
    assert_eq!(state.underground_drag, None);
    assert_eq!(state.highway_drag, None);
    assert_eq!(state.terrain_drag, None);
    assert_eq!(state.edit_map_stretch_drag, None);
    assert_eq!(state.nature_drag, None);
}

fn seed_all_modal_dialogs(state: &mut ViewerState) {
    state.power_plant_dialog = Some(PowerPlantDialogState {
        info_tool: Some(EditTool::PowerPlantCoal),
        pressed_tool: Some(EditTool::PowerPlantOil),
    });
    state.arcology_dialog = Some(ArcologyDialogState {
        info_tool: Some(EditTool::ArcologyForest),
        pressed_tool: Some(EditTool::ArcologyLaunch),
    });
    state.city.set_label(2, "Draft Sign");
    state.sign_dialog = Some(SignDialogState {
        sign_id: 2,
        original_text: "Original Sign".to_owned(),
        text: "Draft Sign".to_owned(),
    });
    state.military_base_dialog = Some(MilitaryBaseDialogState::default());
    state.city_info_dialog = Some(CityInfoDialog::Map);
}

fn assert_modal_dialogs_cleared(state: &ViewerState) {
    assert_eq!(state.power_plant_dialog, None);
    assert_eq!(state.arcology_dialog, None);
    assert_eq!(state.sign_dialog, None);
    assert_eq!(state.military_base_dialog, None);
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.city.label_at(2), Some("Original Sign"));
}

fn tunnel_test_city() -> City {
    let mut city =
        City::new_blank("/tmp/sc2000-tunnel-test.sc2", "tunnel test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_terrain(20, 20, 1);
    city.set_altitude(20, 20, 5, 5);
    city.set_altitude(19, 20, 7, 7);
    city.set_altitude(18, 20, 6, 6);
    city.set_terrain(17, 20, 3);
    city.set_altitude(17, 20, 5, 5);
    city
}

fn stamp_complete_building_for_test(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    building: u8,
    size: usize,
) {
    for y in origin_y..origin_y + size {
        for x in origin_x..origin_x + size {
            // Keep `pe_dw_tile_count` coherent for tests that bypass PlaceTile.
            let old = city.building_at(x, y) as usize;
            city.pe_dw_tile_count[old] = city.pe_dw_tile_count[old].wrapping_sub(1);
            city.pe_dw_tile_count[building as usize] =
                city.pe_dw_tile_count[building as usize].wrapping_add(1);
            city.set_building(x, y, building);
            if rci_building_kind(building).is_some()
                || is_abandoned_building(building)
                || is_construction_building(building)
            {
                // PE ChooseAndPlace leaves an occupied RCI tile powerable;
                // the day-1 flood later republishes its powered bit.
                city.set_tile_bits(x, y, TileBits::new(XBIT_CHOOSE_AND_PLACE));
            }
        }
    }
}

fn set_building_and_pe_count_for_test(city: &mut City, x: usize, y: usize, building: u8) {
    // Raw map edits bypass PlaceTile's saved `dwTileCount` maintenance. Keep
    // fixtures coherent when the PE budget census reads those counters.
    let old = usize::from(city.building_at(x, y));
    let new = usize::from(building);
    if old != new {
        city.pe_dw_tile_count[old] = city.pe_dw_tile_count[old].wrapping_sub(1);
        city.pe_dw_tile_count[new] = city.pe_dw_tile_count[new].wrapping_add(1);
        city.set_building(x, y, building);
    }
}

fn seed_service_coverage_footprint_for_test(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    size: usize,
    powered: bool,
) {
    // PE FUN_0046A840 only counts D2/D3 source tiles with saved XBIT 0x80;
    // XBIT 0x40 on those same tiles selects full rather than half strength.
    for y in origin_y..origin_y + size {
        for x in origin_x..origin_x + size {
            city.set_tile_bits(
                x,
                y,
                TileBits::new(0).with_powerable(true).with_powered(powered),
            );
        }
    }
}

fn set_flat_terrain_neighborhood_for_test(city: &mut City, x: usize, y: usize, land: u8) {
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            let tx = (x as isize + dx) as usize;
            let ty = (y as isize + dy) as usize;
            city.set_altitude(tx, ty, land, 0);
        }
    }
}

fn set_misc_u32_for_test(city: &mut City, offset: usize, value: u32) {
    city.set_misc_u32_at(offset, value);
}

fn set_rci_demand_for_test(city: &mut City, residential: i32, commercial: i32, industrial: i32) {
    for (index, value) in [residential, commercial, industrial]
        .into_iter()
        .enumerate()
    {
        set_misc_u32_for_test(city, RCI_DEMAND_MISC_OFFSET + index * 4, value as u32);
    }
}

fn stamp_xval_for_test(city: &mut City, value: u8) {
    city.sync_land_value_from_slice(&vec![value; CITY_TILES]);
}

fn vacant_rci_transit_ring_count_for_test(
    city: &City,
    origin: (usize, usize),
    size: usize,
) -> usize {
    building_footprint_from_origin(origin.0, origin.1, size)
        .unwrap()
        .into_iter()
        .filter(|&(x, y)| game_is_valid_transit_items(city, x, y))
        .count()
}

fn mark_transit_ring_served_for_test(state: &mut ViewerState, origin: (usize, usize), size: usize) {
    for (x, y) in building_footprint_from_origin(origin.0, origin.1, size).unwrap() {
        if game_is_valid_transit_items(&state.city, x, y) {
            state.trip_served_tiles[y * CITY_WIDTH + x] = true;
        }
    }
}

/// Empty-city PE R/C delta is `ftol(-1 * 600) = -600`. Seed demand so vacant
/// R/C/I still grow from last month's `wCityDemand` after that step.
fn seed_pe_rci_demand_to_survive_months(city: &mut City, months: u32) {
    let seed = 100 + 600 * months as i32;
    set_rci_demand_for_test(city, seed, seed, seed);
}

fn set_budget_funding_for_test(city: &mut City, budget_index: usize, funding: u32) {
    ensure_budget_misc_defaults_before_extension(city);
    let Some(offset) = budget_service_misc_offset(budget_index, 0) else {
        return;
    };
    let mut values = city.misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
    let live_current = budget_service_built_counts_for_city(city).by_budget_index(budget_index);
    if values[BUDGET_SERVICE_CURRENT_FIELD] == 0 {
        values[BUDGET_SERVICE_CURRENT_FIELD] = live_current;
    }
    // Keep explicit 0% distinguishable from a zero-filled / unset PE new-city row.
    if funding == 0 && values[BUDGET_SERVICE_CURRENT_FIELD] == 0 {
        values[BUDGET_SERVICE_CURRENT_FIELD] = 1;
    }
    values[BUDGET_SERVICE_FUNDING_FIELD] = funding;
    city.set_misc_u32_values(offset, &values);
}

fn set_budget_funding_for_city_test(city: &mut City, budget_index: usize, funding: u32) {
    ensure_budget_misc_defaults_before_extension(city);
    let Some(offset) = budget_service_misc_offset_for_city(city, budget_index, 0) else {
        return;
    };
    let mut values = city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
    values.resize(BUDGET_SERVICE_MISC_STRIDE, 0);
    if values[BUDGET_SERVICE_CURRENT_FIELD] == 0 {
        values[BUDGET_SERVICE_CURRENT_FIELD] = 1;
    }
    values[BUDGET_SERVICE_FUNDING_FIELD] = funding;
    city.set_budget_misc_u32_values(offset, &values);
}

fn assert_budget_service_row_for_test(
    city: &City,
    service_index: usize,
    current: u32,
    funding: u32,
    year_to_date: u32,
) {
    assert_eq!(
        city.misc_u32_at(
            budget_service_misc_offset(service_index, BUDGET_SERVICE_CURRENT_FIELD).unwrap()
        ),
        Some(current),
        "budget service row {service_index} current"
    );
    assert_eq!(
        city.misc_u32_at(
            budget_service_misc_offset(service_index, BUDGET_SERVICE_FUNDING_FIELD).unwrap()
        ),
        Some(funding),
        "budget service row {service_index} funding"
    );
    assert_eq!(
        city.misc_u32_at(
            budget_service_misc_offset(service_index, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap()
        ),
        Some(year_to_date),
        "budget service row {service_index} year-to-date"
    );
}

fn stamp_transport_access_for_test(city: &mut City, x: usize, y: usize) {
    city.set_building(
        x,
        y,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
}

fn stamp_pop3_road_orientation_for_test(city: &mut City, origin_x: usize, origin_y: usize) {
    city.set_building(
        origin_x - 1,
        origin_y - 1,
        network_building_for_mask(
            NetworkKind::Road,
            NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT,
        ),
    );
}

fn stamp_pop3_first_candidate_road_for_test(city: &mut City, origin: (usize, usize)) {
    let old_footprint = building_footprint_from_origin(20, 20, 2).unwrap();
    let (x, y, ids) = pop3_startbuilding_road_probes(origin.0, origin.1 + 2)
        .find(|(x, y, _)| !old_footprint.contains(&(*x, *y)))
        .expect("pop3 candidate has a diagonal road probe outside the old footprint");
    city.set_building(x, y, ids[0]);
}

fn or_rci_building_zone_corners_for_test(city: &mut City, origin: (usize, usize), size: usize) {
    let corner_bits = city.footprint_corner_bits();
    for (x, y) in building_footprint_from_origin(origin.0, origin.1, size).unwrap() {
        let zone_id = city.zone_at(x, y).map(|zone| zone.zone_id()).unwrap_or(0);
        city.set_zone(
            x,
            y,
            zone_id | building_zone_bits_for_tile(origin.0, origin.1, x, y, size, corner_bits),
        );
    }
}

fn rci_xzon_view_corner_in_footprint_for_test(
    city: &City,
    origin: (usize, usize),
    size: usize,
) -> (usize, usize) {
    building_footprint_from_origin(origin.0, origin.1, size)
        .unwrap()
        .into_iter()
        .find(|&(x, y)| xzon_corner_check(city, x, y))
        .expect("stamped XZON includes the view-rotation corner")
}

fn rci_expansion_from_2x2_view_corner_for_test(
    state: &mut ViewerState,
    two_by_two: (usize, usize),
) -> Option<RciDevelopmentCandidate> {
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&state.city, two_by_two, 2);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    state.rci_expansion_redevelopment_candidate_at(
        sx,
        sy,
        &reserved,
        &power_maps,
        city_service_outcomes_for_city(&state.city),
        &rci_development_services_for_city(&state.city),
        None,
    )
}

fn rci_expansion_from_3x3_view_corner_for_test(
    state: &mut ViewerState,
    three_by_three: (usize, usize),
) -> Option<RciDevelopmentCandidate> {
    let (sx, sy) = rci_xzon_view_corner_in_footprint_for_test(&state.city, three_by_three, 3);
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    state.rci_expansion_redevelopment_candidate_at(
        sx,
        sy,
        &reserved,
        &power_maps,
        city_service_outcomes_for_city(&state.city),
        &rci_development_services_for_city(&state.city),
        None,
    )
}

fn stamp_zone_footprint_for_test(
    city: &mut City,
    origin: (usize, usize),
    size: usize,
    zone_id: u8,
) {
    for (x, y) in building_footprint_from_origin(origin.0, origin.1, size).unwrap() {
        city.set_zone(x, y, zone_id);
    }
}

fn stamp_zoned_building_footprint_for_test(
    city: &mut City,
    origin: (usize, usize),
    size: usize,
    zone_id: u8,
    building: u8,
) {
    let corner_bits = city.footprint_corner_bits();
    for (x, y) in building_footprint_from_origin(origin.0, origin.1, size).unwrap() {
        city.set_zone(
            x,
            y,
            zone_id | building_zone_bits_for_tile(origin.0, origin.1, x, y, size, corner_bits),
        );
        city.set_building(x, y, building);
        if rci_building_kind(building).is_some()
            || is_abandoned_building(building)
            || is_construction_building(building)
        {
            city.set_tile_bits(x, y, TileBits::new(XBIT_CHOOSE_AND_PLACE));
        }
    }
}

fn enable_airport_demand_for_test(city: &mut City) {
    stamp_special_zone_demand_population_for_test(
        city,
        RciKind::Commercial,
        COMMERCIAL_3X3_BUILDINGS[0],
        (80, 80),
        AIRPORT_DEMAND_MIN_COMMERCIAL_POPULATION,
    );
}

fn enable_seaport_demand_for_test(city: &mut City) {
    stamp_special_zone_demand_population_for_test(
        city,
        RciKind::Industrial,
        INDUSTRIAL_3X3_BUILDINGS[0],
        (80, 104),
        SEAPORT_DEMAND_MIN_INDUSTRIAL_POPULATION,
    );
}

fn enable_special_zone_demand_for_test(city: &mut City) {
    enable_airport_demand_for_test(city);
    enable_seaport_demand_for_test(city);
}

fn stamp_airport_runway_zone_ray_for_test(city: &mut City, origin: (usize, usize)) {
    let (dx, dy) = if city.rotation() & 1 != 0 {
        if origin.1 & 1 != 0 {
            (1usize, 0usize)
        } else if origin.0 & 1 != 0 {
            (0, 1)
        } else {
            panic!("PE runway class-0 rejects even/even candidates");
        }
    } else if origin.0 & 1 != 0 {
        (0, 1)
    } else if origin.1 & 1 != 0 {
        (1, 0)
    } else {
        panic!("PE runway class-0 rejects even/even candidates");
    };
    for step in 0..5 {
        city.set_zone(origin.0 + dx * step, origin.1 + dy * step, ZONE_AIRPORT);
    }
}

fn stamp_seaport_crane_water_ray_for_test(city: &mut City, origin: (usize, usize)) {
    // Choose east (the second direction probe) so the ray stays clear of the
    // fixture's south-side water-pump plumbing. PE checks five XBIT-0x04 cells;
    // only the fifth cell needs water at least two levels above land.
    for step in 1..=5 {
        let x = origin.0 + step;
        let y = origin.1;
        city.set_tile_bits(
            x,
            y,
            city.tile_bits_at(x, y)
                .unwrap_or_default()
                .with_water_tile(true),
        );
        city.set_altitude(x, y, 5, if step == 5 { 7 } else { 5 });
    }
}

fn stamp_city_fame_population_for_test(city: &mut City) {
    for (building, origin) in [
        (RESIDENTIAL_3X3_BUILDINGS[0], (56, 50)),
        (COMMERCIAL_3X3_BUILDINGS[0], (60, 50)),
        (INDUSTRIAL_3X3_BUILDINGS[0], (64, 50)),
    ] {
        stamp_complete_building_for_test(city, origin.0, origin.1, building, 3);
    }
}

fn stamp_special_zone_demand_population_for_test(
    city: &mut City,
    kind: RciKind,
    building: u8,
    start: (usize, usize),
    required_population: u32,
) {
    let size = usize::from(xbld_tile_size(building).max(1));
    let footprint_capacity = rci_tile_capacity(building, kind).saturating_mul((size * size) as u32);
    assert!(footprint_capacity > 0);
    let mut stamped_population = 0_u32;
    let mut origin_x = start.0;
    let mut origin_y = start.1;
    while stamped_population < required_population {
        assert!(origin_y + size <= CITY_HEIGHT);
        stamp_complete_building_for_test(city, origin_x, origin_y, building, size);
        stamped_population = stamped_population.saturating_add(footprint_capacity);
        origin_x += size + 1;
        if origin_x + size > CITY_WIDTH {
            origin_x = start.0;
            origin_y += size + 1;
        }
    }
}

fn tool_routes_to_city_action(tool: EditTool) -> bool {
    matches!(
        tool,
        EditTool::Query
            | EditTool::Bulldoze
            | EditTool::RaiseTerrain
            | EditTool::LowerTerrain
            | EditTool::LevelTerrain
            | EditTool::StretchTerrain
            | EditTool::SpreadTerrain
            | EditTool::Water
            | EditTool::Trees
            | EditTool::Forest
            | EditTool::Sign
            | EditTool::Highway
            | EditTool::Onramp
            | EditTool::SubToRail
            | EditTool::Tunnel
    ) || is_chooser_menu_tool(tool)
        || is_dispatch_tool(tool)
        || network_kind_for_tool(tool).is_some()
        || underground_kind_for_tool(tool).is_some()
        || zone_id_for_tool(tool).is_some()
        || placeable_building_for_tool(tool).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CityToolBehavior {
    Query,
    Bulldoze,
    TerrainEdit,
    SurfaceWater,
    Trees,
    Forest,
    Sign,
    Network(NetworkKind),
    Highway,
    Tunnel,
    Onramp,
    Underground(UndergroundKind),
    SubToRail,
    Zone(u8),
    DirectBuilding(u8),
    HydroPower,
    ChooserDialog,
    Dispatch(DispatchKind),
    EditMapOnly,
}

const ALL_EDIT_TOOLS: &[EditTool] = &[
    EditTool::Query,
    EditTool::Bulldoze,
    EditTool::RaiseTerrain,
    EditTool::LowerTerrain,
    EditTool::StretchTerrain,
    EditTool::LevelTerrain,
    EditTool::SpreadTerrain,
    EditTool::Water,
    EditTool::Stream,
    EditTool::Trees,
    EditTool::Forest,
    EditTool::Sign,
    EditTool::Road,
    EditTool::Highway,
    EditTool::Tunnel,
    EditTool::Onramp,
    EditTool::BusDepot,
    EditTool::Rail,
    EditTool::Subway,
    EditTool::RailDepot,
    EditTool::SubStation,
    EditTool::SubToRail,
    EditTool::Power,
    EditTool::ZoneResidential,
    EditTool::ZoneResidentialDense,
    EditTool::ZoneCommercial,
    EditTool::ZoneCommercialDense,
    EditTool::ZoneIndustrial,
    EditTool::ZoneIndustrialDense,
    EditTool::SmallPark,
    EditTool::BigPark,
    EditTool::Zoo,
    EditTool::Stadium,
    EditTool::Marina,
    EditTool::PowerPlant,
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
    EditTool::Pipes,
    EditTool::WaterPump,
    EditTool::WaterTower,
    EditTool::WaterTreatment,
    EditTool::Desalinization,
    EditTool::Seaport,
    EditTool::Airport,
    EditTool::DispatchPolice,
    EditTool::DispatchFirefighters,
    EditTool::DispatchMilitary,
    EditTool::MayorHouse,
    EditTool::CityHall,
    EditTool::Statue,
    EditTool::BraunLlamaDome,
    EditTool::Arcologies,
    EditTool::ArcologyPlymouth,
    EditTool::ArcologyForest,
    EditTool::ArcologyDarco,
    EditTool::ArcologyLaunch,
    EditTool::DeZone,
];

const ALL_CITY_TOOL_MENUS: &[ui::ToolMenu] = &[
    ui::ToolMenu::Residential,
    ui::ToolMenu::Commercial,
    ui::ToolMenu::Industrial,
    ui::ToolMenu::Parks,
    ui::ToolMenu::Power,
    ui::ToolMenu::Civic,
    ui::ToolMenu::Education,
    ui::ToolMenu::Water,
    ui::ToolMenu::Terrain,
    ui::ToolMenu::TerrainEdit,
    ui::ToolMenu::Road,
    ui::ToolMenu::Rail,
    ui::ToolMenu::Transport,
    ui::ToolMenu::Emergency,
    ui::ToolMenu::Rewards,
];

fn visible_city_menu_tools() -> Vec<EditTool> {
    let mut tools = vec![EditTool::Sign, EditTool::Query];
    for menu in ALL_CITY_TOOL_MENUS {
        tools.extend(ui::tool_menu_items(*menu).iter().map(|item| item.tool));
    }
    tools
}

fn drain_command_queue_for_test() {
    while pop_command().is_some() {}
}

fn expected_city_tool_behavior(tool: EditTool) -> CityToolBehavior {
    match tool {
        EditTool::Query => CityToolBehavior::Query,
        EditTool::Bulldoze => CityToolBehavior::Bulldoze,
        EditTool::RaiseTerrain
        | EditTool::LowerTerrain
        | EditTool::LevelTerrain
        | EditTool::StretchTerrain
        | EditTool::SpreadTerrain => CityToolBehavior::TerrainEdit,
        EditTool::Water => CityToolBehavior::SurfaceWater,
        EditTool::Trees => CityToolBehavior::Trees,
        EditTool::Sign => CityToolBehavior::Sign,
        EditTool::Road => CityToolBehavior::Network(NetworkKind::Road),
        EditTool::Rail => CityToolBehavior::Network(NetworkKind::Rail),
        EditTool::Power => CityToolBehavior::Network(NetworkKind::Power),
        EditTool::Highway => CityToolBehavior::Highway,
        EditTool::Tunnel => CityToolBehavior::Tunnel,
        EditTool::Onramp => CityToolBehavior::Onramp,
        EditTool::Pipes => CityToolBehavior::Underground(UndergroundKind::Pipes),
        EditTool::Subway => CityToolBehavior::Underground(UndergroundKind::Subway),
        EditTool::SubToRail => CityToolBehavior::SubToRail,
        EditTool::ZoneResidential => CityToolBehavior::Zone(ZONE_RESIDENTIAL_LIGHT),
        EditTool::ZoneResidentialDense => CityToolBehavior::Zone(ZONE_RESIDENTIAL_DENSE),
        EditTool::ZoneCommercial => CityToolBehavior::Zone(ZONE_COMMERCIAL_LIGHT),
        EditTool::ZoneCommercialDense => CityToolBehavior::Zone(ZONE_COMMERCIAL_DENSE),
        EditTool::ZoneIndustrial => CityToolBehavior::Zone(ZONE_INDUSTRIAL_LIGHT),
        EditTool::ZoneIndustrialDense => CityToolBehavior::Zone(ZONE_INDUSTRIAL_DENSE),
        EditTool::Airport => CityToolBehavior::Zone(ZONE_AIRPORT),
        EditTool::Seaport => CityToolBehavior::Zone(ZONE_SEAPORT),
        EditTool::DeZone => CityToolBehavior::Zone(0),
        EditTool::BusDepot => CityToolBehavior::DirectBuilding(236),
        EditTool::RailDepot => CityToolBehavior::DirectBuilding(237),
        EditTool::SubStation => CityToolBehavior::DirectBuilding(233),
        EditTool::SmallPark => CityToolBehavior::DirectBuilding(13),
        EditTool::BigPark => CityToolBehavior::DirectBuilding(213),
        EditTool::Zoo => CityToolBehavior::DirectBuilding(218),
        EditTool::Stadium => CityToolBehavior::DirectBuilding(215),
        EditTool::Marina => CityToolBehavior::DirectBuilding(248),
        EditTool::PowerPlantCoal => CityToolBehavior::DirectBuilding(207),
        EditTool::PowerPlantHydro => CityToolBehavior::HydroPower,
        EditTool::PowerPlantOil => CityToolBehavior::DirectBuilding(202),
        EditTool::PowerPlantGas => CityToolBehavior::DirectBuilding(201),
        EditTool::PowerPlantNuclear => CityToolBehavior::DirectBuilding(203),
        EditTool::PowerPlantWind => CityToolBehavior::DirectBuilding(200),
        EditTool::PowerPlantSolar => CityToolBehavior::DirectBuilding(204),
        EditTool::PowerPlantMicrowave => CityToolBehavior::DirectBuilding(205),
        EditTool::PowerPlantFusion => CityToolBehavior::DirectBuilding(206),
        EditTool::Police => CityToolBehavior::DirectBuilding(210),
        EditTool::FireStation => CityToolBehavior::DirectBuilding(211),
        EditTool::Hospital => CityToolBehavior::DirectBuilding(209),
        EditTool::Prison => CityToolBehavior::DirectBuilding(216),
        EditTool::School => CityToolBehavior::DirectBuilding(214),
        EditTool::College => CityToolBehavior::DirectBuilding(217),
        EditTool::Library => CityToolBehavior::DirectBuilding(245),
        EditTool::Museum => CityToolBehavior::DirectBuilding(212),
        EditTool::WaterPump => CityToolBehavior::DirectBuilding(220),
        EditTool::WaterTower => CityToolBehavior::DirectBuilding(235),
        EditTool::WaterTreatment => CityToolBehavior::DirectBuilding(244),
        EditTool::Desalinization => CityToolBehavior::DirectBuilding(250),
        EditTool::MayorHouse => CityToolBehavior::DirectBuilding(243),
        EditTool::CityHall => CityToolBehavior::DirectBuilding(208),
        EditTool::Statue => CityToolBehavior::DirectBuilding(219),
        EditTool::BraunLlamaDome => CityToolBehavior::DirectBuilding(255),
        EditTool::ArcologyPlymouth => CityToolBehavior::DirectBuilding(251),
        EditTool::ArcologyForest => CityToolBehavior::DirectBuilding(252),
        EditTool::ArcologyDarco => CityToolBehavior::DirectBuilding(253),
        EditTool::ArcologyLaunch => CityToolBehavior::DirectBuilding(254),
        EditTool::PowerPlant | EditTool::Arcologies => CityToolBehavior::ChooserDialog,
        EditTool::DispatchPolice => CityToolBehavior::Dispatch(DispatchKind::Police),
        EditTool::DispatchFirefighters => CityToolBehavior::Dispatch(DispatchKind::Firefighters),
        EditTool::DispatchMilitary => CityToolBehavior::Dispatch(DispatchKind::Military),
        EditTool::Stream => CityToolBehavior::EditMapOnly,
        EditTool::Forest => CityToolBehavior::Forest,
    }
}

fn city_tool_execution_fixture(tool: EditTool) -> (ViewerState, (usize, usize)) {
    let behavior = expected_city_tool_behavior(tool);
    let mut city = if behavior == CityToolBehavior::Tunnel {
        tunnel_test_city()
    } else {
        City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city")
    };
    city.set_city_funds(500_000);
    enable_all_conditional_palette_tools(&mut city);
    let mut tile = (20, 20);

    match behavior {
        CityToolBehavior::Bulldoze => {
            city.set_building(tile.0, tile.1, 112);
        }
        CityToolBehavior::Zone(0) => {
            city.set_zone(tile.0, tile.1, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
            city.set_building(tile.0, tile.1, 1);
        }
        CityToolBehavior::TerrainEdit => {
            city.set_altitude(tile.0, tile.1, 5, 0);
        }
        CityToolBehavior::DirectBuilding(248) => {
            for x in tile.0..tile.0 + 3 {
                city.set_terrain(x, tile.1, 0x30);
                city.set_tile_bits(x, tile.1, TileBits::new(0).with_water_tile(true));
            }
        }
        CityToolBehavior::DirectBuilding(250) => {
            city.set_terrain(tile.0 - 1, tile.1, 0x30);
            city.set_tile_bits(tile.0 - 1, tile.1, crate::city::TileBits::new(0b0000_0001));
        }
        CityToolBehavior::HydroPower => {
            city.set_terrain(tile.0, tile.1, 0x3e);
        }
        CityToolBehavior::Onramp => {
            tile = (64, 64);
        }
        CityToolBehavior::SubToRail => {
            // MapTool SubToRail requires an adjacent rail/subway entrance;
            // a blank-cell click is correctly refused by the PE admission gate.
            city.set_building(
                tile.0 - 1,
                tile.1,
                network_building_for_mask(NetworkKind::Rail, NETWORK_LEFT),
            );
        }
        _ => {}
    }

    let mut state = test_state(city);
    state.tool = tool;
    if behavior == CityToolBehavior::Onramp {
        stamp_onramp_fixture(&mut state, tile, NETWORK_TOP, NETWORK_RIGHT);
    }
    (state, tile)
}

fn assert_tool_stamped_building(
    state: &ViewerState,
    tool: EditTool,
    tile: (usize, usize),
    building: u8,
) {
    let size = usize::from(xbld_tile_size(building));
    let footprint = building_footprint_from_origin(tile.0, tile.1, size)
        .unwrap_or_else(|| panic!("{tool:?} footprint should fit at {tile:?}"));
    for &(x, y) in &footprint {
        assert_eq!(
            state.city.building_at(x, y),
            building,
            "{tool:?} did not stamp building {building} at ({x},{y})"
        );
    }
    assert_tool_debited_units(state, tool, 1);
}

fn expected_city_funds_after_tool(tool: EditTool, units: u32) -> i32 {
    let total = tool_build_cost(tool)
        .unwrap_or(0)
        .checked_mul(units)
        .expect("tool cost total fits");
    500_000 - i32::try_from(total).expect("tool cost fits")
}

fn assert_tool_debited_units(state: &ViewerState, tool: EditTool, units: u32) {
    assert_eq!(
        state.city.city_funds(),
        expected_city_funds_after_tool(tool, units),
        "{tool:?} did not debit the selected tool cost"
    );
}

fn tile_next_to_side(x: usize, y: usize, side: u8) -> (usize, usize) {
    match side {
        NETWORK_TOP => (x, y - 1),
        NETWORK_RIGHT => (x + 1, y),
        NETWORK_BOTTOM => (x, y + 1),
        NETWORK_LEFT => (x - 1, y),
        _ => panic!("invalid network side {side}"),
    }
}

fn stamp_onramp_fixture(
    state: &mut ViewerState,
    ramp: (usize, usize),
    highway_side: u8,
    road_side: u8,
) -> (usize, usize) {
    let highway_tile = tile_next_to_side(ramp.0, ramp.1, highway_side);
    let highway_axis = network_side_axis(road_side);
    state.city.set_building(
        highway_tile.0,
        highway_tile.1,
        match highway_axis {
            NetworkAxis::GridX => 74,
            NetworkAxis::GridY => 73,
        },
    );

    let road_tile = tile_next_to_side(ramp.0, ramp.1, road_side);
    let road_axis = network_side_axis(highway_side);
    let road_mask = match road_axis {
        NetworkAxis::GridX => NETWORK_LEFT | NETWORK_RIGHT,
        NetworkAxis::GridY => NETWORK_TOP | NETWORK_BOTTOM,
    };
    let road_building = network_building_for_mask(NetworkKind::Road, road_mask);
    state
        .city
        .set_building(road_tile.0, road_tile.1, road_building);
    match road_axis {
        NetworkAxis::GridX => {
            state
                .city
                .set_building(road_tile.0 - 1, road_tile.1, road_building);
            state
                .city
                .set_building(road_tile.0 + 1, road_tile.1, road_building);
        }
        NetworkAxis::GridY => {
            state
                .city
                .set_building(road_tile.0, road_tile.1 - 1, road_building);
            state
                .city
                .set_building(road_tile.0, road_tile.1 + 1, road_building);
        }
    }
    road_tile
}

fn set_military_base_type_for_test(city: &mut City, base_type: u32) {
    city.set_misc_u32_at(0x0e4c, base_type);
}

/// PE `0x45d066`: DECLINED + `0x402261` RandomAnd3==0 before `FUN_00452FA0`.
/// LFSR state `2` advances to `4` (`&3==0`).
fn arm_pe_maxis_man_spawn_gates(state: &mut ViewerState) {
    set_military_base_type_for_test(&mut state.city, MILITARY_BASE_DECLINED);
    state.pe_process_lfsr = Win95Lfsr::new(2);
}

fn enable_all_conditional_palette_tools(city: &mut City) {
    set_military_base_type_for_test(city, 2);
    city.set_rewards_available_mask(0b1_1111);
    city.set_text_overlay(30, 40, 0xff);
    stamp_complete_building_for_test(city, 100, 100, 210, 3);
    stamp_complete_building_for_test(city, 110, 100, 211, 3);
}

fn disaster_overlay_tiles(city: &City) -> Vec<(usize, usize, u8)> {
    active_disaster_tiles_for_city(city)
}

fn pe_pollution_accident_cloud_tiles(city: &City, x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut rng = Win95CrtRand::new(pe_pollution_accident_scatter_seed(city, x, y));
    let mut tiles = Vec::new();
    for _ in 0..pe_population_jitter_attempt_count(city) {
        let (dx, dy) = pe_pollution_accident_cloud_offset(&mut rng);
        if let Some((tx, ty)) = offset_city_tile(x, y, dx, dy) {
            tiles.push((tx, ty));
        }
    }
    tiles
}

fn pe_volcano_expected_overlays(
    city: &City,
    x: usize,
    y: usize,
) -> std::collections::HashMap<(usize, usize), u8> {
    pe_volcano_overlays(&mut city.clone(), x, y)
}

fn assert_pe_xthg_proxy(city: &City, x: usize, y: usize, slot: usize) {
    assert_eq!(
        city.text_overlay_at(x, y),
        crate::win95_overlay::xthg_proxy_for_slot(slot)
    );
}

fn assert_pe_tornado_actor(city: &City, x: usize, y: usize, fields: (u8, u8, u8, u8, u8)) -> usize {
    let (lane, height, target_x, target_y, saved_a) = fields;
    let (slot, record) = city.pe_tornado_actor_at(x, y).expect("PE tornado actor");
    assert!(record.is_pe_tornado_actor());
    assert_eq!(record.kind, XTHG_TORNADO);
    assert_eq!(record.lane, lane);
    assert_eq!(record.state, 0);
    assert_eq!(record.tile_x as usize, x);
    assert_eq!(record.tile_y as usize, y);
    assert_eq!(record.height, height);
    assert_eq!(record.sub_x, crate::win95_overlay::XTHG_TORNADO_SUBTILE);
    assert_eq!(record.sub_y, crate::win95_overlay::XTHG_TORNADO_SUBTILE);
    assert_eq!(record.target_x, target_x);
    assert_eq!(record.target_y, target_y);
    assert_eq!(
        record.saved_a, saved_a,
        "blank-city tornado saves XTXT before proxy paint"
    );
    assert_eq!(record.saved_b, 0);
    slot
}

fn assert_pe_monster_actor(city: &City, x: usize, y: usize, fields: (u8, u8, u8, u8)) -> usize {
    let (target_x, target_y, saved_a, saved_b) = fields;
    let (slot, record) = city.pe_monster_actor_at(x, y).expect("PE monster actor");
    assert!(record.is_pe_monster_actor());
    assert_eq!(record.kind, XTHG_MONSTER);
    assert_eq!(record.lane, XTHG_DIRECTION_EAST);
    assert_eq!(record.state, 0);
    assert_eq!(record.tile_x as usize, x);
    assert_eq!(record.tile_y as usize, y);
    assert_eq!(record.height, crate::win95_overlay::XTHG_MONSTER_HEIGHT);
    assert_eq!(record.sub_x, crate::win95_overlay::XTHG_MONSTER_SUBTILE);
    assert_eq!(record.sub_y, crate::win95_overlay::XTHG_MONSTER_SUBTILE);
    assert_eq!(record.target_x, target_x);
    assert_eq!(record.target_y, target_y);
    assert_eq!(
        record.saved_a, saved_a,
        "blank-city monster saves XTXT before proxy paint"
    );
    assert_eq!(record.saved_b, saved_b);
    slot
}

fn assert_pe_aircrash_actor(city: &City, x: usize, y: usize) -> usize {
    let (slot, record) = city
        .ambient_port_actor_at(x, y)
        .expect("PE air-crash airplane");
    assert!(record.is_pe_aircrash_actor());
    assert_eq!(record.kind, XTHG_AIRPLANE);
    assert_eq!(record.lane, 0);
    assert_eq!(record.state, crate::win95_overlay::XTHG_AIRCRASH_STATE);
    assert_eq!(record.tile_x as usize, x);
    assert_eq!(record.tile_y as usize, y);
    assert_eq!(record.height, crate::win95_overlay::XTHG_AIRCRASH_HEIGHT);
    assert_eq!(record.sub_x, crate::win95_overlay::XTHG_AIRCRASH_SUBTILE);
    assert_eq!(record.sub_y, crate::win95_overlay::XTHG_AIRCRASH_SUBTILE);
    assert_eq!(record.target_x, 0);
    assert_eq!(record.target_y, 0);
    assert_eq!(record.saved_a, 0);
    assert_eq!(record.saved_b, 0);
    assert_eq!(
        crate::win95_overlay::xthg_proxy_for_slot(slot),
        Some((slot as u8).wrapping_sub(0x37))
    );
    slot
}

fn populate_city_for_ambient_disaster(city: &mut City) {
    // This fixture explicitly seeds the PE runtime global used by the
    // day-22 ambient gate; census-derived population is not that input.
    city.set_pe_dw_city_population(5_000);
    for y in 20..30 {
        for x in 20..30 {
            city.set_building(x, y, 0x70);
        }
    }
}

fn stamp_abandoned_pop4_for_test(city: &mut City, origin_x: usize, origin_y: usize) {
    for (x, y) in building_footprint_from_origin(origin_x, origin_y, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, ABANDONED_POP4_BUILDINGS[0]);
    }
}

fn first_due_ambient_disaster_clock(city: &City) -> GameClock {
    first_ambient_chance_clock_with_remainder(city, 0, 0)
}

fn first_ambient_chance_clock_with_remainder(
    city: &City,
    min_remainder: u16,
    max_remainder: u16,
) -> GameClock {
    first_ambient_chance_clock_with_remainder_for::<Win95CrtRand>(
        city,
        min_remainder,
        max_remainder,
    )
}

fn first_ambient_chance_clock_with_remainder_for<R: AmbientDisasterRandom>(
    city: &City,
    min_remainder: u16,
    max_remainder: u16,
) -> GameClock {
    let difficulty = city_difficulty_for_city(city);
    for year in 1900..2100 {
        for month in 1..=12 {
            let clock = GameClock::new(year, month);
            if !pe_ambient_disaster_age_ready(city, clock) {
                continue;
            }
            let mut rng = R::from_seed(ambient_disaster_chance_seed(city, clock));
            if let Some(remainder) = pe_ambient_disaster_chance_remainder(&mut rng, difficulty) {
                if remainder >= min_remainder && remainder <= max_remainder {
                    return clock;
                }
            }
        }
    }
    panic!(
        "expected an ambient chance remainder in {min_remainder}..={max_remainder} after the PE min age"
    );
}

fn power_water_source_for_test(city: &mut City, x: usize, y: usize) {
    assert!(x >= 1);
    city.set_building(x - 1, y, 200);
    pe_seed_power_plant_xval_for_test(city, x - 1, y, 30);
    stamp_power_plant_xbit_powerable(city, &[(x - 1, y)]);
}

/// Seed the saved XBIT water state that a native PE city-tool/power pass would
/// have written for these hand-built integration fixtures. The worker consumes
/// XBIT directly; underground geometry alone is not a valid PE input.
fn seed_saved_water_worker_bits_for_test(city: &mut City) {
    let (pipe_tiles, _) = water_pipe_connection_maps(city);
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = y * CITY_WIDTH + x;
            let mut bits = city.tile_bits_at(x, y).unwrap_or_default();
            if pipe_tiles[index] {
                bits = bits.with_piped(true);
            }
            if matches!(city.building_at(x, y), 220 | 235 | 250) {
                bits = bits.with_powered(true);
            }
            if city.tile_bits_at(x, y) != Some(bits) {
                city.set_tile_bits(x, y, bits);
            }
        }
    }
}

fn power_adjacent_building_for_test(city: &mut City, x: usize, y: usize) {
    // Coal `0xC6` is a 1×1 fixed-capacity PE source. Wind `0xC8` reads XVAL&0x1f, so a
    // later `stamp_xval_for_test(XVAL_POPLEVEL_LOW)` (32) zeroes plant output.
    city.set_building(x, y, 198);
    stamp_power_plant_xbit_powerable(city, &[(x, y)]);
    // This helper models a saved powered source for candidate-isolation tests.
    // Full network/publisher behavior belongs in tests using
    // `powered_line_corridor_for_test` or the live day-1 worker.
    city.set_tile_bits(
        x,
        y,
        TileBits::new(0).with_powerable(true).with_powered(true),
    );
}

fn set_saved_powered_xbit_for_test(city: &mut City, x: usize, y: usize) {
    let bits = city.tile_bits_at(x, y).unwrap_or_default();
    city.set_tile_bits(x, y, bits.with_powered(true));
}

fn powered_line_corridor_for_test(city: &mut City, x_start: usize, x_end: usize, y: usize) {
    assert!(x_start > 0 && x_end >= x_start && x_end < CITY_WIDTH && y < CITY_HEIGHT);
    let source_x = x_start - 1;
    city.set_building(source_x, y, 198);
    stamp_power_plant_xbit_powerable(city, &[(source_x, y)]);

    let mut state = test_state(city.clone());
    state.city.set_city_funds(1_000_000);
    let path: Vec<_> = (x_start..=x_end).map(|x| (x, y)).collect();
    assert_eq!(
        state.place_network_path(&path, NetworkKind::Power),
        path.len(),
        "fixture power corridor should place the full line"
    );
    *city = state.city;
}

fn set_saved_xbit_growth_power_for_test(
    power_maps: &mut PowerServiceMaps,
    x: usize,
    y: usize,
    powered: bool,
) {
    let index = y * CITY_WIDTH + x;
    power_maps.saved_xbit_powered_tiles[index] = powered;
    // Keep the geometry plane aligned for helpers that inspect it separately.
    power_maps.powered_tiles[index] = powered;
}

fn clear_saved_xbit_growth_power_for_test(power_maps: &mut PowerServiceMaps) {
    power_maps.saved_xbit_powered_tiles.fill(false);
    power_maps.powered_tiles.fill(false);
}

fn water_footprint_from_below_for_test(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    size: usize,
) {
    assert!(origin_x >= 1);
    assert!(size > 0);
    assert!(origin_y + size + 1 < CITY_HEIGHT);
    let pipe_x = origin_x;
    let pipe_y = origin_y + size;
    let pump_y = pipe_y + 1;
    city.set_building(pipe_x, pump_y, 220);
    power_water_source_for_test(city, pipe_x, pump_y);
    city.set_underground(
        pipe_x,
        pipe_y,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_TOP | NETWORK_BOTTOM),
    );
}

fn tile_bits_snapshot(city: &City) -> Vec<Option<u8>> {
    (0..CITY_TILES)
        .map(|index| {
            city.tile_bits_at(index % CITY_WIDTH, index / CITY_WIDTH)
                .map(|bits| bits.raw_bits())
        })
        .collect()
}

#[derive(Default)]
struct UndergroundRenderStats {
    user_pipe: usize,
    watered_user_pipe: usize,
    building_pipe: usize,
    watered_building_pipe: usize,
}

fn underground_render_stats(city: &City) -> UndergroundRenderStats {
    let mut stats = UndergroundRenderStats::default();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let Some(tile) = underground_render_tile(city, x, y, city.rotation()) else {
                continue;
            };
            match tile.source {
                UndergroundTileSource::UserLaid if underground_user_pipe_tile(tile.tile) => {
                    stats.user_pipe += 1;
                    if tile.watered {
                        stats.watered_user_pipe += 1;
                    }
                }
                UndergroundTileSource::BuildingInternal => {
                    stats.building_pipe += 1;
                    if tile.watered {
                        stats.watered_building_pipe += 1;
                    }
                }
                _ => {}
            }
        }
    }
    stats
}

fn collect_city_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_city_files(&path, out);
            continue;
        }
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };
        if extension.eq_ignore_ascii_case("sc2") || extension.eq_ignore_ascii_case("scn") {
            out.push(path);
        }
    }
}

fn available_city_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(catalog) = paths::test_win_catalog() {
        collect_city_files(&catalog, &mut files);
    }
    if let Some(root) = paths::test_win95_root() {
        collect_city_files(&root, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestMouseInputRoute {
    Query,
    ChooserDialog,
    Drag,
    DirectClick,
}

fn test_mouse_input_route_for_tool(tool: EditTool) -> Option<TestMouseInputRoute> {
    if tool == EditTool::Query {
        return Some(TestMouseInputRoute::Query);
    }
    if is_chooser_menu_tool(tool) {
        return Some(TestMouseInputRoute::ChooserDialog);
    }
    if zone_id_for_tool(tool).is_some()
        || network_kind_for_tool(tool).is_some()
        || underground_kind_for_tool(tool).is_some()
        || matches!(
            tool,
            EditTool::Highway
                | EditTool::RaiseTerrain
                | EditTool::LowerTerrain
                | EditTool::LevelTerrain
                | EditTool::StretchTerrain
                | EditTool::SpreadTerrain
                | EditTool::Trees
                | EditTool::Water
        )
    {
        return Some(TestMouseInputRoute::Drag);
    }
    if placeable_building_for_tool(tool).is_some()
        || dispatch_kind_for_tool(tool).is_some()
        || matches!(
            tool,
            EditTool::Bulldoze
                | EditTool::Sign
                | EditTool::Tunnel
                | EditTool::Onramp
                | EditTool::SubToRail
                | EditTool::Forest
        )
    {
        return Some(TestMouseInputRoute::DirectClick);
    }
    None
}

fn expected_tool_preview_for_fixture(
    behavior: CityToolBehavior,
    state: &ViewerState,
    tile: (usize, usize),
) -> Option<ToolPreview> {
    match behavior {
        CityToolBehavior::Query
        | CityToolBehavior::ChooserDialog
        | CityToolBehavior::EditMapOnly => None,
        CityToolBehavior::Bulldoze => Some(ToolPreview::Bulldoze),
        CityToolBehavior::TerrainEdit
        | CityToolBehavior::SurfaceWater
        | CityToolBehavior::Dispatch(_) => Some(ToolPreview::Terrain),
        CityToolBehavior::Trees | CityToolBehavior::Forest => Some(ToolPreview::Building(6)),
        CityToolBehavior::Sign => Some(ToolPreview::Zone(0)),
        CityToolBehavior::Network(kind) => Some(ToolPreview::Network(kind)),
        CityToolBehavior::Highway => Some(ToolPreview::Highway),
        CityToolBehavior::Underground(kind) => Some(ToolPreview::Underground(kind)),
        CityToolBehavior::Zone(zone_id) => Some(ToolPreview::Zone(zone_id)),
        CityToolBehavior::DirectBuilding(building) => Some(ToolPreview::Building(building)),
        CityToolBehavior::HydroPower => Some(ToolPreview::Building(pe_hydro_building_id_441610(
            &state.city,
            tile.0,
            tile.1,
        ))),
        CityToolBehavior::Tunnel => {
            let plan = state
                .tunnel_plan_at(tile.0, tile.1)
                .expect("fixture should provide a tunnel plan");
            Some(ToolPreview::Building(plan.entrance_building))
        }
        CityToolBehavior::Onramp => {
            let plan = state
                .onramp_plan_at(tile.0, tile.1)
                .expect("fixture should provide an onramp plan");
            Some(ToolPreview::Building(plan.building))
        }
        CityToolBehavior::SubToRail => {
            let plan = state
                .sub_to_rail_plan_at(tile.0, tile.1)
                .expect("fixture should provide a subway-to-rail plan");
            Some(ToolPreview::Building(plan.building))
        }
    }
}

fn expected_tool_status_line(tool: EditTool, behavior: CityToolBehavior) -> Option<String> {
    match behavior {
        CityToolBehavior::Network(kind) => {
            Some(format_tool_cost_status(kind.label(), kind.cost_per_tile()?))
        }
        CityToolBehavior::Underground(kind) => {
            Some(format_tool_cost_status(kind.label(), kind.cost_per_tile()))
        }
        CityToolBehavior::Highway => {
            Some(format_tool_cost_status("Highway", HIGHWAY_COST_PER_SEGMENT))
        }
        _ => tool_build_cost(tool).map(|cost| format_tool_cost_status(tool.label(), cost)),
    }
}

fn expected_success_placement_sound(
    tool: EditTool,
    behavior: CityToolBehavior,
) -> Option<PlacementSound> {
    match behavior {
        CityToolBehavior::Zone(_) => Some(zone_placement_sound(tool)),
        CityToolBehavior::TerrainEdit | CityToolBehavior::Highway => Some(PlacementSound::Build),
        CityToolBehavior::Network(kind) => Some(network_placement_sound(kind)),
        CityToolBehavior::Underground(kind) => Some(underground_placement_sound(kind)),
        CityToolBehavior::Trees | CityToolBehavior::Forest => Some(PlacementSound::Plop),
        CityToolBehavior::DirectBuilding(_)
        | CityToolBehavior::HydroPower
        | CityToolBehavior::Tunnel
        | CityToolBehavior::Onramp
        | CityToolBehavior::SubToRail
        | CityToolBehavior::Dispatch(_) => one_click_placement_sound(tool),
        CityToolBehavior::Query
        | CityToolBehavior::ChooserDialog
        | CityToolBehavior::EditMapOnly
        | CityToolBehavior::Bulldoze
        | CityToolBehavior::Sign => None,
        CityToolBehavior::SurfaceWater => Some(PlacementSound::Flood),
    }
}

fn fill_all_usable_xthg_slots(city: &mut City) {
    let mut n = 0usize;
    while pe_xthg_disaster_slot_available(city) {
        let x = n % CITY_WIDTH;
        let y = (n / CITY_WIDTH) % CITY_HEIGHT;
        city.place_pe_explosion_actor(x, y, 0, 0, 1)
            .expect("fill usable XTHG slot");
        n += 1;
        assert!(n < 64, "XTHG usable slots should fill before 64 placements");
    }
}

// A native disaster continuation is separate from the 25 calendar phases.
fn finish_pe_cloud_disaster_for_test(state: &mut ViewerState) {
    persist_pe_disaster_misc(
        &mut state.city,
        Some(DisasterKind::ToxicSpill),
        state.mode,
        state.engine_fidelity,
    );
    state.traffic.set_speed(GameSpeed::Llama);
    let before = state.game_clock;
    for _ in 0..2048 {
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(200))
                .days,
            0
        );
        assert_eq!(state.game_clock, before);
        if !state.city.has_active_disaster() {
            return;
        }
    }
    panic!("fixed-seed native cloud continuation did not finish");
}

fn reward_unlock_test_city() -> City {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for y in 40..50 {
        for x in 40..50 {
            city.set_building(x, y, 0x70);
        }
    }
    stamp_complete_building_for_test(&mut city, 30, 30, 210, 3);
    stamp_complete_building_for_test(&mut city, 33, 30, 211, 3);
    stamp_complete_building_for_test(&mut city, 30, 33, 209, 3);
    stamp_complete_building_for_test(&mut city, 33, 33, 214, 3);
    stamp_complete_building_for_test(&mut city, 30, 36, 217, 4);
    stamp_complete_building_for_test(&mut city, 34, 36, 245, 2);
    stamp_complete_building_for_test(&mut city, 36, 36, 212, 3);
    stamp_complete_building_for_test(&mut city, 35, 39, 213, 3);
    stamp_complete_building_for_test(&mut city, 38, 39, 13, 1);
    stamp_complete_building_for_test(&mut city, 38, 33, 237, 2);
    city
}

fn water_consumer_fixture_for_calendar_test() -> ViewerState {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.set_building(10, 10, 220);
    // The monthly fixture runs PE day-1 power before water day 20. Keep the
    // adjacent pump in the saved powerable flood so that pass restores XBIT 0x40.
    state.city.set_tile_bits(
        10,
        10,
        state
            .city
            .tile_bits_at(10, 10)
            .unwrap_or_default()
            .with_powerable(true),
    );
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.city.set_zone(11, 10, 1);
    state.city.set_building(11, 10, 112);
    state
        .city
        .set_tile_bits(11, 10, crate::city::TileBits::new(0));
    state
}

fn pe_monthly_condition_cover_through_school(state: &mut ViewerState) {
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
}

fn pop1_size_up_state_at_for_test(origin: (usize, usize)) -> ViewerState {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let existing = *RESIDENTIAL_1X1_BUILDINGS.last().unwrap();
    for (x, y) in building_footprint_from_origin(origin.0, origin.1, 2).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, existing);
    }
    stamp_transport_access_for_test(&mut city, origin.0.saturating_sub(1), origin.1 + 1);
    power_adjacent_building_for_test(&mut city, origin.0, origin.1.saturating_sub(1));
    water_footprint_from_below_for_test(&mut city, origin.0, origin.1, 2);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    state
}

fn pop1_quadrant_size_up_candidate_for_test(
    zone_origin: (usize, usize),
    one_by_one: (usize, usize),
) -> Option<RciDevelopmentCandidate> {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(zone_origin.0, zone_origin.1, 2).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    city.set_building(one_by_one.0, one_by_one.1, RESIDENTIAL_1X1_BUILDINGS[0]);
    city.set_tile_bits(
        one_by_one.0,
        one_by_one.1,
        TileBits::new(XBIT_CHOOSE_AND_PLACE),
    );
    stamp_transport_access_for_test(&mut city, zone_origin.0 - 1, zone_origin.1 + 1);
    // StartBuilding checks power at the occupied 1x1 tile, not the selected
    // 2x2 quadrant's northwest corner. Keep the source outside that candidate
    // footprint while placing it cardinally beside the visited tile.
    let power_source = if zone_origin.0 < one_by_one.0 {
        (one_by_one.0 + 1, one_by_one.1)
    } else {
        (one_by_one.0, one_by_one.1 + 1)
    };
    power_adjacent_building_for_test(&mut city, power_source.0, power_source.1);
    water_footprint_from_below_for_test(&mut city, zone_origin.0, zone_origin.1, 2);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let reserved = vec![false; CITY_TILES];
    let power_maps = power_service_maps(&state.city);
    state.rci_expansion_redevelopment_candidate_at(
        one_by_one.0,
        one_by_one.1,
        &reserved,
        &power_maps,
        city_service_outcomes_for_city(&state.city),
        &rci_development_services_for_city(&state.city),
        None,
    )
}

fn pop3_size_up_state_for_test(extra_building: Option<((usize, usize), u8)>) -> ViewerState {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_complete_building_for_test(&mut city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[4], 2);
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    if let Some(((x, y), building)) = extra_building {
        city.set_building(x, y, building);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    stamp_pop3_road_orientation_for_test(&mut city, 20, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    state
}

fn pop3_quadrant_size_up_candidate_for_test(
    zone_origin: (usize, usize),
    two_by_two: (usize, usize),
) -> Option<RciDevelopmentCandidate> {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(zone_origin.0, zone_origin.1, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
    }
    stamp_complete_building_for_test(
        &mut city,
        two_by_two.0,
        two_by_two.1,
        RESIDENTIAL_2X2_BUILDINGS[4],
        2,
    );
    or_rci_building_zone_corners_for_test(&mut city, two_by_two, 2);
    stamp_transport_access_for_test(&mut city, zone_origin.0 - 1, zone_origin.1 + 1);
    stamp_pop3_road_orientation_for_test(&mut city, zone_origin.0, zone_origin.1);
    let visited = rci_xzon_view_corner_in_footprint_for_test(&city, two_by_two, 2);
    // This regression isolates quadrant order from network construction. Model
    // the saved PE-powered marker on the actual visited view-corner cell.
    let bits = city.tile_bits_at(visited.0, visited.1).unwrap_or_default();
    city.set_tile_bits(visited.0, visited.1, bits.with_powered(true));
    water_footprint_from_below_for_test(&mut city, zone_origin.0, zone_origin.1, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_HIGH);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    rci_expansion_from_2x2_view_corner_for_test(&mut state, two_by_two)
}

fn expected_special_zone_development_plans(state: &ViewerState) -> Vec<ZoneDevelopmentPlan> {
    // Mirror `develop_special_zones`: one shared `pe_process_crt` stream across phases
    // (PE DoArmyBaseGrowth / DoAirPortGrowth / DoSeaPortGrowth `rand() & 3` gate).
    let mut plans = Vec::new();
    let mut reserved = vec![false; CITY_TILES];
    let mut rng = state.pe_process_crt;
    for (step, substep) in sim_calendar_growth_phases() {
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            if reserved[y * CITY_WIDTH + x] {
                continue;
            }
            let Some(zone_id) = state.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
                continue;
            };
            if zone_id <= ZONE_INDUSTRIAL_DENSE
                || special_zone_kind_index(zone_id).is_none()
                || !state.special_zone_growth_roll_allows(&mut rng, zone_id)
            {
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
            plans.push(plan);
        }
    }
    plans
}

/// Force a process-CRT seed where special-zone `rand() & 3` admits at least one plan.
fn seed_pe_process_crt_until_special_zone_develops(state: &mut ViewerState) {
    for seed in 0..256u32 {
        state.pe_process_crt = Win95CrtRand::new(seed);
        if expected_special_zone_development_count(state) > 0 {
            return;
        }
    }
    panic!("no pe_process_crt seed admitted special-zone growth");
}

/// Choose a process-CRT seed whose PE special-zone rolls admit these exact plans.
fn seed_special_zone_targets_for_test(state: &mut ViewerState, targets: &[((usize, usize), u8)]) {
    let mut observed = std::collections::BTreeSet::new();
    for seed in 0..4096u32 {
        state.pe_process_crt = Win95CrtRand::new(seed);
        let plans = expected_special_zone_development_plans(state);
        observed.extend(plans.iter().map(|plan| (plan.origin, plan.building)));
        if targets.iter().all(|&(origin, building)| {
            plans
                .iter()
                .any(|plan| plan.origin == origin && plan.building == building)
        }) {
            return;
        }
    }
    panic!("no process-CRT seed admitted special-zone plans {targets:?}; observed {observed:?}");
}

fn expected_special_zone_development_count(state: &ViewerState) -> usize {
    expected_special_zone_development_plans(state)
        .iter()
        .map(|plan| plan.footprint.len())
        .sum()
}

fn city_building_zone_snapshot(city: &City) -> Vec<(u8, u8)> {
    let mut snapshot = Vec::with_capacity(CITY_TILES);
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            snapshot.push((
                city.building_at(x, y),
                city.zone_at(x, y).map_or(0, |zone| zone.raw_bits()),
            ));
        }
    }
    snapshot
}

fn assert_developed_buildings_match_zone_kinds(city: &City) {
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let building = city.building_at(x, y);
            if building == 0 || is_tree_building(building) {
                continue;
            }

            let zone_id = city.zone_at(x, y).map_or(0, |zone| zone.zone_id());
            if let Some(kind) = rci_building_kind(building) {
                assert_eq!(
                    rci_kind_for_zone(zone_id),
                    Some(kind),
                    "RCI building {building} at ({x},{y}) has zone {zone_id}"
                );
                continue;
            }

            if special_zone_developed_building(ZONE_AIRPORT, building) {
                assert_eq!(
                    zone_id, ZONE_AIRPORT,
                    "airport building {building} at ({x},{y}) has zone {zone_id}"
                );
            } else if special_zone_developed_building(ZONE_SEAPORT, building) {
                assert_eq!(
                    zone_id, ZONE_SEAPORT,
                    "seaport building {building} at ({x},{y}) has zone {zone_id}"
                );
            }
        }
    }
}

fn flatten_road_walk_fixture(city: &mut City, x0: usize, x1: usize, y0: usize, y1: usize) {
    for x in x0..=x1 {
        for y in y0..=y1 {
            city.set_terrain(x, y, 0);
            city.set_altitude(x, y, 5, 5);
        }
    }
}

// Subsystem test modules share the fixtures above.
mod app_state;
mod city_data;
mod city_tools;
mod economy_growth;
#[path = "tests/mac_cgms_rust_scan_probe.rs"]
mod mac_cgms_rust_scan_probe;
#[path = "tests/mac_cgms_phase2_probe.rs"]
mod mac_cgms_phase2_probe;
#[path = "tests/mac_cgms_phase1_probe.rs"]
mod mac_cgms_phase1_probe;
mod map_interaction;
mod scenario_disaster;
mod terrain_rendering;
mod traffic_transit;
mod ui_commands;
