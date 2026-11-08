//! Bridge demolition, support-tip cleanup, and span geometry.

use super::*;

/// OpenSC2K `DemolishBridges` bank tip @ `_demolish_bridge` bank loop:
/// PlaceTile EMPTY, ALTM land--, OR water-tile, clear rotate, clear XUND.
/// Skips tiles that already have water-tile. Does not call `FUN_00447020`
/// (its slope `0x32` land++ undoes the OpenSC2K land-- when neighbors are taller).
pub(in crate::app) fn opensc2k_demolish_bridge_retile_bank(city: &mut City, x: usize, y: usize) {
    if city.tile_bits_at(x, y).is_some_and(TileBits::water_tile) {
        return;
    }
    pe_place_tile_441f00(city, x, y, 0);
    let land = city.altitude_at(x, y) & 0x1f;
    if land > 0 {
        city.set_altitude(x, y, land - 1, city.water_height_at(x, y));
    }
    if city.underground_at(x, y) != 0 {
        pe_place_xund_442370(city, x, y, 0);
    }
    if let Some(bits) = city.tile_bits_at(x, y) {
        city.set_tile_bits(x, y, bits.with_water_tile(true).with_rotate(false));
    }
}

/// OpenSC2K `DemolishBridges._demolish_bridge` bank loop: retile both dry land
/// tips just beyond the cleared span (`first - dir`, `finish + dir`).
pub(in crate::app) fn opensc2k_demolish_bridge_retile_banks(
    city: &mut City,
    span: &[(usize, usize)],
) -> Vec<(usize, usize)> {
    if span.len() < 2 {
        return Vec::new();
    }
    let first = span[0];
    let finish = *span.last().expect("span len >= 2");
    let dx = (finish.0 as isize - first.0 as isize).signum();
    let dy = (finish.1 as isize - first.1 as isize).signum();
    if dx == 0 && dy == 0 {
        return Vec::new();
    }
    let mut banks = Vec::new();
    for bank in [
        offset_city_tile(first.0, first.1, -dx, -dy),
        offset_city_tile(finish.0, finish.1, dx, dy),
    ]
    .into_iter()
    .flatten()
    {
        opensc2k_demolish_bridge_retile_bank(city, bank.0, bank.1);
        banks.push(bank);
    }
    banks
}

/// PE DestroyBuilding tip endpoint: pre-walk `0x458fae..0x45907f`, post-walk
/// `0x459655..0x45975b`. Both PlaceTile-clear, decrement ALTM land, set XBIT
/// water, and call `FUN_00447020`; only the pre-walk endpoint clears rotate.
/// Both skip tiles that already have water-tile; the helper may then clear water
/// when sea ≤ land.
pub(in crate::app) fn pe_destroy_bridge_support_tip_epilogue_458fae(
    city: &mut City,
    x: usize,
    y: usize,
    clear_rotate: bool,
) {
    // PE @ `0x458f84`: existing water-tile skips PlaceTile / land-- / epilogue.
    if city.tile_bits_at(x, y).is_some_and(TileBits::water_tile) {
        return;
    }
    // PE @ `0x458fae`: PlaceTile(0) at tip (skip `0x4012c1`→`0x43a520` UI).
    pe_place_tile_441f00(city, x, y, 0);
    // PE @ `0x458fcb..0x459000`: `cx = (ALTM & 0x1f) - 1` in 16-bit
    // arithmetic, then `(ALTM & 0xffe0) | cx`. At land zero this wraps CX to
    // `0xffff` and therefore sets all 16 ALTM bits; preserve that native edge case.
    let sample = city.altm_at(x, y);
    let altm_word = u16::from(sample.land & 0x1f)
        | (u16::from(sample.water & 0x1f) << 5)
        | (u16::from(sample.tunnel_levels & 0x3f) << 10);
    let decremented_land = (altm_word & 0x1f).wrapping_sub(1);
    let altm_word = (altm_word & 0xffe0) | decremented_land;
    city.set_altm_word(x, y, altm_word);
    // PE @ `0x459036`: OR XBIT water-tile after land--.
    if let Some(bits) = city.tile_bits_at(x, y) {
        city.set_tile_bits(x, y, bits.with_water_tile(true));
    }
    // PE @ `0x459044`: `0x402bd0` → `FUN_00447020`.
    let sea = pe_selected_water_height_for_fun_00447020(city);
    pe_fun_00447020_mode1_destroy_and_city_clear_447085(city, None, x, y);
    pe_fun_00447020_mode0_xund_slope_sea(city, x, y, sea);
    // The pre-walk endpoint clears rotate at `0x45907f`; the post-walk endpoint
    // at `0x459655..0x45975B` does not.
    if clear_rotate {
        if let Some(bits) = city.tile_bits_at(x, y) {
            city.set_tile_bits(x, y, bits.with_rotate(false));
        }
    }
}

