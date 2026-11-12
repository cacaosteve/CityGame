// City chunk-layer decode/encode and legacy-resolution conversions.

use super::*;

/// Version-one marker for native-order XFIR in files carrying CityGame metadata.
pub(super) const CITYGAME_FIRE_COVERAGE_CHUNK: &str = "CGFC";
pub(super) const CITYGAME_FIRE_COVERAGE_VERSION: &[u8; 4] = b"\0\0\0\x01";
/// Marks CityGame saves whose other compact maps also use native order.
pub(super) const CITYGAME_MAP_PLANES_CHUNK: &str = "CGMP";
pub(super) const CITYGAME_MAP_PLANES_VERSION: &[u8; 4] = b"\0\0\0\x01";

pub(super) enum MicrosimOrder {
    Native,
    Legacy,
}

impl MicrosimOrder {
    fn index(&self, x: usize, y: usize, size: usize) -> usize {
        match self {
            Self::Native => x * size + y,
            Self::Legacy => y * size + x,
        }
    }
}

fn compact_map_order(chunks: &[CityChunk]) -> MicrosimOrder {
    let canonical = chunks.iter().any(|chunk| {
        chunk.name == CITYGAME_MAP_PLANES_CHUNK && chunk.data == CITYGAME_MAP_PLANES_VERSION
    });
    let legacy_citygame = chunks.iter().any(|chunk| {
        matches!(
            chunk.name.as_str(),
            CITYGAME_EXTENSION_CHUNK | CITYGAME_FIRE_COVERAGE_CHUNK
        )
    });
    if canonical || !legacy_citygame {
        MicrosimOrder::Native
    } else {
        MicrosimOrder::Legacy
    }
}

pub(super) fn decode_expected_chunk(
    chunks: &[CityChunk],
    name: &str,
    expected_len: usize,
) -> Result<Vec<u8>> {
    decode_expected_chunk_for_kind(chunks, name, expected_len, CityFileKind::WinSc2)
}

pub(super) fn decode_expected_chunk_for_kind(
    chunks: &[CityChunk],
    name: &str,
    expected_len: usize,
    kind: CityFileKind,
) -> Result<Vec<u8>> {
    let chunk = chunks
        .iter()
        .find(|chunk| chunk.name == name)
        .ok_or_else(|| anyhow!("missing {name} chunk"))?;

    let decoded = {
        let (first, second): (fn(&[u8]) -> Result<Vec<u8>>, fn(&[u8]) -> Result<Vec<u8>>) =
            if kind == CityFileKind::MacCity {
                (mac_rle_decode, sc2_rle_decode)
            } else {
                (sc2_rle_decode, mac_rle_decode)
            };
        let decoded = first(&chunk.data)
            .ok()
            .filter(|decoded| decoded.len() == expected_len)
            .or_else(|| {
                second(&chunk.data)
                    .ok()
                    .filter(|decoded| decoded.len() == expected_len)
            });
        decoded.unwrap_or_else(|| {
            if chunk.data.len() == expected_len {
                chunk.data.clone()
            } else {
                Vec::new()
            }
        })
    };
    if decoded.len() != expected_len {
        bail!(
            "{name} decoded to {} bytes, expected {expected_len}",
            decoded.len()
        );
    }
    Ok(decoded)
}

fn decode_rle_for_expected_sizes(
    input: &[u8],
    kind: CityFileKind,
    expected_sizes: &[usize],
    name: &str,
) -> Result<Vec<u8>> {
    let (first, second): (fn(&[u8]) -> Result<Vec<u8>>, fn(&[u8]) -> Result<Vec<u8>>) =
        if kind == CityFileKind::MacCity {
            (mac_rle_decode, sc2_rle_decode)
        } else {
            (sc2_rle_decode, mac_rle_decode)
        };
    first(input)
        .ok()
        .filter(|decoded| expected_sizes.contains(&decoded.len()))
        .or_else(|| {
            second(input)
                .ok()
                .filter(|decoded| expected_sizes.contains(&decoded.len()))
        })
        .ok_or_else(|| anyhow!("{name} RLE did not decode to a supported plane size"))
}

