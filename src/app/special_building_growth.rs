//! Win95 special-building growth selectors and placement arms.
//!
//! This module preserves the PE selectors, preflight gates, and XBLD/XZON/XBIT
//! write sequence used by military bases, airports, seaports, and 2×2 growth.

use super::*;

/// PE `FUN_00437310` PlaceChurch (`0x4016f9`): PlaceTile `0xF7` on the 2×2 whose SW is
/// `(sw_x, sw_y)`, clear XZON then OR rotation corners from `0x4dc4d0`, XBIT `|= 0xe0`.
/// PE entry bounds @ `0x43731b`–`0x437343`: SW x/y must be in `1..=0x7e`.
pub(super) fn pe_place_church_437310(city: &mut City, sw_x: usize, sw_y: usize) {
    if !(1..=0x7e).contains(&sw_x) || !(1..=0x7e).contains(&sw_y) {
        return;
    }
    let nw_x = sw_x;
    let nw_y = sw_y - 1;
    let Some(tiles) = building_footprint_from_origin(nw_x, nw_y, CHURCH_BUILDING_SIZE) else {
        return;
    };
    let corner_bits = pe_choose_and_place_corner_bits(city.rotation());
    for &(tx, ty) in &tiles {
        pe_place_tile_441f00(city, tx, ty, CHURCH_BUILDING_ID);
        // PE XZON writes preserve each tile's low zone nibble and replace its CAP corner bits.
        let corners =
            building_zone_bits_for_tile(nw_x, nw_y, tx, ty, CHURCH_BUILDING_SIZE, corner_bits);
        let zone_type = city.zone_at(tx, ty).map(|zone| zone.zone_id()).unwrap_or(0);
        city.set_zone(tx, ty, zone_type | corners);
        let current = city.tile_bits_at(tx, ty).unwrap_or_default();
        city.set_tile_bits(
            tx,
            ty,
            TileBits::new(current.raw_bits() | XBIT_CHOOSE_AND_PLACE),
        );
    }
}

/// Win95 `FUN_004392A0` (`0x4392a0`) before army runway-cross PlaceTile when XBLD `>= 0x0D`
/// (`0x43847a`). Airport-range `0xDB..=0xEA`: single-tile PlaceTile(0), XBIT `&= 0x3F`,
/// XZON `&= 0x0F`. XBLD `< 0xC6` (incl. parks) is a no-op. Else mid-range / `> 0xEA`: even
/// 2×2 PlaceTile(0) + the same XBIT/XZON masks (`0x43935f`).
pub(super) fn pe_fun_004392a0_clear_before_runway(city: &mut City, x: usize, y: usize) {
    let building = city.building_at(x, y);
    if building < 0xc6 {
        return;
    }
    if (0xdb..=0xea).contains(&building) {
        pe_place_tile_441f00(city, x, y, 0);
        pe_fun_004392a0_mask_xbit_xzon(city, x, y);
        return;
    }
    // PE @ 0x43935f: align to even NW, PlaceTile(0) + masks on the 2×2.
    let origin_x = x & !1;
    let origin_y = y & !1;
    for dx in 0..2usize {
        for dy in 0..2usize {
            let tx = origin_x + dx;
            let ty = origin_y + dy;
            if tx >= CITY_WIDTH || ty >= CITY_HEIGHT {
                continue;
            }
            pe_place_tile_441f00(city, tx, ty, 0);
            pe_fun_004392a0_mask_xbit_xzon(city, tx, ty);
        }
    }
}

pub(super) fn pe_fun_004392a0_mask_xbit_xzon(city: &mut City, x: usize, y: usize) {
    if x >= 0x80 || y >= 0x80 {
        return;
    }
    if let Some(bits) = city.tile_bits_at(x, y) {
        city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0x3f));
    }
    if let Some(zone) = city.zone_at(x, y) {
        city.set_zone(x, y, zone.raw_bits() & 0x0f);
    }
}

/// PE army-base crane ALTM sample @ `0x438776`: water altitude `>=` land `+ 2`.
pub(super) fn pe_army_crane_altm_water_deep_enough(city: &City, x: usize, y: usize) -> bool {
    let sample = city.altm_at(x, y);
    u16::from(sample.water) >= u16::from(sample.land).saturating_add(2)
}

