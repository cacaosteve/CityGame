//! Win95 newspaper story gifts and their city-map side effects.
//!
//! Keep story dispatch, placement geometry and CRT draw order together. Newspaper
//! delivery and page presentation remain in their existing modules.

use super::*;

/// PE newspaper stadium gift @ `0x483767`: PlaceTile-size(`0xd7`, 4) then sports-reward
/// ladder/microsim write (same join as `0x4837a5`/`0x483816`, CRT fallback `0x4837bd`).
/// `x,y` are the PlaceTile-size click after `lea eax,[esi+1]` / `sub cx,2`; post-dec NW
/// is `(x-1,y-1)`. PE reads XTXT at the click `(x,y)` via `[esi*4+0x4ca604][edi-2]`.
pub(super) fn pe_newspaper_gift_place_stadium_483767(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
) -> bool {
    let Some((nw_x, nw_y)) = pe_place_tile_size_click_to_nw(x, y, STADIUM_BUILDING_SIZE) else {
        return false;
    };
    if !pe_place_tile_size_440c50(city, nw_x, nw_y, STADIUM_BUILDING_ID, STADIUM_BUILDING_SIZE) {
        return false;
    }
    let Some(xtxt) = city.text_overlay_at(x, y) else {
        return false;
    };
    city.pe_newspaper_stadium_sports_reward_4837a5(xtxt, crt)
}

/// PE newspaper gift PlaceTile-size coal @ `0x4836d7`: (`0xcf`, 4) after caller `dec` x/y;
/// PlaceTile-size size 4 then decs again to NW.
pub(super) fn pe_newspaper_gift_place_coal_4836d7(city: &mut City, x: usize, y: usize) -> bool {
    let Some((nw_x, nw_y)) = pe_place_tile_size_click_to_nw(x, y, COAL_POWER_BUILDING_SIZE) else {
        return false;
    };
    pe_place_tile_size_440c50(
        city,
        nw_x,
        nw_y,
        COAL_POWER_BUILDING_ID,
        COAL_POWER_BUILDING_SIZE,
    )
}

/// PE newspaper gift PlaceTile-size nuclear @ `0x483aab`: (`0xcb`, 4) at click; post-dec NW.
pub(super) fn pe_newspaper_gift_place_nuclear_483aab(city: &mut City, x: usize, y: usize) -> bool {
    let Some((nw_x, nw_y)) = pe_place_tile_size_click_to_nw(x, y, 4) else {
        return false;
    };
    pe_place_tile_size_440c50(city, nw_x, nw_y, NUCLEAR_POWER_BUILDING_ID, 4)
}

/// PE newspaper gift PlaceTile-size fire @ `0x483706`: (`0xd3`, 3) at click; post-dec NW.
pub(super) fn pe_newspaper_gift_place_fire_483706(city: &mut City, x: usize, y: usize) -> bool {
    let Some((nw_x, nw_y)) = pe_place_tile_size_click_to_nw(x, y, FIRE_STATION_BUILDING_SIZE)
    else {
        return false;
    };
    pe_place_tile_size_440c50(
        city,
        nw_x,
        nw_y,
        FIRE_STATION_BUILDING_ID,
        FIRE_STATION_BUILDING_SIZE,
    )
}

/// PE newspaper gift PlaceTile-size police @ `0x483731`: (`0xd2`, 3) at click; post-dec NW.
pub(super) fn pe_newspaper_gift_place_police_483731(city: &mut City, x: usize, y: usize) -> bool {
    let Some((nw_x, nw_y)) = pe_place_tile_size_click_to_nw(x, y, POLICE_STATION_BUILDING_SIZE)
    else {
        return false;
    };
    pe_place_tile_size_440c50(
        city,
        nw_x,
        nw_y,
        POLICE_STATION_BUILDING_ID,
        POLICE_STATION_BUILDING_SIZE,
    )
}

/// PE newspaper gift shore prep @ `0x484320`: OR XBIT water-tile (`0x04`), force ALTM
/// land nibble to `1` (`&=0xffe1` / `|=1`), then PlaceTile XBLD=`0x10`.
pub(super) fn pe_newspaper_gift_shore_prep_484320(city: &mut City, x: usize, y: usize) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    let bits = city
        .tile_bits_at(x, y)
        .unwrap_or_else(|| TileBits::new(0))
        .with_water_tile(true);
    city.set_tile_bits(x, y, bits);
    let sample = city.altm_at(x, y);
    city.set_altitude(x, y, 1, sample.water);
    pe_place_tile_441f00(city, x, y, 0x10);
}

