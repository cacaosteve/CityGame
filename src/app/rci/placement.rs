use super::*;

pub(in crate::app) const TILE_ROAD_LR: u8 = 29;

pub(in crate::app) fn rci_tile_is_vacant_growth_surface(building: u8) -> bool {
    building < TILE_ROAD_LR
}

/// Win95 `CheckTileArea` (`0x438160`) max XBLD for StartBuilding pop1→2.
pub(in crate::app) const CHECK_TILE_AREA_POP1_MAX_XBLD: u8 = 0x8C;
/// Win95 `CheckTileArea` (`0x438160`) max XBLD for StartBuilding pop3→4.
pub(in crate::app) const CHECK_TILE_AREA_POP3_MAX_XBLD: u8 = 0xAE;

pub(in crate::app) fn check_tile_area_max_xbld_for_start_pop(current_pop: u8) -> Option<u8> {
    match current_pop {
        1 => Some(CHECK_TILE_AREA_POP1_MAX_XBLD),
        3 => Some(CHECK_TILE_AREA_POP3_MAX_XBLD),
        _ => None,
    }
}

/// Win95 `CheckTileArea` (`0x438160`): bounds, matching ALTM `&0x1f` and XZON `&0xf`,
/// XBLD `< max`, and not a transit tile. Pop1 size-up passes `max = 0x8C`.
pub(in crate::app) fn check_tile_area(
    city: &City,
    x: usize,
    y: usize,
    altitude: u8,
    zone_id: u8,
    max_building: u8,
) -> bool {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return false;
    }
    if city.altitude_at(x, y) & 0x1f != altitude & 0x1f {
        return false;
    }
    if !city
        .zone_at(x, y)
        .is_some_and(|zone| zone.zone_id() == zone_id)
    {
        return false;
    }
    check_tile_area_xbld_allowed(city.building_at(x, y), max_building)
}

pub(in crate::app) fn check_tile_area_xbld_allowed(building: u8, max_building: u8) -> bool {
    building < max_building && !check_tile_area_is_transit(building)
}

/// Transit ranges `CheckTileArea` rejects after the max-XBLD test.
fn check_tile_area_is_transit(building: u8) -> bool {
    matches!(
        building,
        0x1D..=0x2B
            | 0x2C..=0x3E
            | 0x3F..=0x48
            | 0x4B..=0x4E
            | 0x5D..=0x60
            | 0x6C..=0x6F
    )
}

/// StartBuilding pop1 2×2 northwest corners, PE order: 1×1 as NW, SW, NE, then SE.
pub(in crate::app) fn pop1_startbuilding_quadrant_origins(
    x: usize,
    y: usize,
) -> impl Iterator<Item = (usize, usize)> {
    [
        Some((x, y)),
        y.checked_sub(1).map(|oy| (x, oy)),
        x.checked_sub(1).map(|ox| (ox, y)),
        match (x.checked_sub(1), y.checked_sub(1)) {
            (Some(ox), Some(oy)) => Some((ox, oy)),
            _ => None,
        },
    ]
    .into_iter()
    .flatten()
}

/// Invert a current-pop-1 StartBuilding 2×2 stamp origin to the raw
/// ChooseAndPlace(pop=2) call tile; PE stamps one row above the call tile.
pub(in crate::app) fn pop1_startbuilding_candidate_call_from_origin(
    origin_x: usize,
    origin_y: usize,
) -> Option<(usize, usize)> {
    Some((origin_x, origin_y.checked_add(1)?))
}

/// A raw pop-2 ChooseAndPlace call at `(x, y)` stamps its 2×2 from `(x, y - 1)`.
/// StartBuilding and ChangeBuilding both use this fixed PE Y conversion.
pub(in crate::app) fn pop2_choose_and_place_stamp_origin(
    x: usize,
    y: usize,
) -> Option<(usize, usize)> {
    y.checked_sub(1).map(|origin_y| (x, origin_y))
}

