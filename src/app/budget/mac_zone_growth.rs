//! Mac CODE 8 special-zone growth selectors and placement.
//!
//! Zone 7/8/9 type choice, runway/water rays, square footprints, and the
//! kind-3 boundary exception retain their original draw and write order.

use super::*;
use crate::win95_overlay::{XTHG_FIRST_USABLE_SLOT, XTHG_RECORD_COUNT};

/// Mac CODE 8 `FUN_00037ab0` `0xdd` positive-ray choice. The byte read at
/// `fffff07a+0x1bb` is bit 8 of the XBLD-`0xdd` census WORD, not a separate
/// orientation plane. It only breaks the odd/odd coordinate tie.
pub(in crate::app) fn mac_budget_zone_dd_positive_ray(
    city: &City,
    x: usize,
    y: usize,
) -> Option<(isize, isize)> {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT || (x & 1 == 0 && y & 1 == 0) {
        return None;
    }

    let census_bit_8 = city.mac_xbld_tile_count(0xdd) & 0x0100 != 0;
    Some(
        if x & 1 == 0 || (x & 1 != 0 && y & 1 != 0 && census_bit_8) {
            (1, 0)
        } else {
            (0, 1)
        },
    )
}

/// CODE 8 `export_241` late special-zone type selectors. The returned XBLD is
/// the selector result only; `FUN_00037ab0` still owns admission, placement,
/// and (for zone 9) the caller-owned fallback attempt.
pub(in crate::app) fn mac_budget_zone_special_growth_xbld(
    zone_category: usize,
    mut count: impl FnMut(u8) -> u16,
) -> Option<u8> {
    match zone_category {
        9 => {
            let threshold = count(0xe0);
            if count(0xf2) >> 2 >= threshold {
                Some(0xe0)
            } else if count(0xf0) >> 2 < threshold {
                Some(0xf0)
            } else if count(0xe3) / 3 >= threshold {
                Some(0xe3)
            } else {
                Some(0xf2)
            }
        }
        8 => {
            // 68k ADD.W wraps before DIVU.W; keep that intermediate width.
            let threshold = count(0xdd).wrapping_add(count(0xde)) / 5;
            if count(0xee) >> 2 <= threshold {
                Some(0xdd)
            } else if count(0xe1).wrapping_mul(2) < threshold {
                Some(0xe1)
            } else if count(0xea).wrapping_mul(2) < threshold {
                Some(0xea)
            } else if threshold > count(0xe6) {
                Some(0xe6)
            } else if count(0xe4) >> 1 < threshold {
                Some(0xe4)
            } else if count(0xe5) >> 1 < threshold {
                Some(0xe5)
            } else if count(0xf6) >> 2 < threshold {
                Some(0xf6)
            } else {
                Some(0xee)
            }
        }
        _ => None,
    }
}

