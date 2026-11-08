use super::*;

#[test]
fn env_flag_enabled_parses_expected_values() {
    const ENV_NAME: &str = "SC2000_TEST_TRAFFIC_ENV_FLAG";
    assert!(!env_flag_value(ENV_NAME, None));
    assert!(env_flag_value(ENV_NAME, Some("1")));
    assert!(env_flag_value(ENV_NAME, Some(" true ")));
    assert!(env_flag_value(ENV_NAME, Some("ON")));
    assert!(env_flag_value(ENV_NAME, Some("yes")));
    assert!(env_flag_value(ENV_NAME, Some("Y")));
    assert!(!env_flag_value(ENV_NAME, Some("0")));
    assert!(!env_flag_value(ENV_NAME, Some("FALSE")));
    assert!(!env_flag_value(ENV_NAME, Some("nope")));
}

#[test]
fn mac_live_timer_uses_native_ticks_and_strict_future_deadline() {
    for (speed, interval_ms) in [(GameSpeed::Turtle, 600), (GameSpeed::Llama, 200)] {
        let city = blank_city_layout_fixture("mac-live-deadline.city", "Mac deadline");
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        state.game_clock = GameClock::new(1900, 1);
        state.traffic.set_speed(speed);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            1
        );
        // now == deadline is not due; the next TickCount change admits one.
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(interval_ms))
                .days,
            0
        );
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            1
        );
        assert_eq!(state.game_clock.day(), 2);
    }
}

#[test]
fn mac_live_timer_delayed_delivery_is_one_phase_without_catchup() {
    let city = blank_city_layout_fixture("mac-live-delayed.city", "Mac delayed");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state
        .city
        .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
    state.game_clock = GameClock::new(1900, 1);
    state.traffic.set_speed(GameSpeed::Cheetah);
    let traffic = state.city.traffic_density_slice().unwrap().to_vec();
    let redraw = (state.traffic_redraw_epoch, state.traffic_redraw_accumulator);
    assert_eq!(
        state.advance_frame_simulation(Duration::from_secs(1)).days,
        1
    );
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
    assert_eq!(
        state.advance_frame_simulation(Duration::from_nanos(1)).days,
        0
    );
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(17))
            .days,
        1
    );
    assert_eq!(state.game_clock.day(), 2);
    assert_eq!(state.city.traffic_density_slice().unwrap(), traffic);
    assert_eq!(
        (state.traffic_redraw_epoch, state.traffic_redraw_accumulator),
        redraw
    );
}

#[test]
fn mac_live_timer_uses_saved_run_state_on_both_city_layouts() {
    for (extension, kind) in [
        ("sc2", CityFileKind::WinSc2),
        ("city", CityFileKind::MacCity),
    ] {
        for (run_state, expected_days) in [(0, 0), (1, 1), (2, 0)] {
            let path = std::env::temp_dir().join(format!(
                "mac-run-state-cross-layout-{extension}-{run_state}.{extension}"
            ));
            let mut city = blank_city_layout_fixture(&path, "Mac run state");
            assert_eq!(city.kind, kind);
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, run_state);
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.game_clock = GameClock::new(1900, 1);
            state.traffic.set_speed(GameSpeed::Cheetah);

            assert_eq!(
                state.advance_frame_simulation(Duration::from_secs(1)).days,
                expected_days,
                "layout={kind:?}, run_state={run_state}"
            );
        }
    }
}

#[test]
fn mac_live_timer_tickcount_advances_during_pause_and_editor() {
    for editor in [false, true] {
        let city = blank_city_layout_fixture("mac-live-held.city", "Mac held");
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        state.game_clock = GameClock::new(1900, 1);
        state.traffic.set_speed(if editor {
            GameSpeed::Turtle
        } else {
            GameSpeed::Paused
        });
        if editor {
            state.mode = AppMode::EditNewMap;
        }
        assert_eq!(
            state.advance_frame_simulation(Duration::from_secs(1)).days,
            0
        );
        state.mode = AppMode::CityView;
        state.traffic.set_speed(GameSpeed::Turtle);
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
    }
}

#[test]
fn new_blank_option_defaults_and_explicit_disable_persistence() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
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
    assert!(
        city.misc_u32_at(MISC_DIFFICULTY_OFFSET).unwrap_or(0) == 0,
        "blank city should not yet have a playable difficulty"
    );

    ensure_default_city_options(&mut city);
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
    assert_eq!(
        city.misc_u32_at(MISC_DIFFICULTY_OFFSET),
        Some(CITY_DIFFICULTY_MEDIUM)
    );

    assert!(set_city_sound_effects_enabled(&mut city, false));
    ensure_default_city_options(&mut city);
    assert_eq!(
        city.misc_u32_at(SOUND_EFFECTS_MISC_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
}

#[test]
fn mac_scan_acceptance_class_zero_exits_before_generic_draw() {
    let mut city = blank_city_layout_fixture("mac-scan-class-zero.city", "class zero return");
    city.set_zone(40, 40, 0x82);
    city.set_building(40, 40, 0x70);
    stamp_xval_for_test(&mut city, 255); // Ensure an incorrect fallthrough reaches its generic draw.
    city.set_traffic_density(20, 20, 255);
    let traffic = city.traffic_density_slice().unwrap().to_vec();
    let seed = (1..100_000)
        .find(|seed| MacToolboxRng::new(*seed).random_u16() < 4000)
        .unwrap();
    let mut expected = MacToolboxRng::new(seed);
    for _ in 0..3 {
        expected.random_i16();
    } // Admission, orientation, one export-242 selection.
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);
    assert_eq!(
        rng.seed(),
        expected.seed(),
        "accepted class 0 jumps directly to the tile tail"
    );
    assert_eq!(city.traffic_density_slice().unwrap(), traffic);
}

#[test]
fn mac_scan_acceptance_class_one_exits_before_generic_draw() {
    let mut city = blank_city_layout_fixture("mac-scan-class-one.city", "class one return");
    city.set_zone(40, 40, 0x84);
    city.set_building(40, 40, 0xa6);
    stamp_xval_for_test(&mut city, 255);
    city.set_traffic_density(20, 20, 255);
    let traffic = city.traffic_density_slice().unwrap().to_vec();
    let seed = (1..100_000)
        .find(|seed| MacToolboxRng::new(*seed).random_u16() < 8192)
        .unwrap();
    let mut expected = MacToolboxRng::new(seed);
    for _ in 0..2 {
        expected.random_i16();
    } // Admission and one pattern selection.
    let mut rng = MacToolboxRng::new(seed);
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);
    assert_eq!(
        rng.seed(),
        expected.seed(),
        "accepted class 1 jumps directly to the tile tail"
    );
    assert_eq!(city.traffic_density_slice().unwrap(), traffic);
}

#[test]
fn mac_scan_acceptance_zero_table_coefficient_still_reaches_generic_probe() {
    let mut city = blank_city_layout_fixture("mac-scan-zero-coefficient.city", "zero coefficient");
    city.set_zone(40, 40, 0x81);
    city.set_building(40, 40, 0xc6);
    assert_eq!(mac_growth_coefficient_for_xbld(0xc6), 0);
    let mut rng = MacToolboxRng::new(12345);
    let mut expected = rng;
    expected.random_i16(); // Zero local_c rejects after the generic draw, not before it.
    mac_budget_accumulate_monthly_rci_scan_partition(&mut city, 0, 0, &mut rng);
    assert_eq!(city.building_at(40, 40), 0xc6);
    assert_eq!(rng.seed(), expected.seed());
}

#[test]
fn pe_selected_height_keeps_signed_word_comparisons_and_packed_high_bits() {
    for extension in ["sc2", "city"] {
        for (word, expected_water, expected_tunnel, wet) in [
            (0xffff, 9, 33, false),
            (0x0104, 4, 41, true),
            (0x7fff, 31, 63, true),
        ] {
            for slope32 in [false, true] {
                let mut city = blank_city_layout_fixture(
                    format!("/tmp/pe-selected-word.{extension}"),
                    "SelectedWord",
                );
                for x in 19..=21 {
                    for y in 19..=21 {
                        city.set_altitude(x, y, if slope32 { 8 } else { 4 }, 0);
                    }
                }
                city.set_mac_state_misc_u32_at(0x0e40, 0xabcd_0000 | word);
                city.set_altm_word(20, 20, 0x8524); // land 4, water 9, tunnel 33
                city.set_building(20, 20, 5); // native radioactivity survives wet clearing
                city.set_tile_bits(20, 20, TileBits::new(0xa5));
                city.set_traffic_density(20, 20, 0x63);
                let sea = pe_selected_water_height_for_fun_00447020(&city);
                pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, sea);
                assert_eq!(
                    city.water_height_at(20, 20),
                    expected_water,
                    "layout={extension} word={word:#x} slope32={slope32}"
                );
                assert_eq!(city.altm_at(20, 20).tunnel_levels, expected_tunnel);
                assert_eq!(city.altitude_at(20, 20), if slope32 { 5 } else { 4 });
                assert_eq!(city.terrain_at(20, 20), if wet { 0x10 } else { 0 });
                assert_eq!(city.tile_bits_at(20, 20).unwrap().water_tile(), wet);
                assert_eq!(city.building_at(20, 20), 5);
                assert_eq!(city.traffic_density_at(20, 20), Some(0x63));
                assert_eq!(city.mac_state_misc_u32_at(0x0e40), Some(0xabcd_0000 | word));
            }
        }
    }
}

#[test]
fn stretch_crt_445b50_uses_click_origin_not_jitter_tile() {
    // PE @ `0x443b94`: `0x445b50` uses tool click origin, not the CRT-jittered tile.
    let mut city =
        City::new_blank("/tmp/sc2000-stretch-445b50-origin.sc2", "Stretch445").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    // Cliff NESW of click origin — pulled by origin-scoped `0x445b50`.
    city.set_altitude(20, 19, 2, 0);
    let mut crt = Win95CrtRand::new(12); // dx=1,dy=0 → land++ at (21,20)
    assert!(
        pe_stretch_terrain_hill_step_443a50_with_crt(&mut city, Some(&mut crt), 20, 20).is_some()
    );
    assert_eq!(
        city.altitude_at(20, 20),
        8,
        "click origin is not the land++ tile"
    );
    assert!(city.altitude_at(21, 20) > 8, "CRT-jittered tile raised");
    assert!(
        city.altitude_at(20, 19) > 2,
        "origin 445b50 must pull north cliff (got {})",
        city.altitude_at(20, 19)
    );
}

#[test]
fn pe_stretch_from_packed_443a50_refuses_negative_packed() {
    // PE Stretch packed path shares Lower `0x444bf0` unpack refuse on negative AX.
    let mut city =
        City::new_blank("/tmp/sc2000-stretch-pack-ref.sc2", "StretchPackRef").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 8, 0);
        }
    }
    let mut crt = Win95CrtRand::new(1);
    assert!(pe_stretch_terrain_from_packed_443a50(&mut city, &mut crt, -1).is_none());
    assert_eq!(city.altitude_at(20, 20), 8);
}

#[test]
fn pe_stretch_from_packed_443a50_unpacks_before_jitter() {
    // PE Stretch `0x443ab1`: `& 0x7f` unpack of `0x444bf0` before CRT jitter hill++.
    let mut city =
        City::new_blank("/tmp/sc2000-stretch-packed.sc2", "StretchPacked").expect("blank");
    for y in 0..128 {
        for x in 0..128 {
            city.set_altitude(x, y, 9, 0);
            city.set_terrain(x, y, 0);
        }
    }
    assert_eq!(pe_lower_unpack_tile_444bf0(0x1490), Some((0x10, 0x14)));
    let mut crt = Win95CrtRand::new(1);
    assert!(pe_stretch_terrain_from_packed_443a50(&mut city, &mut crt, 0x1490).is_some());
    // Jitter ±3 around unpacked `(0x10,0x14)`; 447490 may also touch neighbors.
    let mut found = false;
    for y in 0x11..=0x17 {
        for x in 0x0d..=0x13 {
            if city.altitude_at(x, y) > 9 {
                found = true;
            }
        }
    }
    assert!(found, "hill++ near unpacked tile");
    assert_eq!(pe_raise_unpack_tile_444bf0(0x1490), None);
}

#[test]
fn pe_stretch_hold_ui_refresh_due_443ba6_every_four_successes() {
    // PE Stretch hold @ `0x443ba6`: success path adds 2 to `si`; refresh when `si & 7 == 0`.
    assert!(!pe_stretch_hold_ui_refresh_due_443ba6(2));
    assert!(!pe_stretch_hold_ui_refresh_due_443ba6(4));
    assert!(!pe_stretch_hold_ui_refresh_due_443ba6(6));
    assert!(pe_stretch_hold_ui_refresh_due_443ba6(8));
    let mut si = 0u16;
    assert!(!pe_stretch_hold_after_successful_hill_443b88(&mut si));
    assert_eq!(si, 2);
    assert!(!pe_stretch_hold_after_successful_hill_443b88(&mut si));
    assert!(!pe_stretch_hold_after_successful_hill_443b88(&mut si));
    assert!(pe_stretch_hold_after_successful_hill_443b88(&mut si));
    assert_eq!(si, 8);
}

