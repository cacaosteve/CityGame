use crate::assets::{load_replacement_sprite_source, load_sprite_sources, load_ui_palette_sources};
use crate::city::City;
use crate::palette::AnimatedPalette;
use crate::paths;
use crate::win95_import;
use crate::{dos_import, mac_import};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const SETTINGS_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GraphicsPreference {
    #[default]
    PreferOriginal,
    PreferReplacement,
}

impl GraphicsPreference {
    pub fn prefers_replacement(self) -> bool {
        self == Self::PreferReplacement
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TitleMusicPreference {
    #[default]
    Original,
    Replacement,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToolbarPalettePreference {
    #[default]
    CityGame,
    Classic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AppearancePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl AppearancePreference {
    pub fn is_dark(self, system_is_dark: bool) -> bool {
        match self {
            Self::System => system_is_dark,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

impl ToolbarPalettePreference {
    pub fn uses_citygame_palette(self) -> bool {
        self == Self::CityGame
    }

    pub fn next(self) -> Self {
        match self {
            Self::CityGame => Self::Classic,
            Self::Classic => Self::CityGame,
        }
    }
}

impl TitleMusicPreference {
    pub fn next(self) -> Self {
        match self {
            Self::Original => Self::Replacement,
            Self::Replacement => Self::Off,
            Self::Off => Self::Original,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackFormat {
    Mac68k,
    Windows95,
    DosGog,
    DosTileSet,
    Replacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginalMedia {
    Graphics,
    Effects,
    Music,
}

impl PackFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mac68k => "Macintosh 68k",
            Self::Windows95 => "Windows 95 / Special Edition CD",
            Self::DosGog => "DOS / GOG / EA-compatible edition",
            Self::DosTileSet => "DOS SCURK tile set",
            Self::Replacement => "Authored replacement",
        }
    }

    fn output_stem(self) -> &'static str {
        match self {
            Self::Mac68k => "mac-68k",
            Self::Windows95 => "windows-95",
            Self::DosGog => "dos-gog",
            Self::DosTileSet => "dos-tileset",
            Self::Replacement => "replacement",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredPack {
    pub format: PackFormat,
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AssetSettings {
    schema_version: u32,
    #[serde(default)]
    original_graphics: Option<PathBuf>,
    #[serde(default)]
    original_effects: Option<PathBuf>,
    #[serde(default)]
    original_music: Option<PathBuf>,
    #[serde(default)]
    replacement_graphics: Option<PathBuf>,
    #[serde(default)]
    graphics_preference: GraphicsPreference,
    #[serde(default)]
    citygame_effects: bool,
    #[serde(default)]
    citygame_city_music: bool,
    #[serde(default)]
    imported_packs: Vec<RegisteredPack>,
    #[serde(default)]
    recent_city: Option<PathBuf>,
    #[serde(default)]
    tile_set_graphics: Option<PathBuf>,
    #[serde(default)]
    title_music_preference: TitleMusicPreference,
    #[serde(default)]
    toolbar_palette_preference: ToolbarPalettePreference,
    #[serde(default)]
    appearance_preference: AppearancePreference,
    #[serde(default = "default_citygame_newspaper_enabled")]
    citygame_newspaper_enabled: bool,
}

fn default_citygame_newspaper_enabled() -> bool {
    true
}

impl Default for AssetSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA,
            original_graphics: None,
            original_effects: None,
            original_music: None,
            replacement_graphics: None,
            graphics_preference: GraphicsPreference::PreferOriginal,
            citygame_effects: false,
            citygame_city_music: false,
            imported_packs: Vec::new(),
            recent_city: None,
            tile_set_graphics: None,
            title_music_preference: TitleMusicPreference::default(),
            toolbar_palette_preference: ToolbarPalettePreference::default(),
            appearance_preference: AppearancePreference::default(),
            citygame_newspaper_enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SoundPaths {
    /// Original-edition WAV files, plus explicit environment overrides.
    pub effects: HashMap<u16, PathBuf>,
    /// Reviewed CityGame WAVs, plus explicit environment overrides.
    pub citygame_effects: HashMap<u16, PathBuf>,
    pub use_citygame_effects: bool,
}

#[derive(Debug)]
pub struct AssetProfile {
    settings: AssetSettings,
    settings_path: PathBuf,
    environment_original: Option<PathBuf>,
    environment_replacement: Option<PathBuf>,
    bundled_replacement: Option<PathBuf>,
}

fn find_bundled_replacement(executable: Option<&Path>, source_root: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(bin_dir) = executable.and_then(Path::parent) {
        if bin_dir.file_name().is_some_and(|name| name == "MacOS") {
            if let Some(contents) = bin_dir.parent() {
                candidates.push(contents.join("Resources/replacements/our-graphics"));
            }
        }
        candidates.push(bin_dir.join("assets/replacements/our-graphics"));
    }
    candidates.push(source_root.join("assets/replacements/our-graphics"));
    candidates
        .into_iter()
        .find(|root| root.join("asset-pack.json").is_file())
}

impl AssetProfile {
    pub fn load() -> Result<Self> {
        Self::load_from(settings_path())
    }

    pub(crate) fn load_from(settings_path: PathBuf) -> Result<Self> {
        let settings = if settings_path.is_file() {
            let bytes = fs::read(&settings_path)
                .with_context(|| format!("reading {}", settings_path.display()))?;
            let mut parsed: AssetSettings = serde_json::from_slice(&bytes)
                .with_context(|| format!("parsing {}", settings_path.display()))?;
            ensure!(
                parsed.schema_version == SETTINGS_SCHEMA,
                "unsupported asset settings schema {}",
                parsed.schema_version
            );
            if let Some(base) = settings_path.parent() {
                if base.join(".citygame-relative-settings").is_file() {
                    absolutize_settings_paths(&mut parsed, base);
                }
            }
            parsed
        } else {
            AssetSettings::default()
        };
        // Desktop builds ship authored artwork; mobile uses its explicitly
        // selected, memory-budgeted seed tiers instead of the desktop 8x pack.
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let bundled_replacement = find_bundled_replacement(
            std::env::current_exe().ok().as_deref(),
            Path::new(env!("CARGO_MANIFEST_DIR")),
        );
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let bundled_replacement = None;
        Ok(Self {
            settings,
            settings_path,
            environment_original: std::env::var_os("SC2000_ASSET_ROOT").map(PathBuf::from),
            environment_replacement: std::env::var_os("SC2000_REPLACEMENT_ROOT").map(PathBuf::from),
            bundled_replacement,
        })
    }

    pub fn original_root(&self) -> Option<PathBuf> {
        self.environment_original
            .clone()
            .or_else(|| self.settings.original_graphics.clone())
            .or_else(|| {
                let root = paths::asset_root();
                root.is_dir().then_some(root)
            })
    }

    pub fn default_city_path(&self) -> Result<PathBuf> {
        let configured = paths::default_city_path();
        let recent = self.recent_city();
        if !configured.is_file() && std::env::var_os("SC2000_DEFAULT_CITY").is_some() {
            log::warn!(
                "SC2000_DEFAULT_CITY points to a missing file ({}); searching imported city data",
                configured.display()
            );
        }
        if let Some(path) = resolve_default_city(
            &configured,
            recent.as_deref(),
            &self.settings.imported_packs,
        ) {
            return Ok(path);
        }

        let path = cities_directory().join("Bootstrap.SC2");
        if City::load(&path).is_err() {
            fs::create_dir_all(path.parent().context("bootstrap city path has no parent")?)
                .with_context(|| format!("creating city directory for {}", path.display()))?;
            let mut city = City::new_blank(&path, "Untitled")?;
            city.save(&path)?;
            log::info!(
                "no original city files are installed; created a blank startup city at {}",
                path.display()
            );
        }
        Ok(path)
    }

    pub fn replacement_root(&self) -> Option<PathBuf> {
        self.environment_replacement
            .clone()
            .or_else(|| self.settings.replacement_graphics.clone())
            .or_else(|| self.bundled_replacement.clone())
    }

    pub fn preference(&self) -> GraphicsPreference {
        self.settings.graphics_preference
    }

    pub fn title_music_preference(&self) -> TitleMusicPreference {
        self.settings.title_music_preference
    }

    pub fn set_title_music_preference(&mut self, preference: TitleMusicPreference) -> Result<()> {
        self.settings.title_music_preference = preference;
        self.save()
    }

    pub fn toolbar_palette_preference(&self) -> ToolbarPalettePreference {
        self.settings.toolbar_palette_preference
    }

    pub fn set_toolbar_palette_preference(
        &mut self,
        preference: ToolbarPalettePreference,
    ) -> Result<()> {
        self.settings.toolbar_palette_preference = preference;
        self.save()
    }

    pub fn appearance_preference(&self) -> AppearancePreference {
        self.settings.appearance_preference
    }

    pub fn set_appearance_preference(&mut self, preference: AppearancePreference) -> Result<()> {
        self.settings.appearance_preference = preference;
        self.save()
    }

    pub fn citygame_newspaper_enabled(&self) -> bool {
        self.settings.citygame_newspaper_enabled
    }

    pub fn set_citygame_newspaper_enabled(&mut self, enabled: bool) -> Result<()> {
        self.settings.citygame_newspaper_enabled = enabled;
        self.save()
    }

    pub fn imported_packs(&self) -> &[RegisteredPack] {
        &self.settings.imported_packs
    }

    pub fn media_source(&self, media: OriginalMedia) -> Option<PathBuf> {
        match media {
            OriginalMedia::Graphics => self.original_root(),
            OriginalMedia::Effects => self
                .settings
                .original_effects
                .clone()
                .filter(|root| has_media(root, media))
                .or_else(|| self.available_sources(media).into_iter().next()),
            OriginalMedia::Music => self
                .settings
                .original_music
                .clone()
                .filter(|root| has_media(root, media))
                .or_else(|| self.available_sources(media).into_iter().next()),
        }
    }

    pub fn available_sources(&self, media: OriginalMedia) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        for pack in &self.settings.imported_packs {
            if pack.format == PackFormat::Replacement || pack.format == PackFormat::DosTileSet {
                continue;
            }
            if has_media(&pack.root, media) && !roots.contains(&pack.root) {
                roots.push(pack.root.clone());
            }
        }
        if let Some(root) = self.original_root() {
            if has_media(&root, media) && !roots.contains(&root) {
                roots.insert(0, root);
            }
        }
        roots
    }

    pub fn ordered_sources(&self, media: OriginalMedia) -> Vec<PathBuf> {
        let mut roots = self.available_sources(media);
        if let Some(selected) = self.media_source(media) {
            roots.retain(|root| root != &selected);
            roots.insert(0, selected);
        }
        roots
    }

    pub fn cycle_source(&mut self, media: OriginalMedia) -> Result<Option<PathBuf>> {
        let sources = self.available_sources(media);
        if sources.is_empty() {
            return Ok(None);
        }
        let current = self.media_source(media);
        let next = current
            .and_then(|root| sources.iter().position(|candidate| candidate == &root))
            .map_or(0, |index| (index + 1) % sources.len());
        let chosen = sources[next].clone();
        match media {
            OriginalMedia::Graphics => self.settings.original_graphics = Some(chosen.clone()),
            OriginalMedia::Effects => self.settings.original_effects = Some(chosen.clone()),
            OriginalMedia::Music => self.settings.original_music = Some(chosen.clone()),
        }
        self.save()?;
        Ok(Some(chosen))
    }

    pub fn citygame_effects_enabled(&self) -> bool {
        self.settings.citygame_effects
    }

    pub fn set_citygame_effects_enabled(&mut self, enabled: bool) -> Result<()> {
        self.settings.citygame_effects = enabled;
        self.save()
    }

    pub fn citygame_city_music_enabled(&self) -> bool {
        self.settings.citygame_city_music
    }

    pub fn set_citygame_city_music_enabled(&mut self, enabled: bool) -> Result<()> {
        self.settings.citygame_city_music = enabled;
        self.save()
    }

    pub fn recent_city(&self) -> Option<PathBuf> {
        self.settings
            .recent_city
            .clone()
            .filter(|path| path.is_file())
    }

    pub fn tile_set_root(&self) -> Option<PathBuf> {
        self.settings.tile_set_graphics.clone()
    }

    pub fn set_tile_set_pack(&mut self, root: impl AsRef<Path>) -> Result<()> {
        let root = canonical_directory(root.as_ref())?;
        ensure!(
            detect_extracted_pack(&root)? == PackFormat::DosTileSet,
            "choose an extracted SCURK tile-set folder containing large/atlas.json"
        );
        ensure!(
            self.original_root().is_some(),
            "choose original graphics before an alternate tile set"
        );
        validate_original_pack(&root)?;
        self.settings.tile_set_graphics = Some(root.clone());
        self.register(PackFormat::DosTileSet, root);
        self.save()
    }

    pub fn clear_tile_set_pack(&mut self) -> Result<()> {
        self.settings.tile_set_graphics = None;
        self.save()
    }

    pub fn remember_city(&mut self, path: &Path) -> Result<()> {
        if !path.is_file() {
            return Ok(());
        }
        self.settings.recent_city = Some(path.canonicalize()?);
        self.save()
    }

    pub fn set_original_pack(&mut self, root: impl AsRef<Path>) -> Result<PackFormat> {
        let root = canonical_directory(root.as_ref())?;
        let format = detect_extracted_pack(&root)?;
        ensure!(
            format != PackFormat::Replacement && format != PackFormat::DosTileSet,
            "choose an extracted original pack for the original graphics source"
        );
        validate_original_pack(&root)?;
        self.settings.original_graphics = Some(root.clone());
        if self.settings.original_effects.is_none() && has_media(&root, OriginalMedia::Effects) {
            self.settings.original_effects = Some(root.clone());
        }
        if self.settings.original_music.is_none() && has_media(&root, OriginalMedia::Music) {
            self.settings.original_music = Some(root.clone());
        }
        self.register(format, root);
        self.save()?;
        Ok(format)
    }

    pub fn set_replacement_pack(&mut self, root: impl AsRef<Path>) -> Result<()> {
        let root = canonical_directory(root.as_ref())?;
        ensure!(
            detect_extracted_pack(&root)? == PackFormat::Replacement,
            "choose a folder containing an authored asset-pack.json"
        );
        let mut roots = self.available_sources(OriginalMedia::Graphics);
        ensure!(
            !roots.is_empty(),
            "choose an original pack before a replacement pack"
        );
        roots.extend(self.available_sources(OriginalMedia::Effects));
        roots.extend(self.available_sources(OriginalMedia::Music));
        roots.push(root.clone());
        crate::asset_catalog::Catalog::load(&roots)?;
        // Empty packs are valid; parsing still verifies metadata and every listed entry.
        let _ = load_replacement_sprite_source(&root)?;
        self.settings.replacement_graphics = Some(root.clone());
        self.register(PackFormat::Replacement, root);
        self.save()
    }

    pub fn set_preference(&mut self, preference: GraphicsPreference) -> Result<()> {
        self.settings.graphics_preference = preference;
        self.save()
    }

    /// Mobile tier selection: set replacement path without packing atlases or
    /// re-validating the full catalog (seed packs are trusted).
    pub fn set_mobile_replacement_selection(
        &mut self,
        root: Option<PathBuf>,
        preference: GraphicsPreference,
    ) -> Result<()> {
        self.settings.replacement_graphics = root;
        self.settings.graphics_preference = preference;
        self.save()
    }

    pub fn import_original(&mut self, source: impl AsRef<Path>) -> Result<(PackFormat, PathBuf)> {
        let source = canonical_directory(source.as_ref())?;
        if let Ok(format) = detect_extracted_pack(&source) {
            ensure!(
                format != PackFormat::Replacement && format != PackFormat::DosTileSet,
                "use Choose Replacement Pack for authored artwork"
            );
            copy_city_catalog(&source, &source)?;
            self.register_imported_pack(format, source.clone())?;
            return Ok((format, source));
        }

        let format = detect_install_source(&source)?;
        let output = next_import_directory(format)?;
        let import_result = match format {
            PackFormat::Mac68k => {
                mac_import::import(&source, &output).context("extracting Macintosh 68k assets")
            }
            PackFormat::DosGog => {
                dos_import::import(&source, &output).context("extracting DOS / GOG assets")
            }
            PackFormat::Windows95 => win95_import::import(&source, &output)
                .context("extracting Windows 95 / Special Edition assets"),
            _ => bail!("{} is not a raw install format", format.label()),
        };
        if let Err(error) = import_result {
            let _ = fs::remove_dir_all(&output);
            return Err(error);
        }
        if format != PackFormat::Windows95 {
            copy_city_catalog(&source, &output)?;
        }
        let extracted_format = detect_extracted_pack(&output)?;
        self.register_imported_pack(extracted_format, output.clone())?;
        ensure!(
            extracted_format == format,
            "imported pack format changed unexpectedly"
        );
        Ok((format, output))
    }

    pub fn sound_paths(&self) -> SoundPaths {
        let mut effects = HashMap::new();
        let roots = self.ordered_sources(OriginalMedia::Effects);
        for id in 500..=531 {
            for root in &roots {
                let path = root.join(format!("catalog/sounds/{id}.WAV"));
                if path.is_file() {
                    effects.insert(id, path);
                    break;
                }
            }
        }
        let use_citygame_effects = self.citygame_effects_enabled();
        let mut citygame_effects = HashMap::new();
        if use_citygame_effects {
            if let Some(root) = self.replacement_root() {
                citygame_effects = reviewed_effects(&root);
                for (&id, path) in &citygame_effects {
                    effects.insert(id, path.clone());
                }
            }
        }
        for (id, env) in [
            (505, "SC2000_CLICK_SOUND"),
            (500, "SC2000_BUILD_SOUND"),
            (514, "SC2000_POWER_LINE_SOUND"),
        ] {
            if let Some(path) = std::env::var_os(env).map(PathBuf::from) {
                effects.insert(id, path.clone());
                citygame_effects.insert(id, path);
            }
        }
        SoundPaths {
            effects,
            citygame_effects,
            use_citygame_effects,
        }
    }

    pub fn original_music_path(&self, id: u16) -> Option<PathBuf> {
        self.ordered_sources(OriginalMedia::Music)
            .into_iter()
            .map(|root| root.join(format!("catalog/sounds/{id}.MID")))
            .find(|path| path.is_file())
    }

    fn register_imported_pack(&mut self, format: PackFormat, root: PathBuf) -> Result<()> {
        ensure!(
            format != PackFormat::DosTileSet && format != PackFormat::Replacement,
            "use Choose Alternate Tile Set or Choose Replacement Pack for this folder"
        );
        validate_original_pack(&root)?;
        if self.settings.original_graphics.is_none() {
            self.settings.original_graphics = Some(root.clone());
        }
        if self.settings.original_effects.is_none() && has_media(&root, OriginalMedia::Effects) {
            self.settings.original_effects = Some(root.clone());
        }
        if self.settings.original_music.is_none() && has_media(&root, OriginalMedia::Music) {
            self.settings.original_music = Some(root.clone());
        }
        self.register(format, root);
        self.save()
    }

    fn register(&mut self, format: PackFormat, root: PathBuf) {
        if !self
            .settings
            .imported_packs
            .iter()
            .any(|pack| pack.root == root)
        {
            self.settings
                .imported_packs
                .push(RegisteredPack { format, root });
        }
    }

    fn save(&self) -> Result<()> {
        let parent = self
            .settings_path
            .parent()
            .context("asset settings path has no parent")?;
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        let temp = self.settings_path.with_extension("json.tmp");
        fs::write(&temp, serde_json::to_vec_pretty(&self.settings)?)
            .with_context(|| format!("writing {}", temp.display()))?;
        fs::rename(&temp, &self.settings_path)
            .with_context(|| format!("installing {}", self.settings_path.display()))?;
        Ok(())
    }
}

pub fn detect_extracted_pack(root: &Path) -> Result<PackFormat> {
    if root.join("asset-pack.json").is_file() {
        Ok(PackFormat::Replacement)
    } else if root.join("sprites").is_dir() {
        Ok(PackFormat::Mac68k)
    } else if root.join("win95_tiles").is_dir() {
        Ok(PackFormat::Windows95)
    } else if root.join("tiles/SC2000_DAT").is_dir() {
        Ok(PackFormat::DosGog)
    } else if root.join("large/atlas.json").is_file() {
        Ok(PackFormat::DosTileSet)
    } else {
        bail!(
            "{} is not a recognized extracted asset pack",
            root.display()
        )
    }
}

pub fn validate_original_pack(root: &Path) -> Result<()> {
    let mut sources = load_sprite_sources(root)?;
    sources.extend(load_ui_palette_sources(root)?);
    for source in sources {
        let image = source.load_rgba()?;
        for frame in &source.manifest.frames {
            ensure!(
                frame.w > 0
                    && frame.h > 0
                    && frame
                        .x
                        .checked_add(frame.w)
                        .is_some_and(|x| x <= image.width())
                    && frame
                        .y
                        .checked_add(frame.h)
                        .is_some_and(|y| y <= image.height()),
                "invalid sprite rectangle for {} in {}",
                frame.id,
                source.name
            );
        }
    }
    AnimatedPalette::load(root)?;
    Ok(())
}

pub fn detect_install_source(root: &Path) -> Result<PackFormat> {
    if ["PICT", "SPRT", "pltt", "clut", "CURS"]
        .iter()
        .any(|name| find_case_insensitive_path(root, &[*name]).is_some())
    {
        return Ok(PackFormat::Mac68k);
    }
    if (find_case_insensitive_path(root, &["SC2K", "DATA", "LARGE.DAT"])
        .is_some_and(|path| path.is_file())
        && find_case_insensitive_path(root, &["SC2K", "BITMAPS", "PAL_MSTR.BMP"])
            .is_some_and(|path| path.is_file()))
        || (find_case_insensitive_path(root, &["DATA", "LARGE.DAT"])
            .is_some_and(|path| path.is_file())
            && find_case_insensitive_path(root, &["BITMAPS", "PAL_MSTR.BMP"])
                .is_some_and(|path| path.is_file()))
    {
        return Ok(PackFormat::Windows95);
    }
    if find_case_insensitive_path(root, &["SC2000.DAT"]).is_some_and(|path| path.is_file())
        && (find_case_insensitive_path(root, &["RESOURCE.URK"]).is_some_and(|path| path.is_file())
            || find_case_insensitive_path(root, &["SC2000SE.DAT"])
                .is_some_and(|path| path.is_file()))
    {
        return Ok(PackFormat::DosGog);
    }
    bail!(
        "Could not recognize this folder. Choose extracted Mac resources, a Win95 SimCity 2000™ install folder, or a DOS/GOG/EA-compatible install folder."
    )
}

fn find_case_insensitive_path(root: &Path, components: &[&str]) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for component in components {
        let entries = fs::read_dir(&current).ok()?;
        let mut exact = None;
        let mut insensitive = None;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy();
            if name.as_ref() == *component {
                exact = Some(path);
                break;
            }
            if insensitive.is_none() && name.eq_ignore_ascii_case(component) {
                insensitive = Some(path);
            }
        }
        current = exact.or(insensitive)?;
    }
    Some(current)
}

fn first_city_in_pack(root: &Path) -> Option<PathBuf> {
    let directory = find_case_insensitive_path(root, &["catalog", "cities"])?;
    let mut cities = fs::read_dir(directory)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("SC2"))
        })
        .collect::<Vec<_>>();
    cities.sort_by_key(|path| {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase()
    });
    cities.into_iter().next()
}

fn resolve_default_city(
    configured: &Path,
    recent: Option<&Path>,
    packs: &[RegisteredPack],
) -> Option<PathBuf> {
    if configured.is_file() {
        return Some(configured.to_path_buf());
    }
    if let Some(path) = recent.filter(|path| path.is_file()) {
        return Some(path.to_path_buf());
    }

    let mut fallback_city = None;
    for pack in packs {
        if let Some(path) =
            find_case_insensitive_path(&pack.root, &["catalog", "cities", "default.sc2"])
                .filter(|path| path.is_file())
        {
            return Some(path);
        }
        fallback_city = fallback_city.or_else(|| first_city_in_pack(&pack.root));
    }
    fallback_city
}

fn copy_city_catalog(source: &Path, output: &Path) -> Result<()> {
    let mut roots = vec![source.to_path_buf()];
    for wrapper in ["SC2K", "SC2000", "SIMCITY 2000", "SIMCITY2000"] {
        if let Some(root) =
            find_case_insensitive_path(source, &[wrapper]).filter(|path| path.is_dir())
        {
            roots.push(root);
        }
    }

    for (source_name, output_name, extension) in [
        ("CITIES", "cities", "SC2"),
        ("SCENARIO", "scenarios", "SCN"),
    ] {
        for root in &roots {
            let Some(directory) =
                find_case_insensitive_path(root, &[source_name]).filter(|path| path.is_dir())
            else {
                continue;
            };
            let destination = output.join("catalog").join(output_name);
            for entry in fs::read_dir(&directory)
                .with_context(|| format!("reading {}", directory.display()))?
                .flatten()
            {
                let path = entry.path();
                if !path.is_file()
                    || !path
                        .extension()
                        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(extension))
                {
                    continue;
                }
                fs::create_dir_all(&destination)
                    .with_context(|| format!("creating {}", destination.display()))?;
                let target = destination.join(path.file_name().context("city data has no name")?);
                if !target.exists() {
                    fs::copy(&path, &target).with_context(|| {
                        format!("copying original city data from {}", path.display())
                    })?;
                }
            }
        }
    }

    for root in &roots {
        let Some(default_city) =
            find_case_insensitive_path(root, &["DEFAULT.SC2"]).filter(|path| path.is_file())
        else {
            continue;
        };
        let destination = output.join("catalog/cities");
        fs::create_dir_all(&destination)
            .with_context(|| format!("creating {}", destination.display()))?;
        let target = destination.join(
            default_city
                .file_name()
                .context("default city file has no name")?,
        );
        if !target.exists() {
            fs::copy(&default_city, &target).with_context(|| {
                format!(
                    "copying original default city from {}",
                    default_city.display()
                )
            })?;
        }
        break;
    }
    Ok(())
}

pub fn settings_path() -> PathBuf {
    if let Some(path) = std::env::var_os("SC2000_ASSET_SETTINGS") {
        return PathBuf::from(path);
    }
    app_support_dir().join("asset-settings.json")
}

/// Pref / Application Support root (also used when seeding mobile sandbox from bundle).
pub fn app_support_dir_for_seed() -> PathBuf {
    app_support_dir()
}

pub fn cities_directory() -> PathBuf {
    app_support_dir().join("Cities")
}

fn app_support_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("SC2000_APP_SUPPORT_DIR") {
        return PathBuf::from(path);
    }
    // Android/iOS: SDL sandbox pref path (Documents-equivalent inside the app container).
    if cfg!(any(target_os = "android", target_os = "ios")) {
        if let Ok(path) = sdl3::filesystem::get_pref_path("CityGame", "CityGame") {
            return path;
        }
    }
    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            let current = home.join("Library/Application Support/CityGame");
            let legacy = home.join("Library/Application Support/SC2K Rust Viewer");
            return prefer_existing_legacy_dir(current, legacy);
        }
    }
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME") {
        let config = PathBuf::from(config);
        return prefer_existing_legacy_dir(
            config.join("citygame"),
            config.join("sc2k-rust-viewer"),
        );
    }
    prefer_existing_legacy_dir(
        PathBuf::from(".citygame"),
        PathBuf::from(".sc2k-rust-viewer"),
    )
}