/// PE DestroyBuilding size-1 support-walk tip epilogue @ `0x458e0f`–`0x459083`.
/// While live XBLD is in `0x51..=0x5c` / `0x6a` / `0x6b`, step `−size` on x
/// when hit XBIT rotate `0x02` is set, and on y otherwise. Tip (if not already water):
/// PlaceTile(0), ALTM land--, OR water-tile, `FUN_00447020` mode0, clear rotate.
/// PE's `[esp+0x1c]`/`[esp+0x1e]` ALTM index is the walked tip under PlaceTile's
/// pending args (`+0xc` → `[esp+0x10]`/`[0x12]`).
pub(in crate::app) fn pe_destroy_building_size1_support_land_dec_458f90(
    city: &mut City,
    hit_x: usize,
    hit_y: usize,
    old_building: u8,
) {
    let size = pe_destroy_building_size(old_building);
    if size != 1 || !pe_destroy_building_support_xbld_band(old_building) {
        return;
    }
    // PE @ `0x458eb4`: XBIT rotate on the *hit* selects the x axis; clear selects y.
    let walk_x = city
        .tile_bits_at(hit_x, hit_y)
        .is_some_and(TileBits::rotate);
    let mut x = hit_x as isize;
    let mut y = hit_y as isize;
    let step = size as isize;
    loop {
        if x < 0 || y < 0 || x >= CITY_WIDTH as isize || y >= CITY_HEIGHT as isize {
            break;
        }
        let xbld = city.building_at(x as usize, y as usize);
        if !pe_destroy_building_support_xbld_band(xbld) {
            break;
        }
        if walk_x {
            x -= step;
        } else {
            y -= step;
        }
    }
    if x < 0 || y < 0 || x >= CITY_WIDTH as isize || y >= CITY_HEIGHT as isize {
        return;
    }
    pe_destroy_bridge_support_tip_epilogue_458fae(city, x as usize, y as usize, true);
}

