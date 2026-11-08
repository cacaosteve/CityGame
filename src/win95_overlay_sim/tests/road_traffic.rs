use super::*;

#[test]
fn spawns_when_xtrf_above_aa() {
    let mut city = City::new_blank(std::path::Path::new("/tmp/spawn.sc2"), "Spawn").expect("city");
    city.set_building(20, 20, 29);
    city.set_traffic_density(20, 20, 0xbb);
    assert!(spawn_traffic_actor(&mut city, 20, 20, 29, 0xbb));
    let record = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .find(|r| r.is_traffic_slot())
        .copied()
        .expect("spawned traffic slot");
    assert_eq!(record.overlay_state(), OverlayState::SpawnPending);
    assert_eq!(record.tile_x, 20);
    assert_eq!(
        city.traffic_density_at(20, 20),
        Some(0xbb),
        "port-only actor admission leaves the XTRF input unchanged"
    );
}

#[test]
fn road_decoration_lifecycle_never_writes_xtrf() {
    // Port actor spawn/handoff/clear must leave the XTRF congestion plane alone.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/road-xtrf-life.sc2"),
        "RoadXtrfLife",
    )
    .expect("city");
    city.set_building(10, 10, 29);
    city.set_building(12, 10, 29);
    city.set_traffic_density(10, 10, 0xbb);
    city.set_traffic_density(12, 10, 0xcc);
    assert!(spawn_traffic_actor(&mut city, 10, 10, 29, 0xbb));
    assert_eq!(city.traffic_density_at(10, 10), Some(0xbb));
    let mut record = city.traffic_overlays().unwrap()[XTHG_FIRST_USABLE_SLOT];
    record.state = OverlayState::LightActive as u8;
    record.sub_x = 15;
    record.sub_y = 8;
    record.lane = 1;
    cross_tile_byte_handoff(&mut city, &mut record, 10, 10, 12, 10);
    assert_eq!(city.traffic_density_at(10, 10), Some(0xbb));
    assert_eq!(city.traffic_density_at(12, 10), Some(0xcc));
    assert_eq!(record.saved_a, 0xcc);
    clear_traffic_actor(&mut record, &mut city);
    assert_eq!(record.kind, 0);
    assert_eq!(city.traffic_density_at(10, 10), Some(0xbb));
    assert_eq!(city.traffic_density_at(12, 10), Some(0xcc));
}

#[test]
fn road_decoration_spawn_probes_consume_process_crt() {
    // Port road-actor probes use the live process CRT supplied by the app tick.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/spawn-crt.sc2"), "SpawnCrt").expect("city");
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            city.set_building(x, y, 29);
            city.set_traffic_density(x, y, 0xff);
        }
    }
    let mut crt = Win95CrtRand::new(0x5151_5151);
    let before = crt.seed();
    assert!(try_spawn_overlays(&mut city, 0, &mut crt));
    assert_ne!(crt.seed(), before, "spawn probes must consume process CRT");
}

#[test]
fn road_decoration_update_consumes_process_crt_not_sim_rng() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/update-crt.sc2"), "UpdateCrt").expect("city");
    city.set_building(20, 20, 29);
    city.set_building(21, 20, 29);
    city.set_traffic_density(20, 20, 0xc0);
    city.set_traffic_density(21, 20, 0xc0);
    assert!(spawn_traffic_actor(&mut city, 20, 20, 29, 0xc0));
    // Promote pending → light so update_light_actor rolls CRT.
    if let Some(table) = city.traffic_overlays_mut() {
        table[XTHG_FIRST_USABLE_SLOT].state = OverlayState::LightActive as u8;
        table[XTHG_FIRST_USABLE_SLOT].lane = 2;
        table[XTHG_FIRST_USABLE_SLOT].sub_x = 8;
        table[XTHG_FIRST_USABLE_SLOT].sub_y = 8;
    }
    let mut sim = Win95OverlaySim::new();
    sim.reset_for_city(&city);
    let overlay_before = sim.sim_rng_seed();
    let mut crt = Win95CrtRand::new(0x4242_4242);
    let crt_before = crt.seed();
    let mut lfsr = Win95Lfsr::new(1);
    let _ = sim.tick_once(&mut city, &mut crt, &mut lfsr);
    assert_ne!(
        crt.seed(),
        crt_before,
        "decoration update must consume process CRT"
    );
    assert_eq!(
        sim.sim_rng_seed(),
        overlay_before,
        "city-name sim_rng must stay idle on decoration tick"
    );
}

