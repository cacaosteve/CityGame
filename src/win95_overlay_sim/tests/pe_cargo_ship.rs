use super::*;

#[test]
fn tick_ambient_moves_placed_cargo_ship() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/ship-step.sc2"), "Ship").expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    // Subtile-only advance stays on the spawn tile (crane/shore allowed).
    city.set_building(20, 20, 224);
    for x in 20..=24 {
        city.set_tile_bits(x, 20, crate::city::TileBits::new(0x04));
        city.set_terrain(x, 20, 0x10);
    }
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr));
    let record = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find(|record| record.kind == XTHG_CARGO_SHIP)
        .copied()
        .expect("cargo ship still present");
    assert!(
        record.tile_x != 20 || record.sub_x != SUBTILE_CENTER,
        "ship should advance along east facing (tile={}, sub={})",
        record.tile_x,
        record.sub_x
    );
}

#[test]
fn ambient_cargo_ship_state_rolls_consume_process_lfsr() {
    // PE `FUN_00456C70` state rolls use `0x401cad` (process LFSR), not CRT.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/ship-lfsr.sc2"), "ShipLfsr").expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    for x in 18..=24 {
        for y in 18..=24 {
            city.set_terrain(x, y, 0x10); // water
        }
    }
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0x04));
    city.set_building(20, 20, 0);
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].state = 0;
        overlays[slot].target_x = 24;
        overlays[slot].target_y = 20;
    }
    let mut sim = Win95OverlaySim::new();
    let overlay_before = sim.sim_rng_seed();
    let mut crt = Win95CrtRand::new(1);
    let crt_before = crt.seed();
    let mut lfsr = Win95Lfsr::new(1);
    let mut preview_lfsr = lfsr;
    let mut preview_city = city.clone();
    let mut preview = preview_city.traffic_overlays().unwrap()[slot];
    assert!(step_pe_cargo_actor_mut(
        &mut preview,
        &mut preview_city,
        &mut preview_lfsr,
        slot,
    ));
    assert_ne!(
        preview_lfsr.state(),
        lfsr.state(),
        "cargo state-0 must consume process LFSR"
    );
    assert_eq!(
        crt.seed(),
        crt_before,
        "direct state helper must not consume CRT"
    );
    // Live tick also burns CRT on the FUN_00456760 SFX gate before LFSR.
    let mut tick_crt = crt;
    let mut tick_lfsr = lfsr;
    let _ = sim.tick_ambient_once(&mut city, &mut tick_crt, &mut tick_lfsr);
    assert_eq!(sim.sim_rng_seed(), overlay_before);
    assert_ne!(
        tick_crt.seed(),
        crt_before,
        "live tick SFX gate consumes CRT"
    );
    assert_eq!(tick_lfsr.state(), preview_lfsr.state());
}

#[test]
fn ambient_cargo_ship_tick_consumes_process_crt_sfx_gate() {
    // PE FUN_00456760 @ 0x45676e: always CRT rand() before LFSR state rolls.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/ship-sfx.sc2"), "ShipSfx").expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    for x in 18..=24 {
        for y in 18..=24 {
            city.set_terrain(x, y, 0x10);
        }
    }
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0x04));
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].state = 0;
        overlays[slot].target_x = 24;
        overlays[slot].target_y = 20;
    }
    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let crt_before = crt.seed();
    let mut lfsr = Win95Lfsr::new(1);
    let lfsr_before = lfsr.state();
    let mut preview_crt = crt;
    let mut preview_lfsr = lfsr;
    let mut preview_city = city.clone();
    let mut preview = preview_city.traffic_overlays().unwrap()[slot];
    let (ok, _play_sfx) = step_pe_cargo_actor_with_streams(
        &mut preview,
        &mut preview_city,
        &mut preview_crt,
        &mut preview_lfsr,
        slot,
    );
    assert!(ok);
    assert_ne!(preview_crt.seed(), crt_before, "SFX gate must consume CRT");
    assert_ne!(
        preview_lfsr.state(),
        lfsr_before,
        "state rolls still consume LFSR"
    );
    let _ = sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr);
    assert_eq!(crt.seed(), preview_crt.seed());
    assert_eq!(lfsr.state(), preview_lfsr.state());
}

