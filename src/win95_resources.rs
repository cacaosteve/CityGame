//! Small PE resource reader for locally imported Win95 strings.
//!
//! The original new-city reset loads RT_STRING IDs 401–405; keep the bytes
//! in the player's asset pack rather than distributing the resource text.

use anyhow::{Context, Result, anyhow, ensure};

fn word(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        data.get(offset..offset.checked_add(2).context("offset overflow")?)
            .context("truncated PE word")?
            .try_into()?,
    ))
}
fn dword(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset.checked_add(4).context("offset overflow")?)
            .context("truncated PE dword")?
            .try_into()?,
    ))
}

/// Read one PE32 RT_STRING block, preferring US English when available.
fn string_block(data: &[u8], block: u16) -> Result<Vec<String>> {
    ensure!(data.get(..2) == Some(b"MZ"), "not a DOS executable");
    let pe = usize::try_from(dword(data, 0x3c)?)?;
    ensure!(
        data.get(pe..pe.checked_add(4).context("PE offset overflow")?) == Some(b"PE\0\0"),
        "missing PE signature"
    );
    let coff = pe.checked_add(4).context("PE offset overflow")?;
    let sections = usize::from(word(data, coff + 2)?);
    let optional_len = usize::from(word(data, coff + 16)?);
    let optional = coff.checked_add(20).context("PE offset overflow")?;
    ensure!(
        word(data, optional)? == 0x10b && optional_len >= 120,
        "not PE32 with resources"
    );
    let resource_rva = dword(data, optional + 96 + 2 * 8)?;
    let resource_size = dword(data, optional + 96 + 2 * 8 + 4)?;
    ensure!(
        resource_rva != 0 && resource_size >= 16,
        "missing resources"
    );
    let section_table = optional
        .checked_add(optional_len)
        .context("PE offset overflow")?;
    let rva_offset = |rva: u32, size: u32| -> Result<usize> {
        for i in 0..sections {
            let s = section_table
                .checked_add(i.checked_mul(40).context("section overflow")?)
                .context("section overflow")?;
            let va = dword(data, s + 12)?;
            let raw_size = dword(data, s + 16)?;
            let raw = dword(data, s + 20)?;
            if let Some(delta) = rva.checked_sub(va)
                && delta.checked_add(size).is_some_and(|end| end <= raw_size)
            {
                let offset =
                    usize::try_from(raw.checked_add(delta).context("raw offset overflow")?)?;
                let end = offset
                    .checked_add(usize::try_from(size)?)
                    .context("raw size overflow")?;
                ensure!(end <= data.len(), "truncated resource data");
                return Ok(offset);
            }
        }
        Err(anyhow!("resource RVA is outside file sections"))
    };
    let root = rva_offset(resource_rva, resource_size)?;
    let resource = &data[root..root + usize::try_from(resource_size)?];
    let entry = |dir: usize, wanted: Option<u16>| -> Result<u32> {
        let named = usize::from(word(resource, dir + 12)?);
        let ids = usize::from(word(resource, dir + 14)?);
        let count = named.checked_add(ids).context("resource count overflow")?;
        let mut fallback = None;
        for index in 0..count {
            let at = dir
                .checked_add(16)
                .and_then(|n| n.checked_add(index.checked_mul(8)?))
                .context("resource directory overflow")?;
            let name = dword(resource, at)?;
            let target = dword(resource, at + 4)?;
            if name & 0x8000_0000 == 0 {
                if wanted == Some(name as u16) {
                    return Ok(target);
                }
                if wanted.is_none() && fallback.is_none() {
                    fallback = Some(target);
                }
            }
        }
        fallback.context("resource entry not found")
    };
    let table = |value: u32| -> Result<usize> {
        ensure!(value & 0x8000_0000 != 0, "expected resource directory");
        Ok((value & 0x7fff_ffff) as usize)
    };
    let type_dir = table(entry(0, Some(6))?)?;
    let string_dir = table(entry(type_dir, Some(block))?)?;
    let lang = entry(string_dir, Some(0x409)).or_else(|_| entry(string_dir, None))?;
    ensure!(lang & 0x8000_0000 == 0, "expected resource data");
    let data_entry = lang as usize;
    let text_rva = dword(resource, data_entry)?;
    let text_size = dword(resource, data_entry + 4)?;
    let offset = rva_offset(text_rva, text_size)?;
    let text = &data[offset..offset + usize::try_from(text_size)?];
    let mut cursor = 0usize;
    let mut labels = Vec::new();
    for _ in 0..16 {
        let count = usize::from(word(text, cursor)?);
        cursor = cursor.checked_add(2).context("string offset overflow")?;
        let end = cursor
            .checked_add(count.checked_mul(2).context("string length overflow")?)
            .context("string offset overflow")?;
        let bytes = text.get(cursor..end).context("truncated string table")?;
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect();
        labels.push(String::from_utf16(&units).context("invalid resource UTF-16")?);
        cursor = end;
    }
    Ok(labels)
}

