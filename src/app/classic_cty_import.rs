//! Win95 Classic CTY conversion in the live application context.
//!
//! Keep this in `app`: the retail tile converter consumes process-owned CRT
//! state and shares its implementation with the native newspaper-gift path.

use crate::city::{City, classic_cty::ClassicCtyTile};
use crate::win95_random::{Win95CrtRand, Win95GameRand};
use anyhow::Result;

const CLASSIC_CTY_FOUNDED_YEAR: u16 = 1_900;

pub(crate) fn load_classic_cty(
    path: &std::path::Path,
    crt: &mut Win95CrtRand,
    game_rand: &mut Win95GameRand,
) -> Result<City> {
    let classic = crate::city::classic_cty::ClassicCtyFile::read(path)?;
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Classic City");
    let mut city = City::new_blank(path, name)?;
    clear_classic_cty_target_map(&mut city);
    apply_cty_tiles(&mut city, crt, game_rand, &classic.tiles);
    let date = classic.date();
    // FUN_004829F0 sets 0x4ca5f4 to 1900 for this legacy format before it
    // publishes the imported calendar and computes the city's age.
    city.initialize_new_city_founding_year(CLASSIC_CTY_FOUNDED_YEAR);
    city.set_saved_date(date);
    city.pe_reseed_neighbor_fame_ranks_before_year_42e585(crt, date.year);
    Ok(city)
}

/// Retail allocates an empty target document before it feeds the legacy tile
/// words to PlaceTile. `City::new_blank` prepares playable terrain, so erase
/// those generated map planes before applying the Classic CTY payload.
fn clear_classic_cty_target_map(city: &mut City) {
    for x in 0..128 {
        for y in 0..128 {
            city.set_building(x, y, 0);
            city.set_terrain(x, y, 0);
            city.set_zone(x, y, 0);
            city.set_tile_bits(x, y, crate::city::TileBits::new(0));
            city.set_altm_word(x, y, 0);
            city.set_text_overlay(x, y, 0);
        }
    }
}

/// Apply the tile words and border passes in the order read by `FUN_004829F0`.
///
/// `pe_newspaper_gift_entry_483250` mirrors the same `FUN_00483250` converter
/// cases, including the high network/zone bands, DestroyBuilding paths, and
/// CRT draws. `FUN_00484200` then fills the eight map-border regions from the
/// live game LCG and shore-prepares them when the region's anchor is water.
/// Header publication and later reader finalization remain separate.
pub(crate) fn apply_cty_tiles(
    city: &mut City,
    crt: &mut Win95CrtRand,
    game_rand: &mut Win95GameRand,
    tiles: &[ClassicCtyTile],
) {
    apply_cty_tiles_before_normalize(city, crt, game_rand, tiles);
    normalize_cty_map(city);
}

fn apply_cty_tiles_before_normalize(
    city: &mut City,
    crt: &mut Win95CrtRand,
    game_rand: &mut Win95GameRand,
    tiles: &[ClassicCtyTile],
) {
    apply_cty_payload_tiles(city, crt, tiles);
    apply_cty_border(city, game_rand);
}

fn apply_cty_payload_tiles(city: &mut City, crt: &mut Win95CrtRand, tiles: &[ClassicCtyTile]) {
    // A blank map has zeroed process counters. The native map starts as 16,384
    // empty XBLD cells before the 12,000 interior words are placed.
    city.pe_dw_tile_count[0] = 128 * 128;
    for tile in tiles {
        super::newspaper_gifts::pe_newspaper_gift_entry_483250(
            city,
            crt,
            isize::from(tile.x),
            isize::from(tile.y),
            tile.id,
        );
    }
}

