//! Road, rail, power, and highway water-bridge choices and placement transactions.

use super::super::{
    CityInfoDialog, PE_PIPE_WALK_DIRECTIONS_4E7B18, ViewerState, is_tree_building,
    is_water_terrain, pe_pipe_step_coordinate_44f50, pe_place_tile_441f00,
    pe_powerline_post_place_power_recalc_43f396, stamp_xbit_powerable,
};
use super::highway_axis_for_pe_direction;
use super::highways::{
    highway_segment_tiles, mac_highway_water_bridge_candidate, pe_highway_altitude_463f60,
    pe_highway_water_bridge_candidate_45c1e0, pe_highway_water_selector_cost_45c1e0,
    pe_place_highway_tile_for_stamp_462ed0,
};
use super::strokes::{
    pe_place_network_clear_zone_type_nibble, pe_place_rails_coordinate_should_stamp_44b1a0,
    pe_place_rails_crossover_from_existing_44b2f4, pe_place_roads_coordinate_should_stamp_44acf4,
    pe_place_roads_crossover_from_existing_44ae24,
};
use super::{
    HIGHWAY_TILES_PER_SEGMENT, HighwayWaterBridgeCandidate, NetworkAxis, NetworkKind,
    PE_POWER_BRIDGE_SELECTOR_MASK, PE_ROAD_BRIDGE_SELECTOR_MASK, PeRoadBridgeSpanPlan,
    RailNeighborLinkCandidate, RoadNeighborLinkCandidate, crossover_network_axis,
    network_axis_mask_for_side_mask, network_single_axis,
};
use crate::city::{CITY_HEIGHT, CITY_WIDTH, City};

fn stamp_highway_water_far_ramp(
    city: &mut City,
    endpoint: (usize, usize),
    seed_height: u16,
    direction: u8,
) {
    let (x, y) = endpoint;
    let height = pe_highway_altitude_463f60(city, x, y);
    let class = if height == seed_height + 1 {
        direction & 1
    } else {
        ((direction + 3) & 3) + 4
    };
    pe_place_highway_tile_for_stamp_462ed0(city, x, y, i16::from(class));
}

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
    road_bridge_direction_for_compass(city.terrain_at(seed.0, seed.1), city.rotation())
}

pub(super) fn mac_water_bridge_origin_direction(city: &City, seed: (usize, usize)) -> Option<u8> {
    // The native bridge worker reads the persisted compass at A5-0x1f3a.
    road_bridge_direction_for_compass(city.terrain_at(seed.0, seed.1), city.saved_rotation())
}

