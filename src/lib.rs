// Shared library used by the desktop binary and by iOS/Android hosts (staticlib/cdylib).
// Many helpers stay compiled for PE fidelity tests even when unused in a given mode.
#![allow(dead_code)]

mod app;
mod app_icon;
mod asset_browser;
mod asset_catalog;
mod asset_profile;
mod assets;
mod audio;
mod audio_import;
mod city;
mod data_map;
mod docked_palette;
#[cfg(any(target_os = "ios", test))]
mod document_files;
mod dos_import;
mod engine_fidelity;
mod game_clock;
#[cfg(target_os = "ios")]
mod ios_documents;
mod mac_import;
mod mac_scenario;
mod mac_traffic_geometry;
// Retained as a test-only archaeology model; the incomplete Mac actor lifecycle is not a
// supported road-traffic renderer.
#[cfg(target_os = "macos")]
mod mac_folder_picker;
#[cfg(test)]
mod mac_live_traffic;
mod mac_menu;
mod mac_random;
mod mac_render;
mod mac_shape_table;
mod menu_model;
mod menubar;
mod mobile_graphics;
mod mobile_lifecycle;
mod newspaper_font;
mod palette;
mod palette_window;
mod paths;
mod platform_ui;
mod renderer;
mod tiles;
mod title_music;
mod touch_input;
mod traffic;
mod trip_generator;
mod ui;
mod win95_import;
mod win95_overlay;
mod win95_overlay_sim;
mod win95_random;
mod win95_tile_draw;
mod win95_traffic_placement;

/// Run the game (desktop binary and mobile `SDL_main` both call this).
pub fn run() -> anyhow::Result<()> {
    app::run()
}

