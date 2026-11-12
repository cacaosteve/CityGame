use super::*;
use crate::app::building_placement::clipped_building_footprint_from_origin;

mod construction;
mod publication;
mod special_zones;

fn surface_transport_access_tile(building: u8) -> bool {
    // SC2K zones grow with road, highway, rail, or bus-depot adjacency.
    is_road_tile(building) || is_highway_tile(building) || is_rail_tile(building) || building == 236
}

/// PE `FUN_00437f90` @ `0x437fb0`–`0x438013`: map `XZON & 0xf0` (+ view rot `0x4c942c`)
/// to an SE-tile adjust before the four mode-4 stamps.
fn pe_437f90_adjusted_se(city: &City, hit_x: usize, hit_y: usize) -> Option<(usize, usize)> {
    let corners = city
        .zone_at(hit_x, hit_y)
        .map(|zone| zone.raw_bits() & 0xf0)
        .unwrap_or(0);
    let rot = city.rotation() & 3;
    let case = match corners {
        0x10 => 0u8,
        0x20 => 1,
        0x40 => 2,
        0x80 => 3,
        _ => 4,
    };
    let code = match case {
        0 => (0u8.wrapping_sub(rot)) & 3,
        1 => (1u8.wrapping_sub(rot)) & 3,
        2 => (0u8.wrapping_sub(2).wrapping_sub(rot)) & 3,
        3 => (0u8.wrapping_sub(1).wrapping_sub(rot)) & 3,
        _ => 0xff,
    };
    let mut x = hit_x as isize;
    let mut y = hit_y as isize;
    match code {
        0 => y += 1,
        1 => {
            y += 1;
            x -= 1;
        }
        2 => x -= 1,
        _ => {}
    }
    if !(0..CITY_WIDTH as isize).contains(&x) || !(0..CITY_HEIGHT as isize).contains(&y) {
        return None;
    }
    Some((x as usize, y as usize))
}

// ViewerState RCI simulation and development pipeline.
impl ViewerState {
    pub(in crate::app) fn decline_unserved_rci_zones(&mut self) -> usize {
        let mut declined_tiles = 0;
        for (step, substep) in sim_calendar_growth_phases() {
            let mut reserved = vec![false; CITY_TILES];
            declined_tiles +=
                self.decline_unserved_rci_zones_for_phase(step, substep, &mut reserved);
        }
        declined_tiles
    }

