//! Road, rail, power, and highway water-bridge choices and placement transactions.

use super::super::{
    CityInfoDialog, PE_PIPE_WALK_DIRECTIONS_4E7B18, ViewerState, is_tree_building,
    is_water_terrain, pe_pipe_step_coordinate_44f50, pe_place_tile_441f00,
    pe_powerline_post_place_power_recalc_43f396, stamp_xbit_powerable,
};
use super::highway_axis_for_pe_direction;
use super::highways::{
    highway_segment_tiles, pe_highway_altitude_463f60, pe_highway_water_bridge_candidate_45c1e0,
    pe_highway_water_selector_cost_45c1e0, pe_place_highway_tile_for_stamp_462ed0,
};
use super::strokes::{
    pe_place_network_clear_zone_type_nibble, pe_place_rails_coordinate_should_stamp_44b1a0,
    pe_place_rails_crossover_from_existing_44b2f4, pe_place_roads_coordinate_should_stamp_44acf4,
    pe_place_roads_crossover_from_existing_44ae24,
};
use super::{
    HIGHWAY_TILES_PER_SEGMENT, NetworkAxis, NetworkKind, PE_POWER_BRIDGE_SELECTOR_MASK,
    PE_ROAD_BRIDGE_SELECTOR_MASK, PeHighwayWaterBridgeCandidate, PeRoadBridgeSpanPlan,
    crossover_network_axis, network_axis_mask_for_side_mask, network_single_axis,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

pub(in crate::app) fn network_bridge_building_for_tile(
    kind: NetworkKind,
    mask: u8,
    terrain: u8,
) -> Option<u8> {
    if !is_water_terrain(terrain) {
        return None;
    }
    network_single_axis(network_axis_mask_for_side_mask(mask))?;
    let building = match kind {
        NetworkKind::Road => 87,
        NetworkKind::Rail => 91,
        // OpenSC2K `Tiles.POWER_BRIDGE` / wire bridge over water.
        NetworkKind::Power => 92,
    };
    Some(building)
}

pub(in crate::app) fn rail_bridge_building_for_run_index(index: usize) -> u8 {
    if index.is_multiple_of(2) { 91 } else { 90 }
}

pub(in crate::app) fn opposite_rail_bridge_building(building: u8) -> u8 {
    match building {
        90 => 91,
        91 => 90,
        _ => 91,
    }
}

pub(in crate::app) fn can_place_network_bridge_on_building(
    building: u8,
    kind: NetworkKind,
) -> bool {
    building == 0 || is_tree_building(building) || kind.contains(building)
}

pub(in crate::app) fn pe_power_bridge_selector_cost_45b930(selector: u8) -> Option<u32> {
    (selector == 0).then_some(10)
}

pub(in crate::app) fn pe_road_bridge_selectors(mask: u8) -> Vec<u8> {
    (2u8..=4)
        .filter(|&selector| mask & (1u8 << selector) != 0)
        .collect()
}

pub(in crate::app) fn pe_road_bridge_selector_name(selector: u8) -> &'static str {
    match selector {
        2 => "Causeway",
        3 => "Raising Bridge",
        4 => "Suspension",
        _ => "Unknown bridge",
    }
}

pub(in crate::app) fn pe_road_bridge_selector_cost(selector: u8) -> u32 {
    match selector {
        2 => 25,
        3 => 50,
        4 => 75,
        _ => 0,
    }
}

pub(in crate::app) fn pe_road_bridge_selector_mask(span_count: usize) -> u8 {
    pe_bridge_length_selector_mask(span_count) & PE_ROAD_BRIDGE_SELECTOR_MASK
}

pub(in crate::app) fn pe_bridge_length_selector_mask(span_count: usize) -> u8 {
    match span_count {
        1..=4 => 0x07,
        5..=6 => 0x0f,
        7..=11 => 0x1f,
        _ => 0x17,
    }
}

