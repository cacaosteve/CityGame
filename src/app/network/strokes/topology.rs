use super::*;

impl ViewerState {
    pub(in crate::app) fn refresh_network_tile(&mut self, x: usize, y: usize, kind: NetworkKind) {
        self.reshape_network_tile(x, y, kind);
        persist_pe_neighbor_connection_overlay(&mut self.city, x, y);
    }

    pub(in crate::app) fn refresh_mac_road_tile(&mut self, x: usize, y: usize) {
        // CODE 13 @ 0x23f58 only reshapes the ordinary road family.
        if !(0x1d..=0x2b).contains(&self.city.building_at(x, y)) {
            return;
        }
        if self.city.is_synthetic_trip_marker(x, y) {
            self.city.set_text_overlay(x, y, 0);
        }
        if let Some(offset) = mac_connector_slope_offset(self.city.terrain_at(x, y)) {
            self.place_mac_connector_tile(x, y, 0x1d + offset);
            return;
        }

        let xtxt = self.city.text_overlay_at(x, y).unwrap_or(0);
        let mut mask = pe_network_xtxt_fa_edge_mask_bits_44b0ac(x, y, xtxt);
        for (side, nx, ny) in directional_neighbor_tiles(x, y) {
            if matches!(self.city.building_at(nx, ny), 0x1d..=0x2b | 0x3f..=0x46 | 0x4b | 0x4c | 0x5d..=0x60)
                && mac_connector_terrain_open(self.city.terrain_at(nx, ny), nx == x)
            {
                mask |= side;
            }
        }
        pe_place_tile_441f00(&mut self.city, x, y, pe_place_road_mask_tile_44b0e5(mask));
    }

    pub(in crate::app::network) fn refresh_mac_rail_tile(&mut self, x: usize, y: usize) {
        // CODE 13 @ 0x24498 reshapes only the ordinary rail band.
        if !(0x2c..=0x3a).contains(&self.city.building_at(x, y)) {
            return;
        }
        if self.city.is_synthetic_trip_marker(x, y) {
            self.city.set_text_overlay(x, y, 0);
        }

        if let Some(offset) = mac_connector_slope_offset(self.city.terrain_at(x, y)) {
            self.place_mac_connector_tile(x, y, 0x2c + offset);
            return;
        }

        let xtxt = self.city.text_overlay_at(x, y).unwrap_or(0);
        let mut mask = pe_network_xtxt_fa_edge_mask_bits_44b0ac(x, y, xtxt);
        // Native neighbor order and slope IDs use map coordinates.
        let probes = [
            (-1, 0, NETWORK_LEFT, 0x2e, 0x3b),
            (1, 0, NETWORK_RIGHT, 0x30, 0x3d),
            (0, -1, NETWORK_TOP, 0x2f, 0x3c),
            (0, 1, NETWORK_BOTTOM, 0x31, 0x3e),
        ];
        for (dx, dy, side, slope, cap) in probes {
            let Some((nx, ny)) = super::super::super::offset_city_tile(x, y, dx, dy) else {
                continue;
            };
            let building = self.city.building_at(nx, ny);
            if building == slope {
                pe_place_tile_441f00(&mut self.city, x, y, cap);
                return;
            }
            let terrain = self.city.terrain_at(nx, ny);
            let open = terrain < 0x44 && mac_connector_terrain_open(terrain, dx == 0);
            if open && matches!(building, 0x2c..=0x3e | 0x45..=0x48 | 0x6c..=0x6f | 0x4d | 0x4e) {
                mask |= side;
            }
        }
        pe_place_tile_441f00(&mut self.city, x, y, pe_place_rail_mask_tile_44b6a0(mask));
    }