#[test]
fn pe_spread_from_packed_442a30_refuses_negative_packed() {
    // PE tool-4 packed path: negative `0x444bf0` AX → skip NESW copy `(0,0)`.
    let mut city =
        City::new_blank("/tmp/sc2000-spread-pack-ref.sc2", "SpreadPackRef").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(20, 20, 12, 0);
    assert_eq!(
        pe_spread_terrain_from_packed_442a30(&mut city, -1, 19, 20),
        (0, 0)
    );
    assert_eq!(city.altitude_at(21, 20), 9);
}

#[test]
fn pe_stretch_hill_crt_oob_jitter_refuses() {
    // PE Stretch `0x443a50`: out-of-bounds CRT jitter tile → no land++.
    let mut city = City::new_blank("/tmp/sc2000-stretch-oob.sc2", "StretchOob").expect("blank");
    for y in 0..5 {
        for x in 0..5 {
            city.set_altitude(x, y, 5, 0);
        }
    }
    // Seed 1 → dx=-2,dy=2; from (0,0) X is OOB.
    let mut crt = Win95CrtRand::new(1);
    assert!(
        pe_stretch_terrain_hill_step_443a50_with_crt(&mut city, Some(&mut crt), 0, 0).is_none()
    );
    assert_eq!(city.altitude_at(0, 0), 5);
}

#[test]
fn microwave_auto_goto_centers_on_last_periodic_walk_tile_not_hover() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    assert_eq!(pe_microwave_plant_tile(&city), Some((40, 40)));
    assert_eq!(PE_MICROWAVE_WALK_STEPS, 0x27);
    assert_eq!(PE_MICROWAVE_GOTO_PERIOD, 10);
    let mut state = test_state(city);
    state.hover_tile = Some((12, 34));
    if !state.auto_goto_enabled {
        state.toggle_auto_goto();
    }
    let mut preview_city = state.city.clone();
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let goto = pe_microwave_walk_with_rng(&mut preview_city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .periodic_goto;
    assert_ne!(goto, (12, 34));

    let effect = state
        .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Microwave))
        .expect("state command");
    assert_eq!(effect.auto_goto_tile, Some(goto));
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
}

#[test]
fn pe_microwave_periodic_goto_uses_remaining_bx_div_10() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    let mut rng = Win95CrtRand::new(pe_microwave_walk_seed(&city, 40, 40));
    let mut dir = pe_microwave_walk_dir(&mut rng);
    let mut x = 40usize;
    let mut y = 40usize;
    let mut remaining = PE_MICROWAVE_WALK_STEPS;
    let mut periodic = Vec::new();
    while remaining > 0 {
        if city.building_at(x, y) != MICROWAVE_POWER_BUILDING_ID
            && remaining % PE_MICROWAVE_GOTO_PERIOD == 0
        {
            periodic.push((x, y, remaining));
        }
        let (dx, dy) = pe_microwave_walk_step(dir);
        let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
            break;
        };
        x = nx;
        y = ny;
        dir = pe_microwave_walk_dir(&mut rng);
        remaining -= 1;
    }
    assert!(
        periodic
            .iter()
            .all(|&(gx, gy, left)| left % PE_MICROWAVE_GOTO_PERIOD == 0
                && [30, 20, 10].contains(&left)
                && city.building_at(gx, gy) != MICROWAVE_POWER_BUILDING_ID)
    );
    assert_eq!(
        pe_microwave_periodic_goto_tile(&city),
        periodic
            .last()
            .map(|&(gx, gy, _)| (gx, gy))
            .or(Some((40, 40)))
    );
}

#[test]
fn pe_microwave_walk_dir_is_rand_and_7() {
    let mut rng = Win95CrtRand::new(0x45F5_C001);
    let mut expected = Win95CrtRand::new(0x45F5_C001);
    assert_eq!(
        pe_microwave_walk_dir(&mut rng),
        usize::from(expected.random_u16() & 7)
    );
    assert_eq!(pe_microwave_walk_step(0), (0, -1));
    assert_eq!(pe_microwave_walk_step(1), (1, -1));
    assert_eq!(pe_microwave_walk_step(2), (1, 0));
    assert_eq!(pe_microwave_walk_step(3), (1, 1));
    assert_eq!(pe_microwave_walk_step(4), (0, 1));
    assert_eq!(pe_microwave_walk_step(5), (-1, 1));
    assert_eq!(pe_microwave_walk_step(6), (-1, 0));
    assert_eq!(pe_microwave_walk_step(7), (-1, -1));
    assert_eq!(PE_MICROWAVE_WALK_STEPS, 0x27);
}

#[test]
fn pe_microwave_walk_skips_land_when_xtxt_not_igniteable() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    let mut preview = Win95CrtRand::new(1);
    let mut preview_lfsr = Win95Lfsr::from_u32_seed(preview.seed());
    let mut dry_city = city.clone();
    let dry = pe_microwave_walk_with_rng(&mut dry_city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .overlays;
    let probe = dry
        .iter()
        .find_map(|(&(x, y), &marker)| {
            (marker == 0xff && (x < 40 || x > 43 || y < 40 || y > 43)).then_some((x, y))
        })
        .expect("land walk tile outside plant");
    city.set_text_overlay(probe.0, probe.1, 0xc9);
    let mut preview = Win95CrtRand::new(1);
    let mut preview_lfsr = Win95Lfsr::from_u32_seed(preview.seed());
    let blocked = pe_microwave_walk_with_rng(&mut city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .overlays;
    assert!(!blocked.contains_key(&probe));
    assert_eq!(city.text_overlay_at(probe.0, probe.1), Some(0xc9));
}

#[test]
fn pe_microwave_walk_skips_land_on_high_xtxt_rubble_band() {
    // PE FUN_00460e10 rubble band leaves XTXT; microwave walk must not invent fire paint.
    let mut city = City::new_blank("/tmp/sc2000-mw-rubble.sc2", "MwRubble").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, MICROWAVE_POWER_BUILDING_ID, 4);
    let mut preview = Win95CrtRand::new(1);
    let mut preview_lfsr = Win95Lfsr::from_u32_seed(preview.seed());
    let mut dry_city = city.clone();
    let dry = pe_microwave_walk_with_rng(&mut dry_city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .overlays;
    let probe = dry
        .iter()
        .find_map(|(&(x, y), &marker)| {
            (marker == 0xff && (x < 40 || x > 43 || y < 40 || y > 43)).then_some((x, y))
        })
        .expect("land walk tile outside plant");
    city.set_text_overlay(probe.0, probe.1, 0xf1);
    let mut preview = Win95CrtRand::new(1);
    let mut preview_lfsr = Win95Lfsr::from_u32_seed(preview.seed());
    let blocked = pe_microwave_walk_with_rng(&mut city, &mut preview, &mut preview_lfsr)
        .expect("microwave walk")
        .overlays;
    assert!(!blocked.contains_key(&probe));
    assert_eq!(city.text_overlay_at(probe.0, probe.1), Some(0xf1));
}

#[test]
fn mac_mod7e_plus_one_zero_extends_random_word_before_modulo() {
    // These call sites clear D1 and then write the trap's word into D1w before
    // `divsl`, so a negative signed QuickDraw result is still a positive u16.
    let seed = (1..10_000_u32)
        .find(|&seed| {
            let mut probe = MacToolboxRng::new(seed);
            let value = probe.random_i16();
            value < 0
                && usize::from(value as u16) % 0x7e != (i32::from(value).rem_euclid(0x7e) as usize)
        })
        .expect("find a seed where unsigned and signed modulo differ");
    let mut rng = MacToolboxRng::new(seed);
    let mut expected = MacToolboxRng::new(seed);
    let tile = mac_random_interior_map_tile(&mut rng);
    let want = (
        usize::from(expected.random_u16()) % 0x7e + 1,
        usize::from(expected.random_u16()) % 0x7e + 1,
    );
    assert_eq!(tile, want);
    assert!((1..=0x7e).contains(&tile.0));
    assert!((1..=0x7e).contains(&tile.1));
}

#[test]
fn pe_clear_xtrf_microsim64_zeros_even_aligned_2x2_cell() {
    // FUN_00460e10 / pe_clear: one 64×64 microsim cell via even-aligned write.
    let mut city = City::new_blank("/tmp/sc2000-xtrf-clear.sc2", "XtrfClr").expect("blank");
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21), (22, 22)] {
        city.set_traffic_density(x, y, 80);
    }
    pe_clear_xtrf_microsim64(&mut city, 21, 21);
    assert_eq!(city.traffic_density_at(20, 20), Some(0));
    assert_eq!(city.traffic_density_at(21, 20), Some(0));
    assert_eq!(city.traffic_density_at(20, 21), Some(0));
    assert_eq!(city.traffic_density_at(21, 21), Some(0));
    assert_eq!(
        city.traffic_density_at(22, 22),
        Some(80),
        "neighbor microsim cell untouched"
    );
}

#[test]
fn pe_fun_00460e10_flag0_refuses_xbld_below_6() {
    // PE @ `0x460e8c`: XBLD `< 6` + flag 0 → return 0; flag 1 still paints.
    let mut city = City::new_blank("/tmp/sc2000-ignite-low.sc2", "IgniteLow").expect("blank");
    city.set_building(20, 20, 0); // vacant
    city.set_text_overlay(20, 20, 0);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(!pe_fun_00460e10_ignite(
        &mut city,
        Some(&mut crt),
        &mut lfsr,
        20,
        20,
        0
    ));
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
    assert!(pe_fun_00460e10_ignite(
        &mut city,
        Some(&mut crt),
        &mut lfsr,
        20,
        20,
        1
    ));
    assert_eq!(city.text_overlay_at(20, 20), Some(0xff));
}

#[test]
fn pe_fun_00460e10_refuses_existing_ff_without_repaint() {
    // PE @ `0x460f43`: XTXT `> 0xfa` returns 0 — do not count a second paint on `0xff`.
    let mut city = City::new_blank("/tmp/sc2000-ignite-ff.sc2", "IgniteFf").expect("blank");
    city.set_building(20, 20, 0x70);
    city.set_text_overlay(20, 20, 0xff);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(!pe_fun_00460e10_ignite(
        &mut city,
        Some(&mut crt),
        &mut lfsr,
        20,
        20,
        1
    ));
    assert_eq!(city.text_overlay_at(20, 20), Some(0xff));
}

#[test]
fn pe_fun_00460e10_ignite_fa_drops_4cc4d8_like_460f7d() {
    // PE @ `0x460f7d`: XTXT 0xfa on network XBLD → dec `0x4cc4d8`, paint 0xff.
    let mut city = City::new_blank("/tmp/sc2000-ignite-fa-net.sc2", "IgniteFaNet").expect("blank");
    city.set_building(20, 20, 0x1d);
    city.set_text_overlay(20, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
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
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 0);
}

#[test]
fn pe_fb_tick_1_in_64_recede_uses_lfsr_mod64() {
    // PE `0x401415` masks LFSR low 6 bits; CRT `& 0x3f` is a different stream.
    let seed = 0x45FF_9A01u32;
    assert_ne!(
        Win95Lfsr::from_u32_seed(seed).random_mod64(),
        Win95CrtRand::new(seed).random_u16() & 0x3f
    );
}

#[test]
fn pe_fb_tick_active_only_for_cloud_family_types() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!pe_fb_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x01);
    assert!(!pe_fb_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x04);
    assert!(pe_fb_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x09);
    assert!(pe_fb_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0b);
    assert!(pe_fb_tick_active(&city));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0f);
    assert!(pe_fb_tick_active(&city));
}

#[test]
fn pe_fb_tick_moves_cloud_downhill() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfb);
    let src_alt = city.altitude_at(40, 40) & 0x1f;
    // Lower an already-scanned west neighbor so the moved cloud is not
    // re-processed later in the same X/Y sweep (CRT can act twice).
    city.set_altitude(39, 40, src_alt.saturating_sub(1), 0);
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x0f);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_fb_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.moved > 0 {
        assert_eq!(expected.text_overlay_at(40, 40), Some(0));
        assert_eq!(expected.text_overlay_at(39, 40), Some(0xfb));
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
fn pe_fb_tick_rejects_xtxt_ge_0x33_neighbor() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(40, 40, 0xfb);
    let src_alt = city.altitude_at(40, 40) & 0x1f;
    city.set_altitude(41, 40, src_alt.saturating_sub(1), 0);
    city.set_text_overlay(41, 40, 0x33);
    assert_eq!(pe_fb_downhill_dir(&city, 40, 40), Some(1));
    city.set_misc_u32_at(MISC_CURRENT_DISASTER_TYPE_OFFSET, 0x04);
    let mut state = test_state(city);
    let mut preview = state.pe_process_crt;
    let mut preview_lfsr = state.pe_process_lfsr;
    let (expected, stats) = preview_pe_fb_tick(&state, &mut preview, &mut preview_lfsr);
    if stats.receded > 0 {
        assert_eq!(expected.text_overlay_at(40, 40), Some(0));
        assert_eq!(expected.text_overlay_at(41, 40), Some(0x33));
    }
    advance_pe_disaster_scan_preview(&state, &mut preview);
    state.tick_pe_disaster_overlays();
    assert_eq!(
        state.city.text_overlay_at(40, 40),
        expected.text_overlay_at(40, 40)
    );
    assert_eq!(
        state.city.text_overlay_at(41, 40),
        expected.text_overlay_at(41, 40)
    );
    assert_eq!(state.pe_process_crt_seed(), preview.seed());
    assert_eq!(state.pe_process_lfsr_state(), preview_lfsr.state());
}

