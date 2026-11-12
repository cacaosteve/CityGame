use super::*;

#[test]
fn ambient_port_spawn_uses_pe_kind_height_table() {
    // PE `FUN_00450690` / `0x4E8100` table: airplane/cargo 6, heli 4; spawn
    // overrides: plane/heli/sail → 0, cargo → 1.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/port-height.sc2"), "PortHeight").expect("city");
    city.set_building(20, 20, 224);
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
    let cargo = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find(|r| r.kind == XTHG_CARGO_SHIP)
        .expect("cargo");
    assert_eq!(cargo.height, 1, "SpawnShip writes +5=1, not table 6");
    assert_ne!(xthg_spawn_height(XTHG_CARGO_SHIP), 1);
    assert_ne!(cargo.height, city.altitude_at(20, 20));

    let mut city =
        City::new_blank(std::path::Path::new("/tmp/heli-height.sc2"), "HeliHeight").expect("city");
    city.set_text_overlay(10, 10, 0x33);
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 0));
    let (slot, heli) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    assert_eq!(heli.height, 0, "SpawnHelicopter writes +5=0, not table 4");
    assert_eq!(heli.state, 0, "SpawnHelicopter writes +2=0");
    assert_eq!(heli.saved_a, 0x33);
    assert_eq!(city.text_overlay_at(10, 10), xthg_proxy_for_slot(slot));
    assert_ne!(xthg_spawn_height(XTHG_HELICOPTER), 0);

    let mut city = City::new_blank(std::path::Path::new("/tmp/heli-targets.sc2"), "HeliTargets")
        .expect("city");
    let mut crt = Win95CrtRand::new(0x1234_5678);
    let mut expect = crt;
    let tx = (expect.random_u16() & 0x7f) as u8;
    let ty = (expect.random_u16() & 0x7f) as u8;
    assert!(city.place_ambient_port_actor_with_crt(10, 10, XTHG_HELICOPTER, 0, Some(&mut crt)));
    let heli = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find(|r| r.kind == XTHG_HELICOPTER)
        .expect("heli");
    assert_eq!((heli.target_x, heli.target_y), (tx, ty));
    assert_eq!(crt.seed(), expect.seed());

    // PE `0x4528ad` selects airport vs edge; edge then draws a selector and
    // one jitter value at exactly one of `0x4528e8/0x45291c/0x45294d/0x45297e`.
    for expected_case in 0u16..4 {
        let edge_seed = (1..10_000u32)
            .find(|seed| {
                let mut probe = Win95CrtRand::new(*seed);
                probe.random_u16() % 10 < 5 && probe.random_u16() & 3 == expected_case
            })
            .expect("seed for each PE edge-table arm");
        let mut city = City::new_blank(std::path::Path::new("/tmp/plane-edge.sc2"), "PlaneEdge")
            .expect("city");
        let mut slots = empty_overlay_table();
        slots[1].saved_b = 0x17;
        city.set_traffic_overlays_for_test(slots);
        let mut crt = Win95CrtRand::new(edge_seed);
        let mut expect = crt;
        assert!(expect.random_u16() % 10 < 5);
        assert_eq!(expect.random_u16() & 3, expected_case);
        let jitter = (expect.random_u16() % 100) as u8 + 10;
        let (tile_x, tile_y, lane) = match expected_case {
            0 => (0u8, jitter, 3u8),
            1 => (jitter, 0, 5),
            2 => (0x7f, jitter, 7),
            _ => (jitter, 0x7f, 1),
        };
        assert!(city.place_ambient_port_actor_with_crt(40, 40, XTHG_AIRPLANE, 2, Some(&mut crt),));
        let plane = city
            .traffic_overlays()
            .unwrap()
            .iter()
            .skip(XTHG_FIRST_USABLE_SLOT)
            .find(|r| r.kind == XTHG_AIRPLANE)
            .expect("plane");
        assert_eq!(plane.height, crate::win95_overlay::XTHG_AIRCRASH_HEIGHT);
        assert_eq!((plane.tile_x, plane.tile_y), (tile_x, tile_y));
        assert_eq!(plane.lane, lane);
        assert_eq!(plane.saved_b, 0x17);
        assert_eq!(plane.state, 2u8.wrapping_shl(4).wrapping_add(3));
        assert_eq!(crt.seed(), expect.seed());
    }

    // PE airport arm (`rand()%10 >= 5`) consumes only the first draw and
    // keeps the caller coordinate with the airport-state target pair.
    let airport_seed = (1..10_000u32)
        .find(|seed| {
            let mut probe = Win95CrtRand::new(*seed);
            probe.random_u16() % 10 >= 5
        })
        .expect("seed for PE airport arm");
    let mut airport = City::new_blank(
        std::path::Path::new("/tmp/plane-airport.sc2"),
        "PlaneAirport",
    )
    .expect("city");
    let mut slots = empty_overlay_table();
    slots[1].saved_b = 0x17;
    airport.set_traffic_overlays_for_test(slots);
    let mut crt = Win95CrtRand::new(airport_seed);
    let mut expect = crt;
    assert!(expect.random_u16() % 10 >= 5);
    assert!(airport.place_ambient_port_actor_with_crt(40, 40, XTHG_AIRPLANE, 2, Some(&mut crt),));
    let plane = airport
        .traffic_overlays()
        .unwrap()
        .iter()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find(|r| r.kind == XTHG_AIRPLANE)
        .expect("airport plane");
    assert_eq!((plane.tile_x, plane.tile_y), (40, 40));
    assert_eq!(plane.height, 0);
    assert_eq!(plane.saved_b, 0x17);
    assert_eq!(plane.state, 0);
    assert_eq!((plane.target_x, plane.target_y), (0x14, 0x14));
    assert_eq!(crt.seed(), expect.seed());

    let mut city = City::new_blank(std::path::Path::new("/tmp/plane-height.sc2"), "PlaneHeight")
        .expect("city");
    city.set_text_overlay(10, 10, 0x22);
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, plane) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    assert_eq!(plane.height, 0, "SpawnAeroplane airport arm writes +5=0");
    assert_eq!(plane.state, 0);
    assert_eq!(plane.saved_a, 0x22);
    assert_eq!((plane.target_x, plane.target_y), (0x14, 0x14));
    assert_eq!(city.text_overlay_at(10, 10), xthg_proxy_for_slot(slot));
    assert_ne!(xthg_spawn_height(XTHG_AIRPLANE), 0);

    let mut city =
        City::new_blank(std::path::Path::new("/tmp/sail-height.sc2"), "SailHeight").expect("city");
    city.set_terrain(10, 10, 0x10);
    assert!(city.place_ambient_port_actor(10, 10, XTHG_SAILBOAT, 2));
    let sail = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .skip(XTHG_FIRST_USABLE_SLOT)
        .find(|r| r.kind == XTHG_SAILBOAT)
        .expect("sail");
    assert_eq!(sail.height, 0, "SpawnSailBoat forces height 0 after alloc");
}

