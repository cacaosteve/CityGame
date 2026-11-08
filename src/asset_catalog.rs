//! Edition-independent inventory. This does not change the game's asset resolution.
//! Unknown contracts stay unknown: matching numeric IDs is not proof of compatible art.

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub packs: Vec<Pack>,
    pub assets: Vec<Asset>,
    pub warnings: Vec<String>,
    #[serde(skip)]
    pub media: Vec<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub format: String,
    pub kind: String,
    pub root: PathBuf,
    pub author: Option<String>,
    pub license: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Asset {
    pub key: String,
    pub name: String,
    pub category: String,
    pub family: String,
    pub zoom: Option<String>,
    pub sprite_id: Option<u32>,
    pub footprint: Option<[u8; 2]>,
    pub animation: String,
    pub contract_notes: Vec<String>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Serialize)]
pub struct Variant {
    pub pack_id: String,
    pub file: String,
    pub media_url: String,
    pub media_kind: String,
    pub size: Option<[u32; 2]>,
    pub crop: Option<[u32; 4]>,
    pub source_id: Option<u32>,
    pub source_record: Option<usize>,
    pub source_offset: Option<u32>,
    pub status: String,
    pub anchor: Option<[f32; 2]>,
    pub logical_size: Option<[u32; 2]>,
    pub animation: Option<String>,
}

#[derive(Deserialize)]
struct ImportAtlas {
    image: String,
    frames: Vec<ImportFrame>,
}

