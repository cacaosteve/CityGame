//! Native importer for the DOS / GOG data archives and their indexed assets.

use crate::{audio_import, win95_import};
use anyhow::{Context, Result, ensure};
use image::{GenericImage, Rgba, RgbaImage};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const ATLAS_WIDTH: u32 = 4096;
const ATLAS_PADDING: u32 = 2;

#[derive(Debug)]
struct ArchiveEntry {
    name: String,
    offset: usize,
    data: Vec<u8>,
}

#[derive(Debug)]
struct HedRecord {
    id: u32,
    offset: usize,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct Tile {
    id: u32,
    offset: usize,
    image: RgbaImage,
}

#[derive(Serialize)]
struct AtlasManifest {
    image: String,
    frames: Vec<AtlasFrame>,
}

#[derive(Serialize)]
struct AtlasFrame {
    file: String,
    id: u32,
    source_offset: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// Extract the supported graphics, palettes, and sound data with native Rust.
pub fn import(source: &Path, output: &Path) -> Result<()> {
    ensure!(!output.exists(), "{} already exists", output.display());
    ensure!(source.is_dir(), "{} must be a folder", source.display());
    fs::create_dir_all(output).with_context(|| format!("creating {}", output.display()))?;

    let archives = archive_candidates(source);
    let mut dos_palette = None;
    let mut dos_raw = None;
    let mut main_large_tiles = 0usize;
    for path in archives {
        let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let Some(entries) = parse_mxs_archive(&data) else {
            continue;
        };
        let relative = path.strip_prefix(source).unwrap_or(&path).to_string_lossy();
        let archive_id = if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("SC2000.DAT"))
        {
            "SC2000_DAT".to_owned()
        } else {
            clean_name(&relative.replace(['/', '\\', '.'], "_"))
        };
        let raw_dir = output.join("archives").join(&archive_id).join("raw");
        fs::create_dir_all(&raw_dir).with_context(|| format!("creating {}", raw_dir.display()))?;
        let mut child_map = std::collections::HashMap::new();
        for entry in entries {
            let name = clean_name(&entry.name);
            fs::write(raw_dir.join(&name), &entry.data)
                .with_context(|| format!("extracting {name} from {}", path.display()))?;
            child_map.insert(entry.name.to_ascii_uppercase(), entry.data);
        }
        if dos_palette.is_none() {
            dos_palette = child_map
                .get("MINE.PAL")
                .filter(|data| data.len() >= 768)
                .map(|data| {
                    data[..768]
                        .chunks_exact(3)
                        .map(|rgb| [rgb[0], rgb[1], rgb[2]])
                        .collect::<Vec<_>>()
                });
        }
        if archive_id.eq_ignore_ascii_case("SC2000_DAT") {
            dos_raw = Some(raw_dir.clone());
            main_large_tiles = decode_archive_tiles(&archive_id, &child_map, output)?;
        } else {
            decode_archive_tiles(&archive_id, &child_map, output)?;
        }
    }

    ensure!(
        main_large_tiles > 0,
        "SC2000.DAT did not contain a usable LARGE.DAT/HED tile set"
    );

