//! Native importer for classic 68k Macintosh resource exports.

use crate::audio_import;
use anyhow::{Context, Result, ensure};
use image::{GenericImage, Rgba, RgbaImage};
use oxideav_pict::decode_rgba8;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const ATLAS_WIDTH: u32 = 4096;
const ATLAS_PADDING: u32 = 2;

#[derive(Serialize)]
struct AtlasManifest {
    image: String,
    frames: Vec<AtlasFrame>,
}

#[derive(Serialize)]
struct AtlasFrame {
    file: String,
    index: u32,
    id: u32,
    source_offset: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

struct Sprite {
    index: u32,
    id: u32,
    offset: u32,
    image: RgbaImage,
}

pub fn import(source: &Path, output: &Path) -> Result<()> {
    ensure!(!output.exists(), "{} already exists", output.display());
    ensure!(source.is_dir(), "{} must be a folder", source.display());
    fs::create_dir_all(output).with_context(|| format!("creating {}", output.display()))?;

    let (palette, system16) = convert_palettes(source, output)?;
    convert_picts(source, output);
    convert_icons(source, output, &palette, &system16)?;
    convert_cursors(source, output)?;
    import_newspaper_data(source, output)?;
    import_newspaper_strings(source, output)?;
    import_newspaper_picture_index(source, output)?;
    import_newspaper_font(source, output)?;
    import_system_chicago_font(source, output)?;
    import_terrain_shape_map(source, output)?;
    let sprite_count = convert_sprites(source, output, &palette)?;
    ensure!(
        sprite_count > 0,
        "Macintosh resources contain no usable SPRT sprite records"
    );
    let (effects, music) = audio_import::import_mac(source, output)?;
    log::info!("normalized Macintosh media: {effects} effects, {music} music cues");
    Ok(())
}

// CODE 0 describes the below-A5 data image; DATA and ZERO encode that image
// as words interspersed with zero runs. This offset is established for the
// matching Mac 1.2 build, so do not silently apply it to other layouts.
pub(crate) fn decode_terrain_shape_map(code: &[u8], data: &[u8], zero: &[u8]) -> Result<Vec<u16>> {
    ensure!(code.len() >= 8, "short CODE 0 resource");
    let size = u32::from_be_bytes(code[4..8].try_into()?) as usize;
    ensure!(size == 0x7394, "unsupported Mac below-A5 layout");
    ensure!(data.len() % 2 == 0, "odd DATA resource length");
    let mut image = Vec::with_capacity(size);
    let mut runs = zero.chunks_exact(2);
    for word in data.chunks_exact(2) {
        ensure!(image.len() + 2 <= size, "DATA exceeds below-A5 image");
        image.extend_from_slice(word);
        if word == [0, 0] {
            let count = runs.next().context("ZERO stream ended early")?;
            let count = u16::from_be_bytes([count[0], count[1]]) as usize;
            ensure!(image.len() + count <= size, "ZERO exceeds below-A5 image");
            image.resize(image.len() + count, 0);
        }
    }
    ensure!(runs.next().is_none() && runs.remainder().is_empty() && image.len() == size, "DATA/ZERO image length mismatch");
    let start = size - 0x5082;
    Ok(image[start..start + 256].chunks_exact(2)
        .map(|word| u16::from_be_bytes([word[0], word[1]])).collect())
}

pub(crate) fn terrain_shape_map_from_resources(source: &Path) -> Result<Option<Vec<u16>>> {
    let mut resources = Vec::new();
    for group in ["CODE", "DATA", "ZERO"] {
        let Some(dir) = find_dir(source, group) else { return Ok(None); };
        let expected = format!("{group}_00000.bin");
        let Some(path) = files_with_extension(&dir, "bin").into_iter().find(|path| {
            path.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(&expected))
        }) else { return Ok(None); };
        resources.push(fs::read(path)?);
    }
    Ok(Some(decode_terrain_shape_map(&resources[0], &resources[1], &resources[2])?))
}

fn import_terrain_shape_map(source: &Path, output: &Path) -> Result<()> {
    match terrain_shape_map_from_resources(source) {
        Ok(Some(map)) => {
            let path = output.join("catalog/mac-terrain-shapes.json");
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, serde_json::to_vec(&map)?)?;
        }
        Ok(None) => {}
        Err(error) => log::warn!("skipping unsupported Macintosh terrain shape map: {error:#}"),
    }
    Ok(())
}

