use super::*;

impl ViewerState {
    pub(in crate::app) fn begin_network_drag(&mut self, x: usize, y: usize, kind: NetworkKind) {
        self.network_drag = Some(NetworkDrag {
            start: (x, y),
            current: (x, y),
            kind,
        });
    }

    pub(in crate::app) fn update_network_drag_at(
        &mut self,
        screen: Vec2,
        window_size: (u32, u32),
        pixel_scale: f32,
    ) {
        if let Some(tile) = self.tile_at_screen(screen, window_size, pixel_scale) {
            self.update_network_drag_to(tile);
        }
    }

    pub(in crate::app) fn update_network_drag_to(&mut self, tile: (usize, usize)) {
        if let Some(drag) = self.network_drag.as_mut() {
            drag.current = tile;
        }
    }

    pub(in crate::app) fn finish_network_drag(
        &mut self,
    ) -> Option<(
        NetworkKind,
        usize,
        Option<RailNeighborLinkCandidate>,
        Option<RoadNeighborLinkCandidate>,
    )> {
        if let Some(drag) = self.network_drag.take() {
            let selected_win95_rail = drag.kind == NetworkKind::Rail
                && network_kind_for_tool(self.tool) == Some(NetworkKind::Rail)
                && self.engine_fidelity == EngineFidelity::Win95;
            let selected_win95_power = drag.kind == NetworkKind::Power
                && network_kind_for_tool(self.tool) == Some(NetworkKind::Power)
                && self.engine_fidelity.uses_win95();
            // Preserve the singleton no-progress arm when a corridor has
            // water and therefore stays on the existing bridge planner.
            let singleton_rail_neighbor_link = (selected_win95_rail
                && drag.start == drag.current
                && pe_network_route_origin_touches_outer_edge(drag.start))
            .then_some(RailNeighborLinkCandidate {
                endpoint: drag.start,
            });
            let (placed_tiles, rail_neighbor_link, road_neighbor_link) = if selected_win95_rail {
                if let Some(result) = self.place_win95_rail_stroke_449f50(drag.start, drag.current)
                {
                    (result.0, result.1, None)
                } else {
                    let path = network_commit_path_for_kind(drag.start, drag.current, drag.kind);
                    (
                        self.place_network_path(&path, drag.kind),
                        singleton_rail_neighbor_link,
                        None,
                    )
                }
            } else if selected_win95_power {
                (
                    self.place_win95_power_stroke_44a310(drag.start, drag.current),
                    None,
                    None,
                )
            } else if drag.kind == NetworkKind::Road
                && network_kind_for_tool(self.tool) == Some(NetworkKind::Road)
                && self.engine_fidelity.uses_win95()
            {
                let (placed_tiles, neighbor_link) =
                    self.place_win95_road_stroke_449bb0(drag.start, drag.current);
                (placed_tiles, None, neighbor_link)
            } else if self.uses_mac_surface_tool(drag.kind) {
                self.place_mac_surface_stroke(drag)
            } else {
                let path = network_commit_path_for_kind(drag.start, drag.current, drag.kind);
                (self.place_network_path(&path, drag.kind), None, None)
            };
            return Some((
                drag.kind,
                placed_tiles,
                rail_neighbor_link,
                road_neighbor_link,
            ));
        }
        None
    }

    pub(in crate::app) fn cancel_network_drag(&mut self) {
        self.network_drag = None;
    }

    pub(in crate::app) fn network_drag_active(&self) -> bool {
        self.network_drag.is_some()
    }

