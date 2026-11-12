//! Finder-identified scenarios exported as SCDH data and AppleDouble forks.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MacScenarioStart {
    pub action: u16,
    pub point: (u8, u8),
    pub goals: [u32; 16],
}

impl MacScenarioStart {
    pub(crate) fn decode_win95(payload: &[u8]) -> Option<Self> {
        Self::decode(payload.strip_prefix(&[0x80, 0, 0, 0])?)
    }

    fn decode(payload: &[u8]) -> Option<Self> {
        let header = payload.get(..48)?;
        let word = |offset| be_u16(header, offset).expect("within the header");
        let dword = |offset| be_u32(header, offset).expect("within the header");
        let signed_word = |value: u16| i32::from(value as i16) as u32;
        let tile_count = |id, offset| {
            be_u16(payload, offset)
                .map(signed_word)
                .or((id == 0).then_some(0))
        };
        Some(Self {
            action: word(0),
            point: (header[2], header[3]),
            goals: [
                signed_word(word(4)),
                dword(6),
                dword(10),
                dword(14),
                dword(18),
                dword(22),
                dword(26),
                signed_word(word(30)),
                signed_word(word(32)),
                dword(34),
                dword(38),
                dword(42),
                u32::from(header[46]),
                u32::from(header[47]),
                tile_count(header[46], 48)?,
                tile_count(header[47], 50)?,
            ],
        })
    }
}

pub(crate) struct MacScenario {
    pub start: MacScenarioStart,
    pub text: Option<String>,
}

impl MacScenario {
    pub(crate) fn read(path: &Path) -> Option<Self> {
        sidecar_paths(path)?
            .into_iter()
            .filter_map(|path| fs::read(path).ok())
            .find_map(|bytes| Self::from_apple_double(&bytes))
    }

    fn from_apple_double(bytes: &[u8]) -> Option<Self> {
        if apple_double_entry(bytes, 9)?.get(..8)? != b"SCENSCDH" {
            return None;
        }
        let fork = apple_double_entry(bytes, 2)?;
        let start = MacScenarioStart::decode(resource_128(fork, b"SCEN")?)?;
        let text = resource_128(fork, b"TEXT").map(|bytes| {
            String::from_utf8_lossy(bytes)
                .replace('\r', "\n")
                .trim_matches('\0')
                .to_string()
        });
        Some(Self { start, text })
    }
}

pub(crate) fn scenario_sidecar_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".rsrc");
    PathBuf::from(name)
}

pub(crate) fn read_scenario_sidecar(path: &Path) -> Option<Vec<u8>> {
    sidecar_paths(path)?
        .into_iter()
        .filter_map(|path| fs::read(path).ok())
        .find(|bytes| MacScenario::from_apple_double(bytes).is_some())
}

fn sidecar_paths(path: &Path) -> Option<[PathBuf; 2]> {
    let mut hidden = OsString::from("._");
    hidden.push(path.file_name()?);
    Some([scenario_sidecar_path(path), path.with_file_name(hidden)])
}

fn apple_double_entry(bytes: &[u8], id: u32) -> Option<&[u8]> {
    if be_u32(bytes, 0)? != 0x0005_1607 || be_u32(bytes, 4)? != 0x0002_0000 {
        return None;
    }
    let count = usize::from(be_u16(bytes, 24)?);
    let entries = bytes.get(26..26 + count * 12)?;
    for entry in entries.chunks_exact(12) {
        if be_u32(entry, 0)? == id {
            return section(bytes, be_u32(entry, 4)?, be_u32(entry, 8)?);
        }
    }
    None
}

fn resource_128<'a>(fork: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    let data = section(fork, be_u32(fork, 0)?, be_u32(fork, 8)?)?;
    let map = section(fork, be_u32(fork, 4)?, be_u32(fork, 12)?)?;
    let types = map.get(usize::from(be_u16(map, 24)?)..)?;
    let count = be_u16(types, 0)?.wrapping_add(1);
    let types_table = types.get(2..2 + usize::from(count) * 8)?;
    for entry in types_table.chunks_exact(8) {
        if &entry[..4] != kind {
            continue;
        }
        let count = usize::from(be_u16(entry, 4)?) + 1;
        let refs = usize::from(be_u16(entry, 6)?);
        for reference in types.get(refs..refs + count * 12)?.chunks_exact(12) {
            if be_u16(reference, 0)? == 128 {
                let offset = (be_u32(reference, 4)? & 0x00ff_ffff) as usize;
                let len = be_u32(data, offset)? as usize;
                let start = offset + 4;
                return data.get(start..start.checked_add(len)?);
            }
        }
    }
    None
}

fn section(bytes: &[u8], offset: u32, len: u32) -> Option<&[u8]> {
    let start = offset as usize;
    bytes.get(start..start.checked_add(len as usize)?)
}

fn be_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_scenario_resource_uses_native_widths_and_requires_enabled_counts() {
        let mut payload = [0; 52];
        payload[..6].copy_from_slice(&[0, 16, 68, 99, 0xff, 0xff]);
        payload[6..10].copy_from_slice(&45_000_u32.to_be_bytes());
        payload[30..32].copy_from_slice(&(-2_i16).to_be_bytes());
        let start = MacScenarioStart::decode(&payload[..48]).unwrap();
        assert_eq!(start.action, 16);
        assert_eq!(start.point, (68, 99));
        assert_eq!((start.goals[0], start.goals[1]), (u32::MAX, 45_000));
        assert_eq!(start.goals[7], -2_i32 as u32);
        assert_eq!(&start.goals[14..], &[0, 0]);

        payload[46] = 0xf8;
        assert!(MacScenarioStart::decode(&payload[..48]).is_none());
        payload[48..50].copy_from_slice(&0x8000_u16.to_be_bytes());
        let start = MacScenarioStart::decode(&payload).unwrap();
        assert_eq!(start.goals[12], 0xf8);
        assert_eq!(start.goals[14], i32::from(i16::MIN) as u32);
    }
}
