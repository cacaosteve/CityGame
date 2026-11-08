//! Building-internal pipes, saved pipe membership, and water-pipe connectivity.
//!
//! This module owns the inferred pipe geometry and its display/editing
//! predicates. Saved-XBIT allocation workers remain with the water simulation
//! because their native membership contract is distinct from this geometry.

use super::*;

pub(in crate::app) fn visible_building_pipe_mask(city: &City) -> Vec<bool> {
    let mut mask = vec![false; CITY_TILES];
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let render_tile = underground_render_tile(city, x, y, city.rotation());
            let underlay_tile =
                underground_building_pipe_underlay_tile(city, x, y, city.rotation());
            let overlay_tile = underground_building_pipe_overlay_tile(city, x, y, city.rotation());
            mask[y * CITY_WIDTH + x] = render_tile
                .is_some_and(|tile| tile.source == UndergroundTileSource::BuildingInternal)
                || underlay_tile.is_some()
                || overlay_tile.is_some();
        }
    }
    mask
}

pub(in crate::app) fn building_internal_pipe_tile(city: &City, x: usize, y: usize) -> Option<u8> {
    if !building_internal_pipe_eligible(city, x, y) {
        return None;
    }

    Some(building_internal_pipe_shape_tile(city, x, y))
}

pub(in crate::app) fn building_internal_pipe_shape_tile(city: &City, x: usize, y: usize) -> u8 {
    underground_tile_for_mask(
        UndergroundKind::Pipes,
        building_internal_pipe_neighbor_mask(city, x, y),
    )
}

pub(in crate::app) fn building_internal_pipe_neighbor_mask(city: &City, x: usize, y: usize) -> u8 {
    let mut mask = 0;
    for (side, nx, ny) in directional_neighbor_tiles(x, y) {
        if water_pipe_network_tile(city, nx, ny)
            && water_pipe_tile_connects_to(city, nx, ny, opposite_network_side(side))
        {
            mask |= side;
        }
    }
    mask
}

pub(in crate::app) fn building_internal_pipe_eligible(city: &City, x: usize, y: usize) -> bool {
    if city.underground_at(x, y) != 0 {
        return false;
    }
    base_building_pipe_present(city, x, y)
}

pub(in crate::app) fn base_building_pipe_present(city: &City, x: usize, y: usize) -> bool {
    saved_base_building_pipe_present(city, x, y) || building_internal_pipe_present(city, x, y)
}

pub(in crate::app) fn saved_base_building_pipe_present(city: &City, x: usize, y: usize) -> bool {
    !underground_user_pipe_tile(city.underground_at(x, y))
        && structural_internal_pipe_eligible(city, x, y)
        && city.tile_bits_at(x, y).is_some_and(|bits| bits.piped())
}

pub(in crate::app) fn saved_base_building_pipe_watered(city: &City, x: usize, y: usize) -> bool {
    city.tile_bits_at(x, y)
        .is_some_and(|bits| bits.piped() && bits.watered())
}

pub(in crate::app) fn saved_base_pipe_bit_can_persist(city: &City, x: usize, y: usize) -> bool {
    if underground_user_pipe_tile(city.underground_at(x, y))
        || is_water_terrain(city.terrain_at(x, y))
    {
        return false;
    }
    structural_internal_pipe_eligible(city, x, y)
        || zone_serviceable_tile(city, x, y)
        || water_source_connects_as_building_pipe(city, x, y)
}

pub(in crate::app) fn building_internal_pipe_present(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    if is_power_tile(building)
        || is_road_tile(building)
        || is_rail_tile(building)
        || is_highway_tile(building)
    {
        return false;
    }
    if structural_internal_pipe_eligible(city, x, y) {
        return true;
    }
    if is_tree_building(building) {
        return false;
    }
    false
}

pub(in crate::app) fn structural_internal_pipe_eligible(city: &City, x: usize, y: usize) -> bool {
    let building = city.building_at(x, y);
    if is_power_tile(building)
        || is_road_tile(building)
        || is_rail_tile(building)
        || is_highway_tile(building)
    {
        return false;
    }
    building_has_internal_pipes(building) || contained_building_has_internal_pipes(city, x, y)
}

pub(in crate::app) fn contained_building_has_internal_pipes(
    city: &City,
    x: usize,
    y: usize,
) -> bool {
    for origin_y in y.saturating_sub(3)..=y {
        for origin_x in x.saturating_sub(3)..=x {
            if building_origin_has_internal_pipes(city, x, y, origin_x, origin_y) {
                return true;
            }
        }
    }
    for anchor_y in y.saturating_sub(3)..=(y + 3).min(CITY_HEIGHT - 1) {
        for anchor_x in x.saturating_sub(3)..=(x + 3).min(CITY_WIDTH - 1) {
            if building_anchor_covers_tile(city, x, y, anchor_x, anchor_y) {
                return true;
            }
        }
    }
    false
}

