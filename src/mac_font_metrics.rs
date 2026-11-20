//! Character advances from a classic Macintosh bitmap font resource.
//!
//! Font bytes are supplied by the user's original system; no font data is
//! embedded here. Bitmap sizes use NFNT advances; other sizes use the
//! suitcase's hinted TrueType font.

use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The plain faces associated with a family in a classic font suitcase.
/// Data belongs to the user-supplied resource fork, not CityGame.
#[derive(Debug)]
pub(crate) struct MacFontFamily {
    pub(crate) bitmap: BTreeMap<u16, MacBitmapFontMetrics>,
    pub(crate) truetype: Option<Vec<u8>>,
    scaled_advances: Mutex<BTreeMap<u16, Vec<u16>>>,
}

impl MacFontFamily {
    /// Accept a bare resource fork or a MacBinary file containing one.
    pub(crate) fn from_suitcase(data: &[u8], family_id: i16) -> Result<Self> {
        let fork = if data.len() >= 128 && data[0] == 0 && (1..=63).contains(&data[1]) {
            let data_len = be32(data, 83)? as usize;
            let resource_len = be32(data, 87)? as usize;
            let start = 128usize
                .checked_add(
                    data_len
                        .checked_add(127)
                        .context("MacBinary length overflow")?
                        & !127,
                )
                .context("MacBinary resource offset overflow")?;
            slice(data, start, resource_len)?
        } else {
            data
        };
        let resource =
            |kind: &[u8; 4], id: i16| -> Result<&[u8]> { resource_from_fork(fork, kind, id) };
        let fond = resource(b"FOND", family_id)?;
        let count = usize::from(be16(fond, 52)?) + 1;
        ensure!(
            count <= (fond.len().saturating_sub(54)) / 6,
            "truncated FOND associations"
        );
        let mut family = Self {
            bitmap: BTreeMap::new(),
            truetype: None,
            scaled_advances: Mutex::new(BTreeMap::new()),
        };
        for i in 0..count {
            let at = 54 + i * 6;
            let size = be16(fond, at)?;
            let style = be16(fond, at + 2)?;
            let id = be16(fond, at + 4)? as i16;
            if style != 0 {
                continue;
            }
            if size == 0 {
                family.truetype = Some(resource(b"sfnt", id)?.to_vec());
            } else {
                family.bitmap.insert(
                    size,
                    MacBitmapFontMetrics::from_nfnt(resource(b"NFNT", id)?)?,
                );
            }
        }
        ensure!(
            !family.bitmap.is_empty() || family.truetype.is_some(),
            "empty font family"
        );
        Ok(family)
    }

    /// Measure Mac Roman bytes at a point size. A native bitmap takes
    /// precedence over the outline face, as observed on System 7.
    pub(crate) fn width(&self, size: u16, text: &[u8]) -> Result<u32> {
        ensure!(size > 0, "invalid zero font size");
        if let Some(bitmap) = self.bitmap.get(&size) {
            return bitmap.width(text).context("missing bitmap character width");
        }
        let bytes = self.truetype.as_deref().context("missing scaled font")?;
        let mut cached = self
            .scaled_advances
            .lock()
            .map_err(|_| anyhow::anyhow!("font cache poisoned"))?;
        if !cached.contains_key(&size) {
            cached.insert(size, hinted_advances(bytes, size)?);
        }
        text.iter().try_fold(0u32, |sum, &ch| {
            sum.checked_add(u32::from(cached[&size][usize::from(ch)]))
                .context("font width overflow")
        })
    }
}

