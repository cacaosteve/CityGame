#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn sc2000_install_menu(
        callback: extern "C" fn(i32),
        document_open_callback: extern "C" fn(*const std::ffi::c_char),
    );
    fn sc2000_set_edit_map_mode(edit_map: i32);
    fn sc2000_set_game_speed(speed_command: i32);
    fn sc2000_set_auto_budget(enabled: i32);
    fn sc2000_set_auto_goto(enabled: i32);
    fn sc2000_set_sound_effects(enabled: i32);
    fn sc2000_set_music(enabled: i32);
    fn sc2000_set_no_disasters(enabled: i32);
    fn sc2000_set_newspaper_subscription(enabled: i32);
    fn sc2000_set_newspaper_extra(enabled: i32);
    fn sc2000_set_balloon_help(enabled: i32);
    fn sc2000_set_graphics_preference(prefer_replacement: i32);
    fn sc2000_set_citygame_overrides(effects: i32, music: i32, city_music: i32, toolbar: i32);
    fn sc2000_set_citygame_newspaper(enabled: i32);
    fn sc2000_set_appearance_preference(preference: i32);
    fn sc2000_set_data_map_mode(mode_command: i32);
    fn sc2000_set_underground_view(enabled: i32);
    fn sc2000_pump_events();
    fn sc2000_activate_app();
    fn sc2000_configure_palette_window(palette: *mut std::ffi::c_void, main: *mut std::ffi::c_void);
    fn sc2000_order_palette_above_main(palette: *mut std::ffi::c_void, main: *mut std::ffi::c_void);
    fn sc2000_focus_main_window(main: *mut std::ffi::c_void);
    fn sc2000_present_windows(main: *mut std::ffi::c_void, palette: *mut std::ffi::c_void);
}

#[cfg(target_os = "macos")]
pub fn install(
    callback: extern "C" fn(i32),
    document_open_callback: extern "C" fn(*const std::ffi::c_char),
) {
    unsafe {
        sc2000_install_menu(callback, document_open_callback);
    }
}

