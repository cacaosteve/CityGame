use super::{
    PE_447490_UI_WAIT_4012C1_PER_CALL, ZONE_MILITARY, offset_city_tile,
    pe_lower_unpack_tile_444bf0, pe_raise_drag_step_dir_445da0,
    pe_terrain_neighbor_fun_00447020_447490,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City, TileBits};
use crate::win95_random::Win95CrtRand;

pub(in crate::app) const PE_VOLCANO_FLATTEN_COST: u32 = 0x19;

pub(in crate::app) const PE_VOLCANO_FLATTEN_MAX_HEIGHT: u8 = 0x1e;

/// Win95 `0x4e7858` / `0x4e7870` first 8 neighbor steps.
pub(in crate::app) const PE_VOLCANO_FLATTEN_DX: [isize; 8] = [0, 1, 1, 1, 0, -1, -1, -1];

pub(in crate::app) const PE_VOLCANO_FLATTEN_DY: [isize; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

/// Win95 flatten NESW recurse order: west, north, east, south.
pub(in crate::app) const PE_VOLCANO_FLATTEN_NESW_DX: [isize; 4] = [-1, 0, 1, 0];

pub(in crate::app) const PE_VOLCANO_FLATTEN_NESW_DY: [isize; 4] = [0, -1, 0, 1];

/// Win95 `0x4e7918` / `0x4e7920` first 4 steps used by `0x445b50`: north, east, south, west.
pub(in crate::app) const PE_POST_FLATTEN_NEIGHBOR_DX: [isize; 4] = [0, 1, 0, -1];

pub(in crate::app) const PE_POST_FLATTEN_NEIGHBOR_DY: [isize; 4] = [-1, 0, 1, 0];

/// Win95 `0x445b50`: skip neighbor ALTM smooth when XBLD `>= 0x0d`.
pub(in crate::app) const PE_POST_FLATTEN_NEIGHBOR_XBLD_GATE: u8 = 0x0d;

pub(in crate::app) fn pe_set_flatten_mark(city: &mut City, x: usize, y: usize, mark: bool) {
    let bits = city.tile_bits_at(x, y).unwrap_or(TileBits::new(0));
    city.set_tile_bits(x, y, bits.with_flatten_mark(mark));
}

/// Win95 `0x40237e` / `0x443cb0`: military, water, or max height cannot flatten.
pub(in crate::app) fn pe_volcano_flatten_check(city: &mut City, x: usize, y: usize) -> bool {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return true;
    }
    if city
        .zone_at(x, y)
        .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
    {
        return false;
    }
    if city.tile_bits_at(x, y).is_some_and(TileBits::water_tile) {
        return false;
    }
    if city.tile_bits_at(x, y).is_some_and(TileBits::flatten_mark) {
        return true;
    }
    pe_set_flatten_mark(city, x, y, true);
    if city.altitude_at(x, y) >= PE_VOLCANO_FLATTEN_MAX_HEIGHT {
        pe_set_flatten_mark(city, x, y, false);
        return false;
    }
    for (&dx, &dy) in PE_VOLCANO_FLATTEN_DX
        .iter()
        .zip(PE_VOLCANO_FLATTEN_DY.iter())
    {
        let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
            continue;
        };
        if city
            .zone_at(nx, ny)
            .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
            || city.tile_bits_at(nx, ny).is_some_and(TileBits::water_tile)
        {
            pe_set_flatten_mark(city, x, y, false);
            return false;
        }
    }
    let land = city.altitude_at(x, y);
    for (&dx, &dy) in PE_VOLCANO_FLATTEN_NESW_DX
        .iter()
        .zip(PE_VOLCANO_FLATTEN_NESW_DY.iter())
    {
        let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
            continue;
        };
        if city.altitude_at(nx, ny) >= land {
            continue;
        }
        if !pe_volcano_flatten_check(city, nx, ny) {
            pe_set_flatten_mark(city, x, y, false);
            return false;
        }
    }
    true
}