pub(in crate::app) fn pe_road_bridge_direction_mask(xter: u8) -> Option<u8> {
    if !(0x20..0x40).contains(&xter) {
        return None;
    }
    match xter & 0x0f {
        1 => Some(0x02),
        2 => Some(0x04),
        3 => Some(0x08),
        4 => Some(0x01),
        5 => Some(0x06),
        6 => Some(0x0c),
        7 => Some(0x09),
        8 => Some(0x03),
        _ => None,
    }
}

pub(in crate::app) fn pe_road_bridge_mode1_direction(
    city: &City,
    seed: (usize, usize),
) -> Option<u8> {
    let mask = pe_road_bridge_direction_mask(city.terrain_at(seed.0, seed.1))?;
    let compass = city.rotation() & 3;
    [
        compass,
        (compass + 2) & 3,
        (compass + 1) & 3,
        (compass + 3) & 3,
    ]
    .into_iter()
    .find(|&direction| mask & (1u8 << direction) != 0)
}

/// Preflight the shared PE bridge worker from its water seed and saved direction.
/// The count includes the first coordinate outside XTER `0x10..=0x3f`.
pub(in crate::app) fn pe_road_bridge_span_plan(
    city: &City,
    seed: (usize, usize),
    direction: u8,
) -> Option<PeRoadBridgeSpanPlan> {
    pe_water_bridge_span_plan(city, seed, direction, PE_ROAD_BRIDGE_SELECTOR_MASK)
}

/// The Power wrapper admits only shared-worker selector 0 (`FUN_0044A310`).
pub(in crate::app) fn pe_power_bridge_span_plan(
    city: &City,
    seed: (usize, usize),
    direction: u8,
) -> Option<PeRoadBridgeSpanPlan> {
    pe_water_bridge_span_plan(city, seed, direction, PE_POWER_BRIDGE_SELECTOR_MASK)
}

pub(in crate::app) fn pe_water_bridge_span_plan(
    city: &City,
    seed: (usize, usize),
    direction: u8,
    selector_mask: u8,
) -> Option<PeRoadBridgeSpanPlan> {
    if seed.0 >= CITY_WIDTH || seed.1 >= CITY_HEIGHT {
        return None;
    }
    let direction_mask = pe_road_bridge_direction_mask(city.terrain_at(seed.0, seed.1))?;
    if direction_mask & (1u8 << (direction & 3)) == 0 {
        return None;
    }

    let mut cells = vec![seed];
    let mut current = seed;
    let mut terminal = None;
    for _ in 0..(CITY_WIDTH + CITY_HEIGHT) {
        let next = pe_pipe_step_coordinate_44f50(current, direction)?;
        let xter = city.terrain_at(next.0, next.1);
        if !(0x10..=0x3f).contains(&xter) {
            terminal = Some(next);
            break;
        }
        if city.building_at(next.0, next.1) != 0 {
            return None;
        }
        cells.push(next);
        current = next;
    }
    let terminal = terminal?;
    let span_count = cells.len();
    Some(PeRoadBridgeSpanPlan {
        cells,
        terminal,
        span_count,
        selector_mask: pe_bridge_length_selector_mask(span_count) & selector_mask,
    })
}

/// The selected-Rail wrapper intersects the shared length mask with bit 1,
/// leaving selector 1 as its sole bridge family at every valid span length.
pub(in crate::app) fn pe_rail_bridge_span_plan(
    city: &City,
    seed: (usize, usize),
    direction: u8,
) -> Option<PeRoadBridgeSpanPlan> {
    pe_water_bridge_span_plan(city, seed, direction, 0x02)
}

pub(in crate::app) fn pe_rail_bridge_selector_cost() -> u32 {
    75
}

/// PE selector 1's center marker and four-neighbor approach tiles (`0x45BD5A`).
pub(in crate::app) fn pe_rail_bridge_interior_tile(span_count: usize, index: usize) -> u8 {
    let center = span_count / 2;
    if index != center && index.abs_diff(center) <= 2 {
        0x5b
    } else {
        0x5a
    }
}

