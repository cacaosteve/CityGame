use super::*;

pub(super) fn ambient_subtile_step(kind: u8) -> i16 {
    // PE `FUN_004516F0` quantum from `0x4E81F8[arg]`:
    // plane `FUN_00453980` pushes `1` → `0x10`; heli `FUN_004540C0` pushes `2` → `0x08`.
    // Cargo/trains do not call `0x403071` (cargo uses `FUN_00456E10` ring; trains use
    // tile targets). Sailboat has no recovered push — keep provisional `2` until PE
    // dispatch for kinds 8/9 is resolved.
    match kind {
        XTHG_AIRPLANE => 0x10,
        XTHG_HELICOPTER => 0x08,
        // Kind-9 sailboat uses FUN_004562e0 / 0x456470 (±0x10 via 0x4e8350), not this path.
        XTHG_SAILBOAT => 2,
        _ => 4,
    }
}

/// Advance one ambient thing along its facing. Clears the slot when it leaves the map
/// or (for ships/trains) cannot turn onto a legal tile. Trains snap/branch at rail
/// junctions; ships occasionally wander on open water.
pub(super) fn step_ambient_actor<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    rng: &mut R,
    lfsr: &mut Win95Lfsr,
    slot: usize,
) -> bool {
    if !record.is_ambient_port_actor() || record.is_ambient_train_car() {
        return false;
    }
    if record.kind == XTHG_CARGO_SHIP {
        // Offline/generic path: LFSR state rolls only. Live ambient tick uses
        // `step_pe_cargo_actor_with_streams` for the CRT SFX gate first.
        return step_pe_cargo_actor_mut(record, city, lfsr, slot);
    }
    // PE `FUN_004562e0` (kind `0x09`): dedicated sailboat FSM (not ambient free-fly).
    if record.kind == XTHG_SAILBOAT {
        return step_pe_sailboat(record, city, rng, lfsr, slot);
    }
    if record.is_ambient_train_engine() {
        let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
        return match step_ambient_train_engine(record, city, rng, &mut game_rand, None) {
            TrainEngineStep::Advanced | TrainEngineStep::Cleared => true,
            TrainEngineStep::Blocked => false,
            TrainEngineStep::NeedsTurnaround => {
                pe_clear_xthg_kind(record);
                true
            }
        };
    }
    // PE `FUN_004540C0` @ `0x454123`: heli over XBLD `>= 0xfb` morphs in-place to
    // kind-6 / state 5 (no FUN_00453280, XTXT occupancy unchanged).
    if record.kind == XTHG_HELICOPTER {
        let x = record.tile_x as usize;
        let y = record.tile_y as usize;
        if tile_in_bounds(x, y) && city.building_at(x, y) >= 0xfb {
            record.kind = XTHG_EXPLOSION;
            record.state = 5;
            record.lane = 0;
            record.saved_b = 0;
            return true;
        }
        // PE state 0 @ `0x454167`: spin lane, climb height to `0xa`, then state 2.
        if record.state == 0 {
            record.lane = record.lane.wrapping_add(1) & 7;
            if record.height < 0xa {
                record.height = record.height.wrapping_add(1);
            } else {
                record.state = 2;
            }
            return true;
        }
        // PE state 1 @ jump `0x454415`: idle return (no move / no height change).
        if record.state == 1 {
            return true;
        }
        // PE state 3 @ `0x454355`: spin lane, descend height while `> 2`, else state 4.
        if record.state == 3 {
            record.lane = record.lane.wrapping_add(1) & 7;
            if record.height > 2 {
                record.height = record.height.wrapping_sub(1);
            } else {
                record.state = 4;
            }
            return true;
        }
        // PE state 4 @ `0x454384`: CRT `rand() % 0x14 == 0` → state 0; else idle.
        if record.state == 4 {
            if rng.roll_mod_is_zero(0x14) {
                record.state = 0;
            }
            return true;
        }
        // PE state 5 @ `0x4543aa`: lane+=2; descend while height `>2`; else kind-6/state 0x11.
        if record.state == 5 {
            record.lane = record.lane.wrapping_add(2) & 7;
            if record.height > 2 {
                record.height = record.height.wrapping_sub(1);
            } else {
                record.kind = XTHG_EXPLOSION;
                record.state = 0x11;
                record.saved_b = 1;
                record.lane = 0;
            }
            return true;
        }
    }
    // PE `FUN_00453980` runs this XBLD/XZON pre-dispatch for every kind-1 state,
    // including state-7 crash aircraft, before the state-specific jump table.
    if pe_plane_morph_on_building_with_lfsr(record, city, lfsr) {
        return true;
    }
    if record.kind == XTHG_AIRPLANE {
        // PE jump table states 5/6 @ `0x453ee4`: idle RET (`state & 0xf`).
        if matches!(record.state & 0xf, 5 | 6) {
            return true;
        }
        // PE state 7 @ `0x453e68`: height==0 → kind-6/state 5/+B=1/lane 0; else
        // height--, lane=(lane+1)&7, then one move (sound @ height==8 skipped).
        if record.state & 0xf == 7 {
            if record.height == 0 {
                record.kind = XTHG_EXPLOSION;
                record.state = 5;
                record.saved_b = 1;
                record.lane = 0;
                return true;
            }
            record.height = record.height.wrapping_sub(1);
            record.lane = record.lane.wrapping_add(1) & 7;
        }
    }
    let plane_phase = if record.kind == XTHG_AIRPLANE {
        record.state & 0xf
    } else {
        0xff
    };
    // PE heli state 2 @ `0x454196`: one-step turn toward `+8/+9` (FUN_00451550), then move.
    let heli_freefly = record.kind == XTHG_HELICOPTER && record.state == 2;
    if heli_freefly {
        let desired = pe_cargo_direction_to_target(
            record.tile_x,
            record.tile_y,
            record.target_x,
            record.target_y,
        );
        record.lane = pe_step_direction_toward(record.lane, desired);
        pe_heli_avoid_large_building(record, city);
    }
    // PE state 2 @ `0x453c02`: `FUN_00451440(lane, 5)` CRT jitter, then one move.
    if plane_phase == 2 {
        record.lane = pe_plane_maybe_jitter_dir(record.lane, 5, rng);
    }
    // PE state 4 @ `0x453d8b`: one-step turn toward `+8/+9`, then move + approach check.
    if plane_phase == 4 {
        let desired = pe_cargo_direction_to_target(
            record.tile_x,
            record.tile_y,
            record.target_x,
            record.target_y,
        );
        record.lane = pe_step_direction_toward(record.lane, desired);
    }
    // PE state 3 @ `0x453c4b`: absolute octant snap to `+8/+9`, then move + route check.
    let plane_route_abs = if plane_phase == 3 {
        let abs_dir = pe_plane_absolute_direction_to_target(
            record.tile_x,
            record.tile_y,
            record.target_x,
            record.target_y,
        );
        record.lane = abs_dir;
        Some(abs_dir)
    } else {
        None
    };
    let plane_takeoff = plane_phase == 0;
    let plane_landing = plane_phase == 1;
    let plane_approach = plane_phase == 4;
    let plane_route = plane_phase == 3;
    let old_x = record.tile_x as usize;
    let old_y = record.tile_y as usize;
    let (dx_unit, dy_unit) = xthg_direction_delta(record.lane);
    let step = ambient_subtile_step(record.kind);
    let (sub_x, tile_dx) = advance_subtile(record.sub_x as i16 + dx_unit * step);
    let (sub_y, tile_dy) = advance_subtile(record.sub_y as i16 + dy_unit * step);
    let new_x = record.tile_x as i16 + tile_dx;
    let new_y = record.tile_y as i16 + tile_dy;
    if new_x < 0 || new_y < 0 || new_x >= CITY_WIDTH as i16 || new_y >= CITY_HEIGHT as i16 {
        // PE `FUN_004507d0` @ `0x450821`: clear zeros XTXT at the actor tile.
        pe_clear_ambient_xtxt_occupancy(city, record);
        pe_clear_xthg_kind(record);
        return true;
    }
    let nx = new_x as usize;
    let ny = new_y as usize;
    let crossed = nx != old_x || ny != old_y;
    if crossed && !ambient_actor_can_occupy(record.kind, city, nx, ny) {
        if let Some(new_dir) = pick_ambient_turn(record, city, rng) {
            record.lane = new_dir;
            if plane_takeoff {
                pe_plane_state0_after_move(record);
            }
            if plane_landing {
                pe_plane_state1_after_move(record, city);
            }
            if plane_approach {
                pe_plane_state4_after_move(record);
            }
            if plane_route {
                pe_plane_state3_after_move(record, plane_route_abs.unwrap());
            }
            if heli_freefly {
                pe_heli_state2_after_move(record, city, rng);
            }
            return true;
        }
        pe_clear_ambient_xtxt_occupancy(city, record);
        pe_clear_xthg_kind(record);
        return true;
    }
    if crossed && matches!(record.kind, XTHG_AIRPLANE | XTHG_HELICOPTER) {
        // PE `FUN_004516F0` @ `0x451821`/`0x451848`/`0x45185c`: restore +A, then
        // while dest XTXT `>= 0xC9` commit tile and re-add the tile carry; OOB next
        // → `FUN_00401528`; clear neighbor paints past heavy (heavy XTXT unchanged).
        if old_x < CITY_WIDTH && old_y < CITY_HEIGHT {
            city.set_text_overlay(old_x, old_y, record.saved_a);
        }
        let mut cx = new_x;
        let mut cy = new_y;
        loop {
            let nx = cx as usize;
            let ny = cy as usize;
            let dest_xtxt = city.text_overlay_at(nx, ny).unwrap_or(0);
            if dest_xtxt < XTHG_EXPLOSION_XTXT_GATE {
                record.sub_x = sub_x as u8;
                record.sub_y = sub_y as u8;
                record.tile_x = nx as u8;
                record.tile_y = ny as u8;
                if !matches!(record.kind, XTHG_AIRPLANE | XTHG_HELICOPTER) {
                    record.target_x = record.tile_x;
                    record.target_y = record.tile_y;
                }
                record.saved_a = dest_xtxt;
                if let Some(proxy) = xthg_proxy_for_slot(slot) {
                    city.set_text_overlay(nx, ny, proxy);
                }
                maybe_ambient_junction_or_wander(record, city, rng);
                if plane_takeoff {
                    pe_plane_state0_after_move(record);
                }
                if plane_landing {
                    pe_plane_state1_after_move(record, city);
                }
                if plane_approach {
                    pe_plane_state4_after_move(record);
                }
                if plane_route {
                    pe_plane_state3_after_move(record, plane_route_abs.unwrap());
                }
                if heli_freefly {
                    pe_heli_state2_after_move(record, city, rng);
                }
                return true;
            }
            record.sub_x = sub_x as u8;
            record.sub_y = sub_y as u8;
            record.tile_x = nx as u8;
            record.tile_y = ny as u8;
            cx += tile_dx;
            cy += tile_dy;
            if cx < 0 || cy < 0 || cx >= CITY_WIDTH as i16 || cy >= CITY_HEIGHT as i16 {
                pe_zero_tile_xtxt(city, nx, ny);
                pe_clear_xthg_kind(record);
                return true;
            }
        }
    }
    record.sub_x = sub_x as u8;
    record.sub_y = sub_y as u8;
    record.tile_x = new_x as u8;
    record.tile_y = new_y as u8;
    if !matches!(record.kind, XTHG_AIRPLANE | XTHG_HELICOPTER) {
        record.target_x = record.tile_x;
        record.target_y = record.tile_y;
    }
    if crossed {
        if matches!(record.kind, XTHG_AIRPLANE | XTHG_HELICOPTER | XTHG_SAILBOAT) {
            // Same XTXT occupancy handoff as cargo / FUN_004516F0: restore +A, paint proxy.
            record.saved_a =
                city.move_ambient_cargo_xthg_proxy(slot, old_x, old_y, record.saved_a, nx, ny);
        }
        sync_train_kind_to_tile(record, city);
        maybe_ambient_junction_or_wander(record, city, rng);
    }
    if plane_takeoff {
        pe_plane_state0_after_move(record);
    }
    if plane_landing {
        pe_plane_state1_after_move(record, city);
    }
    if plane_approach {
        pe_plane_state4_after_move(record);
    }
    if plane_route {
        pe_plane_state3_after_move(record, plane_route_abs.unwrap());
    }
    if heli_freefly {
        pe_heli_state2_after_move(record, city, rng);
    }
    true
}

