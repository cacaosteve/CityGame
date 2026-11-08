use super::*;

/// PE Raise tool-0 (`0x442db0`) reads submode word `0x4cc950` and dispatches
/// `0..=4` via jump table `0x443104` (ja refuses `> 4`).
pub(in crate::app) const PE_RAISE_SUBMODE_MAX: u16 = 4;

pub(in crate::app) const PE_RAISE_SUBMODE_DEFAULT: u16 = 0;

pub(in crate::app) const PE_RAISE_SUBMODE_ADJUST_TOWARD: u16 = 1;

/// PE Raise submode-4 (`0x442efa`→`0x4495f0`); hold skips tile-change body (`0x442fac`).
pub(in crate::app) const PE_RAISE_SUBMODE_HUGE: u16 = 4;

/// PE Raise submode jump table `0x443104` targets (VA).
pub(in crate::app) const PE_RAISE_SUBMODE_ARM_VA: [u32; 5] = [
    0x442eb3, // 0: `0x401cc1`→`0x443270`
    0x442ec3, // 1: `0x401b9f`→`0x443200` adjust-toward
    0x442ed8, // 2: `0x4024be`→`0x443c60`
    0x442ee8, // 3: `0x402ce8`→`0x444440` mode 1
    0x442efa, // 4: `0x4010d2`→`0x4495f0`
];

/// PE Raise submode-0 bills `0x4dc140[0]` == 1 before the ALTM path (`0x44338a`).
pub(in crate::app) const PE_RAISE_SUBMODE0_FUND_COST: i32 = 1;

/// PE `0x4441c0` bills `[0x4dc148]` == 0x19 before land++ (`0x44432e`).
pub(in crate::app) const PE_RAISE_4441C0_FUND_COST: i32 = 0x19;

/// PE Raise submode-0 mid-band CRT refuse modulus (`0x44347e`: `rand() % 0x14`).
pub(in crate::app) const PE_RAISE_SUBMODE0_MIDBAND_CRT_MOD: u16 = 0x14;

/// PE Raise submode-0 mid-band XBLD lower bound (`0x443470`: `cmp bx,6`).
pub(in crate::app) const PE_RAISE_SUBMODE0_MIDBAND_XBLD_MIN: u8 = 6;

/// PE Raise submode-0 DestroyBuilding XBLD gate (`0x443450`: `cmp bx,0x0d`).
pub(in crate::app) const PE_RAISE_SUBMODE0_DESTROY_XBLD_MIN: u8 = 0x0d;

/// PE midband refuse `0x402c4d` args @ `0x4434aa`–`0x4434b4`: `(0x193, 0xec, 1)`.
pub(in crate::app) const PE_RAISE_MIDBAND_REFUSE_DIALOG_402C4D_A: u32 = 0x193;

pub(in crate::app) const PE_RAISE_MIDBAND_REFUSE_DIALOG_402C4D_B: u32 = 0xec;

pub(in crate::app) const PE_RAISE_MIDBAND_REFUSE_DIALOG_402C4D_C: u32 = 1;

/// PE midband refuse `0x402900` args @ `0x4434bc`–`0x4434c0`: `(0x28, 0)`.
pub(in crate::app) const PE_RAISE_MIDBAND_REFUSE_UI_402900_A: u32 = 0x28;

pub(in crate::app) const PE_RAISE_MIDBAND_REFUSE_UI_402900_B: u32 = 0;

/// PE Raise submode-0 PlaceXUND(0) for XBLD `0xe9` (`0x443430`).
pub(in crate::app) const PE_RAISE_SUBMODE0_XUND_CLEAR_XBLD_E9: u8 = 0xe9;

/// PE Raise submode-0 PlaceXUND(0) for XBLD `0x6c..=0x70` (`0x443438`).
pub(in crate::app) const PE_RAISE_SUBMODE0_XUND_CLEAR_LO: u8 = 0x6c;

pub(in crate::app) const PE_RAISE_SUBMODE0_XUND_CLEAR_HI: u8 = 0x70;

