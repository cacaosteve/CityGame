use super::*;
use crate::app::tests::{blank_city_layout_fixture, test_state};

fn reopen_with_actor_fixture_seed(state: &mut crate::app::ViewerState, path: &std::path::Path) {
    // These native actor slices restart at the worker with a supplied seed.
    // Apply the saved map and actor state, then inject that post-open seed.
    let fixture_rng = state.mac_process_rng;
    let reopened = state.load_city(path, (1280, 800), 1.0);
    std::fs::remove_file(path).unwrap();
    reopened.unwrap();
    state.mac_process_rng = fixture_rng;
}

#[test]
fn mac_action_completion_polls_before_the_next_calendar_dispatch() {
    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = City::new_blank("/tmp/mac-action-poll.sc2", "Poll").unwrap();
        city.kind = kind;
        for (offset, value) in [
            (0x48, 10),
            (0x4c, 25),
            (0xfa4, 12),
            (0x1004, 0),
            (0x1008, 0),
            (0x102c, 1000),
        ] {
            city.set_mac_state_misc_u32_at(offset, value);
        }
        city.set_budget_misc_u32_values(0x0780, &[7]);
        for (index, score) in (4..).zip([10, 20, 40, 30]) {
            city.set_chunk_u32_values("XGRP", index * 52 * 4, &[score]);
        }
        city.set_microsim_tile_id(10, 0xf3);
        city.set_microsim_stat2(10, 63);
        for (suppressed, seed, approval) in [(false, 200_870_715, 29), (true, 123, 0)] {
            let mut city = city.clone();
            city.set_mac_newspaper_suppressed(suppressed);
            scenario_action::begin_action(&mut city, 3);
            let mut rng = crate::mac_random::MacToolboxRng::new(123);
            let pass = scenario_action::advance_action(&mut city, &mut rng, &mut 0, &mut 0);
            assert!(pass.complete);
            assert_eq!(city.mac_state_misc_u32_at(4), Some(1));
            assert_eq!(rng.seed(), seed);
            assert!(!scenario_action::advance_action(&mut city, &mut rng, &mut 0, &mut 0).complete);
            city.refresh_mac_monthly_random_xmic_rows(100, 100, 100, 100, 100, false, &mut rng);
            assert_eq!(rng.seed(), seed);
            assert_eq!(city.microsim_at(10).unwrap().stat2, approval);
        }
    }
}

#[test]
fn saved_flint_mac_rail_sweep_matches_original_on_both_city_layouts() {
    use crate::city::CityFileKind;
    use crate::mac_random::{MacGameLcgRng, MacToolboxRng};
    use crate::win95_overlay::Win95OverlayRecord;

    let fixture = include_str!(
        "../../../tests/fixtures/simulation/mac/mac_saved_flint_actor_sweep_fixture.txt"
    );
    let values: std::collections::HashMap<_, _> = fixture
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect();
    let input = |key: &str| values.get(key).copied().expect("fixture field");
    let parse_pairs = |value: &str| {
        value
            .split(',')
            .map(|pair| {
                let (offset, byte) = pair.split_once(':').expect("offset:value pair");
                (
                    usize::from_str_radix(offset, 16).expect("hex offset"),
                    u8::from_str_radix(byte, 16).expect("hex byte"),
                )
            })
            .collect::<Vec<_>>()
    };

    let initial_bytes = (0..)
        .step_by(2)
        .take(input("xthg_input").len() / 2)
        .map(|i| u8::from_str_radix(&input("xthg_input")[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(initial_bytes.len(), 40 * 12);

    // The native sound callback is reached only when the QuickDraw low byte
    // is zero. Seeds zero, one, seven, and seventeen cover both gate outcomes
    // recorded by the original-instruction replay while preserving identical
    // map/actor writes. These are controlled stream positions, not
    // natural-session frequency estimates.
    for (qd_seed, expected_qd_final, expected_train_sounds) in [
        (0, 0, 5),
        (1, 0x4431_b782, 0),
        (7, 0x5d5c_0491, 0),
        (17, 0x074d_2fab, 0),
    ] {
        for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
            let mut city =
                City::new_blank("/tmp/saved-flint-actor-sweep.sc2", "Flint").expect("blank city");
            city.kind = kind;
            for (offset, value) in parse_pairs(input("build")) {
                // CODE 6's native plane pointer is tile_x*128+tile_y, which
                // matches City's x-major accessors directly.
                city.set_building(offset / 128, offset % 128, value);
            }
            for (offset, value) in parse_pairs(input("xtxt_input")) {
                city.set_text_overlay(offset / 128, offset % 128, value);
            }
            for (slot, bytes) in initial_bytes.chunks_exact(12).enumerate() {
                city.set_mac_object_slot_record(slot, Win95OverlayRecord::from_bytes(bytes));
            }

            let mut quickdraw = MacToolboxRng::new(qd_seed);
            let mut game_rng = MacGameLcgRng::new(1);
            let mut sound_deadline = 0;
            let mut tick_count = 0;
            let sounds = tick_mac_actors_with_rng_and_clock(
                &mut city,
                &mut quickdraw,
                &mut game_rng,
                &mut sound_deadline,
                0,
                &mut tick_count,
                &mut None,
            );

            let expected_changes: std::collections::HashMap<_, _> = input("xthg_changed")
                .split(',')
                .map(|pair| {
                    let (slot, hex) = pair.split_once(':').unwrap();
                    let bytes = (0..hex.len())
                        .step_by(2)
                        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                        .collect::<Vec<_>>();
                    (
                        slot.parse::<usize>().unwrap(),
                        Win95OverlayRecord::from_bytes(&bytes),
                    )
                })
                .collect();
            for (slot, actual) in city.traffic_overlays().unwrap().iter().enumerate() {
                let expected = expected_changes.get(&slot).copied().unwrap_or_else(|| {
                    Win95OverlayRecord::from_bytes(&initial_bytes[slot * 12..slot * 12 + 12])
                });
                assert_eq!(*actual, expected, "layout={kind:?}, actor slot={slot}");
            }

            let expected_xtxt: std::collections::HashMap<_, _> =
                parse_pairs(input("xtxt_changed")).into_iter().collect();
            for (offset, value) in parse_pairs(input("xtxt_input")) {
                let expected = expected_xtxt.get(&offset).copied().unwrap_or(value);
                assert_eq!(
                    city.text_overlay_at(offset / 128, offset % 128),
                    Some(expected),
                    "layout={kind:?}, XTXT offset={offset:#06x}"
                );
            }
            assert_eq!(
                quickdraw.seed(),
                expected_qd_final,
                "layout={kind:?}, seed={qd_seed}"
            );
            assert_eq!(
                sounds,
                vec![PlacementSound::Train; expected_train_sounds],
                "native sound selectors are five 0x020c requests for seed zero and none for seeds 1, 7, or 17; layout={kind:?}"
            );
            let mut expected_game_rng = MacGameLcgRng::new(1);
            for _ in 0..input("game_lcg_calls").parse().unwrap() {
                expected_game_rng.bounded(2);
            }
            assert_eq!(
                game_rng.state(),
                expected_game_rng.state(),
                "layout={kind:?}"
            );
        }
    }
}

#[test]
#[ignore = "requires CITYGAME_TEST_FLINT_SCENARIO pointing to a locally imported FLINT.SCN"]
fn mac_actor_sweep_matches_native_on_imported_win95_flint_scenario() {
    use crate::city::{City as CityModel, CityFileKind};
    use crate::mac_random::{MacGameLcgRng, MacToolboxRng};

    // This is a real Win95-format scenario loaded under the Mac actor engine.
    // Native CODE 6 reads the already-decoded XTHG/XTXT/map planes; it does
    // not choose actor semantics from the container's file kind.
    let path = std::env::var_os("CITYGAME_TEST_FLINT_SCENARIO")
        .expect("set CITYGAME_TEST_FLINT_SCENARIO to a locally imported FLINT.SCN");
    let mut city = CityModel::load(std::path::Path::new(&path))
        .expect("load locally imported Win95 Flint scenario");
    assert_eq!(city.kind, CityFileKind::WinScenario);

    let fixture = include_str!(
        "../../../tests/fixtures/simulation/mac/mac_saved_flint_actor_sweep_fixture.txt"
    );
    let values: std::collections::HashMap<_, _> = fixture
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect();
    let input = |key: &str| values.get(key).copied().expect("fixture field");
    let bytes = |hex: &str| {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("fixture hex"))
            .collect::<Vec<_>>()
    };
    let native_input = bytes(input("xthg_input"));
    let native_changes: std::collections::HashMap<usize, Vec<u8>> = input("xthg_changed")
        .split(',')
        .map(|pair| {
            let (slot, record) = pair.split_once(':').expect("slot:record");
            (slot.parse().expect("slot index"), bytes(record))
        })
        .collect();
    assert_eq!(native_input.len(), 40 * 12);
    for (slot, record) in city.traffic_overlays().unwrap().iter().enumerate() {
        assert_eq!(
            record.to_bytes(),
            &native_input[slot * 12..slot * 12 + 12],
            "Win95 source XTHG slot {slot} before the sweep"
        );
    }

    let mut quickdraw = MacToolboxRng::new(1);
    let mut game_rng = MacGameLcgRng::new(1);
    let mut sound_deadline = 0;
    let mut tick_count = 0;
    let sounds = tick_mac_actors_with_rng_and_clock(
        &mut city,
        &mut quickdraw,
        &mut game_rng,
        &mut sound_deadline,
        0,
        &mut tick_count,
        &mut None,
    );

    // The native Win95-Flint-input replay has the same controlled QuickDraw
    // seed-1 XTHG/XTXT effects and game-LCG call count as the Mac-Flint slice.
    for (slot, record) in city.traffic_overlays().unwrap().iter().enumerate() {
        let expected = native_changes
            .get(&slot)
            .map(Vec::as_slice)
            .unwrap_or(&native_input[slot * 12..slot * 12 + 12]);
        assert_eq!(record.to_bytes(), expected, "Win95 source XTHG slot {slot}");
    }
    assert_eq!(quickdraw.seed(), 0x4431_b782);
    assert!(sounds.is_empty());
    let expected_game_rng = (0..5).fold(MacGameLcgRng::new(1), |mut rng, _| {
        rng.bounded(2);
        rng
    });
    assert_eq!(game_rng.state(), expected_game_rng.state());
    let native_xtxt: std::collections::HashMap<usize, u8> = input("xtxt_changed")
        .split(',')
        .map(|pair| {
            let (offset, value) = pair.split_once(':').expect("offset:value");
            (
                usize::from_str_radix(offset, 16).expect("XTXT offset"),
                u8::from_str_radix(value, 16).expect("XTXT value"),
            )
        })
        .collect();
    for (offset, value) in native_xtxt {
        assert_eq!(
            city.text_overlay_at(offset / 128, offset % 128),
            Some(value),
            "Win95 source XTXT offset {offset:#06x}"
        );
    }
}

#[test]
fn mac_loop_admission_matches_original_instruction_fixture() {
    let mut cases = 0;
    for (line, row) in include_str!("../fixtures/mac_loop_admission_20261023.csv")
        .lines()
        .enumerate()
        .skip(1)
    {
        let v: Vec<i64> = row.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 14);
        let mut scheduler = MacLoopScheduler {
            deadline: v[2] as u32,
            dispatch_counter: v[5] as u16,
            ..MacLoopScheduler::default()
        };
        let dispatch = scheduler.admit_at(v[0] as i16, v[1] as i16, v[3] as u32, v[4] as u32);
        if dispatch.is_some() {
            scheduler.finish_dispatch();
        }
        let calendar = dispatch == Some(MacLoopDispatch::Calendar);
        // Native mode 1 checks pending even if speed/clock did not admit
        // calendar. A calendar callback may change the pending selector.
        let pending = if calendar { v[7] } else { v[6] };
        let action = dispatch == Some(MacLoopDispatch::Action) || (v[0] == 1 && pending != 0);
        assert_eq!(
            (i64::from(calendar), i64::from(action)),
            (v[8], v[9]),
            "native row {}",
            line + 1
        );
        assert_eq!(scheduler.deadline, v[10] as u32, "native row {}", line + 1);
        assert_eq!(
            scheduler.dispatch_counter,
            v[11] as u16,
            "native row {}",
            line + 1
        );
        cases += 1;
    }
    assert_eq!(cases, 11_520);
}

#[test]
fn mac_run_state_two_counts_due_action_without_advancing_calendar() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let path = if kind == CityFileKind::MacCity {
            std::path::Path::new("/tmp/mac-action-mode.city")
        } else {
            std::path::Path::new("/tmp/mac-action-mode.sc2")
        };
        let mut city = City::new_blank(path, "MacActionMode").expect("blank city");
        city.kind = kind;
        city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, 2);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.game_clock = GameClock::new(1900, 1);
        state.traffic.set_speed(GameSpeed::Cheetah);
        let clock_before = state.game_clock;

        assert_eq!(
            state
                .advance_mac_frame_simulation(Duration::from_secs(1))
                .days,
            0,
            "layout={kind:?}"
        );
        assert_eq!(state.game_clock, clock_before, "layout={kind:?}");
        assert_eq!(
            state.mac_loop_scheduler.dispatch_counter, 1,
            "the due mode-2 action callback returns before CODE 2's counter increment; layout={kind:?}"
        );
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(2),
            "layout={kind:?}"
        );
    }
}

#[test]
fn mac_run_state_one_clears_unknown_selector_then_advances_only_an_active_action() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (active, expected_seed) in [(false, 1), (true, 16_807)] {
            let mut city =
                City::new_blank("/tmp/mac-mode-one-unknown.sc2", "Unknown").expect("blank city");
            city.kind = kind;
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, 1);
            city.set_mac_state_misc_u32_at(0x1044, 12);
            city.set_mac_state_misc_u32_at(0x1048, u32::from(active));
            city.set_mac_pending_action(19);
            city.set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 327_630_000);
            city.set_text_overlay(3, 79, 7);
            city.set_text_overlay(20, 30, 0xff);

            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.mac_process_rng = MacToolboxRng::new(1);
            state.mac_loop_scheduler.disaster_point = (42, 40);
            state.mac_loop_scheduler.actor_deadline = u32::MAX;
            state.mac_loop_scheduler.deadline = 2;
            state.traffic.set_speed(GameSpeed::Paused);
            let clock = state.game_clock;

            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(17))
                    .days,
                0
            );
            assert_eq!(state.city.mac_pending_action(), 0, "layout={kind:?}");
            assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(12));
            assert_eq!(
                state.city.mac_state_misc_u32_at(0x1048),
                Some(u32::from(active))
            );
            assert_eq!(state.city.text_overlay_at(3, 79), Some(7));
            assert_eq!(state.city.text_overlay_at(20, 30), Some(0xff));
            assert_eq!(
                state.mac_process_rng.seed(),
                expected_seed,
                "layout={kind:?}"
            );
            assert_eq!(state.game_clock, clock);
        }
    }
}

#[test]
fn mac_kind2_sound_deadline_survives_opening_another_city() {
    let start_path = std::path::Path::new("/tmp/mac-kind2-deadline-start.city");
    let mut state = test_state(City::new_blank(start_path, "Start").expect("blank start city"));
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_kind2_sound_deadline = 0x1234_5678;

    let path = std::env::temp_dir().join(format!(
        "citygame-mac-kind2-deadline-load-{}.city",
        std::process::id()
    ));
    let mut replacement = City::new_blank(&path, "Replacement").expect("blank replacement");
    replacement.kind = CityFileKind::MacCity;
    replacement.save(&path).expect("save replacement city");

    state
        .load_city(&path, (1024, 768), 1.0)
        .expect("load replacement city");
    assert_eq!(state.mac_kind2_sound_deadline, 0x1234_5678);
    std::fs::remove_file(path).expect("remove replacement city");
}

#[test]
fn mac_tickcount_keeps_fractional_ticks_and_wraps_as_a_long() {
    let mut scheduler = MacLoopScheduler::default();
    assert_eq!(scheduler.advance_elapsed(Duration::from_millis(16)), 0);
    assert_eq!(scheduler.advance_elapsed(Duration::from_millis(1)), 1);
    assert_eq!(scheduler.advance_elapsed(Duration::ZERO), 1);
    let mut wrap = MacLoopScheduler::default();
    let at_wrap = Duration::from_nanos((u64::from(u32::MAX) + 1) * 1_000_000_000 / 60 + 1);
    assert_eq!(wrap.advance_elapsed(at_wrap), 0);
}

#[test]
fn mac_actor_timer_uses_scaled_dispatch_count_and_strict_due_gate() {
    let mut slow = MacLoopScheduler::default();
    slow.dispatch_counter = 2;
    assert!(!slow.due_actor_sweep(1, 2, 0)); // Entry initializes the deadline at TickCount 0.
    assert!(!slow.due_actor_sweep(1, 2, 0)); // Native comparison is strict.
    assert!(slow.due_actor_sweep(1, 2, 1));
    assert_eq!(slow.actor_smoother, 1); // (7*0 + 4*2) / 8.
    assert_eq!(slow.dispatch_counter, 0);
    assert_eq!(slow.actor_deadline, 46); // smoothed < 6 → +45.

    let mut fast = MacLoopScheduler {
        actor_deadline: 0,
        dispatch_counter: 12,
        ..MacLoopScheduler::default()
    };
    assert!(fast.due_actor_sweep(1, 4, 1));
    assert_eq!(fast.actor_smoother, 6); // (7*0 + 4*12) / 8.
    assert_eq!(fast.actor_deadline, 31); // smoothed >= 6 → +30.

    let mut action = MacLoopScheduler {
        actor_deadline: 0,
        ..MacLoopScheduler::default()
    };
    assert!(action.due_actor_sweep(2, 2, 1));
    assert_eq!(action.actor_deadline, 31); // Run-state 2 forces +30.

    let mut editor = MacLoopScheduler {
        actor_deadline: 0,
        ..MacLoopScheduler::default()
    };
    assert!(editor.due_actor_sweep(0, 2, 1)); // Native actor gate ignores run state.
    assert_eq!(editor.actor_deadline, 46);
}

#[test]
fn mac_actor_timer_uses_fresh_tick_for_next_deadline() {
    let mut scheduler = MacLoopScheduler {
        actor_deadline: 10,
        ..MacLoopScheduler::default()
    };
    let mut refresh_calls = 0;

    assert!(scheduler.due_actor_sweep_with_refresh(1, 2, 11, || {
        refresh_calls += 1;
        17
    }));
    assert_eq!(refresh_calls, 1);
    assert_eq!(scheduler.actor_deadline, 62); // Fresh TickCount 17 + native +45.

    let mut not_due = MacLoopScheduler {
        actor_deadline: 17,
        ..MacLoopScheduler::default()
    };
    assert!(!not_due.due_actor_sweep_with_refresh(1, 2, 17, || {
        panic!("native second TickCount is not read when the actor sweep is not due")
    }));
    assert_eq!(not_due.actor_deadline, 17);
}

#[test]
fn mac_calendar_timer_uses_fresh_tick_for_next_deadline() {
    let mut state = mac_phase24_fixture("mac-calendar-refresh.city", WEATHER_TREND_BLIZZARD, 1);
    state.city.set_mac_pending_action(0);
    state.traffic.set_speed(GameSpeed::Turtle);
    state.mac_loop_scheduler.deadline = 0;

    let advance = state.finish_mac_frame_calendar_phase_with_tick_refresh(
        Duration::from_millis(200),
        0,
        || Duration::from_millis(17),
    );
    assert_eq!(advance.days, 1);
    // Check sample is tick 12; refresh sample is tick 13; native Turtle adds 36.
    assert_eq!(state.mac_loop_scheduler.deadline, 49);

    let mut not_due = mac_phase24_fixture(
        "mac-calendar-refresh-not-due.city",
        WEATHER_TREND_BLIZZARD,
        1,
    );
    not_due.city.set_mac_pending_action(0);
    not_due.traffic.set_speed(GameSpeed::Turtle);
    not_due.mac_loop_scheduler.deadline = 12;
    assert_eq!(
        not_due.finish_mac_frame_calendar_phase_with_tick_refresh(
            Duration::from_millis(200),
            0,
            || panic!("native second TickCount is not read when calendar is not due"),
        ),
        GameClockAdvance::default()
    );
    assert_eq!(not_due.mac_loop_scheduler.deadline, 12);
}

#[test]
fn mac_actor_timer_keeps_native_unsigned_tickcount_comparison_at_wrap() {
    let mut scheduler = MacLoopScheduler {
        actor_deadline: u32::MAX - 10,
        ..MacLoopScheduler::default()
    };

    // CODE 2 compares the current TickCount with the stored deadline using
    // unsigned CMP.L/BLS semantics. If execution misses the due window and
    // TickCount wraps, the low post-wrap values remain before that old
    // unsigned deadline until the counter catches up on its next cycle.
    assert!(!scheduler.due_actor_sweep(1, 2, u32::MAX - 10));
    assert!(!scheduler.due_actor_sweep(1, 2, 0));
    assert!(!scheduler.due_actor_sweep(1, 2, 1));
    assert!(scheduler.due_actor_sweep(1, 2, u32::MAX - 9));
    assert_eq!(scheduler.actor_deadline, 35);
}

#[test]
fn mac_rail_head_dispatch_is_engine_scoped_not_city_layout_scoped() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-rail-sweep-cross-layout.sc2"),
            "MacRailSweep",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        // The seed-1 app LCG selects the native (-1, 0) neighbor from this
        // head's x-major coordinates.
        city.set_building(49, 50, 0xed);
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 10,
                lane: 0,
                tile_x: 50,
                tile_y: 50,
                ..Default::default()
            },
        );
        let mut quickdraw = MacToolboxRng::new(1);
        let quickdraw_before = quickdraw.seed();
        let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
        let mut expected_game_rng = game_rng;
        assert_eq!(expected_game_rng.bounded(2), 0);

        let sounds = tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);

        assert_eq!(sounds, Vec::<PlacementSound>::new(), "{kind:?}");
        assert_eq!(quickdraw.seed(), quickdraw_before, "{kind:?}");
        assert_eq!(game_rng, expected_game_rng, "{kind:?}");
        assert_eq!(city.traffic_overlays().unwrap()[slot].kind, 10, "{kind:?}");
        assert_eq!(city.mac_rail_actor_counter, 1, "{kind:?}");
    }
}

#[test]
fn mac_rail_sweep_keeps_pre_handler_count_when_consist_retires() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-rail-sweep-retire.sc2"),
            "MacRailRetire",
        )
        .expect("blank city");
        city.kind = kind;
        let head = XTHG_FIRST_USABLE_SLOT;
        let middle = head + 1;
        let tail = head + 2;
        for (slot, x) in [(head, 30_u8), (middle, 31), (tail, 32)] {
            city.set_text_overlay(usize::from(x), 30, (0xc9 + slot) as u8);
            city.set_mac_object_slot_record(
                slot,
                crate::win95_overlay::Win95OverlayRecord {
                    kind: if slot == head { 10 } else { 11 },
                    lane: 0,
                    tile_x: 30,
                    tile_y: x,
                    state: match slot {
                        s if s == head => middle as u8,
                        s if s == middle => tail as u8,
                        _ => 0,
                    },
                    sub_x: 30,
                    sub_y: x,
                    ..Default::default()
                },
            );
        }

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        let quickdraw_before = quickdraw.seed();
        let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
        let mut expected_game_rng = game_rng;
        assert_eq!(expected_game_rng.bounded(2), 0);

        let sounds = tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);

        assert_eq!(city.mac_rail_actor_counter, 1, "{kind:?}");
        assert_eq!(city.traffic_overlays().unwrap()[head].kind, 6, "{kind:?}");
        assert_eq!(city.traffic_overlays().unwrap()[head].lane, 0, "{kind:?}");
        assert!(sounds.is_empty(), "{kind:?}");
        assert_eq!(quickdraw.seed(), quickdraw_before, "{kind:?}");
        assert_eq!(game_rng, expected_game_rng, "{kind:?}");
    }
}

#[test]
fn mac_kind_nine_actor_activates_then_is_removed_on_next_sweep() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-nine-sweep.sc2"),
        "MacKindNineSweep",
    )
    .expect("blank city");
    let (x, y, slot) = (30, 31, XTHG_FIRST_USABLE_SLOT);
    city.set_tile_bits(x, y, TileBits::new(0x04));
    city.set_text_overlay(x, y, (0xc9 + slot) as u8);
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: XTHG_SAILBOAT,
        tile_x: x as u8,
        tile_y: y as u8,
        sub_x: 4,
        sub_y: 4,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };

    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;
    let random_word = expected.random_u16();
    assert_eq!(random_word, 0xe99e); // The signed interpretation is negative.
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0); // Zoom does not gate this update.
    let active = city.traffic_overlays().unwrap()[slot];
    assert_eq!(active.kind, XTHG_SAILBOAT);
    assert_eq!(active.state, 1);
    assert_eq!(active.lane, mac_kind9_turn(0, random_word));
    assert_eq!(quickdraw.seed(), expected.seed());
    assert_eq!(city.mac_marina_actor_counter, 1);

    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);
    assert_eq!(city.traffic_overlays().unwrap()[slot].kind, 0);
    assert_eq!(city.text_overlay_at(x, y), Some(0));
    assert_eq!(quickdraw.seed(), expected.seed());
    assert_eq!(city.mac_marina_actor_counter, 1);
}

#[test]
fn mac_export80_underlay_clear_is_observed_by_later_kind3_actor_sweep_on_both_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-export80-kind3-interleave.sc2"),
            "MacExport80Kind3",
        )
        .expect("blank city");
        city.kind = kind;
        if kind == CityFileKind::WinSc2 {
            city.prepare_mac_fidelity_from_saved_city();
        }

        let (x, y, slot) = (40, 40, 7);
        let proxy = 0xc9 + slot as u8;
        city.set_building(x, y, 0x70); // Native generic extent-1 export-80 case.
        city.set_terrain(x, y, 1); // Nonzero XTER means PlaceTile(0).
        city.set_text_overlay(x, y, proxy);
        city.set_tile_bits(x, y, TileBits::new(0x04));
        city.set_terrain(x + 3, y, 0x10);
        city.set_tile_bits(x + 3, y, TileBits::new(0x04));
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 2,
            state: 0,
            tile_x: y as u8,
            tile_y: x as u8,
            sub_x: 13,
            sub_y: 8,
            target_x: 80,
            target_y: x as u8,
            saved_a: 0x55,
            ..Default::default()
        };

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        crate::app::budget::mac_destroy_building_export80(&mut city, x, y, &mut quickdraw, false);
        assert_eq!(city.text_overlay_at(x, y), Some(proxy), "{kind:?}");
        assert_eq!(
            city.traffic_overlays().unwrap()[slot].saved_a,
            0,
            "{kind:?}"
        );
        assert_ne!(
            city.tile_bits_at(x, y).unwrap().raw_bits() & 0x04,
            0,
            "export-80 must leave kind-3 enabled at its current tile ({kind:?})"
        );

        let _ = tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );

        let actor = city.traffic_overlays().unwrap()[slot];
        assert_eq!(actor.kind, 3, "{kind:?}");
        assert_eq!(actor.tile_x, (x + 1) as u8, "{kind:?}");
        assert_eq!(actor.tile_y, y as u8, "{kind:?}");
        assert_eq!(actor.saved_a, 0, "{kind:?}");
        assert_eq!(city.text_overlay_at(x, y), Some(0), "{kind:?}");
        assert_eq!(city.text_overlay_at(x + 1, y), Some(proxy), "{kind:?}");
    }
}

#[test]
fn mac_actor_sweep_kind3_handoff_matches_across_city_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind3-cross-layout.sc2"),
            "MacKind3CrossLayout",
        )
        .expect("blank city");
        city.kind = kind;
        city.set_tile_bits(30, 30, TileBits::new(0));
        city.set_mac_object_slot_record(
            XTHG_FIRST_USABLE_SLOT,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 3,
                state: 0,
                tile_x: 30,
                tile_y: 30,
                ..Default::default()
            },
        );
        city.set_mac_object_slot_record(
            XTHG_FIRST_USABLE_SLOT + 1,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 6,
                lane: 0,
                tile_x: 40,
                tile_y: 40,
                ..Default::default()
            },
        );

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let records = city.traffic_overlays().expect("actor table");
        outcomes.push((
            records[XTHG_FIRST_USABLE_SLOT],
            records[XTHG_FIRST_USABLE_SLOT + 1],
            city.mac_kind3_actor_counter,
            city.mac_marina_actor_counter,
            quickdraw.seed(),
            sounds,
        ));
    }

    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0].0.kind, 6);
    assert_eq!(outcomes[0].0.state, 0);
    assert_eq!(outcomes[0].1.lane, 1);
    assert_eq!(outcomes[0].2, 1);
}

#[test]
fn mac_tornado_kind15_uses_native_turn_cleanup_and_runs_on_both_city_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-tornado-kind15-cross-layout.sc2"),
            "MacTornado",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        let (x, y) = (50, 50);
        city.set_text_overlay(x, y, 5);
        city.set_text_overlay(x + 1, y, 0xc9u8.wrapping_add(slot as u8));
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: XTHG_TORNADO,
                lane: 2,
                tile_x: y as u8,
                tile_y: x as u8,
                sub_x: 9,
                sub_y: 8,
                saved_a: 5,
                ..Default::default()
            },
        );

        let mut quickdraw = MacToolboxRng::new(0);
        let mut expected_quickdraw = quickdraw;
        for _ in 0..6 {
            assert_eq!(expected_quickdraw.random_u16(), 0);
        }
        let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
        let mut now = 100;
        let mut sound_deadline = 0;

        let sounds = tick_mac_actors_with_rng_and_clock(
            &mut city,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
            0,
            &mut now,
            &mut None,
        );

        assert!(sounds.is_empty(), "{kind:?}");
        assert_eq!(quickdraw.seed(), expected_quickdraw.seed(), "{kind:?}");
        assert_eq!(city.mac_tornado_actor_counter, 1, "{kind:?}");
        let tornado = city.traffic_overlays().expect("actor table")[slot];
        assert_eq!(tornado.kind, 0, "the low-byte-zero Random gate retires it");
        assert_eq!(
            tornado.lane, 2,
            "the mover does not persist its heading arg"
        );
        assert_eq!(
            tornado.tile_x, 52,
            "the optional second movement still runs"
        );
        assert_eq!(
            city.text_overlay_at(x, y),
            Some(5),
            "origin underlay restored"
        );
        assert_eq!(
            city.text_overlay_at(x + 1, y),
            Some(0xc9u8.wrapping_add(slot as u8)),
            "the mover skips the occupied proxy tile before export 121 clears its resulting cell"
        );
        assert_eq!(city.text_overlay_at(x + 2, y), Some(0));
        assert_eq!(now, 100, "empty XBLD does not enter export 80");
    }
}

#[test]
fn mac_tornado_cleanup_tests_low_byte_of_random_word_on_both_city_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-tornado-low-byte-cleanup.sc2"),
            "MacTornadoLowByteCleanup",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        let (x, y) = (50, 50);
        city.set_text_overlay(x, y, 0xc9u8.wrapping_add(slot as u8));
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: XTHG_TORNADO,
                lane: 2,
                tile_x: y as u8,
                tile_y: x as u8,
                sub_x: 9,
                sub_y: 8,
                saved_a: 5,
                ..Default::default()
            },
        );

        // The first two values choose the initial turn. The third is 0xbe00:
        // native BFTST (SP){8:8} observes its zero low byte and retires the
        // actor. A whole-word-zero port would incorrectly keep it alive.
        let mut quickdraw = MacToolboxRng::new(87);
        let expected_words: Vec<_> = (0..6).map(|_| quickdraw.random_u16()).collect();
        assert_eq!(expected_words[2], 0xbe00);
        assert_eq!(expected_words[2] & 0x00ff, 0);
        assert_ne!(expected_words[2], 0);
        assert_ne!(expected_words[5] & 0x00ff, 0);
        let expected_seed = quickdraw.seed();
        let mut quickdraw = MacToolboxRng::new(87);
        let mut game_rng = MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE);
        let mut now = 100;
        let mut sound_deadline = 0;

        let _ = tick_mac_actors_with_rng_and_clock(
            &mut city,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
            0,
            &mut now,
            &mut None,
        );

        let tornado = city.traffic_overlays().expect("actor table")[slot];
        assert_eq!(tornado.kind, 0, "the native low-byte gate retires {kind:?}");
        assert_eq!(quickdraw.seed(), expected_seed, "{kind:?}");
    }
}

#[test]
fn mac_tornado_heading_turns_match_the_two_native_arithmetic_forms() {
    assert_eq!(mac_tornado_heading_mod3(7, 2, 1), 0);
    assert_eq!(mac_tornado_heading_mod3(0, 1, 2), 7);
    assert_eq!(mac_tornado_heading_bit(7, 1, 0), 0);
    assert_eq!(mac_tornado_heading_bit(0, 0, 1), 7);
}

#[test]
fn mac_kind2_forced_state5_countdown_matches_across_city_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let path = if kind == CityFileKind::MacCity {
            std::path::Path::new("/tmp/mac-kind2-phase5.city")
        } else {
            std::path::Path::new("/tmp/mac-kind2-phase5.sc2")
        };
        let mut city = City::new_blank(path, "MacKind2Phase5").expect("blank city");
        city.kind = kind;
        let first = XTHG_FIRST_USABLE_SLOT;
        city.set_mac_object_slot_record(
            first,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 2,
                state: 5,
                lane: 7,
                height: 4,
                ..Default::default()
            },
        );
        city.set_mac_object_slot_record(
            first + 1,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 2,
                state: 5,
                lane: 6,
                height: 2,
                ..Default::default()
            },
        );

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        let quickdraw_before = quickdraw.seed();
        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let records = city.traffic_overlays().expect("actor table");
        outcomes.push((
            records[first],
            records[first + 1],
            city.mac_kind2_actor_counter,
            quickdraw.seed(),
            sounds,
        ));
        assert_eq!(quickdraw.seed(), quickdraw_before, "layout={kind:?}");
    }

    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0].0.lane, 1);
    assert_eq!(outcomes[0].0.height, 3);
    assert_eq!(outcomes[0].1.kind, 6);
    assert_eq!(outcomes[0].1.state, 0x11);
    assert_eq!(outcomes[0].1.saved_b, 1);
    assert_eq!(outcomes[0].1.lane, 0);
    assert_eq!(outcomes[0].2, 2);
    assert_eq!(outcomes[0].4, vec![PlacementSound::Mayday]);
}

#[test]
fn mac_kind2_state_two_mover_and_xtrf_sound_match_across_city_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let path = if kind == CityFileKind::MacCity {
            std::path::Path::new("/tmp/mac-kind2-state-2.city")
        } else {
            std::path::Path::new("/tmp/mac-kind2-state-2.sc2")
        };
        let mut city = City::new_blank(path, "MacKind2State2").expect("blank city");
        city.kind = kind;
        city.set_pe_city_center(12, 34);
        city.set_mac_kind2_hotspot(60, 60);
        // The state-2 lane turns from heading 0 toward the coincident target,
        // then the stride-8 mover advances into row 60, column 59.
        city.set_traffic_density(60, 59, 0xaa);
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 2,
                state: 2,
                tile_x: 60,
                tile_y: 60,
                target_x: 60,
                target_y: 60,
                ..Default::default()
            },
        );

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        let mut kind2_sound_deadline = 0;
        assert_eq!(city.traffic_density_at(60, 59), Some(0xaa));
        let sounds = tick_mac_actors_with_rng_at(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            &mut kind2_sound_deadline,
            0,
            20,
        );
        let record = city.traffic_overlays().expect("actor table")[slot];
        outcomes.push((
            record,
            quickdraw.seed(),
            sounds,
            kind2_sound_deadline,
            city.mac_state_misc_u32_at(0x1018),
            city.mac_state_misc_u32_at(0x101c),
        ));
    }

    assert_eq!(outcomes[0].0, outcomes[1].0);
    assert_eq!(outcomes[0].1, outcomes[1].1);
    assert_eq!(outcomes[0].2, vec![PlacementSound::MacKind2Traffic]);
    assert_eq!(outcomes[0].2, outcomes[1].2);
    assert_eq!(outcomes[0].3, 5020);
    assert_eq!(outcomes[0].3, outcomes[1].3);
    // The Mac layout persists the target pair; Win-layout state stays in its
    // adapter and leaves the PE-owned MISC slots untouched.
    assert_eq!(outcomes[0].4, Some(60));
    assert_eq!(outcomes[0].5, Some(60));
    assert_eq!(outcomes[1].4, Some(12));
    assert_eq!(outcomes[1].5, Some(34));
}

#[test]
fn mac_kind2_states_zero_and_three_match_across_city_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let path = if kind == CityFileKind::MacCity {
            std::path::Path::new("/tmp/mac-kind2-states-0-3.city")
        } else {
            std::path::Path::new("/tmp/mac-kind2-states-0-3.sc2")
        };
        let mut city = City::new_blank(path, "MacKind2States03").expect("blank city");
        city.kind = kind;
        let first = XTHG_FIRST_USABLE_SLOT;
        for (offset, state, lane, height) in [
            (0, 0, 7, 9),  // phase 0 increments below threshold and wraps heading
            (1, 0, 3, 10), // phase 0 enters phase 2 at the threshold
            (2, 3, 7, 4),  // phase 3 requests MAYDAY5 before decrementing
            (3, 3, 6, 2),  // phase 3 enters terminal kind-6 state
        ] {
            city.set_mac_object_slot_record(
                first + offset,
                crate::win95_overlay::Win95OverlayRecord {
                    kind: 2,
                    state,
                    lane,
                    height,
                    ..Default::default()
                },
            );
        }

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        let quickdraw_before = quickdraw.seed();
        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let records = city.traffic_overlays().expect("actor table");
        outcomes.push((
            records[first..first + 4].to_vec(),
            city.mac_kind2_actor_counter,
            quickdraw.seed(),
            sounds,
        ));
        assert_eq!(quickdraw.seed(), quickdraw_before, "layout={kind:?}");
    }

    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(
        (
            outcomes[0].0[0].state,
            outcomes[0].0[0].lane,
            outcomes[0].0[0].height
        ),
        (0, 0, 10)
    );
    assert_eq!(
        (
            outcomes[0].0[1].state,
            outcomes[0].0[1].lane,
            outcomes[0].0[1].height
        ),
        (2, 4, 10)
    );
    assert_eq!(
        (
            outcomes[0].0[2].state,
            outcomes[0].0[2].lane,
            outcomes[0].0[2].height
        ),
        (3, 0, 3)
    );
    assert_eq!(outcomes[0].0[3].kind, 6);
    assert_eq!(outcomes[0].0[3].state, 0x11);
    assert_eq!(outcomes[0].0[3].saved_b, 1);
    assert_eq!(outcomes[0].0[3].lane, 0);
    assert_eq!(outcomes[0].1, 4);
    assert_eq!(outcomes[0].3, vec![PlacementSound::Mayday]);
}

#[test]
fn mac_kind2_state_four_random_reset_matches_across_city_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let path = if kind == CityFileKind::MacCity {
            std::path::Path::new("/tmp/mac-kind2-state-4.city")
        } else {
            std::path::Path::new("/tmp/mac-kind2-state-4.sc2")
        };
        let mut city = City::new_blank(path, "MacKind2State4").expect("blank city");
        city.kind = kind;
        let first = XTHG_FIRST_USABLE_SLOT;
        for offset in 0..2 {
            city.set_mac_object_slot_record(
                first + offset,
                crate::win95_overlay::Win95OverlayRecord {
                    kind: 2,
                    state: 4,
                    ..Default::default()
                },
            );
        }

        // QuickDraw `_Random` returns 8460 then -25898 from seed 20, so only
        // the first signed remainder is zero modulo 20.
        let mut quickdraw = MacToolboxRng::new(20);
        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let records = city.traffic_overlays().expect("actor table");
        outcomes.push((
            records[first].state,
            records[first + 1].state,
            city.mac_kind2_actor_counter,
            quickdraw.seed(),
            sounds,
        ));
    }

    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0].0, 0);
    assert_eq!(outcomes[0].1, 4);
    assert_eq!(outcomes[0].2, 2);
    let mut expected_quickdraw = MacToolboxRng::new(20);
    expected_quickdraw.random_i16();
    expected_quickdraw.random_i16();
    assert_eq!(outcomes[0].3, expected_quickdraw.seed());
    assert!(outcomes[0].4.is_empty());
}

#[test]
fn mac_kind5_overrides_prior_and_later_actor_slots_in_native_order_on_both_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind5-cross-actor-order.sc2"),
            "MacKind5Order",
        )
        .expect("blank city");
        city.kind = kind;
        let first = XTHG_FIRST_USABLE_SLOT;
        // The earlier records are counted, but their deliberately unsupported
        // phase 6 does not alter their state before the Monster's scan.
        city.set_mac_object_slot_record(
            first,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                state: 6,
                tile_x: 30,
                tile_y: 30,
                ..Default::default()
            },
        );
        city.set_mac_object_slot_record(
            first + 1,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 2,
                state: 6,
                ..Default::default()
            },
        );
        city.set_mac_object_slot_record(
            first + 2,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 5,
                ..Default::default()
            },
        );
        // These later slots observe the phase writes during this same sweep.
        city.set_mac_object_slot_record(
            first + 3,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                state: 6,
                lane: 0,
                height: 10,
                tile_x: 40,
                tile_y: 40,
                sub_x: 8,
                sub_y: 8,
                ..Default::default()
            },
        );
        city.set_mac_object_slot_record(
            first + 4,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 2,
                state: 6,
                lane: 0,
                height: 10,
                ..Default::default()
            },
        );

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let records = city.traffic_overlays().expect("actor table");
        outcomes.push((
            records[first..first + 5].to_vec(),
            city.mac_kind1_actor_counter,
            city.mac_kind2_actor_counter,
            quickdraw.seed(),
            sounds,
        ));
    }

    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0].0[0].state, 7, "earlier kind 1 is overwritten");
    assert_eq!(outcomes[0].0[1].state, 5, "earlier kind 2 is overwritten");
    assert_eq!(outcomes[0].0[3].state, 7);
    assert_eq!(outcomes[0].0[3].height, 9, "later kind 1 runs phase 7 now");
    assert_eq!(outcomes[0].0[4].state, 5);
    assert_eq!(outcomes[0].0[4].height, 9, "later kind 2 runs phase 5 now");
    assert_eq!(outcomes[0].1, 2);
    assert_eq!(outcomes[0].2, 2);
    assert!(outcomes[0].4.is_empty());
}

#[test]
fn mac_kind5_state_zero_countdown_and_stride_eight_match_on_both_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind5-state-zero.sc2"),
            "MacKind5StateZero",
        )
        .expect("blank city");
        city.kind = kind;
        city.set_mac_kind2_hotspot(60, 60);
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 5,
                height: 9,
                tile_x: 50,
                tile_y: 50,
                sub_x: 10,
                sub_y: 10,
                ..Default::default()
            },
        );

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        for _ in 0..2 {
            tick_mac_actors_with_rng(
                &mut city,
                &mut quickdraw,
                &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
                0,
            );
        }
        let actor = city
            .traffic_overlays()
            .and_then(|records| records.get(slot))
            .copied()
            .expect("actor slot");
        outcomes.push((
            actor,
            city.mac_kind5_actor_counter,
            quickdraw.seed(),
            city.text_overlay_at(51, 51),
        ));
    }

    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0].0.state, 1, "countdown reaches the state-1 gate");
    assert_eq!(outcomes[0].0.height, 8);
    assert_eq!((outcomes[0].0.tile_x, outcomes[0].0.tile_y), (51, 51));
    assert_eq!((outcomes[0].0.sub_x, outcomes[0].0.sub_y), (10, 10));
    assert_eq!(outcomes[0].0.lane, 3);
    assert_eq!(outcomes[0].1, 1, "kind-5 visit counter resets each sweep");
    assert_eq!(outcomes[0].3, Some(0xc9 + XTHG_FIRST_USABLE_SLOT as u8));
}

#[test]
fn mac_kind5_state_three_retires_actor_and_uses_random_parity_on_both_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind5-state-three.sc2"),
            "MacKind5StateThree",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 5,
                state: 3,
                tile_x: 43,
                tile_y: 57,
                target_x: 9,
                target_y: 10,
                ..Default::default()
            },
        );
        let proxy = crate::win95_overlay::xthg_proxy_for_slot(slot).expect("proxy");
        city.set_text_overlay(43, 57, proxy);

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        let mut expected_rng = quickdraw;
        let expected_target = if expected_rng.random_u16() & 1 != 0 {
            0x24
        } else {
            0
        };
        let mut kind2_sound_deadline = 0;
        tick_mac_actors_with_rng_at(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            &mut kind2_sound_deadline,
            0,
            1,
        );
        let actor = city.traffic_overlays().expect("actor table")[slot];
        outcomes.push((
            actor,
            city.text_overlay_at(43, 57),
            quickdraw.seed(),
            expected_target,
            expected_rng.seed(),
        ));
    }

    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0].0.kind, 0, "export 121 clears the actor kind");
    assert_eq!(outcomes[0].0.state, 3);
    assert_eq!(
        (outcomes[0].0.target_x, outcomes[0].0.target_y),
        (outcomes[0].3, outcomes[0].3)
    );
    assert_eq!(
        outcomes[0].1,
        Some(0),
        "zero-fixed-point app LFSR takes export 121"
    );
    assert_eq!(
        outcomes[0].2, outcomes[0].4,
        "state 3 consumes one QuickDraw _Random value for parity"
    );
}

#[test]
fn mac_kind5_state_zero_kind14_collision_enters_state_three_and_runs_common_mover() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind5-state-zero-kind14.sc2"),
            "MacKind5Collision",
        )
        .expect("blank city");
        city.kind = kind;
        city.set_mac_kind2_hotspot(60, 60);
        let monster_slot = XTHG_FIRST_USABLE_SLOT;
        let marker_slot = monster_slot + 1;
        city.set_mac_object_slot_record(
            monster_slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 5,
                height: 9,
                tile_x: 50,
                tile_y: 50,
                sub_x: 8,
                sub_y: 8,
                ..Default::default()
            },
        );
        city.set_mac_object_slot_record(
            marker_slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 14,
                ..Default::default()
            },
        );
        let proxy = crate::win95_overlay::xthg_proxy_for_slot(marker_slot).expect("proxy");
        city.set_text_overlay(51, 51, proxy);

        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let actor = city
            .traffic_overlays()
            .and_then(|records| records.get(monster_slot))
            .expect("monster slot");
        assert_eq!(actor.state, 3);
        assert_eq!(actor.height, 8);
        assert_eq!((actor.tile_x, actor.tile_y), (50, 50));
        assert_eq!((actor.sub_x, actor.sub_y), (16, 16));
        assert_eq!(city.text_overlay_at(51, 51), Some(proxy));
    }
}

#[test]
fn mac_kind5_state_two_random_gate_target_and_heading_match_on_both_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for now in [0, 0xffff] {
            let mut city = City::new_blank(
                std::path::Path::new("/tmp/mac-kind5-state-two.sc2"),
                "MacKind5StateTwo",
            )
            .expect("blank city");
            city.kind = kind;
            let slot = XTHG_FIRST_USABLE_SLOT;
            let proxy = crate::win95_overlay::xthg_proxy_for_slot(slot).expect("proxy");
            city.set_text_overlay(50, 50, proxy);
            city.set_mac_object_slot_record(
                slot,
                crate::win95_overlay::Win95OverlayRecord {
                    kind: 5,
                    state: 2,
                    lane: 4,
                    height: 15,
                    tile_x: 50,
                    tile_y: 50,
                    sub_x: 10,
                    sub_y: 10,
                    ..Default::default()
                },
            );

            let mut quickdraw = MacToolboxRng::new(1);
            let mut expected_rng = MacToolboxRng::new(1);
            let expected_state = if expected_rng.random_u16() % 3 == 0 {
                3
            } else {
                0
            };
            let expected_target = (expected_rng.random_u16() & 7) * 9;
            let expected_lane = mac_kind1_perturb_heading(4, &mut expected_rng);
            let mut kind2_sound_deadline = 0;
            tick_mac_actors_with_rng_at(
                &mut city,
                &mut quickdraw,
                &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
                &mut kind2_sound_deadline,
                0,
                now,
            );

            let actor = city.traffic_overlays().expect("actor table")[slot];
            outcomes.push((
                actor,
                city.text_overlay_at(50, 51),
                quickdraw.seed(),
                expected_lane,
                expected_rng.seed(),
                expected_state,
                expected_target,
            ));
        }
    }

    assert_eq!(
        outcomes[0], outcomes[2],
        "MacCity and WinSc2 match with different virtual clock readings"
    );
    assert_eq!(
        outcomes[1], outcomes[3],
        "MacCity and WinSc2 match with different virtual clock readings"
    );
    assert_eq!(
        outcomes[0], outcomes[1],
        "kind-5 state 2 is RNG-driven, not clock-driven"
    );
    for (
        actor,
        destination_proxy,
        actual_seed,
        expected_lane,
        expected_seed,
        expected_state,
        expected_target,
    ) in outcomes
    {
        assert_eq!(actor.state, expected_state);
        assert_eq!(actor.height, 15);
        assert_eq!(
            (actor.target_x, actor.target_y),
            (expected_target as u8, expected_target as u8)
        );
        assert_eq!(actor.lane, expected_lane);
        assert_eq!(actual_seed, expected_seed);
        assert_eq!((actor.tile_x, actor.tile_y), (50, 51));
        assert_eq!((actor.sub_x, actor.sub_y), (10, 2));
        assert_eq!(destination_proxy, Some(0xc9 + XTHG_FIRST_USABLE_SLOT as u8));
    }
}

#[test]
fn mac_kind5_state_one_map_branches_match_on_both_city_layouts() {
    let mut outcomes = Vec::new();
    let mut expected_rng = MacToolboxRng::new(1);
    expected_rng.random_u16(); // `_Random % 25` state gate
    expected_rng.random_u16(); // target X (`% 128`)
    expected_rng.random_u16(); // target Y (`% 128`)
    let expected_direction = mac_kind1_perturb_heading(4, &mut expected_rng);
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for status in [0, 1, 2, 3] {
            let mut city = City::new_blank(
                std::path::Path::new("/tmp/mac-kind5-state-one.sc2"),
                "MacKind5StateOne",
            )
            .expect("blank city");
            city.kind = kind;
            let (x, y) = (50, 50);
            city.set_tile_bits(x, y, TileBits::new(0));
            city.set_zone(x, y, 3);
            city.set_building(x, y, if status == 0 { 6 } else { 0x0e });
            if status == 0 {
                city.set_traffic_density(x, y, 0x5a);
            }
            if status == 3 {
                // Export 157 reuses fixed XMIC row 4 for 0xc8 and increments
                // its existing +2/+4 WORDs instead of clearing them.
                city.set_microsim_stat1(4, 0x1234);
                city.set_microsim_stat2(4, 0x4567);
            }

            let slot = XTHG_FIRST_USABLE_SLOT;
            city.set_mac_object_slot_record(
                slot,
                crate::win95_overlay::Win95OverlayRecord {
                    kind: 5,
                    state: 1,
                    lane: 4,
                    height: 15,
                    tile_x: 50,
                    tile_y: 50,
                    saved_b: status,
                    sub_x: 8,
                    sub_y: 8,
                    ..Default::default()
                },
            );

            let mut quickdraw = MacToolboxRng::new(1);
            let mut kind2_sound_deadline = 0;
            tick_mac_actors_with_rng_at(
                &mut city,
                &mut quickdraw,
                &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
                &mut kind2_sound_deadline,
                0,
                16,
            );
            let actor = city.traffic_overlays().expect("actor table")[slot];
            outcomes.push((
                status,
                city.building_at(x, y),
                city.tile_bits_at(x, y).unwrap_or_default().raw_bits(),
                city.zone_at(x, y).map_or(0, |zone| zone.raw_bits()),
                city.text_overlay_at(x, y),
                actor.state,
                actor.lane,
                actor.target_x,
                actor.target_y,
                quickdraw.seed(),
                city.microsim_at(4),
                city.traffic_density_at(x, y),
            ));
        }
    }

    assert_eq!(&outcomes[..4], &outcomes[4..]);
    assert_eq!(outcomes[0].1, 6, "status 0 does not replace XBLD");
    assert_eq!(outcomes[0].4, Some(0xff), "status 0 claims the candidate");
    assert_eq!(outcomes[0].11, Some(0), "status 0 clears native XTRF cell");
    assert!(
        (9..=12).contains(&outcomes[1].1),
        "status 1 places the QuickDraw _Random-selected tile"
    );
    assert_eq!(
        outcomes[2].1, 0,
        "export 80 removes the candidate before water repair"
    );
    assert_eq!(
        outcomes[3].1, 0xc8,
        "status 3 places the fixed C8 structure"
    );
    assert_eq!(outcomes[3].2 & 0xe0, 0xe0);
    assert_eq!(outcomes[3].3 & 0xf0, 0xf0);
    assert_eq!(outcomes[3].4, Some(0x37));
    assert_eq!(outcomes[3].10.map(|record| record.tile_id), Some(0xc8));
    assert_eq!(outcomes[3].10.map(|record| record.stat1), Some(0x1235));
    assert_eq!(outcomes[3].10.map(|record| record.stat2), Some(0x456b));
    for outcome in &outcomes {
        assert_eq!(outcome.5, 1, "seed 1's first Random % 25 is nonzero");
        assert_eq!(outcome.6, expected_direction);
        assert_ne!(outcome.7 & 0x80, 0, "nonzero-status epilogue/success gate");
    }
}

#[test]
fn mac_kind5_state_one_native_tile_families_match_selector_and_xbld_on_both_layouts() {
    use crate::city::CityFileKind;
    use crate::mac_random::{MacGameLcgRng, MacToolboxRng};
    use crate::win95_overlay::Win95OverlayRecord;

    // Original CODE 6 replay at actor tile (45,40), with the same isolated
    // map planes, ALTM water level, XTRF value, QuickDraw seed, and starting
    // actor bytes. The _Random trap at CODE 6 0xE12A supplies the selector;
    // subsequent draws are route-specific handler work.
    let cases = [
        (0x0e, 0x0b, 8, 1_457_850_878, 1),
        (0x1d, 0x0b, 8, 1_457_850_878, 1),
        (0x51, 0x09, 7, 101_027_544, 1),
        (0x61, 0x09, 25, 1_817_129_560, 1),
        (0xdd, 0x0b, 5, 1_144_108_930, 1),
        (0xdf, 0x0b, 5, 1_144_108_930, 1),
        (0xe0, 0x0b, 5, 1_144_108_930, 1),
        // Seed 3 distinguishes the native fifth draw (tile 12) from the
        // former Rust eighth draw (tile 9) on the connected-runway branch.
        (0xdd, 0x0c, 5, 1_284_843_143, 3),
    ];

    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (initial_building, expected_building, expected_draws, expected_seed, seed) in cases {
            let mut city = City::new_blank(
                std::path::Path::new("/tmp/mac-kind5-state-one-native-family.sc2"),
                "MacKind5NativeFamily",
            )
            .expect("blank city");
            city.kind = kind;
            let (x, y) = (45, 40);
            for map_x in 0..128 {
                for map_y in 0..128 {
                    city.set_altitude(map_x, map_y, 0, 0);
                }
            }
            city.set_building(x, y, initial_building);
            city.set_altitude(x, y, 0, 8);
            city.set_traffic_density(x, y, 0x5a);
            city.set_mac_object_slot_record(
                XTHG_FIRST_USABLE_SLOT,
                Win95OverlayRecord {
                    kind: 5,
                    lane: 2,
                    state: 1,
                    tile_x: x as u8,
                    tile_y: y as u8,
                    height: 10,
                    sub_x: 8,
                    sub_y: 8,
                    saved_b: 1,
                    ..Default::default()
                },
            );

            let mut quickdraw = MacToolboxRng::new(seed);
            let mut game_rng = MacGameLcgRng::new(1);
            let mut sound_deadline = 0;
            let mut tick_count = 0;
            tick_mac_actors_with_rng_and_clock(
                &mut city,
                &mut quickdraw,
                &mut game_rng,
                &mut sound_deadline,
                0,
                &mut tick_count,
                &mut None,
            );

            assert_eq!(
                city.building_at(x, y),
                expected_building,
                "layout={kind:?}, initial XBLD={initial_building:#04x}"
            );
            assert_eq!(
                quickdraw.seed(),
                expected_seed,
                "layout={kind:?}, initial XBLD={initial_building:#04x}, seed={seed}, draws={expected_draws}"
            );
            assert_eq!(city.tile_bits_at(x, y), Some(TileBits::new(0)));
            assert_eq!(city.zone_at(x, y).map_or(0, |zone| zone.raw_bits()), 0);
            assert_eq!(city.terrain_at(x, y), 0);
            assert_eq!(city.text_overlay_at(x, y), Some(0));
            assert_eq!(city.traffic_density_at(x, y), Some(0x5a));
            if initial_building == 0x51 {
                for adjacent_y in [39, 41] {
                    let altm = city.altm_at(x, adjacent_y);
                    assert_eq!(
                        (altm.land, altm.water, altm.tunnel_levels),
                        (31, 31, 63),
                        "layout={kind:?}, native bridge classification sample y={adjacent_y}"
                    );
                }
            }
        }
    }
}

#[test]
fn mac_kind5_status_zero_fa_marker_decrements_class_counter_on_both_layouts() {
    for (kind, building, expected_counts) in [
        (CityFileKind::MacCity, 0x1d, [6, 4]),
        (CityFileKind::WinSc2, 0x06, [7, 3]),
    ] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind5-status-zero-fa.sc2"),
            "MacKind5StatusZeroFa",
        )
        .expect("blank city");
        city.kind = kind;
        let (x, y) = (50, 50);
        city.set_tile_bits(x, y, TileBits::new(0));
        city.set_building(x, y, building);
        city.set_text_overlay(x, y, 0xfa);
        city.set_traffic_density(x, y, 0x5a);
        city.set_mac_state_misc_u32_at(0x1050, 7);
        city.set_mac_state_misc_u32_at(0x1024, 4);
        city.set_mac_object_slot_record(
            XTHG_FIRST_USABLE_SLOT,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 5,
                state: 1,
                lane: 4,
                height: 15,
                tile_x: x as u8,
                tile_y: y as u8,
                saved_b: 0,
                sub_x: 8,
                sub_y: 8,
                ..Default::default()
            },
        );

        let mut quickdraw = MacToolboxRng::new(1);
        let mut kind2_sound_deadline = 0;
        tick_mac_actors_with_rng_at(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            &mut kind2_sound_deadline,
            0,
            16,
        );

        assert_eq!(city.mac_rci_overlay_cap_counts(), expected_counts);
        assert_eq!(city.text_overlay_at(x, y), Some(0xff));
        assert_eq!(city.traffic_density_at(x, y), Some(0));
    }
}

#[test]
fn mac_kind5_rendered_export80_wait_advances_later_actor_tickcount() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind5-export80-tickcount-wait.sc2"),
            "MacKind5TickWait",
        )
        .expect("blank city");
        city.kind = kind;
        city.set_building(50, 50, 0x0e);
        city.set_zone(50, 50, 3);
        city.set_tile_bits(50, 50, TileBits::new(0));
        city.set_mac_object_slot_record(
            XTHG_FIRST_USABLE_SLOT,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 5,
                state: 1,
                lane: 0,
                height: 15,
                tile_x: 50,
                tile_y: 50,
                saved_b: 1,
                ..Default::default()
            },
        );
        city.set_mac_object_slot_record(
            XTHG_FIRST_USABLE_SLOT + 1,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 5,
                state: 2,
                lane: 0,
                height: 15,
                tile_x: 80,
                tile_y: 80,
                ..Default::default()
            },
        );

        let mut now = 16;
        let mut kind2_sound_deadline = 0;
        tick_mac_actors_with_rng_and_clock(
            &mut city,
            &mut MacToolboxRng::new(0x1357_9bdf),
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            &mut kind2_sound_deadline,
            0,
            &mut now,
            &mut None,
        );

        assert_eq!(now, 28, "export 80 waits through TickCount + 12");
        let later = city.traffic_overlays().expect("actor table")[XTHG_FIRST_USABLE_SLOT + 1];
        assert_eq!(
            later.target_x % 9,
            0,
            "state-2 target comes from Random & 7"
        );
        assert_eq!(later.target_y, later.target_x);
        assert!(
            matches!(later.state, 0 | 3),
            "state-2 gate comes from Random % 3"
        );
    }
}

#[test]
fn mac_export80_tick_wait_counts_follow_native_extent_and_topology_loops() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut generic_extent_one = City::new_blank(
            std::path::Path::new("/tmp/mac-export80-waits-generic-one.sc2"),
            "WaitExtentOne",
        )
        .expect("blank city");
        generic_extent_one.kind = kind;
        generic_extent_one.set_building(50, 50, 0x0e);
        assert_eq!(
            crate::app::budget::mac_destroy_building_export80(
                &mut generic_extent_one,
                50,
                50,
                &mut MacToolboxRng::new(1),
                true,
            ),
            1,
            "generic extent one waits once; layout={kind:?}"
        );

        let mut generic_extent_two = City::new_blank(
            std::path::Path::new("/tmp/mac-export80-waits-generic-two.sc2"),
            "WaitExtentTwo",
        )
        .expect("blank city");
        generic_extent_two.kind = kind;
        generic_extent_two.set_building(50, 50, 0x8c);
        assert_eq!(
            crate::app::budget::mac_destroy_building_export80(
                &mut generic_extent_two,
                50,
                50,
                &mut MacToolboxRng::new(1),
                true,
            ),
            2,
            "generic extent two waits once per outer pass; layout={kind:?}"
        );

        let mut topology_fallback = City::new_blank(
            std::path::Path::new("/tmp/mac-export80-waits-topology-fallback.sc2"),
            "WaitTopologyFallback",
        )
        .expect("blank city");
        topology_fallback.kind = kind;
        topology_fallback.set_building(50, 50, 0x6a);
        topology_fallback.set_zone(50, 50, 0xf0);
        assert_eq!(
            crate::app::budget::mac_destroy_building_export80(
                &mut topology_fallback,
                50,
                50,
                &mut MacToolboxRng::new(1),
                true,
            ),
            2,
            "rejected extent-2 topology uses the two-pass fallback; layout={kind:?}"
        );

        let mut topology_run = City::new_blank(
            std::path::Path::new("/tmp/mac-export80-waits-topology-run.sc2"),
            "WaitTopologyRun",
        )
        .expect("blank city");
        topology_run.kind = kind;
        topology_run.set_building(50, 50, 0x6a);
        topology_run.set_tile_bits(50, 51, TileBits::new(0x02));
        assert_eq!(
            crate::app::budget::mac_destroy_building_export80(
                &mut topology_run,
                50,
                50,
                &mut MacToolboxRng::new(1),
                true,
            ),
            1,
            "admitted extent-2 topology uses its terminal wait; layout={kind:?}"
        );

        let mut portal_run = City::new_blank(
            std::path::Path::new("/tmp/mac-export80-waits-portal.sc2"),
            "WaitPortal",
        )
        .expect("blank city");
        portal_run.kind = kind;
        portal_run.set_building(50, 50, 0x41);
        portal_run.set_building(53, 50, 0x3f);
        assert_eq!(
            crate::app::budget::mac_destroy_building_export80(
                &mut portal_run,
                50,
                50,
                &mut MacToolboxRng::new(1),
                true,
            ),
            1,
            "portal endpoint match reaches its render wait; layout={kind:?}"
        );

        let mut portal_without_mate = City::new_blank(
            std::path::Path::new("/tmp/mac-export80-waits-portal-no-mate.sc2"),
            "WaitPortalNoMate",
        )
        .expect("blank city");
        portal_without_mate.kind = kind;
        portal_without_mate.set_building(50, 50, 0x41);
        let seed = 0x41_80_0001;
        let mut expected_rng = MacToolboxRng::new(seed);
        expected_rng.random_u16();
        expected_rng.random_u16();
        let mut rng = MacToolboxRng::new(seed);
        assert_eq!(
            crate::app::budget::mac_destroy_building_export80(
                &mut portal_without_mate,
                50,
                50,
                &mut rng,
                true,
            ),
            0,
            "portal with no matching endpoint must not enter the exit wait; layout={kind:?}"
        );
        assert_eq!(portal_without_mate.building_at(50, 50), 0);
        assert_eq!(rng.seed(), expected_rng.seed());
    }
}

#[test]
fn mac_kind5_state_one_random_gate_is_clock_and_height_independent() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for (height, now, seed, expected_state) in [(14, 16, 1, 1), (15, 24, 1, 1), (15, 25, 0, 2)]
        {
            let mut city = City::new_blank(
                std::path::Path::new("/tmp/mac-kind5-state-one-mod25.sc2"),
                "MacKind5StateOneTimer",
            )
            .expect("blank city");
            city.kind = kind;
            let slot = XTHG_FIRST_USABLE_SLOT;
            city.set_mac_object_slot_record(
                slot,
                crate::win95_overlay::Win95OverlayRecord {
                    kind: 5,
                    state: 1,
                    lane: 4,
                    height,
                    tile_x: 50,
                    tile_y: 50,
                    sub_x: 8,
                    sub_y: 8,
                    ..Default::default()
                },
            );
            let mut quickdraw = MacToolboxRng::new(seed);
            let mut kind2_sound_deadline = 0;
            tick_mac_actors_with_rng_at(
                &mut city,
                &mut quickdraw,
                &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
                &mut kind2_sound_deadline,
                0,
                now,
            );
            let actor = city.traffic_overlays().expect("actor table")[slot];
            outcomes.push((actor.state, actor.height));
            assert_eq!(actor.state, expected_state, "layout={kind:?}, now={now}");
            assert_eq!(
                actor.height, height,
                "no state-1 countdown; layout={kind:?}, now={now}"
            );
        }
    }
    assert_eq!(&outcomes[..3], &outcomes[3..]);
}

#[test]
fn mac_kind_one_height_redirect_matches_across_city_layouts() {
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let path = if kind == CityFileKind::MacCity {
            std::path::Path::new("/tmp/mac-kind1-height.city")
        } else {
            std::path::Path::new("/tmp/mac-kind1-height.sc2")
        };
        let mut city = City::new_blank(path, "MacKind1Height").expect("blank city");
        city.kind = kind;
        // Mac XTHG stores row/column while City map methods take x/y.
        city.set_building(41, 27, 0x81); // Native height 14, threshold 4.
        city.set_zone(41, 27, 0x03);
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_text_overlay(41, 27, 0xca);
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                lane: 6,
                state: 3,
                tile_x: 41,
                tile_y: 27,
                height: 3,
                ..Default::default()
            },
        );
        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut MacToolboxRng::new(0x1234_5678),
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let record = city.traffic_overlays().unwrap()[slot];
        outcomes.push((
            record,
            city.text_overlay_at(41, 27),
            city.mac_kind1_actor_counter,
            sounds,
        ));
    }
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(
        (outcomes[0].0.kind, outcomes[0].0.lane, outcomes[0].0.state),
        (6, 0, 5)
    );
    assert_eq!(outcomes[0].1, Some(0xca));
    assert_eq!(outcomes[0].2, 1);
}

#[test]
fn mac_kind_one_height_redirect_skips_threshold_equality_and_zone_eight() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind1-height-gates.city"),
        "MacKind1HeightGates",
    )
    .expect("blank city");
    city.set_building(41, 27, 0x81);
    city.set_zone(41, 27, 0x03);
    city.set_building(42, 27, 0x81);
    city.set_zone(42, 27, 0x08);
    city.set_building(43, 27, 0x70);
    city.set_zone(43, 27, 0x03);
    let first = XTHG_FIRST_USABLE_SLOT;
    let second = first + 1;
    let third = second + 1;
    for (slot, col, height) in [(first, 41u8, 4), (second, 42u8, 0), (third, 43u8, 0)] {
        city.set_text_overlay(usize::from(col), 27, 0x22);
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                lane: 3,
                // Phase 15 is outside the modeled kind-1 cases so this test
                // isolates the common pre-switch redirect gates.
                state: 0x0f,
                tile_x: col,
                tile_y: 27,
                height,
                ..Default::default()
            },
        );
    }
    let before = [
        city.traffic_overlays().unwrap()[first],
        city.traffic_overlays().unwrap()[second],
        city.traffic_overlays().unwrap()[third],
    ];

    tick_mac_actors_with_rng(
        &mut city,
        &mut MacToolboxRng::new(0x1234_5678),
        &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
        0,
    );

    let records = city.traffic_overlays().unwrap();
    assert_eq!(records[first], before[0]);
    assert_eq!(records[second], before[1]);
    assert_eq!(records[third], before[2]);
    assert_eq!(city.text_overlay_at(41, 27), Some(0x22));
    assert_eq!(city.text_overlay_at(42, 27), Some(0x22));
    assert_eq!(city.text_overlay_at(43, 27), Some(0x22));
    assert_eq!(city.mac_kind1_actor_counter, 3);
}

#[test]
fn mac_kind_one_phase_zero_moves_with_shared_proxy_rules_and_takeoff_order() {
    let mut outcomes = Vec::new();
    let slot = XTHG_FIRST_USABLE_SLOT;
    let proxy = 0xc9u8.wrapping_add(slot as u8);
    let threshold_proxy_expected = proxy.wrapping_add(1);
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let path = if kind == CityFileKind::MacCity {
            std::path::Path::new("/tmp/mac-kind1-phase0.city")
        } else {
            std::path::Path::new("/tmp/mac-kind1-phase0.sc2")
        };
        let mut city = City::new_blank(path, "MacKind1Phase0").expect("blank city");
        city.kind = kind;
        city.set_text_overlay(40, 40, proxy);
        // Actor proxies are skipped along the same cardinal carry.
        city.set_text_overlay(40, 41, proxy + 1);
        city.set_text_overlay(40, 42, proxy + 2);
        city.set_text_overlay(40, 43, 0x56);
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                lane: 4,
                state: 0,
                tile_x: 40,
                tile_y: 40,
                sub_x: 8,
                sub_y: 8,
                height: 0,
                saved_a: 0x22,
                ..Default::default()
            },
        );
        let threshold_slot = slot + 1;
        let threshold_proxy = proxy.wrapping_add(1);
        city.set_text_overlay(10, 10, threshold_proxy);
        city.set_text_overlay(10, 11, 0x45);
        city.set_mac_object_slot_record(
            threshold_slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                lane: 4,
                // Native `MOVE.B #2,(+2)` clears the whole byte at the
                // phase-0 countdown boundary, not just its low nibble.
                state: 0xf0,
                tile_x: 10,
                tile_y: 10,
                sub_x: 8,
                sub_y: 8,
                height: 0x0e,
                saved_a: 0x11,
                ..Default::default()
            },
        );

        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut MacToolboxRng::new(0x1234_5678),
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let record = city.traffic_overlays().unwrap()[slot];
        outcomes.push((
            record,
            city.text_overlay_at(40, 40),
            city.text_overlay_at(40, 41),
            city.text_overlay_at(40, 42),
            city.text_overlay_at(40, 43),
            city.mac_kind1_actor_counter,
            city.traffic_overlays().unwrap()[threshold_slot],
            city.text_overlay_at(10, 10),
            city.text_overlay_at(10, 11),
            sounds
                .iter()
                .map(|sound| sound.sound_id())
                .collect::<Vec<_>>(),
        ));
    }
    assert_eq!(outcomes[0], outcomes[1]);
    let (
        record,
        old,
        skipped,
        other,
        destination,
        visits,
        threshold_record,
        threshold_old,
        threshold_destination,
        sounds,
    ) = &outcomes[0];
    assert_eq!((record.kind, record.lane, record.state), (1, 4, 0));
    assert_eq!((record.tile_x, record.tile_y), (40, 43));
    assert_eq!((record.sub_x, record.sub_y, record.height), (8, 8, 1));
    assert_eq!(
        (record.saved_a, *old, *skipped, *other, *destination),
        (
            0x56,
            Some(0x22),
            Some(proxy + 1),
            Some(proxy + 2),
            Some(proxy)
        )
    );
    assert_eq!(*visits, 2);
    assert_eq!(
        (
            threshold_record.kind,
            threshold_record.state,
            threshold_record.height
        ),
        (1, 2, 0x0e)
    );
    assert_eq!((threshold_record.tile_x, threshold_record.tile_y), (10, 11));
    assert_eq!(
        (*threshold_old, *threshold_destination),
        (Some(0x11), Some(threshold_proxy_expected))
    );
    assert_eq!(sounds, &[crate::audio::SOUND_MAC_TAKEOFF]);
}

#[test]
fn mac_kind_one_phase_one_lands_on_runway_or_hands_off_to_kind_six_across_city_layouts() {
    let slot = XTHG_FIRST_USABLE_SLOT;
    let proxy = 0xc9u8.wrapping_add(slot as u8);
    let mut outcomes = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for runway in [false, true] {
            let path = if kind == CityFileKind::MacCity {
                std::path::Path::new("/tmp/mac-kind1-phase1.city")
            } else {
                std::path::Path::new("/tmp/mac-kind1-phase1.sc2")
            };
            let mut city = City::new_blank(path, "MacKind1Phase1").expect("blank city");
            city.kind = kind;
            city.set_text_overlay(40, 40, proxy);
            city.set_text_overlay(40, 39, 0x33);
            if runway {
                city.set_building(40, 39, 0xdd);
            }
            city.set_mac_object_slot_record(
                slot,
                crate::win95_overlay::Win95OverlayRecord {
                    kind: 1,
                    lane: 0,
                    state: 1,
                    tile_x: 40,
                    tile_y: 40,
                    sub_x: 8,
                    sub_y: 8,
                    height: 1,
                    saved_a: 0x22,
                    ..Default::default()
                },
            );

            let sounds = tick_mac_actors_with_rng(
                &mut city,
                &mut MacToolboxRng::new(0x1234_5678),
                &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
                0,
            );
            let record = city.traffic_overlays().unwrap()[slot];
            outcomes.push((
                runway,
                record,
                city.text_overlay_at(40, 40),
                city.text_overlay_at(40, 39),
                sounds
                    .iter()
                    .map(|sound| sound.sound_id())
                    .collect::<Vec<_>>(),
            ));
        }
    }

    assert_eq!(outcomes[0], outcomes[2]);
    assert_eq!(outcomes[1], outcomes[3]);
    let field = &outcomes[0];
    assert_eq!((field.1.kind, field.1.state, field.1.lane), (6, 5, 0));
    assert_eq!(
        (field.1.height, field.1.saved_a, field.1.saved_b),
        (0, 0x33, 1)
    );
    assert_eq!((field.2, field.3), (Some(0x22), Some(0x33)));
    assert_eq!(field.4, &[crate::audio::SOUND_MAC_LAND]);

    let runway = &outcomes[1];
    assert_eq!((runway.1.kind, runway.1.height), (0, 0));
    assert_eq!((runway.2, runway.3), (Some(0x22), Some(0)));
    assert_eq!(runway.4, &[crate::audio::SOUND_MAC_LAND]);
}

#[test]
fn mac_kind_one_phase_two_random_turn_and_arcology_avoidance_match_across_layouts() {
    let slot = XTHG_FIRST_USABLE_SLOT;
    let proxy = 0xc9u8.wrapping_add(slot as u8);
    let mut outcomes = Vec::new();
    for seed in [1u32, 11] {
        for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
            let path = if kind == CityFileKind::MacCity {
                std::path::Path::new("/tmp/mac-kind1-phase2.city")
            } else {
                std::path::Path::new("/tmp/mac-kind1-phase2.sc2")
            };
            let mut city = City::new_blank(path, "MacKind1Phase2").expect("blank city");
            city.kind = kind;
            city.set_text_overlay(60, 60, proxy);
            if seed == 1 {
                // Heading 0 points west three cells; the native helper should
                // choose the first clear turn, heading 1.
                city.set_building(60, 57, 0xfb);
            }
            let expected_heading = if seed == 1 { 1u8 } else { 7u8 };
            let (x_step, y_step) = match expected_heading {
                1 => (1i16, -1i16),
                7 => (-1i16, -1i16),
                _ => unreachable!(),
            };
            let destination_x = (60i16 + x_step) as usize;
            let destination_y = (60i16 + y_step) as usize;
            city.set_text_overlay(destination_x, destination_y, 0x42);
            city.set_mac_object_slot_record(
                slot,
                crate::win95_overlay::Win95OverlayRecord {
                    kind: 1,
                    lane: 0,
                    state: 2,
                    tile_x: 60,
                    tile_y: 60,
                    sub_x: 8,
                    sub_y: 8,
                    saved_a: 0x22,
                    ..Default::default()
                },
            );

            let mut quickdraw = MacToolboxRng::new(seed);
            let first = quickdraw.random_u16();
            let unsteered = if first % 5 == 0 {
                ((quickdraw.random_u16() % 3) as u8).wrapping_sub(1) & 7
            } else {
                0
            };
            // Validate the test vector itself: seed 1 takes one draw and then
            // arcology steering picks 1; seed 11 takes the conditional draw
            // and turns 0 to 7.
            assert_eq!(first % 5 == 0, seed == 11);
            assert_eq!(unsteered, if seed == 1 { 0 } else { 7 });
            let expected_seed = quickdraw.seed();

            let mut actual_rng = MacToolboxRng::new(seed);
            tick_mac_actors_with_rng(
                &mut city,
                &mut actual_rng,
                &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
                0,
            );
            let record = city.traffic_overlays().unwrap()[slot];
            outcomes.push((
                seed,
                record,
                city.text_overlay_at(60, 60),
                city.text_overlay_at(destination_x, destination_y),
                actual_rng.seed(),
                expected_seed,
            ));
        }
    }

    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[2], outcomes[3]);
    for (seed, record, source, destination, actual_seed, expected_seed) in
        [&outcomes[0], &outcomes[2]]
    {
        let expected_heading = if *seed == 1 { 1 } else { 7 };
        let expected_coords = if *seed == 1 { (61, 59) } else { (59, 59) };
        assert_eq!(record.kind, 1);
        assert_eq!(record.lane, expected_heading);
        assert_eq!((record.tile_x, record.tile_y), expected_coords);
        assert_eq!(
            (record.saved_a, *source, *destination),
            (0x42, Some(0x22), Some(proxy))
        );
        assert_eq!(*actual_seed, *expected_seed);
    }
}

#[test]
fn mac_kind_one_phases_three_and_four_align_across_city_layouts() {
    let slot = XTHG_FIRST_USABLE_SLOT;
    let mut phase_three = Vec::new();
    let mut phase_four = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let path = if kind == CityFileKind::MacCity {
            std::path::Path::new("/tmp/mac-kind1-phase34.city")
        } else {
            std::path::Path::new("/tmp/mac-kind1-phase34.sc2")
        };
        let mut city = City::new_blank(path, "MacKind1Phase34").expect("blank city");
        city.kind = kind;

        // Phase 3 uses sign-only FUN_0000b354: row delta 0, column delta +1
        // selects heading 4. Crossing the next tile lands exactly at the
        // target, then state 2 turns the heading toward north (2) and moves
        // the next target row six tiles north.
        city.set_text_overlay(60, 60, 0x22);
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                lane: 0,
                state: 0x23,
                tile_x: 60,
                tile_y: 60,
                target_x: 60,
                target_y: 61,
                sub_x: 8,
                sub_y: 8,
                saved_a: 0x22,
                ..Default::default()
            },
        );
        tick_mac_actors_with_rng(
            &mut city,
            &mut MacToolboxRng::new(0x1234_5678),
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let record = city.traffic_overlays().unwrap()[slot];
        phase_three.push((
            record,
            city.text_overlay_at(60, 60),
            city.text_overlay_at(60, 61),
        ));

        // Phase 4 uses the slope-aware helper and one-step turn, then snaps
        // lane/current column to the high-nibble target direction on arrival.
        let mut city = City::new_blank(path, "MacKind1Phase34").expect("blank city");
        city.kind = kind;
        city.set_text_overlay(60, 60, 0x22);
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                lane: 4,
                state: 0x24,
                tile_x: 60,
                tile_y: 60,
                target_x: 60,
                target_y: 61,
                sub_x: 8,
                sub_y: 8,
                saved_a: 0x22,
                ..Default::default()
            },
        );
        tick_mac_actors_with_rng(
            &mut city,
            &mut MacToolboxRng::new(0x1234_5678),
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        let record = city.traffic_overlays().unwrap()[slot];
        phase_four.push((
            record,
            city.text_overlay_at(60, 60),
            city.text_overlay_at(60, 61),
        ));
    }

    assert_eq!(phase_three[0], phase_three[1]);
    assert_eq!(phase_four[0], phase_four[1]);
    assert_eq!(
        (
            phase_three[0].0.lane,
            phase_three[0].0.state,
            phase_three[0].0.tile_x,
            phase_three[0].0.tile_y,
            phase_three[0].0.target_x,
            phase_three[0].0.target_y,
            phase_three[0].1,
            phase_three[0].2,
        ),
        (3, 0x24, 60, 61, 66, 61, Some(0x22), Some(0xc9 + slot as u8))
    );
    assert_eq!(
        (
            phase_four[0].0.lane,
            phase_four[0].0.state,
            phase_four[0].0.tile_x,
            phase_four[0].0.tile_y,
            phase_four[0].1,
            phase_four[0].2,
        ),
        (2, 1, 60, 61, Some(0x22), Some(0xc9 + slot as u8))
    );
}

#[test]
fn mac_kind_one_phase_seven_and_high_xbld_redirect_match_across_layouts() {
    let slot = XTHG_FIRST_USABLE_SLOT;
    let proxy = 0xc9u8.wrapping_add(slot as u8);
    let mut phase_seven_move = Vec::new();
    let mut phase_seven_done = Vec::new();
    let mut high_xbld = Vec::new();
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let path = if kind == CityFileKind::MacCity {
            std::path::Path::new("/tmp/mac-kind1-phase7-redirect.city")
        } else {
            std::path::Path::new("/tmp/mac-kind1-phase7-redirect.sc2")
        };
        let mut city = City::new_blank(path, "MacKind1Phase7").expect("blank city");
        city.kind = kind;
        city.set_text_overlay(40, 40, 0x22);
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                lane: 0,
                state: 7,
                tile_x: 40,
                tile_y: 40,
                sub_x: 8,
                sub_y: 8,
                height: 9,
                saved_a: 0x22,
                ..Default::default()
            },
        );
        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut MacToolboxRng::new(0x1234_5678),
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        phase_seven_move.push((
            city.traffic_overlays().unwrap()[slot],
            city.text_overlay_at(40, 40),
            city.text_overlay_at(40, 39),
            sounds,
        ));

        let mut city = City::new_blank(path, "MacKind1Phase7").expect("blank city");
        city.kind = kind;
        city.set_text_overlay(40, 40, 0x22);
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                lane: 4,
                state: 7,
                tile_x: 40,
                tile_y: 40,
                height: 0,
                saved_a: 0x22,
                ..Default::default()
            },
        );
        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut MacToolboxRng::new(0x1234_5678),
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        phase_seven_done.push((
            city.traffic_overlays().unwrap()[slot],
            city.text_overlay_at(40, 40),
            sounds,
        ));

        let mut city = City::new_blank(path, "MacKind1HighXbld").expect("blank city");
        city.kind = kind;
        city.set_building(44, 42, 0xfb);
        city.set_text_overlay(44, 42, 0x31);
        city.set_mac_object_slot_record(
            slot,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 1,
                lane: 3,
                state: 2,
                tile_x: 44,
                tile_y: 42,
                height: 4,
                saved_b: 0,
                ..Default::default()
            },
        );
        let mut quickdraw = MacToolboxRng::new(0x1234_5678);
        let seed_before = quickdraw.seed();
        let sounds = tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut MacGameLcgRng::new(MacGameLcgRng::INITIAL_STATE),
            0,
        );
        high_xbld.push((
            city.traffic_overlays().unwrap()[slot],
            city.text_overlay_at(44, 42),
            quickdraw.seed(),
            seed_before,
            sounds,
        ));
    }

    assert_eq!(phase_seven_move[0], phase_seven_move[1]);
    assert_eq!(phase_seven_done[0], phase_seven_done[1]);
    assert_eq!(high_xbld[0], high_xbld[1]);
    assert_eq!(
        (
            phase_seven_move[0].0.lane,
            phase_seven_move[0].0.state,
            phase_seven_move[0].0.height,
            phase_seven_move[0].0.tile_x,
            phase_seven_move[0].0.tile_y,
            phase_seven_move[0].1,
            phase_seven_move[0].2,
            phase_seven_move[0]
                .3
                .iter()
                .map(|sound| sound.sound_id())
                .collect::<Vec<_>>(),
        ),
        (
            1,
            7,
            8,
            40,
            39,
            Some(0x22),
            Some(proxy),
            vec![crate::audio::SOUND_MAYDAY]
        )
    );
    assert_eq!(
        (
            phase_seven_done[0].0.kind,
            phase_seven_done[0].0.lane,
            phase_seven_done[0].0.state,
            phase_seven_done[0].0.saved_b,
            phase_seven_done[0].1,
        ),
        (6, 0, 5, 1, Some(0x22))
    );
    assert!(phase_seven_done[0].2.is_empty());
    assert_eq!(
        (
            high_xbld[0].0.kind,
            high_xbld[0].0.lane,
            high_xbld[0].0.state,
            high_xbld[0].0.saved_b,
            high_xbld[0].0.tile_x,
            high_xbld[0].0.tile_y,
            high_xbld[0].1,
            high_xbld[0].2,
            high_xbld[0].3,
        ),
        (
            6,
            0,
            5,
            1,
            44,
            42,
            Some(0x31),
            high_xbld[0].3,
            high_xbld[0].3,
        )
    );
    assert!(high_xbld[0].4.is_empty());
}

#[test]
fn mac_kind_nine_turn_matches_original_unsigned_word_division() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_kind9_turn.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let lane: u8 = v[0].parse().unwrap();
        let word: u16 = v[1].parse().unwrap();
        assert_eq!(
            mac_kind9_turn(lane, word),
            v[2].parse::<u8>().unwrap(),
            "{line}"
        );
        assert_eq!((v[3], v[4], v[5]), ("1", "1", "1"));
        count += 1;
    }
    assert_eq!(count, 52);
}

#[test]
fn mac_kind_three_sound_gate_matches_original_68020() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_kind3_entry.csv");
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let word: u16 = v[0].parse().unwrap();
        let selector: u16 = v[5].parse().unwrap();
        assert_eq!(mac_kind3_sound_requested(word), selector == 0x205, "{line}");
        assert_eq!((v[3], v[4]), ("1", "1"));
        count += 1;
    }
    assert_eq!(count, 27);

    // Seed 256 yields 0xa700: the native gate tests the low byte, not
    // whether the whole word is zero, and still performs the kind-6 handoff.
    let mut city = City::new_blank("/tmp/mac-kind-three-sound.sc2", "Sound").unwrap();
    let slot = XTHG_FIRST_USABLE_SLOT;
    city.set_tile_bits(50, 30, TileBits::new(0));
    city.set_mac_object_slot_record(
        slot,
        crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            tile_x: 30,
            tile_y: 50,
            ..Default::default()
        },
    );
    let mut quickdraw = MacToolboxRng::new(256);
    let requests = tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);
    assert_eq!(requests, vec![PlacementSound::Cargo]);
    assert_eq!(city.traffic_overlays().unwrap()[slot].kind, 6);
}

#[test]
fn mac_mixed_actor_sweep_matches_original_slot_and_sound_order() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_actor_mixed_sweep.csv");
    verify_mac_mixed_actor_sweep(fixture, false);
}

#[test]
fn mac_mixed_actor_sweep_with_prelude_matches_original_68020() {
    let fixture =
        include_str!("../../../tests/fixtures/simulation/mac/mac_actor_mixed_prelude.csv");
    verify_mac_mixed_actor_sweep(fixture, true);
}

fn verify_mac_mixed_actor_sweep(fixture: &str, with_prelude: bool) {
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let mut city = City::new_blank("/tmp/mac-mixed-actor-sweep.sc2", "Sweep").unwrap();
        if with_prelude {
            city.kind = CityFileKind::MacCity;
            let mut history = vec![0_u32; 16 * 52];
            history[4 * 52] = 41;
            history[7 * 52] = 41;
            city.set_chunk_u32_values("XGRP", 0, &history);
            city.set_mac_state_misc_u32_at(0x01f0 + 0xd2 * 4, 1);
        }
        let slots = [2_usize, 5, 8];
        for (index, slot) in slots.into_iter().enumerate() {
            let kind = v[0].as_bytes()[index] - b'0';
            let row = 30 + slot;
            city.set_mac_object_slot_record(
                slot,
                crate::win95_overlay::Win95OverlayRecord {
                    kind,
                    tile_x: row as u8,
                    tile_y: 50,
                    ..Default::default()
                },
            );
            city.set_text_overlay(row, 50, (0xc9 + slot) as u8);
            city.set_tile_bits(row, 50, TileBits::new(if kind == 9 { 4 } else { 0 }));
        }
        let mut quickdraw = MacToolboxRng::new(v[2].parse().unwrap());
        let sounds = tick_mac_marina_actors_and_count_kind3(
            &mut city,
            &mut quickdraw,
            v[1].parse().unwrap(),
        );
        let expected_sounds: Vec<u16> = if v[8].is_empty() {
            Vec::new()
        } else {
            v[8].split(';').map(|s| s.parse().unwrap()).collect()
        };
        assert_eq!(
            sounds.iter().map(|s| s.sound_id()).collect::<Vec<_>>(),
            expected_sounds,
            "case {count}"
        );
        assert_eq!(
            city.mac_kind3_actor_counter,
            v[5].parse::<u16>().unwrap(),
            "case {count}"
        );
        assert_eq!(
            city.mac_marina_actor_counter,
            v[6].parse::<u16>().unwrap(),
            "case {count}"
        );
        assert_eq!(
            quickdraw.seed(),
            v[7].parse::<u32>().unwrap(),
            "case {count}"
        );
        for (index, slot) in slots.into_iter().enumerate() {
            let hex = &v[3][index * 24..index * 24 + 24];
            let bytes: Vec<u8> = (0..12)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
                .collect();
            assert_eq!(
                city.traffic_overlays().unwrap()[slot],
                crate::win95_overlay::Win95OverlayRecord::from_bytes(&bytes),
                "case {count}, slot {slot}"
            );
            let marker = u8::from_str_radix(&v[4][index * 2..index * 2 + 2], 16).unwrap();
            assert_eq!(
                city.text_overlay_at(30 + slot, 50),
                Some(marker),
                "case {count}, slot {slot}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 36);
}

#[test]
fn mac_actor_sweep_prelude_matches_original_68020() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_actor_prelude.csv");
    let mut cases = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let words: [u16; 2] = [v[3].parse().unwrap(), v[4].parse().unwrap()];
        let mut draws = 0;
        let requests = mac_actor_sweep_prelude(
            v[0].parse().unwrap(),
            v[1].parse().unwrap(),
            v[2].parse().unwrap(),
            || {
                let word = words[draws];
                draws += 1;
                word
            },
        );
        let expected: Vec<u16> = if v[6].is_empty() {
            Vec::new()
        } else {
            v[6].split(';').map(|s| s.parse().unwrap()).collect()
        };
        assert_eq!(draws, v[5].parse::<usize>().unwrap(), "{line}");
        assert_eq!(
            requests.iter().map(|s| s.sound_id()).collect::<Vec<_>>(),
            expected,
            "{line}"
        );
        cases += 1;
    }
    assert_eq!(cases, 128);
}

#[test]
fn mac_actor_history_inputs_follow_original_xgrp_pointers() {
    let fixture = include_str!("../../../tests/fixtures/simulation/mac/mac_xgrp_pointers.csv");
    let pointers: Vec<(usize, usize)> = fixture
        .lines()
        .skip(1)
        .map(|line| {
            let (series, offset) = line.split_once(',').unwrap();
            (series.parse().unwrap(), offset.parse().unwrap())
        })
        .collect();
    assert_eq!(pointers.len(), 16);
    assert_eq!(pointers[4], (4, 4 * 52 * 4));
    assert_eq!(pointers[7], (7, 7 * 52 * 4));
    let mut city = City::new_blank("/tmp/mac-actor-history.sc2", "History").unwrap();
    city.kind = CityFileKind::MacCity;
    let mut history: Vec<u32> = (0..16 * 52).map(|v| 0x1000 + v as u32).collect();
    history[4 * 52] = 41;
    history[7 * 52] = 52;
    city.set_chunk_u32_values("XGRP", 0, &history);
    city.set_mac_state_misc_u32_at(0x01f0 + 0xd2 * 4, 7);
    assert_eq!(mac_actor_sweep_history_inputs(&city), Some((41, 52, 7)));
}

#[test]
fn mac_kind_nine_fifth_actor_is_removed_after_counting_visit() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-nine-cap.sc2"),
        "MacKindNineCap",
    )
    .expect("blank city");
    for slot in XTHG_FIRST_USABLE_SLOT..=XTHG_FIRST_USABLE_SLOT + 4 {
        let x = 20 + slot;
        let y = 30;
        city.set_tile_bits(x, y, TileBits::new(0x04));
        city.set_text_overlay(x, y, (0xc9 + slot) as u8);
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: XTHG_SAILBOAT,
            tile_x: x as u8,
            tile_y: y as u8,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };
    }
    let mut quickdraw = MacToolboxRng::new(1);
    let mut expected = quickdraw;
    for _ in 0..4 {
        expected.random_i16();
    }
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 2);
    assert_eq!(city.mac_marina_actor_counter, 5);
    let records = city.traffic_overlays().unwrap();
    assert!((1..=4).all(|slot| records[slot].state == 1));
    assert_eq!(records[5].kind, 0);
    assert_eq!(city.text_overlay_at(25, 30), Some(0));
    assert_eq!(quickdraw.seed(), expected.seed());
}

/// Optional proprietary-input differential. Set `SC2K_MAC_ACTOR_CGMS_INPUT`
/// and `SC2K_MAC_ACTOR_EXPECTED_XTHG` to replay the first Seven Spires actor
/// sweep against the native phase-3 XTHG snapshot without checking city data
/// into the repository.
#[test]
fn mac_sevenspires_first_actor_sweep_matches_native_xthg_snapshot() {
    let (Ok(input), Ok(expected_xthg)) = (
        std::env::var("SC2K_MAC_ACTOR_CGMS_INPUT"),
        std::env::var("SC2K_MAC_ACTOR_EXPECTED_XTHG"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read Seven Spires CGMS v4");
    let mut city =
        crate::city::city_from_cgms_v4_for_test(&input).expect("decode Seven Spires CGMS v4");
    let mut quickdraw = crate::mac_random::MacToolboxRng::new(1);
    tick_mac_actors_with_rng(
        &mut city,
        &mut quickdraw,
        &mut crate::mac_random::MacGameLcgRng::new(1_103_527_590),
        0,
    );

    let output = crate::city::cgms_v4_from_city_for_test(&city).expect("encode post-sweep CGMS");
    let xtxt_start = 20 + 5 * CITY_TILES;
    let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
    let expected = std::fs::read(expected_xthg).expect("read native phase-3 XTHG");
    assert_eq!(expected.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(&output[actor_start..], expected);
    assert_eq!(
        &output[xtxt_start..xtxt_start + CITY_TILES],
        &input[xtxt_start..xtxt_start + CITY_TILES],
        "native isolated actor sweep leaves the complete XTXT plane unchanged"
    );
    assert_eq!(quickdraw.seed(), 984_943_658);
}

/// Optional saved-state CODE 6 comparison. The input and expected CGMS
/// snapshots stay outside the repository; this pins a native first actor
/// sweep or later chained sweep independently of the saved-file layout.
#[test]
fn mac_saved_actor_sweep_matches_native_snapshot_on_both_layouts() {
    let (Ok(input), Ok(expected), Ok(qd_final), Ok(game_final)) = (
        std::env::var("CITYGAME_MAC_ACTOR_CGMS_INPUT"),
        std::env::var("CITYGAME_MAC_ACTOR_EXPECTED_CGMS"),
        std::env::var("CITYGAME_MAC_ACTOR_EXPECTED_QD_FINAL"),
        std::env::var("CITYGAME_MAC_ACTOR_EXPECTED_GAME_FINAL"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read actor-sweep input CGMS v4");
    let expected = std::fs::read(expected).expect("read native post-sweep CGMS v4");
    let parse_hex = |value: &str| {
        u32::from_str_radix(value.trim_start_matches("0x"), 16).expect("hex RNG seed")
    };
    let qd_seed = std::env::var("CITYGAME_MAC_ACTOR_QD_SEED")
        .map(|value| parse_hex(&value))
        .unwrap_or(1);
    let game_seed = std::env::var("CITYGAME_MAC_ACTOR_GAME_SEED")
        .map(|value| parse_hex(&value))
        .unwrap_or(1);
    let qd_final = parse_hex(&qd_final);
    let game_final = parse_hex(&game_final);
    let xtxt_start = 20 + 5 * CITY_TILES;
    let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
    let source_city = crate::city::city_from_cgms_v4_for_test(&input)
        .expect("decode actor-sweep input CGMS v4");

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = source_city.clone();
        city.kind = kind;
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(qd_seed);
        let mut game_rng = crate::mac_random::MacGameLcgRng::new(game_seed);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);

        let output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode post-sweep CGMS v4");
        assert_eq!(
            &output[actor_start..],
            &expected[actor_start..],
            "XTHG for {kind:?}"
        );
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            &expected[xtxt_start..xtxt_start + CITY_TILES],
            "XTXT for {kind:?}"
        );
        assert_eq!(quickdraw.seed(), qd_final, "QuickDraw state for {kind:?}");
        assert_eq!(game_rng.state(), game_final, "game-LCG state for {kind:?}");
    }
}

/// Optional native CODE 2 differential through the second scheduled actor
/// sweep. The native snapshots are taken after the phase-3 scan partition,
/// the second CODE 6 pass, and the phase-4 scan partition.
/// The QuickDraw seed is reset to the native second-sweep entry value because
/// continuous CODE 14 draw chronology is not yet matched by the Rust prefix.
#[test]
fn mac_sevenspires_second_actor_sweep_matches_native_snapshot_on_both_layouts() {
    let (Ok(input), Ok(expected_xthg), Ok(expected_xtxt)) = (
        std::env::var("SC2K_MAC_ACTOR_CGMS_INPUT"),
        std::env::var("SC2K_MAC_ACTOR_PHASE4_XTHG"),
        std::env::var("SC2K_MAC_ACTOR_PHASE4_XTXT"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read Seven Spires CGMS v4");
    let expected_xthg = std::fs::read(expected_xthg).expect("read native phase-4 XTHG");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native phase-4 XTXT");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city =
            crate::city::city_from_cgms_v4_for_test(&input).expect("decode Seven Spires CGMS v4");
        city.kind = kind;
        for slot in 0..8 {
            city.set_mac_rci_counter(slot, 0);
        }
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(1);
        let mut game_rng = crate::mac_random::MacGameLcgRng::new(1_103_527_590);

        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(quickdraw.seed(), 984_943_658, "first actor sweep: {kind:?}");

        let mut sound_deadline = 0;
        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            0,
            0,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );

        // CODE 14's still-open draw chronology is deliberately not inferred
        // from this Rust scan prefix. Align to the native CODE 6 entry seed so
        // this boundary isolates the second actor transition.
        quickdraw = crate::mac_random::MacToolboxRng::new(1_974_593_928);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(
            quickdraw.seed(),
            1_974_593_928,
            "second actor sweep consumes no QuickDraw values: {kind:?}"
        );
        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            0,
            1,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let output =
            crate::city::cgms_v4_from_city_for_test(&city).expect("encode post-sweep CGMS v4");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
        assert_eq!(&output[actor_start..], expected_xthg, "XTHG for {kind:?}");
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "XTXT for {kind:?}"
        );
    }
}

/// Join native CODE 2 phases 3–10 through the iteration-32 actor pass. External
/// native plane snapshots and post-actor XTHG/XTXT remain outside the repo.
#[test]
fn mac_sevenspires_actor_scan_join_through_iteration32_matches_native() {
    let (
        Ok(input),
        Ok(phase5_dir),
        Ok(expected_xthg),
        Ok(expected_xtxt),
        Ok(phase6_dir),
        Ok(phase7_dir),
        Ok(phase8_dir),
        Ok(phase9_dir),
        Ok(phase10_dir),
    ) = (
        std::env::var("SC2K_MAC_ACTOR_CGMS_INPUT"),
        std::env::var("SC2K_MAC_ACTOR_PHASE5_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_ITER12_NATIVE_XTHG"),
        std::env::var("SC2K_MAC_ACTOR_ITER12_NATIVE_XTXT"),
        std::env::var("SC2K_MAC_ACTOR_PHASE6_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE7_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE8_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE9_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE10_NATIVE_DIR"),
    )
    else {
        return;
    };
    let input = std::fs::read(input).expect("read Seven Spires CGMS v4");
    let phase5_dir = std::path::PathBuf::from(phase5_dir);
    let phase6_dir = std::path::PathBuf::from(phase6_dir);
    let phase7_dir = std::path::PathBuf::from(phase7_dir);
    let phase8_dir = std::path::PathBuf::from(phase8_dir);
    let phase9_dir = std::path::PathBuf::from(phase9_dir);
    let phase10_dir = std::path::PathBuf::from(phase10_dir);
    let expected_xthg = std::fs::read(expected_xthg).expect("read native iteration-12 XTHG");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native iteration-12 XTXT");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);
    let city =
        crate::city::city_from_cgms_v4_for_test(&input).expect("decode Seven Spires CGMS v4");

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = city.clone();
        city.kind = kind;
        for slot in 0..8 {
            city.set_mac_rci_counter(slot, 0);
        }
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(1);
        let mut game_rng = crate::mac_random::MacGameLcgRng::new(1_103_527_590);
        let mut sound_deadline = 0;

        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            0,
            0,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        quickdraw = crate::mac_random::MacToolboxRng::new(1_974_593_928);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            0,
            1,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        quickdraw = crate::mac_random::MacToolboxRng::new(1_783_878_630);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            0,
            2,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );

        let phase5 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-5 pre-actor state");
        let n = CITY_TILES;
        for (name, offset, length) in [
            ("xbld", 20 + n, n),
            ("xzon", 20 + 2 * n, n),
            ("xbit", 20 + 3 * n, n),
            ("xund", 20 + 4 * n, n),
            ("xtxt", 20 + 5 * n, n),
            ("altm", 20 + 9 * n, 2 * n),
            ("xthg", 20 + 12 * n + 1_200 * 4, XTHG_RECORD_COUNT * 12),
        ] {
            let expected = std::fs::read(phase5_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-5 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-5 {name} size");
            assert_eq!(
                &phase5[offset..offset + length],
                expected,
                "phase-5 {name} for {kind:?}"
            );
        }

        // Native XTRF is 64×64. CityGame stores the recovered 2×2 expansion.
        let native_xtrf =
            std::fs::read(phase5_dir.join("xtrf.bin")).expect("read native phase-5 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(
            &phase5[20 + 11 * n..20 + 12 * n],
            expanded_xtrf,
            "phase-5 XTRF expansion for {kind:?}"
        );

        quickdraw = crate::mac_random::MacToolboxRng::new(314_484_211);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(
            quickdraw.seed(),
            1_118_600_160,
            "iteration-12 actor pass consumes two QuickDraw values: {kind:?}"
        );
        let output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust post-iteration-12 actor state");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 12 * CITY_TILES + 1_200 * 4;
        assert_eq!(
            &output[actor_start..],
            expected_xthg,
            "iteration-12 actor XTHG for {kind:?}"
        );
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "iteration-12 actor XTXT for {kind:?}"
        );

        // CODE 14 phase 6 is the next native partition, followed by the
        // iteration-16 CODE 6 sweep. This extends the corrected join without
        // injecting a new QuickDraw seed between the phase-5 actor and scan.
        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            0,
            3,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase6 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-6 pre-actor state");
        let n = CITY_TILES;
        for (name, offset, length) in [
            ("xbld", 20 + n, n),
            ("xzon", 20 + 2 * n, n),
            ("xbit", 20 + 3 * n, n),
            ("xund", 20 + 4 * n, n),
            ("xtxt", 20 + 5 * n, n),
            ("altm", 20 + 9 * n, 2 * n),
            ("xthg", 20 + 12 * n + 1_200 * 4, XTHG_RECORD_COUNT * 12),
        ] {
            let expected = std::fs::read(phase6_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-6 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-6 {name} size");
            assert_eq!(
                &phase6[offset..offset + length],
                expected,
                "phase-6 {name} before iteration-16 actor for {kind:?}"
            );
        }

        let native_xtrf =
            std::fs::read(phase6_dir.join("xtrf.bin")).expect("read native phase-6 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(
            &phase6[20 + 11 * n..20 + 12 * n],
            expanded_xtrf,
            "phase-6 XTRF expansion before iteration-16 actor for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            1_858_765_642,
            "iteration-16 native actor entry seed for {kind:?}"
        );

        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(output_path) = std::env::var("SC2K_MAC_ACTOR_ITER16_PREACTOR_CGMS_OUTPUT") {
                std::fs::write(output_path, &phase6)
                    .expect("write Rust iteration-16 pre-actor CGMS");
            }
        }
        let iter16_expected = match (
            std::env::var("SC2K_MAC_ACTOR_ITER16_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER16_NATIVE_XTXT"),
        ) {
            (Ok(xthg), Ok(xtxt)) => Some((xthg, xtxt)),
            _ => None,
        };
        let Some((iter16_expected_xthg, iter16_expected_xtxt)) = iter16_expected else {
            continue;
        };
        let iter16_expected_xthg =
            std::fs::read(iter16_expected_xthg).expect("read native iteration-16 XTHG");
        let iter16_expected_xtxt =
            std::fs::read(iter16_expected_xtxt).expect("read native iteration-16 XTXT");
        assert_eq!(iter16_expected_xthg.len(), XTHG_RECORD_COUNT * 12);
        assert_eq!(iter16_expected_xtxt.len(), CITY_TILES);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(
            quickdraw.seed(),
            483_596_971,
            "iteration-16 actor pass consumes two QuickDraw values: {kind:?}"
        );
        let output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust post-iteration-16 actor state");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 12 * CITY_TILES + 1_200 * 4;
        assert_eq!(
            &output[actor_start..],
            iter16_expected_xthg,
            "iteration-16 actor XTHG for {kind:?}"
        );
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            iter16_expected_xtxt,
            "iteration-16 actor XTXT for {kind:?}"
        );

        // Continue to native CODE 14 phase 7 (partition 1,0) and the
        // iteration-20 actor entry, preserving the shared QuickDraw stream.
        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            1,
            0,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase7 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-7 pre-actor state");
        for (name, offset, length) in [
            ("xbld", 20 + n, n),
            ("xzon", 20 + 2 * n, n),
            ("xbit", 20 + 3 * n, n),
            ("xund", 20 + 4 * n, n),
            ("xtxt", 20 + 5 * n, n),
            ("altm", 20 + 9 * n, 2 * n),
            ("xthg", 20 + 12 * n + 1_200 * 4, XTHG_RECORD_COUNT * 12),
        ] {
            let expected = std::fs::read(phase7_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-7 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-7 {name} size");
            assert_eq!(
                &phase7[offset..offset + length],
                expected,
                "phase-7 {name} before iteration-20 actor for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase7_dir.join("xtrf.bin")).expect("read native phase-7 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(
            &phase7[20 + 11 * n..20 + 12 * n],
            expanded_xtrf,
            "phase-7 XTRF expansion before iteration-20 actor for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            741_378_047,
            "iteration-20 native actor entry seed for {kind:?}"
        );
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(output_path) = std::env::var("SC2K_MAC_ACTOR_ITER20_PREACTOR_CGMS_OUTPUT") {
                std::fs::write(output_path, &phase7)
                    .expect("write Rust iteration-20 pre-actor CGMS");
            }
        }
        let iter20_expected = match (
            std::env::var("SC2K_MAC_ACTOR_ITER20_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER20_NATIVE_XTXT"),
        ) {
            (Ok(xthg), Ok(xtxt)) => Some((xthg, xtxt)),
            _ => None,
        };
        let Some((iter20_expected_xthg, iter20_expected_xtxt)) = iter20_expected else {
            continue;
        };
        let iter20_expected_xthg =
            std::fs::read(iter20_expected_xthg).expect("read native iteration-20 XTHG");
        let iter20_expected_xtxt =
            std::fs::read(iter20_expected_xtxt).expect("read native iteration-20 XTXT");
        assert_eq!(iter20_expected_xthg.len(), XTHG_RECORD_COUNT * 12);
        assert_eq!(iter20_expected_xtxt.len(), CITY_TILES);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(
            quickdraw.seed(),
            741_378_047,
            "iteration-20 actor pass consumes no QuickDraw values: {kind:?}"
        );
        let output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust post-iteration-20 actor state");
        assert_eq!(
            &output[actor_start..],
            iter20_expected_xthg,
            "iteration-20 actor XTHG for {kind:?}"
        );
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            iter20_expected_xtxt,
            "iteration-20 actor XTXT for {kind:?}"
        );

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            1,
            1,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase8 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-8 pre-actor state");
        for (name, offset, length) in [
            ("xbld", 20 + n, n),
            ("xzon", 20 + 2 * n, n),
            ("xbit", 20 + 3 * n, n),
            ("xund", 20 + 4 * n, n),
            ("xtxt", 20 + 5 * n, n),
            ("altm", 20 + 9 * n, 2 * n),
            ("xthg", 20 + 12 * n + 1_200 * 4, XTHG_RECORD_COUNT * 12),
        ] {
            let expected = std::fs::read(phase8_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-8 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-8 {name} size");
            assert_eq!(
                &phase8[offset..offset + length],
                expected,
                "phase-8 {name} before iteration-24 actor for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase8_dir.join("xtrf.bin")).expect("read native phase-8 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(
            &phase8[20 + 11 * n..20 + 12 * n],
            expanded_xtrf,
            "phase-8 XTRF expansion before iteration-24 actor for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            2_069_043_788,
            "iteration-24 native actor entry seed for {kind:?}"
        );
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(output_path) = std::env::var("SC2K_MAC_ACTOR_ITER24_PREACTOR_CGMS_OUTPUT") {
                std::fs::write(output_path, &phase8)
                    .expect("write Rust iteration-24 pre-actor CGMS");
            }
        }
        let iter24_expected = match (
            std::env::var("SC2K_MAC_ACTOR_ITER24_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER24_NATIVE_XTXT"),
        ) {
            (Ok(xthg), Ok(xtxt)) => Some((xthg, xtxt)),
            _ => None,
        };
        let Some((iter24_expected_xthg, iter24_expected_xtxt)) = iter24_expected else {
            continue;
        };
        let iter24_expected_xthg =
            std::fs::read(iter24_expected_xthg).expect("read native iteration-24 XTHG");
        let iter24_expected_xtxt =
            std::fs::read(iter24_expected_xtxt).expect("read native iteration-24 XTXT");
        assert_eq!(iter24_expected_xthg.len(), XTHG_RECORD_COUNT * 12);
        assert_eq!(iter24_expected_xtxt.len(), CITY_TILES);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(
            quickdraw.seed(),
            955_368_591,
            "iteration-24 actor pass consumes the native QuickDraw draws: {kind:?}"
        );
        let output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust post-iteration-24 actor state");
        assert_eq!(
            &output[actor_start..],
            iter24_expected_xthg,
            "iteration-24 actor XTHG for {kind:?}"
        );
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            iter24_expected_xtxt,
            "iteration-24 actor XTXT for {kind:?}"
        );

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            1,
            2,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase9 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-9 pre-actor state");
        for (name, offset, length) in [
            ("xbld", 20 + n, n),
            ("xzon", 20 + 2 * n, n),
            ("xbit", 20 + 3 * n, n),
            ("xund", 20 + 4 * n, n),
            ("xtxt", 20 + 5 * n, n),
            ("altm", 20 + 9 * n, 2 * n),
            ("xthg", 20 + 12 * n + 1_200 * 4, XTHG_RECORD_COUNT * 12),
        ] {
            let expected = std::fs::read(phase9_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-9 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-9 {name} size");
            assert_eq!(
                &phase9[offset..offset + length],
                expected,
                "phase-9 {name} before iteration-28 actor for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase9_dir.join("xtrf.bin")).expect("read native phase-9 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(
            &phase9[20 + 11 * n..20 + 12 * n],
            expanded_xtrf,
            "phase-9 XTRF expansion before iteration-28 actor for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            1_870_484_732,
            "iteration-28 native actor entry seed for {kind:?}"
        );
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(output_path) = std::env::var("SC2K_MAC_ACTOR_ITER28_PREACTOR_CGMS_OUTPUT") {
                std::fs::write(output_path, &phase9)
                    .expect("write Rust iteration-28 pre-actor CGMS");
            }
        }
        let iter28_expected = match (
            std::env::var("SC2K_MAC_ACTOR_ITER28_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER28_NATIVE_XTXT"),
        ) {
            (Ok(xthg), Ok(xtxt)) => Some((xthg, xtxt)),
            _ => None,
        };
        let Some((iter28_expected_xthg, iter28_expected_xtxt)) = iter28_expected else {
            continue;
        };
        let iter28_expected_xthg =
            std::fs::read(iter28_expected_xthg).expect("read native iteration-28 XTHG");
        let iter28_expected_xtxt =
            std::fs::read(iter28_expected_xtxt).expect("read native iteration-28 XTXT");
        assert_eq!(iter28_expected_xthg.len(), XTHG_RECORD_COUNT * 12);
        assert_eq!(iter28_expected_xtxt.len(), CITY_TILES);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(
            quickdraw.seed(),
            865_098_940,
            "iteration-28 actor pass consumes two native QuickDraw values: {kind:?}"
        );
        let output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust post-iteration-28 actor state");
        assert_eq!(
            &output[actor_start..],
            iter28_expected_xthg,
            "iteration-28 actor XTHG for {kind:?}"
        );
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            iter28_expected_xtxt,
            "iteration-28 actor XTXT for {kind:?}"
        );

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            1,
            3,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase10 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-10 pre-actor state");
        for (name, offset, length) in [
            ("xbld", 20 + n, n),
            ("xzon", 20 + 2 * n, n),
            ("xbit", 20 + 3 * n, n),
            ("xund", 20 + 4 * n, n),
            ("xtxt", 20 + 5 * n, n),
            ("altm", 20 + 9 * n, 2 * n),
            ("xthg", 20 + 12 * n + 1_200 * 4, XTHG_RECORD_COUNT * 12),
        ] {
            let expected = std::fs::read(phase10_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-10 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-10 {name} size");
            assert_eq!(
                &phase10[offset..offset + length],
                expected,
                "phase-10 {name} before iteration-32 actor for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase10_dir.join("xtrf.bin")).expect("read native phase-10 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(
            &phase10[20 + 11 * n..20 + 12 * n],
            expanded_xtrf,
            "phase-10 XTRF expansion before iteration-32 actor for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            412_729_450,
            "iteration-32 native actor entry seed for {kind:?}"
        );
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(output_path) = std::env::var("SC2K_MAC_ACTOR_ITER32_PREACTOR_CGMS_OUTPUT") {
                std::fs::write(output_path, &phase10)
                    .expect("write Rust iteration-32 pre-actor CGMS");
            }
        }
        let iter32_expected = match (
            std::env::var("SC2K_MAC_ACTOR_ITER32_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER32_NATIVE_XTXT"),
        ) {
            (Ok(xthg), Ok(xtxt)) => Some((xthg, xtxt)),
            _ => None,
        };
        let Some((iter32_expected_xthg, iter32_expected_xtxt)) = iter32_expected else {
            continue;
        };
        let iter32_expected_xthg =
            std::fs::read(iter32_expected_xthg).expect("read native iteration-32 XTHG");
        let iter32_expected_xtxt =
            std::fs::read(iter32_expected_xtxt).expect("read native iteration-32 XTXT");
        assert_eq!(iter32_expected_xthg.len(), XTHG_RECORD_COUNT * 12);
        assert_eq!(iter32_expected_xtxt.len(), CITY_TILES);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(
            quickdraw.seed(),
            412_729_450,
            "iteration-32 actor pass consumes no QuickDraw values: {kind:?}"
        );
        let output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust post-iteration-32 actor state");
        assert_eq!(
            &output[actor_start..],
            iter32_expected_xthg,
            "iteration-32 actor XTHG for {kind:?}"
        );
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            iter32_expected_xtxt,
            "iteration-32 actor XTXT for {kind:?}"
        );
    }
}

/// Continue from the native-matched iteration-32 state through CODE 14 phases
/// 11–18 and CODE 6 iterations 36–60. External native snapshots stay outside
/// the repository.
#[test]
fn mac_sevenspires_phase11_to18_actor36_to60_matches_native() {
    let (
        Ok(input),
        Ok(phase11_dir),
        Ok(phase12_dir),
        Ok(iter40_expected_xthg),
        Ok(iter40_expected_xtxt),
        Ok(phase13_dir),
        Ok(phase14_dir),
        Ok(phase15_dir),
        Ok(phase16_dir),
        Ok(phase17_dir),
        Ok(phase18_dir),
    ) = (
        std::env::var("SC2K_MAC_ACTOR_AFTER32_NATIVE_CGMS"),
        std::env::var("SC2K_MAC_ACTOR_PHASE11_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE12_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_ITER40_NATIVE_XTHG"),
        std::env::var("SC2K_MAC_ACTOR_ITER40_NATIVE_XTXT"),
        std::env::var("SC2K_MAC_ACTOR_PHASE13_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE14_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE15_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE16_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE17_NATIVE_DIR"),
        std::env::var("SC2K_MAC_ACTOR_PHASE18_NATIVE_DIR"),
    )
    else {
        return;
    };
    let input = std::fs::read(input).expect("read native-matched post-iteration-32 CGMS");
    let phase11_dir = std::path::PathBuf::from(phase11_dir);
    let phase12_dir = std::path::PathBuf::from(phase12_dir);
    let phase13_dir = std::path::PathBuf::from(phase13_dir);
    let phase14_dir = std::path::PathBuf::from(phase14_dir);
    let phase15_dir = std::path::PathBuf::from(phase15_dir);
    let phase16_dir = std::path::PathBuf::from(phase16_dir);
    let phase17_dir = std::path::PathBuf::from(phase17_dir);
    let phase18_dir = std::path::PathBuf::from(phase18_dir);
    let iter40_expected_xthg =
        std::fs::read(iter40_expected_xthg).expect("read native iteration-40 XTHG");
    let iter40_expected_xtxt =
        std::fs::read(iter40_expected_xtxt).expect("read native iteration-40 XTXT");
    let base = crate::city::city_from_cgms_v4_for_test(&input)
        .expect("decode post-iteration-32 Seven Spires CGMS");
    let n = CITY_TILES;
    let planes = [
        ("xbld", 20 + n, n),
        ("xzon", 20 + 2 * n, n),
        ("xbit", 20 + 3 * n, n),
        ("xund", 20 + 4 * n, n),
        ("xtxt", 20 + 5 * n, n),
        ("altm", 20 + 9 * n, 2 * n),
        ("xthg", 20 + 12 * n + 1_200 * 4, XTHG_RECORD_COUNT * 12),
    ];

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = base.clone();
        city.kind = kind;
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(412_729_450);
        let mut game_rng = crate::mac_random::MacGameLcgRng::new(1_103_527_590);
        let mut sound_deadline = 0;

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            2,
            0,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase11 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-11 pre-actor state");
        for (name, offset, length) in planes {
            let expected = std::fs::read(phase11_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-11 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-11 {name} size");
            assert_eq!(
                &phase11[offset..offset + length],
                expected,
                "phase-11 {name} for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase11_dir.join("xtrf.bin")).expect("read native phase-11 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(&phase11[20 + 11 * n..20 + 12 * n], expanded_xtrf);
        assert_eq!(quickdraw.seed(), 2_132_332_715);
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(path) = std::env::var("SC2K_MAC_ACTOR_PHASE11_RUST_CGMS_OUTPUT") {
                std::fs::write(path, &phase11).expect("write Rust phase-11 CGMS");
            }
        }

        let before_iteration36 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust iteration-36 actor input");
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(quickdraw.seed(), 2_132_332_715);
        let after_iteration36 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust iteration-36 actor output");
        assert_eq!(
            after_iteration36, before_iteration36,
            "native iteration-36 pass is a no-op for this city state"
        );
        if let (Ok(expected_xthg), Ok(expected_xtxt)) = (
            std::env::var("SC2K_MAC_ACTOR_ITER36_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER36_NATIVE_XTXT"),
        ) {
            let expected_xthg = std::fs::read(expected_xthg).expect("read iteration-36 XTHG");
            let expected_xtxt = std::fs::read(expected_xtxt).expect("read iteration-36 XTXT");
            let actor_start = 20 + 12 * n + 1_200 * 4;
            let xtxt_start = 20 + 5 * n;
            assert_eq!(&after_iteration36[actor_start..], expected_xthg);
            assert_eq!(
                &after_iteration36[xtxt_start..xtxt_start + n],
                expected_xtxt
            );
        }

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            2,
            1,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase12 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-12 pre-actor state");
        for (name, offset, length) in planes {
            let expected = std::fs::read(phase12_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-12 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-12 {name} size");
            assert_eq!(
                &phase12[offset..offset + length],
                expected,
                "phase-12 {name} for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase12_dir.join("xtrf.bin")).expect("read native phase-12 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(&phase12[20 + 11 * n..20 + 12 * n], expanded_xtrf);
        assert_eq!(quickdraw.seed(), 1_299_116_765);
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(path) = std::env::var("SC2K_MAC_ACTOR_PHASE12_RUST_CGMS_OUTPUT") {
                std::fs::write(path, &phase12).expect("write Rust phase-12 CGMS");
            }
        }

        assert_eq!(iter40_expected_xthg.len(), XTHG_RECORD_COUNT * 12);
        assert_eq!(iter40_expected_xtxt.len(), n);
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(quickdraw.seed(), 789_230_306);
        let output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust iteration-40 actor output");
        let actor_start = 20 + 12 * n + 1_200 * 4;
        let xtxt_start = 20 + 5 * n;
        assert_eq!(&output[actor_start..], iter40_expected_xthg);
        assert_eq!(&output[xtxt_start..xtxt_start + n], iter40_expected_xtxt);

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            2,
            2,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase13 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-13 pre-actor state");
        for (name, offset, length) in planes {
            let expected = std::fs::read(phase13_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-13 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-13 {name} size");
            assert_eq!(
                &phase13[offset..offset + length],
                expected,
                "phase-13 {name} for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase13_dir.join("xtrf.bin")).expect("read native phase-13 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(&phase13[20 + 11 * n..20 + 12 * n], expanded_xtrf);
        assert_eq!(quickdraw.seed(), 1_332_838_706);
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(path) = std::env::var("SC2K_MAC_ACTOR_PHASE13_RUST_CGMS_OUTPUT") {
                std::fs::write(path, &phase13).expect("write Rust phase-13 CGMS");
            }
        }
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(quickdraw.seed(), 1_332_838_706);
        let iter44_output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust iteration-44 actor output");
        if let (Ok(expected_xthg), Ok(expected_xtxt)) = (
            std::env::var("SC2K_MAC_ACTOR_ITER44_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER44_NATIVE_XTXT"),
        ) {
            let expected_xthg = std::fs::read(expected_xthg).expect("read iteration-44 XTHG");
            let expected_xtxt = std::fs::read(expected_xtxt).expect("read iteration-44 XTXT");
            assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
            assert_eq!(expected_xtxt.len(), n);
            let actor_start = 20 + 12 * n + 1_200 * 4;
            let xtxt_start = 20 + 5 * n;
            assert_eq!(&iter44_output[actor_start..], expected_xthg);
            assert_eq!(&iter44_output[xtxt_start..xtxt_start + n], expected_xtxt);
        }

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            2,
            3,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase14 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-14 pre-actor state");
        for (name, offset, length) in planes {
            let expected = std::fs::read(phase14_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-14 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-14 {name} size");
            assert_eq!(
                &phase14[offset..offset + length],
                expected,
                "phase-14 {name} for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase14_dir.join("xtrf.bin")).expect("read native phase-14 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(&phase14[20 + 11 * n..20 + 12 * n], expanded_xtrf);
        assert_eq!(quickdraw.seed(), 1_657_056_745);
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(path) = std::env::var("SC2K_MAC_ACTOR_PHASE14_RUST_CGMS_OUTPUT") {
                std::fs::write(path, &phase14).expect("write Rust phase-14 CGMS");
            }
        }
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(quickdraw.seed(), 139_617_892);
        let iter48_output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust iteration-48 actor output");
        if let (Ok(expected_xthg), Ok(expected_xtxt)) = (
            std::env::var("SC2K_MAC_ACTOR_ITER48_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER48_NATIVE_XTXT"),
        ) {
            let expected_xthg = std::fs::read(expected_xthg).expect("read iteration-48 XTHG");
            let expected_xtxt = std::fs::read(expected_xtxt).expect("read iteration-48 XTXT");
            assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
            assert_eq!(expected_xtxt.len(), n);
            let actor_start = 20 + 12 * n + 1_200 * 4;
            let xtxt_start = 20 + 5 * n;
            assert_eq!(&iter48_output[actor_start..], expected_xthg);
            assert_eq!(&iter48_output[xtxt_start..xtxt_start + n], expected_xtxt);
        }

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            3,
            0,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase15 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-15 pre-actor state");
        for (name, offset, length) in planes {
            let expected = std::fs::read(phase15_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-15 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-15 {name} size");
            assert_eq!(
                &phase15[offset..offset + length],
                expected,
                "phase-15 {name} for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase15_dir.join("xtrf.bin")).expect("read native phase-15 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(&phase15[20 + 11 * n..20 + 12 * n], expanded_xtrf);
        assert_eq!(quickdraw.seed(), 570_284_233);
        assert_eq!(city.mac_marina_actor_counter, 3);
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(path) = std::env::var("SC2K_MAC_ACTOR_PHASE15_RUST_CGMS_OUTPUT") {
                std::fs::write(path, &phase15).expect("write Rust phase-15 CGMS");
            }
        }
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(quickdraw.seed(), 547_587_470);
        assert_eq!(city.mac_marina_actor_counter, 3);
        let iter52_output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust iteration-52 actor output");
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(path) = std::env::var("SC2K_MAC_ACTOR_ITER52_RUST_CGMS_OUTPUT") {
                std::fs::write(path, &iter52_output).expect("write Rust iteration-52 CGMS");
            }
        }
        if let (Ok(expected_xthg), Ok(expected_xtxt)) = (
            std::env::var("SC2K_MAC_ACTOR_ITER52_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER52_NATIVE_XTXT"),
        ) {
            let expected_xthg = std::fs::read(expected_xthg).expect("read iteration-52 XTHG");
            let expected_xtxt = std::fs::read(expected_xtxt).expect("read iteration-52 XTXT");
            let actor_start = 20 + 12 * n + 1_200 * 4;
            let xtxt_start = 20 + 5 * n;
            assert_eq!(&iter52_output[actor_start..], expected_xthg);
            assert_eq!(&iter52_output[xtxt_start..xtxt_start + n], expected_xtxt);
        }

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            3,
            1,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase16 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-16 pre-actor state");
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(path) = std::env::var("SC2K_MAC_ACTOR_PHASE16_RUST_CGMS_OUTPUT") {
                std::fs::write(path, &phase16).expect("write Rust phase-16 CGMS");
            }
        }
        for (name, offset, length) in planes {
            let expected = std::fs::read(phase16_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-16 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-16 {name} size");
            assert_eq!(
                &phase16[offset..offset + length],
                expected,
                "phase-16 {name} for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase16_dir.join("xtrf.bin")).expect("read native phase-16 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(&phase16[20 + 11 * n..20 + 12 * n], expanded_xtrf);
        assert_eq!(quickdraw.seed(), 886_314_074);
        assert_eq!(city.mac_marina_actor_counter, 5);
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(path) = std::env::var("SC2K_MAC_ACTOR_PHASE16_RUST_CGMS_OUTPUT") {
                std::fs::write(path, &phase16).expect("write Rust phase-16 CGMS");
            }
        }
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(quickdraw.seed(), 1_920_105_002);
        let iter56_output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust iteration-56 actor output");
        if let (Ok(expected_xthg), Ok(expected_xtxt)) = (
            std::env::var("SC2K_MAC_ACTOR_ITER56_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER56_NATIVE_XTXT"),
        ) {
            let expected_xthg = std::fs::read(expected_xthg).expect("read iteration-56 XTHG");
            let expected_xtxt = std::fs::read(expected_xtxt).expect("read iteration-56 XTXT");
            let actor_start = 20 + 12 * n + 1_200 * 4;
            let xtxt_start = 20 + 5 * n;
            assert_eq!(&iter56_output[actor_start..], expected_xthg);
            assert_eq!(&iter56_output[xtxt_start..xtxt_start + n], expected_xtxt);
        }

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            3,
            2,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase17 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-17 pre-actor state");
        for (name, offset, length) in planes {
            let expected = std::fs::read(phase17_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-17 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-17 {name} size");
            assert_eq!(
                &phase17[offset..offset + length],
                expected,
                "phase-17 {name} for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase17_dir.join("xtrf.bin")).expect("read native phase-17 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(&phase17[20 + 11 * n..20 + 12 * n], expanded_xtrf);
        assert_eq!(quickdraw.seed(), 304_622_190);
        if kind == crate::city::CityFileKind::MacCity {
            if let Ok(path) = std::env::var("SC2K_MAC_ACTOR_PHASE17_RUST_CGMS_OUTPUT") {
                std::fs::write(path, &phase17).expect("write Rust phase-17 CGMS");
            }
        }
        tick_mac_actors_with_rng(&mut city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(quickdraw.seed(), 184_132_882);
        let iter60_output = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust iteration-60 actor output");
        if let (Ok(expected_xthg), Ok(expected_xtxt)) = (
            std::env::var("SC2K_MAC_ACTOR_ITER60_NATIVE_XTHG"),
            std::env::var("SC2K_MAC_ACTOR_ITER60_NATIVE_XTXT"),
        ) {
            let expected_xthg = std::fs::read(expected_xthg).expect("read iteration-60 XTHG");
            let expected_xtxt = std::fs::read(expected_xtxt).expect("read iteration-60 XTXT");
            let actor_start = 20 + 12 * n + 1_200 * 4;
            let xtxt_start = 20 + 5 * n;
            assert_eq!(&iter60_output[actor_start..], expected_xthg);
            assert_eq!(&iter60_output[xtxt_start..xtxt_start + n], expected_xtxt);
        }

        crate::app::mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut city,
            3,
            3,
            &mut quickdraw,
            &mut game_rng,
            &mut sound_deadline,
        );
        let phase18 = crate::city::cgms_v4_from_city_for_test(&city)
            .expect("encode Rust phase-18 final state");
        for (name, offset, length) in planes {
            let expected = std::fs::read(phase18_dir.join(format!("{name}.bin")))
                .unwrap_or_else(|error| panic!("read native phase-18 {name}: {error}"));
            assert_eq!(expected.len(), length, "native phase-18 {name} size");
            assert_eq!(
                &phase18[offset..offset + length],
                expected,
                "phase-18 {name} for {kind:?}"
            );
        }
        let native_xtrf =
            std::fs::read(phase18_dir.join("xtrf.bin")).expect("read native phase-18 XTRF");
        assert_eq!(native_xtrf.len(), 64 * 64);
        let mut expanded_xtrf = vec![0; CITY_TILES];
        for x in 0..64 {
            for y in 0..64 {
                let value = native_xtrf[x * 64 + y];
                for dx in 0..2 {
                    for dy in 0..2 {
                        expanded_xtrf[(x * 2 + dx) * CITY_WIDTH + y * 2 + dy] = value;
                    }
                }
            }
        }
        assert_eq!(&phase18[20 + 11 * n..20 + 12 * n], expanded_xtrf);
        assert_eq!(quickdraw.seed(), 63_948_679);
        if let Ok(root) = std::env::var("CITYGAME_MAC_SEVEN_SPIRES_LATE_PHASES") {
            compare_mac_seven_spires_late_phases(city, quickdraw, game_rng, root.as_ref(), &planes);
        }
    }
}

fn compare_mac_seven_spires_late_phases(
    mut city: City,
    mut quickdraw: MacToolboxRng,
    mut game_rng: MacGameLcgRng,
    root: &std::path::Path,
    planes: &[(&str, usize, usize)],
) {
    let kind = city.kind;
    if kind.is_win95() {
        // The preceding CGMS actor probe starts from a Mac state image.
        // Restore its complete scan bank into the Win-layout runtime fields.
        city.prepare_mac_fidelity_from_saved_city();
        let counts = std::fs::read(root.join("phase-18/counters.bin")).unwrap();
        for (slot, value) in counts.chunks_exact(4).enumerate() {
            city.set_mac_rci_counter(slot, u32::from_be_bytes(value.try_into().unwrap()));
        }
    }
    // CGMS carries serialized fields rather than City::load's live caches.
    for offset in [0x1c, 0x50, 0x54, 0x58, 0x5c] {
        let saved = city.mac_state_misc_u32_at(offset).unwrap();
        city.set_mac_state_misc_u32_at(offset, saved);
    }
    let base_year = city.mac_state_misc_u32_at(0x0c).unwrap() as u16;
    let elapsed = city.mac_state_misc_u32_at(0x10).unwrap();
    let mut state = test_state(city);
    state.game_clock = crate::game_clock::GameClock::from_city_age(base_year, elapsed);
    state.engine_fidelity = EngineFidelity::Mac68k;
    for phase in 19..=21 {
        tick_mac_actors_with_rng(&mut state.city, &mut quickdraw, &mut game_rng, 0);
        assert_eq!(quickdraw.seed(), 1_043_624_453, "phase {phase} entry");
        state.mac_process_rng = quickdraw;
        match phase {
            19 => state.sim_calendar_day19_traffic(),
            20 => state.sim_calendar_day20_water(),
            21 => {
                state.sim_calendar_day21_demand_and_graphs();
            }
            _ => unreachable!(),
        }
        quickdraw = state.mac_process_rng;
        let data = crate::city::cgms_v4_from_city_for_test(&state.city).unwrap();
        let phase_dir = root.join(format!("phase-{phase:02}"));
        for &(name, offset, length) in planes {
            let native = std::fs::read(phase_dir.join(format!("{name}.bin"))).unwrap();
            let actual = &data[offset..offset + length];
            assert_eq!(actual.len(), native.len(), "{name} plane length");
            let differences = actual
                .iter()
                .zip(native)
                .enumerate()
                .filter_map(|(i, (&actual, expected))| {
                    (actual != expected).then_some((i, actual, expected))
                })
                .take(8)
                .collect::<Vec<_>>();
            assert!(
                differences.is_empty(),
                "{kind:?} phase {phase} {name} differences (offset, Rust, original): {differences:?}"
            );
        }
        let native_xtrf = std::fs::read(phase_dir.join("xtrf.bin")).unwrap();
        assert_eq!(native_xtrf.len(), 64 * 64);
        let traffic = &data[20 + 11 * CITY_TILES..20 + 12 * CITY_TILES];
        let differences = traffic
            .iter()
            .enumerate()
            .filter(|&(i, value)| {
                let (x, y) = (i / CITY_HEIGHT, i % CITY_HEIGHT);
                *value != native_xtrf[(x / 2) * 64 + y / 2]
            })
            .count();
        assert_eq!(differences, 0, "{kind:?} phase {phase} XTRF differences");
        let globals = std::fs::read(phase_dir.join("app_globals.bin")).unwrap();
        let native_u32 =
            |offset| u32::from_be_bytes(globals[offset..offset + 4].try_into().unwrap());
        assert_eq!(
            state.city.mac_state_misc_u32_at(0x30),
            Some(native_u32(0xf002))
        );
        assert_eq!(state.city.mac_water_service_summary(), native_u32(0xf00e));
        if phase == 21 {
            let expected = std::fs::read(phase_dir.join("cohort_lanes.bin")).unwrap();
            let rust = state.city.misc_u32_values(0x7c, 60);
            for (i, actual) in rust.into_iter().enumerate() {
                let offset = (i % 3) * 0x1000 + (i / 3) * 4;
                assert_eq!(
                    actual,
                    u32::from_be_bytes(expected[offset..offset + 4].try_into().unwrap()),
                    "{kind:?} phase 21 cohort dword {i}"
                );
            }
            for (saved, native) in [
                (0x44, 0xf026),
                (0x48, 0xf02e),
                (0x4c, 0xf02a),
                (0x50, 0xf036),
                (0x54, 0xf03a),
                (0x102c, 0xf01a),
            ] {
                assert_eq!(
                    state.city.mac_state_misc_u32_at(saved),
                    Some(native_u32(native)),
                    "{kind:?} phase 21 MISC {saved:#x}"
                );
            }
            assert_eq!(
                state.city.mac_population_change_balances,
                [native_u32(0xf01e), native_u32(0xf022)]
            );
            for (index, offset) in [0x60, 0x64, 0x68, 0x6c].into_iter().enumerate() {
                assert_eq!(
                    state.city.mac_state_misc_u32_at(offset).unwrap() as u8,
                    globals[0xf084 + index],
                    "{kind:?} phase 21 weather byte {index}"
                );
            }
            assert_eq!(state.city.misc_i32_values(0x718, 3), [2000, 1743, 2000]);
            assert_eq!(quickdraw.seed(), 347_739_176);
        }
    }
}

/// Optional native CODE 6 differential for an isolated kind-16 state-2 actor.
/// The generated input and expected plane snapshots stay outside the repo.
#[test]
fn mac_kind16_state_two_sweep_matches_native_on_both_city_layouts() {
    let (Ok(input), Ok(expected_xthg), Ok(expected_xtxt)) = (
        std::env::var("SC2K_MAC_KIND16_CGMS_INPUT"),
        std::env::var("SC2K_MAC_KIND16_EXPECTED_XTHG"),
        std::env::var("SC2K_MAC_KIND16_EXPECTED_XTXT"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read controlled kind-16 CGMS input");
    let expected_xthg = std::fs::read(expected_xthg).expect("read native XTHG output");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native XTXT output");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = crate::city::city_from_cgms_v4_for_test(&input)
            .expect("decode controlled kind-16 CGMS v4");
        city.kind = kind;
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(0);
        tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut crate::mac_random::MacGameLcgRng::new(1),
            0,
        );

        let output =
            crate::city::cgms_v4_from_city_for_test(&city).expect("encode Rust post-sweep CGMS");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
        assert_eq!(&output[actor_start..], expected_xthg, "XTHG for {kind:?}");
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "XTXT for {kind:?}"
        );
        assert_eq!(quickdraw.seed(), 0, "state 2 consumes no QuickDraw draws");
    }
}

/// Optional native CODE 6 differential for the kind-16 state-1 random branch.
#[test]
fn mac_kind16_state_one_sweep_matches_native_on_both_city_layouts() {
    let (Ok(input), Ok(expected_xthg), Ok(expected_xtxt)) = (
        std::env::var("SC2K_MAC_KIND16_STATE1_CGMS_INPUT"),
        std::env::var("SC2K_MAC_KIND16_STATE1_EXPECTED_XTHG"),
        std::env::var("SC2K_MAC_KIND16_STATE1_EXPECTED_XTXT"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read controlled kind-16 state-1 CGMS input");
    let expected_xthg = std::fs::read(expected_xthg).expect("read native XTHG output");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native XTXT output");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = crate::city::city_from_cgms_v4_for_test(&input)
            .expect("decode controlled kind-16 state-1 CGMS v4");
        city.kind = kind;
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(1);
        tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut crate::mac_random::MacGameLcgRng::new(1),
            0,
        );

        let output =
            crate::city::cgms_v4_from_city_for_test(&city).expect("encode Rust post-sweep CGMS");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
        assert_eq!(&output[actor_start..], expected_xthg, "XTHG for {kind:?}");
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "XTXT for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            282_475_249,
            "state 1 consumes exactly two QuickDraw draws"
        );
    }
}

/// Optional native CODE 6 differential for kind-16 target-marker discovery.
#[test]
fn mac_kind16_state_zero_sweep_matches_native_on_both_city_layouts() {
    let (Ok(input), Ok(expected_xthg), Ok(expected_xtxt)) = (
        std::env::var("SC2K_MAC_KIND16_STATE0_CGMS_INPUT"),
        std::env::var("SC2K_MAC_KIND16_STATE0_EXPECTED_XTHG"),
        std::env::var("SC2K_MAC_KIND16_STATE0_EXPECTED_XTXT"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read controlled kind-16 state-0 CGMS input");
    let expected_xthg = std::fs::read(expected_xthg).expect("read native XTHG output");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native XTXT output");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = crate::city::city_from_cgms_v4_for_test(&input)
            .expect("decode controlled kind-16 state-0 CGMS v4");
        city.kind = kind;
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(0);
        tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut crate::mac_random::MacGameLcgRng::new(1),
            0,
        );

        let output =
            crate::city::cgms_v4_from_city_for_test(&city).expect("encode Rust post-sweep CGMS");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
        assert_eq!(&output[actor_start..], expected_xthg, "XTHG for {kind:?}");
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "XTXT for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            0,
            "clear-target scan consumes no random draws"
        );
    }
}

/// Optional native CODE 6 differential for state 0's reserved-marker gate.
#[test]
fn mac_kind16_reserved_marker_gate_matches_native_on_both_city_layouts() {
    let (Ok(input), Ok(expected_xthg), Ok(expected_xtxt)) = (
        std::env::var("SC2K_MAC_KIND16_RESERVED_CGMS_INPUT"),
        std::env::var("SC2K_MAC_KIND16_RESERVED_EXPECTED_XTHG"),
        std::env::var("SC2K_MAC_KIND16_RESERVED_EXPECTED_XTXT"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read controlled reserved-marker CGMS input");
    let expected_xthg = std::fs::read(expected_xthg).expect("read native XTHG output");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native XTXT output");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = crate::city::city_from_cgms_v4_for_test(&input)
            .expect("decode controlled reserved-marker CGMS v4");
        city.kind = kind;
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(1);
        tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut crate::mac_random::MacGameLcgRng::new(1),
            0,
        );

        let output =
            crate::city::cgms_v4_from_city_for_test(&city).expect("encode Rust post-sweep CGMS");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
        assert_eq!(&output[actor_start..], expected_xthg, "XTHG for {kind:?}");
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "XTXT for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            16_807,
            "reserved marker consumes one draw"
        );
    }
}

/// Optional native CODE 6 differential for the reserved-marker no-move gate.
#[test]
fn mac_kind16_reserved_marker_no_move_matches_native_on_both_city_layouts() {
    let (Ok(input), Ok(expected_xthg), Ok(expected_xtxt)) = (
        std::env::var("SC2K_MAC_KIND16_RESERVED_EVEN_CGMS_INPUT"),
        std::env::var("SC2K_MAC_KIND16_RESERVED_EVEN_EXPECTED_XTHG"),
        std::env::var("SC2K_MAC_KIND16_RESERVED_EVEN_EXPECTED_XTXT"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read controlled no-move marker CGMS input");
    let expected_xthg = std::fs::read(expected_xthg).expect("read native XTHG output");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native XTXT output");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = crate::city::city_from_cgms_v4_for_test(&input)
            .expect("decode controlled no-move marker CGMS v4");
        city.kind = kind;
        // Seed 2 produces an even, nonzero first draw (33614), making the
        // consumed-draw assertion observable while preserving the native
        // no-move branch exercised by the fixed-point seed-0 fixture.
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(2);
        tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut crate::mac_random::MacGameLcgRng::new(1),
            0,
        );

        let output =
            crate::city::cgms_v4_from_city_for_test(&city).expect("encode Rust post-sweep CGMS");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
        assert_eq!(&output[actor_start..], expected_xthg, "XTHG for {kind:?}");
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "XTXT for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            33_614,
            "no-move marker gate consumes exactly one even QuickDraw draw"
        );
    }
}

/// Optional native CODE 6 differential for proxy-to-kind-6 conversion.
#[test]
fn mac_kind16_matching_proxy_conversion_matches_native_on_both_city_layouts() {
    let (Ok(input), Ok(expected_xthg), Ok(expected_xtxt)) = (
        std::env::var("SC2K_MAC_KIND16_MATCH_PROXY_CGMS_INPUT"),
        std::env::var("SC2K_MAC_KIND16_MATCH_PROXY_EXPECTED_XTHG"),
        std::env::var("SC2K_MAC_KIND16_MATCH_PROXY_EXPECTED_XTXT"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read controlled matching-proxy CGMS input");
    let expected_xthg = std::fs::read(expected_xthg).expect("read native XTHG output");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native XTXT output");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = crate::city::city_from_cgms_v4_for_test(&input)
            .expect("decode controlled matching-proxy CGMS v4");
        city.kind = kind;
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(4);
        tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut crate::mac_random::MacGameLcgRng::new(1),
            0,
        );

        let output =
            crate::city::cgms_v4_from_city_for_test(&city).expect("encode Rust post-sweep CGMS");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
        assert_eq!(&output[actor_start..], expected_xthg, "XTHG for {kind:?}");
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "XTXT for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            67_228,
            "matching-proxy conversion consumes exactly one QuickDraw draw"
        );
    }
}

/// Optional native CODE 6 differential for the declined conversion gate.
#[test]
fn mac_kind16_matching_proxy_declined_gate_matches_native_on_both_city_layouts() {
    let (Ok(input), Ok(expected_xthg), Ok(expected_xtxt)) = (
        std::env::var("SC2K_MAC_KIND16_MATCH_PROXY_DECLINE_CGMS_INPUT"),
        std::env::var("SC2K_MAC_KIND16_MATCH_PROXY_DECLINE_EXPECTED_XTHG"),
        std::env::var("SC2K_MAC_KIND16_MATCH_PROXY_DECLINE_EXPECTED_XTXT"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read controlled matching-proxy decline CGMS input");
    let expected_xthg = std::fs::read(expected_xthg).expect("read native XTHG output");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native XTXT output");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = crate::city::city_from_cgms_v4_for_test(&input)
            .expect("decode controlled matching-proxy decline CGMS v4");
        city.kind = kind;
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(1);
        tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut crate::mac_random::MacGameLcgRng::new(1),
            0,
        );

        let output =
            crate::city::cgms_v4_from_city_for_test(&city).expect("encode Rust post-sweep CGMS");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
        assert_eq!(&output[actor_start..], expected_xthg, "XTHG for {kind:?}");
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "XTXT for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            16_807,
            "declined matching-proxy gate consumes exactly one QuickDraw draw"
        );
    }
}

/// Optional native CODE 6 differential for proxy allocation behind the cursor.
#[test]
fn mac_kind16_matching_proxy_reuses_behind_cursor_slot_until_next_sweep() {
    let (Ok(input), Ok(expected_xthg), Ok(expected_xtxt)) = (
        std::env::var("SC2K_MAC_KIND16_BEHIND_CURSOR_CGMS_INPUT"),
        std::env::var("SC2K_MAC_KIND16_BEHIND_CURSOR_EXPECTED_XTHG"),
        std::env::var("SC2K_MAC_KIND16_BEHIND_CURSOR_EXPECTED_XTXT"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read controlled behind-cursor CGMS input");
    let expected_xthg = std::fs::read(expected_xthg).expect("read native XTHG output");
    let expected_xtxt = std::fs::read(expected_xtxt).expect("read native XTXT output");
    assert_eq!(expected_xthg.len(), XTHG_RECORD_COUNT * 12);
    assert_eq!(expected_xtxt.len(), CITY_TILES);

    for kind in [
        crate::city::CityFileKind::MacCity,
        crate::city::CityFileKind::WinSc2,
    ] {
        let mut city = crate::city::city_from_cgms_v4_for_test(&input)
            .expect("decode controlled behind-cursor CGMS v4");
        city.kind = kind;
        let mut quickdraw = crate::mac_random::MacToolboxRng::new(4);
        tick_mac_actors_with_rng(
            &mut city,
            &mut quickdraw,
            &mut crate::mac_random::MacGameLcgRng::new(1),
            0,
        );

        let output =
            crate::city::cgms_v4_from_city_for_test(&city).expect("encode Rust post-sweep CGMS");
        let xtxt_start = 20 + 5 * CITY_TILES;
        let actor_start = 20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4;
        assert_eq!(&output[actor_start..], expected_xthg, "XTHG for {kind:?}");
        assert_eq!(
            &output[xtxt_start..xtxt_start + CITY_TILES],
            expected_xtxt,
            "XTXT for {kind:?}"
        );
        assert_eq!(
            quickdraw.seed(),
            67_228,
            "behind-cursor conversion consumes exactly one QuickDraw draw"
        );
    }
}

#[test]
fn mac_actor_sweep_rebuilds_kind3_counter_and_keeps_kind_six_handoff() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-counter-sweep.sc2"),
        "MacKindThreeCounter",
    )
    .expect("blank city");
    city.mac_kind3_actor_counter = 9;
    for slot in [1, 4] {
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };
    }

    let mut quickdraw = MacToolboxRng::new(1);
    let mut expected = quickdraw;
    expected.random_i16();
    expected.random_i16();
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 2);
    assert_eq!(city.mac_kind3_actor_counter, 2);
    assert_eq!(city.traffic_overlays().unwrap()[1].kind, 6);
    assert_eq!(city.traffic_overlays().unwrap()[4].kind, 6);
    assert_eq!(quickdraw.seed(), expected.seed());
}

#[test]
fn mac_kind_three_loses_xbit_power_and_hands_record_to_kind_six() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-xbit-retirement.sc2"),
        "MacKindThreeXbitRetirement",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y, slot) = (37, 42, XTHG_FIRST_USABLE_SLOT);
    city.set_tile_bits(x, y, TileBits::new(0));
    city.set_text_overlay(x, y, (0xc9 + slot) as u8);
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 3,
        lane: 5,
        state: 4,
        tile_x: x as u8,
        tile_y: y as u8,
        height: 2,
        sub_x: 7,
        sub_y: 11,
        target_x: 0x61,
        target_y: 0x62,
        saved_a: 0x34,
        saved_b: 0x35,
    };

    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;
    expected.random_i16();
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 2);

    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!(record.kind, 6);
    assert_eq!((record.lane, record.state, record.saved_b), (0, 0, 0));
    assert_eq!(
        (
            record.tile_x,
            record.tile_y,
            record.height,
            record.sub_x,
            record.sub_y,
            record.target_x,
            record.target_y,
            record.saved_a,
        ),
        (x as u8, y as u8, 2, 7, 11, 0x61, 0x62, 0x34)
    );
    assert_eq!(city.text_overlay_at(x, y), Some((0xc9 + slot) as u8));
    assert_eq!(city.mac_kind3_actor_counter, 1);
    assert_eq!(quickdraw.seed(), expected.seed());
}

#[test]
fn mac_kind_three_unhandled_phases_only_run_the_common_entry_prelude() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-unhandled-phases.sc2"),
        "MacKindThreeUnhandledPhases",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y, slot) = (37, 42, XTHG_FIRST_USABLE_SLOT);
    city.set_tile_bits(x, y, TileBits::new(0x04));
    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;

    for phase in 5..=u8::MAX {
        let record = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 6,
            state: phase,
            tile_x: x as u8,
            tile_y: y as u8,
            height: 3,
            sub_x: 7,
            sub_y: 11,
            target_x: 0x61,
            target_y: 0x62,
            saved_a: 0x34,
            saved_b: 0x35,
        };
        city.set_mac_object_slot_record(slot, record);
        city.set_text_overlay(x, y, (0xc9 + slot) as u8);

        expected.random_i16();
        tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

        assert_eq!(
            city.traffic_overlays().unwrap()[slot],
            record,
            "phase {phase}"
        );
        assert_eq!(city.text_overlay_at(x, y), Some((0xc9 + slot) as u8));
        assert_eq!(city.mac_kind3_actor_counter, 1);
        assert_eq!(quickdraw.seed(), expected.seed(), "phase {phase}");
    }
}

#[test]
fn mac_kind_six_handoff_advances_at_any_zoom_then_clears() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-six-cleanup.sc2"),
        "MacKindSixCleanup",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y, slot) = (37, 42, XTHG_FIRST_USABLE_SLOT);
    city.set_tile_bits(x, y, TileBits::new(0));
    city.set_text_overlay(x, y, (0xc9 + slot) as u8);
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 3,
        lane: 5,
        state: 4,
        tile_x: x as u8,
        tile_y: y as u8,
        saved_b: 0x35,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };
    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;
    expected.random_i16();

    // The unpowered kind-3 visit performs the handoff at zoom level 0.
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);
    let handoff = city.traffic_overlays().unwrap()[slot];
    assert_eq!(
        (handoff.kind, handoff.lane, handoff.state, handoff.saved_b),
        (6, 0, 0, 0)
    );
    assert_eq!(city.text_overlay_at(x, y), Some((0xc9 + slot) as u8));

    // The threshold gates redraws; kind 6 still advances at zoom 0.
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);
    assert_eq!(city.traffic_overlays().unwrap()[slot].lane, 1);
    assert_eq!(city.text_overlay_at(x, y), Some((0xc9 + slot) as u8));

    // The second visit advances +1 to 2, retaining XTXT.
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);
    assert_eq!(city.traffic_overlays().unwrap()[slot].lane, 2);
    assert_eq!(city.text_overlay_at(x, y), Some((0xc9 + slot) as u8));

    // The next eligible visit frees only the kind byte and clears the proxy.
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);
    let retired = city.traffic_overlays().unwrap()[slot];
    assert_eq!(retired.kind, 0);
    assert_eq!(retired.lane, 2);
    assert_eq!(city.text_overlay_at(x, y), Some(0));
    assert_eq!(quickdraw.seed(), expected.seed());
}

#[test]
fn mac_kind_six_flagged_scan_stamps_empty_dry_candidate_and_clears_mac_route_density() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-six-optional-scan.sc2"),
        "MacKindSixOptionalScan",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET, 0);
    city.set_mac_scenario_active(false);

    // The recovered zero-fixed-point app-LFSR makes each of four probes
    // the same origin-minus-two candidate (row 38, column 46).
    let slot = XTHG_FIRST_USABLE_SLOT;
    let (origin_x, origin_y) = (48, 40);
    let (candidate_x, candidate_y) = (origin_x - 2, origin_y - 2);
    city.set_tile_bits(candidate_x, candidate_y, TileBits::new(0));
    city.set_text_overlay(candidate_x, candidate_y, 0);
    city.set_traffic_density(candidate_x, candidate_y, 19);
    city.set_text_overlay(origin_x, origin_y, (0xc9 + slot) as u8);
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 6,
        lane: 2,
        tile_x: origin_x as u8,
        tile_y: origin_y as u8,
        saved_b: 1,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };

    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let quickdraw_before = quickdraw.seed();
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 1);

    assert_eq!(city.traffic_overlays().unwrap()[slot].kind, 0);
    assert_eq!(city.text_overlay_at(origin_x, origin_y), Some(0));
    assert_eq!(city.text_overlay_at(candidate_x, candidate_y), Some(0xff));
    assert_eq!(city.traffic_density_at(candidate_x, candidate_y), Some(0));
    assert_eq!(quickdraw.seed(), quickdraw_before);
}

#[test]
fn mac_kind_six_no_disasters_gate_skips_optional_scan_but_not_cleanup() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-six-no-disasters-gate.sc2"),
        "MacKindSixNoDisastersGate",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET, 1);
    city.set_mac_scenario_active(false);

    let slot = XTHG_FIRST_USABLE_SLOT;
    let (origin_x, origin_y) = (48, 40);
    let (candidate_x, candidate_y) = (origin_x - 2, origin_y - 2);
    city.set_tile_bits(candidate_x, candidate_y, TileBits::new(0));
    city.set_text_overlay(candidate_x, candidate_y, 0);
    city.set_text_overlay(origin_x, origin_y, (0xc9 + slot) as u8);
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 6,
        lane: 2,
        tile_x: origin_x as u8,
        tile_y: origin_y as u8,
        saved_b: 1,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };

    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 1);

    // +0x0b does not freeze +1/countdown cleanup. No Disasters suppresses
    // only the optional four-candidate scan when no scenario is active.
    assert_eq!(city.traffic_overlays().unwrap()[slot].kind, 0);
    assert_eq!(city.text_overlay_at(origin_x, origin_y), Some(0));
    assert_eq!(city.text_overlay_at(candidate_x, candidate_y), Some(0));
}

#[test]
fn mac_kind_six_flagged_scan_releases_label_before_stamping_candidate() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-six-label-candidate.sc2"),
        "MacKindSixLabelCandidate",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET, 0);
    city.set_mac_scenario_active(false);

    let slot = XTHG_FIRST_USABLE_SLOT;
    let (origin_x, origin_y) = (48, 40);
    let (candidate_x, candidate_y) = (origin_x - 2, origin_y - 2);
    city.set_tile_bits(candidate_x, candidate_y, TileBits::new(0));
    city.set_label(7, "North Shore");
    city.set_text_overlay(candidate_x, candidate_y, 7);
    city.set_traffic_density(candidate_x, candidate_y, 21);
    city.set_text_overlay(origin_x, origin_y, (0xc9 + slot) as u8);
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 6,
        lane: 2,
        tile_x: origin_x as u8,
        tile_y: origin_y as u8,
        saved_b: 1,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };

    let mut quickdraw = MacToolboxRng::new(1);
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 1);

    assert_eq!(city.label_at(7), Some(""));
    assert_eq!(city.text_overlay_at(candidate_x, candidate_y), Some(0xff));
    assert_eq!(city.traffic_density_at(candidate_x, candidate_y), Some(0));
}

#[test]
fn mac_kind_six_flagged_scan_destroys_object_candidate_footprint() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-six-object-candidate.sc2"),
        "MacKindSixObjectCandidate",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(NO_DISASTERS_MISC_OFFSET, 0);
    city.set_mac_scenario_active(false);

    let slot = XTHG_FIRST_USABLE_SLOT;
    let (origin_x, origin_y) = (48, 40);
    let (candidate_x, candidate_y) = (origin_x - 2, origin_y - 2);
    city.set_tile_bits(candidate_x, candidate_y, TileBits::new(0));
    city.set_building(candidate_x, candidate_y, 0x06);
    city.set_text_overlay(candidate_x, candidate_y, 0x3d);
    city.set_microsim_tile_id(0x3d - 0x33, 0xd7);
    city.set_label(0x3d, "North Shore Plaza");
    city.set_traffic_density(candidate_x, candidate_y, 21);
    city.set_text_overlay(origin_x, origin_y, (0xc9 + slot) as u8);
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 6,
        lane: 2,
        tile_x: origin_x as u8,
        tile_y: origin_y as u8,
        saved_b: 1,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };

    let mut quickdraw = MacToolboxRng::new(1);
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 1);

    assert_ne!(city.building_at(candidate_x, candidate_y), 0x06);
    assert_eq!(city.microsim_at(0x3d - 0x33).unwrap().tile_id, 0);
    assert_eq!(city.label_at(0x3d), Some(""));
    assert_eq!(city.text_overlay_at(candidate_x, candidate_y), Some(0xff));
    assert_eq!(city.traffic_density_at(candidate_x, candidate_y), Some(0));
}

#[test]
fn mac_export247_object_candidate_ignites_dry_footprint_and_spends_render_draws() {
    let mut city = blank_city_layout_fixture("mac-object-ignition.city", "Ignition");
    let (x, y) = (40, 41);
    for (column, row) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        city.set_terrain(x + column, y - row, 0);
        city.set_building(x + column, y - row, 0x8c);
        city.set_tile_bits(x + column, y - row, TileBits::new(0));
    }
    city.set_tile_bits(x + 1, y, TileBits::new(4));
    city.set_text_overlay(x, y, 0x33);
    city.set_traffic_density(x, y, 17);
    let mut quickdraw = MacToolboxRng::new(1);
    let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    let mut tick_count = 7;

    assert!(mac_export247_candidate(
        &mut city,
        x,
        y,
        false,
        &mut app_lfsr,
        &mut quickdraw,
        &mut tick_count,
    ));
    assert_eq!(city.text_overlay_at(x, y), Some(0xff));
    assert_eq!(city.text_overlay_at(x + 1, y), Some(0));
    assert_eq!(city.text_overlay_at(x, y - 1), Some(0xff));
    assert_eq!(city.text_overlay_at(x + 1, y - 1), Some(0xff));
    assert_eq!(city.traffic_density_at(x, y), Some(0));
    let mut expected_rng = MacToolboxRng::new(1);
    // Two render draws in each of 2³ visits, then one rubble draw per tile.
    for _ in 0..20 {
        expected_rng.random_u16();
    }
    assert_eq!(quickdraw.seed(), expected_rng.seed());
    assert_eq!(tick_count, 31);
}

#[test]
fn mac_export80_aligns_rotated_two_by_two_before_removal_on_both_layouts() {
    let footprint = [(40, 41, 3), (40, 40, 0), (41, 41, 2), (41, 40, 1)];
    for path in ["mac-fire-anchor.city", "mac-fire-anchor.sc2"] {
        for rotation in 0..4 {
            let mut city = blank_city_layout_fixture(path, "Fire anchor");
            city.saved_rotation = rotation;
            city.set_view_rotation((rotation + 1) & 3);
            for (x, y, corner) in footprint {
                city.set_terrain(x, y, 0);
                city.set_building(x, y, 0x8c);
                city.set_zone(x, y, (0x10 << ((rotation + corner) & 3)) | 2);
            }
            // The southeast hit lies off the export-81 anchor.
            // The adjacent building is where the old unaligned walk extended.
            city.set_building(42, 40, 0x70);
            let mut rng = MacToolboxRng::new(1);
            assert_eq!(
                super::super::budget::mac_destroy_building_export80(
                    &mut city, 41, 40, &mut rng, false,
                ),
                0
            );
            let mut expected = MacToolboxRng::new(1);
            for (x, y, _) in footprint {
                assert_eq!(
                    city.building_at(x, y),
                    (expected.random_u16() as u8 & 3) + 1,
                    "layout {path}, rotation {rotation}, cell ({x},{y})"
                );
                assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 2);
            }
            assert_eq!(rng.seed(), expected.seed());
            assert_eq!(city.building_at(42, 40), 0x70);
        }
    }
}

#[test]
fn mac_hollywood_slot_and_status_follow_native_monster_constructor() {
    for (seed, status, draws) in [(1, 0, 3), (5, 3, 4)] {
        let mut city =
            blank_city_layout_fixture("mac-hollywood-constructor.city", "Monster fixture");
        let mut rng = MacToolboxRng::new(seed);
        city.set_text_overlay(80, 52, 0xca);
        city.set_mac_object_slot_record(
            1,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 8,
                tile_x: 80,
                tile_y: 52,
                saved_a: 0x20,
                ..Default::default()
            },
        );
        city.set_mac_pending_action(8);
        assert!(scenario_action::seed_monster(&mut city, (80, 52), &mut rng));
        assert_eq!(city.mac_pending_action(), 0);
        let actors = city.traffic_overlays().unwrap();
        assert_eq!(actors[1].kind, 0);
        assert_eq!(actors[2].kind, 5);
        assert_eq!(actors[2].saved_a, 0);
        assert_eq!(actors[2].saved_b, status);
        assert_eq!(city.text_overlay_at(80, 52), Some(0xcb));
        let mut expected = MacToolboxRng::new(seed);
        for _ in 0..draws {
            expected.random_u16();
        }
        assert_eq!(rng.seed(), expected.seed());
        city.set_mac_pending_action(8);
        assert!(!scenario_action::seed_monster(
            &mut city,
            (80, 52),
            &mut rng
        ));
        assert_eq!(city.mac_pending_action(), 0);
        assert_eq!(rng.seed(), expected.seed());
    }
}

#[test]
fn mac_oakland_resumes_saved_action_then_releases_calendar() {
    for extension in ["city", "sc2"] {
        let path = std::env::temp_dir().join(format!(
            "citygame-oakland-mode-{}.{extension}",
            std::process::id()
        ));
        let mut city = blank_city_layout_fixture(&path, "Oakland fixture");
        city.prepare_mac_fidelity_from_saved_city();
        city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        city.set_mac_pending_action(12);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_loop_scheduler.load_scenario_point(12, (36, 48));
        state.city.save(&path).unwrap();
        state.load_city(&path, (1280, 800), 1.0).unwrap();
        assert_eq!(state.city.mac_pending_action(), 12);
        assert_eq!(state.mac_loop_scheduler.pending_action_point, None);
        assert_eq!(state.mac_loop_scheduler.disaster_point, (36, 48));
        state.traffic.set_speed(GameSpeed::Paused);
        state.mac_process_rng = MacToolboxRng::new(0);
        let clock = state.game_clock;

        assert_eq!(state.advance_mac_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(state.city.mac_pending_action(), 0);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.game_clock, clock);
        assert_eq!(state.mac_loop_scheduler.dispatch_counter, 0);
        let mut fire_count = 0;
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                if state.city.text_overlay_at(x, y) == Some(0xff) {
                    state.city.set_tile_bits(x, y, TileBits::new(4));
                    fire_count += 1;
                }
            }
        }
        assert_eq!(fire_count, 65);
        assert_eq!(state.mac_loop_scheduler.disaster_point, (39, 52));
        // A service proxy must survive reopen and be recalled on completion.
        state.city.set_text_overlay(0, 0, 0xca);
        state.city.set_mac_object_slot_record(
            1,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 8,
                ..Default::default()
            },
        );
        state.city.save(&path).unwrap();
        state.load_city(&path, (1280, 800), 1.0).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(2)
        );
        assert_eq!(state.city.traffic_overlays().unwrap()[1].kind, 8);

        let clock = state.game_clock;
        state.traffic.set_speed(GameSpeed::Cheetah);
        state.mac_loop_scheduler.actor_deadline = u32::MAX;
        let frame = Duration::from_secs(1);
        assert_eq!(state.advance_mac_frame_simulation(frame).days, 0);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        state.city.set_mac_object_slot_record(
            39,
            crate::win95_overlay::Win95OverlayRecord {
                kind: 6,
                ..Default::default()
            },
        );
        assert_eq!(state.advance_mac_frame_simulation(frame).days, 0);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        state
            .city
            .set_mac_object_slot_record(39, Default::default());
        assert_eq!(state.advance_mac_frame_simulation(frame).days, 0);
        assert!(!scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.city.text_overlay_at(0, 0), Some(0));
        assert_eq!(state.city.traffic_overlays().unwrap()[1].kind, 0);
        assert_eq!(state.game_clock, clock);
        assert_eq!(state.advance_mac_frame_simulation(frame).days, 1);
    }
}

#[test]
fn mac_oakland_selector_five_matches_seeded_native_damage_and_actor() {
    let mut city = blank_city_layout_fixture("mac-oakland-selector-five.city", "Burning building");
    let hit = (45, 40);
    let footprint = [(44, 41, 3), (44, 40, 0), (45, 41, 2), (45, 40, 1)];
    for (x, y, corner) in footprint {
        city.set_terrain(x, y, 0);
        city.set_building(x, y, 0x8c);
        city.set_zone(x, y, (0x10 << corner) | 2);
        city.set_tile_bits(x, y, TileBits::new(0));
    }
    city.set_text_overlay(hit.0, hit.1, 0xff);
    city.set_mac_state_misc_u32_at(0x1044, 12);
    city.set_mac_state_misc_u32_at(0x1048, 0xff);
    let mut rng = MacToolboxRng::new(12);
    assert!(!scenario_action::advance_action(&mut city, &mut rng, &mut 0, &mut 0).complete);
    assert_eq!(rng.seed(), 314_341_360);
    assert_eq!(city.misc_u32_at(0x1048), Some(0xff));
    for ((x, y, _), rubble) in footprint.into_iter().zip([2, 2, 3, 1]) {
        assert_eq!(city.building_at(x, y), rubble);
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 2);
        assert_eq!(
            city.text_overlay_at(x, y),
            Some(if (x, y) == hit { 0xca } else { 0xff })
        );
    }
    assert_eq!(
        city.traffic_overlays().unwrap()[1].to_bytes(),
        [6, 0, 0, 45, 40, 0, 8, 8, 0, 0, 0, 1],
    );
}

#[test]
#[ignore = "requires the local Oakland data fork and CODE 10 replay checkpoint"]
fn mac_oakland_spiral_matches_native_start_planes() {
    let path = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_OAKLAND").unwrap());
    let native =
        std::fs::read(std::env::var_os("CITYGAME_MAC_OAKLAND_CHECKPOINT").unwrap()).unwrap();
    let start = crate::mac_scenario::MacScenario::read(&path).unwrap().start;
    assert_eq!(start.action, 12);
    let mut city = City::load(path).unwrap();
    // Seed the original Mac worker's linear row order. The shared CityGame
    // 64x64 codec currently gives the saved XTRF plane a different order.
    let encoded = &city
        .chunks
        .iter()
        .find(|chunk| chunk.name == "XTRF")
        .unwrap()
        .data;
    let saved_xtrf = crate::city::sc2_rle_decode(encoded).unwrap();
    assert_eq!(saved_xtrf.len(), 64 * 64);
    let mut native_density = Vec::with_capacity(128 * 128);
    for x in 0..128 {
        for y in 0..128 {
            native_density.push(saved_xtrf[(x / 2) * 64 + y / 2]);
        }
    }
    city.sync_traffic_from_slice(&native_density);
    let mut quickdraw = MacToolboxRng::new(1);
    let mut tick_count = 0;
    let (x, y) = start.point;
    let mut point = (i16::from(x), i16::from(y));
    let accepted =
        scenario_action::seed_oakland(&mut city, &mut point, &mut quickdraw, &mut tick_count);
    assert_eq!(accepted, 65);
    assert_eq!(point, (39, 52));
    assert_eq!(quickdraw.seed(), 1);
    assert_eq!(tick_count, 0);
    let mut result = Vec::with_capacity(native.len());
    for read_tile in [
        (|city: &City, x, y| city.text_overlay_at(x, y).unwrap()) as fn(&City, usize, usize) -> u8,
        City::building_at,
        |city, x, y| city.tile_bits_at(x, y).unwrap().raw_bits(),
    ] {
        for x in 0..128 {
            for y in 0..128 {
                result.push(read_tile(&city, x, y));
            }
        }
    }
    for x in (0..128).step_by(2) {
        for y in (0..128).step_by(2) {
            result.push(city.traffic_density_at(x, y).unwrap());
        }
    }
    assert_eq!(result.len(), native.len());
    for (name, offset, side) in [
        ("XTXT", 0, 128),
        ("XBLD", 16_384, 128),
        ("XBIT", 32_768, 128),
        ("XTRF", 49_152, 64),
    ] {
        let end = offset + side * side;
        let differences: Vec<_> = result[offset..end]
            .iter()
            .zip(&native[offset..end])
            .enumerate()
            .filter(|(_, (rust, mac))| rust != mac)
            .collect();
        assert!(
            differences.is_empty(),
            "Oakland {name}: {:?}",
            &differences[..differences.len().min(8)]
        );
    }
}

#[test]
#[ignore = "requires locally owned Win95 Oakland scenario and Mac replay checkpoint"]
fn win95_oakland_spiral_matches_mac_opening_map() {
    let path = std::env::var_os("CITYGAME_WIN95_OAKLAND").unwrap();
    let native =
        std::fs::read(std::env::var_os("CITYGAME_MAC_OAKLAND_CHECKPOINT").unwrap()).unwrap();
    let city = City::load(path).unwrap();
    let goals = scenario_goals_for_city(&city).unwrap();
    assert_eq!(goals.disaster_type, 12);
    assert_eq!((goals.disaster_x, goals.disaster_y), (36, 48));
    let mut state = test_state(city);
    state.auto_goto_enabled = true;
    assert_eq!(state.seed_pe_firestorm_spiral(36, 48), 65);
    assert_eq!(state.take_pending_disaster_auto_goto(), Some((39, 52)));
    for x in 0..128 {
        for y in 0..128 {
            let index = x * 128 + y;
            assert_eq!(state.city.text_overlay_at(x, y), Some(native[index]));
            assert_eq!(state.city.building_at(x, y), native[16_384 + index]);
            assert_eq!(
                state.city.tile_bits_at(x, y).unwrap().raw_bits(),
                native[32_768 + index]
            );
        }
    }
}

#[test]
fn mac_kind_six_export247_object_class_preserves_cardinal_portal_tiles() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-six-portal-object-candidate.sc2"),
        "MacKindSixPortalObjectCandidate",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y) = (46, 38);
    city.set_tile_bits(x, y, TileBits::new(0));
    city.set_building(x, y, 0x3f);
    city.set_text_overlay(x, y, 0x33);
    city.set_traffic_density(x, y, 17);

    let mut quickdraw = MacToolboxRng::new(1);
    let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    assert!(mac_export247_candidate(
        &mut city,
        x,
        y,
        false,
        &mut app_lfsr,
        &mut quickdraw,
        &mut 0
    ));

    assert_eq!(city.building_at(x, y), 0x3f);
    assert_eq!(city.text_overlay_at(x, y), Some(0xff));
    assert_eq!(city.traffic_density_at(x, y), Some(0));
}

#[test]
fn mac_kind_six_rubble_proxy_uses_app_lfsr_and_preserves_xtxt_and_density() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-six-rubble-proxy.sc2"),
        "MacKindSixRubbleProxy",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y) = (46, 38);
    city.set_tile_bits(x, y, TileBits::new(0));
    city.set_building(x, y, 0x06);
    city.set_text_overlay(x, y, 0xf1);
    city.set_traffic_density(x, y, 23);

    let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
    let mut quickdraw = MacToolboxRng::new(1);
    assert!(!mac_export247_candidate(
        &mut city,
        x,
        y,
        false,
        &mut app_lfsr,
        &mut quickdraw,
        &mut 0
    ));

    assert_eq!(city.building_at(x, y), 1);
    assert_eq!(city.text_overlay_at(x, y), Some(0xf1));
    assert_eq!(city.traffic_density_at(x, y), Some(23));
}

#[test]
fn mac_export247_option_zero_requires_developed_xbld_on_both_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-export247-option-zero-gate.sc2"),
            "MacExport247OptionZeroGate",
        )
        .expect("blank city");
        city.kind = kind;
        let (x, y) = (46, 38);
        city.set_tile_bits(x, y, TileBits::new(0));
        city.set_building(x, y, 5);
        city.set_text_overlay(x, y, 0);
        city.set_traffic_density(x, y, 19);

        let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
        let mut quickdraw = MacToolboxRng::new(1);
        assert!(!mac_export247_candidate(
            &mut city,
            x,
            y,
            true,
            &mut app_lfsr,
            &mut quickdraw,
            &mut 0
        ));
        assert_eq!(city.text_overlay_at(x, y), Some(0));
        assert_eq!(city.traffic_density_at(x, y), Some(19));

        city.set_building(x, y, 6);
        assert!(mac_export247_candidate(
            &mut city,
            x,
            y,
            true,
            &mut app_lfsr,
            &mut quickdraw,
            &mut 0
        ));
        assert_eq!(city.text_overlay_at(x, y), Some(0xff));
        assert_eq!(city.traffic_density_at(x, y), Some(0));
    }
}

#[test]
fn mac_kind_six_fa_proxy_decrements_class_counter_then_stamps_candidate() {
    for (building, counter_offset, untouched_offset) in
        [(0x1d, 0x1050, 0x1024), (0x06, 0x1024, 0x1050)]
    {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind-six-fa-proxy.sc2"),
            "MacKindSixFaProxy",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        let (x, y) = (46, 38);
        city.set_tile_bits(x, y, TileBits::new(0));
        city.set_building(x, y, building);
        city.set_text_overlay(x, y, 0xfa);
        city.set_traffic_density(x, y, 23);
        city.set_mac_state_misc_u32_at(counter_offset, 7);
        city.set_mac_state_misc_u32_at(untouched_offset, 11);

        let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
        let mut quickdraw = MacToolboxRng::new(1);
        assert!(mac_export247_candidate(
            &mut city,
            x,
            y,
            false,
            &mut app_lfsr,
            &mut quickdraw,
            &mut 0
        ));

        assert_eq!(city.mac_state_misc_u32_at(counter_offset), Some(6));
        assert_eq!(city.mac_state_misc_u32_at(untouched_offset), Some(11));
        assert_eq!(city.building_at(x, y), building);
        assert_eq!(city.text_overlay_at(x, y), Some(0xff));
        assert_eq!(city.traffic_density_at(x, y), Some(0));
    }
}

#[test]
fn mac_kind_six_export247_rejects_actor_and_reserved_xtxt_classes_unchanged() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-six-ignored-xtxt.sc2"),
        "MacKindSixIgnoredXtxt",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y) = (46, 38);
    city.set_tile_bits(x, y, TileBits::new(0));
    city.set_building(x, y, 0x06);
    for xtxt in (0xc9..=0xf0).chain(0xfb..=0xff) {
        city.set_text_overlay(x, y, xtxt);
        city.set_traffic_density(x, y, 23);

        let mut app_lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
        let mut quickdraw = MacToolboxRng::new(1);
        let app_lfsr_before = app_lfsr.state();
        let quickdraw_before = quickdraw.seed();
        assert!(!mac_export247_candidate(
            &mut city,
            x,
            y,
            false,
            &mut app_lfsr,
            &mut quickdraw,
            &mut 0
        ));

        assert_eq!(city.building_at(x, y), 0x06);
        assert_eq!(city.text_overlay_at(x, y), Some(xtxt));
        assert_eq!(city.traffic_density_at(x, y), Some(23));
        assert_eq!(app_lfsr.state(), app_lfsr_before);
        assert_eq!(quickdraw.seed(), quickdraw_before);
    }
}

#[test]
fn mac_kind_three_phase_one_turns_one_octant_before_candidate_check() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-phase-one-turn.sc2"),
        "MacKindThreePhaseOneTurn",
    )
    .expect("blank city");
    let slot = XTHG_FIRST_USABLE_SLOT;
    city.set_tile_bits(20, 20, TileBits::new(0x04));
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 3,
        lane: 0,
        state: 1,
        tile_x: 20,
        tile_y: 20,
        target_x: 10,
        target_y: 20,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };
    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;
    expected.random_i16();

    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!((record.lane, record.state), (7, 1));
    assert_eq!(quickdraw.seed(), expected.seed());
}

#[test]
fn mac_kind_three_phase_one_aligned_heading_advances_clockwise() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind-three-phase-one-aligned.sc2"),
            "MacKindThreePhaseOneAligned",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_tile_bits(37, 42, TileBits::new(0x04));
        city.set_tile_bits(37, 45, TileBits::new(0x04));
        city.set_terrain(37, 45, 0x10);
        city.set_building(37, 45, 0x51);
        city.set_text_overlay(37, 45, 0);
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 4,
            state: 1,
            tile_x: 37,
            tile_y: 42,
            target_x: 37,
            target_y: 46,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };
        let mut quickdraw = MacToolboxRng::new(1);
        let mut expected = quickdraw;
        expected.random_i16();

        tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

        let record = city.traffic_overlays().unwrap()[slot];
        assert_eq!(
            record.to_bytes(),
            [
                0x03, 0x05, 0x01, 0x25, 0x2a, 0x00, 0x00, 0x00, 0x25, 0x2e, 0, 0
            ]
        );
        assert_eq!(quickdraw.seed(), expected.seed());
    }
}

#[test]
fn mac_kind_three_phase_one_accepts_known_surface_and_rejects_other_xbld() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-phase-one-candidates.sc2"),
        "MacKindThreePhaseOneCandidates",
    )
    .expect("blank city");
    let accepted_slot = XTHG_FIRST_USABLE_SLOT;
    let rejected_slot = accepted_slot + 1;
    for (x, y, building) in [(20, 23, 0x51), (30, 33, 0x53)] {
        city.set_tile_bits(x, y, TileBits::new(0x04));
        city.set_terrain(x, y, 0x10);
        city.set_building(x, y, building);
        city.set_text_overlay(x, y, 0);
    }
    for (slot, x, y) in [(accepted_slot, 20, 20), (rejected_slot, 30, 30)] {
        city.set_tile_bits(x, y, TileBits::new(0x04));
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 3,
            state: 1,
            tile_x: x as u8,
            tile_y: y as u8,
            target_x: x as u8,
            target_y: (y + 4) as u8,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };
    }
    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;
    expected.random_i16();
    expected.random_i16();

    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

    let records = city.traffic_overlays().unwrap();
    assert_eq!(
        (records[accepted_slot].lane, records[accepted_slot].state),
        (4, 0)
    );
    assert_eq!(
        (records[rejected_slot].lane, records[rejected_slot].state),
        (4, 2)
    );
    assert_eq!(quickdraw.seed(), expected.seed());
}

#[test]
fn mac_kind_three_phase_zero_moves_and_checks_original_cardinal_neighbors() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-phase-zero-move.sc2"),
        "MacKindThreePhaseZeroMove",
    )
    .expect("blank city");
    let slot = XTHG_FIRST_USABLE_SLOT;
    city.set_tile_bits(20, 20, TileBits::new(0x04));
    city.set_tile_bits(20, 17, TileBits::new(0x04));
    city.set_terrain(20, 17, 0x10);
    city.set_text_overlay(20, 20, 0xc9 + slot as u8);
    city.set_text_overlay(20, 19, 0x47);
    // The route actor crosses one tile west, but phase 0's hazard census
    // still uses the original X/Y locals loaded before movement.
    city.set_building(22, 20, 0xdf);
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 3,
        lane: 0,
        state: 0,
        tile_x: 20,
        tile_y: 20,
        sub_y: 2,
        target_x: 20,
        target_y: 16,
        saved_a: 0x12,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };
    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;
    expected.random_i16();

    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!((record.kind, record.lane, record.state), (3, 0, 3));
    assert_eq!((record.tile_x, record.tile_y, record.sub_y), (20, 19, 14));
    assert_eq!(record.saved_a, 0x47);
    assert_eq!(city.text_overlay_at(20, 20), Some(0x12));
    assert_eq!(city.text_overlay_at(20, 19), Some(0xc9 + slot as u8));
    assert_eq!(quickdraw.seed(), expected.seed());
}

#[test]
fn mac_kind_three_reads_power_and_accepts_route_at_record_xy_directly() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind-three-direct-xy.sc2"),
            "MacKindThreeDirectXY",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        let (x, y) = (37, 42);
        // Only the native +3/+4 cell and direct-coordinate route candidate
        // are powered; the transposed origin is deliberately unpowered. The
        // candidate's XTER 0x10 and empty XBLD pass the native surface gate.
        city.set_tile_bits(x, y, TileBits::new(0x04));
        city.set_tile_bits(x, y + 3, TileBits::new(0x04));
        city.set_terrain(x, y + 3, 0x10);
        city.set_text_overlay(x, y, 0xc9 + slot as u8);
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 4,
            state: 0,
            tile_x: x as u8,
            tile_y: y as u8,
            target_x: x as u8,
            target_y: (y + 8) as u8,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };

        let mut quickdraw = MacToolboxRng::new(1);
        let mut expected = quickdraw;
        expected.random_i16();
        tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

        let record = city.traffic_overlays().unwrap()[slot];
        assert_eq!(record.kind, 3, "direct +3/+4 XBIT lookup remains powered");
        assert_eq!(
            record.to_bytes(),
            [
                0x03, 0x04, 0x00, 0x25, 0x2a, 0x00, 0x00, 0x04, 0x25, 0x32, 0, 0
            ]
        );
        assert_eq!(
            (record.tile_x, record.tile_y, record.state, record.sub_y),
            (x as u8, y as u8, 0, 4)
        );
        assert_eq!(quickdraw.seed(), expected.seed());
    }
}

#[test]
fn mac_kind_three_phase_zero_enters_phase_one_when_current_route_is_blocked() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-phase-zero-blocked.sc2"),
        "MacKindThreePhaseZeroBlocked",
    )
    .expect("blank city");
    let slot = XTHG_FIRST_USABLE_SLOT;
    city.set_tile_bits(20, 20, TileBits::new(0x04));
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 3,
        lane: 3,
        state: 0,
        tile_x: 20,
        tile_y: 20,
        target_x: 20,
        target_y: 24,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };
    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;
    expected.random_i16();

    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!((record.kind, record.lane, record.state), (3, 3, 1));
    assert_eq!(quickdraw.seed(), expected.seed());
}

#[test]
fn mac_kind_three_subtile_movement_restores_underlay_before_edge_cleanup() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-subtile-edge.sc2"),
        "MacKindThreeSubtileEdge",
    )
    .expect("blank city");
    let slot = XTHG_FIRST_USABLE_SLOT;
    let mut record = crate::win95_overlay::Win95OverlayRecord {
        kind: 3,
        lane: 4,
        tile_x: 40,
        tile_y: 127,
        sub_y: 14,
        saved_a: 0x18,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };
    city.set_text_overlay(40, 127, 0xc9 + slot as u8);

    mac_kind3_move_one_step(&mut city, slot, &mut record);

    assert_eq!(
        (record.kind, record.tile_x, record.tile_y, record.sub_y),
        (0, 40, 127, 2)
    );
    assert_eq!(city.text_overlay_at(40, 127), Some(0));
}

#[test]
fn mac_kind_three_phase_two_uses_zero_lfsr_window_and_stores_first_route() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind-three-phase-two-accept.sc2"),
            "MacKindThreePhaseTwoAccept",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_tile_bits(37, 42, TileBits::new(0x04));
        city.set_tile_bits(37, 45, TileBits::new(0x04));
        city.set_terrain(37, 45, 0x10);
        city.set_building(37, 45, 0x51);
        city.set_text_overlay(37, 45, 0);
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 4,
            state: 2,
            tile_x: 37,
            tile_y: 42,
            target_x: 37,
            target_y: 50,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };
        let mut quickdraw = MacToolboxRng::new(1);
        let mut expected = quickdraw;
        expected.random_i16();

        tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

        let record = city.traffic_overlays().unwrap()[slot];
        assert_eq!(
            record.to_bytes(),
            [
                0x03, 0x04, 0x00, 0x25, 0x2a, 0x00, 0x00, 0x00, 0x25, 0x32, 0, 0
            ]
        );
        assert_eq!(quickdraw.seed(), expected.seed());
    }
}

#[test]
fn mac_kind_three_phase_two_failed_search_keeps_last_window_candidate() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind-three-phase-two-exhausted.sc2"),
            "MacKindThreePhaseTwoExhausted",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_tile_bits(37, 42, TileBits::new(0x04));
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 3,
            state: 2,
            tile_x: 37,
            tile_y: 42,
            target_x: 37,
            target_y: 50,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };
        let mut quickdraw = MacToolboxRng::new(1);
        let mut expected = quickdraw;
        expected.random_i16();

        tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

        let record = city.traffic_overlays().unwrap()[slot];
        assert_eq!(
            record.to_bytes(),
            [
                0x03, 0x05, 0x00, 0x25, 0x2a, 0x00, 0x00, 0x00, 0x25, 0x32, 0, 0
            ]
        );
        assert_eq!(quickdraw.seed(), expected.seed());
    }
}

#[test]
fn mac_kind_three_phase_three_loads_last_creator_boundary_and_enters_four() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind-three-phase-three-boundary.sc2"),
            "MacKindThreePhaseThreeBoundary",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_tile_bits(37, 42, TileBits::new(0x04));
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 4,
            state: 3,
            tile_x: 37,
            tile_y: 42,
            target_x: 37,
            target_y: 50,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };
        let mut quickdraw = MacToolboxRng::new(1);
        let mut expected = quickdraw;
        expected.random_i16();

        tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

        let record = city.traffic_overlays().unwrap()[slot];
        assert_eq!(
            record.to_bytes(),
            [0x03, 0x04, 0x04, 0x25, 0x2a, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(quickdraw.seed(), expected.seed());
    }
}

#[test]
fn mac_kind_three_phase_four_moves_when_current_heading_is_open() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind-three-phase-four-move.sc2"),
            "MacKindThreePhaseFourMove",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_tile_bits(37, 42, TileBits::new(0x04));
        city.set_tile_bits(37, 45, TileBits::new(0x04));
        city.set_terrain(37, 45, 0x10);
        city.set_building(37, 45, 0x51);
        city.set_text_overlay(37, 42, 0xc9 + slot as u8);
        city.set_text_overlay(37, 43, 0x32);
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 4,
            state: 4,
            tile_x: 37,
            tile_y: 42,
            sub_y: 14,
            saved_a: 0x19,
            target_x: 2,
            target_y: 2,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };
        let mut quickdraw = MacToolboxRng::new(1);
        let mut expected = quickdraw;
        expected.random_i16();

        tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

        let record = city.traffic_overlays().unwrap()[slot];
        assert_eq!(
            record.to_bytes(),
            [0x03, 0x04, 0x04, 0x25, 0x2b, 0, 0, 2, 2, 2, 0x32, 0]
        );
        assert_eq!(city.text_overlay_at(37, 42), Some(0x19));
        assert_eq!(city.text_overlay_at(37, 43), Some(0xc9 + slot as u8));
        assert_eq!(quickdraw.seed(), expected.seed());
    }
}

#[test]
fn mac_kind_three_phase_four_searches_forward_and_keeps_phase() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-phase-four-search.sc2"),
        "MacKindThreePhaseFourSearch",
    )
    .expect("blank city");
    let slot = XTHG_FIRST_USABLE_SLOT;
    city.set_tile_bits(20, 20, TileBits::new(0x04));
    // Direction zero is blocked by an unsupported building. Direction one
    // is the first viable entry after the zero LFSR's start index.
    city.set_tile_bits(20, 17, TileBits::new(0x04));
    city.set_terrain(20, 17, 0x10);
    city.set_building(20, 17, 0x53);
    city.set_tile_bits(23, 17, TileBits::new(0x04));
    city.set_terrain(23, 17, 0x10);
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 3,
        lane: 0,
        state: 4,
        tile_x: 20,
        tile_y: 20,
        target_x: 2,
        target_y: 2,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };
    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;
    expected.random_i16();

    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!((record.state, record.lane), (4, 1));
    assert_eq!((record.tile_x, record.tile_y), (20, 20));
    assert_eq!(quickdraw.seed(), expected.seed());
}

#[test]
fn mac_kind_three_phase_four_failed_search_stores_last_direction() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-kind-three-phase-four-exhausted.sc2"),
            "MacKindThreePhaseFourExhausted",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_tile_bits(37, 42, TileBits::new(0x04));
        city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
            kind: 3,
            lane: 3,
            state: 4,
            tile_x: 37,
            tile_y: 42,
            target_x: 2,
            target_y: 2,
            ..crate::win95_overlay::Win95OverlayRecord::default()
        };
        let mut quickdraw = MacToolboxRng::new(1);
        let mut expected = quickdraw;
        expected.random_i16();

        tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 0);

        let record = city.traffic_overlays().unwrap()[slot];
        assert_eq!(
            record.to_bytes(),
            [0x03, 0x03, 0x04, 0x25, 0x2a, 0, 0, 0, 2, 2, 0, 0]
        );
        assert_eq!(quickdraw.seed(), expected.seed());
    }
}

#[test]
fn mac_kind_three_with_xbit_power_stays_kind_three() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/mac-kind-three-xbit-powered.sc2"),
        "MacKindThreeXbitPowered",
    )
    .expect("blank city");
    city.kind = CityFileKind::MacCity;
    let (x, y, slot) = (37, 42, XTHG_FIRST_USABLE_SLOT);
    city.set_tile_bits(x, y, TileBits::new(0x04));
    city.mac_object_slot_records_mut()[slot] = crate::win95_overlay::Win95OverlayRecord {
        kind: 3,
        tile_x: x as u8,
        tile_y: y as u8,
        ..crate::win95_overlay::Win95OverlayRecord::default()
    };

    let mut quickdraw = MacToolboxRng::new(0x1234_5678);
    let mut expected = quickdraw;
    expected.random_i16();
    tick_mac_marina_actors_and_count_kind3(&mut city, &mut quickdraw, 2);

    assert_eq!(city.traffic_overlays().unwrap()[slot].kind, 3);
    assert_eq!(city.mac_kind3_actor_counter, 1);
    assert_eq!(quickdraw.seed(), expected.seed());
}

#[test]
#[ignore = "requires local Charleston and original CODE 10 checkpoints"]
fn mac_charleston_matches_native_opening_and_first_pass() {
    let source =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_SCENARIO_FIXTURES").unwrap())
            .join("Charleston");
    let checkpoints =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_CHARLESTON_CHECKPOINTS").unwrap());
    let mut city = City::load(&source).unwrap();
    assert_eq!(city.saved_rotation(), 3);
    city.initialize_mac_scenario(
        crate::mac_scenario::MacScenario::read(&source)
            .unwrap()
            .start,
    );

    // Compare the Mac simulation plane in native row order. The shared
    // saved XTRF codec's current layout differs from the Mac row table.
    let encoded = &city
        .chunks
        .iter()
        .find(|chunk| chunk.name == "XTRF")
        .unwrap()
        .data;
    let saved_xtrf = crate::city::sc2_rle_decode(encoded).unwrap();
    let mut density = Vec::with_capacity(128 * 128);
    for x in 0..128 {
        for y in 0..128 {
            density.push(saved_xtrf[(x / 2) * 64 + y / 2]);
        }
    }
    city.sync_traffic_from_slice(&density);

    let compare = |city: &City, rotation: u8, checkpoint: &str| {
        let bytes =
            std::fs::read(checkpoints.join(format!("rotation-{rotation}-{checkpoint}.bin")))
                .unwrap();
        let mut base = 0;
        let mut mismatches = Vec::new();
        for (name, side, read) in [
            (
                "XTXT",
                128,
                (|city: &City, x, y| city.text_overlay_at(x, y).unwrap())
                    as fn(&City, usize, usize) -> u8,
            ),
            ("XBLD", 128, City::building_at),
            ("XBIT", 128, |city, x, y| {
                city.tile_bits_at(x, y).unwrap().raw_bits()
            }),
            ("XTRF", 64, |city, x, y| {
                city.traffic_density_at(2 * x, 2 * y).unwrap()
            }),
            ("XZON", 128, |city, x, y| {
                city.zone_at(x, y).unwrap().raw_bits()
            }),
        ] {
            let mut count = 0;
            let mut first = Vec::new();
            for x in 0..side {
                for y in 0..side {
                    let expected = bytes[base + x * side + y];
                    let actual = read(city, x, y);
                    if expected != actual {
                        count += 1;
                        if first.len() < 8 {
                            first.push((x, y, expected, actual));
                        }
                    }
                }
            }
            if count != 0 {
                mismatches.push((name, count, first));
            }
            base += side * side;
        }
        assert!(
            mismatches.is_empty(),
            "rotation {rotation} {checkpoint} mismatches {mismatches:?}"
        );
    };
    for (rotation, opening, first) in [
        (2, (0xe6a9, 971_307_217), (0xd6a7, 685_428_651)),
        (3, (0x188d, 832_633_821), (0xc468, 1_168_120_094)),
    ] {
        let mut city = city.clone();
        city.saved_rotation = rotation;
        let mut rng = MacToolboxRng::new(1);
        let mut now = 0;
        let mut lfsr = hurricane::start(&mut city, &mut rng, &mut now);
        assert_eq!((lfsr, rng.seed()), opening, "rotation {rotation}");
        compare(&city, rotation, "opening");

        let pass = scenario_action::advance_action(&mut city, &mut rng, &mut now, &mut lfsr);
        assert!(!pass.complete);
        assert_eq!((lfsr, rng.seed()), first, "rotation {rotation}");
        assert_eq!(city.misc_u32_at(hurricane::FLOOD_DURATION), Some(59));
        assert_eq!(city.misc_u32_at(hurricane::WIND_DURATION), Some(49));
        compare(&city, rotation, "first");
    }
}

#[test]
fn mac_hurricane_releases_calendar_on_the_pass_after_expiry() {
    let mut city = blank_city_layout_fixture("mac-hurricane-expiry.city", "Flood");
    city.set_mac_state_misc_low_word_at(hurricane::WIND_DURATION, 49);
    city.set_text_overlay(0, 0, 0xfc);
    scenario_action::begin_action(&mut city, scenario_action::HURRICANE_ACTION);

    // Match the controlled native expiry fixture. Both RNG streams stay zero.
    let mut rng = MacToolboxRng::new(0);
    let mut now = 0;
    let mut lfsr = 0;
    let first = scenario_action::advance_action(&mut city, &mut rng, &mut now, &mut lfsr);
    assert!(!first.complete);
    assert_eq!(city.text_overlay_at(0, 0), Some(0));
    assert_eq!(city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET), Some(2));

    let second = scenario_action::advance_action(&mut city, &mut rng, &mut now, &mut lfsr);
    assert!(second.complete);
    assert_eq!(
        city.mac_state_misc_u32_at(hurricane::WIND_DURATION),
        Some(49)
    );
    assert_eq!(city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET), Some(1));
}

#[test]
fn mac_manual_fire_uses_worker_jitter_and_native_candidate_gate_on_both_layouts() {
    for path in ["manual-fire.city", "manual-fire.sc2"] {
        let city = blank_city_layout_fixture(path, "Fire fixture");
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        state.traffic.set_speed(GameSpeed::Paused);
        state.no_disasters_enabled = true;
        state.mac_process_rng = MacToolboxRng::new(1);
        state.city.set_mac_kind2_hotspot(64, 64);
        state.city.set_building(51, 52, 0x70); // First cell north of the menu point.
        state.city.set_building(53, 77, 0x70); // First cell north of the worker point.
        state.city.set_traffic_density(53, 77, 91);

        let effect = state
            .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Fire))
            .unwrap();
        assert_eq!(effect.disaster_log, Some((DisasterKind::Fire, true)));
        assert!(effect.auto_goto_tile.is_none());
        assert_eq!(state.mac_process_rng.seed(), 282475249);
        assert_eq!(state.city.mac_pending_action(), 1);
        assert_eq!(state.city.text_overlay_at(53, 77), Some(0));

        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(state.mac_process_rng.seed(), 1144108930); // Includes one immediate marker visit.
        assert_eq!(state.city.text_overlay_at(51, 52), Some(0));
        assert_eq!(state.city.text_overlay_at(53, 77), Some(0xff));
        assert_eq!(state.mac_loop_scheduler.disaster_point, (53, 77));
        assert_eq!(state.city.traffic_density_at(53, 77), Some(0));
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.city.pe_w_trigger_disaster, 0);
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(CITY_MODE_DISASTER)
        );

        let saved = std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
        state.city.save(&saved).unwrap();
        let reopened = state.load_city(&saved, (1280, 800), 1.0);
        std::fs::remove_file(saved).unwrap();
        reopened.unwrap();
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.city.text_overlay_at(53, 77), Some(0xff));
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(CITY_MODE_DISASTER)
        );

        state.city.set_text_overlay(53, 77, 0);
        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(100))
                .days,
            0
        );
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(CITY_MODE_GAMEPLAY)
        );
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(100))
                .days,
            1
        );
    }

    for building in [5, 6] {
        let city = blank_city_layout_fixture("manual-fire-fallback.city", "Fire fallback");
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        state.traffic.set_speed(GameSpeed::Paused);
        state.mac_process_rng = MacToolboxRng::new(1);
        state.city.set_building(0, 0, building);
        assert!(state.start_disaster(DisasterKind::Fire));
        let menu_point = state.mac_loop_scheduler.disaster_point;
        state.advance_frame_simulation(Duration::ZERO);
        let expected_point = if building == 6 { (0, 0) } else { menu_point };
        assert_eq!(state.mac_loop_scheduler.disaster_point, expected_point);
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(
            scenario_action::mac_scenario_action_active(&state.city),
            building == 6
        );
        assert_eq!(
            state.city.text_overlay_at(0, 0),
            Some(if building == 6 { 0xff } else { 0 })
        );
        assert_eq!(
            state.mac_process_rng.seed(),
            if building == 6 { 1144108930 } else { 984943658 }
        );
    }
}

#[test]
fn mac_manual_monster_clamps_the_signed_menu_point_and_prevents_overlapping_actions() {
    for path in ["manual-monster.city", "manual-monster.sc2"] {
        let city = blank_city_layout_fixture(path, "Monster fixture");
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        state.traffic.set_speed(GameSpeed::Paused);
        state.no_disasters_enabled = true;
        state.mac_process_rng = MacToolboxRng::new(1);
        state.city.set_mac_kind2_hotspot(3, 126);
        state.hover_tile = Some((42, 42));

        assert!(state.start_disaster(DisasterKind::Monster));
        assert_eq!(
            state.mac_loop_scheduler.pending_action_point,
            Some((-5, 128))
        );
        state.advance_frame_simulation(Duration::ZERO);

        let actor = state.city.traffic_overlays().unwrap()[1];
        assert_eq!(
            (actor.kind, actor.lane, actor.tile_x, actor.tile_y),
            (5, 2, 0, 127)
        );
        assert_eq!((actor.target_x, actor.target_y, actor.saved_b), (89, 42, 2));
        assert_eq!(state.city.text_overlay_at(0, 127), Some(0xca));
        assert_eq!(state.city.text_overlay_at(42, 42), Some(0));
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.mac_process_rng.seed(), 470211272);
        assert!(scenario_action::mac_scenario_action_active(&state.city));

        assert!(!state.start_disaster(DisasterKind::Fire));
        assert_eq!(state.mac_process_rng.seed(), 470211272);
    }
}

#[test]
fn mac_manual_hurricane_dispatches_without_a_scenario_point() {
    let city = blank_city_layout_fixture("manual-hurricane.sc2", "Hurricane fixture");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state
        .city
        .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
    state.traffic.set_speed(GameSpeed::Paused);
    state.mac_process_rng = MacToolboxRng::new(1);
    for y in 0..128 {
        state.city.set_building(64, y, 6);
    }

    assert!(state.start_disaster(DisasterKind::Hurricane));
    assert_eq!(state.mac_process_rng.seed(), 1);
    assert!(state.mac_loop_scheduler.pending_action_point.is_none());
    state.advance_frame_simulation(Duration::ZERO);

    assert_eq!(state.city.mac_pending_action(), 0);
    assert!(scenario_action::mac_scenario_action_active(&state.city));
    assert_eq!(
        state.city.mac_state_misc_u32_at(hurricane::WIND_DURATION),
        Some(49)
    );
    assert_eq!(
        state.city.mac_state_misc_u32_at(hurricane::FLOOD_DURATION),
        Some(59)
    );
}

#[test]
fn mac_manual_flood_opener_matches_shoreline_order_and_bounds_map_edges() {
    let surrounding = [(40, 41), (41, 40), (42, 41), (41, 42)];
    for wet in [&surrounding[2..], &surrounding[..2]] {
        let mut city = blank_city_layout_fixture("manual-flood-shore.city", "Flood shoreline");
        city.set_terrain(41, 41, 0x20);
        city.set_terrain(42, 40, 0x2f);
        for (x, y) in surrounding {
            city.set_traffic_density(x, y, 91);
        }
        for &(x, y) in wet {
            city.set_tile_bits(x, y, TileBits::new(4));
        }
        let mut point = (40, 40);
        assert!(hurricane::start_flood(&mut city, &mut point));
        assert_eq!(point, (41, 41));
        assert_eq!(city.text_overlay_at(41, 41), Some(0));
        for (x, y) in surrounding {
            let marker = if wet == &surrounding[2..] { 0xfc } else { 0 };
            assert_eq!(city.text_overlay_at(x, y), Some(marker));
            assert_eq!(city.traffic_density_at(x, y), Some(91));
        }
        assert_eq!(
            city.mac_state_misc_u32_at(hurricane::FLOOD_DURATION),
            Some(60)
        );
    }

    let mut city = blank_city_layout_fixture("manual-flood-edge.city", "Flood edge");
    city.set_terrain(127, 41, 0x20);
    city.set_tile_bits(127, 40, TileBits::new(4));
    let mut point = (127, 41);
    assert!(hurricane::start_flood(&mut city, &mut point));
    assert_eq!(point, (127, 41));
    assert!(
        (0..CITY_WIDTH).all(|x| (0..CITY_HEIGHT).all(|y| city.text_overlay_at(x, y) == Some(0)))
    );

    // Radius ties select the lower X, then lower Y. A candidate 128 cells
    // from the signed origin lies beyond the original search.
    let mut city = blank_city_layout_fixture("manual-flood-order.city", "Flood ordering");
    city.set_terrain(35, 37, 0x20);
    city.set_terrain(35, 43, 0x20);
    city.set_terrain(36, 35, 0x20);
    let mut point = (40, 40);
    assert!(hurricane::start_flood(&mut city, &mut point));
    assert_eq!(point, (35, 37));
    assert_eq!(city.text_overlay_at(36, 37), Some(0xfc));
    assert_eq!(city.text_overlay_at(36, 43), Some(0));
    assert_eq!(city.text_overlay_at(37, 35), Some(0));

    city.set_terrain(0, 0, 0x10);
    for origin in [(164, 40), (i16::MIN, i16::MIN)] {
        let mut point = origin;
        assert!(!hurricane::start_flood(&mut city, &mut point));
        assert_eq!(point, origin);
    }
}

#[test]
fn mac_manual_flood_runs_fallback_action_and_reopens_in_both_layouts() {
    for path in ["manual-flood.city", "manual-flood.sc2"] {
        let city = blank_city_layout_fixture(path, "Flood fixture");
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        state.traffic.set_speed(GameSpeed::Paused);
        state.no_disasters_enabled = true;
        state.mac_process_rng = MacToolboxRng::new(1);
        state.city.set_mac_kind2_hotspot(64, 64);
        state.hover_tile = Some((12, 34));
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                state.city.set_terrain(x, y, 0x10);
            }
        }
        assert!(state.start_disaster(DisasterKind::Flood));
        assert_eq!(state.city.mac_pending_action(), 2);
        assert_eq!(
            state.mac_loop_scheduler.pending_action_point,
            Some((64, 64))
        );
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.mac_loop_scheduler.disaster_point, (64, 64));
        assert_eq!(state.mac_process_rng.seed(), 1);
        assert!(!scenario_action::mac_scenario_action_active(&state.city));
        assert!(state.take_pending_disaster_extra_sounds().is_empty());

        state.city.set_terrain(0, 0, 0);
        state.city.set_terrain(0, 1, 0);
        state.city.set_tile_bits(0, 0, TileBits::new(4));
        state.city.set_building(0, 0, 6);
        state.city.set_traffic_density(0, 1, 91);
        let effect = state
            .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Flood))
            .unwrap();
        assert_eq!(effect.disaster_log, Some((DisasterKind::Flood, true)));
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(state.city.pe_w_trigger_disaster, 0);
        assert_eq!(state.city.text_overlay_at(0, 0), Some(0xfc));
        assert_eq!(state.mac_loop_scheduler.disaster_point, (0, 0));
        assert_eq!(state.city.text_overlay_at(0, 1), Some(0xfc));
        assert_eq!(state.city.traffic_density_at(0, 1), Some(0));
        assert_eq!(
            state.city.mac_state_misc_u32_at(hurricane::FLOOD_DURATION),
            Some(59)
        );
        assert_eq!(state.mac_process_rng.seed(), 1_622_650_073);
        assert_eq!(
            state.take_pending_disaster_extra_sounds(),
            vec![PlacementSound::Flood]
        );

        let saved = std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
        state.city.save(&saved).unwrap();
        reopen_with_actor_fixture_seed(&mut state, &saved);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(CITY_MODE_DISASTER)
        );
        assert_eq!(
            state.city.mac_state_misc_u32_at(hurricane::FLOOD_DURATION),
            Some(59)
        );
        assert_eq!(state.mac_process_rng.seed(), 1_622_650_073);

        state.traffic.set_speed(GameSpeed::Cheetah);
        let frame = Duration::from_millis(17);
        // Original CODE 10 returns to mode 1 after 56 passes, including
        // the first pass immediately after opening.
        for _ in 1..56 {
            assert!(scenario_action::mac_scenario_action_active(&state.city));
            assert_eq!(state.advance_frame_simulation(frame).days, 0);
        }
        assert!(!scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.city.text_overlay_at(0, 0), Some(0));
        assert_eq!(state.city.text_overlay_at(0, 1), Some(0));
        assert_eq!(state.mac_process_rng.seed(), 281_725_226);
        assert_eq!(
            state.city.mac_state_misc_u32_at(hurricane::FLOOD_DURATION),
            Some(4)
        );
        assert_eq!(state.advance_frame_simulation(frame).days, 1);
    }
}

#[test]
fn mac_manual_tornado_preserves_actor_until_ordinary_sweep_on_both_layouts() {
    let opener = [15, 1, 0, 50, 96, 11, 8, 8, 42, 2, 0x22, 0x5a];
    let first_visit = [15, 1, 0, 51, 95, 11, 8, 8, 42, 2, 0, 0x5a];
    for path in ["manual-tornado.city", "manual-tornado.sc2"] {
        let city = blank_city_layout_fixture(path, "Tornado fixture");
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        state.traffic.set_speed(GameSpeed::Paused);
        state.no_disasters_enabled = true;
        state.mac_process_rng = MacToolboxRng::new(1);
        state.mac_loop_scheduler.actor_deadline = 31;
        state.hover_tile = Some((12, 34));
        state.city.set_altm_word(50, 96, 0x02ab);
        state.city.set_text_overlay(50, 96, 0x22);
        state.city.mac_object_slot_records_mut()[1].saved_b = 0x5a;
        let pe_crt = state.pe_process_crt_seed();
        let pe_lfsr = state.pe_process_lfsr_state();

        let effect = state
            .apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Tornado))
            .unwrap();
        assert_eq!(effect.disaster_log, Some((DisasterKind::Tornado, true)));
        assert_eq!(state.city.mac_pending_action(), 7);
        assert_eq!(
            state.mac_loop_scheduler.pending_action_point,
            Some((50, 96))
        );
        assert_eq!(state.mac_process_rng.seed(), 282_475_249);
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.city.pe_w_trigger_disaster, 0);
        assert_eq!(state.pe_process_crt_seed(), pe_crt);
        assert_eq!(state.pe_process_lfsr_state(), pe_lfsr);
        assert_eq!(state.city.traffic_overlays().unwrap()[1].to_bytes(), opener);
        assert_eq!(state.city.text_overlay_at(50, 96), Some(0xca));
        assert_eq!(state.city.mac_tornado_actor_counter, 1);
        assert_eq!(state.mac_process_rng.seed(), 1_144_108_930);
        assert_eq!(state.mac_loop_scheduler.actor_deadline, 31);

        let saved = std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
        state.city.save(&saved).unwrap();
        reopen_with_actor_fixture_seed(&mut state, &saved);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.city.traffic_overlays().unwrap()[1].to_bytes(), opener);

        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(517))
                .days,
            0
        );
        assert_eq!(state.city.traffic_overlays().unwrap()[1].to_bytes(), opener);
        assert_eq!(state.mac_process_rng.seed(), 1_144_108_930);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            0
        );
        assert_eq!(
            state.city.traffic_overlays().unwrap()[1].to_bytes(),
            first_visit
        );
        assert_eq!(state.city.text_overlay_at(50, 96), Some(0x22));
        assert_eq!(state.city.text_overlay_at(51, 95), Some(0xca));
        assert_eq!(state.mac_process_rng.seed(), 823_564_440);

        let frame = Duration::from_millis(517);
        for _ in 1..122 {
            assert!(scenario_action::mac_scenario_action_active(&state.city));
            assert_eq!(state.advance_frame_simulation(frame).days, 0);
        }
        assert!(!scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(
            state.city.traffic_overlays().unwrap()[1].to_bytes(),
            [0, 1, 0, 127, 16, 11, 8, 8, 42, 2, 0, 0x5a]
        );
        assert_eq!(state.mac_process_rng.seed(), 2_014_885_176);
        assert_eq!(state.city.text_overlay_at(127, 16), Some(0xca));
        assert_eq!(state.advance_frame_simulation(frame).days, 1);
    }
}

#[test]
fn mac_tornado_constructor_checks_counter_and_slot_then_clears_collision() {
    let mut city = blank_city_layout_fixture("manual-tornado-gates.city", "Tornado gates");
    let mut rng = MacToolboxRng::new(282_475_249);
    city.set_altm_word(50, 96, 0x02ab);
    city.set_text_overlay(50, 96, 0xcb);
    city.mac_object_slot_records_mut()[1].saved_b = 0x5a;
    city.set_mac_object_slot_record(
        2,
        crate::win95_overlay::Win95OverlayRecord {
            kind: 8,
            tile_x: 50,
            tile_y: 96,
            saved_a: 0x22,
            ..Default::default()
        },
    );

    city.mac_tornado_actor_counter = 1;
    assert!(!scenario_action::seed_tornado(
        &mut city,
        (50, 96),
        &mut rng
    ));
    assert_eq!(rng.seed(), 282_475_249);
    assert_eq!(city.text_overlay_at(50, 96), Some(0xcb));

    city.mac_tornado_actor_counter = 0;
    let mut full = city.clone();
    for record in &mut full.mac_object_slot_records_mut()[1..] {
        record.kind = 2;
    }
    assert!(!scenario_action::seed_tornado(
        &mut full,
        (50, 96),
        &mut rng
    ));
    assert_eq!(rng.seed(), 282_475_249);
    assert_eq!(full.text_overlay_at(50, 96), Some(0xcb));

    assert!(scenario_action::seed_tornado(&mut city, (50, 96), &mut rng));
    let records = city.traffic_overlays().unwrap();
    assert_eq!(records[2].kind, 0);
    assert_eq!(
        records[1].to_bytes(),
        [15, 1, 0, 50, 96, 11, 8, 8, 42, 2, 0, 0x5a]
    );
    assert_eq!(city.text_overlay_at(50, 96), Some(0xca));
}

#[test]
fn mac_phase24_tornado_replaces_pending_and_starts_in_the_same_frame() {
    for path in ["mac-monthly-tornado.city", "mac-monthly-tornado.sc2"] {
        for (trend, seed, point, terminal_seed, record) in [
            (
                WEATHER_TREND_TORNADO,
                1,
                (96, 24),
                470_211_272,
                [15, 2, 0, 96, 24, 11, 8, 8, 2, 72, 0x22, 0x5a],
            ),
            (
                WEATHER_TREND_WINDY,
                69,
                (20, 15),
                1_807_462_820,
                [15, 7, 0, 20, 15, 11, 8, 8, 59, 36, 0x22, 0x5a],
            ),
        ] {
            let mut state = mac_phase24_fixture(path, trend, seed);
            let (x, y) = point;
            state.city.set_altm_word(x, y, 0x02ab);
            state.city.set_text_overlay(x, y, 0x22);
            state.city.mac_object_slot_records_mut()[1].saved_b = 0x5a;

            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(17))
                    .days,
                1
            );

            assert_eq!(state.game_clock.day(), 24);
            assert_eq!(state.city.mac_pending_action(), 0, "{path}, seed {seed}");
            assert_eq!(state.city.traffic_overlays().unwrap()[1].to_bytes(), record);
            assert_eq!(state.city.text_overlay_at(x, y), Some(0xca));
            assert_eq!(
                state.mac_process_rng.seed(),
                terminal_seed,
                "{path}, seed {seed}"
            );
        }
    }
}

#[test]
fn mac_phase24_miss_preserves_pending_and_disabled_recreation_still_draws() {
    for path in ["mac-monthly-miss.city", "mac-monthly-miss.sc2"] {
        for (trend, seed, disabled, terminal_seed) in [
            (WEATHER_TREND_TORNADO, 3, false, 50_421),
            (WEATHER_TREND_WINDY, 69, true, 1_159_683),
        ] {
            let mut state = mac_phase24_fixture(path, trend, seed);
            state.no_disasters_enabled = disabled;

            state.handle_simulation_day_advanced(1910, 1, 24);

            assert_eq!(state.city.mac_pending_action(), 11);
            assert_eq!(
                state.mac_loop_scheduler.pending_action_point,
                Some((13, 14))
            );
            assert_eq!(
                state.mac_process_rng.seed(),
                terminal_seed,
                "{path}, seed {seed}"
            );
        }
    }
}

#[test]
fn mac_phase24_toxic_spill_matches_native_scan_and_miss_point_write() {
    for path in ["mac-monthly-toxic.city", "mac-monthly-toxic.sc2"] {
        for populated in [false, true] {
            let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 20);
            // A previous action has left a Point without a queued point write.
            state.mac_loop_scheduler.disaster_point = (13, 14);
            state.mac_loop_scheduler.pending_action_point = None;
            if populated {
                let mut pollution = vec![0; CITY_TILES];
                pollution[density_map_index(26, 38)] = 0xb4;
                pollution[density_map_index(38, 26)] = 0xb4;
                state.city.sync_pollution_density_from_slice(&pollution);
            }

            state.handle_simulation_day_advanced(1910, 1, 24);

            let (selector, point) = if populated {
                (4, (21, 33))
            } else {
                (11, (-1, 14))
            };
            assert_eq!(state.city.mac_pending_action(), selector);
            assert_eq!(state.mac_loop_scheduler.pending_action_point, Some(point));
            assert_eq!(state.mac_loop_scheduler.disaster_point, point);
            assert_eq!(state.mac_process_rng.seed(), 1_354_537_686);
        }
    }
}

fn mac_phase24_fixture(path: &str, trend: u8, seed: u32) -> ViewerState {
    let city = blank_city_layout_fixture(path, "Mac phase 24");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.no_disasters_enabled = false;
    state.game_clock = GameClock::new_with_day(1910, 1, 23);
    state.traffic.set_speed(GameSpeed::Cheetah);
    state.mac_loop_scheduler.actor_deadline = 31;
    state.mac_process_rng = MacToolboxRng::new(seed);
    state
        .city
        .set_mac_state_misc_u32_at(MISC_YEAR_FOUNDED_OFFSET, 1900);
    state
        .city
        .set_mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET, CITY_DIFFICULTY_HARD);
    state
        .city
        .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
    state
        .city
        .set_mac_state_misc_u32_at(MISC_WEATHER_TREND_OFFSET, u32::from(trend));
    // Keep an unrelated terrain action queued when the monthly proposal misses.
    state.city.set_mac_pending_action(11);
    state.mac_loop_scheduler.pending_action_point = Some((13, 14));
    if trend == WEATHER_TREND_WINDY {
        state
            .city
            .set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 8000);
        for tile in [0xd2, 0xd3, 0xd1, 0xd6] {
            state.city.set_mac_state_misc_u32_at(0x01f0 + tile * 4, 9);
        }
        state.city.prepare_mac_fidelity_from_saved_city();
        state.city.set_mac_power_service_summary(0);
        state.city.set_mac_water_service_summary(0);
        state.city.set_mac_arcology_resident_total(0);
        for (row, current) in [(0, 1000), (1, 0), (2, 0), (10, 81)] {
            let offset =
                budget_row_misc_offset_for_city(&state.city, row, BUDGET_SERVICE_CURRENT_FIELD)
                    .unwrap();
            state.city.set_budget_misc_u32_values(offset, &[current]);
        }
    }
    state
}

#[test]
fn mac_earthquake_manual_menu_and_monthly_empty_scan_match_native() {
    for path in ["mac-empty-earthquake.city", "mac-empty-earthquake.sc2"] {
        let city = blank_city_layout_fixture(path, "Empty earthquake");
        let mut manual = test_state(city);
        manual.engine_fidelity = EngineFidelity::Mac68k;
        manual
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        manual.traffic.set_speed(GameSpeed::Paused);
        manual.mac_process_rng = MacToolboxRng::new(1);
        manual.hover_tile = Some((12, 34));
        let crt = manual.pe_process_crt_seed();

        assert!(manual.start_disaster(DisasterKind::Earthquake));
        assert_eq!(manual.city.mac_pending_action(), 6);
        assert_eq!(
            manual.mac_loop_scheduler.pending_action_point,
            Some((50, 96))
        );
        assert_eq!(manual.mac_process_rng.seed(), 282_475_249);
        assert_eq!(manual.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(manual.city.mac_pending_action(), 0);
        assert_eq!(
            manual.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(CITY_MODE_GAMEPLAY)
        );
        assert_eq!(manual.mac_process_rng.seed(), 1_017_324_090);
        assert_eq!(manual.pe_process_crt_seed(), crt);

        let mut monthly = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1864);
        assert_eq!(monthly.city.mac_pending_action(), 11);
        assert_eq!(
            monthly
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            1
        );
        assert_eq!(monthly.game_clock.day(), 24);
        assert_eq!(monthly.city.mac_pending_action(), 0);
        assert_eq!(
            monthly.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(CITY_MODE_GAMEPLAY)
        );
        assert_eq!(monthly.mac_process_rng.seed(), 2_022_709_593);
    }
}

#[test]
fn mac_earthquake_native_mixed_scan_and_reopened_first_marker_match_both_layouts() {
    for path in ["mac-mixed-earthquake.city", "mac-mixed-earthquake.sc2"] {
        let city = blank_city_layout_fixture(path, "Mixed earthquake");
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.city.prepare_mac_fidelity_from_saved_city();
        state
            .city
            .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        state.traffic.set_speed(GameSpeed::Paused);
        state.mac_process_rng = MacToolboxRng::new(1);
        state.city.set_mac_pending_action(6);
        state.mac_loop_scheduler.pending_action_point = Some((2, 3));
        for (x, y) in [(0, 14), (1, 32)] {
            state.city.set_building(x, y, 0x70);
            state.city.set_zone(x, y, 0x12);
            state.city.set_traffic_density(x, y, 91);
        }
        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(state.city.mac_pending_action(), 0);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.mac_process_rng.seed(), 1_338_654_090);
        assert_eq!(state.city.building_at(0, 14), 2);
        assert_eq!(state.city.zone_at(0, 14).unwrap().raw_bits(), 2);
        assert_eq!(state.city.text_overlay_at(0, 14), Some(0));
        assert_eq!(state.city.traffic_density_at(0, 14), Some(91));
        assert_eq!(state.city.building_at(1, 32), 0x70);
        assert_eq!(state.city.text_overlay_at(1, 32), Some(0xff));
        assert_eq!(state.city.traffic_density_at(1, 32), Some(0));

        let saved = std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
        state.city.save(&saved).unwrap();
        reopen_with_actor_fixture_seed(&mut state, &saved);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.city.text_overlay_at(1, 32), Some(0xff));
        state.mac_loop_scheduler.actor_deadline = 31;
        state.traffic.set_speed(GameSpeed::Cheetah);

        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            0
        );
        assert_eq!(state.mac_process_rng.seed(), 1_720_604_658);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.city.text_overlay_at(1, 32), Some(0xff));
    }
}

#[test]
fn mac_monthly_toxic_spill_releases_calendar_after_the_clearing_pass() {
    for path in ["mac-toxic-action.city", "mac-toxic-action.sc2"] {
        let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 20);
        let mut pollution = vec![0; CITY_TILES];
        pollution[density_map_index(26, 38)] = 0xb4;
        pollution[density_map_index(38, 26)] = 0xb4;
        state.city.sync_pollution_density_from_slice(&pollution);
        state.city.set_traffic_density(21, 33, 91);

        for (days, seed, marker, active) in [
            (1, 240_746_755, 0xfb, true),
            (0, 371_520_337, 0xfb, true),
            (0, 1_407_342_130, 0, true),
            (0, 1_407_342_130, 0, false),
        ] {
            let advance = state.advance_frame_simulation(Duration::from_millis(17));
            assert_eq!(advance.days, days);
            assert_eq!(state.city.mac_pending_action(), 0);
            assert_eq!(state.mac_process_rng.seed(), seed);
            assert_eq!(state.city.text_overlay_at(21, 33), Some(marker));
            assert_eq!(state.city.traffic_density_at(21, 33), Some(91));
            assert_eq!(
                scenario_action::mac_scenario_action_active(&state.city),
                active
            );
        }
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .months,
            1
        );
    }
}

fn mac_repeated_flood_fixture(path: &str, growth: u32) -> ViewerState {
    let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 8210);
    state
        .city
        .set_mac_state_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 1);
    state
        .city
        .set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, growth);
    state.sound_effects_enabled = true;
    state
}

fn mac_cloud_scatter_fixture(path: &str, hotspot_x: u16, growth: u32, funding: u32) -> ViewerState {
    let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 3114);
    state.city.set_mac_kind2_hotspot(hotspot_x, 73);
    state
        .city
        .set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, growth);
    let current =
        budget_row_misc_offset_for_city(&state.city, 2, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
    state.city.set_budget_misc_u32_values(current, &[funding]);
    state.city.set_text_overlay(3, 79, 7);
    state.city.set_traffic_density(3, 79, 91);
    state.city.set_traffic_density(0, 85, 91);
    state.sound_effects_enabled = true;
    state
}

#[test]
fn mac_scenario_loader_replaces_point_with_or_without_opening_action() {
    let mut scheduler = MacLoopScheduler::default();
    scheduler.load_scenario_point(8, (80, 52));
    assert_eq!(scheduler.disaster_point, (80, 52));
    assert_eq!(scheduler.pending_action_point, Some((80, 52)));

    scheduler.load_scenario_point(0, (0, 0));
    assert_eq!(scheduler.disaster_point, (0, 0));
    assert_eq!(scheduler.pending_action_point, None);
}

#[test]
fn mac_due_action_dispatch_consumes_reopened_pending_selector_before_one_marker_pass() {
    for path in ["mac-due-pending.city", "mac-due-pending.sc2"] {
        for (selector, active_flag, marker, expected_selector, expected_seed) in [
            (4, 1, 0xff, 4, 282_475_249),
            (15, 1, 0xff, 12, 16_807),
            (19, 1, 0xff, 12, 16_807),
            (19, 0, 0xff, 12, 1),
            // Completion calls the opinion poll before the next frame.
            (19, 1, 0, 12, 892_053_144),
        ] {
            let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1);
            state.city.prepare_mac_fidelity_from_saved_city();
            state
                .city
                .set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_DISASTER);
            state.city.set_mac_state_misc_u32_at(0x1044, 12);
            state.city.set_mac_state_misc_u32_at(0x1048, active_flag);
            state
                .city
                .set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 327_630_000);
            state.city.set_mac_pending_action(selector);
            state.city.set_text_overlay(20, 30, marker);
            state.mac_loop_scheduler.disaster_point = (40, 42);
            state.mac_loop_scheduler.pending_action_point = None;

            // Reopen valid active saves. Exercise the inactive mode-2 edge
            // directly because the session loader normalizes it to mode 1.
            if active_flag != 0 {
                let saved =
                    std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
                state.city.save(&saved).unwrap();
                reopen_with_actor_fixture_seed(&mut state, &saved);
            }
            state.traffic.set_speed(GameSpeed::Paused);
            state.mac_loop_scheduler.actor_deadline = u32::MAX;
            state.mac_loop_scheduler.deadline = 2;
            let clock = state.game_clock;
            let frame = Duration::from_millis(17);

            assert_eq!(state.advance_frame_simulation(frame).days, 0);
            state.traffic.set_speed(GameSpeed::Turtle);
            assert_eq!(state.advance_frame_simulation(frame).days, 0);
            assert_eq!(state.city.mac_pending_action(), selector);
            assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(12));
            assert_eq!(state.mac_process_rng.seed(), 1);

            assert_eq!(state.advance_frame_simulation(frame).days, 0);
            assert_eq!(state.city.mac_pending_action(), 0);
            assert_eq!(
                state.city.mac_state_misc_u32_at(0x1044),
                Some(expected_selector)
            );
            let expected_flag = if marker == 0 { 0 } else { active_flag };
            let expected_mode = if marker == 0 {
                CITY_MODE_GAMEPLAY
            } else {
                CITY_MODE_DISASTER
            };
            assert_eq!(
                state.city.mac_state_misc_u32_at(0x1048),
                Some(expected_flag)
            );
            assert_eq!(
                state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
                Some(expected_mode)
            );
            assert_eq!(state.city.text_overlay_at(20, 30), Some(marker));
            assert_eq!(
                state.city.text_overlay_at(40, 42),
                Some(if selector == 4 { 0xfb } else { 0 })
            );
            assert_eq!(state.mac_loop_scheduler.disaster_point, (40, 42));
            assert_eq!(state.mac_process_rng.seed(), expected_seed);
            assert_eq!(state.mac_loop_scheduler.dispatch_counter, 1);
            assert_eq!(state.mac_loop_scheduler.deadline, 39);
            assert_eq!(state.game_clock, clock);
        }
    }
}

#[test]
fn mac_kind6_terminal_sweep_queues_only_successful_ignition_outside_action_mode() {
    for path in ["mac-kind6-terminal.city", "mac-kind6-terminal.sc2"] {
        for (phase, mode, wet, pending, point) in [
            (5, CITY_MODE_GAMEPLAY, false, 5, Some((32, 31))),
            (17, CITY_MODE_GAMEPLAY, false, 17, Some((32, 31))),
            (0, CITY_MODE_GAMEPLAY, false, 1, Some((32, 31))),
            (5, CITY_MODE_DISASTER, false, 14, None),
            (17, CITY_MODE_GAMEPLAY, true, 14, None),
        ] {
            let mut city = blank_city_layout_fixture(path, "Kind-6 terminal fixture");
            city.prepare_mac_fidelity_from_saved_city();
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, mode);
            city.set_mac_pending_action(14);
            city.set_mac_object_slot_record(
                1,
                Win95OverlayRecord {
                    kind: 6,
                    lane: 2,
                    state: phase,
                    tile_x: 32,
                    tile_y: 31,
                    sub_x: 8,
                    sub_y: 8,
                    saved_b: 1,
                    ..Win95OverlayRecord::default()
                },
            );
            city.set_text_overlay(32, 31, 0xca);
            city.set_tile_bits(30, 29, TileBits::new(if wet { 4 } else { 0 }));
            let mut quickdraw = MacToolboxRng::new(1);
            let mut action_point = None;

            let sounds = tick_mac_actors_with_rng_and_clock(
                &mut city,
                &mut quickdraw,
                &mut MacGameLcgRng::new(1),
                &mut 0,
                2,
                &mut 0,
                &mut action_point,
            );
            assert_eq!(city.traffic_overlays().unwrap()[1].kind, 0);
            assert_eq!(city.text_overlay_at(32, 31), Some(0));
            assert_eq!(
                city.text_overlay_at(30, 29),
                Some(if wet { 0 } else { 0xff })
            );
            assert_eq!(city.mac_pending_action(), pending);
            assert_eq!(action_point, point);
            assert_eq!(quickdraw.seed(), 1);
            assert!(sounds.is_empty());
        }
    }
}

#[test]
fn mac_kind6_terminal_actor_enters_shared_disaster_pass_in_the_same_frame() {
    for path in ["mac-kind6-admission.city", "mac-kind6-admission.sc2"] {
        for phase in [5, 17] {
            let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1);
            state.city.prepare_mac_fidelity_from_saved_city();
            if state.city.kind.is_win95() {
                state
                    .city
                    .initialize_mac_scenario(crate::mac_scenario::MacScenarioStart {
                        action: 8,
                        point: (80, 52),
                        goals: [0; 16],
                    });
            } else {
                state.city.set_mac_pending_action(14);
            }
            state.mac_loop_scheduler.actor_deadline = 0;
            state.mac_loop_scheduler.deadline = u32::MAX;
            state.mac_loop_scheduler.disaster_point = (13, 14);
            state.mac_loop_scheduler.pending_action_point = None;
            state.city.set_mac_object_slot_record(
                1,
                Win95OverlayRecord {
                    kind: 6,
                    lane: 2,
                    state: phase,
                    tile_x: 32,
                    tile_y: 31,
                    sub_x: 8,
                    sub_y: 8,
                    saved_b: 1,
                    ..Win95OverlayRecord::default()
                },
            );
            state.city.set_text_overlay(32, 31, 0xca);

            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(17))
                    .days,
                0
            );
            assert_eq!(state.city.mac_pending_action(), 0);
            assert_eq!(state.city.take_mac_scenario_pending_point(), None);
            assert_eq!(state.mac_loop_scheduler.disaster_point, (32, 31));
            assert_eq!(
                state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
                Some(CITY_MODE_DISASTER)
            );
            assert_eq!(
                state.city.mac_state_misc_u32_at(0x1044),
                Some(u32::from(phase))
            );
            assert_eq!(state.city.mac_state_misc_u32_at(0x1048), Some(1));
            assert!(scenario_action::mac_scenario_action_active(&state.city));
            assert_eq!(state.city.text_overlay_at(30, 29), Some(0xff));
            assert_eq!(state.mac_process_rng.seed(), 16_807);
        }
    }
}

#[test]
fn mac_air_crash_matches_native_actor_lifecycle_on_both_layouts() {
    let frame = Duration::from_millis(534);
    let opening = [1, 3, 7, 32, 32, 16, 8, 8, 0x51, 0x52, 0, 0x5a];
    for path in ["mac-air-crash.city", "mac-air-crash.sc2"] {
        for (monthly, disabled, seed, ignition_seed, terminal_visit, terminal_seed) in [
            (false, false, 1, 16_807, 26, 470_211_272),
            (true, false, 712_393_418, 974_844_301, 22, 1_055_423_944),
            (false, true, 1, 1, 20, 1),
        ] {
            let mut state =
                mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, if monthly { 258 } else { 1 });
            state.mac_loop_scheduler.disaster_point = (13, 14);
            state.sound_effects_enabled = true;
            state.no_disasters_enabled = disabled;
            set_no_disasters_enabled_for_engine(&mut state.city, EngineFidelity::Mac68k, disabled);
            set_sound_effects_enabled_for_engine(&mut state.city, EngineFidelity::Mac68k, true);
            let record = &mut state.city.mac_object_slot_records_mut()[1];
            record.lane = 3;
            record.target_x = 0x51;
            record.target_y = 0x52;
            record.saved_a = 0x53;
            record.saved_b = 0x5a;
            state.city.set_traffic_density(30, 30, 91);
            let opening_frame = if monthly {
                state.city.prepare_mac_fidelity_from_saved_city();
                state.city.set_building(90, 90, 0xdd);
                assert_ne!(state.city.mac_xbld_tile_count(0xdd), 0);
                Duration::from_millis(17)
            } else {
                state.traffic.set_speed(GameSpeed::Paused);
                state.city.set_mac_pending_action(0);
                assert!(state.start_disaster_at(DisasterKind::AirCrash, (13, 14)));
                assert_eq!(state.city.mac_pending_action(), 18);
                assert_eq!(state.mac_process_rng.seed(), 1);
                Duration::ZERO
            };
            assert_eq!(
                state.advance_frame_simulation(opening_frame).days,
                u32::from(monthly)
            );
            assert_eq!(state.city.mac_pending_action(), 0);
            assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(18));
            assert_eq!(state.city.mac_state_misc_u32_at(0x1048), Some(255));
            assert_eq!(
                state.city.traffic_overlays().unwrap()[1].to_bytes(),
                opening
            );
            assert_eq!(state.city.text_overlay_at(32, 32), Some(0xca));
            assert_eq!(state.mac_loop_scheduler.disaster_point, (32, 32));
            assert_eq!(state.mac_process_rng.seed(), seed);
            assert!(state.take_pending_disaster_extra_sounds().is_empty());
            state.traffic.set_speed(GameSpeed::Cheetah);

            for visit in 1..=terminal_visit {
                assert_eq!(state.advance_frame_simulation(frame).days, 0);
                let actor = state.city.traffic_overlays().unwrap()[1];
                assert_eq!(
                    state.take_pending_disaster_extra_sounds(),
                    match visit {
                        8 => &[PlacementSound::Mayday][..],
                        18 => &[PlacementSound::Explode][..],
                        _ => &[],
                    }
                );
                if visit <= 19 {
                    assert_eq!(state.mac_process_rng.seed(), seed);
                    assert_eq!(
                        state
                            .city
                            .text_overlay_at(actor.tile_x as usize, actor.tile_y as usize),
                        Some(0xca)
                    );
                }
                match visit {
                    1 => assert_eq!(
                        actor.to_bytes(),
                        [1, 4, 7, 33, 33, 15, 8, 8, 0x51, 0x52, 0, 0x5a]
                    ),
                    8 => {
                        assert_eq!(
                            actor.to_bytes(),
                            [1, 3, 7, 32, 32, 8, 8, 8, 0x51, 0x52, 0, 0x5a]
                        );
                        if monthly {
                            let saved = std::env::temp_dir()
                                .join(format!("citygame-{}-{path}", std::process::id()));
                            state.city.save(&saved).unwrap();
                            reopen_with_actor_fixture_seed(&mut state, &saved);
                            state.mac_loop_scheduler.actor_deadline = 31;
                            state.traffic.set_speed(GameSpeed::Cheetah);
                            assert_eq!(state.city.traffic_overlays().unwrap()[1], actor);
                            assert!(scenario_action::mac_scenario_action_active(&state.city));
                        }
                    }
                    17..=19 => assert_eq!(
                        actor.to_bytes(),
                        [6, visit as u8 - 17, 5, 32, 32, 0, 8, 8, 0x51, 0x52, 0, 1,]
                    ),
                    20 => {
                        assert_eq!(
                            actor.to_bytes(),
                            [0, 2, 5, 32, 32, 0, 8, 8, 0x51, 0x52, 0, 1]
                        );
                        assert_eq!(state.city.text_overlay_at(32, 32), Some(0));
                        assert_eq!(
                            state.city.text_overlay_at(30, 30),
                            Some(if disabled { 0 } else { 0xff })
                        );
                        assert_eq!(
                            state.city.traffic_density_at(30, 30),
                            Some(if disabled { 91 } else { 0 })
                        );
                        assert_eq!(state.mac_process_rng.seed(), ignition_seed);
                        if !disabled {
                            state.city.set_tile_bits(30, 30, TileBits::new(4));
                        }
                    }
                    _ => {}
                }
                assert_eq!(
                    scenario_action::mac_scenario_action_active(&state.city),
                    visit < terminal_visit
                );
            }
            assert_eq!(state.city.text_overlay_at(30, 30), Some(0));
            assert_eq!(state.mac_process_rng.seed(), terminal_seed);
            assert_eq!(
                state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
                Some(CITY_MODE_GAMEPLAY)
            );
            assert_eq!(state.advance_frame_simulation(frame).days, 1);
        }
    }
}

#[test]
fn mac_air_crash_rejects_intersection_only_full_table_and_fixed_occupied_cell() {
    for path in ["mac-air-crash-gates.city", "mac-air-crash-gates.sc2"] {
        let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 258);
        state.city.prepare_mac_fidelity_from_saved_city();
        state.city.set_building(90, 90, 0xde);
        assert_ne!(state.city.mac_xbld_tile_count(0xde), 0);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            1
        );
        assert_eq!(state.city.mac_pending_action(), 11);
        assert_eq!(state.mac_process_rng.seed(), 2_011_653_891);
        assert!(!scenario_action::mac_scenario_action_active(&state.city));

        for full in [false, true] {
            let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1);
            state.traffic.set_speed(GameSpeed::Paused);
            state.city.set_mac_pending_action(0);
            state.mac_loop_scheduler.disaster_point = (13, 14);
            if full {
                for actor in state.city.mac_object_slot_records_mut().iter_mut().skip(1) {
                    actor.kind = 2;
                }
            } else {
                state.city.set_text_overlay(32, 32, 0x22);
            }
            assert!(state.start_disaster_at(DisasterKind::AirCrash, (13, 14)));
            state.advance_frame_simulation(Duration::ZERO);
            assert_eq!(state.mac_loop_scheduler.disaster_point, (13, 14));
            assert_eq!(state.city.mac_pending_action(), 0);
            assert_eq!(state.mac_process_rng.seed(), 1);
            assert!(!scenario_action::mac_scenario_action_active(&state.city));
            assert_eq!(
                state.city.text_overlay_at(32, 32).unwrap_or(0),
                if full { 0 } else { 0x22 }
            );
        }
    }
}

fn check_mac_marker_traffic_planes(
    city: &City,
    markers: &[(usize, usize, u8)],
    cleared: &[(usize, usize)],
) {
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            let marker = markers
                .iter()
                .find(|&&(mx, my, _)| (x, y) == (mx, my))
                .map_or(0, |&(_, _, marker)| marker);
            assert_eq!(
                city.text_overlay_at(x, y).unwrap_or(0),
                marker,
                "({x}, {y})"
            );
            assert_eq!(
                city.traffic_density_at(x, y),
                Some(if cleared.contains(&(x / 2, y / 2)) {
                    0
                } else {
                    91
                }),
                "({x}, {y})"
            );
        }
    }
}

fn fill_mac_traffic_plane(city: &mut City) {
    for x in (0..CITY_WIDTH).step_by(2) {
        for y in (0..CITY_HEIGHT).step_by(2) {
            city.set_traffic_density(x, y, 91);
        }
    }
}

#[test]
fn mac_facility_scatter_matches_native_damage_and_retained_point_on_both_layouts() {
    let digest = |bytes: &mut dyn Iterator<Item = u8>| {
        bytes.fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
    };
    let planes = |city: &City| {
        let cells = || (0..CITY_WIDTH).flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)));
        [
            digest(&mut cells().map(|(x, y)| city.building_at(x, y))),
            digest(&mut cells().map(|(x, y)| city.text_overlay_at(x, y).unwrap_or(0))),
            digest(
                &mut cells()
                    .filter(|&(x, y)| x % 2 == 0 && y % 2 == 0)
                    .map(|(x, y)| city.traffic_density_at(x, y).unwrap()),
            ),
            digest(
                &mut cells().map(|(x, y)| city.tile_bits_at(x, y).unwrap_or_default().raw_bits()),
            ),
            digest(&mut cells().map(|(x, y)| city.zone_at(x, y).unwrap_or_default().raw_bits())),
            digest(&mut (0..=255).flat_map(|tile| city.mac_xbld_tile_count(tile).to_be_bytes())),
        ]
    };
    for path in ["mac-facility-scatter.city", "mac-facility-scatter.sc2"] {
        for priority in [false, true] {
            let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 714);
            state.city.prepare_mac_fidelity_from_saved_city();
            state.city.initialize_mac_fidelity_blank_xbld_census();
            if state.city.kind == CityFileKind::MacCity {
                state
                    .city
                    .set_mac_state_misc_u32_at(0x01f0, CITY_TILES as u32);
            }
            assert_eq!(state.city.mac_xbld_tile_count(0), CITY_TILES as u16);
            for (ax, ay) in [(50, 60), (97, 105)] {
                for dx in 0..4 {
                    for dy in 0..4 {
                        state.city.set_building(ax + dx, ay - dy, 0xcb);
                        let zone = match (dx, dy) {
                            (0, 3) => 0x10,
                            (3, 3) => 0x20,
                            (3, 0) => 0x40,
                            (0, 0) => 0x80,
                            _ => 0,
                        };
                        state.city.set_zone(ax + dx, ay - dy, zone);
                    }
                }
            }
            state.city.set_text_overlay(13, 14, 0xcb);
            fill_mac_traffic_plane(&mut state.city);
            if priority {
                state.mac_loop_scheduler.pending_action_point = None;
                state.mac_loop_scheduler.disaster_point = (98, 104);
            } else {
                for x in 26..42 {
                    for y in 41..56 {
                        state.city.set_building(x, y, 0x70);
                    }
                }
                for y in 40..49 {
                    state.city.set_tile_bits(43, y, TileBits::new(4));
                }
            }
            let frame = Duration::from_millis(17);
            assert_eq!(state.advance_frame_simulation(frame).days, 1);
            assert_eq!(state.city.mac_pending_action(), 0);
            assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(9));
            let mut expected = if priority {
                [
                    0xb4aaf1bbb199911f,
                    0xa9d2394cf46037cd,
                    0x3b8f1a3fe9b094c2,
                    0x9c1bda7f8c872325,
                    0xb999844d3df6fd55,
                    0x5bd55f67ddcebdba,
                ]
            } else {
                [
                    0xe26c5034f111fae6,
                    0xcb0a69314c35afad,
                    0xc5c72d5c3368bd54,
                    0xad3ecbfb0a4dfc41,
                    0x80c3850fcccc62b5,
                    0x07896cc5fca332d3,
                ]
            };
            assert_eq!(
                state.mac_process_rng.seed(),
                if priority { 655_604_670 } else { 271_558_597 },
                "{path}, priority={priority}"
            );
            assert_eq!(planes(&state.city), expected, "{path}, priority={priority}");

            let saved =
                std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
            state.city.save(&saved).unwrap();
            reopen_with_actor_fixture_seed(&mut state, &saved);
            state.mac_loop_scheduler.actor_deadline = 1000;
            state.traffic.set_speed(GameSpeed::Cheetah);
            assert_eq!(state.advance_frame_simulation(frame).days, 0);
            assert!(scenario_action::mac_scenario_action_active(&state.city));
            assert_eq!(
                state.mac_process_rng.seed(),
                if priority {
                    1_229_508_459
                } else {
                    1_571_192_825
                }
            );
            if !priority {
                expected[1] = 0xbe16342e9b1126a4;
                expected[2] = 0x989fcb063e6619ff;
            }
            assert_eq!(planes(&state.city), expected, "{path}, priority={priority}");
        }
    }
}

#[test]
fn mac_facility_scatter_rejects_zero_census_or_missing_building() {
    for path in [
        "mac-facility-scatter-gates.city",
        "mac-facility-scatter-gates.sc2",
    ] {
        for census in [false, true] {
            let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 714);
            state.city.prepare_mac_fidelity_from_saved_city();
            if census {
                state.city.set_building(13, 14, 0xcb);
                state.city.set_building_without_mac_census(13, 14, 0);
            }
            state.city.set_text_overlay(13, 14, 0xcb);
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(17))
                    .days,
                1
            );
            assert_eq!(state.city.mac_pending_action(), if census { 0 } else { 11 });
            assert_eq!(state.mac_process_rng.seed(), 1_971_348_615);
            assert!(!scenario_action::mac_scenario_action_active(&state.city));
            assert_eq!(state.city.text_overlay_at(13, 14), Some(0xcb));
        }
    }
}

#[test]
fn mac_facility_walk_matches_native_opening_and_resumes_in_both_layouts() {
    let mut markers = vec![(1, 1, 0xcd), (21, 92, 0xfb)];
    markers.extend(
        [
            (21, 91),
            (21, 94),
            (21, 95),
            (22, 93),
            (22, 94),
            (22, 95),
            (23, 91),
            (23, 92),
            (23, 93),
            (23, 94),
            (24, 92),
            (24, 93),
            (24, 94),
            (25, 91),
            (25, 92),
            (25, 93),
            (26, 92),
            (26, 93),
            (26, 94),
            (27, 93),
        ]
        .map(|(x, y)| (x, y, 0xff)),
    );
    let cleared = [
        (10, 45),
        (10, 47),
        (11, 45),
        (11, 46),
        (11, 47),
        (12, 45),
        (12, 46),
        (12, 47),
        (13, 46),
        (13, 47),
    ];
    for path in ["mac-facility-walk.city", "mac-facility-walk.sc2"] {
        let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1884);
        state.city.prepare_mac_fidelity_from_saved_city();
        state.sound_effects_enabled = true;
        for (x, y) in [(20, 91), (22, 92), (45, 45)] {
            state.city.set_building(x, y, 0xcd);
        }
        state.city.set_text_overlay(1, 1, 0xcd);
        state.city.set_tile_bits(21, 92, TileBits::new(4));
        fill_mac_traffic_plane(&mut state.city);
        let frame = Duration::from_millis(17);
        assert_eq!(state.advance_frame_simulation(frame).days, 1);
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(10));
        assert_eq!(state.mac_process_rng.seed(), 2_025_630_486);
        assert_eq!(
            state.take_pending_disaster_extra_sounds(),
            [PlacementSound::PowerLine; 36]
        );
        check_mac_marker_traffic_planes(&state.city, &markers, &cleared);

        let saved = std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
        state.city.save(&saved).unwrap();
        reopen_with_actor_fixture_seed(&mut state, &saved);
        state.mac_loop_scheduler.actor_deadline = 31;
        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(state.advance_frame_simulation(frame).days, 0);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.mac_process_rng.seed(), 16_439_039);
        check_mac_marker_traffic_planes(&state.city, &markers, &cleared);
    }
}

#[test]
fn mac_facility_walk_checks_saved_census_source_plane_and_edges() {
    for path in [
        "mac-facility-walk-gates.city",
        "mac-facility-walk-gates.sc2",
    ] {
        for (source, stale, pending, seed, active_selector) in [
            (None, false, 11, 1_754_908_307, 0),
            (Some((1, 0)), true, 0, 1_754_908_307, 0),
            (Some((1, 0)), false, 0, 1_203_507_851, 10),
            (Some((127, 127)), false, 0, 207_980_664, 10),
        ] {
            let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1884);
            state.city.prepare_mac_fidelity_from_saved_city();
            state.sound_effects_enabled = true;
            if let Some((x, y)) = source {
                state.city.set_building(x, y, 0xcd);
                if stale {
                    // A stale census admits the proposal while the search
                    // sees only the decoy in the separate XTXT plane.
                    state.city.set_building_without_mac_census(x, y, 0);
                }
            }
            state.city.set_text_overlay(1, 1, 0xcd);
            fill_mac_traffic_plane(&mut state.city);
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(17))
                    .days,
                1
            );
            assert_eq!(state.city.mac_pending_action(), pending);
            assert_eq!(state.mac_process_rng.seed(), seed);
            assert_eq!(
                state.city.mac_state_misc_u32_at(0x1044).unwrap_or(0),
                active_selector
            );
            assert!(!scenario_action::mac_scenario_action_active(&state.city));
            assert!(state.take_pending_disaster_extra_sounds().is_empty());
            check_mac_marker_traffic_planes(&state.city, &[(1, 1, 0xcd)], &[]);
        }
    }
}

#[test]
fn mac_rioters_follow_native_three_searches_and_continue_after_reopen() {
    for path in ["mac-rioters.city", "mac-rioters.sc2"] {
        let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1);
        state.traffic.set_speed(GameSpeed::Paused);
        state.sound_effects_enabled = true;
        state.city.set_mac_pending_action(3);
        state.mac_loop_scheduler.pending_action_point = Some((64, 64));
        for (x, y) in [
            (64, 61),
            (64, 62),
            (64, 63),
            (63, 62),
            (65, 62),
            (65, 63),
            (65, 64),
        ] {
            state.city.set_building(x, y, 0x1d);
        }
        state.city.set_tile_bits(64, 63, TileBits::new(4));
        state.city.set_text_overlay(65, 63, 0x32);
        fill_mac_traffic_plane(&mut state.city);

        assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.mac_loop_scheduler.disaster_point, (64, 61));
        assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(3));
        assert_eq!(state.mac_process_rng.seed(), 2_007_237_709);
        assert_eq!(
            state.take_pending_disaster_extra_sounds(),
            [PlacementSound::Riot; 3]
        );
        check_mac_marker_traffic_planes(
            &state.city,
            &[(64, 61, 0xfe), (64, 62, 0xfe), (65, 63, 0xfe)],
            &[(32, 32)],
        );

        let saved = std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
        state.city.save(&saved).unwrap();
        reopen_with_actor_fixture_seed(&mut state, &saved);
        state.mac_loop_scheduler.actor_deadline = 31;
        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            0
        );
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        assert_eq!(state.mac_process_rng.seed(), 1_137_522_503);
        assert!(state.take_pending_disaster_extra_sounds().is_empty());
        check_mac_marker_traffic_planes(
            &state.city,
            &[(64, 61, 0xfd), (64, 62, 0xfe), (65, 63, 0xfe)],
            &[(32, 30), (32, 32)],
        );
    }
}

#[test]
fn mac_rioters_monthly_scatter_matches_native_edge_searches() {
    for path in ["mac-mass-rioters.city", "mac-mass-rioters.sc2"] {
        let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1310);
        state.sound_effects_enabled = true;
        state.city.set_mac_kind2_hotspot(5, 94);
        for (offset, value) in [
            (MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 30_000),
            (MAC_RIOT_UNEMPLOYMENT_STATE_OFFSET, 10),
            (MAC_RIOT_WEATHER_HEAT_STATE_OFFSET, 0xaa),
        ] {
            state.city.set_mac_state_misc_u32_at(offset, value);
        }
        for x in 0..3 {
            for y in 78..87 {
                state.city.set_building(x, y, 0x1d);
            }
        }
        fill_mac_traffic_plane(&mut state.city);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            1
        );
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.mac_loop_scheduler.disaster_point, (2, 79));
        assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(13));
        assert_eq!(state.mac_process_rng.seed(), 725_359_301);
        assert_eq!(
            state.take_pending_disaster_extra_sounds(),
            [PlacementSound::Riot; 8]
        );
        check_mac_marker_traffic_planes(
            &state.city,
            &[
                (0, 86, 0xfd),
                (1, 78, 0xfe),
                (1, 79, 0xfd),
                (1, 83, 0xfd),
                (2, 79, 0xfd),
                (2, 83, 0xfe),
                (2, 86, 0xfe),
            ],
            &[(0, 39), (0, 41), (1, 39), (1, 43)],
        );
    }
}

#[test]
fn mac_rioters_can_ignite_a_road_during_the_shared_marker_pass() {
    for path in ["mac-riot-ignition.city", "mac-riot-ignition.sc2"] {
        let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1);
        state.traffic.set_speed(GameSpeed::Paused);
        state.sound_effects_enabled = true;
        state.city.set_mac_pending_action(13);
        state.mac_loop_scheduler.pending_action_point = Some((0, 82));
        state
            .city
            .set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 30_000);
        for x in 0..3 {
            for y in 78..87 {
                state.city.set_building(x, y, 0x1d);
            }
        }
        fill_mac_traffic_plane(&mut state.city);
        state.advance_frame_simulation(Duration::ZERO);
        assert_eq!(state.mac_process_rng.seed(), 530_511_967);
        assert_eq!(
            state.take_pending_disaster_extra_sounds(),
            [PlacementSound::Riot; 8]
        );

        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            0
        );
        assert_eq!(state.mac_process_rng.seed(), 685_118_024);
        assert_eq!(
            state.take_pending_disaster_extra_sounds(),
            [PlacementSound::Riot]
        );
        assert_eq!(state.city.building_at(1, 79), 0x1d);
        check_mac_marker_traffic_planes(
            &state.city,
            &[
                (0, 80, 0xfe),
                (0, 84, 0xfe),
                (1, 79, 0xff),
                (1, 86, 0xfe),
                (2, 78, 0xfd),
                (2, 79, 0xfe),
                (2, 86, 0xfe),
            ],
            &[(0, 39), (0, 42), (0, 43)],
        );
    }
}

#[test]
fn mac_rioters_failed_start_and_wet_marker_release_match_native() {
    for path in ["mac-riot-release.city", "mac-riot-release.sc2"] {
        let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1);
        state.traffic.set_speed(GameSpeed::Paused);
        state.sound_effects_enabled = true;
        state.city.set_mac_pending_action(0);
        state.city.set_mac_kind2_hotspot(72, 62);
        assert!(state.start_disaster_at(DisasterKind::Rioters, (1, 2)));
        assert_eq!(state.city.mac_pending_action(), 3);
        assert_eq!(
            state.mac_loop_scheduler.pending_action_point,
            Some((64, 64))
        );
        fill_mac_traffic_plane(&mut state.city);
        state.advance_frame_simulation(Duration::ZERO);
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.mac_process_rng.seed(), 282_475_249);
        assert!(!scenario_action::mac_scenario_action_active(&state.city));
        assert!(state.take_pending_disaster_extra_sounds().is_empty());
        check_mac_marker_traffic_planes(&state.city, &[], &[]);
        assert_eq!(state.mac_loop_scheduler.disaster_point, (64, 64));

        state.mac_process_rng = MacToolboxRng::new(1);
        state
            .city
            .set_mac_state_misc_u32_at(MAC_MASS_RIOT_GROWTH_STATE_OFFSET, 30_000);
        state.city.set_mac_pending_action(13);
        state.mac_loop_scheduler.pending_action_point = Some((0, 82));
        state.advance_frame_simulation(Duration::ZERO);
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.mac_loop_scheduler.disaster_point, (3, 73));
        assert_eq!(state.mac_process_rng.seed(), 1_137_522_503);
        assert!(!scenario_action::mac_scenario_action_active(&state.city));
        assert!(state.take_pending_disaster_extra_sounds().is_empty());
        check_mac_marker_traffic_planes(&state.city, &[], &[]);

        state.mac_process_rng = MacToolboxRng::new(470_211_272);
        state.city.set_tile_bits(64, 62, TileBits::new(4));
        state.city.set_text_overlay(64, 62, 0xfd);
        scenario_action::begin_action(&mut state.city, 3);
        state.traffic.set_speed(GameSpeed::Cheetah);
        for (mode, flag, seed) in [
            (CITY_MODE_DISASTER, 1, 1_458_777_923),
            (CITY_MODE_GAMEPLAY, 0, 1_147_722_294),
        ] {
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(17))
                    .days,
                0
            );
            assert_eq!(
                state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
                Some(mode)
            );
            assert_eq!(state.city.mac_state_misc_u32_at(0x1048), Some(flag));
            assert_eq!(state.mac_process_rng.seed(), seed);
            assert!(state.take_pending_disaster_extra_sounds().is_empty());
            check_mac_marker_traffic_planes(&state.city, &[], &[]);
        }
    }
}

#[test]
fn mac_cloud_scatter_matches_native_lifecycle_after_reopen_in_both_layouts() {
    let check_planes = |city: &City, remaining: u8| {
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                assert_eq!(
                    city.text_overlay_at(x, y).unwrap_or(0),
                    if (x, y) == (0, 85) { remaining } else { 0 },
                    "({x}, {y})"
                );
            }
        }
        for (x, y) in [(0, 85), (3, 79)] {
            assert_eq!(city.traffic_density_at(x, y), Some(91));
        }
    };
    for path in ["mac-cloud-scatter.city", "mac-cloud-scatter.sc2"] {
        let mut state = mac_cloud_scatter_fixture(path, 5, 20_000, 10_000);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            1
        );
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(15));
        assert_eq!(state.city.mac_state_misc_u32_at(0x1048), Some(1));
        assert_eq!(state.mac_loop_scheduler.disaster_point, (3, 79));
        assert_eq!(state.mac_process_rng.seed(), 315_095_192);
        assert!(state.take_pending_disaster_extra_sounds().is_empty());
        check_planes(&state.city, 0xfb);

        let saved = std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
        state.city.save(&saved).unwrap();
        reopen_with_actor_fixture_seed(&mut state, &saved);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        state.mac_loop_scheduler.actor_deadline = 31;
        state.traffic.set_speed(GameSpeed::Cheetah);

        for (mode, flag, seed) in [
            (CITY_MODE_DISASTER, 1, 110_218_442),
            (CITY_MODE_GAMEPLAY, 0, 738_537_539),
        ] {
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(17))
                    .days,
                0
            );
            assert_eq!(
                state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
                Some(mode)
            );
            assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(15));
            assert_eq!(state.city.mac_state_misc_u32_at(0x1048), Some(flag));
            assert_eq!(state.mac_process_rng.seed(), seed);
            check_planes(&state.city, 0);
        }
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            1
        );
    }
}

#[test]
fn mac_cloud_scatter_matches_native_proposal_and_attempt_boundaries() {
    for path in ["mac-cloud-boundary.city", "mac-cloud-boundary.sc2"] {
        for (x, growth, funding, pending, seed, marker, active) in [
            (5, 20_000, 9_999, 11, 1_307_113_763, 7, false),
            (0, 20_000, 10_000, 0, 2_087_680_225, 7, false),
            (5, 327_630_000, 10_000, 0, 507_903_096, 7, false),
            (5, 2_621_400_000, 10_000, 0, 1_067_159_554, 0, true),
        ] {
            let mut state = mac_cloud_scatter_fixture(path, x, growth, funding);
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(17))
                    .days,
                1
            );
            assert_eq!(state.city.mac_pending_action(), pending);
            assert_eq!(state.mac_process_rng.seed(), seed);
            assert_eq!(state.city.text_overlay_at(3, 79), Some(marker));
            assert_eq!(state.city.text_overlay_at(0, 85), Some(0));
            assert_eq!(
                scenario_action::mac_scenario_action_active(&state.city),
                active
            );
            assert!(state.take_pending_disaster_extra_sounds().is_empty());
        }
    }
}

#[test]
fn mac_repeated_flood_monthly_shore_matches_native_through_reopen() {
    let opening = [
        (6, 67),
        (6, 69),
        (12, 88),
        (13, 87),
        (16, 97),
        (16, 99),
        (17, 98),
    ];
    let first = [
        (5, 68),
        (6, 68),
        (11, 88),
        (13, 86),
        (15, 97),
        (15, 100),
        (16, 100),
        (17, 97),
    ];
    let second = [
        (5, 69),
        (6, 71),
        (7, 69),
        (7, 70),
        (7, 71),
        (10, 89),
        (11, 89),
        (13, 85),
        (13, 88),
        (14, 87),
        (14, 88),
        (16, 96),
        (16, 98),
        (17, 100),
        (18, 98),
    ];
    let check_planes = |city: &City, spread: &[(usize, usize)]| {
        for x in 0..CITY_WIDTH {
            for y in 0..CITY_HEIGHT {
                let flooded = opening.contains(&(x, y)) || spread.contains(&(x, y));
                assert_eq!(
                    city.text_overlay_at(x, y),
                    Some(if flooded { 0xfc } else { 0 }),
                    "({x}, {y})"
                );
                let cleared = spread
                    .iter()
                    .any(|&(sx, sy)| (sx / 2, sy / 2) == (x / 2, y / 2));
                assert_eq!(
                    city.traffic_density_at(x, y),
                    Some(if cleared { 0 } else { 91 }),
                    "({x}, {y})"
                );
            }
        }
    };
    for path in ["mac-repeated-flood.city", "mac-repeated-flood.sc2"] {
        let mut state = mac_repeated_flood_fixture(path, 20_000);
        for (x, y) in [(12, 87), (6, 68), (16, 98)] {
            state.city.set_terrain(x, y, 0x20);
        }
        state.city.set_tile_bits(5, 68, TileBits::new(4));
        for x in (0..CITY_WIDTH).step_by(2) {
            for y in (0..CITY_HEIGHT).step_by(2) {
                state.city.set_traffic_density(x, y, 91);
            }
        }
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            1
        );
        assert_eq!(state.city.mac_pending_action(), 0);
        assert_eq!(state.mac_loop_scheduler.disaster_point, (16, 98));
        assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(14));
        assert_eq!(state.city.mac_state_misc_u32_at(0x1048), Some(1));
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(CITY_MODE_DISASTER)
        );
        assert_eq!(
            state.city.mac_state_misc_u32_at(hurricane::FLOOD_DURATION),
            Some(59)
        );
        assert_eq!(state.mac_process_rng.seed(), 1_448_865_284);
        assert_eq!(
            state.take_pending_disaster_extra_sounds(),
            vec![PlacementSound::Flood; 5]
        );
        check_planes(&state.city, &first);

        let saved = std::env::temp_dir().join(format!("citygame-{}-{path}", std::process::id()));
        state.city.save(&saved).unwrap();
        reopen_with_actor_fixture_seed(&mut state, &saved);
        assert!(scenario_action::mac_scenario_action_active(&state.city));
        state.mac_loop_scheduler.actor_deadline = 31;
        state.traffic.set_speed(GameSpeed::Cheetah);

        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(17))
                .days,
            0
        );
        assert_eq!(state.city.mac_state_misc_u32_at(0x1044), Some(14));
        assert_eq!(
            state.city.mac_state_misc_u32_at(hurricane::FLOOD_DURATION),
            Some(58)
        );
        assert_eq!(state.mac_process_rng.seed(), 2_115_612_608);
        assert_eq!(
            state.take_pending_disaster_extra_sounds(),
            [PlacementSound::Flood]
        );
        check_planes(&state.city, &[first.as_slice(), second.as_slice()].concat());
    }
}

#[test]
fn mac_repeated_flood_monthly_failures_do_not_enter_action_mode() {
    for path in ["mac-failed-flood.city", "mac-failed-flood.sc2"] {
        for (growth, seed) in [(20_000, 434_025_329), (327_630_000, 1_111_501_225)] {
            let mut state = mac_repeated_flood_fixture(path, growth);
            if growth == 20_000 {
                for x in 0..CITY_WIDTH {
                    for y in 0..CITY_HEIGHT {
                        state.city.set_terrain(x, y, 0x10);
                    }
                }
            }
            assert_eq!(
                state
                    .advance_frame_simulation(Duration::from_millis(17))
                    .days,
                1
            );
            assert_eq!(state.city.mac_pending_action(), 0);
            let expected_point = if growth == 20_000 { (16, 97) } else { (2, 82) };
            assert_eq!(state.mac_loop_scheduler.disaster_point, expected_point);
            assert!(!scenario_action::mac_scenario_action_active(&state.city));
            assert_eq!(
                state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
                Some(CITY_MODE_GAMEPLAY)
            );
            assert_eq!(
                state.city.mac_state_misc_u32_at(hurricane::FLOOD_DURATION),
                Some(0)
            );
            assert_eq!(state.mac_process_rng.seed(), seed);
            assert!(state.take_pending_disaster_extra_sounds().is_empty());
            for x in 0..CITY_WIDTH {
                for y in 0..CITY_HEIGHT {
                    assert_eq!(state.city.text_overlay_at(x, y), Some(0));
                }
            }
        }
    }
}

#[test]
fn mac_speed_changes_between_actor_and_calendar_admission() {
    let mut state = mac_phase24_fixture("mac-speed-event.city", WEATHER_TREND_BLIZZARD, 1);
    state.city.set_mac_pending_action(0);
    state.game_clock = GameClock::new_with_day(1910, 1, 2);
    state.mac_loop_scheduler.actor_deadline = 0;
    state.traffic.set_speed(GameSpeed::Turtle);
    let frame = Duration::from_millis(200);

    let waits = state.begin_mac_frame_actor_phase(frame);
    assert_eq!(state.mac_loop_scheduler.actor_deadline, 57);
    state
        .apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Paused))
        .unwrap();
    assert_eq!(
        state
            .finish_mac_frame_calendar_phase(Duration::ZERO, waits)
            .days,
        0
    );

    // Pass the actor deadline while still paused, then resume at the event.
    let waits = state.begin_mac_frame_actor_phase(Duration::from_secs(1));
    assert_eq!(state.mac_loop_scheduler.actor_deadline, 57);
    state
        .apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Turtle))
        .unwrap();
    assert_eq!(
        state
            .finish_mac_frame_calendar_phase(Duration::ZERO, waits)
            .days,
        1
    );
    assert_eq!(state.game_clock.day(), 3);
}

#[test]
fn mac_january_budget_keeps_saved_tax_bases_and_refreshes_bond_count() {
    for path in ["mac-annual-currents.city", "mac-annual-currents.sc2"] {
        let mut state = mac_phase24_fixture(path, WEATHER_TREND_BLIZZARD, 1);
        state.city.set_saved_date_and_day(
            crate::city::CityDate {
                year: 1910,
                month: 12,
            },
            24,
        );
        state.city.set_mac_state_misc_u32_at(0x18, 2);
        let tax = budget_row_misc_offset_for_city(&state.city, 0, 0).unwrap();
        let bonds = budget_row_misc_offset_for_city(&state.city, 4, 0).unwrap();
        state.city.set_budget_misc_u32_values(tax, &[4_380, 5, 900]);
        state.city.set_budget_misc_u32_values(bonds, &[1, 4, 0]);
        state.city.set_microsim_tile_id(18, 0xf3);
        state.city.set_microsim_stat0(18, u8::MAX);
        state.city.set_microsim_stat1(18, 1910);
        state.city.set_microsim_stat2(18, 12);
        state.city.set_microsim_stat3(18, 1);

        state.sim_calendar_day0_budget_and_date_at(1911, 1, 0);

        assert_eq!(
            state.city.budget_misc_u32_values(tax, 5),
            [4_380, 5, 21_900, 4_380, 5]
        );
        assert_eq!(state.city.budget_misc_u32_values(bonds, 5), [2, 4, 4, 1, 4]);
        let mayor = state.city.microsim_at(18).unwrap();
        assert_eq!(
            (mayor.stat0, mayor.stat1, mayor.stat2, mayor.stat3),
            (0, 1910, 0, 0)
        );
    }
}

#[test]
fn mac_road_bridge_modal_holds_simulation_and_cancel_keeps_the_road() {
    let mut city = City::new_blank("/tmp/citygame-mac-road-bridge.sc2", "Bridge").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
    city.set_city_funds(500);
    city.set_terrain(11, 10, 0x21);
    city.set_tile_bits(11, 10, TileBits::new(0).with_water_tile(true));

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Road;
    state.traffic.set_speed(GameSpeed::Turtle);
    state.begin_network_drag(10, 10, NetworkKind::Road);
    state.update_network_drag_to((11, 10));

    assert_eq!(
        state.release_left_mouse_drag(false).network,
        Some((NetworkKind::Road, 1))
    );
    assert!(matches!(
        state.city_info_dialog,
        Some(CityInfoDialog::RoadBridge { .. })
    ));
    let clock = state.game_clock;
    let calendar_deadline = state.mac_loop_scheduler.deadline;
    let actor_deadline = state.mac_loop_scheduler.actor_deadline;
    assert_eq!(
        state.advance_mac_frame_simulation(Duration::from_secs(2)),
        GameClockAdvance::default()
    );
    assert_eq!(state.game_clock, clock);
    assert_eq!(state.mac_loop_scheduler.deadline, calendar_deadline);
    assert_eq!(state.mac_loop_scheduler.actor_deadline, actor_deadline);

    state.close_city_info_dialog();
    assert_eq!(state.city.city_funds(), 490);
    assert!(NetworkKind::Road.contains(state.city.building_at(10, 10)));
    assert_eq!(state.city.building_at(11, 10), 0);
    assert!(state.city.tile_bits_at(11, 10).unwrap().water_tile());
    assert_eq!(state.advance_mac_frame_simulation(Duration::ZERO).days, 1);
}

#[test]
fn mac_rail_upkeep_first_enters_history_after_the_current_rebuild() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/citygame-mac-rail-budget.sc2", "Rail").unwrap();
        city.kind = kind;
        if kind.is_win95() {
            city.prepare_mac_fidelity_from_saved_city();
        }
        city.set_city_funds(500);
        let row = budget_row_misc_offset_for_city(&city, 13, 0).unwrap();
        city.set_budget_misc_u32_values(row, &[0, 100, 0]);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.tool = EditTool::Rail;
        state.begin_network_drag(10, 10, NetworkKind::Rail);
        state.update_network_drag_to((11, 10));

        assert_eq!(
            state.release_left_mouse_drag(false).network,
            Some((NetworkKind::Rail, 2))
        );
        assert_eq!(state.city.city_funds(), 450);
        state.sim_calendar_day21_demand_and_graphs();
        assert_eq!(state.city.budget_misc_u32_values(row, 3), [0, 100, 0]);

        state.sim_calendar_day0_budget_and_date_at(1900, 2, 0);
        assert_eq!(state.city.budget_misc_u32_values(row, 3), [2, 100, 0]);
        assert_eq!(state.city.budget_misc_u32_values(row, 7)[5..], [0, 100]);

        state.sim_calendar_day0_budget_and_date_at(1900, 3, 0);
        assert_eq!(state.city.budget_misc_u32_values(row, 3), [2, 100, 200]);
        assert_eq!(state.city.budget_misc_u32_values(row, 9)[7..], [2, 100]);
    }
}

#[test]
fn mac_power_wire_connects_to_powerable_lot_through_open_terrain_axis() {
    for (neighbor, slope, expected) in [
        ((11, 10), 0, 0x0f),
        ((11, 10), 2, 0x0e),
        ((10, 11), 0, 0x0e),
    ] {
        let mut city = City::new_blank("/tmp/citygame-mac-wire.sc2", "Power").unwrap();
        city.kind = CityFileKind::MacCity;
        city.set_city_funds(500);
        city.set_terrain(neighbor.0, neighbor.1, slope);
        city.set_building(neighbor.0, neighbor.1, 0xcf);
        city.set_tile_bits(neighbor.0, neighbor.1, TileBits::new(0x80));
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.tool = EditTool::Power;
        state.begin_network_drag(10, 10, NetworkKind::Power);
        assert_eq!(
            state.release_left_mouse_drag(false).network,
            Some((NetworkKind::Power, 1))
        );
        assert_eq!(state.city.building_at(10, 10), expected);
        assert!(state.city.tile_bits_at(10, 10).unwrap().powerable());
        assert_eq!(state.city.city_funds(), 498);
    }
}

#[test]
fn mac_pipe_stroke_reshapes_each_axis_and_preserves_existing_pipe_flags() {
    let mut city = City::new_blank("/tmp/citygame-mac-pipe.sc2", "Pipe").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(500);
    // Skip the post-route water scan to isolate the coordinate writer.
    city.set_mac_state_misc_u32_at(0x102c, 50_000);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Pipes;
    let pipes = UndergroundKind::Pipes;
    for (start, end, expected) in [((10, 10), (11, 10), 0x11), ((20, 20), (20, 21), 0x10)] {
        state.begin_underground_drag(start.0, start.1, pipes);
        state.update_underground_drag_to(end);
        assert_eq!(
            state.release_left_mouse_drag(false).underground,
            Some((pipes, 2))
        );
        assert_eq!(state.city.underground_at(start.0, start.1), expected);
        assert_eq!(state.city.underground_at(end.0, end.1), expected);
    }
    state.city.set_tile_bits(10, 10, TileBits::new(0));
    state.begin_underground_drag(10, 10, pipes);
    assert_eq!(
        state.release_left_mouse_drag(false).underground,
        Some((pipes, 0))
    );
    assert_eq!(state.city.tile_bits_at(10, 10).unwrap().raw_bits(), 0);
    assert_eq!(state.city.city_funds(), 485);
}

#[test]
fn mac_subway_stops_at_blocked_tile_and_preserves_pipe_crossover_flags() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/citygame-mac-subway.sc2", "Subway").unwrap();
        city.kind = kind;
        if kind.is_win95() {
            city.prepare_mac_fidelity_from_saved_city();
        }
        city.set_city_funds(500);
        city.set_underground(11, 10, 0x10);
        city.set_tile_bits(11, 10, TileBits::new(0x20));
        city.set_underground(12, 10, 0x22);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.tool = EditTool::Subway;
        let subway = UndergroundKind::Subway;
        state.begin_underground_drag(10, 10, subway);
        state.update_underground_drag_to((12, 10));

        assert_eq!(state.underground_drag_cost(), Some((subway, 200)));
        assert_eq!(
            state.release_left_mouse_drag(false).underground,
            Some((subway, 2))
        );
        assert_eq!(state.city.underground_at(10, 10), 2);
        assert_eq!(state.city.underground_at(11, 10), 0x20);
        assert_eq!(state.city.underground_at(12, 10), 0x22);
        assert_eq!(state.city.tile_bits_at(11, 10).unwrap().raw_bits(), 0x20);
        assert_eq!(state.city.city_funds(), 300);
    }
}

#[test]
fn mac_tunnel_quote_holds_the_map_and_charges_only_after_confirmation() {
    let mut city = City::new_blank("/tmp/citygame-mac-tunnel.sc2", "Tunnel").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_city_funds(450);
    for y in [10, 20] {
        for (x, terrain, word) in [(10, 3, 0x800c), (11, 0, 0x800d), (12, 1, 0x800b)] {
            city.set_terrain(x, y, terrain);
            city.set_altm_word(x, y, word);
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.tool = EditTool::Tunnel;
    assert!(!state.apply_tool_at_tile(10, 10));
    assert!(!state.apply_tool_at_tile(10, 20));
    let quote = CityInfoDialog::TunnelQuote {
        entrance: (10, 10),
        exit: (12, 10),
        cost: 450,
    };
    assert_eq!(state.city_info_dialog, Some(quote));
    assert_eq!(state.city.city_funds(), 450);
    assert_eq!(state.city.building_at(10, 10), 0);
    let window_size = (800, 600);
    let rect = super::super::dialog_layout::city_info_dialog_rect_for(quote, window_size);
    let button = super::super::dialog_layout::road_bridge_choice_rect(rect, 0);
    assert_eq!(
        super::super::dialog_input::city_info_dialog_hit_test(
            glam::Vec2::new(button.x + button.w * 0.5, button.y + button.h * 0.5),
            window_size,
            quote,
        ),
        Some(super::super::CityInfoDialogAction::ConfirmTunnel)
    );
    assert_eq!(state.accept_mac_tunnel_quote(), 3);
    assert_eq!(state.city.city_funds(), 0);
    assert_eq!(state.city.building_at(10, 10), 0x41);
    assert_eq!(state.city.building_at(12, 10), 0x3f);
    assert_eq!(state.city.altm_at(10, 10).tunnel_levels, 1);
    assert_eq!(state.city.altm_at(11, 10).tunnel_levels, 2);
    assert_eq!(state.city.altm_at(12, 10).tunnel_levels, 1);
    assert_eq!(state.accept_mac_tunnel_quote(), 0);
}

/// Compare original-instruction captures from the controlled TickCount
/// runner with the production frame entry. The original data stays local.
#[test]
fn mac_seven_spires_live_frame_differential() {
    let (Ok(input), Ok(planes)) = (
        std::env::var("CITYGAME_MAC_FRAME_CGMS"),
        std::env::var("CITYGAME_MAC_FRAME_PLANES"),
    ) else {
        return;
    };
    let input = std::fs::read(input).expect("read source CGMS");
    let root = std::path::Path::new(&planes);
    let native_actors = root.with_file_name("actors");
    let speed_events_path = root.join("speed_events.json");
    let speed_events: std::collections::BTreeMap<u64, u16> = if speed_events_path.is_file() {
        serde_json::from_slice(&std::fs::read(speed_events_path).unwrap()).unwrap()
    } else {
        Default::default()
    };
    let review_path = root.join("budget_review.json");
    let budget_review: Option<serde_json::Value> = review_path
        .is_file()
        .then(|| serde_json::from_slice(&std::fs::read(review_path).unwrap()).unwrap());
    let surface_event = [
        (NetworkKind::Road, "road_event.json"),
        (NetworkKind::Rail, "rail_event.json"),
        (NetworkKind::Power, "power_event.json"),
    ]
    .into_iter()
    .find_map(|(network, name)| {
        let path = root.join(name);
        path.is_file().then(|| {
            let event: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            (network, event)
        })
    });
    let underground_event = [
        (UndergroundKind::Pipes, "pipe_event.json"),
        (UndergroundKind::Subway, "subway_event.json"),
    ]
    .into_iter()
    .find_map(|(underground, name)| {
        let path = root.join(name);
        path.is_file().then(|| {
            let event: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            (underground, event)
        })
    });
    let zone_path = root.join("zone_event.json");
    let zone_event: Option<serde_json::Value> = zone_path
        .is_file()
        .then(|| serde_json::from_slice(&std::fs::read(zone_path).unwrap()).unwrap());
    let tunnel_path = root.join("tunnel_event.json");
    let tunnel_event: Option<serde_json::Value> = tunnel_path
        .is_file()
        .then(|| serde_json::from_slice(&std::fs::read(tunnel_path).unwrap()).unwrap());
    let single_tile_event = [
        (EditTool::SubToRail, "sub-to-rail_event.json"),
        (EditTool::Onramp, "onramp_event.json"),
        (EditTool::Bulldoze, "bulldoze_event.json"),
        (EditTool::BusDepot, "bus-depot_event.json"),
        (EditTool::RailDepot, "rail-depot_event.json"),
        (EditTool::SubStation, "subway-station_event.json"),
        (EditTool::Marina, "marina_event.json"),
        (EditTool::WaterPump, "water-pump_event.json"),
        (EditTool::WaterTower, "water-tower_event.json"),
        (EditTool::WaterTreatment, "water-treatment_event.json"),
        (EditTool::Desalinization, "desalination_event.json"),
        (EditTool::PowerPlantCoal, "power-coal_event.json"),
        (EditTool::PowerPlantHydro, "power-hydro_event.json"),
        (EditTool::PowerPlantOil, "power-oil_event.json"),
        (EditTool::PowerPlantGas, "power-gas_event.json"),
        (EditTool::PowerPlantNuclear, "power-nuclear_event.json"),
        (EditTool::PowerPlantWind, "power-wind_event.json"),
        (EditTool::PowerPlantSolar, "power-solar_event.json"),
        (EditTool::PowerPlantMicrowave, "power-microwave_event.json"),
        (EditTool::PowerPlantFusion, "power-fusion_event.json"),
        (EditTool::School, "school_event.json"),
        (EditTool::College, "college_event.json"),
        (EditTool::Library, "library_event.json"),
        (EditTool::Museum, "museum_event.json"),
        (EditTool::Police, "police_event.json"),
        (EditTool::FireStation, "fire-station_event.json"),
        (EditTool::Hospital, "hospital_event.json"),
        (EditTool::Prison, "prison_event.json"),
        (EditTool::SmallPark, "small-park_event.json"),
        (EditTool::BigPark, "big-park_event.json"),
        (EditTool::Zoo, "zoo_event.json"),
        (EditTool::Stadium, "stadium_event.json"),
        (EditTool::MayorHouse, "mayor-house_event.json"),
        (EditTool::CityHall, "city-hall_event.json"),
        (EditTool::Statue, "statue_event.json"),
        (EditTool::BraunLlamaDome, "braun-dome_event.json"),
        (EditTool::ArcologyPlymouth, "arcology-plymouth_event.json"),
        (EditTool::ArcologyForest, "arcology-forest_event.json"),
        (EditTool::ArcologyDarco, "arcology-darco_event.json"),
        (EditTool::ArcologyLaunch, "arcology-launch_event.json"),
    ]
    .into_iter()
    .find_map(|(tool, name)| {
        let path = root.join(name);
        path.is_file().then(|| {
            let event: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            (tool, event)
        })
    });
    let building_event = single_tile_event
        .as_ref()
        .is_some_and(|(tool, _)| !matches!(tool, EditTool::Onramp | EditTool::SubToRail));
    let highway_path = root.join("highway_event.json");
    let highway_event: Option<serde_json::Value> = highway_path
        .is_file()
        .then(|| serde_json::from_slice(&std::fs::read(highway_path).unwrap()).unwrap());
    let has_map_event = zone_event.is_some()
        || surface_event.is_some()
        || underground_event.is_some()
        || tunnel_event.is_some()
        || single_tile_event.is_some()
        || highway_event.is_some();
    let last_scan_phase = if has_map_event {
        (3..=24)
            .rev()
            .find(|phase| root.join(format!("phase-{phase:02}/frame.json")).is_file())
            .unwrap()
    } else {
        24
    };
    let annual_only = !root.join("phase-03/frame.json").is_file();
    let last_phase = if root.join("phase-02/frame.json").is_file() {
        2
    } else if root.join("phase-01/frame.json").is_file() {
        1
    } else {
        24
    };
    let phase_frames = (3..=last_scan_phase)
        .filter(|_| !annual_only)
        .chain(
            (last_phase < 3)
                .then_some(0..=last_phase)
                .into_iter()
                .flatten(),
        )
        .map(|phase| {
            let dir = root.join(format!("phase-{phase:02}"));
            let frame: serde_json::Value =
                serde_json::from_slice(&std::fs::read(dir.join("frame.json")).unwrap()).unwrap();
            (frame["iteration"].as_u64().unwrap(), (phase, dir, frame))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let (_, (first_phase, _, first_frame)) = phase_frames.first_key_value().unwrap();
    let tick_duration = |ticks: u64| Duration::from_nanos((ticks * 1_000_000_000).div_ceil(60));
    let frame_duration = tick_duration(first_frame["tick_stride"].as_u64().unwrap());
    let initial_duration = tick_duration(first_frame["tick_start"].as_u64().unwrap());
    let last_frame = *phase_frames.last_key_value().unwrap().0;
    let mut saw_independent_frames = false;
    let cgms_budget_start =
        20 + 9 * CITY_TILES + 2 * CITY_TILES + CITY_TILES + 1_200 * 4 + 0x077c;
    let has_saved_budget_book = input
        .get(cgms_budget_start..cgms_budget_start + 16 * 27 * 4)
        .is_some_and(|rows| rows.iter().any(|&value| value != 0));
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = crate::city::city_from_cgms_v4_for_test(&input).unwrap();
        let developed = city.mac_developed_cells().unwrap();
        city.kind = kind;
        if kind.is_win95() {
            city.prepare_mac_fidelity_from_saved_city();
        }
        for slot in 0..8 {
            city.set_mac_rci_counter(slot, 0);
        }
        city.set_mac_developed_cells(developed);
        for offset in [MISC_CITY_FUNDS_OFFSET, 0x1c, 0x50, 0x54, 0x58, 0x5c] {
            let saved = city.mac_state_misc_u32_at(offset).unwrap();
            city.set_mac_state_misc_u32_at(offset, saved);
        }
        city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
        let founded_year = city.mac_state_misc_u32_at(0x0c).unwrap() as u16;
        let elapsed = city.mac_state_misc_u32_at(0x10).unwrap();
        let initial_phase = (first_phase + 24) % 25;
        let clock = GameClock::from_city_age(founded_year, elapsed / 25 * 25 + initial_phase);
        let xmic_path = root.with_file_name("xmic.bin");
        if xmic_path.is_file() {
            let xmic = std::fs::read(xmic_path).unwrap();
            for (slot, record) in xmic.chunks_exact(8).enumerate() {
                city.set_microsim_tile_id(slot, record[0]);
                city.set_microsim_stat0(slot, record[1]);
                city.set_microsim_stat1(slot, u16::from_be_bytes([record[2], record[3]]));
                city.set_microsim_stat2(slot, u16::from_be_bytes([record[4], record[5]]));
                city.set_microsim_stat3(slot, u16::from_be_bytes([record[6], record[7]]));
            }
        }
        city.set_saved_date_and_day(
            crate::city::CityDate {
                year: clock.year(),
                month: clock.month(),
            },
            clock.day(),
        );
        if highway_event.is_some() || building_event {
            // The native harness restores CODE 13's placement compass from MISC.
            city.set_view_rotation(city.saved_rotation());
        }
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.traffic.set_speed(GameSpeed::Turtle);
        state.game_clock = clock;
        state.mac_process_rng = MacToolboxRng::new(1);
        state.mac_game_rng = MacGameLcgRng::new(1_103_527_590);
        let initial_tick = state.mac_loop_scheduler.advance_elapsed(initial_duration);
        state.mac_loop_scheduler.deadline = initial_tick;
        state.mac_loop_scheduler.actor_deadline = initial_tick;
        let n = CITY_TILES;
        let planes = [
            ("xter", 20, n),
            ("xbld", 20 + n, n),
            ("xzon", 20 + 2 * n, n),
            ("xbit", 20 + 3 * n, n),
            ("xund", 20 + 4 * n, n),
            ("xtxt", 20 + 5 * n, n),
            ("altm", 20 + 9 * n, 2 * n),
            ("xthg", 20 + 12 * n + 1_200 * 4, 480),
        ];
        for frame in 1..=last_frame {
            let actor_deadline = state.mac_loop_scheduler.actor_deadline;
            let mut advance = if speed_events.is_empty() && !has_map_event {
                state.advance_frame_simulation(frame_duration)
            } else {
                let waits = state.begin_mac_frame_actor_phase(frame_duration);
                if let Some(speed) = speed_events.get(&frame) {
                    let speed = match speed {
                        1 => GameSpeed::Paused,
                        2 => GameSpeed::Turtle,
                        3 => GameSpeed::Llama,
                        4 => GameSpeed::Cheetah,
                        _ => panic!("invalid native speed selector {speed}"),
                    };
                    state
                        .apply_state_app_command(&AppCommand::SetSpeed(speed))
                        .unwrap();
                }
                if let Some(event) = zone_event.as_ref()
                    && event["frame"].as_u64() == Some(frame)
                {
                    let x = event["x"].as_u64().unwrap() as usize;
                    let y = event["y"].as_u64().unwrap() as usize;
                    let zone_before = state.city.zone_at(x, y).unwrap().raw_bits();
                    assert_eq!(
                        u64::from(state.game_clock.day()),
                        event["phase_before"].as_u64().unwrap()
                    );
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_before"].as_i64().unwrap()
                    );
                    assert_eq!(
                        u64::from(zone_before),
                        event["zone_before"].as_u64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_before"].as_u64().unwrap()
                    );
                    let zone_id = event["zone_id"].as_u64().unwrap_or(1) as u8;
                    state.begin_zone_drag(x, y, zone_id);
                    assert_eq!(
                        state.zone_drag_cost(),
                        event["price"].as_u64().map(|price| price as u32)
                    );
                    assert_eq!(
                        state.release_left_mouse_drag(false).placed_zones,
                        event["written_cells"].as_array().unwrap().len()
                    );
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_after"].as_i64().unwrap()
                    );
                    let zone_after = state.city.zone_at(x, y).unwrap().raw_bits();
                    assert_eq!(
                        u64::from(zone_after),
                        event["zone_after"].as_u64().unwrap(),
                        "{kind:?} committed XZON"
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_after"].as_u64().unwrap()
                    );
                }
                if let Some(event) = highway_event.as_ref()
                    && event["frame"].as_u64() == Some(frame)
                {
                    let x = event["x"].as_u64().unwrap() as usize;
                    let y = event["y"].as_u64().unwrap() as usize;
                    let end: (usize, usize) = serde_json::from_value(event["end"].clone()).unwrap();
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_before"].as_i64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_before"].as_u64().unwrap()
                    );
                    state.tool = EditTool::Highway;
                    state.begin_highway_drag(x, y);
                    state.update_highway_drag_to(end);
                    let cost = event["price"].as_u64().unwrap();
                    assert_eq!(state.highway_drag_cost().map(u64::from).unwrap_or(0), cost);
                    let written: Vec<(usize, usize)> =
                        serde_json::from_value(event["written_cells"].clone()).unwrap();
                    if !written.is_empty() {
                        assert_eq!(state.highway_drag_path_for_city((x, y), end), written);
                    }
                    let outcome = state.release_left_mouse_drag(false);
                    assert_eq!(outcome.highway_segments, written.len());
                    let link = outcome
                        .highway_neighbor_link
                        .filter(|&candidate| highway_neighbor_link_is_available(&state, candidate));
                    if let Some(native) = event.get("neighbor_link") {
                        let candidate = link.expect("Mac highway edge-link offer");
                        let endpoint: (usize, usize) =
                            serde_json::from_value(native["endpoint"].clone()).unwrap();
                        assert_eq!(candidate.endpoint, endpoint);
                        assert_eq!(
                            state.city.mac_rci_overlay_cap_counts()[1],
                            native["count_before"].as_u64().unwrap() as u16
                        );
                        let accepted = native["accepted"].as_bool().unwrap();
                        assert_eq!(
                            apply_highway_neighbor_link_confirmation(
                                &mut state, candidate, accepted
                            ),
                            accepted
                        );
                        assert_eq!(
                            state.city.mac_rci_overlay_cap_counts()[1],
                            native["count_after"].as_u64().unwrap() as u16
                        );
                        assert_eq!(
                            state.city.text_overlay_at(endpoint.0, endpoint.1),
                            Some(native["xtxt_after"].as_u64().unwrap() as u8)
                        );
                    } else {
                        assert!(link.is_none());
                    }
                    if let Some(choice) = event.get("highway_bridge_choice") {
                        let entry = &event["bridge_entry"];
                        let candidate = HighwayWaterBridgeCandidate {
                            seed: serde_json::from_value(entry["seed"].clone()).unwrap(),
                            direction: choice["direction"].as_u64().unwrap() as u8,
                            span_count: choice["span_count"].as_u64().unwrap() as usize,
                            selector_mask: choice["selector_mask"].as_u64().unwrap() as u8,
                            mode: u8::from(entry["water_origin"].as_bool().unwrap()),
                        };
                        assert_eq!(
                            state.city_info_dialog,
                            Some(CityInfoDialog::HighwayWaterBridge { candidate })
                        );
                        assert_eq!(
                            state.finish_mac_frame_calendar_phase(Duration::ZERO, waits),
                            GameClockAdvance::default()
                        );
                        if let Some(selector) = choice["selector"].as_u64() {
                            let expected = if event["highway_bridge_notice"].is_null() {
                                candidate.span_count
                            } else {
                                0
                            };
                            assert_eq!(
                                state.finish_highway_water_bridge(candidate, selector as u8),
                                expected
                            );
                        }
                        state.close_city_info_dialog();
                    } else {
                        assert!(state.city_info_dialog.is_none());
                    }
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_after"].as_i64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_after"].as_u64().unwrap()
                    );
                }
                if let Some((tool, event)) = single_tile_event.as_ref()
                    && event["frame"].as_u64() == Some(frame)
                {
                    let x = event["x"].as_u64().unwrap() as usize;
                    let y = event["y"].as_u64().unwrap() as usize;
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_before"].as_i64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_before"].as_u64().unwrap()
                    );
                    state.tool = *tool;
                    let budget_offset = match tool {
                        EditTool::Police => Some(5),
                        EditTool::FireStation => Some(6),
                        EditTool::Hospital => Some(7),
                        EditTool::School => Some(8),
                        EditTool::College => Some(9),
                        _ => None,
                    }
                    .and_then(|row| budget_row_misc_offset_for_city(&state.city, row, 0));
                    let budget_current = |state: &ViewerState, offset| {
                        state.city.budget_misc_u32_values(offset, 1)[0] as u64
                    };
                    if let Some(offset) = budget_offset
                        && let Some(before) = event["budget_current_before"].as_u64()
                    {
                        assert_eq!(budget_current(&state, offset), before, "{kind:?} {tool:?}");
                    }
                    if let Some(before) = event["reward_latch_before"].as_u64() {
                        assert_eq!(
                            u64::from(state.city.mac_civic_latch()),
                            before,
                            "{kind:?} {tool:?}"
                        );
                    }
                    let expected = event["accepted"]
                        .as_bool()
                        .unwrap_or_else(|| !event["written_cells"].as_array().unwrap().is_empty());
                    assert_eq!(
                        state.apply_tool_at_tile(x, y),
                        expected,
                        "{kind:?} {tool:?}"
                    );
                    if *tool == EditTool::Bulldoze {
                        assert_eq!(
                            state.city.mac_state_misc_u32_at(0x1028).map(u64::from),
                            event["team_mask_after"].as_u64()
                        );
                        let final_stamps: std::collections::BTreeMap<_, _> = event["written_cells"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|stamp| {
                                let tx = stamp[0].as_u64().unwrap() as usize;
                                let ty = stamp[1].as_u64().unwrap() as usize;
                                let tile = stamp[2].as_u64().unwrap() as u8;
                                ((tx, ty), tile)
                            })
                            .collect();
                        for ((tx, ty), tile) in final_stamps {
                            assert_eq!(
                                state.city.building_at(tx, ty),
                                tile,
                                "{kind:?} ({tx},{ty})"
                            );
                        }
                    }
                    if let Some(after) = event["reward_latch_after"].as_u64() {
                        assert_eq!(
                            u64::from(state.city.mac_civic_latch()),
                            after,
                            "{kind:?} {tool:?}"
                        );
                    }
                    if *tool == EditTool::Stadium
                        && let Some(choices) = event["team_choices"].as_array()
                    {
                        assert_eq!(
                            state.city.mac_state_misc_u32_at(0x1028).map(u64::from),
                            event["team_mask_before"].as_u64()
                        );
                        for (i, choice) in choices.iter().enumerate() {
                            assert!(matches!(
                                state.city_info_dialog,
                                Some(CityInfoDialog::StadiumTeam { .. })
                            ));
                            assert_eq!(
                                state.finish_mac_stadium_team(choice.as_i64().unwrap() as u8),
                                i + 1 == choices.len()
                            );
                        }
                        assert!(state.city_info_dialog.is_none());
                        assert_eq!(
                            state.city.mac_state_misc_u32_at(0x1028).map(u64::from),
                            event["team_mask_after"].as_u64()
                        );
                    } else if *tool == EditTool::Stadium && event["team_chooser_reached"].is_null()
                    {
                        assert!(state.city_info_dialog.is_none());
                    }
                    if let Some(offset) = budget_offset
                        && let Some(after) = event["budget_current_after"].as_u64()
                    {
                        assert_eq!(budget_current(&state, offset), after, "{kind:?} {tool:?}");
                    }
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_after"].as_i64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.city.zone_at(x, y).unwrap().raw_bits()),
                        event["zone_after"].as_u64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_after"].as_u64().unwrap()
                    );
                }
                if let Some(event) = tunnel_event.as_ref()
                    && event["frame"].as_u64() == Some(frame)
                {
                    let entrance = (
                        event["x"].as_u64().unwrap() as usize,
                        event["y"].as_u64().unwrap() as usize,
                    );
                    let before = state.city.city_funds();
                    assert_eq!(i64::from(before), event["funds_before"].as_i64().unwrap());
                    state.tool = EditTool::Tunnel;
                    assert!(!state.apply_tool_at_tile(entrance.0, entrance.1));
                    assert_eq!(state.city.city_funds(), before);
                    if let Some(quote) = event.get("tunnel_quote") {
                        let exit: (usize, usize) =
                            serde_json::from_value(quote["exit"].clone()).unwrap();
                        let cost = event["price"].as_u64().unwrap() as u32;
                        assert_eq!(
                            state.city_info_dialog,
                            Some(CityInfoDialog::TunnelQuote {
                                entrance,
                                exit,
                                cost
                            })
                        );
                        assert_eq!(
                            state.finish_mac_frame_calendar_phase(Duration::ZERO, waits),
                            GameClockAdvance::default()
                        );
                        if quote["accepted"].as_bool().unwrap() {
                            let written = event["written_cells"].as_array().unwrap();
                            let distinct = written
                                .iter()
                                .enumerate()
                                .filter(|(index, tile)| !written[..*index].contains(tile))
                                .count();
                            assert_eq!(state.accept_mac_tunnel_quote(), distinct);
                        } else {
                            state.close_city_info_dialog();
                        }
                    }
                    assert!(state.city_info_dialog.is_none());
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_after"].as_i64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_after"].as_u64().unwrap()
                    );
                }
                if let Some((underground, event)) = underground_event.as_ref()
                    && event["frame"].as_u64() == Some(frame)
                {
                    let start = (
                        event["x"].as_u64().unwrap() as usize,
                        event["y"].as_u64().unwrap() as usize,
                    );
                    let end: (usize, usize) = serde_json::from_value(event["end"].clone()).unwrap();
                    let expected: Vec<(usize, usize)> =
                        serde_json::from_value(event["preview_cells"].clone()).unwrap();
                    let underground = *underground;
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_before"].as_i64().unwrap()
                    );
                    state.tool = match underground {
                        UndergroundKind::Pipes => EditTool::Pipes,
                        UndergroundKind::Subway => EditTool::Subway,
                    };
                    assert_eq!(
                        state.underground_drag_path_for_kind(start, end, underground, true),
                        expected,
                        "{kind:?} {underground:?} preview route"
                    );
                    state.begin_underground_drag(start.0, start.1, underground);
                    state.update_underground_drag_to(end);
                    assert_eq!(
                        state
                            .underground_drag_cost()
                            .map(|(_, cost)| u64::from(cost)),
                        event["price"].as_u64()
                    );
                    let outcome = state.release_left_mouse_drag(false);
                    assert_eq!(
                        outcome.underground,
                        Some((
                            underground,
                            event["written_cells"].as_array().unwrap().len()
                        ))
                    );
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_after"].as_i64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_after"].as_u64().unwrap()
                    );
                }
                if let Some(&(network, ref event)) = surface_event.as_ref()
                    && event["frame"].as_u64() == Some(frame)
                {
                    let x = event["x"].as_u64().unwrap() as usize;
                    let y = event["y"].as_u64().unwrap() as usize;
                    let end = (
                        event["end"][0].as_u64().unwrap() as usize,
                        event["end"][1].as_u64().unwrap() as usize,
                    );
                    assert_eq!(
                        u64::from(state.game_clock.day()),
                        event["phase_before"].as_u64().unwrap()
                    );
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_before"].as_i64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_before"].as_u64().unwrap()
                    );
                    let rail = network == NetworkKind::Rail;
                    state.tool = match network {
                        NetworkKind::Road => EditTool::Road,
                        NetworkKind::Rail => EditTool::Rail,
                        NetworkKind::Power => EditTool::Power,
                    };
                    state.begin_network_drag(x, y, network);
                    state.update_network_drag_to(end);
                    if let Some(native_path) = event.get("preview_cells") {
                        let expected: Vec<(usize, usize)> =
                            serde_json::from_value(native_path.clone()).unwrap();
                        assert_eq!(
                            state.network_drag_preview_path(state.network_drag.unwrap()),
                            expected,
                            "{kind:?} {network:?} preview route"
                        );
                    }
                    assert_eq!(
                        state.network_drag_cost().map(|(_, cost)| u64::from(cost)),
                        event["price"].as_u64(),
                        "{kind:?} {network:?} preview cost"
                    );
                    let outcome = state.release_left_mouse_drag(false);
                    let writes = event["written_cells"].as_array().unwrap();
                    let surface_writes = if event.get("bridge_entry").is_some() {
                        let visited = event["visited_cells"].as_array().unwrap();
                        writes.iter().filter(|tile| visited.contains(tile)).count()
                    } else {
                        writes.len()
                    };
                    assert_eq!(outcome.network, Some((network, surface_writes)));

                    let mut road_link = outcome.road_neighbor_link;
                    let mut rail_link = outcome.rail_neighbor_link;
                    if let Some(choice) = event.get("bridge_choice") {
                        let (seed_x, seed_y, direction, span_count, selector_mask) = match state
                            .city_info_dialog
                        {
                            Some(CityInfoDialog::RoadBridge {
                                seed_x,
                                seed_y,
                                direction,
                                span_count,
                                selector_mask,
                                mac_neighbor_link,
                            }) if network == NetworkKind::Road => {
                                road_link = mac_neighbor_link;
                                (seed_x, seed_y, direction, span_count, selector_mask)
                            }
                            Some(CityInfoDialog::RailBridge {
                                seed_x,
                                seed_y,
                                direction,
                                span_count,
                                neighbor_link,
                            }) if rail => {
                                rail_link = neighbor_link;
                                (seed_x, seed_y, direction, span_count, 2)
                            }
                            Some(CityInfoDialog::PowerBridge {
                                seed_x,
                                seed_y,
                                direction,
                                span_count,
                                ..
                            }) if network == NetworkKind::Power => {
                                (seed_x, seed_y, direction, span_count, 1)
                            }
                            _ => panic!("{kind:?} expected the native bridge chooser: {choice}"),
                        };
                        let seed: (usize, usize) =
                            serde_json::from_value(event["bridge_entry"]["seed"].clone()).unwrap();
                        assert_eq!((seed_x, seed_y), seed);
                        assert_eq!(u64::from(direction), choice["direction"].as_u64().unwrap());
                        assert_eq!(span_count as u64, choice["span_count"].as_u64().unwrap());
                        assert_eq!(
                            u64::from(selector_mask),
                            choice["selector_mask"].as_u64().unwrap()
                        );
                        assert_eq!(
                            i64::from(state.city.city_funds()),
                            choice["funds_before"].as_i64().unwrap()
                        );
                        assert_eq!(
                            state.finish_mac_frame_calendar_phase(Duration::ZERO, waits),
                            GameClockAdvance::default()
                        );

                        if let Ok(selector) = u8::try_from(choice["selector"].as_i64().unwrap()) {
                            let placed = match network {
                                NetworkKind::Road => {
                                    state.finish_road_bridge(seed, direction, span_count, selector)
                                }
                                NetworkKind::Rail => {
                                    state.finish_rail_bridge(seed, direction, span_count)
                                }
                                NetworkKind::Power => {
                                    state.finish_power_bridge(seed, direction, span_count)
                                }
                            };
                            let link_charge =
                                if event["neighbor_link"]["accepted"].as_bool() == Some(true) {
                                    if rail { 1_500 } else { 1_000 }
                                } else {
                                    0
                                };
                            let bridge_funds = event["funds_after"].as_i64().unwrap() + link_charge;
                            assert_eq!(i64::from(state.city.city_funds()), bridge_funds);
                            assert_eq!(
                                placed,
                                if choice["funds_before"].as_i64() == Some(bridge_funds) {
                                    0
                                } else {
                                    span_count.max(2)
                                }
                            );
                        }
                        state.close_city_info_dialog();
                    } else {
                        assert!(!matches!(
                            state.city_info_dialog,
                            Some(
                                CityInfoDialog::RoadBridge { .. }
                                    | CityInfoDialog::RailBridge { .. }
                                    | CityInfoDialog::PowerBridge { .. }
                            )
                        ));
                    }
                    if let Some(link) = event.get("neighbor_link") {
                        let endpoint: (usize, usize) =
                            serde_json::from_value(link["endpoint"].clone()).unwrap();
                        let offered = if rail {
                            rail_link.map(|candidate| candidate.endpoint)
                        } else {
                            road_link.map(|candidate| candidate.endpoint)
                        };
                        assert_eq!(offered, Some(endpoint));
                        let bucket = usize::from(rail);
                        let (x, y) = endpoint;
                        assert_eq!(
                            u64::from(state.city.text_overlay_at(x, y).unwrap_or(0)),
                            link["xtxt_before"].as_u64().unwrap()
                        );
                        assert_eq!(
                            u64::from(state.city.mac_rci_overlay_cap_counts()[bucket]),
                            link["count_before"].as_u64().unwrap()
                        );
                        let accepted = link["accepted"].as_bool().unwrap();
                        let paid = if rail {
                            apply_rail_neighbor_link_confirmation(
                                &mut state,
                                rail_link.unwrap(),
                                accepted,
                            )
                        } else {
                            apply_road_neighbor_link_confirmation(
                                &mut state,
                                road_link.unwrap(),
                                accepted,
                            )
                        };
                        assert_eq!(paid, accepted);
                        assert_eq!(
                            u64::from(state.city.text_overlay_at(x, y).unwrap_or(0)),
                            link["xtxt_after"].as_u64().unwrap()
                        );
                        assert_eq!(
                            u64::from(state.city.mac_rci_overlay_cap_counts()[bucket]),
                            link["count_after"].as_u64().unwrap()
                        );
                    } else {
                        assert!(road_link.is_none_or(|candidate| {
                            !road_neighbor_link_is_available(&state, candidate)
                        }));
                        assert!(rail_link.is_none_or(|candidate| {
                            !rail_neighbor_link_is_available(&state, candidate)
                        }));
                    }
                    assert_eq!(
                        i64::from(state.city.city_funds()),
                        event["funds_after"].as_i64().unwrap()
                    );
                    assert_eq!(
                        u64::from(state.mac_process_rng.seed()),
                        event["quickdraw_after"].as_u64().unwrap()
                    );
                }
                state.finish_mac_frame_calendar_phase(Duration::ZERO, waits)
            };
            if let Some(review) = budget_review.as_ref()
                && phase_frames.get(&frame).is_some_and(|entry| entry.0 == 0)
            {
                assert_eq!(advance, GameClockAdvance::default());
                assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
                assert_eq!(
                    state.game_clock,
                    GameClock::from_city_age(founded_year, review["age"].as_u64().unwrap() as u32)
                );
                assert_eq!(
                    i64::from(state.city.city_funds()),
                    review["funds"].as_i64().unwrap()
                );
                assert_eq!(
                    u64::from(state.mac_process_rng.seed()),
                    review["quickdraw_seed"].as_u64().unwrap()
                );
                assert_eq!(
                    u64::from(budget_service_funding_raw_for_city(&state.city, 0)),
                    review["funding_before"].as_u64().unwrap()
                );
                assert!(state.adjust_budget_service_funding(0, -5));
                assert_eq!(
                    u64::from(budget_service_funding_raw_for_city(&state.city, 0)),
                    review["funding_after"].as_u64().unwrap()
                );
                state.close_city_info_dialog();
                advance = state.advance_frame_simulation(Duration::ZERO);
            }
            let actor_snapshot = native_actors.join(format!("iteration-{frame:02}.json"));
            let actor_due = state.mac_loop_scheduler.actor_deadline != actor_deadline;
            let calendar = phase_frames.get(&frame);
            assert_eq!(
                advance.days,
                u32::from(calendar.is_some()),
                "{kind:?} frame {frame}"
            );
            saw_independent_frames |= actor_due && calendar.is_none();
            assert_eq!(
                actor_due,
                actor_snapshot.is_file(),
                "{kind:?} frame {frame}"
            );
            if actor_due {
                let native: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&actor_snapshot).unwrap()).unwrap();
                assert_eq!(
                    native["actor_entries"][0]["actor_deadline"].as_u64(),
                    Some(u64::from(state.mac_loop_scheduler.actor_deadline)),
                    "{kind:?} frame {frame} actor deadline"
                );
                if calendar.is_none() {
                    assert_eq!(
                        native["quickdraw_seed_after"].as_u64(),
                        Some(u64::from(state.mac_process_rng.seed())),
                        "{kind:?} actor-only frame {frame} QuickDraw"
                    );
                    let data = crate::city::cgms_v4_from_city_for_test(&state.city).unwrap();
                    let xthg_start = 20 + 12 * n + 1_200 * 4;
                    assert!(
                        &data[xthg_start..]
                            == std::fs::read(
                                actor_snapshot
                                    .with_file_name(format!("iteration-{frame:02}-xthg.bin"))
                            )
                            .unwrap(),
                        "{kind:?} actor-only frame {frame} XTHG"
                    );
                    assert!(
                        &data[20 + 5 * n..20 + 6 * n]
                            == std::fs::read(
                                actor_snapshot
                                    .with_file_name(format!("iteration-{frame:02}-xtxt.bin"))
                            )
                            .unwrap(),
                        "{kind:?} actor-only frame {frame} XTXT"
                    );
                }
            }
            let Some((phase, phase_dir, native_frame)) = calendar else {
                continue;
            };
            assert_eq!(u32::from(state.game_clock.day()), *phase);
            assert_eq!(advance.months, u32::from(*phase == 0));
            for (name, actual) in [
                ("calendar_deadline", state.mac_loop_scheduler.deadline),
                ("quickdraw_seed", state.mac_process_rng.seed()),
                ("game_lcg_seed", state.mac_game_rng.state()),
            ] {
                assert_eq!(
                    native_frame[name].as_u64(),
                    Some(u64::from(actual)),
                    "{kind:?} phase {phase} {name}"
                );
            }
            let data = crate::city::cgms_v4_from_city_for_test(&state.city).unwrap();
            for (name, offset, len) in planes {
                let plane_path = phase_dir.join(format!("{name}.bin"));
                if name == "xter" && !plane_path.is_file() {
                    continue;
                }
                let native = std::fs::read(plane_path).unwrap();
                let actual = &data[offset..offset + len];
                let diff =
                    actual
                        .iter()
                        .zip(&native)
                        .enumerate()
                        .find_map(|(i, (actual, expected))| {
                            (actual != expected).then_some((i, *actual, *expected))
                        });
                assert_eq!(actual.len(), native.len(), "{name} plane length");
                assert_eq!(
                    diff,
                    None,
                    "{kind:?} phase {phase} {name} first differing offset (QuickDraw {})",
                    state.mac_process_rng.seed()
                );
            }
            let native = std::fs::read(phase_dir.join("xtrf.bin")).unwrap();
            let traffic = &data[20 + 11 * n..20 + 12 * n];
            let diff = traffic.iter().enumerate().position(|(i, &value)| {
                value != native[(i / CITY_HEIGHT / 2) * 64 + (i % CITY_HEIGHT) / 2]
            });
            assert_eq!(diff, None, "{kind:?} phase {phase} XTRF");
            if building_event && matches!(*phase, 1 | 3) {
                let globals = std::fs::read(phase_dir.join("app_globals.bin")).unwrap();
                for (offset, actual) in [
                    (0xf00a, state.city.mac_power_service_summary()),
                    (0xf00e, state.city.mac_water_service_summary()),
                ] {
                    let expected =
                        u32::from_be_bytes(globals[offset..offset + 4].try_into().unwrap());
                    assert_eq!(
                        actual, expected,
                        "{kind:?} phase {phase} service {offset:#x}"
                    );
                }
            }
            let xmic_path = phase_dir.join(if building_event {
                "live_xmic.bin"
            } else {
                "budget_xmic.bin"
            });
            if building_event || (*phase == 0 && xmic_path.is_file()) {
                let xmic = std::fs::read(xmic_path).unwrap();
                assert_eq!(xmic.len(), crate::city::MICROSIM_COUNT * 8);
                for (slot, record) in xmic.chunks_exact(8).enumerate() {
                    let actual = state.city.microsim_at(slot).unwrap();
                    let [tile_id, stat0, s1_hi, s1_lo, s2_hi, s2_lo, s3_hi, s3_lo] =
                        <[u8; 8]>::try_from(record).unwrap();
                    assert_eq!(
                        (
                            actual.tile_id,
                            actual.stat0,
                            actual.stat1,
                            actual.stat2,
                            actual.stat3,
                        ),
                        (
                            tile_id,
                            stat0,
                            u16::from_be_bytes([s1_hi, s1_lo]),
                            u16::from_be_bytes([s2_hi, s2_lo]),
                            u16::from_be_bytes([s3_hi, s3_lo]),
                        ),
                        "{kind:?} phase {phase} XMIC slot {slot}"
                    );
                }
            }
            if *phase == 2 {
                for (name, source, scale) in [
                    ("xval", state.city.land_value_slice(), 2),
                    ("xcrm", state.city.crime_density_slice(), 2),
                    ("xplt", state.city.pollution_density_slice(), 2),
                    ("xplc", state.city.police_coverage_slice(), 4),
                    ("xfir", state.city.fire_coverage_slice(), 4),
                    ("xpop", state.city.population_density_slice(), 4),
                ] {
                    let source = source.unwrap();
                    let native = std::fs::read(phase_dir.join(format!("{name}.bin"))).unwrap();
                    let side = CITY_WIDTH / scale;
                    assert_eq!(native.len(), side * side);
                    let diff = native.iter().enumerate().find_map(|(i, &expected)| {
                        let x = (i / side) * scale;
                        let y = (i % side) * scale;
                        let actual = source[x * CITY_HEIGHT + y];
                        (actual != expected).then_some((x, y, actual, expected))
                    });
                    assert_eq!(diff, None, "{kind:?} phase 2 {name} (x, y, Rust, native)");
                }
            }
            let event_tool = single_tile_event.as_ref().map(|(tool, _)| *tool);
            if *phase == 3
                && (underground_event.is_some()
                    || matches!(event_tool, Some(EditTool::SubStation | EditTool::WaterPump)))
            {
                let globals = std::fs::read(phase_dir.join("app_globals.bin")).unwrap();
                assert_eq!(
                    state.city.mac_xund_class_count(),
                    u16::from_be_bytes(globals[0xf082..0xf084].try_into().unwrap()),
                    "{kind:?} XUND census after the underground tool"
                );
            }
            if *phase == 3 && event_tool == Some(EditTool::RailDepot) {
                let globals = std::fs::read(phase_dir.join("app_globals.bin")).unwrap();
                assert_eq!(
                    state.city.mac_rail_actor_counter,
                    u16::from_be_bytes(globals[0xa09e..0xa0a0].try_into().unwrap()),
                    "{kind:?} rail actor census after depot placement"
                );
            }
            if has_map_event && (3..=19).contains(phase) {
                let counters = std::fs::read(phase_dir.join("counters.bin")).unwrap();
                for (slot, expected) in counters.chunks_exact(4).enumerate() {
                    assert_eq!(
                        state.city.mac_rci_counter(slot),
                        u32::from_be_bytes(expected.try_into().unwrap()),
                        "{kind:?} phase {phase} counter {slot}"
                    );
                }
            }
            if matches!(*phase, 0 | 21) {
                let globals = std::fs::read(phase_dir.join("app_globals.bin")).unwrap();
                let read_long =
                    |offset| u32::from_be_bytes(globals[offset..offset + 4].try_into().unwrap());
                assert_eq!(state.city.city_age_days(), Some(read_long(0xefa4)));
                if *phase == 0 {
                    assert_eq!(state.city.city_funds(), read_long(0xefac) as i32);
                    assert_eq!(
                        mac_auto_budget_enabled_for_city(&state.city),
                        globals[0xd606] != 0,
                        "{kind:?} phase 0 Auto-Budget"
                    );
                    assert_eq!(
                        state.city.ordinance_flags_for_saved_layout(),
                        read_long(0xeff2)
                    );
                    let counters = std::fs::read(phase_dir.join("counters.bin")).unwrap();
                    for (slot, expected) in counters.chunks_exact(4).enumerate() {
                        assert_eq!(
                            state.city.mac_rci_counter(slot),
                            u32::from_be_bytes(expected.try_into().unwrap()),
                            "{kind:?} phase 0 counter {slot}"
                        );
                    }
                    if has_saved_budget_book {
                        let budget = std::fs::read(phase_dir.join("budget_book.bin")).unwrap();
                        for (index, row) in budget.chunks_exact(0x70).enumerate() {
                            let offset =
                                budget_row_misc_offset_for_city(&state.city, index, 0).unwrap();
                            let actual = state.city.budget_misc_u32_values(offset, 27);
                            let row_long = |offset| {
                                u32::from_be_bytes(row[offset..offset + 4].try_into().unwrap())
                            };
                            assert_eq!(
                                &actual[..3],
                                &[row_long(0x60), row_long(0x64), row_long(0x68)],
                                "{kind:?} phase 0 budget row {index} current/funding/accrual"
                            );
                            for month in 0..12 {
                                assert_eq!(
                                    &actual[3 + month * 2..5 + month * 2],
                                    &[row_long(month * 4), row_long(0x30 + month * 4)],
                                    "{kind:?} phase 0 budget row {index}, history month {month}"
                                );
                            }
                        }
                    }
                    continue;
                }
                let demand = std::array::from_fn::<_, 3, _>(|i| {
                    let offset = 0xefc0 + i * 2;
                    i32::from(i16::from_be_bytes(
                        globals[offset..offset + 2].try_into().unwrap(),
                    ))
                });
                assert_eq!(state.city.misc_i32_values(0x718, 3), demand);
                let native = std::fs::read(phase_dir.join("cohort_lanes.bin")).unwrap();
                for (i, actual) in state.city.misc_u32_values(0x7c, 60).into_iter().enumerate() {
                    let offset = (i % 3) * 0x1000 + (i / 3) * 4;
                    let expected =
                        u32::from_be_bytes(native[offset..offset + 4].try_into().unwrap());
                    assert_eq!(actual, expected, "{kind:?} cohort dword {i}");
                }
            }
        }
        assert_eq!(
            u32::from(state.game_clock.day()),
            if annual_only {
                last_phase
            } else {
                last_scan_phase
            }
        );
        assert_eq!(state.city.mac_pending_action(), 0);
        assert!(!scenario_action::mac_scenario_action_active(&state.city));
    }
    if !annual_only && last_scan_phase == 24 && first_frame["tick_stride"].as_u64() == Some(10) {
        assert!(
            saw_independent_frames,
            "the ten-tick schedule must include actor-only frames"
        );
    }
}

#[test]
fn mac_dispatch_response_matches_native_action_lifetime_on_both_layouts() {
    use crate::win95_overlay::{DispatchKind, Win95OverlayRecord};

    for path in ["mac-service-response.city", "mac-service-response.sc2"] {
        let mut base = blank_city_layout_fixture(path, "Dispatch response");
        base.prepare_mac_fidelity_from_saved_city();
        for x in 100..116 {
            base.set_building(x, 100, 210);
            base.set_building(x, 101, 211);
        }
        base.set_mac_state_misc_u32_at(915 * 4, 3);
        let stale = Win95OverlayRecord::from_bytes(&[
            0, 0x93, 0x99, 0x77, 0x78, 0x77, 0x79, 0x7a, 0x51, 0x52, 0x53, 0x5a,
        ]);
        base.set_mac_object_slot_record(1, stale);

        // Native CODE 3 placement joined to CODE 10 export 246. Each row
        // gives the ending QuickDraw seed for the first and (if needed) last pass.
        for (kind, kind_byte, riot, seed_pairs) in [
            (
                DispatchKind::Police,
                7,
                0xfe,
                [
                    (677_268_843, 0),
                    (669_281_627, 1_847_779_995),
                    (677_268_843, 1_259_501_992),
                ],
            ),
            (
                DispatchKind::Firefighters,
                8,
                0,
                [
                    (168_070, 0),
                    (959_744_092, 669_281_627),
                    (168_070, 677_268_843),
                ],
            ),
            (
                DispatchKind::Military,
                14,
                0xfd,
                [
                    (677_268_843, 0),
                    (669_281_627, 1_847_779_995),
                    (677_268_843, 1_259_501_992),
                ],
            ),
        ] {
            for (case, (first_seed, last_seed)) in seed_pairs.into_iter().enumerate() {
                let mut city = base.clone();
                let fidelity = EngineFidelity::Mac68k;
                city.reset_emergency_dispatch_for_engine(fidelity);
                assert!(city.place_dispatch_actor_for_engine(45, 40, kind, fidelity));

                let fire_x = if case == 1 { 44 } else { 46 };
                let mut rng = MacToolboxRng::new(if case == 1 { 11 } else { 10 });
                city.set_text_overlay(fire_x, 40, 0xff);
                city.set_text_overlay(45, 41, riot);
                if case == 2 {
                    city.mac_object_slot_records_mut()[39].kind = 6;
                }
                scenario_action::begin_action(&mut city, scenario_action::OAKLAND_ACTION);

                let mut check_pass = |city: &mut City, active, seed| {
                    let pass = scenario_action::advance_action(city, &mut rng, &mut 0, &mut 0);
                    assert_eq!(rng.seed(), seed, "{path} {kind:?} case {case}");
                    assert_eq!(pass.complete, active == 0);
                    assert!(!pass.riot_sound);
                    assert_eq!(city.mac_state_misc_u32_at(0x1044), Some(12));
                    assert_eq!(city.mac_state_misc_u32_at(0x1048), Some(active));
                    assert_eq!(
                        city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
                        Some(if active == 0 {
                            CITY_MODE_GAMEPLAY
                        } else {
                            CITY_MODE_DISASTER
                        })
                    );
                    assert_eq!(
                        city.text_overlay_at(45, 40),
                        Some(if active == 0 { 0 } else { 0xca })
                    );
                    assert_eq!(city.text_overlay_at(fire_x, 40), Some(0));
                    assert_eq!(city.text_overlay_at(45, 41), Some(0));
                    assert_eq!(city.building_at(fire_x, 40), 1);
                    assert_eq!(
                        city.traffic_overlays().unwrap()[1].to_bytes(),
                        [
                            if active == 0 { 0 } else { kind_byte },
                            0,
                            0x99,
                            45,
                            40,
                            0,
                            0,
                            0,
                            0x51,
                            0x52,
                            0,
                            0x5a,
                        ],
                    );
                };
                check_pass(&mut city, [0, 1, 0xff][case], first_seed);
                if last_seed != 0 {
                    city.mac_object_slot_records_mut()[39].kind = 0;
                    check_pass(&mut city, 0, last_seed);
                }
            }
        }
    }
}

#[test]
fn mac_dispatchers_compete_for_a_live_fire_marker_in_x_major_order() {
    use crate::win95_overlay::DispatchKind;

    for path in [
        "mac-dispatch-contention.city",
        "mac-dispatch-contention.sc2",
    ] {
        let mut city = blank_city_layout_fixture(path, "Dispatch contention");
        city.prepare_mac_fidelity_from_saved_city();
        for x in 100..116 {
            city.set_building(x, 100, 210);
            city.set_building(x, 101, 211);
        }
        city.set_mac_state_misc_u32_at(915 * 4, 3);

        let fidelity = EngineFidelity::Mac68k;
        city.reset_emergency_dispatch_for_engine(fidelity);
        assert!(
            city.place_dispatch_actor_for_engine(45, 40, DispatchKind::Firefighters, fidelity,)
        );
        assert!(
            city.place_dispatch_actor_for_engine(47, 40, DispatchKind::Firefighters, fidelity,)
        );
        city.set_text_overlay(46, 40, 0xff);
        scenario_action::begin_action(&mut city, scenario_action::OAKLAND_ACTION);

        // The first QuickDraw value is 2, selecting +X. The first responder
        // clears the fire before the map scan reaches it; the later proxy
        // still consumes its own direction draw before action cleanup.
        let mut rng = MacToolboxRng::new(10);
        let mut expected_rng = MacToolboxRng::new(10);
        expected_rng.random_u16();
        expected_rng.random_u16();
        let pass = scenario_action::advance_action(&mut city, &mut rng, &mut 0, &mut 0);

        assert!(pass.complete, "{path}");
        assert!(!pass.riot_sound, "{path}");
        assert_eq!(rng.seed(), expected_rng.seed(), "{path}");
        assert_eq!(city.text_overlay_at(45, 40), Some(0));
        assert_eq!(city.text_overlay_at(46, 40), Some(0));
        assert_eq!(city.text_overlay_at(47, 40), Some(0));
        assert_eq!(city.building_at(46, 40), 1);
        let actors = city.traffic_overlays().unwrap();
        assert_eq!(actors[1].kind, 0, "{path} first responder cleanup");
        assert_eq!(actors[2].kind, 0, "{path} later responder cleanup");
        assert_eq!(city.mac_state_misc_u32_at(0x1048), Some(0));
        assert_eq!(
            city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(CITY_MODE_GAMEPLAY),
        );
    }
}