/// Win95 ChooseAndPlace pop≥2 (`0x436e80`): call coords `x,y >= 2` and `<= 0x7e - pop/2`.
pub(in crate::app) const CHOOSE_AND_PLACE_MULTITILE_MIN: usize = 2;
pub(in crate::app) const CHOOSE_AND_PLACE_MULTITILE_MAX: usize = 0x7e;

pub(in crate::app) fn choose_and_place_half_pop(pop: u8) -> usize {
    usize::from(pop / 2)
}

/// `(x, y)` are PE ChooseAndPlace arguments (south-west of the footprint).
pub(in crate::app) fn choose_and_place_multitile_call_in_bounds(
    x: usize,
    y: usize,
    pop: u8,
) -> bool {
    if pop <= 1 {
        return true;
    }
    let max = CHOOSE_AND_PLACE_MULTITILE_MAX.saturating_sub(choose_and_place_half_pop(pop));
    x >= CHOOSE_AND_PLACE_MULTITILE_MIN
        && y >= CHOOSE_AND_PLACE_MULTITILE_MIN
        && x <= max
        && y <= max
}

/// Our stamp origin is northwest; PE calls ChooseAndPlace at the south-west tile.
pub(in crate::app) fn choose_and_place_nw_origin_in_bounds(
    origin_x: usize,
    origin_y: usize,
    pop: u8,
) -> bool {
    choose_and_place_multitile_call_in_bounds(
        origin_x,
        origin_y.saturating_add(choose_and_place_half_pop(pop)),
        pop,
    )
}

/// PE ChooseAndPlace picks a building before it checks multi-tile call bounds.
pub(in crate::app) fn pe_choose_and_place_then_origin_bounds<T>(
    choose_building: impl FnOnce() -> Option<T>,
    origin_x: usize,
    origin_y: usize,
    pop: u8,
) -> Option<T> {
    let building = choose_building()?;
    choose_and_place_nw_origin_in_bounds(origin_x, origin_y, pop).then_some(building)
}

/// StartBuilding current-pop-3 3×3 stamp origins from its raw call tile `(x, y)`.
/// PE checks call tiles `(x-(i&1), y+(i>>1))`; ChooseAndPlace subtracts 2 from Y.
pub(in crate::app) fn pop3_startbuilding_quadrant_origins(
    x: usize,
    y: usize,
) -> impl Iterator<Item = (usize, usize)> {
    [
        y.checked_sub(2).map(|oy| (x, oy)),
        match (x.checked_sub(1), y.checked_sub(2)) {
            (Some(ox), Some(oy)) => Some((ox, oy)),
            _ => None,
        },
        y.checked_sub(1).map(|oy| (x, oy)),
        match (x.checked_sub(1), y.checked_sub(1)) {
            (Some(ox), Some(oy)) => Some((ox, oy)),
            _ => None,
        },
    ]
    .into_iter()
    .flatten()
}

/// Invert a current-pop-3 StartBuilding 3×3 stamp origin to the raw
/// ChooseAndPlace(pop=4) call tile; PE stamps two rows above that call tile.
pub(in crate::app) fn pop3_startbuilding_candidate_call_from_origin(
    origin_x: usize,
    origin_y: usize,
) -> Option<(usize, usize)> {
    Some((origin_x, origin_y.checked_add(2)?))
}

/// StartBuilding pop 3 chooses its pop-4 variant before ChooseAndPlace checks
/// the raw call coordinate, then stamps the 3×3 from `(call_x, call_y - 2)`.
pub(in crate::app) fn pe_startbuilding_pop4_direct_placement_with_picker<T>(
    call_x: usize,
    call_y: usize,
    choose_building: impl FnOnce() -> Option<T>,
) -> Option<(usize, usize, T)> {
    let building = choose_building()?;
    let stamp_y = call_y.checked_sub(2)?;
    pe_choose_and_place_then_origin_bounds(|| Some(building), call_x, stamp_y, 4)
        .map(|building| (call_x, stamp_y, building))
}

