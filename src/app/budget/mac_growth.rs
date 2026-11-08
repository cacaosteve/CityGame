//! Mac CODE 8 growth branching, placement patterns, and transit-conditioned growth.
//!
//! Growth thresholds, footprint selection, and RNG consumption stay in PE
//! caller order; this module is only a code-ownership boundary.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum MacGrowthBranch {
    None,
    ClassZero { orientation: bool },
    Church,
    Pattern,
    Generic,
}

pub(in crate::app) fn mac_growth_counter_slot(
    class: u8,
    coefficient: u16,
    category: usize,
) -> Option<usize> {
    match class {
        0 if coefficient as i16 > 0 => Some(category),
        2 if coefficient != 0 => Some(7),
        _ => None,
    }
}

/// CODE 8 generic gates and export 242 selector zero read the same XVAL
/// row table (`A5-0x16c0`), produced by CODE 14 at 0x28340. Native x/2,y/2
/// indexes its 64x64 grid; CityGame expands that grid over 2x2 map tiles.
/// This is land value, independent of the shared XTRF road-traffic path.
pub(in crate::app) fn mac_budget_growth_land_value_at(city: &City, x: usize, y: usize) -> u8 {
    city.land_value_at(x & !1, y & !1).unwrap_or(0)
}

/// CODE 8 `export_241 @ 0x36584..0x36748`. Accepted class 0/1 branches
/// jump directly to the tile tail. Only rejection can fall through to the
/// generic arm; class 2 always exits. local_c and 4000-local_c are signed
/// WORDs, while Random is zero-extended before the signed LONG comparison.
pub(in crate::app) fn mac_budget_choose_growth_branch(
    class: u8,
    coefficient: u16,
    category: usize,
    local_c: i16,
    land_value: u8,
    church_gate: bool,
    rng: &mut MacToolboxRng,
) -> MacGrowthBranch {
    if class == 0 && coefficient as i16 > 0 {
        let threshold = i32::from(4000_i16.wrapping_sub(local_c)) / i32::from(coefficient as i16);
        if i32::from(rng.random_u16()) < threshold {
            return MacGrowthBranch::ClassZero {
                orientation: rng.random_bit(),
            };
        }
    }
    // Zero is not a reachable class-1/2 table coefficient. Keep malformed
    // caller states bounded rather than emulating a native divide-by-zero trap.
    if class == 1 && coefficient != 0 && rng.random_u16() < 0x4000 / coefficient {
        return if church_gate && coefficient & 2 != 0 && category < 3 {
            MacGrowthBranch::Church
        } else {
            MacGrowthBranch::Pattern
        };
    }
    if class == 2 {
        return if coefficient != 0
            && i32::from(rng.random_u16()) < i32::from(local_c) * 15 / i32::from(coefficient as i16)
        {
            MacGrowthBranch::Pattern
        } else {
            MacGrowthBranch::None
        };
    }
    mac_budget_choose_generic_growth(coefficient, category, local_c, land_value, rng)
}

pub(in crate::app) fn mac_budget_choose_generic_growth(
    coefficient: u16,
    category: usize,
    local_c: i16,
    land_value: u8,
    rng: &mut MacToolboxRng,
) -> MacGrowthBranch {
    if coefficient == 4 || (category & 1 != 0 && coefficient as i16 > 0) {
        return MacGrowthBranch::None;
    }
    if category < 5 {
        let minimum = match coefficient {
            1 => 0x20,
            2 => 0x60,
            3 => 0xc0,
            _ => 0,
        };
        if land_value < minimum {
            return MacGrowthBranch::None;
        }
    }
    let denominator = i32::from(coefficient as i16) + 1;
    if denominator == 0 {
        return MacGrowthBranch::None;
    }
    // Ordinary selector-table zeroes reach this draw too, not only the
    // early low-XBLD/nearby-transit override.
    if i32::from(rng.random_u16()) < i32::from(local_c) * 3 / denominator {
        MacGrowthBranch::Generic
    } else {
        MacGrowthBranch::None
    }
}

