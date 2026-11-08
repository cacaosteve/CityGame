//! Growth-phase dispatch and player-tile eligibility.
//!
//! Keep lattice order, trip-before-development dispatch, reserved footprints and
//! native random-stream handoff intact. RCI, placement and transport workers,
//! shared models, calendar scheduling, XTRF simulation and road rendering retain
//! their existing owners.

use super::{
    CityServiceOutcomes, ViewerState, ZONE_AIRPORT, ZONE_INDUSTRIAL_DENSE, ZONE_SEAPORT,
    church_tile_count_for_city, mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng,
    pe_b_place_church, pe_choose_and_place_corner_bits, pe_class0_growth_aggregate_inputs,
    pe_class2_growth_aggregate_inputs, pe_growth_level_for_xbld,
    power_service_maps_with_entry_seed, power_water, rci_development_services_for_city_with_power,
    sim_calendar_growth_phase_tiles, sim_calendar_growth_phases,
};
use crate::city::{CITY_HEIGHT, CITY_TILES, CITY_WIDTH, TileBits, XBIT_CHOOSE_AND_PLACE};
use crate::trip_generator;
use crate::win95_random::Win95Lfsr;

impl ViewerState {
    pub(super) fn enable_zone_growth_for_player_tiles(&mut self, tiles: &[(usize, usize)]) {
        if self.engine_fidelity.uses_win95() {
            // The PE growth lattice already gates on saved XZON/XBLD/service
            // state. A player-session allowlist would suppress valid imported
            // zones and is not part of the executable contract.
            self.zone_growth_enabled = true;
            self.zone_growth_uses_allowlist = false;
            return;
        }
        let was_enabled = self.zone_growth_enabled;
        self.zone_growth_enabled = true;
        if !was_enabled || self.zone_growth_uses_allowlist {
            self.zone_growth_uses_allowlist = true;
        }
        if !self.zone_growth_uses_allowlist {
            return;
        }
        if self.zone_growth_allowed_tiles.len() != CITY_TILES {
            self.zone_growth_allowed_tiles = vec![false; CITY_TILES];
        }
        for &(x, y) in tiles {
            if x < CITY_WIDTH && y < CITY_HEIGHT {
                self.zone_growth_allowed_tiles[y * CITY_WIDTH + x] = true;
            }
        }
    }

    pub(super) fn clear_zone_growth_for_player_tiles(&mut self, tiles: &[(usize, usize)]) {
        if !self.zone_growth_uses_allowlist {
            return;
        }
        if self.zone_growth_allowed_tiles.len() != CITY_TILES {
            self.zone_growth_allowed_tiles = vec![false; CITY_TILES];
            return;
        }
        for &(x, y) in tiles {
            if x < CITY_WIDTH && y < CITY_HEIGHT {
                self.zone_growth_allowed_tiles[y * CITY_WIDTH + x] = false;
            }
        }
        if self
            .zone_growth_allowed_tiles
            .iter()
            .all(|&allowed| !allowed)
        {
            self.zone_growth_enabled = false;
        }
    }

    pub(super) fn zone_growth_can_touch_tile(&self, x: usize, y: usize) -> bool {
        if !self.zone_growth_uses_allowlist {
            return true;
        }
        x < CITY_WIDTH
            && y < CITY_HEIGHT
            && self
                .zone_growth_allowed_tiles
                .get(y * CITY_WIDTH + x)
                .copied()
                .unwrap_or(false)
    }

    /// Win95 ChooseAndPlace ORs `0xe0` into XBIT (`POWERABLE|POWERED|0x20`).
    /// That `0x80` membership is what PE `FUN_004679B0` walks; power capacity still
    /// comes from the later network rebuild.
    pub(super) fn or_choose_and_place_xbit(&mut self, x: usize, y: usize) {
        let current = self.city.tile_bits_at(x, y);
        let next = TileBits::new(current.unwrap_or_default().raw_bits() | XBIT_CHOOSE_AND_PLACE);
        if current != Some(next) {
            self.city.set_tile_bits(x, y, next);
        }
    }

