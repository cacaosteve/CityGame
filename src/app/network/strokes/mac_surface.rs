use super::super::bridges::mac_water_bridge_origin_direction;
use super::*;

struct MacSurfaceWalk {
    kind: NetworkKind,
    target: (usize, usize),
    bounds: (usize, usize, usize, usize),
    direction: u8,
}

fn military_zone(city: &City, (x, y): (usize, usize)) -> bool {
    city.zone_at(x, y)
        .is_some_and(|zone| zone.zone_id() == ZONE_MILITARY)
}

// CODE 13's A5-0x49c6 table shares the Win95 rows for the four ordinary
// slopes and also gates directions 1/3 for classes 14 and 15.
fn xter_step_gate(xter: u8, direction: u8) -> bool {
    xter < 0x40
        && match xter & 0x0f {
            1 | 3 => direction & 1 == 0,
            2 | 4 | 14 | 15 => direction & 1 == 1,
            _ => false,
        }
}

fn class_height_delta(xter: u8) -> i16 {
    i16::from(matches!(xter & 0x0f, 5..=8 | 13))
}

impl MacSurfaceWalk {
    // CODE 13 @ 0x22a06.
    fn new(city: &City, drag: NetworkDrag) -> Option<Self> {
        let NetworkDrag {
            start,
            current: end,
            kind,
        } = drag;
        if start.0 >= CITY_WIDTH
            || start.1 >= CITY_HEIGHT
            || end.0 >= CITY_WIDTH
            || end.1 >= CITY_HEIGHT
        {
            return None;
        }
        let xter = city.terrain_at(start.0, start.1);
        let building = city.building_at(start.0, start.1);
        if building >= 0x51
            || matches!(building, 5 | 0x0d)
            || (0x10..0x20).contains(&xter)
            || military_zone(city, start)
        {
            return None;
        }

        let mut target = end;
        let mut direction = pe_pipe_direction_to_target_44c680(start, end);
        if (1..=4).contains(&xter) && xter_step_gate(xter, direction) {
            direction = pe_pipe_alternate_direction_44c6f0(start, end);
            if xter_step_gate(xter, direction) {
                target = start;
            }
        }
        Some(Self {
            kind,
            target,
            bounds: (
                start.0.min(end.0),
                start.1.min(end.1),
                start.0.max(end.0),
                start.1.max(end.1),
            ),
            direction,
        })
    }

    // CODE 13 @ 0x22ca6. Water terminates this iterator; the caller probes
    // the last coordinate and direction again for its bridge handoff.
    fn advance(&mut self, city: &City, current: (usize, usize)) -> Option<(usize, usize)> {
        if current == self.target {
            return None;
        }
        let xter = city.terrain_at(current.0, current.1);
        let building = city.building_at(current.0, current.1);
        let keeps_direction = pe_road_xbld_keeps_saved_direction_448f50(building, self.direction)
            || PE_NETWORK_SURFACE_REMAP_GATE_4E7B68[usize::from(xter & 0x0f)] != 0;
        let direction = if keeps_direction {
            self.direction
        } else {
            pe_pipe_direction_to_target_44c680(current, self.target)
        };
        let next = pe_pipe_step_coordinate_44f50(current, direction)?;
        if !pe_pipe_inside_stroke_bounds_44f50(next, self.bounds) {
            return None;
        }

        let next_xter = city.terrain_at(next.0, next.1);
        if keeps_direction {
            if xter_step_gate(next_xter, direction)
                || (self.kind == NetworkKind::Rail
                    && next_xter != 0
                    && city.altitude_at(next.0, next.1) & 0x1f
                        != city.altitude_at(current.0, current.1) & 0x1f)
            {
                return None;
            }
        } else {
            let height =
                i16::from(city.altitude_at(current.0, current.1) & 0x1f) + class_height_delta(xter);
            let ambiguity_gate = |(x, y): (usize, usize), direction| {
                let xter = city.terrain_at(x, y);
                let delta = i16::from(city.altitude_at(x, y) & 0x1f) - height;
                xter_step_gate(xter, direction)
                    || (class_height_delta(xter) != 0 && matches!(delta, 0 | -2))
                    || (xter == 0 && delta == -1)
            };
            if ambiguity_gate(next, direction) {
                let alternate = pe_pipe_alternate_direction_44c6f0(current, self.target);
                if pe_pipe_step_coordinate_44f50(current, alternate)
                    .is_some_and(|tile| ambiguity_gate(tile, alternate))
                {
                    return None;
                }
            }
            if self.kind == NetworkKind::Rail && next_xter != 0 && direction != self.direction {
                return None;
            }
            self.direction = direction;
        }

        if military_zone(city, next) || pe_road_start_is_surface_water_449bb0(city, next) {
            return None;
        }
        pe_road_candidate_xbld_allows_step_448f50(city.building_at(next.0, next.1), direction)
            .then_some(next)
    }
}

impl ViewerState {
    pub(super) fn uses_mac_surface_tool(&self, kind: NetworkKind) -> bool {
        self.engine_fidelity == EngineFidelity::Mac68k
            && network_kind_for_tool(self.tool) == Some(kind)
    }

    pub(super) fn mac_surface_preview(&self, drag: NetworkDrag) -> Vec<(usize, usize)> {
        if pe_road_start_is_surface_water_449bb0(&self.city, drag.start) {
            return Vec::new();
        }
        let Some(mut walk) = MacSurfaceWalk::new(&self.city, drag) else {
            return Vec::new();
        };
        let mut current = drag.start;
        let mut path = vec![current];
        while let Some(next) = walk.advance(&self.city, current) {
            path.push(next);
            current = next;
        }
        path
    }