/// Mac `FUN_00036f96`: unpowered placement is permitted, XZON is cleared,
/// XBIT is ORed with e0, and only the four rotation corners are then marked.
pub(in crate::app) fn mac_budget_place_church(city: &mut City, x: usize, y: usize) {
    if !(1..=126).contains(&x) || !(1..=126).contains(&y) {
        return;
    }
    for tx in x..=x + 1 {
        for ty in y - 1..=y {
            city.set_building(tx, ty, 0xf7);
            city.set_zone(tx, ty, 0);
            let bits = city.tile_bits_at(tx, ty).unwrap_or_default().raw_bits();
            city.set_tile_bits(tx, ty, TileBits::new(bits | 0xe0));
        }
    }
    const CORNERS: [[u8; 4]; 4] = [
        [0x10, 0x20, 0x40, 0x80],
        [0x20, 0x40, 0x80, 0x10],
        [0x40, 0x80, 0x10, 0x20],
        [0x80, 0x10, 0x20, 0x40],
    ];
    let corners = CORNERS[(city.saved_rotation() & 3) as usize];
    for (tx, ty, marker) in [
        (x, y - 1, corners[0]),
        (x + 1, y - 1, corners[1]),
        (x + 1, y, corners[2]),
        (x, y, corners[3]),
    ] {
        city.set_zone(tx, ty, marker);
    }
}

/// CODE 8 `export_242` single-cell/square placement contracts for class-0 and
/// class-1 callers. The A5 selector bands are direct big-endian data from the
/// recovered Mac DATA/ZERO image; each draw precedes the single/square bounds
/// check.
pub(in crate::app) fn mac_budget_place_growth_pattern(
    city: &mut City,
    x: usize,
    y: usize,
    coefficient: u16,
    selector: usize,
    rng: &mut MacToolboxRng,
) {
    const SQUARE_BASES: [[u8; 4]; 5] = [
        [0x70, 0x8c, 0x90, 0xae],
        [0x7c, 0x94, 0x99, 0xb2],
        [0x84, 0x9e, 0xa2, 0xbc],
        [0x88, 0xa6, 0xa8, 0xc2],
        [0x8a, 0xaa, 0xac, 0xc4],
    ];
    const SQUARE_COUNTS: [[u8; 4]; 5] = [
        [12, 4, 4, 4],
        [8, 5, 5, 10],
        [4, 4, 4, 6],
        [2, 2, 2, 2],
        [2, 2, 2, 2],
    ];
    const SINGLE_BASES: [u8; 5] = [0, 0x7c, 0x84, 0x88, 0x8a];
    const SINGLE_COUNTS: [u8; 5] = [0, 8, 4, 2, 2];
    const CORNER_MARKERS: [[u8; 4]; 4] = [
        [0x10, 0x20, 0x40, 0x80],
        [0x20, 0x40, 0x80, 0x10],
        [0x40, 0x80, 0x10, 0x20],
        [0x80, 0x10, 0x20, 0x40],
    ];

    let draw = rng.random_i16() as u16;
    if coefficient == 1 {
        let tile = if selector == 0 {
            let band = mac_budget_growth_land_value_at(city, x, y) / 0x40;
            0x70 + band.min(2) * 4 + (draw as u8 & 3)
        } else {
            let Some(base) = SINGLE_BASES.get(selector).copied() else {
                return;
            };
            let Some(count) = SINGLE_COUNTS
                .get(selector)
                .copied()
                .filter(|count| *count != 0)
            else {
                return;
            };
            base + (draw % u16::from(count)) as u8
        };
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return;
        }
        city.set_building(x, y, tile);
        let zone_low = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits() & 0x0f);
        city.set_zone(x, y, zone_low | 0xf0);
        let bits = city.tile_bits_at(x, y).unwrap_or_default();
        city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0xe0));
        return;
    }

    let Some(coefficient_index) = coefficient.checked_sub(1).map(usize::from) else {
        return;
    };
    let (Some(base_row), Some(count_row)) =
        (SQUARE_BASES.get(selector), SQUARE_COUNTS.get(selector))
    else {
        return;
    };
    let (Some(base), Some(count)) = (
        base_row.get(coefficient_index).copied(),
        count_row
            .get(coefficient_index)
            .copied()
            .filter(|count| *count != 0),
    ) else {
        return;
    };
    let tile = base + (draw % u16::from(count)) as u8;
    // Native export 242 uses a square side of `coefficient / 2 + 1`:
    // coefficients 2/3 stamp 2x2 cells and coefficient 4 stamps 3x3.
    let extent = usize::from(coefficient / 2);
    if x <= 1 || y <= 1 || x > 0x7e - extent || y > 0x7e - extent {
        return;
    }

    let left = x;
    let top = y - extent;
    for tx in left..=left + extent {
        for ty in top..=top + extent {
            city.set_building(tx, ty, tile);
            let zone_low = city
                .zone_at(tx, ty)
                .map_or(0, |zone| zone.raw_bits() & 0x0f);
            city.set_zone(tx, ty, zone_low);
            let bits = city.tile_bits_at(tx, ty).unwrap_or_default();
            city.set_tile_bits(tx, ty, TileBits::new(bits.raw_bits() | 0xe0));
        }
    }

    let corners = CORNER_MARKERS[(city.saved_rotation() & 3) as usize];
    for (cx, cy, marker) in [
        (left, top, corners[0]),
        (left + extent, top, corners[1]),
        (left + extent, top + extent, corners[2]),
        (left, top + extent, corners[3]),
    ] {
        let zone_low = city
            .zone_at(cx, cy)
            .map_or(0, |zone| zone.raw_bits() & 0x0f);
        city.set_zone(cx, cy, zone_low | marker);
    }
}