#[test]
fn cargo_tick_queues_sfx_0x205_when_crt_al_zero() {
    // PE FUN_00456760 @ 0x45676e: test al,al → 0x401096(0x205).
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/ship-horn.sc2"), "ShipHorn").expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    for x in 18..=24 {
        for y in 18..=24 {
            city.set_terrain(x, y, 0x10);
        }
    }
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0x04));
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].state = 0;
        overlays[slot].target_x = 24;
        overlays[slot].target_y = 20;
    }
    let mut record = city.traffic_overlays().unwrap()[slot];
    // CRT seed whose first random_u16 low byte is 0.
    let mut zero_al = Win95CrtRand::new(1);
    for _ in 0..10_000 {
        let mut probe = zero_al;
        if probe.random_u16() & 0xff == 0 {
            break;
        }
        let _ = zero_al.random_u16();
    }
    let mut probe = zero_al;
    assert_eq!(probe.random_u16() & 0xff, 0, "fixture must hit al==0");
    let mut lfsr = Win95Lfsr::new(1);
    let (ok, play) =
        step_pe_cargo_actor_with_streams(&mut record, &mut city, &mut zero_al, &mut lfsr, slot);
    assert!(ok);
    assert!(play, "al==0 must queue SFX 0x205");

    let mut nonzero = Win95CrtRand::new(2);
    for _ in 0..10_000 {
        let mut probe = nonzero;
        if probe.random_u16() & 0xff != 0 {
            break;
        }
        let _ = nonzero.random_u16();
    }
    let mut probe = nonzero;
    assert_ne!(probe.random_u16() & 0xff, 0);
    let mut record2 = record;
    let mut lfsr2 = Win95Lfsr::new(2);
    let (_ok, play2) =
        step_pe_cargo_actor_with_streams(&mut record2, &mut city, &mut nonzero, &mut lfsr2, slot);
    assert!(!play2, "nonzero al must not queue SFX");
}

#[test]
fn pe_cargo_candidate_refuses_xtxt_at_or_above_0xc9() {
    // PE FUN_00456C70 @ 0x456cf4: after FUN_00456D40, refuse dest XTXT >= 0xc9.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/cargo-xtxt-cand.sc2"),
        "CargoXtxtCand",
    )
    .expect("city");
    // Lane 2 → offset (+4, 0).
    for x in [10usize, 14] {
        city.set_tile_bits(x, 10, crate::city::TileBits::new(0x04));
        city.set_terrain(x, 10, 0x10);
    }
    let record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        lane: 2,
        tile_x: 10,
        tile_y: 10,
        ..Default::default()
    };
    city.set_text_overlay(14, 10, XTHG_EXPLOSION_XTXT_GATE);
    assert!(!pe_cargo_candidate_in_direction(&city, &record, 2));
    city.set_text_overlay(14, 10, XTHG_EXPLOSION_XTXT_GATE - 1);
    assert!(pe_cargo_candidate_in_direction(&city, &record, 2));
}

#[test]
fn pe_cargo_candidate_accepts_oob_destination() {
    // PE FUN_00456C70 @ 0x456d04: OOB dest returns ax=1 (accept).
    let city = City::new_blank(
        std::path::Path::new("/tmp/cargo-oob-cand.sc2"),
        "CargoOobCand",
    )
    .expect("city");
    let record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        lane: 2,
        tile_x: 126,
        tile_y: 10,
        ..Default::default()
    };
    // Lane 2 → (+4, 0) → x=130 OOB.
    assert!(pe_cargo_candidate_in_direction(&city, &record, 2));
}