pub(super) fn decode_win95_misc_record(chunks: &[CityChunk]) -> Result<Vec<u8>> {
    let misc = decode_expected_chunk(chunks, "MISC", WIN95_MISC_RECORD_SIZE)?;
    let version = u32::from_be_bytes(misc[..4].try_into().expect("MISC header is four bytes"));
    if version != WIN95_MISC_RECORD_VERSION {
        bail!(
            "unsupported Win95 MISC record version {version:#x}; expected {WIN95_MISC_RECORD_VERSION:#x}"
        );
    }
    Ok(misc)
}

/// Decode Win95 `FUN_00430c00`'s on-disk `[length][text]` XLAB rows directly;
/// the PE expands these into NUL-terminated strings only in its resident table.
pub(super) fn decode_labels(chunks: &[CityChunk], kind: CityFileKind) -> Result<Vec<String>> {
    let raw = decode_expected_chunk_for_kind(
        chunks,
        "XLAB",
        CITY_LABEL_COUNT * CITY_LABEL_RECORD_LEN,
        kind,
    )?;
    let mut labels = Vec::with_capacity(CITY_LABEL_COUNT);
    for record in raw.chunks_exact(CITY_LABEL_RECORD_LEN) {
        let count = usize::from(record[0]).min(CITY_LABEL_TEXT_MAX);
        let text = String::from_utf8_lossy(&record[1..1 + count])
            .trim_end_matches(char::from(0))
            .to_owned();
        labels.push(text);
    }
    Ok(labels)
}

pub(super) fn decode_xmic(chunks: &[CityChunk], kind: CityFileKind) -> Result<Vec<MicrosimRecord>> {
    let expected = MICROSIM_COUNT * MICROSIM_RECORD_LEN;
    let raw = decode_expected_chunk_for_kind(chunks, "XMIC", expected, kind)?;
    Ok(raw
        .chunks_exact(MICROSIM_RECORD_LEN)
        .map(MicrosimRecord::from_bytes)
        .collect())
}

pub(super) fn decode_altm(chunks: &[CityChunk], kind: CityFileKind) -> Result<Vec<AltmSample>> {
    let bytes = decode_expected_chunk_for_kind(chunks, "ALTM", CITY_TILES * 2, kind)?;

    let mut samples = Vec::with_capacity(CITY_TILES);
    for word in bytes.chunks_exact(2) {
        let raw = u16::from_be_bytes([word[0], word[1]]);
        samples.push(AltmSample {
            land: (raw & 0x1f) as u8,
            water: ((raw >> 5) & 0x1f) as u8,
            tunnel_levels: ((raw >> 10) & 0x3f) as u8,
        });
    }
    Ok(samples)
}

pub(super) fn decode_zones(chunks: &[CityChunk], kind: CityFileKind) -> Result<Vec<ZoneFlags>> {
    Ok(
        decode_expected_chunk_for_kind(chunks, "XZON", CITY_TILES, kind)?
            .into_iter()
            .map(ZoneFlags::new)
            .collect(),
    )
}

pub(super) fn decode_tile_bits(chunks: &[CityChunk], kind: CityFileKind) -> Result<Vec<TileBits>> {
    Ok(
        decode_expected_chunk_for_kind(chunks, "XBIT", CITY_TILES, kind)?
            .into_iter()
            .map(TileBits::new)
            .collect(),
    )
}

