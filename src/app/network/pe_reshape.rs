//! Win95 reshape shape selection shared by placement and MapTool.

use super::super::{
    offset_city_tile,
    underground::{PE_PLACE_PIPES_XTXT_TILE, PE_XUND_EW_GATE_4E7C70, PE_XUND_NS_GATE_4E7CB8},
};
use super::strokes::{
    pe_network_xtxt_fa_edge_mask_bits_44b0ac, pe_place_power_mask_tile_44ac2c,
    pe_place_rail_mask_tile_44b6a0, pe_place_road_mask_tile_44b0e5,
};
use super::{NetworkKind, PE_NETWORK_SURFACE_REMAP_GATE_4E7B68, directional_neighbor_tiles};
use crate::city::{City, TileBits};

pub(in crate::app) fn pe_surface_neighbor_shape(
    city: &City,
    x: usize,
    y: usize,
    kind: NetworkKind,
) -> u8 {
    if let Some(offset) = pe_network_slope_offset(city.terrain_at(x, y)) {
        let base = match kind {
            NetworkKind::Power => 0x0e,
            NetworkKind::Road => 0x1d,
            NetworkKind::Rail => 0x2c,
        };
        return offset.wrapping_add(base);
    }
    if kind == NetworkKind::Rail {
        // 0x44b467 checks x−1, x+1, y−1, y+1 before the ordinary rail mask.
        for ((nx, ny), (slope, cap)) in [(-1, 0), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .zip([(0x2e, 0x3b), (0x30, 0x3d), (0x2f, 0x3c), (0x31, 0x3e)])
            .filter_map(|((dx, dy), shape)| Some((offset_city_tile(x, y, dx, dy)?, shape)))
        {
            if city.building_at(nx, ny) == slope {
                return cap;
            }
        }
    }
    let mask = pe_network_terrain_neighbor_mask(city, x, y, |nx, ny| {
        let xbld = city.building_at(nx, ny);
        match kind {
            NetworkKind::Power => city.tile_bits_at(nx, ny).is_some_and(TileBits::powerable),
            NetworkKind::Road => {
                matches!(xbld, 0x1d..=0x2b | 0x3f..=0x46 | 0x4b..=0x4c | 0x5d..=0x60)
            }
            NetworkKind::Rail => {
                matches!(xbld, 0x2c..=0x3e | 0x45..=0x48 | 0x4d..=0x4e | 0x6c..=0x6f)
            }
        }
    });
    let edge =
        pe_network_xtxt_fa_edge_mask_bits_44b0ac(x, y, city.text_overlay_at(x, y).unwrap_or(0));
    match kind {
        NetworkKind::Power => pe_place_power_mask_tile_44ac2c(mask),
        NetworkKind::Road => pe_place_road_mask_tile_44b0e5(mask | edge),
        NetworkKind::Rail => pe_place_rail_mask_tile_44b6a0(mask | edge),
    }
}

pub(in crate::app) fn pe_network_slope_offset(xter: u8) -> Option<u8> {
    let xter = usize::from(xter);
    if xter >= 0x30 || PE_NETWORK_SURFACE_REMAP_GATE_4E7B68[xter & 0x0f] == 0 {
        return None;
    }
    // Surface workers test the low nibble but index 0x4e7c60 with the
    // whole byte. The next 32 words are the first 64 bytes of 0x4e7c70.
    Some(match xter.checked_sub(PE_PLACE_PIPES_XTXT_TILE.len()) {
        Some(index) => PE_XUND_EW_GATE_4E7C70[2 * index],
        None => PE_PLACE_PIPES_XTXT_TILE[xter] as u8,
    })
}

pub(in crate::app) fn pe_network_terrain_neighbor_mask(
    city: &City,
    x: usize,
    y: usize,
    admits: impl Fn(usize, usize) -> bool,
) -> u8 {
    directional_neighbor_tiles(x, y)
        .filter(|&(_, nx, ny)| admits(nx, ny))
        .filter(|&(_, nx, ny)| {
            // PE first-coordinate probes use 0x4e7cb8, second-coordinate probes 0x4e7c70.
            let gate = if nx != x {
                &PE_XUND_NS_GATE_4E7CB8
            } else {
                &PE_XUND_EW_GATE_4E7C70
            };
            gate.get(usize::from(city.terrain_at(nx, ny)))
                .is_some_and(|&entry| entry != 0)
        })
        .fold(0, |mask, (side, _, _)| mask | side)
}
