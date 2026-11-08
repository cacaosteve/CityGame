//! Win95 highway footprint classification and route refusal gates.
//!
//! The functions classify saved tile bytes and elevation/terrain state only;
//! route selection and tile writes remain in their respective workers.

use super::super::NetworkAxis;
use crate::city::City;

pub(in crate::app) fn pe_highway_altitude_463f60(city: &City, x: usize, y: usize) -> u16 {
    let mut best = 0u16;
    for dy in 0..2 {
        for dx in 0..2 {
            let tx = x + dx;
            let ty = y + dy;
            if tx >= 0x80 || ty >= 0x80 {
                continue;
            }
            let mut level = (city.altitude_at(tx, ty) as u16) & 0x1f;
            if city.terrain_at(tx, ty) != 0 {
                level += 1;
            }
            best = best.max(level);
        }
    }
    best
}

pub(in crate::app) fn pe_highway_neighbor_low_bit_462990(
    city: &City,
    x: isize,
    y: isize,
    ns: bool,
) -> bool {
    let classified = pe_highway_classify_463c40(city, x, y);
    if ns {
        if matches!(classified, 1 | 3) {
            return true;
        }
        // PE `0x4629d8`: class 2 connects unless `FUN_00464040` returns `0x4000`.
        if classified == 2
            && (x < 0
                || y < 0
                || !pe_highway_gate_returns_4000_4642e7(city, x as usize, y as usize))
        {
            return true;
        }
    } else if matches!(classified, 0 | 2)
        || (classified == 3
            && (x < 0
                || y < 0
                || !pe_highway_gate_returns_4000_4642e7(city, x as usize, y as usize)))
    {
        // PE `0x462a5a`: class 3 connects unless the gate returns `0x4000`.
        return true;
    }
    if classified < 4 {
        return false;
    }
    const NS_4E89D8: [u32; 17] = [0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0];
    const EW_4E8990: [u32; 17] = [0, 0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1];
    let table = if ns { &NS_4E89D8 } else { &EW_4E8990 };
    table
        .get(classified as usize)
        .is_some_and(|flag| *flag != 0)
}

pub(in crate::app) fn pe_get_highway_tile_classify_refuse_462956(classified: i16) -> bool {
    classified == 0 || classified == 1 || (4..=7).contains(&classified) || classified >= 0x0d
}