/// Load the user's sfnt into an independent FreeType instance. Neither handle
/// escapes this function; the font bytes outlive the memory face.
fn hinted_advances(data: &[u8], size: u16) -> Result<Vec<u16>> {
    use freetype_sys as ft;
    struct Handles {
        library: ft::FT_Library,
        face: ft::FT_Face,
    }
    impl Drop for Handles {
        fn drop(&mut self) {
            // SAFETY: these are either null or handles successfully created
            // below, with the face released before its owning library.
            unsafe {
                if !self.face.is_null() {
                    ft::FT_Done_Face(self.face);
                }
                if !self.library.is_null() {
                    ft::FT_Done_FreeType(self.library);
                }
            }
        }
    }
    let mut h = Handles {
        library: std::ptr::null_mut(),
        face: std::ptr::null_mut(),
    };
    let length = data.len().try_into().context("font resource too large")?;
    // SAFETY: handles are local, all output pointers are valid, and `data`
    // remains borrowed until the memory face is destroyed by Handles.
    unsafe {
        ensure!(
            ft::FT_Init_FreeType(&mut h.library) == 0,
            "FreeType initialization failed"
        );
        ensure!(
            ft::FT_New_Memory_Face(h.library, data.as_ptr(), length, 0, &mut h.face) == 0,
            "invalid TrueType font"
        );
        ensure!(
            ft::FT_Select_Charmap(h.face, ft::FT_ENCODING_APPLE_ROMAN) == 0,
            "font lacks Mac Roman character map"
        );
        ensure!(
            ft::FT_Set_Pixel_Sizes(h.face, 0, u32::from(size)) == 0,
            "invalid TrueType size"
        );
        let mut widths = Vec::with_capacity(256);
        for ch in 0..=255u8 {
            let glyph = ft::FT_Get_Char_Index(h.face, ch.into());
            ensure!(
                ft::FT_Load_Glyph(h.face, glyph, ft::FT_LOAD_DEFAULT) == 0,
                "could not load font glyph"
            );
            let advance = (*(*h.face).glyph).advance.x;
            ensure!(
                advance >= 0 && advance % 64 == 0,
                "non-integral hinted advance"
            );
            widths.push(u16::try_from(advance / 64).context("font advance overflow")?);
        }
        Ok(widths)
    }
}

fn slice(data: &[u8], start: usize, len: usize) -> Result<&[u8]> {
    data.get(start..start.checked_add(len).context("resource offset overflow")?)
        .context("truncated resource fork")
}

fn be16(data: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_be_bytes(slice(data, at, 2)?.try_into()?))
}

fn be32(data: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(slice(data, at, 4)?.try_into()?))
}

fn resource_from_fork<'a>(fork: &'a [u8], kind: &[u8; 4], id: i16) -> Result<&'a [u8]> {
    let data = slice(fork, be32(fork, 0)? as usize, be32(fork, 8)? as usize)?;
    let map = slice(fork, be32(fork, 4)? as usize, be32(fork, 12)? as usize)?;
    let types = be16(map, 24)? as usize;
    let type_count = usize::from(be16(map, types)?) + 1;
    ensure!(
        type_count <= map.len().saturating_sub(types + 2) / 8,
        "truncated resource types"
    );
    for i in 0..type_count {
        let at = types + 2 + i * 8;
        if slice(map, at, 4)? != kind {
            continue;
        }
        let count = usize::from(be16(map, at + 4)?) + 1;
        let refs = types
            .checked_add(usize::from(be16(map, at + 6)?))
            .context("resource list overflow")?;
        ensure!(
            count <= map.len().saturating_sub(refs) / 12,
            "truncated resource references"
        );
        for j in 0..count {
            let entry = refs + j * 12;
            if be16(map, entry)? as i16 != id {
                continue;
            }
            let offset = (be32(map, entry + 4)? & 0x00ff_ffff) as usize;
            let size = be32(data, offset)? as usize;
            return slice(data, offset + 4, size);
        }
    }
    anyhow::bail!(
        "resource {:?} {id} not found",
        String::from_utf8_lossy(kind)
    )
}

#[derive(Debug, Clone)]
pub(crate) struct MacBitmapFontMetrics {
    first: u16,
    last: u16,
    // Includes the final missing-character entry.
    advances: Vec<Option<u8>>,
    resource: Vec<u8>,
}

/// One glyph from the source bitmap, before QuickDraw applies its text mode.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MacBitmapGlyph {
    /// Horizontal bit offset in the original shared NFNT bitmap strip.
    pub(crate) bitmap_x: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) left_bearing: i8,
    pub(crate) advance: u8,
    /// Row-major one-bit samples represented as zero or one.
    pub(crate) pixels: Vec<u8>,
}