#[test]
fn pe_cargo_ship_spawn_initializes_state_and_paints_xtxt() {
    // PE SpawnShip `FUN_00452d00`: state 0 @ `0x452eca`, height 1, prior XTXT @ +0xa,
    // paints XTXT `slot-0x37` @ `0x452ef9`; spawn tile → globals only (not +8/+9).
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/cargo-spawn.sc2"), "CargoSpawn").expect("city");
    city.set_building(10, 10, 224);
    city.set_traffic_density(10, 10, 0x80);
    city.set_text_overlay(10, 10, 0x22);

    assert!(city.place_ambient_port_actor(10, 10, XTHG_CARGO_SHIP, 2));

    let (slot, cargo) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, record)| record.kind == XTHG_CARGO_SHIP)
        .map(|(s, r)| (s, *r))
        .expect("cargo");
    assert_eq!(cargo.state, 0);
    assert_eq!(cargo.height, 1);
    assert_eq!(cargo.saved_a, 0x22);
    assert_eq!((cargo.target_x, cargo.target_y), (0, 0));
    assert_eq!(city.pe_cargo_spawn_tile, (10, 10));
    assert_eq!((cargo.sub_x, cargo.sub_y), (8, 8));
    assert_eq!(city.traffic_density_at(10, 10), Some(0x80));
    assert_eq!(city.text_overlay_at(10, 10), xthg_proxy_for_slot(slot));
    // Free the slot but leave the occupancy proxy; SpawnShip still refuses XTXT >= 0xC9.
    city.traffic_overlays_mut().unwrap()[slot] = Win95OverlayRecord::default();
    assert!(!city.place_ambient_port_actor(10, 10, XTHG_CARGO_SHIP, 2));
}

#[test]
fn ambient_cargo_ship_enters_water_and_avoids_dry_land() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/ship-water.sc2"), "ShipWater").expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_building(10, 10, 224); // crane
    city.set_terrain(10, 10, 0x10);
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0x04));
    for x in 11..=14 {
        city.set_terrain(x, 10, 0x10);
        city.set_tile_bits(x, 10, crate::city::TileBits::new(0x04));
    }
    let mut rng = MacToolboxRng::new(1);
    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        lane: 2, // east
        state: 0,
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
    assert_eq!(record.kind, XTHG_CARGO_SHIP);
    assert_eq!(record.tile_x, 11);
    assert_eq!(record.tile_y, 10);

    // Leaving XBIT water morphs to kind-6 (PE FUN_00456760 @ 0x45680e).
    city.set_tile_bits(11, 10, crate::city::TileBits::new(0));
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        2
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.state, 0);
    assert_eq!(record.lane, 0);
}

#[test]
fn loaded_cargo_ship_route_uses_pe_watercraft_predicate() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-pe-route.sc2"),
        "ShipPeRoute",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XBIT".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0x04));

    city.set_terrain(10, 10, 0x10);
    assert!(cargo_ship_can_occupy(&city, 10, 10));

    // PE: XTER 0x10..=0x1f then XBLD — marina 0xf8 is refused.
    city.set_building(10, 10, 0xf8);
    assert!(!cargo_ship_can_occupy(&city, 10, 10));

    city.set_building(10, 10, 0x51);
    assert!(cargo_ship_can_occupy(&city, 10, 10));

    // PE: XTER outside 0x10..=0x1f refuses even with table XBLD.
    city.set_terrain(10, 10, 0);
    assert!(!cargo_ship_can_occupy(&city, 10, 10));

    city.set_terrain(10, 10, 0x10);
    city.set_building(10, 10, 223);
    assert!(!cargo_ship_can_occupy(&city, 10, 10));

    city.set_building(10, 10, 0);
    city.set_traffic_density(10, 10, MAC_TRAFFIC_HEAVY_BYTE_MIN);
    // PE `FUN_00456D40` ignores XTRF; heavy density must not refuse the route.
    assert!(cargo_ship_can_occupy(&city, 10, 10));

    city.set_traffic_density(10, 10, 0);
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0));
    assert!(!cargo_ship_can_occupy(&city, 10, 10));
}

