use super::*;

impl ViewerState {
    pub(in crate::app) fn special_zone_growth_roll_allows(
        &self,
        rng: &mut Win95CrtRand,
        zone_id: u8,
    ) -> bool {
        // PE SimulationGrow special-zone arms: `call rand` / `test al, 3` before picker.
        // Military army/AF/navy @ `0x436304`/`0x436377`/`0x436462`: `jne` skip → grow when
        // `(al&3)==0`. Seaport @ `0x43652b`: `je` to crane/cargo picker when `(al&3)==0`.
        // Airport @ `0x436601`: `je` to civilian ladder @ `0x43670d` when `(al&3)==0`
        // (nonzero fallthrough is runway-position checks, not the building picker).
        // Silo monthly @ `0x4364f8`: no `rand()&3` gate — Place `0xf9` when XBLD != `0xf9`.
        match zone_id {
            ZONE_AIRPORT | ZONE_SEAPORT => (rng.random_u16() & 3) == 0,
            ZONE_MILITARY => {
                if military_base_type_for_city(&self.city) == MILITARY_BASE_MISSILE_SILOS {
                    true
                } else {
                    (rng.random_u16() & 3) == 0
                }
            }
            _ => true,
        }
    }

    /// PE consumes the special-zone roll for every dispatched lattice cell, including a
    /// cell already covered by a same-phase placement. The reservation affects stamping,
    /// not the roll or whether PE selected the growth-vs-ambient branch.
    pub(in crate::app) fn special_zone_growth_tick_branch(
        &self,
        zone_id: u8,
        footprint_reserved: bool,
        rng: &mut Win95CrtRand,
    ) -> Option<(bool, bool)> {
        if zone_id <= ZONE_INDUSTRIAL_DENSE || special_zone_kind_index(zone_id).is_none() {
            return None;
        }
        let growth_branch_selected = self.special_zone_growth_roll_allows(rng, zone_id);
        Some((
            growth_branch_selected,
            growth_branch_selected && !footprint_reserved,
        ))
    }

    pub(in crate::app) fn special_zone_development_plan_at(
        &self,
        x: usize,
        y: usize,
        reserved: &[bool],
        power_maps: &PowerServiceMaps,
    ) -> Option<ZoneDevelopmentPlan> {
        let zone_id = self.city.zone_at(x, y)?.zone_id();
        let pe_selector_may_own_candidate =
            self.engine_fidelity.uses_win95() && matches!(zone_id, ZONE_AIRPORT | ZONE_SEAPORT);
        let population_gate_failed =
            !pe_selector_may_own_candidate && !special_zone_has_growth_demand(&self.city, zone_id);
        if !matches!(zone_id, ZONE_AIRPORT | ZONE_SEAPORT | ZONE_MILITARY)
            || population_gate_failed
            || !self.special_zone_technology_available(zone_id)
            || (!pe_selector_may_own_candidate
                && zone_id != ZONE_MILITARY
                && !(zone_id == ZONE_AIRPORT
                    && select_airport_growth_building(&self.city) == RUNWAY_BUILDING_ID)
                && !self.can_develop_zone_tile(x, y))
        {
            return None;
        }

        match zone_id {
            ZONE_MILITARY => self.military_zone_development_plan_at(x, y, reserved, power_maps),
            ZONE_AIRPORT => {
                let selected = select_airport_growth_building(&self.city);
                self.special_zone_development_plan_for_building(
                    x, y, reserved, power_maps, zone_id, selected,
                )
            }
            ZONE_SEAPORT => {
                let selected = select_seaport_growth_building(&self.city);
                if self.engine_fidelity.uses_win95() {
                    self.special_zone_development_plan_for_building(
                        x, y, reserved, power_maps, zone_id, selected,
                    )
                } else if let Some(plan) = self.special_zone_development_plan_for_building(
                    x, y, reserved, power_maps, zone_id, selected,
                ) {
                    Some(plan)
                } else if selected == MILITARY_WAREHOUSE_BUILDING_ID {
                    None
                } else {
                    self.special_zone_development_plan_for_building(
                        x,
                        y,
                        reserved,
                        power_maps,
                        zone_id,
                        MILITARY_WAREHOUSE_BUILDING_ID,
                    )
                }
            }
            _ => None,
        }
    }

