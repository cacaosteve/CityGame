use super::*;

impl ViewerState {
    pub(in crate::app) fn begin_terrain_drag(&mut self, x: usize, y: usize) {
        let tool = self.tool;
        let target_altitude = self.city.altitude_at(x, y);
        let placed_tiles =
            usize::from(self.place_city_terrain_drag_tile(x, y, tool, target_altitude));
        self.terrain_drag = Some(TerrainDrag {
            current: (x, y),
            tool,
            target_altitude,
            placed_tiles,
        });
    }

    pub(in crate::app) fn update_terrain_drag_at(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> usize {
        self.tile_at_screen(screen, window_size, pixel_scale)
            .map_or(0, |tile| self.update_terrain_drag_to(tile))
    }

    pub(in crate::app) fn update_terrain_drag_to(&mut self, tile: (usize, usize)) -> usize {
        let Some(drag) = self.terrain_drag else {
            return 0;
        };
        if drag.current == tile {
            return 0;
        }

        let path = network_drag_path(drag.current, tile);
        let mut placed = 0;
        for (tx, ty) in path.into_iter().skip(1) {
            if self.place_city_terrain_drag_tile(tx, ty, drag.tool, drag.target_altitude) {
                placed += 1;
            }
        }
        if let Some(drag) = self.terrain_drag.as_mut() {
            drag.current = tile;
            drag.placed_tiles += placed;
        }
        placed
    }

    pub(in crate::app) fn finish_terrain_drag(&mut self) -> usize {
        self.terrain_drag.take().map_or(0, |drag| drag.placed_tiles)
    }

    pub(in crate::app) fn cancel_terrain_drag(&mut self) {
        self.terrain_drag = None;
    }

    pub(in crate::app) fn terrain_drag_active(&self) -> bool {
        self.terrain_drag.is_some()
    }

    pub(in crate::app) fn begin_edit_map_stretch_drag_at(
        &mut self,
        x: usize,
        y: usize,
        mouse_y: f32,
    ) -> bool {
        if self.mode != AppMode::EditNewMap || self.tool != EditTool::StretchTerrain {
            return false;
        }
        let start_land = self.city.altitude_at(x, y);
        if self.engine_fidelity == EngineFidelity::Win95 {
            self.pe_stretch_hold_cursor_id = 0x1fc;
            self.pe_stretch_hold_cursor_arg = -1;
        }
        self.edit_map_stretch_drag = Some(EditMapStretchDrag {
            tile: (x, y),
            start_mouse_y: mouse_y,
            start_land,
            last_land: start_land,
            changed: false,
        });
        true
    }

    pub(in crate::app) fn begin_edit_map_stretch_drag(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> bool {
        let Some((x, y)) = self.tile_at_screen(screen, window_size, pixel_scale) else {
            return false;
        };
        self.begin_edit_map_stretch_drag_at(x, y, screen.y)
    }

    pub(in crate::app) fn update_edit_map_stretch_drag_at(&mut self, mouse_y: f32) -> bool {
        let Some(mut drag) = self.edit_map_stretch_drag else {
            return false;
        };
        // CityGame's continuous zoom and backing scale map to the
        // original largest-view units (12 pixels per level).
        let delta = ((drag.start_mouse_y - mouse_y)
            / render_zoom(self.zoom, self.viewport_pixel_scale)) as i16;
        let height = match self.engine_fidelity {
            EngineFidelity::Mac68k => mac_terrain_editing::drag_target_height(
                drag.start_land,
                drag.last_land,
                delta,
                0,
                2,
            ),
            EngineFidelity::Win95 => {
                pe_stretch_terrain_mouse_land_444951(drag.start_land, drag.last_land, 0, delta, 2)
            }
        };
        let Some(height) = height else {
            return false;
        };
        let (x, y) = drag.tile;
        match self.engine_fidelity {
            EngineFidelity::Mac68k => mac_terrain_editing::apply_drag_height(
                &mut self.city,
                x,
                y,
                height,
                &mut self.mac_process_rng,
            ),
            EngineFidelity::Win95 => {
                let water = self.city.water_height_at(x, y);
                self.city.set_altitude(x, y, height, water);
                let waits = super::flatten::pe_smooth_and_refresh_terrain_neighbors(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    x,
                    y,
                );
                pe_terrain_neighbor_fun_00447020_447490(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    x,
                    y,
                );
                self.pe_stretch_hold_ui_wait_4012c1_count = self
                    .pe_stretch_hold_ui_wait_4012c1_count
                    .wrapping_add(1 + waits + PE_447490_UI_WAIT_4012C1_PER_CALL);
                self.pe_stretch_hold_ui_refresh_count =
                    self.pe_stretch_hold_ui_refresh_count.wrapping_add(1);
            }
        }
        drag.last_land = height;
        drag.changed = true;
        self.edit_map_stretch_drag = Some(drag);
        true
    }

    pub(in crate::app) fn finish_edit_map_stretch_drag(&mut self) -> usize {
        let changed = self.edit_map_stretch_drag.is_some_and(|drag| drag.changed);
        self.cancel_edit_map_stretch_drag();
        usize::from(changed)
    }

    pub(in crate::app) fn cancel_edit_map_stretch_drag(&mut self) {
        self.edit_map_stretch_drag = None;
        // PE Stretch exit at 0x444a57 restores cursor 0x1fc.
        if self.pe_stretch_hold_cursor_id == 0x1fc {
            self.pe_stretch_hold_cursor_id = 0;
            self.pe_stretch_hold_cursor_arg = 0;
        }
    }

    pub(in crate::app) fn edit_map_stretch_drag_active(&self) -> bool {
        self.edit_map_stretch_drag.is_some()
    }

    /// PE Spread `0x401758`→`0x4519e0` with stack arg (Spread uses `0`).
    pub(in crate::app) fn pe_spread_call_ui_cursor_4519e0(&mut self, arg: i32) {
        self.pe_spread_ui_cursor_4519e0_last_arg = arg;
        self.pe_spread_ui_cursor_4519e0_count =
            self.pe_spread_ui_cursor_4519e0_count.wrapping_add(1);
    }

    /// PE Spread tool-4 exit @ `0x442cd1`/`0x442cf0`: clear prev-tile XTXT then UI `0x40226b`.
    pub(in crate::app) fn finish_edit_map_spread_hold(&mut self) {
        let px = self.pe_spread_prev_tile_x;
        let py = self.pe_spread_prev_tile_y;
        if (0..0x80).contains(&(px as usize)) && (0..0x80).contains(&(py as usize)) {
            self.city.set_text_overlay(px as usize, py as usize, 0);
        }
        // PE Spread exit @ `0x442ce4`: `0x401758(0)` before UI refresh.
        self.pe_spread_call_ui_cursor_4519e0(0);
        // PE Spread exit @ `0x442cda`: `0x4012c1` UI wait.
        self.pe_spread_ui_wait_4012c1_count = self.pe_spread_ui_wait_4012c1_count.wrapping_add(1);
        self.pe_spread_hold_ui_refresh_count = self.pe_spread_hold_ui_refresh_count.wrapping_add(1);
        // PE Spread exit @ `0x442cab`/`0x442cb5`: `0x401ce9(0x1fc)` restore cursor.
        if self.pe_spread_hold_cursor_id == 0x1fc {
            self.pe_spread_hold_cursor_id = 0;
            self.pe_spread_hold_cursor_arg = 0;
        }
    }

    /// Cancel an active Spread hold via the same PE exit path when cursor `0x1fc` is live.
    pub(in crate::app) fn cancel_edit_map_spread_hold(&mut self) {
        if self.pe_spread_hold_cursor_id != 0x1fc {
            return;
        }
        self.finish_edit_map_spread_hold();
    }

    pub(in crate::app) fn begin_edit_map_level_drag_at(&mut self, x: usize, y: usize) -> bool {
        if self.mode != AppMode::EditNewMap
            || self.tool != EditTool::LevelTerrain
            || x >= CITY_WIDTH
            || y >= CITY_HEIGHT
        {
            return false;
        }
        self.cancel_edit_map_level_drag();
        self.edit_map_level_drag = Some(match self.engine_fidelity {
            EngineFidelity::Mac68k => {
                let Some(native) = mac_terrain_editing::LevelDrag::begin(&mut self.city, x, y)
                else {
                    return false;
                };
                EditMapLevelDrag::Mac68k {
                    native,
                    changed: false,
                }
            }
            EngineFidelity::Win95 => EditMapLevelDrag::Win95 {
                height: self.city.altitude_at(x, y),
                previous: None,
            },
        });
        self.sample_edit_map_level_drag(x, y);
        true
    }

    pub(in crate::app) fn begin_edit_map_level_drag(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) -> bool {
        let Some((x, y)) = self.tile_at_screen(screen, window_size, pixel_scale) else {
            return false;
        };
        self.begin_edit_map_level_drag_at(x, y)
    }

    pub(in crate::app) fn update_edit_map_level_drag(&mut self) -> bool {
        self.hover_tile
            .is_some_and(|(x, y)| self.sample_edit_map_level_drag(x, y))
    }

    fn sample_edit_map_level_drag(&mut self, x: usize, y: usize) -> bool {
        match self.edit_map_level_drag.as_mut() {
            Some(EditMapLevelDrag::Win95 { height, previous }) => {
                // 0x442a41 starts both previous coordinates at zero.
                let (px, py) = previous.unwrap_or((0, 0));
                if (x, y) == (px, py) {
                    return false;
                }
                super::flatten::pe_copy_level_height_to_neighbors(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    x,
                    y,
                    *height,
                    true,
                );
                self.city.set_text_overlay(px, py, 0);
                self.city.set_text_overlay(x, y, 0xc9);
                *previous = Some((x, y));
                true
            }
            Some(EditMapLevelDrag::Mac68k { native, changed }) => {
                let sampled = native.sample(&mut self.city, x, y, &mut self.mac_process_rng);
                *changed |= sampled;
                sampled
            }
            None => false,
        }
    }

    pub(in crate::app) fn finish_edit_map_level_drag(&mut self) -> usize {
        let changed = match self.edit_map_level_drag.take() {
            Some(EditMapLevelDrag::Win95 { previous, .. }) => {
                let (x, y) = previous.unwrap_or((0, 0));
                self.city.set_text_overlay(x, y, 0);
                previous.is_some()
            }
            Some(EditMapLevelDrag::Mac68k { native, changed }) => {
                native.finish(&mut self.city);
                changed
            }
            None => false,
        };
        usize::from(changed)
    }

    pub(in crate::app) fn cancel_edit_map_level_drag(&mut self) {
        self.finish_edit_map_level_drag();
    }

    pub(in crate::app) fn edit_map_level_drag_active(&self) -> bool {
        self.edit_map_level_drag.is_some()
    }

    pub(in crate::app) fn edit_map_sampled_height_tool(&self) -> bool {
        self.mode == AppMode::EditNewMap
            && matches!(self.tool, EditTool::RaiseTerrain | EditTool::LowerTerrain)
    }

    pub(in crate::app) fn begin_edit_map_height_hold(&mut self) -> bool {
        let Some((x, y)) = self.hover_tile else {
            return false;
        };
        self.edit_map_height_hold = Some(self.apply_tool_at_tile(x, y));
        true
    }

    pub(in crate::app) fn cancel_edit_map_height_hold(&mut self) {
        self.edit_map_height_hold = None;
    }

    pub(in crate::app) fn tick_edit_map_height_hold(&mut self, held: bool) -> bool {
        if !held || self.edit_map_height_hold.is_none() || !self.edit_map_sampled_height_tool() {
            return false;
        }
        let Some((x, y)) = self.hover_tile else {
            return false;
        };
        let changed = self.apply_tool_at_tile(x, y);
        if changed {
            self.edit_map_height_hold = Some(true);
        }
        changed
    }

    pub(in crate::app) fn city_terrain_edit_placement_at(
        &self,
        x: usize,
        y: usize,
    ) -> ToolPlacement {
        match self.tool {
            EditTool::LevelTerrain => {
                ToolPlacement::valid(ToolPreview::Terrain, self.city_terrain_level_tiles(x, y))
            }
            EditTool::RaiseTerrain
            | EditTool::LowerTerrain
            | EditTool::StretchTerrain
            | EditTool::SpreadTerrain => {
                if self.can_edit_city_terrain_tile(x, y) {
                    ToolPlacement::valid(ToolPreview::Terrain, vec![(x, y)])
                } else {
                    ToolPlacement {
                        valid_tiles: Vec::new(),
                        invalid_tiles: vec![(x, y)],
                        preview: ToolPreview::Terrain,
                    }
                }
            }
            _ => ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview: ToolPreview::Terrain,
            },
        }
    }

    pub(in crate::app) fn can_edit_city_terrain_tile(&self, x: usize, y: usize) -> bool {
        // PE FUN_00447020: no vacant-only / XBLD gate (DestroyBuilding for `>= 0x0D`,
        // skip PlaceTile for radioactivity `5`). Keep dispatch-actor refuse for UI safety.
        self.city.dispatch_actor_at(x, y).is_none()
    }

    pub(in crate::app) fn city_terrain_level_tiles(
        &self,
        x: usize,
        y: usize,
    ) -> Vec<(usize, usize)> {
        // PE DestroyStructure footprint (`0x402496` / `0x45b1c0`): highways are 2×2.
        pe_destroy_building_footprint(&self.city, x, y)
    }

    pub(in crate::app) fn place_city_terrain_edit_at(&mut self, x: usize, y: usize) -> bool {
        match self.tool {
            EditTool::RaiseTerrain => self.place_city_raise_terrain_at(x, y),
            EditTool::LowerTerrain => self.place_city_terrain_step_at(x, y, EditTool::LowerTerrain),
            EditTool::StretchTerrain => self.place_city_stretch_terrain_at(x, y),
            EditTool::LevelTerrain => {
                self.place_city_terrain_level_at(x, y, self.city.altitude_at(x, y))
            }
            EditTool::SpreadTerrain => self.place_city_spread_terrain_at(x, y),
            _ => false,
        }
    }

    pub(in crate::app) fn place_city_spread_terrain_at(&mut self, x: usize, y: usize) -> bool {
        if !self.can_edit_city_terrain_tile(x, y) {
            return false;
        }
        if !self.can_afford_tool_build_cost(EditTool::SpreadTerrain, 1) {
            return false;
        }
        // PE terrain tool-4 `0x402b94`→`0x442a30`.
        let (neighbor_447490, waits_445b50) = pe_terrain_nesw_land_copy_442a30_with_crt(
            &mut self.city,
            Some(&mut self.pe_process_crt),
            x,
            y,
        );
        // PE NESW loop @ `0x442bb3`→`0x445b50` / `0x442bbd`→`0x447490`.
        self.pe_spread_ui_wait_4012c1_count = self
            .pe_spread_ui_wait_4012c1_count
            .wrapping_add(waits_445b50)
            .wrapping_add(neighbor_447490.wrapping_mul(PE_447490_UI_WAIT_4012C1_PER_CALL));
        self.debit_tool_build_cost(EditTool::SpreadTerrain, 1);
        self.query_is_inspect = false;
        self.refresh_status_line();
        true
    }

    pub(in crate::app) fn place_city_stretch_terrain_at(&mut self, x: usize, y: usize) -> bool {
        if !self.can_edit_city_terrain_tile(x, y) {
            return false;
        }
        if !self.can_afford_tool_build_cost(EditTool::StretchTerrain, 1) {
            return false;
        }
        // PE StretchTerrain `0x443a50`: CRT jitter hill step (no FUN_00447020 destroy path).
        let Some(waits_445b50) = pe_stretch_terrain_hill_step_443a50_with_crt(
            &mut self.city,
            Some(&mut self.pe_process_crt),
            x,
            y,
        ) else {
            return false;
        };
        // PE @ `0x443b4a`: `0x4012c1` UI wait on success path.
        self.pe_stretch_hold_ui_wait_4012c1_count =
            self.pe_stretch_hold_ui_wait_4012c1_count.wrapping_add(1);
        // PE @ `0x443b94`→`0x445b50` / `0x443b9e`→`0x447490`.
        self.pe_stretch_hold_ui_wait_4012c1_count = self
            .pe_stretch_hold_ui_wait_4012c1_count
            .wrapping_add(waits_445b50)
            .wrapping_add(PE_447490_UI_WAIT_4012C1_PER_CALL);
        self.debit_tool_build_cost(EditTool::StretchTerrain, 1);
        self.query_is_inspect = false;
        self.refresh_status_line();
        true
    }

    pub(in crate::app) fn place_city_terrain_drag_tile(
        &mut self,
        x: usize,
        y: usize,
        tool: EditTool,
        target_altitude: u8,
    ) -> bool {
        match tool {
            EditTool::RaiseTerrain => self.place_city_raise_terrain_at(x, y),
            EditTool::LowerTerrain => self.place_city_terrain_step_at(x, y, tool),
            EditTool::StretchTerrain => self.place_city_stretch_terrain_at(x, y),
            EditTool::SpreadTerrain => self.place_city_spread_terrain_at(x, y),
            EditTool::LevelTerrain => self.place_city_terrain_level_at(x, y, target_altitude),
            _ => false,
        }
    }

    fn place_city_raise_terrain_at(&mut self, x: usize, y: usize) -> bool {
        if !self.engine_fidelity.uses_win95() {
            return self.place_city_terrain_step_at(x, y, EditTool::RaiseTerrain);
        }
        if !self.can_edit_city_terrain_tile(x, y) {
            return false;
        }
        // MENU 136 command 0x8041 selects Bulldozer group 0, Raise subtool 2.
        let waits = pe_raise_land_inc_443c60_with_crt(
            &mut self.city,
            Some(&mut self.pe_process_crt),
            x,
            y,
            2,
        );
        if !waits.is_some_and(|waits| waits > 0) {
            return false;
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
        true
    }

    pub(in crate::app) fn place_city_terrain_step_at(
        &mut self,
        x: usize,
        y: usize,
        tool: EditTool,
    ) -> bool {
        if !self.can_edit_city_terrain_tile(x, y) {
            return false;
        }
        if !self.can_afford_tool_build_cost(tool, 1) {
            return false;
        }
        // PE FUN_00447020: DestroyBuilding @ 0x447091 when XBLD >= 0x0D; PlaceTile(0) @
        // 0x4470ac when saved XBLD ≠ 5 (uses pre-destroy copy); clear XUND via
        // FUN_00442370 @ 0x4470c4 when underground ≠ 0.
        // Elevated `0x3f..=0x42`: PE DestroyBuilding elevated arm (`0x45994e`), not
        // DestroyStructure early-out (`0x46137b`).
        let building = self.city.building_at(x, y);
        if matches!(building, 0x3f..=0x42) {
            if self.engine_fidelity.uses_win95() {
                pe_destroy_building_mode1_optional_crt(
                    &mut self.city,
                    Some(&mut self.pe_process_crt),
                    x,
                    y,
                );
            } else {
                pe_clear_tunnel_portal_surface(&mut self.city, x, y);
                self.city.clear_altm_tunnel_bits(x, y);
            }
        } else if building >= 0x0d {
            self.pe_destroy_structure_at(x, y);
        }
        if building != 5 {
            pe_place_tile_441f00(&mut self.city, x, y, 0);
        }
        if self.city.underground_at(x, y) != 0 {
            // PE PlaceXUND `0x401e38` → `0x442370` with tile 0 (occupancy counter deferred).
            pe_place_xund_442370(&mut self.city, x, y, 0);
        }
        let sample = self.city.altm_at(x, y);
        let old_land = sample.land;
        let land = match tool {
            EditTool::RaiseTerrain => sample.land.saturating_add(1),
            EditTool::LowerTerrain => sample.land.saturating_sub(1),
            _ => return false,
        };
        self.city.set_altitude(x, y, land, sample.water);
        if land != old_land {
            // Share the recovered signed-WORD classifier tail after the
            // existing destroy/PlaceTile/XUND prefix and altitude step.
            let sea = pe_selected_water_height_for_fun_00447020(&self.city);
            pe_fun_00447020_mode0_xund_slope_sea(&mut self.city, x, y, sea);
        }

        // City-view raise/lower: Destroy/PlaceTile/XUND-clear then ALTM/XZON/XBIT/XTER; leave watered/piped/XTRF
        // when PlaceTile path (Destroy clears XTXT on footprint).
        self.debit_tool_build_cost(tool, 1);
        self.query_is_inspect = false;
        self.refresh_status_line();
        true
    }

    pub(in crate::app) fn place_city_terrain_level_at(
        &mut self,
        x: usize,
        y: usize,
        target_altitude: u8,
    ) -> bool {
        if self
            .city_terrain_level_tiles(x, y)
            .iter()
            .any(|&(tx, ty)| self.city.dispatch_actor_at(tx, ty).is_some())
        {
            return false;
        }
        if !self.can_afford_tool_build_cost(EditTool::LevelTerrain, 1) {
            return false;
        }
        self.level_city_terrain_to(x, y, target_altitude);
        self.debit_tool_build_cost(EditTool::LevelTerrain, 1);
        self.query_is_inspect = false;
        self.refresh_status_line();
        true
    }

    pub(in crate::app) fn level_city_terrain_to(
        &mut self,
        x: usize,
        y: usize,
        target_altitude: u8,
    ) -> Vec<(usize, usize)> {
        let tiles = self.clear_surface_for_level_terrain_at(x, y);
        for &(tx, ty) in &tiles {
            // PE LevelTerrain @ `0x4449c4`: `0x4012c1` before ALTM write.
            self.pe_level_hold_ui_wait_4012c1_count =
                self.pe_level_hold_ui_wait_4012c1_count.wrapping_add(1);
            let sample = self.city.altm_at(tx, ty);
            self.city
                .set_altitude(tx, ty, target_altitude, sample.water);
            // PE LevelTerrain `0x402b2b`→`0x4448f0` @ `0x444a10`: `0x401a78`→`0x445b50`
            // after ALTM write; `0x40215d`→`0x447490` neighbor FUN_00447020 (sound skipped).
            let waits_445b50 = pe_post_flatten_neighbor_altm_445b50(&mut self.city, tx, ty);
            pe_terrain_neighbor_fun_00447020_447490(
                &mut self.city,
                Some(&mut self.pe_process_crt),
                tx,
                ty,
            );
            self.pe_level_hold_ui_wait_4012c1_count = self
                .pe_level_hold_ui_wait_4012c1_count
                .wrapping_add(waits_445b50)
                .wrapping_add(PE_447490_UI_WAIT_4012C1_PER_CALL);
        }
        tiles
    }

    pub(in crate::app) fn clear_surface_for_level_terrain_at(
        &mut self,
        x: usize,
        y: usize,
    ) -> Vec<(usize, usize)> {
        let old_building = self.city.building_at(x, y);
        let old_kinds: Vec<_> = network_kinds_for_building(old_building).collect();
        let tiles = self.city_terrain_level_tiles(x, y);
        self.clear_building_underground_side_effects(x, y, old_building);

        for &(tx, ty) in &tiles {
            if is_decorative_water_terrain(self.city.terrain_at(tx, ty)) {
                let sample = self.city.altm_at(tx, ty);
                self.city.set_terrain(tx, ty, 0);
                self.city.set_altitude(tx, ty, sample.land, 0);
            }
        }
        // PE DestroyStructure (`0x401604` → `0x461330`): CRT PlaceTile rubble; leave XZON/XTRF/watered.
        pe_flood_destroy_tile_with_crt(&mut self.city, &mut self.pe_process_crt, x, y);
        if matches!(old_building, 220 | 250) {
            self.refresh_pipe_neighbors_of_tiles(&tiles);
        }
        // PE DestroyStructure: no invent citywide water-network refresh.
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();

        for kind in old_kinds {
            for &(tx, ty) in &tiles {
                for (nx, ny) in neighbor_tiles(tx, ty) {
                    self.refresh_network_tile(nx, ny, kind);
                }
            }
            self.pending_traffic_redraw = true;
        }

        tiles
    }

    pub(in crate::app) fn edit_map_sea_level(&self) -> u8 {
        let mut sea_level: Option<u8> = None;
        for y in 0..CITY_HEIGHT {
            for x in 0..CITY_WIDTH {
                let sample = self.city.altm_at(x, y);
                if sample.water > sample.land || is_water_terrain(self.city.terrain_at(x, y)) {
                    sea_level = Some(sea_level.unwrap_or(0).max(sample.water));
                }
            }
        }
        sea_level.unwrap_or(BLANK_MAP_BASE_HEIGHT)
    }

    pub(in crate::app) fn adjust_edit_map_sea_level(&mut self, delta: i8) -> usize {
        if self.mode != AppMode::EditNewMap || !matches!(delta, -1 | 1) {
            return 0;
        }
        self.cancel_edit_map_level_drag();
        self.cancel_edit_map_height_hold();
        let Some((next, affected)) =
            terrain_water_level::step(&mut self.city, self.engine_fidelity, delta > 0)
        else {
            return 0;
        };
        self.query_is_inspect = false;
        self.query_line = format!("Sea Level {next}");
        affected
    }
}