    pub(super) fn refresh_mac_power_tile(&mut self, x: usize, y: usize) {
        // CODE 13 @ 0x23c36. The XBIT flag and neighboring terrain gate
        // connections on each axis. The existing wire artwork is irrelevant.
        if !(0x0e..=0x1c).contains(&self.city.building_at(x, y)) {
            return;
        }
        let tile = if let Some(offset) = mac_connector_slope_offset(self.city.terrain_at(x, y)) {
            0x0e + offset
        } else {
            let mut mask = 0;
            for (side, nx, ny) in directional_neighbor_tiles(x, y) {
                if self
                    .city
                    .tile_bits_at(nx, ny)
                    .is_some_and(TileBits::powerable)
                    && mac_connector_terrain_open(self.city.terrain_at(nx, ny), nx == x)
                {
                    mask |= side;
                }
            }
            u16::from(pe_place_power_mask_tile_44ac2c(mask))
        };
        self.place_mac_connector_tile(x, y, tile);
    }

    fn place_mac_connector_tile(&mut self, x: usize, y: usize, tile: u16) {
        if let Ok(tile) = u8::try_from(tile) {
            pe_place_tile_441f00(&mut self.city, x, y, tile);
        } else {
            self.city.mac_connector_place_tile(x, y, tile);
        }
    }

