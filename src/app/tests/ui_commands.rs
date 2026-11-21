use super::*;
use crate::app::dialogs::city_info_dialog_hit_test_for_engine;

#[test]
fn mac_saved_view_uses_native_picker_and_restores_native_zoom() {
    let path =
        std::env::temp_dir().join(format!("citygame-mac-camera-{}.city", std::process::id()));
    let mut city = City::new_blank(&path, "Camera").unwrap();
    city.kind = CityFileKind::MacCity;
    for x in 0..128 {
        for y in 0..128 {
            city.set_altitude(
                x,
                y,
                ((3 * x + 5 * y) % 32) as u8,
                ((7 * x + 11 * y + 3) % 32) as u8,
            );
        }
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.zoom = 0.25;
    // At 640x480 this corresponds to native origin (220,34) and anchor
    // (320,240). The point (312,212) has its own original-instruction
    // fixture; shift both native origin and anchor by (8,28) to preserve it.
    state.camera = glam::Vec2::new(-96.0, -171.0);
    let expected = state.saved_mac_view_for_window((640, 480), 1.0).unwrap();
    assert_eq!(expected, (0x394f, 0));
    assert_eq!(
        state.tile_at_screen(glam::Vec2::new(320.0, 240.0), (640, 480), 1.0),
        Some((79, 57))
    );
    state.show_underground = true;
    assert_eq!(
        state.tile_at_screen(glam::Vec2::new(320.0, 240.0), (640, 480), 1.0),
        Some((77, 56))
    );
    state.show_underground = false;
    let snapshot = state.city_save_snapshot((640, 480), 1.0);
    assert_eq!(snapshot.mac_state_misc_u32_at(0x1010), Some(expected.0));
    state.save_city(&path, (640, 480), 1.0).unwrap();
    let reloaded = City::load(&path).unwrap();
    assert_eq!(reloaded.mac_state_misc_u32_at(0x1010), Some(expected.0));
    assert_eq!(reloaded.mac_state_misc_u32_at(0x1014), Some(0));
    state.zoom = 2.0;
    assert!(state.restore_saved_city_view((640, 480), 1.0));
    assert_eq!(state.zoom, 0.25);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mac_newspaper_menu_selects_only_available_saved_pages() {
    let city = City::new_blank("/tmp/mac-paper-menu.sc2", "Paper").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state
        .city
        .set_mac_state_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 1);
    state
        .city
        .set_mac_state_misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET, 0);
    state
        .city
        .set_mac_state_misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET + 20, 5);
    state
        .city
        .set_mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET, 0);
    state.sync_edit_map_menu();
    assert_eq!(state.menu_checks.mac_paper_count, 2);
    assert_eq!(&state.menu_checks.mac_paper_names[..2], &[0, 5]);
    let seed = state.mac_process_rng.seed();
    state
        .apply_state_app_command(&AppCommand::OpenMacPaper(2))
        .unwrap();
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(0)
    );
    assert_eq!(state.city_info_dialog, None);
    let command = app_command_for_menu_command(crate::menu_model::CMD_MAC_PAPER_FIRST + 1).unwrap();
    state.apply_state_app_command(&command).unwrap();
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(1)
    );
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::MacStandard))
    );
    assert_eq!(state.menu_checks.mac_paper_selected, 1);
    assert_eq!(
        state.mac_process_rng.seed(),
        seed,
        "empty city and absent paper resources draw nothing"
    );
    state
        .apply_state_app_command(&AppCommand::OpenMacPaper(0))
        .unwrap();
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(1),
        "open paper owns input"
    );
    state
        .city
        .set_mac_state_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 6);
    state.refresh_mac_newspaper_menu();
    assert_eq!(state.menu_checks.mac_paper_count, 6);
}

#[test]
#[ignore = "requires locally supplied Mac Fire city"]
fn mac_newspaper_menu_selection_survives_save_and_reload() {
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_PAPER_WIDTHS").unwrap());
    let source = root.join("../manual-fire-protected-full/fireguard.city");
    let city = City::new_blank("/tmp/mac-paper-menu-load.sc2", "Before").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.load_city(&source, (640, 480), 1.0).unwrap();
    state.sync_edit_map_menu();
    assert!(state.menu_checks.mac_paper_count >= 2);
    state
        .apply_state_app_command(&AppCommand::OpenMacPaper(1))
        .unwrap();
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(1)
    );
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "citygame-paper-menu-{}-{nonce}.sc2",
        std::process::id()
    ));
    state.city.save(&path).unwrap();
    let loaded = City::load(&path).unwrap();
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(1)
    );
    assert_eq!(
        loaded.mac_state_misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET + 20),
        state
            .city
            .mac_state_misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET + 20)
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
#[ignore = "requires the private live Quadra paper capture and Mac city"]
fn mac_newspaper_menu_matches_live_chronicle_selection() {
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_PAPER_WIDTHS").unwrap());
    let live = root.join("../mac-paper-live-layout");
    let ram = std::fs::read(live.join("chronicle.ram")).unwrap();
    let read32 = |p: usize| u32::from_be_bytes(ram[p..p + 4].try_into().unwrap()) as usize;
    let a5 = read32(0x904);
    let table = |off: usize, len: usize| {
        let p = read32(a5 - off) & 0xffffff;
        &ram[p..p + len]
    };
    let city = City::new_blank("/tmp/mac-paper-menu-live.sc2", "Before").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_newspaper.tick_word = Some(i16::from_be_bytes(
        ram[a5 - 0x36cc..a5 - 0x36ca].try_into().unwrap(),
    ));
    state.mac_newspaper.data = Some(std::sync::Arc::new(
        crate::mac_newspaper::MacPaperTokens::from_resources(
            table(0x36e4, 500),
            table(0x36e0, 500),
            table(0x36dc, 10000),
            table(0x36d8, 149362),
        )
        .unwrap(),
    ));
    state
        .load_city(
            &root.join("../manual-fire-protected-full/fireguard"),
            (640, 480),
            1.0,
        )
        .unwrap();
    state.sync_edit_map_menu();
    let native_papers = read32(a5 - 0xfa2) & 0xffffff;
    assert_eq!(state.menu_checks.mac_paper_count, 2);
    assert_eq!(state.menu_checks.mac_paper_names[0], ram[native_papers]);
    assert_eq!(state.menu_checks.mac_paper_names[1], ram[native_papers + 6]);
    let command = app_command_for_menu_command(crate::menu_model::CMD_MAC_PAPER_FIRST + 1).unwrap();
    state.apply_state_app_command(&command).unwrap();
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(u32::from(u16::from_be_bytes(
            ram[a5 - 0x36c6..a5 - 0x36c4].try_into().unwrap()
        )))
    );
    let headline = std::fs::read(live.join("native-page1-headline-0.bin")).unwrap();
    assert_eq!(
        state.mac_newspaper.headline.as_deref(),
        Some(crate::city::mac_roman::decode(&headline).as_str())
    );
}

#[test]
fn mac_paper_missing_assets_does_not_change_process_rng() {
    let city = City::new_blank("/tmp/citygame-paper-missing.sc2", "Paper").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_process_rng = MacToolboxRng::new(12345);
    state.mac_newspaper.tick_word = Some(100);
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::MacDisaster));
    assert!(state.mac_newspaper.paint.is_none());
    assert_eq!(state.mac_process_rng.seed(), 12345);
}

#[test]
fn mac_paper_selects_song_once_on_open_from_app_lfsr() {
    let city = City::new_blank("/tmp/citygame-paper-song.sc2", "Paper").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_app_lfsr = crate::mac_random::MacAppLfsrRng::new(1);
    state.music_enabled = false;

    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::MacDisaster));
    assert_eq!(state.mac_app_lfsr.state(), 2);
    assert_eq!(state.pending_newspaper_music.take(), Some(10_014));
    state.close_mac_paper_detail();
    assert_eq!(
        state.mac_app_lfsr.state(),
        2,
        "repainting does not select another song"
    );
    state.close_city_info_dialog();
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::MacDisaster));
    assert_eq!(state.mac_app_lfsr.state(), 4);
    assert_eq!(state.pending_newspaper_music, Some(10_002));
}

#[test]
#[ignore = "requires the private R13 Quadra capture and original Mac paper resource pack"]
fn mac_r13_automatic_story_window_reaches_native_quickdraw_seed() {
    let pack = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_PAPER_PACK").unwrap());
    let capture = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_STORY_R13").unwrap());
    let city_path =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_AUTODEBT_CITY").unwrap());
    let native_events = std::fs::read_to_string(capture.join("story-window-events.csv")).unwrap();
    let native_return = native_events
        .lines()
        .rev()
        .find(|line| line.starts_with("export233-return,"))
        .unwrap();
    let native_return: Vec<_> = native_return.split(',').collect();
    let expected_quickdraw = u32::from_str_radix(native_return[10], 16).unwrap();
    let expected_app_lfsr = u16::from_str_radix(native_return[12], 16).unwrap();
    let settings = std::env::temp_dir().join(format!(
        "citygame-r13-paper-settings-{}.json",
        std::process::id()
    ));
    std::fs::write(
        &settings,
        serde_json::json!({"schema_version": 1, "original_graphics": pack}).to_string(),
    )
    .unwrap();
    let profile = crate::asset_profile::AssetProfile::load_from(settings.clone()).unwrap();
    let mut city = City::load(&city_path).unwrap();
    city.kind = CityFileKind::MacCity;
    // City::load derives this save's name from its temporary filename because
    // it has no CNAM chunk; the native document is named "AutoDebt".
    city.name = "AutoDebt".to_owned();
    city.set_mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET, 29_446);
    city.set_mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET, 0); // AutoDebt Picayune.
    city.set_mac_state_misc_u32_at(0x1008, 1); // Extra enabled.
    city.set_mac_state_misc_u32_at(0x102c, 100); // Native opinion-poll gate.
    city.set_mac_state_misc_u32_at(MISC_HEAT_OFFSET, 170);
    city.set_mac_state_misc_u32_at(MISC_WIND_OFFSET, 47);
    city.set_mac_state_misc_u32_at(MISC_HUMID_OFFSET, 26);
    assert!(city.queue_mac_newspaper_story(5, 5));
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mode = AppMode::CityView;
    state.mac_newspaper.tick_word = Some(2_929);
    state.mac_process_rng = MacToolboxRng::new(0x15b3_0408);
    state.mac_app_lfsr = crate::mac_random::MacAppLfsrRng::new(0x48f3);
    state.configure_original_newspaper_data(&profile);

    state.open_mac_story_window();

    assert!(
        state.mac_newspaper.paint.is_some(),
        "original resources loaded"
    );
    let paint = state.mac_newspaper.paint.as_ref().unwrap();
    assert!(
        paint
            .survey
            .lines
            .iter()
            .any(|line| line.text.starts_with(b"AutoDebt")),
        "the temporary fixture path must not replace the native city name"
    );
    println!(
        "first-paint initial={:08x} lead=({:08x},{:08x}) stories={:08x?} weather={:08x} survey={:08x} final={:08x}",
        paint.initial_seed,
        paint.lead.after_lead_seed,
        paint.lead.after_picture_seed,
        paint
            .stories
            .iter()
            .map(|story| story.ending_seed)
            .collect::<Vec<_>>(),
        paint.weather.ending_seed,
        paint.survey.ending_seed,
        state.mac_process_rng.seed()
    );
    println!(
        "headline={:?} rows={:?}",
        state.mac_newspaper.headline,
        (0..9)
            .map(|i| state.city.mac_story_record(i))
            .collect::<Vec<_>>()
    );
    println!(
        "weather-lines={:?} weather-filler={} survey-lines={:?} survey-filler={} poll={:?}",
        paint
            .weather
            .lines
            .iter()
            .map(|line| String::from_utf8_lossy(&line.text).into_owned())
            .collect::<Vec<_>>(),
        paint.weather.filler.len(),
        paint
            .survey
            .lines
            .iter()
            .map(|line| String::from_utf8_lossy(&line.text).into_owned())
            .collect::<Vec<_>>(),
        paint.survey.filler.len(),
        state.city.mac_opinion_poll_results()
    );
    assert_eq!(state.mac_process_rng.seed(), expected_quickdraw);
    assert_eq!(state.mac_app_lfsr.state(), expected_app_lfsr);
    std::fs::remove_file(settings).unwrap();
}

#[test]
#[ignore = "requires user-supplied Mac System font and private Quadra capture"]
fn mac_newspaper_loads_user_font_from_asset_profile() {
    let capture = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_PAPER_WIDTHS").unwrap());
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "citygame-paper-font-{}-{nonce}",
        std::process::id()
    ));
    let pack = root.join("pack");
    std::fs::create_dir_all(pack.join("catalog/mac-newspaper")).unwrap();
    std::fs::copy(
        capture.join("newyork.bin"),
        pack.join("catalog/mac-newspaper/new-york.bin"),
    )
    .unwrap();
    let settings = root.join("settings.json");
    std::fs::write(
        &settings,
        serde_json::json!({"schema_version": 1, "original_graphics": pack}).to_string(),
    )
    .unwrap();
    let profile = crate::asset_profile::AssetProfile::load_from(settings).unwrap();
    let mut state = test_state(City::new_blank("/tmp/citygame-paper-font.sc2", "City").unwrap());
    state.configure_original_newspaper_data(&profile);
    let font = state
        .mac_newspaper
        .font
        .as_ref()
        .expect("loaded local font");
    let row = std::fs::read_to_string(capture.join("widths-page0-font.csv")).unwrap();
    let fields: Vec<_> = row
        .lines()
        .find(|line| line.split(',').nth(1) == Some("8"))
        .unwrap()
        .split(',')
        .collect();
    let hex = fields.last().unwrap().as_bytes();
    let text: Vec<_> = hex
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(
        font.width(8, &text).unwrap(),
        fields[0].parse::<u32>().unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn win95_new_city_seeds_native_fixed_microsim_rows() {
    let city = City::new_blank("/tmp/new-city-fixed-xmic.sc2", "Before").unwrap();
    let mut state = test_state(city);

    state.new_city().expect("Win95 new city");
    assert_eq!(state.city.label_at(0), Some("Mayor McSim"));

    let expected_rows = [
        (236, None),
        (236, Some("SimBus System")),
        (237, Some("SimRail System")),
        (233, Some("SimSubway")),
        (200, Some("Wind Power")),
        (198, Some("Hydro Power")),
        (213, Some("SimPark System")),
        (212, Some("Museum")),
        (245, Some("Library System")),
        (248, Some("Marina")),
    ];
    for (slot, (expected_tile, expected_label)) in expected_rows.into_iter().enumerate() {
        let row = state
            .city
            .microsim_at(slot)
            .unwrap_or_else(|| panic!("fixed XMIC row {slot}"));
        assert_eq!(row.tile_id, expected_tile, "fixed XMIC row {slot} tag");
        assert_eq!(
            (row.stat0, row.stat1, row.stat2, row.stat3),
            (0, 0, 0, 0),
            "fixed XMIC row {slot} reset state"
        );
        if let Some(expected_label) = expected_label {
            assert_eq!(
                state.city.label_at(MIN_SIM_TEXT_ENTRIES + slot as u8),
                Some(expected_label),
                "fixed XMIC row {slot} fallback label"
            );
        }
    }
    assert_eq!(
        state.city.microsim_at(10).map(|row| row.tile_id),
        Some(0),
        "ordinary structure rows remain free"
    );
}

#[test]
fn win95_generated_city_keeps_fixed_microsims_and_system_labels() {
    let city = City::new_blank("/tmp/generated-fixed-xmic.sc2", "Before").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state
        .new_city_from_title(
            "New City".into(),
            20_000,
            1900,
            1,
            0,
            title_screen::NewCityStartFlow::GenerateAndPlay,
        )
        .unwrap();
    for (slot, tile) in [236, 236, 237, 233, 200, 198, 213, 212, 245, 248]
        .into_iter()
        .enumerate()
    {
        let record = state.city.microsim_at(slot).unwrap();
        assert_eq!(record.tile_id, tile, "slot {slot}");
        assert_eq!(
            (record.stat0, record.stat1, record.stat2, record.stat3),
            (0, 0, 0, 0)
        );
    }
    for (slot, label) in [
        (52, "SimBus System"),
        (53, "SimRail System"),
        (54, "SimSubway"),
        (59, "Library System"),
    ] {
        assert_eq!(state.city.label_at(slot), Some(label));
    }
}

#[test]
fn win95_generated_city_uses_imported_reserved_names() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "citygame-win95-labels-{}-{nonce}",
        std::process::id()
    ));
    let pack = root.join("pack");
    let catalog = pack.join("catalog/data");
    std::fs::create_dir_all(&catalog).unwrap();
    let labels = ["Café", "Second", "Third", "Fourth", "Fifth"];
    std::fs::write(
        catalog.join("win95-new-city-labels.json"),
        serde_json::to_vec(&labels).unwrap(),
    )
    .unwrap();
    let settings = root.join("settings.json");
    std::fs::write(&settings, serde_json::json!({"schema_version": 1, "original_graphics": pack, "win95_mayor_name": "Café – €"}).to_string()).unwrap();
    let profile = crate::asset_profile::AssetProfile::load_from(settings).unwrap();
    let mut state = test_state(City::new_blank(root.join("blank.sc2"), "Blank").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.city.set_label(0, "Previous city's mayor");
    state.configure_original_newspaper_data(&profile);
    state
        .new_city_from_title(
            "New City".into(),
            20_000,
            1900,
            1,
            0,
            title_screen::NewCityStartFlow::GenerateAndPlay,
        )
        .unwrap();
    assert_eq!(state.city.label_at(0), Some("Café – €"));
    for (index, expected) in labels.iter().enumerate() {
        assert_eq!(state.city.label_at(251 + index as u8), Some(*expected));
    }
    let save = root.join("saved.sc2");
    state.city.save(&save).unwrap();
    let loaded = City::load(&save).unwrap();
    assert_eq!(loaded.label_at(0), Some("Café – €"));
    for (index, expected) in labels.iter().enumerate() {
        assert_eq!(loaded.label_at(251 + index as u8), Some(*expected));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires a private original Win95 generated-city save"]
fn win95_generated_city_microsims_match_original_save() {
    let native = City::load(std::path::PathBuf::from(
        std::env::var_os("CITYGAME_WIN95_GENERATED_SAVE").unwrap(),
    ))
    .unwrap();
    let mut state =
        test_state(City::new_blank("/tmp/native-xmic-comparison.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    let imported_labels = std::env::var_os("CITYGAME_WIN95_EXE").map(|path| {
        crate::win95_resources::new_city_reserved_labels(&std::fs::read(path).unwrap()).unwrap()
    });
    state.pe_new_city_reserved_labels = imported_labels.clone();
    let mayor = std::env::var("CITYGAME_WIN95_MAYOR_NAME").ok();
    if let Some(name) = &mayor {
        state.pe_registered_mayor_name.clone_from(name);
    }
    state
        .new_city_from_title(
            "New City".into(),
            20_000,
            1900,
            1,
            0,
            title_screen::NewCityStartFlow::GenerateAndPlay,
        )
        .unwrap();
    for slot in 0..150 {
        assert_eq!(
            state.city.microsim_at(slot),
            native.microsim_at(slot),
            "XMIC {slot}"
        );
    }
    for label in 52..=60 {
        assert_eq!(
            state.city.label_at(label),
            native.label_at(label),
            "XLAB {label}"
        );
    }
    if imported_labels.is_some() {
        for label in 251..=255 {
            assert_eq!(
                state.city.label_at(label),
                native.label_at(label),
                "XLAB {label}"
            );
        }
    }
    if mayor.is_some() {
        assert_eq!(
            state.city.label_at(0),
            native.label_at(0),
            "registered mayor"
        );
    }
}

#[test]
fn startup_newspaper_opens_after_named_generated_city_setup() {
    let city = City::new_blank("/tmp/startup-newspaper.sc2", "Before").unwrap();
    let mut state = test_state(city);
    state.menu_checks.citygame_newspaper = true;
    state
        .new_city_from_title(
            "Oak Bay".into(),
            20_000,
            1900,
            1,
            12345,
            title_screen::NewCityStartFlow::GenerateAndPlay,
        )
        .unwrap();
    assert_eq!(state.city.name, "Oak Bay");
    let rows = state.city.pe_newspaper_tables().unwrap().stories;
    assert_eq!(&rows[..6], &[2, 1000, 0, 255, 255, 255]);
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding))
    );
    assert!(!state.founding_newspaper_pending);
    state.close_city_info_dialog();
    state.maybe_show_founding_newspaper();
    assert_eq!(state.city_info_dialog, None, "welcome opens only once");
    state
        .new_city_from_title(
            "Cedar Bay".into(),
            20_000,
            1950,
            1,
            12345,
            title_screen::NewCityStartFlow::GenerateAndEdit,
        )
        .unwrap();
    assert_eq!(state.mode, AppMode::EditNewMap);
    assert_eq!(state.city_info_dialog, None);
    assert!(state.founding_newspaper_pending);
    state.finish_edit_map_state();
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding))
    );
    assert_eq!(state.city.name, "New City");
}

#[test]
fn win95_founding_mode_transition_queues_condition_story_after_paper() {
    for show_paper in [true, false] {
        let city = City::new_blank("/tmp/win95-founding-transition.sc2", "Before").unwrap();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.menu_checks.citygame_newspaper = show_paper;
        state
            .new_city_from_title(
                "New City".into(),
                20_000,
                1900,
                1,
                0,
                title_screen::NewCityStartFlow::GenerateAndPlay,
            )
            .unwrap();
        assert_eq!(state.city.pe_power_used_percent(), 0);
        if show_paper {
            assert!(state.pe_founding_mode_transition_pending);
            let stories = state.city.pe_newspaper_tables().unwrap().stories;
            assert_eq!(stories[0], 2);
            assert_ne!(stories[6], 47);
            state.close_city_info_dialog();
        }
        assert!(!state.pe_founding_mode_transition_pending);
        let stories = state.city.pe_newspaper_tables().unwrap().stories;
        assert_eq!((stories[0], stories[6], stories[7]), (2, 47, 500));
        state.close_city_info_dialog();
        assert_eq!(state.city.pe_newspaper_tables().unwrap().stories, stories);
    }
}

#[test]
fn startup_newspaper_waits_for_editor_and_respects_suppression() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/startup-newspaper-editor.sc2", "Oak Bay").unwrap();
        city.kind = kind;
        let mut state = test_state(city);
        state.menu_checks.citygame_newspaper = true;
        state.founding_newspaper_pending = true;
        state.mode = AppMode::EditNewMap;
        state.maybe_show_founding_newspaper();
        assert!(state.founding_newspaper_pending);
        assert_eq!(state.city_info_dialog, None);
        state.mode = AppMode::CityView;
        let before: Vec<_> = state.city.chunks.iter().map(|c| c.data.clone()).collect();
        let crt = state.pe_process_crt;
        state.maybe_show_founding_newspaper();
        assert_eq!(
            state.city_info_dialog,
            Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding))
        );
        if kind == CityFileKind::MacCity {
            assert_eq!(
                before,
                state
                    .city
                    .chunks
                    .iter()
                    .map(|c| c.data.clone())
                    .collect::<Vec<_>>()
            );
        } else {
            assert_eq!(
                &state.city.pe_newspaper_tables().unwrap().stories[..6],
                &[2, 1000, 0, 255, 255, 255]
            );
        }
        assert_eq!(crt, state.pe_process_crt);
        state.close_city_info_dialog();
        state.menu_checks.citygame_newspaper = false;
        state.founding_newspaper_pending = true;
        state.maybe_show_founding_newspaper();
        assert_eq!(state.city_info_dialog, None);
        assert!(!state.founding_newspaper_pending);
    }
}

#[test]
fn win95_newspaper_open_draws_shared_lfsr_once_per_open() {
    let city = City::new_blank("/tmp/newspaper-sound-rng.sc2", "New City").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.menu_checks.citygame_newspaper = true;
    state.pe_process_lfsr = Win95Lfsr::new(36_655);
    let paper = CityInfoDialog::Newspaper(NewspaperEdition::Founding);
    state.open_city_info_dialog(paper);
    assert_eq!(state.pe_process_lfsr.state(), 1_451);
    assert_eq!(state.pending_newspaper_music.take(), Some(10_015));
    state.open_city_info_dialog(paper); // Toggle closed, with no new initialization.
    assert_eq!(state.pe_process_lfsr.state(), 1_451);
    assert_eq!(state.pending_newspaper_music, None);
    state.open_city_info_dialog(paper);
    let mut expected = Win95Lfsr::new(1_451);
    expected.random_mod(5);
    assert_eq!(state.pe_process_lfsr.state(), expected.state());
    assert_eq!(state.pending_newspaper_music.take(), Some(10_014));

    state.close_city_info_dialog();
    state.menu_checks.citygame_newspaper = false;
    state.open_city_info_dialog(paper);
    assert_eq!(state.pe_process_lfsr.state(), expected.state());
    state.menu_checks.citygame_newspaper = true;
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.open_city_info_dialog(paper);
    assert_eq!(state.pe_process_lfsr.state(), expected.state());

    state.close_city_info_dialog();
    state.engine_fidelity = EngineFidelity::Win95;
    state.city.kind = CityFileKind::WinSc2;
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Courier));
    expected.random_mod(5);
    assert_eq!(state.pe_process_lfsr.state(), expected.state());

    for (seed, cue) in [
        (1, 10_014),
        (2, 10_002),
        (3, 10_015),
        (4, 10_006),
        (5, 10_009),
    ] {
        state.close_city_info_dialog();
        state.pe_process_lfsr = Win95Lfsr::new(seed);
        state.open_city_info_dialog(paper);
        assert_eq!(state.pending_newspaper_music.take(), Some(cue));
    }
}

#[test]
#[ignore = "requires user-supplied Win95 newspaper DATA"]
fn win95_newspaper_first_paint_runs_once_per_open() {
    let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
    let data = std::sync::Arc::new(
        crate::win95_newspaper::PeNewspaperData::load(
            &dir.join("data_usa.dat"),
            &dir.join("data_usa.idx"),
        )
        .unwrap(),
    );
    let mut state =
        test_state(City::new_blank("/tmp/citygame-pe-live-paper.sc2", "Civic Haven").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.menu_checks.citygame_newspaper = true;
    state.pe_newspaper_data = Some(data);
    state.pe_document_tick_word = Some(-690);
    state.pe_process_crt = Win95CrtRand::new(123);
    let mut tables = crate::city::PeNewspaperTables {
        papers: [0; 30],
        stories: [0; 54],
    };
    tables.papers[3] = 4;
    for row in 0..9 {
        let id = if row == 7 {
            0
        } else if row == 8 {
            44
        } else {
            42
        };
        tables.stories[row * 6..row * 6 + 6].copy_from_slice(&[id, 0, 0, 255, 255, 255]);
    }
    state.city.install_pe_newspaper_tables(tables);
    let paper = CityInfoDialog::Newspaper(NewspaperEdition::Founding);
    state.open_city_info_dialog(paper);
    state.prepare_pe_newspaper_first_paint();
    assert!(state.pe_newspaper_paint.is_some());
    let first_seed = state.pe_process_crt.seed();
    let first_rows = state.city.pe_newspaper_tables().unwrap().stories;
    state.prepare_pe_newspaper_first_paint();
    assert_eq!(state.pe_process_crt.seed(), first_seed);
    assert_eq!(
        state.city.pe_newspaper_tables().unwrap().stories,
        first_rows
    );
    state.close_city_info_dialog();
    assert!(state.pe_newspaper_paint.is_none());
    assert!(!state.pe_newspaper_paint_attempted);
    state.open_city_info_dialog(paper);
    state.prepare_pe_newspaper_first_paint();
    assert!(state.pe_newspaper_paint.is_some());
    assert!(state.pe_newspaper_paint_attempted);
}

#[test]
#[ignore = "requires user-supplied Win95 newspaper assets"]
fn win95_newspaper_opens_for_round_trip_safe_accented_name() {
    let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
    let data = std::sync::Arc::new(
        crate::win95_newspaper::PeNewspaperData::load(
            &dir.join("data_usa.dat"),
            &dir.join("data_usa.idx"),
        )
        .unwrap(),
    );
    let mut state = test_state(City::new_blank("/tmp/citygame-pe-accent.sc2", "Café").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.menu_checks.citygame_newspaper = true;
    state.pe_newspaper_data = Some(data);
    state.pe_document_tick_word = Some(-690);
    state.pe_process_crt = Win95CrtRand::new(123);
    let mut tables = crate::city::PeNewspaperTables {
        papers: [0; 30],
        stories: [0; 54],
    };
    tables.papers[3] = 4;
    for row in 0..9 {
        let id = if row == 7 {
            0
        } else if row == 8 {
            44
        } else {
            42
        };
        tables.stories[row * 6..row * 6 + 6].copy_from_slice(&[id, 0, 0, 255, 255, 255]);
    }
    state.city.install_pe_newspaper_tables(tables);
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Founding));
    state.prepare_pe_newspaper_first_paint();
    assert!(state.pe_newspaper_paint.is_some());
    assert!(state.open_pe_newspaper_article(4));
    assert!(state.pe_newspaper_article.is_some());
    assert_eq!(state.city.name, "Café");
}

#[test]
#[ignore = "requires user-supplied Win95 assets and private live capture"]
fn win95_newspaper_lossy_city_name_matches_live_paper_and_saves() {
    let dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_PE_OEM_ROOT").unwrap());
    let native: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("paper.json")).unwrap()).unwrap();
    let bytes = |s: &str| -> Vec<u8> {
        s.as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect()
    };
    let mut tables = crate::city::PeNewspaperTables {
        papers: [0; 30],
        stories: [0; 54],
    };
    for (slot, byte) in tables
        .papers
        .iter_mut()
        .zip(bytes(native["paper_bytes"].as_str().unwrap()))
    {
        *slot = u32::from(byte);
    }
    for (row, record) in bytes(native["story_bytes"].as_str().unwrap())
        .chunks_exact(8)
        .enumerate()
    {
        tables.stories[row * 6..row * 6 + 6].copy_from_slice(&[
            u16::from_le_bytes([record[0], record[1]]) as u32,
            u16::from_le_bytes([record[2], record[3]]) as u32,
            record[4] as u32,
            255,
            255,
            255,
        ]);
    }
    let mut city = City::new_blank("/tmp/citygame-pe-lossy.sc2", "Café – €").unwrap();
    // The retained post-open snapshot records the native weather row's input.
    city.pe_b_weather_trend = tables.stories[7 * 6 + 2] as u8;
    city.install_pe_newspaper_tables(tables);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.menu_checks.citygame_newspaper = true;
    state.pe_document_tick_word = Some(native["document_tick"].as_i64().unwrap() as i16);
    state.pe_newspaper_data = Some(std::sync::Arc::new(
        crate::win95_newspaper::PeNewspaperData::load(
            &dir.join("data_usa.dat"),
            &dir.join("data_usa.idx"),
        )
        .unwrap(),
    ));
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Founding));
    state.prepare_pe_newspaper_first_paint();
    let paper = state.pe_newspaper_paint.as_ref().expect("paper rendered");
    let headline = crate::win95_oem::decode_oem(&paper.lead.headline);
    assert_eq!(headline, "Crowd Celebrates Café - ?'S Founding");
    assert_eq!(
        crate::city::win1252::encode(&state.city.name, usize::MAX),
        bytes(
            std::fs::read_to_string(root.join("name-after.hex"))
                .unwrap()
                .trim()
        )
    );
    let native_rows: Vec<u32> = bytes(native["story_bytes"].as_str().unwrap())
        .chunks_exact(8)
        .flat_map(|r| {
            [
                u16::from_le_bytes([r[0], r[1]]) as u32,
                u16::from_le_bytes([r[2], r[3]]) as u32,
                r[4] as u32,
                r[5] as u32,
                r[6] as u32,
                r[7] as u32,
            ]
        })
        .collect();
    assert_eq!(
        state.city.pe_newspaper_tables().unwrap().stories.as_slice(),
        native_rows
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("paper-state/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        u64::from(state.pe_process_crt.seed()),
        manifest["crt_tls_candidates"][0]["holdrand"]
            .as_u64()
            .unwrap()
    );
    let path = std::env::temp_dir().join(format!("citygame-pe-lossy-{}.sc2", std::process::id()));
    state.city.save(&path).unwrap();
    let reloaded = City::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(reloaded.name, state.city.name);
}

#[test]
#[ignore = "requires user-supplied Win95 assets and private live capture"]
fn win95_first_paper_captured_tick_diagnostic() {
    let data_dir = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
    let bootstrap =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY").unwrap());
    // The native capture recorded the document tick, but not the two view
    // ticks. Use a placeholder for those and treat this as a diagnostic only.
    let (lfsr, game_rand) = super::super::runtime::win95_view_randoms_from_tick_source(|| 1);
    let (tick, crt, tables) =
        super::super::runtime::win95_document_randoms_from_tick_source(|| 0xfd4e);
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        bootstrap,
        Some(crt),
        Some(game_rand),
        Some(lfsr),
        Some(tables),
    )
    .unwrap();
    state.pe_document_tick_word = Some(tick);
    state.pe_newspaper_data = Some(std::sync::Arc::new(
        crate::win95_newspaper::PeNewspaperData::load(
            &data_dir.join("data_usa.dat"),
            &data_dir.join("data_usa.idx"),
        )
        .unwrap(),
    ));
    state
        .new_city_from_title(
            "New City".into(),
            20_000,
            1900,
            1,
            0,
            title_screen::NewCityStartFlow::GenerateAndPlay,
        )
        .unwrap();
    let before = state.city.pe_newspaper_tables().unwrap();
    state.prepare_pe_newspaper_first_paint();
    let after = state.city.pe_newspaper_tables().unwrap();
    eprintln!(
        "papers={:?} before={:?} after={:?} crt={} composed={}",
        before.papers,
        before.stories,
        after.stories,
        state.pe_process_crt.seed(),
        state.pe_newspaper_paint.is_some()
    );
    assert!(state.pe_newspaper_paint.is_some());
    assert_eq!(
        before.papers,
        [
            0, 1, 0, 4, 3, 5, 0, 1, 1, 0, 4, 0, 2, 0, 1, 2, 1, 0, 2, 4, 1, 2, 1, 5, 5, 3, 2, 2, 3,
            2
        ]
    );
    assert_eq!(
        after.stories,
        [
            2, 1000, 0, 255, 255, 255, 11, 0, 0, 255, 255, 255, 12, 0, 0, 255, 255, 255, 13, 0, 0,
            4, 17, 255, 14, 0, 0, 255, 255, 255, 15, 0, 0, 255, 255, 255, 16, 0, 0, 255, 255, 255,
            0, 0, 4, 255, 255, 255, 44, 0, 0, 255, 255, 255
        ]
    );
    assert_eq!(state.pe_process_crt.seed(), 4_012_721_191);
}

#[test]
fn movable_panels_keep_hit_coordinates_aligned() {
    let city = City::new_blank("/tmp/movable-panels.sc2", "Oak Bay").unwrap();
    let mut state = test_state(city);
    let size = (1280, 800);
    state.city_info_dialog = Some(CityInfoDialog::Graphs);
    let (base, origin, _) = state.info_panel_transform(size).unwrap();
    let start = origin + Vec2::new(20.0, 12.0);
    assert!(state.begin_info_panel_drag(start, size));
    assert!(state.drag_info_panel(start + Vec2::new(70.0, 40.0), size));
    state.info_panel_drag = None;
    let (_, moved, scale) = state.info_panel_transform(size).unwrap();
    assert_eq!(moved, origin + Vec2::new(70.0, 40.0));
    let corner = moved + Vec2::new(base.w * scale - 2.0, base.h * scale - 2.0);
    assert!(state.begin_info_panel_drag(corner, size));
    assert!(state.drag_info_panel(corner - Vec2::new(100.0, 100.0), size));
    state.info_panel_drag = None;
    let (_, moved, scale) = state.info_panel_transform(size).unwrap();
    assert!(scale < 1.0);
    let close = city_info_close_rect(base);
    let logical = Vec2::new(close.x + close.w / 2.0, close.y + close.h / 2.0);
    let actual = moved + (logical - Vec2::new(base.x, base.y)) * scale;
    assert_eq!(
        city_info_dialog_hit_test(
            state.info_panel_input_point(actual, size),
            size,
            CityInfoDialog::Graphs
        ),
        Some(CityInfoDialogAction::Close)
    );
    for dialog in [
        CityInfoDialog::Map,
        CityInfoDialog::Graphs,
        CityInfoDialog::Population,
        CityInfoDialog::Industry,
        CityInfoDialog::Neighbors,
        CityInfoDialog::Budget,
    ] {
        state.city_info_dialog = Some(dialog);
        let (base, origin, scale) = state.info_panel_transform(size).unwrap();
        let close = if dialog == CityInfoDialog::Budget {
            budget_window::close_rect(base)
        } else {
            city_info_close_rect(base)
        };
        let point = Vec2::new(close.x + close.w * 0.5, close.y + close.h * 0.5);
        let transformed = origin + (point - Vec2::new(base.x, base.y)) * scale;
        assert_eq!(
            city_info_dialog_hit_test(
                state.info_panel_input_point(transformed, size),
                size,
                dialog
            ),
            Some(CityInfoDialogAction::Close)
        );
    }
    let status_start = Vec2::new(size.0 as f32 - 30.0, MenubarUi::occupied_top(1.0) + 20.0);
    assert!(state.begin_status_panel_drag(status_start, size, 1.0));
    let target = Vec2::new(40.0, MenubarUi::occupied_top(1.0) + 50.0);
    assert!(state.drag_status_panel(target));
    state.end_status_panel_drag();
    assert!(state.point_in_status_panel(target, size, 1.0));
}

#[test]
fn startup_status_panel_shows_tool_notice_and_weather_within_window_bounds() {
    let city = City::new_blank("/tmp/startup-status.sc2", "Oak Bay").unwrap();
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    state.refresh_idle_status_line();
    assert_eq!(state.status_panel_notice(), "Power Plant Needed");
    let title = state.window_title_base();
    assert!(!title.contains("engine="));
    assert!(!title.contains("anim"));
    assert!(!title.contains("Road"));
    assert!(title.contains("Oak Bay"));
    state.set_sim_status_announcement("Power shortage");
    assert_eq!(state.status_panel_notice(), "Power shortage");
    for size in [(320, 200), (1280, 800), (2624, 1644)] {
        for ps in [1.0, 2.0] {
            for dark in [false, true] {
                state.dark_appearance = dark;
                let mut solids = Vec::new();
                state.push_status_panel(size, ps, &mut solids, None, &mut Vec::new());
                assert!(!solids.is_empty());
                for solid in solids {
                    assert!(solid.x >= 0.0 && solid.y >= 0.0);
                    assert!(solid.x + solid.w <= size.0 as f32 + 0.1);
                    assert!(solid.y + solid.h <= size.1 as f32 + 0.1);
                }
            }
        }
    }
}

#[test]
fn city_information_actions_map_to_the_recovered_dialogs() {
    let cases = [
        (UiAction::OpenCityMap, CityInfoDialog::Map),
        (UiAction::OpenCityPopulation, CityInfoDialog::Population),
        (UiAction::OpenCityNeighbors, CityInfoDialog::Neighbors),
        (UiAction::OpenCityGraphs, CityInfoDialog::Graphs),
        (UiAction::OpenCityIndustry, CityInfoDialog::Industry),
        (UiAction::OpenBudget, CityInfoDialog::Budget),
        (UiAction::OpenHelp, CityInfoDialog::Help),
    ];
    for (action, dialog) in cases {
        assert_eq!(CityInfoDialog::for_action(action), Some(dialog));
    }
    assert_eq!(CityInfoDialog::for_action(UiAction::ZoomIn), None);
}

#[test]
fn native_window_menu_commands_open_city_information_dialogs() {
    let cases = [
        (40, CityInfoDialog::Map),
        (41, CityInfoDialog::Budget),
        (42, CityInfoDialog::Population),
        (43, CityInfoDialog::Industry),
        (44, CityInfoDialog::Graphs),
        (45, CityInfoDialog::Neighbors),
        (46, CityInfoDialog::Ordinances),
        (47, CityInfoDialog::Scenario),
    ];

    for (command, dialog) in cases {
        assert_eq!(
            app_command_for_menu_command(command),
            Some(AppCommand::OpenCityInfo(dialog))
        );
    }
    assert_eq!(app_command_for_menu_command(48), None);
}

#[test]
fn native_file_menu_commands_cover_city_and_tile_set_actions() {
    let cases = [
        (1, AppCommand::OpenCityDialog),
        (2, AppCommand::ReloadAssets),
        (13, AppCommand::SaveCity),
        (14, AppCommand::SaveCityAs),
        (15, AppCommand::NewCity),
        (16, AppCommand::ToggleEditMap),
        (17, AppCommand::OpenTileSetDialog),
        (18, AppCommand::OpenScenarioDialog),
        (19, AppCommand::ReturnToTitle),
    ];
    for (command, app_command) in cases {
        assert_eq!(app_command_for_menu_command(command), Some(app_command));
    }
}

#[test]
fn native_speed_menu_commands_cover_game_speeds() {
    let cases = [
        (20, GameSpeed::Paused),
        (21, GameSpeed::Turtle),
        (22, GameSpeed::Llama),
        (23, GameSpeed::Cheetah),
        (24, GameSpeed::AfricanSwallow),
    ];
    for (command, speed) in cases {
        assert_eq!(
            app_command_for_menu_command(command),
            Some(AppCommand::SetSpeed(speed))
        );
    }
    assert_eq!(app_command_for_menu_command(25), None);
}

#[test]
fn native_newspaper_menu_commands_toggle_extra_and_subscription_and_open_courier() {
    assert_eq!(
        app_command_for_menu_command(70),
        Some(AppCommand::ToggleNewspaperSubscription)
    );
    assert_eq!(
        app_command_for_menu_command(71),
        Some(AppCommand::OpenCityInfo(CityInfoDialog::Newspaper(
            NewspaperEdition::Courier
        )))
    );
    assert_eq!(
        app_command_for_menu_command(72),
        Some(AppCommand::ToggleNewspaperExtra)
    );
    assert_eq!(app_command_for_menu_command(73), None);
}

#[test]
fn newspaper_courier_masthead_uses_pe_paper_choice_name() {
    assert_eq!(
        NewspaperEdition::Courier.masthead("Maxis Land", 0),
        "The Maxis Land Times"
    );
    assert_eq!(
        NewspaperEdition::Courier.masthead("Maxis Land", 2),
        "The Maxis Land Courier"
    );
    assert_eq!(
        NewspaperEdition::Courier.masthead("Maxis Land", 5),
        "The Maxis Land Journal"
    );
    assert_eq!(
        NewspaperEdition::Courier.masthead("Maxis Land", 99),
        "The Maxis Land Journal"
    );
    assert_eq!(
        NewspaperEdition::Extra.masthead("Maxis Land", 2),
        "Maxis Land Extra!!!"
    );
}

#[test]
fn win95_masthead_uses_the_saved_paper_name_index() {
    let mut city = City::new_blank("/tmp/win95-paper-names.sc2", "New City").unwrap();
    let mut tables = crate::city::PeNewspaperTables {
        papers: [0; 30],
        stories: [0; 54],
    };
    tables.papers[0] = 0;
    tables.papers[5] = 5;
    tables.papers[10] = 1;
    city.install_pe_newspaper_tables(tables);
    assert_eq!(
        pe_newspaper_masthead(&city, 0).as_deref(),
        Some("New City Picayune")
    );
    assert_eq!(
        pe_newspaper_masthead(&city, 1).as_deref(),
        Some("New City Chronicle")
    );
    assert_eq!(
        pe_newspaper_masthead(&city, 2).as_deref(),
        Some("New City Courier")
    );
    assert_eq!(pe_newspaper_masthead(&city, 6), None);
}

#[test]
fn newspaper_reward_stories_count_available_reward_bits() {
    let population = PopulationStats {
        total_population: 12_345,
        ..Default::default()
    };

    assert!(
        NewspaperEdition::Extra
            .story("Maxis Land", &population, 0, 0b0_0101)
            .contains("2 available mayoral rewards")
    );
    assert!(
        NewspaperEdition::Courier
            .story("Maxis Land", &population, 0, 0b0_1000)
            .contains("1 available reward")
    );
}

#[test]
fn native_help_menu_command_opens_the_help_dialog() {
    assert_eq!(
        app_command_for_menu_command(80),
        Some(AppCommand::OpenCityInfo(CityInfoDialog::Help))
    );
    assert_eq!(
        app_command_for_menu_command(81),
        Some(AppCommand::ToggleBalloonHelp)
    );
    assert_eq!(app_command_for_menu_command(82), None);
}

#[test]
fn native_options_menu_command_toggles_auto_budget() {
    let cases = [
        (50, AppCommand::ToggleAutoBudget),
        (51, AppCommand::ToggleAutoGoto),
        (52, AppCommand::ToggleSoundEffects),
        (53, AppCommand::ToggleMusic),
    ];
    for (command, app_command) in cases {
        assert_eq!(app_command_for_menu_command(command), Some(app_command));
    }
    assert_eq!(app_command_for_menu_command(54), None);
}

#[test]
fn state_app_commands_apply_runtime_side_effects_without_sdl() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.apply_state_app_command(&AppCommand::ZoomIn), None);

    assert_eq!(
        state.apply_state_app_command(&AppCommand::OpenCityInfo(CityInfoDialog::Ordinances)),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Ordinances));

    assert!(!state.show_grid);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleGrid),
        Some(AppCommandStateEffect::default())
    );
    assert!(state.show_grid);

    assert!(!state.show_zone_overlay);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleZoneOverlay),
        Some(AppCommandStateEffect::default())
    );
    assert!(state.show_zone_overlay);

    assert!(state.show_traffic);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleTraffic),
        Some(AppCommandStateEffect {
            log_traffic_stats: true,
            ..Default::default()
        })
    );
    assert!(!state.show_traffic);

    assert!(!state.show_underground);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleUnderground),
        Some(AppCommandStateEffect::default())
    );
    assert!(state.show_underground);
    assert!(!state.show_traffic);

    state.query_is_inspect = true;
    assert_eq!(
        state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Paused)),
        Some(AppCommandStateEffect {
            update_title: true,
            sync_renderer_to_window: true,
            sync_palette_stacking: true,
            ..Default::default()
        })
    );
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(1));
    assert!(!state.query_is_inspect);
    assert!(state.query_line.contains("*PAUSED*"));

    assert!(state.auto_goto_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleAutoGoto),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(!state.auto_goto_enabled);
    assert!(!auto_goto_enabled_for_city(&state.city));

    assert!(state.sound_effects_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleSoundEffects),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(!state.sound_effects_enabled);
    assert!(!sound_effects_enabled_for_city(&state.city));

    assert!(state.music_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleMusic),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(!state.music_enabled);
    assert!(!music_enabled_for_city(&state.city));

    assert!(!auto_budget_enabled_for_city(&state.city));
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleAutoBudget),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(auto_budget_enabled_for_city(&state.city));

    assert!(!state.balloon_help_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleBalloonHelp),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(state.balloon_help_enabled);

    assert_eq!(
        state.apply_state_app_command(&AppCommand::SetTool(EditTool::Road)),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert_eq!(state.tool, EditTool::Road);

    assert_eq!(
        state.apply_state_app_command(&AppCommand::RotateView(1)),
        Some(AppCommandStateEffect::default())
    );
    assert_eq!(state.city.rotation(), 1);

    state.city.set_building(24, 24, 0x70);
    state.hover_tile = Some((12, 34));
    assert_eq!(
        state.apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Fire)),
        Some(AppCommandStateEffect {
            update_title: true,
            placement_sound: Some(PlacementSound::Fire),
            disaster_log: Some((DisasterKind::Fire, true)),
            ..Default::default()
        })
    );
    assert!(state.city.has_active_disaster());

    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleNoDisasters),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(state.no_disasters_enabled);
    assert_eq!(
        state.apply_state_app_command(&AppCommand::StartDisaster(DisasterKind::Flood)),
        Some(AppCommandStateEffect {
            disaster_log: Some((DisasterKind::Flood, false)),
            ..Default::default()
        })
    );
}

#[test]
fn city_information_dialogs_toggle_and_close_from_the_original_close_button() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_city_info_dialog(CityInfoDialog::Map);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Map));
    assert!(
        state
            .checked_palette_actions()
            .contains(&UiAction::OpenCityMap)
    );
    state.open_city_info_dialog(CityInfoDialog::Graphs);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Graphs));
    assert!(
        state
            .checked_palette_actions()
            .contains(&UiAction::OpenCityGraphs)
    );
    state.open_city_info_dialog(CityInfoDialog::Graphs);
    assert_eq!(state.city_info_dialog, None);

    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let close = city_info_close_rect(dialog);
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(close.x + close.w * 0.5, close.y + close.h * 0.5),
            window_size,
            CityInfoDialog::Map,
        ),
        Some(CityInfoDialogAction::Close)
    );
}

#[test]
fn newspaper_close_button_tracks_the_scaled_page_at_all_window_sizes() {
    let dialog = CityInfoDialog::Newspaper(NewspaperEdition::Courier);
    for window in [(400, 360), (1024, 768), (2624, 1644)] {
        let page = city_info_dialog_rect_for(dialog, window);
        let close = newspaper::close_rect(page);
        let center = Vec2::new(close.x + close.w * 0.5, close.y + close.h * 0.5);
        assert!(point_in_float_rect(center, page));
        assert_eq!(
            city_info_dialog_hit_test(center, window, dialog),
            Some(CityInfoDialogAction::Close)
        );
        assert_eq!(
            city_info_dialog_hit_test(
                Vec2::new(page.x + 5.0, page.y + page.h * 0.5),
                window,
                dialog
            ),
            None
        );
    }
}

#[test]
fn ordinances_dialog_hit_tests_rows_only_for_ordinances() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let row = ordinance_row_rect(dialog, 3);
    let point = Vec2::new(row.x + 14.0, row.y + 14.0);

    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Ordinances),
        Some(CityInfoDialogAction::ToggleOrdinance(3))
    );
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Budget),
        None
    );
}

#[test]
fn graphs_dialog_hit_tests_legend_toggles_only_for_graphs() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let legend = graph_legend_rect(dialog, 5);
    let point = Vec2::new(legend.x + 8.0, legend.y + 8.0);

    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Graphs),
        Some(CityInfoDialogAction::ToggleGraphSeries(5))
    );
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Map),
        None
    );
}

#[test]
fn graphs_dialog_hit_tests_scale_radios_only_for_graphs() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let ten_year = graph_scale_rect(dialog, 1);
    let point = Vec2::new(ten_year.x + 8.0, ten_year.y + 8.0);

    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Graphs),
        Some(CityInfoDialogAction::SetGraphScale(GraphScale::Years10))
    );
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Map),
        None
    );
    let hundred_year = graph_scale_rect(dialog, 2);
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(hundred_year.x + 8.0, hundred_year.y + 8.0),
            window_size,
            CityInfoDialog::Graphs
        ),
        Some(CityInfoDialogAction::SetGraphScale(GraphScale::Years100))
    );
}

#[test]
fn budget_dialog_hit_tests_tax_and_service_controls_only_for_budget() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect_for(CityInfoDialog::Budget, window_size);
    let tax = budget_tax_adjust_rect(dialog, 1, true);
    let service = budget_service_funding_adjust_rect(dialog, 7, false);
    let auto_budget = budget_auto_budget_rect(dialog);
    let issue_bond = budget_bond_button_rect(dialog, true);
    let repay_bond = budget_bond_button_rect(dialog, false);

    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(tax.x + 8.0, tax.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::AdjustBudgetTax {
            tax_index: 1,
            delta: BUDGET_TAX_STEP
        })
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(service.x + 8.0, service.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::AdjustBudgetFunding {
            service_index: 7,
            delta: -BUDGET_FUNDING_STEP
        })
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(auto_budget.x + 8.0, auto_budget.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::ToggleAutoBudget)
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(issue_bond.x + 8.0, issue_bond.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::IssueBond)
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(repay_bond.x + 8.0, repay_bond.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::RepayBond)
    );
    // Different dialogs can place their own controls at the same screen point.
    // The ordinances dialog must never dispatch a budget-tax action there.
    assert!(!matches!(
        city_info_dialog_hit_test(
            Vec2::new(tax.x + 8.0, tax.y + 8.0),
            window_size,
            CityInfoDialog::Ordinances
        ),
        Some(CityInfoDialogAction::AdjustBudgetTax { .. })
    ));
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(auto_budget.x + 8.0, auto_budget.y + 8.0),
            window_size,
            CityInfoDialog::Map
        ),
        None
    );
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(issue_bond.x + 8.0, issue_bond.y + 8.0),
            window_size,
            CityInfoDialog::Population
        ),
        None
    );
}

#[test]
fn industry_dialog_hit_tests_tax_controls_only_for_industry() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let tax = industry_tax_adjust_rect(dialog, 4, false);

    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(tax.x + 8.0, tax.y + 8.0),
            window_size,
            CityInfoDialog::Industry
        ),
        Some(CityInfoDialogAction::AdjustIndustryTax {
            industry_index: 4,
            delta: -BUDGET_TAX_STEP
        })
    );
    // This screen point overlaps a Budget service button in Budget's own,
    // independently scaled layout. Hit testing must follow the active dialog.
    assert_eq!(
        city_info_dialog_hit_test(
            Vec2::new(tax.x + 8.0, tax.y + 8.0),
            window_size,
            CityInfoDialog::Budget
        ),
        Some(CityInfoDialogAction::AdjustBudgetFunding {
            service_index: 1,
            delta: BUDGET_FUNDING_STEP
        })
    );
}

#[test]
fn newspaper_subscription_and_extra_read_and_toggle_saved_misc_flags() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert!(!newspaper_subscription_enabled_for_city(&city));
    assert!(newspaper_extra_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );

    ensure_default_city_options(&mut city);
    assert!(!newspaper_subscription_enabled_for_city(&city));
    assert!(newspaper_extra_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert!(!set_city_newspaper_extra_enabled(&mut city, true));

    assert!(set_city_newspaper_subscription_enabled(&mut city, true));
    assert!(newspaper_subscription_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert!(!set_city_newspaper_subscription_enabled(&mut city, true));
    assert!(newspaper_extra_enabled_for_city(&city));

    assert!(set_city_newspaper_subscription_enabled(&mut city, false));
    assert!(!newspaper_subscription_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert!(newspaper_extra_enabled_for_city(&city));

    assert!(set_city_newspaper_extra_enabled(&mut city, false));
    assert!(!newspaper_extra_enabled_for_city(&city));
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
}

#[test]
fn mac_newspaper_extra_uses_the_saved_low_byte_on_both_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let city = City::new_blank("/tmp/mac-newspaper-extra.sc2", "Extra").unwrap();
        let mut state = test_state(city);
        state.city.kind = kind;
        state.engine_fidelity = EngineFidelity::Mac68k;
        state
            .city
            .set_mac_state_misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET, 0x100);
        state.sync_edit_map_menu();
        assert!(!state.menu_checks.newspaper_extra);
        let pe_seed = state.pe_process_crt.seed();
        let mac_seed = state.mac_process_rng.seed();

        state.apply_state_app_command(&AppCommand::ToggleNewspaperExtra);
        assert!(state.menu_checks.newspaper_extra);
        assert_eq!(
            state
                .city
                .mac_state_misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
            Some(1)
        );
        state.apply_state_app_command(&AppCommand::ToggleNewspaperExtra);
        assert!(!state.menu_checks.newspaper_extra);
        assert_eq!(
            state
                .city
                .mac_state_misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
            Some(0)
        );
        assert_eq!(state.pe_process_crt.seed(), pe_seed);
        assert_eq!(state.mac_process_rng.seed(), mac_seed);
    }
}

#[test]
fn city_state_toggles_newspaper_subscription_and_extra_saved_flags() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert!(!newspaper_subscription_enabled_for_city(&state.city));
    assert!(newspaper_extra_enabled_for_city(&state.city));

    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleNewspaperSubscription),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(newspaper_subscription_enabled_for_city(&state.city));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert!(newspaper_extra_enabled_for_city(&state.city));

    assert_eq!(
        state.apply_state_app_command(&AppCommand::ToggleNewspaperExtra),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert!(!newspaper_extra_enabled_for_city(&state.city));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );

    state.toggle_newspaper_subscription();
    state.toggle_newspaper_extra();
    assert!(!newspaper_subscription_enabled_for_city(&state.city));
    assert!(newspaper_extra_enabled_for_city(&state.city));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
}

#[test]
fn mac_special_zone_single_tile_helper_preserves_acceptance_and_map_writes() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-special-single.sc2", "SpecialSingle").expect("blank city");
    city.kind = CityFileKind::MacCity;

    assert_eq!(
        mac_budget_try_place_zone_single_tile(&mut city, 40, 40, 8, 0xe1),
        MacSpecialTileAttempt::Refused
    );
    city.set_tile_bits(40, 40, TileBits::new(0x5b));
    assert_eq!(
        mac_budget_try_place_zone_single_tile(&mut city, 40, 40, 8, 0xe1),
        MacSpecialTileAttempt::Placed
    );
    assert_eq!(city.building_at(40, 40), 0xe1);
    assert_eq!(city.zone_at(40, 40).unwrap().raw_bits(), 0xf8);
    assert_eq!(city.tile_bits_at(40, 40).unwrap().raw_bits(), 0xfb);
    assert_eq!(city.mac_xbld_tile_count(0xe1), 1);

    // At or above 0x0d the original helper accepts but leaves all map planes
    // untouched. It does not report a failed placement to the caller.
    city.set_tile_bits(42, 40, TileBits::new(0x40));
    city.set_building(42, 40, 0x0d);
    city.set_zone(42, 40, 0x08);
    assert_eq!(
        mac_budget_try_place_zone_single_tile(&mut city, 42, 40, 8, 0xea),
        MacSpecialTileAttempt::AcceptedWithoutPlacement
    );
    assert_eq!(city.building_at(42, 40), 0x0d);
    assert_eq!(city.zone_at(42, 40).unwrap().raw_bits(), 0x08);
    assert_eq!(city.tile_bits_at(42, 40).unwrap().raw_bits(), 0x40);
}

#[test]
fn mac_special_zone_square_helper_clears_and_stamps_exact_2x2_planes() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-special-square.sc2", "SpecialSquare").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.saved_rotation = 0;
    city.set_view_rotation(1);
    for (x, y) in [(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_zone(x, y, 0x28);
        city.set_tile_bits(x, y, TileBits::new(0x5b));
    }
    // A high ordinary building in the NW cell makes FUN_0003844a clear its
    // aligned 2x2 footprint before the new marker/building writes.
    city.set_building(40, 40, 0xc6);
    city.set_building(41, 40, 0x03);
    city.set_building(40, 41, 0x04);
    city.set_building(41, 41, 0x05);

    assert_eq!(
        mac_budget_try_place_zone_square(&mut city, 41, 41, 8, 0xf2),
        MacSpecialTileAttempt::Placed
    );
    for (x, y, marker) in [
        (40, 40, 0x10),
        (41, 40, 0x20),
        (40, 41, 0x80),
        (41, 41, 0x40),
    ] {
        assert_eq!(city.building_at(x, y), 0xf2);
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), marker | 0x08);
        assert_eq!(city.tile_bits_at(x, y).unwrap().raw_bits(), 0xfb);
    }
    assert_eq!(city.mac_xbld_tile_count(0xc6), 0);
    assert_eq!(city.mac_xbld_tile_count(0xf2), 4);

    for (rotation, markers) in [
        [0x10, 0x20, 0x40, 0x80],
        [0x20, 0x40, 0x80, 0x10],
        [0x40, 0x80, 0x10, 0x20],
        [0x80, 0x10, 0x20, 0x40],
    ]
    .into_iter()
    .enumerate()
    {
        city.saved_rotation = rotation as u8;
        city.set_view_rotation((rotation as u8 + 1) & 3);
        for (x, y) in [(40, 40), (41, 40), (40, 41), (41, 41)] {
            city.set_building(x, y, 0);
        }
        assert_eq!(
            mac_budget_try_place_zone_square(&mut city, 41, 41, 8, 0xf2),
            MacSpecialTileAttempt::Placed
        );
        for (x, y, marker) in [
            (40, 40, markers[0]),
            (41, 40, markers[1]),
            (41, 41, markers[2]),
            (40, 41, markers[3]),
        ] {
            assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), marker | 0x08);
        }
    }

    // The special ray sentinel rejects the whole footprint without writes.
    city.set_building(41, 41, 0xe0);
    assert_eq!(
        mac_budget_try_place_zone_square(&mut city, 40, 40, 8, 0xee),
        MacSpecialTileAttempt::Refused
    );
    assert_eq!(city.building_at(40, 40), 0xf2);
    assert_eq!(city.building_at(41, 41), 0xe0);
}

#[test]
fn mac_special_zone_square_helper_refuses_each_rooted_admission_failure_without_writes() {
    for failure in [
        "no power",
        "zone mismatch",
        "northwest limit",
        "ray sentinel",
    ] {
        let mut city = City::new_blank(
            "/tmp/sc2000-mac-special-square-admission.sc2",
            "SquareAdmission",
        )
        .expect("blank city");
        city.kind = CityFileKind::MacCity;
        for (x, y) in [(40, 40), (41, 40), (40, 41), (41, 41)] {
            city.set_zone(x, y, 8);
            city.set_tile_bits(x, y, TileBits::new(0x1b));
        }
        if failure != "no power" {
            city.set_tile_bits(42, 41, TileBits::new(0x40));
        }
        match failure {
            "zone mismatch" => city.set_zone(41, 41, 9),
            "northwest limit" => city.set_building(40, 40, 0xeb),
            "ray sentinel" => city.set_building(40, 41, 0xe0),
            _ => {}
        }
        let before = [(40, 40), (41, 40), (40, 41), (41, 41)].map(|(x, y)| {
            (
                city.building_at(x, y),
                city.zone_at(x, y).unwrap().raw_bits(),
                city.tile_bits_at(x, y).unwrap().raw_bits(),
            )
        });

        assert_eq!(
            mac_budget_try_place_zone_square(&mut city, 41, 41, 8, 0xf6),
            MacSpecialTileAttempt::Refused,
            "{failure}"
        );
        let after = [(40, 40), (41, 40), (40, 41), (41, 41)].map(|(x, y)| {
            (
                city.building_at(x, y),
                city.zone_at(x, y).unwrap().raw_bits(),
                city.tile_bits_at(x, y).unwrap().raw_bits(),
            )
        });
        assert_eq!(after, before, "{failure} must be an all-or-nothing refusal");
    }
}

#[test]
fn mac_special_zone_runway_helper_extends_existing_markers_until_five_new_tiles() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-special-runway.sc2", "SpecialRunway").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.saved_rotation = 0;
    city.set_view_rotation(1);
    let (x, y) = (41, 40); // odd x selects the positive vertical ray
    for ray_y in y..=y + 5 {
        city.set_zone(x, ray_y, 0x08);
    }
    city.set_tile_bits(x, y, TileBits::new(0x40)); // powered cardinal gate
    city.set_building(x, y + 1, 0xdd);
    city.set_tile_bits(x, y + 1, TileBits::new(0x40)); // opposite orientation marker

    assert_eq!(
        mac_budget_try_place_zone_runway_ray(&mut city, x, y, 8),
        MacSpecialTileAttempt::Placed
    );
    assert_eq!(city.building_at(x, y), 0xdd);
    assert_eq!(city.building_at(x, y + 1), 0xde);
    for ray_y in y + 2..=y + 5 {
        assert_eq!(city.building_at(x, ray_y), 0xdd);
        assert_eq!(city.zone_at(x, ray_y).unwrap().raw_bits(), 0xf8);
        assert_eq!(city.tile_bits_at(x, ray_y).unwrap().raw_bits(), 0xc2);
    }
    assert_eq!(city.zone_at(x, y + 1).unwrap().raw_bits(), 0xf8);
    assert_eq!(city.tile_bits_at(x, y + 1).unwrap().raw_bits(), 0xc0);
    assert_eq!(city.mac_xbld_tile_count(0xdd), 5);
    assert_eq!(city.mac_xbld_tile_count(0xde), 1);
}

#[test]
fn ensure_default_city_options_seeds_pe_newspaper_lists() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(
        city.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS),
        vec![0; NEWSPAPER_LIST_MISC_DWORDS]
    );
    assert_eq!(
        city.misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            NEWSPAPER_STORY_LIST_MISC_DWORDS
        ),
        vec![0; NEWSPAPER_STORY_LIST_MISC_DWORDS]
    );

    ensure_default_city_options(&mut city);

    let papers = city.misc_u32_values(MISC_NEWSPAPER_LIST_OFFSET, NEWSPAPER_LIST_MISC_DWORDS);
    assert_eq!(papers, pe_newspaper_paper_list_misc_values(&city));
    let mut identity: Vec<u32> = pe_newspaper_paper_list_identity_bytes()
        .into_iter()
        .map(u32::from)
        .collect();
    assert_ne!(
        papers, identity,
        "PE newspaper init shuffles the identity 6×5 list"
    );
    let mut shuffled = papers.clone();
    shuffled.sort_unstable();
    identity.sort_unstable();
    assert_eq!(shuffled, identity);

    let stories = city.misc_u32_values(
        MISC_NEWSPAPER_STORY_LIST_OFFSET,
        NEWSPAPER_STORY_LIST_MISC_DWORDS,
    );
    assert_eq!(stories, pe_newspaper_story_list_misc_values());
    assert_eq!(stories[0], 0x0B);
    assert_eq!(stories[5], 0xFF);
    assert_eq!(stories[48], 0x13);
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(CITY_OPTION_ENABLED_VALUE)
    );
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_SUBSCRIPTION_OFFSET),
        Some(CITY_OPTION_DISABLED_VALUE)
    );
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(0));

    city.set_misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET, 9);
    ensure_default_city_options(&mut city);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET), Some(9));
    assert_eq!(
        city.misc_u32_values(
            MISC_NEWSPAPER_STORY_LIST_OFFSET,
            NEWSPAPER_STORY_LIST_MISC_DWORDS
        ),
        pe_newspaper_story_list_misc_values()
    );
}

#[test]
fn pe_newspaper_startup_matches_native_shuffle_and_new_city_preserves_it() {
    let mut crt = Win95CrtRand::new(1);
    let tables = super::super::newspaper_state::initialize_pe_newspaper_tables(&mut crt);
    let expected = [
        4, 1, 0, 4, 2, 1, 0, 1, 3, 1, 2, 0, 0, 5, 3, 5, 2, 2, 2, 0, 3, 1, 1, 0, 4, 0, 2, 2, 1, 5,
    ];
    assert_eq!(crt.seed(), 3_018_468_955);
    assert_eq!(tables.papers, expected);

    let path = std::env::temp_dir().join(format!(
        "citygame-paper-bootstrap-{}.sc2",
        std::process::id()
    ));
    let mut placeholder = City::new_blank(&path, "Placeholder").unwrap();
    placeholder.set_building(40, 40, 0xcc);
    placeholder.set_tile_bits(40, 40, TileBits::new(0x88));
    placeholder.set_misc_u32_at(MISC_NEWSPAPER_LIST_OFFSET, 99);
    placeholder.save(&path).unwrap();
    let startup = ViewerState::load_default_from_path_with_process_rng(
        path.clone(),
        Some(crt),
        None,
        None,
        Some(tables),
    );
    std::fs::remove_file(path).unwrap();
    let mut state = startup.unwrap();
    if !state.engine_fidelity.uses_win95() {
        return;
    }
    assert_eq!(state.city.pe_newspaper_tables().unwrap().papers, expected);
    assert_eq!(state.pe_process_crt_seed(), crt.seed());
    let mut preview = crt;
    // The existing direct New City command advances the fame-rank stream.
    let mut preview_city = state.city.clone();
    preview_city.pe_reseed_neighbor_fame_ranks_42e585(&mut preview);

    for _ in 0..2 {
        state.new_city().expect("new city");
        let tables = state.city.pe_newspaper_tables().unwrap();
        assert_eq!(tables.papers, expected);
        assert_eq!(tables.stories, pe_newspaper_story_list_misc_values());
        assert_eq!(state.pe_process_crt_seed(), preview.seed());
        preview_city.pe_reseed_neighbor_fame_ranks_42e585(&mut preview);
    }
}

#[test]
fn pe_newspaper_open_copies_zero_tables_and_keeps_mac_misc_separate() {
    let mut state = test_state(City::new_blank("/tmp/citygame-paper-test.sc2", "test").unwrap());
    let mut crt = Win95CrtRand::new(1);
    state.city.install_pe_newspaper_tables(
        super::super::newspaper_state::initialize_pe_newspaper_tables(&mut crt),
    );

    for extension in ["sc2", "city"] {
        let path = std::env::temp_dir().join(format!(
            "citygame-paper-tables-{}.{extension}",
            std::process::id()
        ));
        let mut saved = blank_city_layout_fixture(&path, "Tables");
        if extension == "city" {
            saved.set_mac_state_misc_u32_at(MISC_NEWSPAPER_STORY_LIST_OFFSET, 0x1234_5678);
        }
        saved.save(&path).unwrap();
        state.pe_process_crt = Win95CrtRand::new(1);
        let opened = state.load_city(&path, (1280, 800), 1.0);
        std::fs::remove_file(&path).unwrap();
        opened.unwrap();
        let tables = state.city.pe_newspaper_tables().unwrap();
        assert_eq!(tables.papers, [0; NEWSPAPER_LIST_MISC_DWORDS]);
        assert_eq!(tables.stories, [0; NEWSPAPER_STORY_LIST_MISC_DWORDS]);
        assert_eq!(state.pe_process_crt_seed(), 2_503_793_637);
    }

    state.city.queue_pe_newspaper_story(2, 7);
    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(2));
    assert_eq!(state.city.pe_newspaper_story_id_at(7), Some(0));
    assert_eq!(
        state
            .city
            .mac_state_misc_u32_at(MISC_NEWSPAPER_STORY_LIST_OFFSET),
        Some(0x1234_5678)
    );
    state.new_city().expect("new city");
    let tables = state.city.pe_newspaper_tables().unwrap();
    assert_eq!(tables.papers, [0; NEWSPAPER_LIST_MISC_DWORDS]);
    assert_eq!(tables.stories[0], 2);
    assert_eq!(tables.stories[2], 7);
}

#[test]
fn newspaper_option_writes_preserve_the_paper_and_story_tables() {
    let mut city = City::new_blank("/tmp/citygame-paper-test.sc2", "test").unwrap();
    assert!(set_city_newspaper_extra_enabled(&mut city, false));
    let tables = city.pe_newspaper_tables().unwrap();
    assert_eq!(tables.papers, [0; NEWSPAPER_LIST_MISC_DWORDS]);
    assert_eq!(tables.stories, [0; NEWSPAPER_STORY_LIST_MISC_DWORDS]);
}

#[test]
fn pe_newspaper_identity_records_are_i_imod3() {
    let recs = pe_newspaper_paper_list_identity_bytes();
    for index in 0..NEWSPAPER_LIST_RECORD_COUNT {
        let rec = index * NEWSPAPER_LIST_RECORD_BYTES;
        let i = index as u8;
        assert_eq!(recs[rec], i);
        assert_eq!(recs[rec + 1], i % 3);
        assert_eq!(recs[rec + 2], i % 3);
        assert_eq!(recs[rec + 3], i);
        assert_eq!(recs[rec + 4], i);
    }
}

#[test]
fn pe_newspaper_shuffle_first_swaps_follow_random_bits() {
    let mut rng = MacToolboxRng::new(0x4797_3001);
    let mut expected = MacToolboxRng::new(0x4797_3001);
    let recs = pe_newspaper_paper_list_shuffled_bytes(&mut rng);
    let mut ident = pe_newspaper_paper_list_identity_bytes();
    let bit = usize::from(expected.random_u16() & 1);
    ident.swap(3, 0x12 + bit * NEWSPAPER_LIST_RECORD_BYTES);
    if expected.random_u16() & 1 != 0 {
        ident.swap(1, 6);
    }
    for _ in 0..PE_NEWSPAPER_SHUFFLE_ROUNDS {
        let a = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES;
        let b = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES;
        ident.swap(a, b);
        let a = pe_newspaper_random_mod(&mut expected, 5) * NEWSPAPER_LIST_RECORD_BYTES + 6;
        let b = pe_newspaper_random_mod(&mut expected, 5) * NEWSPAPER_LIST_RECORD_BYTES + 6;
        ident.swap(a, b);
        let a = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES + 2;
        let b = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES + 2;
        ident.swap(a, b);
        let a = pe_newspaper_random_mod(&mut expected, 5) * NEWSPAPER_LIST_RECORD_BYTES + 8;
        let b = pe_newspaper_random_mod(&mut expected, 5) * NEWSPAPER_LIST_RECORD_BYTES + 8;
        ident.swap(a, b);
        let a = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES + 4;
        let b = pe_newspaper_random_mod(&mut expected, 6) * NEWSPAPER_LIST_RECORD_BYTES + 4;
        ident.swap(a, b);
    }
    assert_eq!(recs, ident);
    assert_eq!(PE_NEWSPAPER_SHUFFLE_ROUNDS, 0x0C);
}

#[test]
fn pe_newspaper_shuffle_differs_by_city_name() {
    let times = City::new_blank("/tmp/sc2000-test.sc2", "Times").expect("blank city");
    let herald = City::new_blank("/tmp/sc2000-test.sc2", "Herald").expect("blank city");
    assert_ne!(
        pe_newspaper_paper_list_misc_values(&times),
        pe_newspaper_paper_list_misc_values(&herald)
    );
}

#[test]
fn newspaper_max_choice_follows_pe_city_progression() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(newspaper_max_choice_for_city(&city), 0);

    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 0);
    assert_eq!(newspaper_max_choice_for_city(&city), 0);
    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 1);
    assert_eq!(newspaper_max_choice_for_city(&city), 1);
    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 2);
    assert_eq!(newspaper_max_choice_for_city(&city), 2);
    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 5);
    assert_eq!(newspaper_max_choice_for_city(&city), NEWSPAPER_CHOICE_MAX);
    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 9);
    assert_eq!(newspaper_max_choice_for_city(&city), NEWSPAPER_CHOICE_MAX);
}

#[test]
fn newspaper_choice_reads_and_clamps_saved_misc_index() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    assert_eq!(newspaper_choice_for_city(&city), 0);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(0));
    assert_eq!(newspaper_paper_name(0), "Times");
    assert_eq!(newspaper_paper_name(2), "Courier");

    ensure_default_city_options(&mut city);
    assert_eq!(newspaper_choice_for_city(&city), 0);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(0));

    assert!(
        !set_city_newspaper_choice(&mut city, 3),
        "locked Picayune should stay Times (progression 0, already seeded)"
    );
    assert_eq!(newspaper_choice_for_city(&city), 0);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(0));

    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, NEWSPAPER_CHOICE_MAX);
    assert!(set_city_newspaper_choice(&mut city, 3));
    assert_eq!(newspaper_choice_for_city(&city), 3);
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(3));
    assert!(!set_city_newspaper_choice(&mut city, 3));
    assert_eq!(
        newspaper_paper_name(newspaper_choice_for_city(&city)),
        "Picayune"
    );

    assert!(set_city_newspaper_choice(&mut city, 99));
    assert_eq!(newspaper_choice_for_city(&city), NEWSPAPER_CHOICE_MAX);
    assert_eq!(
        city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(NEWSPAPER_CHOICE_MAX)
    );
    assert_eq!(
        newspaper_paper_name(newspaper_choice_for_city(&city)),
        "Journal"
    );

    city.set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, 1);
    assert_eq!(newspaper_choice_for_city(&city), 1);
    assert!(set_city_newspaper_choice(&mut city, 5));
    assert_eq!(city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET), Some(1));
}

#[test]
fn opening_newspaper_dialog_persists_pe_choice_misc() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(0)
    );

    assert_eq!(
        state.apply_state_app_command(&AppCommand::OpenCityInfo(CityInfoDialog::Newspaper(
            NewspaperEdition::Courier
        ))),
        Some(AppCommandStateEffect {
            update_title: true,
            ..Default::default()
        })
    );
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(0)
    );
    assert_eq!(newspaper_choice_for_city(&state.city), 0);
    assert_eq!(
        NewspaperEdition::Courier
            .masthead(&state.city.name, newspaper_choice_for_city(&state.city)),
        format!("The {} Times", state.city.name)
    );

    state
        .city
        .set_misc_u32_at(MISC_CITY_PROGRESSION_OFFSET, NEWSPAPER_COURIER_CHOICE);
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Courier));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(NEWSPAPER_COURIER_CHOICE)
    );
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Extra));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET),
        Some(NEWSPAPER_COURIER_CHOICE)
    );
    assert_eq!(
        NewspaperEdition::Courier
            .masthead(&state.city.name, newspaper_choice_for_city(&state.city)),
        format!("The {} Courier", state.city.name)
    );
}

#[test]
fn city_state_toggles_balloon_help() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert!(!state.balloon_help_enabled);
    assert!(!state.set_palette_balloon_help(Some("Road")));
    assert_eq!(state.balloon_help_line, None);

    state.toggle_balloon_help();
    assert!(state.balloon_help_enabled);
    assert!(state.set_palette_balloon_help(Some("Road")));
    assert_eq!(state.balloon_help_line, Some("Road"));
    assert!(!state.set_palette_balloon_help(Some("Road")));
    assert!(state.set_palette_balloon_help(None));
    assert_eq!(state.balloon_help_line, None);

    assert!(state.set_palette_balloon_help(Some("Pipes")));
    assert_eq!(state.balloon_help_line, Some("Pipes"));
    state.toggle_balloon_help();
    assert!(!state.balloon_help_enabled);
    assert_eq!(state.balloon_help_line, None);
}

#[test]
fn city_state_toggles_ordinances_through_dialog_action() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.toggle_ordinance(12);
    assert!(ordinance_enabled_for_city(&state.city, 12));

    state.toggle_ordinance(12);
    assert!(!ordinance_enabled_for_city(&state.city, 12));
}

#[test]
fn city_state_toggles_graph_series_without_dirtying_city() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.clear_dirty();

    assert!(state.graph_series_visible.iter().all(|visible| *visible));
    assert!(state.toggle_graph_series(2));
    assert!(!state.graph_series_visible[2]);
    assert!(state.toggle_graph_series(2));
    assert!(state.graph_series_visible[2]);
    assert!(!state.toggle_graph_series(GRAPH_HISTORY_TOTAL_SERIES));
    assert!(!state.city.has_dirty_tiles());
}

#[test]
fn city_state_sets_graph_scale_without_dirtying_city() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.city.clear_dirty();

    assert_eq!(state.graph_scale, GraphScale::Year1);
    assert!(state.set_graph_scale(GraphScale::Years10));
    assert_eq!(state.graph_scale, GraphScale::Years10);
    assert!(!state.set_graph_scale(GraphScale::Years10));
    assert!(state.set_graph_scale(GraphScale::Years100));
    assert_eq!(state.graph_scale, GraphScale::Years100);
    assert!(!state.city.has_dirty_tiles());
}

#[test]
fn graph_panel_reads_xgrp_monthly_samples_oldest_first() {
    let mut values = vec![0_u32; XGRP_VALUES];
    for month in 0..XGRP_MONTH_SAMPLES {
        values[month] = (XGRP_MONTH_SAMPLES - month) as u32;
    }
    let crime_base = 7 * XGRP_SAMPLES_PER_SERIES;
    values[crime_base] = 99;
    values[crime_base + XGRP_MONTH_SAMPLES - 1] = 1;

    let samples = xgrp_samples_oldest_first(&values, 0, GraphScale::Year1);
    assert_eq!(samples, (1..=XGRP_MONTH_SAMPLES as u32).collect::<Vec<_>>());
    assert_eq!(samples[0], 1, "left/oldest is PE monthly index 11");
    assert_eq!(
        samples[XGRP_MONTH_SAMPLES - 1],
        XGRP_MONTH_SAMPLES as u32,
        "right/newest is PE monthly index 0"
    );
    assert_eq!(
        xgrp_samples_oldest_first(&values, 7, GraphScale::Year1)[0],
        1
    );
    assert_eq!(
        xgrp_samples_oldest_first(&values, 7, GraphScale::Year1)[XGRP_MONTH_SAMPLES - 1],
        99
    );
}

#[test]
fn graph_panel_reads_xgrp_half_year_and_five_year_samples_oldest_first() {
    let mut values = vec![0_u32; XGRP_VALUES];
    let half_year = GraphScale::Years10.xgrp_offset();
    let five_year = GraphScale::Years100.xgrp_offset();
    for sample in 0..XGRP_HALF_YEAR_SAMPLES {
        values[half_year + sample] = (XGRP_HALF_YEAR_SAMPLES - sample) as u32;
    }
    for sample in 0..XGRP_FIVE_YEAR_SAMPLES {
        values[five_year + sample] = 200 - sample as u32;
    }

    let ten_year = xgrp_samples_oldest_first(&values, 0, GraphScale::Years10);
    assert_eq!(ten_year.len(), XGRP_HALF_YEAR_SAMPLES);
    assert_eq!(ten_year[0], 1, "left/oldest is PE half-year index 19");
    assert_eq!(
        ten_year[XGRP_HALF_YEAR_SAMPLES - 1],
        XGRP_HALF_YEAR_SAMPLES as u32,
        "right/newest is PE half-year index 0"
    );

    let hundred_year = xgrp_samples_oldest_first(&values, 0, GraphScale::Years100);
    assert_eq!(hundred_year.len(), XGRP_FIVE_YEAR_SAMPLES);
    assert_eq!(
        hundred_year[0],
        200 - (XGRP_FIVE_YEAR_SAMPLES as u32 - 1),
        "left/oldest is PE five-year index 19"
    );
    assert_eq!(
        hundred_year[XGRP_FIVE_YEAR_SAMPLES - 1],
        200,
        "right/newest is PE five-year index 0"
    );
}

#[test]
fn graph_panel_maps_legend_to_pe_xgrp_series() {
    assert_eq!(XGRP_GRAPH_LEGEND_SERIES, [0, 10, 11, 7, 4, 5]);
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut values = vec![0_u32; XGRP_VALUES];
    for (legend, series_index) in XGRP_GRAPH_LEGEND_SERIES.into_iter().enumerate() {
        let base = series_index * XGRP_SAMPLES_PER_SERIES;
        values[base] = 100 + legend as u32;
        values[base + GraphScale::Years10.xgrp_offset()] = 200 + legend as u32;
        values[base + GraphScale::Years100.xgrp_offset()] = 300 + legend as u32;
    }
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &values);

    let monthly = graph_panel_xgrp_series(&city, GraphScale::Year1);
    assert_eq!(monthly.len(), GRAPH_HISTORY_TOTAL_SERIES);
    for (legend, samples) in monthly.iter().enumerate() {
        assert_eq!(samples.len(), XGRP_MONTH_SAMPLES);
        assert_eq!(samples[XGRP_MONTH_SAMPLES - 1], 100 + legend as u32);
    }

    let ten_year = graph_panel_xgrp_series(&city, GraphScale::Years10);
    for (legend, samples) in ten_year.iter().enumerate() {
        assert_eq!(samples.len(), XGRP_HALF_YEAR_SAMPLES);
        assert_eq!(samples[XGRP_HALF_YEAR_SAMPLES - 1], 200 + legend as u32);
    }

    let hundred_year = graph_panel_xgrp_series(&city, GraphScale::Years100);
    for (legend, samples) in hundred_year.iter().enumerate() {
        assert_eq!(samples.len(), XGRP_FIVE_YEAR_SAMPLES);
        assert_eq!(samples[XGRP_FIVE_YEAR_SAMPLES - 1], 300 + legend as u32);
    }
}

#[test]
fn city_information_dialogs_check_their_palette_buttons() {
    let cases = [
        (CityInfoDialog::Map, UiAction::OpenCityMap),
        (CityInfoDialog::Population, UiAction::OpenCityPopulation),
        (CityInfoDialog::Neighbors, UiAction::OpenCityNeighbors),
        (CityInfoDialog::Graphs, UiAction::OpenCityGraphs),
        (CityInfoDialog::Industry, UiAction::OpenCityIndustry),
        (CityInfoDialog::Budget, UiAction::OpenBudget),
        (CityInfoDialog::Help, UiAction::OpenHelp),
    ];

    for (dialog, action) in cases {
        let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        let mut state = test_state(city);

        state.open_city_info_dialog(dialog);

        assert!(
            state.checked_palette_actions().contains(&action),
            "{dialog:?} did not check {action:?}"
        );
    }
}

#[test]
fn opening_city_information_dialog_clears_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_tool_menu = Some(ui::ToolMenu::Water);
    seed_all_modal_dialogs(&mut state);
    seed_all_drag_states(&mut state);

    state.open_city_info_dialog(CityInfoDialog::Help);

    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.power_plant_dialog, None);
    assert_eq!(state.arcology_dialog, None);
    assert_eq!(state.sign_dialog, None);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Help));
    assert!(CityInfoDialog::Help.holds_mac_simulation());
    assert_eq!(state.city.label_at(2), Some("Original Sign"));
    assert_drag_states_cleared(&state);
}

#[test]
fn mac_budget_help_returns_to_review_without_finishing_it() {
    let city =
        City::new_blank("/tmp/sc2000-mac-budget-help.sc2", "Budget help").expect("blank city");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_loop_scheduler.january_budget_review_pending = true;
    state.mac_loop_scheduler.january_budget_review_started_at = Some(1234);

    state.open_city_info_dialog(CityInfoDialog::Budget);
    state.open_city_info_dialog(CityInfoDialog::Help);

    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Help));
    assert!(state.mac_loop_scheduler.january_budget_review_pending);
    assert_eq!(
        state.mac_loop_scheduler.january_budget_review_started_at,
        Some(1234)
    );

    state.close_city_info_dialog();

    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    assert!(state.mac_loop_scheduler.january_budget_review_pending);
    assert_eq!(
        state.mac_loop_scheduler.january_budget_review_started_at,
        Some(1234)
    );

    state.open_city_info_dialog(CityInfoDialog::Help);
    state.open_city_info_dialog(CityInfoDialog::Help);
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    assert!(state.mac_loop_scheduler.january_budget_review_pending);
    assert_eq!(
        state.mac_loop_scheduler.january_budget_review_started_at,
        Some(1234)
    );
}

#[test]
fn city_map_dialog_hit_tests_overview_cells_as_city_tiles() {
    let window_size = (1024, 768);
    let dialog = city_info_dialog_rect(window_size);
    let (map, cell) = city_map_panel_rect_and_cell(dialog);
    let point = Vec2::new(map.x + cell * 7.5, map.y + cell * 10.5);

    assert_eq!(city_map_tile_for_point(point, dialog), Some((30, 42)));
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Map),
        Some(CityInfoDialogAction::CenterMapOn { x: 30, y: 42 })
    );
    assert_eq!(
        city_info_dialog_hit_test(point, window_size, CityInfoDialog::Graphs),
        None
    );

    let bottom_right = Vec2::new(map.x + map.w - 0.5, map.y + map.h - 0.5);
    assert_eq!(
        city_map_tile_for_point(bottom_right, dialog),
        Some((CITY_WIDTH - 2, CITY_HEIGHT - 2))
    );

    let marker = city_map_cell_rect_for_tile(30, 42, map, cell);
    assert_eq!(marker.x, map.x + cell * 7.0);
    assert_eq!(marker.y, map.y + cell * 10.0);
    assert_eq!(marker.w, cell);
    assert_eq!(marker.h, cell);
}

#[test]
fn population_panel_demographics_include_city_fame() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_CITY_FAME_OFFSET, 73);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 123_000);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 456_000);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 7);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 12);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 72);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 15);
    city.set_misc_u32_at(MISC_HUMID_OFFSET, 42);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 3);
    let population = PopulationStats {
        work_force: 456,
        residential_population: 1_000,
        total_population: 1_000,
        ..Default::default()
    };
    let outcomes = CityServiceOutcomes {
        life_expectancy: 78,
        education_quotient: 91,
        service_score: 64,
        city_value: 5_000,
        land_value: 82,
        ..Default::default()
    };

    assert_eq!(
        population_panel_demographic_rows(&city, population, outcomes),
        vec![
            ("Work Force %", 45),
            ("Unemployment %", 0),
            ("Life Expectancy", 78),
            ("Education Quotient", 91),
            ("Service Score", 64),
            ("City Fame", 73),
            ("National Population", 123_000),
            ("National Value", 456_000),
            ("National Tax", 7),
            ("National Trend", 12),
            ("Heat", 72),
            ("Wind", 15),
            ("Humid", 42),
            ("Weather Trend", 3),
            ("City Value", 5_000),
            ("Land Value", 82),
        ]
    );
    assert_eq!(
        population_panel_transit_rows(TransitPassengerStats {
            bus: 12,
            rail: 4,
            subway: 7,
        }),
        [
            ("Bus Passengers", 12),
            ("Rail Passengers", 4),
            ("Subway Passengers", 7),
        ]
    );
}

#[test]
fn zoom_palette_buttons_disable_at_zoom_bounds() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.zoom = MIN_ZOOM;
    assert!(!state.ui_action_enabled(UiAction::ZoomOut));
    assert!(state.ui_action_enabled(UiAction::ZoomIn));
    assert!(
        state
            .disabled_palette_actions()
            .contains(&UiAction::ZoomOut)
    );
    assert!(!state.disabled_palette_actions().contains(&UiAction::ZoomIn));

    state.zoom = MAX_ZOOM;
    assert!(state.ui_action_enabled(UiAction::ZoomOut));
    assert!(!state.ui_action_enabled(UiAction::ZoomIn));
    assert!(state.disabled_palette_actions().contains(&UiAction::ZoomIn));
    assert!(
        !state
            .disabled_palette_actions()
            .contains(&UiAction::ZoomOut)
    );

    state.zoom = (MIN_ZOOM + MAX_ZOOM) * 0.5;
    assert!(state.ui_action_enabled(UiAction::ZoomOut));
    assert!(state.ui_action_enabled(UiAction::ZoomIn));
    assert!(
        !state
            .disabled_palette_actions()
            .contains(&UiAction::ZoomOut)
    );
    assert!(!state.disabled_palette_actions().contains(&UiAction::ZoomIn));
}

#[test]
fn selecting_city_tool_refreshes_status_line_and_clears_stale_query() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.query_line = "Tile 10,10".to_owned();
    state.query_is_inspect = true;

    assert!(state.handle_ui_action_state(UiAction::SelectTool(EditTool::ZoneCommercialDense,)));

    assert_eq!(state.tool, EditTool::ZoneCommercialDense);
    assert!(!state.query_is_inspect);
    assert!(
        state.query_line.starts_with("Dense Commercial $10  "),
        "status line was {}",
        state.query_line
    );
}

#[test]
fn query_tile_status_line_names_palette_buildings_and_zones() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    for (x, y, building, expected) in [
        (10, 10, 207, "Coal Power"),
        (11, 10, 210, "Police"),
        (12, 10, 214, "School"),
        (13, 10, 218, "Zoo"),
        (14, 10, 220, "Water Pump"),
        (15, 10, 236, "Bus Depot"),
        (16, 10, 221, "Runway"),
        (17, 10, 224, "Crane"),
        (18, 10, 251, "Plymouth Arco"),
        (19, 10, 0x70, "Residential Building"),
        (20, 10, 0x7c, "Commercial Building"),
        (21, 10, 0x84, "Industrial Building"),
        (22, 10, 230, "Tarmac"),
    ] {
        city.set_building(x, y, building);
        assert!(
            query_tile_status_line(&city, x, y, false).starts_with(expected),
            "building {building} query was {}",
            query_tile_status_line(&city, x, y, false)
        );
    }

    city.set_zone(30, 30, ZONE_INDUSTRIAL_DENSE);
    assert!(
        query_tile_status_line(&city, 30, 30, false).starts_with("Dense Industrial"),
        "{}",
        query_tile_status_line(&city, 30, 30, false)
    );
    city.set_zone(31, 30, ZONE_AIRPORT);
    assert!(
        query_tile_status_line(&city, 31, 30, false).starts_with("Airport Zone"),
        "{}",
        query_tile_status_line(&city, 31, 30, false)
    );
}

#[test]
fn query_tile_status_line_prefers_underground_when_requested() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 210);

    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Police"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );
    assert!(
        query_tile_status_line(&city, 10, 10, true).starts_with("Building Pipes"),
        "{}",
        query_tile_status_line(&city, 10, 10, true)
    );

    city.set_underground(10, 10, 16);
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Police"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );
    assert!(
        query_tile_status_line(&city, 10, 10, true).starts_with("Pipes + Building Pipes"),
        "{}",
        query_tile_status_line(&city, 10, 10, true)
    );

    city.set_underground(40, 40, UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR);
    assert!(
        query_tile_status_line(&city, 40, 40, true).starts_with("Pipes/Subway Crossing"),
        "{}",
        query_tile_status_line(&city, 40, 40, true)
    );

    city.set_text_overlay(40, 40, 0xfc);
    assert!(
        query_tile_status_line(&city, 40, 40, true).starts_with("Pipes/Subway Crossing"),
        "{}",
        query_tile_status_line(&city, 40, 40, true)
    );
    assert!(
        query_tile_status_line(&city, 40, 40, false).starts_with("Flood"),
        "{}",
        query_tile_status_line(&city, 40, 40, false)
    );
}

#[test]
fn query_tile_status_line_falls_back_for_unknown_buildings() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 199);

    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Building 199"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );
}

#[test]
fn query_uses_xlabel_title_when_microsim_is_live() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 210);
    let police = city
        .provision_microsim_for_building(210)
        .expect("police xlab");
    city.set_text_overlay(10, 10, police);
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Police Station"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_label(police, "");
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Police"),
        "empty XLAB falls through to the palette name: {}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_building(12, 12, 199);
    let hydro = city
        .provision_microsim_for_building(199)
        .expect("hydro 199");
    city.set_text_overlay(12, 12, hydro);
    assert!(
        query_tile_status_line(&city, 12, 12, false).starts_with("Hydro Power"),
        "{}",
        query_tile_status_line(&city, 12, 12, false)
    );

    let bus = city.provision_microsim_for_building(236).expect("bus");
    city.set_building(14, 14, 236);
    city.set_text_overlay(14, 14, bus);
    city.set_label(bus, "Downtown Transit");
    assert!(
        query_tile_status_line(&city, 14, 14, false).starts_with("Downtown Transit"),
        "{}",
        query_tile_status_line(&city, 14, 14, false)
    );
}

#[test]
fn query_uses_xlabel_title_for_user_signs() {
    let mut city = City::new_blank("/tmp/sc2000-query-sign.sc2", "test").expect("blank city");
    city.set_text_overlay(10, 10, MIN_USER_TEXT_ENTRIES);
    city.set_label(MIN_USER_TEXT_ENTRIES, "Main Street");
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Main Street"),
        "{}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_building(10, 10, 29);
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Main Street"),
        "user-sign XLAB wins over the road palette name: {}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_label(MIN_USER_TEXT_ENTRIES, "");
    assert!(
        query_tile_status_line(&city, 10, 10, false).starts_with("Road"),
        "empty sign XLAB falls through to the palette: {}",
        query_tile_status_line(&city, 10, 10, false)
    );

    city.set_label(0, "Marvin Maxis");
    city.set_text_overlay(11, 11, 0);
    assert!(
        query_tile_status_line(&city, 11, 11, false).starts_with("Terrain"),
        "XLAB[0] is the mayor name, not a map sign: {}",
        query_tile_status_line(&city, 11, 11, false)
    );

    city.set_text_overlay(12, 12, MAX_USER_TEXT_ENTRIES);
    city.set_label(MAX_USER_TEXT_ENTRIES, "North Gate");
    assert!(
        query_tile_status_line(&city, 12, 12, false).starts_with("North Gate"),
        "{}",
        query_tile_status_line(&city, 12, 12, false)
    );
}

#[test]
fn query_tile_status_line_shows_microsim_stats() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);

    assert!(state.place_building_at(20, 20, 207) > 0); // coal
    assert!(state.place_building_at(30, 30, 210) > 0); // police
    assert!(state.place_building_at(40, 40, 251) > 0); // plymouth arco
    let arco_text = state.city.text_overlay_at(40, 40).expect("arco xtxt");
    let arco_index = usize::from(arco_text - MIN_SIM_TEXT_ENTRIES);
    assert!(state.city.set_microsim_stat0(arco_index, 12));
    state.city.set_misc_u32_at(MISC_CITY_CRIME_OFFSET, 9_496);
    state
        .city
        .refresh_structure_microsim_yearly(55_860, 100, 100, 100, 100, 100);

    let coal = query_tile_status_line(&state.city, 20, 20, false);
    assert!(coal.starts_with("Coal Power"), "{coal}");
    assert!(coal.contains("Max 200 MW"), "{coal}");
    assert!(coal.contains("Age 1"), "{coal}");

    let police = query_tile_status_line(&state.city, 30, 30, false);
    assert!(police.starts_with("Police Station"), "{police}");
    assert!(police.contains("Officers 200"), "{police}");
    assert!(police.contains("Crimes 1,055"), "{police}");
    assert!(police.contains("Arrests"), "{police}");

    let arco = query_tile_status_line(&state.city, 40, 40, false);
    assert!(arco.contains("Design Capacity 55,000"), "{arco}");
    let arco_record = state.city.microsim_at(arco_index).expect("arcology XMIC");
    assert!(
        arco.contains(&format!(
            "Residents {}",
            format_number(u32::from(arco_record.stat2))
        )),
        "query should show the resident word stored in XMIC: {arco}"
    );
    assert!(arco.contains("Built 1,900"), "{arco}");
    assert!(arco.contains("Conditions 12"), "{arco}");
}

#[test]
fn query_dialog_shows_general_and_microsim_fields() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 207) > 0);
    state
        .city
        .sync_crime_density_from_slice(&vec![90u8; CITY_TILES]);
    state
        .city
        .sync_pollution_density_from_slice(&vec![30u8; CITY_TILES]);
    state.city.set_traffic_density(20, 20, 150);
    state
        .city
        .refresh_structure_microsim_yearly(10_000, 100, 100, 100, 100, 100);

    let (general, specific) = query_dialog_row_groups(
        &state.city,
        20,
        20,
        false,
        TransitPassengerStats::default(),
        pe_power_recalc_seed(&state.city),
    );
    let general_map: std::collections::HashMap<_, _> = general.into_iter().collect();
    assert_eq!(
        general_map.get("Location").map(String::as_str),
        Some("(20,20)")
    );
    assert!(
        general_map
            .get("Altitude")
            .is_some_and(|v| v.contains("feet")),
        "{general_map:?}"
    );
    assert!(
        general_map
            .get("Land Value")
            .is_some_and(|v| v.contains(",000/acre")),
        "{general_map:?}"
    );
    assert_eq!(general_map.get("Crime").map(String::as_str), Some("Medium"));
    assert_eq!(
        general_map.get("Pollution").map(String::as_str),
        Some("Low")
    );
    assert_eq!(general_map.get("Traffic").map(String::as_str), Some("High"));

    let specific_map: std::collections::HashMap<_, _> = specific.into_iter().collect();
    assert!(
        specific_map.contains_key("Max") && specific_map.contains_key("Age"),
        "{specific_map:?}"
    );

    state.begin_tile_inspect(20, 20);
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Query { x: 20, y: 20 })
    );
    assert!(state.query_is_inspect);
    assert!(
        state.query_line.starts_with("Coal Power"),
        "{}",
        state.query_line
    );

    let mut solids = Vec::new();
    state.push_city_info_dialog(None, (1024, 768), 1.0, &mut solids, &mut Vec::new());
    assert!(solids.len() > 10);

    state.close_city_info_dialog();
    assert_eq!(state.city_info_dialog, None);
}

#[test]
fn query_shows_letter_grades_for_school_hospital_college() {
    assert_eq!(microsim_query_letter_grade(214, 12), 'A');
    assert_eq!(microsim_query_letter_grade(214, 10), 'B');
    assert_eq!(microsim_query_letter_grade(214, 6), 'D');
    assert_eq!(microsim_query_letter_grade(214, 0), 'F');
    assert_eq!(microsim_query_letter_grade(217, 8), 'C');
    assert_eq!(microsim_query_letter_grade(209, 12), 'A');
    assert_eq!(microsim_query_letter_grade(209, 6), 'C');
    assert_eq!(microsim_query_letter_grade(209, 0), 'F');

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    city.set_misc_u32_at(MISC_NORMAL_POPULATION_OFFSET, 7_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 214) > 0); // school
    assert!(state.place_building_at(30, 30, 209) > 0); // hospital
    assert!(state.place_building_at(40, 40, 217) > 0); // college

    let seed_school = query_tile_status_line(&state.city, 20, 20, false);
    assert!(
        seed_school.contains("Grade D"),
        "fresh school seed stat0=6 → D, got {seed_school}"
    );

    state.city.persist_pe_day22_population_leftover(7_000);
    state.city.advance_pe_eq_le_month(
        7_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    state
        .city
        .refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let school = query_tile_status_line(&state.city, 20, 20, false);
    let hospital = query_tile_status_line(&state.city, 30, 30, false);
    let college = query_tile_status_line(&state.city, 40, 40, false);
    assert!(
        school.contains("Grade A") && school.contains("Students 350"),
        "leftover-filled EQ_LE[1]+[2] occupancy, not a 12% pyramid: {school}"
    );
    assert!(hospital.contains("Grade A"), "{hospital}");
    assert!(college.contains("Grade A"), "{college}");

    let (_, specific) = query_dialog_row_groups(
        &state.city,
        20,
        20,
        false,
        TransitPassengerStats::default(),
        pe_power_recalc_seed(&state.city),
    );
    assert!(
        specific
            .iter()
            .any(|(label, value)| label == "Grade" && value == "A"),
        "{specific:?}"
    );
}

#[test]
fn query_shows_stadium_team_name_only_after_team_selection() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 215) > 0);

    let seeded = query_tile_status_line(&state.city, 20, 20, false);
    assert!(
        seeded.contains("Team Football"),
        "an unselected team uses the generic football label: {seeded}"
    );

    state.city.set_label(0xFB, "Nexatrode Knights");
    state
        .city
        .refresh_structure_microsim_yearly(55_860, 100, 100, 100, 100, 100);
    let january = query_tile_status_line(&state.city, 20, 20, false);
    assert!(january.contains("Attendance 3,532"), "{january}");
    assert!(
        january.contains("Team Football"),
        "yearly attendance must not select a team: {january}"
    );
    let xtxt = state
        .city
        .text_overlay_at(20, 20)
        .expect("placed stadium XTXT");
    let index = usize::from(xtxt - MIN_SIM_TEXT_ENTRIES);
    assert!(state.city.set_microsim_stat3(index, 0xfb));
    let line = query_tile_status_line(&state.city, 20, 20, false);
    assert!(line.contains("Team Nexatrode Knights"), "{line}");
    assert_eq!(state.city.label_at(0xFB), Some("Nexatrode Knights"));

    let (_, specific) = query_dialog_row_groups(
        &state.city,
        20,
        20,
        false,
        TransitPassengerStats::default(),
        pe_power_recalc_seed(&state.city),
    );
    assert!(
        specific
            .iter()
            .any(|(label, value)| label == "Team" && value == "Nexatrode Knights"),
        "{specific:?}"
    );
}

#[test]
fn every_city_palette_tool_has_a_handled_preview_or_dialog_route() {
    let tools = visible_city_menu_tools();

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    enable_all_conditional_palette_tools(&mut city);
    let mut state = test_state(city);

    for tool in tools {
        assert_ne!(
            GameCursor::for_tool(tool),
            GameCursor::Arrow,
            "{tool:?} fell back to the arrow cursor"
        );

        state.tool = tool;
        if tool == EditTool::Query {
            assert_eq!(state.tool_placement_at(20, 20), None);
            continue;
        }
        if matches!(tool, EditTool::PowerPlant | EditTool::Arcologies) {
            assert!(
                state.selected_tool_status_line().is_none(),
                "{tool:?} chooser button should open a dialog instead of showing a build price"
            );
            continue;
        }

        assert!(
            state.tool_placement_at(20, 20).is_some(),
            "{tool:?} has no city-view preview route"
        );
    }
}

#[test]
fn city_palette_menus_do_not_expose_edit_map_only_tools() {
    for menu in ALL_CITY_TOOL_MENUS {
        for item in ui::tool_menu_items(*menu) {
            assert_ne!(
                expected_city_tool_behavior(item.tool),
                CityToolBehavior::EditMapOnly,
                "{menu:?} exposes {:?}, but that tool only has edit-map behavior",
                item.tool
            );
        }
    }
}

#[test]
fn visible_city_palette_buttons_apply_expected_runtime_actions() {
    let actions = ui::city_palette_actions_for_test().collect::<Vec<_>>();
    assert_eq!(actions.len(), 34);

    for action in actions {
        drain_command_queue_for_test();
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(500_000);
        enable_all_conditional_palette_tools(&mut city);
        let mut state = test_state(city);

        match action {
            UiAction::RotateViewCcw => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.city.rotation(), 3);
                assert_eq!(state.camera_rotation, 3);
                assert_eq!(pop_command(), None);
            }
            UiAction::RotateViewCw => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.city.rotation(), 1);
                assert_eq!(state.camera_rotation, 1);
                assert_eq!(pop_command(), None);
            }
            UiAction::ZoomOut => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(pop_command(), Some(AppCommand::ZoomOut));
                assert_eq!(pop_command(), None);
            }
            UiAction::ZoomIn => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(pop_command(), Some(AppCommand::ZoomIn));
                assert_eq!(pop_command(), None);
            }
            UiAction::CenterMap => {
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(pop_command(), Some(AppCommand::CenterMap));
                assert_eq!(pop_command(), None);
            }
            UiAction::OpenCityMap
            | UiAction::OpenCityPopulation
            | UiAction::OpenCityNeighbors
            | UiAction::OpenCityGraphs
            | UiAction::OpenCityIndustry
            | UiAction::OpenBudget
            | UiAction::OpenHelp => {
                let expected = CityInfoDialog::for_action(action).expect("city dialog action");
                assert!(state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.city_info_dialog, Some(expected), "{action:?}");
                assert_eq!(pop_command(), None);
            }
            UiAction::OpenMenu(menu) => {
                let expected_tool = ui::tool_menu_items(menu)
                    .iter()
                    .find(|item| {
                        !is_chooser_menu_tool(item.tool)
                            && state.ui_action_enabled(UiAction::SelectTool(item.tool))
                    })
                    .map(|item| item.tool);
                assert!(state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.open_tool_menu, Some(menu), "{action:?}");
                if let Some(expected_tool) = expected_tool {
                    assert_eq!(state.tool, expected_tool, "{action:?}");
                }
                assert_eq!(pop_command(), None);
            }
            UiAction::SelectTool(tool) => {
                assert!(state.handle_ui_action_state(action), "{action:?}");
                assert_eq!(state.tool, tool, "{action:?}");
                assert_eq!(state.open_tool_menu, None, "{action:?}");
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleBuildings => {
                assert!(state.show_buildings);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(!state.show_buildings);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleSigns => {
                assert!(state.show_signs);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(!state.show_signs);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleInfrastructure => {
                assert!(state.show_infrastructure);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(!state.show_infrastructure);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleZones => {
                assert!(state.show_zones);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(!state.show_zones);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleUnderground => {
                assert!(!state.show_underground);
                assert!(state.show_traffic);
                assert!(!state.handle_ui_action_state(action), "{action:?}");
                assert!(state.show_underground);
                assert!(!state.show_traffic);
                assert_eq!(pop_command(), None);
            }
            UiAction::ToggleEditCoast
            | UiAction::ToggleEditRiver
            | UiAction::GenerateEditMap
            | UiAction::RaiseEditSeaLevel
            | UiAction::LowerEditSeaLevel
            | UiAction::DoneEditMap => {
                panic!("{action:?} should not be on the city palette")
            }
        }
    }

    drain_command_queue_for_test();
}

#[test]
fn visible_city_palette_tools_execute_expected_first_action() {
    for tool in visible_city_menu_tools() {
        let behavior = expected_city_tool_behavior(tool);
        let (mut state, tile) = city_tool_execution_fixture(tool);

        match behavior {
            CityToolBehavior::Query => {
                assert_eq!(state.tool_placement_at(tile.0, tile.1), None, "{tool:?}");
                assert!(
                    !state.apply_tool_at_tile(tile.0, tile.1),
                    "{tool:?} should inspect, not build"
                );
            }
            CityToolBehavior::ChooserDialog => {
                assert!(
                    !state.apply_tool_at_tile(tile.0, tile.1),
                    "{tool:?} should not place without a chooser selection"
                );
                match tool {
                    EditTool::PowerPlant => {
                        state.open_power_plant_dialog();
                        assert!(state.power_plant_dialog.is_some(), "{tool:?}");
                    }
                    EditTool::Arcologies => {
                        state.open_arcology_dialog();
                        assert!(state.arcology_dialog.is_some(), "{tool:?}");
                    }
                    _ => unreachable!("{tool:?} is not a chooser tool"),
                }
            }
            CityToolBehavior::EditMapOnly => {
                panic!("{tool:?} should not be visible in the city palette");
            }
            CityToolBehavior::Network(kind) => {
                state.begin_network_drag(tile.0, tile.1, kind);
                let (_, placed, _, _) = state.finish_network_drag().expect("network drag");
                assert!(placed > 0, "{tool:?}");
                assert!(
                    kind.contains(state.city.building_at(tile.0, tile.1)),
                    "{tool:?}"
                );
            }
            CityToolBehavior::Underground(kind) => {
                state.begin_underground_drag(tile.0, tile.1, kind);
                let (_, placed) = state.finish_underground_drag().expect("underground drag");
                assert!(placed > 0, "{tool:?}");
                assert_ne!(state.city.underground_at(tile.0, tile.1), 0, "{tool:?}");
            }
            CityToolBehavior::Zone(zone_id) => {
                state.begin_zone_drag(tile.0, tile.1, zone_id);
                let placed = state.finish_zone_drag();
                if zone_id == 0 {
                    assert_eq!(placed, 1, "{tool:?}");
                    assert_eq!(state.city.building_at(tile.0, tile.1), 0, "{tool:?}");
                    // Native dezone clears small rubble but skips the XZON
                    // nibble write for type zero.
                    assert_eq!(
                        state.city.zone_at(tile.0, tile.1).unwrap().zone_id(),
                        ZONE_RESIDENTIAL_LIGHT,
                        "{tool:?}"
                    );
                } else {
                    assert!(placed > 0, "{tool:?}");
                    assert_eq!(
                        state.city.zone_at(tile.0, tile.1).unwrap().zone_id(),
                        zone_id,
                        "{tool:?}"
                    );
                }
            }
            CityToolBehavior::TerrainEdit => {
                state.begin_terrain_drag(tile.0, tile.1);
                assert!(state.finish_terrain_drag() > 0, "{tool:?}");
            }
            CityToolBehavior::SurfaceWater => {
                state.begin_nature_drag(tile.0, tile.1, tool);
                assert_eq!(state.finish_nature_drag(), Some((tool, 1)), "{tool:?}");
                assert!(is_water_terrain(state.city.terrain_at(tile.0, tile.1)));
            }
            CityToolBehavior::Trees => {
                state.begin_nature_drag(tile.0, tile.1, tool);
                assert_eq!(state.finish_nature_drag(), Some((tool, 1)), "{tool:?}");
                assert!(
                    matches!(state.city.building_at(tile.0, tile.1), 6 | 7),
                    "{tool:?}"
                );
            }
            CityToolBehavior::Forest => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                let mut trees = 0;
                for y in tile.1.saturating_sub(7)..=(tile.1 + 7).min(CITY_HEIGHT - 1) {
                    for x in tile.0.saturating_sub(7)..=(tile.0 + 7).min(CITY_WIDTH - 1) {
                        if is_tree_building(state.city.building_at(x, y)) {
                            trees += 1;
                        }
                    }
                }
                assert!(trees > 0, "{tool:?} PE forest scatter stamps trees");
            }
            CityToolBehavior::Highway => {
                state.begin_highway_drag(tile.0, tile.1);
                assert!(state.finish_highway_drag().0 > 0, "{tool:?}");
            }
            CityToolBehavior::Bulldoze => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert!(
                    matches!(state.city.building_at(tile.0, tile.1), 1..=4),
                    "{tool:?} leaves PE DestroyBuilding rubble"
                );
            }
            CityToolBehavior::Sign => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert!(state.sign_dialog.is_some(), "{tool:?}");
            }
            CityToolBehavior::Tunnel => {
                let plan = state.tunnel_plan_at(tile.0, tile.1).expect("tunnel plan");
                let tiles = plan.tiles.clone();
                let exit = *tiles.last().expect("tunnel exit");
                let entrance_building = plan.entrance_building;
                let exit_building = plan.exit_building;

                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert_eq!(
                    state.city.building_at(tile.0, tile.1),
                    entrance_building,
                    "{tool:?}"
                );
                assert_eq!(
                    state.city.building_at(exit.0, exit.1),
                    exit_building,
                    "{tool:?}"
                );
                assert!(
                    tiles
                        .iter()
                        .all(|&(x, y)| state.city.tunnel_levels_at(x, y) > 0),
                    "{tool:?} did not stamp tunnel depth"
                );
                assert_tool_debited_units(&state, tool, tiles.len() as u32);
            }
            CityToolBehavior::Onramp => {
                let plan = state.onramp_plan_at(tile.0, tile.1).expect("onramp plan");
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert_eq!(
                    state.city.building_at(tile.0, tile.1),
                    plan.building,
                    "{tool:?}"
                );
                assert_eq!(
                    state.city.tile_bits_at(tile.0, tile.1).unwrap().rotate(),
                    plan.rotate,
                    "{tool:?}"
                );
                assert_ne!(
                    state.network_neighbor_mask(
                        plan.road_tile.0,
                        plan.road_tile.1,
                        NetworkKind::Road,
                    ) & opposite_network_side(NETWORK_RIGHT),
                    0,
                    "{tool:?} did not connect the adjacent road"
                );
                assert_tool_debited_units(&state, tool, 1);
            }
            CityToolBehavior::SubToRail => {
                let plan = state
                    .sub_to_rail_plan_at(tile.0, tile.1)
                    .expect("sub-to-rail plan");
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert_eq!(
                    state.city.building_at(tile.0, tile.1),
                    plan.building,
                    "{tool:?}"
                );
                assert_eq!(
                    state.city.underground_at(tile.0, tile.1),
                    UNDER_TILE_GREY_BOX_2,
                    "{tool:?}"
                );
                assert_tool_debited_units(&state, tool, 1);
            }
            CityToolBehavior::Dispatch(kind) => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                let record = state
                    .city
                    .traffic_overlays()
                    .expect("dispatch overlay table")
                    .iter()
                    .skip(crate::win95_overlay::XTHG_FIRST_USABLE_SLOT)
                    .find(|record| record.kind == kind.overlay_kind())
                    .expect("dispatch overlay record");
                assert_eq!((record.tile_x as usize, record.tile_y as usize), tile);
                // PE FUN_00450690: dispatch kinds use fixed XTHG spawn height 0, not ALTM.
                assert_eq!(
                    record.height,
                    crate::win95_overlay::xthg_spawn_height(kind.overlay_kind()),
                    "{tool:?}"
                );
                assert_eq!(record.saved_a, 0);
            }
            CityToolBehavior::DirectBuilding(building) => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                assert_tool_stamped_building(&state, tool, tile, building);
            }
            CityToolBehavior::HydroPower => {
                assert!(state.apply_tool_at_tile(tile.0, tile.1), "{tool:?}");
                // PE @ `0x441610`: rot0 flat → `0xC7` (199).
                let stamped = state.city.building_at(tile.0, tile.1);
                assert!(
                    stamped == 198 || stamped == 199,
                    "{tool:?} hydro XBLD {stamped}"
                );
            }
        }
    }
}

#[test]
fn every_city_palette_tool_has_a_mouse_input_route() {
    let tools = visible_city_menu_tools();

    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    enable_all_conditional_palette_tools(&mut city);
    let mut state = test_state(city);

    for tool in tools {
        let route = test_mouse_input_route_for_tool(tool)
            .unwrap_or_else(|| panic!("{tool:?} has no known city-view mouse route"));
        state.tool = tool;

        match route {
            TestMouseInputRoute::Query => {
                assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
            }
            TestMouseInputRoute::ChooserDialog => {
                assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
            }
            TestMouseInputRoute::Drag | TestMouseInputRoute::DirectClick => {
                assert!(
                    state.tool_placement_at(20, 20).is_some(),
                    "{tool:?} has a mouse route but no preview route"
                );
            }
        }
    }
}

#[test]
fn visible_city_tool_matrix_covers_cursor_route_preview_cost_and_sound() {
    let tools = visible_city_menu_tools();
    let mut seen_tools = Vec::new();

    for tool in tools {
        assert!(
            !seen_tools.contains(&tool),
            "{tool:?} appears more than once in the visible city palette"
        );
        seen_tools.push(tool);

        let behavior = expected_city_tool_behavior(tool);
        let (state, tile) = city_tool_execution_fixture(tool);
        let input_route = test_mouse_input_route_for_tool(tool)
            .unwrap_or_else(|| panic!("{tool:?} has no city palette input route"));

        assert_ne!(
            GameCursor::for_tool(tool),
            GameCursor::Arrow,
            "{tool:?} should have a recovered SC2K cursor"
        );
        assert_ne!(
            behavior,
            CityToolBehavior::EditMapOnly,
            "{tool:?} should not be exposed in the city palette"
        );
        assert_eq!(
            state.selected_tool_status_line(),
            expected_tool_status_line(tool, behavior),
            "{tool:?} status-line cost does not match its placement class"
        );
        if input_route == TestMouseInputRoute::DirectClick {
            assert_eq!(
                one_click_placement_sound(tool),
                expected_success_placement_sound(tool, behavior),
                "{tool:?} one-click sound route drifted"
            );
        }
        match behavior {
            CityToolBehavior::Network(kind) => assert_eq!(
                expected_success_placement_sound(tool, behavior),
                Some(network_placement_sound(kind)),
                "{tool:?} drag sound route drifted"
            ),
            CityToolBehavior::Underground(kind) => assert_eq!(
                expected_success_placement_sound(tool, behavior),
                Some(underground_placement_sound(kind)),
                "{tool:?} underground sound route drifted"
            ),
            CityToolBehavior::Trees | CityToolBehavior::Forest => assert_eq!(
                expected_success_placement_sound(tool, behavior),
                Some(PlacementSound::Plop),
                "{tool:?} nature sound route drifted"
            ),
            CityToolBehavior::SurfaceWater => assert_eq!(
                expected_success_placement_sound(tool, behavior),
                Some(PlacementSound::Flood),
                "{tool:?} surface-water sound route drifted"
            ),
            _ => {}
        }

        let expected_preview = expected_tool_preview_for_fixture(behavior, &state, tile);
        let actual_placement = state.tool_placement_at(tile.0, tile.1);
        assert_eq!(
            actual_placement.as_ref().map(|placement| placement.preview),
            expected_preview,
            "{tool:?} preview class does not match its placement class"
        );
        if let Some(placement) = actual_placement {
            assert!(
                !placement.is_empty(),
                "{tool:?} produced an empty placement at its fixture tile"
            );
            if !matches!(
                behavior,
                CityToolBehavior::Query
                    | CityToolBehavior::ChooserDialog
                    | CityToolBehavior::EditMapOnly
            ) {
                assert!(
                    !placement.valid_tiles.is_empty(),
                    "{tool:?} should have valid tiles in its placement fixture"
                );
            }
        }

        match input_route {
            TestMouseInputRoute::Query => {
                assert_eq!(behavior, CityToolBehavior::Query, "{tool:?}");
            }
            TestMouseInputRoute::ChooserDialog => {
                assert_eq!(behavior, CityToolBehavior::ChooserDialog, "{tool:?}");
            }
            TestMouseInputRoute::Drag => {
                assert!(
                    matches!(
                        behavior,
                        CityToolBehavior::TerrainEdit
                            | CityToolBehavior::SurfaceWater
                            | CityToolBehavior::Trees
                            | CityToolBehavior::Network(_)
                            | CityToolBehavior::Highway
                            | CityToolBehavior::Underground(_)
                            | CityToolBehavior::Zone(_)
                    ),
                    "{tool:?} has a drag route but behavior {behavior:?}"
                );
            }
            TestMouseInputRoute::DirectClick => {
                assert!(
                    matches!(
                        behavior,
                        CityToolBehavior::Bulldoze
                            | CityToolBehavior::Sign
                            | CityToolBehavior::Tunnel
                            | CityToolBehavior::Onramp
                            | CityToolBehavior::SubToRail
                            | CityToolBehavior::DirectBuilding(_)
                            | CityToolBehavior::HydroPower
                            | CityToolBehavior::Dispatch(_)
                            | CityToolBehavior::Forest
                    ),
                    "{tool:?} has a direct-click route but behavior {behavior:?}"
                );
            }
        }
    }
}

#[test]
fn opening_city_tool_menus_selects_first_buildable_choice() {
    let cases = [
        (ui::ToolMenu::Residential, EditTool::ZoneResidential),
        (ui::ToolMenu::Commercial, EditTool::ZoneCommercial),
        (ui::ToolMenu::Industrial, EditTool::ZoneIndustrial),
        (ui::ToolMenu::Parks, EditTool::SmallPark),
        (ui::ToolMenu::Power, EditTool::Power),
        (ui::ToolMenu::Civic, EditTool::Police),
        (ui::ToolMenu::Education, EditTool::School),
        (ui::ToolMenu::Water, EditTool::Pipes),
        (ui::ToolMenu::Terrain, EditTool::Trees),
        (ui::ToolMenu::TerrainEdit, EditTool::Bulldoze),
        (ui::ToolMenu::Road, EditTool::Road),
        (ui::ToolMenu::Rail, EditTool::Rail),
        (ui::ToolMenu::Transport, EditTool::Seaport),
    ];

    for (menu, expected_tool) in cases {
        let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        let mut state = test_state(city);
        state.tool = EditTool::Query;
        state.query_is_inspect = true;

        state.open_tool_menu(menu);

        assert_eq!(state.open_tool_menu, Some(menu), "{menu:?}");
        assert_eq!(state.tool, expected_tool, "{menu:?}");
        assert!(
            !state.query_is_inspect,
            "{menu:?} should leave inspect mode after selecting a build tool"
        );
    }
}

#[test]
fn opening_tool_menu_clears_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    seed_all_modal_dialogs(&mut state);
    seed_all_drag_states(&mut state);

    state.open_tool_menu(ui::ToolMenu::Water);

    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Water));
    assert_eq!(state.tool, EditTool::Pipes);
    assert_modal_dialogs_cleared(&state);
    assert_drag_states_cleared(&state);
}

#[test]
fn selecting_direct_tool_clears_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_tool_menu = Some(ui::ToolMenu::Road);
    seed_all_modal_dialogs(&mut state);
    seed_all_drag_states(&mut state);

    state.select_tool_state(EditTool::Road);

    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.tool, EditTool::Road);
    assert_modal_dialogs_cleared(&state);
    assert_drag_states_cleared(&state);
}

#[test]
fn opening_zone_menu_selects_its_light_tool_default() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.tool = EditTool::ZoneResidential;

    state.open_tool_menu(ui::ToolMenu::Industrial);

    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Industrial));
    assert_eq!(state.tool, EditTool::ZoneIndustrial);
    assert_eq!(zone_id_for_tool(state.tool), Some(5));
}

#[test]
fn opening_power_menu_selects_power_lines_not_power_plant_chooser() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.tool = EditTool::Query;

    state.open_tool_menu(ui::ToolMenu::Power);

    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Power));
    assert_eq!(state.tool, EditTool::Power);
    assert_eq!(state.power_plant_dialog, None);
}

#[test]
fn opening_chooser_dialogs_clear_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_tool_menu = Some(ui::ToolMenu::Power);
    seed_all_modal_dialogs(&mut state);
    state.query_is_inspect = true;
    seed_all_drag_states(&mut state);

    state.open_power_plant_dialog();

    assert_eq!(
        state.power_plant_dialog,
        Some(PowerPlantDialogState {
            info_tool: None,
            pressed_tool: None
        })
    );
    assert_eq!(state.arcology_dialog, None);
    assert_eq!(state.sign_dialog, None);
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.city.label_at(2), Some("Original Sign"));
    assert!(!state.query_is_inspect);
    assert_drag_states_cleared(&state);

    state.open_tool_menu = Some(ui::ToolMenu::Rewards);
    seed_all_modal_dialogs(&mut state);
    state.query_is_inspect = true;
    seed_all_drag_states(&mut state);

    state.open_arcology_dialog();

    assert_eq!(
        state.arcology_dialog,
        Some(ArcologyDialogState {
            info_tool: None,
            pressed_tool: None
        })
    );
    assert_eq!(state.power_plant_dialog, None);
    assert_eq!(state.sign_dialog, None);
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.city.label_at(2), Some("Original Sign"));
    assert!(!state.query_is_inspect);
    assert_drag_states_cleared(&state);
}

#[test]
fn emergency_and_rewards_buttons_are_disabled_by_default() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert!(
        state
            .disabled_palette_actions()
            .contains(&UiAction::OpenMenu(ui::ToolMenu::Emergency))
    );
    assert!(
        state
            .disabled_palette_actions()
            .contains(&UiAction::OpenMenu(ui::ToolMenu::Rewards))
    );
    assert!(!state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Emergency)));
    assert!(!state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Rewards)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::DispatchPolice)));

    state.open_tool_menu(ui::ToolMenu::Emergency);
    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.tool, EditTool::Query);
}

#[test]
fn pe_destroy_building_selector2_highclass_walk_runs_low_endpoint_helpers_in_order() {
    let mut city = City::new_blank(
        "/tmp/sc2000-destroy-sel2-high-dynamic-helpers.sc2",
        "Sel2Helpers",
    )
    .expect("blank city");

    // The initial 0x61 window is class 0x0e (XBIT water-tile), selecting the
    // X axis. Each terminal 0x61 tile has missing XZON corners, so its helper
    // re-enters selector 2 through the low-class fixed-square cleanup.
    for &(x, y) in &[(38, 40), (40, 40), (41, 40), (40, 41), (41, 41), (42, 40)] {
        city.set_building(x, y, 0x61);
        city.set_terrain(x, y, 0);
    }
    for &(x, y) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        city.set_zone(x, y, 0xf3);
    }
    city.set_tile_bits(40, 40, TileBits::new(0x04));
    assert_eq!(pe_highway_classify_463c40(&city, 40, 40), 0x0e);
    assert_eq!(pe_highway_classify_463c40(&city, 38, 40), 4);
    assert_eq!(pe_highway_classify_463c40(&city, 42, 40), 4);

    let seed = 0x4590_c061;
    let mut expected = Win95CrtRand::new(seed);
    // Low-class pre-helper destruction: sixteen scatter draws and four rubble
    // draws. Then five high-walk draws, followed by the same low-class exit
    // helper sequence.
    for _ in 0..45 {
        let _ = expected.random_u15();
    }
    let mut crt = Win95CrtRand::new(seed);
    assert!(pe_flood_destroy_tile_mode_maybe_crt(
        &mut city,
        Some(&mut crt),
        40,
        41,
        true,
    ));
    assert_eq!(
        crt, expected,
        "helper and body draws stay in PE phase order"
    );

    for y in 40..42 {
        for x in 38..40 {
            assert!(
                (1..=4).contains(&city.building_at(x, y)),
                "pre-walk helper leaves the nested low-class rubble at ({x},{y})"
            );
        }
        for x in 40..42 {
            assert_eq!(city.building_at(x, y), 0, "high body pass clears ({x},{y})");
        }
        for x in 42..44 {
            assert!(
                (1..=4).contains(&city.building_at(x, y)),
                "walk-exit helper leaves the nested low-class rubble at ({x},{y})"
            );
        }
    }
    assert_eq!(city.tile_bits_at(40, 40).unwrap().raw_bits(), 0x04);
    for &(x, y) in &[(40, 40), (41, 40), (40, 41), (41, 41)] {
        assert_eq!(city.zone_at(x, y).unwrap().raw_bits(), 0x03);
    }
}

#[test]
fn edit_map_stretch_cancel_restores_cursor_401ce9() {
    // PE Stretch exit `0x401ce9(0x1fc)` also runs when the hold is cancelled.
    let mut city =
        City::new_blank("/tmp/sc2000-stretch-cancel.sc2", "StretchCancel").expect("blank");
    for y in 18..23 {
        for x in 18..23 {
            city.set_altitude(x, y, 5, 0);
        }
    }
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::StretchTerrain;
    assert!(state.begin_edit_map_stretch_drag_at(20, 20, 100.0));
    assert_eq!(state.pe_stretch_hold_cursor_id, 0x1fc);
    state.cancel_edit_map_stretch_drag();
    assert_eq!(state.pe_stretch_hold_cursor_id, 0);
    assert_eq!(state.pe_stretch_hold_cursor_arg, 0);
    assert!(!state.edit_map_stretch_drag_active());
}

#[test]
fn win95_edit_map_level_clears_transient_marker_on_cancel() {
    let city = City::new_blank("/tmp/citygame-win95-level-cancel.sc2", "LevelCancel").unwrap();
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::LevelTerrain;
    for ((x, y), marker) in [((0, 0), 0x41), ((20, 22), 0x42), ((28, 28), 0x43)] {
        state.city.set_text_overlay(x, y, marker);
    }

    assert!(state.begin_edit_map_level_drag_at(0, 0));
    assert_eq!(state.city.text_overlay_at(0, 0), Some(0x41));
    state.hover_tile = Some((20, 22));
    assert!(state.update_edit_map_level_drag());
    assert_eq!(state.city.text_overlay_at(0, 0), Some(0));
    assert_eq!(state.city.text_overlay_at(20, 22), Some(0xc9));
    assert!(!state.update_edit_map_level_drag());
    state.hover_tile = None;
    assert!(!state.update_edit_map_level_drag());
    assert_eq!(state.city.text_overlay_at(20, 22), Some(0xc9));
    state.hover_tile = Some((28, 28));
    assert!(state.update_edit_map_level_drag());
    assert_eq!(state.city.text_overlay_at(20, 22), Some(0));
    assert_eq!(state.city.text_overlay_at(28, 28), Some(0xc9));

    assert!(state.cancel_active_drag());
    assert_eq!(state.city.text_overlay_at(28, 28), Some(0));
    assert!(!state.edit_map_level_drag_active());
}

#[test]
fn win95_edit_map_lower_stops_sampling_after_cancel() {
    let city = City::new_blank("/tmp/citygame-win95-lower-cancel.sc2", "LowerCancel").unwrap();
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.tool = EditTool::LowerTerrain;
    state.hover_tile = Some((20, 20));
    state.pe_process_crt = Win95CrtRand::new(17);

    assert!(state.begin_edit_map_height_hold());
    let seed = state.pe_process_crt.seed();
    assert!(state.cancel_active_drag());
    assert!(!state.tick_edit_map_height_hold(true));
    assert_eq!(state.pe_process_crt.seed(), seed);
}

#[test]
fn pe_terrain_tool_ids_match_edit_menu_writes() {
    // PE `0x47deaf`..`0x47def7`: menu slots write Stretch/Lower/Level/NESW-copy ids.
    assert_eq!(PE_TERRAIN_TOOL_STRETCH, 1);
    assert_eq!(PE_TERRAIN_TOOL_LOWER, 2);
    assert_eq!(PE_TERRAIN_TOOL_LEVEL, 3);
    assert_eq!(PE_TERRAIN_TOOL_NESW_COPY, 4);
    // Tool-4 body is `0x442a30`; EditTool::SpreadTerrain binds the UI path.
    let mut city = City::new_blank("/tmp/sc2000-tool4-id.sc2", "Tool4Id").expect("blank");
    for y in 15..26 {
        for x in 15..26 {
            city.set_altitude(x, y, 9, 0);
        }
    }
    city.set_altitude(20, 19, 4, 0);
    pe_terrain_nesw_land_copy_442a30(&mut city, 20, 20);
    assert_eq!(city.altitude_at(20, 19), 9);
}

#[test]
fn mac_title_generate_and_play_persists_gameplay_run_state_for_both_city_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city =
            City::new_blank("/tmp/sc2000-mac-title-run-state.sc2", "MacTitle").expect("blank city");
        city.kind = kind;
        city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, 0);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::EditNewMap;

        state.apply_new_city_start_flow(
            title_screen::NewCityStartFlow::GenerateAndPlay,
            MacToolboxRng::DEFAULT_SEED,
        );

        assert_eq!(state.mode, AppMode::CityView);
        assert_eq!(
            state.city.mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET),
            Some(1),
            "{kind:?}"
        );
    }
}

#[test]
fn pe_place_tile_size_writes_microsim_xtxt_inside_helper_like_pe() {
    // PE PlaceTile-size @ `0x440ea0` allocates before stamp and writes XTXT per tile in-loop.
    let mut city = City::new_blank("/tmp/sc2000-pts-xtxt.sc2", "PtsXtxt").expect("blank");
    assert!(pe_place_tile_size_440c50(&mut city, 20, 20, 210, 3));
    let xtxt = city.text_overlay_at(20, 20).unwrap_or(0);
    assert!(xtxt >= MIN_SIM_TEXT_ENTRIES);
    for dx in 0..3usize {
        for dy in 0..3usize {
            assert_eq!(city.text_overlay_at(20 + dx, 20 + dy), Some(xtxt));
        }
    }
}

#[test]
fn place_power_lines_skips_outside_crossover_window() {
    // PE @ `0x44a906`–`0x44a90f`: XBLD `>=0x0e` and not `0x1d..=0x4a` → return.
    let mut city = City::new_blank("/tmp/sc2000-pwr-window.sc2", "PwrWin").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x0e);
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 0);
    assert_eq!(state.city.building_at(20, 20), 0x0e);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());

    state.city.set_building(21, 21, 0x4b);
    assert_eq!(state.place_network_at(21, 21, NetworkKind::Power), 0);
    assert_eq!(state.city.building_at(21, 21), 0x4b);
}

#[test]
fn place_power_lines_skips_in_window_non_crossover_slot6() {
    // PE table `0x44a9e8`: in-window XBLD not one of six sources → slot 6 return.
    let mut city = City::new_blank("/tmp/sc2000-pwr-slot6.sc2", "PwrSlot6").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x1f);
    let mut state = test_state(city);
    state.tool = EditTool::Power;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Power), 0);
    assert_eq!(state.city.building_at(20, 20), 0x1f);
    assert!(!state.city.tile_bits_at(20, 20).unwrap().powerable());

    // Contrast: table hit `0x1d` → crossover `0x43`.
    state.city.set_building(22, 22, 0x1d);
    assert_eq!(state.place_network_at(22, 22, NetworkKind::Power), 1);
    assert_eq!(state.city.building_at(22, 22), 0x43);
}

#[test]
fn place_roads_skips_outside_crossover_window() {
    // PE @ `0x44ad2e`–`0x44ad3d`: XBLD not `<0x0e` and not table hit → return.
    let mut city = City::new_blank("/tmp/sc2000-road-window.sc2", "RoadWin").expect("blank");
    city.set_city_funds(50_000);
    city.set_building(20, 20, 0x4d);
    city.set_zone(20, 20, ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL);
    let mut state = test_state(city);
    state.tool = EditTool::Road;
    assert_eq!(state.place_network_at(20, 20, NetworkKind::Road), 0);
    assert_eq!(state.city.building_at(20, 20), 0x4d);
    assert_eq!(
        state.city.zone_at(20, 20).map(|z| z.raw_bits()),
        Some(ZONE_RESIDENTIAL_LIGHT | ZONE_CORNER_ALL)
    );

    // Control: power stub `0x0e` → crossover `0x44`.
    state.city.set_building(22, 22, 0x0e);
    assert_eq!(state.place_network_at(22, 22, NetworkKind::Road), 1);
    assert_eq!(state.city.building_at(22, 22), 0x44);
}

#[test]
fn opening_sign_dialog_clears_stale_modal_and_drag_state() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.open_tool_menu = Some(ui::ToolMenu::Power);
    seed_all_modal_dialogs(&mut state);
    state.query_is_inspect = true;
    seed_all_drag_states(&mut state);

    state.open_sign_dialog(1);

    assert_eq!(
        state.sign_dialog.as_ref().map(|dialog| dialog.sign_id),
        Some(1)
    );
    assert_eq!(state.open_tool_menu, None);
    assert_eq!(state.power_plant_dialog, None);
    assert_eq!(state.arcology_dialog, None);
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.city.label_at(2), Some("Original Sign"));
    assert!(!state.query_is_inspect);
    assert_drag_states_cleared(&state);
}

#[test]
fn emergency_menu_disables_services_without_available_units() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_text_overlay(30, 40, 0xff);
    stamp_complete_building_for_test(&mut city, 100, 100, 210, 3);
    let mut state = test_state(city);

    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::DispatchPolice)));
    for tool in [EditTool::DispatchFirefighters, EditTool::DispatchMilitary] {
        let action = UiAction::SelectTool(tool);
        assert!(!state.ui_action_enabled(action));
        assert!(state.disabled_palette_actions().contains(&action));
    }

    state.open_tool_menu(ui::ToolMenu::Emergency);
    assert_eq!(state.tool, EditTool::DispatchPolice);
}

#[test]
fn reward_mask_enables_only_granted_menu_choices() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_rewards_available_mask(0b1_0100);
    let mut state = test_state(city);

    assert!(state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Rewards)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::MayorHouse)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::CityHall)));
    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::Statue)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::BraunLlamaDome)));
    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::Arcologies)));

    state.open_tool_menu(ui::ToolMenu::Rewards);

    assert_eq!(state.tool, EditTool::Statue);
}

#[test]
fn simulation_month_does_not_recompute_reward_menu_choices() {
    let mut state = test_state(reward_unlock_test_city());

    assert_eq!(state.city.rewards_available_mask(), 0);
    state.handle_simulation_month_advanced();

    assert_eq!(state.city.rewards_available_mask(), 0);
    assert_ne!(state.query_line, "Rewards Available");
    assert!(!state.ui_action_enabled(UiAction::OpenMenu(ui::ToolMenu::Rewards)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::MayorHouse)));
    assert!(!state.ui_action_enabled(UiAction::SelectTool(EditTool::BraunLlamaDome)));
}

#[test]
fn reward_unlock_status_does_not_replace_inspect_query() {
    let mut state = test_state(reward_unlock_test_city());
    state.query_line = "Tile 12,34".to_owned();
    state.query_is_inspect = true;

    state.handle_simulation_month_advanced();

    assert_eq!(state.city.rewards_available_mask() & 0b0_1111, 0);
    assert_eq!(state.query_line, "Tile 12,34");
}

#[test]
fn opening_rewards_menu_does_not_select_arcology_chooser_as_a_default_tool() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);
    state.tool = EditTool::Query;

    state.open_tool_menu(ui::ToolMenu::Rewards);

    assert_eq!(state.open_tool_menu, Some(ui::ToolMenu::Rewards));
    assert_eq!(state.tool, EditTool::Query);
    assert_eq!(state.arcology_dialog, None);
    assert!(state.ui_action_enabled(UiAction::SelectTool(EditTool::Arcologies)));
}

#[test]
fn future_technology_menu_items_are_disabled_in_palette() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_technology_discovery_year(Technology::Highways, 1950);
    city.set_technology_discovery_year(Technology::Buses, 1950);
    city.set_technology_discovery_year(Technology::WaterTreatment, 1950);
    city.set_technology_discovery_year(Technology::Airport, 1950);
    let state = test_state(city);
    let disabled = state.disabled_palette_actions();

    for tool in [
        EditTool::Highway,
        EditTool::Onramp,
        EditTool::BusDepot,
        EditTool::WaterTreatment,
        EditTool::Airport,
    ] {
        assert!(
            disabled.contains(&UiAction::SelectTool(tool)),
            "{tool:?} should draw disabled before discovery"
        );
    }

    for tool in [
        EditTool::Road,
        EditTool::Tunnel,
        EditTool::Pipes,
        EditTool::WaterPump,
        EditTool::Seaport,
    ] {
        assert!(
            !disabled.contains(&UiAction::SelectTool(tool)),
            "{tool:?} should remain enabled"
        );
    }
}

#[test]
fn refresh_network_tile_uses_place_tile_helper() {
    // PE PlaceTile (`0x441f00`) reshape stamps XBLD; leave XTRF alone.
    let mut city = City::new_blank("/tmp/sc2000-net-refresh-place.sc2", "NetPlace").expect("blank");
    city.set_city_funds(20_000);
    let path = vec![(38, 40), (39, 40), (40, 40), (41, 40)];
    let mut state = test_state(city);
    assert_eq!(state.place_network_path(&path, NetworkKind::Road), 4);
    // Wrong orientation on the middle tile — refresh must PlaceTile the mask result.
    state.city.set_building(40, 40, 31); // NS stub vs EW neighbors
    state.city.set_traffic_density(40, 40, 0x2d);
    let before = state.city.building_at(40, 40);
    state.refresh_network_tile(40, 40, NetworkKind::Road);
    let after = state.city.building_at(40, 40);
    assert!(NetworkKind::Road.contains(after));
    assert_ne!(after, before);
    assert_eq!(state.city.traffic_density_at(40, 40), Some(0x2d));
}

#[test]
fn sign_dialog_edits_caps_and_restores_user_label_text() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.tool = EditTool::Sign;

    assert!(state.place_sign_at(10, 10));
    assert_eq!(state.city.text_overlay_at(10, 10), Some(1));
    assert_eq!(
        state.sign_dialog.as_ref().map(|dialog| dialog.sign_id),
        Some(1)
    );

    state
        .sign_dialog
        .as_mut()
        .expect("sign dialog")
        .text
        .clear();
    state.append_sign_dialog_text("Main Street");
    state.commit_sign_dialog();
    assert_eq!(state.city.label_at(1), Some("Main Street"));
    assert!(state.sign_dialog.is_none());

    state.open_sign_dialog(1);
    state
        .sign_dialog
        .as_mut()
        .expect("sign dialog")
        .text
        .clear();
    state.append_sign_dialog_text("123456789012345678901234567890");
    assert_eq!(
        state.sign_dialog.as_ref().expect("sign dialog").text.len(),
        SIGN_LABEL_MAX_BYTES
    );
    state.cancel_sign_dialog();
    assert_eq!(state.city.label_at(1), Some("Main Street"));
    assert!(state.sign_dialog.is_none());
}

#[test]
fn mac_new_sign_waits_for_dialog_commit_before_changing_city() {
    let mut city = City::new_blank("/tmp/mac-pending-sign.city", "Sign").unwrap();
    city.kind = CityFileKind::MacCity;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_sign_at(10, 10));
    assert_eq!(state.city.text_overlay_at(10, 10), Some(0));
    assert_eq!(state.city.label_at(1), Some(""));
    assert_eq!(state.sign_dialog.as_ref().unwrap().text, "");
    state.append_sign_dialog_text("a");
    state.commit_sign_dialog();
    assert_eq!(state.city.text_overlay_at(10, 10), Some(1));
    assert_eq!(state.city.label_at(1), Some("a"));
}

#[test]
fn mac_sign_empty_commit_and_selected_existing_text_match_native() {
    let mut city = City::new_blank("/tmp/mac-sign-empty.city", "Sign").unwrap();
    city.kind = CityFileKind::MacCity;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_sign_at(10, 10));
    state.commit_sign_dialog();
    assert_eq!(state.city.text_overlay_at(10, 10), Some(0));
    assert_eq!(state.city.label_at(1), Some(""));
    assert!(state.place_sign_at(10, 10));
    state.append_sign_dialog_text("abc");
    state.commit_sign_dialog();
    assert!(state.place_sign_at(10, 10));
    assert!(state.sign_dialog.as_ref().unwrap().select_all);
    state.append_sign_dialog_text("z");
    state.commit_sign_dialog();
    assert_eq!(state.city.label_at(1), Some("z"));
    assert_eq!(state.city.text_overlay_at(10, 10), Some(1));
    assert!(state.place_sign_at(10, 10));
    state.backspace_sign_dialog_text();
    state.commit_sign_dialog();
    assert_eq!(state.city.label_at(1), Some(""));
    assert_eq!(state.city.text_overlay_at(10, 10), Some(0));
}

#[test]
fn mac_sign_caret_inserts_and_deletes_at_selected_position() {
    let mut city = City::new_blank("/tmp/mac-sign-caret.city", "Sign").unwrap();
    city.kind = CityFileKind::MacCity;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(state.place_sign_at(10, 10));
    state.append_sign_dialog_text("abc");
    state.commit_sign_dialog();
    assert!(state.place_sign_at(10, 10));
    state.move_sign_dialog_caret(false);
    state.append_sign_dialog_text("z");
    state.move_sign_dialog_caret(true);
    state.move_sign_dialog_caret(true);
    state.append_sign_dialog_text("x");
    assert_eq!(state.sign_dialog.as_ref().unwrap().text, "zabxc");
    state.backspace_sign_dialog_text();
    state.move_sign_dialog_caret(false);
    state.backspace_sign_dialog_text();
    state.commit_sign_dialog();
    assert_eq!(state.city.label_at(1), Some("zbc"));
    assert!(state.place_sign_at(10, 10));
    state.append_sign_dialog_text("éa");
    state.move_sign_dialog_caret(false);
    state.backspace_sign_dialog_text();
    assert_eq!(state.sign_dialog.as_ref().unwrap().text, "a");
}

#[test]
fn mac_sign_dialog_accepts_mac_roman_and_counts_single_byte_characters() {
    let mut city = City::new_blank("/tmp/mac-sign-encoding.city", "test").unwrap();
    city.kind = CityFileKind::MacCity;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.open_sign_dialog(1);
    state.sign_dialog.as_mut().unwrap().text.clear();
    state.append_sign_dialog_text("éééééééééééééééééééééééééZ😀");
    assert_eq!(state.sign_dialog.as_ref().unwrap().text.chars().count(), 24);
    state.commit_sign_dialog();
    assert_eq!(state.city.label_at(1), Some("éééééééééééééééééééééééé"));
}

#[test]
fn palette_surface_tools_map_to_sc2k_building_ids() {
    let cases = [
        (EditTool::Trees, 6),
        (EditTool::BusDepot, 236),
        (EditTool::RailDepot, 237),
        (EditTool::SubStation, 233),
        (EditTool::SmallPark, 13),
        (EditTool::BigPark, 213),
        (EditTool::Zoo, 218),
        (EditTool::Stadium, 215),
        (EditTool::Marina, 248),
        (EditTool::PowerPlantCoal, 207),
        (EditTool::PowerPlantHydro, 198),
        (EditTool::PowerPlantOil, 202),
        (EditTool::PowerPlantGas, 201),
        (EditTool::PowerPlantNuclear, 203),
        (EditTool::PowerPlantWind, 200),
        (EditTool::PowerPlantSolar, 204),
        (EditTool::PowerPlantMicrowave, 205),
        (EditTool::PowerPlantFusion, 206),
        (EditTool::Police, 210),
        (EditTool::FireStation, 211),
        (EditTool::Hospital, 209),
        (EditTool::Prison, 216),
        (EditTool::School, 214),
        (EditTool::College, 217),
        (EditTool::Library, 245),
        (EditTool::Museum, 212),
        (EditTool::WaterPump, 220),
        (EditTool::WaterTower, 235),
        (EditTool::WaterTreatment, 244),
        (EditTool::Desalinization, 250),
        (EditTool::MayorHouse, 243),
        (EditTool::CityHall, 208),
        (EditTool::Statue, 219),
        (EditTool::BraunLlamaDome, 255),
        (EditTool::ArcologyPlymouth, 251),
        (EditTool::ArcologyForest, 252),
        (EditTool::ArcologyDarco, 253),
        (EditTool::ArcologyLaunch, 254),
    ];

    for (tool, building) in cases {
        assert_eq!(
            placeable_building_for_tool(tool),
            Some(building),
            "{tool:?}"
        );
    }
    assert_eq!(placeable_building_for_tool(EditTool::Onramp), None);
    assert_eq!(placeable_building_for_tool(EditTool::SubToRail), None);
    assert_eq!(placeable_building_for_tool(EditTool::Pipes), None);
    assert_eq!(placeable_building_for_tool(EditTool::Subway), None);
    assert_eq!(placeable_building_for_tool(EditTool::Highway), None);
    assert_eq!(placeable_building_for_tool(EditTool::Tunnel), None);
    assert_eq!(placeable_building_for_tool(EditTool::PowerPlant), None);
    assert_eq!(placeable_building_for_tool(EditTool::Arcologies), None);
}

#[test]
fn chooser_palette_tools_do_not_place_default_buildings_directly() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    for tool in [EditTool::PowerPlant, EditTool::Arcologies] {
        state.tool = tool;

        assert_eq!(placeable_building_for_tool(tool), None, "{tool:?}");
        assert_eq!(tool_build_cost(tool), None, "{tool:?}");
        assert_eq!(state.tool_placement_at(20, 20), None, "{tool:?}");
        assert!(
            !state.apply_tool_at_tile(20, 20),
            "{tool:?} should require a dialog choice before placement"
        );
    }
}

#[test]
fn power_plant_palette_choices_use_sc2k_tile_sizes() {
    let cases = [
        (EditTool::PowerPlantWind, 200, 1),
        (EditTool::PowerPlantHydro, 198, 1),
        (EditTool::PowerPlantGas, 201, 4),
        (EditTool::PowerPlantOil, 202, 4),
        (EditTool::PowerPlantNuclear, 203, 4),
        (EditTool::PowerPlantSolar, 204, 4),
        (EditTool::PowerPlantMicrowave, 205, 4),
        (EditTool::PowerPlantFusion, 206, 4),
        (EditTool::PowerPlantCoal, 207, 4),
    ];

    for (tool, building, size) in cases {
        assert_eq!(
            placeable_building_for_tool(tool),
            Some(building),
            "{tool:?}"
        );
        assert_eq!(xbld_tile_size(building), size, "{tool:?}");
    }
}

#[test]
fn tool_build_cost_covers_observed_palette_build_items() {
    let cases = [
        (EditTool::Bulldoze, Some(1)),
        (EditTool::RaiseTerrain, Some(25)),
        (EditTool::LowerTerrain, Some(25)),
        (EditTool::StretchTerrain, Some(25)),
        (EditTool::LevelTerrain, Some(25)),
        (EditTool::SpreadTerrain, Some(25)),
        (EditTool::Trees, Some(3)),
        (EditTool::Forest, Some(3)),
        (EditTool::Water, Some(100)),
        (EditTool::Stream, Some(100)),
        (EditTool::ZoneResidential, Some(5)),
        (EditTool::ZoneResidentialDense, Some(10)),
        (EditTool::ZoneCommercial, Some(5)),
        (EditTool::ZoneCommercialDense, Some(10)),
        (EditTool::ZoneIndustrial, Some(5)),
        (EditTool::ZoneIndustrialDense, Some(10)),
        (EditTool::DeZone, Some(5)),
        (EditTool::Tunnel, Some(150)),
        (EditTool::Onramp, Some(25)),
        (EditTool::BusDepot, Some(250)),
        (EditTool::RailDepot, Some(500)),
        (EditTool::SubStation, Some(250)),
        (EditTool::SubToRail, Some(250)),
        (EditTool::SmallPark, Some(20)),
        (EditTool::BigPark, Some(150)),
        (EditTool::Zoo, Some(3_000)),
        (EditTool::Stadium, Some(5_000)),
        (EditTool::Marina, Some(1_000)),
        (EditTool::PowerPlant, None),
        (EditTool::PowerPlantCoal, Some(4_000)),
        (EditTool::PowerPlantHydro, Some(400)),
        (EditTool::PowerPlantOil, Some(6_600)),
        (EditTool::PowerPlantGas, Some(2_000)),
        (EditTool::PowerPlantNuclear, Some(15_000)),
        (EditTool::PowerPlantWind, Some(100)),
        (EditTool::PowerPlantSolar, Some(1_300)),
        (EditTool::PowerPlantMicrowave, Some(28_000)),
        (EditTool::PowerPlantFusion, Some(40_000)),
        (EditTool::Police, Some(500)),
        (EditTool::FireStation, Some(500)),
        (EditTool::Hospital, Some(500)),
        (EditTool::Prison, Some(3_000)),
        (EditTool::School, Some(250)),
        (EditTool::College, Some(1_000)),
        (EditTool::Library, Some(500)),
        (EditTool::Museum, Some(1_000)),
        (EditTool::WaterPump, Some(100)),
        (EditTool::WaterTower, Some(250)),
        (EditTool::WaterTreatment, Some(500)),
        (EditTool::Desalinization, Some(1_000)),
        (EditTool::Seaport, Some(150)),
        (EditTool::Airport, Some(250)),
        (EditTool::MayorHouse, Some(0)),
        (EditTool::CityHall, Some(0)),
        (EditTool::Statue, Some(0)),
        (EditTool::BraunLlamaDome, Some(0)),
        (EditTool::Arcologies, None),
        (EditTool::ArcologyPlymouth, Some(100_000)),
        (EditTool::ArcologyForest, Some(120_000)),
        (EditTool::ArcologyDarco, Some(150_000)),
        (EditTool::ArcologyLaunch, Some(200_000)),
        (EditTool::Pipes, None),
        (EditTool::Subway, None),
        (EditTool::Highway, None),
    ];

    for (tool, expected) in cases {
        assert_eq!(tool_build_cost(tool), expected, "{tool:?}");
    }
}

#[test]
fn every_menu_tool_routes_to_a_city_action() {
    for menu in ALL_CITY_TOOL_MENUS {
        for item in ui::tool_menu_items(*menu) {
            assert!(
                tool_routes_to_city_action(item.tool),
                "{menu:?} {:?} has a visible menu entry but no city action route",
                item.tool
            );
        }
    }
}

#[test]
fn palette_building_tools_place_through_apply_path() {
    let cases = [
        (EditTool::BusDepot, 236),
        (EditTool::RailDepot, 237),
        (EditTool::SubStation, 233),
        (EditTool::SmallPark, 13),
        (EditTool::BigPark, 213),
        (EditTool::Zoo, 218),
        (EditTool::Stadium, 215),
        (EditTool::Marina, 248),
        (EditTool::PowerPlantCoal, 207),
        (EditTool::PowerPlantHydro, 198),
        (EditTool::PowerPlantOil, 202),
        (EditTool::PowerPlantGas, 201),
        (EditTool::PowerPlantNuclear, 203),
        (EditTool::PowerPlantWind, 200),
        (EditTool::PowerPlantSolar, 204),
        (EditTool::PowerPlantMicrowave, 205),
        (EditTool::PowerPlantFusion, 206),
        (EditTool::Police, 210),
        (EditTool::FireStation, 211),
        (EditTool::Hospital, 209),
        (EditTool::Prison, 216),
        (EditTool::School, 214),
        (EditTool::College, 217),
        (EditTool::Library, 245),
        (EditTool::Museum, 212),
        (EditTool::WaterPump, 220),
        (EditTool::WaterTower, 235),
        (EditTool::WaterTreatment, 244),
        (EditTool::Desalinization, 250),
        (EditTool::MayorHouse, 243),
        (EditTool::CityHall, 208),
        (EditTool::Statue, 219),
        (EditTool::BraunLlamaDome, 255),
        (EditTool::ArcologyPlymouth, 251),
        (EditTool::ArcologyForest, 252),
        (EditTool::ArcologyDarco, 253),
        (EditTool::ArcologyLaunch, 254),
    ];

    for (tool, building) in cases {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_city_funds(500_000);
        let origin = (20, 20);
        match tool {
            EditTool::Marina => {
                for x in origin.0..origin.0 + 3 {
                    city.set_terrain(x, origin.1, 0x30);
                    city.set_tile_bits(x, origin.1, TileBits::new(0).with_water_tile(true));
                }
            }
            EditTool::PowerPlantHydro => {
                city.set_terrain(origin.0, origin.1, 0x3e);
            }
            EditTool::Desalinization => {
                city.set_terrain(origin.0 - 1, origin.1, 0x30);
                city.set_tile_bits(
                    origin.0 - 1,
                    origin.1,
                    crate::city::TileBits::new(0b0000_0001),
                );
            }
            _ => {}
        }
        if let Some(mask) = reward_mask_for_tool(tool) {
            city.set_rewards_available_mask(mask);
        }
        let mut state = test_state(city);
        state.tool = tool;
        let size = usize::from(xbld_tile_size(building));
        let placement = state
            .tool_placement_at(origin.0, origin.1)
            .expect("tool placement");
        let expected_building = match (tool, placement.preview) {
            (EditTool::PowerPlantHydro, ToolPreview::Building(id @ (198 | 199))) => id,
            (_, ToolPreview::Building(id)) => id,
            _ => panic!("{tool:?} should preview a building"),
        };

        if tool == EditTool::PowerPlantHydro {
            assert!(matches!(expected_building, 198 | 199));
        } else {
            assert_eq!(expected_building, building, "{tool:?}");
        }
        assert_eq!(placement.valid_tiles.len(), size * size, "{tool:?}");
        assert!(placement.invalid_tiles.is_empty(), "{tool:?}");
        assert!(state.apply_tool_at_tile(origin.0, origin.1), "{tool:?}");
        assert_eq!(
            state.city.city_funds(),
            500_000 - tool_build_cost(tool).unwrap_or(0) as i32,
            "{tool:?}"
        );

        for (x, y) in
            building_footprint_from_origin(origin.0, origin.1, size).expect("building footprint")
        {
            assert_eq!(state.city.building_at(x, y), expected_building, "{tool:?}");
            if building_has_internal_pipes(expected_building) {
                assert!(
                    state
                        .city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.piped()),
                    "{tool:?} footprint tile ({x}, {y}) should have building plumbing"
                );
            }
        }
    }
}

#[test]
fn palette_building_tools_preview_as_dark_footprints_not_final_art() {
    let origin = (20, 20);
    for tool in [
        EditTool::BusDepot,
        EditTool::RailDepot,
        EditTool::SubStation,
        EditTool::SmallPark,
        EditTool::BigPark,
        EditTool::Zoo,
        EditTool::Stadium,
        EditTool::Marina,
        EditTool::PowerPlantCoal,
        EditTool::PowerPlantHydro,
        EditTool::PowerPlantOil,
        EditTool::PowerPlantGas,
        EditTool::PowerPlantNuclear,
        EditTool::PowerPlantWind,
        EditTool::PowerPlantSolar,
        EditTool::PowerPlantMicrowave,
        EditTool::PowerPlantFusion,
        EditTool::Police,
        EditTool::FireStation,
        EditTool::Hospital,
        EditTool::Prison,
        EditTool::School,
        EditTool::College,
        EditTool::Library,
        EditTool::Museum,
        EditTool::WaterPump,
        EditTool::WaterTower,
        EditTool::WaterTreatment,
        EditTool::Desalinization,
        EditTool::MayorHouse,
        EditTool::CityHall,
        EditTool::Statue,
        EditTool::BraunLlamaDome,
        EditTool::ArcologyPlymouth,
        EditTool::ArcologyForest,
        EditTool::ArcologyDarco,
        EditTool::ArcologyLaunch,
    ] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        if tool == EditTool::Marina {
            for x in origin.0..origin.0 + 3 {
                city.set_terrain(x, origin.1, 0x30);
                // PE marina counts XBIT water-tile, not bare water XTER.
                city.set_tile_bits(x, origin.1, TileBits::new(0).with_water_tile(true));
            }
        }
        if tool == EditTool::PowerPlantHydro {
            city.set_terrain(origin.0, origin.1, 0x3e);
        }
        if tool == EditTool::Desalinization {
            city.set_terrain(origin.0 - 1, origin.1, 0x30);
            city.set_tile_bits(
                origin.0 - 1,
                origin.1,
                crate::city::TileBits::new(0b0000_0001),
            );
        }
        if let Some(mask) = reward_mask_for_tool(tool) {
            city.set_rewards_available_mask(mask);
        }
        let mut state = test_state(city);
        state.tool = tool;

        let placement = state
            .tool_placement_at(origin.0, origin.1)
            .expect("tool placement");

        assert!(
            matches!(placement.preview, ToolPreview::Building(_)),
            "{tool:?}"
        );
        assert!(placement.invalid_tiles.is_empty(), "{tool:?}");
        assert!(
            !placement.preview.draws_valid_diamond(),
            "{tool:?} should use the dark footprint preview branch"
        );
        assert!(
            placement.preview.zone_sprite_id().is_none(),
            "{tool:?} should not use a zone/final-art preview sprite"
        );
    }
}

#[test]
fn pe_newspaper_gift_place_nuclear_483aab_place_tile_size() {
    let mut city = City::new_blank("/tmp/sc2000-gift-nuke.sc2", "GiftNuke").expect("blank");
    city.set_city_funds(50_000);
    assert!(pe_newspaper_gift_place_nuclear_483aab(&mut city, 20, 20));
    assert_eq!(city.building_at(19, 19), NUCLEAR_POWER_BUILDING_ID);
    assert_eq!(city.building_at(20, 20), NUCLEAR_POWER_BUILDING_ID);
}

#[test]
fn pe_newspaper_gift_shore_prep_484320_sets_water_land1_and_xter() {
    // PE @ `0x484320`: XBIT|=water_tile, ALTM land→1, XTER=0x10.
    let mut city = City::new_blank("/tmp/sc2000-gift-shore.sc2", "GiftShore").expect("blank");
    city.set_altitude(20, 20, 9, 3);
    city.set_building(20, 20, 0);
    city.set_tile_bits(20, 20, TileBits::new(0));
    let old_tile_count = city.pe_dw_tile_count;
    pe_newspaper_gift_shore_prep_484320(&mut city, 20, 20);
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
    assert_eq!(city.altitude_at(20, 20), 1);
    assert_eq!(city.altm_at(20, 20).water, 3);
    assert_eq!(city.terrain_at(20, 20), 0x10);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.pe_dw_tile_count, old_tile_count);
}

#[test]
fn pe_newspaper_gift_place_bridge_4838a9_places_rotated_bridge_tiles() {
    // PE @ `0x4838a9`: shore-prep (x,y), PlaceTile 0x57 at (x+1,y+1) and (x+2,y+1)
    // with XBIT rotate.
    let mut city = City::new_blank("/tmp/sc2000-gift-bridge.sc2", "GiftBr").expect("blank");
    pe_newspaper_gift_place_road_stub_483874(&mut city, 8, 8);
    assert_eq!(city.building_at(8, 8), 0x0d);
    pe_newspaper_gift_place_bridge_4838a9(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.terrain_at(20, 20), 0x10);
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
    assert_eq!(city.building_at(21, 21), 0x57);
    assert!(city.tile_bits_at(21, 21).expect("bits").rotate());
    assert_eq!(city.building_at(22, 21), 0x57);
    assert!(city.tile_bits_at(22, 21).expect("bits").rotate());
}

#[test]
fn pe_newspaper_gift_place_bridge_same_tile_483a25_483a43() {
    // PE @ `0x483a25` plain / `0x483a43` rotated same-tile bridge gifts.
    let mut city = City::new_blank("/tmp/sc2000-gift-bridge2.sc2", "GiftBr2").expect("blank");
    pe_newspaper_gift_place_bridge_plain_483a25(&mut city, 30, 30);
    assert_eq!(city.building_at(30, 30), 0x57);
    assert!(!city.tile_bits_at(30, 30).expect("bits").rotate());
    assert!(city.tile_bits_at(30, 30).expect("bits").water_tile());
    pe_newspaper_gift_place_bridge_rotated_483a43(&mut city, 40, 40);
    assert_eq!(city.building_at(40, 40), 0x57);
    assert!(city.tile_bits_at(40, 40).expect("bits").rotate());
    assert!(city.tile_bits_at(40, 40).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_place_bridge_offset_48394d_4839c3_4839f0() {
    let mut city = City::new_blank("/tmp/sc2000-gift-bridge3.sc2", "GiftBr3").expect("blank");
    // PE @ `0x48394d`: prep (20,20), bridge at (19,21) rotated.
    pe_newspaper_gift_place_bridge_48394d(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.terrain_at(20, 20), 0x10);
    assert_eq!(city.building_at(19, 21), 0x57);
    assert!(city.tile_bits_at(19, 21).expect("bits").rotate());
    // PE @ `0x4839c3`: prep (40,40), bridge at (41,39) plain.
    pe_newspaper_gift_place_bridge_4839c3(&mut city, 40, 40);
    assert_eq!(city.building_at(41, 39), 0x57);
    assert!(!city.tile_bits_at(41, 39).expect("bits").rotate());
    // PE @ `0x4839f0`: prep (60,60), bridges at (61,61) and (61,62).
    pe_newspaper_gift_place_bridge_4839f0(&mut city, 60, 60);
    assert_eq!(city.building_at(61, 61), 0x57);
    assert_eq!(city.building_at(61, 62), 0x57);
}

#[test]
fn pe_newspaper_gift_place_table_tile_483412_uses_4ea9d8() {
    let mut city = City::new_blank("/tmp/sc2000-gift-table.sc2", "GiftTbl").expect("blank");
    // index 2 → tile 29, no shore/rotate.
    pe_newspaper_gift_place_table_tile_483412(&mut city, 10, 10, 2);
    assert_eq!(city.building_at(10, 10), 29);
    assert!(!city.tile_bits_at(10, 10).expect("bits").rotate());
    // index 1 → shore-prep then tile 87 + rotate.
    pe_newspaper_gift_place_table_tile_483412(&mut city, 20, 20, 1);
    assert_eq!(city.building_at(20, 20), 87);
    assert!(city.tile_bits_at(20, 20).expect("bits").rotate());
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
    // index 0xd → OR 0xc0 then tile 67.
    pe_newspaper_gift_place_table_tile_483412(&mut city, 30, 30, 0xd);
    assert_eq!(city.building_at(30, 30), 67);
    assert_eq!(
        city.tile_bits_at(30, 30).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_place_table_tile_4834bc_uses_4ea9e8() {
    let mut city = City::new_blank("/tmp/sc2000-gift-table2.sc2", "GiftTb2").expect("blank");
    pe_newspaper_gift_place_table_tile_4834bc(&mut city, 10, 10, 2);
    assert_eq!(city.building_at(10, 10), 14);
    assert_eq!(
        city.tile_bits_at(10, 10).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
    assert!(!city.tile_bits_at(10, 10).expect("bits").rotate());
    pe_newspaper_gift_place_table_tile_4834bc(&mut city, 20, 20, 0);
    assert_eq!(city.building_at(20, 20), 92);
    assert!(city.tile_bits_at(20, 20).expect("bits").rotate());
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_place_table_tile_48358a_uses_4ea9f8() {
    let mut city = City::new_blank("/tmp/sc2000-gift-table3.sc2", "GiftTb3").expect("blank");
    pe_newspaper_gift_place_table_tile_48358a(&mut city, 10, 10, 2);
    assert_eq!(city.building_at(10, 10), 44);
    pe_newspaper_gift_place_table_tile_48358a(&mut city, 20, 20, 1);
    assert_eq!(city.building_at(20, 20), 90);
    assert!(city.tile_bits_at(20, 20).expect("bits").rotate());
    assert!(city.tile_bits_at(20, 20).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_zone_power_48362a_sets_type9_and_power_bits() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zone.sc2", "GiftZn").expect("blank");
    city.set_zone(15, 15, 0xf2); // corners + type 2
    city.set_tile_bits(15, 15, TileBits::new(0));
    pe_newspaper_gift_zone_power_48362a(&mut city, 15, 15);
    assert_eq!(city.zone_at(15, 15).expect("zone").raw_bits(), 0xf9);
    assert_eq!(
        city.tile_bits_at(15, 15).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_zone_power_483aca_sets_type8_and_power_bits() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zone8.sc2", "GiftZn8").expect("blank");
    city.set_zone(15, 15, 0xf3);
    city.set_tile_bits(15, 15, TileBits::new(0));
    pe_newspaper_gift_zone_power_483aca(&mut city, 15, 15);
    assert_eq!(city.zone_at(15, 15).expect("zone").raw_bits(), 0xf8);
    assert_eq!(
        city.tile_bits_at(15, 15).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_zone_type_power_483b48_picks_type_from_story() {
    let mut city = City::new_blank("/tmp/sc2000-gift-ztype.sc2", "GiftZT").expect("blank");
    city.set_zone(10, 10, 0xa0);
    pe_newspaper_gift_zone_type_power_483b48(&mut city, 10, 10, 0x100);
    assert_eq!(city.zone_at(10, 10).expect("z").raw_bits(), 0xa2);
    city.set_zone(20, 20, 0xb0);
    pe_newspaper_gift_zone_type_power_483b48(&mut city, 20, 20, 0x200);
    assert_eq!(city.zone_at(20, 20).expect("z").raw_bits(), 0xb4);
    city.set_zone(30, 30, 0xc0);
    pe_newspaper_gift_zone_type_power_483b48(&mut city, 30, 30, 0x300);
    assert_eq!(city.zone_at(30, 30).expect("z").raw_bits(), 0xc6);
    assert_eq!(
        city.tile_bits_at(30, 30).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_place_hospital_church_483df1_483e14() {
    let mut city = City::new_blank("/tmp/sc2000-gift-hc.sc2", "GiftHC").expect("blank");
    city.set_city_funds(50_000);
    assert!(pe_newspaper_gift_place_hospital_483df1(&mut city, 20, 20));
    // Size 3: PE click (20,20) → post-dec NW (19,19).
    assert_eq!(city.building_at(19, 19), HOSPITAL_BUILDING_ID);
    assert_eq!(city.building_at(20, 20), HOSPITAL_BUILDING_ID);
    assert!(pe_newspaper_gift_place_church_483e14(&mut city, 40, 40));
    assert_eq!(city.building_at(40, 40), CHURCH_BUILDING_ID);
}

#[test]
fn pe_newspaper_gift_mark_zone_corners_436f5f_ors_f0() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zcorn.sc2", "GiftZC").expect("blank");
    city.set_zone(10, 10, 0x03);
    pe_newspaper_gift_mark_zone_corners_436f5f(&mut city, 10, 10);
    assert_eq!(city.zone_at(10, 10).expect("z").raw_bits(), 0xf3);
}

#[test]
fn pe_newspaper_gift_place_rubble_483e37_on_empty_only() {
    let mut city = City::new_blank("/tmp/sc2000-gift-rubble.sc2", "GiftRub").expect("blank");
    let mut crt = Win95CrtRand::new(7);
    city.set_building(12, 12, 0x70);
    pe_newspaper_gift_place_rubble_483e37(&mut city, &mut crt, 12, 12);
    assert_eq!(city.building_at(12, 12), 0x70);
    pe_newspaper_gift_place_rubble_483e37(&mut city, &mut crt, 13, 13);
    let b = city.building_at(13, 13);
    assert!((1..=4).contains(&b), "building {b}");
}

#[test]
fn pe_newspaper_gift_place_trees_4832d9_4832ff_ranges() {
    let mut city = City::new_blank("/tmp/sc2000-gift-trees.sc2", "GiftTrees").expect("blank");
    let mut crt = Win95CrtRand::new(11);
    pe_newspaper_gift_place_trees_4832d9(&mut city, &mut crt, 16, 16);
    let soft = city.building_at(16, 16);
    assert!((6..=9).contains(&soft), "soft {soft}");
    pe_newspaper_gift_place_trees_4832ff(&mut city, &mut crt, 17, 17);
    let hard = city.building_at(17, 17);
    assert!((9..=12).contains(&hard), "hard {hard}");
}

#[test]
fn pe_newspaper_gift_altm_prep_483287_forces_water_2() {
    let mut city = City::new_blank("/tmp/sc2000-gift-altm.sc2", "GiftAltm").expect("blank");
    city.set_altitude(18, 18, 7, 15);
    city.set_tunnel_levels(18, 18, 3);
    pe_newspaper_gift_altm_prep_483287(&mut city, 18, 18);
    let sample = city.altm_at(18, 18);
    assert_eq!(sample.land, 7);
    assert_eq!(sample.water, 2);
    assert_eq!(city.tunnel_levels_at(18, 18), 3);
}

#[test]
fn pe_newspaper_gift_shore_only_4832c1_runs_shore_prep() {
    let mut city = City::new_blank("/tmp/sc2000-gift-shore-only.sc2", "GiftShore").expect("blank");
    city.set_altitude(19, 19, 5, 0);
    pe_newspaper_gift_shore_only_4832c1(&mut city, 19, 19);
    assert_eq!(city.building_at(19, 19), 0);
    assert_eq!(city.terrain_at(19, 19), 0x10);
    assert_eq!(city.altm_at(19, 19).land, 1);
    assert!(city.tile_bits_at(19, 19).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_entry_483250_altm_gate_and_early_arms() {
    let mut city = City::new_blank("/tmp/sc2000-gift-entry.sc2", "GiftEntry").expect("blank");
    let mut crt = Win95CrtRand::new(13);
    city.set_altitude(20, 20, 4, 9);
    // story < 2: ALTM only.
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 20, 20, 1);
    assert_eq!(city.altm_at(20, 20).water, 2);
    assert_eq!(city.building_at(20, 20), 0);
    // soft trees.
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 21, 21, 0x15);
    let soft = city.building_at(21, 21);
    assert!((6..=9).contains(&soft), "soft {soft}");
    // force-tree story 0x34.
    city.set_building(22, 22, 0x70);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 22, 22, 0x34);
    assert_eq!(city.building_at(22, 22), 5);
}

#[test]
fn pe_newspaper_gift_entry_483250_network_table_bands() {
    let mut city = City::new_blank("/tmp/sc2000-gift-entry-tbl.sc2", "GiftEntryT").expect("blank");
    let mut crt = Win95CrtRand::new(14);
    // 0x42 → 4ea9d8[2] == 29
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 24, 24, 0x42);
    assert_eq!(city.building_at(24, 24), 29);
    // 0xd2 → 4ea9e8[2] == 14
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 25, 25, 0xd2);
    assert_eq!(city.building_at(25, 25), 14);
    // 0xe2 → 4ea9f8[2] == 44
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 26, 26, 0xe2);
    assert_eq!(city.building_at(26, 26), 44);
}

#[test]
fn pe_newspaper_gift_entry_483250_zone_power_and_placetile_size() {
    let mut city = City::new_blank("/tmp/sc2000-gift-entry-zps.sc2", "GiftEntryZ").expect("blank");
    let mut crt = Win95CrtRand::new(15);
    city.set_city_funds(50_000);
    // zone-power type9
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 30, 30, 0x2b5);
    assert_eq!(city.zone_at(30, 30).expect("z").raw_bits() & 0x0f, 9);
    assert_eq!(
        city.tile_bits_at(30, 30).expect("b").raw_bits() & 0xc0,
        0xc0
    );
    // zone-power type8
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 31, 31, 0x2c5);
    assert_eq!(city.zone_at(31, 31).expect("z").raw_bits() & 0x0f, 8);
    // fire story 0x2fd — size 3 post-dec NW (31,31)
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 32, 32, 0x2fd);
    assert_eq!(city.building_at(31, 31), FIRE_STATION_BUILDING_ID);
    assert_eq!(city.building_at(32, 32), FIRE_STATION_BUILDING_ID);
    // coal story 0x2f3: caller dec then size-4 dec → NW (38,38)
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 40, 40, 0x2f3);
    assert_eq!(city.building_at(38, 38), COAL_POWER_BUILDING_ID);
    assert_eq!(city.building_at(39, 39), COAL_POWER_BUILDING_ID);
    // road stub
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 33, 33, 0x348);
    assert_eq!(city.building_at(33, 33), 0x0d);
}

#[test]
fn pe_newspaper_gift_entry_483250_zone_type_paint_and_bridge() {
    let mut city = City::new_blank("/tmp/sc2000-gift-entry-ztp.sc2", "GiftEntryP").expect("blank");
    let mut crt = Win95CrtRand::new(16);
    city.set_city_funds(50_000);
    // story 0xf0: zone-type only (ax=0), nibble 2.
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 34, 34, 0xf0);
    assert_eq!(city.zone_at(34, 34).expect("z").raw_bits() & 0x0f, 2);
    assert_eq!(city.building_at(34, 34), 0);
    // story 0xf9: zone-type 2 then size-1 paint with type (2-1)/2=0.
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 35, 35, 0xf9);
    assert_eq!(city.zone_at(35, 35).expect("z").raw_bits() & 0xf0, 0xf0);
    let b = city.building_at(35, 35);
    assert!((0x70..0x74).contains(&u16::from(b)), "building {b}");
    // plain bridge
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 36, 36, 0x33c);
    assert_eq!(city.building_at(36, 36), 0x57);
    assert!(city.tile_bits_at(36, 36).expect("bits").water_tile());
}

#[test]
fn pe_newspaper_gift_entry_483250_nuclear_stadium_twin_size4_zonetype() {
    // PE nuclear exact `0x32b` @ `0x483aa0`; stadium twin `0x31b` @ `0x48375c`;
    // size-4 paint story `0x295` (`0x26d` band ax=4 paint=4); high zone-type-only
    // `0x374` (selector ax=0 after type 6).
    let mut city =
        City::new_blank("/tmp/sc2000-gift-entry-ns4.sc2", "GiftEntryNS4").expect("blank");
    let mut crt = Win95CrtRand::new(19);
    city.set_city_funds(50_000);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 40, 40, 0x32b);
    // Nuclear click (41,38) → NW (40,37).
    assert_eq!(city.building_at(40, 37), NUCLEAR_POWER_BUILDING_ID);
    assert_eq!(city.building_at(41, 38), NUCLEAR_POWER_BUILDING_ID);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 60, 60, 0x31b);
    assert_eq!(city.building_at(60, 57), STADIUM_BUILDING_ID);
    assert_eq!(city.building_at(61, 58), STADIUM_BUILDING_ID);
    assert_eq!(city.pe_w_sports_teams(), 1);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 50, 50, 0x295);
    assert_eq!(city.zone_at(50, 50).expect("ind").raw_bits() & 0x0f, 6);
    // size4 paints from (49,51); paint selector 4 uses table type 2 (0xbc).
    let b = city.building_at(49, 51);
    assert!(
        (0xbc..0xbc + 6).contains(&u16::from(b)),
        "size4 building {b}"
    );
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 70, 70, 0x374);
    assert_eq!(city.zone_at(70, 70).expect("t6").raw_bits() & 0x0f, 6);
    assert_eq!(city.building_at(70, 70), 0, "0x374 paints nothing");
    assert_eq!(
        city.tile_bits_at(70, 70).expect("bits").raw_bits() & 0xc0,
        0xc0
    );
}

#[test]
fn pe_newspaper_gift_place_tree_483e68_destroys_then_places_5() {
    let mut city = City::new_blank("/tmp/sc2000-gift-tree.sc2", "GiftTree").expect("blank");
    city.set_building(14, 14, 0x70);
    let mut crt = Win95CrtRand::new(1);
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 14, 14);
    assert_eq!(city.building_at(14, 14), 5);
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 15, 15);
    assert_eq!(city.building_at(15, 15), 5);
}

#[test]
fn pe_newspaper_gift_place_tree_483e68_destroy_building_consumes_crt_rubble() {
    // PE DestroyBuilding mode 0 @ `0x483e7f` bypasses mode-1 scatter at
    // `0x459c40`, but still makes the conditional CRT PlaceTile rubble draw
    // @ `0x45a01f` before final PlaceTile `5`.
    let mut city = City::new_blank("/tmp/sc2000-gift-tree-crt.sc2", "GiftTCrt").expect("blank");
    city.set_building(14, 14, 0x70);
    let mut preview = Win95CrtRand::new(0x483e_6801);
    let _rubble = preview.rubble_tile_id();
    let mut crt = Win95CrtRand::new(0x483e_6801);
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 14, 14);
    assert_eq!(city.building_at(14, 14), 5);
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn pe_newspaper_gift_place_tree_483e68_elevated_destroy_building_then_5() {
    // PE @ `0x483e7f`: DestroyBuilding mode 0 — elevated arm then PlaceTile `5`.
    let mut city = City::new_blank("/tmp/sc2000-gift-tree-elev.sc2", "GiftTE").expect("blank");
    city.set_building(14, 14, 0x3f);
    city.set_building(11, 14, 0x41);
    city.set_zone(14, 14, ZONE_RESIDENTIAL_LIGHT | 0xf0);
    city.set_tunnel_levels(14, 14, 0x1f);
    let mut crt = Win95CrtRand::new(1);
    let mut preview = crt;
    for _ in 0..4 {
        preview.random_u15();
    }
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 14, 14);
    assert_eq!(city.building_at(14, 14), 5);
    assert_eq!(
        city.zone_at(14, 14).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0
    );
    assert_eq!(city.tunnel_levels_at(14, 14), 0);
    assert_eq!(city.building_at(11, 14), 0);
    assert_eq!(crt.seed(), preview.seed());
}

#[test]
fn pe_newspaper_gift_place_tree_483e68_xbld_lt6_skips_destroy_building() {
    // PE DestroyBuilding `cmp XBLD,6` / `jl` @ `0x458d8a`: no clear; PlaceTile `5`.
    let mut city = City::new_blank("/tmp/sc2000-gift-tree-lt6.sc2", "GiftT6").expect("blank");
    city.set_building(14, 14, 3);
    city.set_zone(14, 14, ZONE_RESIDENTIAL_LIGHT | 0x80);
    let mut crt = Win95CrtRand::new(1);
    let seed_before = crt.seed();
    pe_newspaper_gift_place_tree_483e68(&mut city, &mut crt, 14, 14);
    assert_eq!(city.building_at(14, 14), 5);
    // Zone corners survive the DestroyBuilding no-op (PlaceTile `5` does not clear them).
    assert_eq!(
        city.zone_at(14, 14).map(|z| z.raw_bits()).unwrap_or(0) & 0xf0,
        0x80
    );
    assert_eq!(crt.seed(), seed_before);
}

#[test]
fn pe_newspaper_gift_zone_paint_size1_436ee7_places_and_marks_corners() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint.sc2", "GiftZP").expect("blank");
    let mut crt = Win95CrtRand::new(1);
    pe_newspaper_gift_zone_paint_size1_436ee7(&mut city, &mut crt, 20, 20, 1);
    let b = city.building_at(20, 20);
    assert!((0x7c..0x7c + 8).contains(&u16::from(b)), "building {b}");
    assert_eq!(city.zone_at(20, 20).expect("z").raw_bits() & 0xf0, 0xf0);
    let bits = city.tile_bits_at(20, 20).expect("bits").raw_bits();
    assert_eq!(bits & XBIT_CHOOSE_AND_PLACE, XBIT_CHOOSE_AND_PLACE);
}

#[test]
fn pe_newspaper_gift_zone_story_bands_use_the_native_building_table_selector() {
    for (story_id, table_type) in [(0x158, 0), (0x1e8, 1), (0x272, 2)] {
        let mut city =
            City::new_blank("/tmp/sc2000-gift-zone-table.sc2", "GiftZoneTable").expect("blank");
        let mut crt = Win95CrtRand::new(0x1937);
        let mut expected_crt = crt;
        let expected = if table_type == 0 {
            0x70 + expected_crt.random_u16_mask(3) as u8
        } else {
            pe_zone_paint_pick_building_4e755e(&mut expected_crt, 1, table_type)
        };
        pe_newspaper_gift_entry_483250(&mut city, &mut crt, 20, 20, story_id);
        assert_eq!(
            city.building_at(20, 20),
            expected,
            "story {story_id:#x} native zone table index {table_type}"
        );
        assert_eq!(crt.seed(), expected_crt.seed(), "story {story_id:#x} CRT");
    }
}

#[test]
fn pe_newspaper_gift_zone_paint_size_gt1_436fb6_stamps_2x2() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint2.sc2", "GiftZP2").expect("blank");
    let mut crt = Win95CrtRand::new(2);
    assert!(pe_newspaper_gift_zone_paint_size_gt1_436fb6(
        &mut city, &mut crt, 20, 20, 2, 0
    ));
    // size=2 → half=1 → (x..x+1, y-1..y); type0 base 0x8c mod 4.
    let b = city.building_at(20, 20);
    assert!((0x8c..0x8c + 4).contains(&u16::from(b)), "building {b}");
    assert_eq!(city.building_at(21, 20), b);
    assert_eq!(city.building_at(20, 19), b);
    assert_eq!(city.building_at(21, 19), b);
    let bits = city.tile_bits_at(20, 20).expect("bits").raw_bits();
    assert_eq!(bits & XBIT_CHOOSE_AND_PLACE, XBIT_CHOOSE_AND_PLACE);
    let corners = pe_choose_and_place_corner_bits(city.rotation());
    assert_eq!(
        city.zone_at(20, 19).expect("nw").raw_bits() & 0xf0,
        corners[0]
    );
    assert_eq!(
        city.zone_at(21, 19).expect("ne").raw_bits() & 0xf0,
        corners[1]
    );
    assert_eq!(
        city.zone_at(20, 20).expect("sw").raw_bits() & 0xf0,
        corners[2]
    );
    assert_eq!(
        city.zone_at(21, 20).expect("se").raw_bits() & 0xf0,
        corners[3]
    );
}

#[test]
fn pe_newspaper_gift_zone_paint_size2_483d08_empty_falls_to_size1() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint-s2e.sc2", "GiftS2e").expect("blank");
    let mut crt = Win95CrtRand::new(9);
    // type 1 (not 4) + empty XBLD → size-1 paint from table index 5 (base 0x7c).
    pe_newspaper_gift_zone_paint_size2_483d08(&mut city, &mut crt, 22, 22, 1, 1);
    let b = u16::from(city.building_at(22, 22));
    assert!((0x7c..0x7c + 8).contains(&b), "building {b}");
    assert_eq!(city.zone_at(22, 22).expect("z").raw_bits() & 0xf0, 0xf0);
}

#[test]
fn pe_newspaper_gift_zone_paint_size2_483d08_type4_paints_size2() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint-s2.sc2", "GiftS2").expect("blank");
    let mut crt = Win95CrtRand::new(3);
    pe_newspaper_gift_zone_paint_size2_483d08(&mut city, &mut crt, 30, 30, 4, 4);
    // type4 size2 base 0xaa mod 2 — at least one tile in the jittered 2×2 must match.
    let mut found = false;
    for dx in 0..3u16 {
        for dy in 0..3u16 {
            let x = 30usize.wrapping_add(dx as usize).wrapping_sub(1);
            let y = 30usize.wrapping_add(dy as usize).wrapping_sub(1);
            if x >= CITY_WIDTH || y >= CITY_HEIGHT {
                continue;
            }
            let b = u16::from(city.building_at(x, y));
            if (0xaa..0xaa + 2).contains(&b) {
                found = true;
            }
        }
    }
    assert!(found, "expected size2 type4 building near origin");
}

#[test]
fn pe_newspaper_gift_zone_paint_size3_483d6b_type4_paints_half1() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint-s3.sc2", "GiftS3").expect("blank");
    let mut crt = Win95CrtRand::new(4);
    pe_newspaper_gift_zone_paint_size3_483d6b(&mut city, &mut crt, 40, 40, 4, 4);
    // size3 → half=1 → 2×2; type4 base 0xac mod 2.
    let mut found = false;
    for dx in 0..3usize {
        for dy in 0..3usize {
            let x = 40usize.wrapping_add(dx).wrapping_sub(1);
            let y = 40usize.wrapping_add(dy).wrapping_sub(1);
            if x >= CITY_WIDTH || y >= CITY_HEIGHT {
                continue;
            }
            let b = u16::from(city.building_at(x, y));
            if (0xac..0xac + 2).contains(&b) {
                found = true;
            }
        }
    }
    assert!(found, "expected size3 type4 building near origin");
}

#[test]
fn pe_newspaper_gift_zone_paint_size4_483dce_type4_at_offset() {
    let mut city = City::new_blank("/tmp/sc2000-gift-zpaint-s4.sc2", "GiftS4").expect("blank");
    let mut crt = Win95CrtRand::new(5);
    pe_newspaper_gift_zone_paint_size4_483dce(&mut city, &mut crt, 50, 50, 4, 4);
    // origin (49, 51); size4 → half=2 → 3×3; type4 base 0xc4 mod 2.
    let b = u16::from(city.building_at(49, 51));
    assert!((0xc4..0xc4 + 2).contains(&b), "building {b}");
    assert_eq!(u16::from(city.building_at(51, 51)), b);
    assert_eq!(u16::from(city.building_at(49, 49)), b);
    pe_newspaper_gift_zone_paint_size4_483dce(&mut city, &mut crt, 50, 50, 1, 4);
    // type≠4 skips — leave prior stamp.
    assert_eq!(u16::from(city.building_at(49, 51)), b);
}

#[test]
fn pe_newspaper_gift_size4_keeps_paint_selector_separate_from_table_type() {
    let mut city =
        City::new_blank("/tmp/sc2000-gift-zpaint-s4-type2.sc2", "GiftS4T2").expect("blank");
    // METROPOL tile 0x2a7 reaches the size-4 arm with paint selector 4 but
    // table type 2. Its captured retail entry seed selects 0xbf from 0xbc..0xc1.
    let mut crt = Win95CrtRand::new(0x305e_1c7b);
    pe_newspaper_gift_zone_paint_size4_483dce(&mut city, &mut crt, 74, 122, 4, 2);
    assert_eq!(city.building_at(73, 123), 0xbf);
    assert_eq!(city.building_at(75, 122), 0xbf);
    assert_eq!(crt.seed(), 0xc40d_f552);
}

#[test]
fn pe_newspaper_gift_residential_pop1_uses_xval_band_and_rand_mask() {
    let mut city = City::new_blank("/tmp/sc2000-gift-xval-pop1.sc2", "GiftXval").expect("blank");
    // A blank new-city XVAL plane reads as band 0. PE `0x436ea0` uses
    // `rand & 3`, not the generic `% 12` table used by other zone types.
    let mut crt = Win95CrtRand::new(0xaa09_6edb);
    pe_newspaper_gift_entry_483250(&mut city, &mut crt, 31, 37, 0xfc);
    assert_eq!(city.building_at(31, 37), 0x71);
    assert_eq!(crt.seed(), 0xd00d_7e32);
}

#[test]
fn highway_xter_0x06_stamps_class2_underlay_not_dialog_0x74() {
    // PE CityTool `0x4622f6` has no AtCoordinates XTER dialog. Category-2
    // `0x06` raises origin altitude, so class 2 underlay rewrites `XTER 0x0d`.
    let mut city = City::new_blank("/tmp/sc2000-hwy-xter6.sc2", "HwyXter6").expect("blank");
    city.set_city_funds(50_000);
    let tiles = highway_segment_tiles((20, 20), NetworkAxis::GridX);
    for &(x, y) in &tiles {
        city.set_terrain(x, y, 0x06);
    }
    let mut state = test_state(city);
    state.tool = EditTool::Highway;
    assert_eq!(state.place_highway_path(&[(20, 20)]), 1);
    for &(x, y) in &tiles {
        assert_eq!(
            state.city.building_at(x, y),
            0x49,
            "class 2 underlay at ({x},{y})"
        );
        assert_eq!(
            state.city.terrain_at(x, y),
            0x0d,
            "nonzero XTER counts as higher altitude at ({x},{y})"
        );
    }
}

#[test]
fn power_plant_dialog_hit_tests_cancel_info_and_choice_cards() {
    let window_size = (1024, 768);
    let dialog = power_plant_dialog_rect(window_size);
    let cancel = power_plant_cancel_rect(dialog);
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(cancel.x + 4.0, cancel.y + 4.0), window_size),
        Some(PowerPlantDialogAction::Cancel)
    );

    let info = power_plant_info_rect(dialog, 0);
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(info.x + 4.0, info.y + 4.0), window_size),
        Some(PowerPlantDialogAction::Info(EditTool::PowerPlantCoal))
    );

    let hydro = power_plant_card_rect(dialog, 1);
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(hydro.x + 4.0, hydro.y + 4.0), window_size),
        Some(PowerPlantDialogAction::Select(EditTool::PowerPlantHydro))
    );
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(dialog.x - 1.0, dialog.y - 1.0), window_size),
        None
    );
    assert_eq!(
        power_plant_dialog_hit_test(Vec2::new(dialog.x + 12.0, dialog.y + 52.0), window_size),
        None
    );
    assert!(point_in_power_plant_dialog(
        Vec2::new(dialog.x + 12.0, dialog.y + 52.0),
        window_size
    ));
    assert!(!point_in_power_plant_dialog(
        Vec2::new(dialog.x - 1.0, dialog.y - 1.0),
        window_size
    ));
}

#[test]
fn power_plant_dialog_selects_only_matching_pressed_card() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(4_000);
    let mut state = test_state(city);
    state.open_power_plant_dialog();

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantCoal,
    ));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").pressed_tool,
        Some(EditTool::PowerPlantCoal)
    );
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantHydro
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert_eq!(state.power_plant_dialog.expect("dialog").pressed_tool, None);

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantCoal,
    ));
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantCoal
        ))),
        Some(EditTool::PowerPlantCoal)
    );
    assert_eq!(state.tool, EditTool::PowerPlantCoal);
    assert_eq!(state.power_plant_dialog, None);
}

#[test]
fn power_plant_dialog_ignores_unaffordable_cards_but_keeps_info_enabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(399);
    let mut state = test_state(city);
    state.open_power_plant_dialog();

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Info(
        EditTool::PowerPlantHydro,
    ));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").info_tool,
        Some(EditTool::PowerPlantHydro)
    );

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantHydro,
    ));
    assert_eq!(state.power_plant_dialog.expect("dialog").pressed_tool, None);
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantHydro
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.power_plant_dialog.is_some());

    state.city.set_city_funds(400);
    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantHydro,
    ));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").pressed_tool,
        Some(EditTool::PowerPlantHydro)
    );
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantHydro
        ))),
        Some(EditTool::PowerPlantHydro)
    );
    assert_eq!(state.tool, EditTool::PowerPlantHydro);
}

#[test]
fn power_plant_dialog_ignores_future_locked_cards_but_keeps_info_enabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(50_000);
    city.set_technology_discovery_year(Technology::GasPower, 1950);
    let mut state = test_state(city);
    state.open_power_plant_dialog();

    state
        .apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Info(EditTool::PowerPlantGas));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").info_tool,
        Some(EditTool::PowerPlantGas)
    );

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantGas,
    ));
    assert_eq!(state.power_plant_dialog.expect("dialog").pressed_tool, None);
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantGas
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.power_plant_dialog.is_some());

    state
        .city
        .set_technology_discovery_year(Technology::GasPower, 1900);
    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantGas,
    ));
    assert_eq!(
        state.power_plant_dialog.expect("dialog").pressed_tool,
        Some(EditTool::PowerPlantGas)
    );
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantGas
        ))),
        Some(EditTool::PowerPlantGas)
    );
    assert_eq!(state.tool, EditTool::PowerPlantGas);
}

#[test]
fn power_plant_dialog_selection_refreshes_status_line() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(4_000);
    let mut state = test_state(city);
    state.open_power_plant_dialog();
    state.query_line = "Tile 10,10".to_owned();
    state.query_is_inspect = true;

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantCoal,
    ));
    assert_eq!(
        state.finish_power_plant_dialog_mouse_up(Some(PowerPlantDialogAction::Select(
            EditTool::PowerPlantCoal
        ))),
        Some(EditTool::PowerPlantCoal)
    );

    assert_eq!(state.tool, EditTool::PowerPlantCoal);
    assert!(!state.query_is_inspect);
    assert!(
        state.query_line.starts_with("Coal Power $4,000  "),
        "status line was {}",
        state.query_line
    );
}

#[test]
fn power_plant_dialog_info_and_blank_release_keep_dialog_open() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(6_600);
    let mut state = test_state(city);
    state.open_power_plant_dialog();

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Select(
        EditTool::PowerPlantOil,
    ));
    assert_eq!(state.finish_power_plant_dialog_mouse_up(None), None);
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.power_plant_dialog.is_some());

    state.apply_power_plant_dialog_mouse_down(PowerPlantDialogAction::Info(
        EditTool::PowerPlantCoal,
    ));
    assert_eq!(
        state.power_plant_dialog,
        Some(PowerPlantDialogState {
            info_tool: Some(EditTool::PowerPlantCoal),
            pressed_tool: None
        })
    );
}

#[test]
fn power_plant_dialog_uses_original_order_and_labels() {
    assert_eq!(
        POWER_PLANT_CHOICES
            .iter()
            .map(|choice| choice.label)
            .collect::<Vec<_>>(),
        vec![
            "Coal Power",
            "Hydro Electric",
            "Oil Power",
            "Gas Power",
            "Nuclear",
            "Wind Power",
            "Solar Power",
            "Microwave",
            "Fusion"
        ]
    );
    assert_eq!(
        power_plant_choice_for_tool(EditTool::PowerPlantCoal)
            .expect("coal")
            .info,
        "COAL POWER - coal plants generate large amounts of vile black smoke that will cover your buildings in soot. Your citizens may start complaining when they have to repaint their buildings every two years."
    );
}

#[test]
fn arcology_dialog_hit_tests_cancel_info_and_choice_cards() {
    let window_size = (1024, 768);
    let dialog = arcology_dialog_rect(window_size);
    let cancel = arcology_cancel_rect(dialog);
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(cancel.x + 4.0, cancel.y + 4.0), window_size),
        Some(ArcologyDialogAction::Cancel)
    );

    let info = arcology_info_rect(dialog, 0);
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(info.x + 4.0, info.y + 4.0), window_size),
        Some(ArcologyDialogAction::Info(EditTool::ArcologyPlymouth))
    );

    let launch = arcology_card_rect(dialog, 3);
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(launch.x + 4.0, launch.y + 4.0), window_size),
        Some(ArcologyDialogAction::Select(EditTool::ArcologyLaunch))
    );
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(dialog.x - 1.0, dialog.y - 1.0), window_size),
        None
    );
    assert_eq!(
        arcology_dialog_hit_test(Vec2::new(dialog.x + 12.0, dialog.y + 52.0), window_size),
        None
    );
    assert!(point_in_arcology_dialog(
        Vec2::new(dialog.x + 12.0, dialog.y + 52.0),
        window_size
    ));
    assert!(!point_in_arcology_dialog(
        Vec2::new(dialog.x - 1.0, dialog.y - 1.0),
        window_size
    ));
}

#[test]
fn arcology_dialog_selects_only_matching_pressed_card() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(200_000);
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);
    state.open_arcology_dialog();

    state
        .apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyPlymouth));
    assert_eq!(
        state.arcology_dialog.expect("dialog").pressed_tool,
        Some(EditTool::ArcologyPlymouth)
    );
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyForest
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert_eq!(state.arcology_dialog.expect("dialog").pressed_tool, None);

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyLaunch));
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyLaunch
        ))),
        Some(EditTool::ArcologyLaunch)
    );
    assert_eq!(state.tool, EditTool::ArcologyLaunch);
    assert_eq!(state.arcology_dialog, None);
}

#[test]
fn arcology_dialog_ignores_unavailable_or_unaffordable_cards_but_keeps_info_enabled() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(200_000);
    let mut state = test_state(city);
    state.open_arcology_dialog();

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Info(EditTool::ArcologyLaunch));
    assert_eq!(
        state.arcology_dialog.expect("dialog").info_tool,
        Some(EditTool::ArcologyLaunch)
    );

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyLaunch));
    assert_eq!(state.arcology_dialog.expect("dialog").pressed_tool, None);

    state.city.set_rewards_available_mask(1 << 4);
    state.city.set_city_funds(199_999);
    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyLaunch));
    assert_eq!(state.arcology_dialog.expect("dialog").pressed_tool, None);
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyLaunch
        ))),
        None
    );
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.arcology_dialog.is_some());

    state.city.set_city_funds(200_000);
    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyLaunch));
    assert_eq!(
        state.arcology_dialog.expect("dialog").pressed_tool,
        Some(EditTool::ArcologyLaunch)
    );
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyLaunch
        ))),
        Some(EditTool::ArcologyLaunch)
    );
    assert_eq!(state.tool, EditTool::ArcologyLaunch);
}

#[test]
fn arcology_dialog_selection_refreshes_status_line() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(100_000);
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);
    state.open_arcology_dialog();
    state.query_line = "Tile 10,10".to_owned();
    state.query_is_inspect = true;

    state
        .apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyPlymouth));
    assert_eq!(
        state.finish_arcology_dialog_mouse_up(Some(ArcologyDialogAction::Select(
            EditTool::ArcologyPlymouth
        ))),
        Some(EditTool::ArcologyPlymouth)
    );

    assert_eq!(state.tool, EditTool::ArcologyPlymouth);
    assert!(!state.query_is_inspect);
    assert!(
        state.query_line.starts_with("Plymouth Arco $100,000  "),
        "status line was {}",
        state.query_line
    );
}

#[test]
fn arcology_dialog_info_and_blank_release_keep_dialog_open() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(150_000);
    city.set_rewards_available_mask(1 << 4);
    let mut state = test_state(city);
    state.open_arcology_dialog();

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Select(EditTool::ArcologyDarco));
    assert_eq!(state.finish_arcology_dialog_mouse_up(None), None);
    assert_eq!(state.tool, EditTool::Query);
    assert!(state.arcology_dialog.is_some());

    state.apply_arcology_dialog_mouse_down(ArcologyDialogAction::Info(EditTool::ArcologyPlymouth));
    assert_eq!(
        state.arcology_dialog,
        Some(ArcologyDialogState {
            info_tool: Some(EditTool::ArcologyPlymouth),
            pressed_tool: None
        })
    );
}

#[test]
fn arcology_dialog_uses_original_order_ids_capacities_and_costs() {
    assert_eq!(
        ARCOLOGY_CHOICES
            .iter()
            .map(|choice| (choice.label, choice.building, choice.capacity, choice.cost))
            .collect::<Vec<_>>(),
        vec![
            ("Plymouth Arco", 251, 55_000, 100_000),
            ("Forest Arco", 252, 30_000, 120_000),
            ("Darco", 253, 45_000, 150_000),
            ("Launch Arco", 254, 65_000, 200_000),
        ]
    );
}

#[test]
fn underground_palette_tools_place_xund_tiles() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(
        state.place_underground_at(20, 20, UndergroundKind::Pipes),
        1
    );
    // PE PlacePipes reshape mask0→0x0f → XUND 0x1e after post-place refresh.
    assert_eq!(state.city.underground_at(20, 20), 0x1e);
    assert_eq!(state.city.building_at(20, 20), 0);

    assert_eq!(
        state.place_underground_at(20, 21, UndergroundKind::Pipes),
        1
    );
    assert_eq!(state.city.underground_at(20, 20), 0x10);
    assert_eq!(state.city.underground_at(20, 21), 0x10);

    assert_eq!(
        state.place_underground_at(21, 20, UndergroundKind::Subway),
        1
    );
    assert_eq!(state.city.underground_at(21, 20), 1);
}

#[test]
fn eq_le_tables_survive_graph_history_writes_and_reload() {
    let path = std::env::temp_dir().join(format!("sc2000-eq-le-graph-{}.sc2", std::process::id()));
    let mut city = City::new_blank(&path, "eqg").expect("blank");
    city.test_seed_pe_age_cohorts_from_pop(10_000);
    city.advance_pe_eq_le_month(
        10_000,
        0,
        0,
        0,
        100,
        100,
        100,
        &mut crate::win95_random::Win95CrtRand::new(1),
    );
    let school = city.pe_age_cohort_at(1) + city.pe_age_cohort_at(2);
    let life_avg = city.pe_eq_life_average();
    assert!(school > 0);
    assert!(city.pe_eq_averages_ready());
    let history = (0..GRAPH_HISTORY_VALUES as u32).collect::<Vec<_>>();
    let mut xgrp = vec![0; XGRP_VALUES];
    xgrp[..GRAPH_HISTORY_VALUES].copy_from_slice(&history);
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &xgrp);
    city.save(&path).expect("save");

    let loaded = City::load(&path).expect("reload");
    assert_eq!(
        loaded.pe_age_cohort_at(1) + loaded.pe_age_cohort_at(2),
        school,
        "graph ring must not occupy PE EQ_LE MISC 0x7c"
    );
    assert_eq!(loaded.pe_eq_life_average(), life_avg);
    assert!(loaded.pe_eq_averages_ready());
    assert_eq!(
        loaded.chunk_u32_values(XGRP_CHUNK_NAME, 0, GRAPH_HISTORY_VALUES),
        history
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn january_queues_newspaper_story_when_power_plant_ages_past_48() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let coal = city.provision_microsim_for_building(207).expect("coal");
    let coal_index = usize::from(coal - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(coal_index, 48)); // next January → 49
    city.refresh_structure_microsim_yearly(0, 100, 100, 100, 100, 100);
    assert_eq!(city.microsim_at(coal_index).map(|r| r.stat0), Some(49));
    // PE story 0x24 with priority 100 sorts above default priority-0 fillers.
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(0x24));
    assert_eq!(
        city.pe_newspaper_story_param_at(0),
        Some(6),
        "coal tile 207 → type index 207-201"
    );
    assert_eq!(
        pe_power_plant_aging_headline(NewspaperEdition::Courier, 6),
        "Aging coal power plant"
    );
    assert_eq!(
        pe_power_plant_aging_headline(NewspaperEdition::Extra, 6),
        "Extra: Aging coal plant"
    );
    assert!(
        city.take_pe_newspaper_extra_popup_pending(),
        "PE insert of 0x24 latches Extra auto-open"
    );
    assert!(!city.take_pe_newspaper_extra_popup_pending());
    assert!(crate::city::pe_newspaper_story_triggers_extra_popup(0x24));
    assert!(crate::city::pe_newspaper_story_triggers_extra_popup(3));
    assert!(crate::city::pe_newspaper_story_triggers_extra_popup(5));
    assert!(!crate::city::pe_newspaper_story_triggers_extra_popup(2));
    assert!(!crate::city::pe_newspaper_story_triggers_extra_popup(0x0B));
}

#[test]
fn simulation_day0_january_skips_extra_when_newspaper_extra_is_off() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    assert!(set_city_auto_budget_enabled(&mut city, true));
    // Blank cities default Extra off; keep it off (setter returns false when unchanged).
    let _ = set_city_newspaper_extra_enabled(&mut city, false);
    assert!(!newspaper_extra_enabled_for_city(&city));
    let coal = city.provision_microsim_for_building(207).expect("coal");
    let coal_index = usize::from(coal - MIN_SIM_TEXT_ENTRIES);
    assert!(city.set_microsim_stat0(coal_index, 48));
    city.set_misc_u32_at(MISC_YEAR_END_OFFSET, 1);
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Llama);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x24));
    assert_eq!(state.city_info_dialog, None);
    // Insert still latched; app cleared it without opening because Extra was off.
    assert!(!state.city.take_pe_newspaper_extra_popup_pending());
}

#[test]
fn pe_opinion_poll_callers_match_native_approval_and_crt() {
    let mut city = City::new_blank("/tmp/pe-poll.sc2", "Poll").unwrap();
    city.set_saved_date(CityDate {
        year: 1955,
        month: 6,
    });
    let mayor = city.provision_microsim_for_building(243).unwrap();
    let index = usize::from(mayor - MIN_SIM_TEXT_ENTRIES);
    city.set_microsim_stat0(index, 255);
    city.set_microsim_stat3(index, 1);
    city.set_text_overlay(10, 20, crate::city::QUERY_SPECIFIC_MAYOR_STATS_OVERLAY);
    city.set_pe_mayor_approval(63);
    city.set_pe_traffic_count(10);
    city.set_pe_unemployment_percent(12);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 25);
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 10);
    set_city_newspaper_choice(&mut city, 0);

    let newspaper = CityInfoDialog::Newspaper(NewspaperEdition::Courier);
    for (population, tax_rate, land_value, approval, final_seed) in [
        (1_000, 7, 40, 23, 689_332_111),
        (1_000, 0x0001_0018, 40, 18, 689_332_111),
        (100, 7, 40, 23, 689_332_111),
        (99, 7, 40, 63, 123),
        (1_000, 7, 65_258, 63, 123),
    ] {
        let mut fixture = city.clone();
        fixture.set_pe_dw_city_population(population);
        fixture.set_pe_attitude_demand_inputs(0, 20, land_value, 30);
        fixture.recompute_pe_attitude_scores(0, 0, 0);
        let tax_offset = tax_rate_misc_offset_for_city(&fixture, 0).unwrap();
        fixture.set_budget_misc_u32_values(tax_offset, &[tax_rate]);
        for kind in [CityFileKind::WinSc2, CityFileKind::MacCity] {
            let mut query_city = fixture.clone();
            query_city.kind = kind;
            let tax_offset = tax_rate_misc_offset_for_city(&query_city, 0).unwrap();
            query_city.set_budget_misc_u32_values(tax_offset, &[tax_rate]);
            let mut query = test_state(query_city);
            query.pe_process_crt = Win95CrtRand::new(123);
            query.begin_tile_inspect(10, 20);
            let row = query.city.microsim_at(index).unwrap();
            assert_eq!(
                (row.stat0, row.stat1, row.stat2, row.stat3),
                (0, 1955, approval, 0)
            );
            assert_eq!(query.city.pe_mayor_approval(), approval);
            assert_eq!(query.pe_process_crt_seed(), final_seed);
        }

        let mut paper = test_state(fixture);
        paper.menu_checks.citygame_newspaper = true;
        paper.pe_process_crt = Win95CrtRand::new(123);
        paper.open_city_info_dialog(newspaper);
        assert_eq!(paper.city_info_dialog, Some(newspaper));
        assert_eq!(paper.city.pe_mayor_approval(), approval);
        let expected_poll = if population < 100 || land_value == 65_258 {
            ([0; 7], [0, 1, 2, 3, 4, 5, 6])
        } else if tax_rate == 0x0001_0018 {
            ([28, 20, 15, 9, 4, 3, 3], [5, 6, 3, 2, 1, 0, 4])
        } else {
            ([23, 21, 12, 8, 6, 4, 3], [5, 6, 3, 1, 2, 4, 0])
        };
        assert_eq!(paper.city.pe_opinion_poll_results(), expected_poll);
        assert_eq!(paper.city.pe_newspaper_story_id_at(7), Some(0));
        assert_eq!(
            paper.city.pe_newspaper_story_param_at(7),
            Some(u32::from(paper.city.pe_b_weather_trend))
        );
        assert_eq!(
            paper.city.pe_newspaper_story_param_at(8),
            Some(u32::from(expected_poll.1[0]))
        );
        assert_eq!(paper.pe_process_crt_seed(), final_seed);
        let row = paper.city.microsim_at(index).unwrap();
        assert_eq!(
            (row.stat0, row.stat1, row.stat2, row.stat3),
            (255, 1955, 21, 1)
        );
        paper.open_city_info_dialog(newspaper);
        assert_eq!(paper.city_info_dialog, None);
        assert_eq!(paper.pe_process_crt_seed(), final_seed);
    }

    let mut mac = test_state(city);
    mac.engine_fidelity = EngineFidelity::Mac68k;
    mac.pe_process_crt = Win95CrtRand::new(123);
    mac.begin_tile_inspect(10, 20);
    assert_eq!(mac.city.microsim_at(index).unwrap().stat0, 255);
    assert_eq!(mac.pe_process_crt_seed(), 123);
}

#[test]
fn pe_opinion_poll_high_approval_requests_cheers_without_a_story() {
    let mut city = City::new_blank("/tmp/pe-high-approval.sc2", "Approval").unwrap();
    city.set_pe_dw_city_population(1_000);
    city.set_pe_attitude_demand_inputs(0, 0, 120, 0);
    city.recompute_pe_attitude_scores(0, 0, 0);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 100);
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 70);
    city.set_text_overlay(10, 20, crate::city::QUERY_SPECIFIC_MAYOR_STATS_OVERLAY);
    city.queue_pe_newspaper_story(0x15, 0);
    let mut state = test_state(city);
    for prior in [63, 80] {
        state.city.set_pe_mayor_approval(prior);
        state.pe_process_crt = Win95CrtRand::new(123);
        state.begin_tile_inspect(10, 20);
        assert_eq!(state.city.pe_mayor_approval(), 86);
        assert_eq!(state.city.pe_newspaper_story_id_at(0), Some(0x15));
        let sounds = state.take_pending_disaster_extra_sounds();
        assert_eq!(sounds.len(), usize::from(prior < 80));
        assert!(sounds.iter().all(|sound| *sound == PlacementSound::Cheers));
    }
}

#[test]
fn query_shows_museum_attendance_and_library_books() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let mut state = test_state(city);
    assert!(state.place_building_at(20, 20, 212) > 0);
    assert!(state.place_building_at(30, 30, 245) > 0);
    state
        .city
        .refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let museum = query_tile_status_line(&state.city, 20, 20, false);
    let library = query_tile_status_line(&state.city, 30, 30, false);
    assert!(
        museum.contains("Attendance 350") && museum.contains("Employees 90"),
        "{museum}"
    );
    assert!(
        library.contains("Books 200")
            && library.contains("Visitors 388")
            && library.contains("Grade A"),
        "{library}"
    );
}

#[test]
fn query_shows_park_acres_and_marina_boats() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 20, 20, 213, 3);
    stamp_complete_building_for_test(&mut city, 30, 30, 248, 3);
    let park_xtxt = city.provision_microsim_for_building(213).expect("park");
    let marina_xtxt = city.provision_microsim_for_building(248).expect("marina");
    city.set_text_overlay(20, 20, park_xtxt);
    city.set_text_overlay(30, 30, marina_xtxt);
    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    city.refresh_structure_microsim_yearly(7_000, 100, 100, 100, 100, 100);
    let park = query_tile_status_line(&city, 20, 20, false);
    let marina = query_tile_status_line(&city, 30, 30, false);
    assert!(
        park.contains("Attendance 1,166") && park.contains("Acres 9"),
        "{park}"
    );
    assert!(marina.contains("Boats 46"), "{marina}");
}

#[test]
fn simulation_month_advance_preserves_inspect_query_text() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.query_is_inspect = true;
    state.query_line = "Tile 10,10".to_owned();
    state.city.set_zone(10, 10, 1);

    state.handle_simulation_month_advanced();

    assert_eq!(state.query_line, "Tile 10,10");
    assert!(state.query_is_inspect);
}

#[test]
fn desalinization_palette_tool_places_through_apply_path() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_terrain(9, 10, 0x30);
    city.set_tile_bits(9, 10, crate::city::TileBits::new(0b0000_0101));
    let mut state = test_state(city);
    state.tool = EditTool::Desalinization;

    let placement = state
        .tool_placement_at(10, 10)
        .expect("desalinization placement");
    assert_eq!(placement.preview, ToolPreview::Building(250));
    assert_eq!(placement.valid_tiles.len(), 9);
    assert!(placement.invalid_tiles.is_empty());
    assert!(state.apply_tool_at_tile(10, 10));
    assert_eq!(state.city.city_funds(), 19_000);

    for (x, y) in building_footprint_from_origin(10, 10, 3).expect("building footprint") {
        assert_eq!(state.city.building_at(x, y), 250);
        assert_eq!(
            state.city.underground_at(x, y),
            UNDER_TILE_BUILDING_PIPE_TEE
        );
        assert!(
            state
                .city
                .tile_bits_at(x, y)
                .is_some_and(|bits| bits.piped()),
            "desalinization footprint tile ({x}, {y}) should have building plumbing"
        );
    }
}

#[test]
fn title_new_city_selections_set_difficulty_funds_year_and_national_population() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    apply_new_city_title_selections(&mut city, Some(10_000), Some(2050), Some(3));
    ensure_default_city_options(&mut city);
    initialize_original_new_city_national_misc(&mut city);
    super::super::session::initialize_pe_new_city_national_graph(&mut city);

    assert_eq!(city.city_funds(), 10_000);
    assert_eq!(city.pe_w_city_difficulty, 3);
    assert_eq!(city.saved_date().map(|date| date.year), Some(2050));
    assert_eq!(
        city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(150_000)
    );
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_VALUE_OFFSET), Some(45_000));
    assert_eq!(city.misc_u32_at(MISC_NATIONAL_TREND_OFFSET), Some(2));
    let history = city.chunk_u32_values("XGRP", 0, 16 * 52);
    assert_eq!(history.len(), 16 * 52);
    assert_eq!(history[13 * 52], 3);
    assert_eq!(history[14 * 52], 150_000);
    assert_eq!(history.iter().filter(|&&value| value != 0).count(), 2);
}

#[test]
fn win95_new_city_initializes_national_graph_samples_for_each_start_year() {
    for (year, population) in [
        (1900, 10_000),
        (1950, 25_000),
        (2000, 60_000),
        (2050, 150_000),
    ] {
        let mut city = City::new_blank("/tmp/pe-new-city-graph.sc2", "Graph").unwrap();
        apply_new_city_title_selections(&mut city, Some(20_000), Some(year), Some(1));
        initialize_original_new_city_national_misc(&mut city);
        super::super::session::initialize_pe_new_city_national_graph(&mut city);
        let values = city.chunk_u32_values("XGRP", 0, 16 * 52);
        assert_eq!((values[13 * 52], values[14 * 52]), (3, population));
        assert_eq!(values.iter().filter(|&&value| value != 0).count(), 2);
    }
}

#[test]
fn win95_new_city_retains_previous_budget_history() {
    let mut city = City::new_blank("/tmp/win95-history-before.sc2", "Before").unwrap();
    for row in 0..BUDGET_TOTAL_COUNT {
        let offset = budget_row_misc_offset_for_city(&city, row, 0).unwrap();
        let values: Vec<u32> = (0..BUDGET_SERVICE_MISC_STRIDE)
            .map(|field| 1000 + row as u32 * 100 + field as u32)
            .collect();
        city.set_budget_misc_u32_values(offset, &values);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state
        .new_city_with_setup(Some(20_000), Some(1900), Some(1), true)
        .unwrap();
    for row in 0..BUDGET_TOTAL_COUNT {
        let offset = budget_row_misc_offset_for_city(&state.city, row, 0).unwrap();
        let values = state
            .city
            .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
        assert_eq!(values[0], 0);
        assert_eq!(values[2..4], [0, 0]);
        for field in 5..BUDGET_SERVICE_MISC_STRIDE {
            assert_eq!(values[field], 1000 + row as u32 * 100 + field as u32);
        }
    }
}

#[test]
fn win95_founding_edit_map_exit_resets_current_budget_only() {
    for founding in [false, true] {
        let mut city = City::new_blank("/tmp/pe-edit-budget-exit.sc2", "Before").unwrap();
        let offset = budget_row_misc_offset_for_city(&city, 0, 0).unwrap();
        let values: Vec<u32> = (0..BUDGET_SERVICE_MISC_STRIDE as u32)
            .map(|n| 1000 + n)
            .collect();
        city.set_budget_misc_u32_values(offset, &values);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.mode = AppMode::EditNewMap;
        state.founding_newspaper_pending = founding;
        state.finish_edit_map_state();
        let actual = state
            .city
            .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE);
        if founding {
            assert_eq!(&actual[..5], &[0, 7, 0, 0, 7]);
            assert_eq!(&actual[5..], &values[5..]);
        } else {
            assert_eq!(actual, values);
        }
    }
}

#[test]
fn win95_repeat_edit_new_map_preserves_landscape_and_clears_structures() {
    let mut city = City::new_blank("/tmp/pe-edit-landscape.sc2", "Before").unwrap();
    for (x, building) in [5, 6, 12, 13, 192].into_iter().enumerate() {
        city.set_terrain(x, 10, 17);
        city.set_altitude(x, 10, 9, 11);
        city.set_building(x, 10, building);
        city.set_underground(x, 10, 42);
        city.set_tile_bits(x, 10, TileBits::new(0xff));
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    assert_eq!(state.mode, AppMode::EditNewMap);
    assert!(state.founding_newspaper_pending);
    for (x, building) in [0, 6, 12, 0, 0].into_iter().enumerate() {
        assert_eq!(state.city.terrain_at(x, 10), 17);
        assert_eq!(
            (
                state.city.altitude_at(x, 10),
                state.city.water_height_at(x, 10)
            ),
            (9, 11)
        );
        assert_eq!(state.city.building_at(x, 10), building);
        assert_eq!(state.city.underground_at(x, 10), 0);
        assert_eq!(state.city.tile_bits_at(x, 10).unwrap().raw_bits(), 5);
    }
}

#[test]
fn win95_repeat_edit_new_map_retains_live_water_and_extra_settings() {
    let mut city = City::new_blank("/tmp/pe-edit-settings.sc2", "Before").unwrap();
    city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 7);
    city.set_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, 0);
    city.set_misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET, 255);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    assert_eq!(state.city.misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET), Some(7));
    assert_eq!(state.city.misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET), Some(0));
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(255)
    );
    state.begin_win95_new_city_setup();
    state.commit_win95_new_city_setup();
    state.finish_edit_map_after_charter();
    assert_eq!(state.city.pe_dw_city_has_ocean(), 7);
    assert_eq!(state.city.pe_dw_city_has_river(), 0);
    assert_eq!(
        state.city.misc_u32_at(MISC_NEWSPAPER_EXTRA_OFFSET),
        Some(255)
    );
}

#[test]
fn win95_repeat_edit_new_map_resets_only_native_microsim_fields() {
    let mut city = City::new_blank("/tmp/pe-edit-xmic.sc2", "Before").unwrap();
    for index in [0, 1, 9, 10, 149] {
        city.set_microsim_tile_id(index, 200);
        city.set_microsim_stat0(index, 12);
        city.set_microsim_stat1(index, 345);
        city.set_microsim_stat2(index, 678);
        city.set_microsim_stat3(index, 901);
    }
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    for index in [0, 10, 149] {
        let row = state.city.microsim_at(index).unwrap();
        assert_eq!(row.tile_id, if index == 0 { 236 } else { 0 });
        assert_eq!(
            (row.stat0, row.stat1, row.stat2, row.stat3),
            (12, 345, 678, 901)
        );
    }
    for index in [1, 9] {
        let row = state.city.microsim_at(index).unwrap();
        assert_ne!(row.tile_id, 0);
        assert_eq!((row.stat0, row.stat1, row.stat2, row.stat3), (0, 0, 0, 0));
    }
}

#[test]
fn win95_repeat_edit_new_map_clears_actor_tags_and_retains_payload() {
    let mut city = City::new_blank("/tmp/pe-edit-xthg.sc2", "Before").unwrap();
    let mut rows = crate::win95_overlay::empty_overlay_table();
    for index in [0, 1, 39] {
        rows[index] = Win95OverlayRecord {
            kind: 4,
            lane: 7,
            state: 9,
            tile_x: 11,
            tile_y: 12,
            height: 13,
            sub_x: 14,
            sub_y: 15,
            target_x: 16,
            target_y: 17,
            saved_a: 18,
            saved_b: 19,
        };
    }
    city.set_traffic_overlays_loaded_for_test(rows.clone());
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    for index in [0, 1, 39] {
        rows[index].kind = 0;
        rows[index].lane = 0;
    }
    assert_eq!(state.city.traffic_overlays().unwrap(), rows.as_slice());
    assert!(
        state
            .city
            .traffic_overlays()
            .unwrap()
            .iter()
            .all(|row| row.kind == 0)
    );
}

#[test]
fn win95_editor_make_map_preserves_microsim_and_actor_records() {
    let mut state = test_state(City::new_blank("/tmp/pe-make-map-state.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    state.city.set_microsim_tile_id(149, 0x73);
    state.city.set_microsim_stat0(149, 0x12);
    state.city.set_microsim_stat1(149, 0x3456);
    state.city.set_microsim_stat2(149, 0x5678);
    state.city.set_microsim_stat3(149, 0x9abc);
    state.city.traffic_overlays_mut().unwrap()[39] = Win95OverlayRecord {
        kind: 5,
        lane: 7,
        state: 0x12,
        tile_x: 0x34,
        tile_y: 0x56,
        height: 0x78,
        sub_x: 0x9a,
        sub_y: 0xbc,
        target_x: 0xde,
        target_y: 0xf0,
        saved_a: 0x12,
        saved_b: 0x34,
    };
    let microsims: Vec<_> = (0..crate::city::MICROSIM_COUNT)
        .map(|i| state.city.microsim_at(i))
        .collect();
    let actors = state.city.traffic_overlays().unwrap().to_vec();
    state.generate_edit_map();
    assert_eq!(
        (0..crate::city::MICROSIM_COUNT)
            .map(|i| state.city.microsim_at(i))
            .collect::<Vec<_>>(),
        microsims
    );
    assert_eq!(state.city.traffic_overlays().unwrap(), actors.as_slice());
}

#[test]
fn win95_editor_make_map_preserves_simulation_maps() {
    let mut state = test_state(City::new_blank("/tmp/pe-make-map-planes.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    let values: Vec<Vec<u8>> = (0x41..=0x48).map(|value| vec![value; CITY_TILES]).collect();
    state.city.sync_traffic_from_slice(&values[0]);
    state.city.sync_pollution_density_from_slice(&values[1]);
    state.city.sync_land_value_from_slice(&values[2]);
    state.city.sync_crime_density_from_slice(&values[3]);
    state.city.sync_police_coverage_from_slice(&values[4]);
    state.city.sync_fire_coverage_from_slice(&values[5]);
    state.city.sync_population_density_from_slice(&values[6]);
    state.city.sync_rate_of_growth_from_slice(&values[7]);

    state.generate_edit_map();
    let city = &state.city;
    let planes = [
        city.traffic_density_slice(),
        city.pollution_density_slice(),
        city.land_value_slice(),
        city.crime_density_slice(),
        city.police_coverage_slice(),
        city.fire_coverage_slice(),
        city.population_density_slice(),
        city.rate_of_growth_slice(),
    ];
    for (plane, expected) in planes.into_iter().zip(&values) {
        assert_eq!(plane, Some(expected.as_slice()));
    }
}

#[test]
fn win95_editor_make_map_twice_continues_native_worker_state() {
    let mut state = test_state(City::new_blank("/tmp/pe-repeat-map.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    state.edit_map_sliders = ui::EditMapSliderValues {
        hills: 12,
        water: 5,
        trees: 15,
    };
    state.pe_process_crt = Win95CrtRand::new(0x1234_5678);
    state.pe_process_game_rand = Win95GameRand::new(1);
    state.city.set_misc_u32_at(EDIT_MAP_RIVER_MISC_OFFSET, 0);
    for (coast, expected_hash, expected_crt) in [
        (1, 0x15fd_5854_26b5_92ea, 605_971_404),
        (0, 0xca2f_0301_d76a_562e, 2_204_295_258),
    ] {
        state
            .city
            .set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, coast);
        state.generate_edit_map();
        let bits = (0..CITY_WIDTH).flat_map(|x| (0..CITY_HEIGHT).map(move |y| (x, y)));
        let hash = bits.fold(0xcbf2_9ce4_8422_2325_u64, |hash, (x, y)| {
            (hash ^ u64::from(state.city.tile_bits_at(x, y).unwrap().raw_bits()))
                .wrapping_mul(0x0100_0000_01b3)
        });
        assert_eq!(hash, expected_hash);
        assert_eq!(state.pe_process_crt.seed(), expected_crt);
        assert_eq!(state.pe_process_game_rand.state(), 519_488_550);
    }
}

#[test]
fn win95_editor_make_map_preserves_tile_text_and_labels() {
    let mut state = test_state(City::new_blank("/tmp/pe-make-map-text.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    state.city.set_label(50, "ProbeLabel");
    state.city.set_text_overlay(17, 13, 50);
    state.city.set_text_overlay(103, 119, 0xfa);
    let before: Vec<_> = (0..CITY_TILES)
        .map(|i| state.city.text_overlay_at(i / CITY_HEIGHT, i % CITY_HEIGHT))
        .collect();

    state.generate_edit_map();
    for (i, expected) in before.into_iter().enumerate() {
        assert_eq!(
            state.city.text_overlay_at(i / CITY_HEIGHT, i % CITY_HEIGHT),
            expected
        );
    }
    assert_eq!(state.city.label_at(50), Some("ProbeLabel"));
}

#[test]
fn win95_edit_map_done_opens_post_editor_charter_through_ui_action() {
    let mut state = test_state(City::new_blank("/tmp/pe-charter-done.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();

    assert!(state.handle_ui_action_state(UiAction::DoneEditMap));
    assert_eq!(state.mode, AppMode::EditNewMap);
    assert!(state.founding_newspaper_pending);
    assert!(state.city_info_dialog.is_none());
    let setup = state
        .win95_new_city_setup
        .as_ref()
        .expect("Done opens the post-editor charter");
    assert_eq!(setup.name, "New City");
    assert_eq!(setup.difficulty, 0);
    assert_eq!(setup.year, 0);

    state.commit_win95_new_city_setup();
    state.finish_edit_map_after_charter();
    assert_eq!(state.mode, AppMode::CityView);
    assert_eq!(state.city.name, "New City");
    assert_eq!(state.city.saved_date().unwrap().year, 1900);
    assert_eq!(state.city.city_funds(), 20_000);
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding))
    );
}

#[test]
fn win95_editor_charter_applies_choices_before_founding() {
    let mut state = test_state(City::new_blank("/tmp/pe-charter.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    state.city.set_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, 1);
    state.begin_win95_new_city_setup();
    assert_eq!(state.mode, AppMode::EditNewMap);
    assert!(state.founding_newspaper_pending);
    let setup = state.win95_new_city_setup.as_mut().unwrap();
    setup.name = "Lake City".into();
    setup.difficulty = 2;
    setup.year = 3;
    state.commit_win95_new_city_setup();
    assert_eq!(state.city.name, "Lake City");
    assert_eq!(state.city.city_funds(), 10_000);
    assert_eq!(state.city.saved_date().unwrap().year, 2050);
    assert_eq!(state.game_clock.year(), 2050);
    assert_eq!(state.city.pe_w_city_difficulty, 3);
    assert_eq!(
        state.city.misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET),
        Some(150_000)
    );
    assert_eq!(state.city.misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET), Some(1));
    assert_eq!(outstanding_bond_count_for_city(&state.city), 1);
    let bond_history = budget_row_misc_offset_for_city(&state.city, BUDGET_BOND_ROW, 3).unwrap();
    assert_eq!(
        state.city.budget_misc_u32_values(bond_history, 2),
        [1, 30_000]
    );
    state.finish_edit_map_after_charter();
    assert_eq!(state.mode, AppMode::CityView);
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding))
    );
    assert_eq!(outstanding_bond_count_for_city(&state.city), 1);
    let path = std::env::temp_dir().join(format!("citygame-charter-{}.sc2", std::process::id()));
    state.close_city_info_dialog();
    state.save_city(&path, (1280, 800), 1.0).unwrap();
    let saved = City::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(saved.name, "Lake City");
    assert_eq!(saved.saved_date().unwrap().year, 2050);
    assert_eq!(saved.city_funds(), 10_000);
    assert_eq!(outstanding_bond_count_for_city(&saved), 1);
}

#[test]
fn win95_editor_charter_pointer_selects_and_commits() {
    let mut state = test_state(City::new_blank("/tmp/pe-charter-pointer.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    state.begin_win95_new_city_setup();
    assert!(!state.win95_setup_pointer(Vec2::new(290.0, 430.0), (1000, 800)));
    assert!(!state.win95_setup_pointer(Vec2::new(530.0, 460.0), (1000, 800)));
    assert!(state.win95_setup_pointer(Vec2::new(470.0, 500.0), (1000, 800)));
    assert!(state.win95_new_city_setup.is_none());
    assert_eq!(state.city.pe_w_city_difficulty, 3);
    assert_eq!(state.city.saved_date().unwrap().year, 2050);
}

#[test]
fn win95_editor_done_initializes_neighbors_before_charter() {
    let mut state = test_state(City::new_blank("/tmp/pe-editor-neighbors.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.new_win95_edit_map_from_city().unwrap();
    state.pe_process_crt = Win95CrtRand::new(2);
    assert_eq!(state.city.misc_u32_at(0x06d8), Some(0));
    state.begin_win95_new_city_setup();
    for (i, expected) in [
        (10, 2098, 2098),
        (31, 3405, 1702),
        (14, 706, 235),
        (6, 867, 289),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            state.city.misc_u32_values(0x06d8 + i * 16, 4),
            vec![expected.0, expected.1, expected.2, 0]
        );
    }
    assert_eq!(state.pe_process_crt.seed(), 723_476_577);
    state.commit_win95_new_city_setup();
    state.finish_edit_map_after_charter();
    assert_eq!(
        state.city.misc_u32_values(0x06d8, 4),
        vec![10, 2098, 2098, 0]
    );
}

#[test]
#[ignore = "requires private original Win95 Hard/2050 charter capture"]
fn win95_editor_hard_2050_charter_matches_native_save() {
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_CHARTER_HARD").unwrap());
    let native = City::load(root.join("native-hard2050.sc2")).unwrap();
    let mut state =
        test_state(City::new_blank("/tmp/pe-native-hard-bootstrap.sc2", "Bootstrap").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    let labels_from_exe = std::env::var_os("CITYGAME_WIN95_EXE").map(|path| {
        crate::win95_resources::new_city_reserved_labels(&std::fs::read(path).unwrap()).unwrap()
    });
    state.pe_new_city_reserved_labels = labels_from_exe.clone();
    state
        .load_city(
            &root.join("../budget-second-city-20261114/native-prior.sc2"),
            (1280, 800),
            1.0,
        )
        .unwrap();
    state.new_win95_edit_map_from_city().unwrap();
    state.begin_win95_new_city_setup();
    let setup = state.win95_new_city_setup.as_mut().unwrap();
    setup.name = "lake city".into();
    setup.difficulty = 2;
    setup.year = 3;
    state.commit_win95_new_city_setup();
    state.finish_edit_map_after_charter();
    state.close_city_info_dialog();
    let temp_root =
        std::env::temp_dir().join(format!("citygame-native-hard-{}", std::process::id()));
    std::fs::create_dir_all(&temp_root).unwrap();
    let path = temp_root.join("hard2050.sc2");
    state.save_city_as(&path, (1280, 800), 1.0).unwrap();
    let saved = City::load(&path).unwrap();
    if let Some(output) = std::env::var_os("CITYGAME_WIN95_CHARTER_DIAGNOSTIC_SAVE") {
        std::fs::copy(&path, output).unwrap();
    }
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(temp_root).unwrap();
    assert_eq!(saved.name, native.name);
    assert_eq!(saved.saved_date(), native.saved_date());
    assert_eq!(saved.city_age_days(), native.city_age_days());
    assert_eq!(saved.city_funds(), native.city_funds());
    assert_eq!(saved.pe_w_city_difficulty, native.pe_w_city_difficulty);
    assert_eq!(
        outstanding_bond_count_for_city(&saved),
        outstanding_bond_count_for_city(&native)
    );
    for row in 0..crate::city::MICROSIM_COUNT {
        assert_eq!(
            saved.microsim_at(row),
            native.microsim_at(row),
            "XMIC row {row}"
        );
    }
    assert_eq!(
        saved.traffic_overlays(),
        native.traffic_overlays(),
        "XTHG records"
    );
    if labels_from_exe.is_some() {
        let decode = |city: &City| {
            let bytes = &city.chunk("XLAB").unwrap().data;
            if bytes.len() == 6400 {
                bytes.clone()
            } else {
                crate::city::sc2_rle_decode(bytes).unwrap()
            }
        };
        assert_eq!(
            decode(&saved),
            decode(&native),
            "XLAB resident backing and labels"
        );
    }
    for offset in [
        MISC_NATIONAL_POPULATION_OFFSET,
        MISC_NATIONAL_VALUE_OFFSET,
        MISC_NATIONAL_TAX_OFFSET,
        MISC_NATIONAL_TREND_OFFSET,
    ] {
        assert_eq!(
            saved.misc_u32_at(offset),
            native.misc_u32_at(offset),
            "MISC offset {offset:#x}"
        );
    }
    for offset in [
        EDIT_MAP_COAST_MISC_OFFSET,
        EDIT_MAP_RIVER_MISC_OFFSET,
        MISC_NEWSPAPER_EXTRA_OFFSET,
    ] {
        assert_eq!(
            saved.misc_u32_at(offset),
            native.misc_u32_at(offset),
            "preserved MISC offset {offset:#x}"
        );
    }
    for row in [13, 14] {
        assert_eq!(
            saved.chunk_u32_values("XGRP", row * 52, 52),
            native.chunk_u32_values("XGRP", row * 52, 52),
            "graph row {row}"
        );
    }
    for city in [&saved, &native] {
        let mut names = std::collections::BTreeSet::new();
        for row in 0..4 {
            let values = city.misc_u32_values(0x06d8 + row * 16, 4);
            assert!((1..=36).contains(&values[0]) && names.insert(values[0]));
            assert!((100..=7499).contains(&values[1]));
            assert!((1..=3).any(|divisor| values[2] == values[1] / divisor));
            assert_eq!(values[3], 0);
        }
    }
    for row in 0..BUDGET_TOTAL_COUNT {
        let offset = budget_row_misc_offset_for_city(&saved, row, 0).unwrap();
        assert_eq!(
            saved.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE),
            native.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE),
            "budget row {row}"
        );
    }
}

#[test]
#[ignore = "requires private original Win95 second-city capture"]
fn win95_new_city_budget_matches_live_previous_city_transition() {
    let root =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_BUDGET_SECOND_CITY").unwrap());
    let expected = City::load(root.join("native-third.sc2")).unwrap();
    let prior: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("prior-live.json")).unwrap()).unwrap();
    let after: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("third-naming.json")).unwrap()).unwrap();
    let mut state = test_state(
        City::new_blank("/tmp/pe-budget-transition-bootstrap.sc2", "Bootstrap").unwrap(),
    );
    state.engine_fidelity = EngineFidelity::Win95;
    let labels_from_exe = std::env::var_os("CITYGAME_WIN95_EXE").map(|path| {
        crate::win95_resources::new_city_reserved_labels(&std::fs::read(path).unwrap()).unwrap()
    });
    if labels_from_exe.is_some() {
        state.pe_registered_mayor_name = "SC Trace".into();
    }
    state.pe_new_city_reserved_labels = labels_from_exe.clone();
    state
        .load_city(&root.join("native-prior.sc2"), (1280, 800), 1.0)
        .unwrap();
    for row in 0..BUDGET_TOTAL_COUNT {
        let offset = budget_row_misc_offset_for_city(&state.city, row, 0).unwrap();
        let observed: Vec<u32> = serde_json::from_value(prior["rows"][row].clone()).unwrap();
        assert_eq!(
            state
                .city
                .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE),
            observed,
            "loaded row {row}"
        );
    }
    state
        .new_city_from_title(
            "New City".into(),
            20_000,
            1900,
            1,
            0,
            title_screen::NewCityStartFlow::GenerateAndPlay,
        )
        .unwrap();
    let path =
        std::env::temp_dir().join(format!("citygame-second-budget-{}.sc2", std::process::id()));
    state.close_city_info_dialog();
    state.save_city(&path, (1280, 800), 1.0).unwrap();
    let saved = City::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    for row in 0..crate::city::MICROSIM_COUNT {
        assert_eq!(
            saved.microsim_at(row),
            expected.microsim_at(row),
            "native second city XMIC row {row}"
        );
    }
    assert_eq!(
        saved.traffic_overlays(),
        expected.traffic_overlays(),
        "native second city XTHG records"
    );
    if labels_from_exe.is_some() {
        let decode = |city: &City| {
            let bytes = &city.chunk("XLAB").unwrap().data;
            if bytes.len() == 6400 {
                bytes.clone()
            } else {
                crate::city::sc2_rle_decode(bytes).unwrap()
            }
        };
        assert_eq!(decode(&saved), decode(&expected), "native second city XLAB");
    }
    for row in 0..BUDGET_TOTAL_COUNT {
        let offset = budget_row_misc_offset_for_city(&state.city, row, 0).unwrap();
        let native: Vec<u32> = serde_json::from_value(after["rows"][row].clone()).unwrap();
        assert_eq!(
            state
                .city
                .budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE),
            native,
            "live row {row}"
        );
        assert_eq!(
            saved.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE),
            native,
            "saved row {row}"
        );
        assert_eq!(
            expected.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE),
            native,
            "native save row {row}"
        );
    }
}

#[test]
#[ignore = "requires private original Win95 edit-map capture"]
fn win95_edit_map_budget_matches_live_previous_city_transition() {
    let root =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_BUDGET_EDIT_MAP").unwrap());
    let capture = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(root.join(format!("{name}-live.json"))).unwrap())
            .unwrap()
    };
    let prior = capture("prior");
    let editor = capture("editor");
    let naming = capture("naming");
    let founded = capture("founded");
    let mut state =
        test_state(City::new_blank("/tmp/pe-edit-budget-bootstrap.sc2", "Bootstrap").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state
        .load_city(
            &root.join("../budget-second-city-20261114/native-prior.sc2"),
            (1280, 800),
            1.0,
        )
        .unwrap();
    let compare = |city: &City, expected: &serde_json::Value, boundary: &str| {
        for row in 0..BUDGET_TOTAL_COUNT {
            let offset = budget_row_misc_offset_for_city(city, row, 0).unwrap();
            let native: Vec<u32> = serde_json::from_value(expected["rows"][row].clone()).unwrap();
            assert_eq!(
                city.budget_misc_u32_values(offset, BUDGET_SERVICE_MISC_STRIDE),
                native,
                "{boundary} row {row}"
            );
        }
    };
    compare(&state.city, &prior, "loaded");
    state
        .new_city_from_title(
            "New City".into(),
            20_000,
            1900,
            1,
            0,
            title_screen::NewCityStartFlow::GenerateAndEdit,
        )
        .unwrap();
    compare(&state.city, &editor, "editor");
    state.begin_win95_new_city_setup();
    compare(&state.city, &naming, "editor Done");
    state.commit_win95_new_city_setup();
    state.finish_edit_map_after_charter();
    compare(&state.city, &founded, "founding newspaper");
    state.close_city_info_dialog();
    let path =
        std::env::temp_dir().join(format!("citygame-edit-budget-{}.sc2", std::process::id()));
    state.save_city(&path, (1280, 800), 1.0).unwrap();
    let saved = City::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    compare(&saved, &founded, "save/reload");
    let native_saved = City::load(root.join("native-founded.sc2")).unwrap();
    compare(&native_saved, &founded, "native save/reload");
}

#[test]
#[ignore = "requires private original Win95 edit-map capture"]
fn win95_edit_map_native_terrain_transition_evidence() {
    let root =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_BUDGET_EDIT_MAP").unwrap());
    let prior = City::load(root.join("../budget-second-city-20261114/native-prior.sc2")).unwrap();
    let after = City::load(root.join("native-founded.sc2")).unwrap();
    let mut state =
        test_state(City::new_blank("/tmp/pe-edit-terrain-bootstrap.sc2", "Bootstrap").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state
        .load_city(
            &root.join("../budget-second-city-20261114/native-prior.sc2"),
            (1280, 800),
            1.0,
        )
        .unwrap();
    state.new_win95_edit_map_from_city().unwrap();
    let mut counts = [0; 5];
    let mut bit_mismatches = 0;
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            counts[0] += usize::from(prior.terrain_at(x, y) != after.terrain_at(x, y));
            counts[1] += usize::from(prior.altm_at(x, y) != after.altm_at(x, y));
            counts[2] += usize::from(prior.building_at(x, y) != after.building_at(x, y));
            counts[3] += usize::from(after.building_at(x, y) != 0);
            counts[4] += usize::from(after.underground_at(x, y) != 0);
            let bits = prior.tile_bits_at(x, y).unwrap().raw_bits() & 5;
            bit_mismatches += usize::from(after.tile_bits_at(x, y).unwrap().raw_bits() != bits);
            let building = prior.building_at(x, y);
            if building != 0 {
                assert_eq!(
                    after.building_at(x, y),
                    if (6..=12).contains(&building) {
                        building
                    } else {
                        0
                    },
                    "tile ({x}, {y})"
                );
            }
            assert_eq!(
                state.city.terrain_at(x, y),
                after.terrain_at(x, y),
                "XTER ({x}, {y})"
            );
            assert_eq!(
                state.city.altm_at(x, y),
                after.altm_at(x, y),
                "ALTM ({x}, {y})"
            );
            assert_eq!(
                state.city.building_at(x, y),
                after.building_at(x, y),
                "XBLD ({x}, {y})"
            );
            assert_eq!(
                state.city.underground_at(x, y),
                after.underground_at(x, y),
                "XUND ({x}, {y})"
            );
            assert_eq!(
                state.city.tile_bits_at(x, y),
                after.tile_bits_at(x, y),
                "XBIT ({x}, {y})"
            );
            assert_eq!(
                state.city.zone_at(x, y),
                after.zone_at(x, y),
                "XZON ({x}, {y})"
            );
        }
    }
    eprintln!(
        "native editor prior/founded differences XTER, ALTM, XBLD; remaining XBLD, XUND: {counts:?}"
    );
    eprintln!("XBIT differences from prior & 5: {bit_mismatches}");
    assert_eq!(counts, [0, 0, 602, 633, 0]);
    assert_eq!(bit_mismatches, 0);
    state.finish_edit_map_state();
    state.close_city_info_dialog();
    let path = std::env::temp_dir().join(format!(
        "citygame-edit-landscape-{}.sc2",
        std::process::id()
    ));
    state.save_city(&path, (1280, 800), 1.0).unwrap();
    let saved = City::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    for x in 0..CITY_WIDTH {
        for y in 0..CITY_HEIGHT {
            assert_eq!(
                saved.terrain_at(x, y),
                after.terrain_at(x, y),
                "saved XTER ({x}, {y})"
            );
            assert_eq!(
                saved.altm_at(x, y),
                after.altm_at(x, y),
                "saved ALTM ({x}, {y})"
            );
            assert_eq!(
                saved.building_at(x, y),
                after.building_at(x, y),
                "saved XBLD ({x}, {y})"
            );
            assert_eq!(
                saved.underground_at(x, y),
                after.underground_at(x, y),
                "saved XUND ({x}, {y})"
            );
            assert_eq!(
                saved.tile_bits_at(x, y),
                after.tile_bits_at(x, y),
                "saved XBIT ({x}, {y})"
            );
            assert_eq!(
                saved.zone_at(x, y),
                after.zone_at(x, y),
                "saved XZON ({x}, {y})"
            );
        }
    }
}

#[test]
fn win95_generated_new_city_clears_live_power_utilization() {
    let mut state = test_state(City::new_blank("/tmp/pe-new-city-power.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.city.set_pe_power_used_percent(77);
    state
        .new_city_with_setup(Some(20_000), Some(1900), Some(1), true)
        .unwrap();
    assert_eq!(state.city.pe_power_used_percent(), 0);
}

#[test]
fn win95_edit_map_new_city_retains_live_power_utilization() {
    let mut state = test_state(City::new_blank("/tmp/pe-edit-map-power.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.city.set_pe_power_used_percent(77);
    state
        .new_city_with_setup(Some(20_000), Some(1900), Some(1), false)
        .unwrap();
    assert_eq!(state.city.pe_power_used_percent(), 77);
}

#[test]
fn win95_new_city_preserves_document_sound_and_music() {
    for (sound, music) in [(0, 0), (0, 1), (1, 0), (1, 1), (7, 9)] {
        let mut city = City::new_blank("/tmp/pe-new-city-audio.sc2", "Before").unwrap();
        city.set_misc_u32_at(SOUND_EFFECTS_MISC_OFFSET, sound);
        city.set_misc_u32_at(MUSIC_MISC_OFFSET, music);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.sound_effects_enabled = sound != 0;
        state.music_enabled = music != 0;
        state
            .new_city_with_setup(Some(20_000), Some(1900), Some(1), true)
            .unwrap();
        assert_eq!(
            state.city.misc_u32_at(SOUND_EFFECTS_MISC_OFFSET),
            Some(sound)
        );
        assert_eq!(state.city.misc_u32_at(MUSIC_MISC_OFFSET), Some(music));
        assert_eq!(state.sound_effects_enabled, sound != 0);
        assert_eq!(state.music_enabled, music != 0);
        let snapshot = state.city_save_snapshot((1280, 800), 1.0);
        assert_eq!(snapshot.misc_u32_at(SOUND_EFFECTS_MISC_OFFSET), Some(sound));
        assert_eq!(snapshot.misc_u32_at(MUSIC_MISC_OFFSET), Some(music));
        let path =
            std::env::temp_dir().join(format!("citygame-new-audio-{}.sc2", std::process::id()));
        state.save_city(&path, (1280, 800), 1.0).unwrap();
        let saved = City::load(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(saved.misc_u32_at(SOUND_EFFECTS_MISC_OFFSET), Some(sound));
        assert_eq!(saved.misc_u32_at(MUSIC_MISC_OFFSET), Some(music));
    }
}

#[test]
fn win95_save_and_provider_snapshot_write_current_speed() {
    let path = std::env::temp_dir().join(format!(
        "citygame-win95-save-speed-{}.sc2",
        std::process::id()
    ));
    let mut state = test_state(City::new_blank(&path, "Speed").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    state.city.set_misc_u32_at(MISC_SPEED_OFFSET, 1);
    state.traffic.set_speed(GameSpeed::Llama);
    let snapshot = state.city_save_snapshot((1280, 800), 1.0);
    assert_eq!(snapshot.misc_u32_at(MISC_SPEED_OFFSET), Some(3));
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), Some(1));
    state.save_city(&path, (1280, 800), 1.0).unwrap();
    let saved = City::load(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(saved.misc_u32_at(MISC_SPEED_OFFSET), Some(3));
}

#[test]
#[ignore = "requires private Win95 speed serializer fixture"]
fn win95_save_speed_matches_original_serializer() {
    let input = std::env::var_os("CITYGAME_WIN95_SPEED_FIXTURE").unwrap();
    let cases: serde_json::Value = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "citygame-pe-speed-fixture-{}.sc2",
        std::process::id()
    ));
    let mut state = test_state(City::new_blank(&path, "Speed fixture").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    let mut checked = 0;
    for row in cases.as_array().unwrap() {
        let Some(speed) = game_speed_from_sc2k_misc(row["speed"].as_u64().unwrap() as u32) else {
            continue;
        };
        state.traffic.set_speed(speed);
        state.city.set_misc_u32_at(MISC_SPEED_OFFSET, 0);
        let expected = row["saved"].as_u64().unwrap() as u32;
        assert_eq!(
            state
                .city_save_snapshot((1280, 800), 1.0)
                .misc_u32_at(MISC_SPEED_OFFSET),
            Some(expected)
        );
        state.save_city(&path, (1280, 800), 1.0).unwrap();
        assert_eq!(
            City::load(&path).unwrap().misc_u32_at(MISC_SPEED_OFFSET),
            Some(expected)
        );
        checked += 1;
    }
    assert_eq!(checked, 5, "fixture must cover every supported game speed");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn title_new_city_difficulty_round_trips_in_both_city_layouts() {
    for (kind, path) in [
        (CityFileKind::WinSc2, "/tmp/title-difficulty-win.sc2"),
        (CityFileKind::MacCity, "/tmp/title-difficulty-mac.city"),
    ] {
        let mut city = City::new_blank(path, "Title Difficulty").expect("blank city");
        if kind == CityFileKind::MacCity {
            city.kind = kind;
            city.prepare_mac_fidelity_from_saved_city();
        }

        apply_new_city_title_selections(&mut city, Some(10_000), Some(2050), Some(3));
        assert_eq!(city.city_funds(), 10_000);
        assert_eq!(city.pe_w_city_difficulty, 3);
        assert_eq!(city.misc_u32_at(MISC_DIFFICULTY_OFFSET), Some(3));
        city.save(path).expect("save title city");

        let loaded = City::load(path).expect("reload title city");
        assert_eq!(loaded.kind, kind);
        assert_eq!(loaded.pe_w_city_difficulty, 3);
        assert_eq!(loaded.misc_u32_at(MISC_DIFFICULTY_OFFSET), Some(3));
    }
}

#[test]
fn title_new_city_start_year_filters_inventions_before_founding_year() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    apply_new_city_title_selections(&mut city, Some(10_000), Some(2000), Some(2));
    city.apply_default_new_city_technology_schedule();
    ensure_default_city_options(&mut city);
    initialize_original_new_city_national_misc(&mut city);

    let mut crt = Win95CrtRand::new(1);
    city.pe_reseed_neighbor_fame_ranks_42e585(&mut crt);

    assert_eq!(city.pe_w_year_founded, 2000);
    assert_eq!(city.misc_u32_at(MISC_YEAR_FOUNDED_OFFSET), Some(2000));
    assert_eq!(city.city_age_days(), Some(0));
    for technology in [
        Technology::GasPower,
        Technology::Airport,
        Technology::Highways,
        Technology::WaterTreatment,
        Technology::Desalinization,
    ] {
        assert_eq!(city.technology_discovery_year(technology), Some(0));
    }

    for technology in [
        Technology::MicrowavePower,
        Technology::FusionPower,
        Technology::ForestArcology,
        Technology::DarcoArcology,
        Technology::LaunchArcology,
        Technology::Highways2,
    ] {
        assert!(
            city.technology_discovery_year(technology)
                .unwrap_or_default()
                >= 2000,
            "{technology:?} should retain its future randomized invention year"
        );
    }
}

#[test]
fn mac_title_new_city_runs_reset_then_terrain_then_technology() {
    let city = City::new_blank("/tmp/sc2000-mac-title-tech.sc2", "Before").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    let game_rng = crate::mac_random::MacGameLcgRng::after_startup_playlist_choice();
    let mut expected_game_rng = game_rng;
    // Native CODE 3/CODE 7 reset replay at seed 123.
    let mut expected_rng = MacToolboxRng::new(1_106_567_514);
    let _ = terrain_generation::generate_with_rng(
        &mut expected_rng,
        true,
        true,
        ui::EditMapSliderValues::default(),
        &mut expected_game_rng,
    );
    let terrain_seed = expected_rng.seed();
    assert_eq!(terrain_seed, 308_520_939);
    // Native CODE 11 replay from the controlled post-terrain seed.
    let dates = [
        1959, 1958, 1990, 1981, 2022, 2051, 0, 0, 0, 0, 0, 1985, 1998, 2050, 2108, 2158, 2193,
    ];
    let setup_seed = 396_817_267;
    let neighbor_rows = [
        [6, 3331, 1665, 0],
        [27, 873, 436, 0],
        [20, 718, 718, 0],
        [19, 2495, 2495, 0],
    ];
    let read_neighbors = |city: &City| {
        std::array::from_fn::<_, 4, _>(|i| {
            std::array::from_fn::<_, 4, _>(|j| {
                city.mac_state_misc_u32_at(0x06d8 + i * 16 + j * 4)
                    .unwrap_or(0)
            })
        })
    };

    for (flow, coast) in [
        (title_screen::NewCityStartFlow::GenerateAndPlay, true),
        (title_screen::NewCityStartFlow::GenerateAndEdit, true),
        (title_screen::NewCityStartFlow::GenerateAndEdit, false),
    ] {
        state.mac_process_rng = MacToolboxRng::new(123);
        state.mac_game_rng = game_rng;
        state.game_clock = GameClock::new(2078, 4);
        state
            .new_city_from_title("Mac Engine City".into(), 20_000, 1950, 1, 12345, flow)
            .unwrap();
        assert_eq!(state.city.kind, CityFileKind::WinSc2);
        assert_eq!(state.game_clock, GameClock::new(1950, 1));
        if flow == title_screen::NewCityStartFlow::GenerateAndEdit {
            assert_eq!(state.mac_process_rng.seed(), terrain_seed);
            assert_eq!(read_neighbors(&state.city), neighbor_rows);
            if !coast {
                state.toggle_edit_map_option(EDIT_MAP_COAST_MISC_OFFSET);
            }
            state.finish_edit_map_state();
        }
        let mut expected_neighbors = neighbor_rows;
        if coast {
            expected_neighbors[0] = [0; 4];
        }
        assert_eq!(read_neighbors(&state.city), expected_neighbors);
        assert_eq!(state.mac_process_rng.seed(), setup_seed);
        assert_eq!(state.mac_game_rng.state(), expected_game_rng.state());
        state.finish_edit_map_state();
        assert_eq!(state.mac_process_rng.seed(), setup_seed, "setup runs once");

        let file = std::env::temp_dir().join(format!(
            "citygame-mac-engine-title-tech-cross-open-{}.sc2",
            std::process::id()
        ));
        state.city.save(&file).unwrap();
        let mut cross_open = City::load(&file).unwrap();
        std::fs::remove_file(file).unwrap();
        cross_open.prepare_mac_fidelity_from_saved_city();
        assert_eq!(
            cross_open.mac_state_misc_u32_at(MISC_DIFFICULTY_OFFSET),
            Some(1)
        );
        assert_eq!(read_neighbors(&cross_open), expected_neighbors);
        for (i, date) in dates.into_iter().enumerate() {
            assert_eq!(cross_open.mac_state_misc_u32_at(0x0738 + i * 4), Some(date));
        }
    }
}

#[test]
fn mac_new_city_draft_reopens_in_editor_and_defers_technology() {
    for (extension, coast, expected_name) in [("city", 1, 0), ("sc2", 0x100, 6)] {
        let file = std::env::temp_dir().join(format!(
            "citygame-mac-editor-draft-{}.{}",
            std::process::id(),
            extension
        ));
        let mut draft = blank_city_layout_fixture(&file, "Draft");
        draft.set_saved_date(CityDate {
            year: 1950,
            month: 1,
        });
        draft.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_EDIT_MAP);
        draft.set_mac_state_misc_u32_at(EDIT_MAP_COAST_MISC_OFFSET, coast);
        draft.set_mac_state_misc_u32_at(0x06d8, 6);
        draft.save(&file).unwrap();

        let mut state = test_state(draft);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mac_process_rng = MacToolboxRng::new(123);
        state.load_city(&file, (1280, 800), 1.0).unwrap();
        std::fs::remove_file(&file).unwrap();
        assert_eq!(state.mode, AppMode::EditNewMap);
        assert_eq!(state.mac_process_rng.seed(), 1_106_567_514);
        assert_eq!(state.city.mac_state_misc_u32_at(0x06d8), Some(6));
        assert_eq!(state.city.mac_state_misc_u32_at(0x073c), Some(0));

        state.finish_edit_map_state();
        assert_eq!(state.mode, AppMode::CityView);
        assert_eq!(state.mac_process_rng.seed(), 989_958_143);
        assert_eq!(
            state.city.mac_state_misc_u32_at(0x06d8),
            Some(expected_name)
        );
        assert_eq!(state.city.mac_state_misc_u32_at(0x073c), Some(1955));
    }
}

#[test]
fn query_tile_power_status_uses_process_crt_seed_not_city_name() {
    // Live Query/inspect Power/Water yes-no shares FUN_004679B0 process CRT.
    let mut city = City::new_blank("/tmp/sc2000-query-crt.sc2", "QueryCrt").expect("blank");
    city.set_building(10, 10, 200);
    pe_seed_power_plant_altitude_for_test(&mut city, 10, 10, 18);
    let mut state = test_state(city);
    state.pe_process_crt = Win95CrtRand::new(0xa1a1_1a1a);
    let process = state.pe_process_crt_seed();
    assert_ne!(process, pe_power_recalc_seed(&state.city));
    state.begin_tile_inspect(10, 10);
    let expected = query_tile_status_line_with_passengers(
        &state.city,
        10,
        10,
        false,
        TransitPassengerStats::default(),
        process,
    );
    assert_eq!(state.query_line, expected);
    let city_name_line = query_tile_status_line_with_passengers(
        &state.city,
        10,
        10,
        false,
        TransitPassengerStats::default(),
        pe_power_recalc_seed(&state.city),
    );
    assert_eq!(
        query_tile_status_line(&state.city, 10, 10, false),
        city_name_line,
        "offline query helper keeps city-name seed"
    );
    let (general, _) = query_dialog_row_groups(
        &state.city,
        10,
        10,
        false,
        TransitPassengerStats::default(),
        process,
    );
    let power = general
        .iter()
        .find(|(k, _)| k == "Power")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        power,
        Some(query_service_yes_no(
            tile_power_status_text_with_entry_seed(&state.city, 10, 10, process,)
        )),
    );
}

#[test]
fn palette_buildings_stamp_recovered_underground_side_effects() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(30, 30, 220), 1);
    assert_eq!(state.city.building_at(30, 30), 220);
    assert_eq!(state.city.underground_at(30, 30), UNDER_TILE_PIPES_LR);

    assert_eq!(state.place_building_at(40, 40, 233), 1);
    assert_eq!(state.city.building_at(40, 40), 233);
    assert_eq!(state.city.underground_at(40, 40), UNDER_TILE_GREY_BOX_2);

    assert_eq!(state.place_building_at(50, 50, 108), 1);
    assert_eq!(state.city.building_at(50, 50), 108);
    assert_eq!(state.city.underground_at(50, 50), UNDER_TILE_GREY_BOX_2);

    assert_eq!(state.place_building_at(60, 60, 249), 9);
    for y in 60..=62 {
        for x in 60..=62 {
            assert_eq!(state.city.underground_at(x, y), UNDER_TILE_GREY_BOX_1);
        }
    }

    state.city.set_terrain(69, 70, 0x30);
    state
        .city
        .set_tile_bits(69, 70, crate::city::TileBits::new(0b0000_0001));
    assert_eq!(state.place_building_at(70, 70, 250), 9);
    for y in 70..=72 {
        for x in 70..=72 {
            assert_eq!(
                state.city.underground_at(x, y),
                UNDER_TILE_BUILDING_PIPE_TEE
            );
        }
    }
}

#[test]
fn palette_building_placement_stamps_footprint_and_corner_bits() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(10, 10, 7);
    city.set_zone(10, 10, 1);
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(10, 10, 210), 9);

    for y in 10..=12 {
        for x in 10..=12 {
            assert_eq!(state.city.building_at(x, y), 210, "building at ({x},{y})");
        }
    }
    let [top_left, top_right, bottom_left, bottom_right] = state.city.footprint_corner_bits();
    assert_eq!(state.city.zone_at(10, 10).unwrap().raw_bits(), top_left);
    assert_eq!(state.city.zone_at(12, 10).unwrap().raw_bits(), top_right);
    assert_eq!(state.city.zone_at(10, 12).unwrap().raw_bits(), bottom_left);
    assert_eq!(state.city.zone_at(12, 12).unwrap().raw_bits(), bottom_right);
    assert_eq!(state.city.zone_at(11, 11).unwrap().raw_bits(), 0);

    let drawn_tiles = (10..=12)
        .flat_map(|x| (10..=12).map(move |y| (x, y)))
        .filter(|&(x, y)| state.city.building_should_draw_at(x, y))
        .count();
    assert_eq!(drawn_tiles, 1);
}

#[test]
fn palette_building_placement_rejects_blocked_or_water_footprints() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_building(11, 11, 112);
    city.set_terrain(20, 20, 0x30);
    let mut state = test_state(city);

    let blocked = state.building_placement_at(10, 10, 210);
    assert_eq!(blocked.preview, ToolPreview::Building(210));
    assert!(blocked.invalid_tiles.contains(&(11, 11)));
    assert_eq!(state.place_building_at(10, 10, 210), 0);
    assert_eq!(state.city.building_at(10, 10), 0);
    assert_eq!(state.city.building_at(11, 11), 112);

    let water = state.building_placement_at(20, 20, 210);
    assert!(water.invalid_tiles.contains(&(20, 20)));
    assert_eq!(state.place_building_at(20, 20, 210), 0);
    assert_eq!(state.city.building_at(20, 20), 0);
}

#[test]
fn palette_building_placement_rejects_sloped_or_uneven_footprints() {
    let mut slope_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    slope_city.set_terrain(11, 11, 0x01);
    let mut slope_state = test_state(slope_city);

    let sloped = slope_state.building_placement_at(10, 10, 210);
    assert!(sloped.invalid_tiles.contains(&(11, 11)));
    assert_eq!(slope_state.place_building_at(10, 10, 210), 0);
    assert_eq!(slope_state.city.building_at(10, 10), 0);

    let mut uneven_city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    for (x, y) in building_footprint_from_origin(20, 20, 3).expect("building footprint") {
        uneven_city.set_altitude(x, y, 5, 0);
    }
    uneven_city.set_altitude(21, 21, 6, 0);
    let mut uneven_state = test_state(uneven_city);

    let uneven = uneven_state.building_placement_at(20, 20, 210);
    assert!(uneven.invalid_tiles.contains(&(21, 21)));
    assert_eq!(uneven_state.place_building_at(20, 20, 210), 0);
    assert_eq!(uneven_state.city.building_at(20, 20), 0);
}

#[test]
fn palette_building_placement_rebuilds_generated_pipe_service_bits() {
    // PE zone tool does not invent pipe refresh; set service bits explicitly.
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);

    state.city.set_building(10, 10, 220);
    power_water_source_for_test(&mut state.city, 10, 10);
    state.city.set_underground(
        10,
        10,
        underground_tile_for_mask(UndergroundKind::Pipes, NETWORK_LEFT | NETWORK_RIGHT),
    );
    state.place_zone_at(11, 10, 1);
    state
        .city
        .set_tile_bits(11, 10, TileBits::new(0).with_piped(true).with_watered(true));
    seed_saved_water_worker_bits_for_test(&mut state.city);
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());

    assert_eq!(state.place_building_at(11, 10, 112), 1);
    assert!(state.city.tile_bits_at(11, 10).unwrap().piped());
    assert!(state.city.tile_bits_at(11, 10).unwrap().watered());

    assert_eq!(state.place_building_at(12, 10, 6), 1);
    assert!(!state.city.tile_bits_at(12, 10).unwrap().piped());
    assert!(!state.city.tile_bits_at(12, 10).unwrap().watered());
}

#[test]
fn city_view_terrain_palette_places_trees_and_water_on_clear_tiles() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(30, 30, 112);
    let mut state = test_state(city);

    assert_eq!(state.place_building_at(20, 20, 6), 1);
    assert_eq!(state.city.building_at(20, 20), 6);

    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.terrain_at(20, 20), 0x3d);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert_eq!(state.city.zone_at(20, 20).unwrap().raw_bits(), 0);

    assert_eq!(state.place_surface_water_at(30, 30), 0);
    assert_eq!(state.city.terrain_at(30, 30), 0);
    assert_eq!(state.city.building_at(30, 30), 112);

    state.tool = EditTool::Water;
    assert!(!state.apply_tool_at_tile(30, 30));
}

#[test]
fn map_tool_place_water_existing_0x30_without_bit_rewrites_via_helper() {
    // PE MapTool accepts bare XTER 0x30; helper mask0 rewrites to 0x3D + PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-water-xter30.sc2", "WaterXter30").expect("blank");
    city.set_city_funds(20_000);
    city.set_terrain(20, 20, 0x30);
    city.set_building(20, 20, 2);
    let mut state = test_state(city);
    assert_eq!(state.place_surface_water_at(20, 20), 1);
    assert_eq!(state.city.terrain_at(20, 20), 0x3d);
    assert_eq!(state.city.building_at(20, 20), 0);
    assert!(state.city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_preserves_xtxt() {
    // PE PlaceWater `0x4469ed` → PlaceTile `0x40178f`/`0x441f00` clear XBLD; leave XTXT/XTRF.
    let mut city = City::new_blank("/tmp/sc2000-water-xtxt.sc2", "WaterXtxt").expect("blank");
    city.set_building(20, 20, 5);
    city.set_text_overlay(20, 20, 0x20);
    city.set_traffic_density(20, 20, 0x4a);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 0);
    assert_eq!(city.text_overlay_at(20, 20), Some(0x20));
    assert_eq!(city.traffic_density_at(20, 20), Some(0x4a));
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_skips_when_already_water_tile() {
    // PE @ `0x4469df`: after XTER write, water-tile skips PlaceTile / ALTM / recurse.
    let mut city = City::new_blank("/tmp/sc2000-water-skip.sc2", "WaterSkip").expect("blank");
    city.set_building(20, 20, 5);
    city.set_terrain(20, 20, 0);
    city.set_altitude(20, 20, 7, 0);
    city.set_tile_bits(20, 20, TileBits::new(0).with_water_tile(true));
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 5, "no PlaceTile clear");
    assert_eq!(city.terrain_at(20, 20), 0x3d, "XTER still rewritten");
    assert_eq!(city.altitude_at(20, 20), 7);
    assert_eq!(city.water_height_at(20, 20), 0);
}

#[test]
fn pe_place_water_helper_reslopes_west_neighbor_xter() {
    // PE @ `0x446a66`: after place, recurse W neighbor with water-tile → E-mask 0x42.
    let mut city = City::new_blank("/tmp/sc2000-water-reslope.sc2", "WaterReslope").expect("blank");
    city.set_terrain(19, 20, 0x3d);
    city.set_tile_bits(19, 20, TileBits::new(0).with_water_tile(true));
    city.set_building(19, 20, 3);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x44, "new tile W-mask");
    assert_eq!(city.terrain_at(19, 20), 0x42, "neighbor re-slope E-mask");
    assert_eq!(city.building_at(19, 20), 3, "neighbor skip PlaceTile");
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_skips_waterfall_xter_2e_3e() {
    // PE @ `0x44674b`/`0x446755`: XTER 0x2E/0x3E → exit (no PlaceTile).
    for xter in [0x2e_u8, 0x3e] {
        let mut city = City::new_blank("/tmp/sc2000-water-fall.sc2", "WaterFall").expect("blank");
        city.set_building(20, 20, 3);
        city.set_terrain(20, 20, xter);
        pe_place_water_helper_446650(&mut city, 20, 20);
        assert_eq!(city.building_at(20, 20), 3, "xter {xter:#x}");
        assert_eq!(city.terrain_at(20, 20), xter);
        assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
    }
}

#[test]
fn pe_place_water_helper_slope_land_writes_waterfall_xter_3e() {
    // PE @ `0x44675f`/`0x44676a`: existing XTER `1..=0x0F` → PlaceWater XTER `0x3E`.
    let mut city = City::new_blank("/tmp/sc2000-water-slope.sc2", "WaterSlope").expect("blank");
    city.set_terrain(20, 20, 0x05);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x3e);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_keeps_shallow_water_xter_10_1f() {
    // PE @ `0x446970`/`0x4469a3`: existing `0x10..=0x1F` skips XTER store; still PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-water-shallow.sc2", "WaterShallow").expect("blank");
    city.set_terrain(20, 20, 0x14);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x14, "keep shallow-water XTER");
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mid_band_0x20_writes_xter_minus_10() {
    // PE @ `0x4469a9`: XTER 0x20 ^ nibble0x0d → low nibble ≠0 → write `0x10`.
    let mut city = City::new_blank("/tmp/sc2000-water-20.sc2", "Water20").expect("blank");
    city.set_terrain(20, 20, 0x20);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x10);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mid_band_0x2d_exits_with_mask0() {
    // PE @ `0x4469a9`: XTER 0x2D ^ nibble0x0d → low nibble 0 → full exit.
    let mut city = City::new_blank("/tmp/sc2000-water-2d.sc2", "Water2d").expect("blank");
    city.set_terrain(20, 20, 0x2d);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 2);
    assert_eq!(city.terrain_at(20, 20), 0x2d);
    assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mid_band_0x21_writes_xter_minus_10() {
    // PE @ `0x4469b5`: XTER 0x21 + mask0 → write `0x11`, then PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-water-21.sc2", "Water21").expect("blank");
    city.set_terrain(20, 20, 0x21);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x11);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_existing_0x30_rewrites_to_0x3d() {
    // PE @ `0x446981`: XTER 0x30 + nibble0x0d → not -0x30 → write 0x3D.
    let mut city = City::new_blank("/tmp/sc2000-water-30.sc2", "Water30").expect("blank");
    city.set_terrain(20, 20, 0x30);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x3d);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_existing_0x3d_exits_with_mask0() {
    // PE @ `0x44698f`: XTER 0x3D + nibble0x0d → cx-ax == -0x30 → full exit.
    let mut city = City::new_blank("/tmp/sc2000-water-3d.sc2", "Water3d").expect("blank");
    city.set_terrain(20, 20, 0x3d);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.building_at(20, 20), 2);
    assert_eq!(city.terrain_at(20, 20), 0x3d);
    assert!(!city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_existing_0x31_rewrites_to_0x3d() {
    // PE @ `0x446998`: XTER >=0x30, mask0 nibble → write 0x3D + PlaceTile.
    let mut city = City::new_blank("/tmp/sc2000-water-31.sc2", "Water31").expect("blank");
    city.set_terrain(20, 20, 0x31);
    city.set_building(20, 20, 2);
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x3d);
    assert_eq!(city.building_at(20, 20), 0);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_flat_mask0_writes_xter_3d() {
    // PE table[0]=0x0D + 0x30 → isolated flat PlaceWater XTER 0x3D.
    let mut city = City::new_blank("/tmp/sc2000-water-flat.sc2", "WaterFlat").expect("blank");
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x3d);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_neighbor_mask_west_writes_xter_44() {
    // PE @ `0x446773`/`0x446858`: W water-tile → mask 0x08 → nibble 0x14 → XTER 0x44.
    let mut city = City::new_blank("/tmp/sc2000-water-mask-w.sc2", "WaterMaskW").expect("blank");
    city.set_tile_bits(19, 20, TileBits::new(0).with_water_tile(true));
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x44);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_neighbor_mask_east_writes_xter_42() {
    // PE mask E=0x02 → table nibble 0x12 → XTER 0x42.
    let mut city = City::new_blank("/tmp/sc2000-water-mask-e.sc2", "WaterMaskE").expect("blank");
    city.set_tile_bits(21, 20, TileBits::new(0).with_water_tile(true));
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x42);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mask_f_missing_nw_writes_xter_39() {
    // PE @ `0x446862`: NESW all water, NW diag missing → secondary 1 → nibble 9 → 0x39.
    let mut city = City::new_blank("/tmp/sc2000-water-maskf.sc2", "WaterMaskF").expect("blank");
    for (x, y) in [
        (20, 19),
        (21, 20),
        (20, 21),
        (19, 20),
        (21, 19),
        (21, 21),
        (19, 21),
    ] {
        city.set_tile_bits(x, y, TileBits::new(0).with_water_tile(true));
    }
    // NW (19,19) left dry.
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x39);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn pe_place_water_helper_mask_f_all_diags_writes_xter_30() {
    // PE secondary 0 (all diagonals water) → nibble 0 → XTER 0x30.
    let mut city = City::new_blank("/tmp/sc2000-water-maskf0.sc2", "WaterMaskF0").expect("blank");
    for (x, y) in [
        (20, 19),
        (21, 20),
        (20, 21),
        (19, 20),
        (19, 19),
        (21, 19),
        (21, 21),
        (19, 21),
    ] {
        city.set_tile_bits(x, y, TileBits::new(0).with_water_tile(true));
    }
    pe_place_water_helper_446650(&mut city, 20, 20);
    assert_eq!(city.terrain_at(20, 20), 0x30);
    assert!(city.tile_bits_at(20, 20).unwrap().water_tile());
}

#[test]
fn edit_map_palette_tools_apply_through_selected_tool_path() {
    for tool in [
        EditTool::RaiseTerrain,
        EditTool::LowerTerrain,
        EditTool::StretchTerrain,
        EditTool::LevelTerrain,
        EditTool::Water,
        EditTool::Stream,
        EditTool::Trees,
        EditTool::Forest,
    ] {
        let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
        city.set_mac_selected_water_height(8);
        // PE EditNewMap Raise uses the same billed submode-0 worker; the blank
        // fixture otherwise starts with zero funds and correctly refuses land++.
        city.set_city_funds(20_000);
        city.set_altitude(20, 20, 5, 0);
        city.set_altitude(19, 20, 7, 0);
        city.set_altitude(21, 20, 7, 0);
        city.set_altitude(20, 19, 7, 0);
        city.set_altitude(20, 21, 7, 0);
        if tool == EditTool::RaiseTerrain {
            // PE Raise submode-0 refuses vacant dry; stamp XBLD so land++ runs.
            city.set_building(20, 20, 4);
        }
        let mut state = test_state(city);
        state.mode = AppMode::EditNewMap;
        state.tool = tool;
        if tool == EditTool::LowerTerrain {
            // PE Lower CRT jitter: seed 17 samples the clicked tile.
            state.pe_process_crt = Win95CrtRand::new(17);
        }

        let placement = state.tool_placement_at(20, 20).expect("edit placement");
        assert_eq!(placement.preview, ToolPreview::Terrain, "{tool:?}");
        assert_eq!(placement.valid_tiles, vec![(20, 20)], "{tool:?}");
        assert!(placement.invalid_tiles.is_empty(), "{tool:?}");
        match tool {
            EditTool::StretchTerrain => {
                assert!(!state.apply_tool_at_tile(20, 20));
                assert!(state.begin_edit_map_stretch_drag_at(20, 20, 100.0));
                let two_levels = 24.0 * render_zoom(state.zoom, state.viewport_pixel_scale);
                assert!(state.update_edit_map_stretch_drag_at(100.0 - two_levels));
                assert_eq!(state.finish_edit_map_stretch_drag(), 1);
            }
            EditTool::LevelTerrain => {
                assert!(state.begin_edit_map_level_drag_at(20, 20));
                assert_eq!(state.finish_edit_map_level_drag(), 1);
            }
            _ => assert!(state.apply_tool_at_tile(20, 20), "{tool:?}"),
        }

        match tool {
            EditTool::RaiseTerrain => {
                // The successful billed land++ is followed by NESW smoothing
                // and the 3x3 slope reclassifier; this 5-vs-7 fixture ends at 7.
                assert_eq!(state.city.altitude_at(20, 20), 7)
            }
            EditTool::LowerTerrain => {
                // The full queued relaxation and expanded UnionRect refresh also lower
                // this clicked cell's 7-level cardinal neighbors; it settles at 6.
                assert_eq!(state.city.altitude_at(20, 20), 6)
            }
            EditTool::StretchTerrain | EditTool::LevelTerrain => {
                // Native editor contour repair restores the flat background here.
                for (x, y) in [(20, 20), (19, 20), (21, 20), (20, 19), (20, 21)] {
                    assert_eq!(state.city.altitude_at(x, y), BLANK_MAP_BASE_HEIGHT);
                }
                assert_eq!(state.city.city_funds(), 20_000);
            }
            EditTool::Water | EditTool::Stream => {
                assert_eq!(state.city.terrain_at(20, 20), 0x30);
                assert!(state.city.altm_at(20, 20).water > state.city.altitude_at(20, 20));
            }
            EditTool::Trees => assert_eq!(state.city.terrain_at(20, 20), 0x0b),
            EditTool::Forest => {
                let mut tree_tiles = 0;
                for y in 17..=23 {
                    for x in 17..=23 {
                        tree_tiles +=
                            usize::from((0x06..=0x0c).contains(&state.city.terrain_at(x, y)));
                    }
                }
                assert!(tree_tiles > 20, "expected forest brush to place a patch");
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn edit_map_sea_level_buttons_raise_and_lower_global_waterline() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_altitude(10, 10, BLANK_MAP_BASE_HEIGHT.saturating_sub(1), 0);
    city.set_altitude(11, 10, BLANK_MAP_BASE_HEIGHT, 0);
    // The native control reads the saved WORD, independently of visible water.
    city.set_mac_selected_water_height(8);
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;

    assert!(state.adjust_edit_map_sea_level(1) > 0);
    assert_eq!(
        state.query_line,
        format!("Sea Level {}", BLANK_MAP_BASE_HEIGHT + 1)
    );
    assert!(is_water_terrain(state.city.terrain_at(10, 10)));
    assert!(is_water_terrain(state.city.terrain_at(11, 10)));

    assert!(state.adjust_edit_map_sea_level(-1) > 0);
    assert_eq!(
        state.query_line,
        format!("Sea Level {BLANK_MAP_BASE_HEIGHT}")
    );
    // Native lowering leaves this one-level shoreline as XTER 0x20;
    // the coarse is_water_terrain helper does not include that class.
    assert_eq!(state.city.terrain_at(10, 10), 0x20);
    assert!(state.city.tile_bits_at(10, 10).unwrap().water_tile());
    assert_eq!(state.city.terrain_at(11, 10), 0);
}

#[test]
fn title_generate_and_play_generates_terrain_and_enters_city_view() {
    let city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let mut state = test_state(city);
    state.mode = AppMode::EditNewMap;
    state.show_grid = true;

    state.apply_new_city_start_flow(
        title_screen::NewCityStartFlow::GenerateAndPlay,
        MacToolboxRng::DEFAULT_SEED,
    );

    assert_eq!(state.mode, AppMode::CityView);
    assert_eq!(state.tool, EditTool::Query);
    assert!(!state.show_grid);
    assert!(state.show_traffic);
    assert_eq!(state.edit_map_generation_seed, MacToolboxRng::DEFAULT_SEED);
    assert!(
        (0..CITY_HEIGHT).any(|y| {
            (0..CITY_WIDTH).any(|x| state.city.altitude_at(x, y) != BLANK_MAP_BASE_HEIGHT)
        }),
        "generated terrain should not remain flat"
    );
    assert_eq!(
        state.city.rate_of_growth_at(0, 0),
        Some(RATE_OF_GROWTH_NEUTRAL),
        "terrain replacement must preserve the PE fresh-city XROG seed"
    );
    assert_eq!(state.city.population_density_at(0, 0), Some(0));
    assert_eq!(state.city.police_coverage_at(0, 0), Some(0));
    assert_eq!(state.city.fire_coverage_at(0, 0), Some(0));
}

#[test]
fn mac_title_generated_map_preserves_neutral_xrog_seed() {
    let mut city =
        City::new_blank("/tmp/sc2000-mac-new-city-xrog.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;

    state.apply_new_city_start_flow(
        title_screen::NewCityStartFlow::GenerateAndPlay,
        MacToolboxRng::DEFAULT_SEED,
    );

    assert_eq!(state.city.kind, CityFileKind::MacCity);
    assert_eq!(
        state.city.rate_of_growth_at(0, 0),
        Some(RATE_OF_GROWTH_NEUTRAL)
    );
    assert_eq!(state.city.population_density_at(0, 0), Some(0));
    assert_eq!(state.city.police_coverage_at(0, 0), Some(0));
    assert_eq!(state.city.fire_coverage_at(0, 0), Some(0));
    assert_eq!(
        state
            .city
            .rate_of_growth_at(CITY_WIDTH - 1, CITY_HEIGHT - 1),
        Some(RATE_OF_GROWTH_NEUTRAL)
    );

    let mut first_population = vec![0; CITY_TILES];
    first_population[density_map_index(0, 0)] = 4;
    let first_growth = live_rate_of_growth_map_for_city(&state.city, &first_population);
    assert_eq!(
        first_growth[density_map_index(0, 0)],
        pe_rate_of_growth_step(RATE_OF_GROWTH_NEUTRAL, 0, 4),
        "the first growth update must use the native zero-XPOP prior"
    );

    let path = std::env::temp_dir().join(format!(
        "sc2000-mac-new-city-microsims-{}.city",
        std::process::id()
    ));
    state.city.save(&path).expect("save fresh Mac city");
    let loaded = City::load(&path).expect("reload fresh Mac city");
    assert_eq!(loaded.kind, CityFileKind::MacCity);
    assert_eq!(loaded.population_density_at(0, 0), Some(0));
    assert_eq!(loaded.police_coverage_at(0, 0), Some(0));
    assert_eq!(loaded.fire_coverage_at(0, 0), Some(0));
    assert_eq!(loaded.rate_of_growth_at(0, 0), Some(RATE_OF_GROWTH_NEUTRAL));
    let _ = std::fs::remove_file(path);
}

#[test]
fn budget_service_counts_live_palette_facilities_once() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    stamp_complete_building_for_test(&mut city, 10, 10, 210, 3);
    stamp_complete_building_for_test(&mut city, 20, 10, 211, 3);
    stamp_complete_building_for_test(&mut city, 30, 10, 209, 3);
    stamp_complete_building_for_test(&mut city, 40, 10, 214, 3);
    stamp_complete_building_for_test(&mut city, 50, 10, 217, 4);
    set_building_and_pe_count_for_test(&mut city, 70, 10, 210);

    for (x, y) in highway_segment_tiles((80, 80), NetworkAxis::GridX) {
        set_building_and_pe_count_for_test(&mut city, x, y, 74);
    }
    set_building_and_pe_count_for_test(&mut city, 5, 5, 29);
    set_building_and_pe_count_for_test(&mut city, 6, 5, 63);
    set_building_and_pe_count_for_test(&mut city, 7, 5, 44);
    set_building_and_pe_count_for_test(&mut city, 8, 5, 81);
    set_building_and_pe_count_for_test(&mut city, 9, 5, 90);
    set_building_and_pe_count_for_test(&mut city, 10, 5, 106);
    // PE subway built @ `0x473807`: `0x4ca41c` + `dwTileCount[0xe9]` (not a XUND rescan).
    pe_place_xund_442370(
        &mut city,
        11,
        5,
        underground_tile_for_mask(UndergroundKind::Subway, NETWORK_LEFT | NETWORK_RIGHT),
    );
    pe_place_xund_442370(&mut city, 12, 5, UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR);
    pe_place_xund_442370(&mut city, 13, 5, UNDER_TILE_GREY_BOX_2);
    pe_place_tile_441f00(&mut city, 14, 5, SUBWAY_STATION_BUILDING_ID);

    let counts = budget_service_built_counts_for_city(&city);

    assert_eq!(counts.police, 1);
    assert_eq!(counts.fire, 1);
    assert_eq!(counts.health, 1);
    assert_eq!(counts.schools, 1);
    assert_eq!(counts.colleges, 1);
    assert_eq!(counts.highways, 5);
    assert_eq!(counts.roads, 2);
    assert_eq!(counts.bridges, 3);
    assert_eq!(counts.rail, 1);
    assert_eq!(counts.tunnel, 1);
    assert_eq!(counts.subway, 4); // 3 PlaceXUND occupancy + 1 SubStation
}

#[test]
fn surface_query_distinguishes_tunnel_entrances_from_bridge_tiles() {
    assert_eq!(surface_building_query_label(63), Some("Tunnel"));
    assert_eq!(surface_building_query_label(66), Some("Tunnel"));
    assert_eq!(surface_building_query_label(81), Some("Bridge"));
    assert_eq!(surface_building_query_label(89), Some("Bridge"));
    assert_eq!(surface_building_query_label(90), Some("Bridge"));
    assert_eq!(surface_building_query_label(91), Some("Bridge"));
    assert_eq!(surface_building_query_label(106), Some("Bridge"));
    assert_eq!(surface_building_query_label(107), Some("Bridge"));
}

#[test]
fn construction_refresh_does_not_roll_graph_history() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(500_000);
    let history = (0..GRAPH_HISTORY_VALUES as u32).collect::<Vec<_>>();
    city.set_misc_u32_values(GRAPH_HISTORY_MISC_OFFSET, &history);
    let mut state = test_state(city);
    state.tool = EditTool::Road;

    assert_eq!(
        state.place_network_path(&[(10, 10), (11, 10)], NetworkKind::Road),
        2
    );

    assert_eq!(
        state
            .city
            .misc_u32_values(GRAPH_HISTORY_MISC_OFFSET, GRAPH_HISTORY_VALUES),
        history
    );
}

#[test]
fn facility_census_counts_palette_buildings_once_by_category() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");

    stamp_complete_building_for_test(&mut city, 10, 10, 198, 1);
    stamp_complete_building_for_test(&mut city, 14, 10, 207, 4);
    city.set_building(24, 10, 207);

    stamp_complete_building_for_test(&mut city, 10, 20, 220, 1);
    stamp_complete_building_for_test(&mut city, 14, 20, 235, 2);
    stamp_complete_building_for_test(&mut city, 18, 20, 244, 2);
    stamp_complete_building_for_test(&mut city, 22, 20, 250, 3);

    stamp_complete_building_for_test(&mut city, 10, 30, 210, 3);
    stamp_complete_building_for_test(&mut city, 16, 30, 211, 3);
    stamp_complete_building_for_test(&mut city, 22, 30, 209, 3);
    stamp_complete_building_for_test(&mut city, 28, 30, 216, 4);

    stamp_complete_building_for_test(&mut city, 10, 40, 214, 3);
    stamp_complete_building_for_test(&mut city, 16, 40, 217, 4);
    stamp_complete_building_for_test(&mut city, 24, 40, 245, 2);
    stamp_complete_building_for_test(&mut city, 30, 40, 212, 3);

    stamp_complete_building_for_test(&mut city, 10, 52, 13, 1);
    stamp_complete_building_for_test(&mut city, 14, 52, 213, 3);
    stamp_complete_building_for_test(&mut city, 20, 52, 215, 4);
    stamp_complete_building_for_test(&mut city, 28, 52, 218, 4);
    stamp_complete_building_for_test(&mut city, 36, 52, 248, 3);

    stamp_complete_building_for_test(&mut city, 10, 64, 233, 1);
    stamp_complete_building_for_test(&mut city, 14, 64, 236, 2);
    stamp_complete_building_for_test(&mut city, 18, 64, 237, 2);

    stamp_complete_building_for_test(&mut city, 10, 72, 243, 2);
    stamp_complete_building_for_test(&mut city, 16, 72, 208, 3);
    stamp_complete_building_for_test(&mut city, 22, 72, 219, 1);
    stamp_complete_building_for_test(&mut city, 28, 72, 255, 4);
    stamp_complete_building_for_test(&mut city, 36, 72, 251, 4);
    stamp_complete_building_for_test(&mut city, 44, 72, 254, 4);

    for x in 60..64 {
        city.set_zone(x, 60, ZONE_AIRPORT);
    }
    city.set_building(60, 60, 221);
    for x in 60..63 {
        city.set_zone(x, 62, ZONE_SEAPORT);
    }
    city.set_building(60, 62, 227);

    assert_eq!(
        facility_census_for_city(&city),
        FacilityCensus {
            power_plants: 2,
            water_utilities: 4,
            civic_services: 4,
            education: 4,
            parks: 5,
            transit: 3,
            rewards: 4,
            arcologies: 2,
            airport_tiles: 4,
            seaport_tiles: 3,
            developed_airport_tiles: 1,
            developed_seaport_tiles: 1,
        }
    );
}

#[test]
fn mac_police_coverage_reads_gate_from_misc_cursor_1039() {
    let mut city = City::new_blank("/tmp/sc2000-mac-police-gate.sc2", "test").expect("blank city");
    city.kind = CityFileKind::MacCity;
    initialize_new_city_budget_rows(&mut city, EngineFidelity::Mac68k);
    city.set_building(24, 20, 210);
    city.set_zone(24, 20, 0x81);
    let center = density_map_index(24, 20);

    let baseline =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);

    // Cursor 91 is the first three-lane history row (MISC +0x16c), not the
    // police gate. Changing that unrelated row must not change police strength.
    city.set_mac_state_misc_u32_at(0x16c, 1);
    let wrong_slot =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert_eq!(wrong_slot[center], baseline[center]);

    // CODE 4 serializes fffffe00 at cursor 1039: the signed WORD load reads
    // payload +0x103e and saveback writes the sign-extended dword at +0x103c.
    city.set_mac_state_misc_u32_at(1039 * 4, 1);
    let saved_gate =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert!(saved_gate[center] > baseline[center]);
}

#[test]
fn mac_police_gate_does_not_overwrite_or_read_win_prison_bonus() {
    let mut city = City::new_blank("/tmp/mac-police-win.sc2", "Mixed").expect("blank city");
    initialize_new_city_budget_rows(&mut city, EngineFidelity::Mac68k);
    city.set_building(24, 20, 210);
    city.set_zone(24, 20, 0x81);
    let before =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    city.set_misc_u32_at(0x103c, 99);
    let after =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert_eq!(before, after, "Win95 prison bonus is not a Mac police gate");
    city.refresh_mac_monthly_random_xmic_rows(
        100,
        100,
        100,
        100,
        100,
        false,
        &mut MacToolboxRng::new(1),
    );
    assert_eq!(city.misc_u32_at(0x103c), Some(99));
    city.set_mac_police_coverage_gate(1);
    let updated =
        super::super::density::live_mac_service_coverage_map(&city, 210, EngineFidelity::Mac68k);
    assert!(updated[density_map_index(24, 20)] > before[density_map_index(24, 20)]);
    assert_eq!(city.misc_u32_at(0x103c), Some(99));
}

#[test]
fn simulation_day0_january_skips_budget_window_when_auto_budget_is_on() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_saved_date(CityDate {
        year: 1900,
        month: 12,
    });
    assert!(set_city_auto_budget_enabled(&mut city, true));
    let mut state = test_state(city);
    state.traffic.set_speed(GameSpeed::Llama);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    let start = state.game_clock;
    state.game_clock = start.wrapping_add_days(1);
    state.apply_game_clock_advance(start, GameClockAdvance { days: 1, months: 0 });

    assert_eq!(state.game_clock.month(), 1);
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.traffic.speed(), GameSpeed::Llama);
}

#[test]
fn mac_native_auto_budget_controls_january_budget_window() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for auto_budget in [false, true] {
            let mut city =
                City::new_blank("/tmp/citygame-january-budget.sc2", "MacBudget").unwrap();
            city.kind = kind;
            city.set_saved_date_and_day(
                CityDate {
                    year: 1900,
                    month: 12,
                },
                24,
            );
            city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
            city.set_city_funds(200);
            let accrual = budget_row_misc_offset_for_city(&city, 5, 2).unwrap();
            city.set_budget_misc_u32_values(accrual, &[120]);
            set_auto_budget_enabled_for_engine(&mut city, EngineFidelity::Mac68k, auto_budget);
            city.set_mac_state_misc_low_byte_at(MISC_YEAR_END_OFFSET, 0xff);
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.traffic.set_speed(GameSpeed::Llama);
            state.game_clock = GameClock::new_with_day(1900, 12, 24);

            let advance = state.advance_frame_simulation(Duration::from_millis(20));
            assert_eq!(state.game_clock, GameClock::new(1901, 1));
            assert_eq!(
                state.city_info_dialog,
                (!auto_budget).then_some(CityInfoDialog::Budget)
            );
            assert_eq!(advance.days, u32::from(auto_budget));
            if !auto_budget {
                if kind == CityFileKind::MacCity {
                    assert_eq!(
                        state
                            .city
                            .mac_state_misc_u32_at(MISC_YEAR_END_OFFSET)
                            .unwrap() as u8,
                        0xff
                    );
                }
                assert_eq!(state.city.city_funds(), 200);
                assert_eq!(state.city.saved_date().unwrap().month, 12);
                let held_rng = state.mac_process_rng.seed();
                assert_eq!(
                    state.advance_frame_simulation(Duration::from_secs(2)),
                    GameClockAdvance::default()
                );
                assert_eq!(state.mac_process_rng.seed(), held_rng);
                assert_eq!(state.city.budget_misc_u32_at(accrual), Some(120));
                state.close_city_info_dialog();
                assert_eq!(
                    state.advance_frame_simulation(Duration::ZERO),
                    GameClockAdvance { days: 1, months: 1 }
                );
                assert_eq!(state.game_clock, GameClock::new(1901, 1));
            }
            assert_eq!(state.city.city_funds(), 190);
            if kind == CityFileKind::MacCity {
                assert_eq!(
                    state
                        .city
                        .mac_state_misc_u32_at(MISC_YEAR_END_OFFSET)
                        .unwrap() as u8,
                    0
                );
            }
            assert_eq!(state.city.saved_date().unwrap().month, 1);
            assert_eq!(state.traffic.speed(), GameSpeed::Llama);
        }
    }
}

#[test]
fn mac_january_without_latch_skips_budget_review() {
    let mut city = City::new_blank("/tmp/citygame-unarmed-january.city", "Unarmed").unwrap();
    city.kind = CityFileKind::MacCity;
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        24,
    );
    city.set_mac_state_misc_u32_at(MISC_CITY_MODE_OFFSET, CITY_MODE_GAMEPLAY);
    city.set_mac_state_misc_low_byte_at(MISC_YEAR_END_OFFSET, 0);
    set_auto_budget_enabled_for_engine(&mut city, EngineFidelity::Mac68k, false);
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.traffic.set_speed(GameSpeed::Llama);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(20))
            .days,
        1
    );
    assert_eq!(state.city_info_dialog, None);
    assert_eq!(state.city.saved_date().unwrap().month, 1);
}

#[test]
fn budget_ytd_prorates_original_help_text_september_police_example() {
    let mut values = [0_u32; BUDGET_SERVICE_MISC_STRIDE];
    let police_row = BUDGET_SERVICE_ROW_START;
    for month in 8..12 {
        values[3 + month * 2] = 1;
        values[4 + month * 2] = 100;
    }

    assert_eq!(budget_row_year_to_date(police_row, &values, 12), 400);
}

#[test]
fn monthly_simulation_does_not_roll_misc_graph_history() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let history = (0..GRAPH_HISTORY_VALUES as u32).collect::<Vec<_>>();
    let extra_history = (1_000..1_000 + GRAPH_EXTRA_HISTORY_VALUES as u32).collect::<Vec<_>>();
    city.set_misc_u32_values(GRAPH_HISTORY_MISC_OFFSET, &history);
    city.set_misc_u32_values(GRAPH_EXTRA_HISTORY_MISC_OFFSET, &extra_history);
    city.set_building(10, 10, 0x70);
    city.set_building(11, 10, 0x70);
    city.set_building(12, 10, 0x84);
    let mut state = test_state(city);

    state.refresh_live_misc_stats();
    assert_eq!(
        state
            .city
            .misc_u32_values(GRAPH_HISTORY_MISC_OFFSET, GRAPH_HISTORY_VALUES),
        history,
        "PE graph history is XGRP; leftover MISC rings must stay unread and unrolled"
    );
    assert_eq!(
        state
            .city
            .misc_u32_values(GRAPH_EXTRA_HISTORY_MISC_OFFSET, GRAPH_EXTRA_HISTORY_VALUES),
        extra_history
    );
}

#[test]
fn population_panel_demographics_prefer_bss_fame_national_weather() {
    let mut city = City::new_blank("/tmp/sc2000-pop-bss.sc2", "PopBss").expect("blank");
    city.set_misc_u32_at(MISC_CITY_FAME_OFFSET, 1);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 2);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 3);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 4);
    city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 5);
    city.set_misc_u32_at(MISC_HEAT_OFFSET, 6);
    city.set_misc_u32_at(MISC_WIND_OFFSET, 7);
    city.set_misc_u32_at(MISC_HUMID_OFFSET, 8);
    city.set_misc_u32_at(MISC_WEATHER_TREND_OFFSET, 9);
    city.pe_dw_city_fame = 111;
    city.pe_dw_national_population = 222;
    city.pe_dw_national_value = 333;
    city.pe_w_national_tax = 10;
    city.pe_w_national_trend = 11;
    city.pe_b_weather_heat = 12;
    city.pe_b_weather_wind = 13;
    city.pe_b_weather_humid = 14;
    city.pe_b_weather_trend = 2;
    let rows = population_panel_demographic_rows(
        &city,
        PopulationStats::default(),
        CityServiceOutcomes::default(),
    );
    let get = |label: &str| {
        rows.iter()
            .find(|(name, _)| *name == label)
            .map(|(_, value)| *value)
            .expect(label)
    };
    assert_eq!(get("City Fame"), 111);
    assert_eq!(get("National Population"), 222);
    assert_eq!(get("National Value"), 333);
    assert_eq!(get("National Tax"), 10);
    assert_eq!(get("National Trend"), 11);
    assert_eq!(get("Heat"), 12);
    assert_eq!(get("Wind"), 13);
    assert_eq!(get("Humid"), 14);
    assert_eq!(get("Weather Trend"), 2);
}

#[test]
fn monthly_simulation_rolls_original_xgrp_graph_history() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    let xgrp_history = (0..XGRP_VALUES as u32).collect::<Vec<_>>();
    city.set_chunk_u32_values(XGRP_CHUNK_NAME, 0, &xgrp_history);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 123_000);
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 456_000);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 7);
    city.set_building(10, 10, 0x70);
    city.set_building(11, 10, 0x70);
    city.set_building(12, 10, 0x84);
    let mut state = test_state(city);
    state.game_clock = GameClock::new(1900, 2);
    state.city.pe_dw_city_age = 25;

    state.refresh_live_misc_stats();

    let updated = state.city.chunk_u32_values(XGRP_CHUNK_NAME, 0, XGRP_VALUES);
    let population = population_stats_for_city(&state.city);
    let outcomes = city_service_outcomes_for_city(&state.city);
    let expected_latest = xgrp_history_latest_values(&state.city, population, outcomes);
    for (series_index, latest) in expected_latest.into_iter().enumerate() {
        let base = series_index * XGRP_SAMPLES_PER_SERIES;
        assert_eq!(updated[base], latest);
        assert_eq!(
            &updated[base + 1..base + XGRP_MONTH_SAMPLES],
            &xgrp_history[base..base + XGRP_MONTH_SAMPLES - 1]
        );
        assert_eq!(
            &updated[base + XGRP_HALF_YEAR_OFFSET
                ..base + XGRP_HALF_YEAR_OFFSET + XGRP_HALF_YEAR_SAMPLES],
            &xgrp_history[base + XGRP_HALF_YEAR_OFFSET
                ..base + XGRP_HALF_YEAR_OFFSET + XGRP_HALF_YEAR_SAMPLES]
        );
        assert_eq!(
            &updated[base + XGRP_FIVE_YEAR_OFFSET
                ..base + XGRP_FIVE_YEAR_OFFSET + XGRP_FIVE_YEAR_SAMPLES],
            &xgrp_history[base + XGRP_FIVE_YEAR_OFFSET
                ..base + XGRP_FIVE_YEAR_OFFSET + XGRP_FIVE_YEAR_SAMPLES]
        );
    }
}

#[test]
fn xgrp_latest_values_match_pe_update_graph_data() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_misc_u32_at(MISC_NATIONAL_VALUE_OFFSET, 456_000);
    city.set_misc_u32_at(MISC_NATIONAL_POPULATION_OFFSET, 123_000);
    city.set_misc_u32_at(MISC_NATIONAL_TAX_OFFSET, 7);
    // XGRP reads land/LE/EQ/pollution/crime BSS, not outcomes / density-map fallbacks.
    city.set_misc_u32_at(MISC_LAND_VALUE_OFFSET, 100);
    city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 76);
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 90);
    city.pe_dw_city_land_value = 100;
    city.set_pe_water_coverage_percent(0x2a);
    city.set_pe_dw_arcology_population(8);
    city.set_zone(10, 10, ZONE_RESIDENTIAL_LIGHT);
    city.set_building(10, 10, 0x70);
    city.set_building(
        12,
        10,
        network_building_for_mask(NetworkKind::Road, NETWORK_LEFT | NETWORK_RIGHT),
    );
    let population = PopulationStats {
        residential_population: 40,
        commercial_population: 20,
        industrial_population: 12,
        arcology_population: 8,
        ..PopulationStats::default()
    };
    let outcomes = CityServiceOutcomes {
        land_value: 100,
        life_expectancy: 76,
        education_quotient: 90,
        ..CityServiceOutcomes::default()
    };

    let traffic = live_traffic_count_for_city(&city, population);
    let developed_divisor = pe_attitude_developed_tile_count(&city) / 4 + 1;
    let counters = live_city_scenario_counters_for_city(&city, population, outcomes);
    // Seed CTP BSS so XGRP matches the live scenario counters PE would mirror.
    city.pe_dw_city_pollution = counters.pollution;
    city.pe_dw_city_crime = counters.crime;
    city.set_pe_traffic_count(traffic);
    city.set_pe_power_used_percent(37);

    let latest = xgrp_history_latest_values(&city, population, outcomes);
    let arco = 8;

    assert_eq!(latest[0], 40 + 20 + 12 + arco);
    assert_eq!(latest[1], 40 + arco / 2);
    assert_eq!(latest[2], 20 + arco / 4);
    assert_eq!(latest[3], 12 + arco / 4);
    assert_eq!(latest[4], traffic / 2, "traffic / (road+hwy+bridge+1)");
    assert_eq!(latest[5], counters.pollution / developed_divisor);
    assert_eq!(latest[6], 100 / developed_divisor);
    assert_eq!(latest[7], counters.crime / developed_divisor);
    assert_eq!(
        latest[8], 63,
        "GRP_POWERPERCENT is 100 - dwPowerUsedPercentage"
    );
    assert_eq!(
        latest[9],
        100 - 0x2a,
        "GRP_WATERPERCENT is 100 - [0x4cc4b8] coverage BSS"
    );
    assert_eq!(latest[10], 76);
    assert_eq!(latest[11], 90);
    assert_eq!(latest[12], unemployment_percent_for_city(&city, population));
    assert_eq!(latest[13], 456_000, "GNP is raw dwNationalValue");
    assert_eq!(
        latest[14], 123_000,
        "national pop is raw dwNationalPopulation"
    );
    assert_eq!(latest[15], 7);
}

#[test]
fn special_zone_development_sprites_do_not_use_palette_animation() {
    for &building in AIRPORT_DEVELOPMENT_BUILDINGS
        .iter()
        .chain(AIRPORT_LARGE_DEVELOPMENT_BUILDINGS.iter())
        .chain(SEAPORT_DEVELOPMENT_BUILDINGS.iter())
        .chain(SEAPORT_LARGE_DEVELOPMENT_BUILDINGS.iter())
    {
        let Some(sprite_id) = building_sprite_id(building) else {
            continue;
        };
        assert!(
            !should_animate_sprite(sprite_id),
            "building {building} sprite {sprite_id} must not be frame-remapped"
        );
    }
}

#[test]
fn rci_development_sprites_do_not_use_palette_animation() {
    for &building in RESIDENTIAL_1X1_BUILDINGS
        .iter()
        .chain(RESIDENTIAL_2X2_BUILDINGS.iter())
        .chain(RESIDENTIAL_3X3_BUILDINGS.iter())
        .chain(COMMERCIAL_1X1_BUILDINGS.iter())
        .chain(COMMERCIAL_2X2_BUILDINGS.iter())
        .chain(COMMERCIAL_3X3_BUILDINGS.iter())
        .chain(INDUSTRIAL_1X1_BUILDINGS.iter())
        .chain(INDUSTRIAL_2X2_BUILDINGS.iter())
        .chain(INDUSTRIAL_3X3_BUILDINGS.iter())
    {
        let Some(sprite_id) = building_sprite_id(building) else {
            continue;
        };
        assert!(
            !should_animate_sprite(sprite_id),
            "RCI building {building} sprite {sprite_id} must not be frame-remapped"
        );
    }
}

#[test]
fn water_radioactive_and_smoke_xbld_sprites_use_palette_animation_frames() {
    for building in 1..=u8::MAX {
        let Some(sprite_id) = building_sprite_id(building) else {
            continue;
        };
        let expected =
            sprite_id == 1005 || is_water_sprite(sprite_id) || is_smoke_sprite(sprite_id);
        assert_eq!(
            should_animate_sprite(sprite_id),
            expected,
            "XBLD building {building} sprite {sprite_id} animation classification mismatch"
        );
    }
}

#[test]
fn non_smoke_placeable_building_sprites_do_not_use_palette_animation() {
    let cases = [
        EditTool::BusDepot,
        EditTool::RailDepot,
        EditTool::SubStation,
        EditTool::SmallPark,
        EditTool::BigPark,
        EditTool::Zoo,
        EditTool::Stadium,
        EditTool::Marina,
        EditTool::PowerPlantCoal,
        EditTool::PowerPlantHydro,
        EditTool::PowerPlantOil,
        EditTool::PowerPlantGas,
        EditTool::PowerPlantNuclear,
        EditTool::PowerPlantWind,
        EditTool::PowerPlantSolar,
        EditTool::PowerPlantMicrowave,
        EditTool::PowerPlantFusion,
        EditTool::Police,
        EditTool::FireStation,
        EditTool::Hospital,
        EditTool::Prison,
        EditTool::School,
        EditTool::College,
        EditTool::Library,
        EditTool::Museum,
        EditTool::WaterPump,
        EditTool::WaterTower,
        EditTool::WaterTreatment,
        EditTool::Desalinization,
        EditTool::MayorHouse,
        EditTool::CityHall,
        EditTool::Statue,
        EditTool::BraunLlamaDome,
        EditTool::ArcologyPlymouth,
        EditTool::ArcologyForest,
        EditTool::ArcologyDarco,
        EditTool::ArcologyLaunch,
    ];
    let animated_buildings = [201, 202, 207, 250];

    for tool in cases {
        let building = placeable_building_for_tool(tool).expect("placeable building");
        let sprite_id = building_sprite_id(building).expect("building sprite");
        let should_animate = animated_buildings.contains(&building);
        assert_eq!(
            should_animate_sprite(sprite_id),
            should_animate,
            "{tool:?} building {building} sprite {sprite_id} animation mismatch"
        );
    }
}

#[test]
fn cursor_resources_follow_extracted_sc2k_tool_art() {
    assert_eq!(GameCursor::for_tool(EditTool::Query).resource_id(), 1023);
    assert_eq!(GameCursor::for_tool(EditTool::Bulldoze).resource_id(), 1002);
    assert_eq!(GameCursor::for_tool(EditTool::Trees).resource_id(), 1003);
    assert_eq!(GameCursor::for_tool(EditTool::Power).resource_id(), 1008);
    assert_eq!(
        GameCursor::for_tool(EditTool::PowerPlantCoal).resource_id(),
        1008
    );
    assert_eq!(GameCursor::for_tool(EditTool::Road).resource_id(), 1017);
    assert_eq!(GameCursor::for_tool(EditTool::Highway).resource_id(), 1021);
    assert_eq!(GameCursor::for_tool(EditTool::Rail).resource_id(), 1022);
    assert_eq!(GameCursor::for_tool(EditTool::Subway).resource_id(), 1022);
    assert_eq!(
        GameCursor::for_tool(EditTool::RaiseTerrain).resource_id(),
        1012
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::LowerTerrain).resource_id(),
        1013
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::LevelTerrain).resource_id(),
        1015
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::StretchTerrain).resource_id(),
        1012
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::SpreadTerrain).resource_id(),
        1015
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneResidential).resource_id(),
        1004
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneCommercial).resource_id(),
        1005
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneCommercialDense).resource_id(),
        1005
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneIndustrial).resource_id(),
        1006
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ZoneIndustrialDense).resource_id(),
        1006
    );
    assert_eq!(GameCursor::for_tool(EditTool::Water).resource_id(), 1016);
    assert_eq!(GameCursor::for_tool(EditTool::Stream).resource_id(), 1016);
    assert_eq!(GameCursor::for_tool(EditTool::Forest).resource_id(), 1003);
    assert_eq!(GameCursor::for_tool(EditTool::Pipes).resource_id(), 1019);
    assert_eq!(
        GameCursor::for_tool(EditTool::WaterPump).resource_id(),
        1019
    );
    assert_eq!(GameCursor::for_tool(EditTool::Police).resource_id(), 1020);
    assert_eq!(GameCursor::for_tool(EditTool::School).resource_id(), 1007);
    assert_eq!(
        GameCursor::for_tool(EditTool::SmallPark).resource_id(),
        1026
    );
    assert_eq!(GameCursor::for_tool(EditTool::Marina).resource_id(), 1026);
    assert_eq!(GameCursor::for_tool(EditTool::Seaport).resource_id(), 1025);
    assert_eq!(GameCursor::for_tool(EditTool::Airport).resource_id(), 1025);
    assert_eq!(
        GameCursor::for_tool(EditTool::MayorHouse).resource_id(),
        1024
    );
    assert_eq!(
        GameCursor::for_tool(EditTool::ArcologyPlymouth).resource_id(),
        1024
    );
    assert_eq!(GameCursor::for_tool(EditTool::Sign).resource_id(), 1027);
    assert_eq!(
        GameCursor::for_tool(EditTool::DispatchPolice).resource_id(),
        1028
    );
}

#[test]
fn pe_rci_demand_updates_attitude_newspaper_46881f_queues_low_stories() {
    // PE @ `0x46881f`: rand&0x7f < score → 0x11/0x12/0x10; rand&0xf > score → high ids.
    let mut city = City::new_blank("/tmp/sc2000-attitude-news.sc2", "AttitudeNews").expect("blank");
    city.set_pe_traffic_count(400);
    city.set_pe_attitude_tax_bases([1, 0, 0]);
    city.set_pe_attitude_demand_inputs(1, 200, 0, 200);
    city.recompute_pe_attitude_scores(0, 0, 0);
    assert!(city.pe_attitude_score_at(0) >= 0x80);
    assert!(city.pe_attitude_score_at(1) >= 0x80);
    assert!(city.pe_attitude_score_at(3) >= 0x80);
    let mut crt = Win95CrtRand::new(1);
    pe_rci_demand_updates_attitude_newspaper_46881f(&mut city, &mut crt);
    // High arms need rand&0xf > score (≥128) — impossible; only low stories.
    let mut found = [false; 3];
    for slot in 0..6 {
        match city.pe_newspaper_story_id_at(slot) {
            Some(0x11) => found[0] = true,
            Some(0x12) => found[1] = true,
            Some(0x10) => found[2] = true,
            Some(0x3e) | Some(0x3f) | Some(0x3d) => panic!("unexpected high story"),
            _ => {}
        }
    }
    assert!(found.iter().all(|&f| f), "expected stories 0x11/0x12/0x10");
}

#[test]
fn pe_rci_demand_updates_unemployment_newspaper_4688a8_queues_story_15() {
    // PE @ `0x4688a8`: rand&0x3f < unemp → story 0x15; rand&3 > unemp → 0x42.
    let mut city = City::new_blank("/tmp/sc2000-unemp-news.sc2", "UnempNews").expect("blank");
    city.set_pe_unemployment_percent(0x40); // > 0x3f → always low story
    let mut crt = Win95CrtRand::new(1);
    pe_rci_demand_updates_unemployment_newspaper_4688a8(&mut city, &mut crt);
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(0x15));
    // High arm needs rand&3 > 0x40 — impossible.
    for slot in 0..6 {
        assert_ne!(city.pe_newspaper_story_id_at(slot), Some(0x42));
    }
}

#[test]
fn pe_rci_demand_updates_eq_newspaper_468905_queues_story_13_when_eq_low() {
    // PE @ `0x468905`: EQ < 0x50 and rand%0x50 > EQ → story 0x13.
    let mut city = City::new_blank("/tmp/sc2000-eq-news.sc2", "EqNews").expect("blank");
    city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 0);
    assert_eq!(city.pe_eq_education_average(), 0);
    let mut queued = false;
    for seed in 1..200 {
        let mut c = City::new_blank("/tmp/sc2000-eq-news2.sc2", "EqNews2").expect("blank");
        c.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 0);
        let mut rng = Win95CrtRand::new(seed);
        pe_rci_demand_updates_eq_newspaper_468905(&mut c, &mut rng);
        if c.pe_newspaper_story_id_at(0) == Some(0x13) {
            queued = true;
            break;
        }
    }
    assert!(queued, "expected story 0x13 for EQ=0");
}

#[test]
fn pe_rci_demand_updates_eq_newspaper_468905_queues_story_40_when_eq_high() {
    // PE high arm: EQ >= 0x50 and rand%0x50 < (EQ-0x50) → story 0x40.
    let mut queued = false;
    for seed in 1..400 {
        let mut city = City::new_blank("/tmp/sc2000-eq-news-hi.sc2", "EqNewsHi").expect("blank");
        city.set_misc_u32_at(MISC_CITY_EDUCATION_QUOTIENT_OFFSET, 0x7f); // threshold 0x2f
        let mut rng = Win95CrtRand::new(seed);
        pe_rci_demand_updates_eq_newspaper_468905(&mut city, &mut rng);
        if city.pe_newspaper_story_id_at(0) == Some(0x40) {
            queued = true;
            break;
        }
    }
    assert!(queued, "expected story 0x40 for high EQ");
}

#[test]
fn pe_rci_demand_updates_le_newspaper_468960_queues_story_14_when_le_low() {
    // PE @ `0x468960`: LE < 0x3c and rand%0x3c > LE → story 0x14.
    let mut queued = false;
    for seed in 1..200 {
        let mut city = City::new_blank("/tmp/sc2000-le-news.sc2", "LeNews").expect("blank");
        city.set_misc_u32_at(MISC_CITY_LIFE_EXPECTANCY_OFFSET, 0);
        let mut rng = Win95CrtRand::new(seed);
        pe_rci_demand_updates_le_newspaper_468960(&mut city, &mut rng);
        if city.pe_newspaper_story_id_at(0) == Some(0x14) {
            queued = true;
            break;
        }
    }
    assert!(queued, "expected story 0x14 for LE=0");
}

#[test]
fn health_amenities_newspaper_labels_match_opensc2k_status_names() {
    // OpenSC2K CityStatusBar / NEEDS packaging for LE + amenity demand papers.
    assert_eq!(
        pe_health_amenities_newspaper_label(0x11),
        Some("High traffic")
    );
    assert_eq!(pe_health_amenities_newspaper_label(0x3d), Some("Low crime"));
    assert_eq!(
        pe_health_amenities_newspaper_label(0x13),
        Some("Poor education")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x42),
        Some("Good employment")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x14),
        Some("Poor health")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x41),
        Some("Good health")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x33),
        Some("Hospitals Demanded")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x37),
        Some("Residents Demand Zoo")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x3a),
        Some("Residents Demand Park")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x2e),
        Some("Power Plant Needed")
    );
    assert_eq!(
        pe_health_amenities_newspaper_label(0x3c),
        Some("Commerce Needs Connections")
    );
    assert_eq!(
        pe_health_amenities_newspaper_headline(NewspaperEdition::Extra, 0x14),
        "Extra: Poor health"
    );
    assert_eq!(
        pe_health_amenities_newspaper_headline(NewspaperEdition::Courier, 0x33),
        "Hospitals Demanded"
    );
    assert_eq!(
        pe_health_amenities_newspaper_headline(NewspaperEdition::Courier, 0x2f),
        "Citizens Demand Road & Rail"
    );
    assert!(pe_health_amenities_newspaper_story("Testville", 0x41).contains("Testville"));
    assert!(pe_health_amenities_newspaper_label(0x26).is_none());
}

#[test]
fn bridge_collapse_newspaper_story_has_event_specific_copy() {
    assert_eq!(
        pe_bridge_collapse_headline(NewspaperEdition::Courier),
        "Bridge collapse disrupts traffic"
    );
    assert_eq!(
        pe_bridge_collapse_headline(NewspaperEdition::Extra),
        "Extra: Bridge collapse disrupts traffic"
    );
    assert_eq!(
        pe_bridge_collapse_story("Testville"),
        "A weakened bridge in Testville has collapsed, interrupting travel across the route. Crews must clear the damaged structure before normal traffic can return."
    );
}

#[test]
fn pe_rci_demand_updates_neighbor_fame_newspaper_4689c8_queues_story_4() {
    // PE @ `0x4689c8`: rand&7==0 and (fame_base+year) >= rank → story 4, clear slot.
    let mut city = City::new_blank("/tmp/sc2000-neighbor-fame.sc2", "NFame").expect("blank");
    city.pe_w_year_founded = 1900;
    city.set_pe_w_local_fame_base(50);
    city.set_pe_w_neighbor_fame_rank(3, 100); // threshold 1950 >= 100
    let mut seed = 1u32;
    let mut crt = Win95CrtRand::new(seed);
    while crt.random_u15() & 7 != 0 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
    }
    crt = Win95CrtRand::new(seed);
    pe_rci_demand_updates_neighbor_fame_newspaper_4689c8(&mut city, &mut crt);
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(4));
    assert_eq!(city.pe_newspaper_story_param_at(0), Some(3));
    assert_eq!(city.pe_w_neighbor_fame_rank(3), Some(0));
    assert_eq!(
        city.technology_discovery_year(Technology::WindPower),
        Some(0)
    );
    // PE @ `0x468a1a` rebuilds tech-unlock after clear.
    assert_ne!(city.pe_w_tech_unlock_mask(), 0);
}

#[test]
fn pe_rci_demand_updates_neighbor_fame_newspaper_4689c8_queues_story_5_high_index() {
    let mut city = City::new_blank("/tmp/sc2000-neighbor-fame5.sc2", "NFame5").expect("blank");
    city.pe_w_year_founded = 2000;
    city.set_pe_w_local_fame_base(0);
    city.set_pe_w_neighbor_fame_rank(9, 100); // story 5, param 2
    let mut seed = 1u32;
    let mut crt = Win95CrtRand::new(seed);
    while crt.random_u15() & 7 != 0 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
    }
    crt = Win95CrtRand::new(seed);
    pe_rci_demand_updates_neighbor_fame_newspaper_4689c8(&mut city, &mut crt);
    assert_eq!(city.pe_newspaper_story_id_at(0), Some(5));
    assert_eq!(city.pe_newspaper_story_param_at(0), Some(2));
    assert_eq!(city.pe_w_neighbor_fame_rank(9), Some(0));
}

#[test]
fn pe_rci_demand_updates_neighbor_fame_uses_sign_extended_32bit_threshold_sum() {
    // PE MOVSXes both WORDs to 32-bit registers before ADD/CMP at 0x4689d3..e8.
    // A 16-bit intermediate would wrap 30,000 + 25,000 negative and miss rank 1,000.
    let mut city =
        City::new_blank("/tmp/citygame-neighbor-fame-width.sc2", "NFameWidth").expect("blank");
    city.pe_w_year_founded = 25_000;
    city.set_pe_w_local_fame_base(30_000);
    city.set_pe_w_neighbor_fame_rank(0, 1_000);
    let mut seed = 1u32;
    let mut crt = Win95CrtRand::new(seed);
    while crt.random_u15() & 7 != 0 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
    }
    let mut crt = Win95CrtRand::new(seed);

    pe_rci_demand_updates_neighbor_fame_newspaper_4689c8(&mut city, &mut crt);

    assert_eq!(city.pe_newspaper_story_id_at(0), Some(4));
    assert_eq!(city.pe_w_neighbor_fame_rank(0), Some(0));
}

#[test]
fn pe_rci_demand_updates_neighbor_fame_newspaper_4689c8_skips_when_gate_fails() {
    let mut city =
        City::new_blank("/tmp/sc2000-neighbor-fame-skip.sc2", "NFameSkip").expect("blank");
    city.pe_w_year_founded = 1900;
    city.set_pe_w_local_fame_base(50);
    city.set_pe_w_neighbor_fame_rank(0, 1);
    let mut seed = 1u32;
    let mut crt = Win95CrtRand::new(seed);
    while crt.random_u15() & 7 == 0 {
        seed = seed.wrapping_add(1);
        crt = Win95CrtRand::new(seed);
    }
    crt = Win95CrtRand::new(seed);
    pe_rci_demand_updates_neighbor_fame_newspaper_4689c8(&mut city, &mut crt);
    assert_eq!(
        city.pe_newspaper_story_id_at(0),
        Some(0),
        "empty initialized story slots use the zero sentinel"
    );
    assert_eq!(city.pe_w_neighbor_fame_rank(0), Some(1));
}

#[test]
fn sim_calendar_day21_opens_extra_after_rci_newspaper_pending() {
    // PE Extra latch open after FUN_00468150 inserts (`0x47b681`); day-21 calls
    // maybe_open after the RCI newspaper arms.
    let mut city = City::new_blank("/tmp/sc2000-day21-extra.sc2", "Day21Extra").expect("blank");
    assert!(newspaper_extra_enabled_for_city(&city));
    city.queue_pe_newspaper_story(4, 0);
    assert!(city.take_pe_newspaper_extra_popup_pending());
    city.queue_pe_newspaper_story(4, 0); // re-latch for day-21
    let mut state = test_state(city);
    let _ = state.sim_calendar_day21_demand_and_graphs();
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Extra)),
    );
    assert!(!state.city.take_pe_newspaper_extra_popup_pending());
}

#[test]
fn pe_day21_monthly_stories_match_native_choice_and_conditional_draws() {
    use crate::app::newspaper_events::pe_rci_demand_updates_monthly_stories_468764;

    let cases: [(u32, u32, &[(u32, u32)]); 9] = [
        (7, 4_029_102, &[(11, 0)]),
        (9, 4_457_128, &[(12, 0)]),
        (2, 2_959_037, &[(13, 0)]),
        (6, 3_815_089, &[(14, 0)]),
        (1, 2_745_024, &[(15, 0)]),
        (5, 803_157_710, &[]),
        (3, 2_756_632_324, &[(6, 0)]),
        (14, 2_749_940_187, &[(7, 2)]),
        (82, 756_312_751, &[(6, 0), (7, 2)]),
    ];
    for (seed, end_seed, expected) in cases {
        let mut city = City::new_blank("/tmp/front-page.sc2", "Front page").unwrap();
        let mut tables = city.pe_newspaper_tables().unwrap();
        tables.stories.fill(0);
        city.install_pe_newspaper_tables(tables);
        city.set_misc_u32_at(MISC_NATIONAL_TREND_OFFSET, 2);
        let mut crt = Win95CrtRand::new(seed);
        pe_rci_demand_updates_monthly_stories_468764(&mut city, &mut crt);
        let stories: Vec<_> = (0..9)
            .filter_map(|slot| {
                let id = city.pe_newspaper_story_id_at(slot)?;
                (id != 0).then(|| (id, city.pe_newspaper_story_param_at(slot).unwrap()))
            })
            .collect();
        assert_eq!(stories, expected, "seed {seed}");
        assert_eq!(crt.seed(), end_seed, "seed {seed}");
    }
}

#[test]
fn sim_calendar_day21_inserts_baseline_newspaper_story() {
    let city =
        City::new_blank("/tmp/sc2000-day21-baseline-story.sc2", "Day21Story").expect("blank");
    let mut state = test_state(city);

    let _ = state.sim_calendar_day21_demand_and_graphs();

    assert!(
        (0..7).any(|slot| state.city.pe_newspaper_story_id_at(slot) == Some(1)),
        "Win95 day 21 inserts baseline story 1 before its conditional news"
    );
}

#[test]
fn bulldoze_lowclass_selector2_leaves_north_of_post_cleanup_window_unchanged() {
    // The traced low-class selector-2 post-cleanup calls use the snapped 2x2
    // footprint (y=20..21 for this click); y=19 is outside that window.
    let mut city =
        City::new_blank("/tmp/sc2000-bull-hwy-window.sc2", "BullHwyWindow").expect("blank");
    stamp_complete_building_for_test(&mut city, 20, 20, 0x61, 2);
    city.set_building(20, 19, 0x0d);
    city.set_zone(20, 19, ZONE_RESIDENTIAL_LIGHT | 0x80);
    city.set_terrain(20, 19, 0x7f);
    assert!(pe_destroy_building_selector2_low_class_cleanup_order(&city, 0x61, 20, 20).is_some());

    let mut state = test_state(city);
    state.bulldoze_at(20, 20);
    assert_eq!(state.city.building_at(20, 19), 0x0d);
    assert_eq!(
        state.city.zone_at(20, 19).unwrap().raw_bits(),
        ZONE_RESIDENTIAL_LIGHT | 0x80
    );
    assert_eq!(state.city.terrain_at(20, 19), 0x7f);
}

#[test]
fn win95_bulldozer_debits_per_accepted_cursor_tile() {
    let mut city = City::new_blank("/tmp/sc2000-test.sc2", "test").expect("blank city");
    city.set_city_funds(20_000);
    city.set_building(10, 10, 112);
    city.set_zone(20, 20, 0x02);
    let mut state = test_state(city);
    state.tool = EditTool::Bulldoze;

    assert!(state.apply_tool_at_tile(10, 10));
    assert_eq!(state.city.city_funds(), 19_999);
    assert!(matches!(state.city.building_at(10, 10), 1..=4));

    // The player Bulldozer clears rubble with PlaceTile(0), after its $1 gate;
    // the separate DestroyBuilding early-out for XBLD < 6 does not apply here.
    assert!(state.apply_tool_at_tile(10, 10));
    assert_eq!(state.city.city_funds(), 19_998);
    assert_eq!(state.city.building_at(10, 10), 0);

    // A zone marker on an empty low-class tile fails the rooted native gate.
    assert!(!state.apply_tool_at_tile(20, 20));
    assert_eq!(state.city.city_funds(), 19_998);

    assert!(!state.apply_tool_at_tile(30, 30));
    assert_eq!(state.city.city_funds(), 19_998);
}

#[test]
fn pipes_tool_enters_underground_bulldoze_and_query_keep_it() {
    let city = City::new_blank("/tmp/sc2000-ug-view.sc2", "ug").expect("blank");
    let mut state = test_state(city);
    assert!(!state.show_underground);

    state.apply_selected_tool(EditTool::Pipes);
    assert!(state.show_underground);

    state.apply_selected_tool(EditTool::Bulldoze);
    assert!(state.show_underground, "bulldoze stays underground");

    state.apply_selected_tool(EditTool::Query);
    assert!(state.show_underground, "query stays underground");

    state.apply_selected_tool(EditTool::Road);
    assert!(!state.show_underground, "surface tools leave underground");
}

#[test]
fn budget_window_controls_hit_their_visible_positions_at_every_scale() {
    for window in [(400, 360), (1024, 768), (2624, 1644)] {
        let dialog = CityInfoDialog::Budget;
        let page = city_info_dialog_rect_for(dialog, window);
        let center = |r: FloatRect| Vec2::new(r.x + r.w * 0.5, r.y + r.h * 0.5);
        for increase in [false, true] {
            for tax_index in 0..BUDGET_TAX_COUNT {
                assert_eq!(
                    city_info_dialog_hit_test(
                        center(budget_tax_adjust_rect(page, tax_index, increase)),
                        window,
                        dialog
                    ),
                    Some(CityInfoDialogAction::AdjustBudgetTax {
                        tax_index,
                        delta: if increase {
                            BUDGET_TAX_STEP
                        } else {
                            -BUDGET_TAX_STEP
                        }
                    })
                );
            }
            for service_index in 0..BUDGET_SERVICE_COUNT {
                assert_eq!(
                    city_info_dialog_hit_test(
                        center(budget_service_funding_adjust_rect(
                            page,
                            service_index,
                            increase
                        )),
                        window,
                        dialog
                    ),
                    Some(CityInfoDialogAction::AdjustBudgetFunding {
                        service_index,
                        delta: if increase {
                            BUDGET_FUNDING_STEP
                        } else {
                            -BUDGET_FUNDING_STEP
                        }
                    })
                );
            }
        }
        assert_eq!(
            city_info_dialog_hit_test(center(budget_window::close_rect(page)), window, dialog),
            Some(CityInfoDialogAction::Close)
        );
        assert_eq!(
            city_info_dialog_hit_test(center(budget_auto_budget_rect(page)), window, dialog),
            Some(CityInfoDialogAction::ToggleAutoBudget)
        );
        assert_eq!(
            city_info_dialog_hit_test(center(budget_bond_button_rect(page, true)), window, dialog),
            Some(CityInfoDialogAction::IssueBond)
        );
        assert_eq!(
            city_info_dialog_hit_test(center(budget_bond_button_rect(page, false)), window, dialog),
            Some(CityInfoDialogAction::RepayBond)
        );
    }
}

#[test]
fn mac_budget_native_police_decrement_coordinate_hits_police_control() {
    let point = Vec2::new(329.0, 228.0);
    assert_eq!(
        city_info_dialog_hit_test_for_engine(
            point,
            (640, 480),
            CityInfoDialog::Budget,
            EngineFidelity::Mac68k,
        ),
        Some(CityInfoDialogAction::AdjustBudgetFunding {
            service_index: 0,
            delta: -BUDGET_FUNDING_STEP,
        })
    );
    assert_eq!(
        city_info_dialog_hit_test_for_engine(
            point,
            (640, 480),
            CityInfoDialog::Budget,
            EngineFidelity::Win95,
        ),
        None,
        "Win95 retains its separate budget-panel geometry"
    );
}

#[test]
fn windows_menu_keeps_the_seven_information_windows_and_separate_map_views() {
    use crate::menu_model::{MenuEntry, menus};
    let windows = menus().iter().find(|menu| menu.title == "Windows").unwrap();
    let entries: Vec<_> = windows
        .items
        .iter()
        .filter_map(|entry| match entry {
            MenuEntry::Command { label, accel, .. } => Some((*label, *accel)),
            _ => None,
        })
        .collect();
    assert_eq!(
        entries,
        vec![
            ("Map", Some("Ctrl+M")),
            ("Budget", Some("Ctrl+B")),
            ("Ordinances", Some("Ctrl+O")),
            ("Population", Some("Ctrl+C")),
            ("Industry", Some("Ctrl+I")),
            ("Graphs", Some("Ctrl+G")),
            ("Neighbors", Some("Ctrl+H"))
        ]
    );
    let views = menus().iter().find(|menu| menu.title == "View").unwrap();
    assert_eq!(
        views
            .items
            .iter()
            .filter(|e| matches!(
                e,
                MenuEntry::Command {
                    checkable: true,
                    ..
                }
            ))
            .count(),
        13
    );
    assert!(
        menus()
            .iter()
            .find(|menu| menu.title == "Help")
            .unwrap()
            .items
            .iter()
            .any(|e| matches!(
                e,
                MenuEntry::Command {
                    id: crate::menu_model::CMD_OPEN_SCENARIO_PROGRESS,
                    ..
                }
            ))
    );
}

#[test]
fn windows_menu_shortcuts_preserve_plain_and_shifted_map_keys() {
    use sdl3::keyboard::Mod;
    for (key, dialog) in [
        (Keycode::M, CityInfoDialog::Map),
        (Keycode::B, CityInfoDialog::Budget),
        (Keycode::O, CityInfoDialog::Ordinances),
        (Keycode::C, CityInfoDialog::Population),
        (Keycode::I, CityInfoDialog::Industry),
        (Keycode::G, CityInfoDialog::Graphs),
        (Keycode::H, CityInfoDialog::Neighbors),
    ] {
        assert!(
            matches!(windows_menu_shortcut(key, Mod::LCTRLMOD), Some(AppCommand::OpenCityInfo(actual)) if actual == dialog)
        );
        assert!(windows_menu_shortcut(key, Mod::NOMOD).is_none());
        assert!(windows_menu_shortcut(key, Mod::LCTRLMOD | Mod::LSHIFTMOD).is_none());
    }
}

#[test]
fn win95_file_shortcuts_route_original_commands_without_stealing_plain_keys() {
    use sdl3::keyboard::Mod;
    for (key, expected) in [
        (Keycode::L, AppCommand::OpenCityDialog),
        (Keycode::E, AppCommand::ToggleEditMap),
        (Keycode::Z, AppCommand::OpenScenarioDialog),
        (Keycode::X, AppCommand::Quit),
    ] {
        assert_eq!(win95_file_menu_shortcut(key, Mod::LCTRLMOD), Some(expected));
        assert_eq!(win95_file_menu_shortcut(key, Mod::NOMOD), None);
        assert_eq!(
            win95_file_menu_shortcut(key, Mod::LCTRLMOD | Mod::LSHIFTMOD),
            None
        );
        assert_eq!(
            win95_file_menu_shortcut(key, Mod::LGUIMOD | Mod::LCTRLMOD),
            None
        );
    }
}

#[test]
fn pe_auto_budget_toggle_matches_native_menu_cases() {
    let mut cases = 0;
    for line in include_str!("../fixtures/pe_auto_toggle_native_20261023.csv")
        .lines()
        .skip(1)
    {
        let row: Vec<u32> = line
            .split(',')
            .map(|value| value.parse().unwrap())
            .collect();
        let mut city = City::new_blank("pe-auto-native.sc2", "native option").unwrap();
        city.set_misc_u32_at(AUTO_BUDGET_MISC_OFFSET, row[0]);
        city.set_city_funds(123456);
        let mut state = test_state(city);
        state.traffic.set_speed(GameSpeed::Cheetah);
        state.toggle_auto_budget();
        assert_eq!(
            state.city.misc_u32_at(AUTO_BUDGET_MISC_OFFSET),
            Some(row[1])
        );
        assert_eq!(state.city.city_funds(), row[4] as i32);
        assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
        cases += 1;
    }
    assert_eq!(cases, 5);
}

#[test]
fn pe_modal_newspaper_holds_clock_and_restores_entry_speed() {
    for speed in [
        GameSpeed::Paused,
        GameSpeed::Llama,
        GameSpeed::AfricanSwallow,
    ] {
        let city = blank_city_layout_fixture("pe-modal-paper.sc2", "modal paper");
        let mut state = test_state(city);
        state.apply_state_app_command(&AppCommand::SetSpeed(speed));
        let saved_speed = state.city.misc_u32_at(MISC_SPEED_OFFSET);
        let before = state.game_clock;
        state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Subscription));
        assert_eq!(state.traffic.speed(), GameSpeed::Paused, "{speed:?}");
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(1600))
                .days,
            0
        );
        assert_eq!(state.game_clock, before);
        assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), saved_speed);
        state.close_city_info_dialog();
        assert_eq!(state.traffic.speed(), speed);
        assert_eq!(state.menu_checks.speed, speed);
        assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), saved_speed);
    }
}

#[test]
fn pe_modal_budget_restores_speed_in_both_layouts_without_persisting_pause() {
    for path in ["pe-modal-budget.sc2", "pe-modal-budget.city"] {
        let city = blank_city_layout_fixture(path, "modal budget");
        let mut state = test_state(city);
        state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Llama));
        let saved_speed = state.city.misc_u32_at(MISC_SPEED_OFFSET);
        state.open_january_budget_review();
        assert_eq!(state.traffic.speed(), GameSpeed::Paused);
        assert_eq!(
            state.city.misc_u32_at(MISC_SPEED_OFFSET),
            saved_speed,
            "{path}"
        );
        state.close_modal_dialogs();
        assert_eq!(state.traffic.speed(), GameSpeed::Llama, "{path}");
        assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), saved_speed);
    }
}

#[test]
fn pe_modal_live_january_waits_for_budget_before_financial_publication() {
    let mut city = blank_city_layout_fixture("pe-modal-january.sc2", "modal January");
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        24,
    );
    city.set_city_funds(1000);
    city.set_misc_u32_at(
        budget_row_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        9000,
    );
    set_city_auto_budget_enabled(&mut city, false);
    city.set_misc_u32_at(MISC_YEAR_END_OFFSET, 1);
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    state.traffic.set_speed(GameSpeed::Cheetah);
    assert_eq!(
        state
            .advance_frame_simulation(Duration::from_millis(200))
            .days,
        1
    );
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    assert_eq!(
        state.city.city_funds(),
        1000,
        "settlement waits for DialogBox return"
    );
    assert_eq!(
        state.city.saved_date(),
        Some(CityDate {
            year: 1901,
            month: 1
        })
    );
    state.advance_frame_simulation(Duration::from_millis(400));
    assert_eq!(state.city.city_funds(), 1000);
    state.close_city_info_dialog();
    assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
    assert_eq!(state.city.city_funds(), 1010);
    assert_eq!(state.game_clock, GameClock::new(1901, 1));
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 0);
    assert_eq!(
        state.city.city_funds(),
        1010,
        "continuation settles only once"
    );
}

#[test]
fn pe_modal_subscription_opens_only_on_native_april_august_day_zero() {
    for month in 1..=12 {
        let mut city = blank_city_layout_fixture("pe-modal-subscription.sc2", "subscription");
        set_city_auto_budget_enabled(&mut city, true);
        set_city_newspaper_subscription_enabled(&mut city, true);
        let mut state = test_state(city);
        state.no_disasters_enabled = true;
        let previous = if month == 1 {
            GameClock::new_with_day(1899, 12, 24)
        } else {
            GameClock::new_with_day(1900, month - 1, 24)
        };
        state.game_clock = previous;
        state.city.set_saved_date_and_day(
            CityDate {
                year: previous.year(),
                month: previous.month(),
            },
            24,
        );
        state.traffic.set_speed(GameSpeed::Cheetah);
        assert_eq!(
            state
                .advance_frame_simulation(Duration::from_millis(200))
                .days,
            1
        );
        assert_eq!(
            state.city_info_dialog,
            matches!(month, 4 | 8)
                .then_some(CityInfoDialog::Newspaper(NewspaperEdition::Subscription)),
            "month {month}"
        );
        assert_eq!(
            state.traffic.speed(),
            if matches!(month, 4 | 8) {
                GameSpeed::Paused
            } else {
                GameSpeed::Cheetah
            }
        );
    }
}

#[test]
fn pe_modal_monthly_budget_snapshots_accrue_on_phase_zero_not_day_twenty_one() {
    let mut city = blank_city_layout_fixture("pe-modal-budget-phase.sc2", "budget phase");
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 2,
        },
        20,
    );
    set_city_auto_budget_enabled(&mut city, false);
    let current = budget_service_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
    let funding = budget_service_misc_offset(0, BUDGET_SERVICE_FUNDING_FIELD).unwrap();
    let annual = budget_service_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap();
    city.set_misc_u32_at(current, 9);
    city.set_misc_u32_at(funding, 100);
    city.set_misc_u32_at(annual, 0);
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.game_clock = GameClock::new_with_day(1900, 2, 20);
    state.traffic.set_speed(GameSpeed::Cheetah);
    state.advance_frame_simulation(Duration::from_millis(200));
    assert_eq!(state.game_clock.day(), 21);
    assert_eq!(state.city.misc_u32_at(annual), Some(0));
    assert_eq!(state.city.misc_u32_at(current), Some(9));
    for _ in 0..4 {
        state.advance_frame_simulation(Duration::from_millis(200));
    }
    assert_eq!(state.game_clock, GameClock::new(1900, 3));
    assert_eq!(state.city.misc_u32_at(annual), Some(900));
    let (count_field, funding_field) = pe_budget_month_history_fields(2);
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, count_field).unwrap()),
        Some(9)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, funding_field).unwrap()),
        Some(100)
    );
    state.advance_frame_simulation(Duration::ZERO);
    assert_eq!(state.city.misc_u32_at(annual), Some(900));
}

#[test]
fn pe_modal_replacement_retains_outer_speed_and_rejects_parent_speed_commands() {
    let mut city = blank_city_layout_fixture("pe-modal-replace.sc2", "replacement");
    city.set_building(40, 40, 0x1d);
    city.set_traffic_density(40, 40, 128);
    let mut state = test_state(city);
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Cheetah));
    state.traffic_redraw_epoch = 17;
    state.traffic_redraw_accumulator = 0.07;
    let density = state.city.traffic_density_slice().unwrap().to_vec();
    let traffic = state.traffic.congestion_slice().to_vec();
    let saved_speed = state.city.misc_u32_at(MISC_SPEED_OFFSET);
    state.open_city_info_dialog(CityInfoDialog::Budget);
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Courier));
    let rng = (
        state.pe_process_crt.seed(),
        state.pe_process_lfsr.state(),
        state.pe_process_game_rand.state(),
    );
    state.apply_state_app_command(&AppCommand::SetSpeed(GameSpeed::Turtle));
    assert_eq!(state.traffic.speed(), GameSpeed::Paused);
    state.advance_frame_simulation(Duration::from_millis(1600));
    assert_eq!(
        (
            state.pe_process_crt.seed(),
            state.pe_process_lfsr.state(),
            state.pe_process_game_rand.state()
        ),
        rng
    );
    assert_eq!(state.city.traffic_density_slice().unwrap(), density);
    assert_eq!(state.traffic.congestion_slice(), traffic);
    assert_eq!(state.traffic_redraw_epoch, 17);
    assert_eq!(state.traffic_redraw_accumulator, 0.07);
    assert_eq!(state.city.misc_u32_at(MISC_SPEED_OFFSET), saved_speed);
    state.close_modal_dialogs();
    assert_eq!(state.traffic.speed(), GameSpeed::Cheetah);
}

#[test]
fn pe_modal_manual_fastest_preserves_paused_timer_pending_on_return() {
    let city = blank_city_layout_fixture("pe-modal-fastest.sc2", "fastest paper");
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.traffic.set_speed(GameSpeed::AfricanSwallow);
    let before = state.game_clock;
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::Subscription));
    state.advance_frame_simulation(Duration::from_millis(1600));
    assert_eq!(state.game_clock, before);
    state.close_city_info_dialog();
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
    assert_eq!(state.advance_frame_simulation(Duration::ZERO).days, 1);
}

#[test]
fn mac_manual_budget_action_selects_song_and_advances_shared_lfsr_once() {
    let mut state = test_state(blank_city_layout_fixture("mac-budget-manual.city", "Mac"));
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_app_lfsr = crate::mac_random::MacAppLfsrRng::new(0x05e3);
    assert!(state.handle_ui_action_state(UiAction::OpenBudget));
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    assert_eq!(state.mac_app_lfsr.state(), 0x0bc6);
    assert_eq!(state.pending_newspaper_music.take(), Some(10_002));
    state.close_city_info_dialog();
    assert_eq!(state.mac_app_lfsr.state(), 0x0bc6);
    assert_eq!(state.pending_newspaper_music, None);
}

#[test]
fn mac_manual_budget_still_draws_when_music_is_disabled() {
    let mut state = test_state(blank_city_layout_fixture("mac-budget-muted.city", "Mac"));
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.music_enabled = false;
    state.mac_app_lfsr = crate::mac_random::MacAppLfsrRng::new(0xc257);
    assert!(state.handle_ui_action_state(UiAction::OpenBudget));
    assert_eq!(state.mac_app_lfsr.state(), 0x9f5b);
    assert_eq!(state.pending_newspaper_music.take(), Some(10_010));
    assert!(!state.music_enabled);
    state.close_city_info_dialog();
    assert!(state.handle_ui_action_state(UiAction::OpenBudget));
    assert_eq!(state.mac_app_lfsr.state(), 0x2543);
    assert_eq!(state.pending_newspaper_music, Some(10_010));
}

#[test]
fn pe_modal_mac_fidelity_does_not_inherit_native_pe_pause() {
    let mut state = test_state(blank_city_layout_fixture("pe-modal-Mac.city", "Mac modal"));
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.traffic.set_speed(GameSpeed::Llama);
    state.mac_app_lfsr = crate::mac_random::MacAppLfsrRng::new(0x729a);
    state.open_city_info_dialog(CityInfoDialog::Budget);
    assert_eq!(state.mac_app_lfsr.state(), 0xe534);
    assert_eq!(state.pending_newspaper_music, Some(10_016));
    assert_eq!(state.traffic.speed(), GameSpeed::Llama);
    state.close_city_info_dialog();
    state.open_january_budget_review();
    assert_eq!(state.mac_app_lfsr.state(), 0xd19d);
    assert_eq!(state.pending_newspaper_music, Some(10_005));
    assert_eq!(state.traffic.speed(), GameSpeed::Llama);
    state.close_city_info_dialog();
    assert_eq!(state.traffic.speed(), GameSpeed::Llama);
}

// Captured Quadra age-33299/33300 transition: one budget-song draw consumes
// 0x9a43 first, then the annual marina row consumes the next shared LFSR step.
#[test]
fn mac_quadra_2011_captured_budget_song_and_marina_draws_match_in_order() {
    let mut city =
        City::new_blank("/tmp/mac-quadra-2011-budget-marina.sc2", "2011").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x102c, 5_200);
    city.set_mac_state_misc_u32_at(0x01f0 + 0xf8 * 4, 0);
    let marina = usize::from(
        city.provision_microsim_for_building(0xf8)
            .expect("marina XMIC row")
            - crate::city::MIN_SIM_TEXT_ENTRIES,
    );
    city.set_microsim_stat1(marina, 11);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_app_lfsr = crate::mac_random::MacAppLfsrRng::new(0x9a43);

    state.open_january_budget_review();
    assert_eq!(state.mac_app_lfsr.state(), 0x2f73);
    assert_eq!(state.pending_newspaper_music, Some(10_010));
    state.close_city_info_dialog();

    state.refresh_mac_xmic_with_arcologies([0; 5], [0; 3]);
    assert_eq!(state.mac_app_lfsr.state(), 0x5ee6);
    assert_eq!(state.city.microsim_at(marina).unwrap().stat1, 14);
}

#[test]
fn mac_quadra_2018_budget_open_and_annual_marina_share_lfsr_in_order() {
    let mut city =
        City::new_blank("/tmp/mac-quadra-2018-budget-marina.sc2", "2018").expect("blank city");
    city.kind = CityFileKind::MacCity;
    city.set_mac_state_misc_u32_at(0x102c, 5_200);
    city.set_mac_state_misc_u32_at(0x01f0 + 0xf8 * 4, 0);
    let marina = usize::from(
        city.provision_microsim_for_building(0xf8)
            .expect("marina XMIC row")
            - crate::city::MIN_SIM_TEXT_ENTRIES,
    );
    city.set_microsim_stat1(marina, 10);

    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_app_lfsr = crate::mac_random::MacAppLfsrRng::new(0x45fc);

    state.open_january_budget_review();
    assert_eq!(state.mac_app_lfsr.state(), 0x8bf8);
    assert_eq!(state.pending_newspaper_music, Some(10_016));
    state.close_city_info_dialog();

    state.refresh_mac_xmic_with_arcologies([0; 5], [0; 3]);
    assert_eq!(state.mac_app_lfsr.state(), 0x0c05);
    assert_eq!(state.city.microsim_at(marina).unwrap().stat1, 17);
}

#[test]
fn pe_modal_loading_another_city_discards_the_old_phase_continuation() {
    let mut city = blank_city_layout_fixture("pe-modal-old-city.sc2", "old city");
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 12,
        },
        24,
    );
    city.set_city_funds(1000);
    city.set_misc_u32_at(
        budget_row_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap(),
        9000,
    );
    set_city_auto_budget_enabled(&mut city, false);
    city.set_misc_u32_at(MISC_YEAR_END_OFFSET, 1);
    let mut state = test_state(city);
    state.game_clock = GameClock::new_with_day(1900, 12, 24);
    state.traffic.set_speed(GameSpeed::Cheetah);
    state.advance_frame_simulation(Duration::from_millis(200));
    assert_eq!(state.city_info_dialog, Some(CityInfoDialog::Budget));
    let path = std::env::temp_dir().join(format!(
        "citygame-modal-new-document-{}.sc2",
        std::process::id()
    ));
    let mut next = City::new_blank(&path, "new city").unwrap();
    next.set_city_funds(777);
    next.set_saved_date_and_day(
        CityDate {
            year: 1905,
            month: 9,
        },
        3,
    );
    next.save(&path).unwrap();
    state.load_city(&path, (800, 600), 1.0).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!state.pe_modal_active());
    state.advance_frame_simulation(Duration::ZERO);
    assert_eq!(
        state.city.city_funds(),
        777,
        "old January settlement must not run in the new city"
    );
    assert_eq!(state.city.name, "new city");
}

#[test]
fn pe_modal_phase_zero_preserves_raw_funding_in_current_and_history() {
    let mut city = blank_city_layout_fixture("pe-modal-raw-funding.sc2", "raw funding");
    city.set_saved_date_and_day(
        CityDate {
            year: 1900,
            month: 2,
        },
        24,
    );
    set_city_auto_budget_enabled(&mut city, false);
    let current = budget_service_misc_offset(0, BUDGET_SERVICE_CURRENT_FIELD).unwrap();
    let funding = budget_service_misc_offset(0, BUDGET_SERVICE_FUNDING_FIELD).unwrap();
    let annual = budget_service_misc_offset(0, BUDGET_SERVICE_YEAR_TO_DATE_FIELD).unwrap();
    let raw = 0xcafe_0078_u32;
    city.set_misc_u32_at(current, 9);
    city.set_misc_u32_at(funding, raw);
    city.set_misc_u32_at(annual, 0);
    let mut state = test_state(city);
    state.no_disasters_enabled = true;
    state.game_clock = GameClock::new_with_day(1900, 2, 24);
    state.traffic.set_speed(GameSpeed::Cheetah);
    state.advance_frame_simulation(Duration::from_millis(200));
    assert_eq!(state.city.misc_u32_at(funding), Some(raw));
    assert_eq!(state.city.misc_u32_at(annual), Some(raw.wrapping_mul(9)));
    let (count_field, funding_field) = pe_budget_month_history_fields(2);
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, count_field).unwrap()),
        Some(9)
    );
    assert_eq!(
        state
            .city
            .misc_u32_at(budget_service_misc_offset(0, funding_field).unwrap()),
        Some(raw)
    );
    assert_eq!(
        state.city.misc_u32_at(current),
        Some(0),
        "live cost is rebuilt without changing funding"
    );
}

#[test]
fn new_city_resets_camera_from_previous_rotated_city_in_both_layouts() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        for rotation in 0..4 {
            let mut city = City::new_blank("/tmp/new-city-rotation.sc2", "Before").unwrap();
            city.kind = kind;
            let mut state = test_state(city);
            state.rotate_view(rotation);
            assert_eq!(state.camera_rotation, rotation as u8);
            state.new_city().unwrap();
            assert_eq!(state.city.rotation(), 0);
            assert_eq!(
                state.camera_rotation,
                state.city.rotation(),
                "{kind:?} old={rotation}"
            );
        }
    }
}

#[test]
fn generated_city_keeps_power_drawing_aligned_with_camera_after_rotated_city() {
    for flow in [
        title_screen::NewCityStartFlow::GenerateAndPlay,
        title_screen::NewCityStartFlow::GenerateAndEdit,
    ] {
        let city = City::new_blank("/tmp/generated-power-rotation.sc2", "Before").unwrap();
        let mut state = test_state(city);
        state.rotate_view(3);
        state
            .new_city_from_title("New grid".into(), 100_000, 1900, 1, 12345, flow)
            .unwrap();
        assert_eq!(state.camera_rotation, state.city.rotation());
        // Keep the generated map but clear two flat test runs for deterministic
        // axis checks, without imported artwork or original city fixtures.
        let runs = [((40, 40), (46, 40)), ((50, 40), (50, 46))];
        state.tool = EditTool::Power;
        for (start, end) in runs {
            let path = network_drag_path_for_kind(start, end, NetworkKind::Power);
            for &(x, y) in &path {
                state.city.set_terrain(x, y, 0);
                state.city.set_building(x, y, 0);
                state.city.set_tile_bits(x, y, TileBits::new(0));
            }
            assert_eq!(
                state.place_network_path(&path, NetworkKind::Power),
                path.len()
            );
        }
        for _ in 0..4 {
            for (start, end) in runs {
                let (a, b) = (
                    rotate_tile_coords(start.0, start.1, state.camera_rotation),
                    rotate_tile_coords(end.0, end.1, state.camera_rotation),
                );
                let expected = if a.0 != b.0 { 0x0f } else { 0x0e };
                for (x, y) in network_drag_path_for_kind(start, end, NetworkKind::Power) {
                    assert_eq!(
                        crate::tiles::rotated_building_id(
                            state.city.building_at(x, y),
                            state.city.rotation()
                        ),
                        expected
                    );
                    assert!(state.city.tile_bits_at(x, y).unwrap().powerable());
                }
            }
            state.rotate_view(1);
        }
    }
}

#[test]
fn files_export_snapshot_does_not_mark_the_live_city_saved_on_cancel() {
    for kind in [CityFileKind::MacCity, CityFileKind::WinSc2] {
        let mut city = City::new_blank("/tmp/unsaved-export-city.sc2", "Export Test").unwrap();
        city.kind = kind;
        city.set_city_funds(12345);
        let mut state = test_state(city);
        state.fit_map((800, 600), 1.0);
        state.city.mark_all_dirty();
        let original = state.city.path.clone();
        let modified = state.city.is_modified();
        let saved_view = state.city.saved_view();
        let mut snapshot = state.city_save_snapshot((800, 600), 1.0);
        let export = std::env::temp_dir().join(format!(
            "citygame-export-snapshot-{}-{kind:?}.sc2",
            std::process::id()
        ));
        snapshot.save(&export).unwrap();
        let loaded = City::load(&export).unwrap();
        assert_eq!(loaded.city_funds(), 12345);
        if kind.is_win95() {
            assert_eq!(loaded.saved_view(), snapshot.saved_view());
        }
        assert_eq!(state.city.path, original);
        assert_eq!(state.city.is_modified(), modified);
        assert_eq!(state.city.saved_view(), saved_view);
        assert!(
            !state.city.take_dirty_tiles().is_empty(),
            "preparing a Files snapshot must not erase pending redraws"
        );
        std::fs::remove_file(export).unwrap();
    }
}

#[test]
fn mac_disaster_paper_uses_imported_lead_and_keeps_preview_rng_isolated() {
    let city = City::new_blank("/tmp/mac-paper-headline.sc2", "River").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_newspaper.tick_word = Some(2032);
    let mut starts = vec![0; 46];
    let mut counts = vec![0; 46];
    starts[44..46].copy_from_slice(&1u16.to_be_bytes());
    counts[44..46].copy_from_slice(&1u16.to_be_bytes());
    let offsets = [0u8; 8];
    state.mac_newspaper.data = Some(std::sync::Arc::new(
        crate::mac_newspaper::MacPaperTokens::from_resources(
            &starts,
            &counts,
            &offsets,
            b"fire in =+ a story.  next >\0",
        )
        .unwrap(),
    ));
    state.city.queue_mac_newspaper_story(22, 0);
    state.city.set_label(251, "wolves");
    let seed = state.mac_process_rng.seed();
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::MacDisaster));
    assert_eq!(
        state.mac_newspaper.headline.as_deref(),
        Some("Fire In River")
    );
    assert_eq!(
        state.mac_newspaper.article.as_deref(),
        Some("a story.  Next wolves")
    );
    assert_eq!(state.mac_process_rng.seed(), seed);
    let page = newspaper::page_rect((1024, 768));
    let lead = newspaper::lead_hit_rect(page);
    let click = glam::Vec2::new(lead.x + lead.w * 0.5, lead.y + lead.h * 0.5);
    assert!(super::super::dialog_input::handle_mac_paper_pointer(
        &mut state, click, page
    ));
    assert!(state.mac_newspaper.detail_open);
    let age = state
        .city
        .mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET)
        .unwrap() as i32;
    let mut expected = MacToolboxRng::new(crate::mac_newspaper::story_seed(age, 2032, 0, 28));
    expected.random_u16();
    assert_eq!(state.mac_process_rng.seed(), expected.seed());
    state.mac_newspaper.article = Some("A long newspaper article. ".repeat(200));
    assert!(state.scroll_mac_paper_detail(5));
    assert_eq!(state.mac_newspaper.detail_scroll, 5);
    assert!(state.scroll_mac_paper_detail(i32::MAX));
    assert_eq!(
        state.mac_newspaper.detail_scroll,
        newspaper::detail_scroll_max(state.mac_newspaper.article.as_deref().unwrap())
    );
    let close = newspaper::detail_close_rect(page);
    let click = glam::Vec2::new(close.x + close.w * 0.5, close.y + close.h * 0.5);
    assert!(super::super::dialog_input::handle_mac_paper_pointer(
        &mut state, click, page
    ));
    assert!(!state.mac_newspaper.detail_open);
    assert!(state.city_info_dialog.is_some());
    state.close_city_info_dialog();
    assert!(!state.mac_newspaper.detail_open);
    assert_eq!(state.mac_newspaper.headline, None);
    assert_eq!(state.mac_newspaper.article, None);
}

#[test]
fn mac_open_uses_filename_instead_of_embedded_cnam() {
    let root = std::env::temp_dir().join(format!("citygame-mac-open-name-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("Twenty-character-city-name.city");
    let mut saved = City::new_blank(&path, "Embedded Name").unwrap();
    saved.kind = CityFileKind::MacCity;
    saved.save(&path).unwrap();
    let mut state = test_state(City::new_blank("/tmp/before-name.sc2", "Before").unwrap());
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.load_city(&path, (640, 480), 1.0).unwrap();
    assert_eq!(state.city.name, "Twenty-character-cit");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn mac_save_as_updates_city_name_only_after_success_and_reloads_it() {
    let root = std::env::temp_dir().join(format!("citygame-mac-save-name-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("River Town");
    let mut city = City::new_blank(&path, "Old City").unwrap();
    city.kind = CityFileKind::MacCity;
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    assert!(
        state
            .save_city(&root.join("missing").join("Failed"), (640, 480), 1.0)
            .is_err()
    );
    assert_eq!(state.city.name, "Old City");
    state.save_city(&path, (640, 480), 1.0).unwrap();
    assert_eq!(state.city.name, "River Town");
    state.city.name = "Changed".to_owned();
    state.load_city(&path, (640, 480), 1.0).unwrap();
    assert_eq!(state.city.name, "River Town");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn win95_save_as_uses_filename_and_preserves_name_on_failure() {
    let root = std::env::temp_dir().join(format!("citygame-win-save-name-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut state = test_state(City::new_blank(root.join("old.sc2"), "Café - ?").unwrap());
    state.engine_fidelity = EngineFidelity::Win95;
    assert!(
        state
            .save_city_as(&root.join("missing/Failed.sc2"), (640, 480), 1.0)
            .is_err()
    );
    assert_eq!(state.city.name, "Café - ?");
    let path = root.join("citygame-oem.sc2");
    state.save_city_as(&path, (640, 480), 1.0).unwrap();
    assert_eq!(state.city.name, "citygame-oem");
    assert_eq!(City::load(&path).unwrap().name, "citygame-oem");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn mac_manual_newspaper_opens_and_holds_simulation_unless_inactive() {
    for suppressed in [false, true] {
        let city = City::new_blank("/tmp/mac-paper-manual.sc2", "Test").unwrap();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Mac68k;
        state.mode = AppMode::CityView;
        state.menu_checks.citygame_newspaper = false;
        state.city.set_mac_newspaper_suppressed(suppressed);
        state.apply_state_app_command(&AppCommand::OpenCityInfo(CityInfoDialog::Newspaper(
            NewspaperEdition::Courier,
        )));
        assert_eq!(
            state.city_info_dialog,
            (!suppressed).then_some(CityInfoDialog::Newspaper(NewspaperEdition::MacStandard))
        );
        assert_eq!(
            state
                .city_info_dialog
                .is_some_and(CityInfoDialog::holds_mac_simulation),
            !suppressed
        );
    }
}

#[test]
fn mac_subscription_paper_opens_only_in_april_and_august_when_active() {
    for month in [3, 4, 8, 9] {
        for suppressed in [false, true] {
            let city = City::new_blank("/tmp/mac-paper-subscription.sc2", "Test").unwrap();
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Mac68k;
            state.city.toggle_mac_newspaper_subscription();
            state.city.set_mac_newspaper_suppressed(suppressed);
            state.finish_sim_calendar_day0_publication(1900, month, 0);
            let expected = matches!(month, 4 | 8) && !suppressed;
            assert_eq!(
                state.city_info_dialog,
                expected.then_some(CityInfoDialog::Newspaper(NewspaperEdition::MacStandard)),
                "month {month} suppressed {suppressed}"
            );
        }
    }
}

#[test]
fn mac_standard_paper_uses_saved_page_but_disaster_uses_page_zero() {
    let city = City::new_blank("/tmp/mac-paper-choice.sc2", "River").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_newspaper.tick_word = Some(2032);
    let mut starts = vec![0; 46];
    let mut counts = vec![0; 46];
    starts[44..46].copy_from_slice(&1u16.to_be_bytes());
    counts[44..46].copy_from_slice(&1u16.to_be_bytes());
    state.mac_newspaper.data = Some(std::sync::Arc::new(
        crate::mac_newspaper::MacPaperTokens::from_resources(
            &starts,
            &counts,
            &[0; 8],
            b"headline+ body\0",
        )
        .unwrap(),
    ));
    state.city.queue_mac_newspaper_story(22, 0);
    state
        .city
        .set_mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET, 1);
    let age = state
        .city
        .mac_state_misc_u32_at(MISC_CITY_AGE_OFFSET)
        .unwrap() as i32;
    for (edition, selected_page) in [
        (NewspaperEdition::MacStandard, 1),
        (NewspaperEdition::MacStoryWindow, 1),
        (NewspaperEdition::MacDisaster, 0),
    ] {
        state.open_city_info_dialog(CityInfoDialog::Newspaper(edition));
        state.open_mac_paper_detail();
        assert!(state.mac_newspaper.detail_open);
        let mut expected = MacToolboxRng::new(crate::mac_newspaper::story_seed(
            age,
            2032,
            selected_page,
            28,
        ));
        expected.random_u16();
        assert_eq!(state.mac_process_rng.seed(), expected.seed());
        state.close_city_info_dialog();
    }
}

#[test]
#[ignore = "requires the private Quadra newspaper interaction capture"]
fn mac_chronicle_detail_matches_live_quadra_click() {
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_MAC_PAPER_LIVE").unwrap());
    let ram = std::fs::read(root.join("chronicle.ram")).unwrap();
    let read32 = |p: usize| u32::from_be_bytes(ram[p..p + 4].try_into().unwrap()) as usize;
    let a5 = read32(0x904);
    let table = |displacement: usize, size: usize| {
        let p = read32(a5 - displacement) & 0xffffff;
        &ram[p..p + size]
    };
    let tokens = crate::mac_newspaper::MacPaperTokens::from_resources(
        table(0x36e4, 500),
        table(0x36e0, 500),
        table(0x36dc, 10000),
        table(0x36d8, 149362),
    )
    .unwrap();
    let city = City::new_blank("/tmp/mac-paper-live.sc2", "Before").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Mac68k;
    state.mac_newspaper.tick_word = Some(i16::from_be_bytes(
        ram[a5 - 0x36cc..a5 - 0x36ca].try_into().unwrap(),
    ));
    state.mac_newspaper.data = Some(std::sync::Arc::new(tokens));
    state
        .load_city(
            &root.join("../manual-fire-protected-full/fireguard"),
            (640, 480),
            1.0,
        )
        .unwrap();
    state
        .city
        .set_mac_state_misc_u32_at(MISC_NEWSPAPER_CHOICE_OFFSET, 1);
    state.open_city_info_dialog(CityInfoDialog::Newspaper(NewspaperEdition::MacStandard));
    let expected_headline = std::fs::read(root.join("native-page1-headline-0.bin")).unwrap();
    let expected_body = std::fs::read(root.join("native-page1-body-0.bin")).unwrap();
    assert_eq!(
        state.mac_newspaper.headline.as_deref(),
        Some(crate::city::mac_roman::decode(&expected_headline).as_str())
    );
    state.open_mac_paper_detail();
    assert_eq!(
        state.mac_newspaper.article.as_deref(),
        Some(crate::city::mac_roman::decode(&expected_body).as_str())
    );
    let after = std::fs::read(root.join("chronicle-detail.ram")).unwrap();
    assert!(
        after
            .windows(expected_body.len())
            .any(|bytes| bytes == expected_body),
        "native TextEdit buffer"
    );
    let port = u32::from_be_bytes(after[a5..a5 + 4].try_into().unwrap()) as usize;
    let native_seed = u32::from_be_bytes(after[port - 126..port - 122].try_into().unwrap());
    assert_eq!(state.mac_process_rng.seed(), native_seed);
}

#[test]
#[ignore = "requires private original-PE saved camera captures"]
fn win95_saved_camera_matches_live_original_views() {
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_PE_CAMERA_LIVE").unwrap());
    for (name, sx, sy, mode) in [
        ("second", 617.0, 111.0, 0_u32),
        ("zoom1", 608.0, -129.0, 1),
        ("zoom2", 590.0, -609.0, 2),
        // The PE save has already rotated its map planes; CityGame loads
        // those stored planes as its initial grid with camera rotation zero.
        ("rot1", 606.0, -617.0, 2),
    ] {
        let city = City::load(&root.join(format!("{name}.sc2"))).unwrap();
        let native = city.saved_view().unwrap();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.camera_rotation = 0;
        state.zoom = [0.25, 0.5, 1.0][mode as usize];
        state.camera = Vec2::new(
            sx - 626.0 - 16.0 * state.zoom,
            sy - 351.0 + 28.0 * state.zoom,
        );
        assert_eq!(
            state.saved_pe_view_for_window((1252, 702), 1.0),
            Some(native),
            "{name}"
        );
        let path =
            std::env::temp_dir().join(format!("citygame-camera-{name}-{}.sc2", std::process::id()));
        state.save_city(&path, (1252, 702), 1.0).unwrap();
        assert_eq!(
            City::load(&path).unwrap().saved_view(),
            Some(native),
            "{name} reload"
        );
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
#[ignore = "requires private original-PE rotation history fixture"]
fn win95_special_tiles_and_census_match_original_four_turn_saves() {
    let fixture = std::env::var_os("CITYGAME_PE_ROTATION_HISTORY_FIXTURE").unwrap();
    let rows: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    fn bytes(s: &str) -> Vec<u8> {
        s.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }
    assert_eq!(rows.len(), 4);
    let mut city = City::new_blank("/tmp/citygame-rotation-history.sc2", "history").unwrap();
    for x in 0..128 {
        for y in 0..128 {
            let i = x * 128 + y;
            city.set_building(x, y, ((i * 37 + i / 31) % 112) as u8);
            city.set_tile_bits(
                x,
                y,
                TileBits::new((((i * 13 + i / 29) & 0xfd) | ((i / 7 & 1) * 2)) as u8),
            );
        }
    }
    city.pe_rebuild_dw_tile_count_434f80();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.mode = AppMode::CityView;
    for row in rows {
        state.rotate_view(1);
        let turn = row["turn"].as_u64().unwrap();
        let path = std::env::temp_dir().join(format!(
            "citygame-history-{turn}-{}.sc2",
            std::process::id()
        ));
        state.save_city(&path, (800, 600), 1.0).unwrap();
        let saved = City::load(&path).unwrap();
        let expected_buildings = bytes(row["buildings"].as_str().unwrap());
        let expected_bits = bytes(row["bits"].as_str().unwrap());
        for x in 0..128 {
            for y in 0..128 {
                let i = x * 128 + y;
                assert_eq!(
                    saved.building_at(x, y),
                    expected_buildings[i],
                    "XBLD turn {turn} {x},{y}"
                );
                assert_eq!(
                    saved.tile_bits_at(x, y).unwrap().raw_bits(),
                    expected_bits[i],
                    "XBIT turn {turn} {x},{y}"
                );
            }
        }
        for i in 0..256 {
            assert_eq!(
                saved.misc_u32_at(0x1f0 + i * 4).unwrap() as u16,
                row["census"][i].as_u64().unwrap() as u16,
                "census turn {turn} {i}"
            );
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
#[ignore = "requires private original-PE actor rotation fixtures"]
fn win95_rotated_save_preserves_native_active_actor_records() {
    let fixture = std::env::var_os("CITYGAME_PE_ACTOR_ROTATION_FIXTURE").unwrap();
    let rows: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    fn bytes(s: &str) -> Vec<u8> {
        s.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }
    for (batch, delta, direction) in
        (0_usize..7).flat_map(|batch| [(batch, 1, "cw"), (batch, -1, "ccw")])
    {
        let row = &rows[batch * 2 + usize::from(delta < 0)];
        assert_eq!(row["direction"], direction);
        let original = bytes(row["before"].as_str().unwrap());
        let records = original
            .chunks_exact(12)
            .map(crate::win95_overlay::Win95OverlayRecord::from_bytes)
            .collect();
        let mut city = City::new_blank("/tmp/citygame-actor-source.sc2", "actors").unwrap();
        city.set_traffic_overlays_for_test(records);
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.mode = AppMode::CityView;
        state.rotate_view(delta);
        let path = std::env::temp_dir().join(format!(
            "citygame-rotated-actors-{batch}-{direction}-{}.sc2",
            std::process::id()
        ));
        state.save_city(&path, (800, 600), 1.0).unwrap();
        let saved = City::load(&path).unwrap();
        let actual: Vec<_> = saved
            .traffic_overlays()
            .unwrap()
            .iter()
            .flat_map(|r| r.to_bytes())
            .collect();
        assert_eq!(
            actual,
            bytes(row["after"].as_str().unwrap()),
            "batch {batch}"
        );
        let live: Vec<_> = state
            .city
            .traffic_overlays()
            .unwrap()
            .iter()
            .flat_map(|r| r.to_bytes())
            .collect();
        assert_eq!(live, original, "live records batch {batch}");
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn win95_saved_camera_uses_integer_center_in_odd_window() {
    let city = City::new_blank("/tmp/citygame-odd-center.sc2", "Odd center").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.mode = AppMode::CityView;
    state.zoom = 0.5;
    for x in -12..=12 {
        for y in -12..=12 {
            state.camera = Vec2::new(x as f32 + 0.25, y as f32 + 0.25);
            assert_eq!(
                state.saved_pe_view_for_window((800, 600), 1.0),
                state.saved_pe_view_for_window((801, 601), 1.0),
                "camera {x},{y}"
            );
        }
    }
}

#[test]
#[ignore = "requires private original-PE rotation captures"]
fn win95_rotation_command_matches_live_original_scroll() {
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_PE_CAMERA_LIVE").unwrap());
    let city = City::load(root.join("mx0.sc2")).unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.mode = AppMode::CityView;
    state.camera_rotation = 0;
    state.zoom = 0.25;
    state.viewport_pixel_scale = 1.0;
    state.mac_loop_scheduler.set_viewport_size((1252, 702));
    state.camera = Vec2::new(873.0 - 626.0 - 4.0, -17.0 - 351.0 + 7.0);
    for (delta, sx, sy, rotation) in [(1, 869.0, 237.0, 3), (-1, 873.0, -13.0, 0)] {
        state.rotate_view(delta);
        let native_x = 626.0 + state.camera.x + 4.0;
        let native_y = 351.0 + state.camera.y - 7.0;
        assert_eq!((native_x, native_y), (sx, sy));
        assert_eq!(state.city.pe_w_city_rotation, rotation);
    }
}

#[test]
#[ignore = "requires private original-PE rotation captures"]
fn win95_rotated_save_matches_live_original() {
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_PE_CAMERA_LIVE").unwrap());
    for (name, turn, sx, sy) in [
        ("mx1.sc2", 1, 869.0, 237.0),
        ("mx180.sc2", 2, 361.0, 235.0),
        ("mx270.sc2", 3, 373.0, -19.0),
    ] {
        let mut city = City::load(root.join("mx2.sc2")).unwrap();
        city.pe_rebuild_dw_tile_count_434f80();
        let native = City::load(root.join(name)).unwrap();
        let mut state = test_state(city);
        state.engine_fidelity = EngineFidelity::Win95;
        state.mode = AppMode::CityView;
        state.camera_rotation = turn;
        state.zoom = 0.25;
        state.camera = Vec2::new(
            sx - 626.0 - 16.0 * state.zoom,
            sy - 351.0 + 28.0 * state.zoom,
        );
        let original_tile = state.city.building_at(62, 72);
        let path = std::env::temp_dir().join(format!(
            "citygame-rotated-{turn}-{}.sc2",
            std::process::id()
        ));
        state.save_city(&path, (1252, 702), 1.0).unwrap();
        let saved = City::load(&path).unwrap();
        assert_eq!(
            state.city.building_at(62, 72),
            original_tile,
            "live city {name}"
        );
        assert_eq!(
            saved.saved_rotation(),
            native.saved_rotation(),
            "rotation {name}"
        );
        assert_eq!(saved.saved_view(), native.saved_view(), "view {name}");
        assert_eq!(
            saved.traffic_overlays(),
            native.traffic_overlays(),
            "actors {name}"
        );
        for i in 0..256 {
            assert_eq!(
                saved.misc_u32_at(0x1f0 + i * 4),
                native.misc_u32_at(0x1f0 + i * 4),
                "census {name} {i}"
            );
        }
        for x in 0..128 {
            for y in 0..128 {
                assert_eq!(
                    saved.altm_at(x, y),
                    native.altm_at(x, y),
                    "ALTM {name} {x},{y}"
                );
                assert_eq!(
                    saved.terrain_at(x, y),
                    native.terrain_at(x, y),
                    "XTER {name} {x},{y}"
                );
                assert_eq!(
                    saved.building_at(x, y),
                    native.building_at(x, y),
                    "XBLD {name} {x},{y}"
                );
                assert_eq!(
                    saved.underground_at(x, y),
                    native.underground_at(x, y),
                    "XUND {name} {x},{y}"
                );
                assert_eq!(
                    saved.zone_at(x, y).map(|v| v.raw_bits()),
                    native.zone_at(x, y).map(|v| v.raw_bits()),
                    "XZON {name} {x},{y}"
                );
                assert_eq!(
                    saved.tile_bits_at(x, y).map(|v| v.raw_bits()),
                    native.tile_bits_at(x, y).map(|v| v.raw_bits()),
                    "XBIT {name} {x},{y}"
                );
                assert_eq!(
                    saved.text_overlay_at(x, y),
                    native.text_overlay_at(x, y),
                    "XTXT {name} {x},{y}"
                );
                for (label, left, right) in [
                    (
                        "XTRF",
                        saved.traffic_density_slice(),
                        native.traffic_density_slice(),
                    ),
                    (
                        "XPLT",
                        saved.pollution_density_slice(),
                        native.pollution_density_slice(),
                    ),
                    ("XVAL", saved.land_value_slice(), native.land_value_slice()),
                    (
                        "XCRM",
                        saved.crime_density_slice(),
                        native.crime_density_slice(),
                    ),
                    (
                        "XPLC",
                        saved.police_coverage_slice(),
                        native.police_coverage_slice(),
                    ),
                    (
                        "XFIR",
                        saved.fire_coverage_slice(),
                        native.fire_coverage_slice(),
                    ),
                    (
                        "XPOP",
                        saved.population_density_slice(),
                        native.population_density_slice(),
                    ),
                    (
                        "XROG",
                        saved.rate_of_growth_slice(),
                        native.rate_of_growth_slice(),
                    ),
                ] {
                    assert_eq!(
                        left.unwrap()[x * 128 + y],
                        right.unwrap()[x * 128 + y],
                        "{label} {name} {x},{y}"
                    );
                }
            }
        }
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
#[ignore = "requires private original-PE rotation captures"]
fn win95_view_rotation_matches_live_map_plane_transform() {
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_PE_CAMERA_LIVE").unwrap());
    let original = City::load(&root.join("mx2.sc2")).unwrap();
    for (name, turn) in [("mx1.sc2", 1), ("mx180.sc2", 2), ("mx270.sc2", 3)] {
        let turned = City::load(&root.join(name)).unwrap();
        for x in 0..128 {
            for y in 0..128 {
                let (sx, sy) =
                    super::super::view_navigation::rotate_tile_coords(x, y, (4 - turn) & 3);
                assert_eq!(
                    turned.altm_at(x, y),
                    original.altm_at(sx, sy),
                    "ALTM {x},{y}"
                );
                assert_eq!(
                    turned.terrain_at(x, y),
                    crate::tiles::rotated_terrain_id(original.terrain_at(sx, sy), turn),
                    "XTER {name} {x},{y}"
                );
                assert_eq!(
                    turned.building_at(x, y),
                    crate::tiles::rotated_building_id(original.building_at(sx, sy), turn),
                    "XBLD {name} {x},{y}"
                );
                assert_eq!(
                    turned.underground_at(x, y),
                    rotated_underground_tile(original.underground_at(sx, sy), turn),
                    "XUND {name} {x},{y}"
                );
                assert_eq!(
                    turned.zone_at(x, y).map(|z| z.raw_bits()),
                    original.zone_at(sx, sy).map(|z| z.raw_bits()),
                    "XZON {x},{y}"
                );
                assert_eq!(
                    turned.tile_bits_at(x, y).map(|b| b.raw_bits()),
                    original.tile_bits_at(sx, sy).map(|b| b.raw_bits()),
                    "XBIT {x},{y}"
                );
                assert_eq!(
                    turned.text_overlay_at(x, y),
                    original.text_overlay_at(sx, sy),
                    "XTXT {x},{y}"
                );
                for (label, left, right) in [
                    (
                        "XTRF",
                        turned.traffic_density_slice(),
                        original.traffic_density_slice(),
                    ),
                    (
                        "XPLT",
                        turned.pollution_density_slice(),
                        original.pollution_density_slice(),
                    ),
                    (
                        "XVAL",
                        turned.land_value_slice(),
                        original.land_value_slice(),
                    ),
                    (
                        "XCRM",
                        turned.crime_density_slice(),
                        original.crime_density_slice(),
                    ),
                    (
                        "XPLC",
                        turned.police_coverage_slice(),
                        original.police_coverage_slice(),
                    ),
                    (
                        "XFIR",
                        turned.fire_coverage_slice(),
                        original.fire_coverage_slice(),
                    ),
                    (
                        "XPOP",
                        turned.population_density_slice(),
                        original.population_density_slice(),
                    ),
                    (
                        "XROG",
                        turned.rate_of_growth_slice(),
                        original.rate_of_growth_slice(),
                    ),
                ] {
                    assert_eq!(
                        left.unwrap()[x * 128 + y],
                        right.unwrap()[sx * 128 + sy],
                        "{label} {name} {x},{y}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires private Win95 launch capture and original game data"]
fn win95_first_paper_matches_captured_launch_clocks() {
    let root = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_CLOCK_ROOT").unwrap());
    let inputs: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("clock-input.json")).unwrap()).unwrap();
    let data = std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
    let pack = data.parent().unwrap().parent().unwrap().to_path_buf();
    let settings_root = std::env::temp_dir().join(format!(
        "citygame-win95-first-paper-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&settings_root).unwrap();
    let settings = settings_root.join("asset-settings.json");
    std::fs::write(
        &settings,
        serde_json::json!({"schema_version": 1, "original_graphics": pack}).to_string(),
    )
    .unwrap();
    let profile = crate::asset_profile::AssetProfile::load_from(settings).unwrap();
    let (profile_data, profile_index) = profile.win95_newspaper_data_paths().unwrap();
    assert_eq!(profile_data, data.join("DATA_USA.DAT"));
    assert_eq!(profile_index, data.join("DATA_USA.IDX"));
    let bootstrap =
        std::path::PathBuf::from(std::env::var_os("CITYGAME_WIN95_BOOTSTRAP_CITY").unwrap());
    let mut view = [
        inputs["view_lfsr_tick"].as_u64().unwrap(),
        inputs["view_game_tick"].as_u64().unwrap(),
    ]
    .into_iter();
    let (lfsr, game) =
        super::super::runtime::win95_view_randoms_from_tick_source(|| view.next().unwrap());
    let (tick, crt, tables) =
        super::super::runtime::win95_document_randoms_from_tick_source(|| {
            inputs["document_tick_word"].as_i64().unwrap() as u16 as u64
        });
    let mut state = ViewerState::load_default_from_path_with_process_rng(
        bootstrap,
        Some(crt),
        Some(game),
        Some(lfsr),
        Some(tables),
    )
    .unwrap();
    // Optional client dimensions for investigating a native capture whose
    // window geometry was recorded separately. Apply the same initialization
    // as bootstrap_window before the title choice creates a new city.
    let diagnostic_view_size = std::env::var("CITYGAME_WIN95_VIEW_SIZE").ok().map(|size| {
        let (width, height) = size.split_once('x').expect("WIDTHxHEIGHT");
        (
            width.parse::<u32>().unwrap(),
            height.parse::<u32>().unwrap(),
        )
    });
    if let Some(size) = diagnostic_view_size {
        state.fit_map(size, 1.0);
        state.restore_saved_pe_view(size, 1.0);
    }
    state.pe_document_tick_word = Some(tick);
    state.configure_original_newspaper_data(&profile);
    state.pe_registered_mayor_name = inputs["win95_mayor_name"].as_str().unwrap().to_owned();
    assert!(state.pe_newspaper_data.is_some());
    state
        .new_city_from_title(
            inputs["city_name"].as_str().unwrap_or("New City").into(),
            20_000,
            1900,
            1,
            0,
            title_screen::NewCityStartFlow::GenerateAndPlay,
        )
        .unwrap();
    println!(
        "prepaint game={} lfsr={} crt={}",
        state.pe_process_game_rand.state(),
        state.pe_process_lfsr.state(),
        state.pe_process_crt.seed()
    );
    for (snapshot, plane) in [("clock-before", "before"), ("clock-after", "after")] {
        for (name, side) in [
            ("altm", 128),
            ("xter", 128),
            ("xbld", 128),
            ("xzon", 128),
            ("xund", 128),
            ("xtxt", 128),
            ("xbit", 128),
            ("xtrf", 64),
            ("xplt", 64),
            ("xval", 64),
            ("xcrm", 64),
            ("xplc", 32),
            ("xfir", 32),
            ("xpop", 32),
            ("xrog", 32),
        ] {
            let native =
                std::fs::read(root.join(format!("{snapshot}/{name}-x-major.bin"))).unwrap();
            let mut bad = 0;
            let mut first = None;
            for x in 0..side {
                for y in 0..side {
                    let i = x * side + y;
                    let (x, y) = (x * (128 / side), y * (128 / side));
                    let got: u16 = match name {
                        "altm" => {
                            let v = state.city.altm_at(x, y);
                            u16::from(v.land)
                                | (u16::from(v.water) << 5)
                                | (u16::from(v.tunnel_levels) << 10)
                        }
                        "xter" => state.city.terrain_at(x, y).into(),
                        "xbld" => state.city.building_at(x, y).into(),
                        "xzon" => state.city.zone_at(x, y).unwrap().raw_bits().into(),
                        "xund" => state.city.underground_at(x, y).into(),
                        "xtxt" => state.city.text_overlay_at(x, y).unwrap().into(),
                        "xbit" => state.city.tile_bits_at(x, y).unwrap().raw_bits().into(),
                        "xtrf" => state.city.traffic_density_at(x, y).unwrap().into(),
                        "xplt" => state.city.pollution_density_at(x, y).unwrap().into(),
                        "xval" => state.city.land_value_at(x, y).unwrap().into(),
                        "xcrm" => state.city.crime_density_at(x, y).unwrap().into(),
                        "xplc" => state.city.police_coverage_at(x, y).unwrap().into(),
                        "xfir" => state.city.fire_coverage_at(x, y).unwrap().into(),
                        "xpop" => state.city.population_density_at(x, y).unwrap().into(),
                        "xrog" => state.city.rate_of_growth_at(x, y).unwrap().into(),
                        _ => unreachable!(),
                    };
                    let expected = if name == "altm" {
                        u16::from_le_bytes([native[2 * i], native[2 * i + 1]])
                    } else {
                        native[i].into()
                    };
                    if got != expected {
                        bad += 1;
                        if first.is_none() {
                            first = Some((x, y, got, expected));
                        }
                    }
                }
            }
            println!("MAP {plane} {name} {bad} {first:?}");
            assert_eq!(bad, 0, "{plane} {name}: {first:?}");
        }
        let native_actors = std::fs::read(root.join(format!("{snapshot}/xthg-records.bin")))
            .expect("missing native XTHG capture");
        let actors = crate::win95_overlay::encode_xthg_records(
            state
                .city
                .traffic_overlays()
                .expect("missing CityGame XTHG"),
        )
        .unwrap();
        assert_eq!(actors, native_actors, "{plane} XTHG");
    }
    let before = state.city.pe_newspaper_tables().unwrap();
    println!("PAPERS {:?}", before.papers);
    let paper: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("clock-newspaper.json")).unwrap()).unwrap();
    let unhex = |s: &str| -> Vec<u8> {
        s.as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect()
    };
    let native_papers = unhex(paper["paper_bytes"].as_str().unwrap());
    assert_eq!(
        before.papers.as_slice(),
        native_papers
            .iter()
            .map(|v| u32::from(*v))
            .collect::<Vec<_>>()
    );
    state.prepare_pe_newspaper_first_paint();
    let after = state.city.pe_newspaper_tables().unwrap();
    println!("STORIES {:?}", after.stories);
    println!(
        "FINAL game={} lfsr={} crt={} painted={}",
        state.pe_process_game_rand.state(),
        state.pe_process_lfsr.state(),
        state.pe_process_crt.seed(),
        state.pe_newspaper_paint.is_some()
    );
    let native_story = unhex(paper["story_bytes"].as_str().unwrap());
    let story: Vec<u32> = native_story
        .chunks_exact(8)
        .flat_map(|r| {
            [
                u16::from_le_bytes([r[0], r[1]]) as u32,
                u16::from_le_bytes([r[2], r[3]]) as u32,
                r[4] as u32,
                r[5] as u32,
                r[6] as u32,
                r[7] as u32,
            ]
        })
        .collect();
    assert_eq!(after.stories.as_slice(), story);
    let native: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("clock-after/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(native["crt_tls_candidates"].as_array().unwrap().len(), 1);
    assert_eq!(
        state.pe_process_crt.seed() as u64,
        native["crt_tls_candidates"][0]["holdrand"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(
        state.pe_process_game_rand.state() as u64,
        native["game_lcg_raw"].as_u64().unwrap()
    );
    assert_eq!(
        state.pe_process_lfsr.state() as u64,
        native["lfsr_raw"].as_u64().unwrap()
    );
    assert!(state.pe_newspaper_paint.is_some());
    if let Some(expected) = inputs["expected_city_name"].as_str() {
        assert_eq!(state.city.name, expected);
    }
    if let Some(expected) = inputs["expected_headline"].as_str() {
        assert_eq!(
            crate::win95_oem::decode_oem(&state.pe_newspaper_paint.as_ref().unwrap().lead.headline),
            expected
        );
    }
    if let Some(position) = inputs["article_position"].as_u64() {
        let page = newspaper::page_rect((1280, 800));
        let style = state.pe_newspaper_paint.as_ref().unwrap().style;
        let rect = newspaper::pe_story_rect(page, style, position as u16).unwrap();
        let point = Vec2::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        assert!(super::super::dialog_input::handle_pe_paper_pointer(
            &mut state, point, page
        ));
        let article = state.pe_newspaper_article.as_ref().unwrap();
        let bytes = format!(
            "{} -\n{}",
            article.headline,
            article.body.replace('\r', "\n")
        );
        assert_eq!(
            bytes.as_bytes(),
            std::fs::read(root.join("native-article.bin")).unwrap()
        );
        let original: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("article-open/manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            state.pe_process_crt.seed() as u64,
            original["crt_tls_candidates"][0]["holdrand"]
                .as_u64()
                .unwrap()
        );
        assert_eq!(
            state.pe_process_game_rand.state() as u64,
            original["game_lcg_raw"].as_u64().unwrap()
        );
        assert_eq!(
            state.pe_process_lfsr.state() as u64,
            original["lfsr_raw"].as_u64().unwrap()
        );
        let close = newspaper::pe_detail_close_rect(page, &article.body);
        let point = Vec2::new(close.x + close.w / 2.0, close.y + close.h / 2.0);
        assert!(super::super::dialog_input::handle_pe_paper_pointer(
            &mut state, point, page
        ));
        assert!(state.pe_newspaper_article.is_none());
        let closed: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("article-closed/manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            state.pe_process_crt.seed() as u64,
            closed["crt_tls_candidates"][0]["holdrand"]
                .as_u64()
                .unwrap()
        );
        assert_eq!(
            state.pe_process_game_rand.state() as u64,
            closed["game_lcg_raw"].as_u64().unwrap()
        );
        assert_eq!(
            state.pe_process_lfsr.state() as u64,
            closed["lfsr_raw"].as_u64().unwrap()
        );
    }
    if let Some(path) = std::env::var_os("CITYGAME_WIN95_FIRST_ARTICLE_WORKER") {
        // Original x86 article workers replayed from this paper's captured
        // story row; CityGame reaches the click without a state checkpoint.
        let expected: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let page = newspaper::page_rect((1280, 800));
        let style = state.pe_newspaper_paint.as_ref().unwrap().style;
        let rect = newspaper::pe_story_rect(page, style, 4).unwrap();
        let point = Vec2::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        assert!(super::super::dialog_input::handle_pe_paper_pointer(
            &mut state, point, page
        ));
        let article = state.pe_newspaper_article.as_ref().unwrap();
        let body = article.body.replace('\r', "\n");
        let fnv = |bytes: &[u8]| {
            bytes.iter().fold(0xcbf29ce484222325_u64, |h, byte| {
                (h ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            })
        };
        for (text, native) in [(&article.headline, &expected), (&body, &expected["body"])] {
            assert!(text.is_ascii());
            assert_eq!(text.len() as u64, native["output_len"].as_u64().unwrap());
            assert_eq!(
                fnv(text.as_bytes()),
                native["output_fnv64"].as_u64().unwrap()
            );
        }
        assert_eq!(
            state.pe_process_crt.seed() as u64,
            expected["body"]["final_crt"].as_u64().unwrap()
        );
    }
    let original_save = root.join("native-saved.sc2");
    if original_save.exists() {
        let original = City::load(original_save).unwrap();
        let page = newspaper::page_rect((1280, 800));
        let close = newspaper::pe_close_rect(page);
        assert!(super::super::dialog_input::handle_pe_paper_pointer(
            &mut state,
            Vec2::new(close.x + close.w / 2.0, close.y + close.h / 2.0),
            page
        ));
        assert!(state.city_info_dialog.is_none());
        let folder =
            std::env::temp_dir().join(format!("citygame-oem-launch-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("citygame-oem.sc2");
        state
            .save_city_as(&path, diagnostic_view_size.unwrap_or((1280, 800)), 1.0)
            .unwrap();
        assert_eq!(state.city.name, original.name);
        let reloaded = City::load(&path).unwrap();
        if let Some(output) = std::env::var_os("CITYGAME_WIN95_SAVE_OUTPUT") {
            std::fs::copy(&path, output).unwrap();
        }
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(folder).unwrap();
        assert_eq!(reloaded.name, original.name);
        assert_eq!(
            state.city.chunk_u32_values("XGRP", 0, 16 * 52),
            original.chunk_u32_values("XGRP", 0, 16 * 52),
            "CityGame XGRP vs native save"
        );
        assert_eq!(
            reloaded.chunk_u32_values("XGRP", 0, 16 * 52),
            original.chunk_u32_values("XGRP", 0, 16 * 52),
            "saved XGRP history"
        );
        for row in 0..16 {
            let offset = 0x77c + row * 27 * 4;
            let native = original.budget_misc_u32_values(offset, 5);
            assert_eq!(
                state.city.budget_misc_u32_values(offset, 5),
                native,
                "new-city budget row {row}"
            );
            assert_eq!(
                reloaded.budget_misc_u32_values(offset, 5),
                native,
                "saved budget row {row}"
            );
        }
        assert_eq!(
            reloaded.misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET),
            original.misc_u32_at(MISC_MONSTER_XTHG_INDEX_OFFSET),
            "saved monster slot"
        );
        assert_eq!(
            reloaded.misc_u32_at(MISC_SPEED_OFFSET),
            original.misc_u32_at(MISC_SPEED_OFFSET),
            "saved speed"
        );
        for offset in [SOUND_EFFECTS_MISC_OFFSET, MUSIC_MISC_OFFSET] {
            assert_eq!(
                reloaded.misc_u32_at(offset),
                original.misc_u32_at(offset),
                "saved audio option at {offset:#x}"
            );
        }
        if diagnostic_view_size.is_some() {
            for offset in [0x1010, 0x1014] {
                assert_eq!(
                    reloaded.misc_u32_at(offset),
                    original.misc_u32_at(offset),
                    "saved view at {offset:#x}"
                );
            }
        }
        let native = &original.chunk("CNAM").unwrap().data;
        let ours = &reloaded.chunk("CNAM").unwrap().data;
        let end = native[1..].iter().position(|&byte| byte == 0).unwrap() + 2;
        assert_eq!(&ours[..end], &native[..end]);

        assert_eq!(
            reloaded.misc_u32_values(
                MISC_NEWSPAPER_STORY_LIST_OFFSET,
                NEWSPAPER_STORY_LIST_MISC_DWORDS
            ),
            original.misc_u32_values(
                MISC_NEWSPAPER_STORY_LIST_OFFSET,
                NEWSPAPER_STORY_LIST_MISC_DWORDS
            ),
            "saved story queue after the founding paper closes"
        );
    }
    std::fs::remove_dir_all(settings_root).unwrap();
}

#[test]
fn win95_paper_lead_click_and_detail_controls_preserve_the_paper() {
    let city = City::new_blank("/tmp/win95-paper-detail.sc2", "Paper").unwrap();
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.city_info_dialog = Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding));
    let page = newspaper::page_rect((1280, 800));
    let lead = newspaper::lead_hit_rect(page);
    let point = Vec2::new(lead.x + lead.w / 2.0, lead.y + lead.h / 2.0);
    let crt = state.pe_process_crt.seed();
    // Missing original DATA is a safe no-op, not a fabricated article.
    assert!(super::super::dialog_input::handle_pe_paper_pointer(
        &mut state, point, page
    ));
    assert!(state.pe_newspaper_article.is_none());
    assert_eq!(state.pe_process_crt.seed(), crt);

    state.pe_newspaper_article = Some(super::super::newspaper_state::PePaperArticle {
        headline: "Headline".into(),
        body: "A long paragraph. ".repeat(300),
        scroll: 0,
    });
    let down = newspaper::pe_detail_scroll_rect(
        page,
        &state.pe_newspaper_article.as_ref().unwrap().body,
        true,
    );
    assert!(super::super::dialog_input::handle_pe_paper_pointer(
        &mut state,
        Vec2::new(down.x + 1.0, down.y + 1.0),
        page,
    ));
    assert!(state.pe_newspaper_article.as_ref().unwrap().scroll > 0);
    let close =
        newspaper::pe_detail_close_rect(page, &state.pe_newspaper_article.as_ref().unwrap().body);
    assert!(super::super::dialog_input::handle_pe_paper_pointer(
        &mut state,
        Vec2::new(close.x + 1.0, close.y + 1.0),
        page,
    ));
    assert!(state.pe_newspaper_article.is_none());
    assert_eq!(
        state.city_info_dialog,
        Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding))
    );
}

#[test]
#[ignore = "requires original Win95 DATA and private captured paper rows"]
fn win95_recovered_paper_dispatches_all_visible_article_positions() {
    use crate::win95_newspaper::{
        PeHeadlineContext, PeLeadArticle, PeNewspaperData, PeNewspaperPaint, PeNewspaperSeed,
    };
    let root = PathBuf::from(std::env::var_os("CITYGAME_PE_ARTICLE_ROOT").unwrap());
    let data_dir = PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("before2.json")).unwrap()).unwrap();
    let decode = |text: &str| -> Vec<u8> {
        text.as_bytes()
            .chunks_exact(2)
            .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
            .collect()
    };
    let papers = decode(source["paper_bytes"].as_str().unwrap());
    let stories = decode(source["story_bytes"].as_str().unwrap());
    let data = std::sync::Arc::new(
        PeNewspaperData::load(
            &data_dir.join("data_usa.dat"),
            &data_dir.join("data_usa.idx"),
        )
        .unwrap(),
    );
    let mut successful = 0;
    for style in 0..3_u8 {
        for position in 4..=10_u16 {
            let mut tables = crate::city::PeNewspaperTables {
                papers: [0; 30],
                stories: [0; 54],
            };
            for (i, value) in papers.iter().enumerate() {
                tables.papers[i] = u32::from(*value);
            }
            tables.papers[1] = u32::from(style);
            for (i, row) in stories.chunks_exact(8).enumerate() {
                let fields = [
                    u16::from_le_bytes([row[0], row[1]]) as u32,
                    u16::from_le_bytes([row[2], row[3]]) as u32,
                    row[4] as u32,
                    row[5] as u32,
                    row[6] as u32,
                    row[7] as u32,
                ];
                tables.stories[i * 6..i * 6 + 6].copy_from_slice(&fields);
            }
            let mut city = City::new_blank("/tmp/win95-paper-positions.sc2", "New City").unwrap();
            city.install_pe_newspaper_tables(tables);
            city.set_label(0, "SC Trace");
            for label in 251..=255 {
                city.set_label(label, "Neighbor");
            }
            let mut state = test_state(city);
            state.engine_fidelity = EngineFidelity::Win95;
            state.city_info_dialog = Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding));
            state.pe_document_tick_word = Some(21382);
            state.pe_newspaper_data = Some(data.clone());
            state.pe_process_crt = crate::win95_random::Win95CrtRand::new(0x12345678);
            state.pe_newspaper_paint = Some(PeNewspaperPaint {
                style,
                lead: PeLeadArticle {
                    headline: vec![],
                    picture: 0,
                    columns: vec![],
                    bars: vec![],
                },
                secondary: vec![],
                tail: vec![],
            });
            let label_storage: Vec<Option<Vec<u8>>> = (0..=u8::MAX)
                .map(|i| state.city.label_at(i).map(|v| v.as_bytes().to_vec()))
                .collect();
            let labels: Vec<Option<&[u8]>> = label_storage.iter().map(|v| v.as_deref()).collect();
            let context = PeHeadlineContext {
                city_name_oem: b"New City",
                labels_oem: &labels,
                mutable_state: None,
            };
            let mut expected_city = state.city.clone();
            let mut expected_crt = state.pe_process_crt;
            let expected = data.render_city_article_ascii(
                &mut expected_city,
                style,
                PeNewspaperSeed {
                    days: 0,
                    startup_tick: 21382,
                    paper: 0,
                    slot: position as i16,
                },
                &mut expected_crt,
                Some(&context),
            );
            let page = newspaper::page_rect((1280, 800));
            let rect = newspaper::pe_story_rect(page, style, position).unwrap();
            let point = Vec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
            assert!(super::super::dialog_input::handle_pe_paper_pointer(
                &mut state, point, page
            ));
            match expected {
                Some((headline, body)) => {
                    successful += 1;
                    let article = state
                        .pe_newspaper_article
                        .as_ref()
                        .expect("click opens article");
                    assert_eq!(
                        article.headline.as_bytes(),
                        headline,
                        "style={style} position={position}"
                    );
                    assert_eq!(
                        article.body.replace('\r', "\n").as_bytes(),
                        body,
                        "style={style} position={position}"
                    );
                    assert_eq!(state.pe_process_crt.seed(), expected_crt.seed());
                    assert_eq!(
                        state.city.pe_newspaper_tables().unwrap().stories,
                        expected_city.pe_newspaper_tables().unwrap().stories
                    );
                }
                None => {
                    assert!(
                        state.pe_newspaper_article.is_none(),
                        "style={style} position={position}"
                    );
                    assert_eq!(state.pe_process_crt.seed(), 0x12345678);
                }
            }
        }
    }
    assert!(successful >= 12, "only {successful} supported articles");
}

#[test]
#[ignore = "requires private Win95 secondary-article capture and original DATA"]
fn win95_secondary_article_and_close_match_live_capture() {
    use crate::win95_newspaper::{PeLeadArticle, PeNewspaperData, PeNewspaperPaint};
    let root = PathBuf::from(std::env::var_os("CITYGAME_PE_REPAINT_ROOT").unwrap());
    let data_root = PathBuf::from(std::env::var_os("CITYGAME_WIN95_DATA_DIR").unwrap());
    let json = |path: &str| -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(root.join(path)).unwrap()).unwrap()
    };
    let before = json("paper2.json");
    let close = json("closed2.json");
    let before_manifest = json("paper2/manifest.json");
    let close_manifest = json("closed2/manifest.json");
    let decode = |text: &str| -> Vec<u8> {
        text.as_bytes()
            .chunks_exact(2)
            .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
            .collect()
    };
    let mut tables = crate::city::PeNewspaperTables {
        papers: [0; 30],
        stories: [0; 54],
    };
    for (i, value) in decode(before["paper_bytes"].as_str().unwrap())
        .into_iter()
        .enumerate()
    {
        tables.papers[i] = u32::from(value);
    }
    for (i, row) in decode(before["story_bytes"].as_str().unwrap())
        .chunks_exact(8)
        .enumerate()
    {
        tables.stories[i * 6..i * 6 + 6].copy_from_slice(&[
            u16::from_le_bytes([row[0], row[1]]) as u32,
            u16::from_le_bytes([row[2], row[3]]) as u32,
            row[4] as u32,
            row[5] as u32,
            row[6] as u32,
            row[7] as u32,
        ]);
    }
    let expected_stories = tables.stories;
    let mut city = City::new_blank("/tmp/win95-secondary-live.sc2", "New City").unwrap();
    city.install_pe_newspaper_tables(tables);
    city.set_label(0, "SC Trace");
    let mut state = test_state(city);
    state.engine_fidelity = EngineFidelity::Win95;
    state.city_info_dialog = Some(CityInfoDialog::Newspaper(NewspaperEdition::Founding));
    state.pe_document_tick_word = Some(before["document_tick"].as_i64().unwrap() as i16);
    state.pe_newspaper_data = Some(std::sync::Arc::new(
        PeNewspaperData::load(
            &data_root.join("data_usa.dat"),
            &data_root.join("data_usa.idx"),
        )
        .unwrap(),
    ));
    state.pe_process_crt = Win95CrtRand::new(
        before_manifest["crt_tls_candidates"][0]["holdrand"]
            .as_u64()
            .unwrap() as u32,
    );
    state.pe_newspaper_paint = Some(PeNewspaperPaint {
        style: 1,
        lead: PeLeadArticle {
            headline: vec![],
            picture: 404,
            columns: vec![],
            bars: vec![],
        },
        secondary: vec![],
        tail: vec![],
    });
    state.pe_newspaper_paint_attempted = true;
    let page = newspaper::page_rect((1280, 800));
    let story = newspaper::pe_story_rect(page, 1, 10).unwrap();
    assert!(super::super::dialog_input::handle_pe_paper_pointer(
        &mut state,
        Vec2::new(story.x + story.w * 0.5, story.y + story.h * 0.5),
        page
    ));
    let article = state.pe_newspaper_article.as_ref().unwrap();
    let actual = format!(
        "{} -\n{}",
        article.headline,
        article.body.replace('\r', "\n")
    );
    assert_eq!(
        actual.as_bytes(),
        std::fs::read(root.join("secondary-article.bin")).unwrap()
    );
    let native_crt = close_manifest["crt_tls_candidates"][0]["holdrand"]
        .as_u64()
        .unwrap() as u32;
    assert_eq!(state.pe_process_crt.seed(), native_crt);
    let close_rect =
        newspaper::pe_detail_close_rect(page, &state.pe_newspaper_article.as_ref().unwrap().body);
    assert!(super::super::dialog_input::handle_pe_paper_pointer(
        &mut state,
        Vec2::new(
            close_rect.x + close_rect.w * 0.5,
            close_rect.y + close_rect.h * 0.5
        ),
        page
    ));
    assert!(state.pe_newspaper_article.is_none());
    state.prepare_pe_newspaper_first_paint();
    assert_eq!(state.pe_process_crt.seed(), native_crt);
    assert_eq!(
        state.city.pe_newspaper_tables().unwrap().stories,
        expected_stories
    );
    assert_eq!(before["paper_bytes"], close["paper_bytes"]);
    assert_eq!(before["story_bytes"], close["story_bytes"]);
    for key in ["age", "document_tick", "game_lcg", "lfsr"] {
        assert_eq!(before[key], close[key]);
    }
}
