//! Reader for the Win95 fallback's fixed-size Classic city input.
//!
//! The retail PE seeks to `0x0bc0`, reads a 240-byte WORD-swapped header, then
//! reads 12,000 big-endian tile WORDs. It places those tiles in the 100×120
//! interior in row-major order, with source rows vertically reversed. This
//! module decodes that input contract only; tile conversion and Win95 open-state
//! initialization belong to the caller because they consume live game RNG.

use crate::city::{City, TileBits};
use crate::win95_random::Win95CrtRand;
use anyhow::{Result, bail};

pub(crate) const CLASSIC_CTY_FILE_LEN: usize = 0x6a70;
const CLASSIC_CTY_HEADER_OFFSET: usize = 0x0bc0;
const CLASSIC_CTY_HEADER_LEN: usize = 0x00f0;
const CLASSIC_CTY_MAP_OFFSET: usize = CLASSIC_CTY_HEADER_OFFSET + CLASSIC_CTY_HEADER_LEN;
const CLASSIC_CTY_MAP_WORDS: usize = 12_000;
const CLASSIC_CTY_MAP_LEN: usize = CLASSIC_CTY_MAP_WORDS * 2;
const CLASSIC_CTY_MAP_WIDTH: usize = 100;
const CLASSIC_CTY_MAP_HEIGHT: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClassicCtyTile {
    pub x: u8,
    pub y: u8,
    /// The ten-bit tile identifier passed by the native reader to its converter.
    pub id: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClassicCtyFile {
    /// Header words after the native reader's per-WORD byte swap. Their
    /// semantic interpretation is intentionally left to the native open path.
    pub header_words: [u16; CLASSIC_CTY_HEADER_LEN / 2],
    /// Converter inputs in the order the Win95 reader calls PlaceTile.
    pub tiles: Vec<ClassicCtyTile>,
}

impl ClassicCtyFile {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CLASSIC_CTY_FILE_LEN {
            bail!(
                "Classic CTY input must be exactly {CLASSIC_CTY_FILE_LEN} bytes, got {}",
                bytes.len()
            );
        }

        let header_bytes = &bytes[CLASSIC_CTY_HEADER_OFFSET..CLASSIC_CTY_MAP_OFFSET];
        let mut header_words = [0; CLASSIC_CTY_HEADER_LEN / 2];
        for (word, raw) in header_words.iter_mut().zip(header_bytes.chunks_exact(2)) {
            *word = u16::from_be_bytes([raw[0], raw[1]]);
        }

        let map_bytes =
            &bytes[CLASSIC_CTY_MAP_OFFSET..CLASSIC_CTY_MAP_OFFSET + CLASSIC_CTY_MAP_LEN];
        let mut tiles = Vec::with_capacity(CLASSIC_CTY_MAP_WORDS);
        for (index, raw) in map_bytes.chunks_exact(2).enumerate() {
            let x = 14 + index % CLASSIC_CTY_MAP_WIDTH;
            let y = 123 - index / CLASSIC_CTY_MAP_WIDTH;
            debug_assert!(x < 128 && y < 128);
            tiles.push(ClassicCtyTile {
                x: x as u8,
                y: y as u8,
                id: u16::from_be_bytes([raw[0], raw[1]]) & 0x03ff,
            });
        }

        debug_assert_eq!(tiles.len(), CLASSIC_CTY_MAP_WIDTH * CLASSIC_CTY_MAP_HEIGHT);
        Ok(Self {
            header_words,
            tiles,
        })
    }
}