pub(in crate::app) fn pe_get_highway_tile_462910(
    city: &City,
    x: usize,
    y: usize,
    axis: NetworkAxis,
) -> i16 {
    if x >= 0x80 || y >= 0x80 {
        return -1;
    }
    if pe_get_highway_tile_classify_refuse_462956(pe_highway_classify_463c40(
        city, x as isize, y as isize,
    )) {
        return -1;
    }
    // North `0x4629a2` plus altitude high bits `0x4629f1`–`0x462a22`.
    let classified = pe_highway_classify_463c40(city, x as isize, y as isize);
    let mut mask = 0u8;
    if y > 1 && pe_highway_neighbor_low_bit_462990(city, x as isize, y as isize - 2, true) {
        mask |= 1;
        let north_class = pe_highway_classify_463c40(city, x as isize, y as isize - 2);
        let origin_alt = pe_highway_altitude_463f60(city, x, y);
        let north_alt = pe_highway_altitude_463f60(city, x, y - 2);
        if north_alt > origin_alt && north_class != 5 {
            mask = 0x11;
        } else if north_alt < origin_alt || (north_alt == origin_alt && north_class == 5) {
            mask |= 0x40;
        }
    }
    // East `0x462a25`: x < 0x7e and classify(x+2, y) is 0 or 2 → bit 2.
    if x < 0x7e && pe_highway_neighbor_low_bit_462990(city, x as isize + 2, y as isize, false) {
        mask |= 2;
        let east_class = pe_highway_classify_463c40(city, x as isize + 2, y as isize);
        let origin_alt = pe_highway_altitude_463f60(city, x, y);
        let east_alt = pe_highway_altitude_463f60(city, x + 2, y);
        // PE `0x462a83`–`0x462aa2`: higher and class != 6 → bit `0x20`; lower, or
        // equal class 6, → bit `0x80`.
        if east_alt > origin_alt && east_class != 6 {
            mask |= 0x20;
        } else if east_alt < origin_alt || (east_alt == origin_alt && east_class == 6) {
            mask |= 0x80;
        }
    }
    // South `0x462aa5`: y < 0x7e and classify(x, y+2) is 1 or 3 → bit 4.
    if y < 0x7e && pe_highway_neighbor_low_bit_462990(city, x as isize, y as isize + 2, true) {
        mask |= 4;
        let south_class = pe_highway_classify_463c40(city, x as isize, y as isize + 2);
        let origin_alt = pe_highway_altitude_463f60(city, x, y);
        let south_alt = pe_highway_altitude_463f60(city, x, y + 2);
        // PE `0x462b04`–`0x462b23`: higher and class != 7 → `0x40`; else `0x10`.
        if south_alt > origin_alt && south_class != 7 {
            mask |= 0x40;
        } else if south_alt < origin_alt || (south_alt == origin_alt && south_class == 7) {
            mask |= 0x10;
        }
    }
    // West `0x462b26`: x > 1 and classify(x-2, y) is 0 or 2 → bit 8.
    if x > 1 && pe_highway_neighbor_low_bit_462990(city, x as isize - 2, y as isize, false) {
        mask |= 8;
        let west_class = pe_highway_classify_463c40(city, x as isize - 2, y as isize);
        let origin_alt = pe_highway_altitude_463f60(city, x, y);
        let west_alt = pe_highway_altitude_463f60(city, x - 2, y);
        // PE `0x462b84`–`0x462ba3`: higher and class != 4 → `0x80`; else `0x20`.
        if west_alt > origin_alt && west_class != 4 {
            mask |= 0x80;
        } else if west_alt < origin_alt || (west_alt == origin_alt && west_class == 4) {
            mask |= 0x20;
        }
    }
    // PE `0x462ba6`–`0x462bda`: XTXT (`0x4ca600`) `0xfa` forces map-edge low bits.
    if city.text_overlay_at(x, y) == Some(0xfa) {
        if x < 2 {
            mask |= 8;
        }
        if x >= 0x7e {
            mask |= 2;
        }
        if y < 2 {
            mask |= 1;
        }
        if y >= 0x7e {
            mask |= 4;
        }
    }
    // Origin class 2/3 with a single cardinal bit (`0x462bf4`–`0x462d8c`).
    if matches!(classified, 2 | 3) {
        if mask == 1 || mask == 4 {
            return 2;
        }
        if mask == 2 || mask == 8 {
            return 3;
        }
        if pe_highway_gate_returns_4000_4642e7(city, x, y) {
            return -1;
        }
    }
    let gate_8000 = pe_highway_gate_early_8000_4640a1(city, x, y);
    let gate_4000 = pe_highway_gate_returns_4000_4642e7(city, x, y);
    let gate_si0 = pe_highway_gate_si0_fallthrough_464310(city, x, y);
    // `0x4000` has a zero low byte, so it does not enable class 4–7 returns.
    let gate_bit = |bit: u8| {
        gate_si0.is_some_and(|al| al & bit != 0) || (gate_si0.is_none() && !gate_8000 && !gate_4000)
    };
    if gate_bit(2) && mask & 0x10 != 0 {
        // North-high `0x10` returns class 5 only if gate bit 1 is set (`0x462c9b`).
        return 5;
    }
    if gate_bit(8) && mask & 0x40 != 0 {
        // North-low `0x40` returns class 7 only if gate bit 3 is set (`0x462cb0`).
        return 7;
    }
    if gate_bit(1) && mask & 0x80 != 0 {
        // East-low `0x80` returns class 4 only if gate bit 0 is set (`0x462cc5`).
        return 4;
    }
    if gate_bit(4) && mask & 0x20 != 0 {
        // East-high `0x20` returns class 6 only if gate bit 2 is set (`0x462cda`).
        return 6;
    }
    if let Some(al) = gate_si0 {
        // PE `0x462cef`–`0x462d48`: low neighbor bits plus a non-`0xf` gate.
        if mask & 1 != 0 && al & 2 != 0 {
            return 5;
        }
        if mask & 4 != 0 && al & 8 != 0 {
            return 7;
        }
        if mask & 8 != 0 && al & 1 != 0 {
            return 4;
        }
        if mask & 2 != 0 && al & 4 != 0 {
            return 6;
        }
    }
    if mask != 0 {
        // Gate `0xf` uses `0x4e8950` @ `0x462d4f`. Anything else, including an
        // early `0x8000` and the equal-altitude fallthrough, uses `0x4e8970`.
        const PE_HWY_MASK_CLASS_4E8950: [i16; 16] =
            [2, 2, 3, 8, 2, 2, 9, 0xc, 3, 0xb, 3, 0xc, 0xa, 0xc, 0xc, 0xc];
        const PE_HWY_MASK_CLASS_4E8970: [i16; 16] =
            [2, 2, 3, 3, 2, 2, 3, 2, 3, 3, 3, 2, 3, 2, 2, 2];
        let table = if gate_8000 || gate_4000 || gate_si0.is_some() {
            &PE_HWY_MASK_CLASS_4E8970
        } else {
            &PE_HWY_MASK_CLASS_4E8950
        };
        return table[(mask & 0xf) as usize];
    }
    if let Some(al) = gate_si0 {
        // PE `0x462c5b`–`0x462c8f`: empty mask and gate != `0xf`.
        if al & 2 != 0 {
            return 5;
        }
        if al & 8 != 0 {
            return 7;
        }
        if al & 1 != 0 {
            return 4;
        }
        if al & 4 != 0 {
            return 6;
        }
        return 2;
    }
    if gate_8000 || gate_4000 {
        return 2;
    }
    // PE `0x462c3f`–`0x462c5a`: mask 0 and gate `0xf` returns `2+([0x4c7d60]&1)`.
    // The axis argument is that orientation stand-in, not the tile's building id.
    match axis {
        NetworkAxis::GridY => 2,
        NetworkAxis::GridX => 3,
    }
}

