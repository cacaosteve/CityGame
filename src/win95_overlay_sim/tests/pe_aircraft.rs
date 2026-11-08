use super::*;

#[test]
fn pe_helicopter_tile_cross_transfers_xtxt_occupancy() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/heli-xtxt-move.sc2"), "HeliXtxt").expect("city");
    city.set_text_overlay(10, 10, 0x11);
    city.set_text_overlay(11, 10, 0x44);
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    assert_eq!(city.text_overlay_at(10, 10), xthg_proxy_for_slot(slot));
    // Past climb: free-fly state 2. Heli quantum 0x08; sub 13 → one east cross.
    record.state = 2;
    record.height = 0xa;
    record.sub_x = 13;
    record.lane = 2;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!((record.tile_x, record.tile_y), (11, 10));
    assert_eq!(city.text_overlay_at(10, 10), Some(0x11));
    assert_eq!(city.text_overlay_at(11, 10), xthg_proxy_for_slot(slot));
    assert_eq!(record.saved_a, 0x44);
}

#[test]
fn pe_heli_state0_climbs_height_until_0xa() {
    // PE FUN_004540C0 state 0: lane=(lane+1)&7; height++ until 0xa then state=2.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/heli-climb.sc2"), "HeliClimb").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    assert_eq!(record.state, 0);
    assert_eq!(record.height, 0);
    let start_lane = record.lane;
    let (tx, ty) = (record.tile_x, record.tile_y);
    let (sx, sy) = (record.sub_x, record.sub_y);
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    for expected_h in 1u8..=0xa {
        assert!(step_ambient_actor(
            &mut record,
            &mut city,
            &mut rng,
            &mut lfsr,
            slot
        ));
        assert_eq!(record.height, expected_h);
        assert_eq!(record.state, 0);
        assert_eq!((record.tile_x, record.tile_y), (tx, ty));
        assert_eq!((record.sub_x, record.sub_y), (sx, sy));
    }
    assert_eq!(record.lane, start_lane.wrapping_add(0xa) & 7);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.height, 0xa);
    assert_eq!(record.state, 2);
    assert_eq!(record.lane, start_lane.wrapping_add(0xb) & 7);
    assert_eq!((record.tile_x, record.tile_y), (tx, ty));
}

#[test]
fn pe_heli_state3_descends_height_until_2() {
    // PE FUN_004540C0 state 3: lane spin; height-- while >2; else state=4.
    let mut city = City::new_blank(std::path::Path::new("/tmp/heli-descend.sc2"), "HeliDescend")
        .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    record.state = 3;
    record.height = 0xa;
    let start_lane = record.lane;
    let (tx, ty) = (record.tile_x, record.tile_y);
    let (sx, sy) = (record.sub_x, record.sub_y);
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    for expected_h in (2u8..=9).rev() {
        assert!(step_ambient_actor(
            &mut record,
            &mut city,
            &mut rng,
            &mut lfsr,
            slot
        ));
        assert_eq!(record.height, expected_h);
        assert_eq!(record.state, 3);
        assert_eq!((record.tile_x, record.tile_y), (tx, ty));
        assert_eq!((record.sub_x, record.sub_y), (sx, sy));
    }
    assert_eq!(record.lane, start_lane.wrapping_add(8) & 7);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.height, 2);
    assert_eq!(record.state, 4);
    assert_eq!(record.lane, start_lane.wrapping_add(9) & 7);
    assert_eq!((record.tile_x, record.tile_y), (tx, ty));
}

#[test]
fn pe_heli_state4_crt_mod_20_returns_to_state0() {
    // PE @ 0x454384: CRT rand()%0x14==0 → state 0; else idle (no move).
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/heli-state4.sc2"), "HeliState4").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    record.state = 4;
    record.height = 2;
    let (tx, ty) = (record.tile_x, record.tile_y);
    let mut lfsr = Win95Lfsr::new(1);

    let mut rng = NonZeroLowRng;
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.state, 4);
    assert_eq!((record.tile_x, record.tile_y), (tx, ty));

    let mut rng = ZeroModRng;
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.state, 0);
    assert_eq!((record.tile_x, record.tile_y), (tx, ty));
}

