//! Player building placement, preflight, stamping, and shared footprint geometry.
//!
//! Existing pricing/refusal order, RNG calls, microsim allocation, map-plane writes,
//! service rebuilds, and traffic-redraw requests are preserved. Generic PlaceTile
//! and simulation growth scheduling remain in their existing modules.

use super::{
    CityInfoDialog, MARINA_BUILDING_ID, SMALL_PARK_BUILDING_ID, SUBWAY_STATION_BUILDING_ID,
    ToolPlacement, ToolPreview, UndergroundKind, ViewerState, ZONE_RESIDENTIAL_DENSE,
    ZONE_RESIDENTIAL_LIGHT, building_has_internal_pipes, building_network_access_kinds,
    building_requires_flat_land, building_requires_mixed_shoreline, building_requires_saltwater,
    clear_xbit_powerable, is_power_plant_building, is_saltwater_tile, neighbor_tiles,
    pe_fun_00464550, pe_fun_00467860, pe_place_tile_441f00, stamp_power_plant_xbit_powerable,
    stamp_xbit_powerable, xbld_tile_size,
};
use crate::city::{
    CITY_HEIGHT, CITY_WIDTH, City, TileBits, XBIT_CHOOSE_AND_PLACE, ZONE_CORNER_ALL,
    pe_choose_and_place_corner_bits,
};
use crate::engine_fidelity::EngineFidelity;
use crate::mac_random::MacAppLfsrRng;
use crate::ui::EditTool;
use crate::win95_random::{Win95CrtRand, Win95Lfsr};
use std::collections::HashSet;

const BUS_DEPOT_BUILDING_ID: u8 = 0xec;
const RAIL_DEPOT_BUILDING_ID: u8 = 0xed;

fn mac_generic_building_tool(building: u8) -> Option<EditTool> {
    match building {
        BUS_DEPOT_BUILDING_ID => Some(EditTool::BusDepot),
        RAIL_DEPOT_BUILDING_ID => Some(EditTool::RailDepot),
        SUBWAY_STATION_BUILDING_ID => Some(EditTool::SubStation),
        MARINA_BUILDING_ID => Some(EditTool::Marina),
        0xdb => Some(EditTool::Statue),
        0xdc => Some(EditTool::WaterPump),
        0xeb => Some(EditTool::WaterTower),
        0xf3 => Some(EditTool::MayorHouse),
        0xf4 => Some(EditTool::WaterTreatment),
        0xfa => Some(EditTool::Desalinization),
        0xfb => Some(EditTool::ArcologyPlymouth),
        0xfc => Some(EditTool::ArcologyForest),
        0xfd => Some(EditTool::ArcologyDarco),
        0xfe => Some(EditTool::ArcologyLaunch),
        0xff => Some(EditTool::BraunLlamaDome),
        0xc8 => Some(EditTool::PowerPlantWind),
        0xc9 => Some(EditTool::PowerPlantGas),
        0xca => Some(EditTool::PowerPlantOil),
        0xcb => Some(EditTool::PowerPlantNuclear),
        0xcc => Some(EditTool::PowerPlantSolar),
        0xcd => Some(EditTool::PowerPlantMicrowave),
        0xce => Some(EditTool::PowerPlantFusion),
        0xcf => Some(EditTool::PowerPlantCoal),
        0xd0 => Some(EditTool::CityHall),
        0xd1 => Some(EditTool::Hospital),
        0xd2 => Some(EditTool::Police),
        0xd3 => Some(EditTool::FireStation),
        0xd4 => Some(EditTool::Museum),
        0xd5 => Some(EditTool::BigPark),
        0xd6 => Some(EditTool::School),
        0xd7 => Some(EditTool::Stadium),
        0xd8 => Some(EditTool::Prison),
        0xd9 => Some(EditTool::College),
        0xda => Some(EditTool::Zoo),
        0xf5 => Some(EditTool::Library),
        SMALL_PARK_BUILDING_ID => Some(EditTool::SmallPark),
        _ => None,
    }
}

fn surrounding_tiles(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    let min_x = x.saturating_sub(1);
    let max_x = (x + 1).min(CITY_WIDTH - 1);
    let min_y = y.saturating_sub(1);
    let max_y = (y + 1).min(CITY_HEIGHT - 1);
    (min_y..=max_y).flat_map(move |ty| {
        (min_x..=max_x).filter_map(move |tx| ((tx, ty) != (x, y)).then_some((tx, ty)))
    })
}

fn building_origin_within_edges(x: usize, y: usize, size: usize) -> bool {
    // CODE 5 0x3a90 / PE 0x44057e and 0x4405c8. Coordinates here are
    // footprint origins. Larger buildings shift back one tile from the click.
    let max = match size {
        0 | 1 => return x < CITY_WIDTH && y < CITY_HEIGHT,
        2 => 125,
        _ => 127 - size,
    };
    (1..=max).contains(&x) && (1..=max).contains(&y)
}

/// Win95 PlaceTile `0x441f00` (thunk `0x40178f`): stamp single-tile XBLD; no XTRF write.
/// PE MapTool hydro orientation deltas @ `0x441408`–`0x44143a` stack words, indexed by
/// neighbor-higher ALTM land mask (`0x441537`/`0x441577`/`0x4415c0`/`0x441600`):
/// N=`0x1` (y-1), E=`0x2` (x+1), S=`0x4` (y+1), W=`0x8` (x-1). PlaceTile id is
/// `0xC6 + delta`; rotation word `0x4c942c` bit0 (`city.rotation() & 1`) xors delta
/// (`0x441606`/`0x441618`).
pub(super) const PE_HYDRO_ORIENT_DELTA: [u8; 16] = [1, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0];
// Mac CODE 5 0x422c indexes the WORD table copied from A5-0x627a.
const MAC_HYDRO_ORIENT_DELTA: [u8; 16] = [1, 1, 0, 1, 1, 0, 1, 0, 0, 1, 1, 1, 1, 0, 1, 1];