/// Decode `XTRF` — per-tile congestion (`16384` bytes) or Win95 64×64 microsim (`4096`).
pub(super) fn decode_xtrf(
    chunks: &[CityChunk],
    kind: CityFileKind,
) -> Result<(Vec<u8>, XtrfStorageFormat)> {
    let chunk = chunks
        .iter()
        .find(|chunk| chunk.name == "XTRF")
        .ok_or_else(|| anyhow!("missing XTRF chunk"))?;

    let decoded = if chunk.data.len() == CITY_TILES || chunk.data.len() == 4096 {
        chunk.data.clone()
    } else {
        let expected_sizes = if kind.is_win95() {
            [4096].as_slice()
        } else {
            [CITY_TILES, 4096].as_slice()
        };
        decode_rle_for_expected_sizes(&chunk.data, kind, expected_sizes, "XTRF")?
    };

    match decoded.len() {
        CITY_TILES if !kind.is_win95() => Ok((decoded, XtrfStorageFormat::PerTile128)),
        4096 => Ok((
            resize_map_64_to_128(&decoded, compact_map_order(chunks)),
            XtrfStorageFormat::Microsim64,
        )),
        len if kind.is_win95() => bail!("Win95 XTRF decoded to {len} bytes, expected 4096"),
        len => bail!("XTRF decoded to {len} bytes, expected {CITY_TILES} or 4096"),
    }
}

pub(super) fn encode_xtrf_for_save(
    kind: CityFileKind,
    storage: XtrfStorageFormat,
    data: &[u8],
) -> Vec<u8> {
    if kind.is_win95() || storage == XtrfStorageFormat::Microsim64 {
        resize_map_128_to_64(data)
    } else {
        data.to_vec()
    }
}

/// Decode Win95 `XTHG` — 40 overlay actor records (`0x1e0` bytes raw).
pub(super) fn decode_xthg(
    chunks: &[CityChunk],
    kind: CityFileKind,
) -> Result<Vec<Win95OverlayRecord>> {
    let chunk = chunks
        .iter()
        .find(|chunk| chunk.name == "XTHG")
        .ok_or_else(|| anyhow!("missing XTHG chunk"))?;

    let decoded = if chunk.data.len() == XTHG_CHUNK_SIZE {
        chunk.data.clone()
    } else {
        city_rle_decode(&chunk.data, kind).with_context(|| "decoding XTHG")?
    };
    decode_xthg_records(&decoded)
}

/// Nearest-neighbor upscale for 64×64 simulation maps.
pub(super) fn resize_map_64_to_128(data: &[u8], order: MicrosimOrder) -> Vec<u8> {
    const OLD: usize = 64;
    let mut out = vec![0u8; CITY_TILES];
    for row in 0..CITY_WIDTH {
        for col in 0..CITY_HEIGHT {
            let src_x = col * OLD / CITY_WIDTH;
            let src_y = row * OLD / CITY_HEIGHT;
            out[city_index(col, row)] = data[order.index(src_x, src_y, OLD)];
        }
    }
    out
}

/// Downscale 128×128 tiles to Win95 64×64 microsim (top-left of each 2×2 block).
pub(super) fn resize_map_128_to_64(data: &[u8]) -> Vec<u8> {
    const MICRO: usize = 64;
    let mut out = vec![0u8; MICRO * MICRO];
    for macro_y in 0..MICRO {
        for macro_x in 0..MICRO {
            let tile_x = macro_x * 2;
            let tile_y = macro_y * 2;
            out[macro_x * MICRO + macro_y] = data[city_index(tile_x, tile_y)];
        }
    }
    out
}

/// Nearest-neighbor upscale for 32×32 simulation maps.
fn resize_map_32_to_128(data: &[u8], order: MicrosimOrder) -> Vec<u8> {
    const OLD: usize = 32;
    let mut out = vec![0u8; CITY_TILES];
    for row in 0..CITY_WIDTH {
        for col in 0..CITY_HEIGHT {
            let src_x = col * OLD / CITY_WIDTH;
            let src_y = row * OLD / CITY_HEIGHT;
            out[city_index(col, row)] = data[order.index(src_x, src_y, OLD)];
        }
    }
    out
}