/// PE Raise submode-0 XBIT clear mask when `[0x4e9e48]≠0` (`0x4433f0`: `and …,0xdf`).
pub(in crate::app) const PE_RAISE_SUBMODE0_XBIT_CLEAR_MASK: u8 = 0xdf;

/// PE Raise submode-0 XBIT-clear XBLD upper exclusive bound (`0x4433d0`: `cmp bx,0x70`).
pub(in crate::app) const PE_RAISE_SUBMODE0_XBIT_CLEAR_XBLD_MAX: u8 = 0x70;

/// PE billed Raise submode-0 post-debit `0x4012c1` @ `0x4433bf` (once per success).
pub(in crate::app) const PE_RAISE_SUBMODE0_POST_DEBIT_UI_WAIT_4012C1: u32 = 1;

/// PE Raise tool-0 @ `0x442e8b`: XTER≠0 → UI byte at `[0x4ca434]+5` = 1, else 0.
pub(in crate::app) fn pe_raise_tool0_xter_ui_byte_442e8b(city: &City, x: usize, y: usize) -> u8 {
    if city.terrain_at(x, y) != 0 { 1 } else { 0 }
}

/// PE Raise tool-0 entry @ `0x442e10`: MapTool UI tool-id byte `[0x4ca434]+0` = 4.
pub(in crate::app) const PE_RAISE_TOOL0_UI_TOOL_ID: u8 = 4;

/// PE Raise tool-0 after step @ `0x443003`/`0x44300c`: UI tile X at `+3`, Y at `+4`.
pub(in crate::app) fn pe_raise_tool0_ui_tile_xy_443003(x: usize, y: usize) -> (u8, u8) {
    (x as u8, y as u8)
}

/// PE Raise tool-0 after step @ `0x442ff4`: UI byte `+1` is the press-tile Y (`[esp+0x14]`).
pub(in crate::app) fn pe_raise_tool0_ui_press_y_byte_442ff4(press_y: usize) -> u8 {
    press_y as u8
}

/// PE Raise tool-0 temporary XTXT `0xc9` stamp (`0x442e5c` / `0x442fe7`).
pub(in crate::app) fn pe_raise_tool0_stamp_temp_xtxt_0xc9(city: &mut City, x: usize, y: usize) {
    if city.text_overlay_at(x, y).unwrap_or(0) == 0 {
        city.set_text_overlay(x, y, 0xc9);
    }
}

/// PE Raise tool-0 clears temporary XTXT `0xc9` when leaving a tile / exiting
/// the tool (`0x443064` / `0x443094` / `0x4430c3`).
pub(in crate::app) fn pe_raise_tool0_clear_temp_xtxt_0xc9(city: &mut City, x: usize, y: usize) {
    if city.text_overlay_at(x, y) == Some(0xc9) {
        city.set_text_overlay(x, y, 0);
    }
}

/// Win95 Raise tool-0 drag step direction `0x4015af`→`0x445da0`.
/// Args are (new_x, new_y, prev_x, prev_y). Same tile → `0xffff`.
/// Dominant axis: east=1, west=3, south=2, north=0.
pub(in crate::app) fn pe_raise_drag_step_dir_445da0(
    new_x: i16,
    new_y: i16,
    prev_x: i16,
    prev_y: i16,
) -> u16 {
    if new_x == prev_x && new_y == prev_y {
        return 0xffff;
    }
    let dx = new_x.wrapping_sub(prev_x);
    let dy = new_y.wrapping_sub(prev_y);
    let abs_dx = dx.wrapping_abs();
    let abs_dy = dy.wrapping_abs();
    if abs_dx > abs_dy {
        if dx > 0 { 1 } else { 3 }
    } else if dy > 0 {
        2
    } else {
        0
    }
}