pub(in crate::app) fn building_origin_has_internal_pipes(
    city: &City,
    x: usize,
    y: usize,
    origin_x: usize,
    origin_y: usize,
) -> bool {
    let building = city.building_at(origin_x, origin_y);
    let size = usize::from(xbld_tile_size(building));
    if size <= 1
        || origin_x + size > CITY_WIDTH
        || origin_y + size > CITY_HEIGHT
        || x >= origin_x + size
        || y >= origin_y + size
        || !building_has_internal_pipes(building)
    {
        return false;
    }

    true
}

pub(in crate::app) fn building_anchor_covers_tile(
    city: &City,
    x: usize,
    y: usize,
    anchor_x: usize,
    anchor_y: usize,
) -> bool {
    let building = city.building_at(anchor_x, anchor_y);
    let size = usize::from(xbld_tile_size(building));
    if size <= 1 || !building_has_internal_pipes(building) {
        return false;
    }
    let no_zone_candidates = [
        (anchor_x, anchor_y),
        (anchor_x.saturating_sub(size - 1), anchor_y),
        (anchor_x, anchor_y.saturating_sub(size - 1)),
        (
            anchor_x.saturating_sub(size - 1),
            anchor_y.saturating_sub(size - 1),
        ),
    ];
    let Some(zone) = city.zone_at(anchor_x, anchor_y) else {
        return no_zone_candidates.into_iter().any(|(origin_x, origin_y)| {
            no_zone_anchor_footprint_covers_tile(
                city, x, y, anchor_x, anchor_y, origin_x, origin_y, building, size,
            )
        });
    };
    let corner_bits = zone.raw_bits() & 0xf0;
    if corner_bits == 0 {
        return no_zone_candidates.into_iter().any(|(origin_x, origin_y)| {
            no_zone_anchor_footprint_covers_tile(
                city, x, y, anchor_x, anchor_y, origin_x, origin_y, building, size,
            )
        });
    }

    let [top_left, top_right, bottom_left, bottom_right] = city.footprint_corner_bits();
    let candidates = [
        (top_left, anchor_x, anchor_y),
        (top_right, anchor_x.saturating_sub(size - 1), anchor_y),
        (bottom_left, anchor_x, anchor_y.saturating_sub(size - 1)),
        (
            bottom_right,
            anchor_x.saturating_sub(size - 1),
            anchor_y.saturating_sub(size - 1),
        ),
    ];
    candidates
        .into_iter()
        .filter(|(bit, _, _)| corner_bits & bit != 0)
        .any(|(_, origin_x, origin_y)| {
            origin_x + size <= CITY_WIDTH
                && origin_y + size <= CITY_HEIGHT
                && x >= origin_x
                && y >= origin_y
                && x < origin_x + size
                && y < origin_y + size
        })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::app) fn no_zone_anchor_footprint_covers_tile(
    city: &City,
    x: usize,
    y: usize,
    anchor_x: usize,
    anchor_y: usize,
    origin_x: usize,
    origin_y: usize,
    building: u8,
    size: usize,
) -> bool {
    if anchor_has_complete_same_building_footprint(city, anchor_x, anchor_y, building, size) {
        return complete_building_footprint_at_origin(city, origin_x, origin_y, building, size)
            && footprint_from_origin_covers_tile(anchor_x, anchor_y, origin_x, origin_y, size)
            && footprint_from_origin_covers_tile(x, y, origin_x, origin_y, size);
    }
    footprint_from_origin_covers_tile(x, y, origin_x, origin_y, size)
}

pub(in crate::app) fn anchor_has_complete_same_building_footprint(
    city: &City,
    anchor_x: usize,
    anchor_y: usize,
    building: u8,
    size: usize,
) -> bool {
    let min_x = anchor_x.saturating_sub(size.saturating_sub(1));
    let min_y = anchor_y.saturating_sub(size.saturating_sub(1));
    (min_y..=anchor_y).any(|origin_y| {
        (min_x..=anchor_x).any(|origin_x| {
            footprint_from_origin_covers_tile(anchor_x, anchor_y, origin_x, origin_y, size)
                && complete_building_footprint_at_origin(city, origin_x, origin_y, building, size)
        })
    })
}

pub(in crate::app) fn footprint_from_origin_covers_tile(
    x: usize,
    y: usize,
    origin_x: usize,
    origin_y: usize,
    size: usize,
) -> bool {
    origin_x + size <= CITY_WIDTH
        && origin_y + size <= CITY_HEIGHT
        && x >= origin_x
        && y >= origin_y
        && x < origin_x + size
        && y < origin_y + size
}

