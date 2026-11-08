//! Native importer for the Windows 95 / Special Edition asset layout.
//!
//! This keeps the common Wine-installed edition self-contained: DAT tile
//! decoding, BMP/PCX conversion, palette extraction, and catalog copying all
//! run in the game process without Python or Pillow.

use anyhow::{Context, Result, ensure};
use image::{GenericImage, ImageReader, Rgba, RgbaImage};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const ATLAS_WIDTH: u32 = 4096;
const ATLAS_PADDING: u32 = 2;

#[derive(Debug, Serialize)]
struct AtlasManifest {
    image: String,
    frames: Vec<AtlasFrame>,
}

#[derive(Debug, Serialize)]
struct AtlasFrame {
    file: String,
    id: u32,
    source_offset: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Debug)]
struct TileRecord {
    id: u16,
    offset: usize,
    width: u16,
    height: u16,
}

#[derive(Debug)]
struct DecodedTile {
    image: RgbaImage,
    id: u16,
    offset: usize,
}

/// Extract a Win95/Special Edition installation to the format consumed by the
/// game's asset and catalog loaders.
pub fn import(source: &Path, output: &Path) -> Result<()> {
    ensure!(!output.exists(), "{} already exists", output.display());
    let source = resolve_install_root(source);
    ensure!(source.is_dir(), "{} must be a folder", source.display());
    fs::create_dir_all(output).with_context(|| format!("creating {}", output.display()))?;

    convert_bmps(&source, output);
    convert_pcx_files(&source, output);
    extract_tile_sets(&source, output)?;
    // PAL_MSTR repeats the same RGB values across the animation slots. Keep a separate
    // lossless traffic atlas before converting those indices into ordinary display colors.
    write_atlas(
        &extract_traffic_tiles(&source)?,
        &output.join("traffic_masks"),
    )?;
    copy_catalog(&source, output)?;

    Ok(())
}

fn resolve_install_root(source: &Path) -> PathBuf {
    find_case_insensitive(source, &["SC2K"])
        .filter(|path| path.is_dir() && find_case_insensitive(path, &["DATA"]).is_some())
        .unwrap_or_else(|| source.to_path_buf())
}

fn find_case_insensitive(root: &Path, components: &[&str]) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for component in components {
        let entries = fs::read_dir(&current).ok()?;
        let mut found = None;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy();
            if name.eq_ignore_ascii_case(component) {
                found = Some(path);
                break;
            }
        }
        current = found?;
    }
    Some(current)
}

fn files_with_suffix(root: &Path, suffix: &str, recursive: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort_by_key(|path| path.to_string_lossy().to_lowercase());
        for path in entries {
            if path.is_dir() && recursive {
                pending.push(path);
            } else if path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case(suffix))
            {
                files.push(path);
            }
        }
    }
    files.sort_by_key(|path| path.to_string_lossy().to_lowercase());
    files
}

fn convert_bmps(source: &Path, output: &Path) {
    let Some(bitmap_dir) = find_case_insensitive(source, &["BITMAPS"]) else {
        return;
    };
    for path in files_with_suffix(&bitmap_dir, ".BMP", false) {
        let Ok(reader) = ImageReader::open(&path) else {
            log::warn!("skipping unreadable Win95 bitmap {}", path.display());
            continue;
        };
        let Ok(image) = reader.decode() else {
            log::warn!("skipping unreadable Win95 bitmap {}", path.display());
            continue;
        };
        let Some(stem) = path.file_stem() else {
            continue;
        };
        let dest = output
            .join("bitmaps_png")
            .join(stem.to_string_lossy().to_string() + ".png");
        if let Err(error) = save_image(&dest, &image.to_rgba8()) {
            log::warn!("could not convert {}: {error:#}", path.display());
        }
    }
}

fn convert_pcx_files(source: &Path, output: &Path) {
    for path in files_with_suffix(source, ".PCX", true) {
        let rel = match path.strip_prefix(source) {
            Ok(rel) => rel,
            Err(_) => continue,
        };
        let dest = output.join("pcx_png").join(rel).with_extension("png");
        let decoded = fs::read(&path)
            .with_context(|| format!("reading {}", path.display()))
            .and_then(|bytes| decode_pcx(&bytes));
        match decoded {
            Ok(image) => {
                if let Err(error) = save_image(&dest, &image) {
                    log::warn!(
                        "could not save PCX conversion {}: {error:#}",
                        path.display()
                    );
                }
            }
            Err(error) => log::warn!("skipping unreadable PCX {}: {error:#}", path.display()),
        }
    }
}