#[test]
fn pe_fb_degrade_2x2_uses_abandon_not_invent_0x8c() {
    let mut city = City::new_blank("/tmp/sc2000-fb-2x2.sc2", "Fb2").expect("blank");
    stamp_complete_building_for_test(&mut city, 40, 40, RESIDENTIAL_2X2_BUILDINGS[0], 2);
    // Stale wrong corners must be rewritten by CAP `0x4370e9`.
    city.set_zone(40, 40, ZONE_RESIDENTIAL_DENSE | 0x80);
    city.set_zone(41, 40, ZONE_RESIDENTIAL_DENSE | 0x10);
    city.set_zone(40, 41, ZONE_RESIDENTIAL_DENSE | 0x20);
    city.set_zone(41, 41, ZONE_RESIDENTIAL_DENSE | 0x40);
    city.set_traffic_density(40, 40, 0x31);
    city.set_traffic_density(41, 41, 0x31);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(pe_fb_zone_degradable(city.building_at(40, 40)));
    assert!(pe_fb_degrade_under_cloud(
        &mut city, 40, 40, &mut crt, &mut lfsr
    ));
    let b = city.building_at(40, 40);
    assert!(
        ABANDONED_POP2_BUILDINGS.contains(&b),
        "expected CAP abandon pop2, got {b:#04x}"
    );
    assert_eq!(city.building_at(41, 41), b);
    let corner_bits = pe_choose_and_place_corner_bits(city.rotation());
    for &(tx, ty) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        let expected =
            ZONE_RESIDENTIAL_DENSE | building_zone_bits_for_tile(40, 40, tx, ty, 2, corner_bits);
        assert_eq!(
            city.zone_at(tx, ty).map(|z| z.raw_bits()),
            Some(expected),
            "XZON corner at ({tx},{ty})"
        );
    }
    assert_eq!(city.traffic_density_at(40, 40), Some(0x31));
    assert_eq!(city.traffic_density_at(41, 41), Some(0x31));
}

#[test]
fn pe_fun_00447500_zero_count_is_noop() {
    // PE @ `0x447503`: count 0 returns without CRT/PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-tree-zero.sc2", "TreeZero").expect("blank");
    let mut rng = Win95CrtRand::new(1);
    let before = rng.clone();
    pe_fun_00447500(&mut city, &mut rng, 0);
    assert_eq!(city.building_at(0, 0), 0);
    assert_eq!(rng, before);
}

#[test]
fn pe_fun_004392a0_noop_below_0xc6() {
    // PE @ `0x4392c9`: XBLD < 0xC6 (incl. parks) returns without PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-392a0-noop.sc2", "Fun392Noop").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_tile_bits(20, 20, TileBits::new(0xf0));
    city.set_zone(20, 20, ZONE_MILITARY | ZONE_CORNER_ALL);
    pe_fun_004392a0_clear_before_runway(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0x0d);
    assert_eq!(city.tile_bits_at(20, 20).unwrap().raw_bits(), 0xf0);
    assert_eq!(
        city.zone_at(20, 20).unwrap().raw_bits(),
        ZONE_MILITARY | ZONE_CORNER_ALL
    );
}

#[test]
fn pe_fun_004392a0_mid_range_clears_even_aligned_2x2() {
    // PE @ `0x43935f`: XBLD `0xC6..=0xDA` or `>0xEA` → even NW 2×2 PlaceTile(0)+masks.
    let mut city = City::new_blank("/tmp/sc2000-392a0-2x2.sc2", "Fun392Two").expect("blank");
    // Odd tile in mid-range → origin (20,20).
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        city.set_building(x, y, 0xaa);
        city.set_tile_bits(x, y, TileBits::new(0xff));
        city.set_zone(x, y, ZONE_MILITARY | ZONE_CORNER_ALL);
    }
    city.set_building(21, 21, 0xc8);
    pe_fun_004392a0_clear_before_runway(&mut city, 21, 21);
    for &(x, y) in &[(20, 20), (21, 20), (20, 21), (21, 21)] {
        assert_eq!(city.building_at(x, y), 0, "2x2 cleared at {x},{y}");
        assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits(), 0x3f);
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), ZONE_MILITARY);
    }
}

#[test]
fn underground_preview_uses_virtual_drag_neighbors() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let state = test_state(city);
    let virtual_tiles = [(10, 10), (11, 10), (11, 11)]
        .into_iter()
        .collect::<HashSet<_>>();

    let endpoint =
        state.underground_preview_render_tile(10, 10, UndergroundKind::Pipes, &virtual_tiles);
    let corner =
        state.underground_preview_render_tile(11, 10, UndergroundKind::Pipes, &virtual_tiles);

    assert_eq!(
        endpoint.tile,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_RIGHT)
    );
    assert_eq!(
        corner.tile,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_BOTTOM | NETWORK_LEFT)
    );
    assert_eq!(corner.source, UndergroundTileSource::UserLaid);
    assert!(!corner.watered);
}

#[test]
fn underground_preview_matches_single_tile_crossover_rules() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let existing = underground_tile_for_mask(UndergroundKind::Subway, NETWORK_TOP | NETWORK_BOTTOM);
    city.set_underground(10, 10, existing);
    let state = test_state(city);
    let virtual_tiles = [(10, 10)].into_iter().collect::<HashSet<_>>();

    let preview =
        state.underground_preview_render_tile(10, 10, UndergroundKind::Pipes, &virtual_tiles);

    assert_eq!(
        preview.tile,
        underground_crossing_tile_for(UndergroundKind::Pipes, NetworkAxis::GridX, existing)
            .expect("pipe/subway crossover")
    );
    assert_eq!(underground_tile_sprite_id(preview), Some(1349));
}

#[test]
fn native_global_sea_level_reaches_zero_and_refuses_blocked_steps() {
    for engine in [EngineFidelity::Win95, EngineFidelity::Mac68k] {
        let mut city = City::new_blank("/tmp/native-sea-limits.sc2", "NativeSeaLimits").unwrap();
        city.set_mac_selected_water_height(1);
        city.set_altitude(20, 20, 1, 4);
        city.set_terrain(20, 20, 0x25);
        city.set_tile_bits(20, 20, TileBits::new(4));
        let mut state = test_state(city);
        state.mode = AppMode::EditNewMap;
        state.engine_fidelity = engine;

        assert_eq!(state.adjust_edit_map_sea_level(-1), 1);
        assert_eq!(state.city.mac_selected_water_height(), Some(0));
        assert_eq!(state.city.water_height_at(20, 20), 0);
        assert_eq!(state.city.terrain_at(20, 20), 5);
        state.city.set_tile_bits(20, 20, TileBits::new(4));
        assert_eq!(state.adjust_edit_map_sea_level(-1), 0);
        assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile()); // Blocked: no map scan.

        state.city.set_mac_selected_water_height(30);
        assert!(state.adjust_edit_map_sea_level(1) > 0);
        assert_eq!(state.city.mac_selected_water_height(), Some(31));
        state.city.set_building(20, 20, 6);
        state.city.set_tile_bits(20, 20, TileBits::new(0));
        assert_eq!(state.adjust_edit_map_sea_level(1), 0);
        assert_eq!(state.city.building_at(20, 20), 6);
        assert!(!state.city.tile_bits_at(20, 20).unwrap().water_tile());
    }
}

#[test]
fn refresh_underground_tile_preserves_xtxt() {
    // Underground reshape is XUND only; leave XTXT.
    let mut city = City::new_blank("/tmp/sc2000-und-refresh-xtxt.sc2", "UndRefT").expect("blank");
    city.set_underground(55, 55, 0x01);
    city.set_text_overlay(55, 55, 0x52);
    let mut state = test_state(city);
    state.refresh_underground_tile(55, 55, UndergroundKind::Pipes);
    assert_eq!(state.city.text_overlay_at(55, 55), Some(0x52));
}

#[test]
fn mac68k_refresh_underground_tile_preserves_xtxt() {
    // Shared underground reshape leaves XTXT; Mac68k must not invent a clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-und-refresh-xtxt.sc2", "MacUndRefT").expect("blank");
    city.set_underground(55, 55, 0x01);
    city.set_text_overlay(55, 55, 0x52);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_underground_tile(55, 55, UndergroundKind::Pipes);
    assert_eq!(state.city.text_overlay_at(55, 55), Some(0x52));
}

#[test]
fn mac68k_refresh_underground_tile_preserves_xtrf() {
    // Shared underground reshape is XUND only; Mac68k must not invent an XTRF clear.
    let mut city =
        City::new_blank("/tmp/sc2000-mac-und-refresh-xtrf.sc2", "MacUndRef").expect("blank");
    city.set_underground(55, 55, 0x01);
    city.set_traffic_density(55, 55, 0x2b);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.refresh_underground_tile(55, 55, UndergroundKind::Pipes);
    assert_eq!(state.city.traffic_density_at(55, 55), Some(0x2b));
}

#[test]
fn every_arcology_choice_previews_and_stamps_a_four_by_four_footprint() {
    for (index, choice) in ARCOLOGY_CHOICES.iter().enumerate() {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(500_000);
        city.set_rewards_available_mask(1 << 4);
        let mut state = test_state(city);
        state.tool = choice.tool;
        let origin = (20 + index * 8, 20);
        let placement = state
            .tool_placement_at(origin.0, origin.1)
            .expect("arcology placement");

        assert_eq!(placement.preview, ToolPreview::Building(choice.building));
        assert_eq!(placement.valid_tiles.len(), 16, "{}", choice.label);
        assert!(placement.invalid_tiles.is_empty(), "{}", choice.label);
        assert!(
            state.apply_tool_at_tile(origin.0, origin.1),
            "{}",
            choice.label
        );
        assert_eq!(
            state.city.city_funds(),
            500_000 - choice.cost as i32,
            "{}",
            choice.label
        );
        for y in origin.1..origin.1 + 4 {
            for x in origin.0..origin.0 + 4 {
                assert_eq!(state.city.building_at(x, y), choice.building);
                assert!(
                    state
                        .city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.piped()),
                    "{} footprint tile ({x}, {y}) should have building plumbing",
                    choice.label
                );
            }
        }
    }
}

#[test]
fn pe_fa_elevated_band_exclusive_upper_0x46_vs_0x47_like_pe_45a120() {
    // PE DestroyBuilding @ `0x45a120`: network when XBLD `>= 0x3f && < 0x47` (jl 0x47).
    let mut city = City::new_blank("/tmp/sc2000-fa-elev-bound.sc2", "FaElevBound").expect("blank");
    city.set_building(20, 20, 0x46);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x47);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn mac68k_pe_fa_elevated_band_exclusive_upper_0x46_vs_0x47_like_pe() {
    let mut city =
        City::new_blank("/tmp/sc2000-fa-elev-bound-mac.sc2", "FaElevBoundMac").expect("blank");
    city.set_building(20, 20, 0x46);
    city.set_text_overlay(20, 20, 0xfa);
    city.set_building(21, 20, 0x47);
    city.set_text_overlay(21, 20, 0xfa);
    assert_eq!(pe_fa_network_overlay_count_4cc4d8(&city), 1);
    assert_eq!(citygame_fa_non_network_neighbor_marker_count(&city), 1);
}

#[test]
fn simulation_day22_grants_mayor_house_on_first_progression_milestone() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.no_disasters_enabled = true;
    state.city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 0);
    state.city.set_rewards_available_mask(0);
    // Growth-disabled census stand-in writes pe_dw from R+C+I before the gate.
    state.zone_growth_enabled = false;
    let population = PopulationStats {
        residential_population: 2_001,
        total_population: 2_001,
        populated_tiles: 1,
        ..PopulationStats::default()
    };
    let outcomes = live_city_service_outcomes_for_city(&state.city);

    let day22 = state.sim_calendar_day22_scenario(false, population, outcomes);

    assert!(day22.status_announced);
    assert!(!day22.ambient_disaster_started);
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET),
        Some(1)
    );
    assert_eq!(state.city.rewards_available_mask() & 0b0_0001, 0b0_0001);
    assert_eq!(state.query_line, "Reward Available");
}

#[test]
fn simulation_day22_advances_only_one_progression_level_per_pass() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state.city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 0);
    let population = PopulationStats {
        residential_population: 100_000,
        total_population: 100_000,
        populated_tiles: 1,
        ..PopulationStats::default()
    };
    let outcomes = live_city_service_outcomes_for_city(&state.city);

    let _ = state.sim_calendar_day22_scenario(false, population, outcomes);

    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET),
        Some(1),
        "Win95 advances one milestone per month even when population clears several thresholds"
    );
    assert_eq!(state.city.rewards_available_mask() & 0b0_0001, 0b0_0001);
    assert_eq!(state.city.rewards_available_mask() & 0b0_0010, 0);
}