/// Win95 Raise submode-1 helper `0x401b9f`→`0x443200`: one step toward `target_land`.
/// Land `<` target → raise via `0x4024be`→`0x443c60` (land++ only here; no `0x445b50`).
/// Land `>` target → dig `0x402ce8`→`0x444440` (mode 1, including queued neighbor-height
/// balancing and the padded dirty-rectangle `0x447490` reclassifier).
/// Equal → success. Bounds / funds / UI omitted.
pub(in crate::app) fn pe_raise_adjust_land_toward_443200(
    city: &mut City,
    x: usize,
    y: usize,
    target_land: u8,
) -> bool {
    pe_raise_adjust_land_toward_443200_with_crt(city, None, x, y, target_land).is_some()
}

/// PE `0x443200`: one step toward press-tile land. Raise arm is `0x443c60` (waits);
/// dig arm is mode-1 `0x444440`, including its queue and dirty-rectangle refresh.
/// Equal land returns success with 0 waits.
pub(in crate::app) fn pe_raise_adjust_land_toward_443200_with_crt(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    target_land: u8,
) -> Option<u32> {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return None;
    }
    let sample = city.altm_at(x, y);
    let land = sample.land & 0x1f;
    let target = target_land & 0x1f;
    if land < target {
        pe_raise_land_inc_443c60_with_crt(city, crt, x, y, PE_RAISE_SUBMODE_ADJUST_TOWARD)
    } else if land > target {
        // PE adjust dig pushes mode 1 @ `0x44323e` (bills `0x4dc140`[submode]) and
        // runs `0x447490`'s nine-cell DestroyBuilding/terrain refresh on success.
        if let Some(effects) = pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt_effects(
            city,
            crt.as_deref_mut(),
            x,
            y,
            1,
            PE_RAISE_SUBMODE_ADJUST_TOWARD,
        ) {
            Some(effects.ui_waits_4012c1)
        } else {
            None
        }
    } else {
        Some(0)
    }
}

/// Win95 Raise `0x4024be`→`0x443c60` / submode-2 arm.
/// Refuses land `>= 0x1e` (`0x443dd3` / `0x444274`). First `0x443cb0` gate: zone low
/// nibble `7` returns 0 (`0x443d03`). Sets flatten mark (`0x443db4`) before the
/// `>= 0x1e` refuse. Neighbor military/water clears that mark and refuses.
/// Funds `[0x4dc148]` then land++, XZON `&= 0xf0`, `0x445b50`, and `0x447490`.
/// Returns PE `0x4012c1` wait count on success (`0x445b50` adjusts + `0x447490` ×18, incl. recurse).
/// A lower NESW neighbor is raised first when its flatten mark is set (`0x44427e`).
pub(in crate::app) fn pe_raise_land_inc_443c60(
    city: &mut City,
    x: usize,
    y: usize,
    submode: u16,
) -> Option<u32> {
    pe_raise_land_inc_443c60_with_crt(city, None, x, y, submode)
}

