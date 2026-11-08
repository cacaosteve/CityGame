//! Highway tile stamping and class-specific PlaceHighwayTile workers.
//!
//! These workers preserve the native write order for terrain, building, and
//! zone bytes; routing and neighboring-network policies stay in their own
//! modules.

use super::super::NetworkAxis;
use super::{building_zone_bits_for_tile, pe_choose_and_place_corner_bits, pe_place_tile_441f00};
use super::{
    highway_segment_tiles, pe_get_highway_tile_462910, pe_highway_altitude_463f60,
    pe_highway_classify_463c40, pe_highway_route_gate_low_word_464040,
};
use crate::city::City;

/// PE CityToolPlaceHighway stamp `FUN_004626d0` @ `0x46270b`–`0x4627a1`: for each
/// in-bounds 2×2 footprint tile, `XZON &= 0xf0` (clear zone type, keep corners)
/// before PlaceTile follow-ups. Does not live inside PlaceTile itself.
pub(in crate::app) fn pe_city_tool_place_highway_clear_xzon_4626d0(
    city: &mut City,
    tiles: &[(usize, usize)],
) {
    for &(x, y) in tiles {
        let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
        city.set_zone(x, y, zone & 0xf0);
    }
}

/// PE `FUN_00462910` GetHighwayTile (thunk `0x402072`): this leftover covers the
/// vacant / no-neighbor arm after gate `0x464040` returns `0xf` @ `0x462c3f`–
/// `0x462c5a` — class `2 + (orient&1)`. Rust maps drag axis: GridY→2, GridX→3.
/// Bounds refuse @ `0x46291f`/`0x462d99` → `ax = -1`. Empty mask does not read XBLD.
/// PE GetHighwayTile early classify refuse @ `0x462949`–`0x46298f`:
/// `FUN_00463c40` == 0/1, 4..=7, or `>= 0x0d` → `ax = -1` before the neighbor scan.
/// PE neighbor low-bit connect (`0x4629b8` NS / `0x462a3b` EW): class 1/3 (NS) or
/// 0/2 (EW), else class `>= 4` and the `0x4e89d8` / `0x4e8990` dword is nonzero.
/// Class-2/3 `0x464040 != 0x4000` arms are later.
/// PE `FUN_00463f60`: max of `ALTM&0x1f`, plus 1 when `XTER != 0`, over the 2×2.
/// PE `FUN_00462ed0` PlaceHighwayTile class `< 4` @ `0x46365f`–`0x46385e`:
/// tiles whose `ALTM&0x1f` is below the origin `FUN_00463f60` level get
/// `XTER = 0x0d` (`0x46368e`–`0x46371d`), then the 2×2 underlay forces class
/// 2/3 (`0x463722`–`0x463820`) and per-tile `FUN_00463b30` (`class&1`).
pub(in crate::app) fn pe_place_highway_tile_462ed0_class_lt4(
    city: &mut City,
    x: usize,
    y: usize,
    class: i16,
) {
    let origin_alt = pe_highway_altitude_463f60(city, x, y);
    let tiles = highway_segment_tiles((x, y), NetworkAxis::GridX);
    for &(tx, ty) in &tiles {
        if tx >= 0x80 || ty >= 0x80 {
            continue;
        }
        if (city.altitude_at(tx, ty) as u16) & 0x1f < origin_alt {
            city.set_terrain(tx, ty, 0x0d);
        }
    }
    let mut class = class;
    for &(tx, ty) in &tiles {
        match city.building_at(tx, ty) {
            // PE @ `0x463758` / `0x46379f` / …: `0x1d`/`0x0e`/`0x2c` → class 3.
            0x1d | 0x0e | 0x2c => class = 3,
            // PE @ `0x46374f` / `0x463796` / …: `0x1e`/`0x0f`/`0x2d` → class 2.
            0x1e | 0x0f | 0x2d => class = 2,
            _ => {}
        }
    }
    let class_bit = (class as u16 & 1) as u8;
    for &(tx, ty) in &tiles {
        pe_place_highway_tile_463b30(city, tx, ty, class_bit);
    }
}