fn import_newspaper_picture_index(source: &Path, output: &Path) -> Result<()> {
    let Some(dir) = find_dir(source, "PICT") else { return Ok(()); };
    let files = files_with_extension(&dir, "bin");
    let mut present = [0u8; 12];
    for (index, value) in present.iter_mut().enumerate() {
        let prefix = format!("PICT_{:05}", 400 + index);
        *value = u8::from(files.iter().any(|path| path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name.get(..prefix.len()).is_some_and(|part| part.eq_ignore_ascii_case(&prefix))
                && name.as_bytes().get(prefix.len()).is_some_and(|b| *b == b'_' || *b == b'.')
        })));
    }
    let dest = output.join("catalog/mac-newspaper");
    fs::create_dir_all(&dest)?;
    fs::write(dest.join("pictures.bin"), present)?;
    Ok(())
}

fn import_newspaper_strings(source: &Path, output: &Path) -> Result<()> {
    let Some(dir) = find_dir(source, "STRS") else { return Ok(()); };
    let Some(path) = files_with_extension(&dir, "bin").into_iter().find(|path| {
        path.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("STRS_00000.bin"))
    }) else { return Ok(()); };
    let bytes = fs::read(&path)?;
    if let Err(error) = crate::mac_newspaper::MacPaperDisplayStrings::from_strs(&bytes) {
        log::warn!("skipping unsupported Macintosh newspaper strings {}: {error:#}", path.display());
        return Ok(());
    }
    let dest = output.join("catalog/mac-newspaper");
    fs::create_dir_all(&dest)?;
    fs::write(dest.join("strs-0.bin"), bytes)?;
    Ok(())
}

/// A System font suitcase is optional and remains in the user's local pack.
fn import_newspaper_font(source: &Path, output: &Path) -> Result<()> {
    let Some(dir) = find_dir(source, "mac-system") else { return Ok(()); };
    let path = fs::read_dir(dir)?.flatten().map(|entry| entry.path()).find(|path| {
        path.is_file() && path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name.eq_ignore_ascii_case("New York") || name.eq_ignore_ascii_case("New York.bin")
        })
    });
    let Some(path) = path else { return Ok(()); };
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    crate::mac_font_metrics::MacFontFamily::from_suitcase(&bytes, 2)
        .with_context(|| format!("reading New York font suitcase {}", path.display()))?;
    let dest = output.join("catalog/mac-newspaper");
    fs::create_dir_all(&dest)?;
    fs::write(dest.join("new-york.bin"), bytes)?;
    Ok(())
}

/// The System font stays in the user's local asset pack.
fn import_system_chicago_font(source: &Path, output: &Path) -> Result<()> {
    let Some(dir) = find_dir(source, "mac-system") else { return Ok(()); };
    let path = fs::read_dir(dir)?.flatten().map(|entry| entry.path()).find(|path| {
        path.is_file() && path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name.eq_ignore_ascii_case("Chicago") || name.eq_ignore_ascii_case("Chicago.bin") || name.eq_ignore_ascii_case("Chicago.macbin")
        })
    });
    let Some(path) = path else { return Ok(()); };
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let family = crate::mac_font_metrics::MacFontFamily::from_suitcase(&bytes, 0)
        .with_context(|| format!("reading Chicago font suitcase {}", path.display()))?;
    ensure!(family.bitmap.contains_key(&12), "Chicago font has no 12-point bitmap");
    let dest = output.join("catalog/mac-system");
    fs::create_dir_all(&dest)?;
    fs::write(dest.join("chicago.bin"), bytes)?;
    Ok(())
}

/// Preserve the optional newspaper resources in the user's local imported pack.
fn import_newspaper_data(source: &Path, output: &Path) -> Result<()> {
    let Some(dir) = find_dir(source, "DATA") else { return Ok(()); };
    let paths = files_with_extension(&dir, "bin");
    let selected: Vec<_> = (1000..=1005).filter_map(|id| {
        let prefix = format!("DATA_{id:05}");
        paths.iter().find(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            name.get(..prefix.len()).is_some_and(|p| p.eq_ignore_ascii_case(&prefix))
                && name.as_bytes().get(prefix.len()).is_some_and(|b| *b == b'_' || *b == b'.')
        }).map(|path| (id, path))
    }).collect();
    if selected.is_empty() { return Ok(()); }
    let dest = output.join("catalog/mac-newspaper");
    fs::create_dir_all(&dest)?;
    for (id, path) in selected {
        fs::copy(path, dest.join(format!("{id}.bin")))
            .with_context(|| format!("importing Macintosh DATA {id} from {}", path.display()))?;
    }
    Ok(())
}