#[test]
fn system_microsims_are_shared_and_structure_microsims_allocate() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let mut state = test_state(city);

    assert!(state.place_building_at(10, 10, 236) > 0); // bus 2x2
    assert_eq!(state.city.text_overlay_at(10, 10), Some(0x34));
    assert_eq!(
        state
            .city
            .microsim_at(MICROSIM_SYSTEM_BUS)
            .map(|r| r.tile_id),
        Some(236)
    );

    assert!(state.place_building_at(14, 10, 236) > 0);
    assert_eq!(
        state.city.text_overlay_at(14, 10),
        Some(0x34),
        "second bus depot reuses the bus system microsim"
    );

    assert!(state.place_building_at(20, 20, 212) > 0); // museum 3x3
    let museum_text = state.city.text_overlay_at(20, 20).expect("museum xtxt");
    assert_eq!(museum_text, 0x3a);

    assert!(state.place_building_at(30, 30, 210) > 0); // police 3x3
    let police_text = state.city.text_overlay_at(30, 30).expect("police xtxt");
    assert!(police_text >= 0x3d);
    let police_index = usize::from(police_text - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(
        state.city.microsim_at(police_index).map(|r| r.tile_id),
        Some(210)
    );
}

#[test]
fn arco_steals_structure_microsim_when_table_is_full() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for i in 0..(MICROSIM_COUNT - MICROSIM_STRUCTURE_FIRST) {
        assert!(
            city.provision_microsim_for_building(210).is_some(),
            "failed to fill slot {i}"
        );
    }
    assert!(city.provision_microsim_for_building(210).is_none());
    let stolen = city
        .provision_microsim_for_building(251)
        .expect("arco should steal a structure slot");
    assert_eq!(
        stolen,
        MIN_SIM_TEXT_ENTRIES + MICROSIM_STRUCTURE_FIRST as u8
    );
    assert_eq!(
        city.microsim_at(MICROSIM_STRUCTURE_FIRST)
            .map(|r| r.tile_id),
        Some(251)
    );
}

#[test]
fn january_refreshes_system_microsim_counts() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(20, 20, 200); // wind
    city.set_building(21, 20, 198); // hydro
    // This low-level setup bypasses PlaceTile, so keep the PE BSS tile census
    // coherent with the two XBLD cells as well.
    city.pe_dw_tile_count[200] = 1;
    city.pe_dw_tile_count[198] = 1;
    // PlaceTile/SimulationProvisionMicrosim writes the fixed-row selector tags;
    // a raw XBLD census alone must not activate the annual XMIC arms.
    city.provision_microsim_for_building(200)
        .expect("wind selector row");
    city.provision_microsim_for_building(198)
        .expect("hydro selector row");
    city.refresh_system_microsim_counts();
    assert_eq!(
        city.microsim_at(MICROSIM_SYSTEM_WIND)
            .map(|r| (r.stat1, r.stat2)),
        Some((1, 4)),
        "PE wind: MW = unit count × 4"
    );
    assert_eq!(
        city.microsim_at(MICROSIM_SYSTEM_HYDRO)
            .map(|r| (r.stat1, r.stat2)),
        Some((1, 20)),
        "PE hydro: MW = unit count × 20"
    );
}

#[test]
fn january_year_roll_consumes_process_lfsr_for_arco() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 14, 2, 251, 4);
    let arco = city.provision_microsim_for_building(251).expect("arco");
    let arco_index = usize::from(arco - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(arco_index, 12));
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        1,
    );
    let mut state = test_state(city);
    let before_lfsr = state.pe_process_lfsr_state();
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);
    assert_ne!(
        state.pe_process_lfsr_state(),
        before_lfsr,
        "January arco occupancy should consume pe_process_lfsr"
    );
    assert!(
        state
            .city
            .microsim_at(arco_index)
            .is_some_and(|r| r.stat2 > 0),
        "arco residents should be written"
    );
}

#[test]
fn january_opinion_poll_consumes_process_crt() {
    // PE `0x472510` shares the year-roll process CRT (100 samples) with microsim CRT.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.provision_microsim_for_building(243).expect("mayor");
    city.persist_pe_day22_population_leftover(10_000);
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        1,
    );
    let mut state = test_state(city);
    let before = state.pe_process_crt_seed();
    let mut preview = state.pe_process_crt;
    state
        .city
        .run_pe_opinion_poll_with_rng(10_000, 100, 100, 100, &mut preview);
    assert_ne!(preview.seed(), before, "poll alone advances CRT");
    state.sim_calendar_day0_budget_and_date_at(1901, 1, 1);
    assert_ne!(
        state.pe_process_crt_seed(),
        before,
        "January year-roll must consume pe_process_crt for opinion poll"
    );
    assert!(state.city.pe_mayor_approval() > 0);
}

#[test]
fn day21_eq_le_runs_one_month_instead_of_a_packed_year() {
    let mut month = City::new_blank("/tmp/sc2000-test.sc2", "eq1").expect("blank");
    month.set_pe_life_expectancy(70);
    month.test_seed_pe_age_cohorts_from_pop(20_000);
    let mut rng_m = crate::win95_random::Win95CrtRand::new(1);
    month.advance_pe_eq_le_month(20_000, 0, 0, 0, 100, 100, 100, &mut rng_m);
    assert!(month.pe_eq_averages_ready());
    let month_births = month.pe_eq_births();
    assert!(month_births > 0);

    let mut year = City::new_blank("/tmp/sc2000-test.sc2", "eq12").expect("blank");
    year.set_pe_life_expectancy(70);
    year.test_seed_pe_age_cohorts_from_pop(20_000);
    let mut rng_y = crate::win95_random::Win95CrtRand::new(1);
    year.advance_pe_age_cohorts_yearly(20_000, 0, 0, 0, 100, 100, 100, &mut rng_y);
    assert!(
        year.pe_eq_births() > month_births,
        "12 packed months births={} vs one month {}",
        year.pe_eq_births(),
        month_births
    );

    let mut state_city = City::new_blank("/tmp/sc2000-test.sc2", "eqday").expect("blank");
    state_city.test_seed_pe_age_cohorts_from_pop(20_000);
    assert!(state_city.pe_age_cohort_at(1) + state_city.pe_age_cohort_at(2) > 0);
    let mut state = test_state(state_city);
    state.handle_simulation_month_advanced();
    assert_eq!(
        state.city.pe_age_cohort_at(1) + state.city.pe_age_cohort_at(2),
        0,
        "day 21 with live dwCityPopulation 0 wipes EQ_LE (`0x469537`)"
    );
    assert!(!state.city.pe_eq_averages_ready());
}

#[test]
fn eq_le_month_does_not_invent_pyramid_when_tables_are_empty() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "empty").expect("blank");
    let mut rng = crate::win95_random::Win95CrtRand::new(1);
    let mut expected_rng = crate::win95_random::Win95CrtRand::new(1);
    expected_rng.random_u16();
    city.advance_pe_eq_le_month(10_000, 0, 0, 0, 100, 100, 100, &mut rng);
    assert_eq!(
        city.pe_age_cohort_at(1) + city.pe_age_cohort_at(2),
        0,
        "PE month with pop>0 still runs on zeros; neither month nor January invents a pyramid"
    );
    assert_eq!(city.pe_eq_births(), 0);
    assert_eq!(
        rng.seed(),
        expected_rng.seed(),
        "PE draws once for births even when the fertile quotient is zero"
    );
    assert!(
        city.pe_eq_averages_ready(),
        "pop>0 still writes fertile averages (`0x469be8`)"
    );
}

#[test]
fn simulation_day0_january_occupancy_reads_leftover_eq_le_buckets() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 2, 2, SCHOOL_BUILDING_ID, 3);
    let school = city.provision_microsim_for_building(214).expect("school");
    let school_index = usize::from(school - MIN_SIM_TEXT_ENTRIES);
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
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    assert!(set_city_auto_budget_enabled(&mut city, true));
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.traffic.set_speed(GameSpeed::Llama);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.month(), 1);
    assert_eq!(
        state.city.pe_dw_city_population(),
        10_000,
        "January occupancy must use last day-22 snapshot, not live census"
    );
    assert_eq!(
        state.city.microsim_at(school_index).map(|r| r.stat1),
        Some(500),
        "year-end FUN_0044e390 divides leftover-filled EQ_LE[1]+[2]"
    );
}

#[test]
fn january_eq_le_life_table_deaths_rise_when_life_expectancy_falls() {
    // Seeded buckets 0..=8 fully survive ordinance LE 85..=100 (age_div <= 40).
    // Bucket 19 uses divisor 95: LE 100 lives, LE 85 dies.
    let mut healthy = City::new_blank("/tmp/sc2000-test.sc2", "healthy").expect("blank");
    healthy.set_misc_u32_at(ORDINANCE_FLAGS_MISC_OFFSET, 0x200 | 0x400 | 0x20);
    healthy.test_seed_pe_age_cohorts_from_pop(20_000);
    healthy.test_add_pe_eq_cohort(19, 5_000);
    healthy.advance_pe_eq_le_month(
        25_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    let healthy_deaths = healthy.pe_eq_deaths();

    let mut sickly = City::new_blank("/tmp/sc2000-test.sc2", "sickly").expect("blank");
    sickly.test_seed_pe_age_cohorts_from_pop(20_000);
    sickly.test_add_pe_eq_cohort(19, 5_000);
    sickly.advance_pe_eq_le_month(
        25_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    let sickly_deaths = sickly.pe_eq_deaths();

    assert!(
        sickly_deaths > healthy_deaths,
        "ordinance LE85 deaths={sickly_deaths} should exceed LE100 deaths={healthy_deaths}"
    );
    assert!(
        crate::city::pe_eq_survival_score(70, 1, 5) >= 100,
        "young buckets with LE-scale life points are fully surviving"
    );
    assert!(
        crate::city::pe_eq_survival_score(70, 1, 95) < 100,
        "oldest divisor forces mortality when life/pop == 70"
    );
    assert!(
        crate::city::pe_eq_survival_score(100, 1, 95) >= 100,
        "ordinance LE 100 survives the oldest divisor"
    );
    assert!(
        crate::city::pe_eq_survival_score(85, 1, 95) < 100,
        "ordinance LE 85 dies in the oldest divisor"
    );
}

#[test]
fn january_eq_le_fertile_averages_write_life_eq_and_workforce() {
    let mut cohorts = [0_u32; 20];
    let mut life = [0_u32; 20];
    let mut aux = [0_u32; 20];
    cohorts[4] = 10;
    life[4] = 700;
    aux[4] = 80;
    assert_eq!(
        crate::city::pe_eq_fertile_averages(&cohorts, &life, &aux, 10),
        (70, 8, 90),
        "one fertile bucket: life 700/10, aux 80/10, 10*100/(dwCityPopulation 10+1)"
    );

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "fertile").expect("blank");
    city.set_pe_life_expectancy(70);
    city.test_seed_pe_age_cohorts_from_pop(20_000);
    assert_eq!(
        city.pe_eq_life_average(),
        0,
        "seed path does not write 0x4cb410"
    );

    city.advance_pe_age_cohorts_yearly(
        20_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    let life_avg = city.pe_eq_life_average();
    assert!(
        (70..=100).contains(&life_avg),
        "fertile life average should stay near ordinance LE 85 (got {life_avg})"
    );
    let workforce = city.pe_eq_workforce_percent();
    assert!(
        (1..=100).contains(&workforce),
        "workforce percent is fertile*100/(total+1) (got {workforce})"
    );
    assert!(city.pe_eq_averages_ready());
    assert_eq!(
        city.misc_u32_at(MISC_WORK_FORCE_PERCENT_OFFSET),
        Some(workforce.min(100)),
        "PE writes 0x4cb460 into MISC 0x44"
    );
    assert_eq!(
        work_force_percent_for_city(&city, live_population_stats_for_city(&city)),
        workforce.min(100),
        "live Work Force % follows 0x4cb460 after EQ_LE averages"
    );
    assert_eq!(
        city.misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET),
        Some(life_avg),
        "PE writes 0x4cb410 into MISC 0x48"
    );
    assert_eq!(
        city.misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET),
        Some(city.pe_eq_education_average()),
        "PE writes 0x4cc4b4 into MISC 0x4c"
    );
    assert_eq!(
        u32::from(city.pe_city_pollution()),
        life_avg,
        "poll 0x4cb410 slot is fertile LE after EQ_LE"
    );
    assert_eq!(
        u32::from(city.pe_city_crime()),
        city.pe_eq_education_average(),
        "poll 0x4cc4b4 slot is fertile EQ after EQ_LE"
    );
}

