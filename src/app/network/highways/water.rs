//! Water-route preflight and bridge-candidate classification for highways.
//!
//! The routines here inspect XBIT, XTER, zone, and elevation state only;
//! bridge placement and resource changes remain in the highway and bridge
//! workers.

use super::super::HighwayWaterBridgeCandidate;
use super::{
    highway_canonical_origin, pe_highway_altitude_463f60, pe_highway_gate_early_8000_4640a1,
    pe_highway_route_gate_low_word_464040, pe_highway_route_step_coordinate_461ae0,
};
use crate::app::PE_PIPE_WALK_DIRECTIONS_4E7B18;
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

pub(in crate::app) fn pe_highway_route_footprint_has_water_xbit_461ae0(
    city: &City,
    origin: (usize, usize),
) -> bool {
    [(0, 0), (1, 0), (1, 1), (0, 1)]
        .into_iter()
        .any(|(dx, dy)| {
            city.tile_bits_at(origin.0 + dx, origin.1 + dy)
                .is_some_and(|bits| bits.water_tile())
        })
}

/// PE `FUN_0045C1E0` lookup at `0x4e85f8`, indexed by the saved XTER byte.
///
/// Extracted from the hash-matched Win95 image (image base `0x400000`). The
/// worker combines four results in SE, SW, NW, NE order to classify a 2×2
/// footprint. Keeping the compact ranges here avoids replacing the saved XTER
/// worker input with a terrain-color or XBIT-only guess.
pub(in crate::app) fn pe_highway_water_xter_class_4e85f8(xter: u8) -> u16 {
    match xter {
        0x00 => 0x1000,
        0x01..=0x0f => 0x0100,
        0x10..=0x20 | 0x30 => 0x0010,
        0x21..=0x2f
        | 0x31..=0x3f
        | 0x60..=0x7b
        | 0xb6..=0xb8
        | 0xcb..=0xda
        | 0xf1
        | 0xf6
        | 0xf9
        | 0xfe => 0x0001,
        0x40..=0x45 => 0x1000,
        0x46..=0x5d | 0x5f | 0xf0 | 0xf2 | 0xf5 | 0xf7..=0xf8 | 0xfa | 0xfd | 0xff => 0x0000,
        0x5e | 0xf3..=0xf4 | 0xfb..=0xfc => 0xffff,
        0x7c..=0x9d | 0xdb..=0xe7 => 0x0002,
        0x9e..=0xb5 | 0xc0..=0xc6 | 0xe8..=0xea => 0x0003,
        0xb9..=0xbf | 0xc7..=0xca | 0xeb..=0xef => 0x0004,
    }
}

pub(in crate::app) fn pe_highway_water_footprint_class_45c1e0(
    city: &City,
    origin: (usize, usize),
) -> u16 {
    let (x, y) = origin;
    let xter = |x, y| u16::from(pe_highway_water_xter_class_4e85f8(city.terrain_at(x, y)));
    // The two adjacent row-pointer tables at 0x4c9f58/0x4c9f5c are x and
    // x+1. Native accumulation order is (x,y+1), (x+1,y+1), (x+1,y), (x,y).
    let x_y1 = xter(x, y + 1);
    let x1_y1 = xter(x + 1, y + 1);
    let x1_y = xter(x + 1, y);
    let x_y = xter(x, y);
    (((x_y1 * 2 + x1_y1) * 2 + x1_y) * 2) + x_y
}

pub(in crate::app) fn pe_highway_water_footprint_zone_valid_45c1e0(
    city: &City,
    origin: (usize, usize),
) -> bool {
    [(0, 0), (1, 0), (1, 1), (0, 1)]
        .into_iter()
        .all(|(dx, dy)| {
            origin.0 + dx < CITY_WIDTH
                && origin.1 + dy < CITY_HEIGHT
                && city
                    .zone_at(origin.0 + dx, origin.1 + dy)
                    .is_none_or(|zone| zone.raw_bits() & 0x0f < 0x0e)
        })
}

pub(in crate::app) fn pe_highway_water_mode1_direction_45c1e0(
    city: &City,
    seed: (usize, usize),
) -> Option<u8> {
    highway_water_origin_direction(
        pe_highway_water_footprint_class_45c1e0(city, seed),
        city.rotation(),
    )
}

fn highway_water_origin_direction(mut pattern: u16, rotation: u8) -> Option<u8> {
    // The pattern-to-direction rows agree at Mac ffffb444 and PE 0x4e85d8.
    const XTER_PATTERN_DIRECTIONS_4E85D8: [u8; 16] = [
        0x00, 0x06, 0x0c, 0x04, 0x09, 0x00, 0x08, 0x0c, 0x03, 0x02, 0x00, 0x06, 0x01, 0x03, 0x09,
        0x00,
    ];
    if pattern & 0x0f00 != 0 {
        return None;
    }
    if (pattern >> 8) as u8 != 0 {
        pattern >>= 12;
    }
    let direction_mask = *XTER_PATTERN_DIRECTIONS_4E85D8.get((pattern & 0x0f) as usize)?;
    let rotation = rotation & 3;
    [
        rotation,
        (rotation + 2) & 3,
        (rotation + 1) & 3,
        (rotation + 3) & 3,
    ]
    .into_iter()
    .find(|direction| direction_mask & (1 << direction) != 0)
}

pub(in crate::app) fn pe_highway_water_endpoint_passes_45c1e0(
    city: &City,
    seed_height: u16,
    endpoint: (usize, usize),
    direction_bit: u8,
) -> bool {
    if !pe_highway_water_footprint_zone_valid_45c1e0(city, endpoint)
        || pe_highway_gate_early_8000_4640a1(city, endpoint.0, endpoint.1)
    {
        return false;
    }
    let endpoint_height = pe_highway_altitude_463f60(city, endpoint.0, endpoint.1);
    if endpoint_height < seed_height || endpoint_height > seed_height + 1 {
        return false;
    }
    endpoint_height != seed_height
        || pe_highway_route_gate_low_word_464040(city, endpoint.0, endpoint.1)
            & (1 << (direction_bit & 3))
            != 0
}