fn save_image(path: &Path, image: &RgbaImage) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    image
        .save(path)
        .with_context(|| format!("writing {}", path.display()))
}

fn extract_tile_sets(source: &Path, output: &Path) -> Result<()> {
    let palette_path = find_case_insensitive(source, &["BITMAPS", "PAL_MSTR.BMP"])
        .context("could not find BITMAPS/PAL_MSTR.BMP")?;
    let palette_bitmap = ImageReader::open(&palette_path)
        .with_context(|| format!("opening {}", palette_path.display()))?
        .decode()
        .with_context(|| format!("decoding {}", palette_path.display()))?
        .to_rgba8();
    let palette = master_palette(&palette_bitmap);
    save_palette_image(&palette, &output.join("palettes/win95_pal_mstr.png"))?;

    let data_dir =
        find_case_insensitive(source, &["DATA"]).context("could not find DATA folder")?;
    for filename in ["LARGE.DAT", "SMALLMED.DAT", "SPECIAL.DAT"] {
        let set_name = Path::new(filename)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_uppercase();
        let Some(path) = find_case_insensitive(&data_dir, &[filename]) else {
            write_atlas(&[], &output.join("win95_tiles").join(set_name))?;
            continue;
        };
        if !path.is_file() {
            write_atlas(&[], &output.join("win95_tiles").join(set_name))?;
            continue;
        }
        let records = decode_dat(
            &fs::read(&path).with_context(|| format!("reading {}", path.display()))?,
            &palette,
        )?;
        write_atlas(&records, &output.join("win95_tiles").join(set_name))?;
    }
    Ok(())
}

fn master_palette(image: &RgbaImage) -> [[u8; 3]; 256] {
    let (width, height) = image.dimensions();
    let mut palette = [[0u8; 3]; 256];
    for y in 0..16 {
        for x in 0..16 {
            let px = (1 + (x + 1) * 6).min(width.saturating_sub(1));
            let py = (15 + (y + 1) * 5).min(height.saturating_sub(1));
            let color = image.get_pixel(px, py).0;
            palette[(y * 16 + x) as usize] = [color[0], color[1], color[2]];
        }
    }
    palette
}

fn save_palette_image(palette: &[[u8; 3]; 256], path: &Path) -> Result<()> {
    let cell = 18;
    let mut image = RgbaImage::new(16 * cell, 16 * cell);
    for (index, color) in palette.iter().enumerate() {
        let x0 = (index as u32 % 16) * cell;
        let y0 = (index as u32 / 16) * cell;
        for y in y0..y0 + cell {
            for x in x0..x0 + cell {
                image.put_pixel(x, y, Rgba([color[0], color[1], color[2], 255]));
            }
        }
    }
    save_image(path, &image)
}

fn decode_dat(data: &[u8], palette: &[[u8; 3]; 256]) -> Result<Vec<DecodedTile>> {
    decode_dat_filtered(data, palette, false)
}

fn decode_dat_filtered(
    data: &[u8],
    palette: &[[u8; 3]; 256],
    traffic_only: bool,
) -> Result<Vec<DecodedTile>> {
    if data.len() < 2 {
        return Ok(Vec::new());
    }
    let count = u16::from_be_bytes([data[0], data[1]]) as usize;
    let table_end = 2usize
        .checked_add(count.checked_mul(10).context("DAT record count overflow")?)
        .context("DAT table size overflow")?;
    if count == 0 || table_end > data.len() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for index in 0..count {
        let offset = 2 + index * 10;
        let id = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let start = u32::from_be_bytes(data[offset + 2..offset + 6].try_into().unwrap()) as usize;
        let height = u16::from_be_bytes([data[offset + 6], data[offset + 7]]);
        let width = u16::from_be_bytes([data[offset + 8], data[offset + 9]]);
        if start < table_end || start > data.len() || width == 0 || height == 0 {
            continue;
        }
        records.push(TileRecord {
            id,
            offset: start,
            width,
            height,
        });
    }

    let mut decoded = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        if traffic_only && !crate::palette::is_traffic_sprite(u32::from(record.id)) {
            continue;
        }
        let end = records
            .get(index + 1)
            .map_or(data.len(), |next| next.offset);
        if end < record.offset {
            continue;
        }
        let image = decode_win95_image(
            &data[record.offset..end],
            u32::from(record.width),
            u32::from(record.height),
            palette,
        );
        decoded.push(DecodedTile {
            image,
            id: record.id,
            offset: record.offset,
        });
    }
    Ok(decoded)
}

