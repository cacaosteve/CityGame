//! High-level highway tool state transitions for drag, onramp, and placement.
//!
//! The methods retain their existing `ViewerState` API; lower-level route,
//! geometry, stamp, and neighbor-link decisions remain in sibling modules.

use super::*;

impl ViewerState {
    pub(in crate::app) fn begin_highway_drag(&mut self, x: usize, y: usize) {
        self.highway_drag = Some(HighwayDrag {
            start: (x, y),
            current: (x, y),
        });
    }

    pub(in crate::app) fn update_highway_drag_at(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        if let Some(tile) = self.tile_at_screen(screen, window_size, pixel_scale) {
            self.update_highway_drag_to(tile);
        }
    }

    pub(in crate::app) fn update_highway_drag_to(&mut self, tile: (usize, usize)) {
        if let Some(drag) = self.highway_drag.as_mut() {
            drag.current = tile;
        }
    }

    pub(in crate::app) fn finish_highway_drag(
        &mut self,
    ) -> (usize, Option<HighwayNeighborLinkCandidate>) {
        let Some(drag) = self.highway_drag.take() else {
            return (0, None);
        };
        let plan = self.highway_drag_plan_for_city(drag.start, drag.current);
        if let Some(endpoint) = plan.mac_dry_endpoint {
            return self.finish_mac_highway_dry_plan(&plan.path, endpoint);
        }
        let placed = self.place_highway_plan(&plan);
        let neighbor_link = (placed == plan.path.len())
            .then_some(plan.neighbor_link)
            .flatten();
        if placed == plan.path.len()
            && let Some(candidate) = plan.water_bridge
        {
            self.begin_highway_water_bridge(candidate);
        }
        (placed, neighbor_link)
    }

    pub(in crate::app) fn highway_drag_path_for_city(
        &self,
        start: (usize, usize),
        endpoint: (usize, usize),
    ) -> Vec<(usize, usize)> {
        self.highway_drag_plan_for_city(start, endpoint).path
    }

    pub(in crate::app) fn highway_drag_plan_for_city(
        &self,
        start: (usize, usize),
        endpoint: (usize, usize),
    ) -> HighwayDragPlan {
        if self.engine_fidelity != EngineFidelity::Win95 {
            return super::route::mac_highway_dry_plan(&self.city, start, endpoint);
        }
        let fallback_plan = || HighwayDragPlan {
            path: highway_drag_path(start, endpoint),
            ..HighwayDragPlan::default()
        };
        let Some(plan) = pe_highway_route_path_461ae0(&self.city, start, endpoint) else {
            return fallback_plan();
        };
        if self.highway_pe_route_path_has_simple_stamps(&plan) {
            plan
        } else {
            fallback_plan()
        }
    }

    pub(in crate::app) fn highway_pe_route_path_has_simple_stamps(
        &self,
        plan: &HighwayDragPlan,
    ) -> bool {
        for (index, &origin) in plan.path.iter().enumerate() {
            let axis = highway_axis_for_drag_plan_tile(self.engine_fidelity, plan, index);
            let placement = self.highway_segment_placement_at(origin, axis, index == 0);
            if !placement.invalid_tiles.is_empty()
                || placement.valid_tiles.len() != HIGHWAY_TILES_PER_SEGMENT
                || self.highway_segment_is_water_bridge(&placement.valid_tiles)
                || placement
                    .valid_tiles
                    .iter()
                    .any(|&(x, y)| highway_crossable_building(self.city.building_at(x, y)))
            {
                return false;
            }
            let allow_starting_on_highway =
                index == 0 && is_highway_tile(self.city.building_at(origin.0, origin.1));
            let axis_conflict = self.highway_segment_has_axis_conflict(
                axis,
                &placement.valid_tiles,
                allow_starting_on_highway,
            );
            if axis_conflict {
                // FUN_00461AE0 stops when its current 2×2 footprint contains
                // the perpendicular flat-highway XBLD class. FUN_004621F0
                // still stamps that terminal cursor, and FUN_004626D0 turns
                // it into the crossing. Do not route this rooted case through
                // the longer geometric fallback.
                let conflicts = placement
                    .valid_tiles
                    .iter()
                    .copied()
                    .filter(|&(x, y)| {
                        highway_conflicts_with_axis(
                            self.underground_surface_building_at(x, y),
                            axis,
                        )
                    })
                    .collect::<Vec<_>>();
                let is_terminal_flat_crossing = index + 1 == plan.path.len()
                    && !conflicts.is_empty()
                    && conflicts.iter().all(|&(x, y)| {
                        is_flat_highway_building(self.city.building_at(x, y))
                            && highway_conflicts_with_axis(self.city.building_at(x, y), axis)
                    })
                    && self
                        .highway_intersection_tiles(axis, &placement.valid_tiles)
                        .is_some();
                if !is_terminal_flat_crossing {
                    return false;
                }
            }
        }
        true
    }