#[test]
fn ambient_port_spawn_caps_match_pe_ship_limits() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/ship-caps.sc2"), "ShipCaps").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_CARGO_SHIP, 2));
    assert!(!city.place_ambient_port_actor(11, 10, XTHG_CARGO_SHIP, 2));
    let cargo = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .find(|record| record.kind == XTHG_CARGO_SHIP)
        .copied()
        .expect("cargo ship");
    assert_eq!((cargo.target_x, cargo.target_y), (0, 0));
    assert_eq!(city.pe_cargo_spawn_tile, (10, 10));

    // PE SpawnHelicopter `0x4e99c4 < 1`; SpawnAeroplane `0x4e99c0 < 2`.
    assert!(city.place_ambient_port_actor(30, 30, XTHG_HELICOPTER, 0));
    assert!(!city.place_ambient_port_actor(31, 30, XTHG_HELICOPTER, 0));
    assert!(city.place_ambient_port_actor(40, 40, XTHG_AIRPLANE, 2));
    assert!(city.place_ambient_port_actor(41, 40, XTHG_AIRPLANE, 2));
    assert!(!city.place_ambient_port_actor(42, 40, XTHG_AIRPLANE, 2));

    for x in 20..24 {
        assert!(city.place_ambient_port_actor(x, 20, XTHG_SAILBOAT, 2));
    }
    assert!(!city.place_ambient_port_actor(24, 20, XTHG_SAILBOAT, 2));
}

#[test]
fn pe_plane_heli_spawn_refuse_xtxt_at_or_above_0xc9() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/xtxt-gate.sc2"), "XtxtGate").expect("city");
    city.set_text_overlay(10, 10, XTHG_EXPLOSION_XTXT_GATE);
    assert!(!city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    assert!(!city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 0));
    assert!(!city.place_ambient_port_actor(10, 10, XTHG_CARGO_SHIP, 2));
    city.set_text_overlay(10, 10, XTHG_EXPLOSION_XTXT_GATE - 1);
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
}

#[test]
fn pe_plane_heli_spawn_refuse_when_monster_active() {
    // PE `0x4e99d4 != 0` at SpawnAeroplane `0x452854` / SpawnHelicopter `0x452bc4`.
    let mut city = City::new_blank(std::path::Path::new("/tmp/monster-gate.sc2"), "MonsterGate")
        .expect("city");
    assert!(city.place_pe_monster_actor(40, 40, 50, 50, 0, 0).is_some());
    assert!(!city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    assert!(!city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 0));
    // Cargo/sail ignore the monster word.
    assert!(city.place_ambient_port_actor(20, 20, XTHG_CARGO_SHIP, 2));
}

#[test]
fn ambient_spawn_direction_aims_ship_at_water_and_train_at_rail() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/spawn-dir.sc2"), "SpawnDir").expect("city");
    city.set_building(10, 10, 224); // crane
    city.set_terrain(10, 12, 0x10); // water two south (pier corridor)
    city.set_building(10, 11, 223); // pier
    assert_eq!(
        ambient_spawn_direction(XTHG_CARGO_SHIP, &city, 10, 10, 2),
        4,
        "ship should face water via pier"
    );
    assert_eq!(
        ambient_spawn_direction(XTHG_CARGO_SHIP, &city, 10, 10, 2),
        ambient_spawn_direction(XTHG_SAILBOAT, &city, 10, 10, 0),
        "sailboat uses the same water goal search"
    );

    city.set_building(20, 20, 237); // depot
    city.set_building(19, 20, 45); // rail west
    assert_eq!(
        ambient_spawn_direction(XTHG_TRAIN_ENGINE, &city, 20, 20, 2),
        6,
        "train should face adjacent rail"
    );
    assert_eq!(
        ambient_spawn_direction(XTHG_TRAIN_ENGINE, &city, 30, 30, 2),
        2,
        "no rail nearby keeps east fallback"
    );
}

#[test]
fn xthg_direction_delta_matches_compass() {
    assert_eq!(xthg_direction_delta(0), (0, -1));
    assert_eq!(xthg_direction_delta(2), (1, 0));
    assert_eq!(xthg_direction_delta(4), (0, 1));
    assert_eq!(xthg_direction_delta(6), (-1, 0));
}