/// PE newspaper gift ALTM prep @ `0x483287`: `ALTM &= 0xfc5f` then `|= 0x40` — keeps
/// land/tunnel, forces water nibble to `2`.
pub(super) fn pe_newspaper_gift_altm_prep_483287(city: &mut City, x: usize, y: usize) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    let sample = city.altm_at(x, y);
    city.set_altitude(x, y, sample.land, 2);
}

/// PE newspaper gift shore-only @ `0x4832c1` (story ids `2..=0x14`): shore-prep then ret.
pub(super) fn pe_newspaper_gift_shore_only_4832c1(city: &mut City, x: usize, y: usize) {
    pe_newspaper_gift_shore_prep_484320(city, x, y);
}

/// PE newspaper gift entry @ `0x483250`: in-bounds ALTM prep, refuse `story < 2`, then
/// early/table/zone-power/PlaceTile-size arms, zone-type→paint fallthrough, and bridges.
pub(super) fn pe_newspaper_gift_entry_483250(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: isize,
    y: isize,
    story_id: u16,
) {
    // PE @ `0x483268`..`0x4832a5`: ALTM prep when x/y in `0..=0x7f`.
    if (0..0x80).contains(&x) && (0..0x80).contains(&y) {
        pe_newspaper_gift_altm_prep_483287(city, x as usize, y as usize);
    }
    // PE @ `0x4832b1`: `cmp bx, 2` / `jl` exit.
    if story_id < 2 {
        return;
    }
    let (ux, uy) = (x.max(0) as usize, y.max(0) as usize);
    match story_id {
        0x02..=0x14 => pe_newspaper_gift_shore_only_4832c1(city, ux, uy),
        0x15..=0x24 => pe_newspaper_gift_place_trees_4832d9(city, crt, ux, uy),
        0x25..=0x2b => pe_newspaper_gift_place_trees_4832ff(city, crt, ux, uy),
        0x30..=0x33 => pe_newspaper_gift_start_disaster_48332b(city, ux, uy),
        0x34 => pe_newspaper_gift_place_tree_483e68(city, crt, ux, uy),
        0x38..=0x3f => pe_newspaper_gift_start_disaster_48336d(city, ux, uy),
        // PE @ `0x48331f`/`0x483357`/`0x483396`: `0x2c..=0x2f` and `0x35..=0x37` → rubble.
        0x2c..=0x2f | 0x35..=0x37 => pe_newspaper_gift_place_rubble_483e37(city, crt, ux, uy),
        // PE @ `0x4833a0`: `0x40..=0xcf` → table `0x4ea9d8` index `(story-0x40)&0xf`.
        0x40..=0xcf => {
            let index = ((story_id - 0x40) & 0xf) as u8;
            pe_newspaper_gift_place_table_tile_483412(city, ux, uy, index);
        }
        // PE @ `0x48347c`: `0xd0..=0xdf` → table `0x4ea9e8`.
        0xd0..=0xdf => {
            let index = ((story_id - 0xd0) & 0xf) as u8;
            pe_newspaper_gift_place_table_tile_4834bc(city, ux, uy, index);
        }
        // PE @ `0x48354d`: `0xe0..=0xef` → table `0x4ea9f8`.
        0xe0..=0xef => {
            let index = ((story_id - 0xe0) & 0xf) as u8;
            pe_newspaper_gift_place_table_tile_48358a(city, ux, uy, index);
        }
        // PE @ `0x48362a` via `0x4835f4`/`0x483623`: `0x2b5..=0x2c4`.
        0x2b5..=0x2c4 => pe_newspaper_gift_zone_power_48362a(city, ux, uy),
        // PE @ `0x4836a8` → `0x483aca`: `0x2c5..=0x2e8` and `0x340..=0x347`.
        0x2c5..=0x2e8 | 0x340..=0x347 => pe_newspaper_gift_zone_power_483aca(city, ux, uy),
        // PE @ `0x4836d7`: story `0x2f3` PlaceTile-size coal after `dec` x/y.
        0x2f3 if ux > 0 && uy > 0 => {
            let _ = pe_newspaper_gift_place_coal_4836d7(city, ux - 1, uy - 1);
        }
        // PE @ `0x483706`: stories `0x2fd..=0x301` fire (`cmp 0x302` / `cmp 0x2fd`).
        0x2fd..=0x301 => {
            let _ = pe_newspaper_gift_place_fire_483706(city, ux, uy);
        }
        // PE @ `0x483731`: stories `0x306..=0x30a` police (`cmp 0x30b` / `cmp 0x306`).
        0x306..=0x30a => {
            let _ = pe_newspaper_gift_place_police_483731(city, ux, uy);
        }
        // PE @ `0x483767`: stories `0x30b`/`0x31b` stadium at `(x+1, y-2)`.
        0x30b | 0x31b if uy >= 2 => {
            let _ = pe_newspaper_gift_place_stadium_483767(city, crt, ux + 1, uy - 2);
        }
        // PE @ `0x483aab`: story `0x32b` only (gate `0x483aa0`); PlaceTile-size at `(x+1, y-2)`.
        0x32b if uy >= 2 => {
            let _ = pe_newspaper_gift_place_nuclear_483aab(city, ux + 1, uy - 2);
        }
        // PE @ `0x483874`: stories `0x348..=0x34b` road stub.
        0x348..=0x34b => pe_newspaper_gift_place_road_stub_483874(city, ux, uy),
        // PE @ `0x4835f4`/`0x483b48`: zone-type then optional zone-paint.
        // Includes type-only bands `0x34c..=0x351` / `0x374..=0x3a2` (selector ax=0).
        0xf0..=0x2b4 | 0x34c..=0x351 | 0x374..=0x3a2 => {
            pe_newspaper_gift_zone_type_then_paint_483b48(city, crt, ux, uy, story_id);
        }
        // PE bridge arms @ `0x4838a9`/`0x483a43`/`0x483a25`/`0x48394d`/`0x4839c3`/`0x4839f0`.
        0x3b5 => pe_newspaper_gift_place_bridge_4838a9(city, ux, uy),
        0x3b4 | 0x3b6 => pe_newspaper_gift_place_bridge_rotated_483a43(city, ux, uy),
        0x33c | 0x33e => pe_newspaper_gift_place_bridge_plain_483a25(city, ux, uy),
        0x3b7 => pe_newspaper_gift_place_bridge_48394d(city, ux, uy),
        0x33d => pe_newspaper_gift_place_bridge_4839c3(city, ux, uy),
        0x33f => pe_newspaper_gift_place_bridge_4839f0(city, ux, uy),
        _ => {}
    }
}