/// Extract the five native new-city reserved labels.
pub(crate) fn new_city_reserved_labels(data: &[u8]) -> Result<[String; 5]> {
    let strings = string_block(data, 26)?;
    strings[1..=5]
        .to_vec()
        .try_into()
        .map_err(|_| anyhow!("incomplete new-city labels"))
}

/// Load the native save-success prefix and save-failure message.
pub(crate) fn save_feedback_strings(data: &[u8]) -> Result<[String; 2]> {
    let strings = string_block(data, 4)?;
    Ok([strings[3].clone(), strings[12].clone()])
}

/// Load the native city-open failure message (RT_STRING 412).
pub(crate) fn city_open_failure_string(data: &[u8]) -> Result<String> {
    Ok(string_block(data, 26)?[12].clone())
}

/// Read-error prefix and the 15 cause strings used by the original formatter.
pub(crate) fn read_error_strings(data: &[u8]) -> Result<Vec<String>> {
    let prefix = string_block(data, 4)?;
    let first = string_block(data, 34)?;
    let last = string_block(data, 35)?;
    let mut strings = vec![prefix[0].clone()];
    strings.extend_from_slice(&first[3..]);
    strings.extend_from_slice(&last[..2]);
    Ok(strings)
}

/// Load the file-format error shown before the general city-open failure.
pub(crate) fn not_iff_string(data: &[u8]) -> Result<String> {
    Ok(string_block(data, 4)?[5].clone())
}