fn road_bridge_direction_for_compass(terrain: u8, compass: u8) -> Option<u8> {
    let mask = pe_road_bridge_direction_mask(terrain)?;
    let compass = compass & 3;
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
/// The first coordinate outside XTER `0x10..=0x3f` is the terminal, excluded
/// from the quoted span count.
pub(in crate::app) fn pe_road_bridge_span_plan(
    city: &City,
    seed: (usize, usize),
    direction: u8,
) -> Option<PeRoadBridgeSpanPlan> {
    pe_water_bridge_span_plan(city, seed, direction, PE_ROAD_BRIDGE_SELECTOR_MASK)
}

/// Both Power wrappers admit only shared-worker selector 0.
pub(in crate::app) fn power_bridge_span_plan(
    city: &City,
    seed: (usize, usize),
    direction: u8,
    check_seed_direction: bool,
) -> Option<PeRoadBridgeSpanPlan> {
    water_bridge_span_plan(
        city,
        seed,
        direction,
        PE_POWER_BRIDGE_SELECTOR_MASK,
        check_seed_direction,
    )
}

pub(in crate::app) fn pe_water_bridge_span_plan(
    city: &City,
    seed: (usize, usize),
    direction: u8,
    selector_mask: u8,
) -> Option<PeRoadBridgeSpanPlan> {
    water_bridge_span_plan(city, seed, direction, selector_mask, true)
}

fn water_bridge_span_plan(
    city: &City,
    seed: (usize, usize),
    direction: u8,
    selector_mask: u8,
    check_seed_direction: bool,
) -> Option<PeRoadBridgeSpanPlan> {
    if seed.0 >= CITY_WIDTH || seed.1 >= CITY_HEIGHT {
        return None;
    }
    let direction_mask = pe_road_bridge_direction_mask(city.terrain_at(seed.0, seed.1))?;
    if check_seed_direction && direction_mask & (1u8 << (direction & 3)) == 0 {
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
            let midpoint_seed = ((span_count - 2) % 5) + 2;
            let center = midpoint_seed / 2;
            // The PE returns to causeway padding at the far side as well as
            // before the suspension family. Its upper bound uses the full
            // midpoint seed, not the halved center.
            let far_boundary = span_count - midpoint_seed.div_ceil(2);
            if center > index || index >= far_boundary {
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
    pub(in crate::app) fn begin_power_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        dry_stroke: bool,
    ) -> bool {
        let Some(plan) = power_bridge_span_plan(
            &self.city,
            seed,
            direction,
            self.engine_fidelity.uses_win95() && !dry_stroke,
        ) else {
            return false;
        };
        if plan.selector_mask & PE_POWER_BRIDGE_SELECTOR_MASK == 0 {
            return false;
        }
        self.open_city_info_dialog(CityInfoDialog::PowerBridge {
            seed_x: seed.0,
            seed_y: seed.1,
            direction: direction & 3,
            span_count: plan.span_count,
            dry_stroke,
        });
        true
    }

    pub(in crate::app) fn finish_power_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        expected_span_count: usize,
    ) -> usize {
        let direction = direction & 3;
        let dry_stroke = matches!(
            self.city_info_dialog,
            Some(CityInfoDialog::PowerBridge {
                dry_stroke: true,
                ..
            })
        );
        let Some(plan) = power_bridge_span_plan(
            &self.city,
            seed,
            direction,
            self.engine_fidelity.uses_win95() && !dry_stroke,
        ) else {
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
        if !self.charge_water_bridge(cost) {
            return 0;
        }

        let far_endpoint = if plan.span_count == 1 {
            plan.terminal
        } else {
            plan.cells[plan.span_count - 1]
        };
        self.place_power_bridge_endpoint(seed, ((direction + 1) & 3) + 1);
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
        self.place_power_bridge_endpoint(far_endpoint, ((direction + 3) & 3) + 1);
        changed_tiles.push(far_endpoint);

        self.clear_zone_growth_for_player_tiles(&changed_tiles);
        self.pending_traffic_redraw = true;
        if self.engine_fidelity.uses_win95() {
            if !dry_stroke {
                pe_powerline_post_place_power_recalc_43f396(
                    &mut self.city,
                    &mut self.pe_process_crt,
                );
            }
            self.refresh_live_misc_current_values();
            self.refresh_budget_service_rows_from_live_counts();
            self.refresh_reward_availability();
        } else if !dry_stroke {
            self.mac_power_tool_post_place_recalc();
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
        plan.span_count.max(2)
    }

    pub(in crate::app) fn place_power_bridge_endpoint(
        &mut self,
        tile: (usize, usize),
        orientation: u8,
    ) {
        let (x, y) = tile;
        self.raise_bridge_ramp(x, y, orientation);
        self.place_power_line_coordinate(tile);
    }

    pub(in crate::app) fn begin_win95_rail_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        direct: bool,
        neighbor_link: Option<RailNeighborLinkCandidate>,
    ) -> bool {
        let plan = if direct {
            pe_rail_bridge_span_plan(&self.city, seed, direction)
        } else {
            water_bridge_span_plan(&self.city, seed, direction, 0x02, false)
        };
        let Some(plan) = plan else {
            return false;
        };
        self.open_city_info_dialog(CityInfoDialog::RailBridge {
            seed_x: seed.0,
            seed_y: seed.1,
            direction: direction & 3,
            span_count: plan.span_count,
            neighbor_link,
        });
        true
    }

    pub(super) fn begin_mac_rail_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        mac_neighbor_link: Option<RailNeighborLinkCandidate>,
    ) -> bool {
        let Some(plan) = water_bridge_span_plan(&self.city, seed, direction, 0x02, false) else {
            return false;
        };
        self.open_city_info_dialog(CityInfoDialog::RailBridge {
            seed_x: seed.0,
            seed_y: seed.1,
            direction,
            span_count: plan.span_count,
            neighbor_link: mac_neighbor_link,
        });
        true
    }

    pub(in crate::app) fn finish_rail_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        expected_span_count: usize,
    ) -> usize {
        let direction = direction & 3;
        let win95 = self.engine_fidelity.uses_win95();
        let Some(plan) = water_bridge_span_plan(&self.city, seed, direction, 0x02, false) else {
            return 0;
        };
        if plan.span_count != expected_span_count || plan.selector_mask & 0x02 == 0 {
            return 0;
        }
        let Some(cost) = pe_rail_bridge_selector_cost().checked_mul(plan.span_count as u32) else {
            return 0;
        };
        if !self.charge_water_bridge(cost) {
            return 0;
        }

        let far_endpoint = if plan.span_count == 1 {
            plan.terminal
        } else {
            plan.cells[plan.span_count - 1]
        };
        self.place_rail_bridge_endpoint(seed, ((direction + 1) & 3) + 1);
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
        self.place_rail_bridge_endpoint(far_endpoint, ((direction + 3) & 3) + 1);

        let mut changed_tiles = plan.cells;
        changed_tiles.push(far_endpoint);
        self.clear_zone_growth_for_player_tiles(&changed_tiles);
        self.pending_traffic_redraw = true;
        if win95 {
            self.refresh_live_misc_current_values();
            self.refresh_budget_service_rows_from_live_counts();
            self.refresh_reward_availability();
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
        plan.span_count.max(2)
    }

    fn place_rail_bridge_endpoint(&mut self, tile: (usize, usize), orientation: u8) {
        let (x, y) = tile;
        self.raise_bridge_ramp(x, y, orientation);
        if !self.engine_fidelity.uses_win95() {
            self.place_mac_rail_coordinate(x, y);
            return;
        }
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
        direct: bool,
    ) -> usize {
        let plan = if direct {
            pe_road_bridge_span_plan(&self.city, seed, direction)
        } else {
            // PE mode 0 retains the stroke direction at 0x45b9f9.
            water_bridge_span_plan(
                &self.city,
                seed,
                direction,
                PE_ROAD_BRIDGE_SELECTOR_MASK,
                false,
            )
        };
        let Some(plan) = plan else {
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
            mac_neighbor_link: None,
        });
        0
    }

    pub(super) fn begin_mac_road_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        mac_neighbor_link: Option<RoadNeighborLinkCandidate>,
    ) -> bool {
        // CODE 13 @ 0x1f5cc uses the land approach direction directly. The
        // direct-water caller has already selected a direction from the mask.
        let Some(plan) = water_bridge_span_plan(
            &self.city,
            seed,
            direction,
            PE_ROAD_BRIDGE_SELECTOR_MASK,
            false,
        ) else {
            return false;
        };
        self.open_city_info_dialog(CityInfoDialog::RoadBridge {
            seed_x: seed.0,
            seed_y: seed.1,
            direction,
            span_count: plan.span_count,
            selector_mask: plan.selector_mask,
            mac_neighbor_link,
        });
        true
    }

    pub(in crate::app) fn begin_highway_water_bridge(
        &mut self,
        candidate: HighwayWaterBridgeCandidate,
    ) {
        self.open_city_info_dialog(CityInfoDialog::HighwayWaterBridge { candidate });
    }

    pub(in crate::app) fn finish_highway_water_bridge(
        &mut self,
        expected: HighwayWaterBridgeCandidate,
        selector: u8,
    ) -> usize {
        let mac = !self.engine_fidelity.uses_win95();
        let preflight = if mac {
            mac_highway_water_bridge_candidate
        } else {
            pe_highway_water_bridge_candidate_45c1e0
        };
        let Some(plan) = preflight(&self.city, expected.seed, expected.mode, expected.direction)
        else {
            return 0;
        };
        let Some(cost) = pe_highway_water_selector_cost_45c1e0(selector)
            .and_then(|unit| unit.checked_mul(plan.span_count as u32))
        else {
            return 0;
        };
        // CODE 13 @ 0x1ffe4 uses an unsigned comparison and has no debit.
        let affordable = if mac {
            self.city.city_funds() as u32 >= cost
        } else {
            self.can_afford_build_cost(cost)
        };
        if plan != expected || plan.selector_mask & (1 << selector) == 0 || !affordable {
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
            let (Some(near_x), Some(near_y), Some(far_endpoint)) = (
                plan.seed.0.checked_add_signed(-dx * 2),
                plan.seed.1.checked_add_signed(-dy * 2),
                origin_at(plan.span_count),
            ) else {
                return 0;
            };
            let seed_height = pe_highway_altitude_463f60(&self.city, plan.seed.0, plan.seed.1);
            let near_height = pe_highway_altitude_463f60(&self.city, near_x, near_y);
            let near_class = if i32::from(seed_height) - i32::from(near_height) == -1 {
                i16::from(plan.direction & 1)
            } else {
                i16::from((plan.direction + 1 & 3) + 4)
            };
            pe_place_highway_tile_for_stamp_462ed0(&mut self.city, near_x, near_y, near_class);
            changed_tiles.extend(highway_segment_tiles((near_x, near_y), NetworkAxis::GridX));

            if mac {
                stamp_highway_water_far_ramp(
                    &mut self.city,
                    far_endpoint,
                    seed_height,
                    plan.direction,
                );
            }
            for index in 0..plan.span_count {
                let Some(origin) = origin_at(index) else {
                    return 0;
                };
                let class = if index & 1 == 0 { 0x0e } else { 0x0d };
                pe_place_highway_tile_for_stamp_462ed0(&mut self.city, origin.0, origin.1, class);
                let tiles = highway_segment_tiles(origin, NetworkAxis::GridX);
                for (x, y) in tiles {
                    // CODE 13 @ 0x20256 and 0x2029c both write the southwest
                    // cell twice, leaving the northeast rotate bit alone.
                    if mac && (x, y) == (origin.0 + 1, origin.1) {
                        continue;
                    }
                    let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
                    self.city
                        .set_tile_bits(x, y, bits.with_rotate(plan.direction & 1 != 0));
                    changed_tiles.push((x, y));
                }
            }

            if !mac {
                stamp_highway_water_far_ramp(
                    &mut self.city,
                    far_endpoint,
                    seed_height,
                    plan.direction,
                );
                changed_tiles.extend(highway_segment_tiles(far_endpoint, NetworkAxis::GridX));
            }
        } else {
            return 0;
        }

        if !mac {
            self.debit_build_cost(cost);
            self.clear_zone_growth_for_player_tiles(&changed_tiles);
            self.refresh_live_misc_current_values();
            self.refresh_budget_service_rows_from_live_counts();
            self.refresh_reward_availability();
        }
        self.pending_traffic_redraw = true;
        self.query_is_inspect = false;
        self.refresh_status_line();
        plan.span_count
    }

    pub(in crate::app) fn finish_road_bridge(
        &mut self,
        seed: (usize, usize),
        direction: u8,
        expected_span_count: usize,
        selector: u8,
    ) -> usize {
        let direction = direction & 3;
        let win95 = self.engine_fidelity.uses_win95();
        let Some(plan) = water_bridge_span_plan(
            &self.city,
            seed,
            direction,
            PE_ROAD_BRIDGE_SELECTOR_MASK,
            false,
        ) else {
            return 0;
        };
        if plan.span_count != expected_span_count
            || !(2..=4).contains(&selector)
            || plan.selector_mask & (1 << selector) == 0
        {
            return 0;
        }
        let cost = pe_road_bridge_selector_cost(selector).saturating_mul(plan.span_count as u32);
        if !self.charge_water_bridge(cost) {
            return 0;
        }

        let far_endpoint = if plan.span_count == 1 {
            plan.terminal
        } else {
            plan.cells[plan.span_count - 1]
        };
        self.place_road_bridge_endpoint(seed, ((direction + 1) & 3) + 1);
        for index in 1..plan.span_count.saturating_sub(1) {
            let (x, y) = plan.cells[index];
            if !win95 && direction & 1 != 0 {
                let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
                self.city.set_tile_bits(x, y, bits.with_rotate(true));
            }
            // Mac suspension segments end before the far anchor padding.
            let tile = if !win95
                && selector == 4
                && index >= plan.span_count - (((plan.span_count - 2) % 5) + 2).div_ceil(2)
            {
                0x57
            } else {
                pe_road_bridge_interior_tile(selector, plan.span_count, index, direction)
            };
            pe_place_tile_441f00(&mut self.city, x, y, tile);
            if win95 && direction & 1 != 0 {
                let bits = self.city.tile_bits_at(x, y).unwrap_or_default();
                self.city.set_tile_bits(x, y, bits.with_rotate(true));
            }
        }
        self.place_road_bridge_endpoint(far_endpoint, ((direction + 3) & 3) + 1);

        let placed = plan.span_count.max(2);
        let mut bridge_cells = plan.cells;
        bridge_cells.push(far_endpoint);
        self.clear_zone_growth_for_player_tiles(&bridge_cells);
        self.pending_traffic_redraw = true;
        if win95 {
            self.refresh_live_misc_current_values();
            self.refresh_budget_service_rows_from_live_counts();
            self.refresh_reward_availability();
        }
        self.query_is_inspect = false;
        self.refresh_status_line();
        placed
    }

    fn place_road_bridge_endpoint(&mut self, tile: (usize, usize), orientation: u8) {
        let (x, y) = tile;
        self.raise_bridge_ramp(x, y, orientation);
        if !self.engine_fidelity.uses_win95() {
            self.place_mac_road_coordinate(x, y);
            return;
        }
        let existing = self.city.building_at(x, y);
        if pe_place_roads_coordinate_should_stamp_44acf4(existing)
            && crossover_network_axis(existing, NetworkKind::Road).is_none()
        {
            let building = pe_place_roads_crossover_from_existing_44ae24(existing)
                .unwrap_or_else(|| NetworkKind::Road.base_tile());
            pe_place_tile_441f00(&mut self.city, x, y, building);
            pe_place_network_clear_zone_type_nibble(&mut self.city, x, y, NetworkKind::Road);
            self.refresh_road_neighbors_like_pe_44acc0(x, y);
        }
    }
    fn charge_water_bridge(&mut self, cost: u32) -> bool {
        let funds = self.city.city_funds();
        if self.engine_fidelity.uses_win95() {
            if !self.can_afford_build_cost(cost) {
                return false;
            }
            self.debit_build_cost(cost);
        } else {
            // CODE 13 @ 0x1f724 compares the funds DWORD as unsigned.
            if cost > funds as u32 {
                return false;
            }
            self.city.set_city_funds(funds.wrapping_sub(cost as i32));
        }
        true
    }

    fn raise_bridge_ramp(&mut self, x: usize, y: usize, orientation: u8) {
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
    }
}