fn prefer_existing_legacy_dir(current: PathBuf, legacy: PathBuf) -> PathBuf {
    if current.exists() || !legacy.exists() {
        current
    } else {
        legacy
    }
}

fn absolutize_settings_paths(settings: &mut AssetSettings, base: &Path) {
    let resolve = |path: &Option<PathBuf>| -> Option<PathBuf> {
        let path = path.as_ref()?;
        if path.is_absolute() {
            Some(path.clone())
        } else {
            Some(base.join(path))
        }
    };
    settings.original_graphics = resolve(&settings.original_graphics);
    settings.original_effects = resolve(&settings.original_effects);
    settings.original_music = resolve(&settings.original_music);
    settings.replacement_graphics = resolve(&settings.replacement_graphics);
    settings.tile_set_graphics = resolve(&settings.tile_set_graphics);
    settings.recent_city = resolve(&settings.recent_city);
    for pack in &mut settings.imported_packs {
        if !pack.root.is_absolute() {
            pack.root = base.join(&pack.root);
        }
    }
}

fn next_import_directory(format: PackFormat) -> Result<PathBuf> {
    let parent = app_support_dir().join("Imported Assets");
    fs::create_dir_all(&parent).with_context(|| format!("creating {}", parent.display()))?;
    for suffix in 1..10_000 {
        let name = if suffix == 1 {
            format.output_stem().to_owned()
        } else {
            format!("{}-{suffix}", format.output_stem())
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("too many imported {} packs", format.label())
}

fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("opening {}", path.display()))?;
    ensure!(path.is_dir(), "{} must be a folder", path.display());
    Ok(path)
}