#[test]
fn pe_heli_state5_descends_then_morphs_to_kind6() {
    // PE @ 0x4543aa: lane+=2; height-- while >2; else kind-6 / state 0x11 / +B=1 / lane=0.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/heli-state5.sc2"), "HeliState5").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    record.state = 5;
    record.height = 5;
    record.lane = 1;
    let (tx, ty) = (record.tile_x, record.tile_y);
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);

    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_HELICOPTER);
    assert_eq!(record.height, 4);
    assert_eq!(record.lane, 3);
    assert_eq!((record.tile_x, record.tile_y), (tx, ty));

    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.height, 3);
    assert_eq!(record.lane, 5);

    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.height, 2);
    assert_eq!(record.lane, 7);

    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.state, 0x11);
    assert_eq!(record.saved_b, 1);
    assert_eq!(record.lane, 0);
    assert_eq!(record.height, 2);
    assert_eq!(city.text_overlay_at(10, 10), Some(proxy));
}

#[test]
fn pe_heli_state2_steers_then_rerolls_toward_state3() {
    // PE FUN_004540C0 state 2: turn toward +8/+9, move; manhattan <2 → re-roll
    // targets; rand&1 + empty XBLD/XTER → state 3. GetTickCount arm skipped.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/heli-state2.sc2"), "HeliState2").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    city.set_building(10, 10, 0);
    city.set_terrain(10, 10, 0);
    record.state = 2;
    record.height = 0xa;
    record.lane = 2; // already facing east target
    record.sub_x = 0;
    record.sub_y = 0;
    record.target_x = 11;
    record.target_y = 10;
    let mut lfsr = Win95Lfsr::new(1);

    // Exact 0x10 sub: no tile cross; manhattan 1 < 2 → re-roll. ZeroModRng: keep state 2.
    let mut rng = ZeroModRng;
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.state, 2);
    assert_eq!(record.lane, 2);
    assert_eq!((record.tile_x, record.tile_y), (10, 10));
    assert_eq!((record.target_x, record.target_y), (0x20, 0x20));

    // At target with odd CRT low bit + empty maps → state 3.
    record.target_x = 10;
    record.target_y = 10;
    record.sub_x = 0;
    record.sub_y = 0;
    struct OnesRng;
    impl Win95OverlayRng for OnesRng {
        fn random_i16(&mut self) -> i16 {
            1
        }
        fn random_u16(&mut self) -> u16 {
            1
        }
        fn random_bit(&mut self) -> bool {
            true
        }
        fn roll_mod_is_zero(&mut self, _modulus: u16) -> bool {
            false
        }
    }
    let mut rng = OnesRng;
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.state, 3);
    assert_eq!((record.target_x, record.target_y), (0x21, 0x21));
}

#[test]
fn pe_heli_state2_avoids_xbld_ff_three_tiles_ahead() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/heli-lookahead.sc2"),
        "HeliLookahead",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, record)| record.kind == XTHG_HELICOPTER)
        .map(|(slot, record)| (slot, *record))
        .expect("helicopter");
    record.state = 2;
    record.lane = 2; // target-directed lane is east
    record.target_x = 80;
    record.target_y = 10;
    record.sub_x = 0;
    record.sub_y = 0;
    city.set_building(13, 10, 0xfb); // current heading's three-tile lookahead

    let mut rng = ZeroModRng;
    let mut lfsr = Win95Lfsr::new(1);
    let lfsr_before = lfsr.state();
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot,
    ));

    assert_eq!(record.lane, 3, "first clear offset is current + 1");
    assert_eq!((record.tile_x, record.tile_y), (10, 10));
    assert_eq!((record.sub_x, record.sub_y), (8, 8));
    assert_eq!(record.state, 2);
    assert_eq!(
        lfsr.state(),
        lfsr_before,
        "lookahead consumes no random draw"
    );
}

