use anyhow::{Context, Result, bail, ensure};
use image::{Rgba, RgbaImage};
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct AtlasManifest {
    pub image: String,
    pub frames: Vec<AtlasFrame>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct AtlasFrame {
    pub file: String,
    #[serde(default)]
    pub index: u32,
    pub id: u32,
    pub source_offset: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// World-space canvas dimensions. High-density replacements can carry more
    /// texture pixels while retaining the placement contract of the original sprite.
    #[serde(default)]
    pub logical_size: Option<[u32; 2]>,
    /// Pixel in the source image that should land on the engine's sprite anchor.
    /// Coordinates are in logical pixels when `logical_size` is present. Extracted
    /// atlases omit both fields and retain the historical bottom-center behavior.
    #[serde(default)]
    pub anchor: Option<[f32; 2]>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AtlasSource {
    pub name: String,
    pub dir: PathBuf,
    pub image_path: PathBuf,
    pub manifest: AtlasManifest,
    by_id: HashMap<u32, usize>,
    decoded_image: Option<RgbaImage>,
}

impl AtlasSource {
    pub fn load(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let manifest_path = dir.join("atlas.json");
        let manifest_text = fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading atlas manifest {}", manifest_path.display()))?;
        let mut manifest: AtlasManifest = serde_json::from_str(&manifest_text)
            .with_context(|| format!("parsing atlas manifest {}", manifest_path.display()))?;
        // Older PC extraction manifests omitted `index`; normalize every atlas to its
        // manifest order so runtime animation metadata stays deterministic.
        for (index, frame) in manifest.frames.iter_mut().enumerate() {
            frame.index = index as u32;
        }
        let image_path = dir.join(&manifest.image);
        let name = dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("atlas")
            .to_owned();
        let by_id = manifest
            .frames
            .iter()
            .enumerate()
            .map(|(index, frame)| (frame.id, index))
            .collect();

        Ok(Self {
            name,
            dir,
            image_path,
            manifest,
            by_id,
            decoded_image: None,
        })
    }

    pub fn from_rgba(name: impl Into<String>, image: RgbaImage, frames: Vec<AtlasFrame>) -> Self {
        let name = name.into();
        let by_id = frames
            .iter()
            .enumerate()
            .map(|(index, frame)| (frame.id, index))
            .collect();
        Self {
            dir: PathBuf::new(),
            image_path: PathBuf::from(format!("<{name}>")),
            manifest: AtlasManifest {
                image: format!("{name}.png"),
                frames,
            },
            name,
            by_id,
            decoded_image: Some(image),
        }
    }

    pub fn from_image_path(name: impl Into<String>, image_path: PathBuf, id: u32) -> Result<Self> {
        let name = name.into();
        let (w, h) = image::image_dimensions(&image_path)
            .with_context(|| format!("reading image {}", image_path.display()))?;
        Ok(Self {
            dir: image_path.parent().unwrap_or(Path::new("")).to_path_buf(),
            image_path: image_path.clone(),
            manifest: AtlasManifest {
                image: image_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                frames: vec![AtlasFrame {
                    file: image_path.to_string_lossy().into_owned(),
                    index: 0,
                    id,
                    source_offset: 0,
                    x: 0,
                    y: 0,
                    w,
                    h,
                    logical_size: None,
                    anchor: None,
                }],
            },
            name,
            by_id: HashMap::from([(id, 0)]),
            decoded_image: None,
        })
    }

    pub fn load_rgba(&self) -> Result<RgbaImage> {
        if let Some(image) = &self.decoded_image {
            return Ok(image.clone());
        }
        image::open(&self.image_path)
            .with_context(|| format!("loading atlas image {}", self.image_path.display()))
            .map(|image| image.to_rgba8())
    }

    #[allow(dead_code)]
    pub fn frame_by_id(&self, id: u32) -> Option<&AtlasFrame> {
        self.by_id
            .get(&id)
            .and_then(|index| self.manifest.frames.get(*index))
    }

    #[allow(dead_code)]
    pub fn frame_by_index(&self, index: usize) -> Option<&AtlasFrame> {
        self.manifest.frames.get(index)
    }

    pub fn sorted_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.manifest.frames.iter().map(|frame| frame.id).collect();
        ids.sort_unstable();
        ids
    }
}

impl AtlasFrame {
    pub fn display_size(&self) -> (u32, u32) {
        self.logical_size
            .map(|[w, h]| (w, h))
            .unwrap_or((self.w, self.h))
    }
}

pub fn load_sprite_sources(asset_root: impl AsRef<Path>) -> Result<Vec<AtlasSource>> {
    let root = asset_root.as_ref();
    let (sprite_root, preferred): (PathBuf, &[&str]) = if root.join("sprites").is_dir() {
        (
            root.join("sprites"),
            &[
                "SPRT_00129_ShapeData",
                "SPRT_00130_ShapeData",
                "SPRT_00128_ShapeData",
            ],
        )
    } else if root.join("win95_tiles").is_dir() {
        (root.join("win95_tiles"), &["LARGE", "SPECIAL", "SMALLMED"])
    } else if root.join("tiles/SC2000_DAT").is_dir() {
        (root.join("tiles/SC2000_DAT"), &["large", "other", "small"])
    } else if root.join("large/atlas.json").is_file() {
        (root.to_path_buf(), &["large", "other", "small"])
    } else {
        bail!(
            "{} is not a supported extracted Mac, Win95, DOS/GOG, or SCURK graphics pack",
            root.display()
        );
    };

    preferred
        .iter()
        .map(|name| AtlasSource::load(sprite_root.join(name)))
        .collect()
}

/// Private UI namespace, separate from the world sprite and traffic banks.
pub const NEWSPAPER_PICTURE_BASE: u32 = 70_000;

/// The imported editions expose the same twelve editorial pictures in different folders.
/// Keep their UI IDs separate from road-traffic masks, which also use IDs 400..411.
pub fn load_newspaper_pictures(roots: &[PathBuf]) -> Result<Option<AtlasSource>> {
    let mut atlas = RgbaImage::new(640, 312);
    let mut frames = Vec::new();
    for picture in 400..=411u32 {
        let path = roots.iter().find_map(|root| {
            [
                root.join(format!("pict/PICT_{picture:05}.png")),
                root.join(format!("bitmaps_png/{picture}.png")),
            ]
            .into_iter()
            .find(|path| path.is_file())
        });
        let Some(path) = path else { continue };
        let image = image::open(&path)
            .with_context(|| format!("loading newspaper picture {}", path.display()))?
            .to_rgba8();
        let image = if image.width() > 156 || image.height() > 100 {
            image::imageops::thumbnail(&image, 156, 100)
        } else {
            image
        };
        let index = picture - 400;
        let (x, y) = ((index % 4) * 160 + 2, (index / 4) * 104 + 2);
        image::imageops::overlay(&mut atlas, &image, i64::from(x), i64::from(y));
        frames.push(AtlasFrame {
            index,
            id: NEWSPAPER_PICTURE_BASE + picture,
            file: path.to_string_lossy().into_owned(),
            source_offset: 0,
            x,
            y,
            w: image.width(),
            h: image.height(),
            logical_size: None,
            anchor: None,
        });
    }
    Ok((!frames.is_empty()).then(|| AtlasSource::from_rgba("newspaper_pictures", atlas, frames)))
}

/// Project-authored high-density toolbar sheets are used with every original game edition.
pub fn load_ui_palette_sources(_asset_root: impl AsRef<Path>) -> Result<Vec<AtlasSource>> {
    fn bundled(name: &str, bytes: &[u8]) -> Result<AtlasSource> {
        let image = image::load_from_memory(bytes)
            .with_context(|| format!("decoding bundled {name} toolbar artwork"))?
            .to_rgba8();
        let (w, h) = image.dimensions();
        Ok(AtlasSource::from_rgba(
            name,
            image,
            vec![AtlasFrame {
                file: format!("{name}.png"),
                index: 0,
                id: 1,
                source_offset: 0,
                x: 0,
                y: 0,
                w,
                h,
                logical_size: None,
                anchor: None,
            }],
        ))
    }

    fn light_variant(source: AtlasSource, name: &str) -> AtlasSource {
        let mut image = source
            .decoded_image
            .expect("bundled toolbar has decoded image");
        for pixel in image.pixels_mut() {
            let [r, g, b, a] = pixel.0;
            // Recolor only the palette's muted blue/teal chrome. Saturated tool
            // symbols, highlights, and near-black outlines retain their artwork.
            if a > 0 && r >= 18 && r < 150 && g > r + 6 && b > g + 4 && b < 180 {
                let luminance = (u16::from(r) * 3 + u16::from(g) * 6 + u16::from(b)) / 10;
                let shade = (236i32 - (i32::from(luminance) - 35) * 2 / 3).clamp(166, 242) as u8;
                pixel.0 = [shade.saturating_sub(12), shade.saturating_sub(5), shade, a];
            }
        }
        AtlasSource::from_rgba(name, image, source.manifest.frames)
    }

    let city = bundled(
        "citygame_city_toolbar",
        include_bytes!("../assets/ui/citygame-toolbar/city.png"),
    )?;
    let edit = bundled(
        "citygame_edit_toolbar",
        include_bytes!("../assets/ui/citygame-toolbar/edit-map.png"),
    )?;
    let city_light = light_variant(city.clone(), "citygame_city_toolbar_light");
    let edit_light = light_variant(edit.clone(), "citygame_edit_toolbar_light");
    Ok(vec![
        city,
        edit,
        bundled(
            "classic_city_toolbar",
            include_bytes!("../assets/ui/citygame-toolbar/city-classic.png"),
        )?,
        bundled(
            "classic_edit_toolbar",
            include_bytes!("../assets/ui/citygame-toolbar/edit-map-classic.png"),
        )?,
        city_light,
        edit_light,
    ])
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeReplacementPack {
    schema_version: u32,
    id: String,
    name: String,
    kind: String,
    author: String,
    license: String,
    assets: Vec<RuntimeReplacementAsset>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeReplacementAsset {
    key: String,
    file: String,
    status: String,
    #[serde(default)]
    anchor: Option<[f32; 2]>,
    #[serde(default)]
    logical_size: Option<[u32; 2]>,
    #[serde(default)]
    animation: Option<String>,
}

/// Build one in-memory atlas from reviewed sprite entries in an authored pack.
/// Draft entries remain visible in the asset browser but cannot affect live gameplay.
pub fn load_replacement_sprite_source(root: impl AsRef<Path>) -> Result<Option<AtlasSource>> {
    // A slightly wider replacement atlas keeps the growing reviewed 8x sprite
    // set below the 16k texture-height limit while remaining under 8k on either axis.
    const MAX_WIDTH: u32 = 6656;
    const MAX_ATLAS_HEIGHT: u32 = 16_384;
    let root = root
        .as_ref()
        .canonicalize()
        .with_context(|| format!("opening replacement pack {}", root.as_ref().display()))?;
    let manifest_path = root.join("asset-pack.json");
    let pack: RuntimeReplacementPack = serde_json::from_slice(&fs::read(&manifest_path)?)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    ensure!(
        pack.schema_version == 1 && pack.kind == "replacement",
        "expected schema_version 1 and kind replacement"
    );
    ensure!(
        !pack.id.trim().is_empty()
            && !pack.name.trim().is_empty()
            && !pack.author.trim().is_empty()
            && !pack.license.trim().is_empty(),
        "replacement pack metadata is incomplete"
    );

    let mut seen = BTreeSet::new();
    let mut sprites = Vec::new();
    for entry in pack.assets {
        ensure!(
            seen.insert(entry.key.clone()),
            "duplicate replacement key {}",
            entry.key
        );
        ensure!(
            matches!(entry.status.as_str(), "draft" | "reviewed"),
            "status must be draft or reviewed"
        );
        if entry.status != "reviewed" || !entry.key.starts_with("citygame.sprite.") {
            continue;
        }
        ensure!(
            entry
                .animation
                .as_ref()
                .is_some_and(|note| !note.trim().is_empty()),
            "reviewed sprite {} requires an animation note",
            entry.key
        );
        let anchor = entry
            .anchor
            .filter(|values| values.iter().all(|value| value.is_finite()))
            .with_context(|| format!("reviewed sprite {} requires a finite anchor", entry.key))?;
        if let Some([w, h]) = entry.logical_size {
            ensure!(
                w > 0 && h > 0,
                "logical_size must be positive for {}",
                entry.key
            );
        }
        let id = entry.key["citygame.sprite.".len()..]
            .parse::<u32>()
            .with_context(|| format!("invalid sprite key {}", entry.key))?;
        let relative = Path::new(&entry.file);
        ensure!(
            !relative.is_absolute()
                && !relative
                    .components()
                    .any(|component| matches!(component, Component::ParentDir)),
            "replacement file must stay inside its pack"
        );
        let path = root.join(relative).canonicalize().with_context(|| {
            format!(
                "opening replacement sprite {}",
                root.join(relative).display()
            )
        })?;
        ensure!(
            path.starts_with(&root),
            "replacement file must stay inside its pack"
        );
        ensure!(
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png")),
            "replacement sprite {} must be a PNG",
            entry.key
        );
        let image = image::open(&path)
            .with_context(|| format!("reading replacement sprite {}", path.display()))?
            .to_rgba8();
        ensure!(
            image.width() <= MAX_WIDTH && image.height() <= MAX_ATLAS_HEIGHT,
            "replacement sprite {} is too large",
            entry.key
        );
        sprites.push((id, entry.file, anchor, entry.logical_size, image));
    }
    if sprites.is_empty() {
        return Ok(None);
    }
    sprites.sort_by_key(|(id, _, _, _, _)| *id);

    // Pack tall sprites together to avoid wasting a short row's height on a
    // single large building. Keep the placement vector in sprite-ID order so
    // frame indices and lookups remain stable.
    let mut pack_order: Vec<usize> = (0..sprites.len()).collect();
    pack_order.sort_by_key(|&index| {
        let image = &sprites[index].4;
        (
            std::cmp::Reverse(image.height()),
            std::cmp::Reverse(image.width()),
        )
    });

    let mut cursor_x = 0;
    let mut cursor_y = 0;
    let mut row_height = 0;
    let mut width = 0;
    let mut placements = vec![None; sprites.len()];
    for index in pack_order {
        let (id, file, anchor, logical_size, image) = &sprites[index];
        if cursor_x > 0 && cursor_x + image.width() > MAX_WIDTH {
            cursor_x = 0;
            cursor_y += row_height;
            row_height = 0;
        }
        placements[index] = Some(AtlasFrame {
            file: file.clone(),
            index: index as u32,
            id: *id,
            source_offset: 0,
            x: cursor_x,
            y: cursor_y,
            w: image.width(),
            h: image.height(),
            logical_size: *logical_size,
            anchor: Some(*anchor),
        });
        cursor_x += image.width();
        row_height = row_height.max(image.height());
        width = width.max(cursor_x);
    }
    let placements: Vec<AtlasFrame> = placements
        .into_iter()
        .map(|placement| placement.expect("every replacement sprite is packed"))
        .collect();
    let height = (cursor_y + row_height).max(1);
    ensure!(
        height <= MAX_ATLAS_HEIGHT,
        "replacement atlas height {height} exceeds the {MAX_ATLAS_HEIGHT}-pixel GPU texture limit; use a wider atlas or split the pack"
    );
    let mut atlas = RgbaImage::from_pixel(width.max(1), height, Rgba([0, 0, 0, 0]));
    for ((_, _, _, _, sprite), frame) in sprites.iter().zip(&placements) {
        image::imageops::overlay(&mut atlas, sprite, i64::from(frame.x), i64::from(frame.y));
    }
    Ok(Some(AtlasSource::from_rgba(
        format!("replacement_{}", pack.id),
        atlas,
        placements,
    )))
}

#[cfg(test)]
mod tests;
