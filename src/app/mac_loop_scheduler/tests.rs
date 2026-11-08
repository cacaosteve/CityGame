use super::*;
use crate::app::tests::test_state;

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
fn mac_rail_head_dispatch_is_engine_scoped_not_city_layout_scoped() {
    for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
        let mut city = City::new_blank(
            std::path::Path::new("/tmp/mac-rail-sweep-cross-layout.sc2"),
            "MacRailSweep",
        )
        .expect("blank city");
        city.kind = kind;
        let slot = XTHG_FIRST_USABLE_SLOT;
        city.set_building(50, 49, 0xed);
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
    assert_eq!(outcomes[1].4, Some(0));
    assert_eq!(outcomes[1].5, Some(0));
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
fn mac_kind5_state_three_clears_proxy_and_uses_random_parity_on_both_layouts() {
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
    assert_eq!(outcomes[0].0.kind, 5, "state 3 does not retire the record");
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
        for status in 0..=3u8 {
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
                city.microsim_at(4).map(|record| record.tile_id),
            ));
        }
    }

    assert_eq!(&outcomes[..4], &outcomes[4..]);
    assert_eq!(outcomes[0].1, 6, "status 0 does not replace XBLD");
    assert_eq!(outcomes[0].4, Some(0xff), "status 0 claims the candidate");
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
    assert_eq!(outcomes[3].10, Some(0xc8));
    for outcome in &outcomes {
        assert_eq!(outcome.5, 1, "seed 1's first Random % 25 is nonzero");
        assert_eq!(outcome.6, expected_direction);
        assert_ne!(outcome.7 & 0x80, 0, "nonzero-status epilogue/success gate");
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
    assert_eq!((outcomes[0].0.lane, outcomes[0].0.state), (0, 5));
    assert_eq!(outcomes[0].1, Some(6));
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
fn mac_kind_one_phase_one_lands_on_runway_or_leaves_marker_across_city_layouts() {
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
    assert_eq!((field.1.kind, field.1.state, field.1.lane), (1, 5, 0));
    assert_eq!(
        (field.1.height, field.1.saved_a, field.1.saved_b),
        (0, 0x33, 1)
    );
    assert_eq!((field.2, field.3), (Some(0x22), Some(6)));
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
            city.text_overlay_at(41, 39),
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
            41,
            39,
            Some(0x22),
            Some(proxy),
            vec![crate::audio::SOUND_MAYDAY]
        )
    );
    assert_eq!(
        (
            phase_seven_done[0].0.lane,
            phase_seven_done[0].0.state,
            phase_seven_done[0].0.saved_b,
            phase_seven_done[0].1,
        ),
        (0, 5, 1, Some(6))
    );
    assert!(phase_seven_done[0].2.is_empty());
    assert_eq!(
        (
            high_xbld[0].0.lane,
            high_xbld[0].0.state,
            high_xbld[0].0.saved_b,
            high_xbld[0].0.tile_x,
            high_xbld[0].0.tile_y,
            high_xbld[0].1,
            high_xbld[0].2,
            high_xbld[0].3,
        ),
        (0, 5, 1, 44, 42, Some(6), high_xbld[0].3, high_xbld[0].3,)
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
        &mut quickdraw
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
        &mut quickdraw
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
            &mut quickdraw
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
            &mut quickdraw
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
            &mut quickdraw
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
            &mut quickdraw
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
