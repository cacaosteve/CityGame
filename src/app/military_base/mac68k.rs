//! Mac CODE 14 phase-22 military siting and connector writes.

use crate::app::underground::{mac_military_route_walk, mac_route_commit_xter_and_water_bit};
use crate::app::{offset_city_tile, pe_place_tile_441f00};
use crate::city::City;

use super::{
    EDIT_MAP_COAST_MISC_OFFSET, MILITARY_BASE_AIR_FORCE, MILITARY_BASE_ARMY,
    MILITARY_BASE_MIN_VALID_TILES, MILITARY_BASE_MISSILE_SILOS, MILITARY_BASE_NAVY,
};

type Tile = (usize, usize);

pub(in crate::app) fn place_mac_military_base(
    city: &mut City,
    mut random_word: impl FnMut() -> u16,
) -> Option<(u32, Tile)> {
    if city
        .mac_state_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET)
        .is_some_and(|value| value as u8 != 0)
        && random_word() & 1 != 0
        && let Some((center, plot)) = coastal_plot(city)
    {
        for (x, y) in plot {
            pe_place_tile_441f00(city, x, y, 0);
            city.mac_mark_military_zone_candidate(x, y);
        }
        return Some((MILITARY_BASE_NAVY, center));
    }

    let mut altitude = 0;
    for _ in 0..24 {
        let (x, y) = random_origin(&mut random_word, 120);
        altitude = city.altitude_at(x, y) & 0x1f;
        let plot: Vec<_> = square((x, y), 8)
            .filter(|&tile| vacant_ground(city, tile) && zone_id(city, tile) == 0)
            .collect();
        if plot.len() < MILITARY_BASE_MIN_VALID_TILES {
            continue;
        }
        let matches = plot
            .iter()
            .filter(|&&(tx, ty)| city.altitude_at(tx, ty) & 0x1f == altitude)
            .count();
        if matches < MILITARY_BASE_MIN_VALID_TILES {
            break;
        }

        for &(tx, ty) in &plot {
            city.mac_mark_military_zone_candidate(tx, ty);
        }
        let base = if matches == plot.len() {
            MILITARY_BASE_AIR_FORCE
        } else {
            for (start, end) in [
                ((x + 2, y), (x + 2, y + 7)),
                ((x + 5, y), (x + 5, y + 7)),
                ((x, y + 2), (x + 7, y + 2)),
                ((x, y + 5), (x + 7, y + 5)),
            ] {
                connect_military_route(city, start, end);
                if city.building_at(end.0, end.1) == 0 {
                    connect_military_route(city, end, start);
                }
            }
            MILITARY_BASE_ARMY
        };
        return Some((base, (x + 4, y + 4)));
    }

    let mut patches = Vec::new();
    for _ in 0..40 {
        let origin = random_origin(&mut random_word, 125);
        if square(origin, 3).all(|tile @ (x, y)| {
            vacant_ground(city, tile)
                && zone_id(city, tile) != 7
                && city.altitude_at(x, y) & 0x1f == altitude
        }) {
            patches.push(origin);
        }
        if patches.len() == 6 {
            // The original selects all six origins before modifying the map.
            for (x, y) in patches.into_iter().flat_map(|origin| square(origin, 3)) {
                city.mac_mark_military_zone_candidate(x, y);
            }
            return Some((MILITARY_BASE_MISSILE_SILOS, origin));
        }
    }
    None
}

fn random_origin(random_word: &mut impl FnMut() -> u16, modulus: u16) -> Tile {
    (
        usize::from(random_word() % modulus),
        usize::from(random_word() % modulus),
    )
}

fn square((x, y): Tile, size: usize) -> impl Iterator<Item = Tile> {
    (x..x + size).flat_map(move |x| (y..y + size).map(move |y| (x, y)))
}

fn zone_id(city: &City, (x, y): Tile) -> u8 {
    city.zone_at(x, y).map_or(0, |zone| zone.zone_id())
}

fn water_bits(city: &City, (x, y): Tile) -> u8 {
    city.tile_bits_at(x, y)
        .map_or(0, |bits| bits.raw_bits() & 5)
}

fn vacant_ground(city: &City, tile @ (x, y): Tile) -> bool {
    city.building_at(x, y) < 13 && city.terrain_at(x, y) == 0 && water_bits(city, tile) & 4 == 0
}

