//! First-run artwork setup, runtime media reloads, and bundled mobile assets.
//!
//! Folder selection completes before importing. The runtime retains ownership
//! of game windows, renderers, and audio.

use super::cursors::GameCursors;
use super::{AppCommand, push_command};
use crate::asset_profile::{AssetProfile, OriginalMedia};
use crate::mac_menu;
use crate::renderer::Renderer;
use anyhow::{Context, Result, bail};
#[cfg(not(target_os = "macos"))]
use sdl3::dialog::show_open_folder_dialog;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use sdl3::messagebox::{ButtonData, ClickedButton, MessageBoxButtonFlag, show_message_box};
use sdl3::messagebox::{MessageBoxFlag, show_simple_message_box};
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "macos"))]
use std::sync::mpsc::{self, RecvTimeoutError};
#[cfg(not(target_os = "macos"))]
use std::time::Duration;

pub(super) fn reload_title_assets(
    profile: &AssetProfile,
    renderer: &mut Renderer<'_>,
) -> Result<PathBuf> {
    let asset_root = profile
        .original_root()
        .context("no original graphics pack is configured")?;
    renderer.reload_with_library(
        &profile.ordered_sources(OriginalMedia::Graphics),
        profile.replacement_root().as_deref(),
        profile.tile_set_root().as_deref(),
        profile.preference().prefers_replacement(),
    )?;
    Ok(asset_root)
}