impl MacBitmapFontMetrics {
    pub(crate) fn from_nfnt(data: &[u8]) -> Result<Self> {
        let word = |at: usize| -> Result<u16> {
            let bytes: [u8; 2] = data
                .get(at..at + 2)
                .context("truncated NFNT header")?
                .try_into()?;
            Ok(u16::from_be_bytes(bytes))
        };
        let first = word(2)?;
        let last = word(4)?;
        ensure!(first <= last && last <= 255, "invalid NFNT character range");
        // owTLoc counts words from the address of the owTLoc field itself.
        let table = 16usize
            .checked_add(usize::from(word(16)?) * 2)
            .context("invalid NFNT width table offset")?;
        let count = usize::from(last - first) + 2;
        let end = table
            .checked_add(count * 2)
            .context("invalid NFNT width table length")?;
        let widths = data.get(table..end).context("truncated NFNT width table")?;
        let advances = widths
            .chunks_exact(2)
            .map(|pair| {
                // An offset/width word of -1 marks an absent glyph.
                (pair != [0xff, 0xff]).then_some(pair[1])
            })
            .collect();
        Ok(Self {
            first,
            last,
            advances,
            resource: data.to_vec(),
        })
    }

    /// Read the original NFNT bitmap and location table for a Mac Roman byte.
    /// The metrics-only reader intentionally remains usable for short fixtures;
    /// this method checks the additional bitmap structures when they are used.
    #[allow(dead_code)]
    pub(crate) fn glyph(&self, byte: u8) -> Result<MacBitmapGlyph> {
        let data = &self.resource;
        let height = usize::from(be16(data, 14)?);
        let row_bytes = usize::from(be16(data, 24)?)
            .checked_mul(2)
            .context("invalid NFNT row width")?;
        ensure!(row_bytes != 0, "empty NFNT bitmap row");
        let image_len = height
            .checked_mul(row_bytes)
            .context("invalid NFNT bitmap size")?;
        let image = slice(data, 26, image_len)?;
        let location_table = 26usize
            .checked_add(image_len)
            .context("invalid NFNT location table")?;
        let missing = usize::from(self.last - self.first) + 1;
        let code = u16::from(byte);
        let candidate = if (self.first..=self.last).contains(&code) {
            usize::from(code - self.first)
        } else {
            missing
        };
        let index = if self.advances[candidate].is_some() {
            candidate
        } else {
            missing
        };
        let loc = location_table
            .checked_add(index * 2)
            .context("invalid NFNT glyph index")?;
        let start = usize::from(be16(data, loc)?);
        let end = usize::from(be16(data, loc + 2)?);
        ensure!(
            start <= end && end <= row_bytes * 8,
            "invalid NFNT glyph location"
        );
        let widths = 16usize
            .checked_add(usize::from(be16(data, 16)?) * 2)
            .context("invalid NFNT width table")?;
        let entry = slice(data, widths + index * 2, 2)?;
        ensure!(entry != [0xff, 0xff], "missing NFNT fallback glyph");
        let width = end - start;
        let mut pixels =
            Vec::with_capacity(height.checked_mul(width).context("invalid glyph size")?);
        for row in image.chunks_exact(row_bytes) {
            for x in start..end {
                pixels.push(u8::from(row[x / 8] & (0x80 >> (x % 8)) != 0));
            }
        }
        ensure!(pixels.len() == height * width, "invalid NFNT bitmap rows");
        Ok(MacBitmapGlyph {
            bitmap_x: start,
            width,
            height,
            left_bearing: entry[0] as i8,
            advance: entry[1],
            pixels,
        })
    }