fn extract_traffic_tiles(source: &Path) -> Result<Vec<DecodedTile>> {
    let source = resolve_install_root(source);
    let data_dir =
        find_case_insensitive(&source, &["DATA"]).context("could not find DATA folder")?;
    // Non-traffic pixels (anchors and fixed road details) are deliberately discarded by the
    // traffic baker. Only the original 171..194 cycling indices carry animated lane marks.
    let palette = std::array::from_fn(|index| {
        crate::palette::traffic_mask_color(index as u8).unwrap_or([115, 83, 35])
    });
    let mut tiles = Vec::new();
    for filename in ["LARGE.DAT", "SPECIAL.DAT", "SMALLMED.DAT"] {
        let Some(path) = find_case_insensitive(&data_dir, &[filename]).filter(|p| p.is_file())
        else {
            continue;
        };
        let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        tiles.extend(decode_dat_filtered(&data, &palette, true)?);
    }
    Ok(tiles)
}

/// Load lossless masks independently of ordinary graphics and replacement packs.
/// Older script imports can be repaired from their recorded installation, when available.
pub(crate) fn load_traffic_mask_source(root: &Path) -> Result<Option<crate::assets::AtlasSource>> {
    let masks = root.join("traffic_masks");
    if masks.join("atlas.json").is_file() {
        return crate::assets::AtlasSource::load(masks).map(Some);
    }
    let Some(source) = fs::read(root.join("manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|manifest| manifest.get("source_root")?.as_str().map(PathBuf::from))
        .filter(|source| source.is_dir())
    else {
        return Ok(None);
    };
    let tiles = extract_traffic_tiles(&source)?;
    if !tiles.is_empty() {
        // Keep repaired packs self-contained when they are copied to another computer.
        if let Err(error) = write_atlas(&tiles, &masks) {
            log::warn!("could not cache restored traffic masks: {error:#}");
        }
    }
    let source = traffic_mask_source_from_tiles(&tiles)?;
    log::info!(
        "restored indexed Win95 traffic masks for {}",
        root.display()
    );
    Ok(source)
}

pub(crate) fn traffic_mask_source_from_install(
    source: &Path,
) -> Result<Option<crate::assets::AtlasSource>> {
    let tiles = extract_traffic_tiles(source)?;
    traffic_mask_source_from_tiles(&tiles)
}

fn traffic_mask_source_from_tiles(
    tiles: &[DecodedTile],
) -> Result<Option<crate::assets::AtlasSource>> {
    if tiles.is_empty() {
        return Ok(None);
    }
    let (image, manifest) = pack_atlas(tiles)?;
    let frames = manifest
        .frames
        .into_iter()
        .enumerate()
        .map(|(index, frame)| crate::assets::AtlasFrame {
            index: index as u32,
            id: frame.id,
            file: frame.file,
            source_offset: frame.source_offset,
            x: frame.x,
            y: frame.y,
            w: frame.w,
            h: frame.h,
            logical_size: None,
            anchor: None,
        })
        .collect();
    Ok(Some(crate::assets::AtlasSource::from_rgba(
        "win95_traffic_masks",
        image,
        frames,
    )))
}

fn decode_win95_image(data: &[u8], width: u32, height: u32, palette: &[[u8; 3]; 256]) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let mut pos = 0usize;
    let mut y = -1i64;
    while pos + 2 <= data.len() {
        let row_length = data[pos] as usize;
        let more = data[pos + 1];
        pos += 2;
        y += 1;
        let row_end = pos.saturating_add(row_length).min(data.len());
        let row = &data[pos..row_end];
        pos = row_end;
        let mut x = 0i64;
        let mut chunk = 0usize;
        while chunk < row.len() {
            if row[chunk] == 0 && chunk > 0 {
                chunk += 1;
                continue;
            }
            if chunk + 1 >= row.len() {
                break;
            }
            let mode = row[chunk + 1];
            let (start, length, x_skip) = match mode {
                0x00 | 0x03 => {
                    if chunk + 4 > row.len() {
                        break;
                    }
                    let length = row[chunk + 2] as usize;
                    let extra = row[chunk + 3];
                    let (length, header) = if length == 0 && extra == 0 {
                        if chunk + 6 > row.len() {
                            break;
                        }
                        (row[chunk + 4] as usize, 6)
                    } else {
                        (length, 4)
                    };
                    (chunk + header, length, row[chunk] as i64)
                }
                0x04 => (chunk + 2, row[chunk] as usize, 0),
                _ => break,
            };
            x += x_skip;
            let Some(end) = start.checked_add(length) else {
                break;
            };
            if end > row.len() {
                break;
            }
            for value in &row[start..end] {
                if x >= 0 && y >= 0 && x < i64::from(width) && y < i64::from(height) {
                    let color = palette[*value as usize];
                    image.put_pixel(
                        x as u32,
                        y as u32,
                        Rgba([color[0], color[1], color[2], 255]),
                    );
                }
                x += 1;
            }
            chunk = end;
        }
        if more == 2 {
            break;
        }
    }
    image
}

fn write_atlas(tiles: &[DecodedTile], directory: &Path) -> Result<()> {
    fs::create_dir_all(directory).with_context(|| format!("creating {}", directory.display()))?;
    let (atlas, manifest) = pack_atlas(tiles)?;
    save_image(&directory.join("atlas.png"), &atlas)?;
    fs::write(
        directory.join("atlas.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .with_context(|| format!("writing atlas manifest in {}", directory.display()))?;
    Ok(())
}

fn pack_atlas(tiles: &[DecodedTile]) -> Result<(RgbaImage, AtlasManifest)> {
    let mut placements = Vec::with_capacity(tiles.len());
    let (mut x, mut y, mut row_height) = (ATLAS_PADDING, ATLAS_PADDING, 0u32);
    for tile in tiles {
        let (width, height) = tile.image.dimensions();
        ensure!(
            width + 2 * ATLAS_PADDING <= ATLAS_WIDTH,
            "tile {} is wider than the Win95 atlas",
            tile.id
        );
        if x + width + ATLAS_PADDING > ATLAS_WIDTH {
            x = ATLAS_PADDING;
            y = y
                .checked_add(row_height + ATLAS_PADDING)
                .context("Win95 atlas height overflow")?;
            row_height = 0;
        }
        placements.push((x, y));
        x += width + ATLAS_PADDING;
        row_height = row_height.max(height);
    }
    let atlas_height = y
        .checked_add(row_height + ATLAS_PADDING)
        .context("Win95 atlas height overflow")?;
    let atlas_width = if tiles.is_empty() { 1 } else { ATLAS_WIDTH };
    let mut atlas = RgbaImage::new(atlas_width, atlas_height.max(1));
    let mut frames = Vec::with_capacity(tiles.len());
    for (tile, (x, y)) in tiles.iter().zip(placements) {
        let (w, h) = tile.image.dimensions();
        atlas
            .copy_from(&tile.image, x, y)
            .context("placing tile in Win95 atlas")?;
        frames.push(AtlasFrame {
            file: format!("tile_id_{:05}_{}x{}.png", tile.id, w, h),
            id: u32::from(tile.id),
            source_offset: tile.offset as u32,
            x,
            y,
            w,
            h,
        });
    }
    let manifest = AtlasManifest {
        image: "atlas.png".into(),
        frames,
    };
    Ok((atlas, manifest))
}

fn copy_catalog(source: &Path, output: &Path) -> Result<()> {
    for (source_name, dest_name, extensions) in [
        ("CITIES", "cities", &["SC2"][..]),
        ("SCENARIO", "scenarios", &["SCN"][..]),
        ("SOUNDS", "sounds", &["WAV", "MID"][..]),
        ("SCURKART", "scurk_mif", &["MIF"][..]),
    ] {
        let Some(source_dir) = find_case_insensitive(source, &[source_name]) else {
            continue;
        };
        if !source_dir.is_dir() {
            continue;
        }
        let dest_dir = output.join("catalog").join(dest_name);
        for extension in extensions {
            for path in files_with_suffix(&source_dir, extension, false) {
                fs::create_dir_all(&dest_dir)
                    .with_context(|| format!("creating {}", dest_dir.display()))?;
                let stem = path.file_stem().context("catalog file has no name")?;
                let name = format!("{}.{}", stem.to_string_lossy(), extension);
                fs::copy(&path, dest_dir.join(name))
                    .with_context(|| format!("copying {}", path.display()))?;
            }
        }
    }
    if let Some(default_city) =
        find_case_insensitive(source, &["DEFAULT.SC2"]).filter(|path| path.is_file())
    {
        let dest_dir = output.join("catalog/cities");
        fs::create_dir_all(&dest_dir)
            .with_context(|| format!("creating {}", dest_dir.display()))?;
        fs::copy(
            &default_city,
            dest_dir.join(default_city.file_name().unwrap()),
        )
        .with_context(|| format!("copying {}", default_city.display()))?;
    }
    Ok(())
}

pub(crate) fn decode_pcx(data: &[u8]) -> Result<RgbaImage> {
    ensure!(data.len() >= 128, "PCX header is truncated");
    ensure!(data[0] == 0x0a, "not a ZSoft PCX image");
    ensure!(data[2] == 1, "unsupported PCX encoding {}", data[2]);
    let bits_per_pixel = data[3];
    let xmin = u16::from_le_bytes([data[4], data[5]]) as u32;
    let ymin = u16::from_le_bytes([data[6], data[7]]) as u32;
    let xmax = u16::from_le_bytes([data[8], data[9]]) as u32;
    let ymax = u16::from_le_bytes([data[10], data[11]]) as u32;
    let width = xmax
        .checked_sub(xmin)
        .and_then(|n| n.checked_add(1))
        .context("invalid PCX width")?;
    let height = ymax
        .checked_sub(ymin)
        .and_then(|n| n.checked_add(1))
        .context("invalid PCX height")?;
    ensure!(
        width > 0 && height > 0 && width <= 16_384 && height <= 16_384,
        "unsupported PCX dimensions {width}x{height}"
    );
    let planes = data[65] as usize;
    let bytes_per_line = u16::from_le_bytes([data[66], data[67]]) as usize;
    ensure!(
        planes > 0 && planes <= 4 && bytes_per_line > 0,
        "invalid PCX plane layout"
    );
    ensure!(
        matches!(bits_per_pixel, 1 | 2 | 4 | 8),
        "unsupported PCX bits per pixel: {bits_per_pixel}"
    );
    let scanline_bytes = bytes_per_line
        .checked_mul(planes)
        .context("PCX scanline size overflow")?;
    let decoded_len = scanline_bytes
        .checked_mul(height as usize)
        .context("PCX image size overflow")?;
    ensure!(
        decoded_len <= 256 * 1024 * 1024,
        "PCX image is too large to import safely"
    );
    let mut decoded = Vec::with_capacity(decoded_len);
    let mut pos = 128usize;
    while decoded.len() < decoded_len {
        ensure!(pos < data.len(), "PCX image data is truncated");
        let value = data[pos];
        pos += 1;
        if value & 0xc0 == 0xc0 {
            ensure!(pos < data.len(), "PCX run is truncated");
            let count = (value & 0x3f) as usize;
            let pixel = data[pos];
            pos += 1;
            ensure!(count > 0, "invalid zero-length PCX run");
            decoded.extend(std::iter::repeat(pixel).take(count.min(decoded_len - decoded.len())));
        } else {
            decoded.push(value);
        }
    }

    let palette_256 = data.len() >= 769 && data[data.len() - 769] == 0x0c;
    let mut palette16 = [[0u8; 3]; 16];
    for (index, color) in palette16.iter_mut().enumerate() {
        let offset = 16 + index * 3;
        *color = [data[offset], data[offset + 1], data[offset + 2]];
    }
    let mut image = RgbaImage::new(width, height);
    let row_bytes = bytes_per_line * planes;
    for y in 0..height as usize {
        for x in 0..width as usize {
            let color = if bits_per_pixel == 8 && planes >= 3 {
                let row = &decoded[y * row_bytes..(y + 1) * row_bytes];
                [
                    row[x],
                    row[bytes_per_line + x],
                    row[2 * bytes_per_line + x],
                    255,
                ]
            } else {
                let index = pcx_palette_index(
                    &decoded[y * row_bytes..(y + 1) * row_bytes],
                    x,
                    bits_per_pixel,
                    planes,
                );
                if bits_per_pixel == 8 && palette_256 {
                    let offset = data.len() - 768 + index * 3;
                    [data[offset], data[offset + 1], data[offset + 2], 255]
                } else {
                    let color = palette16.get(index).copied().unwrap_or([0, 0, 0]);
                    [color[0], color[1], color[2], 255]
                }
            };
            image.put_pixel(x as u32, y as u32, Rgba(color));
        }
    }
    Ok(image)
}

fn pcx_palette_index(row: &[u8], x: usize, bits_per_pixel: u8, planes: usize) -> usize {
    if bits_per_pixel == 8 {
        return row.get(x).copied().unwrap_or(0) as usize;
    }
    let mut index = 0usize;
    let pixels_per_byte = 8 / bits_per_pixel as usize;
    for plane in 0..planes {
        let byte = row
            .get(plane * row.len().div_ceil(planes) + x / pixels_per_byte)
            .copied()
            .unwrap_or(0);
        let shift = 8 - bits_per_pixel as usize * ((x % pixels_per_byte) + 1);
        let mask = (1usize << bits_per_pixel) - 1;
        index |= ((byte as usize >> shift) & mask) << (plane * bits_per_pixel as usize);
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_traffic_retains_indices_that_share_display_colors() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch =
            std::env::temp_dir().join(format!("win95-traffic-{}-{unique}", std::process::id()));
        let source = scratch.join("source");
        let output = scratch.join("imported");
        fs::create_dir_all(source.join("Bitmaps")).unwrap();
        fs::create_dir_all(source.join("Data")).unwrap();
        // Deliberately identical display colors: RGB export alone must not decide phases.
        RgbaImage::from_pixel(100, 100, Rgba([140, 140, 140, 255]))
            .save_with_format(source.join("Bitmaps/PAL_MSTR.BMP"), image::ImageFormat::Bmp)
            .unwrap();
        let mut dat = vec![0, 1, 0x05, 0x78, 0, 0, 0, 12, 0, 1, 0, 8]; // 1400, 8x1
        dat.extend_from_slice(&[12, 2, 0, 0, 8, 0, 171, 172, 173, 174, 175, 176, 177, 178]);
        fs::write(source.join("Data/LARGE.DAT"), &dat).unwrap();
        import(&source, &output).unwrap();
        let palette = crate::palette::AnimatedPalette::load(&output).unwrap();
        for legacy in [false, true] {
            if legacy {
                // Old imports can recover the original indexed bytes, without Python or
                // guessing phase numbers from their flattened RGB atlas.
                fs::remove_dir_all(output.join("traffic_masks")).unwrap();
                fs::write(
                    output.join("manifest.json"),
                    serde_json::to_vec(&serde_json::json!({
                        "source_root": source,
                    }))
                    .unwrap(),
                )
                .unwrap();
            }
            let masks = load_traffic_mask_source(&output).unwrap().unwrap();
            assert!(output.join("traffic_masks/atlas.json").is_file());
            let frame = masks.frame_by_id(1400).unwrap();
            assert_eq!((frame.w, frame.h), (8, 1));
            let atlas = masks.load_rgba().unwrap();
            let mask =
                image::imageops::crop_imm(&atlas, frame.x, frame.y, frame.w, frame.h).to_image();
            for phase in 0..8 {
                let mut image = mask.clone();
                palette.bake_traffic_overlay_sprite_frame(&mut image, 81, phase);
                let visible: Vec<u32> = (0..8).filter(|&x| image.get_pixel(x, 0)[3] != 0).collect();
                let mut expected = vec![u32::from(phase), u32::from((phase + 1) % 8)];
                expected.sort_unstable();
                assert_eq!(visible, expected, "legacy={legacy}, phase={phase}");
            }
        }
        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn decodes_a_small_win95_dat_literal_and_transparency() {
        let mut palette = [[0u8; 3]; 256];
        palette[7] = [20, 80, 160];
        // One 2x1 image; row chunk is [x skip, mode, literal length, extra, pixels].
        let mut dat = vec![0, 1, 0, 9, 0, 0, 0, 12, 0, 1, 0, 2];
        dat.extend_from_slice(&[6, 2, 0, 0, 2, 0, 7, 7]);
        let tiles = decode_dat(&dat, &palette).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].image.get_pixel(0, 0).0, [20, 80, 160, 255]);
        assert_eq!(tiles[0].image.get_pixel(1, 0).0, [20, 80, 160, 255]);
    }

    #[test]
    fn decodes_simple_indexed_pcx() {
        let mut pcx = vec![0u8; 128];
        pcx[0] = 0x0a;
        pcx[1] = 5;
        pcx[2] = 1;
        pcx[3] = 8;
        pcx[8..10].copy_from_slice(&1u16.to_le_bytes());
        pcx[10..12].copy_from_slice(&0u16.to_le_bytes());
        pcx[65] = 1;
        pcx[66..68].copy_from_slice(&2u16.to_le_bytes());
        pcx.extend_from_slice(&[1, 2]);
        pcx.push(0x0c);
        for index in 0..256u16 {
            pcx.extend_from_slice(&[index as u8, 0, 0]);
        }
        let image = decode_pcx(&pcx).unwrap();
        assert_eq!(image.dimensions(), (2, 1));
        assert_eq!(image.get_pixel(0, 0).0, [1, 0, 0, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [2, 0, 0, 255]);
    }

    #[test]
    fn imports_mixed_case_install_into_a_loadable_pack() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch =
            std::env::temp_dir().join(format!("sc2k-win95-import-{}-{unique}", std::process::id()));
        let source = scratch.join("source");
        let output = scratch.join("output");
        fs::create_dir_all(source.join("Bitmaps")).unwrap();
        fs::create_dir_all(source.join("Data")).unwrap();
        fs::create_dir_all(source.join("Sounds")).unwrap();

        let palette_bitmap = RgbaImage::from_pixel(100, 100, Rgba([32, 64, 96, 255]));
        palette_bitmap
            .save_with_format(source.join("Bitmaps/pal_mstr.bmp"), image::ImageFormat::Bmp)
            .unwrap();

        let mut dat = vec![0, 1, 0x04, 0xbb, 0, 0, 0, 12, 0, 1, 0, 2];
        dat.extend_from_slice(&[6, 2, 0, 0, 2, 0, 7, 7]);
        fs::write(source.join("Data/large.dat"), dat).unwrap();
        fs::write(source.join("Sounds/10018.mid"), b"MIDI fixture").unwrap();

        import(&source, &output).unwrap();
        crate::asset_profile::validate_original_pack(&output).unwrap();
        assert!(output.join("win95_tiles/LARGE/atlas.json").is_file());
        assert!(output.join("win95_tiles/SMALLMED/atlas.json").is_file());
        assert!(output.join("win95_tiles/SPECIAL/atlas.json").is_file());
        assert!(output.join("catalog/sounds/10018.MID").is_file());
        crate::asset_catalog::Catalog::load(std::slice::from_ref(&output)).unwrap();

        fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    #[ignore = "manual verification; set SC2K_WIN95_TEST_SOURCE to a Win95/Special Edition install"]
    fn imports_a_real_win95_install() {
        let source = PathBuf::from(
            std::env::var_os("SC2K_WIN95_TEST_SOURCE")
                .expect("set SC2K_WIN95_TEST_SOURCE to the game folder"),
        );
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output = std::env::temp_dir().join(format!(
            "sc2k-win95-real-import-{}-{unique}",
            std::process::id()
        ));
        import(&source, &output).unwrap();
        crate::asset_profile::validate_original_pack(&output).unwrap();
        let catalog = crate::asset_catalog::Catalog::load(std::slice::from_ref(&output)).unwrap();
        assert!(
            catalog.assets.len() > 1000,
            "imported only {} assets",
            catalog.assets.len()
        );
        let bitmap_source = find_case_insensitive(&source, &["BITMAPS"]).unwrap();
        assert_eq!(
            files_with_suffix(&bitmap_source, ".BMP", false).len(),
            files_with_suffix(&output.join("bitmaps_png"), ".PNG", true).len(),
            "some Win95 bitmaps were not converted"
        );
        assert_eq!(
            files_with_suffix(&source, ".PCX", true).len(),
            files_with_suffix(&output.join("pcx_png"), ".PNG", true).len(),
            "some Win95 PCX images were not converted"
        );
        fs::remove_dir_all(output).unwrap();
    }
}