/// Apply the retail converter cases currently ported for tile IDs through `0x3f`.
/// This remains a converter building block, not a complete CTY import: later IDs
/// use additional tables, neighboring-tile rules, and both Win95 random streams.
pub(crate) fn apply_supported_tile(
    city: &mut City,
    crt: &mut Win95CrtRand,
    tile: ClassicCtyTile,
) -> Result<()> {
    if tile.id > 0x3f {
        bail!(
            "Classic CTY tile {:#x} is outside the ported converter range",
            tile.id
        );
    }
    let x = usize::from(tile.x);
    let y = usize::from(tile.y);
    if x >= 128 || y >= 128 {
        bail!("Classic CTY tile coordinate ({x},{y}) is outside the city map");
    }

    // FUN_00483250 routes `0x34` through DestroyBuilding(0) before placing
    // the rubble tile. Until the importer can provide that complete footprint
    // operation, reject occupied cells before changing even the common ALTM
    // prelude. Empty cells take the verified PlaceTile-only path.
    if tile.id == 0x34 && city.building_at(x, y) != 0 {
        bail!("Classic CTY rubble tile requires Win95 DestroyBuilding state");
    }

    // FUN_00483250 applies this ALTM mask before dispatching every tile ID.
    city.set_altm_word(x, y, (altm_word(city, x, y) & 0xfc5f) | 0x0040);

    match tile.id {
        0..=1 | 0x2c..=0x2f => {}
        2..=0x14 => {
            // FUN_00484320: retain the existing XBIT flags while marking the
            // water-surface bit, adjust ALTM, and set XTER to 0x10.
            let bits = city.tile_bits_at(x, y).unwrap_or_default();
            city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() | 0x04));
            city.set_altm_word(x, y, (altm_word(city, x, y) & 0xffe1) | 1);
            city.set_terrain(x, y, 0x10);
        }
        0x15..=0x24 => {
            let replacement = ((crt.random_u15() & 3) + 6) as u8;
            place_win95_tile(city, x, y, replacement);
        }
        0x25..=0x2b => {
            let replacement = ((crt.random_u15() & 3) + 9) as u8;
            place_win95_tile(city, x, y, replacement);
        }
        0x30..=0x33 => {
            // FUN_00483250 installs the tornado marker and sets the active
            // disaster BSS (`0x4c9ee8`) and type (`0x4ca93c`) to 2.
            city.set_pe_dw_disaster_active(1);
            city.set_pe_w_current_disaster_type(2);
            city.set_text_overlay(x, y, 0xfc);
        }
        0x34 => place_win95_tile(city, x, y, 5),
        0x35..=0x37 => {
            // Native avoids both rand() and PlaceTile when a building is
            // already present at this coordinate.
            if city.building_at(x, y) == 0 {
                let replacement = ((crt.random_u15() & 3) + 1) as u8;
                place_win95_tile(city, x, y, replacement);
            }
        }
        0x38..=0x3f => {
            // FUN_00483250 sets the active disaster BSS/type to 1 and marks
            // this cell with the native XTXT sentinel.
            city.set_pe_w_current_disaster_type(1);
            city.set_pe_dw_disaster_active(1);
            city.set_text_overlay(x, y, 0xff);
        }
        _ => unreachable!("range checked above"),
    }
    Ok(())
}

fn altm_word(city: &City, x: usize, y: usize) -> u16 {
    let sample = city.altm_at(x, y);
    u16::from(sample.land)
        | (u16::from(sample.water) << 5)
        | (u16::from(sample.tunnel_levels) << 10)
}

