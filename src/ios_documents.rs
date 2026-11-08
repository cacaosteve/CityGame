//! Native Files UI, with sandbox-owned copies for subsequent Save/Continue.

use crate::{app, city::City, document_files};
use anyhow::{Context, Result, bail};
use sdl3::video::Window;
use std::ffi::{CStr, CString, c_char, c_void};
use std::fs;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static PRESENTING: AtomicBool = AtomicBool::new(false);
type Completion = Box<dyn FnOnce(Result<Option<PathBuf>>)>;

unsafe extern "C" {
    fn citygame_ios_pick_document(
        window: *mut c_void,
        initial: *const c_char,
        export: *const c_char,
        context: *mut c_void,
        callback: extern "C" fn(*mut c_void, *const c_char, *const c_char),
    );
    fn citygame_ios_document_error(window: *mut c_void, message: *const c_char);
    fn citygame_ios_documents_directory() -> *const c_char;
    fn citygame_ios_set_document_open_callback(
        callback: extern "C" fn(*const c_char, *const c_char),
    );
}

pub fn initialize() {
    unsafe {
        citygame_ios_set_document_open_callback(external_document_opened);
    }
}

extern "C" fn external_document_opened(path: *const c_char, error: *const c_char) {
    let result = std::panic::catch_unwind(|| -> Result<()> {
        if !error.is_null() {
            bail!("{}", unsafe { CStr::from_ptr(error) }.to_string_lossy());
        }
        anyhow::ensure!(!path.is_null(), "Files did not provide a city path");
        let source = path_from_c(path);
        let filename = source
            .file_name()
            .context("the city has no filename")?
            .to_string_lossy();
        let working = document_files::import_city(&source, &cities_directory()?, &filename)?;
        log::info!("opened city directly from Files: {}", working.display());
        // Both title and gameplay loops consume this command. No provider URL
        // survives in settings, so Continue also works after an ordinary restart.
        app::queue_document_open(working, true);
        Ok(())
    });
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => log::error!("could not open city from Files: {error:#}"),
        Err(_) => log::error!("opening a city from Files panicked"),
    }
}

fn native_window(window: &Window) -> *mut c_void {
    unsafe {
        let props = sdl3::sys::video::SDL_GetWindowProperties(window.raw());
        sdl3::sys::properties::SDL_GetPointerProperty(
            props,
            sdl3::sys::video::SDL_PROP_WINDOW_UIKIT_WINDOW_POINTER,
            std::ptr::null_mut(),
        )
    }
}

pub fn is_presenting() -> bool {
    PRESENTING.load(Ordering::Acquire)
}

pub fn cities_directory() -> Result<PathBuf> {
    let path = unsafe { citygame_ios_documents_directory() };
    if path.is_null() {
        bail!("the app's Documents folder is unavailable");
    }
    let root = path_from_c(path).join("Cities");
    fs::create_dir_all(&root).context("create the CityGame Cities folder")?;
    Ok(root)
}

fn path_from_c(path: *const c_char) -> PathBuf {
    let bytes = unsafe { CStr::from_ptr(path) }.to_bytes().to_vec();
    std::ffi::OsString::from_vec(bytes).into()
}

extern "C" fn completed(context: *mut c_void, path: *const c_char, error: *const c_char) {
    // The native delegate calls exactly once, including cancellation/failure.
    let completion = unsafe { Box::from_raw(context.cast::<Completion>()) };
    let result = if !error.is_null() {
        Err(anyhow::anyhow!(
            unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned()
        ))
    } else if !path.is_null() {
        Ok(Some(path_from_c(path)))
    } else {
        Ok(None)
    };
    // Import the provider copy while the native delegate still holds its access.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| completion(result)));
    PRESENTING.store(false, Ordering::Release);
    if outcome.is_err() {
        log::error!("document picker completion panicked");
    }
}

fn present(
    window: &Window,
    directory: &Path,
    export: Option<&Path>,
    completion: Completion,
) -> Result<()> {
    let native = native_window(window);
    anyhow::ensure!(!native.is_null(), "the iPad game window is unavailable");
    let initial = CString::new(directory.as_os_str().as_bytes())?;
    let export = export
        .map(|path| CString::new(path.as_os_str().as_bytes()))
        .transpose()?;
    anyhow::ensure!(
        PRESENTING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok(),
        "a Files picker is already open"
    );
    let context = Box::into_raw(Box::new(completion)).cast();
    unsafe {
        citygame_ios_pick_document(
            native,
            initial.as_ptr(),
            export
                .as_ref()
                .map_or(std::ptr::null(), |path| path.as_ptr()),
            context,
            completed,
        );
    }
    Ok(())
}

pub fn show_error(window: &Window, error: &anyhow::Error) {
    report_error(native_window(window), error);
}

fn report_error(window: *mut c_void, error: &anyhow::Error) {
    log::error!("CityGame Files: {error:#}");
    let message = format!("{error:#}").replace('\0', " ");
    if let Ok(message) = CString::new(message) {
        unsafe {
            citygame_ios_document_error(window, message.as_ptr());
        }
    }
}

pub fn open_city(window: &Window, from_title: bool) {
    if is_presenting() {
        return;
    }
    let operation = (|| -> Result<()> {
        let cities = cities_directory()?;
        let directory = cities.clone();
        let native = native_window(window);
        present(
            window,
            &directory,
            None,
            Box::new(move |result| {
                let imported = result.and_then(|path| {
                    path.map(|source| {
                        let filename = source
                            .file_name()
                            .context("the selected file has no name")?
                            .to_string_lossy();
                        document_files::import_city(&source, &cities, &filename)
                    })
                    .transpose()
                });
                match imported {
                    Ok(Some(path)) => {
                        log::info!("imported city from Files to {}", path.display());
                        app::queue_document_open(path, from_title);
                    }
                    Ok(None) => {}
                    Err(error) => report_error(native, &error),
                }
            }),
        )
    })();
    if let Err(error) = operation {
        show_error(window, &error);
    }
}

struct ExportFile {
    directory: PathBuf,
}
impl Drop for ExportFile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

pub fn save_city(window: &Window, mut snapshot: City, previous_path: PathBuf) -> Result<()> {
    if is_presenting() {
        return Ok(());
    }
    let cities = cities_directory()?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("CityGame-export-{nonce}"));
    fs::create_dir(&directory)?;
    let temporary = ExportFile { directory };
    let filename = previous_path
        .file_name()
        .filter(|_| {
            previous_path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("sc2"))
        })
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| document_files::city_filename(&snapshot.name));
    let staged = temporary.directory.join(filename);
    snapshot
        .save(&staged)
        .context("prepare the city for Files")?;
    let native = native_window(window);
    let initial = cities.clone();
    let export_path = staged.clone();
    present(
        window,
        &initial,
        Some(&export_path),
        Box::new(move |result| {
            let saved = result.and_then(|path| {
                path.map(|destination| {
                    let working = document_files::keep_export(
                        &staged,
                        &cities,
                        &destination,
                        &previous_path,
                    )?;
                    log::info!(
                        "exported city to Files: {}; working copy: {}",
                        destination.display(),
                        working.display()
                    );
                    Ok(working)
                })
                .transpose()
            });
            match saved {
                Ok(Some(path)) => app::queue_document_saved(path, previous_path),
                Ok(None) => {}
                Err(error) => report_error(native, &error),
            }
            drop(temporary);
        }),
    )
}
