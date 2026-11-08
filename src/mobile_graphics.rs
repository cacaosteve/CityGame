//! Mobile graphics density tiers chosen from device memory.
//!
//! Desktop keeps the authored @8x pack. Mobile SeedAssets bake @4x / @2x only;
//! at launch we point `AssetProfile` at the tier that fits the device.

use crate::asset_profile::{AssetProfile, GraphicsPreference};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Bumped when SeedAssets layout changes (tier folders replace full @8x).
pub const MOBILE_SEED_REVISION: &str = "2";

const LOW_MEMORY_BYTES: u64 = 2_500_000_000;
const HIGH_MEMORY_BYTES: u64 = 5_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsTier {
    /// ~2GB class: lean @2x replacements, or original-only if missing.
    Low,
    /// ~3GB class (e.g. iPad Air 3): @4x replacements.
    Mid,
    /// Roomier devices: @8x when present in the sandbox, else @4x.
    High,
}

impl GraphicsTier {
    pub fn from_physical_memory(bytes: u64) -> Self {
        if bytes < LOW_MEMORY_BYTES {
            Self::Low
        } else if bytes < HIGH_MEMORY_BYTES {
            Self::Mid
        } else {
            Self::High
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "low/@2x",
            Self::Mid => "mid/@4x",
            Self::High => "high/@8x-or-@4x",
        }
    }
}

/// Select replacement root + preference for a support directory and memory tier.
pub fn select_tier_paths(
    support: &Path,
    tier: GraphicsTier,
) -> (Option<PathBuf>, GraphicsPreference) {
    let path_8 = support.join("replacements/our-graphics");
    let path_4 = support.join("replacements/our-graphics@4x");
    let path_2 = support.join("replacements/our-graphics@2x");
    let has = |path: &Path| path.join("asset-pack.json").is_file();

    match tier {
        GraphicsTier::Low => {
            if has(&path_2) {
                (Some(path_2), GraphicsPreference::PreferReplacement)
            } else {
                (None, GraphicsPreference::PreferOriginal)
            }
        }
        GraphicsTier::Mid => {
            if has(&path_4) {
                (Some(path_4), GraphicsPreference::PreferReplacement)
            } else if has(&path_2) {
                (Some(path_2), GraphicsPreference::PreferReplacement)
            } else {
                (None, GraphicsPreference::PreferOriginal)
            }
        }
        GraphicsTier::High => {
            if has(&path_8) {
                (Some(path_8), GraphicsPreference::PreferReplacement)
            } else if has(&path_4) {
                (Some(path_4), GraphicsPreference::PreferReplacement)
            } else if has(&path_2) {
                (Some(path_2), GraphicsPreference::PreferReplacement)
            } else {
                (None, GraphicsPreference::PreferOriginal)
            }
        }
    }
}

/// Apply the memory-selected graphics tier to a loaded mobile profile.
pub fn apply_mobile_graphics_tier(profile: &mut AssetProfile) -> Result<()> {
    let override_tier = std::env::var("CITYGAME_MOBILE_GRAPHICS_TIER")
        .ok()
        .and_then(|value| match value.to_ascii_lowercase().as_str() {
            "low" | "2" | "2x" => Some(GraphicsTier::Low),
            "mid" | "4" | "4x" => Some(GraphicsTier::Mid),
            "high" | "8" | "8x" => Some(GraphicsTier::High),
            _ => None,
        });
    let memory = physical_memory_bytes().unwrap_or(3_000_000_000);
    let tier = override_tier.unwrap_or_else(|| GraphicsTier::from_physical_memory(memory));
    let support = crate::asset_profile::app_support_dir_for_seed();
    let (root, preference) = select_tier_paths(&support, tier);
    log::info!(
        "mobile graphics tier {} (physical_memory≈{:.1} GiB, override={:?}) → replacement={}",
        tier.label(),
        memory as f64 / (1024.0 * 1024.0 * 1024.0),
        override_tier.map(|value| value.label()),
        root.as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none/original".into())
    );
    profile.set_mobile_replacement_selection(root, preference)
}

pub fn physical_memory_bytes() -> Option<u64> {
    if let Ok(value) = std::env::var("CITYGAME_PHYSICAL_MEMORY_BYTES") {
        if let Ok(parsed) = value.parse::<u64>() {
            return Some(parsed);
        }
    }
    physical_memory_bytes_platform()
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
fn physical_memory_bytes_platform() -> Option<u64> {
    use std::ffi::c_void;
    use std::mem;

    unsafe extern "C" {
        fn sysctlbyname(
            name: *const i8,
            oldp: *mut c_void,
            oldlenp: *mut usize,
            newp: *mut c_void,
            newlen: usize,
        ) -> i32;
    }

    let mut size: u64 = 0;
    let mut len = mem::size_of::<u64>();
    let name = c"hw.memsize";
    let status = unsafe {
        sysctlbyname(
            name.as_ptr(),
            (&raw mut size).cast::<c_void>(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if status == 0 && size > 0 {
        Some(size)
    } else {
        None
    }
}

#[cfg(target_os = "android")]
fn physical_memory_bytes_platform() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("MemTotal:") else {
            continue;
        };
        let Some(kb) = rest
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        return Some(kb.saturating_mul(1024));
    }
    None
}

#[cfg(not(any(target_os = "ios", target_os = "macos", target_os = "android")))]
fn physical_memory_bytes_platform() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("citygame-mobile-gfx-{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_pack(root: &Path) {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("asset-pack.json"),
            r#"{"schema_version":1,"id":"t","name":"t","kind":"replacement","author":"t","license":"t","assets":[]}"#,
        )
        .unwrap();
    }

    #[test]
    fn tier_thresholds_match_plan() {
        assert_eq!(
            GraphicsTier::from_physical_memory(2_000_000_000),
            GraphicsTier::Low
        );
        assert_eq!(
            GraphicsTier::from_physical_memory(3_000_000_000),
            GraphicsTier::Mid
        );
        assert_eq!(
            GraphicsTier::from_physical_memory(8_000_000_000),
            GraphicsTier::High
        );
    }

    #[test]
    fn low_tier_prefers_2x_then_original() {
        let support = scratch("low");
        write_pack(&support.join("replacements/our-graphics@2x"));
        write_pack(&support.join("replacements/our-graphics@4x"));
        let (root, pref) = select_tier_paths(&support, GraphicsTier::Low);
        assert_eq!(pref, GraphicsPreference::PreferReplacement);
        assert!(root.unwrap().ends_with("replacements/our-graphics@2x"));

        let empty = scratch("low-empty");
        let (root, pref) = select_tier_paths(&empty, GraphicsTier::Low);
        assert!(root.is_none());
        assert_eq!(pref, GraphicsPreference::PreferOriginal);
    }

    #[test]
    fn mid_tier_prefers_4x() {
        let support = scratch("mid");
        write_pack(&support.join("replacements/our-graphics@2x"));
        write_pack(&support.join("replacements/our-graphics@4x"));
        let (root, pref) = select_tier_paths(&support, GraphicsTier::Mid);
        assert_eq!(pref, GraphicsPreference::PreferReplacement);
        assert!(root.unwrap().ends_with("replacements/our-graphics@4x"));
    }

    #[test]
    fn high_tier_prefers_8x_when_present() {
        let support = scratch("high");
        write_pack(&support.join("replacements/our-graphics"));
        write_pack(&support.join("replacements/our-graphics@4x"));
        let (root, pref) = select_tier_paths(&support, GraphicsTier::High);
        assert_eq!(pref, GraphicsPreference::PreferReplacement);
        assert!(root.unwrap().ends_with("replacements/our-graphics"));
    }
}