#[derive(Deserialize)]
struct ImportFrame {
    id: u32,
    #[serde(default)]
    source_offset: Option<u32>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// Portable authoring manifest. Artwork stays in its pack, never in the catalog.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplacementPack {
    schema_version: u32,
    id: String,
    name: String,
    kind: String,
    author: String,
    license: String,
    assets: Vec<ReplacementAsset>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplacementAsset {
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

impl Catalog {
    pub fn load(roots: &[PathBuf]) -> Result<Self> {
        let mut catalog = Self {
            schema_version: 1,
            packs: vec![],
            assets: vec![],
            warnings: vec![],
            media: vec![],
        };
        let mut assets = BTreeMap::new();
        let mut replacements = vec![];
        let mut seen = BTreeSet::new();
        for root in roots {
            let root = root
                .canonicalize()
                .with_context(|| format!("opening pack {}", root.display()))?;
            ensure!(
                root.is_dir(),
                "pack must be a directory: {}",
                root.display()
            );
            if !seen.insert(root.clone()) {
                continue;
            }
            if root.join("asset-pack.json").is_file() {
                replacements.push(root);
            } else {
                catalog.import_extracted(&root, &mut assets)?;
            }
        }
        // Resolve replacement keys after all imports, independent of argument order.
        for root in replacements {
            catalog.import_replacement(&root, &mut assets)?;
        }
        catalog.assets = assets.into_values().collect();
        Ok(catalog)
    }

    fn media_url(&mut self, path: PathBuf) -> String {
        let index = self.media.len();
        self.media.push(path);
        format!("/media/{index}")
    }

    fn add_pack(&mut self, pack: Pack) -> Result<()> {
        ensure!(
            !self.packs.iter().any(|p| p.id == pack.id),
            "duplicate pack ID: {}",
            pack.id
        );
        self.packs.push(pack);
        Ok(())
    }

    fn import_extracted(
        &mut self,
        root: &Path,
        assets: &mut BTreeMap<String, Asset>,
    ) -> Result<()> {
        let (format, name, atlas_dirs, loose_dirs, notes) = if root.join("sprites").is_dir() {
            (
                "mac68k",
                "Macintosh 68k",
                vec!["sprites"],
                vec![
                    "pict",
                    "icons",
                    "cursors",
                    "ui",
                    "palettes",
                    "catalog/sounds",
                ],
                vec![
                    "Mac sound resources are normalized to WAV and MIDI by the importer.",
                    "Previews show extracted pixels; palette cycling is not played in this browser.",
                ],
            )
        } else if root.join("win95_tiles").is_dir() {
            (
                "win95",
                "Windows 95",
                vec!["win95_tiles"],
                vec!["bitmaps_png", "pcx_png", "catalog/sounds", "palettes"],
                vec!["WAV effects can be auditioned. MIDI files are inventoried, not synthesized."],
            )
        } else if root.join("tiles/SC2000_DAT").is_dir() {
            (
                "dos",
                "DOS / GOG export",
                vec!["tiles/SC2000_DAT"],
                vec!["pcx_png", "palettes", "catalog/sounds"],
                vec![
                    "Only the SC2000.DAT base set is counted here. Add an extracted SCURK tile-set folder separately to compare it.",
                    "DOS VOC and XMI resources are normalized to WAV and MIDI by the importer.",
                ],
            )
        } else if root.join("large/atlas.json").is_file() {
            (
                "dos-tileset",
                "DOS alternate tile set",
                vec!["."],
                vec![],
                vec![
                    "Alternate tile set: matching IDs are candidates for comparison, not verified equivalent artwork.",
                ],
            )
        } else {
            bail!(
                "Unrecognized extracted pack: {}. Choose a Mac sprites/, Win95 win95_tiles/, DOS tiles/SC2000_DAT export, or a replacement folder containing asset-pack.json. Raw CDs and installers need extraction first.",
                root.display()
            );
        };
        let suffix = root.file_name().unwrap_or_default().to_string_lossy();
        let base_id = format!("{format}:{}", suffix);
        let mut pack_id = base_id.clone();
        let mut suffix_number = 2;
        while self.packs.iter().any(|pack| pack.id == pack_id) {
            pack_id = format!("{base_id}-{suffix_number}");
            suffix_number += 1;
        }
        self.add_pack(Pack {
            id: pack_id.clone(),
            name: name.into(),
            format: format.into(),
            kind: "imported".into(),
            root: root.into(),
            author: None,
            license: None,
            notes: notes.into_iter().map(String::from).collect(),
        })?;
        let mut pack_keys = BTreeSet::new();
        for directory in atlas_dirs {
            let files = files_below(&root.join(directory))?;
            for file in files
                .into_iter()
                .filter(|p| p.file_name().is_some_and(|n| n == "atlas.json"))
            {
                let atlas: ImportAtlas = serde_json::from_slice(&fs::read(&file)?)
                    .with_context(|| format!("reading {}", file.display()))?;
                let atlas_path = contained_file(root, &file.parent().unwrap().join(&atlas.image))?;
                let dimensions = image::image_dimensions(&atlas_path)
                    .with_context(|| format!("reading image {}", atlas_path.display()))?;
                let media_url = self.media_url(atlas_path.clone());
                for (record_index, frame) in atlas.frames.into_iter().enumerate() {
                    let key = sprite_key(frame.id);
                    if frame.w == 0
                        || frame.h == 0
                        || frame
                            .x
                            .checked_add(frame.w)
                            .is_none_or(|x| x > dimensions.0)
                        || frame
                            .y
                            .checked_add(frame.h)
                            .is_none_or(|y| y > dimensions.1)
                    {
                        self.warnings.push(format!(
                            "{pack_id}: invalid atlas rectangle for {key}; skipped"
                        ));
                        continue;
                    }
                    let duplicate = !pack_keys.insert(key.clone());
                    let asset = assets.entry(key).or_insert_with(|| sprite_asset(frame.id));
                    let duplicate_note = "Multiple source records share this slot. Resolve record selection before replacement approval.";
                    if duplicate
                        && !asset
                            .contract_notes
                            .iter()
                            .any(|note| note == duplicate_note)
                    {
                        asset.contract_notes.push(duplicate_note.into());
                    }
                    asset.variants.push(Variant {
                        pack_id: pack_id.clone(),
                        file: relative(root, &atlas_path),
                        media_url: media_url.clone(),
                        media_kind: "image".into(),
                        size: Some([frame.w, frame.h]),
                        crop: Some([frame.x, frame.y, frame.w, frame.h]),
                        source_id: Some(frame.id),
                        source_record: Some(record_index),
                        source_offset: frame.source_offset,
                        status: "imported".into(),
                        anchor: None,
                        logical_size: None,
                        animation: None,
                    });
                }
            }
        }
        for directory in loose_dirs {
            for file in files_below(&root.join(directory))? {
                let ext = extension(&file);
                if !matches!(ext.as_str(), "png" | "wav" | "mid" | "midi") {
                    continue;
                }
                let rel = relative(root, &file);
                let source_id = file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse::<u32>().ok());
                let (key, category, media_kind, name) = match ext.as_str() {
                    "wav" => (
                        source_id
                            .map(sound_key)
                            .unwrap_or_else(|| format!("{format}.sound.{rel}")),
                        "Sound effects",
                        "audio",
                        source_id.map(sound_name).unwrap_or_else(|| rel.clone()),
                    ),
                    "mid" | "midi" => (
                        source_id
                            .map(|id| format!("citygame.music.{id}"))
                            .unwrap_or_else(|| format!("{format}.music.{rel}")),
                        "Music",
                        "midi",
                        file.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                    ),
                    _ => (
                        format!("{format}.image.{rel}"),
                        if directory == "palettes" {
                            "Palettes"
                        } else if directory == "cursors" {
                            "Cursors"
                        } else {
                            "Interface"
                        },
                        "image",
                        rel.clone(),
                    ),
                };
                ensure!(
                    pack_keys.insert(key.clone()),
                    "ambiguous duplicate asset {key} in {pack_id}"
                );
                let file = contained_file(root, &file)?;
                let size = if media_kind == "image" {
                    match image::image_dimensions(&file) {
                        Ok((w, h)) => Some([w, h]),
                        Err(error) => {
                            self.warnings
                                .push(format!("{pack_id}: {rel}: {error}; skipped"));
                            continue;
                        }
                    }
                } else {
                    None
                };
                let media_url = self.media_url(file);
                let asset = assets.entry(key.clone()).or_insert_with(|| Asset {
                    key: key.clone(), name, category: category.into(), family: key, zoom: None,
                    sprite_id: None, footprint: None, animation: "Not specified".into(),
                    contract_notes: vec![if media_kind == "image" {
                        "Placement / cursor hotspot needs review; dimensions alone do not define the contract.".into()
                    } else { "Duration, gain, looping and playback contract need review.".into() }], variants: vec![],
                });
                asset.variants.push(Variant {
                    pack_id: pack_id.clone(),
                    file: rel,
                    media_url,
                    media_kind: media_kind.into(),
                    size,
                    crop: None,
                    source_id,
                    source_record: None,
                    source_offset: None,
                    status: "imported".into(),
                    anchor: None,
                    logical_size: None,
                    animation: None,
                });
            }
        }
        ensure!(
            !pack_keys.is_empty(),
            "No supported assets found in {}",
            root.display()
        );
        Ok(())
    }

    fn import_replacement(
        &mut self,
        root: &Path,
        assets: &mut BTreeMap<String, Asset>,
    ) -> Result<()> {
        let manifest: ReplacementPack =
            serde_json::from_slice(&fs::read(root.join("asset-pack.json"))?)
                .with_context(|| format!("reading replacement manifest in {}", root.display()))?;
        ensure!(
            manifest.schema_version == 1 && manifest.kind == "replacement",
            "expected schema_version 1 and kind replacement"
        );
        ensure!(
            !manifest.id.trim().is_empty() && !manifest.name.trim().is_empty(),
            "replacement pack ID and name are required"
        );
        ensure!(
            !manifest.author.trim().is_empty() && !manifest.license.trim().is_empty(),
            "replacement author and license are required"
        );
        let pack_id = manifest.id.clone();
        self.add_pack(Pack {
            id: pack_id.clone(), name: manifest.name, format: "asset-pack-v1".into(), kind: manifest.kind,
            root: root.into(), author: Some(manifest.author), license: Some(manifest.license),
            notes: vec!["Review status is author-declared. This catalog does not activate replacements in the game.".into()],
        })?;
        let mut keys = BTreeSet::new();
        for entry in manifest.assets {
            ensure!(
                keys.insert(entry.key.clone()),
                "duplicate replacement key {}",
                entry.key
            );
            ensure!(
                matches!(entry.status.as_str(), "draft" | "reviewed"),
                "status must be draft or reviewed"
            );
            let asset = assets.get_mut(&entry.key).with_context(|| {
                format!(
                    "unknown replacement key {}; load an original reference pack first",
                    entry.key
                )
            })?;
            let relative_path = Path::new(&entry.file);
            ensure!(
                !relative_path.is_absolute()
                    && !relative_path
                        .components()
                        .any(|c| matches!(c, Component::ParentDir)),
                "replacement file must stay inside its pack"
            );
            let path = contained_file(root, &root.join(relative_path))?;
            let ext = extension(&path);
            let media_kind = match ext.as_str() {
                "png" => "image",
                "wav" => "audio",
                "mid" | "midi" => "midi",
                _ => bail!("unsupported replacement file {}", entry.file),
            };
            ensure!(
                asset.variants.iter().all(|v| v.media_kind == media_kind),
                "replacement media type mismatch for {}",
                entry.key
            );
            let size = if media_kind == "image" {
                let (w, h) = image::image_dimensions(&path)?;
                Some([w, h])
            } else {
                None
            };
            if let Some(anchor) = entry.anchor {
                ensure!(
                    anchor.iter().all(|v| v.is_finite()),
                    "anchor must be finite"
                );
            }
            if let Some([w, h]) = entry.logical_size {
                ensure!(w > 0 && h > 0, "logical_size must be positive");
            }
            if entry.status == "reviewed" {
                ensure!(
                    entry
                        .animation
                        .as_ref()
                        .is_some_and(|v| !v.trim().is_empty()),
                    "reviewed assets require an explicit animation / playback note"
                );
                ensure!(
                    media_kind != "image" || entry.anchor.is_some(),
                    "reviewed images require an explicit pixel anchor"
                );
            }
            let media_url = self.media_url(path);
            asset.variants.push(Variant {
                pack_id: pack_id.clone(),
                file: entry.file,
                media_url,
                media_kind: media_kind.into(),
                size,
                crop: None,
                source_id: None,
                source_record: None,
                source_offset: None,
                status: entry.status,
                anchor: entry.anchor,
                logical_size: entry.logical_size,
                animation: entry.animation,
            });
        }
        Ok(())
    }

    /// Deliberately omit original pixels, filenames, offsets, source roots and pack provenance.
    pub fn artist_brief(&self) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "purpose": "Technical authoring brief. Unknown contracts need specification before final artwork approval.",
            "coverage_basis": "Observed imported assets, not an audited list of every asset required by the game.",
            "assets": self.assets.iter().map(|a| {
                let sizes: BTreeSet<_> = a.variants.iter().filter(|v| v.status == "imported").filter_map(|v| v.size).collect();
                serde_json::json!({"key": a.key, "purpose": a.name, "category": a.category,
                    "family": a.family, "zoom": a.zoom, "footprint_tiles": a.footprint,
                    "observed_pixel_sizes": sizes, "animation_contract": a.animation,
                    "open_questions": a.contract_notes})
            }).collect::<Vec<_>>()
        })
    }
}