/// PE DoArmyBaseGrowth crane parity refuse @ `0x43869f`/`0x4386a8`: when DY!=0 and
/// crane_x odd, or DX!=0 and crane_y odd, return 0.
pub(super) fn pe_army_crane_parity_refuse_4386a0(
    crane_x: usize,
    crane_y: usize,
    dir: usize,
) -> bool {
    let dir = dir & 3;
    let dx = PE_ARMY_CRANE_STRIP_DX[dir];
    let dy = PE_ARMY_CRANE_STRIP_DY[dir];
    (dy != 0 && crane_x & 1 != 0) || (dx != 0 && crane_y & 1 != 0)
}

/// PE DoArmyBaseGrowth crane ray @ `0x4386e0`–`0x4387d4`: five steps along `dir` must
/// each have XBIT bit `0x04` and XBLD `== 0`; the fifth tile needs ALTM water `>=` land+2.
pub(super) fn pe_army_crane_ray_altm_gate_4386e0(
    city: &City,
    crane_x: usize,
    crane_y: usize,
    dir: usize,
) -> bool {
    let dir = dir & 3;
    let dx = PE_ARMY_CRANE_STRIP_DX[dir];
    let dy = PE_ARMY_CRANE_STRIP_DY[dir];
    let mut x = crane_x as i16;
    let mut y = crane_y as i16;
    for step in 0..5 {
        x += dx;
        y += dy;
        if !(0..0x80).contains(&x) || !(0..0x80).contains(&y) {
            return false;
        }
        let ux = x as usize;
        let uy = y as usize;
        let xbit = city
            .tile_bits_at(ux, uy)
            .map(|bits| bits.raw_bits())
            .unwrap_or(0);
        if xbit & 0x04 == 0 {
            return false;
        }
        if city.building_at(ux, uy) != 0 {
            return false;
        }
        if step == 4 {
            return pe_army_crane_altm_water_deep_enough(city, ux, uy);
        }
    }
    false
}

/// PE army crane stamp @ `0x4387ce`–`0x4387e7`: if XBLD `>= 0x0D` clear via
/// `FUN_004392A0`, then PlaceTile-size(`0xe0`, 1). Growth calls go through
/// SimulationGrow wrapper `0x4376b0` (type-7 allowed); stock PlaceTile-size still
/// refuses type 7 for non-growth callers.
pub(super) fn pe_army_crane_place_tile_size_4387e7(city: &mut City, x: usize, y: usize) {
    if city.building_at(x, y) >= 0x0d {
        pe_fun_004392a0_clear_before_runway(city, x, y);
    }
    let _ = pe_place_tile_size_440c50_ex(city, x, y, 0xe0, 1, true);
}

/// PE army crane XZON write @ `0x438809`–`0x438851`: replace type nibble, keep corners;
/// when type==7, XBIT `&= 0x0f` (`0x438851`).
pub(super) fn pe_army_crane_write_zone_after_place(
    city: &mut City,
    x: usize,
    y: usize,
    zone_type: u8,
) {
    let zone_type = zone_type & 0x0f;
    let current = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    city.set_zone(x, y, (current & 0xf0) | zone_type);
    if zone_type == 7 {
        if let Some(bits) = city.tile_bits_at(x, y) {
            city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0x0f));
        }
    }
}

/// PE army crane place + zone + strip @ `0x43862d`..`0x43890d`.
pub(super) fn pe_army_crane_place_and_zone_438809(
    city: &mut City,
    x: usize,
    y: usize,
    zone_type: u8,
) -> bool {
    let Some(dir) = pe_army_crane_select_strip_dir_43862d(city, x, y) else {
        return false;
    };
    if pe_army_crane_parity_refuse_4386a0(x, y, dir) {
        return false;
    }
    if !pe_army_crane_ray_altm_gate_4386e0(city, x, y, dir) {
        return false;
    }
    pe_army_crane_place_tile_size_4387e7(city, x, y);
    pe_army_crane_write_zone_after_place(city, x, y, zone_type);
    pe_army_crane_place_runway_cross_strip_438890(
        city,
        x,
        y,
        dir,
        pe_army_crane_strip_or_xbit_02(dir, city.rotation()),
    );
    true
}