#[test]
fn january_opinion_poll_sets_mayor_approval_when_populated() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mayor = city.provision_microsim_for_building(243).expect("mayor");
    let mayor_index = usize::from(mayor - MIN_SIM_TEXT_ENTRIES);
    city.refresh_structure_microsim_yearly(0, 100, 100, 100, 100, 100);
    assert_eq!(city.pe_mayor_approval(), 0, "pop < 100 skips the poll");
    assert_eq!(city.microsim_at(mayor_index).map(|r| r.stat2), Some(0));

    city.refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);
    let approval = city.pe_mayor_approval();
    assert!(approval > 0 && approval <= 100, "got {approval}");
    assert_eq!(
        city.microsim_at(mayor_index).map(|r| r.stat2),
        Some(approval)
    );
}

#[test]
fn january_museum_and_library_occupancy_follow_fun_0044e390() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 2, 2, 212, 3);
    stamp_complete_building_for_test(&mut city, 8, 2, 245, 2);
    city.provision_microsim_for_building(212).expect("museum");
    city.provision_microsim_for_building(245).expect("library");

    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let museum = city
        .microsim_at(crate::city::MICROSIM_SYSTEM_MUSEUM)
        .expect("museum row");
    let library = city
        .microsim_at(crate::city::MICROSIM_SYSTEM_LIBRARY)
        .expect("library row");
    // Museum @ `0x44e887`: min(pop/20, tiles*college_funding*4); stat2=(funding/10)*tiles.
    assert_eq!(
        (museum.stat1, museum.stat2),
        (350, 90),
        "9 museum tiles, college funding 100"
    );
    // Library @ `0x44ef17`: min(pop/18, tiles*school_funding*4); books+=(funding-50)*tiles.
    assert_eq!(
        (library.stat0, library.stat1, library.stat2),
        (12, 388, 200),
        "4 library tiles, school funding 100, pop 7000"
    );

    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let library = city
        .microsim_at(crate::city::MICROSIM_SYSTEM_LIBRARY)
        .expect("library row");
    assert_eq!(library.stat2, 400, "library books accumulate year to year");

    let mut low = City::new_blank("/tmp/sc2000-lib-low.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut low, 8, 2, 245, 2);
    low.provision_microsim_for_building(245).expect("library");
    low.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 40, 100);
    let low_lib = low
        .microsim_at(crate::city::MICROSIM_SYSTEM_LIBRARY)
        .expect("library row");
    assert_eq!(
        low_lib.stat2, 0,
        "funding 40: (40-50)*tiles is negative, books stay 0"
    );
    assert_eq!(low_lib.stat0, 6, "tiles*funding*300/pop = 6");
}

#[test]
fn january_park_attendance_uses_last_year_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 2, 2, 213, 3);
    city.provision_microsim_for_building(213).expect("park");

    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let park = city
        .microsim_at(crate::city::MICROSIM_SYSTEM_PARK)
        .expect("park row");
    assert_eq!(
        (park.stat1, park.stat2, park.stat3),
        (1_166, 9, 1),
        "first January: InitializeMicrosim +9 acres; min(412×9, pop/6); ÷9 then FUN ÷120"
    );

    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let park = city
        .microsim_at(crate::city::MICROSIM_SYSTEM_PARK)
        .expect("park row");
    assert_eq!(
        (park.stat1, park.stat2, park.stat3),
        (1_166, 9, 1),
        "second January: still min(412×9, pop/6) after yearly tile rewrite"
    );
}

#[test]
fn january_stadium_attendance_divides_by_stadium_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(
        &mut city,
        20,
        10,
        STADIUM_BUILDING_ID,
        STADIUM_BUILDING_SIZE,
    );
    let stadium = city.provision_microsim_for_building(215).expect("stadium");
    let stadium_index = usize::from(stadium - MIN_SIM_TEXT_ENTRIES);

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
    let record = city.microsim_at(stadium_index).expect("stadium row");
    // PE: first ÷ uses raw tiles (16), not buildings. 55860/16=3491; FUN ÷5 is 11172 so
    // attendance is the tile quotient plus CRT jitter (no 25k cap roll).
    assert_eq!(
        (record.stat0, record.stat1),
        (20, 3_716),
        "pop / 16 stadium tiles + CRT jitter (after shared opinion-poll CRT)"
    );
}

#[test]
fn january_fills_prison_zoo_and_llama_flavor_stats() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1955,
        month: 1,
    });

    // Prior-year police arrests feed PE prison intake (`wOldArrests` / `0x4CA430`).
    let police = city.provision_microsim_for_building(210).expect("police");
    let police_index = usize::from(police - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat3(police_index, 9_496));
    city.set_pe_w_old_arrests(9_496);

    let prison = city.provision_microsim_for_building(216).expect("prison");
    let prison_index = usize::from(prison - MIN_SIM_TEXT_ENTRIES);
    let zoo = city.provision_microsim_for_building(218).expect("zoo");
    let zoo_index = usize::from(zoo - MIN_SIM_TEXT_ENTRIES);
    let llama = city.provision_microsim_for_building(255).expect("llama");
    let llama_index = usize::from(llama - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(
        city.microsim_at(prison_index).map(|r| r.stat2),
        Some(0),
        "InitializeMicrosim zeros prison; yearly writes guards"
    );
    assert_eq!(
        city.microsim_at(llama_index).map(|r| r.stat3),
        Some(0x29),
        "llama name != aus: rand & 0x3f until yearly visitor fill"
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
    // PE prison: inmates=3/4*prev + old_arrests/n; capacity min(pop/20, inmates);
    // stat2=min(pop/120, funding*3); stat3=inmates/100; grade when stat3>90.
    // Police row also refreshes first and consumes one CRT roll for arrests.
    assert_eq!(
        city.microsim_at(prison_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((3, 2_793, 300, 94))
    );
    // PE zoo: four independent CRT `rand() % 100` rolls.
    assert_eq!(
        city.microsim_at(zoo_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((38, 58, 13, 15)),
        "convenience annual entry seeds the separate PE game-LCG from CRT seed 1"
    );
    // PE llama (name != "aus"): visitors=(pop>>3)+(rand&0x3ff).
    assert_eq!(
        city.microsim_at(llama_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((31, 7_968, 1_044, 508)),
        "PE CRT stream reaches llama after the annual opinion poll and earlier XMIC slots"
    );
}

#[test]
fn january_ages_mayor_rating_from_stat3_countdown() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1955,
        month: 1,
    });
    let mayor = city.provision_microsim_for_building(243).expect("mayor");
    let mayor_index = usize::from(mayor - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat3(mayor_index, 3));
    assert!(city.set_microsim_stat0(mayor_index, 5));

    city.refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);
    let approval = city.pe_mayor_approval();
    assert_eq!(
        city.microsim_at(mayor_index)
            .map(|r| (r.stat0, r.stat1, r.stat2, r.stat3)),
        Some((6, 1955, approval, 2)),
        "PE mayor: each January with remaining stat3 increments Rating"
    );
    city.refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);
    city.refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);
    assert_eq!(
        city.microsim_at(mayor_index).map(|r| (r.stat0, r.stat3)),
        Some((8, 0))
    );
    city.refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);
    assert_eq!(
        city.microsim_at(mayor_index).map(|r| (r.stat0, r.stat3)),
        Some((8, 0)),
        "Rating freezes once the PE countdown hits 0"
    );
}

#[test]
fn arco_microsim_stat0_follows_win95_formula() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 251) > 0);

    let mut corner = None;
    for y in 20..24 {
        for x in 20..24 {
            // PE @ `0x4352d4`: `(XZON & 0xf0) == 0x80` exact TRIGHT nibble.
            if state
                .city
                .zone_at(x, y)
                .is_some_and(|zone| zone.raw_bits() & 0xf0 == ZONE_CORNER_TRIGHT)
            {
                corner = Some((x, y));
            }
        }
    }
    let (cx, cy) = corner.expect("arco CORNER_TRIGHT tile");
    let text = state.city.text_overlay_at(cx, cy).expect("microsim xtxt");
    assert!(
        (MIN_SIM_TEXT_ENTRIES..=MAX_SIM_TEXT_ENTRIES).contains(&text),
        "expected sim text overlay, got {text}"
    );
    let index = usize::from(text - MIN_SIM_TEXT_ENTRIES);
    assert_eq!(state.city.microsim_at(index).map(|r| r.tile_id), Some(251));

    let mut land = vec![0u8; CITY_TILES];
    let mut crime = vec![0u8; CITY_TILES];
    let mut pollution = vec![0u8; CITY_TILES];
    land[cy * CITY_WIDTH + cx] = 160; // >>5 = 5
    crime[cy * CITY_WIDTH + cx] = 64; // >>5 = 2
    pollution[cy * CITY_WIDTH + cx] = 32; // >>5 = 1
    state.city.sync_land_value_from_slice(&land);
    state.city.sync_crime_density_from_slice(&crime);
    state.city.sync_pollution_density_from_slice(&pollution);

    // Unpowered + unwatered: (5-2-1+12)=14 → 12, /2 → 6, /2 → 3.
    assert!(state.update_arco_microsim_at_for_test(cx, cy));
    assert_eq!(state.city.microsim_at(index).map(|r| r.stat0), Some(3));

    power_adjacent_building_for_test(&mut state.city, cx, cy - 1);
    let bits = state.city.tile_bits_at(cx, cy).unwrap_or_default();
    state.city.set_tile_bits(
        cx,
        cy,
        bits.with_powered(true).with_piped(true).with_watered(true),
    );
    // Powered + watered keeps the clamped 12.
    assert!(state.update_arco_microsim_at_for_test(cx, cy));
    assert_eq!(state.city.microsim_at(index).map(|r| r.stat0), Some(12));
}

#[test]
fn arco_microsim_refuses_corner_all_nibble() {
    // PE @ `0x4352d4`: `(XZON & 0xf0) == 0x80` — CORNER_ALL must not pass.
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 251) > 0);
    let mut corner = None;
    for y in 20..24 {
        for x in 20..24 {
            if state
                .city
                .zone_at(x, y)
                .is_some_and(|zone| zone.raw_bits() & 0xf0 == ZONE_CORNER_TRIGHT)
            {
                corner = Some((x, y));
            }
        }
    }
    let (cx, cy) = corner.expect("arco CORNER_TRIGHT tile");
    state.city.set_zone(cx, cy, ZONE_CORNER_ALL);
    assert!(
        !state.update_arco_microsim_at_for_test(cx, cy),
        "exact TRIGHT nibble required"
    );
}

#[test]
fn simulation_day22_grants_llama_dome_at_fifth_progression() {
    let mut state = test_state(City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank"));
    state.no_disasters_enabled = true;
    state.zone_growth_enabled = false;
    state.city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 4);
    state.city.set_rewards_available_mask(0);
    let population = PopulationStats {
        residential_population: 90_001,
        total_population: 90_001,
        populated_tiles: 1,
        ..PopulationStats::default()
    };
    let outcomes = live_city_service_outcomes_for_city(&state.city);

    let day22 = state.sim_calendar_day22_scenario(false, population, outcomes);

    assert!(day22.status_announced);
    assert_eq!(
        state.city.misc_u32_at(MISC_CITY_PROGRESSION_OFFSET),
        Some(5)
    );
    assert_eq!(state.city.rewards_available_mask() & 0b0_1000, 0b0_1000);
    assert_eq!(state.query_line, "Reward Available");
}

#[test]
fn simulation_day23_does_not_roll_xgrp_history() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let xgrp_history = (0..XGRP_VALUES as u32).collect::<Vec<_>>();
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &xgrp_history);
    city.set_building(20, 20, RESIDENTIAL_1X1_BUILDINGS[0]);
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 1, 22);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 23);
    assert_eq!(
        state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES),
        xgrp_history,
        "UpdateGraphDialog is UI-only; UpdateGraphData already ran on day 21"
    );
}

#[test]
fn simulation_month_runs_day23_and_day24_ui_passes() {
    let mut city = reward_unlock_test_city();
    city.set_building(
        0,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.refresh_live_misc_current_values();
    state
        .city
        .set_misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET, 99);
    state
        .city
        .set_misc_u32_at(MISC_POPULATED_TILE_COUNT_OFFSET, 1);
    state.city.set_rewards_available_mask(0);

    state.handle_simulation_month_advanced();

    assert_eq!(state.city.rewards_available_mask() & 0b0_1111, 0b0_1111);
    assert_eq!(
        state.city.misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET),
        Some(1)
    );
    assert_ne!(
        state.city.misc_u32_at(MISC_POPULATED_TILE_COUNT_OFFSET),
        Some(1),
        "month helper should run day-23 census refresh"
    );
}

#[test]
fn simulation_day24_alone_writes_neighbor_connections() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(
        0,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);
    state.refresh_live_misc_current_values();
    state
        .city
        .set_misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET, 99);
    state.city.clear_dirty();
    state.game_clock = GameClock::new_with_day(1900, 1, 23);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.day(), 24);
    assert_eq!(
        state.city.misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET),
        Some(1)
    );
    assert_eq!(
        state.city.text_overlay_at(0, 10),
        Some(crate::trip_generator::TRIP_NEIGHBOR_CONNECTION_OVERLAY)
    );
    assert!(state.city.has_dirty_tiles());
}