#[test]
fn loaded_cargo_state_one_turns_and_selects_pe_next_state() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-pe-state-one.sc2"),
        "ShipPeStateOne",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0x04));
    city.set_tile_bits(14, 10, crate::city::TileBits::new(0x04));
    city.set_terrain(14, 10, 0x10);

    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        state: 1,
        lane: 1,
        tile_x: 10,
        tile_y: 10,
        target_x: 12,
        target_y: 10,
        ..Default::default()
    };
    update_pe_cargo_state_one(&mut record, &city);
    assert_eq!(record.lane, 2);
    assert_eq!(record.state, 0);

    city.set_tile_bits(14, 10, crate::city::TileBits::new(0));
    record.state = 1;
    update_pe_cargo_state_one(&mut record, &city);
    assert_eq!(record.state, 2);
}

#[test]
fn loaded_cargo_state_zero_falls_back_and_detects_port() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-pe-state-zero.sc2"),
        "ShipPeStateZero",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_building(12, 10, 0xdf);

    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        state: 0,
        lane: 2,
        tile_x: 10,
        tile_y: 10,
        target_x: 12,
        target_y: 10,
        ..Default::default()
    };
    let mut rng = MacToolboxRng::new(1);
    update_pe_cargo_state_zero(&mut record, &city, &mut rng);
    assert_eq!(record.state, 3);
}

#[test]
fn pe_cargo_state3_restores_targets_from_spawn_globals() {
    // PE @ 0x456a48: on 401cad(0x1e)==0, state=4 and +8/+9 ← [0x4e80cc]/[0x4e80d0].
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-state3-globals.sc2"),
        "ShipState3Globals",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_terrain(20, 20, 0x10);
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    assert_eq!(city.pe_cargo_spawn_tile, (20, 20));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].state = 3;
        overlays[slot].target_x = 0x55;
        overlays[slot].target_y = 0x66;
    }
    let mut lfsr = Win95Lfsr::new(1);
    let mut record = city.traffic_overlays().unwrap()[slot];
    loop {
        let mut probe = record;
        let mut probe_lfsr = lfsr;
        update_pe_cargo_state_three(&mut probe, &city, &mut probe_lfsr);
        if probe.state == 4 {
            assert_eq!(probe.target_x, 20);
            assert_eq!(probe.target_y, 20);
            break;
        }
        // Burn one LFSR roll and keep trying (nonzero rem stays in state 3).
        let _ = lfsr.random_mod(30);
        assert_eq!(probe.state, 3);
        assert_eq!(probe.target_x, 0x55);
    }
}

#[test]
fn pe_cargo_state3_skips_movement() {
    // PE state 3 RET without FUN_00456E10 whether LFSR stays or transitions.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-state3-idle.sc2"),
        "ShipState3Idle",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    for x in 18..=24 {
        for y in 18..=24 {
            city.set_terrain(x, y, 0x10);
            city.set_tile_bits(x, y, crate::city::TileBits::new(0x04));
        }
    }
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].state = 3;
        overlays[slot].sub_x = 4;
        overlays[slot].sub_y = 4;
        overlays[slot].target_x = 0x55;
        overlays[slot].target_y = 0x66;
    }
    let mut lfsr = Win95Lfsr::new(1);
    let before = city.traffic_overlays().unwrap()[slot];
    let mut record = before;
    assert!(step_pe_cargo_actor_mut(
        &mut record,
        &mut city,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.tile_x, before.tile_x);
    assert_eq!(record.tile_y, before.tile_y);
    assert_eq!(record.sub_x, before.sub_x);
    assert_eq!(record.sub_y, before.sub_y);
    if record.state == 4 {
        assert_eq!((record.target_x, record.target_y), city.pe_cargo_spawn_tile);
    } else {
        assert_eq!(record.state, 3);
        assert_eq!((record.target_x, record.target_y), (0x55, 0x66));
    }
}