/// PE DoArmyBaseGrowth missile-silo plot @ `0x438def`/`0x438e0a`/`0x438e25`:
/// PlaceTile(`0xf9`), PlaceXUND(`0x22`), then rotation corner XZON (`and 0x0f` / `or`
/// corner). Zone type 7 + `0x4ca934` word0 must already match a military stamp; on
/// blank force-plots Rust stamps type 7 first so PlaceTile uses the military census.
/// Does not OR ChooseAndPlace XBIT `0xe0`.
pub(super) fn pe_army_missile_silo_place_plot_438def(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
) {
    let corner_bits = pe_choose_and_place_corner_bits(city.rotation());
    for y in origin_y..origin_y + MILITARY_SILO_PLOT_SIZE {
        for x in origin_x..origin_x + MILITARY_SILO_PLOT_SIZE {
            if !city
                .zone_at(x, y)
                .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
            {
                pe_stamp_military_zone_tile_4144d4(city, x, y, ZONE_MILITARY);
            }
            pe_place_tile_441f00(city, x, y, MILITARY_SILO_BUILDING_ID);
            pe_place_xund_442370(city, x, y, UNDER_TILE_GREY_BOX_1);
        }
    }
    for y in origin_y..origin_y + MILITARY_SILO_PLOT_SIZE {
        for x in origin_x..origin_x + MILITARY_SILO_PLOT_SIZE {
            let corners = building_zone_bits_for_tile(
                origin_x,
                origin_y,
                x,
                y,
                MILITARY_SILO_PLOT_SIZE,
                corner_bits,
            );
            // PE @ `0x438e59`: `XZON &= 0x0f; XZON |= corner` (keep type 7).
            let zone_type = city
                .zone_at(x, y)
                .map(|z| z.raw_bits() & 0x0f)
                .unwrap_or(ZONE_MILITARY);
            city.set_zone(x, y, zone_type | corners);
        }
    }
}

/// PE DoArmyBaseGrowth silo origin search @ `0x438d6b`..`0x438ddd`: back north up to
/// two cells while the preceding XZON low nibble matches `zone_type`, then back west
/// up to two cells at the adjusted row. The original caller supplies the candidate
/// tile; unlike the other multi-tile growth arms, this is not an even-alignment rule.
pub(super) fn pe_army_missile_silo_origin_438d6b(
    city: &City,
    candidate_x: usize,
    candidate_y: usize,
    zone_type: u8,
) -> (usize, usize) {
    let mut x = candidate_x;
    let mut y = candidate_y;
    let matches_zone = |x: usize, y: usize| {
        city.zone_at(x, y)
            .is_some_and(|zone| zone.zone_id() == (zone_type & 0x0f))
    };
    for _ in 0..2 {
        if y == 0 || !matches_zone(x, y - 1) {
            break;
        }
        y -= 1;
    }
    for _ in 0..2 {
        if x == 0 || !matches_zone(x - 1, y) {
            break;
        }
        x -= 1;
    }
    (x, y)
}

/// PE FormArmy strip DE-upgrade flag @ `0x438407`–`0x438463`: same shape as crane
/// strip `or_xbit_02` — `(dx==0) == (rotation&1)`.
pub(super) fn pe_army_form_strip_de_flag(strip_dx_is_zero: bool, rotation: u8) -> bool {
    strip_dx_is_zero == (rotation & 1 != 0)
}

/// PE FormArmy after PlaceTile(0xDD) @ `0x4384b9`/`0x4384e5`/`0x438520`:
/// XZON`|=0xf0`; type!=7 → XBIT`|=0xc0`; strip flag → XBIT`|=0x02`.
pub(super) fn pe_army_after_runway_dd_zone_4384e6(
    city: &mut City,
    x: usize,
    y: usize,
    zone_type: u8,
    flag: bool,
) {
    let Some(zone) = city.zone_at(x, y) else {
        return;
    };
    let zone_bits = zone.raw_bits();
    city.set_zone(x, y, zone_bits | ZONE_CORNER_ALL);

    let mut xbit = city
        .tile_bits_at(x, y)
        .unwrap_or(TileBits::new(0))
        .raw_bits();
    if zone_type != ZONE_MILITARY {
        xbit |= 0xc0;
    }
    if flag {
        xbit |= 0x02;
    }
    city.set_tile_bits(x, y, TileBits::new(xbit));
}