#[test]
fn mac_phase24_does_not_write_pe_neighbor_connection_summary() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(
        0,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.city.set_rewards_available_mask(0);
    state
        .city
        .set_misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET, 99);
    state.city.clear_dirty();

    assert_eq!(state.sim_calendar_day24_ui_chrome(), 0);
    assert_eq!(state.city.rewards_available_mask(), 0);
    assert_eq!(
        state.city.misc_u32_at(NEIGHBOR_CONNECTION_MISC_OFFSET),
        Some(99),
        "Mac phase 24 proposal producer is separate from PE UI chrome"
    );
    assert!(!state.city.has_dirty_tiles());
}

#[test]
fn weather_season_matches_win95_month_formula() {
    assert_eq!(weather_season_for_month(1), 0);
    assert_eq!(weather_season_for_month(2), 0);
    assert_eq!(weather_season_for_month(3), 1);
    assert_eq!(weather_season_for_month(6), 2);
    assert_eq!(weather_season_for_month(9), 3);
    assert_eq!(weather_season_for_month(12), 0);
}

#[test]
fn weather_state_eases_toward_trend_targets() {
    let mut rng = MacToolboxRng::new(1);
    let before = WeatherState {
        heat: 80,
        wind: 15,
        humidity: 0,
        trend: WEATHER_TREND_COLD,
        season: 0,
    };
    let after = advance_weather_state(before, &mut rng);
    assert!(after.trend <= WEATHER_TREND_TORNADO);
    assert!(WEATHER_TRANSITIONS[0][0].contains(&after.trend));
    let target = WEATHER_HEAT_TARGETS[usize::from(after.trend)];
    assert_eq!(after.heat, (before.heat + u32::from(target)) / 2);
}

#[test]
fn advance_simulation_weather_advances_pe_process_crt() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 3, 20);
    let before = state.pe_process_crt_seed();
    state.advance_simulation_weather();
    assert_ne!(state.pe_process_crt_seed(), before);
}

#[test]
fn mac68k_january_skips_win95_annual_attitude_and_life_cache_writes() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-jan-pe-caches.sc2", "MacJanCaches").expect("blank");
    city.kind = CityFileKind::MacCity;
    city.set_building(10, 10, 0x1d);
    city.set_pe_attitude_tax_bases([1_000, 2_000, 3_000]);
    city.set_pe_attitude_demand_inputs(8, 300, 200, 100);
    city.recompute_pe_attitude_scores(60, 70, 80);
    city.set_pe_life_expectancy(81);
    let scores_before: [u32; 4] = std::array::from_fn(|index| city.pe_attitude_score_at(index));
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
    state.city.recompute_pe_attitude_scores(60, 70, 80);

    let scores_after: [u32; 4] =
        std::array::from_fn(|index| state.city.pe_attitude_score_at(index));
    assert_eq!(scores_after, scores_before);
    assert_eq!(state.city.pe_life_expectancy(), 81);
}

#[test]
fn pe_clear_small_xbld_preserves_xtxt() {
    // Military-zone small-XBLD clear is XBLD-only — leave XTXT/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-mil-clear-xtxt.sc2", "MilXtxt").expect("blank");
    city.set_building(55, 55, 5);
    city.set_text_overlay(55, 55, 0x18);
    city.set_traffic_density(55, 55, 0x61);
    pe_clear_small_xbld_for_military_zone(&mut city, 55, 55);
    assert_eq!(city.building_at(55, 55), 0);
    assert_eq!(city.text_overlay_at(55, 55), Some(0x18));
    assert_eq!(city.traffic_density_at(55, 55), Some(0x61));
}

#[test]
fn partial_desalination_stamp_does_not_become_source_plumbing() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 250);
    city.set_underground(10, 10, UNDER_TILE_BUILDING_PIPE_TEE);

    assert!(!water_source_connects_as_building_pipe(&city, 10, 10));
    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPE_TEE,
            source: UndergroundTileSource::UserLaid,
            watered: false,
        })
    );
    let (_, connected_tiles) = water_pipe_connection_maps(&city);
    assert!(!connected_tiles[10 * CITY_WIDTH + 10]);

    for y in 10..=12 {
        for x in 10..=12 {
            city.set_building(x, y, 250);
        }
    }

    assert!(water_source_connects_as_building_pipe(&city, 10, 10));
    let (_, connected_tiles) = water_pipe_connection_maps(&city);
    assert!(connected_tiles[10 * CITY_WIDTH + 10]);
    assert_eq!(
        underground_render_tile(&city, 10, 10, 0),
        Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPE_TEE,
            source: UndergroundTileSource::BuildingInternal,
            watered: false,
        })
    );
}

#[test]
fn win95_xrog_recurrence_matches_signed_word_steps_for_all_byte_inputs() {
    for old_growth in 0..=u8::MAX {
        for old_pop in 0..=u8::MAX {
            for new_pop in 0..=u8::MAX {
                let delta = i16::from(new_pop) - i16::from(old_pop);
                let numerator = i16::from(old_growth)
                    .wrapping_mul(7)
                    .wrapping_add(delta.wrapping_mul(8))
                    .wrapping_add(0x80);
                let expected = (numerator / 8).clamp(0, i16::from(u8::MAX)) as u8;
                assert_eq!(
                    pe_rate_of_growth_step(old_growth, old_pop, new_pop),
                    expected,
                    "old growth/pop/new pop {old_growth}/{old_pop}/{new_pop}"
                );
            }
        }
    }
}

#[test]
fn refresh_xgrp_history_rebuilds_tech_unlock_mask_like_4138ee() {
    // PE UpdateGraphData @ `0x4138ee` → `0x4023ec` tech-unlock rebuild.
    let mut city = City::new_blank("/tmp/sc2000-xgrp-tech.sc2", "XgrpTech").expect("blank");
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &vec![0u32; XGRP_VALUES]);
    for i in 0..17 {
        city.set_pe_w_neighbor_fame_rank(i, 1900);
    }
    city.set_pe_w_neighbor_fame_rank(0, 0);
    assert_eq!(city.pe_w_tech_unlock_mask(), 0);
    let mut state = test_state(city);
    let population = population_stats_for_city(&state.city);
    let outcomes = city_service_outcomes_for_city(&state.city);
    state.refresh_xgrp_history(population, outcomes);
    assert_eq!(state.city.pe_w_tech_unlock_mask(), 7 | 0x8);
}

#[test]
fn xgrp_history_uses_original_half_year_and_five_year_cadence() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let xgrp_history = (10_000..10_000 + XGRP_VALUES as u32).collect::<Vec<_>>();
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &xgrp_history);
    city.set_building(10, 10, 0x70);
    city.set_building(11, 10, 0x70);
    city.set_building(12, 10, 0x84);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 1);

    state.refresh_live_misc_stats();

    let updated = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    let population = population_stats_for_city(&state.city);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let expected_latest = xgrp_history_latest_values(&state.city, population, outcomes);
    for (series_index, latest) in expected_latest.into_iter().enumerate() {
        let base = series_index * XGRP_SAMPLES_PER_SERIES;
        assert_eq!(updated[base], latest);
        assert_eq!(updated[base + XGRP_HALF_YEAR_OFFSET], latest);
        assert_eq!(updated[base + XGRP_FIVE_YEAR_OFFSET], latest);
    }
}

#[test]
fn pe_xgrp_cadence_uses_age_derived_calendar_words() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (age, half, five) in [
        (0, true, true),
        (25, false, false),
        (150, true, false),
        (300, true, false),
        (1500, true, true),
        (1800, true, false),
    ] {
        city.pe_dw_city_age = age;
        city.pe_publish_graph_calendar_words_from_city_age_41354e();
        assert_eq!(pe_xgrp_rolls_half_year(&city), half, "age {age}");
        assert_eq!(pe_xgrp_rolls_five_year(&city), five, "age {age}");
    }
}

#[test]
fn xgrp_july_rolls_half_year_but_not_five_year() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let xgrp_history = (20_000..20_000 + XGRP_VALUES as u32).collect::<Vec<_>>();
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &xgrp_history);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 7);
    state.city.pe_dw_city_age = 150;
    state.refresh_live_misc_stats();

    let updated = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    let population = population_stats_for_city(&state.city);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let expected_latest = xgrp_history_latest_values(&state.city, population, outcomes);
    for (series_index, latest) in expected_latest.into_iter().enumerate() {
        let base = series_index * XGRP_SAMPLES_PER_SERIES;
        assert_eq!(updated[base], latest);
        assert_eq!(updated[base + XGRP_HALF_YEAR_OFFSET], latest);
        assert_eq!(
            &updated[base + XGRP_FIVE_YEAR_OFFSET
                ..base + XGRP_FIVE_YEAR_OFFSET + XGRP_FIVE_YEAR_SAMPLES],
            &xgrp_history[base + XGRP_FIVE_YEAR_OFFSET
                ..base + XGRP_FIVE_YEAR_OFFSET + XGRP_FIVE_YEAR_SAMPLES],
            "July must not copy five-year XGRP"
        );
    }
}

#[test]
fn civic_outcomes_do_not_read_pe_eq_fields_for_mac_state_or_engine() {
    const PE_LIFE_MISC_OFFSET: usize = 0x0048;
    const PE_EDUCATION_MISC_OFFSET: usize = 0x004c;
    const MAC_EVENT_HISTORY_LIFE_VALUE: u32 = 137;
    const MAC_EVENT_HISTORY_EDUCATION_VALUE: u32 = 121;

    let population = PopulationStats {
        normal_population: 100,
        total_population: 100,
        populated_tiles: 1,
        residential_tiles: 1,
        ..PopulationStats::default()
    };
    let estimate = |city: &City, fidelity| {
        let power_maps = power_service_maps(city);
        city_service_outcomes_for_population_with_power_and_fidelity(
            city,
            population,
            false,
            &power_maps,
            fidelity,
        )
    };

    let mut mac_city =
        City::new_blank("/tmp/sc2000-mac-civic-fields.sc2", "MacFields").expect("blank city");
    mac_city.kind = CityFileKind::MacCity;
    let mac_baseline = estimate(&mac_city, EngineFidelity::Mac68k);

    // These Mac-owned state words occupy the same byte offsets as PE LE/EQ.
    // Loading them sets the PE cache, but Mac fidelity must not consume that cache.
    let mut mac_with_event_history = mac_city.clone();
    mac_with_event_history.set_misc_u32_at(PE_LIFE_MISC_OFFSET, MAC_EVENT_HISTORY_LIFE_VALUE);
    mac_with_event_history
        .set_misc_u32_at(PE_EDUCATION_MISC_OFFSET, MAC_EVENT_HISTORY_EDUCATION_VALUE);
    assert!(mac_with_event_history.pe_eq_averages_ready());
    let mac_outcomes = estimate(&mac_with_event_history, EngineFidelity::Mac68k);
    assert_eq!(mac_outcomes.life_expectancy, mac_baseline.life_expectancy);
    assert_eq!(
        mac_outcomes.education_quotient,
        mac_baseline.education_quotient
    );
    let mac_saved = saved_city_service_outcomes_for_city_with_fidelity(
        &mac_with_event_history,
        EngineFidelity::Mac68k,
    );
    assert_eq!(mac_saved.life_expectancy, 0);
    assert_eq!(mac_saved.education_quotient, 0);

    // The identical offsets remain valid saved averages for a Win95 engine and
    // PE-layout city; a Mac engine must ignore them even on that file format.
    let mut pe_city =
        City::new_blank("/tmp/sc2000-pe-civic-fields.sc2", "PEFields").expect("blank city");
    pe_city.set_misc_u32_at(PE_LIFE_MISC_OFFSET, MAC_EVENT_HISTORY_LIFE_VALUE);
    pe_city.set_misc_u32_at(PE_EDUCATION_MISC_OFFSET, MAC_EVENT_HISTORY_EDUCATION_VALUE);
    let pe_outcomes = estimate(&pe_city, EngineFidelity::Win95);
    assert_eq!(pe_outcomes.life_expectancy, MAC_EVENT_HISTORY_LIFE_VALUE);
    assert_eq!(
        pe_outcomes.education_quotient,
        MAC_EVENT_HISTORY_EDUCATION_VALUE
    );
    let mac_engine_on_pe_file = estimate(&pe_city, EngineFidelity::Mac68k);
    assert_eq!(
        mac_engine_on_pe_file.life_expectancy,
        mac_baseline.life_expectancy
    );
    assert_eq!(
        mac_engine_on_pe_file.education_quotient,
        mac_baseline.education_quotient
    );
}