fn convert_palettes(source: &Path, output: &Path) -> Result<(Vec<[u8; 3]>, Vec<[u8; 3]>)> {
    let out = output.join("palettes");
    fs::create_dir_all(&out)?;
    let gray = grayscale_palette();
    let mut default = None;
    let mut system = None;
    if let Some(dir) = find_dir(source, "pltt") {
        for path in files_with_extension(&dir, "bin") {
            let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let colors = decode_pltt(&data);
            if colors.is_empty() {
                continue;
            }
            let name = clean_name(&path.file_name().unwrap_or_default().to_string_lossy());
            save_palette_image(&colors, &out.join(format!("{name}.png")))?;
            save_palette_csv(&colors, &out.join(format!("{name}.csv")), "index")?;
            let lower = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_ascii_lowercase();
            if lower.contains("00000") && lower.contains("default") {
                default = Some(colors.clone());
            }
            if lower.contains("01212") && lower.contains("system") {
                system = Some(colors.clone());
            }
        }
    }
    if let Some(dir) = find_dir(source, "clut") {
        for path in files_with_extension(&dir, "bin") {
            let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let (values, colors) = decode_clut(&data);
            if colors.is_empty() {
                continue;
            }
            let name = clean_name(&path.file_name().unwrap_or_default().to_string_lossy());
            save_palette_image(&colors, &out.join(format!("{name}.png")))?;
            let mut csv = String::from("value,r,g,b\n");
            for ((value, _), [r, g, b]) in values.iter().zip(&colors) {
                csv.push_str(&format!("{value},{r},{g},{b}\n"));
            }
            fs::write(out.join(format!("{name}.csv")), csv)?;
        }
    }
    let default = default.unwrap_or_else(|| gray.clone());
    let system16 = system.unwrap_or_else(|| default.iter().take(16).copied().collect());
    Ok((default, system16))
}

fn convert_picts(source: &Path, output: &Path) {
    let Some(dir) = find_dir(source, "PICT") else {
        return;
    };
    let dest = output.join("pict");
    for path in files_with_extension(&dir, "bin") {
        let result = (|| -> Result<()> {
            let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let mut wrapped = vec![0u8; 512];
            wrapped.extend_from_slice(&data);
            let decoded =
                decode_rgba8(&wrapped).with_context(|| format!("decoding {}", path.display()))?;
            let image = RgbaImage::from_raw(decoded.width, decoded.height, decoded.data)
                .context("PICT decoder returned an invalid RGBA buffer")?;
            fs::create_dir_all(&dest)?;
            image.save(dest.join(format!(
                "{}.png",
                clean_name(&path.file_name().unwrap_or_default().to_string_lossy())
            )))?;
            Ok(())
        })();
        if let Err(error) = result {
            log::warn!("skipping unsupported PICT {}: {error:#}", path.display());
        }
    }
}

fn convert_sprites(source: &Path, output: &Path, palette: &[[u8; 3]]) -> Result<usize> {
    let Some(dir) = find_dir(source, "SPRT") else {
        return Ok(0);
    };
    let mut total = 0;
    for path in files_with_extension(&dir, "bin") {
        let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let Some(count) = read_be_u16(&data, 0).map(usize::from) else {
            continue;
        };
        let group = clean_name(&path.file_name().unwrap_or_default().to_string_lossy());
        let mut sprites = Vec::new();
        for index in 0..count {
            let Some(record_at) = 2usize.checked_add(index.saturating_mul(10)) else {
                break;
            };
            if record_at + 10 > data.len() {
                break;
            }
            let id = u32::from(u16::from_be_bytes([data[record_at], data[record_at + 1]]));
            let offset = u32::from_be_bytes(data[record_at + 2..record_at + 6].try_into().unwrap());
            let height = u32::from(u16::from_be_bytes([
                data[record_at + 6],
                data[record_at + 7],
            ]));
            let width = u32::from(u16::from_be_bytes([
                data[record_at + 8],
                data[record_at + 9],
            ]));
            if width == 0
                || height == 0
                || width > 16_384
                || height > 16_384
                || offset as usize >= data.len()
            {
                continue;
            }
            match decode_sprt(&data, offset as usize, width, height, palette) {
                Ok(image) => sprites.push(Sprite {
                    index: index as u32,
                    id,
                    offset,
                    image,
                }),
                Err(error) => log::warn!(
                    "skipping malformed Mac sprite {} record {index}: {error:#}",
                    path.display()
                ),
            }
        }
        if sprites.is_empty() {
            continue;
        }
        total += sprites.len();
        write_atlas(&sprites, &output.join("sprites").join(group))?;
    }
    if total > 0 {
        for group in [
            "SPRT_00129_ShapeData",
            "SPRT_00130_ShapeData",
            "SPRT_00128_ShapeData",
        ] {
            let directory = output.join("sprites").join(group);
            if !directory.join("atlas.json").is_file() {
                write_atlas(&[], &directory)?;
            }
        }
    }
    Ok(total)
}