#[cfg(test)]
mod mac_kind3_boundary_target_tests {
    use super::*;

    #[test]
    fn successful_kind3_creation_updates_raw_row_column_return_target() {
        let expected = [(2, 127), (126, 127), (127, 2), (127, 126)];
        for (edge, target) in expected.into_iter().enumerate() {
            let mut city = City::new_blank(
                format!("/tmp/mac-kind3-global-boundary-{edge}.sc2"),
                "MacKind3GlobalBoundary",
            )
            .expect("blank city");
            for offset in 0..CITY_WIDTH {
                city.set_terrain(2, offset, 0x10);
                city.set_terrain(CITY_WIDTH - 2, offset, 0x10);
                city.set_terrain(offset, 2, 0x10);
                city.set_terrain(offset, CITY_HEIGHT - 2, 0x10);
            }
            let seed = (1..1000)
                .find(|seed| MacToolboxRng::new(*seed).random_u16() & 3 == edge as u16)
                .expect("seed for every native edge selector");
            let mut rng = MacToolboxRng::new(seed);

            assert!(mac_budget_try_create_zone_nine_type_three(
                &mut city, 64, 64, &mut rng
            ));
            assert_eq!(city.mac_kind3_boundary_target, target);
        }
    }
}

/// CODE 8 `export_241` class-2 acceptance branch. Unlike class 0, this uses
/// the route result directly: failed power/route still consumes the threshold
/// draw, but its zero `local_c` makes acceptance impossible.
#[cfg(test)]
pub(in crate::app) fn mac_budget_try_class_two_growth(
    city: &mut City,
    x: usize,
    y: usize,
    zone_category: usize,
    growth_level: u16,
    local_c: i32,
    rng: &mut MacToolboxRng,
) {
    if mac_budget_choose_growth_branch(
        2,
        growth_level,
        zone_category,
        local_c as i16,
        0,
        false,
        rng,
    ) == MacGrowthBranch::Pattern
    {
        mac_budget_place_growth_pattern(city, x, y, growth_level, (zone_category - 1) / 2, rng);
    }
}

/// CODE 8 `export_241`'s final generic-growth arm. Class 0 and class 1 can
/// reach this after rejection of their own branch; class 2 is its `if` arm
/// and therefore cannot also enter the generic `else` arm.
#[cfg(test)]
pub(in crate::app) fn mac_budget_try_generic_growth(
    city: &mut City,
    x: usize,
    y: usize,
    zone_byte: u8,
    zone_category: usize,
    growth_level: u16,
    local_c: i32,
    rng: &mut MacToolboxRng,
) {
    debug_assert_eq!(zone_byte & 0x0f, zone_category as u8);
    if mac_budget_choose_generic_growth(
        growth_level,
        zone_category,
        local_c as i16,
        mac_budget_growth_land_value_at(city, x, y),
        rng,
    ) == MacGrowthBranch::Generic
    {
        mac_budget_place_generic_growth(city, x, y, zone_category, growth_level, rng);
    }
}