#[test]
fn class0_aggregate_skips_multitile_continuation_tiles() {
    // PE corner gate @ `0x436056`: only the view-angle corner contributes.
    // Rot 0 SW=0x80. Footprint NW at (20,20) is on lattice (0,0); SW at (20,21)
    // is off-lattice — visiting NW must contribute nothing.
    let mut city = City::new_blank("/tmp/sc2000-class0-cont.sc2", "Class0Cont").expect("blank");
    for y in 20..22 {
        for x in 20..22 {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
    }
    stamp_complete_building_for_test(&mut city, 20, 20, RESIDENTIAL_2X2_BUILDINGS[0], 2);
    or_rci_building_zone_corners_for_test(&mut city, (20, 20), 2);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let _fail = FailGrowthChangeRolls::arm();
    state.sim_calendar_one_growth_tick(0, 0);
    assert_eq!(
        state.pe_growth_aggregate.slots()[2],
        0,
        "NW continuation tile must not credit class-0 aggregate"
    );
}

#[test]
fn class0_aggregate_credits_view_angle_corner_of_multitile() {
    // Rot 0 SW bit 0x80 on (ox, oy+1)=(20,20) for origin (20,19) — on lattice.
    let mut city = City::new_blank("/tmp/sc2000-class0-sw.sc2", "Class0SW").expect("blank");
    for y in 19..21 {
        for x in 20..22 {
            city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        }
    }
    stamp_complete_building_for_test(&mut city, 20, 19, RESIDENTIAL_2X2_BUILDINGS[0], 2);
    or_rci_building_zone_corners_for_test(&mut city, (20, 19), 2);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 18);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    let _fail = FailGrowthChangeRolls::arm();
    state.sim_calendar_one_growth_tick(0, 0);
    let weight = pe_growth_aggregate_weight(pe_growth_level_for_xbld(RESIDENTIAL_2X2_BUILDINGS[0]));
    assert_eq!(state.pe_growth_aggregate.slots()[2], weight);
}

#[test]
fn failed_build_think_does_not_size_up_light_construction() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(20, 20, CONSTRUCTION_POP1_BUILDINGS[0]);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    stamp_xval_for_test(&mut city, XVAL_POPLEVEL_LOW);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    let _fail = FailConstructionCompleteRolls::arm();
    state.sim_calendar_one_growth_tick(0, 0);

    assert_eq!(
        state.city.building_at(20, 20),
        CONSTRUCTION_POP1_BUILDINGS[0],
        "light zones only StartBuilding at pop NONE, got {:#04x}",
        state.city.building_at(20, 20)
    );
}

#[test]
fn day19_xtrf_decay_formula_matches_across_engine_fidelity() {
    // Shared value-(value>>2) decay; pe_traffic_count remains Win95-only.
    let mut win_city = City::new_blank("/tmp/sc2000-d19-win.sc2", "D19W").expect("blank");
    let mut mac_city = City::new_blank("/tmp/sc2000-d19-mac.sc2", "D19M").expect("blank");
    win_city.set_traffic_density(30, 30, 80);
    mac_city.set_traffic_density(30, 30, 80);
    win_city.set_pe_traffic_count(1);
    mac_city.set_pe_traffic_count(1);
    let mut win = test_state(win_city);
    let mut mac = test_state(mac_city);
    win.engine_fidelity = EngineFidelity::Win95;
    mac.engine_fidelity = EngineFidelity::Mac68k;
    win.sim_calendar_day19_traffic();
    mac.sim_calendar_day19_traffic();
    assert_eq!(win.city.traffic_density_at(30, 30), Some(80 - (80 >> 2)));
    assert_eq!(mac.city.traffic_density_at(30, 30), Some(80 - (80 >> 2)));
    assert_eq!(
        win.city.pe_traffic_count(),
        trip_generator::traffic_density_microsim_sum(&win.city)
    );
    assert_eq!(mac.city.pe_traffic_count(), 1);
}

#[test]
fn abandoned_redevelopment_roll_matches_pe_threshold_and_draw_boundaries() {
    fn seed_for_first_crt_result(value: u16) -> u32 {
        // Invert MSVC rand's LCG so the first returned 15-bit value is exact.
        (u32::from(value) << 16)
            .wrapping_sub(0x0026_9EC3)
            .wrapping_mul(0xB9B3_3155)
    }

    for (pop, expected_threshold) in [(1, 30_000), (2, 15_000), (3, 10_000), (4, 7_500)] {
        let threshold = rci_abandoned_redevelop_threshold(0, pop);
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

    assert_eq!(rci_abandoned_redevelop_threshold(-2_000, 2), 0);
    let mut actual = Win95CrtRand::new(0x0043_6D03);
    let mut probe = actual;
    assert_eq!(
        pe_rci_growth_roll_succeeds_with_rng_always_draw(0, &mut actual),
        false,
        "zero current demand never redevelops"
    );
    probe.random_u15();
    assert_eq!(
        actual.seed(),
        probe.seed(),
        "zero threshold still draws once"
    );

    let mut isolated = Win95CrtRand::new(0x0043_6D04);
    let before = isolated.seed();
    assert!(!pe_rci_abandoned_redevelop_roll_succeeds_optional(
        0,
        20,
        20,
        GROWTH_ROLL_ABANDON_REDEVELOP_KEY,
        Some(&mut isolated),
    ));
    assert_eq!(
        isolated.seed(),
        before,
        "test isolation stays deterministic"
    );
}

#[test]
fn construction_completion_roll_matches_pe_integer_and_strict_boundaries() {
    fn seed_for_first_crt_result(value: u16) -> u32 {
        // Invert MSVC rand's LCG so the first returned 15-bit value is exact.
        // 0xB9B33155 is the multiplier inverse modulo 2^32.
        (u32::from(value) << 16)
            .wrapping_sub(0x0026_9EC3)
            .wrapping_mul(0xB9B3_3155)
    }

    for (pop, expected_threshold) in [(1, 16_384), (2, 8_192), (3, 5_461), (4, 4_096)] {
        let threshold = rci_construction_complete_threshold(pop);
        assert_eq!(threshold, expected_threshold, "pop={pop}");

        for (roll, expected_success) in [(threshold - 1, true), (threshold, false)] {
            let seed = seed_for_first_crt_result(roll as u16);
            let mut probe = Win95CrtRand::new(seed);
            assert_eq!(probe.random_u15(), roll as u16);

            let mut actual = Win95CrtRand::new(seed);
            assert_eq!(
                pe_rci_growth_roll_succeeds_with_rng(threshold, &mut actual),
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
}

#[test]
fn decline_and_complete_writeback_keep_shared_process_crt() {
    // Decline ChangeBuilding + construction-complete CAP CRT share process `0x48cdd8`.
    let mut city = City::new_blank("/tmp/sc2000-dec-crt.sc2", "DecCrt").expect("blank");
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
    set_rci_demand_for_test(&mut city, -50, 50, 50);
    let mut state = test_state(city);
    state.bypass_growth_trip_gate = false;
    state.pe_process_crt = Win95CrtRand::new(0x3333_4444);
    let before = state.pe_process_crt_seed();
    assert_eq!(state.decline_unserved_rci_zones(), 1);
    assert_eq!(state.pe_process_crt_seed(), before);

    // Construction-complete path writeback.
    let mut city = City::new_blank("/tmp/sc2000-cmp-crt.sc2", "CmpCrt").expect("blank");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT);
    stamp_transport_access_for_test(&mut city, 19, 20);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 1);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    assert_eq!(state.develop_rci_zones(), 1);
    state.pe_process_crt = Win95CrtRand::new(0x5555_6666);
    let before = state.pe_process_crt_seed();
    assert_eq!(state.complete_rci_construction_sites(), 1);
    assert_eq!(state.pe_process_crt_seed(), before);
}

#[test]
fn dense_industrial_expansion_sizes_up_at_zero_xval() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_INDUSTRIAL_DENSE);
        city.set_building(x, y, INDUSTRIAL_1X1_BUILDINGS[0]);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 0, 0, 1_000);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();
    state.city.sync_land_value_from_slice(&vec![0; CITY_TILES]);

    assert_eq!(state.develop_rci_until_built_for_test(), 4);
    let building = state.city.building_at(20, 20);
    assert!(
        INDUSTRIAL_2X2_BUILDINGS.contains(&building),
        "PE StartBuilding skips XVAL pop gates for industrial, got {building:#04x}"
    );
    for (x, y) in building_footprint_from_origin(20, 20, 2).unwrap() {
        assert_eq!(state.city.building_at(x, y), building);
    }
}

#[test]
fn dense_residential_expansion_treats_missing_xval_as_zero() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).unwrap() {
        city.set_zone(x, y, ZONE_RESIDENTIAL_DENSE);
        city.set_building(x, y, RESIDENTIAL_1X1_BUILDINGS[0]);
    }
    stamp_transport_access_for_test(&mut city, 19, 21);
    power_adjacent_building_for_test(&mut city, 20, 19);
    water_footprint_from_below_for_test(&mut city, 20, 20, 3);
    set_rci_demand_for_test(&mut city, 1_000, 0, 0);
    let mut state = test_state(city);
    state.refresh_simulation_service_bits();

    assert_eq!(state.develop_rci_until_built_for_test(), 0);
    assert_eq!(state.city.building_at(20, 20), RESIDENTIAL_1X1_BUILDINGS[0]);
}

#[test]
fn xzon_corner_check_uses_pe_position_angle() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_zone(20, 20, ZONE_RESIDENTIAL_DENSE | 0x80);
    city.set_zone(21, 20, ZONE_RESIDENTIAL_DENSE | 0x10);
    assert_eq!(PE_POSITION_ANGLE, [0x80, 0x10, 0x20, 0x40]);
    city.set_view_rotation(0);
    assert!(xzon_corner_check(&city, 20, 20));
    assert!(!xzon_corner_check(&city, 21, 20));
    city.set_view_rotation(1);
    assert!(!xzon_corner_check(&city, 20, 20));
    assert!(xzon_corner_check(&city, 21, 20));
}

#[test]
fn industrial_mix_bonus_matches_pe_max_share_formula() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(industrial_mix_bonus_from_tiles(&city), 0);

    // Single industry → 100/(1+1)=50% → (50-20)/5 = 6.
    city.set_building(10, 10, 0x84);
    assert_eq!(industrial_mix_bonus_from_tiles(&city), 6);

    // Three steel + one auto → max 300/(4+1)=60% → (60-20)/5 = 8.
    city.set_building(11, 10, 0x84);
    city.set_building(12, 10, 0x84);
    city.set_building(13, 10, 0x9f);
    assert_eq!(industrial_mix_bonus_from_tiles(&city), 8);
}

#[test]
fn police_old_arrests_and_prison_bonus_match_pe() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(police_old_arrests_for_city(&city), 0);
    assert_eq!(prison_bonus_for_city(&city), 0);

    city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 9_496);
    let police = city.provision_microsim_for_building(210).expect("police");
    let police_index = usize::from(police - MIN_SIM_TEXT_ENTRIES);
    city.refresh_structure_microsim_yearly(55_860, 100, 100, 100, 100, 100);
    let arrests = u32::from(city.microsim_at(police_index).map(|r| r.stat3).unwrap_or(0));
    assert_eq!(
        city.pe_w_old_arrests(),
        arrests as u16,
        "PE 0x44f0fd writes police arrest accumulator to BSS 0x4ca430"
    );
    assert_eq!(police_old_arrests_for_city(&city), arrests);

    let prison = city.provision_microsim_for_building(216).expect("prison");
    let prison_index = usize::from(prison - MIN_SIM_TEXT_ENTRIES);
    city.refresh_structure_microsim_yearly(55_860, 100, 100, 100, 100, 100);
    assert_eq!(
        prison_bonus_for_city(&city),
        1,
        "non-overcrowded prison (inmates/100 < 0x50) sets wPrisonBonus"
    );

    // PE overcrowding: inmate_hundreds (uncapped inmates/100) >= 0x50.
    // Seed prior-year BSS arrests so the prison case crosses the threshold.
    city.set_pe_w_old_arrests(12_000);
    city.refresh_structure_microsim_yearly(55_860, 100, 100, 100, 100, 100);
    assert!(city.microsim_at(prison_index).map(|r| r.stat3).unwrap_or(0) >= 0x50);
    assert_eq!(
        prison_bonus_for_city(&city),
        0,
        "average inmates/100 >= 0x50 clears wPrisonBonus"
    );
}

#[test]
fn manual_special_zoning_keeps_selector_footprint_rules() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    enable_special_zone_demand_for_test(&mut city);
    // The live scanner's `(step=1, substep=0)` visits (21,20). Class 0 chooses
    // the positive vertical ray from this odd/even coordinate and requires five
    // matching zone cells. Put old imported special zones later in scan order so
    // they cannot consume the target's growth roll.
    city.set_zone(80, 20, ZONE_AIRPORT);
    city.set_zone(81, 20, ZONE_SEAPORT);
    stamp_transport_access_for_test(&mut city, 20, 20);
    power_adjacent_building_for_test(&mut city, 22, 20);

    let mut developed = None;
    for seed in 0..256 {
        let mut state = test_state(city.clone());
        state.reset_zone_growth_for_city();
        state.tool = EditTool::Airport;
        assert_eq!(state.place_zone_rect((21, 20), (21, 24), ZONE_AIRPORT), 5);
        assert!((20..=24).all(|y| state.zone_growth_can_touch_tile(21, y)));
        state.pe_process_crt = Win95CrtRand::new(seed);
        state.sim_calendar_one_growth_tick(1, 0);
        if state.city.building_at(21, 20) == RUNWAY_BUILDING_ID {
            developed = Some(state);
            break;
        }
    }
    let state = developed.expect("find a process seed admitting the valid runway ray");

    assert_eq!(state.city.building_at(80, 20), 0);
    assert_eq!(state.city.building_at(81, 20), 0);
    assert_eq!(state.city.building_at(21, 20), RUNWAY_BUILDING_ID);
}

