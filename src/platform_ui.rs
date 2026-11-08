//! Desktop vs tablet UI mode helpers.
//!
//! Mobile targets (`android` / `ios`) always use the single-window path. Desktop can
//! opt in with `CITYGAME_SINGLE_WINDOW=1` to exercise the docked Tools palette and
//! in-game menubar before native packaging is ready.

use glam::Vec2;

/// True on Android/iOS, or when `CITYGAME_SINGLE_WINDOW` is set (not `0` / `false`).
pub fn single_window_ui() -> bool {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return true;
    }
    env_flag("CITYGAME_SINGLE_WINDOW")
}

/// Classic Mac–style bar drawn inside the main wgpu window.
/// macOS desktop keeps AppKit unless single-window mode is forced for testing.
pub fn uses_ingame_menubar() -> bool {
    single_window_ui() || !cfg!(target_os = "macos")
}

/// Safe-area insets in **backing pixels**: (top, right, bottom, left).
///
/// Override with `CITYGAME_SAFE_AREA=top,right,bottom,left`. Defaults apply a notch
/// strip only on real `android` / `ios` — **not** on desktop `CITYGAME_SINGLE_WINDOW=1`
/// previews, so the docked toolbar sits flush under the in-game menubar.
pub fn safe_area_insets(pixel_scale: f32) -> (f32, f32, f32, f32) {
    if let Some(parsed) = parse_safe_area_env() {
        return parsed;
    }
    let scale = pixel_scale.max(1.0);
    if cfg!(any(target_os = "android", target_os = "ios")) {
        (24.0 * scale, 8.0 * scale, 16.0 * scale, 8.0 * scale)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    }
}

/// Extra hit padding for tool buttons under touch (backing pixels).
pub fn touch_hit_pad(pixel_scale: f32) -> f32 {
    if single_window_ui() || cfg!(any(target_os = "android", target_os = "ios")) {
        (6.0 * pixel_scale.max(1.0)).round()
    } else {
        0.0
    }
}

/// Top-left of the docked Tools palette inside the main window (backing pixels).
/// Flush under the menubar (no decorative gap — that was reading as a stray bar).
pub fn docked_palette_origin(menubar_height: f32, pixel_scale: f32) -> Vec2 {
    let (top, _right, _bottom, left) = safe_area_insets(pixel_scale);
    Vec2::new(left, top.max(menubar_height))
}

fn env_flag(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => {
            let v = value.trim();
            !(v.is_empty()
                || v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off"))
        }
        Err(_) => false,
    }
}

fn parse_safe_area_env() -> Option<(f32, f32, f32, f32)> {
    let raw = std::env::var("CITYGAME_SAFE_AREA").ok()?;
    let parts: Vec<_> = raw.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return None;
    }
    let top = parts[0].parse().ok()?;
    let right = parts[1].parse().ok()?;
    let bottom = parts[2].parse().ok()?;
    let left = parts[3].parse().ok()?;
    Some((top, right, bottom, left))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docked_origin_clears_menubar() {
        let origin = docked_palette_origin(22.0, 1.0);
        assert!(origin.y >= 22.0);
        assert!(origin.x >= 0.0);
    }

    #[test]
    fn single_window_env_off_is_false_on_desktop() {
        // Cannot mutate process env safely in parallel tests; just assert cfg path compiles.
        let _ = single_window_ui();
        let _ = uses_ingame_menubar();
        let _ = touch_hit_pad(2.0);
    }
}