    pub(in crate::app) fn network_drag_cost(&self) -> Option<(NetworkKind, u32)> {
        let drag = self.network_drag?;
        let per_tile = drag.kind.cost_per_tile()?;
        if self.uses_mac_surface_tool(drag.kind) {
            let path = self.mac_surface_preview(drag);
            let cost = pe_network_surface_stroke_cost_448a90(&self.city, &path, per_tile);
            return Some((drag.kind, cost));
        }
        let path = self.network_drag_preview_path(drag);
        let placement = self.network_drag_placement_for_path(&path, drag.kind);
        let selected_win95_power = drag.kind == NetworkKind::Power
            && network_kind_for_tool(self.tool) == Some(NetworkKind::Power)
            && self.engine_fidelity.uses_win95();
        let cost = if selected_win95_power {
            if pe_road_start_is_surface_water_449bb0(&self.city, drag.start) {
                0
            } else {
                pe_road_stroke_plan_449bb0(&self.city, drag.start, drag.current).map_or(0, |plan| {
                    pe_network_surface_stroke_cost_448a90(&self.city, &plan.path, per_tile)
                })
            }
        } else if drag.kind == NetworkKind::Road
            && network_kind_for_tool(self.tool) == Some(NetworkKind::Road)
            && self.engine_fidelity.uses_win95()
        {
            pe_road_stroke_plan_449bb0(&self.city, drag.start, drag.current).map_or(0, |plan| {
                if plan.water_bridge_candidate.is_some()
                    || pe_road_start_is_surface_water_449bb0(&self.city, drag.start)
                {
                    pe_network_surface_stroke_cost_448a90(
                        &self.city,
                        &placement.valid_tiles,
                        per_tile,
                    )
                } else {
                    pe_network_surface_stroke_cost_448a90(&self.city, &plan.path, per_tile)
                }
            })
        } else if self.engine_fidelity.uses_win95() {
            pe_network_surface_stroke_cost_448a90(&self.city, &placement.valid_tiles, per_tile)
        } else {
            placement.valid_tiles.len() as u32 * per_tile
        };
        Some((drag.kind, cost))
    }

    pub(in crate::app) fn network_drag_preview_path(
        &self,
        drag: NetworkDrag,
    ) -> Vec<(usize, usize)> {
        if drag.kind == NetworkKind::Power
            && network_kind_for_tool(self.tool) == Some(NetworkKind::Power)
            && self.engine_fidelity.uses_win95()
        {
            if pe_road_start_is_surface_water_449bb0(&self.city, drag.start) {
                return network_preview_path_for_kind(drag.start, drag.current, drag.kind);
            }
            if let Some(plan) = pe_road_stroke_plan_449bb0(&self.city, drag.start, drag.current) {
                if plan.water_bridge_candidate.is_some() {
                    return network_preview_path_for_kind(drag.start, drag.current, drag.kind);
                }
                return plan.path;
            }
            return Vec::new();
        }
        if drag.kind == NetworkKind::Road
            && network_kind_for_tool(self.tool) == Some(NetworkKind::Road)
            && self.engine_fidelity.uses_win95()
        {
            if let Some(plan) = pe_road_stroke_plan_449bb0(&self.city, drag.start, drag.current) {
                if pe_road_start_is_surface_water_449bb0(&self.city, drag.start)
                    || plan.water_bridge_candidate.is_some()
                {
                    return network_preview_path_for_kind(drag.start, drag.current, drag.kind);
                }
                return plan.path;
            }
            return Vec::new();
        }
        if self.uses_mac_surface_tool(drag.kind) {
            return self.mac_surface_preview(drag);
        }
        network_preview_path_for_kind(drag.start, drag.current, drag.kind)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::app) fn push_network_preview(
        &self,
        renderer: &Renderer<'_>,
        window_size: (u32, u32),
        pixel_scale: f32,
        sprites: &mut Vec<RenderSprite>,
        x: usize,
        y: usize,
        kind: NetworkKind,
        virtual_tiles: &HashSet<(usize, usize)>,
    ) {
        let building = network_building_for_tile(
            kind,
            self.network_neighbor_mask_with_virtual(x, y, kind, virtual_tiles),
            self.city.terrain_at(x, y),
        );
        let Some(info) = xbld_sprite_info(building, self.city.rotation()) else {
            return;
        };
        let (anchor, zoom, sx, sy) = self.structure_anchor(window_size, pixel_scale, x, y);
        push_sprite(
            renderer,
            sprites,
            info.sprite_id,
            anchor,
            zoom,
            sprite_depth(sx, sy, info.layer_depth, info.depth_adjustment + 96),
            [0.8, 1.0, 1.0, 0.8],
            self.animation_frame(),
        );
    }

    pub(in crate::app) fn place_network_at(
        &mut self,
        x: usize,
        y: usize,
        kind: NetworkKind,
    ) -> usize {
        let drag = NetworkDrag {
            start: (x, y),
            current: (x, y),
            kind,
        };
        if self.uses_mac_surface_tool(kind) {
            return self.place_mac_surface_stroke(drag).0;
        }
        self.place_network_path(&[(x, y)], kind)
    }

