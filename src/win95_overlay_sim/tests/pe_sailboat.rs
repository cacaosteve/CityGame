use super::*;

#[test]
fn pe_sailboat_nonzero_state_idles_or_despawns_via_lfsr() {
    // PE FUN_004562e0 @ 0x456320: state!=0 → 401cad(5)==0 despawn else no move.
    let mut city = City::new_blank(std::path::Path::new("/tmp/sailboat-state.sc2"), "SailState")
        .expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_SAILBOAT, 2));
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_SAILBOAT)
        .map(|(s, r)| (s, *r))
        .expect("sailboat");
    record.state = 1;
    let (tx, ty) = (record.tile_x, record.tile_y);
    let mut rng = MacToolboxRng::new(1);

    let mut seed = 1u16;
    let idle_seed = loop {
        let mut probe = Win95Lfsr::new(seed);
        if probe.random_mod(5) != 0 {
            break seed;
        }
        seed = seed.wrapping_add(1);
    };
    let mut lfsr = Win95Lfsr::new(idle_seed);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_SAILBOAT);
    assert_eq!(record.state, 1);
    assert_eq!((record.tile_x, record.tile_y), (tx, ty));

    let despawn_seed = loop {
        let mut probe = Win95Lfsr::new(seed);
        if probe.random_mod(5) == 0 {
            break seed;
        }
        seed = seed.wrapping_add(1);
    };
    let mut lfsr = Win95Lfsr::new(despawn_seed);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, 0);
    assert_eq!(city.text_overlay_at(tx as usize, ty as usize), Some(0));
}

#[test]
fn pe_sailboat_tick_budget_despawns_fifth_handler_call() {
    // PE 0x4e99d0: each overlay tick resets; 5th FUN_004562e0 call despawns.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/sail-budget.sc2"), "SailBudget").expect("city");
    let mut slots = Vec::new();
    for x in 10..15 {
        city.set_terrain(x, 20, 0x10);
        city.set_tile_bits(x, 20, crate::city::TileBits::new(0x04));
        let slot = {
            let overlays = city.traffic_overlays_mut().expect("xthg");
            overlays
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .find(|(_, r)| r.kind == 0)
                .map(|(s, _)| s)
                .expect("free slot")
        };
        if let Some(overlays) = city.traffic_overlays_mut() {
            overlays[slot] = Win95OverlayRecord {
                kind: XTHG_SAILBOAT,
                lane: 1,
                state: 1, // nonzero: idle path (no tile motion) so budget is isolated
                tile_x: x as u8,
                tile_y: 20,
                height: 0,
                sub_x: 4,
                sub_y: 4,
                ..Default::default()
            };
        }
        if let Some(proxy) = xthg_proxy_for_slot(slot) {
            city.set_text_overlay(x, 20, proxy);
        }
        slots.push((slot, x));
    }
    slots.sort_by_key(|(slot, _)| *slot);
    assert_eq!(city.ambient_port_actor_count(XTHG_SAILBOAT), 5);
    let (fifth_slot, fifth_x) = slots[4];

    let mut sim = Win95OverlaySim::new();
    let mut crt = Win95CrtRand::new(1);
    // State!=0 arms draw 401cad(5); keep rem!=0 for the four survivors.
    let mut seed = 1u16;
    let idle_seed = 'seed: loop {
        let mut probe = Win95Lfsr::new(seed);
        for _ in 0..4 {
            if probe.random_mod(5) == 0 {
                seed = seed.wrapping_add(1);
                continue 'seed;
            }
        }
        break seed;
    };
    let mut lfsr = Win95Lfsr::new(idle_seed);
    assert!(sim.tick_ambient(
        &mut city,
        GameSpeed::Llama,
        1.0 / WIN95_OVERLAY_SIM_TICKS_PER_SECOND,
        &mut crt,
        &mut lfsr
    ));
    let fifth = city.traffic_overlays().unwrap()[fifth_slot];
    assert_eq!(fifth.kind, 0, "5th sailboat handler call must FUN_00401528");
    assert_eq!(city.text_overlay_at(fifth_x, 20), Some(0));
    assert_eq!(city.ambient_port_actor_count(XTHG_SAILBOAT), 4);
}