/// PE newspaper gift PlaceTile road stub @ `0x483874`: PlaceTile(`0x0d`).
pub(super) fn pe_newspaper_gift_place_road_stub_483874(city: &mut City, x: usize, y: usize) {
    pe_place_tile_441f00(city, x, y, 0x0d);
}

/// PE newspaper gift bridge PlaceTile @ `0x4838a9`: shore-prep then PlaceTile(`0x57`)
/// and OR XBIT rotate (`0x02`) on the placed tile(s).
pub(super) fn pe_newspaper_gift_place_bridge_4838a9(city: &mut City, x: usize, y: usize) {
    // PE @ `0x4838a9`: prep (x,y), PlaceTile 0x57 at (x+1,y+1), OR rotate there;
    // then PlaceTile 0x57 at (x+2,y+1), OR rotate there.
    pe_newspaper_gift_shore_prep_484320(city, x, y);
    let bx = x.saturating_add(1);
    let by = y.saturating_add(1);
    pe_place_tile_441f00(city, bx, by, 0x57);
    if let Some(bits) = city.tile_bits_at(bx, by) {
        city.set_tile_bits(bx, by, bits.with_rotate(true));
    }
    let sx = x.saturating_add(2);
    pe_place_tile_441f00(city, sx, by, 0x57);
    if let Some(bits) = city.tile_bits_at(sx, by) {
        city.set_tile_bits(sx, by, bits.with_rotate(true));
    }
}

/// PE newspaper gift bridge PlaceTile @ `0x483a25`: shore-prep then PlaceTile(`0x57`)
/// on the same tile (no rotate OR).
pub(super) fn pe_newspaper_gift_place_bridge_plain_483a25(city: &mut City, x: usize, y: usize) {
    pe_newspaper_gift_shore_prep_484320(city, x, y);
    pe_place_tile_441f00(city, x, y, 0x57);
}

/// PE newspaper gift bridge PlaceTile @ `0x483a43`: shore-prep then PlaceTile(`0x57`)
/// on the same tile and OR XBIT rotate.
pub(super) fn pe_newspaper_gift_place_bridge_rotated_483a43(city: &mut City, x: usize, y: usize) {
    pe_newspaper_gift_shore_prep_484320(city, x, y);
    pe_place_tile_441f00(city, x, y, 0x57);
    if let Some(bits) = city.tile_bits_at(x, y) {
        city.set_tile_bits(x, y, bits.with_rotate(true));
    }
}