    /// Replay the mutable Win95 Road selector-0 stroke from `FUN_00449BB0` /
    /// `FUN_00448F50`, including the bridge lookahead after the last dry write.
    pub(in crate::app) fn place_win95_road_stroke_449bb0(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
    ) -> (usize, Option<RoadNeighborLinkCandidate>) {
        if pe_road_start_is_surface_water_449bb0(&self.city, start) {
            let Some(direction) = pe_road_bridge_mode1_direction(&self.city, start) else {
                return (0, None);
            };
            return (self.begin_win95_road_bridge(start, direction, true), None);
        }
        let Some(plan) = pe_road_stroke_plan_449bb0(&self.city, start, end) else {
            return (0, None);
        };

        let cost = pe_network_surface_stroke_cost_448a90(
            &self.city,
            &plan.path,
            NetworkKind::Road.cost_per_tile().unwrap_or(0),
        );
        if !self.can_afford_build_cost(cost) {
            return (0, None);
        }

        self.debit_build_cost(cost);

        let Some(walk) = pe_road_stroke_walk_init_448bc0(&self.city, start, end) else {
            return (0, None);
        };
        let mut current = start;
        let mut direction = walk.direction;
        let mut applied_path = Vec::new();

        for _ in 0..(CITY_WIDTH + CITY_HEIGHT) {
            // PE asks for the next coordinate before editing/writing this one;
            // the writer's reshapes then affect the following live query.
            let step = pe_road_next_stroke_tile_448f50(
                &self.city,
                current,
                walk.target,
                walk.bounds,
                direction,
            );
            direction = step.direction;
            pe_network_surface_edit_448e20(&mut self.city, current.0, current.1, direction);

            let existing = self.city.building_at(current.0, current.1);
            if pe_place_roads_coordinate_should_stamp_44acf4(existing)
                && crossover_network_axis(existing, NetworkKind::Road).is_none()
            {
                let building = pe_place_roads_crossover_from_existing_44ae24(existing)
                    .unwrap_or_else(|| NetworkKind::Road.base_tile());
                pe_place_tile_441f00(&mut self.city, current.0, current.1, building);
                pe_place_network_clear_zone_type_nibble(
                    &mut self.city,
                    current.0,
                    current.1,
                    NetworkKind::Road,
                );
                applied_path.push(current);
                self.refresh_road_neighbors_like_pe_44acc0(current.0, current.1);
            }

            if let Some(next) = step.next {
                current = next;
            } else {
                break;
            }
        }

        // FUN_00449BB0 probes one cell beyond its final mutable cursor. An
        // off-map lookahead offers the $1,000 network-bucket confirmation;
        // an in-map lookahead offers it only for a no-progress edge click.
        let lookahead = pe_pipe_step_coordinate_44f50(current, direction);
        let neighbor_link = if lookahead.is_none()
            || (current == start && pe_network_route_origin_touches_outer_edge(current))
        {
            Some(RoadNeighborLinkCandidate { endpoint: current })
        } else {
            None
        };

        self.clear_zone_growth_for_player_tiles(&applied_path);
        self.pending_traffic_redraw = true;
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.query_is_inspect = false;
        self.refresh_status_line();
        let bridge_tiles = lookahead
            .filter(|&seed| pe_road_start_is_surface_water_449bb0(&self.city, seed))
            .map(|seed| self.begin_win95_road_bridge(seed, direction, false))
            .unwrap_or(0);
        (
            applied_path.len().saturating_add(bridge_tiles),
            neighbor_link,
        )
    }