#[test]
fn pe_cargo_tick_missing_xbit_water_morphs_to_kind6() {
    // PE FUN_00456760 @ 0x45680e: missing XBIT 0x04 → kind-6 / state0 / lane0 / +B0.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-xbit-morph.sc2"),
        "ShipXbitMorph",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_terrain(20, 20, 0x10);
    // No XBIT water bit.
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0));
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo");
    let xtxt_before = city.text_overlay_at(20, 20);
    let before = city.traffic_overlays().unwrap()[slot];
    let mut record = before;
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_pe_cargo_actor_mut(
        &mut record,
        &mut city,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.state, 0);
    assert_eq!(record.lane, 0);
    assert_eq!(record.saved_b, 0);
    assert_eq!(record.tile_x, before.tile_x);
    assert_eq!(record.tile_y, before.tile_y);
    assert_eq!(record.sub_x, before.sub_x);
    assert_eq!(record.sub_y, before.sub_y);
    assert_eq!(city.text_overlay_at(20, 20), xtxt_before);
}

#[test]
fn pe_cargo_state1_skips_movement() {
    // PE state 1 @ 0x456923: turn logic only, no FUN_00456E10.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-state1-idle.sc2"),
        "ShipState1Idle",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_terrain(20, 20, 0x10);
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0x04));
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].state = 1;
        overlays[slot].sub_x = 4;
        overlays[slot].sub_y = 4;
    }
    let before = city.traffic_overlays().unwrap()[slot];
    let mut record = before;
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_pe_cargo_actor_mut(
        &mut record,
        &mut city,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_CARGO_SHIP);
    assert_eq!(record.tile_x, before.tile_x);
    assert_eq!(record.tile_y, before.tile_y);
    assert_eq!(record.sub_x, before.sub_x);
    assert_eq!(record.sub_y, before.sub_y);
}

#[test]
fn pe_cargo_state4_blocked_lane_skips_movement() {
    // PE state 4 blocked arm searches lane then RET — no FUN_00456E10.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-state4-block.sc2"),
        "ShipState4Block",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_terrain(20, 20, 0x10);
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0x04));
    // Lane 2 → candidate (+4, 0); leave land so current candidate fails.
    city.set_terrain(24, 20, 0);
    city.set_tile_bits(24, 20, crate::city::TileBits::new(0));
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].state = 4;
        overlays[slot].lane = 2;
        overlays[slot].sub_x = 4;
        overlays[slot].sub_y = 4;
    }
    let before = city.traffic_overlays().unwrap()[slot];
    assert!(!pe_cargo_candidate_in_direction(&city, &before, 2));
    let mut record = before;
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_pe_cargo_actor_mut(
        &mut record,
        &mut city,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_CARGO_SHIP);
    assert_eq!(record.state, 4);
    assert_eq!(record.tile_x, before.tile_x);
    assert_eq!(record.tile_y, before.tile_y);
    assert_eq!(record.sub_x, before.sub_x);
    assert_eq!(record.sub_y, before.sub_y);
}

