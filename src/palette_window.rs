//! SDL3 floating CityGame tool-palette window.
use crate::city::AppMode;
use crate::mac_menu;
use crate::renderer::Renderer;
use crate::ui::{self, EditTool, UiAction};
use anyhow::Context;
use glam::Vec2;
use sdl3::video::Window;

/// Top-left inset for the first CityGame palette placement over the map.
pub const PALETTE_OFFSET_X: i32 = 14;
/// macOS: AppKit menu is outside the SDL window.
#[cfg(target_os = "macos")]
pub const PALETTE_OFFSET_Y: i32 = 54;
/// Linux/Windows: clear the in-game menubar (`menubar::BAR_HEIGHT_LOGICAL` ≈ 22) plus gap.
#[cfg(not(target_os = "macos"))]
pub const PALETTE_OFFSET_Y: i32 = 40;

/// Independent, draggable **CityGame Tools** window stacked above the map in this app.
///
/// When `dormant` is true (single-window / tablet UI), returns `None`: iOS/Android allow
/// only one SDL window per display, and docked tools are drawn into the main surface.
pub fn build_palette_window(
    video: &sdl3::VideoSubsystem,
    parent: &Window,
    logical_size: (u32, u32),
    dormant: bool,
) -> anyhow::Result<Option<Window>> {
    if dormant {
        log::info!("palette: dormant (single-window UI; tools drawn in main window)");
        return Ok(None);
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let window = video
        .window("CityGame Tools", logical_size.0, logical_size.1)
        .high_pixel_density()
        .metal_view()
        .hidden()
        .build()
        .map_err(|err| anyhow::anyhow!(err.to_string()))
        .context("creating palette SDL window")?;
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    let window = video
        .window("CityGame Tools", logical_size.0, logical_size.1)
        .high_pixel_density()
        .hidden()
        .build()
        .map_err(|err| anyhow::anyhow!(err.to_string()))
        .context("creating palette SDL window")?;
    if video.current_video_driver() == "wayland" {
        log::info!(
            "Wayland controls palette placement; use the CityGame Hyprland rule to float both windows"
        );
    } else {
        anchor_floating_palette(video, parent, window.id());
    }
    mac_menu::configure_palette_window(parent, &window);
    log::info!("palette: floating tool window (draggable, no close button; stacked on map)");
    Ok(Some(window))
}

pub fn anchor_floating_palette(video: &sdl3::VideoSubsystem, parent: &Window, palette_id: u32) {
    let (main_x, main_y) = parent.position();
    unsafe {
        if let Ok(mut palette) = video.window_from_id(palette_id) {
            let _ = palette.set_position(
                sdl3::video::WindowPos::Positioned(main_x + PALETTE_OFFSET_X),
                sdl3::video::WindowPos::Positioned(main_y + PALETTE_OFFSET_Y),
            );
        }
    }
}

/// Re-stack the palette above the map without stealing keyboard focus from the main window.
pub fn sync_palette_stacking(main: &Window, palette: &Window) {
    mac_menu::order_palette_above_main(main, palette);
}

/// Keep the palette above the game while either CityGame window has app focus.
/// macOS uses a floating NSWindow level plus hide-on-deactivate; other SDL platforms
/// use SDL's per-window always-on-top flag and turn it off when the app loses focus.
pub fn set_app_active(video: &sdl3::VideoSubsystem, palette: &Window, active: bool) {
    if video.current_video_driver() == "wayland" {
        // Wayland compositors own top-level stacking and intentionally ignore
        // requests to force an ordinary app window above other windows.
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = (palette, active);
    #[cfg(not(target_os = "macos"))]
    unsafe {
        let ok = sdl3::sys::video::SDL_SetWindowAlwaysOnTop(palette.raw(), active);
        if !ok {
            log::debug!("SDL could not update toolbar always-on-top state");
        }
    }
}

pub fn sync_for_mode(
    video: &sdl3::VideoSubsystem,
    palette_id: u32,
    main: &Window,
    renderer: &mut Renderer<'_>,
    mode: AppMode,
) -> anyhow::Result<(u32, u32)> {
    sync_for_mode_ex(video, palette_id, main, renderer, mode, true)
}

/// Like [`sync_for_mode`], but `stack` false keeps a dormant single-window palette hidden.
pub fn sync_for_mode_ex(
    video: &sdl3::VideoSubsystem,
    palette_id: u32,
    main: &Window,
    renderer: &mut Renderer<'_>,
    mode: AppMode,
    stack: bool,
) -> anyhow::Result<(u32, u32)> {
    let logical_size = ui::palette_logical_size(mode);
    if !stack {
        return Ok(logical_size);
    }
    unsafe {
        if let Ok(mut palette) = video.window_from_id(palette_id) {
            palette
                .set_size(logical_size.0, logical_size.1)
                .map_err(|err| anyhow::anyhow!(err.to_string()))?;
            let title = match mode {
                AppMode::CityView => "CityGame Tools",
                AppMode::EditNewMap => "CityGame Map Tools",
            };
            palette.set_title(title).ok();
            sync_renderer_to_window(renderer, &palette);
            palette.sync();
            sync_palette_stacking(main, &palette);
        }
    }
    Ok(logical_size)
}

pub fn pixel_scale(window: &Window, renderer_size: (u32, u32)) -> f32 {
    let scale = input_pixel_scale(window, renderer_size);
    scale.x.max(scale.y).max(1.0)
}

pub fn input_pixel_scale(window: &Window, renderer_size: (u32, u32)) -> Vec2 {
    let (logical_width, logical_height) = window.size();
    let (pixel_width, pixel_height) = renderer_size;
    Vec2::new(
        if logical_width == 0 {
            1.0
        } else {
            pixel_width as f32 / logical_width as f32
        },
        if logical_height == 0 {
            1.0
        } else {
            pixel_height as f32 / logical_height as f32
        },
    )
}

pub fn palette_mouse_action(
    palette: &Window,
    renderer: &Renderer<'_>,
    logical_size: (u32, u32),
    mode: AppMode,
    open_menu: Option<ui::ToolMenu>,
    x: f32,
    y: f32,
) -> Option<UiAction> {
    let screen = Vec2::new(x, y) * input_pixel_scale(palette, renderer.size());
    ui::palette_hit_test(
        screen,
        logical_size,
        pixel_scale(palette, renderer.size()),
        mode,
        open_menu,
    )
}

pub fn palette_mouse_slider(
    palette: &Window,
    renderer: &Renderer<'_>,
    logical_size: (u32, u32),
    mode: AppMode,
    open_menu: Option<ui::ToolMenu>,
    x: f32,
    y: f32,
) -> Option<(ui::EditMapSlider, u8)> {
    let screen = Vec2::new(x, y) * input_pixel_scale(palette, renderer.size());
    ui::edit_map_slider_hit_test(
        screen,
        logical_size,
        pixel_scale(palette, renderer.size()),
        mode,
        open_menu,
    )
}

pub fn palette_hover_label(
    palette: &Window,
    renderer: &Renderer<'_>,
    logical_size: (u32, u32),
    mode: AppMode,
    open_menu: Option<ui::ToolMenu>,
    x: f32,
    y: f32,
) -> Option<&'static str> {
    let screen = Vec2::new(x, y) * input_pixel_scale(palette, renderer.size());
    ui::palette_hover_label(
        screen,
        logical_size,
        pixel_scale(palette, renderer.size()),
        mode,
        open_menu,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn render_palette(
    palette: &Window,
    renderer: &mut Renderer<'_>,
    logical_size: (u32, u32),
    mode: AppMode,
    active_tool: EditTool,
    pressed: Option<UiAction>,
    open_menu: Option<ui::ToolMenu>,
    disabled: &[UiAction],
    checked: &[UiAction],
    edit_map_sliders: Option<ui::EditMapSliderValues>,
) -> anyhow::Result<bool> {
    sync_renderer_to_window(renderer, palette);
    let ps = pixel_scale(palette, renderer.size());
    let layer = ui::build_palette_ui(
        renderer,
        logical_size,
        ps,
        mode,
        active_tool,
        pressed,
        open_menu,
        disabled,
        checked,
        edit_map_sliders,
    );
    renderer.render(&[], &layer.solids, &layer.images)
}

pub fn sync_renderer_to_window(renderer: &mut Renderer<'_>, window: &Window) {
    let (pixel_width, pixel_height) = window.size_in_pixels();
    if pixel_width > 0 && pixel_height > 0 {
        renderer.sync_surface(pixel_width, pixel_height);
    }
}

pub fn wait_for_pixels(window: &Window) {
    for _ in 0..100 {
        let (width, height) = window.size_in_pixels();
        if width > 0 && height > 0 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        window.sync();
    }
}