    /// Replay Win95 Rail mode 1, including the final bridge and edge lookahead.
    pub(in crate::app) fn place_win95_rail_stroke_449f50(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
    ) -> Option<(usize, Option<RailNeighborLinkCandidate>)> {
        if pe_road_start_is_surface_water_449bb0(&self.city, start) {
            let Some(direction) = pe_road_bridge_mode1_direction(&self.city, start) else {
                return Some((0, None));
            };
            self.begin_win95_rail_bridge(start, direction, true, None);
            return Some((0, None));
        }
        let Some(walk) = pe_road_stroke_walk_init_448bc0(&self.city, start, end) else {
            return Some((0, None));
        };
        let Some(plan) = pe_rail_stroke_plan_449f50(&self.city, start, end) else {
            return Some((0, None));
        };
        let cost = pe_network_surface_stroke_cost_448a90(
            &self.city,
            &plan.path,
            NetworkKind::Rail.cost_per_tile().unwrap_or(0),
        );
        if !self.can_afford_build_cost(cost) {
            return Some((0, None));
        }
        self.debit_build_cost(cost);

        let mut current = start;
        let mut direction = walk.direction;
        let mut applied_path = Vec::new();
        for _ in 0..(CITY_WIDTH + CITY_HEIGHT) {
            // PE probes the next mutable cursor before editing and writing the
            // current cell; the live writer's retile affects the next probe.
            let step = pe_rail_next_stroke_tile_449f50(
                &self.city,
                current,
                walk.target,
                walk.bounds,
                direction,
            );
            direction = step.direction;
            pe_network_surface_edit_448e20(&mut self.city, current.0, current.1, direction);
            let existing = self.city.building_at(current.0, current.1);
            if pe_place_rails_coordinate_should_stamp_44b1a0(existing)
                && crossover_network_axis(existing, NetworkKind::Rail).is_none()
            {
                let building = pe_place_rails_crossover_from_existing_44b2f4(existing)
                    .unwrap_or_else(|| NetworkKind::Rail.base_tile());
                pe_place_tile_441f00(&mut self.city, current.0, current.1, building);
                pe_place_network_clear_zone_type_nibble(
                    &mut self.city,
                    current.0,
                    current.1,
                    NetworkKind::Rail,
                );
                applied_path.push(current);
                self.refresh_rail_neighbors_like_pe_44b1a0(current.0, current.1);
            }

            if let Some(next) = step.next {
                current = next;
            } else {
                break;
            }
        }

        let lookahead = pe_pipe_step_coordinate_44f50(current, direction);
        let neighbor_link = if lookahead.is_none()
            || (current == start && pe_network_route_origin_touches_outer_edge(current))
        {
            if self.city.is_synthetic_trip_marker(current.0, current.1) {
                self.city.set_text_overlay(current.0, current.1, 0);
            }
            Some(RailNeighborLinkCandidate { endpoint: current })
        } else {
            None
        };

        self.clear_zone_growth_for_player_tiles(&applied_path);
        self.pending_traffic_redraw = true;
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.query_is_inspect = false;
        self.refresh_status_line();
        let choosing_bridge = lookahead
            .filter(|&seed| pe_road_start_is_surface_water_449bb0(&self.city, seed))
            .is_some_and(|seed| {
                self.begin_win95_rail_bridge(seed, direction, false, neighbor_link)
            });
        Some((
            applied_path.len(),
            if choosing_bridge { None } else { neighbor_link },
        ))
    }

    /// Replay Win95 Power `FUN_0044A310`: shared mode-2 surface walk, per-cell
    /// `FUN_00448E20`/`FUN_0044A8A0`, then its one-cell water lookahead.
    pub(in crate::app) fn place_win95_power_stroke_44a310(
        &mut self,
        start: (usize, usize),
        end: (usize, usize),
    ) -> usize {
        if pe_road_start_is_surface_water_449bb0(&self.city, start) {
            let Some(direction) = pe_road_bridge_mode1_direction(&self.city, start) else {
                return 0;
            };
            self.begin_power_bridge(start, direction, false);
            return 0;
        }
        let Some(walk) = pe_road_stroke_walk_init_448bc0(&self.city, start, end) else {
            return 0;
        };
        // Modes 0 (Road) and 2 (Power) take the same XBLD surface branch in
        // FUN_00448F50; only mode 1 has the additional direction restriction.
        let Some(plan) = pe_road_stroke_plan_449bb0(&self.city, start, end) else {
            return 0;
        };
        let cost = pe_network_surface_stroke_cost_448a90(
            &self.city,
            &plan.path,
            NetworkKind::Power.cost_per_tile().unwrap_or(0),
        );
        if !self.can_afford_build_cost(cost) {
            return 0;
        }
        self.debit_build_cost(cost);

        let mut current = start;
        let mut direction = walk.direction;
        let mut applied_path = Vec::new();
        for _ in 0..(CITY_WIDTH + CITY_HEIGHT) {
            let step = pe_road_next_stroke_tile_448f50(
                &self.city,
                current,
                walk.target,
                walk.bounds,
                direction,
            );
            direction = step.direction;
            pe_network_surface_edit_448e20(&mut self.city, current.0, current.1, direction);
            if self.place_power_line_coordinate(current) {
                applied_path.push(current);
            }
            if let Some(next) = step.next {
                current = next;
            } else {
                break;
            }
        }

        // FUN_0044A310 advances the final mutable cursor once even when the
        // route walker stopped at its target; a water cell there is handed to
        // the shared bridge worker in mode 0.
        let bridge_seed = pe_pipe_step_coordinate_44f50(current, direction).filter(|&(x, y)| {
            self.city.terrain_at(x, y) < 0x40
                && self
                    .city
                    .tile_bits_at(x, y)
                    .is_some_and(TileBits::water_tile)
        });

        self.clear_zone_growth_for_player_tiles(&applied_path);
        self.pending_traffic_redraw = true;
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.query_is_inspect = false;
        self.refresh_status_line();
        if !bridge_seed.is_some_and(|seed| self.begin_power_bridge(seed, direction, true)) {
            pe_powerline_post_place_power_recalc_43f396(&mut self.city, &mut self.pe_process_crt);
        }
        applied_path.len()
    }