/// PE PlaceHighwayTile class `8..=0xc` @ `0x462f14`–`0x46315c`.
/// Each footprint tile with `XTER != 0`, or `ALTM&0x1f` below the origin
/// `FUN_00463f60` altitude, becomes `XTER = 0x0d`. Then PlaceTile `class+0x5d`
/// and `XZON = (XZON&0x0f) |` rotation corner from `0x4dc4d0`.
pub(in crate::app) fn pe_place_highway_tile_462ed0_class_8_to_c(
    city: &mut City,
    x: usize,
    y: usize,
    class: i16,
) {
    let origin_alt = pe_highway_altitude_463f60(city, x, y);
    let tiles = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
    for &(tx, ty) in &tiles {
        if tx >= 0x80 || ty >= 0x80 {
            continue;
        }
        let tile_alt = (city.altitude_at(tx, ty) as u16) & 0x1f;
        if city.terrain_at(tx, ty) != 0 || tile_alt < origin_alt {
            city.set_terrain(tx, ty, 0x0d);
        }
    }
    let building = (class + 0x5d) as u8;
    for &(tx, ty) in &tiles {
        pe_place_tile_441f00(city, tx, ty, building);
    }
    let corners = pe_choose_and_place_corner_bits(city.rotation());
    for &(tx, ty) in &tiles {
        if tx >= 0x80 || ty >= 0x80 {
            continue;
        }
        let corner = building_zone_bits_for_tile(x, y, tx, ty, 2, corners);
        let low = city.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0) & 0x0f;
        city.set_zone(tx, ty, low | corner);
    }
}

/// PE `FUN_00463b30`: underlay `xbld-0x0e` jump-table PlaceTile, else vacant
/// `0x49 + class_bit`, then `XZON |= 0xf0`.
pub(in crate::app) fn pe_place_highway_tile_463b30(
    city: &mut City,
    x: usize,
    y: usize,
    class_bit: u8,
) {
    let xbld = city.building_at(x, y);
    let building = match xbld {
        0x0e => 0x50,
        0x0f => 0x4f,
        0x1d => 0x4c,
        0x1e => 0x4b,
        0x2c => 0x4e,
        0x2d => 0x4d,
        // Other `0x0e..=0x2d` fall to vacant case 6; outside window same.
        _ => 0x49u8.wrapping_add(class_bit),
    };
    pe_place_tile_441f00(city, x, y, building);
    let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    city.set_zone(x, y, zone | 0xf0);
}

/// PE PlaceHighwayTile class `>= 0x0d` @ `0x46315d`–`0x4632bb`: PlaceTile
/// `class+0x5d` and rotation corners. No slope `XTER` rewrite. Both
/// `0x4012c1` neighbor refreshes stay deferred.
pub(in crate::app) fn pe_place_highway_tile_462ed0_class_ge_0d(
    city: &mut City,
    x: usize,
    y: usize,
    class: i16,
) {
    let building = (class + 0x5d) as u8;
    let tiles = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
    for &(tx, ty) in &tiles {
        pe_place_tile_441f00(city, tx, ty, building);
    }
    let corners = pe_choose_and_place_corner_bits(city.rotation());
    for &(tx, ty) in &tiles {
        if tx >= 0x80 || ty >= 0x80 {
            continue;
        }
        let corner = building_zone_bits_for_tile(x, y, tx, ty, 2, corners);
        let low = city.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0) & 0x0f;
        city.set_zone(tx, ty, low | corner);
    }
}

