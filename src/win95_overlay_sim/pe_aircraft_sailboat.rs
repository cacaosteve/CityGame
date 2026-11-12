//! Win95 PE aircraft and sailboat actor state helpers.
//!
//! Ambient dispatch order and shared XTXT occupancy ownership remain in the parent adapter.

use super::{
    City, PeAircraftSlotTickStats, Win95CrtRand, Win95Lfsr, Win95OverlayRng, Win95OverlaySim,
    advance_subtile, pe_clear_ambient_xtxt_occupancy, pe_clear_xthg_kind, pe_step_direction_toward,
    pe_zero_tile_xtxt, step_ambient_actor, tile_in_bounds,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH};
use crate::win95_overlay::{
    Win95OverlayRecord, XTHG_AIRPLANE, XTHG_EXPLOSION, XTHG_HELICOPTER, XTHG_SAILBOAT,
    xthg_proxy_for_slot,
};

impl Win95OverlaySim {
    /// Step an ordinary kind-1 airplane or kind-2 helicopter in native slot
    /// order. Air-crash state 7 is handled by the dedicated disaster path.
    pub fn tick_pe_aircraft_slot(
        &mut self,
        city: &mut City,
        slot: usize,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) -> bool {
        self.tick_pe_aircraft_slot_with_stats(city, slot, crt, lfsr)
            .changed
    }

    pub fn tick_pe_aircraft_slot_with_stats(
        &mut self,
        city: &mut City,
        slot: usize,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) -> PeAircraftSlotTickStats {
        let table_snapshot = city
            .traffic_overlays()
            .map(|table| table.to_vec())
            .unwrap_or_default();
        let Some(mut record) = table_snapshot.get(slot).copied() else {
            return PeAircraftSlotTickStats::default();
        };
        if !matches!(record.kind, XTHG_AIRPLANE | XTHG_HELICOPTER) || record.is_pe_aircrash_actor()
        {
            return PeAircraftSlotTickStats::default();
        }
        let old_x = record.tile_x as usize;
        let old_y = record.tile_y as usize;
        let mayday_sounds = usize::from(
            record.kind == XTHG_HELICOPTER
                && record.state == 5
                && record.height == 4
                && !(tile_in_bounds(old_x, old_y) && city.building_at(old_x, old_y) >= 0xfb),
        );
        if !step_ambient_actor(&mut record, city, crt, lfsr, slot) {
            return PeAircraftSlotTickStats::default();
        }
        if let Some(overlays) = city.traffic_overlays_mut() {
            overlays[slot] = record;
        }
        city.mark_render_dirty(old_x, old_y);
        if record.kind != 0 {
            city.mark_render_dirty(record.tile_x as usize, record.tile_y as usize);
        }
        city.mark_traffic_layers_dirty();
        PeAircraftSlotTickStats {
            changed: true,
            mayday_sounds,
        }
    }

    /// Step a kind-9 sailboat at its position in the Win95 XTHG slot walk.
    /// The dispatcher resets the shared sailboat-handler budget once per pass.
    pub fn tick_pe_sailboat_slot(
        &mut self,
        city: &mut City,
        slot: usize,
        crt: &mut Win95CrtRand,
        lfsr: &mut Win95Lfsr,
    ) -> bool {
        let Some(mut record) = city
            .traffic_overlays()
            .and_then(|table| table.get(slot).copied())
        else {
            return false;
        };
        if record.kind != XTHG_SAILBOAT {
            return false;
        }

        let old_x = usize::from(record.tile_x);
        let old_y = usize::from(record.tile_y);
        city.pe_sailboat_budget = city.pe_sailboat_budget.saturating_add(1);
        if city.pe_sailboat_budget > 4 {
            // PE `0x4562e8`: the fifth kind-9 handler call clears the actor.
            pe_clear_ambient_xtxt_occupancy(city, &record);
            pe_clear_xthg_kind(&mut record);
        } else if !step_ambient_actor(&mut record, city, crt, lfsr, slot) {
            return false;
        }

        if let Some(overlays) = city.traffic_overlays_mut() {
            overlays[slot] = record;
        }
        city.mark_render_dirty(old_x, old_y);
        if record.kind != 0 {
            city.mark_render_dirty(usize::from(record.tile_x), usize::from(record.tile_y));
        }
        city.mark_traffic_layers_dirty();
        true
    }
}