/// PE newspaper gift bridge @ `0x48394d`: shore-prep (x,y), PlaceTile(`0x57`) at
/// (x-1,y+1), OR rotate.
pub(super) fn pe_newspaper_gift_place_bridge_48394d(city: &mut City, x: usize, y: usize) {
    pe_newspaper_gift_shore_prep_484320(city, x, y);
    let Some(px) = x.checked_sub(1) else {
        return;
    };
    let py = y.saturating_add(1);
    pe_place_tile_441f00(city, px, py, 0x57);
    if let Some(bits) = city.tile_bits_at(px, py) {
        city.set_tile_bits(px, py, bits.with_rotate(true));
    }
}

/// PE newspaper gift bridge @ `0x4839c3`: shore-prep (x,y), PlaceTile(`0x57`) at
/// (x+1,y-1) (no rotate OR).
pub(super) fn pe_newspaper_gift_place_bridge_4839c3(city: &mut City, x: usize, y: usize) {
    pe_newspaper_gift_shore_prep_484320(city, x, y);
    let Some(py) = y.checked_sub(1) else {
        return;
    };
    let px = x.saturating_add(1);
    pe_place_tile_441f00(city, px, py, 0x57);
}

/// PE newspaper gift bridge @ `0x4839f0`: shore-prep (x,y), PlaceTile(`0x57`) at
/// (x+1,y+1) and (x+1,y+2) (no rotate OR).
pub(super) fn pe_newspaper_gift_place_bridge_4839f0(city: &mut City, x: usize, y: usize) {
    pe_newspaper_gift_shore_prep_484320(city, x, y);
    let px = x.saturating_add(1);
    let py1 = y.saturating_add(1);
    let py2 = y.saturating_add(2);
    pe_place_tile_441f00(city, px, py1, 0x57);
    pe_place_tile_441f00(city, px, py2, 0x57);
}

/// PE newspaper gift disaster start @ `0x48332b` (story ids `0x30..=0x33`): active=1,
/// type=2, XTXT=`0xfc` at (x,y).
pub(super) fn pe_newspaper_gift_start_disaster_48332b(city: &mut City, x: usize, y: usize) {
    city.set_pe_dw_disaster_active(1);
    city.set_pe_w_current_disaster_type(2);
    if x < CITY_WIDTH && y < CITY_HEIGHT {
        city.set_text_overlay(x, y, 0xfc);
    }
}

/// PE newspaper gift disaster start @ `0x48336d` (story ids `0x38..=0x3f`): active=1,
/// type=1, XTXT=`0xff` at (x,y).
pub(super) fn pe_newspaper_gift_start_disaster_48336d(city: &mut City, x: usize, y: usize) {
    city.set_pe_dw_disaster_active(1);
    city.set_pe_w_current_disaster_type(1);
    if x < CITY_WIDTH && y < CITY_HEIGHT {
        city.set_text_overlay(x, y, 0xff);
    }
}

/// PE newspaper gift network-tile table @ `0x4ea9d8` (indices `0..=15` after
/// `story_id - 0x40` folded with `&0xf`).
const PE_NEWSPAPER_GIFT_TILE_TABLE_4EA9D8: [u8; 16] = [
    87, 87, 29, 30, 38, 35, 36, 37, 42, 39, 40, 41, 43, 67, 68, 0,
];

/// PE newspaper gift PlaceTile-from-table @ `0x4827d1`/`0x483412`: optional shore-prep
/// for indices `0`/`1`/`0xf`, OR XBIT `0xc0` for `0xd`/`0xe`, PlaceTile table tile,
/// OR rotate when index `== 1`.
pub(super) fn pe_newspaper_gift_place_table_tile_483412(
    city: &mut City,
    x: usize,
    y: usize,
    index: u8,
) {
    let index = (index & 0xf) as usize;
    if matches!(index, 0 | 1 | 0xf) {
        pe_newspaper_gift_shore_prep_484320(city, x, y);
    }
    if matches!(index, 0xd | 0xe) {
        if let Some(bits) = city.tile_bits_at(x, y) {
            city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0xc0));
        } else {
            city.set_tile_bits(x, y, TileBits::new(0xc0));
        }
    }
    let tile = PE_NEWSPAPER_GIFT_TILE_TABLE_4EA9D8[index];
    pe_place_tile_441f00(city, x, y, tile);
    if index == 1 {
        if let Some(bits) = city.tile_bits_at(x, y) {
            city.set_tile_bits(x, y, bits.with_rotate(true));
        }
    }
}