pub(in crate::app) fn pe_road_bridge_interior_tile(
    selector: u8,
    span_count: usize,
    index: usize,
    direction: u8,
) -> u8 {
    match selector {
        2 => 0x57,
        3 => {
            let center = (span_count + 1) / 4;
            if center > index {
                0x57
            } else if center == index {
                0x56
            } else {
                let far_center = span_count.saturating_sub(center);
                if far_center.saturating_sub(1) > index {
                    0x58
                } else if far_center.saturating_sub(index) == 1 {
                    0x56
                } else {
                    0x57
                }
            }
        }
        4 => {
            let center = (((span_count - 2) % 5) + 2) / 2;
            if center > index {
                0x57
            } else if matches!(direction & 3, 0 | 3) {
                0x51 + ((index - center) % 5) as u8
            } else {
                0x55 - ((index - center) % 5) as u8
            }
        }
        _ => 0x57,
    }
}

impl ViewerState {
    pub(in crate::app) fn begin_win95_power_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
    ) -> usize {
        let Some(plan) = pe_power_bridge_span_plan(&self.city, seed, direction) else {
            return 0;
        };
        if plan.selector_mask & PE_POWER_BRIDGE_SELECTOR_MASK == 0 {
            return 0;
        }
        self.open_city_info_dialog(CityInfoDialog::PowerBridge {
            seed_x: seed.0,
            seed_y: seed.1,
            direction: direction & 3,
            span_count: plan.span_count,
        });
        0
    }

    pub(in crate::app) fn finish_win95_power_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        expected_span_count: usize,
    ) -> usize {
        let direction = direction & 3;
        let Some(plan) = pe_power_bridge_span_plan(&self.city, seed, direction) else {
            return 0;
        };
        if plan.span_count != expected_span_count
            || plan.selector_mask & PE_POWER_BRIDGE_SELECTOR_MASK == 0
        {
            return 0;
        }
        let Some(cost_per_tile) = pe_power_bridge_selector_cost_45b930(0) else {
            return 0;
        };
        let Some(cost) = cost_per_tile.checked_mul(plan.span_count as u32) else {
            return 0;
        };
        if !self.can_afford_build_cost(cost) {
            return 0;
        }

        let far_endpoint = if plan.span_count == 1 {
            plan.terminal
        } else {
            plan.cells[plan.span_count - 1]
        };
        self.debit_build_cost(cost);
        self.place_win95_power_bridge_endpoint(seed, ((direction + 1) & 3) + 1);
        let mut changed_tiles = vec![seed];
        for index in 1..plan.span_count.saturating_sub(1) {
            let (x, y) = plan.cells[index];
            if direction & 1 != 0 {
                let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
                self.city.set_tile_bits(x, y, bits.with_rotate(true));
            }
            pe_place_tile_441f00(&mut self.city, x, y, 0x5c);
            stamp_xbit_powerable(&mut self.city, x, y);
            changed_tiles.push((x, y));
        }
        self.place_win95_power_bridge_endpoint(far_endpoint, ((direction + 3) & 3) + 1);
        changed_tiles.push(far_endpoint);

        self.clear_zone_growth_for_player_tiles(&changed_tiles);
        self.pending_traffic_redraw = true;
        pe_powerline_post_place_power_recalc_43f396(&mut self.city, &mut self.pe_process_crt);
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.query_is_inspect = false;
        self.refresh_status_line();
        plan.span_count.max(2)
    }

    pub(in crate::app) fn place_win95_power_bridge_endpoint(
        &mut self,
        tile: (usize, usize),
        orientation: u8,
    ) {
        let (x, y) = tile;
        if self.city.terrain_at(x, y) < 0x30 {
            let sample = self.city.altm_at(x, y);
            let word = u16::from(sample.land & 0x1f)
                | (u16::from(sample.water & 0x1f) << 5)
                | (u16::from(sample.tunnel_levels & 0x3f) << 10);
            let next_land = (word & 0x001f) + 1;
            self.city.set_altm_word(x, y, (word & 0xffe0) | next_land);
        }
        self.city.set_terrain(x, y, orientation);
        let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
        self.city.set_tile_bits(x, y, bits.with_water_tile(false));
        self.place_win95_power_line_coordinate_44a8a0(tile);
    }

    pub(in crate::app) fn begin_win95_rail_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
    ) -> usize {
        let Some(plan) = pe_rail_bridge_span_plan(&self.city, seed, direction) else {
            return 0;
        };
        self.open_city_info_dialog(CityInfoDialog::RailBridge {
            seed_x: seed.0,
            seed_y: seed.1,
            direction: direction & 3,
            span_count: plan.span_count,
        });
        0
    }

    pub(in crate::app) fn finish_win95_rail_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        expected_span_count: usize,
    ) -> usize {
        let direction = direction & 3;
        let Some(plan) = pe_rail_bridge_span_plan(&self.city, seed, direction) else {
            return 0;
        };
        if plan.span_count != expected_span_count || plan.selector_mask & 0x02 == 0 {
            return 0;
        }
        let Some(cost) = pe_rail_bridge_selector_cost().checked_mul(plan.span_count as u32) else {
            return 0;
        };
        if !self.can_afford_build_cost(cost) {
            return 0;
        }

        let far_endpoint = if plan.span_count == 1 {
            plan.terminal
        } else {
            plan.cells[plan.span_count - 1]
        };
        self.debit_build_cost(cost);
        self.place_win95_rail_bridge_endpoint(seed, ((direction + 1) & 3) + 1);
        for index in 1..plan.span_count.saturating_sub(1) {
            let (x, y) = plan.cells[index];
            pe_place_tile_441f00(
                &mut self.city,
                x,
                y,
                pe_rail_bridge_interior_tile(plan.span_count, index),
            );
            if direction & 1 != 0 {
                let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
                self.city.set_tile_bits(x, y, bits.with_rotate(true));
            }
            // PE's interior worker follows each direct XBLD/XBIT write with
            // the tile-refresh thunk; City setters dirty the affected cell.
        }
        self.place_win95_rail_bridge_endpoint(far_endpoint, ((direction + 3) & 3) + 1);

        let mut changed_tiles = plan.cells;
        changed_tiles.push(far_endpoint);
        self.clear_zone_growth_for_player_tiles(&changed_tiles);
        self.pending_traffic_redraw = true;
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.query_is_inspect = false;
        self.refresh_status_line();
        plan.span_count.max(2)
    }

    pub(in crate::app) fn place_win95_rail_bridge_endpoint(
        &mut self,
        tile: (usize, usize),
        orientation: u8,
    ) {
        let (x, y) = tile;
        if self.city.terrain_at(x, y) < 0x30 {
            let sample = self.city.altm_at(x, y);
            let word = u16::from(sample.land & 0x1f)
                | (u16::from(sample.water & 0x1f) << 5)
                | (u16::from(sample.tunnel_levels & 0x3f) << 10);
            let next_land = (word & 0x001f) + 1;
            self.city.set_altm_word(x, y, (word & 0xffe0) | next_land);
        }
        self.city.set_terrain(x, y, orientation);
        let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
        self.city.set_tile_bits(x, y, bits.with_water_tile(false));

        let existing = self.city.building_at(x, y);
        if pe_place_rails_coordinate_should_stamp_44b1a0(existing)
            && crossover_network_axis(existing, NetworkKind::Rail).is_none()
        {
            let building = pe_place_rails_crossover_from_existing_44b2f4(existing)
                .unwrap_or_else(|| NetworkKind::Rail.base_tile());
            pe_place_tile_441f00(&mut self.city, x, y, building);
            pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, NetworkKind::Rail);
            self.refresh_rail_neighbors_like_pe_44b1a0(x, y);
        }
    }

    pub(in crate::app) fn begin_win95_road_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
    ) -> usize {
        let Some(plan) = pe_road_bridge_span_plan(&self.city, seed, direction) else {
            return 0;
        };
        let selectors = pe_road_bridge_selectors(plan.selector_mask);
        if selectors.is_empty() {
            return 0;
        }
        self.open_city_info_dialog(CityInfoDialog::RoadBridge {
            seed_x: seed.0,
            seed_y: seed.1,
            direction: direction & 3,
            span_count: plan.span_count,
            selector_mask: plan.selector_mask,
        });
        0
    }

    pub(in crate::app) fn begin_win95_highway_water_bridge(
        &mut self,
        candidate: PeHighwayWaterBridgeCandidate,
    ) {
        self.open_city_info_dialog(CityInfoDialog::HighwayWaterBridge { candidate });
    }

    pub(in crate::app) fn finish_win95_highway_water_bridge(
        &mut self,
        expected: PeHighwayWaterBridgeCandidate,
        selector: u8,
    ) -> usize {
        let Some(plan) = pe_highway_water_bridge_candidate_45c1e0(
            &self.city,
            expected.seed,
            expected.mode,
            expected.direction,
        ) else {
            return 0;
        };
        if plan != expected || plan.selector_mask & (1 << selector) == 0 {
            return 0;
        }
        let Some(cost) = pe_highway_water_selector_cost_45c1e0(selector)
            .and_then(|unit| unit.checked_mul(plan.span_count as u32))
        else {
            return 0;
        };
        if !self.can_afford_build_cost(cost) {
            return 0;
        }

        let (dx, dy) = PE_PIPE_WALK_DIRECTIONS_4E7B18[usize::from(plan.direction)];
        let origin_at = |index: usize| {
            Some((
                plan.seed
                    .0
                    .checked_add_signed(dx.checked_mul(2)?.checked_mul(index as isize)?)?,
                plan.seed
                    .1
                    .checked_add_signed(dy.checked_mul(2)?.checked_mul(index as isize)?)?,
            ))
        };
        let near_endpoint = (
            plan.seed.0.checked_add_signed(-dx * 2),
            plan.seed.1.checked_add_signed(-dy * 2),
        );
        let far_endpoint = origin_at(plan.span_count);
        let (Some(near_x), Some(near_y), Some(far_endpoint)) =
            (near_endpoint.0, near_endpoint.1, far_endpoint)
        else {
            return 0;
        };

        let mut changed_tiles = Vec::with_capacity(plan.span_count * HIGHWAY_TILES_PER_SEGMENT + 8);
        if selector == 5 {
            let building = 0x49 + (plan.direction & 1);
            for index in 0..plan.span_count {
                let Some(origin) = origin_at(index) else {
                    return 0;
                };
                let tiles =
                    highway_segment_tiles(origin, highway_axis_for_pe_direction(plan.direction));
                if tiles.len() != HIGHWAY_TILES_PER_SEGMENT {
                    return 0;
                }
                for (x, y) in tiles {
                    pe_place_tile_441f00(&mut self.city, x, y, building);
                    let zone = self.city.zone_at(x, y).map(|z| z.raw_bits()).unwrap_or(0);
                    self.city.set_zone(x, y, zone | 0xf0);
                    changed_tiles.push((x, y));
                }
            }
        } else if selector == 6 {
            let seed_height = pe_highway_altitude_463f60(&self.city, plan.seed.0, plan.seed.1);
            let near_height = pe_highway_altitude_463f60(&self.city, near_x, near_y);
            let near_class = if i32::from(seed_height) - i32::from(near_height) == -1 {
                i16::from(plan.direction & 1)
            } else {
                i16::from((plan.direction + 1 & 3) + 4)
            };
            pe_place_highway_tile_for_stamp_462ed0(&mut self.city, near_x, near_y, near_class);
            changed_tiles.extend(highway_segment_tiles((near_x, near_y), NetworkAxis::GridX));

            for index in 0..plan.span_count {
                let Some(origin) = origin_at(index) else {
                    return 0;
                };
                let class = if index & 1 == 0 { 0x0e } else { 0x0d };
                pe_place_highway_tile_for_stamp_462ed0(&mut self.city, origin.0, origin.1, class);
                let tiles = highway_segment_tiles(origin, NetworkAxis::GridX);
                for (x, y) in tiles {
                    let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
                    self.city
                        .set_tile_bits(x, y, bits.with_rotate(plan.direction & 1 != 0));
                    changed_tiles.push((x, y));
                }
            }

            let far_height = pe_highway_altitude_463f60(&self.city, far_endpoint.0, far_endpoint.1);
            let far_class = if i32::from(seed_height) - i32::from(far_height) == -1 {
                i16::from(plan.direction & 1)
            } else {
                i16::from((plan.direction + 3 & 3) + 4)
            };
            pe_place_highway_tile_for_stamp_462ed0(
                &mut self.city,
                far_endpoint.0,
                far_endpoint.1,
                far_class,
            );
            changed_tiles.extend(highway_segment_tiles(far_endpoint, NetworkAxis::GridX));
        } else {
            return 0;
        }

        self.debit_build_cost(cost);
        self.clear_zone_growth_for_player_tiles(&changed_tiles);
        self.pending_traffic_redraw = true;
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.query_is_inspect = false;
        self.refresh_status_line();
        plan.span_count
    }

    pub(in crate::app) fn finish_win95_road_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        expected_span_count: usize,
        selector: u8,
    ) -> usize {
        let direction = direction & 3;
        let Some(plan) = pe_road_bridge_span_plan(&self.city, seed, direction) else {
            return 0;
        };
        if plan.span_count != expected_span_count || plan.selector_mask & (1 << selector) == 0 {
            return 0;
        }
        let cost = pe_road_bridge_selector_cost(selector).saturating_mul(plan.span_count as u32);
        if cost == 0 || !self.can_afford_build_cost(cost) {
            return 0;
        }
        self.debit_build_cost(cost);

        let far_endpoint = if plan.span_count == 1 {
            plan.terminal
        } else {
            plan.cells[plan.span_count - 1]
        };
        self.place_win95_road_bridge_endpoint(seed, ((direction + 1) & 3) + 1);
        for index in 1..plan.span_count.saturating_sub(1) {
            let (x, y) = plan.cells[index];
            let tile = pe_road_bridge_interior_tile(selector, plan.span_count, index, direction);
            pe_place_tile_441f00(&mut self.city, x, y, tile);
            if direction & 1 != 0 {
                let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
                self.city.set_tile_bits(x, y, bits.with_rotate(true));
            }
        }
        self.place_win95_road_bridge_endpoint(far_endpoint, ((direction + 3) & 3) + 1);

        let placed = plan.span_count.max(2);
        let mut bridge_cells = plan.cells;
        bridge_cells.push(far_endpoint);
        self.clear_zone_growth_for_player_tiles(&bridge_cells);
        self.pending_traffic_redraw = true;
        self.refresh_live_misc_current_values();
        self.refresh_budget_service_rows_from_live_counts();
        self.refresh_reward_availability();
        self.query_is_inspect = false;
        self.refresh_status_line();
        placed
    }

    pub(in crate::app) fn place_win95_road_bridge_endpoint(
        &mut self,
        tile: (usize, usize),
        orientation: u8,
    ) {
        let (x, y) = tile;
        if self.city.terrain_at(x, y) < 0x30 {
            let sample = self.city.altm_at(x, y);
            let word = u16::from(sample.land & 0x1f)
                | (u16::from(sample.water & 0x1f) << 5)
                | (u16::from(sample.tunnel_levels & 0x3f) << 10);
            let next_land = (word & 0x001f) + 1;
            self.city.set_altm_word(x, y, (word & 0xffe0) | next_land);
        }
        self.city.set_terrain(x, y, orientation);
        let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
        self.city.set_tile_bits(x, y, bits.with_water_tile(false));

        let existing = self.city.building_at(x, y);
        if pe_place_roads_coordinate_should_stamp_44acf4(existing)
            && crossover_network_axis(existing, NetworkKind::Road).is_none()
        {
            let building = pe_place_roads_crossover_from_existing_44ae24(existing)
                .unwrap_or_else(|| NetworkKind::Road.base_tile());
            pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, NetworkKind::Road);
            pe_place_tile_441f00(&mut self.city, x, y, building);
            self.refresh_road_neighbors_like_pe_44acc0(x, y);
        }
    }
}