fn convert_icons(
    source: &Path,
    output: &Path,
    palette: &[[u8; 3]],
    system16: &[[u8; 3]],
) -> Result<()> {
    let out = output.join("icons");
    let masks32 = load_icon_masks(source, "ICN_");
    let masks16 = load_icon_masks(source, "ics_");
    for (folder, width, height) in [("ICN_", 32, 32), ("ics_", 16, 16)] {
        let Some(dir) = find_dir(source, folder) else {
            continue;
        };
        for path in files_with_extension(&dir, "bin") {
            let data = fs::read(&path)?;
            let image = decode_1bit_icon(&data, width, height);
            save_icon(
                &image,
                &out.join(folder).join(format!(
                    "{}.png",
                    clean_name(&path.file_name().unwrap_or_default().to_string_lossy())
                )),
            )?;
        }
    }
    for (kind, width, height, depth, masks) in [
        ("icl8", 32, 32, 8, &masks32),
        ("icl4", 32, 32, 4, &masks32),
        ("ics8", 16, 16, 8, &masks16),
        ("ics4", 16, 16, 4, &masks16),
    ] {
        let Some(dir) = find_dir(source, kind) else {
            continue;
        };
        for path in files_with_extension(&dir, "bin") {
            let data = fs::read(&path)?;
            let resource_id = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .rsplit('_')
                .next()
                .unwrap_or_default()
                .to_owned();
            let mask = masks.get(&resource_id).map(Vec::as_slice);
            let colors = if depth == 4 { system16 } else { palette };
            let image = decode_indexed_icon(&data, width, height, depth, colors, mask);
            save_icon(
                &image,
                &out.join(kind).join(format!(
                    "{}.png",
                    clean_name(&path.file_name().unwrap_or_default().to_string_lossy())
                )),
            )?;
        }
    }
    Ok(())
}

fn convert_cursors(source: &Path, output: &Path) -> Result<()> {
    let Some(dir) = find_dir(source, "CURS") else {
        return Ok(());
    };
    let out = output.join("cursors");
    fs::create_dir_all(&out)?;
    let mut csv = String::from("file,hotspot_x,hotspot_y\n");
    for path in files_with_extension(&dir, "bin") {
        let data = fs::read(&path)?;
        if data.len() < 68 {
            continue;
        }
        let mut image = RgbaImage::new(16, 16);
        for index in 0..256 {
            let image_bit = bit(&data[..32], index);
            let mask_bit = bit(&data[32..64], index);
            if mask_bit {
                let pixel = if image_bit {
                    [0, 0, 0, 255]
                } else {
                    [255, 255, 255, 255]
                };
                image.put_pixel((index % 16) as u32, (index / 16) as u32, Rgba(pixel));
            }
        }
        let filename = format!(
            "{}.png",
            clean_name(&path.file_name().unwrap_or_default().to_string_lossy())
        );
        image.save(out.join(&filename))?;
        let hotspot_y = u16::from_be_bytes([data[64], data[65]]);
        let hotspot_x = u16::from_be_bytes([data[66], data[67]]);
        csv.push_str(&format!("{filename},{hotspot_x},{hotspot_y}\n"));
    }
    fs::write(out.join("cursor_hotspots.csv"), csv)?;
    Ok(())
}

fn decode_pltt(data: &[u8]) -> Vec<[u8; 3]> {
    if data.len() < 16 {
        return Vec::new();
    }
    data[16..]
        .chunks_exact(16)
        .take(256)
        .map(|entry| [entry[0], entry[2], entry[4]])
        .collect()
}

fn decode_clut(data: &[u8]) -> (Vec<(u16, [u8; 3])>, Vec<[u8; 3]>) {
    if data.len() < 8 {
        return (Vec::new(), Vec::new());
    }
    let count = usize::from(u16::from_be_bytes([data[6], data[7]])) + 1;
    let mut entries = Vec::new();
    for row in data[8..].chunks_exact(8).take(count) {
        entries.push((
            u16::from_be_bytes([row[0], row[1]]),
            [row[2], row[4], row[6]],
        ));
    }
    let colors = entries.iter().map(|(_, color)| *color).collect();
    (entries, colors)
}