fn hydro_higher_neighbor_mask(city: &City, x: usize, y: usize) -> usize {
    let local = city.altitude_at(x, y) & 0x1f;
    let mut mask = 0u8;
    if y > 0 && (city.altitude_at(x, y - 1) & 0x1f) > local {
        mask |= 0x01;
    }
    if x + 1 < CITY_WIDTH && (city.altitude_at(x + 1, y) & 0x1f) > local {
        mask |= 0x02;
    }
    if y + 1 < CITY_HEIGHT && (city.altitude_at(x, y + 1) & 0x1f) > local {
        mask |= 0x04;
    }
    if x > 0 && (city.altitude_at(x - 1, y) & 0x1f) > local {
        mask |= 0x08;
    }
    mask.into()
}

pub(super) fn pe_hydro_building_id_441610(city: &City, x: usize, y: usize) -> u8 {
    let delta = PE_HYDRO_ORIENT_DELTA[hydro_higher_neighbor_mask(city, x, y)];
    0xc6 + (delta ^ (city.rotation() & 1))
}

fn mac_hydro_building_id(city: &City, x: usize, y: usize) -> u8 {
    let delta = MAC_HYDRO_ORIENT_DELTA[hydro_higher_neighbor_mask(city, x, y)];
    0xc6 + (delta ^ (city.rotation() & 1))
}

pub(super) fn placeable_building_for_tool(tool: EditTool) -> Option<u8> {
    match tool {
        EditTool::Trees => Some(6),
        EditTool::BusDepot => Some(BUS_DEPOT_BUILDING_ID),
        EditTool::RailDepot => Some(RAIL_DEPOT_BUILDING_ID),
        EditTool::SubStation => Some(SUBWAY_STATION_BUILDING_ID),
        EditTool::SmallPark => Some(13),
        EditTool::BigPark => Some(213),
        EditTool::Zoo => Some(218),
        EditTool::Stadium => Some(215),
        EditTool::Marina => Some(MARINA_BUILDING_ID),
        EditTool::PowerPlantCoal => Some(207),
        EditTool::PowerPlantHydro => Some(198),
        EditTool::PowerPlantOil => Some(202),
        EditTool::PowerPlantGas => Some(201),
        EditTool::PowerPlantNuclear => Some(203),
        EditTool::PowerPlantWind => Some(200),
        EditTool::PowerPlantSolar => Some(204),
        EditTool::PowerPlantMicrowave => Some(205),
        EditTool::PowerPlantFusion => Some(206),
        EditTool::Police => Some(210),
        EditTool::FireStation => Some(211),
        EditTool::Hospital => Some(209),
        EditTool::Prison => Some(216),
        EditTool::School => Some(214),
        EditTool::College => Some(217),
        EditTool::Library => Some(245),
        EditTool::Museum => Some(212),
        EditTool::WaterPump => Some(220),
        EditTool::WaterTower => Some(235),
        EditTool::WaterTreatment => Some(244),
        EditTool::Desalinization => Some(250),
        EditTool::MayorHouse => Some(243),
        EditTool::CityHall => Some(208),
        EditTool::Statue => Some(219),
        EditTool::BraunLlamaDome => Some(255),
        EditTool::ArcologyPlymouth => Some(251),
        EditTool::ArcologyForest => Some(252),
        EditTool::ArcologyDarco => Some(253),
        EditTool::ArcologyLaunch => Some(254),
        _ => None,
    }
}

/// PE CityToolPlaceSelectedBuilding @ `0x440731` → PlaceTile-size for selected buildings.
/// Hydro (`0xC6`) uses the dedicated MapTool path @ `0x441640` (XBIT OR + PlaceTile on
/// waterfall XTER), not PlaceTile-size. Other specials (subway XUND, statue XBIT clear,
/// water tees) still run as post-place side effects.
pub(super) fn pe_citytool_uses_place_tile_size_440c50(building: u8) -> bool {
    // PE CityTool @ `0x440731` PlaceTile-size for selected buildings. Hydro `0xC6` (198)
    // needs waterfall XTER `0x3E` and is excluded until that arm is mapped.
    matches!(
        building,
        SMALL_PARK_BUILDING_ID
            | 200..=220
            | 233
            | 235..=237
            | 243..=245
            | MARINA_BUILDING_ID
            | 250..=255
    )
}

/// PE CityTool post-place @ `0x440906`–`0x440949`: if click in `0..0x80`, XBIT has
/// powerable `0x80`, and `dwCityPopulation < 0xc350`, call `FUN_00467860`.
pub(super) fn pe_citytool_post_place_power_recalc_440906(
    city: &mut City,
    crt: &mut Win95CrtRand,
    x: usize,
    y: usize,
) {
    if x >= 0x80 || y >= 0x80 {
        return;
    }
    if !city.tile_bits_at(x, y).is_some_and(TileBits::powerable) {
        return;
    }
    if city.pe_dw_city_population() >= 0xc350 {
        return;
    }
    pe_fun_00467860(city, crt);
}

/// PE MapTool hydro post-place @ `0x4416d4`–`0x4416e0`: if `dwCityPopulation < 0xc350`,
/// call `FUN_00467860` (no XBIT / bounds gate), including its PE source and queue scans.
pub(super) fn pe_maptool_hydro_post_place_power_recalc_4416d4(
    city: &mut City,
    crt: &mut Win95CrtRand,
) {
    if city.pe_dw_city_population() >= 0xc350 {
        return;
    }
    pe_fun_00467860(city, crt);
}

/// PE CityTool post-place @ `0x44094e`–`0x44098d`: if click in `0..0x80`, XBIT has
/// piped `0x20`, and `dwCityPopulation < 0xc350`, call `FUN_00464550`.
pub(super) fn pe_citytool_post_place_water_recalc_44094e(city: &mut City, x: usize, y: usize) {
    if x >= 0x80 || y >= 0x80 {
        return;
    }
    if !city.tile_bits_at(x, y).is_some_and(TileBits::piped) {
        return;
    }
    if city.pe_dw_city_population() >= 0xc350 {
        return;
    }
    pe_fun_00464550(city);
}

/// PE CityTool NIMBY buildings @ `0x440624`..`0x440642`: gas `0xC9`, oil `0xCA`,
/// nuclear `0xCB`, prison `0xD8`.
pub(super) fn pe_citytool_nimby_building_440624(building: u8) -> bool {
    matches!(building, 201 | 202 | 203 | 216)
}