    pub(super) fn sim_calendar_days_3_18_growth(&mut self) {
        // Days 3–18: same order as live `handle_simulation_day_advanced` growth days —
        // then each lattice trip→ChangeBuilding→BUILD_THINK→StartBuilding.
        // New construction waits for a later lattice visit to complete.
        debug_assert_eq!(sim_calendar_growth_phases().len(), 16);
        self.reset_monthly_growth_budget();
        for (step, substep) in sim_calendar_growth_phases() {
            self.sim_calendar_one_growth_tick(step, substep);
        }
    }

    /// Mac CODE 14 phases 3–18 scan sixteen X-major/Y-minor 4×4 residues.
    /// The local port covers the rooted class-0/1/2 and generic growth
    /// branches, powered transit-route effects, and the recovered transport,
    /// portal, and special-zone map edits. Other special/event mutations,
    /// month-wide QuickDraw interleaving, and original-Mac replay remain open;
    /// keep the PE 4×4 growth lattice disabled for Mac fidelity.
    pub(super) fn sim_calendar_mac_days_3_18_rci_scan(&mut self) {
        for phase in 0..16 {
            self.sim_calendar_mac_rci_scan_phase(phase);
        }
    }

    pub(super) fn sim_calendar_mac_rci_scan_phase(&mut self, phase: usize) {
        if phase >= 16 {
            return;
        }
        mac_budget_accumulate_monthly_rci_scan_partition_with_game_rng(
            &mut self.city,
            phase / 4,
            phase % 4,
            &mut self.mac_process_rng,
            &mut self.mac_game_rng,
            &mut self.mac_kind2_sound_deadline,
        );
    }

