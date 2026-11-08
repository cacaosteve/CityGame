//! Map sprite caches, static tile assembly, and shared sprite drawing helpers.
//!
//! Cache gates, draw order, atlas/frame resolution and depth values retain their
//! existing operations. The traffic workers stay in the app root; this module
//! calls the existing traffic rebuild at the same points.

use super::nature_tools::is_tree_building;
use super::underground::{
    UNDER_TILE_BUILDING_PIPES, UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB,
    UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR, UNDER_TILE_GREY_BOX_1, UNDER_TILE_GREY_BOX_2,
};
use super::view_navigation::{
    ALTITUDE_LAYER_STEP, TILE_HEIGHT_HALF, TILE_WIDTH_HALF, projected_tile_units, render_zoom,
    rotate_tile_coords, viewport_origin,
};
use super::{
    UndergroundRenderTile, UndergroundTileSource, ViewerState, is_highway_tile, is_power_tile,
    is_rail_tile, is_road_tile, is_surface_infrastructure_tile,
    underground_building_pipe_overlay_tile, underground_building_pipe_underlay_tile,
    underground_render_tile, xbld_tile_size,
};
use crate::city::{AppMode, CITY_HEIGHT, CITY_TILES, CITY_WIDTH};
use crate::data_map::{self, CityDataMapMode};
use crate::palette::is_smoke_sprite;
use crate::renderer::{RenderSprite, Renderer};
use crate::tiles::{rotated_building_id, rotated_terrain_id, xbit_sprite_flip_x};
use glam::Vec2;
use std::collections::HashSet;

pub(super) const DEPTH_TERRAIN: i32 = 12;
const DEPTH_WATER: i32 = 14;
pub(super) const DEPTH_ROAD: i32 = 16;
pub(super) const DEPTH_RAIL: i32 = 18;
pub(super) const DEPTH_POWER: i32 = 20;
pub(super) const DEPTH_HIGHWAY: i32 = 22;
pub(super) const DEPTH_ZONE: i32 = 24;
pub(super) const DEPTH_BUILDING: i32 = 26;
const DISASTER_FIRE_SPRITE_BASE: u32 = 1396;
const DISASTER_FIRE_SPRITE_FRAMES: u8 = 4;
pub(super) const DISASTER_FLOOD_SPRITE_ID: u32 = 1492;
pub(super) const DISASTER_RIOTERS_1_SPRITE_ID: u32 = 1493;
pub(super) const DISASTER_RIOTERS_2_SPRITE_ID: u32 = 1494;
pub(super) const DISASTER_TOXIC_CLOUD_SPRITE_ID: u32 = 1496;
/// Above this many dirty tiles, a full sprite rebuild is cheaper than patching buckets.
const INCREMENTAL_DIRTY_MAX: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct SpriteCacheKey {
    camera: Vec2,
    zoom: f32,
    window_size: (u32, u32),
    pixel_scale: f32,
    show_grid: bool,
    show_zone_overlay: bool,
    show_traffic: bool,
    data_map_mode: CityDataMapMode,
    show_buildings: bool,
    show_infrastructure: bool,
    show_zones: bool,
    show_underground: bool,
    mode: AppMode,
    rotation: u8,
    camera_rotation: u8,
    anim_frame: u8,
}

pub(super) struct SpriteCache {
    tile_buckets: Vec<Vec<RenderSprite>>,
    pub(super) traffic: Vec<RenderSprite>,
    flattened: Vec<RenderSprite>,
    pub(super) key: Option<SpriteCacheKey>,
}

impl SpriteCache {
    pub(super) fn new() -> Self {
        Self {
            tile_buckets: vec![Vec::new(); CITY_TILES],
            traffic: Vec::new(),
            flattened: Vec::new(),
            key: None,
        }
    }

    fn invalidate(&mut self) {
        self.key = None;
        self.flattened.clear();
    }
}