/// PE sprite width for plane mid-range morph (`FUN_00453980` @ `0x453a4c`).
/// Heap table `[0x4cb1b8] + xbld*8 + 4` is filled from `SMALLMED.DAT` index `+8`
/// (BE width). For XBLD `0x71..=0xfa` those widths equal footprint tiles × 8.
fn pe_plane_morph_xbld_sprite_width_px(xbld: u8) -> u16 {
    let tiles: u16 = match xbld {
        97..=107 | 140..=173 | 235..=247 => 2,
        174..=197 | 208..=214 | 248..=250 => 3,
        201..=207 | 215..=218 => 4,
        _ => 1,
    };
    tiles.saturating_mul(8)
}

/// Win95 `FUN_00453980` pre-dispatch obstruction morph at `0x4539e7..0x453a95`.
/// Shared by ordinary kind-1 flight and state-7 disaster aircraft, before either
/// path enters its state-specific arm. The `>=0xfb` arm consumes game LFSR.
pub(super) fn pe_plane_morph_on_building_with_lfsr(
    record: &mut Win95OverlayRecord,
    city: &City,
    lfsr: &mut Win95Lfsr,
) -> bool {
    if record.kind != XTHG_AIRPLANE {
        return false;
    }
    let x = record.tile_x as usize;
    let y = record.tile_y as usize;
    if !tile_in_bounds(x, y) {
        return false;
    }
    let xbld = city.building_at(x, y);
    if xbld <= 0x70 || city.zone_at(x, y).is_some_and(|z| z.zone_id() == 8) {
        return false;
    }
    if xbld >= 0xfb {
        record.kind = XTHG_EXPLOSION;
        record.state = 5;
        record.lane = 0;
        record.saved_b = if lfsr.random_mod(0x10) != 0 { 0 } else { 1 };
        return true;
    }
    if pe_plane_morph_xbld_sprite_width_px(xbld) / 3 > u16::from(record.height) {
        record.kind = XTHG_EXPLOSION;
        record.state = 5;
        record.saved_b = 1;
        record.lane = 0;
        return true;
    }
    false
}

/// PE `FUN_004540C0` state 2 @ `0x4541f1` after move: if manhattan to `+8/+9` `< 2`,
/// re-roll targets near city center (`0x4cb458/0x4cb45c` ± `rand&0x3f-0x20`, clamp);
/// with `rand&1` and empty XBLD+XTER at the heli tile, enter state 3.
/// Skips the optional GetTickCount / `0x4e99e0` XTRF-threshold sound arm.
pub(super) fn pe_heli_state2_after_move<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &City,
    rng: &mut R,
) {
    if record.kind != XTHG_HELICOPTER || record.state != 2 {
        return;
    }
    let dist = (i16::from(record.tile_x) - i16::from(record.target_x)).unsigned_abs()
        + (i16::from(record.tile_y) - i16::from(record.target_y)).unsigned_abs();
    if dist >= 2 {
        return;
    }
    // PE reads `wCityCenter` at `0x4cb458/0x4cb45c` (land-value building centroid).
    let (cx, cy) = city.pe_city_center();
    let mut tx = cx.wrapping_add((rng.random_u16() & 0x3f) as i16 - 0x20);
    let mut ty = cy.wrapping_add((rng.random_u16() & 0x3f) as i16 - 0x20);
    // PE clamps against map size `0x80`: out-of-range → `rand&0x3f+0x20`.
    if tx < 0 || tx >= 0x80 {
        tx = (rng.random_u16() & 0x3f) as i16 + 0x20;
    }
    if ty < 0 || ty >= 0x80 {
        ty = (rng.random_u16() & 0x3f) as i16 + 0x20;
    }
    record.target_x = tx as u8;
    record.target_y = ty as u8;
    if rng.random_u16() & 1 == 0 {
        return;
    }
    let x = record.tile_x as usize;
    let y = record.tile_y as usize;
    if city.building_at(x, y) != 0 || city.terrain_at(x, y) != 0 {
        return;
    }
    record.state = 3;
}