/// CODE 8 `export_241` zone-7 selector branches, backed by the saved
/// `fffff07e` 16-WORD class census. `event` is persisted `ffffe0c2`.
pub(in crate::app) fn mac_budget_zone_seven_special_growth_xbld(
    event: u8,
    mut count: impl FnMut(usize) -> u16,
) -> Option<u8> {
    match event {
        4 => {
            let threshold = count(10);
            if count(4) >> 2 < threshold {
                if count(9) >> 2 < threshold {
                    Some(0xf1)
                } else if count(6) / 3 < threshold {
                    Some(0xe3)
                } else {
                    Some(0xf2)
                }
            } else {
                Some(0xe0)
            }
        }
        2 => {
            if count(13) / 12 < count(3) >> 2 {
                Some(0xe8)
            } else {
                Some(0xef)
            }
        }
        3 => {
            let threshold = count(1).wrapping_add(count(2)) / 5;
            if count(3) >> 2 < threshold {
                if count(11).wrapping_mul(2) < threshold {
                    Some(0xe2)
                } else if count(5).wrapping_mul(2) < threshold {
                    Some(0xea)
                } else if count(12) < threshold {
                    Some(0xe7)
                } else if count(7) >> 1 < threshold {
                    Some(0xe4)
                } else if count(8) >> 1 < threshold {
                    Some(0xe5)
                } else if count(14) >> 2 < threshold {
                    Some(0xf6)
                } else {
                    Some(0xef)
                }
            } else {
                Some(0xdd)
            }
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum MacSpecialTileAttempt {
    Refused,
    AcceptedWithoutPlacement,
    Placed,
}

/// CODE 8 `FUN_00037ab0` single-cell `0xe1..=0xea` arm, including CODE 5
/// export 107's deterministic XBIT/XBLD/XZON writes. The stack-dependent
/// `fffffb44` marker is deliberately outside this result contract.
#[allow(dead_code)]
pub(in crate::app) fn mac_budget_try_place_zone_single_tile(
    city: &mut City,
    x: usize,
    y: usize,
    zone_category: usize,
    tile: u8,
) -> MacSpecialTileAttempt {
    if x >= CITY_WIDTH
        || y >= CITY_HEIGHT
        || !(0xe1..=0xea).contains(&tile)
        || (zone_category != 7 && !mac_transit_has_powered_adjacency(city, x, y))
    {
        return MacSpecialTileAttempt::Refused;
    }

    if city.building_at(x, y) >= 0x0d {
        return MacSpecialTileAttempt::AcceptedWithoutPlacement;
    }

    // export 107 retains XBIT's low five bits and writes its XBLD tile before
    // the typed helper restores XZON's special high marker and zone nibble.
    let bits = city.tile_bits_at(x, y).unwrap_or_default();
    city.set_tile_bits(x, y, TileBits::new((bits.raw_bits() & 0x1f) | 0xe0));
    city.set_building(x, y, tile);
    city.set_zone(x, y, 0xf0 | (zone_category as u8 & 0x0f));
    if zone_category == 7 {
        city.set_tile_bits(
            x,
            y,
            TileBits::new(city.tile_bits_at(x, y).unwrap_or_default().raw_bits() & 0x0f),
        );
    }
    MacSpecialTileAttempt::Placed
}

/// CODE 8 `FUN_00037ab0`'s selector-reachable 2×2 arms and deterministic
/// CODE 5 export-107 writes. This intentionally reports helper admission
/// separately from whether a mutation occurred.
#[allow(dead_code)]
pub(in crate::app) fn mac_budget_try_place_zone_square(
    city: &mut City,
    x: usize,
    y: usize,
    zone_category: usize,
    tile: u8,
) -> MacSpecialTileAttempt {
    if x >= CITY_WIDTH
        || y >= CITY_HEIGHT
        || !matches!(tile, 0xee | 0xef | 0xf0 | 0xf1 | 0xf2 | 0xf6)
        || (zone_category != 7 && !mac_transit_has_powered_adjacency(city, x, y))
    {
        return MacSpecialTileAttempt::Refused;
    }

    let left = x & !1;
    let top = y & !1;
    let cells = [
        (left, top),
        (left + 1, top),
        (left, top + 1),
        (left + 1, top + 1),
    ];
    let north_west_building = city.building_at(left, top);
    if north_west_building > 0xea
        || cells
            .iter()
            .any(|&(cx, cy)| matches!(city.building_at(cx, cy), 0xdd | 0xde | 0xe0))
        || cells.iter().any(|&(cx, cy)| {
            city.zone_at(cx, cy)
                .is_none_or(|zone| usize::from(zone.raw_bits() & 0x0f) != zone_category)
        })
    {
        return MacSpecialTileAttempt::Refused;
    }

    // FUN_0003844a is called in NW, NE, SW, SE order. It is a no-op through
    // 0xc5, clears a single cell for 0xdb..=0xea, and otherwise clears the
    // even-aligned 2×2 footprint. Re-read each cell after the prior cleanup,
    // matching the caller's sequential tests.
    for &(cx, cy) in &cells {
        if city.building_at(cx, cy) > 0x0c {
            mac_budget_cleanup_replaced_high_building(city, cx, cy);
        }
    }

    // CODE 5 export 107 size=2: PlaceTile for each cell and then the four
    // orientation-indexed XZON corner markers. Its conditional stack marker
    // in fffffb44 remains intentionally excluded.
    let bits = [0x10, 0x20, 0x40, 0x80];
    let corner_markers = match city.saved_rotation() & 3 {
        0 => bits,
        1 => [0x20, 0x40, 0x80, 0x10],
        2 => [0x40, 0x80, 0x10, 0x20],
        _ => [0x80, 0x10, 0x20, 0x40],
    };
    // The scan/cleanup coordinate list is NW, NE, SW, SE; export 107's
    // marker table is addressed NW, NE, SE, SW.
    for (corner_index, (cx, cy)) in [(0, cells[0]), (1, cells[1]), (3, cells[2]), (2, cells[3])]
        .iter()
        .map(|(index, cell)| (*index, *cell))
    {
        let old_bits = city.tile_bits_at(cx, cy).unwrap_or_default();
        city.set_tile_bits(cx, cy, TileBits::new((old_bits.raw_bits() & 0x1f) | 0xe0));
        city.set_building(cx, cy, tile);
        let old_zone = city.zone_at(cx, cy).map_or(0, |zone| zone.raw_bits());
        city.set_zone(cx, cy, (old_zone & 0x0f) | corner_markers[corner_index]);
    }
    for &(cx, cy) in &cells {
        let old_zone = city.zone_at(cx, cy).map_or(0, |zone| zone.raw_bits());
        city.set_zone(cx, cy, (old_zone & 0xf0) | (zone_category as u8 & 0x0f));
    }
    if zone_category == 7 {
        for &(cx, cy) in &cells {
            let bits = city.tile_bits_at(cx, cy).unwrap_or_default();
            city.set_tile_bits(cx, cy, TileBits::new(bits.raw_bits() & 0x0f));
        }
    }
    MacSpecialTileAttempt::Placed
}

/// CODE 8 `FUN_00037ab0`'s `0xe0` coastal/ray arm. The byte access at
/// `ffffe0c7` is the low byte of the big-endian WORD at `ffffe0c6`; bit 0 is
/// the same saved city-view rotation parity.
#[allow(dead_code)]
pub(in crate::app) fn mac_budget_try_place_zone_water_ray(
    city: &mut City,
    x: usize,
    y: usize,
    zone_category: usize,
) -> MacSpecialTileAttempt {
    const DIRECTIONS: [(isize, isize); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];
    const WATER_BIT: u8 = 0x04;

    if x >= CITY_WIDTH
        || y >= CITY_HEIGHT
        || (zone_category != 7 && !mac_transit_has_powered_adjacency(city, x, y))
    {
        return MacSpecialTileAttempt::Refused;
    }

    // The original scans the four adjacent tiles in table order and takes the
    // first water-bit cell. The anchor itself is the land-side tile.
    let Some((dx, dy)) = DIRECTIONS.iter().copied().find(|&(dx, dy)| {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        (0..CITY_WIDTH as isize).contains(&nx)
            && (0..CITY_HEIGHT as isize).contains(&ny)
            && city
                .tile_bits_at(nx as usize, ny as usize)
                .is_some_and(|bits| bits.raw_bits() & WATER_BIT != 0)
    }) else {
        return MacSpecialTileAttempt::Refused;
    };

    // The continuation must run along a row/column whose cross-axis is even.
    if (dy != 0 && x & 1 != 0) || (dx != 0 && y & 1 != 0) {
        return MacSpecialTileAttempt::Refused;
    }

    // Check five water cells beyond the anchor. The fifth is the far-end
    // height sample; only the first four receive continuation tile 0xdf.
    let mut checked = [(0usize, 0usize); 5];
    for (i, cell) in checked.iter_mut().enumerate() {
        let cx = x as isize + dx * (i as isize + 1);
        let cy = y as isize + dy * (i as isize + 1);
        if !(0..CITY_WIDTH as isize).contains(&cx) || !(0..CITY_HEIGHT as isize).contains(&cy) {
            return MacSpecialTileAttempt::Refused;
        }
        let (cx, cy) = (cx as usize, cy as usize);
        if city.tile_bits_at(cx, cy).unwrap_or_default().raw_bits() & WATER_BIT == 0
            || city.building_at(cx, cy) != 0
        {
            return MacSpecialTileAttempt::Refused;
        }
        *cell = (cx, cy);
    }

    let (end_x, end_y) = checked[4];
    if city.water_height_at(end_x, end_y) < city.altitude_at(end_x, end_y).saturating_add(2) {
        return MacSpecialTileAttempt::Refused;
    }

    if city.building_at(x, y) >= 0x0d {
        mac_budget_cleanup_replaced_high_building(city, x, y);
    }

    // CODE 5 export 107 with packed argument 0xe00001 creates the anchor and
    // gives it the standard special-zone XBIT/XZON writes.
    let old_bits = city.tile_bits_at(x, y).unwrap_or_default().raw_bits();
    city.set_tile_bits(x, y, TileBits::new((old_bits & 0x1f) | 0xe0));
    city.set_building(x, y, 0xe0);
    let old_zone = city.zone_at(x, y).map_or(0, |zone| zone.raw_bits());
    city.set_zone(
        x,
        y,
        (old_zone & 0xf0) | 0xf0 | (zone_category as u8 & 0x0f),
    );
    if zone_category == 7 {
        let bits = city.tile_bits_at(x, y).unwrap_or_default();
        city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0x0f));
    }

    // The direction's nonzero Y component and ffffffe0c7 bit determine
    // whether the continuation path carries XBIT marker bit 1.
    let mark_continuation_xbit = (dy != 0) ^ (city.saved_rotation() & 1 != 0);
    for &(cx, cy) in &checked[..4] {
        city.set_building(cx, cy, 0xdf); // direct CODE 5 export 105 call
        let old_zone = city.zone_at(cx, cy).map_or(0, |zone| zone.raw_bits());
        city.set_zone(cx, cy, (old_zone & 0x0f) | 0xf0);
        if mark_continuation_xbit {
            let bits = city.tile_bits_at(cx, cy).unwrap_or_default();
            city.set_tile_bits(cx, cy, TileBits::new(bits.raw_bits() | 0x02));
        }
    }
    MacSpecialTileAttempt::Placed
}

/// CODE 8 `FUN_00037ab0`'s deterministic `0xdd` five-new-cell runway arm.
/// Existing `0xdd/0xde` markers extend the physical ray without satisfying
/// its five-cell placement count. The stack-dependent export-107 marker is
/// intentionally not part of this helper.
#[allow(dead_code)]
pub(in crate::app) fn mac_budget_try_place_zone_runway_ray(
    city: &mut City,
    x: usize,
    y: usize,
    zone_category: usize,
) -> MacSpecialTileAttempt {
    let Some((dx, dy)) = mac_budget_zone_dd_positive_ray(city, x, y) else {
        return MacSpecialTileAttempt::Refused;
    };
    if zone_category != 7 && !mac_transit_has_powered_adjacency(city, x, y) {
        return MacSpecialTileAttempt::Refused;
    }

    let orientation_marker = (dx != 0) == (city.saved_rotation() & 1 != 0);
    let mut cells = Vec::with_capacity(5);
    let (mut cx, mut cy) = (x as isize, y as isize);
    while cells.len() < 5 {
        if !(0..CITY_WIDTH as isize).contains(&cx) || !(0..CITY_HEIGHT as isize).contains(&cy) {
            return MacSpecialTileAttempt::Refused;
        }
        let cell = (cx as usize, cy as usize);
        if city
            .zone_at(cell.0, cell.1)
            .is_none_or(|zone| usize::from(zone.raw_bits() & 0x0f) != zone_category)
        {
            return MacSpecialTileAttempt::Refused;
        }
        if !matches!(city.building_at(cell.0, cell.1), 0xdd | 0xde) {
            cells.push(cell);
        }
        cx += dx;
        cy += dy;
    }

    // The scan accepts existing runway markers as transparent. An existing
    // 0xdd with the other orientation is converted to 0xde, but remains
    // transparent to the five-new-cell count.
    let mut processed = 0;
    (cx, cy) = (x as isize, y as isize);
    while processed < 5 {
        if !(0..CITY_WIDTH as isize).contains(&cx) || !(0..CITY_HEIGHT as isize).contains(&cy) {
            break; // the successful preflight already proves five placements
        }
        let (tx, ty) = (cx as usize, cy as usize);
        match city.building_at(tx, ty) {
            0xde => {}
            0xdd => {
                let bits = city.tile_bits_at(tx, ty).unwrap_or_default();
                if orientation_marker != (bits.raw_bits() & 0x02 != 0) {
                    city.set_building(tx, ty, 0xde);
                    let zone = city.zone_at(tx, ty).map_or(0, |zone| zone.raw_bits());
                    city.set_zone(tx, ty, (zone & 0x0f) | 0xf0);
                    let mut raw = bits.raw_bits();
                    if zone_category != 7 {
                        raw |= 0xc0;
                    }
                    city.set_tile_bits(tx, ty, TileBits::new(raw & !0x02));
                }
            }
            _ => {
                if city.building_at(tx, ty) > 0x0c {
                    mac_budget_cleanup_replaced_high_building(city, tx, ty);
                }
                city.set_building(tx, ty, 0xdd);
                let zone = city.zone_at(tx, ty).map_or(0, |zone| zone.raw_bits());
                city.set_zone(tx, ty, (zone & 0x0f) | 0xf0);
                let mut raw = city.tile_bits_at(tx, ty).unwrap_or_default().raw_bits();
                if zone_category != 7 {
                    raw |= 0xc0;
                }
                if orientation_marker {
                    raw |= 0x02;
                }
                city.set_tile_bits(tx, ty, TileBits::new(raw));
                processed += 1;
            }
        }
        cx += dx;
        cy += dy;
    }
    MacSpecialTileAttempt::Placed
}

/// CODE 8 `export_241`'s `D5 == 8` arm at `0x36a28..0x36a8e`. The original
/// always consumes one QuickDraw word, then either chooses and attempts one
/// special footprint or takes the `0xdd` timing/event path. This models the
/// gameplay writes and shared random-stream position. Its trailing
/// TickCount/SystemTask calls service the classic Mac event loop and are owned
/// by the SDL host instead.
pub(in crate::app) fn mac_budget_process_zone_eight_special_growth(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    kind2_sound_deadline: &mut u32,
) -> MacSpecialTileAttempt {
    let first = rng.random_i16() as u16;
    if first & 3 == 0 {
        let Some(tile) =
            mac_budget_zone_special_growth_xbld(8, |tile| city.mac_xbld_tile_count(tile))
        else {
            return MacSpecialTileAttempt::Refused;
        };
        return match tile {
            0xdd => mac_budget_try_place_zone_runway_ray(city, x, y, 8),
            0xe1 | 0xe4..=0xe6 | 0xea => mac_budget_try_place_zone_single_tile(city, x, y, 8, tile),
            0xee | 0xf6 => mac_budget_try_place_zone_square(city, x, y, 8, tile),
            _ => MacSpecialTileAttempt::Refused,
        };
    }

    // The alternate arm is entered only for an existing runway marker. The
    // second and third Random calls are strictly behind the original modulo
    // and XBIT gates. The third draw selects the kind-2 constructor for values
    // below four; the remaining values select kind 1. Its caller mode depends
    // on saved orientation and the candidate tile's XBIT bit 1.
    if city.building_at(x, y) == 0xdd {
        let second = rng.random_i16() as u16;
        if second % 30 == 0
            && city
                .tile_bits_at(x, y)
                .is_some_and(|bits| bits.raw_bits() & 0x40 != 0)
        {
            let third = rng.random_i16() as u16;
            if third % 10 < 4 {
                mac_budget_try_create_runway_kind2(city, x, y, rng, kind2_sound_deadline);
            } else {
                let candidate_bit = city
                    .tile_bits_at(x, y)
                    .is_some_and(|bits| bits.raw_bits() & 0x02 != 0);
                let orientation = city.saved_rotation() & 1 != 0;
                let mode = if orientation == candidate_bit { 0 } else { 2 };
                mac_budget_try_create_runway_kind1(city, x, y, mode, rng);
            }
        }
    }
    MacSpecialTileAttempt::Refused
}

/// CODE 6 export 114: construct a kind-1 runway-associated XTHG actor.
/// Mode 0 is selected when saved orientation and candidate XBIT bit 1 agree;
/// mode 2 is selected when they differ. Draw order and writes follow the
/// relocation-aware CODE 8 `0x36a82..0x36af4` and CODE 6 `0xc1ca..0xc39a`
/// listings.
fn mac_budget_try_create_runway_kind1(
    city: &mut City,
    x: usize,
    y: usize,
    mode: u8,
    rng: &mut MacToolboxRng,
) -> bool {
    if x >= CITY_WIDTH
        || y >= CITY_HEIGHT
        || city.text_overlay_at(x, y).unwrap_or(0) >= 0xc9
        || city.mac_kind5_actor_counter != 0
        || (city.mac_kind1_actor_counter as i16) >= 2
    {
        return false;
    }

    let saved_xtxt = city.text_overlay_at(x, y).unwrap_or(0);
    let slot = {
        let records = city.mac_object_slot_records_mut();
        let Some(slot) =
            (XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT).find(|&slot| records[slot].kind == 0)
        else {
            return false;
        };
        slot
    };
    let direct = rng.random_u16() % 10 >= 5;
    let (lane, state, height, tile_x, tile_y, target_x, target_y) = if direct {
        (mode, 0, 0, y as u8, x as u8, 20, 20)
    } else {
        let edge = (rng.random_u16() & 3) as u8;
        let inset = (rng.random_u16() % 100 + 10) as u8;
        let (row, column, heading) = match edge {
            0 => (0, inset, 3),
            1 => (inset, 0, 5),
            2 => (127, inset, 7),
            _ => (inset, 127, 1),
        };
        let state = if mode == 0 { 0x03 } else { 0x23 };
        let (target_x, target_y) = if mode == 0 {
            (y as u8, (x as u8).wrapping_add(16))
        } else {
            ((y as u8).wrapping_sub(16), x as u8)
        };
        (heading, state, 16, row, column, target_x, target_y)
    };

    let records = city.mac_object_slot_records_mut();
    let record = &mut records[slot];
    record.kind = 1;
    record.lane = lane;
    record.state = state;
    // Mac XTHG raw +3/+4 are map row/column; Rust's tile_x/tile_y labels
    // therefore hold Y/X for this engine's record layout.
    record.tile_x = tile_x;
    record.tile_y = tile_y;
    record.height = height;
    record.sub_x = 8;
    record.sub_y = 8;
    record.target_x = target_x;
    record.target_y = target_y;
    record.saved_a = saved_xtxt;
    // Native export 114 leaves record +0x0b untouched.

    city.set_text_overlay(x, y, (slot as u8).wrapping_add(0xc9));
    city.mac_kind1_actor_counter = city.mac_kind1_actor_counter.wrapping_add(1);
    true
}

/// CODE 6 export 115: construct the kind-2 XTHG actor selected by the
/// monthly runway scan. Failed admission and slot gates consume no constructor
/// draws. The final export-111 call only updates scene redraw bounds, so its
/// presentation effect is outside this simulation adapter.
fn mac_budget_try_create_runway_kind2(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
    kind2_sound_deadline: &mut u32,
) -> bool {
    if x >= CITY_WIDTH
        || y >= CITY_HEIGHT
        || city.text_overlay_at(x, y).unwrap_or(0) >= 0xc9
        || city.mac_kind5_actor_counter != 0
        || (city.mac_kind2_actor_counter as i16) >= 1
    {
        return false;
    }

    let saved_xtxt = city.text_overlay_at(x, y).unwrap_or(0);
    let records = city.mac_object_slot_records_mut();
    let Some(slot) =
        (XTHG_FIRST_USABLE_SLOT..XTHG_RECORD_COUNT).find(|&slot| records[slot].kind == 0)
    else {
        return false;
    };

    let record = &mut records[slot];
    record.kind = 2;
    record.lane = 2;
    record.state = 0;
    // Mac XTHG stores map row in +3 and column in +4; the shared record
    // container names these bytes tile_x and tile_y for the PE layout.
    record.tile_x = y as u8;
    record.tile_y = x as u8;
    record.height = 0;
    record.sub_x = 8;
    record.sub_y = 8;
    record.target_x = (rng.random_u16() % 128) as u8;
    record.target_y = (rng.random_u16() % 128) as u8;
    record.saved_a = saved_xtxt;
    // Native export 115 leaves +0x0b untouched.

    city.set_text_overlay(x, y, (slot as u8).wrapping_add(0xc9));
    city.mac_kind2_actor_counter = city.mac_kind2_actor_counter.wrapping_add(1);
    *kind2_sound_deadline = 0;
    true
}

/// CODE 8 `export_241`'s event-5 arm: probe up to two preceding cells along
/// the row coordinate, then the byte-offset coordinate, moving the origin only
/// when that preceding cell is zone 7; place the nine-tile `0xf9` footprint; then stamp
/// rotation-indexed XZON corner markers. The original has no occupancy or
/// terrain admission checks. Rust bounds-checks the footprint because behavior
/// at the original guard-cell edges is not established.
pub(in crate::app) fn mac_budget_try_place_zone_seven_f9_footprint(
    city: &mut City,
    x: usize,
    y: usize,
) -> MacSpecialTileAttempt {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return MacSpecialTileAttempt::Refused;
    }

    let is_zone_seven = |city: &City, x, y| {
        city.zone_at(x, y)
            .is_some_and(|zone| zone.raw_bits() & 0x0f == 7)
    };
    let mut origin_x = x;
    let mut origin_y = y;
    // Native FUN_00037ab0 probes the preceding row before moving D3/X, then
    // probes the preceding byte offset before moving D4/Y (CODE 8
    // 0x3827e..0x382f2). Testing the current cell before decrementing shifts
    // the footprint one tile too far when the first preceding cell is absent.
    for _ in 0..2 {
        let Some(next_x) = origin_x.checked_sub(1) else {
            if is_zone_seven(city, origin_x, origin_y) {
                return MacSpecialTileAttempt::Refused;
            }
            break;
        };
        if is_zone_seven(city, next_x, origin_y) {
            origin_x = next_x;
        } else {
            break;
        }
    }
    for _ in 0..2 {
        let Some(next_y) = origin_y.checked_sub(1) else {
            if is_zone_seven(city, origin_x, origin_y) {
                return MacSpecialTileAttempt::Refused;
            }
            break;
        };
        if is_zone_seven(city, origin_x, next_y) {
            origin_y = next_y;
        } else {
            break;
        }
    }
    if origin_x + 2 >= CITY_WIDTH || origin_y + 2 >= CITY_HEIGHT {
        return MacSpecialTileAttempt::Refused;
    }

    for dy in 0..3 {
        for dx in 0..3 {
            city.set_building(origin_x + dx, origin_y + dy, 0xf9);
        }
    }

    let bits = [0x10, 0x20, 0x40, 0x80];
    let corner_markers = match city.saved_rotation() & 3 {
        0 => bits,
        1 => [0x20, 0x40, 0x80, 0x10],
        2 => [0x40, 0x80, 0x10, 0x20],
        _ => [0x80, 0x10, 0x20, 0x40],
    };
    for (index, (cx, cy)) in [
        (origin_x, origin_y),
        (origin_x + 2, origin_y),
        (origin_x + 2, origin_y + 2),
        (origin_x, origin_y + 2),
    ]
    .into_iter()
    .enumerate()
    {
        let old_zone = city.zone_at(cx, cy).map_or(0, |zone| zone.raw_bits());
        city.set_zone(cx, cy, (old_zone & 0x0f) | corner_markers[index]);
    }
    MacSpecialTileAttempt::Placed
}