    pub(in crate::app) fn military_zone_development_plan_at(
        &self,
        x: usize,
        y: usize,
        reserved: &[bool],
        power_maps: &PowerServiceMaps,
    ) -> Option<ZoneDevelopmentPlan> {
        let base_type = military_base_type_for_city(&self.city);
        if base_type == MILITARY_BASE_MISSILE_SILOS {
            return self.special_zone_development_plan_for_building(
                x,
                y,
                reserved,
                power_maps,
                ZONE_MILITARY,
                MILITARY_SILO_BUILDING_ID,
            );
        }
        let selected = select_military_growth_building(&self.city, base_type)?;
        if let Some(plan) = self.special_zone_development_plan_for_building(
            x,
            y,
            reserved,
            power_maps,
            ZONE_MILITARY,
            selected,
        ) {
            return Some(plan);
        }
        let fallback = military_growth_fallback_building(base_type)?;
        if fallback == selected {
            return None;
        }
        self.special_zone_development_plan_for_building(
            x,
            y,
            reserved,
            power_maps,
            ZONE_MILITARY,
            fallback,
        )
    }

    pub(in crate::app) fn special_zone_development_plan_for_building(
        &self,
        x: usize,
        y: usize,
        reserved: &[bool],
        power_maps: &PowerServiceMaps,
        zone_id: u8,
        building: u8,
    ) -> Option<ZoneDevelopmentPlan> {
        let size = usize::from(xbld_tile_size(building));
        let pe_selector_owns_footprint = (zone_id == ZONE_MILITARY
            && (size == 2
                || building == MILITARY_SILO_BUILDING_ID
                || pe_army_selector_owns_footprint(building)))
            || (self.engine_fidelity.uses_win95()
                && zone_id == ZONE_AIRPORT
                && building != RUNWAY_BUILDING_ID)
            || (self.engine_fidelity.uses_win95()
                && zone_id == ZONE_SEAPORT
                && matches!(
                    building,
                    SEAPORT_CRANE_BUILDING_ID
                        | CIV_LOADING_BAY_BUILDING_ID
                        | MILITARY_WAREHOUSE_BUILDING_ID
                        | MILITARY_CARGO_YARD_BUILDING_ID
                ));
        let (origin_x, origin_y) = if building == MILITARY_SILO_BUILDING_ID {
            pe_army_missile_silo_origin_438d6b(&self.city, x, y, zone_id)
        } else if size <= 1 {
            (x, y)
        } else if self.city.building_at(x, y) == building && !pe_selector_owns_footprint {
            // Existing multi-tile footprints are not re-anchored via containing-origin;
            // vacant growth uses even-align only (GetItemPlacementAreaAndFarPosition).
            return None;
        } else {
            (x - (x & 1), y - (y & 1))
        };
        let footprint = building_footprint_from_origin(origin_x, origin_y, size)?;
        // The class-0 callers ignore refusal, so a rejected parity/ray candidate must
        // not create a plan or reserve a cell that blocks later growth candidates.
        if self.engine_fidelity.uses_win95()
            && matches!(zone_id, ZONE_AIRPORT | ZONE_MILITARY)
            && building == RUNWAY_BUILDING_ID
            && pe_growth_runway_class0_ray_43830d(&self.city, x, y, zone_id).is_none()
        {
            return None;
        }
        // Civilian airport/seaport ladder entries in selector classes 4/5 share the
        // rooted even-aligned 2×2 probe. In particular, its NW XBLD >= 0xEB refusal
        // must happen before we reserve a footprint; otherwise a failed selector
        // attempt can shadow a later valid candidate during this Rust scan phase.
        if self.engine_fidelity.uses_win95()
            && matches!(zone_id, ZONE_AIRPORT | ZONE_SEAPORT)
            && matches!(building, 0xee..=0xf2 | 0xf6)
            && matches!(
                pe_army_2x2_place_probe_4389bf(&self.city, x, y, zone_id),
                PeArmy2x2Probe::Refuse
            )
        {
            return None;
        }
        // PE class 4/5 performs its own footprint probe and clear pass:
        // `FUN_004392A0` may remove selected existing XBLD before PlaceTile-size(2).
        // Class 6's silo arm walks to an origin, then writes a 3x3 plot without checking
        // the destination footprint's zone, occupancy, or terrain. Let those PE paths
        // own their mutations instead of prefiltering them as civilian growth does.
        let transport_ok =
            zone_id == ZONE_MILITARY || self.development_footprint_has_transport_access(&footprint);
        if footprint.iter().all(|&(tx, ty)| {
            !reserved[ty * CITY_WIDTH + tx]
                && (self
                    .city
                    .zone_at(tx, ty)
                    .is_some_and(|zone| zone.zone_id() == zone_id)
                    || (zone_id == ZONE_MILITARY && building == MILITARY_SILO_BUILDING_ID))
                && (pe_selector_owns_footprint || self.can_develop_zone_tile(tx, ty))
        }) && (pe_selector_owns_footprint || self.development_footprint_is_flat_land(&footprint))
            && transport_ok
            // Both editions gate at the visited tile (military skips).
            && (zone_id == ZONE_MILITARY
                || power_water::growth_powered_cross_for_engine(power_maps, x, y))
        {
            Some(ZoneDevelopmentPlan {
                origin: (origin_x, origin_y),
                zone_id,
                building,
                footprint,
                place_as_construction: false,
            })
        } else {
            None
        }
    }