/// PE CityTool residential-zone census @ `0x440648`..`0x4406d5`: count XZON type
/// `1`/`2` in `[click-8, click+size+8)`. `origin` is the CityTool click (before
/// PlaceTile-size size`>2` dec), not the post-dec NW.
pub(super) fn pe_citytool_nimby_residential_count_440648(
    city: &City,
    origin_x: usize,
    origin_y: usize,
    size: usize,
) -> u16 {
    let size = size.max(1);
    let x0 = origin_x as isize - 8;
    let y0 = origin_y as isize - 8;
    let x1 = origin_x as isize + size as isize + 8;
    let y1 = origin_y as isize + size as isize + 8;
    let mut count = 0u16;
    let mut x = x0;
    while x < x1 {
        let mut y = y0;
        while y < y1 {
            if (0..0x80).contains(&x) && (0..0x80).contains(&y) {
                let zone_type = city
                    .zone_at(x as usize, y as usize)
                    .map(|zone| zone.raw_bits() & 0x0f)
                    .unwrap_or(0);
                if zone_type == ZONE_RESIDENTIAL_LIGHT || zone_type == ZONE_RESIDENTIAL_DENSE {
                    count = count.saturating_add(1);
                }
            }
            y += 1;
        }
        x += 1;
    }
    count
}

/// PE CityTool NIMBY gate @ `0x4406d7`..`0x4406e7`: `0x401cad(0xc8) < count` refuses
/// (dialog arm skipped). Returns true when placement must abort.
pub(super) fn pe_citytool_nimby_gate_refuses_4406e7(
    lfsr: &mut Win95Lfsr,
    residential_count: u16,
) -> bool {
    lfsr.random_mod(0xc8) < residential_count
}

/// Win95 PlaceTile-size helper `0x440c50`: stamp `size×size` via PlaceTile `0x441f00`; no XTRF write.
/// Preflight (@ `0x440d25`..): refuse footprint XBLD `>= 0x1D`, `5`, `0x0D`, XZON type `7`,
/// XTER ≠ 0, or XBIT water-tile (non-`0xF8` place). Placing `0x0D` also refuses existing
/// XBLD `>= 0x0D` (@ `0x440e8f`). For `0xF8` (marina), count XBIT water-tile cells and refuse
/// when the count is `0` or `9` (@ `0x440e3e`..`0x440e47`) — skip the PE dialog, keep the gate.
/// PE @ `0x440ea0`→`0x44cf60`: after preflight, allocate microsim; per tile write XTXT when
/// allocation succeeds (`0x440fa7`). Per tile (@ `0x440f14`/`0x440f43`): XBIT `&= 0x1f` then
/// `|=` mask (`0xe0`, or `0x20` for XBLD `0x0D`/`0xD5`); then PlaceTile; then XZON clear.
/// Size-1 epilogue (@ `0x440fe3`/`0x441027`) ORs `ZONE_CORNER_ALL` on the origin. Size>1
/// epilogue (@ `0x441030`..) ORs rotation corner bits from table `0x4dc4d0` onto the
/// footprint corners (no zone type).
///
/// Callers pass the **post-`0x440c64` NW origin**. PE PlaceTile-size decs click `x/y`
/// when caller size `> 2` (`dec size` then `cmp size,1` / `jg` path @ `0x440c58`–`0x440c69`);
/// size 1–2 keep click as NW. CityTool click edges (`0x44057e` size 2, `0x4405c8` size≥3)
/// are paired in `building_placement_at` on that NW convention — do not dec here.
pub(super) fn pe_place_tile_size_440c50(
    city: &mut City,
    x: usize,
    y: usize,
    building: u8,
    size: usize,
) -> bool {
    pe_place_tile_size_440c50_ex(city, x, y, building, size, false)
}

pub(super) fn pe_place_tile_size_440c50_with_crt(
    city: &mut City,
    x: usize,
    y: usize,
    building: u8,
    size: usize,
    crt: &mut Win95CrtRand,
) -> bool {
    pe_place_tile_size_440c50_ex_with_crt(city, x, y, building, size, false, Some(crt))
}

/// PlaceTile-size with optional military-growth type-7 bypass.
///
/// Stock PlaceTile-size refuses XZON type 7 @ `0x440d49`. PE SimulationGrow army
/// place wrapper @ `0x4376b0`/`0x4376bd` skips the pre-place validation when the
/// zone-type arg is 7, so military growth can stamp on type-7 pads.
pub(super) fn pe_place_tile_size_440c50_ex(
    city: &mut City,
    x: usize,
    y: usize,
    building: u8,
    size: usize,
    allow_military_zone: bool,
) -> bool {
    pe_place_tile_size_440c50_ex_with_crt(city, x, y, building, size, allow_military_zone, None)
}