/// PE `FUN_00463c40` (thunk `0x4014c9`) highway classify for ±2 reshape gates:
/// OOB→0; non-road/hwy→`-1`; missing corners → `xbld-0x5d` when `< 0x0d`;
/// 2×2 crossover / XBIT bit4, then SW `0x49`/`0x4a` → 2/3.
pub(in crate::app) fn pe_highway_classify_463c40(city: &City, x: isize, y: isize) -> i16 {
    if !(0..0x80).contains(&x) || !(0..0x80).contains(&y) {
        return 0;
    }
    let (x, y) = (x as usize, y as usize);
    let xbld = city.building_at(x, y);
    if !(0x49..=0x50).contains(&xbld) && !(0x61..=0x6b).contains(&xbld) {
        return -1;
    }
    let zone = city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
    if zone & 0xf0 != 0xf0 {
        // PE `0x463e5f`–`0x463e67`: missing corners return `xbld-0x5d` when that is `< 0x0d`.
        // PE `0x463e69`–`0x463e9e`: `0x6a`/`0x6b` missing corners. XBIT bit `0x02`
        // on `(x, y+1)` → `0x10`, else (including OOB) `0x0f`.
        let delta = xbld as i16 - 0x5d;
        if delta < 0x0d {
            return delta;
        }
        if y + 1 < 0x80
            && city
                .tile_bits_at(x, y + 1)
                .is_some_and(|bits| bits.raw_bits() & 0x02 != 0)
        {
            return 0x10;
        }
        return 0x0f;
    }
    // PE scans the 2×2 as NW/NE/SE/SW (`0x463cc7`, `0x463d12`, `0x463d80`, `0x463de4`).
    // Crossover `0x4b..=0x50` → `xbld&1`. XBIT bit `0x04` → `0x0d` iff that tile is
    // `0x49`, else `0x0e`. Final class 2/3 is the SW tile (`x`, `y+1`) at `0x463e3d`,
    // not the origin, else `-1`.
    let tiles = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
    for (i, &(tx, ty)) in tiles.iter().enumerate() {
        if let Some(class) = pe_highway_classify_footprint_arm_463cc7(city, tx, ty) {
            return class;
        }
        if i + 1 == tiles.len() {
            if !(0..0x80).contains(&tx) || !(0..0x80).contains(&ty) {
                return -1;
            }
            let last = city.building_at(tx, ty);
            if (0x49..0x4b).contains(&last) {
                return 2 + (last & 1) as i16;
            }
            return -1;
        }
    }
    -1
}

/// One footprint tile of `FUN_00463c40`: crossover parity, else XBIT bit `0x04` class.
pub(in crate::app) fn pe_highway_classify_footprint_arm_463cc7(
    city: &City,
    x: usize,
    y: usize,
) -> Option<i16> {
    if !(0..0x80).contains(&x) || !(0..0x80).contains(&y) {
        return None;
    }
    let xbld = city.building_at(x, y);
    if (0x4b..=0x50).contains(&xbld) {
        return Some((xbld & 1) as i16);
    }
    if city
        .tile_bits_at(x, y)
        .is_some_and(|bits| bits.raw_bits() & 0x04 != 0)
    {
        return Some(if xbld == 0x49 { 0x0d } else { 0x0e });
    }
    None
}