    fn run_growth_tick_tile_sequence_for_phase(
        &mut self,
        step: u16,
        substep: u16,
        lfsr: &mut Win95Lfsr,
        reserved: &mut [bool],
        city_outcomes: CityServiceOutcomes,
        special_reserved: &mut [bool],
    ) {
        if self.trip_served_tiles.len() != CITY_TILES {
            self.trip_served_tiles = vec![false; CITY_TILES];
        }
        // PE growth-tick CRT draws share the process `rand()` stream (`0x48cdd8`).
        let mut budget_rng = self.pe_process_crt;
        let mut surface_decayed = false;
        let mut trips_bumped = false;
        let mut declined_tiles = 0usize;
        let mut completed_tiles = 0usize;
        let mut developed_tiles = 0usize;
        let mut special_developed = 0usize;
        let mut post_decayed = 0usize;
        let corner_bits = pe_choose_and_place_corner_bits(self.city.rotation());
        let place_church_tick = pe_b_place_church(
            church_tile_count_for_city(&self.city),
            self.city.pe_dw_city_population(),
        );
        let mut power_maps =
            power_service_maps_with_entry_seed(&self.city, self.pe_process_crt.seed());
        let mut development_services =
            rci_development_services_for_city_with_power(&self.city, &power_maps);
        for (x, y) in sim_calendar_growth_phase_tiles(step, substep) {
            // Win95 DoBudgetOvergroundTransportCheck @ `0x435988`/`0x43598e`: only when
            // XZON low nibble is 0 (unzoned). Loaded/blank cities always have a zones
            // plane — unzoned is `zone_id == 0`, not `zone_at == None`.
            let zone_id = self.city.zone_at(x, y).map(|zone| zone.zone_id());
            if zone_id.is_none_or(|id| id == 0) {
                surface_decayed |=
                    self.maybe_decay_surface_transport_at(x, y, &mut budget_rng, lfsr, false) > 0;
            }
            let is_general_zone = zone_id
                .as_ref()
                .is_some_and(|zone_id| *zone_id <= ZONE_INDUSTRIAL_DENSE);
            if is_general_zone {
                // PE `FUN_00436D00` and Mac `FUN_00036c8c` read saved powered XBIT,
                // not derived connectivity. Maps refresh only after placements.
                let tile_powered =
                    |x, y| power_water::growth_powered_cross_for_engine(&power_maps, x, y);
                let stats = if self.engine_fidelity.uses_win95() {
                    trip_generator::simulate_growth_lattice_trips_with_rng(
                        &mut self.city,
                        &mut self.trip_served_tiles,
                        [(x, y)],
                        &tile_powered,
                        true,
                        &mut budget_rng,
                    )
                } else {
                    trip_generator::simulate_growth_lattice_trips(
                        &mut self.city,
                        &mut self.trip_served_tiles,
                        [(x, y)],
                        &tile_powered,
                        false,
                    )
                };
                self.trip_bus_passengers =
                    self.trip_bus_passengers.wrapping_add(stats.bus_passengers);
                self.trip_rail_passengers = self
                    .trip_rail_passengers
                    .wrapping_add(stats.rail_passengers);
                self.trip_subway_passengers = self
                    .trip_subway_passengers
                    .wrapping_add(stats.subway_passengers);
                trips_bumped |= stats.tiles_bumped > 0;
                // PE pre-roll aggregate uses the visited tile before GROWTH_CHANGE /
                // BUILD_ABANDON / StartBuilding mutate it.
                let class0_aggregate = pe_class0_growth_aggregate_inputs(&self.city, x, y);
                let class2_aggregate = pe_class2_growth_aggregate_inputs(&self.city, x, y);
                declined_tiles += self.decline_unserved_rci_zone_at(
                    x,
                    y,
                    &power_maps,
                    reserved,
                    corner_bits,
                    &mut budget_rng,
                );
                if let Some(plan) = self.complete_rci_construction_site_at(
                    x,
                    y,
                    city_outcomes,
                    &development_services,
                    place_church_tick,
                    reserved,
                    &mut budget_rng,
                ) {
                    completed_tiles += plan.footprint.len();
                    self.stamp_zone_development_with_crt(&plan, &mut budget_rng);
                    power_maps = power_service_maps_with_entry_seed(&self.city, budget_rng.seed());
                    development_services =
                        rci_development_services_for_city_with_power(&self.city, &power_maps);
                }
                if !reserved[y * CITY_WIDTH + x] {
                    let candidate = self
                        .rci_development_candidate_at_with_pre_roll(
                            x,
                            y,
                            reserved,
                            &power_maps,
                            city_outcomes,
                            &development_services,
                            Some(&mut budget_rng),
                        )
                        .or_else(|| {
                            self.rci_expansion_redevelopment_candidate_at_with_pre_roll(
                                x,
                                y,
                                reserved,
                                &power_maps,
                                city_outcomes,
                                &development_services,
                                Some(&mut budget_rng),
                            )
                        });
                    if let Some(candidate) = candidate {
                        if !candidate
                            .plan
                            .footprint
                            .iter()
                            .any(|&(tx, ty)| reserved[ty * CITY_WIDTH + tx])
                        {
                            for &(tx, ty) in &candidate.plan.footprint {
                                reserved[ty * CITY_WIDTH + tx] = true;
                            }
                            developed_tiles += candidate.plan.footprint.len();
                            self.stamp_zone_development_with_crt(&candidate.plan, &mut budget_rng);
                            power_maps =
                                power_service_maps_with_entry_seed(&self.city, budget_rng.seed());
                            development_services = rci_development_services_for_city_with_power(
                                &self.city,
                                &power_maps,
                            );
                        }
                    }
                }
                if let Some((agg_zone, agg_xbld)) = class0_aggregate {
                    // PE @ `0x436112`: zone-type slot with the pre-mutation XBLD weight.
                    self.pe_growth_aggregate
                        .add_normal_candidate(agg_zone, pe_growth_level_for_xbld(agg_xbld));
                } else if let Some(agg_xbld) = class2_aggregate {
                    // PE @ `0x4361c9`: BUILD_ABANDON slot 7 before the redevelop roll.
                    self.pe_growth_aggregate
                        .add_special_candidate(pe_growth_level_for_xbld(agg_xbld));
                }
            }
            let is_civilian_port = zone_id
                .as_ref()
                .is_some_and(|zone_id| matches!(*zone_id, ZONE_AIRPORT | ZONE_SEAPORT));
            let mut port_spawn_branch_already_selected = false;
            let mut port_growth_branch_selected = false;
            if let Some(zone_id) = zone_id {
                if let Some((growth_branch_selected, may_stamp)) = self
                    .special_zone_growth_tick_branch(
                        zone_id,
                        special_reserved[y * CITY_WIDTH + x],
                        &mut budget_rng,
                    )
                {
                    port_growth_branch_selected = is_civilian_port && growth_branch_selected;
                    if growth_branch_selected {
                        if may_stamp {
                            if let Some(plan) = self.special_zone_development_plan_at(
                                x,
                                y,
                                special_reserved,
                                &power_maps,
                            ) {
                                if plan.footprint.iter().all(|&(tx, ty)| {
                                    self.zone_growth_can_touch_tile(tx, ty)
                                        && !special_reserved[ty * CITY_WIDTH + tx]
                                }) {
                                    for &(tx, ty) in &plan.footprint {
                                        special_reserved[ty * CITY_WIDTH + tx] = true;
                                    }
                                    special_developed += plan.footprint.len();
                                    self.stamp_zone_development_with_crt(&plan, &mut budget_rng);
                                    power_maps = power_service_maps_with_entry_seed(
                                        &self.city,
                                        budget_rng.seed(),
                                    );
                                    development_services =
                                        rci_development_services_for_city_with_power(
                                            &self.city,
                                            &power_maps,
                                        );
                                }
                            }
                        }
                    } else if is_civilian_port {
                        port_spawn_branch_already_selected = true;
                    }
                }
            }
            if !is_civilian_port || !port_growth_branch_selected {
                self.maybe_spawn_ambient_port_traffic_at_with_crt(
                    x,
                    y,
                    &mut budget_rng,
                    false,
                    port_spawn_branch_already_selected,
                );
            }
            // The PE's final zone dispatch reaches DoBudgetSubwayCheck only for
            // unclassified nonzero XZON nibbles (> seaport). Unzoned transport
            // and every handled zone 1..=9 jump to the shared tile epilogue first.
            if zone_id.is_some_and(|zone_id| zone_id > ZONE_SEAPORT) {
                post_decayed = post_decayed.saturating_add(self.maybe_decay_subway_at(
                    x,
                    y,
                    &mut budget_rng,
                    lfsr,
                    false,
                ));
            }
        }
        self.pe_process_crt = budget_rng;
        self.persist_transit_passengers_to_city();
        if surface_decayed
            || trips_bumped
            || declined_tiles > 0
            || completed_tiles > 0
            || developed_tiles > 0
            || special_developed > 0
            || post_decayed > 0
        {
            self.pending_traffic_redraw = true;
            self.city.mark_all_dirty();
        }
    }

    pub(super) fn sim_calendar_one_growth_tick(&mut self, step: u16, substep: u16) {
        // Win95 Game_Random (`0x4cdb7c`) is process-wide across growth-tick dispatches.
        let mut growth_lfsr = self.pe_process_lfsr;
        let mut rci_reserved = vec![false; CITY_TILES];
        let mut special_reserved = vec![false; CITY_TILES];
        let outcomes = self.live_city_service_outcomes_with_process_crt();
        // Win95: each lattice tile runs surface budget, trips, then BUILD_START decline.
        self.run_growth_tick_tile_sequence_for_phase(
            step,
            substep,
            &mut growth_lfsr,
            &mut rci_reserved,
            outcomes,
            &mut special_reserved,
        );
        self.pe_process_lfsr = growth_lfsr;
        // Win95 successful GROWTH_CHANGE / GROWTH_COMPLETECONSTRUCTION jump to the
        // shared tile exit. A failed GROWTH_CHANGE roll returns to state dispatch,
        // leaves the tile unreserved, and can reach normal StartBuilding gates. A
        // BUILD_ABANDON path also exits; failed BUILD_THINK may reach StartBuilding.
    }
}