pub(super) fn pe_place_tile_size_440c50_ex_with_crt(
    city: &mut City,
    x: usize,
    y: usize,
    building: u8,
    size: usize,
    allow_military_zone: bool,
    mut crt: Option<&mut Win95CrtRand>,
) -> bool {
    let size = size.max(1);
    let tiles = match building_footprint_from_origin(x, y, size) {
        Some(tiles) => tiles,
        None => return false,
    };
    let mut water_tiles: u16 = 0;
    for &(tx, ty) in &tiles {
        // PE @ `0x440cc5`–`0x440cdb`: size≥2 (post-dec size>0) requires each tile in
        // `1..=0x7e`. Size-1 path @ `0x440ce7`–`0x440d08` allows `0..0x80` only —
        // do not apply the multi-tile edge refuse to 1×1.
        if size > 1 && (!(1..=0x7e).contains(&tx) || !(1..=0x7e).contains(&ty)) {
            return false;
        }
        let existing = city.building_at(tx, ty);
        // PE @ `0x440d25` / `0x440d2e` / `0x440d37`.
        if existing >= 0x1d || existing == 5 || existing == 0x0d {
            return false;
        }
        let zone_type = city
            .zone_at(tx, ty)
            .map(|zone| zone.raw_bits() & 0x0f)
            .unwrap_or(0);
        if zone_type == 7 && !allow_military_zone {
            return false;
        }
        let is_water = city.tile_bits_at(tx, ty).is_some_and(TileBits::water_tile);
        if building == 0xf8 {
            // PE @ `0x440d55`..`0x440d81`: marina counts water-tile XBIT; dry cells still need XTER 0.
            if is_water {
                water_tiles = water_tiles.saturating_add(1);
            } else if city.terrain_at(tx, ty) != 0 {
                return false;
            }
        } else {
            if city.terrain_at(tx, ty) != 0 {
                return false;
            }
            if is_water {
                return false;
            }
        }
    }
    // PE @ `0x440e3e`..`0x440e47`: marina refuses water-count 0 or 9 (dialog arm skipped).
    if building == 0xf8 && (water_tiles == 0 || water_tiles == 9) {
        return false;
    }
    // PE @ `0x440e79`: placing park refuses when origin XBLD already `>= 0x0D`.
    if building == 0x0d && city.building_at(x, y) >= 0x0d {
        return false;
    }
    // PE @ `0x440ea0`→`0x44cf60`: allocate once before the stamp loop.
    let microsim_xtxt = if let Some(crt) = crt.as_deref_mut() {
        city.provision_microsim_for_building_with_crt(building, crt)
    } else {
        city.provision_microsim_for_building(building)
    };
    let xbit_or: u8 = if building == 0x0d || building == 0xd5 {
        0x20
    } else {
        XBIT_CHOOSE_AND_PLACE
    };
    for &(tx, ty) in &tiles {
        let bits = city.tile_bits_at(tx, ty).unwrap_or_default().raw_bits();
        city.set_tile_bits(tx, ty, TileBits::new((bits & 0x1f) | xbit_or));
        pe_place_tile_441f00(city, tx, ty, building);
        city.set_zone(tx, ty, 0);
        // PE @ `0x440fa7`/`0x440fb8`: write XTXT when microsim allocate returned nonzero.
        if let Some(text_id) = microsim_xtxt {
            city.set_text_overlay(tx, ty, text_id);
        }
    }
    if size == 1 {
        // PE @ `0x440fe3`/`0x441027`: size-1 ORs `ZONE_CORNER_ALL` on the origin.
        city.set_zone(x, y, ZONE_CORNER_ALL);
    } else {
        // PE @ `0x441030`..: size>1 ORs rotation corner bits from `0x4dc4d0` (no zone type).
        let corner_bits = pe_choose_and_place_corner_bits(city.rotation());
        for &(tx, ty) in &tiles {
            let corners = building_zone_bits_for_tile(x, y, tx, ty, size, corner_bits);
            if corners != 0 {
                city.set_zone(tx, ty, corners);
            }
        }
    }
    true
}

/// PE PlaceTile-size click → post-`0x440c64` NW: size `> 2` decs x/y; size 1–2 unchanged.
pub(super) fn pe_place_tile_size_click_to_nw(
    click_x: usize,
    click_y: usize,
    size: usize,
) -> Option<(usize, usize)> {
    if size > 2 {
        Some((click_x.checked_sub(1)?, click_y.checked_sub(1)?))
    } else {
        Some((click_x, click_y))
    }
}

/// Inverse of `pe_place_tile_size_click_to_nw`: CityTool click from post-dec NW.
/// Size `> 2` → `NW+1`; size 1–2 unchanged. Used by NIMBY census (`0x440648`).
pub(super) fn pe_place_tile_size_nw_to_click(
    nw_x: usize,
    nw_y: usize,
    size: usize,
) -> (usize, usize) {
    if size > 2 {
        (nw_x.saturating_add(1), nw_y.saturating_add(1))
    } else {
        (nw_x, nw_y)
    }
}

pub(super) fn reward_mask_for_tool(tool: EditTool) -> Option<u32> {
    match tool {
        EditTool::MayorHouse => Some(1 << 0),
        EditTool::CityHall => Some(1 << 1),
        EditTool::Statue => Some(1 << 2),
        EditTool::BraunLlamaDome => Some(1 << 3),
        EditTool::Arcologies
        | EditTool::ArcologyPlymouth
        | EditTool::ArcologyForest
        | EditTool::ArcologyDarco
        | EditTool::ArcologyLaunch => Some(1 << 4),
        _ => None,
    }
}

pub(super) fn building_footprint_from_origin(
    origin_x: usize,
    origin_y: usize,
    size: usize,
) -> Option<Vec<(usize, usize)>> {
    if origin_x + size > CITY_WIDTH || origin_y + size > CITY_HEIGHT {
        return None;
    }

    Some(
        (origin_y..origin_y + size)
            .flat_map(|y| (origin_x..origin_x + size).map(move |x| (x, y)))
            .collect(),
    )
}

pub(super) fn clipped_building_footprint_from_origin(
    origin_x: usize,
    origin_y: usize,
    size: usize,
) -> Vec<(usize, usize)> {
    if origin_x >= CITY_WIDTH || origin_y >= CITY_HEIGHT {
        return Vec::new();
    }
    let max_x = (origin_x + size).min(CITY_WIDTH);
    let max_y = (origin_y + size).min(CITY_HEIGHT);
    (origin_y..max_y)
        .flat_map(|y| (origin_x..max_x).map(move |x| (x, y)))
        .collect()
}

pub(super) fn footprint_origin(footprint: &[(usize, usize)]) -> Option<(usize, usize)> {
    let min_x = footprint.iter().map(|&(x, _)| x).min()?;
    let min_y = footprint.iter().map(|&(_, y)| y).min()?;
    Some((min_x, min_y))
}

pub(super) fn building_zone_bits_for_tile(
    origin_x: usize,
    origin_y: usize,
    x: usize,
    y: usize,
    size: usize,
    corner_bits: [u8; 4],
) -> u8 {
    if size <= 1 {
        return ZONE_CORNER_ALL;
    }

    let right = origin_x + size - 1;
    let bottom = origin_y + size - 1;
    match (x, y) {
        (tx, ty) if tx == origin_x && ty == origin_y => corner_bits[0],
        (tx, ty) if tx == right && ty == origin_y => corner_bits[1],
        (tx, ty) if tx == origin_x && ty == bottom => corner_bits[2],
        (tx, ty) if tx == right && ty == bottom => corner_bits[3],
        _ => 0,
    }
}