/// PE `FUN_00464040` early `0x8000` @ `0x4640a1`–`0x4641eb`: any 2×2 tile that
/// is not vacant/highway (`0x49..=0x50`, `0x61..=0x6b`, `0x2c`/`0x2d`, or
/// other low ids except `5` and `0x0d`) makes the gate return `0x8000`.
pub(in crate::app) fn pe_highway_gate_early_8000_4640a1(city: &City, x: usize, y: usize) -> bool {
    for dy in 0..2 {
        for dx in 0..2 {
            let tx = x + dx;
            let ty = y + dy;
            if tx >= 0x80 || ty >= 0x80 {
                continue;
            }
            if pe_highway_gate_tile_is_8000(city.building_at(tx, ty)) {
                return true;
            }
        }
    }
    pe_highway_gate_slope_nw_se_8000_464224(city, x, y)
}

/// PE `0x464224`–`0x464270`: slope-category tiles return `0x8000` when the
/// opposite corner is lower or higher. The fallthrough after `0x46427e` is the
/// min-altitude `0x4000` arm, not another slope pair.
pub(in crate::app) fn pe_highway_gate_slope_nw_se_8000_464224(
    city: &City,
    x: usize,
    y: usize,
) -> bool {
    if x + 1 >= 0x80 || y + 1 >= 0x80 {
        return false;
    }
    let nw = (city.altitude_at(x, y) as u16) & 0x1f;
    let ne = (city.altitude_at(x + 1, y) as u16) & 0x1f;
    let se = (city.altitude_at(x + 1, y + 1) as u16) & 0x1f;
    let sw = (city.altitude_at(x, y + 1) as u16) & 0x1f;
    let nw_slope = pe_highway_xter_slope_category(city.terrain_at(x, y)) != 0;
    let ne_slope = pe_highway_xter_slope_category(city.terrain_at(x + 1, y)) != 0;
    let se_slope = pe_highway_xter_slope_category(city.terrain_at(x + 1, y + 1)) != 0;
    let sw_slope = pe_highway_xter_slope_category(city.terrain_at(x, y + 1)) != 0;
    (se < nw && nw_slope) || (se > nw && se_slope) || (sw < ne && ne_slope) || (sw > ne && sw_slope)
}

/// PE `0x4642a5`–`0x4642e7`: tiles at the footprint minimum must be exactly the
/// category-4 (`XTER` table `0x4e8a58` == 4) tiles, otherwise that arm is not
/// `0x4000`. Equal altitudes with NW `XTER == 0x0d` still return `0x4000` at
/// `0x464304`. Earlier `0x8000` returns never reach either arm.
pub(in crate::app) fn pe_highway_gate_returns_4000_4642e7(city: &City, x: usize, y: usize) -> bool {
    if pe_highway_gate_early_8000_4640a1(city, x, y) || x + 1 >= 0x80 || y + 1 >= 0x80 {
        return false;
    }
    let tiles = [(x, y, 1u8), (x + 1, y, 2), (x + 1, y + 1, 4), (x, y + 1, 8)];
    let mut min_alt = u16::MAX;
    let mut alts = [0u16; 4];
    let mut cat4 = 0u8;
    for (i, &(tx, ty, bit)) in tiles.iter().enumerate() {
        let alt = (city.altitude_at(tx, ty) as u16) & 0x1f;
        alts[i] = alt;
        min_alt = min_alt.min(alt);
        if pe_highway_xter_slope_category(city.terrain_at(tx, ty)) == 4 {
            cat4 |= bit;
        }
    }
    if cat4 == 0 {
        return false;
    }
    let mut at_min = 0u8;
    for (i, &(_, _, bit)) in tiles.iter().enumerate() {
        if alts[i] == min_alt {
            at_min |= bit;
        }
    }
    if at_min == cat4 {
        return true;
    }
    // PE `0x4642f3`–`0x464304`: `si == 0` (every corner altitude equals the
    // minimum) and the NW `XTER` byte is exactly `0x0d` still returns `0x4000`.
    at_min == 0x0f && city.terrain_at(x, y) == 0x0d
}