/// StartBuilding pop 2 chooses its pop-3 variant before the bounds check, then
/// stamps the 2×2 from `(call_x, call_y - 1)`.
pub(in crate::app) fn pe_startbuilding_pop3_direct_placement_with_picker<T>(
    call_x: usize,
    call_y: usize,
    choose_building: impl FnOnce() -> Option<T>,
) -> Option<(usize, usize, T)> {
    let building = choose_building()?;
    let stamp_y = call_y.checked_sub(1)?;
    pe_choose_and_place_then_origin_bounds(|| Some(building), call_x, stamp_y, 3)
        .map(|building| (call_x, stamp_y, building))
}

/// StartBuilding pop 1 chooses its pop-2 variant before ChooseAndPlace checks
/// the raw call coordinate, then stamps the 2×2 from `(call_x, call_y - 1)`.
pub(in crate::app) fn pe_startbuilding_pop2_quadrant_placement_with_picker<T>(
    call_x: usize,
    call_y: usize,
    choose_building: impl FnOnce() -> Option<T>,
) -> Option<(usize, usize, T)> {
    let building = choose_building()?;
    let stamp_y = call_y.checked_sub(1)?;
    choose_and_place_multitile_call_in_bounds(call_x, call_y, 2)
        .then_some((call_x, stamp_y, building))
}

/// Pop3 StartBuilding DestroyBuilding probes (`0x437a88`) from 3×3 northwest origin.
/// PE uses the south-west call tile and skips the center.
pub(in crate::app) fn pop3_startbuilding_destroy_coords(
    origin_x: usize,
    origin_y: usize,
) -> impl Iterator<Item = (usize, usize)> {
    [
        (0usize, 2usize),
        (0, 1),
        (0, 0),
        (1, 0),
        (2, 0),
        (2, 1),
        (2, 2),
        (1, 2),
    ]
    .into_iter()
    .filter_map(move |(dx, dy)| {
        origin_x
            .checked_add(dx)
            .and_then(|x| origin_y.checked_add(dy).map(|y| (x, y)))
    })
    .filter(|&(x, y)| x < CITY_WIDTH && y < CITY_HEIGHT)
}

/// Road IDs whose connections point into the 3×3 from that diagonal.
const POP3_ROAD_SW: [u8; 4] = [0x23, 0x27, 0x28, 0x2B];
const POP3_ROAD_NW: [u8; 4] = [0x24, 0x28, 0x29, 0x2B];
const POP3_ROAD_NE: [u8; 4] = [0x25, 0x29, 0x2A, 0x2B];
const POP3_ROAD_SE: [u8; 4] = [0x26, 0x2A, 0x27, 0x2B];

/// Pop3 StartBuilding road-orientation probes (`0x43791e`) from south-west `(di, si)`.
/// PE skips a probe that is out of those signed bounds rather than reading it.
pub(in crate::app) fn pop3_startbuilding_road_probes(
    di: usize,
    si: usize,
) -> impl Iterator<Item = (usize, usize, &'static [u8])> {
    [
        (di > 0 && si < 0x7f)
            .then(|| di.checked_sub(1).zip(si.checked_add(1)))
            .flatten()
            .filter(|&(x, y)| x < CITY_WIDTH && y < CITY_HEIGHT)
            .map(|(x, y)| (x, y, POP3_ROAD_SW.as_slice())),
        (di > 0 && si > 2)
            .then(|| di.checked_sub(1).zip(si.checked_sub(3)))
            .flatten()
            .filter(|&(x, y)| x < CITY_WIDTH && y < CITY_HEIGHT)
            .map(|(x, y)| (x, y, POP3_ROAD_NW.as_slice())),
        (di < 0x7d && si > 2)
            .then(|| di.checked_add(3).zip(si.checked_sub(3)))
            .flatten()
            .filter(|&(x, y)| x < CITY_WIDTH && y < CITY_HEIGHT)
            .map(|(x, y)| (x, y, POP3_ROAD_NE.as_slice())),
        (di < 0x7d && si < 0x7f)
            .then(|| di.checked_add(3).zip(si.checked_add(1)))
            .flatten()
            .filter(|&(x, y)| x < CITY_WIDTH && y < CITY_HEIGHT)
            .map(|(x, y)| (x, y, POP3_ROAD_SE.as_slice())),
    ]
    .into_iter()
    .flatten()
}

