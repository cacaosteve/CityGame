//! Surface and underground map-tool clear/refusal helpers.

use super::super::network::pe_reshape::pe_surface_neighbor_shape;
use super::super::terrain_editing::{
    pe_raise_epilogue_surface_reshape_kind_44c7a0,
    pe_raise_epilogue_underground_reshape_kind_44c9b0,
};
use super::super::underground::pe_reshape_underground_tile;
use super::*;

/// Win95 MapTool success tail. The surface sweep completes before the XUND sweep.
pub(in crate::app) fn pe_maptool_epilogue_44378e(city: &mut City, x: usize, y: usize) {
    for (nx, ny) in neighbor_tiles(x, y) {
        if let Some(kind) = pe_raise_epilogue_surface_reshape_kind_44c7a0(city.building_at(nx, ny))
        {
            let tile = pe_surface_neighbor_shape(city, nx, ny, kind);
            pe_place_tile_441f00(city, nx, ny, tile);
        }
    }
    for (nx, ny) in neighbor_tiles(x, y) {
        if let Some(kind) =
            pe_raise_epilogue_underground_reshape_kind_44c9b0(city.underground_at(nx, ny))
        {
            pe_reshape_underground_tile(city, nx, ny, kind);
        }
    }
}

pub(in crate::app) fn pe_clear_xtxt_sign_label(city: &mut City, xtxt: u8) {
    city.pe_401dca_clear_xtxt_resources(xtxt);
}

/// PE MapTool surface subway clear @ `0x443434`–`0x44344b`: SubStation `0xe9` or subway
/// transition `0x6c..=0x70` → PlaceXUND(0) only (no DestroyBuilding; no XUND==0x23 gate).
pub(in crate::app) fn pe_maptool_surface_clear_subway_xund_44344b(
    city: &mut City,
    x: usize,
    y: usize,
    building: u8,
) -> bool {
    if building != 0xe9 && !(0x6c..=0x70).contains(&building) {
        return false;
    }
    pe_place_xund_442370(city, x, y, 0);
    true
}

/// PE MapTool refuse @ `0x4432d2`–`0x4432df`: XZON type nibble == 7 → return 0
/// before DisplayLayer / funds / preclear.
pub(in crate::app) fn pe_maptool_refuse_xzon_type7_4432d2(city: &City, x: usize, y: usize) -> bool {
    city.zone_at(x, y)
        .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
}

/// PE MapTool surface refuse @ `0x4432fd`–`0x44330c` (DisplayLayer clear):
/// XBLD == 5 → return 0 before funds/preclear.
pub(in crate::app) fn pe_maptool_surface_refuse_xbld5_4432fd(
    city: &City,
    x: usize,
    y: usize,
) -> bool {
    city.building_at(x, y) == 5
}

/// PE MapTool surface vacant refuse @ `0x443310`–`0x443323` (DisplayLayer clear):
/// after XBLD==5 gate, if XBLD==0 and XTER (`[0x4c9f58]` row) `< 0x30` → return 0.
pub(in crate::app) fn pe_maptool_surface_vacant_refuse_44331c(
    city: &City,
    x: usize,
    y: usize,
) -> bool {
    city.building_at(x, y) == 0 && city.terrain_at(x, y) < 0x30
}

/// PE MapTool `FUN_00443270` tool-id cost table @ `0x4dc140` (dword per tool).
/// Tool 0 costs `1`; other entries exist in PE but only tool-id 0 calls this path.
pub(in crate::app) const PE_MAPTOOL_FUN_00443270_COST_TABLE_4DC140: [i32; 16] =
    [1, 25, 25, 25, 1, -1, -1, -1, -1, -1, -1, -1, 3, 100, -1, -1];

/// PE MapTool funds debit @ `0x44338a`–`0x4433af`: cost = `[tool_id*4+0x4dc140]`;
/// `cmp funds, cost` / `jl` refuse; else `sub funds, cost`. Tool-id 0 only in PE.
pub(in crate::app) fn pe_maptool_funds_debit_44338a(city: &mut City, tool_id: usize) -> bool {
    let Some(&cost) = PE_MAPTOOL_FUN_00443270_COST_TABLE_4DC140.get(tool_id) else {
        return false;
    };
    let funds = city.city_funds();
    // PE `3B C8` = `cmp ecx, eax` (funds vs cost); `7D` jge → debit.
    if funds < cost {
        return false;
    }
    city.set_city_funds(funds - cost);
    true
}

/// PE MapTool underground empty refuse @ `0x443350`–`0x443389` (DisplayLayer set):
/// if XUND==0 and tunnel_levels≠1 and XBIT piped clear → return 0 before funds/clear.
pub(in crate::app) fn pe_maptool_underground_empty_refuse_443350(
    city: &City,
    x: usize,
    y: usize,
) -> bool {
    if city.underground_at(x, y) != 0 {
        return false;
    }
    if (city.tunnel_levels_at(x, y) & 0x1f) == 1 {
        return false;
    }
    if city.tile_bits_at(x, y).is_some_and(TileBits::piped) {
        return false;
    }
    true
}