/// PE newspaper gift network-tile table @ `0x4ea9e8` (story ids `0xd0..=0xdf` folded).
const PE_NEWSPAPER_GIFT_TILE_TABLE_4EA9E8: [u8; 16] = [
    92, 92, 14, 15, 23, 20, 21, 22, 27, 24, 25, 26, 28, 71, 72, 0,
];

/// PE newspaper gift PlaceTile-from-table @ `0x4828af`/`0x4834bc`: optional shore-prep
/// for `0`/`1`/`0xf`, PlaceTile table tile, OR XBIT `0xc0`, OR rotate when index `== 0`.
pub(super) fn pe_newspaper_gift_place_table_tile_4834bc(
    city: &mut City,
    x: usize,
    y: usize,
    index: u8,
) {
    let index = (index & 0xf) as usize;
    if matches!(index, 0 | 1 | 0xf) {
        pe_newspaper_gift_shore_prep_484320(city, x, y);
    }
    let tile = PE_NEWSPAPER_GIFT_TILE_TABLE_4EA9E8[index];
    pe_place_tile_441f00(city, x, y, tile);
    if let Some(bits) = city.tile_bits_at(x, y) {
        let mut bits = TileBits::new(bits.raw_bits() | 0xc0);
        if index == 0 {
            bits = bits.with_rotate(true);
        }
        city.set_tile_bits(x, y, bits);
    } else {
        let mut bits = TileBits::new(0xc0);
        if index == 0 {
            bits = bits.with_rotate(true);
        }
        city.set_tile_bits(x, y, bits);
    }
}

/// PE newspaper gift network-tile table @ `0x4ea9f8` (story ids `0xe0..=0xef` folded).
const PE_NEWSPAPER_GIFT_TILE_TABLE_4EA9F8: [u8; 16] = [
    90, 90, 44, 45, 53, 50, 51, 52, 57, 54, 55, 56, 58, 70, 69, 0,
];

/// PE newspaper gift PlaceTile-from-table @ `0x482980`/`0x48358a`: optional shore-prep
/// for `0`/`1`/`0xf`, PlaceTile table tile, OR rotate when index `== 1`.
pub(super) fn pe_newspaper_gift_place_table_tile_48358a(
    city: &mut City,
    x: usize,
    y: usize,
    index: u8,
) {
    let index = (index & 0xf) as usize;
    if matches!(index, 0 | 1 | 0xf) {
        pe_newspaper_gift_shore_prep_484320(city, x, y);
    }
    let tile = PE_NEWSPAPER_GIFT_TILE_TABLE_4EA9F8[index];
    pe_place_tile_441f00(city, x, y, tile);
    if index == 1 {
        if let Some(bits) = city.tile_bits_at(x, y) {
            city.set_tile_bits(x, y, bits.with_rotate(true));
        }
    }
}

/// PE newspaper gift zone+power mark @ `0x48362a` (story ids `0x2b5..=0x2c4`):
/// XZON `&=0xf9 |=9`, XBIT `|=0xc0`.
pub(super) fn pe_newspaper_gift_zone_power_48362a(city: &mut City, x: usize, y: usize) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    city.set_zone(x, y, (zone & 0xf9) | 9);
    let bits = city.tile_bits_at(x, y).unwrap_or_else(|| TileBits::new(0));
    city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0xc0));
}

/// PE newspaper gift zone+power mark @ `0x483aca`: XZON `&=0xf8 |=8`, XBIT `|=0xc0`.
pub(super) fn pe_newspaper_gift_zone_power_483aca(city: &mut City, x: usize, y: usize) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    city.set_zone(x, y, (zone & 0xf8) | 8);
    let bits = city.tile_bits_at(x, y).unwrap_or_else(|| TileBits::new(0));
    city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0xc0));
}

/// PE newspaper gift zone-type+power @ `0x483b48`: set XZON type nibble to
/// `2`/`4`/`6` from story id (`<0x1a7` / `<0x264` / else), then XBIT `|=0xc0`.
pub(super) fn pe_newspaper_gift_zone_type_power_483b48(
    city: &mut City,
    x: usize,
    y: usize,
    story_id: u16,
) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    let zone_type: u8 = if story_id < 0x1a7 {
        2
    } else if story_id < 0x264 {
        4
    } else {
        6
    };
    let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    city.set_zone(x, y, (zone & 0xf0) | zone_type);
    let bits = city.tile_bits_at(x, y).unwrap_or_else(|| TileBits::new(0));
    city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0xc0));
}