/// True if any pop3 diagonal road probe matches PE's orientation list.
pub(in crate::app) fn pop3_startbuilding_has_road_orientation(
    city: &City,
    origin_x: usize,
    origin_y: usize,
) -> bool {
    let Some(call_y) = origin_y.checked_add(2) else {
        return false;
    };
    pop3_startbuilding_road_probes(origin_x, call_y)
        .any(|(x, y, ids)| ids.contains(&city.building_at(x, y)))
}

pub(in crate::app) fn vacant_rci_zone_capacity_for_city(city: &City) -> [u32; 3] {
    let mut counts = [0_u32; 3];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if is_water_terrain(city.terrain_at(x, y)) {
                continue;
            }
            let Some(zone) = city.zone_at(x, y) else {
                continue;
            };
            let zone_id = zone.zone_id();
            let Some(kind) = rci_kind_for_zone(zone_id) else {
                continue;
            };
            let building = city.building_at(x, y);
            if !(rci_tile_is_vacant_growth_surface(building) || is_abandoned_building(building)) {
                continue;
            }
            let index = rci_kind_index(kind);
            counts[index] = counts[index].saturating_add(rci_zone_capacity_weight(zone_id));
        }
    }
    counts
}

pub(in crate::app) fn rci_zone_capacity_weight(zone_id: u8) -> u32 {
    match zone_id {
        ZONE_RESIDENTIAL_DENSE | ZONE_COMMERCIAL_DENSE | ZONE_INDUSTRIAL_DENSE => 2,
        _ => 1,
    }
}

pub(in crate::app) fn rci_zone_has_saved_demand(city: &City, zone_id: u8) -> bool {
    // Missing MISC must not force growth; monthly refresh writes real values first.
    // Written demand of 0 still StartBuilding via `iCurrentDemand = demand + 2000`.
    rci_kind_for_zone(zone_id)
        .and_then(|kind| saved_rci_demand_for_kind(city, kind))
        .is_some()
}

pub(in crate::app) fn rci_development_candidate_sizes(zone_id: u8) -> &'static [usize] {
    // Win95 `StartBuilding(0)` on empty tiles always begins at 1×1 construction.
    // Dense size-up happens from existing buildings via the pop-level ladder.
    match zone_id {
        ZONE_RESIDENTIAL_LIGHT
        | ZONE_COMMERCIAL_LIGHT
        | ZONE_INDUSTRIAL_LIGHT
        | ZONE_RESIDENTIAL_DENSE
        | ZONE_COMMERCIAL_DENSE
        | ZONE_INDUSTRIAL_DENSE => RCI_DEVELOPMENT_SIZES_VACANT,
        _ => &[],
    }
}

pub(in crate::app) fn rci_redevelopment_origin_can_start_at(
    city: &City,
    x: usize,
    y: usize,
    zone_id: u8,
    kind: RciKind,
) -> bool {
    let building = city.building_at(x, y);
    if rci_tile_is_vacant_growth_surface(building) {
        return true;
    }
    let size = usize::from(xbld_tile_size(building).max(1));
    if is_construction_building(building) {
        // Failed BUILD_THINK keeps GROWTH_CONSIDERCONSTRUCTION and may StartBuilding
        // at the construction pop from the view-angle corner. Successful complete
        // reserves and does not fall through.
        return rci_view_corner_footprint_origin(city, x, y, building, size).is_some();
    }
    // PE `bTileState == 2` takes BUILD_ABANDON's same-population ChooseAndPlace
    // path and returns; it must not enter this StartBuilding size-up candidate.
    if rci_building_kind(building) != Some(kind) {
        return false;
    }
    rci_development_building_allowed(zone_id, size, building)
        && rci_view_corner_footprint_origin(city, x, y, building, size).is_some()
}

