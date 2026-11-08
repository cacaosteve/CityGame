//! Win95/XTRF traffic overlay sprite assembly and atlas diagnostics.

use super::*;

impl ViewerState {
    pub(super) fn consume_traffic_redraw_request(
        &mut self,
        dirty: &HashSet<(usize, usize)>,
    ) -> bool {
        if !self.show_traffic || self.mode != AppMode::CityView {
            return false;
        }

        if dirty_tiles_repaint_traffic(&self.city, self.engine_fidelity, dirty) {
            self.traffic_redraw_epoch = self.traffic_redraw_epoch.wrapping_add(1);
            return true;
        }

        false
    }

    pub(super) fn consume_pending_traffic_redraw(&mut self) -> bool {
        let pending = self.pending_traffic_redraw;
        self.pending_traffic_redraw = false;
        pending
    }

    pub(super) fn advance_static_traffic_redraw_phase(&mut self, delta_seconds: f32) -> bool {
        let _ = delta_seconds;
        // Do not spin the static Mac traffic family on the game-speed clock. The correct
        // moving-road effect appears to be table/frame based and still needs a disassembly pass;
        // speed-clock re-randomization only produces the visible blue/white line "spinning" bug.
        false
    }

    pub(super) fn reset_traffic_redraw_phase(&mut self) {
        self.traffic_redraw_epoch = 0;
        self.traffic_redraw_accumulator = 0.0;
        self.pending_traffic_redraw = false;
    }

    /// Saved `XTRF` bytes are stable decorative traffic; live sim only when no map is loaded.
    fn traffic_congestion_byte(&self, x: usize, y: usize) -> u8 {
        if self.city.has_traffic_density_map() {
            self.city.traffic_density_at(x, y).unwrap_or(0)
        } else {
            self.traffic.congestion_at(x, y)
        }
    }

    pub(super) fn log_traffic_stats(&self, renderer: &Renderer<'_>) {
        let mut resolved = 0usize;
        let scale_band = TRAFFIC_ROAD_OVERLAY_SPRITE_OFFSET;
        let mut traffic_rng = self.traffic.draw_rng(
            &self.city.name,
            self.city.rotation(),
            self.traffic_redraw_epoch,
        );
        let congestion = |x: usize, y: usize| self.traffic_congestion_byte(x, y);
        let mut sample: Option<(u32, u32, u32, u8)> = None;

        let (total, source) = {
            for y in 0..CITY_HEIGHT {
                for x in 0..CITY_WIDTH {
                    let Some(overlay) =
                        win95_traffic_overlay_at(&self.city, x, y, congestion, &mut traffic_rng)
                    else {
                        continue;
                    };
                    let render_id =
                        traffic_render_sprite_id(renderer, overlay.sprite_id, scale_band);
                    let Some((atlas, resolved_id)) = resolve_traffic_sprite(
                        renderer,
                        overlay.sprite_id,
                        scale_band,
                        overlay.blit_flip,
                        self.animation_frame(),
                    ) else {
                        continue;
                    };
                    resolved += 1;
                    if sample.is_none() {
                        let size = renderer.sprite_size(atlas, resolved_id);
                        let bands = renderer.animation_frame_count(atlas, render_id);
                        sample = size.map(|(w, h)| (render_id, w, h, bands));
                    }
                }
            }
            let total = {
                let mut count_rng = self.traffic.draw_rng(
                    &self.city.name,
                    self.city.rotation(),
                    self.traffic_redraw_epoch,
                );
                count_traffic_overlays(&self.city, congestion, &mut count_rng)
            };
            let source = if self.city.has_traffic_density_map() {
                "xtrf"
            } else {
                "heuristic"
            };
            (total, source)
        };

        if let Some((base_id, w, h, bands)) = sample {
            log::info!(
                "traffic {} ({source}): {} overlay tiles, {} in atlas (sample id={base_id} {w}x{h}, anim_bands={bands})",
                if self.show_traffic { "on" } else { "off" },
                total,
                resolved,
            );
        } else {
            log::info!(
                "traffic {} ({source}): {} overlay tiles, {} in atlas",
                if self.show_traffic { "on" } else { "off" },
                total,
                resolved
            );
        }
    }