/// CODE 8 `export_241`'s persisted zone-7 selector arms. Each eligible
/// selector consumes one QuickDraw WORD and accepts when its low
/// two bits are zero. Fallbacks are caller-owned and happen only when the
/// typed helper refuses admission.
pub(in crate::app) fn mac_budget_process_zone_seven_special_growth(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) -> MacSpecialTileAttempt {
    let event = city
        .mac_state_misc_u32_at(MAC_SPECIAL_RCI_EVENT_MISC_OFFSET)
        .unwrap_or(0) as u8;
    if event == 5 {
        if city.building_at(x, y) == 0xf9 {
            return MacSpecialTileAttempt::Refused;
        }
        return mac_budget_try_place_zone_seven_f9_footprint(city, x, y);
    }
    if !matches!(event, 2 | 3 | 4) {
        return MacSpecialTileAttempt::Refused;
    }
    if rng.random_i16() as u16 & 3 != 0 {
        return MacSpecialTileAttempt::Refused;
    }
    let Some(tile) = mac_budget_zone_seven_special_growth_xbld(event, |class| {
        city.mac_zone7_census_count(class)
    }) else {
        return MacSpecialTileAttempt::Refused;
    };

    let attempt = match tile {
        0xdd => mac_budget_try_place_zone_runway_ray(city, x, y, 7),
        0xe0 => mac_budget_try_place_zone_water_ray(city, x, y, 7),
        0xe1 | 0xe2 | 0xe3 | 0xe4 | 0xe5 | 0xe7 | 0xe8 | 0xea => {
            mac_budget_try_place_zone_single_tile(city, x, y, 7, tile)
        }
        0xef | 0xf1 | 0xf2 | 0xf6 => mac_budget_try_place_zone_square(city, x, y, 7, tile),
        _ => MacSpecialTileAttempt::Refused,
    };

    if attempt != MacSpecialTileAttempt::Refused {
        return attempt;
    }
    match event {
        4 => mac_budget_try_place_zone_single_tile(city, x, y, 7, 0xe3),
        2 => mac_budget_try_place_zone_single_tile(city, x, y, 7, 0xe8),
        _ => attempt,
    }
}

