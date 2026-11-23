//! The Win95 file exception survives successful file operations.
//! City and attempted newspaper archive open failures are connected; see the
//! private trace notes for native callers which are not yet modeled.
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};

static CAUSE: AtomicU8 = AtomicU8::new(0);

fn cause_for_windows_error(code: u32) -> u8 {
    match code {
        0 => 0,
        2 | 6 | 18 | 107 => 2,
        3 | 15 | 17 | 34 | 52 | 53 | 55 | 67 | 70 | 85 | 111 | 123 | 124 | 125 | 144 | 154
        | 161 | 206 | 208 | 267 => 3,
        4 | 36 | 68 | 113 => 4,
        5 | 12 | 19 | 29 | 54 | 58 | 64 | 65 | 71 | 80 | 82 | 86 | 170 | 183 | 999 => 5,
        11 | 26 | 60 | 66 | 114 | 182 | 191 | 193 => 6,
        16 | 145 => 7,
        62 => 8,
        24 | 25 | 27 | 30 | 131 | 132 => 9,
        20 | 21 | 22 | 23 | 57 | 59 | 88 | 117 | 995 | 996 | 997 => 10,
        32 => 11,
        33 | 108 | 167 => 12,
        39 | 112 => 13,
        38 => 14,
        _ => 1,
    }
}

pub(crate) fn current_cause() -> u8 {
    CAUSE.load(Ordering::Relaxed)
}

pub(crate) fn record_city_io_error(path: &Path, error: &anyhow::Error) {
    let Some(open_error) = error.downcast_ref::<crate::city::CityOpenIoError>() else {
        return;
    };
    record_open_io_error(path, &open_error.0);
}

pub(crate) fn record_open_io_error(path: &Path, io: &std::io::Error) {
    #[cfg(windows)]
    let code = io.raw_os_error().map(|code| code as u32).unwrap_or(1);
    // Other hosts do not expose Win32 errors; these common translations are
    // approximations, including the distinction between a missing file and path.
    #[cfg(not(windows))]
    let code = match io.kind() {
        std::io::ErrorKind::NotFound => {
            if path
                .parent()
                .is_some_and(|parent| !parent.as_os_str().is_empty() && !parent.exists())
            {
                3
            } else {
                2
            }
        }
        std::io::ErrorKind::PermissionDenied => 5,
        std::io::ErrorKind::AlreadyExists => 80,
        _ => 1,
    };
    #[cfg(windows)]
    let _ = path;
    CAUSE.store(cause_for_windows_error(code), Ordering::Relaxed);
}

pub(crate) fn read_message(strings: Option<&[String]>, cause: u8) -> String {
    if let Some(strings) = strings.filter(|strings| strings.len() >= 16) {
        let index = if cause <= 14 { usize::from(cause) } else { 1 };
        return format!("{}\r\n{}", strings[0], strings[index + 1]);
    }
    if cause == 0 {
        "Error Reading File\r\nNo error occurred".to_owned()
    } else {
        "Error Reading File".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_native_errors_and_formats_cause() {
        for (code, cause) in [
            (0, 0),
            (2, 2),
            (3, 3),
            (5, 5),
            (32, 11),
            (267, 3),
            (995, 10),
            (999, 5),
            (5000, 1),
        ] {
            assert_eq!(cause_for_windows_error(code), cause);
        }
        let strings: Vec<String> = (0..16).map(|n| n.to_string()).collect();
        assert_eq!(read_message(Some(&strings), 0), "0\r\n1");
        assert_eq!(read_message(Some(&strings), 3), "0\r\n4");
        assert_eq!(read_message(Some(&strings), 255), "0\r\n2");
    }

    #[test]
    fn failed_pe_save_open_survives_successful_save_and_empty_city_read() {
        let root =
            std::env::temp_dir().join(format!("citygame-pe-error-history-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let missing = root.join("missing").join("city.sc2");
        let saved = root.join("saved.sc2");
        let empty = root.join("empty.sc2");
        std::fs::write(&empty, []).unwrap();
        let mut city = crate::city::City::new_blank(&saved, "Test").unwrap();
        let previous_cause = CAUSE.swap(0, Ordering::Relaxed);
        let error = city.save_pe_city(&missing).unwrap_err();
        assert!(
            error
                .downcast_ref::<crate::city::CityOpenIoError>()
                .is_some()
        );
        assert_eq!(current_cause(), 3);
        city.save_pe_city(&saved).unwrap();
        let error = crate::city::City::load(&empty).unwrap_err();
        assert!(error.downcast_ref::<crate::city::CityReadEof>().is_some());
        record_city_io_error(&empty, &error);
        assert_eq!(current_cause(), 3);
        let strings: Vec<String> = (0..16).map(|n| n.to_string()).collect();
        assert_eq!(read_message(Some(&strings), current_cause()), "0\r\n4");
        CAUSE.store(previous_cause, Ordering::Relaxed);
        std::fs::remove_dir_all(root).unwrap();
    }
}
