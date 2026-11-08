use super::*;

#[test]
fn ambient_train_cars_follow_engine_and_enter_subway() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/train-cars.sc2"), "TrainCars").expect("city");
    for x in 8..=14 {
        city.set_building(x, 10, 45);
    }
    city.set_building(12, 10, 108); // PE surface/subway portal
    city.set_underground(13, 10, 1); // subway beyond the portal
    city.set_text_overlay(12, 10, 37);
    assert!(city.place_ambient_train(11, 10, 2)); // east
    assert_eq!(city.ambient_active_train_count(), 1);
    assert_eq!(city.ambient_port_actor_count(XTHG_TRAIN_CAR), 2);

    let mut sim = Win95OverlaySim::new();
    let engine_before = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find(|record| record.kind == XTHG_TRAIN_ENGINE)
        .copied()
        .expect("engine");
    assert_eq!((engine_before.sub_x, engine_before.sub_y), (12, 10));
    // Force engine near the edge so the next tick crosses the PE portal.
    let engine_slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_TRAIN_ENGINE).then_some(slot))
        .expect("engine slot");
    assert_eq!(
        city.text_overlay_at(11, 10),
        crate::win95_overlay::xthg_proxy_for_slot(engine_slot)
    );
    let overlays = city.traffic_overlays_mut().unwrap();
    overlays[engine_slot].tile_x = 11;
    overlays[engine_slot].sub_x = 12;
    overlays[engine_slot].sub_y = 10;
    let car_slots: Vec<usize> = overlays
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .filter_map(|(slot, record)| record.is_ambient_train_car().then_some(slot))
        .collect();
    assert_eq!(car_slots.len(), 2);
    assert_eq!(engine_before.state, car_slots[0] as u8);
    assert_eq!(
        city.traffic_overlays().unwrap()[car_slots[0]].state,
        car_slots[1] as u8
    );
    assert_eq!(city.traffic_overlays().unwrap()[car_slots[1]].state, 0);

    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr));
    let engine = city.traffic_overlays().unwrap()[engine_slot];
    assert_eq!(engine.tile_x, 12);
    assert_eq!(engine.kind, XTHG_SUBWAY_TRAIN_ENGINE);
    assert_eq!(engine.saved_a, 37);
    assert!(engine.ambient_port_sprite_id().is_none());
    assert_eq!(city.text_overlay_at(11, 10), Some(0));
    assert_eq!(
        city.text_overlay_at(12, 10),
        crate::win95_overlay::xthg_proxy_for_slot(engine_slot)
    );

    // Cars still trail the pre-tick leader position for one frame.
    let first_car = city.traffic_overlays().unwrap()[car_slots[0]];
    assert_eq!(first_car.tile_x, engine_before.tile_x);
    assert_eq!(first_car.tile_y, engine_before.tile_y);
    assert!(first_car.is_ambient_train_car());
}

#[test]
fn ambient_train_advances_pe_xtext_proxies_for_spaced_cars() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-xtext-proxies.sc2"),
        "TrainXtextProxies",
    )
    .expect("city");
    for x in 10..=14 {
        city.set_building(x, 10, 45);
    }
    assert!(city.place_ambient_train(10, 10, 2));
    let mut sim = Win95OverlaySim::new();
    let engine_slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_TRAIN_ENGINE).then_some(slot))
        .expect("engine");
    let first_slot = city.traffic_overlays().unwrap()[engine_slot].state as usize;
    let second_slot = city.traffic_overlays().unwrap()[first_slot].state as usize;

    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr));
    assert!(sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr));
    assert!(sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr));

    let overlays = city.traffic_overlays().unwrap();
    assert_eq!(overlays[engine_slot].tile_x, 13);
    assert_eq!(overlays[first_slot].tile_x, 12);
    assert_eq!(overlays[second_slot].tile_x, 11);
    assert_eq!(
        city.text_overlay_at(13, 10),
        crate::win95_overlay::xthg_proxy_for_slot(engine_slot)
    );
    assert_eq!(
        city.text_overlay_at(12, 10),
        crate::win95_overlay::xthg_proxy_for_slot(first_slot)
    );
    assert_eq!(
        city.text_overlay_at(11, 10),
        crate::win95_overlay::xthg_proxy_for_slot(second_slot)
    );
    assert_eq!(city.text_overlay_at(10, 10), Some(0));
}