pub(in crate::app) fn water_pipe_network_tile(city: &City, x: usize, y: usize) -> bool {
    let underground_tile = city.underground_at(x, y);
    underground_user_pipe_tile(underground_tile)
        || underground_building_pipe_tile(underground_tile)
        || water_source_connects_as_building_pipe(city, x, y)
        || saved_base_building_pipe_present(city, x, y)
        || building_internal_pipe_present(city, x, y)
}

/// Zoned consumers show piped service when reachable from conductive pipe tiles through zone adjacency.
pub(in crate::app) fn zone_pipe_service_mask(city: &City) -> Vec<bool> {
    let mut mask = vec![false; CITY_TILES];
    let mut queue = VecDeque::new();
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if !zone_serviceable_tile(city, x, y) {
                continue;
            }
            let index = y * CITY_WIDTH + x;
            if directional_neighbor_tiles(x, y)
                .any(|(_, nx, ny)| water_pipe_network_tile(city, nx, ny))
            {
                mask[index] = true;
                pe_network_queue_push(&mut queue, (x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        for (_, nx, ny) in directional_neighbor_tiles(x, y) {
            let index = ny * CITY_WIDTH + nx;
            if mask[index] || !zone_serviceable_tile(city, nx, ny) {
                continue;
            }
            mask[index] = true;
            pe_network_queue_push(&mut queue, (nx, ny));
        }
    }
    mask
}

pub(in crate::app) fn zone_receives_water_pipe_service(city: &City, x: usize, y: usize) -> bool {
    zone_pipe_service_mask(city)[y * CITY_WIDTH + x]
}

pub(in crate::app) fn water_pipe_tile_connects_to(
    city: &City,
    x: usize,
    y: usize,
    side: u8,
) -> bool {
    let tile = city.underground_at(x, y);
    if water_source_connects_as_building_pipe(city, x, y) {
        return true;
    }
    if underground_building_pipe_tile(tile) {
        return true;
    }
    if tile != 0 && underground_user_pipe_tile(tile) {
        if building_internal_pipe_present(city, x, y) {
            return true;
        }
        return underground_tile_connects_to(tile, UndergroundKind::Pipes, side);
    }
    if building_internal_pipe_present(city, x, y) {
        return true;
    }
    if saved_base_building_pipe_present(city, x, y) {
        return true;
    }
    if zone_serviceable_tile(city, x, y) {
        // CityGame's geometry-derived service policy lets a zoned corridor
        // carry the component onward. Exact PE membership remains a separate
        // saved-XBIT path and must be validated against a rooted caller.
        return true;
    }
    if tile != 0 {
        return underground_tile_connects_to(tile, UndergroundKind::Pipes, side);
    }
    false
}

pub(in crate::app) fn underground_user_pipe_tile(tile: u8) -> bool {
    UndergroundKind::Pipes.contains(tile)
        || underground_crossover_axis(tile, UndergroundKind::Pipes).is_some()
}

pub(in crate::app) fn underground_building_pipe_tile(tile: u8) -> bool {
    tile == UNDER_TILE_BUILDING_PIPES
}

pub(in crate::app) fn saved_underground_water_pipe_tile(city: &City, x: usize, y: usize) -> bool {
    let tile = city.underground_at(x, y);
    underground_user_pipe_tile(tile) || underground_building_pipe_tile(tile)
}

pub(in crate::app) fn water_pipe_connection_maps(city: &City) -> (Vec<bool>, Vec<bool>) {
    let mut pipe_tiles = vec![false; CITY_TILES];
    let mut connected_tiles = vec![false; CITY_TILES];
    let mut queue = VecDeque::new();

    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            let index = y * CITY_WIDTH + x;
            let pipe = water_pipe_network_tile(city, x, y) || zone_serviceable_tile(city, x, y);
            pipe_tiles[index] = pipe;
            if pipe && water_source_connects_as_building_pipe(city, x, y) {
                connected_tiles[index] = true;
                pe_network_queue_push(&mut queue, (x, y));
            }
        }
    }

    while let Some((x, y)) = queue.pop_front() {
        for (side, nx, ny) in directional_neighbor_tiles(x, y) {
            let index = ny * CITY_WIDTH + nx;
            if connected_tiles[index] || !pipe_tiles[index] {
                continue;
            }
            if water_pipe_tile_connects_to(city, x, y, side)
                && water_pipe_tile_connects_to(city, nx, ny, opposite_network_side(side))
            {
                connected_tiles[index] = true;
                pe_network_queue_push(&mut queue, (nx, ny));
            }
        }
    }

    (pipe_tiles, connected_tiles)
}