    pub(super) fn rebuild_traffic_sprites(&mut self, renderer: &Renderer<'_>, ctx: &TileSpriteCtx) {
        let mut traffic = Vec::new();
        if !self.show_traffic || !ctx.show_infrastructure || ctx.edit_terrain_only {
            self.sprite_cache.traffic.clear();
            return;
        }

        // CityGame has one supported traffic presentation: the Win95/XTRF road-overlay
        // contract. Keep it stable across save formats and graphics preferences; the selected
        // engine may still control simulation elsewhere, but must not switch traffic to the
        // incomplete Mac export_80 renderer.
        let mut traffic_rng = self.traffic.draw_rng(
            &self.city.name,
            self.city.rotation(),
            self.traffic_redraw_epoch,
        );
        let rotation = self.city.rotation();
        let camera_rotation = self.camera_rotation;

        // XTHG actors are not a substitute for the supported XTRF road animation.
        // Their Mac lifecycle is incomplete, so only draw the shared Win95-style palette-mask overlay.
        let road_overlay_scale_band = TRAFFIC_ROAD_OVERLAY_SPRITE_OFFSET;
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let building = rotated_building_id(self.city.building_at(x, y), rotation);
                let Some(overlay) = win95_traffic_overlay_at(
                    &self.city,
                    x,
                    y,
                    |cx, cy| self.traffic_congestion_byte(cx, cy),
                    &mut traffic_rng,
                ) else {
                    continue;
                };
                let sub_elevation = self.city.structure_elevation_at(x, y) as f32;
                let (stx, sty) = rotate_tile_coords(x, y, camera_rotation);
                let tile_anchor =
                    ctx.origin + projected_tile_units(stx, sty, sub_elevation) * ctx.zoom;
                self.push_traffic_sprite(
                    renderer,
                    &mut traffic,
                    x,
                    y,
                    building,
                    overlay,
                    tile_anchor,
                    ctx.zoom,
                    road_overlay_scale_band,
                    ctx.anim_frame,
                );
            }
        }

        self.sprite_cache.traffic = traffic;
    }

    #[allow(clippy::too_many_arguments)]
    fn push_traffic_sprite(
        &self,
        renderer: &Renderer<'_>,
        sprites: &mut Vec<RenderSprite>,
        x: usize,
        y: usize,
        building: u8,
        overlay: crate::traffic::TrafficOverlay,
        structure_anchor: Vec2,
        zoom: f32,
        traffic_scale_band: u32,
        anim_frame: u8,
    ) {
        let render_id = traffic_render_sprite_id(renderer, overlay.sprite_id, traffic_scale_band);
        let (car_w, car_h) = renderer
            .traffic_atlas_for_sprite(render_id)
            .and_then(|atlas| renderer.sprite_size(atlas, render_id))
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((8.0, 4.0));
        let (road_w, road_h) = xbld_sprite_info(building, self.city.rotation())
            .and_then(|info| {
                renderer
                    .atlas_for_sprite(info.sprite_id)
                    .and_then(|atlas| renderer.sprite_size(atlas, info.sprite_id))
            })
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((ROAD_SPRITE_W, ROAD_SPRITE_H));
        let traffic_scale =
            traffic_dash_screen_scale(render_id, zoom, car_w, car_h, road_w, road_h);
        // This path draws the supported animated palette-mask mark at the road tile anchor.
        // It deliberately does not place individual car sprites on a sub-tile grid.
        let traffic_frame =
            traffic_anim_frame_for_tile(overlay.density, anim_frame, overlay.blit_flip);
        if let Some((atlas, sprite_id)) = resolve_traffic_sprite(
            renderer,
            overlay.sprite_id,
            traffic_scale_band,
            overlay.blit_flip,
            traffic_frame,
        ) {
            push_sprite_to_atlas_with_anchor(
                renderer,
                sprites,
                atlas,
                sprite_id,
                structure_anchor,
                traffic_scale,
                {
                    let (sx, sy) = rotate_tile_coords(x, y, self.camera_rotation);
                    sprite_depth(sx, sy, traffic_surface_layer_depth(building), 2)
                },
                crate::traffic::TRAFFIC_DECORATIVE_TINT,
                road_tile_overlay_flip_x(&self.city, x, y),
                false,
            );
        }
    }
}