fn decode_sprt(
    data: &[u8],
    offset: usize,
    width: u32,
    height: u32,
    palette: &[[u8; 3]],
) -> Result<RgbaImage> {
    let mut image = RgbaImage::new(width, height);
    let mut pos = offset;
    let mut x = 0i64;
    let mut y = -1i64;
    let mut guard = 0usize;
    while pos + 2 <= data.len() && guard < data.len() {
        guard += 1;
        let count = usize::from(data[pos]);
        match data[pos + 1] {
            0 => pos += 2,
            1 => {
                y += 1;
                x = 0;
                pos += 2;
            }
            2 => break,
            3 => {
                x += count as i64;
                pos += 2;
            }
            4 => {
                let start = pos + 2;
                let end = start
                    .checked_add(count)
                    .context("Mac sprite literal size overflow")?;
                ensure!(end <= data.len(), "Mac sprite pixel block is truncated");
                for index in start..end {
                    if x >= 0 && y >= 0 && x < i64::from(width) && y < i64::from(height) {
                        let [r, g, b] = palette
                            .get(data[index] as usize)
                            .copied()
                            .unwrap_or([0, 0, 0]);
                        image.put_pixel(x as u32, y as u32, Rgba([r, g, b, 255]));
                    }
                    x += 1;
                }
                pos = end + (count & 1);
            }
            kind => anyhow::bail!("unknown Mac sprite block type {kind} at {pos}"),
        }
        if y >= i64::from(height) {
            break;
        }
    }
    Ok(image)
}

fn decode_1bit_icon(data: &[u8], width: usize, height: usize) -> RgbaImage {
    let plane_len = width.div_ceil(8) * height;
    let mut image = RgbaImage::new(width as u32, height as u32);
    for index in 0..width * height {
        if bit(data.get(..plane_len).unwrap_or_default(), index)
            && bit(
                data.get(plane_len..plane_len * 2).unwrap_or_default(),
                index,
            )
        {
            image.put_pixel(
                (index % width) as u32,
                (index / width) as u32,
                Rgba([0, 0, 0, 255]),
            );
        } else if bit(
            data.get(plane_len..plane_len * 2).unwrap_or_default(),
            index,
        ) {
            image.put_pixel(
                (index % width) as u32,
                (index / width) as u32,
                Rgba([255, 255, 255, 255]),
            );
        }
    }
    image
}

fn decode_indexed_icon(
    data: &[u8],
    width: usize,
    height: usize,
    depth: usize,
    palette: &[[u8; 3]],
    mask: Option<&[u8]>,
) -> RgbaImage {
    let plane_len = width.div_ceil(8) * height;
    let mask_bits = mask
        .filter(|bytes| bytes.len() >= plane_len * 2)
        .map(|bytes| &bytes[plane_len..plane_len * 2]);
    let mut image = RgbaImage::new(width as u32, height as u32);
    for index in 0..width * height {
        if mask_bits.is_some_and(|bits| !bit(bits, index)) {
            continue;
        }
        let color_index = if depth == 8 {
            data.get(index).copied().unwrap_or(0) as usize
        } else {
            let value = data.get(index / 2).copied().unwrap_or(0);
            usize::from(if index % 2 == 0 {
                value >> 4
            } else {
                value & 0x0f
            })
        };
        let [r, g, b] = palette.get(color_index).copied().unwrap_or([0, 0, 0]);
        image.put_pixel(
            (index % width) as u32,
            (index / width) as u32,
            Rgba([r, g, b, 255]),
        );
    }
    image
}

fn load_icon_masks(source: &Path, folder: &str) -> HashMap<String, Vec<u8>> {
    let Some(dir) = find_dir(source, folder) else {
        return HashMap::new();
    };
    files_with_extension(&dir, "bin")
        .into_iter()
        .filter_map(|path| {
            let id = path
                .file_stem()?
                .to_string_lossy()
                .rsplit('_')
                .next()?
                .to_owned();
            fs::read(path).ok().map(|data| (id, data))
        })
        .collect()
}

fn bit(bytes: &[u8], index: usize) -> bool {
    bytes
        .get(index / 8)
        .is_some_and(|value| value & (0x80 >> (index % 8)) != 0)
}