    pub(in crate::app) fn special_zone_technology_available(&self, zone_id: u8) -> bool {
        match zone_id {
            ZONE_AIRPORT => self.tool_technology_available(EditTool::Airport),
            _ => true,
        }
    }

    pub(in crate::app) fn stamp_zone_development(&mut self, plan: &ZoneDevelopmentPlan) {
        // Isolated callers (tests / non-growth paths) use the process CRT field.
        let mut crt = self.pe_process_crt;
        self.stamp_zone_development_with_crt(plan, &mut crt);
        self.pe_process_crt = crt;
    }

    /// ChooseAndPlace construction mode-3 CRT (`0x48cdd8`) on the caller stream.
    /// Growth-tick / `develop_rci` pass `budget_rng` / `growth_rng` so writeback keeps those draws.
    pub(in crate::app) fn stamp_zone_development_with_crt(
        &mut self,
        plan: &ZoneDevelopmentPlan,
        crt: &mut Win95CrtRand,
    ) {
        let target_pop = rci_building_pop_level(plan.building).unwrap_or(1);
        let stamped_building =
            if plan.place_as_construction && !is_construction_building(plan.building) {
                construction_building_for_pop_optional(
                    target_pop,
                    plan.origin.0,
                    plan.origin.1,
                    Some(crt),
                )
                .unwrap_or(plan.building)
            } else {
                plan.building
            };
        // PE DoArmyBaseGrowth class-0 runway arm @ `0x43830d`..`0x438603`: the selector
        // owns orientation, five-cell zone preflight, DD/DE runway stamping, and refusal.
        // Its caller ignores failure and never falls back to the generic size-1 stamp.
        if (plan.zone_id == ZONE_MILITARY || self.engine_fidelity.uses_win95())
            && matches!(plan.zone_id, ZONE_MILITARY | ZONE_AIRPORT)
            && stamped_building == RUNWAY_BUILDING_ID
        {
            pe_growth_runway_class0_43830d(
                &mut self.city,
                plan.origin.0,
                plan.origin.1,
                plan.zone_id,
            );
            return;
        }
        // PE DoArmyBaseGrowth class-1 crane arm @ `0x43862d`..`0x43890d`: failed
        // direction/parity/ray gates return zero with no placement. Navy caller tests
        // that result at `0x4364d7` and retries class-4/5 warehouse `0xe3` at `0x4364eb`.
        // The accepted path owns its XBLD clear, stamp, zone write, and four-cell strip.
        if plan.zone_id == ZONE_MILITARY && stamped_building == MILITARY_CRANE_BUILDING_ID {
            let (ox, oy) = plan.origin;
            if pe_army_crane_place_and_zone_438809(&mut self.city, ox, oy, ZONE_MILITARY) {
                self.apply_building_underground_side_effects(ox, oy, stamped_building);
            } else if military_base_type_for_city(&self.city) == MILITARY_BASE_NAVY
                && matches!(
                    pe_army_2x2_place_tile_size_438b8e(
                        &mut self.city,
                        ox,
                        oy,
                        MILITARY_WAREHOUSE_BUILDING_ID,
                        ZONE_MILITARY,
                    ),
                    PeArmy2x2Result::Accepted { placed: true }
                )
            {
                self.apply_building_underground_side_effects(
                    ox & !1,
                    oy & !1,
                    MILITARY_WAREHOUSE_BUILDING_ID,
                );
            }
            return;
        }
        // PE DoArmyBaseGrowth 2×2 PlaceTile-size @ `0x438b8e` (hangar/parking/…).
        // PE caller retries only when the 2×2 selector returns zero from its probe gates:
        // army parking/hangar1 @ `0x436351`/`0x436365` → hangar1 `0xe8`;
        // navy warehouse/cargo has the analogous selector-zero branch.
        if plan.zone_id == ZONE_MILITARY && usize::from(xbld_tile_size(stamped_building)) == 2 {
            let selected_result = pe_army_2x2_place_tile_size_438b8e(
                &mut self.city,
                plan.origin.0,
                plan.origin.1,
                stamped_building,
                ZONE_MILITARY,
            );
            if let PeArmy2x2Result::Accepted { placed } = selected_result {
                if placed {
                    self.apply_building_underground_side_effects(
                        plan.origin.0,
                        plan.origin.1,
                        stamped_building,
                    );
                }
                // PE returns one after the selector guards pass, even when PlaceTile
                // itself refused; the outer growth caller retries only on selector zero.
                return;
            }
            let retry = if matches!(
                stamped_building,
                MILITARY_PARKING_LOT_BUILDING_ID | MILITARY_HANGAR1_BUILDING_ID
            ) && military_base_type_for_city(&self.city) == MILITARY_BASE_ARMY
            {
                Some(MILITARY_HANGAR1_BUILDING_ID)
            } else if matches!(
                stamped_building,
                MILITARY_WAREHOUSE_BUILDING_ID | MILITARY_CARGO_YARD_BUILDING_ID
            ) {
                Some(MILITARY_WAREHOUSE_BUILDING_ID)
            } else {
                None
            };
            if let Some(fallback) = retry.filter(|&b| b != stamped_building) {
                if matches!(
                    pe_army_2x2_place_tile_size_438b8e(
                        &mut self.city,
                        plan.origin.0,
                        plan.origin.1,
                        fallback,
                        ZONE_MILITARY,
                    ),
                    PeArmy2x2Result::Accepted { placed: true }
                ) {
                    self.apply_building_underground_side_effects(
                        plan.origin.0,
                        plan.origin.1,
                        fallback,
                    );
                }
            }
            return;
        }
        // PE SimulationGrow civilian airport @ `0x4367fe` dispatches every selected
        // ladder ID through FUN_004382b0 and ignores its return. Runway `0xdd` is handled
        // above by class 0; the remaining IDs use classes 2–5 with no retry.
        if self.engine_fidelity.uses_win95() && plan.zone_id == ZONE_AIRPORT {
            let (ox, oy) = plan.origin;
            match usize::from(xbld_tile_size(stamped_building)) {
                1 => {
                    if matches!(
                        pe_army_place_tile_size_if_xbld_below_0x0d_43893f(
                            &mut self.city,
                            ox,
                            oy,
                            stamped_building,
                            ZONE_AIRPORT,
                        ),
                        PeArmySize1Result::Accepted { placed: true }
                    ) {
                        self.apply_building_underground_side_effects(ox, oy, stamped_building);
                    }
                }
                2 => {
                    if matches!(
                        pe_army_2x2_place_tile_size_438b8e(
                            &mut self.city,
                            ox,
                            oy,
                            stamped_building,
                            ZONE_AIRPORT,
                        ),
                        PeArmy2x2Result::Accepted { placed: true }
                    ) {
                        self.apply_building_underground_side_effects(ox, oy, stamped_building);
                    }
                }
                _ => {}
            }
            return;
        }
        if !self.engine_fidelity.uses_win95()
            && plan.zone_id == ZONE_AIRPORT
            && usize::from(xbld_tile_size(stamped_building)) >= 2
        {
            let (ox, oy) = plan.origin;
            let size = usize::from(xbld_tile_size(stamped_building));
            if pe_place_tile_size_440c50(&mut self.city, ox, oy, stamped_building, size) {
                self.apply_building_underground_side_effects(ox, oy, stamped_building);
            }
            return;
        }
        // PE SimulationGrow civilian seaport calls FUN_004382b0: class-1 crane refusal
        // and class-4/5 2×2 refusal both retry warehouse `0xe3`; accepted-but-unplaced is
        // still selector success and must not retry. All calls use the raw scan coordinate.
        if self.engine_fidelity.uses_win95()
            && plan.zone_id == ZONE_SEAPORT
            && matches!(
                stamped_building,
                SEAPORT_CRANE_BUILDING_ID
                    | CIV_LOADING_BAY_BUILDING_ID
                    | MILITARY_WAREHOUSE_BUILDING_ID
                    | MILITARY_CARGO_YARD_BUILDING_ID
            )
        {
            let (ox, oy) = plan.origin;
            let selector_accepted = if stamped_building == SEAPORT_CRANE_BUILDING_ID {
                let accepted =
                    pe_army_crane_place_and_zone_438809(&mut self.city, ox, oy, ZONE_SEAPORT);
                if accepted && self.city.building_at(ox, oy) == stamped_building {
                    self.apply_building_underground_side_effects(ox, oy, stamped_building);
                }
                accepted
            } else {
                let result = pe_army_2x2_place_tile_size_438b8e(
                    &mut self.city,
                    ox,
                    oy,
                    stamped_building,
                    ZONE_SEAPORT,
                );
                if let PeArmy2x2Result::Accepted { placed: true } = result {
                    self.apply_building_underground_side_effects(ox, oy, stamped_building);
                }
                matches!(result, PeArmy2x2Result::Accepted { .. })
            };
            if !selector_accepted {
                let fallback_result = pe_army_2x2_place_tile_size_438b8e(
                    &mut self.city,
                    ox,
                    oy,
                    MILITARY_WAREHOUSE_BUILDING_ID,
                    ZONE_SEAPORT,
                );
                if matches!(fallback_result, PeArmy2x2Result::Accepted { placed: true }) {
                    self.apply_building_underground_side_effects(
                        ox & !1,
                        oy & !1,
                        MILITARY_WAREHOUSE_BUILDING_ID,
                    );
                }
            }
            return;
        }
        // Mac's special-zone placement path has not been mapped to the PE selector.
        // Preserve its prior generic PlaceTile behavior until its caller is rooted.
        if !self.engine_fidelity.uses_win95()
            && plan.zone_id == ZONE_SEAPORT
            && matches!(
                stamped_building,
                SEAPORT_CRANE_BUILDING_ID
                    | CIV_LOADING_BAY_BUILDING_ID
                    | MILITARY_WAREHOUSE_BUILDING_ID
                    | MILITARY_CARGO_YARD_BUILDING_ID
            )
        {
            let size = usize::from(xbld_tile_size(stamped_building).max(1));
            let (ox, oy) = plan.origin;
            if pe_place_tile_size_440c50(&mut self.city, ox, oy, stamped_building, size) {
                self.apply_building_underground_side_effects(ox, oy, stamped_building);
                return;
            }
            if stamped_building != MILITARY_WAREHOUSE_BUILDING_ID
                && pe_place_tile_size_440c50(
                    &mut self.city,
                    ox,
                    oy,
                    MILITARY_WAREHOUSE_BUILDING_ID,
                    size,
                )
            {
                self.apply_building_underground_side_effects(
                    ox,
                    oy,
                    MILITARY_WAREHOUSE_BUILDING_ID,
                );
            }
            return;
        }
        // PE DoArmyBaseGrowth size-1 PlaceTile-size @ `0x43893f` when XBLD `< 0x0D`.
        // Missile silo (`0xf9`) uses PlaceTile+PlaceXUND (`0x438def`), not this arm.
        if plan.zone_id == ZONE_MILITARY
            && usize::from(xbld_tile_size(stamped_building)) == 1
            && stamped_building != MILITARY_CRANE_BUILDING_ID
            && stamped_building != MILITARY_SILO_BUILDING_ID
        {
            match pe_army_place_tile_size_if_xbld_below_0x0d_43893f(
                &mut self.city,
                plan.origin.0,
                plan.origin.1,
                stamped_building,
                ZONE_MILITARY,
            ) {
                PeArmySize1Result::NoopXbldAtOrAbove0x0d => return,
                PeArmySize1Result::Accepted { .. } => {
                    self.apply_building_underground_side_effects(
                        plan.origin.0,
                        plan.origin.1,
                        stamped_building,
                    );
                    return;
                }
            }
        }
        // PE SimulationGrow silo monthly @ `0x4364f8` → `0x4026b2`/`0x4382b0` class-6
        // → DoArmyBaseGrowth silo plot @ `0x438def`/`0x438e0a`/`0x438e25` (no CRT gate).
        if plan.zone_id == ZONE_MILITARY && stamped_building == MILITARY_SILO_BUILDING_ID {
            pe_army_missile_silo_place_plot_438def(&mut self.city, plan.origin.0, plan.origin.1);
            return;
        }
        // PE DoArmyBaseGrowth refuses do not fall through to ChooseAndPlace.
        if plan.zone_id == ZONE_MILITARY {
            let sz = usize::from(xbld_tile_size(stamped_building));
            if stamped_building == MILITARY_CRANE_BUILDING_ID
                || sz == 2
                || (sz == 1
                    && stamped_building != MILITARY_CRANE_BUILDING_ID
                    && stamped_building != MILITARY_SILO_BUILDING_ID)
            {
                return;
            }
        }
        // PE BUILD_THINK @ `0x43619a` → `FUN_00437310` PlaceChurch (not ChooseAndPlace).
        if stamped_building == CHURCH_BUILDING_ID {
            let (ox, oy) = plan.origin;
            if let Some(sw_y) = oy.checked_add(1) {
                pe_place_church_437310(&mut self.city, ox, sw_y);
            }
            return;
        }
        let size = usize::from(xbld_tile_size(stamped_building).max(1));
        let corner_bits = pe_choose_and_place_corner_bits(self.city.rotation());
        for &(tx, ty) in &plan.footprint {
            // PE ChooseAndPlace `0x436e80` → PlaceTile `0x40178f` / `0x441f00` (XBLD only).
            pe_place_tile_441f00(&mut self.city, tx, ty, stamped_building);
            // PE ChooseAndPlace only ORs XBIT `0xe0` (`0x436fad` / `0x4370cb`); do not clear watered.
            self.or_choose_and_place_xbit(tx, ty);
            self.city.set_zone(
                tx,
                ty,
                plan.zone_id
                    | building_zone_bits_for_tile(
                        plan.origin.0,
                        plan.origin.1,
                        tx,
                        ty,
                        size,
                        corner_bits,
                    ),
            );
            // PE ChooseAndPlace / PlaceTile never clear XTRF (`0x4ca940`).
        }
        if matches!(
            stamped_building,
            RUNWAY_BUILDING_ID | AIRPORT_RUNWAY_CROSS_BUILDING_ID
        ) {
            // PE/sc2kfix runway place: XBITSetBits(POWERED | POWERABLE).
            for &(tx, ty) in &plan.footprint {
                let bits = self
                    .city
                    .tile_bits_at(tx, ty)
                    .unwrap_or(TileBits::new(0))
                    .with_powered(true)
                    .with_powerable(true);
                self.city.set_tile_bits(tx, ty, bits);
            }
        }
        self.apply_building_underground_side_effects(
            plan.origin.0,
            plan.origin.1,
            stamped_building,
        );
    }
}