    /// PE `FUN_0044A8A0`: powerable early return, base/crossover write, XBIT
    /// powerable flag, then center and cardinal-neighbor reshapes.
    pub(in crate::app) fn place_power_line_coordinate(&mut self, tile: (usize, usize)) -> bool {
        let (x, y) = tile;
        if self
            .city
            .tile_bits_at(x, y)
            .is_some_and(TileBits::powerable)
        {
            return false;
        }
        let existing = self.city.building_at(x, y);
        let building = if existing < 0x0e {
            NetworkKind::Power.base_tile()
        } else if let Some(crossover) = pe_place_power_crossover_from_existing_44a9e8(existing) {
            crossover
        } else {
            return false;
        };
        pe_place_tile_441f00(&mut self.city, x, y, building);
        stamp_xbit_powerable(&mut self.city, x, y);
        if self.engine_fidelity.uses_win95() {
            self.refresh_power_neighbors_like_pe_44a8a0(x, y);
        } else {
            for (nx, ny) in std::iter::once(tile).chain(neighbor_tiles(x, y)) {
                self.refresh_mac_power_tile(nx, ny);
            }
        }
        true
    }

    pub(in crate::app) fn place_network_path(
        &mut self,
        path: &[(usize, usize)],
        kind: NetworkKind,
    ) -> usize {
        let selected_tool = network_kind_for_tool(self.tool) == Some(kind);
        let pe_surface_contract = selected_tool && self.engine_fidelity.uses_win95();
        let pe_power_coordinate_contract = pe_surface_contract && kind == NetworkKind::Power;
        let stroke_path = if pe_surface_contract {
            self.network_drag_placement_for_path(path, kind).valid_tiles
        } else {
            Vec::new()
        };
        // Keep water-crossing strokes on the existing bridge-run planner:
        // caller seeds and the shared PE worker are rooted, but their mutable
        // mouse-walk has not been replayed against this bank-inclusive path or
        // compared through endpoint writes. The per-coordinate land worker
        // alone does not model that separate water-bridge transaction.
        let pe_road_coordinate_contract = pe_surface_contract
            && kind == NetworkKind::Road
            && stroke_path
                .iter()
                .all(|&(x, y)| !is_water_terrain(self.city.terrain_at(x, y)));
        let pe_rail_coordinate_contract = pe_surface_contract
            && kind == NetworkKind::Rail
            && stroke_path
                .iter()
                .all(|&(x, y)| !is_water_terrain(self.city.terrain_at(x, y)));
        let pe_stroke_cost = kind.cost_per_tile().map_or(0, |per_tile| {
            pe_surface_contract
                .then(|| pe_network_surface_stroke_cost_448a90(&self.city, &stroke_path, per_tile))
                .unwrap_or(0)
        });
        if pe_surface_contract && !self.can_afford_build_cost(pe_stroke_cost) {
            return 0;
        }
        if pe_surface_contract {
            // FUN_00448E20 precedes each kind-specific coordinate writer. Run
            // it over the submitted PE stroke before placement gates and early
            // returns; the native helper also runs when that writer is a no-op.
            self.debit_build_cost(pe_stroke_cost);
            if !pe_power_coordinate_contract
                && !pe_road_coordinate_contract
                && !pe_rail_coordinate_contract
            {
                for (index, &(x, y)) in stroke_path.iter().enumerate() {
                    pe_network_surface_edit_448e20(
                        &mut self.city,
                        x,
                        y,
                        pe_pipe_direction_for_path_tile(&stroke_path, index),
                    );
                }
            }
        }

        let mut commit_path = self.network_commit_path(path, kind);
        if selected_tool
            && !pe_surface_contract
            && let Some(per_tile) = kind.cost_per_tile()
        {
            commit_path.truncate(self.affordable_build_units(per_tile));
        }
        if commit_path.is_empty() && !pe_surface_contract {
            return 0;
        }

        // PE PlacePowerLines AtCoordinates @ `0x44a8d3`–`0x44a8d7`: already XBIT
        // powerable `0x80` → return before PlaceTile / OR / reshape.
        // PE @ `0x44a900`–`0x44a90f` + table `0x44a9e8`: XBLD `>= 0x0e` only
        // proceeds for the six crossover sources; all other `>= 0x0e` (incl.
        // in-window slot-6 / outside window) → return.
        if kind == NetworkKind::Power {
            commit_path.retain(|&(x, y)| {
                if self
                    .city
                    .tile_bits_at(x, y)
                    .is_some_and(TileBits::powerable)
                {
                    return false;
                }
                let b = self.city.building_at(x, y);
                b < 0x0e || pe_place_power_crossover_from_existing_44a9e8(b).is_some()
            });
            if commit_path.is_empty() && !pe_surface_contract {
                return 0;
            }
        }
        // PE PlaceRoads AtCoordinates @ `0x44acf4`–`0x44ad28`: XBLD already in
        // flat `0x1d..0x2c`, elevated `0x3f..0x47`, road/rail cross
        // `0x4b`/`0x4c`, or highway `0x5d..0x61` → return before PlaceTile /
        // XZON clear / reshape.
        // PE @ `0x44ad2e`–`0x44ad3d` + table `0x44ae24`: else only `XBLD < 0x0e`
        // or a crossover-table hit proceeds; other tiles → return.
        if kind == NetworkKind::Road && !pe_road_coordinate_contract {
            commit_path.retain(|&(x, y)| {
                let b = self.city.building_at(x, y);
                if (0x1d..0x2c).contains(&b)
                    || (0x3f..0x47).contains(&b)
                    || matches!(b, 0x4b | 0x4c)
                    || (0x5d..0x61).contains(&b)
                {
                    return false;
                }
                b < 0x0e || pe_place_roads_crossover_from_existing_44ae24(b).is_some()
            });
            if commit_path.is_empty() && !pe_surface_contract {
                return 0;
            }
        }
        // PE PlaceRails AtCoordinates @ `0x44b1ca`–`0x44b208`: XBLD already in
        // `0x2c..0x3f`, elevated `0x45..0x49`, tunnel `0x6c..0x70`, or
        // rail/road cross `0x4d`/`0x4e` → return before PlaceTile / XZON /
        // reshape.
        // PE @ `0x44b20e`–`0x44b21d` + table `0x44b2f4`: else only `XBLD < 0x0e`
        // or a crossover-table hit proceeds; other tiles → return.
        if kind == NetworkKind::Rail {
            commit_path.retain(|&(x, y)| {
                let b = self.city.building_at(x, y);
                if (0x2c..0x3f).contains(&b)
                    || (0x45..0x49).contains(&b)
                    || (0x6c..0x70).contains(&b)
                    || matches!(b, 0x4d | 0x4e)
                {
                    return false;
                }
                b < 0x0e || pe_place_rails_crossover_from_existing_44b2f4(b).is_some()
            });
            if commit_path.is_empty() && !pe_surface_contract {
                return 0;
            }
        }

        let build_cost = kind.cost_per_tile().map_or(0, |per_tile| {
            if pe_surface_contract {
                pe_stroke_cost
            } else {
                per_tile.saturating_mul(commit_path.len() as u32)
            }
        });

        let mut applied_path = commit_path.clone();
        if pe_power_coordinate_contract
            || pe_road_coordinate_contract
            || pe_rail_coordinate_contract
        {
            // FUN_0044A310 / FUN_00449BB0 / FUN_00449F50 run the surface
            // helper and their kind-specific writer per coordinate. All three
            // writers return before reshaping when their admission gate
            // refuses; accepted writes reshape before the next point.
            // Preserve each tool's cadence instead of a bulk HashSet refresh.
            let commit_tiles: HashSet<_> = commit_path.iter().copied().collect();
            if pe_road_coordinate_contract || pe_rail_coordinate_contract {
                applied_path.clear();
            }
            for (index, &(x, y)) in stroke_path.iter().enumerate() {
                pe_network_surface_edit_448e20(
                    &mut self.city,
                    x,
                    y,
                    pe_pipe_direction_for_path_tile(&stroke_path, index),
                );

                let mut network_tile_was_written = false;
                if commit_tiles.contains(&(x, y)) {
                    let existing = self.city.building_at(x, y);
                    if pe_power_coordinate_contract {
                        if crossover_network_axis(existing, kind).is_none() {
                            let building = pe_place_network_crossover_from_existing(kind, existing)
                                .unwrap_or_else(|| kind.base_tile());
                            pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, kind);
                            // PE PlaceTile (`0x441f00` / `0x40178f`): leave watered/piped/XTRF.
                            pe_place_tile_441f00(&mut self.city, x, y, building);
                            // PE PlacePowerLines @ `0x44a968`: after PlaceTile, XBIT |= 0x80.
                            stamp_xbit_powerable(&mut self.city, x, y);
                            network_tile_was_written = true;
                        }
                    } else if pe_road_coordinate_contract
                        && pe_place_roads_coordinate_should_stamp_44acf4(existing)
                        && crossover_network_axis(existing, kind).is_none()
                    {
                        let building = pe_place_roads_crossover_from_existing_44ae24(existing)
                            .unwrap_or_else(|| kind.base_tile());
                        // FUN_0044ACC0 writes the base/table tile before XZON and reshape.
                        pe_place_tile_441f00(&mut self.city, x, y, building);
                        pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, kind);
                        applied_path.push((x, y));
                        network_tile_was_written = true;
                    } else if pe_rail_coordinate_contract
                        && pe_place_rails_coordinate_should_stamp_44b1a0(existing)
                        && crossover_network_axis(existing, kind).is_none()
                    {
                        let building = pe_place_rails_crossover_from_existing_44b2f4(existing)
                            .unwrap_or_else(|| kind.base_tile());
                        pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, kind);
                        pe_place_tile_441f00(&mut self.city, x, y, building);
                        applied_path.push((x, y));
                        network_tile_was_written = true;
                    }
                }