fn coastal_plot(city: &City) -> Option<(Tile, Vec<Tile>)> {
    let edge = 127;
    let orientation = usize::from(city.saved_rotation() & 3);
    let mut tile = [(edge, 0), (0, 0), (0, edge), (edge, edge)][orientation];
    let inland = [(-1, 0), (0, 1), (1, 0), (0, -1)][orientation];
    let along = [(0, 1), (1, 0), (0, -1), (-1, 0)][orientation];
    if water_bits(city, tile) & 4 == 0 {
        return None;
    }
    while water_bits(city, tile) & 4 != 0 {
        tile = offset_city_tile(tile.0, tile.1, inland.0, inland.1)?;
    }

    let mut run_start = tile;
    let mut best_start = tile;
    let mut run = 0;
    let mut best = 0;
    loop {
        if vacant_ground(city, tile) {
            run += 1;
            if run > best {
                best = run;
                best_start = run_start;
            }
        } else {
            run_start = tile;
            run = 0;
        }
        let Some(next) = next_shore_tile(city, tile, along, inland) else {
            break;
        };
        tile = next;
    }
    if best < 12 {
        return None;
    }

    tile = best_start;
    let margin = (best - 12) / 2;
    let mut center = None;
    let mut plot = Vec::new();
    for remaining in (margin..best).rev() {
        tile = next_shore_tile(city, tile, along, inland)?;
        if remaining == margin + 5 {
            center = Some(tile);
        }
        if remaining < margin + 10 {
            let mut cell = Some(tile);
            for _ in 0..4 {
                let Some(point) = cell else { break };
                if vacant_ground(city, point) {
                    plot.push(point);
                }
                cell = offset_city_tile(point.0, point.1, inland.0, inland.1);
            }
        }
    }
    Some((center?, plot))
}

fn next_shore_tile(
    city: &City,
    (x, y): Tile,
    along: (isize, isize),
    inland: (isize, isize),
) -> Option<Tile> {
    let mut tile = offset_city_tile(x, y, along.0, along.1)?;
    if water_bits(city, tile) == 5 {
        while water_bits(city, tile) == 5 {
            tile = offset_city_tile(tile.0, tile.1, inland.0, inland.1)?;
        }
        return Some(tile);
    }
    while let Some(next) = offset_city_tile(tile.0, tile.1, -inland.0, -inland.1) {
        if water_bits(city, next) == 5 {
            return Some(tile);
        }
        tile = next;
    }
    None
}

fn connect_military_route(city: &mut City, start: Tile, end: Tile) {
    if let Some(mut walk) = mac_military_route_walk(city, start, end) {
        while let Some(cell) = walk.next_commit(city) {
            mac_route_commit_xter_and_water_bit(city, cell.tile, cell.direction);
            let (x, y) = cell.tile;
            let replacement = match city.building_at(x, y) {
                0..=0x0d => 0x1d,
                0x0e | 0x2c => 0x46,
                0x0f => 0x43,
                0x2d => 0x45,
                0x49 => 0x4c,
                0x4a => 0x4b,
                _ => continue,
            };
            pe_place_tile_441f00(city, x, y, replacement);
            if let Some(zone) = city.zone_at(x, y)
                && zone.zone_id() != 7
            {
                city.set_zone(x, y, zone.raw_bits() & 0xf0);
            }
            for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                if let Some(tile) = offset_city_tile(x, y, dx, dy) {
                    retile_connector_road(city, tile);
                }
            }
        }
    }
    for (x, y) in [start, end] {
        if matches!(city.building_at(x, y), 0x1d | 0x1e) {
            pe_place_tile_441f00(city, x, y, 0xde);
            if let Some(zone) = city.zone_at(x, y) {
                city.set_zone(x, y, zone.zone_id() | 0xf0);
            }
        }
    }
}

fn retile_connector_road(city: &mut City, (x, y): Tile) {
    if !matches!(city.building_at(x, y), 0x1d..=0x2b) {
        return;
    }
    const TERRAIN_VARIANTS: [u16; 48] = [
        0, 2, 3, 4, 5, 0x100, 0x100, 0x101, 0x101, 0x101, 0x101, 0x101, 0x100, 0x100, 0x100, 0x101,
        0x101, 0x101, 0x101, 0x101, 0x100, 0x100, 0x100, 0x101, 0x101, 0x101, 0x101, 0x101, 0x100,
        0x101, 0x101, 0x101, 0x101, 0x101, 0x101, 0x101, 0x101, 0x101, 0x101, 0, 0x101, 1, 1,
        0x101, 0x101, 0x101, 0x101, 0x100,
    ];
    const ADJACENCY_VARIANTS: [u16; 16] = [0, 0, 1, 6, 0, 0, 7, 11, 1, 9, 1, 10, 8, 13, 12, 14];
    let terrain = city.terrain_at(x, y);
    let terrain = if terrain < 0x30 { terrain } else { 0 };
    let variant = if matches!(terrain & 0xf, 1..=4 | 9..=12) {
        TERRAIN_VARIANTS[usize::from(terrain)]
    } else {
        let mut mask = 0;
        for (dx, dy, bit) in [(-1, 0, 8), (1, 0, 2), (0, -1, 1), (0, 1, 4)] {
            let connects = if let Some((nx, ny)) = offset_city_tile(x, y, dx, dy) {
                matches!(city.building_at(nx, ny), 0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c | 0x5d..=0x60)
                    && connector_terrain_faces_axis(city.terrain_at(nx, ny), dx != 0)
            } else {
                city.text_overlay_at(x, y) == Some(0xfa)
            };
            if connects {
                mask |= bit;
            }
        }
        ADJACENCY_VARIANTS[mask]
    };
    city.mac_connector_place_tile(x, y, 0x1d + variant);
}