#[cfg(target_os = "macos")]
pub fn set_edit_map_mode(edit_map: bool) {
    unsafe {
        sc2000_set_edit_map_mode(if edit_map { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_game_speed(speed: crate::traffic::GameSpeed) {
    let command = match speed {
        crate::traffic::GameSpeed::Paused => 20,
        crate::traffic::GameSpeed::Turtle => 21,
        crate::traffic::GameSpeed::Llama => 22,
        crate::traffic::GameSpeed::Cheetah => 23,
        crate::traffic::GameSpeed::AfricanSwallow => 24,
    };
    unsafe {
        sc2000_set_game_speed(command);
    }
}

#[cfg(target_os = "macos")]
pub fn set_auto_budget(enabled: bool) {
    unsafe {
        sc2000_set_auto_budget(if enabled { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_auto_goto(enabled: bool) {
    unsafe {
        sc2000_set_auto_goto(if enabled { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_sound_effects(enabled: bool) {
    unsafe {
        sc2000_set_sound_effects(if enabled { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_music(enabled: bool) {
    unsafe {
        sc2000_set_music(if enabled { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_no_disasters(enabled: bool) {
    unsafe {
        sc2000_set_no_disasters(if enabled { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_newspaper_subscription(enabled: bool) {
    unsafe {
        sc2000_set_newspaper_subscription(if enabled { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_newspaper_extra(enabled: bool) {
    unsafe {
        sc2000_set_newspaper_extra(if enabled { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_balloon_help(enabled: bool) {
    unsafe {
        sc2000_set_balloon_help(if enabled { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_graphics_preference(prefer_replacement: bool) {
    unsafe {
        sc2000_set_graphics_preference(if prefer_replacement { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn set_citygame_overrides(effects: bool, music: bool, city_music: bool, toolbar: bool) {
    unsafe {
        sc2000_set_citygame_overrides(
            i32::from(effects),
            i32::from(music),
            i32::from(city_music),
            i32::from(toolbar),
        );
    }
}

#[cfg(target_os = "macos")]
pub fn set_citygame_newspaper(enabled: bool) {
    unsafe {
        sc2000_set_citygame_newspaper(i32::from(enabled));
    }
}

#[cfg(target_os = "macos")]
pub fn set_appearance_preference(preference: crate::asset_profile::AppearancePreference) {
    let value = match preference {
        crate::asset_profile::AppearancePreference::System => 0,
        crate::asset_profile::AppearancePreference::Light => 1,
        crate::asset_profile::AppearancePreference::Dark => 2,
    };
    unsafe { sc2000_set_appearance_preference(value) }
}

#[cfg(target_os = "macos")]
pub fn set_data_map_mode(mode: crate::data_map::CityDataMapMode) {
    let command = match mode {
        crate::data_map::CityDataMapMode::City => 110,
        crate::data_map::CityDataMapMode::Power => 111,
        crate::data_map::CityDataMapMode::Water => 112,
        crate::data_map::CityDataMapMode::Traffic => 113,
        crate::data_map::CityDataMapMode::LandValue => 114,
        crate::data_map::CityDataMapMode::Pollution => 116,
        crate::data_map::CityDataMapMode::Crime => 117,
        crate::data_map::CityDataMapMode::Police => 118,
        crate::data_map::CityDataMapMode::Fire => 119,
        crate::data_map::CityDataMapMode::Density => 120,
        crate::data_map::CityDataMapMode::Growth => 121,
        crate::data_map::CityDataMapMode::Height => 122,
    };
    unsafe {
        sc2000_set_data_map_mode(command);
    }
}

#[cfg(target_os = "macos")]
pub fn set_underground_view(enabled: bool) {
    unsafe {
        sc2000_set_underground_view(if enabled { 1 } else { 0 });
    }
}

#[cfg(target_os = "macos")]
pub fn pump_events() {
    unsafe {
        sc2000_pump_events();
    }
}

#[cfg(target_os = "macos")]
pub fn activate_app() {
    unsafe {
        sc2000_activate_app();
    }
}

#[cfg(target_os = "macos")]
pub fn cocoa_window(window: &sdl3::video::Window) -> Option<*mut std::ffi::c_void> {
    unsafe {
        use sdl3::sys::properties::SDL_GetPointerProperty;
        let props = sdl3::sys::video::SDL_GetWindowProperties(window.raw());
        let ptr = SDL_GetPointerProperty(
            props,
            sdl3::sys::video::SDL_PROP_WINDOW_COCOA_WINDOW_POINTER,
            std::ptr::null_mut(),
        );
        if ptr.is_null() { None } else { Some(ptr) }
    }
}

#[cfg(target_os = "macos")]
pub fn configure_palette_window(main: &sdl3::video::Window, palette: &sdl3::video::Window) {
    unsafe {
        if let (Some(main_ptr), Some(palette_ptr)) = (cocoa_window(main), cocoa_window(palette)) {
            sdl3::sys::video::SDL_SetWindowAlwaysOnTop(palette.raw(), false);
            sc2000_configure_palette_window(palette_ptr, main_ptr);
        }
    }
}

#[cfg(target_os = "macos")]
pub fn order_palette_above_main(main: &sdl3::video::Window, palette: &sdl3::video::Window) {
    unsafe {
        if let (Some(main_ptr), Some(palette_ptr)) = (cocoa_window(main), cocoa_window(palette)) {
            sc2000_order_palette_above_main(palette_ptr, main_ptr);
        }
    }
}

#[cfg(target_os = "macos")]
pub fn focus_main_window(main: &sdl3::video::Window) {
    unsafe {
        if let Some(main_ptr) = cocoa_window(main) {
            sc2000_focus_main_window(main_ptr);
        }
    }
}

#[cfg(target_os = "macos")]
pub fn present_windows(main: &sdl3::video::Window, palette: &sdl3::video::Window) {
    unsafe {
        if let Some(main_ptr) = cocoa_window(main) {
            let palette_ptr = cocoa_window(palette);
            sc2000_present_windows(main_ptr, palette_ptr.unwrap_or(std::ptr::null_mut()));
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn install(
    _callback: extern "C" fn(i32),
    _document_open_callback: extern "C" fn(*const std::ffi::c_char),
) {
}

#[cfg(not(target_os = "macos"))]
pub fn set_edit_map_mode(_edit_map: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_game_speed(_speed: crate::traffic::GameSpeed) {}

#[cfg(not(target_os = "macos"))]
pub fn set_auto_budget(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_auto_goto(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_sound_effects(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_music(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_no_disasters(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_newspaper_subscription(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_newspaper_extra(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_balloon_help(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_graphics_preference(_prefer_replacement: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_citygame_overrides(_effects: bool, _music: bool, _city_music: bool, _toolbar: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_citygame_newspaper(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn set_appearance_preference(_preference: crate::asset_profile::AppearancePreference) {}

#[cfg(not(target_os = "macos"))]
pub fn set_data_map_mode(_mode: crate::data_map::CityDataMapMode) {}

#[cfg(not(target_os = "macos"))]
pub fn set_underground_view(_enabled: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn pump_events() {}

#[cfg(not(target_os = "macos"))]
pub fn activate_app() {}

#[cfg(not(target_os = "macos"))]
pub fn configure_palette_window(_main: &sdl3::video::Window, _palette: &sdl3::video::Window) {}

#[cfg(not(target_os = "macos"))]
pub fn order_palette_above_main(_main: &sdl3::video::Window, _palette: &sdl3::video::Window) {}

#[cfg(not(target_os = "macos"))]
pub fn focus_main_window(_main: &sdl3::video::Window) {}

#[cfg(not(target_os = "macos"))]
pub fn present_windows(_main: &sdl3::video::Window, _palette: &sdl3::video::Window) {}