fn has_media(root: &Path, media: OriginalMedia) -> bool {
    match media {
        OriginalMedia::Graphics => {
            root.join("sprites").is_dir()
                || root.join("win95_tiles").is_dir()
                || root.join("tiles/SC2000_DAT").is_dir()
        }
        OriginalMedia::Effects | OriginalMedia::Music => {
            let extension = if media == OriginalMedia::Effects {
                "wav"
            } else {
                "mid"
            };
            fs::read_dir(root.join("catalog/sounds"))
                .ok()
                .is_some_and(|entries| {
                    entries.flatten().any(|entry| {
                        entry.path().is_file()
                            && entry.path().extension().is_some_and(|ext| {
                                ext.to_string_lossy().eq_ignore_ascii_case(extension)
                            })
                    })
                })
        }
    }
}

fn reviewed_effects(root: &Path) -> HashMap<u16, PathBuf> {
    let mut effects = HashMap::new();
    let Ok(bytes) = fs::read(root.join("asset-pack.json")) else {
        return effects;
    };
    let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return effects;
    };
    let Some(assets) = manifest.get("assets").and_then(|value| value.as_array()) else {
        return effects;
    };
    for asset in assets {
        if asset.get("status").and_then(|value| value.as_str()) != Some("reviewed") {
            continue;
        }
        let Some(id) = asset
            .get("key")
            .and_then(|value| value.as_str())
            .and_then(|key| key.strip_prefix("citygame.sound."))
            .and_then(|id| id.parse::<u16>().ok())
        else {
            continue;
        };
        let Some(file) = asset.get("file").and_then(|value| value.as_str()) else {
            continue;
        };
        if !(500..=531).contains(&id) || !file.to_ascii_lowercase().ends_with(".wav") {
            continue;
        }
        let path = root.join(file);
        if let (Ok(root), Ok(path)) = (root.canonicalize(), path.canonicalize()) {
            if path.starts_with(root) && path.is_file() {
                effects.insert(id, path);
            }
        }
    }
    effects
}

#[cfg(test)]
#[path = "asset_profile/tests.rs"]
mod tests;