#[test]
fn pe_spawn_sailboat_gates_on_budget_not_live_count() {
    // PE SpawnSailBoat @ 0x453460 gates on word[0x4e99d0] >= 4, not live XTHG count.
    // Four sailboats written directly leave budget 0, so spawn still succeeds.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/sail-budget-spawn.sc2"),
        "SailBudgetSpawn",
    )
    .expect("city");
    for x in 10..14 {
        city.set_terrain(x, 20, 0x10);
        let slot = {
            let overlays = city.traffic_overlays_mut().expect("xthg");
            overlays
                .iter()
                .enumerate()
                .skip(XTHG_FIRST_USABLE_SLOT)
                .find(|(_, r)| r.kind == 0)
                .map(|(s, _)| s)
                .expect("free slot")
        };
        if let Some(overlays) = city.traffic_overlays_mut() {
            overlays[slot] = Win95OverlayRecord {
                kind: XTHG_SAILBOAT,
                lane: 1,
                state: 0,
                tile_x: x as u8,
                tile_y: 20,
                height: 0,
                sub_x: 4,
                sub_y: 4,
                ..Default::default()
            };
        }
    }
    assert_eq!(city.ambient_port_actor_count(XTHG_SAILBOAT), 4);
    assert_eq!(city.pe_sailboat_budget, 0);
    city.set_terrain(30, 30, 0x10);
    assert!(
        city.place_ambient_port_actor(30, 30, XTHG_SAILBOAT, 2),
        "budget 0 must allow spawn despite 4 live sailboats"
    );
    assert_eq!(city.pe_sailboat_budget, 1);
    assert_eq!(city.ambient_port_actor_count(XTHG_SAILBOAT), 5);

    // After four PE spawn incs, budget >= 4 refuses even with empty live table.
    let mut city2 = City::new_blank(
        std::path::Path::new("/tmp/sail-budget-spawn2.sc2"),
        "SailBudgetSpawn2",
    )
    .expect("city");
    city2.pe_sailboat_budget = 4;
    city2.set_terrain(10, 10, 0x10);
    assert!(
        !city2.place_ambient_port_actor(10, 10, XTHG_SAILBOAT, 2),
        "budget >= 4 must refuse spawn with zero live sailboats"
    );
    assert_eq!(city2.ambient_port_actor_count(XTHG_SAILBOAT), 0);
}

#[test]
fn pe_sailboat_state0_drift_jitters_cardinal_lane_without_move() {
    // PE FUN_004562e0 state0 when 401cad(4)==0: water check, CRT (lane+rand%3-1)&3, no step.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/sail-drift.sc2"), "SailDrift").expect("city");
    assert!(city.place_ambient_port_actor(10, 10, XTHG_SAILBOAT, 1));
    city.set_tile_bits(10, 10, crate::city::TileBits::new(0x04)); // XBIT water
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_SAILBOAT)
        .map(|(s, r)| (s, *r))
        .expect("sail");
    record.lane = 1;
    let (tx, ty, sx, sy) = (record.tile_x, record.tile_y, record.sub_x, record.sub_y);
    let mut seed = 1u16;
    let drift_seed = loop {
        let mut probe = Win95Lfsr::new(seed);
        if probe.random_mod(4) == 0 {
            // also need 0xfa0 != 0 so state stays 0
            if probe.random_mod(0xfa0) != 0 {
                break seed;
            }
        }
        seed = seed.wrapping_add(1);
    };
    let mut rng = ZeroModRng; // rand%3==0 → delta -1: lane 1 → 0
    let mut lfsr = Win95Lfsr::new(drift_seed);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, XTHG_SAILBOAT);
    assert_eq!(record.state, 0);
    assert_eq!(record.lane, 0);
    assert_eq!(
        (record.tile_x, record.tile_y, record.sub_x, record.sub_y),
        (tx, ty, sx, sy)
    );
}

#[test]
fn pe_sailboat_oob_despawn_zeros_xtxt_occupancy() {
    // PE kind-9 lanes are 0..=3 (N,E,S,W); west=3. State0 move path needs LFSR%4!=0.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/sail-oob.sc2"), "SailOob").expect("city");
    city.set_terrain(0, 10, 0x10);
    assert!(city.place_ambient_port_actor(0, 10, XTHG_SAILBOAT, 3)); // PE west
    let (slot, mut record) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, r)| r.kind == XTHG_SAILBOAT)
        .map(|(s, r)| (s, *r))
        .expect("sail");
    assert_eq!(city.text_overlay_at(0, 10), xthg_proxy_for_slot(slot));
    record.sub_x = 0;
    record.sub_y = 4;
    let mut rng = MacToolboxRng::new(1);
    let mut seed = 1u16;
    let move_seed = loop {
        let mut probe = Win95Lfsr::new(seed);
        if probe.random_mod(4) != 0 {
            break seed;
        }
        seed = seed.wrapping_add(1);
    };
    let mut lfsr = Win95Lfsr::new(move_seed);
    assert!(step_ambient_actor(
        &mut record,
        &mut city,
        &mut rng,
        &mut lfsr,
        slot
    ));
    assert_eq!(record.kind, 0);
    assert_eq!(city.text_overlay_at(0, 10), Some(0));
}