pub(in crate::app) fn mac_budget_place_generic_growth(
    city: &mut City,
    x: usize,
    y: usize,
    zone_category: usize,
    growth_level: u16,
    rng: &mut MacToolboxRng,
) {
    match growth_level {
        // Both selector-table zero and low-XBLD override enter mode zero.
        0 => mac_budget_place_growth_pattern(city, x, y, 1, 3, rng),
        // Mode 1 performs the eight-neighbor mask and one 3x3 placement.
        1 => mac_budget_try_generic_neighbor_footprint(city, x, y, zone_category, rng),
        // Mode 2 directly forwards export_242 selector 0x30003.
        2 => mac_budget_place_growth_pattern(city, x, y, 3, 3, rng),
        // Mode 3 performs rotating 3x3 perimeter and outer-road probes.
        3 => mac_budget_try_generic_rotating_footprint(city, x, y, zone_category, rng),
        // Other recovered coefficient words pass the caller's threshold but
        // have no corresponding FUN_000370fa mode, so the helper returns.
        _ => {}
    }
}

pub(in crate::app) fn mac_budget_growth_neighbor_eligible(
    city: &City,
    x: isize,
    y: isize,
    height: u8,
    zone_category: usize,
    xbuilding_ceiling: u8,
) -> bool {
    if !(0..CITY_WIDTH as isize).contains(&x) || !(0..CITY_HEIGHT as isize).contains(&y) {
        return false;
    }
    let x = x as usize;
    let y = y as usize;
    let building = city.building_at(x, y);
    building < xbuilding_ceiling
        && (city.altitude_at(x, y) & 0x1f) == height
        && city
            .zone_at(x, y)
            .is_some_and(|zone| zone.zone_id() as usize == zone_category)
        && !matches!(building, 0x1d..=0x46 | 0x4b..=0x4e | 0x5d..=0x60 | 0x6c..=0x6f)
}

pub(in crate::app) fn mac_budget_try_generic_neighbor_footprint(
    city: &mut City,
    x: usize,
    y: usize,
    zone_category: usize,
    rng: &mut MacToolboxRng,
) {
    let height = city.altitude_at(x, y) & 0x1f;
    let mut mask = 0u8;
    let has = |dx, dy| {
        mac_budget_growth_neighbor_eligible(
            city,
            x as isize + dx,
            y as isize + dy,
            height,
            zone_category,
            0x8c,
        )
    };
    if has(1, 0) {
        mask |= 0x01;
    }
    if has(0, 1) {
        mask |= 0x04;
    }
    if has(1, 1) {
        mask |= 0x02;
    }
    if mask == 0x07 {
        mac_budget_place_growth_pattern(city, x, y + 1, 2, 3, rng);
        return;
    }
    // Keep the mask positions aligned with FUN_000379ba's native tests:
    // 0x10=(x,y-1), 0x08=(x+1,y-1), 0x40=(x-1,y), 0x20=(x-1,y+1).
    if has(0, -1) {
        mask |= 0x10;
    }
    if has(1, -1) {
        mask |= 0x08;
    }
    if mask & 0x19 == 0x19 {
        mac_budget_place_growth_pattern(city, x, y, 2, 3, rng);
        return;
    }
    if has(-1, 0) {
        mask |= 0x40;
    }
    if has(-1, 1) {
        mask |= 0x20;
    }
    if mask & 0x64 == 0x64 {
        mac_budget_place_growth_pattern(city, x - 1, y + 1, 2, 3, rng);
        return;
    }
    if has(-1, -1) {
        mask |= 0x80;
    }
    if mask & 0xd0 == 0xd0 {
        mac_budget_place_growth_pattern(city, x - 1, y, 2, 3, rng);
    }
}