    pub(in crate::app) fn cancel_highway_drag(&mut self) {
        self.highway_drag = None;
    }

    pub(in crate::app) fn highway_drag_active(&self) -> bool {
        self.highway_drag.is_some()
    }

    pub(in crate::app) fn highway_drag_cost(&self) -> Option<u32> {
        let drag = self.highway_drag?;
        let plan = self.highway_drag_plan_for_city(drag.start, drag.current);
        let placement = self.highway_drag_placement_for_plan(&plan);
        let segments = placement.valid_tiles.len() / HIGHWAY_TILES_PER_SEGMENT;
        (segments > 0).then_some(segments as u32 * HIGHWAY_COST_PER_SEGMENT)
    }

    pub(in crate::app) fn onramp_plan_at(&self, x: usize, y: usize) -> Option<OnrampPlan> {
        // PE MapTool Onramp `FUN_004417b0` early gates before neighbor masks:
        // `0x44183f` XBLD `>= 0x0e` refuse; `0x44184d` XBLD `== 5` refuse;
        // `0x441869` XTER `!= 0` refuse. Not PlaceTile-size (no `0x0D`/military
        // / water-tile share on this path).
        if x >= crate::city::CITY_WIDTH || y >= crate::city::CITY_HEIGHT {
            return None;
        }
        let building = self.city.building_at(x, y);
        if building >= 0x0e || building == 5 {
            return None;
        }
        if self.city.terrain_at(x, y) != 0 {
            return None;
        }

        let mut highway_mask = 0;
        let mut road_mask = 0;
        for (side, neighbor_x, neighbor_y) in directional_neighbor_tiles(x, y) {
            match self.city.building_at(neighbor_x, neighbor_y) {
                73..=80 => highway_mask |= side,
                29..=43 => road_mask |= side,
                _ => {}
            }
        }
        if !self.engine_fidelity.uses_win95() {
            // CODE 5 @ 0x4436 chooses the first surviving road bit in
            // north, east, south, west order, using only the raw XBLD masks.
            let compatible = mac_onramp_compatible_road_mask(highway_mask, road_mask);
            let (road_side, road_x, road_y) =
                directional_neighbor_tiles(x, y).find(|(side, _, _)| compatible & side != 0)?;
            let (preferred, other) = if matches!(road_side, NETWORK_TOP | NETWORK_BOTTOM) {
                (NETWORK_RIGHT, NETWORK_LEFT)
            } else {
                (NETWORK_TOP, NETWORK_BOTTOM)
            };
            let highway_side = if highway_mask & preferred != 0 {
                preferred
            } else {
                other
            };
            return Some(OnrampPlan {
                building: onramp_building_for_sides(highway_side, road_side)?,
                rotate: matches!(road_side, NETWORK_TOP | NETWORK_BOTTOM),
                road_tile: (road_x, road_y),
            });
        }
        let compatible_road_mask = pe_onramp_compatible_road_mask(highway_mask, road_mask);
        if compatible_road_mask == 0 {
            return None;
        }

        for (highway_side, highway_x, highway_y) in directional_neighbor_tiles(x, y) {
            let highway = self.city.building_at(highway_x, highway_y);
            if !is_flat_highway_building(highway)
                || highway_axis_for_building(highway) == network_side_axis(highway_side)
            {
                continue;
            }

            for (road_side, road_x, road_y) in directional_neighbor_tiles(x, y) {
                if network_side_axis(highway_side) == network_side_axis(road_side)
                    || !matches!(self.city.building_at(road_x, road_y), 29..=43)
                    || highway_axis_for_building(highway) != network_side_axis(road_side)
                    || compatible_road_mask & road_side == 0
                {
                    continue;
                }

                let road_axis = network_side_axis(highway_side);
                let road_mask = self.network_neighbor_mask(road_x, road_y, NetworkKind::Road);
                if network_axis_mask_for_side_mask(road_mask) & network_axis_mask(road_axis) == 0 {
                    continue;
                }

                return Some(OnrampPlan {
                    building: onramp_building_for_sides(highway_side, road_side)?,
                    rotate: matches!(highway_side, NETWORK_LEFT | NETWORK_RIGHT),
                    road_tile: (road_x, road_y),
                });
            }
        }
        None
    }

