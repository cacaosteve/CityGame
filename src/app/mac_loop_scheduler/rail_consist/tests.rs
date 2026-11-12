use super::*;
use crate::city::CityFileKind;

fn record(hex: &str) -> Win95OverlayRecord {
    assert_eq!(hex.len(), 24);
    let bytes: Vec<u8> = (0..12)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        .collect();
    Win95OverlayRecord::from_bytes(&bytes)
}

#[test]
fn mac_rail_full_handler_matches_original_68020() {
    let fixture =
        include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_full_handler.csv");
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        verify_mac_rail_full_handler(fixture, false, kind);
    }
}

#[test]
fn mac_rail_full_handler_preserves_native_game_lcg() {
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_rng.csv");
    verify_mac_rail_full_handler(fixture, true, CityFileKind::WinSc2);
}

fn verify_mac_rail_full_handler(fixture: &str, native_rng: bool, city_kind: CityFileKind) {
    let mut city = City::new_blank("/tmp/mac-rail-handler.sc2", "Rail").unwrap();
    city.kind = city_kind;
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let mode: usize = v[0].parse().unwrap();
        let kind: u8 = v[1].parse().unwrap();
        let station_draw: u16 = v[2].parse().unwrap();
        let route_draw: u16 = v[3].parse().unwrap();
        let quick: u16 = v[4].parse().unwrap();
        let mask: u8 = v[5].parse().unwrap();
        for row in 27..33 {
            for col in 49..52 {
                city.set_building(row, col, 0);
                city.set_underground(row, col, 0);
                let i = (row - 27) * 3 + col - 49;
                city.set_text_overlay(
                    row,
                    col,
                    u8::from_str_radix(&v[7][i * 2..i * 2 + 2], 16).unwrap(),
                );
            }
        }
        let (row, col) = [
            (31_i16, 50_i16),
            (31, 50),
            (31, 50),
            (30, 50),
            (28, 50),
            (31, 50),
        ][mode];
        for (i, (dr, dc)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
            if mask & (1 << i) != 0 {
                city.set_building((row + dr) as usize, (col + dc) as usize, 0x2c);
                city.set_underground((row + dr) as usize, (col + dc) as usize, 1);
            }
        }
        city.set_building(30, 50, if mode == 1 { 0 } else { 0x2c });
        city.set_underground(30, 50, 0);
        if mode == 0 {
            city.set_building(if station_draw == 0 { 29 } else { 31 }, 50, 0xed);
        }
        for slot in 0..40 {
            city.set_mac_object_slot_record(slot, record(&v[6][slot * 24..slot * 24 + 24]));
        }
        let mut app_calls = 0;
        let mut quick_calls = 0;
        let mut game_rng = crate::mac_random::MacGameLcgRng::new(if native_rng {
            v[14].parse().unwrap()
        } else {
            1
        });
        let (outcome, sound) = mac_rail_handler(
            &mut city,
            5,
            || {
                let value = if native_rng {
                    game_rng.bounded(2)
                } else if kind == 10 && app_calls == 0 {
                    station_draw
                } else {
                    route_draw
                };
                app_calls += 1;
                value
            },
            || {
                quick_calls += 1;
                quick
            },
        )
        .unwrap();
        let expected = match v[8] {
            "station" => MacRailPrefixOutcome::Station,
            "retired" => MacRailPrefixOutcome::Retired,
            "blocked" => MacRailPrefixOutcome::Blocked,
            "moved" => MacRailPrefixOutcome::Moved,
            _ => panic!("bad fixture"),
        };
        assert_eq!(outcome, expected, "case {count}");
        assert_eq!(app_calls, v[9].parse::<usize>().unwrap(), "case {count}");
        assert_eq!(quick_calls, v[10].parse::<usize>().unwrap(), "case {count}");
        if native_rng {
            assert_eq!(
                game_rng.state(),
                v[15].parse::<u32>().unwrap(),
                "case {count}"
            );
        }
        assert_eq!(
            usize::from(sound.unwrap_or(false)),
            v[11].parse::<usize>().unwrap(),
            "case {count}"
        );
        for (slot, actual) in city.traffic_overlays().unwrap().iter().enumerate() {
            assert_eq!(
                *actual,
                record(&v[12][slot * 24..slot * 24 + 24]),
                "case {count}, slot {slot}: {line}"
            );
        }
        for row in 27..33 {
            for col in 49..52 {
                let i = (row - 27) * 3 + col - 49;
                let expected = u8::from_str_radix(&v[13][i * 2..i * 2 + 2], 16).unwrap();
                assert_eq!(
                    city.text_overlay_at(row, col),
                    Some(expected),
                    "case {count}, {row},{col}"
                );
            }
        }
        count += 1;
    }
    assert_eq!(count, 384);
}