    fn reshape_network_tile(&mut self, x: usize, y: usize, kind: NetworkKind) {
        // Network reshape updates XBLD and leaves watered/piped/XTRF intact.
        let existing = self.city.building_at(x, y);
        let terrain = self.city.terrain_at(x, y);
        if self.engine_fidelity.uses_win95()
            && kind == NetworkKind::Road
            && (0x51..=0x58).contains(&existing)
            && self
                .city
                .tile_bits_at(x, y)
                .is_some_and(TileBits::water_tile)
        {
            // Win95's bridge worker fixes these interior family tiles before
            // endpoint refresh; XTER can be 0x20..0x2f, outside the generic
            // `is_water_terrain` set, so keep them before ordinary road masks.
            return;
        }
        if self.engine_fidelity.uses_win95() && kind == NetworkKind::Power && existing == 0x5c {
            // Win95's PlacePowerLines reshape arm @ `0x44aa80` only accepts
            // XBLD `0x0e..=0x1c`; the shared water-span worker's `0x5c`
            // interior is not a reshape candidate. Endpoint refresh must not
            // turn the bridge art back into an ordinary `0x0e..=0x1c` wire.
            return;
        }
        // Mac CODE 13's selector-0 surface workers take the gated XTER slope
        // table before building a neighbor mask. Preserve that ordering for
        // the directly decoded 1..=4 slope classes; PE keeps its mask/axis
        // behavior below.
        let in_mac_surface_network_band = match kind {
            NetworkKind::Power => (0x0e..=0x1c).contains(&existing),
            NetworkKind::Road => (0x1d..=0x2b).contains(&existing),
            NetworkKind::Rail => (0x2c..=0x3a).contains(&existing),
        };
        if !self.engine_fidelity.uses_win95()
            && in_mac_surface_network_band
            && let Some(building) = mac_network_slope_building_for_terrain(kind, terrain)
        {
            if existing != building {
                pe_place_tile_441f00(&mut self.city, x, y, building);
            }
            return;
        }
        if is_water_terrain(terrain) {
            if kind.contains(existing)
                && let Some(mut building) = network_bridge_building_for_tile(
                    kind,
                    self.network_neighbor_mask(x, y, kind),
                    terrain,
                )
            {
                if kind == NetworkKind::Rail {
                    building = self.rail_bridge_building_for_tile(x, y, None);
                }
                if existing != building {
                    // PE PlaceTile (`0x441f00` / `0x40178f`): leave watered/piped/XTRF.
                    pe_place_tile_441f00(&mut self.city, x, y, building);
                }
            }
            return;
        }
        if kind == NetworkKind::Rail
            && let Some(current_side) = sub_to_rail_surface_side(existing)
        {
            if let Some(side) = self.adjacent_rail_side(x, y, Some(current_side)) {
                // PE PlaceTile (`0x441f00` / `0x40178f`): leave watered/piped/XTRF.
                pe_place_tile_441f00(
                    &mut self.city,
                    x,
                    y,
                    sub_to_rail_building_for_side(side).expect("cardinal rail side"),
                );
            }
            return;
        }
        // PE PlacePowerLines reshape `0x44aa80`: XTXT gate early-out, else mask table+0x0e.
        if kind == NetworkKind::Power && (0x0e..0x1d).contains(&existing) {
            if pe_place_power_xtxt_reshape_44aafb(&mut self.city, x, y) {
                return;
            }
            let mask = self.network_neighbor_mask(x, y, kind);
            // PE FUN_0044AA80 does not read XTER: after its XTXT gate it always
            // selects `0x4e7c40[mask] + 0x0e`. Keep the terrain-derived slope
            // variant for generic Mac map edits. The selected Mac Power tool
            // uses its own connector above.
            let building = if !self.engine_fidelity.uses_win95()
                && network_slope_building_for_terrain(kind, mask, terrain).is_some()
            {
                network_building_for_tile(kind, mask, terrain)
            } else {
                pe_place_power_mask_tile_44ac2c(mask)
            };
            if existing != building {
                pe_place_tile_441f00(&mut self.city, x, y, building);
            }
            return;
        }
        // Generic road refresh for callers outside the native tool stroke.
        if kind == NetworkKind::Road && (0x1d..0x2c).contains(&existing) {
            if pe_place_road_xtxt_reshape_44af50(&mut self.city, x, y) {
                return;
            }
            let xtxt = self.city.text_overlay_at(x, y).unwrap_or(0);
            let mask = self.network_neighbor_mask(x, y, kind)
                | pe_network_xtxt_fa_edge_mask_bits_44b0ac(x, y, xtxt);
            // Flat mask+0x1d would smash terrain-slope road art `0x1f..0x22` (same need as
            // PlaceRails slope early-out @ `0x44b467` before its mask table).
            let building = if network_slope_building_for_terrain(kind, mask, terrain).is_some() {
                network_building_for_tile(kind, mask, terrain)
            } else {
                pe_place_road_mask_tile_44b0e5(mask)
            };
            if existing != building {
                pe_place_tile_441f00(&mut self.city, x, y, building);
            }
            return;
        }
        // Generic rail refresh for callers outside the native tool stroke.
        if kind == NetworkKind::Rail
            && (0x2c..0x3b).contains(&existing)
            && sub_to_rail_surface_side(existing).is_none()
        {
            if pe_place_rail_xtxt_reshape_44b41d(&mut self.city, x, y) {
                return;
            }
            if let Some(slope) = pe_place_rail_slope_from_cardinal_neighbor_44b467(&self.city, x, y)
            {
                if existing != slope {
                    pe_place_tile_441f00(&mut self.city, x, y, slope);
                }
                return;
            }
            let xtxt = self.city.text_overlay_at(x, y).unwrap_or(0);
            let mask = self.network_neighbor_mask(x, y, kind)
                | pe_network_xtxt_fa_edge_mask_bits_44b0ac(x, y, xtxt);
            // Flat mask+0x2c would smash terrain-slope rail art (same as PlaceRoads).
            let building = if network_slope_building_for_terrain(kind, mask, terrain).is_some() {
                network_building_for_tile(kind, mask, terrain)
            } else {
                pe_place_rail_mask_tile_44b6a0(mask)
            };
            if existing != building {
                pe_place_tile_441f00(&mut self.city, x, y, building);
            }
            return;
        }
        // PE PlaceRoads/PlaceRails tool crossovers / table underlays are not mask-reshaped.
        // Road.contains includes highways for neighbor connectivity only — do not reshape them.
        if crossover_network_axis(existing, kind).is_some()
            || pe_place_network_crossover_from_existing(kind, existing).is_some()
            || (kind == NetworkKind::Road && matches!(existing, 0x49..=0x50))
        {
            return;
        }
        if !kind.contains(existing) {
            return;
        }
        let building = network_building_for_tile(
            kind,
            self.network_neighbor_mask(x, y, kind),
            self.city.terrain_at(x, y),
        );
        if self.city.building_at(x, y) != building {
            // PE PlaceTile (`0x441f00` / `0x40178f`): leave watered/piped/XTRF.
            pe_place_tile_441f00(&mut self.city, x, y, building);
        }
    }

