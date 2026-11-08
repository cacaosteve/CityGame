use super::{pe_destroy_building_mode1_optional_crt, pe_place_tile_441f00, pe_place_xund_442370};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};
use crate::win95_random::Win95CrtRand;

/// PE post-terrain refresh `0x40215d`→`0x447490`: `FUN_00447020` on 8 NESW+diag
/// neighbors plus center (`0x4e7a68`/`0x4e7a80` with trailing 0,0).
/// PE `0x447490` also calls `0x4012c1` twice per neighbor (9×2); see
/// `PE_447490_UI_WAIT_4012C1_PER_CALL`.
/// PE `FUN_00447020` pre-slope sequence @ `0x447085..0x4470b1`:
/// recursively destroy the saved XBLD when it is `>= 0x0d`, then conditionally
/// PlaceTile(0) when saved `wCityMode != 0` and the pre-recursion XBLD is not 5.
pub(in crate::app) fn pe_fun_00447020_mode1_destroy_and_city_clear_447085(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return;
    }
    let saved_building = city.building_at(x, y);
    if saved_building >= 0x0d {
        pe_destroy_building_mode1_optional_crt(city, crt.as_deref_mut(), x, y);
    }
    if city.pe_w_city_mode != 0 && saved_building != 5 {
        pe_place_tile_441f00(city, x, y, 0);
    }
}

pub(in crate::app) fn pe_terrain_neighbor_fun_00447020_447490(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) {
    pe_terrain_neighbor_fun_00447020_447490_signed(city, crt, x as i16, y as i16);
}