                if pe_power_coordinate_contract && network_tile_was_written {
                    self.refresh_power_neighbors_like_pe_44a8a0(x, y);
                } else if pe_road_coordinate_contract && network_tile_was_written {
                    self.refresh_road_neighbors_like_pe_44acc0(x, y);
                } else if pe_rail_coordinate_contract && network_tile_was_written {
                    self.refresh_rail_neighbors_like_pe_44b1a0(x, y);
                }
            }
        } else {
            let mut rail_bridge_run_index = 0usize;
            for (index, &(x, y)) in commit_path.iter().enumerate() {
                let existing = self.city.building_at(x, y);
                let side_mask = network_path_side_mask(&commit_path, index);
                let terrain = self.city.terrain_at(x, y);
                let is_rail_bridge = kind == NetworkKind::Rail
                    && network_bridge_building_for_tile(kind, side_mask, terrain).is_some();
                if kind == NetworkKind::Rail && !is_rail_bridge {
                    rail_bridge_run_index = 0;
                }
                if crossover_network_axis(self.city.building_at(x, y), kind).is_some() {
                    continue;
                }
                if let Some(crossover) = pe_place_network_crossover_from_existing(kind, existing)
                    .or_else(|| {
                        network_crossing_building_for_path_tile(
                            kind,
                            network_axis_mask_for_side_mask(side_mask),
                            existing,
                        )
                    })
                {
                    pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, kind);
                    // PE PlaceTile (`0x441f00` / `0x40178f`): leave watered/piped/XTRF.
                    pe_place_tile_441f00(&mut self.city, x, y, crossover);
                    if kind == NetworkKind::Power {
                        // PE PlacePowerLines @ `0x44a968`: after PlaceTile, `XBIT |= 0x80`.
                        stamp_xbit_powerable(&mut self.city, x, y);
                    }
                    continue;
                }
                let building = if is_rail_bridge {
                    let building = self.rail_bridge_building_for_tile(
                        x,
                        y,
                        Some(rail_bridge_building_for_run_index(rail_bridge_run_index)),
                    );
                    rail_bridge_run_index += 1;
                    building
                } else {
                    network_building_for_tile(kind, side_mask, terrain)
                };
                pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, kind);
                // PE PlaceTile (`0x441f00` / `0x40178f`): leave watered/piped/XTRF.
                pe_place_tile_441f00(&mut self.city, x, y, building);
                if kind == NetworkKind::Power {
                    stamp_xbit_powerable(&mut self.city, x, y);
                }
            }
        }
        self.clear_zone_growth_for_player_tiles(&applied_path);

        if !pe_power_coordinate_contract
            && !pe_road_coordinate_contract
            && !pe_rail_coordinate_contract
        {
            let mut refresh_tiles = HashSet::new();
            for &(x, y) in &commit_path {
                refresh_tiles.insert((x, y));
                for neighbor in neighbor_tiles(x, y) {
                    refresh_tiles.insert(neighbor);
                }
            }
            for (x, y) in refresh_tiles {
                self.refresh_network_tile(x, y, kind);
            }
        }

        // PE power-line tool group @ `0x43f396`–`0x43f3aa`: subtype 0 success →
        // pop `< 0xc350` → `FUN_00467860` (Road/Rail skip).
        if kind == NetworkKind::Power {
            pe_powerline_post_place_power_recalc_43f396(&mut self.city, &mut self.pe_process_crt);
        }

        self.pending_traffic_redraw = true;
        // PE PlaceTile path: no invent citywide water-network refresh.
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        if kind.cost_per_tile().is_some() && !pe_surface_contract {
            self.debit_build_cost(build_cost);
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
        applied_path.len()
    }

    pub(in crate::app) fn network_commit_path(
        &self,
        path: &[(usize, usize)],
        kind: NetworkKind,
    ) -> Vec<(usize, usize)> {
        if kind == NetworkKind::Power {
            return self.network_buildable_prefix_path(path, kind).0;
        }

        let placement = self.network_placement_for_path(path, kind);
        if placement.valid_tiles.is_empty() || !placement.invalid_tiles.is_empty() {
            Vec::new()
        } else {
            path.to_vec()
        }
    }

    pub(in crate::app) fn can_place_network_path_tile(
        &self,
        path: &[(usize, usize)],
        index: usize,
        kind: NetworkKind,
    ) -> bool {
        let Some(&(x, y)) = path.get(index) else {
            return false;
        };
        let terrain = self.city.terrain_at(x, y);
        let terrain = if self.engine_fidelity.uses_win95()
            && network_kind_for_tool(self.tool) == Some(kind)
        {
            pe_network_surface_terrain_result_448e20(
                terrain,
                pe_pipe_direction_for_path_tile(path, index),
            )
            .map_or(terrain, |(mapped, _)| mapped)
        } else {
            terrain
        };
        let side_mask = network_path_side_mask(path, index);
        let axis_mask = network_axis_mask_for_side_mask(side_mask);
        if is_water_terrain(terrain) {
            return network_bridge_building_for_tile(kind, side_mask, terrain).is_some()
                && can_place_network_bridge_on_building(self.city.building_at(x, y), kind);
        }
        if !terrain_allows_network_axis(terrain, axis_mask) {
            return false;
        }
        let building = self.city.building_at(x, y);
        let can_place = building == 0
            || is_tree_building(building)
            || kind.contains(building)
            || crossover_network_axis(building, kind).is_some();
        if !can_place {
            return pe_place_network_crossover_from_existing(kind, building).is_some()
                || network_crossing_building_for_path_tile(kind, axis_mask, building).is_some();
        }
        let Some(crossover_axis) = crossover_network_axis(building, kind) else {
            return true;
        };
        axis_mask != 0 && axis_mask == network_axis_mask(crossover_axis)
    }
}