#[test]
fn pe_sailboat_subtile_step_refuses_tile_at_or_above_0x7f() {
    // PE 0x456470: dest tile >= 0x7f → FUN_00401528 (not land on 0x7f).
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/sail-bound-7f.sc2"),
        "SailBound7f",
    )
    .expect("city");
    let slot = 2usize;
    let proxy = xthg_proxy_for_slot(slot).expect("proxy");
    city.set_text_overlay(0x7e, 10, proxy);
    city.set_tile_bits(0x7f, 10, crate::city::TileBits::new(0x04));
    let mut record = Win95OverlayRecord {
        kind: XTHG_SAILBOAT,
        lane: 1, // east
        tile_x: 0x7e,
        tile_y: 10,
        sub_x: 8,
        sub_y: 8,
        ..Default::default()
    };
    // One +16 on sub_x crosses tile (+1).
    pe_sailboat_subtile_step(&mut record, &mut city, slot);
    assert_eq!(record.kind, 0);
    assert_eq!(city.text_overlay_at(0x7e, 10), Some(0));
    assert_ne!(city.text_overlay_at(0x7f, 10), Some(proxy));
}

#[test]
fn pe_sailboat_spawn_requires_zero_xtxt() {
    // PE SpawnSailBoat @ `0x453491`/`0x4534a1`: XBLD and XTXT must both be 0.
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/sail-xtxt.sc2"), "SailXtxt").expect("city");
    city.set_terrain(10, 10, 0x10);
    city.set_text_overlay(10, 10, 1);
    assert!(!city.place_ambient_port_actor(10, 10, XTHG_SAILBOAT, 2));
    city.set_text_overlay(10, 10, 0);
    city.set_building(10, 10, 1);
    assert!(!city.place_ambient_port_actor(10, 10, XTHG_SAILBOAT, 2));
    city.set_building(10, 10, 0);
    assert!(city.place_ambient_port_actor(10, 10, XTHG_SAILBOAT, 2));
    // Painted slot-0x37 occupancy blocks a second boat on the same tile.
    assert!(!city.place_ambient_port_actor(10, 10, XTHG_SAILBOAT, 2));
}

#[test]
fn pe_sailboat_spawn_tile_from_marina_prefers_north_water() {
    // PE scanner `0x453380` face order N,E,S,W (`0x4e8278`/`0x4e8280`).
    let mut city =
        City::new_blank(std::path::Path::new("/tmp/marina-scan.sc2"), "MarinaScan").expect("city");
    city.set_building(10, 10, 248);
    city.set_tile_bits(11, 10, crate::city::TileBits::new(0x04)); // east
    city.set_terrain(11, 10, 0x10);
    city.set_tile_bits(10, 9, crate::city::TileBits::new(0x04)); // north
    city.set_terrain(10, 9, 0x10);
    assert_eq!(
        city.pe_sailboat_spawn_tile_from_marina(10, 10),
        Some((10, 9))
    );
    city.set_tile_bits(10, 9, crate::city::TileBits::new(0));
    assert_eq!(
        city.pe_sailboat_spawn_tile_from_marina(10, 10),
        Some((11, 10))
    );
}

#[test]
fn pe_sailboat_spawn_initializes_state_and_paints_xtxt() {
    // PE SpawnSailBoat paints XTXT slot-0x37 (`0x45353c`); XTRF is unchanged.
    let mut city = City::new_blank(
        std::path::Path::new("/tmp/sailboat-spawn.sc2"),
        "SailboatSpawn",
    )
    .expect("city");
    city.set_traffic_density(10, 10, 0x80);

    assert!(city.place_ambient_port_actor(10, 10, XTHG_SAILBOAT, 2));

    let (slot, sailboat) = city
        .traffic_overlays()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, record)| record.kind == XTHG_SAILBOAT)
        .map(|(s, r)| (s, *r))
        .expect("sailboat");
    assert_eq!(sailboat.state, 0);
    assert_eq!((sailboat.sub_x, sailboat.sub_y), (4, 4));
    assert_eq!(sailboat.height, 0);
    assert_eq!(sailboat.saved_a, 0);
    assert_eq!(sailboat.target_x, 0);
    assert_eq!(sailboat.target_y, 0);
    assert_eq!(city.traffic_density_at(10, 10), Some(0x80));
    assert_eq!(city.text_overlay_at(10, 10), xthg_proxy_for_slot(slot));
}