/// PE `0x464310`–`0x464439` after the `0xf` / `0x8000` / `0x4000` arms miss.
/// Equal altitudes use the `dx` equality mask, then fall through to the
/// category-3 ORs. Unequal altitudes (`si != 0`) skip the `dx` setup and OR
/// bits from `0x464362`. `None` means the caller keeps a `0xf` gate.
pub(in crate::app) fn pe_highway_gate_si0_fallthrough_464310(
    city: &City,
    x: usize,
    y: usize,
) -> Option<u8> {
    if pe_highway_gate_early_8000_4640a1(city, x, y)
        || pe_highway_gate_returns_4000_4642e7(city, x, y)
        || x + 1 >= 0x80
        || y + 1 >= 0x80
    {
        return None;
    }
    let tiles = [(x, y, 1u8), (x + 1, y, 2), (x + 1, y + 1, 4), (x, y + 1, 8)];
    let alts = tiles.map(|(tx, ty, _)| (city.altitude_at(tx, ty) as u16) & 0x1f);
    let mut cat = [0u8; 5];
    for (tx, ty, bit) in tiles {
        let category = pe_highway_xter_slope_category(city.terrain_at(tx, ty)) as usize;
        if category < cat.len() {
            cat[category] |= bit;
        }
    }
    if cat[1] | cat[2] | cat[3] | cat[4] == 0 {
        return None;
    }
    let dx = cat[1] | cat[3];
    let equal = alts.iter().all(|alt| *alt == alts[0]);
    let mut al = 0u8;
    if equal {
        // PE `0x464310`–`0x464360`, then fall through. `si == 0` makes the
        // altitude ORs no-ops, but the category-3 ORs at `0x4643db` still run.
        if dx == 8 || dx == 1 || dx == 9 {
            al = 1;
        }
        if dx == 1 || dx == 2 || dx == 3 {
            al |= 2;
        }
        if dx == 2 || dx == 4 || dx == 6 {
            al |= 4;
        }
        if dx == 4 || dx == 8 || dx == 0x0c {
            al |= 8;
        }
    } else {
        // PE `0x4642f9` jumps to `0x464362` when `si != 0`, skipping the `dx` setup.
        let min_alt = alts.iter().copied().min().unwrap_or(0);
        let mut si = 0u8;
        for (i, bit) in [1u8, 2, 4, 8].into_iter().enumerate() {
            if alts[i] > min_alt {
                si |= bit;
            }
        }
        if matches!(si, 1 | 8 | 9) && dx & 6 == 6 {
            al |= 1;
        }
        if matches!(si, 1 | 2 | 3) && dx & 0x0c == 0x0c {
            al |= 2;
        }
        if matches!(si, 2 | 4 | 6) && dx & 9 == 9 {
            al |= 4;
        }
        if matches!(si, 4 | 8 | 0x0c) && dx & 3 == 3 {
            al |= 8;
        }
    }
    let cx = cat[1] | cat[2] | cat[3] | cat[4];
    let cat3 = cat[3];
    if cx == 7 {
        if cat3 & 1 != 0 {
            al |= 4;
        }
        if cat3 & 4 != 0 {
            al |= 2;
        }
    }
    if cx == 0x0b {
        if cat3 & 8 != 0 {
            al |= 2;
        }
        if cat3 & 2 != 0 {
            al |= 1;
        }
    }
    if cx == 0x0d {
        if cat3 & 4 != 0 {
            al |= 1;
        }
        if cat3 & 1 != 0 {
            al |= 8;
        }
    }
    if cx == 0x0e {
        if cat3 & 8 != 0 {
            al |= 4;
        }
        if cat3 & 2 != 0 {
            al |= 8;
        }
    }
    Some(al)
}

/// Low WORD returned by the Win95 highway route gate `FUN_00464040`.
///
/// The caller distinguishes only `0x8000`, `0x000f`, and all other results.
/// A slope-derived direction mask may itself equal `0x000f`, so preserve the
/// assembled mask rather than treating “has a slope category” as a separate
/// caller branch.
pub(in crate::app) fn pe_highway_route_gate_low_word_464040(
    city: &City,
    x: usize,
    y: usize,
) -> u16 {
    if pe_highway_gate_early_8000_4640a1(city, x, y) {
        return 0x8000;
    }
    if pe_highway_gate_returns_4000_4642e7(city, x, y) {
        return 0x4000;
    }
    pe_highway_gate_si0_fallthrough_464310(city, x, y)
        .map(u16::from)
        .unwrap_or(0x000f)
}

pub(in crate::app) fn pe_highway_xter_slope_category(xter: u8) -> u8 {
    // PE dword table `0x4e8a58` indices `0..=0x26` (the dumped nonzero prefix).
    const PE_XTER_SLOPE_4E8A58: [u8; 39] = [
        0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 1, 1, 1, 1, 1, 1,
    ];
    PE_XTER_SLOPE_4E8A58
        .get(xter as usize)
        .copied()
        .unwrap_or(0)
}

pub(in crate::app) fn pe_highway_gate_tile_is_8000(xbld: u8) -> bool {
    xbld == 5
        || xbld == 0x0d
        || (0x1f..=0x2b).contains(&xbld)
        || (0x2e..=0x48).contains(&xbld)
        || (xbld >= 0x51 && !(0x61..=0x6b).contains(&xbld))
}