/// Win95 selector-1 support-band walk @ `0x458E0F..0x45975B`.
/// XBIT rotate on the original hit selects the axis: set walks X, clear walks Y.
/// First find the backward endpoint, process it with the pre-walk epilogue, then
/// scan forward through live support cells. Each admitted body cell consumes two
/// direct mode-1 CRT draws before PlaceTile(0), XZON `&= 0x0F`, XBIT `&= 0xFD`.
/// The first unsupported in-bounds cell after the run gets the post-walk endpoint
/// epilogue, which deliberately leaves rotate unchanged.
pub(in crate::app) fn pe_destroy_building_selector1_support_walk_458e0f(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    hit_x: usize,
    hit_y: usize,
    mode1: bool,
) {
    let walk_x = city
        .tile_bits_at(hit_x, hit_y)
        .is_some_and(TileBits::rotate);
    let mut x = hit_x as isize;
    let mut y = hit_y as isize;
    let step = if walk_x { (1isize, 0isize) } else { (0, 1) };

    // PE scans backward while the live XBLD belongs to the support band.
    while (0..CITY_WIDTH as isize).contains(&x)
        && (0..CITY_HEIGHT as isize).contains(&y)
        && pe_destroy_building_support_xbld_band(city.building_at(x as usize, y as usize))
    {
        x -= step.0;
        y -= step.1;
    }

    // Pre-walk endpoint: the PE path checks the map bounds, then XBIT water bit.
    if (0..CITY_WIDTH as isize).contains(&x) && (0..CITY_HEIGHT as isize).contains(&y) {
        pe_destroy_bridge_support_tip_epilogue_458fae(city, x as usize, y as usize, true);
    }

    // The native code advances once from the backward endpoint. This returns to
    // coordinate zero correctly when the backward scan stepped just out of map.
    x += step.0;
    y += step.1;
    while (0..CITY_WIDTH as isize).contains(&x)
        && (0..CITY_HEIGHT as isize).contains(&y)
        && pe_destroy_building_support_xbld_band(city.building_at(x as usize, y as usize))
    {
        if mode1 {
            if let Some(rng) = crt.as_deref_mut() {
                let _ = rng.random_u15();
                let _ = rng.random_u15();
            }
        }
        let (tx, ty) = (x as usize, y as usize);
        pe_place_tile_441f00(city, tx, ty, 0);
        let zone = city.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0);
        city.set_zone(tx, ty, zone & 0x0f);
        if let Some(bits) = city.tile_bits_at(tx, ty) {
            city.set_tile_bits(tx, ty, TileBits::new(bits.raw_bits() & 0xfd));
        }
        x += step.0;
        y += step.1;
    }

    // Post-walk endpoint has the same PlaceTile/ALTM/XBIT/FUN sequence but no
    // rotate-clear instruction. Out-of-bounds coordinates skip the endpoint.
    if (0..CITY_WIDTH as isize).contains(&x) && (0..CITY_HEIGHT as isize).contains(&y) {
        pe_destroy_bridge_support_tip_epilogue_458fae(city, x as usize, y as usize, false);
    }
}

/// Win95 `0x401dca` / `FUN_0044ced0`: free XLAB / XMIC / XTHG for an XTXT id.
/// OpenSC2K `DemolishBridges._demolish_bridge`: contiguous bridge run along XBIT flip axis.
/// Flip set → span along grid-x; clear → grid-y. Falls back to the longer axis run when the
/// saved flip disagrees with connectivity (player packaging).
pub(in crate::app) fn opensc2k_bridge_span_tiles(
    city: &City,
    x: usize,
    y: usize,
) -> Vec<(usize, usize)> {
    if !is_bridge_tile(city.building_at(x, y)) {
        return vec![(x, y)];
    }
    let flipped = city
        .tile_bits_at(x, y)
        .is_some_and(crate::city::TileBits::rotate);
    let primary = if flipped {
        bridge_span_along_axis(city, x, y, 1, 0)
    } else {
        bridge_span_along_axis(city, x, y, 0, 1)
    };
    if primary.len() > 1 {
        return primary;
    }
    let alternate = if flipped {
        bridge_span_along_axis(city, x, y, 0, 1)
    } else {
        bridge_span_along_axis(city, x, y, 1, 0)
    };
    if alternate.len() > primary.len() {
        alternate
    } else {
        primary
    }
}

pub(in crate::app) fn bridge_span_along_axis(
    city: &City,
    x: usize,
    y: usize,
    dx: isize,
    dy: isize,
) -> Vec<(usize, usize)> {
    let mut first = (x, y);
    loop {
        let Some(prev) = offset_city_tile(first.0, first.1, -dx, -dy) else {
            break;
        };
        if !is_bridge_tile(city.building_at(prev.0, prev.1)) {
            break;
        }
        first = prev;
    }
    let mut finish = (x, y);
    loop {
        let Some(next) = offset_city_tile(finish.0, finish.1, dx, dy) else {
            break;
        };
        if !is_bridge_tile(city.building_at(next.0, next.1)) {
            break;
        }
        finish = next;
    }
    let mut tiles = Vec::new();
    let mut cur = first;
    loop {
        tiles.push(cur);
        if cur == finish {
            break;
        }
        cur = offset_city_tile(cur.0, cur.1, dx, dy).expect("bridge span stays in bounds");
    }
    tiles
}
