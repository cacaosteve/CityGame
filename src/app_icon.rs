//! Shared CityGame window / taskbar icon (same art as assets/CityGame.icns).
use anyhow::{Context, Result};
use sdl3::pixels::PixelFormat;
use sdl3::surface::Surface;
use sdl3::video::Window;

/// Apply the CityGame icon to an SDL window (Windows/Linux taskbar + window chrome).
/// On macOS, SDL_SetWindowIcon replaces the application's Dock icon, so leave
/// the bundle's multi-resolution `.icns` in control instead.
#[cfg(target_os = "macos")]
pub fn apply_window_icon(_window: &mut Window) {}

#[cfg(not(target_os = "macos"))]
pub fn apply_window_icon(window: &mut Window) {
    match load_icon_surface() {
        Ok(surface) => {
            if !window.set_icon(surface) {
                log::warn!("SDL_SetWindowIcon failed");
            }
        }
        Err(error) => log::warn!("CityGame window icon not applied: {error:#}"),
    }
}

fn load_icon_surface() -> Result<Surface<'static>> {
    let rgba = image::load_from_memory(include_bytes!("../assets/citygame-icon.png"))
        .context("decoding the embedded CityGame icon")?
        .into_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba.into_raw();
    // image supplies R,G,B,A bytes. RGBA8888 is a packed integer format and
    // reads these as A,B,G,R on little-endian systems (making red become alpha).
    let mut surface = Surface::new(width, height, PixelFormat::RGBA32)
        .map_err(|e| anyhow::anyhow!("Surface::new: {e}"))?;
    let pitch = surface.pitch() as usize;
    let row_bytes = width as usize * 4;
    surface.with_lock_mut(|buf| {
        for (dest, src) in buf
            .chunks_exact_mut(pitch)
            .zip(pixels.chunks_exact(row_bytes))
        {
            dest[..row_bytes].copy_from_slice(src);
        }
    });
    Ok(surface)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_surface_preserves_actual_colors_and_opacity() {
        let original = image::load_from_memory(include_bytes!("../assets/citygame-icon.png"))
            .unwrap()
            .into_rgba8();
        assert_eq!(original.get_pixel(256, 256).0, [8, 15, 19, 255]);
        assert_eq!(original.get_pixel(0, 0).0[3], 0);

        // Ask SDL to interpret the surface and convert to packed 0xRRGGBBAA.
        // Comparing native integers catches RGBA bytes mislabeled as RGBA8888.
        let surface = load_icon_surface()
            .unwrap()
            .convert_format(PixelFormat::RGBA8888)
            .unwrap();
        let pitch = surface.pitch() as usize;
        surface.with_lock(|buf| {
            for (x, y, expected) in original.enumerate_pixels() {
                let offset = y as usize * pitch + x as usize * 4;
                let actual = u32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap());
                assert_eq!(actual, u32::from_be_bytes(expected.0), "pixel {x},{y}");
            }
        });
    }
}
