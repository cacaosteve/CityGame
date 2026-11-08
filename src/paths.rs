use std::path::PathBuf;

/// Optional development graphics export, relative to the current working directory.
/// Installed users should register an asset pack in the app instead.
pub const DEFAULT_ASSET_ROOT: &str = "outputs/sc2000_graphics_png";
/// Portable fallback checked before imported asset packs are searched.
pub const DEFAULT_CITY: &str = "assets/catalog/cities/DEFAULT.SC2";

fn configured_path(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable).map(PathBuf::from)
}

pub fn asset_root() -> PathBuf {
    configured_path("SC2000_ASSET_ROOT").unwrap_or_else(|| PathBuf::from(DEFAULT_ASSET_ROOT))
}

pub fn default_city_path() -> PathBuf {
    configured_path("SC2000_DEFAULT_CITY").unwrap_or_else(|| PathBuf::from(DEFAULT_CITY))
}

pub fn default_open_dialog_dir() -> PathBuf {
    configured_path("SC2000_OPEN_DIALOG_DIR")
        .or_else(documents_dir)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Preferred folder for city open/save on tablets (and as desktop fallback).
///
/// Uses SDL pref path `…/Cities` when available so Android/iOS land in sandbox storage.
pub fn documents_dir() -> Option<PathBuf> {
    if let Ok(pref) = sdl3::filesystem::get_pref_path("CityGame", "CityGame") {
        let cities = pref.join("Cities");
        if cities.is_dir() || std::fs::create_dir_all(&cities).is_ok() {
            return Some(cities);
        }
    }
    None
}

#[cfg(test)]
pub fn test_asset_root() -> Option<PathBuf> {
    let root = asset_root();
    root.is_dir().then_some(root)
}

#[cfg(test)]
pub fn test_replacement_root() -> Option<PathBuf> {
    let root = configured_path("CITYGAME_TEST_REPLACEMENT_ROOT").unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/replacements/our-graphics")
    });
    root.join("asset-pack.json").is_file().then_some(root)
}

#[cfg(test)]
pub fn test_mac_city() -> Option<PathBuf> {
    configured_path("SC2000_TEST_MAC_CITY").filter(|path| path.is_file())
}

#[cfg(test)]
pub fn test_win95_root() -> Option<PathBuf> {
    configured_path("SC2000_TEST_WIN95_ROOT").filter(|path| path.is_dir())
}

#[cfg(test)]
pub fn test_win_city() -> Option<PathBuf> {
    configured_path("SC2000_TEST_WIN_CITY")
        .filter(|path| path.is_file())
        .or_else(|| test_win95_root().map(|root| root.join("DEFAULT.SC2")))
        .filter(|path| path.is_file())
}

#[cfg(test)]
pub fn test_win_populated_city() -> Option<PathBuf> {
    configured_path("SC2000_TEST_WIN_POPULATED_CITY")
        .filter(|path| path.is_file())
        .or_else(|| test_win95_root().map(|root| root.join("CITIES/BAYVIEW.SC2")))
        .filter(|path| path.is_file())
}

#[cfg(test)]
pub fn test_win_cape_city() -> Option<PathBuf> {
    configured_path("SC2000_TEST_WIN_CAPE_CITY")
        .filter(|path| path.is_file())
        .or_else(|| test_win95_root().map(|root| root.join("CITIES/CAPE.SC2")))
        .filter(|path| path.is_file())
}

#[cfg(test)]
pub fn test_win_scenario() -> Option<PathBuf> {
    configured_path("SC2000_TEST_WIN_SCENARIO")
        .filter(|path| path.is_file())
        .or_else(|| test_win95_root().map(|root| root.join("SCENARIO/OAKLAND.SCN")))
        .filter(|path| path.is_file())
}

#[cfg(test)]
pub fn test_win_catalog() -> Option<PathBuf> {
    configured_path("SC2000_TEST_WIN_CATALOG")
        .filter(|path| path.is_dir())
        .or_else(|| test_win95_root().map(|root| root.join("catalog")))
        .filter(|path| path.is_dir())
}

#[cfg(test)]
pub fn test_title_music() -> Option<PathBuf> {
    configured_path("SC2000_TEST_TITLE_MUSIC")
        .filter(|path| path.is_file())
        .or_else(|| test_win95_root().map(|root| root.join("SOUNDS/10018.MID")))
        .filter(|path| path.is_file())
}

#[cfg(test)]
pub fn test_city_from_env(variable: &str) -> Option<PathBuf> {
    configured_path(variable).filter(|path| path.is_file())
}