#[test]
fn pe_train_engine_slot_copies_both_linked_followers_on_crossing() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-native-slot-crossing.sc2"),
        "TrainNativeSlotCrossing",
    )
    .expect("city");
    for x in 10..=14 {
        city.set_building(x, 10, 45);
    }
    assert!(city.place_ambient_train(10, 10, 2));
    let engine_slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| record.is_ambient_train_engine().then_some(slot))
        .expect("engine slot");
    let overlays = city.traffic_overlays().unwrap();
    let first_slot = overlays[engine_slot].state as usize;
    let second_slot = overlays[first_slot].state as usize;
    let engine_before = overlays[engine_slot];
    let first_before = overlays[first_slot];
    let second_before = overlays[second_slot];

    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(0x4567_6015);
    let mut lfsr = Win95Lfsr::new(0x1234);
    let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
    assert!(sim.tick_pe_train_engine_slot(
        &mut city,
        engine_slot,
        &mut crt,
        &mut lfsr,
        &mut game_rand,
    ));

    let overlays = city.traffic_overlays().unwrap();
    let engine_after = overlays[engine_slot];
    let first_after = overlays[first_slot];
    let second_after = overlays[second_slot];
    assert_eq!((engine_after.tile_x, engine_after.tile_y), (11, 10));
    assert_eq!(
        (first_after.tile_x, first_after.tile_y),
        (engine_before.tile_x, engine_before.tile_y)
    );
    assert_eq!(
        (second_after.tile_x, second_after.tile_y),
        (first_before.tile_x, first_before.tile_y)
    );
    assert_eq!(first_after.state, first_before.state);
    assert_eq!(first_after.saved_b, first_before.saved_b);
    assert_eq!(second_after.state, second_before.state);
    assert_eq!(second_after.saved_b, second_before.saved_b);
    assert_eq!(city.text_overlay_at(10, 10), Some(second_before.saved_a));
    assert_eq!(
        city.text_overlay_at(11, 10),
        crate::win95_overlay::xthg_proxy_for_slot(engine_slot)
    );
}

#[test]
fn advance_ambient_train_xthg_proxies_matches_pe_455822() {
    // PE `0x455822`: first@engine-old, second@first-old, restore second-old from +A,
    // then displaced target → engine +A and engine proxy at target.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-xtxt-handoff.sc2"),
        "TrainXtxtHandoff",
    )
    .expect("city");
    let engine_slot = 2usize;
    let first_slot = 3usize;
    let second_slot = 4usize;
    city.set_text_overlay(10, 10, 0x11); // engine old (will become first proxy)
    city.set_text_overlay(9, 10, 0x22); // first old (will become second proxy)
    // PE writes second +A even when the cell is not the second proxy (foreign 0x55).
    city.set_text_overlay(8, 10, 0x55);
    city.set_text_overlay(11, 10, 0x44); // target displaced into engine +A

    let engine = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        tile_x: 10,
        tile_y: 10,
        saved_a: 0x11,
        ..Default::default()
    };
    let first = Win95OverlayRecord {
        kind: XTHG_TRAIN_CAR,
        tile_x: 9,
        tile_y: 10,
        saved_a: 0,
        saved_b: engine_slot as u8,
        ..Default::default()
    };
    let second = Win95OverlayRecord {
        kind: XTHG_TRAIN_CAR,
        tile_x: 8,
        tile_y: 10,
        saved_a: 0x33,
        saved_b: first_slot as u8,
        ..Default::default()
    };

    let displaced = city.advance_ambient_train_xthg_proxies(
        engine_slot,
        engine,
        first_slot,
        first,
        second_slot,
        second,
        11,
        10,
    );
    assert_eq!(displaced, 0x44);
    assert_eq!(
        city.text_overlay_at(10, 10),
        xthg_proxy_for_slot(first_slot)
    );
    assert_eq!(
        city.text_overlay_at(9, 10),
        xthg_proxy_for_slot(second_slot)
    );
    assert_eq!(city.text_overlay_at(8, 10), Some(0x33));
    assert_eq!(
        city.text_overlay_at(11, 10),
        xthg_proxy_for_slot(engine_slot)
    );
}