#[test]
fn pe_heli_state2_reroll_uses_w_city_center() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/heli-center.sc2"), "HeliCenter").expect("city");
    // Single building pulls wCityCenter away from the blank-map midpoint.
    city.set_building(90, 90, 112);
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    city.set_building(10, 10, 0);
    city.set_terrain(10, 10, 0);
    record.state = 2;
    record.height = 0xa;
    record.lane = 2;
    record.sub_x = 0;
    record.sub_y = 0;
    record.target_x = 11;
    record.target_y = 10;
    let mut lfsr = Win95Lfsr::new(1);
    let mut rng = ZeroModRng;
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!((record.target_x, record.target_y), (0x0c, 0x0c)); // (90 / 4 * 2) - 0x20
    assert_ne!((record.target_x, record.target_y), (0x20, 0x20));
}

#[test]
fn pe_heli_state1_is_idle_noop() {
    // PE jump table state 1 → 0x454415 RET (no lane/height/tile change).
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/heli-state1.sc2"), "HeliState1").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    record.state = 1;
    record.height = 0xa;
    record.lane = 3;
    record.sub_x = 13;
    let before = record;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, before.kind);
    assert_eq!(record.state, 1);
    assert_eq!(record.height, before.height);
    assert_eq!(record.lane, before.lane);
    assert_eq!(
        (record.tile_x, record.tile_y),
        (before.tile_x, before.tile_y)
    );
    assert_eq!((record.sub_x, record.sub_y), (before.sub_x, before.sub_y));
}

#[test]
fn pe_heli_converts_to_kind6_when_xbld_ge_0xfb() {
    // PE FUN_004540C0: XBLD >= 0xfb → in-place kind-6/state-5; XTXT unchanged.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/heli-xbld-fb.sc2"), "HeliXbldFb").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.set_building(10, 10, 0xfb);
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.state, 5);
    assert_eq!(record.lane, 0);
    assert_eq!(record.saved_b, 0);
    assert_eq!((record.tile_x, record.tile_y), (10, 10));
    assert_eq!(city.text_overlay_at(10, 10), Some(proxy));
}

#[test]
fn pe_plane_converts_to_kind6_when_xbld_ge_0xfb() {
    // PE FUN_00453980: XBLD >= 0xfb and XZON&0xf != 8 → in-place kind-6.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/plane-xbld-fb.sc2"),
        "PlaneXbldFb",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.set_building(10, 10, 0xfb);
    city.set_zone(10, 10, 1);
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.state, 5);
    assert_eq!(record.lane, 0);
    assert_eq!(record.saved_b, 0); // seed 1 → 0x401cad(0x10) rem 2
    assert_eq!(city.text_overlay_at(10, 10), Some(proxy));
}

#[test]
fn pe_plane_height_table_arm_morphs_when_width_div3_exceeds_height() {
    // PE @ 0x453a4c: XBLD 0xc9 width 32 → 32/3=10; height 9 morphs, +B=1 fixed.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/plane-height-table.sc2"),
        "PlaneHtTbl",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.set_building(10, 10, 0xc9);
    city.set_zone(10, 10, 1);
    record.height = 9;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let before_lfsr = lfsr.state();
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.state, 5);
    assert_eq!(record.lane, 0);
    assert_eq!(record.saved_b, 1);
    assert_eq!(
        lfsr.state(),
        before_lfsr,
        "mid-range arm does not consume LFSR"
    );
    assert_eq!(city.text_overlay_at(10, 10), Some(proxy));
}

#[test]
fn pe_plane_height_table_arm_skips_when_width_div3_le_height() {
    // 0xc9 width/3=10; height 10 → jbe continue (no morph).
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/plane-height-skip.sc2"),
        "PlaneHtSkip",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (_, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    city.set_building(10, 10, 0xc9);
    city.set_zone(10, 10, 1);
    record.height = 10;
    record.state = 2;
    let kind_before = record.kind;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        1
    ));
    assert_eq!(record.kind, kind_before);
}