/// PE gift zone-paint size index after zone-type @ `0x483bd5`..`0x483cec`.
/// Returns `(ax_size, paint_type)` where `ax_size` is the pre-`dec` jump selector
/// (`1`..=`6`); `None` when PE takes the `ax==0` / out-of-band exit.
fn pe_newspaper_gift_zone_paint_selector_483bd5(
    story_id: u16,
    zone_nibble: u8,
) -> Option<(u8, u8)> {
    let paint_from_zone = zone_nibble.saturating_sub(1) / 2;
    let (ax_size, paint_type) = if story_id < 0xf9 {
        return None;
    } else if story_id < 0x105 {
        (1u8, paint_from_zone)
    } else if story_id < 0x195 {
        let delta = u32::from(story_id - 0x105);
        (((delta / 0x24) + 1) as u8, (delta % 9) as u8)
    } else if story_id < 0x19e {
        (5u8, ((story_id - 0x195) % 9) as u8)
    } else if story_id < 0x1a7 {
        (6u8, ((story_id - 0x19e) % 9) as u8)
    } else if (0x1b0..0x264).contains(&story_id) {
        let delta = u32::from(story_id - 0x1b0);
        ((delta / 0x24) as u8, (delta % 9) as u8)
    } else if (0x26d..0x2b5).contains(&story_id) {
        let delta = u32::from(story_id - 0x26d);
        ((2 * (delta / 0x24) + 2) as u8, (delta % 9) as u8)
    } else {
        return None;
    };
    if !(1..=6).contains(&ax_size) {
        return None;
    }
    Some((ax_size, paint_type))
}

/// PE newspaper gift zone-type then zone-paint fallthrough @ `0x483b48`→`0x483cf3`+.
fn pe_newspaper_gift_zone_type_then_paint_483b48(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
    story_id: u16,
) {
    pe_newspaper_gift_zone_type_power_483b48(city, x, y, story_id);
    let zone_nibble = city.zone_at(x, y).map(|z| z.raw_bits() & 0x0f).unwrap_or(0);
    let Some((ax_size, paint_type)) =
        pe_newspaper_gift_zone_paint_selector_483bd5(story_id, zone_nibble)
    else {
        return;
    };
    match ax_size {
        1 => pe_newspaper_gift_zone_paint_size1_436ee7(city, crt, x, y, paint_type),
        2 => pe_newspaper_gift_zone_paint_size2_483d08(city, crt, x, y, paint_type),
        3 => pe_newspaper_gift_zone_paint_size3_483d6b(city, crt, x, y, paint_type),
        4 => pe_newspaper_gift_zone_paint_size4_483dce(city, crt, x, y, paint_type),
        // PE hospital/church JT arms gate on paint type `bx == 4`.
        5 if paint_type == 4 => {
            let _ = pe_newspaper_gift_place_hospital_483df1(city, x, y);
        }
        6 if paint_type == 4 && x > 0 && y > 0 => {
            let _ = pe_newspaper_gift_place_church_483e14(city, x - 1, y - 1);
        }
        _ => {}
    }
}

/// PE newspaper gift PlaceTile-size hospital @ `0x483df1`: (`0xd1`, 3) at click; post-dec NW.
pub(super) fn pe_newspaper_gift_place_hospital_483df1(city: &mut City, x: usize, y: usize) -> bool {
    let Some((nw_x, nw_y)) = pe_place_tile_size_click_to_nw(x, y, 3) else {
        return false;
    };
    pe_place_tile_size_440c50(city, nw_x, nw_y, HOSPITAL_BUILDING_ID, 3)
}

/// PE newspaper gift PlaceTile-size church @ `0x483e14`: (`0xf7`, 2) after `dec` x/y.
/// Size 2 does not dec inside PlaceTile-size; `x,y` are already the NW.
pub(super) fn pe_newspaper_gift_place_church_483e14(city: &mut City, x: usize, y: usize) -> bool {
    pe_place_tile_size_440c50(city, x, y, CHURCH_BUILDING_ID, CHURCH_BUILDING_SIZE)
}

/// PE newspaper gift PlaceTile trees @ `0x4832d9` (story ids `0x15..=0x24`):
/// PlaceTile `(rand() & 3) + 6`.
pub(super) fn pe_newspaper_gift_place_trees_4832d9(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
) {
    let building = ((crt.random_u16() & 3) + 6) as u8;
    pe_place_tile_441f00(city, x, y, building);
}