/// PE FormArmy @ `0x438530`–`0x4385ff`: when XBLD is runway `0xDD` and
/// `(XBIT&0x02==0x02) != flag`, PlaceTile(`0xDE`) then XZON `|= 0xf0`;
/// when zone type `!= 7`, also XBIT `|= 0xc0`; then XBIT `&= ~0x02`.
pub(super) fn pe_army_runway_dd_to_de_43857b(
    city: &mut City,
    x: usize,
    y: usize,
    zone_type: u8,
    flag: bool,
) -> bool {
    if city.building_at(x, y) != RUNWAY_BUILDING_ID {
        return false;
    }
    let zone_bits = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    let xbit_before = city
        .tile_bits_at(x, y)
        .unwrap_or(TileBits::new(0))
        .raw_bits();
    if (xbit_before & 0x02 != 0) == flag {
        return false;
    }
    pe_place_tile_441f00(city, x, y, AIRPORT_RUNWAY_CROSS_BUILDING_ID);
    city.set_zone(x, y, zone_bits | ZONE_CORNER_ALL);
    let mut xbit = city
        .tile_bits_at(x, y)
        .unwrap_or(TileBits::new(0))
        .raw_bits();
    if zone_type != ZONE_MILITARY {
        xbit |= 0xc0;
    }
    xbit &= !0x02;
    city.set_tile_bits(x, y, TileBits::new(xbit));
    true
}

/// Win95 special-growth runway selector class 0 @ `0x43830d`–`0x438603`.
/// It chooses the positive cardinal ray from the candidate parity and city rotation,
/// preflights five matching-zone cells (existing DD/DE runway cells do not consume the
/// five-cell count), then lays DD runway or upgrades DD to DE according to the strip flag.
/// The growth caller ignores the selector's return, so refusal is a no-op with no generic
/// one-tile fallback.
pub(super) fn pe_growth_runway_class0_ray_43830d(
    city: &City,
    x: usize,
    y: usize,
    zone_type: u8,
) -> Option<(i16, i16, Vec<(usize, usize)>)> {
    let rotation = city.rotation() & 1;
    let (dx, dy) = if rotation != 0 {
        if y & 1 != 0 {
            (1i16, 0i16)
        } else if x & 1 != 0 {
            (0, 1)
        } else {
            return None;
        }
    } else if x & 1 != 0 {
        (0, 1)
    } else if y & 1 != 0 {
        (1, 0)
    } else {
        return None;
    };

    let mut ray = Vec::with_capacity(5);
    let (mut cx, mut cy) = (x as i16, y as i16);
    let mut ordinary = 0;
    while ordinary < 5 {
        if !(0..CITY_WIDTH as i16).contains(&cx) || !(0..CITY_HEIGHT as i16).contains(&cy) {
            return None;
        }
        let (tx, ty) = (cx as usize, cy as usize);
        if !city
            .zone_at(tx, ty)
            .is_some_and(|zone| zone.zone_id() == zone_type)
        {
            return None;
        }
        let building = city.building_at(tx, ty);
        ray.push((tx, ty));
        if building != RUNWAY_BUILDING_ID && building != AIRPORT_RUNWAY_CROSS_BUILDING_ID {
            ordinary += 1;
        }
        cx += dx;
        cy += dy;
    }
    Some((dx, dy, ray))
}

pub(super) fn pe_growth_runway_class0_43830d(
    city: &mut City,
    x: usize,
    y: usize,
    zone_type: u8,
) -> bool {
    let Some((dx, _dy, ray)) = pe_growth_runway_class0_ray_43830d(city, x, y, zone_type) else {
        return false;
    };

    let flag = pe_army_form_strip_de_flag(dx == 0, city.rotation());
    for (tx, ty) in ray {
        let building = city.building_at(tx, ty);
        if building == AIRPORT_RUNWAY_CROSS_BUILDING_ID {
            continue;
        }
        if building == RUNWAY_BUILDING_ID {
            pe_army_runway_dd_to_de_43857b(city, tx, ty, zone_type, flag);
            continue;
        }
        if building >= SMALL_PARK_BUILDING_ID {
            pe_fun_004392a0_clear_before_runway(city, tx, ty);
        }
        pe_place_tile_441f00(city, tx, ty, RUNWAY_BUILDING_ID);
        pe_army_after_runway_dd_zone_4384e6(city, tx, ty, zone_type, flag);
        pe_army_runway_dd_to_de_43857b(city, tx, ty, zone_type, flag);
    }
    true
}

