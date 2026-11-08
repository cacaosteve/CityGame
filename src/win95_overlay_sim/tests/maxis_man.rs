use super::*;

#[test]
fn maxis_man_light_zero_roll_returns_to_inactive() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/maxis-man-light.sc2"),
        "MaxisManLight",
    )
    .expect("city");
    let mut record = Win95OverlayRecord {
        kind: XTHG_MAXIS_MAN,
        state: PeMaxisManState::LightActive as u8,
        tile_x: 10,
        tile_y: 10,
        sub_x: SUBTILE_CENTER,
        sub_y: SUBTILE_CENTER,
        ..Default::default()
    };
    let mut rng = ZeroModRng;

    assert!(update_maxis_man_light(
        &mut record,
        &mut city,
        &mut rng,
        XTHG_FIRST_USABLE_SLOT
    ));
    assert_eq!(record.state, PeMaxisManState::Inactive as u8);
    assert_eq!((record.tile_x, record.tile_y), (10, 10));
}

#[test]
fn maxis_man_heavy_dest_oob_zeros_xtxt() {
    // PE FUN_004516F0: heavy XTXT + OOB next → restore +A, zero heavy, clear kind.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/maxis-man-heavy-oob.sc2"),
        "MaxisManHeavyOob",
    )
    .expect("city");
    let slot = XTHG_FIRST_USABLE_SLOT;
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.set_text_overlay(126, 10, proxy);
    city.set_text_overlay(127, 10, XTHG_EXPLOSION_XTXT_GATE);
    let mut record = Win95OverlayRecord {
        kind: XTHG_MAXIS_MAN,
        state: PeMaxisManState::LightActive as u8,
        tile_x: 126,
        tile_y: 10,
        sub_x: 15,
        sub_y: SUBTILE_CENTER,
        saved_a: 0x55,
        ..Default::default()
    };

    assert!(move_maxis_man(
        &mut record,
        &mut city,
        2,
        MAXIS_MAN_SUBTILE_STEP,
        slot,
    ));
    assert_eq!(record.kind, 0);
    assert_eq!((record.tile_x, record.tile_y), (127, 10));
    assert_eq!(city.text_overlay_at(126, 10), Some(0x55));
    assert_eq!(city.text_overlay_at(127, 10), Some(0));
}

#[test]
fn maxis_man_skips_heavy_xtxt_to_clear_neighbor() {
    // PE @ 0x45185c via FUN_00455020: walk past XTXT >= 0xC9 onto clear tile.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/maxis-man-xtxt-skip.sc2"),
        "MaxisManXtxtSkip",
    )
    .expect("city");
    let slot = XTHG_FIRST_USABLE_SLOT;
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.set_text_overlay(9, 10, proxy);
    city.set_text_overlay(10, 10, XTHG_EXPLOSION_XTXT_GATE);
    let mut record = Win95OverlayRecord {
        kind: XTHG_MAXIS_MAN,
        state: PeMaxisManState::LightActive as u8,
        tile_x: 9,
        tile_y: 10,
        sub_x: 15,
        sub_y: SUBTILE_CENTER,
        saved_a: 0x55,
        ..Default::default()
    };

    assert!(move_maxis_man(
        &mut record,
        &mut city,
        2,
        MAXIS_MAN_SUBTILE_STEP,
        slot,
    ));
    assert_eq!(record.kind, XTHG_MAXIS_MAN);
    assert_eq!((record.tile_x, record.tile_y), (11, 10));
    assert_eq!(city.text_overlay_at(9, 10), Some(0x55));
    assert_eq!(city.text_overlay_at(10, 10), Some(XTHG_EXPLOSION_XTXT_GATE));
    assert_eq!(city.text_overlay_at(11, 10), Some(proxy));
    assert_eq!(record.saved_a, 0);
}