fn connector_terrain_faces_axis(terrain: u8, x_axis: bool) -> bool {
    if x_axis {
        matches!(terrain, 0..=1 | 3 | 5..=0x0e | 0x10..=0x11 | 0x13 | 0x15..=0x1e
            | 0x20..=0x21 | 0x23 | 0x25..=0x2e | 0x30..=0x43)
    } else {
        matches!(terrain, 0 | 2 | 4..=0x0e | 0x10 | 0x12 | 0x14..=0x1e | 0x20 | 0x22
            | 0x24..=0x2e | 0x30..=0x43 | 0x46..=0x47 | 0x49 | 0x4b..=0x54
            | 0x56..=0x57 | 0x59 | 0x5b..=0x64 | 0x66..=0x67 | 0x69 | 0x6b..=0x74 | 0x76..=0x89)
    }
}

#[cfg(test)]
mod tests {
    use super::connect_military_route;
    use crate::city::{City, CityFileKind, TileBits};

    #[test]
    fn mac_military_empty_corridor_matches_native_worker_replay_on_both_layouts() {
        for extension in ["city", "sc2"] {
            let path = std::env::temp_dir().join(format!(
                "citygame-mac-military-corridor-{}.{extension}",
                std::process::id()
            ));
            let mut city = City::new_blank(&path, "Military corridor").expect("blank city");
            city.kind = if extension == "sc2" {
                CityFileKind::WinSc2
            } else {
                CityFileKind::MacCity
            };
            if city.kind == CityFileKind::WinSc2 {
                city.prepare_mac_fidelity_from_saved_city();
                city.mac_xbld_census_runtime[0x1d] = 1;
                city.mac_xbld_census_runtime[0x1e] = 1;
                city.pe_dw_tile_count[0x1d] = 1;
                city.pe_dw_tile_count[0x1e] = 1;
                city.pe_military_tile_count[0] = 2;
            } else {
                city.set_mac_state_misc_u32_at(0x01f0 + 0x1d * 4, 1);
                city.set_mac_state_misc_u32_at(0x01f0 + 0x1e * 4, 1);
                city.set_mac_state_misc_u32_at(0x0fa8, 2);
            }
            city.set_mac_state_misc_u32_at(0x01f0, 16_384);

            for x in 20..=24 {
                city.set_zone(x, 40, 3);
                city.set_tile_bits(x, 40, TileBits::new(0x80));
            }
            city.set_zone(20, 40, 7);
            city.set_zone(24, 40, 7);
            city.set_building(20, 40, 0x1d);
            city.set_building(24, 40, 0x1e);

            connect_military_route(&mut city, (20, 40), (24, 40));

            let expected_buildings = [0xde, 0x1e, 0x1e, 0x1e, 0xde];
            let expected_zones = [0xf7, 0x00, 0x00, 0x00, 0xf7];
            for (index, x) in (20..=24).enumerate() {
                assert_eq!(
                    city.building_at(x, 40),
                    expected_buildings[index],
                    "{extension} ({x},40)"
                );
                assert_eq!(
                    city.zone_at(x, 40).expect("zone").raw_bits(),
                    expected_zones[index],
                    "{extension} ({x},40)"
                );
                assert_eq!(city.terrain_at(x, 40), 0, "{extension} ({x},40)");
                assert_eq!(
                    city.tile_bits_at(x, 40).expect("tile bits").raw_bits(),
                    0x80
                );
            }
            for x in 0..128 {
                for y in 0..128 {
                    if !(20..=24).contains(&x) || y != 40 {
                        assert_eq!(city.building_at(x, y), 0, "{extension} XBLD ({x},{y})");
                        assert_eq!(
                            city.zone_at(x, y).expect("zone").raw_bits(),
                            0,
                            "{extension} XZON ({x},{y})"
                        );
                    }
                }
            }
            assert_eq!(
                city.mac_xbld_tile_count(0x1d),
                1,
                "{extension} general census"
            );
            assert_eq!(
                city.mac_xbld_tile_count(0x1e),
                4,
                "{extension} general census"
            );
            assert_eq!(
                city.mac_zone7_census_count(0),
                0,
                "{extension} military census"
            );
            assert_eq!(
                city.mac_zone7_census_count(2),
                2,
                "{extension} military census"
            );
            let _ = std::fs::remove_file(path);
        }
    }
}