#[test]
fn zoning_drag_tracks_inclusive_rectangle_until_release() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    let mut state = test_state(city);

    state.begin_zone_drag(22, 21, 5);
    state.update_zone_drag_to((20, 20));
    let drag = state.zone_drag.expect("drag");
    let placement = state.zone_placement_for_rect(drag.start, drag.current, drag.zone_id);
    assert_eq!(placement.valid_tiles.len(), 6);
    assert!(placement.valid_tiles.contains(&(20, 20)));
    assert!(placement.valid_tiles.contains(&(22, 21)));

    assert_eq!(state.finish_zone_drag(), 6);
    assert!(state.zone_drag.is_none());
    for y in 20..=21 {
        for x in 20..=22 {
            assert_eq!(
                state.city.zone_at(x, y).unwrap().raw_bits(),
                5 | ZONE_CORNER_ALL
            );
        }
    }
}

#[test]
fn new_blank_seeds_dw_tile_count0_like_40e337() {
    // PE new-city @ `0x40e337`: after zeroing the table, `dwTileCount[0] = 0x4000`.
    // PE also zeros `0x4ca934` WORD[16] @ `0x40e301`–`0x40e319`.
    let city = City::new_blank("/tmp/sc2000-dw-blank-seed.sc2", "DwBlankSeed").expect("blank");
    assert_eq!(city.pe_dw_tile_count[0], 0x4000);
    assert_eq!(city.pe_dw_tile_count[1], 0);
    assert_eq!(city.pe_xund_occupancy, 0);
    assert_eq!(city.pe_military_tile_count, [0; 16]);
}

#[test]
fn pe_sim_wind_param_reads_bss_4cae0c() {
    let mut city = City::new_blank("/tmp/sc2000-wind-bss.sc2", "WindBss").expect("blank");
    city.set_misc_u32_at(MISC_WIND_OFFSET, 1);
    city.pe_b_weather_wind = 0x55;
    assert_eq!(pe_sim_wind_param(&city), 0x55);
}

#[test]
fn pe_sim_fusion_humid_param_reads_bss_4cb3d4() {
    let mut city = City::new_blank("/tmp/sc2000-humid-bss.sc2", "HumidBss").expect("blank");
    city.set_misc_u32_at(MISC_HUMID_OFFSET, 1);
    city.pe_b_weather_humid = 0x42;
    assert_eq!(pe_sim_fusion_humid_param(&city), 0x42);
}

#[test]
fn xgrp_latest_values_prefer_national_bss() {
    let mut city = City::new_blank("/tmp/sc2000-xgrp-nat.sc2", "XgrpNat").expect("blank");
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 2);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 3);
    city.pe_dw_national_value = 456_000;
    city.pe_dw_national_population = 123_000;
    city.pe_w_national_tax = 7;
    let latest = xgrp_history_latest_values(
        &city,
        PopulationStats::default(),
        CityServiceOutcomes::default(),
    );
    assert_eq!(latest[13], 456_000);
    assert_eq!(latest[14], 123_000);
    assert_eq!(latest[15], 7);
}

#[test]
fn pe_w_industrial_mix_bonus_bss_4ca1e8_hydrate_and_readers() {
    let path = "/tmp/sc2000-mix-bss.sc2";
    let mut city = City::new_blank(path, "MixBss").expect("blank");
    city.set_misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET, 12);
    assert_eq!(city.pe_w_industrial_mix_bonus(), 12);
    city.set_pe_w_industrial_mix_bonus(9);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_industrial_mix_bonus(), 9);
    assert_eq!(
        loaded.misc_u32_at(MISC_INDUSTRIAL_MIX_BONUS_OFFSET),
        Some(9)
    );
    assert_eq!(industrial_mix_bonus_for_city(&loaded), 9);
    // RCI zone balance reads BSS via industrial_mix_bonus_for_city @ 0x4682be.
    let mut low = City::new_blank("/tmp/sc2000-mix-bss-rci.sc2", "MixRci").expect("blank");
    low.pe_dw_residential_population = 100;
    low.set_pe_w_industrial_mix_bonus(0);
    let population = PopulationStats {
        industrial_population: 100,
        ..Default::default()
    };
    let low_i = pe_rci_zone_balance_scores(&low, population)[2];
    low.set_pe_w_industrial_mix_bonus(50);
    let high_i = pe_rci_zone_balance_scores(&low, population)[2];
    assert!(
        high_i > low_i,
        "PE FUN_00468150 @ 0x4682be reads BSS wIndustrialMixBonus @ 0x4ca1e8"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_w_old_arrests_bss_4ca430_hydrate_and_readers() {
    let path = "/tmp/sc2000-old-arrests-bss.sc2";
    let mut city = City::new_blank(path, "OldArrests").expect("blank");
    city.set_misc_u32_at(MISC_OLD_ARRESTS_OFFSET, 777);
    assert_eq!(city.pe_w_old_arrests(), 777);
    city.set_pe_w_old_arrests(1_234);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_old_arrests(), 1_234);
    assert_eq!(loaded.misc_u32_at(MISC_OLD_ARRESTS_OFFSET), Some(1_234));
    assert_eq!(police_old_arrests_for_city(&loaded), 1_234);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_w_prison_bonus_bss_4ca1dc_hydrate_and_readers() {
    let path = "/tmp/sc2000-prison-bonus-bss.sc2";
    let mut city = City::new_blank(path, "PrisonBonus").expect("blank");
    city.set_misc_u32_at(MISC_PRISON_BONUS_OFFSET, 1);
    assert_eq!(city.pe_w_prison_bonus(), 1);
    city.set_pe_w_prison_bonus(0);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_prison_bonus(), 0);
    assert_eq!(loaded.misc_u32_at(MISC_PRISON_BONUS_OFFSET), Some(0));
    let mut city = City::new_blank("/tmp/sc2000-prison-bonus-cov.sc2", "PBC").expect("blank");
    city.set_pe_w_prison_bonus(1);
    assert_eq!(prison_bonus_for_city(&city), 1);
    assert_eq!(
        pe_police_coverage_strength(100, prison_bonus_for_city(&city)),
        6 * 100 / 2,
        "PE police coverage @ 0x46b334 reads BSS wPrisonBonus (+5)"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn pe_w_sewer_bonus_bss_4cb3dc_hydrate_and_readers() {
    let path = "/tmp/sc2000-sewer-bss.sc2";
    let mut city = City::new_blank(path, "SewerBss").expect("blank");
    city.set_misc_u32_at(MISC_SEWER_BONUS_OFFSET, 1);
    assert_eq!(city.pe_w_sewer_bonus(), 1);
    city.set_pe_w_sewer_bonus(0);
    city.save(path).expect("save");
    let loaded = City::load(path).expect("load");
    assert_eq!(loaded.pe_w_sewer_bonus(), 0);
    assert_eq!(loaded.misc_u32_at(MISC_SEWER_BONUS_OFFSET), Some(0));
    let mut city = City::new_blank("/tmp/sc2000-sewer-bss2.sc2", "SewerBss2").expect("blank");
    publish_pe_sewer_bonus(&mut city);
    assert_eq!(city.pe_w_sewer_bonus(), 1);
    assert_eq!(sewer_bonus_for_pollution_blur(&city), 1);
    assert_eq!(
        pollution_blur_divisor_base_for_city(&city),
        5,
        "PE pollution blur @ 0x46a9a5 reads BSS wSewerBonus"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn weather_advance_seed_reads_bss_trend_4cb40c() {
    let mut city = City::new_blank("/tmp/sc2000-weather-seed-bss.sc2", "WxSeed").expect("blank");
    let clock = GameClock::new(1900, 1);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 7);
    city.pe_b_weather_trend = 1;
    let a = weather_advance_seed(&city, clock);
    city.pe_b_weather_trend = 2;
    let b = weather_advance_seed(&city, clock);
    assert_ne!(a, b, "weather seed folds BSS bWeatherTrend @ 0x4cb40c");
}

#[test]
fn rebuild_dw_tile_count_434f80_counts_xbld() {
    let mut city = City::new_blank("/tmp/sc2000-dw-rebuild.sc2", "DwRebuild").expect("blank");
    city.set_building(10, 10, 0x1d);
    city.set_building(11, 10, 0x1d);
    city.set_building(12, 10, 0x1d);
    city.pe_rebuild_dw_tile_count_434f80();
    assert_eq!(city.pe_dw_tile_count[0x1d], 3);
    assert_eq!(
        city.pe_dw_tile_count[0],
        (CITY_TILES as u16).wrapping_sub(3)
    );
}

#[test]
fn rebuild_dw_tile_count_434f80_counts_vacant0() {
    let mut city =
        City::new_blank("/tmp/sc2000-dw-rebuild-vac.sc2", "DwRebuildVac").expect("blank");
    city.pe_rebuild_dw_tile_count_434f80();
    assert_eq!(city.pe_dw_tile_count[0], CITY_TILES as u16);
}

#[test]
fn rebuild_dw_tile_count_434f80_resets_stale() {
    let mut city =
        City::new_blank("/tmp/sc2000-dw-rebuild-stale.sc2", "DwRebuildStale").expect("blank");
    city.pe_dw_tile_count[0xdd] = 99;
    city.pe_rebuild_dw_tile_count_434f80();
    assert_eq!(city.pe_dw_tile_count[0xdd], 0);
    assert_eq!(city.pe_dw_tile_count[0], CITY_TILES as u16);
}

#[test]
fn pe_fun_00447020_refuses_oob_like_447039() {
    // PE FUN_00447020 @ `0x447039`/`0x447044`/`0x447052`/`0x44705d` → `0x4473a3`.
    let mut city = City::new_blank("/tmp/sc2000-fun-oob.sc2", "FunOob").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_terrain(20, 20, 2);
    city.set_underground(20, 20, 0x12);
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    city.set_altitude(20, 20, 8, 0);
    let sea = pe_selected_water_height_for_fun_00447020(&city);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 0x80, 20, sea);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 0x80, sea);
    assert_eq!(city.underground_at(20, 20), 0x12);
    assert_eq!(city.terrain_at(20, 20), 2);
    assert_eq!(city.building_at(20, 20), 0x0d);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(city.altitude_at(20, 20) & 0x1f, 8);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, sea);
    assert_eq!(city.underground_at(20, 20), 0);
}

#[test]
fn mac68k_pe_fun_00447020_refuses_oob_like_447039() {
    let mut city = City::new_blank("/tmp/sc2000-mac-fun-oob.sc2", "MacFunOob").expect("blank");
    city.set_building(20, 20, 0x0d);
    city.set_underground(20, 20, 0x11);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let sea = pe_selected_water_height_for_fun_00447020(&state.city);
    pe_fun_00447020_mode0_xund_slope_sea(&mut state.city, 0x80, 20, sea);
    pe_fun_00447020_mode0_xund_slope_sea(&mut state.city, 20, 0x80, sea);
    assert_eq!(state.city.underground_at(20, 20), 0x11);
}

#[test]
fn mac68k_pe_fun_00447020_deep_wet_keeps_radioactivity_like_447248() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-fun-rad-deep.sc2", "MacFunRadDeep").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 20);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 9);
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(city.terrain_at(20, 20), slope.wrapping_add(0x10));
}

#[test]
fn mac68k_pe_fun_00447020_shallow_wet_keeps_radioactivity_like_447248() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-fun-rad-shallow.sc2", "MacFunRadShallow").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_altitude(21, 20, 5, 0);
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    let slope = pe_fun_00447020_slope_xter(&city, 20, 20);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 5);
    assert_eq!(city.building_at(20, 20), 5);
    assert_eq!(city.terrain_at(20, 20), slope.wrapping_add(0x20));
}

#[test]
fn pe_fun_00447020_wet_keeps_radioactivity_xbld_5_like_447210() {
    // PE FUN_00447020 wet PlaceTile(0) @ `0x447248`/`0x447399`: skip when XBLD == 5.
    let mut city = City::new_blank("/tmp/sc2000-fun-rad.sc2", "FunRad").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    let sea = 9i16;
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, sea);
    assert_eq!(
        city.building_at(20, 20),
        5,
        "radioactivity must survive wet PlaceTile skip"
    );
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
    assert_eq!(city.water_height_at(20, 20), sea as u8);
}

#[test]
fn mac68k_pe_fun_00447020_wet_keeps_radioactivity_xbld_5_like_447210() {
    let mut city = City::new_blank("/tmp/sc2000-mac-fun-rad.sc2", "MacFunRad").expect("blank");
    for dx in -1isize..=1 {
        for dy in -1isize..=1 {
            city.set_altitude((20 + dx) as usize, (20 + dy) as usize, 4, 0);
        }
    }
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 1);
    pe_fun_00447020_mode0_xund_slope_sea(&mut city, 20, 20, 9);
    assert_eq!(city.building_at(20, 20), 5);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}