pub(in crate::app) fn mac_budget_try_generic_rotating_footprint(
    city: &mut City,
    x: usize,
    y: usize,
    zone_category: usize,
    rng: &mut MacToolboxRng,
) {
    const PERIMETER: [(isize, isize); 8] = [
        (0, 0),
        (0, -1),
        (0, -2),
        (1, -2),
        (2, -2),
        (2, -1),
        (2, 0),
        (1, 0),
    ];
    const ROAD_PROBES: [[u8; 4]; 4] = [
        [0x23, 0x27, 0x28, 0x2b],
        [0x24, 0x28, 0x29, 0x2b],
        [0x25, 0x29, 0x2a, 0x2b],
        [0x26, 0x2a, 0x27, 0x2b],
    ];
    let height = city.altitude_at(x, y) & 0x1f;
    for rotation in 0..4 {
        let origin_x = x as isize - (rotation & 1) as isize;
        let origin_y = y as isize + (rotation / 2) as isize;
        if !PERIMETER.iter().all(|(dx, dy)| {
            mac_budget_growth_neighbor_eligible(
                city,
                origin_x + dx,
                origin_y + dy,
                height,
                zone_category,
                0xae,
            )
        }) {
            continue;
        }
        let probes = [
            (origin_x - 1, origin_y + 1),
            (origin_x - 1, origin_y - 3),
            (origin_x + 3, origin_y - 3),
            (origin_x + 3, origin_y + 1),
        ];
        let road_gate = probes.iter().zip(ROAD_PROBES).any(|(&(px, py), allowed)| {
            (0..CITY_WIDTH as isize).contains(&px)
                && (0..CITY_HEIGHT as isize).contains(&py)
                && allowed.contains(&city.building_at(px as usize, py as usize))
        });
        if !road_gate {
            continue;
        }

        for (dx, dy) in PERIMETER {
            let px = origin_x + dx;
            let py = origin_y + dy;
            if city.building_at(px as usize, py as usize) >= 0x8c {
                mac_budget_place_generic_rotated_edge(city, px as usize, py as usize, rng);
            }
        }
        mac_budget_place_growth_pattern(city, origin_x as usize, origin_y as usize, 4, 3, rng);
        return;
    }
}

pub(in crate::app) fn mac_budget_place_generic_rotated_edge(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) {
    let marker = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits() & 0xf0);
    // The XZON corner marker already records the map-oriented placement
    // orientation. The native helper consumes it directly; subtracting the
    // saved city rotation here rotates the anchors a second time.
    let offsets = match marker {
        // FUN_000378c6's marker-selected four single-cell placement anchors.
        0x10 => [(-1, 1), (0, 1), (0, 0), (-1, 0)],
        0x20 => [(-1, 0), (0, 0), (0, -1), (-1, -1)],
        0x40 => [(0, 0), (1, 0), (1, -1), (0, -1)],
        0x80 => [(0, 1), (1, 1), (1, 0), (0, 0)],
        _ => [(0, 0), (0, 0), (0, -1), (0, -1)],
    };
    for (dx, dy) in offsets {
        let anchor_x = x.checked_add_signed(dx).unwrap_or(CITY_WIDTH);
        let anchor_y = y.checked_add_signed(dy).unwrap_or(CITY_HEIGHT);
        mac_budget_place_growth_pattern(city, anchor_x, anchor_y, 1, 4, rng);
    }
}

/// Mac CODE 8 `export_243` caller-local footprint table. Each `export_242`
/// call consumes its own selection word, even when its square is out of bounds.
pub(in crate::app) fn mac_budget_dispatch_class_zero_footprint(
    city: &mut City,
    x: usize,
    y: usize,
    growth_level: u16,
    orientation: bool,
    rng: &mut MacToolboxRng,
) {
    match growth_level {
        1 => mac_budget_place_growth_pattern(city, x, y, 1, 4, rng),
        2 if !orientation => mac_budget_place_growth_pattern(city, x, y, 2, 4, rng),
        2 => {
            for (dx, dy) in [(0, 0), (1, 0), (1, -1), (0, -1)] {
                let anchor_x = x.checked_add_signed(dx).unwrap_or(CITY_WIDTH);
                let anchor_y = y.checked_add_signed(dy).unwrap_or(CITY_HEIGHT);
                mac_budget_place_growth_pattern(city, anchor_x, anchor_y, 1, 4, rng);
            }
        }
        3 if !orientation => mac_budget_place_growth_pattern(city, x, y, 3, 4, rng),
        3 => mac_budget_place_growth_pattern(city, x, y, 2, 4, rng),
        4 if !orientation => mac_budget_place_growth_pattern(city, x, y, 4, 4, rng),
        4 => {
            // Native CODE 8 export_243's eight one-cell calls followed by one
            // randomized 3x3 call. Preserve call order for the nested RNG.
            for (dx, dy) in [
                (0, 0),
                (1, 0),
                (2, 0),
                (2, -1),
                (2, -2),
                (1, -2),
                (0, -2),
                (0, -1),
            ] {
                let anchor_x = x.checked_add_signed(dx).unwrap_or(CITY_WIDTH);
                let anchor_y = y.checked_add_signed(dy).unwrap_or(CITY_HEIGHT);
                mac_budget_place_growth_pattern(city, anchor_x, anchor_y, 1, 4, rng);
            }
            let center_draw = rng.random_i16() as u16 & 3;
            let center_x = x + usize::from(center_draw & 1);
            let center_y = y
                .checked_sub(usize::from(center_draw / 2))
                .unwrap_or(CITY_HEIGHT);
            mac_budget_place_growth_pattern(city, center_x, center_y, 3, 4, rng);
        }
        _ => {}
    }
}