pub(super) fn building_preview_anchor_tile(
    origin_x: usize,
    origin_y: usize,
    size: usize,
    active_corner: u8,
    corner_bits: [u8; 4],
) -> (usize, usize) {
    if size <= 1 {
        return (origin_x, origin_y);
    }

    let right = origin_x + size - 1;
    let bottom = origin_y + size - 1;
    if active_corner == corner_bits[0] {
        (origin_x, origin_y)
    } else if active_corner == corner_bits[1] {
        (right, origin_y)
    } else if active_corner == corner_bits[2] {
        (origin_x, bottom)
    } else {
        (right, bottom)
    }
}

pub(super) fn is_hydro_power_terrain(terrain: u8) -> bool {
    // PE MapTool hydro `FUN_00441400` @ `0x44146e`–`0x4414a8`: XTER `0x2E` or `0x3E`.
    matches!(terrain, 0x2e | 0x3e)
}

impl ViewerState {
    pub(super) fn player_building_origin(
        &self,
        x: usize,
        y: usize,
        building: u8,
    ) -> Option<(usize, usize)> {
        // CODE 5 0x3d80 shifts footprints larger than 2×2 back from the click.
        if !self.engine_fidelity.uses_win95()
            && mac_generic_building_tool(building).is_some()
            && xbld_tile_size(building) > 2
        {
            Some((x.checked_sub(1)?, y.checked_sub(1)?))
        } else {
            Some((x, y))
        }
    }

    pub(super) fn building_placement_at(&self, x: usize, y: usize, building: u8) -> ToolPlacement {
        let size = usize::from(xbld_tile_size(building));
        let preview = ToolPreview::Building(building);
        let Some(footprint) = building_footprint_from_origin(x, y, size) else {
            return ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: clipped_building_footprint_from_origin(x, y, size),
                preview,
            };
        };

