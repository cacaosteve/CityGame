use super::*;

#[test]
fn ambient_train_follows_rail_or_despawns() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/train-rail.sc2"), "TrainRail").expect("city");
    city.set_building(10, 10, 45); // rail
    city.set_building(11, 10, 45);
    let mut rng = MacToolboxRng::new(1);
    let mut record = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 1, // PE stores east as cardinal index 1.
        state: 1,
        tile_x: 10,
        tile_y: 10,
        sub_x: 11,
        sub_y: 10,
        height: city.altitude_at(10, 10),
        ..Default::default()
    };
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        2
    ));
    assert_eq!(record.kind, XTHG_TRAIN_ENGINE);
    assert_eq!(record.tile_x, 11);
    assert_eq!(
        record.lane, 3,
        "PE selects the reverse rail before the next crossing"
    );
    assert_eq!((record.sub_x, record.sub_y), (10, 10));

    // End of track: reverse onto previous rail.
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        2
    ));
    assert_eq!(record.kind, XTHG_TRAIN_ENGINE);
    assert_eq!(record.tile_x, 10);
    assert_eq!(record.lane, 1);

    // Single rail tile, no neighbors → despawn.
    city.set_building(10, 10, 0);
    record.lane = 1;
    record.tile_x = 11;
    record.sub_x = 12;
    record.sub_y = 10;
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        2
    ));
    assert_eq!(record.kind, 0);
}

#[test]
fn ambient_train_odd_lane_is_native_east_cardinal() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-legacy-lane.sc2"),
        "TrainLegacyLane",
    )
    .expect("city");
    city.set_building(10, 10, 45);
    city.set_building(11, 10, 45);
    city.set_building(12, 10, 45);
    let mut record = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 1, // PE train records store east as cardinal index 1.
        state: 1,
        tile_x: 10,
        tile_y: 10,
        sub_x: 11,
        sub_y: 10,
        height: city.altitude_at(10, 10),
        ..Default::default()
    };
    let mut rng = ZeroModRng;
    let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);

    assert_eq!(
        step_ambient_train_engine(&mut record, &city, &mut rng, &mut game_rand, None),
        TrainEngineStep::Advanced
    );
    assert_eq!((record.tile_x, record.tile_y), (11, 10));
    assert_eq!(record.lane, 1, "straight east route keeps native lane 1");
    assert_eq!((record.sub_x, record.sub_y), (12, 10));
}

#[test]
fn pe_train_cleared_despawn_zeros_xtxt_not_restore_saved_a() {
    // PE train clear / Cleared arms call `FUN_00401528` (zero XTXT), not `+A` restore.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-xtxt-clear.sc2"),
        "TrainXtxtClear",
    )
    .expect("city");
    city.set_building(10, 10, 45);
    // No outbound rail → Cleared on the next occupy probe.
    let mut table = empty_overlay_table();
    let slot = XTHG_FIRST_USABLE_SLOT;
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    table[slot] = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 2,
        state: 1,
        tile_x: 10,
        tile_y: 10,
        sub_x: 11, // next tile has no rail
        sub_y: 10,
        saved_a: 0x22,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    city.set_text_overlay(10, 10, proxy);
    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(sim.tick_ambient(&mut city, GameSpeed::Llama, 1.0, &mut crt, &mut lfsr));
    assert_eq!(city.traffic_overlays().unwrap()[slot].kind, 0);
    assert_eq!(city.text_overlay_at(10, 10), Some(0));
}

#[test]
fn ambient_train_snaps_to_curve_and_branches_at_junction() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/train-junc.sc2"), "TrainJunc").expect("city");
    // L-curve: east then south.
    city.set_building(10, 10, 45);
    city.set_building(11, 10, 45);
    city.set_building(11, 11, 45);
    let mut rng = MacToolboxRng::new(1);
    let mut record = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 1,
        state: 1,
        tile_x: 10,
        tile_y: 10,
        sub_x: 11,
        sub_y: 10,
        height: city.altitude_at(10, 10),
        ..Default::default()
    };
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        2
    ));
    assert_eq!(record.tile_x, 11);
    assert_eq!(
        record.lane, 2,
        "curve should snap to the only forward rail exit"
    );

    // + junction: north/east/south from the hub.
    city.set_building(20, 20, 45);
    city.set_building(21, 20, 45);
    city.set_building(20, 19, 45);
    city.set_building(20, 21, 45);
    city.set_building(19, 20, 45);
    let mut branched = false;
    for seed in 0..64u32 {
        let mut rng = MacToolboxRng::new(seed);
        let mut record = Win95OverlayRecord {
            kind: XTHG_TRAIN_ENGINE,
            lane: 1,
            state: 1,
            tile_x: 19,
            tile_y: 20,
            sub_x: 20,
            sub_y: 20,
            height: city.altitude_at(19, 20),
            ..Default::default()
        };
        let mut lfsr = Win95Lfsr::new(1);
        assert!(step_ambient_actor(
            &mut record,
            &mut city,
            &mut rng,
            &mut lfsr,
            2
        ));
        assert_eq!(record.tile_x, 20);
        assert!(
            matches!(record.lane, 0 | 1 | 2),
            "junction branch must be a non-reverse cardinal, got {}",
            record.lane
        );
        if record.lane != 1 {
            branched = true;
        }
    }
    assert!(
        branched,
        "some junction seeds should take a turn instead of continuing"
    );
}