fn apply_cty_border(city: &mut City, game_rand: &mut Win95GameRand) {
    for (anchor, bounds) in [
        ((14, 4), (0, 0, 14, 4)),
        ((14, 123), (0, 123, 14, 127)),
        ((112, 4), (112, 0, 127, 4)),
        ((112, 123), (112, 123, 127, 127)),
    ] {
        apply_cty_border_rect(city, game_rand, anchor, bounds);
    }
    for x in 15..112 {
        apply_cty_border_rect(city, game_rand, (x, 5), (x, 0, x, 4));
    }
    for x in 15..112 {
        apply_cty_border_rect(city, game_rand, (x, 123), (x, 124, x, 127));
    }
    for y in 5..123 {
        apply_cty_border_rect(city, game_rand, (14, y), (0, y, 13, y));
    }
    for y in 5..123 {
        apply_cty_border_rect(city, game_rand, (112, y), (113, y, 127, y));
    }
}

fn apply_cty_border_rect(
    city: &mut City,
    game_rand: &mut Win95GameRand,
    anchor: (usize, usize),
    bounds: (usize, usize, usize, usize),
) {
    let (anchor_x, anchor_y) = anchor;
    let (x1, y1, x2, y2) = bounds;
    if city
        .tile_bits_at(anchor_x, anchor_y)
        .is_some_and(crate::city::TileBits::water_tile)
    {
        for x in x1..=x2 {
            for y in y1..=y2 {
                super::newspaper_gifts::pe_newspaper_gift_shore_prep_484320(city, x, y);
            }
        }
        return;
    }

    for x in x1..=x2 {
        for y in y1..=y2 {
            let roll = game_rand.random_mod(12);
            if (1..=2).contains(&roll) {
                let building = game_rand.random_mod(7) as u8 + 6;
                super::pe_place_tile_441f00(city, x, y, building);
            }
        }
    }
}

/// In-place passes of retail `FUN_004843D0`.
fn normalize_cty_map(city: &mut City) {
    normalize_cty_water_islands(city);
    assign_cty_water_terrain(city);
    normalize_cty_water_bridges(city);
}

/// First pass: clear an isolated water cell surrounded on all eight sides by
/// land, and clear its land-height nibble when all eight neighbors are water.
/// The scan order is x-major then y-major.
fn normalize_cty_water_islands(city: &mut City) {
    for x in 1..127 {
        for y in 1..127 {
            if !city
                .tile_bits_at(x, y)
                .is_some_and(crate::city::TileBits::water_tile)
            {
                continue;
            }

            let mut missing_water_mask = 0u8;
            for (dx, dy, mask) in [
                (-1isize, -1isize, 0x1),
                (0, -1, 0x3),
                (1, -1, 0x2),
                (1, 0, 0x6),
                (1, 1, 0x4),
                (0, 1, 0xc),
                (-1, 1, 0x8),
                (-1, 0, 0x9),
            ] {
                let neighbor_x = x.wrapping_add_signed(dx);
                let neighbor_y = y.wrapping_add_signed(dy);
                if !city
                    .tile_bits_at(neighbor_x, neighbor_y)
                    .is_some_and(crate::city::TileBits::water_tile)
                {
                    missing_water_mask |= mask;
                }
            }

            if missing_water_mask == 0 {
                let altm = city.altm_at(x, y);
                city.set_altm_word(
                    x,
                    y,
                    (u16::from(altm.land)
                        | (u16::from(altm.water) << 5)
                        | (u16::from(altm.tunnel_levels) << 10))
                        & 0xffe0,
                );
            } else if missing_water_mask == 0x0f {
                let altm = city.altm_at(x, y);
                city.set_altm_word(
                    x,
                    y,
                    ((u16::from(altm.land)
                        | (u16::from(altm.water) << 5)
                        | (u16::from(altm.tunnel_levels) << 10))
                        & 0xffe2)
                        | 2,
                );
                let bits = city.tile_bits_at(x, y).unwrap_or_default();
                city.set_tile_bits(x, y, bits.with_water_tile(false));
                city.set_terrain(x, y, 0);
            }
        }
    }
}