    pub(in crate::app) fn adjacent_rail_side(
        &self,
        x: usize,
        y: usize,
        preferred: Option<u8>,
    ) -> Option<u8> {
        let connects = |side, nx, ny| {
            network_building_connects_to(
                self.city.building_at(nx, ny),
                NetworkKind::Rail,
                opposite_network_side(side),
            )
        };

        if let Some(preferred) = preferred
            && directional_neighbor_tiles(x, y)
                .any(|(side, nx, ny)| side == preferred && connects(side, nx, ny))
        {
            return Some(preferred);
        }

        directional_neighbor_tiles(x, y)
            .find(|&(side, nx, ny)| connects(side, nx, ny))
            .map(|(side, _, _)| side)
    }

    pub(in crate::app) fn rail_bridge_building_for_tile(
        &self,
        x: usize,
        y: usize,
        preferred: Option<u8>,
    ) -> u8 {
        if let Some(neighbor) = self.rail_bridge_neighbor_building(x, y) {
            return opposite_rail_bridge_building(neighbor);
        }
        let existing = self.city.building_at(x, y);
        if matches!(existing, 90 | 91) {
            return existing;
        }
        preferred.unwrap_or(91)
    }

    pub(in crate::app) fn rail_bridge_neighbor_building(&self, x: usize, y: usize) -> Option<u8> {
        for (_, nx, ny) in directional_neighbor_tiles(x, y) {
            if !is_water_terrain(self.city.terrain_at(nx, ny)) {
                continue;
            }
            let building = self.city.building_at(nx, ny);
            if matches!(building, 90 | 91) {
                return Some(building);
            }
        }
        None
    }

    pub(in crate::app) fn network_neighbor_mask(
        &self,
        x: usize,
        y: usize,
        kind: NetworkKind,
    ) -> u8 {
        self.network_neighbor_mask_with_virtual(x, y, kind, &HashSet::new())
    }

    #[cfg(test)]
    pub(in crate::app) fn network_reshape_neighbor_mask(
        &self,
        x: usize,
        y: usize,
        kind: NetworkKind,
        pe_road_neighbor_gate: bool,
        pe_rail_neighbor_gate: bool,
    ) -> u8 {
        if (kind == NetworkKind::Road && !pe_road_neighbor_gate)
            || (kind == NetworkKind::Rail && !pe_rail_neighbor_gate)
            || kind == NetworkKind::Power
        {
            return self.network_neighbor_mask(x, y, kind);
        }

        let pe_gate = |building, side| match kind {
            NetworkKind::Road => pe_place_road_reshape_neighbor_connects_44af6e(building, side),
            NetworkKind::Rail => pe_place_rail_reshape_neighbor_connects_44b485(building, side),
            NetworkKind::Power => unreachable!(),
        };
        let mut mask = 0;
        if y > 0 && pe_gate(self.city.building_at(x, y - 1), NETWORK_BOTTOM) {
            mask |= NETWORK_TOP;
        }
        if x + 1 < CITY_WIDTH && pe_gate(self.city.building_at(x + 1, y), NETWORK_LEFT) {
            mask |= NETWORK_RIGHT;
        }
        if y + 1 < CITY_HEIGHT && pe_gate(self.city.building_at(x, y + 1), NETWORK_TOP) {
            mask |= NETWORK_BOTTOM;
        }
        if x > 0 && pe_gate(self.city.building_at(x - 1, y), NETWORK_RIGHT) {
            mask |= NETWORK_LEFT;
        }
        mask
    }