#[test]
fn pe_plane_kind6_morph_saved_b_from_lfsr_mod_16() {
    // PE @ 0x453a0d: 0x401cad(0x10) → +B = 0 if rem != 0 else 1.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/plane-kind6-b.sc2"),
        "PlaneKind6B",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, record0) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    city.set_building(10, 10, 0xfb);
    city.set_zone(10, 10, 1);

    let mut record = record0;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(8); // rem 0 → saved_b 1
    let before = lfsr.state();
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.saved_b, 1);
    assert_ne!(lfsr.state(), before);

    record = record0;
    record.kind = XTHG_AIRPLANE;
    record.state = 0;
    record.lane = 2;
    record.saved_b = 0xff;
    let mut lfsr = Win95Lfsr::new(1); // rem 2 → saved_b 0
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.saved_b, 0);
}

#[test]
fn pe_plane_state0_moves_then_climbs_height_until_0xe() {
    // PE FUN_00453980 state 0: one FUN_00403071 move, then height++ while <0xe else state 2.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/plane-takeoff.sc2"),
        "PlaneTakeoff",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    assert_eq!(record.state, 0);
    assert_eq!(record.height, 0);
    record.lane = 2;
    record.sub_x = 0;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    for expected_h in 1u8..=0xe {
        let before = (record.tile_x, record.tile_y, record.sub_x);
        assert!(step_ambient_actor(
            &mut record,
            &mut city,
            &mut rng,
            &mut lfsr,
            slot
        ));
        assert_eq!(record.kind, XTHG_AIRPLANE);
        assert_eq!(record.height, expected_h);
        assert_eq!(record.state, 0);
        assert_ne!(
            (record.tile_x, record.tile_y, record.sub_x),
            before,
            "state 0 must call the move helper each tick"
        );
    }
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.height, 0xe);
    assert_eq!(record.state, 2);
}

#[test]
fn pe_plane_state1_descends_then_morphs_or_clears_on_runway() {
    // PE FUN_00453980 state 1: move, height--; at 0 restore +A; XBLD 0xdd clear else kind-6.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/plane-land.sc2"), "PlaneLand").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    record.state = 1;
    record.height = 1;
    record.lane = 2;
    // Exact 0x10 sub after step: no tile cross (PE leaves 0x10 intact).
    record.sub_x = 0;
    record.saved_a = 0x33;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);

    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.state, 5);
    assert_eq!(record.saved_b, 1);
    assert_eq!(record.lane, 0);
    assert_eq!(record.height, 0);
    assert_eq!((record.tile_x, record.tile_y), (10, 10));
    assert_eq!(city.text_overlay_at(10, 10), Some(0x33));

    // Descend-only tick before touchdown.
    assert!(city.place_ambient_port_actor(20, 20, XTHG_AIRPLANE, 2));
    let (slot_d, mut descending) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE && r.tile_x == 20)
        .map(|(s, r)| (s, *r))
        .expect("descending");
    descending.state = 1;
    descending.height = 2;
    descending.lane = 2;
    descending.sub_x = 0;
    assert!(step_ambient_actor(
        &mut descending,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot_d
    ));
    assert_eq!(descending.kind, XTHG_AIRPLANE);
    assert_eq!(descending.height, 1);
    assert_eq!(descending.state, 1);
    assert_eq!((descending.tile_x, descending.tile_y), (20, 20));

    // Runway touchdown clears via FUN_00401528.
    descending.state = 1;
    descending.height = 1;
    descending.lane = 2;
    descending.sub_x = 0;
    descending.saved_a = 0x44;
    city.set_building(20, 20, 0xdd);
    // The plane's pre-dispatch morph gate exempts XZON type 8 (airport).
    city.set_zone(20, 20, 8);
    assert!(step_ambient_actor(
        &mut descending,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot_d
    ));
    assert_eq!(descending.kind, 0);
    assert_eq!((descending.tile_x, descending.tile_y), (20, 20));
    assert_eq!(city.text_overlay_at(20, 20), Some(0));
}

