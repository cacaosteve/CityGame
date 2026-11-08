//! Highway onramp orientation, road compatibility, and placement accounting.
//!
//! These helpers retain the PE and Mac input mappings and the native highway
//! terminal writes; the stroke planner only consumes the decoded onramp sides.

use super::super::{
    MAC_HIGHWAY_ROAD_COMPATIBILITY, NETWORK_BOTTOM, NETWORK_LEFT, NETWORK_RIGHT, NETWORK_TOP,
    PE_HIGHWAY_ROAD_COMPATIBILITY, PE_MAPTOOL_ONRAMP_COST_TABLE_4DC260, PE_TOOL_COST_TABLE_4DC260,
    PLACE_HIGHWAY_SLOPE_WALK_DX, PLACE_HIGHWAY_SLOPE_WALK_DY, network_side_axis,
};
use super::pe_place_tile_441f00;
use crate::city::City;

/// PE MapTool Onramp funds debit @ `0x44197b`–`0x4419a3`: cost = `[tool_id*4+0x4dc260]`;
/// `cmp funds, cost` / `jae` continue else refuse; else `sub funds, cost` before PlaceTile.
pub(in crate::app) fn pe_maptool_onramp_funds_debit_44197b(
    city: &mut City,
    tool_id: usize,
) -> bool {
    let Some(&cost) = PE_MAPTOOL_ONRAMP_COST_TABLE_4DC260.get(tool_id) else {
        return false;
    };
    if cost < 0 {
        return false;
    }
    let funds = city.city_funds();
    if funds < cost {
        return false;
    }
    city.set_city_funds(funds - cost);
    true
}

/// PE PlaceHighway AtCoordinates funds @ `0x44c110`–`0x44c295`:
/// `cost = 0x4dc260[tool_id] * (walk_steps + 1)`; refuse if funds `< cost`, else debit.
/// Highway MapTool id is `1` (`0x43f4c6` jump → `0x401a2d`).
pub(in crate::app) fn pe_place_highway_atcoordinates_funds_debit_44c267(
    city: &mut City,
    tool_id: usize,
    walk_steps: u16,
) -> bool {
    let Some(&unit) = PE_TOOL_COST_TABLE_4DC260.get(tool_id) else {
        return false;
    };
    if unit < 0 {
        return false;
    }
    let cost = unit.saturating_mul(i32::from(walk_steps) + 1);
    let funds = city.city_funds();
    if funds < cost {
        return false;
    }
    city.set_city_funds(funds - cost);
    true
}

/// PE PlaceHighway AtCoordinates post-debit origin stamp @ `0x44c2a6`–`0x44c2e8`:
/// PlaceTile `XTER+0x3e`, then `ALTM &= 0x3ff` / `|= bit10` (tunnel_levels := 1).
/// Skips Onramp UI `0x4017b2`, neighbor `0x401c76`, and `0x4012c1`.
pub(in crate::app) fn pe_place_highway_atcoordinates_origin_place_44c2a6(
    city: &mut City,
    x: usize,
    y: usize,
    start_xter: u8,
) {
    pe_place_tile_441f00(city, x, y, start_xter.wrapping_add(0x3e));
    if x < 0x80 && y < 0x80 {
        city.clear_altm_tunnel_bits(x, y);
        city.set_tunnel_levels(x, y, 1);
    }
}

/// PE PlaceHighway climb ALTM + terminal PlaceTile @ `0x44c347`–`0x44c413`.
/// For each walk step: advance dir tables, write `ALTM` land/water `| ((land-base+1)<<10)`.
/// Then PlaceTile terminal `XBLD = ((start_xter+1)&3)+0x3f` and `ALTM &= 0x3ff` / `|= bit10`,
/// which replaces the climb formula on that tile. Skips `0x4012c1`.
pub(in crate::app) fn pe_place_highway_atcoordinates_climb_and_terminal_44c347(
    city: &mut City,
    origin_x: usize,
    origin_y: usize,
    start_xter: u8,
    steps: u16,
    base_land: u8,
) {
    let dir = ((u16::from(start_xter) + 2) & 3) as usize;
    let mut wx = origin_x as isize;
    let mut wy = origin_y as isize;
    let mut remaining = steps;
    while remaining > 0 {
        wx += PLACE_HIGHWAY_SLOPE_WALK_DX[dir];
        wy += PLACE_HIGHWAY_SLOPE_WALK_DY[dir];
        if (0..0x80).contains(&wx) && (0..0x80).contains(&wy) {
            let cx = wx as usize;
            let cy = wy as usize;
            let land = city.altitude_at(cx, cy) & 0x1f;
            let levels = (i16::from(land) - i16::from(base_land) + 1).clamp(0, 0x1f) as u8;
            city.clear_altm_tunnel_bits(cx, cy);
            city.set_tunnel_levels(cx, cy, levels);
        }
        remaining -= 1;
    }
    if (0..0x80).contains(&wx) && (0..0x80).contains(&wy) {
        let terminal_xbld = ((u16::from(start_xter) + 1) & 3) as u8 + 0x3f;
        let tx = wx as usize;
        let ty = wy as usize;
        pe_place_tile_441f00(city, tx, ty, terminal_xbld);
        // PE `0x44c3f8`–`0x44c413`: clear bits 10–15, then set bit 10. A terminal
        // below the origin base would otherwise keep the climb formula (0).
        city.clear_altm_tunnel_bits(tx, ty);
        city.set_tunnel_levels(tx, ty, 1);
    }
}

pub(in crate::app) fn onramp_building_for_sides(highway_side: u8, road_side: u8) -> Option<u8> {
    if !matches!(
        highway_side,
        NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT
    ) || !matches!(
        road_side,
        NETWORK_TOP | NETWORK_RIGHT | NETWORK_BOTTOM | NETWORK_LEFT
    ) || network_side_axis(highway_side) == network_side_axis(road_side)
    {
        return None;
    }

    let highway_positive = matches!(highway_side, NETWORK_RIGHT | NETWORK_BOTTOM);
    let road_positive = matches!(road_side, NETWORK_RIGHT | NETWORK_BOTTOM);
    match (highway_positive, road_positive) {
        (false, true) => Some(93),
        (false, false) => Some(94),
        (true, false) => Some(95),
        (true, true) => Some(96),
    }
}

pub(in crate::app) fn pe_onramp_compatible_road_mask(highway_mask: u8, road_mask: u8) -> u8 {
    road_mask & PE_HIGHWAY_ROAD_COMPATIBILITY[usize::from(highway_mask & 0x0f)]
}

pub(in crate::app) fn mac_onramp_compatible_road_mask(highway_mask: u8, road_mask: u8) -> u8 {
    road_mask & MAC_HIGHWAY_ROAD_COMPATIBILITY[usize::from(highway_mask & 0x0f)]
}

pub(in crate::app) fn onramp_sides(building: u8, rotate: bool) -> Option<(u8, u8)> {
    let (negative_highway, negative_road) = match building {
        93 => (true, false),
        94 => (true, true),
        95 => (false, true),
        96 => (false, false),
        _ => return None,
    };
    Some(if rotate {
        (
            if negative_highway {
                NETWORK_LEFT
            } else {
                NETWORK_RIGHT
            },
            if negative_road {
                NETWORK_TOP
            } else {
                NETWORK_BOTTOM
            },
        )
    } else {
        (
            if negative_highway {
                NETWORK_TOP
            } else {
                NETWORK_BOTTOM
            },
            if negative_road {
                NETWORK_LEFT
            } else {
                NETWORK_RIGHT
            },
        )
    })
}