/// PE `FUN_00454520` (`0x454520`) state-2 lookahead. It checks the XBLD tile
/// three tiles ahead (`FUN_004545c0`, `0x4545c0`); if blocked by `>=0xfb`, it
/// tries relative headings `[+1,-1,+2,-2,+3,-3,+4]` and keeps the first clear
/// one. If every candidate is blocked, native leaves the last (`+4`) heading.
pub(super) fn pe_heli_avoid_large_building(record: &mut Win95OverlayRecord, city: &City) {
    const X_LOOKAHEAD: [i16; 8] = [0, 3, 3, 3, 0, -3, -3, -3];
    const Y_LOOKAHEAD: [i16; 8] = [-3, -3, 0, 3, 3, 3, 0, -3];
    const SEARCH_OFFSETS: [u8; 7] = [1, 7, 2, 6, 3, 5, 4];

    let x = i16::from(record.tile_x);
    let y = i16::from(record.tile_y);
    let clear = |direction: u8| {
        let direction = usize::from(direction & 7);
        let nx = x + X_LOOKAHEAD[direction];
        let ny = y + Y_LOOKAHEAD[direction];
        nx >= 0
            && ny >= 0
            && nx < CITY_WIDTH as i16
            && ny < CITY_HEIGHT as i16
            && city.building_at(nx as usize, ny as usize) < 0xfb
    };

    let current = record.lane & 7;
    if clear(current) {
        return;
    }
    for offset in SEARCH_OFFSETS {
        record.lane = current.wrapping_add(offset) & 7;
        if clear(record.lane) {
            return;
        }
    }
}

/// PE `FUN_004562e0` kind-9 sailboat. Lanes are cardinal `0..=3` (N,E,S,W), not XTHG octants.
pub(super) fn step_pe_sailboat<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    rng: &mut R,
    lfsr: &mut Win95Lfsr,
    slot: usize,
) -> bool {
    // State != 0 @ `0x456320`: `0x401cad(5)==0` → despawn, else idle.
    if record.state != 0 {
        if lfsr.random_mod(5) == 0 {
            pe_clear_ambient_xtxt_occupancy(city, record);
            pe_clear_xthg_kind(record);
        }
        return true;
    }
    // State 0 @ `0x456344`: `0x401cad(4)==0` → water drift; else neighbor probe + subtile step.
    if lfsr.random_mod(4) == 0 {
        pe_sailboat_state0_drift(record, city, rng, lfsr);
        return true;
    }
    if pe_sailboat_neighbor_probe(record, city) {
        pe_sailboat_subtile_step(record, city, slot);
    }
    true
}

fn pe_sailboat_state0_drift<R: Win95OverlayRng>(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    rng: &mut R,
    lfsr: &mut Win95Lfsr,
) {
    if record.tile_x >= 0x80 || record.tile_y >= 0x80 {
        pe_clear_ambient_xtxt_occupancy(city, record);
        pe_clear_xthg_kind(record);
        return;
    }
    let x = record.tile_x as usize;
    let y = record.tile_y as usize;
    if !city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.water_tile())
    {
        pe_clear_ambient_xtxt_occupancy(city, record);
        pe_clear_xthg_kind(record);
        return;
    }
    if lfsr.random_mod(0xfa0) == 0 {
        record.state = 1;
    }
    let lane = u16::from(record.lane & 3);
    let delta = (rng.random_u16() % 3) as i16 - 1;
    record.lane = (lane as i16 + delta).rem_euclid(4) as u8;
}