#[test]
fn pe_plane_state4_approaches_then_enters_land_state1() {
    // PE FUN_00453980 state 4: turn toward +8/+9, move; manhattan <2 → lane=state>>4, state=1.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/plane-approach.sc2"),
        "PlaneApproach",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    record.state = (2 << 4) | 4; // packed land-lane east + approach
    record.lane = 2; // already facing target
    record.sub_x = 0;
    record.sub_y = 0;
    record.target_x = 11;
    record.target_y = 10;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.state, 1);
    assert_eq!(record.lane, 2);
    assert_eq!((record.tile_x, record.tile_y), (10, 10));
    assert_eq!((record.target_x, record.target_y), (11, 10));
}

#[test]
fn pe_plane_state7_spins_descends_then_morphs_at_height0() {
    // PE FUN_00453980 state 7 @ 0x453e68: height-- + lane+1 + move; at 0 → kind-6/state 5.
    let mut city = City::new_blank(std::path::Path::new("/tmp/plane-state7.sc2"), "PlaneState7")
        .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    record.state = 7;
    record.height = 2;
    record.lane = 2;
    record.sub_x = 0;
    record.sub_y = 0;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    let before = (record.tile_x, record.tile_y, record.sub_x, record.sub_y);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_AIRPLANE);
    assert_eq!(record.state, 7);
    assert_eq!(record.height, 1);
    assert_eq!(record.lane, 3);
    assert_ne!(
        (record.tile_x, record.tile_y, record.sub_x, record.sub_y),
        before
    );

    record.height = 0;
    record.lane = 3;
    let (tx, ty) = (record.tile_x, record.tile_y);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_EXPLOSION);
    assert_eq!(record.state, 5);
    assert_eq!(record.saved_b, 1);
    assert_eq!(record.lane, 0);
    assert_eq!((record.tile_x, record.tile_y), (tx, ty));
}

#[test]
fn pe_plane_state2_jitters_lane_then_moves() {
    // PE FUN_00453980 state 2 @ 0x453c02: FUN_00451440(lane,5) then one move.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/plane-freefly.sc2"),
        "PlaneFreeFly",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    record.state = 2;
    record.lane = 2;
    record.sub_x = 0;
    record.sub_y = 0;
    record.height = 0xe;
    let mut lfsr = Win95Lfsr::new(1);

    // rem != 0: lane stays east(2); exact 0x10 sub → no tile cross.
    let mut rng = NonZeroLowRng;
    let before = (record.tile_x, record.tile_y, record.sub_x, record.lane);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.state, 2);
    assert_eq!(record.lane, 2);
    assert_ne!(
        (record.tile_x, record.tile_y, record.sub_x),
        (before.0, before.1, before.2),
        "state 2 must call the move helper"
    );

    // rem == 0 and rand%3==0 → delta -1: east(2) → NE(1).
    record.lane = 2;
    record.sub_x = 0;
    record.sub_y = 0;
    let before = (record.tile_x, record.tile_y, record.sub_x, record.sub_y);
    let mut rng = ZeroModRng;
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.state, 2);
    assert_eq!(record.lane, 1);
    assert_ne!(
        (record.tile_x, record.tile_y, record.sub_x, record.sub_y),
        before,
        "jittered state 2 must still move"
    );
}

#[test]
fn pe_plane_state3_routes_then_enters_approach_state4() {
    // PE FUN_00453980 state 3: FUN_004514e0 abs snap, move; manhattan <2 →
    // lane=FUN_00451490(abs, state>>4), state=(land<<4)|4, target ±6 by odd land.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/plane-route.sc2"), "PlaneRoute").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_AIRPLANE, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    record.state = (3 << 4) | 3; // land lane 3 (x+6 arm), phase 3
    record.lane = 0;
    record.sub_x = 0;
    record.sub_y = 0;
    record.target_x = 11;
    record.target_y = 10;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    // Exact 0x10 sub after step: no tile cross; manhattan 1 < 2 → enter approach.
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.state, 0x34);
    assert_eq!(record.lane, 3); // step from abs east(2) toward land(3)
    assert_eq!((record.tile_x, record.tile_y), (10, 10));
    assert_eq!((record.target_x, record.target_y), (17, 10));
}