#[test]
fn pe_cargo_state0_blocked_lane_skips_movement() {
    // PE state 0 blocked arm sets state 1 (then crane scan) without FUN_00456E10.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-state0-block.sc2"),
        "ShipState0Block",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_terrain(20, 20, 0x10);
    city.set_tile_bits(20, 20, crate::city::TileBits::new(0x04));
    city.set_terrain(24, 20, 0);
    city.set_tile_bits(24, 20, crate::city::TileBits::new(0));
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let slot = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find_map(|(slot, record)| (record.kind == XTHG_CARGO_SHIP).then_some(slot))
        .expect("cargo");
    {
        let overlays = city.traffic_overlays_mut().unwrap();
        overlays[slot].state = 0;
        overlays[slot].lane = 2;
        overlays[slot].sub_x = 4;
        overlays[slot].sub_y = 4;
        overlays[slot].target_x = 0;
        overlays[slot].target_y = 0;
    }
    let before = city.traffic_overlays().unwrap()[slot];
    let mut record = before;
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_pe_cargo_actor_mut(
        &mut record,
        &mut city,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_CARGO_SHIP);
    assert!(
        matches!(record.state, 1 | 3),
        "blocked → state 1 (or 3 via crane)"
    );
    assert_eq!(record.tile_x, before.tile_x);
    assert_eq!(record.tile_y, before.tile_y);
    assert_eq!(record.sub_x, before.sub_x);
    assert_eq!(record.sub_y, before.sub_y);
}

#[test]
fn loaded_cargo_state_two_searches_pe_direction_order() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-pe-state-two.sc2"),
        "ShipPeStateTwo",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_tile_bits(14, 10, crate::city::TileBits::new(0x04));
    city.set_terrain(14, 10, 0x10);

    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        state: 2,
        lane: 2,
        tile_x: 10,
        tile_y: 10,
        ..Default::default()
    };
    let mut rng = MacToolboxRng::new(1);
    update_pe_cargo_state_two(&mut record, &city, &mut rng);
    assert_eq!(record.lane, 2);
    assert_eq!(record.state, 0);
}

#[test]
fn loaded_cargo_state_four_keeps_valid_lane_and_turns_when_blocked() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-pe-state-four.sc2"),
        "ShipPeStateFour",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_tile_bits(14, 10, crate::city::TileBits::new(0x04));
    city.set_terrain(14, 10, 0x10);

    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        state: 4,
        lane: 2,
        tile_x: 10,
        tile_y: 10,
        ..Default::default()
    };
    let mut rng = MacToolboxRng::new(1);
    update_pe_cargo_state_four(&mut record, &city, &mut rng);
    assert_eq!(record.lane, 2);

    city.set_tile_bits(14, 10, crate::city::TileBits::new(0));
    city.set_tile_bits(13, 13, crate::city::TileBits::new(0x04));
    city.set_terrain(13, 13, 0x10);
    record.lane = 2;
    update_pe_cargo_state_four(&mut record, &city, &mut rng);
    assert_ne!(record.lane, 2);
}

#[test]
fn loaded_cargo_movement_uses_pe_twelve_subtile_ring() {
    let city = City::new_blank(
        std::path::Path::new("/tmp/ship-pe-movement.sc2"),
        "ShipPeMovement",
    )
    .expect("city");
    let mut city = city;
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        lane: 2,
        tile_x: 10,
        tile_y: 10,
        sub_x: 10,
        sub_y: 6,
        ..Default::default()
    };
    assert!(step_pe_cargo_movement(&mut record, &city));
    assert_eq!((record.tile_x, record.tile_y), (11, 10));
    assert_eq!((record.sub_x, record.sub_y), (2, 6));

    record.lane = 6;
    record.tile_x = 10;
    record.sub_x = 2;
    assert!(step_pe_cargo_movement(&mut record, &city));
    assert_eq!((record.tile_x, record.tile_y), (9, 10));
    assert_eq!(record.sub_x, 10);
}