/// PE `0x456600`: tile-neighbor probe for sailboat lane (`0x4e8360/0x4e8370`).
fn pe_sailboat_neighbor_probe(record: &mut Win95OverlayRecord, city: &mut City) -> bool {
    const DX: [i16; 4] = [0, 1, 0, -1];
    const DY: [i16; 4] = [-1, 0, 1, 0];
    let lane = (record.lane & 3) as usize;
    let nx = i16::from(record.tile_x) + DX[lane];
    let ny = i16::from(record.tile_y) + DY[lane];
    // PE returns success on OOB so the follow-up step can despawn.
    if nx < 0 || ny < 0 || nx >= 0x80 || ny >= 0x80 {
        return true;
    }
    let nx = nx as usize;
    let ny = ny as usize;
    let building = city.building_at(nx, ny);
    if building == 0xf8 {
        pe_clear_ambient_xtxt_occupancy(city, record);
        pe_clear_xthg_kind(record);
        return false;
    }
    if building == 0xdf {
        return false;
    }
    if city.text_overlay_at(nx, ny).unwrap_or(0) != 0 {
        return false;
    }
    city.tile_bits_at(nx, ny)
        .is_some_and(|bits| bits.water_tile())
}

/// PE `0x456470`: subtile step via `0x4e8350/0x4e8358` (±0x10); tile-cross zeros old XTXT.
pub(super) fn pe_sailboat_subtile_step(
    record: &mut Win95OverlayRecord,
    city: &mut City,
    slot: usize,
) {
    const DSUB_X: [i16; 4] = [0, 16, 0, -16];
    const DSUB_Y: [i16; 4] = [-16, 0, 16, 0];
    let lane = (record.lane & 3) as usize;
    let (sub_x, tile_dx) = advance_subtile(i16::from(record.sub_x) + DSUB_X[lane]);
    let (sub_y, tile_dy) = advance_subtile(i16::from(record.sub_y) + DSUB_Y[lane]);
    record.sub_x = sub_x as u8;
    record.sub_y = sub_y as u8;
    if tile_dx == 0 && tile_dy == 0 {
        return;
    }
    let old_x = record.tile_x as usize;
    let old_y = record.tile_y as usize;
    if old_x < CITY_WIDTH && old_y < CITY_HEIGHT {
        city.set_text_overlay(old_x, old_y, 0);
    }
    let nx = i16::from(record.tile_x) + tile_dx;
    let ny = i16::from(record.tile_y) + tile_dy;
    // PE `0x456470` @ `0x456553`/`0x456559`: `cmp …,0x7f` / `jnl` → `FUN_00401528`.
    if nx < 0 || ny < 0 || nx >= 0x7f || ny >= 0x7f {
        pe_clear_xthg_kind(record);
        return;
    }
    record.tile_x = nx as u8;
    record.tile_y = ny as u8;
    if let Some(proxy) = xthg_proxy_for_slot(slot) {
        city.set_text_overlay(nx as usize, ny as usize, proxy);
    }
}

/// PE `FUN_00453980` state 0 @ `0x453ae8`: after one move, climb while `height < 0xe`,
/// else enter state 2.
pub(super) fn pe_plane_state0_after_move(record: &mut Win95OverlayRecord) {
    if record.kind != XTHG_AIRPLANE || record.state & 0xf != 0 {
        return;
    }
    if record.height < 0xe {
        record.height = record.height.wrapping_add(1);
    } else {
        record.state = 2;
    }
}

/// PE `FUN_00453980` state 1 @ `0x453b62`: after one move, `height--`; at 0 restore XTXT
/// from `+A`, then runway XBLD `0xdd` → `FUN_00401528` clear, else kind-6/state 5.
pub(super) fn pe_plane_state1_after_move(record: &mut Win95OverlayRecord, city: &mut City) {
    if record.kind != XTHG_AIRPLANE || record.state & 0xf != 1 {
        return;
    }
    record.height = record.height.wrapping_sub(1);
    if record.height >= 1 {
        return;
    }
    let x = record.tile_x as usize;
    let y = record.tile_y as usize;
    if tile_in_bounds(x, y) {
        city.set_text_overlay(x, y, record.saved_a);
        if city.building_at(x, y) == 0xdd {
            pe_zero_tile_xtxt(city, x, y);
            pe_clear_xthg_kind(record);
            return;
        }
    }
    record.kind = XTHG_EXPLOSION;
    record.state = 5;
    record.saved_b = 1;
    record.lane = 0;
}

