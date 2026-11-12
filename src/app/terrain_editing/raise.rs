use super::flatten::{pe_apply_marked_terrain_raise, pe_mark_terrain_raise};
use super::*;

pub(in crate::app) const PE_RAISE_SUBMODE_DEFAULT: u16 = 0;

#[cfg(test)]
const PE_RAISE_SUBMODE_ADJUST_TOWARD: u16 = 1;

/// PE Raise tool-0 entry @ `0x442e10`: MapTool UI tool-id byte `[0x4ca434]+0` = 4.
pub(in crate::app) const PE_RAISE_TOOL0_UI_TOOL_ID: u8 = 4;

/// PE Raise tool-0 after step @ `0x443003`/`0x44300c`: UI tile X at `+3`, Y at `+4`.
pub(in crate::app) fn pe_raise_tool0_ui_tile_xy_443003(x: usize, y: usize) -> (u8, u8) {
    (x as u8, y as u8)
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
/// Land `<` target → raise via `0x4024be`→`0x443c60` (including the marked raise and recursive smoothing).
/// Land `>` target → dig `0x402ce8`→`0x444440` (mode 1, including queued neighbor-height
/// balancing and the padded dirty-rectangle `0x447490` reclassifier).
/// Equal → success. Bounds / funds / UI omitted.
#[cfg(test)]
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
#[cfg(test)]
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
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    submode: u16,
) -> Option<u32> {
    if x >= CITY_WIDTH
        || y >= CITY_HEIGHT
        || !pe_mark_terrain_raise(city, x, y, submode != PE_RAISE_SUBMODE_DEFAULT)
    {
        return None;
    }

    let opening_funds = city.city_funds().max(0) as u32;
    let mut funds = opening_funds;
    let waits = pe_apply_marked_terrain_raise(city, crt, x, y, &mut funds);
    if funds != opening_funds {
        city.debit_city_funds(opening_funds - funds);
    }
    // 0x443c94 reports preflight success even if the worker ran out of cash.
    Some(waits)
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