    pub(super) fn place_mac_surface_stroke(
        &mut self,
        drag: NetworkDrag,
    ) -> (
        usize,
        Option<RailNeighborLinkCandidate>,
        Option<RoadNeighborLinkCandidate>,
    ) {
        if pe_road_start_is_surface_water_449bb0(&self.city, drag.start) {
            if let Some(direction) = mac_water_bridge_origin_direction(&self.city, drag.start) {
                match drag.kind {
                    NetworkKind::Road => {
                        self.begin_mac_road_bridge(drag.start, direction, None);
                    }
                    NetworkKind::Rail => {
                        self.begin_mac_rail_bridge(drag.start, direction, None);
                    }
                    NetworkKind::Power => {
                        self.begin_power_bridge(drag.start, direction, false);
                    }
                }
            }
            return (0, None, None);
        }
        let path = self.mac_surface_preview(drag);
        let cost = pe_network_surface_stroke_cost_448a90(
            &self.city,
            &path,
            drag.kind.cost_per_tile().unwrap(),
        );
        if cost == 0 || !self.can_afford_build_cost(cost) {
            return (0, None, None);
        }
        let Some(mut walk) = MacSurfaceWalk::new(&self.city, drag) else {
            return (0, None, None);
        };

        // CODE 13 debits the preview total before rewalking the live map.
        self.debit_build_cost(cost);
        let mut current = drag.start;
        let mut written = Vec::new();
        loop {
            let next = walk.advance(&self.city, current);
            let (x, y) = current;
            // CODE 13 @ 0x22b9e shares the decoded terrain remap and water
            // flag clear with the Win95 worker.
            pe_network_surface_edit_448e20(&mut self.city, x, y, walk.direction);
            let placed = match drag.kind {
                NetworkKind::Road => self.place_mac_road_coordinate(x, y),
                NetworkKind::Rail => self.place_mac_rail_coordinate(x, y),
                NetworkKind::Power => self.place_power_line_coordinate((x, y)),
            };
            if placed {
                written.push(current);
            }
            let Some(next) = next else { break };
            current = next;
        }
        // CODE 13 @ 0x23436 offers the separate paid link on an off-map
        // lookahead or an edge stroke that stayed at its origin.
        let lookahead = pe_pipe_step_coordinate_44f50(current, walk.direction);
        let link_endpoint = (lookahead.is_none()
            || (current == drag.start && pe_network_route_origin_touches_outer_edge(current)))
        .then_some(current);

        self.clear_zone_growth_for_player_tiles(&written);
        self.pending_traffic_redraw = true;
        self.query_is_inspect = false;
        // The road tool updates cash and tiles here. The calendar owns the
        // next road-upkeep budget snapshot.
        self.refresh_status_line();
        let water_seed =
            lookahead.filter(|&seed| pe_road_start_is_surface_water_449bb0(&self.city, seed));
        if drag.kind == NetworkKind::Power {
            if !water_seed.is_some_and(|seed| self.begin_power_bridge(seed, walk.direction, true)) {
                self.mac_power_tool_post_place_recalc();
            }
            return (written.len(), None, None);
        }
        if drag.kind == NetworkKind::Rail {
            let link = link_endpoint.map(|endpoint| RailNeighborLinkCandidate { endpoint });
            if let Some(seed) = water_seed
                && self.begin_mac_rail_bridge(seed, walk.direction, link)
            {
                return (written.len(), None, None);
            }
            return (
                written.len(),
                link.filter(|&candidate| rail_neighbor_link_is_available(self, candidate)),
                None,
            );
        }

        let link = link_endpoint.map(|endpoint| RoadNeighborLinkCandidate { endpoint });
        if let Some(seed) = water_seed
            && self.begin_mac_road_bridge(seed, walk.direction, link)
        {
            return (written.len(), None, None);
        }
        (
            written.len(),
            None,
            link.filter(|&candidate| road_neighbor_link_is_available(self, candidate)),
        )
    }

    pub(in crate::app::network) fn place_mac_rail_coordinate(
        &mut self,
        x: usize,
        y: usize,
    ) -> bool {
        let existing = self.city.building_at(x, y);
        if !pe_place_rails_coordinate_should_stamp_44b1a0(existing) {
            return false;
        }
        let building = pe_place_rails_crossover_from_existing_44b2f4(existing)
            .unwrap_or_else(|| NetworkKind::Rail.base_tile());
        pe_place_tile_441f00(&mut self.city, x, y, building);
        pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, NetworkKind::Rail);
        for (rx, ry) in std::iter::once((x, y)).chain(neighbor_tiles(x, y)) {
            self.refresh_mac_rail_tile(rx, ry);
        }
        true
    }

    pub(in crate::app::network) fn place_mac_road_coordinate(
        &mut self,
        x: usize,
        y: usize,
    ) -> bool {
        let existing = self.city.building_at(x, y);
        if !pe_place_roads_coordinate_should_stamp_44acf4(existing) {
            return false;
        }
        let building = pe_place_roads_crossover_from_existing_44ae24(existing)
            .unwrap_or_else(|| NetworkKind::Road.base_tile());
        pe_place_tile_441f00(&mut self.city, x, y, building);
        pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, NetworkKind::Road);
        for (rx, ry) in std::iter::once((x, y)).chain(neighbor_tiles(x, y)) {
            self.refresh_mac_road_tile(rx, ry);
        }
        true
    }
}
