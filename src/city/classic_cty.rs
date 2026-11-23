//! Reader for the Win95 fallback's fixed-size Classic city input.
//!
//! The retail PE seeks to `0x0bc0`, reads a 240-byte WORD-swapped header, then
//! reads 12,000 big-endian tile WORDs. It places those tiles in the 100×120
//! interior in row-major order, with source rows vertically reversed. The app
//! layer applies the shared Win95 tile converter with its process-owned RNG.

use crate::city::CityDate;
use anyhow::{Context, Result, bail};
use std::path::Path;

pub(crate) const CLASSIC_CTY_FILE_LEN: usize = 0x6a70;
const CLASSIC_CTY_HEADER_OFFSET: usize = 0x0bc0;
const CLASSIC_CTY_HEADER_LEN: usize = 0x00f0;
const CLASSIC_CTY_MAP_OFFSET: usize = CLASSIC_CTY_HEADER_OFFSET + CLASSIC_CTY_HEADER_LEN;
const CLASSIC_CTY_MAP_WORDS: usize = 12_000;
const CLASSIC_CTY_MAP_LEN: usize = CLASSIC_CTY_MAP_WORDS * 2;
const CLASSIC_CTY_MAP_WIDTH: usize = 100;
const CLASSIC_CTY_MAP_HEIGHT: usize = 120;
const CLASSIC_CTY_FUNDS_WORD_INDEX: usize = 0x64 / 2;
const CLASSIC_CTY_AUTO_GOTO_WORD_INDEX: usize = 0x6c / 2;
const CLASSIC_CTY_AUTO_BUDGET_WORD_INDEX: usize = 0x6a / 2;

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
    pub(crate) fn has_legacy_import_size(path: impl AsRef<Path>) -> bool {
        std::fs::metadata(path).is_ok_and(|metadata| metadata.len() == CLASSIC_CTY_FILE_LEN as u64)
    }

    pub(crate) fn read(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .with_context(|| format!("reading Classic CTY {}", path.display()))?;
        Self::parse(&bytes).with_context(|| format!("parsing Classic CTY {}", path.display()))
    }

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

    /// Decode the retail calendar field at header byte offset `0x10`.
    ///
    /// The reader first swaps each WORD, then `FUN_0042a8e0` swaps the two
    /// WORDs in this signed dword. It stores the absolute value as four units
    /// per month, twelve months per year, based at 1900; values above 32,000
    /// use the retail fallback value 4,800.
    pub(crate) fn date(&self) -> CityDate {
        const DATE_WORD_INDEX: usize = 8;
        let word_swapped = (u32::from(self.header_words[DATE_WORD_INDEX + 1]) << 16)
            | u32::from(self.header_words[DATE_WORD_INDEX]);
        let units = word_swapped.rotate_left(16) as i32;
        let units = units.unsigned_abs();
        let units = if units > 32_000 { 4_800 } else { units };
        let year = 1_900 + units / 48;
        let month = ((units / 4) % 12 + 1) as u8;
        CityDate {
            year: year as u16,
            month,
        }
    }

    /// The retail reader copies the WORD at header byte offset `0x6a` to the
    /// Win95 Auto-Budget DWORD at `0x4cc4b0` without normalization.
    pub(crate) fn auto_budget_option(&self) -> u32 {
        u32::from(self.header_words[CLASSIC_CTY_AUTO_BUDGET_WORD_INDEX])
    }

    /// The retail reader word-swaps the dword at header byte offset `0x64`
    /// before publishing it as city funds at `0x4ca444`.
    pub(crate) fn starting_funds(&self) -> i32 {
        let high = u32::from(self.header_words[CLASSIC_CTY_FUNDS_WORD_INDEX]);
        let low = u32::from(self.header_words[CLASSIC_CTY_FUNDS_WORD_INDEX + 1]);
        ((high << 16) | low) as i32
    }

    /// The retail reader copies the WORD at header byte offset `0x6c` to the
    /// Auto-Goto option global at `0x4ca5d8` without normalization.
    pub(crate) fn auto_goto_option(&self) -> u32 {
        u32::from(self.header_words[CLASSIC_CTY_AUTO_GOTO_WORD_INDEX])
    }
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
    fn decodes_native_four_units_per_month_calendar_and_fallback() {
        let mut bytes = vec![0; CLASSIC_CTY_FILE_LEN];
        let date_offset = CLASSIC_CTY_HEADER_OFFSET + 0x10;
        // FUN_0042a8e0's halfword swap yields 4,324, or 90 years + 1 month.
        bytes[date_offset..date_offset + 4].copy_from_slice(&[0, 0, 0x10, 0xe4]);
        let city = ClassicCtyFile::parse(&bytes).unwrap();
        assert_eq!(
            city.date(),
            CityDate {
                year: 1_990,
                month: 2
            }
        );

        // BAZ.CTY's source field at 0x10 is outside the accepted calendar
        // range, so the retail loader clamps it to 4,800 units (January 2000).
        bytes[date_offset..date_offset + 4].copy_from_slice(&[0x4c, 0xcc, 0x00, 0x04]);
        let baz = ClassicCtyFile::parse(&bytes).unwrap();
        assert_eq!(
            baz.date(),
            CityDate {
                year: 2_000,
                month: 1
            }
        );
    }

    #[test]
    fn decodes_berns_nonfallback_native_calendar_field_at_header_0x10() {
        let mut bytes = vec![0; CLASSIC_CTY_FILE_LEN];
        let header = CLASSIC_CTY_HEADER_OFFSET;
        // The archived BERN.CTY calendar field is 3,734 four-unit months.
        // Its adjacent header dword at 0x0c is unrelated and very large.
        bytes[header + 0x0c..header + 0x10].copy_from_slice(&0xfa24_05dcu32.to_be_bytes());
        bytes[header + 0x10..header + 0x14].copy_from_slice(&3_734u32.to_be_bytes());

        let city = ClassicCtyFile::parse(&bytes).unwrap();

        assert_eq!(
            city.date(),
            CityDate {
                year: 1_977,
                month: 10
            }
        );
    }
}
