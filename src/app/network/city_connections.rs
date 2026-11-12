//! City-edge network census and persisted Win95 neighbor-connection overlays.

use super::super::{UndergroundKind, is_highway_tile, underground_tile_connects_to};
use super::*;
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};
use crate::trip_generator::TRIP_NEIGHBOR_CONNECTION_OVERLAY;

pub(in crate::app) fn neighbor_connection_tiles_for_city(city: &City) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city_edge_connection_at(city, x, y) {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

pub(in crate::app) fn road_neighbor_connection_tiles_for_city(city: &City) -> u32 {
    network_neighbor_connection_tiles_for_city(city, NetworkKind::Road)
}

pub(in crate::app) fn rail_neighbor_connection_tiles_for_city(city: &City) -> u32 {
    network_neighbor_connection_tiles_for_city(city, NetworkKind::Rail)
}

pub(in crate::app) fn highway_neighbor_connection_tiles_for_city(city: &City) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city_edge_highway_connection_at(city, x, y) {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

fn network_neighbor_connection_tiles_for_city(city: &City, kind: NetworkKind) -> u32 {
    let mut count = 0_u32;
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            if city_edge_network_connection_at(city, x, y, kind) {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

fn city_edge_connection_at(city: &City, x: usize, y: usize) -> bool {
    if y == 0 && tile_connects_to_city_edge(city, x, y, NETWORK_TOP) {
        return true;
    }
    if x + 1 == CITY_WIDTH && tile_connects_to_city_edge(city, x, y, NETWORK_RIGHT) {
        return true;
    }
    if y + 1 == CITY_HEIGHT && tile_connects_to_city_edge(city, x, y, NETWORK_BOTTOM) {
        return true;
    }
    if x == 0 && tile_connects_to_city_edge(city, x, y, NETWORK_LEFT) {
        return true;
    }
    false
}

fn city_edge_network_connection_at(city: &City, x: usize, y: usize, kind: NetworkKind) -> bool {
    if y == 0 && network_tile_connects_to(city.building_at(x, y), kind, NETWORK_TOP) {
        return true;
    }
    if x + 1 == CITY_WIDTH && network_tile_connects_to(city.building_at(x, y), kind, NETWORK_RIGHT)
    {
        return true;
    }
    if y + 1 == CITY_HEIGHT
        && network_tile_connects_to(city.building_at(x, y), kind, NETWORK_BOTTOM)
    {
        return true;
    }
    if x == 0 && network_tile_connects_to(city.building_at(x, y), kind, NETWORK_LEFT) {
        return true;
    }
    false
}

fn city_edge_highway_connection_at(city: &City, x: usize, y: usize) -> bool {
    (y == 0 || x + 1 == CITY_WIDTH || y + 1 == CITY_HEIGHT || x == 0)
        && is_highway_tile(city.building_at(x, y))
}

fn pe_trip_neighbor_connection_network_at(city: &City, x: usize, y: usize) -> bool {
    city_edge_network_connection_at(city, x, y, NetworkKind::Road)
        || city_edge_network_connection_at(city, x, y, NetworkKind::Rail)
        || city_edge_highway_connection_at(city, x, y)
}

pub(in crate::app) fn persist_pe_neighbor_connection_overlay(city: &mut City, x: usize, y: usize) {
    let overlay = city.text_overlay_at(x, y).unwrap_or(0);
    let connected = pe_trip_neighbor_connection_network_at(city, x, y);
    if connected {
        if overlay == 0 {
            city.set_synthetic_trip_marker(x, y);
        }
    // Native paid markers also exist on tiles without an outward sprite.
    } else if overlay == TRIP_NEIGHBOR_CONNECTION_OVERLAY
        && city.is_synthetic_trip_marker(x, y)
        && !pe_highway_neighbor_link_origin_at(city, x, y)
    {
        city.set_text_overlay(x, y, 0);
    }
}

/// Stamp Win95 XTXT 250 on live road/rail/highway neighbor connections so trips
/// can complete off-map the same way `L_RunTripGenerator` does.
#[cfg(test)]
pub(in crate::app) fn persist_pe_neighbor_connection_overlays(city: &mut City) {
    for y in 0..CITY_HEIGHT {
        for x in 0..CITY_WIDTH {
            persist_pe_neighbor_connection_overlay(city, x, y);
        }
    }
}

fn tile_connects_to_city_edge(city: &City, x: usize, y: usize, side: u8) -> bool {
    let building = city.building_at(x, y);
    is_highway_tile(building)
        || [NetworkKind::Power, NetworkKind::Road, NetworkKind::Rail]
            .into_iter()
            .any(|kind| network_tile_connects_to(building, kind, side))
        || [UndergroundKind::Pipes, UndergroundKind::Subway]
            .into_iter()
            .any(|kind| underground_tile_connects_to(city.underground_at(x, y), kind, side))
}