        if !building_origin_within_edges(x, y, size) {
            return ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: footprint,
                preview,
            };
        }

        if !self.engine_fidelity.uses_win95() && mac_generic_building_tool(building).is_some() {
            let marina = building == MARINA_BUILDING_ID;
            if marina {
                // CODE 5 0x3e12 counts water flags across the footprint.
                let water_tiles = self.footprint_water_tile_count(&footprint);
                if water_tiles == 0 || water_tiles == footprint.len() {
                    return ToolPlacement {
                        valid_tiles: Vec::new(),
                        invalid_tiles: footprint,
                        preview,
                    };
                }
            }
            // Mac CODE 5 0x3d54 admits eligible XBLD on military land.
            // Marina bypasses the per-tile XTER and water rejection.
            let (valid_tiles, invalid_tiles) = footprint.into_iter().partition(|&(tx, ty)| {
                let existing = self.city.building_at(tx, ty);
                existing < 0x1d
                    && !matches!(existing, 5 | SMALL_PARK_BUILDING_ID)
                    && (building != SMALL_PARK_BUILDING_ID || existing < SMALL_PARK_BUILDING_ID)
                    && (marina
                        || (self.city.terrain_at(tx, ty) == 0
                            && !self
                                .city
                                .tile_bits_at(tx, ty)
                                .is_some_and(TileBits::water_tile)))
            });
            return ToolPlacement {
                valid_tiles,
                invalid_tiles,
                preview,
            };
        }

        if building_requires_saltwater(building) && !self.footprint_borders_saltwater(&footprint) {
            return ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: footprint,
                preview,
            };
        }

        if building_requires_mixed_shoreline(building) {
            let water_tiles = self.footprint_water_tile_count(&footprint);
            if water_tiles == 0 || water_tiles == footprint.len() {
                return ToolPlacement {
                    valid_tiles: Vec::new(),
                    invalid_tiles: footprint,
                    preview,
                };
            }
        }

        let required_land_height = if building_requires_flat_land(building) {
            Some(self.city.altitude_at(x, y))
        } else {
            None
        };
        let mut valid_tiles = Vec::new();
        let mut invalid_tiles = Vec::new();
        for &(tx, ty) in &footprint {
            if self.can_place_building_footprint_tile(tx, ty, building, required_land_height) {
                valid_tiles.push((tx, ty));
            } else {
                invalid_tiles.push((tx, ty));
            }
        }

        ToolPlacement {
            valid_tiles,
            invalid_tiles,
            preview,
        }
    }

    pub(super) fn footprint_water_tile_count(&self, footprint: &[(usize, usize)]) -> usize {
        // PE PlaceTile-size marina @ `0x440d55`: count XBIT water-tile bit `0x04`, not XTER.
        footprint
            .iter()
            .filter(|&&(x, y)| {
                self.city
                    .tile_bits_at(x, y)
                    .is_some_and(TileBits::water_tile)
            })
            .count()
    }

    pub(super) fn can_place_building_footprint_tile(
        &self,
        x: usize,
        y: usize,
        building: u8,
        required_land_height: Option<u8>,
    ) -> bool {
        if building_requires_mixed_shoreline(building) {
            return self.can_place_marina_tile(x, y);
        }
        if !self.can_place_building_tile(x, y) {
            return false;
        }
        // PE PlaceTile-size @ `0x440e79`: SmallPark refuses when XBLD already `>= 0x0D`
        // (mid `0x0E..0x1C` passes general preflight but fails this park arm).
        if building == 0x0d && self.city.building_at(x, y) >= 0x0d {
            return false;
        }
        if let Some(height) = required_land_height {
            return self.city.terrain_at(x, y) == 0 && self.city.altitude_at(x, y) == height;
        }
        true
    }

    pub(super) fn can_place_marina_tile(&self, x: usize, y: usize) -> bool {
        if self.city.dispatch_actor_at(x, y).is_some() {
            return false;
        }
        let existing = self.city.building_at(x, y);
        // PE PlaceTile-size @ `0x440d25`/`0x440d2e`/`0x440d37` (marina included).
        if existing >= 0x1d || existing == 5 || existing == 0x0d {
            return false;
        }
        // PE PlaceTile-size @ `0x440d40`: type nibble 7 refuses (marina included).
        let zone_type = self
            .city
            .zone_at(x, y)
            .map(|zone| zone.raw_bits() & 0x0f)
            .unwrap_or(0);
        if zone_type == 7 {
            return false;
        }
        // PE PlaceTile-size marina @ `0x440d55`..`0x440d81`: water-tile cells count;
        // dry cells still need XTER 0 (bare water XTER without the bit is refused).
        if self
            .city
            .tile_bits_at(x, y)
            .is_some_and(TileBits::water_tile)
        {
            return true;
        }
        self.city.terrain_at(x, y) == 0
    }

    pub(super) fn footprint_borders_saltwater(&self, footprint: &[(usize, usize)]) -> bool {
        footprint
            .iter()
            .copied()
            .flat_map(|(x, y)| surrounding_tiles(x, y))
            .any(|(x, y)| is_saltwater_tile(&self.city, x, y))
    }

    pub(super) fn can_place_building_tile(&self, x: usize, y: usize) -> bool {
        // PE PlaceTile-size @ `0x440db8`: refuse XBIT water-tile (non-marina path).
        if self
            .city
            .tile_bits_at(x, y)
            .is_some_and(TileBits::water_tile)
        {
            return false;
        }
        // PE PlaceTile-size @ `0x440d40`: refuse XZON type nibble 7 (military).
        let zone_type = self
            .city
            .zone_at(x, y)
            .map(|zone| zone.raw_bits() & 0x0f)
            .unwrap_or(0);
        if zone_type == 7 {
            return false;
        }
        // PE PlaceTile-size @ `0x440d89`: refuse any non-flat XTER (non-marina path).
        if self.city.terrain_at(x, y) != 0 {
            return false;
        }
        if self.city.dispatch_actor_at(x, y).is_some() {
            return false;
        }
        // PE PlaceTile-size @ `0x440d25`/`0x440d2e`/`0x440d37`: refuse XBLD `>=0x1D`, `5`, `0x0D`.
        // Trees/rubble/`0x0E..0x1C` may be overwritten.
        let building = self.city.building_at(x, y);
        !(building >= 0x1d || building == 5 || building == 0x0d)
    }

    pub(super) fn hydro_power_placement_at(&self, x: usize, y: usize) -> ToolPlacement {
        let building = if self.engine_fidelity.uses_win95() {
            pe_hydro_building_id_441610(&self.city, x, y)
        } else {
            mac_hydro_building_id(&self.city, x, y)
        };
        let preview = ToolPreview::Building(building);
        if self.can_place_hydro_power_tile(x, y) {
            ToolPlacement::valid(preview, vec![(x, y)])
        } else {
            ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview,
            }
        }
    }

    pub(super) fn can_place_hydro_power_tile(&self, x: usize, y: usize) -> bool {
        is_hydro_power_terrain(self.city.terrain_at(x, y)) && self.city.building_at(x, y) == 0
    }

    pub(super) fn place_hydro_power_at(&mut self, x: usize, y: usize) -> usize {
        let placement = self.hydro_power_placement_at(x, y);
        if placement.invalid_tiles.is_empty() {
            if !self.can_afford_tool_build_cost(EditTool::PowerPlantHydro, 1) {
                return 0;
            }
            if !self.engine_fidelity.uses_win95() {
                // CODE 5 0x40f0 debits before stamping, then allocates XMIC
                // and refreshes power below the Mac weighted growth threshold.
                let building = mac_hydro_building_id(&self.city, x, y);
                self.debit_tool_build_cost(EditTool::PowerPlantHydro, 1);
                stamp_xbit_powerable(&mut self.city, x, y);
                pe_place_tile_441f00(&mut self.city, x, y, building);
                self.city.set_zone(x, y, ZONE_CORNER_ALL);
                if let Some(text) = self.provision_microsim_for_building_by_engine(building) {
                    self.city.set_text_overlay(x, y, text);
                }
                self.mac_power_tool_post_place_recalc();
                self.pending_traffic_redraw = true;
                self.query_is_inspect = false;
                self.refresh_status_line();
                return 1;
            }
            // PE MapTool hydro @ `0x441640`: OR XBIT powerable `0x80`, then PlaceTile
            // (`0x40178f`→`0x441f00`) with neighbor-orient id `0xC6+delta` (`0x441610`);
            // @ `0x441679`/`0x44169e`: XZON `&=0xf0` then `|=0xf0`.
            let building = pe_hydro_building_id_441610(&self.city, x, y);
            for &(tx, ty) in &placement.valid_tiles {
                stamp_xbit_powerable(&mut self.city, tx, ty);
                pe_place_tile_441f00(&mut self.city, tx, ty, building);
                let zone = self.city.zone_at(tx, ty).map(|z| z.raw_bits()).unwrap_or(0);
                self.city.set_zone(tx, ty, (zone & 0xf0) | ZONE_CORNER_ALL);
            }
            // PE MapTool hydro @ `0x4416b7` → microsim allocate `0x44cf60`; on success write XTXT.
            if let Some(text_id) = self.provision_microsim_for_building_by_engine(building) {
                for &(tx, ty) in &placement.valid_tiles {
                    self.city.set_text_overlay(tx, ty, text_id);
                }
            }
            // PE MapTool hydro @ `0x4416d4`–`0x4416e0`: pop `< 0xc350` → `FUN_00467860`.
            pe_maptool_hydro_post_place_power_recalc_4416d4(
                &mut self.city,
                &mut self.pe_process_crt,
            );
            self.clear_zone_growth_for_player_tiles(&placement.valid_tiles);
            self.apply_building_underground_side_effects(x, y, building);
            self.refresh_building_network_neighbors(&placement.valid_tiles, building);
            // PE hydro path: no CityTool water twin / pump scan here.
            self.refresh_live_misc_current_values();
            self.refresh_budget_service_rows_from_live_counts();
            self.refresh_reward_availability();
            self.debit_tool_build_cost(EditTool::PowerPlantHydro, 1);
            self.query_is_inspect = false;
            self.refresh_status_line();
            1
        } else {
            0
        }
    }

    pub(super) fn place_building_at(&mut self, x: usize, y: usize, building: u8) -> usize {
        if !self.engine_fidelity.uses_win95()
            && let Some(tool) = mac_generic_building_tool(building)
        {
            return self.place_mac_generic_building(x, y, building, tool);
        }
        let placement = self.building_placement_at(x, y, building);
        let size = usize::from(xbld_tile_size(building));
        if placement.invalid_tiles.is_empty() && placement.valid_tiles.len() == size * size {
            if !self.can_afford_tool_build_cost(self.tool, 1) {
                return 0;
            }
            // PE CityTool NIMBY gate @ `0x440624`..`0x4406e7` before PlaceTile-size.
            // Census @ `0x440648` uses the CityTool click (`ebp+8`/`+0xc`), not the
            // post-`0x440c64` NW that `place_building_at` passes for size `>2`.
            if pe_citytool_nimby_building_440624(building) {
                let (click_x, click_y) = pe_place_tile_size_nw_to_click(x, y, size);
                let residential =
                    pe_citytool_nimby_residential_count_440648(&self.city, click_x, click_y, size);
                if pe_citytool_nimby_gate_refuses_4406e7(&mut self.pe_process_lfsr, residential) {
                    return 0;
                }
            }
            // PE CityTool SubStation @ `0x44088d` / SubToRail transitions: PlaceTile-size
            // then PlaceSubway/PlaceXUND via underground side-effects — not MapTool
            // `FUN_00443270` (`0x442eb9` is tool-id 0 only).
            // PE CityToolPlaceSelectedBuilding @ `0x440731` → PlaceTile-size `0x4027f2`.
            if pe_citytool_uses_place_tile_size_440c50(building) {
                if !pe_place_tile_size_440c50_with_crt(
                    &mut self.city,
                    x,
                    y,
                    building,
                    size,
                    &mut self.pe_process_crt,
                ) {
                    return 0;
                }
                // PE CityToolPlaceSelectedBuilding statue arm @ `0x440835`: clear XBIT 0x80.
                if building == 219 {
                    for &(tx, ty) in &placement.valid_tiles {
                        clear_xbit_powerable(&mut self.city, tx, ty);
                    }
                }
            } else {
                self.stamp_building_footprint(x, y, building, &placement.valid_tiles);
            }
            self.clear_zone_growth_for_player_tiles(&placement.valid_tiles);
            self.apply_building_underground_side_effects(x, y, building);
            self.refresh_building_network_neighbors(&placement.valid_tiles, building);
            // PE CityTool post-place @ `0x440906`–`0x440949` → `FUN_00467860`.
            pe_citytool_post_place_power_recalc_440906(
                &mut self.city,
                &mut self.pe_process_crt,
                x,
                y,
            );
            // PE CityTool post-place @ `0x44094e`–`0x44098d` → `FUN_00464550`.
            pe_citytool_post_place_water_recalc_44094e(&mut self.city, x, y);
            // FUN_00464550 also republishes served-water bits through the current
            // CityGame allocator; it does not rebuild XBIT pipe membership here.
            self.refresh_live_misc_current_values();
            self.refresh_budget_service_rows_from_live_counts();
            self.refresh_reward_availability();
            self.debit_tool_build_cost(self.tool, 1);
            self.query_is_inspect = false;
            self.refresh_status_line();
            placement.valid_tiles.len()
        } else {
            0
        }
    }

    fn place_mac_generic_building(
        &mut self,
        x: usize,
        y: usize,
        building: u8,
        tool: EditTool,
    ) -> usize {
        if !self.can_afford_tool_build_cost(tool, 1) {
            return 0;
        }
        let size = usize::from(xbld_tile_size(building));
        let click_offset = usize::from(size > 2);
        let (click_x, click_y) = (x + click_offset, y + click_offset);
        if matches!(building, 0xc9..=0xcb | 0xcf | 0xd8 | 0xf4)
            && building_origin_within_edges(x, y, size)
        {
            // CODE 5 0x3b26 counts residential XZON before footprint validation,
            // using the same window as PE. CODE 13 export 203 uses the Mac LFSR.
            let residential =
                pe_citytool_nimby_residential_count_440648(&self.city, click_x, click_y, size);
            let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
            if lfsr.bounded(200) < residential {
                return 0;
            }
        }
        let placement = self.building_placement_at(x, y, building);
        if !placement.invalid_tiles.is_empty() || placement.valid_tiles.len() != size * size {
            return 0;
        }

        // Mac CODE 5 0x39f2 attempts one microsim allocation before
        // writing the footprint in x-major order.
        let text = self
            .city
            .provision_mac_microsim_for_building(building, &mut self.mac_process_rng);
        let corners = pe_choose_and_place_corner_bits(self.city.rotation());
        let service_flags = if matches!(tool, EditTool::SmallPark | EditTool::BigPark) {
            0x20
        } else {
            0xe0
        };
        for tx in x..x + size {
            for ty in y..y + size {
                let bits = self.city.tile_bits_at(tx, ty).map_or(0, TileBits::raw_bits);
                self.city
                    .set_tile_bits(tx, ty, TileBits::new((bits & 0x1f) | service_flags));
                pe_place_tile_441f00(&mut self.city, tx, ty, building);
                self.city.set_zone(
                    tx,
                    ty,
                    building_zone_bits_for_tile(x, y, tx, ty, size, corners),
                );
                if let Some(text) = text {
                    self.city.set_text_overlay(tx, ty, text);
                }
            }
        }
        self.debit_tool_build_cost(tool, 1);
        super::budget::mac_budget_add_civic_placement(&mut self.city, building);
        if tool == EditTool::Stadium && text.is_some() {
            let mask = self.city.mac_state_misc_u32_at(0x1028).unwrap_or(0) as u16;
            let available = !mask & 0x1f;
            if available != 0 {
                self.open_city_info_dialog(CityInfoDialog::StadiumTeam {
                    x: click_x,
                    y: click_y,
                    available,
                });
                return size * size;
            }
        }
        match tool {
            EditTool::Statue => {
                // CODE 5 0x3c9a removes power membership before the service tail.
                clear_xbit_powerable(&mut self.city, click_x, click_y);
            }
            EditTool::WaterPump => {
                // CODE 5 0x3c64 calls the normal Mac pipe connector (export 196).
                super::underground::mac_place_underground_tile(
                    &mut self.city,
                    (x, y),
                    UndergroundKind::Pipes,
                );
            }
            EditTool::SubStation => {
                // CODE 5 0x3c6e updates XUND and clears pipe membership before services.
                self.place_mac_subway_transition_underground(x, y);
            }
            _ => {}
        }
        // The generic service tail tests the original click, which is one
        // tile inside the written footprint for size 3 and larger.
        self.finish_mac_building_services(click_x, click_y);
        if tool == EditTool::RailDepot {
            // CODE 5 0x310a calls export 120 immediately on placement success.
            let mut lfsr = MacAppLfsrRng::new(MacAppLfsrRng::INITIAL_STATE);
            super::mac_loop_scheduler::rail_consist::mac_spawn_rail_near_station(
                &mut self.city,
                x as i16,
                y as i16,
                || lfsr.bounded(4),
                || self.mac_game_rng.bounded(2),
            );
        }
        self.pending_traffic_redraw = true;
        self.query_is_inspect = false;
        self.refresh_status_line();
        size * size
    }

    fn finish_mac_building_services(&mut self, x: usize, y: usize) {
        if self
            .city
            .tile_bits_at(x, y)
            .is_some_and(TileBits::powerable)
        {
            self.mac_power_tool_post_place_recalc();
        }
        if self.city.tile_bits_at(x, y).is_some_and(TileBits::piped) {
            self.mac_pipe_tool_post_place_water_recalc();
        }
    }

    pub(super) fn finish_mac_stadium_team(&mut self, team: u8) -> bool {
        let Some(CityInfoDialog::StadiumTeam { x, y, available }) = self.city_info_dialog else {
            return false;
        };
        if team >= 5 || available & (1 << team) == 0 {
            return false;
        }
        let marker = self.city.text_overlay_at(x, y).unwrap_or(0);
        let mask = self.city.mac_state_misc_u32_at(0x1028).unwrap_or(0) as u16;
        if mask & (1 << team) != 0 {
            return false;
        }
        // Both editions store the selected index and its label slot in XMIC.
        if !self
            .city
            .pe_write_sports_team_index_to_microsim_483816(marker, u16::from(team))
        {
            return false;
        }
        self.city
            .set_mac_state_misc_u32_at(0x1028, (mask | (1 << team)) as i16 as u32);
        self.close_city_info_dialog();
        self.finish_mac_building_services(x, y);
        self.pending_traffic_redraw = true;
        self.query_is_inspect = false;
        self.refresh_status_line();
        true
    }

    pub(super) fn finish_reward_placement(&mut self, tool: EditTool) {
        if !matches!(
            tool,
            EditTool::MayorHouse | EditTool::CityHall | EditTool::Statue | EditTool::BraunLlamaDome
        ) {
            return;
        }
        let Some(mask) = reward_mask_for_tool(tool) else {
            return;
        };
        if self.engine_fidelity.uses_win95() {
            self.city
                .set_rewards_available_mask(self.city.rewards_available_mask() & !mask);
        } else {
            // CODE 5 0x47ce ANDs the full latch with the low-byte clear mask.
            self.city
                .set_mac_civic_latch(self.city.mac_civic_latch() & (0xff - mask));
        }
        self.apply_selected_tool(EditTool::Query);
        self.close_tool_menu();
        self.refresh_status_line();
    }

    pub(super) fn refresh_building_network_neighbors(
        &mut self,
        footprint: &[(usize, usize)],
        building: u8,
    ) {
        let kinds: Vec<_> = building_network_access_kinds(building).collect();
        if kinds.is_empty() {
            return;
        }

        let mut refresh_tiles = HashSet::new();
        for &(tx, ty) in footprint {
            for neighbor in neighbor_tiles(tx, ty) {
                refresh_tiles.insert(neighbor);
            }
        }

        for kind in kinds {
            for &(nx, ny) in &refresh_tiles {
                self.refresh_network_tile(nx, ny, kind);
            }
        }
        self.pending_traffic_redraw = true;
    }

    pub(super) fn stamp_building_footprint(
        &mut self,
        origin_x: usize,
        origin_y: usize,
        building: u8,
        footprint: &[(usize, usize)],
    ) {
        let size = usize::from(xbld_tile_size(building));
        let corner_bits = pe_choose_and_place_corner_bits(self.city.rotation());
        let stamps_base_pipe = building_has_internal_pipes(building);
        for &(tx, ty) in footprint {
            // PE PlaceTile (`0x441f00` / `0x40178f`): leave watered/piped/XTRF.
            pe_place_tile_441f00(&mut self.city, tx, ty, building);
            if stamps_base_pipe {
                self.set_tile_base_pipe_bit(tx, ty);
            }
            self.city.set_zone(
                tx,
                ty,
                if size <= 1 {
                    // Player 1×1 tools are not ChooseAndPlace; keep the tile unzoned.
                    0
                } else {
                    building_zone_bits_for_tile(origin_x, origin_y, tx, ty, size, corner_bits)
                },
            );
        }
        if let Some(text_id) = self.provision_microsim_for_building_by_engine(building) {
            // Win95 `L_ItemPlacementCheck` writes the microsim `XTXT` across the footprint.
            for &(tx, ty) in footprint {
                self.city.set_text_overlay(tx, ty, text_id);
            }
        }
        if is_power_plant_building(building) {
            stamp_power_plant_xbit_powerable(&mut self.city, footprint);
        }
        if building == 219 {
            // PE CityToolPlaceSelectedBuilding statue arm @ 0x440835: clear XBIT 0x80.
            for &(tx, ty) in footprint {
                clear_xbit_powerable(&mut self.city, tx, ty);
            }
        }
    }

    pub(super) fn provision_microsim_for_building_by_engine(&mut self, building: u8) -> Option<u8> {
        match self.engine_fidelity {
            EngineFidelity::Mac68k => self
                .city
                .provision_mac_microsim_for_building(building, &mut self.mac_process_rng),
            EngineFidelity::Win95 => self
                .city
                .provision_microsim_for_building_with_crt(building, &mut self.pe_process_crt),
        }
    }
}