#[test]
fn ambient_train_pe_fallback_order_uses_game_lcg_mode() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-fallback.sc2"),
        "TrainFallback",
    )
    .expect("city");
    city.set_building(10, 10, 45);
    city.set_building(10, 9, 45);
    city.set_building(10, 11, 45);
    city.set_building(9, 10, 45);
    city.set_building(11, 10, 45);
    let mut record = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 1, // native PE east cardinal; east is removed below.
        tile_x: 10,
        tile_y: 10,
        ..Default::default()
    };
    city.set_building(11, 10, 0);
    // `FUN_00456100` chooses [straight, left, right, reverse] on result 0,
    // and [straight, right, left, reverse] on result 1.
    let seed_for_mode = |wanted| {
        (0..1000)
            .find(|&seed| {
                let mut rng = Win95GameRand::new(seed);
                rng.random_mod(2) == wanted
            })
            .expect("both Game_Random(2) modes occur")
    };
    for (mode, expected_cardinal) in [(0, 0), (1, 2)] {
        let seed = seed_for_mode(mode);
        let mut expected_rand = Win95GameRand::new(seed);
        assert_eq!(expected_rand.random_mod(2), mode);
        let mut game_rand = Win95GameRand::new(seed);
        assert_eq!(
            pick_pe_train_fallback(&record, &city, &mut game_rand),
            Some(expected_cardinal),
            "mode {mode} chooses its native first open neighbor"
        );
        assert_eq!(game_rand.state(), expected_rand.state());
    }
}

#[test]
fn ambient_surface_train_route_uses_pe_rail_tile_set() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/train-tiles.sc2"), "TrainTiles").expect("city");
    city.set_building(10, 10, 69);
    assert!(train_can_occupy(&city, 10, 10));
    city.set_building(10, 10, 78);
    assert!(train_can_occupy(&city, 10, 10));
    city.set_building(10, 10, 237);
    assert!(!train_can_occupy(&city, 10, 10));
}

#[test]
fn ambient_train_blocks_a_non_tail_xtext_proxy_target() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-foreign-proxy.sc2"),
        "TrainForeignProxy",
    )
    .expect("city");
    city.set_building(10, 10, 45);
    city.set_building(11, 10, 45);
    assert!(city.place_ambient_train(10, 10, 2));
    let engine_slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_TRAIN_ENGINE).then_some(slot))
        .expect("engine");
    let first_slot = city.traffic_overlays().unwrap()[engine_slot].state as usize;
    city.set_text_overlay(
        11,
        10,
        crate::win95_overlay::xthg_proxy_for_slot(first_slot).expect("first-car proxy"),
    );

    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let _ = sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr);

    let engine = city.traffic_overlays().unwrap()[engine_slot];
    assert_eq!((engine.tile_x, engine.tile_y), (10, 10));
    assert_eq!((engine.sub_x, engine.sub_y), (11, 10));
}

#[test]
fn ambient_train_allows_its_tail_xtext_proxy_target() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-tail-proxy.sc2"),
        "TrainTailProxy",
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
        .find_map(|(slot, record)| (record.kind == XTHG_TRAIN_ENGINE).then_some(slot))
        .expect("engine");
    let first_slot = city.traffic_overlays().unwrap()[engine_slot].state as usize;
    let tail_slot = city.traffic_overlays().unwrap()[first_slot].state as usize;
    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let mut game_rand = Win95GameRand::new(1);
    assert!(sim.tick_ambient_once_with_game_rand(&mut city, &mut crt, &mut lfsr, &mut game_rand,));
    assert!(sim.tick_ambient_once_with_game_rand(&mut city, &mut crt, &mut lfsr, &mut game_rand,));
    assert!(sim.tick_ambient_once_with_game_rand(&mut city, &mut crt, &mut lfsr, &mut game_rand,));
    assert_eq!(
        city.text_overlay_at(11, 10),
        crate::win95_overlay::xthg_proxy_for_slot(tail_slot)
    );
    city.traffic_overlays_mut().unwrap()[engine_slot].sub_x = 11;
    city.traffic_overlays_mut().unwrap()[engine_slot].sub_y = 10;

    let mut engine = city.traffic_overlays().unwrap()[engine_slot];
    assert_eq!(
        step_ambient_train_engine(
            &mut engine,
            &city,
            &mut crt,
            &mut game_rand,
            Some(tail_slot),
        ),
        TrainEngineStep::Advanced
    );
    assert_eq!(engine.tile_x, 11);
}

#[test]
fn ambient_train_station_face_uses_xtext_gate_not_xtrf() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-xtext-gate.sc2"),
        "TrainXtextGate",
    )
    .expect("city");
    city.set_building(10, 9, 45);
    city.set_traffic_density(10, 9, 0xff);
    assert_eq!(
        city.pe_train_spawn_track_from_station(10, 10),
        Some((10, 9))
    );

    city.set_text_overlay(10, 9, 1);
    assert_eq!(city.pe_train_spawn_track_from_station(10, 10), None);
    assert!(!city.place_ambient_train(10, 9, 4));
}