#[test]
fn loaded_cargo_movement_transfers_pe_xtxt_at_tile_crossing() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-pe-xtxt-movement.sc2"),
        "ShipPeXtxtMovement",
    )
    .expect("city");
    city.set_text_overlay(10, 10, 0x11);
    city.set_text_overlay(11, 10, 0x44);
    // Distinct XTRF microsim cells so later asserts stay independent.
    city.set_traffic_density(10, 10, 91);
    city.set_traffic_density(12, 10, 100);
    let slot = 2usize;
    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        lane: 2,
        tile_x: 10,
        tile_y: 10,
        sub_x: 10,
        sub_y: 6,
        saved_a: 0x11,
        ..Default::default()
    };
    // Occupy the old tile with this slot's proxy as SpawnShip would.
    city.set_text_overlay(10, 10, xthg_proxy_for_slot(slot).expect("proxy"));
    assert!(step_pe_cargo_movement_mut(&mut record, &mut city, slot));
    assert_eq!(city.text_overlay_at(10, 10), Some(0x11));
    assert_eq!(city.text_overlay_at(11, 10), xthg_proxy_for_slot(slot));
    assert_eq!(record.saved_a, 0x44);
    // XTRF must stay untouched — PE writes `0x4CA600` only.
    assert_eq!(city.traffic_density_at(10, 10), Some(91));
    assert_eq!(city.traffic_density_at(12, 10), Some(100));
}

#[test]
fn pe_cargo_tile_cross_paints_over_dest_xtxt_without_0xc9_refuse() {
    // PE FUN_00456E10 has no dest XTXT >= 0xC9 refuse (unlike FUN_004516F0).
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-xtxt-paint.sc2"),
        "ShipXtxtPaint",
    )
    .expect("city");
    let slot = 2usize;
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.set_text_overlay(10, 10, proxy);
    city.set_text_overlay(11, 10, XTHG_EXPLOSION_XTXT_GATE);
    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        lane: 2,
        tile_x: 10,
        tile_y: 10,
        sub_x: 10,
        sub_y: 6,
        saved_a: 0x11,
        ..Default::default()
    };
    assert!(step_pe_cargo_movement_mut(&mut record, &mut city, slot));
    assert_eq!(record.kind, XTHG_CARGO_SHIP);
    assert_eq!((record.tile_x, record.tile_y), (11, 10));
    assert_eq!(city.text_overlay_at(10, 10), Some(0x11));
    assert_eq!(city.text_overlay_at(11, 10), Some(proxy));
    assert_eq!(record.saved_a, XTHG_EXPLOSION_XTXT_GATE);
}

#[test]
fn blank_city_cargo_tile_cross_transfers_xtxt_occupancy() {
    // Blank maps still contain fixed records such as MISC; cargo tick must
    // hand off XTXT without relying on source assets.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/blank-cargo-xtxt.sc2"),
        "BlankCargoXtxt",
    )
    .expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    for x in 10..=14 {
        city.set_tile_bits(x, 10, crate::city::TileBits::new(0x04));
        city.set_terrain(x, 10, 0x10);
    }
    city.set_traffic_overlays_for_test(empty_overlay_table());
    let slot = XTHG_FIRST_USABLE_SLOT;
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.traffic_overlays_mut().unwrap()[slot] = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        lane: 2, // east
        state: 0,
        tile_x: 10,
        tile_y: 10,
        sub_x: 10,
        sub_y: 6,
        height: 1,
        saved_a: 0x11,
        ..Default::default()
    };
    city.set_text_overlay(10, 10, proxy);
    city.set_text_overlay(11, 10, 0x44);
    let mut sim = Win95OverlaySim::new();
    sim.reset_for_city(&city);
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(sim.tick_ambient_once(&mut city, &mut crt, &mut lfsr));
    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!((record.tile_x, record.tile_y), (11, 10));
    assert_eq!(city.text_overlay_at(10, 10), Some(0x11));
    assert_eq!(city.text_overlay_at(11, 10), Some(proxy));
    assert_eq!(record.saved_a, 0x44);
}