    pub(crate) fn width(&self, text: &[u8]) -> Option<u32> {
        let missing = *self.advances.last()?;
        text.iter().try_fold(0u32, |sum, &byte| {
            let code = u16::from(byte);
            let advance = if (self.first..=self.last).contains(&code) {
                self.advances[usize::from(code - self.first)].or(missing)
            } else {
                missing
            }?;
            sum.checked_add(u32::from(advance))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_glyphs_use_locations_bearings_and_missing_glyph() {
        let mut nfnt = vec![0; 44];
        nfnt[2..4].copy_from_slice(&65u16.to_be_bytes());
        nfnt[4..6].copy_from_slice(&66u16.to_be_bytes());
        nfnt[14..16].copy_from_slice(&2u16.to_be_bytes());
        nfnt[16..18].copy_from_slice(&11u16.to_be_bytes());
        nfnt[24..26].copy_from_slice(&1u16.to_be_bytes());
        nfnt[26..30].copy_from_slice(&[0xac, 0, 0x54, 0]);
        nfnt[30..38].copy_from_slice(&[0, 0, 0, 3, 0, 5, 0, 6]);
        nfnt[38..44].copy_from_slice(&[0, 4, 0xff, 0xff, 0xfe, 5]);
        let font = MacBitmapFontMetrics::from_nfnt(&nfnt).unwrap();
        assert_eq!(
            font.glyph(b'A').unwrap(),
            MacBitmapGlyph {
                bitmap_x: 0,
                width: 3,
                height: 2,
                left_bearing: 0,
                advance: 4,
                pixels: vec![1, 0, 1, 0, 1, 0],
            }
        );
        assert_eq!(
            font.glyph(b'B').unwrap(),
            MacBitmapGlyph {
                bitmap_x: 5,
                width: 1,
                height: 2,
                left_bearing: -2,
                advance: 5,
                pixels: vec![1, 1],
            }
        );
        assert_eq!(font.glyph(b'Z').unwrap(), font.glyph(b'B').unwrap());
        nfnt[36..38].copy_from_slice(&17u16.to_be_bytes());
        assert!(
            MacBitmapFontMetrics::from_nfnt(&nfnt)
                .unwrap()
                .glyph(b'B')
                .is_err()
        );
    }

    #[test]
    #[ignore = "requires a privately imported System 7 Chicago suitcase"]
    fn chicago_clock_glyph_rows_match_native_text_draw_operands() {
        let path = std::env::var("CITYGAME_MAC_CHICAGO_FONT").expect("private font fixture");
        let family = MacFontFamily::from_suitcase(&std::fs::read(path).unwrap(), 0).unwrap();
        let font = family.bitmap.get(&12).unwrap();
        let one = font.glyph(b'1').unwrap();
        let two = font.glyph(b'2').unwrap();
        assert_eq!((one.bitmap_x, two.bitmap_x), (115, 118));
        assert_eq!(
            (one.width, two.width, one.height, two.height),
            (3, 6, 15, 15)
        );
        assert_eq!(&one.pixels[10 * one.width..11 * one.width], &[0, 1, 1]);
        assert_eq!(
            &two.pixels[10 * two.width..10 * two.width + 4],
            &[1, 1, 0, 0]
        );
    }

    #[test]
    fn parses_widths_and_uses_missing_character() {
        let mut data = vec![0; 30];
        data[2..4].copy_from_slice(&65u16.to_be_bytes());
        data[4..6].copy_from_slice(&66u16.to_be_bytes());
        data[16..18].copy_from_slice(&4u16.to_be_bytes());
        data[24..30].copy_from_slice(&[0, 5, 0xff, 0xff, 0, 3]);
        let font = MacBitmapFontMetrics::from_nfnt(&data).unwrap();
        assert_eq!(font.width(b"ABZ"), Some(11));
        assert!(MacBitmapFontMetrics::from_nfnt(&data[..29]).is_err());
        assert!(MacFontFamily::from_suitcase(&[0; 128], 2).is_err());
    }

    #[test]
    fn reads_associations_from_a_padded_macbinary() {
        // A one-face suitcase with a deliberately unrelated resource ID.
        let mut fond = vec![0; 60];
        fond[54..56].copy_from_slice(&10u16.to_be_bytes());
        fond[58..60].copy_from_slice(&1234u16.to_be_bytes());
        let mut nfnt = vec![0; 28];
        nfnt[2..4].copy_from_slice(&65u16.to_be_bytes());
        nfnt[4..6].copy_from_slice(&65u16.to_be_bytes());
        nfnt[16..18].copy_from_slice(&4u16.to_be_bytes());
        nfnt[24..28].copy_from_slice(&[0, 5, 0, 3]);
        let mut resource_data = Vec::new();
        for item in [&fond, &nfnt] {
            resource_data.extend_from_slice(&(item.len() as u32).to_be_bytes());
            resource_data.extend_from_slice(item);
        }
        let mut map = vec![0; 28 + 2 + 16 + 24];
        map[24..26].copy_from_slice(&28u16.to_be_bytes());
        map[28..30].copy_from_slice(&1u16.to_be_bytes());
        map[30..34].copy_from_slice(b"FOND");
        map[36..38].copy_from_slice(&18u16.to_be_bytes());
        map[38..42].copy_from_slice(b"NFNT");
        map[44..46].copy_from_slice(&30u16.to_be_bytes());
        map[46..48].copy_from_slice(&2u16.to_be_bytes());
        map[58..60].copy_from_slice(&1234u16.to_be_bytes());
        map[62..66].copy_from_slice(&(64u32).to_be_bytes());
        let mut fork = vec![0; 256];
        fork[0..4].copy_from_slice(&256u32.to_be_bytes());
        fork[4..8].copy_from_slice(&(256u32 + resource_data.len() as u32).to_be_bytes());
        fork[8..12].copy_from_slice(&(resource_data.len() as u32).to_be_bytes());
        fork[12..16].copy_from_slice(&(map.len() as u32).to_be_bytes());
        fork.extend_from_slice(&resource_data);
        fork.extend_from_slice(&map);
        let mut macbinary = vec![0; 256];
        macbinary[1] = 4;
        macbinary[2..6].copy_from_slice(b"Font");
        macbinary[83..87].copy_from_slice(&7u32.to_be_bytes());
        macbinary[87..91].copy_from_slice(&(fork.len() as u32).to_be_bytes());
        macbinary.extend_from_slice(&fork);
        let family = MacFontFamily::from_suitcase(&macbinary, 2).unwrap();
        assert_eq!(family.bitmap[&10].width(b"AB"), Some(8));
        assert_eq!(family.width(10, b"AB").unwrap(), 8);
        assert!(family.width(8, b"AB").is_err());
        assert!(family.truetype.is_none());
        macbinary.pop();
        assert!(MacFontFamily::from_suitcase(&macbinary, 2).is_err());
    }

    #[test]
    #[ignore = "requires private original System 7 font and Quadra capture"]
    fn matches_live_quadra_new_york_measurements() -> Result<()> {
        let dir = std::path::PathBuf::from(std::env::var("CITYGAME_MAC_PAPER_WIDTHS")?);
        let suitcase = std::fs::read(dir.join("newyork.bin"))?;
        let family = MacFontFamily::from_suitcase(&suitcase, 2)?;
        assert_eq!(
            family.bitmap.keys().copied().collect::<Vec<_>>(),
            [9, 10, 12, 14, 18, 20, 24]
        );
        assert!(
            family
                .truetype
                .as_ref()
                .is_some_and(|sfnt| sfnt.starts_with(&[0, 1, 0, 0]))
        );
        let raw_family = MacFontFamily::from_suitcase(&suitcase[128..], 2)?;
        assert_eq!(raw_family.bitmap.len(), family.bitmap.len());
        let mut checked = 0;
        for name in ["widths-page0-font.csv", "widths-page1.csv"] {
            for line in std::fs::read_to_string(dir.join(name))?.lines() {
                let fields: Vec<_> = line.split(',').collect();
                let expected: u32 = fields[0].parse()?;
                let size: u16 = fields[1].parse()?;
                let hex = fields.last().context("missing captured text")?.as_bytes();
                ensure!(hex.len() % 2 == 0, "invalid captured text");
                let bytes = hex
                    .chunks_exact(2)
                    .map(|pair| {
                        u8::from_str_radix(std::str::from_utf8(pair)?, 16)
                            .map_err(anyhow::Error::from)
                    })
                    .collect::<Result<Vec<_>>>()?;
                assert_eq!(family.width(size, &bytes)?, expected, "{name}, size {size}");
                checked += 1;
            }
        }
        assert_eq!(checked, 36, "incomplete native measurements");
        Ok(())
    }
}