/// CODE 8 `export_241`'s `D5 == 9` late-growth arm. The normal selector path
/// retries failed typed placement as `0xe3`; the alternate `0xe0` path creates
/// the Mac kind-3 boundary record through relocated CODE 6 export 116.
pub(in crate::app) fn mac_budget_process_zone_nine_special_growth(
    city: &mut City,
    x: usize,
    y: usize,
    rng: &mut MacToolboxRng,
) -> MacSpecialTileAttempt {
    let first = rng.random_i16() as u16;
    if first & 3 == 0 {
        let Some(tile) =
            mac_budget_zone_special_growth_xbld(9, |tile| city.mac_xbld_tile_count(tile))
        else {
            return MacSpecialTileAttempt::Refused;
        };
        let attempt = match tile {
            0xe0 => mac_budget_try_place_zone_water_ray(city, x, y, 9),
            0xe3 => mac_budget_try_place_zone_single_tile(city, x, y, 9, 0xe3),
            0xf0 | 0xf2 => mac_budget_try_place_zone_square(city, x, y, 9, tile),
            _ => MacSpecialTileAttempt::Refused,
        };
        return if attempt == MacSpecialTileAttempt::Refused {
            mac_budget_try_place_zone_single_tile(city, x, y, 9, 0xe3)
        } else {
            attempt
        };
    }

    if city.building_at(x, y) == 0xe0 {
        let second = rng.random_i16() as u16;
        if second & 3 == 0 {
            mac_budget_try_create_zone_nine_type_three(city, x, y, rng);
        }
    }
    MacSpecialTileAttempt::Refused
}