/// PE MapTool underground subway clear @ `0x4433ce`–`0x44342f` when DisplayLayer
/// (`0x4e9e48` / `show_underground`) is set: if XBLD `< 0x70` clear piped; DestroyBuilding
/// mode 1 (`0x402211`) only when tunnel_levels==1 or XUND==0x23; **always** PlaceXUND(0);
/// then epilogue. Elevated `0x3f..=0x42` use the elevated arm (`0x45994e`), not
/// DestroyStructure. When DisplayLayer is clear, return false so the surface arm @
/// `0x443434` runs.
pub(in crate::app) fn pe_maptool_underground_clear_subway_443427(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
    show_underground: bool,
) -> bool {
    if !show_underground {
        return false;
    }
    let building = city.building_at(x, y);
    if building < 0x70 {
        pe_maptool_underground_clear_piped_bit_4433fa(city, x, y);
    }
    let und = city.underground_at(x, y);
    if (city.tunnel_levels_at(x, y) & 0x1f) == 1 || und == UNDER_TILE_GREY_BOX_2 {
        // PE @ `0x44341e`: DestroyBuilding mode 1.
        pe_destroy_building_mode1_optional_crt(city, crt, x, y);
    }
    pe_place_xund_442370(city, x, y, 0);
    true
}

/// PE MapTool surface @ `0x443453`–`0x443461`: when existing XBLD `>= 0x0D`,
/// DestroyBuilding mode 1 (`0x402211` → `0x458d40`) before the SubStation /
/// SubToRail stamp. Elevated `0x3f..=0x42` use the elevated arm (`0x45994e`), not
/// DestroyStructure. Tree dialog `0x443470` is not wired here.
pub(in crate::app) fn pe_maptool_surface_destroy_building_443461(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) -> bool {
    let building = city.building_at(x, y);
    if building < 0x0d {
        return false;
    }
    pe_destroy_building_mode1_optional_crt(city, crt, x, y);
    true
}

/// PE MapTool surface PlaceTile(0) @ `0x4434d2`: when XBLD `< 0x0D` (and
/// DestroyBuilding did not run), PlaceTile(0) before the SubStation / SubToRail
/// stamp. Tree dialog refuse @ `0x443470` is deferred.
pub(in crate::app) fn pe_maptool_surface_placetile_zero_4434d2(
    city: &mut City,
    x: usize,
    y: usize,
) -> bool {
    if city.building_at(x, y) >= 0x0d {
        return false;
    }
    pe_place_tile_441f00(city, x, y, 0);
    true
}

/// PE MapTool surface XTER gate @ `0x4434de`–`0x44356d`: when XTER `>= 0x30`,
/// waterfall `0x3E` → `FUN_00447020`; else write XTER `0`. Then if DestroyBuilding
/// ran and XBIT water-tile was set → PlaceWater (`0x401884`); else XBIT `&= 0xFB`.
/// XTER `< 0x30` skips (`jc 0x44378e`). Neighbor refresh `0x4012c1` deferred.
pub(in crate::app) fn pe_maptool_surface_xter_gate_4434de(
    city: &mut City,
    x: usize,
    y: usize,
    destroyed: bool,
) -> bool {
    let terrain = city.terrain_at(x, y);
    if terrain < 0x30 {
        return false;
    }
    let water_tile = city.tile_bits_at(x, y).is_some_and(TileBits::water_tile);
    if terrain == 0x3e {
        let sea = pe_selected_water_height_for_fun_00447020(city);
        pe_fun_00447020_mode1_destroy_and_city_clear_447085(city, None, x, y);
        pe_fun_00447020_mode0_xund_slope_sea(city, x, y, sea);
    } else {
        city.set_terrain(x, y, 0);
    }
    if destroyed && water_tile {
        pe_place_water_helper_446650(city, x, y);
    } else if x < 0x80 && y < 0x80 {
        if let Some(bits) = city.tile_bits_at(x, y) {
            city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0xfb));
        }
    }
    // PE @ `0x44357b`–`0x443786`: PlaceWater on NESW+diagonal water-tile neighbors.
    // (`0x4012c1` → `0x43a520` immediately before this is deferred.)
    pe_place_water_reslope_neighbors_446a66(city, x, y);
    true
}

/// PE MapTool surface pre-stamp for SubStation / SubToRail @ `0x443434`–`0x44356d`:
/// subway XUND clear, DestroyBuilding or PlaceTile(0), then XTER/`PlaceWater`/XBIT arm.
/// Used when the underground arm (`0x443427`) did not already clear the tile.
pub(in crate::app) fn pe_maptool_surface_preclear_subway_443461(
    city: &mut City,
    crt: Option<&mut Win95CrtRand>,
    x: usize,
    y: usize,
) {
    let existing = city.building_at(x, y);
    pe_maptool_surface_clear_subway_xund_44344b(city, x, y, existing);
    let destroyed = pe_maptool_surface_destroy_building_443461(city, crt, x, y);
    if !destroyed {
        pe_maptool_surface_placetile_zero_4434d2(city, x, y);
    }
    pe_maptool_surface_xter_gate_4434de(city, x, y, destroyed);
}

/// PE MapTool underground clear @ `0x4433fa`: XBIT `&= 0xdf` (drop piped) when XBLD `< 0x70`.
pub(in crate::app) fn pe_maptool_underground_clear_piped_bit_4433fa(
    city: &mut City,
    x: usize,
    y: usize,
) {
    if let Some(bits) = city.tile_bits_at(x, y) {
        city.set_tile_bits(x, y, TileBits::new(bits.raw_bits() & 0xdf));
    }
}
