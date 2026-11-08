use super::*;

/// Win95 `0x401b90`→`0x444ff0`: Stretch hill may raise when land `<= 0x1e` and every
/// in-bounds NESW neighbor has land `>=` center (`0x4e78c8` / `0x4e78d0`).
pub(in crate::app) fn pe_can_raise_stretch_hill_444ff0(city: &City, x: usize, y: usize) -> bool {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return false;
    }
    let center = city.altitude_at(x, y);
    if center > 0x1e {
        return false;
    }
    for (&dx, &dy) in PE_POST_FLATTEN_NEIGHBOR_DX
        .iter()
        .zip(PE_POST_FLATTEN_NEIGHBOR_DY.iter())
    {
        let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
            continue;
        };
        if city.altitude_at(nx, ny) < center {
            return false;
        }
    }
    true
}

/// Win95 StretchTerrain `0x443a50` CRT jitter: four `rand()&3` calls → `(dx,dy)` in `-3..=3`.
pub(in crate::app) fn pe_stretch_hill_crt_jitter(rng: &mut Win95CrtRand) -> (isize, isize) {
    let x0 = (rng.random_u15() & 3) as i16;
    let x1 = (rng.random_u15() & 3) as i16;
    let y0 = (rng.random_u15() & 3) as i16;
    let y1 = (rng.random_u15() & 3) as i16;
    (isize::from(x0 - x1), isize::from(y0 - y1))
}

/// Win95 StretchTerrain tool-1 body step (`0x401ab4`→`0x443a50`): optional CRT jitter,
/// gated land++ on the jittered tile, then `0x401a78`→`0x445b50` on the click origin.
pub(in crate::app) fn pe_stretch_terrain_hill_step_443a50(
    city: &mut City,
    x: usize,
    y: usize,
) -> bool {
    pe_stretch_terrain_hill_step_443a50_with_crt(city, None, x, y).is_some()
}

/// On success returns PE `0x445b50` adjust-path `0x4012c1` wait count.
pub(in crate::app) fn pe_stretch_terrain_hill_step_443a50_with_crt(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) -> Option<u32> {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return None;
    }
    let (tx, ty) = if let Some(rng) = crt.as_mut() {
        let (dx, dy) = pe_stretch_hill_crt_jitter(rng);
        let Some(tile) = offset_city_tile(x, y, dx, dy) else {
            return None;
        };
        tile
    } else {
        (x, y)
    };
    if !pe_can_raise_stretch_hill_444ff0(city, tx, ty) {
        return None;
    }
    let sample = city.altm_at(tx, ty);
    let land = sample.land.saturating_add(1).min(0x1f);
    city.set_altitude(tx, ty, land, sample.water);
    // PE @ `0x443b94`: `0x445b50` uses tool args (click origin), not the jittered tile.
    let waits_445b50 = pe_post_flatten_neighbor_altm_445b50(city, x, y);
    // PE @ `0x443b9e`: `0x40215d`→`0x447490` neighbor FUN_00447020.
    pe_terrain_neighbor_fun_00447020_447490(city, crt, tx, ty);
    Some(waits_445b50)
}

/// PE LevelTerrain hold @ `0x444951`–`0x4449bb`: mouse-Y delta → land delta by view rotation.
/// Rot 0/1/2 divide by 3/6/12; rot 3 uses raw delta. Clamp `1..=0x1e`, floor `base-10`,
/// skip when unchanged.
pub(in crate::app) fn pe_level_terrain_mouse_land_444951(
    base_land: u8,
    current_land: u8,
    press_mouse_y: i16,
    mouse_y: i16,
    view_rotation: u8,
) -> Option<u8> {
    let delta = mouse_y.wrapping_sub(press_mouse_y);
    let land_delta = match view_rotation & 3 {
        0 => delta / 3,
        1 => delta / 6,
        2 => delta / 0xc,
        _ => delta,
    };
    let new = i32::from(base_land) + i32::from(land_delta);
    if !(1..=0x1e).contains(&new) {
        return None;
    }
    if i32::from(base_land) - 10 > new {
        return None;
    }
    let new_u8 = new as u8;
    if new_u8 == current_land {
        return None;
    }
    Some(new_u8)
}

/// PE Stretch hold @ `0x443ba6`: `test si, 7` — UI `0x40226b` when `(si & 7) == 0`.
pub(in crate::app) fn pe_stretch_hold_ui_refresh_due_443ba6(si: u16) -> bool {
    si & 7 == 0
}

/// PE Stretch success-path `si` advances: loop-start `inc` (`0x443a91`) + post-apply `inc`
/// (`0x443b88`). Returns whether `0x443ba6` should fire UI refresh.
pub(in crate::app) fn pe_stretch_hold_after_successful_hill_443b88(si: &mut u16) -> bool {
    *si = si.wrapping_add(2);
    pe_stretch_hold_ui_refresh_due_443ba6(*si)
}

/// PE Stretch `0x443a50` @ `0x443aae`/`0x443ab1`: unpack `0x444bf0` (`& 0x7f`) before CRT jitter.
/// Returns PE `0x445b50` adjust-path `0x4012c1` wait count on success.
pub(in crate::app) fn pe_stretch_terrain_from_packed_443a50(
    city: &mut City,
    crt: &mut Win95CrtRand,
    packed: i16,
) -> Option<u32> {
    let (x, y) = pe_lower_unpack_tile_444bf0(packed)?;
    pe_stretch_terrain_hill_step_443a50_with_crt(city, Some(crt), x, y)
}