/// Downscale 128×128 tiles to 32×32 microsim (max of each 4×4 block).
pub(super) fn encode_microsim32_for_save(data: &[u8]) -> Vec<u8> {
    const MICRO: usize = 32;
    let mut out = vec![0u8; MICRO * MICRO];
    for macro_y in 0..MICRO {
        for macro_x in 0..MICRO {
            let mut value = 0_u8;
            for dy in 0..4 {
                for dx in 0..4 {
                    let tile_x = macro_x * 4 + dx;
                    let tile_y = macro_y * 4 + dy;
                    value = value.max(data[city_index(tile_x, tile_y)]);
                }
            }
            out[macro_x * MICRO + macro_y] = value;
        }
    }
    out
}

pub(super) fn decode_microsim64_chunk(
    chunks: &[CityChunk],
    name: &str,
    kind: CityFileKind,
) -> Result<Vec<u8>> {
    let chunk = chunks
        .iter()
        .find(|chunk| chunk.name == name)
        .ok_or_else(|| anyhow!("missing {name} chunk"))?;
    let decoded = if chunk.data.len() == CITY_TILES || chunk.data.len() == 4096 {
        chunk.data.clone()
    } else {
        let expected_sizes = if kind.is_win95() {
            [4096].as_slice()
        } else {
            [CITY_TILES, 4096].as_slice()
        };
        decode_rle_for_expected_sizes(&chunk.data, kind, expected_sizes, name)?
    };
    match decoded.len() {
        CITY_TILES if !kind.is_win95() => Ok(decoded),
        4096 => Ok(resize_map_64_to_128(&decoded, compact_map_order(chunks))),
        CITY_TILES => bail!("Win95 {name} decoded to {CITY_TILES} bytes, expected 4096"),
        len => bail!("{name} decoded to {len} bytes, expected {CITY_TILES} or 4096"),
    }
}

fn decode_microsim32_chunk(
    chunks: &[CityChunk],
    name: &str,
    kind: CityFileKind,
) -> Result<Vec<u8>> {
    let chunk = chunks
        .iter()
        .find(|chunk| chunk.name == name)
        .ok_or_else(|| anyhow!("missing {name} chunk"))?;
    let decoded = if chunk.data.len() == 1024 {
        chunk.data.clone()
    } else {
        decode_rle_for_expected_sizes(&chunk.data, kind, &[1024], name)?
    };
    match decoded.len() {
        1024 => {
            let order = if name == "XFIR" {
                fire_coverage_order(chunks)
            } else {
                compact_map_order(chunks)
            };
            Ok(resize_map_32_to_128(&decoded, order))
        }
        // Do not accept 128×128 here: older port builds stored pollution in XPLC at full
        // tile resolution, and that must stay a pollution fallback instead of coverage.
        len => bail!("{name} decoded to {len} bytes, expected 1024"),
    }
}

fn fire_coverage_order(chunks: &[CityChunk]) -> MicrosimOrder {
    let canonical = chunks.iter().any(|chunk| {
        chunk.name == CITYGAME_FIRE_COVERAGE_CHUNK && chunk.data == CITYGAME_FIRE_COVERAGE_VERSION
    });
    let citygame = chunks
        .iter()
        .any(|chunk| chunk.name == CITYGAME_EXTENSION_CHUNK);
    if canonical || !citygame {
        MicrosimOrder::Native
    } else {
        MicrosimOrder::Legacy
    }
}

pub(super) fn decode_optional_microsim32_chunk(
    chunks: &[CityChunk],
    name: &str,
    kind: CityFileKind,
) -> Result<Option<Vec<u8>>> {
    decode_optional_chunk_layer(chunks, name, kind, |chunks| {
        decode_microsim32_chunk(chunks, name, kind)
    })
}

pub(super) fn decode_optional_chunk_layer<T>(
    chunks: &[CityChunk],
    name: &str,
    kind: CityFileKind,
    decode: impl FnOnce(&[CityChunk]) -> Result<T>,
) -> Result<Option<T>> {
    if !chunks.iter().any(|chunk| chunk.name == name) {
        return Ok(None);
    }
    if kind.is_win95() {
        return decode(chunks)
            .map(Some)
            .with_context(|| format!("loading {name} chunk"));
    }
    Ok(decode(chunks).ok())
}

