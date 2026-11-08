use super::*;

#[test]
fn ambient_train_off_network_tile_runs_merge_finish_explosion() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-merge-live.sc2"),
        "TrainMergeLive",
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
    let second_slot = city.traffic_overlays().unwrap()[first_slot].state as usize;
    // PE `0x402E6E` accepts xbld 0 → merge path @ `0x4557bb`.
    city.set_building(10, 10, 0);

    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr));

    let overlays = city.traffic_overlays().unwrap();
    // Cleared slots may be reused by the kind-6 create.
    assert!(overlays[first_slot].kind == 0 || overlays[first_slot].is_pe_explosion_actor());
    assert!(overlays[second_slot].kind == 0 || overlays[second_slot].is_pe_explosion_actor());
    assert!(overlays[engine_slot].kind == 0 || overlays[engine_slot].is_pe_explosion_actor());
    assert!(city.pe_explosion_actor_at(10, 10).is_some());
    let (_, explosion) = city.pe_explosion_actor_at(10, 10).unwrap();
    assert_eq!(explosion.height, 0);
    assert_eq!(explosion.state, 0);
    assert_eq!(explosion.saved_b, 0);
    // PE merge `dec word [0x4E99D8]`: engine slots are gone, so wActiveTrains ≡ 0.
    assert_eq!(city.ambient_active_train_count(), 0);
}

#[test]
fn pe_actor_sweep_rebuild_drops_slot_zero_train_count() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-slot-zero-count.sc2"),
        "TrainSlotZeroCount",
    )
    .expect("city");
    let mut table = empty_overlay_table();
    for record in table.iter_mut().skip(XTHG_FIRST_USABLE_SLOT) {
        record.kind = 0xff; // occupies every usable slot without dispatching a handler
    }
    city.set_traffic_overlays_for_test(table);
    city.set_building(10, 10, 45);
    city.set_building(11, 10, 45);
    assert!(city.place_ambient_train(10, 10, 2));
    assert_eq!(city.ambient_active_train_count(), 1);
    assert_eq!(city.text_overlay_at(10, 10), Some(0xc9));

    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(!sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr));
    assert_eq!(city.ambient_active_train_count(), 0);
}

#[test]
fn pe_train_merge_finish_explosion_uses_zero_height_state_saved_b() {
    // PE `0x4557bb`: FUN_00453280(x, y, 0, 0, 0) after clearing linked slots.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-merge-kind6.sc2"),
        "TrainMergeKind6",
    )
    .expect("city");
    let mut table = empty_overlay_table();
    table[2] = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        tile_x: 20,
        tile_y: 20,
        ..Default::default()
    };
    table[3] = Win95OverlayRecord {
        kind: XTHG_TRAIN_CAR,
        tile_x: 20,
        tile_y: 20,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    city.set_text_overlay(20, 20, xthg_proxy_for_slot(2).expect("slot 2 proxy"));

    let slot = pe_train_merge_finish_explosion(&mut city, &[2, 3], 20, 20)
        .expect("merge finish explosion");
    let explosion = city.traffic_overlays().unwrap()[slot];
    assert_eq!(explosion.kind, crate::win95_overlay::XTHG_EXPLOSION);
    assert_eq!(explosion.height, 0);
    assert_eq!(explosion.state, 0);
    assert_eq!(explosion.saved_b, 0);
    assert_eq!(
        explosion.sub_x,
        crate::win95_overlay::XTHG_EXPLOSION_SUBTILE
    );
    assert_eq!(city.text_overlay_at(20, 20), xthg_proxy_for_slot(slot));
    assert_eq!(city.traffic_overlays().unwrap()[2].kind, 0);
    assert_eq!(city.traffic_overlays().unwrap()[3].kind, 0);
}