pub(in crate::app) fn pe_raise_land_inc_443c60_with_crt(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    submode: u16,
) -> Option<u32> {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return None;
    }
    let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    if zone & 0x0f == ZONE_MILITARY {
        return None;
    }
    // PE `0x443d3f`: water bit refuses only when submode word `0x4cc950` is 0.
    if submode == PE_RAISE_SUBMODE_DEFAULT
        && city.tile_bits_at(x, y).is_some_and(TileBits::water_tile)
    {
        return None;
    }
    let sample = city.altm_at(x, y);
    let land = sample.land & 0x1f;
    // PE `0x443d79`: flatten bit 8 already set → `0x443cb0` returns success and
    // skips the height/neighbor gates. Otherwise OR the bit (`0x443db4`) and
    // a height fail leaves it set.
    let already_marked = city.tile_bits_at(x, y).is_some_and(TileBits::flatten_mark);
    if !already_marked {
        pe_set_flatten_mark(city, x, y, true);
        if land >= 0x1e {
            return None;
        }
        // PE `0x443cb0` neighbor walk (`0x4e7858`/`0x4e7870`): military or water refuses.
        for (&dx, &dy) in PE_VOLCANO_FLATTEN_DX
            .iter()
            .zip(PE_VOLCANO_FLATTEN_DY.iter())
        {
            let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
                continue;
            };
            let nzone = city.zone_at(nx, ny).map(|z| z.raw_bits()).unwrap_or(0);
            if nzone & 0x0f == ZONE_MILITARY
                || city.tile_bits_at(nx, ny).is_some_and(TileBits::water_tile)
            {
                pe_set_flatten_mark(city, x, y, false);
                return None;
            }
        }
    }
    // PE `0x44425d`: `0x4441c0` clears flatten bit 8, then refuses land `>= 0x1e`.
    pe_set_flatten_mark(city, x, y, false);
    if land >= 0x1e {
        return None;
    }
    // PE `0x44427e`: before this tile's debit, recurse `0x4441c0` onto NESW
    // neighbors whose land is lower. The callee still requires its own flatten mark.
    let mut waits = 0u32;
    for (&dx, &dy) in PE_VOLCANO_FLATTEN_NESW_DX
        .iter()
        .zip(PE_VOLCANO_FLATTEN_NESW_DY.iter())
    {
        let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
            continue;
        };
        if city.altitude_at(nx, ny) & 0x1f >= land {
            continue;
        }
        if !city
            .tile_bits_at(nx, ny)
            .is_some_and(TileBits::flatten_mark)
        {
            continue;
        }
        if let Some(child) =
            pe_raise_land_inc_443c60_with_crt(city, crt.as_deref_mut(), nx, ny, submode)
        {
            waits = waits.wrapping_add(child);
        }
    }
    let cost = PE_RAISE_4441C0_FUND_COST;
    if city.city_funds() < cost {
        return None;
    }
    city.debit_city_funds(cost as u32);
    city.set_altitude(x, y, land + 1, sample.water);
    // PE `0x44439e`: XZON `&= 0xf0` after the `0x4441c0` land++.
    if let Some(zone) = city.zone_at(x, y) {
        city.set_zone(x, y, zone.raw_bits() & 0xf0);
    }
    // PE `0x4443a4`: `0x401a78`→`0x445b50` after the land write.
    waits = waits.wrapping_add(pe_post_flatten_neighbor_altm_445b50(city, x, y));
    // PE `0x4443ae`: `0x40215d`→`0x447490`.
    pe_terrain_neighbor_fun_00447020_447490(city, crt, x, y);
    Some(waits.wrapping_add(PE_447490_UI_WAIT_4012C1_PER_CALL))
}

