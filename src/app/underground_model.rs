//! Shared underground tile selection, source classification, and view rotation.
//!
//! Rendering, previews, inspection and pipe-mask checks use this read-only model.
//! Water allocation and pipe state mutation remain with the service/tool owners.

use super::power_water::{
    base_building_pipe_present, saved_base_building_pipe_watered, underground_building_pipe_tile,
    underground_user_pipe_tile, water_source_connects_as_building_pipe,
};
use super::underground::{
    UNDER_TILE_BUILDING_PIPES, UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB,
    UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR, UNDER_TILE_GREY_BOX_1, UNDER_TILE_GREY_BOX_2,
    UndergroundKind, underground_crossover_axis,
};
use crate::city::City;

pub(super) fn underground_tile_has_kind(tile: u8, kind: UndergroundKind) -> bool {
    kind.contains(tile) || underground_crossover_axis(tile, kind).is_some()
}

pub(super) fn underground_tile_counts_as_subway(tile: u8) -> bool {
    // PE DoBudgetSubwayCheck @ `0x43682c`–`0x43684e`: admit subway `1..=0x0f`,
    // crossovers `0x1f`/`0x20`, grey-box/silo `0x22`, entrance `0x23`.
    underground_tile_has_kind(tile, UndergroundKind::Subway)
        || matches!(tile, UNDER_TILE_GREY_BOX_1 | UNDER_TILE_GREY_BOX_2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UndergroundTileSource {
    UserLaid,
    BuildingInternal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct UndergroundRenderTile {
    pub(super) tile: u8,
    pub(super) source: UndergroundTileSource,
    pub(super) watered: bool,
}

pub(super) fn underground_render_tile(
    city: &City,
    x: usize,
    y: usize,
    rotation: u8,
) -> Option<UndergroundRenderTile> {
    let watered = saved_base_building_pipe_watered(city, x, y);
    let underground_tile = city.underground_at(x, y);
    if underground_tile != 0 {
        return Some(UndergroundRenderTile {
            tile: rotated_underground_tile(underground_tile, rotation),
            source: underground_tile_source(city, x, y, underground_tile),
            watered,
        });
    }

    if base_building_pipe_present(city, x, y) {
        return Some(UndergroundRenderTile {
            tile: UNDER_TILE_BUILDING_PIPES,
            source: UndergroundTileSource::BuildingInternal,
            watered,
        });
    }

    None
}

pub(super) fn underground_building_pipe_underlay_tile(
    city: &City,
    x: usize,
    y: usize,
    _rotation: u8,
) -> Option<UndergroundRenderTile> {
    let underground_tile = city.underground_at(x, y);
    if !underground_user_pipe_tile(underground_tile) {
        return None;
    }
    if !base_building_pipe_present(city, x, y) {
        return None;
    }

    Some(UndergroundRenderTile {
        tile: UNDER_TILE_BUILDING_PIPES,
        source: UndergroundTileSource::BuildingInternal,
        watered: saved_base_building_pipe_watered(city, x, y),
    })
}

pub(super) fn underground_building_pipe_overlay_tile(
    city: &City,
    x: usize,
    y: usize,
    _rotation: u8,
) -> Option<UndergroundRenderTile> {
    let underground_tile = city.underground_at(x, y);
    if underground_tile == 0
        || underground_building_pipe_tile(underground_tile)
        || underground_user_pipe_tile(underground_tile)
    {
        return None;
    }

    if !base_building_pipe_present(city, x, y) {
        return None;
    }

    Some(UndergroundRenderTile {
        tile: UNDER_TILE_BUILDING_PIPES,
        source: UndergroundTileSource::BuildingInternal,
        watered: saved_base_building_pipe_watered(city, x, y),
    })
}

pub(super) fn rotated_underground_tile(tile: u8, rotation: u8) -> u8 {
    let rotation = rotation & 3;
    match tile {
        1..=15 => rotated_underground_family_tile(1, tile - 1, rotation),
        16..=30 => rotated_underground_family_tile(16, tile - 16, rotation),
        UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR if rotation & 1 == 1 => {
            UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB
        }
        UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB if rotation & 1 == 1 => {
            UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR
        }
        _ => tile,
    }
}

fn rotated_underground_family_tile(base: u8, offset: u8, rotation: u8) -> u8 {
    let rotated_offset = match offset {
        0 | 1 if rotation & 1 == 1 => offset ^ 1,
        0 | 1 => offset,
        2..=5 => 2 + ((offset - 2 + rotation) & 3),
        6..=9 => 6 + ((offset - 6 + rotation) & 3),
        10..=13 => 10 + ((offset - 10 + rotation) & 3),
        14 => 14,
        _ => offset,
    };
    base + rotated_offset
}

pub(super) fn underground_tile_source(
    city: &City,
    x: usize,
    y: usize,
    tile: u8,
) -> UndergroundTileSource {
    if tile == UNDER_TILE_BUILDING_PIPES
        || (UndergroundKind::Pipes.contains(tile)
            && water_source_connects_as_building_pipe(city, x, y))
    {
        UndergroundTileSource::BuildingInternal
    } else {
        UndergroundTileSource::UserLaid
    }
}