#[test]
fn mac_rail_handler_suffix_matches_original_68020() {
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_suffix.csv");
    let mut city = City::new_blank("/tmp/mac-rail-suffix.sc2", "Rail").unwrap();
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let mask: u8 = v[2].parse().unwrap();
        let draw: u16 = v[3].parse().unwrap();
        let transition: u8 = v[4].parse().unwrap();
        let quick: u16 = v[5].parse().unwrap();
        city.set_building(30, 50, if transition == 0 { 0x2c } else { 0x6c });
        for (i, (dr, dc)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
            let (row, col) = (30_i16 + dr, 50_i16 + dc);
            city.set_building(
                row as usize,
                col as usize,
                if mask & (1 << i) != 0 { 0x2c } else { 0 },
            );
            city.set_underground(row as usize, col as usize, u8::from(mask & (1 << i) != 0));
            city.set_text_overlay(row as usize, col as usize, 0);
        }
        city.set_mac_object_slot_record(5, record(v[6]));
        city.set_mac_object_slot_record(13, record(v[7]));
        let mut app_calls = 0;
        let mut quick_calls = 0;
        let sound = mac_rail_handler_suffix(
            &mut city,
            5,
            13,
            || {
                quick_calls += 1;
                quick
            },
            || {
                app_calls += 1;
                draw
            },
        )
        .unwrap();
        assert_eq!(
            city.traffic_overlays().unwrap()[5],
            record(v[8]),
            "case {count}: {line}"
        );
        assert_eq!(
            city.traffic_overlays().unwrap()[13],
            record(v[9]),
            "case {count}: {line}"
        );
        assert_eq!(app_calls, v[10].parse::<usize>().unwrap(), "case {count}");
        assert_eq!(quick_calls, v[11].parse::<usize>().unwrap(), "case {count}");
        assert_eq!(
            usize::from(sound),
            v[12].parse::<usize>().unwrap(),
            "case {count}"
        );
        count += 1;
    }
    assert_eq!(count, 512);
}