/// CODE 14 `export_216`'s early low-XBLD query in CODE 8 export_241. This
/// checks the recovered 24-cell cardinal/diagonal search stencil for a
/// road/rail/transit XBLD marker before entering the shared growth route.
pub(in crate::app) fn mac_budget_has_nearby_transit(city: &City, x: usize, y: usize) -> bool {
    const OFFSETS: [(isize, isize); 24] = [
        (0, 1),
        (1, 0),
        (0, -1),
        (-1, 0),
        (0, 2),
        (2, 0),
        (0, -2),
        (-2, 0),
        (0, 3),
        (3, 0),
        (0, -3),
        (-3, 0),
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
        (2, 1),
        (-2, 1),
        (2, -1),
        (-2, -1),
        (1, 2),
        (-1, 2),
        (1, -2),
        (-1, -2),
    ];
    OFFSETS.into_iter().any(|(dx, dy)| {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        (0..CITY_WIDTH as isize).contains(&nx)
            && (0..CITY_HEIGHT as isize).contains(&ny)
            && matches!(
                city.building_at(nx as usize, ny as usize),
                0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c | 0x5d..=0x60 | 0xe9 | 0xec | 0xed
            )
    })
}

/// Mac CODE 8 `export_241` route branch through `FUN_00036c8c` and CODE 14
/// `export_217`. Ordinary candidates require XZON's orientation-selected
/// corner marker; the low-XBLD override instead reaches this shared worker
/// after its independent nearby-network query. Both paths require power on
/// this cell or a cardinal neighbor. Only a successful route reaches export
/// 217's density and bus/rail/subway accumulator epilogue.
pub(in crate::app) fn mac_budget_try_transit_route_at(
    city: &mut City,
    x: usize,
    y: usize,
    zone_category: usize,
    growth_coefficient: u16,
    require_route_marker: bool,
    rng: &mut MacToolboxRng,
) -> i32 {
    let marker_bits = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
    const ROUTE_MARKER_MASKS: [u8; 4] = [0x80, 0x10, 0x20, 0x40];
    let marker = !require_route_marker
        || (city.building_at(x, y) >= 0x70
            && marker_bits & ROUTE_MARKER_MASKS[(city.saved_rotation() & 3) as usize] != 0);
    let powered = mac_transit_has_powered_adjacency(city, x, y);
    if !marker || !powered {
        return 0;
    }

    let route = mac_transit_route_search(
        x,
        y,
        zone_category,
        growth_coefficient,
        100,
        |cell_x, cell_y| MacTransitRouteCell {
            building: city.building_at(cell_x, cell_y),
            zone_low: city
                .zone_at(cell_x, cell_y)
                .map_or(0, |zone| zone.raw_bits() & 0x0f),
            tunnel_levels: city.tunnel_levels_at(cell_x, cell_y),
            underground: city.underground_at(cell_x, cell_y),
            text_overlay: city.text_overlay_at(cell_x, cell_y).unwrap_or(0),
        },
        || rng.random_i16(),
    );
    if let Some(route) = route {
        mac_transit_apply_route_success(city, &route, growth_coefficient);
        let demand_index = (zone_category.saturating_sub(1) / 2).min(2);
        let demand = city
            .mac_state_misc_u32_at(super::RCI_DEMAND_MISC_OFFSET + demand_index * 4)
            .unwrap_or(0) as u16 as i16;
        i32::from(demand.wrapping_add(2000))
    } else {
        0
    }
}

