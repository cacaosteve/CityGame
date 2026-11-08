use super::*;

impl ViewerState {
    pub(in crate::app) fn complete_rci_construction_sites(&mut self) -> usize {
        self.complete_rci_construction_sites_with_outcomes(city_service_outcomes_for_city(
            &self.city,
        ))
    }

    pub(in crate::app) fn complete_rci_construction_sites_with_outcomes(
        &mut self,
        city_outcomes: CityServiceOutcomes,
    ) -> usize {
        let mut completed = 0;
        let mut reserved = vec![false; CITY_TILES];
        for (step, substep) in sim_calendar_growth_phases() {
            completed += self.complete_rci_construction_sites_for_phase(
                step,
                substep,
                city_outcomes,
                &mut reserved,
            );
        }
        completed
    }

    pub(in crate::app) fn complete_rci_construction_site_at(
        &mut self,
        x: usize,
        y: usize,
        city_outcomes: CityServiceOutcomes,
        development_services: &RciDevelopmentServices,
        place_church_tick: bool,
        reserved: &mut [bool],
        crt: &mut Win95CrtRand,
    ) -> Option<ZoneDevelopmentPlan> {
        if reserved[y * CITY_WIDTH + x] {
            return None;
        }
        let building = self.city.building_at(x, y);
        if !is_construction_building(building) {
            return None;
        }
        let zone_id = self.city.zone_at(x, y)?.zone_id();
        if !rci_zone_id(zone_id) {
            return None;
        }
        let size = usize::from(xbld_tile_size(building).max(1));
        // GetPopulatedTileAndLevel: multi-tile construction only on the view-angle corner.
        let (origin_x, origin_y) = if size <= 1 {
            (x, y)
        } else {
            rci_view_corner_footprint_origin(&self.city, x, y, building, size)?
        };
        if !complete_building_footprint_at_origin(&self.city, origin_x, origin_y, building, size) {
            return None;
        }
        let footprint = building_footprint_from_origin(origin_x, origin_y, size)?;
        if !footprint.iter().all(|&(tx, ty)| {
            !reserved[ty * CITY_WIDTH + tx]
                && self.zone_growth_can_touch_tile(tx, ty)
                && self
                    .city
                    .zone_at(tx, ty)
                    .is_some_and(|zone| zone.zone_id() == zone_id)
                && self.city.building_at(tx, ty) == building
        }) {
            return None;
        }
        let pop = rci_building_pop_level(building)?;
        // Win95: `bPlaceChurch && (iTilePopLevel & 2) && zone < LIGHT_COMMERCIAL`
        // then `PlaceChurch` instead of ChooseAndPlace (no multi-tile edge bounds).
        let place_church =
            rci_place_church_on_construction_complete(place_church_tick, zone_id, pop);
        if !Self::pe_rci_growth_roll_succeeds_with_crt(
            rci_construction_complete_threshold(pop),
            x,
            y,
            GROWTH_ROLL_CONSTRUCTION_KEY,
            crt,
        ) {
            return None;
        }
        let quality_score = rci_development_quality_score_for_footprint(
            &self.city,
            zone_id,
            &footprint,
            city_outcomes,
            development_services,
        );
        let quality_band = rci_development_quality_band_for_score(quality_score);
        let finished = if place_church {
            Some(CHURCH_BUILDING_ID)
        } else {
            match pe_choose_and_place_then_origin_bounds(
                || {
                    rci_choose_and_place_for_city_optional(
                        &self.city,
                        zone_id,
                        pop,
                        x,
                        y,
                        quality_band,
                        Some(crt),
                    )
                },
                origin_x,
                origin_y,
                pop,
            ) {
                Some(building) => Some(building),
                None => {
                    // PE exits BUILD_THINK after its successful roll and ChooseAndPlace
                    // call, even when ChooseAndPlace rejects the raw edge coordinates.
                    // Mark this scan tile handled so StartBuilding cannot run here.
                    reserved[y * CITY_WIDTH + x] = true;
                    None
                }
            }
        };
        let Some(finished) = finished else {
            return None;
        };
        for &(tx, ty) in &footprint {
            reserved[ty * CITY_WIDTH + tx] = true;
        }
        Some(ZoneDevelopmentPlan {
            origin: (origin_x, origin_y),
            zone_id,
            building: finished,
            footprint,
            place_as_construction: false,
        })
    }

    pub(in crate::app) fn complete_rci_construction_sites_for_phase(
        &mut self,
        step: u16,
        substep: u16,
        city_outcomes: CityServiceOutcomes,
        reserved: &mut [bool],
    ) -> usize {
        let mut plans = Vec::new();
        // Win95 growth tick: one `bPlaceChurch` from leftover `0x4caa74`, not live census.
        let place_church_tick = pe_b_place_church(
            church_tile_count_for_city(&self.city),
            self.city.pe_dw_city_population(),
        );
        let power_maps = power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let development_services =
            rci_development_services_for_city_with_power(&self.city, &power_maps);
        // Construction-complete CAP CRT shares process `0x48cdd8` with the growth-tick stream.
        let mut crt = self.pe_process_crt;
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            if let Some(plan) = self.complete_rci_construction_site_at(
                x,
                y,
                city_outcomes,
                &development_services,
                place_church_tick,
                reserved,
                &mut crt,
            ) {
                plans.push(plan);
            }
        }
        self.pe_process_crt = crt;
        for plan in &plans {
            self.stamp_zone_development(plan);
        }
        if !plans.is_empty() {
            self.city.mark_all_dirty();
        }
        plans.iter().map(|plan| plan.footprint.len()).sum()
    }
}