/// After entering a tile: trains follow curves / branch at junctions; ships may wander.
fn maybe_ambient_junction_or_wander<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &City,
    rng: &mut R,
) {
    let x = record.tile_x as usize;
    let y = record.tile_y as usize;
    let current = record.lane % 8;
    match record.kind {
        XTHG_TRAIN_ENGINE | XTHG_SUBWAY_TRAIN_ENGINE => {
            let mut game_rand = Win95GameRand::new(Win95GameRand::DEFAULT_STATE);
            if let Some(direction) = pick_pe_train_route(record, city, rng, &mut game_rand) {
                record.lane = direction;
            } else {
                pe_clear_xthg_kind(record);
            }
        }
        XTHG_CARGO_SHIP | XTHG_SAILBOAT => {
            if !tile_is_water_surface(city, x, y) {
                return;
            }
            // Occasional open-water wander (~1/8 tile crossings).
            if !rng.roll_mod_is_zero(8) {
                return;
            }
            let options = ambient_legal_exits(record.kind, city, x, y, current, true, false);
            if options.len() < 2 {
                return;
            }
            let left = current.wrapping_add(6) % 8;
            let right = current.wrapping_add(2) % 8;
            let turn_options: Vec<u8> = options
                .iter()
                .copied()
                .filter(|dir| *dir == left || *dir == right)
                .collect();
            if !turn_options.is_empty() && !rng.roll_mod_is_zero(2) {
                let idx = (rng.random_u16() as usize) % turn_options.len();
                record.lane = turn_options[idx];
            } else {
                let idx = (rng.random_u16() as usize) % options.len();
                record.lane = options[idx];
            }
        }
        _ => {}
    }
}