pub(super) struct TileSpriteCtx {
    pub(super) origin: Vec2,
    pub(super) zoom: f32,
    pub(super) anim_frame: u8,
    show_grid: bool,
    show_zone_overlay: bool,
    show_buildings: bool,
    pub(super) show_infrastructure: bool,
    show_zones: bool,
    show_underground: bool,
    data_map_mode: CityDataMapMode,
    pub(super) edit_terrain_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TerrainSprites {
    /// Dry-land terrain sprite. Omitted for open water (`XTER` 0x10–0x1d, 0x30–0x45).
    pub(super) terrain: Option<u32>,
    pub(super) water: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum XbldAnchor {
    Tile,
    Building,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct XbldSpriteInfo {
    pub(super) sprite_id: u32,
    pub(super) layer_depth: i32,
    pub(super) depth_adjustment: i32,
    pub(super) anchor: XbldAnchor,
    pub(super) corner_gated: bool,
}

impl ViewerState {
    pub(super) fn invalidate_sprite_cache(&mut self) {
        self.sprite_cache.invalidate();
    }

    pub(super) fn sprite_cache_key(
        &self,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> SpriteCacheKey {
        SpriteCacheKey {
            camera: self.view_camera(),
            zoom: self.zoom,
            window_size,
            pixel_scale,
            show_grid: self.show_grid,
            show_zone_overlay: self.show_zone_overlay,
            show_traffic: self.show_traffic,
            data_map_mode: self.data_map_mode,
            show_buildings: self.show_buildings,
            show_infrastructure: self.show_infrastructure,
            show_zones: self.show_zones,
            show_underground: self.show_underground,
            mode: self.mode,
            rotation: self.city.rotation(),
            camera_rotation: self.camera_rotation,
            anim_frame: self.animation_frame(),
        }
    }

    pub(super) fn build_sprites(
        &mut self,
        renderer: &Renderer<'_>,
        window_size: (u32, u32),
        pixel_scale: f32,
        dirty: HashSet<(usize, usize)>,
        traffic_redraw: bool,
    ) -> Vec<RenderSprite> {
        self.viewport_pixel_scale = pixel_scale;
        let key = self.sprite_cache_key(window_size, pixel_scale);
        let cache_ready = self.sprite_cache.key.as_ref() == Some(&key);
        let anim_frame = self.animation_frame();
        // Traffic redraw is now explicit, so running mode can reuse sprites between real redraw
        // causes instead of rebuilding at display FPS or invalidating all static tile buckets.
        let allow_fast_path =
            cache_ready && dirty.is_empty() && !traffic_redraw && self.mode == AppMode::CityView;
        if allow_fast_path && !self.sprite_cache.flattened.is_empty() {
            return self.sprite_cache.flattened.clone();
        }

        let ctx = TileSpriteCtx {
            origin: viewport_origin(window_size) + self.view_camera(),
            zoom: render_zoom(self.zoom, pixel_scale),
            anim_frame,
            show_grid: self.show_grid,
            show_zone_overlay: self.show_zone_overlay,
            show_buildings: self.show_buildings,
            show_infrastructure: self.show_infrastructure,
            show_zones: self.show_zones,
            show_underground: self.show_underground,
            data_map_mode: self.data_map_mode,
            edit_terrain_only: self.mode == AppMode::EditNewMap,
        };

        let full_rebuild = !cache_ready || dirty.len() > INCREMENTAL_DIRTY_MAX;
        if full_rebuild {
            for bucket in &mut self.sprite_cache.tile_buckets {
                bucket.clear();
            }
            for y in 0..CITY_HEIGHT {
                for x in 0..CITY_WIDTH {
                    self.fill_static_tile_sprites(renderer, &ctx, x, y);
                }
            }
            if !self.show_underground {
                self.rebuild_traffic_sprites(renderer, &ctx);
            } else {
                self.sprite_cache.traffic.clear();
            }
            self.sprite_cache.key = Some(key);
        } else if !dirty.is_empty() {
            for &(x, y) in &dirty {
                self.fill_static_tile_sprites(renderer, &ctx, x, y);
            }
            if self.show_traffic && !self.show_underground {
                self.rebuild_traffic_sprites(renderer, &ctx);
            }
        } else if traffic_redraw
            && self.show_traffic
            && !self.show_underground
            && self.mode == AppMode::CityView
        {
            self.rebuild_traffic_sprites(renderer, &ctx);
        }

        self.sprite_cache.flattened.clear();
        let mut sprites = Vec::with_capacity(CITY_WIDTH * CITY_HEIGHT);
        for bucket in &self.sprite_cache.tile_buckets {
            sprites.extend_from_slice(bucket);
        }
        sprites.extend_from_slice(&self.sprite_cache.traffic);
        sprites.sort_by(|left, right| left.depth.total_cmp(&right.depth));
        self.sprite_cache.flattened = sprites.clone();
        sprites
    }

    fn fill_static_tile_sprites(
        &mut self,
        renderer: &Renderer<'_>,
        ctx: &TileSpriteCtx,
        x: usize,
        y: usize,
    ) {
        let disaster_sprite_id = (!ctx.show_underground)
            .then(|| self.city.text_overlay_at(x, y))
            .flatten()
            .and_then(|value| disaster_sprite_id_for_text_overlay(value, ctx.anim_frame));
        let underground_surface_building = ctx
            .show_underground
            .then(|| self.underground_surface_building_at(x, y));
        let bucket = &mut self.sprite_cache.tile_buckets[tile_sprite_index(x, y)];
        bucket.clear();

        let terrain = rotated_terrain_id(self.city.terrain_at(x, y), self.camera_rotation);
        let altm = self.city.altm_at(x, y);
        let land_elevation = altm.land as f32;
        let water_elevation = altm.water as f32;
        let structure_elevation = self.city.structure_elevation_at(x, y) as f32;
        // Screen position uses the view-rotated tile coordinate; data lookups keep (x, y).
        let (sx, sy) = rotate_tile_coords(x, y, self.camera_rotation);
        let land_anchor = ctx.origin + projected_tile_units(sx, sy, land_elevation) * ctx.zoom;
        let water_anchor = ctx.origin + projected_tile_units(sx, sy, water_elevation) * ctx.zoom;
        let structure_anchor = if structure_elevation == land_elevation {
            land_anchor
        } else {
            ctx.origin + projected_tile_units(sx, sy, structure_elevation) * ctx.zoom
        };

        let grid_tint = if ctx.show_grid && ((x + y) & 1 == 0) {
            [0.88, 0.95, 1.0, 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };
        let data_tint = if ctx.show_underground || ctx.edit_terrain_only {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            data_map::tile_tint(
                ctx.data_map_mode,
                self.city.tile_bits_at(x, y),
                data_map::DataMapSamples {
                    traffic: self.city.traffic_density_at(x, y),
                    land_value: self.city.land_value_at(x, y),
                    pollution: self.city.pollution_density_at(x, y),
                    crime: self.city.crime_density_at(x, y),
                    police: self.city.police_coverage_at(x, y),
                    fire: self.city.fire_coverage_at(x, y),
                    density: self.city.population_density_at(x, y),
                    growth: self.city.rate_of_growth_at(x, y),
                    height: Some(self.city.altitude_at(x, y) & 0x1f),
                },
            )
        };
        let tint = data_map::multiply_rgba(grid_tint, data_tint);

        if ctx.show_underground {
            push_sprite(
                renderer,
                bucket,
                underground_terrain_sprite_id(terrain),
                land_anchor,
                ctx.zoom,
                sprite_depth(sx, sy, DEPTH_TERRAIN, 0),
                [1.0, 1.0, 1.0, 1.0],
                ctx.anim_frame,
            );
            let building = underground_surface_building.expect("underground surface building");
            let xbit_flip = xbit_sprite_flip_x(&self.city, x, y);
            if ctx.show_infrastructure {
                if let Some(base_sprite_id) =
                    underground_surface_base_sprite_id(building, self.city.rotation())
                {
                    push_sprite_with_flip(
                        renderer,
                        bucket,
                        base_sprite_id,
                        structure_anchor,
                        ctx.zoom,
                        sprite_depth(sx, sy, DEPTH_ROAD, -2),
                        [1.0, 1.0, 1.0, 1.0],
                        xbit_flip,
                        ctx.anim_frame,
                    );
                }
                if let Some(info) = underground_surface_sprite_info(building, self.city.rotation())
                    && (!info.corner_gated || self.city.building_should_draw_at(x, y))
                {
                    push_sprite_with_flip(
                        renderer,
                        bucket,
                        info.sprite_id,
                        structure_anchor,
                        ctx.zoom,
                        sprite_depth(sx, sy, info.layer_depth, info.depth_adjustment),
                        [1.0, 1.0, 1.0, 1.0],
                        xbit_flip,
                        ctx.anim_frame,
                    );
                }
            }
            let tunnel_levels = altm.tunnel_levels & 0x1f;
            if ctx.show_infrastructure && tunnel_levels > 1 {
                let tunnel_anchor = land_anchor
                    + Vec2::new(
                        0.0,
                        f32::from(tunnel_levels - 1) * ALTITUDE_LAYER_STEP * ctx.zoom,
                    );
                push_underground_tile_sprite(
                    renderer,
                    bucket,
                    UndergroundRenderTile {
                        tile: UNDER_TILE_GREY_BOX_1,
                        source: UndergroundTileSource::UserLaid,
                        watered: false,
                    },
                    tunnel_anchor,
                    ctx.zoom,
                    sprite_depth(sx, sy, DEPTH_ZONE, 10),
                    ctx.anim_frame,
                );
            }
            if let Some(tile) =
                underground_building_pipe_underlay_tile(&self.city, x, y, self.city.rotation())
            {
                push_underground_tile_sprite(
                    renderer,
                    bucket,
                    tile,
                    structure_anchor,
                    ctx.zoom,
                    sprite_depth(sx, sy, DEPTH_ZONE, 11),
                    ctx.anim_frame,
                );
            }
            if let Some(tile) = underground_render_tile(&self.city, x, y, self.city.rotation()) {
                push_underground_tile_sprite(
                    renderer,
                    bucket,
                    tile,
                    structure_anchor,
                    ctx.zoom,
                    sprite_depth(sx, sy, DEPTH_ZONE, 12),
                    ctx.anim_frame,
                );
            }
            if let Some(tile) =
                underground_building_pipe_overlay_tile(&self.city, x, y, self.city.rotation())
            {
                push_underground_tile_sprite(
                    renderer,
                    bucket,
                    tile,
                    structure_anchor,
                    ctx.zoom,
                    sprite_depth(sx, sy, DEPTH_ZONE, 13),
                    ctx.anim_frame,
                );
            }
            return;
        }

        let terrain_sprites = terrain_sprites(terrain);
        if let Some(sprite_id) = terrain_sprites.terrain {
            push_sprite(
                renderer,
                bucket,
                sprite_id,
                land_anchor,
                ctx.zoom,
                sprite_depth(sx, sy, DEPTH_TERRAIN, 0),
                tint,
                ctx.anim_frame,
            );
        }
        if let Some(sprite_id) = terrain_sprites.water {
            push_sprite(
                renderer,
                bucket,
                sprite_id,
                water_anchor,
                ctx.zoom,
                sprite_depth(sx, sy, DEPTH_WATER, 0),
                if ctx.show_underground {
                    [0.35, 0.38, 0.45, 1.0]
                } else {
                    tint
                },
                ctx.anim_frame,
            );
        }
        let building = self.city.building_at(x, y);
        let building_for_draw = self
            .city
            .win95_pe_raising_bridge_building_for_draw(x, y, building);
        let vacant_zone_tile = building == 0 || is_tree_building(building);
        if !ctx.edit_terrain_only
            && ctx.show_zones
            && (ctx.show_zone_overlay || vacant_zone_tile)
            && let Some(sprite_id) = self
                .city
                .zone_at(x, y)
                .and_then(|zone| zone.zone_sprite_id())
            && renderer.has_sprite(0, sprite_id)
        {
            bucket.push(RenderSprite {
                atlas: 0,
                sprite_id,
                anchor: structure_anchor,
                scale: ctx.zoom,
                depth: sprite_depth(sx, sy, DEPTH_ZONE, 0),
                tint,
                flip_x: false,
                bottom_left_anchor: false,
            });
        }

        if ctx.edit_terrain_only {
            return;
        }

        if let Some((_, record)) = self.city.dispatch_actor_at(x, y)
            && let Some(kind) = record.dispatch_kind()
        {
            push_sprite(
                renderer,
                bucket,
                kind.large_sprite_id(),
                structure_anchor,
                ctx.zoom,
                sprite_depth(sx, sy, DEPTH_BUILDING, 0),
                tint,
                ctx.anim_frame,
            );
            push_disaster_sprite(
                renderer,
                bucket,
                disaster_sprite_id,
                structure_anchor,
                ctx.zoom,
                sprite_depth(sx, sy, DEPTH_BUILDING, 60),
                ctx.anim_frame,
            );
            return;
        }

        if let Some((_, record)) = self.city.ambient_port_actor_at(x, y) {
            let actor_building = self.city.building_at(x, y);
            let actor_xbit = self.city.tile_bits_at(x, y).map(|bits| bits.raw_bits());
            if let Some((sprite_id, flip_x)) =
                record.ambient_port_sprite(actor_building, actor_xbit)
            {
                let train_draw_offset = record
                    .ambient_train_draw_offset(actor_building, actor_xbit)
                    .map(|(offset_x, offset_y)| Vec2::new(f32::from(offset_x), f32::from(offset_y)))
                    .unwrap_or(Vec2::ZERO)
                    * ctx.zoom;
                let actor_anchor = (if record.ambient_port_uses_water_elevation()
                    || record.ambient_train_uses_water_elevation(actor_building)
                {
                    water_anchor
                } else {
                    land_anchor
                }) + if record.is_ambient_train_family() {
                    // Train iPX/iPY are next rail tiles. PE sends these records through
                    // DrawTrain rather than treating those bytes as generic local offsets.
                    train_draw_offset
                } else {
                    ambient_actor_screen_offset(&record, ctx.zoom, self.camera_rotation as usize)
                };
                push_sprite_with_flip(
                    renderer,
                    bucket,
                    sprite_id,
                    actor_anchor,
                    ctx.zoom,
                    sprite_depth(sx, sy, DEPTH_BUILDING, 20),
                    tint,
                    flip_x,
                    ctx.anim_frame,
                );
            }
        }

        let infrastructure = is_surface_infrastructure_tile(building);
        if (infrastructure && !ctx.show_infrastructure)
            || (!infrastructure && !is_tree_building(building) && !ctx.show_buildings)
        {
            push_disaster_sprite(
                renderer,
                bucket,
                disaster_sprite_id,
                structure_anchor,
                ctx.zoom,
                sprite_depth(sx, sy, DEPTH_BUILDING, 60),
                ctx.anim_frame,
            );
            return;
        }

        let xbit_flip = xbit_sprite_flip_x(&self.city, x, y);
        if let Some(base_sprite_id) =
            xbld_base_layer_sprite_id(building_for_draw, self.city.rotation())
        {
            push_sprite_with_flip(
                renderer,
                bucket,
                base_sprite_id,
                structure_anchor,
                ctx.zoom,
                sprite_depth(sx, sy, DEPTH_ROAD, -2),
                tint,
                xbit_flip,
                ctx.anim_frame,
            );
        }
        if let Some(info) = xbld_sprite_info(building_for_draw, self.city.rotation())
            && (!info.corner_gated || self.city.building_should_draw_at(x, y))
        {
            let depth = sprite_depth(sx, sy, info.layer_depth, info.depth_adjustment);
            push_sprite_with_flip(
                renderer,
                bucket,
                info.sprite_id,
                structure_anchor,
                ctx.zoom,
                depth,
                tint,
                xbit_flip,
                ctx.anim_frame,
            );
            push_smoke_overlay_with_flip(
                renderer,
                bucket,
                info.sprite_id,
                structure_anchor,
                ctx.zoom,
                depth + 0.1,
                tint,
                xbit_flip,
                ctx.anim_frame,
            );
        }
        push_disaster_sprite(
            renderer,
            bucket,
            disaster_sprite_id,
            structure_anchor,
            ctx.zoom,
            sprite_depth(sx, sy, DEPTH_BUILDING, 60),
            ctx.anim_frame,
        );
    }
}

fn ambient_actor_screen_offset(
    record: &crate::win95_overlay::Win95OverlayRecord,
    zoom: f32,
    camera_rotation: usize,
) -> Vec2 {
    let (zoom_level, x_divisor, y_divisor) =
        crate::win95_overlay::xthg_zoom_projection_for_scale(zoom);
    let px = f32::from(record.sub_x) - 8.0;
    let py = f32::from(record.sub_y) - 8.0;
    let (px, py) = match camera_rotation & 3 {
        0 => (px, py),
        1 => (-py, px),
        2 => (-px, -py),
        _ => (py, -px),
    };
    let pe_x = (px - py) / x_divisor;
    let pe_y = (px + py) / y_divisor;
    let pe_tile_x = (4_u32 << zoom_level) as f32;
    let pe_tile_y = (2_u32 << zoom_level) as f32;
    let rust_tile_x = TILE_WIDTH_HALF * zoom;
    let rust_tile_y = TILE_HEIGHT_HALF * 0.5 * zoom;
    let height = (f32::from(record.height) * (2_u32 << zoom_level) as f32)
        * (ALTITUDE_LAYER_STEP * zoom / (3_u32 << zoom_level) as f32);

    Vec2::new(
        pe_x * rust_tile_x / pe_tile_x,
        pe_y * rust_tile_y / pe_tile_y - height,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_sprite(
    renderer: &Renderer<'_>,
    sprites: &mut Vec<RenderSprite>,
    sprite_id: u32,
    anchor: Vec2,
    scale: f32,
    depth: f32,
    tint: [f32; 4],
    anim_frame: u8,
) {
    push_sprite_with_flip(
        renderer, sprites, sprite_id, anchor, scale, depth, tint, false, anim_frame,
    );
}

fn push_disaster_sprite(
    renderer: &Renderer<'_>,
    sprites: &mut Vec<RenderSprite>,
    sprite_id: Option<u32>,
    anchor: Vec2,
    scale: f32,
    depth: f32,
    anim_frame: u8,
) {
    let Some(sprite_id) = sprite_id else {
        return;
    };
    push_sprite(
        renderer,
        sprites,
        sprite_id,
        anchor,
        scale,
        depth,
        [1.0, 1.0, 1.0, 1.0],
        anim_frame,
    );
}

pub(super) fn disaster_sprite_id_for_text_overlay(text_overlay: u8, anim_frame: u8) -> Option<u32> {
    // XTXT 0xfb..0xff are saved disaster markers, not building sprite ids.
    // SC2K-docs/OpenCity2k map them to toxic cloud, flood, rioters, and fire.
    match text_overlay {
        0xfb => Some(DISASTER_TOXIC_CLOUD_SPRITE_ID),
        0xfc => Some(DISASTER_FLOOD_SPRITE_ID),
        0xfd => Some(DISASTER_RIOTERS_1_SPRITE_ID),
        0xfe => Some(DISASTER_RIOTERS_2_SPRITE_ID),
        0xff => {
            let frame = anim_frame % DISASTER_FIRE_SPRITE_FRAMES;
            Some(DISASTER_FIRE_SPRITE_BASE + u32::from(frame))
        }
        _ => None,
    }
}

pub(super) fn push_underground_tile_sprite(
    renderer: &Renderer<'_>,
    sprites: &mut Vec<RenderSprite>,
    tile: UndergroundRenderTile,
    anchor: Vec2,
    scale: f32,
    depth: f32,
    anim_frame: u8,
) {
    if let Some(sprite_id) = underground_tile_sprite_id(tile)
        && renderer.atlas_for_sprite(sprite_id).is_some()
    {
        push_sprite(
            renderer,
            sprites,
            sprite_id,
            anchor,
            scale,
            depth,
            [1.0, 1.0, 1.0, 1.0],
            anim_frame,
        );
        return;
    }

    let Some(tint) = underground_tile_tint(tile.tile) else {
        return;
    };
    push_sprite(
        renderer, sprites, 1256, anchor, scale, depth, tint, anim_frame,
    );
}

pub(super) fn underground_tile_sprite_id(tile: UndergroundRenderTile) -> Option<u32> {
    match tile.tile {
        1..=15 => Some(1318 + tile.tile as u32),
        16..=30 => Some(if tile.watered {
            1450 + (tile.tile - 16) as u32
        } else {
            1334 + (tile.tile - 16) as u32
        }),
        UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR => Some(if tile.watered { 1465 } else { 1349 }),
        UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB => Some(if tile.watered { 1466 } else { 1350 }),
        UNDER_TILE_BUILDING_PIPES => Some(if tile.watered { 1467 } else { 1351 }),
        UNDER_TILE_GREY_BOX_1 => Some(1352),
        UNDER_TILE_GREY_BOX_2 => Some(1353),
        _ => None,
    }
}

pub(super) fn underground_tile_tint(tile: u8) -> Option<[f32; 4]> {
    match tile {
        1..=15 => Some([0.20, 0.20, 0.20, 0.95]),
        16..=30 => Some([0.15, 0.15, 0.15, 0.95]),
        UNDER_TILE_CROSSOVER_PIPES_TB_SUBWAY_LR | UNDER_TILE_CROSSOVER_PIPES_LR_SUBWAY_TB => {
            Some([0.18, 0.18, 0.18, 0.95])
        }
        UNDER_TILE_BUILDING_PIPES => Some([0.18, 0.18, 0.18, 0.95]),
        UNDER_TILE_GREY_BOX_1 => Some([0.18, 0.18, 0.18, 0.95]),
        UNDER_TILE_GREY_BOX_2 => Some([0.28, 0.28, 0.28, 0.95]),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_sprite_with_flip(
    renderer: &Renderer<'_>,
    sprites: &mut Vec<RenderSprite>,
    sprite_id: u32,
    anchor: Vec2,
    scale: f32,
    depth: f32,
    tint: [f32; 4],
    flip_x: bool,
    anim_frame: u8,
) {
    let Some(atlas) = renderer.atlas_for_sprite(sprite_id) else {
        return;
    };
    let (atlas, resolved) = renderer.resolve_sprite_frame(atlas, sprite_id, anim_frame);
    push_sprite_to_atlas(
        renderer, sprites, atlas, resolved, anchor, scale, depth, tint, flip_x,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_smoke_overlay_with_flip(
    renderer: &Renderer<'_>,
    sprites: &mut Vec<RenderSprite>,
    sprite_id: u32,
    anchor: Vec2,
    scale: f32,
    depth: f32,
    tint: [f32; 4],
    flip_x: bool,
    anim_frame: u8,
) {
    if !is_smoke_sprite(sprite_id) {
        return;
    }
    let Some((atlas, resolved)) = renderer.resolve_smoke_overlay_frame(sprite_id, anim_frame)
    else {
        return;
    };
    push_sprite_to_atlas(
        renderer, sprites, atlas, resolved, anchor, scale, depth, tint, flip_x,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_sprite_to_atlas(
    renderer: &Renderer<'_>,
    sprites: &mut Vec<RenderSprite>,
    atlas: usize,
    sprite_id: u32,
    anchor: Vec2,
    scale: f32,
    depth: f32,
    tint: [f32; 4],
    flip_x: bool,
) {
    push_sprite_to_atlas_with_anchor(
        renderer, sprites, atlas, sprite_id, anchor, scale, depth, tint, flip_x, false,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_sprite_to_atlas_with_anchor(
    renderer: &Renderer<'_>,
    sprites: &mut Vec<RenderSprite>,
    atlas: usize,
    sprite_id: u32,
    anchor: Vec2,
    scale: f32,
    depth: f32,
    tint: [f32; 4],
    flip_x: bool,
    bottom_left_anchor: bool,
) {
    if renderer.has_sprite(atlas, sprite_id) {
        sprites.push(RenderSprite {
            atlas,
            sprite_id,
            anchor,
            scale,
            depth,
            tint,
            flip_x,
            bottom_left_anchor,
        });
    }
}

pub(super) fn tile_sprite_index(x: usize, y: usize) -> usize {
    x + y * CITY_WIDTH
}

pub(super) fn sprite_depth(x: usize, y: usize, layer_depth: i32, depth_adjustment: i32) -> f32 {
    sprite_depth_f32(x as f32, y as f32, layer_depth, depth_adjustment)
}

fn sprite_depth_f32(x: f32, y: f32, layer_depth: i32, depth_adjustment: i32) -> f32 {
    (x + y) * 64.0 + layer_depth as f32 + depth_adjustment as f32
}

pub(super) fn terrain_sprites(terrain: u8) -> TerrainSprites {
    match terrain {
        0x00..=0x0d => TerrainSprites {
            terrain: Some(1256 + terrain as u32),
            water: None,
        },
        // Underwater: water overlay only (sea floor is below the surface).
        0x10..=0x1d => TerrainSprites {
            terrain: None,
            water: Some(1270),
        },
        // Shoreline: partial land slope plus animated water edge.
        0x20..=0x2d => TerrainSprites {
            terrain: Some(1256 + (terrain - 0x20) as u32),
            water: Some(1270 + (terrain - 0x20) as u32),
        },
        // Surface water and bay pieces: water overlay only (no grass diamond under).
        0x30..=0x3d => TerrainSprites {
            terrain: None,
            water: Some(1270 + (terrain - 0x30) as u32),
        },
        0x3e => TerrainSprites {
            terrain: Some(1269),
            water: Some(1284),
        },
        0x40..=0x45 => TerrainSprites {
            terrain: None,
            water: Some(1285 + (terrain - 0x40) as u32),
        },
        _ => TerrainSprites {
            terrain: Some(1256),
            water: None,
        },
    }
}

pub(super) fn underground_terrain_sprite_id(terrain: u8) -> u32 {
    match terrain {
        0x00..=0x0d => 1305 + terrain as u32,
        0x20..=0x2d => 1305 + (terrain - 0x20) as u32,
        _ => 1305,
    }
}

pub(super) fn building_sprite_id(building: u8) -> Option<u32> {
    match building {
        0 => None,
        id => Some(1000 + id as u32),
    }
}

pub(super) fn xbld_sprite_info(building: u8, rotation: u8) -> Option<XbldSpriteInfo> {
    let building = rotated_building_id(building, rotation);
    let sprite_id = building_sprite_id(building)?;
    let tile_size = xbld_tile_size(building);
    let (layer_depth, anchor, corner_gated) = if is_road_tile(building) {
        (DEPTH_ROAD, XbldAnchor::Tile, false)
    } else if is_rail_tile(building) {
        (DEPTH_RAIL, XbldAnchor::Tile, false)
    } else if is_power_tile(building) {
        (DEPTH_POWER, XbldAnchor::Tile, false)
    } else if is_highway_tile(building) {
        (DEPTH_HIGHWAY, XbldAnchor::Tile, tile_size > 1)
    } else {
        (DEPTH_BUILDING, XbldAnchor::Building, tile_size > 1)
    };

    Some(XbldSpriteInfo {
        sprite_id,
        layer_depth,
        depth_adjustment: xbld_depth_adjustment(building),
        anchor,
        corner_gated,
    })
}

pub(super) fn xbld_base_layer_sprite_id(building: u8, rotation: u8) -> Option<u32> {
    let building = rotated_building_id(building, rotation);
    match building {
        67 => Some(1029),
        68 => Some(1030),
        _ => None,
    }
}

pub(super) fn underground_surface_sprite_info(
    building: u8,
    rotation: u8,
) -> Option<XbldSpriteInfo> {
    let info = xbld_sprite_info(building, rotation)?;
    matches!(info.layer_depth, DEPTH_ROAD | DEPTH_POWER | DEPTH_HIGHWAY).then_some(info)
}

fn underground_surface_base_sprite_id(building: u8, rotation: u8) -> Option<u32> {
    let rotated = rotated_building_id(building, rotation);
    (is_road_tile(rotated) || is_highway_tile(rotated))
        .then(|| xbld_base_layer_sprite_id(building, rotation))
        .flatten()
}

fn xbld_depth_adjustment(building: u8) -> i32 {
    match building {
        63..=66 => -1,
        _ => 0,
    }
}