pub(super) fn encode_microsim64_for_save(kind: CityFileKind, data: &[u8]) -> Vec<u8> {
    let compact = resize_map_128_to_64(data);
    if kind.is_win95() || resize_map_64_to_128(&compact, MicrosimOrder::Native) == data {
        compact
    } else {
        // Older CityGame Mac-layout files can carry genuinely per-tile data.
        data.to_vec()
    }
}

pub(super) fn decode_rotation_for_kind(chunks: &[CityChunk], kind: CityFileKind) -> Result<u8> {
    let data =
        decode_misc_bytes_for_kind(chunks, kind).ok_or_else(|| anyhow!("missing MISC chunk"))?;
    if data.len() < 12 {
        bail!("MISC length {} is too short for rotation", data.len());
    }

    Ok((u32::from_be_bytes(data[8..12].try_into().unwrap()) & 0x03) as u8)
}

pub(super) fn decode_misc_u32(chunks: &[CityChunk], offset: usize) -> Result<u32> {
    decode_misc_u32_for_kind(chunks, offset, CityFileKind::WinSc2)
}

pub(super) fn decode_misc_u32_for_kind(
    chunks: &[CityChunk],
    offset: usize,
    kind: CityFileKind,
) -> Result<u32> {
    const FIELD_LEN: usize = 4;
    let data =
        decode_misc_bytes_for_kind(chunks, kind).ok_or_else(|| anyhow!("missing MISC chunk"))?;
    if data.len() < offset + FIELD_LEN {
        bail!(
            "MISC length {} is too short for field at {offset:#06x}",
            data.len()
        );
    }

    Ok(u32::from_be_bytes(
        data[offset..offset + FIELD_LEN].try_into().unwrap(),
    ))
}

pub(super) fn decode_misc_bytes(chunks: &[CityChunk]) -> Option<Vec<u8>> {
    decode_misc_bytes_for_kind(chunks, CityFileKind::WinSc2)
}

pub(super) fn decode_misc_bytes_for_kind(
    chunks: &[CityChunk],
    kind: CityFileKind,
) -> Option<Vec<u8>> {
    let chunk = chunks.iter().find(|chunk| chunk.name == "MISC")?;
    let (first, first_encode, second, second_encode): (
        fn(&[u8]) -> Result<Vec<u8>>,
        fn(&[u8]) -> Vec<u8>,
        fn(&[u8]) -> Result<Vec<u8>>,
        fn(&[u8]) -> Vec<u8>,
    ) = if kind == CityFileKind::MacCity {
        (mac_rle_decode, mac_rle_encode, sc2_rle_decode, sc2_rle_encode)
    } else {
        (sc2_rle_decode, sc2_rle_encode, mac_rle_decode, mac_rle_encode)
    };
    let first = first(&chunk.data).ok();
    if first.as_ref().is_some_and(|decoded| {
        decoded.len() == WIN95_MISC_RECORD_SIZE || first_encode(decoded) == chunk.data
    }) {
        return first;
    }
    let second = second(&chunk.data).ok();
    if second.as_ref().is_some_and(|decoded| {
        decoded.len() == WIN95_MISC_RECORD_SIZE || second_encode(decoded) == chunk.data
    }) {
        return second;
    }
    // Short raw MISC fixtures can accidentally parse as a partial stream in
    // the other edition's codec. Accept a noncanonical decoded length only
    // when it round-trips through that codec's encoder; this also preserves
    // legitimate variable-length and CityGame-extended MISC records.
    Some(chunk.data.clone())
}