#[test]
fn mac_rail_handler_prefix_matches_original_68020() {
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_prefix.csv");
    let mut city = City::new_blank("/tmp/mac-rail-prefix.sc2", "Rail").unwrap();
    let points = [(30, 50), (29, 50), (28, 50), (31, 50)];
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        assert_eq!(v.len(), 9);
        let mode: usize = v[0].parse().unwrap();
        let draw: u16 = v[2].parse().unwrap();
        for (row, col) in points {
            city.set_building(row, col, 0);
            city.set_underground(row, col, 0);
        }
        city.set_building(30, 50, if mode == 1 { 0 } else { 0x2c });
        if mode == 0 {
            city.set_building(if draw == 0 { 29 } else { 31 }, 50, 0xed);
        }
        for slot in 0..40 {
            city.set_mac_object_slot_record(slot, record(&v[3][slot * 24..slot * 24 + 24]));
        }
        for (i, (row, col)) in points.into_iter().enumerate() {
            city.set_text_overlay(
                row,
                col,
                u8::from_str_radix(&v[4][i * 2..i * 2 + 2], 16).unwrap(),
            );
        }
        let mut draws = 0;
        let actual = mac_rail_handler_prefix(&mut city, 5, || {
            draws += 1;
            draw
        })
        .unwrap();
        let expected = match v[5] {
            "station" => MacRailPrefixOutcome::Station,
            "retired" => MacRailPrefixOutcome::Retired,
            "blocked" => MacRailPrefixOutcome::Blocked,
            "moved" => MacRailPrefixOutcome::Moved,
            _ => panic!("bad fixture"),
        };
        assert_eq!(actual, expected, "case {count}");
        assert_eq!(draws, v[6].parse::<usize>().unwrap(), "case {count}");
        for (slot, actual) in city.traffic_overlays().unwrap().iter().enumerate() {
            assert_eq!(
                *actual,
                record(&v[7][slot * 24..slot * 24 + 24]),
                "case {count}, slot {slot}"
            );
        }
        for (i, (row, col)) in points.into_iter().enumerate() {
            let expected = u8::from_str_radix(&v[8][i * 2..i * 2 + 2], 16).unwrap();
            assert_eq!(
                city.text_overlay_at(row, col),
                Some(expected),
                "case {count}, point {i}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 192);
}

#[test]
fn mac_rail_handler_moves_native_single_free_slot_alias() {
    // CODE 6's unchecked constructor writes both failed follower allocations
    // through reserved slot zero. The next sweep sees head -> slot 0 -> slot
    // 0. The original handler still advances the head and its XTXT marker.
    let mut city = City::new_blank("/tmp/mac-rail-slot-zero-alias.sc2", "Rail").unwrap();
    for (x, y) in [(64, 61), (63, 61), (65, 61), (64, 60), (64, 62)] {
        city.set_building(x, y, 0x2c);
        city.set_underground(x, y, 0);
    }
    city.set_mac_object_slot_record(0, record("0b0000403d00403d8daf0010"));
    city.set_mac_object_slot_record(1, record("0a0000403d00403c1e960050"));
    city.set_text_overlay(64, 61, 0xca);

    let mut draws = 0;
    let result = mac_rail_handler_prefix(&mut city, 1, || {
        draws += 1;
        0
    });

    assert_eq!(result, Some(MacRailPrefixOutcome::Moved));
    assert_eq!(draws, 1);
    assert_eq!(city.text_overlay_at(64, 61), Some(0));
    assert_eq!(city.text_overlay_at(64, 60), Some(0xca));
    let records = city.traffic_overlays().unwrap();
    assert_eq!(records[1].tile_x, 64);
    assert_eq!(records[1].tile_y, 60);
    assert_eq!(records[0].kind, 11);
    assert_eq!(records[0].tile_x, 64);
    assert_eq!(records[0].tile_y, 61);
}

#[test]
fn mac_rail_handler_moves_native_two_free_slot_alias() {
    // The hash-pinned native Charleston replay has head -> slot 2 -> slot 0.
    // Native reaches movement even though the tail link uses reserved slot
    // zero. This prefix test checks that alias does not block the head and
    // proxy move; the native actor-sweep output also includes the later route
    // suffix, so its final XTHG records are checked by the external replay.
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/mac-rail-two-free-slot-move.sc2", "Rail").unwrap();
        city.kind = kind;
        city.set_building(64, 61, 0x2c);
        city.set_building(64, 60, 0x2c);
        city.set_mac_object_slot_record(0, record("0b0008403d00403dd1e5004b"));
        city.set_mac_object_slot_record(1, record("0a0002403d00403c6cd60005"));
        city.set_mac_object_slot_record(2, record("0b0000403d00403d01840011"));
        city.set_text_overlay(64, 61, 0xca);

        let mut draws = 0;
        let result = mac_rail_handler_prefix(&mut city, 1, || {
            draws += 1;
            0
        });

        assert_eq!(result, Some(MacRailPrefixOutcome::Moved));
        assert_eq!(draws, 1);
        assert_eq!(city.text_overlay_at(64, 61), Some(0));
        assert_eq!(city.text_overlay_at(64, 60), Some(0xca));
        let records = city.traffic_overlays().unwrap();
        assert_eq!((records[1].tile_x, records[1].tile_y), (64, 60));
    }
}

#[test]
fn mac_rail_handler_retires_native_single_free_slot_alias() {
    // The original teardown clears the aliased slot-zero follower and reuses
    // the head as a kind-6 effect without removing the head's XTXT proxy.
    let mut city = City::new_blank("/tmp/mac-rail-slot-zero-teardown.sc2", "Rail").unwrap();
    for (x, y) in [(64, 61), (63, 61), (65, 61), (64, 60), (64, 62)] {
        city.set_building(x, y, 0x2c);
        city.set_underground(x, y, 0);
    }
    city.set_building(64, 61, 0);
    city.set_mac_object_slot_record(0, record("0b0000403d00403d8daf0010"));
    city.set_mac_object_slot_record(1, record("0a0000403d00403c1e960050"));
    city.set_text_overlay(64, 61, 0xca);

    let mut draws = 0;
    let result = mac_rail_handler_prefix(&mut city, 1, || {
        draws += 1;
        0
    });

    assert_eq!(result, Some(MacRailPrefixOutcome::Retired));
    assert_eq!(draws, 1);
    assert_eq!(city.text_overlay_at(64, 61), Some(0xca));
    let records = city.traffic_overlays().unwrap();
    assert_eq!(records[0], record("000000403d00403d8daf0010"));
    assert_eq!(records[1], record("060000403d0008081e960000"));
}

#[test]
fn mac_rail_handler_retires_native_two_free_slot_alias() {
    // With two usable records, the constructor stores its third allocation in
    // reserved slot zero. Native teardown clears slots 2 and 0, turns the head
    // into a kind-6 effect, and retains its map proxy.
    let mut city = City::new_blank("/tmp/mac-rail-two-free-slot-teardown.sc2", "Rail").unwrap();
    city.set_mac_object_slot_record(0, record("0b0008403d00403dd1e5004b"));
    city.set_mac_object_slot_record(1, record("0a0002403d00403c6cd60005"));
    city.set_mac_object_slot_record(2, record("0b0000403d00403d01840011"));
    city.set_text_overlay(64, 61, 0xca);

    let mut draws = 0;
    let result = mac_rail_handler_prefix(&mut city, 1, || {
        draws += 1;
        0
    });

    assert_eq!(result, Some(MacRailPrefixOutcome::Retired));
    assert_eq!(draws, 1);
    assert_eq!(city.text_overlay_at(64, 61), Some(0xca));
    let records = city.traffic_overlays().unwrap();
    assert_eq!(records[0], record("000008403d00403dd1e5004b"));
    assert_eq!(records[1], record("060002403d0008086cd60000"));
    assert_eq!(records[2], record("000000403d00403d01840011"));
}

#[test]
fn mac_rail_admission_matches_original_68020() {
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_admission.csv");
    let mut city = City::new_blank("/tmp/mac-rail-admission.sc2", "Rail").unwrap();
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        let kind: u8 = v[0].parse().unwrap();
        let lane: u8 = v[1].parse().unwrap();
        let draw: u16 = v[2].parse().unwrap();
        let row: i16 = v[3].parse().unwrap();
        let col: i16 = v[4].parse().unwrap();
        let station: u8 = v[5].parse().unwrap();
        for (dr, dc) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (r, c) = (row + dr, col + dc);
            if (0..128).contains(&r) && (0..128).contains(&c) {
                city.set_building(r as usize, c as usize, 0);
                city.set_underground(r as usize, c as usize, 0);
            }
        }
        city.set_building(row as usize, col as usize, v[6].parse().unwrap());
        city.set_underground(row as usize, col as usize, v[7].parse().unwrap());
        let (dr, dc) =
            [(-1, 0), (1, 0), (0, -1), (0, 1)][usize::from(draw) + 2 * usize::from(lane)];
        if station != 0 && (0..128).contains(&(row + dr)) && (0..128).contains(&(col + dc)) {
            city.set_building((row + dr) as usize, (col + dc) as usize, 0xed);
        }
        let mut head = record("000000000000000000000000");
        head.kind = kind;
        head.lane = lane;
        head.tile_x = row as u8;
        head.tile_y = col as u8;
        let mut draws = 0;
        let actual = mac_rail_admission(&city, head, || {
            draws += 1;
            draw
        })
        .unwrap();
        let expected = match v[8] {
            "station" => MacRailAdmission::Station,
            "retire" => MacRailAdmission::Retire,
            "move" => MacRailAdmission::Move,
            _ => panic!("bad fixture"),
        };
        assert_eq!(actual, expected, "case {count}: {line}");
        assert_eq!(
            draws,
            v[9].parse::<usize>().unwrap(),
            "case {count}: {line}"
        );
        count += 1;
    }
    assert_eq!(count, 320);
}