#[test]
fn cross_tile_snapshots_arrival_xtrf_without_restoring_departure() {
    let mut city = City::new_blank(std::path::Path::new("/tmp/xfer.sc2"), "Xfer").expect("city");
    city.set_building(10, 10, 29);
    city.set_building(12, 10, 29);
    // Distinct XTRF microsim cells — (10,10) and (11,10) share one 2×2.
    city.set_traffic_density(10, 10, 0x44);
    city.set_traffic_density(12, 10, 0x88);

    let mut record = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::LightActive as u8,
        tile_x: 10,
        tile_y: 10,
        sub_x: 15,
        sub_y: 8,
        lane: 1,
        saved_a: 0x55,
        ..Default::default()
    };
    city.set_traffic_density(10, 10, 0x99);
    cross_tile_byte_handoff(&mut city, &mut record, 10, 10, 12, 10);
    assert_eq!(
        city.traffic_density_at(10, 10),
        Some(0x99),
        "departure must not invent an XTRF restore"
    );
    assert_eq!(record.saved_a, 0x88, "saved_a keeps raw arriving XTRF");
    assert_eq!(
        city.traffic_density_at(12, 10),
        Some(0x88),
        "PE handoff decrements XTXT slot-0x37, not XTRF congestion"
    );
}

#[test]
fn pe_reset_preserves_link_payload_and_xtrf() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/pe-reset.sc2"), "PE reset").expect("city");
    city.set_traffic_density(11, 10, 0x88);
    let mut record = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::HeavyActive as u8,
        tile_x: 11,
        tile_y: 10,
        height: 17,
        saved_a: 0x44,
        saved_b: 3,
        ..Default::default()
    };

    reset_traffic_actor_pe(&mut record, &mut city);

    assert_eq!(
        city.traffic_density_at(11, 10),
        Some(0x88),
        "FUN_004507D0 zeros XTXT, not XTRF"
    );
    assert_eq!(record.kind, 0);
    assert_eq!(record.height, 17);
    assert_eq!(record.saved_b, 3);
    assert_eq!((record.tile_x, record.tile_y), (11, 10));
}

#[test]
fn soft_clear_preserves_xtrf_and_keeps_kind_only_clear() {
    // Soft cleanup: FUN_004507D0 clears kind (XTXT for occupancy actors).
    // Road-decoration must not invent an XTRF restore to `saved_a`.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/soft-clear.sc2"), "SoftClear").expect("city");
    city.set_traffic_density(12, 12, 0x99);
    let mut record = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::LightActive as u8,
        tile_x: 12,
        tile_y: 12,
        height: 9,
        saved_a: 0x44,
        saved_b: 5,
        ..Default::default()
    };

    clear_traffic_actor(&mut record, &mut city);

    assert_eq!(
        city.traffic_density_at(12, 12),
        Some(0x99),
        "soft clear must leave live XTRF alone"
    );
    assert_eq!(record.kind, 0);
    assert_eq!(record.height, 9);
    assert_eq!(record.saved_b, 5);
    assert_eq!((record.tile_x, record.tile_y), (12, 12));
}

#[test]
fn road_decoration_heavy_dest_preserves_both_xtrf_cells() {
    // Heavy dest: clear kind; leave both tiles' XTRF alone.
    // Tiles (9,10) and (10,10) sit in different 2×2 XTRF microsim cells.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/road-heavy-dest.sc2"),
        "RoadHeavyDest",
    )
    .expect("city");
    city.set_building(9, 10, 29);
    city.set_building(10, 10, 29);
    city.set_traffic_density(9, 10, 0x99);
    city.set_traffic_density(10, 10, MAC_TRAFFIC_HEAVY_BYTE_MIN);
    let mut record = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::HeavyActive as u8,
        tile_x: 9,
        tile_y: 10,
        sub_x: 15,
        sub_y: 8,
        lane: 2,
        saved_a: 0x55,
        ..Default::default()
    };
    step_actor(&mut record, 29, &mut city, 0, &mut MacToolboxRng::new(1));
    assert_eq!(record.kind, 0);
    assert_eq!(city.traffic_density_at(9, 10), Some(0x99));
    assert_eq!(
        city.traffic_density_at(10, 10),
        Some(MAC_TRAFFIC_HEAVY_BYTE_MIN),
        "heavy dest must not invent XTRF zero"
    );
}