pub(in crate::app) fn rci_development_building(
    zone_id: u8,
    size: usize,
    x: usize,
    y: usize,
    quality_band: usize,
) -> Option<u8> {
    let pop = size_to_default_pop_level(size)?;
    if rci_kind_for_zone(zone_id) == Some(RciKind::Residential) && size == 1 {
        // Same ChooseAndPlace CRT as XVAL `min(value>>6, 2)*4 + (rand&3)`.
        let xval = u8::try_from(quality_band.min(2) << 6).ok()?;
        return rci_choose_and_place_building(zone_id, pop, x, y, Some(xval))
            .filter(|&building| rci_development_building_allowed(zone_id, size, building));
    }
    let buildings = rci_development_buildings_for_zone_and_size(zone_id, size)?;
    let (start, end) = rci_development_variant_band_range(zone_id, size, buildings, quality_band);
    let variants = &buildings[start..end.max(start + 1)];
    if variants.is_empty() {
        return None;
    }
    let mut rng = rci_choose_and_place_isolated_rng(x, y, zone_id, pop);
    let count = u16::try_from(variants.len()).ok()?.max(1);
    let building = variants[usize::from(rng.random_mod(count))];
    rci_development_building_allowed(zone_id, size, building).then_some(building)
}

pub(in crate::app) fn rci_development_building_for_pop(
    zone_id: u8,
    pop: u8,
    x: usize,
    y: usize,
) -> Option<u8> {
    rci_choose_and_place_building(zone_id, pop, x, y, None)
}

/// Isolated CRT stream for `ChooseAndPlace` (`0x48cdd8`). Per-tile seed keeps
/// occupancy/poll sequences stable while still using MSVC `rand`, not XOR.
pub(in crate::app) fn rci_choose_and_place_isolated_rng(
    x: usize,
    y: usize,
    table_key: u8,
    pop: u8,
) -> Win95CrtRand {
    Win95CrtRand::new(
        Win95CrtRand::DEFAULT_SEED
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(x as u32)
            .wrapping_add((y as u32) << 8)
            .wrapping_add(u32::from(pop) << 16)
            .wrapping_add(u32::from(table_key) << 24),
    )
}

/// Isolation key for StartBuilding's `ChooseAndPlace(..., mode=3)` construction slice.
/// High bit keeps it off raw zone id 3 (`ZONE_COMMERCIAL_LIGHT`).
pub(in crate::app) const CHOOSE_AND_PLACE_CONSTRUCTION_MODE: u8 = 0x83;

fn construction_building_for_size(size: usize, x: usize, y: usize) -> Option<u8> {
    construction_building_for_pop(size_to_default_pop_level(size)?, x, y)
}

pub(in crate::app) fn is_construction_building(building: u8) -> bool {
    CONSTRUCTION_1X1_BUILDINGS.contains(&building)
        || CONSTRUCTION_2X2_BUILDINGS.contains(&building)
        || CONSTRUCTION_3X3_BUILDINGS.contains(&building)
}

pub(in crate::app) fn construction_building_for_pop(pop: u8, x: usize, y: usize) -> Option<u8> {
    // Win95 `StartBuilding` (`0x4375c0`): `ChooseAndPlace(x,y,pop,3)` →
    // `base[pop+12] + rand%count` (`0x88`/`0xA6`/`0xA8`/`0xC2`, count 2).
    let buildings = match pop {
        1 => CONSTRUCTION_POP1_BUILDINGS.as_slice(),
        2 => CONSTRUCTION_POP2_BUILDINGS.as_slice(),
        3 => CONSTRUCTION_POP3_BUILDINGS.as_slice(),
        4 => CONSTRUCTION_POP4_BUILDINGS.as_slice(),
        _ => return None,
    };
    let mut rng = rci_choose_and_place_isolated_rng(x, y, CHOOSE_AND_PLACE_CONSTRUCTION_MODE, pop);
    let count = u16::try_from(buildings.len()).ok()?.max(1);
    Some(buildings[usize::from(rng.random_mod(count))])
}