fn open_folder_dialog(
    window: &sdl3::video::Window,
    initial: Option<&Path>,
    callback: impl Fn(Result<Option<PathBuf>>) + Send + 'static,
) {
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        let initial = initial.map(Path::to_path_buf);
        // The game's main loop stays responsive while the helper owns AppKit.
        std::thread::spawn(move || {
            callback(crate::mac_folder_picker::choose_folder(
                initial.as_deref(),
                || {},
            ));
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        show_open_folder_dialog(
            initial,
            false,
            window,
            Box::new(move |selection, _filter| {
                callback(
                    selection
                        .map(|paths| paths.into_iter().next())
                        .map_err(|error| anyhow::anyhow!(error.to_string())),
                );
            }),
        );
    }
}

pub(super) fn open_tile_set_dialog(window: &sdl3::video::Window, current_root: &Path) {
    open_folder_dialog(window, Some(current_root), |selection| match selection {
        Ok(Some(path)) => push_command(AppCommand::LoadTileSet(path)),
        Ok(None) => {}
        Err(error) => log::error!("tile set dialog error: {error}"),
    });
}

#[derive(Clone, Copy)]
pub(super) enum AssetFolderPurpose {
    ImportOriginal,
    ChooseOriginal,
    ChooseReplacement,
}

pub(super) fn open_asset_folder_dialog(window: &sdl3::video::Window, purpose: AssetFolderPurpose) {
    open_folder_dialog(window, None, move |selection| match selection {
        Ok(Some(path)) => {
            let command = match purpose {
                AssetFolderPurpose::ImportOriginal => AppCommand::ImportOriginalAssets(path),
                AssetFolderPurpose::ChooseOriginal => AppCommand::SetOriginalAssets(path),
                AssetFolderPurpose::ChooseReplacement => AppCommand::SetReplacementAssets(path),
            };
            push_command(command);
        }
        Ok(None) => {}
        Err(error) => log::error!("asset folder dialog error: {error}"),
    });
}

pub(super) fn reload_asset_profile(
    profile: &AssetProfile,
    renderer: &mut Renderer<'_>,
    palette_renderer: Option<&mut Renderer<'_>>,
    cursors: &mut GameCursors,
) -> Result<PathBuf> {
    let asset_root = profile
        .original_root()
        .context("no original graphics pack is configured")?;
    let replacement_root = profile.replacement_root();
    let prefer_replacement = profile.preference().prefers_replacement();
    let new_cursors = GameCursors::for_assets(&asset_root);
    renderer.reload_with_library(
        &profile.ordered_sources(OriginalMedia::Graphics),
        replacement_root.as_deref(),
        profile.tile_set_root().as_deref(),
        prefer_replacement,
    )?;
    if let Some(palette_renderer) = palette_renderer {
        palette_renderer.reload_with_library(
            &profile.ordered_sources(OriginalMedia::Graphics),
            replacement_root.as_deref(),
            profile.tile_set_root().as_deref(),
            prefer_replacement,
        )?;
    }
    *cursors = new_cursors;
    Ok(asset_root)
}

pub(super) fn load_profile_sound(
    audio: Option<&sdl3::AudioSubsystem>,
    profile: &AssetProfile,
) -> Option<crate::audio::SoundPlayer> {
    let paths = profile.sound_paths();
    audio.and_then(|audio| crate::audio::SoundPlayer::new(audio, &paths))
}

pub(super) fn ensure_first_run_assets(
    window: &sdl3::video::Window,
    event_pump: &mut sdl3::EventPump,
    profile: &mut AssetProfile,
) -> Result<()> {
    if profile
        .original_root()
        .is_some_and(|root| crate::asset_profile::validate_original_pack(&root).is_ok())
    {
        return Ok(());
    }

    // iOS/Android cannot import via folder picker, and SDL message boxes at launch are
    // unreliable. Graphics must come from SeedAssets.zip / host seed scripts.
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (window, event_pump, profile);
        bail!(
            "no playable graphics pack after mobile seed — rebuild with \
             scripts/seed_ios_assets.sh or scripts/build_android_host.sh (SeedAssets.zip) \
             and reinstall"
        );
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        loop {
            let buttons = [
                ButtonData {
                    flags: MessageBoxButtonFlag::RETURNKEY_DEFAULT,
                    button_id: 1,
                    text: "Import from Game Files",
                },
                ButtonData {
                    flags: MessageBoxButtonFlag::NOTHING,
                    button_id: 2,
                    text: "Use Extracted Pack",
                },
                ButtonData {
                    flags: MessageBoxButtonFlag::ESCAPEKEY_DEFAULT,
                    button_id: 0,
                    text: "Quit",
                },
            ];
            let choice = show_message_box(
                MessageBoxFlag::INFORMATION,
                &buttons,
                "Set Up Game Artwork",
                "Choose a Win95/Special Edition CD, a DOS/GOG/EA-compatible game folder, or an exported Mac 68k resource folder. This app will identify it and extract a private local asset pack. You can also choose a pack that was already extracted. Imported files stay on this computer.",
                window,
                None,
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            let button_id = match choice {
                ClickedButton::CustomButton(button) => button.button_id,
                ClickedButton::CloseButton => 0,
            };
            if button_id == 0 {
                bail!("asset setup was cancelled");
            }
            let Some(folder) = choose_folder_blocking(window, event_pump)? else {
                continue;
            };
            let result = if button_id == 1 {
                profile.import_original(&folder).map(|_| ())
            } else {
                profile.set_original_pack(&folder).map(|_| ())
            };
            match result {
                Ok(()) => return Ok(()),
                Err(error) => show_asset_error(window, "Could Not Set Up Artwork", &error),
            }
        }
    }
}

/// Copy `SeedAssets/` (or unzip `SeedAssets.zip`) from the app bundle into the SDL pref path
/// when the sandbox is empty. Populated by seed scripts (gitignored media).
#[cfg(any(target_os = "android", target_os = "ios"))]
pub(super) fn seed_bundled_assets_if_needed() {
    use std::fs;
    use std::io;
    use std::path::PathBuf;

    let dest = crate::asset_profile::app_support_dir_for_seed();
    let settings = dest.join("asset-settings.json");
    let revision_path = dest.join(".citygame-mobile-seed-revision");
    let revision_ok = revision_path
        .is_file()
        .then(|| fs::read_to_string(&revision_path).ok())
        .flatten()
        .is_some_and(|text| text.trim() == crate::mobile_graphics::MOBILE_SEED_REVISION);
    // Re-seed when missing OR when settings point at a pack that no longer validates
    // (e.g. leftover settings after a wiped SeedAssets zip / reinstall), OR when the
    // seeded layout revision changed (full @8x → adaptive @4x/@2x tiers).
    let pack_ok = settings.is_file()
        && revision_ok
        && crate::asset_profile::AssetProfile::load()
            .ok()
            .and_then(|p| p.original_root())
            .is_some_and(|root| crate::asset_profile::validate_original_pack(&root).is_ok());
    if pack_ok {
        return;
    }
    if settings.is_file() {
        if !revision_ok {
            log::warn!(
                "mobile seed: revision mismatch (want {}); clearing replacements and re-seeding",
                crate::mobile_graphics::MOBILE_SEED_REVISION
            );
            let _ = fs::remove_dir_all(dest.join("replacements"));
            let _ = fs::remove_file(&settings);
            let _ = fs::remove_file(&revision_path);
        } else {
            log::warn!(
                "mobile seed: existing {} but original pack invalid — re-seeding",
                settings.display()
            );
        }
    }
    let Ok(base) = sdl3::filesystem::get_base_path() else {
        return;
    };
    let base = PathBuf::from(&base);
    let dir_candidates = [
        base.join("SeedAssets"),
        base.join("assets").join("SeedAssets"),
    ];
    if let Some(src) = dir_candidates.into_iter().find(|p| p.is_dir()) {
        if let Err(error) = copy_dir_recursive(&src, &dest) {
            log::warn!("seed assets from {} failed: {error:#}", src.display());
            return;
        }
        log::info!(
            "seeded mobile assets from {} → {}",
            src.display(),
            dest.display()
        );
        return;
    }
    let zip_candidates = [
        base.join("SeedAssets.zip"),
        base.join("assets").join("SeedAssets.zip"),
    ];
    let Some(zip_path) = zip_candidates.into_iter().find(|p| p.is_file()) else {
        log::info!("no bundled SeedAssets under {}", base.display());
        return;
    };
    if let Err(error) = fs::create_dir_all(&dest) {
        log::warn!("create seed dest failed: {error}");
        return;
    }
    match (|| -> Result<()> {
        let file = fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let Some(rel) = file.enclosed_name() else {
                continue;
            };
            let out = dest.join(rel);
            if file.is_dir() {
                fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&out)?;
            io::copy(&mut file, &mut outfile)?;
        }
        Ok(())
    })() {
        Ok(()) => log::info!(
            "seeded mobile assets from {} → {}",
            zip_path.display(),
            dest.display()
        ),
        Err(error) => log::warn!("unzip {} failed: {error:#}", zip_path.display()),
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    use std::fs;
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

pub(super) fn choose_folder_blocking(
    window: &sdl3::video::Window,
    event_pump: &mut sdl3::EventPump,
) -> Result<Option<PathBuf>> {
    #[cfg(target_os = "macos")]
    {
        let _ = window;
        let selection = crate::mac_folder_picker::choose_folder(None, || {
            mac_menu::pump_events();
            event_pump.pump_events();
        });
        mac_menu::activate_app();
        selection
    }
    #[cfg(not(target_os = "macos"))]
    {
        let (sender, receiver) = mpsc::channel();
        open_folder_dialog(window, None, move |selection| {
            let _ = sender.send(selection);
        });
        loop {
            mac_menu::pump_events();
            event_pump.pump_events();
            match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(selection) => return selection,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    bail!("folder picker closed without a result")
                }
            }
        }
    }
}

pub(super) fn show_asset_error(window: &sdl3::video::Window, title: &str, error: &anyhow::Error) {
    log::error!("{title}: {error:#}");
    let _ = show_simple_message_box(MessageBoxFlag::ERROR, title, &format!("{error:#}"), window);
}