#[test]
fn mac_rail_turnaround_matches_original_68020() {
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_turnaround.csv");
    let mut city = City::new_blank("/tmp/mac-rail-turnaround.sc2", "Rail").unwrap();
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        assert_eq!(v.len(), 6);
        for slot in 1..=3 {
            city.set_mac_object_slot_record(slot, record(v[slot - 1]));
        }
        assert!(mac_turn_rail_consist(&mut city, 1, 3));
        for slot in 1..=3 {
            assert_eq!(
                city.traffic_overlays().unwrap()[slot],
                record(v[slot + 2]),
                "case {count}, slot {slot}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 256);
}

#[test]
fn mac_rail_retirement_matches_original_68020() {
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_retire.csv");
    let mut city = City::new_blank("/tmp/mac-rail-retire.sc2", "Rail").unwrap();
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        assert_eq!(v.len(), 8);
        for slot in 0..40 {
            city.set_mac_object_slot_record(slot, record(&v[0][slot * 24..slot * 24 + 24]));
        }
        for (i, row) in [30, 29, 28].into_iter().enumerate() {
            city.set_text_overlay(row, 50, v[i + 1].parse().unwrap());
        }
        assert!(mac_retire_rail_consist(&mut city, 5, 8, 13));
        for (slot, actual) in city.traffic_overlays().unwrap().iter().enumerate() {
            assert_eq!(
                *actual,
                record(&v[4][slot * 24..slot * 24 + 24]),
                "case {count}, slot {slot}"
            );
        }
        for (i, row) in [30, 29, 28].into_iter().enumerate() {
            assert_eq!(
                city.text_overlay_at(row, 50),
                Some(v[i + 5].parse().unwrap()),
                "case {count}, row {row}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 192);
}

#[test]
fn mac_rail_station_search_matches_original_68020() {
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_station.csv");
    let mut city = City::new_blank("/tmp/mac-rail-station.sc2", "Rail").unwrap();
    let mut count = 0;
    let mut successes = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        assert_eq!(v.len(), 15);
        let row: i16 = v[0].parse().unwrap();
        let col: i16 = v[1].parse().unwrap();
        city.mac_rail_actor_counter = v[3].parse().unwrap();
        let old_active = city.mac_rail_actor_counter;
        for dr in -3..=3 {
            for dc in -3..=3 {
                let (r, c) = (row + dr, col + dc);
                let index = ((dr + 3) * 7 + dc + 3) as usize;
                if (0..128).contains(&r) && (0..128).contains(&c) {
                    let b = u8::from_str_radix(&v[5][index * 2..index * 2 + 2], 16).unwrap();
                    let m = u8::from_str_radix(&v[6][index * 2..index * 2 + 2], 16).unwrap();
                    city.set_building(r as usize, c as usize, b);
                    city.set_text_overlay(r as usize, c as usize, m);
                }
            }
        }
        city.set_zone(row as usize, col as usize, v[2].parse().unwrap());
        for slot in 0..40 {
            city.set_mac_object_slot_record(slot, record(&v[7][slot * 24..slot * 24 + 24]));
        }
        let mut lfsr_calls = 0;
        let mut game_calls = 0;
        let spawned = mac_spawn_rail_near_station(
            &mut city,
            row,
            col,
            || {
                lfsr_calls += 1;
                0
            },
            || {
                game_calls += 1;
                v[4].parse().unwrap()
            },
        );
        let expected_active: u16 = v[9].parse().unwrap();
        assert_eq!(city.mac_rail_actor_counter, expected_active, "{line}");
        assert_eq!(spawned, city.mac_rail_actor_counter != old_active, "{line}");
        for dr in -3..=3 {
            for dc in -3..=3 {
                let (r, c) = (row + dr, col + dc);
                let index = ((dr + 3) * 7 + dc + 3) as usize;
                if (0..128).contains(&r) && (0..128).contains(&c) {
                    let expected =
                        u8::from_str_radix(&v[10][index * 2..index * 2 + 2], 16).unwrap();
                    assert_eq!(
                        city.text_overlay_at(r as usize, c as usize),
                        Some(expected),
                        "{line}"
                    );
                }
            }
        }
        for (slot, actual) in city.traffic_overlays().unwrap().iter().enumerate() {
            assert_eq!(
                *actual,
                record(&v[11][slot * 24..slot * 24 + 24]),
                "slot {slot}: {line}"
            );
        }
        assert_eq!(lfsr_calls, v[12].parse::<usize>().unwrap(), "{line}");
        assert_eq!(game_calls, v[13].parse::<usize>().unwrap(), "{line}");
        if (24..32).contains(&count) {
            assert!(
                !spawned,
                "an out-of-bounds first candidate prevents later probes"
            );
            assert_eq!((lfsr_calls, game_calls), (0, 0));
        }
        successes += usize::from(spawned);
        count += 1;
    }
    assert_eq!((count, successes), (128, 37));
}

#[test]
fn mac_rail_constructor_matches_original_68020() {
    let fixture =
        include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_constructor.csv");
    let mut city = City::new_blank("/tmp/mac-rail-constructor.sc2", "Rail").unwrap();
    let mut count = 0;
    let mut successes = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        assert_eq!(v.len(), 15);
        let row: i16 = v[0].parse().unwrap();
        let col: i16 = v[1].parse().unwrap();
        city.mac_rail_actor_counter = v[2].parse().unwrap();
        city.set_building(row as usize, col as usize, v[3].parse().unwrap());
        city.set_text_overlay(row as usize, col as usize, v[4].parse().unwrap());
        for (i, (dr, dc)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
            let (r, c) = (row + dr, col + dc);
            if (0..128).contains(&r) && (0..128).contains(&c) {
                let building = u8::from_str_radix(&v[5][i * 2..i * 2 + 2], 16).unwrap();
                let marker = u8::from_str_radix(&v[6][i * 2..i * 2 + 2], 16).unwrap();
                city.set_building(r as usize, c as usize, building);
                city.set_text_overlay(r as usize, c as usize, marker);
            }
        }
        for slot in 0..40 {
            city.set_mac_object_slot_record(slot, record(&v[8][slot * 24..slot * 24 + 24]));
        }
        let mut lfsr_calls = 0;
        let mut game_calls = 0;
        let spawned = mac_spawn_rail_at(
            &mut city,
            row,
            col,
            || {
                lfsr_calls += 1;
                0
            },
            || {
                game_calls += 1;
                v[7].parse().unwrap()
            },
        );
        assert_eq!(spawned, v[9] == "1", "{line}");
        assert_eq!(
            city.mac_rail_actor_counter,
            v[10].parse::<u16>().unwrap(),
            "{line}"
        );
        assert_eq!(
            city.text_overlay_at(row as usize, col as usize),
            Some(v[11].parse().unwrap()),
            "{line}"
        );
        for (slot, actual) in city.traffic_overlays().unwrap().iter().enumerate() {
            assert_eq!(
                *actual,
                record(&v[12][slot * 24..slot * 24 + 24]),
                "slot {slot}: {line}"
            );
        }
        assert_eq!(lfsr_calls, v[13].parse::<usize>().unwrap(), "{line}");
        assert_eq!(game_calls, v[14].parse::<usize>().unwrap(), "{line}");
        successes += usize::from(spawned);
        count += 1;
    }
    assert_eq!((count, successes), (160, 39));
}

#[test]
fn mac_rail_marker_rotation_matches_original_68020() {
    let fixture = include_str!("../../../../tests/fixtures/simulation/mac/mac_rail_rotation.csv");
    let mut city = City::new_blank("/tmp/mac-rail-rotation.sc2", "Rail").unwrap();
    let mut count = 0;
    for line in fixture.lines().skip(1) {
        let v: Vec<_> = line.split(',').collect();
        assert_eq!(v.len(), 16);
        for slot in 1..=3 {
            city.set_mac_object_slot_record(slot, record(v[slot - 1]));
        }
        let dest = (
            v[3].parse::<usize>().unwrap(),
            v[4].parse::<usize>().unwrap(),
        );
        let points = [(20, 20), (19, 20), (18, 20), dest];
        for (i, (x, y)) in points.iter().copied().enumerate() {
            city.set_text_overlay(x, y, v[5 + i].parse().unwrap());
        }
        assert!(mac_move_rail_consist(&mut city, 1, 2, 3), "{line}");
        let records = city.traffic_overlays().unwrap();
        for slot in 1..=3 {
            assert_eq!(records[slot], record(v[8 + slot]), "slot {slot}: {line}");
        }
        for (i, (x, y)) in points.iter().copied().enumerate() {
            assert_eq!(
                city.text_overlay_at(x, y),
                Some(v[12 + i].parse().unwrap()),
                "{line}"
            );
        }
        count += 1;
    }
    assert_eq!(count, 192);
}

#[test]
fn mac_rail_record_axes_map_directly_to_city_axes() {
    let mut city = City::new_blank("/tmp/mac-rail-row-col.sc2", "Rail").unwrap();
    city.set_mac_object_slot_record(
        1,
        Win95OverlayRecord {
            kind: 10,
            tile_x: 30,
            tile_y: 50,
            sub_x: 31,
            sub_y: 50,
            ..Default::default()
        },
    );
    city.set_mac_object_slot_record(
        2,
        Win95OverlayRecord {
            kind: 11,
            tile_x: 29,
            tile_y: 50,
            ..Default::default()
        },
    );
    city.set_mac_object_slot_record(
        3,
        Win95OverlayRecord {
            kind: 11,
            tile_x: 28,
            tile_y: 50,
            saved_a: 7,
            ..Default::default()
        },
    );
    city.set_text_overlay(30, 50, 0xca);
    city.set_text_overlay(29, 50, 0xcb);
    city.set_text_overlay(28, 50, 0xcc);
    assert!(mac_move_rail_consist(&mut city, 1, 2, 3));
    assert_eq!(city.text_overlay_at(31, 50), Some(0xca));
    assert_eq!(city.text_overlay_at(30, 50), Some(0xcb));
    assert_eq!(city.text_overlay_at(29, 50), Some(0xcc));
    assert_eq!(city.text_overlay_at(28, 50), Some(7));
    assert_eq!(
        city.text_overlay_at(50, 31),
        Some(0),
        "opposite-axis map coordinate remains unchanged"
    );
}

#[test]
fn mac_rail_occupied_destination_leaves_records_and_markers_unchanged() {
    let mut city = City::new_blank("/tmp/mac-rail-blocked.sc2", "Rail").unwrap();
    let mut head = Win95OverlayRecord::default();
    head.kind = 10;
    head.tile_x = 20;
    head.tile_y = 20;
    head.sub_x = 21;
    head.sub_y = 20;
    city.set_mac_object_slot_record(1, head);
    city.set_mac_object_slot_record(
        2,
        Win95OverlayRecord {
            kind: 11,
            tile_x: 19,
            tile_y: 20,
            ..Default::default()
        },
    );
    city.set_mac_object_slot_record(
        3,
        Win95OverlayRecord {
            kind: 11,
            tile_x: 18,
            tile_y: 20,
            ..Default::default()
        },
    );
    city.set_text_overlay(21, 20, 0xfa);
    let before = city.traffic_overlays().unwrap().to_vec();
    assert!(!mac_move_rail_consist(&mut city, 1, 2, 3));
    assert_eq!(city.traffic_overlays().unwrap(), before);
    assert_eq!(city.text_overlay_at(21, 20), Some(0xfa));
}