#[test]
fn recycled_parent_updates_linked_child_height() {
    let city = City::new_blank(
        std::path::Path::new("/tmp/recycle-link.sc2"),
        "Recycle link",
    )
    .expect("city");
    let mut table = empty_overlay_table();
    table[1] = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        height: 9,
        ..Default::default()
    };
    let mut child = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        saved_b: 1,
        ..Default::default()
    };

    refresh_overlay_height(&mut child, &city, &table);
    assert_eq!(child.height, 9);

    table[1] = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        height: 23,
        ..Default::default()
    };
    refresh_overlay_height(&mut child, &city, &table);
    assert_eq!(
        child.height, 23,
        "linked child must observe recycled parent"
    );
}

#[test]
fn traffic_spawn_slot_skips_reserved_zero_and_reuses_first_free_slot() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/spawn-slot.sc2"), "Spawn slot").expect("city");
    city.set_traffic_overlays_for_test(empty_overlay_table());

    assert_eq!(traffic_spawn_free_slot(&city), Some(XTHG_FIRST_USABLE_SLOT));
    if let Some(overlays) = city.traffic_overlays_mut() {
        overlays[XTHG_FIRST_USABLE_SLOT].kind = OVERLAY_KIND_TRAFFIC;
    }
    assert_eq!(
        traffic_spawn_free_slot(&city),
        Some(XTHG_FIRST_USABLE_SLOT + 1)
    );
}

#[test]
fn height_byte_uses_altm_and_state() {
    let mut city = City::new_blank(std::path::Path::new("/tmp/h.sc2"), "Height").expect("city");
    city.set_building(5, 5, 29);
    city.set_traffic_density(5, 5, 0x90);
    let mut record = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::HeavyActive as u8,
        tile_x: 5,
        tile_y: 5,
        saved_b: 0xff,
        ..Default::default()
    };
    refresh_overlay_height(&mut record, &city, &[]);
    let expected_base = ((altm_packed_word(&city, 5, 5) >> 5) & 0x1f) as u8;
    assert_eq!(record.height, expected_base.wrapping_add(2));
}

#[test]
fn height_copies_from_linked_slot() {
    let mut table = empty_overlay_table();
    table[1].height = 17;
    let mut record = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        saved_b: 1,
        ..Default::default()
    };
    refresh_overlay_height(
        &mut record,
        &City::new_blank(std::path::Path::new("/tmp/link.sc2"), "Link").expect("city"),
        &table,
    );
    assert_eq!(record.height, 17);
}

#[test]
fn step_actor_moves_along_road() {
    let mut city = City::new_blank(std::path::Path::new("/tmp/step.sc2"), "Step").expect("city");
    city.set_building(10, 10, 29);
    city.set_building(11, 10, 29);
    city.set_traffic_density(10, 10, 0xd0);
    city.set_traffic_density(11, 10, 0xd0);
    let mut record = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::HeavyActive as u8,
        tile_x: 10,
        tile_y: 10,
        sub_x: SUBTILE_CENTER,
        sub_y: SUBTILE_CENTER,
        lane: 2,
        saved_a: 0xd0,
        ..Default::default()
    };
    step_actor(&mut record, 29, &mut city, 0, &mut MacToolboxRng::new(1));
    assert!(
        record.sub_x != SUBTILE_CENTER || record.tile_x != 10,
        "step_actor should advance sub-tile or tile (got sub={}, tile=({},{}))",
        record.sub_x,
        record.tile_x,
        record.tile_y
    );
}

#[test]
fn update_slot_persists_heavy_actor_step() {
    let mut city = City::new_blank(std::path::Path::new("/tmp/step.sc2"), "Step").expect("city");
    let mut table = empty_overlay_table();
    table[0] = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::HeavyActive as u8,
        tile_x: 10,
        tile_y: 10,
        sub_x: SUBTILE_CENTER,
        sub_y: SUBTILE_CENTER,
        lane: 2,
        saved_a: 0xd0,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    city.set_building(10, 10, 29);
    city.set_building(11, 10, 29);
    city.set_building(12, 10, 29);
    city.set_traffic_density(10, 10, 0xd0);
    city.set_traffic_density(11, 10, 0xd0);

    let snapshot = city.traffic_overlays().unwrap().to_vec();
    let before = snapshot[0];
    let mut rng = MacToolboxRng::new(1);
    assert!(update_slot(&mut city, 0, &snapshot, 0, &mut rng));
    let after = city.traffic_overlays().unwrap()[0];
    assert!(
        after.sub_x != before.sub_x
            || after.sub_y != before.sub_y
            || after.tile_x != before.tile_x
            || after.tile_y != before.tile_y,
        "expected movement, still at sub=({},{}), tile=({},{}",
        after.sub_x,
        after.sub_y,
        after.tile_x,
        after.tile_y
    );
}

