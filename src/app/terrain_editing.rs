//! Terrain-height editing input, placement, smoothing, and classification workers.
//!
//! Existing billing, RNG, hold/release semantics, and map-write order are preserved.
//! Mac arithmetic helpers, generation, water placement, tree tools, and rendering
//! remain in their existing modules.

use super::{
    NetworkKind, ToolPlacement, ToolPreview, UndergroundKind, ViewerState, ZONE_MILITARY,
    is_decorative_water_terrain, is_edit_map_tree_terrain, is_water_terrain, mac_terrain_editing,
    neighbor_tiles, network_drag_path, network_kinds_for_building, offset_city_tile,
    pe_destroy_building_elevated_arm_45994e, pe_destroy_building_footprint,
    pe_destroy_building_mode1_optional_crt, pe_flood_destroy_tile_with_crt,
    pe_maptool_surface_destroy_building_443461, pe_maptool_surface_xter_gate_4434de,
    pe_maptool_underground_clear_subway_443427, pe_maptool_underground_empty_refuse_443350,
    pe_place_tile_441f00, pe_place_xund_442370, render_zoom, terrain_water_level,
};
use crate::city::{AppMode, BLANK_MAP_BASE_HEIGHT, CITY_HEIGHT, CITY_WIDTH, City, TileBits};
use crate::engine_fidelity::EngineFidelity;
use crate::ui::EditTool;
use crate::win95_random::Win95CrtRand;
use glam::Vec2;

mod classification;
mod flatten;
mod lower;
mod raise;
pub(super) use raise::{
    PE_RAISE_MIDBAND_REFUSE_DIALOG_402C4D_A, PE_RAISE_MIDBAND_REFUSE_DIALOG_402C4D_B,
    PE_RAISE_MIDBAND_REFUSE_DIALOG_402C4D_C, PE_RAISE_MIDBAND_REFUSE_UI_402900_A,
    PE_RAISE_MIDBAND_REFUSE_UI_402900_B, PE_RAISE_SUBMODE_ADJUST_TOWARD, PE_RAISE_SUBMODE_DEFAULT,
    PE_RAISE_SUBMODE_HUGE, PE_RAISE_SUBMODE0_DESTROY_XBLD_MIN, PE_RAISE_SUBMODE0_MIDBAND_XBLD_MIN,
    PE_RAISE_SUBMODE0_POST_DEBIT_UI_WAIT_4012C1, PE_RAISE_TOOL0_UI_TOOL_ID,
    pe_raise_adjust_land_toward_443200_with_crt, pe_raise_drag_step_dir_445da0,
    pe_raise_epilogue_surface_reshape_kind_44c7a0,
    pe_raise_epilogue_underground_reshape_kind_44c9b0, pe_raise_land_inc_443c60_with_crt,
    pe_raise_submode0_step_billed_443270, pe_raise_tool0_clear_temp_xtxt_0xc9,
    pe_raise_tool0_stamp_temp_xtxt_0xc9, pe_raise_tool0_submode_step_with_crt,
    pe_raise_tool0_ui_press_y_byte_442ff4, pe_raise_tool0_ui_tile_xy_443003,
    pe_raise_tool0_xter_ui_byte_442e8b,
};
#[cfg(test)]
pub(super) use raise::{
    PE_RAISE_SUBMODE_ARM_VA, PE_RAISE_SUBMODE_MAX, PE_RAISE_SUBMODE0_FUND_COST,
    pe_raise_adjust_land_toward_443200, pe_raise_land_inc_443c60,
    pe_raise_submode0_clear_xbit_20_4433f0, pe_raise_submode0_clears_xund_443430,
    pe_raise_submode0_early_refuse_443270, pe_raise_submode0_midband_crt_refuse_443470,
    pe_raise_tool0_submode_step,
};

#[cfg(test)]
pub(super) use lower::{
    PeTerrainCoordinateQueue, pe_lower_444440_mode_cost, pe_lower_terrain_444440_mode_skips_funds,
    pe_lower_terrain_land_dec_444440_mode_with_submode,
};
pub(super) use lower::{
    pe_lower_terrain_crt_step_443900, pe_lower_terrain_from_packed_443900,
    pe_lower_terrain_land_dec_444440, pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt,
    pe_lower_terrain_land_dec_444440_mode_with_submode_and_crt_effects,
};

#[cfg(test)]
pub(super) use classification::pe_fun_00447020_slope_xter;
pub(super) use classification::{
    PE_447490_UI_WAIT_4012C1_PER_CALL, PE_TERRAIN_SLOPE_BITS, PE_TERRAIN_SLOPE_DX,
    PE_TERRAIN_SLOPE_DY, pe_fun_00447020_mode0_xund_slope_sea,
    pe_fun_00447020_mode1_destroy_and_city_clear_447085, pe_selected_water_height_for_fun_00447020,
    pe_terrain_neighbor_fun_00447020_447490, pe_terrain_neighbor_fun_00447020_447490_signed,
};