#[test]
fn pe_train_merge_finish_requires_clear_before_xtxt_gate() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-merge-gate.sc2"),
        "TrainMergeGate",
    )
    .expect("city");
    let mut table = empty_overlay_table();
    table[2] = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        tile_x: 20,
        tile_y: 20,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    city.set_text_overlay(20, 20, xthg_proxy_for_slot(2).expect("slot 2 proxy"));

    // Proxy still present and slots not cleared → XTXT gate rejects create.
    assert!(city.place_pe_explosion_actor(20, 20, 0, 0, 0).is_none());

    let slot = pe_train_merge_finish_explosion(&mut city, &[2], 20, 20).expect("clear then spawn");
    assert!(city.pe_explosion_actor_at(20, 20).is_some());
    assert_eq!(city.text_overlay_at(20, 20), xthg_proxy_for_slot(slot));
}

#[test]
fn pe_train_merge_tile_eligible_rejects_rail_and_subway_bands() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-merge-elig.sc2"),
        "TrainMergeElig",
    )
    .expect("city");
    // Empty surface tile is merge-eligible.
    assert!(pe_train_merge_tile_eligible(&city, 20, 20));
    // Surface rail band `0x2C..0x3E`.
    city.set_building(20, 20, 45);
    assert!(!pe_train_merge_tile_eligible(&city, 20, 20));
    city.set_building(20, 20, 0);
    // Special rejects.
    city.set_building(20, 20, 0x4d);
    assert!(!pe_train_merge_tile_eligible(&city, 20, 20));
    city.set_building(20, 20, 0);
    // XUND subway track band `1..0xF`.
    city.set_underground(20, 20, 3);
    assert!(!pe_train_merge_tile_eligible(&city, 20, 20));
    city.set_underground(20, 20, 0);
    city.set_underground(20, 20, 0x1f);
    assert!(!pe_train_merge_tile_eligible(&city, 20, 20));
    assert!(!pe_train_merge_tile_eligible(&city, 128, 20));
}

#[test]
fn pe_train_ed_neighbor_defers_only_for_surface_engine_with_ed() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-ed-defer.sc2"),
        "TrainEdDefer",
    )
    .expect("city");
    city.set_building(19, 20, 0xed);
    let record = Win95OverlayRecord {
        kind: XTHG_TRAIN_ENGINE,
        lane: 0, // even → dirs 0/1; game-LCG %2==0 → dir 0 → x-1
        tile_x: 20,
        tile_y: 20,
        ..Default::default()
    };
    // LFSR 0x8001 → first random_word_lsb is 1; game-LCG state 1 → first %2 is 0.
    let mut lfsr = Win95Lfsr::new(0x8001);
    let mut game_rand = Win95GameRand::new(1);
    let game_rand_before = game_rand.state();
    assert!(pe_train_ed_neighbor_defers_tick(
        &record,
        &city,
        &mut lfsr,
        &mut game_rand
    ));
    assert_ne!(game_rand.state(), game_rand_before);
    let subway = Win95OverlayRecord {
        kind: XTHG_SUBWAY_TRAIN_ENGINE,
        ..record
    };
    let mut lfsr = Win95Lfsr::new(0x8001);
    let mut game_rand = Win95GameRand::new(1);
    assert!(!pe_train_ed_neighbor_defers_tick(
        &subway,
        &city,
        &mut lfsr,
        &mut game_rand
    ));
    // LFSR 0x1234 → first lsb is 0: gate refuses without consuming game-LCG direction.
    let mut lfsr = Win95Lfsr::new(0x1234);
    let mut game_rand = Win95GameRand::new(1);
    let game_rand_before = game_rand.state();
    assert!(!pe_train_ed_neighbor_defers_tick(
        &record,
        &city,
        &mut lfsr,
        &mut game_rand
    ));
    assert_eq!(game_rand.state(), game_rand_before);
}