    let palette = dos_palette.unwrap_or_else(grayscale_palette);
    write_palette_image(&palette, &output.join("palettes/mine_palette.png"))?;
    write_palette_csv(&palette, &output.join("palettes/mine_palette.csv"))?;
    convert_pcx(source, output)?;
    if let Some(raw_dir) = dos_raw {
        let (effects, music) = audio_import::import_dos(&raw_dir, output)?;
        log::info!("normalized DOS media: {effects} effects, {music} music cues");
    }
    Ok(())
}

fn archive_candidates(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file()
                && path.extension().is_some_and(|ext| {
                    ["dat", "urk", "til"]
                        .iter()
                        .any(|suffix| ext.to_string_lossy().eq_ignore_ascii_case(suffix))
                })
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn parse_mxs_archive(data: &[u8]) -> Option<Vec<ArchiveEntry>> {
    if data.len() < 16 {
        return None;
    }
    let first_offset = u32::from_le_bytes(data[12..16].try_into().ok()?) as usize;
    if first_offset < 16 || first_offset > data.len() || first_offset % 16 != 0 {
        return None;
    }
    let count = first_offset / 16;
    if count == 0 || count > 4096 {
        return None;
    }
    let mut records = Vec::with_capacity(count);
    let mut previous = first_offset;
    for index in 0..count {
        let row = &data[index * 16..index * 16 + 16];
        let raw_name = row[..12]
            .split(|byte| *byte == 0)
            .next()
            .unwrap_or_default()
            .trim_ascii_end();
        if raw_name.is_empty() || !raw_name.iter().all(|byte| (32..127).contains(byte)) {
            return None;
        }
        let offset = u32::from_le_bytes(row[12..16].try_into().ok()?) as usize;
        if offset < first_offset || offset > data.len() || (index > 0 && offset < previous) {
            return None;
        }
        previous = offset;
        records.push((String::from_utf8_lossy(raw_name).into_owned(), offset));
    }
    let mut entries = Vec::with_capacity(records.len());
    for (index, (name, offset)) in records.iter().enumerate() {
        let end = records.get(index + 1).map_or(data.len(), |next| next.1);
        entries.push(ArchiveEntry {
            name: name.clone(),
            offset: *offset,
            data: data[*offset..end].to_vec(),
        });
    }
    Some(entries)
}

fn decode_archive_tiles(
    archive_id: &str,
    children: &std::collections::HashMap<String, Vec<u8>>,
    output: &Path,
) -> Result<usize> {
    let mut large_count = 0;
    for set in ["LARGE", "SMALL", "OTHER"] {
        let directory = output
            .join("tiles")
            .join(archive_id)
            .join(set.to_ascii_lowercase());
        let (Some(dat), Some(hed)) = (
            children.get(&format!("{set}.DAT")),
            children.get(&format!("{set}.HED")),
        ) else {
            if archive_id.eq_ignore_ascii_case("SC2000_DAT") {
                write_atlas(&[], &directory)?;
            }
            continue;
        };
        let records = parse_hed(hed, dat.len());
        if records.is_empty() {
            if archive_id.eq_ignore_ascii_case("SC2000_DAT") {
                write_atlas(&[], &directory)?;
            }
            continue;
        }
        let mut tiles = Vec::with_capacity(records.len());
        for (index, record) in records.iter().enumerate() {
            let end = records.get(index + 1).map_or(dat.len(), |next| next.offset);
            if end < record.offset {
                continue;
            }
            let image = decode_tile(
                &dat[record.offset..end],
                record.width,
                record.height,
                children
                    .get("MINE.PAL")
                    .filter(|bytes| bytes.len() >= 768)
                    .map(|bytes| {
                        bytes[..768]
                            .chunks_exact(3)
                            .map(|rgb| [rgb[0], rgb[1], rgb[2]])
                            .collect()
                    })
                    .unwrap_or_else(grayscale_palette),
            );
            tiles.push(Tile {
                id: record.id,
                offset: record.offset,
                image,
            });
        }
        if set == "LARGE" {
            large_count = tiles.len();
        }
        write_atlas(&tiles, &directory)?;
    }
    Ok(large_count)
}

fn parse_hed(data: &[u8], dat_len: usize) -> Vec<HedRecord> {
    let mut records = Vec::new();
    for (id, row) in data.chunks_exact(8).enumerate() {
        if row == [0xff; 8] {
            continue;
        }
        let offset = u32::from_le_bytes(row[..4].try_into().unwrap()) as usize;
        let height = u32::from(row[4]);
        let width = u32::from(row[5]);
        if width == 0 || height == 0 || offset > dat_len {
            continue;
        }
        records.push(HedRecord {
            id: id as u32,
            offset,
            width,
            height,
        });
    }
    records.sort_by_key(|record| record.offset);
    records
}

fn decode_tile(data: &[u8], width: u32, height: u32, palette: Vec<[u8; 3]>) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);
    let mut pos = 0usize;
    let mut x = 0i64;
    let mut y = -1i64;
    while pos < data.len() {
        let command = data[pos];
        pos += 1;
        match command {
            0x00 => break,
            0x10 => {
                if pos >= data.len() {
                    break;
                }
                pos += 1;
                y += 1;
                x = 0;
            }
            0x04 => {
                if pos >= data.len() {
                    break;
                }
                x += i64::from(data[pos]);
                pos += 1;
            }
            0x0c => {
                if pos >= data.len() {
                    break;
                }
                let count = data[pos] as usize;
                pos += 1;
                let Some(end) = pos.checked_add(count) else {
                    break;
                };
                if end > data.len() {
                    break;
                }
                for index in pos..end {
                    if x >= 0 && y >= 0 && x < i64::from(width) && y < i64::from(height) {
                        let color = palette
                            .get(data[index] as usize)
                            .copied()
                            .unwrap_or([0, 0, 0]);
                        image.put_pixel(
                            x as u32,
                            y as u32,
                            Rgba([color[0], color[1], color[2], 255]),
                        );
                    }
                    x += 1;
                }
                pos = end;
            }
            _ => break,
        }
    }
    image
}