/// PE terrain tool-4 `0x442a30` entry writes `[0x4cad4c] = 0xfffffc18` (`-1000`).
pub(in crate::app) const PE_SPREAD_TOOL4_UI_TIMER_INIT: i32 = -1000;

/// PE terrain tool-4 @ `0x442b31`: `0x445da0` same-tile `0xffff` skips NESW land copy.
/// Returns `(447490_count, 445b50_wait_count)`; `(0, 0)` if same-tile skip.
pub(in crate::app) fn pe_terrain_nesw_land_copy_442a30_if_moved(
    city: &mut City,
    x: usize,
    y: usize,
    prev_x: i16,
    prev_y: i16,
) -> (u32, u32) {
    pe_terrain_nesw_land_copy_442a30_if_moved_with_crt(city, None, x, y, prev_x, prev_y)
}

pub(in crate::app) fn pe_terrain_nesw_land_copy_442a30_if_moved_with_crt(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    prev_x: i16,
    prev_y: i16,
) -> (u32, u32) {
    if pe_raise_drag_step_dir_445da0(x as i16, y as i16, prev_x, prev_y) == 0xffff {
        return (0, 0);
    }
    let counts = pe_terrain_nesw_land_copy_442a30_with_crt(city, crt, x, y);
    // PE @ `0x442bf2`: clear previous-tile XTXT; @ `0x442c33`: stamp current `0xc9`.
    if (0..0x80).contains(&(prev_x as usize)) && (0..0x80).contains(&(prev_y as usize)) {
        city.set_text_overlay(prev_x as usize, prev_y as usize, 0);
    }
    city.set_text_overlay(x, y, 0xc9);
    counts
}

/// PE terrain tool-4 `0x442a30` @ `0x442b07`: unpack `0x444bf0` (`& 0x7f`) then same-tile gate.
/// Returns `(447490_count, 445b50_wait_count)`; `(0, 0)` if unpack/same-tile skip.
pub(in crate::app) fn pe_spread_terrain_from_packed_442a30(
    city: &mut City,
    packed: i16,
    prev_x: i16,
    prev_y: i16,
) -> (u32, u32) {
    pe_spread_terrain_from_packed_442a30_with_crt(city, None, packed, prev_x, prev_y)
}

pub(in crate::app) fn pe_spread_terrain_from_packed_442a30_with_crt(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    packed: i16,
    prev_x: i16,
    prev_y: i16,
) -> (u32, u32) {
    let Some((x, y)) = pe_lower_unpack_tile_444bf0(packed) else {
        return (0, 0);
    };
    pe_terrain_nesw_land_copy_442a30_if_moved_with_crt(city, crt, x, y, prev_x, prev_y)
}

/// Win95 terrain tool-4 `0x402b94`→`0x442a30`: copy center ALTM land onto in-bounds NESW
/// neighbors (`0x4e7888` / `0x4e7890`), then `0x401a78`→`0x445b50` and `0x40215d`→`0x447490`
/// each. Returns `(447490_count, 445b50_adjust_4012c1_waits)`.
pub(in crate::app) fn pe_terrain_nesw_land_copy_442a30(
    city: &mut City,
    x: usize,
    y: usize,
) -> (u32, u32) {
    pe_terrain_nesw_land_copy_442a30_with_crt(city, None, x, y)
}

pub(in crate::app) fn pe_terrain_nesw_land_copy_442a30_with_crt(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) -> (u32, u32) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return (0, 0);
    }
    let center_land = city.altitude_at(x, y);
    let mut neighbor_447490 = 0u32;
    let mut waits_445b50 = 0u32;
    for (&dx, &dy) in PE_POST_FLATTEN_NEIGHBOR_DX
        .iter()
        .zip(PE_POST_FLATTEN_NEIGHBOR_DY.iter())
    {
        let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
            continue;
        };
        city.set_altitude(nx, ny, center_land, city.water_height_at(nx, ny));
        waits_445b50 =
            waits_445b50.wrapping_add(pe_post_flatten_neighbor_altm_445b50(city, nx, ny));
        pe_terrain_neighbor_fun_00447020_447490(city, crt.as_deref_mut(), nx, ny);
        neighbor_447490 = neighbor_447490.wrapping_add(1);
    }
    (neighbor_447490, waits_445b50)
}