    /// PE `FUN_0044A8A0` reshapes the committed power tile, then its x−1, x+1,
    /// y−1 and y+1 neighbors (`0x44a96e..0x44a9bd`).
    pub(in crate::app) fn refresh_power_neighbors_like_pe_44a8a0(&mut self, x: usize, y: usize) {
        for (nx, ny) in std::iter::once((x, y)).chain(neighbor_tiles(x, y)) {
            if (0x0e..=0x1c).contains(&self.city.building_at(nx, ny)) {
                let tile = crate::app::network::pe_reshape::pe_surface_neighbor_shape(
                    &self.city,
                    nx,
                    ny,
                    NetworkKind::Power,
                );
                pe_place_tile_441f00(&mut self.city, nx, ny, tile);
            }
        }
    }

    /// PE `FUN_0044ACC0` reshapes center, left, right, up, down
    /// (`0x44adac..0x44adfb`) before the stroke advances to its next point.
    pub(in crate::app) fn refresh_road_neighbors_like_pe_44acc0(&mut self, x: usize, y: usize) {
        for (nx, ny) in std::iter::once((x, y)).chain(neighbor_tiles(x, y)) {
            self.refresh_pe_road_tile_44aed0(nx, ny);
            persist_pe_neighbor_connection_overlay(&mut self.city, nx, ny);
        }
    }

    pub(super) fn refresh_pe_road_tile_44aed0(&mut self, x: usize, y: usize) {
        if (0x1d..=0x2b).contains(&self.city.building_at(x, y)) {
            let tile = crate::app::network::pe_reshape::pe_surface_neighbor_shape(
                &self.city,
                x,
                y,
                NetworkKind::Road,
            );
            pe_place_tile_441f00(&mut self.city, x, y, tile);
        }
    }

    /// PE `FUN_0044B1A0` reshapes center, left, right, up, down only after an
    /// admitted PlaceRails write (`0x44b27c..0x44b2cb`).
    pub(in crate::app) fn refresh_rail_neighbors_like_pe_44b1a0(&mut self, x: usize, y: usize) {
        for (nx, ny) in std::iter::once((x, y)).chain(neighbor_tiles(x, y)) {
            self.refresh_pe_rail_tile_44b3a0(nx, ny);
            persist_pe_neighbor_connection_overlay(&mut self.city, nx, ny);
        }
    }

    pub(super) fn refresh_pe_rail_tile_44b3a0(&mut self, x: usize, y: usize) {
        if (0x2c..=0x3a).contains(&self.city.building_at(x, y)) {
            let tile = crate::app::network::pe_reshape::pe_surface_neighbor_shape(
                &self.city,
                x,
                y,
                NetworkKind::Rail,
            );
            pe_place_tile_441f00(&mut self.city, x, y, tile);
        }
    }

    pub(in crate::app) fn network_neighbor_mask_with_virtual(
        &self,
        x: usize,
        y: usize,
        kind: NetworkKind,
        virtual_tiles: &HashSet<(usize, usize)>,
    ) -> u8 {
        let mut mask = 0;
        if y > 0 && self.network_neighbor_matches(x, y - 1, kind, virtual_tiles, NETWORK_BOTTOM) {
            mask |= NETWORK_TOP;
        }
        if x + 1 < CITY_WIDTH
            && self.network_neighbor_matches(x + 1, y, kind, virtual_tiles, NETWORK_LEFT)
        {
            mask |= NETWORK_RIGHT;
        }
        if y + 1 < CITY_HEIGHT
            && self.network_neighbor_matches(x, y + 1, kind, virtual_tiles, NETWORK_TOP)
        {
            mask |= NETWORK_BOTTOM;
        }
        if x > 0 && self.network_neighbor_matches(x - 1, y, kind, virtual_tiles, NETWORK_RIGHT) {
            mask |= NETWORK_LEFT;
        }
        mask
    }