pub(super) fn decode_city_date(chunks: &[CityChunk], kind: CityFileKind) -> Result<CityDate> {
    if !chunks.iter().any(|chunk| chunk.name == "MISC") {
        bail!("missing MISC chunk");
    }
    let data =
        decode_misc_bytes_for_kind(chunks, kind).ok_or_else(|| anyhow!("missing MISC chunk"))?;
    if data.len() < MISC_CITY_AGE_OFFSET + MISC_FIELD_LEN {
        bail!("MISC length {} is too short for city date", data.len());
    }

    let founded_raw = u32::from_be_bytes(
        data[MISC_YEAR_FOUNDED_OFFSET..MISC_YEAR_FOUNDED_OFFSET + MISC_FIELD_LEN]
            .try_into()
            .unwrap(),
    );
    let founded = u32::from(misc_year_founded_word(founded_raw, kind));
    let stored_time = u32::from_be_bytes(
        data[MISC_CITY_AGE_OFFSET..MISC_CITY_AGE_OFFSET + MISC_FIELD_LEN]
            .try_into()
            .unwrap(),
    );
    let (elapsed_years, month) = if kind == CityFileKind::MacCity {
        (
            stored_time / MAC_CALENDAR_PHASES_PER_YEAR,
            ((stored_time % MAC_CALENDAR_PHASES_PER_YEAR) / MAC_CALENDAR_PHASES_PER_MONTH + 1)
                as u8,
        )
    } else {
        (
            stored_time / DAYS_PER_YEAR,
            ((stored_time % DAYS_PER_YEAR) / DAYS_PER_MONTH + 1) as u8,
        )
    };
    let year = founded
        .checked_add(elapsed_years)
        .ok_or_else(|| anyhow!("city date overflow"))?;
    let year = u16::try_from(year).with_context(|| format!("city year {year} out of range"))?;

    Ok(CityDate { year, month })
}

pub(super) fn decode_city_view(chunks: &[CityChunk], kind: CityFileKind) -> Result<CityView> {
    if !kind.is_win95() {
        bail!("saved view layout is only established for Win95 city files");
    }
    const FIELD_LEN: usize = 4;

    let chunk = chunks
        .iter()
        .find(|chunk| chunk.name == "MISC")
        .ok_or_else(|| anyhow!("missing MISC chunk"))?;
    let data = city_rle_decode(&chunk.data, kind)
        .or_else(|_| Ok::<_, anyhow::Error>(chunk.data.clone()))?;
    if data.len() < MISC_SAVED_VIEW_MODE_OFFSET + FIELD_LEN {
        bail!("MISC length {} is too short for city view", data.len());
    }

    let packed_coordinates = u32::from_be_bytes(
        data[MISC_SAVED_VIEW_TARGET_OFFSET..MISC_SAVED_VIEW_TARGET_OFFSET + FIELD_LEN]
            .try_into()
            .unwrap(),
    ) as u16;
    let mode = u32::from_be_bytes(
        data[MISC_SAVED_VIEW_MODE_OFFSET..MISC_SAVED_VIEW_MODE_OFFSET + FIELD_LEN]
            .try_into()
            .unwrap(),
    );
    let (x, y) = if packed_coordinates == u16::MAX {
        (64, 128)
    } else {
        (
            packed_coordinates & 0x7f,
            ((packed_coordinates as i16) >> 8) as u16,
        )
    };

    Ok(CityView { mode, x, y })
}

pub(super) fn encode_altm(samples: &[AltmSample]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let word = (sample.land as u16 & 0x1f)
            | ((sample.water as u16 & 0x1f) << 5)
            | ((sample.tunnel_levels as u16 & 0x3f) << 10);
        raw.extend_from_slice(&word.to_be_bytes());
    }
    raw
}

pub(super) fn decode_cnam_chunk(chunk: &CityChunk) -> String {
    cnam_text(&chunk.data)
}

pub(super) fn cnam_text(data: &[u8]) -> String {
    // PE `FUN_004302E0` reads the variable-length CNAM payload, appends a NUL, then
    // passes `buffer + 1` to `FUN_004A2E6A`, which measures it with `strlen`. The first
    // byte is metadata; the text ends at the first NUL (or the chunk boundary), not
    // at the value stored in that byte.
    let text = data.get(1..).unwrap_or_default();
    let text = &text[..text
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(text.len())];
    String::from_utf8_lossy(text).into_owned()
}