#[test]
fn light_actor_step_persists_subtile_position() {
    let mut city = City::new_blank(std::path::Path::new("/tmp/step.sc2"), "Step").expect("city");
    let mut table = empty_overlay_table();
    let slot = XTHG_FIRST_USABLE_SLOT;
    table[slot] = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::HeavyActive as u8,
        tile_x: 10,
        tile_y: 10,
        sub_x: SUBTILE_CENTER,
        sub_y: SUBTILE_CENTER,
        lane: 2,
        saved_a: 0xd0,
        ..Default::default()
    };
    city.set_traffic_overlays_for_test(table);
    city.set_building(10, 10, 29);
    city.set_building(11, 10, 29);
    city.set_building(12, 10, 29);
    city.set_traffic_density(10, 10, 0xd0);
    city.set_traffic_density(11, 10, 0xd0);
    city.set_traffic_density(12, 10, 0xd0);

    let before = city.traffic_overlays().unwrap()[slot];
    let mut sim = Win95OverlaySim::new();
    let mut moved = false;
    for _ in 0..8 {
        let mut crt = Win95CrtRand::new(1);
        let mut lfsr = Win95Lfsr::new(1);
        if sim.tick_once(&mut city, &mut crt, &mut lfsr) {
            let after = city.traffic_overlays().unwrap()[slot];
            if after.sub_x != before.sub_x
                || after.sub_y != before.sub_y
                || after.tile_x != before.tile_x
                || after.tile_y != before.tile_y
            {
                moved = true;
                break;
            }
        }
    }
    assert!(
        moved,
        "light actor sub-tile position should persist after sim tick"
    );
}

#[test]
fn inactive_follows_saved_b_slot_link() {
    let mut table = empty_overlay_table();
    table[1] = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        tile_x: 30,
        tile_y: 30,
        ..Default::default()
    };
    table[0] = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::Inactive as u8,
        saved_b: 1,
        ..Default::default()
    };
    let (x, y) = resolve_scan_origin(&table[0], &table);
    assert_eq!((x, y), (30, 30));
}

#[test]
fn inactive_scan_reactivates_at_target_road() {
    let mut table = empty_overlay_table();
    table[0] = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        state: OverlayState::Inactive as u8,
        target_x: 12,
        target_y: 12,
        saved_b: 0xff,
        ..Default::default()
    };
    let mut city = City::new_blank(std::path::Path::new("/tmp/react.sc2"), "React").expect("city");
    city.set_traffic_overlays_for_test(table);
    city.set_building(12, 12, 29);
    city.set_traffic_density(12, 12, 0xb0);
    let mut record = city.traffic_overlays().unwrap()[0];
    record.kind = OVERLAY_KIND_TRAFFIC;
    assert!(update_inactive_slot(
        &mut record,
        &city,
        city.traffic_overlays().unwrap(),
        0,
    ));
    assert_eq!(record.overlay_state(), OverlayState::SpawnPending);
    assert_eq!(record.tile_x, 12);
}

#[test]
fn inactive_scan_uses_pe_x_outer_y_inner_order() {
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/scan-order.sc2"), "Scan order").expect("city");
    city.set_building(8, 10, BUILDING_OVERLAY_BLOCK);
    city.set_building(10, 8, BUILDING_OVERLAY_BLOCK);
    let mut record = Win95OverlayRecord {
        kind: OVERLAY_KIND_TRAFFIC,
        tile_x: 10,
        tile_y: 10,
        ..Default::default()
    };

    assert!(scan_inactive_window(&mut record, &city, 0));
    assert_eq!((record.target_x, record.target_y), (8, 10));
}

#[test]
fn port_traffic_actor_spawn_heuristic_uses_xtrf_aa_cutoff() {
    assert!(!port_traffic_actor_spawn_gate(0xa9));
    assert!(port_traffic_actor_spawn_gate(0xaa));
    assert!(port_traffic_actor_spawn_gate(0xc9));
    assert!(port_traffic_actor_spawn_gate(0xff));
}

#[test]
fn port_traffic_actor_spawn_preserves_xtrf() {
    // The port-only road actor does not consume or mutate XTRF.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/spawn-xtrf.sc2"), "SpawnXtrf").expect("city");
    city.set_building(20, 20, 29);
    city.set_traffic_density(20, 20, 0xaa);
    assert!(spawn_traffic_actor(&mut city, 20, 20, 29, 0xaa));
    assert_eq!(city.traffic_density_at(20, 20), Some(0xaa));
}