/// PE Raise submode-0 `0x443270` with fund debit `0x4dc140[0]` @ `0x44338a`.
/// Edit-map Raise stays on the unbilled `pe_raise_tool0_submode_step` path.
/// XBLD `>= 0x0d` destroys via DestroyStructure before land++ (`0x443450`).
/// Mid-band XBLD `6..0x0c` takes CRT `rand()%0x14==0` refuse after debit (`0x443470`).
/// Otherwise PlaceTile(0) @ `0x4434d2` before land++ (`0x40178f`→`0x441f00`).
/// Then XTER `>= 0x30` water clear via `pe_maptool_surface_xter_gate_4434de` (`0x4434de`).
/// When `flag_4e9e48` is set, underground clear @ `0x4433ce` then land++ (skip surface).
pub(in crate::app) fn pe_raise_submode0_step_billed_443270(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
    flag_4e9e48: bool,
) -> Option<u32> {
    if pe_raise_submode0_early_refuse_443270(city, x, y, flag_4e9e48) {
        return None;
    }
    let cost = PE_RAISE_SUBMODE0_FUND_COST;
    if city.city_funds() < cost {
        return None;
    }
    city.set_city_funds(city.city_funds() - cost);
    // PE @ `0x4433bf`: `0x4012c1` once after the funds debit (both underground and surface).
    let post_debit = PE_RAISE_SUBMODE0_POST_DEBIT_UI_WAIT_4012C1;
    if flag_4e9e48 {
        // PE `0x4433ce`–`0x44342f`: piped clear / optional destroy / PlaceXUND(0), then
        // jmp `0x44378e` land++ (surface XUND/PlaceTile/XTER arms skipped).
        pe_maptool_underground_clear_subway_443427(city, Some(crt), x, y, true);
        let waits_445b50 = pe_raise_terrain_land_inc_445b50(city, x, y)?;
        pe_terrain_neighbor_fun_00447020_447490(city, Some(crt), x, y);
        return Some(
            post_debit
                .wrapping_add(waits_445b50)
                .wrapping_add(PE_447490_UI_WAIT_4012C1_PER_CALL),
        );
    }
    let xbld = city.building_at(x, y);
    if pe_raise_submode0_clears_xund_443430(xbld) {
        // PE `0x443430`/`0x443438`: PlaceXUND tile 0 before destroy/raise.
        pe_place_xund_442370(city, x, y, 0);
    }
    let destroyed = if xbld >= PE_RAISE_SUBMODE0_DESTROY_XBLD_MIN {
        // PE `0x443450`/`0x443461`: DestroyBuilding mode 1 (`0x402211`); elevated arm
        // or CRT PlaceTile rubble @ `0x45a01f`.
        pe_maptool_surface_destroy_building_443461(city, Some(crt), x, y)
    } else if pe_raise_submode0_midband_crt_refuse_443470(crt, xbld) {
        // PE already debited; refuse raise. Post-debit `0x4012c1` already ran in PE.
        return None;
    } else {
        // PE `0x4434d2`: PlaceTile(0) when XBLD < 0x0d and mid-band did not refuse.
        pe_place_tile_441f00(city, x, y, 0);
        false
    };
    // PE `0x4434de`: XTER >= 0x30 water/XBIT arm (shared MapTool surface gate).
    pe_maptool_surface_xter_gate_4434de(city, x, y, destroyed);
    let waits_445b50 = pe_raise_terrain_land_inc_445b50(city, x, y)?;
    // Stretch/Level post-raise `0x40215d`→`0x447490` neighbor FUN_00447020.
    pe_terrain_neighbor_fun_00447020_447490(city, Some(crt), x, y);
    Some(
        post_debit
            .wrapping_add(waits_445b50)
            .wrapping_add(PE_447490_UI_WAIT_4012C1_PER_CALL),
    )
}

/// PE Raise submode-0 XBIT `&= 0xdf` @ `0x4433f0` when `[0x4e9e48]≠0` and XBLD `< 0x70`.
pub(in crate::app) fn pe_raise_submode0_clear_xbit_20_4433f0(
    city: &mut City,
    x: usize,
    y: usize,
    xbld: u8,
) {
    if xbld >= PE_RAISE_SUBMODE0_XBIT_CLEAR_XBLD_MAX {
        return;
    }
    let Some(bits) = city.tile_bits_at(x, y) else {
        return;
    };
    city.set_tile_bits(
        x,
        y,
        TileBits::new(bits.raw_bits() & PE_RAISE_SUBMODE0_XBIT_CLEAR_MASK),
    );
}

/// PE Raise submode-0 XUND clear gate @ `0x443430`: XBLD `0xe9` or `0x6c..=0x70`.
pub(in crate::app) fn pe_raise_submode0_clears_xund_443430(xbld: u8) -> bool {
    xbld == PE_RAISE_SUBMODE0_XUND_CLEAR_XBLD_E9
        || (PE_RAISE_SUBMODE0_XUND_CLEAR_LO..=PE_RAISE_SUBMODE0_XUND_CLEAR_HI).contains(&xbld)
}

/// PE Raise submode-0 mid-band CRT refuse @ `0x443470`: XBLD in `6..0x0c` and
/// `rand() % 0x14 == 0` aborts after funds debit (UI/sound omitted).
pub(in crate::app) fn pe_raise_submode0_midband_crt_refuse_443470(
    crt: &mut Win95CrtRand,
    xbld: u8,
) -> bool {
    if xbld < PE_RAISE_SUBMODE0_MIDBAND_XBLD_MIN || xbld >= PE_RAISE_SUBMODE0_DESTROY_XBLD_MIN {
        return false;
    }
    crt.random_mod(PE_RAISE_SUBMODE0_MIDBAND_CRT_MOD) == 0
}