/// PE `FUN_00453980` state 4 @ `0x453e00`: after move, if manhattan to `+8/+9` `< 2`,
/// restore lane from `state>>4`, set state 1, and snap tile on cardinal land lanes.
pub(super) fn pe_plane_state4_after_move(record: &mut Win95OverlayRecord) {
    if record.kind != XTHG_AIRPLANE || record.state & 0xf != 4 {
        return;
    }
    let dist = (i16::from(record.tile_x) - i16::from(record.target_x)).unsigned_abs()
        + (i16::from(record.tile_y) - i16::from(record.target_y)).unsigned_abs();
    if dist >= 2 {
        return;
    }
    let land_lane = record.state >> 4;
    record.lane = land_lane;
    record.state = 1;
    match land_lane {
        1 | 5 => record.tile_x = record.target_x,
        3 | 7 => record.tile_y = record.target_y,
        _ => {}
    }
}

/// PE `FUN_00451440` / thunk `0x401848`: keep `lane`, or with 1-in-`modulus` odds
/// apply `(rand%3)-1` then `&7` (same contract as `pe_xthg_maybe_jitter_dir`).
pub(super) fn pe_plane_maybe_jitter_dir<R: Win95OverlayRng>(
    lane: u8,
    modulus: u16,
    rng: &mut R,
) -> u8 {
    if modulus == 0 || !rng.roll_mod_is_zero(modulus) {
        return lane & 7;
    }
    let delta = (rng.random_u16() % 3) as i16 - 1;
    (i16::from(lane) + delta).rem_euclid(8) as u8
}

/// PE `FUN_004514e0` @ `0x4514e0`: absolute heading from `(x,y)` toward `(tx,ty)`.
/// Sign-octant into XTHG dirs (`0` N, `2` E, `4` S, `6` W); equal `(dx,dy)` → `4`.
pub(super) fn pe_plane_absolute_direction_to_target(x: u8, y: u8, tx: u8, ty: u8) -> u8 {
    let dx = i16::from(tx) - i16::from(x);
    let dy = i16::from(ty) - i16::from(y);
    if dx < 0 {
        if dy < 0 {
            return 7;
        }
        // setng: dy <= 0 → 6, else 5 (dy < 0 already returned).
        return if dy <= 0 { 6 } else { 5 };
    }
    if dx > 0 {
        if dy < 0 {
            return 1;
        }
        // setg: dy > 0 → 3, else 2.
        return if dy > 0 { 3 } else { 2 };
    }
    // dx == 0: setl; dec; and 4 → dy < 0 → 0, else 4 (including equal).
    if dy < 0 { 0 } else { 4 }
}

/// PE `FUN_00453980` state 3 @ `0x453c88`/`0x453d40`: after move, if manhattan to
/// `+8/+9` `< 2`, turn one step toward packed land lane, enter state 4 with the
/// same high nibble, and set approach target ±6 by land lane.
pub(super) fn pe_plane_state3_after_move(record: &mut Win95OverlayRecord, abs_dir: u8) {
    if record.kind != XTHG_AIRPLANE || record.state & 0xf != 3 {
        return;
    }
    let dist = (i16::from(record.tile_x) - i16::from(record.target_x)).unsigned_abs()
        + (i16::from(record.tile_y) - i16::from(record.target_y)).unsigned_abs();
    if dist >= 2 {
        return;
    }
    let land_lane = record.state >> 4;
    record.lane = pe_step_direction_toward(abs_dir, land_lane);
    record.state = (land_lane << 4) | 4;
    match land_lane {
        1 => {
            record.target_y = record.target_y.wrapping_sub(6);
        }
        3 => {
            record.target_x = record.target_x.wrapping_add(6);
        }
        5 => {
            record.target_y = record.target_y.wrapping_add(6);
        }
        7 => {
            record.target_x = record.target_x.wrapping_sub(6);
        }
        _ => {}
    }
}
