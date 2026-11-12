use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sc2k-profile-{name}-{nonce}"));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
#[ignore = "requires locally extracted Macintosh Scenario Disk data and forks"]
fn mac_scenario_retail_catalog_import_preserves_data_and_forks() {
    use crate::mac_scenario::{MacScenario, scenario_sidecar_path};
    let source = PathBuf::from(std::env::var_os("CITYGAME_MAC_SCENARIO_FIXTURES").unwrap());
    let output = scratch("mac-scenario-catalog");
    copy_mac_scenario_catalog(&source, &output).unwrap();
    let catalog = output.join("catalog/scenarios");
    assert_eq!(fs::read_dir(&catalog).unwrap().count(), 10);
    for name in ["Charleston", "Dullsville", "Flint", "Hollywood", "Oakland"] {
        let original = source.join(name);
        let copied = catalog.join(name);
        assert_eq!(fs::read(&original).unwrap(), fs::read(&copied).unwrap());
        assert_eq!(
            fs::read(scenario_sidecar_path(&original)).unwrap(),
            fs::read(scenario_sidecar_path(&copied)).unwrap()
        );
        assert!(MacScenario::read(&copied).is_some());
    }
    fs::remove_dir_all(output).unwrap();
}

#[test]
fn bundled_pack_is_found_in_a_moved_app_without_its_source_checkout() {
    let root = scratch("moved-app-art");
    let exe = root.join("Renamed Game.app/Contents/MacOS/CityGame");
    let bundled = root.join("Renamed Game.app/Contents/Resources/replacements/our-graphics");
    let source = root.join("source");
    let development = source.join("assets/replacements/our-graphics");
    fs::create_dir_all(&development).unwrap();
    fs::write(development.join("asset-pack.json"), "{}").unwrap();
    assert_eq!(
        find_bundled_replacement(Some(&exe), &source),
        Some(development)
    );
    fs::create_dir_all(&bundled).unwrap();
    fs::write(bundled.join("asset-pack.json"), "{}").unwrap();
    assert_eq!(
        find_bundled_replacement(Some(&exe), &source),
        Some(bundled.clone())
    );
    fs::remove_dir_all(&source).unwrap();
    assert_eq!(find_bundled_replacement(Some(&exe), &source), Some(bundled));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unconfigured_override_uses_bundled_art_and_preserves_explicit_selections() {
    let root = scratch("default-authored-art");
    let path = root.join("settings.json");
    fs::write(&path, r#"{"schema_version":1,"replacement_graphics":null,"graphics_preference":"prefer-replacement"}"#).unwrap();
    let mut profile = AssetProfile::load_from(path.clone()).unwrap();
    let bundled = root.join("bundled");
    profile.environment_replacement = None;
    profile.bundled_replacement = Some(bundled.clone());
    assert!(profile.preference().prefers_replacement());
    assert_eq!(profile.replacement_root(), Some(bundled));
    profile
        .set_preference(GraphicsPreference::PreferOriginal)
        .unwrap();
    assert!(!profile.preference().prefers_replacement());
    let saved: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert!(
        saved["replacement_graphics"].is_null(),
        "keep the default portable instead of persisting the current app location"
    );
    let custom = root.join("custom");
    profile.settings.replacement_graphics = Some(custom.clone());
    assert_eq!(profile.replacement_root(), Some(custom));
    let environment = root.join("environment");
    profile.environment_replacement = Some(environment.clone());
    assert_eq!(profile.replacement_root(), Some(environment));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn bundled_pack_discovery_never_selects_an_empty_artwork_directory() {
    let root = scratch("missing-authored-manifest");
    let artwork = root.join("assets/replacements/our-graphics");
    fs::create_dir_all(&artwork).unwrap();
    assert_eq!(find_bundled_replacement(None, &root), None);
    fs::write(artwork.join("asset-pack.json"), "{}").unwrap();
    assert_eq!(find_bundled_replacement(None, &root), Some(artwork));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn detects_supported_install_folders() {
    let win = scratch("win");
    fs::create_dir_all(win.join("SC2K/DATA")).unwrap();
    fs::create_dir_all(win.join("SC2K/BITMAPS")).unwrap();
    fs::write(win.join("SC2K/DATA/LARGE.DAT"), []).unwrap();
    fs::write(win.join("SC2K/BITMAPS/PAL_MSTR.BMP"), []).unwrap();
    assert_eq!(detect_install_source(&win).unwrap(), PackFormat::Windows95);
    fs::remove_dir_all(win).unwrap();

    let mixed_case_win = scratch("win-mixed-case");
    fs::create_dir_all(mixed_case_win.join("Data")).unwrap();
    fs::create_dir_all(mixed_case_win.join("Bitmaps")).unwrap();
    fs::write(mixed_case_win.join("Data/large.dat"), []).unwrap();
    fs::write(mixed_case_win.join("Bitmaps/pal_mstr.bmp"), []).unwrap();
    assert_eq!(
        detect_install_source(&mixed_case_win).unwrap(),
        PackFormat::Windows95
    );
    fs::remove_dir_all(mixed_case_win).unwrap();

    let dos = scratch("dos");
    fs::write(dos.join("SC2000.DAT"), []).unwrap();
    fs::write(dos.join("RESOURCE.URK"), []).unwrap();
    assert_eq!(detect_install_source(&dos).unwrap(), PackFormat::DosGog);
    fs::remove_dir_all(dos).unwrap();

    let mac = scratch("mac");
    fs::create_dir(mac.join("SPRT")).unwrap();
    assert_eq!(detect_install_source(&mac).unwrap(), PackFormat::Mac68k);
    fs::remove_dir_all(mac).unwrap();
}

#[test]
fn preferences_round_trip_without_touching_user_settings() {
    let root = scratch("settings");
    let path = root.join("settings.json");
    let mut profile = AssetProfile::load_from(path.clone()).unwrap();
    assert_eq!(profile.preference(), GraphicsPreference::PreferOriginal);
    assert!(!profile.citygame_effects_enabled());
    assert_eq!(
        profile.title_music_preference(),
        TitleMusicPreference::Original
    );
    assert_eq!(
        profile.toolbar_palette_preference(),
        ToolbarPalettePreference::CityGame
    );
    profile
        .set_preference(GraphicsPreference::PreferReplacement)
        .unwrap();
    profile
        .set_title_music_preference(TitleMusicPreference::Replacement)
        .unwrap();
    profile
        .set_toolbar_palette_preference(ToolbarPalettePreference::Classic)
        .unwrap();
    let loaded = AssetProfile::load_from(path).unwrap();
    assert_eq!(loaded.preference(), GraphicsPreference::PreferReplacement);
    assert_eq!(
        loaded.title_music_preference(),
        TitleMusicPreference::Replacement
    );
    assert_eq!(
        loaded.toolbar_palette_preference(),
        ToolbarPalettePreference::Classic
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn older_settings_default_to_the_citygame_toolbar_palette() {
    let root = scratch("old-settings");
    let path = root.join("settings.json");
    fs::write(&path, r#"{"schema_version":1}"#).unwrap();
    let profile = AssetProfile::load_from(path).unwrap();
    assert_eq!(
        profile.toolbar_palette_preference(),
        ToolbarPalettePreference::CityGame
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recent_city_is_saved_and_missing_files_are_not_offered() {
    let root = scratch("recent-city");
    let path = root.join("settings.json");
    let city = root.join("My City.SC2");
    fs::write(&city, b"test").unwrap();
    let mut profile = AssetProfile::load_from(path.clone()).unwrap();
    profile.remember_city(&city).unwrap();
    assert_eq!(
        AssetProfile::load_from(path.clone()).unwrap().recent_city(),
        Some(city.canonicalize().unwrap())
    );
    fs::remove_file(city).unwrap();
    assert_eq!(AssetProfile::load_from(path).unwrap().recent_city(), None);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_configured_city_falls_back_to_imported_catalog() {
    let root = scratch("default-city");
    let pack = root.join("imported");
    let city = pack.join("catalog/cities/DEFAULT.SC2");
    fs::create_dir_all(city.parent().unwrap()).unwrap();
    fs::write(&city, b"city file").unwrap();
    let registered = RegisteredPack {
        format: PackFormat::Windows95,
        root: pack,
    };

    let resolved =
        resolve_default_city(&root.join("old-mac-path/DEFAULT.SC2"), None, &[registered]);

    assert_eq!(resolved, Some(city));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn independent_sources_and_missing_slot_fallback() {
    let scratch = scratch("media-library");
    let mac = scratch.join("mac");
    let win = scratch.join("win");
    for root in [&mac, &win] {
        fs::create_dir_all(root.join("catalog/sounds")).unwrap();
    }
    fs::create_dir_all(mac.join("sprites")).unwrap();
    fs::create_dir_all(win.join("win95_tiles")).unwrap();
    fs::write(mac.join("catalog/sounds/505.WAV"), b"click").unwrap();
    fs::write(mac.join("catalog/sounds/10018.MID"), b"mac theme").unwrap();
    fs::write(win.join("catalog/sounds/505.WAV"), b"win click").unwrap();
    fs::write(win.join("catalog/sounds/514.WAV"), b"zap").unwrap();
    fs::write(win.join("catalog/sounds/10018.MID"), b"win theme").unwrap();
    let mut profile = AssetProfile {
        settings: AssetSettings {
            original_graphics: Some(mac.clone()),
            original_effects: Some(mac.clone()),
            original_music: Some(win.clone()),
            imported_packs: vec![
                RegisteredPack {
                    format: PackFormat::Mac68k,
                    root: mac.clone(),
                },
                RegisteredPack {
                    format: PackFormat::Windows95,
                    root: win.clone(),
                },
            ],
            ..AssetSettings::default()
        },
        settings_path: scratch.join("settings.json"),
        environment_original: None,
        environment_replacement: None,
        bundled_replacement: None,
    };
    let effects = profile.sound_paths().effects;
    assert_eq!(effects.get(&505), Some(&mac.join("catalog/sounds/505.WAV")));
    assert_eq!(effects.get(&514), Some(&win.join("catalog/sounds/514.WAV")));
    assert_eq!(
        profile.original_music_path(10018),
        Some(win.join("catalog/sounds/10018.MID"))
    );
    profile.cycle_source(OriginalMedia::Effects).unwrap();
    assert_eq!(
        profile.sound_paths().effects.get(&505),
        Some(&win.join("catalog/sounds/505.WAV"))
    );
    assert_eq!(profile.media_source(OriginalMedia::Graphics), Some(mac));
    fs::remove_dir_all(scratch).unwrap();
}
