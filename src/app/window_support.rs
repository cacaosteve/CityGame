//! SDL window setup, initial frame presentation, and logical/pixel coordinates.
//!
//! Driver hints, display selection, retry limits, initial saved-view restoration,
//! and surface synchronization retain their existing ordering. Runtime owns the
//! SDL windows, event pump, and GPU renderer for their original lifetimes.

use super::ViewerState;
use crate::mac_menu;
use crate::renderer::Renderer;
use glam::Vec2;
use sdl3::event::{Event, WindowEvent};
use sdl3::video::WindowPos;
use std::time::Duration;

pub(super) fn raise_window(video: &sdl3::VideoSubsystem, window_id: u32) {
    unsafe {
        if let Ok(mut window) = video.window_from_id(window_id) {
            window.raise();
        }
    }
}

pub(super) fn show_window(video: &sdl3::VideoSubsystem, window_id: u32) {
    unsafe {
        if let Ok(mut window) = video.window_from_id(window_id) {
            window.show();
        }
    }
}

pub(super) fn hide_window(video: &sdl3::VideoSubsystem, window_id: u32) {
    unsafe {
        if let Ok(mut window) = video.window_from_id(window_id) {
            window.hide();
        }
    }
}

pub(super) fn sync_window(video: &sdl3::VideoSubsystem, window_id: u32) {
    unsafe {
        if let Ok(window) = video.window_from_id(window_id) {
            window.sync();
        }
    }
}

/// Prefer SDL's native Wayland driver in Wayland sessions while leaving an
/// explicit SDL driver choice (including X11) under the user's control.
pub(super) fn configure_linux_video_driver() {
    #[cfg(target_os = "linux")]
    {
        let has_explicit_driver = std::env::var_os("SDL_VIDEO_DRIVER").is_some()
            || std::env::var_os("SDL_VIDEODRIVER").is_some();
        if std::env::var_os("SDL_APP_ID").is_none() {
            if sdl3::hint::set("SDL_APP_ID", "com.citygame.CityGame") {
                log::info!("SDL application ID default: com.citygame.CityGame");
            } else {
                log::warn!("could not set SDL application ID hint");
            }
        }
        if std::env::var_os("WAYLAND_DISPLAY").is_some() && !has_explicit_driver {
            if sdl3::hint::set("SDL_VIDEO_DRIVER", "wayland") {
                log::info!("SDL video driver default: Wayland");
            } else {
                log::warn!("could not set SDL video driver hint to Wayland");
            }
        }
    }
}

pub(super) fn fit_window_to_usable_display(
    video: &sdl3::VideoSubsystem,
    window: &mut sdl3::video::Window,
) {
    let Ok(displays) = video.displays() else {
        return;
    };
    let Some(bounds) = displays
        .iter()
        .filter_map(|display| display.get_usable_bounds().ok())
        .max_by_key(|bounds| bounds.width() as u64 * bounds.height() as u64)
    else {
        return;
    };

    window.set_position(
        WindowPos::Positioned(bounds.x()),
        WindowPos::Positioned(bounds.y()),
    );
    if let Err(error) = window.set_size(bounds.width(), bounds.height()) {
        log::warn!("failed to size window to usable display bounds: {error}");
    }
}

pub(super) fn input_pixel_scale(window: &sdl3::video::Window, renderer_size: (u32, u32)) -> Vec2 {
    let (logical_width, logical_height) = window.size();
    let (pixel_width, pixel_height) = renderer_size;
    Vec2::new(
        scale_ratio(pixel_width, logical_width),
        scale_ratio(pixel_height, logical_height),
    )
}

pub(super) fn scale_ratio(pixel: u32, logical: u32) -> f32 {
    if logical == 0 {
        1.0
    } else {
        pixel as f32 / logical as f32
    }
}

pub(super) fn pixel_scale(window: &sdl3::video::Window, renderer_size: (u32, u32)) -> f32 {
    let scale = input_pixel_scale(window, renderer_size);
    scale.x.max(scale.y).max(1.0)
}

pub(super) fn bootstrap_window(
    renderer: &mut Renderer<'_>,
    event_pump: &mut sdl3::EventPump,
    window: &sdl3::video::Window,
    state: &mut ViewerState,
) {
    let mut presented = false;
    for attempt in 0..240 {
        mac_menu::pump_events();
        event_pump.pump_events();
        for event in event_pump.poll_iter() {
            if let Event::Window {
                window_id,
                win_event,
                ..
            } = event
            {
                if window_id != window.id() {
                    continue;
                }
                match win_event {
                    WindowEvent::Resized(width, height)
                    | WindowEvent::PixelSizeChanged(width, height) => {
                        sync_renderer_to_window(renderer, window);
                        let (pixel_width, pixel_height) = window.size_in_pixels();
                        renderer.resize(
                            pixel_width.max(width.max(1) as u32),
                            pixel_height.max(height.max(1) as u32),
                        );
                    }
                    WindowEvent::Maximized | WindowEvent::Restored | WindowEvent::Exposed => {
                        sync_renderer_to_window(renderer, window);
                        let (pixel_width, pixel_height) = window.size_in_pixels();
                        renderer.resize(pixel_width.max(1), pixel_height.max(1));
                    }
                    _ => {}
                }
            }
        }

        sync_renderer_to_window(renderer, window);
        if attempt == 0 {
            state.fit_map(renderer.size(), pixel_scale(window, renderer.size()));
            state.restore_saved_pe_view(renderer.size(), pixel_scale(window, renderer.size()));
        }

        let dirty = state.city.take_dirty_tiles();
        let sprites = state.build_sprites(
            renderer,
            renderer.size(),
            pixel_scale(window, renderer.size()),
            dirty,
            false,
        );
        match renderer.render(&sprites, &[], &[]) {
            Ok(true) => {
                presented = true;
                if attempt >= 8 {
                    break;
                }
            }
            Ok(false) => {
                mac_menu::activate_app();
                mac_menu::focus_main_window(window);
            }
            Err(error) => {
                log::warn!("bootstrap render attempt {attempt}: {error:#}");
            }
        }
        window.sync();

        let (width, height) = renderer.size();
        if width > 0 && height > 0 && presented && attempt >= 8 {
            break;
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    if !presented {
        log::warn!(
            "bootstrap finished without a successful frame present (window may be occluded until focused)"
        );
    }
}

pub(super) fn wait_for_window_pixels(window: &sdl3::video::Window) {
    for _ in 0..100 {
        let (width, height) = window.size_in_pixels();
        if width > 0 && height > 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
        window.sync();
    }
    log::warn!("window pixel size still zero after waiting");
}

pub(super) fn sync_renderer_to_window(renderer: &mut Renderer<'_>, window: &sdl3::video::Window) {
    window.sync();
    let (width, height) = window.size_in_pixels();
    if width > 0 && height > 0 {
        renderer.sync_surface(width, height);
    }
}