/// Live PE construction `ChooseAndPlace` mode 3 using process CRT (`0x48cdd8`).
pub(in crate::app) fn construction_building_for_pop_with_rng(
    pop: u8,
    rng: &mut Win95CrtRand,
) -> Option<u8> {
    let buildings = match pop {
        1 => CONSTRUCTION_POP1_BUILDINGS.as_slice(),
        2 => CONSTRUCTION_POP2_BUILDINGS.as_slice(),
        3 => CONSTRUCTION_POP3_BUILDINGS.as_slice(),
        4 => CONSTRUCTION_POP4_BUILDINGS.as_slice(),
        _ => return None,
    };
    let count = u16::try_from(buildings.len()).ok()?.max(1);
    Some(buildings[usize::from(rng.random_mod(count))])
}

/// Tests keep isolation; live callers pass `Some(pe_process_crt)`.
pub(in crate::app) fn construction_building_for_pop_optional(
    pop: u8,
    x: usize,
    y: usize,
    process_crt: Option<&mut Win95CrtRand>,
) -> Option<u8> {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        return construction_building_for_pop(pop, x, y);
    }
    match process_crt {
        Some(rng) => construction_building_for_pop_with_rng(pop, rng),
        None => construction_building_for_pop(pop, x, y),
    }
}

/// Isolation key for ChangeBuilding's `ChooseAndPlace(..., mode=4)` abandon slice.
/// High bit keeps it off raw zone id 4 (`ZONE_COMMERCIAL_DENSE`).
pub(in crate::app) const CHOOSE_AND_PLACE_ABANDONED_MODE: u8 = 0x84;

pub(in crate::app) fn abandoned_building_for_pop(pop: u8, x: usize, y: usize) -> Option<u8> {
    // Win95 `ChangeBuilding` (`0x437d40`): `ChooseAndPlace(x,y,pop,4)` →
    // `base[pop+16] + rand%count` (`0x8A`/`0xAA`/`0xAC`/`0xC4`, count 2).
    let buildings = match pop {
        1 => ABANDONED_POP1_BUILDINGS.as_slice(),
        2 => ABANDONED_POP2_BUILDINGS.as_slice(),
        3 => ABANDONED_POP3_BUILDINGS.as_slice(),
        4 => ABANDONED_POP4_BUILDINGS.as_slice(),
        _ => return None,
    };
    let mut rng = rci_choose_and_place_isolated_rng(x, y, CHOOSE_AND_PLACE_ABANDONED_MODE, pop);
    let count = u16::try_from(buildings.len()).ok()?.max(1);
    Some(buildings[usize::from(rng.random_mod(count))])
}

/// Live PE abandon `ChooseAndPlace` mode 4 using process CRT (`0x48cdd8`).
pub(in crate::app) fn abandoned_building_for_pop_with_rng(
    pop: u8,
    rng: &mut Win95CrtRand,
) -> Option<u8> {
    let buildings = match pop {
        1 => ABANDONED_POP1_BUILDINGS.as_slice(),
        2 => ABANDONED_POP2_BUILDINGS.as_slice(),
        3 => ABANDONED_POP3_BUILDINGS.as_slice(),
        4 => ABANDONED_POP4_BUILDINGS.as_slice(),
        _ => return None,
    };
    let count = u16::try_from(buildings.len()).ok()?.max(1);
    Some(buildings[usize::from(rng.random_mod(count))])
}

/// Tests keep isolation; live callers pass `Some(pe_process_crt)`.
pub(in crate::app) fn abandoned_building_for_pop_optional(
    pop: u8,
    x: usize,
    y: usize,
    process_crt: Option<&mut Win95CrtRand>,
) -> Option<u8> {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        return abandoned_building_for_pop(pop, x, y);
    }
    match process_crt {
        Some(rng) => abandoned_building_for_pop_with_rng(pop, rng),
        None => abandoned_building_for_pop(pop, x, y),
    }
}