/// Bounded mode-0/1 route preflight for the Win95 water highway worker.
///
/// The seed is admitted by the caller's saved XBIT `0x04` test. Each next
/// 2×2 origin is classified from the four XTER bytes through `0x4e85f8` until
/// the low byte is zero. The count is the number of bridge footprints before
/// that terminal footprint. No city writes or funds changes happen here.
pub(in crate::app) fn pe_highway_water_bridge_candidate_45c1e0(
    city: &City,
    seed: (usize, usize),
    mode: u8,
    saved_direction: u8,
) -> Option<HighwayWaterBridgeCandidate> {
    let seed = highway_canonical_origin(seed);
    if seed.0 > 0x7e
        || seed.1 > 0x7e
        || !pe_highway_route_footprint_has_water_xbit_461ae0(city, seed)
        || !pe_highway_water_footprint_zone_valid_45c1e0(city, seed)
    {
        return None;
    }
    let direction = match mode {
        0 => saved_direction & 3,
        1 => pe_highway_water_mode1_direction_45c1e0(city, seed)?,
        _ => return None,
    };
    let seed_height = pe_highway_altitude_463f60(city, seed.0, seed.1);
    let mut span_count = 1usize;
    let mut current = seed;
    let mut terminal = None;
    for _ in 0..CITY_WIDTH {
        let next = pe_highway_route_step_coordinate_461ae0(current, direction)?;
        if !pe_highway_water_footprint_zone_valid_45c1e0(city, next) {
            return None;
        }
        let class = pe_highway_water_footprint_class_45c1e0(city, next);
        if class & 0x0f00 != 0 {
            return None;
        }
        if class as u8 == 0 {
            terminal = Some(next);
            break;
        }
        span_count += 1;
        current = next;
    }
    let terminal = terminal?;
    let (dx, dy) = PE_PIPE_WALK_DIRECTIONS_4E7B18[usize::from(direction)];
    let near_endpoint = (
        seed.0.checked_add_signed(-dx * 2)?,
        seed.1.checked_add_signed(-dy * 2)?,
    );
    let far_endpoint = terminal;
    let mut selector_mask = 0x20;
    if span_count > 2
        && pe_highway_water_endpoint_passes_45c1e0(city, seed_height, near_endpoint, direction)
        && pe_highway_water_endpoint_passes_45c1e0(
            city,
            seed_height,
            far_endpoint,
            direction.wrapping_add(2),
        )
    {
        selector_mask = 0x60;
    }
    Some(HighwayWaterBridgeCandidate {
        seed,
        direction,
        span_count,
        selector_mask,
        mode,
    })
}

/// CODE 13 @ 0x1fafc inspects XBLD, including the first dry footprint beyond
/// the span. The equivalent PE path uses XZON for this admission test.
pub(in crate::app) fn mac_highway_water_bridge_candidate(
    city: &City,
    seed: (usize, usize),
    mode: u8,
    saved_direction: u8,
) -> Option<HighwayWaterBridgeCandidate> {
    let seed = highway_canonical_origin(seed);
    let vacant = |(x, y)| {
        [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)]
            .into_iter()
            .all(|(tx, ty)| tx < CITY_WIDTH && ty < CITY_HEIGHT && city.building_at(tx, ty) < 0x0e)
    };
    if seed.0 > 0x7e
        || seed.1 > 0x7e
        || !pe_highway_route_footprint_has_water_xbit_461ae0(city, seed)
        || !vacant(seed)
    {
        return None;
    }
    // Mac ffffb3b8 and PE 0x4e85f8 agree over the map terrain domain.
    // Keep raw out-of-domain table reads outside the placement path.
    let footprint_class = |(x, y)| {
        [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)]
            .into_iter()
            .all(|(tx, ty)| city.terrain_at(tx, ty) <= 0x45)
            .then(|| pe_highway_water_footprint_class_45c1e0(city, (x, y)))
    };
    let direction = match mode {
        0 => saved_direction & 3,
        1 => highway_water_origin_direction(footprint_class(seed)?, city.rotation())?,
        _ => return None,
    };

    let mut span_count = 1;
    let mut current = seed;
    let terminal = loop {
        let next = pe_highway_route_step_coordinate_461ae0(current, direction)?;
        if !vacant(next) {
            return None;
        }
        let class = footprint_class(next)?;
        if class & 0x0f00 != 0 {
            return None;
        }
        if class as u8 == 0 {
            break next;
        }
        span_count += 1;
        current = next;
    };
    let opposite = (direction + 2) & 3;
    let seed_height = pe_highway_altitude_463f60(city, seed.0, seed.1);
    let ramp_allowed = |origin: (usize, usize), side: u8| {
        if !vacant(origin) {
            return false;
        }
        let height = pe_highway_altitude_463f60(city, origin.0, origin.1);
        let gate = pe_highway_route_gate_low_word_464040(city, origin.0, origin.1);
        gate != 0x8000
            && (height == seed_height + 1 || (height == seed_height && gate & (1u16 << side) != 0))
    };
    let reinforced = span_count > 2
        && pe_highway_route_step_coordinate_461ae0(seed, opposite)
            .is_some_and(|near| ramp_allowed(near, direction))
        && ramp_allowed(terminal, opposite);
    Some(HighwayWaterBridgeCandidate {
        seed,
        direction,
        span_count,
        selector_mask: if reinforced { 0x60 } else { 0x20 },
        mode,
    })
}