/// CODE 6 export 116: one bounded kind-3 record may be installed at an XTER
/// `0x10` boundary cell. Record bytes not written by the Mac constructor are
/// preserved, as they are in the original 12-byte slot table.
pub(in crate::app) fn mac_budget_try_create_zone_nine_type_three(
    city: &mut City,
    source_x: usize,
    source_y: usize,
    rng: &mut MacToolboxRng,
) -> bool {
    // The original process always allocates this table; older Mac cities may
    // omit the optional serialized chunk, so materialize its runtime slots.
    if city.traffic_overlays().is_none() {
        let _ = city.mac_object_slot_records_mut();
    }
    // Native export 116 gates on the transient sweep counter, not a live-slot
    // census. A saved kind-3 slot can coexist with a zero counter before its
    // first sweep; a visited actor can leave the counter nonzero after changing
    // kind. Preserve that native chronology instead of reconstructing from XTHG.
    if city.mac_kind3_actor_counter >= 1 {
        return false;
    }

    // export 116 consumes this draw after the active-count gate and before
    // edge scanning, even when no eligible XTER cell or free slot exists.
    let edge = (rng.random_i16() as u16 & 3) as usize;
    let mut selected = None;
    for offset in 0..CITY_WIDTH {
        // Export 116 indexes the XTER row-pointer table by fixed X for
        // selectors 0/1, then scans Y; selectors 2/3 fix Y and scan X.
        let (x, y) = match edge {
            0 => (2, offset),
            1 => (CITY_WIDTH - 2, offset),
            2 => (offset, 2),
            _ => (offset, CITY_HEIGHT - 2),
        };
        if city.terrain_at(x, y) == 0x10 {
            selected = Some((x, y));
        }
    }
    let Some((edge_x, edge_y)) = selected else {
        return false;
    };
    let saved_xtxt = city.text_overlay_at(edge_x, edge_y).unwrap_or(0);
    if saved_xtxt >= 0xc9 {
        return false;
    }

    let records = city.mac_object_slot_records_mut();
    let Some(slot) = records
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(slot, record)| (record.kind == 0).then_some(slot))
    else {
        return false;
    };

    let direction =
        crate::mac_traffic_geometry::mac_direction_to_tile(edge_x, edge_y, source_x, source_y);
    let record = &mut records[slot];
    record.kind = 3;
    record.lane = direction;
    record.state = 0;
    // The native map pointer is x*128+y, and record bytes +3/+4 preserve that
    // same raw coordinate order.
    record.tile_x = edge_x as u8;
    record.tile_y = edge_y as u8;
    record.height = 1;
    record.sub_x = 8;
    record.sub_y = 8;
    record.saved_a = saved_xtxt;

    // Export 116 stores the actor proxy in XTXT, bypassing PlaceTile and the
    // saved general-XBLD census. The previous XTXT byte is kept in record +0x0a.
    // The native process globals mirror this record's selected boundary
    // coordinate and are later copied into +8/+9 if phase 3 is reached.
    city.mac_kind3_boundary_target = (edge_x as u8, edge_y as u8);
    city.set_text_overlay(edge_x, edge_y, (slot as u8).wrapping_sub(0x37));
    city.mac_kind3_actor_counter = city.mac_kind3_actor_counter.wrapping_add(1);
    true
}

pub(in crate::app) fn mac_budget_cleanup_replaced_high_building(
    city: &mut City,
    x: usize,
    y: usize,
) {
    let building = city.building_at(x, y);
    if building <= 0xc5 {
        return;
    }
    let cells = if (0xdb..=0xea).contains(&building) {
        vec![(x, y)]
    } else {
        let left = x & !1;
        let top = y & !1;
        vec![
            (left, top),
            (left + 1, top),
            (left, top + 1),
            (left + 1, top + 1),
        ]
    };
    for (cx, cy) in cells {
        city.set_building(cx, cy, 0);
        let bits = city.tile_bits_at(cx, cy).unwrap_or_default();
        city.set_tile_bits(cx, cy, TileBits::new(bits.raw_bits() & 0x3f));
        let zone = city.zone_at(cx, cy).map_or(0, |zone| zone.raw_bits());
        city.set_zone(cx, cy, zone & 0x0f);
    }
}