#[test]
fn pe_cargo_kind_clear_zeros_xtxt_not_restore_saved_a() {
    // PE `FUN_00456C70` state-2 @ `0x456a25` → `FUN_00401528` zeros XTXT;
    // it does not write record `+A` back onto `0x4CA600`.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/ship-pe-xtxt-clear.sc2"),
        "ShipPeXtxtClear",
    )
    .expect("city");
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0x04));
    let slot = 2usize;
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.set_text_overlay(10, 10, proxy);
    let mut record = Win95OverlayRecord {
        kind: 0, // already cleared by state-2 arm
        tile_x: 10,
        tile_y: 10,
        saved_a: 0x22,
        ..Default::default()
    };
    let mut rng = MacToolboxRng::new(1);
    assert!(step_pe_cargo_actor_mut(
        &mut record,
        &mut city,
        &mut rng,
        slot
    ));
    assert_eq!(city.text_overlay_at(10, 10), Some(0));
}

#[test]
fn pe_cargo_oob_restores_saved_a_keeps_kind() {
    // PE FUN_00456E10 OOB restores +A and keeps the cargo actor (no FUN_00401528).
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/ship-oob.sc2"), "ShipOob").expect("city");
    let slot = 2usize;
    city.set_text_overlay(0, 10, xthg_proxy_for_slot(slot).expect("proxy"));
    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        lane: 6, // west
        tile_x: 0,
        tile_y: 10,
        sub_x: 2,
        sub_y: 6,
        saved_a: 0x33,
        ..Default::default()
    };
    assert!(step_pe_cargo_movement_mut(&mut record, &mut city, slot));
    assert_eq!(record.kind, XTHG_CARGO_SHIP);
    assert_eq!((record.tile_x, record.tile_y), (0, 10));
    assert_eq!(city.text_overlay_at(0, 10), Some(0x33));
}

#[test]
fn ambient_ship_wanders_on_open_water() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/ship-wander.sc2"), "ShipWander").expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    for y in 3..=20 {
        for x in 3..=20 {
            city.set_terrain(x, y, 0x10);
            city.set_tile_bits(x, y, crate::city::TileBits::new(0x04));
        }
    }
    let mut turned = false;
    for seed in 0..128u32 {
        let mut rng = MacToolboxRng::new(seed);
        let mut record = Win95OverlayRecord {
            kind: XTHG_CARGO_SHIP,
            lane: 2,
            state: 0,
            tile_x: 11,
            tile_y: 11,
            sub_x: 14,
            sub_y: SUBTILE_CENTER,
            height: city.altitude_at(11, 11),
            ..Default::default()
        };
        // Cargo's PE state rolls use the process LFSR, not the generic
        // Toolbox RNG passed for other ambient actors.
        let mut lfsr = Win95Lfsr::new(seed as u16);
        assert!(step_ambient_actor(
            &mut record,
            &mut city,
            &mut rng,
            &mut lfsr,
            2
        ));
        assert!(
            (3..=20).contains(&record.tile_x) && (3..=20).contains(&record.tile_y),
            "the valid open-water route should keep the actor in bounds"
        );
        if record.lane != 2 {
            turned = true;
            break;
        }
    }
    assert!(
        turned,
        "open-water crossings should occasionally change heading"
    );
}

#[test]
fn loaded_cargo_ship_turns_toward_southern_target_when_blocked() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/ship-turn.sc2"), "ShipTurn").expect("city");
    city.chunks.push(crate::city::CityChunk {
        name: "XTHG".to_string(),
        offset: 0,
        raw_len: 0,
        data: Vec::new(),
    });
    city.set_terrain(10, 10, 0x10);
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0x04));
    let mut rng = MacToolboxRng::new(1);
    let mut record = Win95OverlayRecord {
        kind: XTHG_CARGO_SHIP,
        lane: 2, // east into dry land
        state: 1,
        tile_x: 10,
        tile_y: 10,
        target_x: 10,
        target_y: 14,
        sub_x: 14,
        sub_y: SUBTILE_CENTER,
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
    assert_eq!(record.kind, XTHG_CARGO_SHIP);
    assert_eq!(record.tile_x, 10);
    assert_eq!(record.lane, 3, "turn one octant toward the southern target");
}