    pub(in crate::app) fn network_neighbor_matches(
        &self,
        x: usize,
        y: usize,
        kind: NetworkKind,
        virtual_tiles: &HashSet<(usize, usize)>,
        side: u8,
    ) -> bool {
        if virtual_tiles.contains(&(x, y)) {
            return true;
        }
        // PE PlacePowerLines reshape @ `0x44ab2c`: XBIT 0x80 + NS/EW gates on XBLD.
        if kind == NetworkKind::Power {
            if !self
                .city
                .tile_bits_at(x, y)
                .is_some_and(TileBits::powerable)
            {
                return false;
            }
            let building = self.city.building_at(x, y);
            // Ordinary wires reshape together regardless of their old art.
            // Each newly placed wire starts as 0x0e (T/B); requiring an old
            // L/R endpoint prevents a horizontal run from ever rotating.
            // Keep the PE gates for crossovers and other powered sites.
            return if (0x0e..=0x1c).contains(&building) {
                true
            } else {
                pe_place_power_reshape_neighbor_connects_44ab30(&self.city, x, y, side)
            };
        }
        let building = self.city.building_at(x, y);
        network_building_connects_to(building, kind, side)
            || (kind == NetworkKind::Road
                && onramp_sides(
                    building,
                    self.city
                        .tile_bits_at(x, y)
                        .is_some_and(|bits| bits.rotate()),
                )
                .is_some_and(|(_, road_side)| road_side == side))
    }

    pub(in crate::app) fn can_place_network_at(
        &self,
        x: usize,
        y: usize,
        kind: NetworkKind,
    ) -> bool {
        if is_water_terrain(self.city.terrain_at(x, y)) {
            return false;
        }
        let building = self.city.building_at(x, y);
        building == 0
            || is_tree_building(building)
            || kind.contains(building)
            || crossover_network_axis(building, kind).is_some()
    }

    pub(in crate::app) fn network_placement_for_path(
        &self,
        path: &[(usize, usize)],
        kind: NetworkKind,
    ) -> ToolPlacement {
        let mut valid_tiles = Vec::new();
        let mut invalid_tiles = Vec::new();
        for (index, &(x, y)) in path.iter().enumerate() {
            if self.can_place_network_path_tile(path, index, kind) {
                valid_tiles.push((x, y));
            } else {
                invalid_tiles.push((x, y));
            }
        }
        ToolPlacement {
            valid_tiles,
            invalid_tiles,
            preview: ToolPreview::Network(kind),
        }
    }

    pub(in crate::app) fn network_drag_placement_for_path(
        &self,
        path: &[(usize, usize)],
        kind: NetworkKind,
    ) -> ToolPlacement {
        if kind != NetworkKind::Power {
            return self.network_placement_for_path(path, kind);
        }
        let (valid_tiles, invalid_tile) = self.network_buildable_prefix_path(path, kind);
        ToolPlacement {
            valid_tiles,
            invalid_tiles: invalid_tile.into_iter().collect(),
            preview: ToolPreview::Network(kind),
        }
    }

    pub(in crate::app) fn network_buildable_prefix_path(
        &self,
        path: &[(usize, usize)],
        kind: NetworkKind,
    ) -> BuildablePrefixPath {
        let mut prefix = Vec::new();
        for &tile in path {
            prefix.push(tile);
            if !self
                .network_placement_for_path(&prefix, kind)
                .invalid_tiles
                .is_empty()
            {
                prefix.pop();
                return (prefix, Some(tile));
            }
        }
        (prefix, None)
    }