/// PE army crane neighbor deltas `0x4e75c0` / `0x4e75c8` (S,E,N,W).
pub(super) const PE_ARMY_CRANE_STRIP_DX: [i16; 4] = [0, 1, 0, -1];
pub(super) const PE_ARMY_CRANE_STRIP_DY: [i16; 4] = [1, 0, -1, 0];

/// PE DoArmyBaseGrowth crane strip dir @ `0x43862d`–`0x43869f`: first in-bounds neighbor
/// whose XBIT has bit `0x04` set. Returns `None` when all
/// four neighbors refuse (`cx == 4`).
pub(super) fn pe_army_crane_select_strip_dir_43862d(
    city: &City,
    crane_x: usize,
    crane_y: usize,
) -> Option<usize> {
    for dir in 0..4usize {
        let nx = crane_x as i16 + PE_ARMY_CRANE_STRIP_DX[dir];
        let ny = crane_y as i16 + PE_ARMY_CRANE_STRIP_DY[dir];
        if !(0..0x80).contains(&nx) || !(0..0x80).contains(&ny) {
            continue;
        }
        let xbit = city
            .tile_bits_at(nx as usize, ny as usize)
            .map(|bits| bits.raw_bits())
            .unwrap_or(0);
        // PE @ `0x438662`–`0x438670`: `test byte [XBIT], 0x04` then take this dir.
        if xbit & 0x04 != 0 {
            return Some(dir);
        }
    }
    None
}

/// PE strip `or_xbit_02` @ `0x438854`–`0x438883`: SI from DX[dir] vs rotation word
/// `0x4c942c` bit0 (`city.rotation() & 1`).
pub(super) fn pe_army_crane_strip_or_xbit_02(dir: usize, rotation: u8) -> bool {
    let dx_is_zero = PE_ARMY_CRANE_STRIP_DX[dir & 3] == 0;
    let rot_odd = rotation & 1 != 0;
    dx_is_zero == rot_odd
}

/// PE army crane runway-cross strip @ `0x43888c`–`0x43890d`: four PlaceTile(`0xdf`)
/// steps along the chosen neighbor delta, each OR XZON `0xf0`; optional XBIT `|= 0x02`.
pub(super) fn pe_army_crane_place_runway_cross_strip_438890(
    city: &mut City,
    crane_x: usize,
    crane_y: usize,
    dir: usize,
    or_xbit_02: bool,
) {
    let dir = dir & 3;
    let mut x = crane_x as i16;
    let mut y = crane_y as i16;
    for _ in 0..4 {
        x += PE_ARMY_CRANE_STRIP_DX[dir];
        y += PE_ARMY_CRANE_STRIP_DY[dir];
        if !(0..0x80).contains(&x) || !(0..0x80).contains(&y) {
            continue;
        }
        let ux = x as usize;
        let uy = y as usize;
        pe_place_tile_441f00(city, ux, uy, 0xdf);
        let zone = city.zone_at(ux, uy).map(|z| z.raw_bits()).unwrap_or(0);
        city.set_zone(ux, uy, zone | ZONE_CORNER_ALL);
        if or_xbit_02 {
            if let Some(bits) = city.tile_bits_at(ux, uy) {
                city.set_tile_bits(ux, uy, TileBits::new(bits.raw_bits() | 0x02));
            }
        }
    }
}

/// PE army PlaceTile-size arm @ `0x43892c`–`0x4389b6`: when XBLD `< 0x0D`, call
/// PlaceTile-size then always write type-nibble XZON / type-7 XBIT `&= 0x0f`.
/// Growth uses SimulationGrow wrapper type-7 allow (`0x4376b0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PeArmySize1Result {
    /// The selector reaches its shared success return without calling PlaceTile-size.
    NoopXbldAtOrAbove0x0d,
    /// The selector called PlaceTile-size, then performed its zone/XBIT epilogue.
    Accepted { placed: bool },
}

pub(super) fn pe_army_selector_owns_footprint(building: u8) -> bool {
    building == MILITARY_CRANE_BUILDING_ID
        || building == RUNWAY_BUILDING_ID
        || matches!(building, 0xe1..=0xe8 | 0xea)
}

pub(super) fn pe_army_place_tile_size_if_xbld_below_0x0d_43893f(
    city: &mut City,
    x: usize,
    y: usize,
    building: u8,
    zone_type: u8,
) -> PeArmySize1Result {
    if city.building_at(x, y) >= 0x0d {
        return PeArmySize1Result::NoopXbldAtOrAbove0x0d;
    }
    let placed = pe_place_tile_size_440c50_ex(city, x, y, building, 1, true);
    pe_army_crane_write_zone_after_place(city, x, y, zone_type);
    PeArmySize1Result::Accepted { placed }
}