fn write_atlas(sprites: &[Sprite], directory: &Path) -> Result<()> {
    fs::create_dir_all(directory).with_context(|| format!("creating {}", directory.display()))?;
    let individual = directory.join("individual");
    fs::create_dir_all(&individual)?;
    let mut x = ATLAS_PADDING;
    let mut y = ATLAS_PADDING;
    let mut row_height = 0;
    let mut placements = Vec::with_capacity(sprites.len());
    for sprite in sprites {
        let (width, height) = sprite.image.dimensions();
        ensure!(
            width + ATLAS_PADDING * 2 <= ATLAS_WIDTH,
            "Mac sprite {} exceeds atlas width",
            sprite.id
        );
        if x + width + ATLAS_PADDING > ATLAS_WIDTH {
            x = ATLAS_PADDING;
            y = y
                .checked_add(row_height + ATLAS_PADDING)
                .context("Mac atlas height overflow")?;
            row_height = 0;
        }
        placements.push((x, y));
        x += width + ATLAS_PADDING;
        row_height = row_height.max(height);
        let filename = format!(
            "sprite_{:04}_id_{:05}_{}x{}.png",
            sprite.index, sprite.id, width, height
        );
        sprite.image.save(individual.join(filename))?;
    }
    let atlas_height = y
        .checked_add(row_height + ATLAS_PADDING)
        .context("Mac atlas height overflow")?
        .max(1);
    let mut atlas = RgbaImage::new(
        if sprites.is_empty() { 1 } else { ATLAS_WIDTH },
        atlas_height,
    );
    let mut frames = Vec::with_capacity(sprites.len());
    for (sprite, (x, y)) in sprites.iter().zip(placements) {
        let (w, h) = sprite.image.dimensions();
        atlas
            .copy_from(&sprite.image, x, y)
            .context("placing Mac sprite in atlas")?;
        frames.push(AtlasFrame {
            file: format!(
                "individual/sprite_{:04}_id_{:05}_{}x{}.png",
                sprite.index, sprite.id, w, h
            ),
            index: sprite.index,
            id: sprite.id,
            source_offset: sprite.offset,
            x,
            y,
            w,
            h,
        });
    }
    atlas.save(directory.join("atlas.png"))?;
    fs::write(
        directory.join("atlas.json"),
        serde_json::to_vec_pretty(&AtlasManifest {
            image: "atlas.png".into(),
            frames,
        })?,
    )?;
    Ok(())
}

fn save_palette_image(colors: &[[u8; 3]], path: &Path) -> Result<()> {
    let cell = 18u32;
    let columns = 16usize;
    let rows = colors.len().div_ceil(columns).max(1);
    let mut image = RgbaImage::new(columns as u32 * cell, rows as u32 * cell);
    for (index, [r, g, b]) in colors.iter().enumerate() {
        for y in (index / columns) as u32 * cell..((index / columns) as u32 + 1) * cell {
            for x in (index % columns) as u32 * cell..((index % columns) as u32 + 1) * cell {
                image.put_pixel(x, y, Rgba([*r, *g, *b, 255]));
            }
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    image
        .save(path)
        .with_context(|| format!("writing {}", path.display()))
}

fn save_palette_csv(colors: &[[u8; 3]], path: &Path, first_column: &str) -> Result<()> {
    let mut csv = format!("{first_column},r,g,b\n");
    for (index, [r, g, b]) in colors.iter().enumerate() {
        csv.push_str(&format!("{index},{r},{g},{b}\n"));
    }
    fs::write(path, csv).with_context(|| format!("writing {}", path.display()))
}

fn save_icon(image: &RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    image
        .save(path)
        .with_context(|| format!("writing {}", path.display()))
}

fn grayscale_palette() -> Vec<[u8; 3]> {
    (0..=255).map(|value| [value, value, value]).collect()
}

fn find_dir(root: &Path, component: &str) -> Option<PathBuf> {
    fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(component))
        })
}

fn files_with_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case(extension))
        })
        .collect();
    paths.sort();
    paths
}

fn clean_name(name: &str) -> String {
    let name = name
        .strip_suffix(".bin")
        .or_else(|| name.strip_suffix(".BIN"))
        .unwrap_or(name);
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || "_.+-".contains(ch) {
                ch
            } else {
                '_'
            }
        })
        .collect();
    cleaned.trim_matches('_').to_owned()
}