    pub(in crate::app) fn sub_to_rail_plan_at(&self, x: usize, y: usize) -> Option<SubToRailPlan> {
        if x >= CITY_WIDTH || y >= CITY_HEIGHT {
            return None;
        }
        // PE MapTool SubToRail @ `0x441ba8`: XBLD >= 0x0d → refuse (no DestroyBuilding).
        // PE @ `0x441bb6`: XBLD == 5 → refuse (local early gate; before adjacency).
        let building = self.city.building_at(x, y);
        if building >= 0x0d || building == 5 {
            return None;
        }
        // PE @ `0x441bce`–`0x441bd2`: `cmp XTER,0` / `je` adjacency; non-flat falls
        // through to `return 0`. Not MapTool vacant (`XTER < 0x30`); require flat.
        if pe_sub_to_rail_xter_refuse_441bd2(&self.city, x, y) {
            return None;
        }
        if self.engine_fidelity.uses_win95() {
            let placement = self.building_placement_at(x, y, 108);
            if !placement.invalid_tiles.is_empty() || placement.valid_tiles != [(x, y)] {
                return None;
            }
        }

        // CODE 5 @ 0x4616 uses the same neighbor priority and tile bands.
        // PE MapTool SubToRail adjacency @ `0x441be8`–`0x441cff`: rail XBLD bands or
        // subway XUND (side flipped); refuse when neither.
        let (side, nx, ny) = pe_sub_to_rail_adjacent_side_441be8(&self.city, x, y)?;
        Some(SubToRailPlan {
            building: sub_to_rail_building_for_side(side)?,
            reshape_neighbor: (nx, ny),
        })
    }

    pub(in crate::app) fn sub_to_rail_placement_at(&self, x: usize, y: usize) -> ToolPlacement {
        if let Some(plan) = self.sub_to_rail_plan_at(x, y) {
            ToolPlacement::valid(ToolPreview::Building(plan.building), vec![(x, y)])
        } else {
            ToolPlacement {
                valid_tiles: Vec::new(),
                invalid_tiles: vec![(x, y)],
                preview: ToolPreview::Building(108),
            }
        }
    }

    pub(in crate::app) fn place_sub_to_rail_at(&mut self, x: usize, y: usize) -> bool {
        let Some(plan) = self.sub_to_rail_plan_at(x, y) else {
            return false;
        };
        if self.engine_fidelity == EngineFidelity::Mac68k {
            // CODE 5 @ 0x470e–0x4782 places without a cash check or debit.
            self.place_mac_subway_transition_underground(x, y);
            pe_place_tile_441f00(&mut self.city, x, y, plan.building);
            let zone = self.city.zone_at(x, y).map_or(0, |z| z.raw_bits());
            self.city.set_zone(x, y, zone | ZONE_CORNER_ALL);
            self.refresh_mac_rail_tile(plan.reshape_neighbor.0, plan.reshape_neighbor.1);
            self.pending_traffic_redraw = true;
            self.query_is_inspect = false;
            self.refresh_status_line();
            return true;
        }
        if !self.can_afford_tool_build_cost(EditTool::SubToRail, 1) {
            return false;
        }

        // PE SubToRail place @ `0x441d00` (not MapTool `FUN_00443270` / `0x442eb9`):
        // PlaceSubway → PlaceXUND(0x23) → XBIT &= 0xDF → PlaceTile → XZON |= 0xf0 →
        // PlaceRails on adjacency neighbor. Skip `0x4012c1`.
        self.apply_building_underground_side_effects(x, y, plan.building);
        // PlaceTile does not rewrite XZON; stamp_building_footprint currently zeros
        // 1×1 zones, so capture prior bits for the SubToRail OR.
        let prior_zone = self.city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
        self.stamp_building_footprint(x, y, plan.building, &[(x, y)]);
        // PE MapTool SubToRail @ `0x441d8e`: `XZON |= 0xf0` after PlaceTile.
        self.city.set_zone(x, y, prior_zone | ZONE_CORNER_ALL);
        // PE @ `0x441da1` → `0x401843` / PlaceRails reshape once on adjacency neighbor
        // `(edi,ebx)` — not invent four-neighbor + self rail refresh.
        self.refresh_network_tile(
            plan.reshape_neighbor.0,
            plan.reshape_neighbor.1,
            NetworkKind::Rail,
        );
        self.pending_traffic_redraw = true;
        // PE PlaceTile path: no invent citywide water-network refresh.
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.debit_tool_build_cost(EditTool::SubToRail, 1);
        self.query_is_inspect = false;
        self.refresh_status_line();
        true
    }
}