    pub(in crate::app) fn decline_unserved_rci_zones_for_phase(
        &mut self,
        step: u16,
        substep: u16,
        reserved: &mut [bool],
    ) -> usize {
        // Same process-CRT plant network as growth-tick powered decline gates.
        let power_maps = power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let mut declined_tiles = 0;
        let corner_bits = pe_choose_and_place_corner_bits(self.city.rotation());
        // Decline ChangeBuilding CRT shares process `0x48cdd8` with the growth-tick stream.
        let mut crt = self.pe_process_crt;
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            declined_tiles += self.decline_unserved_rci_zone_at(
                x,
                y,
                &power_maps,
                reserved,
                corner_bits,
                &mut crt,
            );
        }
        self.pe_process_crt = crt;
        if declined_tiles > 0 {
            self.city.mark_all_dirty();
        }
        declined_tiles
    }

    pub(in crate::app) fn pe_rci_growth_roll_succeeds_mut(
        &mut self,
        threshold: u32,
        x: usize,
        y: usize,
        table_key: u8,
    ) -> bool {
        pe_rci_growth_roll_succeeds_optional(
            threshold,
            x,
            y,
            table_key,
            Some(&mut self.pe_process_crt),
        )
    }

    pub(in crate::app) fn pe_rci_growth_roll_succeeds_with_crt(
        threshold: u32,
        x: usize,
        y: usize,
        table_key: u8,
        crt: &mut Win95CrtRand,
    ) -> bool {
        pe_rci_growth_roll_succeeds_optional(threshold, x, y, table_key, Some(crt))
    }

    pub(in crate::app) fn decline_unserved_rci_zone_at(
        &mut self,
        x: usize,
        y: usize,
        power_maps: &PowerServiceMaps,
        reserved: &mut [bool],
        corner_bits: [u8; 4],
        crt: &mut Win95CrtRand,
    ) -> usize {
        let building = self.city.building_at(x, y);
        let Some(kind) = rci_building_kind(building) else {
            return 0;
        };
        let Some(zone_id) = self.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
            return 0;
        };
        if rci_kind_for_zone(zone_id) != Some(kind) {
            return 0;
        }
        let Some(pop) = rci_building_pop_level(building) else {
            return 0;
        };
        let size = usize::from(xbld_tile_size(building).max(1));
        // Dense footprints can remain after rezoning to light; Win95 ChangeBuilding
        // still abandons by pop-level/size and does not require the growth allow-table.
        let Some((origin_x, origin_y)) =
            rci_view_corner_footprint_origin(&self.city, x, y, building, size)
        else {
            return 0;
        };
        if !complete_building_footprint_at_origin(&self.city, origin_x, origin_y, building, size) {
            return 0;
        }
        let Some(footprint) = building_footprint_from_origin(origin_x, origin_y, size) else {
            return 0;
        };
        if !footprint
            .iter()
            .all(|&(tx, ty)| self.zone_growth_can_touch_tile(tx, ty))
        {
            return 0;
        }
        if !footprint.iter().all(|&(tx, ty)| {
            self.city
                .zone_at(tx, ty)
                .and_then(|zone| rci_kind_for_zone(zone.zone_id()))
                == Some(kind)
        }) {
            return 0;
        }
        let remainder = rci_growth_remainder_demand(
            rci_demand_for_kind(&self.city, kind),
            power_water::growth_powered_cross_for_engine(power_maps, x, y)
                && self.tile_has_growth_trip_access(x, y),
        );
        let Some(replace_tile) = pe_rci_growth_change_roll_then_replace_optional(
            rci_growth_change_threshold(remainder, pop),
            x,
            y,
            GROWTH_ROLL_CHANGE_KEY,
            Some(&mut *crt),
        ) else {
            return 0;
        };
        let Some(placements) = rci_decline_change_building_at_call_optional(
            building,
            &footprint,
            x,
            y,
            origin_x,
            origin_y,
            replace_tile,
            Some(crt),
        ) else {
            return 0;
        };
        let pop4_scatter_overlay = replace_tile && rci_building_pop_level(building) == Some(4);
        let mut stamped = vec![false; CITY_TILES];
        for &(tx, ty, next_building) in &placements {
            let index = ty * CITY_WIDTH + tx;
            let size = usize::from(xbld_tile_size(next_building).max(1));
            // ChangeBuilding pop-4 runs the pop-3 PlaceTile after its eight ring calls;
            // that final 2×2 stamp deliberately overwrites ring tiles it covers.
            if stamped[index] && !(pop4_scatter_overlay && size == 2) {
                continue;
            }
            let Some(tiles) = building_footprint_from_origin(tx, ty, size) else {
                continue;
            };
            let pop = rci_building_pop_level(next_building).unwrap_or(1);
            if !choose_and_place_nw_origin_in_bounds(tx, ty, pop) {
                continue;
            }
            let zone_id = self.city.zone_at(tx, ty).map_or(0, |zone| zone.zone_id());
            for &(fx, fy) in &tiles {
                stamped[fy * CITY_WIDTH + fx] = true;
                reserved[fy * CITY_WIDTH + fx] = true;
                // PE ChangeBuilding → ChooseAndPlace → PlaceTile `0x441f00`.
                pe_place_tile_441f00(&mut self.city, fx, fy, next_building);
                let zone = zone_id | building_zone_bits_for_tile(tx, ty, fx, fy, size, corner_bits);
                self.city.set_zone(fx, fy, zone);
                self.or_choose_and_place_xbit(fx, fy);
            }
        }
        for &(fx, fy) in &footprint {
            reserved[fy * CITY_WIDTH + fx] = true;
        }
        footprint.len()
    }

    pub(in crate::app) fn develop_rci_zones(&mut self) -> usize {
        self.develop_rci_zones_with_outcomes(city_service_outcomes_for_city_with_fidelity(
            &self.city,
            self.engine_fidelity,
        ))
    }

    #[cfg(test)]
    pub(in crate::app) fn develop_special_zones(&mut self) -> usize {
        let mut reserved = vec![false; CITY_TILES];
        let mut developed = 0;
        // Match the live growth-tick path: special-zone rolls share `pe_process_crt`.
        let mut rng = self.pe_process_crt;
        for (step, substep) in sim_calendar_growth_phases() {
            for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
                if reserved[y * CITY_WIDTH + x] {
                    continue;
                }
                let Some(zone_id) = self.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
                    continue;
                };
                if zone_id <= ZONE_INDUSTRIAL_DENSE
                    || special_zone_kind_index(zone_id).is_none()
                    || !self.special_zone_growth_roll_allows(&mut rng, zone_id)
                {
                    continue;
                }
                let power_maps =
                    power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
                let Some(plan) =
                    self.special_zone_development_plan_at(x, y, &reserved, &power_maps)
                else {
                    continue;
                };
                if plan
                    .footprint
                    .iter()
                    .any(|&(tx, ty)| reserved[ty * CITY_WIDTH + tx])
                {
                    continue;
                }
                for &(tx, ty) in &plan.footprint {
                    reserved[ty * CITY_WIDTH + tx] = true;
                }
                developed += plan.footprint.len();
                self.stamp_zone_development_with_crt(&plan, &mut rng);
            }
        }
        self.pe_process_crt = rng;
        developed
    }

    pub(in crate::app) fn develop_rci_zones_with_outcomes(
        &mut self,
        city_outcomes: CityServiceOutcomes,
    ) -> usize {
        let mut reserved = vec![false; CITY_TILES];
        let mut developed = 0;
        for (step, substep) in sim_calendar_growth_phases() {
            developed +=
                self.develop_rci_zones_for_phase(step, substep, city_outcomes, &mut reserved);
        }
        developed
    }

    pub(in crate::app) fn develop_rci_zones_for_phase(
        &mut self,
        step: u16,
        substep: u16,
        city_outcomes: CityServiceOutcomes,
        reserved: &mut [bool],
    ) -> usize {
        let power_maps = power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let development_services =
            rci_development_services_for_city_with_power(&self.city, &power_maps);
        // StartBuilding / abandon-redevelop / size-up growth rolls share process CRT.
        let mut growth_rng = self.pe_process_crt;
        let mut developed = 0;
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            if reserved[y * CITY_WIDTH + x] {
                continue;
            }
            let Some(zone_id) = self.city.zone_at(x, y).map(|zone| zone.zone_id()) else {
                continue;
            };
            if rci_kind_for_zone(zone_id).is_none() {
                continue;
            }
            let Some(candidate) = self
                .rci_development_candidate_at(
                    x,
                    y,
                    reserved,
                    &power_maps,
                    city_outcomes,
                    &development_services,
                    Some(&mut growth_rng),
                )
                .or_else(|| {
                    self.rci_expansion_redevelopment_candidate_at(
                        x,
                        y,
                        reserved,
                        &power_maps,
                        city_outcomes,
                        &development_services,
                        Some(&mut growth_rng),
                    )
                })
            else {
                continue;
            };
            if candidate
                .plan
                .footprint
                .iter()
                .any(|&(tx, ty)| reserved[ty * CITY_WIDTH + tx])
            {
                continue;
            }
            for &(tx, ty) in &candidate.plan.footprint {
                reserved[ty * CITY_WIDTH + tx] = true;
            }
            developed += candidate.plan.footprint.len();
            self.stamp_zone_development_with_crt(&candidate.plan, &mut growth_rng);
        }
        self.pe_process_crt = growth_rng;
        if developed > 0 {
            self.city.mark_all_dirty();
        }
        developed
    }

    pub(in crate::app) fn rci_development_candidate_at(
        &self,
        x: usize,
        y: usize,
        reserved: &[bool],
        power_maps: &PowerServiceMaps,
        city_outcomes: CityServiceOutcomes,
        development_services: &RciDevelopmentServices,
        process_crt: Option<&mut Win95CrtRand>,
    ) -> Option<RciDevelopmentCandidate> {
        self.rci_development_candidate_at_with_pre_roll(
            x,
            y,
            reserved,
            power_maps,
            city_outcomes,
            development_services,
            process_crt,
        )
    }

    pub(in crate::app) fn rci_development_candidate_at_with_pre_roll(
        &self,
        x: usize,
        y: usize,
        reserved: &[bool],
        power_maps: &PowerServiceMaps,
        city_outcomes: CityServiceOutcomes,
        development_services: &RciDevelopmentServices,
        mut process_crt: Option<&mut Win95CrtRand>,
    ) -> Option<RciDevelopmentCandidate> {
        let zone_id = self.city.zone_at(x, y)?.zone_id();
        if !rci_zone_id(zone_id) {
            return None;
        }
        let origin_building = self.city.building_at(x, y);
        let trip_succeeded = self.tile_has_growth_trip_access(x, y);
        if self.engine_fidelity.uses_win95() && is_abandoned_building(origin_building) {
            // BUILD_ABANDON goes straight from the threshold comparison to
            // ChooseAndPlace, passing the raw scanned corner at 0x43620f.
            let pop = rci_building_pop_level(origin_building)?;
            if !xzon_corner_check(&self.city, x, y) || reserved[y * CITY_WIDTH + x] {
                return None;
            }
            let threshold = if trip_succeeded
                && rci_zone_has_saved_demand(&self.city, zone_id)
                && power_water::growth_powered_cross_for_engine(power_maps, x, y)
            {
                let demand = rci_demand_for_kind(&self.city, rci_kind_for_zone(zone_id)?);
                rci_abandoned_redevelop_threshold(demand, pop)
            } else {
                0
            };
            if !pe_rci_abandoned_redevelop_roll_succeeds_optional(
                threshold,
                x,
                y,
                GROWTH_ROLL_ABANDON_REDEVELOP_KEY,
                process_crt.as_deref_mut(),
            ) {
                return None;
            }
            let origin = (x, y.saturating_sub(choose_and_place_half_pop(pop)));
            let size = pop_level_footprint_size(pop);
            let footprint = clipped_building_footprint_from_origin(origin.0, origin.1, size);
            let quality_score = rci_development_quality_score_for_footprint(
                &self.city,
                zone_id,
                &footprint,
                city_outcomes,
                development_services,
            );
            let quality_band = rci_development_quality_band_for_score(quality_score);
            let building = rci_choose_and_place_for_city_optional(
                &self.city,
                zone_id,
                pop,
                x,
                y,
                quality_band,
                process_crt,
            )?;
            // The picker runs even when the raw call will fail the bounds check.
            if !choose_and_place_multitile_call_in_bounds(x, y, pop) {
                return None;
            }
            return Some(RciDevelopmentCandidate {
                plan: ZoneDevelopmentPlan {
                    origin,
                    zone_id,
                    building,
                    footprint,
                    place_as_construction: false,
                },
            });
        }
        if !trip_succeeded {
            if self.engine_fidelity.uses_win95() {
                if rci_tile_is_vacant_growth_surface(origin_building)
                    && game_is_valid_transit_items(&self.city, x, y)
                {
                    // 0x43603e admits this vacancy before the power/route check.
                    // StartBuilding at 0x4362b2 draws even with current demand zero.
                    pe_rci_startbuilding_growth_roll_succeeds_optional(
                        0,
                        x,
                        y,
                        GROWTH_ROLL_START_VACANT_KEY,
                        process_crt,
                    );
                }
            }
            return None;
        }
        if !rci_zone_has_saved_demand(&self.city, zone_id) || !self.can_develop_zone_tile(x, y) {
            return None;
        }

        // BUILD_ABANDON → ChooseAndPlace at the abandoned pop level (no construction).
        if is_abandoned_building(origin_building) {
            let pop = rci_building_pop_level(origin_building)?;
            let demand = rci_demand_for_kind(&self.city, rci_kind_for_zone(zone_id)?);
            if !pe_rci_abandoned_redevelop_roll_succeeds_optional(
                rci_abandoned_redevelop_threshold(demand, pop),
                x,
                y,
                GROWTH_ROLL_ABANDON_REDEVELOP_KEY,
                process_crt.as_deref_mut(),
            ) {
                return None;
            }
            let size = pop_level_footprint_size(pop);
            let (origin_x, origin_y) =
                rci_view_corner_footprint_origin(&self.city, x, y, origin_building, size)?;
            let footprint = building_footprint_from_origin(origin_x, origin_y, size)?;
            if !complete_building_footprint_at_origin(
                &self.city,
                origin_x,
                origin_y,
                origin_building,
                size,
            ) {
                return None;
            }
            if footprint.iter().all(|&(tx, ty)| {
                !reserved[ty * CITY_WIDTH + tx]
                    && self
                        .city
                        .zone_at(tx, ty)
                        .is_some_and(|zone| zone.zone_id() == zone_id)
                    && self.city.building_at(tx, ty) == origin_building
            }) && self.development_footprint_is_flat_land(&footprint)
                && self.development_footprint_has_transport_access(&footprint)
                // Both editions check saved powered XBIT at the visited lattice tile.
                && power_water::growth_powered_cross_for_engine(power_maps, x, y)
            {
                let quality_score = rci_development_quality_score_for_footprint(
                    &self.city,
                    zone_id,
                    &footprint,
                    city_outcomes,
                    development_services,
                );
                let quality_band = rci_development_quality_band_for_score(quality_score);
                let building = pe_choose_and_place_then_origin_bounds(
                    || {
                        rci_choose_and_place_for_city_optional(
                            &self.city,
                            zone_id,
                            pop,
                            x,
                            y,
                            quality_band,
                            process_crt.as_deref_mut(),
                        )
                    },
                    origin_x,
                    origin_y,
                    pop,
                )?;
                return Some(RciDevelopmentCandidate {
                    plan: ZoneDevelopmentPlan {
                        origin: (origin_x, origin_y),
                        zone_id,
                        building,
                        footprint,
                        place_as_construction: false,
                    },
                });
            }
            return None;
        }

        let demand = rci_demand_for_kind(&self.city, rci_kind_for_zone(zone_id)?);
        // Vacant StartBuilding uses pop NONE (=0) in the demand threshold.
        if !pe_rci_startbuilding_growth_roll_succeeds_optional(
            rci_growth_start_threshold(demand, 0),
            x,
            y,
            GROWTH_ROLL_START_VACANT_KEY,
            process_crt.as_deref_mut(),
        ) {
            return None;
        }

        for &size in rci_development_candidate_sizes(zone_id) {
            let Some(footprint) = building_footprint_from_origin(x, y, size) else {
                continue;
            };
            if footprint.iter().all(|&(tx, ty)| {
                !reserved[ty * CITY_WIDTH + tx]
                    && self
                        .city
                        .zone_at(tx, ty)
                        .is_some_and(|zone| zone.zone_id() == zone_id)
                    && self.can_develop_zone_tile(tx, ty)
            }) && self.development_footprint_is_flat_land(&footprint)
                && game_is_valid_transit_items(&self.city, x, y)
                // Both editions check saved powered XBIT at the visited lattice tile.
                && power_water::growth_powered_cross_for_engine(power_maps, x, y)
            {
                let quality_score = rci_development_quality_score_for_footprint(
                    &self.city,
                    zone_id,
                    &footprint,
                    city_outcomes,
                    development_services,
                );
                let quality_band = rci_development_quality_band_for_score(quality_score);
                let pop = size_to_default_pop_level(size)?;
                let building = rci_choose_and_place_for_city_optional(
                    &self.city,
                    zone_id,
                    pop,
                    x,
                    y,
                    quality_band,
                    process_crt.as_deref_mut(),
                )?;
                return Some(RciDevelopmentCandidate {
                    plan: ZoneDevelopmentPlan {
                        origin: (x, y),
                        zone_id,
                        building,
                        footprint,
                        // Vacant pop-NONE StartBuilding always stamps construction.
                        // Abandoned already returned via ChooseAndPlace above.
                        place_as_construction: true,
                    },
                });
            }
        }

        None
    }

    pub(in crate::app) fn rci_expansion_redevelopment_candidate_at(
        &mut self,
        x: usize,
        y: usize,
        reserved: &[bool],
        power_maps: &PowerServiceMaps,
        city_outcomes: CityServiceOutcomes,
        development_services: &RciDevelopmentServices,
        process_crt: Option<&mut Win95CrtRand>,
    ) -> Option<RciDevelopmentCandidate> {
        self.rci_expansion_redevelopment_candidate_at_with_pre_roll(
            x,
            y,
            reserved,
            power_maps,
            city_outcomes,
            development_services,
            process_crt,
        )
    }

    pub(in crate::app) fn rci_expansion_redevelopment_candidate_at_with_pre_roll(
        &mut self,
        x: usize,
        y: usize,
        reserved: &[bool],
        power_maps: &PowerServiceMaps,
        city_outcomes: CityServiceOutcomes,
        development_services: &RciDevelopmentServices,
        mut process_crt: Option<&mut Win95CrtRand>,
    ) -> Option<RciDevelopmentCandidate> {
        let zone_id = self.city.zone_at(x, y)?.zone_id();
        let kind = rci_kind_for_zone(zone_id)?;
        // Light zones only StartBuilding from empty (pop NONE); dense can size-up.
        let trip_succeeded = self.tile_has_growth_trip_access(x, y);
        if !zone_is_dense_rci(zone_id)
            || (trip_succeeded && !rci_zone_has_saved_demand(&self.city, zone_id))
            || (!trip_succeeded && !self.engine_fidelity.uses_win95())
            || !rci_redevelopment_origin_can_start_at(&self.city, x, y, zone_id, kind)
        {
            return None;
        }
        let existing = self.city.building_at(x, y);
        let current_pop = rci_building_pop_level(existing)?;
        let demand = rci_demand_for_kind(&self.city, kind);
        // PE `FUN_004358B0` rejects pop 4 / low-XVAL R/C at `0x0043621C..0x0043629B`,
        // before `rand()` at `0x004362B2`.
        let next_pop =
            pe_startbuilding_pre_roll_target_pop(&self.city, x, y, zone_id, current_pop)?;
        if !trip_succeeded && !xzon_corner_check(&self.city, x, y) {
            return None;
        }
        let threshold = if trip_succeeded {
            rci_growth_start_threshold(demand, current_pop)
        } else {
            0
        };
        if !pe_rci_startbuilding_growth_roll_succeeds_optional(
            threshold,
            x,
            y,
            GROWTH_ROLL_START_EXISTING_KEY,
            process_crt.as_deref_mut(),
        ) {
            return None;
        }
        let current_size = pop_level_footprint_size(current_pop);
        let target_size = pop_level_footprint_size(next_pop);
        let (footprint_origin_x, footprint_origin_y) = if current_size <= 1 {
            (x, y)
        } else {
            rci_view_corner_footprint_origin(&self.city, x, y, existing, current_size)?
        };
        if !complete_building_footprint_at_origin(
            &self.city,
            footprint_origin_x,
            footprint_origin_y,
            existing,
            current_size,
        ) {
            return None;
        }
        // The recovered PE and Mac helpers gate the visited lattice tile before
        // the growth roll; neither tests each proposed footprint.
        if matches!(current_pop, 1 | 2 | 3)
            && !power_water::growth_powered_cross_for_engine(power_maps, x, y)
        {
            return None;
        }
        // StartBuilding current-pop-2 calls ChooseAndPlace(pop=3) directly. Keep
        // its picker draw even when an edge call cannot form a bounded 2×2 stamp;
        // the empty-origin case at call_y=0 would otherwise skip the draw entirely.
        if current_pop == 2 && !choose_and_place_multitile_call_in_bounds(x, y, next_pop) {
            let _ = pe_startbuilding_pop3_direct_placement_with_picker(x, y, || {
                construction_building_for_pop_optional(next_pop, x, y, process_crt.as_deref_mut())
            });
            return None;
        }
        // Current-pop-2 StartBuilding (`0x004377D2`) calls mode-3/pop-3
        // ChooseAndPlace directly after the shared pre-roll gates. Unlike the
        // pop-1/pop-3 branches, PE does not run footprint terrain, transport,
        // CheckTileArea, or service-quality filters here.
        if current_pop == 2 {
            let (stamp_x, stamp_y, building) =
                pe_startbuilding_pop3_direct_placement_with_picker(x, y, || {
                    construction_building_for_pop_optional(
                        next_pop,
                        x,
                        y,
                        process_crt.as_deref_mut(),
                    )
                })?;
            let footprint = building_footprint_from_origin(stamp_x, stamp_y, target_size)?;
            if footprint
                .iter()
                .any(|&(tx, ty)| reserved[ty * CITY_WIDTH + tx])
            {
                return None;
            }
            return Some(RciDevelopmentCandidate {
                plan: ZoneDevelopmentPlan {
                    origin: (stamp_x, stamp_y),
                    zone_id,
                    building,
                    footprint,
                    place_as_construction: true,
                },
            });
        }
        let altitude = self.city.altitude_at(x, y);
        let origins: Vec<(usize, usize)> = match current_pop {
            1 => pop1_startbuilding_quadrant_origins(x, y).collect(),
            2 => pop2_choose_and_place_stamp_origin(x, y)
                .into_iter()
                .collect(),
            3 => {
                // PE StartBuilding(=3) enumerates from its raw scan/view-corner call tile;
                // ChooseAndPlace converts each call tile to a 3×3 stamp origin.
                pop3_startbuilding_quadrant_origins(x, y).collect()
            }
            _ => vec![(footprint_origin_x, footprint_origin_y)],
        };
        for (ox, oy) in origins {
            let Some(footprint) = building_footprint_from_origin(ox, oy, target_size) else {
                continue;
            };
            if current_pop != 3
                && !self.rci_redevelopment_footprint_allows_start(
                    &footprint,
                    zone_id,
                    reserved,
                    current_pop == 1,
                )
            {
                // PE pop-1 CheckTileArea ignores XTER; let water-class terrain reach that probe.
                continue;
            }
            // Pop1 uses max 0x8C; pop3→4 uses max 0xAE. Pop2→3 stays in place.
            if let Some(max_building) = check_tile_area_max_xbld_for_start_pop(current_pop) {
                if !footprint.iter().all(|&(tx, ty)| {
                    check_tile_area(&self.city, tx, ty, altitude, zone_id, max_building)
                }) {
                    continue;
                }
            }
            // Pop3's PE order is CheckTileArea, then road orientation. These
            // simulation-only restrictions must not hide either PE probe.
            if current_pop == 3 {
                if !pop3_startbuilding_has_road_orientation(&self.city, ox, oy) {
                    continue;
                }
                // PE proceeds from the road match to perimeter DestroyBuilding and
                // ChooseAndPlace. Keep only simulation reservation/region constraints;
                // XTER, terrain flatness, transport, power, and quality are not gates here.
                if !self
                    .rci_redevelopment_footprint_allows_start(&footprint, zone_id, reserved, true)
                {
                    continue;
                }
                let (call_x, call_y) = pop3_startbuilding_candidate_call_from_origin(ox, oy)?;
                // PE `0x437a88` walks the perimeter and runs FUN_00437f90 before
                // ChooseAndPlace selects the pop-4 construction tile. Both paths
                // share the process CRT, so keep the destruction draws ahead of
                // the picker draw. Candidate-only probes without a live CRT stay
                // observational and do not mutate the city.
                if let Some(crt) = process_crt.as_deref_mut() {
                    self.destroy_pop3_size_up_xbld_at_least_0x8c_with_crt((ox, oy), crt);
                }
                let Some((stamp_x, stamp_y, building)) =
                    pe_startbuilding_pop4_direct_placement_with_picker(call_x, call_y, || {
                        construction_building_for_pop_optional(
                            next_pop,
                            call_x,
                            call_y,
                            process_crt.as_deref_mut(),
                        )
                    })
                else {
                    return None;
                };
                return Some(RciDevelopmentCandidate {
                    plan: ZoneDevelopmentPlan {
                        origin: (stamp_x, stamp_y),
                        zone_id,
                        building,
                        footprint,
                        place_as_construction: true,
                    },
                });
            }
            // PE StartBuilding(pop 1) immediately calls ChooseAndPlace and returns
            // when its CheckTileArea mask succeeds; it does not apply the later
            // per-footprint terrain, transport, or power heuristics.
            if current_pop == 1 {
                let (call_x, call_y) = pop1_startbuilding_candidate_call_from_origin(ox, oy)?;
                // This is already PE's mode-3 ChooseAndPlace result. Keep the
                // selected construction building in the plan so stamping does
                // not consume a second picker draw.
                let Some((stamp_x, stamp_y, building)) =
                    pe_startbuilding_pop2_quadrant_placement_with_picker(call_x, call_y, || {
                        construction_building_for_pop_optional(
                            next_pop,
                            call_x,
                            call_y,
                            process_crt.as_deref_mut(),
                        )
                    })
                else {
                    return None;
                };
                return Some(RciDevelopmentCandidate {
                    plan: ZoneDevelopmentPlan {
                        origin: (stamp_x, stamp_y),
                        zone_id,
                        building,
                        footprint,
                        place_as_construction: false,
                    },
                });
            }
            if !self.development_footprint_is_flat_land(&footprint)
                || !self.development_footprint_has_transport_access(&footprint)
                // Both editions check saved powered XBIT at the visited lattice tile.
                || !power_water::growth_powered_cross_for_engine(power_maps, x, y)
            {
                continue;
            }
            let quality_score = rci_development_quality_score_for_footprint(
                &self.city,
                zone_id,
                &footprint,
                city_outcomes,
                development_services,
            );
            let target_quality_band = rci_development_quality_band_for_score(quality_score);
            // First CheckTileArea-equivalent success: PE calls ChooseAndPlace once and returns.
            // For pop3→4, each candidate origin maps back to its own raw call tile; the PE
            // picker consumes its draw before ChooseAndPlace rejects an out-of-range call.
            let (plan_origin, building) = if current_pop == 2 {
                let Some((stamp_x, stamp_y, building)) =
                    pe_startbuilding_pop3_direct_placement_with_picker(x, y, || {
                        construction_building_for_pop_optional(
                            next_pop,
                            x,
                            y,
                            process_crt.as_deref_mut(),
                        )
                    })
                else {
                    return None;
                };
                ((stamp_x, stamp_y), building)
            } else {
                if !choose_and_place_nw_origin_in_bounds(ox, oy, next_pop) {
                    return None;
                }
                let building = rci_choose_and_place_for_city_optional(
                    &self.city,
                    zone_id,
                    next_pop,
                    x,
                    y,
                    target_quality_band,
                    process_crt.as_deref_mut(),
                )?;
                ((ox, oy), building)
            };
            return Some(RciDevelopmentCandidate {
                plan: ZoneDevelopmentPlan {
                    origin: plan_origin,
                    zone_id,
                    building,
                    footprint,
                    place_as_construction: true,
                },
            });
        }
        None
    }

    /// Win95 pop3 StartBuilding @ `0x437a88`: on XBLD `>= 0x8C` call `0x402180` →
    /// `FUN_00437f90` (ChooseAndPlace mode-4 abandon 2×2), not DestroyStructure.
    pub(in crate::app) fn destroy_pop3_size_up_xbld_at_least_0x8c_with_crt(
        &mut self,
        origin: (usize, usize),
        crt: &mut Win95CrtRand,
    ) {
        let probes: Vec<(usize, usize)> =
            pop3_startbuilding_destroy_coords(origin.0, origin.1).collect();
        for (x, y) in probes {
            if self.city.building_at(x, y) >= CHECK_TILE_AREA_POP1_MAX_XBLD {
                self.pe_fun_00437f90_abandon_2x2_at_with_crt(x, y, crt);
            }
        }
    }

    /// PE `FUN_00437f90` (`0x437f90` via `0x402180`): corner/rotation adjust, then four
    /// `ChooseAndPlace(pop=1, mode=4)` stamps on the 2×2 whose SE is the adjusted tile.
    pub(in crate::app) fn pe_fun_00437f90_abandon_2x2_at_with_crt(
        &mut self,
        hit_x: usize,
        hit_y: usize,
        crt: &mut Win95CrtRand,
    ) {
        let Some((se_x, se_y)) = pe_437f90_adjusted_se(&self.city, hit_x, hit_y) else {
            return;
        };
        if se_x + 1 >= CITY_WIDTH || se_y == 0 {
            return;
        }
        let tiles = [
            (se_x, se_y),
            (se_x + 1, se_y),
            (se_x + 1, se_y - 1),
            (se_x, se_y - 1),
        ];
        for (tx, ty) in tiles {
            self.pe_choose_and_place_abandon_pop1_at_with_crt(tx, ty, crt);
        }
    }

    /// PE ChooseAndPlace mode 4, pop 1: PlaceTile abandon + XZON corners + XBIT `|= 0xe0`.
    pub(in crate::app) fn pe_choose_and_place_abandon_pop1_at_with_crt(
        &mut self,
        x: usize,
        y: usize,
        crt: &mut Win95CrtRand,
    ) {
        let Some(abandoned) = abandoned_building_for_pop_optional(1, x, y, Some(crt)) else {
            return;
        };
        pe_place_tile_441f00(&mut self.city, x, y, abandoned);
        let zone_id = self.city.zone_at(x, y).map_or(0, |zone| zone.zone_id());
        // Size-1 ChooseAndPlace epilogue ORs `ZONE_CORNER_ALL`.
        self.city.set_zone(x, y, zone_id | ZONE_CORNER_ALL);
        self.or_choose_and_place_xbit(x, y);
    }

    /// PE DestroyStructure (`0x401604` → `0x402496` / `0x461330`): clear XBLD + XTXT on the
    /// PE destroy footprint; leave zone corners, watered/piped, and XTRF alone.
    /// Elevated `0x3f..=0x42` early-out @ `0x46137b` — return false (no DestroyBuilding).
    pub(in crate::app) fn pe_destroy_structure_at(&mut self, x: usize, y: usize) -> bool {
        let building = self.city.building_at(x, y);
        if building == 0 || matches!(building, 0x3f..=0x42) {
            return false;
        }
        pe_flood_destroy_tile_with_crt(&mut self.city, &mut self.pe_process_crt, x, y);
        true
    }

    pub(in crate::app) fn rci_redevelopment_footprint_allows_start(
        &self,
        footprint: &[(usize, usize)],
        zone_id: u8,
        reserved: &[bool],
        allow_water_xter: bool,
    ) -> bool {
        for &(tx, ty) in footprint {
            if reserved[ty * CITY_WIDTH + tx]
                || !self.zone_growth_can_touch_tile(tx, ty)
                || (!allow_water_xter && is_water_terrain(self.city.terrain_at(tx, ty)))
                || !self
                    .city
                    .zone_at(tx, ty)
                    .is_some_and(|zone| zone.zone_id() == zone_id)
            {
                return false;
            }
        }
        true
    }

    /// Test helper: StartBuilding + same-tick ChooseAndPlace completion.
    pub(in crate::app) fn develop_rci_until_built_for_test(&mut self) -> usize {
        let started = self.develop_rci_zones();
        let finished = self.complete_rci_construction_sites();
        started.max(finished)
    }

    pub(in crate::app) fn can_develop_zone_tile(&self, x: usize, y: usize) -> bool {
        if !self.zone_growth_can_touch_tile(x, y) {
            return false;
        }
        if is_water_terrain(self.city.terrain_at(x, y)) {
            return false;
        }
        let building = self.city.building_at(x, y);
        rci_tile_is_vacant_growth_surface(building) || is_abandoned_building(building)
    }

    pub(in crate::app) fn development_footprint_is_flat_land(
        &self,
        footprint: &[(usize, usize)],
    ) -> bool {
        let Some(&(origin_x, origin_y)) = footprint.first() else {
            return false;
        };
        let height = self.city.altitude_at(origin_x, origin_y);
        footprint
            .iter()
            .all(|&(x, y)| self.city.terrain_at(x, y) == 0 && self.city.altitude_at(x, y) == height)
    }

    pub(in crate::app) fn development_footprint_has_transport_access(
        &self,
        footprint: &[(usize, usize)],
    ) -> bool {
        footprint.iter().any(|&(x, y)| {
            directional_neighbor_tiles(x, y)
                .any(|(_, nx, ny)| surface_transport_access_tile(self.city.building_at(nx, ny)))
        })
    }
}