pub(in crate::app) fn pe_terrain_neighbor_fun_00447020_447490_signed(
    city: &mut City,
    mut crt: Option<&mut Win95CrtRand>,
    x: i16,
    y: i16,
) {
    let sea = pe_selected_water_height_for_fun_00447020(city);
    for i in 0..9 {
        let nx = x + PE_TERRAIN_NEIGHBOR_REFRESH_DX[i];
        let ny = y + PE_TERRAIN_NEIGHBOR_REFRESH_DY[i];
        if !(0..0x80).contains(&nx) || !(0..0x80).contains(&ny) {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        pe_fun_00447020_mode1_destroy_and_city_clear_447085(city, crt.as_deref_mut(), nx, ny);
        pe_fun_00447020_mode0_xund_slope_sea(city, nx, ny, sea);
    }
}

/// PE `0x447490` UI waits: nine neighbor slots × (`0x4012c1` before + after `0x402bd0`).
pub(in crate::app) const PE_447490_UI_WAIT_4012C1_PER_CALL: u32 = 18;

/// PE `FUN_00447020` mode-0 subset after the XBLD `>= 0x0d` DestroyBuilding gate:
/// clear XUND; remap slope XTER; XZON `&= 0xf0` when slope ≠ 0; wet/dry sea arms
/// (`0x4471b3`–`0x447399`), including slope `0x32` dry XTER0 / wet XTER `0x10`.
/// Mode word `0x4ca42c == 0` skips the early PlaceTile(0).
pub(in crate::app) fn pe_fun_00447020_mode0_xund_slope_sea(
    city: &mut City,
    x: usize,
    y: usize,
    sea: i16,
) {
    // PE FUN_00447020 @ `0x447039`/`0x447044`/`0x447052`/`0x44705d` → `0x4473a3`:
    // refuse x/y outside `0..0x80` before XUND / slope / sea arms.
    if x >= 0x80 || y >= 0x80 {
        return;
    }
    if city.underground_at(x, y) != 0 {
        pe_place_xund_442370(city, x, y, 0);
    }
    let slope = pe_fun_00447020_slope_xter(city, x, y);
    if slope != 0 {
        let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
        city.set_zone(x, y, zone & 0xf0);
    }
    let mut land = i16::from(city.altitude_at(x, y) & 0x1f);
    let bits = city.tile_bits_at(x, y).unwrap_or_default();
    // PE slope `0x32` @ `0x4472ae`: second ALTM land++ then dry XTER 0 (`0x44730e`) /
    // wet XTER `0x10` (`0x447399`) with sea/XBIT/PlaceTile arms.
    if slope == 0x32 {
        land += 1;
        let altitude = city.altm_at(x, y);
        city.set_altm_word(
            x,
            y,
            (u16::from(altitude.water) << 5)
                | (u16::from(altitude.tunnel_levels) << 10)
                | land as u16,
        );
        if sea > land {
            city.set_tile_bits(x, y, bits.with_water_tile(true));
            pe_fun_00447020_pack_selected_water_height(city, x, y, sea);
            if city.building_at(x, y) != 5 {
                pe_place_tile_441f00(city, x, y, 0);
            }
            city.set_terrain(x, y, 0x10);
        } else {
            city.set_tile_bits(x, y, bits.with_water_tile(false));
            city.set_terrain(x, y, 0);
        }
        return;
    }
    if sea > land {
        city.set_tile_bits(x, y, bits.with_water_tile(true));
        pe_fun_00447020_pack_selected_water_height(city, x, y, sea);
        if city.building_at(x, y) != 5 {
            pe_place_tile_441f00(city, x, y, 0);
        }
        let wet_xter = if i32::from(sea) - i32::from(land) == 1 {
            slope.wrapping_add(0x20)
        } else {
            slope.wrapping_add(0x10)
        };
        city.set_terrain(x, y, wet_xter);
    } else {
        city.set_tile_bits(x, y, bits.with_water_tile(false));
        city.set_terrain(x, y, slope);
    }
}

/// PE 0x447219/0x447344: shift the full selected WORD, then OR into
/// `(ALTM & 0xfc1f)`. Values outside 0..31 can set tunnel/high bits.
pub(in crate::app) fn pe_fun_00447020_pack_selected_water_height(
    city: &mut City,
    x: usize,
    y: usize,
    sea: i16,
) {
    let altitude = city.altm_at(x, y);
    city.set_altm_word(
        x,
        y,
        u16::from(altitude.land)
            | (u16::from(altitude.tunnel_levels) << 10)
            | (sea as u16).wrapping_shl(5),
    );
}

pub(in crate::app) fn pe_selected_water_height_for_fun_00447020(city: &City) -> i16 {
    // PE 0x4ca818 loads the low WORD of MISC slot 912. Both classifier
    // comparisons use signed JG, independently of ALTM and saved file layout.
    city.mac_selected_water_height().unwrap_or(0) as u16 as i16
}

/// PE neighbor deltas for `FUN_00447020` terrain-mask @ `0x4470eb` (`0x4e7a68` / `0x4e7a80`).
pub(in crate::app) const PE_TERRAIN_SLOPE_DX: [i16; 8] = [0, 1, 1, 1, 0, -1, -1, -1];

pub(in crate::app) const PE_TERRAIN_SLOPE_DY: [i16; 8] = [-1, -1, 0, 1, 1, 1, 0, -1];

/// PE post-terrain neighbor refresh `0x447490` (`0x40215d`): same 8 deltas plus center.
pub(in crate::app) const PE_TERRAIN_NEIGHBOR_REFRESH_DX: [i16; 9] = [0, 1, 1, 1, 0, -1, -1, -1, 0];

pub(in crate::app) const PE_TERRAIN_NEIGHBOR_REFRESH_DY: [i16; 9] = [-1, -1, 0, 1, 1, 1, 0, -1, 0];

/// PE bit OR table `0x4e7ac0` when neighbor land > self land.
pub(in crate::app) const PE_TERRAIN_SLOPE_BITS: [u16; 8] = [3, 2, 6, 4, 12, 8, 9, 1];

/// PE remap `0x4e7958`: neighbor bit-mask → XTER slope byte.
pub(in crate::app) const PE_TERRAIN_MASK_TO_XTER: [u8; 256] = [
    0, 9, 10, 2, 11, 13, 3, 6, 12, 1, 13, 5, 4, 8, 7, 50, 0, 2, 10, 2, 3, 6, 3, 6, 11, 0, 0, 0, 3,
    6, 3, 6, 4, 13, 13, 13, 13, 13, 13, 13, 4, 13, 13, 13, 7, 13, 7, 50, 12, 13, 13, 13, 13, 13,
    13, 13, 4, 13, 13, 13, 13, 13, 13, 13, 4, 13, 13, 13, 13, 13, 13, 13, 4, 13, 13, 13, 7, 13, 7,
    13, 1, 5, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 1, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    13, 8, 13, 13, 13, 13, 13, 13, 13, 8, 13, 13, 13, 50, 13, 13, 13, 9, 2, 2, 2, 13, 13, 13, 6,
    13, 13, 13, 13, 13, 13, 13, 6, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    1, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 1, 5, 13, 5, 13, 13, 13, 50, 13, 13, 13, 13, 13, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 1, 5, 13, 5, 13, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13,
];

/// Win95 `FUN_00447020` neighbor-slope XTER (`0x4470eb` → remap `0x4e7958`).
pub(in crate::app) fn pe_fun_00447020_slope_xter(city: &City, x: usize, y: usize) -> u8 {
    let self_land = u16::from(city.altitude_at(x, y));
    let mut mask: u16 = 0;
    for i in 0..8 {
        let nx = x as i16 + PE_TERRAIN_SLOPE_DX[i];
        let ny = y as i16 + PE_TERRAIN_SLOPE_DY[i];
        if !(0..0x80).contains(&nx) || !(0..0x80).contains(&ny) {
            continue;
        }
        let nland = u16::from(city.altitude_at(nx as usize, ny as usize));
        if nland > self_land {
            mask |= PE_TERRAIN_SLOPE_BITS[i];
        }
    }
    PE_TERRAIN_MASK_TO_XTER[usize::from(mask & 0xff)]
}