/// Win95 Raise tool-0 submode dispatch (`0x442db0` / table `0x443104`).
/// Submode 0 EditMap uses billed `pe_raise_submode0_step_billed_443270` directly;
/// this helper keeps the unbilled land++/`0x445b50` arm for isolated tests.
/// Submode 2 is `0x443c60`; submode 3 is mode-1 dig (`0x444440`).
pub(in crate::app) fn pe_raise_tool0_submode_step(
    city: &mut City,
    x: usize,
    y: usize,
    submode: u16,
) -> bool {
    pe_raise_tool0_submode_step_with_crt(city, None, x, y, submode)
}

pub(in crate::app) fn pe_raise_tool0_submode_step_with_crt(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    submode: u16,
) -> bool {
    if submode > PE_RAISE_SUBMODE_MAX {
        return false;
    }
    match submode {
        PE_RAISE_SUBMODE_DEFAULT => {
            if pe_raise_submode0_early_refuse_443270(city, x, y, false) {
                return false;
            }
            pe_raise_terrain_land_inc_445b50(city, x, y).is_some()
        }
        2 => pe_raise_land_inc_443c60(city, x, y, submode).is_some(),
        // PE `0x442ee8`: push mode 1 then `0x402ce8`→`0x444440` (bills `0x4dc140`[submode]).
        3 => pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt(
            city,
            x,
            y,
            1,
            submode,
            crt.as_deref_mut(),
        ),
        _ => false,
    }
}

/// PE Raise submode-0 `0x443270` early refuses: zone low nibble `7` (military), then
/// DisplayLayer clear (`[0x4e9e48]==0`): XBLD `5` @ `0x4432fd` and vacant XTER `< 0x30`
/// @ `0x443310`–`0x443323`. DisplayLayer set jumps to underground empty refuse @
/// `0x443350` (skips surface XBLD==5 and vacant XTER gates).
pub(in crate::app) fn pe_raise_submode0_early_refuse_443270(
    city: &City,
    x: usize,
    y: usize,
    flag_4e9e48: bool,
) -> bool {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return true;
    }
    let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    if zone & 0x0f == ZONE_MILITARY {
        return true;
    }
    if flag_4e9e48 {
        // PE @ `0x4432fb`/`0x44332c`→`0x443350`: underground empty refuse only.
        return pe_maptool_underground_empty_refuse_443350(city, x, y);
    }
    let xbld = city.building_at(x, y);
    // PE @ `0x4432fd`: XBLD==5 refuse when DisplayLayer clear.
    if xbld == 5 {
        return true;
    }
    // PE: XBLD==0 && XTER < 0x30 → refuse; XBLD==0 && XTER >= 0x30 → funds.
    if xbld == 0 && city.terrain_at(x, y) < 0x30 {
        return true;
    }
    false
}

/// PE FUN_00443270 epilogue surface reshape bands @ `0x44c7a0`:
/// road `0x1d..0x2c`, rail `0x2c..0x3b`, power `0x0e..0x1d` (exclusive ends).
pub(in crate::app) fn pe_raise_epilogue_surface_reshape_kind_44c7a0(
    xbld: u8,
) -> Option<NetworkKind> {
    if (0x1d..0x2c).contains(&xbld) {
        Some(NetworkKind::Road)
    } else if (0x2c..0x3b).contains(&xbld) {
        Some(NetworkKind::Rail)
    } else if (0x0e..0x1d).contains(&xbld) {
        Some(NetworkKind::Power)
    } else {
        None
    }
}

/// PE FUN_00443270 epilogue underground reshape bands @ `0x44c9b0`:
/// subway `1..0x10`, pipes `0x10..0x1f` (exclusive ends).
pub(in crate::app) fn pe_raise_epilogue_underground_reshape_kind_44c9b0(
    xund: u8,
) -> Option<UndergroundKind> {
    if (1..0x10).contains(&xund) {
        Some(UndergroundKind::Subway)
    } else if (0x10..0x1f).contains(&xund) {
        Some(UndergroundKind::Pipes)
    } else {
        None
    }
}