/// Win95 `0x401a78` / `0x445b50`: after raise, pull NESW neighbors within 1 ALTM of center.
/// Skips XBLD `>= 0x0d`. Returns PE `0x4012c1` wait count (one per adjusted neighbor @
/// `0x445c15`/`0x445c5a`, including recursive pulls).
pub(in crate::app) fn pe_post_flatten_neighbor_altm_445b50(
    city: &mut City,
    x: usize,
    y: usize,
) -> u32 {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return 0;
    }
    let center = city.altitude_at(x, y);
    let mut waits = 0u32;
    for (&dx, &dy) in PE_POST_FLATTEN_NEIGHBOR_DX
        .iter()
        .zip(PE_POST_FLATTEN_NEIGHBOR_DY.iter())
    {
        let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
            continue;
        };
        if city.building_at(nx, ny) >= PE_POST_FLATTEN_NEIGHBOR_XBLD_GATE {
            continue;
        }
        let neighbor = city.altitude_at(nx, ny);
        let diff = i16::from(center) - i16::from(neighbor);
        if (-1..=1).contains(&diff) {
            continue;
        }
        let new_land = if diff > 1 {
            center.saturating_sub(1)
        } else {
            center.saturating_add(1).min(0x1f)
        };
        // PE @ `0x445c15`/`0x445c5a`: `0x4012c1` before writing the adjusted neighbor.
        waits = waits.wrapping_add(1);
        city.set_altitude(nx, ny, new_land, city.water_height_at(nx, ny));
        waits = waits.wrapping_add(pe_post_flatten_neighbor_altm_445b50(city, nx, ny));
    }
    waits
}

/// Win95 `0x402f3b` / `0x4441c0`: clear flatten mark, raise land, spend `0x19` dummy funds.
/// Returns PE `0x4012c1` wait count from `0x445b50` adjusts + `0x447490` (9×2) per successful raise,
/// including NESW recurse.
pub(in crate::app) fn pe_volcano_apply_flatten(
    city: &mut City,
    x: usize,
    y: usize,
    funds: &mut u32,
) -> u32 {
    pe_volcano_apply_flatten_with_crt(city, None, x, y, funds)
}

pub(in crate::app) fn pe_volcano_apply_flatten_with_crt(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    funds: &mut u32,
) -> u32 {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return 0;
    }
    if !city.tile_bits_at(x, y).is_some_and(TileBits::flatten_mark) {
        return 0;
    }
    pe_set_flatten_mark(city, x, y, false);
    let land = city.altitude_at(x, y);
    if land >= PE_VOLCANO_FLATTEN_MAX_HEIGHT {
        return 0;
    }
    let mut waits = 0u32;
    for (&dx, &dy) in PE_VOLCANO_FLATTEN_NESW_DX
        .iter()
        .zip(PE_VOLCANO_FLATTEN_NESW_DY.iter())
    {
        let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) else {
            continue;
        };
        if city.altitude_at(nx, ny) < land {
            waits = waits.wrapping_add(pe_volcano_apply_flatten_with_crt(
                city,
                crt.as_deref_mut(),
                nx,
                ny,
                funds,
            ));
        }
    }
    if *funds < PE_VOLCANO_FLATTEN_COST {
        return waits;
    }
    *funds -= PE_VOLCANO_FLATTEN_COST;
    city.set_altitude(x, y, land.saturating_add(1), city.water_height_at(x, y));
    if let Some(zone) = city.zone_at(x, y) {
        city.set_zone(x, y, zone.raw_bits() & 0xf0);
    }
    // PE @ `0x4443a4`: `0x401a78`→`0x445b50` after raise + XZON clear; `0x40215d`→`0x447490`.
    waits = waits.wrapping_add(pe_post_flatten_neighbor_altm_445b50(city, x, y));
    pe_terrain_neighbor_fun_00447020_447490(city, crt, x, y);
    waits.wrapping_add(PE_447490_UI_WAIT_4012C1_PER_CALL)
}