/// Second pass: assign the coast XTER variant from the eight neighboring land
/// heights. The table and strict comparison follow the native local WORD table
/// and `ALTM & 0x1f` reads at `FUN_004843D0`.
fn assign_cty_water_terrain(city: &mut City) {
    const COAST_VARIANT: [u8; 16] = [0, 9, 10, 2, 11, 13, 3, 6, 12, 1, 13, 5, 4, 8, 7, 13];

    for x in 1..127 {
        for y in 1..127 {
            if !city
                .tile_bits_at(x, y)
                .is_some_and(crate::city::TileBits::water_tile)
            {
                continue;
            }

            let height = city.altm_at(x, y).land;
            let neighbor_higher = |dx: isize, dy: isize| {
                let neighbor_x = x.wrapping_add_signed(dx);
                let neighbor_y = y.wrapping_add_signed(dy);
                height < city.altm_at(neighbor_x, neighbor_y).land
            };
            let mut shape = u8::from(neighbor_higher(-1, -1));
            if neighbor_higher(0, -1) {
                shape = 3;
            }
            if neighbor_higher(1, -1) {
                shape |= 2;
            }
            if neighbor_higher(1, 0) {
                shape |= 6;
            }
            if neighbor_higher(1, 1) {
                shape |= 4;
            }
            if neighbor_higher(0, 1) {
                shape |= 0x0c;
            }
            if neighbor_higher(-1, 1) {
                shape |= 8;
            }
            if neighbor_higher(-1, 0) {
                shape |= 9;
            }

            let terrain_base = if height == 0 { 0x10 } else { 0x20 };
            city.set_terrain(x, y, COAST_VARIANT[usize::from(shape)] + terrain_base);
        }
    }
}