/// Load the message for an IFF FORM with the wrong city type (RT_STRING 54).
pub(crate) fn wrong_iff_city_type_string(data: &[u8]) -> Result<String> {
    Ok(string_block(data, 4)?[6].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x800];
        fn w(b: &mut [u8], at: usize, v: u16) {
            b[at..at + 2].copy_from_slice(&v.to_le_bytes());
        }
        fn d(b: &mut [u8], at: usize, v: u32) {
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes[..2].copy_from_slice(b"MZ");
        d(&mut bytes, 0x3c, 0x80);
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        let coff = 0x84;
        w(&mut bytes, coff + 2, 1);
        w(&mut bytes, coff + 16, 0xe0);
        let opt = coff + 20;
        w(&mut bytes, opt, 0x10b);
        d(&mut bytes, opt + 112, 0x1000);
        d(&mut bytes, opt + 116, 0x300);
        let section = opt + 0xe0;
        d(&mut bytes, section + 12, 0x1000);
        d(&mut bytes, section + 16, 0x400);
        d(&mut bytes, section + 20, 0x200);
        let root = 0x200;
        for (dir, id, target) in [
            (0, 6, 0x8000_0020),
            (0x20, 26, 0x8000_0040),
            (0x40, 0x409, 0x60),
        ] {
            w(&mut bytes, root + dir + 14, 1);
            d(&mut bytes, root + dir + 16, id);
            d(&mut bytes, root + dir + 20, target);
        }
        let mut text = Vec::new();
        for value in [
            "",
            "Lămas",
            "Two",
            "Three",
            "Four",
            "Five",
            "Wrong type",
            "",
            "",
            "",
            "",
            "",
            "Open failed",
            "",
            "",
            "",
        ] {
            let words: Vec<u16> = value.encode_utf16().collect();
            text.extend_from_slice(&(words.len() as u16).to_le_bytes());
            for word in words {
                text.extend_from_slice(&word.to_le_bytes());
            }
        }
        d(&mut bytes, root + 0x60, 0x1100);
        d(&mut bytes, root + 0x64, text.len() as u32);
        bytes[0x300..0x300 + text.len()].copy_from_slice(&text);
        bytes
    }

    #[test]
    fn extracts_new_city_resource_strings_and_rejects_truncation() {
        let bytes = fixture();
        assert_eq!(
            new_city_reserved_labels(&bytes).unwrap(),
            ["Lămas", "Two", "Three", "Four", "Five"]
        );
        for end in [0, 0x40, 0x200, 0x310] {
            assert!(
                new_city_reserved_labels(&bytes[..end]).is_err(),
                "truncation at {end:x}"
            );
        }
        let mut non_english = bytes;
        non_english[0x250..0x254].copy_from_slice(&0x040c_u32.to_le_bytes());
        assert_eq!(
            new_city_reserved_labels(&non_english).unwrap(),
            ["Lămas", "Two", "Three", "Four", "Five"]
        );
    }

    #[test]
    fn extracts_save_feedback_from_its_resource_block() {
        let mut bytes = fixture();
        bytes[0x230..0x234].copy_from_slice(&4_u32.to_le_bytes());
        assert_eq!(
            save_feedback_strings(&bytes).unwrap(),
            ["Three", "Open failed"]
        );
        assert_eq!(not_iff_string(&bytes).unwrap(), "Five");
        assert_eq!(wrong_iff_city_type_string(&bytes).unwrap(), "Wrong type");
        assert!(new_city_reserved_labels(&bytes).is_err());
    }

    #[test]
    fn extracts_city_open_failure_from_its_resource_block() {
        assert_eq!(city_open_failure_string(&fixture()).unwrap(), "Open failed");
    }

    #[test]
    #[ignore = "requires a locally supplied original Win95 executable"]
    fn extracts_reserved_labels_from_original_executable() {
        let path = std::env::var_os("CITYGAME_WIN95_EXE").unwrap();
        let labels = new_city_reserved_labels(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            labels,
            ["Llamas", "Alpacas", "Camels", "Dromedaries", "Army Ants"]
        );
    }

    #[test]
    #[ignore = "requires a local Win95 executable and independent resource extraction"]
    fn extracts_save_feedback_from_original_executable() {
        let executable = std::fs::read(std::env::var_os("CITYGAME_WIN95_EXE").unwrap()).unwrap();
        let expected: [String; 2] = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_WIN95_SAVE_FEEDBACK").unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(save_feedback_strings(&executable).unwrap(), expected);
    }

    #[test]
    #[ignore = "requires a local Win95 executable and independent resource extraction"]
    fn extracts_city_open_failure_from_original_executable() {
        let executable = std::fs::read(std::env::var_os("CITYGAME_WIN95_EXE").unwrap()).unwrap();
        let expected: String = serde_json::from_slice(
            &std::fs::read(std::env::var_os("CITYGAME_WIN95_OPEN_FAILURE").unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(city_open_failure_string(&executable).unwrap(), expected);
    }

    #[test]
    #[ignore = "requires a locally supplied original Win95 executable"]
    fn extracts_read_errors_from_original_executable() {
        let executable = std::fs::read(std::env::var_os("CITYGAME_WIN95_EXE").unwrap()).unwrap();
        let strings = read_error_strings(&executable).unwrap();
        assert_eq!(strings.len(), 16);
        assert_eq!(strings[0], "Error Reading File");
        assert_eq!(strings[1], "No error occurred");
        assert_eq!(strings[4], "All or part of the path is invalid");
    }
}