fn convert_pcx(source: &Path, output: &Path) -> Result<()> {
    for path in recursive_files(source) {
        if !path
            .extension()
            .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("pcx"))
        {
            continue;
        }
        let relative = path
            .strip_prefix(source)
            .context("PCX path escaped the install root")?;
        let target = output.join("pcx_png").join(relative).with_extension("png");
        let decoded = fs::read(&path)
            .with_context(|| format!("reading {}", path.display()))
            .and_then(|bytes| win95_import::decode_pcx(&bytes));
        match decoded {
            Ok(image) => {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                image
                    .save(&target)
                    .with_context(|| format!("writing {}", target.display()))?;
            }
            Err(error) => log::warn!("skipping unreadable PCX {}: {error:#}", path.display()),
        }
    }
    Ok(())
}

fn write_atlas(tiles: &[Tile], directory: &Path) -> Result<()> {
    fs::create_dir_all(directory).with_context(|| format!("creating {}", directory.display()))?;
    let individual = directory.join("individual");
    fs::create_dir_all(&individual)?;
    let mut x = ATLAS_PADDING;
    let mut y = ATLAS_PADDING;
    let mut row_height = 0;
    let mut placements = Vec::with_capacity(tiles.len());
    for tile in tiles {
        let (width, height) = tile.image.dimensions();
        ensure!(
            width + ATLAS_PADDING * 2 <= ATLAS_WIDTH,
            "DOS tile {} exceeds atlas width",
            tile.id
        );
        if x + width + ATLAS_PADDING > ATLAS_WIDTH {
            x = ATLAS_PADDING;
            y = y
                .checked_add(row_height + ATLAS_PADDING)
                .context("DOS atlas height overflow")?;
            row_height = 0;
        }
        placements.push((x, y));
        x += width + ATLAS_PADDING;
        row_height = row_height.max(height);
        let file = format!("tile_id_{:05}_{}x{}.png", tile.id, width, height);
        tile.image
            .save(individual.join(&file))
            .with_context(|| format!("writing DOS tile {}", file))?;
    }
    let atlas_height = y
        .checked_add(row_height + ATLAS_PADDING)
        .context("DOS atlas height overflow")?
        .max(1);
    let mut atlas = RgbaImage::new(if tiles.is_empty() { 1 } else { ATLAS_WIDTH }, atlas_height);
    let mut frames = Vec::with_capacity(tiles.len());
    for (tile, (x, y)) in tiles.iter().zip(placements) {
        let (w, h) = tile.image.dimensions();
        atlas
            .copy_from(&tile.image, x, y)
            .context("placing DOS tile in atlas")?;
        frames.push(AtlasFrame {
            file: format!("individual/tile_id_{:05}_{}x{}.png", tile.id, w, h),
            id: tile.id,
            source_offset: u32::try_from(tile.offset).unwrap_or(u32::MAX),
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

fn grayscale_palette() -> Vec<[u8; 3]> {
    (0..=255).map(|value| [value, value, value]).collect()
}

fn write_palette_image(colors: &[[u8; 3]], path: &Path) -> Result<()> {
    let cell = 18;
    let columns = 16;
    let rows = colors.len().div_ceil(columns).max(1);
    let mut image = RgbaImage::new((columns * cell) as u32, (rows * cell) as u32);
    for (index, color) in colors.iter().enumerate() {
        let x0 = (index % columns * cell) as u32;
        let y0 = (index / columns * cell) as u32;
        for y in y0..y0 + cell as u32 {
            for x in x0..x0 + cell as u32 {
                image.put_pixel(x, y, Rgba([color[0], color[1], color[2], 255]));
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

fn write_palette_csv(colors: &[[u8; 3]], path: &Path) -> Result<()> {
    let mut text = String::from("index,r,g,b\n");
    for (index, [r, g, b]) in colors.iter().enumerate() {
        text.push_str(&format!("{index},{r},{g},{b}\n"));
    }
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

fn recursive_files(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn clean_name(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || "_.+-".contains(ch) {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "unnamed".into()
    } else {
        trimmed.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mxs_archive_and_decodes_small_tile_set() {
        let mut archive = vec![0u8; 48];
        archive[..12].copy_from_slice(b"MINE.PAL\0\0\0\0");
        archive[12..16].copy_from_slice(&32u32.to_le_bytes());
        archive[16..28].copy_from_slice(b"LARGE.DAT\0\0\0");
        archive[28..32].copy_from_slice(&48u32.to_le_bytes());
        archive[32..44].copy_from_slice(b"LARGE.HED\0\0\0");
        archive[44..48].copy_from_slice(&48u32.to_le_bytes());
        let entries = parse_mxs_archive(&archive).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "MINE.PAL");

        let palette = vec![[3, 4, 5]; 256];
        let image = decode_tile(&[0x10, 1, 0x0c, 2, 5, 6, 0], 2, 1, palette);
        assert_eq!(image.get_pixel(0, 0).0, [3, 4, 5, 255]);
        assert_eq!(image.get_pixel(1, 0).0, [3, 4, 5, 255]);
    }

    #[test]
    fn imports_synthetic_gog_archive_into_runtime_pack_layout() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let scratch =
            std::env::temp_dir().join(format!("sc2k-dos-import-{}-{unique}", std::process::id()));
        let source = scratch.join("source");
        let output = scratch.join("output");
        fs::create_dir_all(&source).unwrap();

        let mut palette = vec![0; 768];
        palette[15..18].copy_from_slice(&[20, 40, 60]);
        let mut dat = vec![0, 1, 0x04, 0xbb, 0, 0, 0, 12, 0, 1, 0, 2];
        dat.extend_from_slice(&[0x10, 0, 0x0c, 2, 5, 6, 0]);
        let mut hed = vec![0; 8];
        hed[..4].copy_from_slice(&0u32.to_le_bytes());
        hed[4] = 1;
        hed[5] = 2;
        let archive = build_archive(&[
            ("MINE.PAL", palette),
            ("LARGE.DAT", dat.clone()),
            ("LARGE.HED", hed.clone()),
            ("SMALL.DAT", dat.clone()),
            ("SMALL.HED", hed.clone()),
            ("OTHER.DAT", dat),
            ("OTHER.HED", hed),
        ]);
        fs::write(source.join("SC2000.DAT"), archive).unwrap();

        import(&source, &output).unwrap();
        assert!(output.join("tiles/SC2000_DAT/large/atlas.json").is_file());
        assert!(output.join("archives/SC2000_DAT/raw/MINE.PAL").is_file());
        crate::asset_profile::validate_original_pack(&output).unwrap();
        crate::asset_catalog::Catalog::load(std::slice::from_ref(&output)).unwrap();
        fs::remove_dir_all(scratch).unwrap();
    }

    fn build_archive(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let table_size = entries.len() * 16;
        let mut archive = vec![0; table_size];
        let mut offset = table_size;
        for (index, (name, body)) in entries.iter().enumerate() {
            let row = &mut archive[index * 16..index * 16 + 16];
            row[..name.len()].copy_from_slice(name.as_bytes());
            row[12..16].copy_from_slice(&(offset as u32).to_le_bytes());
            offset += body.len();
        }
        archive[12..16].copy_from_slice(&(table_size as u32).to_le_bytes());
        for (_, body) in entries {
            archive.extend_from_slice(body);
        }
        archive
    }
}