/// Third pass: orient eligible bridge endpoints on high water terrain, then
/// replace the bridge tile with water, preserve native altitude/XBIT bits, and
/// refresh adjacent road/rail/power shapes for the matching bridge IDs.
fn normalize_cty_water_bridges(city: &mut City) {
    for x in 1..127 {
        for y in 1..127 {
            let building = city.building_at(x, y);
            if !matches!(building, 0x51..=0x5c | 0x6a..=0x6b)
                || !city
                    .tile_bits_at(x, y)
                    .is_some_and(crate::city::TileBits::water_tile)
                || city.terrain_at(x, y) <= 0x20
            {
                continue;
            }

            let bits = city.tile_bits_at(x, y).unwrap_or_default();
            let vertical_axis = bits.raw_bits() & 0x02 != 0;
            let mut direction = 0u8;
            if vertical_axis {
                if city.building_at(x + 1, y) == building {
                    direction |= 2;
                }
                if city.building_at(x - 1, y) == building {
                    direction |= 8;
                }
            } else {
                if city.building_at(x, y - 1) == building {
                    direction |= 1;
                }
                if city.building_at(x, y + 1) == building {
                    direction |= 4;
                }
            }

            let orientation = match direction {
                1 => 2,
                2 => 3,
                4 => 4,
                8 => 1,
                _ => continue,
            };
            city.set_terrain(x, y, orientation);
            super::pe_place_tile_441f00(city, x, y, 0);

            let altm = city.altm_at(x, y);
            city.set_altm_word(
                x,
                y,
                (u16::from(altm.land)
                    | (u16::from(altm.water) << 5)
                    | (u16::from(altm.tunnel_levels) << 10))
                    & 0xffe2
                    | 2,
            );
            let bits = city.tile_bits_at(x, y).unwrap_or_default();
            city.set_tile_bits(x, y, crate::city::TileBits::new(bits.raw_bits() & 0x39));

            let network = match building {
                0x57 => Some(super::network::NetworkKind::Road),
                0x5a => Some(super::network::NetworkKind::Rail),
                0x5c => Some(super::network::NetworkKind::Power),
                _ => None,
            };
            if let Some(network) = network {
                super::network::pe_refresh_cty_bridge_neighbors(city, x, y, network);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::City;

    #[test]
    fn applies_native_converter_ranges_and_crt_stream_in_input_order() {
        let mut city = City::new_blank("classic-converter.sc2", "Classic converter").unwrap();
        let tiles = [
            ClassicCtyTile {
                x: 14,
                y: 123,
                id: 0x15,
            },
            ClassicCtyTile {
                x: 15,
                y: 123,
                id: 0x42,
            },
            ClassicCtyTile {
                x: 16,
                y: 123,
                id: 0xd2,
            },
            ClassicCtyTile {
                x: 17,
                y: 123,
                id: 0xe2,
            },
            ClassicCtyTile {
                x: 18,
                y: 123,
                id: 0x31,
            },
            ClassicCtyTile {
                x: 19,
                y: 123,
                id: 0x38,
            },
        ];
        let mut crt = Win95CrtRand::new(1);
        let mut game_rand = Win95GameRand::new(0x1234_5679);
        let mut expected_crt = crt;
        let expected_tree = ((expected_crt.random_u15() & 3) + 6) as u8;

        apply_cty_tiles(&mut city, &mut crt, &mut game_rand, &tiles);

        assert_eq!(city.building_at(14, 123), expected_tree);
        // These are the retail 0x4ea9d8/0x4ea9e8/0x4ea9f8 lookup results
        // for converter indices 2, 2, and 2.
        assert_eq!(city.building_at(15, 123), 29);
        assert_eq!(city.building_at(16, 123), 14);
        assert_eq!(city.building_at(17, 123), 44);
        assert_eq!(city.text_overlay_at(18, 123), Some(0xfc));
        assert_eq!(city.pe_w_current_disaster_type(), 1);
        assert_eq!(city.pe_dw_disaster_active(), 1);
        assert_eq!(city.text_overlay_at(19, 123), Some(0xff));
        assert_eq!(crt, expected_crt);
        assert_ne!(game_rand.state(), 0x1234_5679);
        assert_eq!(
            city.pe_dw_tile_count
                .iter()
                .map(|&count| u32::from(count))
                .sum::<u32>(),
            128 * 128
        );
    }

    #[test]
    fn water_border_anchor_shore_prepares_region_without_game_lcg_draws() {
        let mut city = City::new_blank("classic-border.sc2", "Classic border").unwrap();
        city.set_tile_bits(20, 20, crate::city::TileBits::new(0x04));
        let mut game_rand = Win95GameRand::new(0x2468_1357);

        apply_cty_border_rect(&mut city, &mut game_rand, (20, 20), (30, 40, 31, 40));

        assert_eq!(game_rand.state(), 0x2468_1357);
        for x in 30..=31 {
            assert!(city.tile_bits_at(x, 40).unwrap().water_tile());
            assert_eq!(city.altm_at(x, 40).land, 1);
            assert_eq!(city.terrain_at(x, 40), 0x10);
        }
    }

    #[test]
    fn dry_border_region_uses_live_game_lcg_and_places_native_tree_ids() {
        let mut city = City::new_blank("classic-border.sc2", "Classic border").unwrap();
        let mut game_rand = Win95GameRand::new(0x2468_1357);
        let mut expected_rand = game_rand;
        let mut expected = [0u8; 2];
        for building in &mut expected {
            let roll = expected_rand.random_mod(12);
            if (1..=2).contains(&roll) {
                *building = expected_rand.random_mod(7) as u8 + 6;
            }
        }

        apply_cty_border_rect(&mut city, &mut game_rand, (20, 20), (30, 40, 31, 40));

        assert_eq!(game_rand, expected_rand);
        assert_eq!(city.building_at(30, 40), expected[0]);
        assert_eq!(city.building_at(31, 40), expected[1]);
    }

    #[test]
    fn normalizer_clears_isolated_water_and_land_height_in_native_scan_order() {
        let mut city = City::new_blank("classic-water-normalize.sc2", "Water normalize").unwrap();
        let center = crate::city::TileBits::new(0x24);
        city.set_tile_bits(20, 20, center);
        city.set_altm_word(20, 20, 0xffff);
        city.set_terrain(20, 20, 0x34);
        for x in 19..=21 {
            for y in 19..=21 {
                if (x, y) != (20, 20) {
                    city.set_tile_bits(x, y, crate::city::TileBits::new(0));
                }
            }
        }

        normalize_cty_water_islands(&mut city);

        assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
        assert_eq!(city.altm_at(20, 20).land, 2);
        assert_eq!(city.altm_at(20, 20).water, 31);
        assert_eq!(city.altm_at(20, 20).tunnel_levels, 63);
        assert_eq!(city.terrain_at(20, 20), 0);
    }

    #[test]
    fn normalizer_clears_land_height_for_water_with_eight_water_neighbors() {
        let mut city = City::new_blank("classic-ocean-normalize.sc2", "Ocean normalize").unwrap();
        for x in 19..=21 {
            for y in 19..=21 {
                city.set_tile_bits(x, y, crate::city::TileBits::new(0x04));
                city.set_altitude(x, y, 2, 2);
            }
        }
        city.set_altitude(20, 20, 1, 1);

        assign_cty_water_terrain(&mut city);

        assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
        assert_eq!(
            city.altm_at(20, 20).land,
            1,
            "terrain assignment leaves the land nibble unchanged"
        );
        assert_eq!(city.altm_at(20, 20).water, 1);
        assert_eq!(city.altm_at(20, 20).tunnel_levels, 0);
        assert_eq!(city.terrain_at(20, 20), 0x2d);
    }

    #[test]
    fn bridge_normalizer_orients_single_endpoint_and_clears_water_state() {
        let mut city = City::new_blank("classic-bridge-normalize.sc2", "Bridge normalize").unwrap();
        city.set_building(30, 30, 0x56);
        city.set_building(30, 31, 0x56);
        city.set_tile_bits(30, 30, crate::city::TileBits::new(0x04));
        city.set_altm_word(30, 30, 0xffff);
        city.set_terrain(30, 30, 0x21);
        city.pe_dw_tile_count[0] = 128 * 128 - 2;
        city.pe_dw_tile_count[0x56] = 2;

        normalize_cty_water_bridges(&mut city);

        assert_eq!(city.terrain_at(30, 30), 4);
        assert_eq!(city.building_at(30, 30), 0);
        let altm = city.altm_at(30, 30);
        assert_eq!((altm.land, altm.water, altm.tunnel_levels), (2, 31, 63));
        assert_eq!(city.tile_bits_at(30, 30).unwrap().raw_bits(), 0);
        assert_eq!(city.pe_dw_tile_count[0x56], 1);
        assert_eq!(city.pe_dw_tile_count[0], 128 * 128 - 1);
    }

    #[test]
    fn bridge_normalizer_leaves_non_endpoint_segments_unchanged() {
        let mut city = City::new_blank("classic-bridge-segment.sc2", "Bridge segment").unwrap();
        city.set_building(30, 30, 0x56);
        city.set_building(30, 31, 0x56);
        city.set_building(30, 29, 0x56);
        city.set_tile_bits(30, 30, crate::city::TileBits::new(0x06));
        city.set_terrain(30, 30, 0x21);
        city.pe_dw_tile_count[0] = 128 * 128 - 3;
        city.pe_dw_tile_count[0x56] = 3;

        normalize_cty_water_bridges(&mut city);

        assert_eq!(city.terrain_at(30, 30), 0x21);
        assert_eq!(city.building_at(30, 30), 0x56);
        assert_eq!(city.tile_bits_at(30, 30).unwrap().raw_bits(), 0x06);
    }

    #[test]
    fn bridge_normalizer_refreshes_road_neighbors_after_removal() {
        let mut city = City::new_blank("classic-road-bridge-normalize.sc2", "Road bridge").unwrap();
        city.set_building(30, 30, 0x57);
        city.set_building(30, 31, 0x57);
        city.set_building(30, 29, 0x1d);
        city.set_building(30, 28, 0x1d);
        city.set_building(31, 29, 0x1d);
        city.set_tile_bits(30, 30, crate::city::TileBits::new(0x04));
        city.set_altm_word(30, 30, 0xffff);
        city.set_terrain(30, 30, 0x21);
        city.pe_dw_tile_count[0] = 128 * 128 - 5;
        city.pe_dw_tile_count[0x57] = 2;
        city.pe_dw_tile_count[0x1d] = 3;

        normalize_cty_water_bridges(&mut city);

        let refreshed_road = city.building_at(30, 29);
        assert_ne!(refreshed_road, 0x1d);
        assert!((0x1d..=0x2b).contains(&refreshed_road));
    }

    #[test]
    fn path_import_uses_native_tile_and_calendar_data_with_live_streams() {
        let path = std::env::temp_dir().join(format!(
            "citygame-classic-cty-import-{}.CTY",
            std::process::id()
        ));
        let mut bytes = vec![0; crate::city::classic_cty::CLASSIC_CTY_FILE_LEN];
        let header = 0x0bc0 + 0x10;
        bytes[header..header + 4].copy_from_slice(&[0, 0, 0x10, 0xe4]);
        let map = 0x0bc0 + 0x00f0;
        bytes[map..map + 2].copy_from_slice(&0x15u16.to_be_bytes());
        std::fs::write(&path, bytes).unwrap();

        let mut crt = Win95CrtRand::new(1);
        let mut expected_crt = crt;
        let expected_tree = ((expected_crt.random_u15() & 3) + 6) as u8;
        for _ in 0..17 {
            expected_crt.random_u15();
        }
        let mut game_rand = Win95GameRand::new(0x2468_1357);
        let city = load_classic_cty(&path, &mut crt, &mut game_rand).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(city.name, path.file_stem().unwrap().to_str().unwrap());
        assert_eq!(
            city.saved_date(),
            Some(crate::city::CityDate {
                year: 1_990,
                month: 2
            })
        );
        assert_eq!(
            city.year_founded(),
            Some(u32::from(CLASSIC_CTY_FOUNDED_YEAR))
        );
        assert_eq!(city.city_age_days(), Some(90 * 300 + 25));
        assert_eq!(city.building_at(14, 123), expected_tree);
        assert_eq!(crt, expected_crt);
    }

    #[test]
    #[ignore = "requires the private authentic BAZ CTY and native Win95 reader plane capture"]
    fn authentic_baz_reader_map_planes_match_native_win95() {
        use std::collections::BTreeMap;

        let cty =
            std::path::PathBuf::from(std::env::var("CITYGAME_CLASSIC_CTY_NATIVE_INPUT").unwrap());
        let native_planes =
            std::path::PathBuf::from(std::env::var("CITYGAME_CLASSIC_CTY_NATIVE_PLANES").unwrap());
        let expected: BTreeMap<String, String> = serde_json::from_slice(
            &std::fs::read(native_planes).expect("read native reader-return planes"),
        )
        .expect("decode native plane JSON");
        let bytes = |planes: &BTreeMap<String, String>, name: &str| {
            let hex = planes.get(name).unwrap_or_else(|| panic!("missing {name}"));
            hex.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
        };

        // The native reader-return snapshot was captured with a blank target
        // and the authentic PE process initialized from a CityGame save.
        let mut crt = Win95CrtRand::new(2_503_793_637);
        let mut game_rand = Win95GameRand::new(1);
        let city = load_classic_cty(&cty, &mut crt, &mut game_rand).unwrap();

        let collect_planes = |city: &City| {
            let (mut xbld, mut xzon, mut xbit, mut altm, mut xtxt) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for x in 0..128 {
                for y in 0..128 {
                    xbld.push(city.building_at(x, y));
                    xzon.push(city.zone_at(x, y).unwrap_or_default().raw_bits());
                    xbit.push(city.tile_bits_at(x, y).unwrap_or_default().raw_bits());
                    xtxt.push(city.text_overlay_at(x, y).unwrap_or_default());
                    let sample = city.altm_at(x, y);
                    let altm_word = u16::from(sample.land)
                        | (u16::from(sample.water) << 5)
                        | (u16::from(sample.tunnel_levels) << 10);
                    altm.extend_from_slice(&altm_word.to_le_bytes());
                }
            }
            [
                ("XBLD", xbld),
                ("XZON", xzon),
                ("XBIT", xbit),
                ("ALTM", altm),
                ("XTXT", xtxt),
            ]
            .into_iter()
            .collect::<BTreeMap<_, _>>()
        };
        let city_planes = collect_planes(&city);
        for name in ["XBLD", "XZON", "XBIT", "ALTM", "XTXT"] {
            assert_eq!(
                city_planes[name],
                bytes(&expected, name),
                "{name} native plane"
            );
        }
        assert_eq!(
            crt.seed(),
            0x4621_c31c,
            "Win95 CRT after 2,744 converter and 17 native post-reader draws"
        );
        assert_eq!(game_rand.state(), 0x9302_aeea, "Win95 game LCG final state");
        assert_eq!(
            (0..17)
                .map(|index| city.pe_w_neighbor_fame_rank(index).unwrap())
                .collect::<Vec<_>>(),
            [
                0, 0, 0, 0, 2026, 2061, 0, 0, 0, 0, 0, 0, 0, 2047, 2102, 2151, 2203
            ],
            "native post-reader neighbor fame ranks"
        );
    }

    #[test]
    #[ignore = "requires private authentic BERN CTY and native Win95 reader plane capture"]
    fn authentic_bern_reader_calendar_and_map_planes_match_native_win95() {
        use std::collections::BTreeMap;

        let cty =
            std::path::PathBuf::from(std::env::var("CITYGAME_CLASSIC_CTY_BERN_INPUT").unwrap());
        let native_planes =
            std::path::PathBuf::from(std::env::var("CITYGAME_CLASSIC_CTY_BERN_PLANES").unwrap());
        let expected: BTreeMap<String, String> = serde_json::from_slice(
            &std::fs::read(native_planes).expect("read native BERN reader-return planes"),
        )
        .expect("decode native BERN plane JSON");
        let before_border_path = std::path::PathBuf::from(
            std::env::var("CITYGAME_CLASSIC_CTY_BERN_PRE_BORDER").unwrap(),
        );
        let before_border: BTreeMap<String, String> = serde_json::from_slice(
            &std::fs::read(before_border_path).expect("read native BERN pre-border planes"),
        )
        .expect("decode native BERN pre-border plane JSON");
        let classic = crate::city::classic_cty::ClassicCtyFile::read(&cty).unwrap();
        let native_crt_entries: Vec<u32> = serde_json::from_slice(
            &std::fs::read(std::env::var("CITYGAME_CLASSIC_CTY_BERN_CRT_ENTRIES").unwrap())
                .expect("read native BERN per-tile CRT states"),
        )
        .expect("decode native BERN per-tile CRT states");
        let native_xbld_entries: Vec<u8> = serde_json::from_slice(
            &std::fs::read(std::env::var("CITYGAME_CLASSIC_CTY_BERN_XBLD_ENTRIES").unwrap())
                .expect("read native BERN per-tile XBLD bytes"),
        )
        .expect("decode native BERN per-tile XBLD bytes");
        let mut payload_city = City::new_blank(&cty, "BERN").unwrap();
        clear_classic_cty_target_map(&mut payload_city);
        payload_city.pe_dw_tile_count[0] = 128 * 128;
        let mut payload_crt = Win95CrtRand::new(2_503_793_637);
        assert_eq!(native_crt_entries.len(), classic.tiles.len());
        assert_eq!(native_xbld_entries.len(), classic.tiles.len());
        for (index, tile) in classic.tiles.iter().enumerate() {
            let citygame_entry_building = payload_city.building_at(tile.x.into(), tile.y.into());
            assert_eq!(
                citygame_entry_building, native_xbld_entries[index],
                "first XBLD target-cell divergence at CTY tile #{index} ({}, {}, {:#x})",
                tile.x, tile.y, tile.id
            );
            assert_eq!(
                payload_crt.seed(),
                native_crt_entries[index],
                "first CRT stream divergence before CTY tile #{index} ({}, {}, {:#x}, XBLD {citygame_entry_building}); previous tile {:?}",
                tile.x,
                tile.y,
                tile.id,
                index.checked_sub(1).map(|previous| classic.tiles[previous])
            );
            super::super::newspaper_gifts::pe_newspaper_gift_entry_483250(
                &mut payload_city,
                &mut payload_crt,
                isize::from(tile.x),
                isize::from(tile.y),
                tile.id,
            );
        }
        let native_xbld = before_border.get("XBLD").unwrap();
        let native_xbld = native_xbld
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        let mut payload_xbld = Vec::new();
        for x in 0..128 {
            for y in 0..128 {
                payload_xbld.push(payload_city.building_at(x, y));
            }
        }
        let payload_differences = payload_xbld
            .iter()
            .zip(&native_xbld)
            .enumerate()
            .filter_map(|(i, (actual, native))| (actual != native).then_some((i, *actual, *native)))
            .collect::<Vec<_>>();
        assert!(
            payload_differences.is_empty(),
            "payload-only XBLD differs at {} cells; CRT after payload {:#010x}; first differences {:?}",
            payload_differences.len(),
            payload_crt.seed(),
            &payload_differences[..payload_differences.len().min(20)]
        );
        let native_pre_normalizer_path = std::path::PathBuf::from(
            std::env::var("CITYGAME_CLASSIC_CTY_BERN_PRE_NORMALIZER").unwrap(),
        );
        let native_pre_normalizer: BTreeMap<String, String> = serde_json::from_slice(
            &std::fs::read(native_pre_normalizer_path)
                .expect("read native BERN pre-normalizer planes"),
        )
        .expect("decode native BERN pre-normalizer plane JSON");
        let native_pre_normalizer_xbld = native_pre_normalizer
            .get("XBLD")
            .unwrap()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        let mut border_rand = Win95GameRand::new(1);
        apply_cty_border(&mut payload_city, &mut border_rand);
        for x in 0..128 {
            for y in 0..128 {
                let index = x * 128 + y;
                assert_eq!(
                    payload_city.building_at(x, y),
                    native_pre_normalizer_xbld[index],
                    "first pre-normalizer XBLD divergence at ({x},{y})"
                );
            }
        }
        let bytes = |name: &str| {
            expected
                .get(name)
                .unwrap_or_else(|| panic!("missing native {name}"))
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
        };
        let mut crt = Win95CrtRand::new(2_503_793_637);
        let mut game_rand = Win95GameRand::new(1);
        let city = load_classic_cty(&cty, &mut crt, &mut game_rand).unwrap();

        assert_eq!(
            city.saved_date(),
            Some(crate::city::CityDate {
                year: 1_977,
                month: 10
            })
        );
        assert_eq!(city.year_founded(), Some(1_900));
        let mut xbld = Vec::new();
        let mut xzon = Vec::new();
        let mut xbit = Vec::new();
        let mut altm = Vec::new();
        let mut xtxt = Vec::new();
        for x in 0..128 {
            for y in 0..128 {
                xbld.push(city.building_at(x, y));
                xzon.push(city.zone_at(x, y).unwrap_or_default().raw_bits());
                xbit.push(city.tile_bits_at(x, y).unwrap_or_default().raw_bits());
                xtxt.push(city.text_overlay_at(x, y).unwrap_or_default());
                let sample = city.altm_at(x, y);
                let word = u16::from(sample.land)
                    | (u16::from(sample.water) << 5)
                    | (u16::from(sample.tunnel_levels) << 10);
                altm.extend_from_slice(&word.to_le_bytes());
            }
        }
        for (name, plane) in [
            ("XBLD", xbld),
            ("XZON", xzon),
            ("XBIT", xbit),
            ("ALTM", altm),
            ("XTXT", xtxt),
        ] {
            let native = bytes(name);
            let differences = plane
                .iter()
                .zip(&native)
                .enumerate()
                .filter_map(|(index, (citygame, original))| {
                    (citygame != original).then_some((index, *citygame, *original))
                })
                .collect::<Vec<_>>();
            assert_eq!(plane.len(), native.len(), "{name} plane length");
            assert!(
                differences.is_empty(),
                "{name} has {} differing cells; first differences: {:?}",
                differences.len(),
                &differences[..differences.len().min(12)]
            );
        }
        assert_eq!(crt.seed(), 0xb2ed_7061);
        assert_eq!(game_rand.state(), 0x0b76_55c9);
        assert_eq!(
            (0..17)
                .map(|index| city.pe_w_neighbor_fame_rank(index).unwrap())
                .collect::<Vec<_>>(),
            [
                0, 0, 1997, 0, 2032, 2053, 0, 0, 0, 0, 0, 1999, 1996, 2056, 2094, 2152, 2207,
            ],
            "native post-reader neighbor fame ranks"
        );
    }
}
