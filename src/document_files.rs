//! Persistent working copies for files imported/exported through mobile providers.

use crate::city::City;
use anyhow::{Context, Result, bail};
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

pub fn city_filename(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '/' | '\\' | ':') {
                '-'
            } else {
                ch
            }
        })
        .collect();
    let clean = clean.trim().trim_matches('.');
    let clean = if clean.is_empty() { "Untitled" } else { clean };
    format!("{clean}.sc2")
}

/// Keep both files when the user imports distinct cities with the same name.
pub fn import_city(source: &Path, cities: &Path, filename: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        fs::metadata(source)?.len() <= 64 * 1024 * 1024,
        "the selected city is too large for mobile import"
    );
    City::load(source).context("the selected file is not a readable city or scenario")?;
    fs::create_dir_all(cities)?;
    let cities = cities.canonicalize()?;
    let source = source.canonicalize()?;
    if source.parent() == Some(cities.as_path()) && source.file_name() == Some(filename.as_ref()) {
        return Ok(source);
    }
    let name = Path::new(filename)
        .file_name()
        .context("missing city filename")?;
    anyhow::ensure!(
        Path::new(filename) == Path::new(name),
        "invalid city filename"
    );
    let name = Path::new(name);
    let stem = name.file_stem().unwrap_or_default().to_string_lossy();
    let extension = name.extension().unwrap_or_default().to_string_lossy();
    for suffix in 1..10_000 {
        let filename = if suffix == 1 {
            name.to_path_buf()
        } else {
            PathBuf::from(format!("{stem}-{suffix}.{extension}"))
        };
        let target = cities.join(filename);
        let mut output = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let result = (|| -> Result<()> {
            let mut input = fs::File::open(&source)?;
            io::copy(&mut input, &mut output)?;
            output.sync_all()?;
            Ok(())
        })();
        if let Err(error) = result {
            drop(output);
            let _ = fs::remove_file(&target);
            return Err(error);
        }
        return Ok(target);
    }
    bail!("too many saved cities with the same filename")
}

/// Replace only the app's current working copy, never another imported city.
pub fn keep_export(
    source: &Path,
    cities: &Path,
    destination: &Path,
    current: &Path,
) -> Result<PathBuf> {
    fs::create_dir_all(cities)?;
    let directory = cities.canonicalize()?;
    // Files may have already exported into this app's visible Cities folder.
    if let Ok(saved) = destination.canonicalize() {
        if saved.parent() == Some(directory.as_path()) {
            City::load(&saved).context("read the exported city")?;
            return Ok(saved);
        }
    }
    let filename = destination
        .file_name()
        .context("the saved file has no name")?
        .to_string_lossy();
    let owned = current.canonicalize().ok().filter(|path| {
        path.parent() == Some(directory.as_path()) && path.file_name() == destination.file_name()
    });
    let fresh = import_city(source, &directory, &filename)?;
    if let Some(owned) = owned {
        // Both paths are in Cities on the same volume; rename atomically replaces
        // the working copy only after the complete new snapshot is on disk.
        fs::rename(&fresh, &owned)?;
        Ok(owned)
    } else {
        Ok(fresh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn document_copies_survive_provider_cleanup_and_do_not_overwrite_other_cities() {
        let root = std::env::temp_dir().join(format!(
            "citygame-documents-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("provider.sc2");
        let mut city = City::new_blank(&source, "Mixed Case City").unwrap();
        city.set_city_funds(12345);
        city.save(&source).unwrap();
        let cities = root.join("Documents/Cities");
        let first = import_city(&source, &cities, "My City.sc2").unwrap();
        city.set_city_funds(6789);
        city.save(&source).unwrap();
        let second = import_city(&source, &cities, "My City.sc2").unwrap();
        assert_ne!(first, second);
        assert_eq!(City::load(&first).unwrap().city_funds(), 12345);
        assert_eq!(City::load(&second).unwrap().city_funds(), 6789);
        let saved =
            keep_export(&source, &cities, Path::new("/provider/My City.sc2"), &first).unwrap();
        assert_eq!(saved, first);
        assert_eq!(City::load(&first).unwrap().city_funds(), 6789);
        assert_eq!(import_city(&first, &cities, "My City.sc2").unwrap(), first);
        assert_eq!(
            keep_export(&source, &cities, &first, &first).unwrap(),
            first
        );
        fs::remove_file(source).unwrap();
        assert!(City::load(first).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_names_keep_case_and_strip_path_separators() {
        assert_eq!(city_filename("My City"), "My City.sc2");
        assert_eq!(city_filename("A/B\\C:D"), "A-B-C-D.sc2");
        assert_eq!(city_filename(" ... "), "Untitled.sc2");
    }

    #[test]
    fn invalid_import_does_not_leave_a_working_city_behind() {
        let root = std::env::temp_dir().join(format!(
            "citygame-invalid-document-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("Not A City.sc2");
        fs::write(&source, b"not a city file").unwrap();
        let cities = root.join("Cities");
        assert!(import_city(&source, &cities, "Not A City.sc2").is_err());
        assert!(!cities.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