#[test]
fn pe_plane_skips_kind6_morph_when_zone_nibble_is_8() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/plane-zone8.sc2"), "PlaneZone8").expect("city");
    assert!(city.place_ambient_port_actor(20, 20, XTHG_AIRPLANE, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_AIRPLANE)
        .map(|(s, r)| (s, *r))
        .expect("plane");
    city.set_building(20, 20, 0xfb);
    city.set_zone(20, 20, 8);
    assert_eq!(city.zone_at(20, 20).map(|z| z.zone_id()), Some(8));
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_AIRPLANE);
}

#[test]
fn pe_plane_heli_tile_cross_refuses_xtxt_at_or_above_0xc9() {
    // PE FUN_004516F0: heavy dest + OOB next carry → restore +A, zero heavy, clear kind.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/heli-xtxt-refuse.sc2"),
        "HeliXtxtRefuse",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(124, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    assert_eq!(city.text_overlay_at(124, 10), Some(proxy));
    for x in 125..=127 {
        city.set_text_overlay(x, 10, XTHG_EXPLOSION_XTXT_GATE);
    }
    record.state = 2;
    record.height = 0xa;
    record.sub_x = 13;
    record.lane = 2;
    record.target_x = 127;
    record.target_y = 10;
    record.saved_a = 0x22;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, 0);
    assert_eq!((record.tile_x, record.tile_y), (127, 10));
    assert_eq!(city.text_overlay_at(124, 10), Some(0x22));
    assert_eq!(city.text_overlay_at(127, 10), Some(0));
}

#[test]
fn pe_plane_heli_skips_heavy_xtxt_to_clear_neighbor() {
    // PE @ 0x45185c: walk past XTXT >= 0xC9 onto the next clear tile.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/heli-xtxt-skip.sc2"),
        "HeliXtxtSkip",
    )
    .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_HELICOPTER, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_HELICOPTER)
        .map(|(s, r)| (s, *r))
        .expect("heli");
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.set_text_overlay(11, 10, XTHG_EXPLOSION_XTXT_GATE);
    record.state = 2;
    record.height = 0xa;
    record.sub_x = 13;
    record.lane = 2;
    record.saved_a = 0x22;
    let mut rng = MacToolboxRng::new(1);
    let mut lfsr = Win95Lfsr::new(1);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_HELICOPTER);
    assert_eq!((record.tile_x, record.tile_y), (12, 10));
    assert_eq!(city.text_overlay_at(10, 10), Some(0x22));
    assert_eq!(city.text_overlay_at(11, 10), Some(XTHG_EXPLOSION_XTXT_GATE));
    assert_eq!(city.text_overlay_at(12, 10), Some(proxy));
    assert_eq!(record.saved_a, 0);
}

#[test]
fn ambient_airplane_steps_east_and_despawns_off_map() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/plane-step.sc2"), "Plane").expect("city");
    let mut rng = MacToolboxRng::new(1);
    let mut record = Win95OverlayRecord {
        kind: XTHG_AIRPLANE,
        lane: 2,  // east
        state: 2, // free-fly (state 1 is land)
        tile_x: 10,
        tile_y: 10,
        // Airplane PE quantum is 0x10; start past center so one tick crosses a tile.
        sub_x: SUBTILE_CENTER + 1,
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
    assert_eq!(record.kind, XTHG_AIRPLANE);
    assert_eq!(record.tile_x, 11);
    assert_eq!(record.tile_y, 10);

    record.tile_x = (CITY_WIDTH - 1) as u8;
    record.sub_x = SUBTILE_CENTER + 1;
    city.set_text_overlay(CITY_WIDTH - 1, 10, xthg_proxy_for_slot(2).expect("proxy"));
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        2
    ));
    assert_eq!(record.kind, 0, "plane should despawn past the map edge");
    // PE FUN_004507d0 zeros XTXT occupancy when clearing the slot.
    assert_eq!(city.text_overlay_at(CITY_WIDTH - 1, 10), Some(0));
    // PE FUN_004507D0 clears kind only; height remains.
    assert_eq!(record.height, city.altitude_at(10, 10));
}
