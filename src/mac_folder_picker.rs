//! Folder selection in an independent AppKit process. SDL's process continues
//! servicing events, while the native browser owns its normal application loop.
use anyhow::{Context, Result, bail, ensure};
use std::ffi::OsString;
use std::fs;
use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

const EXECUTABLE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/CityGameFolderPicker"));
const INFO: &[u8] = include_bytes!("../native/folder_picker.plist");
static OPEN: AtomicBool = AtomicBool::new(false);
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct PickerGuard;
impl Drop for PickerGuard {
    fn drop(&mut self) {
        OPEN.store(false, Ordering::Release);
    }
}

// Embed the helper so a copied executable and CityGame.app both work without
// a build tree, compiler, scripts, or extra installation steps at runtime.
fn helper_executable() -> Result<PathBuf> {
    let root = sdl3::filesystem::get_pref_path("CityGame", "CityGame")
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .join("Native Picker");
    fs::create_dir_all(&root).context("create native picker cache")?;
    let hash = EXECUTABLE
        .iter()
        .chain(INFO)
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    let bundle = root.join(format!("{hash:016x}.app"));
    let relative = Path::new("Contents/MacOS/CityGameFolderPicker");
    if bundle.join(relative).is_file() {
        return Ok(bundle.join(relative));
    }
    let temporary = root.join(format!(
        ".picker-{}-{}.app",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let install = (|| -> Result<()> {
        fs::create_dir_all(temporary.join("Contents/MacOS"))?;
        fs::write(temporary.join("Contents/Info.plist"), INFO)?;
        fs::write(temporary.join(relative), EXECUTABLE)?;
        fs::set_permissions(temporary.join(relative), fs::Permissions::from_mode(0o700))?;
        match fs::rename(&temporary, &bundle) {
            Ok(()) => Ok(()),
            Err(_) if bundle.join(relative).is_file() => Ok(()),
            Err(error) => Err(error.into()),
        }
    })();
    let _ = fs::remove_dir_all(&temporary);
    install.context("install the bundled native folder picker")?;
    Ok(bundle.join(relative))
}

fn selected_folder(output: Output) -> Result<Option<PathBuf>> {
    ensure!(
        output.status.success(),
        "native folder picker failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    if output.stdout.is_empty() {
        return Ok(None);
    }
    ensure!(
        !output.stdout.contains(&0),
        "native folder picker returned an invalid path"
    );
    // No trimming: spaces, Unicode, and newlines can be part of a folder name.
    let path = PathBuf::from(OsString::from_vec(output.stdout));
    ensure!(
        path.is_absolute(),
        "native folder picker returned a relative path"
    );
    Ok(Some(path))
}

pub(crate) fn choose_folder(
    initial: Option<&Path>,
    mut pump_events: impl FnMut(),
) -> Result<Option<PathBuf>> {
    if OPEN
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        bail!("a folder picker is already open");
    }
    let _guard = PickerGuard;
    let mut command = Command::new(helper_executable()?);
    if let Some(initial) = initial {
        command.arg(initial);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("open the native folder picker")?;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                pump_events();
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context("wait for the native folder picker");
            }
        }
    }
    selected_folder(
        child
            .wait_with_output()
            .context("read the selected folder")?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    fn output(code: i32, path: &[u8], error: &[u8]) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: path.to_vec(),
            stderr: error.to_vec(),
        }
    }
    #[test]
    fn cancel_returns_no_selection() {
        assert_eq!(selected_folder(output(0, b"", b"")).unwrap(), None);
    }
    #[test]
    fn selected_path_preserves_spaces_unicode_and_newlines() {
        let path = "/tmp/City assets/é\n";
        assert_eq!(
            selected_folder(output(0, path.as_bytes(), b"")).unwrap(),
            Some(PathBuf::from(path))
        );
    }
    #[test]
    fn failures_are_reported_instead_of_treated_as_cancel() {
        let error = selected_folder(output(1, b"", b"could not show panel")).unwrap_err();
        assert!(error.to_string().contains("could not show panel"));
    }
    #[test]
    fn malformed_results_are_rejected() {
        assert!(selected_folder(output(0, b"relative/folder", b"")).is_err());
        assert!(selected_folder(output(0, b"/tmp/a\0b", b"")).is_err());
    }
}