fn read_be_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_shape_map_expands_data_and_rejects_bad_streams() {
        let mut code = [0u8; 8];
        code[4..].copy_from_slice(&0x7394u32.to_be_bytes());
        let mut data = vec![1u8; 0x7394];
        let start = 0x7394 - 0x5082;
        data[start..start + 2].copy_from_slice(&0x1234u16.to_be_bytes());
        let map = decode_terrain_shape_map(&code, &data, &[]).unwrap();
        assert_eq!(map.len(), 128);
        assert_eq!(map[0], 0x1234);
        assert_eq!(map[1], 0x0101);
        let zero_run = (0x7394u16 - 2).to_be_bytes();
        assert_eq!(decode_terrain_shape_map(&code, &[0, 0], &zero_run).unwrap(), vec![0; 128]);
        assert!(decode_terrain_shape_map(&code, &[0, 0], &[]).is_err());
        assert!(decode_terrain_shape_map(&code, &[0, 0], &[0xff, 0xff]).is_err());
        assert!(decode_terrain_shape_map(&code, &data[..data.len() - 1], &[]).is_err());
    }

    #[test]
    fn imported_terrain_map_and_atlases_form_shape_lookup() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("citygame-mac-shapes-{}-{unique}", std::process::id()));
        let source = root.join("source");
        let output = root.join("output");
        for group in ["CODE", "DATA", "ZERO"] { fs::create_dir_all(source.join(group)).unwrap(); }
        let mut code = [0u8; 8];
        code[4..].copy_from_slice(&0x7394u32.to_be_bytes());
        fs::write(source.join("CODE/CODE_00000.bin"), code).unwrap();
        let mut data = vec![1u8; 0x7394];
        let start = 0x7394 - 0x5082;
        data[start..start + 2].copy_from_slice(&12u16.to_be_bytes());
        fs::write(source.join("DATA/DATA_00000.bin"), data).unwrap();
        fs::write(source.join("ZERO/ZERO_00000.bin"), []).unwrap();
        import_terrain_shape_map(&source, &output).unwrap();
        for (group, width) in [(128, 3), (129, 4), (130, 5)] {
            let path = output.join(format!("sprites/SPRT_{group:05}_ShapeData"));
            fs::create_dir_all(&path).unwrap();
            let atlas = serde_json::json!({"image":"atlas.png", "frames":[
                {"file":"x", "id":12, "source_offset":0, "x":0, "y":0, "w":width, "h":7},
                {"file":"x", "id":512, "source_offset":0, "x":0, "y":0, "w":8, "h":9}
            ]});
            fs::write(path.join("atlas.json"), serde_json::to_vec(&atlas).unwrap()).unwrap();
        }
        let shapes = crate::mac_shape_table::MacShapeDimensions::from_pack(&output).unwrap();
        assert_eq!(shapes.terrain_dimensions(0, 0), Some((5, 7)));
        assert_eq!(shapes.terrain_dimensions(0, 1), Some((8, 9)));
        assert_eq!(shapes.building_dimensions(12, 0), Some((5, 7)));
        assert_eq!(shapes.terrain_dimensions(128, 0), None);
        assert_eq!(shapes.terrain_dimensions(0, 3), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn imports_optional_newspaper_resources() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("citygame-mac-paper-{}-{unique}", std::process::id()));
        let source = root.join("source"); let output = root.join("output");
        fs::create_dir_all(source.join("DATA")).unwrap();
        fs::write(source.join("DATA/DATA_01000_Group Start.bin"), b"start").unwrap();
        fs::write(source.join("DATA/DATA_01003_Token Data.bin"), b"tokens").unwrap();
        fs::write(source.join("DATA/DATA_010030_Other.bin"), b"wrong").unwrap();
        import_newspaper_data(&source, &output).unwrap();
        assert_eq!(fs::read(output.join("catalog/mac-newspaper/1000.bin")).unwrap(), b"start");
        assert_eq!(fs::read(output.join("catalog/mac-newspaper/1003.bin")).unwrap(), b"tokens");
        assert!(!output.join("catalog/mac-newspaper/1001.bin").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn indexes_only_matching_newspaper_picture_ids() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("citygame-mac-pictures-{}-{unique}", std::process::id()));
        let source = root.join("source"); let output = root.join("output");
        fs::create_dir_all(source.join("PICT")).unwrap();
        fs::write(source.join("PICT/PICT_00403.bin"), b"present").unwrap();
        fs::write(source.join("PICT/PICT_00411_label.bin"), b"present").unwrap();
        fs::write(source.join("PICT/PICT_004000.bin"), b"wrong id").unwrap();
        import_newspaper_picture_index(&source, &output).unwrap();
        let mask = fs::read(output.join("catalog/mac-newspaper/pictures.bin")).unwrap();
        assert_eq!(mask, [0,0,0,1,0,0,0,0,0,0,0,1]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires the locally extracted Macintosh STRS resource"]
    fn imports_optional_mac_newspaper_display_strings() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("citygame-mac-strs-{}-{unique}", std::process::id()));
        let source = root.join("source"); let output = root.join("output");
        fs::create_dir_all(source.join("STRS")).unwrap();
        let bytes = fs::read(std::env::var_os("CITYGAME_MAC_STRS").unwrap()).unwrap();
        fs::write(source.join("STRS/STRS_00000.bin"), &bytes).unwrap();
        import_newspaper_strings(&source, &output).unwrap();
        assert_eq!(fs::read(output.join("catalog/mac-newspaper/strs-0.bin")).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires the locally extracted Macintosh System font suitcase"]
    fn imports_optional_chicago_suitcase() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("citygame-mac-chicago-{}-{unique}", std::process::id()));
        let source = root.join("source"); let output = root.join("output");
        fs::create_dir_all(source.join("mac-system")).unwrap();
        let original = fs::read(std::env::var_os("CITYGAME_MAC_CHICAGO_FONT").unwrap()).unwrap();
        fs::write(source.join("mac-system/Chicago.macbin"), &original).unwrap();
        import_system_chicago_font(&source, &output).unwrap();
        assert_eq!(fs::read(output.join("catalog/mac-system/chicago.bin")).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires the locally extracted Macintosh System font suitcase"]
    fn imports_optional_new_york_suitcase() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("citygame-mac-font-{}-{unique}", std::process::id()));
        let source = root.join("source");
        let output = root.join("output");
        fs::create_dir_all(source.join("mac-system")).unwrap();
        let capture = PathBuf::from(std::env::var("CITYGAME_MAC_PAPER_WIDTHS").unwrap());
        let original = fs::read(capture.join("newyork.bin")).unwrap();
        fs::write(source.join("mac-system/New York.bin"), &original).unwrap();
        import_newspaper_font(&source, &output).unwrap();
        assert_eq!(fs::read(output.join("catalog/mac-newspaper/new-york.bin")).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn decodes_palette_resource_components_and_clut() {
        let mut pltt = vec![0; 32];
        pltt[16..32].copy_from_slice(&[12, 0, 34, 0, 56, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(decode_pltt(&pltt), vec![[12, 34, 56]]);

        let mut clut = vec![0; 16];
        clut[6..8].copy_from_slice(&0u16.to_be_bytes());
        clut[8..16].copy_from_slice(&[0, 7, 11, 0, 22, 0, 33, 0]);
        assert_eq!(
            decode_clut(&clut),
            (vec![(7, [11, 22, 33])], vec![[11, 22, 33]])
        );
    }

    #[test]
    fn icon_mask_makes_background_transparent() {
        let mut data = vec![0; 256];
        data[0] = 0x80;
        data[128] = 0x80;
        let icon = decode_1bit_icon(&data, 32, 32);
        assert_eq!(icon.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(icon.get_pixel(1, 0).0, [0, 0, 0, 0]);
    }

    #[test]
    fn imports_synthetic_mac_resource_export_into_runtime_pack_layout() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch =
            std::env::temp_dir().join(format!("sc2k-mac-import-{}-{unique}", std::process::id()));
        let source = scratch.join("source");
        let output = scratch.join("output");
        fs::create_dir_all(source.join("SPRT")).unwrap();
        fs::create_dir_all(source.join("pltt")).unwrap();

        let mut palette = vec![0; 16 + 256 * 16];
        palette[16 + 5 * 16..16 + 5 * 16 + 5].copy_from_slice(&[20, 0, 40, 0, 60]);
        fs::write(source.join("pltt/pltt_00000_Default Palette.bin"), palette).unwrap();
        let mut sprite = vec![0, 1, 0x04, 0xb3, 0, 0, 0, 12, 0, 1, 0, 2];
        sprite.extend_from_slice(&[0, 1, 2, 4, 5, 6, 0, 2]);
        fs::write(source.join("SPRT/SPRT_00128_ShapeData.bin"), sprite).unwrap();
        for (resource, id) in [(129, 1201), (130, 1202)] {
            let mut extra_sprite = vec![0, 1];
            extra_sprite.extend_from_slice(&u16::try_from(id).unwrap().to_be_bytes());
            extra_sprite.extend_from_slice(&12u32.to_be_bytes());
            extra_sprite.extend_from_slice(&1u16.to_be_bytes());
            extra_sprite.extend_from_slice(&1u16.to_be_bytes());
            extra_sprite.extend_from_slice(&[0, 1, 1, 4, 5, 0, 0, 2]);
            fs::write(
                source
                    .join("SPRT")
                    .join(format!("SPRT_{resource:05}_ShapeData.bin")),
                extra_sprite,
            )
            .unwrap();
        }

        import(&source, &output).unwrap();
        let atlas = output.join("sprites/SPRT_00128_ShapeData/atlas.json");
        assert!(atlas.is_file());
        crate::asset_profile::validate_original_pack(&output).unwrap();
        crate::asset_catalog::Catalog::load(std::slice::from_ref(&output)).unwrap();
        fs::remove_dir_all(scratch).unwrap();
    }
}