#[test]
fn pe_train_ed_neighbor_defers_consumes_process_lfsr_then_game_rand() {
    // Live ambient tick draws FUN_00455500 gate from process LFSR+game-LCG, not sim_rng.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-ed-process.sc2"),
        "TrainEdProcess",
    )
    .expect("city");
    city.set_building(19, 20, 0xed);
    city.set_building(20, 20, 45); // rail under engine
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let height = city.altitude_at(20, 20);
    let engine_slot = {
        let overlays = city.traffic_overlays_mut().unwrap();
        let slot = overlays
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find_map(|(slot, record)| (record.kind == 0).then_some(slot))
            .expect("free slot");
        overlays[slot] = Win95OverlayRecord {
            kind: XTHG_TRAIN_ENGINE,
            lane: 0,
            tile_x: 20,
            tile_y: 20,
            sub_x: 8,
            sub_y: 8,
            height,
            ..Default::default()
        };
        slot
    };
    let before = city.traffic_overlays().unwrap()[engine_slot];
    let mut sim = Win95OverlaySim::new();
    let overlay_before = sim.sim_rng_seed();
    let mut crt = Win95CrtRand::new(3);
    let mut lfsr = Win95Lfsr::new(0x8001);
    let mut game_rand = Win95GameRand::new(1);
    let mut preview_crt = crt;
    let mut preview_lfsr = lfsr;
    let mut preview_game_rand = game_rand;
    assert!(pe_train_ed_neighbor_defers_tick(
        &before,
        &city,
        &mut preview_lfsr,
        &mut preview_game_rand
    ));
    let _ = sim.tick_ambient_once_with_game_rand(&mut city, &mut crt, &mut lfsr, &mut game_rand);
    assert_eq!(sim.sim_rng_seed(), overlay_before);
    assert_eq!(
        crt.seed(),
        preview_crt.seed(),
        "ED gate leaves CRT untouched"
    );
    assert_eq!(lfsr.state(), preview_lfsr.state());
    assert_eq!(game_rand.state(), preview_game_rand.state());
    assert_eq!(
        city.traffic_overlays().unwrap()[engine_slot],
        before,
        "ED defer must skip merge/move this tick"
    );
}

#[test]
fn ambient_train_route_consumes_process_crt() {
    // FUN_00455500 route/fallback CRT shares the live process stream after ED defer.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/train-route-crt.sc2"),
        "TrainRouteCrt",
    )
    .expect("city");
    for x in 18..=22 {
        city.set_building(x, 20, 45);
    }
    city.set_traffic_overlays_for_test(crate::win95_overlay::empty_overlay_table());
    let height = city.altitude_at(20, 20);
    let engine_slot = {
        let overlays = city.traffic_overlays_mut().unwrap();
        let slot = overlays
            .iter()
            .enumerate()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find_map(|(slot, record)| (record.kind == 0).then_some(slot))
            .expect("free slot");
        overlays[slot] = Win95OverlayRecord {
            kind: XTHG_TRAIN_ENGINE,
            lane: 2, // east
            tile_x: 20,
            tile_y: 20,
            sub_x: 21, // next rail tile already chosen
            sub_y: 20,
            height,
            ..Default::default()
        };
        slot
    };
    let mut sim = Win95OverlaySim::new();
    let overlay_before = sim.sim_rng_seed();
    // LFSR lsb 0 → skip ED defer; CRT advances in route selection.
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(0x1234);
    let mut game_rand = Win95GameRand::new(1);
    let mut preview_crt = crt;
    let mut preview_lfsr = lfsr;
    let mut preview_game_rand = game_rand;
    let before = city.traffic_overlays().unwrap()[engine_slot];
    assert!(!pe_train_ed_neighbor_defers_tick(
        &before,
        &city,
        &mut preview_lfsr,
        &mut preview_game_rand
    ));
    let mut preview_engine = before;
    let _ = step_ambient_train_engine(
        &mut preview_engine,
        &city,
        &mut preview_crt,
        &mut preview_game_rand,
        None,
    );
    assert!(
        preview_crt.seed() != crt.seed(),
        "route pick must consume CRT"
    );
    assert!(sim.tick_ambient_once_with_game_rand(&mut city, &mut crt, &mut lfsr, &mut game_rand,));
    assert_eq!(sim.sim_rng_seed(), overlay_before);
    assert_eq!(crt.seed(), preview_crt.seed());
    assert_eq!(lfsr.state(), preview_lfsr.state());
    assert_eq!(game_rand.state(), preview_game_rand.state());
}