/// PE `FUN_004626d0` @ `0x4626df`–`0x46288d`: gate `0x8000` returns first, then
/// origin Get→Place, four ±2 neighbor arms (classify `>= 2` and Get `>= 0`),
/// then trailing origin Get→Place.
pub(in crate::app) fn pe_city_tool_place_highway_stamp_body_4626d0(
    city: &mut City,
    x: usize,
    y: usize,
    axis: NetworkAxis,
) {
    // PE `0x4626df`–`0x4626eb`: gate `0x8000` returns before the `XZON` clear.
    if pe_highway_route_gate_low_word_464040(city, x, y) == 0x8000 {
        return;
    }
    pe_city_tool_place_highway_clear_xzon_4626d0(
        city,
        &highway_segment_tiles((x, y), NetworkAxis::GridX),
    );
    let class = pe_get_highway_tile_462910(city, x, y, axis);
    pe_place_highway_tile_for_stamp_462ed0(city, x, y, class);
    let ox = x as isize;
    let oy = y as isize;
    for (nx, ny) in [(ox - 2, oy), (ox + 2, oy), (ox, oy - 2), (ox, oy + 2)] {
        if pe_highway_classify_463c40(city, nx, ny) < 2 {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        let nclass = pe_get_highway_tile_462910(city, nx, ny, axis);
        pe_place_highway_tile_for_stamp_462ed0(city, nx, ny, nclass);
    }
    // Trailing origin re-Get→Place @ `0x462873`.
    let class = pe_get_highway_tile_462910(city, x, y, axis);
    pe_place_highway_tile_for_stamp_462ed0(city, x, y, class);
}

pub(in crate::app) fn pe_place_highway_tile_for_stamp_462ed0(
    city: &mut City,
    x: usize,
    y: usize,
    class: i16,
) {
    if (0..4).contains(&class) {
        pe_place_highway_tile_462ed0_class_lt4(city, x, y, class);
    } else if (4..8).contains(&class) {
        pe_place_highway_tile_462ed0_class_4_to_7(city, x, y, class);
    } else if (8..0x0d).contains(&class) {
        pe_place_highway_tile_462ed0_class_8_to_c(city, x, y, class);
    } else if class >= 0x0d {
        pe_place_highway_tile_462ed0_class_ge_0d(city, x, y, class);
    }
}

/// PE PlaceHighwayTile class `4..=7` @ `0x4632bc`–`0x46355c`.
/// Mismatched footprint altitude is rewritten first (`0x463325`). Then the
/// `0x46389c` switch writes the slope `XTER` pair and PlaceTile `class+0x5d`
/// plus `0x4dc4d0` corners. Neighbor refresh `0x4012c1` stays deferred.
pub(in crate::app) fn pe_place_highway_tile_462ed0_class_4_to_7(
    city: &mut City,
    x: usize,
    y: usize,
    class: i16,
) {
    let origin_alt = pe_highway_altitude_463f60(city, x, y);
    let tiles = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
    let all_match = tiles.iter().all(|&(tx, ty)| {
        tx < 0x80 && ty < 0x80 && (city.altitude_at(tx, ty) as u16) & 0x1f == origin_alt
    });
    if !all_match {
        let land = (origin_alt.saturating_sub(1) & 0x1f) as u8;
        for &(tx, ty) in &tiles {
            if tx >= 0x80 || ty >= 0x80 {
                continue;
            }
            city.set_altitude(tx, ty, land, city.water_height_at(tx, ty));
        }
    }
    // class-4: `0x463425`; 5: `0x463458`; 6: `0x463481`; 7: `0x4634b1`.
    let xter = match class {
        4 => [0x0d, 1, 0x0d, 1],
        5 => [0x0d, 0x0d, 2, 2],
        6 => [3, 0x0d, 3, 0x0d],
        _ => [4, 4, 0x0d, 0x0d],
    };
    let xter_tiles = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
    for (&(tx, ty), slope) in xter_tiles.iter().zip(xter) {
        if tx < 0x80 && ty < 0x80 {
            city.set_terrain(tx, ty, slope);
        }
    }
    let building = (class + 0x5d) as u8;
    for &(tx, ty) in &tiles {
        pe_place_tile_441f00(city, tx, ty, building);
    }
    let corners = pe_choose_and_place_corner_bits(city.rotation());
    for &(tx, ty) in &tiles {
        if tx >= 0x80 || ty >= 0x80 {
            continue;
        }
        let corner = building_zone_bits_for_tile(x, y, tx, ty, 2, corners);
        let low = city.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0) & 0x0f;
        city.set_zone(tx, ty, low | corner);
    }
}
