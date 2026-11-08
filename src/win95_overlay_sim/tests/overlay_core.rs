use super::*;

#[test]
fn pe_ambient_dispatcher_uses_pe_kind_three_on_mac_layout() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-native-type-three.sc2", "MacNative").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let record = Win95OverlayRecord {
        kind: 3,
        lane: 5,
        state: 2,
        tile_x: 0x42,
        tile_y: 0x43,
        height: 1,
        sub_x: 8,
        sub_y: 8,
        saved_a: 0xe0,
        ..Win95OverlayRecord::default()
    };
    city.traffic_overlays_mut().unwrap()[1] = record;
    let mut direct = city.clone();
    let mut sim = Win95OverlaySim::new();
    let mut direct_sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(0x1234);
    let mut lfsr = Win95Lfsr::new(0x5678);
    let mut game_rand = Win95GameRand::new(0x9abc);
    let mut direct_crt = Win95CrtRand::new(0x1234);
    let mut direct_lfsr = Win95Lfsr::new(0x5678);

    assert!(sim.tick_ambient_once_with_game_rand(&mut city, &mut crt, &mut lfsr, &mut game_rand));
    assert!(direct_sim.tick_pe_cargo_ship_slot(&mut direct, 1, &mut direct_crt, &mut direct_lfsr));
    assert_eq!(
        city.traffic_overlays().unwrap()[1],
        direct.traffic_overlays().unwrap()[1]
    );
    assert_eq!(
        (crt.seed(), lfsr.state()),
        (direct_crt.seed(), direct_lfsr.state())
    );
}

#[test]
fn subtile_wrap_crosses_tile_boundary() {
    let (sub, delta) = advance_subtile(17);
    assert_eq!(sub, 1);
    assert_eq!(delta, 1);
    let (sub, delta) = advance_subtile(-1);
    assert_eq!(sub, 15);
    assert_eq!(delta, -1);
}

#[test]
fn ambient_plane_heli_subtile_steps_match_pe_4e81f8() {
    // PE plane push 1 → 0x4E81F8[1]=0x10; heli push 2 → [2]=0x08.
    // Cargo/train arms are not 0x4E81F8 callers (see ambient_subtile_step).
    assert_eq!(ambient_subtile_step(XTHG_AIRPLANE), 0x10);
    assert_eq!(ambient_subtile_step(XTHG_HELICOPTER), 0x08);
    assert_eq!(ambient_subtile_step(XTHG_SAILBOAT), 2);
}

#[test]
fn spawn_pending_promotes_to_light() {
    let mut table = empty_overlay_table();
    let slot = XTHG_FIRST_USABLE_SLOT;
    table[slot] = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::SpawnPending as u8,
        tile_x: 10,
        tile_y: 10,
        sub_x: SUBTILE_CENTER,
        sub_y: SUBTILE_CENTER,
        saved_a: 0x90,
        ..Default::default()
    };
    let mut city = City::new_blank(std::path::Path::new("/tmp/sim.sc2"), "Sim").expect("city");
    city.set_traffic_overlays_for_test(table);
    city.set_building(10, 10, 29);
    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(sim.tick_once(&mut city, &mut crt, &mut lfsr));
    let record = city.traffic_overlays().unwrap()[slot];
    assert_eq!(record.overlay_state(), OverlayState::LightActive);
    assert_eq!(record.sub_x, SUBTILE_CENTER);
    assert_eq!(record.saved_a, 0x90);
}

#[test]
fn traffic_tick_skips_reserved_slot_zero() {
    let mut table = empty_overlay_table();
    table[0] = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::SpawnPending as u8,
        tile_x: 10,
        tile_y: 10,
        saved_a: 0x90,
        ..Default::default()
    };
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/reserved.sc2"), "Reserved").expect("city");
    city.set_traffic_overlays_for_test(table);
    city.set_building(10, 10, 29);
    let before = city.traffic_overlays().unwrap()[0];

    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(!sim.tick_once(&mut city, &mut crt, &mut lfsr));
    assert_eq!(city.traffic_overlays().unwrap()[0], before);
}

#[test]
fn traffic_tick_does_not_mutate_dispatch_actor() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/dispatch.sc2"), "Dispatch").expect("city");
    for y in 100..103 {
        for x in 100..103 {
            city.set_building(x, y, 210);
        }
    }
    city.pe_rebuild_dw_tile_count_434f80();
    assert!(city.place_dispatch_actor(10, 10, DispatchKind::Police));
    let before = city.traffic_overlays().unwrap()[1];

    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(!sim.tick_once(&mut city, &mut crt, &mut lfsr));

    assert_eq!(city.traffic_overlays().unwrap()[1], before);
    assert!(city.dispatch_actor_at(10, 10).is_some());
}

#[test]
fn tick_ambient_skips_pe_aircrash_disaster_airplane() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/aircrash-skip.sc2"), "Crash").expect("city");
    let slot = city
        .place_pe_aircrash_actor(40, 40)
        .expect("aircrash actor");
    let before = city.traffic_overlays().unwrap()[slot];
    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(!sim.tick_ambient(&mut city, GameSpeed::Llama, 1.0, &mut crt, &mut lfsr));
    let after = city.traffic_overlays().unwrap()[slot];
    assert_eq!(
        after, before,
        "PE air-crash state 7 must not use ambient airplane step"
    );
    assert!(after.is_pe_disaster_xthg_actor());
}

#[test]
fn overlay_height_selects_altm_field_using_pe_xbit_bit_2() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/overlay-height-xbit.sc2"),
        "OverlayHeightXbit",
    )
    .expect("city");
    let mut record = Win95OverlayRecord {
        kind: XTHG_MAXIS_MAN,
        state: 3,
        tile_x: 10,
        tile_y: 10,
        height: 77,
        saved_b: SAVED_B_SLOT_LINK,
        ..Default::default()
    };
    let table = empty_overlay_table();

    city.set_tile_bits(10, 10, crate::city::TileBits::new(0));
    refresh_overlay_height(&mut record, &city, &table);
    assert_eq!(
        record.height,
        (altm_packed_word(&city, 10, 10) as u8 & 0x1f).wrapping_add(record.state)
    );

    city.set_tile_bits(10, 10, crate::city::TileBits::new(0x04));
    refresh_overlay_height(&mut record, &city, &table);
    assert_eq!(
        record.height,
        (((altm_packed_word(&city, 10, 10) >> 5) & 0x1f) as u8).wrapping_add(record.state)
    );
}