/// PE army 2×2 even-align probe result @ `0x4389bf`..`0x438b1c`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PeArmy2x2Probe {
    Refuse,
    Ready { origin_x: usize, origin_y: usize },
}

/// PE class 4/5 returns zero only for the explicit probe gates. Once those pass,
/// it returns one after the zone epilogue even when PlaceTile-size refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PeArmy2x2Result {
    Refused,
    Accepted { placed: bool },
}

/// PE army 2×2 probe @ `0x4389bf`: even-align; NW XBLD `>=0xEB` refuse; any `0xDD`/`0xDE`/
/// `0xE0` refuse (`0x438d4d`/`0x438d57`/`0x438d61` return 0); all four zone type nibbles
/// must match `zone_type`.
pub(super) fn pe_army_2x2_place_probe_4389bf(
    city: &City,
    x: usize,
    y: usize,
    zone_type: u8,
) -> PeArmy2x2Probe {
    let origin_x = x & !1;
    let origin_y = y & !1;
    if origin_x + 1 >= CITY_WIDTH || origin_y + 1 >= CITY_HEIGHT {
        return PeArmy2x2Probe::Refuse;
    }
    if city.building_at(origin_x, origin_y) >= 0xeb {
        return PeArmy2x2Probe::Refuse;
    }
    let tiles = [
        (origin_x, origin_y),
        (origin_x + 1, origin_y),
        (origin_x, origin_y + 1),
        (origin_x + 1, origin_y + 1),
    ];
    for &(tx, ty) in &tiles {
        // PE @ `0x4389f6`..`0x438a9d`: existing crane/runway-cross refuse (return 0).
        if matches!(city.building_at(tx, ty), 0xdd | 0xde | 0xe0) {
            return PeArmy2x2Probe::Refuse;
        }
    }
    let want = zone_type & 0x0f;
    for &(tx, ty) in &tiles {
        let got = city
            .zone_at(tx, ty)
            .map(|z| z.raw_bits() & 0x0f)
            .unwrap_or(0);
        if got != want {
            return PeArmy2x2Probe::Refuse;
        }
    }
    PeArmy2x2Probe::Ready { origin_x, origin_y }
}

/// PE army 2×2 clear @ `0x438b1c`..: per footprint tile, XBLD `>=0x0D` → `FUN_004392A0`.
pub(super) fn pe_army_2x2_clear_before_place_438b1c(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
) {
    for dx in 0..2usize {
        for dy in 0..2usize {
            let tx = origin_x + dx;
            let ty = origin_y + dy;
            if tx >= CITY_WIDTH || ty >= CITY_HEIGHT {
                continue;
            }
            if city.building_at(tx, ty) >= 0x0d {
                pe_fun_004392a0_clear_before_runway(city, tx, ty);
            }
        }
    }
}

/// PE army 2×2 PlaceTile-size @ `0x438b8e` then per-tile type-nibble XZON write
/// (`0x438bb0`..). Growth uses SimulationGrow wrapper type-7 allow (`0x4376b0`).
/// Mirrors the selector result, which is distinct from PlaceTile-size's result.
pub(super) fn pe_army_2x2_place_tile_size_438b8e(
    city: &mut City,
    x: usize,
    y: usize,
    building: u8,
    zone_type: u8,
) -> PeArmy2x2Result {
    let PeArmy2x2Probe::Ready { origin_x, origin_y } =
        pe_army_2x2_place_probe_4389bf(city, x, y, zone_type)
    else {
        return PeArmy2x2Result::Refused;
    };
    pe_army_2x2_clear_before_place_438b1c(city, origin_x, origin_y);
    let placed = pe_place_tile_size_440c50_ex(city, origin_x, origin_y, building, 2, true);
    for dx in 0..2usize {
        for dy in 0..2usize {
            let tx = origin_x + dx;
            let ty = origin_y + dy;
            if tx >= CITY_WIDTH || ty >= CITY_HEIGHT {
                continue;
            }
            pe_army_crane_write_zone_after_place(city, tx, ty, zone_type);
        }
    }
    PeArmy2x2Result::Accepted { placed }
}