#[test]
fn ambient_train_toggles_only_at_pe_portal_before_routing() {
    let mut city = City::new_blank(std::path::Path::new("/tmp/train-portal.sc2"), "TrainPortal")
        .expect("city");
    city.set_building(10, 10, 45);
    city.set_building(11, 10, 45);
    city.set_underground(11, 10, 1);
    city.set_building(12, 10, 108);
    city.set_underground(13, 10, 1);
    let mut record = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 2,
        state: 1,
        tile_x: 10,
        tile_y: 10,
        sub_x: 11,
        sub_y: 10,
        ..Default::default()
    };
    let mut rng = MacToolboxRng::new(1);

    // Underground below an ordinary rail does not convert the surface engine.
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        2
    ));
    assert_eq!((record.tile_x, record.kind), (11, XTHG_TRAIN_ENGINE));

    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        2
    ));
    assert_eq!((record.tile_x, record.kind), (12, XTHG_SUBWAY_TRAIN_ENGINE));
    assert_eq!(
        record.lane, 1,
        "subway route must be chosen after the portal flip"
    );
}

#[test]
fn ambient_train_route_validity_uses_current_pe_kind() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-kind-route.sc2"),
        "TrainKindRoute",
    )
    .expect("city");
    city.set_underground(10, 10, 1);
    assert!(!ambient_actor_can_occupy(XTHG_TRAIN_ENGINE, &city, 10, 10));
    assert!(ambient_actor_can_occupy(
        XTHG_SUBWAY_TRAIN_ENGINE,
        &city,
        10,
        10
    ));

    city.set_underground(10, 10, 0);
    city.set_building(10, 10, 45);
    assert!(ambient_actor_can_occupy(XTHG_TRAIN_ENGINE, &city, 10, 10));
    assert!(!ambient_actor_can_occupy(
        XTHG_SUBWAY_TRAIN_ENGINE,
        &city,
        10,
        10
    ));

    city.set_building(10, 10, 112);
    assert!(ambient_actor_can_occupy(XTHG_TRAIN_ENGINE, &city, 10, 10));
    assert!(ambient_actor_can_occupy(
        XTHG_SUBWAY_TRAIN_ENGINE,
        &city,
        10,
        10
    ));
}

#[test]
fn ambient_train_spawn_direction_uses_pe_cardinal_fallback() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-spawn-direction.sc2"),
        "TrainSpawnDirection",
    )
    .expect("city");
    city.set_building(10, 10, 45);
    city.set_building(10, 9, 45);
    city.set_building(10, 11, 45);
    let seed_for_mode = |wanted| {
        (0..1000)
            .find(|&seed| {
                let mut rng = Win95GameRand::new(seed);
                rng.random_mod(2) == wanted
            })
            .expect("both Game_Random(2) modes occur")
    };
    let mut zero_mode = Win95GameRand::new(seed_for_mode(0));
    assert_eq!(
        ambient_train_spawn_direction(&city, 10, 10, 1, &mut zero_mode),
        Some(0),
        "zero mode chooses north before south when east is blocked"
    );
    let mut nonzero_mode = Win95GameRand::new(seed_for_mode(1));
    assert_eq!(
        ambient_train_spawn_direction(&city, 10, 10, 1, &mut nonzero_mode),
        Some(4),
        "nonzero mode chooses south before north when east is blocked"
    );
}

#[test]
fn ambient_train_occupancy_includes_pe_endpoint_rail_ids() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-endpoint-rail.sc2"),
        "TrainEndpointRail",
    )
    .expect("city");
    city.set_building(10, 10, 63);
    assert!(train_can_occupy(&city, 10, 10));
    city.set_building(10, 10, 73);
    assert!(train_can_occupy(&city, 10, 10));
}

#[test]
fn ambient_train_occupancy_rejects_xtxt_gate_for_both_arms() {
    // PE `FUN_00455C60` refuses destination XTXT `>= 0xC9`, not XTRF.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-xtxt-gate.sc2"),
        "TrainXtxtGate",
    )
    .expect("city");
    city.set_building(10, 10, 45);
    city.set_traffic_density(10, 10, MAC_TRAFFIC_HEAVY_BYTE_MIN);
    assert!(
        train_can_occupy(&city, 10, 10),
        "heavy XTRF must not refuse"
    );

    city.set_text_overlay(10, 10, XTHG_EXPLOSION_XTXT_GATE);
    assert!(!train_can_occupy(&city, 10, 10));

    city.set_text_overlay(10, 10, 0);
    city.set_building(10, 10, 108);
    city.set_traffic_density(10, 10, MAC_TRAFFIC_HEAVY_BYTE_MIN);
    assert!(
        subway_can_occupy(&city, 10, 10),
        "heavy XTRF must not refuse subway"
    );
    city.set_text_overlay(10, 10, XTHG_EXPLOSION_XTXT_GATE);
    assert!(!subway_can_occupy(&city, 10, 10));
}