/// CLI helpers used only by the desktop binary.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod desktop_cli {
    use super::*;
    use std::ffi::OsString;
    use std::path::Path;

    pub fn maybe_run_cli(args: Vec<OsString>) -> anyhow::Result<bool> {
        if args.first().and_then(|arg| arg.to_str()) == Some("--probe-mac-rci-candidates") {
            anyhow::ensure!(
                args.len() >= 10,
                "usage: --probe-mac-rci-candidates R C I PREVIOUS_R CURRENT_SCALAR INDUSTRY_MOD DIFFICULTY CAP_MOD_1 CAP_MOD_2 [--finalize TAX_R TAX_C TAX_I PREV_R PREV_C PREV_I] [XBLD=COUNT ...]"
            );
            let parse_i32 = |index: usize| -> anyhow::Result<i32> {
                Ok(args[index]
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("argument {} is not UTF-8", index + 1))?
                    .parse()?)
            };
            let finalize = args.get(10).and_then(|arg| arg.to_str()) == Some("--finalize");
            if finalize {
                anyhow::ensure!(args.len() >= 17, "--finalize requires six WORD values");
            }
            let mut xbld_counts = [0u16; 256];
            for argument in &args[if finalize { 17 } else { 10 }..] {
                let value = argument
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("XBLD argument is not UTF-8"))?;
                let (tile, count) = value
                    .split_once('=')
                    .ok_or_else(|| anyhow::anyhow!("expected XBLD=COUNT, got {value}"))?;
                let tile: usize = tile.parse()?;
                let count: u16 = count.parse()?;
                anyhow::ensure!(tile < xbld_counts.len(), "XBLD tile ID must be 0..255");
                xbld_counts[tile] = count;
            }
            let difficulty: usize = args[7]
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("difficulty is not UTF-8"))?
                .parse()?;
            let pair_totals = [parse_i32(1)?, parse_i32(2)?, parse_i32(3)?];
            if finalize {
                anyhow::ensure!(
                    pair_totals.iter().all(|&total| total >= 0),
                    "finalizer probe currently requires nonnegative pair totals"
                );
            }
            let candidates = super::app::mac_rci_candidate_probe(
                pair_totals,
                parse_i32(4)?,
                parse_i32(5)?,
                parse_i32(6)?.try_into()?,
                difficulty,
                &xbld_counts,
                args[8]
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("cap modifier 1 is not UTF-8"))?
                    .parse::<u16>()?,
                args[9]
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("cap modifier 2 is not UTF-8"))?
                    .parse::<u16>()?,
            );
            println!(
                "mac_rci_candidate_bits={:#010x},{:#010x},{:#010x}",
                candidates[0], candidates[1], candidates[2]
            );
            println!(
                "mac_rci_candidate_values={},{},{}",
                f32::from_bits(candidates[0]),
                f32::from_bits(candidates[1]),
                f32::from_bits(candidates[2])
            );
            if finalize {
                let parse_i16 = |index: usize| -> anyhow::Result<i16> {
                    Ok(args[index]
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("argument {} is not UTF-8", index + 1))?
                        .parse()?)
                };
                let finalized = super::app::mac_rci_candidate_finalizer_probe(
                    pair_totals,
                    parse_i32(4)?,
                    parse_i32(5)?,
                    parse_i32(6)?.try_into()?,
                    difficulty,
                    &xbld_counts,
                    args[8]
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("cap modifier 1 is not UTF-8"))?
                        .parse::<u16>()?,
                    args[9]
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("cap modifier 2 is not UTF-8"))?
                        .parse::<u16>()?,
                    [parse_i16(11)?, parse_i16(12)?, parse_i16(13)?],
                    [parse_i16(14)?, parse_i16(15)?, parse_i16(16)?],
                )?;
                println!(
                    "mac_rci_finalized_demand_words={},{},{}",
                    finalized[0], finalized[1], finalized[2]
                );
            }
            return Ok(true);
        }
        if args.first().and_then(|arg| arg.to_str()) == Some("--export-mac-scan-state") {
            anyhow::ensure!(
                args.len() == 3,
                "usage: --export-mac-scan-state MAC_CITY OUTPUT.bin"
            );
            export_mac_scan_state(Path::new(&args[1]), Path::new(&args[2]), true)?;
            return Ok(true);
        }
        if args.first().and_then(|arg| arg.to_str()) == Some("--export-mac-actor-state") {
            anyhow::ensure!(
                args.len() == 3,
                "usage: --export-mac-actor-state CITY_FILE OUTPUT.bin"
            );
            export_mac_scan_state(Path::new(&args[1]), Path::new(&args[2]), false)?;
            return Ok(true);
        }
        if args.first().and_then(|arg| arg.to_str()) == Some("--export-mac-xgrp") {
            anyhow::ensure!(
                args.len() == 3,
                "usage: --export-mac-xgrp MAC_CITY OUTPUT.bin"
            );
            export_mac_xgrp(Path::new(&args[1]), Path::new(&args[2]))?;
            return Ok(true);
        }
        if args.first().and_then(|arg| arg.to_str()) == Some("--export-mac-xmic") {
            anyhow::ensure!(
                args.len() == 3,
                "usage: --export-mac-xmic MAC_CITY OUTPUT.bin"
            );
            export_mac_xmic(Path::new(&args[1]), Path::new(&args[2]))?;
            return Ok(true);
        }
        if args.first().and_then(|arg| arg.to_str()) == Some("--export-city-xmic") {
            anyhow::ensure!(
                args.len() == 3,
                "usage: --export-city-xmic CITY_FILE OUTPUT.bin"
            );
            export_city_xmic(Path::new(&args[1]), Path::new(&args[2]))?;
            return Ok(true);
        }
        if args.first().and_then(|arg| arg.to_str()) == Some("--render-title-theme") {
            anyhow::ensure!(args.len() == 2, "usage: --render-title-theme PATH.wav");
            title_music::export_citygame_theme(Path::new(&args[1]))?;
            return Ok(true);
        }
        if asset_browser::run_cli(args)? {
            return Ok(true);
        }
        Ok(false)
    }

    fn export_mac_scan_state(
        city_path: &Path,
        output_path: &Path,
        include_mac_misc: bool,
    ) -> anyhow::Result<()> {
        use std::io::Write;

        let city = super::city::City::load(city_path)?;
        if include_mac_misc {
            anyhow::ensure!(
                city.kind == super::city::CityFileKind::MacCity,
                "Mac scan-state export requires a Mac-format city file; use --export-mac-actor-state for cross-format actor input"
            );
        }
        for name in ["XTER", "XBLD", "XZON", "XBIT", "XUND", "XTXT"] {
            anyhow::ensure!(
                city.chunk(name).is_some(),
                "Mac city is missing required {name} map plane"
            );
        }

        const SIDE: usize = 128;
        const TILES: usize = SIDE * SIDE;
        const MISC_DWORDS: usize = 0x12c0 / 4;
        let mut presence = 0u32;
        let plane_names = [
            "XTER", "XBLD", "XZON", "XBIT", "XUND", "XTXT", "XVAL", "XCRM", "XPLT",
        ];
        for (bit, name) in plane_names.iter().enumerate() {
            if city.chunk(name).is_some() {
                presence |= 1 << bit;
            }
        }
        if let Some(traffic) = city.traffic_density_slice() {
            anyhow::ensure!(traffic.len() == TILES, "Mac XTRF must be 128×128 in memory");
            presence |= 1 << 10;
        }
        anyhow::ensure!(
            city.chunk("ALTM").is_some(),
            "Mac city is missing required ALTM map plane"
        );
        presence |= 1 << 9;
        let xthg = city
            .chunk("XTHG")
            .map(|chunk| {
                if chunk.data.len() == 40 * 12 {
                    Ok(chunk.data.clone())
                } else {
                    super::city::sc2_rle_decode(&chunk.data)
                }
            })
            .transpose()?;
        if let Some(xthg) = xthg.as_deref() {
            anyhow::ensure!(
                xthg.len() == 40 * 12,
                "Mac XTHG must contain 40 12-byte records, got {} bytes",
                xthg.len()
            );
            presence |= 1 << 11;
        }

        const ALTM_BYTES: usize = TILES * 2;
        const XTHG_BYTES: usize = 40 * 12;
        let scan_map_plane_count = plane_names.len() + 1;
        let mut output = Vec::with_capacity(
            20 + plane_names.len() * TILES + ALTM_BYTES + TILES + MISC_DWORDS * 4 + XTHG_BYTES,
        );
        output.write_all(b"CGMS")?;
        output.write_all(&4u16.to_be_bytes())?;
        output.write_all(&(SIDE as u16).to_be_bytes())?;
        output.write_all(&(SIDE as u16).to_be_bytes())?;
        output.write_all(&(scan_map_plane_count as u16).to_be_bytes())?;
        output.write_all(&(MISC_DWORDS as u32).to_be_bytes())?;
        output.write_all(&presence.to_be_bytes())?;

        for name in plane_names {
            for x in 0..SIDE {
                for y in 0..SIDE {
                    let value = match name {
                        "XTER" => city.terrain_at(x, y),
                        "XBLD" => city.building_at(x, y),
                        "XZON" => city.zone_at(x, y).map_or(0, |zone| zone.raw_bits()),
                        "XBIT" => city.tile_bits_at(x, y).map_or(0, |bits| bits.raw_bits()),
                        "XUND" => city.underground_at(x, y),
                        "XTXT" => city.text_overlay_at(x, y).unwrap_or(0),
                        "XVAL" => city.land_value_at(x, y).unwrap_or(0),
                        "XCRM" => city.crime_density_at(x, y).unwrap_or(0),
                        "XPLT" => city.pollution_density_at(x, y).unwrap_or(0),
                        _ => unreachable!(),
                    };
                    output.push(value);
                }
            }
        }

        // Native Mac CODE 14 power and CODE 8 terrain workers access ALTM as
        // a WORD row plane. Preserve its full packed word and X-major order.
        for x in 0..SIDE {
            for y in 0..SIDE {
                let sample = city.altm_at(x, y);
                let raw = u16::from(sample.land & 0x1f)
                    | (u16::from(sample.water & 0x1f) << 5)
                    | (u16::from(sample.tunnel_levels & 0x3f) << 10);
                output.write_all(&raw.to_be_bytes())?;
            }
        }

        if let Some(traffic) = city.traffic_density_slice() {
            output.write_all(traffic)?;
        } else {
            output.extend_from_slice(&vec![0; TILES]);
        }

        for index in 0..MISC_DWORDS {
            // The PE MISC chunk has a different ownership map. Actor-only
            // replays do not consume MISC, so keep it zero instead of
            // presenting PE fields as native Mac simulation state.
            let value = if include_mac_misc {
                city.mac_state_misc_u32_at(index * 4).unwrap_or_default()
            } else {
                0
            };
            output.write_all(&value.to_be_bytes())?;
        }
        if let Some(xthg) = xthg.as_deref() {
            output.write_all(xthg)?;
        } else {
            output.extend_from_slice(&vec![0; XTHG_BYTES]);
        }

        std::fs::write(output_path, &output)?;
        println!(
            "exported {} Mac {}-state bytes from {} (present plane mask {presence:#05x})",
            output.len(),
            if include_mac_misc { "scan" } else { "actor" },
            city_path.display()
        );
        Ok(())
    }

    fn export_mac_xgrp(city_path: &Path, output_path: &Path) -> anyhow::Result<()> {
        let city = super::city::City::load(city_path)?;
        anyhow::ensure!(
            city.kind == super::city::CityFileKind::MacCity,
            "XGRP export requires a Mac-format city file"
        );
        let chunk = city
            .chunk("XGRP")
            .ok_or_else(|| anyhow::anyhow!("city has no XGRP history chunk"))?;
        let decoded =
            super::city::sc2_rle_decode(&chunk.data).unwrap_or_else(|_| chunk.data.clone());
        anyhow::ensure!(
            decoded.len() == 0xd00,
            "expected 0xd00-byte Mac XGRP history, got {:#x}",
            decoded.len()
        );
        std::fs::write(output_path, &decoded)?;
        println!(
            "exported 0xd00-byte decoded Mac XGRP history from {}",
            city_path.display()
        );
        Ok(())
    }

    fn export_mac_xmic(city_path: &Path, output_path: &Path) -> anyhow::Result<()> {
        let city = super::city::City::load(city_path)?;
        anyhow::ensure!(
            city.kind == super::city::CityFileKind::MacCity,
            "XMIC export requires a Mac-format city file"
        );
        let chunk = city
            .chunk("XMIC")
            .ok_or_else(|| anyhow::anyhow!("Mac city has no XMIC chunk"))?;
        let decoded =
            super::city::sc2_rle_decode(&chunk.data).unwrap_or_else(|_| chunk.data.clone());
        anyhow::ensure!(
            decoded.len() == 150 * 8,
            "expected 1,200-byte Mac XMIC table, got {} bytes",
            decoded.len()
        );
        std::fs::write(output_path, &decoded)?;
        println!(
            "exported 150 decoded Mac XMIC records from {}",
            city_path.display()
        );
        Ok(())
    }

    fn export_city_xmic(city_path: &Path, output_path: &Path) -> anyhow::Result<()> {
        let city = super::city::City::load(city_path)?;
        let chunk = city
            .chunk("XMIC")
            .ok_or_else(|| anyhow::anyhow!("city has no XMIC chunk"))?;
        let decoded =
            super::city::sc2_rle_decode(&chunk.data).unwrap_or_else(|_| chunk.data.clone());
        anyhow::ensure!(
            decoded.len() == 150 * 8,
            "expected 1,200-byte XMIC table, got {} bytes",
            decoded.len()
        );
        std::fs::write(output_path, &decoded)?;
        println!(
            "exported 150 decoded XMIC records from {} ({:?})",
            city_path.display(),
            city.kind
        );
        Ok(())
    }
}

/// SDL entry for iOS/Android hosts (Xcode / SDLActivity link this symbol).
#[cfg(any(target_os = "android", target_os = "ios"))]
mod sdl_mobile_entry {
    use std::ffi::{c_char, c_int};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[unsafe(no_mangle)]
    pub extern "C" fn SDL_main(_argc: c_int, _argv: *mut *mut c_char) -> c_int {
        let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
            .try_init();

        match catch_unwind(AssertUnwindSafe(|| crate::run())) {
            Ok(Ok(())) => 0,
            Ok(Err(error)) => {
                log::error!("CityGame failed: {error:#}");
                1
            }
            Err(_) => {
                log::error!("CityGame panicked");
                1
            }
        }
    }
}