fn contained_file(root: &Path, path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("reading {}", path.display()))?;
    ensure!(
        canonical.starts_with(root) && canonical.is_file(),
        "asset escapes its pack or is not a file: {}",
        path.display()
    );
    Ok(canonical)
}

fn files_below(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut files = vec![];
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        // Never follow directory symlinks into an unrelated tree or a cycle.
        if kind.is_dir() {
            files.extend(files_below(&entry.path())?);
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn extension(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase()
}
pub fn sprite_key(id: u32) -> String {
    format!("citygame.sprite.{id:05}")
}
fn sound_key(id: u32) -> String {
    format!("citygame.sound.{id:03}")
}

fn sound_name(id: u32) -> String {
    let name = match id {
        500 => "Construction",
        502 => "Wind",
        503 => "Placement",
        504 => "Explosion",
        505 => "UI click",
        506 => "Police",
        507 => "Fire",
        509 => "Fire truck",
        511 => "Flood",
        512 => "Riot",
        513 => "Cheers",
        514 => "Electrical zap",
        515 => "Mayday",
        517 => "Cargo",
        520 => "Disaster alert",
        521 => "Horns",
        522 => "Prison",
        523 => "School",
        524 => "Train",
        525 => "Military",
        527 => "Monster",
        _ => return format!("Sound {id} · purpose needs review"),
    };
    format!("{name} · {id}")
}

fn sprite_asset(id: u32) -> Asset {
    let base = id % 500;
    let (category, title): (&str, String) = match base {
        0..=13 => (
            "Terrain & nature",
            match base {
                0 => "Clear tile",
                1..=5 => "Rubble",
                6..=12 => "Trees",
                _ => "Small park",
            }
            .into(),
        ),
        14..=28 => ("Utilities", "Power lines".into()),
        29..=43 => ("Roads & rail", "Road".into()),
        44..=62 => ("Roads & rail", "Rail".into()),
        63..=111 => ("Roads & rail", "Infrastructure / crossing".into()),
        112..=197 => ("Buildings", "Development / construction".into()),
        198..=255 => ("Buildings", building_name(base).into()),
        256..=269 => ("Terrain & nature", "Terrain shape".into()),
        270..=289 => ("Water", "Water / shoreline".into()),
        291..=299 => ("Overlays", "Zone overlay".into()),
        400..=449 => ("Traffic", "Traffic overlay".into()),
        _ => ("Needs mapping", "Unmapped shape".into()),
    };
    let zoom = match id {
        0..=499 => Some("Small"),
        500..=999 => Some("Medium"),
        1000..=1499 => Some("Large"),
        _ => None,
    };
    let animation = if crate::palette::is_water_sprite(id) {
        "Water palette cycle (current renderer, 4 fps)"
    } else if crate::palette::is_smoke_sprite(id) {
        "Smoke palette cycle (current renderer, 4 fps)"
    } else if crate::palette::is_traffic_sprite(id) {
        "Traffic behavior depends on engine / shape; review required"
    } else {
        "Not audited"
    };
    Asset {
        key: sprite_key(id), name: format!("{title} · {base}"), category: category.into(),
        family: format!("citygame.shape.{base:03}"), zoom: zoom.map(String::from), sprite_id: Some(id),
        footprint: if base <= 255 { let n = crate::city::multi_tile_building_size(base as u8); Some([n,n]) } else { None },
        animation: animation.into(),
        contract_notes: vec![
            "Pixel anchor, drawing layer and masks need a per-family contract review.".into(),
            "Shape families group observed zoom IDs. Rotation / connection dependencies are not yet a complete replacement contract.".into(),
            "Cross-edition correspondence is by numeric ID; semantic equivalence still needs review.".into(),
        ], variants: vec![],
    }
}

fn building_name(id: u32) -> &'static str {
    match id {
        198 | 199 => "Hydro power",
        200 => "Wind power",
        201 => "Gas power",
        202 => "Oil power",
        203 => "Nuclear power",
        204 => "Solar power",
        205 => "Microwave power",
        206 => "Fusion power",
        207 => "Coal power",
        208 => "City hall",
        209 => "Hospital",
        210 => "Police station",
        211 => "Fire station",
        212 => "Museum",
        213 => "Large park",
        214 => "School",
        215 => "Stadium",
        216 => "Prison",
        217 => "College",
        218 => "Zoo",
        219 => "Statue",
        220 => "Water pump",
        221 => "Runway",
        222 => "Runway intersection",
        223 => "Seaport pier",
        224 => "Crane",
        225 => "Control tower",
        226 => "Military control tower",
        227 => "Warehouse",
        228 | 229 => "Airport building",
        230 => "Tarmac",
        231 => "Military aircraft",
        232 => "Military hangar",
        233 => "Subway station",
        234 => "Radar",
        235 => "Water tower",
        236 => "Bus depot",
        237 => "Rail depot",
        238 => "Parking lot",
        239 => "Military parking lot",
        240 => "Loading bay",
        241 => "Military facility",
        242 => "Cargo yard",
        243 => "Mayor's house",
        244 => "Water treatment",
        245 => "Library",
        246 => "Large hangar",
        247 => "Church",
        248 => "Marina",
        249 => "Missile silo",
        250 => "Desalination",
        251..=254 => "Arcology",
        255 => "Dome",
        _ => "Building",
    }
}

#[cfg(test)]
mod tests;