fn place_win95_tile(city: &mut City, x: usize, y: usize, replacement: u8) {
    let old = city.building_at(x, y);
    if city.zone_at(x, y).is_some_and(|zone| zone.zone_id() == 7) {
        city.pe_adjust_military_tile_count_441f86(old, replacement);
    } else {
        city.pe_dw_tile_count[usize::from(old)] =
            city.pe_dw_tile_count[usize::from(old)].wrapping_sub(1);
        city.pe_dw_tile_count[usize::from(replacement)] =
            city.pe_dw_tile_count[usize::from(replacement)].wrapping_add(1);
    }
    city.set_building(x, y, replacement);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_native_header_and_map_word_order() {
        let mut bytes = vec![0; CLASSIC_CTY_FILE_LEN];
        bytes[CLASSIC_CTY_HEADER_OFFSET..CLASSIC_CTY_HEADER_OFFSET + 2]
            .copy_from_slice(&0x1234u16.to_be_bytes());
        bytes[CLASSIC_CTY_HEADER_OFFSET + 8 * 2..CLASSIC_CTY_HEADER_OFFSET + 9 * 2]
            .copy_from_slice(&0xabcd_u16.to_be_bytes());

        let put_map_word = |bytes: &mut [u8], index: usize, word: u16| {
            let offset = CLASSIC_CTY_MAP_OFFSET + index * 2;
            bytes[offset..offset + 2].copy_from_slice(&word.to_be_bytes());
        };
        put_map_word(&mut bytes, 0, 0xfc35);
        put_map_word(&mut bytes, CLASSIC_CTY_MAP_WIDTH - 1, 0xfffe);
        put_map_word(&mut bytes, CLASSIC_CTY_MAP_WIDTH, 0x0123);
        put_map_word(&mut bytes, CLASSIC_CTY_MAP_WORDS - 1, 0x03ff);

        let parsed = ClassicCtyFile::parse(&bytes).expect("valid fixed-size Classic CTY");
        assert_eq!(parsed.header_words[0], 0x1234);
        assert_eq!(parsed.header_words[8], 0xabcd);
        assert_eq!(parsed.tiles.len(), CLASSIC_CTY_MAP_WORDS);
        assert_eq!(
            parsed.tiles[0],
            ClassicCtyTile {
                x: 14,
                y: 123,
                id: 0x35
            }
        );
        assert_eq!(
            parsed.tiles[CLASSIC_CTY_MAP_WIDTH - 1],
            ClassicCtyTile {
                x: 113,
                y: 123,
                id: 0x3fe
            }
        );
        assert_eq!(
            parsed.tiles[CLASSIC_CTY_MAP_WIDTH],
            ClassicCtyTile {
                x: 14,
                y: 122,
                id: 0x123
            }
        );
        assert_eq!(
            parsed.tiles[CLASSIC_CTY_MAP_WORDS - 1],
            ClassicCtyTile {
                x: 113,
                y: 4,
                id: 0x3ff
            }
        );
    }

    #[test]
    fn rejects_short_and_overlong_inputs() {
        assert!(ClassicCtyFile::parse(&vec![0; CLASSIC_CTY_FILE_LEN - 1]).is_err());
        assert!(ClassicCtyFile::parse(&vec![0; CLASSIC_CTY_FILE_LEN + 1]).is_err());
    }

    #[test]
    fn native_low_tile_range_updates_planes_and_consumes_crt_draws() {
        use crate::city::City;

        let mut city = City::new_blank("classic-prefix.sc2", "Classic prefix").unwrap();
        // The loaded Win95 document's native tile census already includes
        // every tile; mirror the all-empty map returned by this constructor.
        city.pe_dw_tile_count[0] = 128 * 128;
        let mut crt = Win95CrtRand::new(1);
        let mut reference_crt = crt;

        city.set_altm_word(14, 123, 0x3fff);
        city.set_tile_bits(14, 123, TileBits::new(0x21));
        apply_supported_tile(
            &mut city,
            &mut crt,
            ClassicCtyTile {
                x: 14,
                y: 123,
                id: 2,
            },
        )
        .unwrap();
        assert_eq!(city.tile_bits_at(14, 123).unwrap().raw_bits(), 0x25);
        assert_eq!(city.altm_at(14, 123).land, 1);
        assert_eq!(city.terrain_at(14, 123), 0x10);
        assert_eq!(city.text_overlay_at(14, 123), Some(0));
        assert_eq!(city.building_at(14, 123), 0);

        let expected_building = ((reference_crt.random_u15() & 3) + 6) as u8;
        apply_supported_tile(
            &mut city,
            &mut crt,
            ClassicCtyTile {
                x: 15,
                y: 123,
                id: 0x15,
            },
        )
        .unwrap();
        assert_eq!(city.building_at(15, 123), expected_building);
        assert_eq!(crt, reference_crt);
        assert_eq!(city.pe_dw_tile_count[0], 128 * 128 - 1);
        assert_eq!(city.pe_dw_tile_count[usize::from(expected_building)], 1);

        let expected_building = ((reference_crt.random_u15() & 3) + 9) as u8;
        apply_supported_tile(
            &mut city,
            &mut crt,
            ClassicCtyTile {
                x: 16,
                y: 123,
                id: 0x25,
            },
        )
        .unwrap();
        assert_eq!(city.building_at(16, 123), expected_building);
        assert_eq!(crt, reference_crt);
        assert_eq!(city.pe_dw_tile_count[0], 128 * 128 - 2);
    }

    #[test]
    fn applies_native_disaster_markers_and_conditional_tile_draws() {
        use crate::city::City;

        let mut city = City::new_blank("classic-prefix.sc2", "Classic prefix").unwrap();
        let mut crt = Win95CrtRand::new(7);
        let original_crt = crt;

        apply_supported_tile(
            &mut city,
            &mut crt,
            ClassicCtyTile {
                x: 14,
                y: 123,
                id: 0x31,
            },
        )
        .unwrap();
        assert_eq!(city.text_overlay_at(14, 123), Some(0xfc));
        assert_eq!(city.pe_w_current_disaster_type(), 2);
        assert_eq!(city.pe_dw_disaster_active(), 1);
        assert_eq!(crt, original_crt);

        apply_supported_tile(
            &mut city,
            &mut crt,
            ClassicCtyTile {
                x: 15,
                y: 123,
                id: 0x38,
            },
        )
        .unwrap();
        assert_eq!(city.text_overlay_at(15, 123), Some(0xff));
        assert_eq!(city.pe_w_current_disaster_type(), 1);
        assert_eq!(city.pe_dw_disaster_active(), 1);
        assert_eq!(crt, original_crt);

        city.set_building(16, 123, 5);
        let before_occupied_rubble = city.clone();
        assert!(
            apply_supported_tile(
                &mut city,
                &mut crt,
                ClassicCtyTile {
                    x: 16,
                    y: 123,
                    id: 0x34,
                },
            )
            .is_err()
        );
        assert_eq!(
            city.altm_at(16, 123),
            before_occupied_rubble.altm_at(16, 123)
        );
        assert_eq!(crt, original_crt);

        let mut expected_crt = crt;
        let expected = ((expected_crt.random_u15() & 3) + 1) as u8;
        apply_supported_tile(
            &mut city,
            &mut crt,
            ClassicCtyTile {
                x: 17,
                y: 123,
                id: 0x35,
            },
        )
        .unwrap();
        assert_eq!(city.building_at(17, 123), expected);
        assert_eq!(crt, expected_crt);

        let after_empty_rubble = crt;
        apply_supported_tile(
            &mut city,
            &mut crt,
            ClassicCtyTile {
                x: 17,
                y: 123,
                id: 0x36,
            },
        )
        .unwrap();
        assert_eq!(crt, after_empty_rubble, "occupied tile must skip CRT rand");

        apply_supported_tile(
            &mut city,
            &mut crt,
            ClassicCtyTile {
                x: 18,
                y: 123,
                id: 0x34,
            },
        )
        .unwrap();
        assert_eq!(city.building_at(18, 123), 5);
    }

    #[test]
    fn rejects_unported_tile_ids_before_mutating_city_or_rng() {
        use crate::city::City;

        let mut city = City::new_blank("classic-prefix.sc2", "Classic prefix").unwrap();
        let before = city.clone();
        let mut crt = Win95CrtRand::new(7);
        assert!(
            apply_supported_tile(
                &mut city,
                &mut crt,
                ClassicCtyTile {
                    x: 14,
                    y: 123,
                    id: 0x40,
                },
            )
            .is_err()
        );
        assert_eq!(city.altm_at(14, 123), before.altm_at(14, 123));
        assert_eq!(city.building_at(14, 123), before.building_at(14, 123));
        assert_eq!(crt, Win95CrtRand::new(7));
    }
}