pub(in crate::app) fn rci_choose_and_place_building(
    zone_id: u8,
    pop: u8,
    x: usize,
    y: usize,
    land_value: Option<u8>,
) -> Option<u8> {
    let mut rng = rci_choose_and_place_isolated_rng(x, y, zone_id, pop);
    rci_choose_and_place_building_with_rng(zone_id, pop, land_value, &mut rng)
}

pub(in crate::app) fn rci_choose_and_place_building_with_rng(
    zone_id: u8,
    pop: u8,
    land_value: Option<u8>,
    rng: &mut Win95CrtRand,
) -> Option<u8> {
    if !(1..=4).contains(&pop) {
        return None;
    }
    let kind = rci_kind_for_zone(zone_id)?;
    if kind == RciKind::Residential && pop == 1 {
        // Win95 `0x436ea0`: `min(XVAL>>6, 2)*4 + (rand&3)` from base `0x4E7560` (0x70).
        let band = land_value
            .map(|value| usize::from(value >> 6).min(2))
            .unwrap_or(0);
        let variant = usize::from(rng.random_u16_mask(3));
        return Some(RESIDENTIAL_1X1_BUILDINGS[band * 4 + variant]);
    }
    // Win95 `0x4E755E`/`0x4E7586`: building = base[pop+zone*4] + rand%count.
    let buildings = rci_development_buildings_for_zone_and_pop(zone_id, pop)?;
    let count = u16::try_from(buildings.len()).ok()?.max(1);
    Some(buildings[usize::from(rng.random_mod(count))])
}

pub(in crate::app) fn rci_choose_and_place_for_city(
    city: &City,
    zone_id: u8,
    pop: u8,
    x: usize,
    y: usize,
    quality_band: usize,
) -> Option<u8> {
    if rci_kind_for_zone(zone_id) == Some(RciKind::Residential) && pop == 1 {
        if let Some(value) = pe_xval_byte_at_city_coords(city, x, y) {
            return rci_choose_and_place_building(zone_id, pop, x, y, Some(value));
        }
        // CityGame fallback only. PE new-city setup clears XVAL, but the PE
        // loader does not establish a default for a malformed file omitting
        // the XVAL chunk; this quality-band choice is not parity for that case.
        return rci_development_building(zone_id, 1, x, y, quality_band);
    }
    rci_choose_and_place_building(zone_id, pop, x, y, pe_xval_byte_at_city_coords(city, x, y))
}

/// Live PE finished-building `ChooseAndPlace` using process CRT (`0x48cdd8`).
pub(in crate::app) fn rci_choose_and_place_for_city_optional(
    city: &City,
    zone_id: u8,
    pop: u8,
    x: usize,
    y: usize,
    quality_band: usize,
    process_crt: Option<&mut Win95CrtRand>,
) -> Option<u8> {
    #[cfg(test)]
    if use_isolated_growth_rng() {
        return rci_choose_and_place_for_city(city, zone_id, pop, x, y, quality_band);
    }
    let Some(rng) = process_crt else {
        return rci_choose_and_place_for_city(city, zone_id, pop, x, y, quality_band);
    };
    if rci_kind_for_zone(zone_id) == Some(RciKind::Residential) && pop == 1 {
        if let Some(value) = pe_xval_byte_at_city_coords(city, x, y) {
            return rci_choose_and_place_building_with_rng(zone_id, pop, Some(value), rng);
        }
        // CityGame-only missing-XVAL fallback. PE new-city setup clears the
        // plane; a missing chunk in a malformed save has no recovered default.
        // Keep this resilience path out of the PE CRT stream.
        return rci_development_building(zone_id, 1, x, y, quality_band);
    }
    rci_choose_and_place_building_with_rng(
        zone_id,
        pop,
        pe_xval_byte_at_city_coords(city, x, y),
        rng,
    )
}