/// Mac CODE 8 `export_241` selector table at `A5-0x354c`, indexed by
/// `XBLD - 0x70`. This is kept separate from the PE table because Mac selects
/// level 4 for XBLD `0xc5`, while PE selects zero. The final two zero entries
/// are Mac's `0xc6`/`0xc7` selectors.
const MAC_GROWTH_LEVELS_BY_XBLD: [i16; 88] = [
    // 0x70..0x7f
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0x80..0x8f
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, // 0x90..0x9f
    3, 3, 3, 3, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 2, 2, // 0xa0..0xaf
    2, 2, 3, 3, 3, 3, 2, 2, 3, 3, 2, 2, 3, 3, 4, 4, // 0xb0..0xbf
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, // 0xc0..0xc7
    4, 4, 4, 4, 4, 4, 0, 0,
];

/// Mac CODE 8 `export_241` byte table at `A5-0x34a0`, indexed by XBLD−0x70.
/// Values outside 0, 1, and 2 take the generic growth branch.
pub(in crate::app) fn mac_growth_class_for_xbld(tile: u8) -> u8 {
    match tile {
        0x88..=0x89 | 0xa6..=0xa9 | 0xc2..=0xc3 => 1,
        0x8a..=0x8b | 0xaa..=0xad | 0xc4..=0xc5 => 2,
        0xc7 => 112,
        0xc9 => 140,
        0xcb => 144,
        0xcd => 174,
        0xcf => 124,
        0xd1 => 148,
        0xd3 => 153,
        0xd5 => 178,
        0xd7 => 132,
        0xd9 => 158,
        0xdb => 162,
        0xdd => 188,
        0xdf => 136,
        0xe1 => 166,
        0xe3 => 168,
        0xe5 => 194,
        0xe7 => 138,
        0xe9 => 170,
        0xeb => 172,
        0xed => 196,
        0xef => 12,
        0xf1 | 0xf3 | 0xf5 => 4,
        0xf7 => 8,
        0xf9 | 0xfb => 5,
        0xfd => 10,
        0xff => 4,
        _ => 0,
    }
}

/// Mac CODE 8 `export_241` WORD table at `A5-0x354c`, indexed by XBLD−0x70.
pub(in crate::app) fn mac_growth_coefficient_for_xbld(tile: u8) -> u16 {
    match tile {
        0x70..=0x8b => 1,
        0x8c..=0x8f => 2,
        0x90..=0x93 => 3,
        0x94..=0x98 => 2,
        0x99..=0x9d => 3,
        0x9e..=0xa1 => 2,
        0xa2..=0xa5 => 3,
        0xa6..=0xa7 => 2,
        0xa8..=0xa9 => 3,
        0xaa..=0xab => 2,
        0xac..=0xad => 3,
        0xae..=0xc5 => 4,
        0xd2 | 0xe1..=0xe2 | 0xef => 0x0101,
        0xd3 | 0xe3..=0xe4 | 0xf0 => 0x0202,
        0xf1..=0xff => [
            112, 140, 144, 174, 124, 148, 153, 178, 132, 158, 162, 188, 136, 166, 168,
        ][usize::from(tile - 0xf1)],
        _ => 0,
    }
}

/// Signed lookup at `A5-0x33fa + 2*coefficient`; only the recovered values
/// reached by the caller's 144-entry selector table are represented here.
pub(in crate::app) fn mac_growth_counter_delta(coefficient: u16) -> i16 {
    match coefficient {
        1 => 1,
        2 => 8,
        3 => 12,
        4 => 36,
        166 | 168 => 20,
        174 => 15,
        188 => 30,
        514 => -1,
        _ => 0,
    }
}

pub(in crate::app) fn mac_growth_level_for_xbld(tile: u8) -> i16 {
    tile.checked_sub(0x70)
        .map(usize::from)
        .and_then(|index| MAC_GROWTH_LEVELS_BY_XBLD.get(index))
        .copied()
        .unwrap_or(0)
}