#[test]
fn maxis_man_search_uses_xtxt_not_xbld() {
    // PE FUN_00455020 unlinked/scan target gate is XTXT >= 0xf1, not XBLD.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/maxis-search-xtxt.sc2"),
        "MaxisSearchXtxt",
    )
    .expect("city");
    let table = empty_overlay_table();
    let mut record = Win95OverlayRecord {
        kind: XTHG_MAXIS_MAN,
        state: PeMaxisManState::Inactive as u8,
        tile_x: 20,
        tile_y: 20,
        target_x: 25,
        target_y: 20,
        saved_b: SAVED_B_SLOT_LINK,
        ..Default::default()
    };
    city.set_building(25, 20, 0xf1);
    city.set_text_overlay(25, 20, 0);
    let mut rng = MacToolboxRng::new(1);
    assert!(update_maxis_man_search(
        &mut record,
        &mut city,
        &table,
        &mut rng,
        XTHG_FIRST_USABLE_SLOT
    ));
    assert_eq!(
        record.state,
        PeMaxisManState::HeavyActive as u8,
        "XBLD alone must not count as a search target"
    );

    city.set_building(25, 20, 0);
    city.set_text_overlay(25, 20, 0xf1);
    record.state = PeMaxisManState::Inactive as u8;
    record.target_x = 25;
    record.target_y = 20;
    assert!(update_maxis_man_search(
        &mut record,
        &mut city,
        &table,
        &mut rng,
        XTHG_FIRST_USABLE_SLOT
    ));
    assert_ne!(
        record.state,
        PeMaxisManState::HeavyActive as u8,
        "XTXT >= 0xf1 should follow the unlinked target"
    );
}

#[test]
fn maxis_man_search_spawns_kind6_when_ahead_holds_linked_proxy() {
    // PE `0x45526b`: linked-proxy XTXT ahead + `rand()&3==0` → clear link, FUN_00453280, state 2.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/maxis-man-kind6.sc2"),
        "MaxisManKind6",
    )
    .expect("city");
    let mut table = empty_overlay_table();
    table[2] = Win95OverlayRecord {
        kind: XTHG_AIRPLANE,
        tile_x: 11,
        tile_y: 10,
        ..Default::default()
    };
    table[1] = Win95OverlayRecord {
        kind: XTHG_MAXIS_MAN,
        state: PeMaxisManState::Inactive as u8,
        tile_x: 10,
        tile_y: 10,
        height: 0x0c,
        sub_x: SUBTILE_CENTER,
        sub_y: SUBTILE_CENTER,
        saved_b: 2,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table.clone());
    city.set_text_overlay(11, 10, xthg_proxy_for_slot(2).expect("slot 2 proxy"));

    let mut record = table[1];
    assert!(update_maxis_man_search(
        &mut record,
        &mut city,
        &table,
        &mut ZeroModRng,
        1,
    ));
    assert_eq!(record.state, PeMaxisManState::HeavyActive as u8);
    let (slot, explosion) = city
        .pe_explosion_actor_at(11, 10)
        .expect("kind-6 explosion");
    // PE clears the linked slot then allocates; the free-slot scan may reuse it.
    assert_eq!(explosion.kind, crate::win95_overlay::XTHG_EXPLOSION);
    assert_eq!(explosion.height, 0x0c);
    assert_eq!(explosion.state, 0);
    assert_eq!(explosion.saved_b, 1);
    assert_eq!(
        explosion.sub_x,
        crate::win95_overlay::XTHG_EXPLOSION_SUBTILE
    );
    assert_eq!(city.text_overlay_at(11, 10), xthg_proxy_for_slot(slot));
    assert!(
        city.traffic_overlays()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(i, r)| *i != slot && r.kind == XTHG_AIRPLANE)
            .count()
            == 0,
        "linked airplane slot must be cleared"
    );
}

#[test]
fn maxis_man_search_enters_light_when_linked_proxy_roll_misses() {
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/maxis-man-kind6-miss.sc2"),
        "MaxisManKind6Miss",
    )
    .expect("city");
    let mut table = empty_overlay_table();
    table[2] = Win95OverlayRecord {
        kind: XTHG_AIRPLANE,
        tile_x: 11,
        tile_y: 10,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table.clone());
    city.set_text_overlay(11, 10, xthg_proxy_for_slot(2).expect("slot 2 proxy"));

    let mut record = Win95OverlayRecord {
        kind: XTHG_MAXIS_MAN,
        state: PeMaxisManState::Inactive as u8,
        tile_x: 10,
        tile_y: 10,
        height: 0x0c,
        sub_x: SUBTILE_CENTER,
        sub_y: SUBTILE_CENTER,
        saved_b: 2,
        ..Default::default()
    };
    assert!(update_maxis_man_search(
        &mut record,
        &mut city,
        &table,
        &mut NonZeroLowRng,
        1,
    ));
    assert_eq!(record.state, PeMaxisManState::LightActive as u8);
    assert_eq!(city.traffic_overlays().unwrap()[2].kind, XTHG_AIRPLANE);
    assert!(city.pe_explosion_actor_at(11, 10).is_none());
    assert_eq!(city.text_overlay_at(11, 10), xthg_proxy_for_slot(2));
}