/// PE newspaper gift PlaceTile trees @ `0x4832ff` (story ids `0x25..=0x2b`):
/// PlaceTile `(rand() & 3) + 9`.
pub(super) fn pe_newspaper_gift_place_trees_4832ff(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
) {
    let building = ((crt.random_u16() & 3) + 9) as u8;
    pe_place_tile_441f00(city, x, y, building);
}

/// PE newspaper gift empty-tile rubble @ `0x483e37`: if XBLD `== 0`, PlaceTile
/// `(rand() & 3) + 1`.
pub(super) fn pe_newspaper_gift_place_rubble_483e37(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT || city.building_at(x, y) != 0 {
        return;
    }
    let building = ((crt.random_u16() & 3) + 1) as u8;
    pe_place_tile_441f00(city, x, y, building);
}

/// PE newspaper gift force-tree @ `0x483e68`: if XBLD nonzero, DestroyBuilding mode 0
/// (`0x402211` → `0x458d40`); always PlaceTile `5`. XBLD `< 6` DestroyBuilding early-outs
/// (`0x458d8a`); elevated `0x3f..=0x42` use the elevated arm (`0x45994e`), not
/// DestroyStructure. Non-elevated `>= 6` consumes process CRT PlaceTile rubble
/// (`0x45a01f`) before the final PlaceTile `5`.
pub(super) fn pe_newspaper_gift_place_tree_483e68(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    // 0x483e7f pushes flag 0 for DestroyBuilding before planting the tree.
    pe_destroy_building_mode0_optional_crt(city, Some(crt), x, y);
    pe_place_tile_441f00(city, x, y, 5);
}

/// PE newspaper gift zone-paint post-mark @ `0x436f5f`: XZON `|= 0xf0` (all corners).
pub(super) fn pe_newspaper_gift_mark_zone_corners_436f5f(city: &mut City, x: usize, y: usize) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    city.set_zone(x, y, zone | 0xf0);
}

/// PE zone-paint building pick @ `0x436ee7`/`0x436fc8`: index `size + type*4` into
/// base `0x4e755e` and modulus `0x4e7586`, then `base + rand()%mod` (mod `0` → `1`).
fn pe_zone_paint_pick_building_4e755e(crt: &mut Win95CrtRand, size: u8, zone_type: u8) -> u8 {
    let size = size.max(1).min(4);
    let zone_type = zone_type.min(6);
    let index = usize::from(size) + usize::from(zone_type) * 4;
    // Parallel word tables @ `0x4e755e` / `0x4e7586` (indices 0..28).
    const BASES: [u16; 29] = [
        0, 0x70, 0x8c, 0x90, 0xae, 0x7c, 0x94, 0x99, 0xb2, 0x84, 0x9e, 0xa2, 0xbc, 0x88, 0xa6,
        0xa8, 0xc2, 0x8a, 0xaa, 0xac, 0xc4, 0x0c, 0x04, 0x04, 0x04, 0x08, 0x05, 0x05, 0x0a,
    ];
    const MODS: [u16; 29] = [
        0xc4, 0x0c, 0x04, 0x04, 0x04, 0x08, 0x05, 0x05, 0x0a, 0x04, 0x04, 0x04, 0x06, 0x02, 0x02,
        0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x00, 0x01, 0x08, 0x0c, 0x24, 0x00, 0x00, 0x00,
    ];
    let base = BASES.get(index).copied().unwrap_or(0);
    let modulus = MODS.get(index).copied().unwrap_or(1).max(1);
    (base.wrapping_add(crt.random_u16() % modulus) & 0xff) as u8
}

/// PE newspaper gift zone-paint size-1 @ `0x436ee7`/`0x483cf3`: PlaceTile
/// `base[type] + rand()%mod[type]`, then XZON `|=0xf0`, XBIT `|=0xe0`.
pub(super) fn pe_newspaper_gift_zone_paint_size1_436ee7(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
    zone_type: u8,
) {
    let building = pe_zone_paint_pick_building_4e755e(crt, 1, zone_type);
    pe_place_tile_441f00(city, x, y, building);
    pe_newspaper_gift_mark_zone_corners_436f5f(city, x, y);
    // PE @ `0x436fad`: XBIT `|= 0xe0` after corner OR.
    if x < CITY_WIDTH && y < CITY_HEIGHT {
        let bits = city.tile_bits_at(x, y).unwrap_or_default().raw_bits();
        city.set_tile_bits(x, y, TileBits::new(bits | XBIT_CHOOSE_AND_PLACE));
    }
}