#[test]
fn ambient_train_no_route_reverses_pe_consist_instead_of_clearing() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-turnaround.sc2"),
        "TrainTurnaround",
    )
    .expect("city");
    // On-network end tile (`0x402E6E` false) with no outbound rail → NeedsTurnaround.
    city.set_building(11, 10, 45);
    let mut engine = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 1,
        tile_x: 11,
        tile_y: 10,
        sub_x: 12,
        sub_y: 13,
        state: 8,
        height: 5,
        target_x: 6,
        target_y: 9,
        saved_a: 0x33,
        saved_b: 0x44,
        ..Default::default()
    };
    let mut tail = Win95OverlayRecord {
        kind: XTHG_TRAIN_CAR,
        lane: 2,
        tile_x: 10,
        tile_y: 10,
        sub_x: 9,
        sub_y: 8,
        state: 4,
        height: 7,
        target_x: 7,
        target_y: 6,
        saved_a: 0x55,
        saved_b: 0x66,
        ..Default::default()
    };
    let mut rng = ZeroModRng;
    let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
    let mut stepping_engine = engine;
    stepping_engine.sub_x = 11;
    stepping_engine.sub_y = 10;
    assert_eq!(
        step_ambient_train_engine(
            &mut stepping_engine,
            &city,
            &mut rng,
            &mut game_rand,
            Some(3)
        ),
        TrainEngineStep::NeedsTurnaround
    );
    let mut engine = engine;
    reverse_pe_train_consist(&mut engine, &mut tail);
    assert_eq!((engine.tile_x, engine.tile_y), (10, 10));
    assert_eq!((engine.sub_x, engine.sub_y), (10, 10));
    assert_eq!((engine.lane, engine.target_x), (0, 4));
    assert_eq!((engine.saved_a, engine.state, engine.height), (0x55, 8, 5));
    assert_eq!((engine.target_y, engine.saved_b), (9, 0x44));
    assert_eq!((tail.tile_x, tail.tile_y), (11, 10));
    assert_eq!((tail.sub_x, tail.sub_y), (12, 13));
    assert_eq!((tail.lane, tail.target_x), (2, 7));
    assert_eq!((tail.saved_a, tail.state, tail.height), (0x33, 4, 7));
    assert_eq!((tail.target_y, tail.saved_b), (6, 0x66));
    // Live tick must not merge-explode while the engine remains on rail.
    assert!(!pe_train_merge_tile_eligible(&city, 11, 10));
}

#[test]
fn pe_train_turnaround_keeps_proxy_handoff_until_next_tile_crossing() {
    // PE `0x455822` paints the crossing proxies before `FUN_00455FC0` swaps
    // the engine/tail records. The engine therefore briefly stands on the
    // linked tail's proxy; that exact proxy is allowed by the next-tick
    // XTXT gate. The next crossing's ordinary handoff restores the displaced
    // underlay and realigns the slot proxies.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-turnaround-proxy-handoff.sc2"),
        "TrainTurnaroundProxyHandoff",
    )
    .expect("city");
    for x in 8..=11 {
        city.set_building(x, 10, 45);
    }

    let engine_slot = 2usize;
    let first_slot = 3usize;
    let second_slot = 4usize;
    let engine_proxy = xthg_proxy_for_slot(engine_slot).expect("engine proxy");
    let first_proxy = xthg_proxy_for_slot(first_slot).expect("first-car proxy");
    let second_proxy = xthg_proxy_for_slot(second_slot).expect("tail proxy");
    city.set_text_overlay(10, 10, engine_proxy);
    city.set_text_overlay(9, 10, first_proxy);
    city.set_text_overlay(8, 10, second_proxy);
    city.set_text_overlay(11, 10, 0x45);

    let mut table = empty_overlay_table();
    table[engine_slot] = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 1,
        state: first_slot as u8,
        tile_x: 10,
        tile_y: 10,
        sub_x: 11,
        sub_y: 10,
        saved_a: 0x12,
        ..Default::default()
    };
    table[first_slot] = Win95OverlayRecord {
        kind: XTHG_TRAIN_CAR,
        lane: 1,
        state: second_slot as u8,
        tile_x: 9,
        tile_y: 10,
        saved_a: 0x23,
        saved_b: engine_slot as u8,
        ..Default::default()
    };
    table[second_slot] = Win95OverlayRecord {
        kind: XTHG_TRAIN_CAR,
        lane: 1,
        tile_x: 8,
        tile_y: 10,
        saved_a: 0x34,
        saved_b: first_slot as u8,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);

    let mut crt = ZeroModRng;
    let mut lfsr = Win95Lfsr::new(Win95Lfsr::DEFAULT_STATE);
    let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
    let snapshot = city.traffic_overlays().unwrap().to_vec();
    assert!(step_pe_train_engine_slot(
        &mut city,
        engine_slot,
        snapshot[engine_slot],
        &snapshot,
        &mut crt,
        &mut lfsr,
        &mut game_rand,
    ));

    let overlays = city.traffic_overlays().unwrap();
    let engine = overlays[engine_slot];
    let first = overlays[first_slot];
    let tail = overlays[second_slot];
    assert_eq!((engine.tile_x, engine.tile_y), (9, 10));
    assert_eq!((first.tile_x, first.tile_y), (10, 10));
    assert_eq!((tail.tile_x, tail.tile_y), (11, 10));
    assert_eq!(city.text_overlay_at(9, 10), Some(second_proxy));
    assert_eq!(city.text_overlay_at(10, 10), Some(first_proxy));
    assert_eq!(city.text_overlay_at(11, 10), Some(engine_proxy));
    assert_eq!(city.text_overlay_at(8, 10), Some(0x34));
    assert_eq!(
        engine.saved_a, 0x23,
        "turnaround carries the second car's saved-underlay byte"
    );
    assert_eq!(
        tail.saved_a, 0x45,
        "tail now owns the engine target underlay"
    );
    assert!(pe_train_target_xtext_is_open(
        &city,
        Some(second_slot),
        9,
        10,
    ));

    let displaced = city.advance_ambient_train_xthg_proxies(
        engine_slot,
        engine,
        first_slot,
        first,
        second_slot,
        tail,
        12,
        10,
    );
    assert_eq!(displaced, 0);
    assert_eq!(city.text_overlay_at(11, 10), Some(0x45));
    assert_eq!(city.text_overlay_at(9, 10), Some(first_proxy));
    assert_eq!(city.text_overlay_at(10, 10), Some(second_proxy));
    assert_eq!(city.text_overlay_at(12, 10), Some(engine_proxy));
}