pub(super) use flatten::{
    PE_POST_FLATTEN_NEIGHBOR_DX, PE_POST_FLATTEN_NEIGHBOR_DY, PE_SPREAD_TOOL4_UI_TIMER_INIT,
    PE_VOLCANO_FLATTEN_DX, PE_VOLCANO_FLATTEN_DY, PE_VOLCANO_FLATTEN_NESW_DX,
    PE_VOLCANO_FLATTEN_NESW_DY, pe_post_flatten_neighbor_altm_445b50, pe_set_flatten_mark,
    pe_spread_terrain_from_packed_442a30_with_crt, pe_terrain_nesw_land_copy_442a30_with_crt,
    pe_volcano_apply_flatten_with_crt, pe_volcano_flatten_check,
};
#[cfg(test)]
pub(super) use flatten::{
    PE_POST_FLATTEN_NEIGHBOR_XBLD_GATE, PE_VOLCANO_FLATTEN_COST, PE_VOLCANO_FLATTEN_MAX_HEIGHT,
    pe_spread_terrain_from_packed_442a30, pe_terrain_nesw_land_copy_442a30,
    pe_terrain_nesw_land_copy_442a30_if_moved, pe_volcano_apply_flatten,
};

pub(super) const EDIT_MAP_STRETCH_PIXELS_PER_LEVEL: f32 = 8.0;

pub(super) const EDIT_MAP_LOWER_PIXELS_PER_LEVEL: f32 = 8.0;

/// PE edit-terrain menu writes to `0x4ca1ec` (@ `0x47deaf`..`0x47def7`).
/// Tool `0` is the default Raise path (`0x442db0`); menu slots set 1..=4.
pub(super) const PE_TERRAIN_TOOL_STRETCH: u16 = 1;

pub(super) const PE_TERRAIN_TOOL_LOWER: u16 = 2;

pub(super) const PE_TERRAIN_TOOL_LEVEL: u16 = 3;

pub(super) const PE_TERRAIN_TOOL_NESW_COPY: u16 = 4;

/// Win95 raise ALTM land++ then `0x445b50` (Stretch `0x443a50` / volcano `0x444376` pattern).
/// Refuses land `>= 0x1f`. Returns PE `0x445b50` adjust-path `0x4012c1` wait count.
pub(super) fn pe_raise_terrain_land_inc_445b50(city: &mut City, x: usize, y: usize) -> Option<u32> {
    if x >= CITY_WIDTH || y >= CITY_HEIGHT {
        return None;
    }
    let sample = city.altm_at(x, y);
    if sample.land >= 0x1f {
        return None;
    }
    city.set_altitude(x, y, sample.land + 1, sample.water);
    // Volcano flatten raise @ `0x44439e`: XZON `&= 0xf0` after land++.
    if let Some(zone) = city.zone_at(x, y) {
        city.set_zone(x, y, zone.raw_bits() & 0xf0);
    }
    Some(pe_post_flatten_neighbor_altm_445b50(city, x, y))
}

/// PE Raise tool-0 `0x444bf0` result unpack (`0x442f5d`): low byte X (`& 0xff`), high byte Y.
/// Negative AX from `0x444bf0` is out of range (`0x442f54`).
pub(super) fn pe_raise_unpack_tile_444bf0(packed: i16) -> Option<(usize, usize)> {
    if packed < 0 {
        return None;
    }
    let x = (packed as u16 & 0xff) as usize;
    let y = ((packed as u16) >> 8) as usize;
    if x >= 0x80 || y >= 0x80 {
        return None;
    }
    Some((x, y))
}

/// PE Lower tool-2 `0x444bf0` result unpack (`0x443964`): low 7 bits X (`& 0x7f`), high byte Y.
pub(super) fn pe_lower_unpack_tile_444bf0(packed: i16) -> Option<(usize, usize)> {
    if packed < 0 {
        return None;
    }
    let x = (packed as u16 & 0x7f) as usize;
    let y = ((packed as u16) >> 8) as usize;
    if x >= 0x80 || y >= 0x80 {
        return None;
    }
    Some((x, y))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TerrainDrag {
    pub(super) current: (usize, usize),
    pub(super) tool: EditTool,
    pub(super) target_altitude: u8,
    pub(super) placed_tiles: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EditMapStretchDrag {
    pub(super) tile: (usize, usize),
    pub(super) start_mouse_y: f32,
    pub(super) start_land: u8,
    pub(super) water: u8,
    /// PE hill-step count, or Mac's last requested height minus press height.
    pub(super) applied_levels: i16,
    pub(super) changed: bool,
}

/// Edit-map LowerTerrain vertical dig drag (mouse-down digs; mouse-up does not raise).
pub(super) type EditMapLowerDrag = EditMapStretchDrag;

/// Edition-specific Level hold: PE adjusts press-tile land from mouse y;
/// Mac selector 4 carries the press height along moved-to map samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EditMapLevelDrag {
    pub(super) tile: (usize, usize),
    /// PE press mouse Y (`[esp+0x20]` at `0x44495d`).
    pub(super) start_mouse_y: i16,
    /// PE press-tile land (`[esp+0x10]`).
    pub(super) base_land: u8,
    pub(super) water: u8,
    pub(super) changed: bool,
    /// Mac selector 4 copies the press height along moved-to map samples.
    pub(super) native_mac: Option<mac_terrain_editing::LevelDrag>,
}

mod stretch;
#[cfg(test)]
pub(super) use stretch::{
    pe_can_raise_stretch_hill_444ff0, pe_stretch_hold_ui_refresh_due_443ba6,
    pe_stretch_terrain_hill_step_443a50,
};
pub(super) use stretch::{
    pe_level_terrain_mouse_land_444951, pe_stretch_hill_crt_jitter,
    pe_stretch_hold_after_successful_hill_443b88, pe_stretch_terrain_from_packed_443a50,
    pe_stretch_terrain_hill_step_443a50_with_crt,
};

mod state;