/// PE zone-paint size>1 @ `0x436fb6`: pick building, `half = size/2`, stamp
/// `(half+1)×(half+1)` from `(x, y-half)` via PlaceTile; each tile `XZON&=0x0f` and
/// XBIT `|=0xe0`; then OR `0x4dc4d0` corners on the four corners (same CAP table).
pub(super) fn pe_newspaper_gift_zone_paint_size_gt1_436fb6(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
    size: u8,
    zone_type: u8,
) -> bool {
    let size = size.max(2).min(4);
    let half = usize::from(size) / 2;
    // PE @ `0x436fee`..`0x43702d`: x/y ≥ 2 and ≤ `0x7e - half`.
    if x < 2 || y < 2 || x > 0x7e - half || y > 0x7e - half || y < half {
        return false;
    }
    let building = pe_zone_paint_pick_building_4e755e(crt, size, zone_type);
    let y0 = y - half;
    let corner_bits = pe_choose_and_place_corner_bits(city.rotation());
    for tx in x..=(x + half) {
        for ty in y0..=(y0 + half) {
            if tx >= CITY_WIDTH || ty >= CITY_HEIGHT {
                continue;
            }
            pe_place_tile_441f00(city, tx, ty, building);
            let z = city.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0) & 0x0f;
            city.set_zone(tx, ty, z);
            let bits = city.tile_bits_at(tx, ty).unwrap_or_default().raw_bits();
            city.set_tile_bits(tx, ty, TileBits::new(bits | XBIT_CHOOSE_AND_PLACE));
        }
    }
    // PE @ `0x4370e9`..`0x43721b`: NW/NE/SE/SW from `0x4dc4d0+[rot*8]`.
    for (tx, ty, bit) in [
        (x, y0, corner_bits[0]),
        (x + half, y0, corner_bits[1]),
        (x + half, y0 + half, corner_bits[3]),
        (x, y0 + half, corner_bits[2]),
    ] {
        if tx < CITY_WIDTH && ty < CITY_HEIGHT {
            let z = city.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0) & 0x0f;
            city.set_zone(tx, ty, z | bit);
        }
    }
    true
}

/// PE newspaper gift zone-paint size-2 @ `0x483d08`: type 4 jitters origin then size-2
/// paint; else empty XBLD falls through to size-1 paint.
pub(super) fn pe_newspaper_gift_zone_paint_size2_483d08(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
    zone_type: u8,
) {
    pe_newspaper_gift_zone_paint_size_jitter_483d08(city, crt, x, y, zone_type, 2);
}

/// PE newspaper gift zone-paint size-3 @ `0x483d6b`: same jitter/empty gates as size-2
/// with paint size 3.
pub(super) fn pe_newspaper_gift_zone_paint_size3_483d6b(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
    zone_type: u8,
) {
    pe_newspaper_gift_zone_paint_size_jitter_483d08(city, crt, x, y, zone_type, 3);
}

/// Shared gift size-2/3 type-4 jitter (`0x483d08`/`0x483d6b`): `y' = y-(rand&1)+1`,
/// `x' = x+(rand&1)-1`, then size-N paint; non-type-4 empty XBLD → size-1.
fn pe_newspaper_gift_zone_paint_size_jitter_483d08(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
    zone_type: u8,
    paint_size: u8,
) {
    if zone_type == 4 {
        let y2 = (y as isize) - ((crt.random_u16() & 1) as isize) + 1;
        let x2 = (x as isize) + ((crt.random_u16() & 1) as isize) - 1;
        if x2 < 0 || y2 < 0 {
            return;
        }
        let _ = pe_newspaper_gift_zone_paint_size_gt1_436fb6(
            city,
            crt,
            x2 as usize,
            y2 as usize,
            paint_size,
            zone_type,
        );
        return;
    }
    if x < CITY_WIDTH && y < CITY_HEIGHT && city.building_at(x, y) == 0 {
        pe_newspaper_gift_zone_paint_size1_436ee7(city, crt, x, y, zone_type);
    }
}

/// PE newspaper gift zone-paint size-4 @ `0x483dce`: type 4 only — `(x-1, y+1)` then
/// size-4 paint; other types skip.
pub(super) fn pe_newspaper_gift_zone_paint_size4_483dce(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
    zone_type: u8,
) {
    if zone_type != 4 || x == 0 {
        return;
    }
    let _ = pe_newspaper_gift_zone_paint_size_gt1_436fb6(city, crt, x - 1, y + 1, 4, zone_type);
}