#[test]
fn pe_w_active_trains_counts_engines_not_cars() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/w-active-trains.sc2"),
        "WActiveTrains",
    )
    .expect("city");
    city.set_building(10, 10, 45);
    city.set_building(11, 10, 45);
    city.set_building(20, 20, 45);
    city.set_building(21, 20, 45);
    assert!(city.place_ambient_train(10, 10, 2));
    assert_eq!(city.ambient_active_train_count(), 1);
    assert_eq!(city.ambient_port_actor_count(XTHG_TRAIN_CAR), 2);
    assert!(city.place_ambient_train(20, 20, 2));
    assert_eq!(city.ambient_active_train_count(), 2);
    assert_eq!(city.ambient_port_actor_count(XTHG_TRAIN_CAR), 4);
}

#[test]
fn ambient_train_car_copies_only_pe_linked_fields() {
    let mut snapshot = vec![Win95OverlayRecord::default(); XTHG_RECORD_COUNT];
    snapshot[7] = Win95OverlayRecord {
        kind: XTHG_SUBWAY_TRAIN_ENGINE,
        lane: 6,
        state: 3,
        tile_x: 30,
        tile_y: 31,
        height: 9,
        sub_x: 4,
        sub_y: 12,
        target_x: 29,
        target_y: 28,
        saved_a: 0xaa,
        saved_b: 6,
    };
    let mut car = Win95OverlayRecord {
        kind: XTHG_TRAIN_CAR,
        lane: 0,
        state: 1,
        tile_x: 1,
        tile_y: 2,
        height: 0,
        sub_x: 8,
        sub_y: 8,
        target_x: 3,
        target_y: 77,
        saved_a: 0,
        saved_b: 7,
    };

    assert!(step_ambient_train_car(&mut car, &snapshot));
    assert_eq!(car.kind, XTHG_SUBWAY_TRAIN_CAR);
    assert_eq!(car.lane, 6);
    assert_eq!((car.tile_x, car.tile_y), (30, 31));
    assert_eq!((car.sub_x, car.sub_y), (4, 12));
    assert_eq!(car.target_x, 29);
    assert_eq!(car.saved_a, 0xaa);
    assert_eq!(car.state, 1);
    assert_eq!(car.height, 0);
    assert_eq!(car.target_y, 77);
    assert_eq!(car.saved_b, 7);
}

#[test]
fn pe_xthg_copy_train_follower_matches_fun_004029ff() {
    // PE `0x455EF0`: copy +0,+1,+3,+4,+6,+7,+8,+A; convert 0x0A->0x0B / 0x0C->0x0D.
    let engine = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 4,
        state: 9,
        tile_x: 12,
        tile_y: 13,
        height: 5,
        sub_x: 2,
        sub_y: 3,
        target_x: 7,
        target_y: 0x55,
        saved_a: 0x40,
        saved_b: 0x11,
    };
    let mut car = Win95OverlayRecord {
        kind: XTHG_TRAIN_CAR,
        lane: 0,
        state: 2,
        tile_x: 0,
        tile_y: 0,
        height: 8,
        sub_x: 0,
        sub_y: 0,
        target_x: 0,
        target_y: 0x66,
        saved_a: 0,
        saved_b: 3,
    };
    pe_xthg_copy_train_follower(&mut car, &engine);
    assert_eq!(car.kind, XTHG_TRAIN_CAR);
    assert_eq!(car.lane, 4);
    assert_eq!((car.tile_x, car.tile_y), (12, 13));
    assert_eq!((car.sub_x, car.sub_y), (2, 3));
    assert_eq!(car.target_x, 7);
    assert_eq!(car.saved_a, 0x40);
    assert_eq!(car.state, 2);
    assert_eq!(car.height, 8);
    assert_eq!(car.target_y, 0x66);
    assert_eq!(car.saved_b, 3);

    let subway = Win95OverlayRecord {
        kind: XTHG_SUBWAY_TRAIN_ENGINE,
        ..engine
    };
    pe_xthg_copy_train_follower(&mut car, &subway);
    assert_eq!(car.kind, XTHG_SUBWAY_TRAIN_CAR);
}