    pub(in crate::app) fn onramp_placement_at(&self, x: usize, y: usize) -> ToolPlacement {
        if let Some(plan) = self.onramp_plan_at(x, y) {
            ToolPlacement::valid(ToolPreview::Building(plan.building), vec![(x, y)])
        } else {
            ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview: ToolPreview::Building(93),
            }
        }
    }

    pub(in crate::app) fn place_onramp_at(&mut self, x: usize, y: usize) -> bool {
        let Some(plan) = self.onramp_plan_at(x, y) else {
            return false;
        };
        if !self.engine_fidelity.uses_win95() {
            // CODE 5 @ 0x4458 compares unsigned cash and subtracts $25.
            let funds = self.city.city_funds();
            let cost = 25;
            if (funds as u32) < cost {
                return false;
            }
            self.city.set_city_funds(funds.wrapping_sub(cost as i32));
            if plan.rotate {
                let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
                self.city.set_tile_bits(x, y, bits.with_rotate(true));
            }
            pe_place_tile_441f00(&mut self.city, plan.road_tile.0, plan.road_tile.1, 0x2b);
            pe_place_tile_441f00(&mut self.city, x, y, plan.building);
            self.pending_traffic_redraw = true;
            self.query_is_inspect = false;
            self.refresh_status_line();
            return true;
        }
        // PE MapTool Onramp @ `0x44197b`–`0x4419a3`: debit `0x4dc260[tool_id]` (id 3 → 25)
        // immediately after mask AND, before PlaceTile arms.
        if !pe_maptool_onramp_funds_debit_44197b(&mut self.city, 3) {
            return false;
        }

        self.clear_zone_growth_for_player_tiles(&[(x, y)]);
        // PE @ `0x4419db` / `0x441a65`: NS-road arms (`bl&1`/`bl&4`) OR click XBIT
        // `|= 0x02` (rotate) before PlaceTiles. EW-road arms leave XBIT alone.
        // `plan.rotate` is true iff highway is EW iff road is NS.
        if plan.rotate {
            let bits = self
                .city
                .tile_bits_at(x, y)
                .unwrap_or_default()
                .with_rotate(true);
            self.city.set_tile_bits(x, y, bits);
        }
        // PE MapToolOnramp @ `0x441a9b`: PlaceTile ramp XBLD (`0x40178f`→`0x441f00`).
        // PE leaves XZON alone (no type clear unlike PlaceRoads/Rails).
        pe_place_tile_441f00(&mut self.city, x, y, plan.building);
        // PE PlaceTile (`0x441f00`) never writes XTRF (`0x4ca940`).
        self.refresh_network_tile(plan.road_tile.0, plan.road_tile.1, NetworkKind::Road);
        // PE MapToolOnramp @ `0x4419e2`: PlaceTile adjacent road XBLD `0x2b` (43).
        pe_place_tile_441f00(&mut self.city, plan.road_tile.0, plan.road_tile.1, 43);
        self.pending_traffic_redraw = true;
        // PE PlaceTile path: no invent citywide water-network refresh.
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.query_is_inspect = false;
        self.refresh_status_line();
        true
    }

    pub(in crate::app) fn highway_placement_for_path(
        &self,
        path: &[(usize, usize)],
    ) -> ToolPlacement {
        let mut valid_tiles = Vec::new();
        let mut invalid_tiles = Vec::new();
        for (index, &origin) in path.iter().enumerate() {
            let axis = highway_axis_for_path_tile(self.engine_fidelity, path, index);
            let segment = self.highway_segment_placement_at(origin, axis, index == 0);
            valid_tiles.extend(segment.valid_tiles);
            invalid_tiles.extend(segment.invalid_tiles);
        }
        ToolPlacement {
            valid_tiles,
            invalid_tiles,
            preview: ToolPreview::Highway,
        }
    }

    pub(in crate::app) fn highway_drag_placement_for_path(
        &self,
        path: &[(usize, usize)],
    ) -> ToolPlacement {
        self.highway_drag_placement_for_plan(&HighwayDragPlan {
            mac_dry_endpoint: None,
            path: path.to_vec(),
            terminal_direction: None,
            neighbor_link: None,
            water_bridge: None,
        })
    }

    pub(in crate::app) fn highway_drag_placement_for_plan(
        &self,
        plan: &HighwayDragPlan,
    ) -> ToolPlacement {
        let mut valid_tiles = Vec::new();
        let mut invalid_tiles = Vec::new();
        if plan.mac_dry_endpoint.is_some() {
            for &origin in &plan.path {
                valid_tiles.extend(highway_segment_tiles(origin, NetworkAxis::GridX));
            }
            return ToolPlacement {
                valid_tiles,
                invalid_tiles,
                preview: ToolPreview::Highway,
            };
        }
        for (index, &origin) in plan.path.iter().enumerate() {
            let axis = highway_axis_for_drag_plan_tile(self.engine_fidelity, plan, index);
            let segment = self.highway_segment_placement_at(origin, axis, index == 0);
            if segment.invalid_tiles.is_empty() {
                valid_tiles.extend(segment.valid_tiles);
                let allow_starting_on_highway =
                    index == 0 && is_highway_tile(self.city.building_at(origin.0, origin.1));
                if self.highway_segment_has_axis_conflict(
                    axis,
                    &highway_segment_tiles(origin, axis),
                    allow_starting_on_highway,
                ) {
                    // Commit stamps this crossing and exits the route loop;
                    // preview and cost must end at the same terminal segment.
                    break;
                }
            } else {
                invalid_tiles.extend(segment.invalid_tiles);
                break;
            }
        }
        ToolPlacement {
            valid_tiles,
            invalid_tiles,
            preview: ToolPreview::Highway,
        }
    }

    pub(in crate::app) fn highway_segment_placement_at(
        &self,
        origin: (usize, usize),
        axis: NetworkAxis,
        allow_start_crossing: bool,
    ) -> ToolPlacement {
        let preview = ToolPreview::Highway;
        let footprint = highway_segment_tiles(origin, axis);
        if footprint.is_empty() {
            return ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![origin],
                preview,
            };
        }

        let footprint_has_crossing = footprint
            .iter()
            .any(|&(x, y)| highway_crossable_building(self.city.building_at(x, y)));
        let allow_crossing = allow_start_crossing && footprint_has_crossing;
        let allow_water_bridge = self.highway_segment_is_water_bridge(&footprint);
        let mut valid_tiles = Vec::new();
        let mut invalid_tiles = Vec::new();
        for &(x, y) in &footprint {
            if self.can_place_highway_tile(x, y, axis, allow_crossing, allow_water_bridge) {
                valid_tiles.push((x, y));
            } else {
                invalid_tiles.push((x, y));
            }
        }

        ToolPlacement {
            valid_tiles,
            invalid_tiles,
            preview,
        }
    }

    pub(in crate::app) fn can_place_highway_tile(
        &self,
        x: usize,
        y: usize,
        axis: NetworkAxis,
        allow_crossing: bool,
        allow_water_bridge: bool,
    ) -> bool {
        // PE PlaceHighway AtCoordinates @ `0x44bef9`–`0x44bf33`: refuse XBLD
        // `== 5` or XUND `!= 0` before slope walk. Flat CityTool uses `0x464040`
        // for the XBLD ladder (see below); slope still refuses `>= 0x0e`.
        let building = self.city.building_at(x, y);
        if building == 5 {
            return false;
        }
        if self.city.underground_at(x, y) != 0 {
            return false;
        }
        let _ = allow_crossing; // PE early refuse covers crossings (`>= 0x0e`).
        let _ = axis; // full slope walk / axis leftover after first-step probes.
        let terrain = self.city.terrain_at(x, y);
        // PE CityToolPlaceHighway @ `0x462200`–`0x4622c2`: even-snap the click,
        // then any of the 2×2 XBIT bit 2 (`water_tile`) refuses before the gate.
        let ox = x & !1;
        let oy = y & !1;
        if [
            (ox, oy),
            (ox.wrapping_add(1), oy),
            (ox, oy.wrapping_add(1)),
            (ox.wrapping_add(1), oy.wrapping_add(1)),
        ]
        .into_iter()
        .any(|(tx, ty)| {
            tx < 0x80
                && ty < 0x80
                && self
                    .city
                    .tile_bits_at(tx, ty)
                    .is_some_and(|bits| bits.water_tile())
        }) {
            return false;
        }
        // PE `0x4622da`–`0x4622e4`: same even origin, gate `0x8000` returns 0.
        if pe_highway_gate_early_8000_4640a1(&self.city, ox, oy) {
            return false;
        }
        // PE CityToolPlaceHighway `0x464040` @ `0x4640c1`: XBLD `0x0d` → `ax=0x8000`
        // refuse (`0x4622e4`). Slope AtCoordinates still allows `0x0d` (`< 0x0e`).
        if building == 0x0d {
            return false;
        }
        // PE `0x464040` @ `0x4640d5`: `cmp ax,0x1e` / `jle` continue — flat allows
        // XBLD `0x0e..=0x1e`. @ `0x4640e5`: `cmp ax,0x2d` / `jle` continue —
        // flat allows `0x2c`/`0x2d` (`0x1f..=0x2b` refuse @ `0x4640db`).
        // @ `0x4640a9`–`0x4640b3`: early continue `0x61..=0x6b`.
        // @ `0x4640b5`–`0x4640bf`: early continue `0x49..=0x50`. Other `> 0x1e`
        // still refuse (`0x2e..=0x48`, `0x51..=0x60`, `>= 0x6c`).
        if building > 0x1e
            && building != 0x2c
            && building != 0x2d
            && !(0x49..=0x50).contains(&building)
            && !(0x61..=0x6b).contains(&building)
        {
            return false;
        }
        // PE CityTool has no AtCoordinates `1..=4` dialog. A highway whose stamp
        // already wrote `XTER 0x0d` still continues (`0x4640b5`). Vacant `0x0d`
        // stays refused below.
        if terrain == 0x0d && is_highway_tile(building) {
            return true;
        }
        // Vacant `XTER 0x0d` stays the AtCoordinates dialog `0x74` retainer.
        if terrain == 0x0d {
            return false;
        }
        // Water-bridge invent is still not the CityTool stamp. Other dry faces
        // pass: PE `0x4622f6` has no AtCoordinates XTER dialog (`0x06` included).
        if is_water_terrain(terrain) {
            return allow_water_bridge;
        }
        true
    }

    /// PE PlaceHighway AtCoordinates @ `0x44bef9`–`0x44c0f8`. Not CityTool drag:
    /// XBLD `== 5` / XUND / `>= 0x0e` refuse, then the slope walk (dialog `0x72`
    /// without a terminal). Water-bit and gate `0x8000` are CityTool only.
    pub(in crate::app) fn can_place_highway_slope_atcoordinates(&self, x: usize, y: usize) -> bool {
        let building = self.city.building_at(x, y);
        if building == 5 || self.city.underground_at(x, y) != 0 || building >= 0x0e {
            return false;
        }
        let terrain = self.city.terrain_at(x, y);
        if !(1..=4).contains(&terrain) {
            return false;
        }
        self.place_highway_slope_walk_steps(x, y, terrain).is_some()
    }

    /// PE PlaceHighway AtCoordinates slope walk @ `0x44bf5b`–`0x44c0f8`.
    /// Returns PE `[ebp-0x10]` step count on success (cost uses `steps+1`).
    pub(in crate::app) fn place_highway_slope_walk_steps(
        &self,
        x: usize,
        y: usize,
        start_xter: u8,
    ) -> Option<u16> {
        let base = self.city.altitude_at(x, y) & 0x1f;
        let dir = ((u16::from(start_xter) + 2) & 3) as usize;
        let mut wx = x as isize;
        let mut wy = y as isize;
        let mut steps: u16 = 0;

        loop {
            // Body bounds `0..=0x7e` @ `0x44bf93`–`0x44bfb3` (dialog `0x70`).
            if !(0..0x7f).contains(&wx) || !(0..0x7f).contains(&wy) {
                return None;
            }
            let cx = wx as usize;
            let cy = wy as usize;
            // `ah & 0x7c` → dialog `0x6f`.
            if self.city.tunnel_levels_at(cx, cy) != 0 {
                return None;
            }
            let land = self.city.altitude_at(cx, cy) & 0x1f;
            let delta = i16::from(land) - i16::from(base);
            // land-delta `> 0x1e` → dialog `0x71`.
            if delta > 0x1e {
                return None;
            }
            // delta == 1 → XBLD refuse dialog `0x73` @ `0x44bfea`–`0x44c057`.
            if delta == 1 {
                let xbld = self.city.building_at(cx, cy);
                if place_highway_slope_delta1_xbld_refused(xbld) {
                    return None;
                }
            }

            // Step `(XTER+2)&3` via `0x4e7b18` / `0x4e7b20`; PE `inc [ebp-0x10]`.
            wx += PLACE_HIGHWAY_SLOPE_WALK_DX[dir];
            wy += PLACE_HIGHWAY_SLOPE_WALK_DY[dir];
            steps = steps.saturating_add(1);

            // After step: `dx < 0` → dialog `0x72`; out of `0..0x80` ends walk.
            if wx < 0 {
                return None;
            }
            if wx >= 0x80 || wy < 0 || wy >= 0x80 {
                break;
            }
            let next_land = self.city.altitude_at(wx as usize, wy as usize) & 0x1f;
            if next_land > base {
                continue;
            }
            break;
        }

        // Terminal tile in `0..0x80` with XTER == `((start+1)&3)+1` else dialog `0x72`.
        if !(0..0x80).contains(&wx) || !(0..0x80).contains(&wy) {
            return None;
        }
        let end_xter = self.city.terrain_at(wx as usize, wy as usize);
        let expect = ((u16::from(start_xter) + 1) & 3) as u8 + 1;
        (end_xter == expect).then_some(steps)
    }

    /// PE PlaceHighway AtCoordinates place: funds, origin PlaceTile+ALTM, climb ALTM,
    /// terminal PlaceTile. Skips `0x4017b2` / `0x401c76` / `0x4012c1`.
    pub(in crate::app) fn place_highway_slope_atcoordinates(&mut self, x: usize, y: usize) -> bool {
        if !self.can_place_highway_slope_atcoordinates(x, y) {
            return false;
        }
        let start_xter = self.city.terrain_at(x, y);
        if !(1..=4).contains(&start_xter) {
            return false;
        }
        let base_land = self.city.altitude_at(x, y) & 0x1f;
        let Some(steps) = self.place_highway_slope_walk_steps(x, y, start_xter) else {
            return false;
        };
        if !pe_place_highway_atcoordinates_funds_debit_44c267(&mut self.city, 1, steps) {
            return false;
        }
        pe_place_highway_atcoordinates_origin_place_44c2a6(&mut self.city, x, y, start_xter);
        pe_place_highway_atcoordinates_climb_and_terminal_44c347(
            &mut self.city,
            x,
            y,
            start_xter,
            steps,
            base_land,
        );
        true
    }

    pub(in crate::app) fn place_highway_path(&mut self, path: &[(usize, usize)]) -> usize {
        self.place_highway_path_with_terminal_direction(path, None)
    }

    pub(in crate::app) fn place_highway_plan(&mut self, plan: &HighwayDragPlan) -> usize {
        self.place_highway_path_with_terminal_direction(&plan.path, plan.terminal_direction)
    }

    fn finish_mac_highway_dry_plan(
        &mut self,
        path: &[(usize, usize)],
        endpoint: (usize, usize),
    ) -> (usize, Option<HighwayNeighborLinkCandidate>) {
        let Some(&start) = path.first() else {
            return (0, None);
        };
        let cost = path.len() as u32 * HIGHWAY_COST_PER_SEGMENT;
        if !self.can_afford_build_cost(cost) {
            return (0, None);
        }
        self.debit_build_cost(cost);
        let (placed, terminal, direction) =
            super::route::stamp_mac_highway_dry_route(&mut self.city, start, endpoint);
        self.pending_traffic_redraw = true;
        self.query_is_inspect = false;
        self.refresh_status_line();

        let next = pe_highway_route_step_coordinate_461ae0(terminal, direction);
        if let Some(seed) = next
            && super::water::pe_highway_route_footprint_has_water_xbit_461ae0(&self.city, seed)
        {
            if let Some(bridge) = mac_highway_water_bridge_candidate(&self.city, seed, 0, direction)
            {
                self.begin_highway_water_bridge(bridge);
            }
            return (placed, None);
        }

        // CODE 13 @ 0x20c9e also offers a link for a single footprint
        // on the border, even when the saved direction points inland.
        let link = next.is_none()
            || (terminal == start
                && super::route::pe_highway_origin_touches_outer_edge_461ae0(start));
        (
            placed,
            link.then_some(HighwayNeighborLinkCandidate {
                start,
                endpoint: terminal,
            }),
        )
    }

    pub(in crate::app) fn place_highway_path_with_terminal_direction(
        &mut self,
        path: &[(usize, usize)],
        terminal_direction: Option<u8>,
    ) -> usize {
        let affordable_segments = if self.tool == EditTool::Highway {
            self.affordable_build_units(HIGHWAY_COST_PER_SEGMENT)
        } else {
            usize::MAX
        };
        if affordable_segments == 0 {
            return 0;
        }

        let mut placed_segments = 0;
        let mut changed_origins = Vec::new();
        let mut changed_tiles = Vec::new();
        let mut water_bridge_run_index = 0usize;
        for (index, &origin) in path.iter().enumerate() {
            if placed_segments >= affordable_segments {
                break;
            }
            let axis = if terminal_direction.is_some() && index + 1 == path.len() {
                highway_axis_for_pe_direction(terminal_direction.unwrap())
            } else {
                highway_axis_for_path_tile(self.engine_fidelity, path, index)
            };
            let placement = self.highway_segment_placement_at(origin, axis, index == 0);
            if !placement.invalid_tiles.is_empty()
                || placement.valid_tiles.len() != HIGHWAY_TILES_PER_SEGMENT
            {
                break;
            }
            changed_tiles.extend(placement.valid_tiles.iter().copied());
            let allow_starting_on_highway =
                index == 0 && is_highway_tile(self.city.building_at(origin.0, origin.1));
            let stop_at_intersection = self.highway_segment_has_axis_conflict(
                axis,
                &placement.valid_tiles,
                allow_starting_on_highway,
            );
            let water_bridge = self.highway_segment_is_water_bridge(&placement.valid_tiles);
            let preferred_bridge = if water_bridge {
                Some(highway_bridge_building_for_run_index(
                    water_bridge_run_index,
                ))
            } else {
                water_bridge_run_index = 0;
                None
            };
            self.stamp_highway_segment(axis, &placement.valid_tiles, preferred_bridge);
            changed_origins.push(origin);
            if stop_at_intersection
                && let Some(intersection_tiles) =
                    self.highway_intersection_tiles(axis, &placement.valid_tiles)
                && let Some(intersection_origin) = footprint_origin(&intersection_tiles)
            {
                changed_origins.push(intersection_origin);
            }
            placed_segments += 1;
            if water_bridge {
                water_bridge_run_index += 1;
            }
            if stop_at_intersection {
                break;
            }
        }
        if placed_segments > 0 {
            self.clear_zone_growth_for_player_tiles(&changed_tiles);
            self.refresh_highway_segments_around(&changed_origins);
            // PE PlaceTile path: no invent citywide water-network refresh.
            self.refresh_live_misc_current_values();
            self.refresh_budget_service_rows_from_live_counts();
            self.refresh_reward_availability();
            self.debit_build_cost((placed_segments as u32) * HIGHWAY_COST_PER_SEGMENT);
            self.query_is_inspect = false;
            self.refresh_status_line();
        }
        placed_segments
    }

    pub(in crate::app) fn stamp_highway_segment(
        &mut self,
        axis: NetworkAxis,
        tiles: &[(usize, usize)],
        preferred_bridge_building: Option<u8>,
    ) {
        if self.highway_segment_is_water_bridge(tiles) {
            let building =
                self.highway_bridge_building_for_segment(tiles, preferred_bridge_building);
            // PE `FUN_004626d0` before PlaceTile: XZON `&= 0xf0` on footprint.
            pe_city_tool_place_highway_clear_xzon_4626d0(&mut self.city, tiles);
            self.stamp_flat_highway_tiles(building, tiles);
            return;
        }
        if self.highway_segment_has_axis_conflict(axis, tiles, false) {
            let Some(intersection_tiles) = self.highway_intersection_tiles(axis, tiles) else {
                return;
            };
            let Some((origin_x, origin_y)) = footprint_origin(&intersection_tiles) else {
                return;
            };
            // PE `FUN_004626d0` Get→Place. Mask `9` is `0x4e8950` class `0xb`
            // (building `0x68`), not a hardcoded 4-way `105`.
            pe_city_tool_place_highway_stamp_body_4626d0(&mut self.city, origin_x, origin_y, axis);
        } else if let Some((ox, oy)) = footprint_origin(tiles) {
            // PE `FUN_004626d0`: slope faces use the same Get→Place stamp as flat.
            pe_city_tool_place_highway_stamp_body_4626d0(&mut self.city, ox, oy, axis);
        }
    }

    pub(in crate::app) fn highway_building_for_segment(
        &self,
        axis: NetworkAxis,
        tiles: &[(usize, usize)],
    ) -> u8 {
        tiles
            .iter()
            .filter(|&&(x, y)| !is_water_terrain(self.city.terrain_at(x, y)))
            .find_map(|&(x, y)| {
                highway_slope_building_for_terrain(axis, self.city.terrain_at(x, y))
            })
            .unwrap_or_else(|| highway_building_for_axis(axis))
    }

    pub(in crate::app) fn highway_bridge_building_for_segment(
        &self,
        tiles: &[(usize, usize)],
        preferred: Option<u8>,
    ) -> u8 {
        if let Some(origin) = footprint_origin(tiles) {
            if let Some(neighbor) = self.highway_bridge_neighbor_building(origin) {
                return opposite_highway_bridge_building(neighbor);
            }
            let existing = self.city.building_at(origin.0, origin.1);
            if matches!(existing, 106 | 107) {
                return existing;
            }
        }
        preferred.unwrap_or(107)
    }

    pub(in crate::app) fn highway_bridge_neighbor_building(
        &self,
        origin: (usize, usize),
    ) -> Option<u8> {
        for (_, neighbor_origin) in highway_neighbor_origins(origin).into_iter().flatten() {
            let tiles = highway_segment_tiles(neighbor_origin, NetworkAxis::GridX);
            if !self.highway_segment_is_water_bridge(&tiles) {
                continue;
            }
            let building = self.city.building_at(neighbor_origin.0, neighbor_origin.1);
            if matches!(building, 106 | 107) {
                return Some(building);
            }
        }
        None
    }

    pub(in crate::app) fn highway_segment_has_axis_conflict(
        &self,
        axis: NetworkAxis,
        tiles: &[(usize, usize)],
        allow_starting_on_highway: bool,
    ) -> bool {
        tiles.iter().any(|&(x, y)| {
            let building = self.underground_surface_building_at(x, y);
            highway_conflicts_with_axis(building, axis)
                && !(allow_starting_on_highway && is_highway_tile(building))
        })
    }

    pub(in crate::app) fn highway_intersection_tiles(
        &self,
        axis: NetworkAxis,
        tiles: &[(usize, usize)],
    ) -> Option<Vec<(usize, usize)>> {
        let (min_x, min_y) = footprint_origin(tiles)?;
        let conflict_tile = tiles
            .iter()
            .copied()
            .find(|&(x, y)| highway_conflicts_with_axis(self.city.building_at(x, y), axis))?;
        let origin = match axis {
            NetworkAxis::GridY => {
                let y = if conflict_tile.1 > 0
                    && highway_conflicts_with_axis(
                        self.city.building_at(conflict_tile.0, conflict_tile.1 - 1),
                        axis,
                    ) {
                    conflict_tile.1 - 1
                } else {
                    conflict_tile.1
                };
                (min_x, y)
            }
            NetworkAxis::GridX => {
                let x = if conflict_tile.0 > 0
                    && highway_conflicts_with_axis(
                        self.city.building_at(conflict_tile.0 - 1, conflict_tile.1),
                        axis,
                    ) {
                    conflict_tile.0 - 1
                } else {
                    conflict_tile.0
                };
                (x, min_y)
            }
        };
        let intersection_tiles = highway_segment_tiles(origin, axis);
        (intersection_tiles.len() == HIGHWAY_TILES_PER_SEGMENT).then_some(intersection_tiles)
    }

    pub(in crate::app) fn refresh_highway_segments_around(&mut self, origins: &[(usize, usize)]) {
        let mut refresh_origins = HashSet::new();
        for &origin in origins {
            refresh_origins.insert(origin);
            for (_, neighbor_origin) in highway_neighbor_origins(origin).into_iter().flatten() {
                refresh_origins.insert(neighbor_origin);
            }
        }
        for origin in refresh_origins {
            self.refresh_highway_segment_at_origin(origin);
        }
    }

    pub(in crate::app) fn refresh_highway_segment_at_origin(&mut self, origin: (usize, usize)) {
        if !self.has_highway_segment_at_origin(origin) {
            return;
        }
        let water_tiles = highway_segment_tiles(origin, NetworkAxis::GridX);
        if self.highway_segment_is_water_bridge(&water_tiles) {
            let building = self.highway_bridge_building_for_segment(&water_tiles, None);
            self.stamp_flat_highway_tiles(building, &water_tiles);
            return;
        }
        let tiles = highway_segment_tiles(origin, NetworkAxis::GridX);
        // PE class `< 4` already rewrote low land to `XTER 0x0d` and underlay
        // `0x49`. The invent refresh must not replace that with an axis sprite.
        if tiles
            .iter()
            .any(|&(x, y)| self.city.terrain_at(x, y) == 0x0d)
        {
            return;
        }
        let mask = self.highway_neighbor_mask(origin);
        // Class `8..=0x0c` tiles are `0x65..=0x69`. Invent refresh must not
        // replace a Get→Place junction with the mask-table 4-way.
        if (0x65..=0x69).contains(&self.city.building_at(origin.0, origin.1))
            && mask.count_ones() >= 2
        {
            return;
        }
        // The PE Get→Place worker preserves the saved drag axis for an
        // isolated flat highway. A zero neighbor mask carries no orientation
        // evidence, so the generic refresh must not turn 0x49 into 0x4a (or
        // vice versa) after the native stamp has already chosen it.
        let existing = self.city.building_at(origin.0, origin.1);
        if mask == 0 && is_flat_highway_building(existing) {
            return;
        }
        let mut building = highway_building_for_mask(mask);
        let tiles = highway_segment_tiles(origin, highway_axis_for_building(building));
        if is_flat_highway_building(building) {
            building =
                self.highway_building_for_segment(highway_axis_for_building(building), &tiles);
        }
        if is_flat_highway_building(building) {
            self.stamp_flat_highway_tiles(building, &tiles);
        } else {
            self.stamp_building_footprint(origin.0, origin.1, building, &tiles);
        }
    }

    pub(in crate::app) fn stamp_flat_highway_tiles(
        &mut self,
        building: u8,
        tiles: &[(usize, usize)],
    ) {
        for &(x, y) in tiles {
            // PE PlaceTile (`0x441f00` / `0x40178f`): stamp XBLD; leave XZON/watered/piped/XTRF.
            pe_place_tile_441f00(&mut self.city, x, y, building);
        }
    }

    pub(in crate::app) fn has_highway_segment_at_origin(&self, origin: (usize, usize)) -> bool {
        let tiles = highway_segment_tiles(origin, NetworkAxis::GridX);
        tiles.len() == HIGHWAY_TILES_PER_SEGMENT
            && tiles
                .iter()
                .all(|&(x, y)| is_highway_tile(self.city.building_at(x, y)))
    }

    pub(in crate::app) fn highway_segment_is_water_bridge(&self, tiles: &[(usize, usize)]) -> bool {
        tiles.len() == HIGHWAY_TILES_PER_SEGMENT
            && tiles
                .iter()
                .all(|&(x, y)| is_water_terrain(self.city.terrain_at(x, y)))
    }

    pub(in crate::app) fn highway_neighbor_mask(&self, origin: (usize, usize)) -> u8 {
        let mut mask = 0;
        for (side, neighbor_origin) in highway_neighbor_origins(origin).into_iter().flatten() {
            if self.has_highway_segment_at_origin(neighbor_origin) {
                mask |= side;
            }
        }
        mask
    }
}
